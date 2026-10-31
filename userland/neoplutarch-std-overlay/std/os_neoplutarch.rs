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

/// OT-61 rung 4 S3: Atrium shared pixel buffers, so a browser content process
/// can render into memory the compositor reads.
///
/// The process creates a buffer (its own memory, charged to its account; no
/// capability needed), maps it read-write, and renders. The compositor gets a
/// *derived* handle with at most `MAP_READ | QUERY | PRESENT`, which maps the
/// same frames read-only. Presenting to the screen is never the client's: an
/// armed scanout gate refuses it without a `GpuSubmitCap` (OT-52).
#[unstable(feature = "neoplutarch_ext", issue = "none")]
pub mod atrium {
    use super::io::RawHandle;
    use super::rt;
    use crate::io;

    /// DRM fourcc `XR24`: little-endian XRGB8888, the one Atrium format.
    pub const FORMAT_XRGB8888_LE: u32 = 0x3432_5258;

    /// A buffer handle's semantic rights (the kernel's
    /// `atrium::logic::buffer_rights`).
    pub mod rights {
        pub const MAP_WRITE: u8 = 1 << 0;
        pub const MAP_READ: u8 = 1 << 1;
        pub const QUERY: u8 = 1 << 2;
        pub const PRESENT: u8 = 1 << 3;
        pub const COMMIT: u8 = 1 << 4;
    }

    /// The `USE` meta right a derived handle needs to be used at all.
    pub const META_USE: u8 = 1 << 0;
    /// The meta right to derive narrower handles from this one.
    pub const META_DERIVE: u8 = 1 << 1;
    /// The meta right to grant this handle over a channel.
    pub const META_GRANT: u8 = 1 << 2;

    /// `BufferDescV1`: the buffer's geometry (24 bytes on the wire).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct BufferDesc {
        pub width: u32,
        pub height: u32,
        /// Bytes per row: at least `width * 4`, a multiple of 4.
        pub stride: u32,
        pub format: u32,
        /// Total bytes: `stride * height`.
        pub bytes: u64,
    }

    impl BufferDesc {
        /// An XRGB8888 buffer `width` x `height` with rows `stride` bytes apart.
        pub fn xrgb(width: u32, height: u32, stride: u32) -> Self {
            Self {
                width,
                height,
                stride,
                format: FORMAT_XRGB8888_LE,
                bytes: stride as u64 * height as u64,
            }
        }

        fn encode(&self) -> [u8; 24] {
            let mut out = [0u8; 24];
            out[0..4].copy_from_slice(&self.width.to_le_bytes());
            out[4..8].copy_from_slice(&self.height.to_le_bytes());
            out[8..12].copy_from_slice(&self.stride.to_le_bytes());
            out[12..16].copy_from_slice(&self.format.to_le_bytes());
            out[16..24].copy_from_slice(&self.bytes.to_le_bytes());
            out
        }

        fn decode(b: &[u8; 24]) -> Self {
            let u = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
            let mut q = [0u8; 8];
            q.copy_from_slice(&b[16..24]);
            Self { width: u(0), height: u(4), stride: u(8), format: u(12), bytes: u64::from_le_bytes(q) }
        }
    }

    /// Which view a mapping is: the client's read-write one, or the
    /// compositor's read-only one.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Role {
        Client = 0,
        Compositor = 1,
    }

    fn check(r: i64) -> io::Result<i64> {
        if r < 0 { Err(io::Error::from_raw_os_error((-r) as i32)) } else { Ok(r) }
    }

    /// Create a buffer; returns its root handle (all buffer rights).
    pub fn create(desc: BufferDesc) -> io::Result<RawHandle> {
        check(rt::buffer_create(&desc.encode())).map(|h| h as RawHandle)
    }

    /// Map the buffer behind `handle` in `role`'s view; returns the base.
    pub fn map(handle: RawHandle, role: Role) -> io::Result<*mut u8> {
        check(rt::buffer_map(handle, role as u32)).map(|base| base as usize as *mut u8)
    }

    /// Remove a mapping made by [`map`].
    ///
    /// # Safety
    /// Nothing may use the mapping after this returns.
    pub unsafe fn unmap(base: *mut u8) -> io::Result<()> {
        check(rt::buffer_unmap(base as usize as u64)).map(|_| ())
    }

    /// The buffer's geometry.
    pub fn query(handle: RawHandle) -> io::Result<BufferDesc> {
        let mut out = [0u8; 24];
        check(rt::buffer_query(handle, &mut out))?;
        Ok(BufferDesc::decode(&out))
    }

    /// Publish one damaged rectangle for the compositor.
    pub fn commit_damage(handle: RawHandle, x: u16, y: u16, width: u16, height: u16) -> io::Result<()> {
        let mut rect = [0u8; 8];
        rect[0..2].copy_from_slice(&x.to_le_bytes());
        rect[2..4].copy_from_slice(&y.to_le_bytes());
        rect[4..6].copy_from_slice(&width.to_le_bytes());
        rect[6..8].copy_from_slice(&height.to_le_bytes());
        check(rt::buffer_commit_damage(handle, &rect)).map(|_| ())
    }

    /// A new handle to the same buffer with at most `meta` and `semantic`
    /// rights (the kernel refuses anything wider than `handle`'s own).
    pub fn derive(handle: RawHandle, meta: u8, semantic: u8) -> io::Result<RawHandle> {
        check(rt::derive(handle, meta, semantic)).map(|h| h as RawHandle)
    }
}

/// OT-61 rung 4 S3b: the rendezvous channels a launcher hands a process, so
/// a browser process can give the compositor its read-only buffer view.
///
/// Every call parks until the peer meets it (the kernel's rendezvous IPC).
/// A process holds no channel unless it was given one: these reach only the
/// peers its launcher connected it to.
#[unstable(feature = "neoplutarch_ext", issue = "none")]
pub mod channel {
    use super::io::RawHandle;
    use super::rt;
    use crate::io;

    fn check(r: i64) -> io::Result<i64> {
        if r < 0 { Err(io::Error::from_raw_os_error((-r) as i32)) } else { Ok(r) }
    }

    fn pack(meta: u8, semantic: u8) -> u16 {
        (meta as u16) << 8 | semantic as u16
    }

    /// Send `msg` (not empty) to the peer; returns once the peer has it.
    pub fn send(channel: RawHandle, msg: &[u8]) -> io::Result<usize> {
        check(rt::send(channel, msg)).map(|n| n as usize)
    }

    /// Receive one message into `buf` (not empty); returns its length. A
    /// longer message is cut to `buf.len()`.
    pub fn recv(channel: RawHandle, buf: &mut [u8]) -> io::Result<usize> {
        check(rt::recv(channel, buf)).map(|n| n as usize)
    }

    /// Give the peer a copy of `handle` with at most `meta`/`semantic`
    /// rights. `handle` needs the `GRANT` meta right; the peer gets the
    /// intersection of these, `handle`'s own, and what it asked for.
    pub fn grant(channel: RawHandle, handle: RawHandle, meta: u8, semantic: u8) -> io::Result<()> {
        check(rt::grant(channel, handle, pack(meta, semantic))).map(|_| ())
    }

    /// Receive a granted handle, keeping at most `meta`/`semantic` rights.
    pub fn recv_grant(channel: RawHandle, meta: u8, semantic: u8) -> io::Result<RawHandle> {
        check(rt::recv_grant(channel, pack(meta, semantic))).map(|h| h as RawHandle)
    }
}
