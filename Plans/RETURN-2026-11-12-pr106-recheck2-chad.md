<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 403 — Chad re-checks #106 F2

**Dispatch:** `Plans/DISPATCH-2026-11-12-relay-403-pr106-recheck2.md`.
**Reviewed head:** `09414118fcf9df07cde314998dda65ed69a01372` (`8149e858` plus main merge).
**Requested reviewer:** GPT-6-Astra, medium effort.
**RETURN branch:** `codex/pr106-recheck2-chad`, based on `origin/main`.
**Verdict: FAIL.** The eight-attempt fallback is fixed, but a successful F0 mutation can still read the device while holding `entries`: publishing a 512-byte superblock uses the cache's partial-write merge, which reads its missing 4-KiB block. The discriminator also lacks a held-completion/concurrent-writer leg. F1, rights, one-commit structure and the kill-switch remain intact.

All source references below are at the reviewed head. This is an F0-reachable call-chain finding, not a request to fix unrelated older write paths or OT-101 dirty throttling.

## Dispatch checklist — F2

### 1. PASS — bounded attempts return Contended / EAGAIN

`kernel/src/fs/neofs/inode/dir_ops.rs:50` sets eight attempts. The root clone's temporary lock ends at `:100`; the read closure runs at `:101`, before acquiring `DirOpLock` at `:106`. Only a matching generation returns its result (`:107`); exhaustion returns `NeoFsError::Contended` at `:113`. There is no post-loop read closure or locked fallback. The error maps to `VfsError::Again` at `dir_ops.rs:735`, then `EAGAIN` at `kernel/src/arch/x86_64/syscall.rs:8258`.

### 2. FAIL — a commit callee still reads under entries

The optimistic read closures themselves pass:

- mkdir, `dir_ops.rs:136`: `handle_chain` loads the directory chain; name/capacity checks are in memory.
- unlink, `:178`: chain, child inode (`:187`) and regular-file indirect block collection (`:193`) all precede the guard; empty-directory collection (`:200`) only enumerates existing direct pointers.
- rename, `:244`: both chains and the optional recursive subtree walk (`:273`, `:426`) precede the guard; the remaining plan operations manipulate vectors and names.
- stat, `:327`: chain and child inode (`:335`) precede the guard; after validation it only drops the guard and returns (`:345`).

The post-guard audit finds the remaining read:

1. mkdir writes its empty child at `dir_ops.rs:149`, then calls `commit_dir_tree` at `:155`. Unlink and rename call the same helper at `:216` and `:313`. The guard remains live throughout.
2. `commit_dir_tree` uses in-memory pointer edits (`:458`, `:467`, `:473`), direct block enumeration (`:449`; implementation `kernel/src/fs/neofs/inode.rs:2934`) and directory-version writes (`:461`, `:487`). Empty versions write a complete block (`dir_ops.rs:406`); nonempty versions use `inode.rs:1688` and `:1710`. Allocation uses metadata in memory (`inode.rs:1500`, `:1505`), and complete-block cache writes at `kernel/src/fs/neofs/cache.rs:312` do not read media.
3. However, root publication at `dir_ops.rs:482` enters `inode.rs:1642`, then `publish_superblock` at `:1655`. Its free-map writes are complete blocks (`:3511`), but its superblock writes (`:1768` / `:1785`) enter `write_superblock_copies_cached` at `:3548`.
4. That helper calls **`cache.write_bytes_at(offset, &bytes, Superblock)` at `inode.rs:3564`**. `kernel/src/fs/neofs/format.rs:13` makes the payload **512 bytes**, whereas `:11` makes a cache block **4096 bytes**. The copies start at offsets 0, 1 MiB and 64 MiB (`:26`).
5. On a cache miss, **`cache.rs:378` reads the entire existing block from the device before merging those 512 bytes**. The miss branch is `:364` through `:379`; it is not a memory-only cache write. `entries` is still held by the F0 mutation.

Reloads after a successful operation are outside the operation's guard: `kernel/src/fs/mod.rs:700` calls `dir_ops.rs:595`, whose `directory_now` uses `root_snapshot` or `resolve_path`; `inode.rs:319` clones entries and drops that temporary guard before the path's reads at `:326` / `:333`. They add no held-entries read.

Dirty throttling (`inode.rs:985`) drains under entries before optimistic work. Its drain (`cache.rs:1024`) writes complete filesystem blocks and flushes; that inherited write/flush behavior is OT-101, not the new failure cited here.

