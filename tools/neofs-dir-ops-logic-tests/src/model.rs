// SPDX-License-Identifier: MPL-2.0
//! A write-back-cache disk, and a copy-on-write volume with nested
//! directories shaped like NeoFS.
//!
//! The disk is `neofs-commit-logic-tests`' disk: a write sits in a volatile
//! cache until a flush returns; power can be cut just before any write or
//! flush, and then any subset of the unflushed writes reaches the media.
//!
//! The volume keeps, like NeoFS, a directory as an inode block naming one
//! data block of entries, a file as an inode naming one data block, a free
//! map, and three superblock copies naming the root. Its operations are the
//! kernel's (`kernel/src/fs/neofs/inode/dir_ops.rs`), step for step:
//! - load the chain of directories from the root to each directory the
//!   operation edits;
//! - edit the entries (the rules are `dir_ops_logic`, the kernel's own);
//! - write a new version of every directory on the chains, bottom-up, each
//!   a data block then an inode block; for a rename across directories,
//!   each side's own tail first, then the directories both share;
//! - publish one superblock generation, freeing what it supersedes (every
//!   old directory version on the chains, and an unlinked entry's blocks)
//!   deferred to that generation (`commit_logic`, rule 2).
//!
//! Mount takes the highest superblock copy whose tree validates: hashes,
//! depth at most `MAX_DEPTH`, no block named twice, the free map disjoint
//! from the live set.

use std::collections::{BTreeMap, BTreeSet};

use crate::commit_logic::{DeferredFrees, DrainStep, Role, SuperblockDurability, next_drain_step};
use crate::dir_ops_logic as logic;

pub type Block = u64;

/// Superblock copy `i` lives in block `i`.
pub const COPIES: usize = 3;
const FIRST_BLOCK: Block = COPIES as Block;

/// An entry: name, child inode block, is a directory.
pub type Entry = (String, Block, bool);

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Content {
    Super {
        generation: u64,
        root: Block,
        root_hash: u64,
        free_map: Block,
        free_hash: u64,
    },
    /// `data` is `None` for an empty directory: no data block (the kernel's
    /// `write_directory_version`).
    DirInode {
        data: Option<Block>,
        data_hash: u64,
    },
    DirData(Vec<Entry>),
    FileInode {
        data: Block,
        hash: u64,
    },
    Data(Vec<u8>),
    FreeMap(Vec<Block>),
}

/// FNV-1a over the debug form: an independent content hash.
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
    /// has its bit in `land` reach the media.
    Cut {
        at: usize,
        land: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskError {
    Dead,
}

pub struct Disk {
    pub media: BTreeMap<Block, Content>,
    pending: Vec<(Block, Content)>,
    pub events: usize,
    fault: Fault,
    pub pending_at_event: Vec<usize>,
}

impl Disk {
    pub fn new(media: BTreeMap<Block, Content>, fault: Fault) -> Self {
        Self {
            media,
            pending: Vec::new(),
            events: 0,
            fault,
            pending_at_event: Vec::new(),
        }
    }

    fn event(&mut self) -> Result<(), DiskError> {
        let at = self.events;
        self.events += 1;
        self.pending_at_event.push(self.pending.len());
        if let Fault::Cut { at: cut, land } = self.fault
            && cut == at
        {
            for (i, (block, content)) in std::mem::take(&mut self.pending).into_iter().enumerate() {
                if i < 64 && land & (1 << i) != 0 {
                    self.media.insert(block, content);
                }
            }
            return Err(DiskError::Dead);
        }
        Ok(())
    }

    fn write(&mut self, block: Block, content: Content) -> Result<(), DiskError> {
        self.event()?;
        self.pending.push((block, content));
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DiskError> {
        self.event()?;
        let pending = std::mem::take(&mut self.pending);
        self.media.extend(pending);
        Ok(())
    }

    /// The end of the run: power goes with nothing more landing.
    pub fn power_off(&mut self) {
        self.pending.clear();
    }
}

/// How the drain orders its writes and flushes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// `commit_logic::next_drain_step`: the barrier before the superblocks.
    Barrier,
    /// No barrier: every role in order, one flush at the end (the
    /// falsifier: a cut can then land a superblock before its tree).
    SingleFlush,
}

/// The volume's tree: every path, `None` for a directory, a file's bytes.
pub type Tree = BTreeMap<String, Option<Vec<u8>>>;

/// Why an operation was refused (nothing was written).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    NotFound,
    Exists,
    NotEmpty,
    Pinned,
    TooDeep,
    IntoItself,
    NotDirectory,
}

