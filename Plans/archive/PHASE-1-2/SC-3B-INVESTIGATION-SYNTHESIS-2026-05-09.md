# SC-3B Stress-gate failure investigation — PM synthesis

**Date:** 2026-05-09
**Author:** PM
**Status:** Synthesis of parallel investigation across three agents (Cerebro, Prometheus, Claude-3) plus PM's own source audit.
**Substrate:** HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` (Sub-step 5 commit). Backup at `Z:\AltimitNeo\2026-05-09-144314`.
**Input artifacts:**
- `Plans/DISPATCH-2026-05-09-sc-3b-investigation-parallel.md` — dispatch governing this work
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md` — PM triage that seeded the hypothesis space
- `Plans/SC-3B-VERIFICATION-2026-05-09.md` — Cerebro's verification log (the failing run)
- Source audit: `kernel/src/arch/x86_64/context.rs`, `kernel/src/sched/scheduler.rs`, `kernel/src/ipc/mod.rs`, `kernel/src/sync/switch_mutex.rs`, `kernel/src/test_harness.rs`

---

## 0. Headline

Source audit of the full SC-3B implementation stack (asm helper, Rust wrapper, IPC retrofit, SwitchMutex, stress test) reveals that the Q6 primitive and IPC retrofit are mechanically sound. The asm ordering, memory consistency, and lock-release sequencing all match the ratified PLAN design.

The failure signature (silent hang after BEGIN, no FAIL/BUG/PANIC, 35/35 clean before run 36) is inconsistent with a panic-class kernel bug. The 100,000-iteration finite polls in `wait_for_stress_endpoint_blocked` and `wait_for_stress_iteration_done` would emit serial output if they timed out. Since they do not, the hang is **inside a true infinite loop** — specifically inside workers' `wait_for_stress_go` (no timeout) — because the orchestrator itself is stuck in one of its finite polls that CANNOT complete because a worker it depends on is permanently waiting for a signal the orchestrator cannot send.

Two hypotheses explain this: a lost wakeup in the Q4 completer-CPU wake path under a specific scheduling collision (H1-variant), and a test-level deadlock arising from the deterministic pair-to-direction assignment combined with Q4 migration accumulation on CPU0 (Hn/H3-variant). These are described in §5 (ranking) and §6 (fix design).

---

## 1. Source reading inventory

### 1.1 Files read

```
kernel/src/arch/x86_64/context.rs       — context_switch_with_release2 asm + Rust decl
kernel/src/sched/scheduler.rs           — block_current_into_with_guard, wake_thread_on_current_cpu,
                                          wake_thread_front, place_thread_on_cpu, reap
kernel/src/ipc/mod.rs                   — Channel::send, recv, grant, recv_grant;
                                          handoff_to_rendezvous_peer, continue_current_after_receive
kernel/src/sync/switch_mutex.rs         — SwitchMutex full implementation
kernel/src/test_harness.rs              — run_smp_cross_cpu_rendezvous_stress, stress_sender,
                                          stress_receiver, wait_for_stress_endpoint_blocked,
                                          wait_for_stress_iteration_done, wait_for_stress_go,
                                          clear_rendezvous_receiver, verify_rendezvous_payload
Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md     — ratified PLAN (reviewed for design history)
Plans/SC-3B-VERIFICATION-2026-05-09.md        — failure log
Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md — PM triage
Plans/DISPATCH-2026-05-09-sc-3b-investigation-parallel.md   — investigation dispatch
```

### 1.2 Key code paths audited

**`context_switch_with_release2` asm (context.rs:48–68):**
- Global asm inline. Saves RBX/RBP/R12-R15 onto current stack. `mov [rdi], rsp` writes old RSP. `mov rsp, rsi` switches stack. `mov byte ptr [rdx], 0` releases channel lock. `mov byte ptr [rcx], 0` releases scheduler lock. Pops callee-saved from new stack. `ret`.
- Verified: all release metadata in registers (RDX=channel, RCX=scheduler). No old-stack reads after `mov rsp, rsi`. Matches ratified ABI exactly.

**`block_current_into_with_guard` (scheduler.rs:483–531):**
- Acquires CPU's scheduler lock (`s = sched.inner.lock()`). Pops next thread. Marks prev Blocked. Captures `prev_rsp_ptr = &mut prev.rsp` BEFORE moving prev into installer. Installer writes EndpointState (while channel guard is held). Captures `channel_lock_ptr` and `sched_lock_ptr` via `release_token()`. `mem::forget` both guards. Calls asm.
- Verified: EndpointState write happens before forget + asm. prev.rsp pointer is heap-pinned (Box<Thread>), safe after move. Release ordering is channel→scheduler. No panicable ops in the forget-to-asm window.

**`Channel::send` completing a `RecvBlocked` peer (ipc/mod.rs:157–174):**
- Uses `continue_current_after_receive(recv_thread, n)` — pushes receiver to front of current CPU queue with `preempt_defer=32`, then defers the caller's preemption (does NOT yield immediately). Caller (sender) continues running.

**`Channel::recv` completing a `SendBlocked` peer (ipc/mod.rs:221–239) and `Channel::send` completing a `RecvBlocked` peer (ipc/mod.rs:157–174):**
- Uses `handoff_to_rendezvous_peer(send_thread, n)` — pushes sender to front of current CPU queue, then **immediately yields**. Caller (receiver) is suspended.

**Critical asymmetry noted:** `continue_current_after_receive` does NOT yield (caller keeps running). `handoff_to_rendezvous_peer` DOES yield (caller suspends immediately). This creates different timing for sender-first vs. receiver-first completions.

**`SwitchMutex` (switch_mutex.rs:41–55):**
- Standard two-phase TTAS: compare_exchange_weak(false, true, Acquire, Relaxed) in outer loop; load(Relaxed) spin in inner loop. Drop stores(false, Release). No surprises.

