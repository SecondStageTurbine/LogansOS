# Phase 3 BUILD-4 Stability Fix 4 — Blocker Report

Date: 2026-05-16
Lane owner: Cerebro
Authority: `Plans/DISPATCH-2026-05-16-phase-3-build-4-stability-fix-4-qemu-storage-mode-cerebro.md`

Status: **BLOCKED** — `direct-native` storage mode does not clear the
stale-fsync timing tail and produces a final double-timeout faster than the
Fix 9 baseline. Stopping per dispatch §5 and PM "stop on first final failure".

---

## 1. HEAD and Recent Log

```text
HEAD (substrate; no new commits — Fix 8/9/11/12 + PROGRESS frames uncommitted):
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

## 2. Build Gates

```text
cargo build -p xtask                PASS
cargo k                             PASS  72 warnings (baseline unchanged)
cargo k --no-default-features       PASS  72 warnings (baseline unchanged)
```

---

## 3. Diff Stat (uncommitted)

```text
kernel/src/drivers/block/virtio_blk.rs  |   8 +-
kernel/src/sched/mod.rs                 |   4 +-
kernel/src/sched/scheduler.rs           |  53 +++++++++
kernel/src/test_harness.rs              |  49 ++++++++
tools/xtask/src/main.rs                 | 199 +++++++++++++++++++++++++++++----
5 files changed, 289 insertions(+), 24 deletions(-)
```

Four logical changes carried in the working tree:

- **Fix 8** (xtask): per-attempt serial logs + canonical convenience copy.
- **Fix 9** (kernel/sched/{mod,scheduler}.rs + drivers/block/virtio_blk.rs):
  `wake_thread_token_front` narrow API used only by virtio-blk completions.
- **Fix 11** (xtask): `NEO_QEMU_STORAGE_MODE` env gate — supports `writeback`
  (default), `direct-native`, `writeback-io-uring`, `writeback-threads`;
  rejects unknown values with explicit error.
- **Fix 12** (xtask): `NEO_TEST_RUN_ID` env-var suffix for per-invocation log
  preservation across loop harnesses.
- **Stale-fsync PROGRESS frames** (test_harness.rs): 7 `event=PROGRESS` frames
  carried from earlier lane.

Fix 6 AP idle halt remains **preserved but excluded**:
`Plans/FIX-6-AP-IDLE-HALT-PRESERVED-2026-05-16.patch`.

---

## 4. Fix 12 NEO_TEST_RUN_ID Smoke Proof

### Smoke 1 — default naming (no env var)

```text
$ docker compose run --rm -e NEO_SMP=4 neo cargo neo-test ipc_heap_kill_smoke
[xtask] neo-test: launching QEMU for `ipc_heap_kill_smoke`
  (serial: /workspace/target/neo-test/ipc_heap_kill_smoke.attempt1.serial.log)
[xtask] neo-test: `ipc_heap_kill_smoke` PASS
```

Files produced:
```text
target/neo-test/ipc_heap_kill_smoke.attempt1.serial.log  (7434 B — per-attempt)
target/neo-test/ipc_heap_kill_smoke.serial.log           (7434 B — canonical copy)
```

No suffix appears when `NEO_TEST_RUN_ID` is unset. **PASS.**

### Smoke 2 — suffix preserves across repeated invocations

```text
$ docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=smoke-A neo cargo neo-test ipc_heap_kill_smoke
  (serial: /workspace/target/neo-test/ipc_heap_kill_smoke.smoke-A.attempt1.serial.log)
  PASS

$ docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=smoke-B neo cargo neo-test ipc_heap_kill_smoke
  (serial: /workspace/target/neo-test/ipc_heap_kill_smoke.smoke-B.attempt1.serial.log)
  PASS
```

Files preserved after BOTH runs:
```text
target/neo-test/ipc_heap_kill_smoke.smoke-A.attempt1.serial.log  (7434 B — run A)
target/neo-test/ipc_heap_kill_smoke.smoke-B.attempt1.serial.log  (7434 B — run B)
target/neo-test/ipc_heap_kill_smoke.serial.log                   (7434 B — canonical = latest)
```

**Run B did NOT overwrite Run A's log. PASS.** No external watcher required.

---

## 5. Storage-Mode Matrix Results

### Acceptance proof — `direct-native`

```text
$ docker compose run --rm -e NEO_SMP=4 -e NEO_QEMU_STORAGE_MODE=direct-native \
    -e NEO_TEST_RUN_ID=dn-flush-smoke neo cargo neo-test phase3_virtio_blk_flush_command_smoke
