// SPDX-License-Identifier: MPL-2.0
//! Pure policy for the MT7601U channel-switch failure contract.
//!
//! The device-touching driver consumes these decisions; `ai-ops-tests` imports
//! this file directly so the two control-flow seams remain host-testable without
//! pretending that QEMU emulates the vendor MCU.

#![allow(dead_code)]

/// Why the RF channel is being programmed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TunePath {
    /// Ordinary 1..=14 survey scan. Raw upstream runs BW filter calibration
    /// inside every channel tune before its scanning-state early return.
    Survey,
    /// A configured association target's operating channel.
    Targeted,
}

/// Whether `set_channel` should issue its inline MCU CAL_BW commands.
///
/// Raw Linux `__mt7601u_phy_set_channel` runs `mt7601u_set_bw_filter(false)` on
/// scan hops before `mt7601u_phy_set_channel` returns early for
/// `MT7601U_STATE_SCANNING`. Keep that parity for CHANNEL=0 surveys. A failed
/// targeted tune does not consume the existing targeted one-shot.
pub const fn inline_cal_bw(path: TunePath, successful_targeted_tunes: u8) -> bool {
    match path {
        TunePath::Survey => true,
        TunePath::Targeted => successful_targeted_tunes != 0,
    }
}

/// Select the scan route from the optional configured channel. Channel zero is
/// the shipping scan-and-join contract; non-zero remains an optional targeted
/// fast path and is not required for correctness.
pub const fn scan_tune_path(configured_channel: u8) -> TunePath {
    if configured_channel == 0 {
        TunePath::Survey
    } else {
        TunePath::Targeted
    }
}

/// A successful tune that omitted CAL_BW must be completed after association.
/// Keeping this policy shared closes the last-channel edge where association is
/// already on the survey channel and therefore performs no targeted re-tune.
pub const fn deferred_cal_pending_after_tune(inline_cal_bw: bool) -> bool {
    !inline_cal_bw
}

/// Advance the one-shot only after the RF/LNA/VCO/TX-power tune completed.
pub const fn note_targeted_tune_success(successful_targeted_tunes: u8) -> u8 {
    successful_targeted_tunes.saturating_add(1)
}

/// Silicon traps retained after the f4a0940 ch7 RF17-20 timeout. These are
/// diagnostic labels, not recovery actions; the device path logs enough state to
/// distinguish them on the next bare-metal run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelSwitchTrap {
    AccumulatedMcuQueueDepth,
    RfFreqplanReadyPollWindow,
    DestructiveColdReloadRejected,
}

pub const RF_FREQPLAN_TIMEOUT_TRAPS: [ChannelSwitchTrap; 3] = [
    ChannelSwitchTrap::AccumulatedMcuQueueDepth,
    ChannelSwitchTrap::RfFreqplanReadyPollWindow,
    ChannelSwitchTrap::DestructiveColdReloadRejected,
];

pub const fn mcu_response_budget_has_upstream_margin(
    scan_budget: u32,
    upstream_equivalent_budget: u32,
) -> bool {
    scan_budget >= upstream_equivalent_budget
}

/// Evidence-neutral channel-command-path state after a programming failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelCommandPathState {
    Usable,
    /// The timeout does not distinguish transport desynchronization from MCU
    /// failure. No further command may be submitted in this device lifetime.
    Unusable,
}

/// The one truthful terminal category for a channel-programming failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelTerminal {
    ChannelCommandPathUnusable,
}

/// Fail-hard policy selected after any channel-programming operation fails.
///
/// Mainline Linux's MCU helper returns the timeout; the channel error path does
/// not vendor-reset and re-upload firmware. TEST 3 observed the rejected path
/// clear residency and then make zero upload progress. That rejects the forced
/// reload without identifying whether transport or the MCU caused the timeout.
///
/// Control-flow evidence was independently read in Linux `phy.c` (the ordered
/// channel-programming phases), `main.c` (error propagation), and `mcu.c`
/// (bounded command wait and firmware load). This is a locally derived policy
/// model; no GPL implementation text was copied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelFaultDisposition {
    pub command_path: ChannelCommandPathState,
    pub terminal: ChannelTerminal,
    pub attempt_destructive_reload: bool,
    pub continue_survey: bool,
    pub allow_association: bool,
    pub allow_dhcp: bool,
    pub allow_link_up: bool,
}

pub const fn channel_fault_disposition() -> ChannelFaultDisposition {
    ChannelFaultDisposition {
        command_path: ChannelCommandPathState::Unusable,
        terminal: ChannelTerminal::ChannelCommandPathUnusable,
        attempt_destructive_reload: false,
        continue_survey: false,
        allow_association: false,
        allow_dhcp: false,
        allow_link_up: false,
    }
}

