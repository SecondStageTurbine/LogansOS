# Phase 3 BUILD-4 WAKE_TOKEN_STATE IF-Invariant Correction Close

Date: 2026-05-19
Agent: Cerebro
Status: IN PROGRESS — fix landed, build gates clean, targeted loops in flight
Authority: `Plans/DISPATCH-2026-05-19-wake-token-state-if-invariant-correction-cerebro.md`

## 1. Worktree state

- HEAD at lane start: `1789cb727088abfbc930cd99b6b9b65dcdbc1994` (main, post `plans: block wake-token close on IF invariant`)
- Final intended-commit diff: **1 file changed, 6 insertions, 6 deletions** (test_harness.rs only)

## 2. What was missed in the prior commit (`f63232d`)

The prior wake-token close fix wrapped the five wait-side helpers but missed three post-wake writer sites in the blocker functions:

| Site | Function | What it writes |
|---|---|---|
| `test_harness.rs:8425` | `wake_token_round_trip_blocker` | `state.ret = ret; state.done = true;` |
| `test_harness.rs:8542` | `wake_token_stale_first_blocker` | `state.ret = ret; state.done = true;` |
| `test_harness.rs:8558` | `wake_token_stale_second_blocker` | `state.second_ret = ret; state.second_done = true;` |

These are post-`block_current_with_token_and_guard` writes — after the blocker resumed from its IF=0 critical section. The acquisition was happening at IF=1 because the resumed thread is no longer inside `without_interrupts`. None of them are on the exact failing path that the close artifact named (the abandon scenario never reaches them), but **they violate the same invariant**: every `WAKE_TOKEN_STATE` holder must run with IF=0.

PM was right to block the close.

## 3. Code diff

Each of the three sites had the same `{ let mut state = WAKE_TOKEN_STATE.lock(); … }` shape. Wrapping changed the bracket scope to a `without_interrupts(|| { … });` closure. No semantic change beyond IF state during the lock.

```rust
// Before (round-trip blocker, ~8424)
{
    let mut state = WAKE_TOKEN_STATE.lock();
    state.ret = ret;
    state.done = true;
}

// After
crate::arch::x86_64::without_interrupts(|| {
    let mut state = WAKE_TOKEN_STATE.lock();
    state.ret = ret;
    state.done = true;
});
```

Same shape applied to the two stale-generation blockers (`state.ret/done` and `state.second_ret/second_done`).

## 4. rg coverage verification (dispatch §2 requirement)

`rg -n "WAKE_TOKEN_STATE\\.lock" kernel/src/test_harness.rs`:

```
8315:        let mut state = WAKE_TOKEN_STATE.lock();      <- reset_wake_token_state (inside without_interrupts at :8314)
8327:        let token = ... without_interrupts(|| WAKE_TOKEN_STATE.lock().token);    <- wait_for_primary_token
8341:        ... without_interrupts(|| WAKE_TOKEN_STATE.lock().second_token);          <- wait_for_second_token
8359:                let state = WAKE_TOKEN_STATE.lock();   <- wait_for_primary_done (inside without_interrupts at :8358)
8390:                let state = WAKE_TOKEN_STATE.lock();   <- wait_for_second_done (inside without_interrupts at :8389)
8419:        let guard = WAKE_TOKEN_STATE.lock();           <- wake_token_round_trip_blocker pre-block (inside without_interrupts at :8418)
8425:        let mut state = WAKE_TOKEN_STATE.lock();       <- wake_token_round_trip_blocker post-wake (NEW: inside without_interrupts at :8424)
8458:        let guard = WAKE_TOKEN_STATE.lock();           <- wake_token_abandon_blocker (inside without_interrupts at :8457)
8536:        let guard = WAKE_TOKEN_STATE.lock();           <- wake_token_stale_first_blocker pre-block (inside without_interrupts at :8535)
8542:        let mut state = WAKE_TOKEN_STATE.lock();       <- stale_first post-wake (NEW: inside without_interrupts at :8541)
8552:        let guard = WAKE_TOKEN_STATE.lock();           <- wake_token_stale_second_blocker pre-block (inside without_interrupts at :8551)
8558:        let mut state = WAKE_TOKEN_STATE.lock();       <- stale_second post-wake (NEW: inside without_interrupts at :8557)
```

**Every one of the 12 acquisitions is inside a `without_interrupts` block.** Invariant satisfied.

## 5. Build gates

