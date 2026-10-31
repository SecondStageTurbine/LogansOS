<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FINDINGS: the Zenbook S14 touchpad, rung 1 (I2C-HID over Serial-IO I2C, mouse mode)

**Date:** 2026-10-23 · **Author:** S3 · **Dispatch:** `Plans/DISPATCH-2026-10-23-zenbook-i2c-hid-touchpad.md` ·
**Branch:** `claude/zenbook-i2c-hid-touchpad` · **Driver-coverage tier:** 4

**Why.** The Zenbook's built-in touchpad does nothing: Windows lists it as `ACPI\ASCE1206` "ASUS Precision
Touchpad", an I2C-HID device on Intel Serial-IO I2C (`8086:E351`), and the kernel has no I2C driver.

## 0. Result

- **Feature `i2c-hid-touchpad` (the kill-switch).** Off ⇒ nothing compiled, not even the IDT entry stub. The
  `.text` of `default`, `desktop`, `product` and image A's feature set is byte-identical to `main` (§7.3).
- **It is added to image A** (`xtask iso --with-model`) and to CI's `product-real-lm` matrix row, so the next
  Zenbook image carries it. On any machine its quirk entry does not match, it prints one line and touches
  nothing. **This is a PM call**: dropping the line in `tools/xtask/src/main.rs` takes it back out.
- **Matched, then brought up.** A machine qualifies only if its DSDT describes this exact touchpad (§2) and
  PCI 00:19.1 is `8086:E351`. The driver is PIO only:
  - the LPSS DMA engine stays in reset;
  - bus mastering is cleared;
  - every wait is bounded.
- **Rung 1 is mouse mode.** The touchpad's mouse collection is read from the HID report descriptor, and its
  reports become the USB mouse's pointer events (`InputEventV1::pointer`), through the same stage and flush.
  The USB mouse path is unchanged.
- **The interrupt** is the `_CRS`'s `Interrupt` descriptor: GSI 0x69, level, active-low. It is an APIC
  interrupt, **not GpioInt**, so no GPIO driver is needed. The ISR masks the line and signals. A kernel
  thread reads the report and unmasks. A storm leaves the line masked and falls back to polling. A dead
  route also falls back to polling, and the console says so.
- **On-screen evidence.** Each bring-up step prints an `i2c-hid:` line. In image A, a **TOUCHPAD CHECK**
  follows the USB summary and is the last thing on the boot console. Every failure mode in §6 can be told
  apart from one photo.
- **Evidence:**
  - host KATs: `cargo test -p i2c-hid-logic-tests`, 28/28, including the real corpus DSDTs;
  - `i2c_hid_absent` PASSES on QEMU, and its forced-match control FAILs as designed (§7).
- **Every dispatch assumption held** (§2): an APIC `Interrupt`, controller at 00:19.1, and a descriptor
  register that is a static constant (`Return (One)`).

## 1. What changed

