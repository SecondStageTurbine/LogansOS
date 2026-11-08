<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: an index for the NeoFS block cache (the follow-up to #69)

**Date:** 2026-11-08. **Seat:** E. **Status:** proposal for the PM. Updated the same day: option B is prototyped on `claude/neofs-cache-index`
(`06993824`, no PR yet); see §9 for what the prototype changed and measured.
**Builds on:** #69 (`kernel/src/fs/neofs/cache_logic.rs`, landed `97874d3b`), OT-92 (`neofs-cache-evict`),
`kernel/src/fs/neofs/commit_logic.rs` (the write-back order).

## 1. The problem

`find_cached_block` (`kernel/src/fs/neofs/cache.rs:760`, now `cache_logic::find_block`) walks `CacheInner::blocks`, a
`Vec<CachedBlock>`, from the front on every read, write, hit and drain step. #69 made each step cheaper; it did not
change the shape. The cost per lookup is the number of cached blocks, so a large read-back is quadratic.

| Build | Cache size during the installer's W5 verify | Effect |
|---|---|---|
| neo-test kernels without `neofs-cache-evict` (`install_fresh_4k` and most scenarios) | never shrinks: every block read or written stays (about 35,000 blocks for the 140 MB the installer writes) | each verify reads slower than the last (12.6, 17.4, 21.6, 28.0 s for files of about the same size on main, before #69); `install_fresh_4k` was 465 s of its 600 s on hosted runners before #69, 292-307 s after |
| `product` (has `neofs-cache-evict`, `kernel/Cargo.toml:159`) | at most 256 clean blocks plus the dirty ones (`CACHE_CAPACITY_BLOCKS`, `cache.rs:48`) | bounded while reads dominate; a single large mutation still dirties one block per 4 KiB it writes (the comment at `cache.rs:38` names this: truncating an empty file to 300 blocks makes the next insert scan 301), and dirty blocks are never evicted, so writing a 24 MB file scans about 6,000 entries per block until the drain catches up |

Two more linear walks sit next to it:
- `evict_locked` (`cache.rs:813`) finds the least recently used clean block with a full scan, once per insert over
  capacity (eviction on only).
- `drain_pass` (`cache.rs:903`) builds `next_drain_step`'s input from a full scan per step. `commit_logic::next_drain_step`
  (`commit_logic.rs:68`) takes `min_by_key` on the role, so among blocks of the same role **the first in `Vec`
  order is written first**. That order is part of today's behaviour and the kill-switch must keep it.

## 2. Constraints

1. **Bit-for-bit kill-switch** (complexity-governance rule 1): with the index off, the cache must behave as it does
   today. With it on, every lookup must return the same block as the linear scan. That includes the write-back
   order, which depends on `Vec` order.
2. **Same lock, no new lock.** Everything runs under `CacheInner`'s `IrqSafeMutex` (IF=0). The index is a field of
   `CacheInner`, so it is updated in the same critical sections as `blocks`.
3. **Allocation under the lock is already normal here** (`push_block` grows the `Vec` under it; the heap is
   `IrqSafeHeap`), so a tree node allocation adds no new hazard class.
4. **No new dependency**, and nothing in a hot path that only the product build exercises.

## 3. Options

| # | Design | Lookup | Insert | Remove (eviction, `forget_range`) | Keeps `Vec` order (the drain's tie-break) | Size of change |
|---|---|---|---|---|---|---|
| A | Turn `neofs-cache-evict` on for the scenarios that mirror the product (the installer ones) | linear, bounded by 256 + dirty | as today | as today | yes | a feature line per scenario; changes what those scenarios test |
| B | **Side index:** a map of block number to slot next to the `Vec` (a sorted `Vec<(block, slot)>`, see §9); `Vec::remove` stays, and the index's slots above the removed one are shifted down | O(log n) | O(log n) | O(n), as `Vec::remove` already is | **yes**, the `Vec` is untouched | small: `cache_logic` gains the index ops, `cache.rs` calls them at the four places that change `blocks` |
| C | Key the cache by block: `BTreeMap<u64, CachedBlock>` plus an insertion sequence number for the drain's tie-break | O(log n) | O(log n) | O(log n) | only if every scan sorts by the sequence number | large: every `idx` user in `cache.rs` changes, and the drain must sort |
| D | Slot arena with stable slot ids and a hash map (`hashbrown`) | O(1) | O(1) | O(1), but free slots break `Vec` order | needs a separate order list | large, and a new vendored crate |

**Recommendation: B, behind a kill-switch feature (`neofs-cache-index`, on in `product` and in the default test
build once the KATs and the scenarios are green).**
- It is the only option that changes lookup cost without touching the order the drain relies on, so "index on"
  can be checked against "index off" exactly. The KAT runs both over the same operations and compares every result.
- Removal stays O(n). That is no regression, because `Vec::remove` is already O(n) and removals are rare: only
  eviction (product) and `forget_range`.
- C and D are faster on removal but change the drain's iteration order or need a new crate. Neither is worth it while
  removal is rare.

**Separately, for the PM (not part of B):** whether the installer scenarios should run with `neofs-cache-evict` as
the product does (option A). It makes them test what ships, but it is a test-coverage decision, not a fix: the
unbounded cache is also a test of the no-evict layer, which stays the kill-switch's "off" leg. I'd keep at least one
installer scenario in each mode.

## 4. B in detail

- **`cache_logic.rs` (pure, host-tested)** gains `BlockIndex` (a `Vec<(block, slot)>` sorted by block; §9 says why
  not a `BTreeMap`) with `find`, `pushed`, `removed(slot, block)` (drops that block's entry and shifts every slot
  above it down by one), `rebuild(&[T])`, and `matches(&[T])` for the KATs and the crosscheck scenarios.
- **`cache.rs`:** `find_cached_block` uses the index when the feature is on, else today's loop. The four places
  that change `blocks` keep the index in step: `push_block`, the `remove` in `evict_locked`, the `remove` loop and
  the `retain` in `forget_range` (the `retain` becomes a rebuild).
- **Checks:** in the crosscheck scenarios, every lookup runs through both the index and the scan, which must
  agree, and `matches` checks the index against a rebuild after every change. A missed update then fails at the
  change, not at a wrong read later. Not in every debug kernel: a rebuild per change costs more than the scan.
- **The evict scan and the drain scan** stay linear in this change. The drain's scan covers dirty blocks, which the
  throttle bounds between mutations. An ordered LRU index for `evict_locked` is a possible later step, which is only
  worth doing if a profile shows it.

## 5. Proof plan

**Host KATs (`tools/neofs-cache-logic-tests`):**
- hand-written sequences: insert 10, 20, 30; remove slot 1; find 30 → slot 1; find 20 → none; `forget_range` over a
  middle range; rebuild after `retain`;
- a model check: random operation sequences (fixed seeds) applied to a `Vec` with the linear `find_block` and to the
  `Vec` with the index, comparing every lookup and checking `check` after every step.

**Scenarios:**
- the existing NeoFS cache set (`phase3_neofs_cache_*`, `neofs_large_file`, `neofs_overwrite_frees`, the writeback and
  crash ones, and the OT-92 `neofs-cache-evict` and `-off` legs), with the index on and off;
- a new kill-switch scenario, run with the index off, that proves the drain writes the same blocks in the same order
  as with it on. A dirty-heavy write that records its `write_fs_block` order should do it.

**Timing:** `install_fresh_4k` boot 1, locally and on hosted runners, with the same comparison table as #69:

| Main | Index on | Index off | Hosted shard 4 |
|---|---|---|---|

The expectation is that the W5 verify stops slowing down file by file, which the serial timestamps show directly.

## 6. Prior art (survey rule, 2026-10-27)

| Project | What it does | Licence | Last commit (checked 2026-11-08) | Fit / what we take |
|---|---|---|---|---|
| Rust `alloc::collections::BTreeMap` (toolchain) | ordered map, `no_std` with `alloc` | MIT OR Apache-2.0 | ships with our pinned toolchain | tried first, then **replaced by a sorted `Vec`** (§9): its node allocations fragmented the kernel heap |
| [rust-lang/hashbrown](https://github.com/rust-lang/hashbrown) | SwissTable hash map, `no_std` | MIT OR Apache-2.0 | `f7bae3d`, 2026-10-31; active | option D; not needed while O(log n) is enough and the order must hold |
| [jeromefroe/lru-rs](https://github.com/jeromefroe/lru-rs) | O(1) LRU cache (hash map plus intrusive list), `no_std` | MIT | `3ed2e28`, 2026-10-18; active | design reference for a later O(1) `evict_locked`; no code taken |
| [tokio-rs/slab](https://github.com/tokio-rs/slab) | slot arena with stable keys, `no_std` | MIT | `7d1d8ab`, 2026-08-16; active | the slot half of option D; no code taken |
| [Amanieu/intrusive-rs](https://github.com/Amanieu/intrusive-rs) | intrusive lists and trees, `no_std` | MIT OR Apache-2.0 | `0a89564`, 2026-08-04; active | an ordered intrusive LRU if one is ever needed; no code taken |
| Linux page cache (`mm/filemap.c`, the XArray) | per-file radix tree keyed by page index | GPL-2.0 | n/a | **principle only, nothing copied**: caches keyed by block number, never scanned |
| FreeBSD buffer cache (`sys/kern/vfs_bio.c`, per-vnode trie) | buffers looked up by logical block through a per-object trie | BSD-2-Clause | n/a | principle only: the same keyed lookup, with the LRU list kept separately from the index, which is the shape of B plus a later LRU index |

## 7. Concurrency pitfall bank (CP consult)

- **Lock ordering:** no new lock. The index is a field under `CacheInner`'s existing `IrqSafeMutex`.
- **Torn state between `blocks` and the index:** guarded, because both change in the same critical section, and the
  debug `check` is the discriminator.
- **Allocation with IF=0:** unchanged class, as `push_block` already allocates there.
- **Read outside the lock, then insert** (`insert_read_locked`, the OT-92 epoch check): unchanged. The index does not
  move the miss path out of the lock; it is consulted under it as the scan is.
- None of the scheduler, IPI, TLB or atomics classes are touched.

## 8. Decision asked of the PM

1. Option B as described, behind `neofs-cache-index`, on in `product` once green. One PR, with the KATs and the
   timing table.
2. Whether some installer scenarios should also run with `neofs-cache-evict` (option A, a coverage question), keeping
   at least one in each mode.

## 9. Prototype results (2026-11-08)

The prototype of option B, `claude/neofs-cache-index` at `06993824`, pushed but with no PR, is built as §4 says,
with one change of data structure.

**A `BTreeMap` index made the install slower, because of the heap, not the lookup.**
- With it, the W5 verify ran in 31 s instead of 64 s. But the read-only mount's drop after the verify took **38 s,
  against 1 s** with no index.
- Guest RIP samples in that window were all in `linked_list_allocator::hole::deallocate`.
- The cause: the kernel heap keeps one address-ordered free list. The tree's roughly 3,000 small nodes sat between
  the cache's 4 KiB buffers, so each freed buffer became a hole of its own, and every free walked them all.
- A `Vec<(block, slot)>` sorted by block is a handful of allocations, and an insert is nearly always an append
  (the installer and the verify read in nearly ascending block order).
- This is worth knowing for any kernel code that holds many small allocations next to many page-sized ones.

**Measured locally** (QEMU 8.2.2 under TCG, the pinned OVMF; `cargo xtask neo-test install_fresh_4k`, boot 1,
serial timestamps):

| Build | Boot 1 | W5 verify | Verify end to "install done" |
|---|---|---|---|
| main `13eed2c5` | 133.2, 133.7 s | 63.7, 63.7 s | 1.0 s |
| index, `BTreeMap` (discarded) | 131.1 s | 31.1 s | 38.3 s |
| **index, sorted `Vec` (`06993824`)** | **87.2, 86.5 s** | **26.8, 26.9 s** | 1.0 s |

**Scenarios, all PASS locally:**
- `phase3_neofs_cache_index_crosscheck` (40.7 s), `_crosscheck_noevict` (11.1 s) and `phase3_neofs_cache_index_off`
  (37.2 s, the kill-switch leg);
- the OT-92 pair, `phase3_neofs_cache_evict` (35.5 s) and `_off` (10.8 s), with the index on by default.

**Falsified:**
- With the slot shift in `removed` disabled, the crosscheck scenario panics with "neofs cache index out of step with
  the cache (256 blocks)".
- Three host KATs fail with the same break, the model check among them.

**Host KATs:** 10 pass (`tools/neofs-cache-logic-tests`), among them a 64-seed model check against the scan, with
and without repeated blocks.

**Still to do before the PR:** merge main (the branch is behind `91f70620`), then the close-gate. The hosted
`install_fresh_4k` time will be in the PR run.

