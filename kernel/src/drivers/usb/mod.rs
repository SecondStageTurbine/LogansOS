// SPDX-License-Identifier: MPL-2.0
//! USB host stack (Phase 7.5 BUILD-A enumeration, ADR-046; BUILD-B class
//! binding, ADR-047): the xHCI controller driver (`xhci`), the pure
//! descriptor parsers (`usb_parse`), the enumerated-device model, the
//! class-driver seam, and the first class driver (`cdc_ecm` → `NetDevice`).
//!
//! The claim contract (BUILD-B): a registered class driver is offered every
//! newly enumerated device. `probe` is a pure descriptor match; on a match,
//! `bind` receives the live [`xhci::ClaimedDevice`] — claiming it keeps the
//! slot, rings, and contexts alive for the driver's lifetime. A declined or
//! unprobed device takes the BUILD-A path bit-for-bit: census, then
//! teardown (DCBAA entry zeroed before the backing context drops). The
//! kill-switch is that branch: with `cdc_ecm` disabled, every offer
//! declines and the system is BUILD-A exactly.
//!
//! I-14 containment posture (dispatch §2, both builds):
//! - (a) the xHCI ISR touches ONLY USB state (an `IrqSafeMutex` + atomics);
//!   process-side USB locks are `SwitchMutex`es ordered ABOVE every
//!   AI-governance level (`sync::lock_order::USB_*`), so acquiring any AI
//!   lock while holding a USB lock panics in the lockdep.
//! - (b) the ISR drains a bounded burst (`xhci::ISR_EVENT_CAP`); BUILD-B
//!   adds per-endpoint completion queues with a hard cap
//!   (`xhci::EP_QUEUE_CAP`) and a drop counter — an RX flood degrades USB,
//!   never governance.
//! - (c1) every DMA address the controller sees is a `FencedDmaAddr` from
//!   the 7.5-0 arena — including the BUILD-B streaming (bulk/interrupt)
//!   path: `PhysAddr` does not appear in the programming API.

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::sync::SwitchMutex;

#[cfg(feature = "boot-eth0")]
pub mod boot_eth0;
#[cfg(feature = "boot-eth0")]
pub mod boot_eth0_logic;
pub mod cdc_ecm;
#[cfg(feature = "usb-hid")]
pub mod hid_keyboard;
#[cfg(feature = "usb-hid")]
pub mod hid_logic;
#[cfg(feature = "usb-hid-mouse")]
pub mod hid_mouse;
// L3u: USB mass storage (Bulk-Only Transport + SCSI), read-only.
#[cfg(feature = "usb-storage")]
pub mod msc;
#[cfg(feature = "usb-storage")]
pub mod msc_logic;
#[cfg(feature = "mt7601u")]
pub mod mt7601u;
#[cfg(feature = "mt7601u")]
pub mod operation_budget_logic;
pub mod usb_parse;
pub mod xhci;
#[cfg(feature = "xhci-silicon")]
pub mod xhci_silicon_logic;

pub use usb_parse::{UsbConfiguration, UsbEndpoint};

/// One enumerated USB device — identity + the parsed descriptor tree
/// (device → ALL configurations → interfaces/alt-settings → endpoints).
#[derive(Clone, Debug)]
pub struct UsbDevice {
    pub slot_id: u8,
    pub port: u8,
    /// The route string below the root port (0 on the root port itself), so
    /// a hub-port detach finds the devices under it.
    #[cfg(feature = "usb-hotplug")]
    pub route: u32,
    pub speed: UsbSpeed,
    pub vendor_id: u16,
    pub product_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub configurations: Vec<UsbConfiguration>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum UsbSpeed {
    Full,
    Low,
    High,
    Super,
    Unknown(u8),
}

impl UsbSpeed {
    pub fn from_portsc(bits: u32) -> Self {
        match bits {
            1 => Self::Full,
            2 => Self::Low,
            3 => Self::High,
            4 => Self::Super,
            other => Self::Unknown(other as u8),
        }
    }
    /// The default control-endpoint max packet size for enumeration
    /// (USB 2.0 §5.5.3 / xHCI §4.3): LS/FS start at 8, HS 64, SS 512.
    pub fn default_max_packet(&self) -> u16 {
        match self {
            Self::Low | Self::Full => 8,
            Self::High => 64,
            Self::Super => 512,
            Self::Unknown(_) => 8,
        }
    }
    /// Periodic-endpoint interval semantics differ below high speed
    /// (xHCI §6.2.3.6) — the pure `endpoint_interval` takes this.
    pub fn is_high_or_super(&self) -> bool {
        matches!(self, Self::High | Self::Super)
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Full => "full",
            Self::High => "high",
            Self::Super => "super",
            Self::Unknown(_) => "unknown",
        }
    }
}

