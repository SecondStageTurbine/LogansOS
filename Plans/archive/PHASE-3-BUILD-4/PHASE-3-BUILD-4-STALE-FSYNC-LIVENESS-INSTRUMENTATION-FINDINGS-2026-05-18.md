# Phase 3 BUILD-4 Stale-Fsync Liveness Instrumentation Findings

Date: 2026-05-18
Agent: Cerebro
Status: CLASSIFICATION COMPLETE (no fix yet, per dispatch)
Authority: `Plans/DISPATCH-2026-05-18-stale-fsync-liveness-instrumentation-cerebro.md`

The commandment held: classify first, fix second.

## 1. Worktree state

- HEAD: `13bbfe20b6e4dc73d5900d2ce52356849c1fcb4f` (main)
- Uncommitted diff: 781 insertions across 8 files (Fix 8/9/11/12/14/15 + this instrumentation pass)
- No commit yet, per dispatch §7

```
 kernel/src/drivers/block/virtio_blk.rs |  58 ++-
 kernel/src/fs/mod.rs                   |   5 +
 kernel/src/fs/neofs/cache.rs           | 150 +++-
 kernel/src/fs/neofs/mod.rs             |   2 +-
 kernel/src/sched/mod.rs                |   4 +-
 kernel/src/sched/scheduler.rs          |  53 ++
 kernel/src/test_harness.rs             |  51 ++
 tools/xtask/src/main.rs                | 494 ++++-
```

## 2. Instrumentation summary

### 2.1 virtio_blk submission-side counters (`kernel/src/drivers/block/virtio_blk.rs`)

New `#[cfg(feature = "neo-test")]` atomics + accessors:

| Atomic | Accessor | Set by | Cleared by |
|---|---|---|---|
| `SUBMISSIONS` | `debug_submission_count` | `submit_pending` | `reset_probe_state` |
| `LAST_SUBMITTED_REQUEST_TYPE` | `debug_last_submitted_request_type` | `submit_pending` | reset to `u32::MAX` |
| `LAST_SUBMITTED_LBA` | `debug_last_submitted_lba` | `submit_pending` | reset to `u64::MAX` |
| `LAST_SUBMITTED_TOKEN_CPU/SLOT` | `debug_last_submitted_token_cpu/_slot` | `submit_pending` | reset |
| `CURRENT_IN_FLIGHT_REQUEST_TYPE` | `debug_current_in_flight_request_type` | `submit_pending` set; `poll_used` clear | reset |

### 2.2 NeoFS fsync liveness runtime gate (`kernel/src/fs/neofs/cache.rs`, `mod.rs`, `kernel/src/fs/mod.rs`)

- `FSYNC_LIVENESS_TRACE_ENABLED: AtomicBool` (default false)
- `FSYNC_LIVENESS_TRACE_NAME: SwitchMutex<Option<&'static str>>`
- `FSYNC_DRAIN_SEQ: AtomicU64`
- `pub fn debug_set_fsync_liveness_trace(enabled, name)` re-exported through `fs::debug_neofs_set_fsync_liveness_trace`

OFF by default. Only the stale-fsync test enables it.

### 2.3 NEO_DIAG/1 frames around drain (`cache.rs`)

Seven event types bracketing every blocking write/flush:

| Event | Bracket |
|---|---|
| `FSYNC_DRAIN_ENTER` | start of `drain_all` |
| `FSYNC_DRAIN_EXIT` | clean exit |
| `FSYNC_DRAIN_YIELD` | drain_all yield-on-no-progress |
| `FSYNC_WRITE_BEGIN` | before `write_fs_block` |
| `FSYNC_WRITE_DONE` | after `write_fs_block` |
| `FSYNC_FLUSH_BEGIN` | before `flush_device_with_retry` |
| `FSYNC_FLUSH_DONE` | after `flush_device_with_retry` |

Uniform field set: scenario name, drain seq, block/role, dirty/drained/flush_required/flushes, submissions/completions/flush_commands/flush_completions, in_flight_dma, current_in_flight_type, last_submitted_type/lba/token_cpu/token_slot, last_completed_type, last_wake.

### 2.4 Test-harness opt-in (`kernel/src/test_harness.rs`)

`run_phase3_neofs_fsync_filecap_stale_regression` enables the gate immediately after `mount_neofs_rw_or_fail()` and disables it immediately before emitting PASS. No other scenario touches the gate.

### 2.5 xtask on-timeout monitor dump (`tools/xtask/src/main.rs`)

- New env: `NEO_QEMU_TIMEOUT_DUMP=1` (requires `NEO_QEMU_MONITOR_PORT`)
- On timeout, before SIGKILL:
  - TCP connect with 1000ms / 1500ms / 500ms (connect/read/write) timeouts
  - Issues `info status`, `info cpus`, **`info registers -a`** (per-CPU register state)
  - Writes capture to `<serial_log_stem>.timeout-dump.log`
  - Capture failure recorded in dump file (failure itself is Case-A evidence)
- Then SIGKILL as before

## 3. Build gates

