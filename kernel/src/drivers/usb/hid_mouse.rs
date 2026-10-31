// SPDX-License-Identifier: MPL-2.0
//! USB HID boot-protocol mouse class driver — the pointer for the GUI (OT-52).
//!
//! The exact mirror of `hid_keyboard.rs` on the same xHCI host stack: it claims
//! a boot-protocol **mouse** interface (HID class 0x03 / boot subclass 0x01 /
//! mouse protocol 0x02), posts arena-fenced report buffers on the interrupt-IN
//! endpoint, decodes each boot mouse report into a [`PointerEvent`], and feeds a
//! bounded pointer-event queue the input consumer (later the OT-52 compositor)
//! reads. This driver DELIVERS events only — routing a pointer event to a
//! focused surface is OT-52's job, not here.
//!
//! I-14 is inherited from xHCI exactly like the keyboard, no new bus master:
//!  - (a) the interrupt-EP path acquires NO AI-governance lock; it only routes
//!    completions through xHCI's bounded per-endpoint queue, and the driver's
//!    process-side lock is an `IrqSafeMutex` ordered at `USB_DEVICE` (above every
//!    AI-governance level, so taking a governance lock under it panics lockdep).
//!  - (b) the pointer-event queue is hard-capped (`PENDING_MAX`); a spamming or
//!    flooding mouse has its excess events DROPPED (counted), never queued
//!    unboundedly — it degrades the pointer, never governance liveness.
//!  - (c) every report buffer submitted to the controller is a `FencedDmaAddr`
//!    through `dma_arena::fence(...)` under the existing xHCI IOMMU containment —
//!    no new DMA path (`PhysAddr` never appears in the programming API here).
//!
//! Composite-device note: this stack claims one slot per enumerated device, so a
//! *separate* USB mouse binds cleanly (the keyboard driver declines it on the
//! protocol byte). A *composite* single-slot keyboard+mouse device is claimed
//! whole by whichever HID driver probes first — driving both interfaces needs
//! multi-interface claim support (a future build); the gate + bare-metal use a
//! separate mouse.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::drivers::block::DmaBuffer;
use crate::drivers::dma_arena::{self, FencedDmaAddr};
use crate::sync::irqsafe_mutex::{IrqSafeGuard, IrqSafeMutex};

use super::hid_logic::{BOOT_MOUSE_REPORT_MIN_LEN, BootMouseDecoder, PointerEvent};
use super::usb_parse;
use super::xhci::{ClaimedDevice, EndpointPipe};
use super::{UsbClassDriver, UsbDevice};

const REPORT_SLOTS: usize = 16;
/// Fixed per-slot report buffer size: generous enough for any boot mouse report
/// (3–4 meaningful bytes) and the common 8-byte interrupt-IN `wMaxPacketSize`.
const MOUSE_REPORT_SLOT_LEN: usize = 8;
/// Bounded pointer-event queue depth (I-14(b)): excess events are dropped, not
/// queued unboundedly, so a flooding mouse cannot exhaust memory.
const PENDING_MAX: usize = 64;

static REPORTS: AtomicU64 = AtomicU64::new(0);
static EVENTS: AtomicU64 = AtomicU64::new(0);
static EP_ERRORS: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicU64 = AtomicU64::new(0);

pub struct HidMouseDriver;

