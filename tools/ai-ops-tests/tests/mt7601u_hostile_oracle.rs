// SPDX-License-Identifier: MPL-2.0
//! Lane 3 (Cerebro) — the **hostile host oracle** for the MT7601U TEST 3
//! channel-7 trace, per `Plans/DISPATCH-2026-07-24-mt7601u-test3-parallel-fanout.md`
//! §"Lane 3" and the controlling fix-forward's required-repair property **C**.
//!
//! # What this file is, and what it is not
//!
//! The existing MT7601U host oracles in `src/lib.rs` are **per-decision** unit
//! tests: `classify_wpdma`, `charge_step`, `pass_eligible`, `inline_cal_bw`.
//! Each is correct in isolation. None of them models the **composed sequence**
//! that went RED on silicon, because the composition — "the tune failed, now
//! what?" — lives as imperative control flow inside the device-touching
//! `mt7601u::Device::scan`/`recover_chip`, where no host test can reach it.
//!
//! This file supplies the missing layer: a deterministic, device-free model of
//! the whole survey → wedge → recovery → terminal → evidence pipeline, driven
//! by a **scripted hostile device**, plus a **contract checker**
//! ([`verify_contract`]) and a **binding seam** ([`qualify_policy`]) that are
//! the reusable deliverables.
//!
//! ## The reference model alone is NOT candidate proof
//!
//! Read this before quoting any green result from this file.
//!
//! [`ReferencePolicy`] is an existence proof that the contract is *satisfiable*.
//! Running it through [`qualify_policy`] exercises **the harness**, not the
//! driver. It says nothing whatsoever about whether Jarvis's candidate is
//! correct. A candidate is qualified only when a [`WedgePolicy`] impl **backed
//! by the production decision function** is passed to [`qualify_policy`] and
//! reports `qualified()`. Until that binding exists, the honest claim is:
//!
//! > "The contract is stated, satisfiable, and has teeth. The candidate is
//! > unqualified against it."
//!
//! Not: "the oracle is green, so the driver is fine."
//!
//! Likewise a green [`qualify_policy`] on the bound production seam proves the
//! candidate satisfies **this contract on this modelled trace**. It is not a
//! substitute for the four-shard gate, the AAG panel, or silicon.
//!
//! # Terminology: evidence-neutral, not causal
//!
//! Nothing in the TEST 3 evidence establishes *why* the chip stopped answering.
//! The serial log shows an MCU **response timeout** on `RF_FREQPLAN`, then
//! bulk-out timeouts. "The MCU died" is an inference, and naming it that way in
//! a test suite launders a hypothesis into a fact — precisely the mistake the
//! controlling fix-forward warns against when it forbids reporting an unemitted
//! `fence_refusals` as zero.
//!
//! So this file names only what was observed: the **transport became
//! unresponsive**. `TransportUnresponsive`, `ClassifyUnresponsiveAndStop`,
//! `TransportUnresponsiveDuringSurvey`. Whether the silicon MCU is dead,
//! hung, holding a stalled endpoint, or waiting on a condition we never
//! satisfied is **an open question for Lane 2**, and the contract is
//! deliberately agnostic to the answer — it constrains what the driver may
//! *do* when the transport stops answering, not what caused it.
//!
//! # The scripted trace (from the cold PuTTY log, TEST 3, `2d43670e`)
//!
//! 1. channels 1..=6 tune and dwell; 8 BSSIDs found;
//! 2. channel 7 `RF_FREQPLAN` → MCU response timeout; the transport stops
//!    answering;
//! 3. host xHC endpoint recovery reports **success**; device
//!    `ClearFeature(ENDPOINT_HALT)` reports **success** — neither restores it;
//! 4. `vendor_reset` does **not** clear `COM_REG0` on this silicon, so the
//!    driver force-writes `COM_REG0 = 0` and the forced cold reload bulk-out
//!    times out at ILM byte `0/45380`.
//!
//! Points 3 and 4 are the hostile part: **every recovery primitive reports
//! success while the transport stays unresponsive.** A policy that trusts those
//! reports keeps issuing commands into a channel that is not answering.
//!
//! # Forced cold reload is outside the accepted contract
//!
//! The accepted contract permits exactly one bounded, **non-destructive** probe
//! (the host + device endpoint-clear pair, then one retry tune). It does **not**
//! permit a forced cold reload, and no green control in this file depends on
//! one. Rationale, in the fix-forward's own terms: the reload is destructive
//! (`vendor_reset` wipes chip state and `COM_REG0 = 0` overrides the shipped
//! resident-firmware branch), it is known on this silicon to stall at ILM byte
//! 0, and attempting it consumes the operation ledger that the terminal evidence
//! still needs. [`Violation::ForcedReloadAfterUnresponsive`] fires on **any**
//! reload issued after the transport went unresponsive — including one that
//! would have worked, because the driver cannot know in advance which world it
//! is in, and the losing branch strands the run with nothing true left to say.
//!
//! If Lane 2's upstream trap-mine establishes a *provably safe* reload
//! precondition, that is a contract amendment for the PM to ratify — not
//! something this oracle should silently permit.
//!
//! # Binding this to Jarvis's production seam
//!
//! [`qualify_policy`] is the single entry point. To bind:
//!
//! 1. Lift the recovery decision out of `Device::scan` — the
//!    `if let Err(reason) = tuned { … }` block — into a **pure, host-testable
//!    function** with no device access, alongside the existing
//!    `wedge_logic`/`post_fw_logic` cores. Suggested home: a
//!    `wedge_recovery_logic.rs` next to them, so `ai-ops-tests` can `#[path]`
//!    it exactly the way it already mounts `wedge_logic`.
//! 2. Path-`mod` it here and implement [`WedgePolicy`] for it — the trait is
//!    two methods and the observation struct carries every fact the decision
//!    needs (channel, observed failure, probes/reloads already spent, whether a
//!    reload already stalled at ILM byte 0, whether firmware still reports
//!    resident, remaining ledger).
//! 3. Call `qualify_policy(ProductionPolicy::default)` and assert
//!    `report.qualified()`.
//!
//! While that decision stays inline in `scan()`, no host oracle can assert on
//! it — which is the structural reason TEST 3's failure mode was observable
//! only on silicon.
//!
//! # Production code touched
//!
//! None. The only kernel file referenced is `wedge_logic.rs`, path-`mod`'d
//! read-only so the `CHANNEL=0` survey semantics and the resident-firmware
//! branch are asserted against the **real shipped policy** rather than a copy.

// The observation struct deliberately carries every fact a production policy
// might need, including ones the reference policy ignores — the surface is the
// contract, not the reference implementation's appetite.
#![allow(dead_code)]
#![allow(clippy::enum_glob_use)]

use std::collections::BTreeSet;

// The real shipped scan-route / firmware-branch policy. Read-only: this oracle
// asserts against it, it does not redefine it. Path is relative to this file's
// directory (`tools/ai-ops-tests/tests/`), so `../../../` is the repo root.
#[path = "../../../kernel/src/drivers/usb/mt7601u/linux_parity_logic.rs"]
mod linux_parity_logic;
#[path = "../../../kernel/src/drivers/usb/mt7601u/wedge_logic.rs"]
mod wedge_logic;

// ───────────────────────────────────────────────────────────────────────────
// Operation-wide budget
//
// A deliberately API-compatible stand-in for
// `kernel/src/drivers/usb/operation_budget_logic.rs::BudgetModel`, which does
// not exist on `origin/main` (it arrives with Jarvis's candidate). Keeping this
// self-contained is what makes the commit cherry-pick onto any branch. When it
// lands next to the real module, swap the three call sites — `new`, `charge`,
// `remaining`/`exhausted` have matching semantics: one non-replenishing ledger,
// `charge` returns false exactly once the ledger is spent.
// ───────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpBudget {
    limit: u64,
    remaining: u64,
    exhausted: bool,
}

impl OpBudget {
    const fn new(limit: u64) -> Self {
        Self {
            limit,
            remaining: limit,
            exhausted: false,
        }
    }

    /// Consume `units`. Never replenishes; latches `exhausted` on the first
    /// charge it cannot satisfy in full.
    fn charge(&mut self, units: u64) -> bool {
        if self.remaining < units {
            self.remaining = 0;
            self.exhausted = true;
            return false;
        }
        self.remaining -= units;
        true
    }

    const fn spent(&self) -> u64 {
        self.limit - self.remaining
    }
}

/// One non-replenishing ledger for the whole post-firmware control plane.
const POST_FW_OPERATION_BUDGET: u64 = 600;

// Per-operation structural costs. These mirror the *shape* of the driver's
// nested charging (vendor attempt + wait call + poll gap per register op), not
// its absolute constants; the oracle asserts exact totals so a change in
// composition is a test failure rather than a silent drift.
const COST_TUNE: u64 = 6; // RF_FREQPLAN + LNA + VCO + TX-power + 2 polls
const COST_DWELL: u64 = 1; // event poll during the dwell
const COST_HOST_EP_RECOVER: u64 = 2;
const COST_DEVICE_CLEAR_HALT: u64 = 2;
/// One idempotent no-op `CMD_FUN_SET_OP` issued to retire an accumulated MCU
/// response slot (Q-B step 2). Priced per command; the drain issues one per
/// prior hop.
const COST_MCU_DRAIN_CMD: u64 = 2;
/// The single response-bearing command whose decoded `CMD_DONE` is the only
/// accepted proof that the command path is usable again (Q-B step 3).
const COST_COMMAND_PATH_PROBE: u64 = 2;
const COST_VENDOR_RESET: u64 = 4;
const COST_WRITE_REG: u64 = 2;
const COST_FW_PRELOAD_SETUP: u64 = 12;
/// A bulk-out that times out burns the whole scoped bulk-poll budget.
const COST_BULK_OUT_TIMEOUT: u64 = 96;
/// `load_firmware` retries the upload three times (the `fw a{1,2,3}` breadcrumbs).
const FW_UPLOAD_ATTEMPTS: u64 = 3;

const COST_ASSOC_ATTEMPT: u64 = 8;
const COST_DHCP_ATTEMPT: u64 = 8;

/// The ILM image size the cold reload stalls on, verbatim from the serial log.
const ILM_TOTAL_BYTES: usize = 45_380;

/// 2.4 GHz regulatory sweep, matching `ee.reg_start`/`ee.reg_num`.
const REG_START: u8 = 1;
const REG_NUM: u8 = 14;

/// Hard cap on policy iterations per failed channel. Exceeding it means the
/// policy does not terminate — reported as [`Violation::Unbounded`] rather than
/// hanging the test suite.
const POLICY_STEP_CAP: u32 = 16;

/// Once the transport stops answering, the accepted contract allows exactly one
/// bounded **non-destructive** recovery. The allowance is derived, not fudged —
/// it is the Q-B sequence and nothing else.
///
/// It grew from 3 to 5 when the drain and the command-path probe were bound
/// into this oracle. That is a widening of a contract bound, so it is spelled
/// out term by term: the two extra ops are the two Q-B steps that make a retry
/// *safe*, and both are themselves bounded — the drain is clamped to
/// [`wedge_logic::MAX_MCU_DRAIN_COMMANDS`] commands (compile-asserted in the
/// driver against the operation ledger) and the probe is exactly one command.
/// A forced cold reload is still deliberately NOT in this budget — see the
/// module docs and [`Violation::ForcedReloadAfterUnresponsive`].
const MAX_POST_UNRESPONSIVE_DEVICE_OPS: usize = 2 // host + device endpoint clear
    + 1 // Q-B step 2: the bounded MCU queue drain
    + 1 // Q-B step 3: the command-path probe
    + 1; // the retry tune that a *re-proven* command path earns

// ───────────────────────────────────────────────────────────────────────────
// Device model
// ───────────────────────────────────────────────────────────────────────────

/// One recorded device interaction. `touches_device` is the discriminator the
/// "no commands into an unresponsive transport" clause is evaluated against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Op {
    Tune {
        channel: u8,
    },
    Dwell {
        channel: u8,
    },
    HostEndpointRecover,
    DeviceClearHalt,
    VendorReset,
    WriteComReg0 {
        value: u32,
    },
    FwPreloadSetup {
        attempt: u64,
    },
    FwUploadChunk {
        offset: usize,
    },
    AssociationAttempt,
    DhcpAttempt,
    /// Q-B step 2: `commands` idempotent no-op `CMD_FUN_SET_OP`s, one per prior
    /// hop, issued to retire accumulated MCU response slots.
    McuQueueDrain {
        commands: u8,
    },
    /// Q-B step 3: the one response-bearing command that must decode `CMD_DONE`
    /// before any retry may be authorized.
    CommandPathProbe,
    /// Host-local: writing a breadcrumb costs no bus traffic.
    EmitEvidence,
}

