# SC-3B stress-gate failure investigation (Cerebro)

Date: 2026-05-09
Author: Cerebro
Status: Analysis complete; no code changes; no kernel execution

## 1. Substrate verification

This report is based on the SC-3B stress-gate failure artifact at:

- `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log`

Commit under test:

- `a61e318d93321c9e84732869f5de9da96abe3d5f`

Required context read:

- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md`
- `Plans/SC-3B-VERIFICATION-2026-05-09.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md`
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md`
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md`
- `docs/ADR-013-phase-2-smp-retrofit.md`

Relevant source inspected:

- `kernel/src/sync/switch_mutex.rs`
- `kernel/src/arch/x86_64/context.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/ipc/mod.rs`
- `kernel/src/sched/thread.rs`
- `kernel/src/test_harness.rs`

Constraints observed:

- No source code changed.
- No tests or kernel boots run.
- No stash interaction performed.
- No other agents' investigation outputs read.

Failure signature from run 036:

- Boot is clean through SMP startup.
- All 4 CPUs initialize.
- The default phase-0 ping/pong/hog smoke completes and reaps cleanly.
- Stress scenario emits only:

```text
NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
```

- No scenario `PASS`.
- No scenario `FAIL`.
- No `BUG:`.
- No `KERNEL PANIC`.
- No final frame-leak audit.
- Harness timeout at 180 seconds.

This rules out early boot failure and points at an in-kernel liveness loss during the stress scenario.

## 2. Phase A: code-path audit

### 2.1 SwitchMutex

`SwitchMutex<T>` is the lock type that enables cross-stack unlock after context switch:

- Lock byte is an `AtomicBool`.
- Normal lock path uses `compare_exchange_weak(false, true, Acquire, Relaxed)`.
- Unlock path stores `false` with `Release`.
- `release_token()` exposes a pointer to the lock byte.

This matches the intended minimal ownership model. No third-party lock layout is depended on.

The risk is not the basic lock algorithm. The risk is observability and ownership at failure time:

- There is no owner CPU/thread tracking.
- There is no contention counter.
- There is no max-spin counter.
- A leaked lock byte is currently indistinguishable from a highly contended lock in post-mortem output.

### 2.2 Assembly release path

`context_switch_with_release2` saves the old stack, switches to the next stack, releases the channel lock, releases the scheduler lock, then restores callee-saved registers:

```asm
push rbx
push rbp
push r12
push r13
push r14
push r15
mov [rdi], rsp
mov rsp, rsi
mov byte ptr [rdx], 0
mov byte ptr [rcx], 0
pop r15
pop r14
pop r13
pop r12
pop rbp
pop rbx
ret
```

The release order is correct for SC-3B:

- Channel lock first.
- Scheduler lock second.

The old-stack metadata failure mode from the SC-3B THINK stage is not present: the release pointers are passed in registers and consumed after `rsp` changes.

The strongest H2-style concern is narrower: whether the saved `prev_rsp_ptr` remains valid if the parked thread is extracted by a peer immediately after the channel lock releases. Source audit reduces this risk. The asm writes `prev.rsp` before releasing the channel lock, and the parked `Thread` allocation is heap-pinned through `Box<Thread>`. The raw pointer points into the `Thread` object, not into the containing `Box` slot. That makes this less likely than the scheduler/rendezvous hypotheses.

### 2.3 Scheduler block path

`block_current_into_with_guard` does the load-bearing SC-3B transition:

- Runs inside `without_interrupts`.
- Locks the current CPU scheduler.
- Pops the next ready thread.
- Replaces current with next.
- Marks previous thread `Blocked`.
- Marks next thread `Running`.
- Calls the IPC installer while holding the channel guard.
- Captures channel and scheduler release tokens.
- Forgets both guards.
- Calls `context_switch_with_release2`.
- Returns the resumed thread's `blocked_return`.

Important ordering:

- Blocked thread state is installed before the cross-stack unlock.
- Saved stack pointer is written before channel unlock.
- The channel lock is released before the scheduler lock.

This is the intended mechanism. The remaining failure surface is liveness, not obvious memory corruption:

- The parked thread may not be woken.
- The woken thread may be queued on a CPU that does not run it.
- A lock byte may stay held after a failed transition.
- The coordinator may stop running and leave workers in unbounded go-wait loops.

### 2.4 Scheduler wake/yield behavior

`wake_thread_on_current_cpu` routes to `wake_thread_front`:

- Marks target thread `Ready`.
- Writes `blocked_return`.
- Sets `preempt_defer_ticks = HANDOFF_PREEMPT_DEFER_TICKS`.
- Locks current CPU scheduler.
- Stamps `thread.cpu_id` to the current CPU.
- Pushes the thread to the front of that CPU's ready queue.

`handoff_to_rendezvous_peer` wakes the peer and calls `yield_now`.

`continue_current_after_receive` wakes the peer and defers current preemption instead of yielding immediately.

This split is intentional, but it creates a diagnostic need. On failure we must distinguish:

- Peer was never woken.
- Peer was woken but queued on the wrong CPU.
- Peer was queued correctly but the CPU never scheduled it.
- Current thread deferred preemption and starved progress.

Current serial output cannot distinguish these.

### 2.5 IPC rendezvous paths

The stress scenario exercises channel send/receive only, not grant:

- Sender blocks with `EndpointState::SendBlocked`.
- Receiver later observes the blocked sender, copies payload, wakes sender, and returns.
- Receiver blocks with `EndpointState::RecvBlocked`.
- Sender later observes the blocked receiver, copies payload, wakes receiver, and returns.

The production path does not currently guard against multiple waiters on the same endpoint overwriting a same-side blocked state. That is a real future correctness risk. It is less likely to be the run-036 root cause because the stress scenario creates one sender and one receiver per channel pair, and the coordinator serializes each pair's go/done sequence.

Still, instrumentation should assert endpoint overwrite discipline during stress:

- Installing a `SendBlocked` state should see the same endpoint `Idle`.
- Installing a `RecvBlocked` state should see the same endpoint `Idle`.
- Completing a blocked peer should clear exactly the peer generation being completed.

### 2.6 Stress scenario

`smp_cross_cpu_rendezvous_stress` is much more complex than the prior one-shot smoke scenarios:

- It detects ready SMP CPUs.
- It creates up to 4 independent channel pairs.
- It places sender and receiver workers on different CPUs.
- It runs 1000 rendezvous iterations.
- It alternates send-first and receive-first ordering.
- It uses static per-pair fixtures and atomics for orchestration.
- Worker `wait_for_stress_go` loops are unbounded and emit no serial progress.
- Coordinator wait loops are bounded and should emit `FAIL` on local timeout.

The silent failure is important: the coordinator did not reach its bounded failure path. If it had kept executing normally, it should have emitted a structured FAIL for endpoint-block wait, iteration completion wait, or worker-exit wait. Therefore the most useful next instrumentation is not only "which endpoint is blocked" but also "is the coordinator still running."

## 3. Phase B: instrumentation strategy

### 3.1 Instrumentation principles

Use a throwaway branch or stash. Do not commit investigation instrumentation.

Prefer in-memory ring buffers over high-volume serial logging. The stress race is timing-sensitive; serial per iteration may mask it. Serial should be used for low-frequency progress and for final dump paths.

Recommended structure:

- Add a fixed-size per-CPU debug ring for stress and scheduler events.
- Emit serial progress every 64 or 128 stress iterations.
- Emit full state dump if a bounded watchdog fires.
- Add optional NMI dump only after the lighter instrumentation reproduces or fails to reproduce.

### 3.2 Stress-test progress frames

Add progress visibility at the coordinator level. The minimum useful structured frames:

```text
NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress phase=setup cpus=<n> pairs=<n>
NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress phase=iter_start iter=<seq> pair=<p> send_first=<0|1>
NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress phase=first_blocked iter=<seq> pair=<p> side=<send|recv>
NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress phase=second_released iter=<seq> pair=<p> side=<send|recv>
NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress phase=iter_done iter=<seq> pair=<p> sender_done=<n> receiver_done=<n>
NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress phase=workers_exit exited=<n> expected=<n>
```

For low perturbation, only print `iter_start` and `iter_done` every 64 iterations by default, but record every iteration in the in-memory ring.

Add worker-side progress into the ring buffer:

```text
worker=<sender|receiver> pair=<p> seq=<seq> phase=wait_go|go_seen|before_ipc|after_ipc|done_store|exit cpu=<cpu>
```

The most important field is the last completed coordinator iteration. If the next timeout again prints only the dispatch frame, this immediately distinguishes setup hang from mid-iteration hang.

### 3.3 Scheduler snapshots

Add a debug-only scheduler snapshot helper for all CPUs:

```text
cpu=<id> current_tid=<tid> current_name=<name> current_state=<state> current_preempt_defer=<n> ready_len=<n> zombie_count=<n> lock_held=<0|1>
```

Useful dump points:

- Before every stress worker block.
- After every stress worker wake.
- On coordinator bounded-wait failure.
- On watchdog/NMI dump.

If possible, include ready queue front thread id/name without mutating the queue.

### 3.4 IPC endpoint snapshots

Add debug-only channel snapshot for each stress fixture:

```text
pair=<p> side_a=<Idle|SendBlocked|RecvBlocked|GrantBlocked|RecvGrantBlocked> side_a_tid=<tid|none> side_a_cpu=<cpu|none> side_b=<...>
```

Dump at:

- Coordinator iteration start.
- After first endpoint-block detection.
- After second side release.
- On watchdog/NMI dump.

Add generation numbers for investigation only:

- Increment when installing a blocked endpoint.
- Copy generation into the blocked state.
- Log generation when completing/waking.

This distinguishes "blocked state never observed" from "blocked state overwritten" from "wake completed stale state."

### 3.5 SwitchMutex diagnostics

For investigation only, extend `SwitchMutex` with debug counters or a side table:

- Owner CPU.
- Owner thread id if available.
- Last acquire event id.
- Last release event id.
- Contended acquire count.
- Max spin count.
- Current lock byte.

The critical dump question is:

- Is the channel lock held forever?
- Is the scheduler lock held forever?
- Which CPU/thread last acquired it?

If adding fields directly changes layout too much, use a separate debug registry keyed by lock address.

### 3.6 Context-switch boundary checks

Instrument `block_current_into_with_guard` around the unsafe switch:

- Before installer: previous thread id, current CPU, next thread id, ready depth.
- After installer: endpoint state, previous state, saved rsp address.
- Immediately before asm: channel lock pointer, scheduler lock pointer, prev rsp pointer, next rsp value.
- On resume: current thread id, return value, current CPU.

Add debug assertions:

- `prev_rsp_ptr` is non-null.
- `next_rsp` is canonical and non-zero.
- Channel and scheduler release tokens are distinct.
- Current thread state is `Running` after resume.
- The resumed thread id matches the thread whose stack is running if thread-local identity exists.

### 3.7 NMI watchdog recommendation

Use NMI watchdog only as the second investigation pass.

Recommended threshold:

- Fire after 10 seconds with no stress progress frame or no stress progress counter advance.
- Keep the existing 180 second QEMU timeout as the outer guard.

Reasoning:

- A 10 second no-progress interval during a 1000-iteration in-kernel stress scenario is already abnormal.
- It is short enough to capture the stuck state before unrelated timeout noise.
- It avoids relying on the final QEMU timeout, which destroys the in-kernel state.

NMI dump should print:

- Per-CPU current thread and ready queue length.
- Per-CPU scheduler lock status.
- Stress coordinator iteration/pair/phase.
- All stress fixture endpoint states.
- All stress GO/DONE atomics.
- SwitchMutex owner/spin diagnostics for stress channel locks and scheduler locks.

## 4. Phase C: reproduction predictions

### H1: lost wakeup or broken rendezvous transition

Prediction:

- Coordinator reaches an iteration where one side is confirmed blocked.
- Second side runs and should wake the peer.
- Endpoint state clears or changes, but the peer never stores DONE.
- Scheduler snapshot shows peer either not present in any ready queue, or present but never running.

Confirming evidence:

- Wake event logged for thread X, but no resume event for X.
- Endpoint generation completed, but blocked thread remains `Blocked`.
- `SENDER_DONE` or `RECEIVER_DONE` stuck one behind.

Refuting evidence:

- Both worker threads complete the iteration and store DONE, but coordinator does not observe it.
- Coordinator stops before any endpoint block/release sequence.

### H2: raw pointer or assembly boundary corruption

Prediction:

- Failure happens immediately around `context_switch_with_release2`.
- Saved rsp is zero, non-canonical, or does not match the resumed thread.
- Thread state or stack metadata corrupts without a normal endpoint/scheduler explanation.

Confirming evidence:

- Pre-asm event exists without matching resume/wake state.
- Saved rsp pointer points outside the expected `Thread` allocation.
- Resume occurs on the wrong thread object or with impossible state.

Refuting evidence:

- Failure shows all thread/stack identities coherent, with the hang instead due to ready queue or endpoint state.

Current audit result:

- Less likely than H1/H3/H5 because asm writes `prev.rsp` before releasing the channel lock and does not read old-stack metadata after `rsp` changes.

### H3: stress orchestration deadlock or silent worker wait

Prediction:

- Coordinator stops advancing before it reaches a bounded failure path.
- One or more workers sit in `wait_for_stress_go`.
- GO/DONE atomics show a mismatch that no bounded coordinator loop reports.
- The last progress event is `iter_start`, `first_blocked`, or setup.

Confirming evidence:

- Coordinator current thread is not running or is stuck in `yield_now`/`reap`.
- Workers are alive and yielding in unbounded `wait_go`.
- No endpoint-level lost wakeup is present for the current pair.

Refuting evidence:

- Coordinator continues running and reaches a specific endpoint or done wait, but a worker is blocked in IPC.

### H4: ready-queue starvation or CPU placement bug

Prediction:

- A required thread is `Ready` on a CPU queue but never becomes `Running`.
- Ready queue length remains non-zero while CPU current thread id does not change.
- Woken peer is stamped to an unexpected CPU.

Confirming evidence:

- Scheduler snapshot shows ready work outstanding on a CPU that is looping in AP bootstrap or idle-like yield.
- `thread.cpu_id` changes to current completer CPU, but the thread is never selected.

Refuting evidence:

- No stuck thread is ready; the thread is still blocked or the coordinator is gone.

### H5: SwitchMutex starvation or leaked lock

Prediction:

- One CPU spins on a channel or scheduler lock indefinitely.
- Lock byte remains true.
- Owner tracking identifies a thread/CPU that is no longer running or already switched away.

Confirming evidence:

- NMI snapshot shows same lock address held across all dumps.
- Spin counters grow while owner does not change.
- The leaked lock is one of the two release tokens passed through `context_switch_with_release2`.

Refuting evidence:

- No lock remains held; all lock acquisitions progress; stuck state is endpoint/queue logic.

### H6: missing memory ordering

Prediction:

- Atomic GO/DONE values or endpoint state appear inconsistent across CPUs without matching lock/transition logs.
- Adding stronger fences changes the failure rate materially.

Confirming evidence:

- Worker stores DONE but coordinator repeatedly reads an older value despite Acquire/Release discipline and continued execution.

Refuting evidence:

- Failure is explained by a concrete stuck thread, stuck lock, or missing wake.

Current audit result:

- Least likely. x86 TSO and the current atomic patterns make a pure fence issue unlikely.

### H7: remote-reschedule or timer asymmetry

Prediction:

- A wake is placed onto a CPU that does not promptly schedule it.
- Timer/preemption behavior differs between BSP and APs.
- Ready queue has work, but the running thread on that CPU does not yield or get preempted.

Confirming evidence:

- Peer is ready on a remote CPU, CPU continues running the same non-yielding thread, and timer interrupts are absent or not causing scheduling.

Refuting evidence:

- Required peer is never ready, or all CPUs are spinning on locks, or coordinator is blocked in orchestration.

## 5. Phase D: root-cause ranking

### 1. H3: stress orchestration/liveness hole

Confidence: 30 percent

Why:

- The only serial line from the scenario is the dispatch BEGIN.
- The coordinator's own waits are bounded and should emit FAIL if it keeps running.
- Worker `wait_for_stress_go` loops are unbounded and silent.
- The stress scenario is the first test that combines 4 CPUs, 4 channel pairs, placement, 1000 iterations, alternating ordering, static fixtures, and repeated yield/reap loops.

What would change my mind:

- A progress frame proves the coordinator reaches an iteration and a specific rendezvous endpoint gets stuck after the peer wake should occur.

### 2. H1: lost wakeup or rendezvous transition failure

Confidence: 25 percent

Why:

- The failure appears only under high rendezvous contention.
- The SC-3B mechanism directly changed the block/unlock/wake path.
- A missed wake naturally produces silent in-kernel hang if the coordinator then waits for DONE and stops getting scheduled.

What would change my mind:

- Endpoint generations and wake/resume logs show every rendezvous completes, and the stuck state is entirely in GO/DONE orchestration.

### 3. H5: SwitchMutex leaked lock or pathological contention

Confidence: 18 percent

Why:

- SC-3B introduced a new lock release mechanism.
- A leaked channel or scheduler lock can stall all relevant progress without panic.
- Current artifacts have no lock owner or spin diagnostics.

What would change my mind:

- NMI/debug snapshots show no lock held indefinitely and no growing spin counters.

### 4. H4: ready-queue starvation or placement bug

Confidence: 12 percent

Why:

- Q4 intentionally wakes peers onto the completer's current CPU.
- Stress creates more cross-CPU placement and migration pressure than one-shot tests.
- A ready-but-never-run thread would match timeout without panic.

What would change my mind:

- Snapshots show no stuck ready threads and all queues drain normally until the endpoint/orchestration stall.

### 5. H2: raw pointer or assembly boundary corruption

Confidence: 8 percent

Why:

- The assembly boundary is new and load-bearing.
- A corruption here can produce arbitrary liveness loss.

Why lower:

- Source ordering writes the saved rsp before channel unlock.
- Release pointers are registers, not old-stack metadata.
- `Thread` storage is heap-pinned.

### 6. H7: remote-reschedule/timer asymmetry

Confidence: 5 percent

Why:

- No IPI is used for remote scheduling in this phase.
- AP scheduler/timer behavior is still young.

Why lower:

- Stress workers explicitly yield between iterations.
- Prior SMP smoke and 50-run supplementary loops passed.

### 7. H6: missing memory fence

Confidence: 2 percent

Why:

- No artifact indicates stale data or reordering.
- x86 TSO makes this a weak primary explanation.
- Adding fences before observing state would be a phantom-win risk.

## 6. Phase E: fix design for top hypotheses

### Fix path for H3: stress scenario liveness hardening

If investigation confirms the scenario deadlocks independently of a kernel rendezvous bug, fix the test harness first.

Candidate changes:

- Replace worker `wait_for_stress_go` unbounded loops with bounded waits.
- Add a global `STRESS_ABORT` atomic so any worker/coordinator failure causes all workers to exit.
- Emit a structured FAIL when a worker waits too long for GO.
- Emit setup and periodic progress frames so future failures are not silent.
- Record the current coordinator iteration and pair in atomics for watchdog/NMI dumps.
- Add per-pair generation numbers for GO/DONE so stale sequence observations are diagnosable.

Verification:

- Re-run `smp_cross_cpu_rendezvous_stress` NEO_SMP=4 for the full closure count.
- Preserve the stress intensity: 4 CPUs, 4 pairs, 1000 iterations.
- Add a meta-test or forced-abort mode only if it can prove the harness reports FAIL instead of timing out.

Simplify gate:

- Do not add a general stress framework if a small bounded-wait/abort mechanism solves this scenario.
- Do not reduce iteration count to make the failure disappear.
- Do not accept "timeout gone" without a PASS or explicit structured FAIL path.

### Fix path for H1: rendezvous transition hardening

If investigation confirms a missed wake or stuck blocked endpoint, fix the kernel path, not the stress scenario.

Candidate changes:

- Add debug assertions that installing a blocked endpoint only happens when the same endpoint is `Idle`.
- Add endpoint generation tracking in debug builds to prove blocked state completion is one-to-one.
- Make wake helpers return or record a transition result: `Blocked -> Ready`, previous CPU, target CPU, queue depth after enqueue.
- Add a post-wake invariant in debug stress builds: woken thread is either current, ready in some scheduler queue, or already exited.
- If ready-but-not-run is proven, add a targeted scheduler fix rather than changing IPC semantics.

Possible production fixes depending on evidence:

- Reject or queue same-endpoint multiple waiters instead of overwriting endpoint state.
- Strengthen `wake_thread_on_current_cpu` to assert the target thread was actually `Blocked`.
- Add a rendezvous-specific handoff path that records peer enqueue success before returning to the completer.
- If remote ready queues are implicated, introduce the minimum reschedule signal required by ADR-013 rather than broad load balancing.

Verification:

- Re-run the exact failing stress closure.
- Re-run the four supplementary SC-3B loops.
- Re-run full default `cargo neo-test` at NEO_SMP=4.
- Add a focused regression scenario only if it reproduces the concrete failure mechanism, not merely the broad stress workload.

Simplify gate:

- Do not add fences unless logs prove stale visibility.
- Do not move back to scheduler-first unlock; that reopens Q4 latency risk.
- Do not add global scheduler locking; that would erase the per-CPU scheduler direction of ADR-013.

### Fix path for H5: lock ownership or release-token failure

If a lock is leaked:

- First prove which lock byte remains true.
- Then verify whether its guard was forgotten and release-token path was supposed to release it.
- If the assembly release stores are correct but owner tracking says the lock remained held, inspect pointer identity and lifetime.

Candidate fixes:

- Add debug-only token validation before `mem::forget`.
- Wrap release tokens in a small typed pair for channel/scheduler to reduce accidental pointer mixups.
- Keep the production ABI at `context_switch_with_release2` unless evidence requires generalization.

Avoid:

- Replacing `SwitchMutex` with `spin::Mutex`.
- Depending on third-party internal lock layout.
- Adding a const-generic release-list loop that reads metadata after stack switch.

## 7. Risks and unknowns

The current artifact lacks the decisive state:

- Last coordinator iteration is unknown.
- Current CPU/thread snapshots at timeout are unknown.
- Endpoint states at timeout are unknown.
- Scheduler ready queue contents at timeout are unknown.
- Lock ownership at timeout is unknown.

Main investigation risk:

- Heavy serial instrumentation may mask the race. Use memory rings and periodic progress frames first.

Main interpretation risk:

- A stress harness liveness bug and a real kernel wakeup bug can look identical from the current serial log. The next run must capture both coordinator progress and kernel scheduler/IP addressable state.

Important non-conclusion:

- The failure does not justify changing Q6 back to scheduler-first release, adding fences, or replacing the lock type without new evidence. The observed failure is liveness under stress, not a demonstrated memory-ordering or ABI-shape failure.

## 8. Cross-references

- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md`
- `Plans/SC-3B-VERIFICATION-2026-05-09.md`
- `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log`
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md`
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md`
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md`
- `docs/ADR-013-phase-2-smp-retrofit.md`
- `kernel/src/sync/switch_mutex.rs`
- `kernel/src/arch/x86_64/context.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/ipc/mod.rs`
- `kernel/src/test_harness.rs`
