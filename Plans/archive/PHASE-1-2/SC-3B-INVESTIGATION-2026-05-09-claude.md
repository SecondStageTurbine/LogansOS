# SC-3B stress-gate failure investigation (Claude-3)

**Date:** 2026-05-09
**Author:** Claude-3 (independent session, no prior project memory)
**Status:** Investigation analysis. Awaiting PM synthesis with parallel agents' analyses.

---

## 1. Substrate verification

Files provided and read for this analysis:

```
Plans/DISPATCH-2026-05-09-sc-3b-investigation-parallel.md     — dispatch + project briefing
Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md  — PM hypothesis space + failure signal
Plans/SC-3B-VERIFICATION-2026-05-09.md                        — Cerebro's gate log with failure
Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md                      — ratified PLAN (design reference)
Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md              — NOT PROVIDED. Analysis of Invariants
                                                                  I1–I6 constrained to references
                                                                  in the dispatch and plan synthesis.
kernel/src/arch/x86_64/context.rs                             — asm helper
kernel/src/sched/scheduler.rs                                 — scheduler, block_current_into_with_guard
kernel/src/ipc/mod.rs                                         — IPC channel operations
kernel/src/sync/switch_mutex.rs                               — SwitchMutex implementation
kernel/src/test_harness.rs (stress test sections)             — smp_cross_cpu_rendezvous_stress,
                                                                  stress_sender, stress_receiver,
                                                                  wait_for_stress_* functions
```

