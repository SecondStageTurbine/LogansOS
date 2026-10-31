# SC-3B source audit findings

Date: 2026-05-09
Status: Hp confirmed by source audit

## Substrate

HEAD:

```text
a61e318d93321c9e84732869f5de9da96abe3d5f
```

Tracked working tree before audit:

```text
git status --short -uno
[no output]
```

Stash list at audit start:

```text
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

No kernel source files were modified. No stash was applied.

## §2.1 place_thread_on_cpu

Source: `kernel/src/sched/scheduler.rs`

```rust
#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn place_thread_on_cpu(thread: Box<Thread>, target_cpu: u32) -> Result<(), &'static str> {
    without_interrupts(|| {
        let idx = target_cpu as usize;
        if idx >= per_cpu::MAX_CPUS {
            return Err("target_cpu_exceeds_max");
        }

        let ptr = PER_CPU[idx]
            .scheduler_ptr
            .load(Ordering::Acquire)
            .cast::<PerCpuScheduler>();
        if ptr.is_null() {
            return Err("target_scheduler_not_initialized");
        }

        // SAFETY: non-null scheduler_ptr values are installed by
        // init_cpu_scheduler via Box::into_raw and live for kernel lifetime.
        let sched = unsafe { &*ptr };
        let mut s = sched.inner.lock();
        stamp_thread_cpu(&thread, target_cpu);
        debug_assert!(
            s.ready.capacity() > s.ready.len(),
            "scheduler-held ready push would allocate (cap={}, len={})",
            s.ready.capacity(),
            s.ready.len()
        );
        s.ready.push_back(thread);
        Ok(())
    })
}
```

Lock acquisition pattern:

```text
without_interrupts() on the calling CPU.
Then target CPU scheduler lock: sched.inner.lock().
No current-CPU scheduler lock is taken unless target_cpu equals current CPU.
```

Enqueue mechanism:

```text
s.ready.push_back(thread)
```

Cpu_id update:

```rust
stamp_thread_cpu(&thread, target_cpu);
```

`stamp_thread_cpu`:

```rust
fn stamp_thread_cpu(thread: &Thread, cpu_id: u32) {
    thread.cpu_id.store(cpu_id, Ordering::Relaxed);
}
```

IPI or wake-signal:

```text
No IPI send.
No wake_target_cpu / notify_cpu / send_resched call.
No helper call after enqueue.
The function returns immediately after push_back.
```

Cross-reference:

```text
No helper called by place_thread_on_cpu performs an IPI. It directly loads PER_CPU[target].scheduler_ptr, locks sched.inner, stamps cpu_id, pushes to ready, and returns.
```

Additional observation:

```text
The helper is feature-gated behind `neo-test`. It validates target_cpu < MAX_CPUS and target scheduler initialized, not a separate active CPU count.
```

## §2.2 wake_thread_on_current_cpu and Q4 helpers

Source: `kernel/src/sched/scheduler.rs`

```rust
/// Phase 1.5b: re-enqueue a previously-blocked thread with a return value.
///
/// The matching peer (or the process-exit cleanup path) calls this after
/// extracting the parked Box from its holder. Sets the thread's state to
/// `Ready`, writes `return_value` into its `blocked_return` slot (which
/// the resumed thread will read once `block_current_into` returns), and
/// pushes onto the back of the current CPU's ready queue.
///
/// Caller must already hold `without_interrupts` (every IPC syscall does).
pub fn wake_thread(mut thread: Box<Thread>, return_value: i64) {
    prepare_woken_thread(&mut thread, return_value);
    let sched = current_scheduler();
    let mut s = sched.inner.lock();
    stamp_thread_cpu(&thread, sched.cpu_id);
    debug_assert!(
        s.ready.capacity() > s.ready.len(),
        "scheduler-held ready push would allocate (cap={}, len={})",
        s.ready.capacity(),
        s.ready.len()
    );
    s.ready.push_back(thread);
}

