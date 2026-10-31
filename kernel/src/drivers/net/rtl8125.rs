// SPDX-License-Identifier: MPL-2.0
//! Realtek RTL8125 2.5GbE driver, rung 1: bind, identify, MAC, PHY link,
//! TX/RX descriptor rings, and a [`NetDevice`] the existing stack runs DHCP on.
//!
//! **Kill-switch.** Compiled only under `rtl8125`; off ⇒ nothing here exists
//! and the kernel is bit-for-bit `main`.
//!
//! **DMA containment (I-14(c)), fail-closed.** Every ring and packet buffer is
//! allocated in the fenced DMA arena, and every address written to the chip
//! (ring bases, descriptor buffer addresses) is a [`FencedDmaAddr`]: the
//! functions that program the chip take nothing else. Whether the chip may
//! bus-master at all is [`logic::containment`]: with the device in the VT-d
//! fence domain (`dma=iommu`), or — on a host with no supported IOMMU, e.g.
//! the AMD main PC — only with the separate `rtl8125-dma-without-iommu`
//! opt-in (`dma=arena-only`). Otherwise (`dma=blocked (no iommu)`) the driver
//! binds, reads the MAC and the link, reports, and stops with bus mastering
//! off; it clears a Bus Master Enable that firmware left on. The one call
//! that sets Bus Master Enable is in [`grant_bus_master`], behind
//! [`logic::bus_master_allowed`], after the rings are programmed (trap 4).
//!
//! **Interrupts.** Rung 1 runs polled (`irq=polled`): IMR is zero and INTx is
//! disabled in PCI COMMAND, so the device cannot interrupt at all. The
//! service path ([`NetDevice::service`]) acknowledges status, reaps both rings
//! with a per-pass budget, and feeds the storm detector (I-14(b)): a status
//! bit that re-asserts on every pass with no ring progress is declared a
//! storm and, past the limit, masked out of the per-pass work. A TX ring that
//! stops completing is reported `tx=wedged` and refuses new frames; nothing
//! here waits without a bound.
//!
//! **Locks (I-14(a)).** One device lock, ordered at
//! [`lock_order::NET_DRIVER`](crate::sync::lock_order::NET_DRIVER), above the
//! whole AI-governance band: acquiring any governance lock under it is a
//! lockdep panic in debug/neo-test builds. It is never held across anything
//! but this device's MMIO and the arena span table (an unordered leaf).
//!
//! Prior art read for facts and traps only: OpenBSD `rge(4)` (ISC) and
//! Linux `r8169` (GPL, nothing copied). See `rtl8125_logic.rs`.

extern crate alloc;

use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Write as _;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering, fence};

use x86_64::VirtAddr;
use x86_64::instructions::port::Port;

use crate::drivers::block::DmaBuffer;
use crate::drivers::dma_arena::{self, FencedDmaAddr};
use crate::drivers::pci::{self, Device};
use crate::net::{LinkState, NetDevice, NetDeviceIdentity, NetError, NetResult, RxFrame, TxFrame};
use crate::serial_println;
use crate::sync::SwitchMutex;

#[path = "rtl8125_logic.rs"]
mod rtl8125_logic;
use rtl8125_logic as logic;
use rtl8125_logic::{DmaMode, MacVersion, reg};

// --- boot evidence (image A's on-screen summary) --------------------------------------------

/// The key boot lines (bind, DHCP outcome, the counters/`gw-arp` line, or
/// `absent`), kept so [`evidence_summary`] can print them again just before
/// the desktop takes the screen: by then they have scrolled off the boot
/// console, which is the only evidence on a machine with no serial port.
/// Boot-time only; the service/poll path keeps nothing.
#[cfg(any(feature = "fb-progress-console", feature = "neo-test-rtl8125-absent"))]
static BOOT_LINES: spin::Mutex<Vec<String>> = spin::Mutex::new(Vec::new());

/// Keep one boot line. A DHCP discover line replaces the previous one, so the
/// summary shows the last attempt only.
#[cfg(any(feature = "fb-progress-console", feature = "neo-test-rtl8125-absent"))]
fn keep(line: String) {
    const DISCOVER: &str = " dhcp discover attempt=";
    let mut lines = BOOT_LINES.lock();
    if line.contains(DISCOVER) && lines.last().is_some_and(|l| l.contains(DISCOVER)) {
        lines.pop();
    }
    lines.push(line);
}

/// A boot line: printed, and kept for [`evidence_summary`]. Without the boot
/// console this is exactly `serial_println!`.
macro_rules! say {
    ($($arg:tt)*) => {{
        #[cfg(any(feature = "fb-progress-console", feature = "neo-test-rtl8125-absent"))]
        {
            let line = alloc::format!($($arg)*);
            serial_println!("{}", line);
            keep(line);
        }
        #[cfg(not(any(feature = "fb-progress-console", feature = "neo-test-rtl8125-absent")))]
        serial_println!($($arg)*);
    }};
}

/// Header of the summary block.
#[cfg(any(feature = "fb-progress-console", feature = "neo-test-rtl8125-absent"))]
pub const SUMMARY_HEADER: &str = "rtl8125-summary: ---- Ethernet evidence ----";

/// Print the kept boot lines again under [`SUMMARY_HEADER`], as the
/// last-but-one block on the boot console (the touchpad check follows).
/// Returns how many lines were re-printed.
#[cfg(any(feature = "fb-progress-console", feature = "neo-test-rtl8125-absent"))]
pub fn evidence_summary() -> usize {
    let lines = BOOT_LINES.lock();
    serial_println!("{}", SUMMARY_HEADER);
    if lines.is_empty() {
        serial_println!("rtl8125: (driver did not run)");
    }
    for line in lines.iter() {
        serial_println!("{}", line);
    }
    crate::log::flush();
    lines.len()
}

/// Test hook: the kept lines.
#[cfg(feature = "neo-test-rtl8125-absent")]
pub fn kept_boot_lines() -> Vec<String> {
    BOOT_LINES.lock().clone()
}

const VENDOR_REALTEK: u16 = 0x10ec;
const DEVICE_RTL8125: u16 = 0x8125;
/// The RTL8126 (5GbE) shares much of the register map but has its own PHY,
/// RX config and XIDs. Rung 1 recognises it and refuses it by name.
const DEVICE_RTL8126: u16 = 0x8126;

const TX_SLOTS: usize = 8;
const RX_SLOTS: usize = 16;
/// Extra RX buffers beyond the ring, so frames the stack holds do not leave
/// the ring short.
const RX_SPARE: usize = 8;
/// One arena page per buffer; 2 KiB of it is the frame buffer.
const BUF_LEN: usize = 2048;
const MAX_FRAME: usize = 1514;
const MIN_FRAME: usize = 60;
/// Arena pages to reserve when no earlier driver (xHCI) did.
const ARENA_PAGES: u64 = 64;

