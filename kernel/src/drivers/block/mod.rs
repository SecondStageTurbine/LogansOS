// SPDX-License-Identifier: MPL-2.0
//! Phase 3 BUILD-1 block substrate.
//!
//! BUILD-1 exposes a synchronous block-device shape to filesystems while
//! drivers remain free to complete requests through IRQ-backed hardware
//! paths. The submission API takes `&[Arc<DmaBuffer>]` from day one so later
//! AHCI PRDT and NVMe PRP/SGL work can grow into scatter/gather without
//! changing filesystem callers.

extern crate alloc;

pub mod ahci;
// OT-87 R4.1: device identity and the install claim (THINK §2a).
#[cfg(feature = "installer")]
pub mod claim_logic;
pub mod dma;
// OT-87 R3: the shared GPT reader and writer (pure).
pub mod gpt_logic;
#[cfg(feature = "installer")]
pub mod install_claim;
// OT-87 R4.3: the install state machine (pure, beside gpt_logic).
#[cfg(feature = "installer")]
pub mod install_logic;
pub mod nvme;
// The NVMe events the install claim needs (Muse's R4.1 review, item 4).
#[cfg(feature = "installer")]
pub mod nvme_event_logic;
#[cfg(feature = "neofs-partition")]
pub mod partition;
pub mod virtio_barrier;
pub mod virtio_blk;
pub mod write_guard_logic;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Lazy;

use crate::serial_println;
use crate::sync::SwitchMutex;

pub use dma::DmaBuffer;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BlockOp {
    Read,
    Write,
    Flush,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct BlockGeometry {
    pub logical_block_size: u32,
    pub physical_block_size: u32,
    pub block_count: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BlockError {
    NoDevice,
    Unsupported,
    InvalidRequest,
    Io,
    /// The write guard refused: the device carries no NeoPlutarch marker
    /// (`write_guard_logic`), so it is not ours to write.
    WriteRefused,
}

pub type BlockResult<T> = Result<T, BlockError>;

pub struct BlockRequest<'a> {
    pub op: BlockOp,
    pub lba: u64,
    pub buffers: &'a [Arc<DmaBuffer>],
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct BlockDeviceIdentity {
    pub driver: &'static str,
    pub name: &'static str,
    pub pci_bdf: Option<(u8, u8, u8)>,
}

pub trait BlockDevice: Send + Sync {
    fn identity(&self) -> &BlockDeviceIdentity;
    fn geometry(&self) -> BlockGeometry;

    fn submit(&self, _request: BlockRequest<'_>) -> BlockResult<()> {
        Err(BlockError::Unsupported)
    }

    fn supports_flush(&self) -> bool {
        false
    }

    fn flush(&self) -> BlockResult<()> {
        Err(BlockError::Unsupported)
    }

    fn read_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        self.submit(BlockRequest {
            op: BlockOp::Read,
            lba,
            buffers,
        })
    }

    fn write_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        self.submit(BlockRequest {
            op: BlockOp::Write,
            lba,
            buffers,
        })
    }

    /// OT-87 R4.1: the identity the driver reads from the device now (NVMe
    /// Identify Controller, ATA IDENTIFY DEVICE, virtio-blk `GET_ID`). It is
    /// re-read on every call: the claim compares it at each step boundary.
    /// Process context, interrupts on (it issues a command and may park).
    #[cfg(feature = "installer")]
    fn driver_identity(&self) -> Option<claim_logic::DriverIdentity> {
        None
    }

    /// OT-87 R4.1: the medium's state, polled from the device's own status
    /// registers. `generation` moves on every event that can mean a different
    /// medium; `alive` falls once on a fatal state.
    #[cfg(feature = "installer")]
    fn media_state(&self) -> MediaState {
        MediaState {
            generation: 0,
            alive: true,
        }
    }
}

/// OT-87 R4.1: what [`BlockDevice::media_state`] reports.
#[cfg(feature = "installer")]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct MediaState {
    pub generation: u64,
    pub alive: bool,
}

/// OT-87 R4.1: a driver's media generation and liveness (THINK §2a item 5).
/// Plain atomics: a driver bumps it from its own command or poll path, never
/// under another lock, and nothing waits on it.
#[cfg(feature = "installer")]
pub struct MediaTracker {
    generation: AtomicU64,
    alive: AtomicBool,
}