impl Op {
    const fn touches_device(self) -> bool {
        !matches!(self, Op::EmitEvidence)
    }
}

/// Observed failures only — no causal claims. `McuRespTimeout` names what the
/// serial log actually recorded; `TransportUnresponsive` means "we issued a
/// command and the transport did not answer", not "the chip is dead".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeviceError {
    /// `RF_FREQPLAN` issued, no MCU response before the scoped budget expired.
    McuRespTimeout,
    /// A command issued while the transport is known not to be answering.
    TransportUnresponsive,
    /// Bulk-out stalled; `uploaded`/`total` are the ILM progress breadcrumb.
    BulkOutTimeout { uploaded: usize, total: usize },
    /// The composed operation consumed its non-replenishing ledger.
    OperationBudgetExhausted,
}

/// Result of one host-side xHC endpoint recovery / device clear-halt. The
/// hostile point: `reported_ok` is `true` on the real silicon while the
/// transport stays unresponsive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EndpointRecovery {
    reported_ok: bool,
}

/// What, if anything, actually restores the transport. Reporting success is not
/// restoring it — that distinction is the whole hostility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WedgeClearedBy {
    /// Nothing available to the driver. The TEST 3 silicon: nothing the driver
    /// tried, including the full host + device reset and a cold reload,
    /// restored it.
    Nothing,
    /// A genuine transient bulk-endpoint stall — the non-destructive
    /// host + device clear-halt pair restores it.
    EndpointClear,
    /// The `cea89e6a` state: the endpoint clear restores the **transport** —
    /// the host rings and device halts genuinely recover and the probe reports
    /// success — but the **MCU wedge stays latched**, so re-issuing the tune
    /// wedges again. This is the world in which "endpoint recovery reported ok"
    /// is a lie about command-path liveness.
    TransportOnlyEndpointClear,
    /// Only a forced cold firmware reload restores it. Used exclusively to
    /// prove the contract still rejects the reload in a world where it *would*
    /// have worked — never as a green path.
    ColdReload,
}

/// Knobs describing which physical world the device lives in. `test3()` is the
/// observed silicon; the others are controls that keep the oracle from
/// overfitting to "always give up".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HostileScript {
    /// Channel whose `RF_FREQPLAN` wedges, if any.
    wedge_channel: Option<u8>,
    /// Host xHC endpoint reset reports success (it did, on silicon).
    host_recovery_reports_ok: bool,
    /// Device `ClearFeature(ENDPOINT_HALT)` reports success (it did).
    device_clear_halt_reports_ok: bool,
    /// What genuinely restores the transport. `Nothing` on the TEST 3 silicon.
    wedge_cleared_by: WedgeClearedBy,
    /// `vendor_reset` clears `COM_REG0`. **False** on this silicon — which is
    /// precisely why the rejected model force-writes it to 0.
    vendor_reset_clears_com_reg0: bool,
    /// The raw NVMe evidence sink is installed and writable at USB **bind**
    /// time. **False** in the rejected system: the sink is installed after USB
    /// init, so a bind-time scan failure never reaches it (the all-zero 16 MiB
    /// sink paired with a substantial serial log).
    sink_available_at_bind: bool,
    /// Channels on which the configured target SSID is actually broadcasting.
    target_channels: &'static [u8],
    /// Whether the MCU answers the Q-B command-path probe with a decoded
    /// `CMD_DONE` once the transport is answering again.
    ///
    /// This is the axis that separates *transport recovered* from *command path
    /// re-proven*. `cea89e6a` hung precisely because those were conflated: the
    /// endpoint probe reported success while the MCU stayed latched, and the
    /// driver re-issued the identical RF17-20 write anyway.
    mcu_command_path_reproves: bool,
    /// Whether the drain no-ops actually retire queued response slots. Purely
    /// diagnostic — a drain never authorizes a retry on its own.
    mcu_drain_retires_slots: bool,
}

impl HostileScript {
    /// The observed TEST 3 cold boot.
    const fn test3() -> Self {
        Self {
            wedge_channel: Some(7),
            host_recovery_reports_ok: true,
            device_clear_halt_reports_ok: true,
            wedge_cleared_by: WedgeClearedBy::Nothing,
            vendor_reset_clears_com_reg0: false,
            sink_available_at_bind: false,
            target_channels: &[11],
            // On TEST 3 silicon nothing restored the transport, so the probe
            // could never have decoded a CMD_DONE either way.
            mcu_command_path_reproves: true,
            mcu_drain_retires_slots: true,
        }
    }

    /// The `cea89e6a` shape, isolated: the endpoint probe reports success and
    /// the transport answers again, but the **MCU command path stays latched**
    /// and never decodes a `CMD_DONE`.
    ///
    /// This is the script that makes the retry-authorization seam load-bearing.
    /// Any policy that treats endpoint recovery as permission to retry will
    /// re-issue the identical write into a latched MCU here.
    const fn transport_up_mcu_latched() -> Self {
        Self {
            wedge_cleared_by: WedgeClearedBy::TransportOnlyEndpointClear,
            sink_available_at_bind: true,
            mcu_command_path_reproves: false,
            mcu_drain_retires_slots: false,
            ..Self::test3()
        }
    }

    /// Control: no wedge at all. Proves the oracle is not vacuously red.
    const fn healthy() -> Self {
        Self {
            wedge_channel: None,
            sink_available_at_bind: true,
            ..Self::test3()
        }
    }

    /// Control: a genuine transient endpoint stall — the bounded non-destructive
    /// probe *does* fix it. Proves the contract does not mandate giving up.
    const fn transient_endpoint_stall() -> Self {
        Self {
            wedge_cleared_by: WedgeClearedBy::EndpointClear,
            sink_available_at_bind: true,
            ..Self::test3()
        }
    }

    /// Adversarial world in which a forced reload *would* have restored the
    /// transport. Not a green control — it exists to show the contract rejects
    /// the reload even here, because the driver cannot tell this world from
    /// TEST 3's in advance.
    const fn reload_would_have_worked() -> Self {
        Self {
            wedge_cleared_by: WedgeClearedBy::ColdReload,
            sink_available_at_bind: true,
            ..Self::test3()
        }
    }
}

/// How many BSSIDs each channel yields. Channels 1..=6 carry the eight BSSIDs
/// the cold run actually found before the wedge; channel 11 carries the
/// configured target, which the wedged run never reaches.
const fn bssids_on(channel: u8) -> usize {
    match channel {
        1 => 2,
        6 => 3,
        2..=4 => 1,
        11 => 1,
        _ => 0,
    }
}

/// BSSIDs discoverable before the channel-7 wedge.
const PRE_WEDGE_BSSIDS: usize = 8;
/// BSSIDs discoverable by a complete 1..=14 survey.
const FULL_SURVEY_BSSIDS: usize = 9;

struct HostileDevice {
    script: HostileScript,
    /// The transport is answering commands. Not a claim about chip liveness.
    transport_responsive: bool,
    /// The wedge is still latched in silicon; tuning the wedge channel while
    /// armed makes the transport stop answering again.
    wedge_armed: bool,
    com_reg0: u32,
    ops: Vec<Op>,
    budget: OpBudget,
    fence_refusals: u32,
    /// Index into `ops` of the FIRST unresponsive observation. Never cleared —
    /// governs the "no forced reload once it stopped answering" clause, which
    /// must hold even if the transport later comes back.
    first_unresponsive_op_index: Option<usize>,
    /// Index into `ops` of an unresponsive episode the run never recovered
    /// from. Cleared when the transport answers again — a channel that came
    /// back is not an unrecovered failure, and the terminal/association clauses
    /// must not fire on a survivable hiccup.
    unrecovered_unresponsive_op_index: Option<usize>,
    /// Total unresponsive episodes, recovered or not (reporting only).
    unresponsive_episodes: usize,
}

impl HostileDevice {
    fn new(script: HostileScript) -> Self {
        Self {
            script,
            transport_responsive: true,
            wedge_armed: true,
            com_reg0: 1, // firmware resident after the cold upload
            ops: Vec::new(),
            budget: OpBudget::new(POST_FW_OPERATION_BUDGET),
            fence_refusals: 0,
            first_unresponsive_op_index: None,
            unrecovered_unresponsive_op_index: None,
            unresponsive_episodes: 0,
        }
    }

    /// Record `op` and charge `units`. Recording happens *before* the budget
    /// check so a budget-exhausted attempt still shows up as attempted traffic.
    fn issue(&mut self, op: Op, units: u64) -> Result<(), DeviceError> {
        self.ops.push(op);
        if !self.budget.charge(units) {
            return Err(DeviceError::OperationBudgetExhausted);
        }
        Ok(())
    }

    fn note_unresponsive(&mut self) {
        if self.transport_responsive {
            self.transport_responsive = false;
            self.unresponsive_episodes += 1;
            let at = self.ops.len().saturating_sub(1);
            self.unrecovered_unresponsive_op_index = Some(at);
            if self.first_unresponsive_op_index.is_none() {
                self.first_unresponsive_op_index = Some(at);
            }
        }
    }

    /// The transport answered again. Disarm the wedge and clear the
    /// *unrecovered* marker — but never `first_unresponsive_op_index`.
    fn note_responsive_again(&mut self) {
        self.transport_responsive = true;
        self.wedge_armed = false;
        self.unrecovered_unresponsive_op_index = None;
    }

    /// The transport answers again but the MCU wedge stays **armed** — so a
    /// tune re-issued on the wedge channel wedges a second time. This is the
    /// state that makes "endpoint recovery reported ok" insufficient.
    fn note_transport_only_recovered(&mut self) {
        self.transport_responsive = true;
        self.unrecovered_unresponsive_op_index = None;
    }

    fn tune(&mut self, channel: u8) -> Result<(), DeviceError> {
        self.issue(Op::Tune { channel }, COST_TUNE)?;
        if !self.transport_responsive {
            return Err(DeviceError::TransportUnresponsive);
        }
        if self.wedge_armed && self.script.wedge_channel == Some(channel) {
            self.note_unresponsive();
            return Err(DeviceError::McuRespTimeout);
        }
        Ok(())
    }

    fn dwell(&mut self, channel: u8) -> Result<usize, DeviceError> {
        self.issue(Op::Dwell { channel }, COST_DWELL)?;
        Ok(bssids_on(channel))
    }

    fn recover_host_endpoints(&mut self) -> Result<EndpointRecovery, DeviceError> {
        self.issue(Op::HostEndpointRecover, COST_HOST_EP_RECOVER)?;
        Ok(EndpointRecovery {
            reported_ok: self.script.host_recovery_reports_ok,
        })
    }

    fn clear_device_halt(&mut self) -> Result<EndpointRecovery, DeviceError> {
        self.issue(Op::DeviceClearHalt, COST_DEVICE_CLEAR_HALT)?;
        match self.script.wedge_cleared_by {
            WedgeClearedBy::EndpointClear => self.note_responsive_again(),
            WedgeClearedBy::TransportOnlyEndpointClear => self.note_transport_only_recovered(),
            _ => {}
        }
        Ok(EndpointRecovery {
            reported_ok: self.script.device_clear_halt_reports_ok,
        })
    }

    /// Q-B step 2 — issue `depth` idempotent no-op commands to retire queued
    /// MCU response slots. Returns how many decoded a `CMD_DONE`.
    ///
    /// Best-effort by construction: the count is diagnostic evidence only and
    /// never authorizes anything. A drain that retires every slot on a device
    /// whose command path is still latched must not produce a retry.
    fn drain_mcu_command_queue(&mut self, depth: u8) -> Result<u8, DeviceError> {
        if depth == 0 {
            return Ok(0);
        }
        self.issue(
            Op::McuQueueDrain { commands: depth },
            COST_MCU_DRAIN_CMD * depth as u64,
        )?;
        if self.transport_responsive && self.script.mcu_drain_retires_slots {
            Ok(depth)
        } else {
            Ok(0)
        }
    }

