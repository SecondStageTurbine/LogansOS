// SPDX-License-Identifier: MPL-2.0
//! xHCI host controller driver (Phase 7.5 BUILD-A, ANSPEC §10.7, ADR-046).
//!
//! Bring-up order (xHCI 1.2 §4.2): PCIe class 0x0C0330 discovery → MMIO map
//! (cap/op/runtime/doorbell) → USBLEGSUP BIOS→OS handoff (F1 breadcrumbs) →
//! HCRST + CNR wait → MaxSlots/DCBAA/scratchpad (F2) → command ring + event
//! ring + interrupter 0 (F4/F5 counters + breadcrumbs) → RS → port scan
//! (F7 PORTSC timeline) → per-port: reset → Enable Slot → Address Device
//! (both 32/64-byte context layouts compiled, CSZ-selected at runtime, F3)
//! → GET_DESCRIPTOR → the `usb` device model.
//!
//! **Structural fencing (the ADR-045 carry, made construction):** every ring,
//! context, scratchpad, and data buffer is a `DmaBuffer::alloc_in_arena`
//! allocation, and every function that programs an address into a register,
//! TRB, or DCBAA entry takes a [`FencedDmaAddr`] — `PhysAddr` does not appear
//! in the programming API (grep-asserted by the gate).
//!
//! **I-14:** the ISR ([`xhci_msi_interrupt`]) touches only the `IrqSafeMutex`
//! event-ring state and atomics — no AI-governance lock is reachable from it
//! (grep + the lockdep: the process-side USB locks order ABOVE every AI
//! level, so AI-under-USB panics at the acquire). The ISR drains at most
//! [`ISR_EVENT_CAP`] events per pass (I-14(b)); the remainder waits for the
//! next pass or the poll path. Poll mode ([`set_poll_mode`]) is the F6
//! discriminator: works-polled + no-IRQ ⇒ routing, not logic.
//!
//! `unsafe` is confined to MMIO volatile access and DMA-buffer byte access,
//! each with the ADR-025-style justification on the block.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use spin::Lazy;

#[cfg(feature = "mt7601u")]
use super::operation_budget_logic::ChargeKind;
use x86_64::VirtAddr;

use crate::drivers::block::DmaBuffer;
use crate::drivers::dma_arena::{self, FencedDmaAddr};
use crate::drivers::pci::{self, Device as PciDeviceId};
use crate::sync::IrqSafeMutex;

#[path = "xhci_ring_logic.rs"]
mod ring;
use ring::{ConsumerRing, ProducerRing, Trb, TrbType, control_for};

// Zenbook USB input lane: silicon bring-up (census + choice of xHCI, D0,
// forced handoff + SMIs off, port power/settle, hub slot via Configure
// Endpoint, the USB summary) and the port service. Off: neither is compiled.
#[cfg(feature = "xhci-silicon")]
#[path = "xhci_silicon.rs"]
mod silicon;
#[cfg(feature = "neo-test-usb-xhci-select")]
pub use silicon::census_for_test;
#[cfg(feature = "xhci-silicon")]
pub use silicon::{keyboard_bound, log_usb_summary, mouse_bound, report_counts};
#[cfg(feature = "usb-hotplug")]
#[path = "xhci_hotplug.rs"]
pub mod hotplug;

pub const XHCI_MSI_VECTOR: u8 = 0xf3;

/// I-14(b): the ISR drains at most this many events per pass; a storm
/// degrades USB latency, never governance liveness.
pub const ISR_EVENT_CAP: usize = 16;

/// I-14(b) BUILD-B: per-endpoint completion queues are hard-capped — a
/// flooded endpoint DROPS (counted) rather than growing ISR-side memory.
/// The RX-flood gate drives the real routing path against this bound.
pub const EP_QUEUE_CAP: usize = 64;

const CLASS_SERIAL_BUS: u8 = 0x0C;
const SUBCLASS_USB: u8 = 0x03;
const PROGIF_XHCI: u8 = 0x30;

const RING_TRBS: usize = 64; // 16 bytes each → 1 KiB per ring
/// Ring size for driver-side ring-index → buffer tables (BUILD-B).
pub const RING_TABLE: usize = RING_TRBS;
/// Blind-run 1 sizing (the OptiPlex reported 34 scratchpads + 26 ports —
/// QEMU asks for 0 and 8): 512 KiB of arena margin.
const ARENA_PAGES: u64 = 128;
/// The FULL 64 KiB xHCI register file. Blind-run 1 page-faulted at
/// BAR+0x8000: Intel PCH silicon puts the extended-capability chain at
/// dword offset 0x2000 (byte 0x8000) — exactly the first byte past the
/// original 32 KiB window. QEMU keeps everything compact and never showed
/// it. The fault was localized in one run from cr2 alone (= HHDM + BAR +
/// 0x8000): the conclusiveness contract, working.
const XHCI_BAR_WINDOW: u64 = 0x1_0000;

const SPIN_BUDGET: u64 = 5_000_000;
/// Forge #4: hardware waits (CNR after HCRST, port reset ~50ms by spec,
/// warm reset) get a 10× budget — spin counts are not wall-clock, so the
/// ceiling is generous; every expiry still lands on a localizing breadcrumb.
const HW_WAIT_BUDGET: u64 = 50_000_000;

/// The active [`wait_event`] poll budget (control + command completions).
/// Defaults to `SPIN_BUDGET`. A connectivity driver can temporarily LOWER it
/// for a phase where a non-responding device must fail FAST instead of burning
/// the full (~minutes) budget on every wedged transfer — the MT7601U cold
/// firmware-upload wedge (7.5-W1): a wedged control transfer otherwise drains
/// all 5M iterations (~minutes) before failing, turning a retry into a stall.
/// Valid transfers complete in a handful of iterations, far under any sane
/// lowered budget, so the bound only changes how fast a DEAD device gives up.
static WAIT_BUDGET: AtomicU64 = AtomicU64::new(SPIN_BUDGET);

/// RAII: lower the `wait_event` budget for a phase, restore on drop (so an
/// early return / `?` in the guarded phase can never leak the lowered budget).
pub struct WaitBudgetGuard(u64);
impl WaitBudgetGuard {
    pub fn new(budget: u64) -> Self {
        WaitBudgetGuard(WAIT_BUDGET.swap(budget, Ordering::AcqRel))
    }
}
impl Drop for WaitBudgetGuard {
    fn drop(&mut self) {
        WAIT_BUDGET.store(self.0, Ordering::Release);
    }
}

#[cfg(feature = "mt7601u")]
mod mt7601u_operation_budget {
    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use super::super::operation_budget_logic::{
        BudgetCounters, ChargeKind, ChargeStep, charge_step,
    };
    use super::WAIT_BUDGET;

    /// Inactive sentinel for the operation-wide ledger. A real operation may have
    /// a zero remaining budget, so zero cannot double as "not armed".
    const OPERATION_BUDGET_INACTIVE: u64 = u64::MAX;
    static OPERATION_BUDGET_LIMIT: AtomicU64 = AtomicU64::new(0);
    static OPERATION_BUDGET_REMAINING: AtomicU64 = AtomicU64::new(OPERATION_BUDGET_INACTIVE);
    static OPERATION_BUDGET_EXHAUSTED: AtomicBool = AtomicBool::new(false);
    static OP_WAIT_CALLS: AtomicU64 = AtomicU64::new(0);
    static OP_WAIT_ITERATIONS: AtomicU64 = AtomicU64::new(0);
    static OP_VENDOR_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
    static OP_VENDOR_RETRY_GAPS: AtomicU64 = AtomicU64::new(0);
    static OP_VENDOR_REQUEST_EXHAUSTIONS: AtomicU64 = AtomicU64::new(0);
    static OP_OBSERVATIONS: AtomicU64 = AtomicU64::new(0);
    static OP_POLL_GAPS: AtomicU64 = AtomicU64::new(0);
    static OP_BULK_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
    static OP_BULK_POLLS: AtomicU64 = AtomicU64::new(0);
    static OP_LATE_EVENTS: AtomicU64 = AtomicU64::new(0);
    static OP_NO_EVENT_EXHAUSTIONS: AtomicU64 = AtomicU64::new(0);
    static OP_DESYNC_EXHAUSTIONS: AtomicU64 = AtomicU64::new(0);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct OperationBudgetSnapshot {
        pub active: bool,
        pub limit: u64,
        pub remaining: u64,
        pub exhausted: bool,
        pub counters: BudgetCounters,
    }

    pub fn operation_budget_snapshot() -> OperationBudgetSnapshot {
        let remaining = OPERATION_BUDGET_REMAINING.load(Ordering::Acquire);
        OperationBudgetSnapshot {
            active: remaining != OPERATION_BUDGET_INACTIVE,
            limit: OPERATION_BUDGET_LIMIT.load(Ordering::Acquire),
            remaining: if remaining == OPERATION_BUDGET_INACTIVE {
                0
            } else {
                remaining
            },
            exhausted: OPERATION_BUDGET_EXHAUSTED.load(Ordering::Acquire),
            counters: BudgetCounters {
                wait_calls: OP_WAIT_CALLS.load(Ordering::Acquire),
                wait_iterations: OP_WAIT_ITERATIONS.load(Ordering::Acquire),
                vendor_attempts: OP_VENDOR_ATTEMPTS.load(Ordering::Acquire),
                vendor_retry_gaps: OP_VENDOR_RETRY_GAPS.load(Ordering::Acquire),
                vendor_request_exhaustions: OP_VENDOR_REQUEST_EXHAUSTIONS.load(Ordering::Acquire),
                observations: OP_OBSERVATIONS.load(Ordering::Acquire),
                poll_gaps: OP_POLL_GAPS.load(Ordering::Acquire),
                bulk_attempts: OP_BULK_ATTEMPTS.load(Ordering::Acquire),
                bulk_polls: OP_BULK_POLLS.load(Ordering::Acquire),
                late_events: OP_LATE_EVENTS.load(Ordering::Acquire),
                no_event_exhaustions: OP_NO_EVENT_EXHAUSTIONS.load(Ordering::Acquire),
                desync_exhaustions: OP_DESYNC_EXHAUSTIONS.load(Ordering::Acquire),
            },
        }
    }

    fn restore_operation_budget(snapshot: OperationBudgetSnapshot) {
        // Publish inactive while the prior counter bank is restored. The guarded
        // path is process-context serialized; the sentinel also prevents a stray
        // observer from charging a half-restored ledger.
        OPERATION_BUDGET_REMAINING.store(OPERATION_BUDGET_INACTIVE, Ordering::Release);
        OPERATION_BUDGET_LIMIT.store(snapshot.limit, Ordering::Release);
        OPERATION_BUDGET_EXHAUSTED.store(snapshot.exhausted, Ordering::Release);
        OP_WAIT_CALLS.store(snapshot.counters.wait_calls, Ordering::Release);
        OP_WAIT_ITERATIONS.store(snapshot.counters.wait_iterations, Ordering::Release);
        OP_VENDOR_ATTEMPTS.store(snapshot.counters.vendor_attempts, Ordering::Release);
        OP_VENDOR_RETRY_GAPS.store(snapshot.counters.vendor_retry_gaps, Ordering::Release);
        OP_VENDOR_REQUEST_EXHAUSTIONS.store(
            snapshot.counters.vendor_request_exhaustions,
            Ordering::Release,
        );
        OP_OBSERVATIONS.store(snapshot.counters.observations, Ordering::Release);
        OP_POLL_GAPS.store(snapshot.counters.poll_gaps, Ordering::Release);
        OP_BULK_ATTEMPTS.store(snapshot.counters.bulk_attempts, Ordering::Release);
        OP_BULK_POLLS.store(snapshot.counters.bulk_polls, Ordering::Release);
        OP_LATE_EVENTS.store(snapshot.counters.late_events, Ordering::Release);
        OP_NO_EVENT_EXHAUSTIONS.store(snapshot.counters.no_event_exhaustions, Ordering::Release);
        OP_DESYNC_EXHAUSTIONS.store(snapshot.counters.desync_exhaustions, Ordering::Release);
        OPERATION_BUDGET_REMAINING.store(
            if snapshot.active {
                snapshot.remaining
            } else {
                OPERATION_BUDGET_INACTIVE
            },
            Ordering::Release,
        );
    }

    /// One non-replenishing budget across every nested layer of a composed USB
    /// control-plane operation. No lock, DMA surface, or IRQ path is introduced:
    /// the process-side path charges lock-free atomics, and Drop restores both the
    /// prior per-wait ceiling and any prior operation ledger.
    pub struct OperationBudgetGuard {
        prior_wait_budget: u64,
        prior_operation: OperationBudgetSnapshot,
    }