#[cfg(feature = "installer")]
impl MediaTracker {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            alive: AtomicBool::new(true),
        }
    }

    /// An event that can mean a different medium (a link change, a resize, a
    /// reset notice).
    pub fn changed(&self, why: &str) {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        serial_println!(
            "[ kernel ] block: media changed reason={} generation={}",
            why,
            generation
        );
    }

    /// A fatal state: the device is dead from now on (counted once).
    pub fn dead(&self, why: &str) {
        if self.alive.swap(false, Ordering::AcqRel) {
            self.changed(why);
        }
    }

    pub fn state(&self) -> MediaState {
        MediaState {
            generation: self.generation.load(Ordering::Acquire),
            alive: self.alive.load(Ordering::Acquire),
        }
    }
}

/// Every registered device is wrapped in this guard. Reads, flushes and
/// geometry pass straight through. The first write judges the device
/// (`write_guard_logic::judge`) by READING its markers and partition table,
/// once: a write then reaches the driver only if its whole byte range lies in
/// an extent a sealed, valid marker claims and no partition table allocates.
/// Everything else is refused, and a device with no such claim is never
/// written at all (the Zenbook's Windows NVMe, any stray disk).
struct GuardedDevice {
    inner: Arc<dyn BlockDevice>,
    index: usize,
    arming: spin::Once<write_guard_logic::Arming>,
    /// Set by the one caller that runs the judgement; every other caller
    /// waits for `arming` without entering `call_once` (see `arming`).
    judging: AtomicBool,
    /// Out-of-extent writes refused so far (the first is logged).
    refused_writes: AtomicU64,
    /// OT-87 R4.1: an install claim holds this device (one at a time).
    /// R4.4 (Muse, R4.3 note 1(a)): while it is set, the plain write path
    /// refuses every write to the device; the claim's own path is the only
    /// one left.
    #[cfg(feature = "installer")]
    claimed: AtomicBool,
    /// OT-87 R4.4: plain writes past the `claimed` check and not yet
    /// complete. A claim waits for this to reach zero after it sets
    /// `claimed`, so no plain write overlaps it (`take_claim`).
    #[cfg(feature = "installer")]
    plain_writes: AtomicU64,
    /// OT-87 R4.3: reads through this device object, so the install
    /// scenario can show the boot medium is never read once the payload is
    /// in RAM (THINK §4.7, "USB removed mid-install").
    #[cfg(feature = "installer")]
    reads: AtomicU64,
}

/// `block_write_guard`'s judge-contention leg: the next judgement's first read
/// holds for this many timer ticks (0: off), with `JUDGE_PAUSED` set while it
/// does, so a second caller provably waits on an unfinished judgement.
#[cfg(feature = "neo-test-block-write-guard")]
static JUDGE_PAUSE_TICKS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-block-write-guard")]
static JUDGE_PAUSED: AtomicBool = AtomicBool::new(false);
/// How callers waited on another caller's judgement: yielding with
/// interrupts on (the fix), or spinning with them off (the pre-fix wait).
#[cfg(feature = "neo-test-block-write-guard")]
static JUDGE_WAITS_YIELDING: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-block-write-guard")]
static JUDGE_SPINS_IF_OFF: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "neo-test-block-write-guard")]
pub fn pause_next_judgement_for_test(ticks: u64) {
    JUDGE_PAUSE_TICKS.store(ticks, Ordering::Release);
}

#[cfg(feature = "neo-test-block-write-guard")]
pub fn judgement_paused_for_test() -> bool {
    JUDGE_PAUSED.load(Ordering::Acquire)
}

/// (waits that yielded with interrupts on, spins with interrupts off).
#[cfg(feature = "neo-test-block-write-guard")]
pub fn judge_wait_counts_for_test() -> (u64, u64) {
    (
        JUDGE_WAITS_YIELDING.load(Ordering::Acquire),
        JUDGE_SPINS_IF_OFF.load(Ordering::Acquire),
    )
}

#[cfg(feature = "neo-test-block-write-guard")]
fn judge_pause_point() {
    let ticks = JUDGE_PAUSE_TICKS.swap(0, Ordering::AcqRel);
    if ticks == 0 {
        return;
    }
    let tick = || crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
    let until = tick().saturating_add(ticks);
    JUDGE_PAUSED.store(true, Ordering::Release);
    crate::arch::x86_64::with_interrupts_enabled(|| {
        while tick() < until {
            crate::sched::yield_now();
        }
    });
    JUDGE_PAUSED.store(false, Ordering::Release);
}

