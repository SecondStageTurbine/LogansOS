// SPDX-License-Identifier: MPL-2.0
//! F0 (`neofs-dir-ops`): `mkdir_at`, `unlink_at`, `rename_at` and `stat_at`
//! on a NeoFS volume.
//!
//! Each mutation is one commit through #180's barrier, the path every write
//! takes: under the namespace lock (`entries`), it loads the directory chain
//! from the root to each directory it edits, edits the entries, writes a new
//! version of every directory on those chains bottom-up (copy on write), and
//! publishes one superblock that names the new root. What the commit
//! supersedes (the old directory versions, and an unlinked entry's blocks)
//! is freed in that publish, deferred until it is durable (`commit_logic`,
//! rule 2). A cut anywhere before the superblocks land leaves the old tree;
//! after, the new one.
//!
//! A directory handle names a path: OT-66 K1b scopes authorize paths, and a
//! NeoFS directory has no identity that survives a change beneath it (copy
//! on write gives every new version a new inode block and generation, so an
//! ancestor's generation moves on every write below it). The operations act
//! on the directory at the handle's path now, resolved under the namespace
//! lock; a handle whose directory is gone is `Stale`. The generation keeps
//! its one job, readdir's listing: after its own operation a handle reloads
//! and holds the new version.
//!
//! The rules (names, depth, pinned entries, rename into itself) are
//! `dir_ops_logic`, shared with the host model that cuts power at every
//! write of every operation.

use super::super::dir_ops_logic as logic;
use super::*;

/// The test cut (`neo-test-neofs-dir-ops`): the next directory operation
/// writes every block of its commit except the superblocks, and returns.
#[cfg(feature = "neo-test-neofs-dir-ops")]
pub(in crate::fs::neofs) static STOP_NEXT_BEFORE_PUBLISH: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// What a stat reports (`dir_ops_logic::StatAtV1` before the ABI numbers).
pub(in crate::fs::neofs) struct DirStat {
    pub kind: super::super::format::NeoFsKind,
    pub size: u64,
    pub entry_count: u32,
}

