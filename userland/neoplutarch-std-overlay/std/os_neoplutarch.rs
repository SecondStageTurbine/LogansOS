// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.1b — `std::os::neoplutarch`: LogansOS raw handles and the
//! process's own handle table.
//!
//! Overlaid into `library/std/src/os/neoplutarch/mod.rs` (declared from
//! `os/mod.rs` under `#[cfg(target_os = "neoplutarch")]`). A sealed program
//! receives everything it may touch as handles at spawn; [`handles`] tells it
//! what it holds (kind and rights, never the object), and
//! [`io::FromRawHandle`] wraps an inherited file handle as a `std::fs::File`.
//! Nothing here creates authority: it reads the caller's own table, and a
//! `File` made from a raw handle has exactly that handle's rights.
//!
//! Use needs `#![feature(neoplutarch_ext)]`.

#![unstable(feature = "neoplutarch_ext", issue = "none")]

/// Raw handles: the LogansOS analogue of `std::os::fd`.
#[unstable(feature = "neoplutarch_ext", issue = "none")]
pub mod io {
    use crate::sys::{AsInner, FromInner, IntoInner};

    /// A handle index in this process's table.
    pub type RawHandle = u32;

    /// Borrow the raw handle of an object.
    pub trait AsRawHandle {
        fn as_raw_handle(&self) -> RawHandle;
    }

    /// Take ownership of a raw handle.
    pub trait FromRawHandle {
        /// # Safety
        /// `handle` must be an open handle of the right kind that nothing
        /// else owns: the returned object closes it when dropped.
        unsafe fn from_raw_handle(handle: RawHandle) -> Self;
    }

    /// Release ownership of a raw handle without closing it.
    pub trait IntoRawHandle {
        fn into_raw_handle(self) -> RawHandle;
    }

    impl AsRawHandle for crate::fs::File {
        fn as_raw_handle(&self) -> RawHandle {
            self.as_inner().as_raw()
        }
    }

    impl FromRawHandle for crate::fs::File {
        unsafe fn from_raw_handle(handle: RawHandle) -> Self {
            crate::fs::File::from_inner(crate::sys::fs::File::from_raw(handle))
        }
    }

    impl IntoRawHandle for crate::fs::File {
        fn into_raw_handle(self) -> RawHandle {
            self.into_inner().into_raw()
        }
    }
}

use self::io::RawHandle;
use crate::sys::neoplutarch_rt as rt;

/// A handle's kind, by the kernel's stable `SYS_HANDLE_INFO` code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
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
    /// A code this std does not know (a newer kernel).
    Unknown(u32),
}

impl HandleKind {
    fn from_code(code: u32) -> Self {
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
            other => Self::Unknown(other),
        }
    }
}

/// What one of this process's handles is, and what it may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandleInfo {
    pub kind: HandleKind,
    /// `FileRights` bits for a file handle, else 0.
    pub file_rights: u32,
    pub cap_meta: u8,
    pub cap_semantic: u8,
    /// A file handle on "/", a RootDirCap: `std::fs` opens through it.
    pub root_dir: bool,
}

/// The record of handle `handle`, or the kernel's error (`InvalidInput` for an
/// empty slot, `Unsupported` on a kernel without `SYS_HANDLE_INFO`).
pub fn handle_info(handle: RawHandle) -> crate::io::Result<HandleInfo> {
    let mut rec = [0u8; 16];
    let r = rt::handle_info(handle, &mut rec);
    if r < 0 {
        return Err(crate::io::Error::from_raw_os_error((-r) as i32));
    }
    let word = |i: usize| u32::from_le_bytes([rec[i], rec[i + 1], rec[i + 2], rec[i + 3]]);
    if word(0) != 1 {
        return Err(crate::io::const_error!(
            crate::io::ErrorKind::InvalidData,
            "SYS_HANDLE_INFO record version is not 1"
        ));
    }
    Ok(HandleInfo {
        kind: HandleKind::from_code(word(4)),
        file_rights: word(8),
        cap_meta: rec[12],
        cap_semantic: rec[13],
        root_dir: rec[14] & rt::HANDLE_FLAG_ROOT_DIR != 0,
    })
}

/// Every handle this process holds, in index order.
pub fn handles() -> impl Iterator<Item = (RawHandle, HandleInfo)> {
    (0..rt::HANDLE_SCAN).filter_map(|h| handle_info(h).ok().map(|info| (h, info)))
}