#[derive(Clone)]
enum Node {
    Dir {
        data: Option<Block>,
        entries: Vec<Entry>,
    },
    File {
        data: Block,
        bytes: Vec<u8>,
    },
}

/// One directory on a chain: its name in its parent (`None`: the root), the
/// version read (inode and data block), and its entries being edited.
struct Frame {
    name: Option<String>,
    inode: Block,
    data: Option<Block>,
    entries: Vec<Entry>,
}

pub struct Volume {
    pub order: Order,
    generation: u64,
    root: Block,
    nodes: BTreeMap<Block, Node>,
    free_map_block: Option<Block>,
    next_free: Block,
    free: BTreeSet<Block>,
    deferred: DeferredFrees,
    durability: SuperblockDurability,
    data_unflushed: bool,
    flush_required: bool,
    dirty: BTreeMap<Block, (Role, Content)>,
    /// Every generation ever published: its tree.
    pub history: BTreeMap<u64, Tree>,
}

fn parts(path: &str) -> Vec<&str> {
    if path.is_empty() {
        Vec::new()
    } else {
        path.split('/').collect()
    }
}

impl Volume {
    /// A formatted volume with an empty root at generation 1, and its media.
    pub fn format(order: Order) -> (Self, BTreeMap<Block, Content>) {
        let mut v = Self {
            order,
            generation: 0,
            root: 0,
            nodes: BTreeMap::new(),
            free_map_block: None,
            next_free: FIRST_BLOCK,
            free: BTreeSet::new(),
            deferred: DeferredFrees::new(),
            durability: SuperblockDurability::new([true; COPIES], 1),
            data_unflushed: false,
            flush_required: false,
            dirty: BTreeMap::new(),
            history: BTreeMap::new(),
        };
        let mut root = [Frame {
            name: None,
            inode: 0,
            data: None,
            entries: Vec::new(),
        }];
        v.commit_tree(&mut root, &mut [], Vec::new(), false);
        let media = v.dirty.iter().map(|(b, (_, c))| (*b, c.clone())).collect();
        v.dirty.clear();
        v.flush_required = false;
        (v, media)
    }

    /// Blocks in use: allocated and not free.
    pub fn used(&self) -> usize {
        (self.next_free - FIRST_BLOCK) as usize - self.free.len()
    }