    impl OperationBudgetGuard {
        pub fn new(limit: u64) -> Self {
            let limit = limit.max(1);
            let prior_wait_budget = WAIT_BUDGET.swap(limit, Ordering::AcqRel);
            let prior_operation = operation_budget_snapshot();
            restore_operation_budget(OperationBudgetSnapshot {
                active: true,
                limit,
                remaining: limit,
                exhausted: false,
                counters: BudgetCounters::default(),
            });
            Self {
                prior_wait_budget,
                prior_operation,
            }
        }

        pub fn snapshot(&self) -> OperationBudgetSnapshot {
            operation_budget_snapshot()
        }
    }

    impl Drop for OperationBudgetGuard {
        fn drop(&mut self) {
            restore_operation_budget(self.prior_operation);
            WAIT_BUDGET.store(self.prior_wait_budget, Ordering::Release);
        }
    }

    /// Charge one layer of the active composed operation. Outside a guard this is a
    /// no-op, preserving every pre-existing xHCI caller.
    pub fn charge_operation(kind: ChargeKind) -> bool {
        loop {
            let current = OPERATION_BUDGET_REMAINING.load(Ordering::Acquire);
            if current == OPERATION_BUDGET_INACTIVE {
                return true;
            }
            let ChargeStep::Charged { remaining } = charge_step(current) else {
                OPERATION_BUDGET_EXHAUSTED.store(true, Ordering::Release);
                return false;
            };
            if OPERATION_BUDGET_REMAINING
                .compare_exchange_weak(current, remaining, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                let counter = match kind {
                    ChargeKind::WaitCall => &OP_WAIT_CALLS,
                    ChargeKind::WaitIteration => &OP_WAIT_ITERATIONS,
                    ChargeKind::VendorAttempt => &OP_VENDOR_ATTEMPTS,
                    ChargeKind::VendorRetryGap => &OP_VENDOR_RETRY_GAPS,
                    ChargeKind::Observation => &OP_OBSERVATIONS,
                    ChargeKind::PollGap => &OP_POLL_GAPS,
                    ChargeKind::BulkAttempt => &OP_BULK_ATTEMPTS,
                    ChargeKind::BulkPoll => &OP_BULK_POLLS,
                };
                counter.fetch_add(1, Ordering::Relaxed);
                return true;
            }
        }
    }

