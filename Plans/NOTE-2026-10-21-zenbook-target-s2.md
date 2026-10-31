<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: Zenbook S14 (UX5406AA, Panther Lake) bring-up: trap-mining and failure bank (S2)

**Lane:** the PL's new bare-metal target (PL decision 2026-10-21). **Branch:** `claude/zenbook-target`.
**Runbook:** `docs/runbook-baremetal-zenbook-s14.md`.

**Blind-hardware rules:** every failure below has an **on-screen discriminator**, because there is no
serial port and a photo is the evidence. `fb-progress-console` mirrors every kernel log line, so "on
screen" means "the line the kernel prints".

## 1. Sources (read for principle only; no GPL code copied)

- **Intel SDM Vol. 3A ch. 11** (local APIC timer, TSC-deadline mode) and **Vol. 3B ch. 18** (TSC, the
  CPUID 0x15/0x16 frequency leaves).
- **Limine `PROTOCOL.md`** (raw): the framebuffer and MP responses, and the base-revision 5 LVT/IOMMU
  handling. We request revision 3.
- **Public Panther Lake material:** hybrid P / E / LP-E clusters in the H-series parts, and Intel's
  move away from legacy timers on client platforms ("8254 clock gating" is a firmware option on
  Skylake-and-later client chipsets, and modern laptops ship it on for S0ix).
- **Linux's x86 timer setup**, read for its decision order only: it treats the PIT as optional,
  calibrates the TSC from CPUID 0x15/0x16 when present, and runs the tick on the LAPIC timer
  (TSC-deadline where available). It also detects VMD, under `drivers/pci/controller/vmd.c`.
- **OSDev:** "APIC timer", "8259 PIC", "NVMe", "Intel VMD".

## 2. Facts about our kernel that set the exposure

