# WEDGE-INV-J1 audit: CPU0 scheduler-lock paths

**Date:** 2026-05-08  
**Substrate:** `3b3b4db xtask: optional QEMU monitor for investigation use`  
**Dispatch:** `Plans/DISPATCH-2026-05-08-wedge-investigation-J1-audit-cerebro.md`  
**Type:** Static audit only. No builds, no tests, no commits, no stash apply.

## Summary

**Verdict A - bug identified.**

The original "timer ISR interrupts process context while that same context holds `PerCpuScheduler.inner`" variant is not the direct shape: every direct scheduler-lock holder audited either wraps the acquire in `without_interrupts(...)` or is only called under that documented contract.

The actual static match is a nearby re-entrance/lock-order bug:

1. `scheduler::reap()` drains zombies with `mem::take(&mut s.zombies)` while holding CPU0 `PerCpuScheduler.inner` (`kernel/src/sched/scheduler.rs:541-546`).
2. `mem::take` replaces the scheduler's zombie `VecDeque` with a default empty queue. That default queue has zero capacity.
3. The old drained queue is later dropped outside the scheduler lock and outside a surrounding `without_interrupts` region. Dropping that queue can take the global heap allocator lock, which is `linked_list_allocator::LockedHeap`, backed by `spinning_top::Spinlock`.
4. If the timer fires while the BSP is in that heap deallocation, the timer path can switch to another ready `smp-hog-*` worker.
5. That worker reaches `sched::exit()`, takes CPU0 `PerCpuScheduler.inner`, then calls `s.zombies.push_back(prev)` in `rotate(..., WhichQueue::Zombie)` (`kernel/src/sched/scheduler.rs:241-247`).
6. Because `reap()` left `s.zombies` with zero capacity, that `push_back` allocates while CPU0 `PerCpuScheduler.inner` is held.
7. The allocation attempts to acquire the same global heap lock held by the interrupted reaper. CPU0 spins in `spinning_top::RawSpinlock::lock` with CPU0 scheduler inner held forever.

This matches WEDGE-INV-8's convergence better than "CPU0 is spinning on the scheduler lock" itself:

- Source code defines `PerCpuScheduler.inner` as `spin::Mutex<PerCpuSchedulerInner>` (`kernel/src/sched/scheduler.rs:35`, `kernel/src/sched/scheduler.rs:60-66`), not `spinning_top::Spinlock`.
- `spinning_top` enters through `linked_list_allocator` (`Cargo.lock:56-63`), and the kernel global allocator is `LockedHeap` (`kernel/src/mm/heap.rs:32-33`).
- WEDGE-INV-8 resolved the stuck RIP to `spinning_top::spinlock::RawSpinlock::lock`, which is the heap lock path, while its NMI snapshot also observed CPU0 scheduler inner as locked. Those two facts are consistent if CPU0 holds scheduler inner and is spinning on the heap allocator lock.
- The wedge occurs after partial reaping of `smp-hog-*` workers, exactly when `reap()` has had an opportunity to replace the zombie queue with a zero-capacity queue and later drop the drained buffer.

## Lock Type Confirmation

`PerCpuScheduler.inner` is:

```rust
use spin::Mutex;

pub struct PerCpuScheduler {
    pub cpu_id: u32,
    inner: Mutex<PerCpuSchedulerInner>,
    zombie_count: AtomicUsize,
}
```

Source: `kernel/src/sched/scheduler.rs:35`, `kernel/src/sched/scheduler.rs:60-66`.

This is a plain `spin::Mutex`, not an IRQ-safe wrapper. It does not disable interrupts by itself. Callers currently provide IRQ masking through `crate::arch::x86_64::without_interrupts`, which re-exports `x86_64::instructions::interrupts::without_interrupts` (`kernel/src/arch/x86_64/mod.rs:293-302`).

The observed WEDGE-INV-8 RIP, however, was in `spinning_top::RawSpinlock::lock`. That is not the source-level type of `PerCpuScheduler.inner`. The source-level `spinning_top` path is the heap allocator:

- `linked_list_allocator` depends on `spinning_top` (`Cargo.lock:56-63`).
- `kernel/src/mm/heap.rs:32-33` declares `static ALLOCATOR: LockedHeap`.
- `linked_list_allocator::LockedHeap` is `Spinlock<Heap>`, and its `GlobalAlloc::{alloc,dealloc}` call `.lock()` (`linked_list_allocator-0.10.6/src/lib.rs:285-343`).

Conclusion: WEDGE-INV-8's RIP is best read as "CPU0 is spinning on the heap allocator lock while CPU0 scheduler inner is held", not "CPU0 is spinning inside the scheduler lock primitive."

## Section 3.4 Result

Timer path:

- IDT vector 32 is installed as an interrupt gate and clears IF on entry (`kernel/src/arch/x86_64/idt.rs:59-66`).
- `timer_isr_entry` documents `IF=0` at entry (`kernel/src/arch/x86_64/timer.rs:55-59`).
- `timer_rust_handler()` calls `pic::end_of_interrupt(0)` and then `sched::on_tick()` (`kernel/src/arch/x86_64/timer.rs:181-189`).
- `sched::on_tick()` calls `consume_preempt_defer()` and then `yield_now()` (`kernel/src/sched/scheduler.rs:306-310`).
- `consume_preempt_defer()` locks `current_scheduler().inner` (`kernel/src/sched/scheduler.rs:333-342`).
- `yield_now()` calls `rotate(...)`, which locks `current_scheduler().inner` (`kernel/src/sched/scheduler.rs:290-299`, `kernel/src/sched/scheduler.rs:217-220`).

So yes, the timer ISR takes `PerCpuScheduler.inner` transitively.

Direct process-context scheduler-lock acquirers audited:

- `spawn`, `push_ready`, `yield_now`, `exit`, `current_pid`, `block_current_into`, `kill_thread_by_pid`, and `reap` all wrap the lock in `without_interrupts(...)`.
- `wake_thread` and `wake_thread_front` do not wrap internally, but all discovered callers are under syscall/channel paths that wrap with `without_interrupts`, or `close_peer` wraps explicitly (`kernel/src/ipc/mod.rs:419-445`, `kernel/src/arch/x86_64/syscall.rs:579-599`, `kernel/src/arch/x86_64/syscall.rs:731-741`).

APIC/IPI paths:

- No TLB-shootdown, reschedule IPI, or APIC IPI handler currently exists in `kernel/src/arch/x86_64` on this substrate. SMP bring-up starts APs through Limine and APs enter `run_ap_bootstrap`; there is no IPI vector that takes the scheduler lock (`kernel/src/arch/x86_64/smp.rs:20-113`).

Exception paths:

- Standard exception handlers in `idt.rs` print and halt, or route test page faults through `test_harness`; they do not call scheduler APIs (`kernel/src/arch/x86_64/idt.rs:90-213`).

Conclusion: simple timer-vs-process re-entry on the same `PerCpuScheduler.inner` lock is refuted by the current call-site discipline. The live bug is timer-mediated heap-lock re-entry combined with scheduler-held heap allocation.

## Per-Call-Site Table