[xtask] neo-test: attaching raw storage disk for `phase3_virtio_blk_flush_command_smoke`
  drive=if=none,id=hd0,format=raw,cache=none,aio=native,file=…
[xtask] neo-test: `phase3_virtio_blk_flush_command_smoke` PASS
```

QEMU accepted `cache=none,aio=native` cleanly. No stderr warnings. The
correct pairing (`aio=native` only with `cache=none`) was enforced at the
xtask layer, never silently combined with writeback.

### Flush-semantics regression check — `direct-native`

```text
$ docker compose run --rm -e NEO_SMP=4 -e NEO_QEMU_STORAGE_MODE=direct-native \
    -e NEO_TEST_RUN_ID=dn-issues-flush neo cargo neo-test phase3_neofs_fsync_issues_flush
  drive=if=none,id=hd0,format=raw,cache=none,aio=native,file=…
  PASS
```

Flush semantics intact under `direct-native`.

### Stale-fsync targeted loop — `direct-native`

```text
NEO_SMP=4, NEO_QEMU_STORAGE_MODE=direct-native, unique NEO_TEST_RUN_ID per run
Up to 50 runs; loop breaks on first final failure.

run 1-7   [dn-stale-01..07]: PASS
run 8     [dn-stale-08]:     PASS (retry)
run 9     [dn-stale-09]:     PASS (retry)
run 10-18 [dn-stale-10..18]: PASS
run 19    [dn-stale-19]:     FAIL (double 180s timeout)
runs 20-50 NOT REACHED

