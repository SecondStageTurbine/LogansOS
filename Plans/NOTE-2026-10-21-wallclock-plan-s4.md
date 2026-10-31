<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: the wall-clock rung (RTC epoch + Atrium time) — S4's plan

**Author:** S4 (Claude). **Date:** 2026-10-21. **Branch:** `claude/wallclock-rtc-atrium` (from `origin/main`
`d4244be5`). **Thread:** OT-85 (OT-83/OT-84 are claimed on the in-flight `claude/ot61-c05-std-overlay`).
**Not for merge by S4:** the PL reserves the merge.

## What exists (verified on `d4244be5`)

- `SYS_CLOCK_GETTIME` 0x43, `CLOCK_MONOTONIC` 1 / `CLOCK_REALTIME` 2, `sys_clock_gettime_impl`
  (`kernel/src/arch/x86_64/syscall.rs:6268-6285`): both clocks = `TICKS * 10 ms`. Compiled under
  `any(browser-runtime, atrium-pointer-forward, desktop-session)`.
- No CMOS/MC146818 code. `kernel/src/acpi` is `vtd`-gated and parses only the DMAR.
- PR #84 (`claude/lapic-timer`) also edits `kernel/src/acpi/mod.rs` (FADT PM timer, `read_table`) and the
  `mod acpi` / `RSDP_REQUEST` cfg lines in `main.rs`. Both branches widen the same cfg line; the conflict is one
  line either way. I do not touch `timer.rs`, `scheduler.rs`, `apic.rs` or anything else #84 owns.
- The intro card (`neo_atrium::intro`) has no ticks-since-boot *row*: the clock concession in
  NOTE-2026-10-20 is the greeting word itself ("Good day", §3 Q3). This rung replaces that word and adds one
  date/time row; the superseding note says so.

## Pieces (each with its kill-switch)

1. **Kernel seed** — feature `rtc-epoch` (in the shipping `desktop` set; not in `default`).
   - `arch/x86_64/rtc_logic.rs` (pure, host-KAT'd by `tools/rtc-logic-tests`): BCD/binary decode, 12/24 h
     (PM bit 7), status-D VRT, field ranges, century (register or 2000+yy), `days_from_civil`, the bounded
     "two consecutive equal snapshots" loop over a reader closure.
   - `arch/x86_64/rtc.rs`: ports 0x70/0x71 (NMI bit left clear), UIP wait (bounded), write-once
     `REALTIME_BASE_NS` + state; `realtime_base_ns()` is a leaf atomic load.
   - `acpi/rtc_fadt.rs` + `rtc_fadt_logic.rs`: FADT `CENTURY` (byte 108, ignored when HW_REDUCED_ACPI) and
     `IAPC_BOOT_ARCH` bit 5 "CMOS RTC Not Present" (byte 109, rev ≥ 2). Child module of `acpi` so it reuses the
     private RSDP/XSDT walker without editing #84's lines.
   - Seed once after `mm::init` (beside `acpi::init`). `CLOCK_REALTIME = monotonic + base`; monotonic's code is
     untouched. Fallback (invalid fields, VRT clear, UIP stuck, never-agreeing reads, FADT says no RTC): base
     stays 0 = today's boot-relative realtime, one serial line, boot continues.
   - Test hook `rtc-epoch-force-invalid`: the reader returns an all-zero register file, so the real validator
     takes the fallback.
2. **libneo civil** — `libneo::civil` (no alloc, no syscall): `civil_from_unix_seconds`, weekday, 12 h,
   fixed-buffer `DateText`/`TimeText`. Host KATs with hand-computed values (epoch, 2000-02-29, 2100-03-01,
   month ends, 2026-10-21T17:48 = 1 792 604 880).
3. **Atrium** — neo-atrium `clock` module (pure words: greeting by hour, the card row) + `BaseScene` readout
   (two text lines on the backdrop above the intent field, no panel, masked like the labels). neo-init
   feature `desktop-clock` (in the product set): Home readout, time-aware intro greeting and date row, minute
   refresh. Off ⇒ today's Home / intro bit for bit (every existing desktop scenario is the regression).
   - Refresh: the Home loop's park deadline becomes the next minute boundary (was ∞); on wake, repaint only the
     readout box (and the card row if the intro is up) when the minute changed. No new thread, no per-second
     work. Without `desktop-park` the loop already spins; the minute check rides it.

## Gate

- `wallclock_rtc` (kernel `rtc-epoch` + `browser-runtime`, QEMU `-rtc base=2026-10-21T17:48:00`): neo-init
  probe formats realtime with `libneo::civil`; asserts the boot instant (realtime − monotonic) reads Wednesday,
  October 21, 2026, 5:48 PM, the base is stable within one tick across reads, monotonic nondecreasing. xtask
  checks the kernel's seeded wall time against the pinned base with its own (independent) civil arithmetic.
- `wallclock_rtc_fallback`: + `rtc-epoch-force-invalid`; boots, realtime == monotonic ±1 tick, exactly one
  fallback line. Control = `wallclock_rtc`.
- `vm_primitives` gains realtime == monotonic ±1 tick (the `rtc-epoch`-off regression).
- `desktop_clock_boot` (desktop-intro + desktop-clock, pinned RTC): the WM logs the readout and the card row;
  same date as the probe; xtask oracle-checks the text against the logged realtime; the kernel readback digest
  of the final Home with the readout box masked must equal the intro build's (the clock adds nothing outside
  its box).
- Host: persona KAT over every new line and greeting variant; civil KATs; rtc/FADT logic KATs.

Then the close-gate script, findings doc, OT-85 + follow-ups, the cross-family review checklist, PR.