    pub fn note_vendor_request_exhaustion() {
        if OPERATION_BUDGET_REMAINING.load(Ordering::Acquire) != OPERATION_BUDGET_INACTIVE {
            OP_VENDOR_REQUEST_EXHAUSTIONS.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn note_operation_late_event() {
        if OPERATION_BUDGET_REMAINING.load(Ordering::Acquire) != OPERATION_BUDGET_INACTIVE {
            OP_LATE_EVENTS.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn note_operation_no_event_exhaustion() {
        if OPERATION_BUDGET_REMAINING.load(Ordering::Acquire) != OPERATION_BUDGET_INACTIVE {
            OP_NO_EVENT_EXHAUSTIONS.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn note_operation_desync_exhaustion() {
        if OPERATION_BUDGET_REMAINING.load(Ordering::Acquire) != OPERATION_BUDGET_INACTIVE {
            OP_DESYNC_EXHAUSTIONS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(feature = "mt7601u")]
pub use mt7601u_operation_budget::{
    OperationBudgetGuard, OperationBudgetSnapshot, charge_operation,
    note_vendor_request_exhaustion, operation_budget_snapshot,
};
#[cfg(feature = "mt7601u")]
use mt7601u_operation_budget::{
    note_operation_desync_exhaustion, note_operation_late_event, note_operation_no_event_exhaustion,
};

// --- Discriminator counters (F4/F5/F6 + the storm cap; Rule 5) ---------------

static CMDS_SUBMITTED: AtomicU64 = AtomicU64::new(0);
static CMDS_COMPLETED: AtomicU64 = AtomicU64::new(0);
static IRQS: AtomicU64 = AtomicU64::new(0);
static EVENTS_DRAINED: AtomicU64 = AtomicU64::new(0);
static ISR_PASSES: AtomicU64 = AtomicU64::new(0);
static ISR_MAX_BURST: AtomicU64 = AtomicU64::new(0);
static POLL_MODE: AtomicBool = AtomicBool::new(false);
/// Unconsumed event classes (port-change, host-controller) — counted, never
/// queued (Forge #6: the PENDING bound under real-port chatter).
static OTHER_EVENTS: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)] // F4/F6 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn debug_counters() -> (u64, u64, u64, u64, u64, u64) {
    (
        CMDS_SUBMITTED.load(Ordering::Relaxed),
        CMDS_COMPLETED.load(Ordering::Relaxed),
        IRQS.load(Ordering::Relaxed),
        EVENTS_DRAINED.load(Ordering::Relaxed),
        ISR_PASSES.load(Ordering::Relaxed),
        ISR_MAX_BURST.load(Ordering::Relaxed),
    )
}

/// Whether the controller completed bring-up and is running (the Stage-B
/// probe's conclusiveness gate: dead-controller and zero-devices must be
/// distinguishable in the sink).
#[allow(dead_code)] // Stage-B probe consumer
pub fn is_running() -> bool {
    CONTROLLER.lock().is_some()
}

/// F6: poll mode — the conclusive IRQ-routing discriminator (works-polled
/// with zero IRQs ⇒ routing, not driver logic).
#[allow(dead_code)] // F6 discriminator entry — gate/bare-metal consumer
pub fn set_poll_mode(enabled: bool) {
    POLL_MODE.store(enabled, Ordering::Release);
}

// --- MMIO (the ADR-025 idiom: tiny volatile helpers, justified per call) ------

#[inline]
fn mmio_read32(base: VirtAddr, offset: u64) -> u32 {
    // SAFETY: `base` is a BAR mapping established by `pci::map_bar_region`
    // for this controller; `offset` is a register offset inside the mapped
    // window. Volatile because the device mutates these.
    unsafe { core::ptr::read_volatile((base.as_u64() + offset) as *const u32) }
}

#[inline]
fn mmio_write32(base: VirtAddr, offset: u64, value: u32) {
    // SAFETY: as `mmio_read32`; register writes are the programming model.
    unsafe { core::ptr::write_volatile((base.as_u64() + offset) as *mut u32, value) }
}

#[inline]
fn mmio_write64(base: VirtAddr, offset: u64, value: u64) {
    // SAFETY: as `mmio_read32`; 64-bit registers (CRCR/DCBAAP/ERSTBA/ERDP)
    // are naturally aligned in the xHCI register file.
    unsafe { core::ptr::write_volatile((base.as_u64() + offset) as *mut u64, value) }
}

// --- Register offsets (xHCI 1.2 §5) -------------------------------------------

// Capability registers (from the BAR base).
const CAP_CAPLENGTH: u64 = 0x00; // byte 0; HCIVERSION in [31:16]
const CAP_HCSPARAMS1: u64 = 0x04;
const CAP_HCSPARAMS2: u64 = 0x08;
const CAP_HCCPARAMS1: u64 = 0x10;
const CAP_DBOFF: u64 = 0x14;
const CAP_RTSOFF: u64 = 0x18;

// Operational registers (from op base = BAR + CAPLENGTH).
const OP_USBCMD: u64 = 0x00;
const OP_USBSTS: u64 = 0x04;
const OP_CRCR: u64 = 0x18;
const OP_DCBAAP: u64 = 0x30;
const OP_CONFIG: u64 = 0x38;
const OP_PORTSC_BASE: u64 = 0x400;
const PORTSC_STRIDE: u64 = 0x10;

const USBCMD_RS: u32 = 1 << 0;
const USBCMD_HCRST: u32 = 1 << 1;
const USBCMD_INTE: u32 = 1 << 2;
const USBSTS_HCH: u32 = 1 << 0;
const USBSTS_HSE: u32 = 1 << 2;
const USBSTS_EINT: u32 = 1 << 3;
const USBSTS_CNR: u32 = 1 << 11;

const PORTSC_CCS: u32 = 1 << 0;
const PORTSC_PED: u32 = 1 << 1;
const PORTSC_PR: u32 = 1 << 4;
const PORTSC_PP: u32 = 1 << 9;
const PORTSC_CSC: u32 = 1 << 17;
const PORTSC_WRC: u32 = 1 << 19;
const PORTSC_PRC: u32 = 1 << 21;
/// Warm Port Reset (USB3 — xHCI §4.19.5.1): the recovery for an SS link
/// stuck outside U0 (Polling/Compliance/Inactive), which real silicon
/// produces and QEMU does not (Forge #3).
const PORTSC_WPR: u32 = 1 << 31;
/// PORTSC write-1-to-clear change bits — masked out on read-modify-write so
/// a plain status write never acks a change unintentionally (incl. WRC).
const PORTSC_RW1C: u32 = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21) | (1 << 22);

// Runtime registers (from rt base = BAR + RTSOFF), interrupter 0.
const RT_IMAN0: u64 = 0x20;
const RT_IMOD0: u64 = 0x24;
const RT_ERSTSZ0: u64 = 0x28;
const RT_ERSTBA0: u64 = 0x30;
const RT_ERDP0: u64 = 0x38;

const IMAN_IP: u32 = 1 << 0;
const IMAN_IE: u32 = 1 << 1;
const ERDP_EHB: u64 = 1 << 3;

// --- The controller -----------------------------------------------------------

struct EventRingState {
    consumer: ConsumerRing,
    ring: Arc<DmaBuffer>,
    ring_addr: FencedDmaAddr,
}

struct Controller {
    op: VirtAddr,
    rt: VirtAddr,
    db: VirtAddr,
    csz64: bool,
    max_ports: u32,
    // Command ring (process-side only).
    cmd_ring: Arc<DmaBuffer>,
    cmd_addr: FencedDmaAddr,
    cmd_state: ProducerRing,
    // Event ring state is ISR-shared.
    // (held in the static EVENT below — split so the ISR never touches this)
    // Keep-alives: DCBAA, ERST, scratchpads (programmed once, owned forever).
    _dcbaa: Arc<DmaBuffer>,
    _erst: Arc<DmaBuffer>,
    _scratchpads: Vec<Arc<DmaBuffer>>,
}

/// Process-side controller state. Ordered ABOVE all AI locks (I-14(a)).
static CONTROLLER: Lazy<crate::sync::SwitchMutex<Option<Controller>>> =
    Lazy::new(|| crate::sync::SwitchMutex::new_ordered(None, crate::sync::lock_order::USB_XHCI));

/// Addressed hub slots must outlive downstream children whose route strings
/// name them. The controller's slot limit bounds this collection.
static HUBS: Lazy<crate::sync::SwitchMutex<Vec<ClaimedDevice>>> = Lazy::new(|| {
    crate::sync::SwitchMutex::new_ordered(Vec::new(), crate::sync::lock_order::USB_XHCI)
});

/// ISR-shared event-ring state (mixed process/interrupt context — the AHCI
/// `IrqSafeMutex` precedent; the ISR touches ONLY this and atomics).
static EVENT: Lazy<IrqSafeMutex<Option<EventRingState>>> = Lazy::new(|| IrqSafeMutex::new(None));

/// Drained-but-unprocessed events (ISR producer, process consumer).
static PENDING: Lazy<IrqSafeMutex<Vec<Trb>>> = Lazy::new(|| IrqSafeMutex::new(Vec::new()));

/// One async-endpoint completion route: Transfer Events for (slot, DCI) are
/// delivered to the bounded queue; the owning [`EndpointPipe`] drains it in
/// process context. ISR-shared (the ISR only ever pushes, under the cap —
/// the queue is pre-reserved at registration so the ISR never allocates).
struct EpRoute {
    slot: u8,
    dci: u8,
    queue: Arc<IrqSafeMutex<Vec<Trb>>>,
    drops: Arc<AtomicU64>,
}

/// Async-endpoint routing table (BUILD-B). Registered at Configure
/// Endpoint; unrouted Transfer Events fall through to PENDING exactly as
/// BUILD-A (the EP0 control-transfer flows).
static EP_ROUTES: Lazy<IrqSafeMutex<Vec<EpRoute>>> = Lazy::new(|| IrqSafeMutex::new(Vec::new()));

/// Deliver one Transfer Event to its registered route, if any. Bounded:
/// a full queue counts a drop and the event is consumed (the producer ring
/// re-posts; losing a completion under flood is the I-14(b) trade — USB
/// degrades, memory does not grow).
fn route_transfer_event(slot: u8, dci_value: u8, trb: Trb) -> bool {
    let routes = EP_ROUTES.lock();
    for route in routes.iter() {
        if route.slot == slot && route.dci == dci_value {
            let mut queue = route.queue.lock();
            if queue.len() < EP_QUEUE_CAP {
                queue.push(trb);
            } else {
                route.drops.fetch_add(1, Ordering::Relaxed);
            }
            return true;
        }
    }
    false
}

fn fence_buffer(buffer: &DmaBuffer) -> Result<FencedDmaAddr, &'static str> {
    dma_arena::fence(buffer.physical_addr().as_u64(), buffer.len() as u64)
        .map_err(|_| "buffer outside the fence")
}

/// Read ONLY the control dword of TRB `index` — the ownership-publishing
/// field (the cycle bit). Forge CRITICAL #5: ownership must be decided
/// BEFORE the data dwords are read, or a TRB the device is mid-DMA-ing can
/// yield a torn status/parameter whose cycle bit already reads owned —
/// QEMU writes event TRBs atomically and can never show this; real silicon
/// DMAs them across the fabric and can.
fn read_trb_control(ringbuf: &DmaBuffer, index: usize) -> u32 {
    // SAFETY: arena allocation exclusively owned by the controller; index
    // bounded by RING_TRBS by every caller. Volatile: the device writes it.
    unsafe { core::ptr::read_volatile(ringbuf.as_ptr().add(index * 16 + 12) as *const u32) }
}

/// Read the data dwords of an ALREADY-OWNED TRB — called only after
/// [`read_trb_control`] confirmed ownership, behind an Acquire fence so the
/// data reads post-date the cycle-bit observation.
fn read_trb_data(ringbuf: &DmaBuffer, index: usize, control: u32) -> Trb {
    core::sync::atomic::fence(Ordering::Acquire);
    // SAFETY: as `read_trb_control`; the ownership gate above means the
    // device finished publishing this TRB before we read its data.
    unsafe {
        let base = ringbuf.as_ptr().add(index * 16);
        Trb {
            parameter: core::ptr::read_volatile(base as *const u64),
            status: core::ptr::read_volatile(base.add(8) as *const u32),
            control,
        }
    }
}

/// Write TRB `index` into a ring buffer — parameter/status first, the
/// control dword (with the cycle bit) LAST, behind a fence, so the device
/// never sees a half-written TRB as owned (xHCI §4.9.2).
fn write_trb(ringbuf: &DmaBuffer, index: usize, trb: Trb) {
    // SAFETY: as `read_trb`; the producer exclusively owns slots whose cycle
    // bit it has not yet handed to the consumer.
    unsafe {
        let base = ringbuf.as_mut_ptr().add(index * 16);
        core::ptr::write_volatile(base as *mut u64, trb.parameter);
        core::ptr::write_volatile(base.add(8) as *mut u32, trb.status);
        core::sync::atomic::fence(Ordering::SeqCst);
        core::ptr::write_volatile(base.add(12) as *mut u32, trb.control);
        core::sync::atomic::fence(Ordering::SeqCst);
    }
}

fn spin_until(check: impl FnMut() -> bool) -> bool {
    spin_until_budget(check, SPIN_BUDGET)
}

fn spin_until_budget(mut check: impl FnMut() -> bool, budget: u64) -> bool {
    let mut budget = budget;
    while !check() {
        budget -= 1;
        if budget == 0 {
            return false;
        }
        core::hint::spin_loop();
    }
    true
}

// --- Discovery + bring-up ------------------------------------------------------

pub fn init() {
    #[cfg(not(feature = "xhci-silicon"))]
    let Some(dev) = find_xhci() else {
        crate::serial_println!("xhci: no controller (class 0x0C0330) — usb inactive");
        return;
    };
    #[cfg(feature = "xhci-silicon")]
    let Some(dev) = silicon::census_and_choose() else {
        crate::serial_println!("xhci: no controller (class 0x0C0330) — usb inactive");
        return;
    };
    // The fence precedes the feature (ADR-045): the arena must exist before
    // any USB DMA structure does. A previously initialized arena is fine.
    let _ = dma_arena::init(ARENA_PAGES);
    if dma_arena::bounds().is_none() {
        crate::serial_println!("xhci: dma arena unavailable — refusing unfenced bring-up");
        return;
    }
    match bring_up(dev) {
        Ok(()) => {}
        Err(reason) => {
            crate::serial_println!("xhci: bring-up failed: {}", reason);
            crate::log::flush();
        }
    }
}

#[cfg_attr(feature = "xhci-silicon", allow(dead_code))]
fn find_xhci() -> Option<PciDeviceId> {
    pci::find_by_class(CLASS_SERIAL_BUS, SUBCLASS_USB, PROGIF_XHCI)
}

fn bring_up(dev: PciDeviceId) -> Result<(), &'static str> {
    let (bus, device, function) = dev.bdf();
    let base = pci::map_bar_region(dev, 0, 0, XHCI_BAR_WINDOW as u32).ok_or("bar0 map failed")?;
    pci::enable_memory_and_bus_mastering(dev);

    let caplength = (mmio_read32(base, CAP_CAPLENGTH) & 0xFF) as u64;
    let hciversion = mmio_read32(base, CAP_CAPLENGTH) >> 16;
    let hcs1 = mmio_read32(base, CAP_HCSPARAMS1);
    let hcs2 = mmio_read32(base, CAP_HCSPARAMS2);
    let hcc1 = mmio_read32(base, CAP_HCCPARAMS1);
    let max_slots = hcs1 & 0xFF;
    let max_ports = hcs1 >> 24;
    let csz64 = hcc1 & (1 << 2) != 0;
    // F2: Max Scratchpad Buffers = Hi[25:21] << 5 | Lo[31:27] (xHCI §5.3.4).
    let scratchpads = (((hcs2 >> 21) & 0x1F) << 5) | (hcs2 >> 27);
    crate::serial_println!(
        "xhci: found {:02x}:{:02x}.{} version={:x}.{:02x} slots={} ports={} csz={} scratchpads={}",
        bus,
        device,
        function,
        hciversion >> 8,
        hciversion & 0xFF,
        max_slots,
        max_ports,
        if csz64 { 64 } else { 32 },
        scratchpads,
    );

    // F1: BIOS→OS handoff via USBLEGSUP, breadcrumbed before/after.
    #[cfg(not(feature = "xhci-silicon"))]
    legacy_handoff(base, hcc1);
    #[cfg(feature = "xhci-silicon")]
    silicon::note_version(hciversion as u16);
    #[cfg(feature = "xhci-silicon")]
    silicon::legacy_handoff(base, hcc1);

    let op = base + caplength;
    let rt = base + (mmio_read32(base, CAP_RTSOFF) & !0x1F) as u64;
    let db = base + (mmio_read32(base, CAP_DBOFF) & !0x3) as u64;

    // Halt (if running), reset, wait CNR clear (xHCI §4.2).
    let cmd = mmio_read32(op, OP_USBCMD);
    if cmd & USBCMD_RS != 0 {
        mmio_write32(op, OP_USBCMD, cmd & !USBCMD_RS);
        if !spin_until(|| mmio_read32(op, OP_USBSTS) & USBSTS_HCH != 0) {
            dump_halt_state(op, "halt-timeout");
            return Err("controller would not halt");
        }
    }
    mmio_write32(op, OP_USBCMD, USBCMD_HCRST);
    if !spin_until_budget(
        || {
            mmio_read32(op, OP_USBCMD) & USBCMD_HCRST == 0
                && mmio_read32(op, OP_USBSTS) & USBSTS_CNR == 0
        },
        HW_WAIT_BUDGET,
    ) {
        dump_halt_state(op, "reset-timeout");
        return Err("reset/CNR timeout");
    }

    // DCBAA + scratchpads (F2): allocate per HCSPARAMS2, breadcrumb both.
    let dcbaa = DmaBuffer::alloc_in_arena(4096).map_err(|_| "dcbaa alloc")?;
    let dcbaa_addr = fence_buffer(&dcbaa)?;
    let mut scratch_keepalive = Vec::new();
    if scratchpads > 0 {
        let array = DmaBuffer::alloc_in_arena(4096).map_err(|_| "scratch array alloc")?;
        let array_addr = fence_buffer(&array)?;
        for i in 0..scratchpads as usize {
            let page = DmaBuffer::alloc_in_arena(4096).map_err(|_| "scratch page alloc")?;
            let page_addr = fence_buffer(&page)?;
            write_dma_u64(&array, i, page_addr);
            scratch_keepalive.push(page);
        }
        write_dma_u64(&dcbaa, 0, array_addr);
        scratch_keepalive.push(array);
    }
    crate::serial_println!(
        "xhci: scratchpads requested={} allocated={}",
        scratchpads,
        scratch_keepalive
            .len()
            .saturating_sub(usize::from(scratchpads > 0)),
    );
    mmio_write64(op, OP_DCBAAP, dcbaa_addr.phys());
    mmio_write32(op, OP_CONFIG, max_slots.min(16));

    // Command ring (F4): Link TRB at the tail, RCS=1.
    let cmd_ring = DmaBuffer::alloc_in_arena(4096).map_err(|_| "cmd ring alloc")?;
    let cmd_addr = fence_buffer(&cmd_ring)?;
    let cmd_state = ProducerRing::new(RING_TRBS);
    arm_link_trb(&cmd_ring, cmd_addr, cmd_state.cycle);
    mmio_write64(op, OP_CRCR, cmd_addr.phys() | 1 /* RCS */);

    // Event ring + ERST + interrupter 0 (F5).
    let ev_ring = DmaBuffer::alloc_in_arena(4096).map_err(|_| "event ring alloc")?;
    let ev_addr = fence_buffer(&ev_ring)?;
    let erst = DmaBuffer::alloc_in_arena(4096).map_err(|_| "erst alloc")?;
    let erst_addr = fence_buffer(&erst)?;
    // ERST entry 0: {base, size, reserved} (xHCI §6.5).
    write_dma_u64(&erst, 0, ev_addr);
    // SAFETY: the ERST buffer is an exclusively owned arena allocation;
    // entry 0's size dword lives at byte offset 8.
    unsafe {
        core::ptr::write_volatile(erst.as_mut_ptr().add(8) as *mut u32, RING_TRBS as u32);
        core::ptr::write_volatile(erst.as_mut_ptr().add(12) as *mut u32, 0);
    }
    mmio_write32(rt, RT_ERSTSZ0, 1);
    mmio_write64(rt, RT_ERSTBA0, erst_addr.phys());
    mmio_write64(rt, RT_ERDP0, ev_addr.phys());
    mmio_write32(rt, RT_IMOD0, 0);
    mmio_write32(rt, RT_IMAN0, IMAN_IE);

    *EVENT.lock() = Some(EventRingState {
        consumer: ConsumerRing::new(RING_TRBS),
        ring: ev_ring,
        ring_addr: ev_addr,
    });

    // MSI → our fixed vector (F6's counted side; poll mode is the fallback).
    let msi = pci::configure_msi_fixed_vector(dev, XHCI_MSI_VECTOR);
    crate::serial_println!("xhci: msi_configured={} vector={:#x}", msi, XHCI_MSI_VECTOR);
    #[cfg(feature = "xhci-silicon")]
    silicon::note_msi(msi);

    // Run + interrupts on.
    mmio_write32(op, OP_USBCMD, USBCMD_RS | USBCMD_INTE);
    if !spin_until(|| mmio_read32(op, OP_USBSTS) & USBSTS_HCH == 0) {
        dump_halt_state(op, "run-timeout");
        return Err("controller did not run");
    }

    OP_BASE.store(op.as_u64(), Ordering::Release);
    RT_BASE.store(rt.as_u64(), Ordering::Release);
    *CONTROLLER.lock() = Some(Controller {
        op,
        rt,
        db,
        csz64,
        max_ports,
        cmd_ring,
        cmd_addr,
        cmd_state,
        _dcbaa: dcbaa,
        _erst: erst,
        _scratchpads: scratch_keepalive,
    });

    #[cfg(feature = "xhci-silicon")]
    silicon::power_and_settle_ports(op, max_ports);
    crate::serial_println!("xhci: running — scanning {} ports", max_ports);
    crate::log::flush();
    scan_ports();
    Ok(())
}

/// Program a 64-bit DMA pointer into entry `index` of a table buffer — the
/// fenced currency only (DCBAA entries, ERST bases, scratchpad arrays).
fn write_dma_u64(table: &DmaBuffer, index: usize, addr: FencedDmaAddr) {
    // SAFETY: the table is an exclusively owned arena allocation; entries
    // are 8 bytes; every caller passes an in-bounds index.
    unsafe {
        core::ptr::write_volatile(table.as_mut_ptr().add(index * 8) as *mut u64, addr.phys());
    }
}

/// (Re-)arm the Link TRB at the ring tail pointing back to the ring head,
/// with Toggle Cycle and the given cycle bit (xHCI §4.9.2.2 — F4).
fn arm_link_trb(ringbuf: &DmaBuffer, ring_addr: FencedDmaAddr, cycle: bool) {
    let link = Trb {
        parameter: ring_addr.phys(),
        status: 0,
        control: control_for(TrbType::Link, cycle, ring::TRB_TOGGLE_CYCLE),
    };
    write_trb(ringbuf, RING_TRBS - 1, link);
}

/// F1: the USBLEGSUP handoff with before/after breadcrumbs and a distinct
/// "bios-owned" verdict on timeout.
#[cfg_attr(feature = "xhci-silicon", allow(dead_code))]
fn legacy_handoff(base: VirtAddr, hcc1: u32) {
    let mut ecp = ((hcc1 >> 16) as u64) << 2;
    while ecp != 0 {
        // The walk must never leave the mapped window (blind-run 1: the
        // chain itself STARTS at 0x8000 on Intel PCH; a corrupt next-ptr
        // must breadcrumb, not fault).
        if ecp >= XHCI_BAR_WINDOW {
            crate::serial_println!(
                "xhci: xecp walk left the {:#x} window at {:#x} — stopping (no handoff)",
                XHCI_BAR_WINDOW,
                ecp
            );
            return;
        }
        let cap = mmio_read32(base, ecp);
        if cap & 0xFF == 0x01 {
            crate::serial_println!("xhci: usblegsup before={:#010x}", cap);
            mmio_write32(base, ecp, cap | (1 << 24)); // HC OS Owned
            let released = spin_until(|| {
                let now = mmio_read32(base, ecp);
                now & (1 << 16) == 0 && now & (1 << 24) != 0
            });
            let after = mmio_read32(base, ecp);
            crate::serial_println!(
                "xhci: usblegsup after={:#010x} verdict={}",
                after,
                if released { "os-owned" } else { "bios-owned" }
            );
            return;
        }
        let next = ((cap >> 8) & 0xFF) as u64;
        if next == 0 {
            break;
        }
        ecp += next << 2;
    }
    crate::serial_println!("xhci: no usblegsup capability (no legacy bios) — os-owned");
}

/// F2 companion: HCH/HSE dumped whenever a wait times out.
fn dump_halt_state(op: VirtAddr, what: &str) {
    let sts = mmio_read32(op, OP_USBSTS);
    crate::serial_println!(
        "xhci: {} usbsts={:#010x} hch={} hse={}",
        what,
        sts,
        sts & USBSTS_HCH != 0,
        sts & USBSTS_HSE != 0
    );
    crate::log::flush();
}

// --- Event handling (ISR + poll share one drain; F5/F6) ------------------------

/// The MSI handler body: ack, then drain a BOUNDED burst (I-14(b)). Touches
/// ONLY the IrqSafeMutex event state + atomics — no AI-governance lock is
/// reachable from here (I-14(a), grep + lockdep).
pub fn handle_interrupt() {
    IRQS.fetch_add(1, Ordering::Relaxed);
    drain_events(ISR_EVENT_CAP);
}

/// Drain up to `cap` events into PENDING; ack EINT + IMAN.IP + ERDP (with
/// EHB) every pass, with the F5 breadcrumb. Shared by the ISR and the poll
/// path; the IrqSafeMutex serializes them.
fn drain_events(cap: usize) -> usize {
    ISR_PASSES.fetch_add(1, Ordering::Relaxed);
    let mut drained = 0usize;
    let mut event_guard = EVENT.lock();
    let Some(event) = event_guard.as_mut() else {
        return 0;
    };
    // Snapshot op/rt bases from atomics published at bring-up.
    let rt = VirtAddr::new(RT_BASE.load(Ordering::Acquire));
    let op = VirtAddr::new(OP_BASE.load(Ordering::Acquire));
    if rt.as_u64() == 0 {
        return 0;
    }
    // Ack the interrupt sources first (xHCI §4.17.5): USBSTS.EINT then
    // IMAN.IP (both RW1C).
    mmio_write32(op, OP_USBSTS, USBSTS_EINT);
    let iman = mmio_read32(rt, RT_IMAN0);
    if iman & IMAN_IP != 0 {
        mmio_write32(rt, RT_IMAN0, iman | IMAN_IP);
    }
    while drained < cap {
        let control = read_trb_control(&event.ring, event.consumer.dequeue);
        if !event.consumer.owns(control) {
            break;
        }
        let trb = read_trb_data(&event.ring, event.consumer.dequeue, control);
        // Forge #6: only events somebody consumes are queued. Port-change /
        // host-controller events are counted and dropped — a chattering
        // real port must not grow PENDING unboundedly (the storm class).
        // BUILD-B: Transfer Events for registered async endpoints route to
        // their bounded per-EP queues; everything unrouted is BUILD-A.
        let ty = trb.trb_type();
        if ty == TrbType::TransferEvent as u32 {
            let slot = trb.slot_id() as u8;
            let dci_value = trb.ep_id() as u8;
            if !route_transfer_event(slot, dci_value, trb) {
                PENDING.lock().push(trb);
            }
        } else if ty == TrbType::CmdCompletionEvent as u32 {
            PENDING.lock().push(trb);
        } else {
            OTHER_EVENTS.fetch_add(1, Ordering::Relaxed);
        }
        event.consumer.advance();
        EVENTS_DRAINED.fetch_add(1, Ordering::Relaxed);
        drained += 1;
    }
    // ERDP ← current dequeue, EHB cleared (xHCI §5.5.2.3.3).
    let erdp = event
        .ring_addr
        .phys()
        .wrapping_add((event.consumer.dequeue * 16) as u64);
    mmio_write64(rt, RT_ERDP0, erdp | ERDP_EHB);
    let prior = ISR_MAX_BURST.load(Ordering::Relaxed);
    if (drained as u64) > prior {
        ISR_MAX_BURST.store(drained as u64, Ordering::Relaxed);
    }
    drained
}

/// Published op/rt bases so the ISR never takes the process-side controller
/// lock (I-14(a)).
static OP_BASE: AtomicU64 = AtomicU64::new(0);
static RT_BASE: AtomicU64 = AtomicU64::new(0);

/// Pull the next pending event of `ty`, polling (and draining, in poll mode
/// or as the storm backstop) under a budget. Process context only.
fn wait_event(ty: TrbType) -> Option<Trb> {
    #[cfg(feature = "mt7601u")]
    if !charge_operation(ChargeKind::WaitCall) {
        note_operation_no_event_exhaustion();
        return None;
    }
    let mut budget = WAIT_BUDGET.load(Ordering::Acquire);
    loop {
        {
            let mut pending = PENDING.lock();
            if let Some(pos) = pending.iter().position(|t| t.trb_type() == ty as u32) {
                return Some(pending.remove(pos));
            }
        }
        #[cfg(feature = "mt7601u")]
        if !charge_operation(ChargeKind::WaitIteration) {
            note_operation_no_event_exhaustion();
            return None;
        }
        // Poll path (F6): in poll mode the ISR may never fire; the same
        // bounded drain keeps everything flowing.
        drain_events(ISR_EVENT_CAP);
        budget -= 1;
        if budget == 0 {
            #[cfg(feature = "mt7601u")]
            note_operation_no_event_exhaustion();
            return None;
        }
        core::hint::spin_loop();
    }
}

/// The I-14(b) simulated-storm seam: one bounded ISR-equivalent pass, callable
/// in a tight loop by the storm scenario. Returns the burst size (≤ cap).
#[allow(dead_code)] // I-14(b) storm-gate seam — feature-consumed
pub fn debug_simulate_isr_pass() -> usize {
    drain_events(ISR_EVENT_CAP)
}

/// One bounded poll-mode drain (the F6 path, production): class drivers
/// call this from their service path so streaming completions flow even
/// when no interrupt fires — QEMU's qemu-xhci delivers no MSI here
/// (`msi_configured=false`), and BUILD-A only ever drained inside
/// `wait_event` loops, so a steady-state consumer that merely read its
/// routed queues would starve (BUILD-B gate run 1: pump census frames=0
/// with a fully bound device — this fn is that lesson). When MSI does fire
/// (the OptiPlex), the IrqSafeMutex serializes the two paths and this is a
/// cheap no-op.
pub fn poll_events() {
    drain_events(ISR_EVENT_CAP);
}

/// The I-14(b) RX-flood seam (BUILD-B): drive `count` synthetic Transfer
/// Events for (slot, dci) through the REAL routing path; returns the
/// route's (queue_len, drops) afterwards — the gate asserts queue_len ≤
/// [`EP_QUEUE_CAP`] and drops == the overflow.
#[allow(dead_code)] // I-14(b) flood-gate seam — feature-consumed
#[cfg(feature = "neo-test")]
pub fn debug_flood_route(slot: u8, dci_value: u8, count: usize) -> (usize, u64) {
    let control = ring::control_for(
        TrbType::TransferEvent,
        true,
        ((slot as u32) << 24) | ((dci_value as u32) << 16),
    );
    for _ in 0..count {
        let _ = route_transfer_event(
            slot,
            dci_value,
            Trb {
                parameter: 0,
                status: 1 << 24, // completion code 1 (Success), residue 0
                control,
            },
        );
    }
    let routes = EP_ROUTES.lock();
    for route in routes.iter() {
        if route.slot == slot && route.dci == dci_value {
            return (
                route.queue.lock().len(),
                route.drops.load(Ordering::Relaxed),
            );
        }
    }
    (0, 0)
}

// --- Commands -------------------------------------------------------------------

/// Submit one command TRB and wait for its completion event (F4 counters).
fn submit_command(trb: Trb) -> Option<Trb> {
    let cmd_phys;
    {
        let mut guard = CONTROLLER.lock();
        let ctrl = guard.as_mut()?;
        let step = ctrl.cmd_state.enqueue_step();
        let mut armed = trb;
        armed.control =
            (armed.control & !ring::TRB_CYCLE) | if step.cycle { ring::TRB_CYCLE } else { 0 };
        write_trb(&ctrl.cmd_ring, step.index, armed);
        if let Some(link_cycle) = step.rearm_link_cycle {
            arm_link_trb(&ctrl.cmd_ring, ctrl.cmd_addr, link_cycle);
        }
        cmd_phys = ctrl.cmd_addr.phys() + (step.index * 16) as u64;
        CMDS_SUBMITTED.fetch_add(1, Ordering::Relaxed);
        // Doorbell 0, target 0 = the command ring (xHCI §5.6).
        mmio_write32(ctrl.db, 0, 0);
    }
    let mut budget: u32 = 64;
    loop {
        let event = wait_event(TrbType::CmdCompletionEvent)?;
        if event.parameter == cmd_phys {
            CMDS_COMPLETED.fetch_add(1, Ordering::Relaxed);
            return Some(event);
        }
        // A completion for an older command — keep waiting (bounded).
        budget -= 1;
        if budget == 0 {
            return None;
        }
    }
}

/// CB-2: outcome of an endpoint-recovery attempt — the EP state observed before
/// recovery, and whether each command stage completed (completion code 1).
/// `recovered()` is the gate the mt7601u scan reports as `recovered_after_wedge`.
#[cfg(feature = "mt7601u")]
#[derive(Copy, Clone, Debug, Default)]
pub struct EndpointRecovery {
    /// EP State (xHCI §6.2.3, `ring::EP_STATE_*`) read before recovery — the
    /// Halted-vs-stuck discriminator captured at wedge time.
    pub state_before: u8,
    /// The Reset Endpoint (Halted) or Stop Endpoint (otherwise) command
    /// completed successfully.
    pub reset_or_stop_ok: bool,
    /// The Set TR Dequeue Pointer command completed successfully.
    pub set_dequeue_ok: bool,
}

#[cfg(feature = "mt7601u")]
impl EndpointRecovery {
    /// Host-side recovery completed: the EP is Stopped at a clean dequeue and
    /// the next `submit()` resumes it Running.
    pub fn recovered(&self) -> bool {
        self.reset_or_stop_ok && self.set_dequeue_ok
    }
}

#[cfg(feature = "mt7601u")]
/// CB-2: Reset Endpoint Command (xHCI §4.6.8) — Halted → Stopped. Returns the
/// completion code (1 = success; 19 = Context State Error ⇒ the EP was not
/// Halted), or `None` if no completion arrived within `submit_command`'s budget.
fn cmd_reset_endpoint(slot_id: u8, dci: u8) -> Option<u32> {
    submit_command(ring::reset_endpoint_trb(slot_id, dci)).map(|e| e.completion_code())
}

#[cfg(feature = "mt7601u")]
/// CB-2: Stop Endpoint Command (xHCI §4.6.9) — any state → Stopped.
fn cmd_stop_endpoint(slot_id: u8, dci: u8) -> Option<u32> {
    submit_command(ring::stop_endpoint_trb(slot_id, dci, false)).map(|e| e.completion_code())
}

#[cfg(feature = "mt7601u")]
/// CB-2: Set TR Dequeue Pointer Command (xHCI §4.6.10) — repoint the transfer
/// ring's dequeue + Dequeue Cycle State after a Reset/Stop.
fn cmd_set_tr_dequeue(slot_id: u8, dci: u8, deq_ptr: u64, dcs: bool) -> Option<u32> {
    submit_command(ring::set_tr_dequeue_trb(slot_id, dci, deq_ptr, dcs))
        .map(|e| e.completion_code())
}

// --- Ports + enumeration ---------------------------------------------------------

fn portsc(op: VirtAddr, port: u32) -> u32 {
    mmio_read32(op, OP_PORTSC_BASE + (port as u64 - 1) * PORTSC_STRIDE)
}

fn portsc_write(op: VirtAddr, port: u32, value: u32) {
    mmio_write32(
        op,
        OP_PORTSC_BASE + (port as u64 - 1) * PORTSC_STRIDE,
        value,
    )
}

fn scan_ports() {
    let (op, max_ports) = {
        let guard = CONTROLLER.lock();
        let Some(ctrl) = guard.as_ref() else {
            return;
        };
        (ctrl.op, ctrl.max_ports)
    };
    // F7: the PORTSC timeline — sample every port several times before
    // acting, so a dead-port / no-power / reset-timing failure on real
    // hardware is distinguishable from "no device".
    for sample in 0..3u32 {
        for port in 1..=max_ports {
            let sc = portsc(op, port);
            if sc & (PORTSC_CCS | PORTSC_CSC) != 0 || sample == 0 {
                // PLS [8:5] is THE SS-link discriminator (Forge #3.1): it
                // distinguishes "Enabled-in-U0, addressable" from
                // "Polling/Compliance/Inactive, needs a warm reset".
                crate::serial_println!(
                    "xhci: portsc[{}] sample={} raw={:#010x} ccs={} ped={} pp={} speed={} pls={}",
                    port,
                    sample,
                    sc,
                    sc & PORTSC_CCS != 0,
                    sc & PORTSC_PED != 0,
                    sc & PORTSC_PP != 0,
                    (sc >> 10) & 0xF,
                    (sc >> 5) & 0xF,
                );
            }
        }
        for _ in 0..200_000u32 {
            core::hint::spin_loop();
        }
    }
    crate::log::flush();
    #[cfg(not(feature = "usb-hotplug"))]
    for port in 1..=max_ports {
        if portsc(op, port) & PORTSC_CCS != 0
            && let Err(reason) = enumerate_port(op, port)
        {
            crate::serial_println!("xhci: port {} enumeration failed: {}", port, reason);
            crate::log::flush();
        }
    }
    // The port service starts from the boot scan's results: an enumerated
    // port is attached, a failed one has used one of its attempts.
    #[cfg(feature = "usb-hotplug")]
    for port in 1..=max_ports {
        if portsc(op, port) & PORTSC_CCS == 0 {
            continue;
        }
        let result = enumerate_port(op, port);
        hotplug::note_root_result(port, result.is_ok());
        if let Err(reason) = result {
            crate::serial_println!("xhci: port {} enumeration failed: {}", port, reason);
            crate::log::flush();
        }
    }
}

fn enumerate_port(op: VirtAddr, port: u32) -> Result<(), &'static str> {
    // Reset the port; USB2 ports need PR. A USB3 port may already be
    // enabled — but ONLY a link in U0 (PLS == 0) is addressable: an SS link
    // stuck in Polling/Compliance/Inactive needs a WARM reset, which real
    // silicon produces and QEMU never shows (Forge #3.2).
    let sc = portsc(op, port);
    let pls = (sc >> 5) & 0xF;
    if sc & PORTSC_PED != 0 && pls != 0 {
        crate::serial_println!(
            "xhci: port {} enabled but pls={} (not U0) — warm reset",
            port,
            pls
        );
        portsc_write(
            op,
            port,
            (sc & !PORTSC_RW1C & !PORTSC_PED) | PORTSC_WPR | PORTSC_PP,
        );
        if !spin_until_budget(
            || portsc(op, port) & (PORTSC_WRC | PORTSC_PRC) != 0,
            HW_WAIT_BUDGET,
        ) {
            return Err("warm reset timeout (F7: check pls in the PORTSC timeline)");
        }
        let now = portsc(op, port);
        portsc_write(
            op,
            port,
            (now & !PORTSC_RW1C & !PORTSC_PED) | PORTSC_WRC | PORTSC_PRC | PORTSC_CSC,
        );
    }
    let sc = portsc(op, port);
    if sc & PORTSC_PED == 0 {
        portsc_write(
            op,
            port,
            (sc & !PORTSC_RW1C & !PORTSC_PED) | PORTSC_PR | PORTSC_PP,
        );
        if !spin_until_budget(|| portsc(op, port) & PORTSC_PRC != 0, HW_WAIT_BUDGET) {
            return Err("port reset timeout (F7: check the PORTSC timeline)");
        }
        // Ack PRC + CSC.
        let now = portsc(op, port);
        portsc_write(
            op,
            port,
            (now & !PORTSC_RW1C & !PORTSC_PED) | PORTSC_PRC | PORTSC_CSC,
        );
    }
    let sc = portsc(op, port);
    if sc & PORTSC_PED == 0 {
        return Err("port not enabled after reset");
    }
    let speed = super::UsbSpeed::from_portsc((sc >> 10) & 0xF);

    // Enable Slot (xHCI §4.3.2).
    let enable = Trb {
        parameter: 0,
        status: 0,
        control: control_for(TrbType::EnableSlotCmd, false, 0),
    };
    let completion = submit_command(enable).ok_or("enable-slot: no completion (F4)")?;
    if completion.completion_code() != 1 {
        return Err("enable-slot refused");
    }
    let slot_id = completion.slot_id() as u8;
    if slot_id == 0 {
        return Err("enable-slot returned slot 0");
    }

    address_and_describe(
        op,
        AddressRoute {
            root_port: port,
            route_string: 0,
            parent_hub_slot: 0,
            parent_port: 0,
            parent_speed: speed,
        },
        slot_id,
        speed,
    )
}

#[derive(Copy, Clone)]
struct AddressRoute {
    root_port: u32,
    route_string: u32,
    parent_hub_slot: u8,
    parent_port: u8,
    parent_speed: super::UsbSpeed,
}

/// Input/device context offsets honoring CSZ (F3): both layouts compiled,
/// the stride selected at runtime from HCCPARAMS1.
fn ctx_stride(csz64: bool) -> usize {
    if csz64 { 64 } else { 32 }
}

fn address_and_describe(
    _op: VirtAddr,
    route: AddressRoute,
    slot_id: u8,
    speed: super::UsbSpeed,
) -> Result<(), &'static str> {
    let (csz64, db, dcbaa) = {
        let guard = CONTROLLER.lock();
        let ctrl = guard.as_ref().ok_or("controller gone")?;
        (ctrl.csz64, ctrl.db, ctrl._dcbaa.clone())
    };
    let stride = ctx_stride(csz64);
    crate::serial_println!(
        "xhci: addressing slot={} port={} speed={} csz={} (F3 layout breadcrumb)",
        slot_id,
        route.root_port,
        speed.name(),
        stride,
    );

