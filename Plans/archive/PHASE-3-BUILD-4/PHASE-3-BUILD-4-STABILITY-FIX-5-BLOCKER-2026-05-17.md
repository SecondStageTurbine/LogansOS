# Phase 3 BUILD-4 Stability Fix 5 — Blocker Report

Date: 2026-05-17
Lane owner: Cerebro
Authority: `Plans/DISPATCH-2026-05-17-phase-3-build-4-stability-fix-5-targeted-timeout-cerebro.md`

Status: **BLOCKED** — stale-fsync final double-timeout at run 18 of 50 under
the new 360s per-attempt policy. Per dispatch §4 and the PM guardrail
("Any final double-timeout under the extended budget is still a blocker"),
stopping the lane and deferring to PM.

---

## 1. Framing (locked-in language)

> The targeted timeout policy does not claim to fix the storage latency tail.
> It prevents a known environmental tail in I/O-heavy NeoFS scenarios from
> being misclassified as a kernel semantic failure, while preserving
> per-attempt logs and retry evidence for diagnosis.
>
> Guardrail: any final double-timeout under the extended budget is still a
> blocker.

Run 18 triggered the guardrail.

---

## 2. HEAD and Recent Log

```text
HEAD (substrate; Fix 8/9/11/12/14 + PROGRESS frames uncommitted):
8f1a4b2 plans: authorize Phase 3 BUILD-4 targeted timeout policy
2377d4d plans: authorize Phase 3 BUILD-4 storage backend matrix
dbe59a3 plans: authorize Phase 3 BUILD-4 stability fix 4
014e5d7 plans: authorize Phase 3 BUILD-4 stability fix 3
584d3e8 plans: authorize Phase 3 BUILD-4 stability fix 2
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
75e00a5 plans: authorize Phase 3 BUILD-4 stability fix
00df29b docs: correct BUILD-4A close evidence
```

---

## 3. Build Gates

```text
cargo build -p xtask                PASS
cargo k                             PASS  72 warnings (baseline unchanged)
cargo k --no-default-features       PASS  72 warnings (baseline unchanged)
```

---

## 4. Diff Stat (uncommitted)

```text
kernel/src/drivers/block/virtio_blk.rs  |   8 +-
kernel/src/sched/mod.rs                 |   4 +-
kernel/src/sched/scheduler.rs           |  53 ++++++
kernel/src/test_harness.rs              |  49 ++++++
tools/xtask/src/main.rs                 | 290 +++++++++++++++++++++++++++++++---
5 files changed, 378 insertions(+), 26 deletions(-)
```

Carried fixes (still uncommitted, awaiting PM authorization):
- Fix 8: per-attempt serial logs;
- Fix 9: narrow virtio-only `wake_thread_token_front`;
- Fix 11: `NEO_QEMU_STORAGE_MODE` env gate (default unchanged: writeback);
- Fix 12: `NEO_TEST_RUN_ID` env-var suffix (proven across this experiment);
- Fix 14 (new this lane): per-scenario timeout policy + invalid-override
  rejection + 15-scenario I/O-heavy allowlist at 360s;
- stale-fsync PROGRESS frames.

---

## 5. Timeout Policy — Implementation Details

### Allowlist (15 scenarios, all I/O-heavy NeoFS)

```text
phase3_neofs_fsync_filecap_stale_regression
phase3_neofs_fsync_dirty_persists_after_crash
phase3_neofs_unfsynced_dirty_may_be_lost
phase3_neofs_fsync_issues_flush
phase3_neofs_fsync_clean_no_extra_flush
phase3_neofs_fsync_coordinates_with_writeback_gc
phase3_neofs_writeback_reboot_after_drain
phase3_neofs_writeback_crash_before_drain
phase3_neofs_gc_reboot_persistence
phase3_neofs_gc_crash_before_publish
phase3_build1_reboot_persistence_smoke
phase3_neofs_reboot_persistence_small_file
phase3_neofs_crash_mid_overwrite_recovery
phase3_neofs_superblock_rotation_recovery
phase3_neofs_crash_mid_create_recovery
```

Membership criterion (carried from dispatch §2): scenario uses a NeoFS storage
disk AND performs fsync/flush/writeback/GC or reboot persistence work dense
enough to issue hundreds of virtio operations or multiple durable flushes per
attempt. Read-only or mount-once scenarios are explicitly excluded — they
finish well within 180s on every observed run.

