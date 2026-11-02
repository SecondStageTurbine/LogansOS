<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FINDINGS: i8042 / PS/2 keyboard field traps and their discriminators

**Author:** Seat I. **Date:** 2026-11-02.
**Why this exists:** `kernel/src/drivers/ps2/i8042_logic.rs` says its field traps come from "the FINDINGS note". This is
that note. The cross-family review of #53/#54 (relay 269) found that it was missing.
**Covers:**
- the i8042 driver (`kernel/src/drivers/ps2/i8042.rs`, `i8042_logic.rs`);
- the shared US keymap and lock state (`kernel/src/drivers/keymap_us_logic.rs`, `key_locks.rs`, PR #53);
- the keypad (PR #54).

**Sources, read for the principle only (no code copied, GPL code never copied into this MPL tree):**
- Linux `drivers/input/serio/i8042.c` and `drivers/input/keyboard/atkbd.c`;
- the IBM PC/AT technical reference;
- OSDev "8042 PS/2 Controller" and "PS/2 Keyboard";
- the USB HID Usage Tables (Keyboard/Keypad page 0x07).

Each trap below gives what goes wrong in the field, what we do, and the **discriminator** that proves it: a host KAT,
a QEMU scenario, or a serial line that tells the cases apart on a failing run.

## Controller (i8042)

| # | Trap | What we do | Discriminator |
|---|---|---|---|
| 1 | **Translation.** The controller translates the keyboard's set 2 into set 1 when config bit 6 is set. Reading set 2 as set 1 gives the wrong keys, not missing ones. | Translation stays on (the firmware default) and the decoder reads set 1. We never send the keyboard `F0` (set select): EC-emulated keyboards (the Zenbook's `ATK3001`) may ignore or garble it. | `ps2_keyboard` with `neo-test-ps2-keyboard-control` (translation off) must FAIL `ps2_keyboard_decode_mismatch`. KAT `i8042_config_byte_transform` pins bit 6. |
| 2 | **Self-test can reset the config byte.** On some controllers `AA` resets the configuration (Linux rewrites its CTR after it). | `bring_up` writes the quiesced config, runs the self-test, then **writes the config again** before testing port 1. | The `i8042: present=… selftest=… port1=… kbd=…` probe line names the step that failed. |
| 3 | **Mouse bytes share port 0x60.** Status bit 5 marks a byte from the AUX port; taken as a key byte, it decodes as garbage keys. | `classify(status)`: AUX bytes and parity/timeout errors are discarded and counted. | KAT `i8042_status_classification`. The `discarded=` counter on the `i8042:` status line. |
| 4 | **IRQ1 routing.** Boards override ISA IRQ1 in the MADT, sometimes level-triggered active-low. Ignoring the override loses every interrupt. | IRQ1 goes through the I/O APIC with the MADT override's polarity and trigger. | KATs `madt_parses_ioapic_and_overrides` and `isa_irq_routing_honours_overrides` (with and without an IRQ1 override). `ps2_keyboard` requires `irqs > 0`, `polled == 0` and every byte via IRQ1. |
| 5 | **Lost interrupts under TCG.** Back-to-back IF=0 sections plus the STI shadow can starve IRQ1, and QEMU's 16-byte PS/2 queue then overflows. | The test loop leaves a real IF=1 window between drains. The production pump also polls (`poll_port`) and counts what it polled. | The `ps2_keyboard diag … lapic_isr/irr` line, and the `polled=` counter. |
| 6 | **Keys typed before the desktop.** Bytes queued during boot screens were delivered to Home's first route (an Enter among them submitted it). A break lost to a full ring left a key held. | `tail_after_cut`: bytes received before the first keyboard route are dropped and the decoder starts afresh. | KATs `boot_cut_skips_only_what_came_before_the_route` and `boot_keys_never_reach_the_first_route`. The `i8042: dropped N pre-desktop bytes` line. |

## Keyboard byte stream (set 1 through translation)

| # | Trap | What we do | Discriminator |
|---|---|---|---|
| 7 | **Replies mixed with keys.** `FA` (ACK), `FE` (resend), `EE` (echo), `00`/`FF` (overrun/error) and `FC`/`FD` (self-test failures) can arrive between keys. | The decoder drops them and clears a pending `E0`. | KAT `set1_typematic_repeat_and_noise_are_not_keys`. |
| 8 | **`AA` means two things.** It's left Shift's break code and the keyboard's self-test pass byte. | Not filtered. The driver never resets the keyboard, and a stray `AA` is a break for a key that isn't held, which the held-set check drops. | The same KAT, plus `ps2_keyboard`'s Shift events. |
| 9 | **Typematic repeat.** A held key repeats its make code. Reported as presses, it would be a second repeat source on top of software auto-repeat. | Held-set dedupe: a make for a held key isn't a press. It's counted (`take_typematic`) for `key-autorepeat`. | KATs `set1_typematic_repeat_and_noise_are_not_keys` and `set1_typematic_makes_are_counted`. `ps2_key_repeat` (with control). |
| 10 | **Fake shifts.** The keyboard wraps the grey navigation keys, Print Screen and keypad `/` in `E0 2A` / `E0 AA` (and `E0 36` / `E0 B6` with right Shift), depending on its own Shift and Num Lock state. Taken as Shift, they change the case of the next key and can leave Shift stuck. Linux `atkbd` ignores them too. | `E0 2A`, `E0 AA`, `E0 36` and `E0 B6` are not keys; only a real `2A`/`36` (no prefix) is Shift. | KAT `set1_decodes_to_hid_usages` (Print Screen wrapped in fake shifts). KAT `ps2_set1_keypad_bytes` (#54: grey Home wrapped in fake shifts gives one Home press and no Shift). |
| 11 | **Pause has no break code.** It's the six-byte `E1 1D 45 E1 9D C5`, which contains 0x45 (Num Lock's make). A decoder that doesn't swallow the sequence whole reports a phantom Num Lock and a stuck Ctrl. | The `E1` sequence is swallowed whole. The press is reported at the second `E1` and the release at the last byte. | KAT `set1_decodes_to_hid_usages` (Pause). `ps2_keyboard` sends `pause`, and the Num Lock state stays on (default) through it (#54 expects lock state 0 on every event before its own Num Lock press). |
| 12 | **Grey keys and the keypad share codes.** KP7 is `47` and Home is `E0 47`, and likewise for the arrows, PgUp/PgDn, Ins/Del and End. Dropping the prefix turns arrows into digits. | Separate tables: `SET1` (no prefix) and `set1_extended` (`E0`). | KAT `set1_decodes_to_hid_usages` (Up = `E0 48`). KAT `ps2_set1_keypad_bytes` (#54: `47` gives keypad 7, `E0 47` gives Home). |
| 13 | **Keypad `/` and keypad Enter carry `E0`**, unlike the other keypad keys: `E0 35` and `E0 1C`. Without the prefix they read as `/` and Enter on the main block. | `set1_extended` maps them to usages 0x54 and 0x58. Callers treat 0x58 as Enter. | KAT `ps2_set1_keypad_bytes`. `ps2_keyboard` sends `kp_divide` and `kp_enter` (#54). |
| 14 | **The keyboard doesn't own Num Lock or Caps Lock.** PS/2 keys send the same codes whatever the lock state (apart from fake shifts); the host keeps the state and the LEDs. | One host-side lock state for every keyboard (`key_locks`, PS/2 and USB alike), stamped on each key event's `y`. LEDs aren't driven yet (no `ED` command); see "Open" below. | KATs `lock_keys_toggle_on_each_press_only`. `ps2_keyboard` checks the lock state on every event (Caps on and off, Num Lock off and on). |
| 15 | **Shift with the keypad.** On a PC, Shift inverts Num Lock for the keypad digit keys (Shift+KP8 is Up with Num Lock on). | `effective_usage` / `printable` apply the inversion. Operators ignore Shift and Num Lock. | KAT `keypad_under_num_lock_shift_and_caps` (#54). |
| 16 | **Lock state must survive auto-repeat.** A repeat that drops the press's lock state types lower case under Caps Lock. | A repeat carries the press's `y`. A lock key's press ends any repeat, so the state can't change mid-run. | KAT `wire_oracle_repeat_carries_the_press_lock_state`. |

## Open (not a trap we've hit; recorded so it isn't rediscovered)

- **LEDs.** Setting the Caps/Num Lock LEDs on PS/2 means sending `ED <mask>` and waiting for two ACKs from the pump,
  which takes an `FA`/`FE` exchange interleaved with key bytes (Linux `atkbd` serialises it in a work item). That needs
  a command queue in the driver, so it's left out, and the lights show whatever firmware left. USB needs a HID
  SET_REPORT on endpoint 0.
- **Set-2-only keyboards** (translation forced off by firmware): the control scenario shows what that looks like. A
  real one would need set 2 tables, so we'd decide then.
- **J.c.'s Shift+`;` report (2026-11-02).** It isn't reproduced under QEMU HMP on either path (#53). If it recurs, the
  `ps2_keyboard` per-event `modifiers=` lines, or the same output from a real keyboard, separate "Shift never arrived"
  (a host or frontend problem) from a decode problem.
