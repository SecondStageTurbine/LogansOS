# Sub-commit 3a checkpoint pre-thinking: lock-order × cross-CPU reap interaction

**Date:** 2026-05-07
**Author:** PM
**Status:** Design notes — informational, pre-checkpoint scratchpad. NOT a dispatch, NOT an ADR amendment, NOT normative. Becomes input to the SC 3a pre-implementation checkpoint conversation when SC 3a is scheduled (after sub-commit 2 closes).
**Substrate:** ADR-013 §Q6 specifies SC 3a's mechanism (ManuallyDrop + asm release on new stack). Sub-commit 2 ships BSP-scans-all-CPUs reap (Q9 deferred). These two mechanisms compose; this note explores the interaction surface.

---

## 1. Why this note exists

ADR-013 §Q6 was written before sub-commit 2's reaper became "BSP-scans-all-CPUs." The §Q6 design conversation assumed sub-commit 2 would ship per-CPU reapers (the original §Q9 mechanism). That mechanism was deferred. The shipped sub-commit 2 reaper is structurally different from what §Q6 anticipated: instead of each CPU reaping its own zombies on its own bootstrap thread, *the BSP*'s bootstrap thread iterates `0..N` per-CPU schedulers, takes each one's lock briefly, drains its zombies queue, and processes them locally on the BSP.

This means **the BSP can take CPU N's scheduler lock at any moment**, not just when CPU N is idle. That changes the lock-contention shape that §Q6's mechanism interacts with. SC 3a's pre-implementation checkpoint should think about this before writing the asm helper, not during.

The note doesn't propose changes to §Q6's choice — Option (b) ManuallyDrop + asm release remains the right answer. The note explores the *contention pattern* SC 3a's primitive will operate under, and surfaces a few invariants the asm helper must preserve.

---

## 2. The two mechanisms in play

### 2.1 SC 2 (working tree, uncommitted): BSP-scans-all-CPUs `reap()`

The BSP's bootstrap thread runs the reaper loop. On each iteration:

```text
for cpu_id in 0..MAX_CPUS:
    if !per_cpu_scheduler_initialized(cpu_id): continue
    sched = per_cpu_scheduler(cpu_id)
    let zombies = without_interrupts:
        guard = sched.lock.lock()
        taken = mem::take(&mut sched.zombies)
        // ZOMBIE_COUNT decrement
        // guard drops here
    for zombie in zombies:
        Thread::free(zombie)         // kernel stack reclaim, slab cache return
        process::on_thread_reap(pid) // possibly removes Process from registry
```

The lock on CPU N's scheduler is held briefly: just long enough to `mem::take` the queue and decrement the zombie counter. The actual zombie processing (Thread::free, on_thread_reap) happens *after* the lock is released, on the BSP's own stack with the BSP's own current thread context.

APs do not reap. APs run their own bootstrap threads which `hlt`-loop or yield indefinitely. The BSP's reaper handles all CPUs.

### 2.2 SC 3a (planned per ADR-013 §Q6): ManuallyDrop + asm release in `block_current_into`

`block_current_into` becomes:

```text
without_interrupts:
    // Channel mutex is already held by caller (IPC syscall path).
    sched = current_per_cpu_scheduler()
    sched_guard = sched.lock.lock()

    next = sched.ready.pop_front()
    prev = mem::replace(&mut sched.current, next)
    prev.state = Blocked
    sched.current.state = Running
    next_rsp = sched.current.rsp

    // cross-process / cross-thread bookkeeping
    if prev.pid != sched.current.pid: cross_process_prep(...)

    prev_rsp_ptr = &mut prev.rsp
    installer(prev)  // moves prev Box into Channel EndpointState

    // Convert both held mutexes to raw lock pointers for asm release.
    channel_lock_ptr = channel_guard.into_raw_lock_ptr()
    sched_lock_ptr = sched_guard.into_raw_lock_ptr()
    mem::forget(channel_guard)
    mem::forget(sched_guard)

    context_switch_with_release(
        prev_rsp_ptr,
        next_rsp,
        &[channel_lock_ptr, sched_lock_ptr],
    )
    // ↑ asm: switch stacks, then on the new stack, release locks in order.

    // Resumed here on the previously-blocked thread later.
    current_per_cpu_scheduler().current.blocked_return
```

