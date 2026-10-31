# Dispatch: BUILD-3 sub-commit 2 — restore deferred-quantum mechanism (per-CPU)

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent:** Cerebro's existing uncommitted working tree (sub-commit 2 work-in-progress).
**Substrate:** The Q5 100x closure gate failed at run 24 exactly as ADR-013 §Q5 said it could. PM has decided: restore the deferred-quantum mechanism in per-CPU form. See `Plans/PM-DECISION-2026-05-06-Q5-bet-failed.md` for the full diagnosis and rationale.
**Type:** Code restoration + ADR-013 amendment + commit. The original sub-commit 2 dispatch's §Q5 deletions are reverted; everything else (Q3 per-CPU schedulers, Q9 per-CPU reapers, AP GS-base fix, test scenarios) stays.

---

## 0. Standing instructions

Same as the original sub-commit 2 dispatch.

1. **Commit on `main`, not detached HEAD.** Verify with `git status` and `git branch --show-current`.
2. **Working tree starts where you left it** — uncommitted sub-commit 2 work plus the AP GS-base fix plus the ADR-013 BUILD-3 amendment. Do NOT discard or stash. The restoration is on top of what you have.
3. **Verification gates from the original dispatch §5 still apply.** The 100x `ipc_heap_kill_smoke` gate stays as the closure gate; with the mechanism restored it should pass. If it doesn't, STOP and surface.
4. **Diff warnings against the same baseline (18 warnings).**

---

## 1. The decision

The §Q5 bet was: under per-CPU schedulers + IF=0 discipline + wake-on-completer-CPU, the deferred-quantum mechanism is unnecessary because no PIT tick can fire on the current CPU during the front-queue + yield critical section. **The bet was wrong, but for a subtle reason:** the kernel-side critical section is correctly atomic under `without_interrupts`, but once `block_current_into`'s scope exits and the resumed thread runs in userspace, IF is re-enabled and the next PIT tick is on the BSP's clock, independent of "the resumed thread just got the CPU." The resumed thread can be preempted before reaching its next syscall.

The deferred-quantum mechanism gave the just-resumed thread (and, separately, a receiver completing a parked sender) a quantum-bounded credit of preemption immunity, ensuring it reached its next syscall before the timer could intervene. **This is not optional for the userland-observable semantic that several Phase 1.5b smoke tests depend on.**

`ipc_heap_kill_smoke` is the canonical evidence: it asserts that `sys_kill` on a peer that *should* be `RecvBlocked` returns `EINVAL`. That assertion can only hold if the woken peer has actually reached its next blocking recv. Without the defer credit, run 24 had a PIT tick land between worker waking from `sys_recv(BLOCK)` and worker reaching `sys_recv(MAIN)` — worker was preempted with state `Ready`, and `kill_thread_by_pid` found it on the ready queue.

Restore the mechanism in per-CPU form. Close sub-commit 2 with it intact.

---

## 2. What to restore

The restoration is the inverse of the original dispatch's §4.4 deletion list, with one refinement: every read/write of `preempt_defer_ticks` lives on the per-CPU scheduler's locked inner state (because `current` is now per-CPU), and `consume_preempt_defer` reads `current_scheduler().inner.lock().current.preempt_defer_ticks`.

### 2.1 `kernel/src/sched/thread.rs`

Restore the field on `Thread`:

```rust
pub(super) preempt_defer_ticks: u8,
```

at the same struct position the original had (the field was at line 101 in the pre-deletion file). Restore the three initializers (lines 173, 191, 279 in the original — `Thread::new`, `Thread::new_user`, `Thread::adopt_current`). Each initializes to `0`.

### 2.2 `kernel/src/sched/scheduler.rs`

Restore the constant:

```rust
const HANDOFF_PREEMPT_DEFER_TICKS: u8 = 32;
```

at the same module-constants region near `TIMER_HZ`.

