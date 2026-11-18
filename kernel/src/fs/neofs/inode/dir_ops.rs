// SPDX-License-Identifier: MPL-2.0
//! F0 (`neofs-dir-ops`): `mkdir_at`, `unlink_at`, `rename_at` and `stat_at`
//! on a NeoFS volume.
//!
//! Each mutation is one commit through #180's barrier, the path every write
//! takes. It reads the directory chain from the root to each directory it
//! edits with no lock held (#106 F2: the namespace lock, `entries`, spins,
//! so no device read happens under it), then, under `entries` and only if
//! no commit came in between ([`NeoFsState::after_reads`]; `EAGAIN` after
//! repeated misses), edits the entries, writes a new version of every
//! directory on those chains bottom-up (copy on write), and publishes one
//! superblock that names the new root. What the commit
//! supersedes (the old directory versions, and an unlinked entry's blocks)
//! is freed in that publish, deferred until it is durable (`commit_logic`,
//! rule 2). A cut anywhere before the superblocks land leaves the old tree;
//! after, the new one.
//!
//! A directory handle names a path: OT-66 K1b scopes authorize paths, and a
//! NeoFS directory has no identity that survives a change beneath it (copy
//! on write gives every new version a new inode block and generation, so an
//! ancestor's generation moves on every write below it). The operations act
//! on the directory at the handle's path as of the commit (read before the
//! lock, checked under it); a handle whose directory is gone is `Stale`. The generation keeps
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

/// How many times an operation reads before the lock and finds a commit came
/// in between, before it gives up with `Contended` (`EAGAIN`; see
/// [`NeoFsState::after_reads`]).
const READ_BEFORE_LOCK_TRIES: usize = 8;

/// The namespace lock as an F0 operation holds it: only after its reads.
struct DirOpLock<'a> {
    guard: crate::sync::park_mutex::ParkMutexGuard<'a, Vec<DirEntry>>,
}

impl<'a> DirOpLock<'a> {
    fn new(guard: crate::sync::park_mutex::ParkMutexGuard<'a, Vec<DirEntry>>) -> Self {
        #[cfg(feature = "neo-test-neofs-dir-ops")]
        probe::ENTRIES_HELD.store(true, core::sync::atomic::Ordering::SeqCst);
        Self { guard }
    }
}

impl Drop for DirOpLock<'_> {
    fn drop(&mut self) {
        #[cfg(feature = "neo-test-neofs-dir-ops")]
        probe::ENTRIES_HELD.store(false, core::sync::atomic::Ordering::SeqCst);
    }
}

/// Everything a rename reads before the lock: the chains it rewrites and the
/// entry it moves.
struct RenamePlan {
    shared: Vec<DirectoryFrame>,
    tails: [Vec<DirectoryFrame>; 2],
    moved: DirEntry,
}

