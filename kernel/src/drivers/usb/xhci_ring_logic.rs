// SPDX-License-Identifier: MPL-2.0
//! Pure xHCI ring arithmetic (Phase 7.5 BUILD-A, ADR-046): TRB layout, the
//! cycle-bit protocol, producer (command/transfer ring) enqueue with Link-TRB
//! wrap, and consumer (event ring) dequeue — kernel-free and host-testable
//! (path-`mod`'d by `tools/ai-ops-tests`). This file IS the F4/F5
//! discriminator arithmetic: if the cycle/dequeue logic is wrong, it is
//! wrong HERE, under a truth table, not in a blind bare-metal run.
//!
//! xHCI 1.2 §4.9 (rings), §4.11 (TRBs), §6.4 (TRB formats). A ring is a
//! contiguous array of 16-byte TRBs. Producers own a Cycle State (PCS) bit:
//! a TRB belongs to the consumer when its cycle bit == the consumer's CCS.
//! The last TRB of a producer ring is a Link TRB back to the start with the
//! Toggle Cycle flag; crossing it flips PCS. Event rings have no Link TRB —
//! the consumer walks segments and flips CCS at the segment end.

/// One Transfer Request Block — four little-endian dwords (xHCI §4.11.1).
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct Trb {
    pub parameter: u64,
    pub status: u32,
    pub control: u32,
}

pub const TRB_CYCLE: u32 = 1 << 0;
pub const TRB_TOGGLE_CYCLE: u32 = 1 << 1;
/// control\[15:10\] — TRB Type.
pub const TRB_TYPE_SHIFT: u32 = 10;
pub const TRB_TYPE_MASK: u32 = 0x3F << TRB_TYPE_SHIFT;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum TrbType {
    Normal = 1,
    SetupStage = 2,
    DataStage = 3,
    StatusStage = 4,
    Link = 6,
    EnableSlotCmd = 9,
    DisableSlotCmd = 10,
    AddressDeviceCmd = 11,
    ConfigureEndpointCmd = 12,
    EvaluateContextCmd = 13,
    // CB-2 endpoint-recovery command TRBs — constructed only by the
    // `endpoint_recovery` builders (gated `mt7601u` OR `test`), so gate the
    // variants identically to avoid a never-constructed warning in a non-WiFi
    // kernel under `-D warnings`.
    #[cfg(any(feature = "mt7601u", feature = "usb-storage", test))]
    ResetEndpointCmd = 14,
    #[cfg(any(feature = "mt7601u", feature = "usb-storage", test))]
    StopEndpointCmd = 15,
    #[cfg(any(feature = "mt7601u", feature = "usb-storage", test))]
    SetTrDequeuePointerCmd = 16,
    NoOpCmd = 23,
    TransferEvent = 32,
    CmdCompletionEvent = 33,
    PortStatusChangeEvent = 34,
    HostControllerEvent = 37,
}

impl Trb {
    pub fn trb_type(&self) -> u32 {
        (self.control & TRB_TYPE_MASK) >> TRB_TYPE_SHIFT
    }
    pub fn cycle(&self) -> bool {
        self.control & TRB_CYCLE != 0
    }
    /// Completion code for event TRBs (status\[31:24\], xHCI §6.4.5).
    pub fn completion_code(&self) -> u32 {
        self.status >> 24
    }
    /// Slot ID for event TRBs (control\[31:24\]).
    pub fn slot_id(&self) -> u32 {
        self.control >> 24
    }
    /// Endpoint ID (the DCI) for Transfer Event TRBs (control\[20:16\],
    /// xHCI §6.4.2.1) — the routing key for async endpoint completions.
    pub fn ep_id(&self) -> u32 {
        (self.control >> 16) & 0x1F
    }
    /// TRB Transfer Length residue for Transfer Events (status\[23:0\]):
    /// bytes NOT transferred — a Short Packet completion's actual length is
    /// `posted - residue`.
    pub fn transfer_residue(&self) -> u32 {
        self.status & 0x00FF_FFFF
    }
}