Restore `clear_current_preempt_defer`, `defer_current_preempt_for_handoff`, `consume_preempt_defer`. Per-CPU shape — read/write through `current_scheduler().inner.lock()`:

```rust
pub fn clear_current_preempt_defer() {
    without_interrupts(|| {
        current_scheduler().inner.lock().current.preempt_defer_ticks = 0;
    });
}

pub fn defer_current_preempt_for_handoff() {
    without_interrupts(|| {
        current_scheduler().inner.lock().current.preempt_defer_ticks = HANDOFF_PREEMPT_DEFER_TICKS;
    });
}

fn consume_preempt_defer() -> bool {
    without_interrupts(|| {
        let mut s = current_scheduler().inner.lock();
        if s.current.preempt_defer_ticks == 0 {
            return false;
        }
        s.current.preempt_defer_ticks -= 1;
        true
    })
}
```

(Adjust the exact field path to match whatever your `PerCpuScheduler`/`PerCpuSchedulerInner` shape ended up being. The pattern is: same as the rest of sub-commit 2's per-CPU accesses.)

In `on_tick`, restore the early-return:

```rust
pub fn on_tick() {
    if consume_preempt_defer() {
        return;
    }
    yield_now();
}
```

In `prepare_woken_thread`, restore the credit-clear:

```rust
fn prepare_woken_thread(thread: &mut Thread, return_value: i64) {
    debug_assert!(...);
    thread.state = ThreadState::Ready;
    thread.blocked_return = return_value;
    thread.preempt_defer_ticks = 0;
}
```

In `wake_thread_front`, restore the credit assignment on the woken thread:

```rust
pub fn wake_thread_front(mut thread: Box<Thread>, return_value: i64) {
    prepare_woken_thread(&mut thread, return_value);
    // Direct IPC handoff gets a bounded receiver quantum, cleared on the
    // receiver's next syscall entry. Without this, timer delivery can run
    // the sender before the receiver reaches its follow-on blocking syscall.
    thread.preempt_defer_ticks = HANDOFF_PREEMPT_DEFER_TICKS;
    current_scheduler().inner.lock().ready.push_front(thread);
}
```

(Note: `prepare_woken_thread` zeros the credit, then `wake_thread_front` overwrites it with the handoff value. This ordering matches the original. `wake_thread` — the back-queue path used for `close_peer` and the like — does NOT set the credit; only the front-queue path does.)

In `block_current_into`, restore the line that zeros prev's credit before parking it:

```rust
prev.state = ThreadState::Blocked;
prev.preempt_defer_ticks = 0;
```

(The blocked thread shouldn't carry stale credit when it's later resumed via `wake_thread` or `wake_thread_front`. `wake_thread_front` overwrites with the handoff value anyway, but the explicit zero in `block_current_into` is defensive against any path that wakes the same Box without going through `prepare_woken_thread`.)

### 2.3 `kernel/src/ipc/mod.rs`

Restore the `defer_current_preempt_for_handoff` call in `continue_current_after_receive`:

```rust
fn continue_current_after_receive(thread: Box<Thread>, return_value: i64) {
    // When a receiver completes a previously-blocked sender/grantor, the
    // consumer of the payload is the current thread. Let it reach the next
    // syscall (usually a follow-on block) before the producer observes it as
    // Ready again.
    sched::wake_thread_front(thread, return_value);
    sched::defer_current_preempt_for_handoff();
}
```

This restores the original comment too; the experimental "per-CPU IF=0 discipline closes the stale-tick handoff window" wording in your working tree was the §Q5 claim that turned out to be wrong. Replace it with the original comment.

### 2.4 `kernel/src/arch/x86_64/syscall.rs`

Restore the `clear_current_preempt_defer()` call at the syscall entry path. There was exactly one call site in the pre-deletion file. Re-add it at the same point. (If you took a clean diff before deleting, you have its location; otherwise scan for whatever syscall-entry function the original called from — likely `syscall_dispatch` or its prologue.)

The semantic this gives back: every syscall entry zeros the current thread's defer credit. This bounds the protection to one quantum's worth, ensuring a thread can't accumulate defer credit across multiple syscalls and starve other ready threads.

---

## 3. ADR-013 §Q5 amendment

ADR-013 §Q5 currently specifies the deletion. It needs an amendment subsection acknowledging the failed bet. Append to the end of §Q5 (or as the last subsection within ADR-013 if the document structure prefers) a section like the following. Adapt the wording to ADR-013's voice, but the substance is non-negotiable:

```markdown
### Q5 amendment: the bet failed; the mechanism stays

Sub-commit 2's 100x `ipc_heap_kill_smoke` closure gate failed at run 24
under `NEO_SMP=4`. The `[ neo-init ] BUG: sys_kill on Blocked did NOT
return EINVAL` line appeared exactly when this section said it might.

The §Q5 argument relied on a claim that does not hold:

> the front-queue + yield window is closed by construction — no PIT tick
> can fire on this CPU while IF=0.

This is true for the kernel-side critical section. It is not true for the
end-to-end semantic the deferred-quantum mechanism actually protected:
"after `sys_send` to a `RecvBlocked` peer returns, the peer has reached
its next blocking syscall." Once `block_current_into`'s `without_interrupts`
scope exits and the resumed thread runs in userspace, IF is re-enabled and
the next PIT tick is on the CPU's hardware clock, independent of when
the thread became current. Under QEMU SMP, the host's scheduling jitter
makes the inter-tick gap small enough that the resumed thread can be
preempted before reaching its next syscall.

**Resolution:** the deferred-quantum mechanism is restored in per-CPU
form. `Thread.preempt_defer_ticks`, `HANDOFF_PREEMPT_DEFER_TICKS`,
`consume_preempt_defer`, `clear_current_preempt_defer`,
`defer_current_preempt_for_handoff` all return. Reads and writes of
`preempt_defer_ticks` go through the per-CPU scheduler's locked inner
state, the same way sub-commit 2 routes the rest of `current`/`ready`/
`zombies` access.

Sub-commit 2's actual scope is therefore: §Q3 (per-CPU schedulers), §Q9
(per-CPU reapers + `live_thread_count`), the AP GS-base rewire amendment,
and the two new test scenarios. The §Q5 deletion is reverted and will
not be retried — the mechanism is small, correct, and load-bearing for
several Phase 1.5b smoke-test semantics.

The §Q5 idea was wrong-shaped: it bundled a cleanup attempt with the
per-CPU scheduler refactor. Future cleanup proposals must cleanly
separate "is this mechanism still needed?" from "are we changing the
container shape?" The container shape changed correctly; the mechanism
inside the container should not have been touched in the same commit.
```

This amendment is part of sub-commit 2's commit. Don't make it a separate commit.

---

## 4. Verification gates

Same as the original dispatch §5. With the mechanism restored:

### 4.1 Build green

```
docker compose run --rm neo cargo k
```

Expected: green build, **18 warnings** (same as the failed-attempt working tree had — adding back fields/functions doesn't add new warnings; if anything, it may retire one if any was added by `dead_code` on unused state).

### 4.2 SMP=1 default suite

Same as before: 25/25 PASS + meta-FAIL.

### 4.3 SMP=4 default suite

Same as before: 25/25 PASS + meta-FAIL.

### 4.4 The 100x gate (the actual closure gate)

Run `ipc_heap_kill_smoke` 100x consecutively under `NEO_SMP=4`. Stop on first non-PASS. **100/100 PASS is the closure gate.** With the mechanism restored, this should pass.

If it doesn't pass — e.g., the per-CPU form of the mechanism has a subtle break (most likely candidate: `consume_preempt_defer` reads the wrong CPU's current somehow) — STOP and surface to PM. Don't try to fix it without surfacing first.

### 4.5 Frame leak audit clean

Same invariant as before. Every boot must end with `frame: leak audit clean (baseline=N final=N)` matching.

---

## 5. Commit shape

Single commit on `main`. Suggested message:

```
build-3 sub-commit 2: per-CPU schedulers + Q9 per-CPU reapers + AP GS-base fix

Per ADR-013 §Q3 (per-CPU schedulers), §Q9 (per-CPU reapers).

- Replace `static SCHED: Lazy<Mutex<Scheduler>>` with `PerCpuScheduler`
  per CPU, accessed via `PER_CPU[cpu_id].scheduler_ptr`. Each CPU has
  its own current/ready/zombies queues + scheduler lock. No global
  scheduler.
- APs run their bootstrap thread (`ap-N-main`) in a per-CPU reaper +
  yield loop via `sched::run_ap_bootstrap`. AP LAPIC timer remains
  masked in sub-commit 2; APs idle harmlessly because cross-CPU IPC
  still returns EAGAIN.
- AP GS-base rewire after GDT load (ADR-013 BUILD-3 amendment): APs
  must rewire IA32_GS_BASE after gdt::init_ap clears it via segment
  reload. init_traps_ap order is now: gdt::init_ap → per_cpu rewire →
  IDT load → syscall MSRs → sentinel check.
- `Process` gains `live_thread_count: AtomicU32`. `on_thread_reap`
  decrements; only proceeds with deregister if post-decrement is 0
  with a double-check under the registry lock.
- Two new scenarios: `smp_per_cpu_scheduler_smoke`, `smp_same_cpu_ipc`.

§Q5 deletion REVERTED — see ADR-013 §Q5 amendment. The deferred-quantum
mechanism (HANDOFF_PREEMPT_DEFER_TICKS, Thread.preempt_defer_ticks,
consume_preempt_defer, clear_current_preempt_defer,
defer_current_preempt_for_handoff) stays in per-CPU form. The §Q5
deletion bet failed at run 24 of the 100x ipc_heap_kill_smoke closure
gate; the bet's argument confused kernel-side IF=0 atomicity with
end-to-end "receiver reaches next syscall before any other thread runs"
atomicity. The mechanism is load-bearing for the latter and will not
be retried.

Verified: 100/100 PASS on ipc_heap_kill_smoke under NEO_SMP=4.
Default suite: 25/25 PASS + meta-FAIL on both NEO_SMP=1 and NEO_SMP=4.
Build: 18 warnings unchanged from baseline.
```

Adjust the verification numbers to match what your final run actually produces.

---

## 6. If the 100x gate still fails after restoration

Genuinely unexpected. Possible causes:

- The per-CPU form of `consume_preempt_defer` is reading the wrong CPU's `current`. Verify by inspecting `current_scheduler()` on the path called from `on_tick`. The timer ISR fires on the CPU that owns it; `on_tick` should read that CPU's per-CPU scheduler.
- The credit isn't getting set. Verify `wake_thread_front`'s assignment lands before push_front, and the path from `Channel::send` peer=`RecvBlocked` arm reaches `wake_thread_front` via `handoff_to_rendezvous_peer`.
- Some other §Q5 path was missed. Search for any remaining references to the function names you restored — there should be exactly the call sites listed in §2.

Stop and surface to PM. Don't try heroics.

---

## 7. The bigger lesson (for future cleanup proposals)

The §Q5 deletion was bundled with the per-CPU scheduler refactor because they live in the same files and felt related. They aren't. "Per-CPU scheduler containers" is a structural refactor. "Is the deferred-quantum mechanism still needed?" is a semantic question about kernel/userland atomicity invariants. The semantic question deserved its own ADR / its own commit / its own gate.

ADR-013 §Q5 amendment captures this. Future cleanup proposals against any kernel atomicity mechanism follow the same discipline: separate the structural change from the semantic question, gate them independently, and never bundle "delete a mechanism we think is now redundant" with a refactor of the same files.

---

**End of dispatch.**
