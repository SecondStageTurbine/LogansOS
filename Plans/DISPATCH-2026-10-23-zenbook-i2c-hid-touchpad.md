<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Zenbook touchpad, rung 1: I2C-HID over Intel Serial-IO I2C (S3)

**Date:** 2026-10-23 · **Author:** PM · **Seat:** S3 (Claude cloud session) · **PL request:** J.c., 2026-10-23
("Start S3 on the touchpad driver now").
**Branch:** `claude/zenbook-i2c-hid-touchpad` (cut from `origin/main`). **Driver-coverage tier:** 4
(`Plans/NOTE-2026-10-22-driver-coverage.md`).

## Goal

The Zenbook S14's built-in touchpad moves the desktop pointer and clicks, on silicon. The Windows inventory names it
`ACPI\ASCE1206` "ASUS Precision Touchpad", an I2C-HID device on Intel Serial-IO I2C (`8086:E350/E351`).

**Rung 1 = mouse mode.** The touchpad exposes a plain mouse collection (`HID\ASCE1206&COL01` "HID-compliant mouse")
next to the precision-touchpad collection (`COL02`). Rung 1 reads the mouse-mode input report and feeds the existing
pointer path. Precision-touchpad (multi-finger, gestures, Input Mode feature report = 3) is rung 2, a separate
dispatch.

## Why this can start before ACPI Fix 3