| Site | Context | IRQ state while scheduler lock held | Held-region summary | Blocking / allocation risk | Re-entry / lock-order risk |
|---|---|---|---|---|---|
| `spawn` (`scheduler.rs:179-185`) | Process/test/BSP or AP current CPU | Disabled by `without_interrupts` | Push new `Thread` into current ready queue | `VecDeque::push_back` can allocate if ready capacity grows | Takes scheduler lock then may enter heap allocator |
| `push_ready` (`scheduler.rs:192-197`) | User-process spawn path | Disabled by `without_interrupts`; caller also runs in `spawn_user` `without_interrupts` | Push prebuilt user thread into current ready queue | `VecDeque::push_back` can allocate | Scheduler-then-heap if queue grows |
| `rotate` (`scheduler.rs:217-249`) via `yield_now` | Process context or timer ISR | Caller has IF disabled; timer ISR entered with IF=0 | Pop next ready, replace current, push previous to ready | Ready `push_back` can allocate if capacity grows | Timer-context scheduler-lock holder; can become scheduler-then-heap |
| `rotate` (`scheduler.rs:217-249`) via `exit` | Process context | Disabled by `exit`'s `without_interrupts` | Pop next ready, move previous to zombies | `s.zombies.push_back(prev)` can allocate; high-risk after `reap()` zeroes capacity | Primary bug site: scheduler lock held, then heap lock attempt |
| `clear_current_preempt_defer` (`scheduler.rs:313-320`) | Syscall entry | Disabled by `without_interrupts`; syscall entry also masks IF via SFMASK | Mutate current thread field | No allocation | No same-lock re-entry observed |
| `defer_current_preempt_for_handoff` (`scheduler.rs:323-330`) | IPC rendezvous | Disabled by `without_interrupts` | Mutate current thread field | No allocation | No same-lock re-entry observed |
| `consume_preempt_defer` (`scheduler.rs:333-342`) | Timer ISR | IF already 0; also wraps defensively | Decrement small field | No allocation | Timer takes scheduler lock, but only short field mutation |
| `current_pid` (`scheduler.rs:371-373`) | Syscall/IPC/process helpers | Disabled by `without_interrupts` | Read current pid | No allocation | No same-lock re-entry observed |
| `block_current_into` (`scheduler.rs:407-445`) | IPC block path | Disabled by `without_interrupts` | Pop next, mark prev Blocked, call installer while scheduler lock held, then switch | Installer takes channel lock; ready pop does not allocate | Known scheduler/channel lock-order inversion documented in comments; not exercised by `smp_per_cpu_scheduler_smoke` |
| `wake_thread` (`scheduler.rs:457-460`) | IPC wake / close_peer | Caller contract: already in `without_interrupts`; `close_peer` wraps explicitly | Set return value, push to ready back | Ready `push_back` can allocate if capacity grows | Scheduler-then-heap if ready queue grows |
| `wake_thread_front` (`scheduler.rs:468-471`) | IPC handoff | Caller contract: syscall/channel path under `without_interrupts` | Set return value/defer, push to ready front | Ready `push_front` can allocate if capacity grows | Scheduler-then-heap if ready queue grows |
| `kill_thread_by_pid` (`scheduler.rs:498-514`) | `sys_kill` | Disabled by `without_interrupts` | Remove ready thread, push to zombies | Zombie `push_back` can allocate after `reap()` zeroes capacity | Same allocation-under-scheduler-lock risk as `exit`; not the smp smoke path |
| `reap` (`scheduler.rs:523-560`) | BSP only | Disabled only for the lock/drain block | If `zombie_count>0`, lock per-CPU scheduler, `mem::take` zombies, decrement count; then process drained zombies outside lock | `mem::take` leaves scheduler zombie queue zero-capacity; drained queue drops outside IRQ mask | Primary enabling site for later scheduler-held heap allocation |

## Reaper Analysis

`reap()` is BSP-only:

```rust
if per_cpu::current_cpu_id() != 0 {
    return;
}
```

Source: `kernel/src/sched/scheduler.rs:523-526`.

Traversal order is `PER_CPU.iter()` from index 0 upward (`kernel/src/sched/scheduler.rs:528`). For each initialized scheduler, it checks the lock-free `zombie_count` first (`kernel/src/sched/scheduler.rs:537-539`). If zero, it skips the lock. If nonzero, it runs:

```rust
let drained: VecDeque<Box<Thread>> = without_interrupts(|| {
    let mut s = sched.inner.lock();
    let taken = mem::take(&mut s.zombies);
    sched.zombie_count.fetch_sub(taken.len(), Ordering::Release);
    taken
});
```

Source: `kernel/src/sched/scheduler.rs:541-546`.

The lock-hold region itself is short and does not explicitly call serial, `Thread::free`, or `process::on_thread_reap`. That part is good.

