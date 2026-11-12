// SPDX-License-Identifier: MPL-2.0
//! Minimal VFS/testfs substrate for Phase 3 BUILD-1 sub-commit 1C.
//!
//! This is deliberately small: one root mount backed by an in-memory testfs,
//! regular-file open/read/write, and explicit forward hooks for the wider
//! capability-rooted model. The temporary syscall ABI still opens from the
//! root namespace; callers must not treat that as the permanent authority
//! model.

extern crate alloc;

pub mod ext4;
// L3a-2: FAT12/16/32 read-only through the vendored rust-fatfs. Off (the
// kill-switch): the native FAT32 driver, as before, and no rust-fatfs code.
#[cfg(feature = "fat-fs")]
pub mod fat;
#[cfg(not(feature = "fat-fs"))]
pub mod fat32;
#[cfg(feature = "fat-fs")]
pub mod fat_logic;
pub mod neofs;

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp;
#[cfg(all(feature = "boot-neofs", feature = "neo-test"))]
use core::sync::atomic::AtomicU8;
use core::sync::atomic::{AtomicUsize, Ordering};

use spin::Lazy;

use crate::serial_println;
use crate::sync::SwitchMutex;

#[cfg(feature = "neo-test-neofs-read-recheck")]
static DEBUG_AFTER_READ: SwitchMutex<Option<fn()>> = SwitchMutex::new(None);

pub const OPEN_ROOT_TEMPORARY: u32 = 0;
pub const OPEN_CREATE_REGULAR: u32 = 1 << 0;
/// OT-66 K1b (`cap-rooted-open`): open a directory as a scope, a cap whose
/// handle rights bound the root-form opens beneath it. The request may name
/// file rights (READ, WRITE, EXECUTE) for the handle; the directory itself
/// opens with only the directory rights asked for. Refused without the
/// feature (`Invalid`, as any unknown flag).
pub const OPEN_SCOPE_DIR: u32 = 1 << 1;
pub const MOUNT_FLAG_EXEC_ALLOWED: u32 = 1 << 0;
const MOUNT_KNOWN_FLAGS_BUILD4C: u32 = MOUNT_FLAG_EXEC_ALLOWED;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct FileRights(u32);

impl FileRights {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const LOOKUP: Self = Self(1 << 2);
    pub const READDIR: Self = Self(1 << 3);

    // Reserved Phase 3+ bits: MMAP, GRANT, DERIVE, RETYPE.
    // BUILD-4A activated LOOKUP/READDIR; BUILD-4B activates MOUNT while
    // preserving the `*_RESERVED` aliases for source compatibility.
    pub const LOOKUP_RESERVED: Self = Self::LOOKUP;
    pub const READDIR_RESERVED: Self = Self::READDIR;
    pub const EXECUTE: Self = Self(1 << 4);
    pub const EXECUTE_RESERVED: Self = Self::EXECUTE;
    pub const MMAP_RESERVED: Self = Self(1 << 5);
    pub const GRANT_RESERVED: Self = Self(1 << 24);
    pub const DERIVE_RESERVED: Self = Self(1 << 25);
    pub const RETYPE_RESERVED: Self = Self(1 << 26);
    pub const MOUNT: Self = Self(1 << 27);
    pub const MOUNT_RESERVED: Self = Self::MOUNT;

    pub const KNOWN_BUILD1: Self = Self(Self::READ.0 | Self::WRITE.0);
    pub const KNOWN_BUILD2: Self = Self(Self::KNOWN_BUILD1.0 | Self::EXECUTE.0);
    pub const KNOWN_BUILD4A: Self = Self(Self::KNOWN_BUILD2.0 | Self::LOOKUP.0 | Self::READDIR.0);
    pub const KNOWN_BUILD4B: Self = Self(Self::KNOWN_BUILD4A.0 | Self::MOUNT.0);

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn contains(self, required: Self) -> bool {
        (self.0 & required.0) == required.0
    }

