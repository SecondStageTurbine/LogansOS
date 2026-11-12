<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 394 — PR #104, OT-100 R2, F1–F3 re-check

**Reviewer:** Janus (Codex).
**Dispatch:** `Plans/DISPATCH-2026-11-12-relay-394-pr104-recheck.md`.
**Reviewed head:** `c7235a49ca5625afe2db1f6268397562e2f408af`.
**RETURN branch:** `codex/pr104-recheck-janus`, based on `origin/main`.
**Verdict: FAIL — F1 remains open; F2 passes; F3's UNIT ATTENTION discriminator passes, but its deadline guarantee remains incomplete.**

All source coordinates refer to the reviewed head, read from an isolated worktree.
`git range-diff 2cf42002~5..2cf42002 c7235a49~6..c7235a49^` confirms all five
previously reviewed commits are unchanged (`=`). This re-check reviews the new
commit and its integration, not another implementation pass. No source is changed.

## Remaining findings and concrete inputs

### F1a — P1: ready completions bypass an expired deadline

`wait_by` returns `Poll::Done` before reading the deadline
(`kernel/src/drivers/usb/msc_logic.rs:821`, `kernel/src/drivers/usb/msc_logic.rs:826`).
The actual driver uses that helper (`kernel/src/drivers/usb/msc.rs:182`). The same
ordering exists in xHCI: a pending event is returned at
`kernel/src/drivers/usb/xhci.rs:951`, before the deadline check at
`kernel/src/drivers/usb/xhci.rs:968`.

**Concrete input:** begin a WRITE at tick 0, with `Deadline::after(0, 10000).end = 1001`.
Its CBW and data complete. On the last pending CSW poll at tick 1000, there is no
completion. The tick advances to 1001 during the idle spin, and the next poll finds
a valid, passed CSW with residue 0. `wait_by` returns `Done`; `command_dir` returns
`Passed`; `write_chunk` returns `Written` at
`kernel/src/drivers/usb/msc_logic.rs:1022`. There is no final deadline check and no
deadline event. This is a completion arriving at the expired tick, not an assumption
about an event's unavailable hardware timestamp. A delayed thread resumed after
expiry has the same problem.

The failed-command branch also accepts a successful REQUEST SENSE after expiry:
the check at `kernel/src/drivers/usb/msc_logic.rs:1029` requires `sense.is_none()`.
A late, decoded UNIT ATTENTION or NOT READY can therefore authorize another
attempt after the old attempt has expired. SYNC has the analogous conditions at
`kernel/src/drivers/usb/msc_logic.rs:1147` and
`kernel/src/drivers/usb/msc_logic.rs:1154`.

**Bound counterexample:** seven attempts, with UNIT ATTENTION on attempt 1, NOT
READY on attempts 2–5, a recovered reset on attempt 6, and a deadline-ending reset
on attempt 7. For attempts 1–5, finish the failed command at `start + 1000` ticks
and accept its sense at `start + 1001`; attempt 6 finishes recovered at
`start + 1000`; attempt 7 times out at `start + 1001`. Four pauses take 25 ticks
each. The actual policy permits `5*1001 + 1000 + 1001 + 4*25 = 7106` ticks,
**71,060 ms**, exceeding `write_chunk_bound_ms() == 71000` at
`kernel/src/drivers/usb/msc_logic.rs:761`. The existing worst-sequence test instead
chooses durations below the nominal bound
(`tools/usb-msc-logic-tests/src/policy_tests.rs:358`); its mock rejects ready
completions at expiry before the production wait helper can accept them
(`tools/usb-msc-logic-tests/src/policy_tests.rs:189`).

**Required correction:** give an expired deadline precedence over accepting a
completion on the bounded path, and enforce it before advancing the command or
retry policy. Keep `None` behavior unchanged. Account for the extra tick in
`Deadline::after` when defining/testing the claimed worst-case bound. Test a
completion becoming ready on the first expired tick and a late successful sense.

### F1b — P1: recovery submits further commands after its deadline

