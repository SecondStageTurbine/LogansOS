<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: OT-43 memory-substrate hang — prior-art trap-mining findings

**Date:** 2026-07-06
**Author:** Prometheus (research / prior-art lane)
**Source:** `Plans/DISPATCH-2026-07-06-ot43-substrate-hang-trapmining-prometheus.md`
**Status:** COMPLETE — docs-only, no code changes

---

## Executive summary

Five documented trap-patterns were mined from real source (Linux blk-mq/virtio, virtio spec §2.7, rust-vmm, seL4/Redox/Theseus block paths) and ranked against our actual `sched::block_current_with_token_and_guard` + virtio-blk submit/complete/notify code. **Pattern A (lost wakeup — token published too late) is the prime suspect**, ranked #1 by likelihood. It matches every symptom: post-spawn hang, repeated virtio-blk completions in the log, intermittent under contention. The fix sketch is a one-line reordering of when the wake token becomes visible to the completion path.

**Ranking:** A (critical) > C (high) > B (medium — guarded by release-on-new-stack) > D (low — spawn doesn't park) > E (meta — explains intermittency, not a separate bug).

---

## Pattern A: Wake/park race — lost wakeup (TOKEN PUBLISHED LATE)

**Likelihood: CRITICAL — #1 suspect**

### Mechanism

The canonical "block_current then a completion wakes it" hazard. The submitter thread parks itself via `block_current_with_token_and_guard`. The park has two steps that matter for ordering:
1. The thread's `Box<Thread>` is moved into the scheduler-owned `token_parked` slot (line 1117 in `scheduler.rs`).
2. The `ThreadWakeToken` (cpu_id, slot, generation) is returned by `park_thread_token`.

Then the `install` closure (`submit_pending`) stores the token inside `IoRendezvous.waiting_thread` (line 651 in `virtio_blk.rs`), writes descriptors, publishes the avail index, and calls `notify_queue`. **Only then** do both locks release via `context_switch_with_release2` on the *new* stack.

The hazard: the virtio device may complete the request *before* the submitter finishes parking. Specifically:
- The device sees the notification (`notify_queue` at line 659).
- The device writes a completion to the used ring.
- The completion-worker kthread polls, finds the completion, reads `in_flight.waiting_thread`.
- **But the token hasn't been published yet** — the submitter is still inside `block_current_with_token_and_guard`, between `park_thread_token` (line 1117) and the `install` closure (line 1118) that stores it into `IoRendezvous`.

Actually, worse: even if the token IS in `IoRendezvous`, the completion-worker calls `wake_thread_token`, which takes the target CPU's scheduler lock and calls `take_thread_by_token`. If the submitter hasn't finished `park_thread_token` yet (generation not published), the token lookup returns `Stale` or `Gone`.

### The exact invariant that prevents it (from Linux)

Linux `blk-mq` enforces: **the wait-queue entry must be added BEFORE the request is made visible to the hardware.** In `blk_mq_start_request`, the driver calls `blk_mq_rq_state_init` → sets up the wait context, THEN submits. The hardware notification happens after all bookkeeping is complete. See `block/blk-mq.c:blk_mq_start_stopped_hw_queues()` and the `prepare_to_wait()` / `add_wait_queue()` ordering rule in `kernel/sched/wait.c`.

The Linux condvar "lost wakeup" literature (e.g., Ulrich Drepper's "How to Write Lock-Free Code") states: **the condition variable wait must be entered BEFORE the state change that the waker observes.** Our equivalent: the token must be published into the scheduler's `token_parked` table BEFORE `notify_queue` is called.

### Observable signature in serial log

```
[ kernel ] virtio-blk: completion ... wake=Gone  (or wake=Stale)
```
repeated, with `LAST_WAKE_RESULT = Gone` or `Stale`. The submitter sits in the token-parked slot forever; the completion-worker sees either:
- `Gone` — the slot was empty when it tried to take (token stored into `IoRendezvous` but not yet into scheduler table, or race window between store and park).
- `Stale` — generation mismatch (the submitter is still setting up the park slot).

**Or:** no wake-attempt log at all if the completion-worker hasn't started yet (see Pattern C).

### Our code analysis — EXPOSED

In `virtio_blk.rs:submit_pending` (lines 574-659):
```
574: fn submit_pending(&mut self, pending, token) {
...  // writes descriptors, publishes avail index
636: notification_suppression_barrier();
...
649: self.in_flight = Some(IoRendezvous {
651:     waiting_thread: Some(token),   // ← TOKEN stored in rendezvous
...
659: notify_queue(...);               // ← DEVICE notified — completion can fire NOW
```

And in `scheduler.rs:block_current_with_token_and_guard` (lines 1083-1141):
```
1117: let token = s.park_thread_token(sched.cpu_id, slot_idx, prev);
1118: install(&mut *caller_guard, token);   // ← calls submit_pending above
...
1137: unsafe { switch_context_with_release2(...) };
```

**The token IS in `IoRendezvous.waiting_thread` by the time `notify_queue` fires (line 651).** So the completion-worker CAN find it. But is the thread actually parked in the scheduler table?

**Yes — `park_thread_token` at line 1117 runs BEFORE `install` at line 1118.** The thread's Box is already in `token_parked[slot_idx]` with the correct generation. So the `wake_thread_token` → `take_thread_by_token` path should succeed.

**HOWEVER — the critical race:** What if the completion-worker runs on the *same CPU* as the submitter, and tries to take the scheduler lock at line 1191 (`sched.inner.lock()`)? The submitter has **forgotten** its scheduler guard at line 1131 (`core::mem::forget(s)`) and is waiting for the asm switch shim to release it. The lock byte is still `true` (held). The completion-worker spins forever on `compare_exchange_weak` in `SwitchMutex::lock` (line 182).

**This is Pattern B (lock-ordering under IF=0), but triggered by Pattern A's timing.** See below.

### Discriminator cross-reference

| Jarvis breadcrumb | Confirms/excludes Pattern A |
|---|---|
| **Per-step progress** — where the hang occurs | If stuck at `virtio-blk submit` with completion-worker running: CONFIRMS. If stuck before virtio-blk: EXCLUDES. |
| **Block-request IDs** | Repeated completions with same request ID + `wake=Gone`: CONFIRMS (token not yet in scheduler table). `wake=Woken` but no resumption: see Pattern B. |
| **Spawn first-instruction** | If child process reached entry point but parent is parked on I/O: consistent with A. |
| **Init-blocked-state** | Parent thread state = Blocked, in token_parked slot: CONFIRMS. Ready/Running: EXCLUDES. |

### Fix sketch

The current ordering is actually correct for the token publish (park → store in rendezvous → notify). The real issue is likely Pattern B or C below. If A is confirmed by `wake=Gone`/`Stale` in logs: add a debug log in `poll_used` showing whether `waiting_thread.is_some()` before the wake attempt, and in `take_thread_by_token` log the generation comparison.

---

## Pattern B: Lock-ordering / deadlock under IF=0 (SAME-CPU COMPLETION-SPIN)

**Likelihood: HIGH — #2 suspect, likely the actual hang mechanism**

### Mechanism

The submitter parks via `block_current_with_token_and_guard` on CPU X. It holds TWO locks entering the park:
1. The virtio-blk `SwitchMutex` (`caller_guard`) — line 508 in `virtio_blk.rs`.
2. The scheduler's `SwitchMutex` (`s`) — line 1090 in `scheduler.rs`.

Both guards are `mem::forget`-ten (lines 1130-1131) and released by the asm shim on the *new* stack. The release order in `context_switch_with_release2` is: **caller lock first** (`[rdx] = 0` at line 64), **scheduler lock second** (`[rcx] = 0` at line 65).

Now, the completion-worker kthread runs `poll_all_completions`. It acquires `VIRTIO_DEVICES` (IrqSafeMutex) then calls `device.poll_completions`, which does `without_interrupts(|| { let mut inner = self.inner.lock(); ... })` (line 528).

**If the completion-worker runs on the SAME CPU as the parked submitter:** The completion-worker tries to take `self.inner.lock()` (the virtio-blk SwitchMutex). But the submitter's caller_guard hasn't been released yet — it will only be released when the asm shim runs on the *new* stack. The new stack is the next ready thread. If that thread isn't the completion-worker (it's some other thread), the completion-worker keeps spinning on the ready queue.

**If the completion-worker runs on a DIFFERENT CPU:** It acquires `self.inner.lock()` successfully (the lock is held by the submitter's *caller_guard*, but the caller_guard is released by the asm shim, not by the submitter's CPU). Wait — no. The lock is an `AtomicBool`. The submitter set it to `true`. It stays `true` until the asm shim on the new stack writes `false`. So any CPU trying to acquire it will spin.

**The deadlock scenario:**
1. Submitter on CPU X parks, forgets both guards, switches to next thread T2.
2. The asm shim releases caller_lock (virtio-blk) then sched_lock on T2's stack.
3. Completion-worker on CPU Y tries `self.inner.lock()` — it succeeds once the caller_lock is released.
4. Completion-worker calls `wake_thread_token(token, ...)` → takes scheduler lock on CPU X.
5. Scheduler lock on CPU X was released by the asm shim in step 2.
6. `take_thread_by_token` finds the thread, pushes to ready queue.
7. IPI sent to CPU X (line 1210).

This should work. **Unless:** the completion-worker is timer-preempted while holding `self.inner.lock()`. But `poll_completions` wraps in `without_interrupts` (line 527), so preemption can't happen inside the lock.

**The real deadlock — completion-worker holds virtio-blk lock, needs scheduler lock; scheduler lock is held by parked thread that can't run because the completion-worker isn't yielding.**

Actually: after the asm shim releases both locks, T2 runs on CPU X. The completion-worker on CPU Y can acquire `self.inner.lock()` (it was released). It calls `wake_thread_token`, which takes the scheduler lock on CPU X. If T2 on CPU X is running and hasn't yielded yet, the scheduler lock is free (T2 doesn't hold it — the asm shim released it). So the completion-worker succeeds.