/// `write_guard_logic::Disk` over a device's driver: aligned block reads into
/// a DMA bounce buffer, at most 64 KiB at a time.
struct GuardReader<'a>(&'a dyn BlockDevice);

impl write_guard_logic::Disk for GuardReader<'_> {
    fn capacity(&self) -> u64 {
        let g = self.0.geometry();
        g.block_count.saturating_mul(g.logical_block_size as u64)
    }

    fn sector_size(&self) -> u64 {
        self.0.geometry().logical_block_size as u64
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> bool {
        const CHUNK: u64 = 64 * 1024;
        let block = self.sector_size();
        let end = offset.saturating_add(out.len() as u64);
        if block == 0 || end > self.capacity() {
            return false;
        }
        let mut at = offset;
        while at < end {
            let lba = at / block;
            let first = lba * block;
            let blocks = (end - first).min(CHUNK.max(block)).div_ceil(block);
            let Ok(buffer) = DmaBuffer::alloc_zeroed((blocks * block) as usize) else {
                return false;
            };
            #[cfg(feature = "neo-test-block-write-guard")]
            judge_pause_point();
            if self
                .0
                .read_block(lba, core::slice::from_ref(&buffer))
                .is_err()
            {
                return false;
            }
            // SAFETY: the buffer was just filled by the read and is not
            // shared with any in-flight request.
            let bytes = unsafe { buffer.as_slice() };
            let skip = (at - first) as usize;
            let take = ((end - at) as usize).min(bytes.len() - skip);
            let dst = (at - offset) as usize;
            out[dst..dst + take].copy_from_slice(&bytes[skip..skip + take]);
            at += take as u64;
        }
        true
    }
}

