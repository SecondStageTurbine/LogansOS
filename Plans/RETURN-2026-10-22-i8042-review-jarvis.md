<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: i8042 keyboard driver review — Jarvis

**Branch:** `claude/i8042-keyboard` at `23a443e1796d72da70e059c7f016fef338d70304`
**Reviewed against:** `origin/main` (`4086a8ed`)
**Verdict: MERGE**

No checklist failures found. The line references below refer to the reviewed PR head. The scancode table was cross-checked against the IBM PC/AT Technical Reference’s Set 1 table and USB-IF HID Usage Tables page 0x07 ([IBM reference](https://bitsavers.org/pdf/ibm/pc/at/6183355_PC_AT_Technical_Reference_Mar86.pdf), [USB-IF HID Usage Tables](https://www.usb.org/sites/default/files/hut1_3_0.pdf)).

1. **PASS — ISR lock isolation.** `kernel/src/drivers/ps2/i8042.rs:195-215` uses a single `compare_exchange` claim and returns on contention; `drain_port` only performs port reads, atomics, and ring writes (`:164-193`). It does not touch `STATE`, `DELIVERY`, or governance state. The interrupt body calls it and sends EOI (`kernel/src/arch/x86_64/idt.rs:984-990`).

2. **PASS — ring correctness.** `HEAD` is written only inside the claimed producer (`i8042.rs:60-63, 183-204`); the consumer advances `TAIL` under `STATE` (`:338-357`). The byte store precedes Release publication and the consumer Acquire-loads `HEAD` (`:183-189, 338-344`). Full detection reads `TAIL` with Acquire and drops instead of overwriting (`:183-187`). `wrapping_add` handles index overflow (`:188-189, 344`).

3. **PASS — edge-triggered IRQ1 / CP-72.** `drain_port` empties OBF until clear or the counted 256-byte cap (`i8042.rs:164-192`). After enabling IRQ1, init immediately drains any already-pending byte (`:369-375`); the syscall pump polls as fallback (`:400-405`). The scenario requires every byte to have arrived via IRQ1 (`kernel/src/test_harness.rs:24703-24706`). FINDINGS §7 records 18/18 final passes and the earlier cap-related hangs.

4. **PASS — bounded/safe init.** All waits use `WAIT_SPINS` (`i8042.rs:43-45, 104-129, 131-153`). Initial status `0xFF` returns before any write (`:314-320`). Controller self-test and port-test timeouts continue while wrong replies abort (`:337-356`). No keyboard reset command exists; only `0xF4` is sent (`:11-13, 139-154`). AUX is disabled and its IRQ is cleared (`:319-320`; `i8042_logic.rs:41-45, 66-72`). Port constants are only `0x60` and `0x64` (`i8042_logic.rs:23-26`).

5. **PASS — config byte.** `config_quiesced` preserves system/reserved bits, clears both IRQ bits and port-1 clock-off, and enables translation; `config_running` adds only IRQ1 (`i8042_logic.rs:55-72`). The hand-calculated firmware-default `0x47` cases expect `0x44` then `0x45` (`tools/xtask/src/i8042_tests.rs:168-178`). Port 2 is separately disabled by controller command.

6. **PASS — Set-1 decode.** The base and E0 tables map standard Set-1 keys to HID page-0x07 usages (`i8042_logic.rs:103-145`), consistent with the cited references. Fake shifts are dropped (`:218-224`); Pause emits one press/release for its six-byte sequence (`:188-204`); repeated makes are suppressed by held-state (`:225-238`). Noise filters cover the listed bytes (`:208-216`); `0xAA` remains a left-Shift break and is dropped when Shift is not held (`:208-214, 225-238`).

7. **PASS — USB event compatibility/coexistence.** PS/2 tracks modifier usages as the boot-report modifier byte and emits no modifier events (`i8042.rs:341-359`); routed keys use `InputEventV1`, the same SAK handling, focus stage/legacy route, and flush (`:414-451`). USB’s matching decoder/event path and SAK/stage handling are at `kernel/src/drivers/usb/hid_keyboard.rs:250-286`. Both pumps run side by side at both receive sites (`kernel/src/arch/x86_64/syscall.rs:5554-5561, 7489-7496`). Init follows USB init (`kernel/src/main.rs:710-719`), ending firmware USB legacy ownership first.

8. **PASS — MADT parsing.** Type 1/2 layouts and offsets are documented and read at `kernel/src/acpi/madt_logic.rs:6-12, 70-95`; only bus 0 overrides are kept (`:84-94`). Polarity/trigger conforming and reserved encodings default correctly (`:99-116`). Length and bounds checks stop safely on truncated/zero-length entries (`:60-69`); fixed arrays cap records at 4 I/O APICs and 16 overrides (`:15-19, 75-86`). KATs cover fixture parsing, truncation, and route decoding (`tools/xtask/src/i8042_tests.rs:64-148`).

9. **PASS — I/O APIC programming.** Version reads follow `reg_base`’s MMIO mapping (`ioapic.rs:60-63, 89-96`). The selected entry is written masked-low, high, then final low (`:101-109`); encoding is fixed delivery/physical destination with trigger/polarity (`ioapic_logic.rs:34-57`). BSP APIC IDs above 255 are refused (`ioapic.rs:97-100`), and GSI-to-controller/pin selection uses the discovered pin count (`:89-96`; `madt_logic.rs:119-126`). Only that pin is changed (`ioapic.rs:101-109`). The PIC IRQ lines remain masked by existing `pic::init`, as noted in `ioapic.rs:11-13`.

10. **PASS — vector and EOI.** IRQ1 uses `0xF5` (`i8042.rs:40-41`), outside the documented MSI/IPI/spurious allocations (`idt.rs:353-385`); its assembly entry is the shared `swapgs_irq_entry` (`:357-372, 500-506`). The body sends EOI after the handler (`:984-990`).

11. **PASS — kill-switch.** `ps2-keyboard` is an empty, default-off feature (`kernel/Cargo.toml:125-132`); `desktop-session` enables it, and therefore `desktop` does too (`:125, 153-159`). The driver, ACPI additions, IOAPIC modules, IDT gate/body and stub are feature-gated (`kernel/src/drivers/mod.rs`, `kernel/src/acpi/mod.rs:34-37, 290-304`, `kernel/src/arch/x86_64/mod.rs`, `kernel/src/arch/x86_64/idt.rs:254-256, 357-372, 520-521`). `product` does not enable `desktop-session` (`Cargo.toml:110-116, 153-159`), preserving the default/product behavior.

12. **PASS — coverage / CP-72 / CP-73.** The `ps2_keyboard` scenario expects 14 exact transitions, checks IRQ byte counts and decode equality, and its control is documented to fail with translation disabled (`kernel/src/test_harness.rs:24615-24707`; `kernel/Cargo.toml:524-529`). FINDINGS §7 records 18/18 final passes, 0 polled bytes, and the control failure. The revised loop includes a TSC deadline and an IF=1 spin window between drains (`test_harness.rs:24643-24670`), addressing CP-73; FINDINGS §5-7 describes the starvation discriminator and fix. FINDINGS §6’s listed Zenbook failure signatures are consistent with the status/counter paths; I did not identify a likely missing failure class.

**Notes:** This is a source review of the named head; I did not rerun QEMU or host tests. FINDINGS documents the reported runs and test outcomes.