/// Device Context Index for an endpoint (xHCI §4.5.1): EP0 is DCI 1; for
/// endpoint number n, OUT is 2n and IN is 2n+1.
pub fn dci(ep_number: u8, dir_in: bool) -> u8 {
    if ep_number == 0 {
        return 1;
    }
    ep_number * 2 + u8::from(dir_in)
}

/// Endpoint Context EP-Type field (xHCI table 6-9) from the descriptor's
/// bmAttributes transfer type and direction: 1/5 isoch, 2/6 bulk, 3/7
/// interrupt (OUT/IN), 4 control (bidirectional).
pub fn ep_type_code(attributes: u8, dir_in: bool) -> u32 {
    match attributes & 0x03 {
        0 => 4,
        ty => u32::from(ty) + if dir_in { 4 } else { 0 },
    }
}

/// Endpoint Context Interval field (xHCI §6.2.3.6): periodic endpoints
/// express their service interval as 2^interval × 125 µs. HS/SS interrupt
/// bInterval is already an exponent (1..16 ⇒ field 0..15); FS/LS interrupt
/// bInterval is in MILLISECONDS (1 ms = 8 × 125 µs = 2^3), so the field is
/// `3 + floor(log2(bInterval))`. Bulk/control endpoints use 0.
pub fn endpoint_interval(high_speed_or_super: bool, b_interval: u8, attributes: u8) -> u32 {
    if attributes & 0x03 != 3 {
        return 0;
    }
    if high_speed_or_super {
        u32::from(b_interval.clamp(1, 16)) - 1
    } else {
        let ms = u32::from(b_interval.max(1));
        3 + (31 - ms.leading_zeros())
    }
}

/// Build the control dword for a TRB of `ty` with `cycle`, plus extra flags.
pub fn control_for(ty: TrbType, cycle: bool, extra_flags: u32) -> u32 {
    ((ty as u32) << TRB_TYPE_SHIFT) | extra_flags | if cycle { TRB_CYCLE } else { 0 }
}

// ── CB-2: endpoint-halt recovery (xHCI §4.6.8 Reset Endpoint, §4.6.9 Stop
// Endpoint, §4.6.10 Set TR Dequeue Pointer) ──────────────────────────────────
//
// After a USB Transaction Error on a bulk pipe the xHC transitions that
// endpoint to the *Halted* state HOST-SIDE and stops servicing its transfer
// ring. A device-side reset (the mt7601u `vendor_reset` + firmware reload)
// cannot clear this — only the host controller can, via Reset Endpoint. That is
// why a full chip re-init still left `recovered_after_wedge=0`: the chip booted
// fresh but the host's view of the bulk pipe stayed Halted. Recovery sequence:
// read the EP state; Reset Endpoint (Halted) or Stop Endpoint (stuck but not
// Halted — Reset Endpoint requires Halted or the xHC returns Context State
// Error); then Set TR Dequeue Pointer to a known ring position with the
// matching Dequeue Cycle State; then ring the doorbell.
//
// Command-TRB field layout (xHCI §6.4.3): Slot ID in control[31:24], Endpoint
// ID (DCI) in control[20:16], TRB Type in control[15:10] — corroborated by the
// in-repo `slot_id()`/`ep_id()` readers above. `submit_command` overwrites the
// cycle bit when it enqueues, so these builders emit cycle=0.

// Gated to `mt7601u` (the only kernel consumer) OR `test` (the host byte-oracle
// in tools/ai-ops-tests), so a non-WiFi kernel carries none of it — no dead code
// under the default-features `-D warnings` clippy gate.
#[cfg(any(feature = "mt7601u", feature = "usb-storage", test))]
pub use endpoint_recovery::*;

#[cfg(any(feature = "mt7601u", feature = "usb-storage", test))]
mod endpoint_recovery {
    use super::{Trb, TrbType, control_for};

    /// EP Context `EP State` field (xHCI §6.2.3, table 6-8): Output Device
    /// Context endpoint-context dword0, bits[2:0].
    pub const EP_STATE_DISABLED: u8 = 0;
    pub const EP_STATE_RUNNING: u8 = 1;
    pub const EP_STATE_HALTED: u8 = 2;
    pub const EP_STATE_STOPPED: u8 = 3;
    pub const EP_STATE_ERROR: u8 = 4;

