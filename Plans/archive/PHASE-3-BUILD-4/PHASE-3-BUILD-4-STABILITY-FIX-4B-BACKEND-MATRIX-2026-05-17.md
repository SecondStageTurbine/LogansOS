# Phase 3 BUILD-4 Stability Fix 4B — Writeback Backend Matrix

Date: 2026-05-17
Lane owner: Cerebro
Authority: `Plans/DISPATCH-2026-05-17-phase-3-build-4-stability-fix-4b-writeback-backends-cerebro.md`

Status: **BLOCKED** — neither already-implemented writeback AIO variant beats
the Fix 9 baseline. `writeback-io-uring` is rejected by host kernel.
`writeback-threads` is materially WORSE than Fix 9 (3× retry-pass rate,
new pre-mount stall mode). Stopping per dispatch §5 stop conditions and
deferring to PM for next-step authorization.

---

## 1. Mode Matrix Summary

| Mode | QEMU accept | flush_command_smoke | fsync_issues_flush | stale-fsync result |
|------|-------------|---------------------|---------------------|---------------------|
| `writeback` (Fix 9 baseline) | accepted | PASS | PASS | 41/42 PASS, 5 retry, 1 fail @ run 42; retry rate 12.2% |
| `direct-native` | accepted | PASS | PASS | 18/19 PASS, 2 retry, 1 fail @ run 19; **rejected** (worse final-fail rate) |
| **`writeback-io-uring`** | **REJECTED** | N/A | N/A | N/A |
| **`writeback-threads`** | accepted | PASS | PASS | 11/11 PASS, **4 retry-pass = 36.4% rate (3× Fix 9)**; **rejected** |

---

## 2. Experiment A — `writeback-io-uring`

### Acceptance attempt

Command:
```text
docker compose run --rm -e NEO_SMP=4 \
  -e NEO_QEMU_STORAGE_MODE=writeback-io-uring \
  -e NEO_TEST_RUN_ID=ur-flush-smoke \
  neo cargo neo-test phase3_virtio_blk_flush_command_smoke
```

Drive string from xtask:
```text
if=none,id=hd0,format=raw,cache=writeback,aio=io_uring,file=…
```

QEMU stderr:
```text
qemu-system-x86_64: -drive if=none,id=hd0,format=raw,cache=writeback,aio=io_uring,
  file=/workspace/target/neo-test/phase3_virtio_blk_flush_command_smoke.raw:
  Unable to use io_uring: failed to init linux io_uring ring
```

QEMU exited non-zero without producing any serial output. Xtask correctly
detected the failure via the `read serial log … No such file or directory`
fallback path (a side-effect of the rejection — QEMU never wrote the file).

### Root cause

The host kernel under the Docker compose container does not expose io_uring
to the guest namespace. The container's QEMU binary (Debian-packaged 7.2.22)
is built with io_uring support, but the io_uring_setup syscall is unavailable
at runtime. This is likely a WSL2 host kernel feature gap or a seccomp/capability
restriction in the Docker default profile. Either way, **io_uring is not
viable for local validation on this CI runner equivalent**.

### Decision

Per dispatch §2 sequencing, io_uring rejection authorizes proceeding to
Experiment B (`writeback-threads`).

---

## 3. Experiment B — `writeback-threads`

### Acceptance and flush-semantics smoke

Command:
```text
docker compose run --rm -e NEO_SMP=4 \
  -e NEO_QEMU_STORAGE_MODE=writeback-threads \
  -e NEO_TEST_RUN_ID=thr-flush-smoke \
  neo cargo neo-test phase3_virtio_blk_flush_command_smoke
```

Drive string from xtask:
```text
if=none,id=hd0,format=raw,cache=writeback,aio=threads,file=…
```

- `phase3_virtio_blk_flush_command_smoke` → **PASS** (no QEMU stderr warnings)
- `phase3_neofs_fsync_issues_flush` → **PASS** (no QEMU stderr warnings)

Both flush-semantics checks PASS. QEMU accepted the backend without warnings
or fallback. The mode itself is functional — the question becomes whether it
improves the stale-fsync timing tail.

### Stale-fsync targeted loop

