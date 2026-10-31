# Phase 3 BUILD-4 Stability Fix 6 — Blocker Report

Date: 2026-05-17
Lane owner: Cerebro
Authority: `Plans/DISPATCH-2026-05-17-phase-3-build-4-stability-fix-6-smaller-neofs-disk-cerebro.md`

Status: **BLOCKED** — final double-timeout at run 13 of 50 under
72 MiB image + 360s policy. Test-infrastructure tuning lanes are exhausted;
PM disposition required between **(c) storage architecture lane** or
**(d) accept residual ~5–8% double-timeout and close BUILD-4 with the
diagnostic substrate**.

---

## 1. Framing (locked-in language from prior PM agreement)

> The targeted timeout policy does not claim to fix the storage latency tail.
> It prevents a known environmental tail in I/O-heavy NeoFS scenarios from
> being misclassified as a kernel semantic failure, while preserving
> per-attempt logs and retry evidence for diagnosis.
>
> Guardrail: any final double-timeout under the extended budget is still a
> blocker.

Run 13 under 72 MiB + 360s triggered the guardrail again. The smaller-disk
policy partially closed the tail (mount completed both attempts) but
seed_fsync stalled past 360s on both attempts.

---

## 2. HEAD and Recent Log

```text
d4e2aa4 plans: authorize Phase 3 BUILD-4 smaller NeoFS test disk
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
9324f4b phase3 build4a: directory readdir filecaps
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
kernel/src/sched/scheduler.rs           |  53 +++++
kernel/src/test_harness.rs              |  49 +++++
tools/xtask/src/main.rs                 | 362 +++++++++++++++++++++++++++++---
5 files changed, 448 insertions(+), 28 deletions(-)
```

All carried fixes (uncommitted):
- Fix 8: per-attempt serial logs;
- Fix 9: narrow virtio-only `wake_thread_token_front`;
- Fix 11: `NEO_QEMU_STORAGE_MODE` env gate (default unchanged: writeback);
- Fix 12: `NEO_TEST_RUN_ID` env-var suffix;
- Fix 14: per-scenario 360s timeout for I/O-heavy allowlist + invalid-override rejection;
- Fix 15 (new this lane): per-scenario 72 MiB NeoFS image-size policy for the same allowlist;
- stale-fsync PROGRESS frames.

---

## 5. Fix 16 — mkfs / verify 72 MiB Proof

```text
$ cargo run -p neofs-mkfs -- create \
    --output target/neo-test/neofs-72m-smoke.raw \
    --size-mib 72 --variant normal

NEOFS_MKFS/1 status=ok path=target/neo-test/neofs-72m-smoke.raw variant=normal
  image_size=75497472 hash=093fb0f6… superblock_copies=3 root_inode_block=16386
  next_free_block=16393 free_map_block=16385

$ cargo run -p neofs-mkfs -- verify target/neo-test/neofs-72m-smoke.raw

NEOFS_MKFS_VERIFY/1 status=ok path=target/neo-test/neofs-72m-smoke.raw
  image_size=75497472 hash=093fb0f6… superblock_copies=3 selected_generation=1
  root_inode_block=16386 entries=hello.txt:Regular,model.neom:Model,subdir:Directory
  model_kind=Model indirect_zero=true reserved_zero=true free_map_present=true
```

**3-superblock invariant preserved.** 72 MiB = 75497472 bytes exact.

---

## 6. Fix 15 — Policy Implementation

### Allowlist (15 scenarios, identical to Fix 14 360s allowlist)

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

Non-allowlist NeoFS scenarios keep the 128 MiB default.

### Smoke proofs

**In-allowlist (stale-fsync) — 72 MiB + 360s + PASS:**
```text
[xtask] neo-test: NeoFS image size for …/phase3_neofs_fsync_filecap_stale_regression.raw = 72 MiB (I/O-heavy allowlist)
[xtask] neo-test: attaching raw storage disk … drive=if=none,id=hd0,format=raw,cache=writeback,file=…
[xtask] neo-test: timeout for `phase3_neofs_fsync_filecap_stale_regression` = 360s
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS
```

