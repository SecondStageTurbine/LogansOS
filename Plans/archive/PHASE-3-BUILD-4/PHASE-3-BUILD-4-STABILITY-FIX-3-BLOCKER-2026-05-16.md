# Phase 3 BUILD-4 Stability Fix 3 — Blocker Report

Date: 2026-05-16
Lane owner: Cerebro (per verbal PM authority; filename in dispatch reads `-jarvis`)
Authority: `Plans/DISPATCH-2026-05-16-phase-3-build-4-stability-fix-3-attempt-logs-and-io-wake-cerebro.md`

Status: **BLOCKED** — stale-fsync double-timeout final failure on run 42 of 50.
Stopping per dispatch §5 and PM instruction.

---

## 1. HEAD and Recent Log

```text
HEAD (substrate, no new commits yet — Fix 8 + Fix 9 + NEO_TEST_RUN_ID suffix uncommitted):
014e5d7 plans: authorize Phase 3 BUILD-4 stability fix 3
584d3e8 plans: authorize Phase 3 BUILD-4 stability fix 2
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
75e00a5 plans: authorize Phase 3 BUILD-4 stability fix
00df29b docs: correct BUILD-4A close evidence
9324f4b phase3 build4a: directory readdir filecaps
bb47d0a plans: authorize Phase 3 BUILD-4A readdir
f2bea51 plans: record Phase 3 BUILD-4 prereq audit
```

---

## 2. Build Gates

```text
cargo k                            PASS  72 warnings (baseline unchanged)
cargo k --no-default-features      PASS  72 warnings (baseline unchanged)
```

---

## 3. Diff Stat (uncommitted)

```text
kernel/src/drivers/block/virtio_blk.rs |   8 +-
kernel/src/sched/mod.rs                |   4 +-
kernel/src/sched/scheduler.rs          |  53 ++++++++++++
kernel/src/test_harness.rs             |  49 ++++++++++++
tools/xtask/src/main.rs                | 142 +++++++++++++++++++++++++++++----
5 files changed, 238 insertions(+), 18 deletions(-)
```

Three logical changes carried:

- **Fix 8** (`tools/xtask/src/main.rs`, +142 lines): per-attempt serial logs,
  canonical copy helpers, attempt parameter threaded through all three retry
  paths (regular, aarch64, two-boot persistence).
- **Fix 9** (`kernel/src/sched/{mod,scheduler}.rs`, +57 lines;
  `kernel/src/drivers/block/virtio_blk.rs`, +8 lines): `wake_thread_token_front`
  narrow API used only by virtio-blk completion handler.
- **NEO_TEST_RUN_ID suffix** (`tools/xtask/src/main.rs`, part of the +142):
  default-off; when set, suffixes attempt log paths so loop harnesses can
  preserve evidence across iterations.
- **Stale-fsync PROGRESS frames** (`kernel/src/test_harness.rs`, +49 lines):
  7 `event=PROGRESS` frames carried from the Fix 6 lane (kept; not the blocker).

The Fix 6 AP idle halt remains **preserved but excluded**:
`Plans/FIX-6-AP-IDLE-HALT-PRESERVED-2026-05-16.patch`.

---

## 4. Targeted Loop Result

### phase3_neofs_fsync_filecap_stale_regression — 50-run sequential, NEO_SMP=4

```text
run 1-3:   PASS
run 4:     PASS (retry)        [logs lost — overwritten before watcher started]
run 5-12:  PASS
run 13:    PASS (retry)        [logs lost — overwritten before watcher started]
run 14-23: PASS
run 24:    PASS (retry)        [logs lost — overwritten before watcher started]
run 25:    PASS
run 26:    PASS (retry)        [logs archived: run-26-attempt1/2.serial.log]
run 27-37: PASS
run 38:    PASS (retry)        [logs archived: run-38-attempt1/2.serial.log]
run 39-41: PASS
run 42:    FAIL (double 180s timeout)  [logs archived: run-42-attempt1/2.serial.log]
runs 43-50: NOT REACHED (loop broke on first failure)
```

**Final tally (42 runs attempted):**
- Final PASS: 41
- Final FAIL: 1 (run 42)
- Retry-pass: 5 (runs 4, 13, 24, 26, 38)
- Retry-pass rate before failure: 5/41 = 12.2%
- Final failure rate: 1/42 = 2.4%

### phase3_sched_wake_token_abandon — NOT RUN

