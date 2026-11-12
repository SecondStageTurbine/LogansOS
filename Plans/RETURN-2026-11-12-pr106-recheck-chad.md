<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 399 — Chad re-checks #106 F1/F2

**Dispatch:** `Plans/DISPATCH-2026-11-12-relays-398-399-rechecks.md`, Section 399.
**Reviewed head:** `e7a50407f6a4215bf52606540635e86c5e33e528`, `claude/f0-neofs-dir-ops`.
**Change:** `b6ee94b45d0264a896b0bab0aecfd9d6565eefac`, followed by the main merge.
**RETURN branch:** `codex/pr106-recheck-chad`, based on `origin/main` (`457dfdff9`).
**Requested model / effort:** GPT-6.1-Sol / medium.
**Verdict: FAIL — F1's reported stale-listing schedule is fixed; F2 remains open within F0 itself.**
All source `file:line` references below are at the reviewed head. D1 needs no change under the PM ruling;
older write paths are OT-101 and are not grounds for this verdict.

## F1 — handle snapshot

- **PASS — consistent snapshot publication for the reported safety property.**
  `kernel/src/fs/neofs/inode/dir_ops.rs:607` holds the record lock while comparing generations,
  replacing the inode block (`:609`) and record (`:610`), and obtaining the returned generation
  (`:612`). The record cannot be replaced independently by another F0 reload during that section.
  `kernel/src/fs/mod.rs:695` reloads before publishing the handle generation; `:708` locks that
  generation and `:709` accepts only a newer one. These are two ordered publication stages, not
  an atomic three-field store: the handle generation can temporarily lag the record. That lag
  conservatively produces ESTALE, rather than licensing an older listing with a newer generation.
  Readdir consumes a record clone under its lock at `kernel/src/fs/neofs/inode.rs:2077`.
- **PASS — the two-mkdir shared-fd schedule now yields the current listing or ESTALE.**
  For `/neofs/d`, first `mkdir_at(fd, "a")` obtains `(B1,R1,G1)`, then `mkdir_at(fd, "b")`
  obtains `(B2,R2,G2)`, with G2 > G1. If T2 publishes first, T1's older record is rejected at
  `dir_ops.rs:608`; if T1 publishes first, T2 replaces it. The record lock prevents the old
  pause between block and record stores from admitting the other publisher. Likewise a delayed
  G1 store cannot overwrite G2 (`fs/mod.rs:709`). After both operations complete, the saved
  record and handle generation are G2. With another intervening mutation, validation rejects
  a mismatched generation at `fs/mod.rs:415` / `:417`; readdir invokes it at `:742`.
  This assesses the requested schedule, not arbitrary whole-directory iteration linearizability.
- **PASS — protocol: newest-first.** Enforcing comparisons are
  `kernel/src/fs/neofs/inode/dir_ops.rs:608` and `kernel/src/fs/mod.rs:709`;
  `kernel/src/fs/neofs/dir_ops_logic.rs:147` implements strict `new > held`.
- **PASS — a discriminator forces the relevant deferred publications.**
  `kernel/src/test_harness.rs:33849` captures the older reload as a closure, `:33850` performs
  the second mkdir, `:33852` resumes the old reload, `:33853` explicitly publishes the second
  generation last, and `:33854` requires `[a,b]`. The snapshot is captured immediately at
  `dir_ops.rs:623`, while its publication is deferred to `:626`. This is a deterministic
  hook replay of the publication order, not two concurrently running syscall threads; the
  formerly possible mid-block/record pause is now inside the record critical section.
  The host KAT at `tools/neofs-dir-ops-logic-tests/src/logic_tests.rs:183` separately checks
  the newest-first rule and contrasts unguarded last-store behavior. I ran the host KAT as part
  of the suite; I did not run the kernel scenario.

## F2 — namespace lock across device reads

- **FAIL — not all F0 device reads precede `entries`.** All four operations use `after_reads`:
  mkdir `kernel/src/fs/neofs/inode/dir_ops.rs:129`, unlink `:171`, rename `:237`, stat `:320`.
  The first eight attempts read without the guard (`:96`), then acquire it (`:97`) and compare
  the superblock generation (`:98`). However, after those eight mismatches (`:47`, `:93`),
  **`:104` acquires `entries` and `:105` invokes the very same read closure under it**.
  Stat's closure reads the child inode at `:328`; chain reads occur at
  `kernel/src/fs/neofs/inode.rs:1051` / `:1055`. A miss reaches the device at
  `kernel/src/fs/neofs/cache.rs:249` and `kernel/src/fs/neofs/inode.rs:2863`.
  This is new F0 code, independent of the older paths excluded by the PM ruling.
- **PASS — mismatched speculative results are discarded and revalidated, with the F2 caveat above.**
  `dir_ops.rs:98` returns a value or error only if the generation still matches; otherwise the
  loop drops the lock and retries, including when the speculative read returned an error.
  The post-loop path is serialized and propagates errors through `:105`'s `?`, but achieves
  progress by reinstating the forbidden held-lock read. Replace that fallback with a clean
  retry/error policy or another protocol that never reads the device under `entries`.
