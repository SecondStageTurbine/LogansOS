// SPDX-License-Identifier: MPL-2.0
//! CDC-ECM USB-Ethernet class driver (Phase 7.5 BUILD-B, ANSPEC §10.8,
//! ADR-047): the first consumer of the 7.5-A class-driver seam. Implements
//! the EXISTING `NetDevice` trait over a claimed ECM function's bulk pair,
//! so the whole Phase-4 stack (DHCP/ARP/DNS/TCP/TLS) runs over USB with
//! zero new networking code.
//!
//! Bind sequence (ECM 1.2 §3/§5 — every step breadcrumbed for the E-bank):
//! MAC from the iMACAddress string descriptor → Configure Endpoint
//! (interrupt-IN + the alt-setting-1 bulk pair) → SET_CONFIGURATION →
//! SET_INTERFACE(data, alt 1) — THE classic ECM trap: alt 0 has no
//! endpoints — → SET_ETHERNET_PACKET_FILTER (a STALL is tolerated:
//! breadcrumbed, not fatal) → post RX/interrupt buffers → register with
//! `drivers::net`.
//!
//! Containment: the device lock is `lock_order::USB_DEVICE` (above all AI
//! levels — I-14(a)); completions arrive via the xHCI bounded per-EP queues
//! (I-14(b)); every streaming buffer address is fenced (`FencedDmaAddr`)
//! before the controller sees it (I-14(c1)).
//!
//! Kill-switch (Rule 1): [`set_enabled`]`(false)` makes `probe` decline, so
//! the offer path takes BUILD-A's unclaimed branch bit-for-bit.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};

use crate::drivers::block::DmaBuffer;
use crate::drivers::dma_arena::{self, FencedDmaAddr};
use crate::net::{LinkState, NetDevice, NetDeviceIdentity, NetError, NetResult, RxFrame, TxFrame};
use crate::sync::SwitchMutex;

use super::usb_parse::{self, EcmNotification};
use super::xhci::{ClaimedDevice, EndpointPipe};
use super::{UsbClassDriver, UsbDevice};

const RX_BUFFERS: usize = 4;
const TX_BUFFERS: usize = 8;
/// AX88179 SuperSpeed bulk-IN aggregation uses 20 KiB URBs; 2 KiB RX buffers
/// stayed posted forever on OptiPlex with a linked PHY and a live LAN.
const RX_BUFFER_LEN: usize = 20 * 1024;
/// One TX frame per buffer; ECM segments are <= wMaxSegmentSize (1514 typical).
const TX_BUFFER_LEN: usize = 2048;
/// Interrupt-notification slots: 64-byte sub-spans of one arena page.
const INT_SLOTS: usize = 4;
const INT_SLOT_LEN: usize = 64;
/// E3 discriminator: a silent interrupt endpoint (many devices notify only
/// on a CHANGE) flips link to assumed-up after this many service passes —
/// once, with a breadcrumb. The gate never hangs on link.
const LINK_ASSUME_UP_AFTER: u64 = 64;

const LINK_UNKNOWN: u8 = 0;
const LINK_UP: u8 = 1;
const LINK_DOWN: u8 = 2;

/// ASIX vendor id — the AX88179/178A USB-GbE family needs a vendor TX
/// enable its CDC-ECM config does not perform automatically.
const ASIX_VENDOR_ID: u16 = 0x0b95;

/// AX88179 cfg1-init diagnostics, captured into statics so the SCENARIO
/// (which runs after the framebuffer, i.e. after the NVMe log sink is
/// installed) can print them — the bind itself runs during driver init,
/// BEFORE the sink exists, so its serial breadcrumbs never reach the sink.
/// Flag bits: 0=attempted 1=set_cfg1_ok 2=cfg1_read_ok 3=write_ok
/// 4=readback_ok. Values are u16 register reads (0xFFFF = not read).
static AX_DIAG_FLAGS: AtomicU32 = AtomicU32::new(0);
static AX_DIAG_READ_VAL: AtomicU32 = AtomicU32::new(0xFFFF);
static AX_DIAG_RB_VAL: AtomicU32 = AtomicU32::new(0xFFFF);
/// H2 PHY link diagnostics: the PHY physical-status read, the post-link
/// AX_MEDIUM readback, and how many poll iterations link-up took (0xFFFF = not
/// read). The wire capture proved the chip never transmits until the link is
/// negotiated and AX_MEDIUM is rewritten post-link, so these localize whether
/// the PHY linked and whether EN_125MHZ finally stuck.
static AX_DIAG_PHYSR: AtomicU32 = AtomicU32::new(0xFFFF);
static AX_DIAG_FINAL_MEDIUM: AtomicU32 = AtomicU32::new(0xFFFF);
static AX_DIAG_LINKWAIT: AtomicU32 = AtomicU32::new(0);
/// The native link_reset TX-FIFO status (bRequest 0x81/0x8c): bit 0x40000000
/// clear == TX FIFO ready. The leading candidate for the missing TX-enable.
static AX_DIAG_FIFO: AtomicU32 = AtomicU32::new(0xFFFF_FFFF);
/// The last cdc-ecm bind failure reason (set on the declined-bind path).
static AX_BIND_FAIL: SwitchMutex<Option<&'static str>> = SwitchMutex::new(None);

