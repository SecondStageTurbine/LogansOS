// SPDX-License-Identifier: MPL-2.0
//! The directory trait, shaped like F0's calls (#106, `userland/libneo/src/lib.rs`
//! at `7557b87e`): `sys_readdir`, `sys_stat_at`, `sys_mkdir_at`, `sys_unlink_at`,
//! `sys_rename_at`, and the root-form `sys_open` of one path with given rights
//! (K1b has no `open_at` yet, so a file or folder is opened by its path under
//! Files' scope). F1b's adapter onto libneo is one call per method plus
//! [`FsError::from_errno`]; nothing here is richer than the syscalls.
//!
//! **A handle binds a PATH, not a folder's identity** (Muse on #106,
//! observation 1). The kernel resolves a directory handle's path at each call:
//! after its folder is renamed away the handle is stale, and after a new folder
//! is made at the old path the handle acts on the new one. The fake in the
//! tests behaves the same way, and [`crate::Files`] is written for it.

use alloc::string::String;
use alloc::vec::Vec;

/// A file or folder name's longest length in bytes of UTF-8 (NeoFS
/// `format.rs`, F0's `NAME_MAX`).
pub const NAME_MAX: usize = 47;

/// The most entries a folder holds: one 4 KiB block of 64-byte dirents.
pub const FOLDER_MAX: usize = 64;

/// The largest file Files copies: the ordinary write path's limit.
pub const COPY_MAX: usize = 32 * 1024;

/// What an entry is: the `NeoDirentV1` / `StatAtV1` numbering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Kind {
    Directory,
    Regular,
    Symlink,
    Model,
}

impl Kind {
    /// The kind for the dirent and stat numbering (1..=4).
    pub const fn from_raw(raw: u32) -> Option<Kind> {
        match raw {
            1 => Some(Kind::Directory),
            2 => Some(Kind::Regular),
            3 => Some(Kind::Symlink),
            4 => Some(Kind::Model),
            _ => None,
        }
    }
}

/// One `readdir` record: its name and kind (the dirent has no size).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub kind: Kind,
}

/// `stat_at`'s reply (`libneo::stat_at::StatAt`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stat {
    pub kind: Kind,
    /// A directory with no entries: the one `unlink_at` removes.
    pub empty_directory: bool,
    /// A file's length in bytes; a directory's entry count.
    pub size: u64,
}

/// File rights, by `libneo::FileRights`' bits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rights(pub u32);

impl Rights {
    pub const READ: Rights = Rights(1 << 0);
    pub const WRITE: Rights = Rights(1 << 1);
    pub const LOOKUP: Rights = Rights(1 << 2);
    pub const READDIR: Rights = Rights(1 << 3);

    pub const fn union(self, other: Rights) -> Rights {
        Rights(self.0 | other.0)
    }

    pub const fn contains(self, other: Rights) -> bool {
        self.0 & other.0 == other.0
    }
}

/// The rights Files opens a folder with: list it, look in it, change it.
pub const DIR_RIGHTS: Rights = Rights(Rights::LOOKUP.0 | Rights::READDIR.0 | Rights::WRITE.0);

/// Why a call failed: F0's error set, by the errno each returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FsError {
    /// `ESTALE`, or `ENOTDIR` on a directory handle: the handle's path no
    /// longer names a folder (moved, removed, or replaced by a file).
    Stale,
    /// `EEXIST`: the name is taken (F0 never replaces).
    Exists,
    /// `ENOTEMPTY`: only an empty folder is removed.
    NotEmpty,
    /// The volume has no room. F0 folds this into `EINVAL`; the fake returns
    /// it on its own so the message is tested.
    NoSpace,
    /// `EACCES`, `EPERM`, `EISDIR` (a model or a symlink), `EXDEV`: the
    /// handle or the place doesn't allow it.
    Refused,
    /// A name over [`NAME_MAX`] bytes. F0 returns `EINVAL`; Files checks the
    /// name first, so this comes from the check (and the fake).
    NameTooLong,
    /// A folder at [`FOLDER_MAX`] entries. F0 returns `EINVAL`; Files checks
    /// the listing first, so this comes from the check (and the fake).
    FolderFull,
    /// `ENOENT`: the entry is gone.
    NotFound,
    /// `EBUSY`: an entry the volume needs (the root's `hello.txt`,
    /// `model.neom`).
    Busy,
    /// `EAGAIN`: other commits kept landing; nothing changed.
    Retry,
    /// `EIO`: the device failed.
    Io,
    /// `EINVAL` that Files' own checks didn't predict: too deep, no space, or
    /// a rename into itself.
    Invalid,
    /// Any other errno.
    Other(i64),
}