/// Per-pass reap budget per ring (I-14(b): a pass is bounded).
const REAP_BUDGET: usize = 32;
/// Service passes with TX in flight and no completion before the TX ring is
/// declared wedged.
const TX_WEDGE_PASSES: u32 = 200_000;

/// Bounded waits, in microseconds (see [`udelay`]).
const RESET_WAIT_US: u32 = 10_000;
const FIFO_DRAIN_WAIT_US: u32 = 150_000;
const PHY_READY_WAIT_US: u32 = 100_000;
/// Autonegotiation to 2.5G can take several seconds (trap 9).
const AUTONEG_WAIT_US: u32 = 6_000_000;
const OCP_WAIT_US: u32 = 2_000;
const D0_SETTLE_US: u32 = 10_000;

const PCI_COMMAND: u8 = 0x04;
const PCI_CAP_PM: u8 = 0x01;
const PCI_CAP_EXP: u8 = 0x10;
/// PCIe capability: Link Control register offset.
const PCIE_LINK_CONTROL: u8 = 0x10;

// --- module status (the scenario and the console line read these) ------------

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BindStatus {
    NotRun,
    Absent,
    Unsupported,
    Refused,
    DmaBlocked,
    Live,
}

impl BindStatus {
    const fn code(self) -> u8 {
        match self {
            Self::NotRun => 0,
            Self::Absent => 1,
            Self::Unsupported => 2,
            Self::Refused => 3,
            Self::DmaBlocked => 4,
            Self::Live => 5,
        }
    }
    const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Absent,
            2 => Self::Unsupported,
            3 => Self::Refused,
            4 => Self::DmaBlocked,
            5 => Self::Live,
            _ => Self::NotRun,
        }
    }
}

static STATUS: AtomicU8 = AtomicU8::new(0);
/// Times this driver granted Bus Master Enable (0 unless DMA is allowed).
static BUS_MASTER_GRANTS: AtomicU32 = AtomicU32::new(0);

#[allow(dead_code)] // gate seam: read by the rtl8125 scenarios
pub fn status() -> BindStatus {
    BindStatus::from_code(STATUS.load(Ordering::Acquire))
}

#[allow(dead_code)] // gate seam: read by the rtl8125 scenarios
pub fn bus_master_grants() -> u32 {
    BUS_MASTER_GRANTS.load(Ordering::Acquire)
}

fn set_status(s: BindStatus) {
    STATUS.store(s.code(), Ordering::Release);
}

// --- small helpers -----------------------------------------------------------------

/// About one microsecond: a write to the POST diagnostic port (the classic
/// I/O delay). Only ever used to bound a wait, never to time anything.
fn udelay(us: u32) {
    let mut port = Port::<u8>::new(0x80);
    for _ in 0..us {
        // SAFETY: port 0x80 is the POST code port; writes have no effect
        // beyond the bus cycle that provides the delay.
        unsafe { port.write(0) };
    }
}

/// The device's MMIO window (uncached; mapped by `pci::map_bar_region_validated`).
#[derive(Copy, Clone)]
struct Regs {
    base: VirtAddr,
}

impl Regs {
    fn ptr<T>(&self, offset: usize) -> *mut T {
        (self.base.as_u64() as usize + offset) as *mut T
    }
    fn r8(&self, offset: usize) -> u8 {
        // SAFETY: `offset` is a register inside the validated BAR mapping.
        unsafe { read_volatile(self.ptr::<u8>(offset)) }
    }
    fn r16(&self, offset: usize) -> u16 {
        // SAFETY: as above; registers are naturally aligned.
        u16::from_le(unsafe { read_volatile(self.ptr::<u16>(offset)) })
    }
    fn r32(&self, offset: usize) -> u32 {
        // SAFETY: as above.
        u32::from_le(unsafe { read_volatile(self.ptr::<u32>(offset)) })
    }
    fn w8(&self, offset: usize, value: u8) {
        // SAFETY: as above.
        unsafe { write_volatile(self.ptr::<u8>(offset), value) }
    }
    fn w16(&self, offset: usize, value: u16) {
        // SAFETY: as above.
        unsafe { write_volatile(self.ptr::<u16>(offset), value.to_le()) }
    }
    fn w32(&self, offset: usize, value: u32) {
        // SAFETY: as above.
        unsafe { write_volatile(self.ptr::<u32>(offset), value.to_le()) }
    }

    /// Spin until `done()` or `us` microseconds pass. True if `done()`.
    fn wait(&self, us: u32, mut done: impl FnMut(&Self) -> bool) -> bool {
        for _ in 0..us {
            if done(self) {
                return true;
            }
            udelay(1);
        }
        done(self)
    }

    fn mac_ocp_read(&self, reg: u16) -> u16 {
        let Some(req) = logic::ocp_read_request(reg) else {
            return 0;
        };
        self.w32(reg::MAC_OCP, req);
        self.r32(reg::MAC_OCP) as u16
    }
    fn mac_ocp_write(&self, reg: u16, data: u16) {
        if let Some(cmd) = logic::ocp_write_word(reg, data) {
            self.w32(reg::MAC_OCP, cmd);
        }
    }
    fn mac_ocp_modify(&self, m: logic::OcpModify) {
        let v = self.mac_ocp_read(m.reg);
        self.mac_ocp_write(m.reg, logic::ocp_apply(v, m));
    }

    /// PHY-OCP read; `None` if the window never reports data valid.
    fn phy_ocp_read(&self, reg: u16) -> Option<u16> {
        let req = logic::ocp_read_request(reg)?;
        self.w32(reg::PHY_OCP, req);
        if self.wait(OCP_WAIT_US, |r| r.r32(reg::PHY_OCP) & logic::OCP_FLAG != 0) {
            Some(self.r32(reg::PHY_OCP) as u16)
        } else {
            None
        }
    }
    /// PHY-OCP write; false if the window stays busy.
    fn phy_ocp_write(&self, reg: u16, data: u16) -> bool {
        let Some(cmd) = logic::ocp_write_word(reg, data) else {
            return false;
        };
        self.w32(reg::PHY_OCP, cmd);
        self.wait(OCP_WAIT_US, |r| r.r32(reg::PHY_OCP) & logic::OCP_FLAG == 0)
    }
    fn phy_ocp_modify(&self, m: logic::OcpModify) -> bool {
        match self.phy_ocp_read(m.reg) {
            Some(v) => self.phy_ocp_write(m.reg, logic::ocp_apply(v, m)),
            None => false,
        }
    }
    fn mii_read(&self, mii: u8) -> Option<u16> {
        self.phy_ocp_read(logic::mii_ocp(mii))
    }
    fn mii_write(&self, mii: u8, value: u16) -> bool {
        self.phy_ocp_write(logic::mii_ocp(mii), value)
    }