    /// Q-B step 3 — the command-path proof. Returns whether a `CMD_DONE` was
    /// decoded for the awaited sequence number.
    ///
    /// A completed transfer is not a proof; only the decode is. A device whose
    /// transport answers but whose MCU stays latched returns `false` here, and
    /// that is exactly the case the retry authorization must refuse.
    fn prove_command_path(&mut self) -> Result<bool, DeviceError> {
        self.issue(Op::CommandPathProbe, COST_COMMAND_PATH_PROBE)?;
        if !self.transport_responsive {
            return Ok(false);
        }
        if self.script.mcu_command_path_reproves {
            self.note_responsive_again();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// The destructive path: vendor reset, clear both halts, force `COM_REG0=0`
    /// so `firmware_load_action` is overridden into a cold upload, then upload.
    /// Modelled so the rejected behaviour can be replayed and refuted — it is
    /// **not** part of the accepted contract.
    fn forced_cold_reload(&mut self) -> Result<(), DeviceError> {
        self.issue(Op::VendorReset, COST_VENDOR_RESET)?;
        if self.script.vendor_reset_clears_com_reg0 {
            self.com_reg0 = 0;
        }
        self.issue(Op::DeviceClearHalt, COST_DEVICE_CLEAR_HALT)?;
        self.issue(Op::DeviceClearHalt, COST_DEVICE_CLEAR_HALT)?;
        // The override: a wedged device still reports resident firmware, so the
        // rejected model zeroes COM_REG0 to defeat `firmware_load_action`'s
        // resync branch and force a cold upload.
        self.issue(Op::WriteComReg0 { value: 0 }, COST_WRITE_REG)?;
        self.com_reg0 = 0;

        for attempt in 1..=FW_UPLOAD_ATTEMPTS {
            self.issue(Op::FwPreloadSetup { attempt }, COST_FW_PRELOAD_SETUP)?;
            self.issue(Op::FwUploadChunk { offset: 0 }, COST_BULK_OUT_TIMEOUT)?;
            if self.script.wedge_cleared_by == WedgeClearedBy::ColdReload {
                self.note_responsive_again();
                self.com_reg0 = 1;
                return Ok(());
            }
        }
        Err(DeviceError::BulkOutTimeout {
            uploaded: 0,
            total: ILM_TOTAL_BYTES,
        })
    }

    fn attempt_association(&mut self, found_target: bool) -> Result<(), DeviceError> {
        self.issue(Op::AssociationAttempt, COST_ASSOC_ATTEMPT)?;
        if !self.transport_responsive || !found_target {
            return Err(DeviceError::TransportUnresponsive);
        }
        Ok(())
    }

    fn attempt_dhcp(&mut self) -> Result<[u8; 4], DeviceError> {
        self.issue(Op::DhcpAttempt, COST_DHCP_ATTEMPT)?;
        if !self.transport_responsive {
            return Err(DeviceError::TransportUnresponsive);
        }
        Ok([192, 168, 1, 8])
    }
}

// ───────────────────────────────────────────────────────────────────────────
// The policy surface — the decision Jarvis must lift out of `scan()`
// ───────────────────────────────────────────────────────────────────────────

/// Everything the driver knows at the moment a tune fails. Deliberately
/// includes the *reported* recovery results, because the whole hazard is that
/// they say "ok" while the transport is not answering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WedgeObservation {
    channel: u8,
    failure: DeviceError,
    /// Non-destructive endpoint probes already spent on this wedge.
    endpoint_probes_used: u32,
    /// Forced cold reloads already spent on this wedge. A conforming policy
    /// keeps this at zero; the field exists so a candidate that *does* reload
    /// can be modelled and refuted rather than being unrepresentable.
    reloads_used: u32,
    /// Whether a previous reload stalled at ILM byte 0.
    reload_stalled_at_byte_zero: bool,
    /// The device still claims resident firmware (`COM_REG0 == 1`).
    firmware_reports_resident: bool,
    budget_remaining: u64,
    /// Headroom a policy should leave so terminal evidence stays affordable.
    terminal_evidence_reserve: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WedgeAction {
    /// Non-destructive: clear host + device endpoint halts, retry the tune.
    ClearEndpointsAndRetry,
    /// Destructive: vendor reset, force `COM_REG0=0`, full cold reload.
    /// **Outside the accepted contract** — modelled so it can be refuted.
    ForceColdReload,
    /// Abandon this channel; keep surveying (the transport is answering).
    SkipChannel,
    /// Classify the transport unresponsive: stop **all** device traffic and
    /// terminate the survey with a truthful terminal category.
    ClassifyUnresponsiveAndStop,
}

/// The decision surface a production driver must expose to be oracle-able.
///
/// Jarvis's binding step: lift the `if let Err(reason) = tuned { … }` block out
/// of `Device::scan` into a pure function with this shape, implement this trait
/// for it, and hand it to [`qualify_policy`].
/// What the non-destructive recovery sequence actually observed, after the
/// endpoint probe, the MCU queue drain, and the command-path probe have run.
///
/// This is the input to the **retry authorization** — the decision that
/// `cea89e6a` got wrong. Drain counts are carried because a real driver logs
/// them as discriminators, not because they may authorize anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecoveryObservation {
    /// Host ring restart + device clear-halt both reported success. Transport
    /// evidence only — never MCU liveness.
    endpoint_probe_reported_ok: bool,
    drain_commands_issued: u8,
    drain_cmd_done_observed: u8,
    /// The probe command completed its transfer at all.
    probe_command_completed: bool,
    /// The probe's response decoded as the awaited seq + `CMD_DONE`.
    probe_cmd_done_observed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetryAuthorization {
    /// The command path is re-proven; re-issuing the tune is permitted.
    Retry,
    /// Not re-proven. No further command may be issued on this path.
    RefuseAndClassifyUnusable,
}

trait WedgePolicy {
    fn on_tune_failure(&mut self, obs: &WedgeObservation) -> WedgeAction;
    /// The second half of the recovery decision: having entered recovery and
    /// run it, may the tune actually be re-issued?
    ///
    /// Deliberately a **required** method with no default. A policy that does
    /// not state its authorization behaviour cannot be qualified, because this
    /// is the seam with a known silicon hang behind it.
    fn authorize_retry(&mut self, obs: &RecoveryObservation) -> RetryAuthorization;
    /// Name for failure messages.
    fn name(&self) -> &'static str;
}

/// **A reviewed model of the rejected behaviour — NOT source-extracted code.**
///
/// This is hand-authored by Cerebro from two untrusted diagnostic inputs: the
/// controlling fix-forward's prose description of what the candidate does after
/// an MCU timeout, and the cold PuTTY log's ordering of breadcrumbs. **No line
/// of `2d43670e` was copied, decompiled, or mechanically derived**, and this
/// file does not compile against the candidate.
///
/// Consequence for how its result may be quoted: the negative control proves
/// *this modelled behaviour* violates the contract. Treating that as a finding
/// **about the candidate** requires someone — Jarvis, who owns the source, or
/// the PM — to first confirm the model faithfully describes
/// `Device::scan`/`recover_chip` at `2d43670e`. Specifically that:
///
/// * a tune failure triggers the full destructive recovery
///   (`recover_chip` → `vendor_reset` → `wr(MCU_COM_REG0, 0)` → `bring_up`);
/// * that recovery is bounded only by an attempt counter
///   (`MAX_SCAN_RECOVERY_ATTEMPTS = 2`) and not by an unresponsive
///   classification;
/// * no path classifies the transport unresponsive and stops issuing commands.
///
/// If any of those is wrong, this model is wrong and its violation set says
/// nothing about the candidate — only about the contract's teeth.
#[derive(Debug, Default)]
struct RejectedBehaviourModel {
    recovery_attempts: u32,
}

impl WedgePolicy for RejectedBehaviourModel {
    fn on_tune_failure(&mut self, _obs: &WedgeObservation) -> WedgeAction {
        // Modelled from the described `MAX_SCAN_RECOVERY_ATTEMPTS = 2`.
        if self.recovery_attempts >= 2 {
            return WedgeAction::SkipChannel;
        }
        self.recovery_attempts += 1;
        WedgeAction::ForceColdReload
    }

    /// The rejected behaviour, faithfully: retry on the strength of endpoint
    /// recovery alone, never consulting whether the MCU command path re-proved.
    /// This is the `cea89e6a` mistake, and it is here so the contract can
    /// refute it rather than leaving it unrepresentable.
    fn authorize_retry(&mut self, _obs: &RecoveryObservation) -> RetryAuthorization {
        RetryAuthorization::Retry
    }

    fn name(&self) -> &'static str {
        "rejected-model(2d43670e, PM-described — NOT source-extracted)"
    }
}

/// One existence proof that the contract is satisfiable. **Not** a prescribed
/// repair, and **not** evidence about any candidate — see the module docs.
///
/// The rule set, entirely non-destructive:
/// * one host + device endpoint-clear probe, then one retry tune;
/// * if the transport still does not answer, classify unresponsive and stop.
///
/// No forced cold reload, ever.
#[derive(Debug, Default)]
struct ReferencePolicy;

/// Headroom reserved so terminal evidence is always affordable.
const TERMINAL_EVIDENCE_RESERVE: u64 = 32;

impl WedgePolicy for ReferencePolicy {
    fn on_tune_failure(&mut self, obs: &WedgeObservation) -> WedgeAction {
        if obs.endpoint_probes_used == 0
            && obs.budget_remaining
                >= COST_HOST_EP_RECOVER
                    + COST_DEVICE_CLEAR_HALT
                    + COST_TUNE
                    + obs.terminal_evidence_reserve
        {
            return WedgeAction::ClearEndpointsAndRetry;
        }
        WedgeAction::ClassifyUnresponsiveAndStop
    }

    /// Hand-written and deliberately **not** calling the production helper —
    /// that is what keeps this an independent existence proof rather than a
    /// tautology. It states the same rule in its own terms: only a decoded
    /// `CMD_DONE` authorizes a retry; endpoint recovery and a fully-retired
    /// drain do not.
    fn authorize_retry(&mut self, obs: &RecoveryObservation) -> RetryAuthorization {
        if obs.endpoint_probe_reported_ok
            && obs.probe_command_completed
            && obs.probe_cmd_done_observed
        {
            RetryAuthorization::Retry
        } else {
            RetryAuthorization::RefuseAndClassifyUnusable
        }
    }

    fn name(&self) -> &'static str {
        "reference (harness proof only — not candidate proof)"
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Terminal categories and the evidence path
// ───────────────────────────────────────────────────────────────────────────

/// Adapter backed by the two pure decision seams used by the shipped channel
/// failure path. The hostile runner scales hardware operation units down to
/// model units, so the reserve supplied to the production decision is derived
/// from the model's actual probe, retry, and terminal-evidence costs.
#[derive(Debug, Default)]
struct ProductionPolicy;

impl WedgePolicy for ProductionPolicy {
    fn on_tune_failure(&mut self, obs: &WedgeObservation) -> WedgeAction {
        let probe_and_retry_reserve = COST_HOST_EP_RECOVER
            + COST_DEVICE_CLEAR_HALT
            + COST_TUNE
            + obs.terminal_evidence_reserve;
        let action = wedge_logic::channel_failure_action(wedge_logic::ChannelFailureObservation {
            endpoint_probes_used: u8::try_from(obs.endpoint_probes_used).unwrap_or(u8::MAX),
            operation_remaining: obs.budget_remaining,
            probe_and_retry_reserve,
        });

        match action {
            // NOTE: entering recovery is not permission to retry. The retry is
            // authorized separately in `authorize_retry`, which binds the
            // production `channel_retry_action(recovery_retry_observation(..))`.
            wedge_logic::ChannelFailureAction::ProbeEndpointsAndRetry => {
                WedgeAction::ClearEndpointsAndRetry
            }
            wedge_logic::ChannelFailureAction::ClassifyCommandPathUnusable => {
                let disposition = wedge_logic::channel_fault_disposition();
                if disposition.attempt_destructive_reload {
                    return WedgeAction::ForceColdReload;
                }
                if disposition.command_path == wedge_logic::ChannelCommandPathState::Unusable
                    && disposition.terminal
                        == wedge_logic::ChannelTerminal::ChannelCommandPathUnusable
                    && !disposition.continue_survey
                    && !disposition.allow_association
                    && !disposition.allow_dhcp
                    && !disposition.allow_link_up
                {
                    WedgeAction::ClassifyUnresponsiveAndStop
                } else {
                    WedgeAction::SkipChannel
                }
            }
        }
    }

