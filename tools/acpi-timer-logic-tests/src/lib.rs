// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/acpi/timers_logic.rs`: locating the ACPI PM
//! timer and the HPET from hand-laid FADT / HPET table bytes, taking the
//! HPET over from firmware (register images), and the PIT channel-2
//! one-shot's completion test (port 0x61 traces). The real module is
//! mounted through `#[path]`.

#![allow(dead_code)]

#[path = "../../../kernel/src/acpi/timers_logic.rs"]
mod timers_logic;

#[cfg(test)]
mod tests {
    use super::timers_logic::{
        HPET_CONFIG, HPET_ENABLE_CNF, HPET_INT_STATUS, HPET_LEG_RT_CNF, HPET_TN_INT_ENB_CNF,
        HW_REDUCED_ACPI, HpetRegs, HpetTakeover, PmTimer, fadt_pm_timer, hpet_base, hpet_caps,
        hpet_comparators, hpet_hz, hpet_take_ownership, hpet_timer_config, pit_one_shot_completed,
    };
    use std::collections::BTreeMap;

    /// A FADT laid out by hand (ACPI 6.5 §5.2.9): 276 bytes, PM_TMR_BLK at
    /// 76, PM_TMR_LEN at 91, flags at 112, X_PM_TMR_BLK at 208.
    fn fadt(pm_port: u32, pm_len: u8, flags: u32, x_space: u8, x_addr: u64) -> Vec<u8> {
        let mut f = vec![0u8; 276];
        f[..4].copy_from_slice(b"FACP");
        f[76..80].copy_from_slice(&pm_port.to_le_bytes());
        f[91] = pm_len;
        f[112..116].copy_from_slice(&flags.to_le_bytes());
        f[208] = x_space;
        f[212..220].copy_from_slice(&x_addr.to_le_bytes());
        f
    }

    #[test]
    fn pm_timer_from_the_legacy_block() {
        // QEMU q35 (ICH9): PM base 0x600, timer at +8, TMR_VAL_EXT set.
        assert_eq!(
            fadt_pm_timer(&fadt(0x608, 4, 1 << 8, 0, 0)),
            Some(PmTimer {
                port: 0x608,
                wide: true
            })
        );
        assert_eq!(
            fadt_pm_timer(&fadt(0x408, 4, 0, 0, 0)),
            Some(PmTimer {
                port: 0x408,
                wide: false
            }),
            "24-bit counter"
        );
    }

    #[test]
    fn pm_timer_prefers_the_extended_block() {
        assert_eq!(
            fadt_pm_timer(&fadt(0x408, 4, 0, 1, 0x1808)),
            Some(PmTimer {
                port: 0x1808,
                wide: false
            })
        );
        // An X block in memory space is not a port: fall back.
        assert_eq!(
            fadt_pm_timer(&fadt(0x408, 4, 0, 0, 0xfed0_0000)).map(|t| t.port),
            Some(0x408)
        );
    }

    #[test]
    fn no_pm_timer() {
        assert_eq!(
            fadt_pm_timer(&fadt(0, 0, 0, 0, 0)),
            None,
            "hardware-reduced"
        );
        assert_eq!(
            fadt_pm_timer(&fadt(0x408, 0, 0, 0, 0)),
            None,
            "PM_TMR_LEN 0"
        );
        let mut bad = fadt(0x408, 4, 0, 0, 0);
        bad[..4].copy_from_slice(b"APIC");
        assert_eq!(fadt_pm_timer(&bad), None);
        // A short (ACPI 1.0) FADT still yields its legacy block.
        assert_eq!(
            fadt_pm_timer(&fadt(0x408, 4, 0, 0, 0)[..116]).map(|t| t.port),
            Some(0x408)
        );
    }

    /// The HPET table (HPET spec 1.0a §3.2.4): the block's generic address at
    /// 40, its 64-bit address at 44.
    #[test]
    fn hpet_table() {
        let mut t = vec![0u8; 56];
        t[..4].copy_from_slice(b"HPET");
        t[44..52].copy_from_slice(&0xfed0_0000u64.to_le_bytes());
        assert_eq!(hpet_base(&t), Some(0xfed0_0000));
        t[40] = 1; // I/O space: not supported
        assert_eq!(hpet_base(&t), None);
    }

