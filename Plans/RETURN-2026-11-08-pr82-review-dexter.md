<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 341: #82 (NeoFS block-cache index build) review

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §341, which runs `Plans/DISPATCH-2026-11-08-relays-329-332.md` §331 as written.
**Reviewed:** PR #82 head `c30975a38b2d06d82a952391f460a26ea62fc57f` (`claude/neofs-cache-index`), base `856d1820a593d39a73e0622dde496009776f38f2` (`main`; the PR merge-base). The `kernel/src/fs/neofs` code is identical at `cc04384f7` and `c30975a3` (`git diff cc04384f c30975a3 -- kernel/src/fs/neofs` is empty); the later commit only merges `origin/main`. The design is `docs/THINK-2026-11-08-neofs-cache-index.md` at `4b2628c0` (PR #73), whose §4/§9 the build must match.
**Overall: PASS on all six checklist items.** The index is coherent at every `Vec` mutation, one entry per block is enforced, the debug check is incremental, the lock order is unchanged, the `-off` build is the previous layer in the emitted code, and the `install_fresh_4k` pair compares the same source tree/build with the index the only code difference. I verified the host KATs (12 pass), the `xtask` defaults test, and both the `-off` and true-previous-layer kernel compiles. Seven non-blocking notes are listed at the end.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| 1 | `BlockIndex` stays in step with the cache's `Vec` on insert, eviction, write-back and invalidation; each mutation site named | **PASS**, all four sites | `cache.rs:870,882,886` (insert); `:934,940,941` (eviction); `:1091` (write-back, no index change needed); `:416,423,424` and `:435,442,443` (invalidation); `cache_logic.rs:55-139` |
| 2 | One entry per (device, block): enforced by a debug assert, and a host KAT covers a double insert | **PASS** | `cache_logic.rs:79-93`; `cache.rs:884`; `cache.rs:154,186`; `inode.rs:197`; KAT `tools/neofs-cache-logic-tests/src/lib.rs:170-179` |
| 3 | The debug check is incremental, so the debug-assertions build's write path is not quadratic | **PASS** | `cache.rs:834-856`; `cache.rs:886,941,424,443`; full `matches` only under `neofs-cache-index-crosscheck` (`:848-854`) |
| 4 | Lock order: index only under `inner`; `drain → inner` never reversed; no lookup under a lock eviction takes the other way | **PASS** | `cache.rs:145-150`; `:798-820` (lookup, guard held by callers); `:463-478,550-557,577-578` (`drain → inner`); `:934-941` (`inner` only) |
| 5 | Kill-switch: the index-off build is #69's linear scan, bit for bit in behaviour | **PASS** (one wording caveat) | `kernel/Cargo.toml:81,161,1450,1728`; `cache.rs:799-820`; `test_harness.rs:1194-1207,34442-34450` |
| 6 | The timing claim (`install_fresh_4k` boot 1 ~92 s vs ~133 s) is from the same image and build on both sides | **PASS** | THINK `4b2628c0` §9; tree identity below; `tools/xtask/src/main.rs:7097-7108` |

## 1. PASS — the index tracks all four `Vec` mutation sites

The only writes to `inner.blocks` are `push`, `remove` (two sites) and `retain`; every one is paired with an index call in the same critical section, and the index call is beside the `Vec` mutation:

- **Insert — `push_block`, `cache.rs:870`** (`inner.blocks.push(..)`), index at `:882` (`inner.index.pushed(&inner.blocks)`), then `check_index` at `:886`. `pushed` indexes the new last slot; its `false` path is the double-insert report (item 2).
- **Eviction — `evict_locked`, `cache.rs:934`** (`let block = inner.blocks.remove(victim).block`), index at `:940` (`inner.index.removed(&inner.blocks, victim, block)`), then `check_index` at `:941`. The `removed` call gets the post-removal `Vec`, so it can find the next first entry for a block if a duplicate ever existed; it shifts every indexed slot above `victim` down by one (`cache_logic.rs:102-119`).
- **Write-back — `drain_pass`, `cache.rs:1091`** sets `inner.blocks[idx].dirty = false` and nothing else. It never changes `blocks`' length, block numbers or order, so no index update is needed. Its lookup at `:1086` goes through `find_cached_block(&inner, block)`, i.e. the index when on. This is the THINK's "write-back is a no-op for the index" (§4), and it is true in the build.
- **Invalidation — `forget_range`, no-evict branch `cache.rs:416`** (`retain`), index rebuilt at `:423` (`inner.index.rebuild(&inner.blocks)`), then `check_index` at `:424`. **Evict branch `:435`** (`inner.blocks.remove(idx)`), index at `:442` (`inner.index.removed(&inner.blocks, idx, block)`), then `check_index` at `:443`.