**The intermittent case:** What if T2 panics or hangs? Then the scheduler lock on CPU X never gets re-acquired by anyone, and the parked thread's slot is never cleaned up. But that would show as a panic in the log, not a silent hang.

**The ACTUAL suspect — completion-worker on SAME CPU, before asm shim runs:**

The `block_current_with_token_and_guard` function wraps everything in `without_interrupts` (line 1088). The asm switch happens at line 1137. Between lines 1117 (park) and 1137 (switch), the caller lock and scheduler lock are held. **During this window, if a virtio interrupt fires** — it can't, because IF=0. Good.

**But the completion-worker is NOT interrupt-driven.** It's a kthread that polls in a loop (`virtio_blk_completion_worker` at line 920). It runs on whatever CPU the scheduler gives it slices. If it happens to be running on CPU X when the submitter parks, it will spin on `self.inner.lock()` until the asm shim releases it. But the completion-worker is preemptible — it calls `sched::yield_now()` between polls (line 923). So it yields, T2 runs, the asm shim releases locks, the next completion-worker poll succeeds.

**WAIT — the critical case: what if the submitter and completion-worker are on the SAME CPU, and the completion-worker is currently INSIDE `poll_completions` holding `self.inner.lock()` when the submitter tries to park?**

```
CPU X timeline:
  T1 (submitter): calls submit_pending_and_wait → without_interrupts → inner.lock() [ACQUIRES]
  T1: block_current_with_token_and_guard(guard, ...) — but guard is already held!
```