No scenarios added or removed from the dispatch-suggested list.

### Override env var

`NEO_QEMU_TEST_TIMEOUT_SECS` — optional manual diagnostic override. Accepts
a positive integer number of seconds. Invalid values fail before launching
QEMU. Unset behavior falls through to the allowlist.

---

## 6. Smoke Proofs

### Timeout assignment

```text
$ docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=timeout-smoke-180 neo cargo neo-test ipc_heap_kill_smoke
[xtask] neo-test: building scenario `ipc_heap_kill_smoke`
[xtask] neo-test: launching QEMU for `ipc_heap_kill_smoke` (serial: …/ipc_heap_kill_smoke.timeout-smoke-180.attempt1.serial.log)
[xtask] neo-test: timeout for `ipc_heap_kill_smoke` = 180s
[xtask] neo-test: `ipc_heap_kill_smoke` PASS
```

```text
$ docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=smoke2 neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
[xtask] neo-test: building scenario `phase3_neofs_fsync_filecap_stale_regression`
[xtask] neo-test: launching QEMU for `phase3_neofs_fsync_filecap_stale_regression` (serial: …/phase3_neofs_fsync_filecap_stale_regression.smoke2.attempt1.serial.log)
[xtask] neo-test: attaching raw storage disk … drive=if=none,id=hd0,format=raw,cache=writeback,file=…
[xtask] neo-test: timeout for `phase3_neofs_fsync_filecap_stale_regression` = 360s
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS
```

Default writeback drive string confirmed (no storage-mode override applied),
180s and 360s both confirmed by literal xtask output.

### Invalid override rejection

```text
$ docker compose run --rm -e NEO_SMP=4 -e NEO_QEMU_TEST_TIMEOUT_SECS=not-a-number \
    -e NEO_TEST_RUN_ID=invalid2 neo cargo neo-test ipc_heap_kill_smoke
xtask: `ipc_heap_kill_smoke` failed after 2 attempts;
  last error: NEO_QEMU_TEST_TIMEOUT_SECS="not-a-number" is not a positive integer; refusing to launch QEMU;
  previous error: NEO_QEMU_TEST_TIMEOUT_SECS="not-a-number" is not a positive integer; refusing to launch QEMU
```

```text
$ docker compose run --rm -e NEO_SMP=4 -e NEO_QEMU_TEST_TIMEOUT_SECS=0 \
    -e NEO_TEST_RUN_ID=zero2 neo cargo neo-test ipc_heap_kill_smoke
xtask: `ipc_heap_kill_smoke` failed after 2 attempts;
  last error: NEO_QEMU_TEST_TIMEOUT_SECS=0 is invalid; refusing to launch QEMU;
  previous error: NEO_QEMU_TEST_TIMEOUT_SECS=0 is invalid; refusing to launch QEMU
```

QEMU was never spawned in either invalid case (no `qemu stderr` line, no
serial log written). The retry loop still ran both attempts and produced the
"failed after 2 attempts" wrapper, but the underlying error is identical and
explicit. **Invalid override is rejected before QEMU launch in both cases.**

---

## 7. Stale-fsync 50x Loop Under 360s Policy

```text
Sequential stale-fsync 50x NEO_SMP=4 default writeback + 360s policy
run 1  [t5-stale-01]: PASS
run 2  [t5-stale-02]: PASS
run 3  [t5-stale-03]: PASS
run 4  [t5-stale-04]: PASS (retry)
run 5  [t5-stale-05]: PASS
run 6  [t5-stale-06]: PASS
run 7  [t5-stale-07]: PASS
run 8  [t5-stale-08]: PASS
run 9  [t5-stale-09]: PASS
run 10 [t5-stale-10]: PASS
run 11 [t5-stale-11]: PASS
run 12 [t5-stale-12]: PASS
run 13 [t5-stale-13]: PASS
run 14 [t5-stale-14]: PASS
run 15 [t5-stale-15]: PASS
run 16 [t5-stale-16]: PASS
run 17 [t5-stale-17]: PASS (retry)
run 18 [t5-stale-18]: FAIL (double 360s timeout)
runs 19-50 NOT REACHED

SUMMARY: 17 passed, 1 failed, 2 retry-pass
```

### Final-failure command line and error