impl NeoFsState {
    /// Read first, then lock (#106 F2). `entries` is a spinning lock, so no
    /// F0 operation holds it across a device read: `read` runs on a copy of
    /// the root's entries with no lock held, and may miss the cache. Then the
    /// lock is taken and the superblock generation compared with the one
    /// before the reads. Every publish bumps it, under this lock, so an equal
    /// generation means no commit came in between: the blocks read were the
    /// live tree's (copy on write never changes them in place, and a commit
    /// frees the ones it supersedes only at its publish), and `read`'s result
    /// stands, an error included. Otherwise it drops the lock and reads
    /// again. After [`READ_BEFORE_LOCK_TRIES`] such misses it returns
    /// `Contended` (`EAGAIN`) and the caller retries: it never reads under
    /// the lock, on any path (#106 relay 399). Nothing has been written by
    /// then, so the refusal leaves the volume as it was.
    fn after_reads<T>(
        &self,
        read: impl Fn(&[DirEntry]) -> Result<T, NeoFsError>,
    ) -> Result<(DirOpLock<'_>, T), NeoFsError> {
        for _ in 0..READ_BEFORE_LOCK_TRIES {
            let before = self.generation();
            let root_entries = self.entries.lock().clone();
            let result = read(&root_entries);
            // The scenario's contention leg: another writer commits between
            // the reads and the lock.
            #[cfg(feature = "neo-test-neofs-dir-ops")]
            probe::interleave(self);
            let lock = DirOpLock::new(self.entries.lock());
            if self.generation() == before {
                return result.map(|value| (lock, value));
            }
        }
        #[cfg(feature = "neo-test-neofs-dir-ops")]
        probe::CONTENDED.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        Err(NeoFsError::Contended)
    }

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
        #[cfg(feature = "neo-test-neofs-dir-ops")]
        let _probe = probe::InOp::on(self);
        let dir_parts = logic::components(dir);
        // An empty directory is one level tall: its entries will sit below it.
        if !logic::depth_fits(&dir_parts, 1) {
            return Err(NeoFsError::TooDeep);
        }
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let (mut lock, mut frames) = self.after_reads(|root| {
            let frames = self.handle_chain(root, &dir_parts)?;
            let parent = frames.last().ok_or(NeoFsError::InvalidDirectory)?;
            if parent.entries.iter().any(|entry| entry.name == name) {
                return Err(NeoFsError::Exists);
            }
            check_directory_capacity(parent.entries.len() + 1)?;
            Ok(frames)
        })?;
        let parent = frames.last_mut().ok_or(NeoFsError::InvalidDirectory)?;
        // The new directory's rights are its parent's (the format keeps
        // them per inode; every directory carries LOOKUP|READDIR).
        let template = parent.record.clone();
        let (child_block, _record) = self.write_directory_version(&template, &[])?;
        parent.entries.push(DirEntry {
            name: name.into(),
            child_block,
            kind: super::super::format::NeoFsKind::Directory,
        });
        self.commit_dir_tree(&mut lock.guard, &mut frames, &mut [], Vec::new())
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
        #[cfg(feature = "neo-test-neofs-dir-ops")]
        let _probe = probe::InOp::on(self);
        let dir_parts = logic::components(dir);
        if logic::is_pinned(&dir_parts, name) {
            return Err(NeoFsError::Pinned);
        }
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let (mut lock, (mut frames, superseded)) = self.after_reads(|root| {
            let frames = self.handle_chain(root, &dir_parts)?;
            let parent = frames.last().ok_or(NeoFsError::InvalidDirectory)?;
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
            Ok((frames, superseded))
        })?;
        let parent = frames.last_mut().ok_or(NeoFsError::InvalidDirectory)?;
        logic::remove_named(&mut parent.entries, name, |e| e.name.as_str())
            .ok_or(NeoFsError::NotFound)?;
        let superseded = if cfg!(feature = "neofs-leak-superseded") {
            Vec::new()
        } else {
            superseded
        };
        self.commit_dir_tree(&mut lock.guard, &mut frames, &mut [], superseded)
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
        #[cfg(feature = "neo-test-neofs-dir-ops")]
        let _probe = probe::InOp::on(self);
        let src_parts = logic::components(src_dir);
        let dst_parts = logic::components(dst_dir);
        if logic::is_pinned(&src_parts, src_name) {
            return Err(NeoFsError::Pinned);
        }
        let common = logic::rename_check(&src_parts, src_name, &dst_parts, dst_name)
            .map_err(|_| NeoFsError::IntoItself)?;
        self.throttle_dirty()?;
        let _writeback_pause = self.cache.pause_worker();
        let (mut lock, plan) = self.after_reads(|root| {
            let mut src_chain = self.handle_chain(root, &src_parts)?;
            let mut dst_chain = self.handle_chain(root, &dst_parts)?;
            // Frames 0..=common are the directories both chains share (the
            // root and the common components); the rest are each side's own
            // tail.
            let shared_len = common + 1;
            let src_tail: Vec<DirectoryFrame> = src_chain.split_off(shared_len);
            let dst_tail: Vec<DirectoryFrame> = dst_chain.split_off(shared_len);
            let mut shared = src_chain;
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
            // Only a move deeper can break the depth limit: a subtree that
            // fit where it was fits at the same depth or above, so the walk
            // is skipped then.
            if dst_parts.len() > src_parts.len() {
                let height = self.subtree_height(&moved, 0)?;
                if !logic::depth_fits(&dst_parts, height) {
                    return Err(NeoFsError::TooDeep);
                }
            }
            if !(tails[0].is_empty() && tails[1].is_empty()) {
                let dst = last_of(&mut shared, &mut tails, 1)?;
                check_directory_capacity(dst.entries.len() + 1)?;
            }
            Ok(RenamePlan {
                shared,
                tails,
                moved,
            })
        })?;
        let RenamePlan {
            mut shared,
            mut tails,
            moved,
        } = plan;
        if tails[0].is_empty() && tails[1].is_empty() {
            // In place: the entry keeps its position.
            let dir = last_of(&mut shared, &mut tails, 0)?;
            let entry = dir
                .entries
                .iter_mut()
                .find(|entry| entry.name == src_name)
                .ok_or(NeoFsError::NotFound)?;
            entry.name = dst_name.into();
        } else {
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
        self.commit_dir_tree(&mut lock.guard, &mut shared, &mut tails, Vec::new())
    }