The bug is the side effect of `mem::take`: it installs `VecDeque::default()` as the live scheduler zombie queue. The initial queue was created with capacity 4 (`ZOMBIE_INITIAL_CAPACITY`, `kernel/src/sched/scheduler.rs:50-51`, `kernel/src/sched/scheduler.rs:135-139`). After the first nonempty reap, the live zombie queue has no retained capacity. The next thread exit can therefore allocate while holding scheduler inner.

Processing of each drained zombie happens outside the scheduler lock:

- `serial_println!` logs the reap (`kernel/src/sched/scheduler.rs:547-553`).
- `Thread::free(zombie)` runs (`kernel/src/sched/scheduler.rs:554`).
- `on_thread_reap(pid)` runs (`kernel/src/sched/scheduler.rs:557`).

`Thread::free` wraps its slab/stack work in `without_interrupts` (`kernel/src/sched/thread.rs:126-135`), so timer preemption should not interrupt that specific heap free. But the drained `VecDeque` buffer itself is dropped when `drained` goes out of scope, outside the local `without_interrupts` block. That drop can acquire the heap allocator lock with IF enabled.

That creates the re-entry window: heap lock held by reaper's drop path, timer fires, timer switches to a worker, worker exits, worker tries to allocate a replacement zombie queue buffer while holding CPU0 scheduler inner.

## Worker Exit-Path Analysis

`smp_per_cpu_scheduler_smoke` spawns all four workers from the BSP:

```rust
crate::sched::spawn("smp-hog-0", smp_scheduler_worker);
crate::sched::spawn("smp-hog-1", smp_scheduler_worker);
crate::sched::spawn("smp-hog-2", smp_scheduler_worker);
crate::sched::spawn("smp-hog-3", smp_scheduler_worker);
```

Source: `kernel/src/test_harness.rs:1281-1289`.

`spawn()` enqueues on `current_scheduler()` (`kernel/src/sched/scheduler.rs:179-184`), so the workers are CPU0 scheduler-local in the current scenario. H1 observed the same: all active worker zombie observations were `cpu=0`.

Each worker loops through cooperative yields, increments `SMP_SCHED_DONE`, and calls `crate::sched::exit()` (`kernel/src/test_harness.rs:1273-1279`).

`exit()` calls `rotate(ThreadState::Zombie, WhichQueue::Zombie)` under `without_interrupts` (`kernel/src/sched/scheduler.rs:354-364`). The zombie registration occurs here:

```rust
WhichQueue::Zombie => {
    s.zombies.push_back(prev);
    sched.zombie_count.fetch_add(1, Ordering::Release);
}
```

Source: `kernel/src/sched/scheduler.rs:244-247`.

Because Q9 rollback leaves APs in bootstrap loops and does not route work to AP scheduler queues (`kernel/src/sched/scheduler.rs:100-109`), the current smoke does not require cross-CPU acquisition of CPU0's scheduler lock. The exit path registers zombies on CPU0 because the workers are CPU0-local.

The high-risk condition is not cross-CPU ownership. It is CPU0 running a different worker while the interrupted reaper still owns the heap lock.

## Mechanism-Family Ranking

1. **Heap-lock re-entry while CPU0 scheduler inner is held - highest confidence.** Explains `spinning_top` RIP, CPU0 scheduler lock held, partial reaping, AP queues empty, and `zombie_count` observations. Concrete sites: `reap()` zero-capacity `mem::take` plus `exit()` zombie `push_back`.
2. **Generic scheduler-held heap allocation - high confidence as a broader bug class.** `ready` and `zombies` are `VecDeque`s. Any push under scheduler inner can allocate if capacity is exhausted. The current reproducible path is zombie capacity loss after `reap()`.
3. **Simple same-lock timer re-entry on `PerCpuScheduler.inner` - low after audit.** Timer does take the lock, but audited process-context holders mask interrupts while holding it.
4. **Cross-CPU lock-while-blocked - low for current smoke.** APs are initialized and loop in `run_ap_bootstrap`, but the scenario workers are spawned on CPU0 and no AP routing primitive is present.
5. **Atomicity/corruption of scheduler spinlock - low.** No static evidence. WEDGE-INV-8's `spinning_top` address points away from the scheduler lock implementation and toward the heap allocator.