    // Output device context → DCBAA[slot].
    let out_ctx = DmaBuffer::alloc_in_arena(4096).map_err(|_| "out ctx alloc")?;
    let out_addr = fence_buffer(&out_ctx)?;
    write_dma_u64(&dcbaa, slot_id as usize, out_addr);

    // EP0 transfer ring.
    let ep0_ring = DmaBuffer::alloc_in_arena(4096).map_err(|_| "ep0 ring alloc")?;
    let ep0_addr = fence_buffer(&ep0_ring)?;
    let mut ep0_state = ProducerRing::new(RING_TRBS);
    arm_link_trb(&ep0_ring, ep0_addr, ep0_state.cycle);

    // Input context: control(0) + slot(1) + EP0(2), CSZ-strided.
    let input = DmaBuffer::alloc_in_arena(4096).map_err(|_| "input ctx alloc")?;
    let input_addr = fence_buffer(&input)?;
    let mps = speed.default_max_packet();
    // SAFETY: the input context is an exclusively owned arena allocation;
    // every offset below is `< 3 * stride ≤ 192 < 4096`. Volatile: the
    // controller reads it on Address Device.
    unsafe {
        let p = input.as_mut_ptr();
        core::ptr::write_bytes(p, 0, 3 * stride);
        // Input Control Context: Add Context flags A0|A1 (slot + EP0).
        core::ptr::write_volatile(p.add(4) as *mut u32, 0b11);
        // Slot Context: route string, speed, 1 context entry, root hub port.
        // FS/LS children of a HS hub also name its transaction translator.
        let slot = p.add(stride);
        let speed_bits = match speed {
            super::UsbSpeed::Full => 1u32,
            super::UsbSpeed::Low => 2,
            super::UsbSpeed::High => 3,
            super::UsbSpeed::Super => 4,
            super::UsbSpeed::Unknown(raw) => raw as u32,
        };
        core::ptr::write_volatile(
            slot as *mut u32,
            (route.route_string & 0x000f_ffff) | (1 << 27) | (speed_bits << 20),
        );
        core::ptr::write_volatile(slot.add(4) as *mut u32, route.root_port << 16);
        if route.parent_hub_slot != 0
            && route.parent_speed == super::UsbSpeed::High
            && matches!(speed, super::UsbSpeed::Low | super::UsbSpeed::Full)
        {
            core::ptr::write_volatile(
                slot.add(8) as *mut u32,
                u32::from(route.parent_hub_slot) | (u32::from(route.parent_port) << 8),
            );
        }
        // EP0 Context (offset = 2*stride): type=Control(4), CErr=3, MPS;
        // dequeue = EP0 ring | DCS=1; average TRB length 8.
        let ep0 = p.add(2 * stride);
        core::ptr::write_volatile(
            ep0.add(4) as *mut u32,
            ((mps as u32) << 16) | (4 << 3) | (3 << 1),
        );
        core::ptr::write_volatile(ep0.add(8) as *mut u64, ep0_addr.phys() | 1);
        core::ptr::write_volatile(ep0.add(16) as *mut u32, 8);
    }

