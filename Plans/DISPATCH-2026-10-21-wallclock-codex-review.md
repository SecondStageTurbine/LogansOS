<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family review of the wall-clock rung (OT-85)

**For:** a Codex-family seat (the PM relays; suggested GPT-6-Luna, high, since the scope is contained).
**Writer:** S4 (Claude), so the reviewer must be from a different family (CLAUDE.md §Cross-model diversity).
**Branch:** `claude/wallclock-rtc-atrium` (the PR is titled "[DO NOT MERGE: PL decision]").
**Read:** `git fetch origin && git show origin/claude/wallclock-rtc-atrium:<path>`. The diff is
`git diff origin/main...origin/claude/wallclock-rtc-atrium`.
**Context:** `docs/RESEARCH-2026-10-21-wallclock-FINDINGS.md` and `Plans/NOTE-2026-10-21-wallclock-plan-s4.md` on
the same branch.

**How to answer.** For each item:
- state **PASS** or **FAIL**;
- give the enforcing line (`file:line`), or the line that breaks it;
- for a FAIL, add a concrete input or interleaving that shows it.

Default to suspicion. A PASS must say what you checked. An uncertain finding is a note, not a FAIL.

**Delivery.** Commit your answers as `Plans/RETURN-2026-10-21-wallclock-codex-review.md` on the same branch and
push. A review delivered only in chat does not exist (CLAUDE.md §"Work is only real when it is on origin").

## 1. RTC read protocol (`kernel/src/arch/x86_64/rtc.rs`, `rtc_logic.rs`)

- [ ] **1a UIP.** No snapshot is taken while register A bit 7 (UIP) is set. The UIP wait is bounded, and a stuck
  UIP becomes the `uip-stuck` fallback, never a hang. (`rtc.rs` `snapshot`, the `UIP_POLLS` loop.)
- [ ] **1b Double-read agreement.** A seed uses only two consecutive identical snapshots. Disagreeing pairs
  retry, bounded by `MAX_READS`, after which the fallback is `unstable`. (`rtc_logic.rs` `read_stable`; KATs
  `an_update_between_reads_takes_another_pair`, `never_agreeing_reads_are_unstable`.) Can an update cycle land
  inside one snapshot so that two consecutive snapshots are equal *and* wrong?
- [ ] **1c BCD vs binary.** Register B bit 2 selects the decode. BCD nibbles above 9 are refused (`bad-bcd`).
  (`rtc_logic.rs` `field`.)
- [ ] **1d 12/24-hour.** Register B bit 1. In 12-hour mode, bit 7 of the hour is PM in both data modes, 12 AM
  maps to 0, 12 PM maps to 12, and 0 or 13 are refused. (`rtc_logic.rs` `hour24`; KAT
  `twelve_hour_mode_uses_bit_seven_for_pm`.)
- [ ] **1e Century.**
  - The FADT `CENTURY` index (byte 108) is used, and ignored when `HW_REDUCED_ACPI` is set.
  - A century byte is used only if it reads 19..=21; otherwise the year is 2000 + yy (the unmaintained-byte trap).
  - (`acpi/rtc_fadt_logic.rs` `parse`; `rtc_logic.rs` `decode`; KATs `an_unmaintained_century_byte_is_ignored`,
    `fadt_*`.)
- [ ] **1f Validity.** Register D VRT clear means `power-lost`. Every field is range-checked, and non-existent
  dates (Feb 29 of common years, Apr 31) are refused. The year must be in 1970..=2199, so realtime in ns stays a
  positive `i64`. (`rtc_logic.rs` `decode`, `MAX_YEAR`.)
- [ ] **1g Port discipline.** The index write leaves the NMI-disable bit clear (`reg & 0x7F`). The read runs with
  interrupts off, so no interleaved index write is possible. Nothing else in the kernel touches 0x70/0x71.
  Confirm with grep.

## 2. Days-from-civil correctness

- [ ] **2a** `rtc_logic::days_from_civil`, the kernel side (the seed), against the hand-computed KATs: epoch = 0;
  2000-01-01 = 10 957; 2000-02-29 exists; 2100-02-28 → 2100-03-01; 2026-10-21 = 20 747; 17:48 = 1 792 604 880 s.