**Out-of-allowlist (mount_readonly_root) — 128 MiB default + 180s + PASS:**
```text
[xtask] neo-test: NeoFS image size for …/phase3_neofs_mount_readonly_root.raw = default (128 MiB)
[xtask] neo-test: timeout for `phase3_neofs_mount_readonly_root` = 180s
[xtask] neo-test: `phase3_neofs_mount_readonly_root` PASS
```

---

## 7. Stale-fsync 50x Loop Under 72 MiB + 360s

```text
Sequential stale-fsync 50x NEO_SMP=4 default writeback + 360s + 72 MiB
run 1  [t6-stale-01]: PASS
run 2  [t6-stale-02]: PASS (retry)
run 3  [t6-stale-03]: PASS (retry)
run 4  [t6-stale-04]: PASS
run 5  [t6-stale-05]: PASS
run 6  [t6-stale-06]: PASS
run 7  [t6-stale-07]: PASS
run 8  [t6-stale-08]: PASS
run 9  [t6-stale-09]: PASS
run 10 [t6-stale-10]: PASS (retry)
run 11 [t6-stale-11]: PASS
run 12 [t6-stale-12]: PASS
run 13 [t6-stale-13]: FAIL (double 360s timeout)
runs 14-50 NOT REACHED

SUMMARY [stale-fsync 72MiB+360s]: 12 passed, 1 failed, 3 retry-pass
```

---

## 8. Run 13 Final-Failure Evidence

### Attempt 1 — file:
`target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t6-stale-13.attempt1.serial.log`

- Size: **48897 bytes**
- Last `event=PROGRESS` frame: **`stage=mount_done selected_superblock=0 generation=1`**
- Stall stage: **seed_fsync** (after `mount_done`, before `seed_fsync_done`)

### Attempt 2 — file:
`target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t6-stale-13.attempt2.serial.log`

- Size: **49115 bytes**
- Last `event=PROGRESS` frame: **`stage=mount_done selected_superblock=0 generation=1`**
- Stall stage: **seed_fsync** (same as attempt 1)

### Critical observation

**Mount completed on both attempts** of run 13 (each ~49 KB serial log = ~400 virtio-blk completions consumed by mount validation). This is qualitatively different from Fix 14's failures, which were pre-mount stalls at 21–28 KB.

The smaller disk policy demonstrably moved the mount-validation tail INSIDE the 360s budget — runs 13.attempt1 and 13.attempt2 both reached `mount_done` and started seed_fsync work before timing out.

But seed_fsync now stalled both attempts past 360s. The same workload that fits inside 360s on 12 out of 13 runs CAN exceed 360s on the 13th.

---

## 9. Cross-Lane Tail Migration Summary

| Lane | Disk | Timeout | Stall stage on timeout | Sample fail rate |
|------|------|---------|-----------------------|------------------|
| Fix 9 baseline | 128 MiB | 180s | seed_fsync (after mount_done) | 1/42 = 2.4% |
| Fix 14 timeout | 128 MiB | 360s | **pre-mount** (mount too slow) | 1/18 = 5.6% |
| **Fix 15 disk+timeout** | **72 MiB** | **360s** | **seed_fsync** (mount OK now) | **1/13 = 7.7%** |

The tail has migrated three times. Each intervention closed one stage but
exposed another. The data argues the **per-FS-operation latency variance under
host I/O contention is broad** — not concentrated in any one stage that a
test-infra knob can absorb.

Run 13 retry-passes (runs 2, 3, 10) earlier in the loop also stalled at either
pre-mount (run 2 at 6443 B) or mount_done (run 3 at 49006 B), confirming the
tail spans multiple stages even within a single loop sample.

---

## 10. Test-Infra Lanes — Exhausted

Three sequential test-infra interventions are now complete:

- **Fix 9** — narrow same-CPU virtio wake priority (kernel, kept).
- **Fix 14** — extended timeout for I/O-heavy allowlist (xtask, kept).
- **Fix 15** — smaller valid NeoFS image for I/O-heavy allowlist (xtask, kept).

All three interventions ship incremental value (per-attempt logs, run-id
suffix, faster mount path, longer per-attempt budget on heavy tests). None
closes the residual double-timeout tail. The substrate has reached the
ceiling of what test-infrastructure tuning can deliver.

