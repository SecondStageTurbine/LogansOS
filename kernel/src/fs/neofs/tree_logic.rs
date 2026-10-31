// SPDX-License-Identifier: MPL-2.0
//! NFS-W1: the block tree of a file written in one piece
//! (`docs/specs/neofs-format.md` §4.3). Pure: the kernel's large-file writer
//! uses it, and `tools/neofs-tree-logic-tests` holds it to a hand-laid byte
//! oracle and to an independent walk that mirrors the kernel's reader.
//!
//! The writer allocates the file's `n` data blocks as one contiguous run
//! starting at `data`, and its pointer blocks as one run starting at `ptr`.
//! Logical block `i` is then physical block `data + i`, so every pointer a
//! pointer block holds follows from its position, and each pointer block can
//! be produced, written and dropped one at a time: the writer holds no list
//! of block numbers, whatever the file's size.
//!
//! Pointer blocks are numbered in this order (only the ones the file needs):
//!
//! 1. the single-indirect root (logical blocks 8..520);
//! 2. the double-indirect root, then its leaves in order;
//! 3. the triple-indirect root, then each level-2 block followed by its
//!    leaves, in order.

/// Logical blocks the inode addresses directly.
pub const DIRECT: u64 = 8;
/// `u64` pointers in one 4096-byte pointer block.
pub const PTRS: u64 = 512;
/// Logical blocks under one double-indirect root.
pub const DOUBLE: u64 = PTRS * PTRS;
pub const BLOCK_BYTES: usize = 4096;

/// How the logical blocks split across the inode's slots.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Split {
    single: u64,
    double: u64,
    triple: u64,
}

fn split(n: u64) -> Split {
    let rest = n.saturating_sub(DIRECT);
    let single = rest.min(PTRS);
    let rest = rest - single;
    let double = rest.min(DOUBLE);
    Split {
        single,
        double,
        triple: rest - double,
    }
}

/// Pointer blocks the double-indirect level needs for `double` data blocks:
/// the root and its leaves.
fn double_blocks(double: u64) -> u64 {
    if double == 0 {
        0
    } else {
        1 + double.div_ceil(PTRS)
    }
}

/// Pointer blocks the triple-indirect level needs for `triple` data blocks:
/// the root, the level-2 blocks and their leaves.
fn triple_blocks(triple: u64) -> u64 {
    if triple == 0 {
        0
    } else {
        1 + triple.div_ceil(DOUBLE) + triple.div_ceil(PTRS)
    }
}

/// How many pointer blocks a file of `n` data blocks needs.
pub fn pointer_blocks(n: u64) -> u64 {
    let s = split(n);
    u64::from(s.single != 0) + double_blocks(s.double) + triple_blocks(s.triple)
}

/// One file's tree: `n` data blocks from `data`, its pointer blocks from
/// `ptr`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tree {
    pub n: u64,
    pub data: u64,
    pub ptr: u64,
}

/// What a pointer block's entries point at: data blocks, or other pointer
/// blocks (by their index in this tree's pointer run).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Entries {
    /// Data blocks `first..first + count` (logical numbers).
    Data { first: u64, count: u64 },
    /// Pointer blocks with indices `first + k * stride` for `k < count`.
    Pointers { first: u64, count: u64, stride: u64 },
}

impl Tree {
    /// The inode's `direct_blocks` (zero past the file's end).
    pub fn direct(&self) -> [u64; 8] {
        let mut out = [0u64; 8];
        for (i, slot) in out.iter_mut().enumerate().take(self.n.min(DIRECT) as usize) {
            *slot = self.data + i as u64;
        }
        out
    }

    /// The inode's `single_indirect_block`, `double_indirect_block` and
    /// `triple_indirect_block` (zero when unused).
    pub fn roots(&self) -> (u64, u64, u64) {
        let s = split(self.n);
        let mut next = self.ptr;
        let mut take = |used: bool, blocks: u64| {
            let root = if used { next } else { 0 };
            next += blocks;
            root
        };
        let single = take(s.single != 0, u64::from(s.single != 0));
        let double = take(s.double != 0, double_blocks(s.double));
        let triple = take(s.triple != 0, triple_blocks(s.triple));
        (single, double, triple)
    }

    /// What pointer block `i` (0-based in the pointer run) holds.
    fn entries(&self, i: u64) -> Option<Entries> {
        let s = split(self.n);
        let mut base = 0u64;
        if s.single != 0 {
            if i == 0 {
                return Some(Entries::Data {
                    first: DIRECT,
                    count: s.single,
                });
            }
            base = 1;
        }
        let first_double = DIRECT + PTRS;
        if s.double != 0 {
            let leaves = s.double.div_ceil(PTRS);
            if i == base {
                return Some(Entries::Pointers {
                    first: base + 1,
                    count: leaves,
                    stride: 1,
                });
            }
            if i <= base + leaves {
                let leaf = i - base - 1;
                let first = first_double + leaf * PTRS;
                let count = (s.double - leaf * PTRS).min(PTRS);
                return Some(Entries::Data { first, count });
            }
            base += 1 + leaves;
        }
        let first_triple = first_double + DOUBLE;
        if s.triple != 0 {
            let level2s = s.triple.div_ceil(DOUBLE);
            // Each level-2 block is followed by its own leaves.
            let group = |j: u64| {
                let data = (s.triple - j * DOUBLE).min(DOUBLE);
                1 + data.div_ceil(PTRS)
            };
            if i == base {
                // The root: the level-2 blocks, each at the start of its
                // group. Every group before the last is full (1 + 512
                // blocks), so they are evenly spaced.
                return Some(Entries::Pointers {
                    first: base + 1,
                    count: level2s,
                    stride: 1 + PTRS,
                });
            }
            let mut start = base + 1;
            for j in 0..level2s {
                let len = group(j);
                if i < start + len {
                    let data = (s.triple - j * DOUBLE).min(DOUBLE);
                    if i == start {
                        return Some(Entries::Pointers {
                            first: start + 1,
                            count: data.div_ceil(PTRS),
                            stride: 1,
                        });
                    }
                    let leaf = i - start - 1;
                    let first = first_triple + j * DOUBLE + leaf * PTRS;
                    let count = (data - leaf * PTRS).min(PTRS);
                    return Some(Entries::Data { first, count });
                }
                start += len;
            }
        }
        None
    }

    /// Pointer block `i` (0-based): its physical block and its bytes, 512
    /// little-endian `u64` pointers, zero past the last used one. `None` past
    /// [`pointer_blocks`].
    pub fn pointer_block(&self, i: u64) -> Option<(u64, [u8; BLOCK_BYTES])> {
        let mut out = [0u8; BLOCK_BYTES];
        let block = self.pointer_block_into(i, &mut out)?;
        Some((block, out))
    }

    /// [`Self::pointer_block`] into `out`, which the caller owns (the kernel
    /// keeps it on the heap, off the stack). Returns the physical block.
    pub fn pointer_block_into(&self, i: u64, out: &mut [u8; BLOCK_BYTES]) -> Option<u64> {
        let (count, at): (u64, &dyn Fn(u64) -> u64) = match self.entries(i)? {
            Entries::Data { first, count } => (count, &move |k| self.data + first + k),
            Entries::Pointers {
                first,
                count,
                stride,
            } => (count, &move |k| self.ptr + first + k * stride),
        };
        out.fill(0);
        for k in 0..count {
            let at_byte = k as usize * 8;
            out[at_byte..at_byte + 8].copy_from_slice(&at(k).to_le_bytes());
        }
        Some(self.ptr + i)
    }
}