| Gate | Command | Result | Warnings |
|---|---|---|---|
| xtask | `cargo build -p xtask` | OK | 0 |
| kernel default | `cargo k` | OK | 72 (baseline preserved) |
| kernel no-default | `cargo k --no-default-features` | OK | 72 (baseline preserved) |

## 4. Instrumented smoke

```
NEO_SMP=4 NEO_TEST_RUN_ID=diag-smoke NEO_QEMU_TIMEOUT_DUMP=1 NEO_QEMU_MONITOR_PORT=45555 \
  cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

Result: **PASS attempt 1**. 40 NEO_DIAG frames covering both drains (seed_fsync + refreshed_fsync) plus the no-drain stale_fsync. Counters move in lockstep across WRITE_BEGIN/WRITE_DONE pairs (in_flight_dma: 3→0, submissions: 392→403, completions: 391→403).

## 5. Targeted loop

Two consecutive sub-loops captured. Stop condition fired in the second (final double-timeout at run 2).

### 5.1 Loop 1 (prefix `t7-diag-`, CPU0-only dump)

| Run | Tag | Outcome | Last frame |
|---|---|---|---|
| 1 | t7-diag-01 | PASS (retry) | attempt-1: `FSYNC_WRITE_BEGIN seq=1 block=16400 role=data dirty=7 drained=10` (refreshed_fsync first block); attempt-2 PASS |
| 2 | t7-diag-02 | (interrupted to upgrade dump) | not run to completion |

Stopped early by Cerebro after run-1 evidence preserved, to upgrade dump from `info registers` (CPU0 only) to `info registers -a` (all 4 vCPUs). Loop-1 evidence archived to `target/neo-test/preserved/2026-05-18-t7-diag-01/`.

### 5.2 Loop 2 (prefix `t7b-diag-`, full per-CPU dump)

| Run | Tag | Outcome | Stall point on failing attempts |
|---|---|---|---|
| 1 | t7b-diag-01 | PASS (retry) | attempt-1 timed out (timeout-dump captured); attempt-2 PASS |
| 2 | t7b-diag-02 | **FINAL FAIL** | attempt-1: `FSYNC_WRITE_BEGIN seq=0 block=16396 role=data dirty=10 drained=0` (seed_fsync FIRST block); attempt-2: `FSYNC_WRITE_BEGIN seq=0 block=256 role=superblock dirty=2 drained=1` (seed_fsync THIRD block) |

`STOP: final double-timeout at run 2` — dispatch stop condition fired.

### 5.3 Timeout dump evidence — convergent multi-CPU spin

All three captured timeouts (loop-2 run-1 attempt-1, loop-2 run-2 attempt-1, loop-2 run-2 attempt-2) show the **identical pattern**:

| Field | Value |
|---|---|
| VM status | `running` (NOT frozen) |
| Online vCPUs | 4 (per `info cpus`) |
| All 4 RIPs | identical within a given run (e.g., run-2 attempt-1: all 4 at `0xffffffff867e9f42`; run-2 attempt-2: all 4 at `0xffffffffbce1ef42`) |
| CPU0 RFL | `0x00000002` → IF=0 (in `without_interrupts` critical section) |
| CPU1, CPU2, CPU3 RFL | `0x00000202` → IF=1 (normal kernel execution) |
| HLT | 0 on all 4 CPUs |
| Different RSPs / GS-bases | yes — 4 distinct threads on 4 distinct stacks |

Symbol resolution (debug build, KASLR-adjusted run-2 attempt-1):

```
0xffffffff867e9f42 - KASLR_offset(0x067bf000) = 0xffffffff8002af42
  → core::core_arch::x86::sse2::__mm_pause
    at .../core_arch/src/x86/sse2.rs:26