    fn mac_bytes(&self, offset: usize) -> [u8; 6] {
        let mut mac = [0u8; 6];
        for (i, b) in mac.iter_mut().enumerate() {
            *b = self.r8(offset + i);
        }
        mac
    }
}

// --- descriptor memory --------------------------------------------------------------

static PUBLISH_BARRIERS: AtomicU64 = AtomicU64::new(0);
static ACQUIRE_BARRIERS: AtomicU64 = AtomicU64::new(0);
static DOORBELL_BARRIERS: AtomicU64 = AtomicU64::new(0);
/// RX buffers the fence refused at refill (kept in `rx_unfenced`).
static RX_UNFENCED: AtomicU64 = AtomicU64::new(0);

/// One descriptor ring: an arena page the chip reads and writes.
struct RingMem {
    page: Arc<DmaBuffer>,
    slots: usize,
}

impl RingMem {
    fn desc(&self, index: usize) -> *mut u8 {
        debug_assert!(index < self.slots);
        // SAFETY (pointer arithmetic only): `slots * DESC_LEN` fits the page.
        unsafe { self.page.as_mut_ptr().add(index * logic::DESC_LEN) }
    }
}

impl logic::DescMem for RingMem {
    fn read_opts1(&self, index: usize) -> u32 {
        // SAFETY: the descriptor lies inside the ring page this ring owns; the
        // chip writes it concurrently, hence the volatile read.
        u32::from_le(unsafe { read_volatile(self.desc(index) as *const u32) })
    }

    fn publish(&mut self, index: usize, opts1: u32, opts2: u32, addr: u64) {
        let d = self.desc(index);
        // SAFETY: as above. The address and opts2 go first; the release fence
        // orders them before the opts1 store that carries OWN (traps 5/6: the
        // chip must never see OWN with a stale address).
        unsafe {
            write_volatile(d.add(8) as *mut u64, addr.to_le());
            write_volatile(d.add(4) as *mut u32, opts2.to_le());
            fence(Ordering::Release);
            write_volatile(d as *mut u32, opts1.to_le());
        }
        PUBLISH_BARRIERS.fetch_add(1, Ordering::Relaxed);
    }
}

/// Fence a whole buffer (I-14(c1)); the only way an address reaches the chip.
fn fence_buffer(buf: &DmaBuffer, len: usize) -> Option<FencedDmaAddr> {
    dma_arena::fence(buf.physical_addr().as_u64(), len as u64).ok()
}

// --- containment (the seam the scenarios drive) ---------------------------------------

/// Whether this build carries the explicit opt-in.
const DMA_WITHOUT_IOMMU_OPT_IN: bool = cfg!(feature = "rtl8125-dma-without-iommu");

/// Make sure the fenced arena exists (xHCI usually reserved it first; a
/// second init refuses, which is fine). Returns whether it is available.
fn ensure_arena() -> bool {
    let _ = dma_arena::init(ARENA_PAGES);
    dma_arena::bounds().is_some()
}

/// Whether the device's requester is now confined to the VT-d fence domain.
#[cfg(feature = "vtd")]
fn vtd_fence(dev: Device) -> bool {
    use crate::drivers::iommu;
    if !iommu::enabled() {
        return false;
    }
    let sid = ((dev.bus as u16) << 8) | ((dev.device as u16) << 3) | dev.function as u16;
    iommu::fence_source_id(sid)
}

#[cfg(not(feature = "vtd"))]
fn vtd_fence(_dev: Device) -> bool {
    false
}

/// Bind-time PCI command and the containment decision. Writes COMMAND with
/// memory decode on, I/O and INTx off and **Bus Master Enable cleared** (it
/// fails closed even if firmware left it on), then decides the DMA mode. It
/// never sets Bus Master Enable; [`grant_bus_master`] does, later.
pub fn bind_containment(dev: Device, opt_in: bool) -> DmaMode {
    let command = pci::config_read16(dev, PCI_COMMAND);
    pci::config_write16(dev, PCI_COMMAND, logic::bind_command(command));
    let arena = ensure_arena();
    // Fence the requester only when the arena exists to fence it to.
    let fenced = arena && vtd_fence(dev);
    logic::containment(arena, fenced, opt_in)
}

/// The single place this driver sets Bus Master Enable, and only for a mode
/// the decision table allows. Called after both rings are programmed.
pub fn grant_bus_master(dev: Device, mode: DmaMode) -> bool {
    if !logic::bus_master_allowed(mode) {
        return false;
    }
    pci::enable_bus_mastering(dev);
    BUS_MASTER_GRANTS.fetch_add(1, Ordering::AcqRel);
    true
}

/// Whether the device's PCI COMMAND has Bus Master Enable set (hardware
/// state, for the scenarios and the console line).
pub fn bus_master_set(dev: Device) -> bool {
    pci::config_read16(dev, PCI_COMMAND) & logic::PCI_COMMAND_BUS_MASTER != 0
}

// --- the device ---------------------------------------------------------------------------

#[derive(Default)]
struct Counters {
    tx_frames: AtomicU64,
    tx_done: AtomicU64,
    rx_frames: AtomicU64,
    rx_dropped: AtomicU64,
    rx_bad_length: AtomicU64,
    rx_overflow: AtomicU64,
    storms: AtomicU64,
    err_status: AtomicU64,
    services: AtomicU64,
}

struct Inner {
    tx: logic::TxRing,
    tx_mem: RingMem,
    tx_slot: [Option<Arc<DmaBuffer>>; TX_SLOTS],
    tx_free: Vec<Arc<DmaBuffer>>,
    rx: logic::RxRing,
    rx_mem: RingMem,
    rx_slot: [Option<Arc<DmaBuffer>>; RX_SLOTS],
    rx_pool: Vec<Arc<DmaBuffer>>,
    /// Buffers the fence refused at refill: never posted, kept for the count.
    rx_unfenced: Vec<Arc<DmaBuffer>>,
    rx_ready: VecDeque<(Arc<DmaBuffer>, usize)>,
    storm: logic::StormDetector,
    tx_idle_passes: u32,
}

struct Rtl8125 {
    identity: NetDeviceIdentity,
    mac: [u8; 6],
    regs: Regs,
    tx_wedged: AtomicBool,
    counters: Counters,
    inner: SwitchMutex<Inner>,
}