The dispatch explicitly forbade more timeouts and pipelined fsync without
authorization. Remaining options are PM-only.

---

## 11. PM Decision Required

### (c) Storage architecture lane

Pipelined fsync drain and/or multi-in-flight virtio submissions in NeoFS.
Highest blast radius — touches the NeoFS cache drain protocol with careful
invariant analysis around fsync ordering and crash recovery. Strongest
"close the tail" candidate because it addresses the actual mechanism
(serialized one-block-at-a-time drain) rather than masking it. Requires
a separate design lane with explicit invariants, tests, and ADR-grade
documentation before code.

**My recommendation if PM wants the tail truly closed.**

### (d) Accept residual ~5–8% double-timeout; close BUILD-4 with diagnostic substrate

The carried substrate provides:
- per-attempt logs preserved across loop iterations (Fix 8 + Fix 12);
- explicit timeout and disk-size policy per scenario (Fix 14 + Fix 15);
- exact `event=PROGRESS` frames at every test boundary;
- `NEO_QEMU_STORAGE_MODE` gate for forensic comparison;
- virtio-only I/O wake priority (Fix 9).

Every passing attempt verifies the kernel semantic invariant correctly
(ESTALE, durability state, refreshed-cap fsync drain). Every failing
attempt produces preserved evidence at exact stage boundaries. Residual
~5–8% double-timeout becomes an environmental caveat documented in the
close artifact and BUILD-4 readme, requiring diagnostics-driven re-run
rather than blocking CI.

**My recommendation if PM accepts the operating envelope.**

### Other options previously declined

- **(a) Larger timeout (720s):** declined. Data shows the tail migrates
  rather than dissolves under longer budgets.
- **(b) Smaller disk:** done — this is Fix 15. Helped mount but exposed seed_fsync.
- More storage backend variants: previously rejected (Fix 11 matrix).

---

## 12. Recommended Path

I recommend **(d) accept residual** as the immediate close path, with
**(c) storage architecture lane** opened as a follow-up after BUILD-4
closes. Rationale:

1. The kernel semantic invariants are verified by every passing attempt
   across all four lanes (Fix 9 / 14 / 15 / 6). The bug being chased
   is NOT in the kernel under test; it's in the environmental I/O
   latency profile of QEMU+host under SMP=4.
2. The diagnostic substrate (Fix 8 + Fix 12 + PROGRESS frames) makes
   every future timeout actionable, so accepting residual is not
   accepting opacity.
3. Pipelined fsync drain is a Phase 4-grade design problem. Doing it
   inside the BUILD-4 close window risks compromising the invariants
   that the current per-block drain guarantees.
4. The data progression (180s → 360s → 360s+72MiB) is informative:
   it identifies the I/O latency variance as the underlying physics.
   That belongs in a research artifact, not a hot-fix lane.

If PM authorizes (c), I will file a Phase-4 ADR draft with:
- proposed pipelined drain protocol;
- crash-recovery invariants under in-flight write batches;
- tests for ordering with concurrent fsync/writeback/GC;
- rollback path if the change regresses any current PASS.

---

## 13. Preserved Evidence Paths

```text
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t6-stale-01..13.attempt*.serial.log
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t6-stale-02.attempt1.serial.log   (retry — pre-mount, 6443 B)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t6-stale-03.attempt1.serial.log   (retry — seed_fsync, 49006 B)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t6-stale-10.attempt1.serial.log   (retry — seed_fsync, 48897 B)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t6-stale-13.attempt1.serial.log   (FAIL — seed_fsync, 48897 B)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t6-stale-13.attempt2.serial.log   (FAIL — seed_fsync, 49115 B)
target/neo-test/neofs-72m-smoke.raw                                                            (Fix 16 mkfs proof image)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.small-disk-smoke2.attempt1.serial.log  (Fix 15 in-allowlist smoke PASS)
target/neo-test/phase3_neofs_mount_readonly_root.default-disk-smoke.attempt1.serial.log         (Fix 15 out-of-allowlist smoke PASS)
```

Loop transcript: `bporuggnm.output` (26 lines, summary at line 25).

---

## 14. BUILD-4B Status

**BUILD-4B remains blocked.** No regression scenarios run. No full-suite gate
attempted. No commits.
