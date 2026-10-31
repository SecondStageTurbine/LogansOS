# SC-3B stress-gate failure investigation (Prometheus)

**Date:** 2026-05-09
**Author:** Prometheus
**Status:** Investigation analysis. Awaiting PM synthesis with parallel agents' analyses.

---

## 1. Substrate verification

**Files read directly:**
- `kernel/src/sync/switch_mutex.rs` — SwitchMutex implementation
- `kernel/src/sched/scheduler.rs` — `block_current_into_with_guard`, `wake_thread_on_current_cpu`, `place_thread_on_cpu`
- `kernel/src/ipc/mod.rs` — 4 IPC retrofit sites (send, recv, grant, recv_grant)
- `kernel/src/arch/x86_64/context.rs` — `context_switch_with_release2` asm
- `kernel/src/test_harness.rs` — `run_smp_cross_cpu_rendezvous_stress` scenario
- `kernel/src/sched/thread.rs` — Thread struct, cpu_id field
- `kernel/src/sched/process.rs` — process registry (for Q4 wake context)
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md` — PM's triage (H1-H6)

**Key findings per source:**
- `Thread.cpu_id` exists as `AtomicU32` (line 85, 173, 192, 281)
- `SwitchMutex` provides `release_token()` returning `*const AtomicBool` (line 71-73)
- Q4 wake uses `wake_thread_on_current_cpu` which calls `wake_thread_front` (lines 577-581)
- `wake_thread_front` stamps cpu_id before push (line 567)

---

## 2. Phase A — Source reading and hypothesis enumeration

### 2.1 Files read
- `kernel/src/sync/switch_mutex.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/ipc/mod.rs`
- `kernel/src/arch/x86_64/context.rs`
- `kernel/src/test_harness.rs`
- `kernel/src/sched/thread.rs`
- `kernel/src/sched/process.rs`
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md`

### 2.2 Relevant code paths

**`block_current_into_with_guard` (scheduler.rs:483-531):**
1. Acquires scheduler lock
2. Pops next, replaces current, captures `prev_rsp_ptr`
3. Calls installer (which holds channel guard and publishes thread into EndpointState)
4. Captures `channel_lock_ptr` and `sched_lock_ptr` via `release_token()`
5. `mem::forget` both guards
6. Calls `context_switch_with_release2` (asm: switch stack, release Channel lock FIRST, scheduler lock SECOND)
7. On resume, reads `blocked_return`

**Q4 wake (`wake_thread_on_current_cpu`):**
- Calls `wake_thread_front` which:
  - Updates `thread.cpu_id` to completer's CPU (line 567)
  - Pushes to completer's ready queue (front for handoff)

**Stress test pairing (test_harness.rs:2750-2762):**
- Pair i uses sender_cpu=i, receiver_cpu=(i+1)%pairs
- At NEO_SMP=4 with 4 pairs: (0→1), (1→2), (2→3), (3→0)
- Each thread runs every `pairs` iterations (seq += pairs)

### 2.3 Hypothesis enumeration

**H1: Lost wakeup in Q4 wake-on-completer-CPU under contention**

**Rationale:** Under high contention, two completers might simultaneously try to wake the same parked thread (e.g., if test state is inconsistent or a thread is extracted twice from EndpointState). One push succeeds; the other is silently dropped or duplicates, leaving an inconsistent state.

**Confirming evidence:** Instrumentation showing that for some rendezvous, the woken thread appears on the ready queue with the same `return_value` written twice, or that a parked thread is never extracted from EndpointState.

**Disconfirming evidence:** Instrumentation showing each parked thread is extracted exactly once from EndpointState and pushed exactly once to a ready queue.

**Assessment:** H1 is plausible. The stress test creates 4 pairs × 1000 iterations = 4000 rendezvous per run. With Q4's cpu_id stamp and front-queue push, contention windows are large. The "same parked thread woken twice" scenario could cause a thread to never complete (if duplicated) or a waiter to be lost (if push silently dropped).

---

**H2: Race in `block_current_into_with_guard` between installer publication and asm release**