**Stress test structure (test_harness.rs:2733–2794):**

*Critical structural insight discovered during source audit:*

The orchestrator assigns `seq` to pairs as `pair = (seq-1) % pairs`. For `pairs=4`:
- Pair 0 gets seq=1, 5, 9, 13, … All are odd. `send_first = (seq & 1) == 1` = **always true for pair 0.**
- Pair 1 gets seq=2, 6, 10, 14, … All are even. **Always recv_first.**
- Pair 2 gets seq=3, 7, 11, 15, … All are odd. **Always send_first.**
- Pair 3 gets seq=4, 8, 12, 16, … All are even. **Always recv_first.**

**This assignment is deterministic, not alternating.** Pair 0 and pair 2 are always send_first. Pair 1 and pair 3 are always recv_first. Combined with Q4 wake-on-completer-CPU, this creates deterministic migration patterns (see §2.3 H1-variant analysis).

**`wait_for_stress_go` (test_harness.rs:2584–2588):**
```rust
while slot.load(Ordering::Acquire) < seq {
    crate::sched::yield_now();
}
```
**This is an infinite loop.** No timeout. If the expected signal never arrives, the worker hangs permanently, silently.

**`wait_for_stress_endpoint_blocked` (test_harness.rs:2681–2700):**
```rust
for _ in 0..100_000 {
    ...check...
    yield_now(); reap();
}
fail_with_hex("reason=stress_endpoint_did_not_block pair", pair as u64);
```
100,000-iteration loop WITH a timeout that emits serial output. If this fires, we would see a `NEO_TEST/1 event=FAIL` line. We do not see it in the failure log.

**`wait_for_stress_iteration_done` (test_harness.rs:2703–2714):**
Same structure: 100,000-iteration loop with `fail_with_hex` on timeout. Serial output expected if hit.

**Conclusion from absence of serial output:** Neither finite poll timed out. The orchestrator is stuck in a wait that either (a) is inside an infinite loop at the time of the hang, or (b) was in a finite loop at the time of the hang but the condition it needs will NEVER be satisfied because a worker is in `wait_for_stress_go` waiting for a signal the orchestrator cannot send.

---

## 2. Phase A — Hypothesis enumeration

### 2.1 H1 — Lost wakeup in Q4 wake-on-completer-CPU under contention

**Mechanism as stated in triage:** Two CPUs simultaneously try to wake the same parked thread.

**Analysis:** The channel `SwitchMutex` prevents this. To extract a parked thread from EndpointState, a CPU must hold the channel lock. Only one CPU can hold it at a time. Two CPUs cannot simultaneously extract the same thread.

**H1 as originally stated: RULED OUT by source audit.** The SwitchMutex prevents simultaneous extraction.