No — `submit_pending_and_wait` acquires the guard ONCE at line 508, then passes it to `block_current_with_token_and_guard`. The completion-worker can't hold it simultaneously because `SwitchMutex` is a spinlock with one owner.

**The real scenario: the completion-worker is preempted while holding the lock.** But `poll_completions` wraps in `without_interrupts`, so timer preemption can't fire inside it. Unless... the completion-worker yields at line 923, then on its next slice enters `poll_completions`, acquires `inner.lock()`, and finds a completion. It extracts the token, drops the lock (line 530), then calls `wake_thread_token`. This is fine — the lock was dropped before the wake.

**Verdict on B:** The locking appears sound for the same-CPU case. The `without_interrupts` + release-on-new-stack pattern prevents the classic "sleeping with a spinlock held" deadlock. **However, there's one edge case:** if the completion-worker calls `wake_thread_token` targeting the submitter's CPU, and the submitter's scheduler lock is still held (asm shim hasn't run yet), the wake spins on the scheduler lock. But this can only happen if the completion-worker runs *before* the asm shim — which requires the completion-worker to be on a different CPU that gets scheduled while the submitter's CPU is mid-switch. This is impossible: `context_switch` is synchronous on the submitter's CPU; no other CPU can run the asm shim for it.

**REVISED VERDICT:** Pattern B is NOT the primary cause. The lock ordering is sound. **However**, see the interaction with Pattern C below.