    /// `name` in `dir`: its kind, size and entry count. Read with no lock
    /// held, then checked against the generation under it, as a mutation's
    /// reads are.
    pub(in crate::fs::neofs) fn stat_at(
        &self,
        dir: Option<&str>,
        name: &str,
    ) -> Result<DirStat, NeoFsError> {
        #[cfg(feature = "neo-test-neofs-dir-ops")]
        let _probe = probe::InOp::on(self);
        let dir_parts = logic::components(dir);
        let (lock, stat) = self.after_reads(|root| {
            let frames = self.handle_chain(root, &dir_parts)?;
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
        })?;
        drop(lock);
        Ok(stat)
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

    /// A new version of a directory holding `entries`. With entries, it is
    /// `write_directory_tree_without_publish`'s. Empty, it is an inode with no
    /// data block: the format counts a file's blocks from its size
    /// (`parse_inode`: `block_count == file_data_blocks(file_size)`, 0 for an
    /// empty directory), and its content hash is BLAKE3 of nothing (spec
    /// §4.4). F0 is the first writer of an empty directory: a new folder, a
    /// folder emptied by an unlink, a rename's source.
    fn write_directory_version(
        &self,
        old_record: &InodeRecord,
        entries: &[DirEntry],
    ) -> Result<(u64, InodeRecord), NeoFsError> {
        if !entries.is_empty() {
            return self.write_directory_tree_without_publish(old_record, entries);
        }
        let (inode_block, generation) = self.reserve_inode_block_and_generation()?;
        let record = InodeRecord {
            kind: super::super::format::NeoFsKind::Directory,
            generation,
            file_size: 0,
            block_count: 0,
            direct_blocks: [0; DIRECT_BLOCKS],
            single_indirect_block: 0,
            double_indirect_block: 0,
            triple_indirect_block: 0,
            content_hash: hash(&[]),
            available_rights: old_record.available_rights,
            directory_entry_count: 0,
            fs_block_count: self.total_blocks(),
        };
        write_inode_to_cache(
            &self.cache,
            inode_block,
            self.total_blocks(),
            &record,
            CachedBlockRole::RootDirectory,
        )?;
        Ok((inode_block, record))
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
                let (dir_block, _record) =
                    self.write_directory_version(&tail[idx].record, &tail[idx].entries)?;
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
            let (dir_block, _record) =
                self.write_directory_version(&shared[idx].record, &shared[idx].entries)?;
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

/// After its own commit, a directory handle reads the new version (no lock
/// held), then publishes it ([`publish`]). Returns the generation the inode
/// now holds, for the handle.
pub(super) fn reload(inode: &NeoInode) -> Result<Option<u32>, VfsError> {
    let snapshot = inode
        .state
        .directory_now(inode.name.as_deref())
        .map_err(|_| VfsError::Stale)?;
    Ok(publish(inode, snapshot))
}

/// Publish a directory snapshot to the inode only if it is newer than the
/// one it holds (#106 F1, CP-13). Two operations through one shared handle
/// each reload; without this, the later commit's reload could store its
/// inode block and the earlier one's record after it, and a readdir would
/// pass the generation check and list the old entries. Generations only
/// grow (each version gets the next one), so the newest snapshot wins
/// whatever order the reloads finish in. The block and the record are
/// stored under the record's lock, with no I/O, and the generation returned
/// is the one now held, so the handle never holds a generation newer than
/// the listing its inode has.
fn publish(inode: &NeoInode, (block, record): (u64, InodeRecord)) -> Option<u32> {
    let mut held = inode.record.lock();
    if logic::newer_generation(Some(record.generation), Some(held.generation)) {
        *inode.inode_block.lock() = block;
        *held = record;
    }
    Some(held.generation)
}

/// The scenario's F1 discriminator: a reload whose read is done now and
/// whose publish runs when the returned closure is called, later. It stands
/// in for an operation paused after its reload's read while another one
/// through the same handle completes.
#[cfg(feature = "neo-test-neofs-dir-ops")]
pub(super) fn reload_later<'a>(
    inode: &'a NeoInode,
) -> alloc::boxed::Box<dyn FnOnce() -> Result<Option<u32>, VfsError> + 'a> {
    let snapshot = inode.state.directory_now(inode.name.as_deref());
    alloc::boxed::Box::new(move || {
        let snapshot = snapshot.map_err(|_| VfsError::Stale)?;
        Ok(publish(inode, snapshot))
    })
}

/// The scenario's F2 discriminator: every device read a cache miss makes
/// while an F0 operation runs is counted, and separately the ones made
/// while that operation holds the namespace lock (which must be none).
#[cfg(feature = "neo-test-neofs-dir-ops")]
pub(in crate::fs::neofs) mod probe {
    use super::NeoFsState;
    use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