**Concrete input:** mount a valid writable fixture with available space and a root handle with LOOKUP|WRITE; ensure root name `x` is absent. Run the existing `debug_neofs_cold_cache()` to completion, then `mkdir_at(root_fd, "x")`. The helper drains and evicts **every** cache block, including superblocks (`inode.rs:872` through `:874`; `cache.rs:416`). Root `handle_chain` needs only the in-memory root snapshot and root entries (`inode.rs:1034` through `:1041`), so it does not warm block 0. The new directory/root/free-map allocations do not allocate the reserved superblock block 0. The first superblock copy at offset 0 consequently misses at `cache.rs:364` and reads at `:378` while the mkdir owns entries. No generation mismatch or eight-attempt exhaustion is needed. Ordinary clean-cache eviction can also remove superblock blocks: `cache.rs:935` selects clean victims without a superblock exemption.

### 3. PASS — the EAGAIN refusal makes no operation mutation or handle publication

All mutation edits and reservations are after the `after_reads(...)?` boundaries (`dir_ops.rs:144`, `:207`, `:287`). Eight mismatches therefore reach none of the mutation commit calls. Stat has no mutation. `kernel/src/fs/mod.rs:699` propagates the operation error before reload/publication, including both rename handles (`:642`, `:646`). Speculative reads can populate clean cache entries (`cache.rs:291`), and preflight throttling can drain earlier work; “written nothing” means no partial dirty edit, namespace commit or handle publication by the refused operation, not absence of read-cache fills or other writers' commits.

### 4. FAIL — the general held-read / third-writer schedule remains possible; old fallback branch is fixed

The previous schedule's exact post-eight-mismatch fallback is gone: after eight successful interleaved commits invalidate T1's observations, `dir_ops.rs:113` returns EAGAIN, with no ninth read. Cold-evicting immediately before that return cannot resurrect the removed fallback.

But the required no-held-lock-read property still fails during the interleaved commits themselves. Concrete source-level replay:

1. T1 starts `stat_at(a_fd, "note")` in a valid `/neofs/a/note` fixture. Pause it after each attempt's reads (`dir_ops.rs:101`) and before validation (`:106`).
2. T2 alternates root mkdir/unlink of `z`, producing eight commits, one per attempt. Complete the first seven in the usual way.
3. Before the eighth T2 mutation, drain and cold-evict the cache, with the helper finished before T2 begins. The eighth operation is root `unlink_at(root_fd, "z")`; its child read cannot warm reserved superblock block 0.
4. Pause completion of T2's superblock partial-merge read at `cache.rs:378`. T2 holds entries through `dir_ops.rs:216` / `:482` / `inode.rs:3564`.
5. T3 attempts `mkdir_at(root_fd, "w")`. Its root clone at `dir_ops.rs:100` spins acquiring entries (`kernel/src/sync/switch_mutex.rs:180`, `:187`) until T2's read completes.
6. Release the read; T2 completes commit eight. Resume T1: its eighth validation mismatches and it returns EAGAIN.

Thus T1's refusal is clean, but F0 still admits a held device read and a spinning third writer. This is source replay, not an executed kernel schedule. QEMU is unavailable on this review host; no device-completion runtime result is claimed.

### 5. FAIL — discriminator checks mismatches/EAGAIN but not held-completion concurrency

The new hook genuinely nests successful commits: `dir_ops.rs:692` / `:694` run root mkdir/unlink, recursion is suppressed at `:685`, and `:697` counts only successful results. The harness arms eight at `kernel/src/test_harness.rs:33917`, expects EAGAIN at `:33920`, checks eight commits (`:33925`), one refusal (`:33931`), positive reads (`:33934`), zero held reads (`:33937`), absence of the refused name (`:33943`) and a successful retry (`:33949`). Those are substantive assertions, not a forced-generation-only mock.

However, the hook runs **after** `read` returns (`dir_ops.rs:101`, `:105`). There is no held completion, simultaneous third writer or assertion that a writer finishes before a held read is released. `note_device_read` (`:704`) counts, but never delays a completion. The harness comment at `test_harness.rs:33913` only describes potential spinning at a nested call between reads. It cannot establish the requested concurrent-writer property during a read.