impl Inner {
    /// Arm every empty RX slot that a pooled buffer can fill.
    fn refill_rx(&mut self) {
        let Inner {
            rx,
            rx_mem,
            rx_slot,
            rx_pool,
            rx_unfenced,
            ..
        } = self;
        rx.refill_from(
            rx_mem,
            || rx_pool.pop(),
            |buf| fence_buffer(buf, BUF_LEN).map(|a| a.phys()),
            // Refused by the fence: never handed to the chip, kept and
            // counted (`unfenced=`) rather than dropped.
            |buf| {
                RX_UNFENCED.fetch_add(1, Ordering::Relaxed);
                rx_unfenced.push(buf);
            },
            |slot, buf| rx_slot[slot] = Some(buf),
        );
    }

    fn reap_tx(&mut self, counters: &Counters) -> usize {
        let Inner {
            tx,
            tx_mem,
            tx_slot,
            tx_free,
            ..
        } = self;
        let n = tx.reap(tx_mem, REAP_BUDGET, |slot| {
            if let Some(buf) = tx_slot[slot].take() {
                tx_free.push(buf);
            }
        });
        counters.tx_done.fetch_add(n as u64, Ordering::Relaxed);
        n
    }

    fn reap_rx(&mut self, counters: &Counters) -> usize {
        let Inner {
            rx,
            rx_mem,
            rx_slot,
            rx_pool,
            rx_ready,
            ..
        } = self;
        let n = rx.reap(
            rx_mem,
            REAP_BUDGET,
            || {
                // The buffer read below must not pass the OWN-clear status
                // read (trap 5).
                fence(Ordering::Acquire);
                ACQUIRE_BARRIERS.fetch_add(1, Ordering::Relaxed);
            },
            |event| match event {
                logic::RxEvent::Frame { slot, len } => {
                    if let Some(buf) = rx_slot[slot].take() {
                        counters.rx_frames.fetch_add(1, Ordering::Relaxed);
                        rx_ready.push_back((buf, len));
                    }
                }
                logic::RxEvent::Dropped { slot, .. } => {
                    if let Some(buf) = rx_slot[slot].take() {
                        counters.rx_dropped.fetch_add(1, Ordering::Relaxed);
                        rx_pool.push(buf);
                    }
                }
                logic::RxEvent::BadLength { slot, .. } => {
                    if let Some(buf) = rx_slot[slot].take() {
                        counters.rx_bad_length.fetch_add(1, Ordering::Relaxed);
                        rx_pool.push(buf);
                    }
                }
            },
        );
        self.refill_rx();
        n
    }
}

impl Rtl8125 {
    /// One bounded service pass: acknowledge status, count it, reap both
    /// rings, feed the storm detector and the TX watchdog.
    fn service_pass(&self) {
        self.counters.services.fetch_add(1, Ordering::Relaxed);
        let regs = self.regs;
        let status = regs.r32(reg::ISR);
        if logic::mmio_dead(status) {
            return;
        }
        let mut inner = self.inner.lock();
        let acted = status & !inner.storm.masked();
        if acted != 0 {
            regs.w32(reg::ISR, acted);
        }
        if acted & logic::isr::RX_OVERFLOW_CLASS != 0 {
            self.counters.rx_overflow.fetch_add(1, Ordering::Relaxed);
        }
        let errors = acted
            & (logic::isr::RX_OVERFLOW_CLASS
                | logic::isr::RX_ERR
                | logic::isr::TX_ERR
                | logic::isr::SYSTEM_ERR);
        if errors & !logic::isr::RX_OVERFLOW_CLASS != 0 {
            self.counters.err_status.fetch_add(1, Ordering::Relaxed);
        }
        let tx_done = inner.reap_tx(&self.counters);
        let rx_seen = inner.reap_rx(&self.counters);
        let progress = tx_done + rx_seen > 0;
        if let logic::StormVerdict::Storm { bits, total } = inner.storm.observe(errors, progress) {
            self.counters.storms.fetch_add(1, Ordering::Relaxed);
            serial_println!(
                "rtl8125: irq storm bits={:#x} total={} masked={:#x} (polled; service bounded)",
                bits,
                total,
                inner.storm.masked()
            );
        }
        if inner.tx.in_flight() > 0 && tx_done == 0 {
            inner.tx_idle_passes = inner.tx_idle_passes.saturating_add(1);
            if inner.tx_idle_passes == TX_WEDGE_PASSES
                && !self.tx_wedged.swap(true, Ordering::AcqRel)
            {
                serial_println!(
                    "rtl8125: tx=wedged in_flight={} after {} passes; refusing new frames",
                    inner.tx.in_flight(),
                    TX_WEDGE_PASSES
                );
            }
        } else {
            inner.tx_idle_passes = 0;
        }
    }

    fn link(&self) -> logic::LinkReport {
        logic::decode_phy_status(self.regs.r16(reg::PHY_STATUS))
    }
}

impl NetDevice for Rtl8125 {
    fn identity(&self) -> &NetDeviceIdentity {
        &self.identity
    }

    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }

    fn link_state(&self) -> LinkState {
        if self.link().up {
            LinkState::Up
        } else {
            LinkState::Down
        }
    }

    fn mtu(&self) -> usize {
        1500
    }

    fn tx_reserve(&self) -> Option<TxFrame> {
        if self.tx_wedged.load(Ordering::Acquire) {
            return None;
        }
        let mut inner = self.inner.lock();
        inner.reap_tx(&self.counters);
        let buffer = inner.tx_free.pop()?;
        Some(TxFrame { buffer })
    }

    fn tx_submit(&self, frame: TxFrame, len: usize) -> NetResult<()> {
        if len == 0 || len > MAX_FRAME || len > frame.buffer.len() {
            self.inner.lock().tx_free.push(frame.buffer);
            return Err(NetError::InvalidFrame);
        }
        // Short frames are padded to the Ethernet minimum here rather than
        // trusting a per-version auto-pad.
        let wire = len.max(MIN_FRAME);
        if wire > len {
            // SAFETY: the TxFrame is exclusively ours until it is posted.
            unsafe { frame.buffer.as_mut_slice()[len..wire].fill(0) };
        }
        let mut inner = self.inner.lock();
        let Some(addr) = fence_buffer(&frame.buffer, wire) else {
            inner.tx_free.push(frame.buffer);
            return Err(NetError::Io);
        };
        let Inner { tx, tx_mem, .. } = &mut *inner;
        let Some(slot) = tx.post(tx_mem, addr.phys(), wire as u16) else {
            inner.tx_free.push(frame.buffer);
            return Err(NetError::RingFull);
        };
        inner.tx_slot[slot] = Some(frame.buffer);
        // Every descriptor store must be visible before the doorbell (trap 6).
        fence(Ordering::SeqCst);
        DOORBELL_BARRIERS.fetch_add(1, Ordering::Relaxed);
        self.regs.w16(reg::TX_POLL, logic::TX_POLL_NPQ);
        self.counters.tx_frames.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn rx_take(&self) -> Option<RxFrame> {
        let mut inner = self.inner.lock();
        if inner.rx_ready.is_empty() {
            inner.reap_rx(&self.counters);
        }
        let (buffer, len) = inner.rx_ready.pop_front()?;
        // The ring already refused lengths beyond the armed capacity; this
        // re-check keeps the `RxFrame` slice in bounds whatever produced the
        // entry (a frame never reaches the stack longer than its buffer).
        if len > buffer.len() {
            self.counters.rx_bad_length.fetch_add(1, Ordering::Relaxed);
            inner.rx_pool.push(buffer);
            inner.refill_rx();
            return None;
        }
        Some(RxFrame {
            buffer,
            offset: 0,
            len,
        })
    }

    fn rx_recycle(&self, frame: RxFrame) {
        let mut inner = self.inner.lock();
        inner.rx_pool.push(frame.buffer);
        inner.refill_rx();
    }

    fn service(&self) {
        self.service_pass();
    }
}

