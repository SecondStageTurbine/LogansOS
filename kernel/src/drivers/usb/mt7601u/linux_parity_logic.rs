// SPDX-License-Identifier: MPL-2.0
//! Pure Linux-parity policy for MT7601U firmware and post-Q_SELECT ordering.
//!
//! The device-touching driver consumes these decisions. Host tests import this
//! file directly so the resident-firmware, RX-ownership, and MAC-init terminal
//! contracts cannot drift into a test-only model.

#![allow(dead_code)]

/// One finite, non-replenishing ledger from the first MAC RANDOM_WRITE after
/// post-Q_SELECT receive ownership through successful EEPROM acquisition.
///
/// 250k retains the already-proven 50k MAC allowance, covers the 100,001-us
/// MAC-status poll, BBP readiness/tables, all three WCID-family bursts, and the
/// bounded eFUSE polls without reopening a nested ledger. Individual waits keep
/// their existing Linux-shaped limits; this is the enclosing fail-stop ceiling.
pub const POST_QSELECT_OPERATION_BUDGET: u64 = 250_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PostQselectStage {
    MacCommon = 0,
    MacChip = 1,
    MacAux = 2,
    MacStatus = 3,
    BbpReady = 4,
    BbpCommon = 5,
    BbpChip = 6,
    WcidAddress = 7,
    SharedKeyMode = 8,
    WcidAttribute = 9,
    InterveningRegisters = 10,
    EepromAcquire = 11,
}

impl PostQselectStage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::MacCommon => "mac_common",
            Self::MacChip => "mac_chip",
            Self::MacAux => "mac_aux",
            Self::MacStatus => "mac_status",
            Self::BbpReady => "bbp_ready",
            Self::BbpCommon => "bbp_common",
            Self::BbpChip => "bbp_chip",
            Self::WcidAddress => "wcid_address",
            Self::SharedKeyMode => "shared_key_mode",
            Self::WcidAttribute => "wcid_attribute",
            Self::InterveningRegisters => "intervening_registers",
            Self::EepromAcquire => "eeprom_acquire",
        }
    }

    pub const fn bit(self) -> u16 {
        1u16 << self as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FirmwareLoadAction {
    /// Linux returns immediately after the initial USB-DMA enable when COM_REG0
    /// reports a running firmware image.
    UseResident,
    /// No resident firmware was observed; execute the existing cold upload.
    UploadCold,
}

pub const fn firmware_load_action(firmware_running: bool) -> FirmwareLoadAction {
    if firmware_running {
        FirmwareLoadAction::UseResident
    } else {
        FirmwareLoadAction::UploadCold
    }
}

/// Observable device mutations made by a firmware-load branch. This is used by
/// the hostile oracle to pin Linux's resident early-return semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirmwareLoadEffects {
    pub preload_runs: u8,
    pub vendor_resets: u8,
    pub fce_pbf_writes: u8,
    pub cold_uploads: u8,
}

