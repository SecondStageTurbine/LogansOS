<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: LAPIC timer tick, round-4 review (#84) → GPT-6-Astra (PL escalation rule)

**PR:** #84, `claude/lapic-timer`, on `main` `d4244be5` (which contains #82). Its diff against `main` is
the timer lane only. Three Codex rounds came before this one (Jarvis):

| Round | RETURN | Branch / SHA | Outcome |
|---|---|---|---|
| 1 | original cross-check | `codex/lapic-timer-crosscheck-jarvis` | P2-a, P2-b, fallback limitation |
| 2 | `RETURN-2026-10-21-lapic-timer-recheck-jarvis.md` | `codex/lapic-timer-recheck-jarvis` `2b45c36e` | R1–R4 |
| 3 | `RETURN-2026-10-21-lapic-timer-recheck3-jarvis.md` | `codex/lapic-timer-recheck3-jarvis` `37a0ac3c` | R4-P2, R1 remainder, R2-P3 |

```
git fetch origin
git show origin/claude/lapic-timer:Plans/DISPATCH-2026-10-21-lapic-timer-recheck4.md
git show origin/codex/lapic-timer-recheck3-jarvis:Plans/RETURN-2026-10-21-lapic-timer-recheck3-jarvis.md
git diff origin/main origin/claude/lapic-timer -- kernel tools
```

WORKTREE: `E:\AI Automation\workers\<reviewer>` (yours alone; never another coder's folder, never the repo root)
BRANCH: `codex/lapic-timer-recheck4-<reviewer>` (`git fetch origin && git checkout -B <branch> origin/claude/lapic-timer`)

## What round 3 changed (verify each)

| Finding | Fix | Where | KAT |
|---|---|---|---|
| **R4-P2**: a late PIT edge was accepted | `pit_one_shot_completed(read, now, low_timeout, min_window, high_timeout)` times each edge on the read that observes it. A LOW after `low_timeout`, a HIGH after `high_timeout`, or a HIGH before `min_window` (all from the call) is refused. `refclock` passes `min_window` = half of 10 ms at a 500 MHz TSC floor | `acpi/timers_logic.rs`, `refclock.rs::measure_pit` | `pit_edges_are_timed_on_the_read_that_sees_them` (late LOW, late HIGH, too fast, inclusive bounds), plus `pit_one_shot_needs_both_edges` |
| **R1 remainder**: an estimate was still used | `Estimated` TSC: no TSC-deadline period (deadline refused and logged), `TSC_HZ` 0, wait budgets at the 10 GHz bound. `calibrate_lapic` no longer measures against an estimated TSC: with no crystal and no clock it is `Unknown`, so the PIT is tried. `rate_hz` is removed | `lapic_timer.rs::start`, `calibrate_lapic` | `budget_rate_by_outcome`, `a_refused_tsc_rate_never_sizes_a_wait` |
| **R2-P3**: status write for edge mode | Status is written back only for comparators with `Tn_INT_TYPE_CNF` (level) set | `timers_logic.rs::hpet_take_ownership` | `hpet_status_write_back_is_level_only`, `hpet_takeover_while_running` |

## Refute it (the whole lane, not only round 3)

1. **Calibration.** Can any path still program a tick, or size a wait or a measurement, from a rate that is
   neither exact (CPUID 0x15) nor measured against an independent clock and cross-checked when a second
   source exists?
2. **PIT.** Is the 500 MHz floor sound? Can a real 8254 under SMI or VM-exit jitter be refused often enough
   to lose the only reference clock? A refusal means falling through to the next clock or to `Unknown`, so
   the cost is a lost calibration, not a wrong one. Is that acceptable?
3. **HPET.** Is the takeover ordering (stop + `LEG_RT` off, comparator interrupts off, level-only status
   clear, enable) right for every firmware state? What about a 32-bit counter wrapping inside a window?
4. **The tick.**
   - Do first-tick verification, demotion (deadline → periodic → PIT), the EOI by ISR bit, and the
     verified PIT fallback with `NO TICK SOURCE` hold in both APIC modes?
   - Is there any interleaving where a tick is lost, or the LAPIC ISR bit stays set?
5. **Completeness.** What does the 4-shard gate not cover? TSC-deadline mode cannot run under TCG; a
   firmware-configured HPET and a real gated PIT are host-KAT only.

Optional runs at `NEO_SMP=4`:
- `lapic_timer_tick`, `lapic_timer_tick_x2apic` (QEMU ≥ 9) and `lapic_timer_demote` should PASS.
- The controls should FAIL as named:
  - `neo-test-lapic-timer-control` → `timer_not_lapic`;
  - `+ neo-test-lapic-timer-pit-gated` → `no_tick_source`;
  - `neo-test-lapic-timer-no-verify` → `no_tick_after_unverified_mode`;
  - `xapic-only` on the x2APIC variant → `apic_mode_not_x2apic`.

Host checks: `cargo test -p acpi-timer-logic-tests -p lapic-timer-logic-tests`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='PIT channel 2 one-shot OUT2 port 0x61 calibration'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-21-lapic-timer-recheck4-<reviewer>.md` to your branch and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- round 3's three items, each closed or not, with evidence;
- each new finding with severity, file:line and a repro;
- what you checked for items 1–5.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete
the worktree.