/// One bounded, non-destructive transport probe is permitted before terminal
/// classification. Success of host endpoint recovery / device clear-halt is not
/// treated as MCU-liveness evidence; only the retried command completing proves
/// that the command path is usable again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelFailureAction {
    ProbeEndpointsAndRetry,
    ClassifyCommandPathUnusable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelFailureObservation {
    pub endpoint_probes_used: u8,
    pub operation_remaining: u64,
    pub probe_and_retry_reserve: u64,
}

pub const fn channel_failure_action(
    observation: ChannelFailureObservation,
) -> ChannelFailureAction {
    if observation.endpoint_probes_used == 0
        && observation.operation_remaining >= observation.probe_and_retry_reserve
    {
        ChannelFailureAction::ProbeEndpointsAndRetry
    } else {
        ChannelFailureAction::ClassifyCommandPathUnusable
    }
}

/// A host/device endpoint reset is not MCU command-path proof. The only safe
/// retry is one preceded by an explicit non-destructive MCU liveness proof; the
/// ch7 silicon run showed that repeating RF17-20 after endpoint-only recovery
/// can wedge without terminal evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelRetryAction {
    Retry,
    ClassifyCommandPathUnusable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelRetryObservation {
    pub endpoint_probe_reported_ok: bool,
    pub command_path_proven_usable: bool,
}

pub const fn channel_retry_action(observation: ChannelRetryObservation) -> ChannelRetryAction {
    if observation.endpoint_probe_reported_ok && observation.command_path_proven_usable {
        ChannelRetryAction::Retry
    } else {
        ChannelRetryAction::ClassifyCommandPathUnusable
    }
}

// ── Q-B: non-destructive command-path recovery ───────────────────────────────
//
// The `cea89e6a` silicon run showed the shape of the bug precisely: after an
// MCU timeout the endpoint probe reports the *transport* healthy while the MCU
// *command path* stays latched, and re-issuing the identical RF17-20 write
// against it wedges with no terminal. The recovery below is the minimal
// non-destructive sequence that can clear the latch and, more importantly,
// **prove** it cleared before any retry is permitted.
//
// Cold firmware reload is not a step and has no variant here: TEST 3 observed
// it clear residency and then upload zero bytes, so it is rejected by
// construction rather than by a runtime flag someone can flip.

/// The ordered, non-destructive recovery steps permitted after a
/// channel-programming failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelRecoveryStep {
    /// Host ring restart + device clear-halt on both MCU pipes. Already correct
    /// in CB-2. Reported success is transport evidence, never MCU liveness.
    EndpointRecovery,
    /// Issue one idempotent no-op MCU command per prior hop, each requesting a
    /// response, so accumulated unread response slots are retired.
    McuQueueDrain,
    /// One final response-bearing command whose observed `CMD_DONE` is the only
    /// accepted proof that the command path is usable again.
    CommandPathProbe,
}

/// The full recovery sequence, in order. The absence of a reload step is the
/// point: no ordering of these steps can reach a destructive cold reload.
pub const CHANNEL_RECOVERY_SEQUENCE: [ChannelRecoveryStep; 3] = [
    ChannelRecoveryStep::EndpointRecovery,
    ChannelRecoveryStep::McuQueueDrain,
    ChannelRecoveryStep::CommandPathProbe,
];

/// Ceiling on drain commands: a 14-channel survey has at most 13 prior hops, so
/// the drain can never outgrow one full survey's accumulation.
pub const MAX_MCU_DRAIN_COMMANDS: u8 = 13;

/// Drain depth: one no-op per hop completed since the command path was last
/// proven fresh, clamped to the survey span. Zero prior hops means nothing has
/// accumulated, so the drain is skipped and the probe runs alone.
pub const fn mcu_drain_depth(prior_hops: u8) -> u8 {
    if prior_hops > MAX_MCU_DRAIN_COMMANDS {
        MAX_MCU_DRAIN_COMMANDS
    } else {
        prior_hops
    }
}

/// The drain must not be able to change device state. The only commands that
/// qualify are ones the device already accepted this boot, re-issued with
/// byte-identical arguments — the post-firmware `Q_SELECT` function-select is
/// exactly that, so re-selecting the same queue is an observable no-op whose
/// sole effect is retiring one response slot.
pub const fn drain_command_is_idempotent_noop(
    func: u32,
    val: u32,
    accepted_func: u32,
    accepted_val: u32,
) -> bool {
    func == accepted_func && val == accepted_val
}

/// What the recovery attempt observed. Drain counts are diagnostic only — they
/// feed the discriminator bank ("did the drain depth match a ~12-deep response
/// FIFO?") and never gate the retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelRecoveryObservation {
    pub endpoint_probe_reported_ok: bool,
    pub drain_commands_issued: u8,
    pub drain_cmd_done_observed: u8,
    /// The probe command completed its transfer at all.
    pub probe_command_completed: bool,
    /// The probe's response decoded as the awaited seq + `CMD_DONE`.
    pub probe_cmd_done_observed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelRecoveryVerdict {
    CommandPathReproven,
    CommandPathStillLatched,
}