    pub const fn is_subset_of(self, available: Self) -> bool {
        (self.0 & available.0) == self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn without(self, denied: Self) -> Self {
        Self(self.0 & !denied.0)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum InodeKind {
    Directory,
    Regular,
    Symlink,
    Model,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum VfsError {
    NotFound,
    Invalid,
    Unsupported,
    TypeForbidden,
    NotDirectory,
    InsufficientRights,
    Stale,
    AlreadyExists,
    Busy,
    /// The storage device failed the operation (`EIO`).
    Io,
    /// B1c: a pinned boot module failed its digest check (`EBADMSG`).
    #[cfg(feature = "model-boot-module")]
    Integrity,
    /// F0: `unlink_at` of a directory with entries (`ENOTEMPTY`).
    #[cfg(feature = "neofs-dir-ops")]
    NotEmpty,
    /// F0: a rename between two volumes (`EXDEV`).
    #[cfg(feature = "neofs-dir-ops")]
    CrossDevice,
}

#[derive(Clone, Debug)]
pub struct VfsDirEntry {
    pub name: String,
    pub kind: InodeKind,
    pub next_cursor: u64,
}

pub trait Inode: Send + Sync {
    fn kind(&self) -> InodeKind;
    fn available_rights(&self) -> FileRights;
    fn current_generation(&self) -> Result<Option<u32>, VfsError> {
        Ok(None)
    }
    fn fsync(&self) -> Result<(), VfsError> {
        Ok(())
    }
    fn readdir_at(&self, _cursor: u64) -> Result<Option<VfsDirEntry>, VfsError> {
        Err(VfsError::NotDirectory)
    }
    fn read_at(&self, offset: usize, out: &mut [u8]) -> Result<usize, VfsError>;
    fn write_at(&self, offset: usize, input: &[u8]) -> Result<usize, VfsError>;
    fn truncate(&self, _new_len: u64) -> Result<(), VfsError> {
        Err(VfsError::Unsupported)
    }
    /// OT-67 W1: `bytes` become the whole content, in one commit.
    #[cfg(feature = "neofs-file-replace")]
    fn replace(&self, _bytes: &[u8]) -> Result<(), VfsError> {
        Err(VfsError::Unsupported)
    }
    /// F0: a new empty directory `name` in this directory.
    #[cfg(feature = "neofs-dir-ops")]
    fn mkdir_at(&self, _name: &str) -> Result<(), VfsError> {
        Err(VfsError::Unsupported)
    }
    /// F0: remove `name` (a file, or an empty directory) from this directory.
    #[cfg(feature = "neofs-dir-ops")]
    fn unlink_at(&self, _name: &str) -> Result<(), VfsError> {
        Err(VfsError::Unsupported)
    }
    /// F0: move `name` here to `to_name` in `to`, on the same volume, never
    /// replacing.
    #[cfg(feature = "neofs-dir-ops")]
    fn rename_at(&self, _name: &str, _to: &dyn Inode, _to_name: &str) -> Result<(), VfsError> {
        Err(VfsError::Unsupported)
    }
    /// F0: `name`'s kind, flags and size.
    #[cfg(feature = "neofs-dir-ops")]
    fn stat_at(&self, _name: &str) -> Result<neofs::dir_ops_logic::StatAtV1, VfsError> {
        Err(VfsError::Unsupported)
    }
    /// F0: after this directory's own commit, read its new version; returns
    /// the generation a handle on it now holds.
    #[cfg(feature = "neofs-dir-ops")]
    fn reload_dir(&self) -> Result<Option<u32>, VfsError> {
        self.current_generation()
    }
    /// F0: the NeoFS volume this directory is on (an identity) and its path
    /// there (`None`: the volume's root). `None` off NeoFS.
    #[cfg(feature = "neofs-dir-ops")]
    fn neofs_volume(&self) -> Option<(usize, Option<&str>)> {
        None
    }
    /// F0's scenario: a reload read now and published later (see
    /// `OpenFile::debug_after_dir_op_later`). `None` off NeoFS.
    #[cfg(feature = "neo-test-neofs-dir-ops")]
    #[allow(clippy::type_complexity)]
    fn debug_reload_later(
        &self,
    ) -> Option<alloc::boxed::Box<dyn FnOnce() -> Result<Option<u32>, VfsError> + '_>> {
        None
    }
    /// File length in bytes, when the inode knows it (a regular/model file on a
    /// backing FS). `None` for inodes with no fixed length. Used by readers that
    /// pre-size a buffer to avoid reallocation — e.g. the Stage-B real-brain model
    /// load, where a growing Vec doubles (256->512 MiB) and momentarily holds both
    /// the old and new buffers, exhausting the 768 MiB heap (observed on silicon).
    fn byte_len(&self) -> Option<u64> {
        None
    }
    /// OT-87 R2: the BLAKE3 content hash this inode's bytes were verified
    /// against, and the generation it holds for. Verifies first when this
    /// generation was not yet verified. `None`: the filesystem keeps no
    /// content hash (only NeoFS does).
    #[cfg(any(feature = "model-from-neofs", feature = "installer"))]
    fn verified_content_hash(&self) -> Result<Option<VerifiedHash>, VfsError> {
        Ok(None)
    }
}

/// OT-87 R2: a content hash the filesystem checked against the bytes, and the
/// inode generation it was checked for.
#[cfg(any(feature = "model-from-neofs", feature = "installer"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedHash {
    pub blake3: [u8; 32],
    pub generation: u32,
}

pub struct OpenFile {
    inode: Arc<dyn Inode>,
    rights: FileRights,
    /// Process-context VFS cursor state: only syscall/read/write/seek-like
    /// paths acquire it; no ISR or IF=0 path reaches this per-handle lock.
    offset: SwitchMutex<usize>,
    /// Process-context stale-handle generation snapshot; no interrupt handler
    /// observes or mutates OpenFile generation state.
    generation: SwitchMutex<Option<u32>>,
    /// Mount entry this OpenFile was opened through. `sys_mount`/`sys_umount`
    /// use this to identify the parent mount when the OpenFile is a directory
    /// FileCap. `None` for synthetic OpenFiles built directly against an
    /// `Arc<dyn Inode>` (debug helpers).
    mount_id: Option<MountId>,
    /// Counter on the mount entry above. Bumped when this struct is constructed
    /// and decremented in `Drop`. `sys_umount` reads it to answer `EBUSY`
    /// precisely. `None` mirrors `mount_id`.
    mount_live: Option<Arc<AtomicUsize>>,
    /// OT-66 K1a: opened at "/", the root of the root mount. A file handle on
    /// such an OpenFile is a RootDirCap.
    #[cfg(feature = "cap-rooted-open")]
    vfs_root: bool,
    /// OT-66 K1b: a directory opened by path records that path (normalized,
    /// no leading `/`; "" is the VFS root). A handle on it is a scope: it
    /// authorizes root-form opens at or beneath this path, within the
    /// handle's rights (`sched::process::root_dir_cap_for`).
    #[cfg(feature = "cap-rooted-open")]
    scope: Option<String>,
    /// OT-66 K1a: the inode is a directory, recorded at open (an inode's kind
    /// never changes). Spawn reads it under the caller's IF-off handles lock,
    /// where taking the inode's own lock (NeoFS `record`, a preemptible
    /// `SwitchMutex`) could spin forever on a holder this CPU preempted
    /// (CP-37, CP-56).
    #[cfg(feature = "cap-rooted-open")]
    is_dir: bool,
}

#[derive(Clone)]
pub struct FileProvenance {
    inode: Arc<dyn Inode>,
    generation: Option<u32>,
    mount_id: Option<MountId>,
}

impl FileProvenance {
    /// Test-only synthetic provenance for a model built from `.neom` bytes embedded in
    /// the kernel image rather than mounted from a disk. `generation: None` +
    /// `mount_id: None` ⇒ `validate_current` always returns `Ok` (never stale). Used by
    /// the BUILD-4b-2 accel_intel forward-pass-equivalence scenario so the model rides
    /// in the kernel image — no new disk/mount hardware dependency on the blind
    /// bare-metal OptiPlex run. `exec::run_with_driver` never consults provenance, so the
    /// synthetic value is sound for the forward pass.
    #[cfg(feature = "neo-test")]
    pub fn synthetic_always_valid() -> Self {
        Self {
            inode: Arc::new(TestInode::regular(FileRights::READ, &[])),
            generation: None,
            mount_id: None,
        }
    }

    /// B1b: provenance for a model built from an immutable boot module. The
    /// bytes come from the boot image, which cannot change under the model, so
    /// like the synthetic value it never goes stale.
    #[cfg(feature = "model-boot-module")]
    pub fn boot_module() -> Self {
        Self {
            inode: Arc::new(TestInode::regular(FileRights::READ, &[])),
            generation: None,
            mount_id: None,
        }
    }

    pub fn validate_current(&self) -> Result<(), VfsError> {
        if let Some(mount_id) = self.mount_id {
            // IF-off: callers run this IF-on (`model_still_current`), and
            // `MOUNTS` is also taken at IF=0 (see `resolve_path_to_inode`).
            let mounted =
                crate::arch::x86_64::without_interrupts(|| lock_mounts().contains_id(mount_id));
            if !mounted {
                return Err(VfsError::Stale);
            }
        }
        let Some(captured) = self.generation else {
            return Ok(());
        };
        match self.inode.current_generation()? {
            Some(current) if current == captured => Ok(()),
            Some(_) | None => Err(VfsError::Stale),
        }
    }
}

impl OpenFile {
    fn new(inode: Arc<dyn Inode>, rights: FileRights) -> Result<Self, VfsError> {
        Self::new_with_mount(inode, rights, None, None)
    }

    fn new_with_mount(
        inode: Arc<dyn Inode>,
        rights: FileRights,
        mount_id: Option<MountId>,
        mount_live: Option<Arc<AtomicUsize>>,
    ) -> Result<Self, VfsError> {
        let generation = inode.current_generation()?;
        #[cfg(feature = "cap-rooted-open")]
        let is_dir = inode.kind() == InodeKind::Directory;
        if let Some(counter) = mount_live.as_ref() {
            counter.fetch_add(1, Ordering::AcqRel);
        }
        Ok(Self {
            inode,
            rights,
            offset: SwitchMutex::new(0),
            generation: SwitchMutex::new(generation),
            mount_id,
            mount_live,
            #[cfg(feature = "cap-rooted-open")]
            vfs_root: false,
            #[cfg(feature = "cap-rooted-open")]
            scope: None,
            #[cfg(feature = "cap-rooted-open")]
            is_dir,
        })
    }

    /// OT-66 K1a: whether this is the VFS root directory, opened at "/".
    #[cfg(feature = "cap-rooted-open")]
    pub fn is_vfs_root(&self) -> bool {
        self.vfs_root
    }

    /// OT-66 K1b: the path this directory was opened at, if it was opened by
    /// path (see `scope`). `None` for a file.
    #[cfg(feature = "cap-rooted-open")]
    pub fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }

    /// OT-66 K1a: whether this OpenFile is a directory. Takes no lock: the
    /// kind was recorded at open.
    #[cfg(feature = "cap-rooted-open")]
    pub fn is_directory(&self) -> bool {
        self.is_dir
    }

    /// Snapshot the current live-handle count on this OpenFile's mount (if
    /// any). Used by the BUILD-4B umount-busy test to observe accounting.
    #[cfg(feature = "neo-test")]
    pub fn debug_mount_live_count(&self) -> Option<usize> {
        self.mount_live.as_ref().map(|c| c.load(Ordering::Acquire))
    }

    pub fn rights(&self) -> FileRights {
        self.rights
    }

    pub fn validate_current(&self) -> Result<(), VfsError> {
        self.validate_snapshot().map(|_| ())
    }

    /// [`Self::validate_current`], returning the generation it checked
    /// against. A read keeps this snapshot for its second check
    /// ([`Self::validate_after_read`]): the handle's own field is refreshed
    /// by a writer on this same OpenFile (a derived fd or another thread
    /// sharing it), so after such a commit it would match the new version
    /// and miss the one the read started on (#213 AAG).
    fn validate_snapshot(&self) -> Result<Option<u32>, VfsError> {
        let captured = *self.generation.lock();
        let Some(captured) = captured else {
            return Ok(None);
        };
        match self.inode.current_generation()? {
            Some(current) if current == captured => Ok(Some(captured)),
            Some(_) | None => Err(VfsError::Stale),
        }
    }

    /// OT-87 R2: the open inode's verified content hash (see
    /// [`Inode::verified_content_hash`]).
    #[cfg(any(feature = "model-from-neofs", feature = "installer"))]
    pub fn verified_content_hash(&self) -> Result<Option<VerifiedHash>, VfsError> {
        self.inode.verified_content_hash()
    }

    pub fn provenance_snapshot(&self) -> FileProvenance {
        FileProvenance {
            inode: self.inode.clone(),
            generation: *self.generation.lock(),
            mount_id: self.mount_id,
        }
    }

    pub fn check_executable_policy(&self) -> Result<(), VfsError> {
        if self.inode.kind() != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        if !self.rights.contains(FileRights::EXECUTE) {
            return Err(VfsError::InsufficientRights);
        }
        self.validate_current()
    }

    pub fn fsync(&self) -> Result<(), VfsError> {
        self.validate_current()?;
        self.inode.fsync()
    }

    #[cfg(feature = "neo-test-phase3-neofs-2e")]
    pub fn debug_captured_generation(&self) -> Option<u32> {
        *self.generation.lock()
    }

    #[cfg(feature = "neo-test-phase3-neofs-2e")]
    pub fn debug_current_generation(&self) -> Result<Option<u32>, VfsError> {
        self.inode.current_generation()
    }

    fn refresh_generation(&self) -> Result<(), VfsError> {
        let generation = self.inode.current_generation()?;
        *self.generation.lock() = generation;
        Ok(())
    }

    /// Test only: run `hook` once, inside the next read, between the read
    /// and its re-check.
    #[cfg(feature = "neo-test-neofs-read-recheck")]
    pub fn debug_set_after_read_hook(hook: fn()) {
        *DEBUG_AFTER_READ.lock() = Some(hook);
    }

    pub fn read(&self, out: &mut [u8]) -> Result<usize, VfsError> {
        self.read_as(out, self.rights)
    }

    /// `read` through a file handle whose rights are `handle`: the handle may
    /// be narrower than this OpenFile (a derive narrows only the handle), so
    /// both must hold READ. Type first, rights second, as before.
    pub fn read_as(&self, out: &mut [u8], handle: FileRights) -> Result<usize, VfsError> {
        let read = self.read_as_then(out, handle, |_| Ok::<(), core::convert::Infallible>(()))?;
        Ok(read.unwrap_or_else(|never| match never {}))
    }

    /// `read_as`, handing the bytes read to `deliver` before the offset
    /// moves (under the offset lock). The offset advances only when `deliver`
    /// succeeds; its error comes back as the inner `Err`, with the offset
    /// where it was. `SYS_READ` delivers to the user buffer here, so a read
    /// whose copy faults (EFAULT) leaves the file as it found it (CP-99).
    pub fn read_as_then<E>(
        &self,
        out: &mut [u8],
        handle: FileRights,
        deliver: impl FnOnce(&[u8]) -> Result<(), E>,
    ) -> Result<Result<usize, E>, VfsError> {
        let snapshot = self.validate_snapshot()?;
        if !matches!(self.inode.kind(), InodeKind::Regular | InodeKind::Model) {
            return Err(VfsError::TypeForbidden);
        }
        if !self.rights.intersection(handle).contains(FileRights::READ) {
            return Err(VfsError::InsufficientRights);
        }
        let mut offset = self.offset.lock();
        let n = self.inode.read_at(*offset, out)?;
        self.validate_after_read(snapshot)?;
        if let Err(e) = deliver(&out[..n]) {
            return Ok(Err(e));
        }
        *offset = offset.saturating_add(n);
        Ok(Ok(n))
    }

    pub fn read_at_offset(&self, offset: usize, out: &mut [u8]) -> Result<usize, VfsError> {
        let snapshot = self.validate_snapshot()?;
        if !matches!(self.inode.kind(), InodeKind::Regular | InodeKind::Model) {
            return Err(VfsError::TypeForbidden);
        }
        if !self.rights.contains(FileRights::READ) {
            return Err(VfsError::InsufficientRights);
        }
        let n = self.inode.read_at(offset, out)?;
        self.validate_after_read(snapshot)?;
        Ok(n)
    }

    /// The second half of a read's check (#206 AAG, U2), as a seqlock reads:
    /// the version must still be current after the read. A commit that
    /// superseded it during the read could have freed its blocks, and a
    /// later one reused them, so the bytes read are only good when the
    /// generation is unchanged. Generations only grow, so "unchanged" means
    /// no commit ran. `snapshot` is the generation the read started on
    /// ([`Self::validate_snapshot`]), not the handle's field.
    fn validate_after_read(&self, snapshot: Option<u32>) -> Result<(), VfsError> {
        #[cfg(feature = "neo-test-neofs-read-recheck")]
        {
            let hook = DEBUG_AFTER_READ.lock().take();
            if let Some(hook) = hook {
                hook();
            }
        }
        if cfg!(feature = "vfs-read-no-recheck") {
            return Ok(());
        }
        // Test only: the comparison as first written, against the handle's
        // field (#213 AAG's falsification).
        if cfg!(feature = "vfs-read-recheck-against-handle") {
            return self.validate_current();
        }
        let Some(snapshot) = snapshot else {
            return Ok(());
        };
        match self.inode.current_generation()? {
            Some(current) if current == snapshot => Ok(()),
            Some(_) | None => Err(VfsError::Stale),
        }
    }

    pub fn write(&self, input: &[u8]) -> Result<usize, VfsError> {
        if !self.rights.contains(FileRights::WRITE) {
            return Err(VfsError::InsufficientRights);
        }
        if self.inode.kind() != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        self.validate_current()?;
        let mut offset = self.offset.lock();
        let n = self.inode.write_at(*offset, input)?;
        *offset = offset.saturating_add(n);
        self.refresh_generation()?;
        Ok(n)
    }

    pub fn truncate(&self, new_len: u64) -> Result<(), VfsError> {
        if !self.rights.contains(FileRights::WRITE) {
            return Err(VfsError::InsufficientRights);
        }
        if self.inode.kind() != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        self.validate_current()?;
        self.inode.truncate(new_len)?;
        self.refresh_generation()
    }

    /// OT-67 W1: the file's content becomes exactly `bytes`, in one commit.
    /// The offset moves to the end, as after a write of the whole file.
    #[cfg(feature = "neofs-file-replace")]
    pub fn replace(&self, bytes: &[u8]) -> Result<(), VfsError> {
        if !self.rights.contains(FileRights::WRITE) {
            return Err(VfsError::InsufficientRights);
        }
        if self.inode.kind() != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        self.validate_current()?;
        self.inode.replace(bytes)?;
        *self.offset.lock() = bytes.len();
        self.refresh_generation()
    }

    /// F0: a new empty directory `name` here. `handle` is the rights of the
    /// file handle naming this OpenFile (see [`Self::dir_op_check`]).
    #[cfg(feature = "neofs-dir-ops")]
    pub fn mkdir_at(&self, name: &[u8], handle: FileRights) -> Result<(), VfsError> {
        let name = self.dir_op_check(name, handle, true)?;
        let result = self.inode.mkdir_at(name);
        self.after_dir_op(result)
    }

    /// F0: remove `name`, a regular file or an empty directory.
    #[cfg(feature = "neofs-dir-ops")]
    pub fn unlink_at(&self, name: &[u8], handle: FileRights) -> Result<(), VfsError> {
        let name = self.dir_op_check(name, handle, true)?;
        let result = self.inode.unlink_at(name);
        self.after_dir_op(result)
    }

    /// F0: move `name` here to `to_name` in `to`. Both handles need the
    /// mutation's rights; both directories must be on one volume
    /// (`CrossDevice` otherwise), and the destination name free.
    #[cfg(feature = "neofs-dir-ops")]
    pub fn rename_at(
        &self,
        name: &[u8],
        handle: FileRights,
        to: &OpenFile,
        to_name: &[u8],
        to_handle: FileRights,
    ) -> Result<(), VfsError> {
        let name = self.dir_op_check(name, handle, true)?;
        let to_name = to.dir_op_check(to_name, to_handle, true)?;
        if self.mount_id != to.mount_id {
            return Err(VfsError::CrossDevice);
        }
        let result = self.inode.rename_at(name, &*to.inode, to_name);
        let reloaded = self.after_dir_op(result);
        if !core::ptr::eq(self, to) {
            to.after_dir_op(reloaded)
        } else {
            reloaded
        }
    }

    /// F0: `name`'s kind, flags and size. LOOKUP only.
    #[cfg(feature = "neofs-dir-ops")]
    pub fn stat_at(
        &self,
        name: &[u8],
        handle: FileRights,
    ) -> Result<neofs::dir_ops_logic::StatAtV1, VfsError> {
        let name = self.dir_op_check(name, handle, false)?;
        self.inode.stat_at(name)
    }

    /// F0's checks, in `mount_child`'s order: the inode is a directory, then
    /// the rights, then the name.
    ///
    /// - Every operation needs LOOKUP on both this OpenFile and the handle.
    /// - A mutation (`mutate`) needs WRITE on the handle. A directory's
    ///   OpenFile never holds WRITE: a scope's file rights are its handle's
    ///   (OT-66 K1b, `open_root_temporary`), so a handle narrowed by a derive
    ///   without WRITE can look but not change.
    ///
    /// No generation check: a directory handle names a path (K1b scopes
    /// authorize paths), and its generation moves on any change beneath it.
    /// The filesystem resolves the path under its namespace lock; a
    /// directory that is gone is `Stale` there.
    #[cfg(feature = "neofs-dir-ops")]
    fn dir_op_check<'a>(
        &self,
        name: &'a [u8],
        handle: FileRights,
        mutate: bool,
    ) -> Result<&'a str, VfsError> {
        if self.inode.kind() != InodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        if !self
            .rights
            .intersection(handle)
            .contains(FileRights::LOOKUP)
            || (mutate && !handle.contains(FileRights::WRITE))
        {
            return Err(VfsError::InsufficientRights);
        }
        neofs::dir_ops_logic::check_name(name).map_err(|_| VfsError::Invalid)
    }

    /// After a directory operation through this handle: on success, the
    /// handle holds the new version, so its readdir lists what it just did.
    #[cfg(feature = "neofs-dir-ops")]
    fn after_dir_op(&self, result: Result<(), VfsError>) -> Result<(), VfsError> {
        result?;
        let generation = self.inode.reload_dir()?;
        self.publish_dir_generation(generation);
        Ok(())
    }

    /// The handle takes a reload's generation only if it is newer than the
    /// one it holds (#106 F1): two operations through one shared handle can
    /// finish their reloads in either order, and the older must not win.
    /// The inode publishes its listing the same way, before returning the
    /// generation, so the handle's generation is never newer than its
    /// listing.
    #[cfg(feature = "neofs-dir-ops")]
    fn publish_dir_generation(&self, generation: Option<u32>) {
        let mut held = self.generation.lock();
        if neofs::dir_ops_logic::newer_generation(generation, *held) {
            *held = generation;
        }
    }

    /// F0's F1 discriminator: a delayed generation store, as an operation's
    /// paused `after_dir_op` makes it.
    #[cfg(feature = "neo-test-neofs-dir-ops")]
    pub fn debug_publish_dir_generation(&self, generation: Option<u32>) {
        self.publish_dir_generation(generation);
    }

    /// F0's F1 discriminator: an `after_dir_op` reload whose read is done now
    /// and whose publish (the inode's, then the handle's) runs when the
    /// returned closure is called.
    #[cfg(feature = "neo-test-neofs-dir-ops")]
    pub fn debug_after_dir_op_later(&self) -> impl FnOnce() -> Result<(), VfsError> + '_ {
        let later = self.inode.debug_reload_later();
        move || {
            let generation = later.ok_or(VfsError::Unsupported)?()?;
            self.publish_dir_generation(generation);
            Ok(())
        }
    }

    /// One directory entry, through a file handle whose rights are `handle`:
    /// both the handle and this OpenFile must hold READDIR (a derive narrows
    /// only the handle).
    pub fn readdir_one(
        &self,
        cursor: u64,
        handle: FileRights,
    ) -> Result<Option<VfsDirEntry>, VfsError> {
        self.validate_current()?;
        if self.inode.kind() != InodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        if !self
            .rights
            .intersection(handle)
            .contains(FileRights::READDIR)
        {
            return Err(VfsError::InsufficientRights);
        }
        self.inode.readdir_at(cursor)
    }

    /// Mount a child filesystem under this directory FileCap. Enforces
    /// type-first, rights-second discipline matching BUILD-4A's `readdir_one`
    /// shape: `NotDirectory` precedes `InsufficientRights`. Delegates the
    /// actual mount-table mutation + filesystem construction to
    /// `vfs_mount_at_parent`. Both the handle (`handle`, its rights) and this
    /// OpenFile must hold MOUNT.
    pub fn mount_child(
        &self,
        target: &str,
        source: &str,
        fs_type: u32,
        flags: MountFlags,
        handle: FileRights,
    ) -> Result<(), VfsError> {
        if self.inode.kind() != InodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        if !self.rights.intersection(handle).contains(FileRights::MOUNT) {
            return Err(VfsError::InsufficientRights);
        }
        let parent = self.mount_id.ok_or(VfsError::Invalid)?;
        vfs_mount_at_parent(parent, target, source, fs_type, flags)
    }

    /// Unmount a named child filesystem under this directory FileCap. Same
    /// type-first, rights-second discipline as `mount_child`, and the same
    /// handle-and-OpenFile MOUNT check.
    pub fn umount_child(&self, name: &str, handle: FileRights) -> Result<(), VfsError> {
        if self.inode.kind() != InodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        if !self.rights.intersection(handle).contains(FileRights::MOUNT) {
            return Err(VfsError::InsufficientRights);
        }
        let parent = self.mount_id.ok_or(VfsError::Invalid)?;
        vfs_umount_at_parent(parent, name)
    }
}