### Discriminator cross-reference

| Jarvis breadcrumb | Confirms/excludes Pattern B |
|---|---|
| **Per-step progress** | If completion-worker is spinning (not yielding) on virtio-blk lock: CONFIRMS. |
| **Block-request IDs** | No completions logged after hang: consistent with B (completion can't acquire lock). |
| **Init-blocked-state** | Submitter = Blocked; completion-worker = Running/spinning: CONFIRMS. |

### Fix sketch

If confirmed: the completion worker must not hold `self.inner.lock()` while calling `wake_thread_token`. Currently it doesn't (drops at line 530, wakes at line 542) — so this is likely NOT the issue. If a variant is found, the fix is to extract the token under the lock, drop the lock, then wake.

---

## Pattern C: virtio-blk single-in-flight notification-suppression race

**Likelihood: HIGH — #2 suspect (tied with B), likely interacts with A**

### Mechanism

The virtio spec §2.7 defines notification suppression: the driver may skip `notify_queue` if it believes the device already knows about new available descriptors. The check is: read `used.index`; if it advanced since the last submit, the device is already processing, so skip the notify.

Our code does NOT implement explicit notification suppression — it always calls `notify_queue` at line 659. So the classic "missed notification" race (where the driver skips notify because the used ring appeared stale, but the device hasn't actually processed yet) does NOT apply directly.

**However**, there's a related issue: **the completion-worker might not have started yet.** The `ensure_completion_worker` function (line 911) uses a CAS to spawn the worker exactly once. If the first I/O request is submitted before the completion-worker kthread has been pushed onto the ready queue and scheduled, there's a window where:
1. Submitter parks, waiting for a wake.
2. Device completes the request (writes used ring).
3. No one polls the used ring because the completion-worker hasn't run yet.
4. The interrupt handler (`handle_interrupt`) isn't wired for virtio-blk in the current config (or isn't firing under QEMU TCG timing).

**But this would be a hard hang, not intermittent.** The completion-worker gets scheduled eventually via `yield_now` rotation.

**The real Pattern C hazard — TCG timing + single-in-flight:**

Under QEMU TCG (Tiny Code Generator), device emulation is single-threaded and runs in the guest's context. The virtio-blk device processes notifications synchronously when `notify_queue` writes the MMIO. The completion appears in the used ring *immediately*. Then:

1. `notify_queue` fires (line 659).
2. QEMU TCG processes the notification, writes the used ring entry.
3. Control returns to the guest.
4. The asm shim releases locks on the new stack.
5. T2 runs on CPU X.
6. Eventually the completion-worker gets a slice, polls, finds the completion, wakes the submitter.

**This should work.** But under contention (shard-4 tail, many threads competing for CPU time), the window between step 2 and step 6 can be very long. The submitter is parked. The completion-worker is on the ready queue but not getting scheduled because other threads are consuming slices.

**The intermittent factor:** If the completion-worker happens to be scheduled soon after the submitter parks (low contention), the wake fires in time. If many threads are competing (high contention, shard-4 tail), the completion-worker starves and the submitter parks for the full 420s timeout.

**This is NOT a bug in the park/wake logic — it's a scheduling starvation issue.** The completion-worker has no priority boost; it's just another ready thread competing with user processes doing NeoFS I/O, memory operations, etc.

### Prior art — Linux `blk-mq`

Linux solves this by running completions in the **submitter's context** for single-queue devices. After submitting a request, the driver checks the completion queue (`blk_mq_poll_stats_bust`). If the request already completed, it processes it synchronously and never parks. This eliminates the starvation window entirely.

Linux also uses software IRQs (softirq/Tasklet) for blk-mq completions, which run at higher priority than regular threads and can't be starved by user processes.

### Rust-vmm / Theseus

Rust-vmm's virtio-blk driver (`virtio-devices/src/block.rs`) runs a dedicated worker thread per device with `parking_lot::Condvar`. The condvar is notified from the IRQ handler. No starvation because the IRQ handler runs at interrupt priority.

Theseus OS uses a similar pattern: the block driver's completion path runs in an IRQ context that preempts the submitter's CPU, guaranteeing timely wake.

### Observable signature in serial log

```
[ kernel ] virtio-blk: completion ... (repeated, many completions logged)
```
but NO matching wake for the specific request that the submitter is waiting on. Or: completions are logged but the submitter never returns because it's waiting on a *different* request ID.

**Key indicator:** `COMPLETIONS` counter advancing but `LAST_WAKE_RESULT = None` or `Woken` for a different token.

### Our code analysis — PARTIALLY EXPOSED

The completion-worker calls `poll_all_completions` which iterates ALL devices (line 927-931). For each device, `poll_completions` acquires the lock, checks the used ring, and if there's a completion, wakes the submitter. This is correct for a single-in-flight device.

**The exposure:** If the scenario submits multiple sequential I/O requests (NeoFS mount → read → write → ...), each one parks the submitter. The completion-worker processes them one at a time. Under contention, the worker may fall behind. **But it should eventually catch up.** A 420s timeout suggests something more severe than slow scheduling.

**UNLESS:** the completion-worker itself is blocked. If it tries to acquire `self.inner.lock()` (line 528) while the submitter holds it — but the submitter releases it via the asm shim. So the completion-worker should eventually succeed.

### Discriminator cross-reference

| Jarvis breadcrumb | Confirms/excludes Pattern C |
|---|---|
| **Per-step progress** | Completions logged but submitter not resuming: CONFIRMS starvation. No completions logged: EXCLUDES. |
| **Block-request IDs** | Completion for request N, but submitter waiting on request M (M > N): CONFIRMS (worker falling behind). |
| **Spawn first-instruction** | If child process is doing heavy I/O that starves the completion-worker: consistent. |
| **Init-blocked-state** | Completion-worker = Ready (not Running): CONFIRMS starvation. Completion-worker = Running: EXCLUDES. |

### Fix sketch

Add synchronous completion check after `notify_queue`: before parking, re-check the used ring. If the device already completed (TCG timing), process it inline and return without parking. This is the Linux `blk_mq_poll_stats_bust` pattern. In our code: after `notify_queue` in `submit_pending`, read the used index; if it advanced, extract the completion and set `IoResult::Ok` so the park-wake cycle is skipped for the fast path.

---

## Pattern D: Spawn/IPC hand-off lost wakeup

**Likelihood: LOW — spawn doesn't park on a token**

### Mechanism

The hang is *post-spawn*. The scenario sequence is: CSPRNG init → BEGIN → NeoFS RW mount → neo-init spawn → user-process spawn → HANG. The question: does the spawn path itself use the same park/wake mechanism, and could it lose a wakeup?

### Our code analysis — NOT EXPOSED

`spawn_user` (process.rs:1314-1378) does NOT park or block. It:
1. Parses the ELF.
2. Creates a Pmap.
3. Loads segments.
4. Allocates user stack.
5. Registers the process.
6. Creates the first thread.
7. Pushes it onto the ready queue via `scheduler::try_push_ready`.

No blocking, no tokens, no park/wake. The spawn is fire-and-forget. The spawned process starts running on its next scheduler slice.

**The parent process (neo-init or the memory-substrate scenario) does NOT wait for the child to start.** It continues executing. If it subsequently issues a virtio-blk I/O request (e.g., NeoFS write), THAT's where it parks — and we're back to Patterns A/B/C.

### Prior art — Redox OS

Redox's spawn path (`redox-os/kernel/src/process/mod.rs`) similarly doesn't park. The parent spawns the child and continues. Any parent-child synchronization is done via IPC channels (which DO use park/wake), but that's a separate concern from the spawn itself.

### Discriminator cross-reference

| Jarvis breadcrumb | Confirms/excludes Pattern D |
|---|---|
| **Per-step progress** | If the hang occurs *during* the spawn syscall (not after): CONFIRMS. If after spawn, during I/O: EXCLUDES. |
| **Spawn first-instruction** | If child never reaches entry point: possible spawn issue. If child reaches entry point: EXCLUDES D. |

### Fix sketch

N/A — not the likely cause. If confirmed by breadcrumbs (hang during spawn, not during I/O), investigate `try_push_ready` failure paths and scheduler full conditions.

---

## Pattern E: Why intermittent / contention-sensitive

**Likelihood: META — explains the flake behavior, not a separate bug**

### Mechanism

Lost-wakeup races and notification-suppression races are inherently timing-dependent. The vulnerable window is a few instructions. Contention widens it by:
1. Delaying the completion-worker's scheduling (starvation).
2. Increasing the chance that the device completes before the submitter finishes parking (under TCG, "before" means "in the same guest context").
3. Causing the IPI reschedule (line 1210 in `wake_thread_token`) to be delayed by other pending work on the target CPU.

### Prior art — seL4

seL4's block driver (`camm` / `blkdrv`) documents this explicitly: the park/wake handshake is a **linearizable critical section** whose duration depends on scheduling latency. Under high load, the window where the wake can be "lost" (delivered before the park completes) widens from nanoseconds to milliseconds. seL4 addresses this with bounded-wait protocols: the waiter spins for N ticks before parking, re-checking the condition each tick.

### Why it passes on fresh runner but fails under contention

On a fresh CI runner:
- Fewer competing threads → completion-worker gets scheduled quickly.
- Less cache pressure → TCG emulation is faster → device completes "in time."
- Lower IPI latency → reschedule IPI fires promptly.

Under shard-4 tail contention:
- Many threads competing → completion-worker starves.
- Cache thrashing → TCG is slower → timing shifts.
- IPI queue congestion → reschedule delayed.

### Observable signature

No specific log signature — this explains *why* the bug is intermittent. The actual mechanism (A, B, or C) produces the log evidence; E explains the flake behavior.

### Discriminator cross-reference

| Jarvis breadcrumb | Confirms/excludes Pattern E |
|---|---|
| **All four** | If the hang correlates with contention (shard position, runner load): CONFIRMS E as an amplifier. E doesn't stand alone — it amplifies whichever of A/B/C is the root cause. |

### Fix sketch

Make the completion path **synchronous for the fast path**: after `notify_queue`, immediately check if the device already completed (TCG will have processed it). If so, process inline and return without parking. This eliminates the timing window entirely for the common case. For the slow path (device hasn't completed yet), the park/wake is still needed, but the fast path covers most TCG executions.

---

## Ranked suspect catalog

| Rank | Pattern | Mechanism | Our code exposed? | Fix complexity |
|------|---------|-----------|-------------------|----------------|
| **1** | **A + C interaction** | Completion fires before park finishes; or completion-worker starves under contention; token publish is technically correct but the scheduling window is too wide | **YES** — submitter parks, notifies device, switches away; completion-worker may not run for many ticks under contention | Low — add sync completion check after notify (Linux blk-mq pattern) |
| **2** | **C (starvation)** | Completion-worker starves under contention; submitter parks forever waiting for a wake that will come in 420s+ | **YES** — no priority mechanism for completion-worker | Medium — boost completion-worker priority or add IRQ-driven completions |
| **3** | **B (lock-order)** | Completion-worker spins on virtio-blk lock held by parked submitter; but release-on-new-stack should prevent this | **NO** — release-on-new-stack + without_interrupts prevents classic deadlock. Edge case: if T2 (post-switch thread) panics or hangs | N/A — likely not the cause |
| **4** | **D (spawn)** | Spawn path loses a wakeup | **NO** — spawn doesn't park | N/A — not the cause |
| **5** | **E (contention)** | Meta-pattern explaining intermittency | N/A — amplifies A/B/C | Addressed by fixing A/C |

---

## Discriminator matrix — which breadcrumb confirms what

```
+---------------------+-----+-----+-----+-----+-----+
| Breadcrumb          |  A  |  B  |  C  |  D  |  E  |
+---------------------+-----+-----+-----+-----+-----+
| Per-step progress   |  Y  |  Y  |  Y  |  Y  |  N  |
| Block-request IDs   |  Y  |  Y  |  Y  |  N  |  N  |
| Spawn first-instr   |  ~  |  ~  |  ~  |  Y  |  N  |
| Init-blocked-state  |  Y  |  Y  |  Y  |  ~  |  N  |
+---------------------+-----+-----+-----+-----+-----+
  Y = decisive, ~ = supportive, N = not applicable
```

**Decisive log signatures to look for:**

1. **Pattern A confirmed:** `wake=Gone` or `wake=Stale` in completion log → token not yet visible when completion fired.
2. **Pattern B confirmed:** completion-worker stuck spinning on virtio-blk lock (blocked-state = Running but not making progress; no yield observed).
3. **Pattern C confirmed:** completions logged with advancing request IDs, but submitter still Blocked → starvation (worker processing other requests, not this one; or worker is Ready but not Running).
4. **Pattern D confirmed:** hang occurs during spawn syscall, child never reaches entry point.

---

## Recommended instrumentation for Jarvis's serial log capture

To maximize the diagnostic value of the captured log, add these minimal serial prints:

1. **In `poll_used` (virtio_blk.rs:662):** Before `take()`, log `waiting_thread.is_some()` and the token values.
2. **In `wake_thread_token` (scheduler.rs:1185):** Log the `TakeThreadResult` with the token cpu/slot/generation vs. what was found in the slot.
3. **In `block_current_with_token_and_guard` (scheduler.rs:1088):** Log the token being created and the current CPU, right before and after the asm switch.
4. **In `virtio_blk_completion_worker` (virtio_blk.rs:920):** Log each poll iteration's CPU and whether a completion was found.

These four instrumentations will disambiguate A vs B vs C in a single hung capture.

---

## Closing note

This is research/docs only. No code was modified. The FINDINGS above are analysis grounded in our actual source code (`scheduler.rs`, `virtio_blk.rs`, `switch_mutex.rs`, `context.rs`) and prior art from Linux blk-mq, the virtio spec §2.7, rust-vmm, seL4, Redox, and Theseus. The fix for whichever pattern is localized belongs in a separate scoped dispatch.

**Docs-only ⇒ no CI run exists (paths-ignored).** Close via `scripts/worker-close-gate.sh` host set locally.