/// The class-driver seam: `probe` is a pure descriptor match; `bind`
/// receives the live claimed slot and either keeps it (Ok — the driver now
/// owns the device) or hands it back (Err — the offer continues, and an
/// unclaimed device takes the BUILD-A teardown).
pub trait UsbClassDriver: Send + Sync {
    fn name(&self) -> &'static str;
    fn probe(&self, device: &UsbDevice) -> bool;
    // clippy-bank: oversized ClaimedDevice errors are a CB-4 USB error-type refactor.
    #[allow(clippy::result_large_err)]
    fn bind(
        &self,
        device: &Arc<UsbDevice>,
        claimed: xhci::ClaimedDevice,
    ) -> Result<(), xhci::ClaimedDevice> {
        let _ = device;
        Err(claimed)
    }
    /// The device this driver bound has been unplugged (#30): stop using it,
    /// fail its outstanding and later I/O, and withdraw whatever it
    /// registered. Called by the port service, from process context, with
    /// no USB lock held. The default keeps nothing, so needs nothing.
    #[cfg(feature = "usb-hotplug")]
    fn detach(&self, device: &UsbDevice) {
        let _ = device;
    }
}

/// A bound device and the driver that bound it.
#[cfg(feature = "usb-hotplug")]
type Binding = (Arc<UsbDevice>, Arc<dyn UsbClassDriver>);

/// Which driver bound which device (#30: the detach goes to it).
#[cfg(feature = "usb-hotplug")]
static BOUND: SwitchMutex<Vec<Binding>> =
    SwitchMutex::new_ordered(Vec::new(), crate::sync::lock_order::USB_CORE);

/// The port service saw `root_port` (and `route` below it: 0 for the root
/// port itself) disconnect: every enumerated device in that subtree leaves
/// the device list, and the driver that bound it is told (`detach`).
/// Returns how many bound devices were detached.
#[cfg(feature = "usb-hotplug")]
pub(crate) fn detach_subtree(root_port: u32, route: u32) -> usize {
    let under = |d: &UsbDevice| {
        u32::from(d.port) == root_port && xhci_silicon_logic::route_is_under(d.route, route)
    };
    DEVICES.lock().retain(|d| !under(d));
    #[cfg(feature = "xhci-silicon")]
    {
        let gone: Vec<u8> = BOUND
            .lock()
            .iter()
            .filter(|(d, _)| under(d))
            .map(|(d, _)| d.slot_id)
            .collect();
        CLAIMS.lock().retain(|(slot, _)| !gone.contains(slot));
    }
    let detached: Vec<Binding> = {
        let mut bound = BOUND.lock();
        let mut out = Vec::new();
        let mut index = 0;
        while index < bound.len() {
            if under(&bound[index].0) {
                out.push(bound.swap_remove(index));
            } else {
                index += 1;
            }
        }
        out
    };
    for (device, driver) in &detached {
        crate::serial_println!(
            "usb: device slot={} vid={:04x} pid={:04x} detached from '{}'",
            device.slot_id,
            device.vendor_id,
            device.product_id,
            driver.name()
        );
        driver.detach(device);
    }
    detached.len()
}

/// Registered class drivers + enumerated devices. Ordered ABOVE every AI
/// lock (I-14(a)): holding this while touching governance panics in the
/// lockdep.
static CLASS_DRIVERS: SwitchMutex<Vec<Arc<dyn UsbClassDriver>>> =
    SwitchMutex::new_ordered(Vec::new(), crate::sync::lock_order::USB_CORE);
static DEVICES: SwitchMutex<Vec<Arc<UsbDevice>>> =
    SwitchMutex::new_ordered(Vec::new(), crate::sync::lock_order::USB_CORE);
