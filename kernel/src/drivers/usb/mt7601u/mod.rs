// SPDX-License-Identifier: MPL-2.0
//! MT7601U USB SoftMAC driver — Phase 7.5 W1 (scan) → W2 (connect: MLME auth/
//! assoc, WPA2 4-way, CCMP, `NetDevice`) → OT-27a/OT-2 (cold firmware self-upload
//! + CSPRNG SNonce). ANSPEC §10.10/§10.11, ADR-054/ADR-057. The WiFi arc meeting
//!   silicon: enumerate the `148f:7601` dongle on the 7.5-A/B USB core, upload its
//!   firmware from a COLD device, read eFUSE, bring up MAC/PHY, scan real 2.4 GHz
//!   BSSIDs, and associate to a WPA2 AP as a real security boundary.
//!
//! **Faithfulness:** the bring-up sequence is the Linux `mt7601u` driver
//! (v6.6, GPL-2.0), `RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md
//! §1`. The byte layouts (firmware header/chunk, MCU framing, eFUSE decode,
//! RXWI/beacon) and the cold-upload fault classifier are the host-oracle'd pure
//! cores ([`fw_logic`], [`mcu_logic`], [`eeprom_logic`], [`rx_logic`],
//! [`upload_logic`]) — proven against hand-laid fixtures in `tools/ai-ops-tests`
//! (I-12) so this device layer only adds the USB transfers, never re-derives the
//! bytes.
//!
//! **I-14 containment (DISPATCH §3 / W2 THINK §4 — restated, no new guard):**
//! - (a) lock isolation: the driver state is one `SwitchMutex` at
//!   `lock_order::USB_DEVICE` (29, above every AI level); the bulk/event path
//!   takes NO AI-governance lock — it reuses the xHCI bounded ISR drain.
//! - (b) liveness: RX is drained in bounded passes (the 7.5-B `EP_QUEUE_CAP`
//!   pattern); a firmware/RX flood degrades USB, never governance.
//! - (c) memory isolation: every DMA address is a [`FencedDmaAddr`] from the
//!   7.5-0 arena; xHCI mediates all device DMA and sits in VT-d's fence domain
//!   (7.5-V, silicon-proven). The OT-27a cold-upload path adds NO new (c) vector:
//!   its FCE-DMA firmware chunks reuse the fenced `staging` buffer and the IVB
//!   rides a host-managed control transfer (no device-initiated DMA) — re-assert
//!   `fence_refusals=0` on the Stage-B. USB contains the firmware adversary
//!   structurally.
//!
//! **Kill-switch (Rule 1):** the whole module is `#[cfg(feature = "mt7601u")]`;
//! off ⇒ the driver and the PHY tables are not compiled and the kernel is
//! bit-for-bit today. OT-27a: the firmware blob is NO LONGER compiled in — it is
//! staged as a Limine boot module (`/modules/mt7601u-fw`, not in git) and looked
//! up at runtime via [`firmware_blob`]. OT-2: `wifi` pulls `kernel-rng`, so the
//! 4-way handshake's SNonce is a CSPRNG draw (a security boundary), not a counter.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::drivers::block::DmaBuffer;
use crate::drivers::dma_arena::{self, FencedDmaAddr};
// W2 ISC-1: the MLME frame/state logic lives in net/wifi (both gated on `wifi`).
use crate::net::wifi::mlme_logic;
// W2 ISC-4: the NetDevice seam (present the associated link to the net stack).
use crate::net::device::{
    LinkState, NetDevice, NetDeviceIdentity, NetError, NetResult, RxFrame, TxFrame,
};
use crate::sync::SwitchMutex;

use super::operation_budget_logic::ChargeKind;
use super::usb_parse::UsbEndpoint;
use super::xhci::{self, ClaimedDevice, EndpointPipe};
use super::{UsbClassDriver, UsbDevice};

pub mod eeprom_logic;
pub mod fw_logic;
pub mod initvals;
pub mod initvals_phy;
pub mod key_logic;
pub mod link_result_logic;
pub mod linux_parity_logic;
pub mod mcu_logic;
pub mod phy;
pub mod post_fw_logic;
pub mod regs;
pub mod rx_logic;
pub mod tx_logic;
pub mod upload_logic;
pub mod wedge_logic;

/// The MT7601U firmware blob is the vendor binary (linux-firmware
/// `mediatek/mt7601u.bin`, v34, 45,412 bytes, "Redistributable" with WHENCE
/// attribution in NOTICE). OT-27a (ISC-2): it is **NOT committed to git** — a
/// vendor binary + licensing, the same discipline as the AI model blob. xtask
/// stages it as a **Limine boot module** at `/modules/mt7601u-fw` for WiFi ISOs
/// (only when the blob is present on disk), and this driver looks it up at
/// bring-up via the same module mechanism the userspace ELFs use. Decoupling the
/// blob from `include_bytes!` also lets a fresh (blob-less) checkout *compile* the
/// WiFi kernel — the blob is only needed at ISO-assembly + run time.
const FIRMWARE_MODULE_SUFFIX: &str = "/mt7601u-fw";

/// Locate the firmware blob staged as a Limine boot module. **Fail-closed:** an
/// absent module (a WiFi ISO assembled without the out-of-band vendor blob)
/// yields a clear error, which the caller turns into a clean bring-up failure —
/// never a crash. Only the *cold* path needs it; a warm-resident MCU (COM_REG0=1)
/// skips the upload before this is called, so a warm device needs no module.
fn firmware_blob() -> Result<&'static [u8], &'static str> {
    crate::find_module_by_path_suffix(FIRMWARE_MODULE_SUFFIX).ok_or(
        "mt7601u: firmware module /mt7601u-fw not found — stage the vendor blob (not in git)",
    )
}

/// Linux's `mt7601u_vendor_request` (usb.c) retries every vendor control transfer
/// up to `MT_VEND_REQ_MAX_RETRY` times with a 5 ms gap (`msleep(5)`).
const VEND_REQ_MAX_RETRY: usize = 10;

/// BUILD-2 C4 (the wedge fix): retry a vendor control transfer like Linux does.
///
/// The W1 driver issued each vendor control transfer **once** — fine on the warm
/// path, where the device answers immediately. But the cold firmware-DMA phase
/// transiently NAKs vendor control transfers, and a single un-retried failure
/// there returns the all-ones sentinel (read) / a "no event" error (write), which
/// wedged the cold upload — the BUILD-1-on-silicon signature. Linux survives this
/// by retrying up to `VEND_REQ_MAX_RETRY` times with a 5 ms gap; this is a
/// faithful port (`usb.c` `for (i = 0; i < MAX_RETRY; i++) { … msleep(5); }`).
/// Healthy transfers succeed on the first attempt ⇒ no behavior change warm.
fn vend_retry<T>(mut op: impl FnMut() -> Result<T, &'static str>) -> Result<T, &'static str> {
    let mut last: Result<T, &'static str> = Err("vendor request: no attempt");
    for attempt in 0..VEND_REQ_MAX_RETRY {
        if !xhci::charge_operation(ChargeKind::VendorAttempt) {
            return Err("post-firmware operation budget exhausted before vendor attempt");
        }
        match op() {
            ok @ Ok(_) => return ok,
            err => {
                last = err;
                if attempt + 1 < VEND_REQ_MAX_RETRY {
                    if !xhci::charge_operation(ChargeKind::VendorRetryGap) {
                        return Err(
                            "post-firmware operation budget exhausted before vendor retry gap",
                        );
                    }
                    msleep(5);
                }
            }
        }
    }
    xhci::note_vendor_request_exhaustion();
    last
}

/// RX buffers posted on the packet-RX pipe for the scan.
const RX_BUFFERS: usize = 16;
/// One RX/MCU/firmware staging buffer: ≥ 4 (DMA hdr) + 14336 (max fw chunk) + 4.
const STAGING_LEN: usize = 16 * 1024;
/// One RX buffer holds an aggregated burst of 802.11 frames.
const RX_BUF_LEN: usize = 4096;
/// MCU response buffer (`MCU_RESP_URB_SIZE`).
const MCU_RESP_LEN: usize = 1024;
/// 802.11 TX staging: the wrapped [TXINFO][TXWI][frame][pad][zero]. A mgmt frame
/// (assoc-req with IEs) is ~150 B; 2 KiB leaves room for data frames (ISC-4).
const TX_STAGING_LEN: usize = 2048;
/// Bound on poll spins for one bulk completion (liveness — never an unbounded
/// wait on a dead device; the breadcrumb says which stage stalled). Generous:
/// covers the multi-KiB firmware-chunk DMA AND the MCU calibration responses
/// (a tighter per-retry bound re-posted the resp buffer too often and desynced
/// the calibration phase on silicon — keep the proven value).
const BULK_POLL_SPINS: u32 = 200_000;
/// ISC-0 (F1-part-2): the bulk-poll budget for scan/targeted channel MCU
/// responses. Upstream waits about 500 ms per response attempt; at our 5 us
/// poll cadence, 100k polls gives the same order of tolerance. f4a0940 showed
/// the old 8k window was too tight after consecutive channel switches, not that
/// channel 7 is special.
const SCAN_BULK_BUDGET: u32 = 100_000;
/// The value paired with `FUNC_Q_SELECT` in the post-firmware function-select.
/// Shared with the Q-B drain/proof no-op so the two can never drift: the no-op
/// is only an observable no-op while it re-issues *this* accepted operation.
const POST_FW_Q_SELECT_VALUE: u32 = 1;
/// Response-wait attempts per MCU command — the outer bound of
/// [`Device::mcu_wait_resp`]. Named so the recovery budget arithmetic can be
/// asserted against the same number the wait loop actually uses.
const MCU_RESPONSE_ATTEMPTS: u32 = 5;
/// Per-attempt poll window for one drain no-op (~10 ms at the 5 us cadence).
/// The drain is not waiting for slow silicon work — each no-op either retires a
/// stale response promptly or is written off — so it stays far tighter than the
/// scan budget and cannot eat the retry it exists to enable.
const CHANNEL_DRAIN_BULK_BUDGET: u32 = 2_000;
/// Per-attempt poll window for the command-path proof (~100 ms). More generous
/// than a drain no-op because a false negative here needlessly terminalizes a
/// device that had actually recovered.
const CHANNEL_PROBE_BULK_BUDGET: u32 = 20_000;
/// Worst-case ledger charge of one complete drain + probe sequence, computed
/// from the constants above. `wedge_logic::recovery_reserve_covers` asserts the
/// reserve funds this plus the retry it authorizes.
const CHANNEL_RECOVERY_RESERVE: u64 = (wedge_logic::MAX_MCU_DRAIN_COMMANDS as u64)
    * (MCU_RESPONSE_ATTEMPTS as u64)
    * (CHANNEL_DRAIN_BULK_BUDGET as u64 + 1)
    + (MCU_RESPONSE_ATTEMPTS as u64) * (CHANNEL_PROBE_BULK_BUDGET as u64 + 1);
/// Ledger required for one complete bounded retry at the scan budget.
const CHANNEL_RETRY_RESERVE: u64 = 525_000;
/// Minimum live ledger required to perform one host/device endpoint probe, the
/// full non-destructive recovery sequence, and one complete bounded retry while
/// retaining room for terminal accounting.
const CHANNEL_PROBE_AND_RETRY_RESERVE: u64 = CHANNEL_RETRY_RESERVE + CHANNEL_RECOVERY_RESERVE;
/// One non-replenishing ledger for the complete channel-programming operation.
/// It includes every nested xHCI wait, MCU bulk attempt, and MCU response poll.
/// Sized so a first failure that burns a full scan-budget response wait still
/// leaves the probe-and-retry reserve live.
const CHANNEL_OPERATION_BUDGET: u64 = 1_100_000 + CHANNEL_RECOVERY_RESERVE;

/// The recovery plan the constants above actually describe.
const CHANNEL_RECOVERY_PLAN: wedge_logic::RecoveryBudgetPlan = wedge_logic::RecoveryBudgetPlan {
    drain_commands: wedge_logic::MAX_MCU_DRAIN_COMMANDS,
    attempts_per_command: MCU_RESPONSE_ATTEMPTS,
    drain_polls_per_attempt: CHANNEL_DRAIN_BULK_BUDGET,
    probe_polls_per_attempt: CHANNEL_PROBE_BULK_BUDGET,
};

// The reserve that authorizes `ProbeEndpointsAndRetry` must fund the whole
// recovery sequence *and* the retry it enables — otherwise the driver would
// grant itself a retry it cannot pay for and exhaust mid-RF-write, which is the
// budget-shaped version of the `cea89e6a` failure. Compile-time so a future
// edit to any one constant cannot silently break the relationship.
const _: () = assert!(
    CHANNEL_RECOVERY_RESERVE == wedge_logic::recovery_worst_case_cost(CHANNEL_RECOVERY_PLAN)
);
const _: () = assert!(wedge_logic::recovery_reserve_covers(
    CHANNEL_RECOVERY_PLAN,
    CHANNEL_PROBE_AND_RETRY_RESERVE,
    CHANNEL_RETRY_RESERVE,
));
// A first failure that burns a full scan-budget response wait must still leave
// the reserve live, or the recovery path is unreachable in exactly the case it
// was built for.
const _: () = assert!(
    CHANNEL_OPERATION_BUDGET - (MCU_RESPONSE_ATTEMPTS as u64) * (SCAN_BULK_BUDGET as u64 + 1)
        >= CHANNEL_PROBE_AND_RETRY_RESERVE
);
/// Poll windows used between successful survey hops to retire stale MCU
/// response-slot completions before the next RF RANDOM_WRITE. This is not a
/// liveness proof and never sends an MCU command.
const CHANNEL_BETWEEN_HOP_DRAIN_POLLS: u32 = 64;
/// The `wait_event` budget during the COLD firmware-upload phase only: a
/// wedged control transfer fails in seconds (vs ~minutes at the default 5M),
/// so the retry can recover or fail-fast-with-diagnostics instead of stalling.
/// Valid transfers complete in a handful of iterations — far under this.
const FW_LOAD_WAIT_BUDGET: u64 = 300_000;
/// One non-replenishing structural budget for each post-firmware control-plane
/// boundary. Every xHCI event-wait call/iteration, vendor attempt/retry gap,
/// register observation/poll gap, and bulk-response attempt/poll consumes one
/// unit from the same ledger.
///
/// Therefore the complete nested ceiling is the sum of all charged counters,
/// never their product:
/// `wait_calls + wait_iterations + vendor_attempts + vendor_retry_gaps +
/// observations + poll_gaps + bulk_attempts + bulk_polls <= 50_000`.
/// Linux-shaped limits remain nested inside it (<=10 vendor attempts/request,
/// <=8 tolerated transfer events/control request, <=10,001 WPDMA observations).
const POST_FW_OPERATION_BUDGET: u64 = 50_000;
/// Per-channel scan dwell: passes × the ~2ms step between them. Beacons arrive
/// every ~100ms, so each channel must be listened to long enough to catch one —
/// ~600ms nominal per channel here (robust to the coarse spin calibration; even
/// 10× off either way stays useful and well within the scenario timeout).
const SCAN_DWELL_PASSES: u32 = 300;
const SCAN_DWELL_STEP_US: u64 = 2000;
/// Distinct BSSIDs the scan list holds.
const MAX_BSS: usize = 64;

// --- Stage breadcrumbs (DISPATCH §4 — every run conclusive pass OR fail) ------
// The bring-up writes these so a Stage-B failure self-localizes against the
// DISPATCH §2 known-good values without a debugger.
static STAGE: AtomicU32 = AtomicU32::new(0);
static ASIC_REV: AtomicU32 = AtomicU32::new(0);
static MAC_REV: AtomicU32 = AtomicU32::new(0);
static EE_VER: AtomicU32 = AtomicU32::new(0xffff);
static READBACK_MAC: SwitchMutex<[u8; 6]> = SwitchMutex::new([0; 6]);
static BSS_COUNT: AtomicUsize = AtomicUsize::new(0);
static BOUND: AtomicBool = AtomicBool::new(false);
static BIND_FAIL: SwitchMutex<Option<&'static str>> = SwitchMutex::new(None);
/// Bind-time channel terminal retained for the post-sink scenario summary.
static CHANNEL_TERMINAL: AtomicU32 = AtomicU32::new(0);
static CHANNEL_TERMINAL_CHANNEL: AtomicU32 = AtomicU32::new(0);
static CHANNEL_OPERATION_CONSUMED: AtomicU64 = AtomicU64::new(0);
static CHANNEL_OPERATION_REMAINING: AtomicU64 = AtomicU64::new(0);
static CHANNEL_OPERATION_EXHAUSTED: AtomicBool = AtomicBool::new(false);
static CHANNEL_TERMINAL_FENCE_REFUSALS: AtomicU64 = AtomicU64::new(0);
/// Calls blocked after terminal channel-path classification. This proves no
/// command traffic was submitted after the evidence-neutral terminal.
static CHANNEL_COMMANDS_BLOCKED_AFTER_TERMINAL: AtomicU64 = AtomicU64::new(0);
/// Bind-time MAC-init terminal and its retained operation evidence. One
/// terminal is emitted before bind unwinds; the counters prove that no later
/// MCU command crossed the production gate.
static MAC_INIT_TERMINAL: AtomicU32 = AtomicU32::new(0);
static MAC_INIT_TERMINAL_COUNT: AtomicU32 = AtomicU32::new(0);
static MAC_INIT_COMMANDS_BLOCKED_AFTER_TERMINAL: AtomicU64 = AtomicU64::new(0);
static MAC_INIT_BUDGET_REMAINING: AtomicU64 = AtomicU64::new(0);
static MAC_INIT_BUDGET_EXHAUSTED: AtomicBool = AtomicBool::new(false);
static MAC_INIT_FENCE_REFUSALS: AtomicU64 = AtomicU64::new(0);
static MAC_INIT_LAST_TABLE: AtomicU32 = AtomicU32::new(0);
static MAC_INIT_LAST_CHUNK: AtomicU32 = AtomicU32::new(0);
static MAC_INIT_LAST_CHUNKS: AtomicU32 = AtomicU32::new(0);
static MAC_INIT_LAST_SEQ: AtomicU32 = AtomicU32::new(0);
static MAC_INIT_LAST_PAYLOAD_LEN: AtomicUsize = AtomicUsize::new(0);
static MAC_INIT_LAST_FRAME_LEN: AtomicUsize = AtomicUsize::new(0);
static MAC_INIT_LAST_WAIT_RESPONSE: AtomicBool = AtomicBool::new(false);
static POST_QSELECT_STAGE: AtomicU32 = AtomicU32::new(0);
static POST_QSELECT_BOUNDARY_FLUSHES: AtomicU32 = AtomicU32::new(0);

// OT-39 channel-wedge discriminators retained for scenario summaries. In the
// MT7601U bare-metal lanes the sink is installed before USB bind, so the live
// terminal is already durable; these atomics are secondary replay evidence.
static WARM_RESYNC_STATE: AtomicU32 = AtomicU32::new(0);
static POST_FW_STATE: AtomicU32 = AtomicU32::new(0);
static POST_FW_WPDMA_LAST: AtomicU32 = AtomicU32::new(0);
static POST_FW_BUDGET_OPERATION: AtomicU32 = AtomicU32::new(0);
static POST_FW_BUDGET_LIMIT: AtomicU64 = AtomicU64::new(0);
static POST_FW_BUDGET_REMAINING: AtomicU64 = AtomicU64::new(0);
static POST_FW_BUDGET_EXHAUSTED: AtomicBool = AtomicBool::new(false);
static POST_FW_WAIT_CALLS: AtomicU64 = AtomicU64::new(0);
static POST_FW_WAIT_ITERATIONS: AtomicU64 = AtomicU64::new(0);
static POST_FW_VENDOR_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static POST_FW_VENDOR_RETRY_GAPS: AtomicU64 = AtomicU64::new(0);
static POST_FW_VENDOR_REQUEST_EXHAUSTIONS: AtomicU64 = AtomicU64::new(0);
static POST_FW_OBSERVATIONS: AtomicU64 = AtomicU64::new(0);
static POST_FW_POLL_GAPS: AtomicU64 = AtomicU64::new(0);
static POST_FW_BULK_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static POST_FW_BULK_POLLS: AtomicU64 = AtomicU64::new(0);
static POST_FW_LATE_EVENTS: AtomicU64 = AtomicU64::new(0);
static POST_FW_NO_EVENT_EXHAUSTIONS: AtomicU64 = AtomicU64::new(0);
static POST_FW_DESYNC_EXHAUSTIONS: AtomicU64 = AtomicU64::new(0);
static TARGET_TUNE_STATE: AtomicU32 = AtomicU32::new(0);
static POST_ASSOC_CAL_STATE: AtomicU32 = AtomicU32::new(0);

mod warm_resync_state {
    pub const COLD: u32 = 0;
    pub const RESIDENT_EARLY_RETURN: u32 = 1;
}

mod mac_init_terminal {
    pub const NONE: u32 = 0;
    pub const OPERATION_FAILED: u32 = 1;
}

mod mac_init_table {
    pub const NONE: u32 = 0;
    pub const COMMON: u32 = 1;
    pub const CHIP: u32 = 2;
}

mod post_fw_state {
    pub const NOT_RUN: u32 = 0;
    pub const WPDMA_WAIT: u32 = 1;
    pub const WPDMA_IDLE: u32 = 2;
    pub const WPDMA_UNRESPONSIVE: u32 = 3;
    pub const WPDMA_BUSY_TIMEOUT: u32 = 4;
    pub const ASIC_WAIT: u32 = 5;
    pub const ASIC_READY: u32 = 6;
    pub const ASIC_FAILED: u32 = 7;
    pub const CSR_BBP_RESET: u32 = 8;
    pub const CSR_BBP_READY: u32 = 9;
    pub const CSR_BBP_FAILED: u32 = 10;
    pub const USB_DMA_INIT: u32 = 11;
    pub const USB_DMA_READY: u32 = 12;
    pub const USB_DMA_FAILED: u32 = 13;
    pub const MCU_Q_SELECT: u32 = 14;
    pub const CONTROL_PLANE_READY: u32 = 15;
    pub const MCU_Q_SELECT_FAILED: u32 = 16;
    pub const WPDMA_OPERATION_BUDGET: u32 = 17;
}

mod post_fw_operation {
    pub const NONE: u32 = 0;
    pub const WPDMA: u32 = 1;
    pub const ASIC: u32 = 2;
    pub const CSR_BBP: u32 = 3;
    pub const USB_DMA: u32 = 4;
    pub const Q_SELECT: u32 = 5;
}

mod target_tune_state {
    pub const BEFORE: u32 = 1;
    pub const TUNED_OK: u32 = 2;
    pub const FAILED: u32 = 3;
}

mod post_assoc_cal_state {
    pub const BEFORE: u32 = 1;
    pub const OK: u32 = 2;
    pub const FAILED: u32 = 3;
}

fn warm_resync_label(state: u32) -> &'static str {
    match state {
        warm_resync_state::RESIDENT_EARLY_RETURN => "resident_early_return",
        _ => "cold",
    }
}

fn mac_init_terminal_label(state: u32) -> &'static str {
    match state {
        mac_init_terminal::OPERATION_FAILED => "post_qselect_operation_failed",
        _ => "none",
    }
}

fn mac_init_table_label(table: u32) -> &'static str {
    match table {
        mac_init_table::COMMON => "mac_common",
        mac_init_table::CHIP => "mac_chip",
        _ => "none",
    }
}

fn post_fw_label(state: u32) -> &'static str {
    match state {
        post_fw_state::WPDMA_WAIT => "wpdma_wait",
        post_fw_state::WPDMA_IDLE => "wpdma_idle",
        post_fw_state::WPDMA_UNRESPONSIVE => "wpdma_unresponsive",
        post_fw_state::WPDMA_BUSY_TIMEOUT => "wpdma_busy_timeout",
        post_fw_state::ASIC_WAIT => "asic_wait",
        post_fw_state::ASIC_READY => "asic_ready",
        post_fw_state::ASIC_FAILED => "asic_failed",
        post_fw_state::CSR_BBP_RESET => "csr_bbp_reset",
        post_fw_state::CSR_BBP_READY => "csr_bbp_ready",
        post_fw_state::CSR_BBP_FAILED => "csr_bbp_failed",
        post_fw_state::USB_DMA_INIT => "usb_dma_init",
        post_fw_state::USB_DMA_READY => "usb_dma_ready",
        post_fw_state::USB_DMA_FAILED => "usb_dma_failed",
        post_fw_state::MCU_Q_SELECT => "mcu_q_select",
        post_fw_state::CONTROL_PLANE_READY => "control_plane_ready",
        post_fw_state::MCU_Q_SELECT_FAILED => "mcu_q_select_failed",
        post_fw_state::WPDMA_OPERATION_BUDGET => "wpdma_operation_budget",
        _ => "not_run",
    }
}

fn post_fw_operation_label(operation: u32) -> &'static str {
    match operation {
        post_fw_operation::WPDMA => "wpdma",
        post_fw_operation::ASIC => "asic",
        post_fw_operation::CSR_BBP => "csr_bbp",
        post_fw_operation::USB_DMA => "usb_dma",
        post_fw_operation::Q_SELECT => "q_select",
        _ => "none",
    }
}

fn target_tune_label(state: u32) -> &'static str {
    match state {
        target_tune_state::BEFORE => "before",
        target_tune_state::TUNED_OK => "tuned_ok",
        target_tune_state::FAILED => "failed",
        _ => "not_run",
    }
}

fn post_assoc_cal_label(state: u32) -> &'static str {
    match state {
        post_assoc_cal_state::BEFORE => "before",
        post_assoc_cal_state::OK => "ok",
        post_assoc_cal_state::FAILED => "failed",
        _ => "not_run",
    }
}

/// Bring-up stages, in order — the breadcrumb's `stage=` value.
mod stage {
    pub const ENUMERATED: u32 = 1;
    pub const ASIC_READY: u32 = 2;
    pub const FIRMWARE_LOADED: u32 = 3;
    pub const MCU_READY: u32 = 4;
    pub const EFUSE_READ: u32 = 5;
    pub const MAC_BBP_INIT: u32 = 6;
    pub const PHY_INIT: u32 = 7;
    pub const MAC_STARTED: u32 = 8;
    pub const SCANNED: u32 = 9;
}

// ── W2 ISC-1: association breadcrumbs + target config (DISPATCH §4 discriminator)
/// Association progress — the `assoc_stage=` breadcrumb (DISPATCH §4 mode 1).
mod assoc_stage {
    pub const IDLE: u32 = 0;
    pub const AUTH_SENT: u32 = 1;
    pub const AUTHED: u32 = 2;
    pub const ASSOC_SENT: u32 = 3;
    pub const ASSOCIATED: u32 = 4;
}
static ASSOC_STAGE: AtomicU32 = AtomicU32::new(0);
/// The last mgmt status code seen (auth/assoc response) — the §4 discriminator
/// between "no response" (stays 0xffff) and "rejected" (a non-zero status).
static LAST_STATUS: AtomicU32 = AtomicU32::new(0xffff);
/// The AID granted by the AP (assoc response), 0 until associated.
static ASSOC_AID: AtomicU32 = AtomicU32::new(0);

/// 4-way handshake progress — the `handshake=` breadcrumb (DISPATCH §4 mode 2),
/// so a stalled handshake self-localizes (which message never arrived).
mod handshake_stage {
    pub const NONE: u32 = 0;
    pub const M1_RX: u32 = 1;
    pub const M2_TX: u32 = 2;
    pub const M3_RX: u32 = 3;
    pub const M4_TX: u32 = 4;
}
static HANDSHAKE_STAGE: AtomicU32 = AtomicU32::new(0);
/// SNonce counter for the INSECURE bring-up fallback (see `gen_snonce`). Only
/// the `#[cfg(not(kernel-rng))]` fallback uses it; under the shipped graph
/// (`wifi` pulls `kernel-rng`) the SNonce is a CSPRNG draw and this is absent.
#[cfg(not(feature = "kernel-rng"))]
static SNONCE_CTR: AtomicU32 = AtomicU32::new(0);

/// The legacy counter-derived SNonce (W1/W2 bring-up). NOT cryptographically
/// random. Retained as (a) the `#[cfg(not(kernel-rng))]` insecure fallback and
/// (b) the *non-counter oracle* for the OT-2 KAT: a CSPRNG draw MUST differ from
/// this for the same counter, or the swap didn't happen. `#[allow(dead_code)]`
/// because under the shipped `kernel-rng` graph only the neo-test KAT calls it.
#[allow(dead_code)]
fn counter_snonce(ctr: u32, mac: [u8; 6]) -> [u8; 32] {
    let mut n = [0u8; 32];
    for i in 0..32 {
        n[i] = mac[i % 6] ^ (ctr.rotate_left((i as u32) & 31) as u8) ^ (i as u8);
    }
    n
}
/// Set when `associate` reaches `Connected` — gates the NetDevice link state +
/// its registration (ISC-4). The associated BSSID is the data-frame addr1.
static CONNECTED: AtomicBool = AtomicBool::new(false);
static ASSOC_BSSID: SwitchMutex<[u8; 6]> = SwitchMutex::new([0; 6]);
static DHCP_TERMINAL: AtomicU32 = AtomicU32::new(link_result_logic::DhcpTerminal::NotRun as u32);
static DHCP_ASSIGNED_IP: AtomicU32 = AtomicU32::new(0);
static DHCP_XID: AtomicU32 = AtomicU32::new(0);
static LINK_UP: AtomicBool = AtomicBool::new(false);

/// CB-1 ISC-6 kill-switch: gates the group-key (GTK) install + broadcast RX — the
/// CB-1 link-completion feature. `false` ⇒ the 4-way installs only the pairwise
/// TK and `wlan0` carries unicast CCMP exactly as the W2 association-only baseline
/// (no WCID-0 write, no broadcast RX, no DHCP-over-broadcast). Default ON.
static GROUP_RX_ENABLED: AtomicBool = AtomicBool::new(true);
/// Set when the GTK is installed to `GROUP_WCID` at the 4-way M3 step. The
/// DHCP-over-`wlan0` path gates on this (T-043: a broadcast DHCP Discover/Offer
/// can't pass before the GTK lands), and it is the broadcast-RX discriminator.
static GTK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Discriminator: every RX frame `drain_mgmt` delivers (beacons included) bumps
/// this. `associate()` zeroes it before the auth wait and reads it after, so a
/// stalled auth self-localizes — 0 ⇒ RX dead / AP silent; >0 with no Auth ⇒ the
/// directed response is arriving-but-dropped, or the AP simply isn't answering.
static RX_MGMT_FRAMES: AtomicU32 = AtomicU32::new(0);

