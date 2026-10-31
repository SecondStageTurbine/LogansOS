<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Zenbook touchpad rung 1 (I2C-HID over Serial-IO I2C), cross-family review

- **To:** a Codex-family seat (the PM picks). The author, S3, is Claude.
- **MODEL:** GPT-6-Luna · **EFFORT:** high. It is one contained subsystem: no SMP core, no new syscall. It does
  add a level-triggered IRQ path, the first run-time I/O APIC writes, and a kernel thread.
- **Branch under review:** `claude/zenbook-i2c-hid-touchpad`, at the head named in its PR.
- **Your RETURN:** `Plans/RETURN-2026-10-23-zenbook-i2c-hid-touchpad-review-<seat>.md`.
  - Put it on branch `codex/zenbook-i2c-hid-touchpad-review-<seat>`, cut from `origin/main`, containing
    only that file.
  - Push it. A review that exists only in chat does not count.
- **Verdict:** SOUND, SOUND-WITH-NITS, or BOUNCE.
  - Each item is PASS or FAIL, with the enforcing `file:line`.
  - A FAIL names a concrete input or sequence.
  - An uncertain item is a note, not a FAIL.

## Read first

```
git fetch origin
git show origin/claude/zenbook-i2c-hid-touchpad:Plans/DISPATCH-2026-10-23-zenbook-i2c-hid-touchpad-review.md
git show origin/main:Plans/DISPATCH-2026-10-23-zenbook-i2c-hid-touchpad.md
git show origin/claude/zenbook-i2c-hid-touchpad:docs/RESEARCH-2026-10-23-zenbook-i2c-hid-touchpad-FINDINGS.md
git diff origin/main...origin/claude/zenbook-i2c-hid-touchpad --stat
```

The Zenbook DSDT is `fixtures/acpi/zenbook-s14-ux5406aa-303/dsdt.dat`. It is on `main` once PR #96 merges;
before that, read it from `origin/claude/acpi-aml-rung0`. To check §2's offsets, disassemble it with ACPICA
`iasl -d` and read the raw bytes at each offset (`xxd -s <off> -l 16`).

Core files:
- `kernel/src/drivers/i2c_hid/touchpad.rs` (match, bring-up, ISR, service thread, ring, pump, console);
- `kernel/src/drivers/i2c_hid/dw_logic.rs` (LPSS + DesignWare, polled);
- `kernel/src/drivers/i2c_hid/hid_logic.rs` (HID over I2C, report descriptor, `LineGovernor`);
- `kernel/src/drivers/i2c_hid/quirk_logic.rs` (the quirk table and its match);
- `kernel/src/arch/x86_64/ioapic.rs` (`route_gsi`, `GsiPin`);
- the vector and stub in `kernel/src/arch/x86_64/idt.rs`;
- the call sites in `kernel/src/main.rs` and `kernel/src/arch/x86_64/syscall.rs`;
- `tools/i2c-hid-logic-tests/` (the host KATs and the register model).

## Checklist

1. [ ] **The quirk values equal the DSDT bytes.**
   - Each row of the findings §2 table names a value and a DSDT offset; check each against the fixture's raw
     bytes, not the code.
   - Slave address 0x15, 400 kHz, controller `\_SB.PC00.I2C5`, GSI 0x69 with flags 0x15 (level, active-low,
     wake), `_DSM` function 1 = `One`, I2C5 `_ADR` = 0x00190001.
   - `ZENBOOK_TOUCHPAD_CRS` is byte-for-byte the template at 0x676B3.
2. [ ] **A non-matching machine is never touched.**
   - Before all DSDT conditions hold, `init` does no PCI config read, no MMIO and no I/O APIC write.
   - Every non-`Match` verdict returns before `bring_up`.
   - The forced match exists only under `neo-test-i2c-hid-absent-control`.
3. [ ] **The match is specific.**
   - `inspect_dsdt` requires the template inside `Device (ETPD)`, with a PkgLength that covers it.
   - It requires the `_DSM` constant to come after the HID UUID inside that device.
   - It requires the OEM pair.
   - Check that the PkgLength decode (ACPI 6.5 §20.2.4) and the integer decode are right.
   - Is there a plausible firmware that matches all of this and is not this touchpad? (Note, not FAIL, unless
     you find a concrete one.)
4. [ ] **PIO only (I-14(c)).**
   - The LPSS resets register is written 0, then 0x3, and never with bit 2.
   - The read-back is checked, with a refusal if bit 2 is set.
   - `IC_DMA_CR` is written 0.
   - PCI bus mastering is cleared and checked.
   - No LPSS DMA register is programmed anywhere.
5. [ ] **DesignWare configuration.**
   - `IC_CON` = 0x65.
   - `IC_TAR` and the SCL counts are written only while `IC_ENABLE_STATUS` reads disabled.
   - The counts (78/150 at 100 MHz) follow the stated I2C-bus minimums. Check the arithmetic and whether
     100 MHz is right for `8086:E351`.
   - The SDA hold keeps the RX field.
   - `IC_INTR_MASK` is 0: the DesignWare interrupt is never used.
