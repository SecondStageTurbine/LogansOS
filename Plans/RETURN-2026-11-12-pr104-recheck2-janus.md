<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 395 — PR #104, OT-100 R2, third check

Reviewer: Janus (Codex).
Dispatch: `Plans/DISPATCH-2026-11-12-relays-395-397.md`, Section 395.
Reviewed head: `16bb8f56926f3be0d21e76411d140835f8827ee2`
(`claude/ot100-r2-msc-write`).
RETURN branch: `codex/pr104-recheck2-janus`, based on `origin/main`.

**Verdict: FAIL — the expired-completion and WRITE-sense cases are fixed;
SYNC still retries after expiry, and command publication can follow a lock wait
that crosses the deadline.** All source coordinates below refer to the reviewed
head. This is a review only; no implementation changes are included.

## F1a

- **PASS — expiry beats a completion on the first expired tick.**
  `kernel/src/drivers/usb/msc_logic.rs:843` checks before polling, and
  `kernel/src/drivers/usb/msc_logic.rs:848` checks again after finding a completion.
  The driver actually uses this helper at `kernel/src/drivers/usb/msc.rs:182`.
  xHCI checks before searching pending events and again under the pending lock
  before removing the selected event (`kernel/src/drivers/usb/xhci.rs:957`,
  `kernel/src/drivers/usb/xhci.rs:963`). A late CSW cannot produce `Written` through
  this wait. The real `command_dir`/`wait_by`/`write_chunk` host case at
  `tools/usb-msc-logic-tests/src/policy_tests.rs:926` returns `Io` with a CSW
  deadline event; its one-tick-earlier control at line 957 returns `Written`.
- **FAIL — a successful REQUEST SENSE after expiry cannot authorize another
  attempt: WRITE passes, SYNC does not meet this end-to-end requirement.**
  WRITE checks expiry before acting on UNIT ATTENTION or NOT READY
  (`kernel/src/drivers/usb/msc_logic.rs:1110`). SYNC also checks expiry at
  `kernel/src/drivers/usb/msc_logic.rs:1239`, but its deadline result is ordinary
  `SyncResult::Failed` (`kernel/src/drivers/usb/msc_logic.rs:1220`). `flush`
  retries that result at `kernel/src/drivers/usb/msc_logic.rs:1295` and
  `kernel/src/drivers/usb/msc_logic.rs:1305`.

  **Concrete input:** set `sync_ok=true`; begin SYNC at tick 0 (deadline 3001),
  fail the command at tick 3000, deliver a decoded UNIT ATTENTION sense
  `(key=6, asc=0x28, ascq=0)` at tick 3001, and let the next SYNC succeed one tick
  later. `flush` sends two commands and returns `Flushed`; the second command's
  deadline is 6002. A scratch host test against the production policy reproduced
  this exact sequence. The late sense is no longer classified for retry, but the
  timeout itself remains retryable, so another attempt still follows. The
  repository test calls only `sync_once`, not `flush`
  (`tools/usb-msc-logic-tests/src/policy_tests.rs:500`). Preserve an explicit
  terminal timeout result through `flush` to meet the dispatched requirement;
  test the outer operation as well as the individual attempt.
- **PASS — `write_chunk_bound_ms` accounts for the extra tick.**
  `attempt_ticks` is `ceil(ms / MS_PER_TICK) + 1`
  (`kernel/src/drivers/usb/msc_logic.rs:761`), shared by `Deadline::after`
  (`kernel/src/drivers/usb/msc_logic.rs:797`) and the chunk bound
  (`kernel/src/drivers/usb/msc_logic.rs:768`). The result is 7107 ticks,
  **71,070 ms**, at `kernel/src/drivers/usb/msc_logic.rs:774`.
- **PASS — the earlier 7106-tick sequence no longer exceeds that bound.**
  Numerically, 7106 < 7107. More strongly, that exact sequence now ends at its
  first late UNIT ATTENTION sense, tick 1001, through
  `kernel/src/drivers/usb/msc_logic.rs:1110`; the regression checks one WRITE and
  tick 1001 at `tools/usb-msc-logic-tests/src/policy_tests.rs:460`. The revised
  seven-attempt sequence, with senses completing before expiry, is checked at
  `tools/usb-msc-logic-tests/src/policy_tests.rs:425` and line 452. These are
  controlled-clock policy bounds, not a guarantee against arbitrary scheduling
  delays or lock contention.
- **PASS — tests cover expired-tick completions and late successful sense.**
  `tools/usb-msc-logic-tests/src/policy_tests.rs:110`, line 121 and line 140 cover
  ready-at-expiry, resumed-after-expiry and expiry during polling. Lines 460,
  478 and 500 cover late WRITE UNIT ATTENTION, WRITE NOT READY and SYNC sense.
  Line 489 preserves DATA PROTECT's refusal/latch. The SYNC test's outer-loop
  coverage gap is the failure described above.

