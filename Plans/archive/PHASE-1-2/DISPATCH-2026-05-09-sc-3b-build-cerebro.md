# Dispatch: SC-3B BUILD — Q4 wake-on-completer-CPU + Q6 lock-order asm release + IPC retrofit

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-09
**Type:** BUILD-stage implementation dispatch. Per Bible Article 14, this opens the BUILD/VERIFY stage for SC-3B. The PLAN-stage synthesis (`Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md`) was ratified by project lead in conversation 2026-05-09. This dispatch is the consolidated execution playbook covering the 6-sub-step plan in §4 of the synthesis. Multi-commit within one BUILD-3 sub-commit boundary.
**Substrate:** Project HEAD `79b35f4`. Three preserved stashes (unchanged):

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this dispatch.

---

## 0. Standing instructions

1. **Multi-commit within one sub-commit boundary.** Each of the 6 sub-steps below should be at least one commit. Sub-step 1 may split into 2 commits if cleaner (mutex work + Thread.cpu_id+test-hook). Aim for ~6-10 commits total over the BUILD; do not exceed 12.
2. **Build verification before EVERY commit.** `docker compose run --rm neo cargo k` must pass with the existing 18-warning baseline (no new warnings). This is non-negotiable per Bible 1.3.
3. **Anti-regression scenarios after Sub-step 1, Sub-step 3, and Sub-step 6.** Run the full default `cargo neo-test` suite at NEO_SMP=4 to verify no existing scenarios broke. Document any required test updates as deliberate decisions, not silent drift.
4. **No stash interactions.** Stashes remain untouched.
5. **Backup runs after EACH commit lands.** `Plans/backup-to-synology.ps1`. With three stashes, the 2026-05-08-refined backup mechanism handles preservation.
6. **Stop conditions are honored.** See §10. If a stop condition fires, halt the BUILD, capture diagnostics, ping PM. Do not push through.
7. **Source-grounded:** This dispatch references the synthesis and Cerebro's own PLAN. Re-reference both during BUILD when ambiguity arises. PM is on call for any clarification.
8. **Ratified positions are load-bearing.** The BUILD direction in PL's ratification message is normative:
    - Q6 ABI: `context_switch_with_release2(prev_rsp_ptr, next_rsp, channel_lock, sched_lock)`, explicit register lock pointers, no release-list metadata after stack switch.
    - Lock primitive: kernel-owned `SwitchMutex`, not `spin::Mutex` internals.
    - Release order: Channel first, scheduler second.
    - BUILD scope includes Thread.cpu_id and controlled cross-CPU test placement hook.
    - IPC retrofit covers exactly four block paths: send, recv, grant, recv_grant.

---

## 1. Goal

Implement Q4 (wake-on-completer-CPU) and Q6 (lock-order asm release) per ADR-013. Make the IPC handoff path SMP-safe under per-CPU schedulers (which sub-commit 2 already landed). Validate via cross-CPU IPC scenarios at NEO_SMP=4 with a 200/200 closure gate on the primary stress scenario.

After SC-3B closes, BUILD-3 has Q7 (TLB management) and Q8 (capability table SMP composition) remaining. Phase 2 BUILD-4-or-later picks up ADR-016 A2 (scheduler heap-decoupling) as Bible 8.5.3 conformance work.

---

## 2. SUB-STEP 1 — Infrastructure prerequisites

This sub-step establishes the substrate the rest of the BUILD requires. No behavior change; types and fields land first so subsequent commits can use them.

### 2.1 Add `SwitchMutex<T>`

Add a kernel-owned spinlock type. Location suggestion: `kernel/src/sync/switch_mutex.rs` (new file) or as a new module in `kernel/src/sched/`. PM defers location to Cerebro's judgment.

Required surface:

```rust
pub struct SwitchMutex<T> {
    lock: AtomicBool,
    inner: UnsafeCell<T>,
}

pub struct SwitchMutexGuard<'a, T> {
    mutex: &'a SwitchMutex<T>,
    _not_send: PhantomData<*const ()>,
}

impl<T> SwitchMutex<T> {
    pub const fn new(value: T) -> Self;
    pub fn lock(&self) -> SwitchMutexGuard<'_, T>;
    pub fn try_lock(&self) -> Option<SwitchMutexGuard<'_, T>>;
}

impl<'a, T> SwitchMutexGuard<'a, T> {
    pub fn release_token(&self) -> *const AtomicBool;
}

impl<'a, T> Deref for SwitchMutexGuard<'a, T> { /* &T */ }
impl<'a, T> DerefMut for SwitchMutexGuard<'a, T> { /* &mut T */ }
impl<'a, T> Drop for SwitchMutexGuard<'a, T> { /* AtomicBool::store(false, Release) */ }
```

Required semantics:

- Acquisition: `AtomicBool::compare_exchange(false, true, Acquire, Relaxed)` spin loop. Match `spin::Mutex` semantics.
- Release on normal Drop: `AtomicBool::store(false, Release)`.
- `release_token()`: returns a stable `*const AtomicBool` pointing into the lock byte. Used by Q6 asm helper.
- No poisoning, no `Result`, no allocation, no IRQ awareness (callers wrap in `without_interrupts` themselves where needed — same convention as `spin::Mutex`).

`unsafe` blocks carry SAFETY comments per Bible 15.2. The unsafe surface is small: `UnsafeCell::get()` for inner access, `release_token` returning a raw pointer.

The `_not_send: PhantomData<*const ()>` makes `SwitchMutexGuard` `!Send` — same discipline as `spin::MutexGuard`. This prevents accidentally moving the guard across threads.

### 2.2 Migrate `Channel.state` and `PerCpuScheduler.inner` to `SwitchMutex`

Change the type declarations:

- `kernel/src/ipc/mod.rs` — `Channel.state: spin::Mutex<EndpointState>` → `SwitchMutex<EndpointState>`.
- `kernel/src/sched/scheduler.rs` — `PerCpuScheduler.inner: spin::Mutex<PerCpuSchedulerInner>` → `SwitchMutex<PerCpuSchedulerInner>`.

Update all call sites that lock these mutexes. The API surface (`.lock()`, `.try_lock()`, deref) is identical, so most call sites need zero changes.

**No behavior change.** This commit (or sub-commit-1 step) is a pure type substitution. Existing scenarios all pass.

**Anti-scope:** do NOT migrate any other `spin::Mutex` user. Only Channel.state and PerCpuScheduler.inner participate in Q6's switch-release pattern; everything else (process registry, slab caches, etc.) keeps `spin::Mutex`.

### 2.3 Add `Thread.cpu_id`

`kernel/src/sched/thread.rs` — add a field:

```rust
pub struct Thread {
    // existing fields
    pub cpu_id: AtomicU32,  // current CPU placement; 0..MAX_CPUS-1
}
```

Set `cpu_id` to the current CPU at:

- `Thread::new` / spawn — initial cpu_id = spawn-time current CPU (or BSP if pre-AP-bringup).
- `push_ready` / `wake_thread` / `wake_thread_front` — update cpu_id to the CPU whose scheduler the thread is being placed on.
- Q4 wake-on-completer-CPU (Sub-step 3) — update cpu_id to the completer CPU before pushing to that CPU's ready queue.

Reads of `cpu_id` are `AtomicU32::load(Relaxed)` — informational, not load-bearing for correctness. Used by Q4 wake decisions, future Q8.c blocked-thread-kill, and test PASS-frame fields.

### 2.4 Add cross-CPU test placement hook

The current test harness has no way to place a test thread on a specific AP. SC-3B's cross-CPU scenarios require it.

Add a test-feature-gated helper. Location suggestion: `kernel/src/test_harness.rs` or `kernel/src/sched/scheduler.rs` behind `#[cfg(feature = "test_smp_placement")]`. PM defers location to Cerebro's judgment.

Required surface:

```rust
#[cfg(any(test, feature = "test_smp_placement"))]
pub fn place_thread_on_cpu(thread: Box<Thread>, target_cpu: u32) -> Result<(), Error>;
```

Semantics:
- Acquires the target CPU's `PerCpuScheduler.inner` lock under `without_interrupts`.
- Sets `thread.cpu_id = target_cpu`.
- Pushes thread to target's ready queue.
- Returns error if target_cpu >= active CPU count or target's scheduler is not initialized.

If gating behind a Cargo feature is awkward, gating behind `cfg(test)` or via a `pub(crate)` function only callable from `kernel/src/test_harness.rs` is acceptable. The constraint is: production code must not use this helper.

### 2.5 Pre-commit checklist for Sub-step 1

Before each commit in this sub-step:

- [ ] `cargo k` passes with 18-warning baseline.
- [ ] All existing default `cargo neo-test` scenarios pass at NEO_SMP=4.
- [ ] No stash interactions.
- [ ] `git diff --stat` shows changes confined to expected files.
- [ ] All new `unsafe` blocks have SAFETY comments per Bible 15.2.
- [ ] Backup runs after commit lands.

### 2.6 Commit message templates for Sub-step 1

If Cerebro splits into 1a + 1b:

**1a:**
```
sched: SwitchMutex type for Q6 switch-release primitive (SC-3B prep)

[body explaining the new type, its rationale (Bible 4.3 + spin::Mutex
private layout), API surface, and that no behavior change lands in this
commit. Reference Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md §1.2.]
```

**1b:**
```
sched/ipc: migrate Channel.state and PerCpuScheduler.inner to SwitchMutex

[body explaining the type substitution, no behavior change, that
spin::Mutex remains for all other callers. Reference SwitchMutex commit
SHA and the synthesis.]
```

If combined:
```
sched/ipc: introduce SwitchMutex and migrate two switch-release call sites

[combined body]
```

Plus a separate commit for Thread.cpu_id and test placement hook (or fold into 1b if scope stays small):