**Rationale:** The installer publishes `prev_thread` into EndpointState. Then `mem::forget` happens. Then asm switches stack and releases locks. After Channel release but before scheduler release, CPU M could extract prev_thread. CPU M then pushes prev_thread to its ready queue via Q4 wake. But: prev_thread.rsp was set by the asm BEFORE the lock release — this should be fine because `[rdi]` write happens before Channel release in the asm.

**Confirming evidence:** Code audit would need to show a window where CPU M extracts prev_thread before the asm's `mov [rdi], rsp` writes the saved RSP. If such a window exists, CPU M would see stale `rsp` and hang.

**Disconfirming evidence:** Code audit confirms the asm writes `[rdi]` (save old rsp) BEFORE the Channel release (`mov [rdx], 0`). This means when CPU M extracts prev_thread, prev_thread.rsp is already the saved rsp.

**Assessment:** H2 is unlikely based on code audit. The asm sequence is:
```
mov [rdi], rsp     # save old rsp to *prev_rsp_ptr (BEFORE lock release)
mov rsp, rsi       # switch to new stack
mov [rdx], 0       # release Channel lock
mov [rcx], 0       # release scheduler lock
```
The `[rdi]` write happens before Channel release, so CPU M sees the correct rsp when it extracts prev_thread.

---

**H3: Test scenario logic deadlock**

**Rationale:** The stress test creates pairs of senders and receivers. If pairing logic is wrong (e.g., sender on CPU 0 paired with receiver on CPU 0, both expecting the OTHER to be on a different CPU), they may both block on each other in a way that doesn't deadlock at low load but does at high load.

**Confirming evidence:** Reading test_harness.rs lines 2750-2762 shows the pairing logic:
```rust
let sender_cpu = pair as u32;
let receiver_cpu = ((pair + 1) % pairs) as u32;
```
This is correct: pair 0 → sender 0, receiver 1; pair 1 → sender 1, receiver 2; etc. No self-pairing.

**Disconfirming evidence:** The pairing logic is verified correct. However, the test could have a different bug — e.g., iteration cleanup that wedges after many iterations.

**Assessment:** H3 is unlikely. The pairing logic is correct. The test's "same CPU" scenario would manifest earlier than run 36, not after 35 clean runs.

---

**H4: Per-CPU ready queue starvation under stress**

**Rationale:** With Q4 wake-on-completer-CPU, all rendezvous completions push to the completer's queue. Under heavy contention, one CPU may end up the completer for most rendezvous and accumulate a long ready queue while other CPUs idle. If the orchestrator (CPU 0) is waiting for progress but all completions push to a busy CPU, progress stalls.

**Confirming evidence:** Instrumentation showing per-CPU ready-queue depth over time during the stress test reveals one CPU's queue growing unbounded while others idle.

**Disconfirming evidence:** Instrumentation shows balanced queue depths across CPUs, or the busy CPU makes progress fast enough to keep up.

**Assessment:** H4 is plausible. The stress test uses round-robin pairing (0→1, 1→2, 2→3, 3→0), so CPU 1 receives from CPU 0 and sends to CPU 2, etc. Under high contention, CPU 1 could be the completer for both pair 0 (receiver) and pair 1 (sender). If CPU 1's queue fills up and the test orchestrator (CPU 0) waits for progress, a deadlock-like state emerges.

---

**H5: SwitchMutex starvation under stress**

**Rationale:** Plain spinlock with no fairness; under heavy contention, one CPU may starve waiting for a mutex repeatedly stolen by other CPUs.

**Confirming evidence:** Instrumentation showing switch_mutex acquisition spin-count or wait-time shows one CPU waiting orders of magnitude longer than others.

**Disconfirming evidence:** Code audit shows SwitchMutex uses the same pattern as `spin::Mutex` (compare_exchange_weak with spin_loop). SwitchMutex's lock byte is kernel-owned (not external crate), so it's not vulnerable to external crate internal layout changes.