    #[test]
    fn hpet_capabilities() {
        // QEMU's HPET: 10 ns period (10_000_000 fs), 64-bit counter.
        let caps = (10_000_000u64 << 32) | (1 << 13) | 0x8086_a201;
        assert_eq!(hpet_caps(caps), Some((10_000_000, true)));
        assert_eq!(hpet_hz(10_000_000), 100_000_000);
        // Intel PCH HPET: 69.841279 ns period, ~14.31818 MHz.
        assert_eq!(
            hpet_hz(69_841_279),
            14_318_179,
            "integer division of 10^15 fs"
        );
        assert_eq!(hpet_caps(0), None);
        assert_eq!(hpet_caps(200_000_000u64 << 32), None, "period above 100 ns");
    }

    /// A rev-6 FADT as firmware lays it out: 276 bytes, length and revision
    /// in the header, a valid checksum.
    fn fadt_rev6(pm_port: u32, flags: u32, x_addr: u64) -> Vec<u8> {
        let mut f = fadt(pm_port, 4, flags, if x_addr != 0 { 1 } else { 0 }, x_addr);
        f[4..8].copy_from_slice(&276u32.to_le_bytes());
        f[8] = 6;
        f[209] = 32; // register bit width
        let sum = f.iter().fold(0u8, |a, b| a.wrapping_add(*b));
        f[9] = 0u8.wrapping_sub(sum);
        assert_eq!(f.iter().fold(0u8, |a, b| a.wrapping_add(*b)), 0);
        f
    }

    /// Jarvis R3 (ACPI 6.5 §5.2.9): with `HW_REDUCED_ACPI` (flags bit 20)
    /// both PM timer fields are to be ignored. His case: legacy 0x608,
    /// PM_TMR_LEN 4, extended system-I/O 0x1808, which the parser used to
    /// return as port 6152.
    #[test]
    fn hardware_reduced_has_no_pm_timer() {
        assert_eq!(HW_REDUCED_ACPI, 1 << 20);
        assert_eq!(
            fadt_pm_timer(&fadt_rev6(0x608, 1 << 20, 0x1808)),
            None,
            "extended form"
        );
        assert_eq!(
            fadt_pm_timer(&fadt_rev6(0x608, 1 << 20, 0)),
            None,
            "legacy form"
        );
        assert_eq!(
            fadt_pm_timer(&fadt_rev6(0x608, (1 << 20) | (1 << 8), 0x1808)),
            None,
            "with TMR_VAL_EXT"
        );
        // The same tables without the flag do name a PM timer.
        assert_eq!(
            fadt_pm_timer(&fadt_rev6(0x608, 0, 0x1808)),
            Some(PmTimer {
                port: 0x1808,
                wide: false
            })
        );
        assert_eq!(
            fadt_pm_timer(&fadt_rev6(0x608, 0, 0)),
            Some(PmTimer {
                port: 0x608,
                wide: false
            })
        );
    }

    /// An HPET register image, recording every write in order.
    struct Image {
        regs: BTreeMap<usize, u64>,
        writes: Vec<(usize, u64)>,
    }

    impl Image {
        fn new(config: u64, timers: &[u64], status: u64) -> Self {
            let mut regs = BTreeMap::new();
            regs.insert(HPET_CONFIG, config);
            regs.insert(HPET_INT_STATUS, status);
            for (n, t) in timers.iter().enumerate() {
                regs.insert(hpet_timer_config(n), *t);
            }
            Image {
                regs,
                writes: Vec::new(),
            }
        }
    }

    impl HpetRegs for Image {
        fn read(&mut self, offset: usize) -> u64 {
            *self
                .regs
                .get(&offset)
                .expect("read of an unmodelled register")
        }