Files referenced in the dispatch but NOT read in this session:
- `docs/bible.md` — governance posture referenced by name only
- `docs/ADR-013-phase-2-smp-retrofit.md` — Q4/Q6 substrate; design intent inferred from plan synthesis
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` — BUILD dispatch; §6.2 stress test spec referenced but not read directly
- `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log` — failure artifact; content reproduced in verification log (BEGIN frame, then silence)

This analysis is therefore strongest on: kernel source mechanics, stress test protocol logic, and fresh hypothesis generation. Weakest on: ADR cross-references and Bible article citations (referenced by number from dispatch rather than direct reads).

---

## 2. Phase A — Source reading and hypothesis enumeration

### 2.1 Relevant code paths

**`context_switch_with_release2` (context.rs:48–68)**

Assembly: pushes RBX/RBP/R12–R15, saves RSP to `[rdi]`, loads `rsi` into RSP, stores zero byte to `[rdx]` (channel lock), stores zero byte to `[rcx]` (scheduler lock), pops callee-saved from new stack, returns.

Critical properties:
- All lock metadata in registers (RDX, RCX). Nothing read from old stack after `mov rsp, rsi`.
- Release ordering is channel → scheduler.
- No fence instruction. No RFLAGS save/restore.
- Matches the ratified ABI in the plan synthesis exactly.

**`block_current_into_with_guard` (scheduler.rs:483–531)**

Execution order: acquires scheduler lock, pops next thread, marks prev Blocked, runs installer (writes EndpointState while channel guard is held), captures `prev_rsp_ptr = &mut prev.rsp` (before installer moves prev), captures both release tokens, `mem::forget` both guards, calls asm. On resumed path, returns `current.blocked_return`.

Key invariant: `prev.rsp` is in heap-allocated `Box<Thread>`. The pointer is captured before the installer moves `prev`, but the Box's content has a stable heap address, so the pointer survives the move. The asm writes `*prev_rsp_ptr` before releasing the channel lock, so any CPU that extracts the thread after channel lock release sees the correctly saved RSP.

**`Channel::send` (ipc/mod.rs:140–204)**

Two paths when peer is `RecvBlocked`:
→ calls `continue_current_after_receive(recv_thread, n)` — wakes receiver, **does NOT yield**. Caller continues running with deferred preemption (`preempt_defer_ticks = 32`).

When peer is Idle / otherwise:
→ calls `block_current_into_with_guard(guard, installer)` — caller blocks, holding channel guard.

**`Channel::recv` (ipc/mod.rs:206–259)**

When peer is `SendBlocked`:
→ calls `continue_current_after_receive(send_thread, n)` — wakes sender to front of current CPU queue, **does NOT yield**. Receiver continues running.

When peer is Idle:
→ `block_current_into_with_guard` — blocks while holding channel guard.

**`handoff_to_rendezvous_peer` (ipc/mod.rs:461–465)**

Used when `send` finds `RecvBlocked` and when `grant` operations complete. Pushes peer to front of current CPU queue, then **immediately yields**. Caller suspends.

**Note on asymmetry:** `continue_current_after_receive` (no yield) is used when `recv` finds `SendBlocked` or when `recv_grant` completes. `handoff_to_rendezvous_peer` (yields immediately) is used when `send` finds `RecvBlocked`. This asymmetry is load-bearing for the Q4 CPU migration behavior, as analyzed in §2.3.

**`SwitchMutex` (switch_mutex.rs)**

Standard TTAS spinlock: `compare_exchange_weak(false, true, Acquire, Relaxed)` outer loop; `load(Relaxed)` inner spin; `store(false, Release)` on drop. `release_token()` returns `&self.lock` as a raw pointer. Correct.

**`run_smp_cross_cpu_rendezvous_stress` (test_harness.rs:2733–2794)**

Key observations:

1. **Pairs = 4 at NEO_SMP=4.** CPU assignments: pair P → sender on CPU P, receiver on CPU (P+1) % 4.
   - Pair 0: sender CPU0, receiver CPU1
   - Pair 1: sender CPU1, receiver CPU2
   - Pair 2: sender CPU2, receiver CPU3
   - Pair 3: sender CPU3, receiver CPU0

2. **`send_first` assignment:** `let send_first = (seq & 1) == 1`. Pair P uses seq values `P+1, P+1+4, P+1+8, ...`. For pair 0: seq = 1, 5, 9, … (all odd → always send_first). For pair 1: seq = 2, 6, 10, … (all even → always recv_first). For pair 2: seq = 3, 7, 11, … (all odd → always send_first). For pair 3: seq = 4, 8, 12, … (all even → always recv_first). **This is fully deterministic. Pairs 0 and 2 are always send_first; pairs 1 and 3 are always recv_first.**

3. **`wait_for_stress_go` is an infinite loop** with no timeout. Workers spin here until the orchestrator sends the GO signal.

4. **Finite polls:** `wait_for_stress_endpoint_blocked` and `wait_for_stress_iteration_done` both loop 100,000 times and then call `fail_with_hex`, which emits serial output (`NEO_TEST\1 event=FAIL ...`) and then `debug_exit(EXIT_FAILURE)`. If these timeouts fire, xtask receives the exit and records the failure with serial content. **The absence of serial output in the run-036 failure means neither fired.** The kernel is silent — in an infinite loop or within a very long finite loop not yet expired.

5. **`wait_for_stress_workers_exit` loops 1,000,000 times** before calling `fail_with_hex`. This is the last thing the orchestrator does.

### 2.2 Q4 migration map (derived from source)

For pair 0 (always send_first):
- Sender_0 (CPU0) blocks → receiver_0 (CPU1) calls recv, finds SendBlocked → `continue_current_after_receive(sender_0)` → pushes sender_0 to CPU1's front queue. Receiver_0 continues, writes RECEIVER_DONE, yields in wait_for_stress_go → sender_0 runs on CPU1, writes SENDER_DONE.
- **After iteration 1:** sender_0 is on CPU1. Receiver_0 is on CPU1.
- **After iteration 5** (next pair-0): sender_0 (on CPU1) blocks on CPU1 → receiver_0 (on CPU1) completes → `continue_current_after_receive` → sender_0 stays on CPU1. Both stay on CPU1.
- **Stable state:** sender_0 and receiver_0 both permanently on CPU1 after first iteration.

For pair 1 (always recv_first):
- Receiver_1 (CPU2) blocks → sender_1 (CPU1) calls send, finds RecvBlocked → `handoff_to_rendezvous_peer(receiver_1)` → pushes receiver_1 to CPU1's front queue, sender_1 yields.
- **After iteration 2:** receiver_1 migrates to CPU1. Sender_1 stays on CPU1.
- **Stable state:** sender_1 and receiver_1 both permanently on CPU1 after first iteration.

For pair 2 (always send_first):
- Sender_2 (CPU2) blocks → receiver_2 (CPU3) completes → `continue_current_after_receive` → sender_2 migrates to CPU3. Both on CPU3.

For pair 3 (always recv_first):
- Receiver_3 (CPU0) blocks → sender_3 (CPU3) completes → `handoff_to_rendezvous_peer` → receiver_3 migrates to CPU3. Both on CPU3.

**Stable CPU composition (after first iteration of each pair):**

| CPU | Threads |
|-----|---------|
| CPU0 | orchestrator, BSP bootstrap |
| CPU1 | AP-1 bootstrap, sender_0, receiver_0, sender_1, receiver_1 (4 stress workers) |
| CPU2 | AP-2 bootstrap |
| CPU3 | AP-3 bootstrap, sender_2, receiver_2, sender_3, receiver_3 (4 stress workers) |

CPU0 is nearly idle (orchestrator + bootstrap). CPU2 is completely idle. CPU1 and CPU3 each have 4 stress workers plus bootstrap — **5 threads per CPU competing for scheduling.**

### 2.3 Hypothesis enumeration

**H1 — Lost wakeup in Q4 under contention**

*Original mechanism:* Two CPUs simultaneously try to wake the same parked thread.

*Analysis:* The channel SwitchMutex prevents this. Only one CPU can hold the channel lock; extraction of a parked thread requires the lock. Two CPUs cannot simultaneously extract the same thread.

*H1 as stated: not possible given source.*

*H1-variant:* Could a parked thread be extracted but then lost before being enqueued? In `handoff_to_rendezvous_peer`:
```rust
sched::wake_thread_on_current_cpu(thread, return_value);  // enqueues to CPU_X front
sched::yield_now();                                        // switches to that thread
```
Between the enqueue and yield_now, the thread is in CPU_X's ready queue. Can it be lost? No — the scheduler lock is held across `wake_thread_front` (push), and yield_now acquires its own lock. The push is atomic with respect to the scheduler.

In `continue_current_after_receive`:
```rust
sched::wake_thread_on_current_cpu(thread, return_value);  // enqueues to CPU_X front
sched::defer_current_preempt_for_handoff();               // sets preempt_defer, does not yield
```
Thread is in CPU_X's ready queue but CPU_X keeps running the completer. Eventually timer preemption or a cooperative yield will run the woken thread. No wakeup loss.

**H1: RULED OUT by source analysis. No mechanism for a parked thread to be lost during extraction or enqueue.**

---

**H2 — Race in `block_current_into_with_guard` (prev_rsp_ptr aliasing timing)**

*Analysis:* Tracing the execution order:
1. `prev_rsp_ptr = &mut prev.rsp` — raw pointer captured.
2. `installer(&mut *channel_guard, prev)` — prev moved into EndpointState (Box heap-pinned, so prev_rsp_ptr still valid).
3. `context_switch_with_release2(prev_rsp_ptr, next_rsp, channel_lock, sched_lock)` — ASM: saves RSP to `*prev_rsp_ptr` (`mov [rdi], rsp`) **before** releasing the channel lock (`mov byte ptr [rdx], 0`).

A completer CPU extracting prev_thread from EndpointState must acquire the channel lock. That acquire happens after `mov byte ptr [rdx], 0`. By then the RSP is already written. On x86 TSO, store-store ordering is guaranteed; the RSP write precedes the lock-release write in program order.

**H2: RULED OUT by source analysis.**

---

**H3 — Test scenario logic deadlock**

*Analysis:* The pairing protocol itself (GO signals, DONE signals, wait_for_stress_iteration_done) is correct. The orchestrator sequences pair usage through `(seq-1) % pairs`. Within each pair, the worker seq advances by `pairs`, so workers and orchestrator use the same seq for each pair iteration. The `>= seq` semantics on DONE polls are correct because workers cannot advance past their current seq until the orchestrator signals the next iteration.

However, the source audit reveals a structural property of the test that constitutes a real H3 variant: see **Hn** below.

*Direct pairing deadlock:* No deadlock found in the protocol. Workers in `wait_for_stress_go` loop indefinitely, but only because the GO signal has not arrived — the orchestrator will send it as part of its sequential seq loop. No circular dependency exists in the protocol itself.

**H3 as deadlock-in-logic: NOT PRESENT. H3-variant (structural design issue) — see Hn.**

---

**H4 — Per-CPU ready queue starvation under Q4 bias**

*Analysis:* The migration map in §2.2 shows a clear imbalance: CPU1 and CPU3 each accumulate 4 stress workers plus bootstrap. CPU0 and CPU2 end up with only orchestrator/bootstrap and no stress workers.

The orchestrator on CPU0 polls via `yield_now()` inside the 100,000-iteration loops. Since CPU0 only has the orchestrator and bootstrap, each `yield_now()` cycle is: orchestrator → bootstrap (which immediately calls `yield_now()` itself) → orchestrator. This is extremely fast — each round trip may take microseconds.

Meanwhile workers on CPU1 and CPU3 share 5 threads of CPU time. Each worker gets roughly 1/5th of the available CPU cycles. Under QEMU's SMP emulation, this can mean workers on CPU1/CPU3 get turns infrequently.

**The potential starvation scenario:**
- Orchestrator sets SENDER_GO[P] for a pair whose worker is on CPU3 (heavily loaded).
- Worker on CPU3 needs 5 turns in wait_for_stress_go to check the signal, wake, call channel.send, and complete the block.
- Meanwhile, orchestrator on CPU0 spins through `wait_for_stress_endpoint_blocked` extremely quickly.
- If each orchestrator `yield_now()` takes ~100ns (conservative) and each CPU3 worker turn takes ~1ms (shared among 5 threads at 100Hz QEMU scheduling), the orchestrator runs 10,000 cycles per CPU3 worker turn.
- 100,000 orchestrator cycles / 10,000 = 10 CPU3 worker turns needed. This is fine — 10 turns is more than enough for a worker to complete one IPC operation.

*However:* if QEMU's host is under additional load and CPU3's 5 threads contend not just with each other but with host-level scheduling, a worker might get a turn only every 10ms. Then 100,000 orchestrator cycles × 100ns = 10ms = exactly the time a worker gets one turn. The threshold is tight.

**H4 at steady state: LOW probability of causing a permanent hang.** More likely causes a very slow run that stretches toward the 180s QEMU timeout.

---

**H5 — SwitchMutex starvation**

*Analysis:* The TTAS pattern prevents livelock on x86 (no LL/SC spurious failures). Each of the 4 pairs has its own channel SwitchMutex — there is no single global SwitchMutex being contested by all workers. The per-CPU scheduler SwitchMutexes are also per-CPU, not global.

**H5: RULED OUT. No global SwitchMutex hotspot and TTAS is sound on x86.**

---

**H6 — Missing memory barrier**

*Analysis:* On x86 TSO, stores from the same CPU are observed by all other CPUs in program order. The sequence in the asm is:
1. `mov [rdi], rsp` — writes prev RSP into EndpointState (via Box heap)
2. `mov byte ptr [rdx], 0` — releases channel lock

Step 2 precedes step 1 in terms of visibility to other CPUs? No — TSO guarantees steps 1 and 2 are observed in program order. Any CPU that acquires the channel lock after step 2 will have already observed step 1. The Acquire-on-lock / Release-on-unlock pairing is correct in SwitchMutex.

**H6: RULED OUT. x86 TSO + Acquire/Release semantics are sufficient.**

---

**Hn — Deterministic Q4 migration produces permanent CPU imbalance leading to orchestrator poll timeout**

*This hypothesis is not in the original triage and is the primary finding of this analysis.*

**Mechanism:**

The stress test uses `send_first = (seq & 1) == 1`. Because pair P always uses seq values `P+1, P+1+4, P+1+8, ...`, the parity of seq for each pair is fixed. Pairs 0 and 2 always get odd seq (send_first). Pairs 1 and 3 always get even seq (recv_first).

Combined with Q4 wake-on-completer-CPU:
- Always-send_first pairs: sender blocks, receiver completes via `continue_current_after_receive` (no yield). Receiver stays on its CPU. Sender is woken to the receiver's CPU front queue. After first iteration, both sender and receiver are on the original receiver's CPU. They stay there.
- Always-recv_first pairs: receiver blocks, sender completes via `handoff_to_rendezvous_peer` (yields immediately). Receiver wakes to sender's CPU. After first iteration, both are on the original sender's CPU.

Applying to pair assignments (pair 0: sender=CPU0, receiver=CPU1):
- Pair 0 (send_first): receiver_0 stays CPU1, sender_0 migrates to CPU1. **Both on CPU1.**
- Pair 1 (recv_first): sender_1 stays CPU1, receiver_1 migrates to CPU1. **Both on CPU1.**
- Pair 2 (send_first): receiver_2 stays CPU3, sender_2 migrates to CPU3. **Both on CPU3.**
- Pair 3 (recv_first): sender_3 stays CPU3, receiver_3 migrates to CPU3. **Both on CPU3.**

After the first iteration of each pair (which happens within the first 4 orchestrator sequences):
- **CPU0:** only orchestrator and BSP bootstrap. Zero stress workers.
- **CPU1:** 4 stress workers (sender_0, receiver_0, sender_1, receiver_1) + AP-1 bootstrap.
- **CPU2:** only AP-2 bootstrap. Zero stress workers.
- **CPU3:** 4 stress workers (sender_2, receiver_2, sender_3, receiver_3) + AP-3 bootstrap.

This is the permanent CPU composition for the remaining ~996 of the 1000 iterations.

**Why this causes the 180s timeout without serial output:**

The orchestrator on CPU0 polls using `yield_now()`. CPU0 has only orchestrator and bootstrap. Each `yield_now()` pair (orchestrator → bootstrap → orchestrator) may be extremely fast — on the order of microseconds in QEMU — because neither CPU0 nor CPU2 has any competing stress workers.

But workers the orchestrator needs to signal are on CPU1 and CPU3, each running 5 threads. QEMU's SMP emulation uses one host thread per vCPU. Under moderate host load, vCPU1 and vCPU3 host threads may be scheduled at OS-level preemption granularity (typically 1ms–10ms timeslices on Linux). Each of 5 workers gets roughly 200μs–2ms of wall time per 1ms–10ms host timeslice.

Under stress, a worker on CPU1 might get a turn only every 5–50ms. For the orchestrator on CPU0 to exhaust 100,000 `yield_now()` iterations at ~1μs each takes 100ms. For a worker on CPU1 to respond (wake from wait_for_stress_go, call channel.send, complete the block) requires only 1 turn — but that turn must arrive within the orchestrator's 100ms window.

In 35/36 successful runs, this timing works. In run 36, some combination of host system load increases the per-worker turn interval on CPU1/CPU3 enough that 100,000 orchestrator iterations expire before the worker completes the channel operation, and `fail_with_hex` fires. BUT: `fail_with_hex` would produce serial output, which we do NOT see.

**Revised mechanism (explaining absence of serial output):**

The 100,000 iterations of `wait_for_stress_endpoint_blocked` or `wait_for_stress_iteration_done` are NOT fast — they appear fast in the common case, but under heavy QEMU host load, `yield_now()` on CPU0 is itself slower. Why? Because QEMU's thread scheduler determines when the vCPU0 host thread runs. If the host is under load (which changes from run to run), vCPU0 might also get scheduled infrequently, making each `yield_now()` on CPU0 take much longer. At ~1.8ms per iteration (possible under heavy host load), 100,000 iterations = 180s = exactly the QEMU timeout.

The QEMU process-level timeout fires at 180s. At that moment, the orchestrator is still inside `wait_for_stress_endpoint_blocked` or `wait_for_stress_iteration_done`, has NOT reached 100,000 iterations, and has NOT called `fail_with_hex`. Zero serial output beyond BEGIN.

**This is fully consistent with all observations:**
- BEGIN frame present (orchestrator started)
- No FAIL frame (fail_with_hex never reached 100,000)
- No PASS frame (test never completed)
- No BUG/PANIC (no kernel error)
- 35/36 clean runs (this timing condition is rare; most runs complete well before 180s)
- Run 36 fails (host load momentarily elevated, slowing QEMU CPU scheduling)

**Additional contributing factor:** The 4-worker accumulation on CPU1 means pair 0 and pair 1 workers compete with each other for CPU1 time. When the orchestrator is working on pair 0 (for example), pair 1's workers are also on CPU1 consuming CPU time in yield loops — slowing down pair 0's workers.

---

## 3. Phase B — Instrumentation strategy

### 3.1 NMI watchdog (`stash@{0}`) — Strongly recommend

Applying the wedge-investigation NMI watchdog will convert the silent hang into a panic with per-CPU stack traces. This is the highest-value single diagnostic action.

**Recommended NMI threshold:** 5 seconds. This is well below the minimum per-iteration wall time for normal execution (each of 1000 iterations takes <10ms in normal runs, so 5s represents >500 stuck iterations), and well below the 180s QEMU timeout. It will fire only on a genuine hang.

The NMI will show:
- CPU0: orchestrator in `yield_now` inside `wait_for_stress_endpoint_blocked` or `wait_for_stress_iteration_done`
- CPU1: 4-5 threads in various states, likely all in `yield_now` (inside `wait_for_stress_go` or `yield_now` from bootstrap)
- CPU2: AP-2 bootstrap idle
- CPU3: 4-5 threads similarly

This directly confirms or refutes the Hn CPU composition picture.

### 3.2 Per-iteration PROGRESS frames

Add to the orchestrator's iteration loop, emitted before each `wait_for_stress_endpoint_blocked` call:

```rust
crate::serial_println!(
    "NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress iter={} pair={} send_first={}",
    seq, pair, send_first
);
```

The last PROGRESS line in a failure run identifies which iteration and pair hung. If the failing pair is consistently pair 1 or pair 3 (the recv_first pairs that push all workers to CPU1 or CPU3), Hn is confirmed.

**Heisenbug mitigation:** Only emit every 50th iteration to reduce serial output volume and timing perturbation. The last-emitted PROGRESS line will be within 50 iterations of the hanging one.

### 3.3 Thread CPU tracking in workers

In `stress_sender` and `stress_receiver`, log the CPU before and after the channel call:

```rust
// Before channel.send/recv:
let pre_cpu = crate::arch::x86_64::per_cpu::current_cpu_id();
let ret = crate::arch::x86_64::without_interrupts(|| { fixture.channel.send(...) });
let post_cpu = crate::arch::x86_64::per_cpu::current_cpu_id();
if pre_cpu != post_cpu {
    crate::serial_println!("SC3B_MIGRATE pair={} seq={} from={} to={}", pair, seq, pre_cpu, post_cpu);
}
```

This captures every Q4 migration. Logging only when pre_cpu != post_cpu keeps output volume low. The resulting log confirms the migration map in §2.2 and shows whether the stable CPU composition matches the prediction.

### 3.4 Ready-queue depth diagnostic

Add a function to scheduler.rs:

```rust
#[cfg(feature = "neo-test-smp-cross-cpu-rendezvous-stress")]
pub fn ready_queue_depth(cpu_id: u32) -> usize {
    without_interrupts(|| {
        let idx = cpu_id as usize;
        if idx >= per_cpu::MAX_CPUS { return 0; }
        let ptr = PER_CPU[idx].scheduler_ptr.load(Ordering::Acquire).cast::<PerCpuScheduler>();
        if ptr.is_null() { return 0; }
        unsafe { &*ptr }.inner.lock().ready.len()
    })
}
```

Call this from the orchestrator every 100 iterations:
```rust
if seq % 100 == 1 {
    serial_println!("SC3B_READY cpu0={} cpu1={} cpu2={} cpu3={}",
        ready_queue_depth(0), ready_queue_depth(1),
        ready_queue_depth(2), ready_queue_depth(3));
}
```

This directly shows the CPU load imbalance over the course of 1000 iterations. If CPU1 and CPU3 consistently show 4–5 threads while CPU0 shows 0–1, Hn is confirmed.

### 3.5 Poll iteration count logging

In `wait_for_stress_endpoint_blocked`, log the number of iterations taken:

```rust
for i in 0..100_000 {
    if blocked { 
        if i > 1000 {
            serial_println!("SC3B_SLOW endpoint_blocked pair={} iter={}", pair, i);
        }
        return; 
    }
    yield_now(); reap();
}
```

High iter counts (>1000) indicate the worker was slow to block — consistent with H4/Hn on a heavily loaded CPU.

### 3.6 Recommended order

1. Apply NMI watchdog (`stash@{0}`, NOT committed). Run 5-run loop. Any hang produces stack traces.
2. Add PROGRESS frames (every 50 iterations). Run 20-run loop. Failure identifies stuck iteration/pair.
3. Add migration logging in workers. Run 10-run loop. Confirms migration map.
4. Add ready-queue depth diagnostic (every 100 iterations). Run 10-run loop. Quantifies imbalance.

Do NOT add all instrumentation at once — the accumulated serial output may hide the timing issue.

---

## 4. Phase C — Reproduction predictions

### 4.1 Reproduction probability

- **Uninstrumented, 50-run loop:** ~30–70% chance of observing at least one failure. Based on 1/36 historical rate, expected ~1.4 failures in 50 runs. Variance is high.
- **With NMI watchdog only:** 100% of failures that occur will produce actionable stack traces. Does not change failure rate.
- **With PROGRESS frames:** Slight heisenbug risk. Serial output timing may perturb QEMU scheduling enough to reduce or increase failure rate by up to 2×. Run NMI watchdog first to get at least one clean stack trace before adding PROGRESS frames.
- **At NEO_SMP=2:** Based on the migration map, with only 2 CPUs the composition after first pair iteration would be very different (and potentially more balanced). Failure rate at NEO_SMP=2 is likely much lower. This is consistent with the dispatch's instruction to defer NEO_SMP=2 testing.

### 4.2 Per-hypothesis predicted signatures

**If Hn (primary hypothesis):**

NMI stack traces:
- CPU0: orchestrator in `yield_now` inside `wait_for_stress_endpoint_blocked` or `wait_for_stress_iteration_done`. No workers on CPU0.
- CPU1: 4–5 threads in `yield_now` loops (3–4 in `wait_for_stress_go`, 1 possibly in channel.send/recv, 1 AP-1 bootstrap in its yield loop)
- CPU2: AP-2 bootstrap, nothing else
- CPU3: 4–5 threads in yield loops (same pattern as CPU1)

PROGRESS frames (if added): last frame is consistently `pair=1` or `pair=3` (the always-recv_first pairs, whose workers accumulate on CPU1/CPU3).

Migration log: shows all 8 stress workers migrating to CPU1 and CPU3 within the first 8 iterations. No stress workers on CPU0 or CPU2 after iteration 8.

Ready-queue depth: CPU1 = 4–5, CPU3 = 4–5, CPU0 = 0–1, CPU2 = 0–1 consistently from iteration 10 onward.

**If H1-variant (Q4 wake race):**

NMI would show: one CPU with a sender/receiver stuck in `wait_for_stress_go` AND the corresponding channel endpoint in a Blocked state (i.e., the worker that should have completed the rendezvous is somehow not running). The orchestrator might be past `wait_for_stress_endpoint_blocked` (already saw Blocked state) but stuck in `wait_for_stress_iteration_done`.

PROGRESS frames: last frame would be immediately before `wait_for_stress_iteration_done`. The failing pair would vary across runs (not always pair 1/3).

**If H4 (starvation):**

Slow endpoint_blocked logs showing iter counts of 30,000–90,000 (approaching the 100,000 limit) across multiple iterations. Eventually one iteration exceeds 100,000 and fail_with_hex fires — but this would produce serial output, which we don't see. So H4 alone doesn't fully explain the symptom; it needs the additional Hn element (CPU0 yield_now is also slow).

### 4.3 Heisenbug risks

- **Serial output (PROGRESS frames):** UART write involves a spinlock (serial output is serialized). Each write adds ~1ms under QEMU. With 20 PROGRESS frames per run, this adds ~20ms but also slows the orchestrator relative to workers, potentially reducing the timing disparity that causes the failure. Mitigation: emit only every 50 iterations.

- **NMI watchdog:** The NMI fires regardless of interrupt state. It will interrupt whichever thread is current on each CPU. If it fires during the `mem::forget + asm` window (the panic-safe window in `block_current_into_with_guard`), it could produce a confusing stack trace (we'd see the asm return address, not a Rust frame). Mitigation: accept this and use the function name at the call site for diagnosis.

- **Migration logging:** Each serial write in `stress_sender/receiver` adds latency to the hot path. This may slow workers enough to increase the time gap between worker completion and orchestrator polling — potentially making the timing worse or better depending on direction. Mitigation: only log migrations (not every iteration), keeping the hot path mostly unaffected.

---

## 5. Phase D — Root-cause hypothesis ranking

### 5.1 Top-3 ranking

**Rank 1: Hn — Deterministic Q4 migration creates permanent 4-worker accumulation on CPU1/CPU3; combined with QEMU scheduling variability, the orchestrator's finite poll loops take >180s before reaching 100,000 iterations.**

Confidence: **55%**

Evidence:
- The pair-to-direction assignment is provably deterministic (mathematical: pair P uses seq ≡ P+1 mod 4, parity is fixed).
- Q4 migration map (§2.2) is derived directly from the `continue_current_after_receive` / `handoff_to_rendezvous_peer` paths in source.
- The "no serial output + 180s timeout" symptom is uniquely consistent with the orchestrator NOT reaching `fail_with_hex` because 100,000 iterations haven't completed — which happens when each iteration is ~1.8ms (QEMU scheduling under load).
- Runs 1–35 passed because host load was lower; run 36 failed because host load peaked.

What would increase confidence to >90%: NMI stack traces showing CPU0 in `yield_now` inside a finite poll, with CPU1/CPU3 each having 4+ workers in yield loops.

What would decrease confidence: NMI showing CPU0 is NOT in a poll loop (e.g., in `wait_for_stress_workers_exit`) or that CPU1 has fewer than 3 workers.

**Rank 2: H4/Hn combination — CPU1/CPU3 worker starvation means workers don't complete IPC operations within the orchestrator's poll window, but the proximate cause is the orchestrator's poll being too fast relative to worker execution (same root as Hn, different framing).**

Confidence: **25%** (partially subsumed by Hn; separated here because the fix direction is slightly different)

This variant's fix would focus on slowing the orchestrator's poll (adding a sleep or reducing poll frequency) rather than rebalancing worker CPU distribution.

**Rank 3: H3-variant — `continue_current_after_receive` asymmetry (no yield) causes send-first pairs to create "two workers on same CPU" situations that degrade responsiveness for receive-first pairs sharing the same CPU.**

Confidence: **15%**

This is a secondary mechanism that amplifies Hn. The no-yield path in `continue_current_after_receive` is what keeps pairs 0 and 2 on the same CPU (receiver stays; sender migrates to receiver's CPU), which is the root of the accumulation. The fix for Hn (break deterministic pair-direction assignment) also fixes this mechanism.

### 5.2 Definitive ruling-out evidence (from source)

H1 (as stated), H2, H5, H6: definitively ruled out by source analysis. The kernel primitives — asm ordering, lock acquisition timing, SwitchMutex correctness, x86 TSO memory model — are all correct.

---

## 6. Phase E — Fix design (top-2 hypotheses)

### 6.1 Fix for Hn: Break deterministic pair-to-direction assignment

**Root cause:** The `(seq & 1)` direction assignment gives each pair a fixed direction. Q4 migration accumulates workers permanently on specific CPUs.

**Fix:** Replace global-seq-parity direction with per-pair alternation:

```rust
// In test_harness.rs, add to static section:
#[cfg(feature = "neo-test-smp-cross-cpu-rendezvous-stress")]
static STRESS_PAIR_DIRECTION: [core::sync::atomic::AtomicBool; STRESS_MAX_PAIRS] =
    [const { core::sync::atomic::AtomicBool::new(false) }; STRESS_MAX_PAIRS];