/// Same as `wake_thread`, but the resumed thread is placed at the front of
/// the ready queue. IPC rendezvous completion uses this with an immediate
/// yield to implement the documented same-CPU handoff invariant: the blocked
/// peer resumes before the completing syscall returns to userspace. The
/// bounded defer credit lets the peer reach its next syscall before a stale
/// timer tick makes it observable as Ready to the original producer.
pub fn wake_thread_front(mut thread: Box<Thread>, return_value: i64) {
    prepare_woken_thread(&mut thread, return_value);
    thread.preempt_defer_ticks = HANDOFF_PREEMPT_DEFER_TICKS;
    let sched = current_scheduler();
    let mut s = sched.inner.lock();
    stamp_thread_cpu(&thread, sched.cpu_id);
    debug_assert!(
        s.ready.capacity() > s.ready.len(),
        "scheduler-held ready push would allocate (cap={}, len={})",
        s.ready.capacity(),
        s.ready.len()
    );
    s.ready.push_front(thread);
}

/// Q4 rendezvous wake helper: enqueue the parked peer on the completer's
/// current CPU and give it front-of-queue handoff priority.
pub fn wake_thread_on_current_cpu(thread: Box<Thread>, return_value: i64) {
    wake_thread_front(thread, return_value);
}
```

Does it ever push to a non-current CPU's queue?

```text
No. wake_thread and wake_thread_front both call current_scheduler().
wake_thread_on_current_cpu is a thin wrapper over wake_thread_front.
```

Does it cause a yield/reschedule on the current CPU?

```text
No, not directly. wake_thread_on_current_cpu only enqueues.
The immediate yield is performed by IPC helper code after this call.
```

IPC call sites:

```rust
fn handoff_to_rendezvous_peer(thread: Box<Thread>, return_value: i64) {
    // Q4: enqueue the blocked peer on the completer's current CPU. The
    // immediate yield preserves ADR-011's consumer-first handoff behavior.
    sched::wake_thread_on_current_cpu(thread, return_value);
    sched::yield_now();
}

fn continue_current_after_receive(thread: Box<Thread>, return_value: i64) {
    // When a receiver completes a previously-blocked sender/grantor, the
    // consumer of the payload is the current thread. Let it reach the next
    // syscall (usually a follow-on block) before the producer observes it as
    // Ready again.
    sched::wake_thread_on_current_cpu(thread, return_value);
    sched::defer_current_preempt_for_handoff();
}
```

Sibling helpers that push cross-CPU:

```text
Only place_thread_on_cpu explicitly targets a non-current CPU. It is test-feature-gated.
wake_thread and wake_thread_front are current-CPU helpers.
push_ready and spawn also push to current_scheduler().
```

## §2.3 AP-N-main

AP entry source: `kernel/src/arch/x86_64/smp.rs`

```rust
/// Limine AP handoff target. Must never return.
unsafe extern "C" fn ap_entry(info: &MpInfo) -> ! {
    // CR0/CR4 are CPU-local. Enable SSE/SMEP/SMAP before this AP executes any
    // meaningful Rust path that might use compiler-emitted SIMD or user-access
    // policy state.
    unsafe {
        x86_64::enable_sse();
    }

    let cpu_id = info.extra_argument() as u32;
    if cpu_id as usize >= per_cpu::MAX_CPUS {
        serial_println!(
            "[ kernel ] smp: AP invalid cpu_id={} (lapic_id={}, processor_id={}) - halting",
            cpu_id,
            info.lapic_id,
            info.processor_id
        );
        halt_forever();
    }

    per_cpu::init_ap(cpu_id);

    let observed = per_cpu::current_cpu_id();
    if observed != cpu_id {
        serial_println!(
            "[ kernel ] smp: AP cpu_id={} sentinel MISMATCH (gs:[24] reads {}) - halting",
            cpu_id,
            observed
        );
        halt_forever();
    }

    x86_64::init_traps_ap(cpu_id);
    mm::slab::init_per_cpu(cpu_id);

    serial_println!(
        "[ kernel ] smp: AP cpu_id={} ready (lapic_id={}, processor_id={})",
        cpu_id,
        info.lapic_id,
        info.processor_id
    );

    sched::run_ap_bootstrap(cpu_id);
}
```

AP bootstrap source: `kernel/src/sched/scheduler.rs`

```rust
/// Initialize an AP's per-CPU scheduler and run its bootstrap loop forever.
/// APs do not receive routed work in sub-commit 2; the loop only yields, which
/// no-ops while the queue is empty. BSP remains responsible for reaping.
pub fn run_ap_bootstrap(cpu_id: u32) -> ! {
    let name = ap_bootstrap_name(cpu_id);
    init_cpu_scheduler(cpu_id, name, "AP");
    loop {
        yield_now();
        core::hint::spin_loop();
    }
}