```
sched/test: add Thread.cpu_id field and cross-CPU test placement hook

[body explaining the field, atomicity choice, set-points (spawn, push_ready,
wake, wake_front), and the test placement helper's gating. References ADR-013
§Q4 dependency.]
```

---

## 3. SUB-STEP 2 — Q6 asm helper and Rust wrapper

This sub-step adds the asm primitive and Rust wrapper. After this commit, the helper exists and is unit-tested via a smoke scenario, but no production callers use it yet. Sub-steps 3-4 wire it into IPC.

### 3.1 Add `context_switch_with_release2` in `kernel/src/arch/x86_64/context.rs`

C-ABI signature:

```rust
unsafe extern "C" {
    pub fn context_switch_with_release2(
        prev_rsp_ptr: *mut u64,    // RDI
        next_rsp: u64,             // RSI
        channel_lock: *const AtomicBool,  // RDX (Channel.state's SwitchMutex lock byte) - released FIRST
        sched_lock: *const AtomicBool,    // RCX (PerCpuScheduler.inner's SwitchMutex lock byte) - released SECOND
    );
}
```

**Argument order in registers is load-bearing.** RDI=prev_rsp_ptr, RSI=next_rsp, RDX=channel_lock, RCX=sched_lock — System V AMD64 ABI's first four integer arguments. The asm release order is Channel-first per pre-thinking I1.

Asm body (Intel syntax, conceptual):

```asm
.global context_switch_with_release2
context_switch_with_release2:
    # Save callee-saved registers on current (old) stack
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    # Save current RSP to *prev_rsp_ptr
    mov [rdi], rsp

    # Switch to new stack
    mov rsp, rsi

    # Release locks IN ORDER: Channel first, scheduler second
    # All metadata is in registers (RDX, RCX) — no memory reads
    # from old stack post-switch
    mov byte ptr [rdx], 0    # Release Channel.state (SwitchMutex byte = 0)
    mov byte ptr [rcx], 0    # Release PerCpuScheduler.inner

    # Restore callee-saved registers from new stack
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
```

**Critical invariants** (must be in SAFETY comment on the unsafe extern block):

- Both `*const AtomicBool` arguments must point to lock bytes inside `SwitchMutex` instances (NOT `spin::Mutex`).
- Both locks must be currently held by the calling CPU at function entry.
- Caller must have already executed `mem::forget` on both `SwitchMutexGuard` instances before this call.
- The function reads no memory off the old stack after `mov rsp, rsi`. All release metadata is in registers.
- `prev_rsp_ptr` must point to writable memory that survives the call (typically `&mut prev_thread.rsp` where `prev_thread` is parked into Channel state by the caller).
- No RFLAGS save/restore. Caller is responsible for interrupt state (which is `disabled` per `without_interrupts` wrapping).
- No fence. x86 TSO + Release semantics on the AtomicBool stores are sufficient.

### 3.2 Add `block_current_into_with_guard` in `kernel/src/sched/scheduler.rs`

Replace the existing `block_current_into` (or add this as a new function and migrate callers in Sub-step 3-4).

Signature:

```rust
pub fn block_current_into_with_guard<T, F>(
    channel_guard: SwitchMutexGuard<'_, T>,
    installer: F,
) -> i64
where
    F: FnOnce(&mut T, Box<Thread>),
{
    // ...
}
```

Body shape (sketch):

```rust
without_interrupts(|| {
    let sched = current_per_cpu().scheduler();
    let mut s = sched.inner.lock();

    let next = s.ready.pop_front()
        .expect("block_current_into_with_guard: ready queue empty (shouldn't happen post-Q5)");
    let mut prev_box = mem::replace(&mut s.current, next);
    prev_box.state = ThreadState::Blocked;
    s.current.state = ThreadState::Running;
    let next_rsp = s.current.rsp;

    // Cross-process / cross-thread bookkeeping (BEFORE installer; see §3.2.1)
    if prev_box.pid != s.current.pid {
        cross_process_prep(&s.current);
    } else if s.current.pid != KERNEL_PID {
        cross_thread_kstack(&s.current);
    }

    let prev_rsp_ptr: *mut u64 = &mut prev_box.rsp;

    // Installer publishes prev_box into channel state
    // (channel_guard is moved INTO the installer via &mut deref)
    let mut channel_state_ref = unsafe { &mut *channel_guard.mutex.inner.get() };
    installer(channel_state_ref, prev_box);

    // Capture release tokens BEFORE forget
    let channel_lock_ptr = channel_guard.release_token();
    let sched_lock_ptr = s.release_token();

    // Forget guards in same scope as asm call — no panicable ops between
    mem::forget(channel_guard);
    mem::forget(s);

    // Asm call: switch stacks, release Channel first, then scheduler
    unsafe {
        context_switch_with_release2(
            prev_rsp_ptr,
            next_rsp,
            channel_lock_ptr,
            sched_lock_ptr,
        );
    }

    // Resumed here when this thread later unblocks
});

current_per_cpu().scheduler().inner.lock().current.blocked_return
```