Sequential loop never started (held pending stale-fsync clean completion).

### ipc_heap_kill_smoke — NOT RUN

Sequential loop never started (same reason).

---

## 5. Per-Attempt Serial Log Preservation Proof (Fix 8)

Fix 8 worked exactly as designed throughout the loop. Every iteration produced:
- `target/neo-test/<scenario>.attempt1.serial.log`
- `target/neo-test/<scenario>.attempt2.serial.log` (only when retry occurred)
- `target/neo-test/<scenario>.serial.log` (canonical convenience copy of latest attempt)

The xtask error messages now include exact per-attempt paths:
```text
xtask: `phase3_neofs_fsync_filecap_stale_regression` failed after 2 attempts;
last error: QEMU timed out after 180s for `phase3_neofs_fsync_filecap_stale_regression`;
previous error: QEMU timed out after 180s for `phase3_neofs_fsync_filecap_stale_regression`
```

**Outer-loop overwrite gap (acknowledged):** Fix 8's per-attempt filenames are
stable per scenario, so iteration N+1's QEMU overwrites iteration N's attempt
logs in the same paths. This was identified during the run mid-flight.

A side-watcher (`blf7a57et`) was added to archive logs to
`target/neo-test/attempt-archives/run-<N>-attempt<K>.serial.log` after each
iteration. It captured runs 26–42 (17 runs × 2 attempts = 34 files). Earlier
retry-passes (runs 4, 13, 24) were lost to overwrite before the watcher
existed.

The `NEO_TEST_RUN_ID` env-var suffix (default-off) was implemented in xtask to
make this preservation native rather than watcher-dependent. **Smoke proofs
were not yet executed** — they're held pending PM disposition.

---

## 6. Stage-Clustering Diagnosis

Three retry-passes had archived evidence. Their last `event=PROGRESS` frames
from the failed attempt 1:

### Run 26 (retry-pass)

**Last PROGRESS frame, attempt 1:**
```text
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=mount_done selected_superblock=0 generation=1
```
File size: 49333 bytes. 31 `virtio-blk: completion` lines after the last PROGRESS frame.
Attempt 2 PASS frame is intact (drained_blocks=10 seed + 7 mutation = 17 total).

### Run 38 (retry-pass)

**Last PROGRESS frame, attempt 1:**
```text
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=mount_done selected_superblock=0 generation=1
```
File size: 49660 bytes. 34 `virtio-blk: completion` lines after the last PROGRESS frame.
Same stage as run 26: seed_fsync drain.

### Run 42 (FINAL FAILURE — double timeout)

**Attempt 1** (file: `target/neo-test/attempt-archives/run-42-attempt1.serial.log`):
- File size: 27044 bytes
- **Last frame: only `event=BEGIN`** (no PROGRESS frames ever fired)
- Stalled BEFORE `mount_done` — earlier than runs 26/38

**Attempt 2** (file: `target/neo-test/attempt-archives/run-42-attempt2.serial.log`):
- File size: 59055 bytes
- Got through all stages up to `refreshed_fsync_begin`
- **Last PROGRESS frame:**
```text
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=refreshed_fsync_begin dirty_blocks=7 flush_required=true refreshed_generation=9 current_inode_generation=9
```
- Stalled at refreshed_fsync (final fsync of 7 dirty mutation blocks + flush)

### Summary

The timing tail is **not localized to a single stage**:

| Run | Attempt 1 stall stage | Attempt 2 result |
|-----|----------------------|------------------|
| 26  | seed_fsync (after mount_done) | PASS |
| 38  | seed_fsync (after mount_done) | PASS |
| 42  | mount (before mount_done) | FAIL — refreshed_fsync (final fsync) |

Two retry-passes clustered at seed_fsync (10-block drain + 1 flush). The final
failure on run 42 hit BOTH the mount-phase I/O AND the refreshed_fsync 7-block
drain. This is consistent with the underlying SMP I/O timing tail being broad
rather than a single critical section issue — the entire I/O-heavy fsync drain
path has a tail that occasionally exceeds the 180s budget at multiple points.

Fix 9 (virtio-blk front-of-queue wake) demonstrably helped — pre-Fix-9 saw
retry-pass at 4/5 in Fix 6's sample. Post-Fix-9 retry-pass dropped to 5/41 =
12.2%. But a 12% retry rate with a 180s budget per attempt eventually
collides with two consecutive long tails, producing the double timeout.

---