// The errnos F0 returns (`kernel/src/errno.rs`; `libneo` mirrors them).
const EPERM: i64 = -1;
const ENOENT: i64 = -2;
const EIO: i64 = -5;
const EAGAIN: i64 = -11;
const EACCES: i64 = -13;
const EBUSY: i64 = -16;
const EEXIST: i64 = -17;
const EXDEV: i64 = -18;
const ENOTDIR: i64 = -20;
const EISDIR: i64 = -21;
const EINVAL: i64 = -22;
const ENOTEMPTY: i64 = -39;
const ESTALE: i64 = -116;

impl FsError {
    /// The error for a negative errno from one of F0's calls.
    pub const fn from_errno(errno: i64) -> FsError {
        match errno {
            ESTALE | ENOTDIR => FsError::Stale,
            EEXIST => FsError::Exists,
            ENOTEMPTY => FsError::NotEmpty,
            EACCES | EPERM | EISDIR | EXDEV => FsError::Refused,
            ENOENT => FsError::NotFound,
            EBUSY => FsError::Busy,
            EAGAIN => FsError::Retry,
            EIO => FsError::Io,
            EINVAL => FsError::Invalid,
            e => FsError::Other(e),
        }
    }
}

/// F0's calls, as Files uses them. Paths are the components below Files' own
/// root (its directory cap); the empty path is the root.
pub trait Fs {
    /// A directory handle.
    type Dir;
    /// A file handle.
    type File;

    /// `sys_open(path, 0, rights)` of a folder: a handle that names `path`.
    fn open_dir(&mut self, path: &[String], rights: Rights) -> Result<Self::Dir, FsError>;
    /// One `sys_readdir`: appends this batch to `out` and returns how many it
    /// appended; 0 at the end. `cursor` starts at 0.
    fn readdir(
        &mut self,
        dir: &Self::Dir,
        cursor: &mut u64,
        out: &mut Vec<DirEntry>,
    ) -> Result<usize, FsError>;
    /// `sys_stat_at`.
    fn stat_at(&mut self, dir: &Self::Dir, name: &str) -> Result<Stat, FsError>;
    /// `sys_mkdir_at`.
    fn mkdir_at(&mut self, dir: &Self::Dir, name: &str) -> Result<(), FsError>;
    /// `sys_unlink_at`: a file, or an empty folder.
    fn unlink_at(&mut self, dir: &Self::Dir, name: &str) -> Result<(), FsError>;
    /// `sys_rename_at`; never replaces.
    fn rename_at(
        &mut self,
        dir: &Self::Dir,
        name: &str,
        to: &Self::Dir,
        to_name: &str,
    ) -> Result<(), FsError>;
    /// `sys_open(path, OPEN_CREATE_REGULAR if create, rights)` of one file.
    fn open_file(
        &mut self,
        path: &[String],
        create: bool,
        rights: Rights,
    ) -> Result<Self::File, FsError>;
    /// `sys_read`: bytes read, 0 at the end.
    fn read(&mut self, file: &Self::File, buf: &mut [u8]) -> Result<usize, FsError>;
    /// `sys_write` through the ordinary (32 KiB) path: bytes written.
    fn write(&mut self, file: &Self::File, buf: &[u8]) -> Result<usize, FsError>;
    /// `sys_close` of a folder.
    fn close_dir(&mut self, dir: Self::Dir);
    /// `sys_close` of a file.
    fn close_file(&mut self, file: Self::File);
}