    fn allocate(&mut self) -> Block {
        self.deferred.release(self.durability.durable_generation());
        let reusable = self
            .free
            .iter()
            .copied()
            .find(|b| !self.deferred.is_deferred(*b));
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

    fn free_superseded(&mut self, blocks: &[Block]) {
        for &block in blocks {
            assert!(self.free.insert(block), "block {block} freed twice");
            self.deferred.defer(block, self.generation + 1);
        }
    }

    /// `write_directory_version`: a data block (none when empty), then an
    /// inode.
    fn write_dir(&mut self, entries: &[Entry]) -> Block {
        let content = Content::DirData(entries.to_vec());
        let data_hash = hash(&content);
        let data = if entries.is_empty() {
            None
        } else {
            let block = self.allocate();
            self.dirty(block, Role::RootDirectory, content);
            Some(block)
        };
        let inode = self.allocate();
        self.dirty(
            inode,
            Role::RootDirectory,
            Content::DirInode { data, data_hash },
        );
        self.nodes.insert(
            inode,
            Node::Dir {
                data,
                entries: entries.to_vec(),
            },
        );
        inode
    }

    fn load_chain(&self, dir: &[&str]) -> Result<Vec<Frame>, Refused> {
        let Some(Node::Dir { data, entries }) = self.nodes.get(&self.root) else {
            unreachable!("the root is a directory");
        };
        let mut frames = vec![Frame {
            name: None,
            inode: self.root,
            data: *data,
            entries: entries.clone(),
        }];
        for part in dir {
            let current = frames.last().expect("the root");
            let (_, child, is_dir) = current
                .entries
                .iter()
                .find(|(name, _, _)| name == part)
                .ok_or(Refused::NotFound)?;
            if !is_dir {
                return Err(Refused::NotDirectory);
            }
            let Some(Node::Dir { data, entries }) = self.nodes.get(child) else {
                return Err(Refused::NotDirectory);
            };
            frames.push(Frame {
                name: Some((*part).into()),
                inode: *child,
                data: *data,
                entries: entries.clone(),
            });
        }
        Ok(frames)
    }

    /// `commit_dir_tree`: each tail bottom-up into the shared chain's last
    /// directory, then the shared chain bottom-up, then one publish.
    fn commit_tree(
        &mut self,
        shared: &mut [Frame],
        tails: &mut [Vec<Frame>],
        mut superseded: Vec<Block>,
        frees_frames: bool,
    ) {
        if frees_frames {
            for frame in shared.iter().chain(tails.iter().flatten()) {
                superseded.push(frame.inode);
                superseded.extend(frame.data);
            }
        }
        let point = |entries: &mut Vec<Entry>, name: &str, block: Block| {
            let entry = entries
                .iter_mut()
                .find(|(n, _, _)| n == name)
                .expect("the child is named");
            entry.1 = block;
        };
        for tail in tails.iter_mut() {
            let mut update: Option<(String, Block)> = None;
            for idx in (0..tail.len()).rev() {
                if let Some((name, block)) = update.take() {
                    point(&mut tail[idx].entries, &name, block);
                }
                let entries = tail[idx].entries.clone();
                let block = self.write_dir(&entries);
                update = Some((tail[idx].name.clone().expect("named"), block));
            }
            if let Some((name, block)) = update {
                point(
                    &mut shared.last_mut().expect("shared").entries,
                    &name,
                    block,
                );
            }
        }
        let mut update: Option<(String, Block)> = None;
        for idx in (0..shared.len()).rev() {
            if let Some((name, block)) = update.take() {
                point(&mut shared[idx].entries, &name, block);
            }
            let entries = shared[idx].entries.clone();
            let block = self.write_dir(&entries);
            if idx == 0 {
                self.root = block;
            } else {
                update = Some((shared[idx].name.clone().expect("named"), block));
            }
        }
        for block in &superseded {
            self.nodes.remove(block);
        }
        self.free_superseded(&superseded);
        self.publish();
    }

    /// The new free map (the old one free in it), then the three copies.
    fn publish(&mut self) {
        let free_map = self.allocate();
        if let Some(old) = self.free_map_block {
            self.free_superseded(&[old]);
        }
        self.free_map_block = Some(free_map);
        let free_content = Content::FreeMap(self.free.iter().copied().collect());
        let free_hash = hash(&free_content);
        self.dirty(free_map, Role::FreeMap, free_content);
        self.generation += 1;
        let root_hash = hash(&self.dirty_or_node_content(self.root));
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
        let tree = self.tree();
        self.history.insert(self.generation, tree);
    }

    fn dirty_or_node_content(&self, inode: Block) -> Content {
        match self.dirty.get(&inode) {
            Some((_, content)) => content.clone(),
            None => unreachable!("the root was just written"),
        }
    }

    /// The tree as it is in memory now.
    pub fn tree(&self) -> Tree {
        let mut out = Tree::new();
        self.walk(self.root, "", &mut out);
        out
    }

    fn walk(&self, inode: Block, prefix: &str, out: &mut Tree) {
        let Some(Node::Dir { entries, .. }) = self.nodes.get(&inode) else {
            return;
        };
        for (name, child, _) in entries {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            match self.nodes.get(child) {
                Some(Node::File { bytes, .. }) => {
                    out.insert(path, Some(bytes.clone()));
                }
                Some(Node::Dir { .. }) => {
                    out.insert(path.clone(), None);
                    self.walk(*child, &path, out);
                }
                None => unreachable!("an entry names a live node"),
            }
        }
    }

    /// A regular file `name` in `dir` (NeoFS's create and write, as one
    /// commit: the data, the inode, then the chain).
    pub fn create(&mut self, dir: &str, name: &str, bytes: &[u8]) -> Result<(), Refused> {
        let dir = parts(dir);
        let mut frames = self.load_chain(&dir)?;
        let parent = frames.last_mut().expect("chain");
        if parent.entries.iter().any(|(n, _, _)| n == name) {
            return Err(Refused::Exists);
        }
        let data = self.allocate();
        self.dirty(data, Role::Data, Content::Data(bytes.to_vec()));
        let inode = self.allocate();
        self.dirty(
            inode,
            Role::Inode,
            Content::FileInode {
                data,
                hash: hash(&bytes.to_vec()),
            },
        );
        self.nodes.insert(
            inode,
            Node::File {
                data,
                bytes: bytes.to_vec(),
            },
        );
        frames
            .last_mut()
            .expect("chain")
            .entries
            .push((name.into(), inode, false));
        self.commit_tree(&mut frames, &mut [], Vec::new(), true);
        Ok(())
    }

    /// `mkdir_at`.
    pub fn mkdir(&mut self, dir: &str, name: &str) -> Result<(), Refused> {
        let dir = parts(dir);
        if !logic::depth_fits(&dir, 1) {
            return Err(Refused::TooDeep);
        }
        let mut frames = self.load_chain(&dir)?;
        if frames
            .last()
            .expect("chain")
            .entries
            .iter()
            .any(|(n, _, _)| n == name)
        {
            return Err(Refused::Exists);
        }
        let child = self.write_dir(&[]);
        frames
            .last_mut()
            .expect("chain")
            .entries
            .push((name.into(), child, true));
        self.commit_tree(&mut frames, &mut [], Vec::new(), true);
        Ok(())
    }

    /// `unlink_at`: a file, or an empty directory.
    pub fn unlink(&mut self, dir: &str, name: &str) -> Result<(), Refused> {
        let dir = parts(dir);
        if logic::is_pinned(&dir, name) {
            return Err(Refused::Pinned);
        }
        let mut frames = self.load_chain(&dir)?;
        let parent = frames.last_mut().expect("chain");
        let (_, child, _) = parent
            .entries
            .iter()
            .find(|(n, _, _)| n == name)
            .cloned()
            .ok_or(Refused::NotFound)?;
        let superseded = match self.nodes.get(&child) {
            Some(Node::File { data, .. }) => vec![child, *data],
            Some(Node::Dir { data, entries }) if entries.is_empty() => {
                let mut blocks = vec![child];
                blocks.extend(*data);
                blocks
            }
            Some(Node::Dir { .. }) => return Err(Refused::NotEmpty),
            None => unreachable!("an entry names a live node"),
        };
        logic::remove_named(&mut parent.entries, name, |e| e.0.as_str())
            .ok_or(Refused::NotFound)?;
        self.commit_tree(&mut frames, &mut [], superseded, true);
        Ok(())
    }

    /// `rename_at`, never replacing.
    pub fn rename(
        &mut self,
        src_dir: &str,
        src_name: &str,
        dst_dir: &str,
        dst_name: &str,
    ) -> Result<(), Refused> {
        let src_parts = parts(src_dir);
        let dst_parts = parts(dst_dir);
        if logic::is_pinned(&src_parts, src_name) {
            return Err(Refused::Pinned);
        }
        let common = logic::rename_check(&src_parts, src_name, &dst_parts, dst_name)
            .map_err(|_| Refused::IntoItself)?;
        let mut src_chain = self.load_chain(&src_parts)?;
        let mut dst_chain = self.load_chain(&dst_parts)?;
        let src_tail = src_chain.split_off(common + 1);
        let dst_tail = dst_chain.split_off(common + 1);
        let mut shared = src_chain;
        let mut tails = [src_tail, dst_tail];
        let moved = {
            let src = tails[0]
                .last()
                .unwrap_or_else(|| shared.last().expect("shared"));
            src.entries
                .iter()
                .find(|(n, _, _)| n == src_name)
                .cloned()
                .ok_or(Refused::NotFound)?
        };
        {
            let dst = tails[1]
                .last()
                .unwrap_or_else(|| shared.last().expect("shared"));
            if dst.entries.iter().any(|(n, _, _)| n == dst_name) {
                return Err(Refused::Exists);
            }
        }
        // As the kernel: only a move deeper walks the subtree.
        if dst_parts.len() > src_parts.len() && !logic::depth_fits(&dst_parts, self.height(moved.1))
        {
            return Err(Refused::TooDeep);
        }
        if tails[0].is_empty() && tails[1].is_empty() {
            let dir = shared.last_mut().expect("shared");
            let entry = dir
                .entries
                .iter_mut()
                .find(|(n, _, _)| n == src_name)
                .expect("found above");
            entry.0 = dst_name.into();
        } else {
            let src = match tails[0].last_mut() {
                Some(frame) => frame,
                None => shared.last_mut().expect("shared"),
            };
            logic::remove_named(&mut src.entries, src_name, |e| e.0.as_str()).expect("found above");
            let dst = match tails[1].last_mut() {
                Some(frame) => frame,
                None => shared.last_mut().expect("shared"),
            };
            dst.entries.push((dst_name.into(), moved.1, moved.2));
        }
        self.commit_tree(&mut shared, &mut tails, Vec::new(), true);
        Ok(())
    }

    /// As the kernel's `subtree_height`.
    fn height(&self, inode: Block) -> usize {
        match self.nodes.get(&inode) {
            Some(Node::Dir { entries, .. }) => entries
                .iter()
                .map(|(_, child, _)| 1 + self.height(*child))
                .max()
                .unwrap_or(1)
                .max(1),
            _ => 0,
        }
    }

    /// Drain the cache in this volume's order; stops at the first disk error.
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

/// Mount as NeoFS does: validate each copy, take the highest valid one.
pub fn mount(media: &BTreeMap<Block, Content>) -> Option<(u64, Tree)> {
    let mut best: Option<(u64, Tree)> = None;
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
        let Some(tree) = validate(media, *root, *root_hash, *free_map, *free_hash) else {
            continue;
        };
        if best.as_ref().is_none_or(|(g, _)| generation > g) {
            best = Some((*generation, tree));
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
) -> Option<Tree> {
    if hash(media.get(&root)?) != root_hash {
        return None;
    }
    let mut live: BTreeSet<Block> = [free_map].into_iter().collect();
    let mut tree = Tree::new();
    walk_media(media, root, "", 0, &mut live, &mut tree)?;
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
    Some(tree)
}

fn mark(live: &mut BTreeSet<Block>, block: Block) -> Option<()> {
    live.insert(block).then_some(())
}

/// `mark_inode_tree`: a directory at `depth` (the root is 0); each block
/// live once; nothing deeper than `MAX_DEPTH`.
fn walk_media(
    media: &BTreeMap<Block, Content>,
    inode: Block,
    prefix: &str,
    depth: usize,
    live: &mut BTreeSet<Block>,
    tree: &mut Tree,
) -> Option<()> {
    if depth > logic::MAX_DEPTH {
        return None;
    }
    let Content::DirInode { data, data_hash } = media.get(&inode)? else {
        return None;
    };
    mark(live, inode)?;
    let entries = match data {
        // An empty directory names no data block.
        None => {
            if *data_hash != hash(&Content::DirData(Vec::new())) {
                return None;
            }
            Vec::new()
        }
        Some(data) => {
            let data_content = media.get(data)?;
            if hash(data_content) != *data_hash {
                return None;
            }
            let Content::DirData(entries) = data_content else {
                return None;
            };
            if entries.is_empty() {
                return None;
            }
            mark(live, *data)?;
            entries.clone()
        }
    };
    for (name, child, is_dir) in &entries {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if *is_dir {
            tree.insert(path.clone(), None);
            walk_media(media, *child, &path, depth + 1, live, tree)?;
        } else {
            if depth + 1 > logic::MAX_DEPTH {
                return None;
            }
            let Content::FileInode { data, hash: h } = media.get(child)? else {
                return None;
            };
            let Content::Data(bytes) = media.get(data)? else {
                return None;
            };
            if hash(bytes) != *h {
                return None;
            }
            mark(live, *child)?;
            mark(live, *data)?;
            tree.insert(path, Some(bytes.clone()));
        }
    }
    Some(())
}
