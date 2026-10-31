// SPDX-License-Identifier: MPL-2.0
//! A write-back-cache disk and a copy-on-write volume shaped like NeoFS.
//!
//! The disk: a write sits in a volatile cache until a flush returns; reads
//! are never needed (the volume keeps its own cache, as NeoFS does). Power can
//! be cut just before any event (a write or a flush): then any subset of the
//! unflushed writes reaches the media, each write whole, and the device is
//! gone. A flush can fail: it lands any subset of what it covered and reports
//! failure, and the volume stops.
//!
//! The volume: every commit writes new blocks (copy on write): file data, a
//! file inode, a directory block, a root inode, a free map, and then three
//! superblock copies naming the new root. Mount validates a copy the way
//! NeoFS does (`kernel/src/fs/neofs/mount.rs`): the root, the directory, every
//! file's inode and data (by hash), and the free map against the live set,
//! and takes the highest valid generation.
//!
//! What a commit frees ([`Volume::frees_superseded`]):
//! - on (the kernel since the write-leak fix): every commit frees what it
//!   supersedes in its own publish, deferred to its generation: a written
//!   file's old inode and data, the old directory and root inode, and the
//!   free map it moves off;
//! - off (`main` before it): only a truncate frees, the old data blocks, in a
//!   second commit; everything else leaks.

use std::collections::{BTreeMap, BTreeSet};

use crate::commit_logic::{DeferredFrees, DrainStep, Role, SuperblockDurability, next_drain_step};

pub type Block = u64;

/// Superblock copy `i` lives in block `i`.
pub const COPIES: usize = 3;
/// File bytes per data block (small, so files span several blocks).
pub const BYTES_PER_BLOCK: usize = 2;

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Content {
    Super {
        generation: u64,
        root: Block,
        root_hash: u64,
        free_map: Block,
        free_hash: u64,
    },
    RootInode {
        dir: Block,
        dir_hash: u64,
    },
    Dir(Vec<(String, Block)>),
    Inode {
        data: Vec<Block>,
        hash: u64,
    },
    Data(Vec<u8>),
    FreeMap(Vec<Block>),
}

