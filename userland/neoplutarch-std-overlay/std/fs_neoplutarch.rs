// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.1b — the `neoplutarch` arm of `std::sys::fs`: files, through the
//! process's own capabilities.
//!
//! Overlaid into `library/std/src/sys/fs/neoplutarch.rs`. A `File` is a
//! LogansOS file handle index. `File::open` takes the kernel's root form
//! (`SYS_OPEN` on an absolute path), which on a `cap-rooted-open` kernel
//! resolves only through a RootDirCap the process holds, with rights within
//! the cap's (OT-66 K1a). **No ambient namespace:** `open` refuses with
//! `PermissionDenied` before any syscall unless the process holds a
//! RootDirCap, so a renderer spawned without one cannot open anything, even on
//! a kernel without `cap-rooted-open`.
//!
//! Supported: open (read, write, create), read, write (`SYS_WRITE` on the
//! handle index), flush (a no-op), sync (`SYS_FSYNC`), close on drop.
//! Everything else (metadata, seek, directories, rename, links, permissions,
//! times, locks) is `Unsupported`, loudly.

use crate::ffi::OsString;
use crate::fmt;
use crate::fs::TryLockError;
use crate::hash::{Hash, Hasher};
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut, SeekFrom};
use crate::path::{Path, PathBuf};
pub use crate::sys::fs::common::Dir;
use crate::sys::neoplutarch_rt as rt;
use crate::sys::time::SystemTime;
use crate::sys::unsupported;

/// An open LogansOS file handle (its index in the process's table).
pub struct File(u32);

pub struct FileAttr(!);

pub struct ReadDir(!);

pub struct DirEntry(!);

#[derive(Clone, Debug)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    create: bool,
    create_new: bool,
    append: bool,
    truncate: bool,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FileTimes {}

pub struct FilePermissions(!);

pub struct FileType(!);

#[derive(Debug)]
pub struct DirBuilder {}

fn cvt(r: i64) -> io::Result<usize> {
    if r < 0 { Err(io::Error::from_raw_os_error((-r) as i32)) } else { Ok(r as usize) }
}

impl FileAttr {
    pub fn size(&self) -> u64 {
        self.0
    }

    pub fn perm(&self) -> FilePermissions {
        self.0
    }

    pub fn file_type(&self) -> FileType {
        self.0
    }

    pub fn modified(&self) -> io::Result<SystemTime> {
        self.0
    }

    pub fn accessed(&self) -> io::Result<SystemTime> {
        self.0
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        self.0
    }
}

impl Clone for FileAttr {
    fn clone(&self) -> FileAttr {
        self.0
    }
}

impl FilePermissions {
    pub fn readonly(&self) -> bool {
        self.0
    }

    pub fn set_readonly(&mut self, _readonly: bool) {
        self.0
    }
}

impl Clone for FilePermissions {
    fn clone(&self) -> FilePermissions {
        self.0
    }
}

impl PartialEq for FilePermissions {
    fn eq(&self, _other: &FilePermissions) -> bool {
        self.0
    }
}

impl Eq for FilePermissions {}

impl fmt::Debug for FilePermissions {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
    }
}

impl FileTimes {
    pub fn set_accessed(&mut self, _t: SystemTime) {}
    pub fn set_modified(&mut self, _t: SystemTime) {}
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.0
    }

    pub fn is_file(&self) -> bool {
        self.0
    }

    pub fn is_symlink(&self) -> bool {
        self.0
    }
}

impl Clone for FileType {
    fn clone(&self) -> FileType {
        self.0
    }
}

impl Copy for FileType {}

impl PartialEq for FileType {
    fn eq(&self, _other: &FileType) -> bool {
        self.0
    }
}

impl Eq for FileType {}

impl Hash for FileType {
    fn hash<H: Hasher>(&self, _h: &mut H) {
        self.0
    }
}

impl fmt::Debug for FileType {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
    }
}

impl fmt::Debug for ReadDir {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        self.0
    }
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.0
    }

    pub fn file_name(&self) -> OsString {
        self.0
    }

    pub fn metadata(&self) -> io::Result<FileAttr> {
        self.0
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        self.0
    }
}

impl OpenOptions {
    pub fn new() -> OpenOptions {
        OpenOptions {
            read: false,
            write: false,
            create: false,
            create_new: false,
            append: false,
            truncate: false,
        }
    }

    pub fn read(&mut self, read: bool) {
        self.read = read;
    }
    pub fn write(&mut self, write: bool) {
        self.write = write;
    }
    pub fn append(&mut self, append: bool) {
        self.append = append;
    }
    pub fn truncate(&mut self, truncate: bool) {
        self.truncate = truncate;
    }
    pub fn create(&mut self, create: bool) {
        self.create = create;
    }
    pub fn create_new(&mut self, create_new: bool) {
        self.create_new = create_new;
    }
}

