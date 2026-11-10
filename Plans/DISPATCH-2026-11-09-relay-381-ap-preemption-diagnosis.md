<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 381: Dexter, diagnosis — a preempted user thread starves beside non-yielding spinners (2026-11-09)

**MODEL** DeepSeek, high. This is a diagnosis job, one hard question about the SMP scheduler core, and the
cross-family lens is the point: the evidence came from a Claude-family seat.
**RETURN:** `Plans/RETURN-2026-11-09-ap-preemption-diagnosis-dexter.md` on branch `dexter/ap-preemption-diagnosis`,
pushed.

## The observation (Seat B, PR #98, run 42695916915, `desktop_terminal_shell_boot`)

The setup is `NEO_SMP=4`, with the WM and Arthur each spinning on `core::hint::spin_loop()` with no yield (image
A's default, without `desktop-park`). A third user process, the terminal, prints `TERM step=0 draw` and then makes
**no syscall** before its next statement, `TERM commit`. Yet that next line appears **about 20 s later**, at the
moment Arthur exits. The terminal was preempted at an ordinary point and didn't get a CPU back until a spinner
went away. The full trace is in Seat B's PM: comment on #98 (2026-11-09 21:39 UTC). A probe client that ends each
loop in a yield never shows this.

Seat B's unverified reading: the preemption tick is programmed on the BSP only (`kernel/src/arch/x86_64/lapic_timer.rs`),
so a thread queued behind a spinner on an AP has nothing to preempt that spinner.

## The question

**Why does a runnable, preempted user thread wait about 20 s while two user threads spin without yielding on four
CPUs, and what's the smallest correct kernel fix?**

Read:
- `kernel/src/arch/x86_64/lapic_timer.rs`, `lapic_timer_logic.rs` and `apic.rs`: which CPUs get a periodic or
  one-shot tick, and when;
- `kernel/src/sched/`: the run queues (per-CPU or global), the preemption point, how a preempted thread is
  re-queued, which CPU's queue it goes on, and whether an idle or other CPU can steal it;
- the AP bring-up path: is the LAPIC timer armed on APs, and is IF=1 in user mode there;
- `docs/concurrency-pitfalls.md`: CP-19 to CP-29, the AP-idle and cross-CPU scheduler hand-off class.

## Close condition (always for a diagnosis job)

1. **A hypothesis:** the exact mechanism, with `file:line` for each link in the chain.
2. **A named falsification test,** a scenario or a measurement that proves the hypothesis wrong if it is wrong. For
   example: two spinner user threads and one compute thread that prints per-ms progress, on `NEO_SMP=4`, with and
   without the AP timer. Say what each outcome means.
3. **The smallest fix** and the pitfall-bank classes it touches (introduces, guards or leaves alone), with what would
   regress if the fix were wrong.
4. **Whether image A is exposed today:** which shipped clients could be starved the same way.

If you can't name a falsification test, that is itself the finding, so report it as such. The fix goes to an
implementation seat afterwards. Don't edit kernel code in this relay.

## Added evidence (Seat B, #98, 2026-11-09 22:31 UTC, run 42704551922 at `496ba8c1`)

- **The stall survives yielding spinners.** With `atrium-terminal`, the WM's polling loop and Arthur's idle loop call
  `sys_sched_yield` instead of `spin_loop()`, and the terminal still stalls about 20 s after `TERM step=0 draw`. It
  is resumed only when Arthur's session ends.
- All four terminal scenarios stall at the same point, on every attempt.
- The burst before the gap is a 430 ms render, long enough under TCG to be preempted at least once.
- The child processes (echo, shell) only exit after the terminal is killed.

So "a spinner never yields" isn't the whole story. Consider a preempted thread re-queued somewhere it isn't picked
from (a per-CPU queue no CPU drains, or a parked-state flag left set), or a wake that targets the wrong CPU. Your
falsification test should tell these apart from "no AP tick".