    /// The binding this AAG bounce required: the retry authorization is the
    /// shipped `channel_retry_action(recovery_retry_observation(..))`, consumed
    /// from the same read-only `#[path]`-mod'd `wedge_logic` the driver calls.
    /// Production drives the identical pair at `mt7601u/mod.rs`
    /// (`set_channel_accounted`, the `ProbeEndpointsAndRetry` arm).
    fn authorize_retry(&mut self, obs: &RecoveryObservation) -> RetryAuthorization {
        let action = wedge_logic::channel_retry_action(wedge_logic::recovery_retry_observation(
            wedge_logic::ChannelRecoveryObservation {
                endpoint_probe_reported_ok: obs.endpoint_probe_reported_ok,
                drain_commands_issued: obs.drain_commands_issued,
                drain_cmd_done_observed: obs.drain_cmd_done_observed,
                probe_command_completed: obs.probe_command_completed,
                probe_cmd_done_observed: obs.probe_cmd_done_observed,
            },
        ));
        match action {
            wedge_logic::ChannelRetryAction::Retry => RetryAuthorization::Retry,
            wedge_logic::ChannelRetryAction::ClassifyCommandPathUnusable => {
                RetryAuthorization::RefuseAndClassifyUnusable
            }
        }
    }

    fn name(&self) -> &'static str {
        "production channel failure policy"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Terminal {
    Pass {
        assigned_ip: [u8; 4],
    },
    /// The truthful category for this trace: the transport stopped answering
    /// mid-survey, so the survey is **truncated**, not complete. Deliberately
    /// says nothing about cause.
    TransportUnresponsiveDuringSurvey {
        last_channel: u8,
        channels_surveyed: u8,
        bss_found: usize,
    },
    /// The AP was found and the survey completed, but association failed.
    AssociationFailed {
        bss_found: usize,
    },
    DhcpTimeout,
    /// A *complete* survey found no target. Claiming this after a truncated
    /// survey is a lie about coverage.
    ApNotFound {
        channels_surveyed: u8,
    },
    /// The ledger ran out before any of the above could be determined.
    OperationBudgetExhausted,
}

impl Terminal {
    const fn is_transport_unresponsive(self) -> bool {
        matches!(self, Terminal::TransportUnresponsiveDuringSurvey { .. })
    }