    /// Extract the EP State (bits[2:0]) from an endpoint-context dword0.
    pub fn ep_state(ctx_dword0: u32) -> u8 {
        (ctx_dword0 & 0x7) as u8
    }

    /// What recovery the host controller should perform on a wedged endpoint —
    /// the kill-switch AND the Halted-vs-stuck discriminator in one pure,
    /// host-tested decision. `Skip` is governance Rule 1: disabled ≡ prior
    /// behavior, no command submitted, bit-for-bit.
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub enum RecoveryAction {
        /// Kill-switch disabled: do nothing (prior behavior).
        Skip,
        /// EP Halted ⇒ Reset Endpoint Command (Halted → Stopped).
        Reset,
        /// EP stuck but not Halted ⇒ Stop Endpoint Command (→ Stopped).
        Stop,
        /// EP already Stopped ⇒ no command (re-Stopping a Stopped EP is rejected
        /// by the xHC as Context State Error — the silicon bug at ch14); go
        /// straight to Set TR Dequeue.
        Requeue,
    }

    /// Decide the recovery action from the kill-switch + the captured EP state.
    pub fn recovery_action(enabled: bool, ep_state: u8) -> RecoveryAction {
        if !enabled {
            RecoveryAction::Skip
        } else if ep_state == EP_STATE_HALTED {
            RecoveryAction::Reset
        } else if ep_state == EP_STATE_STOPPED {
            RecoveryAction::Requeue
        } else {
            RecoveryAction::Stop
        }
    }

    /// Reset Endpoint Command TRB (xHCI §6.4.3.7) — Halted → Stopped. `dci` is
    /// the Device Context Index of the endpoint (e.g. `dci(1, false)` = 2 for
    /// EP1 OUT).
    pub fn reset_endpoint_trb(slot_id: u8, dci: u8) -> Trb {
        Trb {
            parameter: 0,
            status: 0,
            control: control_for(
                TrbType::ResetEndpointCmd,
                false,
                ((slot_id as u32) << 24) | ((dci as u32) << 16),
            ),
        }
    }

    /// Stop Endpoint Command TRB (xHCI §6.4.3.6) — any state → Stopped. Used
    /// when the endpoint is wedged but NOT Halted (Reset Endpoint requires
    /// Halted). `suspend` sets the SP bit (control[23]).
    pub fn stop_endpoint_trb(slot_id: u8, dci: u8, suspend: bool) -> Trb {
        Trb {
            parameter: 0,
            status: 0,
            control: control_for(
                TrbType::StopEndpointCmd,
                false,
                ((slot_id as u32) << 24) | (u32::from(suspend) << 23) | ((dci as u32) << 16),
            ),
        }
    }

    /// Set TR Dequeue Pointer Command TRB (xHCI §6.4.3.9). `deq_ptr` is the new
    /// TR dequeue pointer (16-byte aligned — low 4 bits are cleared here);
    /// `dcs` is the Dequeue Cycle State the consumer resumes with. Stream ID 0
    /// (non-streaming bulk), so the Stream Context Type subfield (param
    /// bits[3:1]) is 0.
    pub fn set_tr_dequeue_trb(slot_id: u8, dci: u8, deq_ptr: u64, dcs: bool) -> Trb {
        Trb {
            parameter: (deq_ptr & !0xF) | u64::from(dcs),
            status: 0,
            control: control_for(
                TrbType::SetTrDequeuePointerCmd,
                false,
                ((slot_id as u32) << 24) | ((dci as u32) << 16),
            ),
        }
    }
}

/// Producer-ring state (command or transfer ring): the enqueue index and the
/// Producer Cycle State. The ring has `len` entries; entry `len-1` is the
/// permanent Link TRB (written once at init, its cycle bit maintained here).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ProducerRing {
    pub len: usize,
    pub enqueue: usize,
    pub cycle: bool,
}