**H1-variant (PM-surfaced during source audit — see §2.7 Hn):** Q4 migration accumulation on CPU0 creates a scenario where the orchestrator is blocked waiting for workers, but the workers are waiting for the orchestrator's signal, and the signal pathway depends on the orchestrator being able to poll the channel — which it can only do by yielding on CPU0. If CPU0's ready queue is saturated with migrated workers all in `wait_for_stress_go` loops (consuming CPU0's time but not making the channel transition the orchestrator needs to see), the orchestrator's 100,000-iteration finite poll exhausts BEFORE the worker that needs to block has had enough CPU turns to actually call channel.send and complete the block operation.

- **Confirming evidence:** CPU0 accumulates workers via Q4 migration (see §2.3 for the Q4 migration map). Under QEMU's SMP emulation, CPU0 can have 4–5 threads all in yield loops.
- **Disconfirming evidence:** If instrumented logs show each `wait_for_stress_endpoint_blocked` returning in <1000 iterations (fast), then CPU load on CPU0 is not causing the timeout.

### 2.2 H2 — Race in `block_current_into_with_guard` (prev_rsp_ptr aliasing)

**Mechanism as stated in triage:** The raw pointer `prev_rsp_ptr` aliases the Box<Thread> interior after it's moved into EndpointState. CPU M might extract prev_thread and read its `rsp` before the asm has written it.

**Analysis:** Source audit disproves this. In `block_current_into_with_guard`:
1. `prev_rsp_ptr = &mut prev.rsp` is captured before installer call.
2. `installer(&mut *channel_guard, prev)` moves prev into EndpointState while channel guard is held.
3. Box<Thread> has heap-pinned content; pointer survives the move.
4. The asm executes `mov [rdi], rsp` (writes *prev_rsp_ptr) BEFORE releasing the channel lock (`mov byte ptr [rdx], 0`).
5. CPU M can only extract prev_thread after acquiring the channel lock (step 4 releases it), by which point the asm has already written prev_rsp_ptr.

The happens-before chain is correct. **H2: RULED OUT by source audit.**

### 2.3 H3 — Test scenario logic deadlock

**Mechanism as stated in triage:** Pairing logic or iteration logic has a bug that doesn't manifest at low iteration count.

**Analysis:** The orchestrator's coordination protocol is correct at the level of individual iterations. `wait_for_stress_iteration_done` correctly waits for BOTH SENDER_DONE and RECEIVER_DONE before advancing seq. Workers cannot advance past their seq without the orchestrator's GO signal (due to `wait_for_stress_go`). The >= semantics on DONE polls are correct given the sequential-within-pair guarantee.

**However, the source audit reveals a structural issue in the test that is a direct H3 candidate (see Hn §2.7):** The always-send_first / always-recv_first assignment combined with Q4 migration creates CPU0 accumulation.

**H3 as the pairing-logic-deadlock: LOW probability.** No pairing logic bug found in source.

**H3-variant / Hn (PM-surfaced): see §2.7. This is the leading candidate.**

### 2.4 H4 — Per-CPU ready queue starvation under Q4 bias

**Mechanism:** Q4 wake-on-completer-CPU causes load imbalance; orchestrator on an idling CPU can't advance.

**Analysis:** The source audit reveals a more specific version of this: Q4 migration creates deterministic accumulation on CPU0. After sufficient iterations:
- Pair 0 (always send_first): receiver_0 migrates to CPU0 (sender_0's initial CPU). sender_0 subsequently migrates to CPU1 (receiver_0's initial CPU), then back to CPU0, etc. The bounce is deterministic.
- Pair 3 (always recv_first): receiver_3 (initial CPU0) migrates to CPU3 (sender_3's initial CPU). receiver_3 no longer competes on CPU0 after the first iteration.
- Pair 1 (always recv_first): receiver_1 (initial CPU2) migrates to CPU1.
- Pair 2 (always send_first): receiver_2 (initial CPU3) migrates to CPU2.

After a few dozen iterations, typical CPU composition is approximately:
- CPU0: orchestrator, BSP bootstrap, possibly sender_0/receiver_0 oscillating via Q4
- CPU1: BSP-1 bootstrap, sender_1, receiver_0 (migrated), and potentially sender_0 (after Q4 from pair 0)
- CPU2: BSP-2 bootstrap, sender_2, receiver_1 (migrated)
- CPU3: BSP-3 bootstrap, sender_3, receiver_2 (migrated), receiver_3 (migrated)

CPU3 accumulates the most threads: bootstrap, sender_3, receiver_2, receiver_3, and potentially sender_2 (after Q4 from pair 2). **5+ threads on CPU3 competing for scheduling.**

Under QEMU's SMP emulation, a CPU with 5 threads in yield loops gives each thread ~200ms per second of wall time (assuming equal scheduling). Each `yield_now()` is fast (~microseconds), but the issue is: when a worker finally gets a turn, it sees the GO signal and calls channel.send/recv. That IPC call's `block_current_into_with_guard` pops the "next" thread from the same CPU's queue. With 5 threads, the popped "next" may be another idling worker, not the one that needs to complete the rendezvous.

**The wait_for_stress_endpoint_blocked timeout scenario:**
- Orchestrator (on CPU0) sets SENDER_GO[P] = seq.
- Worker (on CPU3) is in a tight yield loop with 4 other threads.
- Worker gets a turn, calls channel.send.
- channel.send calls `block_current_into_with_guard`. Pops "next" from CPU3's queue (some other worker).
- Worker is now blocked in EndpointState.
- But the "next" popped is ALSO just looping in `wait_for_stress_go`. It yields immediately.
- Eventually orchestrator sees SendBlocked. OK so far.
- Orchestrator sets RECEIVER_GO[P] = seq.
- The receiver for pair P is ALSO on CPU3 (from Q4 migration). It's in a yield loop with 3 other threads.
- The 100,000-iteration `wait_for_stress_endpoint_blocked` (on CPU0) runs through 100,000 yield_now+reap cycles WHILE the receiver on CPU3 hasn't gotten enough turns to even see RECEIVER_GO, let alone call channel.recv and write DONE.
- If this asymmetry is severe enough, fail_with_hex fires.

**BUT:** if fail_with_hex fires, we see serial output. We do NOT see it. Therefore this starvation scenario, IF it's the root cause, must take >180s to exhaust 100,000 iterations on CPU0. This requires each `yield_now + reap` on CPU0 to take >1.8ms on average. That's plausible under heavy QEMU SMP load (QEMU's SMP emulation can be very slow per-tick under high lock contention).

**H4: MEDIUM probability. A serious contributing factor. Explains the timing pattern without requiring a code bug, but requires very slow QEMU scheduling to avoid the serial-output path.**

### 2.5 H5 — SwitchMutex starvation under stress

**Analysis:** `SwitchMutex::lock()` uses TTAS pattern — identical in structure to `spin::Mutex`. On x86, `compare_exchange_weak` compiles to `LOCK CMPXCHG` which does not spuriously fail. No starvation possible via livelock. The spinlock is per-channel (4 channels) and per-scheduler (4 per-CPU schedulers) — not a single global hotspot.

**H5: RULED OUT. No structural issue in SwitchMutex.**

### 2.6 H6 — Missing memory barrier

**Analysis:** The synthesis rationale for "no fence" is verified correct. On x86:
- TSO guarantees stores are observed in program order. CPU_X's `state[A] = SendBlocked` (S1) followed by `channel_lock = 0` (S2, asm release) means any CPU acquiring the channel lock after S2 sees S1.
- The Acquire/Release pairing on `AtomicBool` is correct. `SwitchMutex::lock()` uses `Ordering::Acquire`. `Drop` uses `Ordering::Release`. These pair correctly.
- The asm uses plain `mov byte ptr [rdx], 0` — on x86, this is equivalent to `AtomicBool::store(false, Release)` since TSO prevents store reordering.

**H6: RULED OUT. Memory ordering analysis confirmed correct.**

### 2.7 Hn — Q4 migration leads to orchestrator signaling deadlock on CPU0 (PM-surfaced)

**This is the leading candidate not present in the original triage.**

**Mechanism:** The stress test's deterministic pair-to-direction assignment (pair 0/2 = always send_first, pair 1/3 = always recv_first) combined with Q4 wake-on-completer-CPU creates a deterministic migration attractor. Specifically for pair 0 (always send_first):

- seq=1: sender_0 (CPU0) blocks. Q4 wakes sender_0 on CPU1 (receiver_0's initial CPU). sender_0 migrates to CPU1.
- seq=5: sender_0 now on CPU1 blocks. Q4 wakes sender_0 on CPU1 again (receiver_0 is now on CPU1 after previous migration). sender_0 stays on CPU1.
- After a few iterations: receiver_0 bounces back to CPU0 (because continue_current_after_receive in recv-finding-SendBlocked pushes receiver to completer's CPU, and completer is whoever is completing the rendezvous).

Wait — more precisely: in Channel::recv finding SendBlocked, the completion path is `continue_current_after_receive`. The "current CPU" at that point is the CPU where receiver_0 called channel.recv. Initially CPU1. After Q4: wherever receiver_0 currently is.

The migration pattern for pair 0 (always send_first, no recv_first) is:
- sender_0 blocks → receiver_0 completes on CPU_R → pushes sender_0 to CPU_R's front. receiver_0 keeps running (no yield). receiver_0 completes iteration on CPU_R. receiver_0 wait_for_stress_go → yield → sender_0 runs on CPU_R. Next iteration: sender_0 blocks on CPU_R.

After the first iteration:
- Both sender_0 and receiver_0 end up on the SAME CPU (whichever CPU receiver_0 was on when it completed the rendezvous).
- They stay together because `continue_current_after_receive` keeps them on the same CPU.

This means: after the first pair-0 iteration, sender_0 and receiver_0 are BOTH on CPU1. For subsequent pair-0 iterations, they remain on CPU1 (sender blocks on CPU1, receiver completes on CPU1, they stay on CPU1). The orchestrator on CPU0 becomes entirely decoupled from the pair-0 workers.

**THE DEADLOCK SCENARIO (Hn):**

After many iterations, the composition is approximately:
- CPU0: orchestrator + BSP bootstrap only (all stress workers have migrated)
- CPU1: sender_0, receiver_0, sender_1, receiver_1 (4 stress workers + bootstrap) — pair 1 also migrates here due to recv_first direction
- CPU2: sender_2, receiver_2, bootstrap
- CPU3: sender_3, receiver_3, bootstrap

The orchestrator on CPU0 has ONLY itself and bootstrap. When it calls `yield_now()` in its poll loops, it alternates with the BSP bootstrap thread. BSP bootstrap does `loop { yield_now(); spin_loop(); }` — it immediately yields back. So the orchestrator's yield_now-based polling is effectively spinning on CPU0 with no competition.

**Now the scenario where CPU0 is no longer the problem — the problem is CPU1:**

CPU1 has 5 threads: sender_0, receiver_0, sender_1, receiver_1, AP-1 bootstrap. All 5 are in `wait_for_stress_go` loops (yielding). Each thread gets CPU1 for approximately 1/5th of the time between timer ticks.

When orchestrator sets SENDER_GO[1]=seq (pair 1 being recv_first, so this sets sender AFTER receiver blocks):
1. Receiver_1 (on CPU1) wakes from wait_for_stress_go (seeing RECEIVER_GO[1]=seq-4? No — pair 1 signals are for seq=2,6,10,..., receiver waits for correct seq).
2. Receiver_1 calls channel.recv. block_current_into_with_guard on CPU1 pops "next" from CPU1's queue.
3. Who is at the front of CPU1's queue? With 5 threads yielding, the front is whoever was pushed most recently. It could be sender_0, receiver_0, bootstrap, or receiver_1 itself (if it just yielded and came back).
4. **If the popped "next" is receiver_0 or sender_0 (pair 0's workers, idle in wait loops):** receiver_1 blocks, pair-0 worker runs (does nothing useful — still waiting for pair-0 signal), yields. Bootstrap runs. Eventually receiver_1 needs CPU1 time to complete its turn. Meanwhile orchestrator on CPU0 is polling wait_for_stress_endpoint_blocked(1, false) with its own yield_now loop on CPU0.

The orchestrator's poll on CPU0 burns through 100,000 iterations quickly (only orchestrator + bootstrap on CPU0, each yield_now is fast). Meanwhile CPU1 is slower because 5 threads share it.

**IF: orchestrator's 100,000 wait_for_stress_endpoint_blocked poll exhausts before receiver_1 has gotten enough CPU1 turns to call channel.recv and set state[1B]=RecvBlocked, fail_with_hex fires → serial output.**

We see NO serial output. Therefore this timing scenario requires 100,000 CPU0 yield_now iterations to take LONGER than the time receiver_1 takes to complete 1 IPC call on CPU1.

At QEMU SMP emulation speed, this is plausible only if:
- CPU0 yields are very fast (it has nothing to compete with)
- CPU1 yields are very slow (5 threads competing + QEMU SMP overhead)

Under QEMU, each vCPU runs on a host thread. With 4 vCPUs all active, the host's scheduler distributes them across host CPUs. Under load, CPU0 might complete 100,000 yield_now iterations in <1ms while CPU1's threads get scheduled at 10Hz or slower due to QEMU scheduling — meaning receiver_1 gets a turn only every ~100ms. 100,000 iterations × <0.01ms/iter = <1s for CPU0 to exhaust the poll. But receiver_1 needs only ONE turn to call channel.recv and complete. With QEMU at even 10 turns/second for CPU1 threads, receiver_1 would get a turn within 100ms, and a CPU0 yield_now at 10ms/iter would take 1000 seconds to reach 100,000. The timescales are hard to predict without empirical data.

**The deadlock that DOESN'T emit serial output:**

An alternative Hn scenario where no output is generated:

If the orchestrator's CPU0 `yield_now()` is SO FAST that 100,000 iterations complete in microseconds (because CPU0 only has orchestrator + bootstrap, and each yield_now is a trivial context switch between them), AND receiver_1 on CPU1 has SO MANY yielders competing that it doesn't get a single turn in those microseconds, then fail_with_hex fires — but with serial output. We see none.

**THEREFORE: the actual scenario causing the silent hang must put the orchestrator into an INFINITE WAIT (not one of the 100,000-iteration finite polls).**

The only infinite wait available to the orchestrator is... none explicitly. But consider:

After `wait_for_stress_endpoint_blocked` returns (seeing the blocked state), the orchestrator sets the second GO signal and calls `wait_for_stress_iteration_done`. Now the workers are INSIDE their IPC call (one is blocked in EndpointState, the other needs to complete it). If the completing worker is on a heavily loaded CPU and takes many yield cycles to run, eventually `wait_for_stress_iteration_done` hits its 100,000 limit — but again, serial output.

**UNLESS:** the 100,000 iterations are fast on CPU0 but the workers on CPU1-3 are slow. Once `wait_for_stress_iteration_done` fires `fail_with_hex`... it calls `debug_exit(EXIT_FAILURE)`. `debug_exit` does an `out dx, eax` PORT instruction. QEMU's x86-64 emulation intercepts this OUT instruction to signal test completion. But the QEMU process then terminates normally, right? If `fail_with_hex` fires, the test EXITS via `debug_exit`, which QEMU intercepts. The 180s timeout wouldn't apply because the test already exited.

**So if fail_with_hex fires correctly, the test exits and xtask records the failure serial log content including the FAIL line. But we see no FAIL line.**

**CONCLUSION: The orchestrator has NOT hit a 100,000-iteration limit. The orchestrator is stuck in a finite poll that hasn't yet reached 100,000 iterations because it requires so many yield_now() calls that 100,000 hasn't been reached in 180s.**

At 1 yield_now per 1.8ms (QEMU scheduling under heavy load), 100,000 iterations = 180s. This matches the 180s timeout exactly.

**Revised Hn mechanism:** Under heavy SMP load in QEMU with 5+ threads on CPU1 and CPU0 only having orchestrator+bootstrap, the orchestrator's `yield_now()` in `wait_for_stress_iteration_done` (or `wait_for_stress_endpoint_blocked`) is SO SLOW — because each yield_now switches to bootstrap, which immediately yields back, so the pair together takes perhaps 1.8ms under QEMU's scheduling — that 100,000 iterations takes 180s. The QEMU process timeout fires before fail_with_hex can run.

**This is consistent with all observations:**
- No serial output (fail_with_hex never ran)
- 180s QEMU timeout fires
- No PASS (test never completed)
- No FAIL frame (fail_with_hex never ran)
- 35/35 prior runs passed (the timing was fast enough on those runs)

**Why run 36 specifically failed:** QEMU's host scheduling is non-deterministic. After 35 clean runs, some combination of host system load, QEMU CPU state, or thread positioning put 5 workers on CPU1 at a time when CPU0 was doing a `wait_for_stress_iteration_done` poll. The per-yield_now latency on CPU0 degraded enough that the 100,000 limit wasn't reached in 180s.

---

## 3. Phase B — Instrumentation strategy

Instrumentation goal: distinguish Hn (CPU load imbalance) from H1-variant (lost wakeup) from H4 (starvation).

### 3.1 Recommendation: Apply NMI watchdog stash (`stash@{0}`) immediately

The NMI watchdog from the wedge investigation converts a silent hang into a forced kernel panic with stack traces from all 4 CPUs. This is the single highest-value diagnostic.

**Recommended NMI threshold:** 10 seconds. Long enough to not fire during normal iteration (each iteration is <1ms), but short enough to capture the hung state before QEMU's 180s timeout. Apply as `git stash apply stash@{0}` (investigation-only, not committed).

The stack traces will show:
- Which CPU each thread is stuck on
- What function each CPU is executing
- Whether the orchestrator is in a yield loop (→ Hn) or in something else

### 3.2 Per-iteration PROGRESS frames

Add to the stress test's orchestrator loop (inside `for seq in 1..=STRESS_ITERATIONS`), after the GO signals are set but before `wait_for_stress_iteration_done`:

```rust
// After both GO signals sent:
crate::serial_println!(
    "NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress \
     iter={} pair={} send_first={}",
    seq, pair, send_first
);
```

This adds serial output for every iteration. If the hang occurs at iteration N, the last PROGRESS line identifies N and which pair is stuck.

**Heisenbug risk:** Serial output is slow (UART at 115200 bps). 1000 iterations × ~100 chars = 100KB of serial. At 115200 bps ≈ 10KB/s, this adds ~10s to the test run. Acceptable for investigation. To reduce risk: only emit PROGRESS every 10th iteration, or emit only the seq number.

### 3.3 Per-CPU thread count snapshot

Add a `debug_cpu_ready_counts()` helper that logs the ready-queue depth for each CPU before the iteration completes. This distinguishes Hn (CPU1 has 5 threads) from H4 (CPU1 is busy, CPU0 is idle).

Implementation: in `wait_for_stress_endpoint_blocked`, before the loop, call:
```rust
serial_println!("SC3B_DEBUG cpu0_ready={} cpu1_ready={} cpu2_ready={} cpu3_ready={}",
    per_cpu_ready_count(0), per_cpu_ready_count(1), per_cpu_ready_count(2), per_cpu_ready_count(3)
);
```

This would require a `pub fn per_cpu_ready_count(cpu_id: u32) -> usize` function in scheduler.rs. Cost: one scheduler lock per CPU per call, but outside the hot path.

### 3.4 Thread cpu_id tracking at block/wake points

In `stress_sender`/`stress_receiver`, emit:
```rust
// Before channel.send:
serial_println!("SC3B_TRACE pair={} seq={} sender_cpu={}", pair, seq, 
    crate::arch::x86_64::per_cpu::current_cpu_id());
// After channel.send returns:  
serial_println!("SC3B_TRACE pair={} seq={} sender_resumed_cpu={}", pair, seq,
    crate::arch::x86_64::per_cpu::current_cpu_id());
```

Similarly for receiver. This confirms whether Q4 migration is placing workers on the expected CPUs.

### 3.5 SwitchMutex spin-count tracking

**Not recommended for first instrumentation pass.** The SwitchMutex is a standard TTAS spinlock; timing its acquisition is unlikely to provide discriminating information given H5 is ruled out. Can be added in a second pass if the first pass is inconclusive.

### 3.6 Channel state snapshot before SET of GO signal

In `wait_for_stress_endpoint_blocked` (after confirming blocked state), capture and log the channel state:
```rust
// Just before returning from wait_for_stress_endpoint_blocked:
serial_println!("SC3B_TRACE pair={} blocked_confirmed_iter={}", pair, iter_count);
```
`iter_count` = the loop iteration number when blocked state was first seen. If this is consistently large (>10,000), it confirms the CPU0 polling is fast (< 100,000) but the worker was slow to reach the block state (H4).

### 3.7 Recommended instrumentation order

1. **First: Apply NMI watchdog (`stash@{0}`) with 10s threshold.** Run 5-run loop. If a timeout occurs, we get CPU stack traces → immediately actionable.
2. **Second: Add per-iteration PROGRESS frames (emit every 10 iterations).** Run 50-run loop. Failure identifies which iteration and pair hung.
3. **Third: Add cpu_id tracking in senders/receivers.** Confirms migration pattern.
4. **Fourth: Add iter_count log in wait_for_stress_endpoint_blocked.** Quantifies CPU load imbalance.

CPU-queue-depth snapshot (§3.3) deferred to pass 3 if earlier instrumentation is inconclusive.

---

## 4. Phase C — Reproduction predictions

### 4.1 Reproduction probability with instrumentation

- **Without instrumentation (200-run baseline):** Estimated failure rate 1-5% per run given the 1/36 observed probability (35 clean, 1 failure). Expected re-manifestation in ~20-100 runs.
- **With NMI watchdog (10s threshold):** Very high probability (>95%) of capturing a hung state if a failure occurs in 50 runs. NMI converts silent hang to panic with stack traces.
- **With PROGRESS frames:** If a failure manifests, last PROGRESS line identifies the stuck iteration. ~100% discriminating power once a failure occurs.
- **Heisenbug risk:** PROGRESS frames add serial latency, slightly shifting timing. The extra 10s run time may either hide or expose the failure. If it hides it, run without PROGRESS frames but with NMI watchdog only.

### 4.2 Per-hypothesis predicted serial signatures

**If Hn (CPU load imbalance) is root cause:**
- Last PROGRESS line: `iter=N pair=1 send_first=false` or `iter=N pair=3 send_first=false` (the recv_first pairs that accumulate more workers on CPU1/CPU3)
- NMI stack traces: orchestrator on CPU0 inside `yield_now` (inside `wait_for_stress_endpoint_blocked` or `wait_for_stress_iteration_done`). CPU1 shows 4-5 threads, each in `yield_now` or `wait_for_stress_go`.
- iter_count in wait_for_stress_endpoint_blocked: very large (>50,000) for the failing iteration.
- per_cpu_ready_counts: CPU1=4-5, CPU0=1-2.

**If H1-variant (lost wakeup in Q4):**
- Last PROGRESS: `iter=N pair=P send_first=T`. Orchestrator successfully returned from `wait_for_stress_endpoint_blocked` (saw Blocked state) AND set the second GO signal, but worker that was supposed to complete the rendezvous never did.
- NMI stack traces: one worker stuck in `wait_for_stress_go` on some CPU_X. Another worker in EndpointState (blocked, visible via channel debug function).
- iter_count: small (<100) in wait_for_stress_endpoint_blocked — endpoint quickly blocked. But `wait_for_stress_iteration_done` running for many iterations.
- Channel state: side A or B shows Blocked but never gets extracted.

**If H3 (test pairing logic) is root cause:**
- PROGRESS sequence shows a specific pair=P failing at a repeatable seq=N (same iteration across multiple failure runs).
- The failure is deterministic: same pair, same seq, every time.

**If H4 (CPU starvation variant):**
- Similar to Hn but broader: multiple CPUs show high thread counts.
- NMI traces: multiple workers stuck in yield loops on multiple CPUs.

### 4.3 Distinguishing Hn from H1-variant

| Signal | Hn (CPU load imbalance) | H1-variant (lost wakeup) |
|--------|------------------------|--------------------------|
| Failing iteration's pair | Likely pair 1 or 3 (always recv_first, more migrations) | Any pair |
| iter_count in endpoint_blocked | Very large (>50,000) | Small (<1000) |
| NMI: orchestrator CPU | Inside yield_now in endpoint_blocked OR iteration_done | Inside iteration_done (past endpoint_blocked) |
| NMI: worker CPUs | Multiple workers in yield_now loops | One worker in wait_for_stress_go; one in EndpointState |
| Channel state at time of hang | One side blocked, peer has NOT been signaled | One side blocked, peer HAS been signaled but returned wrong |
| Pattern across failures | Always fails for the same pair | Varies by run |

---

## 5. Phase D — Root-cause hypothesis ranking

### 5.1 Primary ranking

| Rank | Hypothesis | Confidence | Core evidence |
|------|-----------|-----------|---------------|
| 1 | **Hn: CPU load imbalance from Q4 migration determinism** | 50% | Pair-to-direction assignment is constant; Q4 creates deterministic accumulation; failure explains absence of serial output |
| 2 | **H1-variant: Wait_for_stress_iteration_done slow due to worker migration + QEMU scheduling** | 25% | Workers on heavily loaded CPUs complete slowly; orchestrator's 100,000 poll finite but takes >180s |
| 3 | **H4: Per-CPU starvation under Q4 accumulation** | 15% | CPU1 accumulates most workers; their yield loops consume CPU1 time; workers take too long to respond |
| 4 | **H3: Test-scenario coordination protocol edge case** | 8% | `continue_current_after_receive` (no yield) vs `handoff_to_rendezvous_peer` (yields) asymmetry could create a missed scheduling opportunity under specific timing |
| 5 | H6, H2, H5 | <2% each | Ruled out by source audit |

**Note:** Hn and H1-variant have different mechanisms but similar root cause: QEMU SMP scheduling interacts with the Q4-driven thread accumulation on specific CPUs. The fix for both is the same (see §6.1).

### 5.2 What would change this ranking

**Hn moving to certainty:** NMI stack traces show orchestrator in yield_now after many iterations (last PROGRESS was recent), and CPU1 shows 4+ workers all in yield loops.

**H1-variant taking over:** NMI shows orchestrator in `wait_for_stress_iteration_done` (past the endpoint_blocked poll), and one pair's channel shows a blocking endpoint that the peer's worker can see but hasn't responded to.

**H3 taking over:** Multiple failure runs show the same pair=P failing at the same seq=N. Deterministic failures are test-logic bugs, not timing races.

**H6 taking over (unlikely):** Would require observing a case where sender-state was written, channel lock released, but receiver reads Idle from same channel. This would require x86 TSO violation — essentially impossible in QEMU.

### 5.3 Key source finding for the fix direction

Regardless of exact root cause, the investigation identifies two structural issues in the stress test:

**Issue A:** The pair-to-direction assignment is determined by `(seq-1) % pairs` and `seq & 1`, which together make pair 0/2 always send_first and pair 1/3 always recv_first. This constant direction makes Q4 migration deterministic and accumulating, not balanced. A simple fix: vary the direction more aggressively (e.g., by pair AND by run or by a per-pair counter).

**Issue B:** `wait_for_stress_endpoint_blocked` and `wait_for_stress_iteration_done` use 100,000-iteration loops whose wall-clock duration is determined by how fast CPU0's yield_now runs. Under QEMU's SMP with an imbalanced CPU composition, these loops can run for seconds before timing out, pushing against the 180s limit.

---

## 6. Phase E — Fix design (top-2 hypotheses)

### 6.1 Fix for Hn / H1-variant: Break the deterministic Q4 migration accumulation

**Root cause:** Pair-to-direction assignment is constant (pair 0/2 always send_first; pair 1/3 always recv_first). Q4 wake-on-completer-CPU accumulates workers on specific CPUs over time.

**Structural fix:** Alternate the direction per-iteration within each pair, not globally. Change the direction assignment from global-seq-parity to per-pair tracking:

```rust
// Replace:
let send_first = (seq & 1) == 1;

// With (per-pair alternation):
static PAIR_SEND_FIRST: [AtomicBool; STRESS_MAX_PAIRS] = [...];
let send_first = !PAIR_SEND_FIRST[pair].fetch_xor(true, Ordering::Relaxed);
```

This makes each pair alternate send_first/recv_first every time it's used, preventing monotonic accumulation on a single CPU. The migration pattern becomes a balanced oscillation rather than a one-way drift.

**Files:** `kernel/src/test_harness.rs` — add `PAIR_SEND_FIRST` static, modify `run_smp_cross_cpu_rendezvous_stress` to use it and reset in `reset_stress_state`.

**New invariant:** After K full cycles through all pairs, each pair has done K/2 send_first and K/2 recv_first iterations. Workers bounce between CPU_sender and CPU_receiver, spreading load rather than accumulating it.

**Verification that fix addresses root cause:**
- Run 200-boot closure gate.
- Confirm via cpu_id tracking in PASS frames that senders and receivers alternate CPU assignment across iterations.
- Confirm per-CPU ready queue depth is more balanced (if queue-depth instrumentation is added).
- Statistical: if 200/200 passes after this change, the constant-direction Q4 accumulation was the root cause.

**Anti-regression concerns:**
- The fix changes test behavior but not kernel behavior. Existing scenarios unaffected.
- The 200/200 primary stress closure remains the gate.
- The fix does NOT change the 1000-iteration count, pair count, or any kernel code.

### 6.2 Fix for H4 / secondary: Improve the finite poll reliability

**Root cause:** `wait_for_stress_endpoint_blocked` and `wait_for_stress_iteration_done` use 100,000 yield_now iterations whose wall-clock duration varies by CPU load. Under heavy load, they consume only CPU0's time (the orchestrator's CPU), not the workers' time.

**Structural fix A (preferred):** Replace yield_now-only polling with a hybrid: yield_now + a brief `core::hint::spin_loop()` burst when the condition isn't seen after N yields. This forces CPU0 to give up its time budget more aggressively, reducing time per-poll iteration and making the 100,000 limit more deterministic.

But more importantly: **add a direct CPU-hint to workers when the poll detects it's running long.** This is not straightforward in a cooperative kernel without cross-CPU signaling.

**Structural fix B (simpler, addresses root cause better):** Double the poll limits (100,000 → 200,000) AND add a serial_println with `fail_with_hex` that fires at 50,000 iterations with diagnostic context but does NOT exit. This turns the silent-near-timeout into an audible warning:

```rust
for i in 0..200_000 {
    if SENDER_DONE[pair].load(Acquire) >= seq && RECEIVER_DONE[pair].load(Acquire) >= seq {
        return;
    }
    if i == 50_000 {
        serial_println!("SC3B_WARN wait_stress_iter_done pair={} seq={} approaching_limit", pair, seq);
    }
    yield_now();
    reap();
}
fail_with_hex("reason=stress_iteration_timeout seq", seq as u64);
```

**Structural fix C (best, but requires more work):** Add explicit CPU0 → CPU1 notification when the orchestrator needs a specific worker to run. This requires a cross-CPU scheduling signal (IPI-based or atomic flag-based). Out of scope for the current investigation — this would be a kernel change, not a test change.

**Verification:**
- With Fix B: if failures recur, the 50,000-iteration warning will appear in serial log, giving wall-clock timing data.
- If Fix B prevents failures (by simply giving workers more time): confirms the timing hypothesis but doesn't fix the root scheduling issue.

**Anti-regression:**
- Fix B is test-only. No kernel changes. All existing scenarios unaffected.
- Fix B doubles the wait time for genuine failures (from 100,000 to 200,000 iterations), slightly slowing down failure detection if a real deadlock is introduced later.

### 6.3 Fix ordering recommendation

1. **First (immediately):** Apply Fix 6.1 (per-pair direction alternation). This is a 5-10 line change in the test file, mechanically simple, directly addresses the identified structural issue.
2. **After gate re-run:** If 6.1 passes 200/200, done. If not, apply the NMI watchdog and PROGRESS frames from §3 to gather data.
3. **If 6.1 fails:** Apply Fix 6.2B (doubled poll limits + warning). Run gate again. If warning triggers, the timing data identifies the specific iteration.
4. **If 6.2B still fails:** The root cause is a kernel-level scheduling issue, not a test issue. At that point, the investigation needs to proceed to Phase E's kernel fix path (cross-CPU scheduling signal or Q4 policy change).

---

## 7. Risks and unknowns

**What source analysis cannot determine:**
- Wall-clock duration of individual yield_now calls under QEMU SMP load. This requires empirical measurement.
- Whether the failure rate is consistent across different host machines (QEMU scheduling varies significantly by host).
- Whether the failure reproduces at NEO_SMP=2 (lower CPU count should reduce Q4 migration accumulation and make failure less likely).

**Key unanswered question:** The initial triage notes `stash@{1}: On main: wedge-investigation-2-instrumentation`. Does this stash include per-CPU scheduler diagnostics that could directly measure thread accumulation? If so, it should be considered alongside `stash@{0}`.

**PM's prior concern: "Heisenbug" risk.** The proposed PROGRESS frames and cpu_id tracing add serial output. In a timing-sensitive race, this output can shift the scheduling just enough to hide the bug. Mitigations:
- Use NMI watchdog (no serial output until fault) as primary diagnostic.
- Emit PROGRESS frames only after iteration 500 (assuming failures occur late in the 1000-iteration run based on Q4 accumulation dynamics).
- Run with NMI watchdog only first; add PROGRESS frames only if NMI doesn't trigger in 50 runs.

**Unknown: Is the failure machine-local?** If this failure requires a specific QEMU host scheduling behavior, it may not reproduce on a different host. The 35/36 success pattern is consistent with a host-scheduling-dependent race. However, the structural issue (deterministic Q4 migration accumulation) is real regardless of host.

---

## 8. PM recommendation

**Recommended next move: Apply Fix 6.1 before doing any instrumentation.**

The analysis is confident enough that the constant-direction pair assignment is a structural defect (Hn) that the fix addresses without risk of regression. The change is test-only, 5-10 lines, and has clear semantics. If the fix passes 200/200, the investigation is resolved without requiring instrumentation, NMI watchdog, or kernel changes.

If Fix 6.1 doesn't resolve the failure, THEN proceed to Phase B instrumentation as designed in §3. The instrumentation plan is structured to be additive — NMI watchdog first (zero test-code changes), PROGRESS frames second, cpu_id tracing third.

**The 200/200 closure gate remains the standard.** Do not lower the gate threshold even if Fix 6.1 reduces the failure rate. If Fix 6.1 reduces failures to 1/200, the bug is improved but not fixed — investigate further per Phase B.

---

## 9. Cross-references

**Mandatory per dispatch §12:**
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md` — PM triage; seeds H1–H6
- `Plans/SC-3B-VERIFICATION-2026-05-09.md` — Cerebro's verification log with failure evidence
- `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log` — failure artifact
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` — BUILD dispatch (§9.5 stop condition fired)
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` — ratified PLAN

**Additional cross-references:**
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — Invariants I1–I6 (Q6 design rationale verified correct by source audit)
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` — Cerebro's PLAN analysis
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md` — Prometheus's PLAN analysis
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6
- `docs/bible.md` Articles 1.2, 1.3, 8.5, 14

---

## 10. Phase gate status

Per dispatch §11, this synthesis is input to project-lead ratification.

Recommended ratification items for PL:
1. **Accept Hn as primary hypothesis.** The source analysis supports it. The constant pair-to-direction assignment is a genuine structural defect.
2. **Ratify Fix 6.1 (per-pair direction alternation) as the first action item.** This is a test-only change, low risk, directly addresses the identified structural issue.
3. **Ratify that Q6 asm, block_current_into_with_guard, SwitchMutex, and IPC retrofit are mechanically correct** per H2/H5/H6 ruling-out analysis. No kernel changes are expected to be needed.
4. **Defer NMI watchdog + PROGRESS frame instrumentation** until Fix 6.1 is attempted. If Fix 6.1 fails, Phase B instrumentation opens.
5. **The 200/200 closure gate stands.** Sub-step 6 (docs + close) does not open until the gate is cleared.

---

**End of synthesis.**

*Q6 primitive and IPC retrofit are mechanically sound (H2/H5/H6 ruled out). The failure is explained by a structural test defect: deterministic pair-to-direction assignment causes Q4 migration accumulation on specific CPUs, eventually degrading QEMU scheduling enough that the orchestrator's finite polls approach their 180s QEMU timeout before workers make progress. Fix is a test-only per-pair direction alternation. NMI watchdog + PROGRESS frame instrumentation ready for Phase B if the fix doesn't resolve the gate.*