**Assessment:** H5 is unlikely. SwitchMutex structurally matches `spin::Mutex`, which has worked in production. The lock is kernel-owned, so there's no external dependency issue.

---

**H6: Asm helper missing memory barrier somewhere it's needed**

**Rationale:** Synthesis §1.4 ratified "no fence." Both PLAN agents agreed x86 TSO is sufficient for AtomicBool::store(false, Release). But: the asm sequence has `mov rsp, rsi` between the stack switch and the lock releases.

**Confirming evidence:** Compiler/CPU reordering the lock-release stores against the stack-switch-derived state. However, stack switches don't have memory dependencies that a CPU would reorder against.

**Disconfirming evidence:** x86 TSO + plain stores is well-understood. Linux uses this exact pattern (spin_unlock is plain MOV + compiler barrier).

**Assessment:** H6 is very unlikely. x86 TSO guarantees stores are observed in program order. The asm release is two plain MOVs; no fence is needed.

---

**H7: Thread extraction from EndpointState not atomic under contention**

**Rationale:** When two completers simultaneously extract the same parked thread from EndpointState, the first writer wins but the second writer's state transition is lost. The thread is pushed to the ready queue once, but both completers think they completed the rendezvous.

**Confirming evidence:** Instrumentation showing that for some iterations, both sender and receiver see their `send/recv` return with the expected value, but only one thread was woken.

**Disconfirming evidence:** Instrumentation shows each rendezvous has exactly one completer that extracts the thread from EndpointState and pushes it to a ready queue.

**Assessment:** H7 is very plausible and matches the failure signature (silent hang after BEGIN). Under high contention, two completers could both extract the same parked thread. One push succeeds; the other is a duplicate (thread already on ready queue). The duplicate push might overwrite the first thread's `blocked_return` or leave the thread in an inconsistent state. The thread that should complete never resumes, causing a hang.

**This is the top candidate** — it explains:
- Why 35/35 passes: low contention makes duplicate extraction rare
- Why run 36 fails: high contention triggers the race
- Why silent hang: the parked thread is never extracted or is extracted but never woken
- Why no BUG/PANIC: no memory corruption, just logic inconsistency

---

## 3. Phase B — Instrumentation strategy

### Per-iteration progress frames

Add to `wait_for_stress_iteration_done`:
```rust
serial_println!(
    "NEO_TEST/1 event=PROGRESS name={} iter={} pair={} sender_cpu={} receiver_cpu={} sender_done={} receiver_done={}",
    SCENARIO, seq, pair,
    STRESS_SENDER_DONE[pair].load(Ordering::Acquire),
    STRESS_RECEIVER_DONE[pair].load(Ordering::Acquire),
    STRESS_SENDER_GO[pair].load(Ordering::Acquire),
    STRESS_RECEIVER_GO[pair].load(Ordering::Acquire)
);
```

### Per-CPU snapshots

Add to the test orchestrator (after each iteration):
```rust
for cpu in 0..pairs as u32 {
    let sched = crate::sched::debug_scheduler_for_cpu(cpu);
    let depth = crate::sched::debug_ready_queue_depth(sched);
    serial_println!(
        "NEO_TEST/1 event=CPU_SNAPSHOT name={} cpu={} ready_depth={}",
        SCENARIO, cpu, depth
    );
}
```

### Thread.cpu_id tracking

Add to `wake_thread_front`:
```rust
serial_println!(
    "NEO_TEST/1 event=WAKE_CPU id={} old_cpu={} new_cpu={}",
    thread.id.0, thread.cpu_id.load(Ordering::Relaxed), cpu_id
);
thread.cpu_id.store(cpu_id, Ordering::Relaxed);
```

### Lock contention stats

Add to SwitchMutex's `lock` method:
```rust
let mut spin_count = 0u32;
while self.lock.compare_exchange_weak(...) {
    while self.lock.load(Ordering::Relaxed) {
        spin_loop();
        spin_count += 1;
    }
}
if spin_count > 1000 {
    serial_println!(
        "NEO_TEST/1 event=LOCK_SPIN mutex={} count={}",
        mutex_name, spin_count
    );
}
```