#[allow(dead_code)] // gate seam — feature-gated scenario consumer
#[cfg(feature = "neo-test")]
pub fn debug_ax88179_diag() -> (u32, u16, u16, Option<&'static str>) {
    (
        AX_DIAG_FLAGS.load(Ordering::Relaxed),
        AX_DIAG_READ_VAL.load(Ordering::Relaxed) as u16,
        AX_DIAG_RB_VAL.load(Ordering::Relaxed) as u16,
        *AX_BIND_FAIL.lock(),
    )
}

/// PHY/TX diagnostics replayed into the sink: (physr, final_medium, linkwait, fifo_status).
#[allow(dead_code)] // gate seam — feature-gated scenario consumer
#[cfg(feature = "neo-test")]
pub fn debug_ax88179_phy() -> (u16, u16, u32, u32) {
    (
        AX_DIAG_PHYSR.load(Ordering::Relaxed) as u16,
        AX_DIAG_FINAL_MEDIUM.load(Ordering::Relaxed) as u16,
        AX_DIAG_LINKWAIT.load(Ordering::Relaxed),
        AX_DIAG_FIFO.load(Ordering::Relaxed),
    )
}

/// ASIX AX88179 vendor TX bring-up: a faithful port of the native
/// `ax88179_reset()` + `ax88179_link_reset()` register sequence, done in the
/// vendor config (config 1) where `AX_ACCESS_MAC`/`AX_ACCESS_PHY` work, before
/// the bind switches to the CDC-ECM config. Two wire captures proved the chip
/// transmits nothing under the earlier reduced init (PHY linked, USB bulk-OUT
/// completed, but 0 frames on copper), so this runs the full power-cycle +
/// COE/bulk/pause/medium writes + the TX-FIFO-ready handshake that arms TX.
/// Failures are logged, not fatal — RX already works, so a partial init still
/// yields a diagnosable run (the diagnostics are replayed into the sink).
fn ax88179_vendor_config_init(claimed: &mut super::xhci::ClaimedDevice) -> Option<bool> {
    const AX_ACCESS_MAC: u8 = 0x01;
    const VENDOR_OUT: u8 = 0x40; // vendor request, host→device, device recipient
    const VENDOR_IN: u8 = 0xC0; // vendor request, device→host, device recipient

    AX_DIAG_FLAGS.store(1, Ordering::Relaxed); // attempted

    // Activate the VENDOR config (config 1) — the silicon discriminator
    // proved the ECM config (3) STALLS AX_ACCESS_MAC and breaks its own
    // data path when probed, so the register writes must happen here. EP0
    // is config-independent; the bind's SET_CONFIGURATION(3) below switches
    // the device to ECM after this returns.
    if let Err(e) = claimed.control_no_data(0x00, 9, 1, 0) {
        crate::serial_println!(
            "cdc-ecm: AX88179 SET_CONFIGURATION(1) failed: {} — TX unreachable",
            e
        );
        crate::log::flush();
        return None;
    }
    AX_DIAG_FLAGS.fetch_or(2, Ordering::Relaxed); // set_cfg1_ok
    spin_delay(20_000_000);

    // Discriminator: does config 1 answer a vendor READ? (config 3 did not)
    let mut probe = [0u8; 2];
    let read_res = claimed.control_read(VENDOR_IN, AX_ACCESS_MAC, 0x22, 2, 2, &mut probe);
    crate::serial_println!(
        "cdc-ecm: AX88179 [cfg1] vendor READ AX_MEDIUM(0x22) = {:?} value={:02x}{:02x}",
        read_res,
        probe[1],
        probe[0],
    );
    crate::log::flush();
    if read_res.is_err() {
        crate::serial_println!(
            "cdc-ecm: AX88179 rejects vendor access in cfg1 too — TX unreachable from CDC mode"
        );
        crate::log::flush();
        return None;
    }
    AX_DIAG_FLAGS.fetch_or(4, Ordering::Relaxed); // cfg1_read_ok
    AX_DIAG_READ_VAL.store(
        u16::from_le_bytes([probe[0], probe[1]]) as u32,
        Ordering::Relaxed,
    );

    // === Faithful port of the native ax88179_reset() (authoritative Linux
    // sequence, fetched from the kernel source). Our prior IPRL-only init left
    // TX dead: PHY linked at gigabit + USB bulk-OUT completed, but TWO wire
    // captures showed ZERO frames egress. The full PHY power-cycle brings up the
    // MAC TX clock domain; the COE/bulk/pause writes set the defined TX state.
    // Full PHY power-cycle: reset (0x0000) then IPRL (0x0020), per native.
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x26, 2, &[0x00, 0x00]);
    spin_delay(40_000_000);
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x26, 2, &[0x20, 0x00]); // IPRL
    spin_delay(220_000_000); // ~native msleep(500)
    // AX_CLK_SELECT (0x33): ACS|BCS = 0x03.
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x33, 1, &[0x03]);
    spin_delay(100_000_000); // ~native msleep(200)
    // AX_RX_BULKIN_QCTRL (0x2e): default table[0] {07,4f,00,12,ff}.
    let _ = claimed.control_write(
        VENDOR_OUT,
        AX_ACCESS_MAC,
        0x2e,
        5,
        &[0x07, 0x4f, 0x00, 0x12, 0xff],
    );
    // AX_PAUSE_WATERLVL_LOW (0x54)=0x34, HIGH (0x55)=0x52.
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x54, 1, &[0x34]);
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x55, 1, &[0x52]);
    // AX_RXCOE_CTL (0x34) / AX_TXCOE_CTL (0x35): IP|TCP|UDP|TCPV6|UDPV6 = 0x65.
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x34, 1, &[0x65]);
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x35, 1, &[0x65]);
    // AX_RX_CTL (0x0b): DROPCRCERR|IPE|START|AP|AMALL|AB = 0x039a (native value).
    let rxctl: [u8; 2] = [0x9a, 0x03];
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x0b, 2, &rxctl);
    // AX_MONITOR_MOD (0x24): PMETYPE|PMEPOL|RWMP = 0x64.
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x24, 1, &[0x64]);
    // AX_MEDIUM_STATUS_MODE (0x22): native reset default 0x0133 (no EN_125MHZ yet).
    let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x22, 2, &[0x33, 0x01]);
    spin_delay(20_000_000);
    AX_DIAG_FLAGS.fetch_or(8, Ordering::Relaxed); // reset sequence written

    // === Port of ax88179_link_reset(): poll PHY link, TX-FIFO-ready handshake,
    // then write the negotiated medium. AX_ACCESS_PHY works in cfg1 (H2 proved).
    const AX_ACCESS_PHY: u8 = 0x02;
    const AX88179_PHY_ID: u16 = 0x03;
    const GMII_PHY_PHYSR: u16 = 0x11;
    const PHYSR_LINK: u16 = 0x0400;
    const PHYSR_SPEED: u16 = 0xC000;
    const PHYSR_GIGA: u16 = 0x8000;
    const PHYSR_FULL: u16 = 0x2000;
    let mut physr: u16 = 0;
    let mut physr_read = false;
    let mut waited: u32 = 0;
    loop {
        let mut p = [0u8; 2];
        if claimed
            .control_read(
                VENDOR_IN,
                AX_ACCESS_PHY,
                AX88179_PHY_ID,
                GMII_PHY_PHYSR,
                2,
                &mut p,
            )
            .is_ok()
        {
            physr_read = true;
            physr = u16::from_le_bytes([p[0], p[1]]);
            if physr & PHYSR_LINK != 0 {
                break;
            }
        }
        waited += 1;
        if waited >= 160 {
            break;
        }
        spin_delay(50_000_000);
    }
    AX_DIAG_PHYSR.store(physr as u32, Ordering::Relaxed);
    AX_DIAG_LINKWAIT.store(waited, Ordering::Relaxed);

    // TX-FIFO-ready handshake — the native link_reset's first act and the
    // leading candidate for the missing TX-enable: toggle RX_CTL off→on, then
    // poll the TX-FIFO status (bRequest 0x81, wValue 0x8c, 4 bytes) until bit
    // 0x40000000 clears (FIFO ready). Without this the prior init left TX dead.
    let mut fifo_wait: u32 = 0;
    let mut fifo_status: u32 = 0xFFFF_FFFF;
    loop {
        let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x0b, 2, &[0x00, 0x00]);
        let _ = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x0b, 2, &rxctl);
        let mut st = [0u8; 4];
        if claimed
            .control_read(VENDOR_IN, 0x81, 0x8c, 0, 4, &mut st)
            .is_ok()
        {
            fifo_status = u32::from_le_bytes(st);
            if fifo_status & 0x4000_0000 == 0 {
                break;
            }
        }
        fifo_wait += 1;
        if fifo_wait >= 50 {
            break;
        }
        spin_delay(20_000_000);
    }
    AX_DIAG_FIFO.store(fifo_status, Ordering::Relaxed);

    // Compose + write the final medium with the negotiated speed/duplex, adding
    // EN_125MHZ for gigabit (the native link_reset value 0x013b). Force gigabit/
    // full if the poll timed out — the cable is GbE and the PHY links by DORA.
    let linked = physr_read && physr & PHYSR_LINK != 0;
    let mut mode: u16 = 0x0100 | 0x0020 | 0x0010; // RECEIVE_EN | TXFLOW | RXFLOW (0x130)
    if !linked || (physr & PHYSR_SPEED) == PHYSR_GIGA {
        mode |= 0x0001 | 0x0008; // GIGAMODE | EN_125MHZ
    }
    if !linked || (physr & PHYSR_FULL) != 0 {
        mode |= 0x0002; // FULL_DUPLEX
    }
    let mb = mode.to_le_bytes();
    let post = claimed.control_write(VENDOR_OUT, AX_ACCESS_MAC, 0x22, 2, &[mb[0], mb[1]]);
    spin_delay(20_000_000);
    let mut fb = [0u8; 2];
    let fb_res = claimed.control_read(VENDOR_IN, AX_ACCESS_MAC, 0x22, 2, 2, &mut fb);
    let final_medium = if fb_res.is_ok() {
        u16::from_le_bytes([fb[0], fb[1]])
    } else {
        0xFFFF
    };
    AX_DIAG_FINAL_MEDIUM.store(final_medium as u32, Ordering::Relaxed);
    if fb_res.is_ok() {
        AX_DIAG_FLAGS.fetch_or(16, Ordering::Relaxed); // readback_ok
        AX_DIAG_RB_VAL.store(final_medium as u32, Ordering::Relaxed);
    }
    crate::serial_println!(
        "cdc-ecm: AX88179 [cfg1] native-init physr={:#06x} linked={} linkwait={} fifo={:#010x} fifo_wait={} medium_post={:?} final_medium={:#06x}",
        physr,
        linked,
        waited,
        fifo_status,
        fifo_wait,
        post,
        final_medium,
    );
    crate::log::flush();

    // Deconfigure back to the addressed state. The AX88179 ERRORS a direct
    // cfg1→cfg3 switch (silicon: bind_fail "control transfer failed at status
    // stage" with the device left in cfg1); from the addressed state the
    // bind's SET_CONFIGURATION(3) is the SAME addressed→cfg3 path that worked
    // in the RX-only boots. The MAC medium config (TX-enable, AX_ACCESS_MAC
    // register state) is not config-specific and persists across the dance.
    if let Err(e) = claimed.control_no_data(0x00, 9, 0, 0) {
        crate::serial_println!(
            "cdc-ecm: AX88179 deconfigure (SET_CONFIGURATION 0) failed: {}",
            e
        );
        crate::log::flush();
    }
    spin_delay(20_000_000);
    if physr_read { Some(linked) } else { None }
}