- [ ] **2b** `libneo::civil::civil_from_days`, the display side, against the same values. Check the weekday
  ((days + 4) mod 7, with 1970-01-01 a Thursday) and the 12-hour text (12:00 AM / 12:00 PM).
- [ ] **2c** The kernel↔libneo round trip over every day 1970..=2199
  (`kernel_and_libneo_calendars_round_trip`). Two implementations from the same algorithm family could share a
  bug. Is the hand-computed set enough to break that? Independently, xtask `civil_oracle` walks years then
  months and re-derives every value the run logs.

## 3. The fallback

- [ ] **3a** Every failure path (FADT "CMOS RTC Not Present" bit 5, UIP stuck, unstable, power lost, bad BCD,
  out of range, year out of range) leaves `STATE != SEEDED`, so `realtime_base_ns()` returns 0 (boot-relative).
  Each logs exactly one line and returns; none panics or blocks. (`rtc.rs` `seed` / `fallback`.)
- [ ] **3b** `rtc-epoch-force-invalid` exercises the real validator, not a short-cut: every register reads 0xFF,
  and `decode` refuses it. The `wallclock_rtc_fallback` scenario asserts one fallback line, no seed, and realtime
  == monotonic. Its control is `wallclock_rtc`.
- [ ] **3c** The WM never shows a boot-relative "1970" clock: no readout and the "Good day" intro when realtime −
  monotonic == 0. (`neo-init/src/desktop_session.rs` `clock::start`.)

## 4. Monotonic isolation

- [ ] **4a** The `CLOCK_MONOTONIC` path in `sys_clock_gettime_impl` reads only `TICKS`. The seed's state
  (`BASE_NS`, `STATE`) is read only on the `CLOCK_REALTIME` branch. (`syscall.rs`, the `#[cfg(feature =
  "rtc-epoch")]` block.)
- [ ] **4b** Write-once:
  - `seed` claims with `compare_exchange(UNSEEDED → CLAIMED)`; a second call returns.
  - `BASE_NS` is stored before the `Release` store of `SEEDED`, and `realtime_base_ns` loads `STATE` with
    `Acquire` before `BASE_NS` (CP-11 / CP-17).
  - Nothing else writes either static.
- [ ] **4c** With `rtc-epoch` off, the kernel is C1.0's: realtime == monotonic (the `vm_primitives` regression,
  check 5). The FADT is not read, and the RTC is never touched.
- [ ] **4d** I-14: no governance lock is taken. Seed and query are lock-free. Confirm with grep.

## 5. The persona lines (`userland/neo-atrium/src/clock.rs`, `intro.rs`, `tests.rs`)

- [ ] **5a** Every new line passes the persona rules: ASCII only, no `!`, no emoji, ≤ 80 bytes, fits the card at
  15 px. The greeting row is at most 63 bytes and the name row is 77. No KAT bound was relaxed.
  (`clock_persona_kat_every_variant_every_hour`.)
- [ ] **5b** The greeting word matches the stated boundaries (05–11 morning, 12–17 afternoon, else evening) at
  every hour (`clock_greeting_hour_table`). Only row 1 carries a time-of-day word.
- [ ] **5c** The clock-less intro is word-for-word NOTE-2026-10-20's (`desktop_intro_boot` still passes against
  its unchanged fixture).

## 6. The minute refresh and the no-wakeup claim

- [ ] **6a** With `desktop-park`, Home's park deadline is `clock.deadline()`, the monotonic instant of the next
  minute boundary. Without a readout it is `u64::MAX`, as before. Is the boundary arithmetic
  (`mono + 60 s − realtime mod 60 s`) right at an exact boundary?
- [ ] **6b** A wake repaints only `layout.clock_slot()`, and the card slot only while the intro shows. It
  repaints only when the minute changed; an early wake moves the deadline and draws nothing.
  (`desktop_session.rs` `clock::tick`.)
- [ ] **6c** No new thread, timer, or per-second work. Without `desktop-park` (the current product WM spins),
  `tick` adds one monotonic read per spin turn and reads realtime only past the deadline. Is that honestly "no
  busy polling", given the loop already spins on input?
- [ ] **6d** Coverage gap (stated, not a FAIL by itself): no scenario runs across a real minute boundary. The
  repaint-equals-full-paint property is host-KAT'd (`clock_off_is_home_bit_for_bit_and_on_draws_only_in_its_box`).
  Would you require a gate scenario for the live minute tick?