`Transport::clear` checks the clock once, caches `until`, runs host recovery, and
then unconditionally starts device-side CLEAR_FEATURE with that cached value
(`kernel/src/drivers/usb/msc.rs:272`, `kernel/src/drivers/usb/msc.rs:281`,
`kernel/src/drivers/usb/msc.rs:283`). Inside host recovery, Set TR Dequeue is
submitted even if the preceding Reset/Stop Endpoint wait exhausted the deadline
(`kernel/src/drivers/usb/xhci.rs:2058`, `kernel/src/drivers/usb/xhci.rs:2073`).
Neither submission helper rejects an expired `Some(end)` before publishing work:
`submit_command_until` enqueues and rings the doorbell at
`kernel/src/drivers/usb/xhci.rs:1048` and `kernel/src/drivers/usb/xhci.rs:1055`;
the control helper publishes EP0 stages and rings its doorbell at
`kernel/src/drivers/usb/xhci.rs:2396` and `kernel/src/drivers/usb/xhci.rs:2420`.

**Concrete input:** a WRITE's bulk-OUT data stalls at tick 1000, deadline 1001.
Host Reset Endpoint never completes, and its wait ends at 1001. The code still
submits Set TR Dequeue at or after tick 1001, then submits CLEAR_FEATURE on EP0
after that. Those are recovery operations started after the absolute deadline.
The outer `Some(0)` checks protect entry into `clear` and the class reset
(`kernel/src/drivers/usb/msc.rs:273`, `kernel/src/drivers/usb/msc.rs:439`), but
cannot protect later submissions within an already-started recovery.

**Required correction:** recheck `Some(end)` before every recovery submission and
between recovery steps; stop once it expires. Add a host test that advances the
clock during Reset/Stop Endpoint and asserts that no Set TR Dequeue or
CLEAR_FEATURE follows. Preserve all existing `None` behavior.

## Dispatch checklist

### F1: the time bounds

- **FAIL — every wait stops at one absolute deadline.** The plumbing is present:
  WRITE makes one `Deadline` at `kernel/src/drivers/usb/msc_logic.rs:1005`, SYNC at
  `kernel/src/drivers/usb/msc_logic.rs:1130`, and the probe's MODE SENSE and TEST
  UNIT READY at `kernel/src/drivers/usb/msc_logic.rs:1243` and
  `kernel/src/drivers/usb/msc_logic.rs:1256`. `run_dir` installs it across CBW,
  data, CSW and recovery (`kernel/src/drivers/usb/msc.rs:339`); `sense_by` passes
  the failed command's same deadline (`kernel/src/drivers/usb/msc.rs:350`).
  `wait_for` uses it for both bulk directions (`kernel/src/drivers/usb/msc.rs:217`),
  and stall/class-reset recovery forwards it (`kernel/src/drivers/usb/msc.rs:281`,
  `kernel/src/drivers/usb/msc.rs:446`). F1a and F1b give the concrete exceptions.
- **PASS — 9+9+9 s WRITE and 29+29 s SYNC schedules time out under one deadline.**
  The controlled-clock tests execute the production `wait_by`, reuse the same
  deadline, and cut off the next pending stage:
  `tools/usb-msc-logic-tests/src/policy_tests.rs:55` and
  `tools/usb-msc-logic-tests/src/policy_tests.rs:69`.
- **PASS — all five xHCI `None` paths preserve prior behavior.** Compared the
  pre-commit bodies with the new bodies, removing only inert deadline plumbing
  and formatting. Specifically:
  - `recover_endpoint_until`: same endpoint-state decision, kill switch,
    Reset/Stop, ring reinitialization, Set TR Dequeue, and result fields;
    `kernel/src/drivers/usb/xhci.rs:2034`, `kernel/src/drivers/usb/xhci.rs:2048`.
  - `clear_endpoint_halt_until`: same endpoint-address calculation and USB request;
    `kernel/src/drivers/usb/xhci.rs:2094`, `kernel/src/drivers/usb/xhci.rs:2106`.
  - `control_no_data_until`: same parameters, EP0 TRBs, doorbell, tolerance,
    completion classification and feature-gated counters;
    `kernel/src/drivers/usb/xhci.rs:2153`, `kernel/src/drivers/usb/xhci.rs:2167`,
    `kernel/src/drivers/usb/xhci.rs:2391`.
  - `submit_command_until`: same enqueue, counters, doorbell and 64-event budget;
    `kernel/src/drivers/usb/xhci.rs:1034`, `kernel/src/drivers/usb/xhci.rs:1040`.
  - `wait_event_until`: same operation charges, pending-event selection, event
    drain and poll budget. `None.is_some_and(...)` is false and does not read
    `TICKS`; `kernel/src/drivers/usb/xhci.rs:933`,
    `kernel/src/drivers/usb/xhci.rs:940`, `kernel/src/drivers/usb/xhci.rs:968`.
  Thus existing callers through the old wrappers retain the old behavior,
  including mt7601u's feature-gated accounting. No behavioral difference found.