The lookup is `cache.rs:798-820`: index `find` at `:804`, and under `neofs-cache-index-crosscheck` an always-on `assert_eq!` against `cache_logic::find_block(&inner.blocks, block)` at `:806-812`. The index itself is `cache_logic.rs:55-139`: a `Vec<(u64, usize)>` sorted by block, `binary_search_by_key` (`:67`), `find` (`:71`), `pushed` (`:79`), `removed` (`:102`), stable `rebuild`/dedup (`:121-133`), `matches` (`:135`). A concrete check: after a push of block 30 into `[10,20]`, `index.find(30) == Some(2)`; after `remove(1)`, `index.find(20) == None` and `index.find(30) == Some(1)` — the KAT `removing_a_slot_moves_the_later_ones_down` exercises exactly this and passes.

No site is missed: `blocks` has no other `push`/`remove`/`retain`/`clear`/`swap_remove`/`sort` (grep at c30975a3), and `touch`/`mark_dirty`/the drain's `dirty = false` change only per-entry fields, not the key or order.

## 2. PASS — one entry per block is enforced, and the double insert is in the KAT

`BlockIndex::pushed` (`cache_logic.rs:79-93`) reports a block that is already indexed (`false`) and keeps the first slot; `push_block` `debug_assert!`s that the push was indexed (`cache.rs:884`, message "neofs cache: block {} pushed twice"). The scope is one cache per device: `CacheInner` is inside `NeoFsBlockCache`, which owns exactly one `device` (`cache.rs:154,186`), constructed per mount at `inode.rs:197`, so the key is the block number, not a composite key.

The host KAT `a_double_insert_is_reported_and_the_first_slot_kept` (`tools/neofs-cache-logic-tests/src/lib.rs:170-179`) is the concrete input: push 5, push 9, push 5 again; it asserts the third push returns `false`, `index.find(5) == Some(0)` (the first slot), and `index.len() == 2` while the entries `Vec` has three. I ran `cargo test -p neofs-cache-logic-tests` at `c30975a3`: **12 passed, 0 failed**, including this test, `one_entry_per_block_keeps_the_lengths_equal` (`:181-189`), and the 64-seed model check (`:218`).

Every push path does follow a lookup miss under the same `inner` guard: `read_block`/`read_block_prefix` re-check `find_cached_block` after the device read and before `insert_read_locked` (`cache.rs:248-255,281-290`); `write_bytes_at` re-checks under the lock before `push_block` (`:376-388`); `write_block`/`find_or_insert_full_block` check then push under the lock (`:316,858-866`). So the invariant is structural, not just asserted.

## 3. PASS — the debug check is incremental

`check_index` (`cache.rs:834-856`) does exactly what the THINK §4 says, after each index change:

- `debug_assert_eq!(inner.index.len(), inner.blocks.len())` — O(1);
- after a push only, `debug_assert_eq!(inner.index.find(block), Some(slot))` — O(log n);
- the full `inner.index.matches(&inner.blocks)` comparison is behind `#[cfg(feature = "neofs-cache-index-crosscheck")]` (`:848-854`), so it runs only in the crosscheck scenarios and the KATs, not in every debug kernel.

It is called at the push (`:886`), the two removals (`:941`, `:443`), and the rebuild (`:424`); a hit (`read_block` at `:236-241`) calls only `touch` and runs no check. Concrete check: a hit on a cached block changes `last_use` and nothing else, so the W5 verify's reads do not trigger a check; the W5 write path pays one O(1) length check plus one O(log n) slot check per mutated block, not a full scan. The index's own `Vec::insert` at `cache_logic.rs:90` is O(n) worst case (near-append for the installer's/verify's ascending blocks), which is the data-structure cost the THINK §9/§3 already states — it is not the debug check. The measured ~5 s delta between `06993824` (no checks) and `b1b49a31` (checks) is consistent with that.

