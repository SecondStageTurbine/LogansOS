<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #75): tick-sleep worker-witness re-check (#138)

Reviewed SHA: `c3aae11c5953f5058fa476ecd9dfd6c93c0a0bde`.
Branch: `codex/tick-sleep-witness-recheck-chad`.
Overall checklist verdict: **FAIL item 5 for retry after timeout; PASS for the relay #70 witness correction itself.** Items 1-4 and 6 pass. The item-5 limitation already existed in the prior selftest's detached-worker/static-state design; this is not evidence that #138 introduced a regression. Completed sequential runs reset correctly. No production edit or revert is included; the PL should distinguish this existing retry limitation from the repaired overwrite finding before applying revert-first handling.

| Item | Verdict | Evidence at reviewed SHA |
|---|---|---|
| 1. Witness keyed to the worker | PASS | `kernel/src/sched/tick_sleep.rs:149` compares `s.token.tid` to `SELFTEST_TID`; `:150` uses compare_exchange from 0 and `:152` stores a value at least 1. Replay below: the USB token cannot replace 119 with 120. |
| 2. Tid published before sleeper visibility | PASS | The worker Release-stores its current tid at `kernel/src/sched/tick_sleep.rs:247`, before DEADLINE at `:253` and sleep_until at `:254`. The slot is installed at `:101` under TABLE. The per-CPU getter is `kernel/src/arch/x86_64/per_cpu.rs:172`; scheduler dispatch maintains that id (`kernel/src/sched/scheduler.rs:1846`), and the token carries the parked thread's own id (`:1243`, `:1254`), rather than the next thread's id after the switch. |
| 3. Installed matches worker slot | PASS | `kernel/src/sched/tick_sleep.rs:232` loads tid and `:240` requires both tid and deadline under TABLE. Same-deadline USB evidence alone cannot satisfy it. |
| 4. Original schedule and other controls fail | PASS | `kernel/src/sched/tick_sleep.rs:273` computes expired_at < deadline OR woke_at < deadline. For start100/deadline120/expiry119/resume120 this is true, and `kernel/src/test_harness.rs:24955` rejects with tick_sleep_not_parked at `:24956`. No-wake uses effective now=0 (`tick_sleep.rs:134`), so the positive worker deadline never expires; the bounded wait (`:218`, `:260`) returns woken=false through `:264`. Never-park returns Full at `:113`, cannot produce the worker slot, and its past-deadline probe cannot return Due (`:276`), so the harness rejects it too. |
| 5. Reset between runs | FAIL for timeout/retry; PASS after completed runs | Entry clears SELFTEST_EXPIRED_AT at `kernel/src/sched/tick_sleep.rs:245`; the completed path joins at `:266` and clears SELFTEST_TID at `:270`, so a completed run cannot supply the next run's expiry. However the timeout path at `:261` detaches and returns at `:264` without clearing tid or terminating/draining its worker. Detach only changes ownership (`kernel/src/sched/kthread.rs:377`), not execution. A retry can read an expiry written by that first run after its own reset; concrete schedule below. |
| 6. Production unchanged | PASS | The requested diff adds a cfg-gated expiry block (`kernel/src/sched/tick_sleep.rs:148`), cfg-gated statics (`:189`, `:191`), comments, and changes inside the already cfg-gated selftest (`:206`). All executable additions require neo-test-usb-hotplug. No independent binary comparison was needed or performed. |

## Relay #70 overwrite replay

USB computes deadline120 at tick95 and is delayed before installation. The test worker starts100, publishes its tid and deadline120, then parks. Early-control expires it at real tick119 (effective now120): the witness changes 0 -> 119. USB installs its distinct token at real119 and expires at120. Its token tid differs, so it cannot write the witness. Even a subsequent matching-tid expiry would fail the compare_exchange once the record is nonzero. Worker resumes120; expired_at119 < deadline120 remains true. The harness fails as required. This directly closes relay #70 item 1.

## Item 5: retry can overlap the detached first worker

This schedule uses ordinary expiry, distinct workers A/B, and the source's existing bounded observer. Tick values are illustrative; a delayed runnable worker is sufficient.

1. Tick100: run A spawns its worker, which is not scheduled during the observer's waits. The parked wait ends after 10 ticks and the wake wait after another 200 ticks (or earlier via the TSC bound). By tick310, run A detaches and returns woken=false; its worker is still runnable.
2. Tick311: caller retries. Run B clears WOKE_AT, DEADLINE and SELFTEST_EXPIRED_AT, then spawns B.
3. Tick312: B runs, publishes tidB, START312 and DEADLINE332, installs its slot and parks. B's observer sees its matching slot, recording parked=true.
4. Tick313: delayed A finally runs. It publishes tidA, START313 and DEADLINE333 into the same statics and parks its own token.
5. Tick332: B's token expires. Since SELFTEST_TID now equals tidA, this does not write the witness. B remains runnable but delayed until333.
6. Tick333: A's token expires and records SELFTEST_EXPIRED_AT333. B then resumes, writes WOKE_AT333 and exits. B's observer joins B and reads expiry333, deadline333 and start313: the expiry and deadline came from A, not B. Its slept value is20 and its early predicate is false.

Thus the literal guarantee that a second run cannot read the first run's record is not established after timeout. Clearing tid alone at entry/timeout would not prevent a still-live A from republishing it; retries need a worker-lifetime policy or isolated per-run state. This limitation predates #138: the prior selftest already detached timed-out workers while reusing static START/DEADLINE/WOKE_AT and witness state. The harness's single invocation (`kernel/src/test_harness.rs:24945`) does not exercise this retry schedule.

## Validation and close

Read the dispatch, relay #70 RETURN, complete tick_sleep.rs, scheduler token construction/park path, per-CPU current-thread getter, kthread detach implementation and harness rejection predicate. Source-level schedule review only; no build or QEMU run, as instructed. RAG health was healthy (ollama=true, qdrant=true); known file paths made direct reads sufficient. Used the Rust best-practices testing and thread-safety references.

Only this RETURN is committed. Push/upstream verification is performed after commit. The requested empty-worktree close condition cannot be met without touching unrelated pre-existing content: `git status --porcelain` already showed `?? graft/` before this review; it is preserved untouched and is the only expected residual. No excludes or stash hide it.

## PM disposition (2026-10-26)

Item 5 stays as is: no revert and no code. The overlap needs a second `tick_sleep::selftest()` call in the same boot
after the first one timed out. There is one caller (`kernel/src/test_harness.rs`, the `neo-test-usb-hotplug` scenario),
it calls the selftest once, and the selftest result decides that boot's verdict. The xtask retry of a failed scenario
is a fresh QEMU boot, so every static starts at its initial value. The relay's item 5 asked for more than the code
needs today. If a second in-boot caller is ever added, it needs per-run state or a worker-lifetime policy, as Chad
says. The fix #138 was for (relay #70 item 1) passes.