// In reset_stress_state():
for idx in 0..STRESS_MAX_PAIRS {
    STRESS_PAIR_DIRECTION[idx].store(false, Ordering::Release);
    // ... existing resets ...
}

// In run_smp_cross_cpu_rendezvous_stress(), replace:
let send_first = (seq & 1) == 1;
// With:
let send_first = STRESS_PAIR_DIRECTION[pair].fetch_xor(true, Ordering::Relaxed);
```

**Effect:** Each pair alternates between send_first and recv_first on consecutive uses. After K uses of pair P, K/2 were send_first and K/2 were recv_first. Workers for pair P bounce between CPU_sender and CPU_receiver rather than converging on one. No persistent accumulation.

**New invariant:** Workers are distributed across CPUs in a balanced, alternating pattern. CPU1 and CPU3 will each hold roughly 2 stress workers on average rather than 4.

**Files changed:** `kernel/src/test_harness.rs` only. No kernel code changes.

**Verification:**
1. Apply the fix.
2. Run the 200/200 primary stress closure gate at NEO_SMP=4.
3. Add migration logging (§3.3) and confirm that workers alternate CPUs rather than converging.
4. If 200/200 passes, the fix resolved the issue.
5. If failures recur: proceed to Phase B instrumentation to identify the actual cause.

**Anti-regression:** Test-only change. Existing scenarios are unaffected. The fix does not change any kernel behavior — only the direction in which the stress test exercises it.

**Simplify check:** Is there a smaller fix? Yes: simply set `send_first = true` always (never recv_first). This avoids the recv_first Q4 accumulation pattern entirely. But it would reduce test coverage of the recv_first path. The alternating approach maintains full coverage.

**Bible alignment:** The fix removes a test implementation defect without touching kernel code. Bible Article 1.3 ("every commit must be shippable") is satisfied — no kernel changes means no kernel risk.

### 6.2 Fix for H4 (secondary): Improve poll loop robustness for imbalanced QEMU scheduling

**Root cause:** `wait_for_stress_endpoint_blocked` and `wait_for_stress_iteration_done` poll loops are sensitive to CPU0's per-yield_now latency under QEMU load. The 100,000-iteration limit may expire before workers complete if QEMU schedules CPU0 slowly.

**Fix:** Add a longer warning before the timeout, and a staged fallback:

```rust
fn wait_for_stress_iteration_done(pair: usize, seq: u32) {
    const WARN_AT: u32 = 50_000;
    const FAIL_AT: u32 = 500_000;  // 5× increase
    for i in 0..FAIL_AT {
        if STRESS_SENDER_DONE[pair].load(Ordering::Acquire) >= seq
            && STRESS_RECEIVER_DONE[pair].load(Ordering::Acquire) >= seq
        {
            return;
        }
        if i == WARN_AT {
            crate::serial_println!(
                "SC3B_WARN iteration_done slow pair={} seq={} i={}",
                pair, seq, i
            );
        }
        crate::sched::yield_now();
        crate::sched::reap();
    }
    fail_with_hex("reason=stress_iteration_timeout seq", seq as u64);
}
```

The warning at 50,000 provides diagnostic data without causing early failure. The 500,000 limit gives ~5× more wall time for workers to complete under heavy QEMU load.

**Files changed:** `kernel/src/test_harness.rs` only.

**Effect:** Raises the tolerance threshold so that QEMU scheduling variability does not cause false timeouts. Does not fix the root cause (CPU imbalance from Hn) but eliminates the timeout path as a source of false failures.

**Anti-regression:** Test-only. Increases time to detect genuine deadlocks (from 100,000 to 500,000 iterations) but does not mask them — they still emit serial output and fail.

**Fix ordering:** Apply Fix 6.1 first. If Fix 6.1 resolves the 200/200 gate, Fix 6.2 is not needed. Apply Fix 6.2 only if failures persist after 6.1, to get better diagnostic data before escalating to kernel investigation.

---

## 7. Risks and unknowns

**What this analysis cannot determine from the provided files:**

- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` not read. Invariants I1–I6 were analyzed via the plan synthesis references. If I6 or another invariant constrains something about the Q4 wake or test design that modifies the Hn analysis, that would require direct reading.

