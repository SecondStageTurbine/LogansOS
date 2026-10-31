<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: LAPIC timer round-5 narrow re-check (PR #84)

**Verdict: MERGE. R4-P2b is closed.** All four requested checks pass within this narrow review. No new finding.

Reviewed head `9a235ac3491cb44fe512d066ffa2ad27faef377a`, branch `codex/lapic-timer-recheck5-jarvis`, exclusively in `E:\AI Automation\workers\jarvis`. Read the round-5 dispatch, round-4 RETURN at `dbad7bae966569e0483accb937c3ba6adfc7a65e`, and the fix diff `33cd1ca3..a6874b28`. Source lines below refer to the reviewed head.

## Requested checks

| Item | Result | Enforcing lines / evidence |
| --- | --- | --- |
| 1. PIT is never primary or sole calibration source; PIT alone falls back to verified PIT/PIC | **PASS** | `kernel/src/arch/x86_64/lapic_timer_logic.rs:205-212` selects CPUID 0x15 or the first non-PIT measurement; `:215-220` returns `PitOnly` when only PIT measurements exist, even with a leaf-16 estimate. `kernel/src/arch/x86_64/lapic_timer.rs:422` maps TSC `PitOnly` to `None`; `:444-446` maps LAPIC `PitOnly` to `None` and records `pit-only-clock`. `lapic_timer_logic.rs:111-122` consequently selects `Plan::Pit`; `lapic_timer.rs:471` returns false. `kernel/src/sched/scheduler.rs:369-383` programs PIT, unmasks IRQ0 and verifies progress; `:447-452` accepts observed ticks, while `:456-464` names `NO TICK SOURCE` and fails/halts if none arrive. |
| 2. Round-4 delayed observation cannot calibrate; as checker against HPET or leaf 0x15 its roughly 2x rate is refused | **PASS** | `lapic_timer_logic.rs:220` gives the injected sole PIT rate `PitOnly`; `:233-236` returns `Mismatch` when the selected checker differs by more than 5%. `lapic_timer.rs:415-420` refuses disputed TSC for deadline mode; `:438-440` refuses disputed LAPIC for periodic mode. KAT `tools/lapic-timer-logic-tests/src/lib.rs:263-308` (`the_pit_never_calibrates_alone`) passes: sole PIT, PIT plus estimate, HPET/PIT in both orders, leaf-15/PIT mismatch, and a good PIT checker. |
| 3. Poll-gap check refuses the held-off sample | **PASS** | `kernel/src/acpi/timers_logic.rs:228-232` timestamps each observation and returns false before accepting an edge if the gap exceeds `max_gap`. `kernel/src/arch/x86_64/refclock.rs:72` sets the production limit to 500,000 TSC cycles; `:268-271` passes it and drops a failed sample. KAT `tools/acpi-timer-logic-tests/src/lib.rs:474-487` (`pit_poll_held_off_across_the_edge_is_refused`) passes, demonstrating acceptance with only the old bounds, refusal with the gap bound, and acceptance with steady polling. |
| 4. No additional timer-lane changes since `33cd1ca3` beyond this fix | **PASS** | `git diff 33cd1ca3 a6874b28 -- kernel tools` contains only the four timer source files above, the PIT-only feature at `kernel/Cargo.toml:687`, and the two host KAT files. `git diff --exit-code a6874b28 HEAD` over those timer source/KAT paths plus scheduler, timer, PIT, PIC and APIC returns 0. Inspection of the remaining shared-file diff shows unrelated framebuffer/panic and block-write-guard changes from main; timer scenario code/registration is unchanged. This is a timer-lane assertion, not a claim that the entire `kernel`/`tools` trees are identical after the main merge. |

The rate exclusion also covers waits: `lapic_timer_logic.rs:82-85` gives `PitOnly` and `Mismatch` the 10 GHz bound, published at `lapic_timer.rs:425-426`. Measurement limits are fixed constants (`refclock.rs:53,62,65-72`); neither a PIT-only rate nor a refused rate sizes a later measurement. Both calibration entry points use `choose_rate` (`lapic_timer.rs:280,298`).

The two protections are distinct. The original trace `(LOW, 2,400), (LOW, 21,600,000), (HIGH, 48,000,000)` now fails the production gap bound by inspection (already at the second observation). If its approximately 2x rate is injected past that measurement filter, selection yields `PitOnly` alone or `Mismatch` as the selected checker against an accurate HPET/leaf-15 rate. In the real filtered path it yields no PIT measurement, rather than a `PitOnly` value. The KAT exercises rate selection separately from poll filtering.

“Disturbed sample refuses” here means the requested 2x case, or a selected checker outside the explicit 5% tolerance. Small differences within tolerance can pass, and only the selected independent checker is compared. No arbitrary-jitter guarantee is claimed. A platform with CPUID 0x15 still has an independent rate even if PIT is its only reference-clock device; the PIT-only control hides HPET/PM, not CPUID. These qualifications do not reopen the sole-PIT calibration defect.

## Validation

- `cargo test -p acpi-timer-logic-tests -p lapic-timer-logic-tests --target-dir 'E:/AI Automation/workers/jarvis/target/recheck5'`: **PASS**, 14 + 12 unit tests, zero failures; both named regression KATs passed. No doc tests.
- `git diff --check 33cd1ca3 HEAD -- kernel tools`: **PASS**.
- Integration/fallback guards reviewed in source. No kernel build, QEMU control, four-shard gate or silicon run was performed for this narrow re-check.
- RAG health was healthy. Graft's discovery result contained pre-fix source; checked-out Git source supplied the verdict evidence.
- Ran worktree-local `cargo clean --target-dir 'E:/AI Automation/workers/jarvis/target/recheck5'` after testing (18.8 MiB removed).
- Report-only change. Existing `.gitignore` and `.ignore` changes are excluded from the commit.