## 4. PASS — the index adds no lock and no edge

The index is a field of `CacheInner` (`cache.rs:145-150`), so it is only reachable through the `inner` guard. Every index operation is inside a critical section that already holds `inner`:

- lookup: `find_cached_block(&inner, ..)` at `:235,248,267,281,359,376,388,1086`; all callers hold the guard;
- push: `push_block` (`:867-888`) is reached from `find_or_insert_full_block` under the `write_block` guard (`:316`), from `write_bytes_at`'s loop guard (`:376-388`), and from `insert_read_locked` called under the read guards (`:248-255`, `:281-290`);
- eviction: `evict_locked` (`:917-945`) takes only `inner`, called from `:318,391,919`;
- invalidation: `forget_range` takes `inner` at `:410` before the retain/remove/index calls.

The existing order is `drain → inner` and is unchanged: `drain_all` takes `drain` at `:464` and `drain_pass` takes `inner` inside it (`:1029`, `:1061`, `:1076`, `:1097`); `drain_worker_cycle` takes and drops `inner` before `drain.try_lock()` (`:550-557`); `debug_drain_to_barrier` takes `drain` then `inner` (`:577-578`). No path holds `inner` and then takes `drain`. `evict_locked` is never called under `drain`, and the index is not touched outside `inner`, so there is no lookup under a lock that eviction takes the other way. CP-14/CP-33 are not engaged: the index adds no wake token, no park path, and no teardown path; the `inner` lock's release-token behaviour for the writeback worker is untouched.

## 5. PASS — the `-off` build is the previous layer, with one wording caveat

The feature is `neofs-cache-index = []` (`kernel/Cargo.toml:1450`), on in `default` (`:81`) and `product` (`:161`). Every use is gated by `all(feature = "neofs-cache-index", not(feature = "neo-test-neofs-cache-index-off"))`:

- the field and initialiser: `cache.rs:145-150,224-228`;
- `forget_range`'s rebuild/remove paths: `:417-424,436-443`;
- the lookup branch: `:799-820` (the fallback is the scan);
- `check_index` itself: `:829-832`;
- `push_block`'s upkeep/check: `:876-886`;
- `evict_locked`'s removal/check: `:935-941`.

The scenario `phase3_neofs_cache_index_off` is `neo-test-neofs-cache-index-off = ["neo-test-phase3-neofs-cache-evict"]` (`kernel/Cargo.toml:1728`), and it runs `run_neofs_cache_evict` with `FILES = 40` and `evict_on = true` (`test_harness.rs:34442-34450,34605`) — the same legs as `phase3_neofs_cache_evict`, only the scenario name and the off flag differ. I built the off kernel at `c30975a3` with `--features neo-test,neo-test-neofs-cache-index-off`: it compiles, and the ELF has **no** `BlockIndex`/`check_index` symbols and **no** "neofs cache index" assertion strings; the only cache scenario name in it is `phase3_neofs_cache_index_off`. The off `find_cached_block` fallback is `cache_logic::find_block(&inner.blocks, block)` (`cache.rs:820`), identical to the pre-PR function, and the retain/remove/push bodies are the pre-PR bodies with the index blocks compiled out.

Caveat, same as relay 338: `neofs-cache-index` remains in `default`, and a Cargo feature cannot unset another, so the `-off` build still passes `--cfg 'feature="neofs-cache-index"'`; what makes it the previous layer is that every *use* is excluded. The THINK at `4b2628c0` says "compiles every part of the index out", which is true of the code; the literal "no `neofs-cache-index` cfg is active" would be false. The `test_harness.rs:1200-1201` comment ("or (`-off`) with every lookup by scan") is true but incomplete; it should say "with the index compiled out", as the other three comments do.

## 6. PASS — the two timings are the same tree/build with the index the only code difference

The THINK `4b2628c0` §9 table's 133 s arm is `main 13eed2c5`; the 92 s arm is `b1b49a31` (the same index with the incremental checks). The provenance is clean:

- `06993824`'s parent is `42ccc9238`; `b1b49a31`'s parent is `06993824`.
- `13eed2c5` and `42ccc9238` have identical code subtrees: `kernel 03ccd937…`, `tools e14b6154…`, `Cargo.toml 6d564f75…`, `Cargo.lock 7769844e…`; `git diff 13eed2c5 42ccc9238` is `Plans/` only.
- `git diff --stat 13eed2c5 b1b49a31` is therefore `Plans/` plus the six index files (`kernel/Cargo.toml`, `cache.rs`, `cache_logic.rs`, `test_harness.rs`, `tools/neofs-cache-logic-tests/src/lib.rs`, `tools/xtask/src/main.rs`) — the index is the only code difference.

Both arms use the same `cargo xtask neo-test install_fresh_4k` boot-1 command, the same QEMU 8.2.2/TCG and pinned OVMF (PR #82 body and THINK §9). The 87 s `06993824` vs 92 s `b1b49a31` pair is the same tree plus the incremental checks, so the ~5 s check cost is measured cleanly; the 133 s baseline is the same tree before the index. The "same image" phrase is loose — a compile-time feature is necessarily two builds — but the property the timing needs ("same source tree and build configuration apart from the index") holds, and the prior relay 338 reached the same conclusion for the small-cache pair. The hosted timing is not part of this claim; the PR body says it will come from this PR's run.

One provenance caveat: the 92 s absolute is measured at `b1b49a31`, and the PR head `c30975a3` has since merged main's non-index commits (FPU/SMP/ACPI/etc. — `git diff --stat 42ccc9238 856d1820a` is 23 code files, not docs). So the number is not a `c30975a3` measurement; it is the index build on the same tree as the 133 s baseline. The comparison itself is clean, but if the table is cited as the head's timing, say "measured at `b1b49a31`, index code unchanged since" rather than "this PR".

## The build matches the THINK at `4b2628c0`

I compared the THINK's §4/§9 claims to the build at `c30975a3`:

- **Data structure:** one `Vec<(u64, usize)>` sorted by block, not a `BTreeMap` (`cache_logic.rs:55-58`); binary-search `find` (`:66-73`); `pushed` (`:79-93`); `removed` (`:102-119`); stable `rebuild`/dedup (`:121-133`); `matches` (`:135-139`). Matches §4. The THINK writes `removed(slot, block)`; the code is `removed(entries, slot, block)`, the extra post-removal slice being what lets the duplicate fallback re-find the next first slot. That is a superset of the THINK's signature, not a behavioural mismatch.
- **Four mutation sites / write-back no-op:** matches §4 (item 1 above).
- **Incremental checks:** matches §4/§9 (item 3 above).
- **Crosscheck and kill-switch:** the crosscheck scenarios run every lookup through both index and scan and `matches` after every change (`cache.rs:806-812,848-854`); the `-off` leg compiles every index use out (item 5 above). Matches §4/§9.
- **Host KATs:** 12 pass, including the double insert, the length check and the 64-seed model check (THINK §5/§9).
- **Timing:** the table's 133/87/92 s provenance matches §9 (item 6 above).

The textual mismatches I found are the `-off` wording caveat (item 5), the THINK §9 `phase3_neofs_cache_index_off` row still carrying the older 40.0 s figure with the "index still kept" note (the true-off pair is in the parenthetical, 40.4/38.4 s), and the `removed` signature note. The timing caveat (the 92 s arm is `b1b49a31`, not the PR head) and the `forget_range` rebuild-cost sentence are provenance/cost wording, not build mismatches. None changes a verdict.

## Verification performed

- `git fetch origin +pull/82/head:refs/pr/82`; PR #82 head `c30975a3`, base `856d1820a` (GitHub API); read the diff vs `856d1820a` (6 files, +503/−25) and the full `cache.rs`, `cache_logic.rs`, `test_harness.rs`, `kernel/Cargo.toml`, `tools/xtask/src/main.rs`, and the KAT at that head.
- `cargo test -p neofs-cache-logic-tests` at `c30975a3`: 12 passed, 0 failed.
- `cargo test -p xtask fxsave_kernel_defaults_are_the_defaults_without_xsave` at `c30975a3`: passed (the new `neofs-cache-index` default is in `FXSAVE_KERNEL_DEFAULTS`, `tools/xtask/src/main.rs:7097`).
- Built the off kernel at `c30975a3` (`cargo build -Zjson-target-spec --bin kernel --features neo-test,neo-test-neofs-cache-index-off`): success; no `BlockIndex`/`check_index` symbols and no "neofs cache index" strings; scenario name `phase3_neofs_cache_index_off`.
- Built the true previous layer at `c30975a3` (`--no-default-features` plus `kernel-diagnostic,accel-sim,inference-preemptible,user-copy-fixup,neo-test,neo-test-phase3-neofs-cache-evict`): success; no index symbols/strings; scenario name `phase3_neofs_cache_evict`. The two ELFs' `.text`/`.data`/`.bss` are within 64/32/16 bytes of each other (the differing cfg set and scenario-name constant); the index property is identical (absent in both). Relay 338's more careful comparison at the same head found the defined-text symbol sets identical.
- Read the THINK at `4b2628c0` and compared its §4/§9 to the build; checked the timing tree identity with `git diff --stat 13eed2c5 b1b49a31` and the subtree hashes above; read the PR #82 body at `c30975a3`.
- CI for `c30975a3` (run 42476674596): 9/10 jobs green at review time (`Host Tests`, `Build Checks`, product boot, RNG seed boot, fuzz, shards 1/3/4, nightly skip); QEMU shard 2 was still in progress. This is a review, not the merge gate; the PM's merge evidence is the batch run.
- RAG: no response at `http://127.0.0.1:8404/health`; used grep/read, not a blocker.
- Working tree: only this RETURN on `dexter/pr82-review`; the pre-existing untracked `pr59-review/` directory is untouched. The temporary worktree `/tmp/pr82-review` is outside the repo.

## Non-blocking notes

1. `test_harness.rs:1200-1201` says the `-off` leg runs "with every lookup by scan"; it now compiles the whole index out. Say "with the index compiled out" for consistency with `kernel/Cargo.toml:1722-1723`, `cache.rs:793-796` and `tools/xtask/src/main.rs:6039-6043`.
2. THINK §9's scenario table row for `phase3_neofs_cache_index_off` still lists the old 40.0 s ("index still kept") figure; the true-off pair is in the parenthetical. Name `b1b49a31` in the row, as the table does for the 37.2 s figure, so the stale number cannot be read as current.
3. The `-off` Cargo feature does not unset `neofs-cache-index`; the emitted code is the previous layer because every use is cfg-excluded. Prefer "no index code is active" to "no cfg is active" (same note as relay 338).
4. THINK §3's "Removal stays O(n)" is about the index's `removed`; the no-evict `forget_range` path changes the previous `retain` (O(n)) into `rebuild` (O(n log n)). The measured boot/verify are still better (92 s vs 133 s, 28 s vs 64 s), so this is a cost sentence to tighten, not a defect.
5. `BlockIndex::pushed`'s `false` path in release leaves `index.len() != blocks.len()`, but the lookup still returns the first slot and the cache's miss-before-push invariant makes the path unreachable; the model check covers duplicates and passes. The THINK's "release builds stay correct" is true for lookups, and the length invariant is debug-only by design.
6. The THINK's `removed(slot, block)` signature is `removed(entries, slot, block)` in the build (the extra slice supports the duplicate fallback); a one-line THINK update would keep the API description exact.
7. The `install_fresh_4k` table's 92 s arm is `b1b49a31`, not the PR head; `c30975a3` has since merged main's non-index code. Label it as measured at `b1b49a31` (index code unchanged), not as the head's own run.

## Recommendation to the PM

**PASS — merge #82 on this evidence.** The index is coherent at every mutation site, the one-entry invariant is enforced and KAT-covered, the debug check is incremental, the lock order is unchanged, the `-off` build is the previous layer, and the timing pair isolates the index. The seven notes are docs/wording only; none changes the build or the verdict. The design THINK is Claude-authored and my relay-319/326 reads were the cross-family check on it; this build is Claude-authored too, so this review is cross-family against the author, though same-family as my own design review (as the dispatch notes).
