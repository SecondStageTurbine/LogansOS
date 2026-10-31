# SC-3B PLAN checkpoint - Cerebro

Date: 2026-05-09
Stage: PLAN only
Substrate observed: HEAD `79b35f4`
Artifact scope: analysis only, no code changes, no commits, no backup

Prometheus independence note: the Prometheus checkpoint was absent at the start of this pass and was not read. This checkpoint is based on local source and the dispatch context only.

## Substrate check

Observed HEAD:

```text
79b35f4 docs: Plans/INDEX.md - wedge-fix discharge + beta outcome artifacts
1dbbfa7 docs: ADR-016 - Amendment 2026-05-08, A2 reframed to Phase 2 conformance work
2fe5a70 docs: Plans/INDEX.md - sweep through 2026-05-08 governance commits (8a2311d)
aac0c11 mm: ADR-016 IRQ-safe global heap allocator (A1 defense-in-depth)
8a2311d docs: ADR-016 IRQ-safe heap allocator and scheduler heap-decoupling (ratified)
8ef8cac docs: bible.md - Article 8.5 critical-section discipline (v1.0.5)
```

Relevant existing stashes:

```text
stash@{0}: On main: wedge-investigation-8-nmi-watchdog
stash@{1}: On main: wedge-investigation-2-instrumentation
stash@{2}: On main: h1-experiment-instrumentation
```

Files read for this checkpoint:

```text
Plans/DISPATCH-2026-05-09-sc-3b-plan-stage-parallel.md
Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md
Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md
Plans/PM-NOTE-2026-05-08-beta-outcome.md
Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md
docs/ADR-013-phase-2-smp-retrofit.md
kernel/src/sched/scheduler.rs
kernel/src/ipc/mod.rs
kernel/src/arch/x86_64/context.rs
kernel/src/arch/x86_64/syscall.rs
kernel/src/arch/x86_64/per_cpu.rs
kernel/src/sched/process.rs
kernel/src/test_harness.rs
tools/xtask/src/main.rs
```

## Summary recommendation

SC-3B should implement Q4 and Q6 together. The current source already has the Q4 policy shape in the happy path: rendezvous completion enqueues the blocked peer on the completer CPU through `wake_thread_front()`, then either yields for receiver-first completion or continues with a bounded defer for consumer-first completion. The unsafe part is Q6: the IPC block paths currently drop `Channel.state` before entering `sched::block_current_into()`, then re-lock inside the installer closure. Under SMP, that creates a lost-wakeup and torn-state window between "decided to block" and "published blocked state".

The BUILD should therefore focus on one load-bearing primitive: a scheduler context-switch helper that parks the current thread, publishes the blocked endpoint state, switches to the next thread, and releases the channel lock plus scheduler lock only after the CPU is on the next stack.

The biggest design correction from the dispatch options: do not release raw `spin::Mutex` internals from assembly. The `spin` crate's `AtomicBool` field is private, so relying on its layout would be a private-layout ABI dependency. Introduce a small kernel-owned switch-release mutex for the two lock classes that participate in Q6, and expose an explicit release token from that guard.

## Q1 - Q6 ABI shape

Recommendation: choose the per-call-site specialization for SC-3B, implemented as a two-lock helper:

```rust
unsafe extern "C" {
    fn context_switch_with_release2(
        prev_rsp_ptr: *mut u64,
        next_rsp: u64,
        release_first: *const AtomicBool,
        release_second: *const AtomicBool,
    );
}
```

The release order is fixed at the call site:

```text
release_first  = Channel.state lock
release_second = current CPU scheduler lock
```

This is the safest production shape for this BUILD because every current `block_current_into()` call site has exactly those two locks once corrected. It also avoids the old-stack release-list hazard. A borrowed slice, pointer to a `ReleaseSet`, or by-value struct that spills to the caller stack is unsafe in this specific context: after `rsp` is switched, the helper must not read release metadata from the blocked thread's old stack while releasing the first lock may make that old thread visible to other CPUs.

Rejected options:

```text
const-generic borrowed array:
Rejected for SC-3B. It creates pointer-based release metadata, likely on the old stack, and gives no useful monomorphization benefit to a single assembly symbol.

fixed ReleaseSet passed by pointer:
Rejected as written. The pointer target can live on the old stack, and a C ABI by-value struct can still be lowered in ways that are harder to audit than explicit registers.

private spin::Mutex layout release:
Rejected. spin::Mutex's lock byte is private crate internals. Q6 would turn an implementation detail of a dependency into a kernel ABI.
```