    // Address Device (BSR=0).
    let address = Trb {
        parameter: input_addr.phys(),
        status: 0,
        control: control_for(TrbType::AddressDeviceCmd, false, (slot_id as u32) << 24),
    };
    let completion = submit_command(address).ok_or("address-device: no completion (F4)")?;
    if completion.completion_code() != 1 {
        crate::serial_println!(
            "xhci: address-device completion code={} (F3: csz breadcrumb above)",
            completion.completion_code()
        );
        return Err("address-device refused");
    }

    // GET_DESCRIPTOR(device): 8 bytes first (the MPS probe), then 18.
    let data = DmaBuffer::alloc_in_arena(4096).map_err(|_| "xfer buf alloc")?;
    let data_addr = fence_buffer(&data)?;
    control_in(
        db,
        slot_id,
        &ep0_ring,
        ep0_addr,
        &mut ep0_state,
        0x80,
        6,
        0x0100,
        0,
        8,
        data_addr,
    )?;
    let mps0 = read_dma_u8(&data, 7);
    // For SuperSpeed, bMaxPacketSize0 is an EXPONENT (2^n — USB 3 §9.6.1);
    // for the others it is the byte count directly.
    let actual_mps0: u32 = match speed {
        super::UsbSpeed::Super => 1u32 << mps0.min(15),
        _ => u32::from(mps0),
    };
    if actual_mps0 != u32::from(speed.default_max_packet()) {
        // Blind-run 2 (OptiPlex port 14): an FS device with MPS0=64. The
        // 8-byte probe fits any packet; an 18-byte read crosses the packet
        // boundary and fails mid-stage unless EP0's context MPS is updated
        // first — the standard Evaluate Context dance (xHCI §4.6.7).
        crate::serial_println!(
            "xhci: bMaxPacketSize0={} (effective {}) differs from the speed default {} — evaluate context",
            mps0,
            actual_mps0,
            speed.default_max_packet(),
        );
        // Input context: control(0) + slot(1) + EP0(2); ONLY A1 (EP0) set.
        // SAFETY: exclusively owned arena allocation; offsets < 3*stride.
        unsafe {
            let p = input.as_mut_ptr();
            core::ptr::write_bytes(p, 0, 3 * stride);
            core::ptr::write_volatile(p.add(4) as *mut u32, 0b10); // A1 only
            let ep0 = p.add(2 * stride);
            core::ptr::write_volatile(
                ep0.add(4) as *mut u32,
                ((actual_mps0.min(1024)) << 16) | (4 << 3) | (3 << 1),
            );
            core::ptr::write_volatile(ep0.add(16) as *mut u32, 8);
        }
        let evaluate = Trb {
            parameter: input_addr.phys(),
            status: 0,
            control: control_for(TrbType::EvaluateContextCmd, false, (slot_id as u32) << 24),
        };
        let completion = submit_command(evaluate).ok_or("evaluate-context: no completion (F4)")?;
        if completion.completion_code() != 1 {
            crate::serial_println!(
                "xhci: evaluate-context completion code={}",
                completion.completion_code()
            );
            return Err("evaluate-context refused");
        }
    }
    control_in(
        db,
        slot_id,
        &ep0_ring,
        ep0_addr,
        &mut ep0_state,
        0x80,
        6,
        0x0100,
        0,
        18,
        data_addr,
    )?;
    let vendor_id = read_dma_u16(&data, 8);
    let product_id = read_dma_u16(&data, 10);
    let class = read_dma_u8(&data, 4);
    let subclass = read_dma_u8(&data, 5);
    let protocol = read_dma_u8(&data, 6);
    let num_configs = read_dma_u8(&data, 17).clamp(1, 4);