impl Drop for OpenFile {
    fn drop(&mut self) {
        if let Some(counter) = self.mount_live.as_ref() {
            counter.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

// === BUILD-4B mount table ==============================================
//
// Every filesystem mount — including the root testfs — is an explicit entry
// in the mount table. Each entry carries a `live_handles` counter that
// `OpenFile` increments when constructed and decrements on Drop, so
// `sys_umount` can answer `EBUSY` precisely without scanning handle tables.
//
// The root mount lives at slot 0 with `parent: None` and name `""`; child
// mounts have `parent: Some(root_id)` and a single-component name.

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
#[repr(transparent)]
pub struct MountId(pub u64);

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
#[repr(transparent)]
pub struct MountFlags(pub u32);

/// Filesystem behind a mount entry. Implemented by `TestFs` (root) and
/// `neofs::NeoFs`; future foreign filesystems plug in here. Methods take
/// `&self` so the kernel can keep a clonable `Arc<dyn MountedFs>` in the
/// mount table.
pub trait MountedFs: Send + Sync {
    fn root(&self) -> Result<Arc<dyn Inode>, VfsError>;
    fn lookup(&self, relative: &str) -> Result<Arc<dyn Inode>, VfsError>;
    fn create_regular(&self, _relative: &str) -> Result<Arc<dyn Inode>, VfsError> {
        Err(VfsError::TypeForbidden)
    }
    /// OT-90 P3a (K2): prepare for power-off. `None`: nothing to do (no
    /// write cache).
    #[cfg(feature = "power-clean")]
    fn shutdown(&self, _stall_ticks: u64) -> Option<neofs::ShutdownReport> {
        None
    }
}

/// OT-90 P3a (K2): shut down every mounted filesystem, in mount order: each
/// freezes writes, drains with a no-progress bound of `stall_ticks` timer
/// ticks, and stops its writeback worker. The table's lock is released
/// before any of it (a drain does block I/O and parks; `MOUNTS` is also
/// taken IF-off, CP-37). Returns each mount's name and report.
#[cfg(feature = "power-clean")]
pub fn shutdown_mounts(stall_ticks: u64) -> Vec<(String, neofs::ShutdownReport)> {
    let mounted: Vec<(String, Arc<dyn MountedFs>)> = {
        let table = lock_mounts();
        table
            .entries
            .iter()
            .map(|entry| (entry.name.clone(), entry.fs.clone()))
            .collect()
    };
    mounted
        .into_iter()
        .filter_map(|(name, fs)| fs.shutdown(stall_ticks).map(|report| (name, report)))
        .collect()
}

const ROOT_MOUNT_ID: MountId = MountId(0);

struct MountEntry {
    id: MountId,
    parent: Option<MountId>,
    name: String,
    fs: Arc<dyn MountedFs>,
    rights_mask: FileRights,
    #[allow(dead_code)]
    flags: MountFlags,
    live_handles: Arc<AtomicUsize>,
}

struct MountTable {
    entries: Vec<MountEntry>,
    next_id: u64,
    /// `cfg(neo-test)`-only typed shortcut to the current NeoFS mount, populated
    /// by `mount_neofs_from_device0_*` so debug helpers can keep their typed API
    /// (`Arc<neofs::NeoFs>`) without forcing `MountedFs` to expose downcasts.
    /// The shortcut is replaced/cleared alongside the table's `neofs` entry; it
    /// is not load-bearing for the syscall path.
    #[cfg(feature = "neo-test")]
    neofs_typed: Option<Arc<neofs::NeoFs>>,
    /// Filesystems the table has let go of, dropped by `MountsGuard` after
    /// `MOUNTS` is released, never under it.
    graveyard: Graveyard,
}

#[derive(Default)]
struct Graveyard {
    fs: Vec<Arc<dyn MountedFs>>,
    #[cfg(feature = "neo-test")]
    typed: Vec<Arc<neofs::NeoFs>>,
}

impl MountTable {
    fn new() -> Self {
        Self {
            entries: vec![MountEntry {
                id: ROOT_MOUNT_ID,
                parent: None,
                name: String::new(),
                fs: Arc::new(TestFs::new()),
                rights_mask: FileRights::KNOWN_BUILD4B,
                flags: MountFlags::default(),
                live_handles: Arc::new(AtomicUsize::new(0)),
            }],
            next_id: 1,
            #[cfg(feature = "neo-test")]
            neofs_typed: None,
            graveyard: Graveyard::default(),
        }
    }

    /// Replace the typed NeoFS shortcut; the old one goes to the graveyard.
    #[cfg(feature = "neo-test")]
    fn set_neofs_typed(&mut self, typed: Option<Arc<neofs::NeoFs>>) {
        if let Some(old) = core::mem::replace(&mut self.neofs_typed, typed) {
            self.graveyard.typed.push(old);
        }
    }

    fn root(&self) -> &MountEntry {
        &self.entries[0]
    }

    fn find_index_under(&self, parent: MountId, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.parent == Some(parent) && e.name == name)
    }

    fn contains_id(&self, id: MountId) -> bool {
        self.entries.iter().any(|entry| entry.id == id)
    }

    /// Resolve `path` (already normalized: no leading `/`, no `.`/`..`) to the
    /// containing mount + remainder. Used by `open_root_temporary` and the
    /// debug helpers that still take string paths. The remainder `&str` is
    /// borrowed from `normalized` (not `self`), so callers can drop the table
    /// lock and still use it.
    fn resolve_path<'a>(&self, normalized: &'a str) -> (usize, &'a str) {
        if normalized.is_empty() {
            return (0, "");
        }
        let (first, remainder) = match normalized.find('/') {
            Some(slash) => (&normalized[..slash], &normalized[slash + 1..]),
            None => (normalized, ""),
        };
        if let Some(idx) = self.find_index_under(ROOT_MOUNT_ID, first) {
            (idx, remainder)
        } else {
            (0, normalized)
        }
    }

    /// Replace an existing root-child mount, or insert a new one. Used by the
    /// internal `mount_neofs_from_device0_*` helpers so the public behavior
    /// (re-mount on repeated calls) is preserved without going through
    /// `sys_mount`. Returns `Err(VfsError::Busy)` if the existing mount has
    /// live handles.
    fn install_root_child(
        &mut self,
        name: &str,
        fs: Arc<dyn MountedFs>,
        rights_mask: FileRights,
        flags: MountFlags,
    ) -> Result<MountId, VfsError> {
        if let Some(idx) = self.find_index_under(ROOT_MOUNT_ID, name) {
            if self.entries[idx].live_handles.load(Ordering::Acquire) > 0 {
                self.graveyard.fs.push(fs);
                return Err(VfsError::Busy);
            }
            let id = self.entries[idx].id;
            let old = core::mem::replace(&mut self.entries[idx].fs, fs);
            self.graveyard.fs.push(old);
            self.entries[idx].rights_mask = rights_mask;
            self.entries[idx].flags = flags;
            self.entries[idx].live_handles.store(0, Ordering::Release);
            return Ok(id);
        }
        let id = MountId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.push(MountEntry {
            id,
            parent: Some(ROOT_MOUNT_ID),
            name: name.to_string(),
            fs,
            rights_mask,
            flags,
            live_handles: Arc::new(AtomicUsize::new(0)),
        });
        Ok(id)
    }

    fn add_child(
        &mut self,
        parent: MountId,
        name: String,
        fs: Arc<dyn MountedFs>,
        rights_mask: FileRights,
        flags: MountFlags,
    ) -> Result<MountId, VfsError> {
        if self.find_index_under(parent, &name).is_some() {
            self.graveyard.fs.push(fs);
            return Err(VfsError::AlreadyExists);
        }
        let id = MountId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.entries.push(MountEntry {
            id,
            parent: Some(parent),
            name,
            fs,
            rights_mask,
            flags,
            live_handles: Arc::new(AtomicUsize::new(0)),
        });
        Ok(id)
    }

    fn remove_child(&mut self, parent: MountId, name: &str) -> Result<MountId, VfsError> {
        let idx = self
            .find_index_under(parent, name)
            .ok_or(VfsError::NotFound)?;
        if self.entries[idx].live_handles.load(Ordering::Acquire) > 0 {
            return Err(VfsError::Busy);
        }
        let entry = self.entries.remove(idx);
        self.graveyard.fs.push(entry.fs);
        Ok(entry.id)
    }
}

/// Process-context mount table: sys_mount/sys_umount/open/readdir mutate or
/// resolve it, and no interrupt handler touches VFS namespace state. Taken
/// only through `lock_mounts`.
static MOUNTS: Lazy<SwitchMutex<MountTable>> = Lazy::new(|| SwitchMutex::new(MountTable::new()));

/// The `MOUNTS` guard. What the table lets go of (an unmounted filesystem, a
/// replaced root child, a rejected mount) waits in its graveyard and is dropped
/// here only after the lock is released. Dropping the last `NeoFsState` stops
/// and joins its writeback kthread (block I/O, a park), and `MOUNTS` is also
/// taken at IF=0, so it must never be held across that (CP-37). Kill-switch:
/// `neo-test-mounts-drop-under-lock-control` drops before releasing, as before.
struct MountsGuard {
    guard: Option<crate::sync::switch_mutex::SwitchMutexGuard<'static, MountTable>>,
}

fn lock_mounts() -> MountsGuard {
    MountsGuard {
        guard: Some(MOUNTS.lock()),
    }
}

impl core::ops::Deref for MountsGuard {
    type Target = MountTable;
    fn deref(&self) -> &MountTable {
        self.guard.as_ref().expect("MountsGuard used after drop")
    }
}

impl core::ops::DerefMut for MountsGuard {
    fn deref_mut(&mut self) -> &mut MountTable {
        self.guard.as_mut().expect("MountsGuard used after drop")
    }
}

impl Drop for MountsGuard {
    fn drop(&mut self) {
        let Some(mut guard) = self.guard.take() else {
            return;
        };
        let graveyard = core::mem::take(&mut guard.graveyard);
        #[cfg(not(feature = "neo-test-mounts-drop-under-lock-control"))]
        {
            drop(guard);
            drop(graveyard);
        }
        #[cfg(feature = "neo-test-mounts-drop-under-lock-control")]
        {
            drop(graveyard);
            drop(guard);
        }
    }
}

#[cfg(all(feature = "boot-neofs", feature = "neo-test"))]
static BOOT_NEOFS_OUTCOME: AtomicU8 = AtomicU8::new(0);

// OT-78 observability is carried only by the scenarios that assert the count;
// other neo-test builds retain the ordinary probe path without this atomic.
#[cfg(any(
    feature = "neo-test-product-integration",
    feature = "neo-test-boot-neofs-no-block-device",
    feature = "neo-test-boot-neofs-no-valid-superblock",
    feature = "neo-test-boot-neofs-unexpected-error"
))]
static NEOFS_PROBE_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "neo-test-boot-neofs-unexpected-error")]
static BOOT_NEOFS_BUSY_HOLD: Lazy<SwitchMutex<Option<Arc<OpenFile>>>> =
    Lazy::new(|| SwitchMutex::new(None));

