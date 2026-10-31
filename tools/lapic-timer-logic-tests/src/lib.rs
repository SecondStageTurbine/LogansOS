// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/arch/x86_64/lapic_timer_logic.rs`: TSC rate from
//! CPUID 0x15/0x16, measured rates, and the tick plan. The real module is
//! mounted through `#[path]`.

#![allow(dead_code)]

#[path = "../../../kernel/src/arch/x86_64/lapic_timer_logic.rs"]
mod lapic_timer_logic;

#[cfg(test)]
mod tests {
    use super::lapic_timer_logic::{
        CROSS_CHECK_PERCENT, DIVIDE_BY_1, LVT_MASKED, LVT_PERIODIC, LVT_TSC_DEADLINE, Plan, Rate,
        RateChoice, RateSource, TSC_HZ_BOUND, budget_tsc_hz, choose_rate, first_tick_budget,
        periodic_initial_count, plan, rate_per_second, tsc_hz_from_leaf15, tsc_hz_from_leaf16,
        within_percent,
    };

    /// SDM register encodings, by hand.
    #[test]
    fn lvt_and_divide_encodings() {
        assert_eq!(LVT_PERIODIC, 0x0002_0000);
        assert_eq!(LVT_TSC_DEADLINE, 0x0004_0000);
        assert_eq!(LVT_MASKED, 0x0001_0000);
        assert_eq!(DIVIDE_BY_1, 0b1011);
    }

    /// A 38.4 MHz crystal with ratio 2 : 125 = 2.4 GHz TSC; zeros mean
    /// "not enumerated".
    #[test]
    fn leaf15_and_leaf16() {
        assert_eq!(tsc_hz_from_leaf15(2, 125, 38_400_000), Some(2_400_000_000));
        assert_eq!(tsc_hz_from_leaf15(0, 125, 38_400_000), None);
        assert_eq!(tsc_hz_from_leaf15(2, 125, 0), None);
        assert_eq!(tsc_hz_from_leaf16(2_400), Some(2_400_000_000));
        assert_eq!(tsc_hz_from_leaf16(0), None);
    }

    #[test]
    fn periodic_count_fits_32_bits() {
        assert_eq!(periodic_initial_count(1_000_000_000, 100), Some(10_000_000));
        assert_eq!(
            periodic_initial_count(1_000_000_000_000, 100),
            None,
            "10^10 > u32"
        );
        assert_eq!(periodic_initial_count(50, 100), None, "zero period");
    }

    #[test]
    fn plan_prefers_deadline_then_periodic_then_pit() {
        let tsc = Some(2_400_000_000);
        assert_eq!(
            plan(true, tsc, None, 100),
            Plan::TscDeadline { period: 24_000_000 }
        );
        // No TSC-deadline: periodic from the LAPIC rate.
        assert_eq!(
            plan(false, tsc, Some(100_000_000), 100),
            Plan::Periodic {
                initial_count: 1_000_000
            }
        );
        // Deadline flag but no TSC rate: periodic if the LAPIC rate is known.
        assert_eq!(
            plan(true, None, Some(100_000_000), 100),
            Plan::Periodic {
                initial_count: 1_000_000
            }
        );
        // Nothing calibrated: keep the PIT.
        assert_eq!(plan(true, None, None, 100), Plan::Pit);
        assert_eq!(plan(false, tsc, None, 100), Plan::Pit);
    }

    /// A dead mode demotes: deadline -> periodic -> PIT.
    #[test]
    fn demotion_chain() {
        let tsc = Some(2_400_000_000);
        let lapic = Some(100_000_000);
        assert!(matches!(
            plan(true, tsc, lapic, 100),
            Plan::TscDeadline { .. }
        ));
        assert_eq!(
            plan(false, tsc, lapic, 100),
            Plan::Periodic {
                initial_count: 1_000_000
            }
        );
        assert_eq!(plan(false, tsc, None, 100), Plan::Pit);
    }

    #[test]
    fn first_tick_budget_is_five_periods() {
        assert_eq!(first_tick_budget(2_400_000_000, 100), 120_000_000);
        assert_eq!(first_tick_budget(TSC_HZ_BOUND, 100), 500_000_000);
        assert_eq!(first_tick_budget(TSC_HZ_BOUND, 0), TSC_HZ_BOUND);
    }