- `docs/ADR-013-phase-2-smp-retrofit.md` not read directly. Q4 and Q6 design intent inferred from plan synthesis. If ADR-013 specifies something about the intended test structure that conflicts with the "per-pair direction alternation" fix, that would need to be reconciled.

- The failure serial log `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log` was not read directly (content reproduced in the verification log as: BEGIN then silence). The full log might contain partial serial output indicating which iteration was in progress, which would refine the Hn vs. H1-variant distinction.

**PM should consider:**

- Whether the worker-on-CPU0 phase (first 4 iterations, before all workers migrate) has any exposure to issues not covered here. The migration map assumes first-iteration migration completes quickly. If host scheduling is very unfavorable, the migration might take multiple iterations before completing — during which time CPU0 has 2 workers AND the orchestrator.

- Whether fixing the test is sufficient or whether the Q4 accumulation behavior also represents a kernel concern for production AI workloads (ANSPEC-001 §3.7). If real AI-inference workloads follow a similar pattern (long-running IPC-heavy tasks accumulating on one CPU), the root cause is a scheduling design issue worth documenting even if the test passes.

---

## 8. Cross-references

Files read for this analysis:
- `Plans/DISPATCH-2026-05-09-sc-3b-investigation-parallel.md`
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md`
- `Plans/SC-3B-VERIFICATION-2026-05-09.md`
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md`
- `kernel/src/arch/x86_64/context.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/ipc/mod.rs`
- `kernel/src/sync/switch_mutex.rs`
- `kernel/src/test_harness.rs` (stress-test sections)

Files NOT read (analysis constrained):
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`
- `docs/ADR-013-phase-2-smp-retrofit.md`
- `docs/bible.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md`
- `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log` (full content)

---

**End of investigation.**

*Core finding: H1/H2/H5/H6 ruled out by source analysis. Hn (deterministic Q4 migration accumulation from constant pair-direction assignment, leading to CPU load imbalance that degrades orchestrator polling under QEMU scheduling variability) is the leading hypothesis at 55% confidence. The structural fix is a test-only per-pair direction alternation (5–10 lines). Phase B instrumentation (NMI watchdog, PROGRESS frames, migration logging) provides definitive confirmation if the fix is insufficient.*