- **PASS — explicit retry bounds are 7 and 3.**
  `kernel/src/drivers/usb/msc_logic.rs:755`,
  `kernel/src/drivers/usb/msc_logic.rs:757`; enforced by the loops at
  `kernel/src/drivers/usb/msc_logic.rs:999` and
  `kernel/src/drivers/usb/msc_logic.rs:1215`.
- **FAIL — no recovery starts after expiry.** F1b's stalled-data input starts
  Set TR Dequeue and CLEAR_FEATURE after expiry;
  `kernel/src/drivers/usb/xhci.rs:2073`, `kernel/src/drivers/usb/msc.rs:283`.
- **FAIL — worst allowed sequence stays within `write_chunk_bound_ms`.** F1a's
  seven-attempt sequence reaches 71,060 ms against 71,000 ms;
  `kernel/src/drivers/usb/msc_logic.rs:779`,
  `kernel/src/drivers/usb/msc_logic.rs:1029`,
  `kernel/src/drivers/usb/msc_logic.rs:761`.
- **PASS — poll backstops remain.** MSC sizes its finite backstop from remaining
  ticks (`kernel/src/drivers/usb/msc.rs:177`), `wait_by` enforces the poll count
  (`kernel/src/drivers/usb/msc_logic.rs:820`), recovery's xHCI waits keep their
  budgets (`kernel/src/drivers/usb/xhci.rs:946`), and NOT READY's pause remains
  finite (`kernel/src/drivers/usb/msc.rs:562`). Frozen-clock KAT:
  `tools/usb-msc-logic-tests/src/policy_tests.rs:79`.
- **PASS — read-only `command()` path is unchanged.** The entire 26,166-byte
  pre-commit `msc_logic.rs` is an identical prefix of this head, including
  `command` at `kernel/src/drivers/usb/msc_logic.rs:379`. Reads still use `run`
  and that function (`kernel/src/drivers/usb/msc.rs:305`,
  `kernel/src/drivers/usb/msc.rs:314`), with the old poll-only wait when no
  deadline is installed (`kernel/src/drivers/usb/msc.rs:230`). Recovery with
  `None` is unchanged as checked above.
- **PASS — moving policy behind `WritePort` preserves the previously passed
  admission and data-publication properties.** `Port::command` goes through
  `run_dir` (`kernel/src/drivers/usb/msc.rs:547`), whose `command_dir` still
  checks opcode and direction before CBW output
  (`kernel/src/drivers/usb/msc_logic.rs:640`). The registered device is still
  the guard (`kernel/src/drivers/block/mod.rs:767`,
  `kernel/src/drivers/block/mod.rs:781`); both write entry points check the range
  (`kernel/src/drivers/block/mod.rs:585`, `kernel/src/drivers/block/mod.rs:627`),
  and an unarmed probe is refused (`kernel/src/drivers/block/mod.rs:612`).
  Payload still selects the bounce buffer, is copied there, and is fenced
  before the TRB submit (`kernel/src/drivers/usb/msc.rs:378`,
  `kernel/src/drivers/usb/msc.rs:389`, `kernel/src/drivers/usb/msc.rs:395`,
  `kernel/src/drivers/usb/msc.rs:396`). The new policy failures are F1a/F1b;
  no additional allowlist or guard bypass was found.
- **PASS — REQUEST SENSE is an existing opcode with direction In.** Opcode
  `0x03` is in the old set (`kernel/src/drivers/usb/msc_logic.rs:134`) and in
  `WRITE_PATH_COMMANDS` as `Dir::In`
  (`kernel/src/drivers/usb/msc_logic.rs:497`); `sense_by` actually uses `Data::In`
  (`kernel/src/drivers/usb/msc.rs:352`).

### F2: DATA PROTECT on SYNC

- **PASS — the specified failed-SYNC sense latches write-protect.** The exact
  bytes `70 00 07 00 00 00 00 0a 00 00 00 00 27 00 00 00 00 00` are decoded
  in `tools/usb-msc-logic-tests/src/policy_tests.rs:400`. `sync_once` handles
  `WriteProtected` by setting the latch and returning that result
  (`kernel/src/drivers/usb/msc_logic.rs:1159`); flush stops without retrying
  (`kernel/src/drivers/usb/msc_logic.rs:1223`) and the driver returns
  `WriteRefused` (`kernel/src/drivers/usb/msc.rs:895`).