impl NeoFsState {
    /// A new, empty directory `name` in `dir`.
    pub(in crate::fs::neofs) fn mkdir_at(
        &self,
        dir: Option<&str>,
        name: &str,
    ) -> Result<(), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;
        let dir_parts = logic::components(dir);
        // An empty directory is one level tall: its entries will sit below it.
        if !logic::depth_fits(&dir_parts, 1) {
            return Err(NeoFsError::TooDeep);
        }
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let mut root_entries = self.entries.lock();
        let mut frames = self.handle_chain(&root_entries, &dir_parts)?;
        let parent = frames.last_mut().ok_or(NeoFsError::InvalidDirectory)?;
        if parent.entries.iter().any(|entry| entry.name == name) {
            return Err(NeoFsError::Exists);
        }
        check_directory_capacity(parent.entries.len() + 1)?;
        // The new directory's rights are its parent's (the format keeps
        // them per inode; every directory carries LOOKUP|READDIR).
        let template = parent.record.clone();
        let (child_block, _record) = self.write_directory_tree_without_publish(&template, &[])?;
        parent.entries.push(DirEntry {
            name: name.into(),
            child_block,
            kind: super::super::format::NeoFsKind::Directory,
        });
        self.commit_dir_tree(&mut root_entries, &mut frames, &mut [], Vec::new())
    }

    /// Remove `name` from `dir`: a regular file, or an empty directory. Its
    /// blocks are freed in the commit's publish.
    pub(in crate::fs::neofs) fn unlink_at(
        &self,
        dir: Option<&str>,
        name: &str,
    ) -> Result<(), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;
        let dir_parts = logic::components(dir);
        if logic::is_pinned(&dir_parts, name) {
            return Err(NeoFsError::Pinned);
        }
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let mut root_entries = self.entries.lock();
        let mut frames = self.handle_chain(&root_entries, &dir_parts)?;
        let parent = frames.last_mut().ok_or(NeoFsError::InvalidDirectory)?;
        let entry = parent
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .cloned()
            .ok_or(NeoFsError::NotFound)?;
        let (record, _bytes) = self.read_cached_inode(entry.child_block)?;
        if record.kind != entry.kind {
            return Err(NeoFsError::DirectoryTypeMismatch);
        }
        let superseded = match record.kind {
            super::super::format::NeoFsKind::Regular => {
                collect_regular_tree_blocks_cached(&self.cache, entry.child_block, &record)?
            }
            super::super::format::NeoFsKind::Directory => {
                if record.directory_entry_count != 0 {
                    return Err(NeoFsError::NotEmpty);
                }
                let mut blocks = Vec::new();
                collect_directory_blocks(entry.child_block, &record, &mut blocks)?;
                blocks
            }
            // A model or a symlink: not F0's to remove.
            _ => return Err(NeoFsError::UnsupportedType),
        };
        logic::remove_named(&mut parent.entries, name, |e| e.name.as_str())
            .ok_or(NeoFsError::NotFound)?;
        let superseded = if cfg!(feature = "neofs-leak-superseded") {
            Vec::new()
        } else {
            superseded
        };
        self.commit_dir_tree(&mut root_entries, &mut frames, &mut [], superseded)
    }

    /// Move `src_name` in `src_dir` to `dst_name` in `dst_dir`, on this
    /// volume. Never replaces: `Exists` when the destination name is taken.
    /// The entry keeps its inode (and so its generation); only directories
    /// are rewritten.
    pub(in crate::fs::neofs) fn rename_at(
        &self,
        (src_dir, src_name): (Option<&str>, &str),
        (dst_dir, dst_name): (Option<&str>, &str),
    ) -> Result<(), NeoFsError> {
        if !self.writable {
            return Err(NeoFsError::ReadOnly);
        }
        #[cfg(feature = "power-clean")]
        let _op = self.begin_write()?;
        let src_parts = logic::components(src_dir);
        let dst_parts = logic::components(dst_dir);
        if logic::is_pinned(&src_parts, src_name) {
            return Err(NeoFsError::Pinned);
        }
        let common = logic::rename_check(&src_parts, src_name, &dst_parts, dst_name)
            .map_err(|_| NeoFsError::IntoItself)?;
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let mut root_entries = self.entries.lock();
        let mut src_chain = self.handle_chain(&root_entries, &src_parts)?;
        let mut dst_chain = self.handle_chain(&root_entries, &dst_parts)?;

        // Frames 0..=common are the directories both chains share (the root
        // and the common components); the rest are each side's own tail.
        let shared_len = common + 1;
        let src_tail: Vec<DirectoryFrame> = src_chain.split_off(shared_len);
        let dst_tail: Vec<DirectoryFrame> = dst_chain.split_off(shared_len);
        let mut shared = src_chain;
        drop(dst_chain);
        let mut tails = [src_tail, dst_tail];

        let moved = {
            let src = last_of(&mut shared, &mut tails, 0)?;
            src.entries
                .iter()
                .find(|entry| entry.name == src_name)
                .cloned()
                .ok_or(NeoFsError::NotFound)?
        };
        {
            let dst = last_of(&mut shared, &mut tails, 1)?;
            if dst.entries.iter().any(|entry| entry.name == dst_name) {
                return Err(NeoFsError::Exists);
            }
        }
        let height = self.subtree_height(&moved, 0)?;
        if !logic::depth_fits(&dst_parts, height) {
            return Err(NeoFsError::TooDeep);
        }
        let same_directory = tails[0].is_empty() && tails[1].is_empty();
        if same_directory {
            // In place: the entry keeps its position.
            let dir = last_of(&mut shared, &mut tails, 0)?;
            let entry = dir
                .entries
                .iter_mut()
                .find(|entry| entry.name == src_name)
                .ok_or(NeoFsError::NotFound)?;
            entry.name = dst_name.into();
        } else {
            {
                let dst = last_of(&mut shared, &mut tails, 1)?;
                check_directory_capacity(dst.entries.len() + 1)?;
            }
            let src = last_of(&mut shared, &mut tails, 0)?;
            logic::remove_named(&mut src.entries, src_name, |e| e.name.as_str())
                .ok_or(NeoFsError::NotFound)?;
            let dst = last_of(&mut shared, &mut tails, 1)?;
            dst.entries.push(DirEntry {
                name: dst_name.into(),
                child_block: moved.child_block,
                kind: moved.kind,
            });
        }
        self.commit_dir_tree(&mut root_entries, &mut shared, &mut tails, Vec::new())
    }

    /// `name` in `dir`: its kind, size and entry count.
    pub(in crate::fs::neofs) fn stat_at(
        &self,
        dir: Option<&str>,
        name: &str,
    ) -> Result<DirStat, NeoFsError> {
        let dir_parts = logic::components(dir);
        let root_entries = self.entries.lock();
        let frames = self.handle_chain(&root_entries, &dir_parts)?;
        let parent = frames.last().ok_or(NeoFsError::InvalidDirectory)?;
        let entry = parent
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .ok_or(NeoFsError::NotFound)?;
        let (record, _bytes) = self.read_cached_inode(entry.child_block)?;
        if record.kind != entry.kind {
            return Err(NeoFsError::DirectoryTypeMismatch);
        }
        Ok(DirStat {
            kind: record.kind,
            size: record.file_size,
            entry_count: record.directory_entry_count,
        })
    }

    /// The chain from the root to the handle's directory `dir_parts`, as it
    /// is now. A directory that is gone (renamed away, or removed) is
    /// `Stale`: the handle names nothing.
    fn handle_chain(
        &self,
        root_entries: &[DirEntry],
        dir_parts: &[&str],
    ) -> Result<Vec<DirectoryFrame>, NeoFsError> {
        self.load_directory_chain(root_entries, dir_parts)
            .map_err(|e| match e {
                NeoFsError::NotFound => NeoFsError::Stale,
                e => e,
            })
    }

    /// The directory at `dir` now: its inode block and record, for a handle
    /// to reload after its own commit.
    pub(in crate::fs::neofs) fn directory_now(
        &self,
        dir: Option<&str>,
    ) -> Result<(u64, InodeRecord), NeoFsError> {
        match dir {
            None => Ok(self.root_snapshot()),
            Some(path) => self.resolve_path(path),
        }
    }

    /// How many directory levels `entry` adds below its own: 0 for a file,
    /// 1 for an empty directory (its future entries), and one more than its
    /// tallest child directory otherwise. Bounded by the depth mount allows.
    fn subtree_height(&self, entry: &DirEntry, depth: usize) -> Result<usize, NeoFsError> {
        if entry.kind != super::super::format::NeoFsKind::Directory {
            return Ok(0);
        }
        if depth > logic::MAX_DEPTH {
            return Err(NeoFsError::TooDeep);
        }
        let (record, _bytes) = self.read_cached_inode(entry.child_block)?;
        let entries = self.read_cached_directory_entries(&record)?;
        let mut height = 1;
        for child in &entries {
            height = height.max(1 + self.subtree_height(child, depth + 1)?);
        }
        Ok(height)
    }

    /// Write a new version of every directory in `tails` (each hangs below
    /// `shared`'s last frame) and then of `shared`, bottom-up, and publish
    /// the new root. The publish frees `superseded` and every old directory
    /// version here. As `commit_directory_frames`, for a commit that edits
    /// two chains (a rename across directories).
    fn commit_dir_tree(
        &self,
        root_entries: &mut Vec<DirEntry>,
        shared: &mut [DirectoryFrame],
        tails: &mut [Vec<DirectoryFrame>],
        mut superseded: Vec<u64>,
    ) -> Result<(), NeoFsError> {
        if !cfg!(feature = "neofs-leak-superseded") {
            for frame in shared.iter().chain(tails.iter().flatten()) {
                collect_directory_blocks(frame.inode_block, &frame.record, &mut superseded)?;
            }
        }
        // Each tail, bottom-up, into its parent; the top of each tail into
        // the shared directory both hang from.
        for tail in tails.iter_mut() {
            let mut child_update: Option<(String, u64)> = None;
            for idx in (0..tail.len()).rev() {
                if let Some((name, block)) = child_update.take() {
                    point_entry(&mut tail[idx].entries, &name, block)?;
                }
                let (dir_block, _record) = self
                    .write_directory_tree_without_publish(&tail[idx].record, &tail[idx].entries)?;
                let name = tail[idx].name.clone().ok_or(NeoFsError::InvalidDirectory)?;
                child_update = Some((name, dir_block));
            }
            if let Some((name, block)) = child_update {
                let top = shared.last_mut().ok_or(NeoFsError::InvalidDirectory)?;
                point_entry(&mut top.entries, &name, block)?;
            }
        }
        let mut child_update: Option<(String, u64)> = None;
        for idx in (0..shared.len()).rev() {
            if let Some((name, block)) = child_update.take() {
                point_entry(&mut shared[idx].entries, &name, block)?;
            }
            if idx == 0 {
                #[cfg(feature = "neo-test-neofs-dir-ops")]
                if STOP_NEXT_BEFORE_PUBLISH.swap(false, core::sync::atomic::Ordering::SeqCst) {
                    // The cut: the new root written, no superblock.
                    self.write_root_tree_without_publish(&shared[0].entries)?;
                    return Ok(());
                }
                self.commit_root_entries(&shared[0].entries, &superseded)?;
                *root_entries = shared[0].entries.clone();
                return Ok(());
            }
            let (dir_block, _record) = self
                .write_directory_tree_without_publish(&shared[idx].record, &shared[idx].entries)?;
            let name = shared[idx]
                .name
                .clone()
                .ok_or(NeoFsError::InvalidDirectory)?;
            child_update = Some((name, dir_block));
        }
        Err(NeoFsError::InvalidDirectory)
    }
}