pub fn init() {
    drop(lock_mounts());
    serial_println!("[ kernel ] fs: init (Phase 3 BUILD-4B mount table; root testfs)");
}

/// OT-78 catch-all-arm setup. Install a harmless in-memory filesystem at
/// `/neofs` and keep one live directory handle so the real boot mount reaches
/// the existing `Busy -> BlockDevice` error path after selecting a valid NeoFS
/// device. This is test-only and happens before the boot mount.
#[cfg(feature = "neo-test-boot-neofs-unexpected-error")]
pub fn debug_prepare_boot_neofs_busy_mount() -> Result<(), VfsError> {
    let fs: Arc<dyn MountedFs> = Arc::new(TestFs::new());
    lock_mounts().install_root_child(
        "neofs",
        fs,
        FileRights::KNOWN_BUILD4B,
        MountFlags::default(),
    )?;
    let hold = open_root_temporary(
        "/neofs",
        OPEN_ROOT_TEMPORARY,
        FileRights::from_bits(FileRights::LOOKUP.bits() | FileRights::READDIR.bits()),
    )?;
    *BOOT_NEOFS_BUSY_HOLD.lock() = Some(hold);
    Ok(())
}

/// Resolve a normalized path to (inode, mount id, mount live-handle counter).
/// The counter is what `OpenFile` increments to keep `sys_umount` honest;
/// the mount id is what `sys_mount`/`sys_umount` use to identify the parent
/// mount when a directory FileCap is provided.
type ResolveResult = (Arc<dyn Inode>, MountId, Arc<AtomicUsize>, FileRights);

