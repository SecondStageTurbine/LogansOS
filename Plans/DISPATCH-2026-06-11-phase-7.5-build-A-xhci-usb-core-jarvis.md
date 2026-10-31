<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7.5 BUILD-A — xHCI + USB core (the lane's first blind-hardware build) → Jarvis

**Date:** 2026-06-11
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `8234e80`: `codex/phase-7.5-build-A-xhci-usb-core-jarvis`)
**Status:** DISPATCHED. **QEMU-first, then OptiPlex blind runs (breadcrumb observability only — the Phase-6 archetype). Both standing rules apply: the blind-hardware failure-mode/discriminator section AND the I-14 governance fault-containment analysis are below.**
**Builds on:** 7.5-0 (`eef664a`, the fenced DMA arena, `FencedDmaAddr`, ADR-045) + the ratified lane THINK (`Plans/THINK-2026-06-11-phase-7.5-opening-connectivity-lane.md`) + existing PCI(e) enumeration (NVMe/AHCI/virtio precedent).
**Carried obligation (Forge, ADR-045 — REQUIRED here):** the xHCI descriptor-programming path must **structurally consume `FencedDmaAddr` only** — until this build, the single-gate property is convention; this build makes it construction.

## 0. The decision this build makes
The kernel gains a USB host: the xHCI controller brought up from scratch (command ring, event ring, interrupters, ports, slots) and a minimal USB core (enumeration: reset → address → descriptors), so 7.5-B can put a CDC-ECM Ethernet adapter on it. **Class drivers are out of scope** — the gate is *enumeration*: a plugged device's VID/PID/class read and logged, on QEMU first, then on the OptiPlex's real root ports, blind.

## 1. Scope
- **ISC-1 — xHCI discovery + takeover.** PCIe class `0x0C0330` discovery, MMIO map (cap/op/runtime/doorbell), **BIOS→OS handoff** via USBLEGSUP (log before/after — discriminator F1), controller reset/CNR wait, basic config (slots, DCBAA, scratchpad — F2).
- **ISC-2 — The rings, arena-fenced by construction.** Command ring + event ring + interrupter, with **every ring, context, and buffer allocated from the 7.5-0 arena**, and **every address programmed into a TRB/register/DCBAA entry passed as `FencedDmaAddr`** — the typed currency, no raw `PhysAddr` accepted anywhere on the programming path (the carried obligation: grep-assertable — `PhysAddr` does not appear in the xHCI programming API). Cycle-bit protocol + dequeue semantics per the pre-seeded bank (F4/F5).
- **ISC-3 — Ports + enumeration.** Port status/reset, slot enable, Address Device, control transfers (the default endpoint), read device/config descriptors. Context size from HCCPARAMS1.CSZ — **both 32/64-byte layouts built, selected at runtime** (F3).
- **ISC-4 — The minimal USB core.** A device model (device → configs → interfaces) + a registration seam for class drivers (7.5-B consumes it). No HID, no MSC, no hubs beyond the root (external hubs = follow-on; log-and-skip a hub cleanly).
- **ISC-5 — Kill-switch (Rule 1).** USB compiled-out / no controller present ⇒ today's system bit-for-bit; full family unchanged.
- **ISC-6 — Standards.** SPDX; new `unsafe` confined to MMIO via the ADR-025 typed wrappers (no new raw classes — justify each block); `cargo fmt`; host suite (ring/cycle/dequeue logic in the pure tier where expressible + truth tables) green; `shard-coverage`; `clippy -D warnings`; builds on+off. **ANSPEC §10.5 (USB host)** + **ADR-046**.