```text
$ docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=t5-stale-18 \
    neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression

[xtask] neo-test: timeout for `phase3_neofs_fsync_filecap_stale_regression` = 360s
[xtask] neo-test: launching QEMU for `phase3_neofs_fsync_filecap_stale_regression`
  (serial: …/phase3_neofs_fsync_filecap_stale_regression.t5-stale-18.attempt2.serial.log)
[xtask] neo-test: attaching raw storage disk … drive=if=none,id=hd0,format=raw,cache=writeback,file=…

xtask: `phase3_neofs_fsync_filecap_stale_regression` failed after 2 attempts;
  last error: QEMU timed out after 360s for `phase3_neofs_fsync_filecap_stale_regression`;
  previous error: QEMU timed out after 360s for `phase3_neofs_fsync_filecap_stale_regression`
```

---

## 8. Run 18 Final-Failure Evidence (Both Attempts)

Native preservation via Fix 12 — both attempt logs are on disk at the
exact paths reported by xtask.

### Attempt 1 — file:
`target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-18.attempt1.serial.log`

- Size: **28243 bytes**
- Last `event=PROGRESS` frame: **none — only `event=BEGIN` reached**
- Tail (last 5 lines): all `[ kernel ] virtio-blk: completion ... status=0 ... wake=Woken`
- Stall stage: **pre-mount** (issued ~235 virtio-blk completions, never reached `mount_done`)

### Attempt 2 — file:
`target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-18.attempt2.serial.log`

- Size: **21049 bytes**
- Last `event=PROGRESS` frame: **none — only `event=BEGIN` reached**
- Tail (last 5 lines): all `[ kernel ] virtio-blk: completion ... status=0 ... wake=Woken`
- Stall stage: **pre-mount** (issued ~175 virtio-blk completions, never reached `mount_done`)

### Adjacent context: Run 17 attempt 1 (retry-pass)

`target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-17.attempt1.serial.log`

- Size: **17561 bytes**
- Last `event=PROGRESS` frame: **none — only `event=BEGIN` reached**
- Same stall pattern: pre-mount, ~145 virtio-blk completions
- Attempt 2 succeeded with full PROGRESS + PASS frames (mount completed)

### Run 4 retry-pass (preserved earlier, also pre-mount)

`target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-04.attempt1.serial.log`
- Size: 16907 bytes, only `event=BEGIN`, pre-mount stall

### Cluster summary under 360s policy

| Run | Attempt | Last PROGRESS | Size | Stage |
|-----|---------|---------------|------|-------|
| 4   | 1 (failed) | none | 16907 B | pre-mount |
| 4   | 2 (PASS) | refreshed_fsync_done | — | full |
| 17  | 1 (failed) | none | 17561 B | pre-mount |
| 17  | 2 (PASS) | refreshed_fsync_done | — | full |
| 18  | 1 (failed) | none | 28243 B | pre-mount |
| 18  | 2 (FAILED) | none | 21049 B | pre-mount |

**All four failed attempts under 360s stalled in pre-mount.** None reached
`mount_done`. The seed_fsync clustering observed under Fix 9 (180s) is gone
— the 360s budget covers seed_fsync entirely. The tail has shifted upstream
to **mount-validation I/O**.

Critically: the failed attempts ARE making I/O progress (status=0 completions,
~145–235 per attempt). The mount-validation loop is iterating, just not fast
enough. This is consistent with the run 17 retry recovery: identical workload,
different host scheduling, retry completed mount in time.

---

## 9. Comparison Across Lanes

| | Fix 9 (180s default) | Fix 14 (360s allowlist) |
|---|---|---|
| Final PASS rate | 41/42 = 97.6% | 17/18 = 94.4% |
| Final-fail rate | 2.4% | **5.6% — worse?** |
| Hit failure at run | 42 | **18 — sooner** |
| Retry-pass rate | 12.2% | 11.1% (2/18, similar) |
| Tail stall stage | seed_fsync (after mount_done) | **pre-mount (before mount_done)** |
| Retry recovery on stalled attempt | usually | usually |

Two notable observations:

1. **The failure rate at run 18 looks worse than at run 42**, but this is a
   tiny sample (1 in 18 vs 1 in 42). True comparison needs more samples; the
   Fix 9 run-42 double-failure was also a single event in a ~50% probability
   tail. What's defensible: the 360s policy did NOT eliminate final
   double-timeouts. It shifted the tail's location.

