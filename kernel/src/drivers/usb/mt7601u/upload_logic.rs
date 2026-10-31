// SPDX-License-Identifier: MPL-2.0
//! Pure, host-testable cold-firmware-upload decision logic (Phase 7.5 OT-27a,
//! ISC-1/ISC-3 + BUILD-2 C1/C2): the all-ones-aware poll predicate and the
//! blind-hardware fault classifier that make a cold-boot self-upload failure
//! *self-localize*, from the breadcrumbs the device-touching loader (`mod.rs`
//! `load_firmware`) captures.
//!
//! This is the THINK §2 P2/P6 discriminator bank rendered as code: the loader
//! records what it observed (blob valid, chip-id, byte-progress, COM_REG0
//! progression, **whether the device fell off the bus**) and hands it here; a
//! Stage-B failure then names its cause instead of being a blind stall — the
//! standing-rule contract that every blind run be conclusive on failure.
//!
//! **BUILD-2 (un-mask the wedge):** BUILD-1's Stage-B reported a *completed*
//! upload + `McuNoBoot`, but the post-upload reads were `0xffffffff` and the
//! `dma_fw` poll FALSE-PASSED on all-ones — hiding *where* the device wedged.
//! [`poll_step`] (C1) names the all-ones sentinel, and [`ColdUploadFault::DeviceUnresponsive`]
//! (C2) is the distinct bucket for it.
//!
//! NO kernel deps — path-mod'd into `tools/ai-ops-tests` and asserted directly
//! (the `*_logic` discipline). The actual cold upload is the bare-metal Stage-B
//! (QEMU has no MT7601U model), so this pure core is the host-side gate for the
//! poll/retry/classification logic.

#![allow(dead_code)] // host-oracle'd core; the kernel loader is the consumer

/// The all-ones MMIO sentinel: the MT7601U's vendor control-read returns this
/// when the chip has fallen off the bus (a failed/timed-out control transfer).
pub const BUS_DROP_SENTINEL: u32 = 0xffff_ffff;

/// The decision for one poll read in the cold-upload path (BUILD-2 C1).
///
/// The old `value & mask == expected` poll would FALSE-PASS the all-ones
/// sentinel for any mask whose bits are a subset of `0xffffffff` (e.g. the
/// `dma_fw` DMA-done poll on `COM_REG1 & BIT(31)`), so a wedged device looked
/// like a *completed* upload. This predicate names that case instead.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PollStep {
    /// `0xffffffff` — the device is unresponsive (bus drop). Fail fast at the
    /// point of the read so the breadcrumb captures *where* it went away.
    Unresponsive,
    /// `value & mask == expected` — the awaited condition is met.
    Done,
    /// Neither — keep polling until the timeout.
    Retry,
}

/// Classify one poll read (C1). **`0xffffffff` ⇒ `Unresponsive` takes precedence
/// over the match**, so a wedged device can never false-pass a `BIT(31)`-style
/// mask. No legitimate poll in the cold-upload path awaits all-ones (`COM_REG0`
/// awaits `1`; `COM_REG1` awaits `BIT(31)` with the other bits clear), so this is
/// safe for every cold-upload poll.
pub fn poll_step(value: u32, mask: u32, expected: u32) -> PollStep {
    if value == BUS_DROP_SENTINEL {
        PollStep::Unresponsive
    } else if value & mask == expected {
        PollStep::Done
    } else {
        PollStep::Retry
    }
}