    if class == 0x09 {
        return enumerate_hub(
            _op,
            route,
            ClaimedDevice {
                slot_id,
                port: route.root_port,
                speed,
                csz64,
                db,
                dcbaa: dcbaa.clone(),
                out_ctx,
                input,
                input_addr,
                ep0_ring,
                ep0_addr,
                ep0_state,
                data,
                data_addr,
            },
            vendor_id,
            product_id,
        );
    }

    // ALL configurations (BUILD-B: an ECM function may live in any of them
    // — dual-config RNDIS+ECM devices put it in the second): header (9)
    // then the full set (wTotalLength), parsed by the pure walker.
    let mut configurations = Vec::new();
    for idx in 0..num_configs {
        control_in(
            db,
            slot_id,
            &ep0_ring,
            ep0_addr,
            &mut ep0_state,
            0x80,
            6,
            0x0200 | idx as u16,
            0,
            9,
            data_addr,
        )?;
        let total_len = read_dma_u16(&data, 2).clamp(9, 1024);
        control_in(
            db,
            slot_id,
            &ep0_ring,
            ep0_addr,
            &mut ep0_state,
            0x80,
            6,
            0x0200 | idx as u16,
            0,
            total_len,
            data_addr,
        )?;
        let raw = read_dma_bytes(&data, total_len as usize);
        configurations.push(super::usb_parse::parse_configuration_set(&raw));
    }

    // The port service's record of a finished enumeration (every descriptor
    // read; only the offer, which cannot fail, remains).
    #[cfg(feature = "usb-hotplug")]
    hotplug::note_addressed(route.root_port, route.route_string);
    let device = super::UsbDevice {
        slot_id,
        port: route.root_port as u8,
        speed,
        vendor_id,
        product_id,
        class,
        subclass,
        protocol,
        configurations,
    };
    // The offer (BUILD-B): a claiming driver keeps the slot, rings, and
    // contexts alive for its lifetime; an unclaimed device takes the
    // BUILD-A teardown exactly (DCBAA entry cleared BEFORE the backing
    // context drops — the controller never holds a dangling pointer).
    let claimed = ClaimedDevice {
        slot_id,
        port: route.root_port,
        speed,
        csz64,
        db,
        dcbaa: dcbaa.clone(),
        out_ctx,
        input,
        input_addr,
        ep0_ring,
        ep0_addr,
        ep0_state,
        data,
        data_addr,
    };
    if let Some(unclaimed) = super::offer_device(device, claimed) {
        unclaimed.teardown();
    }
    Ok(())
}

fn enumerate_hub(
    op: VirtAddr,
    route: AddressRoute,
    mut hub: ClaimedDevice,
    vendor_id: u16,
    product_id: u16,
) -> Result<(), &'static str> {
    let mut config = [0u8; 9];
    hub.control_read(0x80, 6, 0x0200, 0, 9, &mut config)?;
    let config_value = config[5];
    hub.control_no_data(0x00, 9, u16::from(config_value), 0)?;

    let mut descriptor = [0u8; 9];
    hub.control_read(0xA0, 6, 0x2900, 0, 9, &mut descriptor)?;
    let ports = descriptor[2].min(15);
    #[cfg(not(feature = "xhci-silicon"))]
    mark_slot_as_hub(&mut hub, route, ports)?;
    #[cfg(feature = "xhci-silicon")]
    silicon::mark_slot_as_hub(
        &mut hub,
        route,
        ports,
        u16::from_le_bytes([descriptor[3], descriptor[4]]),
    )?;
    crate::serial_println!(
        "xhci: hub slot={} vid={:04x} pid={:04x} config={} downstream_ports={}",
        hub.slot_id,
        vendor_id,
        product_id,
        config_value,
        ports
    );

    // USB 2 hub class: power every downstream port, then inspect/reset only
    // connected ports. Both loops are bounded; a bad hub degrades USB only.
    for child_port in 1..=ports {
        if let Err(reason) = hub.control_no_data(0x23, 3, 8, u16::from(child_port)) {
            crate::serial_println!(
                "xhci: hub slot={} port={} power failed: {}",
                hub.slot_id,
                child_port,
                reason
            );
        }
    }
    for _ in 0..400_000u32 {
        core::hint::spin_loop();
    }
    #[cfg(feature = "xhci-silicon")]
    silicon::hub_power_good_wait(&descriptor);

    for child_port in 1..=ports {
        let mut status = [0u8; 4];
        if hub
            .control_read(0xA3, 0, 0, u16::from(child_port), 4, &mut status)
            .is_err()
        {
            continue;
        }
        let initial = u16::from_le_bytes([status[0], status[1]]);
        if initial & 1 == 0 {
            continue;
        }
        crate::serial_println!(
            "xhci: hub slot={} port={} connected status={:#06x}; resetting",
            hub.slot_id,
            child_port,
            initial
        );
        hub.control_no_data(0x23, 3, 4, u16::from(child_port))?;
        let mut ready = None;
        for _ in 0..128 {
            for _ in 0..50_000u32 {
                core::hint::spin_loop();
            }
            hub.control_read(0xA3, 0, 0, u16::from(child_port), 4, &mut status)?;
            let current = u16::from_le_bytes([status[0], status[1]]);
            if current & 1 != 0 && current & (1 << 4) == 0 && current & (1 << 1) != 0 {
                ready = Some(current);
                break;
            }
        }
        let Some(current) = ready else {
            crate::serial_println!(
                "xhci: hub slot={} port={} reset did not enable child",
                hub.slot_id,
                child_port
            );
            continue;
        };
        let child_speed = if current & (1 << 9) != 0 {
            super::UsbSpeed::Low
        } else if current & (1 << 10) != 0 {
            super::UsbSpeed::High
        } else {
            super::UsbSpeed::Full
        };
        let enable = Trb {
            parameter: 0,
            status: 0,
            control: control_for(TrbType::EnableSlotCmd, false, 0),
        };
        let Some(completion) = submit_command(enable) else {
            crate::serial_println!("xhci: hub child enable-slot: no completion");
            continue;
        };
        if completion.completion_code() != 1 || completion.slot_id() == 0 {
            crate::serial_println!(
                "xhci: hub child enable-slot refused code={}",
                completion.completion_code()
            );
            continue;
        }
        let child_route = append_route(route.route_string, child_port);
        let child = AddressRoute {
            root_port: route.root_port,
            route_string: child_route,
            parent_hub_slot: hub.slot_id,
            parent_port: child_port,
            parent_speed: hub.speed,
        };
        if let Err(reason) =
            address_and_describe(op, child, completion.slot_id() as u8, child_speed)
        {
            crate::serial_println!(
                "xhci: hub slot={} port={} child enumeration failed: {}",
                hub.slot_id,
                child_port,
                reason
            );
        }
    }
    crate::log::flush();
    #[cfg(feature = "usb-hotplug")]
    hotplug::note_hub(hub.slot_id, route, ports);
    #[cfg(feature = "usb-hotplug")]
    hotplug::note_addressed(route.root_port, route.route_string);
    HUBS.lock().push(hub);
    Ok(())
}

fn append_route(route: u32, port: u8) -> u32 {
    for tier in 0..5 {
        let shift = tier * 4;
        if route & (0xF << shift) == 0 {
            return route | (u32::from(port.min(15)) << shift);
        }
    }
    route
}

fn mark_slot_as_hub(
    hub: &mut ClaimedDevice,
    route: AddressRoute,
    ports: u8,
) -> Result<(), &'static str> {
    let stride = ctx_stride(hub.csz64);
    unsafe {
        let p = hub.input.as_mut_ptr();
        core::ptr::write_bytes(p, 0, 2 * stride);
        core::ptr::write_volatile(p.add(4) as *mut u32, 1); // A0: slot context
        let slot = p.add(stride);
        core::ptr::write_volatile(
            slot as *mut u32,
            (route.route_string & 0x000f_ffff)
                | (slot_speed_bits(hub.speed) << 20)
                | (1 << 26)
                | (1 << 27),
        );
        core::ptr::write_volatile(
            slot.add(4) as *mut u32,
            (route.root_port << 16) | (u32::from(ports) << 24),
        );
    }
    let evaluate = Trb {
        parameter: hub.input_addr.phys(),
        status: 0,
        control: control_for(
            TrbType::EvaluateContextCmd,
            false,
            (hub.slot_id as u32) << 24,
        ),
    };
    let completion = submit_command(evaluate).ok_or("hub evaluate-context: no completion")?;
    if completion.completion_code() == 1 {
        Ok(())
    } else {
        Err("hub evaluate-context refused")
    }
}

fn write_dma_u64_zero(table: &DmaBuffer, index: usize) {
    // SAFETY: as `write_dma_u64` — clearing the entry before the backing
    // context drops, so the controller never holds a dangling pointer.
    unsafe {
        core::ptr::write_volatile(table.as_mut_ptr().add(index * 8) as *mut u64, 0);
    }
}

fn read_dma_u8(buf: &DmaBuffer, offset: usize) -> u8 {
    // SAFETY: exclusively owned arena buffer; offsets bounded by callers.
    unsafe { core::ptr::read_volatile(buf.as_ptr().add(offset)) }
}

fn read_dma_u16(buf: &DmaBuffer, offset: usize) -> u16 {
    u16::from_le_bytes([read_dma_u8(buf, offset), read_dma_u8(buf, offset + 1)])
}

#[cfg(feature = "mt7601u")]
fn read_dma_u32(buf: &DmaBuffer, offset: usize) -> u32 {
    u32::from_le_bytes([
        read_dma_u8(buf, offset),
        read_dma_u8(buf, offset + 1),
        read_dma_u8(buf, offset + 2),
        read_dma_u8(buf, offset + 3),
    ])
}