    const fn claims_pass(self) -> bool {
        matches!(self, Terminal::Pass { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SinkRecord {
    Terminal(Terminal),
    BudgetSummary {
        limit: u64,
        remaining: u64,
        exhausted: bool,
    },
    FenceSummary {
        fence_refusals: u32,
    },
}

/// Serial and the raw NVMe sink are two independent channels. The controlling
/// fix-forward's rule: a zero sink paired with a non-empty serial log is RED.
#[derive(Debug, Default)]
struct Evidence {
    serial: Vec<String>,
    sink: Vec<SinkRecord>,
    sink_installed: bool,
}

impl Evidence {
    fn breadcrumb(&mut self, line: String) {
        self.serial.push(line);
    }

    /// Terminal evidence goes to *both* channels — but only reaches the sink if
    /// the sink is actually installed at this point in bring-up.
    fn terminal(&mut self, record: SinkRecord) {
        self.serial.push(format!("{record:?}"));
        if self.sink_installed {
            self.sink.push(record);
        }
    }

    fn sink_terminal(&self) -> Option<Terminal> {
        self.sink.iter().find_map(|r| match r {
            SinkRecord::Terminal(t) => Some(*t),
            _ => None,
        })
    }

    fn has_budget_summary(&self) -> bool {
        self.sink
            .iter()
            .any(|r| matches!(r, SinkRecord::BudgetSummary { .. }))
    }

    fn has_fence_summary(&self) -> bool {
        self.sink
            .iter()
            .any(|r| matches!(r, SinkRecord::FenceSummary { .. }))
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Trace + contract checker (the reusable deliverable)
// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Trace {
    policy: &'static str,
    ops: Vec<Op>,
    /// First unresponsive observation; never cleared by a later recovery.
    first_unresponsive_op_index: Option<usize>,
    /// An unresponsive episode the run never recovered from. `None` when the
    /// transport answered again.
    unrecovered_unresponsive_op_index: Option<usize>,
    /// Episodes observed in total, recovered or not.
    unresponsive_episodes: usize,
    /// Index into `ops` at which the policy classified the transport
    /// unresponsive.
    classified_unresponsive_op_index: Option<usize>,
    /// Every terminal category the run emitted. Must be exactly one.
    terminals: Vec<Terminal>,
    channels_surveyed: u8,
    bss_found: usize,
    target_found: bool,
    association_attempted: bool,
    association_succeeded: bool,
    dhcp_attempted: bool,
    dhcp_succeeded: bool,
    link_up: bool,
    /// True iff the policy loop terminated within [`POLICY_STEP_CAP`].
    bounded: bool,
    budget: OpBudget,
    fence_refusals: u32,
    evidence: Evidence,
}

impl Trace {
    fn device_ops_after(&self, index: usize) -> usize {
        self.ops
            .iter()
            .skip(index + 1)
            .filter(|op| op.touches_device())
            .count()
    }

    fn forced_reload_count(&self) -> usize {
        self.ops
            .iter()
            .filter(|op| matches!(op, Op::WriteComReg0 { value: 0 }))
            .count()
    }

    fn forced_reloads_after(&self, index: usize) -> usize {
        self.ops
            .iter()
            .skip(index + 1)
            .filter(|op| matches!(op, Op::WriteComReg0 { value: 0 }))
            .count()
    }

    fn terminal(&self) -> Option<Terminal> {
        self.terminals.first().copied()
    }
}

/// Every way a run can breach the Lane-3 contract. Ordered so the asserted
/// violation sets read deterministically.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Violation {
    /// The policy loop never terminated.
    Unbounded,
    NoTerminalCategory,
    MultipleTerminalCategories,
    /// The transport stopped answering, but the terminal blames something else
    /// (association, a "complete" survey that found nothing, a budget).
    UntruthfulTerminal,
    /// Commands kept flowing after the policy classified the transport
    /// unresponsive.
    TrafficAfterClassifiedUnresponsive,
    /// Traffic after an unrecovered unresponsive observation exceeded the
    /// bounded non-destructive probe allowance.
    UnboundedTrafficAfterUnresponsive,
    AssociationAttemptedAfterUnresponsive,
    DhcpAttemptedAfterUnresponsive,
    LinkUpClaimedAfterUnresponsive,
    PassClaimedAfterUnresponsive,
    /// **Any** forced cold reload issued after the transport stopped answering.
    /// Outside the accepted contract even when it would have succeeded — see
    /// the module docs.
    ForcedReloadAfterUnresponsive,
    /// More than one forced cold reload — the futile-retry signature.
    RepeatedFutileReload,
    /// The ledger was spent before a terminal category could be produced.
    BudgetExhaustedBeforeTerminal,
    /// Serial has content but the raw sink is empty — the observed TEST 3 split.
    SinkEmptyWhileSerialNonEmpty,
    TerminalMissingFromSink,
    BudgetSummaryMissingFromSink,
    FenceSummaryMissingFromSink,
}

/// **The deliverable.** The complete Lane-3 contract, evaluated against a trace.
///
/// Returns `Ok(())` only when every clause of the dispatch's Lane-3 bullet list
/// holds. Bind a candidate to it via [`qualify_policy`].
fn verify_contract(t: &Trace) -> Result<(), Vec<Violation>> {
    use Violation::*;
    let mut v = Vec::new();

    // 1. Bounded completion.
    if !t.bounded {
        v.push(Unbounded);
    }

    // 2. Exactly one terminal category, and it must be truthful.
    match t.terminals.len() {
        0 => v.push(NoTerminalCategory),
        1 => {}
        _ => v.push(MultipleTerminalCategories),
    }
    if t.unrecovered_unresponsive_op_index.is_some()
        && let Some(term) = t.terminal()
        && !term.is_transport_unresponsive()
    {
        v.push(UntruthfulTerminal);
    }

    // 3. No association / DHCP / LINK UP / PASS while the transport is not
    //    answering, and only a bounded non-destructive probe after it stopped.
    if let Some(at) = t.unrecovered_unresponsive_op_index {
        let after: Vec<Op> = t.ops.iter().skip(at + 1).copied().collect();
        if after.contains(&Op::AssociationAttempt) {
            v.push(AssociationAttemptedAfterUnresponsive);
        }
        if after.contains(&Op::DhcpAttempt) {
            v.push(DhcpAttemptedAfterUnresponsive);
        }
        if t.link_up {
            v.push(LinkUpClaimedAfterUnresponsive);
        }
        if t.terminal().is_some_and(Terminal::claims_pass) {
            v.push(PassClaimedAfterUnresponsive);
        }
        if t.device_ops_after(at) > MAX_POST_UNRESPONSIVE_DEVICE_OPS {
            v.push(UnboundedTrafficAfterUnresponsive);
        }
    }

    // 4. No command traffic after the transport is *classified* unresponsive.
    if let Some(classified) = t.classified_unresponsive_op_index
        && t.device_ops_after(classified) > 0
    {
        v.push(TrafficAfterClassifiedUnresponsive);
    }

    // 5. The forced cold reload is outside the accepted contract, full stop —
    //    keyed to the FIRST unresponsive observation so a reload that happens
    //    to succeed is still caught.
    if let Some(first) = t.first_unresponsive_op_index
        && t.forced_reloads_after(first) > 0
    {
        v.push(ForcedReloadAfterUnresponsive);
    }
    if t.forced_reload_count() > 1 {
        v.push(RepeatedFutileReload);
    }

    // 6. The terminal discriminator plus budget/fence accounting must survive
    //    to the evidence path — including the raw sink, not serial alone.
    if t.budget.exhausted
        && t.terminals
            .iter()
            .all(|term| matches!(term, Terminal::OperationBudgetExhausted))
    {
        v.push(BudgetExhaustedBeforeTerminal);
    }
    if !t.evidence.serial.is_empty() && t.evidence.sink.is_empty() {
        v.push(SinkEmptyWhileSerialNonEmpty);
    }
    if t.evidence.sink_terminal().is_none() {
        v.push(TerminalMissingFromSink);
    }
    if !t.evidence.has_budget_summary() {
        v.push(BudgetSummaryMissingFromSink);
    }
    if !t.evidence.has_fence_summary() {
        v.push(FenceSummaryMissingFromSink);
    }

    v.sort_unstable();
    v.dedup();
    if v.is_empty() { Ok(()) } else { Err(v) }
}

// ───────────────────────────────────────────────────────────────────────────
// The binding seam — the single entry point for a production candidate
// ───────────────────────────────────────────────────────────────────────────

/// Verdict of running a policy across every script the accepted contract must
/// hold on.
#[derive(Debug)]
struct CandidateReport {
    policy: &'static str,
    /// The observed TEST 3 world, with the evidence-sink repair applied.
    test3: Result<(), Vec<Violation>>,
    test3_terminal: Option<Terminal>,
    /// A genuinely transient stall: must recover and reach PASS.
    transient: Result<(), Vec<Violation>>,
    transient_terminal: Option<Terminal>,
    /// No wedge at all: must reach PASS over the full 1..=14 survey.
    healthy: Result<(), Vec<Violation>>,
    healthy_terminal: Option<Terminal>,
    /// Forced cold reloads issued anywhere, in any script. Must be zero.
    forced_reloads_anywhere: usize,
}

impl CandidateReport {
    fn qualified(&self) -> bool {
        self.test3.is_ok()
            && self.transient.is_ok()
            && self.healthy.is_ok()
            && self.forced_reloads_anywhere == 0
            && self
                .test3_terminal
                .is_some_and(Terminal::is_transport_unresponsive)
            && self.transient_terminal.is_some_and(Terminal::claims_pass)
            && self.healthy_terminal.is_some_and(Terminal::claims_pass)
    }
}

/// **The binding seam.** Hand it a constructor for a [`WedgePolicy`] and it
/// runs every accepted script, returning a verdict.
///
/// Qualifying [`ReferencePolicy`] proves the *harness* works. Qualifying a
/// policy backed by the production decision function is what qualifies the
/// *candidate* — and even then only against this contract on this modelled
/// trace, never in place of the four-shard gate, the AAG panel, or silicon.
fn qualify_policy<P: WedgePolicy, F: Fn() -> P>(make: F) -> CandidateReport {
    let sinked = |s: HostileScript| HostileScript {
        sink_available_at_bind: true,
        ..s
    };

    let mut p = make();
    let t3 = run_survey(sinked(HostileScript::test3()), &mut p);
    let mut p = make();
    let tr = run_survey(sinked(HostileScript::transient_endpoint_stall()), &mut p);
    let mut p = make();
    let hz = run_survey(sinked(HostileScript::healthy()), &mut p);

    CandidateReport {
        policy: t3.policy,
        test3_terminal: t3.terminal(),
        transient_terminal: tr.terminal(),
        healthy_terminal: hz.terminal(),
        forced_reloads_anywhere: t3.forced_reload_count()
            + tr.forced_reload_count()
            + hz.forced_reload_count(),
        test3: verify_contract(&t3),
        transient: verify_contract(&tr),
        healthy: verify_contract(&hz),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// The runner: models the driver's control flow, with the policy as the variable
// ───────────────────────────────────────────────────────────────────────────

fn run_survey<P: WedgePolicy>(script: HostileScript, policy: &mut P) -> Trace {
    let mut dev = HostileDevice::new(script);
    let mut evidence = Evidence {
        sink_installed: script.sink_available_at_bind,
        ..Evidence::default()
    };

    // Frozen constraint: `CHANNEL=0` ⇒ the genuine survey. Asserted against the
    // real shipped policy, not a copy.
    assert_eq!(
        wedge_logic::scan_tune_path(0),
        wedge_logic::TunePath::Survey,
        "CHANNEL=0 must select the survey route (no baked channel)"
    );

    let mut bss: BTreeSet<(u8, usize)> = BTreeSet::new();
    let mut channels_surveyed = 0u8;
    let mut classified_unresponsive_op_index: Option<usize> = None;
    let mut terminals: Vec<Terminal> = Vec::new();
    let mut bounded = true;
    let mut last_channel = REG_START;
    let mut reload_stalled_at_byte_zero = false;

    'survey: for ch in REG_START..REG_START + REG_NUM {
        if classified_unresponsive_op_index.is_some() {
            // Fast-skip with zero device traffic.
            evidence.breadcrumb(format!(
                "survey hop ch={ch} skipped (transport classified unresponsive)"
            ));
            continue;
        }
        last_channel = ch;

        // Every survey hop runs inline CAL_BW, matching raw upstream phy.c.
        let inline_cal = wedge_logic::inline_cal_bw(wedge_logic::TunePath::Survey, 0);
        assert!(inline_cal, "survey hops must run inline CAL_BW");

        let mut endpoint_probes_used = 0u32;
        let mut reloads_used = 0u32;
        let mut steps = 0u32;

        let mut outcome = dev.tune(ch);
        while let Err(failure) = outcome {
            steps += 1;
            if steps > POLICY_STEP_CAP {
                bounded = false;
                break 'survey;
            }
            if matches!(failure, DeviceError::OperationBudgetExhausted) {
                terminals.push(Terminal::OperationBudgetExhausted);
                break 'survey;
            }

            let obs = WedgeObservation {
                channel: ch,
                failure,
                endpoint_probes_used,
                reloads_used,
                reload_stalled_at_byte_zero,
                firmware_reports_resident: dev.com_reg0 == 1,
                budget_remaining: dev.budget.remaining,
                terminal_evidence_reserve: TERMINAL_EVIDENCE_RESERVE,
            };
            evidence.breadcrumb(format!(
                "survey hop ch={ch} failed: {failure:?} (probes={endpoint_probes_used} reloads={reloads_used})"
            ));

            match policy.on_tune_failure(&obs) {
                WedgeAction::ClearEndpointsAndRetry => {
                    endpoint_probes_used += 1;
                    let host = dev.recover_host_endpoints();
                    let device = dev.clear_device_halt();
                    evidence.breadcrumb(format!(
                        "endpoint recovery host={host:?} device={device:?} (reported ok ≠ restored)"
                    ));
                    let (Ok(host), Ok(device)) = (host, device) else {
                        terminals.push(Terminal::OperationBudgetExhausted);
                        break 'survey;
                    };

                    // Q-B steps 2 and 3, in the order production runs them.
                    // Neither may authorize anything by itself.
                    let drain_depth = wedge_logic::mcu_drain_depth(channels_surveyed);
                    let Ok(drain_cmd_done) = dev.drain_mcu_command_queue(drain_depth) else {
                        terminals.push(Terminal::OperationBudgetExhausted);
                        break 'survey;
                    };
                    let Ok(proven) = dev.prove_command_path() else {
                        terminals.push(Terminal::OperationBudgetExhausted);
                        break 'survey;
                    };

                    let recovery = RecoveryObservation {
                        endpoint_probe_reported_ok: host.reported_ok && device.reported_ok,
                        drain_commands_issued: drain_depth,
                        drain_cmd_done_observed: drain_cmd_done,
                        // The model cannot observe a completed-but-undecoded
                        // response separately, so it reports the conservative
                        // pair: a decode implies a completion, never the
                        // reverse. This can only ever refuse a retry that the
                        // real driver would have allowed, never allow one it
                        // would have refused.
                        probe_command_completed: proven,
                        probe_cmd_done_observed: proven,
                    };
                    evidence.breadcrumb(format!(
                        "mcu recovery drain={drain_depth} cmd_done={drain_cmd_done} \
                         command_path_proven={proven}"
                    ));

                    match policy.authorize_retry(&recovery) {
                        RetryAuthorization::Retry => {
                            outcome = dev.tune(ch);
                        }
                        RetryAuthorization::RefuseAndClassifyUnusable => {
                            classified_unresponsive_op_index =
                                Some(dev.ops.len().saturating_sub(1));
                            evidence.breadcrumb(format!(
                                "retry REFUSED at ch={ch}: command path not re-proven — \
                                 halting all command traffic"
                            ));
                            continue 'survey;
                        }
                    }
                }
                WedgeAction::ForceColdReload => {
                    reloads_used += 1;
                    match dev.forced_cold_reload() {
                        Ok(()) => {
                            evidence.breadcrumb("forced cold reload completed".to_string());
                            outcome = dev.tune(ch);
                        }
                        Err(DeviceError::BulkOutTimeout { uploaded, total }) => {
                            reload_stalled_at_byte_zero = uploaded == 0;
                            evidence.breadcrumb(format!(
                                "forced cold reload bulk-out timeout at ILM {uploaded}/{total}"
                            ));
                            outcome = Err(DeviceError::TransportUnresponsive);
                        }
                        Err(DeviceError::OperationBudgetExhausted) => {
                            terminals.push(Terminal::OperationBudgetExhausted);
                            break 'survey;
                        }
                        Err(other) => outcome = Err(other),
                    }
                }
                WedgeAction::SkipChannel => {
                    evidence.breadcrumb(format!("survey hop ch={ch} result=WEDGED — skipping"));
                    continue 'survey;
                }
                WedgeAction::ClassifyUnresponsiveAndStop => {
                    classified_unresponsive_op_index = Some(dev.ops.len().saturating_sub(1));
                    evidence.breadcrumb(format!(
                        "transport classified UNRESPONSIVE at ch={ch} — halting all command traffic"
                    ));
                    continue 'survey;
                }
            }
        }

        if outcome.is_ok() {
            channels_surveyed += 1;
            match dev.dwell(ch) {
                Ok(found) => {
                    for i in 0..found {
                        bss.insert((ch, i));
                    }
                }
                Err(_) => {
                    terminals.push(Terminal::OperationBudgetExhausted);
                    break 'survey;
                }
            }
        }
    }

    let bss_found = bss.len();
    let target_found = script
        .target_channels
        .iter()
        .any(|c| bss.iter().any(|(ch, _)| ch == c));

    let mut association_attempted = false;
    let mut association_succeeded = false;
    let mut dhcp_attempted = false;
    let mut dhcp_succeeded = false;
    let mut link_up = false;

    if terminals.is_empty() {
        if classified_unresponsive_op_index.is_some() {
            terminals.push(Terminal::TransportUnresponsiveDuringSurvey {
                last_channel,
                channels_surveyed,
                bss_found,
            });
        } else if !target_found {
            terminals.push(Terminal::ApNotFound { channels_surveyed });
        } else {
            association_attempted = true;
            match dev.attempt_association(target_found) {
                Ok(()) => {
                    association_succeeded = true;
                    dhcp_attempted = true;
                    match dev.attempt_dhcp() {
                        Ok(ip) => {
                            dhcp_succeeded = true;
                            link_up = true;
                            terminals.push(Terminal::Pass { assigned_ip: ip });
                        }
                        Err(_) => terminals.push(Terminal::DhcpTimeout),
                    }
                }
                Err(_) => terminals.push(Terminal::AssociationFailed { bss_found }),
            }
        }
    }

    // Terminal evidence: the discriminator plus the budget and fence summaries,
    // written host-locally (no device traffic) to both channels.
    dev.ops.push(Op::EmitEvidence);
    if let Some(term) = terminals.first().copied() {
        evidence.terminal(SinkRecord::Terminal(term));
    }
    evidence.terminal(SinkRecord::BudgetSummary {
        limit: dev.budget.limit,
        remaining: dev.budget.remaining,
        exhausted: dev.budget.exhausted,
    });
    evidence.terminal(SinkRecord::FenceSummary {
        fence_refusals: dev.fence_refusals,
    });

    Trace {
        policy: policy.name(),
        ops: dev.ops,
        first_unresponsive_op_index: dev.first_unresponsive_op_index,
        unrecovered_unresponsive_op_index: dev.unrecovered_unresponsive_op_index,
        unresponsive_episodes: dev.unresponsive_episodes,
        classified_unresponsive_op_index,
        terminals,
        channels_surveyed,
        bss_found,
        target_found,
        association_attempted,
        association_succeeded,
        dhcp_attempted,
        dhcp_succeeded,
        link_up,
        bounded,
        budget: dev.budget,
        fence_refusals: dev.fence_refusals,
        evidence,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

/// **Negative control (required by the dispatch).** The reviewed model of the
/// rejected behaviour, run against the exact TEST 3 script, must be RED — and
/// RED for the *specific* reasons, not "red somehow". Asserting the exact
/// violation set is what makes this a falsifiable control instead of a rubber
/// stamp.
///
/// Scope reminder: this refutes the **model** described on
/// [`RejectedBehaviourModel`]. Treating it as a finding about `2d43670e`
/// requires confirming the model against the candidate's actual source first.
#[test]
fn rejected_model_on_test3_trace_is_red_with_exact_violations() {
    use Violation::*;

    let mut policy = RejectedBehaviourModel::default();
    let trace = run_survey(HostileScript::test3(), &mut policy);

    // The scripted trace happened as designed: ch1..=6 surveyed, transport
    // stopped answering on ch7.
    assert_eq!(trace.channels_surveyed, 6, "trace: {:?}", trace.terminals);
    assert_eq!(
        trace.bss_found, PRE_WEDGE_BSSIDS,
        "the eight pre-wedge BSSIDs"
    );
    assert!(
        trace.unrecovered_unresponsive_op_index.is_some(),
        "the transport must have stopped answering and never come back"
    );
    assert!(
        trace.classified_unresponsive_op_index.is_none(),
        "the modelled behaviour never classifies the transport unresponsive — that is the defect"
    );

    let violations = verify_contract(&trace)
        .expect_err("the rejected behaviour must not satisfy the Lane-3 contract");

    assert_eq!(
        violations,
        vec![
            UntruthfulTerminal,
            UnboundedTrafficAfterUnresponsive,
            ForcedReloadAfterUnresponsive,
            RepeatedFutileReload,
            BudgetExhaustedBeforeTerminal,
            SinkEmptyWhileSerialNonEmpty,
            TerminalMissingFromSink,
            BudgetSummaryMissingFromSink,
            FenceSummaryMissingFromSink,
        ],
        "policy={} terminals={:?} budget_spent={} ops={}",
        trace.policy,
        trace.terminals,
        trace.budget.spent(),
        trace.ops.len(),
    );
}

/// The modelled run's ledger is destroyed by the futile reloads — the
/// "recovery destroys forward progress" boundary from the controlling
/// fix-forward, quantified.
#[test]
fn rejected_model_futile_reloads_exhaust_the_operation_ledger() {
    let mut policy = RejectedBehaviourModel::default();
    let trace = run_survey(HostileScript::test3(), &mut policy);

    assert_eq!(
        trace.forced_reload_count(),
        2,
        "the modelled MAX_SCAN_RECOVERY_ATTEMPTS=2 ⇒ two forced cold reloads"
    );
    assert!(
        trace.budget.exhausted,
        "two futile reloads must exhaust the {POST_FW_OPERATION_BUDGET}-unit ledger (spent={})",
        trace.budget.spent()
    );
    // The second reload is itself cut short by the exhausted ledger — it never
    // gets its third attempt. That truncation *is* the destroyed forward
    // progress: the run can no longer afford to say anything true.
    let stalled_chunks = trace
        .ops
        .iter()
        .filter(|op| matches!(op, Op::FwUploadChunk { offset: 0 }))
        .count();
    assert_eq!(
        stalled_chunks, 5,
        "3 attempts on reload 1, 2 on reload 2 before the ledger ran dry"
    );
    assert_eq!(
        trace.terminals,
        vec![Terminal::OperationBudgetExhausted],
        "the only category the modelled run can still reach is 'we ran out'"
    );
}

/// **The binding seam works, and the contract is satisfiable.** Running the
/// reference policy through [`qualify_policy`] is exactly the call Jarvis makes
/// with the production decision function bound in.
///
/// This qualifies the HARNESS. It is not candidate proof — see the module docs.
#[test]
fn reference_policy_qualifies_through_the_binding_seam() {
    let report = qualify_policy(ReferencePolicy::default);

    assert!(
        report.qualified(),
        "the reference policy must satisfy the contract on every accepted script: {report:#?}"
    );
    assert_eq!(
        report.forced_reloads_anywhere, 0,
        "no accepted script may involve a forced cold reload"
    );
    assert_eq!(
        report.test3_terminal,
        Some(Terminal::TransportUnresponsiveDuringSurvey {
            last_channel: 7,
            channels_surveyed: 6,
            bss_found: PRE_WEDGE_BSSIDS,
        }),
        "exactly one, truthful, and it names the truncation point"
    );
    assert!(
        report.transient_terminal.is_some_and(Terminal::claims_pass),
        "a genuinely transient stall must still reach PASS"
    );
    assert!(report.healthy_terminal.is_some_and(Terminal::claims_pass));
}

/// The accepted TEST 3 path claims nothing it did not earn: no association, no
/// DHCP, no LINK UP, no PASS, and the ledger survives with headroom.
#[test]
fn accepted_test3_path_claims_nothing_it_did_not_earn() {
    let mut policy = ReferencePolicy;
    let trace = run_survey(
        HostileScript {
            sink_available_at_bind: true,
            ..HostileScript::test3()
        },
        &mut policy,
    );

    verify_contract(&trace).expect("the accepted path must be green");
    assert!(!trace.association_attempted);
    assert!(!trace.dhcp_attempted);
    assert!(!trace.link_up);
    assert_eq!(trace.forced_reload_count(), 0, "no destructive step at all");
    assert!(!trace.budget.exhausted, "spent={}", trace.budget.spent());
    // Exactly the bounded non-destructive recovery, and — the property that
    // `cea89e6a` violated — it stops one op SHORT of the allowance, because on
    // this silicon the command path never re-proved and the retry tune was
    // therefore refused. Asserting equality with the maximum would silently
    // re-encode the old "entering recovery means retrying" behaviour.
    let at = trace.unrecovered_unresponsive_op_index.unwrap();
    let after: Vec<Op> = trace.ops.iter().skip(at + 1).copied().collect();
    assert_eq!(
        after
            .iter()
            .filter(|op| op.touches_device())
            .copied()
            .collect::<Vec<_>>(),
        vec![
            Op::HostEndpointRecover,
            Op::DeviceClearHalt,
            Op::McuQueueDrain { commands: 6 },
            Op::CommandPathProbe,
        ],
        "the recovery sequence must be exactly endpoint -> drain -> probe, and \
         must NOT reach a retry tune; ops={:?}",
        trace.ops
    );
    assert!(
        trace.device_ops_after(at) < MAX_POST_UNRESPONSIVE_DEVICE_OPS,
        "the refused retry must leave the allowance unspent: {} of {}",
        trace.device_ops_after(at),
        MAX_POST_UNRESPONSIVE_DEVICE_OPS
    );
    assert!(
        !after.iter().any(|op| matches!(op, Op::Tune { .. })),
        "no tune may be re-issued against a command path that never re-proved"
    );
}

/// Required property B, as an oracle: with the sink installed only *after* USB
/// init, the identical green-policy run goes RED — a zero sink paired with a
/// non-empty serial log. This is the TEST 3 evidence split, isolated from the
/// driver defect.
#[test]
fn late_sink_installation_is_red_even_under_the_accepted_policy() {
    use Violation::*;

    let mut policy = ReferencePolicy;
    let trace = run_survey(HostileScript::test3(), &mut policy); // sink_available_at_bind: false

    assert!(
        !trace.evidence.serial.is_empty(),
        "serial must carry substantial evidence, as it did on silicon"
    );
    assert!(
        trace.evidence.sink.is_empty(),
        "the 16 MiB sink is all-zero"
    );

    let violations = verify_contract(&trace).expect_err("zero sink + non-empty serial is RED");
    assert_eq!(
        violations,
        vec![
            SinkEmptyWhileSerialNonEmpty,
            TerminalMissingFromSink,
            BudgetSummaryMissingFromSink,
            FenceSummaryMissingFromSink,
        ],
        "the *only* failures must be evidence-path failures — the driver policy is sound here"
    );
}

/// The pre-wedge survey result is not destroyed: all eight BSSIDs discovered on
/// channels 1..=6 survive into the terminal record in the sink.
#[test]
fn accepted_policy_preserves_pre_wedge_survey_evidence() {
    let mut policy = ReferencePolicy;
    let trace = run_survey(
        HostileScript {
            sink_available_at_bind: true,
            ..HostileScript::test3()
        },
        &mut policy,
    );

    match trace.evidence.sink_terminal() {
        Some(Terminal::TransportUnresponsiveDuringSurvey {
            bss_found,
            channels_surveyed,
            last_channel,
        }) => {
            assert_eq!(bss_found, PRE_WEDGE_BSSIDS);
            assert_eq!(channels_surveyed, 6);
            assert_eq!(last_channel, 7);
        }
        other => panic!("sink must carry the truncation-aware terminal, got {other:?}"),
    }
}

/// **Truthfulness.** The configured AP is on channel 11, beyond the wedge. A
/// truncated survey must never report `ApNotFound` — that would claim coverage
/// the run never achieved, and would send the next silicon boot chasing an
/// association bug that does not exist.
#[test]
fn truncated_survey_must_not_claim_the_ap_is_absent() {
    let mut policy = ReferencePolicy;
    let trace = run_survey(
        HostileScript {
            sink_available_at_bind: true,
            target_channels: &[11],
            ..HostileScript::test3()
        },
        &mut policy,
    );

    assert!(!trace.target_found, "ch11 was never reached");
    assert!(
        trace
            .terminal()
            .is_some_and(Terminal::is_transport_unresponsive),
        "got {:?}",
        trace.terminal()
    );
    assert!(
        !matches!(trace.terminal(), Some(Terminal::ApNotFound { .. })),
        "a truncated survey may not be reported as a complete one"
    );
    assert!(
        !matches!(trace.terminal(), Some(Terminal::AssociationFailed { .. })),
        "NOTE-A is reserved for AP-found-but-association-failed; this is not that"
    );
}

/// **Anti-overfit control.** If the wedge really is a transient endpoint stall,
/// the bounded non-destructive probe fixes it, the survey completes, and the run
/// reaches PASS. A contract that merely mandated "give up" would fail this.
#[test]
fn transient_endpoint_stall_recovers_and_reaches_pass() {
    let mut policy = ReferencePolicy;
    let trace = run_survey(HostileScript::transient_endpoint_stall(), &mut policy);

    verify_contract(&trace).expect("a genuinely recoverable stall must stay green");
    assert_eq!(trace.unresponsive_episodes, 1, "it did stop answering…");
    assert!(
        trace.unrecovered_unresponsive_op_index.is_none(),
        "…but it came back, so the unrecovered clauses must not fire"
    );
    assert!(
        trace.first_unresponsive_op_index.is_some(),
        "the first-unresponsive marker is never cleared"
    );
    assert_eq!(trace.channels_surveyed, REG_NUM);
    assert!(trace.target_found);
    assert!(trace.association_succeeded && trace.dhcp_succeeded && trace.link_up);
    assert_eq!(
        trace.terminals,
        vec![Terminal::Pass {
            assigned_ip: [192, 168, 1, 8]
        }]
    );
    assert_eq!(trace.forced_reload_count(), 0, "no destructive step needed");
}

/// **The forced cold reload is outside the accepted contract even in a world
/// where it would have worked.**
///
/// Two runs on the same adversarial script, in which a reload *does* restore the
/// transport:
/// * the accepted policy never reloads — it classifies unresponsive and stops,
///   and stays green;
/// * a reload-using policy reaches PASS on this script and is still RED, because
///   [`Violation::ForcedReloadAfterUnresponsive`] is keyed to the first
///   unresponsive observation, not to whether the gamble paid off.
///
/// The driver cannot tell this world from TEST 3's in advance, and on TEST 3's
/// silicon the losing branch strands the run with nothing true left to say.
#[test]
fn forced_reload_is_outside_the_contract_even_when_it_would_succeed() {
    use Violation::*;

    // (a) the accepted policy declines the gamble and stays green.
    let mut accepted = ReferencePolicy;
    let declined = run_survey(HostileScript::reload_would_have_worked(), &mut accepted);
    verify_contract(&declined).expect("declining the reload must be green");
    assert_eq!(declined.forced_reload_count(), 0);
    assert!(
        declined
            .terminal()
            .is_some_and(Terminal::is_transport_unresponsive),
        "got {:?}",
        declined.terminal()
    );

    // (b) a policy that reloads wins the gamble — and is still RED.
    #[derive(Default)]
    struct ReloadingPolicy {
        used: u32,
    }
    impl WedgePolicy for ReloadingPolicy {
        fn on_tune_failure(&mut self, _obs: &WedgeObservation) -> WedgeAction {
            if self.used == 0 {
                self.used += 1;
                return WedgeAction::ForceColdReload;
            }
            WedgeAction::ClassifyUnresponsiveAndStop
        }
        fn authorize_retry(&mut self, _obs: &RecoveryObservation) -> RetryAuthorization {
            RetryAuthorization::Retry
        }
        fn name(&self) -> &'static str {
            "reloading (outside the contract)"
        }
    }

    let mut reloading = ReloadingPolicy::default();
    let gambled = run_survey(HostileScript::reload_would_have_worked(), &mut reloading);
    assert_eq!(gambled.forced_reload_count(), 1);
    assert_eq!(gambled.channels_surveyed, REG_NUM, "the gamble paid off…");
    assert!(gambled.link_up);

    let violations = verify_contract(&gambled)
        .expect_err("…and the contract still refuses it: a win is not a licence");
    assert_eq!(violations, vec![ForcedReloadAfterUnresponsive]);

    // And the same policy on the real TEST 3 world loses the gamble outright.
    let mut reloading = ReloadingPolicy::default();
    let lost = run_survey(
        HostileScript {
            sink_available_at_bind: true,
            ..HostileScript::test3()
        },
        &mut reloading,
    );
    let lost_violations = verify_contract(&lost).expect_err("the losing branch must be RED");
    assert!(
        lost_violations.contains(&ForcedReloadAfterUnresponsive),
        "{lost_violations:?}"
    );
}

/// **Positive control.** With no wedge at all the oracle is green and reaches
/// PASS — so a green result carries information (the checker is not vacuously
/// satisfiable, and not vacuously red).
#[test]
fn healthy_survey_reaches_pass() {
    let mut policy = ReferencePolicy;
    let trace = run_survey(HostileScript::healthy(), &mut policy);

    verify_contract(&trace).expect("an unwedged survey must be green");
    assert!(trace.first_unresponsive_op_index.is_none());
    assert_eq!(trace.unresponsive_episodes, 0);
    assert_eq!(trace.channels_surveyed, REG_NUM);
    assert_eq!(trace.bss_found, FULL_SURVEY_BSSIDS);
    assert!(trace.terminal().is_some_and(Terminal::claims_pass));
}

/// Frozen constraint: `CHANNEL=0` selects a genuine survey, asserted against the
/// **real shipped** `wedge_logic`, and the survey route runs inline CAL_BW on
/// every hop. No baked channel, no targeted shortcut, may become shipping proof.
#[test]
fn channel_zero_survey_semantics_come_from_the_real_shipped_policy() {
    use wedge_logic::{TunePath, inline_cal_bw, scan_tune_path};

    assert_eq!(scan_tune_path(0), TunePath::Survey);
    for ch in 1..=14u8 {
        assert_eq!(
            scan_tune_path(ch),
            TunePath::Targeted,
            "a non-zero configured channel is the optional fast path, never the survey"
        );
    }
    for tunes in [0u8, 1, u8::MAX] {
        assert!(
            inline_cal_bw(TunePath::Survey, tunes),
            "every survey hop runs inline CAL_BW regardless of prior targeted tunes"
        );
    }

    // And the runner actually walks all 14 regulatory channels when unwedged.
    let mut policy = ReferencePolicy;
    let trace = run_survey(HostileScript::healthy(), &mut policy);
    let tuned: Vec<u8> = trace
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::Tune { channel } => Some(*channel),
            _ => None,
        })
        .collect();
    assert_eq!(tuned, (REG_START..REG_START + REG_NUM).collect::<Vec<u8>>());
}

/// The forced reload works by **overriding** the real shipped
/// `firmware_load_action` policy: an unresponsive device still reports resident
/// firmware, so `UseResident` is what the shipped policy says, and the
/// modelled rejected behaviour writes `COM_REG0 = 0` specifically to defeat it.
/// Pinning this makes the destructive override visible in the oracle rather than
/// buried in `recover_chip`.
#[test]
fn forced_reload_overrides_the_real_resident_firmware_policy() {
    use linux_parity_logic::{FirmwareLoadAction, firmware_load_action};

    // What the shipped policy says about a device that still claims residency.
    assert_eq!(firmware_load_action(true), FirmwareLoadAction::UseResident);
    assert_eq!(firmware_load_action(false), FirmwareLoadAction::UploadCold);

    let mut policy = RejectedBehaviourModel::default();
    let trace = run_survey(HostileScript::test3(), &mut policy);

    // The wedged device never stopped claiming residency…
    let zeroed = trace
        .ops
        .iter()
        .filter(|op| matches!(op, Op::WriteComReg0 { value: 0 }))
        .count();
    // …so the modelled behaviour zeroed COM_REG0 to force `UploadCold` — twice.
    assert_eq!(zeroed, 2);
    assert!(
        trace
            .ops
            .windows(2)
            .any(|w| matches!(w[0], Op::WriteComReg0 { value: 0 })
                && matches!(w[1], Op::FwPreloadSetup { .. })),
        "the COM_REG0 override is what immediately precedes each cold upload"
    );
}

/// Production-bound Linux-parity oracle for the exact TEST 3A seam. The
/// accepted trace performs zero resident-path mutations, owns both receive
/// surfaces before MAC init, and fail-stops with a sink flush. The negative
/// control recreates the rejected resident-reset/late-RX ordering.
#[test]
fn linux_parity_post_qselect_policy_is_production_bound() {
    use linux_parity_logic::{
        FirmwareLoadAction, FirmwareLoadEffects, PostQselectTrace, PostQselectTrafficAction,
        firmware_load_effects, parity_violation, post_qselect_failure_disposition,
        post_qselect_parity_violations, post_qselect_traffic_action, ready_for_mac_init,
    };

    let accepted = PostQselectTrace {
        resident_effects: firmware_load_effects(FirmwareLoadAction::UseResident),
        command_response_owned: true,
        packet_rx_posted: 16,
        packet_rx_required: 16,
    };
    assert_eq!(post_qselect_parity_violations(accepted), 0);
    assert!(ready_for_mac_init(true, 16, 16));

    let rejected = PostQselectTrace {
        resident_effects: FirmwareLoadEffects {
            preload_runs: 1,
            vendor_resets: 1,
            fce_pbf_writes: 1,
            cold_uploads: 1,
        },
        command_response_owned: false,
        packet_rx_posted: 0,
        packet_rx_required: 16,
    };
    let violations = post_qselect_parity_violations(rejected);
    assert_ne!(violations & parity_violation::RESIDENT_PRELOAD, 0);
    assert_ne!(violations & parity_violation::RESIDENT_VENDOR_RESET, 0);
    assert_ne!(violations & parity_violation::RESIDENT_FCE_PBF_WRITE, 0);
    assert_ne!(violations & parity_violation::RESIDENT_COLD_UPLOAD, 0);
    assert_ne!(violations & parity_violation::RESPONSE_NOT_OWNED, 0);
    assert_ne!(violations & parity_violation::PACKET_RX_NOT_READY, 0);

    let terminal = post_qselect_failure_disposition();
    assert!(terminal.stop);
    assert!(!terminal.attempt_firmware_reload);
    assert!(!terminal.allow_later_device_traffic);
    assert!(terminal.flush_serial_and_sink);
    assert_eq!(
        post_qselect_traffic_action(true),
        PostQselectTrafficAction::Block
    );
}

fn run_production_post_qselect_trace(
    injected_failure: Option<linux_parity_logic::PostQselectStage>,
) -> linux_parity_logic::PostQselectOperationTrace {
    use linux_parity_logic::{
        PostQselectOperationTrace, PostQselectOutcome, PostQselectStage as Stage,
        PostQselectTerminalAction, PostQselectTrafficAction, post_qselect_boundary as boundary,
        post_qselect_failure_disposition, post_qselect_terminal_action,
        post_qselect_traffic_action,
    };

    let ordered = [
        Stage::MacCommon,
        Stage::MacChip,
        Stage::MacAux,
        Stage::MacStatus,
        Stage::BbpReady,
        Stage::BbpCommon,
        Stage::BbpChip,
        Stage::WcidAddress,
        Stage::SharedKeyMode,
        Stage::WcidAttribute,
        Stage::InterveningRegisters,
        Stage::EepromAcquire,
    ];
    let mut stages_entered = 0;
    let mut boundary_flushes = 0;

    for stage in ordered {
        match stage {
            Stage::BbpReady => boundary_flushes |= boundary::BBP_BEFORE,
            Stage::WcidAddress => {
                boundary_flushes |= boundary::BBP_AFTER | boundary::WCID_BEFORE;
            }
            Stage::InterveningRegisters => boundary_flushes |= boundary::WCID_AFTER,
            Stage::EepromAcquire => boundary_flushes |= boundary::EEPROM_BEFORE,
            _ => {}
        }
        stages_entered |= stage.bit();
        if injected_failure == Some(stage) {
            let disposition = post_qselect_failure_disposition();
            let command_path_unusable = disposition.stop && !disposition.allow_later_device_traffic;
            let mut terminal_latched = false;
            let mut terminal_count = 0;
            for _ in 0..2 {
                match post_qselect_terminal_action(terminal_latched) {
                    PostQselectTerminalAction::Emit => {
                        terminal_latched = true;
                        terminal_count += 1;
                    }
                    PostQselectTerminalAction::SuppressDuplicate => {}
                }
            }
            let mut traffic_after = 0;
            let mut traffic_blocked = 0;
            // Exercise both production-gated traffic classes: one MCU command
            // and one direct vendor-register operation.
            for _ in 0..2 {
                match post_qselect_traffic_action(command_path_unusable) {
                    PostQselectTrafficAction::Block => traffic_blocked += 1,
                    PostQselectTrafficAction::Allow => traffic_after += 1,
                }
            }
            return PostQselectOperationTrace {
                outcome: PostQselectOutcome::Failed(stage),
                enclosing_budget: true,
                budget_starts: 1,
                stages_entered,
                boundary_flushes,
                terminal_count,
                terminal_flushed: disposition.flush_serial_and_sink,
                device_traffic_after_terminal: traffic_after,
                device_traffic_blocked_after_terminal: traffic_blocked,
            };
        }
    }
    boundary_flushes |= boundary::EEPROM_AFTER;
    PostQselectOperationTrace {
        outcome: PostQselectOutcome::Success,
        enclosing_budget: true,
        budget_starts: 1,
        stages_entered,
        boundary_flushes,
        terminal_count: 0,
        terminal_flushed: false,
        device_traffic_after_terminal: 0,
        device_traffic_blocked_after_terminal: 0,
    }
}

/// The complete success path and every required injected stage failure consume
/// the production stage/disposition/gate seams. This pins one terminal, one
/// enclosing ledger, durable boundaries, and zero device traffic after latch.
#[test]
fn complete_post_qselect_contract_is_production_bound() {
    use linux_parity_logic::{PostQselectStage as Stage, post_qselect_operation_violations};

    assert_eq!(
        post_qselect_operation_violations(run_production_post_qselect_trace(None)),
        0
    );
    for stage in [
        Stage::BbpReady,
        Stage::BbpCommon,
        Stage::BbpChip,
        Stage::WcidAddress,
        Stage::SharedKeyMode,
        Stage::WcidAttribute,
        Stage::InterveningRegisters,
        Stage::EepromAcquire,
    ] {
        let trace = run_production_post_qselect_trace(Some(stage));
        assert_eq!(
            post_qselect_operation_violations(trace),
            0,
            "production contract rejected injected stage {stage:?}: {trace:?}"
        );
    }
}

/// Focused mutations prove that the checker rejects each property whose
/// removal recreates the stale-sink or post-terminal-traffic failure class.
#[test]
fn complete_post_qselect_contract_negative_controls_have_teeth() {
    use linux_parity_logic::{
        PostQselectStage as Stage, post_qselect_operation_violations,
        post_qselect_violation as violation,
    };

    let mut trace = run_production_post_qselect_trace(Some(Stage::EepromAcquire));
    trace.enclosing_budget = false;
    assert_ne!(
        post_qselect_operation_violations(trace) & violation::MISSING_ENCLOSING_BUDGET,
        0
    );

    let mut trace = run_production_post_qselect_trace(Some(Stage::EepromAcquire));
    trace.budget_starts = 2;
    assert_ne!(
        post_qselect_operation_violations(trace) & violation::BUDGET_REPLENISHED,
        0
    );

    let mut trace = run_production_post_qselect_trace(Some(Stage::BbpCommon));
    trace.terminal_flushed = false;
    assert_ne!(
        post_qselect_operation_violations(trace) & violation::TERMINAL_NOT_FLUSHED,
        0
    );

    let mut trace = run_production_post_qselect_trace(Some(Stage::WcidAttribute));
    trace.device_traffic_after_terminal = 1;
    trace.device_traffic_blocked_after_terminal = 0;
    assert_ne!(
        post_qselect_operation_violations(trace) & violation::TRAFFIC_AFTER_TERMINAL,
        0
    );
    assert_ne!(
        post_qselect_operation_violations(trace) & violation::POST_TERMINAL_GATE_NOT_EXERCISED,
        0
    );

    let mut trace = run_production_post_qselect_trace(Some(Stage::EepromAcquire));
    trace.boundary_flushes = 0;
    assert_ne!(
        post_qselect_operation_violations(trace) & violation::MISSING_BOUNDARY_FLUSH,
        0
    );
}

/// A policy that never terminates is caught by the step cap rather than hanging
/// the suite — the "bounded completion" clause has teeth.
#[test]
fn a_nonterminating_policy_is_reported_as_unbounded() {
    #[derive(Default)]
    struct SpinForever;
    impl WedgePolicy for SpinForever {
        fn on_tune_failure(&mut self, _obs: &WedgeObservation) -> WedgeAction {
            WedgeAction::ClearEndpointsAndRetry
        }
        /// Always authorizes — the point of this control is that an unbounded
        /// retry loop is caught by the step cap, not by the authorization.
        fn authorize_retry(&mut self, _obs: &RecoveryObservation) -> RetryAuthorization {
            RetryAuthorization::Retry
        }
        fn name(&self) -> &'static str {
            "spin-forever"
        }
    }

    let mut policy = SpinForever;
    let trace = run_survey(
        HostileScript {
            sink_available_at_bind: true,
            ..HostileScript::test3()
        },
        &mut policy,
    );

    assert!(!trace.bounded);
    let violations = verify_contract(&trace).expect_err("an unbounded policy must be RED");
    assert!(violations.contains(&Violation::Unbounded), "{violations:?}");
}

/// **Checker teeth, clause by clause.** Each case flips exactly one property of
/// an otherwise-green trace and asserts the matching violation fires. A checker
/// that only ever notices one thing cannot pass this battery.
#[test]
fn checker_has_teeth_on_every_clause() {
    use Violation::*;

    let green = || {
        let mut policy = ReferencePolicy;
        run_survey(
            HostileScript {
                sink_available_at_bind: true,
                ..HostileScript::test3()
            },
            &mut policy,
        )
    };
    verify_contract(&green()).expect("baseline must be green");

    // (a) no terminal category at all
    let mut t = green();
    t.terminals.clear();
    t.evidence
        .sink
        .retain(|r| !matches!(r, SinkRecord::Terminal(_)));
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&NoTerminalCategory), "{v:?}");

    // (b) two terminal categories
    let mut t = green();
    t.terminals.push(Terminal::DhcpTimeout);
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&MultipleTerminalCategories), "{v:?}");

