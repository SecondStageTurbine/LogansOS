// SPDX-License-Identifier: MPL-2.0
//! L3a-2: FAT12/16/32, read-only, through the vendored rust-fatfs
//! (`third_party/fatfs`, MIT; `docs/THINK-2026-10-31-l3a-fat-readonly.md` §5).
//!
//! The pieces:
//! - [`BlockIo`]: rust-fatfs's `Read + Seek` over a `BlockDevice`, with a
//!   one-chunk read cache (rust-fatfs reads the FAT one entry at a time).
//!   Its `Write` always fails, so nothing this module does can reach the
//!   device with a write. A read-only session issues none anyway: rust-fatfs's
//!   `Drop` runs `unmount`, which writes FSInfo only when an allocation
//!   changed its counts and rewrites the status byte only when a write set
//!   the dirty flag, and it keeps the flags the BPB had (a volume mounted
//!   dirty stays dirty: trap T4/T5). The failing `Write` is the backstop if
//!   that ever changes upstream; `phase3_fat32_readonly_no_write` proves the
//!   device's bytes are unchanged after mount, reads and unmount.
//! - [`Fat`], the `MountedFs`: the `FileSystem` behind one `SwitchMutex`
//!   (rust-fatfs uses `RefCell` inside, so it is not `Sync`). The lock is held
//!   for one VFS call, around block reads; it takes no other lock, and no
//!   AI-governance lock (I-14(a)).
//! - [`FatInode`]: a path. rust-fatfs's `File` and `Dir` borrow the
//!   `FileSystem`, so an inode re-opens its path under the lock for each call.
//!
//! Imported kinds are `Directory` and `Regular` only, as the native driver
//! did: a foreign filesystem has no NeoFS typed-inode metadata.

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use fatfs::{IoBase, Read, Seek, SeekFrom, Write};

use crate::drivers::block::{BlockDevice, DmaBuffer, ReadOnlyDevice};
use crate::fs::{FileRights, Inode, InodeKind, MountedFs, VfsDirEntry, VfsError};
use crate::sync::SwitchMutex;

/// rust-fatfs supports any sector size; the native driver took 512 only,
/// and so does this one until a 4K-sector device is in a scenario.
const SECTOR_SIZE: u64 = 512;
/// The read cache's size: eight sectors, one request per miss.
const CHUNK: u64 = 8 * SECTOR_SIZE;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BlockIoError {
    /// The device refused or failed a read.
    Device,
    /// A read past the device's end.
    UnexpectedEof,
    /// Any write, always.
    ReadOnly,
    /// A seek before 0 or past `u64`.
    BadSeek,
}

impl fatfs::IoError for BlockIoError {
    fn is_interrupted(&self) -> bool {
        false
    }

    fn new_unexpected_eof_error() -> Self {
        Self::UnexpectedEof
    }

    fn new_write_zero_error() -> Self {
        Self::ReadOnly
    }
}

/// The byte stream rust-fatfs reads: the device's bytes, from 0 to its end.
pub struct BlockIo {
    /// Read-only by construction: no write method exists on this type.
    device: ReadOnlyDevice,
    len: u64,
    pos: u64,
    /// The cached chunk's first byte, and its bytes (empty: nothing cached).
    cache_at: u64,
    cache: Vec<u8>,
}

impl BlockIo {
    fn new(device: ReadOnlyDevice) -> Result<Self, BlockIoError> {
        let geometry = device.geometry();
        if u64::from(geometry.logical_block_size) != SECTOR_SIZE {
            return Err(BlockIoError::Device);
        }
        let len = geometry
            .block_count
            .checked_mul(SECTOR_SIZE)
            .ok_or(BlockIoError::Device)?;
        Ok(Self {
            device,
            len,
            pos: 0,
            cache_at: 0,
            cache: Vec::new(),
        })
    }