fn resolve_path_to_inode(
    normalized: &str,
    create_regular: bool,
) -> Result<ResolveResult, VfsError> {
    // `sys_open` runs this IF-on, but `MOUNTS` is also taken at IF=0 (mount,
    // umount, model provenance checks), so it is held only with interrupts off:
    // a holder can never be preempted while an IF=0 taker spins (CP-37).
    let (remainder, fs, id, live, rights_mask) = crate::arch::x86_64::without_interrupts(|| {
        let mounts = lock_mounts();
        let (idx, remainder) = mounts.resolve_path(normalized);
        let entry = &mounts.entries[idx];
        (
            remainder,
            entry.fs.clone(),
            entry.id,
            entry.live_handles.clone(),
            entry.rights_mask,
        )
    });
    let inode = if create_regular {
        fs.create_regular(remainder)?
    } else {
        fs.lookup(remainder)?
    };
    Ok((inode, id, live, rights_mask))
}

fn normalized_targets_root_child_mount(normalized: &str) -> bool {
    if normalized.is_empty() {
        return false;
    }
    let first = match normalized.find('/') {
        Some(slash) => &normalized[..slash],
        None => normalized,
    };
    // IF-off for the same reason as `resolve_path_to_inode`.
    crate::arch::x86_64::without_interrupts(|| {
        lock_mounts()
            .find_index_under(ROOT_MOUNT_ID, first)
            .is_some()
    })
}

fn find_neofs_block_device()
-> Result<(usize, Arc<dyn crate::drivers::block::BlockDevice>), neofs::NeoFsError> {
    #[cfg(any(
        feature = "neo-test-product-integration",
        feature = "neo-test-boot-neofs-no-block-device",
        feature = "neo-test-boot-neofs-no-valid-superblock",
        feature = "neo-test-boot-neofs-unexpected-error"
    ))]
    NEOFS_PROBE_ATTEMPTS.store(0, Ordering::Release);
    let count = crate::drivers::block::device_count();
    if count == 0 {
        return Err(neofs::NeoFsError::NoBlockDevice);
    }
    // OT-87 R3: a LogansOS NeoFS partition, on any device, before any whole
    // device.
    #[cfg(feature = "neofs-partition")]
    if let Some(found) = find_neofs_partition(count) {
        return Ok(found);
    }
    for index in 0..count {
        #[cfg(any(
            feature = "neo-test-product-integration",
            feature = "neo-test-boot-neofs-no-block-device",
            feature = "neo-test-boot-neofs-no-valid-superblock",
            feature = "neo-test-boot-neofs-unexpected-error"
        ))]
        NEOFS_PROBE_ATTEMPTS.fetch_add(1, Ordering::AcqRel);
        let Some(device) = crate::drivers::block::get_device(index) else {
            continue;
        };
        let ident = device.identity();
        serial_println!(
            "[ kernel ] fs: probing NeoFS device index={} driver={} name={} pci_bdf={:?}",
            index,
            ident.driver,
            ident.name,
            ident.pci_bdf
        );
        if neofs::NeoFs::probe(&device).is_ok() {
            return Ok((index, device));
        }
    }
    Err(neofs::NeoFsError::NoValidSuperblock)
}

/// OT-87 R3: the first LogansOS-typed partition, without the INCOMPLETE
/// attribute (an install that did not finish, §4.7), whose NeoFS probe
/// passes, as a `PartitionDevice` over the registered (guarded) device.
#[cfg(feature = "neofs-partition")]
fn find_neofs_partition(
    count: usize,
) -> Option<(usize, Arc<dyn crate::drivers::block::BlockDevice>)> {
    use crate::drivers::block::gpt_logic::{ATTR_INCOMPLETE, LOGANSOS_NEOFS_TYPE_GUID};
    use crate::drivers::block::partition::PartitionDevice;
    for index in 0..count {
        let Some(device) = crate::drivers::block::get_device(index) else {
            continue;
        };
        let Some(gpt) = crate::drivers::block::read_gpt(device.as_ref()) else {
            continue;
        };
        for entry in &gpt.entries {
            if entry.type_guid != LOGANSOS_NEOFS_TYPE_GUID {
                continue;
            }
            if entry.attributes & ATTR_INCOMPLETE != 0 {
                serial_println!(
                    "[ kernel ] fs: NeoFS partition skipped device_index={} partition={} reason=incomplete",
                    index,
                    entry.index
                );
                continue;
            }
            let Some(part) = PartitionDevice::new(device.clone(), &gpt, entry) else {
                continue;
            };
            let part: Arc<dyn crate::drivers::block::BlockDevice> = Arc::new(part);
            if neofs::NeoFs::probe(&part).is_ok() {
                serial_println!(
                    "[ kernel ] fs: NeoFS partition device_index={} partition={} first_lba={} lba_count={}",
                    index,
                    entry.index,
                    entry.first_lba,
                    entry.last_lba + 1 - entry.first_lba
                );
                return Some((index, part));
            }
            serial_println!(
                "[ kernel ] fs: NeoFS partition skipped device_index={} partition={} reason=probe",
                index,
                entry.index
            );
        }
    }
    None
}

pub fn mount_neofs_from_device0_readonly() -> Result<neofs::MountReport, neofs::NeoFsError> {
    let (device_index, device) = find_neofs_block_device()?;
    let ident = device.identity().clone();
    let fs = neofs::NeoFs::mount_readonly(device)?;
    let report = fs.report();
    let typed: Arc<neofs::NeoFs> = Arc::new(fs);
    let fs_arc: Arc<dyn MountedFs> = typed.clone();
    let mut mounts = lock_mounts();
    mounts
        .install_root_child("neofs", fs_arc, FileRights::KNOWN_BUILD4B, MountFlags(0))
        .map_err(|_| neofs::NeoFsError::BlockDevice)?;
    #[cfg(feature = "neo-test")]
    {
        mounts.set_neofs_typed(Some(typed));
    }
    drop(mounts);
    serial_println!(
        "[ kernel ] fs: mounted NeoFS readonly at /neofs device_index={} driver={} name={} selected_superblock={} generation={}",
        device_index,
        ident.driver,
        ident.name,
        report.selected_superblock_slot,
        report.generation
    );
    Ok(report)
}

pub fn mount_neofs_from_device0_readwrite() -> Result<neofs::MountReport, neofs::NeoFsError> {
    let (device_index, device) = find_neofs_block_device()?;
    let ident = device.identity().clone();
    let fs = neofs::NeoFs::mount_readwrite(device)?;
    let report = fs.report();
    let typed: Arc<neofs::NeoFs> = Arc::new(fs);
    let fs_arc: Arc<dyn MountedFs> = typed.clone();
    let mut mounts = lock_mounts();
    mounts
        .install_root_child("neofs", fs_arc, FileRights::KNOWN_BUILD4B, MountFlags(0))
        .map_err(|_| neofs::NeoFsError::BlockDevice)?;
    #[cfg(feature = "neo-test")]
    {
        mounts.set_neofs_typed(Some(typed));
    }
    drop(mounts);
    serial_println!(
        "[ kernel ] fs: mounted NeoFS readwrite at /neofs device_index={} driver={} name={} selected_superblock={} generation={}",
        device_index,
        ident.driver,
        ident.name,
        report.selected_superblock_slot,
        report.generation
    );
    Ok(report)
}

#[cfg(feature = "boot-neofs")]
pub fn mount_boot_neofs_from_device0_readwrite() {
    serial_println!("[ kernel ] boot-neofs: readwrite mount requested");
    match mount_neofs_from_device0_readwrite() {
        Ok(report) => {
            #[cfg(feature = "neo-test")]
            BOOT_NEOFS_OUTCOME.store(1, Ordering::Release);
            serial_println!(
                "[ kernel ] boot-neofs: mounted /neofs readwrite selected_superblock={} generation={} valid_superblocks={} rejected_superblocks={}",
                report.selected_superblock_slot,
                report.generation,
                report.valid_superblocks,
                report.rejected_superblocks
            );
        }
        Err(neofs::NeoFsError::NoBlockDevice) => {
            #[cfg(feature = "neo-test")]
            BOOT_NEOFS_OUTCOME.store(2, Ordering::Release);
            serial_println!("[ kernel ] boot-neofs: no block devices; boot continues");
        }
        Err(neofs::NeoFsError::NoValidSuperblock) | Err(neofs::NeoFsError::InvalidSuperblock) => {
            #[cfg(feature = "neo-test")]
            BOOT_NEOFS_OUTCOME.store(3, Ordering::Release);
            serial_println!("[ kernel ] boot-neofs: no valid NeoFS superblock; boot continues");
        }
        Err(error) => {
            #[cfg(feature = "neo-test")]
            BOOT_NEOFS_OUTCOME.store(4, Ordering::Release);
            serial_println!(
                "[ kernel ] boot-neofs: mount failed {:?}; boot continues",
                error
            );
        }
    }
}

#[cfg(all(feature = "boot-neofs", feature = "neo-test"))]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BootNeoFsOutcome {
    NotAttempted,
    Mounted,
    NoBlockDevice,
    NoValidSuperblock,
    UnexpectedError,
}

#[cfg(all(feature = "boot-neofs", feature = "neo-test"))]
pub fn debug_boot_neofs_outcome() -> BootNeoFsOutcome {
    match BOOT_NEOFS_OUTCOME.load(Ordering::Acquire) {
        1 => BootNeoFsOutcome::Mounted,
        2 => BootNeoFsOutcome::NoBlockDevice,
        3 => BootNeoFsOutcome::NoValidSuperblock,
        4 => BootNeoFsOutcome::UnexpectedError,
        _ => BootNeoFsOutcome::NotAttempted,
    }
}