    // (c) untruthful terminal — blames association after the transport stopped
    let mut t = green();
    t.terminals = vec![Terminal::AssociationFailed { bss_found: 8 }];
    t.evidence.sink = vec![
        SinkRecord::Terminal(Terminal::AssociationFailed { bss_found: 8 }),
        SinkRecord::BudgetSummary {
            limit: POST_FW_OPERATION_BUDGET,
            remaining: 1,
            exhausted: false,
        },
        SinkRecord::FenceSummary { fence_refusals: 0 },
    ];
    let v = verify_contract(&t).unwrap_err();
    assert_eq!(v, vec![UntruthfulTerminal], "{v:?}");

    // (d) PASS claimed while the transport was never restored
    let mut t = green();
    t.terminals = vec![Terminal::Pass {
        assigned_ip: [10, 0, 0, 2],
    }];
    t.link_up = true;
    t.evidence.sink[0] = SinkRecord::Terminal(t.terminals[0]);
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&PassClaimedAfterUnresponsive), "{v:?}");
    assert!(v.contains(&LinkUpClaimedAfterUnresponsive), "{v:?}");
    assert!(v.contains(&UntruthfulTerminal), "{v:?}");

    // (e) association / DHCP attempted while unresponsive
    let mut t = green();
    t.ops.push(Op::AssociationAttempt);
    t.ops.push(Op::DhcpAttempt);
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&AssociationAttemptedAfterUnresponsive), "{v:?}");
    assert!(v.contains(&DhcpAttemptedAfterUnresponsive), "{v:?}");

    // (f) traffic after the transport was classified unresponsive
    let mut t = green();
    t.ops.push(Op::Tune { channel: 8 });
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&TrafficAfterClassifiedUnresponsive), "{v:?}");

    // (g) traffic past the bounded non-destructive probe allowance
    let mut t = green();
    for _ in 0..(MAX_POST_UNRESPONSIVE_DEVICE_OPS + 1) {
        t.ops.push(Op::Tune { channel: 9 });
    }
    t.classified_unresponsive_op_index = None;
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&UnboundedTrafficAfterUnresponsive), "{v:?}");

    // (h) a single forced reload — outside the contract on its own
    let mut t = green();
    t.ops.push(Op::WriteComReg0 { value: 0 });
    t.classified_unresponsive_op_index = None;
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&ForcedReloadAfterUnresponsive), "{v:?}");
    assert!(
        !v.contains(&RepeatedFutileReload),
        "one is not a repeat: {v:?}"
    );

    // (h2) a repeated forced reload adds the futile-retry signature
    let mut t = green();
    t.ops.push(Op::WriteComReg0 { value: 0 });
    t.ops.push(Op::WriteComReg0 { value: 0 });
    t.classified_unresponsive_op_index = None;
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&RepeatedFutileReload), "{v:?}");

    // (i) budget exhausted before any real terminal could be produced
    let mut t = green();
    t.budget.exhausted = true;
    t.terminals = vec![Terminal::OperationBudgetExhausted];
    t.evidence.sink[0] = SinkRecord::Terminal(Terminal::OperationBudgetExhausted);
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&BudgetExhaustedBeforeTerminal), "{v:?}");

    // (j) each evidence channel, independently
    let mut t = green();
    t.evidence
        .sink
        .retain(|r| !matches!(r, SinkRecord::Terminal(_)));
    let v = verify_contract(&t).unwrap_err();
    assert_eq!(v, vec![TerminalMissingFromSink], "{v:?}");

    let mut t = green();
    t.evidence
        .sink
        .retain(|r| !matches!(r, SinkRecord::BudgetSummary { .. }));
    let v = verify_contract(&t).unwrap_err();
    assert_eq!(v, vec![BudgetSummaryMissingFromSink], "{v:?}");

    let mut t = green();
    t.evidence
        .sink
        .retain(|r| !matches!(r, SinkRecord::FenceSummary { .. }));
    let v = verify_contract(&t).unwrap_err();
    assert_eq!(v, vec![FenceSummaryMissingFromSink], "{v:?}");

    let mut t = green();
    t.evidence.sink.clear();
    let v = verify_contract(&t).unwrap_err();
    assert!(v.contains(&SinkEmptyWhileSerialNonEmpty), "{v:?}");
}