    /// Jarvis R1: CPUID 0x15 reports (1, 1, 24 MHz), a TSC of 24 MHz, but
    /// the TSC really runs at 2.4 GHz, which the HPET measures. The LAPIC
    /// really runs at the 24 MHz crystal, which the HPET agrees with, so
    /// periodic mode is calibrated. The refused TSC rate must not size the
    /// waits for the periodic tick's first tick or for the PIT fallback.
    #[test]
    fn a_refused_tsc_rate_never_sizes_a_wait() {
        let actual_tsc = 2_400_000_000u64;
        let tsc = choose_rate(
            tsc_hz_from_leaf15(1, 1, 24_000_000),
            &[r(actual_tsc, RateSource::Hpet)],
            None,
        );
        assert!(matches!(tsc, RateChoice::Mismatch { .. }), "{tsc:?}");
        let lapic = choose_rate(Some(24_000_000), &[r(24_000_000, RateSource::Hpet)], None);
        assert!(matches!(lapic, RateChoice::Calibrated { .. }), "{lapic:?}");
        let initial = periodic_initial_count(24_000_000, 100).unwrap();
        assert_eq!(initial, 240_000);
        // The first periodic tick comes after one period: 10 ms of real TSC.
        let first_tick_cycles = actual_tsc / 100;

        // The old sizing, from the refused 24 MHz: 1.2M cycles, 0.5 ms of
        // the real TSC, shorter than the 10 ms first tick. A working
        // periodic tick was disarmed.
        assert_eq!(first_tick_budget(24_000_000, 100), 1_200_000);
        assert!(first_tick_budget(24_000_000, 100) < first_tick_cycles);

        // Now: the bound, five periods at 10 GHz, which outlasts the first
        // tick at any real rate up to the bound.
        let budget_hz = budget_tsc_hz(&tsc);
        assert_eq!(budget_hz, TSC_HZ_BOUND);
        let first = first_tick_budget(budget_hz, 100);
        assert!(
            first >= 5 * first_tick_cycles,
            "{first} vs {first_tick_cycles}"
        );
        // The PIT fallback waits half a second at that rate: at least 0.5 s
        // of the real TSC, fifty PIT periods.
        assert!(budget_hz / 2 >= actual_tsc / 2);
    }

    /// Jarvis R1: only a calibrated rate sizes a wait; an estimate, a
    /// refusal or nothing gives the bound.
    #[test]
    fn budget_rate_by_outcome() {
        let hpet = r(2_000_000_000, RateSource::Hpet);
        assert_eq!(
            budget_tsc_hz(&RateChoice::Calibrated {
                rate: hpet,
                checked: None
            }),
            2_000_000_000
        );
        assert_eq!(
            budget_tsc_hz(&RateChoice::Estimated(r(
                2_400_000_000,
                RateSource::Leaf16Estimate
            ))),
            TSC_HZ_BOUND,
            "an estimate is not trusted to size a wait"
        );
        assert_eq!(
            budget_tsc_hz(&RateChoice::Mismatch {
                primary: r(24_000_000, RateSource::Leaf15),
                check: hpet
            }),
            TSC_HZ_BOUND
        );
        assert_eq!(budget_tsc_hz(&RateChoice::Unknown), TSC_HZ_BOUND);
        assert_eq!(
            budget_tsc_hz(&RateChoice::Calibrated {
                rate: r(0, RateSource::Hpet),
                checked: None
            }),
            TSC_HZ_BOUND
        );
    }

    const fn r(hz: u64, source: RateSource) -> Rate {
        Rate { hz, source }
    }