#[cfg(any(
    feature = "neo-test-product-integration",
    feature = "neo-test-boot-neofs-no-block-device",
    feature = "neo-test-boot-neofs-no-valid-superblock",
    feature = "neo-test-boot-neofs-unexpected-error"
))]
pub fn debug_neofs_probe_attempts() -> usize {
    NEOFS_PROBE_ATTEMPTS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
fn mounted_neofs_for_debug() -> Result<Arc<neofs::NeoFs>, VfsError> {
    lock_mounts().neofs_typed.clone().ok_or(VfsError::NotFound)
}

fn validate_mount_name(name: &str) -> Result<(), VfsError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(VfsError::Invalid);
    }
    Ok(())
}

fn parse_block_device_source(source: &str) -> Result<usize, VfsError> {
    let Some(rest) = source.strip_prefix("device") else {
        return Err(VfsError::Invalid);
    };
    if rest.is_empty() {
        return Err(VfsError::Invalid);
    }

    let mut index = 0usize;
    for byte in rest.bytes() {
        if !byte.is_ascii_digit() {
            return Err(VfsError::Invalid);
        }
        index = index
            .checked_mul(10)
            .and_then(|value| value.checked_add((byte - b'0') as usize))
            .ok_or(VfsError::Invalid)?;
    }
    Ok(index)
}

/// The block device a FAT mount's `source` names: `"deviceN"` (the whole
/// device, as before) or, with `mbr-partitions` (L3a, G7), `"deviceN:pM"`,
/// MBR partition M of device N. Without the feature a `:pM` source is
/// `Invalid`, as before (the kill-switch).
fn fat_source_device(
    source: &str,
) -> Result<Arc<dyn crate::drivers::block::BlockDevice>, VfsError> {
    #[cfg(feature = "mbr-partitions")]
    {
        let (device_index, part) =
            crate::drivers::block::mbr_logic::parse_source(source).ok_or(VfsError::Invalid)?;
        let device = crate::drivers::block::get_device(device_index).ok_or(VfsError::NotFound)?;
        match part {
            None => Ok(device),
            Some(index) => {
                let part = crate::drivers::block::mbr_partition(device, index)
                    .ok_or(VfsError::NotFound)?;
                serial_println!(
                    "[ kernel ] fs: FAT source device_index={} mbr_partition={} first_lba={} lba_count={}",
                    device_index,
                    part.entry_index(),
                    part.first_lba(),
                    part.lba_count()
                );
                Ok(Arc::new(part))
            }
        }
    }
    #[cfg(not(feature = "mbr-partitions"))]
    {
        let device_index = parse_block_device_source(source)?;
        crate::drivers::block::get_device(device_index).ok_or(VfsError::NotFound)
    }
}

fn parse_block_device_source_or_default(
    source: &str,
    default_index: usize,
) -> Result<usize, VfsError> {
    if source.is_empty() {
        Ok(default_index)
    } else {
        parse_block_device_source(source)
    }
}

fn vfs_mount_at_parent(
    parent: MountId,
    target: &str,
    source: &str,
    fs_type: u32,
    flags: MountFlags,
) -> Result<(), VfsError> {
    validate_mount_name(target)?;
    if flags.0 & !MOUNT_KNOWN_FLAGS_BUILD4C != 0 {
        return Err(VfsError::Invalid);
    }

    let (fs, _neofs_typed, rights_mask): (
        Arc<dyn MountedFs>,
        Option<Arc<neofs::NeoFs>>,
        FileRights,
    ) = match fs_type {
        1 | 2 => {
            let device_index = parse_block_device_source_or_default(source, 0)?;
            let device =
                crate::drivers::block::get_device(device_index).ok_or(VfsError::NotFound)?;
            let neofs_result = if fs_type == 2 {
                neofs::NeoFs::mount_readwrite(device)
            } else {
                neofs::NeoFs::mount_readonly(device)
            };
            let neofs = neofs_result.map_err(|_| VfsError::NotFound)?;
            let typed: Arc<neofs::NeoFs> = Arc::new(neofs);
            let fs: Arc<dyn MountedFs> = typed.clone();
            (fs, Some(typed), FileRights::KNOWN_BUILD4B)
        }
        3 => {
            let device = fat_source_device(source)?;
            #[cfg(feature = "fat-fs")]
            let fs: Arc<dyn MountedFs> =
                Arc::new(fat::Fat::mount_readonly(device).map_err(|_| VfsError::NotFound)?);
            #[cfg(not(feature = "fat-fs"))]
            let fs: Arc<dyn MountedFs> =
                Arc::new(fat32::Fat32::mount_readonly(device).map_err(|_| VfsError::NotFound)?);
            let denied = if flags.0 & MOUNT_FLAG_EXEC_ALLOWED != 0 {
                FileRights::WRITE
            } else {
                FileRights::from_bits(FileRights::WRITE.bits() | FileRights::EXECUTE.bits())
            };
            (fs, None, FileRights::KNOWN_BUILD4B.without(denied))
        }
        4 => {
            let device_index = parse_block_device_source(source)?;
            let device =
                crate::drivers::block::get_device(device_index).ok_or(VfsError::NotFound)?;
            let ext4 = ext4::Ext4::mount_readonly(device).map_err(|_| VfsError::NotFound)?;
            let fs: Arc<dyn MountedFs> = Arc::new(ext4);
            let denied = if flags.0 & MOUNT_FLAG_EXEC_ALLOWED != 0 {
                FileRights::WRITE
            } else {
                FileRights::from_bits(FileRights::WRITE.bits() | FileRights::EXECUTE.bits())
            };
            (fs, None, FileRights::KNOWN_BUILD4B.without(denied))
        }
        _ => return Err(VfsError::Invalid),
    };

    let mut mounts = lock_mounts();
    mounts.add_child(parent, target.to_string(), fs, rights_mask, flags)?;
    #[cfg(feature = "neo-test")]
    if parent == ROOT_MOUNT_ID
        && target == "neofs"
        && let Some(typed) = _neofs_typed
    {
        mounts.set_neofs_typed(Some(typed));
    }
    Ok(())
}

fn vfs_umount_at_parent(parent: MountId, name: &str) -> Result<(), VfsError> {
    validate_mount_name(name)?;
    let mut mounts = lock_mounts();
    mounts.remove_child(parent, name)?;
    #[cfg(feature = "neo-test")]
    if parent == ROOT_MOUNT_ID && name == "neofs" {
        mounts.set_neofs_typed(None);
    }
    Ok(())
}

#[cfg(feature = "neo-test")]
pub fn debug_neofs_mount_exists() -> bool {
    let mounts = lock_mounts();
    mounts.find_index_under(ROOT_MOUNT_ID, "neofs").is_some()
}

/// Direct mount-table entry used by the BUILD-4B test scenarios. Calls the
/// same `vfs_mount_at_parent` logic the syscall handler uses; exposed at the
/// module boundary so kernel-side test harness code does not depend on
/// `OpenFile` internals.
#[cfg(feature = "neo-test-phase3-mount-table")]
pub fn debug_vfs_mount_at_parent(
    parent: MountId,
    target: &str,
    source: &str,
    fs_type: u32,
    flags: MountFlags,
) -> Result<(), VfsError> {
    vfs_mount_at_parent(parent, target, source, fs_type, flags)
}

#[cfg(feature = "neo-test-phase3-mount-table")]
pub fn debug_root_mount_id() -> MountId {
    ROOT_MOUNT_ID
}

#[cfg(feature = "neo-test")]
pub fn debug_neofs_generation() -> Result<u64, VfsError> {
    Ok(mounted_neofs_for_debug()?.debug_generation())
}

#[cfg(feature = "neo-test")]
pub fn debug_neofs_regular_blocks(path: &str) -> Result<([u64; 8], u64), VfsError> {
    let normalized = normalize_absolute_path(path)?;
    let rest = normalized.strip_prefix("neofs/").ok_or(VfsError::Invalid)?;
    mounted_neofs_for_debug()?.debug_regular_blocks(rest)
}

#[cfg(feature = "neo-test")]
pub fn debug_neofs_cache_stats() -> Result<neofs::CacheStats, VfsError> {
    Ok(mounted_neofs_for_debug()?.debug_cache_stats())
}

/// OT-92's kill-switch leg: drop cached blocks `first..first + count` as a
/// large write's allocation does (`NeoFsBlockCache::forget_range`).
#[cfg(all(feature = "neo-test", feature = "neofs-large-write"))]
pub fn debug_neofs_forget_range(first: u64, count: u64) -> Result<(), VfsError> {
    mounted_neofs_for_debug()?.debug_forget_range(first, count);
    Ok(())
}

#[cfg(feature = "neo-test")]
pub fn debug_neofs_set_writeback_hold(hold: bool) -> Result<(), VfsError> {
    mounted_neofs_for_debug()?.debug_set_writeback_hold(hold);
    Ok(())
}

/// The `entries` holder's gate (`phase3_neofs_syscall_if_on`): the holder
/// sets `HELD` once it has the lock and swaps in `RELEASED` just before it
/// drops it; the caller claims `HELD -> CALLING` with interrupts off,
/// immediately before its call. A won claim proves the lock was held when the
/// call began.
#[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
pub const ENTRIES_GATE_IDLE: u8 = 0;
#[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
pub const ENTRIES_GATE_HELD: u8 = 1;
#[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
pub const ENTRIES_GATE_CALLING: u8 = 2;
#[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
pub const ENTRIES_GATE_RELEASED: u8 = 3;

/// Hold the mounted NeoFS's `entries` lock for `ticks` ticks, yielding with
/// interrupts on (a preempted `sys_write` in `commit_child_update`), with
/// `gate` as above. Returns the tick at which it released, and whether a
/// caller had claimed the gate before the release.
#[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
pub fn debug_neofs_hold_entries_yielding(
    ticks: u64,
    gate: &core::sync::atomic::AtomicU8,
) -> Result<(u64, bool), VfsError> {
    Ok(mounted_neofs_for_debug()?.debug_hold_entries_yielding(ticks, gate))
}

#[cfg(feature = "neo-test")]
pub fn debug_neofs_wait_clean_cache(max_yields: usize) -> Result<bool, VfsError> {
    Ok(mounted_neofs_for_debug()?.debug_wait_clean_cache(max_yields))
}

#[cfg(feature = "neo-test")]
pub fn debug_neofs_stop_writeback_worker() -> Result<neofs::WriteBackWorkerStopReport, VfsError> {
    Ok(mounted_neofs_for_debug()?.debug_stop_writeback_worker())
}

