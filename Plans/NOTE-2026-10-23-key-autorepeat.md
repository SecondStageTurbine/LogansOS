<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE 2026-10-23: key auto-repeat (`key-autorepeat`)

**Symptom (PL, Zenbook image A):** holding Backspace on the built-in keyboard (i8042, #98) deletes one
character. Every key needs a separate press.

## 1. Diagnosis: nothing in the stack repeats

- **i8042.** `Decoder::feed` reports a held key once and drops every further make for it
  (`kernel/src/drivers/ps2/i8042_logic.rs`, the "A repeat make while held" branch). That was by design, to match
  the USB HID boot report. So any typematic the EC sends is swallowed.
- **USB HID.** `BootKeyEventDecoder` emits an event only when the key set changes. A USB keyboard never repeats
  by itself; the host has to.
- **Above the drivers.** The focus queue, the SAK filter and the clients (`neo-atrium/src/keys.rs`) act on each
  press event. None of them generates repeats.

So the fix is to generate repeats in software, once, for every keyboard.

## 2. Design

- **The state machine.** `kernel/src/atrium/key_repeat_logic.rs` is pure and host-tested. The most recently
  pressed key that repeats becomes the repeat key. After `DELAY_NS` (500 ms) it repeats every `INTERVAL_NS`
  (1 s / 30).
- **What ends a repeat:**
  - release of that key;
  - any other press;
  - a modifier change;
  - a focus move (the existing `ACTIVATION_EPOCH`);
  - `MAX_HOLD_NS` (20 s), in case a break code is lost.
- **Keys that never repeat:** the modifiers, Caps/Num/Scroll Lock, Pause, Print Screen, Escape, Enter and keypad
  Enter. Enter is excluded because it submits an Ask, so holding it would queue several. **This is a policy
  choice; the PM may want Enter to repeat.**
- **Wire format.** A repeat is the key's press again with `flags` bit 1 set (`KEY_REPEAT` = 0x02, so flags =
  0x03). Clients that test only bit 0 see an ordinary press, so no client changes. The byte oracle is
  `wire_oracle_backspace_repeat_with_left_shift`: `01 03 2A 00 02 00 00 00`.
- **Where it runs.** The i8042 pump (`pump_routed_events`) and the USB keyboard pump (`Inner::drain`), under each
  driver's existing lock, never in an ISR. `SYS_WAIT_ANY` with an input item already re-pumps every tick, so a
  client waiting for keys gets repeats on tick edges. A client that has stopped reading (for example one busy
  generating) gets none generated for it by its own waits.
- **SAK.** A repeat bypasses the SAK filter, which is safe for two reasons:
  - it starts only from a press the filter passed;
  - a modifier change ends it, so it always carries that press's modifiers and can never become a Super chord.
- **No double repeat.** The i8042 still drops the EC's typematic makes. It now counts them as `typematic=`, so
  software is the only repeat source.
- **Queue rule.** In `input_queue_logic.rs`, a repeat joins a queue only if the queue has room and holds no
  unread repeat. So a stalled client can't build up a backlog of Backspaces, and no keystroke is ever evicted to
  make room for a repeat. The dropped repeat is counted as coalesced.
- **What a photo shows.** The watchdog `in:` line gains `ps2=` (i8042 keys), `rep=` and `typematic=`. The boot
  console prints `key-repeat: rep= typematic=` next to the i8042 counter line.
- **Shipping.** Enabled in image A (`xtask iso --with-model`) and in CI's `product-real-lm` clippy arm. It is not
  in `desktop` yet: no desktop scenario holds a key, but enabling it there is a separate decision.

## 3. Governance (I-14) and the pitfall bank

- **I-14(a).** The ISR is unchanged. The new code runs in syscall context and takes no lock of its own. Its state
  lives under the i8042 `STATE` lock and the USB device lock, both `IrqSafeMutex`, IF=0, as before. It takes no
  governance lock.
- **I-14(b) and (c).** There is no new loop, so liveness is unaffected (at most one repeat per pump call). There
  is no DMA, so memory isolation is unaffected.
- **CP-72 (edge-triggered line left asserted): N/A.** `handle_interrupt` and `drain_port` are untouched.
- **CP-73 (back-to-back IF=0 sections): N/A.** The change adds a few instructions inside the pump's existing IF=0
  section and no loop. The new scenario's wait loop keeps `run_ps2_keyboard`'s explicit IF=1 window.
- **CP-37 / CP-56 (mixed-IF locks): N/A.** There is no new lock. `stage_event`/`route_event` are called from the
  same sites and contexts as the key events already are.
- **CP-14 (lost wakeup): N/A.** There is no new park or wake edge. Repeats ride the existing per-tick re-pump.

## 4. Kill-switch and evidence

