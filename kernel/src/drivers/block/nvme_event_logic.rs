// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.1 fix (Muse's R4.1 review, item 4): what an NVMe controller
//! reports that can mean a different medium, decided purely.
//!
//! Host-tested by `tools/install-claim-logic-tests` through `#[path]` (a
//! model controller drives these functions the way `nvme.rs` does), so keep
//! it free of `crate::` paths and feature cfgs. The field layouts below were
//! checked against SPDK's `include/spdk/nvme_spec.h` (BSD-3-Clause), read raw.
//!
//! - [`namespace_ids`]: the namespace's NGUID and EUI64 (Identify Namespace
//!   bytes 104..120 and 120..128), part of the claim's identity, so a twin
//!   namespace with the same serial, model and capacity is not the same disk.
//! - [`AerResult`]: an Asynchronous Event Request completion. Every one is a
//!   media change for the claim: namespace-attribute notices are the case the
//!   claim needs, and any other (or an error completion: the request was
//!   aborted, the controller is going away) is treated the same, fail-closed.
//! - [`register_verdict`]: the controller's registers against the values the
//!   driver programmed. A reset the driver didn't make leaves CC.EN clear and
//!   CSTS.RDY clear until host software enables it again, and the driver never
//!   does after its probe; a function or conventional reset also clears the
//!   admin queue registers; an NVM subsystem reset leaves CSTS.NSSRO set.

/// Admin opcodes.
pub const OPC_GET_LOG_PAGE: u8 = 0x02;
pub const OPC_ASYNC_EVENT_REQUEST: u8 = 0x0c;
/// Namespace Attachment (the scenario's detach).
pub const OPC_NAMESPACE_ATTACHMENT: u8 = 0x15;
/// Feature identifier: Asynchronous Event Configuration.
pub const FEATURE_ASYNC_EVENT_CONFIG: u32 = 0x0b;
/// AEC bit 8: Namespace Attribute Notices. Bits 7:0 (SMART critical
/// warnings) stay clear: the claim needs only medium changes.
pub const AEC_NS_ATTR_NOTICES: u32 = 1 << 8;
/// Log identifier 04h: the Changed Namespace List. Reading it (with RAE
/// clear) clears the notice, so the controller can send the next one.
pub const LOG_CHANGED_NS_LIST: u8 = 0x04;
/// Every log page the driver reads is one 4 KiB page.
pub const LOG_BYTES: usize = 4096;

pub const AER_TYPE_NOTICE: u8 = 0x2;
pub const AER_INFO_NS_ATTR_CHANGED: u8 = 0x00;

/// CSTS bits.
pub const CSTS_RDY: u32 = 1 << 0;
pub const CSTS_CFS: u32 = 1 << 1;
/// CSTS.SHST (bits 3:2): a shutdown is in progress or complete.
pub const CSTS_SHST: u32 = 0b11 << 2;
pub const CSTS_NSSRO: u32 = 1 << 4;

/// The namespace identifiers: NGUID (16 bytes) then EUI64 (8 bytes), as
/// Identify Namespace stores them. `None` when the buffer is too short.
pub fn namespace_ids(identify_namespace: &[u8]) -> Option<[u8; 24]> {
    let ids = identify_namespace.get(104..128)?;
    let mut out = [0u8; 24];
    out.copy_from_slice(ids);
    Some(out)
}

/// One Asynchronous Event Request completion's Dword 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AerResult {
    /// Bits 2:0.
    pub event_type: u8,
    /// Bits 15:8.
    pub info: u8,
    /// Bits 23:16: the log page that clears it.
    pub log_page: u8,
}

impl AerResult {
    pub fn decode(dw0: u32) -> Self {
        Self {
            event_type: (dw0 & 0x7) as u8,
            info: ((dw0 >> 8) & 0xff) as u8,
            log_page: ((dw0 >> 16) & 0xff) as u8,
        }
    }

    pub fn namespace_attribute_changed(&self) -> bool {
        self.event_type == AER_TYPE_NOTICE && self.info == AER_INFO_NS_ATTR_CHANGED
    }
}

/// What the driver does with one AER completion, `status` 0 or not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AerAction {
    /// Bump the media generation: always (see the module doc).
    pub reason: &'static str,
    /// Read this log page (RAE clear) before arming the next request, so the
    /// controller unmasks the event. `None` after an error completion.
    pub read_log: Option<u8>,
    /// Post a new Asynchronous Event Request. Not after an error completion:
    /// the controller refused or aborted the request, and the claim has
    /// already been told.
    pub rearm: bool,
}

pub fn aer_action(status: u32, dw0: u32) -> AerAction {
    if status != 0 {
        return AerAction {
            reason: "nvme_aer_error",
            read_log: None,
            rearm: false,
        };
    }
    let r = AerResult::decode(dw0);
    AerAction {
        reason: if r.namespace_attribute_changed() {
            "nvme_namespace_attribute_changed"
        } else {
            "nvme_async_event"
        },
        read_log: Some(r.log_page),
        rearm: true,
    }
}

/// Get Log Page Command Dword 10 for `log` of [`LOG_BYTES`]: the log
/// identifier, RAE clear (bit 15), and NUMDL, the number of dwords minus one.
pub fn get_log_page_cdw10(log: u8) -> u32 {
    let numd = (LOG_BYTES / 4 - 1) as u32;
    ((numd & 0xffff) << 16) | log as u32
}

/// The controller registers `media_state` reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Regs {
    pub cc: u32,
    pub csts: u32,
    pub aqa: u32,
    pub asq: u64,
    pub acq: u64,
}

/// Why the controller is no longer the one the driver set up, or `None`.
/// `ours` is what the driver programmed (CC, AQA, ASQ, ACQ) at probe.
pub fn register_verdict(now: Regs, ours: Regs) -> Option<&'static str> {
    if now.csts == u32::MAX {
        // Gone from the bus: reads return all ones.
        return Some("nvme_csts_all_ones");
    }
    if now.csts & CSTS_CFS != 0 {
        return Some("nvme_csts_cfs");
    }
    if now.csts & CSTS_NSSRO != 0 {
        return Some("nvme_csts_nssro");
    }
    if now.csts & CSTS_RDY == 0 {
        return Some("nvme_csts_not_ready");
    }
    if now.csts & CSTS_SHST != 0 {
        return Some("nvme_csts_shutdown");
    }
    if now.cc != ours.cc {
        // CC.EN cleared (a controller reset), CC.SHN set, or reprogrammed.
        return Some("nvme_cc_changed");
    }
    if now.aqa != ours.aqa || now.asq != ours.asq || now.acq != ours.acq {
        // A function-level or conventional reset clears them; nothing but
        // the driver's probe writes them.
        return Some("nvme_admin_queue_registers_changed");
    }
    None
}