#[cfg(feature = "neo-test-phase3-neofs-crash-mid-create-recovery")]
pub fn debug_neofs_inject_create_before_superblock(path: &str) -> Result<(), VfsError> {
    let normalized = normalize_absolute_path(path)?;
    let rest = normalized.strip_prefix("neofs/").ok_or(VfsError::Invalid)?;
    mounted_neofs_for_debug()?.debug_inject_create_before_superblock(rest)
}

#[cfg(feature = "neo-test-phase3-neofs-crash-mid-overwrite-recovery")]
pub fn debug_neofs_inject_overwrite_before_superblock(
    path: &str,
    input: &[u8],
) -> Result<(), VfsError> {
    let normalized = normalize_absolute_path(path)?;
    let rest = normalized.strip_prefix("neofs/").ok_or(VfsError::Invalid)?;
    mounted_neofs_for_debug()?.debug_inject_overwrite_before_superblock(rest, input)
}

/// W1's cut scenario: everything a replace of `path` writes, but no publish.
#[cfg(feature = "neo-test-neofs-file-replace")]
pub fn debug_neofs_replace_before_publish(path: &str, bytes: &[u8]) -> Result<(), VfsError> {
    let rest = path.strip_prefix("/neofs/").ok_or(VfsError::Invalid)?;
    mounted_neofs_for_debug()?.debug_replace_before_publish(rest, bytes)
}

/// W1's byte oracle: `path`'s inode block and its 256 bytes on the device,
/// and any block's 4096 bytes on the device (the cache drained first).
#[cfg(any(
    feature = "neo-test-neofs-file-replace",
    feature = "neo-test-neofs-dir-ops"
))]
pub fn debug_neofs_raw_inode(path: &str) -> Result<(u64, alloc::vec::Vec<u8>), VfsError> {
    let rest = path.strip_prefix("/neofs/").ok_or(VfsError::Invalid)?;
    mounted_neofs_for_debug()?.debug_raw_inode(rest)
}

#[cfg(any(
    feature = "neo-test-neofs-file-replace",
    feature = "neo-test-neofs-dir-ops"
))]
pub fn debug_neofs_raw_block(block: u64) -> Result<alloc::vec::Vec<u8>, VfsError> {
    mounted_neofs_for_debug()?.debug_raw_block(block)
}

/// F0's F2 leg: write everything cached and drop it, so the next reads miss.
#[cfg(feature = "neo-test-neofs-dir-ops")]
pub fn debug_neofs_cold_cache() -> Result<(), VfsError> {
    mounted_neofs_for_debug()?.debug_cold_cache()
}

/// F0's F2 leg: (device reads during F0 operations, of them made with the
/// namespace lock held, operations that read under the lock after repeated
/// commits), counted since the last call.
#[cfg(feature = "neo-test-neofs-dir-ops")]
pub fn debug_neofs_dir_op_reads() -> (u64, u64, u64) {
    neofs::debug_dir_op_reads()
}

/// The cut after the commit barrier: the unpublished overwrite's blocks
/// written and flushed, no superblock (`NeoFsBlockCache::debug_drain_to_barrier`).
#[cfg(any(
    feature = "neo-test-phase3-neofs-crash-mid-overwrite-recovery",
    feature = "neo-test-neofs-file-replace",
    feature = "neo-test-neofs-dir-ops"
))]
pub fn debug_neofs_drain_to_barrier() -> Result<u64, VfsError> {
    mounted_neofs_for_debug()?.debug_drain_to_barrier()
}

#[cfg(feature = "neo-test-phase3-neofs-superblock-rotation-recovery")]
pub fn debug_neofs_write_invalid_superblock_copy(slot: usize) -> Result<(), VfsError> {
    mounted_neofs_for_debug()?.debug_write_invalid_superblock_copy(slot)
}

#[cfg(any(
    feature = "neo-test-phase3-neofs-3c",
    feature = "neo-test-neofs-file-replace",
    feature = "neo-test-neofs-dir-ops"
))]
pub fn debug_neofs_free_map_stats() -> Result<neofs::FreeMapStats, VfsError> {
    mounted_neofs_for_debug()?.debug_free_map_stats()
}

#[cfg(feature = "neo-test-phase3-neofs-3c")]
pub fn debug_neofs_run_quiescent_gc() -> Result<neofs::GcStats, VfsError> {
    mounted_neofs_for_debug()?.debug_run_quiescent_gc()
}

#[cfg(feature = "neo-test-phase3-neofs-3c")]
pub fn debug_neofs_inject_gc_before_superblock() -> Result<(), VfsError> {
    mounted_neofs_for_debug()?.debug_inject_gc_before_superblock()
}

/// Temporary BUILD-1 root-open ABI backing:
/// `sys_open(path_ptr, path_len, flags, requested_rights_u32)`.
///
/// The target model is capability-rooted open:
/// `sys_open(dir_cap, path, flags, requested_rights)`.
pub fn open_root_temporary(
    path: &str,
    flags: u32,
    requested: FileRights,
) -> Result<Arc<OpenFile>, VfsError> {
    let (create_regular, scope_dir) = match flags {
        OPEN_ROOT_TEMPORARY => (false, false),
        OPEN_CREATE_REGULAR => (true, false),
        #[cfg(feature = "cap-rooted-open")]
        OPEN_SCOPE_DIR => (false, true),
        _ => return Err(VfsError::Invalid),
    };
    if requested.is_empty() {
        return Err(VfsError::Invalid);
    }
    if !requested.is_subset_of(FileRights::KNOWN_BUILD4B) {
        return Err(VfsError::InsufficientRights);
    }
    let normalized = normalize_absolute_path(path)?;
    if create_regular && !normalized_targets_root_child_mount(&normalized) {
        // BUILD-1 quirk: bare-root CREATE_REGULAR reports Invalid, while child
        // mounts may decide whether their filesystem supports creation.
        return Err(VfsError::Invalid);
    }
    let (inode, mount_id, mount_live, rights_mask) =
        resolve_path_to_inode(&normalized, create_regular)?;
    if inode.kind() == InodeKind::Symlink {
        return Err(VfsError::TypeForbidden);
    }
    // OT-66 K1b: a scope is a directory; its file rights are the handle's
    // (the bound on opens beneath it), not the directory's.
    let requested = if scope_dir {
        if inode.kind() != InodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        FileRights::from_bits(
            requested.bits()
                & !(FileRights::READ.bits()
                    | FileRights::WRITE.bits()
                    | FileRights::EXECUTE.bits()),
        )
    } else {
        requested
    };
    if inode.kind() == InodeKind::Directory
        && (requested.bits()
            & (FileRights::READ.bits() | FileRights::WRITE.bits() | FileRights::EXECUTE.bits()))
            != 0
    {
        return Err(VfsError::TypeForbidden);
    }
    let effective_rights = inode.available_rights().intersection(rights_mask);
    if !requested.is_subset_of(effective_rights) {
        return Err(VfsError::InsufficientRights);
    }
    #[cfg(feature = "cap-rooted-open")]
    {
        let mut file =
            OpenFile::new_with_mount(inode, requested, Some(mount_id), Some(mount_live))?;
        file.vfs_root = normalized.is_empty() && mount_id == ROOT_MOUNT_ID;
        if file.is_dir {
            file.scope = Some(normalized);
        }
        Ok(Arc::new(file))
    }
    #[cfg(not(feature = "cap-rooted-open"))]
    Ok(Arc::new(OpenFile::new_with_mount(
        inode,
        requested,
        Some(mount_id),
        Some(mount_live),
    )?))
}

#[cfg(feature = "neo-test-phase3-neofs-2e")]
pub fn debug_open_root_temporary_for_policy(
    path: &str,
    requested: FileRights,
) -> Result<Arc<OpenFile>, VfsError> {
    let normalized = normalize_absolute_path(path)?;
    let (inode, mount_id, mount_live, _rights_mask) = resolve_path_to_inode(&normalized, false)?;
    Ok(Arc::new(OpenFile::new_with_mount(
        inode,
        requested,
        Some(mount_id),
        Some(mount_live),
    )?))
}

/// Phase 7.5 CB-3 (ISC-4): a synthetic read-only inode for an interface, addressed
/// as `/net/if/<name>`. Opening it through `sys_open` mints a `Handle::File`
/// capability that enters the I-15 derivation/grant graph exactly like any file cap
/// — **zero new `Handle` kind**. Reading it yields the interface's current
/// `<name> <up|down|unknown>` status line (so the cap is a usable, observable file).
struct InterfaceInode {
    /// The InterfaceId / device-registry index this node names (ISC-1 binding).
    id: u32,
}

impl InterfaceInode {
    fn new(id: u32) -> Self {
        Self { id }
    }

    /// `"<name> <up|down|unknown>\n"` from the registry (the ISC-2 single source of
    /// truth), rendered fresh on each read so the status reflects live link-state.
    fn status_bytes(&self) -> Vec<u8> {
        let name = crate::net::identity::name(self.id).unwrap_or_default();
        let link = match crate::net::identity::link_state(self.id) {
            crate::net::LinkState::Up => "up",
            crate::net::LinkState::Down => "down",
            crate::net::LinkState::Unknown => "unknown",
        };
        let mut s = String::new();
        s.push_str(&name);
        s.push(' ');
        s.push_str(link);
        s.push('\n');
        s.into_bytes()
    }
}

impl Inode for InterfaceInode {
    fn kind(&self) -> InodeKind {
        InodeKind::Regular
    }

    fn available_rights(&self) -> FileRights {
        FileRights::READ
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> Result<usize, VfsError> {
        let data = self.status_bytes();
        if offset >= data.len() {
            return Ok(0);
        }
        let n = cmp::min(out.len(), data.len() - offset);
        out[..n].copy_from_slice(&data[offset..offset + n]);
        Ok(n)
    }

    fn write_at(&self, _offset: usize, _input: &[u8]) -> Result<usize, VfsError> {
        // An interface status node is read-only; administrative up/down would be a
        // separate, capability-gated op (deferred — see ADR-068).
        Err(VfsError::InsufficientRights)
    }
}

/// Phase 7.5 CB-3 (ISC-4): the synthetic `net` filesystem. A single-level mount at
/// `/net` whose `lookup` parses `if/<name>` into an [`InterfaceInode`] by consulting
/// the InterfaceId registry. The directory levels (`/net`, `/net/if`) reuse the
/// existing empty-directory inode. Mounted only when `interface-identity` is on, so
/// with it off `/net/...` is `NotFound` — bit-for-bit prior behavior.
struct NetIfFs;

impl MountedFs for NetIfFs {
    fn root(&self) -> Result<Arc<dyn Inode>, VfsError> {
        Ok(Arc::new(TestInode::directory(&[])))
    }

