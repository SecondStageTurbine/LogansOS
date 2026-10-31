# Phase 3 BUILD-4 Stale-Fsync Virtio IRQ-Fix Close

Date: 2026-05-18
Agent: Cerebro
Status: COMPLETE — fix landed, simplify gate done, full suite green, ready for PM consumption
Authority: `Plans/DISPATCH-2026-05-18-stale-fsync-virtio-irqsafe-completion-fix-cerebro.md`

## 1. Worktree state

- HEAD before commit: `c184c40c7a61404db673a4c1ae4a454a6868cb22` (main)
- HEAD at lane start: `e2b5c6949005ff50ac543c65d2a710229871306b`
- Final intended-commit diff: **2 files changed, 273 insertions(+), 30 deletions(-)**

```
 kernel/src/drivers/block/virtio_blk.rs |   7 +-
 tools/xtask/src/main.rs                | 296 +++++++++++++++++++++++++++++----
```

## 2. Root cause (one paragraph)

Same-CPU `SwitchMutex` deadlock on `VirtioBlkDevice::inner`. The completion-worker kthread took the lock at `virtio_blk.rs:538` (`poll_completions`) with IF=1; a timer IRQ preempted it mid-critical-section while the lock was still held; the scheduler picked the submitter thread on the same CPU; the submitter entered `submit_pending_and_wait`, called `without_interrupts(|| { self.inner.lock(); ... })` at `virtio_blk.rs:521`, and spun forever — its IF=0 blocked the timer IRQs that could have re-scheduled the preempted holder. Full localization in `Plans/PHASE-3-BUILD-4-STALE-FSYNC-SPIN-LOCALIZATION-FINDINGS-2026-05-18.md`.

## 3. Code diff — kernel mechanism fix

```diff
--- a/kernel/src/drivers/block/virtio_blk.rs
+++ b/kernel/src/drivers/block/virtio_blk.rs
@@ -498,10 +498,13 @@ impl VirtioBlkDevice {
     fn poll_completions(&self) {
-        let completion = {
+        // `submit_pending_and_wait` may spin on this same lock with IF=0 while
+        // parking the submitter; therefore the completion worker must not be
+        // timer-preempted while holding it.
+        let completion = crate::arch::x86_64::without_interrupts(|| {
             let mut inner = self.inner.lock();
             inner.poll_used()
-        };
+        });
```

Seven-line change. No kernel API surface change. No scheduler touch. No fsync touch. No mutex-type swap. No retries/timeouts/disk/backend tuning.

## 4. Why `IrqSafeMutex` direct swap was rejected for this lane

The dispatch authorized only the completion-side critical section. Direct mutex-type swap from `SwitchMutex` to `IrqSafeMutex` would touch every caller (probe, submit, flush, primary-device registration, debug accessors) and bloat the diff well beyond the seven-line fix above. Wrapping `poll_completions` in `without_interrupts` produces the same effective behavior (lock held with IF=0 on this code path) at one site instead of the whole API surface. If a future audit shows other holders of this lock need the same treatment, promoting to `IrqSafeMutex` is the appropriate broader lane — not this one.

## 5. Targeted loop results

### 5.1 `phase3_neofs_fsync_filecap_stale_regression` × 50

- **50 / 50 final PASS**
- **0 retry-pass**
- **0 final double-timeouts**
- Dispatch §4 acceptance bar met cleanly
- Tags: `irqfix-stale-01` through `irqfix-stale-50`

Sample PASS line:
```
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS
```

### 5.2 `phase3_sched_wake_token_abandon` × 30 (mixed loop) → final-fail at run 19

