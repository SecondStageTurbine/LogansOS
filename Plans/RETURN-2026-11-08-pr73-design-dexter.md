<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 319: #73 design read (an index for the NeoFS block cache)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-315-321.md` §319.
**Reviewed:** PR #73 head `e3fdcf4270f0c4cc50ce43aeb85a4175f9ae4f4a` (`docs/THINK-2026-11-08-neofs-cache-index.md`), against `origin/main` `cf41929d76bf43be14609813da0484a0e17ebe08`. The code the THINK cites is byte-identical between the PR base `13eed2c54` and that main: `git diff --stat 13eed2c54 origin/main -- kernel/src/fs/neofs kernel/Cargo.toml tools/neofs-cache-logic-tests` is empty. Code references are to that main; `cache.rs`, `cache_logic.rs`, `commit_logic.rs` and `inode.rs` are the files under `kernel/src/fs/neofs/`.
**Overall: FAIL.** Option B is the right shape, but the proposed debug check re-quadraticises the write path in the exact debug-assertions build used for the timing discriminator, the lock-order statement omits the `drain` lock, and the index's one-entry-per-block invariant is neither stated nor enforced. The fixes are small; no code was changed.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| 1 | The index replaces the linear `find_cached_block` scan with a bounded lookup, keyed correctly (device, block), and stays coherent on insert, eviction, write-back and invalidation | **FAIL** (1c, 1d); 1a/1b pass | THINK:10-12, 42, 62-70; `cache_logic.rs:22-34`; `cache.rs:146,178,774-784,813-834,395-417,974-980`; `tools/neofs-cache-logic-tests/src/lib.rs:52-54` |
| 2 | Memory bound and the no-eviction default are addressed | **PASS**, one clarification | THINK:14-17, 46-47, 55-58; `cache.rs:38-52,813-816`; `mm/heap.rs:30-52` |
| 3 | Lock order against the cache's existing locks is stated; no lookup under a lock that eviction also takes in reverse | **FAIL** (statement); property passes | THINK:31-32,110; `cache.rs:158-162,427-437,517-529,543-553,571-575,644-652,813-816` |
| 4 | A kill-switch (the indexed loop from #69) and the discriminators (host KATs, `install_fresh_4k` timing) are named | **PASS** (named); 1d must be fixed first | THINK:46-47,62-70,77-94; `cache_logic.rs:25-34` |
| 5 | The prior-art survey (BSD/Linux buffer caches read for principle only; any Rust crate with licence and commit) holds | **PASS** | THINK:96-106; commits checked below |

## Item 1 — bounded lookup passes; key and mutation sites pass with a sentence; two fails

**1a, bounded lookup — PASS.** `BTreeMap<u64 block, usize slot>` is O(log n) (THINK:42), replacing the linear `find_block` (`cache_logic.rs:25-34`). The `Vec` is untouched, so the drain's first-minimum tie-break (`commit_logic.rs:68-80`) keeps its order.

**1b, key — PASS, with a sentence to add.** Block-only is correct because `CacheInner` is per `NeoFsBlockCache`, and `NeoFsBlockCache` owns exactly one `device` (`cache.rs:146,178`), constructed per mount (`inode.rs:197`). The THINK never states that scope; add it, so a later multi-device cache cannot reuse the index with an under-specified key. The dispatch's "(device, block)" is satisfied by the per-cache device, not by a composite key.

**1c, duplicate-key invariant — FAIL (narrow).** `find_block` is specified as the *first* match (`cache_logic.rs:22-23`), and the #69 KAT pins `[7,5,5] → Some(1)` (`tools/neofs-cache-logic-tests/src/lib.rs:52-54`). A `BTreeMap` keyed by `block` cannot hold two entries with the same key; `insert_at_end` would overwrite the first slot, and the indexed `find` would return the later slot. All current insert paths re-check under `inner` before pushing (`cache.rs:236`, `:364-369`, `:765-777`), so duplicates are impossible today — but the THINK neither states that invariant nor asserts it in `insert_at_end`. Concrete input: a second insertion of block `5`; the linear scan returns slot 1, the map returns the later slot, and the per-change `check` (one entry per cached block) fires only in debug. Fix: state "one entry per block, enforced by the check-before-push at every call site"; `debug_assert!(!index.contains_key(&block))` in `insert_at_end`; add the duplicate-insert case to the KAT (expect the assert, or keep first-match if duplicates are ever to be supported).

**1d, debug check cost — FAIL.** THINK:68-70 runs a full `check(&[T])` after *each* change. `check` is O(n) (or O(n log n)), so the write path is O(n²) again. The QEMU scenarios are dev builds with `debug-assertions=on`: `build_test_kernel` calls `build_member` (`tools/xtask/src/main.rs:7064`), which passes no opt-level (`:8869-8888`), so `cargo build` stays on the dev profile (`:8905-8912`; workspace `Cargo.toml:151-152` leaves debug assertions at Cargo's default on), and the kernel banner prints `debug-assertions=on` (`kernel/src/main.rs:711-716`). `install_fresh_4k` is the timing discriminator (THINK:89-94). Concrete input: the W5 write phase, ~35,840 blocks (THINK:16); a full check after each insert visits ~6.4e8 entries, each with a BTreeMap lookup, before the verify starts. Fix: at each mutation assert only the changed entry plus `index.len() == blocks.len()` (O(log n)); run the full `check` at scenario checkpoints, after `forget_range`/rebuild, and in the host KAT. Also define "change" as an index mutation, not a `touch`/hit — a hit does not change the index and must not trigger `check`, or the W5 verify itself becomes O(n) per read.

**Write-back — PASS, with a sentence to add.** `drain_pass` only clears `dirty` (`cache.rs:974-980`); it does not change `blocks`, so no index update is needed. Its lookup at `:974` uses the index and relies on the four update sites. The THINK's §4 four places are complete (`push_block` `:774-784`, `evict_locked` `:813-834`, `forget_range` both legs `:395-417`), but it should say explicitly that write-back is a no-op for the index.

**Implementation note.** `remove_at(slot)` as written (THINK:63) cannot identify the block from a block→slot map alone; the caller has it (`cache.rs:830` `let block = inner.blocks.remove(victim).block`, `:409-411`). Pass the block with the slot, or state the reverse scan. Also the host KAT crate mounts `cache_logic.rs` through `#[path]` (`tools/neofs-cache-logic-tests/src/lib.rs:9-10`) and has no `extern crate alloc`; if `BlockIndex` uses `alloc::collections::BTreeMap`, add it there and keep `rebuild`/`check` generic over `T: BlockKey`.

## Item 2 — PASS, one clarification

The cache's bound is stated: product at most 256 clean plus dirty (THINK:17; `cache.rs:48-52`), with dirty blocks unbounded between mutations (THINK:17; `cache.rs:38-45`). The no-evict default is stated and kept as a coverage leg (THINK:16, 55-58; `evict_locked` returns at `cache.rs:813-816`). One clarification: the index is one entry per cached block, so it is bounded by `blocks.len()`, not a new tier. At ~35,840 entries a `BTreeMap<u64, usize>` is roughly 0.6-1 MB (node capacity 11, order tens of bytes per entry), against the 140 MB cache and the installer kernel's 305 MiB boot-module heap (`mm/heap.rs:30-52`; `mm/heap_policy.rs:37`), so it does not change the tier. Say that in §2.3/§3; it is a sentence, not a design change.

## Item 3 — FAIL on the statement; the property is clean

The THINK only names `inner` (THINK:31-32, 110). The cache also has `drain: SwitchMutex<()>` (`cache.rs:158-162`), held across block I/O with IF=1. The order is `drain → inner`: `drain_all` (`:427-437`), `drain_worker_cycle` (`:517-529`), `debug_drain_to_barrier` (`:543-553`); `drain_pass` takes `inner` inside (`:903-...`, `:974`). No path holds `inner` then takes `drain`: `drain_worker_cycle` drops `inner` before `try_lock` (`:517-524`), and `pause_worker` returns a guard that holds no `inner` (`:571-575`, `:644-652`). `evict_locked` takes only `inner` and is never called under `drain` (callers `:303-311`, `:375-381`, `:796-807`). So no lookup is under a lock that eviction also takes in reverse; add the `drain → inner` sentence to §2.2/§7.

## Item 4 — PASS on naming; one ordering condition

The feature is `neofs-cache-index` (THINK:46-47); off is #69's indexed loop `find_block` (THINK:65; `cache_logic.rs:25-34`); on in product and the default test build after green. Host KATs are `tools/neofs-cache-logic-tests` (THINK:77-81) with the model check; scenarios are the existing NeoFS set on/off plus the new drain-order kill-switch scenario (THINK:83-87); timing is `install_fresh_4k` main/on/off plus hosted shard 4 (THINK:89-94). Two conditions: (i) fix 1d before the timing table means anything; (ii) the index is on for every scenario, and a `BTreeMap` lookup can be slower than a 4-10 entry linear scan, so add a small-cache timing or a size threshold — the `install_fresh_4k` table only covers the large case.

## Item 5 — PASS

Checked at 2026-11-08 against the cited commits (GitHub API plus raw `Cargo.toml` at those SHAs): hashbrown `f7bae3d165a3` 2026-10-31, `license = "MIT OR Apache-2.0"`; lru-rs `3ed2e28cfc53` 2026-10-18, `MIT`; slab `7d1d8ab00a7b` 2026-08-16, `MIT`; intrusive-rs `0a895642801c` 2026-08-04, `MIT OR Apache-2.0`; all non-archived with recent pushes. Linux `mm/filemap.c`/XArray is GPL-2.0, principle only; FreeBSD `sys/kern/vfs_bio.c` includes `sys/pctrie.h` (per-vnode trie), BSD-2-Clause, principle only. Nothing is copied, no new dependency; `alloc::BTreeMap` is already used (`kernel/src/ai/evict.rs:30`, `reservation.rs:52`).

Minor: the BTreeMap row (THINK:100) gives no SHA/date; cite the toolchain pin (`rust-toolchain.toml:10`, `nightly-2026-04-16`) and, for the survey rule's liveness column, the open-issue/release/archived evidence (all four crates are active). The four external rows are accurate as written.

## Verification performed

- Read the THINK at `e3fdcf42`; read the code at `origin/main` `cf41929d7`; confirmed the PR base `13eed2c54` and that main differ only under `Plans/` for the cited paths (`git diff --stat` empty).
- Checked the four Rust prior-art commits and licences via the GitHub API and raw `Cargo.toml` at those SHAs; checked FreeBSD `sys/pctrie.h` and the Linux XArray reference. No summarizing fetch was used for exact data.
- Did not build or run anything; this is a design read and there is no code gate. The `install_fresh_4k` numbers in the THINK are #69's/PM's, not re-run here.
- RAG: no response at `http://127.0.0.1:8404/health`; used grep/read, not a blocker.
- Working tree: only this RETURN on `dexter/pr73-design`; no code or THINK changes.

## Recommendation to the PM

Approve option B in shape, but do not ratify §4/§7 as written: require (a) the incremental debug check, (b) the `drain → inner` statement, and (c) the one-entry-per-block invariant before the build PR. The two §8 decisions are unaffected: B is still the only option that keeps the drain order exactly, and the installer-evict coverage question can keep one scenario in each mode. The fixes are small; none changes the option choice.
