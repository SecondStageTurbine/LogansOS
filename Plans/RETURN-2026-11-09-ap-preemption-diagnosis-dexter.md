<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 381: a preempted user thread starves beside non-yielding spinners (Dexter)

**Dispatch:** `Plans/DISPATCH-2026-11-09-relay-381-ap-preemption-diagnosis.md`
**Branch:** `dexter/ap-preemption-diagnosis`
**Seat:** Dexter (DeepSeek family, diagnosis). Evidence under test came from Seat B / PR #98
(`claude/l1a3-terminal`, head `09e62bcb1` at run `42695916915`).

## Verdict in one paragraph

Seat B's reading — “the tick is BSP-only, so a thread queued behind a spinner on an AP has nothing to
preempt that spinner” — names a **real latent gap**, but it is **not the mechanism in this trace**. In
this kernel every user thread is created on, and remains on, the BSP: `spawn_user` pushes the first
thread onto `current_scheduler()` and every wake path targets either the parked CPU or the waker's CPU;
`place_thread_on_cpu` is test-only, and no production path moves a user thread to an AP. The terminal
cannot be “behind a spinner on an AP” today. The stall is on the BSP: the tick still arrives, but the
ring-3 spinner is allowed to stay `current` because `on_tick` honors `PREEMPT_DISABLE_DEPTH` and the
IPC handoff credit (`preempt_defer_ticks`) *before* it rotates. The smallest correct fix is to make a
tick that interrupted **ring 3** bypass those two kernel-context gates: a ring-3 thread holds no kernel
critical section, so it must always be preemptible on a CPU that has a tick. The AP tick remains a
separate, real exposure to fix before any user thread is ever placed on an AP.

## 0. What the evidence actually is

Seat B's excerpt says the `draw` and `commit` lines are adjacent statements “with no syscall between
them.” That is not quite true, and the correction matters: `log_pair` is a `sys_write` to fd 1
(`userland/neo-worker/src/terminal_app.rs:486`, helper at `:651-659`; on `09e62bcb1`). The gap is
therefore between two `sys_write` calls: the terminal returned from the `draw` trace's write, was
descheduled in userland or on the write's return path, and only executed the `commit` trace's write
about 20 s later. It was **runnable**, not blocked in a wait — so this is a dispatch/fairness question,
not a lost-wakeup question.

The earlier run (`42670742054`, the parked `SYS_WAIT_ANY` design) also showed the terminal's 20 ms
deadline **firing** (it woke, read 12 bytes, polled focus — 20–40 ms slices). That is useful: the tick
source was alive at least during the launch. The stall is what happens after the draw, when the two
spinners stop blocking and keep the BSP.

## 1. Hypothesis — exact mechanism, with file:line

**H1 (primary, this trace). The BSP's tick fires, but `on_tick` refuses to rotate a ring-3 spinner
because of a kernel-context gate that a ring-3 thread should not be subject to; the terminal is left
Ready at the back of the BSP ready ring until a spinner leaves the CPU.**

Chain:

1. All user threads are BSP-resident.
   - `process::spawn_user` → `spawn_user_body!` pushes the first thread with
     `scheduler::try_push_ready(thread)` (`kernel/src/sched/process.rs:4613`).
   - `try_push_ready` uses `current_scheduler()` (`kernel/src/sched/scheduler.rs:661-680`), and
     `push_ready_locked` stamps that CPU (`:760-766`).
   - The only cross-CPU enqueue is `wake_thread_front`/`wake_thread_on_current_cpu`: it targets the
     **waker's** CPU (`:2344-2398`, `dst = current_scheduler()` at `:2356`), and the fallback targets the
     parked CPU. `place_thread_on_cpu` is test-only (`:2565+`; called only from `test_harness.rs`).
   - The design states the same invariant: “User threads run on the BSP today (`place_thread_on_cpu`
     is test-only)” — `docs/THINK-2026-10-19-ot22-readiness-multiplex.md:92`.
   - Therefore the terminal is on CPU 0; the two spinners are on CPU 0 too. The AP has no role in this
     trace.

