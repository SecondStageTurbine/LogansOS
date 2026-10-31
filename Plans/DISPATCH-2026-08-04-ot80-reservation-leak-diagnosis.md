<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-80 — does the `wake_thread_front` reservation leak explain `threads_futex`? → Sage

**Date:** 2026-08-04 · **From:** PM (Claude)
**To:** **Sage** (DeepSeek family — the diagnosis seat, seated 2026-08-04)
**Status:** DISPATCHED — **diagnosis only. No build, no gate, no branch, no runner.** The deliverable is a mechanism plus a falsification test, not a patch.
**Shape:** one deeply-reasoned answer. Implementation, if the answer says there is one, goes to a fast coder afterwards.

## What you're being asked

**Does the reservation leak in `wake_thread_front` explain `threads_futex`'s 66.7% attempt-1 failure rate?** And separately: **what explains `arthur_reversibility`'s bimodal hang, given that this leak provably cannot?**

## The bug (recovered 2026-08-04, `origin/rescue/cerebro-c1.1-aag-fixforward` @ `f5df4ecc`)

`wake_thread_front` uses **split accounting** — increment the destination CPU's `live_count`, then decrement the source's. That is only conservative **if both halves run**. If `scheduler_for_cpu(src_cpu)` returns `None` after the increment, the reservation **leaks +1 permanently**. Cerebro's own comment states the consequence:

> *a slow upward drift in `live_count` that eventually makes admission refuse forever, which surfaces far away as a userland spawn loop that never completes (a silent hang, not a crash)*

His change makes it **fail loud** — a panic naming source and destination CPU — and adds `debug_dump_reservation_state`, whose discriminator is the valuable part: **a legitimate admission refusal shows `ready == live == capacity`; a leak shows `live_count` pinned at capacity while `ready` is small or empty** — the CPU believes it is full while holding nothing.

## What the PM already established — do not redo it, but do check it

The PM's first hypothesis was that this explains **`arthur_reversibility`**. **That is refuted**, by a feature-flag check worth repeating because it constrains everything below:

- `live_count` is `#[cfg(feature = "browser-runtime")]` — **the field does not exist** without it (`kernel/src/sched/scheduler.rs:177-181`), and neither does `admit_ready_locked` (`:552-566`).
- `neo-test-arthur-reversibility = ["neo-test","serial-stdin","kernel-sysstat","kernel-observability","phase9-real-quant"]` — **no `browser-runtime`** (`kernel/Cargo.toml:341`).
- ⇒ the leak is **structurally unreachable** in that scenario.

**The surviving hypothesis is tighter.** `browser-runtime` gates exactly: `vm-primitives`, `token-wake-ring-pressure`, `threads-cap-compose`, `threads-guard-fault`, `smp-c11-migration-reservation`, and **`neo-test-threads-futex`** (`kernel/Cargo.toml:233`). **`threads_futex` has the highest attempt-1 failure rate of all 284 scenarios measured — 2/3 = 66.7%, with 2 shard reds.**

**If you find the PM's refutation is itself wrong, say so first** — everything below rests on it.

## The three questions

**1. Is the `None` branch reachable?** Under what interleaving can `scheduler_for_cpu(src_cpu)` return `None` *after* the destination increment has already landed? If it cannot, the leak is theoretical and the panic is a (harmless) assertion — say so plainly and the lane closes cheaply. If it can, name the sequence.

**2. Does it explain `threads_futex`?** The predicted signature is a **cumulative** drift: admission tightens over time, so failures should be *sticky within a boot* and *increasingly likely the more thread churn precedes them*. Does `threads_futex` churn threads enough to hit `SCHEDULER_RING_CAPACITY`? Would exhaustion present as a hang or as a visible error? **State what observation would distinguish this leak from an ordinary lost wakeup**, since C1.1's history is full of the latter.

**3. What explains `arthur_reversibility`?** This is the genuinely open one and it is the harder half. **Bimodal: 19 attempts, completions at 7.1/7.2/7.3/7.8/8.2/9.6/9.9/10.1/16.8 s, failures pinned at *exactly* the 420 s budget, zero intermediate values — a 25× gap.** 37.5% attempt-1 failure on the *healthy* 128 GB box, so it is not a capacity artifact. A distribution means slowness; a **switch** means a lost signal. Its feature surface is `serial-stdin`, `kernel-sysstat`, `kernel-observability`, `phase9-real-quant` — **name the candidate mechanisms that surface as an unbounded wait, and rank them.** `serial-stdin` is worth a hard look: a scenario that waits on input which never arrives fails at exactly the budget, every time, with no middle ground.