/// A completed transfer is not a proof; only the decoded `CMD_DONE` is. This is
/// the same distinction the endpoint probe got wrong, applied one layer down —
/// the transport can complete a TD while the MCU never answers.
pub const fn classify_channel_recovery(
    observation: ChannelRecoveryObservation,
) -> ChannelRecoveryVerdict {
    if observation.endpoint_probe_reported_ok
        && observation.probe_command_completed
        && observation.probe_cmd_done_observed
    {
        ChannelRecoveryVerdict::CommandPathReproven
    } else {
        ChannelRecoveryVerdict::CommandPathStillLatched
    }
}

/// Feed the recovery verdict into the existing retry gate.
pub const fn recovery_retry_observation(
    observation: ChannelRecoveryObservation,
) -> ChannelRetryObservation {
    ChannelRetryObservation {
        endpoint_probe_reported_ok: observation.endpoint_probe_reported_ok,
        command_path_proven_usable: matches!(
            classify_channel_recovery(observation),
            ChannelRecoveryVerdict::CommandPathReproven
        ),
    }
}

/// The ledger cost of the recovery sequence, so the operation budget can be
/// sized against it instead of hoping it fits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryBudgetPlan {
    pub drain_commands: u8,
    /// Response-wait attempts per command (the MCU wait loop's outer bound).
    pub attempts_per_command: u32,
    pub drain_polls_per_attempt: u32,
    pub probe_polls_per_attempt: u32,
}

/// Worst-case charge of drain + probe against the non-replenishing channel
/// ledger. Every attempt charges one bulk attempt plus its poll window.
pub const fn recovery_worst_case_cost(plan: RecoveryBudgetPlan) -> u64 {
    let attempts = plan.attempts_per_command as u64;
    let drain = (plan.drain_commands as u64) * attempts * (plan.drain_polls_per_attempt as u64 + 1);
    let probe = attempts * (plan.probe_polls_per_attempt as u64 + 1);
    drain.saturating_add(probe)
}