impl File {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<File> {
        // No ambient namespace: without a RootDirCap there is nothing to
        // open through. The kernel enforces the same on `cap-rooted-open`.
        if !rt::holds_root_dir_cap() {
            return Err(io::const_error!(
                io::ErrorKind::PermissionDenied,
                "no RootDirCap: this process was given no file authority"
            ));
        }
        // The kernel has no append, truncate or exclusive-create; refuse them
        // rather than open with different semantics.
        if opts.append || opts.truncate || opts.create_new {
            return unsupported();
        }
        let mut rights = 0;
        if opts.read {
            rights |= rt::FILE_READ;
        }
        if opts.write {
            rights |= rt::FILE_WRITE;
        }
        if rights == 0 {
            return Err(io::const_error!(io::ErrorKind::InvalidInput, "open with neither read nor write"));
        }
        let path = path.as_os_str().as_encoded_bytes();
        let flags = if opts.create { rt::OPEN_CREATE_REGULAR } else { rt::OPEN_ROOT };
        cvt(rt::open(path, flags, rights)).map(|h| File(h as u32))
    }

    /// The handle index (for `std::os::neoplutarch`).
    pub fn as_raw(&self) -> u32 {
        self.0
    }

    /// Takes ownership of handle `h` (for `std::os::neoplutarch`).
    pub fn from_raw(h: u32) -> File {
        File(h)
    }

    /// Releases ownership of the handle without closing it.
    pub fn into_raw(self) -> u32 {
        let h = self.0;
        crate::mem::forget(self);
        h
    }

    pub fn file_attr(&self) -> io::Result<FileAttr> {
        unsupported()
    }

    pub fn fsync(&self) -> io::Result<()> {
        cvt(rt::fsync(self.0)).map(drop)
    }

    pub fn datasync(&self) -> io::Result<()> {
        self.fsync()
    }

    pub fn lock(&self) -> io::Result<()> {
        unsupported()
    }

    pub fn lock_shared(&self) -> io::Result<()> {
        unsupported()
    }

    pub fn try_lock(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(io::Error::UNSUPPORTED_PLATFORM))
    }

    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(io::Error::UNSUPPORTED_PLATFORM))
    }

    pub fn unlock(&self) -> io::Result<()> {
        unsupported()
    }

    pub fn truncate(&self, _size: u64) -> io::Result<()> {
        unsupported()
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        cvt(rt::read(self.0, buf))
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        crate::io::default_read_vectored(|b| self.read(b), bufs)
    }

    pub fn is_read_vectored(&self) -> bool {
        false
    }

    pub fn read_buf(&self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        crate::io::default_read_buf(|b| self.read(b), cursor)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        cvt(rt::write(self.0 as u64, buf))
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        crate::io::default_write_vectored(|b| self.write(b), bufs)
    }

    pub fn is_write_vectored(&self) -> bool {
        false
    }

    pub fn flush(&self) -> io::Result<()> {
        Ok(())
    }

    pub fn seek(&self, _pos: SeekFrom) -> io::Result<u64> {
        unsupported()
    }

    pub fn size(&self) -> Option<io::Result<u64>> {
        None
    }

    pub fn tell(&self) -> io::Result<u64> {
        unsupported()
    }

    pub fn duplicate(&self) -> io::Result<File> {
        unsupported()
    }

    pub fn set_permissions(&self, perm: FilePermissions) -> io::Result<()> {
        match perm.0 {}
    }

    pub fn set_times(&self, _times: FileTimes) -> io::Result<()> {
        unsupported()
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = rt::close(self.0);
    }
}

impl DirBuilder {
    pub fn new() -> DirBuilder {
        DirBuilder {}
    }

    pub fn mkdir(&self, _p: &Path) -> io::Result<()> {
        unsupported()
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("File").field("handle", &self.0).finish()
    }
}

pub fn readdir(_p: &Path) -> io::Result<ReadDir> {
    unsupported()
}

pub fn unlink(_p: &Path) -> io::Result<()> {
    unsupported()
}

pub fn rename(_old: &Path, _new: &Path) -> io::Result<()> {
    unsupported()
}

pub fn set_perm(_p: &Path, perm: FilePermissions) -> io::Result<()> {
    match perm.0 {}
}

pub fn set_times(_p: &Path, _times: FileTimes) -> io::Result<()> {
    unsupported()
}

pub fn set_times_nofollow(_p: &Path, _times: FileTimes) -> io::Result<()> {
    unsupported()
}

pub fn rmdir(_p: &Path) -> io::Result<()> {
    unsupported()
}

pub fn remove_dir_all(_path: &Path) -> io::Result<()> {
    unsupported()
}

pub fn exists(_path: &Path) -> io::Result<bool> {
    unsupported()
}

pub fn readlink(_p: &Path) -> io::Result<PathBuf> {
    unsupported()
}

pub fn symlink(_original: &Path, _link: &Path) -> io::Result<()> {
    unsupported()
}

pub fn link(_src: &Path, _dst: &Path) -> io::Result<()> {
    unsupported()
}

pub fn stat(_p: &Path) -> io::Result<FileAttr> {
    unsupported()
}

pub fn lstat(_p: &Path) -> io::Result<FileAttr> {
    unsupported()
}

pub fn canonicalize(_p: &Path) -> io::Result<PathBuf> {
    unsupported()
}

pub fn copy(_from: &Path, _to: &Path) -> io::Result<u64> {
    unsupported()
}