// --- bring-up ---------------------------------------------------------------------------------

/// Everything the console line reports.
#[derive(Default)]
struct Report {
    bdf: (u8, u8, u8),
    xid: u16,
    version: Option<MacVersion>,
    mac: [u8; 6],
    mac_source: &'static str,
    phy_ready: bool,
    phy_state: u16,
    phy_id: (u16, u16),
    ramcode: Option<u16>,
    an_done: bool,
    link: Option<logic::LinkReport>,
    dma: &'static str,
    bus_master: bool,
    note: &'static str,
}

fn mac_str(mac: [u8; 6]) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );
    s
}

fn print_line(r: &Report, dev: Option<&Rtl8125>) {
    let mut line = String::new();
    let _ = write!(
        line,
        "rtl8125: {:02x}:{:02x}.{} xid={:#05x}",
        r.bdf.0, r.bdf.1, r.bdf.2, r.xid
    );
    if let Some(v) = r.version {
        let _ = write!(line, " ({})", v.name());
    }
    let _ = write!(line, " mac={} src={}", mac_str(r.mac), r.mac_source);
    match r.link {
        Some(l) if l.up => {
            let _ = write!(
                line,
                " link=up {}/{}",
                l.speed_mbps,
                if l.full_duplex { "full" } else { "half" }
            );
        }
        Some(_) => {
            let _ = write!(
                line,
                " link=down an={}",
                if r.an_done { "done" } else { "timeout" }
            );
        }
        None => {
            let _ = write!(line, " link=unknown");
        }
    }
    let _ = write!(
        line,
        " phy={} state={} id={:04x}:{:04x}",
        if r.phy_ready { "ready" } else { "timeout" },
        r.phy_state,
        r.phy_id.0,
        r.phy_id.1
    );
    match r.ramcode {
        Some(v) => {
            let _ = write!(line, " phyram={:#06x}", v);
        }
        None => {
            let _ = write!(line, " phyram=unread");
        }
    }
    let _ = write!(
        line,
        " dma={} bme={} irq=polled",
        r.dma,
        if r.bus_master { 1 } else { 0 }
    );
    if let Some(d) = dev {
        let c = &d.counters;
        let _ = write!(
            line,
            " tx={} rx={} err={} rxovf={} drop={} badlen={} unfenced={} storm={}{}",
            c.tx_done.load(Ordering::Relaxed),
            c.rx_frames.load(Ordering::Relaxed),
            c.err_status.load(Ordering::Relaxed),
            c.rx_overflow.load(Ordering::Relaxed),
            c.rx_dropped.load(Ordering::Relaxed),
            c.rx_bad_length.load(Ordering::Relaxed),
            RX_UNFENCED.load(Ordering::Relaxed),
            c.storms.load(Ordering::Relaxed),
            if d.tx_wedged.load(Ordering::Acquire) {
                " tx=wedged"
            } else {
                ""
            }
        );
    } else {
        let _ = write!(line, " tx=0 rx=0 err=0");
    }
    if !r.note.is_empty() {
        let _ = write!(line, " {}", r.note);
    }
    serial_println!("{}", line);
    #[cfg(any(feature = "fb-progress-console", feature = "neo-test-rtl8125-absent"))]
    keep(line);
    crate::log::flush();
}

/// Boot entry: called once from `_kernel_start` after VT-d init, so the
/// containment decision sees the IOMMU state.
pub fn init() {
    let devices = pci::enumerate_all();
    let mut found = None;
    for dev in devices {
        if dev.vendor_id != VENDOR_REALTEK {
            continue;
        }
        if dev.device_id == DEVICE_RTL8126 {
            say!(
                "rtl8125: pci 10ec:8126 at {:02x}:{:02x}.{} present, unsupported (rung 1 drives the RTL8125 only)",
                dev.bus,
                dev.device,
                dev.function
            );
            set_status(BindStatus::Unsupported);
            continue;
        }
        if dev.device_id == DEVICE_RTL8125 && found.is_none() {
            found = Some(dev);
        }
    }
    let Some(dev) = found else {
        if status() == BindStatus::NotRun {
            set_status(BindStatus::Absent);
        }
        say!("rtl8125: pci 10ec:8125 absent");
        crate::log::flush();
        return;
    };
    match bring_up(dev) {
        Some(device) => {
            #[cfg(not(feature = "neo-test"))]
            boot_dhcp(&device);
            #[cfg(feature = "neo-test")]
            let _ = device;
        }
        None => crate::log::flush(),
    }
}

/// Power the function to D0 and turn ASPM off (trap 7).
fn pci_prepare(dev: Device) {
    if let Some(pm) = pci::capability(dev, PCI_CAP_PM) {
        let pmcsr = pci::config_read16(dev, pm + 4);
        if pmcsr & 0x3 != 0 {
            pci::config_write16(dev, pm + 4, pmcsr & !0x3);
            udelay(D0_SETTLE_US);
        }
    }
    if let Some(exp) = pci::capability(dev, PCI_CAP_EXP) {
        let lc = pci::config_read16(dev, exp + PCIE_LINK_CONTROL);
        pci::config_write16(
            dev,
            exp + PCIE_LINK_CONTROL,
            logic::link_control_aspm_off(lc),
        );
    }
}