```

This is the body of `core::hint::spin_loop()`. Because the kernel is a **debug build** (no inlining), every `spin_loop_hint()` call site shares this single `__mm_pause` symbol — RIP alone does not pick the specific spin loop. But the fact that **every CPU's current thread converges on it** is the signature of multi-CPU spin contention.

### 5.4 What the data rules out

- **Case A (host froze):** ruled out — `info status: VM status: running` and 4 active vCPU threads on every dump.
- **Pure Case B (single virtio submit lost):** ruled out — pre-stall submission/completion counters are matched in 2 of 3 timeouts (run-2 attempt-1 had 1 pending submit; run-2 attempt-2 had submissions=completions=459 immediately before the next WRITE_BEGIN). A bare "virtio lost the request" would leave one CPU spinning on `poll_used` while the others halt or run other work, not all 4 converging on `__mm_pause`.
- **Case G (semantic invariant):** ruled out — no `event=FAIL` frame in any timeout attempt; every termination was at the test-runner timeout, not a kernel-side assertion.

### 5.5 What the data establishes

- Stall point is consistently inside `write_fs_block` (the blocking virtio submit-and-wait), between an emitted `FSYNC_WRITE_BEGIN` and the never-emitted matching `FSYNC_WRITE_DONE`.
- Stall point is **not block-specific**: across the three captures we see attempts stuck on the first data block of seed_fsync, the third superblock block of seed_fsync, and the first data block of refreshed_fsync. Bug fires on any drain iteration, not a specific block.
- One CPU (CPU0 in every capture) has interrupts disabled — consistent with being inside `without_interrupts` (`submit_pending_and_wait` and `wake_thread_token_front` both wrap their critical sections in it).
- Three CPUs have interrupts enabled — consistent with idle/yield-loop threads or non-critical-section kernel work.
- Failure is **timing-sensitive**: 2 of 4 timeouts were rescued by attempt-2 retry on a clean QEMU boot. Same kernel binary, same disk, same env — pass-on-retry confirms a race, not a deterministic bug.

## 6. Classification (A-H)

| Case | Definition | Observed? | Evidence |
|---|---|---|---|
| A | Guest stopped executing | **NO** | VM status: running on every dump |
| B | Virtio submitted, never completed | **PARTIAL** | symptom-matches (WRITE_BEGIN with no DONE) but cannot be the whole story — pre-stall counters often show queue empty |
| C | Completed but waiter not woken | **PROBABLE** | LAST_WAKE_RESULT not directly observed during the stall, but the wake-handoff path is the natural site for the lock-spin pattern |
| D | Waiter woken but did not run | **PROBABLE** | scheduler/wake_thread_token_front lock contention is the natural site for all-4-CPU PAUSE convergence under SMP=4 |
| E | Waiter ran but no fsync progress | **NO** | drained-counter monotone in all PASS runs; no FSYNC_DRAIN_YIELD frames trailing the WRITE_BEGIN |
| F | Steady but too slow | **NO** | not slow-progress — drain-counter does not advance during the 360s stall (no further NEO_DIAG frames at all) |
| G | Semantic invariant failure | **NO** | no FAIL frame |
| H | Insufficient instrumentation | **NO** | instrumentation classified mechanism shape; further isolation (which lock) requires stack-unwind, not more atomics |

**Primary classification: convergent multi-CPU spin livelock in the wake/scheduler handoff path (Case C-or-D family) under SMP=4.**

The specific shape — one CPU inside `without_interrupts` (almost certainly the completion handler executing `wake_thread_token_front` or the submitter mid-`block_current_with_token_and_guard`), and three other CPUs in spin-loop hints with interrupts enabled — is consistent with a contended scheduler-inner lock on the wake-target CPU. The cross-CPU IPI to reschedule the target arrives at a target whose scheduler is already pinned by another spinner; the spin path is shared with idle-yield, so all CPUs converge.

## 7. Observer-effect assessment

The instrumentation **did not mask the failure**. Across 2 runs (loop 2):

| Outcome | Loop 2 (instrumented) | Fix 6 baseline (72 MiB + 360s, uninstrumented, prior session) |
|---|---|---|
| Final-fail rate | 1/2 = 50% (small sample) | 1/13 = 7.7% |
| Retry-pass rate | 1/2 = 50% | 3/13 = 23% |
| Clean PASS rate | 0/2 = 0% | 9/13 = 69% |

Small sample (2 runs). Instrumented failure rates are higher than the Fix 6 baseline rate, but the loop was stopped early at the first final-fail — so this is upper-bound. Important point: **instrumentation REPRODUCED the failure** (both as a retry-pass and a final-fail). Whatever observer-effect the diag frames have, they don't hide the bug.

## 8. Recommended next lane

The classification supports a Case-C/D-family fix lane targeting the **wake-token-to-scheduler handoff** under SMP=4. Specifically:

**Recommended investigation lane (no fix yet):**
1. Stack-unwind instrumentation: add a one-shot kernel-side per-CPU stack dump triggered when the NMI handler fires. QEMU monitor `nmi` can deliver an NMI to one CPU; extending the NMI handler to walk the stack frames and print symbols would let a future dump identify *which* spin site each CPU is in.
2. Lock-holder tracking: instrument `SwitchMutex::lock()` to record the holder CPU/thread-id in the mutex itself (gated by `neo-test`), so a contended mutex shows which CPU is sitting on it.
3. Re-examine the wake path under SMP=4: `wake_thread_token_front` does `without_interrupts → scheduler_for_cpu(token.cpu_id) → sched.inner.lock() → take_thread_by_token → push_front → IPI`. If the target CPU's scheduler is held by an idle-yield context that doesn't yield the lock, the wake spins. Audit whether any code path holds `scheduler.inner.lock()` across a long-running operation or yield.

**Forbidden by the dispatch:** no kernel fix yet. The classification is enough to choose the lane; fix lands after PM consumes findings.

## 9. Preserved evidence

| Location | Contents |
|---|---|
| `target/neo-test/preserved/2026-05-18-t7-diag-01/` | Loop-1 run-1 evidence: attempt1.serial.log, attempt1.timeout-dump.log (CPU0-only), attempt2.serial.log, run-context.txt |
| `target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t7b-diag-01.*` | Loop-2 run-1 attempt logs + timeout dump (all 4 CPUs) |
| `target/neo-test/phase3_neofs_fsync_filecap_stale_regression.t7b-diag-02.*` | Loop-2 run-2 final-fail evidence: 2 attempts + 2 per-CPU timeout dumps |

## 10. Commit policy reminder

No commit until PM consumes findings. No push. No merge. No BUILD-4B. No storage redesign. No fix.