/// One control IN transfer on EP0: Setup (IDT) → Data IN → Status OUT, then
/// the doorbell and the transfer-event wait (xHCI §4.11.2.2).
#[allow(clippy::too_many_arguments)] // the EP0 programming tuple — local to enumeration
fn control_in(
    db: VirtAddr,
    slot_id: u8,
    ep0_ring: &DmaBuffer,
    ep0_addr: FencedDmaAddr,
    ep0_state: &mut ProducerRing,
    bm_request_type: u8,
    b_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    data: FencedDmaAddr,
) -> Result<(), &'static str> {
    let setup_param = (bm_request_type as u64)
        | ((b_request as u64) << 8)
        | ((w_value as u64) << 16)
        | ((w_index as u64) << 32)
        | ((w_length as u64) << 48);
    // Setup Stage: IDT (immediate data), TRT=3 (IN data stage).
    let _ = enqueue_ep0(
        ep0_ring,
        ep0_addr,
        ep0_state,
        Trb {
            parameter: setup_param,
            status: 8,
            control: control_for(TrbType::SetupStage, false, (1 << 6) | (3 << 16)),
        },
    );
    // Data Stage IN (DIR=1).
    let _ = enqueue_ep0(
        ep0_ring,
        ep0_addr,
        ep0_state,
        Trb {
            parameter: data.phys(),
            status: w_length as u32,
            control: control_for(TrbType::DataStage, false, 1 << 16),
        },
    );
    // Status Stage OUT (DIR=0), IOC.
    let status_phys = enqueue_ep0(
        ep0_ring,
        ep0_addr,
        ep0_state,
        Trb {
            parameter: 0,
            status: 0,
            control: control_for(TrbType::StatusStage, false, 1 << 5),
        },
    );
    // Doorbell: slot's EP0 = DB target 1.
    mmio_write32(db, (slot_id as u64) * 4, 1);
    // A short data stage emits its own Transfer Event before the status
    // stage's IOC event — consume events until the STATUS TRB's arrives
    // (matching by TRB pointer), so a leftover never desyncs the next
    // transfer (the F5 one-event-then-silence class, designed out).
    let mut tolerance: u32 = 8;
    loop {
        let event = wait_event(TrbType::TransferEvent).ok_or("control transfer: no event (F5)")?;
        let code = event.completion_code();
        if event.parameter == status_phys {
            return match code {
                1 | 13 => Ok(()),
                _ => Err("control transfer failed at status stage"),
            };
        }
        if code != 1 && code != 13 {
            return Err("control transfer failed mid-stage");
        }
        #[cfg(feature = "mt7601u")]
        note_operation_late_event();
        tolerance -= 1;
        if tolerance == 0 {
            #[cfg(feature = "mt7601u")]
            note_operation_desync_exhaustion();
            return Err("control transfer: event stream desync (F5)");
        }
    }
}

fn enqueue_ep0(
    ep0_ring: &DmaBuffer,
    ep0_addr: FencedDmaAddr,
    state: &mut ProducerRing,
    trb: Trb,
) -> u64 {
    let step = state.enqueue_step();
    let mut armed = trb;
    armed.control =
        (armed.control & !ring::TRB_CYCLE) | if step.cycle { ring::TRB_CYCLE } else { 0 };
    write_trb(ep0_ring, step.index, armed);
    if let Some(link_cycle) = step.rearm_link_cycle {
        arm_link_trb(ep0_ring, ep0_addr, link_cycle);
    }
    ep0_addr.phys() + (step.index * 16) as u64
}

/// Copy `len` bytes out of a DMA buffer for the pure parsers (≤ 1 KiB,
/// volatile per byte — the device wrote them).
fn read_dma_bytes(buf: &DmaBuffer, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        out.push(read_dma_u8(buf, i));
    }
    out
}

/// Slot-context speed field (xHCI table 6-4) — shared by Address Device
/// and Configure Endpoint (both re-supply the full slot context).
fn slot_speed_bits(speed: super::UsbSpeed) -> u32 {
    match speed {
        super::UsbSpeed::Full => 1,
        super::UsbSpeed::Low => 2,
        super::UsbSpeed::High => 3,
        super::UsbSpeed::Super => 4,
        super::UsbSpeed::Unknown(raw) => raw as u32,
    }
}

// --- Claimed devices (BUILD-B, ADR-047) -----------------------------------------

/// A claimed device's live xHCI state: the slot stays addressed, EP0 stays
/// usable for class/standard control requests, and `configure_endpoints`
/// opens async bulk/interrupt pipes. Handed to exactly one class driver by
/// the offer; the driver owns it for the device's lifetime (no unbind in
/// scope). Every DMA address programmed through this surface is a
/// [`FencedDmaAddr`] — `PhysAddr` does not appear (I-14(c1), structural).
pub struct ClaimedDevice {
    pub slot_id: u8,
    port: u32,
    pub speed: super::UsbSpeed,
    csz64: bool,
    db: VirtAddr,
    dcbaa: Arc<DmaBuffer>,
    // Held for RAII (keeps the Output Device Context alive for the xHC) and read
    // by `endpoint_state` (CB-2) only under `mt7601u`; without it the field is
    // RAII-only, so allow the never-read lint in that configuration.
    #[cfg_attr(not(feature = "mt7601u"), allow(dead_code))]
    out_ctx: Arc<DmaBuffer>,
    input: Arc<DmaBuffer>,
    input_addr: FencedDmaAddr,
    ep0_ring: Arc<DmaBuffer>,
    ep0_addr: FencedDmaAddr,
    ep0_state: ProducerRing,
    data: Arc<DmaBuffer>,
    data_addr: FencedDmaAddr,
}

/// One configured async endpoint: its fenced transfer ring, producer state,
/// doorbell coordinates, and the bounded completion queue the ISR routes
/// into. Owned by the class driver (inside ITS lock — the pipe itself
/// takes no xHCI lock: the doorbell is MMIO, the ring is exclusively ours).
pub struct EndpointPipe {
    slot_id: u8,
    pub dci: u8,
    db: VirtAddr,
    ring: Arc<DmaBuffer>,
    ring_addr: FencedDmaAddr,
    state: ProducerRing,
    queue: Arc<IrqSafeMutex<Vec<Trb>>>,
    drops: Arc<AtomicU64>,
}

impl EndpointPipe {
    /// Enqueue one Normal TRB (IOC | ISP — every frame completes with an
    /// event, short packets included) and ring the doorbell. Returns the
    /// ring index used — the driver's buffer-table key.
    pub fn submit(&mut self, addr: FencedDmaAddr, len: u32) -> usize {
        let step = self.state.enqueue_step();
        let trb = Trb {
            parameter: addr.phys(),
            status: len & 0x1_FFFF,
            control: control_for(TrbType::Normal, step.cycle, (1 << 5) | (1 << 2)),
        };
        write_trb(&self.ring, step.index, trb);
        if let Some(link_cycle) = step.rearm_link_cycle {
            arm_link_trb(&self.ring, self.ring_addr, link_cycle);
        }
        mmio_write32(self.db, (self.slot_id as u64) * 4, self.dci as u32);
        step.index
    }

    /// Drain this endpoint's routed completions (process context). The
    /// replacement queue is pre-reserved so the ISR never allocates
    /// (Forge m2: replace, not clone — no per-drain copy).
    pub fn completions(&mut self) -> Vec<Trb> {
        let mut queue = self.queue.lock();
        core::mem::replace(&mut *queue, Vec::with_capacity(EP_QUEUE_CAP))
    }

    /// Map a Transfer Event's TRB pointer back to this ring's index.
    pub fn index_of(&self, trb_pointer: u64) -> Option<usize> {
        let base = self.ring_addr.phys();
        if trb_pointer < base {
            return None;
        }
        let idx = ((trb_pointer - base) / 16) as usize;
        if idx >= RING_TRBS { None } else { Some(idx) }
    }

    /// I-14(b): completions dropped because the bounded queue was full.
    pub fn drops(&self) -> u64 {
        self.drops.load(Ordering::Relaxed)
    }
}

impl ClaimedDevice {
    /// CB-2: read endpoint `dci`'s current EP State (xHCI §6.2.3) from the
    /// Output Device Context the xHC maintains. This IS the recovery
    /// discriminator (capture-before-you-need-it): `EP_STATE_HALTED` ⇒ a
    /// transaction error halted the pipe host-side; any other state ⇒
    /// stuck-but-not-halted.
    #[cfg(feature = "mt7601u")]
    pub fn endpoint_state(&self, dci: u8) -> u8 {
        let off = (dci as usize) * ctx_stride(self.csz64);
        ring::ep_state(read_dma_u32(&self.out_ctx, off))
    }

    /// Restart one bulk endpoint host-side for a bounded MT7601U transport
    /// probe. Reads the EP state and (per `recovery_action`) issues Reset Endpoint
    /// (Halted) or Stop Endpoint (stuck) to reach Stopped, restarts the transfer
    /// ring at a fresh lap, and Set-TR-Dequeues the xHC onto it with the matching
    /// cycle. The EP is left Stopped at a clean dequeue; the next `pipe.submit()`
    /// rings the doorbell and the EP resumes Running. Stale TRBs from before the
    /// wedge are overwritten by the next submit; in-flight transfers on this pipe
    /// are abandoned (the driver re-posts) — correct for a recovery, not a resume.
    /// `enabled` is the CB-2 kill-switch: `false` ⇒ no command is submitted and
    /// the pipe is left exactly as before (prior behavior, bit-for-bit).
    ///
    /// I-14: submitted from process context via `submit_command`
    /// (`CONTROLLER.lock` = `IrqSafeMutex`, no governance lock); the transfer
    /// ring is already a `FencedDmaAddr` so Set-TR-Dequeue adds no unfenced DMA;
    /// every command wait is bounded by `submit_command`'s budget.
    #[cfg(feature = "mt7601u")]
    pub fn recover_endpoint(&self, pipe: &mut EndpointPipe, enabled: bool) -> EndpointRecovery {
        let state_before = self.endpoint_state(pipe.dci);
        let reset_or_stop_ok = match ring::recovery_action(enabled, state_before) {
            // Kill-switch disabled: submit nothing — prior behavior, bit-for-bit.
            ring::RecoveryAction::Skip => {
                return EndpointRecovery {
                    state_before,
                    reset_or_stop_ok: false,
                    set_dequeue_ok: false,
                };
            }
            ring::RecoveryAction::Reset => cmd_reset_endpoint(self.slot_id, pipe.dci) == Some(1),
            ring::RecoveryAction::Stop => cmd_stop_endpoint(self.slot_id, pipe.dci) == Some(1),
            // Already Stopped: no Reset/Stop command (the xHC rejects re-Stopping
            // a Stopped EP as Context State Error — the ch14 silicon bug). The EP
            // is already in the state Set-TR-Dequeue needs.
            ring::RecoveryAction::Requeue => true,
        };
        // Restart the transfer ring at a fresh lap and point the xHC's dequeue
        // there with the matching cycle; the next submit() rings the doorbell.
        pipe.state = ProducerRing::new(RING_TRBS);
        arm_link_trb(&pipe.ring, pipe.ring_addr, pipe.state.cycle);
        let set_dequeue_ok = cmd_set_tr_dequeue(
            self.slot_id,
            pipe.dci,
            pipe.ring_addr.phys(),
            pipe.state.cycle,
        ) == Some(1);
        EndpointRecovery {
            state_before,
            reset_or_stop_ok,
            set_dequeue_ok,
        }
    }

