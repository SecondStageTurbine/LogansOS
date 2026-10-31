<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: Arthur Agentic IPC-Frequency Measurement

**Date:** 2026-07-04
**Status:** measurement note for OT-15 input; not an actuator.

## Scope

This pass measures Arthur's NL -> tool-call -> cap-check -> action path. It
does not change action behavior, policy, grammar, scheduling, or QoS.

Instrumentation is feature-gated:

- kernel syscall class counters: `kernel-observability`
- userland cap-check/action counters: `libneo/action-observability`
- Arthur scenario wiring: `agentic-ipc-frequency`

Disabled builds keep the prior path: the added producers are compiled out or
return zero snapshots.

## Measurement Window

The scenario starts the turn snapshot before Arthur receives the natural-language
prompt on the conversation channel. The window includes the inbound receive, the
decode/tool-call path, cap-checks, and action execution. It ends before Arthur
sends the reply. The final `sys_observe` read used to collect the snapshot is
subtracted from `syscalls` and `other`, so the measurement syscall itself is not
counted.

Counts are exact syscall/counter deltas from QEMU/TCG for this deterministic
scenario. They are not wall-clock latency measurements.

## Observed Counts

Serial proof came from:

`target/neo-test/arthur_agentic_actions.attempt1.serial.log`

Distinct reported rows:

| Tier | Action | Syscalls | IPC | File | AI | Net | Other | Cap checks | Execute round-trips |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| A1 | list_dir | 33 | 3 | 9 | 2 | 0 | 19 | 1 | 1 |
| A1 | read_file | 41 | 3 | 13 | 2 | 0 | 23 | 1 | 1 |
| A1 | read_file | 42 | 3 | 12 | 2 | 0 | 25 | 1 | 1 |
| A1 | read_file | 47 | 3 | 16 | 2 | 0 | 26 | 1 | 1 |
| A1 | read_file | 49 | 3 | 17 | 2 | 0 | 27 | 1 | 1 |
| A1 | read_file | 50 | 3 | 17 | 2 | 0 | 28 | 1 | 1 |
| A2 | undo | 37 | 3 | 10 | 2 | 0 | 22 | 1 | 1 |
| A2 | write_file | 32 | 3 | 7 | 2 | 0 | 20 | 1 | 1 |
| A2 | write_file | 38 | 3 | 12 | 2 | 0 | 21 | 2 | 2 |
| A2 | write_file | 50 | 3 | 16 | 2 | 0 | 29 | 1 | 1 |
| A2 | write_file | 54 | 3 | 20 | 2 | 0 | 29 | 2 | 2 |
| A3 | trash_file | 58 | 3 | 22 | 2 | 0 | 31 | 2 | 2 |
| A3 | kill_process | 84 | 3 | 33 | 2 | 0 | 46 | 1 | 1 |
| A3 | spawn_process | 96 | 3 | 39 | 2 | 0 | 52 | 2 | 1 |
| A3 | trash_file | 100 | 3 | 43 | 2 | 0 | 52 | 2 | 2 |
| A3 | load_model | 108 | 3 | 45 | 2 | 0 | 58 | 1 | 1 |
| A3 | external_send | 114 | 3 | 48 | 2 | 0 | 61 | 2 | 2 |
| A3 | move_file | 128 | 3 | 57 | 2 | 0 | 66 | 2 | 2 |

Headline ranges from this scenario:

- A1 read/list actions: 33-50 syscalls/action, always 3 IPC crossings, 1
  cap-check, and 1 execute round-trip.
- A2 write/undo actions: 32-54 syscalls/action. Reversible writes that take the
  undo snapshot cost 2 cap-checks and 2 execute round-trips; denied writes and
  undo restores are 1/1.
- A3 actions: 58-128 syscalls/action. The highest observed file-heavy paths are
  `move_file` and `external_send`; the current process/model/network seams still
  report the deterministic scenario's syscall surface and should be remeasured
  when those seams become fully live kernel effects.

The stable part of the measurement is more important than the exact totals:
every executed action pays the Arthur conversation/decode envelope, then adds
action-specific file/capability work. The IPC count is flat in this scenario;
file and "other" syscall volume dominate the spread.

## Hot Spots

- The call-coalescing candidate is not A1. A1 is already one cap-check and one
  execute round-trip; optimizing the single boundary crossing is unlikely to be
  the first OT-15 win.
- A2 writes pay for safety: read/check snapshot before write, then write. That
  is the clearest repeated file path in this measurement.
- A3 trash/move-style actions build on A2 snapshot semantics and therefore show
  the same file-heavy shape at larger totals.
- Non-file A3 seams should not drive policy from these numbers yet. The scenario
  keeps deterministic/fixture plumbing in the measurement window, so their
  syscall totals are useful for regression visibility but not a final real-world
  cost model.

## Double-Bookkeeping Scan

No kernel/userland pair currently owns the same enforcement state on this path.

Single-owner split observed:

- Kernel: capability handles, rights, derivation/grant/revoke mechanics, file
  handle enforcement, process/model/network capability checks.
- Arthur/userland: posture, pending A2/A3 confirmation state, undo snapshots,
  deterministic tool-call parsing, runtime `Grant` arrays passed to
  `execute_action`, and the measurement-only cap-check/action counters.
- Durable grant store: `.arthur/grants.json` is persistence/UX/audit input. It
  is not authority. The live kernel cap remains the enforcement boundary; a grant
  record without a backed kernel cap still fails at the syscall boundary.

The durable grant store is the only HM-antipattern watch item. It is acceptable
as a user-facing record today because the code treats it as non-authoritative.
It would become double-bookkeeping debt only if a future slice let
`grants.json` independently authorize actions without a live kernel capability.

## Recommendation For OT-15

Feed OT-15 with the current boundary counts, but do not build a rate limiter from
this pass.

Recommended next measurement target:

1. Keep A1 as the baseline path.
2. For A2/A3, measure whether snapshot/read/write coalescing can reduce repeated
   file syscalls without weakening undo or cap enforcement.
3. Re-run A3 process/network/model numbers once the currently deterministic seams
   become live kernel effects.
4. Attribute the counts per principal/action in OT-15 before adding any actuator.

This pass supports a measured QoS/accounting follow-on. It does not justify
collapsing Arthur into a more privileged service or bypassing capabilities.
