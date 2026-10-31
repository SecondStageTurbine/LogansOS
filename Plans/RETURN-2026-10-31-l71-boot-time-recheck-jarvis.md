<!-- SPDX-License-Identifier: CC-BY-4.0 -->
Relay 222 / Jarvis. Section 217 re-queued re-check of PR #254.

Reviewed fetched head `39389ac7c67d70d88de562915f479b5f9fb0ea62` using
`git fetch origin +pull/254/head:pr254` and `git diff origin/main...pr254`.
References below are to that head. The fetched head supersedes the dispatch's
`040d0e62`. Delivery branch is
`RETURN-2026-10-31-l71-boot-time-recheck-jarvis-relay222`, created from
`origin/main` (`ba12be7d`); the unsuffixed RETURN branch already exists remotely.
Overall: FAIL.

Validation: extracted the exact PR host-test source, shared logic and binary
FPDT fixture to a temporary directory, compiled with `rustc --edition=2024
--test`, and ran all 10 tests successfully. A separate host probe importing
the same unchanged logic reproduced the three cases below. No kernel/QEMU
scenario or physical boot was run. This commit delivers only this RETURN.

- FAIL - Item 1: the power-on origin is still neither established nor
  honestly qualified, and out-of-order times are not refused.
  `kernel/src/boot_time_logic.rs:68` accepts a power-on origin solely when
  entry milliseconds are at least firmware handoff milliseconds (`:73`).
  This is a one-sided consistency check, not evidence of a common epoch.
  `:355` then prints `From power-on` without marking that assumption.
  Concrete input: rate 1,000,000,000 Hz, firmware EBS exit 4,000,000,000 ns,
  entry/model/desktop/Home TSC readings 6,000,000,000 / 7,000,000,000 /
  8,000,000,000 / 9,000,000,000. A counter advanced by 2 s satisfies this
  check just as an unadjusted counter would; it prints
  `From power-on: firmware 4.00 s, model ready 7.00 s, desktop 8.00 s, Home 9.00 s.`
  There is no offset measurement or origin adjustment.
  `:391`, `:396` and `:401` print subsequent stamps without ordering checks.
  With the same rate and firmware, entry/model/desktop/Home readings
  6,000,000,000 / 3,000,000,000 / 2,000,000,000 / 1,000,000,000 are accepted:
  `From power-on: firmware 4.00 s, model ready 3.00 s, desktop 2.00 s, Home 1.00 s.`
  Even the entry/handoff check loses sub-ms ordering: EBS exit
  4,000,900,000 ns and entry 4,000,100,000 ticks at 1 GHz both truncate to
  4000 ms and return `power_on_origin=true`. The original before-handoff
  fixture now correctly says `Since TSC reset`, but these cases remain open.

- PASS - Item 4: an unrecorded firmware handoff is omitted, and an estimated
  TSC rate is marked. `kernel/src/boot_time_logic.rs:60` filters zero EBS
  exit before conversion; `:313` prints `firmware handoff not recorded`
  in the kernel. Summaries (`:386`) and milestones (`:428`) omit that
  unrecorded firmware duration, rather than displaying `firmware 0.00 s`.
  `kernel/src/boot_time.rs:87` propagates the CPUID-0x16 estimate indicator;
  the record encoder/decoder preserve flag bit 1, and kernel output
  (`kernel/src/boot_time_logic.rs:338`), summary origin (`:364`) and
  milestone output (`:420`) mark the estimate. The zero-handoff,
  estimate encode/decode and all three display tests pass.

- PASS - Item 2, only the requested no-worse confirmation: the re-check
  fixes leave the previously reported kernel-wide bad-pointer issue
  unchanged. `kernel/src/arch/x86_64/syscall.rs:7522` still checks length
  80, performs the same numerical range validation (`:7527`), and uses
  the same raw copyout (`:7533`). Its function body is unchanged from
  the original reviewed `04bddd7a` head; the estimate flag occupies the
  same initialized 80-byte record and adds no copyout or length.
  This does not close the bad-pointer finding: an unmapped user-half
  address such as `SYS_BOOT_TIMES(0x0000400000000000, 80)` still reaches
  the unsafe copy. That existing kernel-wide issue remains in the separate
  lane specified by Section 217.