- **FAIL — no held-completion / contending-writer discriminator.**
  `kernel/src/test_harness.rs:33863` empties the cache, then `:33869` through `:33891` run five
  operations serially. `:33893` obtains read/held-read/fallback counters and `:33900` merely
  asserts no fallback occurred. There is no held device completion, competing writer, or
  forced generation mismatch in this leg. `dir_ops.rs:660` only counts reads; it does not delay
  a completion. Thus the uncontended test excludes the branch responsible for the remaining bug.
- **FAIL — CP-63 is only partly corrected.** The actual [#106 PR body](https://github.com/SecondStageTurbine/LogansOS/pull/106)
  now correctly calls `entries` a spinning `SwitchMutex` and explicitly mentions the eight-try
  fallback. It nevertheless labels the row “guards for F0's operations” and says “Under the
  lock it only edits and writes to the cache (no device reads).” Those claims contradict
  `dir_ops.rs:104` / `:105`. The spin implementation is
  `kernel/src/sync/switch_mutex.rs:180` / `:187`. The row must describe the actual guarantee;
  acknowledging the fallback does not satisfy the dispatch's no-device-read requirement.

**Concrete F2 input / schedule (source-level counterexample, not an executed kernel test):**

1. Mount a writable fixture with `/neofs/a/note`; T1 invokes `stat_at(a_fd, "note")`.
2. On each of T1's eight optimistic attempts, pause after its reads at `dir_ops.rs:96` and
   before its lock at `:97`. T2 alternates successful `mkdir_at(root_fd, "z")` and
   `unlink_at(root_fd, "z")`, for eight commits total. Each bumps the superblock generation
   (`inode.rs:1652`), so each T1 validation fails.
3. Pause T1 before fallback lock acquisition (`dir_ops.rs:104`). Drain and evict the clean
   cache using the existing test fixture's `debug_neofs_cold_cache()`
   (`kernel/src/fs/neofs/inode.rs:872`); finish that helper before resuming T1. This makes the
   next inode read a device read without changing the namespace. Ordinary clean-cache eviction
   can create the same miss; the helper makes the input deterministic.
4. Resume T1 into `:104` / `:105`; hold completion of its chain/child inode read. T1 now holds
   `entries` across `cache.rs:249`. Virtio's wait path is
   `kernel/src/drivers/block/virtio_blk.rs:777`, with the park path at `:902`.
5. T3 calls `mkdir_at(root_fd, "w")`. Its initial root clone needs `entries`
   (`dir_ops.rs:95`), so it spins at `switch_mutex.rs:187` until T1's read completes.

This disproves the unconditional F2 requirement using F0 operations alone. It establishes spinning
contention during a device wait, not a claim of an ABBA deadlock.

## Still true

- **PASS — one commit per successful mutation.** The sole mutation calls are
  `kernel/src/fs/neofs/inode/dir_ops.rs:148`, `:209`, `:306`; the common helper publishes once
  at `:475`, and `kernel/src/fs/neofs/inode.rs:1652` bumps the superblock generation once.
  `stat_at` (`dir_ops.rs:312`) publishes nothing, as the PM ruling requires.
- **PASS — rights checked before any device read by these operations.**
  `kernel/src/fs/mod.rs:606`, `:614`, `:631` / `:632`, `:652` all run `dir_op_check` before
  entering the filesystem. `:679` checks LOOKUP in the rights intersection and `:683` checks
  mutation WRITE, returning at `:685`. The earlier kind check (`:676`) only reads the saved
  record, not the device. Both rename handles are checked before invoking the filesystem.
- **PASS — kill-switch remains.** `kernel/Cargo.toml:1971` leaves `neofs-dir-ops` opt-in;
  syscall numbers are gated at `kernel/src/arch/x86_64/syscall.rs:125` through `:135`, arms
  at `:1254` through `:1264`, and handlers from `:7937`. OpenFile methods and the new publisher
  are gated (`fs/mod.rs:604`, `:706`); discriminator hooks require `neo-test-neofs-dir-ops`.
  This is a source/cfg re-check, not a newly run feature-off kernel equivalence test.

## Verification

Read the reviewed source in a detached worktree at the exact head, and read #106's current PR body
via the GitHub API (its head matched the reviewed SHA). No implementation changes or PR comments.
Known paths and exact symbols sufficed; semantic discovery was unnecessary.

Ran `cargo test -p neofs-dir-ops-logic-tests --target x86_64-unknown-linux-gnu` there:
**14 passed, 0 failed**, unit tests in **43.46 s**, including the new F1 KAT, crash cuts and falsifier.
These host tests do not exercise the kernel lock fallback or a delayed device completion.
No new QEMU execution or CI-run claim. RETURN changes documentation only.
