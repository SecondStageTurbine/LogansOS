<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Zenbook USB input second narrow re-check (Chad, PR #93)

**Reviewed head:** `3829b9f1a09c6ac2dbb929d77b53d820f75d9137`
**Fix diff:** `7bcd49acf61dbdc8901305fbfd788e61a8d17443..3829b9f1a09c6ac2dbb929d77b53d820f75d9137` (3 files, +54/-3).
**Prior report:** `f20f9e38887e2273e07e91cd56ec97a3a6a53d78:Plans/RETURN-2026-10-22-zenbook-usb-input-recheck-chad.md`.
**Verdict:** CLOSED — the one remaining item 1 counterexample is fixed.

## 1. Dropped Supported Protocol ranges — CLOSED

`kernel/src/drivers/usb/xhci_silicon_logic.rs:67` records `ExtCaps::protocols_dropped`.
The storage guard at `:116` keeps the first eight ranges, and the subsequent
Supported Protocol match arm at `:128` increments the dropped count for every
additional range. `protocols_known` at `:159` now requires that count to be zero,
as well as a completed walk and at least one recorded range.

The production probe feeds this predicate into the candidate at
`kernel/src/drivers/usb/xhci_silicon.rs:259`. For the prior nine-capability
recipe, the resulting state is `protocol_count=8`, `protocols_dropped=1`,
`stop=None`, `usb2_ports=0`, and `protocols_known=false`. With the readable,
known SuperSpeed-only controller at index 0 and this controller at index 1,
`choose` takes the unknown-protocol branch at
`kernel/src/drivers/usb/xhci_silicon_logic.rs:230` and returns index 1 at `:231`.
The reason's displayed name is exactly `AMBIGUOUS-protocols-unknown` at `:202`.
This recipe no longer reaches the index-0 `FirstReadable` fallback.

## 2. Regression KATs — CLOSED

The new KAT at `tools/xhci-silicon-logic-tests/src/lib.rs:181` uses the exact
register recipe from the prior report: HCCPARAMS1 `0x00400000`, window `0x1000`,
capability offsets `0x100 + 16*i` for `i=0..8`, headers `0x03000402` for the
first eight and `0x02000002` for the ninth, and DWORD2 `0x100 | (i+1)`
(`:183`, `:185`, `:187`, `:190`). It asserts one dropped range at `:192`,
unknown protocols at `:195`, and `Some((1, ChoiceReason::ProtocolsUnknown))`
at `:203`.

The existing ten-range test asserts `protocols_dropped=2` at
`tools/xhci-silicon-logic-tests/src/lib.rs:163` and unknown protocols at `:166`.
These tests import the actual kernel logic module through `#[path]` at `:15`.

## 3. New findings in the fix diff — none

No new finding identified in the complete three-file fix diff. The added
census diagnostic at `kernel/src/drivers/usb/xhci_silicon.rs:390` prints the
dropped count and uses the same completeness predicate at `:393` to print
`protocols=UNKNOWN`. The `u16` dropped counter cannot overflow during this
walk: `MAX_EXT_CAPS=64` at `kernel/src/drivers/usb/xhci_silicon_logic.rs:28`,
enforced before each capability at `:108`, limits drops to at most 56 after
the eight stored ranges.

## Validation and scope

- Ran `cargo test -p xhci-silicon-logic-tests`: 26 passed, 0 failed; doc tests passed (0 tests).
- Both the exact nine-capability regression and the existing ten-range test passed.
- Read the prior report and the specified fix diff; traced production predicate use and choice precedence directly in the checked-out source.
- Used graft for exact symbol references; supplied file paths were read directly, so semantic RAG discovery was unnecessary.
- Host logic validation only; no hardware or QEMU run was performed for this narrow re-check.
- Preserved the pre-existing `.gitignore` modification and untracked `.ignore`; only this report is committed.