Acceptable future-compatible variant if PM wants four release slots now:

```rust
fn context_switch_with_release4(prev_rsp_ptr, next_rsp, l1, l2, l3, l4)
```

with all lock pointers passed explicitly in registers and null meaning unused. I do not recommend paying that surface area in SC-3B without a real three-lock caller.

Required supporting type:

```text
SwitchMutex<T> or KernelSpinMutex<T>
```

This should be a kernel-owned spin mutex with:

```text
AtomicBool lock field controlled by our code
normal RAII guard drop using Release store
guard.release_token() returning *const AtomicBool
no poisoning
no allocation
Acquire lock, Release unlock semantics matching spin::Mutex
```

Use this type only for the Q6 switch-release locks initially:

```text
Channel.state
PerCpuScheduler.inner
```

All other existing `spin::Mutex` users can remain unchanged until they need switch-release semantics.

## Q2 - Memory ordering

Recommendation: no explicit fence instruction in the x86_64 assembly helper.

Required semantics:

```text
Acquire on lock acquisition
Release on normal guard drop
Release-equivalent store in context_switch_with_release2
Channel lock released before scheduler lock
```

On x86_64, an aligned byte store of `0` to the kernel-owned `AtomicBool` lock is sufficient for the Release side of this spinlock protocol. TSO preserves store-store ordering, and the helper does not need `mfence`, `lfence`, or `sfence`.

The assembly sequence should be structurally equivalent to the existing `context_switch()` plus two lock stores after the stack switch:

```asm
push rbx
push rbp
push r12
push r13
push r14
push r15
mov [rdi], rsp
mov rsp, rsi
mov byte ptr [rdx], 0    ; release Channel.state
mov byte ptr [rcx], 0    ; release scheduler.inner
pop r15
pop r14
pop r13
pop r12
pop rbp
pop rbx
ret
```

The helper should not save or restore RFLAGS unless separately ratified. `kernel/src/arch/x86_64/context.rs` currently does not save RFLAGS, and the dispatch path runs under `without_interrupts()`. Adding `pushfq` or `popfq` here would silently change the scheduler ABI and could resurrect stale interrupt state from an old context.

Safety comment requirement:

```text
The two release pointers must refer to kernel-owned SwitchMutex lock bytes, not spin::Mutex private layout.
The pointed locks must still be held by the current CPU at entry.
The helper releases l1 before l2.
The helper reads no release metadata from the old stack after mov rsp, rsi.
The helper performs no calls and no heap access.
```

## Q3 - Panic-safety of mem::forget plus assembly window

Recommendation: use normal RAII guards until all fallible work is complete, then enter a tiny no-panic window that only forgets guards and calls assembly.

Correct shape:

```text
1. IPC holds Channel.state guard.
2. block_current_into_with_guard() acquires current scheduler guard.
3. It computes next thread, state transitions, next_rsp, prev_rsp_ptr.
4. It runs cross_process_prep() or cross_thread_kstack() while guards are still RAII-managed.
5. It publishes the blocked thread into Channel.state through the caller-provided installer.
6. It extracts release tokens from both guards.
7. It forgets both guards.
8. It immediately calls context_switch_with_release2().
```

No operations after the first `mem::forget()` except the second `mem::forget()` and the assembly call. Specifically no logging, no `debug_assert!`, no array indexing, no unwrap, no allocation, no function call other than the assembly entry point.

This makes panic behavior simple:

```text
panic before forget:
RAII drops both locks normally.

panic after forget:
not allowed by code shape; there must be no panic-capable operation in that window.
```

Do not use `#[track_caller]` or panic-abort policy as the defense. The defense is code structure and a narrow unsafe block.

The scheduler helper should be generic over the outer guard type so IPC does not need scheduler-specific knowledge of `EndpointState`:

```rust
pub fn block_current_into_with_guard<T, F>(
    outer_guard: SwitchMutexGuard<'_, T>,
    installer: F,
) -> i64
where
    F: FnOnce(&mut T, Box<Thread>);
```

Exact names can change, but ownership must not: the scheduler helper owns the already-held channel guard and is responsible for the assembly release path.

