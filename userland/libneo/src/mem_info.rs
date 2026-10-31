// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.2: decoding the `SYS_MEM_INFO` record (see
//! `kernel/src/mm/mem_account_logic.rs` for the layout). Written
//! independently of the kernel's encoder; `tools/mem-account-logic-tests`
//! checks both against hand-laid bytes.

/// The v1 record length.
pub const RECORD_LEN: usize = 32;

/// No limit.
pub const UNLIMITED: u64 = u64::MAX;

/// `flags` bit: the process was spawned with its own budget.
pub const FLAG_OWN_BUDGET: u32 = 1 << 0;

/// The caller's memory account, in 4 KiB frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemInfo {
    pub flags: u32,
    pub used_frames: u64,
    pub limit_frames: u64,
    /// Frames in use system-wide (every process and the kernel).
    pub global_used_frames: u64,
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16 | u32::from(b[3]) << 24
}

fn le_u64(b: &[u8]) -> u64 {
    u64::from(le_u32(&b[0..4])) | u64::from(le_u32(&b[4..8])) << 32
}

/// Decode a v1 record; `None` for another version.
pub fn decode(rec: &[u8; RECORD_LEN]) -> Option<MemInfo> {
    if le_u32(&rec[0..4]) != 1 {
        return None;
    }
    Some(MemInfo {
        flags: le_u32(&rec[4..8]),
        used_frames: le_u64(&rec[8..16]),
        limit_frames: le_u64(&rec[16..24]),
        global_used_frames: le_u64(&rec[24..32]),
    })
}

/// OT-61 C2.2b: the v2 record length (v1 plus the object counter).
pub const RECORD_V2_LEN: usize = 48;

/// The caller's account with its object counter (bytes of declared
/// kernel-heap weight).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemInfoV2 {
    pub frames: MemInfo,
    pub obj_used_bytes: u64,
    pub obj_limit_bytes: u64,
}

/// Decode a v2 record; `None` for another version.
pub fn decode_v2(rec: &[u8; RECORD_V2_LEN]) -> Option<MemInfoV2> {
    if le_u32(&rec[0..4]) != 2 {
        return None;
    }
    Some(MemInfoV2 {
        frames: MemInfo {
            flags: le_u32(&rec[4..8]),
            used_frames: le_u64(&rec[8..16]),
            limit_frames: le_u64(&rec[16..24]),
            global_used_frames: le_u64(&rec[24..32]),
        },
        obj_used_bytes: le_u64(&rec[32..40]),
        obj_limit_bytes: le_u64(&rec[40..48]),
    })
}
