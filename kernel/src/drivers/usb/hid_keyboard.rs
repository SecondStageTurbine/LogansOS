// SPDX-License-Identifier: MPL-2.0
//! USB HID boot-protocol keyboard class driver for fd=0.
//!
//! I-14 is inherited from xHCI: the interrupt endpoint uses the existing
//! arena-fenced endpoint pipe, all submitted report buffers are
//! `FencedDmaAddr`s, and the ISR only routes completions into xHCI's bounded
//! per-endpoint queue. No new bus master or DMA path is introduced here.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::drivers::block::DmaBuffer;
use crate::drivers::dma_arena::{self, FencedDmaAddr};
use crate::sync::irqsafe_mutex::{IrqSafeGuard, IrqSafeMutex};

use super::hid_logic::{BOOT_REPORT_LEN, BootKeyboardDecoder};
#[cfg(feature = "atrium-input-routing")]
use super::hid_logic::{BootKeyEventDecoder, KeyEvent};
use super::usb_parse;
use super::xhci::{ClaimedDevice, EndpointPipe};
use super::{UsbClassDriver, UsbDevice};

const REPORT_SLOTS: usize = 16;
const PENDING_MAX: usize = 64;

static REPORTS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static EP_ERRORS: AtomicU64 = AtomicU64::new(0);

pub struct HidKeyboardDriver;