| Area | Today | Source |
|---|---|---|
| Tick | PIT ch.0 → 8259 IRQ0 → vector 32 on the BSP only. APs have no timer. | `sched::scheduler::init_bsp`, `arch::x86_64::{pit,pic,timer}` |
| Keyboard | **USB HID only.** No PS/2 / i8042 code exists, so nothing can wait on a missing i8042. | grep; `drivers::usb::hid_keyboard` |
| Pointer | USB HID mouse only. No I2C-HID. | `drivers::usb::hid_mouse` |
| ACPI | Only DMAR (under `vtd`). **The MADT is not parsed.** Limine enumerates CPUs and APIC IDs. | `acpi/mod.rs` |
| CPUs | `MAX_CPUS = 64`; extra CPUs are skipped with a named line. | `per_cpu.rs`, `smp.rs` |
| APIC | x2APIC-capable since CP-68 (#82). Limine enables it on this CPU. | `apic.rs` |
| Framebuffer | Limine GOP; **32 bpp asserted** (panics otherwise); pitch honoured. | `drivers::fb::init` |
| xHCI | **Only the first** xHCI function found is driven. | `drivers::usb::xhci` (`find_by_class`) |
| Disk writes | Marker-gated (this lane); the sink needs a pre-provisioned header. | `drivers::block` |

## 3. Failure bank (most likely first)

### 4a. The timer: the predicted first failure

- **Trap.** Panther Lake client firmware may clock-gate the 8254 (it does not count, so no IRQ0), or
  leave no working 8259 → LINT0 virtual-wire path to the BSP: the PCH's legacy PIC output is unused
  when firmware runs the IOAPIC and masks LINT0.
- **What our kernel does:** it never preempts and never advances `TICKS`. Anything that waits on time
  (sleep, yield-to-timeout, the desktop's 60 Hz pacing) waits forever. The boot looks frozen right
  after the scheduler starts.
- **On-screen discriminator (added in this lane).** Right after `sched: preemption ready (cpu=0, PIT
  @ 100 Hz)`, `fb-progress-console` builds wait a bounded ~1 s of TSC for the first tick and print one
  of:
  - `timer: PIT/PIC tick alive (first tick after N TSC cycles)`: the tick works; look later;
  - `timer: NO TICK after N TSC cycles - the PIT is gated or the 8259 route is missing ...`: this trap.
- **Fix (its own PR, stacked on #82, per the PM):** a LAPIC timer tick on the BSP, in xAPIC and x2APIC
  mode.
  - Mode: TSC-deadline when CPUID advertises it, otherwise one-shot or periodic.
  - Calibration: from CPUID 0x15/0x16 when present, and against the PIT only where the PIT works.
  - Kill-switch: the PIT/PIC path stays as the feature-off build.
  - Gate: an SMP/timer-core change, so it needs a Codex cross-check dispatch.

### 4b. Keyboard and pointer

- **Trap.** On Zenbooks the internal keyboard is typically wired through the EC as PS/2 (i8042). The
  touchpad and touchscreen are I2C-HID. We drive neither.
- **What applies:** **a USB keyboard is required** for any run that needs input, and pointer input needs
  a USB mouse. There is **no hang risk** from a missing i8042: we never touch ports 0x60/0x64 (no code
  path does).
- **Second trap: which USB port.** Panther Lake has a PCH xHCI and an integrated (TCSS / Thunderbolt)
  xHCI for the Type-C ports. We drive only the first xHCI the PCI scan finds, so a keyboard on the other
  controller's port is invisible.
- **Discriminator.** The `xhci:` lines show the controller's bus/device/function. If no keyboard
  enumerates, move it to the other port type (USB-A vs Type-C, via an adapter). A multi-controller
  xHCI is a follow-up.

### 4c. Hybrid P / E / LP-E cores and APIC IDs

- **Trap.** Many CPUs with non-contiguous, possibly large APIC IDs. LP-E cores sit on a separate low-power
  island. Core types differ in performance (CPUID 0x1A), not ISA, so nothing faults.
- **What our kernel does:**
  - `MAX_CPUS = 64` covers the part (Panther Lake-H is up to 16 cores per public material).
  - x2APIC (CP-68) carries 32-bit IDs in IPIs.
  - MSI targets the BSP's ID and refuses above 255.
- **Discriminators:**
  - `smp: Limine reports N cpu(s); bsp_lapic_id=...`, then a `starting cpu_id=.. lapic_id=..` line per
    CPU;
  - `AP ready-barrier cleared (k/k)`, or the degraded line naming a stuck AP;
  - `apic: MSI REFUSED` if the BSP's ID is above 255.

### 4d. GOP framebuffer on Xe3

- **Trap.** The resolution and pitch may differ from what the desktop requests (`set_desktop_resolution`
  asks Limine for a mode, and GOP may not offer it). A non-32-bpp mode is possible in principle.
- **What our kernel does:** it uses the mode Limine sets and honours the pitch. A non-32-bpp mode
  **panics** (`neoplutarch currently requires a 32-bpp framebuffer`), which `fb-progress-console`
  cannot show because it is the framebuffer.
- **Discriminators:**
  - `framebuffer: WxH @ B bpp, pitch P` and `fb-progress: on WxH scale=S grid=CxR` (the first
    screen-mirrored lines);
  - a blank screen with no text at all means either a framebuffer that never initialized, or a bpp
    panic.

### 4e. NVMe (read-only) and Intel VMD / RST

- **Trap.** With VMD on (common on Core Ultra laptops shipped with Intel RST), the NVMe sits behind a
  VMD domain and does not appear as a class 0x0108 function. Without VMD, the Windows NVMe *is* visible.
- **What our kernel does:**
  - **No write ever reaches it:** the write guard finds no NeoPlutarch marker, and the sink finds no
    pre-provisioned header.
  - The NVMe probe issues only controller commands (identify, queue setup) and reads; no media write.
  - With VMD on, no internal disk appears at all.
- **Discriminators:**
  - `pci: Intel RAID/VMD controller ... no internal disk will be probed` (added in this lane);
  - or `pci: NVMe controller ...`, then `block: registered device ...`;
  - and on any write attempt, `block: WRITE REFUSED device=N ... this disk is never written`.

### 4f. ACPI MADT x2APIC entries (type 9)

- **Trap.** A kernel that walks the MADT must also handle Local x2APIC (type 9) and x2APIC NMI (type 0xA)
  entries, or it misses CPUs.
- **What applies: N/A today.** We do not parse the MADT; Limine does and hands us the CPU list with
  32-bit APIC IDs. Any future MADT parser (an IOAPIC driver, say) must handle both entry types.
- **Discriminator:** the `smp: Limine reports N cpu(s)` count against the CPU's real core count.

### Others to watch

- **Memory map.** 32 GB is far above anything we size for. Large reserved holes are normal. The heap and
  buddy setup print their extents.
- **Panic before the framebuffer.** Anything that fails before `mm::init` returns cannot reach the
  screen, because the mirror starts there. Those are Limine-stage or very early kernel faults.
  Discriminator: the screen stays at Limine's last output.

## 4. Pitfall-bank consultation (for the timer lane)

The LAPIC timer change touches CP-15/CP-21 (EOI-only reschedule vs a busy CPU: a per-CPU tick changes
who preempts), CP-22/CP-23 (IPI lost at `sti; hlt`), CP-27 (an AP still in init) and CP-64 (long `IF=0`
sections versus tick delivery). It is assessed in that PR's dispatch, not here. This lane adds no
cross-CPU code; the first-tick check is BSP-local and read-only.