/// FNV-1a over the content's debug form: an independent content hash.
pub fn hash<T: std::fmt::Debug>(value: &T) -> u64 {
    let text = format!("{value:?}");
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[derive(Clone, Copy, Debug)]
pub enum Fault {
    None,
    /// Power is cut just before event `at`; the unflushed writes whose index
    /// (since the last flush) has its bit in `land` reach the media.
    Cut {
        at: usize,
        land: u64,
    },
    /// Event `at`, a flush, lands `land` of what it covered and fails.
    FlushFails {
        at: usize,
        land: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskError {
    Dead,
    FlushFailed,
}

pub struct Disk {
    pub media: BTreeMap<Block, Content>,
    pending: Vec<(Block, Content)>,
    pub events: usize,
    fault: Fault,
    pub dead: bool,
    /// The number of unflushed writes at each event, for enumerating cuts.
    pub pending_at_event: Vec<usize>,
    /// Which events were flushes.
    pub flush_events: Vec<usize>,
}

impl Disk {
    pub fn new(media: BTreeMap<Block, Content>, fault: Fault) -> Self {
        Self {
            media,
            pending: Vec::new(),
            events: 0,
            fault,
            dead: false,
            pending_at_event: Vec::new(),
            flush_events: Vec::new(),
        }
    }

    fn land(&mut self, mask: u64) {
        for (i, (block, content)) in std::mem::take(&mut self.pending).into_iter().enumerate() {
            if i < 64 && mask & (1 << i) != 0 {
                self.media.insert(block, content);
            }
        }
    }

    fn event(&mut self, is_flush: bool) -> Result<(), DiskError> {
        if self.dead {
            return Err(DiskError::Dead);
        }
        let at = self.events;
        self.events += 1;
        self.pending_at_event.push(self.pending.len());
        if is_flush {
            self.flush_events.push(at);
        }
        if let Fault::Cut { at: cut, land } = self.fault
            && cut == at
        {
            self.land(land);
            self.dead = true;
            return Err(DiskError::Dead);
        }
        Ok(())
    }

    pub fn write(&mut self, block: Block, content: Content) -> Result<(), DiskError> {
        self.event(false)?;
        self.pending.push((block, content));
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), DiskError> {
        let at = self.events;
        self.event(true)?;
        if let Fault::FlushFails { at: fail, land } = self.fault
            && fail == at
        {
            self.land(land);
            return Err(DiskError::FlushFailed);
        }
        let pending = std::mem::take(&mut self.pending);
        self.media.extend(pending);
        Ok(())
    }

    /// The end of the run: power goes with no more of the cache landing.
    pub fn power_off(&mut self) {
        self.pending.clear();
        self.dead = true;
    }
}

/// How the drain orders its writes and flushes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// `main` before this fix: every role in order, one flush at the end.
    SingleFlush,
    /// `commit_logic::next_drain_step`: the barrier before the superblocks.
    Barrier,
}

pub struct Volume {
    pub order: Order,
    /// Deferred reuse of freed blocks (`commit_logic`, rule 2).
    pub defer_frees: bool,
    /// Commits free what they supersede (the write-leak fix).
    pub frees_superseded: bool,
    free_map_block: Option<Block>,
    generation: u64,
    root: Block,
    root_inode: Option<Content>,
    files: BTreeMap<String, (Block, Vec<Block>, Vec<u8>)>,
    next_free: Block,
    free: BTreeSet<Block>,
    deferred: DeferredFrees,
    durability: SuperblockDurability,
    data_unflushed: bool,
    flush_required: bool,
    dirty: BTreeMap<Block, (Role, Content)>,
    /// Every generation ever published: its files.
    pub history: BTreeMap<u64, BTreeMap<String, Vec<u8>>>,
}

/// The first block after the superblock copies.
const FIRST_BLOCK: Block = COPIES as Block;

impl Volume {
    /// A formatted, empty volume at generation 1, and its media.
    pub fn format(
        order: Order,
        defer_frees: bool,
        frees_superseded: bool,
    ) -> (Self, BTreeMap<Block, Content>) {
        let mut v = Self {
            order,
            defer_frees,
            frees_superseded,
            free_map_block: None,
            generation: 0,
            root: 0,
            root_inode: None,
            files: BTreeMap::new(),
            next_free: FIRST_BLOCK,
            free: BTreeSet::new(),
            deferred: DeferredFrees::new(),
            durability: SuperblockDurability::new([true; COPIES], 1),
            data_unflushed: false,
            flush_required: false,
            dirty: BTreeMap::new(),
            history: BTreeMap::new(),
        };
        v.commit(true);
        let media = v.dirty.iter().map(|(b, (_, c))| (*b, c.clone())).collect();
        v.dirty.clear();
        v.flush_required = false;
        (v, media)
    }

    /// The highest block ever allocated plus one: the volume's footprint.
    pub fn high_water(&self) -> Block {
        self.next_free
    }

    /// Blocks in use: allocated and not free.
    pub fn used(&self) -> usize {
        (self.next_free - FIRST_BLOCK) as usize - self.free.len()
    }

    /// Free `blocks` in the commit being built: free in its map, and with
    /// deferred reuse not allocated until its generation is durable.
    fn free_superseded(&mut self, blocks: &[Block]) {
        for &block in blocks {
            assert!(self.free.insert(block), "block {block} freed twice");
            if self.defer_frees {
                self.deferred.defer(block, self.generation + 1);
            }
        }
    }

    fn allocate(&mut self) -> Block {
        if self.defer_frees {
            self.deferred.release(self.durability.durable_generation());
        }
        let reusable = self
            .free
            .iter()
            .copied()
            .find(|b| !self.defer_frees || !self.deferred.is_deferred(*b));
        if let Some(block) = reusable {
            self.free.remove(&block);
            return block;
        }
        let block = self.next_free;
        self.next_free += 1;
        block
    }

    fn dirty(&mut self, block: Block, role: Role, content: Content) {
        if role == Role::Superblock {
            self.flush_required = true;
        }
        self.dirty.insert(block, (role, content));
    }

    /// Publish the next generation: a new directory and root (unless only
    /// the free map changed), a new free map, then the three copies.
    fn commit(&mut self, new_root: bool) {
        if new_root || self.generation == 0 {
            let old = match (&self.root_inode, self.generation) {
                (Some(Content::RootInode { dir, .. }), 1..) => Some([*dir, self.root]),
                _ => None,
            };
            let entries: Vec<(String, Block)> = self
                .files
                .iter()
                .map(|(name, (inode, _, _))| (name.clone(), *inode))
                .collect();
            let dir = self.allocate();
            let dir_content = Content::Dir(entries);
            let dir_hash = hash(&dir_content);
            self.dirty(dir, Role::RootDirectory, dir_content);
            let root = self.allocate();
            let root_inode = Content::RootInode { dir, dir_hash };
            self.dirty(root, Role::RootDirectory, root_inode.clone());
            self.root = root;
            self.root_inode = Some(root_inode);
            if let (true, Some(old)) = (self.frees_superseded, old) {
                self.free_superseded(&old);
            }
        }
        // As `publish_superblock`: the new map's blocks first, then the old
        // map is free in it.
        let free_map = self.allocate();
        if let (true, Some(old)) = (self.frees_superseded, self.free_map_block) {
            self.free_superseded(&[old]);
        }
        self.free_map_block = Some(free_map);
        let free_content = Content::FreeMap(self.free.iter().copied().collect());
        let free_hash = hash(&free_content);
        self.dirty(free_map, Role::FreeMap, free_content);
        self.generation += 1;
        let root_hash = hash(&self.root_content());
        for copy in 0..COPIES {
            self.dirty(
                copy as Block,
                Role::Superblock,
                Content::Super {
                    generation: self.generation,
                    root: self.root,
                    root_hash,
                    free_map,
                    free_hash,
                },
            );
        }
        let files = self
            .files
            .iter()
            .map(|(name, (_, _, bytes))| (name.clone(), bytes.clone()))
            .collect();
        self.history.insert(self.generation, files);
    }

    fn root_content(&self) -> Content {
        self.root_inode
            .clone()
            .expect("every commit after format names a root")
    }

    /// Write `name` with `bytes` (copy on write: new data and inode). With
    /// `frees_superseded`, the old version's inode and data are freed in the
    /// same commit.
    pub fn write(&mut self, name: &str, bytes: &[u8]) {
        let old = self.files.get(name).map(|(inode, data, _)| {
            let mut blocks = data.clone();
            blocks.push(*inode);
            blocks
        });
        let mut data = Vec::new();
        for chunk in bytes.chunks(BYTES_PER_BLOCK) {
            let block = self.allocate();
            self.dirty(block, Role::Data, Content::Data(chunk.to_vec()));
            data.push(block);
        }
        let inode = self.allocate();
        let inode_hash = hash(&bytes.to_vec());
        self.dirty(
            inode,
            Role::Inode,
            Content::Inode {
                data: data.clone(),
                hash: inode_hash,
            },
        );
        self.files
            .insert(name.into(), (inode, data, bytes.to_vec()));
        if let (true, Some(old)) = (self.frees_superseded, old) {
            self.free_superseded(&old);
        }
        self.commit(true);
    }

    /// Truncate `name` to `len`. With `frees_superseded`, one commit, as a
    /// write. Without (`main` before the fix): a new tree, then a second
    /// commit that frees the old data blocks.
    pub fn truncate(&mut self, name: &str, len: usize) {
        let (_, old_data, old_bytes) = self.files.get(name).expect("file exists").clone();
        let new_bytes = old_bytes[..len.min(old_bytes.len())].to_vec();
        self.write(name, &new_bytes);
        if self.frees_superseded {
            return;
        }
        // As the kernel does: the blocks are deferred before the commit that
        // frees them allocates its own free map, with that commit's
        // generation.
        for block in &old_data {
            self.free.insert(*block);
            if self.defer_frees {
                self.deferred.defer(*block, self.generation + 1);
            }
        }
        self.commit(false);
    }

    /// Drain the cache to the disk in this volume's order. Stops at the first
    /// disk error (a cut or a failed flush).
    pub fn drain(&mut self, disk: &mut Disk) -> Result<(), DiskError> {
        loop {
            let step = match self.order {
                Order::Barrier => next_drain_step(
                    self.dirty
                        .iter()
                        .enumerate()
                        .map(|(i, (_, (role, _)))| (i, *role)),
                    self.data_unflushed,
                    self.flush_required,
                ),
                Order::SingleFlush => {
                    let next = self
                        .dirty
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, (_, (role, _)))| role.writeback_order())
                        .map(|(i, _)| i);
                    match next {
                        Some(i) => DrainStep::Write(i),
                        None if self.flush_required => DrainStep::Flush,
                        None => DrainStep::Done,
                    }
                }
            };
            match step {
                DrainStep::Done => return Ok(()),
                DrainStep::Flush => {
                    disk.flush()?;
                    self.data_unflushed = false;
                    self.durability.flushed();
                    if self.dirty.is_empty() {
                        self.flush_required = false;
                    }
                }
                DrainStep::Write(i) => {
                    let (&block, (role, content)) =
                        self.dirty.iter().nth(i).expect("index from this map");
                    let (role, content) = (*role, content.clone());
                    disk.write(block, content.clone())?;
                    if role == Role::Superblock {
                        if let Content::Super { generation, .. } = content {
                            self.durability.written(block as usize, generation);
                        }
                    } else {
                        self.data_unflushed = true;
                    }
                    self.dirty.remove(&block);
                }
            }
        }
    }
}