/// Discriminator (part 2): frames whose 802.11 addr1 (RA) == our own MAC — i.e.
/// directed AT us, not broadcast beacons. Splits the auth-stall tree: 0 ⇒ the AP
/// never hears our request (TX-side); >0 ⇒ the AP IS answering and the response
/// is arriving-but-dropped/mis-parsed (RX-side).
static RX_DIRECTED_COUNT: AtomicU32 = AtomicU32::new(0);
/// The frame-control word of the FIRST directed-to-us frame seen, OR'd with
/// 0x1_0000 as a "seen" flag (0 ⇒ none). 0xb0 = auth resp, 0xc0 = deauth,
/// 0x10/0x30 = assoc resp, low bits 0x08 = data.
static RX_DIRECTED_FC: AtomicU32 = AtomicU32::new(0);
/// One-shot guard: hexdump the first wrapped TX buffer (TXINFO+TXWI+frame) so the
/// exact on-air bytes can be byte-verified against the 802.11 auth/TXWI spec — the
/// last unknown in the power-independent "AP never hears us" failure.
static TX_DUMPED: AtomicBool = AtomicBool::new(false);

/// Spin budgets for awaiting a mgmt response (×5µs ≈ the FINDINGS §1.3 timeouts;
/// generous — a healthy AP replies in a handful, a silent one fails self-localizing).
const AUTH_RESP_SPINS: u32 = 80_000;
const ASSOC_RESP_SPINS: u32 = 80_000;
/// Budget for awaiting an EAPOL-Key (M1/M3) — the AP can take longer to start
/// the 4-way than a mgmt-response round-trip.
const EAPOL_RESP_SPINS: u32 = 200_000;
/// WCID for the AP unicast context — both pre-assoc mgmt (auth/assoc, WIV/no-key)
/// and post-assoc data (CCMP key). `1` = the BSSID slot (mt7601u.h WCID map);
/// `wcid_setup` programs its address to the AP before the first auth TX. The W2
/// bug was sending mgmt on WCID `0` (the broadcast mcast slot) — the chip then had
/// no unicast context and the AP never received/ACKed our auth.
const STA_WCID: u8 = 1;
/// The group/multicast slot for the GTK (0 = mcast wcid, mt7601u.h map).
const GROUP_WCID: u8 = 0;

/// A configured association target. `None` ⇒ scan-only (the W1 scenario path).
/// An association scenario calls `set_target` before bind; the supplicant matches
/// `ssid` against the scan list to learn the BSSID + channel.
#[derive(Clone)]
pub struct AssocTarget {
    pub ssid: [u8; 32],
    pub ssid_len: u8,
    pub bssid: [u8; 6],
    pub channel: u8,
    /// Open network (no RSN) — skip the 4-way. WPA2-PSK runs the handshake.
    pub open: bool,
    /// The PMK (WPA2-PSK: `kdf::pmk_from_passphrase(passphrase, ssid)`), for the
    /// 4-way handshake. Ignored for `open`. The scenario computes it.
    pub pmk: [u8; 32],
}
static TARGET: SwitchMutex<Option<AssocTarget>> = SwitchMutex::new(None);

#[allow(dead_code)] // association-scenario seam (feature-gated consumer)
pub fn set_target(target: AssocTarget) {
    *TARGET.lock() = Some(target);
}

/// The assoc-scenario's baked-in target — the Stage-B config-swap point.
/// SWAP only SSID/passphrase for the test AP. CHANNEL remains zero so the
/// shipping path discovers the BSSID and operating channel by SSID.
#[cfg(feature = "neo-test-mt7601u-assoc-baremetal")]
fn configured_target() -> Option<AssocTarget> {
    // LOCAL, UNCOMMITTED Stage-B config (the WiFi password must NOT enter git —
    // revert this hunk with `git checkout` after the run; origin keeps the empty
    // placeholders). Slice B (WPA2-PSK): the full 4-way handshake.
    const SSID: &[u8] = b"";
    const OPEN: bool = false;
    const PASSPHRASE: &[u8] = b"";
    // Shipping contract: no baked channel. Zero performs the full survey and
    // selects a matching BSS by SSID. Non-zero remains available only through a
    // runtime AssocTarget for the optional targeted fast path.
    const CHANNEL: u8 = 0;
    if SSID.is_empty() {
        return None; // not yet configured — fall back to scan-only
    }
    let n = SSID.len().min(32);
    let mut ssid = [0u8; 32];
    ssid[..n].copy_from_slice(&SSID[..n]);
    let pmk = if OPEN {
        [0u8; 32]
    } else {
        crate::net::wpa::kdf::pmk_from_passphrase(PASSPHRASE, SSID)
    };
    Some(AssocTarget {
        ssid,
        ssid_len: n as u8,
        bssid: [0; 6],
        channel: CHANNEL,
        open: OPEN,
        pmk,
    })
}
#[cfg(not(feature = "neo-test-mt7601u-assoc-baremetal"))]
fn configured_target() -> Option<AssocTarget> {
    None
}