The critical invariant: **between the start of asm `context_switch_with_release` and the lock-release step on the new stack, both locks are held**. CPU N's scheduler lock is held during this window.

---

## 3. The composition surface

### 3.1 BSP reaper × CPU N's `block_current_into`: the simple cases

**Case A: BSP reaper runs first, completes; CPU N enters block_current_into later.**

BSP grabs CPU N's lock, drains zombies, releases. CPU N's IPC syscall acquires the lock, holds across asm, releases on new stack. No interaction. Sequential.

**Case B: CPU N enters block_current_into first; BSP reaper tries CPU N's lock during.**

CPU N has the lock. BSP spins on the lock. CPU N's asm release fires on the new stack — instant in nanoseconds. BSP acquires, drains, releases. Brief contention but bounded; no deadlock possible because the BSP holds no lock while waiting.

**Case C: Strict interleave — BSP iterates past CPU N just before CPU N enters block_current_into.**

BSP already moved on. CPU N's IPC syscall takes the lock unimpeded. Future BSP iterations will catch the new zombies if any. Steady state.

All three cases are fine. No new bugs surfaced by composition.

### 3.2 The interesting case: BSP reaper observing zombies during CPU N's block

Suppose CPU N's currently-running thread T1 is being killed by `sys_kill` from another CPU (call it CPU M) while CPU N is mid-block. Wait — `sys_kill` runs on the killer's CPU, not the target's. Let me reconsider.

In sub-commit 2's model, `kill_thread_by_pid` searches *all CPU* ready queues for the target. If found, it removes from ready and pushes to that CPU's zombie queue. So:

1. CPU M (killer) acquires CPU N's scheduler lock (cross-CPU spinlock acquire).
2. CPU M finds T1 in CPU N's ready queue (T1 was in CPU N's ready queue, *not* currently running on CPU N — recall §3.1's setup: CPU N is mid-block, so its current thread is the *new* thread, not T1).

Wait — that doesn't compose. Let me re-set the scene. If CPU N is mid-`block_current_into`, the prev thread is being moved to Blocked state and parked in a Channel EndpointState. It's *not* on CPU N's scheduler queues at all — it's in the Channel structure. So `kill_thread_by_pid` from CPU M wouldn't find it via a queue scan.

The actual blocked-thread-kill path requires walking the target process's handle table for Channel handles, then inspecting each Channel's EndpointState — that's ADR-013 §Q8.c, which is sub-commit 5 work. Sub-commit 3a doesn't have to handle blocked-thread-kill yet; SC 5 layers it on top.

So during the SC 3a window: a thread that's parked in Blocked state is invisible to `kill_thread_by_pid`. The only kill path that affects sub-commit 3a's semantics is "kill a thread that's in some CPU's ready queue."

### 3.3 The actually-interesting case: BSP reaper finds CPU N's stale `current`

Here's a subtle one. During CPU N's `block_current_into`:

1. CPU N takes its scheduler lock.
2. CPU N pops `next` from ready, calls `mem::replace(&mut sched.current, next)`. `sched.current` now points at `next`. The `prev` Box is in a local variable.
3. CPU N hands `prev` Box to the installer closure, which moves it into a Channel EndpointState.
4. CPU N enters asm `context_switch_with_release`.
5. ... asm ... releases CPU N's scheduler lock ... eventually finishes, returns into `next`'s code.

Between steps 4 and 5, what does the BSP reaper see if it acquires CPU N's scheduler lock? Answer: it can't, because CPU N still holds it. The asm release fires after the stack switch. So the BSP reaper waits.

But what about steps 1-3? At step 2 specifically, `sched.current` was atomically replaced from `prev` to `next`. If the BSP reaper observes `sched.current` between steps 2 and 3, it sees `next` — which is correct (next is the new running thread on CPU N), even though the asm switch hasn't actually happened yet.

This is a *temporal* gap: `sched.current = next` is true in the in-memory data structure before the asm switch makes it true at the hardware level. **Does any reaper code path care about this gap?**

The BSP reaper's loop body is roughly:

```text
guard = sched.lock.lock()     // blocks if CPU N holds it
zombies = mem::take(&mut sched.zombies)
guard drops
for zombie in zombies:
    Thread::free(zombie)
    process::on_thread_reap(pid)
```

The reaper reads `sched.zombies`, not `sched.current`. The temporal gap on `sched.current` is invisible to the reaper. ✅

### 3.4 The other interesting case: cross-CPU rendezvous wake during SC 3a

ADR-013 §Q4 specifies wake-on-completer-CPU. When CPU M's IPC syscall completes a rendezvous with a thread blocked from CPU N, CPU M extracts the thread from the Channel EndpointState and front-queues it on **CPU M's** scheduler — not CPU N's.

So in SC 3a's window:

- CPU N is in `block_current_into`, holds CPU N's scheduler lock + Channel mutex.
- CPU M wants to complete the rendezvous. CPU M needs to acquire the *Channel mutex* (not CPU N's scheduler lock).
- CPU M spins on Channel mutex. CPU N's asm release fires on the new stack — Channel mutex is released *first* (per ADR-013 §Q6's release order: Channel before scheduler).
- CPU M acquires Channel mutex, extracts the parked thread, releases Channel mutex.
- CPU M front-queues the woken thread on CPU M's own scheduler. CPU M never touches CPU N's scheduler lock.

This is the load-bearing reason §Q6 specifies the release order as "Channel mutex first, then scheduler lock":

- If scheduler lock was released first, then Channel mutex was held while a different CPU's scheduler operations happened — fine actually for correctness, but the reverse order (Channel-first) lets cross-CPU wakers proceed sooner and shortens the window during which the parked thread is "logically blocked but the rendezvous-completer can't reach it."
- More importantly: releasing Channel mutex first means the parked thread is immediately reachable for completion, and only then does CPU N's scheduler become available for other operations (BSP reaper, kill scans, etc.).

**Composition confirmed:** the release order Channel-then-scheduler composes correctly with sub-commit 2's BSP-scans-all-CPUs reap. The reaper never deadlocks because it holds no lock while waiting. The cross-CPU waker proceeds on Channel mutex availability, independently of CPU N's scheduler lock.

---

## 4. Invariants the asm helper must preserve

From the analysis above, the SC 3a asm helper must satisfy:

### Invariant I1: Release order is fixed Channel-first, scheduler-second

Reverse order is wrong because it lets the per-CPU scheduler become available for other operations before the parked thread is reachable for rendezvous completion. The reverse-order failure mode is bounded (no deadlock, just latency), but it inverts the design intent.

### Invariant I2: Both releases happen before the new thread's code resumes