The exact code shape may differ; the important invariants are:

- All bookkeeping happens before `mem::forget` calls.
- `mem::forget` calls and the asm call are in a single `unsafe` block with no intervening Rust operations that could panic.
- Channel guard is consumed (moved) into the function and returned via the asm release; caller cannot reuse it.

### 3.2.1 Bookkeeping ordering

Per synthesis §1.6 and Cerebro's PLAN §2.4:

- `cross_process_prep(&s.current)`: writes CR3. Required before stack switch so the new thread runs in its address space.
- `cross_thread_kstack(&s.current)`: writes TSS.rsp0 and per_cpu.kernel_rsp. Required before stack switch so SYSCALL entry on the new thread lands on its kernel stack.
- Both must complete before `mem::forget` and the asm call.

The current ordering in `block_current_into` is correct; preserve it.

### 3.3 Add `smp_block_release_smoke` test scenario

Minimal smoke test that exercises `block_current_into_with_guard` without IPC. Location: `tests/` or wherever test scenarios live (mirror existing `smp_per_cpu_scheduler_smoke` pattern).

Test logic (high-level):

1. Spawn a "blocker" thread on CPU 0.
2. Spawn a "releaser" thread on CPU 1 via `place_thread_on_cpu`.
3. Blocker thread acquires a test channel's `SwitchMutex<EndpointState>`, calls `block_current_into_with_guard` with an installer that parks itself.
4. Releaser thread, on CPU 1, acquires the same `SwitchMutex`, extracts the parked thread, pushes it to CPU 1's ready queue.
5. Blocker thread resumes on CPU 1.
6. Verify `blocked_cpu=0`, `completer_cpu=1`, `resumed_cpu=1`.

PASS frame:
```
NEO_TEST/1 event=PASS name=smp_block_release_smoke blocked_cpu=0 completer_cpu=1 resumed_cpu=1 release_path=asm
```

This scenario validates the Q6 helper alone. It does NOT use IPC — it uses a synthetic test channel. This is the exception to "no scaffolding tests" because the helper has just been introduced and no IPC caller uses it yet.

### 3.4 Pre-commit checklist for Sub-step 2

- [ ] `cargo k` passes; 18 warnings.
- [ ] `smp_block_release_smoke` PASSes at NEO_SMP=4.
- [ ] All existing default scenarios pass at NEO_SMP=4.
- [ ] Unsafe blocks have SAFETY comments.
- [ ] `git diff --stat` confined to expected files.
- [ ] Backup runs after commit lands.

### 3.5 Commit message for Sub-step 2

```
sched/arch: Q6 context_switch_with_release2 + block_current_into_with_guard (SC-3B)

Adds the asm primitive that switches stacks and releases Channel/scheduler
locks in order on the new stack, plus the Rust wrapper that wraps the
locked critical section + forget + asm-call window.

ABI per Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md §1.1: per-call-site
specialization, explicit register lock pointers, no release-list metadata
on old stack post-switch.

Lock order Channel-first, scheduler-second per pre-thinking note I1.

No fence; x86 TSO + Release semantics sufficient. No RFLAGS save/restore;
caller's without_interrupts is the interrupt-state contract.

Helper is unit-tested via smp_block_release_smoke (synthetic test channel,
no IPC). Sub-step 3 wires it into Channel::send and Channel::recv.

Reference: docs/ADR-013 §Q6, Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md.
```

---

## 4. SUB-STEP 3 — Send/recv retrofit + Q4 wake-on-completer-CPU

This sub-step retrofits two of the four IPC block paths, plus implements Q4 wake semantics. Cross-CPU send/recv tests validate.

### 4.1 Retrofit `Channel::send`

Locate `Channel::send` in `kernel/src/ipc/mod.rs`. Replace the current "drop-channel-lock-then-block" pattern with hold-channel-lock-through-block.

Sketch:

```rust
impl Channel {
    pub fn send(&self, payload: ...) -> i64 {
        let mut state_guard = self.state.lock();  // SwitchMutex now
        // ... existing state inspection ...
        match state_to_take {
            EndpointState::Empty => {
                // Park self
                sched::block_current_into_with_guard(
                    state_guard,  // moves the guard into the helper
                    |state, prev_thread| {
                        // Installer: state is &mut EndpointState (channel state),
                        // prev_thread is the now-blocked Box<Thread>
                        *state = EndpointState::SendBlocked {
                            payload: ...,
                            waiter: prev_thread,
                        };
                    },
                );
                // Resumes here when receiver completes the rendezvous
                // and pushes us back to a ready queue
                return current_thread().blocked_return;
            }
            EndpointState::RecvBlocked { ... } => {
                // Q4: wake receiver on COMPLETER's CPU (this CPU) per pre-thinking I1
                // and synthesis §1.3
                let receiver = take_receiver_from_state(&mut state_guard);
                drop(state_guard);
                sched::wake_thread_on_current_cpu(receiver);
                return SUCCESS;
            }
            // ... other state cases unchanged ...
        }
    }
}
```

