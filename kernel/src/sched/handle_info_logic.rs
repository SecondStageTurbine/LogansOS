// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.1: the `SYS_HANDLE_INFO` record, kept kernel-free for the host
//! KAT (`tools/handle-info-logic-tests`, which checks it against hand-laid
//! bytes, not against this encoder).
//!
//! `SYS_HANDLE_INFO(idx, out, len)` tells a process what its own handle table
//! holds at `idx`, so a program spawned with inherited handles can find the
//! sealed resources it was given. It reports and grants nothing: the record is
//! the handle's kind and rights, never a capability.
//!
//! **Record v1, 16 bytes, little-endian:**
//!
//! | offset | size | field |
//! |---|---|---|
//! | 0 | 4 | `version` = 1 |
//! | 4 | 4 | `kind`, one of the `KIND_*` codes (stable; never renumbered) |
//! | 8 | 4 | `file_rights`: `FileRights` bits for a `File` handle, else 0 |
//! | 12 | 1 | `cap_meta`: `CapRights.meta` |
//! | 13 | 1 | `cap_semantic`: `CapRights.semantic` |
//! | 14 | 1 | `flags`: [`FLAG_ROOT_DIR`] |
//! | 15 | 1 | reserved, 0 |

/// `SYS_HANDLE_INFO`'s record length (v1).
pub const RECORD_LEN: usize = 16;
/// The record version this kernel writes.
pub const RECORD_VERSION: u32 = 1;

pub const KIND_CHANNEL: u32 = 1;
pub const KIND_PROCESS: u32 = 2;
pub const KIND_FILE: u32 = 3;
pub const KIND_SOCKET: u32 = 4;
pub const KIND_TCP_SOCKET: u32 = 5;
pub const KIND_TCP_LISTENER: u32 = 6;
pub const KIND_NET_DIAL: u32 = 7;
pub const KIND_TENSOR: u32 = 8;
pub const KIND_MODEL: u32 = 9;
pub const KIND_BUFFER: u32 = 10;
pub const KIND_INPUT_FOCUS: u32 = 11;
pub const KIND_GPU_SUBMIT: u32 = 12;
pub const KIND_INPUT_SYSTEM: u32 = 13;
pub const KIND_GPU_COMPUTE: u32 = 14;
pub const KIND_BROWSER_RUNTIME: u32 = 15;
pub const KIND_INSTALLER: u32 = 16;
pub const KIND_SYSTEM_STORE: u32 = 17;
pub const KIND_MOUNT_AUTHORITY: u32 = 18;
pub const KIND_POWER_CONTROL: u32 = 19;
/// L1a.1 step 2a: one end of a byte pipe (`pipe_rights` say which end).
pub const KIND_PIPE: u32 = 20;

/// `flags` bit: a `File` handle on the VFS root, a RootDirCap (OT-66 K1a).
pub const FLAG_ROOT_DIR: u8 = 1 << 0;

/// One handle's record, before encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandleInfo {
    pub kind: u32,
    pub file_rights: u32,
    pub cap_meta: u8,
    pub cap_semantic: u8,
    pub flags: u8,
}

/// The v1 record for `info`.
pub fn encode(info: HandleInfo) -> [u8; RECORD_LEN] {
    let mut out = [0u8; RECORD_LEN];
    out[0..4].copy_from_slice(&RECORD_VERSION.to_le_bytes());
    out[4..8].copy_from_slice(&info.kind.to_le_bytes());
    out[8..12].copy_from_slice(&info.file_rights.to_le_bytes());
    out[12] = info.cap_meta;
    out[13] = info.cap_semantic;
    out[14] = info.flags;
    out
}
