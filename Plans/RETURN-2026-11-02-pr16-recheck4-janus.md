<!-- SPDX-License-Identifier: CC-BY-4.0 -->
Relay 277 / Janus: PR #16, item 1, re-check of relay 262.

Reviewed fetched head `4b53b40ea5d8a45d390b0e2a16d918166408ff91`
(`git fetch origin +pull/16/head:pr16`), the relay 262 RETURN on
`codex/pr16-recheck3-janus`, and the latest `PM:` comment
(2026-11-02 08:48:32 UTC). Source line references below name that head.
RETURN branch `codex/pr16-recheck4-janus` starts at `origin/main`,
`d92da64796be781854b708aba74db49cc6b6b163`.

- PASS — Earlier FAIL, item 1 (firmware backing validation).
  `kernel/src/boot_time_logic.rs:201` allows only ACPI reclaim (9) and
  ACPI NVS (10); `:206` ignores attributes and `:259` excludes every
  other descriptor before it can enter the captured regions. Thus the
  relay 262 input (48-byte descriptor, type 0, physical start
  `0x6b920000`, one page, attribute `0x8`, other fields zero) now yields
  no region, as does attribute `0x2008` (WB|RP). Type 0 at
  `0xfee00000` with attribute 0 or UC, and runtime data (6) with WB,
  are also rejected. The regression test at
  `tools/boot-time-logic-tests/src/lib.rs:499` exercises those cases
  and the accepted types 9 and 10. Capture uses this parser at
  `kernel/src/boot_time.rs:65`; the containment predicate checks the
  complete extent with overflow rejection at
  `kernel/src/boot_time_logic.rs:280`. Root/FPDT discovery receives it
  at `kernel/src/acpi/mod.rs:494`, with checks before root reads at
  `:154` and `:160`, signature reads at `:183`, and FPDT header/body
  reads at `:300` and `:306`. The FBPT header and body checks at
  `:499` and `:505` precede their reads at `:504` and `:510`.
  An FPDT pointing into the earlier reserved range therefore returns
  `NotFirmwareMemory` before reading the FBPT. This closes the earlier
  failure by leaving ambiguous backing unavailable. It intentionally
  loses firmware times when the tables occupy reserved memory, as the
  latest PM comment states.

- PASS — Prior response-cast/HHDM fix remains intact:
  `kernel/src/boot_time.rs:40` selects `EfiMemmapRespData` before field
  extraction, and `kernel/src/boot_time_logic.rs:230` preserves an
  existing HHDM address; the tests at
  `tools/boot-time-logic-tests/src/lib.rs:544` and `:564` pass.

- PASS — New: no additional actionable regression found in this narrow re-check.

Validation: all 18 tests passed with
`cargo test -p boot-time-logic-tests --target x86_64-unknown-linux-gnu --offline`
in a detached worktree at the reviewed head;
`git diff origin/main...pr16 --check` passed. No kernel build, QEMU,
physical boot, hostile-firmware boot, or full CI gate was run in this
re-check; the PM's gate and QEMU claims were not independently rerun.
This is a source/host-test verdict on item 1, not a full PR approval.