Initial run hit a final-fail in `wake_token_abandon` at run 19 (run #69 overall when counting from stale-fsync). Investigation:

- Both attempts truncated at `event=BEGIN ... [ kernel ] sched: enqueued user thread (id=7)`
- Zero virtio/spin signature in either attempt's serial log (only boot-time `virtio-blk: cap …` lines)
- Pattern matches the pre-existing nondeterministic scheduler timing tail documented in `Plans/CI-SMP-RACE-TRIAGE-FINDINGS-2026-05-16.md`

### 5.3 `phase3_sched_wake_token_abandon` × 30 (isolated re-run with `wake-isolate-NN`)

Re-ran the same scenario standalone (no preceding stale-fsync loop) to disambiguate pre-existing instability from regression:

- **30 / 30 final PASS**
- **5 retry-pass** at runs 1, 8, 18, 22, 29 (≈16.7% retry rate)
- **0 final double-timeouts**
- Zero virtio/spin signature in any of the 5 attempt-1 logs — every truncation lands at the same `enqueued user thread (id=7)` point

Comparison vs mixed loop:

| Metric | Mixed loop (after stale-fsync) | Isolation | Verdict |
|---|---|---|---|
| Retry-pass rate | 3/19 = 15.8% | **5/30 = 16.7%** | identical distribution |
| Final-fail rate | 1/19 = 5.3% | 0/30 = 0% | tail event in mixed, not present in isolation |

**Disposition: pre-existing scheduler instability, orthogonal to the IRQ-mask fix. Wake-token-abandon stays as a separate blocker lane (not part of this close).**

### 5.4 `ipc_heap_kill_smoke` × 20

Not run (the mixed loop's run-19 final-fail stopped it). Re-ran a single instance after the simplify gate (see §7); passed under default 180s timeout.

## 6. Simplify gate

Per dispatch §5, classified each WIP component and removed investigation scaffolding. Stale-fsync smoke between each group. No phantom wins.

| Group | Component | Files | Decision | Smoke after |
|---|---|---|---|---|
| **KEEP** | `poll_completions` IRQ-mask | `virtio_blk.rs:498-510` | KEEP — the fix | n/a |
| **Candidate keep** | Per-attempt serial logs + `NEO_TEST_RUN_ID` suffix | xtask | KEEP (durable substrate per §5) | n/a |
| **Candidate keep** | `NEO_QEMU_TIMEOUT_DUMP` register capture | xtask | KEEP (durable substrate per §5) | n/a |
| **A** | NeoFS fsync liveness gate + NEO_DIAG frames + drain_pass sig + test_harness PROGRESS frames + toggle + fs/mod wrapper + neofs/mod re-export | cache.rs, mod.rs, fs/mod.rs, test_harness.rs | REMOVE | PASS attempt 1 |
| **B** | virtio_blk submission-side debug atomics + accessors + submit_pending/poll_used/reset wire-up | virtio_blk.rs | REMOVE | PASS attempt 1 |
| **C** | NMI handler `dump_all_to_serial` extension + `NEO_QEMU_TIMEOUT_NMI` + NMI injection in xtask | idt.rs, xtask | REMOVE | PASS attempt 1 |
| **D** | `spin_snapshot` module entirely + SwitchMutex contention slow-path + `Location::caller` for owner-site (reverted — out of scope) | spin_snapshot.rs, sync/mod.rs, switch_mutex.rs | REMOVE (file deleted) | PASS attempt 1 |
| **E** | `wake_thread_token_front` (Fix 9) impl + sched/mod re-export + virtio_blk caller | scheduler.rs, sched/mod.rs, virtio_blk.rs | REMOVE | PASS attempt 1 |
| **F** | `NEO_QEMU_STORAGE_MODE` (Fix 11), 360s targeted timeout + I/O-heavy allowlist + `NEO_QEMU_TEST_TIMEOUT_SECS` (Fix 14), 72 MiB smaller-disk policy + allowlist (Fix 15) | xtask | REMOVE | PASS attempt 1 |

After all six removal groups: 72-warning baseline preserved on `cargo k` and `cargo k --no-default-features`, 0 warnings on `cargo build -p xtask`, stale-fsync smoke clean on default 180s timeout / 128 MiB disk / writeback drive.

## 7. Final verification after simplify

Per dispatch §6, in order:

| Gate | Command | Result |
|---|---|---|
| xtask build | `docker compose run --rm neo cargo build -p xtask` | OK (0 warnings) |
| kernel default | `docker compose run --rm neo cargo k` | OK (72 warnings — baseline) |
| kernel no-default | `docker compose run --rm neo cargo k --no-default-features` | OK (72 warnings — baseline) |
| Stale-fsync single | `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression` | **PASS attempt 1** (180s timeout, 128 MiB disk, default writeback) |
| Full suite | `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` | **exit 0** — all scenarios pass under NEO_SMP=4 |

## 8. Literal stale-fsync PASS line after final intended diff

```
[xtask] neo-test: launching QEMU for `phase3_neofs_fsync_filecap_stale_regression` (serial: /workspace/target/neo-test/phase3_neofs_fsync_filecap_stale_regression.attempt1.serial.log)
[xtask] neo-test: attaching raw storage disk for `phase3_neofs_fsync_filecap_stale_regression` (/workspace/target/neo-test/phase3_neofs_fsync_filecap_stale_regression.raw) drive=if=none,id=hd0,format=raw,cache=writeback,file=/workspace/target/neo-test/phase3_neofs_fsync_filecap_stale_regression.raw
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS
```

Note: no `[xtask] neo-test: timeout for ... = Ns` line (Fix 14 eprintln removed); no `NeoFS image size for ... = N MiB` line (Fix 15 eprintln removed); `cache=writeback` is the only mode (Fix 11 env-gate removed).

## 9. Preserved evidence

Investigation evidence kept on disk for audit:

| Location | Contents |
|---|---|
| `target/neo-test/preserved/2026-05-18-t7-diag-01/` | Liveness-instrumentation retry-pass (CPU0-only dump + run-context.txt) |
| `target/neo-test/phase3_neofs_fsync_filecap_stale_regression.spinloc-smoke2.attempt{1,2}.serial.log` | SPIN_SNAPSHOT capture identifying the contended lock + owner |
| `target/neo-test/phase3_neofs_fsync_filecap_stale_regression.spinloc-smoke2.attempt{1,2}.serial.timeout-dump.log` | Per-CPU register dumps showing 4-CPU convergent PAUSE |
| `target/neo-test/phase3_sched_wake_token_abandon.wake-isolate-*.attempt*.serial.log` | Pre-existing wake-token-abandon retry distribution (5/30, 0 final-fails) |

## 10. Lane status

- **BUILD-4 stale-fsync deadlock**: ROOT-CAUSED + FIXED + VERIFIED. The five-week timing-instability rabbit hole that drove Fix 8/9/11/12/14/15 was caused entirely by the IRQ-state mismatch in `poll_completions`.
- **BUILD-4B**: REMAINS BLOCKED until PM consumes this close artifact.
- **Wake-token-abandon timing tail**: separate, pre-existing, documented in CI-SMP-RACE-TRIAGE-FINDINGS-2026-05-16. Stays as its own blocker lane — not in scope for this commit.
- **Failed Fix 11/14/15 tuning** (storage backend, 360s timeout, 72 MiB disk): completely removed. Not shipped.

## 11. Commit policy compliance

All five preconditions met:
- [x] mechanism fix implemented (§3)
- [x] targeted stale-fsync 50x has 0 final double-timeouts (§5.1)
- [x] simplify gate complete (§6)
- [x] final full `NEO_SMP=4 cargo neo-test --no-meta` exits 0 (§7)
- [x] close artifact is written (this file)

Ready to commit.

## 12. Suggested commit shape

Two commits, kernel-then-tooling for clean review:

```
fs/virtio-blk: hold device-inner lock with IF=0 in completion worker

The completion worker took VirtioBlkDevice::inner with IF=1; a timer
preempting mid-critical-section combined with a submitter spinning on
the same lock inside without_interrupts produced a same-CPU livelock
under NEO_SMP=4. Wrap the poll_completions critical section in
without_interrupts so the holder cannot be timer-preempted while the
submitter is parked.

Root cause + localization:
Plans/PHASE-3-BUILD-4-STALE-FSYNC-SPIN-LOCALIZATION-FINDINGS-2026-05-18.md
Close artifact:
Plans/PHASE-3-BUILD-4-STALE-FSYNC-VIRTIO-IRQFIX-CLOSE-2026-05-18.md

Verified: 50/50 stale-fsync clean, full neo-test suite exits 0.
```

```
tools/xtask: preserve per-attempt serial logs + timeout monitor dump

Adds attempt-numbered serial logs, NEO_TEST_RUN_ID suffix for outer-loop
archives, and NEO_QEMU_TIMEOUT_DUMP register capture via the QEMU
monitor. Durable diagnostic substrate kept after the BUILD-4 stale-fsync
investigation closed; nothing in this commit changes default test
behavior.
```

No push. No merge. No BUILD-4B until PM consumes this artifact.