impl UsbClassDriver for HidMouseDriver {
    fn name(&self) -> &'static str {
        "hid-mouse"
    }

    fn probe(&self, device: &UsbDevice) -> bool {
        usb_parse::find_hid_boot_mouse(&device.configurations).is_some()
    }

    fn bind(&self, device: &Arc<UsbDevice>, claimed: ClaimedDevice) -> Result<(), ClaimedDevice> {
        match bind_inner(device, claimed) {
            Ok(()) => Ok(()),
            Err((claimed, reason)) => {
                crate::serial_println!(
                    "hid-mouse: bind failed for vid={:04x} pid={:04x}: {}",
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

pub fn init() {
    super::register_class_driver(Arc::new(HidMouseDriver));
}

// clippy-bank: oversized ClaimedDevice errors are a CB-4 USB error-type refactor.
#[allow(clippy::result_large_err)]
fn bind_inner(
    device: &Arc<UsbDevice>,
    mut claimed: ClaimedDevice,
) -> Result<(), (ClaimedDevice, &'static str)> {
    let Some(topo) = usb_parse::find_hid_boot_mouse(&device.configurations) else {
        return Err((claimed, "no boot mouse topology"));
    };
    // Effective report length: the endpoint's max packet, clamped to a real boot
    // mouse report window. Do NOT hard-assume 3 (variable-length trap) — read the
    // endpoint and decode defensively; the decoder handles 3 vs 4 either way.
    let report_len =
        (topo.int_in.max_packet as usize).clamp(BOOT_MOUSE_REPORT_MIN_LEN, MOUSE_REPORT_SLOT_LEN);
    crate::serial_println!(
        "hid-mouse: binding vid={:04x} pid={:04x} config={} interface={} int_ep=0x{:02x} max_packet={} report_len={}",
        device.vendor_id,
        device.product_id,
        topo.config_value,
        topo.interface,
        topo.int_in.address,
        topo.int_in.max_packet,
        report_len
    );
    crate::log::flush();

    let Ok(report_buf) = DmaBuffer::alloc_in_arena(4096) else {
        return Err((claimed, "report buffer alloc (arena)"));
    };
    let mut pipes = match claimed.configure_endpoints(&[topo.int_in]) {
        Ok(pipes) => pipes,
        Err(reason) => return Err((claimed, reason)),
    };
    let int_in = pipes.pop().expect("one pipe requested");

    // SET_CONFIGURATION, then SET_PROTOCOL(boot=0) on the mouse interface, then
    // SET_IDLE (infinite) — the exact keyboard sequence, on the mouse interface.
    // SET_PROTOCOL(boot) is MANDATORY: without it some mice send the (variable)
    // report-protocol format, not boot format → garbage decode (trap-bank M3).
    if let Err(reason) = claimed.control_no_data(0x00, 9, topo.config_value as u16, 0) {
        return Err((claimed, reason));
    }
    if let Err(reason) = claimed.control_no_data(0x21, 0x0B, 0, topo.interface as u16) {
        return Err((claimed, reason));
    }
    match claimed.control_no_data(0x21, 0x0A, 0, topo.interface as u16) {
        Ok(()) | Err("control stalled") => {}
        Err(reason) => return Err((claimed, reason)),
    }

    let mut inner = Inner {
        _claimed: claimed,
        int_in,
        report_buf: report_buf.clone(),
        report_len,
        report_table: [const { None }; super::xhci::RING_TABLE],
        decoder: BootMouseDecoder::new(),
        pending: Vec::with_capacity(PENDING_MAX),
        #[cfg(feature = "atrium-input-routing")]
        stage: crate::atrium::focus::Stage::with_capacity(
            crate::atrium::focus::INPUT_STAGE_CAPACITY,
        ),
    };
    for slot in 0..REPORT_SLOTS {
        if let Err(reason) = inner.post_report(slot) {
            crate::serial_println!("hid-mouse: report post failed: {}", reason);
        }
    }

    let bound = Arc::new(Mouse {
        inner: IrqSafeMutex::new_ordered(inner, crate::sync::lock_order::USB_DEVICE),
    });
    // CP-37: `PRIMARY` is only ever held IF=0 (see `primary`).
    #[cfg(not(feature = "usb-hotplug"))]
    crate::arch::x86_64::without_interrupts(|| *primary_slot() = Some(bound));
    // With the port service a re-plugged device binds again and replaces
    // this one. The replaced device is leaked, not dropped: its rings and
    // contexts may still be referenced by the controller (no Disable Slot
    // is issued), so freeing them could let a late DMA write land in reused
    // memory (`xhci::hotplug` module docs).
    #[cfg(feature = "usb-hotplug")]
    if let Some(replaced) =
        crate::arch::x86_64::without_interrupts(|| primary_slot().replace(bound))
    {
        core::mem::forget(replaced);
    }
    crate::serial_println!(
        "hid-mouse: ready boot_protocol=true report_len={} report_slots={}",
        report_len,
        REPORT_SLOTS
    );
    crate::serial_println!(
        "hid_mouse bound vid={:04x} pid={:04x}",
        device.vendor_id,
        device.product_id
    );
    crate::log::flush();
    Ok(())
}

/// CP-37/CP-56 (#55): `IrqSafeMutex`, like the device lock below, so an
/// acquisition is IF=0 by construction, not by each caller remembering.
static PRIMARY: IrqSafeMutex<Option<Arc<Mouse>>> = IrqSafeMutex::new(None);

/// The bound device. CP-37: `PRIMARY` is a spinning lock taken by
/// the RECV pump with interrupts OFF, so every acquisition is IF=0 too —
/// an fd-0 reader preempted while holding it at IF=1 would deadlock a
/// pump spinning on the same CPU (Jarvis, #44 RETURN F2).
fn primary() -> Option<Arc<Mouse>> {
    crate::arch::x86_64::without_interrupts(|| primary_slot().clone())
}

/// Every `PRIMARY` acquisition (the bind and every `primary`), sampled by
/// the CP-37 witness (Jarvis N1).
#[track_caller]
fn primary_slot() -> IrqSafeGuard<'static, Option<Arc<Mouse>>> {
    #[cfg(feature = "neo-test-hid-contention")]
    crate::drivers::usb::hid_keyboard::note_acquisition();
    PRIMARY.lock()
}

/// CP-37: a device-lock section runs with interrupts OFF. The device-IF
/// control runs it as the caller has it (the pre-fix shape, for this lock
/// class), so `hid_contention`'s witness must fire.
fn device_section<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(feature = "neo-test-hid-device-if-control")]
    return f();
    #[cfg(not(feature = "neo-test-hid-device-if-control"))]
    crate::arch::x86_64::without_interrupts(f)
}

struct Mouse {
    inner: IrqSafeMutex<Inner>,
}

impl Mouse {
    /// The device lock: every acquisition goes through here and is sampled
    /// by the CP-37 witness (Jarvis N1). The device-IF control takes it as
    /// the caller has it.
    #[track_caller]
    fn device(&self) -> impl core::ops::DerefMut<Target = Inner> + '_ {
        #[cfg(feature = "neo-test-hid-contention")]
        crate::drivers::usb::hid_keyboard::note_acquisition();
        #[cfg(feature = "neo-test-hid-device-if-control")]
        return self.inner.lock_as_caller();
        #[cfg(not(feature = "neo-test-hid-device-if-control"))]
        self.inner.lock()
    }
}

