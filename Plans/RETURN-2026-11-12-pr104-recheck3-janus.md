<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 398 — PR #104, OT-100 R2, round 4

Reviewer: Janus (Codex).
Dispatch: `Plans/DISPATCH-2026-11-12-relays-398-399-rechecks.md`, Section 398.
Reviewed head: `b6ee1046f9f922e9dbcb05e4ef4c731dce78a7d7`
(`claude/ot100-r2-msc-write`).
RETURN branch: `codex/pr104-recheck3-janus`, based on `origin/main`.

**Verdict: PASS — both relay 395 items are fixed.** Coordinates below refer
to the reviewed head. This RETURN contains no implementation changes.

## Item 1: a SYNC timeout must end the flush

- **PASS — deadline expiry is terminal through `flush`.** The deadline
  closure returns `SyncResult::TimedOut` at
  `kernel/src/drivers/usb/msc_logic.rs:1225`; late sense reaches it at line
  1244 and expired command/recovery reaches it at line 1266. The outer loop
  immediately returns `FlushResult::TimedOut` at line 1313; only `Failed`
  continues at line 1314. The driver maps timeout to `BlockError::Io` at
  `kernel/src/drivers/usb/msc.rs:931`. DATA PROTECT and disappearance remain
  terminal refusals as well.
- **PASS — the specified input sends one SYNC and never returns `Flushed`.**
  `tools/usb-msc-logic-tests/src/policy_tests.rs:1012` supplies command failure
  after 3000 ticks, UNIT ATTENTION `(6, 0x28, 0)` one tick later, and a queued
  succeeding SYNC. `late_senses=true` delivers the sense at tick 3001 rather
  than suppressing it. Lines 1020-1022 assert `TimedOut`, exactly one SYNC,
  and deadline 3001. The mock starts at zero (line 216), advances the failed
  command clock at line 249, and delivers/advances the late sense at line 263.
  Production enforcement is the expiry check at
  `kernel/src/drivers/usb/msc_logic.rs:1244` and terminal return at line 1313.
- **PASS — a host test drives `flush` itself.** The regression calls `flush`
  at `tools/usb-msc-logic-tests/src/policy_tests.rs:1020`. The transport-timeout
  case also calls it at line 1042, while the in-time failure control at line
  1057 still reaches `Flushed` after three attempts. All passed locally.

## Item 2: ring admission must honour the deadline

- **PASS — the deadline is rechecked after acquiring `CONTROLLER`, before
  command-ring mutation or doorbell.** `submit_command_until` calls
  `ring::admit_until` with the real clock and `CONTROLLER.try_lock` at
  `kernel/src/drivers/usb/xhci.rs:1060`. After winning the lock,
  `kernel/src/drivers/usb/xhci_ring_logic.rs:364` rereads the clock; expiry
  drops the guard and returns `None` at lines 365-366. The caller's `?` at
  `kernel/src/drivers/usb/xhci.rs:1066` therefore prevents enqueue (line
  1068), TRB write (line 1072), and doorbell (line 1079).
  EP0 has no intervening controller-lock acquisition: its caller owns the
  device mutably (`kernel/src/drivers/usb/xhci.rs:2218`), checks expiry at
  line 2454, then enqueues at lines 2462/2476 and rings at line 2486.
  `enqueue_ep0` (line 1904), `write_trb` (line 558), and `arm_link_trb`
  (line 789) introduce no lock wait. The ownership explanation is at line
  2451. This satisfies the dispatched admission check; it is not a claim
  of atomicity between the timer read and the subsequent MMIO writes.
- **PASS — the controller-lock wait is deadline-aware.**
  `kernel/src/drivers/usb/xhci_ring_logic.rs:360` tests the deadline before
  each try; line 363 uses the nonblocking acquisition supplied by the caller.
  `kernel/src/sync/switch_mutex.rs:207` implements that acquisition with a
  single compare-exchange, returning `None` on contention at line 226.
  The unconditional `lock()` closure is used only for `None` (ring helper
  line 357). A lock held beyond tick 1001 no longer admits the command when
  finally released.
- **PASS — the admission test advances the clock and rejects publication.**
  `tools/ai-ops-tests/src/lib.rs:4820` calls the production admission helper.
  Its mock advances the clock inside `try_lock` at line 4846. Lines
  4862/4865 assert no admission both when the lock remains held beyond the
  deadline and when it is won at 1000 but acquisition advances to 1001;
  the latter also asserts that the acquired guard was dropped. In-time,
  already-expired, and `None` controls are at lines 4867/4869/4871. This is
  a helper-level test, not hardware MMIO instrumentation: rejection means
  no publication because the production caller returns via `?` before any
  ring access (`kernel/src/drivers/usb/xhci.rs:1066`). The test passed.

## Still true

- **PASS — every `_until` variant preserves its `None` path.**
  `kernel/src/drivers/usb/xhci_ring_logic.rs:356` immediately selects the
  original unconditional lock without reading the clock or trying the lock.
  `kernel/src/drivers/usb/xhci.rs:940` makes the remaining deadline checks
  inert for `None`. Inspected `wait_event_until` (line 949),
  `submit_command_until` (line 1057), `recover_endpoint_until` (line 2066),
  `clear_endpoint_halt_until` (line 2160), and `control_no_data_until`
  (line 2218): original wait budgets, recovery ordering, requests, and EP0
  stages remain. MSC's deadline-free transfers still call the original
  wait at `kernel/src/drivers/usb/msc.rs:230`; its `wait_until` at line 168
  takes a required deadline, not an optional one. The `None` admission
  control at `tools/ai-ops-tests/src/lib.rs:4871` passed.
- **PASS — the read-only `command()` path is unchanged.**
  `kernel/src/drivers/usb/msc_logic.rs:379` and the entire prefix through
  line 758 are byte-identical to the previous reviewed head `16bb8f56`.
  Its driver call remains at `kernel/src/drivers/usb/msc.rs:298`. The only
  driver change in the new commit is the flush timeout mapping at line 931.

## Validation

- Read the implementation and regression tests at the exact head in a
  detached worktree; that worktree remains clean.
- `git range-diff 16bb8f56~7..16bb8f56 b6ee1046~8..b6ee1046^`:
  all seven previously reviewed commits compare unchanged (`=`).
- `cargo test -p usb-msc-logic-tests`: **75 passed**, zero failures.
- `cargo test -p ai-ops-tests xhci_admit_until_refuses_after_the_deadline`:
  **1 passed**, zero failures (other tests filtered out).
- No QEMU or physical USB execution is claimed. The evidence is source
  inspection and host execution of production policy/admission helpers.
