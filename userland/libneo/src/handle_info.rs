// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.1: decoding the `SYS_HANDLE_INFO` record (see
//! `kernel/src/sched/handle_info_logic.rs` for the layout). Written
//! independently of the kernel's encoder; `tools/handle-info-logic-tests`
//! checks both against hand-laid bytes.

/// The v1 record length.
pub const RECORD_LEN: usize = 16;

/// `flags` bit: a file handle on the VFS root, a RootDirCap (OT-66 K1a).
pub const FLAG_ROOT_DIR: u8 = 1 << 0;

/// A handle's kind, by the record's stable code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleKind {
    Channel,
    Process,
    File,
    Socket,
    TcpSocket,
    TcpListener,
    NetDial,
    Tensor,
    Model,
    Buffer,
    InputFocus,
    GpuSubmit,
    InputSystem,
    GpuCompute,
    BrowserRuntime,
    /// OT-87 R4.4: the installer authority.
    Installer,
    /// OT-66 L1: the system-store authority (the session root's).
    SystemStore,
    /// Dexter #187 item 3a: the mount authority (the session root's).
    MountAuthority,
    /// OT-90 P4: the power-off and restart authority (the session root's).
    PowerControl,
    /// L1a.1 step 2a: one end of a byte pipe (its rights say which end).
    Pipe,
    /// A code this decoder does not know (a newer kernel).
    Unknown(u32),
}

impl HandleKind {
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::Channel,
            2 => Self::Process,
            3 => Self::File,
            4 => Self::Socket,
            5 => Self::TcpSocket,
            6 => Self::TcpListener,
            7 => Self::NetDial,
            8 => Self::Tensor,
            9 => Self::Model,
            10 => Self::Buffer,
            11 => Self::InputFocus,
            12 => Self::GpuSubmit,
            13 => Self::InputSystem,
            14 => Self::GpuCompute,
            15 => Self::BrowserRuntime,
            16 => Self::Installer,
            17 => Self::SystemStore,
            18 => Self::MountAuthority,
            19 => Self::PowerControl,
            20 => Self::Pipe,
            other => Self::Unknown(other),
        }
    }
}

/// One handle's kind and rights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandleInfo {
    pub kind: HandleKind,
    /// `FileRights` bits for a `File` handle, else 0.
    pub file_rights: u32,
    pub cap_meta: u8,
    pub cap_semantic: u8,
    pub flags: u8,
}

impl HandleInfo {
    /// Decode a v1 record; `None` for any other version.
    pub fn decode(bytes: &[u8; RECORD_LEN]) -> Option<Self> {
        let word =
            |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        if word(0) != 1 {
            return None;
        }
        Some(Self {
            kind: HandleKind::from_code(word(4)),
            file_rights: word(8),
            cap_meta: bytes[12],
            cap_semantic: bytes[13],
            flags: bytes[14],
        })
    }
}