/// The source (`side` 0) or destination (`side` 1) directory of a rename:
/// the last frame of its tail, or the shared chain's last when it has none.
fn last_of<'a>(
    shared: &'a mut [DirectoryFrame],
    tails: &'a mut [Vec<DirectoryFrame>; 2],
    side: usize,
) -> Result<&'a mut DirectoryFrame, NeoFsError> {
    match tails[side].last_mut() {
        Some(frame) => Ok(frame),
        None => shared.last_mut().ok_or(NeoFsError::InvalidDirectory),
    }
}

/// Point the directory entry `name` at its new version.
fn point_entry(entries: &mut [DirEntry], name: &str, block: u64) -> Result<(), NeoFsError> {
    let entry = entries
        .iter_mut()
        .find(|entry| entry.name == name)
        .ok_or(NeoFsError::NotFound)?;
    entry.child_block = block;
    entry.kind = super::super::format::NeoFsKind::Directory;
    Ok(())
}

/// A directory handle's inode, as the VFS hands it in.
fn directory(inode: &NeoInode) -> Result<(), VfsError> {
    if inode.record.lock().kind.as_vfs_kind() != InodeKind::Directory {
        return Err(VfsError::NotDirectory);
    }
    Ok(())
}

pub(super) fn mkdir(inode: &NeoInode, name: &str) -> Result<(), VfsError> {
    directory(inode)?;
    inode
        .state
        .mkdir_at(inode.name.as_deref(), name)
        .map_err(to_vfs)
}