## Q4 - Cross-process bookkeeping ordering

Recommendation: preserve the current scheduler ordering exactly, only changing where the locks are released.

Current load-bearing ordering in `rotate()` and `block_current_into()`:

```text
select next thread
replace current thread
set thread states
run cross_process_prep(next) or cross_thread_kstack(next)
capture prev_rsp_ptr and next_rsp
context switch
```

SC-3B must keep that ordering:

```text
Channel.state guard held
current scheduler guard held
prev thread removed from current slot
next thread installed as current
next kernel stack/TSS/per_cpu rsp prepared
CR3 switched if pid differs
prev thread published into Channel.state blocked endpoint
Channel lock released on next stack
scheduler lock released on next stack
new context resumes
```

Rationale:

```text
cross_thread_kstack(next) must complete before the next user thread can make a syscall, because syscall_entry reads gs:[0] for the kernel stack.

cross_process_prep(next) must complete before the context switch because the CPU will return on the next thread's address space assumptions.

The blocked thread must be visible in Channel.state before the channel lock is released, otherwise the peer can observe no waiter.

The scheduler current slot must not be unlocked until after the CPU has moved to the next stack, otherwise another CPU can see scheduler state that does not match the live stack.
```

Observed source details:

```text
cross_thread_kstack(next):
sets TSS rsp0 through gdt::set_kernel_rsp()
sets per_cpu kernel_rsp through per_cpu::set_kernel_rsp()
no heap allocation observed

cross_process_prep(next):
calls cr3_for_thread(next.pid)
writes CR3
no heap allocation observed in the path read for this checkpoint
```

The current `cr3_for_thread()` locks `PROCESSES` to read the process pmap. That is not ideal long-term, but it is existing behavior and does not add heap activity to the Q6 release window. If SC-3B wants to shrink this later, cache the active CR3 on `Thread`, but do not combine that with the Q6 primitive unless a concrete failure forces it.

## Q5 - Block-target enumeration

Observed production call sites for `sched::block_current_into()`:

```text
kernel/src/ipc/mod.rs: Channel::send
parks current side as SendBlocked

kernel/src/ipc/mod.rs: Channel::recv
parks current side as RecvBlocked

kernel/src/ipc/mod.rs: Channel::grant
parks current side as GrantBlocked

kernel/src/ipc/mod.rs: Channel::recv_grant
parks current side as RecvGrantBlocked
```

All four are logically the same lockset after correction:

```text
Lock 1: Channel.state
Lock 2: current CPU PerCpuScheduler.inner
```

No scheduler-only `block_current_into()` callers were found. No three-lock callers were found. This supports the two-lock ABI recommendation.

Required IPC retrofit:

```text
Do not drop Channel.state before blocking.
Do not re-lock Channel.state inside the installer closure.
Pass the held Channel.state guard into the scheduler helper.
Publish the blocked endpoint under that same guard.
Let context_switch_with_release2 release Channel.state and scheduler.inner on the next stack.
```

Q4 wake-on-completer policy:

```text
send completes waiting receiver:
receiver is enqueued on sender CPU and sender yields.

recv completes waiting sender:
sender is enqueued on receiver CPU and receiver continues with defer quantum.

grant/recv_grant:
recipient is the consumer; use the same consumer-first policy as recv.
```

Implementation risk found: ADR-013 Q4 talks about `Thread.cpu_id`, but `kernel/src/sched/thread.rs` at this substrate does not appear to expose a `cpu_id` field. SC-3B should add or otherwise account for this explicitly. At minimum, `spawn`, `push_ready`, `wake_thread`, and `wake_thread_front` need to record the CPU that owns the ready/current placement if future cross-CPU kill, reap diagnostics, and test assertions depend on it.

## Q6 - Test scenario set

Recommended new scenarios:

```text
smp_block_release_smoke
Validates the Q6 helper in the simplest path. One thread blocks while holding the channel lock through the scheduler switch; the peer completes it. PASS frame should include blocked_cpu, completer_cpu, resumed_cpu, and release_path=asm.

smp_send_blocks_first_then_completes
Sender blocks on CPU A, receiver completes on CPU B. Sender resumes on CPU B, receives success return, and no lost wake occurs. Tests sender-first rendezvous and handoff_to_rendezvous_peer().

smp_recv_blocks_first_then_completes
Receiver blocks on CPU A, sender completes on CPU B. Receiver resumes on CPU B with the expected payload. Tests consumer-first completion and continue_current_after_receive().

smp_grant_cross_cpu
Grantor blocks on CPU A, recipient completes recv_grant on CPU B. The received handle has the expected rights subset and a fresh derived CapId. This exercises Q6 plus cap grant transfer without making revoke semantics the focus.

smp_cross_cpu_migration_reap
A blocked thread migrates through rendezvous completion, then exits on the completer CPU. BSP reap must observe and clean it under the current Q9 policy. This catches stale CPU ownership and cross-CPU Thread slab/free issues.

smp_cross_cpu_rendezvous_stress
Primary stress scenario. Multiple send-first and recv-first pairs across NEO_SMP=4, fixed iteration count, deterministic payload checks, timeout classified as failure. This is the SC-3B equivalent of the wedge-fix 200-run gate.
```

Test harness prerequisite:

```text
The current test harness has smp_per_cpu_scheduler_smoke and smp_same_cpu_ipc, but no obvious production-grade way to place a user or kernel test actor on a specific AP for real cross-CPU IPC. SC-3B needs a minimal test placement hook, preferably a scheduler helper that enqueues a prepared test thread onto a target CPU under that CPU's scheduler lock. This should be production-safe or test-feature-gated, not an ad hoc global.
```

Scenarios to defer:

```text
smp_kill_during_rendezvous
Defer to Q8.c. Current blocked-thread kill semantics are explicitly not the SC-3B target.

smp_multi_grant_chain_cross_cpu
Useful later, but it tests cap derivation and cap-table SMP composition more than Q4/Q6. Keep SC-3B's grant test narrow unless a cap regression appears.
```

Required anti-regression set:

```text
ipc_heap_kill_smoke
smp_per_cpu_scheduler_smoke
smp_same_cpu_ipc
cap_grant_basic
cap_revoke_local
cap_revoke_cross_process
cap_survive_exit
buddy_split_coalesce
slab_thread_migration
all default non-ignored xtask scenarios
```

Existing Phase 1.5b/1.5c and BUILD-2 tests must pass unmodified. Any green-to-red transition there is a BUILD bug, not a test update.

## Q7 - Verification methodology

Recommended gate order:

```text
1. cargo k
2. single cargo neo-tty boot at NEO_SMP=4
3. single run of each new scenario at NEO_SMP=4
4. full default xtask suite at NEO_SMP=4
5. 50-run loop for each supplementary SC-3B scenario
6. 200-run loop for smp_cross_cpu_rendezvous_stress at NEO_SMP=4
7. 25-run smoke for the primary stress scenario at NEO_SMP=2
```

Optional if host runtime permits:

```text
25-run smoke for smp_cross_cpu_rendezvous_stress at NEO_SMP=8
```

Pass criteria for every scenario:

```text
BEGIN frame observed
exactly one terminal PASS or FAIL frame
terminal frame is PASS
debug-exit code matches PASS
no duplicate terminal frames
no BUG:
no KERNEL PANIC
no timeout
frame leak audit clean when the scenario reaches process teardown
```

Required serial fields in new PASS frames:

```text
blocked_cpu=<id>
completer_cpu=<id>
resumed_cpu=<id>
iterations=<n> for stress
send_first=<n> and recv_first=<n> for stress
grant_rights=<mask> for grant
exit_reaped=ok for migration/reap
```

Statistical target:

```text
0/200 on the primary stress scenario is the minimum closure signal. It gives a comparable standard to the wedge-fix discharge and makes a recurrent lost-wakeup class unlikely enough to proceed. Any timeout or FAIL in the 200-run loop should stop the loop, preserve logs, and classify before retrying.
```

Failure classes to classify explicitly:

```text
lost wakeup
deadlock with one waiter unpublished
deadlock with waiter published but not woken
wrong CPU migration
duplicate terminal frame
missing terminal frame
panic or BUG
frame leak
rights mismatch in grant
unexpected EBADF or EPIPE
```

## Q8 - Composition with A1 and Bible 8.5.3 advancement

A1 composition:

```text
ADR-016 A1 makes the global heap allocator IRQ-safe as defense in depth. SC-3B should not rely on that to justify allocations in scheduler-critical windows.
```

