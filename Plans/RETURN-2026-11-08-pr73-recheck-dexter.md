<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 326: #73 re-check (incremental check, `drain → inner`, one entry per block, small-cache timing)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-325-328.md` §326.
**Reviewed:** PR #73 head `89127f21` (`docs/THINK-2026-11-08-neofs-cache-index.md`), PR base `13eed2c54`, against `origin/main` `7072cf582`. `git diff 13eed2c54 origin/main -- kernel/src/fs/neofs` is empty, so the cache code the THINK cites is the same at the base and at current main; the wider cited set differs only in `kernel/Cargo.toml`, `kernel/src/test_harness.rs` and `tools/xtask/src/main.rs`. The enforcement code is the prototype/build PR `claude/neofs-cache-index` @ `2b3db438` (the THINK cites `b1b49a31`; these files are the same at both). Line references prefixed `proto` are that head; `kernel/Cargo.toml`, `tools/xtask` and `tools/neofs-cache-logic-tests` references are also `2b3db438`; the rest of `cache.rs` is `origin/main`.
**Overall: FAIL — narrow, item 5 only.** (a), (b), (c) and the sorted-Vec choice pass. The small-cache timing is not a same-image/same-build comparison, so its "no size threshold is needed" conclusion is not established as written.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| a | The debug check is incremental; the write path stays O(log n) per insert in the debug-assertions build | **PASS** | THINK:91-102; proto `cache.rs:817-836,861-868`; proto `cache_logic.rs:79-91` |
| b | The lock order is stated as `drain → inner`, and no path takes them the other way | **PASS** | THINK:32-41,142-143; `cache.rs:426-449,515-530,542-557,571-576,813-823,903-1085` |
| c | One entry per (device, block) is stated and enforced (debug assert, host KAT for a double insert) | **PASS** | THINK:44-52; proto `cache.rs:861-868`; proto `cache_logic.rs:79-91`; `tools/neofs-cache-logic-tests/src/lib.rs:170-189` |
| 4 | The sorted-Vec choice keeps 1a/1b: bounded lookup, keyed (device, block) | **PASS** (one cost-cell correction) | THINK:60,83-85,164-172; proto `cache_logic.rs:55-149` |
| 5 | The small-cache timing compares index off against eviction fairly (same image, same build) | **FAIL** | THINK:197-201; `tools/xtask/src/main.rs:5997-6036,7069-7103`; `kernel/Cargo.toml:81,1702`; proto `cache.rs:784-807,817-836,861-868,915-920` |

## (a) PASS — the check is per-change, not per-cache

THINK:91-102 defines a "change" as an index mutation (a push, a removal or a rebuild), says a hit or `touch` changes nothing and checks nothing, and states that the debug-assertions check is O(log n). The prototype matches:

- `check_index` (`proto cache.rs:817-836`) does `index.len() == blocks.len()` (O(1)) and, after a push, `index.find(block)` (O(log n)). The full `index.matches(&blocks)` comparison is behind `neofs-cache-index-crosscheck` (`:831-836`), so it is not in every debug kernel.
- `push_block` calls it once, after `index.pushed` (`proto cache.rs:861-868`); `pushed` is a binary search plus the insert (`proto cache_logic.rs:79-91`).
- It runs after each removal and rebuild with `None` (`proto cache.rs:415,431,918`), i.e. the length assertion only; the O(n log n) rebuild comparison stays on the crosscheck and host-KAT paths.
- The measured cost is consistent: THINK:203-204 reports about 5 s over `install_fresh_4k` boot 1, not the quadratic cost the old full check had.

The sorted `Vec`'s insert does move the tail (item 4), but that is the index's data-structure cost, not the debug check; for the installer's ascending order it is a near-append (THINK:170-171).

## (b) PASS — `drain → inner` is stated and held

THINK:32-41 states both locks and the order, and `origin/main` `7072cf582` agrees:

- `drain_all` (`cache.rs:426`): takes `drain` (`:431`), and `drain_pass` takes `inner` inside it (`:903-...`); after the pass it drops `drain` before its own `inner` (`:442`).
- `drain_worker_cycle` (`:515-530`): takes `inner` in an inner scope, drops it, then `drain.try_lock()` (`:517-523`). No reverse edge.
- `debug_drain_to_barrier` (`:542-557`): `pause_worker` (`inner` only, dropped), then `drain`, then `inner` under it.
- `pause_worker` (`:571-576`) takes only `inner`; its `WriteBackPauseGuard` holds no `inner` (`:172-176`).
- `evict_locked` (`:813-823`) takes only `inner` and is called from `write_bytes_at`/`insert_read_locked` (`:307,380`), never under `drain`.
- The index is touched only under `inner`, in the same critical sections as `blocks` (THINK:40-41).

No path in `cache.rs` takes `inner` and then `drain`; the THINK's three named exemptions (`drain_worker_cycle`, `pause_worker`, `evict_locked`) are the complete set.

## (c) PASS — one entry per block, enforced at the push and in the KAT

THINK:44-52 states the scope (one `NeoFsBlockCache` per device, `cache.rs:146,178`, made per mount `inode.rs:197`, so block is the key; a cache shared by several devices would need `(device, block)` and the THINK says not to reuse the index there), the invariant (every push follows a miss under the same `inner`), and the enforcement:

- `BlockIndex::pushed` returns false when the block is already indexed (`proto cache_logic.rs:79-91`).
- `push_block` `debug_assert!`s on that false (`proto cache.rs:861-868`), so a double push fails in every debug-assertions kernel.
- The host KAT `a_double_insert_is_reported_and_the_first_slot_kept` (`tools/neofs-cache-logic-tests/src/lib.rs:170-179`) checks the false return and the first-slot answer; `one_entry_per_block_keeps_the_lengths_equal` (`:181-189`) checks the length invariant across push, remove and retain.
- I ran the crate at `2b3db438`: **12 passed, 0 failed** (`cargo test -p neofs-cache-logic-tests`), including those two and the 64-seed model check.

The release-build fallback is stated and holds: `pushed` keeps the first slot, so `find` still matches the scan's first-match answer (THINK:51-52; `proto cache_logic.rs:88-91`).

## Item 4 — PASS: bounded lookup and the key survive the sorted-Vec change

THINK:60 and §9:164-172 replace the `BTreeMap` with `Vec<(block, slot)>` sorted by block. `find` is `binary_search_by_key` (`proto cache_logic.rs:71-73`), O(log n), and returns the slot of the first entry. The cache's `blocks` `Vec` and its order are untouched, so the drain's first-minimum tie-break (`commit_logic.rs:68-80`) keeps its order; `rebuild` is stable and dedups by first slot (`proto cache_logic.rs:121-132`). The key is still block, per one-device cache (item c).

One correction to the cost cell: THINK:60 says Insert O(log n). The binary search is O(log n), but `Vec::insert` moves the tail, O(n) worst case; it is near-append for the installer's ascending order (THINK:170-171). This does not break 1a/1b (lookup is bounded; the key is right), but the table should not claim O(log n) for the whole insert. A descending-block fill would be quadratic in insert; that is the trade for the heap behaviour in §9, and it is worth stating.

## Item 5 — FAIL: the two numbers are two different kernels, and the "off" one still maintains the index

THINK:197-201 says `phase3_neofs_cache_index_off` runs `phase3_neofs_cache_evict`'s legs "with every lookup by scan", on at most 256 clean blocks, and reports 40.0 s (scan) against 36.1 s (index), so "no size threshold is needed".

The workload and cache size are fair: both scenarios run the same evict legs (the `_off` feature depends on `neo-test-phase3-neofs-cache-evict`, `kernel/Cargo.toml:1702`) and both are bounded by `CACHE_CAPACITY_BLOCKS`. Both are built from the same prototype commit (`b1b49a31`), so the source tree matches. They are not the same image or build:

- They are two `TestScenario` entries (`tools/xtask/src/main.rs:5997-6036`); `build_test_kernel` builds each with `["neo-test", scenario.feature]` and default features on (`:7069-7103`), so each is a separate kernel.
- `neofs-cache-index` is in `default` (`kernel/Cargo.toml:81`). The `_off` feature (`:1702`) only selects the scan in `find_cached_block` (`proto cache.rs:784-807`); the update sites are still `#[cfg(feature = "neofs-cache-index")]` (`push_block` `proto cache.rs:861-868`, `evict_locked` `:915-920`, `forget_range` `:415,431`), and `check_index` still runs (`:817-836`).
- So the 40.0 s arm is "scan lookup + index maintenance + debug checks", not the index-off build. A true off build (`neofs-cache-index` absent) would be at most 40.0 s, and could be below 36.1 s if the index maintenance costs more than the 3.9 s lookup gap. The pair isolates the lookup (both arms pay maintenance), which is useful, but it does not establish "no size threshold": the true-off total is unmeasured.

Concrete input: build `phase3_neofs_cache_index_off` with `neofs-cache-index` disabled (the PR's clippy configuration: `--no-default-features` plus `kernel-diagnostic`, `accel-sim`, `inference-preemptible`, `user-copy-fixup`, plus `neo-test`/`neo-test-neofs-cache-index-off`), rerun the evict legs on the same machine as the 36.1 s run, and report that pair. If the intent is only to show the lookup delta, reword the claim to that and drop "no size threshold is needed". A same-image runtime switch would also satisfy the dispatch's "same image, same build".

## Minor notes (not blockers)

- THINK:4-5 and :161 still call `06993824` "the prototype"; §9 also cites `b1b49a31`, and the build PR is `2b3db438`. Update the header to the current prototype SHA so the citation is not stale.
- THINK:60 Insert cost: see item 4.
- THINK:185-186 calls `phase3_neofs_cache_index_off` "the kill-switch leg"; as built it is the scan-lookup leg with the index still maintained. The true feature-off build is the one in item 5.
- The PR #73 body still recommends the `BTreeMap` and does not mention §9's sorted `Vec`; refresh it so the PR does not describe a discarded data structure.

## Verification performed

- Read the THINK at `89127f21`; read `cache.rs`/`cache_logic.rs` at `origin/main` `7072cf582` for the lock order and at the prototype `2b3db438` for the enforcement. `git diff 13eed2c54 origin/main -- kernel/src/fs/neofs` is empty.
- Ran `cargo test -p neofs-cache-logic-tests` in a temporary worktree at `2b3db438`: 12 passed, 0 failed.
- Did not build a kernel or run QEMU/`install_fresh_4k`; the timing numbers are the author's, and item 5 is about the comparison's provenance, which the source answers.
- RAG: no response at `http://127.0.0.1:8404/health`; used grep/read, not a blocker.
- Working tree: only this RETURN on `dexter/pr73-recheck`; a pre-existing untracked `pr59-review/` directory is untouched and not committed.

## Recommendation to the PM

Approve (a), (b), (c) and the sorted-Vec choice — the three relay-319 fixes are in the THINK and in the prototype, and the host KATs pass. For item 5, either require the true index-off run or accept the 40.0/36.1 pair as a lookup-only delta and remove "no size threshold is needed"; option B itself is unaffected.