impl GuardedDevice {
    /// The device's judgement, made once by the first caller.
    ///
    /// Only that caller enters `call_once`. The judgement does block I/O, so
    /// it can park, and on a preemptible kthread (`neofs-writeback`) it can be
    /// preempted. A second caller that entered `call_once` would spin on the
    /// unfinished `Once` at whatever IF it had, and a syscall arrives at IF=0:
    /// on the judge's CPU it then never yields, the judge never runs again,
    /// and the machine hangs (CP-37; `arthur_memory_forgetting`'s 900 s
    /// timeouts, Plans/NOTE-2026-10-25-write-guard-judge-hang.md). Every other
    /// caller instead waits with interrupts on and yields each round, the
    /// virtio-blk I/O gate's discipline (CP-2).
    fn arming(&self) -> &write_guard_logic::Arming {
        if let Some(arming) = self.arming.get() {
            return arming;
        }
        if self
            .judging
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return self.arming.call_once(|| self.judge());
        }
        self.wait_for_judgement()
    }

    #[cfg(not(feature = "neo-test-write-guard-judge-spin-control"))]
    fn wait_for_judgement(&self) -> &write_guard_logic::Arming {
        #[cfg(feature = "neo-test-block-write-guard")]
        JUDGE_WAITS_YIELDING.fetch_add(1, Ordering::AcqRel);
        crate::arch::x86_64::with_interrupts_enabled(|| {
            loop {
                if let Some(arming) = self.arming.get() {
                    break arming;
                }
                crate::sched::yield_now();
            }
        })
    }

    /// Control: the pre-fix wait, `call_once` at the caller's IF (it spins on
    /// the unfinished `Once`).
    #[cfg(feature = "neo-test-write-guard-judge-spin-control")]
    fn wait_for_judgement(&self) -> &write_guard_logic::Arming {
        if !crate::arch::x86_64::interrupts_enabled() {
            JUDGE_SPINS_IF_OFF.fetch_add(1, Ordering::AcqRel);
        }
        self.arming.call_once(|| self.judge())
    }

    /// Whether any write to this device can be allowed.
    fn writes_allowed(&self) -> bool {
        !self.arming().claims.is_empty()
    }

    /// Whether a write of `buffers` at `lba` lies in one armed extent. The
    /// first refused write is logged.
    fn write_allowed(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> bool {
        let len: u64 = buffers.iter().map(|b| b.len() as u64).sum();
        self.write_range_allowed(lba, len)
    }

    /// Whether a write of `len` bytes at `lba` lies in one armed extent.
    fn write_range_allowed(&self, lba: u64, len: u64) -> bool {
        let block = self.inner.geometry().logical_block_size as u64;
        let start = lba.saturating_mul(block);
        let arming = self.arming();
        if arming.allows(start, start.saturating_add(len)) {
            return true;
        }
        if !arming.claims.is_empty() && self.refused_writes.fetch_add(1, Ordering::AcqRel) == 0 {
            serial_println!(
                "[ kernel ] block: WRITE REFUSED device={} lba={} bytes={} - outside every armed extent",
                self.index,
                lba,
                len
            );
        }
        false
    }

    #[cfg(not(feature = "neo-test-block-write-guard-control"))]
    fn judge(&self) -> write_guard_logic::Arming {
        self.judge_guarded()
    }

    /// The write guard's judgement of this device, logged.
    fn judge_guarded(&self) -> write_guard_logic::Arming {
        #[cfg(not(feature = "neofs-partition"))]
        let arming = write_guard_logic::judge(&GuardReader(self.inner.as_ref()));
        // OT-87 R3: plus a LogansOS NeoFS partition that passes all five
        // conditions (`write_guard_logic::partition_claims`).
        #[cfg(feature = "neofs-partition")]
        let arming = write_guard_logic::judge_with_partitions(&GuardReader(self.inner.as_ref()));
        let ident = self.inner.identity();
        if arming.claims.is_empty() {
            serial_println!(
                "[ kernel ] block: WRITE REFUSED device={} driver={} name={} table={} blocked_by_table={} - no sealed NeoPlutarch marker on free space; this disk is never written",
                self.index,
                ident.driver,
                ident.name,
                arming.table,
                arming.blocked_by_table
            );
        }
        for claim in &arming.claims {
            serial_println!(
                "[ kernel ] block: writes armed device={} driver={} marker={} extent=0x{:x}..0x{:x} table={}",
                self.index,
                ident.driver,
                claim.marker.name(),
                claim.extent.start,
                claim.extent.end,
                arming.table
            );
        }
        arming
    }

    /// `block_write_guard` control: the pre-guard behaviour, every write
    /// reaches the driver, but only on a virtio-blk disk, which exists only
    /// under QEMU (R1-5): any other device is judged as usual.
    #[cfg(feature = "neo-test-block-write-guard-control")]
    fn judge(&self) -> write_guard_logic::Arming {
        use write_guard_logic::{Arming, Claim, Extent, WriteMarker};
        if self.inner.identity().driver != "virtio-blk" {
            return self.judge_guarded();
        }
        // Printed, so the marker is kept in the image for xtask to find.
        serial_println!(
            "[ kernel ] block: write guard OFF (control) {}",
            core::str::from_utf8(core::hint::black_box(&GUARD_OFF)).unwrap_or("")
        );
        Arming {
            claims: alloc::vec![Claim {
                marker: WriteMarker::Scratch,
                extent: Extent {
                    start: 0,
                    end: u64::MAX,
                },
            }],
            blocked_by_table: 0,
            table: "control",
        }
    }
}

impl BlockDevice for GuardedDevice {
    fn identity(&self) -> &BlockDeviceIdentity {
        self.inner.identity()
    }

    fn geometry(&self) -> BlockGeometry {
        self.inner.geometry()
    }

    fn submit(&self, request: BlockRequest<'_>) -> BlockResult<()> {
        #[cfg(feature = "installer")]
        let _plain = if request.op == BlockOp::Write {
            Some(self.begin_plain_write()?)
        } else {
            None
        };
        if request.op == BlockOp::Write && !self.write_allowed(request.lba, request.buffers) {
            return Err(BlockError::WriteRefused);
        }
        #[cfg(feature = "installer")]
        if request.op == BlockOp::Read {
            self.reads.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.submit(request)
    }

    fn supports_flush(&self) -> bool {
        self.inner.supports_flush()
    }

    fn flush(&self) -> BlockResult<()> {
        self.inner.flush()
    }

    fn read_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        #[cfg(feature = "installer")]
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.inner.read_block(lba, buffers)
    }

    fn write_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        #[cfg(feature = "installer")]
        let _plain = self.begin_plain_write()?;
        if !self.write_allowed(lba, buffers) {
            return Err(BlockError::WriteRefused);
        }
        self.inner.write_block(lba, buffers)
    }

    #[cfg(feature = "installer")]
    fn driver_identity(&self) -> Option<claim_logic::DriverIdentity> {
        self.inner.driver_identity()
    }

    #[cfg(feature = "installer")]
    fn media_state(&self) -> MediaState {
        self.inner.media_state()
    }
}