## F1b

- **FAIL — the deadline is checked between recovery steps, but not reliably at
  command publication after waiting for the controller lock.**
  The sequencing fix is present: `clear_halt_by` checks before each step
  (`kernel/src/drivers/usb/msc_logic.rs:889`, line 893 and line 897), wired to
  the controller by `kernel/src/drivers/usb/msc.rs:365`. The direct xHCI recovery
  wrapper checks between Reset/Stop and Set TR Dequeue at
  `kernel/src/drivers/usb/xhci.rs:2080`. Reset Endpoint and Stop Endpoint reach
  the submission helper through `kernel/src/drivers/usb/xhci.rs:2110` and
  line 2113; Set TR Dequeue does so from line 2131. CLEAR_FEATURE reaches the
  EP0 admission check at `kernel/src/drivers/usb/xhci.rs:2446` through line 2162.

  **Concrete input:** invoke a Reset Endpoint with `Some(1001)` at tick 1000.
  `submit_command_until` passes its check at
  `kernel/src/drivers/usb/xhci.rs:1055`; another CPU holds `CONTROLLER` until
  tick 1001. The caller waits at `kernel/src/drivers/usb/xhci.rs:1060`, then
  enqueues at line 1062, writes the TRB at line 1066 and rings the doorbell at
  line 1073 without another deadline read. Its subsequent event wait times out,
  but the command has already been published after expiry. `CONTROLLER` is a
  `SwitchMutex` (`kernel/src/drivers/usb/xhci.rs:469`), whose acquisition spins
  without a deadline (`kernel/src/sync/switch_mutex.rs:180`). The same submission
  helper serves Stop Endpoint and Set TR Dequeue. This counterexample follows
  directly from the actual lock/submission code; it is not a hardware test.

  Recheck after acquiring the controller lock and before mutating/publishing
  the command ring, and make lock acquisition deadline-aware if the claimed
  bound includes that wait. Cover clock advancement during submission admission,
  in addition to advancement during a completed recovery step. The original
  Reset-exhausts-deadline sequence is fixed, but it does not exercise this gap.
- **PASS — a host test advances the clock during Reset/Stop and prevents later
  steps.** `tools/usb-msc-logic-tests/src/policy_tests.rs:665` advances the shared
  clock in the mock Reset/Stop step. Line 700 checks that only that step ran;
  line 709 checks that expiry during Set TR Dequeue prevents CLEAR_FEATURE.
  Line 982 runs the stalled-write case through the production command, wait and
  recovery sequencer and confirms no subsequent dequeue, clear or class reset.
  These tests mock the individual controller operations, so they do not exercise
  the controller-lock publication gap above.

## Still true

- **PASS — every xHCI `_until` variant's `None` behavior is preserved.**
  `until_passed(None)` returns false without reading the clock
  (`kernel/src/drivers/usb/xhci.rs:940`). The new wait and submit checks are inert
  for `None` (lines 957, 963, 982 and 1055). Recovery preserves the kill switch,
  Reset/Stop/Requeue decision, ring reset and result fields (lines 2066, 2107,
  2128); the between-step check at line 2080 is inert. CLEAR_FEATURE's request
  and EP0 control stages remain the same (lines 2159, 2446, 2454 and 2468).
  Compared the full change with its parent, including feature-gated counters
  and wait budgets. `clear_halt_by(None)` also runs every step and preserves
  the old `set_dequeue_ok && device` result
  (`kernel/src/drivers/usb/msc_logic.rs:885`, line 901).
- **PASS — the read-only `command()` path is unchanged.**
  `kernel/src/drivers/usb/msc_logic.rs:379` is byte-identical to the parent;
  the first 27,513 bytes (through line 758) are unchanged. The driver still calls
  it from `kernel/src/drivers/usb/msc.rs:289`, and no-deadline transfers still
  use the original wait at `kernel/src/drivers/usb/msc.rs:230`. The refactored
  halt clear retains the same no-deadline operation order and result.

## Validation

- Read the four changed files at the exact reviewed head in an isolated worktree.
- `git range-diff c7235a49~6..c7235a49 16bb8f56~7..16bb8f56^`:
  all six earlier commits are unchanged (`=`).
- `cargo test -p usb-msc-logic-tests`: **71 passed**, zero failures.
- Added a temporary host test using the existing clocked policy mock with
  `late_senses=true`, the concrete SYNC input above, and assertions for
  `Flushed`, two commands, deadlines 3001/6002 and a SYNC deadline event:
  **1 passed**, confirming the remaining behavior. Removed the scratch test;
  the reviewed worktree is clean and this RETURN is the only committed file.
- No QEMU or physical USB execution is claimed. The xHCI lock finding is a
  source-level concurrency counterexample, while the policy finding is also
  reproduced by host execution.