fn map_registers(dev: Device) -> Option<Regs> {
    // BAR2 is the 64-bit memory BAR on the r8169 family; BAR1 a 32-bit
    // fallback on some boards. BAR0 is I/O and is never used.
    for bar in [2u8, 1] {
        if let Some(pci::PciBar::Memory32 { .. } | pci::PciBar::Memory64 { .. }) =
            pci::bar(dev, bar)
            && let Some(base) = pci::map_bar_region_validated(dev, bar, 0, reg::MAP_LEN)
        {
            return Some(Regs { base });
        }
    }
    None
}

/// Stop the chip and soft-reset it, bounded. Interrupts masked, RX filters
/// off, the RX DV gate closed while it drains.
fn reset_chip(regs: Regs, version: MacVersion) -> bool {
    regs.w32(reg::IMR, 0);
    regs.w32(reg::ISR, logic::isr::ALL);
    let rx = regs.r32(reg::RX_CONFIG);
    regs.w32(reg::RX_CONFIG, rx & !logic::rx_accept::MASK);
    regs.w32(reg::MISC, regs.r32(reg::MISC) | logic::MISC_RXDV_GATE);
    if version.is_b_or_later() {
        regs.w8(reg::CMD, regs.r8(reg::CMD) | logic::cmd::STOP_REQ);
    }
    let _ = regs.wait(FIFO_DRAIN_WAIT_US, |r| {
        r.r8(reg::MCU) & logic::MCU_FIFOS_EMPTY == logic::MCU_FIFOS_EMPTY
    });
    if version.is_b_or_later() {
        let _ = regs.wait(FIFO_DRAIN_WAIT_US, |r| {
            r.r16(reg::INTR_MITIGATE) & logic::MITIGATE_IDLE == logic::MITIGATE_IDLE
        });
    }
    regs.w8(reg::CMD, logic::cmd::RESET);
    regs.wait(RESET_WAIT_US, |r| r.r8(reg::CMD) & logic::cmd::RESET == 0)
}

/// MAC-side setup for one queue, no offloads, EEE off.
fn mac_init(regs: Regs, version: MacVersion) {
    for m in logic::MAC_INIT_COMMON {
        regs.mac_ocp_modify(*m);
    }
    for m in logic::mac_init_versioned(version) {
        regs.mac_ocp_modify(m);
    }
    // Clear the TCAM (pulse 0xeb54 bit 0).
    regs.mac_ocp_modify(logic::OcpModify {
        reg: 0xeb54,
        clear: 0,
        set: 0x0001,
    });
    udelay(1);
    regs.mac_ocp_modify(logic::OcpModify {
        reg: 0xeb54,
        clear: 0x0001,
        set: 0,
    });
    regs.w16(0x1880, regs.r16(0x1880) & !0x0030);
    regs.w32(reg::RSS_CTRL, 0);
    regs.w8(reg::INT_CFG0, 0);
    // Interrupt moderation off: 32 timers on the 8125B, 64 on the others.
    let timers = if version == MacVersion::B { 32 } else { 64 };
    for i in 0..timers {
        regs.w32(0xa00 + i * 4, 0);
    }
    if version == MacVersion::B {
        regs.w16(reg::INT_CFG1, 0);
    }
    regs.w8(reg::CONFIG2, regs.r8(reg::CONFIG2) & !logic::CONFIG2_CLKREQ);
    regs.w8(
        reg::CONFIG1,
        regs.r8(reg::CONFIG1) & !logic::CONFIG1_SPEED_DOWN,
    );
}

/// Power the PHY up, advertise 10/100/1000/2500, EEE off, restart
/// autonegotiation and wait (bounded) for link.
fn phy_bring_up(regs: Regs, r: &mut Report) {
    // PMCH bits 7..6: PHY power.
    regs.w8(0x6f, regs.r8(0x6f) | 0xc0);
    let _ = regs.mii_write(logic::mii::BMCR, logic::mii::BMCR_ANENABLE);
    r.phy_ready = regs.wait(PHY_READY_WAIT_US, |rr| {
        rr.phy_ocp_read(logic::PHY_OCP_STATE)
            .is_some_and(|s| s & 0x7 == logic::PHY_STATE_LAN_ON)
    });
    r.phy_state = regs
        .phy_ocp_read(logic::PHY_OCP_STATE)
        .map(|s| s & 0x7)
        .unwrap_or(0xffff);
    r.phy_id = (
        regs.mii_read(logic::mii::PHYID1).unwrap_or(0xffff),
        regs.mii_read(logic::mii::PHYID2).unwrap_or(0xffff),
    );
    // The PHY RAM-code (patch) version: what a firmware patch would have
    // changed. Reported so a `link=down` is tellable from "needs firmware".
    r.ramcode = if regs.phy_ocp_write(
        logic::PHY_OCP_INDIRECT_ADDR,
        logic::PHY_RAMCODE_VERSION_SLOT,
    ) {
        regs.phy_ocp_read(logic::PHY_OCP_INDIRECT_DATA)
    } else {
        None
    };

    let _ = regs.mii_write(logic::mii::ANAR, logic::mii::ANAR_ALL);
    if let Some(gb) = regs.mii_read(logic::mii::GBCR) {
        let _ = regs.mii_write(
            logic::mii::GBCR,
            gb | logic::mii::GBCR_1000FULL | logic::mii::GBCR_1000HALF,
        );
    }
    let _ = regs.phy_ocp_modify(logic::OcpModify {
        reg: logic::PHY_OCP_ADV_2500,
        clear: 0,
        set: logic::ADV_2500_FULL,
    });
    for m in logic::PHY_EEE_OFF {
        let _ = regs.phy_ocp_modify(*m);
    }
    let _ = regs.mii_write(
        logic::mii::BMCR,
        logic::mii::BMCR_ANENABLE | logic::mii::BMCR_ANRESTART,
    );
    let up = regs.wait(AUTONEG_WAIT_US, |rr| {
        logic::decode_phy_status(rr.r16(reg::PHY_STATUS)).up
    });
    r.an_done = up
        || regs
            .mii_read(logic::mii::BMSR)
            .is_some_and(|s| s & logic::mii::BMSR_ANCOMPLETE != 0);
    r.link = Some(logic::decode_phy_status(regs.r16(reg::PHY_STATUS)));
}