    fn lookup(&self, relative: &str) -> Result<Arc<dyn Inode>, VfsError> {
        // `relative` is the path under the `net` mount: "", "if", or "if/<name>".
        match relative {
            "" | "if" => Ok(Arc::new(TestInode::directory(&[]))),
            other => {
                let name = other.strip_prefix("if/").ok_or(VfsError::NotFound)?;
                if name.is_empty() || name.contains('/') {
                    return Err(VfsError::NotFound);
                }
                let id = crate::net::identity::id_for_name(name).ok_or(VfsError::NotFound)?;
                Ok(Arc::new(InterfaceInode::new(id)))
            }
        }
    }
}

/// B1b: an empty, listable directory inode for a synthetic mount's
/// intermediate levels (the `/boot` module namespace).
#[cfg(feature = "model-boot-module")]
pub fn empty_directory_inode() -> Arc<dyn Inode> {
    Arc::new(TestInode::directory(&[]))
}

/// B1b: install a synthetic read-only filesystem as a root-child mount
/// (`/boot`), with the same rights mask as `/net`.
#[cfg(feature = "model-boot-module")]
pub fn install_read_only_root_child(name: &str, fs: Arc<dyn MountedFs>) -> Result<(), VfsError> {
    let rights_mask = FileRights::from_bits(
        FileRights::READ.bits() | FileRights::LOOKUP.bits() | FileRights::READDIR.bits(),
    );
    lock_mounts()
        .install_root_child(name, fs, rights_mask, MountFlags::default())
        .map(|_| ())
}

/// Install the synthetic `/net` interface namespace as a root-child mount (ISC-4).
/// Called once at boot after `fs::init`. Inert when `interface-identity` is off (the
/// `net` mount never appears ⇒ `/net/if/<name>` resolves to `NotFound`, bit-for-bit
/// pre-identity).
pub fn mount_net_interface_fs() {
    if !crate::net::identity_logic::INTERFACE_IDENTITY_ENABLED {
        return;
    }
    let fs: Arc<dyn MountedFs> = Arc::new(NetIfFs);
    let rights_mask = FileRights::from_bits(
        FileRights::READ.bits() | FileRights::LOOKUP.bits() | FileRights::READDIR.bits(),
    );
    let installed = lock_mounts().install_root_child("net", fs, rights_mask, MountFlags::default());
    match installed {
        Ok(_) => {
            serial_println!("[ kernel ] fs: mounted synthetic /net interface namespace (CB-3)");
        }
        Err(_) => {
            serial_println!("[ kernel ] fs: /net interface namespace already mounted");
        }
    }
}

struct TestFs {
    hello: Arc<TestInode>,
    scratch: Arc<TestInode>,
    root: Arc<TestInode>,
    directory: Arc<TestInode>,
    symlink: Arc<TestInode>,
}

impl TestFs {
    fn new() -> Self {
        #[cfg(any(
            feature = "neo-test-arthur-agentic-actions",
            feature = "neo-test-arthur-reversibility",
            feature = "neo-test-arthur-multistep-agentic"
        ))]
        const SCRATCH_INITIAL: &[u8] = b"NeoPlutarch scratch base";
        #[cfg(not(any(
            feature = "neo-test-arthur-agentic-actions",
            feature = "neo-test-arthur-reversibility",
            feature = "neo-test-arthur-multistep-agentic"
        )))]
        const SCRATCH_INITIAL: &[u8] = b"";

        Self {
            hello: Arc::new(TestInode::regular(
                FileRights::READ,
                b"NeoPlutarch testfs hello\n",
            )),
            scratch: Arc::new(TestInode::regular(FileRights::READ_WRITE, SCRATCH_INITIAL)),
            // Root directory of testfs is the mount-authority surface for
            // BUILD-4B, so it offers MOUNT in addition to LOOKUP/READDIR.
            // Subdirectories ("dir") deliberately keep LOOKUP/READDIR only so
            // mount-right enforcement remains testable.
            root: Arc::new(TestInode::directory_with_rights(
                &[
                    ("hello.txt", InodeKind::Regular),
                    ("scratch", InodeKind::Regular),
                    ("dir", InodeKind::Directory),
                    ("link", InodeKind::Symlink),
                ],
                FileRights::ROOT_DIRECTORY,
            )),
            directory: Arc::new(TestInode::directory(&[])),
            symlink: Arc::new(TestInode::special(InodeKind::Symlink)),
        }
    }

    fn lookup_normalized(&self, normalized: &str) -> Result<Arc<dyn Inode>, VfsError> {
        match normalized {
            "" => Ok(self.root.clone()),
            "hello.txt" => Ok(self.hello.clone()),
            "scratch" => Ok(self.scratch.clone()),
            "dir" => Ok(self.directory.clone()),
            "link" => Ok(self.symlink.clone()),
            _ => Err(VfsError::NotFound),
        }
    }
}

impl MountedFs for TestFs {
    fn root(&self) -> Result<Arc<dyn Inode>, VfsError> {
        Ok(self.root.clone())
    }

    fn lookup(&self, relative: &str) -> Result<Arc<dyn Inode>, VfsError> {
        self.lookup_normalized(relative)
    }

    // No create_regular — the default `TypeForbidden` is correct for testfs.
}

impl FileRights {
    const READ_WRITE: Self = Self(Self::READ.0 | Self::WRITE.0);
    const LOOKUP_READDIR: Self = Self(Self::LOOKUP.0 | Self::READDIR.0);
    /// Rights a root directory carries: LOOKUP + READDIR + MOUNT. Only the
    /// root testfs entry uses this; subdirectories keep LOOKUP_READDIR so
    /// BUILD-4B's mount-right enforcement stays testable.
    const ROOT_DIRECTORY: Self = Self(Self::LOOKUP.0 | Self::READDIR.0 | Self::MOUNT.0);
}

struct TestInode {
    kind: InodeKind,
    available_rights: FileRights,
    entries: Vec<VfsDirEntry>,
    /// Process-context in-memory testfs payload; accessed only by VFS syscall
    /// paths and test setup, never by interrupt handlers.
    data: SwitchMutex<Vec<u8>>,
}

impl TestInode {
    fn regular(available_rights: FileRights, initial: &[u8]) -> Self {
        Self {
            kind: InodeKind::Regular,
            available_rights,
            entries: Vec::new(),
            data: SwitchMutex::new(initial.to_vec()),
        }
    }

    fn directory(entries: &[(&str, InodeKind)]) -> Self {
        Self::directory_with_rights(entries, FileRights::LOOKUP_READDIR)
    }

    fn directory_with_rights(entries: &[(&str, InodeKind)], available_rights: FileRights) -> Self {
        Self {
            kind: InodeKind::Directory,
            available_rights,
            entries: entries
                .iter()
                .enumerate()
                .map(|(idx, (name, kind))| VfsDirEntry {
                    name: String::from(*name),
                    kind: *kind,
                    next_cursor: (idx + 1) as u64,
                })
                .collect(),
            data: SwitchMutex::new(Vec::new()),
        }
    }

    fn special(kind: InodeKind) -> Self {
        Self {
            kind,
            available_rights: FileRights::empty(),
            entries: Vec::new(),
            data: SwitchMutex::new(Vec::new()),
        }
    }
}

impl Inode for TestInode {
    fn kind(&self) -> InodeKind {
        self.kind
    }

    fn available_rights(&self) -> FileRights {
        self.available_rights
    }

    fn readdir_at(&self, cursor: u64) -> Result<Option<VfsDirEntry>, VfsError> {
        if self.kind != InodeKind::Directory {
            return Err(VfsError::NotDirectory);
        }
        let idx = usize::try_from(cursor).map_err(|_| VfsError::Invalid)?;
        Ok(self.entries.get(idx).cloned())
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> Result<usize, VfsError> {
        if self.kind != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        let data = self.data.lock();
        if offset >= data.len() {
            return Ok(0);
        }
        let n = cmp::min(out.len(), data.len() - offset);
        out[..n].copy_from_slice(&data[offset..offset + n]);
        Ok(n)
    }

    fn write_at(&self, offset: usize, input: &[u8]) -> Result<usize, VfsError> {
        if self.kind != InodeKind::Regular {
            return Err(VfsError::TypeForbidden);
        }
        if !self.available_rights.contains(FileRights::WRITE) {
            return Err(VfsError::InsufficientRights);
        }
        let mut data = self.data.lock();
        let end = offset.checked_add(input.len()).ok_or(VfsError::Invalid)?;
        if end > data.len() {
            data.resize(end, 0);
        }
        data[offset..end].copy_from_slice(input);
        Ok(input.len())
    }
}

/// OT-66 L1: is `path` (absolute) at or under a mount's `.system/`, the
/// sign-in record's store? Decided on the normalized components, so
/// `/neofs//.system`, `/neofs/./.system` and `/neofs/x/../.system` all are.
/// An unnormalizable path is not (its open fails as `Invalid` anyway).
#[cfg(feature = "system-store")]
pub fn is_system_store_path(path: &str) -> bool {
    // A child mount's `.system` (`/neofs/.system`), or the root mount's own
    // (`/.system`; Dexter #187 qualifier A): the record must stay closed if
    // it ever lives on the root volume.
    normalize_absolute_path(path).is_ok_and(|normalized| {
        let mut parts = normalized.split('/');
        parts.next() == Some(".system") || parts.next() == Some(".system")
    })
}

/// OT-66 K1b: is `path` at or beneath `scope`? Both normalized (no leading
/// `/`, "" the root), compared by whole components, so `neofs/.arthur`
/// covers `neofs/.arthur/memory/x` but not `neofs/.arthurx`.
#[cfg(feature = "cap-rooted-open")]
pub fn scope_contains(scope: &str, path: &str) -> bool {
    cfg!(feature = "neo-test-desktop-arthur-scope-control")
        || scope.is_empty()
        || path == scope
        || (path.len() > scope.len()
            && path.starts_with(scope)
            && path.as_bytes()[scope.len()] == b'/')
}

pub(crate) fn normalize_absolute_path(path: &str) -> Result<String, VfsError> {
    if !path.starts_with('/') {
        return Err(VfsError::Invalid);
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            segment => parts.push(segment),
        }
    }

    let mut normalized = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            normalized.push('/');
        }
        normalized.push_str(part);
    }
    Ok(normalized)
}