/// `fence_refusals` is *reported*, never assumed. The controlling fix-forward
/// is explicit that an unemitted value must not be reported as zero, so the
/// sink record carries the observed count and its absence is a violation
/// (clause (j) above), not a default.
#[test]
fn fence_refusals_are_reported_not_defaulted() {
    let mut policy = ReferencePolicy;
    let trace = run_survey(
        HostileScript {
            sink_available_at_bind: true,
            ..HostileScript::test3()
        },
        &mut policy,
    );

    assert!(
        trace.evidence.has_fence_summary(),
        "the fence summary must reach the sink"
    );
    assert_eq!(
        trace.evidence.sink.iter().copied().find_map(|r| match r {
            SinkRecord::FenceSummary { fence_refusals } => Some(fence_refusals),
            _ => None,
        }),
        Some(trace.fence_refusals),
        "the reported value must be the observed counter, not a literal"
    );
}

/// The production decision seams, rather than a constant-returning oracle or a
/// never-mutated counter, drive the hostile operation trace. `qualified()`
/// checks the recorded operations for destructive reload and post-terminal
/// traffic across every accepted script.
/// The repair required by `AAG-BOUNCE-2026-08-01-wifi-retry-authorization-unbound`.
///
/// `set_channel_accounted` drives three pure decision seams; this oracle
/// previously bound two, leaving the **retry authorization** — the one with a
/// known silicon hang behind it (`cea89e6a`) — unqualified. These are the two
/// negative controls the bounce named, plus the teeth that prove they can fail.
#[test]
fn retry_authorization_is_bound_to_the_production_seam() {
    // ── Control 1: transport recovered, NO decoded CMD_DONE ⇒ MUST refuse.
    // This is the exact cea89e6a shape and the reason the finding was a blocker.
    let refused = ProductionPolicy.authorize_retry(&RecoveryObservation {
        endpoint_probe_reported_ok: true, // the endpoint probe lied
        drain_commands_issued: 6,
        drain_cmd_done_observed: 6, // even a fully-retired drain
        probe_command_completed: false,
        probe_cmd_done_observed: false,
    });
    assert_eq!(
        refused,
        RetryAuthorization::RefuseAndClassifyUnusable,
        "endpoint recovery + a fully drained queue must NOT authorize a retry"
    );

    // A completed transfer whose response never decoded CMD_DONE is not proof
    // either — the same category error one layer down.
    assert_eq!(
        ProductionPolicy.authorize_retry(&RecoveryObservation {
            endpoint_probe_reported_ok: true,
            drain_commands_issued: 6,
            drain_cmd_done_observed: 6,
            probe_command_completed: true,
            probe_cmd_done_observed: false,
        }),
        RetryAuthorization::RefuseAndClassifyUnusable,
        "a completed transfer without a decoded CMD_DONE is not command-path proof"
    );

    // ── Control 2: CMD_DONE decoded after the probe ⇒ MAY authorize.
    assert_eq!(
        ProductionPolicy.authorize_retry(&RecoveryObservation {
            endpoint_probe_reported_ok: true,
            drain_commands_issued: 6,
            drain_cmd_done_observed: 6,
            probe_command_completed: true,
            probe_cmd_done_observed: true,
        }),
        RetryAuthorization::Retry,
        "a re-proven command path must be allowed to retry — the oracle must not \
         be vacuously refusing everything"
    );

    // A dead transport is never rescued by a probe that somehow reported.
    assert_eq!(
        ProductionPolicy.authorize_retry(&RecoveryObservation {
            endpoint_probe_reported_ok: false,
            drain_commands_issued: 0,
            drain_cmd_done_observed: 0,
            probe_command_completed: true,
            probe_cmd_done_observed: true,
        }),
        RetryAuthorization::RefuseAndClassifyUnusable
    );

    // ── The binding itself: the oracle's verdict IS the shipped function's
    // verdict across the whole truth table, not a re-implementation that could
    // drift from it.
    for &endpoint_ok in &[false, true] {
        for &completed in &[false, true] {
            for &decoded in &[false, true] {
                let obs = RecoveryObservation {
                    endpoint_probe_reported_ok: endpoint_ok,
                    drain_commands_issued: 3,
                    drain_cmd_done_observed: 1,
                    probe_command_completed: completed,
                    probe_cmd_done_observed: decoded,
                };
                let shipped =
                    wedge_logic::channel_retry_action(wedge_logic::recovery_retry_observation(
                        wedge_logic::ChannelRecoveryObservation {
                            endpoint_probe_reported_ok: endpoint_ok,
                            drain_commands_issued: 3,
                            drain_cmd_done_observed: 1,
                            probe_command_completed: completed,
                            probe_cmd_done_observed: decoded,
                        },
                    ));
                let expected = match shipped {
                    wedge_logic::ChannelRetryAction::Retry => RetryAuthorization::Retry,
                    wedge_logic::ChannelRetryAction::ClassifyCommandPathUnusable => {
                        RetryAuthorization::RefuseAndClassifyUnusable
                    }
                };
                assert_eq!(
                    ProductionPolicy.authorize_retry(&obs),
                    expected,
                    "oracle must consume the shipped decision (ep={endpoint_ok} \
                     completed={completed} decoded={decoded})"
                );
            }
        }
    }
}