/// Which driver claimed which slot (the USB summary's `driver=` column).
#[cfg(feature = "xhci-silicon")]
static CLAIMS: spin::Mutex<Vec<(u8, &'static str)>> = spin::Mutex::new(Vec::new());

/// (slot, driver name) for every claimed device.
#[cfg(feature = "xhci-silicon")]
pub fn claims() -> Vec<(u8, &'static str)> {
    CLAIMS.lock().clone()
}

pub fn register_class_driver(driver: Arc<dyn UsbClassDriver>) {
    CLASS_DRIVERS.lock().push(driver);
}

/// Record an enumerated device, log its identity (the gate's observable),
/// and offer it to the registered class drivers. Returns the claimed-slot
/// state back to the caller when NO driver kept it — the caller then runs
/// the BUILD-A teardown. Returning `None` means a driver owns the slot now.
pub(crate) fn offer_device(
    device: UsbDevice,
    claimed: xhci::ClaimedDevice,
) -> Option<xhci::ClaimedDevice> {
    let mut line = String::new();
    use core::fmt::Write as _;
    let _ = write!(
        line,
        "usb: enumerated port={} slot={} speed={} vid={:04x} pid={:04x} class={:02x}/{:02x}/{:02x} configs={}",
        device.port,
        device.slot_id,
        device.speed.name(),
        device.vendor_id,
        device.product_id,
        device.class,
        device.subclass,
        device.protocol,
        device.configurations.len(),
    );
    crate::serial_println!("{}", line);
    crate::log::flush();

    let device = Arc::new(device);
    let drivers: Vec<Arc<dyn UsbClassDriver>> = CLASS_DRIVERS.lock().clone();
    let mut unclaimed = Some(claimed);
    for driver in drivers {
        if !driver.probe(&device) {
            continue;
        }
        let Some(offer) = unclaimed.take() else { break };
        match driver.bind(&device, offer) {
            Ok(()) => {
                crate::serial_println!(
                    "usb: device vid={:04x} pid={:04x} claimed by '{}'",
                    device.vendor_id,
                    device.product_id,
                    driver.name()
                );
                crate::log::flush();
                #[cfg(feature = "xhci-silicon")]
                CLAIMS.lock().push((device.slot_id, driver.name()));
                #[cfg(feature = "usb-hotplug")]
                BOUND.lock().push((device.clone(), driver.clone()));
                DEVICES.lock().push(device);
                return None;
            }
            Err(back) => {
                // E-bank: a probe-positive bind that backs out is logged
                // with the driver name — distinct from "not ECM" (E1).
                crate::serial_println!(
                    "usb: '{}' probed vid={:04x} pid={:04x} but declined to bind",
                    driver.name(),
                    device.vendor_id,
                    device.product_id
                );
                unclaimed = Some(back);
            }
        }
    }
    DEVICES.lock().push(device);
    unclaimed
}

/// The enumerated-device snapshot (gate + class-driver consumers).
#[allow(dead_code)] // gate/baseline entry point — feature-gated scenario consumer
pub fn devices() -> Vec<Arc<UsbDevice>> {
    DEVICES.lock().clone()
}

/// Kill-switch leg (ISC-10): re-run the PROBE-ONLY offer loop (BUILD-A's
/// exact semantics — no bind, no claim) over the recorded devices and
/// return how many drivers matched. With `cdc_ecm` disabled this is 0 for
/// an ECM device — the disabled branch IS the BUILD-A branch.
#[allow(dead_code)] // kill-switch gate seam — feature-gated scenario consumer
#[cfg(feature = "neo-test")]
pub fn debug_reoffer_all() -> usize {
    let devices = DEVICES.lock().clone();
    let drivers: Vec<Arc<dyn UsbClassDriver>> = CLASS_DRIVERS.lock().clone();
    let mut matches = 0usize;
    for device in &devices {
        for driver in &drivers {
            if driver.probe(device) {
                matches += 1;
            }
        }
    }
    matches
}

pub fn init() {
    #[cfg(feature = "usb-hid")]
    hid_keyboard::init();
    // OT (Milestone-A): the USB HID mouse pointer driver. Feature-gated ⇒ off means
    // it is not registered, so the offer path is keyboard-only (BUILD-A/B exactly) —
    // the mouse kill-switch. Registered independently of the keyboard.
    #[cfg(feature = "usb-hid-mouse")]
    hid_mouse::init();
    register_class_driver(Arc::new(cdc_ecm::CdcEcmDriver));
    // L3u-2: USB sticks as read-only block devices. Off (the kill-switch):
    // not registered, so a mass-storage device enumerates and is left
    // unclaimed, as before.
    #[cfg(feature = "usb-storage")]
    msc::init();
    // Phase 7.5 BUILD-W1: the MT7601U WiFi scan driver (feature-gated; off ⇒
    // not registered ⇒ the offer path is BUILD-A/B exactly).
    #[cfg(feature = "mt7601u")]
    mt7601u::init();
    xhci::init();
    // F11 S4: the setup menu's Network gate boots the product's bring-up too
    // (`neo-test-setup-net-boot`: its page reads the lease this takes).
    #[cfg(all(
        feature = "boot-eth0",
        any(
            not(feature = "neo-test"),
            feature = "neo-test-product-integration",
            feature = "neo-test-setup-net-boot"
        )
    ))]
    boot_eth0::bring_up();
}

