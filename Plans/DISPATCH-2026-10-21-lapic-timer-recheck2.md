<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: re-check R1–R4 of the LAPIC timer tick (#84, round 3) → the PM's chosen Codex-family reviewer

**PR:** #84, `claude/lapic-timer`, stacked on #82 (`claude/x2apic-support`) by merge, not rebase.
**Previous RETURN (round 2):** `Plans/RETURN-2026-10-21-lapic-timer-recheck-jarvis.md` on
`codex/lapic-timer-recheck-jarvis` (`2b45c36e`). It closed P2-a, P2-b and the fallback limitation, and found
R1–R4, all P2.

```
git fetch origin
git show origin/claude/lapic-timer:Plans/DISPATCH-2026-10-21-lapic-timer-recheck2.md
git show origin/codex/lapic-timer-recheck-jarvis:Plans/RETURN-2026-10-21-lapic-timer-recheck-jarvis.md
git diff origin/claude/x2apic-support origin/claude/lapic-timer -- kernel tools
```

WORKTREE: `E:\AI Automation\workers\<reviewer>` (yours alone; never another coder's folder, never the repo root)
BRANCH: `codex/lapic-timer-recheck2-<reviewer>` (`git fetch origin && git checkout -B <branch> origin/claude/lapic-timer`)

## What changed for each finding (refute each)

| Finding | Fix | Where | KAT (`cargo test -p acpi-timer-logic-tests -p lapic-timer-logic-tests`) |
|---|---|---|---|
| **R1** A refused TSC rate sized the waits | `budget_tsc_hz(choice)`: a `Calibrated` rate, else `TSC_HZ_BOUND` (10 GHz). It sizes `first_tick_budget` and `verify_pit_fallback` (`lapic_timer::budget_hz()`). On `Mismatch` the TSC rate is `None` and `TSC_HZ` publishes 0. `Estimated` also gives the bound | `lapic_timer_logic.rs` (`budget_tsc_hz`), `lapic_timer.rs::start`, `scheduler.rs::verify_pit_fallback` | `a_refused_tsc_rate_never_sizes_a_wait` (your 24 MHz / 2.4 GHz case), `budget_rate_by_outcome` |
| **R2** HPET enabled without ownership | `hpet_take_ownership(regs, caps)` over a register trait, in order: (1) stop the counter and clear `LEG_RT_CNF`, (2) clear `Tn_INT_ENB_CNF` on each of `NUM_TIM_CAP+1` comparators, (3) write-1-clear latched status, (4) set `ENABLE_CNF`. Other bits are kept, and the takeover is logged when firmware left anything set | `timers_logic.rs`, `refclock.rs::discover` (`MmioHpet`) | `hpet_takeover_from_legacy_with_an_armed_comparator` (your config 0x2 / timer0 0x4 case), `hpet_takeover_while_running`, `hpet_takeover_clean`, plus a replay asserting no write sets ENABLE while legacy routing or a comparator interrupt is set |
| **R3** HW_REDUCED_ACPI ignored | `fadt_pm_timer` returns `None` when flags bit 20 is set, before reading either address form | `timers_logic.rs` | `hardware_reduced_has_no_pm_timer` (your 276-byte rev-6 FADT, checksum-correct, extended and legacy forms) |
| **R4** Stuck-high OUT2 accepted | `pit_one_shot_completed`: OUT2 must read LOW within a short timeout (TSC/1000 of the long one), then HIGH within the long timeout. Otherwise `measure_pit` returns `None` | `timers_logic.rs`, `refclock.rs::measure_pit` | `pit_one_shot_needs_both_edges` (constant 0xff, constant 0x00, a real trace, a late HIGH) |

Each KAT fails with its fix removed; the author checked this by reverting each fix.

## Refute it

1. **R1:** is there any other wait or deadline still sized from a rate that is not `Calibrated`? Check:
   - `calibrate_lapic`'s `Estimated` window;
   - the `no-verify` control's `no_tick_for`;
   - `deadline` planning with `Estimated`.

   Is a 10 GHz bound safe in the other direction? At 1 GHz it means a 0.5 s first-tick wait and a 5 s
   fallback wait. Is that acceptable?
2. **R2:**
   - Is stopping a firmware-running HPET safe at this point in boot?
   - Does clearing `LEG_RT_CNF` actually restore the 8254→IRQ0 route the PIT fallback uses, through the
     8259 virtual wire?
   - Is clearing the status register by write-1 correct for edge-mode comparators?
   - Is anything else needed for FSB/MSI-mode comparators?
3. **R3:** do any other FADT consumers in the timer lane read the fixed-hardware fields?
4. **R4:** can a real PIT ever show OUT2 HIGH for longer than the short timeout after a mode-0 control
   word? Can a device that toggles OUT2 at the wrong rate still pass? Is the outer cross-check enough for
   that?
5. **Completeness:**
   - What does the 4-shard gate still not cover? Firmware-configured HPET states, and a real gated PIT,
     are host-KAT only.

Optional runs, at `NEO_SMP=4`:
- `lapic_timer_tick`, `lapic_timer_tick_x2apic` (QEMU ≥ 9) and `lapic_timer_demote` should PASS.
- Controls should FAIL as named:
  - `neo-test-lapic-timer-control` → `timer_not_lapic`;
  - `+ neo-test-lapic-timer-pit-gated` → `no_tick_source`;
  - `neo-test-lapic-timer-no-verify` on `lapic_timer_demote` → `no_tick_after_unverified_mode`;
  - `xapic-only` on the x2APIC variant → `apic_mode_not_x2apic`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='HPET general configuration legacy replacement comparator'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-21-lapic-timer-recheck2-<reviewer>.md` to your branch and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- for each of R1–R4: closed or not, with evidence;
- each new finding with severity, file:line and a repro;
- what you checked for items 1–5.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete
the worktree.