/// Whether the supplicant reached `Connected` (associated; WPA2 keys installed).
#[allow(dead_code)]
pub fn is_connected() -> bool {
    CONNECTED.load(Ordering::Acquire)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssociationOutcome {
    pub bound: bool,
    pub connected: bool,
    pub gtk_installed: bool,
    pub dhcp: link_result_logic::DhcpTerminal,
    pub assigned_ip: [u8; 4],
    pub xid: u32,
    pub link_up: bool,
}

impl AssociationOutcome {
    pub const fn pass_eligible(self) -> bool {
        link_result_logic::pass_eligible(link_result_logic::LinkEvidence {
            bound: self.bound,
            connected: self.connected,
            gtk_installed: self.gtk_installed,
            dhcp: self.dhcp,
            assigned_ip: self.assigned_ip,
            link_up: self.link_up,
        })
    }
}

fn association_outcome() -> AssociationOutcome {
    AssociationOutcome {
        bound: BOUND.load(Ordering::Acquire),
        connected: CONNECTED.load(Ordering::Acquire),
        gtk_installed: GTK_INSTALLED.load(Ordering::Acquire),
        dhcp: link_result_logic::DhcpTerminal::from_raw(DHCP_TERMINAL.load(Ordering::Acquire)),
        assigned_ip: link_result_logic::unpack_ip(DHCP_ASSIGNED_IP.load(Ordering::Acquire)),
        xid: DHCP_XID.load(Ordering::Acquire),
        link_up: LINK_UP.load(Ordering::Acquire),
    }
}

#[allow(dead_code)]
pub fn debug_association_outcome() -> AssociationOutcome {
    association_outcome()
}

/// Association breadcrumbs for the assoc scenario (DISPATCH §4): `(assoc_stage,
/// handshake_stage, last_status, aid)`.
#[allow(dead_code)]
pub fn debug_assoc_breadcrumbs() -> (u32, u32, u32, u32) {
    (
        ASSOC_STAGE.load(Ordering::Relaxed),
        HANDSHAKE_STAGE.load(Ordering::Relaxed),
        LAST_STATUS.load(Ordering::Relaxed),
        ASSOC_AID.load(Ordering::Relaxed),
    )
}

/// A management frame surfaced from the RX path to the supplicant.
enum RxMgmt {
    Auth(mlme_logic::AuthFrame),
    AssocResp(mlme_logic::AssocResp),
    Deauth,
}

/// Walk a received DMA buffer's segments and return the first auth/assoc-resp/
/// deauth management frame (W2 ISC-1 RX). Mirrors `parse_rx`'s segment walk but
/// parses mgmt frames instead of harvesting BSSIDs.
fn parse_rx_mgmt(buffer: &Arc<DmaBuffer>, len: usize, our_mac: &[u8; 6]) -> Option<RxMgmt> {
    let len = len.min(RX_BUF_LEN);
    // SAFETY: buffer is exclusively owned; reading device-written bytes.
    let bytes = unsafe { core::slice::from_raw_parts(buffer.as_ptr(), len) };
    let mut off = 0;
    while off < bytes.len() {
        let seg_len = rx_logic::next_seg_len(&bytes[off..])?;
        if off + seg_len > bytes.len() {
            break;
        }
        let seg = &bytes[off..off + seg_len];
        if rx_logic::rxwi_zero_ok(seg)
            && let Some(frame) = rx_logic::segment_frame(seg)
        {
            // Discriminator: is this frame directed AT us (addr1 == our MAC)?
            // If so it is the AP answering, not a broadcast beacon — record
            // the count and the first such frame's FC so a stalled auth says
            // whether the AP is responding (and with what) vs never hearing us.
            if frame.len() >= 10 && &frame[4..10] == our_mac {
                RX_DIRECTED_COUNT.fetch_add(1, Ordering::Relaxed);
                let fc = u16::from_le_bytes([frame[0], frame[1]]);
                let _ = RX_DIRECTED_FC.compare_exchange(
                    0,
                    (fc as u32) | 0x1_0000,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
            }
            if let Some(a) = mlme_logic::parse_auth(frame) {
                return Some(RxMgmt::Auth(a));
            }
            if let Some(r) = mlme_logic::parse_assoc_resp(frame) {
                return Some(RxMgmt::AssocResp(r));
            }
            if frame.len() >= mlme_logic::MGMT_HDR_LEN {
                let fc = u16::from_le_bytes([frame[0], frame[1]]);
                if (fc & mlme_logic::FCTL_FTYPE) == mlme_logic::FTYPE_MGMT {
                    let stype = fc & mlme_logic::FCTL_STYPE;
                    if stype == mlme_logic::STYPE_DEAUTH || stype == mlme_logic::STYPE_DISASSOC {
                        return Some(RxMgmt::Deauth);
                    }
                }
            }
        }
        off += seg_len;
    }
    None
}

/// A copied EAPOL payload (the 4-way M1/M3 frames are well under 256 bytes).
/// Derefs to the payload slice so callers index it like `&[u8]`.
struct EapolBuf {
    bytes: [u8; 256],
    len: usize,
}
impl core::ops::Deref for EapolBuf {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Walk a received DMA buffer and return the first EAPOL payload (the bytes
/// after the 802.11 data header + LLC/SNAP — i.e. the EAPOL-Key frame).
fn parse_rx_eapol(buffer: &Arc<DmaBuffer>, len: usize, our_mac: &[u8; 6]) -> Option<EapolBuf> {
    let len = len.min(RX_BUF_LEN);
    // SAFETY: buffer is exclusively owned; reading device-written bytes.
    let bytes = unsafe { core::slice::from_raw_parts(buffer.as_ptr(), len) };
    let mut off = 0;
    while off < bytes.len() {
        let seg_len = rx_logic::next_seg_len(&bytes[off..])?;
        if off + seg_len > bytes.len() {
            break;
        }
        let seg = &bytes[off..off + seg_len];
        if rx_logic::rxwi_zero_ok(seg)
            && let Some(frame) = rx_logic::segment_frame(seg)
        {
            // Discriminator (M3 wait): record any frame directed AT us so a
            // 4-way M3 timeout self-localizes — captures the FC even if the
            // EAPOL extraction below fails (e.g. M3 in a QoS-data frame our
            // parse_eapol_data doesn't decode). directed=0 ⇒ AP dropped our
            // M2 (MIC/PTK); directed>0 ⇒ M3 arrived but wasn't extracted.
            if frame.len() >= 10 && &frame[4..10] == our_mac {
                RX_DIRECTED_COUNT.fetch_add(1, Ordering::Relaxed);
                let fc = u16::from_le_bytes([frame[0], frame[1]]);
                let _ = RX_DIRECTED_FC.compare_exchange(
                    0,
                    (fc as u32) | 0x1_0000,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
            }
            if let Some(payload) = mlme_logic::parse_eapol_data(frame)
                && !payload.is_empty()
                && payload.len() <= 256
            {
                let mut b = EapolBuf {
                    bytes: [0u8; 256],
                    len: payload.len(),
                };
                b.bytes[..payload.len()].copy_from_slice(payload);
                return Some(b);
            }
        }
        off += seg_len;
    }
    None
}

/// Walk a received DMA buffer for one DATA frame and write the reconstructed
/// ethernet frame into `out`: `[da = our MAC][sa = 802.11 addr3][EtherType]
/// [payload]`. Returns the ethernet length, or `None` if no SNAP data frame is
/// present / `out` is too small. (ISC-4 RX: 802.11 → 802.3 for the net stack.)
fn build_eth_from_rx(
    buffer: &Arc<DmaBuffer>,
    len: usize,
    mac: &[u8; 6],
    out: &mut [u8],
) -> Option<usize> {
    let len = len.min(RX_BUF_LEN);
    // SAFETY: buffer is exclusively owned; reading device-written bytes.
    let bytes = unsafe { core::slice::from_raw_parts(buffer.as_ptr(), len) };
    let mut off = 0;
    while off < bytes.len() {
        let seg_len = rx_logic::next_seg_len(&bytes[off..])?;
        if off + seg_len > bytes.len() {
            break;
        }
        let seg = &bytes[off..off + seg_len];
        if rx_logic::rxwi_zero_ok(seg)
            && let Some(frame) = rx_logic::segment_frame(seg)
            && frame.len() >= mlme_logic::MGMT_HDR_LEN
            && let Some((ethertype, payload)) = mlme_logic::parse_data_frame(frame)
        {
            let total = 14 + payload.len();
            if out.len() >= total {
                out[0..6].copy_from_slice(mac); // DA = us
                out[6..12].copy_from_slice(&frame[16..22]); // SA = addr3
                out[12..14].copy_from_slice(&ethertype.to_be_bytes());
                out[14..total].copy_from_slice(payload);
                return Some(total);
            }
        }
        off += seg_len;
    }
    None
}

/// The kill-switch: disabled ⇒ probe declines ⇒ BUILD-A's unclaimed path.
static ENABLED: AtomicBool = AtomicBool::new(true);

#[allow(dead_code)] // kill-switch gate seam — feature-gated scenario consumer
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Release);
}
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// CB-1 ISC-6 kill-switch: toggle the group-key (GTK) install + broadcast-RX path.
/// Off ⇒ the 4-way installs only the pairwise TK (the W2 association-only baseline).
#[allow(dead_code)] // feature-gated scenario consumer (kill-switch regression)
pub fn set_group_rx_enabled(enabled: bool) {
    GROUP_RX_ENABLED.store(enabled, Ordering::Release);
}
#[allow(dead_code)]
pub fn group_rx_enabled() -> bool {
    GROUP_RX_ENABLED.load(Ordering::Acquire)
}

/// Legacy CB-2 kill-switch retained for ABI and host-oracle compatibility.
/// Channel-switch failure is now terminal and never invokes endpoint reset;
/// the terminal breadcrumb records this setting as a negative discriminator.
static ENDPOINT_RESET_ENABLED: AtomicBool = AtomicBool::new(true);

#[allow(dead_code)] // feature-gated scenario consumer (kill-switch regression)
pub fn set_endpoint_reset_enabled(enabled: bool) {
    ENDPOINT_RESET_ENABLED.store(enabled, Ordering::Release);
}
pub fn endpoint_reset_enabled() -> bool {
    ENDPOINT_RESET_ENABLED.load(Ordering::Acquire)
}
/// Whether the GTK was installed to `GROUP_WCID` this association (T-043: the
/// DHCP-over-`wlan0` broadcast path gates on this; the broadcast-RX discriminator).
#[allow(dead_code)]
pub fn gtk_installed() -> bool {
    GTK_INSTALLED.load(Ordering::Acquire)
}

/// Whether the dongle bound this boot (Stage-B conclusiveness).
#[allow(dead_code)]
pub fn is_bound() -> bool {
    BOUND.load(Ordering::Acquire)
}

/// The bring-up breadcrumbs the scenario consumes (Rule 5: never dead).
#[allow(dead_code)]
#[cfg(feature = "neo-test")]
pub fn debug_breadcrumbs() -> (u32, u32, u32, u16, [u8; 6], usize, Option<&'static str>) {
    (
        STAGE.load(Ordering::Relaxed),
        ASIC_REV.load(Ordering::Relaxed),
        MAC_REV.load(Ordering::Relaxed),
        EE_VER.load(Ordering::Relaxed) as u16,
        *READBACK_MAC.lock(),
        BSS_COUNT.load(Ordering::Relaxed),
        *BIND_FAIL.lock(),
    )
}

/// Post-firmware state retained across the pre-sink USB bind. Both bare-metal
/// scenarios replay this tuple after sink installation, including bind failure.
#[allow(dead_code)]
#[cfg(feature = "neo-test")]
pub fn debug_post_firmware_breadcrumbs() -> (u32, &'static str, u32) {
    let state = POST_FW_STATE.load(Ordering::Relaxed);
    (
        state,
        post_fw_label(state),
        POST_FW_WPDMA_LAST.load(Ordering::Relaxed),
    )
}

#[allow(dead_code)]
#[cfg(feature = "neo-test")]
pub fn debug_channel_terminal_breadcrumbs() -> (u32, &'static str, u8, u64, u64, u64, bool, u64) {
    let terminal = CHANNEL_TERMINAL.load(Ordering::Acquire);
    (
        terminal,
        if terminal == 1 {
            "channel_command_path_unusable"
        } else {
            "none"
        },
        CHANNEL_TERMINAL_CHANNEL.load(Ordering::Relaxed) as u8,
        CHANNEL_COMMANDS_BLOCKED_AFTER_TERMINAL.load(Ordering::Relaxed),
        CHANNEL_OPERATION_CONSUMED.load(Ordering::Relaxed),
        CHANNEL_OPERATION_REMAINING.load(Ordering::Relaxed),
        CHANNEL_OPERATION_EXHAUSTED.load(Ordering::Relaxed),
        CHANNEL_TERMINAL_FENCE_REFUSALS.load(Ordering::Relaxed),
    )
}

#[allow(dead_code)]
#[cfg(feature = "neo-test")]
pub fn debug_mac_init_terminal_breadcrumbs() -> (
    u32,
    &'static str,
    u32,
    u64,
    u64,
    bool,
    u64,
    &'static str,
    u32,
    u32,
) {
    let terminal = MAC_INIT_TERMINAL.load(Ordering::Acquire);
    (
        terminal,
        mac_init_terminal_label(terminal),
        MAC_INIT_TERMINAL_COUNT.load(Ordering::Relaxed),
        MAC_INIT_COMMANDS_BLOCKED_AFTER_TERMINAL.load(Ordering::Relaxed),
        MAC_INIT_BUDGET_REMAINING.load(Ordering::Relaxed),
        MAC_INIT_BUDGET_EXHAUSTED.load(Ordering::Relaxed),
        MAC_INIT_FENCE_REFUSALS.load(Ordering::Relaxed),
        mac_init_table_label(MAC_INIT_LAST_TABLE.load(Ordering::Relaxed)),
        MAC_INIT_LAST_CHUNK.load(Ordering::Relaxed),
        MAC_INIT_LAST_CHUNKS.load(Ordering::Relaxed),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostFwBudgetBreadcrumb {
    pub operation: u32,
    pub operation_label: &'static str,
    pub limit: u64,
    pub remaining: u64,
    pub exhausted: bool,
    pub counters: super::operation_budget_logic::BudgetCounters,
}

fn retain_post_fw_budget(
    operation: u32,
    snapshot: xhci::OperationBudgetSnapshot,
) -> PostFwBudgetBreadcrumb {
    let counters = snapshot.counters;
    POST_FW_BUDGET_OPERATION.store(operation, Ordering::Relaxed);
    POST_FW_BUDGET_LIMIT.store(snapshot.limit, Ordering::Relaxed);
    POST_FW_BUDGET_REMAINING.store(snapshot.remaining, Ordering::Relaxed);
    POST_FW_BUDGET_EXHAUSTED.store(snapshot.exhausted, Ordering::Relaxed);
    POST_FW_WAIT_CALLS.store(counters.wait_calls, Ordering::Relaxed);
    POST_FW_WAIT_ITERATIONS.store(counters.wait_iterations, Ordering::Relaxed);
    POST_FW_VENDOR_ATTEMPTS.store(counters.vendor_attempts, Ordering::Relaxed);
    POST_FW_VENDOR_RETRY_GAPS.store(counters.vendor_retry_gaps, Ordering::Relaxed);
    POST_FW_VENDOR_REQUEST_EXHAUSTIONS
        .store(counters.vendor_request_exhaustions, Ordering::Relaxed);
    POST_FW_OBSERVATIONS.store(counters.observations, Ordering::Relaxed);
    POST_FW_POLL_GAPS.store(counters.poll_gaps, Ordering::Relaxed);
    POST_FW_BULK_ATTEMPTS.store(counters.bulk_attempts, Ordering::Relaxed);
    POST_FW_BULK_POLLS.store(counters.bulk_polls, Ordering::Relaxed);
    POST_FW_LATE_EVENTS.store(counters.late_events, Ordering::Relaxed);
    POST_FW_NO_EVENT_EXHAUSTIONS.store(counters.no_event_exhaustions, Ordering::Relaxed);
    POST_FW_DESYNC_EXHAUSTIONS.store(counters.desync_exhaustions, Ordering::Relaxed);
    PostFwBudgetBreadcrumb {
        operation,
        operation_label: post_fw_operation_label(operation),
        limit: snapshot.limit,
        remaining: snapshot.remaining,
        exhausted: snapshot.exhausted,
        counters,
    }
}

fn retained_post_fw_budget() -> PostFwBudgetBreadcrumb {
    let operation = POST_FW_BUDGET_OPERATION.load(Ordering::Relaxed);
    PostFwBudgetBreadcrumb {
        operation,
        operation_label: post_fw_operation_label(operation),
        limit: POST_FW_BUDGET_LIMIT.load(Ordering::Relaxed),
        remaining: POST_FW_BUDGET_REMAINING.load(Ordering::Relaxed),
        exhausted: POST_FW_BUDGET_EXHAUSTED.load(Ordering::Relaxed),
        counters: super::operation_budget_logic::BudgetCounters {
            wait_calls: POST_FW_WAIT_CALLS.load(Ordering::Relaxed),
            wait_iterations: POST_FW_WAIT_ITERATIONS.load(Ordering::Relaxed),
            vendor_attempts: POST_FW_VENDOR_ATTEMPTS.load(Ordering::Relaxed),
            vendor_retry_gaps: POST_FW_VENDOR_RETRY_GAPS.load(Ordering::Relaxed),
            vendor_request_exhaustions: POST_FW_VENDOR_REQUEST_EXHAUSTIONS.load(Ordering::Relaxed),
            observations: POST_FW_OBSERVATIONS.load(Ordering::Relaxed),
            poll_gaps: POST_FW_POLL_GAPS.load(Ordering::Relaxed),
            bulk_attempts: POST_FW_BULK_ATTEMPTS.load(Ordering::Relaxed),
            bulk_polls: POST_FW_BULK_POLLS.load(Ordering::Relaxed),
            late_events: POST_FW_LATE_EVENTS.load(Ordering::Relaxed),
            no_event_exhaustions: POST_FW_NO_EVENT_EXHAUSTIONS.load(Ordering::Relaxed),
            desync_exhaustions: POST_FW_DESYNC_EXHAUSTIONS.load(Ordering::Relaxed),
        },
    }
}

fn print_post_fw_budget(b: PostFwBudgetBreadcrumb) {
    crate::serial_println!(
        "mt7601u: POST-FW budget op={} limit={} remaining={} exhausted={} wait_calls={} wait_iters={} vendor_attempts={} retry_gaps={} vendor_exhaustions={} observations={} poll_gaps={} bulk_attempts={} bulk_polls={} late_events={} no_event={} desync={}",
        b.operation_label,
        b.limit,
        b.remaining,
        b.exhausted,
        b.counters.wait_calls,
        b.counters.wait_iterations,
        b.counters.vendor_attempts,
        b.counters.vendor_retry_gaps,
        b.counters.vendor_request_exhaustions,
        b.counters.observations,
        b.counters.poll_gaps,
        b.counters.bulk_attempts,
        b.counters.bulk_polls,
        b.counters.late_events,
        b.counters.no_event_exhaustions,
        b.counters.desync_exhaustions,
    );
}

#[allow(dead_code)]
#[cfg(feature = "neo-test")]
pub fn debug_post_firmware_budget() -> PostFwBudgetBreadcrumb {
    retained_post_fw_budget()
}

// --- The eFUSE parameters the PHY bring-up consumes ---------------------------

/// The subset of `mt7601u_eeprom_params` the W1 PHY path reads (eeprom.c).
#[derive(Clone, Copy, Default)]
pub struct EepromParams {
    pub rf_freq_off: u8,
    pub rssi_offset: [i8; 2],
    pub lna_gain: i8,
    pub ref_temp: i8,
    pub tssi_enabled: bool,
    /// Country-region channel bounds (start, count); default ch 1-14.
    pub reg_start: u8,
    pub reg_num: u8,
    pub chan_pwr: [u8; 14],
    /// eeprom.c `config_tx_power_per_rate`: the 5 per-rate power words written
    /// verbatim to TX_PWR_CFG_0..4. `0xffff_ffff` = invalid (skip that write).
    pub tx_pwr_cfg: [u32; 5],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct McuResponseStats {
    arms: u64,
    completions: u64,
    mismatches: u64,
    unowned_events: u64,
    timeout_windows: u64,
}

#[derive(Clone, Copy, Debug)]
struct MacInitChunkBreadcrumb {
    table: u32,
    index: usize,
    count: usize,
}

#[derive(Clone, Copy, Debug)]
struct PostQselectFailure {
    stage: linux_parity_logic::PostQselectStage,
    reason: &'static str,
}

impl PostQselectFailure {
    const fn new(stage: linux_parity_logic::PostQselectStage, reason: &'static str) -> Self {
        Self { stage, reason }
    }
}

#[derive(Clone, Copy, Debug)]
struct ChannelOperationAccount {
    budget: xhci::OperationBudgetSnapshot,
    bulk_budget: u32,
    fence_refusals_before: u64,
    fence_refusals_after: u64,
    fce_upload_kicks_before: u64,
    fce_upload_kicks_after: u64,
    response_before: McuResponseStats,
    response_after: McuResponseStats,
    status_before: ChannelStatusSnapshot,
    status_after: ChannelStatusSnapshot,
    endpoint_probe_attempted: bool,
    endpoint_probe_reported_ok: bool,
    recovery: Option<ChannelRecoveryAccount>,
}

/// What the Q-B non-destructive recovery sequence did and observed. Carried
/// into the terminal record so a failed recovery is self-diagnosing from the
/// serial log alone.
#[derive(Clone, Copy, Debug)]
struct ChannelRecoveryAccount {
    drain: ChannelDrainOutcome,
    command_path_proven_usable: bool,
    retry_action: wedge_logic::ChannelRetryAction,
    retry_attempted: bool,
    retry_succeeded: bool,
}

/// Drain accounting. `first_cmd_done_at` is 0 when no no-op ever completed —
/// the "command path is genuinely dead, not merely backed up" discriminator.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ChannelDrainOutcome {
    planned: u8,
    issued: u8,
    cmd_done: u8,
    first_cmd_done_at: u8,
}

#[derive(Clone, Copy, Debug)]
struct ChannelStatusSnapshot {
    mcu_com0: u32,
    mcu_com1: u32,
    wpdma_glo_cfg: u32,
    usb_dma_cfg: u32,
    pbf_cfg: u32,
    fce_pse_ctrl: u32,
    fce_cpu_desc_idx: u32,
    fce_pdma_global_conf: u32,
    mac_status: u32,
    rf_csr_cfg: u32,
    rf_bypass0: u32,
    rf_setting0: u32,
    rf17_20: [u8; 4],
}

impl ChannelStatusSnapshot {
    fn rf_matched_prefix(self, expected: [u8; 4]) -> usize {
        let mut matched = 0usize;
        while matched < self.rf17_20.len() && self.rf17_20[matched] == expected[matched] {
            matched += 1;
        }
        matched
    }

    fn rf_first_mismatch_reg(self, expected: [u8; 4]) -> u8 {
        let matched = self.rf_matched_prefix(expected);
        if matched == self.rf17_20.len() {
            0
        } else {
            17 + matched as u8
        }
    }
}

fn expected_rf_freqplan(channel: u8) -> [u8; 4] {
    if (1..=14).contains(&channel) {
        initvals_phy::FREQ_PLAN[(channel - 1) as usize]
    } else {
        [0xff; 4]
    }
}

// --- The claimed device --------------------------------------------------------

/// One claimed MT7601U: the live EP0/slot, the configured bulk pipes, the DMA
/// staging buffers, and the parsed eFUSE. Owned by exactly one driver binding;
/// guarded by a single `USB_DEVICE`-ordered lock when registered.
pub struct Mt7601uDev {
    claimed: ClaimedDevice,
    /// In-band command pipe (firmware upload + MCU commands) — `out_eps[0]`.
    inband: EndpointPipe,
    /// Data/mgmt TX pipe (auth/assoc frames, then data) — `out_eps[2]` (AC_BE).
    /// W2 ISC-1; W1 (scan-only) never transmitted 802.11 frames.
    data_tx: EndpointPipe,
    /// Packet-RX pipe (beacons) — `in_eps[0]`.
    pkt_rx: EndpointPipe,
    /// MCU command-response pipe — `in_eps[1]`.
    cmd_resp: EndpointPipe,
    /// TX staging (firmware chunks + MCU frames). Single outstanding transfer.
    staging: Arc<DmaBuffer>,
    /// 802.11 TX staging (the wrapped TXINFO+TXWI+frame). Single outstanding.
    tx_staging: Arc<DmaBuffer>,
    /// MCU response staging.
    resp: Arc<DmaBuffer>,
    /// Exact transfer-ring slot owning the persistent response buffer. Mainline
    /// arms its response URB before sending response-bearing commands and keeps
    /// one URB outstanding across commands; this is the xHCI equivalent.
    cmd_resp_pending: Option<usize>,
    cmd_resp_stats: McuResponseStats,
    /// Number of cold-firmware FCE descriptor kicks. Channel programming does
    /// not touch the firmware-upload descriptor index; per-hop evidence records
    /// this counter unchanged.
    fce_upload_kicks: u64,
    /// RX buffer pool, indexed by the pipe ring index.
    rx_table: [Option<Arc<DmaBuffer>>; xhci::RING_TABLE],
    /// MCU sequence counter (1..=15, never 0).
    msg_seq: u8,
    pub ee: EepromParams,
    /// Current PHY bandwidth (`regs::BW_20`/`BW_40`); `0xff` = unset. Tracked
    /// so `bbp_set_bw` skips the MAC stop/start + table reload when the
    /// bandwidth is unchanged (the faithful driver behavior) — a scan stays at
    /// BW20, so only the first `set_channel` pays that cost (phy.c:1184).
    pub bw: u8,
    /// ISC-0: scoped bulk-poll spin budget for `inband_out_sync`/`mcu_wait_resp`
    /// (default `BULK_POLL_SPINS`). The scan loop lowers it to `SCAN_BULK_BUDGET`
    /// around each `set_channel` so a wedged channel fast-fails; restored after.
    bulk_budget: u32,
    /// Latched by the first failed channel-programming operation. The root
    /// cause is intentionally undistinguished; [`Self::mcu_send`] refuses all
    /// further command traffic.
    channel_command_path_unusable: bool,
    /// Latched by the first operation failure after Q_SELECT readiness through
    /// EEPROM acquisition. Bind unwinds immediately; every direct register and
    /// MCU submission gate refuses and counts accidental later device traffic.
    mac_init_command_path_unusable: bool,
    /// The channel currently tuned (set by the scan loop / `associate`), so an
    /// RX'd BSS is tagged with where it was heard. 0 = untuned.
    cur_channel: u8,
    /// The scan list (deduplicated BSSIDs).
    bss: Vec<rx_logic::Bss>,
    /// Successful targeted operating-channel tunes this boot. Zero selects the
    /// one-shot CAL_BW deferral; failed tunes do not advance it.
    successful_targeted_tunes: u8,
    /// The current survey/targeted tune completed without CAL_BW; clear only
    /// after the bounded post-association calibration succeeds.
    deferred_cal_pending: bool,
    /// Channel hops completed since the MCU command path was last proven fresh.
    /// Sets the Q-B drain depth: one idempotent no-op per hop whose response
    /// slots may still be unretired. Cleared by a successful command-path proof.
    channel_hops_completed: u8,
}

/// A coarse busy-wait — no sleep primitive on this path (the cdc-ecm idiom).
fn spin(iters: u64) {
    for _ in 0..iters {
        core::hint::spin_loop();
    }
}
/// ~microsecond and ~millisecond spins (generous; bring-up is not timing-tight).
fn udelay(us: u64) {
    spin(us * 2_000);
}
fn msleep(ms: u64) {
    spin(ms * 2_000_000);
}

impl Mt7601uDev {
    // --- Register access (usb.c: all register I/O is USB vendor control) ------

    /// Read a 32-bit register (`MT_VEND_MULTI_READ`: IN, 4-byte payload).
    pub fn rr(&mut self, offset: u32) -> u32 {
        if linux_parity_logic::post_qselect_traffic_action(self.mac_init_command_path_unusable)
            == linux_parity_logic::PostQselectTrafficAction::Block
        {
            MAC_INIT_COMMANDS_BLOCKED_AFTER_TERMINAL.fetch_add(1, Ordering::Relaxed);
            return !0;
        }
        let mut buf = [0u8; 4];
        // BUILD-2 C4: retry the vendor control-in like Linux (`mt7601u_rr` →
        // `mt7601u_vendor_request`, 10× / 5 ms) so a transient NAK during the cold
        // firmware-DMA phase doesn't wedge us. A *persistent* failure still returns
        // the all-ones sentinel, which the cold-upload poll (C1) reads as a wedge.
        let ok = vend_retry(|| {
            self.claimed.control_read(
                regs::REQ_TYPE_VENDOR_IN,
                regs::VEND_MULTI_READ,
                0,
                offset as u16,
                4,
                &mut buf,
            )
        })
        .is_ok();
        if ok {
            u32::from_le_bytes(buf)
        } else {
            !0 // the C returns ~0 on a failed vendor read
        }
    }

    /// Write a 32-bit register via a vendor request `req` (low16 then high16,
    /// no data stage — `__mt7601u_vendor_single_wr`).
    fn vendor_single_wr(&mut self, req: u8, offset: u32, val: u32) -> Result<(), &'static str> {
        if linux_parity_logic::post_qselect_traffic_action(self.mac_init_command_path_unusable)
            == linux_parity_logic::PostQselectTrafficAction::Block
        {
            MAC_INIT_COMMANDS_BLOCKED_AFTER_TERMINAL.fetch_add(1, Ordering::Relaxed);
            return Err("post-Q_SELECT terminal blocks further device traffic");
        }
        // BUILD-2 C4: retry each half like Linux's vendor request (10× / 5 ms).
        vend_retry(|| {
            self.claimed.control_no_data(
                regs::REQ_TYPE_VENDOR_OUT,
                req,
                (val & 0xffff) as u16,
                offset as u16,
            )
        })?;
        vend_retry(|| {
            self.claimed.control_no_data(
                regs::REQ_TYPE_VENDOR_OUT,
                req,
                (val >> 16) as u16,
                (offset + 2) as u16,
            )
        })
    }

    /// Write a 32-bit register (`MT_VEND_WRITE`).
    pub fn wr(&mut self, offset: u32, val: u32) -> Result<(), &'static str> {
        self.vendor_single_wr(regs::VEND_WRITE, offset, val)
    }

    /// Read-modify-write (`mt7601u_rmw`).
    pub fn rmw(&mut self, offset: u32, mask: u32, val: u32) -> Result<u32, &'static str> {
        let cur = self.rr(offset);
        let new = val | (cur & !mask);
        self.wr(offset, new)?;
        Ok(new)
    }

    /// `mt76_set` / `mt76_clear` helpers.
    fn set(&mut self, offset: u32, bits: u32) -> Result<u32, &'static str> {
        self.rmw(offset, bits, bits)
    }
    fn clear(&mut self, offset: u32, bits: u32) -> Result<(), &'static str> {
        self.rmw(offset, bits, 0).map(|_| ())
    }

    /// `mt76_poll`: spin until `(rr(offset) & mask) == val` or timeout.
    /// `timeout_us` mirrors the C's microsecond budget.
    pub fn poll(&mut self, offset: u32, mask: u32, val: u32, timeout_us: u32) -> bool {
        let iters = (timeout_us / 10).max(1);
        for _ in 0..=iters {
            if !xhci::charge_operation(ChargeKind::Observation) {
                return false;
            }
            if self.rr(offset) & mask == val {
                return true;
            }
            if !xhci::charge_operation(ChargeKind::PollGap) {
                return false;
            }
            udelay(10);
        }
        false
    }

    /// BUILD-2 C1: the cold-upload-aware poll. Unlike the boolean [`Self::poll`],
    /// this uses [`upload_logic::poll_step`] so an all-ones (`0xffffffff`) read is
    /// **`Unresponsive` (fail fast)** rather than a false-pass of a `BIT(31)`-style
    /// mask — the BUILD-1 wedge was hidden because `0xffffffff & BIT(31) == BIT(31)`
    /// read as "DMA done". Returns the outcome plus the last value read (for the
    /// C3 breadcrumb). `Retry` means the timeout elapsed without a match.
    fn cold_poll(
        &mut self,
        offset: u32,
        mask: u32,
        val: u32,
        timeout_us: u32,
    ) -> (upload_logic::PollStep, u32) {
        let iters = (timeout_us / 10).max(1);
        let mut last = 0u32;
        for _ in 0..=iters {
            last = self.rr(offset);
            match upload_logic::poll_step(last, mask, val) {
                upload_logic::PollStep::Retry => {}
                resolved => return (resolved, last),
            }
            udelay(10);
        }
        (upload_logic::PollStep::Retry, last)
    }

    /// Post-firmware WPDMA poll with an explicit all-ones terminal state.
    ///
    /// Unlike generic [`Self::poll`], a failed vendor read cannot be mistaken
    /// for "still busy" and multiplied by all 10,001 outer observations.
    fn post_fw_wpdma_poll(
        &mut self,
        busy_mask: u32,
        timeout_us: u32,
    ) -> (post_fw_logic::WpdmaStep, u32) {
        let iters = (timeout_us / 10).max(1);
        let mut last = 0u32;
        for _ in 0..=iters {
            if !xhci::charge_operation(ChargeKind::Observation) {
                return (post_fw_logic::WpdmaStep::OperationBudgetExhausted, last);
            }
            last = self.rr(regs::WPDMA_GLO_CFG);
            if xhci::operation_budget_snapshot().exhausted {
                return (post_fw_logic::WpdmaStep::OperationBudgetExhausted, last);
            }
            match post_fw_logic::classify_wpdma(last, busy_mask) {
                post_fw_logic::WpdmaStep::Busy => {}
                terminal => return (terminal, last),
            }
            if !xhci::charge_operation(ChargeKind::PollGap) {
                return (post_fw_logic::WpdmaStep::OperationBudgetExhausted, last);
            }
            udelay(10);
        }
        (post_fw_logic::WpdmaStep::Busy, last)
    }

    /// The vendor reset (`mt7601u_vendor_reset`).
    fn vendor_reset(&mut self) -> Result<(), &'static str> {
        // BUILD-2 C4: retry like Linux's `mt7601u_vendor_reset` → vendor_request.
        vend_retry(|| {
            self.claimed.control_no_data(
                regs::REQ_TYPE_VENDOR_OUT,
                regs::VEND_DEV_MODE,
                regs::VEND_DEV_MODE_RESET,
                0,
            )
        })
    }

    // --- BBP / RF register access (phy.c) — used by the PHY port ---------------

    /// BBP register write via `MT_BBP_CSR_CFG` (phy.c `mt7601u_bbp_wr`):
    /// `VAL[7:0] | REG_NUM[15:8] | RW_MODE(BIT19) | BUSY(BIT17)`.
    pub fn bbp_wr(&mut self, offset: u8, val: u8) -> Result<(), &'static str> {
        if !self.poll(regs::BBP_CSR_CFG, regs::BBP_CSR_CFG_BUSY, 0, 1000) {
            return Err("bbp_wr: busy");
        }
        let word = (val as u32)
            | ((offset as u32) << 8)
            | regs::BBP_CSR_CFG_RW_MODE
            | regs::BBP_CSR_CFG_BUSY;
        self.wr(regs::BBP_CSR_CFG, word)
    }

    /// BBP register read (phy.c `mt7601u_bbp_rr`): `REG_NUM | RW_MODE | BUSY |
    /// READ`, then poll BUSY→0 and read `VAL[7:0]`.
    pub fn bbp_rr(&mut self, offset: u8) -> u8 {
        if !self.poll(regs::BBP_CSR_CFG, regs::BBP_CSR_CFG_BUSY, 0, 1000) {
            return 0xff;
        }
        let word = ((offset as u32) << 8)
            | regs::BBP_CSR_CFG_RW_MODE
            | regs::BBP_CSR_CFG_BUSY
            | regs::BBP_CSR_CFG_READ;
        if self.wr(regs::BBP_CSR_CFG, word).is_err() {
            return 0xff;
        }
        if !self.poll(regs::BBP_CSR_CFG, regs::BBP_CSR_CFG_BUSY, 0, 1000) {
            return 0xff;
        }
        (self.rr(regs::BBP_CSR_CFG) & 0xff) as u8
    }

    /// BBP read-modify-write (phy.c `mt7601u_bbp_rmw`).
    pub fn bbp_rmw(&mut self, offset: u8, mask: u8, val: u8) -> u8 {
        let cur = self.bbp_rr(offset);
        let new = val | (cur & !mask);
        let _ = self.bbp_wr(offset, new);
        new
    }

    /// RF register write through `MT_RF_CSR_CFG` (phy.c RF access).
    pub fn rf_wr(&mut self, bank: u8, offset: u8, val: u8) -> Result<(), &'static str> {
        if !self.poll(regs::RF_CSR_CFG, regs::RF_CSR_CFG_KICK, 0, 1000) {
            return Err("rf_wr: kick stuck");
        }
        let word = (val as u32)
            | ((offset as u32 & 0x3f) << 8)
            | ((bank as u32 & 0xf) << 14)
            | regs::RF_CSR_CFG_WR
            | regs::RF_CSR_CFG_KICK;
        self.wr(regs::RF_CSR_CFG, word)
    }

    /// RF register read (phy.c RF access).
    pub fn rf_rr(&mut self, bank: u8, offset: u8) -> u8 {
        if !self.poll(regs::RF_CSR_CFG, regs::RF_CSR_CFG_KICK, 0, 1000) {
            return 0xff;
        }
        let word =
            ((offset as u32 & 0x3f) << 8) | ((bank as u32 & 0xf) << 14) | regs::RF_CSR_CFG_KICK;
        if self.wr(regs::RF_CSR_CFG, word).is_err() {
            return 0xff;
        }
        if !self.poll(regs::RF_CSR_CFG, regs::RF_CSR_CFG_KICK, 0, 1000) {
            return 0xff;
        }
        (self.rr(regs::RF_CSR_CFG) & 0xff) as u8
    }

    fn channel_status_snapshot(&mut self) -> ChannelStatusSnapshot {
        ChannelStatusSnapshot {
            mcu_com0: self.rr(regs::MCU_COM_REG0),
            mcu_com1: self.rr(regs::MCU_COM_REG1),
            wpdma_glo_cfg: self.rr(regs::WPDMA_GLO_CFG),
            usb_dma_cfg: self.rr(regs::USB_DMA_CFG),
            pbf_cfg: self.rr(regs::PBF_CFG),
            fce_pse_ctrl: self.rr(regs::FCE_PSE_CTRL),
            fce_cpu_desc_idx: self.rr(regs::TX_CPU_FROM_FCE_CPU_DESC_IDX),
            fce_pdma_global_conf: self.rr(regs::FCE_PDMA_GLOBAL_CONF),
            mac_status: self.rr(regs::MAC_STATUS),
            rf_csr_cfg: self.rr(regs::RF_CSR_CFG),
            rf_bypass0: self.rr(regs::RF_BYPASS_0),
            rf_setting0: self.rr(regs::RF_SETTING_0),
            rf17_20: [
                self.rf_rr(0, 17),
                self.rf_rr(0, 18),
                self.rf_rr(0, 19),
                self.rf_rr(0, 20),
            ],
        }
    }

    // --- Synchronous bulk OUT (the cdc-ecm async pipe, polled to completion) ---

    /// Submit one bulk-OUT transfer on the in-band pipe and poll until it
    /// completes (firmware chunks + MCU sends are serial — one outstanding).
    /// `addr` is already fenced. Returns the completion code on success.
    fn inband_out_sync(&mut self, addr: FencedDmaAddr, len: u32) -> Result<(), &'static str> {
        let idx = self.inband.submit(addr, len);
        for _ in 0..self.bulk_budget {
            if !xhci::charge_operation(ChargeKind::BulkPoll) {
                return Err("post-firmware operation budget exhausted during bulk-out");
            }
            xhci::poll_events();
            for event in self.inband.completions() {
                if self.inband.index_of(event.parameter) == Some(idx) {
                    let code = event.completion_code();
                    if code == 1 || code == 13 {
                        return Ok(());
                    }
                    return Err("inband bulk-out completion error");
                }
            }
            udelay(5);
        }
        Err("inband bulk-out timeout")
    }

    /// Fence + transmit `bytes` (already staged in `self.staging`) of `len`.
    fn send_staging(&mut self, len: usize) -> Result<(), &'static str> {
        let addr = dma_arena::fence(self.staging.physical_addr().as_u64(), len as u64)
            .map_err(|_| "staging outside the fence (I-14c1)")?;
        if addr.phys() != self.staging.physical_addr().as_u64() {
            return Err("staging fence changed DMA address");
        }
        self.inband_out_sync(addr, len as u32)
    }

    /// Copy `src` into the staging buffer (bounds-checked).
    fn stage(&mut self, src: &[u8]) -> Result<(), &'static str> {
        if src.len() > self.staging.len() {
            return Err("staging overflow");
        }
        // SAFETY: staging is an exclusively owned arena buffer; len bounded.
        unsafe {
            let p = self.staging.as_mut_ptr();
            for (i, &b) in src.iter().enumerate() {
                core::ptr::write_volatile(p.add(i), b);
            }
        }
        Ok(())
    }

    // --- MCU command interface (mcu.c) ----------------------------------------

    /// Ensure exactly one command-response TD owns `self.resp`.
    ///
    /// Mainline arms its response URB before response-bearing commands and
    /// re-arms it immediately after copying each completed RXFCE word. Posting
    /// only after command bulk-OUT creates a response-loss window; posting a new
    /// TD after a timeout leaves the older TD first in the endpoint queue.
    fn ensure_mcu_response_armed(&mut self) -> Result<usize, &'static str> {
        if let Some(index) = self.cmd_resp_pending {
            return Ok(index);
        }
        let addr = dma_arena::fence(self.resp.physical_addr().as_u64(), MCU_RESP_LEN as u64)
            .map_err(|_| "resp outside the fence")?;
        if addr.phys() != self.resp.physical_addr().as_u64() {
            return Err("response fence changed DMA address");
        }
        let index = self.cmd_resp.submit(addr, MCU_RESP_LEN as u32);
        self.cmd_resp_pending = Some(index);
        self.cmd_resp_stats.arms = self.cmd_resp_stats.arms.saturating_add(1);
        Ok(index)
    }

    /// Send an MCU command (frame in staging, bulk OUT on in-band) and, if
    /// `wait_resp`, await the matching `CMD_DONE` on the response pipe.
    pub fn mcu_send(
        &mut self,
        payload: &[u8],
        cmd: u8,
        wait_resp: bool,
    ) -> Result<(), &'static str> {
        self.mcu_send_inner(payload, cmd, wait_resp, None)
    }

    fn mcu_send_inner(
        &mut self,
        payload: &[u8],
        cmd: u8,
        wait_resp: bool,
        mac_init: Option<MacInitChunkBreadcrumb>,
    ) -> Result<(), &'static str> {
        if linux_parity_logic::post_qselect_traffic_action(self.mac_init_command_path_unusable)
            == linux_parity_logic::PostQselectTrafficAction::Block
        {
            MAC_INIT_COMMANDS_BLOCKED_AFTER_TERMINAL.fetch_add(1, Ordering::Relaxed);
            return Err("post-Q_SELECT terminal blocks further MCU traffic");
        }
        if self.channel_command_path_unusable {
            CHANNEL_COMMANDS_BLOCKED_AFTER_TERMINAL.fetch_add(1, Ordering::Relaxed);
            return Err("channel command path classified unusable; command blocked");
        }
        let seq = if wait_resp {
            self.msg_seq = (self.msg_seq % 15) + 1; // 1..=15, never 0
            self.msg_seq
        } else {
            0
        };
        let mut frame = [0u8; regs::INBAND_PACKET_MAX_LEN + 8];
        let n = mcu_logic::build_mcu_frame(&mut frame, payload, seq, cmd)
            .ok_or("mcu frame too large")?;
        if let Some(trace) = mac_init {
            MAC_INIT_LAST_TABLE.store(trace.table, Ordering::Relaxed);
            MAC_INIT_LAST_CHUNK.store(trace.index as u32 + 1, Ordering::Relaxed);
            MAC_INIT_LAST_CHUNKS.store(trace.count as u32, Ordering::Relaxed);
            MAC_INIT_LAST_SEQ.store(seq as u32, Ordering::Relaxed);
            MAC_INIT_LAST_PAYLOAD_LEN.store(payload.len(), Ordering::Relaxed);
            MAC_INIT_LAST_FRAME_LEN.store(n, Ordering::Relaxed);
            MAC_INIT_LAST_WAIT_RESPONSE.store(wait_resp, Ordering::Relaxed);
            crate::serial_println!(
                "mt7601u: MAC-INIT chunk before table={} chunk={}/{} command=RANDOM_WRITE seq={} payload_len={} frame_len={} wait_response={}",
                mac_init_table_label(trace.table),
                trace.index + 1,
                trace.count,
                seq,
                payload.len(),
                n,
                wait_resp,
            );
        }

        let result = (|| {
            // The response buffer must own an xHCI TD before the device can
            // observe any response-bearing command.
            if wait_resp {
                self.ensure_mcu_response_armed()?;
            }
            self.stage(&frame[..n])?;
            self.send_staging(n)?;
            if wait_resp {
                self.mcu_wait_resp(seq)?;
            }
            Ok(())
        })();

        if let Some(trace) = mac_init {
            match result {
                Ok(()) => {
                    crate::serial_println!(
                        "mt7601u: MAC-INIT chunk after table={} chunk={}/{} command=RANDOM_WRITE seq={} result=OK",
                        mac_init_table_label(trace.table),
                        trace.index + 1,
                        trace.count,
                        seq,
                    );
                }
                Err(reason) => {
                    crate::serial_println!(
                        "mt7601u: MAC-INIT chunk after table={} chunk={}/{} command=RANDOM_WRITE seq={} result=FAIL reason={}",
                        mac_init_table_label(trace.table),
                        trace.index + 1,
                        trace.count,
                        seq,
                        reason,
                    );
                }
            }
        }
        result
    }

    /// Await an MCU response and match seq + `CMD_DONE`.
    ///
    /// A no-event window continues waiting on the same outstanding TD; it never
    /// queues a second TD behind it. After a completion, copy RXFCE before
    /// reusing the buffer, then re-arm and validate the response.
    fn mcu_wait_resp(&mut self, seq: u8) -> Result<(), &'static str> {
        for _ in 0..5 {
            if !xhci::charge_operation(ChargeKind::BulkAttempt) {
                return Err("post-firmware operation budget exhausted before MCU response attempt");
            }
            let pending = self.ensure_mcu_response_armed()?;
            let mut owned_event = None;
            for _ in 0..self.bulk_budget {
                if !xhci::charge_operation(ChargeKind::BulkPoll) {
                    return Err(
                        "post-firmware operation budget exhausted during MCU response wait",
                    );
                }
                xhci::poll_events();
                for event in self.cmd_resp.completions() {
                    let Some(event_index) = self.cmd_resp.index_of(event.parameter) else {
                        self.cmd_resp_stats.unowned_events =
                            self.cmd_resp_stats.unowned_events.saturating_add(1);
                        continue;
                    };
                    match mcu_logic::classify_response_transfer(
                        pending,
                        event_index,
                        event.completion_code(),
                        MCU_RESP_LEN,
                        event.transfer_residue(),
                    ) {
                        mcu_logic::ResponseTransfer::Unowned => {
                            self.cmd_resp_stats.unowned_events =
                                self.cmd_resp_stats.unowned_events.saturating_add(1);
                        }
                        verdict => {
                            owned_event = Some(verdict);
                            break;
                        }
                    }
                }
                if owned_event.is_some() {
                    break;
                }
                udelay(5);
            }
            let Some(verdict) = owned_event else {
                self.cmd_resp_stats.timeout_windows =
                    self.cmd_resp_stats.timeout_windows.saturating_add(1);
                continue;
            };

            self.cmd_resp_pending = None;
            let rxfce = match verdict {
                mcu_logic::ResponseTransfer::Read { .. } => {
                    // SAFETY: the owned TD completed at least four bytes into
                    // the exclusively owned response buffer.
                    unsafe {
                        let p = self.resp.as_ptr();
                        u32::from_le_bytes([
                            core::ptr::read_volatile(p),
                            core::ptr::read_volatile(p.add(1)),
                            core::ptr::read_volatile(p.add(2)),
                            core::ptr::read_volatile(p.add(3)),
                        ])
                    }
                }
                mcu_logic::ResponseTransfer::Invalid => {
                    self.ensure_mcu_response_armed()?;
                    return Err("MCU response transfer invalid");
                }
                mcu_logic::ResponseTransfer::Unowned => unreachable!(),
            };
            self.cmd_resp_stats.completions = self.cmd_resp_stats.completions.saturating_add(1);
            // Keep one response TD armed across commands, as mainline does.
            self.ensure_mcu_response_armed()?;
            if mcu_logic::resp_is_done(rxfce, seq) {
                return Ok(());
            }
            let decoded = mcu_logic::decode_resp(rxfce);
            self.cmd_resp_stats.mismatches = self.cmd_resp_stats.mismatches.saturating_add(1);
            crate::serial_println!(
                "mt7601u: MCU response mismatch want_seq={} got_seq={} evt_type={} pending_rearmed=1",
                seq,
                decoded.seq,
                decoded.evt_type,
            );
        }
        Err("mcu response timed out")
    }

    /// RANDOM_WRITE a set of reg pairs to `base` (chunked ≤ 24/cmd; mcu.c
    /// `mt7601u_write_reg_pairs`).
    pub fn mcu_write_reg_pairs(
        &mut self,
        base: u32,
        pairs: &[(u32, u32)],
    ) -> Result<(), &'static str> {
        let chunk_count = pairs.len().div_ceil(mcu_logic::MAX_REG_PAIRS_PER_CMD);
        for (index, chunk) in pairs.chunks(mcu_logic::MAX_REG_PAIRS_PER_CMD).enumerate() {
            let mut payload = [0u8; regs::INBAND_PACKET_MAX_LEN];
            let n = mcu_logic::build_reg_pair_payload(&mut payload, base, chunk)
                .ok_or("reg-pair payload")?;
            // Mainline requests CMD_DONE only for the final chunk. Waiting on
            // every intermediate chunk adds response traffic not present in the
            // reference sequence and increases response-ring pressure.
            self.mcu_send(
                &payload[..n],
                regs::CMD_RANDOM_WRITE,
                mcu_logic::wait_response_for_chunk(index, chunk_count),
            )?;
        }
        Ok(())
    }

    fn mcu_write_mac_reg_pairs(
        &mut self,
        table: u32,
        pairs: &[(u32, u32)],
    ) -> Result<(), &'static str> {
        let chunk_count = pairs.len().div_ceil(mcu_logic::MAX_REG_PAIRS_PER_CMD);
        for (index, chunk) in pairs.chunks(mcu_logic::MAX_REG_PAIRS_PER_CMD).enumerate() {
            let mut payload = [0u8; regs::INBAND_PACKET_MAX_LEN];
            let n = mcu_logic::build_reg_pair_payload(&mut payload, regs::MCU_MEMMAP_WLAN, chunk)
                .ok_or("MAC reg-pair payload")?;
            let wait_resp = mcu_logic::wait_response_for_chunk(index, chunk_count);
            self.mcu_send_inner(
                &payload[..n],
                regs::CMD_RANDOM_WRITE,
                wait_resp,
                Some(MacInitChunkBreadcrumb {
                    table,
                    index,
                    count: chunk_count,
                }),
            )?;
        }
        Ok(())
    }

    /// BURST_WRITE a run of words to `offset` (chunked ≤ 47/cmd; mcu.c
    /// `mt7601u_burst_write_regs`).
    pub fn mcu_burst_write(&mut self, offset: u32, data: &[u32]) -> Result<(), &'static str> {
        let mut off = offset;
        let chunk_count = data.len().div_ceil(mcu_logic::MAX_BURST_REGS_PER_CMD);
        for (index, chunk) in data.chunks(mcu_logic::MAX_BURST_REGS_PER_CMD).enumerate() {
            let mut payload = [0u8; regs::INBAND_PACKET_MAX_LEN];
            let n =
                mcu_logic::build_burst_payload(&mut payload, off, chunk).ok_or("burst payload")?;
            self.mcu_send(
                &payload[..n],
                regs::CMD_BURST_WRITE,
                mcu_logic::wait_response_for_chunk(index, chunk_count),
            )?;
            off += (chunk.len() * 4) as u32;
        }
        Ok(())
    }

    /// MCU function-select (mcu.c `mt7601u_mcu_function_select`).
    pub fn mcu_function_select(&mut self, func: u32, val: u32) -> Result<(), &'static str> {
        let mut payload = [0u8; 8];
        payload[0..4].copy_from_slice(&func.to_le_bytes());
        payload[4..8].copy_from_slice(&val.to_le_bytes());
        self.mcu_send(&payload, regs::CMD_FUN_SET_OP, func == 5)
    }

    /// MCU calibration (mcu.c `mt7601u_mcu_calibrate`).
    pub fn mcu_calibrate(&mut self, cal: u32, val: u32) -> Result<(), &'static str> {
        let mut payload = [0u8; 8];
        payload[0..4].copy_from_slice(&cal.to_le_bytes());
        payload[4..8].copy_from_slice(&val.to_le_bytes());
        self.mcu_send(&payload, regs::CMD_CALIBRATION_OP, true)
    }

    // --- Firmware load (mcu.c `mt7601u_load_firmware`) ------------------------

    fn firmware_running(&mut self) -> bool {
        self.rr(regs::MCU_COM_REG0) == 1
    }

    /// Upload one firmware region in `MCU_FW_URB_MAX_PAYLOAD` chunks via FCE DMA
    /// (mcu.c `mt7601u_dma_fw` / `__mt7601u_dma_fw`). BUILD-2 C1/C3: the per-chunk
    /// DMA-done poll uses [`Self::cold_poll`] (an all-ones `COM_REG1` read is
    /// `Unresponsive`, not a false-pass of `BIT(31)`); each chunk emits a
    /// breadcrumb (index/offset/dst/`COM_REG1`); and a bus drop fails fast,
    /// **naming the wedge chunk** and setting `*unresponsive`.
    fn dma_fw(
        &mut self,
        data: &[u8],
        dst_addr: u32,
        region: &str,
        attempt: usize,
        unresponsive: &mut bool,
    ) -> Result<(), &'static str> {
        let mut off = 0usize;
        let mut dst = dst_addr;
        let mut chunk_idx = 0usize;
        while off < data.len() {
            let n = (data.len() - off).min(regs::MCU_FW_URB_MAX_PAYLOAD);
            let chunk = &data[off..off + n];
            // FCE DMA addr + len (vendor req 0x42).
            self.vendor_single_wr(regs::VEND_WRITE_FCE, regs::FCE_DMA_ADDR, dst)?;
            self.vendor_single_wr(
                regs::VEND_WRITE_FCE,
                regs::FCE_DMA_LEN,
                fw_logic::fce_dma_len_reg(n),
            )?;
            // Build [4B TXINFO][chunk][zero pad] into staging and transmit.
            let xfer = {
                // SAFETY: staging owned arena buffer; build_chunk bounds-checks.
                let dst_slice = unsafe {
                    core::slice::from_raw_parts_mut(self.staging.as_mut_ptr(), self.staging.len())
                };
                fw_logic::build_chunk(dst_slice, chunk).ok_or("fw chunk staging overflow")?
            };
            self.send_staging(xfer)?;
            // Bump the FCE CPU desc index (this kicks the FCE to process the
            // descriptor) — the C does this at the END of __mt7601u_dma_fw,
            // BEFORE the caller polls COM_REG1. Order matters: poll-before-bump
            // would wait on a DMA the engine was never told to start.
            let v = self.rr(regs::TX_CPU_FROM_FCE_CPU_DESC_IDX).wrapping_add(1);
            self.wr(regs::TX_CPU_FROM_FCE_CPU_DESC_IDX, v)?;
            self.fce_upload_kicks = self.fce_upload_kicks.saturating_add(1);
            // C1: poll FCE DMA done (COM_REG1 bit 31) with the all-ones-aware
            // predicate. C3: breadcrumb every chunk + the COM_REG1 value read, so
            // a Stage-B failure shows the last-good chunk before the bus drop.
            let (step, com_reg1) = self.cold_poll(regs::MCU_COM_REG1, 1 << 31, 1 << 31, 500_000);
            crate::serial_println!(
                "mt7601u: fw a{} {} chunk={} off={} dst={:#x} com_reg1={:#010x}",
                attempt,
                region,
                chunk_idx,
                off,
                dst,
                com_reg1,
            );
            match step {
                upload_logic::PollStep::Done => {}
                upload_logic::PollStep::Unresponsive => {
                    *unresponsive = true;
                    crate::serial_println!(
                        "mt7601u: fw a{} {} chunk={} DEVICE UNRESPONSIVE (com_reg1 all-ones) — wedge localized here",
                        attempt,
                        region,
                        chunk_idx,
                    );
                    return Err("fw FCE DMA: device unresponsive (all-ones)");
                }
                upload_logic::PollStep::Retry => return Err("fw FCE DMA timeout"),
            }
            off += n;
            dst += n as u32;
            chunk_idx += 1;
        }
        Ok(())
    }

    /// The full firmware-load handshake (mcu.c `mt7601u_load_firmware` +
    /// `mt7601u_upload_firmware`).
    /// FCE/PBF pre-load register setup for a cold firmware upload (mcu.c 446-477).
    /// Extracted from `load_firmware` so its retry loop can treat a transient
    /// control-transfer wedge here as a retryable attempt (W1 F2: cold-upload
    /// reliability) instead of aborting bind via `?` on the first failure.
    fn fce_preload_setup(&mut self) -> Result<(), &'static str> {
        self.wr(0x94c, 0)?;
        self.wr(regs::FCE_PSE_CTRL, 0)?;
        self.vendor_reset()?;
        msleep(5);
        self.wr(0xa44, 0)?;
        self.wr(0x230, 0x84210)?;
        self.wr(0x400, 0x80c00)?;
        self.wr(0x800, 1)?;
        self.rmw(
            regs::PBF_CFG,
            0,
            regs::PBF_CFG_TX0Q_EN
                | regs::PBF_CFG_TX1Q_EN
                | regs::PBF_CFG_TX2Q_EN
                | regs::PBF_CFG_TX3Q_EN,
        )?;
        self.wr(regs::FCE_PSE_CTRL, 1)?;
        self.wr(
            regs::USB_DMA_CFG,
            regs::USB_DMA_CFG_RX_BULK_EN | regs::USB_DMA_CFG_TX_BULK_EN,
        )?;
        let v = self.set(regs::USB_DMA_CFG, regs::USB_DMA_CFG_TX_CLR)?;
        self.wr(regs::USB_DMA_CFG, v & !regs::USB_DMA_CFG_TX_CLR)?;
        self.wr(regs::TX_CPU_FROM_FCE_BASE_PTR, 0x400230)?;
        self.wr(regs::TX_CPU_FROM_FCE_MAX_COUNT, 1)?;
        self.wr(regs::FCE_PDMA_GLOBAL_CONF, 0x44)?;
        self.wr(regs::FCE_SKIP_FS, 3)?;
        Ok(())
    }

    fn load_firmware(&mut self) -> Result<(), &'static str> {
        self.wr(
            regs::USB_DMA_CFG,
            regs::USB_DMA_CFG_RX_BULK_EN | regs::USB_DMA_CFG_TX_BULK_EN,
        )?;
        if linux_parity_logic::firmware_load_action(self.firmware_running())
            == linux_parity_logic::FirmwareLoadAction::UseResident
        {
            // Linux `mt7601u_load_firmware`: resident firmware returns
            // immediately after the initial USB-DMA enable. Cold preload,
            // vendor reset, and FCE/PBF programming are mutually exclusive.
            WARM_RESYNC_STATE.store(warm_resync_state::RESIDENT_EARLY_RETURN, Ordering::Relaxed);
            crate::serial_println!(
                "mt7601u: firmware already resident — Linux early-return preload_runs=0 vendor_resets=0 fce_pbf_writes=0 upload_skipped=true"
            );
            crate::log::flush();
            return Ok(());
        }
        WARM_RESYNC_STATE.store(warm_resync_state::COLD, Ordering::Relaxed);

        // OT-27a (ISC-2): the blob is a Limine boot module, NOT `include_bytes!`
        // — fail-closed if it wasn't staged (clear error, not a crash). Only the
        // COLD path reaches here; a warm-resident MCU returned above without
        // touching the module, so a warm device needs no blob.
        let firmware = firmware_blob()?;
        let hdr = fw_logic::parse_header(firmware).ok_or("fw image truncated (-ENOENT)")?;
        let layout =
            fw_logic::validate(firmware.len(), &hdr).map_err(|_| "fw image invalid (-ENOENT)")?;
        // ISC-3 discriminator (capture-before-you-need-it): the chip-id breadcrumb.
        // The wrong-USB-variant bucket is actually rejected earlier — `bring_up`
        // errors `ASIC version mismatch (not 0x7601)` before here — so this records
        // that the silicon was right when a *later* upload stage fails.
        let asic = ASIC_REV.load(Ordering::Relaxed);
        let chip_id_ok = (asic >> 16) as u16 == regs::ASIC_REV_MT7601;
        let (a, b, c) = fw_logic::fw_version(&hdr);
        crate::serial_println!(
            "mt7601u: cold fw upload — {}.{}.{:02} build {:#x} ({} bytes), chip {:#010x}",
            a,
            b,
            c,
            hdr.build_ver,
            firmware.len(),
            asic,
        );

        let ilm = &firmware[layout.ilm_off..layout.ilm_off + layout.ilm_data_len];
        let dlm = &firmware[layout.dlm_off..layout.dlm_off + layout.dlm_len];
        let ivb = &firmware[layout.ivb_off..layout.ivb_off + regs::MCU_IVB_SIZE];
        let bytes_total = (ilm.len() + dlm.len() + ivb.len()) as u32;

        // Cold-upload phase: bound the xHCI wait so a wedged control transfer
        // fails FAST (seconds), letting the retry recover or fail-with-
        // diagnostics rather than stall ~minutes on the full budget. Restored
        // on return by the guard's Drop.
        let _budget = xhci::WaitBudgetGuard::new(FW_LOAD_WAIT_BUDGET);

        // ISC-1/ISC-3: a COLD firmware boot can need a reset-and-retry — the chunks
        // DMA in fine, but the MCU sometimes doesn't latch COM_REG0=1 on the first
        // try (silicon Stage-B). Each attempt re-runs `fce_preload_setup` (which
        // does `vendor_reset` + `msleep(5)` — the THINK §5 P2 recovery), so the
        // 3-attempt loop is a *bounded vendor-reset retry*. These breadcrumbs are
        // kept ACROSS attempts so a final failure self-localizes: `bytes_uploaded`
        // is the furthest any attempt's upload egressed (endpoint-stall vs done),
        // and `com_reg0_ever_nonzero` splits "MCU never stirred" from "latched ≠ 1".
        let mut bytes_uploaded: u32 = 0;
        let mut com_reg0_ever_nonzero = false;
        // BUILD-2 C1/C2: set when any cold-upload poll sees the all-ones bus-drop
        // sentinel, or a control transfer reports "no event" — the device fell off
        // the bus. The dominant failure signal (classified `DeviceUnresponsive`).
        let mut device_unresponsive = false;

        const FW_LOAD_ATTEMPTS: usize = 3;
        for attempt in 1..=FW_LOAD_ATTEMPTS {
            // Pre-load register setup (mcu.c 446-477) for cold initialization.
            // A transient control-transfer wedge here logs and falls through to the
            // next attempt instead of aborting bind via `?` (W1 F2 gap).
            if let Err(e) = self.fce_preload_setup() {
                // C1/C2: a control "no event" here = the device is already off the
                // bus (the BUILD-1 attempts-2/3 signature after an attempt-1 wedge).
                if e.contains("no event") {
                    device_unresponsive = true;
                }
                crate::serial_println!("mt7601u: fw attempt {} reg setup: {}", attempt, e);
                continue;
            }

            // Upload ILM (dst = IVB size) → DLM (dst = DLM offset) → IVB (control),
            // tracking byte-progress for the endpoint-stall discriminator. C3: each
            // region/chunk breadcrumbs inside `dma_fw`; a bus drop names the chunk.
            let mut done: u32 = 0;
            if let Err(e) = self.dma_fw(
                ilm,
                regs::MCU_IVB_SIZE as u32,
                "ILM",
                attempt,
                &mut device_unresponsive,
            ) {
                crate::serial_println!(
                    "mt7601u: fw attempt {} ILM upload ({}/{}B): {}",
                    attempt,
                    done,
                    bytes_total,
                    e
                );
                continue;
            }
            done += ilm.len() as u32;
            if let Err(e) = self.dma_fw(
                dlm,
                regs::MCU_DLM_OFFSET,
                "DLM",
                attempt,
                &mut device_unresponsive,
            ) {
                crate::serial_println!(
                    "mt7601u: fw attempt {} DLM upload ({}/{}B): {}",
                    attempt,
                    done,
                    bytes_total,
                    e
                );
                bytes_uploaded = bytes_uploaded.max(done);
                continue;
            }
            done += dlm.len() as u32;
            // BUILD-2 C4: the IVB upload is the firmware-boot trigger — retry it
            // like Linux's `mt7601u_vendor_request` (10× / 5 ms).
            let ivb_res = vend_retry(|| {
                self.claimed.control_write(
                    regs::REQ_TYPE_VENDOR_OUT,
                    regs::VEND_DEV_MODE,
                    regs::FW_IVB_DEV_MODE_VALUE,
                    0,
                    ivb,
                )
            });
            if let Err(e) = ivb_res {
                if e.contains("no event") {
                    device_unresponsive = true;
                }
                crate::serial_println!(
                    "mt7601u: fw attempt {} IVB upload ({}/{}B): {}",
                    attempt,
                    done,
                    bytes_total,
                    e
                );
                bytes_uploaded = bytes_uploaded.max(done);
                continue;
            }
            done += ivb.len() as u32;
            bytes_uploaded = bytes_uploaded.max(done);
            crate::serial_println!(
                "mt7601u: fw a{} IVB uploaded ({}B) — awaiting COM_REG0==1",
                attempt,
                ivb.len(),
            );

            // BUILD-2 C1 success oracle: the all-ones-aware predicate — COM_REG0==1
            // ⇒ booted; all-ones ⇒ the device fell off the bus (fail fast this
            // attempt, don't burn the full 3s reporting a phantom McuNoBoot);
            // genuine nonzero ⇒ the MCU stirred. (300 × 10ms.)
            let mut booted = false;
            for i in 0..300 {
                let r0 = self.rr(regs::MCU_COM_REG0);
                match upload_logic::poll_step(r0, u32::MAX, 1) {
                    upload_logic::PollStep::Done => {
                        crate::serial_println!("mt7601u: firmware booted (attempt {})", attempt);
                        booted = true;
                        break;
                    }
                    upload_logic::PollStep::Unresponsive => {
                        device_unresponsive = true;
                        crate::serial_println!(
                            "mt7601u: fw attempt {} ready-poll DEVICE UNRESPONSIVE (COM_REG0 all-ones) at poll={}",
                            attempt,
                            i,
                        );
                        break;
                    }
                    upload_logic::PollStep::Retry => {
                        if r0 != 0 {
                            com_reg0_ever_nonzero = true;
                        }
                    }
                }
                msleep(10);
            }
            if booted {
                return Ok(());
            }
            crate::serial_println!(
                "mt7601u: fw attempt {} ready-poll done — COM_REG0={:#010x} COM_REG1={:#010x} unresponsive={}",
                attempt,
                self.rr(regs::MCU_COM_REG0),
                self.rr(regs::MCU_COM_REG1),
                device_unresponsive,
            );
        }

        // ISC-3: every attempt failed — bucket it from the captured breadcrumbs so
        // the blind Stage-B run is conclusive on failure (THINK P2/P6). Blob-corrupt
        // and wrong-variant are caught earlier with their own errors; here the live
        // discriminators are endpoint-stall (upload never finished) vs MCU-no-boot
        // (all bytes in, COM_REG0 never reached 1).
        let progress = upload_logic::ColdUploadProgress {
            blob_valid: true, // parse + validate already passed above
            chip_id_ok,
            bytes_total,
            bytes_uploaded,
            device_unresponsive,
            com_reg0_ever_nonzero,
            com_reg0_running: false,
        };
        crate::serial_println!(
            "mt7601u: cold fw upload FAILED — fault={:?} uploaded={}/{}B unresponsive={} com_reg0_stirred={} chip={:#010x}",
            upload_logic::classify_cold_upload_failure(&progress),
            bytes_uploaded,
            bytes_total,
            device_unresponsive,
            com_reg0_ever_nonzero,
            asic,
        );
        Err("firmware ready-poll timeout (-ETIMEDOUT)")
    }

    // --- eFUSE / EEPROM read (eeprom.c) ---------------------------------------

    /// Read one 16-byte eFUSE block into `out` (eeprom.c `mt7601u_efuse_read`).
    fn efuse_read(&mut self, addr: u16, out: &mut [u8; 16], mode: u32) -> Result<(), &'static str> {
        let cur = self.rr(regs::EFUSE_CTRL);
        let kick = eeprom_logic::efuse_ctrl_kick(cur, addr, mode);
        self.wr(regs::EFUSE_CTRL, kick)?;
        if !self.poll(regs::EFUSE_CTRL, regs::EFUSE_CTRL_KICK, 0, 10_000) {
            return Err("efuse kick timeout");
        }
        let after = self.rr(regs::EFUSE_CTRL);
        if eeprom_logic::efuse_out_of_map(after) {
            out.fill(0xff);
            return Ok(());
        }
        for i in 0..4 {
            let v = self.rr(regs::efuse_data(i as u32));
            out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        Ok(())
    }

    /// The eFUSE physical-size HARD-FAIL gate (eeprom.c
    /// `mt7601u_efuse_physical_size_check`): a blank clone aborts here. For the
    /// confirmed adapter the usage map is programmed and this passes (§2).
    fn efuse_size_check(&mut self) -> Result<(), &'static str> {
        let mut map = [0u8; 32];
        let reads = regs::EFUSE_USAGE_MAP_SIZE.div_ceil(16);
        for i in 0..reads {
            let mut block = [0u8; 16];
            self.efuse_read(
                (regs::EE_USAGE_MAP_START + i * 16) as u16,
                &mut block,
                regs::EE_PHYSICAL_READ,
            )?;
            map[i * 16..i * 16 + 16].copy_from_slice(&block);
        }
        eeprom_logic::usage_map_ok(&map)
            .map_err(|_| "eFUSE blank — needs default EEPROM (-EINVAL, T-011)")
    }

    /// Read + parse the EEPROM (eeprom.c `mt7601u_eeprom_init`). Returns the
    /// parsed params and the readback MAC (the §2 discriminator).
    fn eeprom_init(&mut self) -> Result<[u8; 6], &'static str> {
        self.efuse_size_check()?;
        let mut ee = [0u8; regs::EEPROM_SIZE];
        let mut i = 0;
        while i + 16 <= regs::EEPROM_SIZE {
            let mut block = [0u8; 16];
            self.efuse_read(i as u16, &mut block, regs::EE_READ)?;
            ee[i..i + 16].copy_from_slice(&block);
            i += 16;
        }
        let ver = eeprom_logic::version(&ee);
        let fae = eeprom_logic::version_fae(&ee);
        EE_VER.store(ver as u32, Ordering::Relaxed);
        if eeprom_logic::version_unsupported(&ee) {
            crate::serial_println!("mt7601u: unsupported EEPROM version {:#04x}", ver);
        }
        crate::serial_println!("mt7601u: EEPROM ver={:#04x} fae={:#04x}", ver, fae);
        let mac = eeprom_logic::mac(&ee);
        *READBACK_MAC.lock() = mac;

        // Per-channel TX power (eeprom.c `mt7601u_set_channel_power`). TWO paths —
        // and which one matters: a TSSI part uses the TSSI TARGET power as the ALC
        // setpoint, NOT the per-channel 0x52 value.
        //   - TSSI enabled: chan_pwr[all] = trgt_pwr (EEPROM 0xd0, ~0x20) — the
        //     closed loop's target. Used open-loop here it's far higher than 0x52.
        //   - TSSI disabled: chan_pwr[i] = field_validate(eeprom[0x52 + i]).
        // W1 used a hardcoded 0x0d placeholder; my first port did ONLY the non-TSSI
        // path (0x52 → 0x0c on this dongle), which is far too weak for a TSSI part —
        // the reason the AP still couldn't hear us after the per-rate fix. max_pwr is
        // the ALC LIMIT field (TX_ALC_CFG_0 bits 16-21).
        let alc = self.rr(regs::TX_ALC_CFG_0);
        let mut max_pwr =
            ((alc >> regs::TX_ALC_CFG_0_LIMIT_0_SHIFT) & regs::TX_ALC_CFG_0_LIMIT_0_MASK) as i32;
        if max_pwr == 0 {
            max_pwr = 0x2f; // ALC reg not yet programmed — sane ceiling so valid EEPROM values pass
        }
        // mt7601u_has_tssi: nic_conf1 valid (not 0xffff) AND TX_ALC_EN (bit 13) set.
        let nic_conf1 = u16::from_le_bytes([ee[regs::EE_NIC_CONF_1], ee[regs::EE_NIC_CONF_1 + 1]]);
        let has_tssi = nic_conf1 != 0xffff && (nic_conf1 & regs::EE_NIC_CONF_1_TX_ALC_EN) != 0;
        let mut chan_pwr = [regs::EE_DEFAULT_TX_POWER; 14];
        if has_tssi {
            // mt7601u_set_channel_target_power: ALC target = the TSSI target power
            // (EEPROM 0xd0), clamped to max_pwr; default 0x20 if zero/out-of-range.
            let mut trgt = ee[regs::EE_TX_TSSI_TARGET_POWER] as i32;
            if trgt == 0 || trgt > max_pwr {
                trgt = regs::EE_TSSI_TARGET_DEFAULT as i32;
            }
            chan_pwr = [trgt as u8; 14];
        } else {
            for (i, slot) in chan_pwr.iter_mut().enumerate() {
                let raw = ee[regs::EE_TX_POWER_OFFSET + i];
                let p = if raw == 0xff { 0 } else { raw as i8 as i32 }; // field_validate
                // upstream rejects p<0 or p>max; also treat 0/blank as invalid → default.
                *slot = if p <= 0 || p > max_pwr {
                    regs::EE_DEFAULT_TX_POWER
                } else {
                    p as u8
                };
            }
        }
        // Per-rate TX power words (eeprom.c `config_tx_power_per_rate`): 5 LE u32 at
        // EE_TX_POWER_BYRATE_BASE, written to TX_PWR_CFG_0..4 by phy::phy_init.
        let mut tx_pwr_cfg = [0u32; 5];
        for (i, slot) in tx_pwr_cfg.iter_mut().enumerate() {
            let o = regs::EE_TX_POWER_BYRATE_BASE + i * 4;
            *slot = u32::from_le_bytes([ee[o], ee[o + 1], ee[o + 2], ee[o + 3]]);
        }
        // tssi_enabled (eeprom.c): has_tssi AND NOT TEMP_TX_ALC (bit 1). Drives the
        // (deferred) closed-loop cal; the ALC-target choice above already uses it.
        let tssi_enabled = has_tssi && (nic_conf1 & regs::EE_NIC_CONF_1_TEMP_TX_ALC) == 0;
        crate::serial_println!(
            "mt7601u: eeprom tx-power: has_tssi={} chan_pwr[0]={:#04x} max_pwr={:#04x} byrate0={:#010x}",
            has_tssi,
            chan_pwr[0],
            max_pwr,
            tx_pwr_cfg[0],
        );

        // The PHY-relevant params (eeprom.c setters; country bounds default 1-14).
        self.ee = EepromParams {
            rf_freq_off: ee[regs::EE_FREQ_OFFSET],
            rssi_offset: [ee[0x46] as i8, ee[0x47] as i8],
            lna_gain: ee[0x44] as i8,
            ref_temp: ee[0xd1] as i8,
            tssi_enabled,
            reg_start: 1,
            reg_num: 14,
            chan_pwr,
            tx_pwr_cfg,
        };
        Ok(mac)
    }

    // --- MAC start + RX (init.c `mt7601u_mac_start`) ---------------------------

    /// Enable TX then RX with the scan RX filter (init.c `mt7601u_mac_start`).
    fn mac_start(&mut self) -> Result<(), &'static str> {
        self.wr(regs::MAC_SYS_CTRL, regs::MAC_SYS_CTRL_ENABLE_TX)?;
        if !self.poll(
            regs::WPDMA_GLO_CFG,
            regs::WPDMA_GLO_CFG_TX_DMA_BUSY | regs::WPDMA_GLO_CFG_RX_DMA_BUSY,
            0,
            2_000_000,
        ) {
            return Err("mac_start: WPDMA busy (TX)");
        }
        let rxfilter = regs::RX_FILTR_CFG_CRC_ERR
            | regs::RX_FILTR_CFG_PHY_ERR
            | regs::RX_FILTR_CFG_PROMISC
            | regs::RX_FILTR_CFG_VER_ERR
            | regs::RX_FILTR_CFG_DUP
            | regs::RX_FILTR_CFG_CFACK
            | regs::RX_FILTR_CFG_CFEND
            | regs::RX_FILTR_CFG_ACK
            | regs::RX_FILTR_CFG_CTS
            | regs::RX_FILTR_CFG_RTS
            | regs::RX_FILTR_CFG_PSPOLL
            | regs::RX_FILTR_CFG_BA
            | regs::RX_FILTR_CFG_CTRL_RSV;
        self.wr(regs::RX_FILTR_CFG, rxfilter)?;
        self.wr(
            regs::MAC_SYS_CTRL,
            regs::MAC_SYS_CTRL_ENABLE_TX | regs::MAC_SYS_CTRL_ENABLE_RX,
        )?;
        if !self.poll(
            regs::WPDMA_GLO_CFG,
            regs::WPDMA_GLO_CFG_TX_DMA_BUSY | regs::WPDMA_GLO_CFG_RX_DMA_BUSY,
            0,
            500,
        ) {
            return Err("mac_start: WPDMA busy (RX)");
        }
        Ok(())
    }

    /// Post one RX buffer on the packet-RX pipe (fenced).
    fn post_rx(&mut self, buffer: Arc<DmaBuffer>) -> Result<(), &'static str> {
        let addr = dma_arena::fence(buffer.physical_addr().as_u64(), RX_BUF_LEN as u64)
            .map_err(|_| "rx buffer outside the fence")?;
        let idx = self.pkt_rx.submit(addr, RX_BUF_LEN as u32);
        self.rx_table[idx] = Some(buffer);
        Ok(())
    }

    fn posted_rx_count(&self) -> usize {
        self.rx_table.iter().filter(|entry| entry.is_some()).count()
    }

    /// Linux posts packet-RX URBs during DMA initialization, before the first
    /// MAC RANDOM_WRITE. The scan path calls this again and reuses the same pool.
    fn ensure_packet_rx_ready(&mut self) -> Result<usize, &'static str> {
        let posted = self.posted_rx_count();
        match linux_parity_logic::packet_rx_action(posted, RX_BUFFERS) {
            linux_parity_logic::PacketRxAction::PostMissing { count } => {
                for _ in 0..count {
                    let buffer =
                        DmaBuffer::alloc_in_arena(RX_BUF_LEN).map_err(|_| "rx pool alloc")?;
                    self.post_rx(buffer)?;
                }
            }
            linux_parity_logic::PacketRxAction::ReusePosted => {}
            linux_parity_logic::PacketRxAction::InvalidOwnership => {
                return Err("packet-RX ownership exceeds fixed pool");
            }
        }
        let ready = self.posted_rx_count();
        if ready != RX_BUFFERS {
            return Err("packet-RX pool incomplete");
        }
        Ok(ready)
    }

    /// Drain received frames, parse beacons/probe-responses, and add new
    /// BSSIDs to the scan list. Returns the number of new BSSIDs this pass.
    fn drain_rx(&mut self) -> usize {
        let mut added = 0;
        let events = self.pkt_rx.completions();
        for event in events {
            let Some(idx) = self.pkt_rx.index_of(event.parameter) else {
                continue;
            };
            let Some(buffer) = self.rx_table[idx].take() else {
                continue;
            };
            let code = event.completion_code();
            if code == 1 || code == 13 {
                let len = RX_BUF_LEN.saturating_sub(event.transfer_residue() as usize);
                added += self.parse_rx(&buffer, len);
            }
            let _ = self.post_rx(buffer);
        }
        added
    }

    /// Walk the DMA segments in a received buffer and harvest BSSIDs.
    fn parse_rx(&mut self, buffer: &Arc<DmaBuffer>, len: usize) -> usize {
        let len = len.min(RX_BUF_LEN);
        // SAFETY: buffer is exclusively owned; reading device-written bytes.
        let bytes = unsafe { core::slice::from_raw_parts(buffer.as_ptr(), len) };
        let mut off = 0;
        let mut added = 0;
        while off < bytes.len() {
            let Some(seg_len) = rx_logic::next_seg_len(&bytes[off..]) else {
                break;
            };
            if off + seg_len > bytes.len() {
                break;
            }
            let seg = &bytes[off..off + seg_len];
            if rx_logic::rxwi_zero_ok(seg)
                && let Some(frame) = rx_logic::segment_frame(seg)
                && let Some(mut bss) = rx_logic::frame_bssid(frame)
            {
                // Use the AP's REAL channel from the beacon's DS Parameter Set
                // IE, not the channel we were tuned to. A strong, close AP on
                // an overlapping channel is heard while tuned elsewhere; tagging
                // the tuned channel made us TX auth on the wrong channel and the
                // AP never heard us. Fall back to the tuned channel only when the
                // beacon carries no DS/HT channel IE.
                bss.channel = rx_logic::frame_channel(frame).unwrap_or(self.cur_channel);
                if let Some(ssid) = rx_logic::frame_ssid(frame) {
                    let n = ssid.len().min(bss.ssid.len());
                    bss.ssid[..n].copy_from_slice(&ssid[..n]);
                    bss.ssid_len = n as u8;
                }
                if self.record_bss(bss) {
                    added += 1;
                }
            }
            off += seg_len;
        }
        added
    }

    fn record_bss(&mut self, bss: rx_logic::Bss) -> bool {
        if self.bss.iter().any(|b| b.bssid == bss.bssid) {
            return false;
        }
        if self.bss.len() >= MAX_BSS {
            return false;
        }
        self.bss.push(bss);
        BSS_COUNT.store(self.bss.len(), Ordering::Relaxed);
        true
    }

    /// One non-destructive probe of host and device endpoint state. Reported
    /// success is not a causal/liveness classification; the only positive proof
    /// is a subsequent response-bearing channel command completing.
    fn probe_channel_transport(&mut self) -> bool {
        let enabled = endpoint_reset_enabled();
        if !enabled {
            crate::serial_println!(
                "mt7601u: CHANNEL-PROBE enabled=0 result=SKIP command_path_proven_usable=0"
            );
            return false;
        }

        let host_inband = self.claimed.recover_endpoint(&mut self.inband, enabled);
        let host_response = self.claimed.recover_endpoint(&mut self.cmd_resp, enabled);
        // Both rings were restarted. The old response TD no longer owns the DMA
        // buffer, and any already-routed completion belongs to the abandoned lap.
        self.cmd_resp_pending = None;
        let abandoned_inband_events = self.inband.completions().len();
        let abandoned_response_events = self.cmd_resp.completions().len();

        let device_inband = self.claimed.clear_endpoint_halt(self.inband.dci);
        let device_response = self.claimed.clear_endpoint_halt(self.cmd_resp.dci);
        let reported_ok = host_inband.recovered()
            && host_response.recovered()
            && device_inband.is_ok()
            && device_response.is_ok();
        crate::serial_println!(
            "mt7601u: CHANNEL-PROBE enabled=1 host_inband_state={} host_inband_requeued={} \
             host_response_state={} host_response_requeued={} device_inband_clear={} \
             device_response_clear={} abandoned_inband_events={} abandoned_response_events={} \
             reported_ok={} command_path_proven_usable=0 destructive_reload=0 com_reg0_write=0",
            host_inband.state_before,
            host_inband.recovered(),
            host_response.state_before,
            host_response.recovered(),
            device_inband.is_ok(),
            device_response.is_ok(),
            abandoned_inband_events,
            abandoned_response_events,
            reported_ok,
        );
        reported_ok
    }

    /// One idempotent no-op MCU command that requests a response.
    ///
    /// The payload is byte-identical to the post-firmware `Q_SELECT`
    /// function-select this device already accepted during bring-up, so
    /// re-issuing it cannot change device state: it re-selects the queue that is
    /// already selected. The only effects are that one response slot is retired
    /// and one `CMD_DONE` becomes observable — which is the whole mechanism
    /// behind both the drain and the proof.
    ///
    /// [`Self::mcu_function_select`] cannot be reused here because it derives
    /// `wait_resp` from the function id (`func == 5`, upstream parity) and both
    /// Q-B steps need the response.
    fn mcu_noop_response_command(&mut self) -> Result<(), &'static str> {
        let mut payload = [0u8; 8];
        payload[0..4].copy_from_slice(&regs::FUNC_Q_SELECT.to_le_bytes());
        payload[4..8].copy_from_slice(&POST_FW_Q_SELECT_VALUE.to_le_bytes());
        self.mcu_send(&payload, regs::CMD_FUN_SET_OP, true)
    }

    /// Q-B step 2 — retire accumulated MCU response slots with one idempotent
    /// no-op per hop completed since the path was last proven fresh.
    ///
    /// A stale slot surfaces as a sequence mismatch, which
    /// [`Self::mcu_wait_resp`] counts and re-arms past; that retirement is the
    /// drain. A no-op that fails is **not** fatal — this is best-effort state
    /// clearing, and nothing here authorizes a retry. Only the separate
    /// command-path proof does.
    ///
    /// `first_cmd_done_at` is the discriminator the next bare-metal run needs:
    /// it is the measured number of no-ops the MCU consumed before answering,
    /// so a ~12-deep response FIFO shows up as a number near the prior-hop
    /// count, and a genuinely dead command path shows up as zero.
    fn drain_mcu_command_queue(&mut self, channel: u8, prior_hops: u8) -> ChannelDrainOutcome {
        let depth = wedge_logic::mcu_drain_depth(prior_hops);
        let response_before = self.cmd_resp_stats;
        let saved = self.bulk_budget;
        self.bulk_budget = CHANNEL_DRAIN_BULK_BUDGET;

        let mut outcome = ChannelDrainOutcome {
            planned: depth,
            issued: 0,
            cmd_done: 0,
            first_cmd_done_at: 0,
        };
        for index in 0u8..depth {
            outcome.issued = outcome.issued.saturating_add(1);
            if self.mcu_noop_response_command().is_ok() {
                outcome.cmd_done = outcome.cmd_done.saturating_add(1);
                if outcome.first_cmd_done_at == 0 {
                    outcome.first_cmd_done_at = index.saturating_add(1);
                }
            }
        }

        self.bulk_budget = saved;
        crate::serial_println!(
            "mt7601u: MCU-DRAIN channel={} prior_hops={} planned={} issued={} cmd_done={} \
             first_cmd_done_at={} drain_bulk_budget={} resp_arms_delta={} resp_completions_delta={} \
             resp_mismatches_delta={} resp_unowned_delta={} resp_timeout_windows_delta={} \
             command_path_proven_usable=0 destructive_reload=0",
            channel,
            prior_hops,
            outcome.planned,
            outcome.issued,
            outcome.cmd_done,
            outcome.first_cmd_done_at,
            CHANNEL_DRAIN_BULK_BUDGET,
            self.cmd_resp_stats
                .arms
                .saturating_sub(response_before.arms),
            self.cmd_resp_stats
                .completions
                .saturating_sub(response_before.completions),
            self.cmd_resp_stats
                .mismatches
                .saturating_sub(response_before.mismatches),
            self.cmd_resp_stats
                .unowned_events
                .saturating_sub(response_before.unowned_events),
            self.cmd_resp_stats
                .timeout_windows
                .saturating_sub(response_before.timeout_windows),
        );
        outcome
    }

    /// Q-B step 3 — the only accepted proof that the MCU command path is usable
    /// again.
    ///
    /// [`Self::mcu_send`] returns `Ok` only after [`Self::mcu_wait_resp`]
    /// matched the awaited sequence number **and** decoded `EVT_CMD_DONE`, so
    /// `Ok` here is exactly "a `CMD_DONE` was observed". A completed transfer
    /// without that decode is not proof — which is the same mistake the
    /// endpoint probe made one layer up, and the reason `cea89e6a` re-issued
    /// RF17-20 into a latched MCU and hung.
    fn prove_command_path(&mut self, channel: u8) -> bool {
        let response_before = self.cmd_resp_stats;
        let saved = self.bulk_budget;
        self.bulk_budget = CHANNEL_PROBE_BULK_BUDGET;
        let result = self.mcu_noop_response_command();
        self.bulk_budget = saved;

        let proven = result.is_ok();
        if proven {
            // The path answered, so whatever had accumulated is retired: the
            // next drain starts from a freshly proven baseline.
            self.channel_hops_completed = 0;
        }
        crate::serial_println!(
            "mt7601u: MCU-PROOF channel={} command=FUN_SET_OP wait_response=1 result={} reason={} \
             probe_bulk_budget={} resp_completions_delta={} resp_mismatches_delta={} \
             resp_timeout_windows_delta={} command_path_proven_usable={} destructive_reload=0",
            channel,
            if proven { "CMD_DONE" } else { "NO_CMD_DONE" },
            result.err().unwrap_or("none"),
            CHANNEL_PROBE_BULK_BUDGET,
            self.cmd_resp_stats
                .completions
                .saturating_sub(response_before.completions),
            self.cmd_resp_stats
                .mismatches
                .saturating_sub(response_before.mismatches),
            self.cmd_resp_stats
                .timeout_windows
                .saturating_sub(response_before.timeout_windows),
            u8::from(proven),
        );
        proven
    }

    fn drain_channel_response_slot(&mut self, channel: u8, path: wedge_logic::TunePath) {
        let response_before = self.cmd_resp_stats;
        let mut drained_owned = 0u32;
        let mut drained_unowned = 0u32;
        let mut drained_invalid = 0u32;
        let mut last_seq = 0u8;
        let mut last_evt = 0u8;

        for _ in 0..CHANNEL_BETWEEN_HOP_DRAIN_POLLS {
            xhci::poll_events();
            for event in self.cmd_resp.completions() {
                let Some(event_index) = self.cmd_resp.index_of(event.parameter) else {
                    self.cmd_resp_stats.unowned_events =
                        self.cmd_resp_stats.unowned_events.saturating_add(1);
                    drained_unowned = drained_unowned.saturating_add(1);
                    continue;
                };
                let Some(pending) = self.cmd_resp_pending else {
                    self.cmd_resp_stats.unowned_events =
                        self.cmd_resp_stats.unowned_events.saturating_add(1);
                    drained_unowned = drained_unowned.saturating_add(1);
                    continue;
                };
                match mcu_logic::classify_response_transfer(
                    pending,
                    event_index,
                    event.completion_code(),
                    MCU_RESP_LEN,
                    event.transfer_residue(),
                ) {
                    mcu_logic::ResponseTransfer::Read { .. } => {
                        // SAFETY: the owned TD completed at least four bytes into
                        // the exclusively owned response buffer.
                        let rxfce = unsafe {
                            let p = self.resp.as_ptr();
                            u32::from_le_bytes([
                                core::ptr::read_volatile(p),
                                core::ptr::read_volatile(p.add(1)),
                                core::ptr::read_volatile(p.add(2)),
                                core::ptr::read_volatile(p.add(3)),
                            ])
                        };
                        let decoded = mcu_logic::decode_resp(rxfce);
                        last_seq = decoded.seq;
                        last_evt = decoded.evt_type;
                        self.cmd_resp_pending = None;
                        self.cmd_resp_stats.completions =
                            self.cmd_resp_stats.completions.saturating_add(1);
                        drained_owned = drained_owned.saturating_add(1);
                    }
                    mcu_logic::ResponseTransfer::Invalid => {
                        self.cmd_resp_pending = None;
                        drained_invalid = drained_invalid.saturating_add(1);
                    }
                    mcu_logic::ResponseTransfer::Unowned => {
                        self.cmd_resp_stats.unowned_events =
                            self.cmd_resp_stats.unowned_events.saturating_add(1);
                        drained_unowned = drained_unowned.saturating_add(1);
                    }
                }
            }
            let _ = self.ensure_mcu_response_armed();
            udelay(5);
        }

        crate::serial_println!(
            "mt7601u: CHANNEL-DRAIN channel={} path={:?} polls={} owned={} unowned={} invalid={} \
             last_seq={} last_evt={} resp_pending={} resp_arms_delta={} resp_completions_delta={} \
             resp_mismatches_delta={} resp_unowned_delta={} resp_timeout_windows_delta={} \
             command_path_proven_usable=0 destructive_reload=0",
            channel,
            path,
            CHANNEL_BETWEEN_HOP_DRAIN_POLLS,
            drained_owned,
            drained_unowned,
            drained_invalid,
            last_seq,
            last_evt,
            self.cmd_resp_pending.is_some(),
            self.cmd_resp_stats
                .arms
                .saturating_sub(response_before.arms),
            self.cmd_resp_stats
                .completions
                .saturating_sub(response_before.completions),
            self.cmd_resp_stats
                .mismatches
                .saturating_sub(response_before.mismatches),
            self.cmd_resp_stats
                .unowned_events
                .saturating_sub(response_before.unowned_events),
            self.cmd_resp_stats
                .timeout_windows
                .saturating_sub(response_before.timeout_windows),
        );
    }

    fn set_channel_accounted(
        &mut self,
        channel: u8,
        path: wedge_logic::TunePath,
        inline_cal_bw: bool,
    ) -> (Result<(), phy::ChannelError>, ChannelOperationAccount) {
        let fence_refusals_before = dma_arena::fence_refusals();
        let fce_upload_kicks_before = self.fce_upload_kicks;
        let response_before = self.cmd_resp_stats;
        let status_before = self.channel_status_snapshot();
        let operation = xhci::OperationBudgetGuard::new(CHANNEL_OPERATION_BUDGET);
        let saved = self.bulk_budget;
        let bulk_budget = SCAN_BULK_BUDGET;
        self.bulk_budget = bulk_budget;
        let mut result = phy::set_channel(self, channel, path, inline_cal_bw);
        let mut endpoint_probe_attempted = false;
        let mut endpoint_probe_reported_ok = false;
        let mut recovery = None;
        if let Err(first_error) = result {
            let observation = wedge_logic::ChannelFailureObservation {
                endpoint_probes_used: 0,
                operation_remaining: operation.snapshot().remaining,
                probe_and_retry_reserve: CHANNEL_PROBE_AND_RETRY_RESERVE,
            };
            if wedge_logic::channel_failure_action(observation)
                == wedge_logic::ChannelFailureAction::ProbeEndpointsAndRetry
            {
                // The Q-B non-destructive recovery sequence, in order and with
                // no destructive cold reload available at any point:
                //   1. endpoint recovery — transport evidence only,
                //   2. MCU queue drain — retire accumulated response slots,
                //   3. command-path proof — an observed CMD_DONE, the *only*
                //      thing that may authorize a retry.
                // `cea89e6a` hung because it re-issued the identical RF17-20
                // write after step 1 alone. Step 3 is what makes a retry safe;
                // step 2 is what gives step 3 a chance of passing.
                endpoint_probe_attempted = true;
                endpoint_probe_reported_ok = self.probe_channel_transport();
                let drain = self.drain_mcu_command_queue(channel, self.channel_hops_completed);
                let command_path_proven_usable = self.prove_command_path(channel);
                let retry_action =
                    wedge_logic::channel_retry_action(wedge_logic::recovery_retry_observation(
                        wedge_logic::ChannelRecoveryObservation {
                            endpoint_probe_reported_ok,
                            drain_commands_issued: drain.issued,
                            drain_cmd_done_observed: drain.cmd_done,
                            probe_command_completed: command_path_proven_usable,
                            probe_cmd_done_observed: command_path_proven_usable,
                        },
                    ));
                let retry_attempted = retry_action == wedge_logic::ChannelRetryAction::Retry;
                crate::serial_println!(
                    "mt7601u: CHANNEL-PROBE retry channel={} initial_phase={} reported_ok={} \
                     drain_issued={} drain_cmd_done={} drain_first_cmd_done_at={} \
                     command_path_proven_usable={} retry_action={:?} result={} \
                     destructive_reload=0",
                    channel,
                    first_error.phase,
                    endpoint_probe_reported_ok,
                    drain.issued,
                    drain.cmd_done,
                    drain.first_cmd_done_at,
                    u8::from(command_path_proven_usable),
                    retry_action,
                    if retry_attempted {
                        "RETRY"
                    } else {
                        "SKIP reason=command_path_not_reproven"
                    },
                );
                crate::log::flush();

                // The retry runs inside the same non-replenishing ledger, so it
                // is bounded by construction: it can only fail or exhaust, never
                // hang. The reserve check above guaranteed the ledger can fund
                // it after the recovery sequence's worst case.
                let mut retry_succeeded = false;
                if retry_attempted {
                    let retried = phy::set_channel(self, channel, path, inline_cal_bw);
                    retry_succeeded = retried.is_ok();
                    crate::serial_println!(
                        "mt7601u: CHANNEL-RETRY channel={} result={} reason={} \
                         operation_remaining={} destructive_reload=0",
                        channel,
                        if retry_succeeded { "OK" } else { "FAIL" },
                        retried.err().map_or("none", |error| error.reason),
                        operation.snapshot().remaining,
                    );
                    crate::log::flush();
                    // Preserve the original error on a failed retry: the first
                    // failure is the diagnostic one, and the terminal record is
                    // keyed to its phase.
                    if retry_succeeded {
                        result = retried;
                    }
                }
                recovery = Some(ChannelRecoveryAccount {
                    drain,
                    command_path_proven_usable,
                    retry_action,
                    retry_attempted,
                    retry_succeeded,
                });
            }
        }
        self.bulk_budget = saved;
        // One more hop's worth of MCU response slots may now be outstanding.
        // This is the Q-B drain depth for the *next* failure, so it counts every
        // completed tune on both the survey and targeted routes.
        if result.is_ok() {
            self.channel_hops_completed = self.channel_hops_completed.saturating_add(1);
        }
        let status_after = self.channel_status_snapshot();
        let budget = operation.snapshot();
        drop(operation);
        let account = ChannelOperationAccount {
            budget,
            bulk_budget,
            fence_refusals_before,
            fence_refusals_after: dma_arena::fence_refusals(),
            fce_upload_kicks_before,
            fce_upload_kicks_after: self.fce_upload_kicks,
            response_before,
            response_after: self.cmd_resp_stats,
            status_before,
            status_after,
            endpoint_probe_attempted,
            endpoint_probe_reported_ok,
            recovery,
        };
        if let Err(error) = &result {
            let expected = expected_rf_freqplan(channel);
            let prefix = account.status_after.rf_matched_prefix(expected);
            let first_mismatch = account.status_after.rf_first_mismatch_reg(expected);
            crate::serial_println!(
                "mt7601u: CHANNEL-STATUS channel={} path={:?} phase={} bulk_budget={} \
                 mcu_com0_before={:#010x} mcu_com0_after={:#010x} mcu_com1_before={:#010x} mcu_com1_after={:#010x} \
                 wpdma_before={:#010x} wpdma_after={:#010x} usb_dma_before={:#010x} usb_dma_after={:#010x} \
                 pbf_before={:#010x} pbf_after={:#010x} fce_pse_before={:#010x} fce_pse_after={:#010x} \
                 fce_idx_before={:#010x} fce_idx_after={:#010x} fce_global_before={:#010x} fce_global_after={:#010x} \
                 mac_status_before={:#010x} mac_status_after={:#010x} rf_csr_before={:#010x} rf_csr_after={:#010x} \
                 rf_bypass0_before={:#010x} rf_bypass0_after={:#010x} rf_setting0_before={:#010x} rf_setting0_after={:#010x} \
                 rf17_20_expected={:02x},{:02x},{:02x},{:02x} rf17_20_before={:02x},{:02x},{:02x},{:02x} \
                 rf17_20_after={:02x},{:02x},{:02x},{:02x} rf_matched_prefix={} rf_first_mismatch_reg={}",
                channel,
                path,
                error.phase,
                account.bulk_budget,
                account.status_before.mcu_com0,
                account.status_after.mcu_com0,
                account.status_before.mcu_com1,
                account.status_after.mcu_com1,
                account.status_before.wpdma_glo_cfg,
                account.status_after.wpdma_glo_cfg,
                account.status_before.usb_dma_cfg,
                account.status_after.usb_dma_cfg,
                account.status_before.pbf_cfg,
                account.status_after.pbf_cfg,
                account.status_before.fce_pse_ctrl,
                account.status_after.fce_pse_ctrl,
                account.status_before.fce_cpu_desc_idx,
                account.status_after.fce_cpu_desc_idx,
                account.status_before.fce_pdma_global_conf,
                account.status_after.fce_pdma_global_conf,
                account.status_before.mac_status,
                account.status_after.mac_status,
                account.status_before.rf_csr_cfg,
                account.status_after.rf_csr_cfg,
                account.status_before.rf_bypass0,
                account.status_after.rf_bypass0,
                account.status_before.rf_setting0,
                account.status_after.rf_setting0,
                expected[0],
                expected[1],
                expected[2],
                expected[3],
                account.status_before.rf17_20[0],
                account.status_before.rf17_20[1],
                account.status_before.rf17_20[2],
                account.status_before.rf17_20[3],
                account.status_after.rf17_20[0],
                account.status_after.rf17_20[1],
                account.status_after.rf17_20[2],
                account.status_after.rf17_20[3],
                prefix,
                first_mismatch,
            );
        }
        crate::serial_println!(
            "mt7601u: CHANNEL-OP channel={} path={:?} result={} phase={} bulk_budget={} consumed={} remaining={} exhausted={} \
             resp_pending={} resp_arms_delta={} resp_completions_delta={} resp_mismatches_delta={} \
             resp_unowned_delta={} resp_timeout_windows_delta={} fce_upload_kicks_before={} fce_upload_kicks_after={} \
             endpoint_probe_attempted={} endpoint_probe_reported_ok={} \
             tx_dma={:#x} resp_dma={:#x} dma_distinct={} fence_refusals_before={} fence_refusals_after={}",
            channel,
            path,
            if result.is_ok() { "OK" } else { "FAIL" },
            result.as_ref().err().map_or("none", |error| error.phase),
            account.bulk_budget,
            budget.limit.saturating_sub(budget.remaining),
            budget.remaining,
            budget.exhausted,
            self.cmd_resp_pending.is_some(),
            account
                .response_after
                .arms
                .saturating_sub(account.response_before.arms),
            account
                .response_after
                .completions
                .saturating_sub(account.response_before.completions),
            account
                .response_after
                .mismatches
                .saturating_sub(account.response_before.mismatches),
            account
                .response_after
                .unowned_events
                .saturating_sub(account.response_before.unowned_events),
            account
                .response_after
                .timeout_windows
                .saturating_sub(account.response_before.timeout_windows),
            account.fce_upload_kicks_before,
            account.fce_upload_kicks_after,
            account.endpoint_probe_attempted,
            account.endpoint_probe_reported_ok,
            self.staging.physical_addr().as_u64(),
            self.resp.physical_addr().as_u64(),
            self.staging.physical_addr() != self.resp.physical_addr(),
            account.fence_refusals_before,
            account.fence_refusals_after,
        );
        (result, account)
    }

    /// Permanently classify a failed channel-switch MCU transaction for this
    /// device lifetime. TEST 3 observed host/device endpoint repair followed by
    /// a forced reload that cleared residency and made zero upload progress.
    /// That rejects this specific destructive recovery; it does not distinguish
    /// transport desynchronization from MCU failure.
    fn terminalize_channel_failure(
        &mut self,
        channel: u8,
        error: phy::ChannelError,
        account: ChannelOperationAccount,
    ) -> &'static str {
        let disposition = wedge_logic::channel_fault_disposition();
        self.channel_command_path_unusable =
            disposition.command_path == wedge_logic::ChannelCommandPathState::Unusable;
        let budget = account.budget;
        let operation_fence_refusals = account
            .fence_refusals_after
            .saturating_sub(account.fence_refusals_before);
        let operation_evidence = wedge_logic::channel_operation_evidence(
            budget.limit,
            budget.remaining,
            budget.exhausted,
            operation_fence_refusals,
        );
        CHANNEL_TERMINAL_CHANNEL.store(channel as u32, Ordering::Relaxed);
        CHANNEL_OPERATION_CONSUMED.store(operation_evidence.consumed, Ordering::Relaxed);
        CHANNEL_OPERATION_REMAINING.store(operation_evidence.remaining, Ordering::Relaxed);
        CHANNEL_OPERATION_EXHAUSTED.store(operation_evidence.exhausted, Ordering::Relaxed);
        CHANNEL_TERMINAL_FENCE_REFUSALS.store(operation_evidence.fence_refusals, Ordering::Relaxed);
        if CHANNEL_TERMINAL
            .compare_exchange(0, 1, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            return "channel command path unusable";
        }
        let expected = expected_rf_freqplan(channel);
        let rf_prefix = account.status_after.rf_matched_prefix(expected);
        let rf_first_mismatch = account.status_after.rf_first_mismatch_reg(expected);
        crate::serial_println!(
            "mt7601u: TERMINAL category=CHANNEL_COMMAND_PATH_UNUSABLE channel={} phase={} reason={} \
             command_path=unusable root_cause=undistinguished destructive_reload={} continue_survey={} association={} dhcp={} \
             link_up={} bulk_budget={} channel_op_consumed={} channel_op_remaining={} channel_op_exhausted={} \
             channel_op_wait_calls={} channel_op_wait_iters={} channel_op_bulk_attempts={} \
             channel_op_bulk_polls={} resp_pending={} resp_arms={} resp_completions={} \
             resp_mismatches={} resp_unowned_events={} resp_timeout_windows={} \
             mcu_com0={:#010x} mcu_com1={:#010x} wpdma={:#010x} usb_dma={:#010x} \
             pbf={:#010x} fce_pse={:#010x} fce_idx={:#010x} fce_global={:#010x} \
             mac_status={:#010x} rf_csr={:#010x} rf_bypass0={:#010x} rf_setting0={:#010x} \
             rf17_20_expected={:02x},{:02x},{:02x},{:02x} rf17_20_after={:02x},{:02x},{:02x},{:02x} \
             rf_matched_prefix={} rf_first_mismatch_reg={} \
             fce_upload_kicks_before={} fce_upload_kicks_after={} endpoint_probe_attempted={} \
             endpoint_probe_reported_ok={} legacy_endpoint_reset_enabled={} \
             blocked_commands_after_terminal={} fence_state={} channel_fence_refusals={} fence_refusals_total={}",
            channel,
            error.phase,
            error.reason,
            disposition.attempt_destructive_reload,
            disposition.continue_survey,
            disposition.allow_association,
            disposition.allow_dhcp,
            disposition.allow_link_up,
            account.bulk_budget,
            operation_evidence.consumed,
            operation_evidence.remaining,
            operation_evidence.exhausted,
            budget.counters.wait_calls,
            budget.counters.wait_iterations,
            budget.counters.bulk_attempts,
            budget.counters.bulk_polls,
            self.cmd_resp_pending.is_some(),
            self.cmd_resp_stats.arms,
            self.cmd_resp_stats.completions,
            self.cmd_resp_stats.mismatches,
            self.cmd_resp_stats.unowned_events,
            self.cmd_resp_stats.timeout_windows,
            account.status_after.mcu_com0,
            account.status_after.mcu_com1,
            account.status_after.wpdma_glo_cfg,
            account.status_after.usb_dma_cfg,
            account.status_after.pbf_cfg,
            account.status_after.fce_pse_ctrl,
            account.status_after.fce_cpu_desc_idx,
            account.status_after.fce_pdma_global_conf,
            account.status_after.mac_status,
            account.status_after.rf_csr_cfg,
            account.status_after.rf_bypass0,
            account.status_after.rf_setting0,
            expected[0],
            expected[1],
            expected[2],
            expected[3],
            account.status_after.rf17_20[0],
            account.status_after.rf17_20[1],
            account.status_after.rf17_20[2],
            account.status_after.rf17_20[3],
            rf_prefix,
            rf_first_mismatch,
            account.fce_upload_kicks_before,
            account.fce_upload_kicks_after,
            account.endpoint_probe_attempted,
            account.endpoint_probe_reported_ok,
            endpoint_reset_enabled(),
            CHANNEL_COMMANDS_BLOCKED_AFTER_TERMINAL.load(Ordering::Relaxed),
            if operation_evidence.fence_refusals == 0 {
                "clean"
            } else {
                "refused"
            },
            operation_evidence.fence_refusals,
            account.fence_refusals_after,
        );
        // The Q-B recovery record, emitted alongside the terminal so a failed
        // recovery is diagnosable from the serial log alone. `drain_cmd_done=0`
        // with `drain_issued>0` says the command path never answered at all;
        // a non-zero `drain_first_cmd_done_at` says it answered only after that
        // many slots were retired, which is the response-FIFO depth measurement.
        match account.recovery {
            Some(recovery) => {
                crate::serial_println!(
                    "mt7601u: CHANNEL-RECOVERY channel={} sequence=endpoint,drain,proof \
                 drain_planned={} drain_issued={} drain_cmd_done={} drain_first_cmd_done_at={} \
                 command_path_proven_usable={} retry_action={:?} retry_attempted={} \
                 retry_succeeded={} destructive_reload=0",
                    channel,
                    recovery.drain.planned,
                    recovery.drain.issued,
                    recovery.drain.cmd_done,
                    recovery.drain.first_cmd_done_at,
                    u8::from(recovery.command_path_proven_usable),
                    recovery.retry_action,
                    recovery.retry_attempted,
                    recovery.retry_succeeded,
                );
            }
            None => {
                crate::serial_println!(
                    "mt7601u: CHANNEL-RECOVERY channel={} sequence=none reason=reserve_unavailable \
                     command_path_proven_usable=0 destructive_reload=0",
                    channel,
                );
            }
        }
        crate::log::flush();
        "channel command path unusable"
    }

    /// The scan: reuse the Linux-ordered packet-RX pool, hop channels, and
    /// collect beacons. Returns the number of distinct BSSIDs found.
    fn scan(&mut self) -> Result<usize, &'static str> {
        let posted = self.ensure_packet_rx_ready()?;
        crate::serial_println!(
            "mt7601u: PACKET-RX scan reuse posted={}/{} double_post=0",
            posted,
            RX_BUFFERS
        );
        crate::log::flush();
        // Hop the 2.4 GHz channels, dwelling on each long enough to catch a
        // beacon. A per-channel breadcrumb reports the running BSS count so the
        // Stage-B log shows WHICH channels yielded APs (vs a silent radio).
        //
        // ISC-0 (F1-part-2): each `set_channel` runs under the lowered
        // `SCAN_BULK_BUDGET`, so a missing channel response terminates under a
        // bounded operation ledger instead of burning the full bulk budget per
        // sub-operation. The persistent pre-armed response TD prevents the
        // clean-run response-loss/desynchronization path; the terminal below is
        // containment if a timeout still occurs, not acceptance success.
        // Capture the configured/runtime association target ONCE (the
        // `configured_target` path runs PBKDF2 to derive the PMK, so we must not
        // call it per-channel). `None` ⇒ no target ⇒ full survey hop below
        // (default / W1 scan scenario), kill-switch preserved bit-for-bit.
        let assoc_target = TARGET.lock().clone().or_else(configured_target);

        // ── Linux-faithful targeted connect (PL directive: "make it work like Ubuntu
        // — connect to ch11"). A station joining a KNOWN AP parks on that AP's channel;
        // it does NOT rapid-hop all 14. When a channel is explicitly configured,
        // tune once and dwell. Shipping proof remains CHANNEL=0 and therefore
        // takes the genuine survey below. Any tune failure is terminalized
        // without reset or destructive reload.
        if let Some(t) = assoc_target.as_ref()
            && wedge_logic::scan_tune_path(t.channel) == wedge_logic::TunePath::Targeted
        {
            let ch = t.channel;
            let inline_cal = wedge_logic::inline_cal_bw(
                wedge_logic::TunePath::Targeted,
                self.successful_targeted_tunes,
            );
            let bulk_budget = SCAN_BULK_BUDGET;
            TARGET_TUNE_STATE.store(target_tune_state::BEFORE, Ordering::Relaxed);
            crate::serial_println!(
                "mt7601u: targeted scan — tuning directly to ch={} cal_bw={} budget={} (explicit configured-channel path; CHANNEL=0 still performs the full survey)",
                ch,
                inline_cal,
                bulk_budget,
            );
            crate::log::flush();
            let (tuned, account) =
                self.set_channel_accounted(ch, wedge_logic::TunePath::Targeted, inline_cal);
            if let Err(reason) = tuned {
                TARGET_TUNE_STATE.store(target_tune_state::FAILED, Ordering::Relaxed);
                return Err(self.terminalize_channel_failure(ch, reason, account));
            }
            self.successful_targeted_tunes =
                wedge_logic::note_targeted_tune_success(self.successful_targeted_tunes);
            self.deferred_cal_pending = wedge_logic::deferred_cal_pending_after_tune(inline_cal);
            TARGET_TUNE_STATE.store(target_tune_state::TUNED_OK, Ordering::Relaxed);
            self.cur_channel = ch;
            let before = self.bss.len();
            for _ in 0..SCAN_DWELL_PASSES {
                xhci::poll_events();
                self.drain_rx();
                udelay(SCAN_DWELL_STEP_US);
            }
            crate::serial_println!(
                "mt7601u: targeted scan ch={} dwell done — bss total={} (+{} on this channel)",
                ch,
                self.bss.len(),
                self.bss.len() - before,
            );
            self.drain_channel_response_slot(ch, wedge_logic::TunePath::Targeted);
            return Ok(self.bss.len());
        }

        let mut channels_completed = 0u8;
        for ch in self.ee.reg_start..self.ee.reg_start + self.ee.reg_num {
            let inline_cal = wedge_logic::inline_cal_bw(
                wedge_logic::TunePath::Survey,
                self.successful_targeted_tunes,
            );
            let bulk_budget = SCAN_BULK_BUDGET;
            crate::serial_println!(
                "mt7601u: survey hop ch={} cal_bw={} begin budget={}",
                ch,
                inline_cal,
                bulk_budget,
            );
            crate::log::flush();
            let (tuned, account) =
                self.set_channel_accounted(ch, wedge_logic::TunePath::Survey, inline_cal);
            if let Err(reason) = tuned {
                return Err(self.terminalize_channel_failure(ch, reason, account));
            }
            self.deferred_cal_pending = wedge_logic::deferred_cal_pending_after_tune(inline_cal);
            self.cur_channel = ch; // tag BSSes heard during this dwell
            let before = self.bss.len();
            for _ in 0..SCAN_DWELL_PASSES {
                xhci::poll_events();
                self.drain_rx();
                udelay(SCAN_DWELL_STEP_US);
            }
            crate::serial_println!(
                "mt7601u: survey hop ch={} cal_bw={} dwell done — bss total={} (+{} this channel)",
                ch,
                inline_cal,
                self.bss.len(),
                self.bss.len() - before,
            );
            self.drain_channel_response_slot(ch, wedge_logic::TunePath::Survey);
            channels_completed = channels_completed.saturating_add(1);
            // No early-out: scan ALL channels, with upstream-parity inline CAL_BW
            // on each hop, so we collect EVERY BSS broadcasting the target SSID — a
            // mesh has several (e.g. a strong ch6 node that steers 2.4 GHz clients away
            // and a ch11 node that accepts them). `maybe_associate` then fails over
            // across them, exactly like a roaming client, instead of fixating on the
            // first/strongest match. (`assoc_target` is still captured above only to
            // detect the assoc-vs-W1-scan case; the full hop is what both now run.)
            let _ = &assoc_target;
        }
        crate::serial_println!(
            "mt7601u: scan summary — route=genuine_survey channels_completed={} expected_channels={} survey_cal_bw=inline response_path=persistent_prearmed terminal=none",
            channels_completed,
            self.ee.reg_num,
        );
        Ok(self.bss.len())
    }

    // ── W2 ISC-1: the in-kernel supplicant (mgmt TX/RX + the auth→assoc loop) ──

    /// Wrap `frame` (prepend TXWI + the DMA TXINFO) into `tx_staging`, fence,
    /// submit on the data-TX pipe, and poll to completion under `bulk_budget`.
    /// Builds directly into the DMA buffer (the `load_firmware`/`build_chunk`
    /// idiom — no large stack copy, so data frames up to the MTU fit). `wcid` +
    /// `flags` distinguish mgmt (MGMT_WCID, WIV/no-key) from data (STA_WCID, the
    /// WCID key → HW CCMP).
    fn tx_raw(&mut self, frame: &[u8], wcid: u8, flags: u32) -> Result<(), &'static str> {
        let n = {
            // SAFETY: tx_staging is an exclusively owned arena buffer;
            // build_tx_frame bounds-checks against the slice length.
            let dst = unsafe {
                core::slice::from_raw_parts_mut(self.tx_staging.as_mut_ptr(), self.tx_staging.len())
            };
            // pkt_id=1: tag the TXWI so the HW reports this frame's result in
            // TX_STAT_FIFO (the TX-status discriminator read below).
            tx_logic::build_tx_frame(dst, frame, wcid, 0, true, 1, flags).ok_or("tx frame build")?
        };
        // One-shot: dump the EXACT wrapped on-air bytes ([TXINFO 4][TXWI 20][frame])
        // so the frame can be byte-verified against the 802.11/TXWI spec — the last
        // unknown in the power-independent no-ACK failure (directed_to_us=0).
        if !TX_DUMPED.swap(true, Ordering::Relaxed) {
            // SAFETY: tx_staging is exclusively owned and was just populated above.
            let bytes = unsafe { core::slice::from_raw_parts(self.tx_staging.as_ptr(), n) };
            crate::serial_println!(
                "mt7601u: TXDUMP n={} wcid={} flags={:#010x} [TXINFO|TXWI20|frame]={:02x?}",
                n,
                wcid,
                flags,
                bytes,
            );
            crate::log::flush();
        }
        let addr = dma_arena::fence(self.tx_staging.physical_addr().as_u64(), n as u64)
            .map_err(|_| "tx staging outside the fence (I-14c1)")?;
        let idx = self.data_tx.submit(addr, n as u32);
        for _ in 0..self.bulk_budget {
            xhci::poll_events();
            for event in self.data_tx.completions() {
                if self.data_tx.index_of(event.parameter) == Some(idx) {
                    let code = event.completion_code();
                    if code == 1 || code == 13 {
                        // TX-status discriminator: the bulk-out reached the chip's TX
                        // queue — now poll TX_STAT_FIFO for the HW's per-frame result
                        // (TXWI pkt_id=1). VALID+SUCCESS ⇒ frame went out AND the peer
                        // ACKed it (the AP HEARS us — directed_to_us=0 is then an
                        // RX-of-response bug); VALID+!SUCCESS ⇒ TX'd, no ACK (RF/AP);
                        // never VALID ⇒ frame never left the radio (TX-path/MAC bug,
                        // the warm-reboot suspect).
                        let mut ts = 0u32;
                        for _ in 0..32 {
                            let v = self.rr(regs::TX_STAT_FIFO);
                            if v & regs::TX_STAT_FIFO_VALID != 0 {
                                ts = v;
                                break;
                            }
                        }
                        crate::serial_println!(
                            "mt7601u: tx_stat raw={:#010x} valid={} success={} ackreq={}",
                            ts,
                            ts & regs::TX_STAT_FIFO_VALID != 0,
                            ts & regs::TX_STAT_FIFO_SUCCESS != 0,
                            ts & regs::TX_STAT_FIFO_ACKREQ != 0,
                        );
                        return Ok(());
                    }
                    return Err("data-tx completion error");
                }
            }
            udelay(5);
        }
        Err("data-tx timeout")
    }

    /// Transmit one raw 802.11 management frame (auth/assoc) — unencrypted,
    /// WIV-flagged (no key). Sent on the AP's unicast WCID (`STA_WCID`, programmed
    /// with the AP address by `wcid_setup` in `associate`), NOT the broadcast mcast
    /// slot — the chip needs a real unicast context for the AP to receive/ACK us.
    /// WIV keeps it unencrypted even though `STA_WCID` later carries the CCMP key.
    fn tx_frame(&mut self, frame: &[u8]) -> Result<(), &'static str> {
        self.tx_raw(frame, STA_WCID, tx_logic::mgmt_tx_flags())
    }

    /// Transmit one ethernet payload as an encrypted 802.11 DATA frame (ISC-4,
    /// post-association): build the To-DS data frame (HW CCMP via the installed
    /// pairwise key) and send it on the data pipe. `da` is the ethernet dest.
    fn data_tx(
        &mut self,
        da: &[u8; 6],
        ethertype: u16,
        payload: &[u8],
    ) -> Result<(), &'static str> {
        let sta = *READBACK_MAC.lock();
        let bssid = *ASSOC_BSSID.lock();
        let mut frame = [0u8; 1600]; // 802.11 data frame (hdr + SNAP + et + payload)
        let n = mlme_logic::build_data_frame(&mut frame, &sta, &bssid, da, ethertype, payload)
            .ok_or("data frame build")?;
        self.tx_raw(&frame[..n], STA_WCID, tx_logic::data_tx_flags())
    }

    /// Drain the RX pipe for one received DATA frame and write the reconstructed
    /// ethernet frame (`[da=our MAC][sa=addr3][EtherType][payload]`) into `out`,
    /// returning its length. The HW has already CCMP-decrypted. `None` if no data
    /// frame is ready. Re-posts each consumed RX buffer.
    fn data_rx(&mut self, out: &mut [u8]) -> Option<usize> {
        let mac = *READBACK_MAC.lock();
        let mut result = None;
        for event in self.pkt_rx.completions() {
            let Some(idx) = self.pkt_rx.index_of(event.parameter) else {
                continue;
            };
            let Some(buffer) = self.rx_table[idx].take() else {
                continue;
            };
            let code = event.completion_code();
            if (code == 1 || code == 13) && result.is_none() {
                let len = RX_BUF_LEN.saturating_sub(event.transfer_residue() as usize);
                result = build_eth_from_rx(&buffer, len, &mac, out);
            }
            let _ = self.post_rx(buffer);
        }
        result
    }

    /// Drain the packet-RX pipe and return the first management frame of interest
    /// (auth / assoc-resp / deauth) to the supplicant. Re-posts each consumed RX
    /// buffer (same cadence as `drain_rx`).
    fn drain_mgmt(&mut self) -> Option<RxMgmt> {
        let our_mac = *READBACK_MAC.lock();
        let mut found = None;
        for event in self.pkt_rx.completions() {
            let Some(idx) = self.pkt_rx.index_of(event.parameter) else {
                continue;
            };
            let Some(buffer) = self.rx_table[idx].take() else {
                continue;
            };
            let code = event.completion_code();
            if code == 1 || code == 13 {
                RX_MGMT_FRAMES.fetch_add(1, Ordering::Relaxed);
                let len = RX_BUF_LEN.saturating_sub(event.transfer_residue() as usize);
                let m = parse_rx_mgmt(&buffer, len, &our_mac);
                if found.is_none() {
                    found = m;
                }
            }
            let _ = self.post_rx(buffer);
        }
        found
    }

    /// Poll for a mgmt response up to `spins` iterations (×5µs). `None` on timeout.
    fn poll_mgmt(&mut self, spins: u32) -> Option<RxMgmt> {
        for _ in 0..spins {
            xhci::poll_events();
            if let Some(m) = self.drain_mgmt() {
                return Some(m);
            }
            udelay(5);
        }
        None
    }

    /// Install a key into the hardware WCID key table (mac.c
    /// `mt76_mac_wcid_set_key`): write the 32-byte key material, the IV/PN seed,
    /// and the cipher-mode/pairwise ATTR word. After this the firmware does CCMP
    /// on the data path (ISC-3). `key` ≤ 32 bytes (a CCMP TK is 16, a GTK 16/32);
    /// the register block is written as consecutive 32-bit words (the driver's
    /// own `addr_wr` uses individual `wr` for register blocks; `wr_copy` is just
    /// the burst optimization of the same writes). Called by the 4-way M3 step.
    fn wcid_set_key(
        &mut self,
        wcid: u8,
        key: &[u8],
        cipher: u8,
        pairwise: bool,
        keyidx: u8,
    ) -> Result<(), &'static str> {
        if key.len() > 32 {
            return Err("wcid key too long");
        }
        let mut key_data = [0u8; 32];
        key_data[..key.len()].copy_from_slice(key);
        let base = regs::wcid_key(wcid as u32);
        for (i, chunk) in key_data.chunks_exact(4).enumerate() {
            let w = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            self.wr(base + (i as u32) * 4, w)?;
        }
        let iv = key_logic::build_wcid_iv(keyidx, cipher);
        let iv_base = regs::wcid_iv(wcid as u32);
        for (i, chunk) in iv.chunks_exact(4).enumerate() {
            let w = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            self.wr(iv_base + (i as u32) * 4, w)?;
        }
        let prev = self.rr(regs::wcid_attr(wcid as u32));
        let val = key_logic::wcid_attr_word(prev, cipher, pairwise);
        self.wr(regs::wcid_attr(wcid as u32), val)?;
        Ok(())
    }

    /// mac.c `mt7601u_mac_wcid_setup`: bind a WCID slot to a peer MAC so the chip
    /// transmits and ACK-tracks unicast frames on it with the correct address.
    /// MUST run BEFORE the first frame is sent on `idx` — otherwise the slot's
    /// init address is broadcast (`ff:ff:ff:ff:ff:ff`, init.c `init_wcid_mem`), and
    /// a unicast frame tagged with it gets the chip's broadcast/no-ACK context. That
    /// is the W2 bug: auth TX'd with `MGMT_WCID=0` (the mcast slot) reported
    /// `valid=true` but `success=false` / `directed_to_us=0`, power-independent —
    /// the AP never cleanly received our auth. Upstream sends client mgmt/data on
    /// the AP's real WCID (`mt7601u_tx`: `sta ⇒ msta->wcid`), set up here. ATTR
    /// carries only BSS_IDX (vif 0); the pairwise/cipher bits are added later by
    /// `wcid_set_key` when the CCMP key is installed.
    fn wcid_setup(&mut self, idx: u8, mac: &[u8; 6]) -> Result<(), &'static str> {
        // ATTR(idx): BSS_IDX = 0 (single vif). Cleared key bits are re-OR'd in by
        // `wcid_set_key` at the 4-way M3 step.
        self.wr(regs::wcid_attr(idx as u32), 0)?;
        // ADDR(idx) = WCID_ADDR_BASE + idx*8: DW0 = mac[0..4], DW1 = mac[4..6].
        let base = regs::WCID_ADDR_BASE + (idx as u32) * 8;
        self.wr(base, u32::from_le_bytes([mac[0], mac[1], mac[2], mac[3]]))?;
        self.wr(base + 4, (mac[4] as u32) | ((mac[5] as u32) << 8))?;
        // Read back so the boot self-confirms the AP address landed in the slot
        // (rb should echo the AP MAC; broadcast `ffffffff/0000ffff` ⇒ write failed).
        let rb0 = self.rr(base);
        let rb1 = self.rr(base + 4);
        crate::serial_println!(
            "mt7601u: wcid_setup idx={} ap={:02x?} WCID_ADDR rb0={:#010x} rb1={:#010x}",
            idx,
            mac,
            rb0,
            rb1,
        );
        crate::log::flush();
        Ok(())
    }

    /// The supplicant loop: tune to the target, Open-System authenticate, then
    /// associate — driving `mlme_logic`'s state machine over the mgmt TX/RX.
    /// Returns the terminal MLME state (`Connected` for an open AP; `Handshaking`
    /// for WPA2/3 → ISC-2). Breadcrumbs each stage (DISPATCH §4).
    fn associate(&mut self, target: &AssocTarget) -> Result<mlme_logic::MlmeState, &'static str> {
        use mlme_logic::{MlmeEvent, MlmeState, mlme_next};
        let sta = *READBACK_MAC.lock();
        let ap = target.bssid;
        // CB-5a ISC-5: the WPA3-SAE connect path. The orchestrator
        // (`net::wifi::sae_connect`) is gate-proven hermetically via the ISC-5
        // MockAp scenario; the thin abstract→real-mt7601u mgmt/EAPOL adapter that
        // lets it drive THIS driver on silicon is CB-5b's deliverable. So when the
        // runtime kill-switch is engaged we refuse the SAE target explicitly (naming
        // the CB-5b adapter) rather than silently negotiating WPA2 with a WPA3-only
        // AP. The switch defaults OFF, so a stock `wpa3-sae` kernel never takes this
        // branch and the connect path is byte-identical to CB-2/WPA2 (the ISC-5
        // "off ≡ CB-2" equality). This hook adds nothing to the WPA2 path.
        #[cfg(feature = "wpa3-sae")]
        if crate::net::wifi::sae_connect::sae_enabled() {
            crate::serial_println!(
                "mt7601u: wpa3-sae requested — SAE orchestrator is hermetically proven; \
                 the on-air mt7601u adapter is CB-5b"
            );
            crate::log::flush();
            return Err("wpa3_sae_onair_adapter_is_cb5b");
        }
        // The scan's targeted-join early-out left us tuned to the target's
        // channel — that is HOW we heard its beacon (cur_channel == the channel
        // the BSS was learned on == target.channel). Re-tuning to the channel
        // we are already on is redundant command traffic. Only tune when a
        // runtime target names a different channel, under the same accounted
        // response-ownership path used by survey hops.
        if self.cur_channel != target.channel {
            let inline_cal = wedge_logic::inline_cal_bw(
                wedge_logic::TunePath::Targeted,
                self.successful_targeted_tunes,
            );
            TARGET_TUNE_STATE.store(target_tune_state::BEFORE, Ordering::Relaxed);
            crate::serial_println!(
                "mt7601u: assoc set_channel({}) (cur={}) cal_bw={}",
                target.channel,
                self.cur_channel,
                inline_cal,
            );
            crate::log::flush();
            let (tuned, account) = self.set_channel_accounted(
                target.channel,
                wedge_logic::TunePath::Targeted,
                inline_cal,
            );
            if let Err(e) = tuned {
                TARGET_TUNE_STATE.store(target_tune_state::FAILED, Ordering::Relaxed);
                return Err(self.terminalize_channel_failure(target.channel, e, account));
            }
            self.successful_targeted_tunes =
                wedge_logic::note_targeted_tune_success(self.successful_targeted_tunes);
            self.deferred_cal_pending = wedge_logic::deferred_cal_pending_after_tune(inline_cal);
            TARGET_TUNE_STATE.store(target_tune_state::TUNED_OK, Ordering::Relaxed);
            self.cur_channel = target.channel;
        } else {
            // Already on the target channel. The PLL may be locked only tightly enough
            // for RX (we hear beacons fine) but not for TX. Re-lock JUST the VCO/PLL via
            // vco_cal — RF-register writes through MT_RF_CSR_CFG only, with no
            // MCU command-response transaction — giving TX a fresh lock without
            // a redundant channel re-tune.
            crate::serial_println!(
                "mt7601u: assoc — already tuned to ch={}; re-locking PLL (vco_cal, RF-only, no MCU)",
                target.channel
            );
            crate::log::flush();
            phy::vco_cal(self)?;
        }
        // Channel-triple discriminator (THINK §3) — mandatory on every W2 bring-up
        // run, regardless of outcome. beacon-DS (target.channel, from the beacon's
        // DS Parameter Set IE) vs tuned (cur_channel) vs tx (RF synth readback).
        // Any divergence = wrong-channel TX, conclusively localized from this one line.
        phy::log_channel_triple(self, target.channel, self.cur_channel);

        // Program the joined BSSID into the HW (main.c `mt7601u_bss_info_changed`
        // → MT_MAC_BSSID_DW0/DW1) so the chip filters/ACKs frames for this BSS;
        // without it the AP's directed responses are not accepted.
        self.wr(
            regs::MAC_BSSID_DW0,
            u32::from_le_bytes([ap[0], ap[1], ap[2], ap[3]]),
        )
        .map_err(|_| "assoc: set bssid dw0")?;
        self.wr(regs::MAC_BSSID_DW1, (ap[4] as u32) | ((ap[5] as u32) << 8))
            .map_err(|_| "assoc: set bssid dw1")?;
        // Bind STA_WCID to the AP's address BEFORE the first auth TX, so the auth/
        // assoc (and later data) frames transmit with a real unicast context instead
        // of the broadcast mcast slot (WCID 0). Without this the chip never cleanly
        // sends our unicast auth and the AP stays silent (directed_to_us=0). Upstream:
        // mt7601u_sta_add → mt7601u_mac_wcid_setup(idx, vif, AP) before any TX.
        self.wcid_setup(STA_WCID, &ap)
            .map_err(|_| "assoc: wcid setup")?;
        // Reset the RX-frame discriminator: drain_mgmt counts every delivered RX
        // frame (beacons included) into this counter so a failed auth wait
        // self-localizes — 0 ⇒ RX path dead / AP silent; >0 with no Auth ⇒ the
        // directed response is arriving-but-dropped or the AP isn't answering.
        RX_MGMT_FRAMES.store(0, Ordering::Relaxed);
        RX_DIRECTED_COUNT.store(0, Ordering::Relaxed);
        RX_DIRECTED_FC.store(0, Ordering::Relaxed);
        crate::serial_println!(
            "mt7601u: associating to {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} ch={} (open={})",
            ap[0],
            ap[1],
            ap[2],
            ap[3],
            ap[4],
            ap[5],
            target.channel,
            target.open
        );
        crate::log::flush();

        // --- Authenticate (Open System, transaction 1) ---
        let mut state = MlmeState::Authenticating;
        for _try in 0..mlme_logic::AUTH_MAX_TRIES {
            ASSOC_STAGE.store(assoc_stage::AUTH_SENT, Ordering::Relaxed);
            let mut frame = [0u8; 64];
            let n = mlme_logic::build_open_auth(&mut frame, &sta, &ap).ok_or("build auth")?;
            self.tx_frame(&frame[..n])?;
            match self.poll_mgmt(AUTH_RESP_SPINS) {
                Some(RxMgmt::Auth(a)) => {
                    LAST_STATUS.store(a.status as u32, Ordering::Relaxed);
                    state = mlme_next(state, MlmeEvent::AuthResp(a.status), target.open);
                    break;
                }
                Some(RxMgmt::Deauth) => return Err("assoc: deauth during auth"),
                _ => {} // timeout or stray frame → retry
            }
        }
        if state != MlmeState::Associating {
            let directed_fc = RX_DIRECTED_FC.load(Ordering::Relaxed);
            crate::serial_println!(
                "mt7601u: auth incomplete (state={:?}) — rx_frames={} directed_to_us={} first_directed_fc={:#06x} (directed=0 ⇒ AP never hears us/TX-side; directed>0 fc=0xb0 ⇒ auth resp arriving-but-dropped; fc=0xc0 ⇒ AP deauthing us)",
                state,
                RX_MGMT_FRAMES.load(Ordering::Relaxed),
                RX_DIRECTED_COUNT.load(Ordering::Relaxed),
                directed_fc & 0xffff,
            );
            crate::log::flush();
            return Err("assoc: auth did not complete");
        }
        ASSOC_STAGE.store(assoc_stage::AUTHED, Ordering::Relaxed);
        crate::serial_println!(
            "mt7601u: authenticated to {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            ap[0],
            ap[1],
            ap[2],
            ap[3],
            ap[4],
            ap[5]
        );
        crate::log::flush();

        // --- Associate ---
        let rates: [u8; 4] = [0x82, 0x84, 0x8b, 0x96]; // 1/2/5.5/11 Mbps basic CCK
        for _try in 0..mlme_logic::ASSOC_MAX_TRIES {
            ASSOC_STAGE.store(assoc_stage::ASSOC_SENT, Ordering::Relaxed);
            let mut frame = [0u8; 128];
            let n = mlme_logic::build_assoc_req(
                &mut frame,
                &sta,
                &ap,
                &mlme_logic::AssocReqParams {
                    ssid: &target.ssid[..target.ssid_len as usize],
                    rates: &rates,
                    capab: mlme_logic::CAP_ESS,
                    listen_interval: 10,
                    rsn: !target.open,
                },
            )
            .ok_or("build assoc")?;
            self.tx_frame(&frame[..n])?;
            match self.poll_mgmt(ASSOC_RESP_SPINS) {
                Some(RxMgmt::AssocResp(r)) => {
                    LAST_STATUS.store(r.status as u32, Ordering::Relaxed);
                    ASSOC_AID.store(r.aid as u32, Ordering::Relaxed);
                    state = mlme_next(state, MlmeEvent::AssocResp(r.status), target.open);
                    break;
                }
                Some(RxMgmt::Deauth) => return Err("assoc: deauth during assoc"),
                _ => {}
            }
        }
        if state == MlmeState::Connected || state == MlmeState::Handshaking {
            ASSOC_STAGE.store(assoc_stage::ASSOCIATED, Ordering::Relaxed);
        }
        crate::serial_println!(
            "mt7601u: assoc result state={:?} aid={} last_status={:#06x}",
            state,
            ASSOC_AID.load(Ordering::Relaxed),
            LAST_STATUS.load(Ordering::Relaxed),
        );
        crate::log::flush();
        if (state == MlmeState::Connected || state == MlmeState::Handshaking)
            && self.deferred_cal_pending
        {
            // OT-39 targeted CAL-defer: the first targeted tune deliberately
            // omitted CAL_BW. Once the AP has accepted association and RF is
            // settled, restore the BW calibration before the 4-way/data path.
            // Keep the same fast-fail
            // budget as channel switching: a vendor-MCU wedge degrades this join
            // attempt and releases USB_DEVICE instead of stalling governance.
            POST_ASSOC_CAL_STATE.store(post_assoc_cal_state::BEFORE, Ordering::Relaxed);
            let cal_bulk_budget = SCAN_BULK_BUDGET;
            crate::serial_println!(
                "mt7601u: POST-ASSOC-CAL before cmd=CAL_BW ch={} budget={}",
                target.channel,
                cal_bulk_budget,
            );
            crate::log::flush();
            let saved = self.bulk_budget;
            self.bulk_budget = cal_bulk_budget;
            let recal = phy::calibrate_bw(self);
            self.bulk_budget = saved;
            match recal {
                Ok(()) => {
                    self.deferred_cal_pending = false;
                    POST_ASSOC_CAL_STATE.store(post_assoc_cal_state::OK, Ordering::Relaxed);
                    crate::serial_println!("mt7601u: POST-ASSOC-CAL after cmd=CAL_BW result=OK");
                }
                Err(reason) => {
                    POST_ASSOC_CAL_STATE.store(post_assoc_cal_state::FAILED, Ordering::Relaxed);
                    crate::serial_println!(
                        "mt7601u: POST-ASSOC-CAL after cmd=CAL_BW result=FAIL reason={}",
                        reason
                    );
                    crate::log::flush();
                    return Err("assoc: deferred CAL_BW failed");
                }
            }
            crate::log::flush();
        }
        // WPA2: association leaves us in Handshaking — run the 4-way to reach
        // Connected (open networks short-circuit straight to Connected above).
        if state == MlmeState::Handshaking {
            state = self.four_way(sta, ap, target)?;
        }
        if state == MlmeState::Connected {
            *ASSOC_BSSID.lock() = ap;
            CONNECTED.store(true, Ordering::Release);
        }
        Ok(state)
    }

    /// The WPA2 4-way handshake (M1→M4) over the EAPOL data path, reusing W0's
    /// crypto. RX M1 (ANonce) → derive PTK → TX M2 (SNonce+MIC) → RX M3 (verify
    /// MIC, unwrap+install GTK) → install PTK → TX M4 → Connected. SECURITY (OT-2,
    /// CLOSED): the SNonce is now a CSPRNG draw from the kernel ChaCha20 DRBG
    /// (`gen_snonce` → `crate::rng::fill_random`, pulled by `wifi`), so the PTK is
    /// derived from cryptographically-random material — W2 is a security boundary.
    /// (Only the explicit no-`kernel-rng` bring-up config keeps the insecure
    /// counter, and is documented as not-a-security-boundary.)
    fn four_way(
        &mut self,
        sta: [u8; 6],
        ap: [u8; 6],
        target: &AssocTarget,
    ) -> Result<mlme_logic::MlmeState, &'static str> {
        use crate::net::wpa::{eapol, keywrap};
        crate::serial_println!("mt7601u: 4-way — awaiting M1 (ANonce)");
        crate::log::flush();

        // --- M1: AP → STA (ANonce) ---
        let m1 = self
            .poll_eapol(EAPOL_RESP_SPINS)
            .ok_or("4way: M1 timeout")?;
        let kf1 = eapol::KeyFrame::parse(&m1).ok_or("4way: M1 parse")?;
        let version = kf1.version();
        let mut anonce = [0u8; 32];
        anonce.copy_from_slice(kf1.nonce());
        let mut replay = [0u8; 8];
        replay.copy_from_slice(&m1[9..17]); // echo the AP's replay counter
        HANDSHAKE_STAGE.store(handshake_stage::M1_RX, Ordering::Relaxed);
        crate::serial_println!("mt7601u: 4-way M1 received — deriving PTK");
        crate::log::flush();

        // --- Derive PTK (W0): aa = AP, spa = STA ---
        let snonce = self.gen_snonce()?;
        let ptk = crate::net::wpa::kdf::ptk_ccmp(&target.pmk, &ap, &sta, &anonce, &snonce);
        crate::serial_println!("mt7601u: 4-way PTK derived — sending M2");
        crate::log::flush();

        // --- M2: STA → AP (SNonce + MIC + the STA's RSN IE) ---
        // WPA2 M2 MUST carry the supplicant's RSNE in Key Data: hostapd authenticates
        // the MIC, then `wpa_validate_wpa_ie` checks it matches the (Re)Assoc Request's
        // RSNE and SILENTLY DROPS M2 (→ no M3) if it's absent or differs. Echo the same
        // 22-byte WPA2-PSK IE build_assoc_req sent.
        let mut rsne = [0u8; 22];
        let rsne_len =
            mlme_logic::build_rsn_ie_wpa2_psk(&mut rsne, 0).ok_or("4way: M2 rsne build")?;
        let key_info = 2 | eapol::KEY_INFO_KEY_TYPE | eapol::KEY_INFO_MIC; // v2, pairwise
        let mut m2 = [0u8; 128];
        let m2n = eapol::build_key_frame(
            &mut m2,
            1,
            key_info,
            16,
            &replay,
            &snonce,
            &rsne[..rsne_len],
        )
        .ok_or("4way: M2 build")?;
        if !eapol::install_mic(version, &ptk.kck, &mut m2[..m2n]) {
            return Err("4way: M2 MIC");
        }
        self.tx_eapol(&sta, &ap, &m2[..m2n])?;
        HANDSHAKE_STAGE.store(handshake_stage::M2_TX, Ordering::Relaxed);
        RX_MGMT_FRAMES.store(0, Ordering::Relaxed);
        RX_DIRECTED_COUNT.store(0, Ordering::Relaxed);
        RX_DIRECTED_FC.store(0, Ordering::Relaxed);
        crate::serial_println!("mt7601u: 4-way M2 sent — awaiting M3 (GTK)");
        crate::log::flush();

        // --- M3: AP → STA (GTK + MIC) ---
        let m3 = match self.poll_eapol(EAPOL_RESP_SPINS) {
            Some(m) => m,
            None => {
                let directed_fc = RX_DIRECTED_FC.load(Ordering::Relaxed);
                crate::serial_println!(
                    "mt7601u: 4way M3 timeout — rx_frames={} directed_to_us={} first_directed_fc={:#06x} (directed=0 ⇒ AP rejected our M2 MIC/PTK; directed>0 fc=0x08/0x88 ⇒ M3 arrived as data but EAPOL not extracted; fc=0xc0 ⇒ deauth)",
                    RX_MGMT_FRAMES.load(Ordering::Relaxed),
                    RX_DIRECTED_COUNT.load(Ordering::Relaxed),
                    directed_fc & 0xffff,
                );
                crate::log::flush();
                return Err("4way: M3 timeout");
            }
        };
        // Discriminator (M3 MIC fix): the MIC must cover EXACTLY the EAPOL PDU
        // (4 + body_len), not the trailing FCS/padding the 802.11 frame carries.
        // Log buf-len vs declared PDU len so a residual mismatch is self-diagnosing.
        let m3_body = if m3.len() >= 4 {
            u16::from_be_bytes([m3[2], m3[3]]) as usize
        } else {
            0
        };
        crate::serial_println!(
            "mt7601u: 4-way M3 rx — buf_len={} eapol_pdu_len={} (trailing={})",
            m3.len(),
            4 + m3_body,
            m3.len().saturating_sub(4 + m3_body),
        );
        crate::log::flush();
        if !eapol::verify_mic(version, &ptk.kck, &m3) {
            return Err("4way: M3 MIC mismatch");
        }
        let kf3 = eapol::KeyFrame::parse(&m3).ok_or("4way: M3 parse")?;
        let mut replay3 = [0u8; 8];
        replay3.copy_from_slice(&m3[9..17]);
        HANDSHAKE_STAGE.store(handshake_stage::M3_RX, Ordering::Relaxed);
        crate::serial_println!("mt7601u: 4-way M3 received (MIC ok) — installing PTK");
        crate::log::flush();

        // Install the pairwise TK (CCMP, WCID for the STA link).
        self.wcid_set_key(STA_WCID, &ptk.tk, regs::CIPHER_AES_CCMP, true, 0)?;

        // CB-1 ISC-1: GTK install (first-class — broadcast RX is the missing piece
        // for a usable link). M3 key-data is AES-key-wrapped (KEK); unwrap it, pull
        // the GTK from its KDE (`eapol::gtk_from_kde`, T-038 byte layout confirmed
        // against raw hostap), and install it to GROUP_WCID (0, the mcast slot — the
        // WCID-ATTR is programmed off its all-1s init by `wcid_set_key`, T-041).
        // ISC-6 kill-switch: skipped when `GROUP_RX_ENABLED` is off ⇒ pairwise-only,
        // the W2 baseline. A GTK failure does not block unicast (the pairwise key
        // carries DHCP unicast) — `GTK_INSTALLED` gates the broadcast/DHCP path.
        GTK_INSTALLED.store(false, Ordering::Release);
        let group_rx = GROUP_RX_ENABLED.load(Ordering::Relaxed);
        let kd_len = kf3.key_data_len() as usize;
        let kd_start = eapol::KEY_DATA_OFFSET;
        if kd_len >= 16 && m3.len() >= kd_start + kd_len {
            let mut unwrapped = [0u8; 64];
            match keywrap::aes_unwrap(&m3[kd_start..kd_start + kd_len], &ptk.kek, &mut unwrapped) {
                // `group_key_to_install` is the ISC-6 kill-switch gate: `None` when
                // the switch is off (pairwise-only, the W2 baseline) OR no GTK-KDE.
                Some(n) => match eapol::group_key_to_install(group_rx, &unwrapped[..n]) {
                    Some((gtk, idx)) => {
                        match self.wcid_set_key(GROUP_WCID, gtk, regs::CIPHER_AES_CCMP, false, idx)
                        {
                            Ok(()) => {
                                GTK_INSTALLED.store(true, Ordering::Release);
                                crate::serial_println!(
                                    "mt7601u: gtk_install wcid={} cipher=ccmp gtk_kde_len={} key_idx={} (broadcast RX on)",
                                    GROUP_WCID,
                                    gtk.len(),
                                    idx,
                                );
                                // Read back WCID-0 ATTR so a wrong/failed group-key
                                // write self-localizes (T-041: it must leave the all-1s
                                // init — cipher bits[3:1]=CCMP(4), PAIRWISE bit0=0).
                                let attr = self.rr(regs::WCID_ATTR_BASE);
                                let cipher = (attr & regs::WCID_ATTR_PKEY_MODE_MASK)
                                    >> regs::WCID_ATTR_PKEY_MODE_SHIFT;
                                let pairwise = attr & regs::WCID_ATTR_PAIRWISE;
                                crate::serial_println!(
                                    "mt7601u: gtk_wcid0_attr readback={:#010x} cipher={} pairwise={} (want cipher=4 pairwise=0, not all-1s)",
                                    attr,
                                    cipher,
                                    pairwise,
                                );
                            }
                            Err(e) => {
                                crate::serial_println!(
                                    "mt7601u: 4way M3 GTK WCID-install failed: {} (broadcast RX off)",
                                    e
                                );
                            }
                        }
                    }
                    None if !group_rx => {
                        crate::serial_println!(
                            "mt7601u: 4way M3 GTK install SKIPPED (group-rx kill-switch off — pairwise-only)"
                        );
                    }
                    None => {
                        crate::serial_println!(
                            "mt7601u: 4way M3 GTK-KDE not found (broadcast RX off)"
                        );
                    }
                },
                None => {
                    crate::serial_println!("mt7601u: 4way M3 GTK unwrap failed");
                }
            }
        }

        // --- M4: STA → AP (MIC only, secure) ---
        let key_info4 = 2 | eapol::KEY_INFO_KEY_TYPE | eapol::KEY_INFO_MIC | eapol::KEY_INFO_SECURE;
        let mut m4 = [0u8; 128];
        let m4n = eapol::build_key_frame(&mut m4, 1, key_info4, 16, &replay3, &[0u8; 32], &[])
            .ok_or("4way: M4 build")?;
        if !eapol::install_mic(version, &ptk.kck, &mut m4[..m4n]) {
            return Err("4way: M4 MIC");
        }
        self.tx_eapol(&sta, &ap, &m4[..m4n])?;
        HANDSHAKE_STAGE.store(handshake_stage::M4_TX, Ordering::Relaxed);
        crate::serial_println!("mt7601u: 4-way handshake complete — CCMP keys installed");
        crate::log::flush();
        Ok(mlme_logic::MlmeState::Connected)
    }

    /// Transmit an EAPOL payload as an 802.11 data frame (mlme_logic encap +
    /// the mgmt-TX path; the EAPOL exchange is unencrypted, pre-key).
    fn tx_eapol(&mut self, sta: &[u8; 6], ap: &[u8; 6], eapol: &[u8]) -> Result<(), &'static str> {
        let mut frame = [0u8; 192];
        let n = mlme_logic::build_eapol_data(&mut frame, sta, ap, eapol).ok_or("eapol encap")?;
        self.tx_frame(&frame[..n])
    }

    /// Poll for a received EAPOL-Key frame (the 4-way M1/M3): drain the RX pipe,
    /// extract the EAPOL payload from any data frame, return it. `None` on timeout.
    fn poll_eapol(&mut self, spins: u32) -> Option<EapolBuf> {
        for _ in 0..spins {
            xhci::poll_events();
            if let Some(buf) = self.drain_eapol() {
                return Some(buf);
            }
            udelay(5);
        }
        None
    }

    /// Drain the RX pipe and return the first EAPOL-Key payload (copied into a
    /// fixed buffer; the 4-way frames are < 256 bytes).
    fn drain_eapol(&mut self) -> Option<EapolBuf> {
        let our_mac = *READBACK_MAC.lock();
        let mut found: Option<EapolBuf> = None;
        for event in self.pkt_rx.completions() {
            let Some(idx) = self.pkt_rx.index_of(event.parameter) else {
                continue;
            };
            let Some(buffer) = self.rx_table[idx].take() else {
                continue;
            };
            let code = event.completion_code();
            if code == 1 || code == 13 {
                RX_MGMT_FRAMES.fetch_add(1, Ordering::Relaxed);
                let len = RX_BUF_LEN.saturating_sub(event.transfer_residue() as usize);
                let m = parse_rx_eapol(&buffer, len, &our_mac);
                if found.is_none() {
                    found = m;
                }
            }
            let _ = self.post_rx(buffer);
        }
        found
    }

    /// Draw a fresh SNonce for the 4-way handshake.
    ///
    /// OT-2 (CLOSED): with `kernel-rng` — which `wifi` now pulls — the SNonce is a
    /// CSPRNG draw from the kernel ChaCha20 DRBG (`crate::rng::fill_random`),
    /// making the derived PTK depend on cryptographically-random material. This is
    /// the difference between W2 *associating* and W2 being a *security boundary*.
    /// **Fail-closed:** if the DRBG is unavailable we return an error rather than
    /// fall back to a guessable nonce mid-handshake.
    #[cfg(feature = "kernel-rng")]
    fn gen_snonce(&self) -> Result<[u8; 32], &'static str> {
        let mut n = [0u8; 32];
        crate::rng::fill_random(&mut n).map_err(|_| "snonce: kernel CSPRNG unavailable")?;
        Ok(n)
    }

    /// INSECURE bring-up fallback, compiled only when `kernel-rng` is absent. W2
    /// is then NOT a security boundary (documented). Unreachable in the shipped
    /// feature graph (`wifi` ⇒ `kernel-rng`); retained for an explicit no-DRBG
    /// bring-up config.
    #[cfg(not(feature = "kernel-rng"))]
    fn gen_snonce(&self) -> Result<[u8; 32], &'static str> {
        let ctr = SNONCE_CTR.fetch_add(1, Ordering::Relaxed);
        let mac = *READBACK_MAC.lock();
        Ok(counter_snonce(ctr, mac))
    }

    /// If an association target is configured — either a runtime `set_target` or
    /// the assoc-scenario's baked-in `configured_target()` — match its SSID
    /// against the scan list and associate. Scan-only builds have neither, so
    /// this is a no-op and the W1 path is unchanged.
    fn maybe_associate(&mut self) {
        let Some(base) = TARGET.lock().clone().or_else(configured_target) else {
            return;
        };
        // A pre-set runtime BSSID ⇒ try exactly that one. Otherwise FAIL OVER across
        // EVERY BSS broadcasting the SSID: a mesh has several nodes (a strong one that
        // steers 2.4 GHz clients away and silently ignores our auth, and another that
        // accepts them), so try each until one associates — exactly like a roaming
        // client. This is what lets us land on the node Ubuntu uses instead of
        // fixating on the first/strongest match.
        if base.bssid != [0u8; 6] {
            let _ = self.associate(&base);
            crate::log::flush();
            return;
        }
        let want = &base.ssid[..base.ssid_len as usize];
        // Targeted connect (base.channel != 0): we are PARKED on the AP's channel
        // (the targeted scan tuned us there). A station joining a known AP NEVER
        // leaves its channel — and on this chip a channel switch wedges the MCU
        // (mcu-resp-timeout → unusable command path). So when
        // a channel is configured, only consider BSSes on THAT channel. This also
        // naturally skips the strong ch6 steering node (e0:63:da) that ignores us —
        // we go straight to the ch11 node (6c:63:f8) Ubuntu uses. base.channel == 0
        // (full-sweep / W1 scan path) keeps the old any-channel behavior.
        let stay_ch = base.channel;
        let mut candidates: alloc::vec::Vec<([u8; 6], u8)> = self
            .bss
            .iter()
            .filter(|b| b.ssid_len as usize == want.len() && &b.ssid[..want.len()] == want)
            .filter(|b| stay_ch == 0 || b.channel == stay_ch)
            .map(|b| (b.bssid, b.channel))
            .collect();
        if candidates.is_empty() {
            crate::serial_println!(
                "mt7601u: assoc target SSID not found in scan on ch={} (bss={})",
                stay_ch,
                self.bss.len()
            );
            crate::log::flush();
            return;
        }
        // Lowest channel first. Any failed retune terminalizes the device and
        // prevents remaining candidates from reaching association or DHCP.
        candidates.sort_by_key(|&(_, ch)| ch);
        crate::serial_println!(
            "mt7601u: {} BSS(es) match the target SSID — failover order by channel",
            candidates.len()
        );
        crate::log::flush();
        for (bssid, channel) in candidates {
            let mut t = base.clone();
            t.bssid = bssid;
            t.channel = channel;
            crate::serial_println!(
                "mt7601u: failover → BSS {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} ch={}",
                bssid[0],
                bssid[1],
                bssid[2],
                bssid[3],
                bssid[4],
                bssid[5],
                channel,
            );
            crate::log::flush();
            match self.associate(&t) {
                Ok(state) => {
                    crate::serial_println!("mt7601u: associate() → {:?}", state);
                }
                Err(e) => {
                    crate::serial_println!(
                        "mt7601u: BSS ch={} failed: {} — trying next",
                        channel,
                        e
                    );
                }
            }
            if CONNECTED.load(Ordering::Acquire) {
                crate::log::flush();
                return;
            }
        }
        crate::log::flush();
    }

    /// The full hardware bring-up (init.c `mt7601u_init_hardware`), each stage
    /// breadcrumbed (DISPATCH §4).
    fn bring_up(&mut self) -> Result<(), &'static str> {
        // WLAN enable + PLL/XTAL (init.c mt7601u_chip_onoff/set_wlan_state).
        let cur = self.rr(regs::WLAN_FUN_CTRL);
        self.wr(
            regs::WLAN_FUN_CTRL,
            cur | regs::WLAN_FUN_CTRL_WLAN_EN | regs::WLAN_FUN_CTRL_WLAN_CLK_EN,
        )?;
        udelay(20);
        if !self.poll(
            regs::CMB_CTRL,
            regs::CMB_CTRL_XTAL_RDY | regs::CMB_CTRL_PLL_LD,
            regs::CMB_CTRL_XTAL_RDY | regs::CMB_CTRL_PLL_LD,
            4_000,
        ) {
            crate::serial_println!("mt7601u: PLL/XTAL check failed (non-fatal)");
        }

        // wait_asic_ready: MAC_CSR0 reads neither 0 nor all-ones.
        if !self.wait_asic_ready() {
            return Err("ASIC not ready (pre-firmware)");
        }
        let asic = self.rr(regs::ASIC_VERSION);
        let mac_rev = self.rr(regs::MAC_CSR0);
        ASIC_REV.store(asic, Ordering::Relaxed);
        MAC_REV.store(mac_rev, Ordering::Relaxed);
        crate::serial_println!("mt7601u: ASIC rev {:#010x} MAC rev {:#010x}", asic, mac_rev);
        if (asic >> 16) as u16 != regs::ASIC_REV_MT7601 {
            return Err("ASIC version mismatch (not 0x7601)");
        }
        STAGE.store(stage::ASIC_READY, Ordering::Relaxed);

        self.load_firmware()?;
        STAGE.store(stage::FIRMWARE_LOADED, Ordering::Relaxed);
        crate::serial_println!("mt7601u: firmware running");
        crate::log::flush();

        // Post-FW: WPDMA idle, ASIC ready again, CSR/BBP reset, USB DMA.
        // Every boundary receives one operation-wide, non-replenishing budget;
        // the RAII guard restores the prior xHCI state on every return.
        let wpdma_budget = xhci::OperationBudgetGuard::new(POST_FW_OPERATION_BUDGET);
        let busy_mask = regs::WPDMA_GLO_CFG_TX_DMA_BUSY | regs::WPDMA_GLO_CFG_RX_DMA_BUSY;
        POST_FW_STATE.store(post_fw_state::WPDMA_WAIT, Ordering::Relaxed);
        crate::serial_println!(
            "mt7601u: POST-FW before op=WPDMA_IDLE total_budget={}",
            POST_FW_OPERATION_BUDGET
        );
        crate::log::flush();
        let (wpdma_step, wpdma_last) = self.post_fw_wpdma_poll(busy_mask, 100_000);
        POST_FW_WPDMA_LAST.store(wpdma_last, Ordering::Relaxed);
        let wpdma_account =
            retain_post_fw_budget(post_fw_operation::WPDMA, wpdma_budget.snapshot());
        print_post_fw_budget(wpdma_account);
        match wpdma_step {
            post_fw_logic::WpdmaStep::OperationBudgetExhausted => {
                POST_FW_STATE.store(post_fw_state::WPDMA_OPERATION_BUDGET, Ordering::Relaxed);
                crate::serial_println!(
                    "mt7601u: POST-FW after op=WPDMA_IDLE result=FAIL reason=operation-budget last={:#010x}",
                    wpdma_last
                );
                crate::log::flush();
                return Err("post-firmware WPDMA operation budget exhausted");
            }
            post_fw_logic::WpdmaStep::Idle => {
                POST_FW_STATE.store(post_fw_state::WPDMA_IDLE, Ordering::Relaxed);
                crate::serial_println!(
                    "mt7601u: POST-FW after op=WPDMA_IDLE result=OK last={:#010x}",
                    wpdma_last
                );
                crate::log::flush();
            }
            post_fw_logic::WpdmaStep::DeviceUnresponsive => {
                POST_FW_STATE.store(post_fw_state::WPDMA_UNRESPONSIVE, Ordering::Relaxed);
                crate::serial_println!(
                    "mt7601u: POST-FW after op=WPDMA_IDLE result=FAIL reason=device-unresponsive last={:#010x}",
                    wpdma_last
                );
                crate::log::flush();
                return Err("post-firmware WPDMA device unresponsive");
            }
            post_fw_logic::WpdmaStep::Busy => {
                POST_FW_STATE.store(post_fw_state::WPDMA_BUSY_TIMEOUT, Ordering::Relaxed);
                crate::serial_println!(
                    "mt7601u: POST-FW after op=WPDMA_IDLE result=FAIL reason=busy-timeout last={:#010x}",
                    wpdma_last
                );
                crate::log::flush();
                return Err("post-firmware WPDMA busy timeout");
            }
        }
        drop(wpdma_budget);

        let asic_budget = xhci::OperationBudgetGuard::new(POST_FW_OPERATION_BUDGET);
        POST_FW_STATE.store(post_fw_state::ASIC_WAIT, Ordering::Relaxed);
        crate::serial_println!(
            "mt7601u: POST-FW before op=ASIC_READY total_budget={}",
            POST_FW_OPERATION_BUDGET
        );
        crate::log::flush();
        let asic_ready = self.wait_asic_ready_post_fw();
        let asic_account = retain_post_fw_budget(post_fw_operation::ASIC, asic_budget.snapshot());
        print_post_fw_budget(asic_account);
        if !asic_ready {
            POST_FW_STATE.store(post_fw_state::ASIC_FAILED, Ordering::Relaxed);
            crate::serial_println!(
                "mt7601u: POST-FW after op=ASIC_READY result=FAIL reason={}",
                if asic_account.exhausted {
                    "operation-budget"
                } else {
                    "not-ready"
                }
            );
            crate::log::flush();
            return Err("ASIC not ready (post-firmware)");
        }
        POST_FW_STATE.store(post_fw_state::ASIC_READY, Ordering::Relaxed);
        crate::serial_println!("mt7601u: POST-FW after op=ASIC_READY result=OK");
        crate::log::flush();
        drop(asic_budget);

        let csr_budget = xhci::OperationBudgetGuard::new(POST_FW_OPERATION_BUDGET);
        POST_FW_STATE.store(post_fw_state::CSR_BBP_RESET, Ordering::Relaxed);
        crate::serial_println!(
            "mt7601u: POST-FW before op=CSR_BBP_RESET total_budget={}",
            POST_FW_OPERATION_BUDGET
        );
        crate::log::flush();
        let csr_result = self.reset_csr_bbp();
        let csr_account = retain_post_fw_budget(post_fw_operation::CSR_BBP, csr_budget.snapshot());
        print_post_fw_budget(csr_account);
        if let Err(reason) = csr_result {
            POST_FW_STATE.store(post_fw_state::CSR_BBP_FAILED, Ordering::Relaxed);
            crate::serial_println!(
                "mt7601u: POST-FW after op=CSR_BBP_RESET result=FAIL reason={} budget_exhausted={}",
                reason,
                csr_account.exhausted
            );
            crate::log::flush();
            return Err("post-firmware CSR/BBP reset failed");
        }
        POST_FW_STATE.store(post_fw_state::CSR_BBP_READY, Ordering::Relaxed);
        crate::serial_println!("mt7601u: POST-FW after op=CSR_BBP_RESET result=OK");
        crate::log::flush();
        drop(csr_budget);

        let usb_dma_budget = xhci::OperationBudgetGuard::new(POST_FW_OPERATION_BUDGET);
        POST_FW_STATE.store(post_fw_state::USB_DMA_INIT, Ordering::Relaxed);
        crate::serial_println!(
            "mt7601u: POST-FW before op=USB_DMA_INIT total_budget={}",
            POST_FW_OPERATION_BUDGET
        );
        crate::log::flush();
        let usb_dma_result = self.init_usb_dma();
        let usb_dma_account =
            retain_post_fw_budget(post_fw_operation::USB_DMA, usb_dma_budget.snapshot());
        print_post_fw_budget(usb_dma_account);
        if let Err(reason) = usb_dma_result {
            POST_FW_STATE.store(post_fw_state::USB_DMA_FAILED, Ordering::Relaxed);
            crate::serial_println!(
                "mt7601u: POST-FW after op=USB_DMA_INIT result=FAIL reason={} budget_exhausted={}",
                reason,
                usb_dma_account.exhausted
            );
            crate::log::flush();
            return Err("post-firmware USB DMA init failed");
        }
        POST_FW_STATE.store(post_fw_state::USB_DMA_READY, Ordering::Relaxed);
        crate::serial_println!("mt7601u: POST-FW after op=USB_DMA_INIT result=OK");
        crate::log::flush();
        drop(usb_dma_budget);

        // MCU command interface up (Q_SELECT) + the response pipe is live.
        let q_select_budget = xhci::OperationBudgetGuard::new(POST_FW_OPERATION_BUDGET);
        POST_FW_STATE.store(post_fw_state::MCU_Q_SELECT, Ordering::Relaxed);
        crate::serial_println!(
            "mt7601u: POST-FW before op=MCU_Q_SELECT total_budget={}",
            POST_FW_OPERATION_BUDGET
        );
        crate::log::flush();
        let q_select_result = self.mcu_function_select(regs::FUNC_Q_SELECT, POST_FW_Q_SELECT_VALUE);
        let q_select_account =
            retain_post_fw_budget(post_fw_operation::Q_SELECT, q_select_budget.snapshot());
        print_post_fw_budget(q_select_account);
        if let Err(reason) = q_select_result {
            POST_FW_STATE.store(post_fw_state::MCU_Q_SELECT_FAILED, Ordering::Relaxed);
            crate::serial_println!(
                "mt7601u: POST-FW after op=MCU_Q_SELECT result=FAIL reason={} budget_exhausted={}",
                reason,
                q_select_account.exhausted
            );
            crate::log::flush();
            return Err("post-firmware MCU Q-select failed");
        }
        POST_FW_STATE.store(post_fw_state::CONTROL_PLANE_READY, Ordering::Relaxed);
        crate::serial_println!("mt7601u: POST-FW after op=MCU_Q_SELECT result=OK");
        crate::log::flush();
        STAGE.store(stage::MCU_READY, Ordering::Relaxed);
        drop(q_select_budget);

        // Linux `mt7601u_mcu_cmd_init` posts exactly one command-response URB
        // immediately after Q_SELECT. `mt7601u_dma_init` then posts packet-RX
        // URBs before `mt7601u_init_hardware` issues MAC RANDOM_WRITEs.
        let response_index = self.ensure_mcu_response_armed()?;
        let packet_rx_posted = self.ensure_packet_rx_ready()?;
        if !linux_parity_logic::ready_for_mac_init(
            self.cmd_resp_pending.is_some(),
            packet_rx_posted,
            RX_BUFFERS,
        ) {
            return Err("Linux post-Q_SELECT readiness incomplete");
        }
        crate::serial_println!(
            "mt7601u: POST-QSELECT readiness response_td_owned=true response_index={} response_arms={} packet_rx_posted={}/{} before_mac_init=true",
            response_index,
            self.cmd_resp_stats.arms,
            packet_rx_posted,
            RX_BUFFERS,
        );
        crate::log::flush();

        // MAC + BBP initvals (RANDOM_WRITE), beacon offsets, misc.
        let post_qselect_budget =
            xhci::OperationBudgetGuard::new(linux_parity_logic::POST_QSELECT_OPERATION_BUDGET);
        let fence_refusals_before = dma_arena::fence_refusals();
        crate::serial_println!(
            "mt7601u: POST-QSELECT operation before total_budget={} response_td_owned={} packet_rx_posted={}/{}",
            linux_parity_logic::POST_QSELECT_OPERATION_BUDGET,
            self.cmd_resp_pending.is_some(),
            self.posted_rx_count(),
            RX_BUFFERS,
        );
        crate::log::flush();
        let post_qselect_result = self.run_post_qselect_init();
        let post_qselect_snapshot = post_qselect_budget.snapshot();
        MAC_INIT_BUDGET_REMAINING.store(post_qselect_snapshot.remaining, Ordering::Relaxed);
        MAC_INIT_BUDGET_EXHAUSTED.store(post_qselect_snapshot.exhausted, Ordering::Relaxed);
        let mac = match post_qselect_result {
            Ok(mac) => {
                crate::serial_println!(
                    "mt7601u: POST-QSELECT operation after result=OK budget_limit={} budget_consumed={} budget_remaining={} budget_exhausted={} boundary_flushes={:#04x} response_td_owned={} packet_rx_posted={}/{} fence_refusals_total={}",
                    post_qselect_snapshot.limit,
                    post_qselect_snapshot
                        .limit
                        .saturating_sub(post_qselect_snapshot.remaining),
                    post_qselect_snapshot.remaining,
                    post_qselect_snapshot.exhausted,
                    POST_QSELECT_BOUNDARY_FLUSHES.load(Ordering::Relaxed),
                    self.cmd_resp_pending.is_some(),
                    self.posted_rx_count(),
                    RX_BUFFERS,
                    dma_arena::fence_refusals(),
                );
                crate::log::flush();
                mac
            }
            Err(failure) => {
                self.terminalize_post_qselect(
                    failure,
                    post_qselect_snapshot,
                    fence_refusals_before,
                );
                return Err("post-Q_SELECT operation failed");
            }
        };
        drop(post_qselect_budget);
        // eFUSE / EEPROM (the HARD-FAIL gate + the §2 MAC discriminator).
        crate::serial_println!(
            "mt7601u: MAC {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0],
            mac[1],
            mac[2],
            mac[3],
            mac[4],
            mac[5],
        );
        // Program the MAC address registers (mac.c mt7601u_set_macaddr).
        self.wr(
            regs::MAC_ADDR_DW0,
            u32::from_le_bytes([mac[0], mac[1], mac[2], mac[3]]),
        )?;
        // DW1 carries the high MAC bytes PLUS the U2ME_MASK=0xff field — without
        // it the HW does not match directed unicast frames to our MAC and drops
        // the AP's auth/assoc response (mac.c `mt7601u_set_macaddr`).
        self.wr(
            regs::MAC_ADDR_DW1,
            (mac[4] as u32) | ((mac[5] as u32) << 8) | regs::MAC_ADDR_DW1_U2ME_MASK,
        )?;
        STAGE.store(stage::EFUSE_READ, Ordering::Relaxed);

        // PHY init (RF/BBP bring-up + calibration) — the Forge-ported port.
        phy::phy_init(self)?;
        STAGE.store(stage::PHY_INIT, Ordering::Relaxed);

        self.mac_start()?;
        STAGE.store(stage::MAC_STARTED, Ordering::Relaxed);
        Ok(())
    }

    fn wait_asic_ready(&mut self) -> bool {
        for _ in 0..100 {
            let v = self.rr(regs::MAC_CSR0);
            if v != 0 && v != 0xffffffff {
                return true;
            }
            udelay(10);
        }
        false
    }

    /// Post-firmware variant: the all-ones transport sentinel is terminal.
    /// The pre-firmware helper above stays byte-for-byte unchanged so the cold
    /// initialization path retains its established retry shape.
    fn wait_asic_ready_post_fw(&mut self) -> bool {
        for _ in 0..100 {
            if !xhci::charge_operation(ChargeKind::Observation) {
                return false;
            }
            let v = self.rr(regs::MAC_CSR0);
            if v == post_fw_logic::DEVICE_UNRESPONSIVE {
                return false;
            }
            if v != 0 {
                return true;
            }
            if !xhci::charge_operation(ChargeKind::PollGap) {
                return false;
            }
            udelay(10);
        }
        false
    }

    fn reset_csr_bbp(&mut self) -> Result<(), &'static str> {
        self.wr(
            regs::MAC_SYS_CTRL,
            regs::MAC_SYS_CTRL_RESET_CSR | regs::MAC_SYS_CTRL_RESET_BBP,
        )?;
        self.wr(regs::USB_DMA_CFG, 0)?;
        if !xhci::charge_operation(ChargeKind::PollGap) {
            return Err("post-firmware operation budget exhausted before CSR/BBP settle");
        }
        msleep(1);
        self.wr(regs::MAC_SYS_CTRL, 0)
    }

    fn init_usb_dma(&mut self) -> Result<(), &'static str> {
        // RX bulk-agg timeout/limit + RX/TX bulk enable (init.c values).
        let val = regs::USB_DMA_CFG_RX_BULK_EN | regs::USB_DMA_CFG_TX_BULK_EN;
        self.wr(regs::USB_DMA_CFG, val)?;
        self.wr(regs::USB_DMA_CFG, val | regs::USB_DMA_CFG_UDMA_RX_WL_DROP)?;
        self.wr(regs::USB_DMA_CFG, val)
    }

    fn terminalize_post_qselect(
        &mut self,
        failure: PostQselectFailure,
        budget: xhci::OperationBudgetSnapshot,
        fence_refusals_before: u64,
    ) {
        self.mac_init_command_path_unusable = true;
        MAC_INIT_BUDGET_REMAINING.store(budget.remaining, Ordering::Relaxed);
        MAC_INIT_BUDGET_EXHAUSTED.store(budget.exhausted, Ordering::Relaxed);
        let fence_refusals = dma_arena::fence_refusals();
        MAC_INIT_FENCE_REFUSALS.store(fence_refusals, Ordering::Relaxed);

        if linux_parity_logic::post_qselect_terminal_action(
            MAC_INIT_TERMINAL.load(Ordering::Acquire) != mac_init_terminal::NONE,
        ) == linux_parity_logic::PostQselectTerminalAction::SuppressDuplicate
        {
            return;
        }
        if MAC_INIT_TERMINAL
            .compare_exchange(
                mac_init_terminal::NONE,
                mac_init_terminal::OPERATION_FAILED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return;
        }
        MAC_INIT_TERMINAL_COUNT.fetch_add(1, Ordering::Relaxed);
        let disposition = linux_parity_logic::post_qselect_failure_disposition();
        let consumed = budget.limit.saturating_sub(budget.remaining);
        let failure_class = linux_parity_logic::classify_post_qselect_failure(failure.reason);
        crate::serial_println!(
            "mt7601u: TERMINAL category=POST_QSELECT_OPERATION_FAILED stage={} failure_class={} reason={} \
             stop={} destructive_reload={} later_device_traffic={} terminal_count={} \
             device_traffic_after_terminal={} budget_limit={} budget_consumed={} \
             budget_remaining={} budget_exhausted={} wait_calls={} wait_iters={} \
             vendor_attempts={} retry_gaps={} vendor_exhaustions={} bulk_attempts={} \
             observations={} poll_gaps={} bulk_polls={} late_events={} no_event={} desync={} last_table={} \
             last_chunk={}/{} last_seq={} last_payload_len={} last_frame_len={} \
             last_wait_response={} response_td_owned={} response_arms={} \
             response_completions={} response_mismatches={} response_unowned={} \
             response_timeouts={} packet_rx_posted={}/{} fence_state={} \
             post_qselect_fence_refusals={} fence_refusals_total={} boundary_flushes={:#04x} sink_flush={}",
            failure.stage.label(),
            failure_class.label(),
            failure.reason,
            disposition.stop,
            disposition.attempt_firmware_reload,
            disposition.allow_later_device_traffic,
            MAC_INIT_TERMINAL_COUNT.load(Ordering::Relaxed),
            MAC_INIT_COMMANDS_BLOCKED_AFTER_TERMINAL.load(Ordering::Relaxed),
            budget.limit,
            consumed,
            budget.remaining,
            budget.exhausted,
            budget.counters.wait_calls,
            budget.counters.wait_iterations,
            budget.counters.vendor_attempts,
            budget.counters.vendor_retry_gaps,
            budget.counters.vendor_request_exhaustions,
            budget.counters.bulk_attempts,
            budget.counters.observations,
            budget.counters.poll_gaps,
            budget.counters.bulk_polls,
            budget.counters.late_events,
            budget.counters.no_event_exhaustions,
            budget.counters.desync_exhaustions,
            mac_init_table_label(MAC_INIT_LAST_TABLE.load(Ordering::Relaxed)),
            MAC_INIT_LAST_CHUNK.load(Ordering::Relaxed),
            MAC_INIT_LAST_CHUNKS.load(Ordering::Relaxed),
            MAC_INIT_LAST_SEQ.load(Ordering::Relaxed),
            MAC_INIT_LAST_PAYLOAD_LEN.load(Ordering::Relaxed),
            MAC_INIT_LAST_FRAME_LEN.load(Ordering::Relaxed),
            MAC_INIT_LAST_WAIT_RESPONSE.load(Ordering::Relaxed),
            self.cmd_resp_pending.is_some(),
            self.cmd_resp_stats.arms,
            self.cmd_resp_stats.completions,
            self.cmd_resp_stats.mismatches,
            self.cmd_resp_stats.unowned_events,
            self.cmd_resp_stats.timeout_windows,
            self.posted_rx_count(),
            RX_BUFFERS,
            if fence_refusals == fence_refusals_before {
                "clean"
            } else {
                "refused"
            },
            fence_refusals.saturating_sub(fence_refusals_before),
            fence_refusals,
            POST_QSELECT_BOUNDARY_FLUSHES.load(Ordering::Relaxed),
            disposition.flush_serial_and_sink,
        );
        if disposition.flush_serial_and_sink {
            crate::log::flush();
        }
    }

    fn enter_post_qselect_stage(&self, stage: linux_parity_logic::PostQselectStage) {
        POST_QSELECT_STAGE.store(stage as u32 + 1, Ordering::Release);
        crate::serial_println!("mt7601u: POST-QSELECT stage={} enter", stage.label());
    }

    fn flush_post_qselect_boundary(&self, boundary: &'static str, bit: u8) {
        POST_QSELECT_BOUNDARY_FLUSHES.fetch_or(bit as u32, Ordering::AcqRel);
        let budget = xhci::operation_budget_snapshot();
        crate::serial_println!(
            "mt7601u: POST-QSELECT boundary={} sink_flush=true budget_remaining={} budget_exhausted={}",
            boundary,
            budget.remaining,
            budget.exhausted,
        );
        crate::log::flush();
    }

    fn run_post_qselect_init(&mut self) -> Result<[u8; 6], PostQselectFailure> {
        use linux_parity_logic::PostQselectStage as Stage;
        use linux_parity_logic::post_qselect_boundary as boundary;

        POST_QSELECT_BOUNDARY_FLUSHES.store(0, Ordering::Release);
        self.write_mac_initvals()?;

        self.enter_post_qselect_stage(Stage::MacStatus);
        if !self.poll(
            regs::MAC_STATUS,
            regs::MAC_STATUS_TX | regs::MAC_STATUS_RX,
            0,
            100_000,
        ) {
            return Err(PostQselectFailure::new(
                Stage::MacStatus,
                "MAC status busy after initvals",
            ));
        }

        self.flush_post_qselect_boundary("before_bbp", boundary::BBP_BEFORE);
        self.enter_post_qselect_stage(Stage::BbpReady);
        phy::wait_bbp_ready(self)
            .map_err(|reason| PostQselectFailure::new(Stage::BbpReady, reason))?;
        self.enter_post_qselect_stage(Stage::BbpCommon);
        self.mcu_write_reg_pairs(regs::MCU_MEMMAP_BBP, initvals::BBP_COMMON_VALS)
            .map_err(|reason| PostQselectFailure::new(Stage::BbpCommon, reason))?;
        self.enter_post_qselect_stage(Stage::BbpChip);
        self.mcu_write_reg_pairs(regs::MCU_MEMMAP_BBP, initvals::BBP_CHIP_VALS)
            .map_err(|reason| PostQselectFailure::new(Stage::BbpChip, reason))?;
        self.flush_post_qselect_boundary("after_bbp", boundary::BBP_AFTER);

        self.flush_post_qselect_boundary("before_wcid", boundary::WCID_BEFORE);
        self.enter_post_qselect_stage(Stage::WcidAddress);
        self.mcu_burst_write(regs::WCID_ADDR_BASE, &[0xffff_ffff, 0x00ff_ffff])
            .map_err(|reason| PostQselectFailure::new(Stage::WcidAddress, reason))?;
        self.enter_post_qselect_stage(Stage::SharedKeyMode);
        self.mcu_burst_write(regs::SKEY_MODE_BASE_0, &[0; 4])
            .map_err(|reason| PostQselectFailure::new(Stage::SharedKeyMode, reason))?;
        self.enter_post_qselect_stage(Stage::WcidAttribute);
        self.mcu_burst_write(regs::WCID_ATTR_BASE, &[1, 1])
            .map_err(|reason| PostQselectFailure::new(Stage::WcidAttribute, reason))?;
        self.flush_post_qselect_boundary("after_wcid", boundary::WCID_AFTER);

        self.enter_post_qselect_stage(Stage::InterveningRegisters);
        self.clear(
            regs::BEACON_TIME_CFG,
            regs::BEACON_TIME_CFG_TIMER_EN
                | regs::BEACON_TIME_CFG_SYNC_MODE
                | regs::BEACON_TIME_CFG_TBTT_EN
                | regs::BEACON_TIME_CFG_BEACON_TX,
        )
        .and_then(|_| {
            self.rmw(regs::US_CYC_CFG, regs::US_CYC_CNT, 0x1e)
                .map(|_| ())
        })
        .and_then(|_| self.wr(regs::TXOP_CTRL_CFG, (0x3f) | (0x58 << 8)))
        .map_err(|reason| PostQselectFailure::new(Stage::InterveningRegisters, reason))?;
        STAGE.store(stage::MAC_BBP_INIT, Ordering::Relaxed);

        self.flush_post_qselect_boundary("before_eeprom", boundary::EEPROM_BEFORE);
        self.enter_post_qselect_stage(Stage::EepromAcquire);
        let mac = self
            .eeprom_init()
            .map_err(|reason| PostQselectFailure::new(Stage::EepromAcquire, reason))?;
        self.flush_post_qselect_boundary("after_eeprom", boundary::EEPROM_AFTER);
        Ok(mac)
    }

    fn write_mac_initvals(&mut self) -> Result<(), PostQselectFailure> {
        let common_chunks = initvals::MAC_COMMON_VALS
            .len()
            .div_ceil(mcu_logic::MAX_REG_PAIRS_PER_CMD);
        let chip_chunks = initvals::MAC_CHIP_VALS
            .len()
            .div_ceil(mcu_logic::MAX_REG_PAIRS_PER_CMD);
        crate::serial_println!(
            "mt7601u: MAC-INIT before operation total_budget={} common_pairs={} common_chunks={} chip_pairs={} chip_chunks={} response_td_owned={} packet_rx_posted={}/{}",
            linux_parity_logic::POST_QSELECT_OPERATION_BUDGET,
            initvals::MAC_COMMON_VALS.len(),
            common_chunks,
            initvals::MAC_CHIP_VALS.len(),
            chip_chunks,
            self.cmd_resp_pending.is_some(),
            self.posted_rx_count(),
            RX_BUFFERS,
        );
        crate::log::flush();

        self.enter_post_qselect_stage(linux_parity_logic::PostQselectStage::MacCommon);
        self.mcu_write_mac_reg_pairs(mac_init_table::COMMON, initvals::MAC_COMMON_VALS)
            .map_err(|reason| {
                PostQselectFailure::new(linux_parity_logic::PostQselectStage::MacCommon, reason)
            })?;
        self.enter_post_qselect_stage(linux_parity_logic::PostQselectStage::MacChip);
        self.mcu_write_mac_reg_pairs(mac_init_table::CHIP, initvals::MAC_CHIP_VALS)
            .map_err(|reason| {
                PostQselectFailure::new(linux_parity_logic::PostQselectStage::MacChip, reason)
            })?;
        self.enter_post_qselect_stage(linux_parity_logic::PostQselectStage::MacAux);
        self.wr(regs::AUX_CLK_CFG, 0).map_err(|reason| {
            PostQselectFailure::new(linux_parity_logic::PostQselectStage::MacAux, reason)
        })?;
        crate::serial_println!(
            "mt7601u: MAC-INIT after operation result=OK enclosing_budget=true response_td_owned={} packet_rx_posted={}/{} fence_refusals_total={}",
            self.cmd_resp_pending.is_some(),
            self.posted_rx_count(),
            RX_BUFFERS,
            dma_arena::fence_refusals(),
        );
        crate::log::flush();
        Ok(())
    }

    fn init_bbp(&mut self) -> Result<(), &'static str> {
        // wait_bbp_ready (phy.c) — handled inside phy module helpers; here we
        // write the common+chip BBP tables via RANDOM_WRITE to the BBP memmap.
        phy::wait_bbp_ready(self)?;
        self.mcu_write_reg_pairs(regs::MCU_MEMMAP_BBP, initvals::BBP_COMMON_VALS)?;
        self.mcu_write_reg_pairs(regs::MCU_MEMMAP_BBP, initvals::BBP_CHIP_VALS)
    }

    fn init_wcid_mem(&mut self) -> Result<(), &'static str> {
        // WCID addr mem: pairs of 0xffffffff/0x00ffffff (init.c). Scan needs
        // only the table cleared; a single burst of the first entries suffices.
        let vals = [0xffff_ffffu32, 0x00ff_ffff];
        self.mcu_burst_write(regs::WCID_ADDR_BASE, &vals)?;
        let key0 = [0u32; 4];
        self.mcu_burst_write(regs::SKEY_MODE_BASE_0, &key0)?;
        let attr = [1u32, 1];
        self.mcu_burst_write(regs::WCID_ATTR_BASE, &attr)
    }
}

