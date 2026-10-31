<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: LAPIC timer tick, round-5 narrow re-check (#84, R4-P2b) → GPT-6-Astra

**PR:** #84, `claude/lapic-timer`, on `main` `d4244be5`. **Your round-4 RETURN:**
`Plans/RETURN-2026-10-21-lapic-timer-recheck4-jarvis.md` at `dbad7bae`. That round closed R4, R1 and
R2-P3, and found **R4-P2b**: a delayed PIT observation was accepted as a calibration.

```
git fetch origin
git show origin/claude/lapic-timer:Plans/DISPATCH-2026-10-21-lapic-timer-recheck5.md
git diff 33cd1ca3 origin/claude/lapic-timer -- kernel tools docs
```

WORKTREE: `E:\AI Automation\workers\<reviewer>` (yours alone; never another coder's folder, never the repo root)
BRANCH: `codex/lapic-timer-recheck5-<reviewer>` (`git fetch origin && git checkout -B <branch> origin/claude/lapic-timer`)

## The fix (PM decision: remove the class, not tighten it)

- **PIT channel 2 is never the primary or sole source of a rate.**
  - `choose_rate` takes CPUID 0x15, or else the best **non-PIT** measurement, as the primary. The PIT can
    only be the checker, so a disturbed PIT sample becomes a `Mismatch`, which refuses.
  - A PIT measurement alone is `RateChoice::PitOnly`, which calibrates nothing: no TSC-deadline, no
    periodic mode (`failure=pit-only-clock`), and the wait budgets use the 10 GHz bound. The verified
    PIT/PIC tick runs instead, and a gated PIT still ends in the named `NO TICK SOURCE` halt.
  - Where: `lapic_timer_logic.rs::choose_rate` and `lapic_timer.rs::start` / `log_choice`.
- **Defence in depth.** `pit_one_shot_completed` also refuses a sample when two polls are more than
  `max_gap` apart. In `refclock`, that is a tenth of the window at the 500 MHz TSC floor.
- **KATs:**
  - `the_pit_never_calibrates_alone`: your 2x case alone is `PitOnly` and plans the PIT tick. As the
    checker against the HPET or CPUID 0x15 it is a `Mismatch`, even when the PIT is listed first. A good
    PIT confirms the HPET.
  - `pit_poll_held_off_across_the_edge_is_refused`.
- **Manual control** `neo-test-lapic-timer-pit-only` hides the HPET and PM timer. `lapic_timer_tick` must
  log `measured by the PIT alone - not trusted`, run the verified PIT/PIC tick, and FAIL `timer_not_lapic`.

## Check

1. Is there any path left where the PIT alone, or the PIT as primary, sets a rate that programs a tick,
   sizes a wait or sizes a measurement?
2. With the PIT as checker, does a disturbed sample always refuse, never pass?
3. On a platform with only the PIT, is the outcome always the verified PIT/PIC tick, or the named halt?

Optional: `NEO_TEST_EXTRA_FEATURES=neo-test-lapic-timer-pit-only cargo xtask neo-test lapic_timer_tick`
(expects that FAIL), and `cargo test -p acpi-timer-logic-tests -p lapic-timer-logic-tests`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='choose_rate PIT cross-check calibration source'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-21-lapic-timer-recheck5-<reviewer>.md` to your branch and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- R4-P2b closed or not, with evidence;
- any new finding with file:line and a repro.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete
the worktree.
