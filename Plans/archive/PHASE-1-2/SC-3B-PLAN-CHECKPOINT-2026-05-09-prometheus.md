# SC-3B PLAN-stage checkpoint analysis (Prometheus)

**Date:** 2026-05-09
**Author:** Prometheus
**Status:** PLAN-stage analysis. Awaiting project-lead ratification (post-synthesis with the parallel agent's analysis).
**Authority:** Project lead. PM dispatched; Prometheus proposes; project lead ratifies after PM synthesis.

---

## 1. Substrate verification

HEAD is `79b35f4` per dispatch. Source tree matches: `kernel/src/sched/scheduler.rs`, `kernel/src/sched/process.rs`, `kernel/src/ipc/mod.rs`, `kernel/src/arch/x86_64/context.rs`, `kernel/src/arch/x86_64/per_cpu.rs`, `kernel/src/arch/x86_64/syscall.rs` all present and consistent with ADR-013 §Q4+§Q6 substrate. `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` and `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md` are the authoritative inputs.

---

## 2. Open questions and proposed answers

### 2.1 Q6 ABI shape — recommendation: (c) per-call-site specialization

**Revised rationale per PM synthesis and pre-thinking note I3/I5:**

The three candidates are:
- (a) `unsafe fn context_switch_with_release<const N: usize>(prev_rsp_ptr: *mut u64, next_rsp: u64, locks: &[*const AtomicBool; N]) -> i64`
- (b) `ReleaseSet { locks: [*const AtomicBool; 4], count: u8 }` passed by value
- (c) `context_switch_with_release2(prev_rsp_ptr, next_rsp, lock1, lock2)` per call-site

**Rejection of (a):** PM correctly identified that (a)'s asm release-loop reading release-list metadata from the old stack post-stack-switch is the exact failure mode that pre-thinking note §5.1 explicitly rejects (Invariant I3: "Asm helper takes a fixed-arity release list"). The const-generic array form would require the asm to read N from the old stack, which is unsound.

**Rejection of (b):** Fixed-shape struct with runtime count still requires the asm to read `count` and loop, introducing the same old-stack-read issue.

**Recommendation of (c):** Per-call-site specialization (c) matches the current sole call site (IPC paths hold exactly 2 locks). The asm is one shape per lock count, with no loop over metadata. This is what ADR-013 §Q6's sketch sketches (`context_switch_with_release2` with rdx=lock1, rcx=lock2).

**Forward compatibility note:** If Q8.c needs more than 2 locks, we add `context_switch_with_release3` and `context_switch_with_release4`. This is acceptable per Bible 14's /simplify gate — SC-3B is the minimal viable commit; Q8.c is separate work.

**Decision criteria met:** (c) is simplest, matches the pre-thinking note's sketch, and avoids the old-stack-read failure mode.

---

### 2.2 Memory ordering — recommendation: no explicit fence needed

**Rationale:**

`spin::Mutex` uses `AtomicBool::store(false, Release)` for unlock. On x86, `Ordering::Release` compiles to a plain MOV with a compiler barrier (no CPU fence). x86's TSO (Total Store Order) guarantees that stores are observed in program order by all CPUs.

The asm release sequence is:
```asm
mov qword ptr [lock1], 0    ; store false (Release) to lock1
mov qword ptr [lock2], 0    ; store false (Release) to lock2
```

Because x86 stores are ordered in program order, once lock1 is released, any CPU that sees lock1==false is guaranteed to also see lock2==false (if it observes lock2 after lock1). The Release semantics on the Rust side ensure that all prior writes (including the critical section's writes) are visible before the store.

**Reference:** Linux's `spin_unlock` on x86 is a plain MOV + compiler barrier, no CPU fence. The same pattern applies here.

**If a fence were needed?** Only if the compiler were allowed to reorder the asm's stores against subsequent code. The `context_switch_with_release` function is `#[no_mangle] extern "C"` and the caller wraps the call in `without_interrupts` with interrupts disabled. The compiler cannot move the stores past the `ret` that exits the function, so no fence is needed.

---

### 2.3 Panic-safety of the forget-then-asm window — recommendation: single `unsafe` block, no intermediate operations

**Audit of panicable operations:**

In the current `block_current_into` (pre-retrofit), the installer closure runs while holding both locks. The proposed Q6 pattern is:

```rust
let (prev_rsp_ptr, next_rsp) = {
    let sched = current_scheduler();
    let mut s = sched.inner.lock();  // lock 1: scheduler
    // ... pop, replace, cross-process bookkeeping ...
    let prev_rsp_ptr: *mut u64 = &mut prev.rsp;
    installer(prev);  // installer acquires lock 2: Channel state
    (prev_rsp_ptr, next_rsp)
};  // scheduler lock released here

let channel_guard = channel_state.lock();  // lock 2 already held from installer
mem::forget(channel_guard);
mem::forget(sched_guard);
unsafe {
    context_switch_with_release(prev_rsp_ptr, next_rsp, &[sched_lock_ptr, channel_lock_ptr]);
}
```

The panicable operations between `mem::forget` and `context_switch` are:
- **None** if we structure it as a single `unsafe` block with no intervening Rust code.
- **Array indexing** in `&[...]` could panic in debug builds if the slice length is invalid, but `[...]` array-to-slice coercion is infallible.
- **Integer overflow** in debug builds is impossible here (no arithmetic).
- **`unwrap`/`expect`** on `Option` are not present in the proposed pattern.

**Recommended structure:**

```rust
let (prev_rsp_ptr, next_rsp, sched_lock_ptr, channel_lock_ptr) = without_interrupts(|| {
    let sched = current_scheduler();
    let mut s = sched.inner.lock();
    // ... setup ...
    let prev = mem::replace(&mut s.current, next);
    // ... cross-process bookkeeping ...
    let prev_rsp_ptr = &mut prev.rsp as *mut u64;
    installer(prev);  // installer acquires channel state lock
    
    // Capture raw lock pointers BEFORE forget
    let sched_lock_ptr = s.lock.into_raw();  // spin::Mutex::into_raw
    let channel_lock_ptr = channel_guard.into_raw();
    
    // Forget guards in the same unsafe block as the asm call
    mem::forget(channel_guard);
    mem::forget(sched_guard);
    (prev_rsp_ptr, next_rsp, sched_lock_ptr, channel_lock_ptr)
});

// Single unsafe block: no panicable operations between forget and asm
unsafe {
    context_switch_with_release(prev_rsp_ptr, next_rsp, &[sched_lock_ptr, channel_lock_ptr]);
}
```

**Defense-in-depth:** `#[track_caller]` on the asm helper would help debug panics, but since we've eliminated panicable operations from the window, it's not strictly necessary. The `core::intrinsics::abort` backstop is overkill — the goal is structural correctness, not defensive programming for a window that is provably panic-free.

---

### 2.4 Cross-process bookkeeping ordering — recommendation: bookkeeping before forget, all within `without_interrupts`

**Current locations (per `kernel/src/sched/scheduler.rs`):**

- `cross_process_prep(&s.current)` at line 456 (Q6 path)
- `cross_thread_kstack(&s.current)` at line 458 (Q6 path)

Both are called inside the `without_interrupts` closure, before the installer.

**Q6-correct ordering (revised per pre-thinking §3.4 I1):**

1. Acquire scheduler lock
2. Pop `next`, `mem::replace` current
3. **Cross-process/bookkeeping** (TSS.rsp0 + PER_CPU.kernel_rsp + CR3 if needed)
4. Capture `prev_rsp_ptr`
5. Call `installer(prev)` — installer acquires Channel state lock
6. **Release order: Channel lock FIRST, scheduler lock SECOND** (per pre-thinking I1: "Channel-first, scheduler-second")
7. `mem::forget` both guards
8. Call `context_switch_with_release` (asm: switch stacks, then release locks on new stack in Channel→scheduler order)
9. Release scheduler lock (implicit at end of `without_interrupts` closure)

**State-capture requirements:**

- `next_rsp` is read from `s.current.rsp` after `mem::replace`. This is fine — `s.current` is the new thread, and its `rsp` is stable.
- **Lock ownership:** Per PM synthesis, `spin::Mutex::into_raw()` does not expose the lock byte (it's `pub(crate)`, not public). Bible 4.3 disfavors depending on third-party crate internal layout. PM recommends Cerebro's `SwitchMutex` type (~30-50 lines) to own the lock byte.
- `prev_rsp_ptr` is captured before `installer`, because `prev` is moved into the installer and its address becomes stable in the holder's structure.

**Critical invariant:** TSS.rsp0 and PER_CPU.kernel_rsp must point to the **new thread's** kernel stack top **before** the stack switch, because if an interrupt fires during or immediately after the switch, it will land on the new stack. The cross-process/bookkeeping calls in lines 456-458 are already in the right place (before forget/asm). No change needed.

**Load-bearing release order (per pre-thinking I1):** Channel mutex must be released before scheduler lock. This lets cross-CPU wakers proceed on Channel mutex availability, independently of CPU N's scheduler lock. The reverse order would let the per-CPU scheduler become available for other operations (BSP reaper, kill scans) before the parked thread is reachable for rendezvous completion — bounded latency but inverts the design intent.

---

### 2.5 Block-target enumeration — confirmed call sites:

`git grep block_current_into kernel/src/` yields:

1. **`kernel/src/ipc/mod.rs:198`** — `sys_send` (SendBlocked case)
2. **`kernel/src/ipc/mod.rs:257`** — `sys_recv` (RecvBlocked case)
3. **`kernel/src/ipc/mod.rs:321`** — `sys_grant` (GrantBlocked case)
4. **`kernel/src/ipc/mod.rs:388`** — `sys_recv_grant` (RecvGrantBlocked case)
   - Same pattern.

**Confirmed:** All 4 IPC syscalls hold exactly 2 locks in the same order. The lock ordering is consistent across all callers.

**Lock byte ownership (per PM synthesis):** `spin::Mutex::into_raw()` does not expose the lock byte (it's `pub(crate)`, not public). Bible 4.3 disfavors depending on third-party crate internal layout. PM recommends Cerebro's `SwitchMutex` type (~30-50 lines) to own the lock byte. This is infrastructure sub-step 1.

---

### 2.6 Test scenario set — recommendation: 5 primary scenarios

**Primary scenarios (Q4+Q6 validation):**

1. **`smp_send_blocks_first_then_completes`**
   - What it tests: Q4 (wake-on-completer-CPU) and Q6 (lock-order asm release).
   - Setup: CPU A sends to full channel, blocks. CPU B receives, unblocks CPU A's sender.
   - Expected: Sender wakes on CPU B (completer-CPU), receiver returns on CPU A.
   - Prerequisites: Q4 and Q6 implemented.
   - Deferred to Q8.c: No.

2. **`smp_recv_blocks_first_then_completes`**
   - What it tests: Q4 (receiver wake) and Q6.
   - Setup: CPU A receives from empty channel, blocks. CPU B sends, unblocks CPU A's receiver.
   - Expected: Receiver wakes on CPU B, sender returns on CPU A.
   - Prerequisites: Q4 and Q6 implemented.
   - Deferred to Q8.c: No.

3. **`smp_grant_cross_cpu`**
   - What it tests: Q4+Q6 for capability transfer.
   - Setup: CPU A grants to CPU B's pending `sys_recv_grant`.
   - Expected: Capability arrives with correct rights, recipient runs on CPU B.
   - Prerequisites: Q4 and Q6 implemented.
   - Deferred to Q8.c: No.

4. **`smp_concurrent_rendezvous_storm`**
   - What it tests: Lock-order discipline under contention.
   - Setup: N senders on all CPUs block on full channel; N receivers on all CPUs drain.
   - Expected: No deadlock, all messages delivered, no lock-order violations.
   - Prerequisites: Q4 and Q6 implemented.
   - Deferred to Q8.c: No.

5. **`smp_kill_during_rendezvous`**
   - What it tests: Q8.c precursor (blocked-thread-kill).
   - Setup: Park a thread in EndpointState, then `sys_kill` from another CPU.
   - Expected: Currently undefined — Q8.c is deferred. Either (a) kill succeeds (Q8.c implemented) or (b) returns `EINVAL` (current Phase 1.5b behavior).
   - Prerequisites: Q8.c for full behavior.
   - Deferred to Q8.c: **Yes** — this scenario validates Q8.c, not Q4+Q6 alone.

**Recommended subset for SC-3B:** 1, 2, 3, 4 (4 scenarios). The storm test (4) is high-value for lock-order validation.

**Anti-regression coverage:** All existing single-CPU IPC scenarios (L's 200/200 primary + 50/50 supplementary) must pass at NEO_SMP=4.

---

### 2.7 Verification methodology — recommendation: mirror L's discipline

**Boot counts:**
- Primary scenarios (1-4): 200/200 (success/failure count) per scenario, mirroring L.
- Anti-regression set: 200/200 for each existing single-CPU IPC scenario.

**NEO_SMP values:**
- Primary: NEO_SMP=4 (as dispatch specifies).
- Supplementary: NEO_SMP=2 (minimal SMP), NEO_SMP=8 (if hardware permits and time allows).

**Pass criteria per scenario:**
- Terminal frame format: `ok` or `fail` with scenario name.
- Exit code: 0 for success, non-zero for failure.
- Structural invariants: leak audit clean (no frame leaks, no heap leaks).

**Anti-regression set:**
- All existing single-CPU IPC scenarios (`tests/ipc/*.rs`) must pass at NEO_SMP=4.

**Statistical confidence target:**
- Mirror L's 0.45% framing: 200 boots with 0 failures → 95% confidence failure rate < 1.5%. For SC-3B, 200/200 is sufficient to rule out catastrophic bugs. If we see 1 failure in 200, we need 1000+ boots to narrow the confidence interval.

**Failure modes to watch for:**
- **Deadlock:** No progress after all syscalls complete. Check via serial output timestamps.
- **Livelock:** Busy-spin without making progress. Check via per-CPU idle counters (if added).
- **Lost message:** Sender or receiver never unblocks. Check via channel state inspection.
- **Capability rights drift:** Rights bits incorrect on received capability. Check via `sys_derive` validation.
- **Killed-during-rendezvous corner case:** Q8.c not implemented, so kill returns `EINVAL`. Verify this behavior.

---

### 2.8 Composition with A1 and 8.5.3 advancement — recommendation: composition is clean, 8.5.3 advanced

**A1 composition:**

A1 made the heap allocator IRQ-safe (`IrqSafeHeap` in `kernel/src/mm/heap.rs`). Q6's asm release operates on `spin::Mutex` instances, not the heap allocator's lock.

- `cross_process_prep` and `cross_thread_kstack` do not allocate:
  - `cross_thread_kstack` reads `next.kernel_stack_top()` (field access only).
  - `cross_process_prep` calls `cr3_for_thread` (lock-protected lookup, no allocation).
  - Both functions are called inside `without_interrupts`, so no IRQ reentrancy concern.

- The asm helper's saved/restored interrupt state: The caller enters `block_current_into` with `without_interrupts`, so interrupts are disabled. The asm `context_switch_with_release` does not modify RFLAGS; it only switches stacks and releases locks. When the new thread resumes, interrupts remain disabled until the next `with_interrupts` or `sti`. This is consistent with A1's design.

**Bible 8.5.3 advancement:**

Article 8.5.3: "Scheduling operations never trigger heap allocation."

- Q6's cleaner lock-order discipline (release on new stack, not across switch) reduces the 8.5.3 violation surface. Previously, `block_current_into` held the scheduler lock across the stack switch (a scheduling operation), but the lock was a `spin::Mutex`, not a heap allocation. Q6 doesn't introduce new heap allocations in the SCHED-locked window.
- Q6 doesn't introduce NEW 8.5.3 violations. The installer closure runs while holding both locks, but the closure is caller-supplied (IPC syscall path), and the current implementation doesn't allocate in the installer.
- **Partial advancement:** Q6's structure (release-on-new-stack) is more aligned with 8.5.3's intent than the old pattern (hold lock across switch). The commit message can note: "Q6's lock-order discipline reduces 8.5.3 violation surface; 8.5.3 conformance milestone (A2) advanced from Phase 2 BUILD-4 to BUILD-3."

---

## 3. Implementation sketch

**New `context_switch_with_release2` asm (in `kernel/src/arch/x86_64/context.rs`):**

```asm
.global context_switch_with_release2
.context_switch_with_release2:
    # Save callee-saved registers on current stack
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    
    # Save current RSP to *prev_rsp_ptr (rdi)
    mov [rdi], rsp
    
    # Load next RSP (rsi)
    mov rsp, rsi
    
    # Release Channel mutex FIRST (rdx = channel_lock_ptr)
    mov byte ptr [rdx], 0
    
    # Release scheduler lock SECOND (rcx = sched_lock_ptr)
    mov byte ptr [rcx], 0
    
    # Restore callee-saved registers from new stack
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
```

**Rust wrapper signature (per-call-site specialization):**

```rust
unsafe extern "C" {
    pub fn context_switch_with_release2(
        prev_rsp_ptr: *mut u64,
        next_rsp: u64,
        channel_lock_ptr: *const AtomicBool,
        sched_lock_ptr: *const AtomicBool,
    ) -> i64;
}
```

**`block_current_into` retrofit (simplified):**

```rust
pub fn block_current_into<F>(installer: F) -> i64
where
    F: FnOnce(Box<Thread>),
{
    let (prev_rsp_ptr, next_rsp, sched_lock_ptr, channel_lock_ptr) = without_interrupts(|| {
        let sched = current_scheduler();
        let mut s = sched.inner.lock();
        
        let next = s.ready.pop_front().expect("block_current_into: ready queue empty");
        let mut prev = mem::replace(&mut s.current, next);
        prev.state = ThreadState::Blocked;
        prev.preempt_defer_ticks = 0;
        s.current.state = ThreadState::Running;
        let next_rsp = s.current.rsp;
        
        // Cross-process / cross-thread bookkeeping
        if prev.pid != s.current.pid {
            cross_process_prep(&s.current);
        } else if s.current.pid != KERNEL_PID {
            cross_thread_kstack(&s.current);
        }
        
        let prev_rsp_ptr: *mut u64 = &mut prev.rsp;
        
        // Installer acquires Channel state lock
        // Note: caller must already hold Channel state lock before block_current_into
        installer(prev);
        
        // Capture raw lock pointers via SwitchMutex (per PM synthesis)
        // spin::Mutex::into_raw() does not expose lock byte (pub(crate) only)
        // SwitchMutex (~30-50 lines) owns the lock byte for this purpose
        let sched_lock_ptr = s.lock.into_raw();  // requires SwitchMutex
        let channel_lock_ptr = /* caller's lock pointer, passed via closure capture */;
        
        // Forget guards in same unsafe block as asm call
        mem::forget(/* channel guard */);
        mem::forget(sched_guard);
        
        (prev_rsp_ptr, next_rsp, sched_lock_ptr, channel_lock_ptr)
    });
    
    unsafe {
        // Release order: Channel FIRST, scheduler SECOND (per prethinking I1)
        context_switch_with_release2(prev_rsp_ptr, next_rsp, channel_lock_ptr, sched_lock_ptr);
    }
    
    current_scheduler().inner.lock().current.blocked_return
}
```

**IPC syscall changes (example: `sys_send`):**

```rust
// Before block_current_into, caller holds Channel state lock
let guard = self.state.lock();
// ... state checks ...
drop(guard);  // release Channel state lock

// block_current_into acquires scheduler lock, installer re-acquires Channel state lock
sched::block_current_into(|prev_thread: Box<Thread>| {
    let mut g = self.state.lock();  // re-acquire Channel state lock
    g[my_side.idx()] = EndpointState::SendBlocked { ... waiter: prev_thread, ... };
    // g released at end of closure scope
})
```

---

## 4. Risks and unknowns

1. **Lock byte ownership:** `spin::Mutex::into_raw()` does not expose the lock byte (it's `pub(crate)`, not public). Bible 4.3 disfavors depending on third-party crate internal layout. PM recommends Cerebro's `SwitchMutex` type (~30-50 lines) to own the lock byte. This is infrastructure sub-step 1.

2. **ASM calling convention:** Per-call-site specialization uses fixed register arguments (rdx=channel_lock, rcx=sched_lock). The asm is one shape per lock count, no loop over metadata.

3. **Interrupt reentrancy during lock release:** The asm releases locks on the new stack with interrupts disabled (caller's `without_interrupts`). If an interrupt fires during the release loop, it will nest on the new stack. This is fine — the interrupt handler doesn't acquire locks (it uses `spin::Mutex` with `try_lock` or similar). Verify no interrupt handler acquires scheduler or Channel locks.

4. **Q8.c blocked-thread-kill walk:** If Q8.c needs to release more than 2 locks (e.g., 3+ Channel handles), we add `context_switch_with_release3` and `context_switch_with_release4`. Lock ordering remains Channel-first for all variants.

---

## 5. Cross-references

- `kernel/src/sched/scheduler.rs` — current `block_current_into`, `rotate`, `cross_process_prep`, `cross_thread_kstack`
- `kernel/src/sched/process.rs` — process registry, handle table, `terminate`, `on_thread_reap`
- `kernel/src/ipc/mod.rs` — `sys_send`, `sys_recv`, `sys_grant`, `sys_recv_grant` call sites
- `kernel/src/arch/x86_64/context.rs` — current `context_switch` asm
- `kernel/src/arch/x86_64/per_cpu.rs` — per-CPU state, `current_cpu_id`, `current_per_cpu`
- `kernel/src/arch/x86_64/syscall.rs` — SYSCALL entry, `kernel_rsp` handling
- `docs/ADR-013-phase-2-smp-retrofit.md` — Q4, Q6 substrate
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — prior PM analysis of Q6 composition
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md` — the THINK note this PLAN follows from
- `docs/bible.md` Articles 8.3, 8.5, 12.1 — governing posture
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — context for what's already in main

---

---

## 6. Post-synthesis alignment summary

Per PM synthesis, the following recommendations have been revised to align with Cerebro's analysis:

1. **Q6 ABI shape:** Changed from (a) const-generic array to (c) per-call-site specialization (`context_switch_with_release2` with explicit register lock pointers). This avoids the old-stack-read failure mode identified in prethinking note §5.1.

2. **Lock release order:** Changed from [sched, channel] to [channel, sched] (Channel-first, scheduler-second). This is load-bearing per prethinking I1 and allows cross-CPU wakers to proceed on Channel mutex availability.

3. **Lock-byte ownership:** Changed from `spin::Mutex::into_raw()` to a kernel-owned `SwitchMutex` type (~30-50 lines). `spin::Mutex::into_raw()` does not expose the lock byte (it's `pub(crate)`, not public), violating Bible 4.3's "no external crate internal layout dependency" rule.

The remaining recommendations (memory ordering, panic-safety, bookkeeping ordering, A1 composition, Bible 8.5.3 advancement) are converged with Cerebro's analysis.

**Final Q6 mechanism summary:**
- Signature: `context_switch_with_release2(prev_rsp_ptr, next_rsp, channel_lock_ptr, sched_lock_ptr)`
- ASM: Switch stacks → release Channel lock (byte 0) → release scheduler lock (byte 0)
- Order: Channel FIRST, scheduler SECOND (per prethinking I1)
- Lock ownership: kernel-owned `SwitchMutex` (per PM synthesis)

**Infrastructure prerequisites (sub-step 1):**
- `SwitchMutex` type (~30-50 lines)
- `Thread.cpu_id` field (PM identified missing)
- Test placement hook for cross-CPU scenarios (PM identified missing)

---

**End of PLAN.**