/// OT-87 R4.4: a plain write past the `claimed` check, counted until it
/// completes.
#[cfg(feature = "installer")]
struct PlainWrite<'a>(&'a AtomicU64);

#[cfg(feature = "installer")]
impl Drop for PlainWrite<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(feature = "installer")]
impl GuardedDevice {
    /// OT-87 R4.4: admit a plain write unless a claim holds the device.
    /// With [`Self::take_claim`] this is the store-then-load pair on both
    /// sides (all SeqCst): either this sees `claimed` and refuses, or the
    /// claim sees this write counted and waits for it to finish.
    fn begin_plain_write(&self) -> BlockResult<PlainWrite<'_>> {
        self.plain_writes.fetch_add(1, Ordering::SeqCst);
        let write = PlainWrite(&self.plain_writes);
        if self.claimed.load(Ordering::SeqCst) {
            serial_println!(
                "[ kernel ] block: WRITE REFUSED device={} - an install claim holds it",
                self.index
            );
            return Err(BlockError::WriteRefused);
        }
        Ok(write)
    }

    /// OT-87 R4.4: set `claimed`; if it was clear, wait until no plain write
    /// admitted before it is still in flight. Returns whether a claim
    /// already held the device. Process context: it yields while it waits.
    pub(super) fn take_claim(&self) -> bool {
        let already = self.claimed.swap(true, Ordering::SeqCst);
        if !already {
            while self.plain_writes.load(Ordering::SeqCst) != 0 {
                crate::sched::yield_now();
            }
        }
        already
    }

    /// OT-87 R4.1: the one write path past the guard. Only `install_claim`
    /// reaches it, and only through a live claim's checked write
    /// (`claim_logic::Claim::write`): this device's `claimed` flag must be set.
    fn claimed_write(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        if !self.claimed.load(Ordering::Acquire) {
            return Err(BlockError::WriteRefused);
        }
        self.inner.write_block(lba, buffers)
    }
}

/// Whether registered device `index` has any armed extent (judging it now if
/// no write has been attempted yet). Test and diagnostic use.
#[allow(dead_code)]
pub fn writes_allowed(index: usize) -> Option<bool> {
    let registry = REGISTRY.lock();
    let device = registry.guards.get(index)?.clone();
    drop(registry);
    Some(device.writes_allowed())
}

struct Registry {
    devices: Vec<Arc<dyn BlockDevice>>,
    guards: Vec<Arc<GuardedDevice>>,
    /// OT-87 R4.1: each slot's generation (THINK §2a item 1), and whether
    /// its device's death was already counted in it.
    #[cfg(feature = "installer")]
    generations: Vec<(u64, bool)>,
}

/// Process-context block registry: storage probes register devices and VFS
/// paths look them up; interrupt handlers use driver-local state instead.
static REGISTRY: Lazy<SwitchMutex<Registry>> = Lazy::new(|| {
    SwitchMutex::new(Registry {
        devices: Vec::new(),
        guards: Vec::new(),
        #[cfg(feature = "installer")]
        generations: Vec::new(),
    })
});

pub fn init() {
    serial_println!("[ kernel ] block: init (generic block-device substrate)");
    let summary = crate::drivers::pci::scan_storage_devices();
    serial_println!(
        "[ kernel ] block: pci scan complete functions={} virtio_blk={} ahci={} ahci_ok={} nvme={} nvme_ok={} attached_devices={}",
        summary.functions_seen,
        summary.virtio_blk_seen,
        summary.ahci_seen,
        summary.ahci_probe_ok,
        summary.nvme_seen,
        summary.nvme_probe_ok,
        device_count()
    );
}

#[allow(dead_code)]
pub fn register_device(device: Arc<dyn BlockDevice>) -> usize {
    let mut registry = REGISTRY.lock();
    let idx = registry.devices.len();
    let ident = device.identity();
    serial_println!(
        "[ kernel ] block: registered device index={} driver={} name={} pci_bdf={:?}",
        idx,
        ident.driver,
        ident.name,
        ident.pci_bdf
    );
    let guard = Arc::new(GuardedDevice {
        inner: device,
        index: idx,
        arming: spin::Once::new(),
        judging: AtomicBool::new(false),
        refused_writes: AtomicU64::new(0),
        #[cfg(feature = "installer")]
        claimed: AtomicBool::new(false),
        #[cfg(feature = "installer")]
        plain_writes: AtomicU64::new(0),
        #[cfg(feature = "installer")]
        reads: AtomicU64::new(0),
    });
    registry.guards.push(guard.clone());
    registry.devices.push(guard);
    #[cfg(feature = "installer")]
    registry.generations.push((1, false));
    idx
}