/// A coarse busy-wait — the AX88179 PHY power-up needs real settle time and
/// we have no sleep primitive on this path; the counts are generous spins.
fn spin_delay(iters: u64) {
    for _ in 0..iters {
        core::hint::spin_loop();
    }
}

/// The kill-switch: disabled ⇒ probe declines ⇒ BUILD-A's unclaimed path.
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Per-device observables (Forge M2: link/service state and the counters
/// are DEVICE-scoped, not module-global — a second adapter must not alias
/// the first's link state or inherit its assume-up timer; and `link_state`
/// stays a pure read because `service` owns the assume-up latch).
struct DeviceStats {
    tx_submissions: AtomicU64,
    tx_completions: AtomicU64,
    tx_bytes: AtomicU64,
    tx_last_len: AtomicU64,
    tx_last_completion_code: AtomicU64,
    rx_urb_completed: AtomicU64,
    rx_completions: AtomicU64,
    rx_zero_len: AtomicU64,
    rx_last_len: AtomicU64,
    rx_last_completion_code: AtomicU64,
    /// E5: bulk completions with a non-success, non-short code, by count
    /// (the per-event breadcrumb carries the code itself).
    ep_errors: AtomicU64,
    notifications: AtomicU64,
    services: AtomicU64,
    link: AtomicU8,
    ax88179_phy_link_up: AtomicBool,
    ax88179_false_down_logged: AtomicBool,
    assume_up_logged: AtomicBool,
}

impl DeviceStats {
    const fn new(initial_link: u8, ax88179_phy_link_up: bool) -> Self {
        Self {
            tx_submissions: AtomicU64::new(0),
            tx_completions: AtomicU64::new(0),
            tx_bytes: AtomicU64::new(0),
            tx_last_len: AtomicU64::new(0),
            tx_last_completion_code: AtomicU64::new(u64::MAX),
            rx_urb_completed: AtomicU64::new(0),
            rx_completions: AtomicU64::new(0),
            rx_zero_len: AtomicU64::new(0),
            rx_last_len: AtomicU64::new(0),
            rx_last_completion_code: AtomicU64::new(u64::MAX),
            ep_errors: AtomicU64::new(0),
            notifications: AtomicU64::new(0),
            services: AtomicU64::new(0),
            link: AtomicU8::new(initial_link),
            ax88179_phy_link_up: AtomicBool::new(ax88179_phy_link_up),
            ax88179_false_down_logged: AtomicBool::new(false),
            assume_up_logged: AtomicBool::new(false),
        }
    }
}

#[allow(dead_code)] // kill-switch gate seam — feature-gated scenario consumer
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Release);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// The registered class driver (offered every enumerated device).
pub struct CdcEcmDriver;