    /// Jarvis P2-a: the selection order and the 5% cross-check.
    #[test]
    fn rate_selection_and_cross_check() {
        let hpet = r(2_400_000_000, RateSource::Hpet);
        let pm = r(2_401_000_000, RateSource::PmTimer);
        // CPUID 0x15 is primary, checked against the best measurement.
        assert_eq!(
            choose_rate(Some(2_400_000_000), &[hpet, pm], Some(2_000_000_000)),
            RateChoice::Calibrated {
                rate: r(2_400_000_000, RateSource::Leaf15),
                checked: Some(hpet)
            }
        );
        // No 0x15: the HPET measurement is primary, checked by the PM timer.
        assert_eq!(
            choose_rate(None, &[hpet, pm], None),
            RateChoice::Calibrated {
                rate: hpet,
                checked: Some(pm)
            }
        );
        // One measurement only: calibrated, unchecked.
        assert_eq!(
            choose_rate(None, &[hpet], Some(1)),
            RateChoice::Calibrated {
                rate: hpet,
                checked: None
            }
        );
        // Jarvis's example: 0x16 says 2.4 GHz, the TSC really runs 2.0 GHz.
        // With a reference clock the 0x16 value is never used.
        let real = r(2_000_000_000, RateSource::Hpet);
        assert_eq!(
            choose_rate(None, &[real], Some(2_400_000_000)),
            RateChoice::Calibrated {
                rate: real,
                checked: None
            }
        );
        // Without one it is an estimate, never "calibrated".
        assert_eq!(
            choose_rate(None, &[], Some(2_400_000_000)),
            RateChoice::Estimated(r(2_400_000_000, RateSource::Leaf16Estimate))
        );
        assert_eq!(choose_rate(None, &[], None), RateChoice::Unknown);
    }

    #[test]
    fn a_disagreement_beyond_5_percent_is_refused() {
        assert_eq!(CROSS_CHECK_PERCENT, 5);
        let hpet = r(2_000_000_000, RateSource::Hpet);
        assert_eq!(
            choose_rate(Some(2_400_000_000), &[hpet], None),
            RateChoice::Mismatch {
                primary: r(2_400_000_000, RateSource::Leaf15),
                check: hpet
            }
        );
        // 4.9% apart is accepted; 5.1% is not.
        assert!(within_percent(1_049_000_000, 1_000_000_000, 5));
        assert!(!within_percent(1_051_000_000, 1_000_000_000, 5));
        assert!(within_percent(951_000_000, 1_000_000_000, 5));
        assert!(!within_percent(949_000_000, 1_000_000_000, 5));
    }

    #[test]
    fn measured_rate() {
        // 24,000,000 TSC cycles in 10 ms = 2.4 GHz.
        assert_eq!(rate_per_second(24_000_000, 10_000_000), Some(2_400_000_000));
        assert_eq!(rate_per_second(0, 10_000_000), None);
        assert_eq!(rate_per_second(5, 0), None);
    }

    /// #84 round 4 (R4-P2b): the PIT is never the only calibration source.
    /// Jarvis's case: the poller is held off across terminal count, so a
    /// 20 ms window reads as 10 ms and the PIT-measured rate is 2x. Alone it
    /// is `PitOnly` and the tick is the PIT's own; as the checker against a
    /// good HPET or CPUID 0x15 it is a mismatch, refused.
    #[test]
    fn the_pit_never_calibrates_alone() {
        let real = 1_000_000_000u64;
        let pit_2x = r(2 * real, RateSource::Pit);
        // Sole source: never Calibrated, and the plan is the PIT tick.
        let alone = choose_rate(None, &[pit_2x], None);
        assert_eq!(alone, RateChoice::PitOnly(pit_2x));
        assert_eq!(budget_tsc_hz(&alone), TSC_HZ_BOUND);
        assert_eq!(plan(true, None, None, 100), Plan::Pit);
        assert_eq!(plan(false, None, None, 100), Plan::Pit);
        // Even an accurate PIT sample alone is not trusted.
        assert!(matches!(
            choose_rate(None, &[r(real, RateSource::Pit)], Some(real)),
            RateChoice::PitOnly(_)
        ));
        // As the checker: the HPET is primary, the bad PIT disputes it.
        let hpet = r(real, RateSource::Hpet);
        assert_eq!(
            choose_rate(None, &[hpet, pit_2x], None),
            RateChoice::Mismatch {
                primary: hpet,
                check: pit_2x
            }
        );
        // The PIT listed first still never leads.
        assert_eq!(
            choose_rate(None, &[pit_2x, hpet], None),
            RateChoice::Mismatch {
                primary: hpet,
                check: pit_2x
            }
        );
        // CPUID 0x15 primary, the bad PIT disputes it too.
        assert!(matches!(
            choose_rate(Some(real), &[pit_2x], None),
            RateChoice::Mismatch { .. }
        ));
        // A good PIT confirms the HPET.
        assert!(matches!(
            choose_rate(None, &[hpet, r(real + real / 100, RateSource::Pit)], None),
            RateChoice::Calibrated {
                checked: Some(_),
                ..
            }
        ));
    }
}
