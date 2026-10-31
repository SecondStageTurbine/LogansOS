<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: The physical-input path — framebuffer console + USB-HID keyboard (OD-1 Phase 8b) → Janus

**Date:** 2026-06-17
**From:** PM (Claude)
**To:** Janus (free after Slice 4; built the fd=0 serial-stdin path in Slice 1 — direct continuity). New branch off `origin/main` HEAD: `codex/phase-8-input-path-framebuffer-keyboard-janus`.
**Status:** DISPATCHED. **The "talk to Arthur at the physical machine" build.** Slice 1 gave fd=0 over *serial* (OD-1=A, Phase 8a); this is **OD-1 Phase 8b**: a framebuffer text console (fd=1 → screen) + a USB-HID keyboard (fd=0 ← keyboard), so a human types at the real keyboard and sees output on the real screen — not serial. With Phase 9 (the fast brain), this completes "sit down and use Arthur."

## 0. The decision this build makes
Route the **physical** keyboard and screen into the existing fd=0/fd=1 stdin/stdout path: a minimal framebuffer text console renders fd=1 to the UEFI framebuffer (Phase 0), and a USB-HID **boot-protocol** keyboard (riding the 7.5-A xHCI/USB stack) feeds fd=0. neo-shell/Arthur are unchanged — they already read fd=0 / write fd=1; this swaps the *transport* from serial to physical. Minimal-sufficient (Rule 3): boot-protocol HID + a basic glyph console — **not** a full TTY, not report-descriptor HID, not a windowing system.

## 1. References
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-16-arthur-userland-arc-and-architecture.md   # §2C/OD-1: serial (8a) vs keyboard+framebuffer (8b)
```
Reuse: Phase-0 UEFI framebuffer; the 7.5-A **xHCI/USB host stack** (the HID keyboard is a USB device on it); Slice 1's fd=0/fd=1 namespace-reserved routing.

## 2. ISCs
- **ISC-1 — Framebuffer text console (fd=1 → screen).** Monospace glyph blit to the UEFI framebuffer (an embedded bitmap font), a cursor, scroll-on-overflow, newline handling. fd=1 writes route here when the `framebuffer-console` feature is on (alongside or instead of serial). Minimal — no colors/escape-sequences beyond newline/backspace required.
- **ISC-2 — USB-HID boot-protocol keyboard (fd=0 ← keyboard).** Enumerate a HID keyboard on xHCI (class 0x03, boot subclass/protocol); `SET_PROTOCOL(boot)`; poll the interrupt-IN endpoint for the fixed **8-byte boot report** (modifier byte + 6 keycodes); translate HID usage codes → ASCII (with shift/modifiers); feed the fd=0 read path (the OD-1 stdin path from Slice 1). Boot protocol only — **no report-descriptor parsing**.
- **ISC-3 — I-14 fault-containment note (required — input is a connectivity/device driver).** The HID keyboard is a **USB device behind xHCI** → its DMA is already **arena-fenced** (BUILD-7.5-0/A; no new bus master, no new DMA path); the interrupt-EP ISR takes **no AI-governance lock** (a) and is bounded per poll (b); (c) inherited from the existing xHCI fencing. **No new I-14 guard needed** — state this explicitly (if you find a genuine new vector, BLOCKER-to-PM).
- **ISC-4 — Standards + kill-switch.** **Kill-switch:** `framebuffer-console` / `usb-hid` features **default-off ⇒ serial-only, kernel bit-for-bit today** (the Rule-1 regression). Host-testable HID-keycode→ASCII + console-layout logic in a `*_logic` split. `fmt`/`clippy -D warnings` (kernel clippy now **blocking** — run the exact `--workspace --all-targets` via Docker). No new invariant; no new `unsafe` beyond documented MMIO/framebuffer writes (ADR-025 idiom). ANSPEC §10.x (input) or a short note.

## 3. Gate
- **QEMU scenario `input_path_console`:** boot with QEMU `usb-kbd` + a framebuffer device → a prompt renders **on the framebuffer** → scripted keystrokes arrive via the HID path → echo to the screen → `neo-shell` reads the line (e.g. types `exit`) → shell acts. The physical-input round-trip exercised at NEO_SMP=4. *(QEMU emulates `usb-kbd`; the framebuffer is checkable via the existing framebuffer test hooks / a captured glyph region.)*
- **Kernel-touching ⇒ run the pre-merge `workflow_dispatch` QEMU gate green** (all shards) + the now-blocking clippy via Docker.
- **Bare-metal Stage-B (follow-on, PL-coordinated):** a real USB keyboard at the OptiPlex → prompt on the real screen → type → echo. This is a **blind-hardware run** — include a brief expected-failure-mode/discriminator note (the known HID traps from prior art: `SET_PROTOCOL` boot-mode required, the interrupt-EP polling interval, the LED/output-report handling, NKRO vs boot 6-key) so a blind failure self-localizes. QEMU-first de-risks it; the silicon run can bundle with the next OptiPlex trip (W2 / the benchmark).

## 4. Scope guard
The framebuffer text console + the boot-protocol HID keyboard + the fd routing **only**. **Do NOT** build a full TTY/line-discipline (job control, escape sequences), report-descriptor HID parsing, mouse/touchpad, a windowing system/compositor, or change neo-shell/Arthur (they already use fd=0/fd=1). Reuse the xHCI stack + Phase-0 framebuffer + Slice-1 fd routing. No new invariant.

## 5. Merge (PM)
Close-gate: rebase onto current `main`; host logic tests + the `input_path_console` scenario green; **pre-merge QEMU gate green**; `clippy --workspace --all-targets` green via Docker; push. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (Article 13/28 — **the physical-input path: type at the machine, not serial**). With Phase 9 (the fast brain) + Slice 4 (Arthur acts), this completes the "sit down and talk to Arthur at your desk" path — the bare-metal keyboard run being the final hardware proof.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed (never an unmerged tree). See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Janus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-17-input-path-framebuffer-keyboard-janus.md`. **Branch:** `codex/phase-8-input-path-framebuffer-keyboard-janus` off `origin/main`.
- **One-line:** Build **the physical-input path (OD-1 Phase 8b)** — so a human types at the real keyboard and sees output on the real screen, not serial. (1) A **minimal framebuffer text console** (glyph blit to the UEFI framebuffer, cursor, scroll) → fd=1 routes to it. (2) A **USB-HID boot-protocol keyboard** on the 7.5-A xHCI stack (`SET_PROTOCOL(boot)`, poll the 8-byte boot report, HID-keycode→ASCII) → feeds the Slice-1 fd=0 path. **Boot protocol only — no report-descriptor parsing**, no full TTY, no mouse, no windowing. **I-14:** the HID keyboard is a USB device behind xHCI — arena-fenced already, no new DMA master, ISR takes no AI-gov lock — inherited, no new guard (state it). Kill-switch: `framebuffer-console`/`usb-hid` default-off ≡ serial-only today. Gate: the `input_path_console` QEMU scenario (QEMU `usb-kbd` + framebuffer: prompt-on-screen → keystrokes → echo → neo-shell reads) at NEO_SMP=4 + **pre-merge QEMU gate green** + `clippy --workspace --all-targets` via Docker (kernel clippy now BLOCKING). Bare-metal real-keyboard run is a PL-coordinated Stage-B follow-on (brief blind-hardware discriminator note for the known HID traps). No new invariant; reuse xHCI + Phase-0 framebuffer + Slice-1 fd routing.