2. The BSP tick runs and calls `sched::on_tick`.
   - `timer_rust_handler(interrupted_cs)` (`kernel/src/arch/x86_64/timer.rs:345`) does the EOI and
     expiry scans, then `sched::on_tick()` (`:415`).
   - `on_tick` first checks `PREEMPT_DISABLE_DEPTH[cpu] != 0` and returns (`scheduler.rs:1453`); then
     `consume_preempt_defer()` and returns (`:1456`, `:1763-1772`); only then
     `yield_now_recording(YieldKind::TimerPreemption)` (`:1458`).

3. When `on_tick` does rotate, a preempted thread is pushed to the **back** of the ring:
   `rotate_compute` → `push_ready_locked_or_panic(..., ReadyPlacement::Back, "rotate")`
   (`scheduler.rs:951-955`). The terminal's own per-row yield does the same
   (`userland/neo-terminal/src/render.rs:153`). So a terminal that yields at the end of a draw is at
   the back; it needs a subsequent tick to move forward.

4. The gates in step 2 are meant for **kernel** critical sections and IPC handoff. For a ring-3
   spinner they are not merely unnecessary — they can be permanent:
   - `PREEMPT_DISABLE_DEPTH` is only written in production by `spawn_user_in`:
     `let _preemption_guard = scheduler::disable_preemption()` (`process.rs:4643`), held across
     `shootdown_quiesce.resume()` (`:4644`), which **enables interrupts** while it spins on
     `SHOOTDOWN_LOCK` (`kernel/src/mm/tlb_shootdown.rs:39-50`, lock at `:267-288`). Any scheduler entry
     reached in that window is deliberately not allowed to switch (`on_tick` gate above).
   - `preempt_defer_ticks` is set to 32 by every front handoff
     (`scheduler.rs:2309`, `:2346`; `HANDOFF_PREEMPT_DEFER_TICKS` at `:68`) and is only cleared at
     syscall entry (`enter_syscall`, `:1570`). A user spinner that receives a handoff and then runs
     without entering a syscall gets up to 320 ms of un-preemptible user time; a sequence of such
     handoffs re-arms it. The terminal, which scans/yields rather than blocking, carries 0.
   - Neither gate can distinguish “kernel critical section” from “ring-3 code.” But the timer entry
     already **knows** the interrupted ring: `interrupted_cs & 3 == 3` is the ring-3 test used in the
     same handler for force-kill/retire (`timer.rs:408-414`). That fact is not passed to `on_tick`.

5. The terminal resumes when a spinner stops being `current`: `sched::exit` calls
   `rotate_compute(..., WhichQueue::Zombie)` and does **not** consult either preemption gate
   (`scheduler.rs:1797-1810`, `:883-960`). That is exactly the observed “resumed the moment Arthur
   exited.” A spinner that blocks/yields also lets the terminal run; that is why the early 20–40 ms
   slices exist, and why the probe app (short bursts, ends in a yield) never reproduces the 20 s gap.