### NMI watchdog usage

**Recommendation:** Apply `stash@{0}` (NMI watchdog from wedge investigation) for this investigation.

**NMI threshold:** 60 seconds (matches the wedge investigation). This converts silent hangs into panics with stack traces from all 4 CPUs.

---

## 4. Phase C — Reproduction predictions

### Reproduction probability

- **With H7 instrumentation (thread extraction tracking):** 80% probability the failure reproduces in the next 50 runs (H7 is contention-amplified)
- **Without instrumentation:** 20% probability in next 50 runs (same underlying rate, but no data capture)
- **200 runs:** >95% probability of reproduction with H7 instrumentation

### Per-hypothesis predicted signatures

**H1 (Lost wakeup):**
- Instrumented data shows: thread ID appears on ready queue twice with same return_value, or a parked thread is never extracted from EndpointState
- The woken thread's `blocked_return` slot has a value, but the thread never resumes

**H2 (Race in block_current_into_with_guard):**
- Instrumented data shows: thread's `rsp` field contains a stale value (from before the asm write)
- This would cause a crash or hang, but no PANIC would be emitted (stack pointer invalid → triple fault)

**H3 (Test scenario logic deadlock):**
- Instrumented data shows: iteration N where both sender and receiver are blocked on each other (both `EndpointState::SendBlocked` and `EndpointState::RecvBlocked`)
- The test orchestrator waits forever for `STRESS_SENDER_DONE[pair]` and `STRESS_RECEIVER_DONE[pair]` to reach seq

**H4 (Per-CPU ready queue starvation):**
- Instrumented data shows: one CPU's ready queue depth grows unbounded (e.g., >100) while others idle (depth ≈ 0)
- The busy CPU makes progress slowly; orchestrator on CPU 0 never sees all iterations complete

**H5 (SwitchMutex starvation):**
- Instrumented data shows: one CPU's lock acquisition spin_count >> others (e.g., 10000 vs 100)
- The starved CPU's scheduler lock acquisition is delayed, but eventually succeeds

**H6 (Asm helper missing memory barrier):**
- Instrumented data shows: random failures that don't correlate with any pattern (true race condition)
- The failure would be less reproducible than the observed 35/35 then 1 failure

**H7 (Thread extraction not atomic):**
- Instrumented data shows: for iteration N, both sender and receiver extract the same parked thread from EndpointState
- One push succeeds; the other is a duplicate (thread already on ready queue)
- The thread that should complete never resumes (either extracted twice with conflicting return_values, or not extracted at all)

### Heisenbug risk

**Risk:** Instrumentation adds serial output overhead, which could slow the test enough to avoid the contention window.

**Mitigation:** 
- Use conditional emission only after iteration N (e.g., `if seq > 35 { serial_println!(...) }`)
- Keep instrumentation minimal (only essential fields)
- Test with and without instrumentation to verify timing isn't perturbed

---

## 5. Phase D — Root-cause hypothesis ranking

### Most likely hypothesis

**H7: Thread extraction from EndpointState not atomic under contention**

**Confidence:** 70% confident

**Rationale:** 
- Matches failure signature (silent hang after BEGIN, 35/35 then 1 failure)
- Stress test creates high contention (4 pairs × 1000 iterations = 4000 rendezvous)
- Q4 wake extracts parked thread from EndpointState; under contention, two completers could both extract the same thread
- The thread that should complete never resumes (either extracted twice with conflicting state, or not extracted at all)

**What would change my mind:**
- Instrumentation shows the thread is extracted exactly once and pushed exactly once to a ready queue, but still hangs
- Instrumentation shows all ready queues are empty and no threads are blocked in EndpointState (suggests a different bug)

### Top-3 ranked

1. **H7 (70%):** Thread extraction not atomic under contention
2. **H4 (20%):** Per-CPU ready queue starvation
3. **H1 (10%):** Lost wakeup in Q4 wake-on-completer-CPU

### Confidence assessment