/// What the producer must write for one enqueue step.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct EnqueueStep {
    /// Write the payload TRB at this index with this cycle bit.
    pub index: usize,
    pub cycle: bool,
    /// After the payload write, also re-arm the Link TRB at `len-1` with
    /// this cycle bit (Some ⇒ the enqueue just landed on the last payload
    /// slot and the NEXT submission wraps; the link's cycle must match the
    /// CURRENT producer cycle so the consumer chases it).
    pub rearm_link_cycle: Option<bool>,
}

impl ProducerRing {
    /// `len` total entries including the Link slot ⇒ `len - 1` payload slots.
    pub fn new(len: usize) -> Self {
        Self {
            len,
            enqueue: 0,
            cycle: true,
        }
    }

    /// One payload enqueue: returns where/how to write, and advances —
    /// wrapping past the Link TRB flips the producer cycle (xHCI §4.9.2.2).
    pub fn enqueue_step(&mut self) -> EnqueueStep {
        let index = self.enqueue;
        let cycle = self.cycle;
        let rearm_link_cycle;
        if self.enqueue + 1 == self.len - 1 {
            // The next slot is the Link TRB: arm it with the current cycle,
            // wrap to 0, and flip PCS for the next lap.
            rearm_link_cycle = Some(self.cycle);
            self.enqueue = 0;
            self.cycle = !self.cycle;
        } else {
            rearm_link_cycle = None;
            self.enqueue += 1;
        }
        EnqueueStep {
            index,
            cycle,
            rearm_link_cycle,
        }
    }
}

/// Consumer-ring state (the event ring, single segment): the dequeue index
/// and the Consumer Cycle State (xHCI §4.9.4).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ConsumerRing {
    pub len: usize,
    pub dequeue: usize,
    pub cycle: bool,
}

impl ConsumerRing {
    pub fn new(len: usize) -> Self {
        Self {
            len,
            dequeue: 0,
            cycle: true,
        }
    }

    /// Does a TRB with this control dword belong to us (the consumer) at the
    /// current dequeue position?
    pub fn owns(&self, control: u32) -> bool {
        (control & TRB_CYCLE != 0) == self.cycle
    }

    /// Consume one event: advance the dequeue; at the segment end wrap to 0
    /// and flip CCS. Returns the new dequeue index (for the ERDP write).
    pub fn advance(&mut self) -> usize {
        self.dequeue += 1;
        if self.dequeue == self.len {
            self.dequeue = 0;
            self.cycle = !self.cycle;
        }
        self.dequeue
    }
}

/// OT-100 R2, relay 395: admission to a ring shared under a lock, against an
/// optional deadline tick. With `None` this is `lock()`, today's unbounded
/// wait. With `Some(end)` the lock is only tried until `end`, and `end` is
/// read again once the lock is held: if it has passed meanwhile, the guard is
/// given back (dropped) and nothing is admitted, so no command is published
/// after its deadline, however long another CPU held the lock.
pub fn admit_until<G>(
    until: Option<u64>,
    mut now: impl FnMut() -> u64,
    mut try_lock: impl FnMut() -> Option<G>,
    lock: impl FnOnce() -> G,
    mut spin: impl FnMut(),
) -> Option<G> {
    let Some(end) = until else {
        return Some(lock());
    };
    loop {
        if now() >= end {
            return None;
        }
        if let Some(guard) = try_lock() {
            if now() >= end {
                drop(guard);
                return None;
            }
            return Some(guard);
        }
        spin();
    }
}

#[cfg(test)]
mod cb2_endpoint_recovery_oracle {
    //! Hand-laid-out byte fixtures for the CB-2 endpoint-recovery command TRBs
    //! (the independent format/ABI byte-oracle — every control/parameter dword
    //! below is computed BY HAND from the xHCI §6.4.3 field tables, NOT derived
    //! from the function under test, so a writer+reader that agree on a wrong
    //! layout cannot pass). The reverse cross-checks via the in-repo
    //! `trb_type()`/`ep_id()`/`slot_id()` readers confirm the same bytes decode
    //! back to the intended fields.
    use super::*;

    // slot=1, EP1 OUT ⇒ DCI = 1*2 + 0 = 2 (the mt7601u inband bulk-out pipe).
    const SLOT: u8 = 1;
    const DCI: u8 = 2;