**H2 (Seat B's reading — real gap, wrong trace). APs have no preemption tick.**
- The only `lapic_timer::start(TIMER_HZ)` call is in `init_bsp` (`scheduler.rs:381`); the AP path is
  `ap_entry` → `sched::run_ap_bootstrap` (`smp.rs:358`) → `init_cpu_scheduler` + a
  `yield_now`/`spin_loop` loop (`scheduler.rs:485-517`). No AP arms a timer.
- The tick is PIC IRQ0 vector 32, delivered to the BSP only (“every AP reads 0 … PIC IRQ0 … delivered
  to the BSP only”, `timer.rs:354-359`; `wake_trace.rs:36-40`).
- `reschedule_ipi_body` only EOIs (`idt.rs:1160-1185`), so it cannot force a busy AP to reschedule
  (`docs/concurrency-pitfalls.md` CP-15/CP-21).
- This is a genuine CP-15/CP-21/CP-25 exposure, and it must be fixed **before** any user thread is ever
  placed on an AP. It does not explain this trace, because the trace's thread is not on an AP.

## 2. Falsification test (named, with outcomes)

Add a diagnostic-only scenario (or extend `desktop_terminal_shell_boot` behind a test feature) that
prints, at the stall and once a second thereafter:

- per CPU: `TICKS_SEEN` (already `wake_trace::note_tick`, `timer.rs:360-361`), `ROTATE`, `READY_LEN`,
  `CURRENT_TID`, `CURRENT_IS_USER` (all already in `wake_trace`; `READY_LEN`/`CURRENT_IS_USER` are
  exactly the CP-21 discriminator, `wake_trace.rs:100-112`);
- on the BSP: `PREEMPT_DISABLE_DEPTH[0]` and `current.preempt_defer_ticks`;
- the terminal's tid and its `state`.

Run it on `NEO_SMP=4` with two ring-3 spinner processes and one compute process that prints per-ms
progress, and again on the terminal scenario.

**Outcomes and what each proves:**

- `TICKS_SEEN[0]` advances, `ROTATE[0]` is frozen or far below `TICKS_SEEN[0]`, `READY_LEN[0] > 0`,
  and either `PREEMPT_DISABLE_DEPTH[0] != 0` or `preempt_defer_ticks != 0` for the current spinner:
  **H1 confirmed** — the tick is alive and `on_tick`'s gates are the starvation. This is the expected
  result if the BSP-only reading is wrong.
- `TICKS_SEEN[0]` is frozen while `TICKS_SEEN[1..3]` are also 0: the tick source itself has stopped
  (PIC/PIT/LAPIC), not a scheduling gate. H1's fix will not help; the timer path is the bug.
- `CURRENT_IS_USER[1..3]` is nonzero during the stall: user threads *are* on APs (contrary to the
  current code reading). Then H2 is live and the AP tick fix is required as well.
- With the H1 fix applied, the same scenario shows the compute thread's worst gap ≤ 1 tick (10 ms);
  with the fix reverted (control), the gap reproduces. A fix that only improves the terminal's
  wall-clock but leaves a spinner un-preempted is not accepted.

The negative control is the current `on_tick` (gates honored for ring 3). The positive build is the
fix below. Both must use the same tree and the same `NEO_SMP=4`.

## 3. Smallest correct fix

**Change `timer_rust_handler` to pass the interrupted ring to `on_tick`, and have `on_tick` skip
`PREEMPT_DISABLE_DEPTH` / `consume_preempt_defer()` when the interrupted context is ring 3.**

Shape (implementation detail for the implementation seat, not applied here):

```rust
// timer.rs
sched::on_tick(interrupted_cs & 3 == 3);   // was sched::on_tick()

// scheduler.rs
pub fn on_tick(from_user: bool) {
    // ... idle witness / idle probe unchanged ...
    if !from_user {
        if PREEMPT_DISABLE_DEPTH[cpu].load(Acquire) != 0 { return; }
        if consume_preempt_defer() { return; }
    }
    yield_now_recording(YieldKind::TimerPreemption);
}
```

Why this is the smallest correct fix:

- A ring-3 thread holds **no kernel lock** and owns **no kernel critical section**; the two gates
  exist for kernel contexts (`PREEMPT_DISABLE_DEPTH` for `spawn_user`'s publication window;
  `preempt_defer` for IPC handoff). Honoring them for ring 3 is a category error and is what lets a
  ring-3 spinner monopolize the BSP.
- It needs no new state, no ready-age bookkeeping, no lock, and no change to `rotate`/the ready ring.
  It is one bool through the existing handler and one `if` in `on_tick`.
- It is robust to the *cause* of a nonzero depth/defer: even if a future path leaks either, a
  ring-3 thread is still preempted on the next tick. That matters because this trace's evidence cannot
  distinguish a leaked `spawn_user` guard from a re-armed handoff credit without the dump; the fix
  makes both harmless for user-mode preemption.
- It does **not** weaken kernel preemption: ring-0 interrupted contexts keep both gates exactly as
  today (spawn publication, handoff, ISR bodies).

**What must be re-proven (regression surface):**

- The IPC/thread batteries that rely on `HANDOFF_PREEMPT_DEFER_TICKS` — `threads_tls`,
  `threads_exit_group`, `smp_same_cpu_ipc`, `smp_cross_cpu_rendezvous_stress`, `smp_c11_migration_reservation`
  — because a ring-3 peer may now be preempted immediately after a handoff and be observed `Ready`
  before its next syscall. That is a test-determinism/latency concern (`wake_thread_front`'s comment,
  `scheduler.rs:2330-2342`), not a data-corruption one: the peer holds no lock in ring 3. If a
  battery flakes, the correct follow-up is to scope the defer to the peer's **first syscall** (which
  `enter_syscall` already clears) rather than to re-enable the ring-3 gate.