    pub(super) static ENTRIES_HELD: AtomicBool = AtomicBool::new(false);
    /// How many F0 operations are running (the contention leg nests the
    /// interleaved writers' operations inside one).
    static IN_OP: AtomicU32 = AtomicU32::new(0);
    static OP_READS: AtomicU64 = AtomicU64::new(0);
    static HELD_READS: AtomicU64 = AtomicU64::new(0);
    pub(super) static CONTENDED: AtomicU64 = AtomicU64::new(0);
    /// Interleaved commits still to make, one per optimistic attempt.
    static INTERLEAVE: AtomicU32 = AtomicU32::new(0);
    /// Device reads of F0 operations at which another writer commits twice
    /// (before the read is issued: the lock is free at the read boundary).
    static AT_READ: AtomicU32 = AtomicU32::new(0);
    static AT_READ_DONE: AtomicU64 = AtomicU64::new(0);
    /// The state the at-read writer commits on (set by `InOp::on`).
    static STATE: core::sync::atomic::AtomicPtr<NeoFsState> =
        core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());
    static INTERLEAVING: AtomicBool = AtomicBool::new(false);
    static INTERLEAVED: AtomicU64 = AtomicU64::new(0);
    /// The root folder the interleaved writer makes and removes.
    const INTERLEAVE_NAME: &str = "f2-writer";

    /// The span of one F0 operation.
    pub(super) struct InOp;

    impl InOp {
        pub(super) fn new() -> Self {
            IN_OP.fetch_add(1, Ordering::SeqCst);
            Self
        }

        /// As `new`, for an operation on `state`: the at-read writer
        /// commits there.
        pub(super) fn on(state: &NeoFsState) -> Self {
            STATE.store(
                state as *const NeoFsState as *mut NeoFsState,
                Ordering::SeqCst,
            );
            Self::new()
        }
    }

    impl Drop for InOp {
        fn drop(&mut self) {
            if IN_OP.fetch_sub(1, Ordering::SeqCst) == 1 {
                STATE.store(core::ptr::null_mut(), Ordering::SeqCst);
            }
        }
    }

    /// The next `reads` device reads of F0 operations each have another
    /// writer commit twice (a root mkdir, then its unlink) before the read
    /// returns, leaving the tree as it was.
    pub(in crate::fs::neofs) fn commit_at_reads(reads: u32) {
        AT_READ.store(reads, Ordering::SeqCst);
    }