    /// Clear a device-side endpoint halt via ClearFeature(ENDPOINT_HALT) on EP0
    /// (USB 2.0 §9.4.1). Success proves only that the default control path
    /// accepted the request; it does not establish MCU liveness or the cause of
    /// a command-response timeout. `dci` is the endpoint's Device Context Index
    /// (DCI = 2·ep_num + dir_in), so the USB endpoint address is
    /// `ep_num | (dir_in << 7)`.
    #[cfg(feature = "mt7601u")]
    pub fn clear_endpoint_halt(&mut self, dci: u8) -> Result<(), &'static str> {
        let ep_addr = (u16::from(dci) >> 1) | (u16::from(dci & 1) << 7);
        // bmRequestType=0x02 (host→device | standard | endpoint),
        // bRequest=CLEAR_FEATURE(1), wValue=ENDPOINT_HALT(0), wIndex=ep address.
        self.control_no_data(0x02, 0x01, 0x0000, ep_addr)
    }

    /// One control IN transfer on EP0; the payload is copied into `out`
    /// (up to `len` bytes).
    pub fn control_read(
        &mut self,
        bm_request_type: u8,
        b_request: u8,
        w_value: u16,
        w_index: u16,
        len: u16,
        out: &mut [u8],
    ) -> Result<(), &'static str> {
        control_in(
            self.db,
            self.slot_id,
            &self.ep0_ring,
            self.ep0_addr,
            &mut self.ep0_state,
            bm_request_type,
            b_request,
            w_value,
            w_index,
            len,
            self.data_addr,
        )?;
        let n = out.len().min(len as usize);
        for (i, byte) in out.iter_mut().enumerate().take(n) {
            *byte = read_dma_u8(&self.data, i);
        }
        Ok(())
    }

    /// One no-data control transfer (SET_CONFIGURATION / SET_INTERFACE /
    /// class requests). A STALL is reported as the distinct
    /// `"control stalled"` so callers can tolerate optional requests.
    pub fn control_no_data(
        &mut self,
        bm_request_type: u8,
        b_request: u8,
        w_value: u16,
        w_index: u16,
    ) -> Result<(), &'static str> {
        control_no_data(
            self.db,
            self.slot_id,
            &self.ep0_ring,
            self.ep0_addr,
            &mut self.ep0_state,
            bm_request_type,
            b_request,
            w_value,
            w_index,
        )
    }

    /// One control transfer with a host-to-device DATA stage — vendor or
    /// class register writes (e.g. the ASIX AX88179 `AX_ACCESS_MAC`
    /// TX-enable). The payload is copied into the shared data buffer and the
    /// device reads it; ≤ 64 bytes (the buffer is a 4 KiB arena page).
    pub fn control_write(
        &mut self,
        bm_request_type: u8,
        b_request: u8,
        w_value: u16,
        w_index: u16,
        data: &[u8],
    ) -> Result<(), &'static str> {
        let len = data.len().min(64);
        // SAFETY: the data buffer is an exclusively owned arena page; `len`
        // is bounded to 64 < 4096. The device reads these bytes on the OUT
        // data stage after we ring the doorbell.
        unsafe {
            let p = self.data.as_mut_ptr();
            for (i, byte) in data.iter().enumerate().take(len) {
                core::ptr::write_volatile(p.add(i), *byte);
            }
        }
        control_out(
            self.db,
            self.slot_id,
            &self.ep0_ring,
            self.ep0_addr,
            &mut self.ep0_state,
            bm_request_type,
            b_request,
            w_value,
            w_index,
            len as u16,
            self.data_addr,
        )
    }

    /// Read a string descriptor (US English) into `out`; the descriptor's
    /// own bLength bounds the useful bytes.
    pub fn read_string(&mut self, index: u8, out: &mut [u8; 64]) -> Result<(), &'static str> {
        self.control_read(0x80, 6, 0x0300 | index as u16, 0x0409, 64, out)
    }

    /// Configure async endpoints (xHCI §4.6.6): fenced transfer rings, EP
    /// contexts in both CSZ layouts, one Configure Endpoint command, and
    /// ISR routing registration. Returns one pipe per requested endpoint,
    /// in order.
    pub fn configure_endpoints(
        &mut self,
        endpoints: &[super::UsbEndpoint],
    ) -> Result<Vec<EndpointPipe>, &'static str> {
        let stride = ctx_stride(self.csz64);
        struct PendingEp {
            dci: u8,
            ring: Arc<DmaBuffer>,
            addr: FencedDmaAddr,
            state: ProducerRing,
            ep_type: u32,
            max_packet: u16,
            interval: u32,
            max_burst: u8,
            is_interrupt: bool,
        }
        let mut pending = Vec::new();
        let mut add_flags: u32 = 1; // A0: the slot context rides along.
        let mut max_dci = 1u8;
        for ep in endpoints {
            let dci_value = ring::dci(ep.number(), ep.is_in());
            let ringbuf = DmaBuffer::alloc_in_arena(4096).map_err(|_| "ep ring alloc")?;
            let addr = fence_buffer(&ringbuf)?;
            let state = ProducerRing::new(RING_TRBS);
            arm_link_trb(&ringbuf, addr, state.cycle);
            add_flags |= 1u32 << dci_value;
            max_dci = max_dci.max(dci_value);
            // The E-bank configure breadcrumb (Forge M1): per-EP geometry,
            // including the SS companion's Max Burst — a SuperSpeed
            // adapter mis-running at burst-1 must be visible in the sink.
            crate::serial_println!(
                "xhci: configure slot={} dci={} type={} mps={} interval={} max_burst={}",
                self.slot_id,
                dci_value,
                ring::ep_type_code(ep.attributes, ep.is_in()),
                ep.max_packet,
                ring::endpoint_interval(self.speed.is_high_or_super(), ep.interval, ep.attributes),
                ep.max_burst,
            );
            pending.push(PendingEp {
                dci: dci_value,
                ring: ringbuf,
                addr,
                state,
                ep_type: ring::ep_type_code(ep.attributes, ep.is_in()),
                max_packet: ep.max_packet,
                interval: ring::endpoint_interval(
                    self.speed.is_high_or_super(),
                    ep.interval,
                    ep.attributes,
                ),
                max_burst: ep.max_burst,
                is_interrupt: ep.is_interrupt(),
            });
        }
        crate::log::flush();
        // Input context: ICC(0) + slot(1) + EP contexts at (1 + DCI)·stride.
        // SAFETY: the input context is an exclusively owned arena page;
        // (2 + max_dci) · stride ≤ 33 · 64 = 2112 < 4096. Volatile: the
        // controller reads it on Configure Endpoint.
        unsafe {
            let p = self.input.as_mut_ptr();
            core::ptr::write_bytes(p, 0, (2 + max_dci as usize) * stride);
            core::ptr::write_volatile(p.add(4) as *mut u32, add_flags);
            // Slot context: context entries = max DCI, plus the same
            // route/speed/port the slot was addressed with.
            let slot = p.add(stride);
            core::ptr::write_volatile(
                slot as *mut u32,
                ((max_dci as u32) << 27) | (slot_speed_bits(self.speed) << 20),
            );
            core::ptr::write_volatile(slot.add(4) as *mut u32, self.port << 16);
            for ep in &pending {
                let base = p.add((1 + ep.dci as usize) * stride);
                core::ptr::write_volatile(base as *mut u32, ep.interval << 16);
                // Max Burst Size [15:8] from the SS Endpoint Companion
                // (0 for FS/HS, where no companion exists — Forge M1).
                core::ptr::write_volatile(
                    base.add(4) as *mut u32,
                    ((ep.max_packet as u32) << 16)
                        | ((ep.max_burst as u32) << 8)
                        | (ep.ep_type << 3)
                        | (3 << 1),
                );
                core::ptr::write_volatile(base.add(8) as *mut u64, ep.addr.phys() | 1);
                // Average TRB length; interrupt EPs also carry Max ESIT
                // Payload in [31:16].
                let avg: u32 = if ep.is_interrupt {
                    ((ep.max_packet as u32) << 16) | 16
                } else {
                    1024
                };
                core::ptr::write_volatile(base.add(16) as *mut u32, avg);
            }
        }
        let configure = Trb {
            parameter: self.input_addr.phys(),
            status: 0,
            control: control_for(
                TrbType::ConfigureEndpointCmd,
                false,
                (self.slot_id as u32) << 24,
            ),
        };
        let completion =
            submit_command(configure).ok_or("configure-endpoint: no completion (F4)")?;
        if completion.completion_code() != 1 {
            // E2 discriminator: the code names the refusal class.
            crate::serial_println!(
                "xhci: configure-endpoint slot={} completion code={} (E2)",
                self.slot_id,
                completion.completion_code()
            );
            crate::log::flush();
            return Err("configure-endpoint refused");
        }
        let mut pipes = Vec::new();
        for ep in pending {
            let queue = Arc::new(IrqSafeMutex::new(Vec::with_capacity(EP_QUEUE_CAP)));
            let drops = Arc::new(AtomicU64::new(0));
            EP_ROUTES.lock().push(EpRoute {
                slot: self.slot_id,
                dci: ep.dci,
                queue: queue.clone(),
                drops: drops.clone(),
            });
            pipes.push(EndpointPipe {
                slot_id: self.slot_id,
                dci: ep.dci,
                db: self.db,
                ring: ep.ring,
                ring_addr: ep.addr,
                state: ep.state,
                queue,
                drops,
            });
        }
        Ok(pipes)
    }

    /// The unclaimed path — BUILD-A's teardown exactly: clear the DCBAA
    /// entry FIRST, then let the contexts and rings drop (Anti-3: the
    /// controller never holds a dangling pointer).
    pub(crate) fn teardown(self) {
        write_dma_u64_zero(&self.dcbaa, self.slot_id as usize);
    }
}

/// One no-data control transfer on EP0: Setup (TRT=0) → Status IN (IOC).
/// A STALL at the status stage returns the distinct `"control stalled"`.
#[allow(clippy::too_many_arguments)] // the EP0 programming tuple — local to this module
fn control_no_data(
    db: VirtAddr,
    slot_id: u8,
    ep0_ring: &DmaBuffer,
    ep0_addr: FencedDmaAddr,
    ep0_state: &mut ProducerRing,
    bm_request_type: u8,
    b_request: u8,
    w_value: u16,
    w_index: u16,
) -> Result<(), &'static str> {
    let setup_param = (bm_request_type as u64)
        | ((b_request as u64) << 8)
        | ((w_value as u64) << 16)
        | ((w_index as u64) << 32);
    // Setup Stage: IDT, TRT=0 (no data stage).
    let _ = enqueue_ep0(
        ep0_ring,
        ep0_addr,
        ep0_state,
        Trb {
            parameter: setup_param,
            status: 8,
            control: control_for(TrbType::SetupStage, false, 1 << 6),
        },
    );
    // Status Stage: a control transfer with NO data stage ALWAYS has an
    // IN status stage (DIR=1), regardless of bmRequestType's direction —
    // USB 2.0 §8.5.3 / xHCI §4.11.2.2. Do not "fix" this to OUT for
    // host-to-device requests; that breaks every SET_* through here.
    let status_phys = enqueue_ep0(
        ep0_ring,
        ep0_addr,
        ep0_state,
        Trb {
            parameter: 0,
            status: 0,
            control: control_for(TrbType::StatusStage, false, (1 << 16) | (1 << 5)),
        },
    );
    mmio_write32(db, (slot_id as u64) * 4, 1);
    let mut tolerance: u32 = 8;
    loop {
        let event = wait_event(TrbType::TransferEvent).ok_or("control transfer: no event (F5)")?;
        let code = event.completion_code();
        if event.parameter == status_phys {
            return match code {
                1 | 13 => Ok(()),
                6 => Err("control stalled"),
                _ => Err("control transfer failed at status stage"),
            };
        }
        if code == 6 {
            return Err("control stalled");
        }
        if code != 1 && code != 13 {
            return Err("control transfer failed mid-stage");
        }
        #[cfg(feature = "mt7601u")]
        note_operation_late_event();
        tolerance -= 1;
        if tolerance == 0 {
            #[cfg(feature = "mt7601u")]
            note_operation_desync_exhaustion();
            return Err("control transfer: event stream desync (F5)");
        }
    }
}

/// One control transfer with a host-to-device DATA stage on EP0: Setup
/// (TRT=2, OUT data) → Data OUT → Status IN (IOC). The device reads
/// `w_length` bytes from `data`. A STALL returns `"control-out stalled"`.
#[allow(clippy::too_many_arguments)] // the EP0 programming tuple — local to this module
fn control_out(
    db: VirtAddr,
    slot_id: u8,
    ep0_ring: &DmaBuffer,
    ep0_addr: FencedDmaAddr,
    ep0_state: &mut ProducerRing,
    bm_request_type: u8,
    b_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    data: FencedDmaAddr,
) -> Result<(), &'static str> {
    let setup_param = (bm_request_type as u64)
        | ((b_request as u64) << 8)
        | ((w_value as u64) << 16)
        | ((w_index as u64) << 32)
        | ((w_length as u64) << 48);
    // Setup Stage: IDT, TRT=2 (OUT data stage) when there is a payload.
    let trt: u32 = if w_length > 0 { 2 << 16 } else { 0 };
    let _ = enqueue_ep0(
        ep0_ring,
        ep0_addr,
        ep0_state,
        Trb {
            parameter: setup_param,
            status: 8,
            control: control_for(TrbType::SetupStage, false, (1 << 6) | trt),
        },
    );
    if w_length > 0 {
        // Data Stage OUT (DIR=0).
        let _ = enqueue_ep0(
            ep0_ring,
            ep0_addr,
            ep0_state,
            Trb {
                parameter: data.phys(),
                status: w_length as u32,
                control: control_for(TrbType::DataStage, false, 0),
            },
        );
    }
    // Status Stage IN (DIR=1), IOC — an OUT-data control transfer's status
    // stage is IN (xHCI §4.11.2.2).
    let status_phys = enqueue_ep0(
        ep0_ring,
        ep0_addr,
        ep0_state,
        Trb {
            parameter: 0,
            status: 0,
            control: control_for(TrbType::StatusStage, false, (1 << 16) | (1 << 5)),
        },
    );
    mmio_write32(db, (slot_id as u64) * 4, 1);
    let mut tolerance: u32 = 8;
    loop {
        let event = wait_event(TrbType::TransferEvent).ok_or("control-out: no event")?;
        let code = event.completion_code();
        if event.parameter == status_phys {
            return match code {
                1 | 13 => Ok(()),
                6 => Err("control-out stalled"),
                _ => Err("control-out failed at status stage"),
            };
        }
        if code == 6 {
            return Err("control-out stalled");
        }
        if code != 1 && code != 13 {
            return Err("control-out failed mid-stage");
        }
        #[cfg(feature = "mt7601u")]
        note_operation_late_event();
        tolerance -= 1;
        if tolerance == 0 {
            #[cfg(feature = "mt7601u")]
            note_operation_desync_exhaustion();
            return Err("control-out: event stream desync");
        }
    }
}