## Candidate Sites In Priority Order

1. `kernel/src/sched/scheduler.rs:541-546` plus `kernel/src/sched/scheduler.rs:244-247`.
   - Family: timer-mediated heap-lock re-entry / scheduler-held heap allocation.
   - Evidence: `mem::take` zeroes zombie capacity; next `exit` allocates under scheduler inner; WEDGE RIP is `spinning_top` heap lock; WEDGE phase is partial reap.
   - Falsification: instrument queue capacity and allocator lock pointer. If wedged runs show `s.zombies.capacity() > s.zombies.len()` before every zombie push and RDI is not the heap lock, this candidate weakens.

2. `kernel/src/sched/scheduler.rs:241-247` and `kernel/src/sched/scheduler.rs:457-471`.
   - Family: any scheduler-held `VecDeque` growth.
   - Evidence: `VecDeque` can allocate on push; scheduler lock is held during push.
   - Falsification: prove all ready/zombie queues retain sufficient capacity at every push in all tested paths, or replace with no-alloc queues and observe wedge persists.

3. `kernel/src/sched/scheduler.rs:407-445`.
   - Family: documented scheduler/channel AB-BA inversion in `block_current_into`.
   - Evidence: comment already names the inversion; scheduler lock can be held while installer takes `Channel.state`.
   - Falsification: not exercised by `smp_per_cpu_scheduler_smoke`; a wedge during kernel-thread worker exit does not need IPC.

4. `kernel/src/sched/scheduler.rs:306-310` / `kernel/src/arch/x86_64/timer.rs:181-189`.
   - Family: timer ISR scheduler entry.
   - Evidence: timer takes scheduler lock and can context-switch away from an interrupted heap-lock holder.
   - Falsification: disable timer preemption during allocator critical sections or make allocator IRQ-safe; if wedge disappears, this becomes supporting evidence for candidate 1.

## Proposed Fix

Do not commit from this note; fix dispatch should decide final shape. The fix should target the invariant, not the symptom:

**No heap allocation or deallocation may occur while `PerCpuScheduler.inner` is held, and no timer path may re-enter the heap allocator while the interrupted context holds the heap lock.**

Minimal fix family:

1. Replace `reap()`'s `mem::take(&mut s.zombies)` with a capacity-preserving drain/swap strategy so the live scheduler zombie queue never falls back to zero capacity.
2. Ensure the replacement queue is allocated before taking `PerCpuScheduler.inner`, or use a fixed/preallocated queue representation.
3. Add a debug assertion before `s.zombies.push_back(prev)` and scheduler ready pushes that the queue has spare capacity, so future scheduler-held allocation attempts fail loudly during tests.
4. Consider dropping the drained queue buffer with interrupts disabled, or avoid dropping a heap-backed drained queue on the reaper path at all. This closes the timer interrupt window while the global heap lock is held.

Architectural follow-up:

- Introduce an IRQ-safe heap lock or allocator wrapper. The global allocator currently uses `spinning_top` through `linked_list_allocator`; that lock is not IRQ-aware. This is broader than the scheduler bug, but the current wedge demonstrates the class is real.
- Longer term, replace scheduler-owned `VecDeque` growth with bounded/no-alloc queues or pre-reserved queue storage. Scheduler critical sections should not depend on the general heap.

## Final Notes

The J1 dispatch expected the audit to decide whether `PerCpuScheduler.inner` itself was the contended `spinning_top` lock. The source code says it is not. That mismatch is the useful finding: WEDGE-INV-8 saw `spinning_top` because the BSP was most likely in the global heap allocator, while the NMI snapshot also saw CPU0 scheduler inner held because the allocator call happened inside the scheduler critical section.

This is enough to proceed to a fix dispatch without J2. J2's saved-GPR lock-pointer capture would still be useful as confirmation, but not necessary to justify the fix: removing heap allocation/deallocation from scheduler critical sections is independently correct and directly addresses the observed wedge geometry.

