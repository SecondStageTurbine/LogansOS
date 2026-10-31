# Phase 3 BUILD-4 Wake-Token-Abandon Root-Cause Close

Date: 2026-05-18 (lane authorized 2026-05-18; verification completed 2026-05-19 local)
Agent: Cerebro
Status: COMPLETE — root-caused + narrow test-harness fix + targeted loops + full suite green, ready for PM consumption
Authority: `Plans/DISPATCH-2026-05-18-wake-token-abandon-root-cause-cerebro.md`

## 1. Worktree state

- HEAD before this lane's commit: `7d8448c1d14970397fca7479fca37e172f748938` (main, post `plans: consume stale-fsync irqfix close`)
- Final intended-commit diff: **1 file changed, 50 insertions, 36 deletions** (test_harness.rs only)

## 2. Root cause (one paragraph)

Same-CPU `SwitchMutex` deadlock on the test-harness-static `WAKE_TOKEN_STATE`. The worker thread (`wake_token_abandon_blocker`) acquires the lock **inside `without_interrupts`** (IF=0) at `test_harness.rs:8443-8447`. Five other helpers (`reset_wake_token_state`, `wait_for_primary_token`, `wait_for_second_token`, `wait_for_primary_done`, `wait_for_second_done`) acquired the same lock **without `without_interrupts`** (IF=1). Under `NEO_SMP=4` a timer can preempt an IF=1 holder between `lock()` and the drop; the worker scheduled to the same CPU enters its `without_interrupts` block and spins forever on the held lock; the preempted holder's CPU has IF=0 so the timer that would re-schedule the holder never fires. Identical mechanism class to the BUILD-4 stale-fsync deadlock, different lock.

## 3. Classification (dispatch §3 A-G)

**Case E** — scheduler lock/livelock — with the specific mechanism being a same-CPU `SwitchMutex` deadlock between IF=0 and IF=1 holders of `WAKE_TOKEN_STATE`.

Cases ruled out by audit + reproduction evidence:

| Case | Why ruled out |
|---|---|
| A | The harness never observes past `enqueued user thread (id=7)` — observation has not yet happened. |
| B | Ready-queue and parked-token scans (`kill_thread_by_pid`, `zombify_parked_thread_by_pid`) both multi-CPU at current HEAD; not reached on the failing path anyway. |
| C/D | `process::terminate` and `wake_thread_token` are not reached on the failing path. |
| F | No panic frames or partial-fail output. |
| G | Reproduces locally — 1 retry-pass at run 4, 1 final-fail at run 6 of the 50× reproduction loop, with identical convergent-PAUSE signatures across all 3 timed-out attempts. |

## 4. Serial last-frame evidence (3 captured timeouts)

All three attempts show identical truncation:

```
NEO_TEST/1 event=BEGIN name=phase3_sched_wake_token_abandon phase=dispatch
[ kernel ] sched: enqueued user thread (id=7)
<truncated at 360s timeout>
```

| Run | Tag | Outcome | Last serial line |
|---|---|---|---|
| 4 attempt 1 | wake-token-rootcause-04 | retry-pass | `[ kernel ] sched: enqueued user thread (id=7)` |
| 6 attempt 1 | wake-token-rootcause-06 | final-fail | `[ kernel ] sched: enqueued user thread (id=7)` |
| 6 attempt 2 | wake-token-rootcause-06 | final-fail | `[ kernel ] sched: enqueued user thread (id=7)` |

## 5. Timeout monitor dumps — convergent multi-CPU spin

| Run | KASLR-corrected RIP | All 4 CPUs at same RIP? | CPU0 IF | CPU1-3 IF |
|---|---|---|---|---|
| 4 attempt 1 | `0xffffffff8008ec72` → `core::core_arch::x86::sse2::__mm_pause` (sse2.rs:26) | YES | 0 | 1 |
| 6 attempt 1 | same offset (different KASLR base) | YES | 0 | 1 |
| 6 attempt 2 | same offset (different KASLR base) | YES | 0 | 1 |

Symbol resolution via `addr2line -e target/x86_64-neoplutarch/debug/kernel`. KASLR offset computed from `PER_CPU` ELF address (`0xffffffff80121f40`) vs CPU0 GS-base in each dump.

