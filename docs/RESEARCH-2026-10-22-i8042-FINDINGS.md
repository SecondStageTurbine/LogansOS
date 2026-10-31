<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FINDINGS: the i8042 (PS/2) keyboard driver

**Date:** 2026-10-22 · **Author:** S3 · **Lane:** `Plans/NOTE-2026-10-22-driver-coverage.md` tier 2 ·
**Branch:** `claude/i8042-keyboard`

**Why.** On the Zenbook S14, with no USB devices plugged in, the built-in keyboard does nothing: there is no
PS/2 driver. Windows lists the keyboard as `ACPI\ATK3001` "PC/AT Enhanced PS/2 Keyboard", an i8042 emulated
by the embedded controller (EC).

## 0. Result

- **`ps2-keyboard` feature (the kill-switch).** It is on in `desktop-session`, so it is in `desktop` and in
  image A (`xtask iso --with-model`). Feature off ⇒ none of it is compiled, including the IRQ entry stub.
- **The keyboard's key events are the USB HID boot keyboard's,** delivered the same way:
  - `InputEventV1::key(usage, modifiers, pressed)` with the HID page-0x07 usage and the boot-report modifier
    byte;
  - modifiers are state on key events, never events of their own;
  - one press and one release per keystroke (the keyboard's typematic repeat is suppressed);
  - the same SAK filter, `focus::Stage` and `flush_staged`, pumped beside `hid_keyboard::pump_routed_events`
    in `SYS_INPUT_FOCUS_RECV` and `SYS_WAIT_ANY`.
  - No new syscall. USB HID and PS/2 coexist; each driver owns its own stage.
- **IRQ1 goes through the I/O APIC.** The kernel had no I/O APIC code; this adds one redirection entry
  (`arch/x86_64/ioapic.rs`). The GSI, polarity and trigger come from the MADT (an interrupt source override,
  else identity, edge, active high). The 8259's IRQ1 stays masked.
- **QEMU proof:** `ps2_keyboard` passes 18/18 (QEMU 8.2.2 at 4 and 1 CPUs, and 10.2.3), and its control
  fails. See §7.
- **Two interrupt pitfalls found on the way,** both added to the bank:
  - **CP-72**, in the driver: an edge-triggered ISR that leaves a byte behind;
  - **CP-73**, in the test loop: back-to-back IF=0 sections starve interrupts.

## 1. What changed

| File | What |
|---|---|
| `kernel/src/acpi/madt_logic.rs` (new, pure) | MADT walk: type 1 (I/O APIC) and type 2 (ISA override); `route_isa_irq`, `ioapic_for_gsi` |
| `kernel/src/acpi/mod.rs` | `madt()` and `fadt_boot_arch()` readers (`ps2-keyboard`) |
| `kernel/src/arch/x86_64/ioapic_logic.rs` (new, pure) | register indices and the redirection-entry encoding (82093AA) |
| `kernel/src/arch/x86_64/ioapic.rs` (new) | `route_isa_irq`: masked write, destination, then the unmasked low half, to the BSP |
| `kernel/src/drivers/ps2/i8042_logic.rs` (new, pure) | command set, config transform, status classification, set-1 → HID decoder |
| `kernel/src/drivers/ps2/i8042.rs` (new) | init, lock-free ISR + ring, pump, polled fallback, counters and the status line |
| `kernel/src/arch/x86_64/idt.rs` | vector `0xF5` with a `swapgs_irq_entry` stub (feature-gated); body = ISR + LAPIC EOI |
| `kernel/src/main.rs` | `i8042::init` right after `drivers::usb::init()` |
| `kernel/src/arch/x86_64/syscall.rs` | the pump next to the USB keyboard's, at both call sites |
| `tools/xtask/src/i8042_tests.rs` (new) | host KAT (8 tests) against hand-laid MADT bytes and hand-written tables |
| `tools/xtask/src/main.rs`, `kernel/src/test_harness.rs` | the `ps2_keyboard` scenario |

## 2. Scancode set: set 1 through the controller's translation (decided)

The controller is left in the firmware-default **translating** mode (config bit 6). The keyboard keeps
sending set 2, the controller turns it into set 1, and the decoder reads set 1.

- **Why not select set 2 and turn translation off?** That needs the keyboard to honour `0xF0 0x02`. An
  EC-emulated keyboard is not obliged to, and prior art (Linux `atkbd`'s set-selection quirks, field reports
  on laptop ECs) shows some ignore or garble it.
- **Translation needs nothing from the keyboard.** Every PC firmware boots with it on, and every BIOS-era OS
  has read set 1 through it. Set 1 covers every key a laptop keyboard has.
- **The cost:** set 1's break code is make | 0x80, so `0xAA` (left Shift's break) is the same byte as the
  keyboard's self-test pass. The driver never resets the keyboard, so no self-test byte is expected; a stray one
  decodes as a break for a key not held, and is dropped.