// --- The class driver ----------------------------------------------------------

/// The registered MT7601U class driver (offered every enumerated device).
pub struct Mt7601uDriver;

impl UsbClassDriver for Mt7601uDriver {
    fn name(&self) -> &'static str {
        "mt7601u"
    }

    /// Pure descriptor match: the confirmed `148f:7601` (+ the upstream clone
    /// VIDs). Disabled ⇒ decline (the BUILD-A unclaimed path).
    fn probe(&self, device: &UsbDevice) -> bool {
        is_enabled() && is_mt7601u(device.vendor_id, device.product_id)
    }

    fn bind(&self, device: &Arc<UsbDevice>, claimed: ClaimedDevice) -> Result<(), ClaimedDevice> {
        match bind_inner(device, claimed) {
            Ok(()) => Ok(()),
            Err((claimed, reason)) => {
                *BIND_FAIL.lock() = Some(reason);
                crate::serial_println!(
                    "mt7601u: bind failed vid={:04x} pid={:04x}: {}",
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

// ── W2 ISC-4: the WiFi link as a Phase-4 NetDevice ──────────────────────────
/// Ethernet TX/RX staging buffer length (MTU 1500 + headers, rounded).
const ETH_BUF_LEN: usize = 1600;
const NET_TX_BUFFERS: usize = 8;
const NET_RX_BUFFERS: usize = 8;

/// The associated WiFi link presented as a Phase-4 `NetDevice`, so the existing
/// IPv4/DHCP stack runs over it (DHCP-over-WiFi). Wraps the supplicant
/// (`Mt7601uDev` in PRIMARY) + ethernet buffer pools; converts each frame
/// 802.3↔802.11 (`build_data_frame`/`build_eth_from_rx`) with HW CCMP underneath.
pub struct WifiNetDevice {
    dev: Arc<SwitchMutex<Mt7601uDev>>,
    identity: NetDeviceIdentity,
    mac: [u8; 6],
    mtu: usize,
    tx_free: SwitchMutex<Vec<Arc<DmaBuffer>>>,
    rx_free: SwitchMutex<Vec<Arc<DmaBuffer>>>,
}

impl WifiNetDevice {
    fn new(dev: Arc<SwitchMutex<Mt7601uDev>>, mac: [u8; 6]) -> Result<Arc<Self>, &'static str> {
        let mut tx_free = Vec::new();
        for _ in 0..NET_TX_BUFFERS {
            tx_free.push(DmaBuffer::alloc_in_arena(ETH_BUF_LEN).map_err(|_| "net tx alloc")?);
        }
        let mut rx_free = Vec::new();
        for _ in 0..NET_RX_BUFFERS {
            rx_free.push(DmaBuffer::alloc_in_arena(ETH_BUF_LEN).map_err(|_| "net rx alloc")?);
        }
        Ok(Arc::new(WifiNetDevice {
            dev,
            identity: NetDeviceIdentity {
                driver: "mt7601u",
                name: "wlan0",
                pci_bdf: None,
            },
            mac,
            mtu: 1500,
            tx_free: SwitchMutex::new(tx_free),
            rx_free: SwitchMutex::new(rx_free),
        }))
    }
}

impl NetDevice for WifiNetDevice {
    fn identity(&self) -> &NetDeviceIdentity {
        &self.identity
    }
    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }
    fn link_state(&self) -> LinkState {
        if CONNECTED.load(Ordering::Acquire) {
            LinkState::Up
        } else {
            LinkState::Down
        }
    }
    fn mtu(&self) -> usize {
        self.mtu
    }
    fn tx_reserve(&self) -> Option<TxFrame> {
        let buffer = self.tx_free.lock().pop()?;
        Some(TxFrame { buffer })
    }
    fn tx_submit(&self, frame: TxFrame, len: usize) -> NetResult<()> {
        if !(14..=ETH_BUF_LEN).contains(&len) {
            return Err(NetError::InvalidFrame);
        }
        let result = {
            // SAFETY: a submitted TxFrame buffer is exclusively ours until recycle.
            let eth = unsafe { frame.buffer.as_slice() };
            let mut da = [0u8; 6];
            da.copy_from_slice(&eth[0..6]);
            let ethertype = u16::from_be_bytes([eth[12], eth[13]]);
            #[cfg(all(feature = "wire-rx", feature = "neo-test"))]
            crate::net::rx::assert_if_off("wlan0 dev");
            let mut dev = self.dev.lock();
            dev.data_tx(&da, ethertype, &eth[14..len])
        };
        self.tx_free.lock().push(frame.buffer);
        result.map_err(|_| NetError::Io)
    }
    fn rx_take(&self) -> Option<RxFrame> {
        let buffer = self.rx_free.lock().pop()?;
        let n = {
            #[cfg(all(feature = "wire-rx", feature = "neo-test"))]
            crate::net::rx::assert_if_off("wlan0 dev");
            let mut dev = self.dev.lock();
            // SAFETY: the rx_free buffer is exclusively ours until returned.
            let out = unsafe { buffer.as_mut_slice() };
            dev.data_rx(out)
        };
        match n {
            Some(len) => Some(RxFrame {
                buffer,
                offset: 0,
                len,
            }),
            None => {
                self.rx_free.lock().push(buffer);
                None
            }
        }
    }
    fn rx_recycle(&self, frame: RxFrame) {
        self.rx_free.lock().push(frame.buffer);
    }
    fn service(&self) {
        xhci::poll_events();
    }
}

/// The `148f:7601` + the upstream `mt7601u_device_table` clone ids (usb.c).
fn is_mt7601u(vid: u16, pid: u16) -> bool {
    matches!(
        (vid, pid),
        (0x0b05, 0x17d3)
            | (0x0e8d, 0x760a)
            | (0x0e8d, 0x760b)
            | (0x13d3, 0x3431)
            | (0x13d3, 0x3434)
            | (0x148f, 0x7601)
            | (0x148f, 0x760a)
            | (0x148f, 0x760b)
            | (0x148f, 0x760c)
            | (0x148f, 0x760d)
            | (0x2001, 0x3d04)
            | (0x2717, 0x4106)
            | (0x2955, 0x0001)
            | (0x2955, 0x1001)
            | (0x2955, 0x1003)
            | (0x2a5f, 0x1000)
            | (0x7392, 0x7710)
    )
}

/// Discover the bulk endpoints (usb.c `mt7601u_assign_pipes`): in descriptor
/// order, the bulk INs are [PKT_RX, CMD_RESP] and the bulk OUTs are
/// [INBAND_CMD, AC_BK/BE/VI/VO, HCCA]. Requires exactly 2 IN + 6 OUT.
fn assign_pipes(device: &UsbDevice) -> Result<(Vec<UsbEndpoint>, Vec<UsbEndpoint>), &'static str> {
    let iface = device
        .configurations
        .first()
        .and_then(|c| c.interfaces.first())
        .ok_or("no interface descriptor")?;
    let mut ins = Vec::new();
    let mut outs = Vec::new();
    for ep in &iface.endpoints {
        if ep.is_bulk() && ep.is_in() && ins.len() < regs::N_EP_IN {
            ins.push(*ep);
        } else if ep.is_bulk() && !ep.is_in() && outs.len() < regs::N_EP_OUT {
            outs.push(*ep);
        }
    }
    if ins.len() != regs::N_EP_IN || outs.len() != regs::N_EP_OUT {
        return Err("wrong bulk endpoint count (not 2-in/6-out)");
    }
    Ok((ins, outs))
}

// clippy-bank: oversized ClaimedDevice errors are a CB-4 USB error-type refactor.
#[allow(clippy::result_large_err)]
fn bind_inner(
    device: &Arc<UsbDevice>,
    mut claimed: ClaimedDevice,
) -> Result<(), (ClaimedDevice, &'static str)> {
    crate::serial_println!(
        "mt7601u: binding vid={:04x} pid={:04x}",
        device.vendor_id,
        device.product_id
    );
    crate::log::flush();
    STAGE.store(stage::ENUMERATED, Ordering::Relaxed);

    let (ins, outs) = match assign_pipes(device) {
        Ok(eps) => eps,
        Err(reason) => return Err((claimed, reason)),
    };

    // Allocate DMA staging BEFORE configuring endpoints (clean decline on OOM).
    let staging = match DmaBuffer::alloc_in_arena(STAGING_LEN) {
        Ok(b) => b,
        Err(_) => return Err((claimed, "staging alloc (arena)")),
    };
    let resp = match DmaBuffer::alloc_in_arena(MCU_RESP_LEN) {
        Ok(b) => b,
        Err(_) => return Err((claimed, "resp alloc (arena)")),
    };
    let tx_staging = match DmaBuffer::alloc_in_arena(TX_STAGING_LEN) {
        Ok(b) => b,
        Err(_) => return Err((claimed, "tx staging alloc (arena)")),
    };
    if mcu_logic::dma_spans_overlap(
        staging.physical_addr().as_u64(),
        staging.len(),
        resp.physical_addr().as_u64(),
        resp.len(),
    ) {
        return Err((claimed, "MCU command/response DMA spans overlap"));
    }
    crate::serial_println!(
        "mt7601u: MCU DMA spans tx={:#x}+{} resp={:#x}+{} distinct=1 fenced_on_submit=1",
        staging.physical_addr().as_u64(),
        staging.len(),
        resp.physical_addr().as_u64(),
        resp.len(),
    );

    // Configure the three pipes the scan uses: in-band OUT, packet-RX IN,
    // command-response IN. (The 5 TX-queue OUT EPs are not needed for scan.)
    let want = [
        outs[regs::EP_OUT_INBAND_CMD],
        outs[regs::EP_OUT_DATA],
        ins[regs::EP_IN_PKT_RX],
        ins[regs::EP_IN_CMD_RESP],
    ];
    let mut pipes = match claimed.configure_endpoints(&want) {
        Ok(p) => p,
        Err(reason) => return Err((claimed, reason)),
    };
    let cmd_resp = pipes.pop().expect("four pipes");
    let pkt_rx = pipes.pop().expect("four pipes");
    let data_tx = pipes.pop().expect("four pipes");
    let inband = pipes.pop().expect("four pipes");

    // SET_CONFIGURATION(1) — the MT7601U has a single vendor config.
    let cfg = device.configurations.first().map(|c| c.value).unwrap_or(1);
    if let Err(reason) = claimed.control_no_data(0x00, 9, cfg as u16, 0) {
        return Err((claimed, reason));
    }

    let mut dev = Mt7601uDev {
        claimed,
        inband,
        data_tx,
        pkt_rx,
        cmd_resp,
        staging,
        tx_staging,
        resp,
        cmd_resp_pending: None,
        cmd_resp_stats: McuResponseStats::default(),
        fce_upload_kicks: 0,
        rx_table: [const { None }; xhci::RING_TABLE],
        msg_seq: 0,
        ee: EepromParams::default(),
        bw: 0xff,                     // unset — first bbp_set_bw does the full path
        bulk_budget: BULK_POLL_SPINS, // full budget for bring-up/calibration
        channel_command_path_unusable: false,
        mac_init_command_path_unusable: false,
        cur_channel: 0,
        bss: Vec::new(),
        successful_targeted_tunes: 0,
        deferred_cal_pending: false,
        channel_hops_completed: 0,
    };

    if let Err(reason) = dev.bring_up() {
        return Err((dev.claimed, reason));
    }

    let count = match dev.scan() {
        Ok(n) => n,
        Err(reason) => return Err((dev.claimed, reason)),
    };
    STAGE.store(stage::SCANNED, Ordering::Relaxed);
    crate::serial_println!("mt7601u: scan complete — {} BSSID(s):", count);
    for bss in &dev.bss {
        let b = bss.bssid;
        let ssid = &bss.ssid[..(bss.ssid_len as usize).min(bss.ssid.len())];
        let ssid_str = core::str::from_utf8(ssid).unwrap_or("<non-utf8>");
        crate::serial_println!(
            "mt7601u:   bssid {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} ch={} ssid_len={} ssid=\"{}\" ({})",
            b[0],
            b[1],
            b[2],
            b[3],
            b[4],
            b[5],
            bss.channel,
            bss.ssid_len,
            ssid_str,
            if bss.is_probe_resp {
                "probe-resp"
            } else {
                "beacon"
            },
        );
    }
    crate::log::flush();

    // Bind only scans + parks the device. The association is driven LATER, from
    // the assoc scenario's run fn (`run_configured_association`), once the NVMe
    // log-sink is up — so the auth/assoc/4-way breadcrumbs (and any fault) are
    // CAPTURED, instead of being lost in this pre-sink enumeration path. Scan-only
    // scenarios never call it, so the W1 path is unchanged.
    BOUND.store(true, Ordering::Release);
    let arc = Arc::new(SwitchMutex::new_ordered(
        dev,
        crate::sync::lock_order::USB_DEVICE,
    ));
    *PRIMARY.lock() = Some(arc);
    Ok(())
}

/// W2 ISC-1/4: drive the configured association AFTER bind + the log-sink install
/// (called from the assoc scenario's run fn), so the auth → assoc → 4-way
/// breadcrumbs and any fault are captured. On `Connected`, present the link as a
/// `NetDevice` (DHCP-over-WiFi). No-op if nothing is bound.
#[allow(dead_code)] // assoc-scenario seam (feature-gated consumer)
pub fn run_configured_association() -> AssociationOutcome {
    DHCP_TERMINAL.store(
        link_result_logic::DhcpTerminal::NotRun as u32,
        Ordering::Release,
    );
    DHCP_ASSIGNED_IP.store(0, Ordering::Release);
    DHCP_XID.store(0, Ordering::Release);
    LINK_UP.store(false, Ordering::Release);
    let warm = WARM_RESYNC_STATE.load(Ordering::Relaxed);
    let post_fw = POST_FW_STATE.load(Ordering::Relaxed);
    let wpdma_last = POST_FW_WPDMA_LAST.load(Ordering::Relaxed);
    let tune = TARGET_TUNE_STATE.load(Ordering::Relaxed);
    let post_cal = POST_ASSOC_CAL_STATE.load(Ordering::Relaxed);
    crate::serial_println!(
        "mt7601u: WEDGE-STATE warm_resync={}({}) post_fw={}({}) wpdma_last={:#010x} target_tune={}({}) post_assoc_cal={}({})",
        warm,
        warm_resync_label(warm),
        post_fw,
        post_fw_label(post_fw),
        wpdma_last,
        tune,
        target_tune_label(tune),
        post_cal,
        post_assoc_cal_label(post_cal),
    );
    print_post_fw_budget(retained_post_fw_budget());
    crate::log::flush();
    let Some(arc) = PRIMARY.lock().clone() else {
        crate::serial_println!("mt7601u: assoc — no bound device");
        DHCP_TERMINAL.store(
            link_result_logic::DhcpTerminal::AssociationFailed as u32,
            Ordering::Release,
        );
        crate::log::flush();
        return association_outcome();
    };
    // Retry the full handshake several times within this boot. On real silicon
    // the auth/assoc responses are RF/timing-probabilistic — a single attempt
    // often misses one directed frame and fails, while the AP is present and
    // responsive. Retrying (standard supplicant behavior) reliably reaches the
    // 4-way / Connected within one boot instead of wasting a whole reboot per
    // missed frame. Stops early on success.
    // ── Discriminator (DISPATCH §4): init_cal / eeprom-power / scan all run during
    // pre-sink USB enumeration, so their serial lines are lost when only the NVMe
    // sink is read. Re-emit the TX-relevant state INTO the sink here (the sink is
    // up by now) so a sink-only read is self-diagnosing about whether DPD ran sane
    // and what power the hardware is actually holding for TX. ──
    {
        let mut dev = arc.lock();
        let raw = phy::CAL_RAW_TEMP.load(Ordering::Relaxed);
        let dpd = phy::CAL_DPD_TEMP.load(Ordering::Relaxed);
        if raw == i32::MIN {
            crate::serial_println!("mt7601u: STATE dpd_cal=DID-NOT-RUN");
        } else {
            crate::serial_println!(
                "mt7601u: STATE dpd_cal raw_temp={} ref_temp={} dpd_temp={:#x}",
                raw,
                dev.ee.ref_temp,
                dpd,
            );
        }
        crate::serial_println!(
            "mt7601u: STATE eeprom tssi_en={} chan_pwr={:02x?}",
            dev.ee.tssi_enabled,
            dev.ee.chan_pwr,
        );
        crate::serial_println!("mt7601u: STATE eeprom byrate={:08x?}", dev.ee.tx_pwr_cfg);
        // Actual programmed TX power, read back from the hardware registers — the
        // smoking-gun check: TX_PWR_CFG_0 is CCK power, and our auth frame is CCK
        // 1Mbps. A zero/garbage readback here localizes the TX failure to power.
        let alc0 = dev.rr(regs::TX_ALC_CFG_0);
        let pwr0 = dev.rr(regs::TX_PWR_CFG_0);
        let pwr1 = dev.rr(regs::TX_PWR_CFG_1);
        crate::serial_println!(
            "mt7601u: STATE tx-regs ALC_CFG_0={:#010x} PWR_CFG_0(cck)={:#010x} PWR_CFG_1={:#010x}",
            alc0,
            pwr0,
            pwr1,
        );
        // MAC_SYS_CTRL: confirm TX+RX are still enabled at assoc time (bit0=TX,
        // bit1=RX). RF_PA_MODE: confirm the PA-mode regs are non-zero (PA configured
        // for TX, not idle). A power-independent no-ACK with these wrong ⇒ the TX RF
        // path isn't actually radiating despite valid=true.
        let sysctrl = dev.rr(regs::MAC_SYS_CTRL);
        let pa0 = dev.rr(regs::RF_PA_MODE_CFG0);
        let pa1 = dev.rr(regs::RF_PA_MODE_CFG1);
        crate::serial_println!(
            "mt7601u: STATE rf MAC_SYS_CTRL={:#010x} PA_MODE0={:#010x} PA_MODE1={:#010x}",
            sysctrl,
            pa0,
            pa1,
        );
        crate::serial_println!("mt7601u: STATE scan bss={}", dev.bss.len());
        for b in dev.bss.iter() {
            let s = core::str::from_utf8(&b.ssid[..b.ssid_len as usize]).unwrap_or("<non-utf8>");
            crate::serial_println!(
                "mt7601u: STATE bss ch={} bssid={:02x?} ssid={:?}",
                b.channel,
                b.bssid,
                s,
            );
        }
        crate::log::flush();
    }

    const ASSOC_ATTEMPTS: u32 = 6;
    for attempt in 1..=ASSOC_ATTEMPTS {
        {
            let mut dev = arc.lock();
            crate::serial_println!("mt7601u: assoc attempt {}/{}", attempt, ASSOC_ATTEMPTS);
            crate::log::flush();
            dev.maybe_associate();
        }
        if CONNECTED.load(Ordering::Acquire) {
            break;
        }
    }
    if CONNECTED.load(Ordering::Acquire) {
        let mac = *READBACK_MAC.lock();
        match WifiNetDevice::new(arc, mac) {
            Ok(netdev) => {
                // Keep a handle to pump wlan0 RX, and capture the registered index:
                // the kernel DHCP client targets interface 0 (`DhcpClient::new(0)`),
                // so a non-zero index means the Discover never reaches wlan0.
                let netdev_handle: Arc<dyn NetDevice> = netdev.clone();
                let idx = crate::drivers::net::register_device(netdev);
                crate::serial_println!(
                    "mt7601u: wlan0 registered as net device index={} (DHCP bound to this index)",
                    idx
                );
                // CB-1 ISC-2: DHCP-over-wlan0 — only once the GTK lands (T-043: a
                // broadcast DHCP Discover/Offer can't pass before the group key).
                if !GTK_INSTALLED.load(Ordering::Acquire) {
                    DHCP_TERMINAL.store(
                        link_result_logic::DhcpTerminal::GtkMissing as u32,
                        Ordering::Release,
                    );
                    crate::serial_println!(
                        "mt7601u: wlan0 DHCP NOT started — GTK not installed (broadcast RX off; T-043)"
                    );
                } else {
                    // Bounded DORA poll, conclusive-on-failure (blind-hardware rule):
                    // bind the client, kick the Discover, then pump wlan0 RX THROUGH
                    // the production demux + the net stack, breadcrumbing each DHCP
                    // state transition. CB-1b wired the wire-RX→dhcpv4 bridge, so a
                    // real Offer/Ack off the wire completes DORA (not inject_*); the
                    // timeout breadcrumb maps a stall to the §5 discriminators (no
                    // offer_rx ⇒ broadcast-RX filter/AP-silent; offer but no ack ⇒
                    // TX device / AP).
                    const DORA_POLL_PASSES: u32 = 4000;
                    // CB-1b ISC-2: bind the DHCP client to wlan0's registered index so
                    // the Discover/Request TX on THIS device (not the hardcoded 0).
                    // IF-off: the client's lock and wlan0's TX (OT-96 F1, below).
                    let xid = crate::arch::x86_64::without_interrupts(|| {
                        crate::net::dhcpv4::set_interface_id(idx as u32);
                        crate::net::dhcpv4::start_discover()
                    });
                    DHCP_XID.store(xid, Ordering::Release);
                    DHCP_TERMINAL.store(
                        link_result_logic::DhcpTerminal::Discovering as u32,
                        Ordering::Release,
                    );
                    crate::serial_println!(
                        "mt7601u: wlan0 dhcp discover xid={:#010x} iface={} (GTK installed — broadcast RX on)",
                        xid,
                        idx,
                    );
                    crate::log::flush();
                    let mut last_state =
                        crate::arch::x86_64::without_interrupts(crate::net::dhcpv4::snapshot).state;
                    let mut leased: Option<[u8; 4]> = None;
                    let mut saw_offer = false;
                    // OT-96 F1 (Chad #173, CP-37): wlan0 is registered above, so
                    // `net-rx` and the syscalls take its `dev` mutex IF-off from
                    // here on. A late (hotplug) bind runs this loop in a thread
                    // with IF on, so each pass's steps on the shared locks run
                    // IF-off too, a holder here never preempted while a peer on
                    // this CPU spins; the drain takes at most this many frames
                    // per pass, so the section stays bounded. The delay is IF on.
                    const DORA_FRAMES_PER_PASS: usize = 24;
                    for _ in 0..DORA_POLL_PASSES {
                        let snap = crate::arch::x86_64::without_interrupts(|| {
                            netdev_handle.service();
                            // CB-1b ISC-4: drain received frames through the PRODUCTION
                            // demux so a real Offer/Ack off the wire reaches the client
                            // (offer_rx→request_tx→ack_rx breadcrumbs fire inside). This
                            // is the wire-RX→dhcpv4 path CB-1 lacked — DORA now completes
                            // for real, not via inject_*.
                            for _ in 0..DORA_FRAMES_PER_PASS {
                                let Some(frame) = netdev_handle.rx_take() else {
                                    break;
                                };
                                let _ = crate::net::dhcpv4::demux_eth_rx(frame.as_slice());
                                netdev_handle.rx_recycle(frame);
                            }
                            crate::net::tick();
                            crate::net::dhcpv4::snapshot()
                        });
                        if snap.state != last_state {
                            crate::serial_println!("mt7601u: wlan0 dhcp state -> {:?}", snap.state);
                            last_state = snap.state;
                        }
                        if snap.state == crate::net::dhcpv4::DhcpState::Requesting
                            || snap.offered.is_some()
                        {
                            saw_offer = true;
                        }
                        if let Some(lease) = snap.lease
                            && snap.state == crate::net::dhcpv4::DhcpState::Bound
                        {
                            let terminal = link_result_logic::ack_terminal(lease.ip, true);
                            DHCP_TERMINAL.store(terminal as u32, Ordering::Release);
                            if terminal == link_result_logic::DhcpTerminal::BoundLinkUp {
                                DHCP_ASSIGNED_IP
                                    .store(link_result_logic::pack_ip(lease.ip), Ordering::Release);
                                LINK_UP.store(true, Ordering::Release);
                                leased = Some(lease.ip);
                            }
                            break;
                        }
                        udelay(200);
                    }
                    match leased {
                        Some(ip) => {
                            crate::serial_println!(
                                "mt7601u: wlan0 dhcp ack ip={}.{}.{}.{} — LINK UP",
                                ip[0],
                                ip[1],
                                ip[2],
                                ip[3],
                            );
                        }
                        None => {
                            let terminal = if DHCP_TERMINAL.load(Ordering::Acquire)
                                == link_result_logic::DhcpTerminal::InvalidLease as u32
                            {
                                link_result_logic::DhcpTerminal::InvalidLease
                            } else {
                                link_result_logic::timeout_terminal(saw_offer)
                            };
                            DHCP_TERMINAL.store(terminal as u32, Ordering::Release);
                            crate::serial_println!(
                                "mt7601u: wlan0 dhcp FAIL terminal={} stalled_at={:?} discover_xid={:#010x} iface={} — Selecting⇒no offer_rx (AP silent / broadcast-RX filter T-047); Requesting⇒offer seen, no ack (TX device / AP)",
                                terminal.label(),
                                last_state,
                                xid,
                                idx,
                            );
                        }
                    }
                    crate::log::flush();
                }
            }
            Err(e) => {
                DHCP_TERMINAL.store(
                    link_result_logic::DhcpTerminal::NetDeviceFailed as u32,
                    Ordering::Release,
                );
                crate::serial_println!("mt7601u: NetDevice registration failed: {}", e);
            }
        }
    } else {
        DHCP_TERMINAL.store(
            link_result_logic::DhcpTerminal::AssociationFailed as u32,
            Ordering::Release,
        );
    }
    let outcome = association_outcome();
    crate::serial_println!(
        "mt7601u: LINK-SUMMARY dhcp={} xid={:#010x} ip={}.{}.{}.{} gtk={} link_up={} pass_eligible={}",
        outcome.dhcp.label(),
        outcome.xid,
        outcome.assigned_ip[0],
        outcome.assigned_ip[1],
        outcome.assigned_ip[2],
        outcome.assigned_ip[3],
        outcome.gtk_installed,
        outcome.link_up,
        outcome.pass_eligible(),
    );
    crate::log::flush();
    outcome
}

/// The bound device (held for its lifetime; the scan list lives here).
static PRIMARY: SwitchMutex<Option<Arc<SwitchMutex<Mt7601uDev>>>> = SwitchMutex::new(None);

/// Register the class driver (called from `usb::init` under the feature).
pub fn init() {
    super::register_class_driver(Arc::new(Mt7601uDriver));
}