- **PASS — observable protection is `Some(true)` and a subsequent WRITE is
  refused.** `WriteLatches::write_protected` reports the latched value
  (`kernel/src/drivers/usb/msc_logic.rs:938`), the driver's public method forwards
  it (`kernel/src/drivers/usb/msc.rs:902`), and `write_block` refuses it before
  any transport work (`kernel/src/drivers/usb/msc.rs:821`). The exact-input KAT
  asserts the observable latch at
  `tools/usb-msc-logic-tests/src/policy_tests.rs:410`; subsequent write refusal
  is verified by the driver's entry check, not by claiming that this KAT calls
  the full hardware driver.
- **PASS — later MODE SENSE never clears the latch.** The update uses monotonic
  `fetch_max` (`kernel/src/drivers/usb/msc_logic.rs:955`); DATA PROTECT stores the
  maximum state (`kernel/src/drivers/usb/msc_logic.rs:961`). A later negative
  MODE SENSE is explicitly exercised at
  `tools/usb-msc-logic-tests/src/policy_tests.rs:419`.

### F3: the discriminators

- **PASS — retried UNIT ATTENTION prints `sense=6/…`, including a successful
  retry.** The event is emitted before retry
  (`kernel/src/drivers/usb/msc_logic.rs:1035`), the kernel formats the actual
  sense key/ASC/ASCQ (`kernel/src/drivers/usb/msc.rs:589`), and the successful
  retry KAT checks the event
  (`tools/usb-msc-logic-tests/src/policy_tests.rs:305`).
- **FAIL — every passed deadline prints the stage and chunk size.** The formatter
  exists and is correct (`kernel/src/drivers/usb/msc.rs:678`), and a pending
  timeout produces it (`kernel/src/drivers/usb/msc_logic.rs:1074`). However,
  F1a's valid CSW becoming ready at tick 1001 returns `Written` with zero events.
  The early success at `kernel/src/drivers/usb/msc_logic.rs:1022` bypasses the
  deadline discriminator entirely. The ordinary pending-timeout case is fixed;
  the ready-at-expiry case remains open.
- **FAIL — the PR trap-bank table fully matches the code.** The GitHub API body
  at this exact head was read. Its UNIT ATTENTION, DATA PROTECT-on-SYNC and
  configured-64-block-fallback corrections match the implementation. Its
  `MAX_SECTORS_64 / 240` deadline-line promise and recovery-under-the-deadline
  claim still have F1a/F1b's concrete exceptions at
  `kernel/src/drivers/usb/msc_logic.rs:821` and
  `kernel/src/drivers/usb/msc.rs:283`. The other previously passed trap handling
  remains present. One additional literal discriminator mismatch: the pulled
  stick row quotes `device gone during WRITE lba=…`, while the new formatter
  prints `device gone during WRITE(10) lba=…`
  (`kernel/src/drivers/usb/msc.rs:576`, `kernel/src/drivers/usb/msc.rs:581`).

## Validation and limits

- At the reviewed head, `cargo test -p usb-msc-logic-tests`: **55 passed**.
- `cargo test -p xtask usb_msc_write -- --nocapture`: **1 passed**, 212 filtered out.
- Compared every xHCI `None` body with its parent, including the underlying
  control helper; identical after removing inert deadline plumbing and formatting.
- Four scratch host probes compile against the exact reviewed `msc_logic.rs`;
  the halt-clear probe uses the unchanged extracted `Transport::clear` body with
  a mock host recovery that consumes the remaining tick. They reproduce:
  1. `wait_by` returning `Done(1, 0)` at tick 5000 with deadline 1001;
  2. `command_dir` accepting a literal passed CSW at tick 1001, and `write_chunk`
     returning `Written` with zero diagnostic events;
  3. the seven-attempt policy sequence taking 71,060 ms against 71,000 ms;
  4. `clear_endpoint_halt_until` starting at tick 1001 after host recovery consumes
     the deadline. The extra xHCI Set TR Dequeue submission is established by
     direct source inspection at `xhci.rs:2073`.
  These are deterministic clock/transport simulations, not physical USB tests.
- The scratch probes include the actual `wait_by` result for ready completions;
  the repository's policy mock instead assumes expiry always beats a completion.
  That assumption is why the green policy tests do not close F1a/F1b.
- QEMU is unavailable in this worker; no QEMU or physical-device run is claimed.
  The RETURN changes documentation only. F1a/F1b need correction and re-check
  before closing this review.