pub const fn firmware_load_effects(action: FirmwareLoadAction) -> FirmwareLoadEffects {
    match action {
        FirmwareLoadAction::UseResident => FirmwareLoadEffects {
            preload_runs: 0,
            vendor_resets: 0,
            fce_pbf_writes: 0,
            cold_uploads: 0,
        },
        FirmwareLoadAction::UploadCold => FirmwareLoadEffects {
            preload_runs: 1,
            vendor_resets: 1,
            fce_pbf_writes: 1,
            cold_uploads: 1,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketRxAction {
    /// Allocate and post exactly the missing packet-RX buffers.
    PostMissing { count: usize },
    /// The Linux-equivalent pool is already owned by the packet-RX endpoint.
    ReusePosted,
    /// More buffers are recorded than the fixed pool permits.
    InvalidOwnership,
}

pub const fn packet_rx_action(posted: usize, required: usize) -> PacketRxAction {
    if posted > required {
        PacketRxAction::InvalidOwnership
    } else if posted == required {
        PacketRxAction::ReusePosted
    } else {
        PacketRxAction::PostMissing {
            count: required - posted,
        }
    }
}

/// Production precondition for entering the first MAC RANDOM_WRITE.
pub const fn ready_for_mac_init(
    command_response_owned: bool,
    packet_rx_posted: usize,
    packet_rx_required: usize,
) -> bool {
    command_response_owned && packet_rx_posted == packet_rx_required
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostQselectFailureClass {
    BulkOutTimeout,
    ResponseTimeout,
    OperationBudget,
    Other,
}

pub fn classify_post_qselect_failure(reason: &str) -> PostQselectFailureClass {
    if reason.contains("bulk-out") && reason.contains("timeout") {
        PostQselectFailureClass::BulkOutTimeout
    } else if reason.contains("response") && reason.contains("timed out") {
        PostQselectFailureClass::ResponseTimeout
    } else if reason.contains("operation budget exhausted") {
        PostQselectFailureClass::OperationBudget
    } else {
        PostQselectFailureClass::Other
    }
}

impl PostQselectFailureClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::BulkOutTimeout => "bulk_out_timeout",
            Self::ResponseTimeout => "response_timeout",
            Self::OperationBudget => "operation_budget",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostQselectTerminal {
    OperationFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostQselectFailureDisposition {
    pub terminal: PostQselectTerminal,
    pub stop: bool,
    pub attempt_firmware_reload: bool,
    pub allow_later_device_traffic: bool,
    pub flush_serial_and_sink: bool,
}

/// Evidence-neutral, fail-hard behavior for any MAC-init operation failure.
pub const fn post_qselect_failure_disposition() -> PostQselectFailureDisposition {
    PostQselectFailureDisposition {
        terminal: PostQselectTerminal::OperationFailed,
        stop: true,
        attempt_firmware_reload: false,
        allow_later_device_traffic: false,
        flush_serial_and_sink: true,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostQselectTerminalAction {
    Emit,
    SuppressDuplicate,
}

/// Production pre-CAS decision for the one-terminal latch. The atomic CAS in
/// the driver remains the race-closing authority.
pub const fn post_qselect_terminal_action(
    terminal_already_latched: bool,
) -> PostQselectTerminalAction {
    if terminal_already_latched {
        PostQselectTerminalAction::SuppressDuplicate
    } else {
        PostQselectTerminalAction::Emit
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostQselectTrafficAction {
    Allow,
    Block,
}

/// Production command gate after a MAC-init terminal. Hostile tests consume
/// this same seam, so zero continued traffic is derived rather than declared.
pub const fn post_qselect_traffic_action(command_path_unusable: bool) -> PostQselectTrafficAction {
    if command_path_unusable {
        PostQselectTrafficAction::Block
    } else {
        PostQselectTrafficAction::Allow
    }
}

pub mod post_qselect_boundary {
    pub const BBP_BEFORE: u8 = 1 << 0;
    pub const BBP_AFTER: u8 = 1 << 1;
    pub const WCID_BEFORE: u8 = 1 << 2;
    pub const WCID_AFTER: u8 = 1 << 3;
    pub const EEPROM_BEFORE: u8 = 1 << 4;
    pub const EEPROM_AFTER: u8 = 1 << 5;
    pub const ALL: u8 =
        BBP_BEFORE | BBP_AFTER | WCID_BEFORE | WCID_AFTER | EEPROM_BEFORE | EEPROM_AFTER;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostQselectOutcome {
    Success,
    Failed(PostQselectStage),
}

/// Observed trace consumed by both production-bound hostile tests and their
/// mutations. The verifier checks the same stage, terminal, traffic-gate, and
/// flush decisions used by the driver rather than a parallel policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostQselectOperationTrace {
    pub outcome: PostQselectOutcome,
    pub enclosing_budget: bool,
    pub budget_starts: u8,
    pub stages_entered: u16,
    pub boundary_flushes: u8,
    pub terminal_count: u8,
    pub terminal_flushed: bool,
    pub device_traffic_after_terminal: u32,
    pub device_traffic_blocked_after_terminal: u32,
}

pub mod post_qselect_violation {
    pub const MISSING_ENCLOSING_BUDGET: u16 = 1 << 0;
    pub const BUDGET_REPLENISHED: u16 = 1 << 1;
    pub const STAGE_NOT_ENTERED: u16 = 1 << 2;
    pub const TERMINAL_COUNT: u16 = 1 << 3;
    pub const TERMINAL_NOT_FLUSHED: u16 = 1 << 4;
    pub const TRAFFIC_AFTER_TERMINAL: u16 = 1 << 5;
    pub const MISSING_BOUNDARY_FLUSH: u16 = 1 << 6;
    pub const POST_TERMINAL_GATE_NOT_EXERCISED: u16 = 1 << 7;
}

pub const fn required_boundary_flushes(outcome: PostQselectOutcome) -> u8 {
    use PostQselectStage as stage;
    use post_qselect_boundary as boundary;

    match outcome {
        PostQselectOutcome::Success => boundary::ALL,
        PostQselectOutcome::Failed(failed) => match failed {
            stage::MacCommon | stage::MacChip | stage::MacAux | stage::MacStatus => 0,
            stage::BbpReady | stage::BbpCommon | stage::BbpChip => boundary::BBP_BEFORE,
            stage::WcidAddress | stage::SharedKeyMode | stage::WcidAttribute => {
                boundary::BBP_BEFORE | boundary::BBP_AFTER | boundary::WCID_BEFORE
            }
            stage::InterveningRegisters => {
                boundary::BBP_BEFORE
                    | boundary::BBP_AFTER
                    | boundary::WCID_BEFORE
                    | boundary::WCID_AFTER
            }
            stage::EepromAcquire => {
                boundary::BBP_BEFORE
                    | boundary::BBP_AFTER
                    | boundary::WCID_BEFORE
                    | boundary::WCID_AFTER
                    | boundary::EEPROM_BEFORE
            }
        },
    }
}

pub const fn post_qselect_operation_violations(trace: PostQselectOperationTrace) -> u16 {
    use post_qselect_violation as violation;

    let mut violations = 0;
    if !trace.enclosing_budget {
        violations |= violation::MISSING_ENCLOSING_BUDGET;
    }
    if trace.budget_starts != 1 {
        violations |= violation::BUDGET_REPLENISHED;
    }
    let required_flushes = required_boundary_flushes(trace.outcome);
    if trace.boundary_flushes & required_flushes != required_flushes {
        violations |= violation::MISSING_BOUNDARY_FLUSH;
    }
    match trace.outcome {
        PostQselectOutcome::Success => {
            if trace.terminal_count != 0 {
                violations |= violation::TERMINAL_COUNT;
            }
        }
        PostQselectOutcome::Failed(stage) => {
            if trace.stages_entered & stage.bit() == 0 {
                violations |= violation::STAGE_NOT_ENTERED;
            }
            if trace.terminal_count != 1 {
                violations |= violation::TERMINAL_COUNT;
            }
            if !trace.terminal_flushed {
                violations |= violation::TERMINAL_NOT_FLUSHED;
            }
            if trace.device_traffic_after_terminal != 0 {
                violations |= violation::TRAFFIC_AFTER_TERMINAL;
            }
            if trace.device_traffic_blocked_after_terminal == 0 {
                violations |= violation::POST_TERMINAL_GATE_NOT_EXERCISED;
            }
        }
    }
    violations
}

pub mod parity_violation {
    pub const RESIDENT_PRELOAD: u8 = 1 << 0;
    pub const RESIDENT_VENDOR_RESET: u8 = 1 << 1;
    pub const RESIDENT_FCE_PBF_WRITE: u8 = 1 << 2;
    pub const RESIDENT_COLD_UPLOAD: u8 = 1 << 3;
    pub const RESPONSE_NOT_OWNED: u8 = 1 << 4;
    pub const PACKET_RX_NOT_READY: u8 = 1 << 5;
}

/// Hostile-oracle trace at the exact boundary before MAC initialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostQselectTrace {
    pub resident_effects: FirmwareLoadEffects,
    pub command_response_owned: bool,
    pub packet_rx_posted: usize,
    pub packet_rx_required: usize,
}

pub const fn post_qselect_parity_violations(trace: PostQselectTrace) -> u8 {
    use parity_violation as violation;

    let mut violations = 0;
    if trace.resident_effects.preload_runs != 0 {
        violations |= violation::RESIDENT_PRELOAD;
    }
    if trace.resident_effects.vendor_resets != 0 {
        violations |= violation::RESIDENT_VENDOR_RESET;
    }
    if trace.resident_effects.fce_pbf_writes != 0 {
        violations |= violation::RESIDENT_FCE_PBF_WRITE;
    }
    if trace.resident_effects.cold_uploads != 0 {
        violations |= violation::RESIDENT_COLD_UPLOAD;
    }
    if !trace.command_response_owned {
        violations |= violation::RESPONSE_NOT_OWNED;
    }
    if trace.packet_rx_posted != trace.packet_rx_required {
        violations |= violation::PACKET_RX_NOT_READY;
    }
    violations
}