Observed Q6 critical path should remain non-allocating:

```text
cross_thread_kstack:
TSS/per_cpu writes only.

cross_process_prep:
PROCESSES lock plus CR3 read/write; no heap allocation observed.

IPC block installer:
moves the existing Box<Thread> and already-owned payload fields into EndpointState. It should not allocate.

context_switch_with_release2:
assembly only; no calls; no heap.
```

Areas that still need care:

```text
Immediate grant completion can call process::complete_grant() while Channel.state is held. That path may mutate handle tables and derivation maps. It is not inside the scheduler switch-release window, but it is still relevant to Article 8.5 discipline and should not be expanded silently during SC-3B.

Ready-queue pushes still use VecDeque. Existing scheduler invariants and debug asserts are the current defense. Full heap-decoupled scheduler data structures remain A2 and are not solved by SC-3B.
```

Bible Article 8.5.3 advancement:

```text
SC-3B materially advances Article 8.5.3 because blocking IPC no longer drops and re-acquires locks around the publication point, and the assembly release path avoids heap work, logging, or arbitrary Rust calls while scheduler state is inconsistent.

SC-3B does not complete Article 8.5.3 by itself. A2 remains the phase-level conformance work for removing scheduler heap dependence and formalizing the broader critical-section rules.
```

## Risks and required PM decisions

Recommended PM decisions before code:

```text
Decision 1:
Approve `context_switch_with_release2` as the SC-3B ABI, with explicit register lock pointers and Channel-before-scheduler release order.

Decision 2:
Approve a kernel-owned switch-release mutex for Channel.state and PerCpuScheduler.inner instead of releasing private spin::Mutex internals.

Decision 3:
Ratify no RFLAGS save/restore in the Q6 helper, preserving the current context_switch ABI.

Decision 4:
Confirm whether `Thread.cpu_id` is required in SC-3B implementation. ADR-013 Q4 assumes it, but the current source does not yet expose it.

Decision 5:
Approve a minimal cross-CPU test placement hook so the new scenarios can exercise real cross-CPU IPC rather than same-CPU IPC with SMP enabled.
```

Primary risk:

```text
The highest-risk mistake would be implementing a release-list pointer or by-value struct that lives on the old stack, then reading it after `mov rsp, rsi`. The two-lock explicit-register ABI avoids that class completely.
```

Second risk:

```text
Treating `spin::Mutex` layout as stable would create a dependency on private crate internals. That is not acceptable for a kernel ABI. Use a kernel-owned lock byte.
```

Third risk:

```text
Adding cross-CPU tests without a controlled placement primitive can produce tests that pass because everything accidentally ran on the BSP. PASS frames must print observed CPU ids and assert that blocking and completion occurred on different CPUs.
```

## Proposed BUILD slicing

Recommended SC-3B implementation order:

```text
Sub-step 1:
Add kernel-owned SwitchMutex and migrate only Channel.state and PerCpuScheduler.inner. No behavior change. cargo k plus existing suite.

Sub-step 2:
Add context_switch_with_release2 and block_current_into_with_guard(), but keep IPC behavior equivalent. Add smp_block_release_smoke.

Sub-step 3:
Retrofit send/recv block paths to hold Channel.state through block_current_into_with_guard(). Add send-first and recv-first cross-CPU scenarios.

Sub-step 4:
Retrofit grant/recv_grant block paths. Add smp_grant_cross_cpu.

Sub-step 5:
Add migration/reap and stress scenarios. Run closure loops.

Sub-step 6:
Document ADR-013 amendment for Q6 ABI, switch-release mutex, no-RFLAGS decision, and exact verification evidence.
```

Stop conditions:

```text
Any failure in existing default scenarios after Sub-step 1.
Any timeout in a new cross-CPU scenario before the first PASS.
Any PASS frame where blocked_cpu == completer_cpu in a cross-CPU test.
Any need to add a third lock to block_current_into_with_guard().
Any heap allocation discovered between scheduler lock acquisition and context_switch_with_release2().
```

## Close

My recommendation is not to build a generalized release-list abstraction in SC-3B. The real production requirement at this boundary is exactly two locks, exactly ordered, released from registers after the stack switch. Implement that narrowly, with kernel-owned lock tokens, then let future Q8/A2 work generalize only when a concrete caller proves it necessary.