- **Feature off.** Nothing is compiled. The `.text` of the debug kernel ELF is identical to `main` for every set
  checked (see the PR's ready comment).
- **Host KATs:**
  - `tools/atrium-logic-tests/src/key_repeat_kats.rs`, 13 tests: delay, rate, no burst, cancels, key set, wire
    oracle, queue rule;
  - `tools/xtask/src/i8042_tests.rs`, `set1_typematic_makes_are_counted`.
- **QEMU scenario `ps2_key_repeat`.** HMP holds Backspace 1.5 s, then Escape 1.5 s, then types `x`. It requires:
  - the Backspace press;
  - at least 3 repeats, with repeat k (from 0) no sooner than press + 500 ms + k/30 s;
  - the release, with nothing repeating after it;
  - Escape and `x` exactly as press then release.

  The timing check is exact. `drain_events_stamped` reads the clock once per pass, gives that value to the
  state machine and stamps it on every event, so the scenario compares the very times the machine used. It
  rejects a first repeat at 490 ms and a run 30 ms apart (Chad, #106 review item 11). It does not hold
  adjacent repeats to 33 ms each: the clock is the 10 ms tick, so correct repeats land on tick edges 30 or
  40 ms apart. The KAT `repeat_k_never_before_500_ms_plus_k_thirtieths` pins both facts.

  The control, `neo-test-ps2-key-repeat-control`, never arms and must FAIL `key_repeat_none`. Local QEMU 8.2.2:
  PASS 5/5 (30–31 repeats, the first after 50 ticks). The control FAILs as designed.

## 5. The next Zenbook run: what each reading means

Hold Backspace in the Ask field for about 3 s, then photograph the bottom-left corner.

| Reading | Meaning |
|---|---|
| Text deletes continuously, `rep=` climbs | Fixed. |
| `ps2=` climbs on the press, `rep=0` | Nothing armed. Either the press was not delivered through the routed pump, or the focus moved (epoch). |
| `rep=` climbs, the screen does not repeat | The repeats are routed but not shown. Check the client's handling of a press with flags 0x03, and the queue rule. |
| `typematic=` climbs while held | The EC does send typematic; it is dropped, as designed. If it stays 0, the EC sends none (the USB-like case). |

## 6. Cross-family review checklist

**MODEL:** GPT-6-Luna. **EFFORT:** high. It is contained input-path code in one subsystem, with no SMP core change
and no new syscall.

Read the files at the head SHA. Answer each item pass or fail, with the enforcing `file:line`, and give a
concrete input for any fail.

1. **Key set.** `repeats()` returns false for 0xE0–0xE7, Caps Lock, Scroll Lock, Num Lock, Pause, Print Screen,
   Escape, Enter and keypad Enter, and true otherwise (`key_repeat_logic.rs`).
2. **Timing.** `KeyRepeat::poll` emits nothing before press + 500 ms, then at most one event per call. After a
   late call it does not emit a burst.
3. **Cancels.** The repeat ends on release of the repeat key, on any press, on a modifier change, on an epoch
   change and after 20 s. Release of a different key does not end it.
4. **i8042 pump.** `on_event` is called only for events the SAK filter passed (not for `Chord` or `Swallow`). A
   repeat is staged only when `routing` is true.
5. **USB keyboard.** `on_modifiers(report[0])` runs before each report's events. `on_event` runs after the SAK
   filter. The poll is gated on `input_routing_enabled()`.
6. **Ordering.** No repeat of a key can reach a client after that key's release. A repeat is emitted only after
   the decode loop, and a release in the same pass ends the repeat first.
7. **SAK.** A repeat can never be a Super chord: a repeat's modifiers always equal those of the press the filter
   passed.
8. **Queue rule.** `input_queue_logic.rs`: a repeat is never appended to a full queue, or to one that holds a
   repeat, and it never evicts an event. Non-repeat events behave exactly as before.
9. **Feature off.** Every new item and statement is behind `#[cfg(feature = "key-autorepeat")]` (the test hook
   `drain_events_stamped` behind its scenario's feature), so no code changes. `drain_events` is as on `main`.
10. **Locks and ISR.** The new code takes no lock, runs in no ISR, and touches only the `REPEATS`/`TYPEMATIC`
    atomics outside the driver's lock.
11. **Scenario.** The assertions in `run_ps2_key_repeat` (`test_harness.rs`) fail for:
    - a first repeat before press + 500 ms;
    - any repeat k earlier than press + 500 ms + k/30 s, exact at the stamps, with no slack;
    - a repeat after the release;
    - an Escape repeat.

    Per-pair spacing of 33 ms is not claimed: tick quantization makes correct gaps 30 or 40 ms.
12. **Wire format.** The wire oracle bytes match the `InputEventV1` layout in `input_logic.rs`.