## 3. Prior-art trap-mining (principle only; no code copied)

Sources read: the IBM PC/AT technical reference; OSDev "8042 PS/2 Controller" and "PS/2 Keyboard"; the USB HID
Usage Tables (page 0x07); ACPI 6.5 §5.2.9 (FADT `IAPC_BOOT_ARCH`) and §5.2.12 (MADT); the Intel 82093AA I/O
APIC datasheet; Linux `drivers/input/serio/i8042*.c` and `drivers/input/keyboard/atkbd.c`, read for their
quirk tables and comments (GPL: no code taken). Each trap below is in the build, or in the failure bank (§6).

| # | Trap | Guard here |
|---|---|---|
| T1 | FADT `IAPC_BOOT_ARCH` bit 1 ("8042 present") is wrong on many laptops, both ways (Linux ignores it on x86 by default) | Logged as `fadt_boot_arch=`, never trusted. The ports are probed. |
| T2 | No controller: the ports float, and status reads `0xFF` | `present=no`, return before any write. |
| T3 | Firmware or a previous OS leaves bytes in the output buffer, which then answer the next command | Both ports disabled first, then up to 32 bytes flushed. |
| T4 | The self-test resets the config byte on some controllers | The config is rewritten after the self-test. |
| T5 | Some EC emulations do not implement the self-test or port test (no reply) | A timeout continues (`selftest=timeout`); a wrong value aborts. |
| T6 | A keyboard reset (`0xFF`) is not answered by some EC keyboards, or takes hundreds of ms | Never sent. Only `0xF4` (enable scanning) is sent. |
| T7 | `0xFE` resend and `0xFA` ACK: a command may be NAKed and must be resent | Up to 3 resends, then `kbd=Resend`. A timeout is logged as `kbd=Timeout` and the driver continues (some ECs scan without an ACK). |
| T8 | A missing or broken AUX port; some ECs hang on AUX commands | Port 2 is only disabled (`0xA7`), and its IRQ bit cleared. No AUX test or command. AUX bytes (status bit 5) are discarded. |
| T9 | Parity/timeout errors (status bits 6, 7) | Those bytes are discarded and counted. |
| T10 | Firmware leaves IRQ1 masked, or routes it only through the 8259 (absent or unrouted on client silicon, CP-69's lesson) | The I/O APIC is programmed directly, with the MADT override. |
| T11 | IRQ1 with an override (non-identity GSI or level/low) | `madt_logic::route_isa_irq`, KAT-covered. |
| T12 | **Edge-triggered IRQ1: a byte already in the buffer when the IRQ is enabled raises no edge** | Drained once after enabling. The polled fallback covers it as well. |
| T13 | **Edge-triggered IRQ1: an ISR that leaves a byte behind gets no new edge, and the keyboard goes silent** | Found by our own scenario (§7): the ISR drains until the buffer is empty. A runaway is capped at 256 and counted (`capped=`), and the polled fallback takes the rest. New pitfall **CP-72**. |
| T14 | USB legacy support: firmware emulates an 8042 for USB keyboards via SMI until the OS takes the xHCI | `i8042::init` runs after `drivers::usb::init()`. |
| T15 | Typematic repeat (the keyboard re-sends make codes) | Suppressed: a make for a held key is not an event (matches the USB boot report). |
| T16 | `E0 2A` / `E0 AA` "fake shifts" around navigation keys and Print Screen | Dropped. |
| T17 | Pause is one six-byte `E1` sequence with no break of its own | Decoded as a press plus a release. |
| T18 | Waits must be bounded: a dead controller must not hang the boot | Every wait is a bounded number of status reads (about 100 ms on hardware). |
| T19 | **A polling loop of back-to-back IF=0 sections starves interrupts.** The `sti` ending one is directly followed by the `cli` of the next, and the STI interrupt shadow means nothing is recognised in between | Found in our own scenario's wait loop (§7), not in the driver: the production pump runs once per syscall. The loop now spins with IF=1 between passes. New pitfall **CP-73**. |

Easier-path question (CLAUDE.md): is there a class that does the hard part for us? Not here. The EC already
emulates the simplest, best-documented keyboard interface there is. The alternative path (the keyboard as I2C-HID
or an ACPI device) needs the AML lanes.

## 4. Fault containment (I-14)

The I-14 rule targets connectivity drivers; it is applied here because this adds an interrupt path.

- **(a) Lock isolation.** The ISR takes **no lock at all.**
  - It reads ports 0x60/0x64 and stores into a single-producer ring of atomics.
  - The port is claimed with a `compare_exchange` on `PORT_OWNER`; a loser skips and never spins.
  - It never touches `INFLIGHT`, `RECENCY`, `ARENA`, the reservation, `TABLE`, `DELIVERY` or `STATE`.
  - Grep-checkable: `handle_interrupt` → `try_drain_port` → `drain_port`.
- **(b) Liveness.**
  - The ISR is bounded: at most 256 bytes per pass, each needing the controller to have a byte.
  - A storm costs at most that per interrupt, and the edge-triggered line cannot re-fire without a new byte.
  - The pump's decode is bounded by the 256-byte ring.
- **(c) Memory.** No DMA. The i8042 is port I/O only.

## 5. Pitfall-bank consultation (`docs/concurrency-pitfalls.md`)

| Class | Introduced / guarded / N-A | How | Discriminator |
|---|---|---|---|
| CP-37 (a lock taken at IF=1 and waited for at IF=0) | **guarded** | `STATE` is an `IrqSafeMutex` (always IF=0). The ISR takes no lock. `DELIVERY` is only `try_lock`ed (via `flush_staged`), as for USB. | `ps2_keyboard`; the USB `hid_contention` scenario covers the shared `DELIVERY` path |
| CP-2, CP-3 (spin with IF=0 on a lock an ISR holds) | **N-A** | No lock is shared with the ISR. | — |
| CP-39, CP-40, CP-41 (DF/AC/MXCSR in handlers) | **guarded (existing)** | The vector uses the shared `swapgs_irq_entry` stub. | existing `exception_entry_flags` / `irq_xmm_preserve` |
| CP-44 (per-CPU access through the user GS) | **guarded (existing)** | `swapgs_irq_entry` handles the ring-3 entry. | existing IRQ-entry scenarios |
| CP-64 (masked longer than the shootdown deadline) | **guarded** | Bounded ISR; init waits run at IF=1 at boot. | — |
| CP-69 (the 8259 route trusted to exist) | **guarded** | The I/O APIC is programmed from the MADT; the 8259 is not used for IRQ1. | `ps2_keyboard` asserts every byte came by IRQ1 on the I/O APIC (`polled=0`) |
| SPSC ring ordering | **guarded** | Producer: `HEAD` is written with Release after the byte, only under `PORT_OWNER`. Consumer: `TAIL` is written under `STATE`. A full ring drops (counted). | `ps2_keyboard` (exact event order) |
| **New: CP-72** (an edge-triggered line left asserted) | **was EXPOSED → guarded** | §3 T12/T13 | `ps2_keyboard` 8/8 after the fix; it hung 3 runs in 6 before |
| **New: CP-73** (back-to-back IF=0 sections, no IF=1 window) | **was EXPOSED in the test loop → guarded** | §3 T19 | `ps2_keyboard` 18/18 with the window; about half the runs failed without it on QEMU 8.2.2 |

## 6. Failure modes and discriminators for the Zenbook run

The evidence is a photo of the screen (fb progress console, `docs/runbook-baremetal-zenbook-s14.md`).

- **At boot,** the driver prints one line:
  `i8042: present=… selftest=… port1=… kbd=… irq1=… fadt_boot_arch=… keys=0`.
- **Later,** at the 1st, 2nd, 4th, 8th … key event, it prints:
  `i8042: keys=N irqs=I irq_bytes=B polled=P bytes=T dropped=D discarded=X capped=C`.

**The run:** boot image A with no USB devices, and press keys (letters, Shift, the arrows) once the desktop is up.

| Expected failure | What the screen shows | Next step |
|---|---|---|
| No controller visible (the EC does not decode 0x60/0x64 until something we have not done, e.g. an ACPI `_INI`/`_PS0`) | `present=no` | AML lane, or the EC's enable sequence |
| The EC does not implement the controller tests | `selftest=timeout` and/or `port1=timeout`, keys then work | none; record it |
| The keyboard does not ACK `0xF4` | `kbd=Timeout`, keys may still work | if keys do not come, try a keyboard reset (T6) on a variant |
| IRQ1 never arrives: wrong override, wrong GSI, or the EC raising it elsewhere | `irqs=0 polled>0` and keys **work** (the polled fallback) | read the MADT overrides; keys are usable meanwhile |
| IRQ1 routing refused | `irq1=no-madt`, `no-ioapic`, `no-pin-gsiN` or `dest-apic-id-N-too-wide`, all with `(polled)` | the named cause |
| Keys arrive but decode wrong (the EC ignores translation, or sends set 2 raw) | `keys` rising, wrong characters on screen; `bytes` about double `keys` | a set-2 decoder variant |
| A storm or a stuck output buffer | `capped>0` or `discarded` rising fast | a controller quirk, to mine further |
| USB emulation still owns the ports (T14) | `present=yes` at boot but `bytes=0` | the init order, or the xHCI handoff |
| Nothing at all: no `i8042:` line | the feature is not in the image, or the boot died earlier | check the image's features |

Each run is conclusive: every branch above names its cause from the two status lines.

## 7. Evidence (local QEMU 8.2.2, q35, `NEO_SMP=4`)

**`ps2_keyboard`.** The keys go in by HMP `sendkey`. No USB keyboard is attached, so QEMU routes them to its
i8042. The scenario expects the 14 events below, exactly:

| Key | Usage | Modifiers |
|---|---|---|
| `a` | `0x04` | — |
| `1` | `0x1E` | — |
| `shift-b` | `0x05` | `0x02` |
| `up` | `0x52` (`E0`) | — |
| `ctrl_r-c` | `0x06` | `0x10` (`E0 1D`) |
| `pause` | `0x48` (`E1`) | — |
| Enter | `0x28` | — |

Each key is one press and one release. QEMU releases keys in reverse order (`ui/input-legacy.c`), so the
release still carries the held modifier. The test hook drains without the polled fallback, so the scenario also
requires `irqs>0`, `polled=0` and `irq_bytes == bytes`.

| Run | Result |
|---|---|
| first version (ISR capped at 16 bytes) | PASS 3 of 6; **3 hung** with `keys=10 irqs=2 bytes=16`, the T13 trap |
| ISR fixed, before the rebase onto `4086a8ed` | PASS 8/8, and 5/5 again later, each `irqs=22 irq_bytes=26` |
| ISR fixed, after the rebase, original wait loop | **about half failed** on QEMU 8.2.2 (1 and 4 CPUs), with `irqs` 2–8, bytes lost and the tick frozen; 4/4 PASS on QEMU 10.2.3. The same code before the rebase passed, so this was timing, and the cause was the test loop (T19): see below |
| **final** (IF=1 window in the wait loop) | **PASS 18/18**: 10 at 4 CPUs and 5 at 1 CPU on QEMU 8.2.2, 3 on 10.2.3. Every run `keys=14 irqs=22 irq_bytes=26 polled=0 capped=0` |
| control `neo-test-ps2-keyboard-control` (translation off) | **FAIL** `ps2_keyboard_decode_mismatch` (set 2 read as set 1) |
| `desktop_session_boot` (feature on, USB keyboard present) | PASS; the boot line shows `present=yes … irq1=gsi1/pin1` |
| `ring3_fault_ud`, `admission_park_kill_sink_held` (#92's, on the rebased tree) | PASS |
| host KAT `cargo test -p xtask i8042` | 8 passed |

**How T19 was found.** The failing runs hit QEMU's 180 s budget and never reached the scenario's own 20 s
deadline, so the tick had stopped as well as IRQ1. Both are interrupts, which points at the CPU not taking
interrupts at all, not at the i8042 path.
- The scenario's loop was `drain_events()` (an `IrqSafeMutex`, IF=0), then `yield_now()` (all
  `without_interrupts`), repeated.
- After `sti`, x86 recognises interrupts only after the next instruction (SDM, the STI shadow, the same
  property CP-22 relies on for `sti; hlt`).
- With the two sections back to back, the IF=1 window could be empty.
- Adding work between them (first a TSC read, then a short `spin_loop`) made every run clean.

The loop also gained a TSC bound and prints `ps2_keyboard diag … lapic_isr= lapic_irr= ioapic_low=` on every
run, so a future stall prints its own state.

The q35 boot line: `i8042: present=yes selftest=ok port1=ok kbd=Ack irq1=gsi1/pin1 vector=0xf5
fadt_boot_arch=8042 keys=0`.

## 8. Notes and limits

- **Kill-switch:** `ps2-keyboard` off ⇒ no code, no IDT entry, no asm stub. The default and `product`
  builds are unchanged (they do not enable it).
- **Not in scope:**
  - the keyboard LEDs (Caps/Num lock);
  - typematic configuration;
  - the legacy fd-0 ASCII console path (USB-only today);
  - the AUX port (the Zenbook's touchpad is I2C-HID, not PS/2);
  - ASUS hotkeys (`ASUH2024`, ACPI/WMI).
- **Not covered by QEMU:** EC timing, and a missing override on real firmware. Those are what §6 is for.
- **CP-72 numbering:** CP-70 and CP-71 are taken by #92. If another lane lands a CP-72 first, this one
  renumbers.