impl UsbClassDriver for CdcEcmDriver {
    fn name(&self) -> &'static str {
        "cdc-ecm"
    }

    /// Pure descriptor match: an ECM function anywhere in the configuration
    /// set. `find_ecm` returning None IS the E1 verdict ("not ECM") —
    /// distinct from every bind failure.
    fn probe(&self, device: &UsbDevice) -> bool {
        is_enabled() && usb_parse::find_ecm(&device.configurations).is_some()
    }

    fn bind(&self, device: &Arc<UsbDevice>, claimed: ClaimedDevice) -> Result<(), ClaimedDevice> {
        match bind_inner(device, claimed) {
            Ok(()) => Ok(()),
            Err((claimed, reason)) => {
                *AX_BIND_FAIL.lock() = Some(reason);
                crate::serial_println!(
                    "cdc-ecm: bind failed for vid={:04x} pid={:04x}: {} (E-bank)",
                    device.vendor_id,
                    device.product_id,
                    reason
                );
                crate::log::flush();
                Err(claimed)
            }
        }
    }
}

// clippy-bank: oversized ClaimedDevice errors are a CB-4 USB error-type refactor.
#[allow(clippy::result_large_err)]
fn bind_inner(
    device: &Arc<UsbDevice>,
    mut claimed: ClaimedDevice,
) -> Result<(), (ClaimedDevice, &'static str)> {
    let Some(topo) = usb_parse::find_ecm(&device.configurations) else {
        return Err((claimed, "no ecm topology (E1)"));
    };
    crate::serial_println!(
        "cdc-ecm: binding vid={:04x} pid={:04x} config={} comm_if={} data_if={} data_alt={} maxseg={}",
        device.vendor_id,
        device.product_id,
        topo.config_value,
        topo.comm_interface,
        topo.data_interface,
        topo.data_alt_setting,
        topo.max_segment_size,
    );
    crate::log::flush();

    // MAC from the iMACAddress string descriptor (12 UTF-16LE hex digits).
    let mut string_buf = [0u8; 64];
    if let Err(reason) = claimed.read_string(topo.mac_string_index, &mut string_buf) {
        return Err((claimed, reason));
    }
    let Some(mac) = usb_parse::mac_from_string_descriptor(&string_buf) else {
        return Err((claimed, "mac string descriptor malformed"));
    };

    // ASIX AX88179 TX-enable: must run in the vendor config (config 1)
    // BEFORE the ECM switch — the ECM config rejects AX_ACCESS_MAC. This
    // leaves the device in config 1; the SET_CONFIGURATION(3) below
    // activates ECM. The device is currently addressed/unconfigured (only
    // EP0 standard requests have run — the MAC string read above), so
    // SET_CONFIGURATION(1) here is the first config activation.
    let mut initial_link = LINK_UNKNOWN;
    let mut ax88179_phy_link_up = false;
    if device.vendor_id == ASIX_VENDOR_ID
        && let Some(linked) = ax88179_vendor_config_init(&mut claimed)
    {
        initial_link = if linked { LINK_UP } else { LINK_DOWN };
        ax88179_phy_link_up = linked;
    }

    // Allocate ALL streaming buffers BEFORE endpoints exist: an arena
    // failure here declines cleanly (the slot tears down on the BUILD-A
    // path); after Configure Endpoint there is no clean unwind in scope.
    let mut rx_pool = Vec::new();
    for _ in 0..RX_BUFFERS {
        match DmaBuffer::alloc_in_arena(RX_BUFFER_LEN) {
            Ok(buffer) => rx_pool.push(buffer),
            Err(_) => return Err((claimed, "rx buffer alloc (arena)")),
        }
    }
    let mut tx_pool = Vec::new();
    for _ in 0..TX_BUFFERS {
        match DmaBuffer::alloc_in_arena(TX_BUFFER_LEN) {
            Ok(buffer) => tx_pool.push(buffer),
            Err(_) => return Err((claimed, "tx buffer alloc (arena)")),
        }
    }
    let Ok(int_buf) = DmaBuffer::alloc_in_arena(4096) else {
        return Err((claimed, "interrupt buffer alloc (arena)"));
    };

    // Async pipes: interrupt-IN, bulk-IN, bulk-OUT — one Configure Endpoint.
    let mut pipes = match claimed.configure_endpoints(&[topo.int_ep, topo.bulk_in, topo.bulk_out]) {
        Ok(pipes) => pipes,
        Err(reason) => return Err((claimed, reason)),
    };
    let bulk_out = pipes.pop().expect("three pipes requested");
    let bulk_in = pipes.pop().expect("three pipes requested");
    let int_in = pipes.pop().expect("three pipes requested");

    // SET_CONFIGURATION — the device-side activation.
    if let Err(reason) = claimed.control_no_data(0x00, 9, topo.config_value as u16, 0) {
        return Err((claimed, reason));
    }
    // SET_INTERFACE(data, alt) — THE ECM trap (E2): the bulk endpoints only
    // exist on the non-default alt setting.
    if let Err(reason) = claimed.control_no_data(
        0x01,
        11,
        topo.data_alt_setting as u16,
        topo.data_interface as u16,
    ) {
        return Err((claimed, reason));
    }
    // SET_ETHERNET_PACKET_FILTER: DIRECTED | ALL_MULTICAST | BROADCAST.
    // Optional in practice — a STALL is tolerated with a breadcrumb.
    match claimed.control_no_data(0x21, 0x43, 0x000E, topo.comm_interface as u16) {
        Ok(()) => {}
        Err("control stalled") => {
            crate::serial_println!("cdc-ecm: packet-filter request stalled — tolerated");
        }
        Err(reason) => return Err((claimed, reason)),
    }

    // Post the pre-allocated buffers — fencing each address on the way in
    // (I-14(c1)); the buffers are fresh arena allocations, so the fence
    // cannot refuse them (a refusal here would be the E7 verdict).
    let mut inner = Inner {
        _claimed: claimed,
        int_in,
        bulk_in,
        bulk_out,
        rx_table: [const { None }; super::xhci::RING_TABLE],
        tx_table: [const { None }; super::xhci::RING_TABLE],
        int_table: [const { None }; super::xhci::RING_TABLE],
        rx_ready: Vec::new(),
        tx_free: tx_pool,
        int_buf: None,
    };
    for buffer in rx_pool {
        if let Err(reason) = inner.post_rx(buffer) {
            crate::serial_println!("cdc-ecm: rx post failed: {} (E7)", reason);
        }
    }
    for slot in 0..INT_SLOTS {
        match fence_sub_span(&int_buf, slot * INT_SLOT_LEN, INT_SLOT_LEN) {
            Ok(addr) => {
                let index = inner.int_in.submit(addr, INT_SLOT_LEN as u32);
                inner.int_table[index] = Some(slot);
            }
            Err(reason) => {
                crate::serial_println!("cdc-ecm: int post failed: {} (E7)", reason);
            }
        }
    }
    inner.int_buf = Some(int_buf);

    let mtu = if topo.max_segment_size as usize >= 64 + 14 {
        (topo.max_segment_size as usize - 14).min(TX_BUFFER_LEN - 14)
    } else {
        // Defensive fallback for a nonsensical wMaxSegmentSize — loud, so
        // a Stage-B mis-MTU is diagnosable from the sink (Forge m3).
        crate::serial_println!(
            "cdc-ecm: wMaxSegmentSize={} implausible — defaulting mtu to 1500",
            topo.max_segment_size
        );
        1500
    };
    let ecm = Arc::new(EcmNetDevice {
        identity: NetDeviceIdentity {
            driver: "cdc-ecm",
            name: "usb-ecm0",
            pci_bdf: None,
        },
        mac,
        mtu,
        inner: SwitchMutex::new_ordered(inner, crate::sync::lock_order::USB_DEVICE),
        stats: DeviceStats::new(initial_link, ax88179_phy_link_up),
    });
    crate::serial_println!(
        "cdc-ecm: up mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} mtu={} rx_buffers={} rx_len={} tx_buffers={} tx_len={}",
        mac[0],
        mac[1],
        mac[2],
        mac[3],
        mac[4],
        mac[5],
        mtu,
        RX_BUFFERS,
        RX_BUFFER_LEN,
        TX_BUFFERS,
        TX_BUFFER_LEN,
    );
    crate::log::flush();
    crate::drivers::net::register_device(ecm.clone());
    *PRIMARY.lock() = Some(ecm);
    Ok(())
}

