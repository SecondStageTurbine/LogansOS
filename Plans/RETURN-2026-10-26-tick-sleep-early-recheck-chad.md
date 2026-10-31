<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #70): tick-sleep early-control re-check (#132)

Reviewed SHA: `e4c4baec6b5a03e54f457418db07778d1cf6a3a7`.
Branch: `codex/tick-sleep-early-recheck-chad`.
Overall verdict: **FAIL, item 1: deadline equality does not identify the selftest's sleeper.** The original relay #61 schedule is detected, but another sleeper can overwrite the early-expiry evidence. Review only; no production change or revert committed. Report this confirmed control-coverage failure to the PL for revert-first handling.

| Item | Verdict | Evidence at reviewed SHA |
|---|---|---|
| 1. Expiry witnessed for own sleeper | FAIL | `kernel/src/sched/tick_sleep.rs:147` matches only `s.deadline == SELFTEST_DEADLINE`, then unconditionally stores the current tick at `:148`. Two different tokens may have the same deadline; neither the installer at `:101` nor the selftest publication at `:237` makes deadlines unique. `installed` also matches only deadline at `:228`. USB's real shared-table caller uses a 25-tick period (`kernel/src/drivers/usb/mod.rs:296`, `:316`, `:330`). Concrete overwrite schedule below. |
| 2. Expiry-based predicate / original schedule | PASS (predicate and isolated replay) | `kernel/src/sched/tick_sleep.rs:258` implements `expired_at < deadline || woke_at < deadline`. With start=100, deadline=120, expired_at=119, woke_at=120, it yields true, so `kernel/src/test_harness.rs:24955` rejects with `tick_sleep_not_parked`. Missing expiry 0 is also less than positive deadline 120. The item-1 overwrite can corrupt the predicate's input; this PASS does not establish unconditional control discrimination. |
| 3. Other controls fail | PASS | No-wake uses effective now=0 (`kernel/src/sched/tick_sleep.rs:134`), so a positive deadline does not expire. The wait at `:245` is bounded to 200 ticks, with a TSC fallback at `:209`; failure detaches at `:248` and returns woken=false at `:249`, rejected at `kernel/src/test_harness.rs:24955`. Never-park returns Full at `tick_sleep.rs:113`; regardless of coincidental installed-deadline evidence, `past_skips` at `:261` is false because that same control cannot return Due, so the harness rejects. No unbounded join is reached for a worker that never wakes. |
| 4. Production unchanged | PASS (source gating) | The requested kernel diff changes only `kernel/src/sched/tick_sleep.rs`. The new expiry instrumentation is individually gated at `:146`, the two statics at `:178` / `:180`, and the remainder is inside the already gated selftest at `:195`. With neo-test-usb-hotplug absent, the executable source path is unchanged. S2's reported identical .text builds were not independently reproduced. |

## Confirmed counterexample: another token overwrites the witness

Use early-control alone, with an otherwise normal successful selftest:

1. Tick 95: the USB port thread enters `sleep_ticks(25)`. At `tick_sleep.rs:59`, it computes the absolute deadline 120, then is delayed before entering `park_until`'s interrupt-disabled critical section. No table slot has been installed for it yet. The argument remains 120 when this thread next runs.
2. Tick 100: the selftest worker publishes deadline 120, installs its own token, and parks. The observer sees it and records parked=true.
3. Real tick 119: early-control uses effective now=120 (`:129`), removes the worker's token, records SELFTEST_EXPIRED_AT=119, and wakes it. The ready worker is delayed until tick 120.
4. After that tick-119 timer, the USB thread resumes. Its saved deadline is still 120. The actual-tick check at `:93` sees 119<120, so it installs its different token with deadline 120 in a free slot. There is no immediate call to on_tick_expire from the installer.
5. Real tick 120: effective now=121 expires the USB token. Since its deadline equals SELFTEST_DEADLINE, `:148` overwrites SELFTEST_EXPIRED_AT with 120. The selftest worker then resumes at tick 120, publishes WOKE_AT=120, and completes before the observer reads the result.
6. The observer reads expired_at=120, deadline=120, woke_at=120: woken_early=false, slept=20, woken=true, parked=true, past_skips=true. The harness's entire rejection predicate is false. The worker's actual expiry was nevertheless one tick early.

This uses the existing USB period and the ordinary possibility of delaying a runnable thread; it needs neither a second selftest nor a new sleep caller. The 24-tick delay in step 1 is before the interrupt-disabled park, so it does not require preemption inside that critical section. Distinct tokens are representable and generation checked (`kernel/src/sched/scheduler.rs:153`); the witness simply does not compare them.

I evaluated the exact match/store and final harness predicates locally in Python: the isolated original schedule gives `woken_early=true`; the two expiry events `(worker,119,deadline120)` and `(usb,120,deadline120)` leave the record at 120 and give `harness_rejected=false`. This is a checked source-level scheduling counterexample, not a QEMU reproduction or a claim of production sleep returning early. An identity tied to the worker's actual token/registration is needed to satisfy item 1; an unconditional last-writer timestamp keyed only by deadline cannot do so.

## Validation and close

Read the dispatch, prior relay #61 RETURN, complete tick-sleep source, timer call site, USB caller, wake-token representation, and harness predicate. RAG health was healthy (ollama=true, qdrant=true); exact known paths made direct reads sufficient. No build or QEMU run was required or performed. Only this RETURN is committed. Preexisting graft cache content was preserved in stash `chad preexisting graft before relays 70 and 71`. After push, close checks require empty `git status --porcelain` and empty `git log '@{u}..'`.