Already specified in §Q6. Reaffirmed: the new thread cannot observe state where one lock is released and the other isn't, because it doesn't care about either lock. But subsequent system code (the new thread's syscall return path, e.g.) might call into the scheduler — if it observes "scheduler lock not yet released," it'll spin briefly, harmless. If it observes "Channel mutex not yet released" but tries to operate on that channel, also harmless (spins). Bounded latency, no correctness violation.

### Invariant I3: Asm helper takes a fixed-arity release list

ADR-013 §Q6 said "max 4 entries." The current SC 3a use case needs 2 (Channel mutex + scheduler lock). A future blocking primitive (e.g., futex with a userspace lock + scheduler lock + something else) might need 3 or 4. Bounding the arity is correct. The helper's signature should make the bound explicit — `&[*const AtomicBool; N]` with a const generic, or a fixed-shape struct.

### Invariant I4: The release list is not a `Vec`

The `Vec<*const AtomicBool>` form would heap-allocate during the syscall path. That's fine for correctness, but a perf-sensitive primitive should not allocate. Use stack arrays or const-generic arrays.

### Invariant I5: Every lock in the release list is owned-but-leaked

Each lock has `mem::forget` called on its guard before the asm. The asm performs the release as a single atomic store (e.g., `mov [lock_addr], 0`). If any lock isn't owned at the time of forget, the release is unsound — the asm would clear a lock the caller doesn't hold, letting a different waiter into a critical section under false pretenses. The const-generic / fixed-shape struct should make accidentally constructing a release list with stale lock pointers harder.

### Invariant I6: Lock release happens on the new stack

This is the entire point of §Q6's design. Spelling it out: the asm performs the stack switch *before* the release-list stores. After the stack switch, the old thread's stack is no longer in use — meaning a concurrent waker on another CPU can wake the old thread (push it to a ready queue, write blocked_return, etc.) without racing on the old thread's stack memory.

---

## 5. Concerns the SC 3a checkpoint should resolve

These are *open* questions for the implementation conversation when SC 3a is scheduled. Not answered here.

### 5.1 What's the exact ABI of `context_switch_with_release`?

Candidates:

```rust
// (a) Const-generic array:
unsafe fn context_switch_with_release<const N: usize>(
    prev_rsp_ptr: *mut u64,
    next_rsp: u64,
    locks: &[*const AtomicBool; N],
) -> i64;

// (b) Fixed shape with optional entries:
struct ReleaseSet {
    locks: [*const AtomicBool; 4],
    count: u8,
}
unsafe fn context_switch_with_release(
    prev_rsp_ptr: *mut u64,
    next_rsp: u64,
    releases: ReleaseSet,
) -> i64;

// (c) Per-call-site specialization:
unsafe fn context_switch_with_release2(
    prev_rsp_ptr: *mut u64,
    next_rsp: u64,
    lock1: *const AtomicBool,
    lock2: *const AtomicBool,
) -> i64;
```

(c) is simplest and matches the current sole call site. (a) is the most general. (b) is a middle ground that lets the count be runtime but still lets the asm be one shape. The checkpoint conversation picks one.

### 5.2 What does the asm actually do?

Sketch (Intel syntax, conceptual):

```text
context_switch_with_release2:
    ; rdi = prev_rsp_ptr, rsi = next_rsp, rdx = lock1, rcx = lock2
    ; save callee-saved + flags onto current stack
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15
    pushfq

    ; *prev_rsp_ptr = current rsp
    mov [rdi], rsp

    ; switch to new stack
    mov rsp, rsi

    ; release lock1 (Channel mutex) — single store
    mov byte ptr [rdx], 0

    ; release lock2 (scheduler lock) — single store
    mov byte ptr [rcx], 0

    ; pop callee-saved + flags from new stack
    popfq
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp

    ret
```

Open: should the lock releases use `mfence` between them, or is the natural x86 strong ordering sufficient for AtomicBool::Release-equivalent semantics? Likely no fence needed (x86 stores are ordered), but the checkpoint conversation should confirm against the exact `spin::Mutex` implementation we're using.

### 5.3 What if the installer panics?

If the installer closure panics between locking and asm-call, both locks need normal-Drop unwinding. The current sketch in §2.2 calls `mem::forget` on the guards *after* `installer(prev)` — if installer panics, the guards drop normally, locks release normally, no leak. ✅

But what if the panic occurs *after* `mem::forget` but *before* `context_switch_with_release`? Window is one-or-two-machine-instruction wide. If panic-handling runs there, locks are forgotten and never released — system locks up.

Mitigation: the `mem::forget` calls and the asm call happen in a single tight unsafe block with no panicable operations between. Code review verifies this. CI lint could in principle check for this pattern.

### 5.4 Interaction with `cross_process_prep` and `cross_thread_kstack`

These already happen *inside* the SCHED-locked block in HEAD's `block_current_into`. They write CR3 and TSS.rsp0 / per_cpu.kernel_rsp. Under SC 3a:

- They run while both locks are still held (per §2.2's sketch).
- This means CR3 update and TSS.rsp0 update happen on CPU N before the asm release.
- After the asm switch, the new thread's stack is the new thread's kernel stack — which TSS.rsp0 already points to. ✅
- After the asm release, the locks are free. New thread runs.

No issue, as long as the bookkeeping happens before the asm. Sketch in §2.2 reflects this; checkpoint should verify the implementation does.

### 5.5 What does sub-commit 5's blocked-thread kill walk add?

Sub-commit 5 (Q8.c) adds the ability to kill a thread that's parked in a Channel EndpointState. The walk is:

```text
target_proc = PROCESSES.read().get(&pid)
for handle in target_proc.handles:
    if handle is Channel:
        guard = channel.state.lock()
        for endpoint in guard.iter_mut():
            if endpoint is *Blocked.waiter where waiter.pid == pid:
                yank waiter Box, mark Zombie, route to killer's CPU's zombie queue
                return true
        guard drops
```

The lock acquisition order is `PROCESSES.read() → target.handles.lock() → channel.state.lock()`. Sub-commit 3a doesn't change this; sub-commit 5 adds it on top.

Composition: the channel.state mutex acquired by SC 5's killer is the *same* Channel mutex that SC 3a's `block_current_into` holds across its asm. They contend on the same lock; the SC 3a asm release frees it; SC 5's killer proceeds. Bounded latency, no deadlock.

### 5.6 What about the `ZOMBIE_COUNT` atomic?

The reaper short-circuits when `ZOMBIE_COUNT == 0`, avoiding the per-CPU lock acquisitions. This is preserved under SC 2's BSP-scans-all-CPUs design. Under SC 3a:

- `block_current_into` does not produce zombies. It produces *Blocked* threads, which sit in EndpointState, not in any zombie queue.
- ZOMBIE_COUNT is unaffected by `block_current_into`.
- SC 5's blocked-thread-kill, when it yanks a thread from EndpointState and pushes to a zombie queue, does increment ZOMBIE_COUNT. That's SC 5's concern, not SC 3a's.

The atomic's correctness is preserved across the SC 3a transition. ✅

---

## 6. Summary

The SC 3a asm helper composes correctly with sub-commit 2's BSP-scans-all-CPUs reaper. The composition is bounded by:

1. **Release order Channel-first, scheduler-second** (§4 I1) — load-bearing.
2. **Both locks released before new thread runs** (§4 I2) — already in §Q6, reaffirmed.
3. **Fixed-arity release list, no heap allocation** (§4 I3, I4) — perf and correctness.
4. **Lock ownership invariant: every release-list entry is owned-but-forgotten** (§4 I5) — soundness.
5. **Asm release on new stack** (§4 I6) — already in §Q6, the load-bearing primitive.

Open implementation questions for the SC 3a checkpoint conversation:

1. ABI shape of `context_switch_with_release` (const-generic vs fixed-shape vs per-call-site).
2. Whether `mfence` is needed between the release stores.
3. Panic-safety review of the `mem::forget`-then-asm window.
4. Verify the cross_process_prep / cross_thread_kstack ordering preserves the load-bearing TSS.rsp0 invariant.
5. SC 5's blocked-thread-kill walk composition — already analyzed, but the SC 5 dispatch should reference this analysis.

No new bugs surfaced by the composition. ADR-013 §Q6's choice (Option b: ManuallyDrop + asm release) remains the right answer. Sub-commit 2's BSP-scans-all-CPUs reap pattern is invisible to SC 3a's correctness.

---

## 7. Cross-references

- **ADR-013 §Q6** — the design substrate for SC 3a; this note doesn't amend it, just pre-thinks the implementation.
- **ADR-013 §Q4 (wake-on-completer-CPU)** — drives the cross-CPU rendezvous semantics this composes with.
- **ADR-013 §Q9 BUILD-3 amendment (Q9 deferred)** — explains why the reaper is BSP-scans-all-CPUs instead of per-CPU.
- **ADR-011 Invariant 1** — the predecessor consumer-first property §Q4 preserves.
- **`kernel/src/sched/scheduler.rs`** — current `block_current_into`, `reap`, `kill_thread_by_pid` source. SC 2 working tree extends the reap to cross-CPU.
- **`kernel/src/sched/process.rs`** — `on_thread_reap`'s "Two PROCESSES.lock() acquisitions, intentional" comment is load-bearing context for the SC 5 work that layers on top of SC 3a.
- **PM-DECISION-2026-05-06-sub-commit-2-bisect-rollback.md** — the §Q9 deferral decision that established the BSP-scans-all-CPUs reaper shape this analysis composes against.

---

**End of design notes.**

*Becomes input to SC 3a pre-implementation checkpoint conversation. Discard or fold into the checkpoint dispatch when SC 3a is scheduled.*
