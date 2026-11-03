<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PR #61 re-check: key repeat focus/lock race

Reviewed head: `b878b84e70ebcc3edfc3d57214257d9e193d8f4d`.

- PASS — Initial delay is 500 ms, interval is 1/30 s, and polling emits at most one repeat without catch-up bursts: `kernel/src/atrium/key_repeat_logic.rs:41-46`, `:138-154`.
- PASS — Modifiers, lock keys, Pause, Print Screen, Escape, Enter, and keypad Enter are excluded: `kernel/src/atrium/key_repeat_logic.rs:48-65`.
- PASS — Release, another press, modifier change, focus epoch change, and the 20 s maximum stop a repeat run: `kernel/src/atrium/key_repeat_logic.rs:112-145`.
- PASS — PS/2 and USB HID pumps use the shared repeat state machine and deliver through the same epoch-aware helper: `kernel/src/drivers/ps2/i8042.rs:543-552`, `kernel/src/drivers/usb/hid_keyboard.rs:329-338`, `kernel/src/atrium/key_repeat.rs:54-76`.
- PASS — Previous FAIL, focus/lock race: `poll` returns the checked epoch with the event (`kernel/src/atrium/key_repeat.rs:50-58`); staged delivery drops an event if that epoch is already stale and stamps accepted events with the checked epoch (`kernel/src/atrium/focus.rs:445-469`). Flush compares the stage epoch while holding `DELIVERY` and discards stale batches (`kernel/src/atrium/focus.rs:480-505`). The legacy path checks the epoch while holding `DELIVERY` before enqueue (`kernel/src/atrium/focus.rs:363-392`). Both keyboard pumps call `deliver` (`kernel/src/drivers/ps2/i8042.rs:546-551`, `kernel/src/drivers/usb/hid_keyboard.rs:330-335`).
- PASS — Race scenario is non-vacuous and exercises a delivered baseline, focus change between poll and stage, lock between poll and stage, lock between stage and flush, and stale/current legacy routing: `kernel/src/test_harness.rs:28247-28369`. The control restores the pre-fix fresh-epoch staging and expects `repeat_crossed_focus_change` (`kernel/src/atrium/key_repeat.rs:64-70`, `kernel/src/test_harness.rs:28240-28246`). Seat I reports the scenario and control pass/fail as expected in its latest PM note.
- PASS — Feature-off build regression from the earlier red gate is addressed: `stage_event_at` and `route_event_at` are gated on `key-autorepeat` (`kernel/src/atrium/focus.rs:368-370`, `:450-452`), and ordinary `stage_event` uses the shared stage body without referencing key-repeat-only code (`kernel/src/atrium/focus.rs:422-443`). Seat I reports all 13 kernel clippy matrix rows clean locally with CI flags, and `lock_route_races` passing without `key-autorepeat`, in its latest PM note.
- PASS — CP-102 is recorded in the concurrency pitfall bank: `docs/concurrency-pitfalls.md` (the added row in this head).

New: the close-gate run for reviewed head `b878b84e` was still in progress at re-check time (run [41553394403](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41553394403)); no CI conclusion yet.