## 2. I-14 governance fault-containment analysis (standing rule)
- **(a) Lock isolation:** the xHCI ISR touches *only* USB state — **no** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`) on any IRQ path. Grep-assertable + the runtime assertion; the new USB lock(s) enter `sync::lock_order` under the lockdep.
- **(b) Liveness:** an event-ring interrupt storm (F6's failure shape) is the I-14(b) case — cap events processed per ISR pass + defer to a worker; the gate includes a simulated-storm scenario asserting the governance family stays green (daemon backstop unaffected).
- **(c1):** closed by ISC-2 — structurally, not by convention. **(c2):** out of scope (7.5-V); USB silicon is the low-(c2)-risk class per the THINK.

## 3. Blind-hardware failure-mode + discriminator section (standing rule)
QEMU (`qemu-xhci` + `usb-kbd`/`usb-storage`) green is the **entry gate** for every bare-metal iteration — bare-metal runs are spent on hardware-only unknowns. The bank (pre-seeded from field prior art + Phase-6 discipline); capture-before-you-need-it: every discriminator below ships *in this build*:

| # | Failure mode (most likely even if the logic is right) | Discriminator (observation → cause) |
|---|---|---|
| F1 | **BIOS keeps ownership / SMM interference** — controller ignores us or SMIs fight us | breadcrumb USBLEGSUP+USBLEGCTLSTS before/after handoff; timeout ⇒ "bios-owned" verdict, distinct code |
| F2 | **Scratchpad missing** — real controllers halt where QEMU forgives | log HCSPARAMS2 scratchpad count + allocated count at init; HCH/HSE bits dumped on any hang |
| F3 | **64-byte contexts** — Address Device fails/garbled on real HW | log HCCPARAMS1.CSZ at init; both layouts compiled; a CSZ-mismatch is impossible by construction, the breadcrumb proves which ran |
| F4 | **Cycle-bit mismatch** — commands silently ignored | command-submitted vs command-completion-event counters per iteration; submitted>completed with no error event ⇒ cycle/dequeue class |
| F5 | **Event-ring dequeue/ERDP mishandling** — one event then silence, or storm | ERDP + cycle-state breadcrumb each ISR pass; "one-event-then-silence" vs "storm" map to distinct causes in the table |
| F6 | **IRQ routing (MSI vs legacy)** — events ring fills, no interrupt | **poll-mode fallback flag** (the conclusive discriminator: works-polled + no-IRQ ⇒ routing, not logic) + USBSTS.EINT vs vector-count breadcrumbs |
| F7 | **Port power/reset timing** — no CSC on a real port | per-port PORTSC sampled on a breadcrumb timer for the first seconds; distinguishes dead-port / no-power / reset-timing |

Every bare-metal run must be conclusive pass **or** fail per this table; an unlocalizable failure is a dispatch defect.

## 4. Gate (two-stage)
- **Stage A (QEMU):** enumerate `usb-kbd` and `usb-storage` — VID/PID/class logged, descriptors parsed; the simulated-storm I-14(b) scenario; kill-switch equivalence; family green; the `FencedDmaAddr`-only grep assertion.
- **Stage B (OptiPlex, blind):** enumerate whatever is physically on the root ports (VID/PID breadcrumbs); all F1–F7 breadcrumbs captured regardless of outcome. Iterate per the Phase-6 loop until enumeration lands; each iteration conclusive per §3.

## 5. Scope guard
No class drivers (HID/MSC/CDC — 7.5-B+), no external hubs, no isoch/streams, no power management, no VT-d (7.5-V), no `NetDevice` work. If the USB core wants something the arena/`FencedDmaAddr` can't express, BLOCKER to PM — do not widen the gate type.

## 6. Merge (PM, on Stage-B PASS)
Close-gate: rebase, re-run Stage-A, push. `pm-merge-check` → `--ff-only` → `main`. Then **7.5-B: USB-Ethernet (CDC-ECM → the existing `NetDevice`)** — the Phase-4 stack lights up over real silicon for the first time.

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-11-phase-7.5-build-A-xhci-usb-core-jarvis.md`
- **THINK:** `Plans/THINK-2026-06-11-phase-7.5-opening-connectivity-lane.md` · **Arena:** ADR-045
- **Branch:** `codex/phase-7.5-build-A-xhci-usb-core-jarvis` off `origin/main` `8234e80`.
- **One-line:** xHCI + minimal USB core, enumeration-only gate (VID/PID/descriptors), QEMU-first then OptiPlex blind. **Structural fencing is required:** every ring/context/buffer from the 7.5-0 arena, every programmed address a `FencedDmaAddr` — no raw `PhysAddr` on the programming path (grep-asserted; the ADR-045 carry). I-14 analysis: no AI locks in ISR (lockdep-ordered USB locks), storm-capped ISR + the simulated-storm governance-green scenario, (c1) by construction. Blind-run bank F1–F7 ships in-build (USBLEGSUP handoff, scratchpad, CSZ both-layouts, cycle/dequeue counters, ERDP breadcrumbs, **poll-mode fallback** for IRQ-routing, PORTSC timeline). Kill-switch: USB-off ≡ today. No class drivers/hubs. ANSPEC §10.5 + ADR-046.