The AML interpreter can't yet decode the touchpad's `_CRS` or run its `_DSM` (rung-0 findings §4a, on `main` once
#96 merges). But the facts rung 1 needs are **static values in the Zenbook's firmware**, already in the corpus:

| Fact | Value (rung-0 findings §4a; re-derive and cite the AML bytes) |
|---|---|
| ACPI device | `\_SB.PC00.I2C5.ETPD` (`_HID` ASCE1206 on the PL's machine; NVS picks among ASCE1206/ASCF1206/ELAN1010) |
| Bus | `I2cSerialBusV2`, slave address **0x15**, **400 kHz**, controller `\_SB.PC00.I2C5` |
| Interrupt | **`Interrupt` descriptor, GSI 0x69, level, active-low, wake**: an APIC interrupt, **not GpioInt**, so rung 1 needs no GPIO driver |
| HID descriptor register | from `_DSM` UUID `3cdff6f7-4267-4555-ad05-b30a3d8938de`, function 1. **Read the constant out of the DSDT bytes** (`fixtures/acpi/zenbook-s14-ux5406aa-303/`, on #96's branch until it merges) |
| PCI function of I2C5 | from I2C5's `_ADR` in the DSDT, cross-checked against the inventory's `8086:E350`/`E351` |

So rung 1 uses a **platform quirk entry**. It is matched on a firmware identity the kernel already parses (e.g. the
DSDT/FADT OEM ID + OEM table ID, or SMBIOS product name if available) **and** the PCI device ID. A non-matching
machine never touches the controller. When Fix 3 lands, the AML path replaces the quirk entry, and the quirk entry
stays as the cross-check (the quirk values must equal what AML returns). Record every static value in a
`zenbook_s14_quirk` table with the DSDT byte offset it came from.

## Scope

1. **Intel LPSS / DesignWare I2C master, PIO only.**
   - PCI enable, BAR0 MMIO. Put the device in D0 (PCI PM) and take the LPSS private block out of reset: the LPSS
     private registers sit at BAR0 + 0x200 (clock params) and + 0x204 (resets). Verify against the Intel Serial-IO
     programming guide / the PCH EDS for this generation.
   - DesignWare core: `IC_ENABLE`/`IC_ENABLE_STATUS` handshake, `IC_CON` (master, 7-bit, restart-enable, fast mode),
     `IC_TAR`, FS SCL HCNT/LCNT (from ACPI `FMCN` if you can read it statically, else computed from the LPSS input
     clock and documented), `IC_DATA_CMD` with RESTART and STOP bits, TX/RX FIFO depth from `IC_COMP_PARAM_1`, and
     abort-source decode (`IC_TX_ABRT_SOURCE`).
   - **Polled transfers with a bounded timeout.** No DesignWare interrupt and no LPSS DMA in rung 1.
2. **HID-over-I2C protocol** (Microsoft HID-over-I2C spec v1.0):
   - read the 30-byte HID descriptor from the register above;
   - check `wHIDDescLength == 30` and `bcdVersion == 0x0100`;
   - fetch the report descriptor;
   - send RESET, wait for the device's reset-complete (an all-zero length read after the interrupt), then SET_POWER ON;
   - input reports: 2-byte little-endian length prefix, then the report ID and payload.
3. **Report parsing.** Parse the report descriptor enough to find the mouse collection's report ID, the
   button/X/Y/wheel field offsets and sizes. Reuse the USB HID code where it fits; the existing USB path only parses
   boot protocol. Feed relative motion and buttons into the same pointer event path the USB mouse uses
   (`drivers/usb/hid_mouse.rs` → the desktop's pointer), without changing that path's behaviour for USB.
4. **The interrupt.** GSI 0x69, level-triggered, active-low, through the I/O APIC. `ioapic::route_isa_irq` exists
   for ISA IRQs (the i8042 lane); rung 1 adds a GSI route with explicit trigger/polarity. Level-triggered rules:
   - the ISR only signals: it masks the line, or records pending and EOIs;
   - a service thread drains the report over I2C until the device deasserts, then unmasks;
   - never leave a level line unmasked while the device still asserts it.
   A polled fallback (read input every N ms when the IRQ route fails) is acceptable for rung 1 if the overlay says
   which mode is live.
5. **Boot console line** (fb progress console, as #93 did for USB): controller found/not, D0/reset state, HID
   descriptor fields (VID/PID, report-desc length), RESET ack seen, IRQ mode (routed GSI vs polled), report count.
   That line is the run's discriminator on a machine with no serial port.

**Out of scope:** precision-touchpad mode, the touchscreen (`ELAN9008` on I2C4; rung 3), GPIO controller drivers,
LPSS DMA, and any AML change (that's S5's lane).

## Kill-switch (required)

Feature `i2c-hid-touchpad`. Off ⇒ the kernel `.text` is **bit-identical** to `main` for `default`, `desktop`,
`product`, and image A's feature set (the S2 #95 identity method). On ⇒ only machines that pass the quirk match touch
the controller.

## I-14 governance fault-containment analysis (required)

- **(a) Lock isolation.**
  - The ISR takes no AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`) and no lock the I2C
    service thread holds across a transfer.
  - Show it by grep, plus a runtime assertion in the `neo-test` build.
- **(b) Liveness.**
  - A stuck-asserted level line must not monopolise CPU 0. Mask on entry; on the Kth consecutive spurious drain, stay
    masked and fall back to polled mode, then report it on the console line.
  - Every I2C transfer has a bounded timeout; a wedged bus aborts and is reported, never spins forever.
- **(c) Memory isolation.**
  - Rung 1 is **PIO only**: no device-initiated DMA, because the LPSS DMA engine is never programmed. State this, and
    assert in code that the LPSS DMA registers are left in reset.
  - Any later DMA use goes through `dma_arena::fence(...)` under the IOMMU containment.

## Prior-art trap-mining (required; seed the failure bank from these, then add what you find)

Read for principle only (Linux `drivers/i2c/busses/i2c-designware-*`, `drivers/mfd/intel-lpss*`,
`drivers/hid/i2c-hid/`, FreeBSD `ig4`/`iichid`, OSDev). **Never copy GPL code.** The known traps to check and record:

1. **LPSS held in reset / clock gated after firmware hand-off.** Writes are silently ignored and reads return
   0xFFFFFFFF or 0.
2. **The device is in D3hot.** BAR reads return all-ones until the PCI PM transition to D0.
3. **`IC_ENABLE` → `IC_ENABLE_STATUS` lag.** Configuration registers are writable only while disabled; poll status.
4. **SCL timing.** Wrong HCNT/LCNT gives NAKs or a hung bus. ACPI `FMCN`/`SSCN` hold the platform values; Linux
   prefers them over computed ones.
5. **TX FIFO underrun between the register write and the read.** A missing RESTART in a write-then-read becomes a
   STOP+START, which some devices reject. Use `IC_DATA_CMD.RESTART` and keep the FIFO fed.
6. **Abort sources:** 7-bit address NAK (wrong address or the device is unpowered), and arbitration lost.
7. **HID-over-I2C RESET.** The device asserts its interrupt and returns a zero-length report after reset. Reading
   input before that sees stale data. Some devices need a short delay after SET_POWER ON (Linux has per-device
   quirks).
8. **Level-triggered interrupt storm** if the report isn't drained before unmask (cf. CP-72, the edge-triggered
   leftover byte, for the opposite class).
9. **`_STA` / NVS-selected identity.** The firmware picks ASCE1206/ASCF1206/ELAN1010 by NVS. The quirk entry must not
   assume the ELAN variant's descriptor address.
10. **The touchpad may boot in PTP mode on some firmware.** Check which report IDs actually arrive and handle both
    the mouse collection and an unexpected PTP report (count it; don't misparse it).

## Concurrency (required): pitfall-bank consultation

Consult `docs/concurrency-pitfalls.md` (CP-1…CP-73 on `main`; CP-74 arrives with #99). For each class the IRQ → service
thread → pointer path touches, state whether you introduce, guard, or leave it N/A, and name the discriminator.
CP-72 and CP-73 (the i8042 lane) are the closest precedents. Add any new pitfall back to the bank.

## Gates

**Merge bar:**
- rebase-fresh;
- `scripts/worker-close-gate.sh`;
- the 4-shard QEMU gate, green on the exact SHA (the PM dispatches it);
- the cross-family review (below);
- the kill-switch identity.

**Acceptance bar:** the Zenbook photo (below). It is tracked as a milestone, not a merge gate (CLAUDE.md, merge bar vs
acceptance bar).

QEMU has no DesignWare I2C, so the gate covers logic and absence:
- **Host logic-test crate** (`tools/i2c-hid-logic-tests`):
  - **byte oracles**, hand-laid from the HID-over-I2C spec: a 30-byte HID descriptor; a length-prefixed input
    report; a mouse report descriptor with its parsed field layout; the zero-length reset ack. Take exact bytes from
    the raw spec or raw source files, never through a summarising fetch.
  - the DesignWare transfer sequencer, run against a register model: write-then-read with RESTART, NAK abort,
    timeout;
  - the quirk match: the Zenbook identity matches, and every other corpus machine does not.
- **QEMU scenarios:**
  - `i2c_hid_absent`: the feature is on, on QEMU (no matching device). The quirk does not match, the driver reports
    "not present", and nothing is probed. Its control forces the match on QEMU and must fail closed: no controller,
    a bounded timeout, no hang;
  - the existing `desktop_*` and `usb_*` scenarios still pass.

## Bare-metal acceptance run (Zenbook; required failure-mode + discriminator section)

Follow `docs/runbook-baremetal-zenbook-s14.md` (`git fetch origin && git show
origin/main:docs/runbook-baremetal-zenbook-s14.md`).
- No serial port. **Photos of the screen are valid evidence on this target.**
- The image boots from the Kingston stick. **The kernel never writes the Zenbook's internal NVMe;** it sits behind
  VMD, and the write guard stays on.
- The image is built by Janus (a PM relay) once this merges; the PL runs it.

Pre-seeded failure bank. The console line must tell each mode apart from one photo:

| Mode | What the console line shows |
|---|---|
| quirk didn't match | `i2c-hid: no match (oem=… dev=…)` |
| controller not found / BAR all-ones | `bar0=0xffffffff` or `pci 8086:e35x absent` |
| still in reset / D3 | `lpss reset=… pmcsr=…` before and after |
| bus NAK on 0x15 | `abort=0x…` with the decoded source |
| wrong HID descriptor register | `hiddesc len=… ver=…` ≠ 30 / 0x0100 |
| no reset ack | `reset-ack=timeout` |
| IRQ never fires | `irq=gsi105 count=0` vs `polled reports=N` |
| storm | `irq masked-storm` |
| reports arrive but the pointer doesn't move | `reports=N id=… parsed=0` |

## Cross-family review

**MODEL:** GPT-6-Luna. **EFFORT:** high. One contained subsystem: no SMP core and no new syscall. S3 writes
`Plans/DISPATCH-<date>-zenbook-i2c-hid-touchpad-review.md` as a **plain pass/fail checklist** (no
refute/attack/exploit wording). The PM relays it to a Codex seat.

## Close

- Commit and push at every checkpoint.
- Open a PR against `main`, and post "PM: ready to gate <SHA>" with:
  - the close-gate output;
  - the identity table;
  - the host-test and scenario results;
  - the quirk table with DSDT byte citations;
  - the pitfall consult.
- **Do not dispatch the 4-shard gate yourself.**
- End GitHub comments with the Claude Code attribution footer.
- **Disk reclaim:** after merge, `cargo clean` if you keep the tree.