/// The reserve gating `ProbeEndpointsAndRetry` must fund the whole recovery
/// sequence **and** the retry it enables. Asserted as a host test so a future
/// budget edit cannot silently starve the retry it just authorized.
pub const fn recovery_reserve_covers(
    plan: RecoveryBudgetPlan,
    reserve: u64,
    retry_cost: u64,
) -> bool {
    match recovery_worst_case_cost(plan).checked_add(retry_cost) {
        Some(total) => reserve >= total,
        None => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelOperationEvidence {
    pub limit: u64,
    pub consumed: u64,
    pub remaining: u64,
    pub exhausted: bool,
    pub fence_refusals: u64,
}

pub const fn channel_operation_evidence(
    limit: u64,
    remaining: u64,
    exhausted: bool,
    fence_refusals: u64,
) -> ChannelOperationEvidence {
    ChannelOperationEvidence {
        limit,
        consumed: limit.saturating_sub(remaining),
        remaining,
        exhausted,
        fence_refusals,
    }
}

/// The hostile TEST 3 trace retained as a pure host oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RejectedRecoveryTrace {
    pub channels_completed: u8,
    pub failed_channel: u8,
    pub failed_phase: ChannelFailurePhase,
    pub cal_bw_reached: bool,
    pub host_inband_recovered: bool,
    pub host_response_recovered: bool,
    pub device_inband_clear_halt: bool,
    pub device_response_clear_halt: bool,
    pub reload_uploaded_bytes: u32,
    pub reload_expected_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelFailurePhase {
    RfFreqplanRf17To20RandomWrite,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectedRecoveryVerdict {
    DifferentTrace,
    /// Endpoint recovery succeeded, but the destructive firmware reload made
    /// zero progress. This rejects reload; it does not identify whether the MCU
    /// or the USB command transport failed.
    ReloadFailedAtByteZero,
}

pub const fn classify_rejected_recovery(trace: RejectedRecoveryTrace) -> RejectedRecoveryVerdict {
    if trace.channels_completed == 6
        && trace.failed_channel == 7
        && matches!(
            trace.failed_phase,
            ChannelFailurePhase::RfFreqplanRf17To20RandomWrite
        )
        && !trace.cal_bw_reached
        && trace.host_inband_recovered
        && trace.host_response_recovered
        && trace.device_inband_clear_halt
        && trace.device_response_clear_halt
        && trace.reload_uploaded_bytes == 0
        && trace.reload_expected_bytes != 0
    {
        RejectedRecoveryVerdict::ReloadFailedAtByteZero
    } else {
        RejectedRecoveryVerdict::DifferentTrace
    }
}

/// Pure hostile-host result used to prove the terminal contract without USB
/// hardware. This models only policy, not Linux implementation code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelFailureOracle {
    pub bounded_completion: bool,
    pub terminal_count: u8,
    pub attempted_destructive_reload: bool,
    pub commands_after_terminal: u32,
    pub association_reached: bool,
    pub dhcp_reached: bool,
    pub link_up_reached: bool,
    pub pass_reached: bool,
    pub terminal_sink_available: bool,
    pub budget_summary_available: bool,
    pub fence_summary_available: bool,
    pub operation_evidence: ChannelOperationEvidence,
}

/// Fixed disposition for the rejected TEST-3 trace.
pub const fn terminal_channel_failure_oracle(
    trace: RejectedRecoveryTrace,
    operation_evidence: ChannelOperationEvidence,
) -> ChannelFailureOracle {
    let matched = matches!(
        classify_rejected_recovery(trace),
        RejectedRecoveryVerdict::ReloadFailedAtByteZero
    );
    ChannelFailureOracle {
        bounded_completion: matched,
        terminal_count: if matched { 1 } else { 0 },
        attempted_destructive_reload: false,
        commands_after_terminal: 0,
        association_reached: false,
        dhcp_reached: false,
        link_up_reached: false,
        pass_reached: false,
        terminal_sink_available: matched,
        budget_summary_available: matched,
        fence_summary_available: matched,
        operation_evidence,
    }
}

/// Negative control for the removed reset/reload behavior. It records why that
/// path is rejected: it attempts the destructive reload and cannot reach the
/// terminal sink/budget/fence evidence after the byte-zero failure.
pub const fn rejected_reload_oracle(
    trace: RejectedRecoveryTrace,
    operation_evidence: ChannelOperationEvidence,
) -> ChannelFailureOracle {
    let matched = matches!(
        classify_rejected_recovery(trace),
        RejectedRecoveryVerdict::ReloadFailedAtByteZero
    );
    ChannelFailureOracle {
        bounded_completion: matched,
        terminal_count: 0,
        attempted_destructive_reload: matched,
        commands_after_terminal: if matched { 1 } else { 0 },
        association_reached: false,
        dhcp_reached: false,
        link_up_reached: false,
        pass_reached: false,
        terminal_sink_available: false,
        budget_summary_available: false,
        fence_summary_available: false,
        operation_evidence,
    }
}

pub mod channel_contract_violation {
    pub const UNBOUNDED: u16 = 1 << 0;
    pub const TERMINAL_COUNT: u16 = 1 << 1;
    pub const DESTRUCTIVE_RELOAD: u16 = 1 << 2;
    pub const COMMAND_AFTER_TERMINAL: u16 = 1 << 3;
    pub const LATER_NETWORK_STAGE: u16 = 1 << 4;
    pub const TERMINAL_SINK_MISSING: u16 = 1 << 5;
    pub const BUDGET_SUMMARY_MISSING: u16 = 1 << 6;
    pub const FENCE_SUMMARY_MISSING: u16 = 1 << 7;
    pub const INVALID_OPERATION_ACCOUNTING: u16 = 1 << 8;
}

/// Contract checker shared by the hostile host mutation matrix. Zero is green;
/// every set bit names an independently falsifiable violation.
pub const fn channel_contract_violations(oracle: ChannelFailureOracle) -> u16 {
    use channel_contract_violation as violation;

    let mut violations = 0;
    if !oracle.bounded_completion {
        violations |= violation::UNBOUNDED;
    }
    if oracle.terminal_count != 1 {
        violations |= violation::TERMINAL_COUNT;
    }
    if oracle.attempted_destructive_reload {
        violations |= violation::DESTRUCTIVE_RELOAD;
    }
    if oracle.commands_after_terminal != 0 {
        violations |= violation::COMMAND_AFTER_TERMINAL;
    }
    if oracle.association_reached
        || oracle.dhcp_reached
        || oracle.link_up_reached
        || oracle.pass_reached
    {
        violations |= violation::LATER_NETWORK_STAGE;
    }
    if !oracle.terminal_sink_available {
        violations |= violation::TERMINAL_SINK_MISSING;
    }
    if !oracle.budget_summary_available {
        violations |= violation::BUDGET_SUMMARY_MISSING;
    }
    if !oracle.fence_summary_available {
        violations |= violation::FENCE_SUMMARY_MISSING;
    }
    let account = oracle.operation_evidence;
    if account.consumed == 0 || account.consumed != account.limit.saturating_sub(account.remaining)
    {
        violations |= violation::INVALID_OPERATION_ACCOUNTING;
    }
    violations
}