Same convergent-PAUSE signature as the BUILD-4 stale-fsync deadlock. Debug build inlines `core::hint::spin_loop()` to the shared `__mm_pause` symbol, so RIP alone says "spinning somewhere" — but the IF=0 / IF=1 split across the 4 CPUs combined with the test-harness audit narrows the spin site uniquely to `WAKE_TOKEN_STATE` lock acquisition under `without_interrupts`.

## 6. Code diff summary — test-harness fix

Single file: `kernel/src/test_harness.rs`. Five helpers wrapped:

| Helper | Old behavior | New behavior |
|---|---|---|
| `reset_wake_token_state` | `WAKE_TOKEN_STATE.lock()` at IF=1, assign | wrapped in `without_interrupts(\|\| {…})` |
| `wait_for_primary_token` | `WAKE_TOKEN_STATE.lock().token` at IF=1, read | lock+read wrapped; yield/reap stay at IF=1 |
| `wait_for_second_token` | `WAKE_TOKEN_STATE.lock().second_token` at IF=1, read | same shape |
| `wait_for_primary_done` | `WAKE_TOKEN_STATE.lock()` at IF=1, check done+ret, drop, reap loop | lock+check returns `Option<i64>`; reap loop stays at IF=1 |
| `wait_for_second_done` | `WAKE_TOKEN_STATE.lock()` at IF=1, check second_done+second_ret, drop, reap loop | same shape |

The yield/reap loops are deliberately NOT inside `without_interrupts` — they need IF=1 to make scheduling progress. Only the lock acquisition + critical-section reads run with IF=0, matching the worker.

The added comment in `reset_wake_token_state` documents the invariant for future readers (cross-references the BUILD-4 virtio fix).

## 7. Targeted loop results (dispatch §7)

All four under `NEO_SMP=4` with `NEO_QEMU_TIMEOUT_DUMP=1` + unique `NEO_TEST_RUN_ID`:

| Scenario | Required | Result | Retry-pass | Final-fail |
|---|---|---|---|---|
| `phase3_sched_wake_token_abandon` × 50 | 50/50 final PASS, no truncation recurrence | **50/50 PASS** | **0** | **0** |
| `ipc_heap_kill_smoke` × 30 | scheduler-adjacent guardrail | **30/30 PASS** | **0** | **0** |
| `cap_revoke_local` × 30 | scheduler-adjacent guardrail | **30/30 PASS** | **0** | **0** |
| `phase3_neofs_fsync_filecap_stale_regression` × 20 | stale-fsync stays clean | **20/20 PASS** | **0** | **0** |

130 / 130 total. Wake-token-abandon dropped from ~17% retry-pass (pre-fix, isolated 30-run baseline) to **0%**. The truncation signature `event=BEGIN ... enqueued user thread (id=7) <truncated>` did not recur. Stale-fsync remained clean of the prior virtio deadlock signature.

## 8. Build gates

| Gate | Command | Result | Warnings |
|---|---|---|---|
| xtask build | `cargo build -p xtask` | OK | 0 |
| kernel default | `cargo k` | OK | 72 (baseline preserved) |
| kernel no-default | `cargo k --no-default-features` | OK | 72 (baseline preserved) |

## 9. Final full-suite gate (dispatch §7)