static PRIMARY: SwitchMutex<Option<Arc<EcmNetDevice>>> = SwitchMutex::new(None);

struct EcmNetDevice {
    identity: NetDeviceIdentity,
    mac: [u8; 6],
    mtu: usize,
    inner: SwitchMutex<Inner>,
    stats: DeviceStats,
}

/// Ring-index → buffer tables: `EndpointPipe::submit` returns the ring
/// index; the Transfer Event's TRB pointer maps back to it.
struct Inner {
    _claimed: ClaimedDevice,
    int_in: EndpointPipe,
    bulk_in: EndpointPipe,
    bulk_out: EndpointPipe,
    rx_table: [Option<Arc<DmaBuffer>>; super::xhci::RING_TABLE],
    tx_table: [Option<Arc<DmaBuffer>>; super::xhci::RING_TABLE],
    /// Interrupt slots: the sub-span index (0..INT_SLOTS) posted at each
    /// ring index.
    int_table: [Option<usize>; super::xhci::RING_TABLE],
    rx_ready: Vec<(Arc<DmaBuffer>, usize)>,
    tx_free: Vec<Arc<DmaBuffer>>,
    int_buf: Option<Arc<DmaBuffer>>,
}

impl Inner {
    fn post_rx(&mut self, buffer: Arc<DmaBuffer>) -> Result<(), &'static str> {
        let addr = fence_whole(&buffer)?;
        let index = self.bulk_in.submit(addr, RX_BUFFER_LEN as u32);
        self.rx_table[index] = Some(buffer);
        Ok(())
    }

    fn drain_tx(&mut self, stats: &DeviceStats) {
        for event in self.bulk_out.completions() {
            let Some(index) = self.bulk_out.index_of(event.parameter) else {
                continue;
            };
            let code = event.completion_code();
            stats
                .tx_last_completion_code
                .store(code as u64, Ordering::Relaxed);
            if code != 1 && code != 13 {
                stats.ep_errors.fetch_add(1, Ordering::Relaxed);
                crate::serial_println!("cdc-ecm: bulk-out completion code={} (E5)", code);
            }
            if let Some(buffer) = self.tx_table[index].take() {
                self.tx_free.push(buffer);
                stats.tx_completions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn drain_rx(&mut self, stats: &DeviceStats) {
        for event in self.bulk_in.completions() {
            let Some(index) = self.bulk_in.index_of(event.parameter) else {
                continue;
            };
            let code = event.completion_code();
            stats
                .rx_last_completion_code
                .store(code as u64, Ordering::Relaxed);
            let Some(buffer) = self.rx_table[index].take() else {
                continue;
            };
            if code == 1 || code == 13 {
                stats.rx_urb_completed.fetch_add(1, Ordering::Relaxed);
                let len = RX_BUFFER_LEN.saturating_sub(event.transfer_residue() as usize);
                stats.rx_last_len.store(len as u64, Ordering::Relaxed);
                if len > 0 && len <= RX_BUFFER_LEN {
                    stats.rx_completions.fetch_add(1, Ordering::Relaxed);
                    self.rx_ready.push((buffer, len));
                    continue;
                }
                stats.rx_zero_len.fetch_add(1, Ordering::Relaxed);
            } else {
                stats.ep_errors.fetch_add(1, Ordering::Relaxed);
                crate::serial_println!("cdc-ecm: bulk-in completion code={} (E5)", code);
            }
            // Error or empty: repost the buffer directly.
            let _ = self.post_rx(buffer);
        }
    }

    fn drain_int(&mut self, stats: &DeviceStats) {
        let Some(int_buf) = self.int_buf.clone() else {
            return;
        };
        for event in self.int_in.completions() {
            let Some(index) = self.int_in.index_of(event.parameter) else {
                continue;
            };
            let Some(slot) = self.int_table[index].take() else {
                continue;
            };
            if event.completion_code() == 1 || event.completion_code() == 13 {
                let mut header = [0u8; 8];
                // SAFETY: `int_buf` is an exclusively owned arena page; the
                // slot sub-span was device-written (volatile) and is fully
                // inside the page (slot < INT_SLOTS, 8 ≤ INT_SLOT_LEN).
                unsafe {
                    let base = int_buf.as_ptr().add(slot * INT_SLOT_LEN);
                    for (i, byte) in header.iter_mut().enumerate() {
                        *byte = core::ptr::read_volatile(base.add(i));
                    }
                }
                match usb_parse::parse_notification(&header) {
                    Some(EcmNotification::NetworkConnection { up }) => {
                        stats.notifications.fetch_add(1, Ordering::Relaxed);
                        if !up
                            && stats.ax88179_phy_link_up.load(Ordering::Acquire)
                            && stats.link.load(Ordering::Acquire) == LINK_UP
                        {
                            if !stats.ax88179_false_down_logged.swap(true, Ordering::AcqRel) {
                                crate::serial_println!(
                                    "cdc-ecm: link notification connected=false ignored; AX88179 PHY linked"
                                );
                            }
                        } else {
                            let link = if up { LINK_UP } else { LINK_DOWN };
                            let previous = stats.link.swap(link, Ordering::AcqRel);
                            if previous != link {
                                crate::serial_println!(
                                    "cdc-ecm: link notification connected={}",
                                    up
                                );
                            }
                        }
                    }
                    Some(EcmNotification::ConnectionSpeedChange)
                    | Some(EcmNotification::Other(_)) => {
                        stats.notifications.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {}
                }
            }
            // Repost the slot.
            if let Ok(addr) = fence_sub_span(&int_buf, slot * INT_SLOT_LEN, INT_SLOT_LEN) {
                let new_index = self.int_in.submit(addr, INT_SLOT_LEN as u32);
                self.int_table[new_index] = Some(slot);
            }
        }
    }
}

fn fence_whole(buffer: &DmaBuffer) -> Result<FencedDmaAddr, &'static str> {
    dma_arena::fence(buffer.physical_addr().as_u64(), buffer.len() as u64)
        .map_err(|_| "buffer outside the fence (E7)")
}

fn fence_sub_span(
    buffer: &DmaBuffer,
    offset: usize,
    len: usize,
) -> Result<FencedDmaAddr, &'static str> {
    if offset + len > buffer.len() {
        return Err("sub-span out of buffer bounds");
    }
    dma_arena::fence(buffer.physical_addr().as_u64() + offset as u64, len as u64)
        .map_err(|_| "sub-span outside the fence (E7)")
}

impl NetDevice for EcmNetDevice {
    fn identity(&self) -> &NetDeviceIdentity {
        &self.identity
    }

    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }

    /// A pure read (Forge M2): `service` owns the assume-up latch; a
    /// getter never mutates link state.
    fn link_state(&self) -> LinkState {
        match self.stats.link.load(Ordering::Acquire) {
            LINK_UP => LinkState::Up,
            LINK_DOWN => LinkState::Down,
            _ => LinkState::Unknown,
        }
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn tx_reserve(&self) -> Option<TxFrame> {
        let mut inner = self.inner.lock();
        inner.drain_tx(&self.stats);
        let buffer = inner.tx_free.pop()?;
        Some(TxFrame { buffer })
    }

    fn tx_submit(&self, frame: TxFrame, len: usize) -> NetResult<()> {
        if len == 0 || len > self.mtu + 14 || len > TX_BUFFER_LEN {
            return Err(NetError::InvalidFrame);
        }
        let mut inner = self.inner.lock();
        // The CDC-ECM data path transmits raw Ethernet frames (per the spec).
        // NOTE: an earlier hypothesis prepended the native ASIX 8-byte TX header
        // here for the AX88179 — that was WRONG and silently corrupted config-3
        // TX (three wire captures showed 0 frames egress). The chip's ECM config
        // wants raw frames; the ASIX header belongs only to the native vendor
        // config. Raw frames + the cfg1 vendor TX-enable transmit cleanly on
        // silicon (real DHCP lease + DNS round-trip, BUILD-B Stage B).
        let addr = fence_whole(&frame.buffer).map_err(|_| NetError::Io)?;
        let index = inner.bulk_out.submit(addr, len as u32);
        inner.tx_table[index] = Some(frame.buffer);
        self.stats.tx_submissions.fetch_add(1, Ordering::Relaxed);
        self.stats.tx_bytes.fetch_add(len as u64, Ordering::Relaxed);
        self.stats.tx_last_len.store(len as u64, Ordering::Relaxed);
        Ok(())
    }

    fn rx_take(&self) -> Option<RxFrame> {
        let mut inner = self.inner.lock();
        inner.drain_rx(&self.stats);
        let (buffer, len) = inner.rx_ready.pop()?;
        Some(RxFrame {
            buffer,
            offset: 0,
            len,
        })
    }

    fn rx_recycle(&self, frame: RxFrame) {
        let mut inner = self.inner.lock();
        let _ = inner.post_rx(frame.buffer);
    }

    fn service(&self) {
        let services = self.stats.services.fetch_add(1, Ordering::AcqRel) + 1;
        // The F6 poll path: drain the controller's event ring into the
        // routed queues first — without this, a no-MSI environment (QEMU)
        // never delivers a single streaming completion.
        super::xhci::poll_events();
        {
            let mut inner = self.inner.lock();
            inner.drain_tx(&self.stats);
            inner.drain_rx(&self.stats);
            inner.drain_int(&self.stats);
        }
        // E3: a silent interrupt endpoint must not wedge the stack — the
        // SERVICE path (the writer side) latches assume-up after a budget,
        // once, breadcrumbed. The getter stays pure.
        if services >= LINK_ASSUME_UP_AFTER
            && self.stats.link.load(Ordering::Acquire) == LINK_UNKNOWN
            && !self.stats.assume_up_logged.swap(true, Ordering::AcqRel)
        {
            crate::serial_println!(
                "cdc-ecm: no link notification after {} services — assuming up (E3)",
                LINK_ASSUME_UP_AFTER
            );
            self.stats.link.store(LINK_UP, Ordering::Release);
        }
    }
}

// --- Gate observables (Rule 5: counters the scenarios consume) ----------------

#[derive(Copy, Clone, Default)]
pub struct CdcEcmIoDebug {
    pub tx_submissions: u64,
    pub tx_completions: u64,
    pub tx_bytes: u64,
    pub tx_last_len: u64,
    pub tx_last_completion_code: u64,
    pub rx_urb_completed: u64,
    pub rx_frames_parsed: u64,
    pub rx_zero_len: u64,
    pub rx_last_len: u64,
    pub rx_last_completion_code: u64,
    pub ep_errors: u64,
}

pub fn debug_primary_io() -> Option<CdcEcmIoDebug> {
    PRIMARY.lock().as_ref().map(|device| CdcEcmIoDebug {
        tx_submissions: device.stats.tx_submissions.load(Ordering::Relaxed),
        tx_completions: device.stats.tx_completions.load(Ordering::Relaxed),
        tx_bytes: device.stats.tx_bytes.load(Ordering::Relaxed),
        tx_last_len: device.stats.tx_last_len.load(Ordering::Relaxed),
        tx_last_completion_code: device.stats.tx_last_completion_code.load(Ordering::Relaxed),
        rx_urb_completed: device.stats.rx_urb_completed.load(Ordering::Relaxed),
        rx_frames_parsed: device.stats.rx_completions.load(Ordering::Relaxed),
        rx_zero_len: device.stats.rx_zero_len.load(Ordering::Relaxed),
        rx_last_len: device.stats.rx_last_len.load(Ordering::Relaxed),
        rx_last_completion_code: device.stats.rx_last_completion_code.load(Ordering::Relaxed),
        ep_errors: device.stats.ep_errors.load(Ordering::Relaxed),
    })
}

#[allow(dead_code)] // gate seam — feature-gated scenario consumer
#[cfg(feature = "neo-test")]
pub fn debug_counters() -> (u64, u64, u64, u64, u64) {
    PRIMARY
        .lock()
        .as_ref()
        .map(|device| {
            (
                device.stats.tx_submissions.load(Ordering::Relaxed),
                device.stats.tx_completions.load(Ordering::Relaxed),
                device.stats.rx_completions.load(Ordering::Relaxed),
                device.stats.notifications.load(Ordering::Relaxed),
                device.stats.ep_errors.load(Ordering::Relaxed),
            )
        })
        .unwrap_or((0, 0, 0, 0, 0))
}

#[allow(dead_code)] // gate seam — feature-gated scenario consumer
#[cfg(feature = "neo-test")]
pub fn debug_primary_device() -> Option<Arc<dyn NetDevice>> {
    PRIMARY
        .lock()
        .as_ref()
        .map(|device| device.clone() as Arc<dyn NetDevice>)
}

/// Whether an ECM device bound this boot (Stage-B conclusiveness: "claimed"
/// vs "present but not ECM" vs "no controller" are distinct verdicts).
#[allow(dead_code)] // Stage-B probe consumer
pub fn is_bound() -> bool {
    PRIMARY.lock().is_some()
}

/// The bound device's (slot, bulk-IN DCI) for the flood gate.
#[allow(dead_code)] // gate seam — feature-gated scenario consumer
#[cfg(feature = "neo-test")]
pub fn debug_bulk_in_route() -> Option<(u8, u8)> {
    PRIMARY.lock().as_ref().map(|device| {
        let inner = device.inner.lock();
        (inner._claimed.slot_id, inner.bulk_in.dci)
    })
}