**Q4 wake-on-completer-CPU semantics:**

When CPU M's `Channel::send` finds a `RecvBlocked` waiter (from CPU N), CPU M:
1. Extracts the parked thread from `EndpointState`.
2. Pushes it to **CPU M's** ready queue (NOT CPU N's), with `thread.cpu_id` updated to M.
3. Yields to it (consumer-first per ADR-011 BUILD-1 amendment, restated in synthesis §1.3).

A new helper `wake_thread_on_current_cpu(thread: Box<Thread>)` enqueues to current CPU's per-CPU scheduler with cpu_id update. Add this to `kernel/src/sched/scheduler.rs`.

### 4.2 Retrofit `Channel::recv`

Symmetric to send. Holds the lock through `block_current_into_with_guard`. When completing a `SendBlocked` waiter, wake on the current (receiver's) CPU and continue running with bounded defer per ADR-011 BUILD-1 amendment (sender finished its send, receiver is the consumer that just got woken — receiver continues).

### 4.3 Add `smp_send_blocks_first_then_completes` test scenario

Test logic:
1. Place sender on CPU 0, receiver on CPU 1 via `place_thread_on_cpu`.
2. Sender sends to a small channel; channel is full so sender blocks. Capture `blocked_cpu=0`.
3. Receiver receives; capture `completer_cpu=1`.
4. Sender resumes; capture `resumed_cpu`. Per Q4, this should be 1.
5. PASS frame: `blocked_cpu=0 completer_cpu=1 resumed_cpu=1`.

### 4.4 Add `smp_recv_blocks_first_then_completes` test scenario

Symmetric. Receiver on CPU 0 blocks on empty channel; sender on CPU 1 sends. Receiver wakes on CPU 1.

### 4.5 Pre-commit checklist for Sub-step 3

- [ ] `cargo k` passes; 18 warnings.
- [ ] Both new `smp_send_blocks_first_then_completes` and `smp_recv_blocks_first_then_completes` PASS at NEO_SMP=4.
- [ ] `smp_block_release_smoke` still passes.
- [ ] All existing IPC scenarios (`ipc_heap_kill_smoke`, `cap_*` scenarios) pass at NEO_SMP=4.
- [ ] All existing default scenarios pass at NEO_SMP=4.
- [ ] PASS frames include the new fields (`blocked_cpu`, `completer_cpu`, `resumed_cpu`).
- [ ] Backup runs after commit lands.

### 4.6 Commit message for Sub-step 3

```
ipc/sched: Q4 wake-on-completer-CPU + Q6 retrofit for send/recv (SC-3B)

Channel::send and Channel::recv now hold the channel SwitchMutex through
block_current_into_with_guard rather than dropping/re-locking around the
block. Rendezvous completers wake the parked peer on the completer's own
CPU per ADR-013 §Q4 (consumer-first wake, ADR-011 BUILD-1 amendment).

Adds wake_thread_on_current_cpu helper for the Q4 enqueue path.
Updates Thread.cpu_id at the wake site.

New scenarios:
  - smp_send_blocks_first_then_completes (NEO_SMP=4)
  - smp_recv_blocks_first_then_completes (NEO_SMP=4)

Both verify cross-CPU rendezvous: blocker resumes on the completer's CPU.

Reference: Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md §4 sub-step 3.
```

---

## 5. SUB-STEP 4 — Grant/recv_grant retrofit

Symmetric to Sub-step 3 for the capability transfer paths.

### 5.1 Retrofit `Channel::grant`

Same pattern as `send`. Hold channel SwitchMutex through `block_current_into_with_guard`. Q4 wake-on-completer-CPU when completing a `RecvGrantBlocked`.

### 5.2 Retrofit `Channel::recv_grant`

Same pattern as `recv`. Capability rights computation (`donor_actual & offered & recipient_max` per ADR-010) is unchanged; only the lock-holding pattern changes.

### 5.3 Add `smp_grant_cross_cpu` test scenario

Test logic:
1. Grantor on CPU 0 has a derived capability (e.g., a SEND-only channel cap).
2. Recipient on CPU 1 has an empty handle slot.
3. Grantor calls `sys_grant`, blocks (no recipient yet); `blocked_cpu=0`.
4. Recipient calls `sys_recv_grant` on CPU 1; rendezvous completes; `completer_cpu=1`.
5. Verify recipient handle has correct rights (subset of grantor's).
6. Grantor resumes; `resumed_cpu=1`.
7. PASS frame: `blocked_cpu=0 completer_cpu=1 resumed_cpu=1 grant_rights=<expected_mask>`.

### 5.4 Pre-commit checklist for Sub-step 4

- [ ] `cargo k` passes; 18 warnings.
- [ ] `smp_grant_cross_cpu` PASSes.
- [ ] Existing cap scenarios (`cap_grant_basic`, `cap_revoke_local`, `cap_survive_exit`, etc.) all pass at NEO_SMP=4.
- [ ] PASS frame includes `grant_rights` field.
- [ ] Backup runs after commit lands.

### 5.5 Commit message for Sub-step 4

```
ipc: Q4+Q6 retrofit for grant/recv_grant (SC-3B)

Symmetric to send/recv retrofit. Channel::grant and Channel::recv_grant
now hold channel SwitchMutex through block_current_into_with_guard with
Q4 wake-on-completer-CPU semantics. Capability rights computation
(donor_actual & offered & recipient_max) preserved unchanged from BUILD-1.

New scenario: smp_grant_cross_cpu (NEO_SMP=4) verifies cap transfer with
correct rights and cross-CPU resume.

All four IPC block paths (send, recv, grant, recv_grant) now SMP-safe.
ADR-011 single-CPU rendezvous invariants formally superseded by ADR-013
§Q4+§Q6.

Reference: Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md §4 sub-step 4.
```

---

## 6. SUB-STEP 5 — Migration/reap + stress

### 6.1 Add `smp_cross_cpu_migration_reap` test scenario

Test logic:
1. Spawn worker on CPU 0; worker blocks in a channel rendezvous.
2. Completer on CPU 1 unblocks worker; worker resumes on CPU 1 (cpu_id migrates 0 → 1).
3. Worker exits on CPU 1.
4. Reap completes; verify worker thread is freed.
5. PASS frame: `blocked_cpu=0 resumed_cpu=1 exit_reaped=ok`.

This catches stale CPU-ownership bugs (e.g., reaper looking at the wrong CPU's queues).

### 6.2 Add `smp_cross_cpu_rendezvous_stress` test scenario

Stress test. Logic:
1. Place N senders on CPUs 0..NEO_SMP-1 (e.g., 4 senders at NEO_SMP=4).
2. Place N receivers on CPUs 0..NEO_SMP-1.
3. Run K iterations where senders and receivers rendezvous on shared channels.
4. Mix sender-blocks-first and receiver-blocks-first deterministically (e.g., alternating iterations).
5. Each iteration checks: payload integrity, no lost messages, no duplicate deliveries.
6. PASS frame: `iterations=<K> send_first=<n_send_first> recv_first=<n_recv_first>`.

K should be large enough to exercise the lock-order machinery thoroughly. Suggested K = 1000 iterations per scenario invocation.

### 6.3 Run the verification gate

After this commit, run the verification gate per §9. Specifically:

- 200 boots of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4. Zero failures of any classifier signature.
- 50 boots each of `smp_send_blocks_first_then_completes`, `smp_recv_blocks_first_then_completes`, `smp_grant_cross_cpu`, `smp_cross_cpu_migration_reap`. Zero failures.
- Anti-regression: full default `cargo neo-test` suite passes 50/50 at NEO_SMP=4.
- Supplementary: 25 boots of stress at NEO_SMP=2.

Document results in a verification log (see §6.5).

### 6.4 Pre-commit checklist for Sub-step 5

- [ ] `cargo k` passes; 18 warnings.
- [ ] All new scenarios in §6.1, §6.2 PASS in single-boot sanity check.
- [ ] Backup runs after commit lands.
- [ ] Verification gate (§9) executed and recorded BEFORE Sub-step 6 closes.

### 6.5 Verification log artifact

Produce `Plans/SC-3B-VERIFICATION-2026-05-09.md` (or whenever the verification runs). Structure:

```
# SC-3B verification log

## Gate steps
[per-step results]

## Statistical interpretation
[rate analysis, comparable to L's discharge framing]

## Failure classification
[any timeouts or failures, classified by failure-class enumeration in §9.4]

## Conclusion
[pass / partial pass / fail; recommendation for Sub-step 6]
```

This is a working-tree artifact; not a commit. Captured by next backup.

### 6.6 Commit message for Sub-step 5

```
test: SC-3B cross-CPU IPC stress + migration/reap scenarios

Adds the primary closure-gate scenario (smp_cross_cpu_rendezvous_stress)
and the migration/reap correctness scenario (smp_cross_cpu_migration_reap).

Stress scenario runs 1000 mixed sender-first/receiver-first rendezvous
iterations across NEO_SMP=4. Migration/reap scenario verifies cross-CPU
exit cleanup after Q4 wake-on-completer-CPU migration.

Verification log at Plans/SC-3B-VERIFICATION-2026-05-09.md (working-tree).
200/200 stress at NEO_SMP=4, zero historical-signature timeouts. Anti-
regression set passes 50/50 across all existing scenarios. Supplementary
NEO_SMP=2 25/25 clean.

Reference: Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md §4 sub-step 5,
§9 verification methodology.
```

---

## 7. SUB-STEP 6 — Documentation + close

### 7.1 Update ADR-013 §Q6

Append an "Implementation 2026-05-09" subsection to `docs/ADR-013-phase-2-smp-retrofit.md` §Q6:

```
### Implementation 2026-05-09

Q6 implemented as `context_switch_with_release2` per `Plans/SC-3B-PLAN-
SYNTHESIS-2026-05-09.md` §1.1 ratified shape:

- Per-call-site specialization (not const-generic array).
- Explicit register lock pointers (RDX = channel, RCX = scheduler).
- Release order: Channel first, scheduler second per Invariant I1.
- Lock-byte ownership: kernel-owned `SwitchMutex` type, not
  `spin::Mutex` internals.
- No fence; x86 TSO + Release semantics sufficient.
- No RFLAGS save/restore.
- Thread.cpu_id field added to support Q4 wake-on-completer-CPU.

ADR-011 single-CPU rendezvous invariants formally superseded.

Implementation commits: [sub-step 1 SHA] through [sub-step 5 SHA].
Verification: Plans/SC-3B-VERIFICATION-2026-05-09.md.
```

### 7.2 Update ADR-013 §Q4

Append an "Implementation 2026-05-09" subsection covering the wake-on-completer-CPU implementation, Thread.cpu_id usage, and the consumer-first wake semantics preserved from ADR-011 BUILD-1.

### 7.3 PHASE-2.md update

Update `docs/phases/PHASE-2.md` BUILD-3 section: add SC-3B sub-commit description, ADRs amended, commits, and "Next build: BUILD-3 sub-commit 4 (Q7 TLB management)".

### 7.4 Close report

Produce `Plans/CLOSE-REPORT-2026-05-09-sc-3b.md`. Structure mirrors prior close reports:

- Sub-commit summary (what shipped)
- Verification results (cite §9 gate)
- Anti-regression confirmed
- Bible 8.5.3 partial advancement noted
- Forward state (Q7, Q8, A2 pending)
- Cross-references

### 7.5 INDEX sweep

Update `Plans/INDEX.md`:
- Add entries for all SC-3B artifacts (PLAN docs, synthesis, verification log, close report, this dispatch).
- Status flips on artifacts whose status changed.
- Move SC-3B from "Open" planning state to closed sub-commit reference.

This INDEX sweep can be Jarvis's work if PL prefers parallel — Cerebro can land the close report and ADR-013 amendments while Jarvis handles INDEX. Or Cerebro lands all of Sub-step 6 sequentially. PM defers to PL.

### 7.6 Pre-commit checklist for Sub-step 6

- [ ] ADR-013 §Q4 and §Q6 amended.
- [ ] PHASE-2.md updated.
- [ ] Close report at `Plans/CLOSE-REPORT-2026-05-09-sc-3b.md`.
- [ ] INDEX.md updated (or dispatched to Jarvis).
- [ ] `cargo k` passes; full default scenario suite passes at NEO_SMP=4.
- [ ] Backup runs after final commit.

### 7.7 Commit message templates for Sub-step 6

For ADR/docs commit:
```
docs: ADR-013 §Q4+§Q6 implementation 2026-05-09 + PHASE-2.md SC-3B close

[body summarizing what was implemented, references SC-3B-VERIFICATION,
mentions ADR-011 supersession, advances Bible 8.5.3 partially.]
```

For close report (working-tree only, no commit):
N/A — close report is in `Plans/`, not tracked.

For INDEX sweep (separate commit, possibly Jarvis):
```
docs: Plans/INDEX.md — SC-3B sub-commit close + artifact sweep

[body listing entries added/updated]
```

---

## 8. SUB-COMMIT BOUNDARY

All commits in Sub-steps 1-6 ship within ONE BUILD-3 sub-commit boundary. Cerebro decides exact commit count within the bounds (~6-10 commits). At sub-commit close, the SC-3B sub-commit is reported as complete and BUILD-3 is ready for sub-commit 4 (Q7).

---

## 9. Verification methodology (the gate)

Per Cerebro's PLAN §Q7 and synthesis §1.9:

### 9.1 Gate order

Execute in sequence; stop on first failure:

1. `docker compose run --rm neo cargo k` — PASS (18 warnings).
2. Single `cargo neo-tty` boot at NEO_SMP=4 — clean boot, no panic, leak audit clean.
3. Single run of each new scenario at NEO_SMP=4:
   - `smp_block_release_smoke`
   - `smp_send_blocks_first_then_completes`
   - `smp_recv_blocks_first_then_completes`
   - `smp_grant_cross_cpu`
   - `smp_cross_cpu_migration_reap`
   - `smp_cross_cpu_rendezvous_stress` (single iteration check)
4. Full default `cargo neo-test` suite at NEO_SMP=4 — all PASS, no FAIL.
5. 50-run loop for each supplementary scenario (`smp_block_release_smoke`, `smp_send_blocks_first_then_completes`, `smp_recv_blocks_first_then_completes`, `smp_grant_cross_cpu`, `smp_cross_cpu_migration_reap`) at NEO_SMP=4 — 50/50 PASS each.
6. 200-run loop for `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4 — 200/200 PASS, zero classifier-signature timeouts.
7. 25-run smoke for `smp_cross_cpu_rendezvous_stress` at NEO_SMP=2 — 25/25 PASS.

Optional: 25-run smoke for stress at NEO_SMP=8 if hardware permits.

### 9.2 PASS criteria per scenario

Every PASS frame must satisfy:

- BEGIN frame observed.
- Exactly one terminal PASS or FAIL frame (no duplicate terminal frames).
- Terminal frame is PASS.
- Debug-exit code matches PASS.
- No `BUG:` line.
- No `KERNEL PANIC` line.
- No timeout.
- Frame leak audit clean when scenario reaches process teardown.

### 9.3 Required PASS frame fields per Cerebro PLAN

- `blocked_cpu=<id>` for Q6 scenarios.
- `completer_cpu=<id>` for Q4 scenarios.
- `resumed_cpu=<id>` for Q4 scenarios.
- `iterations=<n>` for stress.
- `send_first=<n>` and `recv_first=<n>` for stress.
- `grant_rights=<mask>` for grant scenarios.
- `exit_reaped=ok` for migration/reap.
- `release_path=asm` for `smp_block_release_smoke`.

### 9.4 Failure classification

If any timeout or failure occurs in the gate, classify against:

- **Lost wakeup** — blocker never resumes.
- **Deadlock with one waiter unpublished** — block site held lock too long; completer never sees waiter.
- **Deadlock with waiter published but not woken** — Q4 wake-on-completer-CPU broken.
- **Wrong CPU migration** — `resumed_cpu != completer_cpu` (Q4 violation).
- **Duplicate terminal frame** — test harness or scheduler double-reporting.
- **Missing terminal frame** — kernel hung pre-teardown.
- **Panic or BUG** — pure kernel correctness violation.
- **Frame leak** — Thread reaper missed a thread.
- **Rights mismatch in grant** — capability rights computation off.
- **Unexpected EBADF or EPIPE** — channel state corruption.

### 9.5 Statistical confidence target

200/200 on `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4 is the closure signal. This mirrors the wedge-fix-discharge framing: P[200/200 PASS | underlying rate = X] is small for X > ~0.5%.

If a single timeout occurs in the 200-run loop, do NOT extend to 1000+ in pursuit of statistical zero. Stop, classify, report, and bring to PM for triage.

---

## 10. Stop conditions

Halt the BUILD and ping PM if any of these fire:

- Any failure in existing default scenarios after Sub-step 1 (regression).
- Any timeout in a new cross-CPU scenario before its first PASS.
- Any PASS frame where `blocked_cpu == completer_cpu` in a cross-CPU test (placement hook broken or test logic flawed).
- Any need to add a third lock to `block_current_into_with_guard` (suggests scope creep or design issue).
- Any heap allocation discovered between `SwitchMutex` lock acquisition and `context_switch_with_release2` entry (Bible 8.5.3 violation; A2 territory).
- Any panic between `mem::forget` and the asm call (panic-safety window broken).
- Any 18-warning baseline violation (new compile warnings introduced).
- More than 2 backup script failures in a row (something's structurally wrong).

---

## 11. Sub-step ordering and dependencies

```
Sub-step 1 (infrastructure)
    ↓
Sub-step 2 (Q6 helper + smoke)
    ↓
Sub-step 3 (send/recv retrofit + Q4 + 2 scenarios)
    ↓
Sub-step 4 (grant/recv_grant retrofit + 1 scenario)
    ↓
Sub-step 5 (migration/reap + stress + verification gate)
    ↓
Sub-step 6 (docs + close)
```

Each step ends with a working build at the 18-warning baseline and all current scenarios passing at NEO_SMP=4. Ordering is forced; no parallelism within sub-steps.

---

## 12. Cross-references

- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` — the ratified PLAN that drives this BUILD.
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` — Cerebro's PLAN, primary input.
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md` — Prometheus's parallel PLAN (informational).
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — Invariants I1–I6.
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md` — THINK-stage scope ratification.
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6 — substrate.
- `docs/ADR-011-single-cpu-ipc-handoff-invariants.md` — superseded; SC-3B closes the supersession.
- `docs/bible.md` Articles 1.2, 1.3, 4.3, 8.3, 8.5, 12.1, 14, 15.2 — governing posture.
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §4.6, §10.2 — AI-native primitives that reuse this work.
- `Plans/PM-NOTE-2026-05-08-beta-outcome.md` — context for A1 composition.
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — wedge fix discharge.

---

**End of dispatch.**

*~6-10 commits within one BUILD-3 sub-commit boundary. Q4 + Q6 + 4 IPC retrofit sites + cross-CPU scenarios + 200/200 stress closure gate. Ratified positions: per-call-site ABI, SwitchMutex, Channel-first release order, Thread.cpu_id, test placement hook. PM is on call for any clarification during BUILD. Cerebro decides commit splits within sub-step bounds.*