impl UsbClassDriver for HidKeyboardDriver {
    fn name(&self) -> &'static str {
        "hid-keyboard"
    }

    fn probe(&self, device: &UsbDevice) -> bool {
        usb_parse::find_hid_boot_keyboard(&device.configurations).is_some()
    }

    fn bind(&self, device: &Arc<UsbDevice>, claimed: ClaimedDevice) -> Result<(), ClaimedDevice> {
        match bind_inner(device, claimed) {
            Ok(()) => Ok(()),
            Err((claimed, reason)) => {
                crate::serial_println!(
                    "hid-keyboard: bind failed for vid={:04x} pid={:04x}: {}",
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
    super::register_class_driver(Arc::new(HidKeyboardDriver));
}

// clippy-bank: oversized ClaimedDevice errors are a CB-4 USB error-type refactor.
#[allow(clippy::result_large_err)]
fn bind_inner(
    device: &Arc<UsbDevice>,
    mut claimed: ClaimedDevice,
) -> Result<(), (ClaimedDevice, &'static str)> {
    let Some(topo) = usb_parse::find_hid_boot_keyboard(&device.configurations) else {
        return Err((claimed, "no boot keyboard topology"));
    };
    crate::serial_println!(
        "hid-keyboard: binding vid={:04x} pid={:04x} config={} interface={} int_ep=0x{:02x}",
        device.vendor_id,
        device.product_id,
        topo.config_value,
        topo.interface,
        topo.int_in.address
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
        report_table: [const { None }; super::xhci::RING_TABLE],
        decoder: BootKeyboardDecoder::new(),
        #[cfg(feature = "atrium-input-routing")]
        event_decoder: BootKeyEventDecoder::new(),
        #[cfg(feature = "atrium-input-routing")]
        stage: crate::atrium::focus::Stage::with_capacity(
            crate::atrium::focus::INPUT_STAGE_CAPACITY,
        ),
        #[cfg(feature = "atrium-input-routing")]
        sak: crate::atrium::sak_logic::SakFilter::new(),
        #[cfg(feature = "key-autorepeat")]
        repeat: crate::atrium::key_repeat_logic::KeyRepeat::new(),
        pending: Vec::with_capacity(PENDING_MAX),
    };
    for slot in 0..REPORT_SLOTS {
        if let Err(reason) = inner.post_report(slot) {
            crate::serial_println!("hid-keyboard: report post failed: {}", reason);
        }
    }

    let bound = Arc::new(Keyboard {
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
        "hid-keyboard: ready boot_protocol=true report_len={} report_slots={}",
        BOOT_REPORT_LEN,
        REPORT_SLOTS
    );
    crate::serial_println!(
        "hid_keyboard bound vid={:04x} pid={:04x}",
        device.vendor_id,
        device.product_id
    );
    crate::log::flush();
    Ok(())
}

/// CP-37/CP-56 (#55): `IrqSafeMutex`, like the device lock below, so an
/// acquisition is IF=0 by construction, not by each caller remembering.
static PRIMARY: IrqSafeMutex<Option<Arc<Keyboard>>> = IrqSafeMutex::new(None);

/// The bound device. CP-37: `PRIMARY` is a spinning lock taken by
/// the RECV pump with interrupts OFF, so every acquisition is IF=0 too —
/// an fd-0 reader preempted while holding it at IF=1 would deadlock a
/// pump spinning on the same CPU (Jarvis, #44 RETURN F2).
fn primary() -> Option<Arc<Keyboard>> {
    // The discriminator's control: main's acquisition shape (IF as the
    // caller has it); `hid_contention` must then FAIL.
    #[cfg(feature = "neo-test-hid-mixed-if-control")]
    return {
        let guard = PRIMARY.lock_as_caller();
        note_acquisition();
        guard.clone()
    };
    #[cfg(not(feature = "neo-test-hid-mixed-if-control"))]
    crate::arch::x86_64::without_interrupts(|| primary_slot().clone())
}

/// Every `PRIMARY` acquisition but the mixed-IF control's (the bind and
/// every `primary`), sampled by the CP-37 witness (Jarvis N1).
#[track_caller]
fn primary_slot() -> IrqSafeGuard<'static, Option<Arc<Keyboard>>> {
    #[cfg(feature = "neo-test-hid-contention")]
    note_acquisition();
    PRIMARY.lock()
}

/// Test-only CP-37 witness: acquisitions of `PRIMARY` or a HID device lock
/// made with interrupts ON (each must be zero; see `hid_contention`). Both
/// drivers sample it in their only acquisition paths (`primary_slot`,
/// `device`), so no acquisition goes unsampled (Jarvis N1), and sample it
/// *before* acquiring: `IrqSafeMutex::lock` masks IF itself, so a sample
/// taken after it could only ever see a `lock_as_caller` site. Sampled
/// first, it sees the caller's IF: a call site that stops masking fires it.
#[cfg(feature = "neo-test-hid-contention")]
static IF_ON_ACQUISITIONS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

#[cfg(feature = "neo-test-hid-contention")]
pub(crate) fn note_acquisition() {
    if x86_64::instructions::interrupts::are_enabled() {
        IF_ON_ACQUISITIONS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(feature = "neo-test-hid-contention")]
pub fn debug_if_on_acquisitions() -> u64 {
    IF_ON_ACQUISITIONS.load(core::sync::atomic::Ordering::Relaxed)
}

struct Keyboard {
    inner: IrqSafeMutex<Inner>,
}

impl Keyboard {
    /// The device lock: every acquisition goes through here and is sampled
    /// by the CP-37 witness (Jarvis N1).
    #[track_caller]
    fn device(&self) -> IrqSafeGuard<'_, Inner> {
        #[cfg(feature = "neo-test-hid-contention")]
        note_acquisition();
        self.inner.lock()
    }
}

struct Inner {
    _claimed: ClaimedDevice,
    int_in: EndpointPipe,
    report_buf: Arc<DmaBuffer>,
    report_table: [Option<usize>; super::xhci::RING_TABLE],
    decoder: BootKeyboardDecoder,
    #[cfg(feature = "atrium-input-routing")]
    event_decoder: BootKeyEventDecoder,
    /// S1-F1: decoded events not yet in `DELIVERY` (bounded).
    #[cfg(feature = "atrium-input-routing")]
    stage: crate::atrium::focus::Stage,
    /// SAK swallow-release state (this device's lock).
    #[cfg(feature = "atrium-input-routing")]
    sak: crate::atrium::sak_logic::SakFilter,
    /// Key auto-repeat (a USB keyboard never repeats by itself).
    #[cfg(feature = "key-autorepeat")]
    repeat: crate::atrium::key_repeat_logic::KeyRepeat,
    pending: Vec<u8>,
}

impl Inner {
    fn post_report(&mut self, slot: usize) -> Result<(), &'static str> {
        let addr = fence_report_slot(&self.report_buf, slot)?;
        let index = self.int_in.submit(addr, BOOT_REPORT_LEN as u32);
        self.report_table[index] = Some(slot);
        Ok(())
    }

    fn drain(&mut self) {
        super::xhci::poll_events();
        #[cfg(feature = "key-autorepeat")]
        let now_ns = crate::atrium::key_repeat::now_ns();
        for event in self.int_in.completions() {
            let Some(index) = self.int_in.index_of(event.parameter) else {
                continue;
            };
            let Some(slot) = self.report_table[index].take() else {
                continue;
            };
            let code = event.completion_code();
            if code == 1 || code == 13 {
                let report = read_report(&self.report_buf, slot);
                REPORTS.fetch_add(1, Ordering::Relaxed);
                #[cfg(feature = "atrium-input-routing")]
                if crate::atrium::input_routing_enabled() {
                    let mut events = [KeyEvent {
                        keycode: 0,
                        modifiers: 0,
                        pressed: false,
                    }; 12];
                    let n = self.event_decoder.decode_report(&report, &mut events);
                    // Byte 0 is the modifier byte: a change ends a repeat.
                    #[cfg(feature = "key-autorepeat")]
                    self.repeat.on_modifiers(report[0]);
                    for event in &events[..n] {
                        let locks = crate::drivers::key_locks::on_key(event.keycode, event.pressed);
                        let event = crate::atrium::input_logic::InputEventV1::key_locked(
                            event.keycode,
                            event.modifiers,
                            locks,
                            event.pressed,
                        );
                        // DI-1 SAK (`atrium-sak`): a chord goes to the
                        // lock-free command latch, never a queue.
                        if cfg!(feature = "atrium-sak") {
                            use crate::atrium::sak_logic::SakVerdict;
                            match self.sak.filter(event) {
                                SakVerdict::Chord(key) => {
                                    crate::atrium::focus::latch_sak(key);
                                    continue;
                                }
                                SakVerdict::Swallow => continue,
                                SakVerdict::Pass => {}
                            }
                        }
                        #[cfg(feature = "key-autorepeat")]
                        crate::atrium::key_repeat::on_event(&mut self.repeat, event, now_ns);
                        // S1-F1: stage, then flush below; the legacy switch
                        // restores the direct route (lossy under contention).
                        if cfg!(feature = "atrium-input-stage-legacy") {
                            crate::atrium::focus::route_event(event);
                        } else {
                            crate::atrium::focus::stage_event(&mut self.stage, event);
                        }
                    }
                    let _ = self.decoder.decode_report(&report, &mut []);
                    let _ = self.post_report(slot);
                    continue;
                }
                let mut decoded = [0u8; BOOT_REPORT_LEN];
                let n = self.decoder.decode_report(&report, &mut decoded);
                for &byte in &decoded[..n] {
                    if self.pending.len() < PENDING_MAX {
                        self.pending.push(byte);
                        BYTES.fetch_add(1, Ordering::Relaxed);
                    }
                }
            } else {
                EP_ERRORS.fetch_add(1, Ordering::Relaxed);
                crate::serial_println!("hid-keyboard: interrupt completion code={}", code);
            }
            let _ = self.post_report(slot);
        }
        // Key auto-repeat: the repeat due now (see the i8042 pump).
        #[cfg(feature = "key-autorepeat")]
        if crate::atrium::input_routing_enabled()
            && let Some(event) = crate::atrium::key_repeat::poll(&mut self.repeat, now_ns)
        {
            if cfg!(feature = "atrium-input-stage-legacy") {
                crate::atrium::focus::route_event(event);
            } else {
                crate::atrium::focus::stage_event(&mut self.stage, event);
            }
        }
        // Every pump retries what an earlier contended flush left staged.
        #[cfg(feature = "atrium-input-routing")]
        crate::atrium::focus::flush_staged(&mut self.stage);
    }
}

pub fn try_read_byte() -> Option<u8> {
    let keyboard = primary()?;
    // CP-37: fd-0 reads with interrupts ON, the RECV pump with them OFF; the
    // device lock (and `DELIVERY` under it) is only ever held IF=0.
    crate::arch::x86_64::without_interrupts(|| {
        let mut inner = keyboard.device();
        inner.drain();
        if inner.pending.is_empty() {
            None
        } else {
            Some(inner.pending.remove(0))
        }
    })
}

/// Production Slice-4 pull-pump, independent of the legacy fd-0 consumer.
#[cfg(feature = "atrium-input-routing")]
pub fn pump_routed_events() {
    let Some(keyboard) = primary() else {
        return;
    };
    // CP-37: the device lock is only ever held IF=0, whoever pumps.
    crate::arch::x86_64::without_interrupts(|| {
        let mut inner = keyboard.device();
        inner.drain();
    });
}

/// True once a keyboard has bound.
#[cfg(any(feature = "neo-test-hid-contention", feature = "xhci-silicon"))]
pub fn is_bound() -> bool {
    primary().is_some()
}

#[cfg(any(feature = "neo-test", feature = "xhci-silicon"))]
pub fn debug_counters() -> (u64, u64, u64) {
    (
        REPORTS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
        EP_ERRORS.load(Ordering::Relaxed),
    )
}

fn fence_report_slot(buffer: &DmaBuffer, slot: usize) -> Result<FencedDmaAddr, &'static str> {
    let offset = slot
        .checked_mul(BOOT_REPORT_LEN)
        .ok_or("report slot offset overflow")?;
    if offset + BOOT_REPORT_LEN > buffer.len() {
        return Err("report slot out of buffer bounds");
    }
    dma_arena::fence(
        buffer.physical_addr().as_u64() + offset as u64,
        BOOT_REPORT_LEN as u64,
    )
    .map_err(|_| "report slot outside the fence")
}

fn read_report(buffer: &DmaBuffer, slot: usize) -> [u8; BOOT_REPORT_LEN] {
    let mut report = [0u8; BOOT_REPORT_LEN];
    let offset = slot * BOOT_REPORT_LEN;
    // SAFETY: `report_buf` is an exclusively owned arena page; `slot` is one
    // of REPORT_SLOTS posted by this driver, so the 8-byte read is in-bounds.
    unsafe {
        let base = buffer.as_ptr().add(offset);
        for (idx, byte) in report.iter_mut().enumerate() {
            *byte = core::ptr::read_volatile(base.add(idx));
        }
    }
    report
}