        fn write(&mut self, offset: usize, value: u64) {
            assert!(self.regs.contains_key(&offset), "write to 0x{offset:x}");
            self.writes.push((offset, value));
            if offset == HPET_INT_STATUS {
                // Write 1 to clear.
                *self.regs.get_mut(&offset).unwrap() &= !value;
            } else {
                self.regs.insert(offset, value);
            }
        }
    }

    /// Caps for an HPET with `n` comparators and a 10 ns period.
    fn caps(n: u64) -> u64 {
        (10_000_000u64 << 32) | ((n - 1) << 8)
    }

    /// No write turns the counter on while legacy routing is selected or
    /// a comparator can interrupt: replay the writes and check the state
    /// at each one that sets ENABLE_CNF.
    fn assert_never_enabled_unsafely(start: &Image, img: &Image) {
        let mut regs = start.regs.clone();
        for &(at, v) in &img.writes {
            if at == HPET_INT_STATUS {
                *regs.get_mut(&at).unwrap() &= !v;
            } else {
                regs.insert(at, v);
            }
            if regs[&HPET_CONFIG] & HPET_ENABLE_CNF != 0 {
                assert_eq!(
                    regs[&HPET_CONFIG] & HPET_LEG_RT_CNF,
                    0,
                    "enabled with legacy routing"
                );
                for (&off, &t) in &regs {
                    if off >= 0x100 {
                        assert_eq!(
                            t & HPET_TN_INT_ENB_CNF,
                            0,
                            "enabled with comparator 0x{off:x} armed"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn hpet_comparator_count() {
        // QEMU's HPET: caps low word 0x8086_a201, NUM_TIM_CAP = 2: three.
        assert_eq!(hpet_comparators((10_000_000u64 << 32) | 0x8086_a201), 3);
        assert_eq!(hpet_comparators(caps(8)), 8);
        assert_eq!(hpet_comparators(0x1f00), 32);
    }

    /// Jarvis R2, his case: general config 0x2 (counter off, legacy
    /// replacement selected), comparator 0's interrupt enabled. Merely
    /// OR-ing in ENABLE_CNF gave config 0x3 and timer 0 at 0x4: comparator
    /// delivery on, legacy routing kept.
    #[test]
    fn hpet_takeover_from_legacy_with_an_armed_comparator() {
        let start = Image::new(0x2, &[0x4, 0x0, 0x0], 0);
        let mut img = Image::new(0x2, &[0x4, 0x0, 0x0], 0);
        let took = hpet_take_ownership(&mut img, caps(3));
        assert_eq!(
            took,
            HpetTakeover {
                was_enabled: false,
                was_legacy: true,
                comparators_quieted: 1
            }
        );
        assert_eq!(img.regs[&HPET_CONFIG], 0x1, "running, legacy routing off");
        assert_eq!(img.regs[&hpet_timer_config(0)], 0x0, "comparator 0 silent");
        assert_never_enabled_unsafely(&start, &img);
    }

    /// Already enabled with legacy routing (config 0x3), several armed
    /// comparators, a latched status: the counter is stopped first, other
    /// configuration bits are kept, the status is cleared.
    #[test]
    fn hpet_takeover_while_running() {
        // Comparator 0: periodic (bit 3) + level (bit 1) + int enable;
        // comparator 2: FSB (bit 14) + int enable; reserved high bits kept.
        let timers = [0x4 | 0x8 | 0x2, 0x0, 0x4 | (1 << 14), 0xff00_0000_0000_0000];
        let start = Image::new(0x3 | (1 << 40), &timers, 0b101);
        let mut img = Image::new(0x3 | (1 << 40), &timers, 0b101);
        let took = hpet_take_ownership(&mut img, caps(4));
        assert_eq!(
            took,
            HpetTakeover {
                was_enabled: true,
                was_legacy: true,
                comparators_quieted: 2
            }
        );
        assert_eq!(
            img.writes.first(),
            Some(&(HPET_CONFIG, 1 << 40)),
            "first, stop the counter and drop legacy routing"
        );
        assert_eq!(img.regs[&HPET_CONFIG], 0x1 | (1 << 40));
        assert_eq!(img.regs[&hpet_timer_config(0)], 0x8 | 0x2);
        assert_eq!(img.regs[&hpet_timer_config(1)], 0x0);
        assert_eq!(img.regs[&hpet_timer_config(2)], 1 << 14);
        assert_eq!(img.regs[&hpet_timer_config(3)], 0xff00_0000_0000_0000);
        // Status: comparator 0 is level-triggered, its bit is cleared;
        // comparator 2 is edge-triggered, its bit is written 0 (Jarvis
        // R2-P3), so it stays as the image reports it.
        assert!(
            img.writes.contains(&(HPET_INT_STATUS, 0b001)),
            "{:?}",
            img.writes
        );
        assert_eq!(img.regs[&HPET_INT_STATUS], 0b100);
        assert_never_enabled_unsafely(&start, &img);
    }

    /// Firmware left it clean: stop, start, nothing else written.
    #[test]
    fn hpet_takeover_clean() {
        let mut img = Image::new(0x0, &[0x0, 0x0, 0x0], 0);
        let took = hpet_take_ownership(&mut img, caps(3));
        assert_eq!(
            took,
            HpetTakeover {
                was_enabled: false,
                was_legacy: false,
                comparators_quieted: 0
            }
        );
        assert_eq!(img.writes, vec![(HPET_CONFIG, 0x0), (HPET_CONFIG, 0x1)]);
    }

    /// Run the PIT completion test over a port-0x61 trace, a clock that
    /// advances one unit per read of it.
    fn pit(trace: &[u8], low_timeout: u64, high_timeout: u64) -> (bool, usize) {
        let mut reads = 0usize;
        let mut clock = 0u64;
        let done = pit_one_shot_completed(
            &mut || {
                let v = trace.get(reads).copied().unwrap_or(*trace.last().unwrap());
                reads += 1;
                v
            },
            &mut || {
                clock += 1;
                clock
            },
            low_timeout,
            0,
            high_timeout,
            u64::MAX,
        );
        (done, reads)
    }

    /// Run the PIT completion test over (port 0x61 value, time of that
    /// read) pairs: the clock can jump between reads, as it does when the
    /// CPU is held off (an SMI, a VM exit). The call's start is time 0.
    fn pit_timed(events: &[(u8, u64)], low: u64, min: u64, high: u64) -> bool {
        pit_gapped(events, low, min, high, u64::MAX)
    }

    fn pit_gapped(events: &[(u8, u64)], low: u64, min: u64, high: u64, gap: u64) -> bool {
        let mut started = false;
        let at = |i: usize| events[i.min(events.len() - 1)];
        let reads_ref = std::cell::Cell::new(0usize);
        pit_one_shot_completed(
            &mut || {
                let i = reads_ref.get();
                reads_ref.set(i + 1);
                at(i).0
            },
            &mut || {
                if !started {
                    started = true;
                    return 0;
                }
                at(reads_ref.get().saturating_sub(1)).1
            },
            low,
            min,
            high,
            gap,
        )
    }

    /// Jarvis R4: OUT2 (port 0x61 bit 5) must go LOW, then HIGH.
    #[test]
    fn pit_one_shot_needs_both_edges() {
        // A real PIT: low at once, high at terminal count.
        assert!(pit(&[0x00, 0x00, 0x00, 0x20], 10, 1000).0);
        // Other port 0x61 bits do not matter.
        assert!(pit(&[0xdf, 0xdf, 0xff], 10, 1000).0);
        // Stuck HIGH (0xff, an absent or gated PIT): refused, and after the
        // short low timeout, not the long one.
        let (done, reads) = pit(&[0xff], 10, 1_000_000);
        assert!(!done, "constant 0xff is not a finished window");
        assert!(reads < 100, "{reads} reads: gave up at the low timeout");
        // Stuck LOW (0x00): refused at the high timeout.
        let (done, reads) = pit(&[0x00], 10, 1000);
        assert!(!done, "constant 0x00 is not a finished window");
        assert!(reads < 2000);
        // High, then low, then high: the low edge was seen; it completes.
        assert!(pit(&[0x20, 0x00, 0x20], 10, 1000).0);
        // Low, but high only after the high timeout: refused.
        let mut late = vec![0x00u8; 2000];
        late.push(0x20);
        assert!(!pit(&late, 10, 1000).0);
    }

    /// Jarvis R4, round 3: an edge first seen after its deadline is late,
    /// even on the read that sees it; a window far shorter than programmed
    /// is refused. (LOW by 10, HIGH between 500 and 1000.)
    #[test]
    fn pit_edges_are_timed_on_the_read_that_sees_them() {
        let (low, min, high) = (10, 500, 1000);
        // On time.
        assert!(pit_timed(
            &[(0x00, 1), (0x00, 600), (0x20, 700)],
            low,
            min,
            high
        ));
        // Late LOW: high until a jump to 100, then LOW, then HIGH.
        assert!(!pit_timed(
            &[(0x20, 1), (0x00, 100), (0x20, 700)],
            low,
            min,
            high
        ));
        // Late HIGH: LOW at once, then the first HIGH read at 5000.
        assert!(!pit_timed(&[(0x00, 1), (0x20, 5000)], low, min, high));
        // Too fast: HIGH at 50, a tenth of the window.
        assert!(!pit_timed(&[(0x00, 1), (0x20, 50)], low, min, high));
        // The bounds are inclusive.
        assert!(pit_timed(&[(0x00, 10), (0x20, 500)], low, min, high));
        assert!(pit_timed(&[(0x00, 1), (0x20, 1000)], low, min, high));
        assert!(!pit_timed(&[(0x00, 11), (0x20, 600)], low, min, high));
        assert!(!pit_timed(&[(0x00, 1), (0x20, 1001)], low, min, high));
    }

    /// Jarvis R2-P3 (HPET 1.0a §2.3.6): only a level-triggered comparator's
    /// status bit is written 1; an edge-triggered comparator's is written 0.
    #[test]
    fn hpet_status_write_back_is_level_only() {
        // Comparator 0 edge-triggered with its status set; comparator 1
        // level-triggered with its status set.
        let mut img = Image::new(0x0, &[0x0, 0x2, 0x0], 0b011);
        hpet_take_ownership(&mut img, caps(3));
        let status_writes: Vec<u64> = img
            .writes
            .iter()
            .filter(|(at, _)| *at == HPET_INT_STATUS)
            .map(|(_, v)| *v)
            .collect();
        assert_eq!(status_writes, vec![0b010]);
        // Only edge-triggered status set: no status write at all.
        let mut img = Image::new(0x0, &[0x0, 0x0, 0x0], 0b001);
        hpet_take_ownership(&mut img, caps(3));
        assert!(
            img.writes.iter().all(|(at, _)| *at != HPET_INT_STATUS),
            "{:?}",
            img.writes
        );
    }

    /// #84 round 4 (R4-P2b), defence in depth: Jarvis's trace. LOW is seen,
    /// the poller is held off across terminal count, and HIGH is first seen
    /// at 2x the window. Every bound passes, but the poll gap betrays it.
    #[test]
    fn pit_poll_held_off_across_the_edge_is_refused() {
        // Window 1000, min 500, high timeout 3000, max gap 100.
        let held = [(0x00, 1), (0x00, 50), (0x20, 2000)];
        assert!(
            pit_timed(&held, 10, 500, 3000),
            "the bounds alone accept it"
        );
        assert!(!pit_gapped(&held, 10, 500, 3000, 100));
        // Steady polling with the same edge time is a window.
        let mut steady: Vec<(u8, u64)> = vec![(0x00, 1)];
        steady.extend((1..20).map(|i| (0x00, i * 50)));
        steady.push((0x20, 1000));
        assert!(pit_gapped(&steady, 10, 500, 3000, 100));
    }
}