/// Allocate every ring and buffer in the arena. `None` if the arena cannot
/// hold them (nothing is handed to the chip in that case).
fn alloc_rings() -> Option<Inner> {
    let tx_page = DmaBuffer::alloc_in_arena(TX_SLOTS * logic::DESC_LEN).ok()?;
    let rx_page = DmaBuffer::alloc_in_arena(RX_SLOTS * logic::DESC_LEN).ok()?;
    let mut tx_free = Vec::with_capacity(TX_SLOTS);
    for _ in 0..TX_SLOTS {
        tx_free.push(DmaBuffer::alloc_in_arena(BUF_LEN).ok()?);
    }
    let mut rx_pool = Vec::with_capacity(RX_SLOTS + RX_SPARE);
    for _ in 0..RX_SLOTS + RX_SPARE {
        rx_pool.push(DmaBuffer::alloc_in_arena(BUF_LEN).ok()?);
    }
    Some(Inner {
        tx: logic::TxRing::new(TX_SLOTS),
        tx_mem: RingMem {
            page: tx_page,
            slots: TX_SLOTS,
        },
        tx_slot: Default::default(),
        tx_free,
        rx: logic::RxRing::new(RX_SLOTS, BUF_LEN as u16),
        rx_mem: RingMem {
            page: rx_page,
            slots: RX_SLOTS,
        },
        rx_slot: Default::default(),
        rx_pool,
        rx_unfenced: Vec::new(),
        rx_ready: VecDeque::new(),
        storm: logic::StormDetector::new(),
        tx_idle_passes: 0,
    })
}

/// Program a ring base. Takes only a fenced address (I-14(c1)).
fn write_ring_base(regs: Regs, lo: usize, hi: usize, addr: FencedDmaAddr) {
    regs.w32(hi, (addr.phys() >> 32) as u32);
    regs.w32(lo, addr.phys() as u32);
}

fn bring_up(dev: Device) -> Option<Arc<Rtl8125>> {
    let mut r = Report {
        bdf: dev.bdf(),
        mac_source: "none",
        dma: "undecided",
        ..Report::default()
    };
    // Bus mastering off and the containment decided before the chip is
    // touched at all.
    let mode = bind_containment(dev, DMA_WITHOUT_IOMMU_OPT_IN);
    r.dma = logic::dma_label(mode);
    pci_prepare(dev);

    let Some(regs) = map_registers(dev) else {
        r.note = "bar=unmapped";
        set_status(BindStatus::Refused);
        print_line(&r, None);
        return None;
    };
    let tx_config = regs.r32(reg::TX_CONFIG);
    r.xid = logic::xid_of(tx_config);
    if logic::mmio_dead(tx_config) {
        r.note = "mmio=dead";
        set_status(BindStatus::Refused);
        print_line(&r, None);
        return None;
    }
    let version = match logic::decode_xid(tx_config) {
        Ok(v) => v,
        Err(_) => {
            r.note = "unsupported";
            set_status(BindStatus::Unsupported);
            print_line(&r, None);
            return None;
        }
    };
    r.version = Some(version);

    if !reset_chip(regs, version) {
        r.note = "reset=timeout";
        set_status(BindStatus::Refused);
        print_line(&r, None);
        return None;
    }
    // Leave out-of-band (DASH) mode if firmware entered it.
    regs.w8(reg::MCU, regs.r8(reg::MCU) & !0x80);
    regs.w8(reg::CFG9346, logic::CFG9346_UNLOCK);

    // Station address: MAC0 (as firmware left it), else the backup copy;
    // re-read for the autoload window before giving up (trap 8).
    let mut choice = (regs.mac_bytes(reg::MAC0), logic::MacSource::None);
    let _ = regs.wait(100_000, |rr| {
        choice = logic::choose_mac(rr.mac_bytes(reg::MAC0), rr.mac_bytes(reg::MAC0_BKP));
        choice.1 != logic::MacSource::None
    });
    r.mac = choice.0;
    r.mac_source = match choice.1 {
        logic::MacSource::Mac0 => "mac0",
        logic::MacSource::Backup => "backup",
        logic::MacSource::None => "autoload=timeout",
    };
    if choice.1 != logic::MacSource::None {
        let (lo, hi) = logic::mac_registers(r.mac);
        regs.w32(reg::MAC4, hi);
        regs.w32(reg::MAC0, lo);
    }

    mac_init(regs, version);
    phy_bring_up(regs, &mut r);

    if choice.1 == logic::MacSource::None {
        regs.w8(reg::CFG9346, logic::CFG9346_LOCK);
        set_status(BindStatus::Refused);
        print_line(&r, None);
        return None;
    }
    if !logic::bus_master_allowed(mode) {
        // Fail closed: MAC and link are reported, no ring is allocated, the
        // chip's TX/RX stay disabled and bus mastering stays off.
        regs.w8(reg::CFG9346, logic::CFG9346_LOCK);
        r.bus_master = bus_master_set(dev);
        set_status(BindStatus::DmaBlocked);
        print_line(&r, None);
        return None;
    }

    let Some(mut inner) = alloc_rings() else {
        regs.w8(reg::CFG9346, logic::CFG9346_LOCK);
        r.note = "rings=arena-exhausted";
        set_status(BindStatus::Refused);
        print_line(&r, None);
        return None;
    };
    inner.refill_rx();
    let (Some(tx_base), Some(rx_base)) = (
        fence_buffer(&inner.tx_mem.page, TX_SLOTS * logic::DESC_LEN),
        fence_buffer(&inner.rx_mem.page, RX_SLOTS * logic::DESC_LEN),
    ) else {
        regs.w8(reg::CFG9346, logic::CFG9346_LOCK);
        r.note = "rings=unfenced";
        set_status(BindStatus::Refused);
        print_line(&r, None);
        return None;
    };

    regs.w16(reg::RX_MAX_SIZE, BUF_LEN as u16);
    write_ring_base(regs, reg::TX_DESC_LO, reg::TX_DESC_HI, tx_base);
    write_ring_base(regs, reg::RX_DESC_LO, reg::RX_DESC_HI, rx_base);
    regs.w8(reg::CFG9346, logic::CFG9346_LOCK);
    // Rings are programmed: only now may the chip bus-master (trap 4).
    r.bus_master = grant_bus_master(dev, mode) && bus_master_set(dev);

    regs.w8(reg::CMD, logic::cmd::TX_ENABLE | logic::cmd::RX_ENABLE);
    regs.w32(reg::RX_CONFIG, logic::rx_config(version));
    regs.w32(reg::TX_CONFIG, logic::TX_CONFIG);
    // Accept every multicast (IPv6 neighbour discovery) — rung 1 has no hash.
    regs.w32(reg::MAR4, 0xffff_ffff);
    regs.w32(reg::MAR0, 0xffff_ffff);
    // Polled: nothing unmasked. Then open the RX DV gate; until this the chip
    // receives nothing ("link up, rx=0").
    regs.w32(reg::IMR, 0);
    regs.w32(reg::ISR, logic::isr::ALL);
    regs.w32(reg::MISC, regs.r32(reg::MISC) & !logic::MISC_RXDV_GATE);

    let device = Arc::new(Rtl8125 {
        identity: NetDeviceIdentity {
            driver: "rtl8125",
            name: "eth0",
            pci_bdf: Some(dev.bdf()),
        },
        mac: r.mac,
        regs,
        tx_wedged: AtomicBool::new(false),
        counters: Counters::default(),
        inner: SwitchMutex::new_ordered(inner, crate::sync::lock_order::NET_DRIVER),
    });
    set_status(BindStatus::Live);
    print_line(&r, Some(&device));
    crate::drivers::net::register_device(device.clone());
    Some(device)
}