    #[test]
    fn reset_endpoint_bytes() {
        // control = (Type 14 << 10) | (DCI 2 << 16) | (Slot 1 << 24)
        //         = 0x0000_3800 | 0x0002_0000 | 0x0100_0000 = 0x0102_3800
        let trb = reset_endpoint_trb(SLOT, DCI);
        assert_eq!(trb.control, 0x0102_3800, "Reset Endpoint control dword");
        assert_eq!(trb.parameter, 0);
        assert_eq!(trb.status, 0);
        assert_eq!(trb.control & TRB_CYCLE, 0, "builder emits cycle=0");
        // reverse: the bytes decode back to the intended fields.
        assert_eq!(trb.trb_type(), 14);
        assert_eq!(trb.ep_id(), DCI as u32);
        assert_eq!(trb.slot_id(), SLOT as u32);
    }

    #[test]
    fn stop_endpoint_bytes() {
        // control = (Type 15 << 10) | (DCI 2 << 16) | (Slot 1 << 24) = 0x0102_3C00
        let trb = stop_endpoint_trb(SLOT, DCI, false);
        assert_eq!(trb.control, 0x0102_3C00, "Stop Endpoint control (SP=0)");
        assert_eq!(trb.trb_type(), 15);
        // SP=1 lights control[23]: 0x0102_3C00 | 0x0080_0000 = 0x0182_3C00
        let trb_sp = stop_endpoint_trb(SLOT, DCI, true);
        assert_eq!(
            trb_sp.control, 0x0182_3C00,
            "Stop Endpoint SP=1 sets bit 23"
        );
    }

    #[test]
    fn set_tr_dequeue_bytes() {
        // control = (Type 16 << 10) | (DCI 2 << 16) | (Slot 1 << 24) = 0x0102_4000
        // param   = (deq & !0xF) | DCS; deq=0x1234_5680 (16-aligned), DCS=1
        //         = 0x1234_5680 | 1 = 0x1234_5681
        let trb = set_tr_dequeue_trb(SLOT, DCI, 0x1234_5680, true);
        assert_eq!(trb.control, 0x0102_4000, "Set TR Dequeue control dword");
        assert_eq!(trb.parameter, 0x1234_5681, "deq ptr | DCS=1");
        assert_eq!(trb.trb_type(), 16);
        // A non-aligned pointer has its low nibble cleared before OR-ing DCS.
        let trb2 = set_tr_dequeue_trb(SLOT, DCI, 0x1234_568F, false);
        assert_eq!(trb2.parameter, 0x1234_5680, "low nibble cleared, DCS=0");
    }

    #[test]
    fn ep_state_extractor() {
        assert_eq!(ep_state(0x0000_0002), EP_STATE_HALTED);
        assert_eq!(ep_state(0x0000_0001), EP_STATE_RUNNING);
        assert_eq!(ep_state(0xDEAD_BEF3), EP_STATE_STOPPED); // low 3 bits = 0b011
        assert_eq!(ep_state(0xFFFF_FFF8), EP_STATE_DISABLED); // low 3 bits = 0
    }

    #[test]
    fn recovery_action_killswitch_and_discriminator() {
        // Kill-switch disabled ⇒ Skip regardless of state (governance Rule 1:
        // disabled ≡ prior behavior, no command submitted).
        assert_eq!(
            recovery_action(false, EP_STATE_HALTED),
            RecoveryAction::Skip
        );
        assert_eq!(
            recovery_action(false, EP_STATE_RUNNING),
            RecoveryAction::Skip
        );
        // Enabled: Halted ⇒ Reset Endpoint; any other (stuck) state ⇒ Stop
        // (Reset Endpoint requires Halted or the xHC returns Context State Error).
        assert_eq!(
            recovery_action(true, EP_STATE_HALTED),
            RecoveryAction::Reset
        );
        assert_eq!(
            recovery_action(true, EP_STATE_RUNNING),
            RecoveryAction::Stop
        );
        assert_eq!(
            recovery_action(true, EP_STATE_STOPPED),
            RecoveryAction::Requeue
        );
    }
}