/// End-to-end: in the world where the transport comes back but the MCU stays
/// latched, the production policy must not re-issue the tune — and a policy
/// that authorizes on endpoint recovery alone must be caught doing so.
///
/// Without this script the seam is untestable end-to-end, because every other
/// script either restores everything or nothing.
#[test]
fn transport_recovery_without_command_path_proof_refuses_the_retry() {
    let script = HostileScript::transport_up_mcu_latched();

    // Production: refuses, and never re-tunes the wedge channel.
    let mut production = ProductionPolicy;
    let good = run_survey(script, &mut production);
    verify_contract(&good).expect("refusing an unproven retry must be contract-green");
    assert_eq!(
        good.ops
            .iter()
            .filter(|op| matches!(op, Op::Tune { channel: 7 }))
            .count(),
        1,
        "the wedge channel must be tuned exactly once — no unproven re-issue; ops={:?}",
        good.ops
    );
    assert!(
        good.ops.contains(&Op::CommandPathProbe),
        "the probe must actually have been attempted"
    );

    // Teeth: a policy that treats endpoint recovery as permission — the
    // pre-fix behaviour — re-issues the identical tune into the latched MCU.
    #[derive(Default)]
    struct AuthorizeOnEndpointRecovery {
        probes: u32,
    }
    impl WedgePolicy for AuthorizeOnEndpointRecovery {
        fn on_tune_failure(&mut self, _obs: &WedgeObservation) -> WedgeAction {
            if self.probes >= 2 {
                return WedgeAction::ClassifyUnresponsiveAndStop;
            }
            self.probes += 1;
            WedgeAction::ClearEndpointsAndRetry
        }
        fn authorize_retry(&mut self, obs: &RecoveryObservation) -> RetryAuthorization {
            // The cea89e6a mistake, stated exactly.
            if obs.endpoint_probe_reported_ok {
                RetryAuthorization::Retry
            } else {
                RetryAuthorization::RefuseAndClassifyUnusable
            }
        }
        fn name(&self) -> &'static str {
            "authorize-on-endpoint-recovery (the cea89e6a mistake)"
        }
    }

    let mut bad = AuthorizeOnEndpointRecovery::default();
    let hung = run_survey(script, &mut bad);
    assert!(
        hung.ops
            .iter()
            .filter(|op| matches!(op, Op::Tune { channel: 7 }))
            .count()
            > 1,
        "the control must actually re-issue the tune into a latched MCU, \
         otherwise it proves nothing; ops={:?}",
        hung.ops
    );
}

#[test]
fn production_policy_qualifies_through_the_binding_seam() {
    let report = qualify_policy(ProductionPolicy::default);

    assert!(
        report.qualified(),
        "the production-backed policy must satisfy every hostile script: {report:#?}"
    );
}