| Gate | Command | Result | Warnings |
|---|---|---|---|
| xtask build | `cargo build -p xtask` | OK | 0 |
| kernel default | `cargo k` | OK | 72 (baseline preserved) |
| kernel no-default | `cargo k --no-default-features` | OK | 72 (baseline preserved) |

## 6. Targeted loop results (dispatch §4)

All under `NEO_SMP=4` with `NEO_QEMU_TIMEOUT_DUMP=1` + unique `NEO_TEST_RUN_ID`:

| Scenario | Required | Result | Retry-pass | Final-fail |
|---|---|---|---|---|
| `phase3_sched_wake_token_abandon` × 50 | 50/50 final PASS | **50/50 PASS** | **0** | **0** |
| `phase3_sched_wake_token_round_trip` × 50 | 50/50 final PASS | **50/50 PASS** | **0** | **0** |
| `phase3_sched_wake_token_stale_generation` × 50 | 50/50 final PASS | **50/50 PASS** | **0** | **0** |

All three wake-token scenarios clean. Previous `event=BEGIN … enqueued user thread (id=7)` truncation did not recur in any of the 150 wake-token runs.

## 7. Guardrail loop results (dispatch §4)

| Scenario | Required | Result | Retry-pass | Final-fail |
|---|---|---|---|---|
| `ipc_heap_kill_smoke` × 30 | scheduler-adjacent guardrail | **30/30 PASS** | **0** | **0** |
| `cap_revoke_local` × 30 | scheduler-adjacent guardrail | **30/30 PASS** | **0** | **0** |
| `phase3_neofs_fsync_filecap_stale_regression` × 20 | stale-fsync stays clean | **20/20 PASS** | **0** | **0** |

230 / 230 total across targeted + guardrail loops. Stale-fsync remained clean of the prior virtio deadlock signature.

## 8. Final full-suite gate

```
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result: **exit 0**. Zero `FAIL` / `TIMEOUT` / `panic` strings in the output. Last visible scenarios were `phase3_neofs_readdir_stale_generation` PASS and `phase3_aarch64_virtio_barrier_lane` PASS — same end-of-suite ordering as the prior wake-token-abandon close, confirming the suite ran end-to-end.

## 9. Class-level follow-up: broader mixed-IF SwitchMutex audit

The prior wake-token close noted that this deadlock class extends beyond virtio and beyond test-harness statics. PM disposition agreed but explicitly out-of-scope for this lane. Filing as a forward-looking follow-up: a kernel-wide audit of `SwitchMutex` holders for mixed-IF acquisition patterns would catch the next instance before it becomes a livelock. Recommended candidates for first inspection:

- `PROCESSES` BTreeMap mutex
- `PRIMARY_DEVICE: SwitchMutex<Option<Arc<VirtioBlkDevice>>>`
- IPC channel state mutexes
- The slab-level `THREAD_CACHE` mutex

Not in scope for this commit. Naming it here so future Cerebro / PM can find it.

## 10. BUILD-4B status

REMAINS BLOCKED until PM consumes this correction close. Per dispatch §6: no push, no merge, no BUILD-4B until consumption.

## 11. Suggested commit shape

```
test/wake-token: wrap remaining WAKE_TOKEN_STATE writes in without_interrupts

The f63232d wake-token close wrapped the five wait-side helpers but
missed three post-wake writer sites inside the blocker functions:

- wake_token_round_trip_blocker post-wake ret/done write
- wake_token_stale_first_blocker post-wake ret/done write
- wake_token_stale_second_blocker post-wake second_ret/second_done write

These do not appear on the failing wake_token_abandon path, but they
violate the same invariant: every WAKE_TOKEN_STATE holder must run
with IF=0. Wrap each in without_interrupts so the lock is held
consistently with the blockers' IF=0 pre-wake critical sections and
the wait-side helpers from f63232d.

After this commit, every WAKE_TOKEN_STATE.lock() in test_harness.rs
is inside a without_interrupts block.

Verified: 50/50 abandon, 50/50 round-trip, 50/50 stale-generation,
30/30 ipc_heap_kill_smoke, 30/30 cap_revoke_local, 20/20 stale-fsync.
Full NEO_SMP=4 cargo neo-test --no-meta exits 0.

Close: Plans/PHASE-3-BUILD-4-WAKE-TOKEN-STATE-IF-INVARIANT-CORRECTION-CLOSE-2026-05-19.md
```