fn ap_bootstrap_name(cpu_id: u32) -> &'static str {
    match cpu_id {
        1 => "ap-1-main",
        2 => "ap-2-main",
        3 => "ap-3-main",
        4 => "ap-4-main",
        5 => "ap-5-main",
        6 => "ap-6-main",
        7 => "ap-7-main",
        _ => "ap-main",
    }
}
```

Post-init behavior:

```text
AP initializes per-CPU state, traps, slab state, then calls sched::run_ap_bootstrap(cpu_id).
run_ap_bootstrap initializes that AP's scheduler with an adopted bootstrap thread named ap-N-main, then loops forever.
```

Yield discipline:

```text
The AP bootstrap loop calls yield_now() every loop iteration, then core::hint::spin_loop().
This is aggressive cooperative yielding, not periodic yielding.
```

Halt / wait-for-event:

```text
No hlt in run_ap_bootstrap.
The AP main loop does not block waiting for an interrupt. It busy-yields and then spin_loop()s.
```

Wake mechanism:

```text
There is no explicit wake mechanism in AP main.
If AP main is executing, it polls its own ready queue by repeatedly calling yield_now().
If AP main is not executing because another thread is current, progress depends on that current thread yielding or being preempted.
```

Lifetime:

```text
AP main never exits. run_ap_bootstrap returns ! and loops forever.
```

Important interrupt detail:

```text
init_traps_ap() loads GDT/TSS, rewires GS, loads shared IDT, and writes syscall MSRs.
It does not call enable_interrupts().
The first spawned kernel thread reached through context_switch executes thread_start_trampoline, whose first instruction is `sti`.
The adopted AP bootstrap thread itself is not created through Thread::new and does not run thread_start_trampoline on initial entry.
```

## §2.4 Reschedule-IPI infrastructure

Source search:

```text
rg -n "send_ipi|IPI|ipi|resched|RESCHED|LOCAL_APIC|lapic|apic" kernel/src
```

Findings:

```text
No send_ipi, send_resched, wake_target_cpu, notify_cpu, APIC ICR writer, IPI vector, or reschedule vector exists in kernel/src.
The only lapic references are Limine MP metadata fields in smp.rs.
```

PIC/PIT source: `kernel/src/arch/x86_64/pic.rs`

```rust
//! Legacy 8259 PIC bring-up.
//!
//! Phase 1.3 uses the PIC (not the APIC) because it's simpler and sufficient
//! for single-CPU. Phase 2+ may switch to APIC for SMP.
```

Timer setup source: `kernel/src/sched/scheduler.rs`

```rust
pub fn init_bsp() {
    init_cpu_scheduler(0, "main", "BSP");
    pit::init(TIMER_HZ);
    // Unmask IRQ0 LAST so the first tick can never fire before the scheduler
    // is installed and PIT is programmed.
    pic::unmask_irq(0);
    serial_println!(
        "[ kernel ] sched: preemption ready (cpu=0, PIT @ {} Hz)",
        TIMER_HZ
    );
}
```

Timer handler source: `kernel/src/arch/x86_64/timer.rs`

```rust
extern "C" fn timer_rust_handler() {
    TICKS.fetch_add(1, Ordering::Relaxed);
    // EOI before sched::on_tick: the PIC is now armed for the next IRQ, but
    // IF stays 0 until iretq pops RFLAGS — so no nested entry. If on_tick
    // yields, we may not iretq for many ticks, and the PIC may collapse the
    // backlog to one pending IRQ. Acceptable: 100 Hz preemption tolerates
    // missed ticks.
    pic::end_of_interrupt(0);
    sched::on_tick();
}
```

Timer interrupt scope:

```text
The only hardware timer source in the code is PIT/PIC IRQ0, initialized from sched::init_bsp().
No per-CPU local APIC timer setup exists.
No APIC EOI exists.
No AP-local timer interrupt setup exists.
```

Reserved IPI vectors:

```text
None in shipped source.
IDT installs exceptions, NMI, and vector 32 for PIC IRQ0 timer.
```

Existing IPI senders and callers:

```text
None found.
```

TODO / future references:

```text
docs/ADR-013-phase-2-smp-retrofit.md §Q7 plans IPI infrastructure for sub-commit 4 TLB shootdowns.
docs/ADR-013-phase-2-smp-retrofit.md says Q4 chose wake-on-completer-CPU specifically to avoid an IPI dependency in sub-commit 3.
kernel/src/arch/x86_64/pic.rs says Phase 2+ may switch to APIC for SMP, but current source still uses PIC.
```

## §2.5 yield_now

Source: `kernel/src/sched/scheduler.rs`

```rust
/// Cooperative yield. Rotates the current thread to the back of the ready
/// queue and switches into the front. No-op if the ready queue is empty.
pub fn yield_now() {
    without_interrupts(|| {
        let switch_params = rotate(ThreadState::Ready, WhichQueue::Ready);
        // MutexGuard from rotate() is dropped at the end of that fn.
        if let Some((prev_rsp_ptr, next_rsp)) = switch_params {
            // SAFETY: see rotate() and module-level comment.
            unsafe { context_switch(prev_rsp_ptr, next_rsp) };
        }
    });
}
```

Relevant `rotate` source:

```rust
fn rotate(prev_state: ThreadState, dest: WhichQueue) -> Option<(*mut u64, u64)> {
    let sched = current_scheduler();
    let mut s = sched.inner.lock();
    let next = s.ready.pop_front()?;
    let mut prev = mem::replace(&mut s.current, next);
    prev.state = prev_state;
    s.current.state = ThreadState::Running;
    let next_rsp = s.current.rsp;

    // Phase 1.4: handle cross-process boundary work BEFORE the asm switch.
    // - TSS.rsp0 + PER_CPU.kernel_rsp must point at next's kernel stack
    //   so any subsequent ring 0 transition (SYSCALL or int from user) lands
    //   on the right stack.
    // - CR3 swap when crossing into a different process (with its own Pmap).
    //   Kernel process (pid=0) has no Pmap — CR3 stays at the kernel PML4 set at
    //   boot — so we skip the Cr3::write when next.pid == KERNEL_PID.
    if prev.pid != s.current.pid {
        cross_process_prep(&s.current);
    } else if s.current.pid != KERNEL_PID {
        // Same user pid (multi-thread within one user process — Phase 1.5+),
        // still need TSS.rsp0 update because each thread has its own kernel
        // stack. For Phase 1.4 single-user-thread, this branch is dead.
        cross_thread_kstack(&s.current);
    }

    let prev_rsp_ptr: *mut u64 = &mut prev.rsp;
    match dest {
        WhichQueue::Ready => {
            stamp_thread_cpu(&prev, sched.cpu_id);
            debug_assert!(
                s.ready.capacity() > s.ready.len(),
                "scheduler-held ready push would allocate (cap={}, len={})",
                s.ready.capacity(),
                s.ready.len()
            );
            s.ready.push_back(prev)
        }
        WhichQueue::Zombie => {
            debug_assert!(
                s.zombies.capacity() > s.zombies.len(),
                "scheduler-held zombie push would allocate (cap={}, len={})",
                s.zombies.capacity(),
                s.zombies.len()
            );
            s.zombies.push_back(prev);
            sched.zombie_count.fetch_add(1, Ordering::Release);
        }
    }
    Some((prev_rsp_ptr, next_rsp))
}
```

Does it cause a context switch?

```text
Yes, if the current CPU's ready queue is non-empty.
It pops the front ready thread, replaces current with it, pushes the previous current to the back of ready, drops the scheduler lock, and calls context_switch(prev_rsp_ptr, next_rsp).
```

What if the ready queue is empty?

```text
rotate returns None via `let next = s.ready.pop_front()?;`.
yield_now then returns without switching.
It does not hlt, sleep, or pause.
```

Assembly footprint:

```rust
context_switch:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rdi], rsp
    mov rsp, rsi
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
```

`yield_now` itself does not call `_mm_pause` or `hlt`. The only explicit `_mm_pause`-class call in the AP bootstrap loop is the `core::hint::spin_loop()` after `yield_now()`. Other stress/test loops also call `yield_now()` and some code paths use `spin_loop()`.

Implication for the NMI snapshot:

```text
If AP main is running and its ready queue is non-empty, yield_now should rotate to the next ready thread.
The NMI snapshot showing AP main as current with ready stress threads behind it is compatible with catching AP main between rotations or inside its loop, but if AP main remains current indefinitely then the issue is not because yield_now lacks a switch path.
CPU0's RIP in _mm_pause is not explained by yield_now's asm; it is from a spin_loop/pause site in the currently-running stress-sender or lock/spin code.
```

## Hp confirmation status

Hp statement under audit:

```text
Missing reschedule-IPI on cross-CPU enqueue + AP main / target CPU progress mechanism is insufficient.
```

Per-question status:

```text
place_thread_on_cpu sends reschedule IPI after enqueue: REFUTED. It does not send any IPI or wake signal.

