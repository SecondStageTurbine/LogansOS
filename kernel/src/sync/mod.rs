// SPDX-License-Identifier: MPL-2.0
//! Kernel-owned synchronization primitives.
//!
//! Keep synchronization mechanisms that participate in low-level kernel ABI
//! here instead of depending on third-party private layouts.

pub mod irqsafe_mutex;
pub mod lock_owner;
pub mod switch_mutex;
pub mod ticket_mutex;

pub use irqsafe_mutex::IrqSafeMutex;
pub use switch_mutex::SwitchMutex;

/// The documented global lock order for the AI-residency locks (Phase 7
/// BUILD-P4a, ANSPEC-001 §9.12; extends `admission.rs`'s historical
/// `QUEUES → TABLE → scheduler` note). A thread may only acquire an ordered
/// lock whose level is strictly greater than every ordered level it already
/// holds; debug builds assert this at the acquire (see `switch_mutex::lockdep`).
/// The scheduler's internal lock is acquired after any of these (a park
/// releases the caller's guard atomically with the switch) and stays unordered.
pub mod lock_order {
    /// Block-device I/O gate (OT-59 #2, virtio-blk single-in-flight
    /// serialization). Thread-context-ONLY: acquired at the top of
    /// `submit_pending_and_wait`, OUTER of the device `inner` lock, and below the
    /// whole AI-residency band — block I/O is a leaf that never nests an AI or USB
    /// lock, and the AI path never does block I/O while holding an AI lock. The
    /// gate carries this single lockdep level so a future *thread-context*
    /// `inner → io_gate` inversion trips the CP-1 panic instead of a silent stall.
    /// The device `inner` lock is deliberately NOT keyed here (left unordered):
    /// it is also acquired in the completion ISR, where lockdep is invalid (it
    /// keys by thread-id) — ordering it would false-panic when a completion IRQ
    /// preempts a thread holding an AI-governance lock (RECENCY, etc.). This
    /// matches AHCI/NVMe, which keep their ISR-touched inner lock unordered.
    pub const BLOCK_IO_GATE: u8 = 5;
    /// `ai::admission` `QUEUES` — taken first on the admission path.
    pub const ADMISSION_QUEUES: u8 = 10;
    /// The slot-arena table (BUILD-O1a). Below TENSOR_SLOT so glue MAY nest a
    /// slot acquire under the table lock, though the arena never parks under
    /// it (mint snapshots the SlotRef and drops the table lock first).
    pub const ARENA: u8 = 12;
    /// A tensor-owning slot (the container references are minted from).
    pub const TENSOR_SLOT: u8 = 15;
    /// `ai::reservation` `RESERVATIONS` — taken under the owning slot.
    pub const RESERVATION: u8 = 20;
    /// `accounting` `INFLIGHT` — principal resolution; never held across TABLE.
    pub const ACCT_INFLIGHT: u8 = 22;
    /// `accounting` `TABLE` (§9.5) — leaf-like; never nested with RECENCY.
    pub const ACCT_TABLE: u8 = 25;
    /// `accounting` `RECENCY` (§9.5 extension) — leaf-like.
    pub const ACCT_RECENCY: u8 = 28;
    /// USB locks (Phase 7.5 BUILD-A/B) order ABOVE every AI-governance
    /// level: acquiring any AI lock while holding a USB lock is a lockdep
    /// panic — the machine-checked half of I-14(a). A class driver's
    /// device lock (USB_DEVICE) sits below USB_XHCI so the driver may call
    /// into the controller (which takes USB_XHCI) while holding it.
    #[allow(dead_code)] // Documented lock-order level for USB class drivers.
    pub const USB_DEVICE: u8 = 29;
    #[allow(dead_code)] // Documented lock-order level for xHCI controller paths.
    pub const USB_XHCI: u8 = 30;
    #[allow(dead_code)] // Documented lock-order level for USB core paths.
    pub const USB_CORE: u8 = 31;
    /// PCI NIC driver device locks (the RTL8125, rung 1): above every
    /// AI-governance level, like the USB band, so acquiring a governance lock
    /// while holding one is a lockdep panic (I-14(a)). Never nested with a
    /// USB lock.
    #[cfg(feature = "rtl8125")]
    pub const NET_DRIVER: u8 = 29;
}