    /// The next F0 operation's next `commits` optimistic attempts each see
    /// another writer commit between their reads and the lock: a real
    /// `mkdir_at`, then `unlink_at`, of a root folder, alternating, so an even
    /// count leaves the tree as it was. Each takes `entries`, which the
    /// operation must not be holding.
    pub(in crate::fs::neofs) fn interleave_commits(commits: u32) {
        INTERLEAVE.store(commits, Ordering::SeqCst);
    }

    pub(super) fn interleave(state: &NeoFsState) {
        if INTERLEAVING.swap(true, Ordering::SeqCst) {
            return;
        }
        let left = INTERLEAVE.load(Ordering::SeqCst);
        if left > 0 {
            INTERLEAVE.store(left - 1, Ordering::SeqCst);
            let result = if left.is_multiple_of(2) {
                state.mkdir_at(None, INTERLEAVE_NAME)
            } else {
                state.unlink_at(None, INTERLEAVE_NAME)
            };
            if result.is_ok() {
                INTERLEAVED.fetch_add(1, Ordering::SeqCst);
            }
        }
        INTERLEAVING.store(false, Ordering::SeqCst);
    }

    /// Called by the block cache before each device read it makes.
    pub(in crate::fs::neofs) fn note_device_read() {
        if IN_OP.load(Ordering::SeqCst) > 0 {
            OP_READS.fetch_add(1, Ordering::SeqCst);
            if ENTRIES_HELD.load(Ordering::SeqCst) {
                HELD_READS.fetch_add(1, Ordering::SeqCst);
            }
            commit_at_read();
        }
    }

    /// The read-boundary leg: before this read is issued, another writer
    /// takes `entries` and commits. It runs on this CPU, so if the operation
    /// held `entries` here the writer would spin for ever: the scenario's
    /// timeout is the witness. No completion is held. It counts only
    /// commits that completed.
    fn commit_at_read() {
        let state = STATE.load(Ordering::SeqCst);
        if state.is_null() || AT_READ.load(Ordering::SeqCst) == 0 {
            return;
        }
        if INTERLEAVING.swap(true, Ordering::SeqCst) {
            return;
        }
        AT_READ.fetch_sub(1, Ordering::SeqCst);
        // SAFETY: `STATE` is set by `InOp::on` from a live `&NeoFsState`
        // and cleared when the outermost operation's `InOp` drops, so it
        // points to the state that operation borrows for as long as this
        // read (inside that operation) runs.
        let state = unsafe { &*state };
        if state.mkdir_at(None, INTERLEAVE_NAME).is_ok() {
            AT_READ_DONE.fetch_add(1, Ordering::SeqCst);
            if state.unlink_at(None, INTERLEAVE_NAME).is_ok() {
                AT_READ_DONE.fetch_add(1, Ordering::SeqCst);
            }
        }
        INTERLEAVING.store(false, Ordering::SeqCst);
    }

    /// Commits the at-read writer completed since the last call.
    pub(in crate::fs::neofs) fn take_at_read() -> u64 {
        AT_READ_DONE.swap(0, Ordering::SeqCst)
    }

    /// (device reads during F0 operations, of them with the lock held,
    /// operations refused as `Contended`, interleaved commits made), and
    /// reset.
    pub(in crate::fs::neofs) fn take() -> (u64, u64, u64, u64) {
        (
            OP_READS.swap(0, Ordering::SeqCst),
            HELD_READS.swap(0, Ordering::SeqCst),
            CONTENDED.swap(0, Ordering::SeqCst),
            INTERLEAVED.swap(0, Ordering::SeqCst),
        )
    }
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
        NeoFsError::Contended => VfsError::Again,
        NeoFsError::UnsupportedType => VfsError::TypeForbidden,
        NeoFsError::BlockDevice | NeoFsError::NoDurableFlush => VfsError::Io,
        // A path component that is not a directory, a full directory (64
        // entries), too deep, a rename into itself, corruption.
        _ => {
            // The scenario names the NeoFS error behind an EINVAL.
            #[cfg(feature = "neo-test-neofs-dir-ops")]
            crate::serial_println!("[ kernel ] neofs dir op: {:?} -> EINVAL", error);
            VfsError::Invalid
        }
    }
}