AP-N-main yields aggressively: CONFIRMED. It loops forever calling yield_now() then spin_loop().

AP-N-main halts waiting for IPI/timer: REFUTED. No hlt in AP bootstrap loop.

Reschedule-IPI infrastructure exists: REFUTED. No IPI send function, no resched vector, no APIC ICR writer, no callers.

Per-CPU local timer drives AP scheduling: REFUTED by source. Only PIT/PIC IRQ0 is initialized from BSP scheduler init; no LAPIC timer setup exists.

yield_now actually context-switches if ready queue non-empty: CONFIRMED.
```

Overall Hp status:

```text
PARTIAL-to-CONFIRMED, with refinement.

Confirmed: cross-CPU enqueue has no out-of-band notification mechanism. No reschedule IPI exists in shipped source. No per-CPU timer infrastructure exists. place_thread_on_cpu can enqueue work onto another CPU without notifying that CPU.

Refined: AP main is not non-yielding. It is an aggressive cooperative yield loop. Therefore the source does not support the narrow claim "AP main never yields." It supports the broader Hp concern that cross-CPU enqueue lacks a real wake/preemption mechanism beyond hoping the target CPU is already executing its AP-main polling loop or a yielding thread.
```

Outcome classification per dispatch:

```text
Outcome C2-ish rather than pure Outcome A.