| File | What |
|---|---|
| `kernel/src/drivers/i2c_hid/quirk_logic.rs` (new, pure) | `ZENBOOK_S14_QUIRK` (the `zenbook_s14_quirk` table) with a DSDT citation per value; the DSDT inspection (`_CRS` template inside `Device (ETPD)`, `_DSM` function 1's constant) and the verdict |
| `kernel/src/drivers/i2c_hid/dw_logic.rs` (new, pure) | LPSS private registers; the DesignWare register map; SCL counts; `configure`; the polled write-then-read sequencer with bounded waits and abort decode |
| `kernel/src/drivers/i2c_hid/hid_logic.rs` (new, pure) | HID-over-I2C descriptor, commands, input framing; the report-descriptor walk (mouse collection, report IDs and their kinds); the mouse decode; the service thread's line policy (`LineGovernor`) |
| `kernel/src/drivers/i2c_hid/touchpad.rs` (new) | match → PCI/LPSS/DesignWare bring-up → HID descriptor, report descriptor, power, reset; the ISR; the service thread; the SPSC ring; the pump; the console lines; image A's touchpad check; test hooks |
| `kernel/src/arch/x86_64/ioapic.rs` | `route_gsi` (explicit polarity/trigger, routed masked) and `GsiPin` (mask/unmask, read-back) |
| `kernel/src/arch/x86_64/idt.rs` | vector `0xF6` with a `swapgs_irq_entry` stub (feature-gated); body = ISR + LAPIC EOI |
| `kernel/src/acpi/mod.rs` | `dsdt()` (via the FADT's `X_DSDT`/`DSDT`) |
| `kernel/src/drivers/pci.rs` | `device_at`, `disable_bus_mastering`; the PMCSR helpers' cfg widened |
| `kernel/src/arch/x86_64/apic.rs` | `spurious_vector_register()` (for the EOI-broadcast-suppression bit on the console) |
| `kernel/src/main.rs`, `syscall.rs` | init after the i8042, the service thread after the USB port service, the pump beside the USB mouse's (both call sites), the touchpad check after the USB summary |
| `kernel/src/drivers/fb/font.rs` | the extra glyphs also compiled with `fb-progress-console` + `i2c-hid-touchpad` |
| `tools/i2c-hid-logic-tests` (new) | 28 host KATs (§7.1) |
| `kernel/src/test_harness.rs`, `tools/xtask/src/main.rs` | the `i2c_hid_absent` scenario and its control; image A's feature list |

## 2. The quirk table (DSDT citations)

Source: `fixtures/acpi/zenbook-s14-ux5406aa-303/dsdt.dat` (BIOS UX5406AA.303, 423 950 bytes; rung-0 corpus,
PR #96). The DSDT was disassembled with ACPICA `iasl` 20230628, and each value was then read as raw bytes at
the offset given. `quirk_tests::the_zenbook_corpus_dsdt_matches_and_every_citation_holds` re-reads every
offset from the fixture.

| Value | Bytes at the offset | Offset | AML |
|---|---|---|---|
| OEM ID / table ID `_ASUS_` / `Notebook` | header | 0x0A / 0x10 | (every ASUS notebook: it narrows, it does not identify) |
| I2C5 `_ADR` = 0x00190001 (PCI 00:19.1) | `A4 0C [01 00 19 00]` | 0xFFF5 | `Method (_ADR) { Return (0x00190001) }` |
| `Device (ETPD)`, PkgLength 0x12B | `5B 82 4B 12 45 54 50 44` | 0x675B7 | `Scope (_SB.PC00.I2C5) { Device (ETPD) ...` |
| `_CRS` template (44 bytes) | `8E 1E 00 01 00 01 02 00 00 01 06 00 80 1A 06 00 15 00 "\_SB.PC00.I2C5" 00 89 06 00 15 01 69 00 00 00 79 00` | 0x676B3 | `I2cSerialBusV2 (0x0015, ControllerInitiated, 0x00061A80, AddressingMode7Bit, "\\_SB.PC00.I2C5", ...)` + `Interrupt (ResourceConsumer, Level, ActiveLow, ExclusiveAndWake) { 0x69 }` |
| slave address 0x15 | `15 00` | 0x676C3 | |
| speed 400 000 Hz | `80 1A 06 00` | 0x676BF | |
| GSI 0x69 (105), flags 0x15 | `89 06 00 15 01 [69 00 00 00]` | 0x676D9 | flags: bit 0 consumer, bit 1 = 0 level, bit 2 active-low, bit 4 wake |
| `_DSM` UUID `3cdff6f7-4267-4555-ad05-b30a3d8938de` | `F7 F6 DF 3C 67 42 55 45 AD 05 B3 0A 3D 89 38 DE` | 0x67652 | `ToUUID (...)` |
| **HID descriptor register = 0x0001** | `93 6A 01 A4 [01]` | 0x6767E | `If ((Arg2 == One)) { Return (One) }` |

**The controller.** `8086:E351` is 00:19.1 in the Windows inventory (`...&C9` = device 0x19, function 1). That
matches I2C5's `_ADR`. `8086:E350` at 00:19.0 is I2C4, the touchscreen's controller (`TPL0`, `ELAN9008`, GSI
0x6C: rung 3).

**Trap 9 (NVS-selected identity) checked.** `_HID` picks ASCE1206, ASCF1206 or ELAN1010 from NVS (`TPDI`,
`TPHI`). `_DSM` function 1 returns `One` unconditionally, so the descriptor register is the same for all
three. `_CRS` is a constant `ResourceTemplate` with no NVS field, so it is the same for all three as well.
`_STA` reads NVS (`TPIF`), but the Windows inventory proves the device is present on the PL's machine.

**`FMCN` is not static.** `\_SB.PC00.I2C5.FMCN` returns `PKG3 (FMH5, FML5, FMD5)`, which are fields of the
`GNVS` SystemMemory region (`OperationRegion (GNVS, SystemMemory, 0x5CAA5000, 0x0B31)`), filled by firmware
at boot. So the SCL counts are computed from the LPSS input clock (§3, T4) and printed.

**How the kernel matches** (`quirk_logic::inspect_dsdt`, `verdict`):
1. The DSDT header's OEM ID and table ID.
2. The exact 44-byte template, which must sit inside a `Device (ETPD)` whose PkgLength covers it.
3. Inside that device, the HID UUID followed by `LEqual (Arg2, One)` then `Return`, whose operand must decode
   to the integer 0x0001.
4. Only then, PCI 00:19.1 must be `8086:E351`.

The match is by pattern, not by offset, so a BIOS update that only moves the AML still matches. The offsets
above are citations. The host tests run the matcher on every DSDT in the rung-0 corpus. The Zenbook matches at
exactly these offsets. None of the other eight sets has the template at all: HP, two Lenovo, MSI, two
OptiPlex, three QEMU.

When ACPI Fix 3 lands, the AML path evaluates `_CRS` and `_DSM`, and the quirk entry becomes its cross-check
(the values must be equal).

## 3. Prior-art trap-mining (principle only; no code copied)

Sources read:
- the Microsoft HID-over-I2C specification v1.0, and HID 1.11 (with its Appendix E examples);
- ACPI 6.5 §6.4.3 (resource descriptors) and §20 (AML encoding);
- the I2C-bus specification UM10204;
- the DesignWare DW_apb_i2c register map, and the Intel Serial-IO register layout as used by every LPSS
  driver;
- Linux `drivers/mfd/intel-lpss{,-pci}.c`, `drivers/i2c/busses/i2c-designware-*`,
  `drivers/hid/i2c-hid/i2c-hid-{core,acpi}.c`, and FreeBSD `ig4`/`iichid`. These were read for their
  register names, platform clock table and quirk lists. They are GPL/BSD: no code was taken, and every
  routine here is written from the register semantics.

| # | Trap | Guard here |
|---|---|---|
| T1 | LPSS held in reset or clock-gated after firmware hand-off: writes ignored, reads 0 or all-ones | Resets register written 0 then function-out-of-reset (bits 1:0). Before/after printed (`lpss reset=…->…`). All-ones refused (`bar0=0xffffffff`). The clock-parameters register is printed, not written: Linux registers no divider for LPSS I2C, whose clock is fixed |
| T2 | Device in D3hot: BAR reads all-ones until D0 | PMCSR → D0, then 10 ms (PCI PM 1.2), printed `pmcsr=D3->D0` |
| T3 | `IC_ENABLE` → `IC_ENABLE_STATUS` lag; config writable only while disabled | Every enable/disable polls the status, bounded at 25 ms (at a 10 GHz TSC bound). The register model flags any configuration write made while enabled (KAT `enable_status_lag_is_waited_out`) |
| T4 | SCL timing: wrong HCNT/LCNT ⇒ NAKs or a hung bus; ACPI `FMCN` preferred by Linux | `FMCN` is NVS-backed (§2), so it is not usable statically. The counts are computed from the Panther Lake LPSS I2C clock, 100 MHz (Linux's `ehl_i2c_info` for `0xe350`/`0xe351`), the I2C-bus minimums and a 208 ns fall time: HCNT 78, LCNT 150, SDA hold 4. KAT'd, and printed `fs-hcnt= fs-lcnt=` |
| T5 | TX underrun / missing RESTART: a write-then-read becomes STOP+START, which some devices reject | One message per transfer. The first read carries `RESTART` and only the last command `STOP`. The TX FIFO is fed while there is room, with at most an RX FIFO's worth of reads outstanding. The model flags a read-after-write without RESTART and any FIFO overflow (KATs `write_then_read_uses_restart_and_one_stop`, `a_long_read_never_overflows_either_fifo`) |
| T6 | Abort sources: 7-bit address NAK (wrong address, unpowered device), arbitration lost; a sleeping device NAKs its first request | `IC_TX_ABRT_SOURCE` read and decoded (`abort=0x… (addr-nak)`). The descriptor read and SET_POWER are retried once after 1 ms on an address NAK (Linux retries after 400–500 µs for STM/Weida parts) |
| T7 | HID-over-I2C RESET: the device asserts and returns a zero-length read; input read before that is stale. Some need a delay after SET_POWER ON (Windows 1 ms, Goodix 60 ms). ELAN breaks if SET_POWER ON follows RESET, and may answer an IRQ with length 0xFFFF | Stale input drained while masked (`stale=`). SET_POWER ON, then 60 ms, then RESET with the line unmasked, then up to 1 s for the IRQ and a zero-length read (`reset-ack=irq/polled(no-irq)/irq-with-data/timeout`). SET_POWER ON again with 60 ms, except for VID 0x04F3 (ELAN). 0xFFFF is counted as `bogus`, never a report |
| T8 | Level-triggered interrupt storm if the report is not drained before unmask | Mask in the ISR. The thread reads, then unmasks, and an asserted line re-fires. 16 assertions in a row without data ⇒ stay masked, poll (`irq masked-storm`). KAT `line_governor_masks_a_storm_and_stops_on_errors`. New pitfall **CP-75** |
| T9 | `_STA`/NVS-selected identity; don't assume the ELAN variant's descriptor address | §2: the register is the same constant for all three identities |
| T10 | Touchpad may boot in PTP mode | Report IDs are classified from the descriptor's collections (`ids=1m,3d,…`). A Digitizer report is counted as `unexpected=`, an undeclared one as `unknown=`, and neither is misparsed. KAT `composite_descriptor_finds_the_mouse_and_names_the_digitizer` |
| T11 (found) | LPSS iDMA: Linux takes it out of reset with the function. A driver that copies that runs with a live DMA engine | Only bits 1:0 are written. The read-back must show bit 2 clear, or the driver puts the block back in reset and refuses (I-14(c)) |
| T12 (found) | Firmware may leave PCI bus mastering on | Cleared, and the read-back must show it clear or the driver refuses |
| T13 (found) | EOI broadcast suppression (LAPIC SVR bit 12, preserved from firmware by `apic.rs`): a level entry's remote IRR is never cleared by the local EOI, so the line delivers once and never again | Detected and printed (`eoi-suppress=`, `rirr=`). Not mitigated in rung 1: the symptom is `count=1` and no reports, with the polled fallback not engaged. Needs the I/O APIC's directed EOI (version ≥ 0x20) if ever seen |
| T14 (found) | The I/O APIC's `IOREGSEL`/`IOWIN` pair is two writes; two writers interleave | Single writer per phase (§5, new pitfall **CP-76**) |
| T15 (found) | A wrong polarity looks like a line that is always asserted | It shows as an immediate `masked-storm` with `empty=16`, then polling |
| T16 (found) | Reading the input register with no interrupt pending: some devices NAK or repeat the last report | Only in polled mode. Counted (`errors=`, or repeated motion). The console names the mode, so polled-mode oddities are attributable |
| T17 | SDA held low by an interrupted firmware transfer | No GPIO bus recovery in rung 1. It shows as `abort=… (arb-lost)` or `timeout (Transfer)`, followed by the abort and disable |

**Easier-path question (CLAUDE.md):** is there a class that does the hard part for us? Partly: the PTP's mouse
collection is that class. It gives relative motion and buttons with no gesture processing. Precision mode
(rung 2) is the hard part, and is deferred. THC (the Touch Host Controller) is not involved: the inventory
shows this touchpad on Serial-IO I2C.

## 4. Fault containment (I-14)

- **(a) Lock isolation.** The ISR (`touchpad::handle_interrupt`) takes no lock at all. It makes one I/O APIC
  write (mask) and two atomic stores (`IRQS`, `PENDING`).
  - It reaches `PIN` through `spin::Once::get`, a lock-free load.
  - It never touches `INFLIGHT`, `RECENCY`, `ARENA`, the reservation, `TABLE`, `DELIVERY`, `STATE`,
    `BUS_OWNER`, or the bus.
  - Grep-checkable: `handle_interrupt` has no `lock(`, and `i2c_hid_interrupt_body` calls only it and the EOI.
  - Runtime assertion (neo-test, review F9 and its re-check): `isr_with_driver_locks_held_for_test` runs
    the ISR body with IF=0 while holding nine locks and flags, in lockdep order:
    - the AI-governance `ARENA` (12), `RESERVATIONS` (20), `INFLIGHT` (22), `TABLE` (25) and
      `RECENCY` (28);
    - `DELIVERY`;
    - the driver's `STATE` and `BOOT_LINES`;
    - the `BUS_OWNER` flag (checked as really taken).

    A lock the ISR wanted would never be released, so a hang would fail the scenario. `i2c_hid_absent` passes
    it.
  - I2C transfers run only at boot (before the thread exists) or on the service thread, under `BUS_OWNER`
    (compare-exchange, never waited for).
  - The pump holds only `STATE` (an `IrqSafeMutex`, as the i8042's) and the focus path's `try_lock`.
- **(b) Liveness.**
  - The ISR is O(1).
  - A stuck-asserted line costs one ISR per thread pass, since it stays masked until the thread has read. 16
    assertions in a row without data leave it masked for good and switch to polling (`irq masked-storm`).
  - A thread pass handles at most 64 assertions, then naps a tick.
  - Every transfer is bounded: 10 ms plus 1 ms a byte, at a 10 GHz TSC bound. A timed-out transfer is
    aborted (`IC_ENABLE.ABORT`) and the controller disabled. 8 failed transfers in a row stop the reader.
  - Bring-up waits: 25 ms per handshake, 10 ms after D3→D0, 60 ms after each SET_POWER, 1 s for the reset
    interrupt. All are bounded and printed.
- **(c) Memory isolation.**
  - Rung 1 is **PIO only.** The LPSS DMA engine is never programmed: resets bit 2 stays 0, checked on the
    read-back (refused otherwise). `IC_DMA_CR` is written 0, and PCI bus mastering is cleared and checked.
  - A later DMA lane must route every buffer through `dma_arena::fence(...)` under the IOMMU containment.

## 5. Pitfall-bank consultation (`docs/concurrency-pitfalls.md`)

| Class | Introduced / guarded / N-A | How | Discriminator |
|---|---|---|---|
| CP-2, CP-3 (IF=0 spin on a lock an ISR holds) | **N-A** | No lock is shared with the ISR | `isr_with_driver_locks_held_for_test` (in `i2c_hid_absent`) |
| CP-37 (a lock taken at IF=1 and waited for at IF=0) | **guarded** | `STATE` is an `IrqSafeMutex` (always IF=0). `BUS_OWNER` is never waited for. `DELIVERY` is only `try_lock`ed (`flush_pointer_staged`) | `i2c_hid_absent`; the USB `hid_contention` covers the shared `DELIVERY` path |
| CP-14 (lost wakeup) | **guarded** | The line is level-triggered: if the device still asserts when the thread unmasks, the ISR fires again. `PENDING` is swapped, not loaded and stored | bare-metal: `count` tracks `reads`; review item |
| CP-17 (publication before ready) | **guarded** | `PIN` and `DEVICE` are `Once`, published before the line is first unmasked. The IDT vector can only fire after `route_gsi` returned and `PIN` was set. `READY` is released after `STATE` | review item |
| CP-39–CP-41, CP-44 (stub hygiene) | **guarded (existing)** | `swapgs_irq_entry`, as the i8042 | existing IRQ-entry scenarios |
| CP-64 (long IF=0 while shootdown-ready) | **guarded** | Bring-up waits run at IF=1 at boot. The thread's unmask is the only IF=0 section (two MMIO writes). No transfer runs at IF=0 | review item |
| CP-69 (trusting a tick) | **guarded** | Bring-up waits are TSC-bounded, not tick-bounded. The thread's nap uses the tick (it runs after the tick is verified) | — |
| CP-72 (edge line left asserted) | **N-A, by contrast** | The line is level. The opposite failure is CP-75 | — |
| CP-73 (back-to-back IF=0 sections) | **N-A** | The thread naps between passes (`sleep_ticks` / yield), and the pump runs once per syscall | — |
| SPSC ring ordering | **guarded** | Producer: the thread only, `HEAD` released after the slot. Consumer: under `STATE`, `TAIL` released after the read. Full ⇒ dropped and counted | review item |
| **New: CP-75** (level line unmasked while still asserted ⇒ storm) | **guarded** | §3 T8 | KAT `line_governor_masks_a_storm_and_stops_on_errors`; bare-metal `irq masked-storm` |
| **New: CP-76** (`IOREGSEL`/`IOWIN` interleaving) | **guarded** | The ISR writes only while the pin is unmasked (the only time it can fire). The thread writes only while it is masked, with IF=0 on its CPU. Boot writes precede both. The only other runtime reader is test-only | review item |

CP-74 is #99's (boot-model preload, merged). CP-75/CP-76 are this lane's.

## 6. Failure modes and discriminators (the Zenbook run)

The evidence is a photo of the screen (`docs/runbook-baremetal-zenbook-s14.md`). Image A prints these lines at
boot (one per step, so the last line names where it stopped):

```
i2c-hid: quirk=zenbook-s14-ux5406aa match (oem=_ASUS_/Notebook crs=0x676b3 hiddesc-reg=0x0001 dev=8086:e351)
i2c-hid: pci 00:19.1 8086:e351 pmcsr=D?->D0 cmd=0x…  bar0=0x…
i2c-hid: lpss reset=0x…->0x00000003 dma=in-reset caps=0x… clk=0x…
i2c-hid: dw ver=0x… fifo=txN/rxN fs-hcnt=78 fs-lcnt=150 sda-hold=0x… addr=0x15 clk-hz=100000000
i2c-hid: hiddesc len=30 ver=0x0100 vid=0x… pid=0x… rdesc-len=… rdesc-reg=… in-reg=… max-in=… cmd-reg=…
i2c-hid: rdesc bytes=… items=… ids=… mouse=idN buttons=…x1/bitN x=…b/bitN y=…b/bitN wheel=… report-bits=…
i2c-hid: power-on=ok reset=sent reset-ack=irq stale=0 irq=gsi105/pin105/low/level vector=0xf6 rirr=Some(0) eoi-suppress=0 mode=irq
i2c-hid: service started (kthread slot=…) mode=irq
```

After the USB summary comes `i2c-hid: TOUCHPAD CHECK - move a finger on the touchpad and click now (6 s)`, then
`touchpad check RECEIVED|NOTHING RECEIVED: i2c-hid: reports=… id=… parsed=… … irq=gsi105 count=… mode=…`.
NOTHING RECEIVED holds the screen for up to 30 s.

**The run:** boot image A from the Kingston stick, with or without a USB mouse. At the TOUCHPAD CHECK, move a
finger and click. At the desktop, move the pointer with the touchpad. Photograph the check's lines.

| Mode | What the screen shows | Next step |
|---|---|---|
| quirk didn't match | `i2c-hid: no match (oem=… dev=…) reason=…`, replayed after the USB summary (review F16) (`oem`, `no-touchpad-crs`, `crs-outside-device`, `hiddesc-reg=…`, `pci-is-…`) | the reason names the condition. A BIOS update that changed the AML ⇒ re-derive §2 from a new dump |
| controller not found / BAR all-ones | `pci 8086:e351 absent at 00:19.1` or `lpss bar0=0xffffffff` | the function hidden (ACPI mode, `IM05`) or not decoding |
| still in reset / D3 | `pmcsr=D3->D3`, or `lpss reset=…->0x00000000`, then `dw configure FAILED: all-ones` or `timeout (Disable)` | the PM/reset sequence |
| iDMA would not stay in reset | `dma=OUT-OF-RESET` + `refusing (I-14(c))` | read the LPSS resets semantics for this PCH |
| not DesignWare | `dw configure FAILED: comp-type=0x… (not DesignWare)` | a different I2C IP: new lane |
| bus NAK on 0x15 | `hiddesc read FAILED: abort=0x… (addr-nak)` | device unpowered (`_PS0`/power resource), or the address |
| bus hung / SCL timing | `hiddesc read FAILED: timeout (Transfer)` or `abort=… (arb-lost)` | the counts (`fs-hcnt`), SDA stuck (T17) |
| wrong HID descriptor register | `hiddesc len=… ver=…` ≠ 30 / 0x0100, then `INVALID` | the `_DSM` value (§2) |
| report descriptor not understood | `rdesc … parse FAILED: … first=[…]` | extend the walk (Push/Pop, long items) |
| no reset ack | `reset-ack=timeout` (or `irq-with-data`) | device quirk (T7) |
| IRQ never fires | `reset-ack=polled(no-irq) … mode=polled(irq-never-fired)`; counters `count=0` with `reads` rising | the GPIO pad's IOxAPIC routing, or the route (`irq=…`). The touchpad still works polled |
| IRQ route refused | `irq=no-madt / no-ioapic / no-pin-gsi105 / dest-apic-id-…-too-wide`, `mode=polled(no-route)` | the named cause |
| EOI suppressed | `eoi-suppress=1`, then `count=1`, `reports` ≤ 1 | directed EOI (T13) |
| storm (or wrong polarity) | `irq masked-storm (16 assertions without data)`, `empty=16`, `mode=polled(masked-storm)` | the polarity (T15), or the device's quirk |
| reports arrive but don't parse | `reports=N id=… parsed=0`, with `unexpected=N` (PTP mode, T10) or `unknown=N` / `short=N` | the Input Mode (rung 2), or the layout |
| reports parse, pointer doesn't move | `parsed=N delivered=0` at the desktop | the pump (not called) or the pointer route |
| transfers fail at runtime | `… transfer errors in a row (last 0x…) - stopped reading`, `mode=failed` | the decoded abort |
| nothing at all: no `i2c-hid:` line | the feature is not in the image, or the boot died earlier | the image's features |

Every branch above names its cause from the console lines, whether the run passes or fails.

## 7. Evidence

### 7.1 Host KATs: `cargo test -p i2c-hid-logic-tests` (28 passed)

- **Byte oracles,** hand-laid, every byte annotated:
  - a 30-byte HID descriptor (each field a distinct value);
  - RESET/SET_POWER command bytes;
  - input framing: zero-length reset ack, 0xFFFF, bad lengths, a 6-byte report;
  - HID 1.11 Appendix E.10's boot mouse descriptor, with its parsed layout and decodes;
  - a PTP-shaped composite: 12-bit X/Y, wheel, a Digitizer collection, a feature report;
  - field extraction across bytes.
- **The DesignWare sequencer against a register model** (`model.rs`). The model runs a HID device on the bus
  and flags config-while-enabled, FIFO overflow and a read-after-write without RESTART. The tests cover:
  - `configure`'s writes;
  - write-then-read with exactly one RESTART and one STOP;
  - a 700-byte read through 8/16-deep FIFOs;
  - write-only and read-only messages;
  - an address NAK (and the next message working);
  - enable-status lag;
  - a never-enabling controller, timing out within its budget;
  - a wedged bus, aborted within its budget;
  - all-ones mid-transfer.
- **The quirk:**
  - the template's fields;
  - PkgLength and integer decoding;
  - synthetic DSDTs for every verdict;
  - **the real corpus:** the Zenbook matches at 0x676B3 / 0x675B7 with register 1, every §2 offset re-read
    from the fixture, and the other 8 machines do not match.

  The corpus tests read `fixtures/acpi/`, on `main` since PR #96 (`ff25f384`). A missing corpus fails them;
  they never skip.
- `fb-progress-logic-tests`: 8/8. New: `i2c_hid_lines_all_have_glyphs`, which checks the photo-read lines,
  with the touchpad feature's glyph gating.
- `line_governor_masks_a_storm_and_stops_on_errors` (CP-75's policy).

### 7.2 QEMU (local QEMU 8.2.2, q35)

| Run | Result |
|---|---|
| `i2c_hid_absent` | **PASS**: `i2c-hid: no match (oem=BOCHS /BXPC dev=not-read-or-absent) reason=oem`; `probed=false`; no-controller `enable=Err(Timeout(Enable)) transfer=Err(Timeout(Enable))` in 2.0× the handshake budget; ISR under every driver lock ran |
| control `neo-test-i2c-hid-absent-control` (match forced) | **FAIL** `i2c_hid_probed_without_match`, after `pci 8086:e351 absent at 00:19.1 - touchpad not present`: failed closed, no hang (4 s) |

### 7.3 Kill-switch identity (`.text`, feature off, each build checked to succeed)

First measured against `main` `b7b4a6a2`, then `84b6686d`. Re-measured after #95, #96 and #99 merged: branch
`093ef1f8` against `main` `cb165c11`. `d300419f`, merged after that, is docs only.

| Feature set | `.text` |
|---|---|
| default | identical |
| `desktop` | identical |
| `product` | identical |
| image A's set without this lane (`desktop,atrium-pointer-forward,real-lm,model-boot-module,fb-progress-console,lapic-timer,boot-model-preload,freeze-watchdog`) | identical |
| `ps2-keyboard` (its cfgs were widened) | identical |
| `xhci-silicon,fb-progress-console` (the font's cfg was widened) | identical |
| `freeze-watchdog` (shares the font's cfg) | identical |

## 8. Notes and limits

- **Not in scope:**
  - precision-touchpad mode (rung 2: Input Mode feature report = 3, contacts, gestures);
  - the wheel (decoded, then dropped: `InputEventV1::pointer` has no wheel field);
  - the touchscreen (`ELAN9008` on I2C4, rung 3);
  - GPIO controllers, bus recovery, LPSS DMA, and suspend/resume (`_S0W`, wake);
  - any AML change.
- **Not covered by QEMU:** everything past the match. QEMU has no DesignWare I2C. The register model and the
  Zenbook photo are what cover it.
- **Latency.** The ISR cannot wake the thread (there is no wake primitive callable from an ISR here), so the
  thread notices an assertion at its next tick: up to 10 ms, then one report per assertion, with a line still
  asserted re-firing at once. That is enough for a pointer. Rung 2's multi-contact reports may want an ISR wake.
- **Polled mode** reads without an interrupt. A device that repeats its last report there would make the
  pointer drift. That is visible as `mode=polled(...)` and is a fallback only.
