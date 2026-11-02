<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: #61 key repeat review (Jarvis)

Reviewed head: `1b4fd36b9f6da1afb9a02fa7ea30094b7004624a`

- PASS — First repeat is due after 500 ms; subsequent repeats use a 30/s interval. Modifier usages and the lock keys, Pause, Print Screen, Escape, and Enter are excluded. `kernel/src/atrium/key_repeat_logic.rs:41-65`
- FAIL — A concurrent focus change or lock can let a repeat reach the new focus. The pump polls against the activation epoch, then stages the event separately; `stage_event` stamps it with whichever epoch is current at that later point. `kernel/src/drivers/usb/hid_keyboard.rs:329-338`, `kernel/src/drivers/ps2/i8042.rs:543-552`, `kernel/src/atrium/focus.rs:400-408`, `kernel/src/atrium/focus.rs:863-869`. Concrete input/schedule: hold Backspace until a repeat is due; the pump reads the old epoch and produces the repeat, another CPU activates a different focus or engages the lock (bumping the epoch), then the pump stages it. The repeat is tagged with the new epoch and can be flushed to that new client or the lock screen.
- PASS — The run ends on release of the repeating key, another press, modifier change, focus epoch change, or after the 20 s cap. Poll emits at most one event per call and resets after a late pump instead of catching up in a burst. `kernel/src/atrium/key_repeat_logic.rs:105-146`
- PASS — USB HID and PS/2 pumps call the same `KeyRepeat` state machine and shared repeat event helper. `kernel/src/drivers/usb/hid_keyboard.rs:301-338`, `kernel/src/drivers/ps2/i8042.rs:535-552`