- `irq_xmm_preserve`'s timer leg and the `lapic_timer`/`lapic_timer_demote` scenarios, because the
  timer body's switching behavior becomes visible from ring 3.
- The full 4-shard QEMU gate at `NEO_SMP=4`, plus the new scenario above.

**If the tick itself is frozen** (second outcome), this fix is not sufficient; the implementation
seat should then treat the timer source (PIC/PIT/LAPIC EOI / re-arm) as the bug. The falsification
test's `TICKS_SEEN` row makes that call before any fix is written.

**The separate AP fix (do not fold into this trace's fix):** arm the LAPIC timer on every AP (per-CPU
tick, not only `init_bsp`), and make `reschedule_ipi_body` a real reschedule point (check the ready
queue and call `on_tick`/`rotate`), per CP-15/CP-21. That is the right general fix for the class and is
required before `place_thread_on_cpu`/AP user scheduling lands. It is larger than H1's fix and is not
needed to close this trace; keep it as its own lane so a scheduler-core change is not smuggled into a
userland PR.

## 4. Is image A exposed today?

**Yes, to H1.** Image A's WM and Arthur spin in ring 3 without yielding (`desktop-park` off,
`userland/neo-arthur/src/main.rs:1178-1190, 1392-1405`,
`userland/neo-init/src/desktop_session.rs:2103-2115` on `09e62bcb1`). Any client whose burst is long
enough to be preempted at the back of the BSP ready ring — the terminal is the first — can wait behind
them until one of them blocks or exits. The probe app is saved only by its short bursts and ending
yield. The terminal's own per-row/per-4KiB yields do not help once it is behind two users that are
allowed to stay `current`.

**Not exposed today, but armed:** the AP-tick gap (H2). No production path puts a user thread on an
AP now, but `wake_thread_front` already migrates a woken peer to the waker's CPU; the day a user thread
is placed on an AP (or AP work starts waking user peers), a ring-3 spinner there has no tick and no
EOI-only IPI can preempt it. That is CP-15/CP-21/CP-25 and should be fixed before that placement is
enabled, not after.

## Pitfall-bank classes

- **H1 touches:** CP-15/CP-21 (the reschedule point must actually reschedule a busy ring-3 target) and
  the fairness assumption behind CP-25. It **does not introduce** CP-37: a ring-3 thread holds no
  kernel lock, so skipping the kernel-context gates cannot expose a critical section. It leaves
  CP-22/CP-28 (`sti; hlt` idle) untouched.
- **H2 touches:** CP-15/CP-21 (EOI-only IPI / no-timer AP) and CP-25 (AP busy-yield); it is the
  existing latent entry, not a new one.
- **If H1's fix is wrong:** the failure mode is a handoff-latency/determinism flake in the IPC
  batteries (peer observed Ready earlier than the test expected), not corruption or a deadlock; the
  remedy is the syscall-scoped defer above. If the fix is applied when the tick is actually frozen,
  it changes nothing and the `TICKS_SEEN` test says so before merge.

## Files read

`kernel/src/arch/x86_64/lapic_timer.rs`, `lapic_timer_logic.rs`, `apic.rs`, `smp.rs`, `timer.rs`,
`idt.rs`; `kernel/src/sched/scheduler.rs`, `wait_any.rs`, `wake_trace.rs`, `process.rs`,
`kernel/src/mm/tlb_shootdown.rs`; `docs/concurrency-pitfalls.md` CP-15/CP-19–CP-29,
`docs/THINK-2026-10-19-ot22-readiness-multiplex.md`; the #98 branch's
`userland/neo-worker/src/terminal_app.rs`, `userland/neo-terminal/src/render.rs`,
`userland/neo-init/src/desktop_session.rs`, `userland/neo-arthur/src/main.rs` at `09e62bcb1`, and the
PR #98 PM comment (2026-11-09 21:39 UTC).