/// Zenbook USB input lane: start the port service — a kernel thread that
/// re-reads root and hub ports every [`PORT_SERVICE_TICKS`] and enumerates
/// what connected after the boot scan (`xhci::hotplug`). Called once the
/// scheduler exists. Feature off: not compiled, ports are scanned once.
#[cfg(feature = "usb-hotplug")]
pub fn start_port_service() {
    if cfg!(feature = "neo-test-usb-hotplug-control") {
        crate::serial_println!("usb-hotplug: port service not started (control)");
        return;
    }
    if !xhci::is_running() {
        crate::serial_println!("usb-hotplug: no running controller - port service not started");
        return;
    }
    let id = crate::sched::kthread::spawn_detached("usb-ports", port_service);
    crate::serial_println!(
        "usb-hotplug: port service started (kthread slot={}, every {} ticks)",
        id.slot(),
        PORT_SERVICE_TICKS
    );
}

/// Port service period: 250 ms at the 100 Hz tick.
#[cfg(feature = "usb-hotplug")]
pub const PORT_SERVICE_TICKS: u64 = 25;

/// Port service passes (the overlay's `usb: loops`): about 4 a second.
#[cfg(all(feature = "usb-hotplug", feature = "tick-sleep"))]
static PORT_LOOPS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

#[cfg(all(feature = "usb-hotplug", feature = "tick-sleep"))]
pub fn port_service_loops() -> u64 {
    PORT_LOOPS.load(core::sync::atomic::Ordering::Relaxed)
}

#[cfg(feature = "usb-hotplug")]
fn port_service(
    stop: crate::sched::kthread::KthreadStopToken,
) -> crate::sched::kthread::KthreadExitState {
    loop {
        if stop.is_stop_requested() {
            return crate::sched::kthread::KthreadExitState::Success;
        }
        #[cfg(feature = "tick-sleep")]
        PORT_LOOPS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        nap_ticks(PORT_SERVICE_TICKS);
        xhci::hotplug::service_ports();
    }
}

/// Wait at least `ticks` timer ticks, letting other threads run. With the
/// wait multiplex or `tick-sleep` the thread parks (CP-81); with neither, it
/// yields until the tick count has moved on (the boot path's own
/// pre-multiplex idiom).
#[cfg(any(feature = "usb-hotplug", feature = "xhci-silicon"))]
pub fn nap_ticks(ticks: u64) {
    #[cfg(feature = "wait-multiplex")]
    crate::sched::wait_any::sleep_ticks(ticks);
    #[cfg(all(not(feature = "wait-multiplex"), feature = "tick-sleep"))]
    crate::sched::tick_sleep::sleep_ticks(ticks);
    #[cfg(all(not(feature = "wait-multiplex"), not(feature = "tick-sleep")))]
    {
        let ticks_now =
            || crate::arch::x86_64::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
        let deadline = ticks_now().saturating_add(ticks.max(1));
        while ticks_now() < deadline {
            crate::sched::yield_now();
            core::hint::spin_loop();
        }
    }
}

/// Image A without `boot-evidence-holds`: the USB summary as it stands now,
/// printed (serial, and the boot console), with no wait for a bind, no input
/// check and no hold. A keyboard or mouse that binds later is still taken by
/// the port service (`usb_hotplug`). Returns whether one is bound now.
#[cfg(feature = "xhci-silicon")]
pub fn input_evidence_summary() -> bool {
    xhci::log_usb_summary();
    let bound = xhci::keyboard_bound() || xhci::mouse_bound();
    if !bound {
        crate::serial_println!(
            "usb-summary: no keyboard or mouse bound yet - not waiting (boot-evidence-holds off)"
        );
    }
    crate::log::flush();
    bound
}