pub(super) fn unlink(inode: &NeoInode, name: &str) -> Result<(), VfsError> {
    directory(inode)?;
    inode
        .state
        .unlink_at(inode.name.as_deref(), name)
        .map_err(to_vfs)
}

/// A rename to a directory of another volume (or another filesystem) is
/// `CrossDevice`: the entry would have to be copied, not moved.
pub(super) fn rename(
    inode: &NeoInode,
    name: &str,
    to: &dyn Inode,
    to_name: &str,
) -> Result<(), VfsError> {
    directory(inode)?;
    if to.kind() != InodeKind::Directory {
        return Err(VfsError::NotDirectory);
    }
    let (volume, to_dir) = to.neofs_volume().ok_or(VfsError::CrossDevice)?;
    if volume != Arc::as_ptr(&inode.state) as usize {
        return Err(VfsError::CrossDevice);
    }
    inode
        .state
        .rename_at((inode.name.as_deref(), name), (to_dir, to_name))
        .map_err(to_vfs)
}

pub(super) fn stat(inode: &NeoInode, name: &str) -> Result<logic::StatAtV1, VfsError> {
    directory(inode)?;
    let stat = inode
        .state
        .stat_at(inode.name.as_deref(), name)
        .map_err(to_vfs)?;
    use super::super::format::NeoFsKind;
    let (kind, size, flags) = match stat.kind {
        NeoFsKind::Directory => (
            logic::KIND_DIRECTORY,
            u64::from(stat.entry_count),
            if stat.entry_count == 0 {
                logic::STAT_EMPTY_DIRECTORY
            } else {
                0
            },
        ),
        NeoFsKind::Regular => (logic::KIND_REGULAR, stat.size, 0),
        NeoFsKind::Symlink => (logic::KIND_SYMLINK, stat.size, 0),
        NeoFsKind::Model => (logic::KIND_MODEL, stat.size, 0),
    };
    Ok(logic::StatAtV1 { kind, flags, size })
}

/// After its own commit, a directory handle reads the new version: its
/// inode block and record (which `readdir_at` lists), and the generation
/// the handle now holds, taken from the same record so the two agree.
pub(super) fn reload(inode: &NeoInode) -> Result<Option<u32>, VfsError> {
    let (block, record) = inode
        .state
        .directory_now(inode.name.as_deref())
        .map_err(|_| VfsError::Stale)?;
    let generation = record.generation;
    *inode.inode_block.lock() = block;
    *inode.record.lock() = record;
    Ok(Some(generation))
}

/// The VFS error for a directory operation's NeoFS error.
fn to_vfs(error: NeoFsError) -> VfsError {
    match error {
        NeoFsError::NotFound => VfsError::NotFound,
        NeoFsError::ReadOnly => VfsError::InsufficientRights,
        NeoFsError::Exists => VfsError::AlreadyExists,
        NeoFsError::NotEmpty => VfsError::NotEmpty,
        NeoFsError::Stale => VfsError::Stale,
        NeoFsError::Pinned => VfsError::Busy,
        NeoFsError::UnsupportedType => VfsError::TypeForbidden,
        NeoFsError::BlockDevice | NeoFsError::NoDurableFlush => VfsError::Io,
        // A path component that is not a directory, a full directory (64
        // entries), too deep, a rename into itself, corruption.
        _ => VfsError::Invalid,
    }
}