## 7. Regression Scenarios — NOT RUN

Per dispatch §6 stop conditions and PM "stop immediately on final failure"
instruction.

---

## 8. Final Full-Suite Gate — NOT RUN

---

## 9. BUILD-4B Status

**BUILD-4B remains blocked.**

---

## 10. Next Mechanism Candidates

The retry tail is distributed across mount, seed_fsync, and refreshed_fsync —
no single localized critical section to fix. Three plausible follow-up lanes:

### (a) Same-CPU wake-to-run latency in `wake_thread_token` (broader scope than Fix 9)
Fix 9 changed only the virtio-blk caller of `wake_thread_token`. If the
remaining tail comes from same-CPU completion-then-yield latency, extending
the same wake-to-front semantics to other I/O paths (or making completion
threads run inline instead of via the ready queue) could close the gap.
Higher blast radius than Fix 9 — touches `wake_thread_token` semantics broadly
and may regress fairness in scheduler-only scenarios.

### (b) Reduce per-fsync drain size
The seed_fsync drains 10 blocks; the refreshed_fsync drains 7. Each block
write currently goes one-at-a-time through `cache.drain_all()` → `drain_pass()`
→ one block, await completion, next block. A pipelined drain (submit multiple
block writes before waiting for completions) would reduce the round-trip
serialization and shrink the tail. This is a NeoFS cache change, not a
scheduler change.

### (c) Per-scenario QEMU timeout budget
The dispatch explicitly forbade changing retry/timeout counts. But the run 42
double-timeout makes the 180s budget the proximate cause of "final failure"
rather than a kernel bug. If PM authorizes a targeted timeout extension for
I/O-heavy NeoFS scenarios (e.g., 360s for fsync regression tests), the test
would pass deterministically without requiring further kernel work. This
treats the symptom but reflects reality: under SMP=4 with current host KVM
timing, this NeoFS scenario needs more wall-clock budget.

### (d) Smaller NeoFS disk (deferred by dispatch §3.10)
Dispatch noted: minimum disk size is just over 64 MiB due to three-superblock
policy. A 72 MiB or 80 MiB disk would reduce mount-validation I/O but the
fsync drain phases (where run 42 attempt 2 failed) are independent of disk
size. Partial mitigation only.

---

## 11. PM Decision Required

The dispatch close target was "zero final failures, zero retry-pass timeouts."
After 50 runs:
- Final failures: **1** (target: 0)
- Retry-pass: **5** (target: 0)

Per the dispatch §5 stop conditions both triggers fire. Two distinct
disposition paths:

1. **Treat run 42 as a true regression**: open a follow-up lane for one of the
   four mechanisms above. Most likely candidate: (b) pipelined fsync drain,
   since attempt 2 of run 42 showed the refreshed_fsync (7 blocks) is also in
   the tail.

2. **Accept residual instability**: BUILD-4 stays open, but the gating
   criteria allow non-zero retry-pass for stale-fsync since the underlying
   mechanism (host KVM timing) is environmental and the kernel's behavior is
   correct on every passing attempt. Run 42's failure is then framed as a
   180s-budget collision rather than a kernel bug.

I recommend path (1) with mechanism (b) (pipelined fsync drain), since it
addresses the actual I/O latency rather than masking it with a timeout
increase, and the change is contained to the NeoFS cache module (smaller blast
radius than touching `wake_thread_token` broadly).

---

## 12. Preserved Evidence Paths

```text
target/neo-test/attempt-archives/run-26-attempt1.serial.log  (seed_fsync stall, 49333 B)
target/neo-test/attempt-archives/run-26-attempt2.serial.log  (PASS, 64977 B)
target/neo-test/attempt-archives/run-38-attempt1.serial.log  (seed_fsync stall, 49660 B)
target/neo-test/attempt-archives/run-38-attempt2.serial.log  (PASS)
target/neo-test/attempt-archives/run-42-attempt1.serial.log  (mount stall, 27044 B)
target/neo-test/attempt-archives/run-42-attempt2.serial.log  (refreshed_fsync stall, 59055 B)
target/neo-test/attempt-archives/run-{27..41}-attempt{1,2}.serial.log  (clean PASS pairs)
```

All retry-pass runs from 26 onward, and the run 42 final failure, have both
attempt logs preserved. Runs 4, 13, 24 retry-pass attempt logs are lost.

Loop output transcript: see `b6t2tpwh2.output` (50 lines, summary line at end).