/// Whether the write guard of the registered device `ident` names allows a
/// write of `len` bytes at `lba`. For a driver's own test path that submits
/// below the `BlockDevice` trait (virtio's abandoned-write probe, Janus F3):
/// it must ask the same guard. False when no registered device matches.
#[cfg(all(feature = "neo-test", not(feature = "neo-test-bare-metal-mode")))]
pub(crate) fn guard_allows_raw_write(ident: &BlockDeviceIdentity, lba: u64, len: u64) -> bool {
    let guard = REGISTRY
        .lock()
        .guards
        .iter()
        .find(|g| g.inner.identity() == ident)
        .cloned();
    guard.is_some_and(|g| g.write_range_allowed(lba, len))
}

#[allow(dead_code)]
pub fn get_device(index: usize) -> Option<Arc<dyn BlockDevice>> {
    REGISTRY.lock().devices.get(index).cloned()
}

#[allow(dead_code)]
pub fn find_device_by_driver(driver: &str) -> Option<(usize, Arc<dyn BlockDevice>)> {
    REGISTRY
        .lock()
        .devices
        .iter()
        .enumerate()
        .find(|(_, device)| device.identity().driver == driver)
        .map(|(index, device)| (index, device.clone()))
}

pub fn device_count() -> usize {
    REGISTRY.lock().devices.len()
}

/// OT-87 R4.3: reads through registered device `index`'s object so far.
#[cfg(feature = "installer")]
pub fn read_count(index: usize) -> Option<u64> {
    REGISTRY
        .lock()
        .guards
        .get(index)
        .map(|g| g.reads.load(Ordering::Relaxed))
}

/// OT-87 R3: the validated GPT of `device` (the primary, else the backup),
/// read through the driver as the write guard reads it.
#[cfg(feature = "neofs-partition")]
pub fn read_gpt(device: &dyn BlockDevice) -> Option<gpt_logic::Gpt> {
    gpt_logic::read(&GuardReader(device))
}

/// The guard-off marker (`write_guard_logic::GUARD_OFF_MARKER`), in the
/// image of a control build only.
#[cfg(feature = "neo-test-block-write-guard-control")]
static GUARD_OFF: [u8; 35] = *write_guard_logic::GUARD_OFF_MARKER;

// Janus F2, R1-5: the guard-off control exists only for the QEMU neo-test
// harness. It can never be a bare-metal, product or progress-console (Zenbook)
// build, however the feature was named or reached (the resolved feature set,
// not a string), whichever xtask command built it.
#[cfg(all(
    feature = "neo-test-block-write-guard-control",
    any(
        feature = "neo-test-bare-metal-mode",
        feature = "product",
        feature = "fb-progress-console"
    )
))]
compile_error!(
    "neo-test-block-write-guard-control disables the block write guard: only in a QEMU neo-test build (never bare-metal, product or fb-progress-console)"
);