```
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result: **exit 0** — all scenarios PASS sequentially under `NEO_SMP=4`. Zero `FAIL` / `TIMEOUT` / `panic` strings in the output. Last visible scenarios were `phase3_neofs_readdir_stale_generation` PASS and `phase3_aarch64_virtio_barrier_lane` PASS.

## 10. Simplify gate (dispatch §8)

The fix is the entire diff — five small `without_interrupts` wraps in one file. No instrumentation was added during this lane (the existing per-attempt logs + `NEO_QEMU_TIMEOUT_DUMP` durable substrate inherited from the stale-fsync close were sufficient to localize the bug without new scaffolding).

Simplify-gate decisions:

| Component | Class | Action |
|---|---|---|
| `without_interrupts` wraps on `WAKE_TOKEN_STATE` helpers | the fix | KEEP |
| Explanatory comment in `reset_wake_token_state` | documents invariant | KEEP |
| Anything else | — | nothing else added |

## 11. Literal wake-token-abandon PASS line after fix

```
[xtask] neo-test: launching QEMU for `phase3_sched_wake_token_abandon` (serial: /workspace/target/neo-test/phase3_sched_wake_token_abandon.wake-irqfix-smoke.attempt1.serial.log)
[xtask] neo-test: attaching raw storage disk for `phase3_sched_wake_token_abandon` (/workspace/target/neo-test/phase3_sched_wake_token_abandon.raw) drive=if=none,id=hd0,format=raw,cache=writeback,file=/workspace/target/neo-test/phase3_sched_wake_token_abandon.raw
[xtask] neo-test: `phase3_sched_wake_token_abandon` PASS
```

## 12. Preserved evidence

| Location | Contents |
|---|---|
| `target/neo-test/phase3_sched_wake_token_abandon.wake-token-rootcause-04.attempt{1,2}.serial.log` | Retry-pass run from the 50× reproduction loop; attempt-1 timed out with convergent-PAUSE dump, attempt-2 PASSED |
| `target/neo-test/phase3_sched_wake_token_abandon.wake-token-rootcause-04.attempt1.serial.timeout-dump.log` | Per-CPU register dump showing 4-CPU `__mm_pause` convergence |
| `target/neo-test/phase3_sched_wake_token_abandon.wake-token-rootcause-06.attempt{1,2}.serial.timeout-dump.log` | Final double-timeout: same convergent-PAUSE signature both attempts |
| `target/neo-test/phase3_sched_wake_token_abandon.wake-irqfix-*.attempt1.serial.log` | 130 PASS runs across 4 scenarios post-fix (no timeouts, no retries) |

## 13. Lane status

- **Wake-token-abandon scheduler timing tail**: ROOT-CAUSED + FIXED + VERIFIED. Was the same-CPU `SwitchMutex` deadlock class as BUILD-4 stale-fsync, on a test-harness-static lock. The fix is test-harness-only.
- **BUILD-4 stale-fsync**: stays closed; 20/20 PASS in the regression guardrail above.
- **BUILD-4B**: REMAINS BLOCKED until PM consumes this close artifact (dispatch §10).

## 14. Audit-derived lessons

The audit in `Plans/AUDIT-2026-05-18-wake-token-abandon-substrate-survey.md` correctly ruled out the older 2026-05-16 CPU-local `kill_thread_by_pid` finding (already fixed at HEAD), correctly named Case B and Case E as the surviving candidates, and correctly identified `NEO_QEMU_TIMEOUT_DUMP=1` as a sufficient discriminator. The reproduction loop with no new instrumentation classified the bug in 6 runs — durable substrate from the prior lane paid for itself immediately.

**A class-level observation worth recording**: the SwitchMutex IF=0/IF=1 deadlock is not specific to virtio or to test-harness statics. Any code path where some holders take a `SwitchMutex` inside `without_interrupts` and others take it outside is vulnerable. A broader audit of `SwitchMutex` usage with mixed-IF holders is the natural next-step lane after BUILD-4B unblocks — not in scope here, but worth filing as a follow-up.

## 15. Commit policy compliance (dispatch §10)

All five preconditions met:

- [x] tail classified (Case E, §3)
- [x] fix narrow and tied to classification (test-harness only, §6)
- [x] simplify gate complete (§10 — no scaffolding to remove)
- [x] targeted loops pass (§7 — 130/130)
- [x] final full-suite `cargo neo-test --no-meta` exits 0 (§9)
- [x] close artifact written (this file)

## 16. Suggested commit shape

Single commit, test-harness only:

```
test/wake-token: hold WAKE_TOKEN_STATE with IF=0 in wait helpers

The wake_token_abandon_blocker (and the round-trip / stale-generation
blockers) acquire WAKE_TOKEN_STATE inside without_interrupts. The five
helpers — reset_wake_token_state, wait_for_primary_token,
wait_for_second_token, wait_for_primary_done, wait_for_second_done —
acquired it at IF=1, opening a same-CPU SwitchMutex deadlock window
under NEO_SMP=4: timer preempts an IF=1 holder; worker scheduled to
the same CPU spins on the held lock at IF=0; the preempted holder
can never be re-scheduled.

Same deadlock class as the BUILD-4 virtio IRQ fix
(7166128 fs/virtio-blk: hold device-inner lock with IF=0).

Verified: 130/130 targeted loops, full NEO_SMP=4 cargo neo-test --no-meta
exits 0. Pre-fix isolated baseline: 17% retry-pass on
phase3_sched_wake_token_abandon.

Close: Plans/PHASE-3-BUILD-4-WAKE-TOKEN-ABANDON-ROOT-CAUSE-CLOSE-2026-05-18.md
```

No push. No merge. No BUILD-4B until PM consumes this artifact.