/// Mount the media as NeoFS does: validate each copy, take the highest valid
/// generation. `None`: no copy is valid (the volume does not mount).
pub fn mount(media: &BTreeMap<Block, Content>) -> Option<(u64, BTreeMap<String, Vec<u8>>)> {
    let mut best: Option<(u64, BTreeMap<String, Vec<u8>>)> = None;
    for copy in 0..COPIES as Block {
        let Some(Content::Super {
            generation,
            root,
            root_hash,
            free_map,
            free_hash,
        }) = media.get(&copy)
        else {
            continue;
        };
        let Some(files) = validate(media, *root, *root_hash, *free_map, *free_hash) else {
            continue;
        };
        if best.as_ref().is_none_or(|(g, _)| generation > g) {
            best = Some((*generation, files));
        }
    }
    best
}

fn validate(
    media: &BTreeMap<Block, Content>,
    root: Block,
    root_hash: u64,
    free_map: Block,
    free_hash: u64,
) -> Option<BTreeMap<String, Vec<u8>>> {
    let root_content = media.get(&root)?;
    if hash(root_content) != root_hash {
        return None;
    }
    let Content::RootInode { dir, dir_hash } = root_content else {
        return None;
    };
    let dir_content = media.get(dir)?;
    if hash(dir_content) != *dir_hash {
        return None;
    }
    let Content::Dir(entries) = dir_content else {
        return None;
    };
    let mut live: BTreeSet<Block> = [root, *dir, free_map].into_iter().collect();
    let mut files = BTreeMap::new();
    for (name, inode) in entries {
        let Some(Content::Inode { data, hash: h }) = media.get(inode) else {
            return None;
        };
        live.insert(*inode);
        let mut bytes = Vec::new();
        for block in data {
            let Some(Content::Data(chunk)) = media.get(block) else {
                return None;
            };
            live.insert(*block);
            bytes.extend_from_slice(chunk);
        }
        if hash(&bytes) != *h {
            return None;
        }
        files.insert(name.clone(), bytes);
    }
    let free_content = media.get(&free_map)?;
    if hash(free_content) != free_hash {
        return None;
    }
    let Content::FreeMap(free) = free_content else {
        return None;
    };
    if free.iter().any(|b| live.contains(b)) {
        return None;
    }
    Some(files)
}