2. **The stall stage has migrated from seed_fsync to pre-mount.** Under 180s,
   tests that needed >180s for seed_fsync timed out there. Under 360s,
   seed_fsync completes; the new bottleneck is mount-validation, which can
   itself take >360s on unlucky iterations.

This pattern argues that the underlying issue is **per-FS-operation latency
variance under host I/O contention**, not a fixed per-test budget shortfall.
Doubling the budget shifted but didn't remove the tail.

---

## 10. PM Decision Required

The dispatch's residual options after Fix 14 fails:

### (a) Extend timeout further (720s)

Mechanical: change `TEST_TIMEOUT_NEOFS_IO_HEAVY` from 360 to 720 (or higher).
Treats the symptom. The data argues this would just shift the tail again to
some next bottleneck. NOT recommended — diminishing returns.

### (b) Smaller valid NeoFS test disk (just >64 MiB)

Implementation: change `neofs-mkfs` to allow a smaller-than-128-MiB image
(provided the three-superblock policy still holds at e.g. 72 MiB). Would
reduce mount-validation I/O proportionally — the dominant work under the
mount-stall failure mode. **This now looks like the highest-impact lane**
since the tail has shifted upstream to mount, which scales directly with
disk size. Earlier dispatch §3.10 had partially deprioritized this; the
Fix 14 data inverts that — mount is now the dominant bottleneck.

### (c) Storage architecture lane (pipelined drain / multi-in-flight virtio)

Highest blast radius. Still forbidden without prior design-note authorization.
Would address both mount-validation I/O serialization and fsync drain
serialization in one shot. Best engineering payoff if PM is willing to
fund the design + invariants + tests.

### (d) Accept residual tail; close BUILD-4 with the policy + caveat

Substrate now passes 17/18 with 360s, including 2 clean retry-pass recoveries.
Fix 14's policy + Fix 8/11/12 preservation means even the final failure has
exact diagnostic logs. PM could accept this as the operating envelope: NeoFS
I/O-heavy tests get 360s, occasional final double-timeouts at ~5% rate
require diagnostics-driven re-run rather than block CI.

---

## 11. Recommended Path

I recommend **(b) smaller valid NeoFS test disk** as the next lane, scoped to:

1. Verify `neofs-mkfs` accepts a ~72 MiB or ~80 MiB image (preserving the
   three-superblock invariant).
2. Update the per-scenario disk-size policy in xtask to use the smaller image
   for the I/O-heavy allowlist.
3. Re-run the stale-fsync 50x loop under 360s + smaller disk.

Rationale:
- The Fix 14 data shows mount-validation I/O is now the dominant tail
  contributor (4/4 failed attempts under 360s stalled pre-mount).
- Mount work scales with `total_blocks` (free-map validation, inode-tree
  walks). A ~40% disk shrink should shrink mount-validation I/O proportionally.
- It does NOT change kernel code paths — same blast radius as Fix 14.
- If it fails, (c) storage architecture lane is the right next step with
  strong evidence.

Alternative: **(d) accept and close** is a defensible choice if PM views
the residual ~5% per-run double-timeout rate (at 360s) as environmental
enough to live with. The substrate has Fix 8/11/12/14 + PROGRESS frames
which together make every timeout diagnosable.

I do NOT recommend (a) further timeout extension — the data argues this
won't solve the underlying I/O variance, just shift it.

---

## 12. Preserved Evidence Paths

```text
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-01..17.attempt1.serial.log    (loop attempt 1 logs)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-04.attempt1.serial.log        (retry — pre-mount, 16907 B)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-04.attempt2.serial.log        (retry attempt 2 PASS)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-17.attempt1.serial.log        (retry — pre-mount, 17561 B)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-17.attempt2.serial.log        (retry attempt 2 PASS)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-18.attempt1.serial.log        (FAIL — pre-mount, 28243 B)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t5-stale-18.attempt2.serial.log        (FAIL — pre-mount, 21049 B)
target/neo-test/ipc_heap_kill_smoke.timeout-smoke-180.attempt1.serial.log                          (180s policy smoke)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.smoke2.attempt1.serial.log             (360s policy smoke PASS)
```

Loop transcript: `bl1gtowg2.output` (31 lines, summary at line 30).

---

## 13. BUILD-4B Status

**BUILD-4B remains blocked.** No regression scenarios run. No full-suite gate
attempted. No commits.