6. [ ] **The transfer sequencer.**
   - The first read after a write carries `RESTART`, and only the last command carries `STOP`.
   - The TX FIFO is never overfilled.
   - Outstanding reads never exceed the RX depth.
   - An abort reads and decodes `IC_TX_ABRT_SOURCE` and clears it.
   - Every loop has a deadline; a timeout aborts and disables.
   - `received == read.len() && issued == total && stopped` cannot accept a NAK'd message.
   - The register model (`tools/i2c-hid-logic-tests/src/model.rs`) is faithful where it matters: the
     enable-status lag, TX flushed until `IC_CLR_TX_ABRT`, STOP_DET.
7. [ ] **HID over I2C.**
   - The descriptor offsets and checks match the spec (§5.1.1).
   - The RESET and SET_POWER bytes are `[reg lo, reg hi, 0x00|state, opcode]`.
   - The input length counts itself.
   - 0 is the reset ack, and 0xFFFF is never decoded as a report.
   - The reset order: SET_POWER ON, 60 ms, RESET, 1 s for the ack, then SET_POWER ON again (skipped for
     VID 0x04F3).
8. [ ] **Report descriptor walk and decode.**
   - Global/local item handling, Usage Min/Max, and 4-byte extended usages.
   - The mouse is found only inside a Generic Desktop / Mouse Application collection.
   - Bit offsets are kept per report ID, and constant fields advance the offset.
   - Sign extension follows the logical minimum.
   - A Digitizer report is counted, not misparsed.
   - What a real precision touchpad's descriptor would break here: Push/Pop or long items are refused with a
     named error (PASS if the refusal is named on the console).
9. [ ] **The ISR takes no lock (I-14(a)).**
   - `i2c_hid_interrupt_body` → `handle_interrupt`: one `GsiPin::set_masked(true)` and two atomic stores.
   - `PIN` is read through `Once::get`.
   - Nothing it calls takes `STATE`, `BUS_OWNER`, `BOOT_LINES`, `DELIVERY` or an AI-governance lock.
   - `isr_with_driver_locks_held_for_test` really runs the body with those held and IF=0.
10. [ ] **Level-triggered handling (CP-75).**
    - The line is masked from the ISR until the thread has read once.
    - There is no path where the thread unmasks without a read, except the first unmask after the reset's
      read.
    - A still-asserted line re-fires, so no wake is lost (`PENDING.swap`).
    - The storm and error limits (`LineGovernor`) end in masked polling or a stopped reader, never a spin.
    - The polled modes leave the line masked.
11. [ ] **I/O APIC writes (CP-76).**
    - After boot, only `GsiPin::set_masked` writes the I/O APIC: the ISR while the pin is unmasked, the
      thread while it is masked and with IF=0.
    - No other runtime writer exists: grep `ioapic::` users.
    - The `read_low` diagnostic runs only while the pin is masked.
    - `route_gsi` programs the entry masked.
12. [ ] **Ring and pump.**
    - One producer (the service thread) and one consumer (the pump, under `STATE`; or the boot check before
      any pump runs).
    - Release/Acquire are on the right indices.
    - Full ⇒ dropped and counted.
    - The pump delivers exactly as `hid_mouse.rs` does (`note_raw_buttons`, `stage_event` /
      `flush_pointer_staged`, or `route_event` under the legacy stage).
    - The USB mouse path is unchanged.
13. [ ] **Liveness (I-14(b)).**
    - Every bring-up wait is TSC-bounded, and none relies on the tick before it is known to run.
    - The thread naps between passes and handles at most 64 assertions per pass.
    - A wedged bus costs bounded time per transfer and stops after 8 in a row.
14. [ ] **Kill-switch.**
    - With `i2c-hid-touchpad` off, every added item is `cfg`-gated: the module, the IDT entry and asm stub,
      `route_gsi`, `GsiPin`, `dsdt()`, the PCI and APIC helpers, the pump calls and the main.rs calls.
    - The widened cfgs (`ps2-keyboard`, `xhci-silicon`, the font) compile the same items as before for every
      old feature set.
    - The findings §7.3 identity table matches what you rebuild for at least `desktop`.
15. [ ] **The scenario and its control.**
    - `i2c_hid_absent` fails if anything was probed.
    - It checks the no-controller timeout against the handshake budget, from both sides.
    - The control forces the match, and must reach `i2c_hid_probed_without_match` without hanging.
    - Is the RAM page standing in for BAR0 a fair test of the real MMIO path? (It uses the same `Mmio`
      type.)
16. [ ] **The failure bank (§6) is conclusive.**
    - Each listed mode prints a distinct line.
    - Image A's touchpad check re-prints the bring-up lines, so one photo shows the failing step.
    - Name any mode that would leave the screen ambiguous.