I PI/reschedule infrastructure is absent and place_thread_on_cpu does not notify the target CPU.
AP main does yield aggressively, so one Hp sub-claim is refuted.
No AP-local timer exists, so a non-yielding current thread on an AP has no visible source-level preemption mechanism.
```

## Recommendation

Per the dispatch, PM consumes this report and decides the next step. Source-read recommendation only:

```text
If PM treats "no IPI + no per-CPU timer + cross-CPU enqueue" as sufficient, proceed to kernel-fix design dispatch.

If PM requires distinguishing AP-main polling latency from true lost wakeup, proceed to second-pass instrumentation with PROGRESS frames, cpu_id tracking, and channel endpoint state dump.

If PM wants a minimal extra source audit before design, inspect whether APs ever receive maskable timer interrupts in practice. The source only shows PIT/PIC IRQ0, not LAPIC timer or IPI infrastructure.
```

No code fix is proposed here.

## Cross-references

```text
Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md
Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md
Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md §2.3, §2.4, §4.1
Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md
docs/ADR-013-phase-2-smp-retrofit.md §Q4, §Q7
kernel/src/sched/scheduler.rs
kernel/src/arch/x86_64/smp.rs
kernel/src/arch/x86_64/pic.rs
kernel/src/arch/x86_64/timer.rs
kernel/src/arch/x86_64/context.rs
kernel/src/ipc/mod.rs
```