Command shape (each iteration with unique NEO_TEST_RUN_ID):
```text
docker compose run --rm -e NEO_SMP=4 \
  -e NEO_QEMU_STORAGE_MODE=writeback-threads \
  -e NEO_TEST_RUN_ID=thr-stale-NN \
  neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

Result (loop stopped at run 11 per stop condition):
```text
run 1  [thr-stale-01]: PASS
run 2  [thr-stale-02]: PASS (retry)
run 3  [thr-stale-03]: PASS
run 4  [thr-stale-04]: PASS
run 5  [thr-stale-05]: PASS
run 6  [thr-stale-06]: PASS (retry)
run 7  [thr-stale-07]: PASS
run 8  [thr-stale-08]: PASS
run 9  [thr-stale-09]: PASS (retry)
run 10 [thr-stale-10]: PASS
run 11 [thr-stale-11]: PASS (retry)
[loop stopped — retry rate 36.4% = 3× Fix 9 baseline, far above stop threshold]

SUMMARY [writeback-threads]: 11 passed, 0 failed, 4 retry-pass
```

### Retry-pass evidence

All four retry-passes preserved natively via NEO_TEST_RUN_ID. Last `event=PROGRESS`
frames from each failed attempt 1:

| Run | Last PROGRESS frame on attempt 1 | File size | Stall stage |
|-----|----------------------------------|-----------|-------------|
| `thr-stale-02` | `stage=refreshed_fsync_begin dirty_blocks=7 flush_required=true refreshed_generation=9 current_inode_generation=9` | 59055 B | **refreshed_fsync** (final fsync) |
| `thr-stale-06` | `stage=mount_done selected_superblock=0 generation=1` | 50859 B | **seed_fsync** (between mount_done and seed_fsync_done) |
| `thr-stale-09` | (no PROGRESS — only `event=BEGIN`) | 6116 B | **pre-mount** (never reached mount_done) |
| `thr-stale-11` | (no PROGRESS — only `event=BEGIN`) | 6770 B | **pre-mount** (never reached mount_done) |

### Stall-stage distribution comparison

| Stall stage | Fix 9 retry-passes (5 of 41) | direct-native retry-passes (2 of 18) | writeback-threads retry-passes (4 of 11) |
|---|---|---|---|
| pre-mount (only BEGIN) | 0 | 0 | **2 (50%)** |
| seed_fsync (mount_done last) | 5 (100%) | 2 (100%) | 1 (25%) |
| refreshed_fsync (begin last) | 0 | 0 | 1 (25%) |

Under `writeback-threads`, the stall distribution skews EARLIER. Half of the
retries don't even reach `mount_done` — a failure mode never observed under
either `writeback` (Fix 9) or `direct-native`. The mount-phase I/O itself
becomes unstable under threaded AIO.

### Performance comparison summary

| | Fix 9 (writeback default AIO) | direct-native | writeback-threads |
|---|---|---|---|
| Final PASS rate | 41/42 = 97.6% | 18/19 = 94.7% | 11/11 = 100% (small sample) |
| Final-fail rate | 2.4% | 5.3% | unknown (stopped early) |
| **Retry-pass rate** | **12.2%** | 11.1% (within noise) | **36.4% (3× baseline)** |
| Earliest stall (worst case) | seed_fsync | pre-mount | **pre-mount** |
| Stall localization | tight (seed_fsync) | tight (seed_fsync) | **broad** |

`writeback-threads` retry rate at 36.4% is decisively above the Fix 9 baseline
of 12.2%. Per dispatch §5 stop condition ("a mode reaches a retry-pass rate at
or above the Fix 9 baseline by run 30"), the loop was halted at run 11 — the
trend would not statistically improve with more samples (would need 19 of the
next 19 runs to be clean just to reach Fix 9 parity, an impossibly clean
streak given the observed cadence).

---

## 4. Combined Mode-Matrix Conclusion

Neither already-implemented writeback AIO variant beats Fix 9:

- **io_uring**: not runnable on this host (host kernel/container restriction).
  Cannot be evaluated locally even if it would help on a different host.
- **threads**: runnable but materially worse — 3× retry rate and a new
  pre-mount stall mode that was never observed under default writeback or
  direct-native.

Per dispatch §3: "If both writeback backend variants fail to beat the
baseline, file a blocker." Done.

---

## 5. PM Decision Required

Per dispatch §3, the next PM choice is one of:

### (a) Targeted timeout policy for I/O-heavy NeoFS scenarios

Add a `NEO_QEMU_TEST_TIMEOUT_SECS` or per-scenario timeout override. The Fix 9
baseline (12.2% retry rate) × itself = ~1.5% double-timeout probability
yielding the run-42 failure. A targeted 360s budget for stale-fsync would
deterministically eliminate the tail without kernel changes. Lowest blast
radius, fastest to deliver, treats the symptom.

### (b) Smaller valid NeoFS test disk

Dispatch noted minimum is just-over-64-MiB due to three-superblock policy.
The mount-validation I/O scales with disk size, so a 72-MiB or 80-MiB image
would reduce mount cost. But run 19 attempt 1 (direct-native) and run 11
runs (threads) showed stalls at seed_fsync and refreshed_fsync too — these
are independent of disk size. Partial mitigation; lower-confidence fix.

### (c) Storage architecture design lane

Pipelined fsync drain or multi-in-flight virtio. Highest blast radius — touches
the NeoFS cache module's drain protocol and may require careful invariant
analysis around fsync ordering. The dispatch explicitly forbade implementing
this without prior design-note authorization. Strongest "kill the tail"
candidate but expensive.

### (d) Accept retry-pass with explicit caveat

If PM decides the residual tail is environmental enough to live with, Fix 9
+ Fixes 8/11/12 substrate is the de-facto best baseline. BUILD-4 closes with
a documented caveat that stale-fsync may retry-pass on slow hosts. This is
the lowest-cost decision but kicks the underlying issue into a future
"why do flush-heavy tests need retries" debt.

---

## 6. Recommended Path

I recommend **(a) targeted timeout policy**, narrow scope only for the
demonstrably-slow stale-fsync scenario (and its near siblings if shared
fixtures show the same I/O density).

Reasoning:
- The Fix 9 baseline retry rate of 12.2% × probability of second-attempt
  collision yields a ~1.5% double-timeout rate. With 50 runs, ~52% chance
  of at least one double-timeout — purely a budget problem.
- The kernel's behavior on every passing attempt is correct; the test
  faithfully validates stale-cap fsync semantics. The retry-pass attempts
  show the correct invariants (ESTALE returned, durability state unchanged,
  refreshed cap fsync drains).
- (c) is the right "real fix" if PM wants to invest in storage architecture
  evolution, but the data does not show a kernel bug — it shows an I/O
  serialization cost that's borderline on a 180s budget.
- (b) is unreliable because the stalls happen across mount AND fsync.
- (d) is a non-decision that compounds debt.

If PM authorizes (a), the implementation is mechanical: an env-gated timeout
override in xtask, applied only to a denylist/allowlist of I/O-heavy
scenarios.

---

## 7. Preserved Evidence Paths

```text
target/neo-test/phase3_virtio_blk_flush_command_smoke.thr-flush-smoke.attempt1.serial.log     (smoke: threads accept PASS)
target/neo-test/phase3_neofs_fsync_issues_flush.thr-issues-flush.attempt1.serial.log          (smoke: threads fsync PASS)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.thr-stale-01..11.attempt1.serial.log  (loop attempt logs)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.thr-stale-02.attempt2.serial.log  (retry-pass — refreshed_fsync)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.thr-stale-06.attempt2.serial.log  (retry-pass — seed_fsync)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.thr-stale-09.attempt2.serial.log  (retry-pass — pre-mount)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.thr-stale-11.attempt2.serial.log  (retry-pass — pre-mount)
```

Loop transcripts:
- `baf0ryv4i.output` — direct-native loop (30 lines, summary at line 29)
- `bc18pw6sy.output` — writeback-threads loop (12 lines, stopped at run 11)

---

## 8. Diff Stat (unchanged from Stability Fix 4)

```text
kernel/src/drivers/block/virtio_blk.rs  |   8 +-
kernel/src/sched/mod.rs                 |   4 +-
kernel/src/sched/scheduler.rs           |  53 +++++++++
kernel/src/test_harness.rs              |  49 ++++++++
tools/xtask/src/main.rs                 | 199 +++++++++++++++++++++++++++++----
```

Carried fixes (uncommitted, awaiting PM authorization):
- Fix 8: per-attempt serial logs
- Fix 9: `wake_thread_token_front` narrow API + virtio-blk completion uses it
- Fix 11: `NEO_QEMU_STORAGE_MODE` env gate (all 4 modes supported)
- Fix 12: `NEO_TEST_RUN_ID` env-var suffix (proven across this experiment)
- stale-fsync PROGRESS frames (test_harness.rs)

The Fix 6 AP idle halt remains preserved at
`Plans/FIX-6-AP-IDLE-HALT-PRESERVED-2026-05-16.patch` (rejected, kept for
reference).

---

## 9. Build Gates

```text
cargo build -p xtask          PASS
cargo k                       PASS  72 warnings (baseline unchanged)
cargo k --no-default-features PASS  72 warnings (baseline unchanged)
```

---

## 10. BUILD-4B Status

**BUILD-4B remains blocked.** No regression scenarios run. No full-suite gate
attempted. No commits.
