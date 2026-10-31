<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: LAPIC timer recheck, round 4 (PR #84)

**Verdict: BOUNCE.** All three round-3 findings are closed as specifically requested. The whole-lane review finds one remaining **P2**: an interrupted PIT measurement can be accepted as a calibrated rate and program the wrong tick frequency. The repaired edge deadlines do not bound the uncertainty between the actual terminal count and its observation.

Reviewed head **`33cd1ca3ffabd895b35b1011d44368456892f070`**, against fetched `origin/main` **`d4244be5`**, in `E:\AI Automation\workers\jarvis`, branch `codex/lapic-timer-recheck4-jarvis`. Read the round-4 dispatch, round-3 RETURN, the complete timer/calibration modules, host KATs, and the kernel/tools diff. All source line numbers below refer to that reviewed head.

## Requested pass/fail checks

| Item | Result | Enforcing line |
| --- | --- | --- |
| **R4: time each observed edge; reject late LOW, late HIGH, and HIGH before min_window** | **PASS / closed.** | `kernel/src/acpi/timers_logic.rs:221-225`: read OUT2, then check `now() - t0 > low_timeout` before accepting LOW. Lines `231-237`: read OUT2, then reject `elapsed > high_timeout`, then require `elapsed >= min_window` for HIGH. Bounds are inclusive. `refclock.rs:250-257` supplies all three bounds. |
| **R1: estimated TSC never sets a period or sizes a wait; no crystal/independent clock means Unknown LAPIC** | **PASS / closed.** | `kernel/src/arch/x86_64/lapic_timer.rs:392-399` disables deadline mode and returns `None` for `Estimated`; `414-416` publishes zero TSC Hz and the safe budget. `lapic_timer_logic.rs:82-85` gives estimates/mismatches/unknowns the 10 GHz bound. `lapic_timer.rs:277-288` calls `choose_rate(crystal, measured, None)` without an estimated-TSC measurement; `lapic_timer_logic.rs:204-210` returns `Unknown` when all three inputs are absent. `lapic_timer.rs:426-432` admits only `Calibrated` to periodic mode. |
| **R2-P3: HPET status writes only for level comparators** | **PASS / closed.** | `kernel/src/acpi/timers_logic.rs:176-177` builds a mask from `Tn_INT_TYPE_CNF`; `184-186` writes only `read_status & level`. Edge and unimplemented comparator bits cannot be written as 1. |
| **Whole lane, dispatch items 1-5** | **FAIL overall: P2 below.** | The PIT path assigns a fixed duration at `kernel/src/arch/x86_64/refclock.rs:263`; its caller uses the complete outer sample delta at `203-208`. An accepted, disturbed sample becomes `Calibrated` at `lapic_timer_logic.rs:221-224` when no second source exists. Details for each dispatch item follow. |

The checked-in edge and HPET regression tests pass (`tools/acpi-timer-logic-tests/src/lib.rs:412-436,442-461`), as do budget and unknown-rate tests (`tools/lapic-timer-logic-tests/src/lib.rs:108-178,228`). The `start`/`calibrate_lapic` integration guards were checked by source inspection; the host crates import the pure logic modules, not the hardware driver.

## New finding R4-P2b: delayed PIT observation corrupts an accepted calibration

**Severity: P2. Location:** `kernel/src/arch/x86_64/refclock.rs:263`, used by `203-208`; acceptance at `kernel/src/acpi/timers_logic.rs:231-237`.

**Trigger:** no usable HPET/PM timer or CPUID-0x15 rate; PIT is the sole reference. The TSC really runs at 2.4 GHz. Program the normal 11,931-count PIT shot, observe LOW promptly, and pause the CPU from 9 ms until 20 ms (for example, a descheduled vCPU). The PIT itself reaches terminal count at about 10 ms and remains HIGH. This is normal mode-0 behavior, not a faulty PIT: [Intel 8254 datasheet, Mode 0, p. 10](https://www.scs.stanford.edu/10wi-cs140/pintos/specs/8254.pdf).

The observed `(OUT2, TSC cycles since helper entry)` trace is:

```text
LOW       2,400          (1 us)
LOW      21,600,000      (9 ms)
HIGH     48,000,000      (20 ms; terminal count happened during the pause)
```

Using the production bounds `(3,000,000, 2,500,000, 3,000,000,000)`, `pit_one_shot_completed` returns **true**. Neither the repaired upper deadline nor the coarse lower bound rejects this trace. `measure_pit` nevertheless returns **9,999,312 ns**, while the measured counter includes the full 20 ms. `measure` also includes programming/return overhead in its outer sample; it does not correct the delay.

**Executed host reproduction, importing the unchanged production helpers:**

```text
accepted=Calibrated { rate: Rate { hz: 4800330262, source: Pit }, checked: None }
reported_ns=9999312
real_tick_period_ns=20001375
real_lapic_hz=24000000; accepted_lapic_hz=48003302; initial=480033
```

The result programs a roughly **50 Hz** deadline tick instead of 100 Hz. A similarly disturbed LAPIC calibration programs a roughly 50 Hz periodic tick. The deadline first-tick budget is five periods of the same inflated rate, so the first tick arrives within it. Thus liveness verification does not catch the frequency error. With a second good source, the cross-check can reject this sample; the sole-reference path expressly has no such check.

This closes the narrow round-3 deadline defect but refutes the dispatch's broader premise that SMI/VM-exit jitter can only cause a lost calibration. Here it produces a **wrong accepted calibration**. No silicon or VM execution is claimed: the reproduced result is the production edge helper plus production rate/plan arithmetic with injected observation times and the duration calculated by `refclock`.

**Required correction:** reject measurements whose sampling gaps or endpoint uncertainty could materially distort the programmed PIT interval, including pauses across terminal count and around programming/sample boundaries. Bracket the observations and retry or return `None` when that uncertainty is excessive. Merely retaining the 3-billion-cycle liveness timeout cannot establish a 10 ms measurement. Linux's [PIT TSC calibration](https://github.com/torvalds/linux/blob/master/arch/x86/kernel/tsc.c) also explicitly checks for disturbed sampling; it is a reference for the failure mode, not a proposed patch to copy.

### Reproduction recipe

Save this as `target/round4-review/repro.rs` after creating that directory, then run `rustc --edition=2024 --test target/round4-review/repro.rs -o target/round4-review/repro.exe` and `target/round4-review/repro.exe --nocapture` from this worktree. This is the principal test from the temporary harness used for this review:

```rust
#![allow(dead_code)]
#[path = "../../kernel/src/acpi/timers_logic.rs"]
mod timers_logic;
#[path = "../../kernel/src/arch/x86_64/lapic_timer_logic.rs"]
mod lapic_timer_logic;
use lapic_timer_logic::*;
use std::cell::Cell;

#[test]
fn delayed_observation_is_promoted_to_a_wrong_calibrated_rate() {
    let events = [(0, 2400), (0, 21_600_000), (0x20, 48_000_000)];
    let index = Cell::new(0usize);
    let started = Cell::new(false);
    assert!(timers_logic::pit_one_shot_completed(
        &mut || {
            let i = index.get();
            index.set(i + 1);
            events[i].0
        },
        &mut || {
            if !started.replace(true) { 0 }
            else { events[index.get() - 1].1 }
        },
        3_000_000, 2_500_000, 3_000_000_000,
    ));
    let count = 1_193_182u64 * 10_000_000 / 1_000_000_000;
    let reported_ns = count * 1_000_000_000 / 1_193_182;
    assert_eq!(reported_ns, 9_999_312);
    let hz = rate_per_second(48_000_000, reported_ns).unwrap();
    let choice = choose_rate(None, &[Rate { hz, source: RateSource::Pit }], None);
    assert!(matches!(choice, RateChoice::Calibrated { checked: None, .. }));
    let Plan::TscDeadline { period } = plan(true, Some(hz), None, 100)
        else { panic!() };
    let real_period_ns = period * 1_000_000_000 / 2_400_000_000;
    assert!(real_period_ns > 20_000_000);
    assert!(period < first_tick_budget(budget_tsc_hz(&choice), 100));
    println!("accepted={choice:?}; reported_ns={reported_ns}; real_tick_period_ns={real_period_ns}");
    let lapic_hz = rate_per_second(480_000, reported_ns).unwrap();
    let initial = periodic_initial_count(lapic_hz, 100).unwrap();
    assert!(initial >= 480_000);
    println!("real_lapic_hz=24000000; accepted_lapic_hz={lapic_hz}; initial={initial}");
}
```

## Whole-lane checklist

1. **Calibration â€” FAIL for measurement validity; PASS for rate provenance/estimate exclusion.** `lapic_timer.rs:175-200` tries HPET, PM timer, then PIT and collects up to two successful measurements. `lapic_timer_logic.rs:196-224` prefers CPUID 0x15, otherwise the best measurement, and rejects disagreement beyond 5% with the next independent source. There is no remaining estimated-rate period or measurement wait; `rate_hz` was removed. However, P2b demonstrates that a sample labeled as independently measured can be wrong. Fixed reference-clock timeout constants (`refclock.rs:52,61,67`) do not depend on an estimated rate. First-tick, no-verify-control, and PIT fallback budgets consume the selected safe budget at `lapic_timer.rs:460,499` and `scheduler.rs:439`.

2. **PIT â€” FAIL for the claimed jitter safety; PASS for the specified edge conditions.** `refclock.rs:64-67` sets a 2.5-million-cycle minimum: 5 ms at the assumed 500 MHz floor, approximately 1.042 ms at 2.4 GHz. This is a coarse filter, not a proof that 10 ms elapsed. A normal uninterrupted 10 ms shot passes on the intended >=500 MHz platforms; 500 MHz is a platform assumption, not a CPUID-enforced architectural floor. A sufficiently slow virtual TSC or a pause that misses the entire LOW phase can cause rejection. The 3-million-cycle LOW budget is 1.25 ms at 2.4 GHz; there is no retry before moving to the next source. Conservative rejection and fallback are acceptable for correctness, with an availability cost if no tick source survives. The accepted delayed-HIGH case above is not acceptable. The normal mode-0 setup at `refclock.rs:242-245` is consistent with the datasheet.

3. **HPET â€” PASS for takeover ordering and ordinary 32-bit rollover.** `timers_logic.rs:169-188` stops the counter and clears legacy replacement, disables interrupt generation on every implemented comparator, clears level-only status, and restarts. FSB delivery is also inhibited by the comparator interrupt-enable bit. These match [HPET 1.0a sections 2.3.5, 2.3.6, and 2.3.8](https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/software-developers-hpet-spec-1-0a.pdf). `refclock.rs:108-119` takes ownership before publishing the mapping. The 32-bit mask at `170` and modular subtraction at `219` handle a window that crosses zero: e.g. start `0xffff0000`, advance 1,000,000 counts at 100 MHz, and the masked difference is still 1,000,000 (10 ms). A full counter revolution hidden during a long VM suspension is not distinguished from the remainder; the code assumes the sampling interval is shorter than a revolution. This review does not claim arbitrary suspension robustness. The hardware-reduced FADT guard also remains effective at `timers_logic.rs:58-60`.

4. **Tick â€” PASS for the intended startup/demotion and EOI paths by inspection, with explicit limits.** Interrupts are enabled at `main.rs:584`, before scheduler startup at `746`; the scheduler exists before arming (`scheduler.rs:362-366`). PIC IRQ0 starts masked (`pic.rs:76`) and is only unmasked on fallback (`scheduler.rs:376-377`). `lapic_timer.rs:353-361,460-474` observes a subsequent tick or disarms/demotes deadline -> periodic -> PIT. `scheduler.rs:444-464` verifies the fallback and names `NO TICK SOURCE` before halting. Timer registers, ISR reads and EOI use the common xAPIC/MMIO or x2APIC/MSR path (`apic.rs:280-313,331-353`).

   A late LAPIC interrupt after `ACTIVE=false` still receives LAPIC EOI because `lapic_timer.rs:135-145` tests ISR[32], not `ACTIVE`. If it interrupts disarming before the mode switch, a rearmed deadline is canceled by that switch/zeroing; after the switch, deadline writes are ignored by hardware. The initial LVT-to-deadline ordering fence is at `320-325`. These behaviors match [Intel SDM Vol. 3A section 11.5.4.1](https://cdrdv2-public.intel.com/774490/253668-sdm-vol-3a.pdf). I found no additional demonstrated path leaving ISR[32] stuck. A literal guarantee of no lost ticks would be false: the existing handler explicitly tolerates coalesced/missed ticks (`timer.rs:357-361`). First-tick checks prove observed progress, not long-term frequency or continued delivery; P2b exploits that distinction. No adversarial hardware interleaving was executed.

5. **Completeness â€” PASS as a coverage audit; QEMU/silicon validation NOT RUN here.** The four-shard gate is selected by `.github/workflows/ci.yml:117`; ordinary PR checks do not imply it ran. Scenario registration is at `tools/xtask/src/main.rs:3380,3397,3407`, with the x2APIC CPU choice at `7642-7644`. The scenario checks frequency against an independent clock (`test_harness.rs:18483-18503`), which is useful on QEMU's ordinary HPET path but does not inject P2b's PIT-only paused observation. TCG does not exercise TSC-deadline; its demotion case exercises periodic -> PIT, not a real deadline -> periodic transition. Host KATs model firmware HPET register states and PIT port traces; they do not establish behavior on an actual firmware-configured HPET or gated 8254. The gate also does not cover scaled-low virtual TSCs, long counter-wrap suspensions, or arbitrary disarm races.

   The requested control markers remain wired: `timer_not_lapic` at `test_harness.rs:18468`; `no_tick_source` at `18519`; `no_tick_after_unverified_mode` at `18525-18527`; `apic_mode_not_x2apic` at `18475-18480`. The PIT-gated control masks IRQ0 (`scheduler.rs:376`); it does not simulate a gated channel-2 reference clock. These controls were inspected, not rerun. GitHub returned an empty `statusCheckRollup` for PR #84 at the reviewed head when queried; that is not evidence of a green four-shard run. Its live main base had advanced beyond the fetched review base, so this review stays pinned to the requested `d4244be5` comparison.

## Validation and handoff

- `cargo test -p acpi-timer-logic-tests -p lapic-timer-logic-tests`: **PASS**, 13 + 11 unit tests, no doc tests.
- Temporary host harness: **PASS**, three tests demonstrating P2b, rejecting all three requested edge failures with production bounds, and returning `Unknown` with no exact/measured/estimated input. A passing reproduction means the defect was observed, not repaired.
- `git diff --check origin/main...HEAD -- kernel tools`: **PASS** at the reviewed head.
- RAG health: healthy. Semantic discovery returned baseline/graph material; Git source at the requested head supplied the review evidence.
- No kernel build, QEMU scenario/control run, four-shard dispatch, or silicon run was performed. Production code is unchanged by this review.
- Build output and the temporary reproduction are removed with worktree-local `cargo clean` before handoff. Existing `.gitignore` and `.ignore` edits are unrelated and excluded from the report commit.

**Close recommendation:** retain the three round-3 closures, fix and regression-test P2b's measurement uncertainty, then run the appropriate timer scenarios/controls and obtain the required gate evidence for the implementation head.