/// OT-66 L1 test-only (`neo-test-login-cut`; never in a shipped kernel): the
/// cut the sign-in scenario injects. Every write and flush a device
/// completes counts ([`login_cut::Counting`], which [`login_cut::install`]
/// puts under each guard, so only what the guard lets through reaches it);
/// once armed, the boot's step-th one ends the boot right after it
/// completes, as a power cut would
/// (`login_cut_steps`). The step comes from the boot module
/// `/modules/login-cut` (`LOGINCUT step=NNNN`, rewritten by xtask per boot;
/// 0 without one), so a verify boot and a cut boot run the same build.
/// Userland arms it (`SYS_TEST_BLOCK_CUT`) just before the password change it
/// cuts, so nothing else the boot writes counts.
#[cfg(feature = "neo-test-login-cut")]
pub mod login_cut {
    use super::{
        Arc, BlockDevice, BlockDeviceIdentity, BlockGeometry, BlockOp, BlockRequest, BlockResult,
        DmaBuffer, GuardedDevice, REGISTRY,
    };
    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    /// Put [`Counting`] under every registered device's guard. The test
    /// harness calls it once, after the PCI probes and before `fs::init`
    /// mounts anything, so every later handle is a counted one. It lives here,
    /// and not in `register_device`, so a kernel without the feature has the
    /// same lines in this file as before, and the same panic locations.
    pub fn install() {
        let mut registry = REGISTRY.lock();
        for slot in 0..registry.devices.len() {
            let old = registry.guards[slot].clone();
            let guard = Arc::new(GuardedDevice {
                inner: Arc::new(Counting {
                    inner: old.inner.clone(),
                }),
                index: old.index,
                arming: spin::Once::new(),
                judging: AtomicBool::new(false),
                refused_writes: AtomicU64::new(0),
                #[cfg(feature = "installer")]
                claimed: AtomicBool::new(false),
                #[cfg(feature = "installer")]
                reads: AtomicU64::new(0),
            });
            registry.guards[slot] = guard.clone();
            registry.devices[slot] = guard;
        }
    }

    /// The device a guard wraps, counting the writes and flushes it
    /// completes.
    pub(super) struct Counting {
        pub(super) inner: Arc<dyn BlockDevice>,
    }

    impl BlockDevice for Counting {
        fn identity(&self) -> &BlockDeviceIdentity {
            self.inner.identity()
        }

        fn geometry(&self) -> BlockGeometry {
            self.inner.geometry()
        }

        fn submit(&self, request: BlockRequest<'_>) -> BlockResult<()> {
            let (op, lba) = (request.op, request.lba);
            let result = self.inner.submit(request);
            if result.is_ok() && op == BlockOp::Write {
                after(b"write", lba);
            }
            result
        }

        fn supports_flush(&self) -> bool {
            self.inner.supports_flush()
        }

        fn flush(&self) -> BlockResult<()> {
            let result = self.inner.flush();
            if result.is_ok() {
                after(b"flush", 0);
            }
            result
        }

        fn read_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
            self.inner.read_block(lba, buffers)
        }

        fn write_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
            let result = self.inner.write_block(lba, buffers);
            if result.is_ok() {
                after(b"write", lba);
            }
            result
        }

        #[cfg(feature = "installer")]
        fn driver_identity(&self) -> Option<super::claim_logic::DriverIdentity> {
            self.inner.driver_identity()
        }

        #[cfg(feature = "installer")]
        fn media_state(&self) -> super::MediaState {
            self.inner.media_state()
        }
    }

    static OPS: AtomicU64 = AtomicU64::new(0);
    static AT: AtomicU64 = AtomicU64::new(0);

    /// The boot module's step: the digits after `step=`, else 0.
    pub fn module_step() -> u64 {
        let Some(bytes) = crate::find_module_by_path_suffix("/login-cut") else {
            return 0;
        };
        let Some(at) = bytes.windows(5).position(|w| w == b"step=") else {
            return 0;
        };
        bytes[at + 5..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .fold(0u64, |n, b| {
                n.saturating_mul(10).saturating_add(u64::from(b - b'0'))
            })
    }

    /// `SYS_TEST_BLOCK_CUT(op)`: 0, the boot's step; 1, arm at it (the count
    /// restarts) and return it; 2, disarm and return the ops since arming.
    pub fn syscall(op: u64) -> i64 {
        match op {
            0 => module_step() as i64,
            1 => {
                let step = module_step();
                OPS.store(0, Ordering::SeqCst);
                AT.store(step, Ordering::SeqCst);
                step as i64
            }
            2 => {
                AT.store(0, Ordering::SeqCst);
                OPS.swap(0, Ordering::SeqCst) as i64
            }
            _ => crate::errno::EINVAL,
        }
    }

    fn after(op: &'static [u8], lba: u64) {
        let n = OPS.fetch_add(1, Ordering::SeqCst) + 1;
        let at = AT.load(Ordering::SeqCst);
        if at == 0 {
            return;
        }
        if n == at {
            crate::test_harness::login_cut_fired(n, op, lba);
        }
        // Armed: each op the change makes, in order, so xtask can pick the
        // steps to cut from the calibration boot's list.
        crate::serial_println!(
            "LOGIN-CUT op step={} op={} lba={}",
            n,
            core::str::from_utf8(op).unwrap_or("?"),
            lba
        );
    }
}