struct Inner {
    _claimed: ClaimedDevice,
    int_in: EndpointPipe,
    report_buf: Arc<DmaBuffer>,
    report_len: usize,
    report_table: [Option<usize>; super::xhci::RING_TABLE],
    decoder: BootMouseDecoder,
    pending: Vec<PointerEvent>,
    /// S1-F1: decoded events not yet in `DELIVERY` (bounded, motion coalesces).
    #[cfg(feature = "atrium-input-routing")]
    stage: crate::atrium::focus::Stage,
}

impl Inner {
    fn post_report(&mut self, slot: usize) -> Result<(), &'static str> {
        let addr = fence_report_slot(&self.report_buf, slot)?;
        let index = self.int_in.submit(addr, self.report_len as u32);
        self.report_table[index] = Some(slot);
        Ok(())
    }

    fn drain(&mut self) {
        super::xhci::poll_events();
        for event in self.int_in.completions() {
            let Some(index) = self.int_in.index_of(event.parameter) else {
                continue;
            };
            let Some(slot) = self.report_table[index].take() else {
                continue;
            };
            let code = event.completion_code();
            // Completion code 1 = Success, 13 = Short Packet (a boot mouse report
            // shorter than the posted length — normal, decode it).
            if code == 1 || code == 13 {
                let report = read_report(&self.report_buf, slot);
                REPORTS.fetch_add(1, Ordering::Relaxed);
                if let Some(pe) = self.decoder.decode_report(&report[..self.report_len]) {
                    #[cfg(feature = "atrium-input-routing")]
                    if crate::atrium::input_routing_enabled() {
                        // A9-C2: the wheel byte rides along under
                        // atrium-pointer-wheel; without it, 0 (today's event).
                        let event = crate::atrium::input_logic::InputEventV1::pointer_wheel(
                            pe.dx,
                            pe.dy,
                            pe.buttons,
                            if cfg!(feature = "atrium-pointer-wheel") {
                                pe.wheel
                            } else {
                                0
                            },
                        );
                        // Jarvis F2: the raw state is latched before staging,
                        // so it is current even if the event is later dropped.
                        crate::atrium::focus::note_raw_buttons(pe.buttons);
                        // S1-F1: stage, flush at the end of the drain.
                        if cfg!(feature = "atrium-input-stage-legacy") {
                            crate::atrium::focus::route_event(event);
                        } else {
                            crate::atrium::focus::stage_event(&mut self.stage, event);
                        }
                        let _ = self.post_report(slot);
                        continue;
                    }
                    if self.pending.len() < PENDING_MAX {
                        self.pending.push(pe);
                        EVENTS.fetch_add(1, Ordering::Relaxed);
                    } else {
                        // I-14(b): bounded queue — a flood degrades the pointer,
                        // never governance. Dropped, counted, never unbounded.
                        DROPPED.fetch_add(1, Ordering::Relaxed);
                    }
                }
            } else {
                EP_ERRORS.fetch_add(1, Ordering::Relaxed);
                crate::serial_println!("hid-mouse: interrupt completion code={}", code);
            }
            let _ = self.post_report(slot);
        }
        #[cfg(feature = "atrium-input-routing")]
        crate::atrium::focus::flush_pointer_staged(&mut self.stage);
    }
}