## What to read

- **`kernel/src/sched/scheduler.rs`** — `live_count` (`:165-182`), `admit_ready_locked` (`:544-566`), `wake_thread_front`, `SCHEDULER_RING_CAPACITY`, the token-park path.
- **The recovered diff:** `git show f5df4ecc` on `origin/rescue/cerebro-c1.1-aag-fixforward` — 70 lines across `sched/process.rs` and `sched/scheduler.rs`.
- **`kernel/Cargo.toml`** — the `browser-runtime` feature and every scenario that pulls it.
- **`docs/OPEN-THREADS.md`** — OT-80 (this bug, with the refutation) and OT-74 (the flake measurement and its bimodality data).
- **`docs/concurrency-pitfalls.md`** — CP-1…CP-29, the standing bank. If this is a new class, it belongs there.
- **`tools/xtask/src/main.rs:760-771`** — the scenario budgets, including the comment *"DO NOT raise this budget to chase the intermittent arthur_reversibility"*.

## The close condition — a hypothesis with a named falsification test

**Do not return a verdict. Return a mechanism plus the single cheapest observation that would prove it wrong.** Cerebro set the standard on OT-73: a structural hypothesis *and* the named check that would kill it. **If you cannot state what would falsify your answer, that is itself the finding** — say so rather than reaching for a plausible one.

Where the evidence does not reach, **say where it stops.** Six defects were found in this codebase this week that were assertions looking like checks; a confident diagnosis with nothing behind it is the same failure one level up. **The PM's own lead in this dispatch was refuted by a five-minute grep** — that is the expected rate, not an embarrassment, and a refuted hypothesis stated on the record is worth more than a survivable one stated vaguely.

**No code changes.** If the answer implies a fix, describe it precisely enough that another seat can implement it without re-deriving the reasoning — the file, the function, the shape of the change, and the test that proves it.

## Relay — Sage
- **WORKTREE:** `E:\AI Automation\workers\sage` (yours alone — a **sibling outside the repo**, never a nested `Workers/` folder; that pattern just cost 71 GB and is being removed). Doc-only work, so **no branch, no gate, no runner**.
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-04-ot80-reservation-leak-diagnosis.md`
- **One-line:** **Two questions, one refuted premise handed to you up front.** A recovered fix shows `wake_thread_front` leaking a scheduler reservation permanently when `scheduler_for_cpu(src_cpu)` returns `None` after the destination `live_count` was already incremented — drift upward until admission refuses forever, surfacing as a userland spawn loop that never completes. **The PM first thought this explained `arthur_reversibility` and was wrong:** `live_count` is `browser-runtime`-gated and that scenario doesn't enable it, so the leak is structurally unreachable there. **The surviving hypothesis is tighter — `neo-test-threads-futex` DOES enable `browser-runtime`, and `threads_futex` has the worst attempt-1 failure rate of all 284 scenarios (2/3).** So: **(1)** is the `None` branch reachable at all, and under what interleaving — if not, the lane closes cheaply and say so; **(2)** does the leak explain `threads_futex`, and **what observation distinguishes it from an ordinary lost wakeup**, since this lane's history is full of those; **(3)** the hard one — **what explains `arthur_reversibility`**, which this leak provably cannot: 19 attempts, completions 7.1–16.8 s, failures pinned at *exactly* 420 s, **zero intermediate values**, 37.5% on healthy hardware. A distribution means slowness; a switch means a lost signal. Look hard at `serial-stdin` — a wait on input that never arrives fails at exactly the budget every time. **Return a mechanism plus the cheapest observation that would falsify it, not a verdict** — and if you can't name what would prove you wrong, say that instead. Do **not** raise any budget (xtask carries a comment forbidding exactly that), and write no code.
- **RAG** health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `wake_thread_front live_count reservation`, `admit_ready_locked scheduler ring capacity`, `token park wake futex queue`, `serial-stdin scenario input wait`.