- **H7:** 70% — Strong match to failure signature and contention pattern
- **H4:** 20% — Plausible but less direct match (would show queue depth imbalance)
- **H1:** 10% — H7 is a more specific formulation of the same concern

---

## 6. Phase E — Fix design (top-2 hypotheses)

### Fix for H7 (Thread extraction not atomic under contention)

**Code shape:**
- File: `kernel/src/sched/scheduler.rs`
- Change: Add atomic extraction from EndpointState that returns a boolean indicating whether this CPU successfully extracted the thread

**Semantics change:**
- Current: `wake_thread_on_current_cpu` takes a `Box<Thread>` directly (caller must have extracted it)
- New: Add `try_extract_parked_thread` that atomically removes the thread from EndpointState and returns `Option<Box<Thread>>`
- Callers of `wake_thread_on_current_cpu` must use `try_extract_parked_thread` and handle the "already extracted" case

**New invariant:**
- A parked thread can only be extracted once from EndpointState
- The first extractor wins; subsequent extractors return `None`
- The first extractor is responsible for pushing the thread to a ready queue

**Verification:**
- Instrumentation shows each parked thread is extracted exactly once
- The stress test passes 200/200 with the fix
- The thread's `blocked_return` slot is written exactly once

**Anti-regression:**
- Existing single-CPU IPC scenarios continue to work (they don't have contention)
- The 5 supplementary 50-run loops continue to pass

**/simplify gate:**
- This is the minimal fix (add atomic extraction)
- A larger fix (e.g., redesign EndpointState to be lock-free) is overkill for this bug

---

### Fix for H4 (Per-CPU ready queue starvation)

**Code shape:**
- File: `kernel/src/sched/scheduler.rs`
- Change: Add load balancing to `wake_thread_on_current_cpu` — push to the least-loaded CPU instead of the completer's CPU

**Semantics change:**
- Current: Q4 wake pushes to completer's CPU (consumer-first for same-CPU handoff)
- New: Q4 wake pushes to the least-loaded CPU (balancing under contention)
- Same-CPU handoff invariant is relaxed to "prefer completer's CPU if load is balanced"

**New invariant:**
- The completer's CPU is preferred for handoff if its ready queue depth is below threshold (e.g., <8)
- If the completer's CPU is overloaded (queue depth >= threshold), push to the least-loaded CPU

**Verification:**
- Instrumentation shows balanced queue depths across CPUs
- The stress test passes 200/200 with the fix
- Same-CPU handoff latency is slightly higher under contention, but throughput is better

**Anti-regression:**
- Single-CPU IPC scenarios continue to work (they don't have contention)
- The 5 supplementary 50-run loops continue to pass

**/simplify gate:**
- This fix is more invasive (changes Q4 semantics)
- H7 is the minimal fix; H4 is a performance improvement that happens to fix the hang

---

## 7. Risks and unknowns

1. **Unknown:** The exact location of the race (H7) — is it in `wake_thread_on_current_cpu` or somewhere else? Instrumentation is needed to pinpoint.

2. **Unknown:** Whether the test orchestrator (CPU 0) is waiting for progress or the busy CPU is stuck. NMI watchdog stack traces would answer this.

3. **Unknown:** Whether the failure is reproducible with instrumentation enabled (Heisenbug risk). Mitigation: conditional emission only after iteration 35.

---

## 8. Cross-references

- `kernel/src/sync/switch_mutex.rs` — SwitchMutex implementation
- `kernel/src/sched/scheduler.rs` — `block_current_into_with_guard`, `wake_thread_on_current_cpu`
- `kernel/src/ipc/mod.rs` — 4 IPC retrofit sites
- `kernel/src/arch/x86_64/context.rs` — `context_switch_with_release2` asm
- `kernel/src/test_harness.rs` — stress test scenario
- `kernel/src/sched/thread.rs` — Thread struct, cpu_id field
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md` — PM's triage (H1-H6)
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` — ratified PLAN
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` — BUILD dispatch
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6

---

**End of investigation.**