    /// Load the chunk holding byte `at` (`at < len`).
    fn fill(&mut self, at: u64) -> Result<(), BlockIoError> {
        let start = at - at % CHUNK;
        let bytes = CHUNK.min(self.len - start) as usize;
        let dma = DmaBuffer::alloc_zeroed(bytes).map_err(|_| BlockIoError::Device)?;
        self.device
            .read_block(start / SECTOR_SIZE, core::slice::from_ref(&dma))
            .map_err(|_| BlockIoError::Device)?;
        self.cache.clear();
        // SAFETY: the synchronous read has completed, and the buffer is not
        // part of any other request while it is copied.
        unsafe {
            self.cache.extend_from_slice(&dma.as_slice()[..bytes]);
        }
        self.cache_at = start;
        Ok(())
    }
}

impl IoBase for BlockIo {
    type Error = BlockIoError;
}

impl Read for BlockIo {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() || self.pos >= self.len {
            return Ok(0);
        }
        let cached = !self.cache.is_empty()
            && self.pos >= self.cache_at
            && self.pos < self.cache_at + self.cache.len() as u64;
        if !cached {
            self.fill(self.pos)?;
        }
        let from = (self.pos - self.cache_at) as usize;
        let n = buf.len().min(self.cache.len() - from);
        buf[..n].copy_from_slice(&self.cache[from..from + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

/// Test-only: writes rust-fatfs attempted (all refused). The no-write
/// scenario expects 0: a read-only session should not even try.
#[cfg(feature = "neo-test")]
pub static WRITE_ATTEMPTS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

impl Write for BlockIo {
    fn write(&mut self, _buf: &[u8]) -> Result<usize, Self::Error> {
        #[cfg(feature = "neo-test")]
        WRITE_ATTEMPTS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        Err(BlockIoError::ReadOnly)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Seek for BlockIo {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        let next = match pos {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
            SeekFrom::End(d) => self.len.checked_add_signed(d),
        };
        self.pos = next.ok_or(BlockIoError::BadSeek)?;
        Ok(self.pos)
    }
}

type FatFs = fatfs::FileSystem<BlockIo, fatfs::NullTimeProvider, fatfs::LossyOemCpConverter>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FatError {
    /// The device is not 512-byte sectored or could not be read.
    BlockDevice,
    /// No valid FAT boot sector (rust-fatfs refused the volume).
    Invalid,
}

pub struct Fat {
    state: Arc<FatState>,
}

struct FatState {
    fs: SwitchMutex<FatFs>,
}

/// Test-only: the last mount's dirty indicators (bit 0 BPB, bit 1 FAT[1]).
#[cfg(feature = "neo-test")]
pub static LAST_DIRTY: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

/// One 512-byte sector, read through the read-only device.
fn read_sector(device: &ReadOnlyDevice, lba: u64) -> Result<Vec<u8>, FatError> {
    let dma = DmaBuffer::alloc_zeroed(SECTOR_SIZE as usize).map_err(|_| FatError::BlockDevice)?;
    device
        .read_block(lba, core::slice::from_ref(&dma))
        .map_err(|_| FatError::BlockDevice)?;
    // SAFETY: the synchronous read has completed; the buffer is in no other
    // request while it is copied.
    Ok(unsafe { dma.as_slice()[..SECTOR_SIZE as usize].to_vec() })
}

impl Fat {
    pub fn mount_readonly(device: Arc<dyn BlockDevice>) -> Result<Self, FatError> {
        let device = ReadOnlyDevice::new(device);
        // T13: the boot sector's geometry is checked before the library
        // divides by it; then T4: the two dirty indicators are read apart
        // (rust-fatfs's `read_status_flags` merges them) and logged.
        let sector0 = read_sector(&device, 0)?;
        let bpb = super::fat_logic::check_bpb(&sector0).map_err(|why| {
            crate::serial_println!("fat: boot sector refused: {:?}", why);
            FatError::Invalid
        })?;
        let fat_lba =
            u64::from(bpb.reserved_sectors) * u64::from(bpb.bytes_per_sector) / SECTOR_SIZE;
        let fat_sector = read_sector(&device, fat_lba)?;
        let io = BlockIo::new(device).map_err(|_| FatError::BlockDevice)?;
        // `update_accessed_date(false)` is the default; stated, since a read
        // must never write. `strict` stays on (the BPB is validated).
        let options = fatfs::FsOptions::new()
            .update_accessed_date(false)
            .time_provider(fatfs::NullTimeProvider::new())
            .oem_cp_converter(fatfs::LossyOemCpConverter::new());
        let fs = fatfs::FileSystem::new(io, options).map_err(|_| FatError::Invalid)?;
        let dirty = super::fat_logic::dirty_flags(
            &bpb,
            &sector0,
            &fat_sector,
            fs.fat_type() == fatfs::FatType::Fat16,
        );
        crate::serial_println!(
            "fat: mounted read-only type={:?} dirty bpb={} fat1={}",
            fs.fat_type(),
            u8::from(dirty.bpb),
            u8::from(dirty.fat1)
        );
        #[cfg(feature = "neo-test")]
        LAST_DIRTY.store(
            u8::from(dirty.bpb) | (u8::from(dirty.fat1) << 1),
            core::sync::atomic::Ordering::Relaxed,
        );
        Ok(Self {
            state: Arc::new(FatState {
                fs: SwitchMutex::new(fs),
            }),
        })
    }
}

/// The path components of `relative`, refusing `.`, `..` and empty ones (as
/// the native driver did; the VFS resolves those before a mount sees them).
fn components(relative: &str) -> Result<Vec<&str>, VfsError> {
    if relative.is_empty() {
        return Ok(Vec::new());
    }
    let parts: Vec<&str> = relative.split('/').collect();
    if parts
        .iter()
        .any(|c| c.is_empty() || *c == "." || *c == "..")
    {
        return Err(VfsError::Invalid);
    }
    Ok(parts)
}

fn kind_of(
    entry: &fatfs::DirEntry<'_, BlockIo, fatfs::NullTimeProvider, fatfs::LossyOemCpConverter>,
) -> InodeKind {
    if entry.is_dir() {
        InodeKind::Directory
    } else {
        InodeKind::Regular
    }
}

/// Walk `parts` from the root: the last component's kind and length, or the
/// root's. Case-insensitive, as FAT is.
fn resolve(fs: &FatFs, parts: &[&str]) -> Result<(InodeKind, u64), VfsError> {
    let mut dir = fs.root_dir();
    let Some((last, parents)) = parts.split_last() else {
        return Ok((InodeKind::Directory, 0));
    };
    for name in parents {
        dir = find(&dir, name)?
            .filter(|e| e.is_dir())
            .map(|e| e.to_dir())
            .ok_or(VfsError::NotFound)?;
    }
    let entry = find(&dir, last)?.ok_or(VfsError::NotFound)?;
    Ok((kind_of(&entry), entry.len()))
}

type FatDir<'a> = fatfs::Dir<'a, BlockIo, fatfs::NullTimeProvider, fatfs::LossyOemCpConverter>;
type FatEntry<'a> =
    fatfs::DirEntry<'a, BlockIo, fatfs::NullTimeProvider, fatfs::LossyOemCpConverter>;

/// The entry `name` of `dir` (not `.`/`..`), matched case-insensitively on
/// the long name or the short one.
fn find<'a>(dir: &FatDir<'a>, name: &str) -> Result<Option<FatEntry<'a>>, VfsError> {
    for entry in dir.iter() {
        let entry = entry.map_err(|_| VfsError::Invalid)?;
        if is_dot(&entry) {
            continue;
        }
        if same_name(&entry.file_name(), name) || same_name(&entry.short_file_name(), name) {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

/// FAT names compare case-insensitively: both sides folded to uppercase
/// character by character (rust-fatfs's own rule for its path walk).
fn same_name(a: &str, b: &str) -> bool {
    a.chars()
        .flat_map(char::to_uppercase)
        .eq(b.chars().flat_map(char::to_uppercase))
}

fn is_dot(entry: &FatEntry<'_>) -> bool {
    let short = entry.short_file_name_as_bytes();
    short == b"." || short == b".."
}

/// The directory at `parts`.
fn open_dir<'a>(fs: &'a FatFs, parts: &[&str]) -> Result<FatDir<'a>, VfsError> {
    let mut dir = fs.root_dir();
    for name in parts {
        dir = find(&dir, name)?
            .filter(|e| e.is_dir())
            .map(|e| e.to_dir())
            .ok_or(VfsError::NotDirectory)?;
    }
    Ok(dir)
}

impl MountedFs for Fat {
    fn root(&self) -> Result<Arc<dyn Inode>, VfsError> {
        Ok(Arc::new(FatInode {
            state: self.state.clone(),
            path: String::new(),
            kind: InodeKind::Directory,
            size: 0,
        }))
    }

    fn lookup(&self, relative: &str) -> Result<Arc<dyn Inode>, VfsError> {
        let parts = components(relative)?;
        let (kind, size) = {
            let fs = self.state.fs.lock();
            resolve(&fs, &parts)?
        };
        Ok(Arc::new(FatInode {
            state: self.state.clone(),
            path: String::from(relative),
            kind,
            size,
        }))
    }
}

struct FatInode {
    state: Arc<FatState>,
    /// Mount-relative, `/`-separated, as looked up.
    path: String,
    kind: InodeKind,
    size: u64,
}

impl Inode for FatInode {
    fn kind(&self) -> InodeKind {
        self.kind
    }

    fn available_rights(&self) -> FileRights {
        match self.kind {
            InodeKind::Directory => {
                FileRights::from_bits(FileRights::LOOKUP.bits() | FileRights::READDIR.bits())
            }
            InodeKind::Regular => {
                FileRights::from_bits(FileRights::READ.bits() | FileRights::EXECUTE.bits())
            }
            InodeKind::Symlink | InodeKind::Model => FileRights::empty(),
        }
    }

    fn byte_len(&self) -> Option<u64> {
        (self.kind == InodeKind::Regular).then_some(self.size)
    }

    fn readdir_at(&self, cursor: u64) -> Result<Option<VfsDirEntry>, VfsError> {
        if self.kind != InodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        let idx = usize::try_from(cursor).map_err(|_| VfsError::Invalid)?;
        let parts = components(&self.path)?;
        let fs = self.state.fs.lock();
        let dir = open_dir(&fs, &parts)?;
        let mut seen = 0usize;
        for entry in dir.iter() {
            let entry = entry.map_err(|_| VfsError::Invalid)?;
            if is_dot(&entry) {
                continue;
            }
            if seen == idx {
                return Ok(Some(VfsDirEntry {
                    name: entry.file_name(),
                    kind: kind_of(&entry),
                    next_cursor: cursor + 1,
                }));
            }
            seen += 1;
        }
        Ok(None)
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> Result<usize, VfsError> {
        if self.kind != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        if out.is_empty() || offset as u64 >= self.size {
            return Ok(0);
        }
        let parts = components(&self.path)?;
        let Some((last, parents)) = parts.split_last() else {
            return Err(VfsError::TypeForbidden);
        };
        let fs = self.state.fs.lock();
        let dir = open_dir(&fs, parents)?;
        let entry = find(&dir, last)?.ok_or(VfsError::NotFound)?;
        if entry.is_dir() {
            return Err(VfsError::TypeForbidden);
        }
        let mut file = entry.to_file();
        file.seek(SeekFrom::Start(offset as u64))
            .map_err(|_| VfsError::Invalid)?;
        let want = out.len().min((self.size - offset as u64) as usize);
        let mut done = 0;
        while done < want {
            match file.read(&mut out[done..want]) {
                Ok(0) => break,
                Ok(n) => done += n,
                Err(_) => return Err(VfsError::Invalid),
            }
        }
        Ok(done)
    }

    fn write_at(&self, _offset: usize, _input: &[u8]) -> Result<usize, VfsError> {
        if self.kind != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        Err(VfsError::InsufficientRights)
    }
}