/// Zenbook USB input lane: the on-screen evidence. Waits up to `wait_ticks`
/// for a keyboard or mouse to bind, prints the USB summary, and if nothing is
/// bound, holds the screen up to `hold_ticks` more (ending early if a device
/// binds, then printing the summary again). Called before the desktop takes
/// the framebuffer, so the summary is the last thing the boot console shows.
///
/// When a device is bound, an input check follows: up to `check_ticks` for a
/// key press or mouse move, with the drivers pumped so their report counters
/// move. Bound with `reports=0` after the check means the interrupt endpoint
/// never completed; reports counted here but no input at the desktop means
/// the desktop's input path.
#[cfg(feature = "xhci-silicon")]
pub fn input_evidence_hold_for(wait_ticks: u64, hold_ticks: u64, check_ticks: u64) -> bool {
    let bound = || xhci::keyboard_bound() || xhci::mouse_bound();
    wait_ticks_until(wait_ticks, bound);
    xhci::log_usb_summary();
    if bound() {
        input_check(check_ticks, hold_ticks);
        return true;
    }
    crate::serial_println!(
        "usb-summary: NO keyboard or mouse after {} s - holding this screen up to {} s (plugging one in ends the hold)",
        wait_ticks / 100,
        hold_ticks / 100
    );
    crate::log::flush();
    let late = wait_ticks_until(hold_ticks, bound);
    if late {
        crate::serial_println!("usb-summary: a device bound during the hold:");
        xhci::log_usb_summary();
    }
    late
}

#[cfg(feature = "xhci-silicon")]
fn input_check(check_ticks: u64, hold_ticks: u64) {
    if check_ticks == 0 {
        crate::serial_println!("usb-summary: input device bound - continuing");
        crate::log::flush();
        return;
    }
    crate::serial_println!(
        "usb-summary: INPUT CHECK - press a key and move the mouse now ({} s)",
        check_ticks / 100
    );
    crate::log::flush();
    // Ends once every bound device has sent a report, or at the deadline.
    let (keyboard_before, mouse_before) = xhci::report_counts();
    let all_reported = || {
        let (keyboard, mouse) = xhci::report_counts();
        (!xhci::keyboard_bound() || keyboard > keyboard_before)
            && (!xhci::mouse_bound() || mouse > mouse_before)
    };
    wait_ticks_until(check_ticks, || {
        pump_input_for_check();
        all_reported()
    });
    let moved = {
        let (keyboard, mouse) = xhci::report_counts();
        keyboard > keyboard_before || mouse > mouse_before
    };
    let (keyboard, mouse) = xhci::report_counts();
    crate::serial_println!(
        "usb-summary: input check {}: keyboard reports={} mouse reports={}",
        if moved {
            "RECEIVED"
        } else {
            "NOTHING RECEIVED"
        },
        keyboard,
        mouse
    );
    crate::log::flush();
    if !moved {
        // Bound but silent: keep the evidence on screen, as for no device.
        crate::serial_println!(
            "usb-summary: holding this screen up to {} s (a key press or mouse move ends the hold)",
            hold_ticks / 100
        );
        crate::log::flush();
        if wait_ticks_until(hold_ticks, || {
            pump_input_for_check();
            all_reported()
        }) {
            let (keyboard, mouse) = xhci::report_counts();
            crate::serial_println!(
                "usb-summary: input arrived during the hold: keyboard reports={} mouse reports={}",
                keyboard,
                mouse
            );
            crate::log::flush();
        }
    }
}

/// Drain the bound HID drivers once, so their report counters move before
/// any desktop pumps them. Events pumped here are routed (or read) like any
/// others; this runs only during the boot-time input check.
#[cfg(feature = "xhci-silicon")]
fn pump_input_for_check() {
    xhci::poll_events();
    #[cfg(all(feature = "usb-hid", feature = "atrium-input-routing"))]
    hid_keyboard::pump_routed_events();
    #[cfg(all(feature = "usb-hid", not(feature = "atrium-input-routing")))]
    while hid_keyboard::try_read_byte().is_some() {}
    #[cfg(all(feature = "usb-hid-mouse", feature = "atrium-input-routing"))]
    hid_mouse::pump_routed_events();
}

#[cfg(feature = "xhci-silicon")]
fn wait_ticks_until(ticks: u64, mut done: impl FnMut() -> bool) -> bool {
    let ticks_now =
        || crate::arch::x86_64::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
    let deadline = ticks_now().saturating_add(ticks);
    loop {
        if done() {
            return true;
        }
        if ticks_now() >= deadline {
            return false;
        }
        nap_ticks(10);
    }
}
