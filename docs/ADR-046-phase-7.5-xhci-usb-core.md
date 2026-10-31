<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-046: Phase 7.5 BUILD-A — xHCI + USB Core (the lane's first hardware driver)

**Status:** Accepted
**Date:** 2026-06-11
**Builds on:** ADR-045 (the fenced DMA arena this driver is the first consumer of); the ratified lane THINK (USB-first, QEMU-first)
**Closes:** the ADR-045 carry — the single-gate property is now construction, not convention

## What landed

A from-scratch xHCI 1.2 host controller driver (`drivers/usb/xhci.rs`) and a
minimal USB core (`drivers/usb/mod.rs`): PCIe 0x0C0330 discovery, USBLEGSUP
BIOS→OS handoff, controller reset, DCBAA + HCSPARAMS2-sized scratchpads,
command + event rings with the cycle-bit/Link-TRB protocol, interrupter 0
(MSI vector 0xf3, ISR burst-capped), port reset, Enable Slot, Address Device
with **both 32/64-byte context layouts compiled** (CSZ-selected at runtime),
EP0 control transfers, device/config descriptor parsing, and the
class-driver registration seam (7.5-B's consumption point). Hubs are logged
and skipped; everything else (classes, isoch, PM, external hubs) is
out of scope by dispatch.

**The maiden QEMU run passed** — both root-port devices enumerated on the
driver's first boot (usb-storage SuperSpeed `46f4:0001`/MSC, usb-kbd
HighSpeed `0627:0001`/HID), descriptors parsed, F4 command counters balanced
4/4, ISR burst max 2 ≤ cap 16. The pure ring arithmetic (`xhci_ring_logic`,
host-truth-tabled across two full cycle laps before any MMIO existed) and
the pre-run design review (which caught the short-data-stage event-stream
desync — the F5 class — before it could happen) are why.

## The structural fence (the ADR-045 carry, closed)

Every ring, context, scratchpad, and transfer buffer is a
`DmaBuffer::alloc_in_arena` allocation; **every function that programs an
address into a TRB, register, or DCBAA entry takes a `FencedDmaAddr`** —
`PhysAddr` does not appear in the programming API (grep-asserted at the
gate). The enumeration run's fence verdict: admits > 0, **refusals == 0** —
the driver never even *attempted* an out-of-arena address. I-14(c1) holds
in production shape on the first real consumer.

## I-14 posture

- **(a)** The ISR touches only the `IrqSafeMutex` event state + atomics; the
  process-side USB locks (`USB_XHCI`=30, `USB_CORE`=31) order ABOVE every
  AI-governance level — acquiring an AI lock while holding a USB lock is a
  lockdep panic. Machine-checked, both directions covered (no AI code takes
  USB locks; USB code under AI levels panics).
- **(b)** `ISR_EVENT_CAP` bounds every drain pass; the `usb_storm` gate runs
  50 inferences byte-identically while a storm thread monopolizes a core
  with ISR-equivalent passes — governance green under storm, the cap held.
- **(c1)** by construction (above). **(c2)** untouched, 7.5-V.

## F1–F7 in the field (what the breadcrumbs already taught)

- **F6 caveat (QEMU):** `irqs=0` on a fully passing run — the synchronous
  enumeration path polls aggressively and acks `IMAN.IP` before MSI
  delivery wins the race. Benign by design (the poll path IS the F6
  fallback), but it means **IRQ counts can be 0 with correct routing**; the
  bare-metal F6 verdict must use poll-mode + the USBSTS.EINT breadcrumbs,
  not the IRQ count alone.
- **F3:** QEMU reports CSZ=32; the 64-byte layout is compiled and selected
  by the same code path — the breadcrumb proves which ran.
- **SuperSpeed `bMaxPacketSize0` is an exponent** (2^n, USB 3 §9.6.1) — the
  quirk breadcrumb decodes it before comparing (a maiden-run finding).

## Stage B (OptiPlex, blind)

QEMU green is the entry gate (met). The blind runs land on whatever is
physically on the root ports; every F1–F7 discriminator ships in this build
(handoff verdicts, scratchpad counts, CSZ, command counters, ERDP
breadcrumbs, poll-mode, the PORTSC timeline) — each run is conclusive
pass-or-fail per the bank.