**Concrete missing discriminator input:** the T2-held-superblock-read/T3-root-mkdir schedule above. The supplied hook cannot represent that pause. Moreover, its existing cold-mutation tests should already detect the substantive bug: cold mkdir at `test_harness.rs:33877` / `:33878` reaches the partial-write read and increments `HELD_READS` via `cache.rs:376`; `test_harness.rs:33902` then rejects the result. This is a source prediction, not a claimed QEMU run. The global ENTRIES_HELD boolean is sufficient for these serial nested legs, but is not per-owner attribution for arbitrary concurrent execution.

### 6. FAIL — actual CP-63 body still overstates the guarantee

Read the actual [PR #106 body](https://github.com/SecondStageTurbine/LogansOS/pull/106), with API head matching the reviewed SHA. Its CP-63 row now correctly says optimistic reads are outside entries and exhaustion after eight attempts returns EAGAIN. It still says: “Under the lock there are only edits and cache writes, no device reads.” The cold-root-mkdir input above contradicts this through `inode.rs:3564` / `cache.rs:378`. Calling a cache write is not sufficient evidence when that API implements a read-modify-write miss. The row's nested-writer discriminator wording also does not establish progress during a held completion, as item 5 explains.

### 7. PASS — ABI and libneo record EAGAIN

`docs/SDK-ABI.md:188` records it for row 26, rows 27/28 inherit it at `:189` / `:190`, and row 29 explicitly lists it at `:191`. `userland/libneo/src/lib.rs:1335` documents contention/retry for mkdir; unlink and rename inherit those errors in their wrapper documentation, and stat references the shared errors. This accurately describes the bounded-refusal path.

## Still true

- **PASS — F1 newest-first publication unchanged.** `dir_ops.rs:614` holds the record lock while checking `newer_generation` (`:615`), replacing block and record (`:616`, `:617`), and obtaining the held generation (`:619`). The handle publishes only a newer generation at `kernel/src/fs/mod.rs:713` through `:715`. The F1 deferred-publication discriminator remains at `test_harness.rs:33854` through `:33859`; its shared host KAT passed.
- **PASS — rights before device reads.** `kernel/src/fs/mod.rs:609`, `:617`, `:634` / `:635`, and `:657` call `dir_op_check` before entering the inode operation. The intersection must contain LOOKUP and mutation handles must contain WRITE (`:684` through `:690`). The preceding kind check uses the saved record. Both rename handles pass before any operation read.
- **PASS — one commit per successful mutation.** Mutation paths converge on the single `commit_dir_tree` call at `dir_ops.rs:155`, `:216`, `:313`; it publishes the root once at `:482` then returns at `:484`. `inode.rs:1652` bumps the superblock generation once. Stat commits nothing. The new finding is a read within that commit, not an extra commit.
- **PASS — kill-switch remains.** `kernel/Cargo.toml:1977` defines opt-in `neofs-dir-ops`; test feature `:1980` implies it. Syscall numbers (`syscall.rs:125` through `:135`), dispatch arms (`:1254` through `:1264`), and new errno mapping (`:8257`) are gated. `NeoFsError::Contended` is gated at `kernel/src/fs/neofs/mod.rs:78`; OpenFile methods and publication are gated in `fs/mod.rs:607`, `:615`, `:625`, `:651`, `:711`. This is source/cfg verification, not a feature-off kernel boot.

## Verification and limits

Read the dispatch, prior RETURN, applicable AGENTS instructions, exact fix and source in detached worktree `/tmp/pr106-astra-09414118`. Known paths and exact-symbol lookups sufficed; no semantic discovery was needed. The GitHub CLI is absent, so the parent fetched the public REST PR JSON to `/tmp/pr106-recheck2-pr.json`; its live head equals the reviewed SHA. No PR edits or implementation changes.

Executed `cargo test -p neofs-dir-ops-logic-tests --target x86_64-unknown-linux-gnu` at the exact head: **14 passed, 0 failed**, unit tests **43.41 s**. This includes crash cuts, no-barrier falsifier and F1 generation KAT; it does not execute the kernel block-cache partial-write path.

The parent also compiled and ran `/tmp/pr106-cache-harness.rs`, generated by `/tmp/pr106-cache-harness-gen.py`, with the exact reviewed `write_bytes_at` method (only `pub(super)` visibility removed) and stubbed device/cache plumbing. A cold 512-byte write at offset 0 performs one 4096-byte device read; a warm repeat performs none, and trailing bytes are preserved. This corroborates the callee behavior; it is not a kernel lock or driver test.

No QEMU/device-held execution or new CI result is claimed. The source counterexample and callee harness establish the remaining F2 read; kernel runtime confirmation and a real held-completion discriminator remain required.