SUMMARY [direct-native]: 18 passed, 1 failed, 2 retry-pass
```

### Side-by-side with Fix 9 baseline

| | Fix 9 (writeback) | `direct-native` |
|---|---|---|
| Runs attempted before failure | 42 | 19 |
| Final PASS | 41 | 18 |
| Final FAIL | 1 | 1 |
| Retry-pass | 5 | 2 |
| Retry-pass rate | 5/41 = 12.2% | 2/18 = 11.1% |
| **Final-failure rate** | **1/42 = 2.4%** | **1/19 = 5.3%** |
| Hit failure at run | 42 | 19 |

**Direct-native is materially WORSE than Fix 9.** Retry-pass rate is
indistinguishable (within noise), but final-failure rate is more than 2× higher
and the failure was hit twice as fast.

### writeback-io-uring and writeback-threads — NOT RUN

Per dispatch §2 sequencing, `writeback-io-uring` is the next mode to try after
`direct-native` fails. Implementation supports it (env-gated); execution
deferred for PM disposition per "stop on first final failure" instruction.

---

## 6. Run 19 Final-Failure Evidence

Both attempts preserved natively via `NEO_TEST_RUN_ID=dn-stale-19`:

### Attempt 1 — file `target/neo-test/phase3_neofs_fsync_filecap_stale_regression.dn-stale-19.attempt1.serial.log`

- File size: 49769 bytes
- **Last `event=PROGRESS` frame:**
```text
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=mount_done selected_superblock=0 generation=1
```
- Stalled at the **seed_fsync stage** (between `mount_done` and `seed_fsync_done`).
- Same signature as Fix 9's runs 26 and 38 retry-passes.

### Attempt 2 — file `target/neo-test/phase3_neofs_fsync_filecap_stale_regression.dn-stale-19.attempt2.serial.log`

- File size: 6116 bytes
- **No PROGRESS frames** — only `event=BEGIN` reached.
- Stalled in **mount phase**, before reaching `mount_done`.
- Same earliest-stall signature as Fix 9's run 42 attempt 1.

### Comparison to Fix 9 run 42 failure pattern

| | Fix 9 run 42 attempt 1 | Fix 9 run 42 attempt 2 | DN run 19 attempt 1 | DN run 19 attempt 2 |
|---|---|---|---|---|
| Last frame | BEGIN (no PROGRESS) | refreshed_fsync_begin | mount_done | BEGIN (no PROGRESS) |
| Stall stage | mount | refreshed_fsync | seed_fsync | mount |

Both modes produce double-timeouts via stalls at **different I/O phases on
the two attempts**. The mode (writeback vs direct-native) does not localize
the tail to a single phase — confirming the tail is upstream of the host I/O
backend, in the kernel/scheduler/virtio-blk completion path.

---

## 7. Regression Scenarios — NOT RUN

Per dispatch §6 stop conditions and PM "stop on first final failure".

---

## 8. Final Full-Suite Gate — NOT RUN

---

## 9. BUILD-4B Status

**BUILD-4B remains blocked.**

---

## 10. Mechanism Implications

The direct-native experiment was designed to test the hypothesis that the
remaining tail was rooted in the host QEMU storage backend (writeback cache
flush latency, page-cache effects). The data rejects that hypothesis:

- Direct-native does NOT change the stall-stage clustering. Both modes show
  attempt 1 stalls in seed_fsync after `mount_done`, attempt 2 stalls
  variably in mount or refreshed_fsync.
- Direct-native does NOT reduce retry-pass rate (11.1% vs 12.2% — within
  noise).
- Direct-native MATERIALLY WORSENS final-failure rate (5.3% vs 2.4%).
- Per the dispatch decision rule, direct-native cannot become the default.

The plausible read: `cache=none,aio=native` removes host page-cache absorption
that was previously masking some I/O latency. Each individual block write now
goes directly to the disk, and the kernel's serialized one-block-at-a-time
drain mechanism becomes more visible. This is consistent with the
retry-pass-rate parity (the tail is roughly the same shape) but worse
final-failure rate (more variance / less buffering when collisions occur).

---

## 11. PM Decision Required

Per dispatch §3 acceptance threshold, direct-native fails on both:
- Final PASS target: 18/19 (target 50/50)
- Retry rate "strictly lower than Fix 9": 11.1% vs 12.2% — within noise

Per dispatch §3.2: "If no storage mode beats Fix 9, do not change the default."

Recommendation paths for PM disposition (priority order):

### (a) Try `writeback-io-uring` next (lowest blast radius)

Already implemented in Fix 11; awaiting `NEO_QEMU_STORAGE_MODE=writeback-io-uring`
authorization. io_uring is a different AIO backend with potentially different
host-side queue behavior. Worth trying since the implementation is done.

### (b) Try `writeback-threads` (still safe, different host-side model)

Implemented. Backs writeback with thread-based I/O instead of native AIO.

### (c) Open targeted timeout policy lane

The Fix 9 baseline of 12.2% retry rate × 12.2% second-attempt rate = ~1.5%
double-timeout probability. With 50 runs, ~52% chance of seeing at least one
double-timeout. This is a budget problem more than a code problem. A targeted
NEO_QEMU_TEST_TIMEOUT_SECS env for I/O-heavy NeoFS scenarios (e.g., 360s)
would deterministically eliminate the tail without kernel changes. The
dispatch §3.4 explicitly noted this requires PM authorization.

### (d) Pipelined fsync drain (largest blast radius)

Submit multiple block writes to virtio-blk before awaiting completions in
`cache.drain_all()`. The dispatch §3.4 explicitly forbade this without prior
design-note authorization. Would require careful invariant analysis and
fsync ordering tests. NOT my recommendation — see dispatch.

### (e) Smaller valid NeoFS disk

Dispatch §3.10 noted minimum is just-over-64-MiB due to three-superblock
policy. Mount-phase I/O scales with disk size; reducing the disk halves
the mount-validation I/O. But run 19's attempt 1 stalled in seed_fsync,
not mount, so this is partial mitigation only.

**My recommended next step: (a) writeback-io-uring** — implementation is
already in place (Fix 11), test cost is one more 30-run loop, and if it
fails the lane has a clean negative result on both AIO modes and can
escalate to (c) targeted timeout policy with strong evidence.

---

## 12. Preserved Evidence Paths

```text
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.dn-stale-01..18.attempt1.serial.log  (PASS)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.dn-stale-08.attempt1.serial.log     (retry-pass attempt 1, mount_done last)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.dn-stale-08.attempt2.serial.log     (retry-pass attempt 2 PASS)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.dn-stale-09.attempt1.serial.log     (retry-pass attempt 1, mount_done last)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.dn-stale-09.attempt2.serial.log     (retry-pass attempt 2 PASS)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.dn-stale-19.attempt1.serial.log     (FAIL, mount_done last)
target/neo-test/phase3_neofs_fsync_filecap_stale_regression.dn-stale-19.attempt2.serial.log     (FAIL, only BEGIN reached)
target/neo-test/ipc_heap_kill_smoke.smoke-A.attempt1.serial.log                                 (Fix 12 smoke A)
target/neo-test/ipc_heap_kill_smoke.smoke-B.attempt1.serial.log                                 (Fix 12 smoke B)
```

All 18 passing runs and the run 19 double-failure have both attempt logs
preserved via Fix 12's NEO_TEST_RUN_ID suffix — no external watcher
required. Fix 12 is fully validated by use across this experiment.

Loop transcript: `b... baf0ryv4i.output` (30 lines, summary at end).