// --- boot DHCP + gateway reachability (production boot only) ------------------------------

#[cfg(not(feature = "neo-test"))]
const DHCP_ATTEMPTS: u32 = 4;
#[cfg(not(feature = "neo-test"))]
const DHCP_PUMP_PASSES_PER_ATTEMPT: u64 = 750_000;
#[cfg(not(feature = "neo-test"))]
const ARP_PUMP_PASSES: u64 = 750_000;

/// Run bounded DHCP on the RTL8125 if no other interface already holds a
/// lease, then ARP-resolve the router as the TX+RX round-trip check (the
/// stack has no IPv4 ICMP echo yet). Boot always continues.
#[cfg(not(feature = "neo-test"))]
fn boot_dhcp(device: &Arc<Rtl8125>) {
    use crate::net::dhcpv4;

    if dhcpv4::snapshot().state == dhcpv4::DhcpState::Bound {
        say!("rtl8125: another interface holds the boot lease; dhcp not run");
        return;
    }
    let Some(index) = (0..crate::drivers::net::device_count()).find(|&i| {
        crate::drivers::net::get_device(i).is_some_and(|d| d.identity().driver == "rtl8125")
    }) else {
        return;
    };
    let iface = index as u32;
    let name = crate::net::identity::name(iface).unwrap_or_else(|| String::from("eth0"));
    let dev: Arc<dyn NetDevice> = device.clone();
    dhcpv4::set_interface_id(iface);
    let mut bound = false;
    for attempt in 1..=DHCP_ATTEMPTS {
        let xid = dhcpv4::start_discover();
        say!(
            "rtl8125: {} dhcp discover attempt={}/{} xid={:#010x} link={:?}",
            name,
            attempt,
            DHCP_ATTEMPTS,
            xid,
            dev.link_state()
        );
        crate::log::flush();
        for _ in 0..DHCP_PUMP_PASSES_PER_ATTEMPT {
            pump_once(&dev);
            crate::net::tick();
            if dhcpv4::snapshot().state == dhcpv4::DhcpState::Bound {
                bound = true;
                break;
            }
            core::hint::spin_loop();
        }
        if bound {
            break;
        }
    }
    let lease = dhcpv4::snapshot().lease;
    let Some(lease) = lease.filter(|_| bound) else {
        print_counters(device, &name, "dhcp=timeout");
        return;
    };
    say!(
        "rtl8125: {} lease ip={}.{}.{}.{} router={:?} dns={:?}",
        name,
        lease.ip[0],
        lease.ip[1],
        lease.ip[2],
        lease.ip[3],
        lease.router,
        lease.dns
    );
    let gw = match lease.router {
        Some(router) => arp_probe(&dev, lease.ip, router),
        None => "gw=none",
    };
    print_counters(device, &name, gw);
}

#[cfg(not(feature = "neo-test"))]
fn arp_probe(dev: &Arc<dyn NetDevice>, src_ip: [u8; 4], router: [u8; 4]) -> &'static str {
    let mut frame = [0u8; 64];
    let Ok(len) =
        crate::net::arp::build_ethernet_request(dev.mac_address(), src_ip, router, &mut frame)
    else {
        return "gw-arp=build-failed";
    };
    let Some(mut tx) = dev.tx_reserve() else {
        return "gw-arp=no-tx";
    };
    tx.as_mut_slice()[..len].copy_from_slice(&frame[..len]);
    if dev.tx_submit(tx, len).is_err() {
        return "gw-arp=tx-failed";
    }
    for _ in 0..ARP_PUMP_PASSES {
        pump_once(dev);
        if crate::net::arp::lookup(router).is_some() {
            return "gw-arp=ok";
        }
        core::hint::spin_loop();
    }
    "gw-arp=timeout"
}

#[cfg(not(feature = "neo-test"))]
fn pump_once(dev: &Arc<dyn NetDevice>) {
    dev.service();
    // Bounded per pass: a receive stream the chip keeps replenishing must not
    // keep the caller's DHCP/ARP pass counter from advancing (I-14(b)).
    logic::drain_bounded(logic::PUMP_FRAME_BUDGET, || {
        let Some(frame) = dev.rx_take() else {
            return false;
        };
        let bytes = frame.as_slice();
        if bytes.len() >= 14 {
            match u16::from_be_bytes([bytes[12], bytes[13]]) {
                0x0806 => {
                    let _ = crate::net::arp::inject(bytes);
                }
                0x0800 => {
                    let _ = crate::net::dhcpv4::demux_eth_rx(bytes);
                }
                _ => {}
            }
        }
        dev.rx_recycle(frame);
        true
    });
}

#[cfg(not(feature = "neo-test"))]
fn print_counters(device: &Rtl8125, name: &str, outcome: &str) {
    let c = &device.counters;
    let l = device.link();
    say!(
        "rtl8125: {} link={} {}/{} tx={} rx={} err={} rxovf={} drop={} badlen={} unfenced={} storm={} {} barriers=pub:{} acq:{} bell:{} fence_refusals={}",
        name,
        if l.up { "up" } else { "down" },
        l.speed_mbps,
        if l.full_duplex { "full" } else { "half" },
        c.tx_done.load(Ordering::Relaxed),
        c.rx_frames.load(Ordering::Relaxed),
        c.err_status.load(Ordering::Relaxed),
        c.rx_overflow.load(Ordering::Relaxed),
        c.rx_dropped.load(Ordering::Relaxed),
        c.rx_bad_length.load(Ordering::Relaxed),
        RX_UNFENCED.load(Ordering::Relaxed),
        c.storms.load(Ordering::Relaxed),
        outcome,
        PUBLISH_BARRIERS.load(Ordering::Relaxed),
        ACQUIRE_BARRIERS.load(Ordering::Relaxed),
        DOORBELL_BARRIERS.load(Ordering::Relaxed),
        dma_arena::fence_refusals()
    );
    crate::log::flush();
}