/// The five distinct ways a cold firmware self-upload fails, each mapped to a
/// captured breadcrumb so a blind Stage-B run is conclusive on failure.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ColdUploadFault {
    /// The vendor blob was absent (Limine module not staged) or failed
    /// header/size validation. (In the loader this short-circuits earlier with
    /// its own `firmware_blob()` / `validate` error; the classifier keeps the
    /// bucket so the logic is complete and host-testable.)
    BlobMissingOrCorrupt,
    /// The chip-id / eFUSE didn't match the MT7601U (same USB-ID, different
    /// silicon — the "vendor could not connect" class). `bring_up` rejects this
    /// before the upload; recorded here for completeness.
    WrongVariant,
    /// The device fell off the bus mid-sequence — an all-ones (`0xffffffff`)
    /// read or a control transfer with no event (BUILD-2 C2). Distinct from
    /// `McuNoBoot`: the chip is *gone*, not "alive but the firmware didn't run."
    /// This is the BUILD-1-on-silicon signature, formerly masked as `McuNoBoot`.
    DeviceUnresponsive,
    /// A USB control/bulk transfer stalled mid-upload while the device stayed
    /// responsive — not all bytes egressed (`bytes_uploaded < bytes_total`).
    EndpointStall,
    /// Every byte uploaded and the device stayed responsive, but the MCU never
    /// latched `COM_REG0 == 1` — the firmware didn't boot (honest no-boot, not a
    /// bus drop).
    McuNoBoot,
}

/// Captured cold-upload breadcrumbs (ISC-3, capture-before-you-need-it).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ColdUploadProgress {
    /// The blob was found AND passed header/size validation.
    pub blob_valid: bool,
    /// The chip-id / asic-rev read back matched the MT7601U signature.
    pub chip_id_ok: bool,
    /// Total firmware bytes the upload intended to push (ILM data + DLM + IVB).
    pub bytes_total: u32,
    /// The furthest any attempt's upload actually egressed.
    pub bytes_uploaded: u32,
    /// An all-ones read or a control-no-event was observed — the device fell off
    /// the bus (BUILD-2 C2). The dominant failure signal when set.
    pub device_unresponsive: bool,
    /// `COM_REG0` was ever observed at a *genuine* nonzero value (not the
    /// all-ones sentinel) — the MCU started to stir.
    pub com_reg0_ever_nonzero: bool,
    /// `COM_REG0` reached the running value (`== 1`) — i.e. the upload succeeded.
    pub com_reg0_running: bool,
}

impl ColdUploadProgress {
    /// All intended bytes egressed (and there was something to upload).
    pub fn upload_complete(&self) -> bool {
        self.bytes_total > 0 && self.bytes_uploaded >= self.bytes_total
    }
}

/// Bucket a cold-upload failure from the captured breadcrumbs.
///
/// Returns `None` iff the upload actually succeeded (`com_reg0_running`).
/// Otherwise the *most-specific root cause wins*, in order — so the bucket names
/// the cause, not a downstream symptom (e.g. a bus drop reads as
/// `DeviceUnresponsive`, not the `EndpointStall`/`McuNoBoot` it *also* produces):
///   1. blob invalid              → [`ColdUploadFault::BlobMissingOrCorrupt`]
///   2. wrong silicon             → [`ColdUploadFault::WrongVariant`]
///   3. device fell off the bus   → [`ColdUploadFault::DeviceUnresponsive`]
///   4. upload didn't finish      → [`ColdUploadFault::EndpointStall`]
///   5. finished but no boot      → [`ColdUploadFault::McuNoBoot`]
pub fn classify_cold_upload_failure(p: &ColdUploadProgress) -> Option<ColdUploadFault> {
    if p.com_reg0_running {
        return None; // success — nothing to classify
    }
    if !p.blob_valid {
        return Some(ColdUploadFault::BlobMissingOrCorrupt);
    }
    if !p.chip_id_ok {
        return Some(ColdUploadFault::WrongVariant);
    }
    if p.device_unresponsive {
        // All-ones / bus-drop is the dominant signal: it's *why* the upload
        // didn't finish or the MCU "didn't boot". Rank it above both.
        return Some(ColdUploadFault::DeviceUnresponsive);
    }
    if !p.upload_complete() {
        return Some(ColdUploadFault::EndpointStall);
    }
    // Blob valid, right chip, responsive, every byte egressed, but the MCU never
    // latched COM_REG0 == 1 → the firmware didn't boot (honest no-boot).
    Some(ColdUploadFault::McuNoBoot)
}