/// Read the next decoded pointer event, draining the interrupt endpoint first.
/// Mirrors `hid_keyboard::try_read_byte`. Returns `None` when no mouse is bound
/// or no event is pending. The OT-52 compositor is the intended consumer.
#[allow(dead_code)] // input consumer entry point — OT-52 compositor + the gate scenario
pub fn try_read_pointer_event() -> Option<PointerEvent> {
    let mouse = primary()?;
    // CP-37: the device lock is only ever held IF=0.
    device_section(|| {
        let mut inner = mouse.device();
        inner.drain();
        if inner.pending.is_empty() {
            None
        } else {
            Some(inner.pending.remove(0))
        }
    })
}

/// Production Slice-4 pull-pump, called by InputFocus receive polling.
#[cfg(feature = "atrium-input-routing")]
pub fn pump_routed_events() {
    let Some(mouse) = primary() else {
        return;
    };
    // CP-37: the device lock is only ever held IF=0, whoever pumps.
    device_section(|| {
        let mut inner = mouse.device();
        inner.drain();
    });
}

#[cfg(any(feature = "neo-test", feature = "xhci-silicon"))]
pub fn debug_counters() -> (u64, u64, u64, u64) {
    (
        REPORTS.load(Ordering::Relaxed),
        EVENTS.load(Ordering::Relaxed),
        EP_ERRORS.load(Ordering::Relaxed),
        DROPPED.load(Ordering::Relaxed),
    )
}

/// True once a mouse has bound (the gate observable: enumeration → claim → ready).
#[cfg(any(feature = "neo-test", feature = "xhci-silicon"))]
pub fn is_bound() -> bool {
    primary().is_some()
}

/// Test-only: push a boot mouse report through the bound driver's REAL decode +
/// bounded-queue + counter path (the exact code `drain()` runs on a wire
/// completion), so the hermetic QEMU scenario proves decode → pointer-event
/// queue → consumer read → the I-14(b) drop cap deterministically, without
/// depending on QEMU's mouse-input routing. Returns false when no mouse is bound.
/// The wire/DMA path (real motion → fenced buffer → this decode) is proven by the
/// real usb-mouse binding here and definitively by the bare-metal plug-in close.
#[cfg(feature = "neo-test")]
pub fn debug_inject_report(bytes: &[u8]) -> bool {
    let Some(mouse) = primary() else {
        return false;
    };
    // CP-37: the device lock is only ever held IF=0.
    device_section(|| {
        let mut inner = mouse.device();
        REPORTS.fetch_add(1, Ordering::Relaxed);
        if let Some(pe) = inner.decoder.decode_report(bytes) {
            if inner.pending.len() < PENDING_MAX {
                inner.pending.push(pe);
                EVENTS.fetch_add(1, Ordering::Relaxed);
            } else {
                DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    true
}

/// Test-only: the bounded queue depth, so the scenario can drive the I-14(b)
/// drop cap deterministically (inject > this many events → assert DROPPED > 0).
#[cfg(feature = "neo-test")]
pub fn debug_pending_cap() -> usize {
    PENDING_MAX
}

fn fence_report_slot(buffer: &DmaBuffer, slot: usize) -> Result<FencedDmaAddr, &'static str> {
    let offset = slot
        .checked_mul(MOUSE_REPORT_SLOT_LEN)
        .ok_or("report slot offset overflow")?;
    if offset + MOUSE_REPORT_SLOT_LEN > buffer.len() {
        return Err("report slot out of buffer bounds");
    }
    dma_arena::fence(
        buffer.physical_addr().as_u64() + offset as u64,
        MOUSE_REPORT_SLOT_LEN as u64,
    )
    .map_err(|_| "report slot outside the fence")
}

fn read_report(buffer: &DmaBuffer, slot: usize) -> [u8; MOUSE_REPORT_SLOT_LEN] {
    let mut report = [0u8; MOUSE_REPORT_SLOT_LEN];
    let offset = slot * MOUSE_REPORT_SLOT_LEN;
    // SAFETY: `report_buf` is an exclusively owned arena page; `slot` is one of
    // REPORT_SLOTS posted by this driver, so the read is in-bounds.
    unsafe {
        let base = buffer.as_ptr().add(offset);
        for (idx, byte) in report.iter_mut().enumerate() {
            *byte = core::ptr::read_volatile(base.add(idx));
        }
    }
    report
}
