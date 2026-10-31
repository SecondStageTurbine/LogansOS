<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-8 — talk to Arthur on the physical console (keyboard → Arthur → screen) → Janus

**Date:** 2026-06-24
**From:** PM (Claude)
**To:** Janus (Phase-8 Arthur lane; physical-input path author). New branch off `origin/main` HEAD: `codex/phase-8-arthur-physical-console-janus`.
**Status:** DISPATCHED — the build that completes the headline on-device: **sit at the NeoPlutarch box and converse with Arthur.** **Complete-and-wire, not from-scratch** — the framebuffer console (`eacb7f7`), USB-HID keyboard, and an `arthur_console` QEMU scenario (`005b980`) are already on `main`; the gap is the **interactive loop**. **Non-conflicting with the live bare-metal connectivity subsystem** (this is fb/HID + the Arthur I/O loop, not mt7601u/xHCI/DHCP) — but **coordinate the MERGE** on the shared Arthur boot path with OT-40 (Jarvis) and the bare-metal session (see §5). **Kernel-touching ⇒ 4-shard QEMU gate + AAG.**

## 0. The decision this build makes
Turn "Arthur answers a hardcoded prompt over serial" (OT-37) into "**the operator types a prompt on the OptiPlex keyboard and reads Arthur's reply on the OptiPlex screen**" — a first-class interactive console REPL: keyboard input → frame the turn → `sys_generate` → render the reply to the framebuffer, loop.

## 1. References (OBSERVE what's already landed)
```
git fetch origin
git show origin/main:kernel/src/drivers/fb/console.rs          # framebuffer text console (render path)
git show origin/main:kernel/src/drivers/fb/console_logic.rs    # the console logic (host-testable)
git show origin/main:kernel/src/drivers/usb/hid_keyboard.rs    # USB-HID keyboard driver
git show origin/main:kernel/src/drivers/usb/hid_logic.rs       # HID scancode→char logic
git show origin/main:kernel/src/test_harness.rs                # the existing `arthur_console` QEMU scenario (005b980) — OBSERVE what it already drives
git show origin/main:userland/neo-arthur/src/main.rs           # Arthur boot loop: frame_turn_ids → sys_generate → reply (OT-37) — where the I/O is currently serial/hardcoded
git show origin/main:userland/neo-arthur/src/conversation_logic.rs  # the turn framing
```

## 2. ISCs
- **ISC-1 — The interactive loop.** Wire the existing pieces into a REPL: **read a line from the USB-HID keyboard** (line-buffered, with backspace/enter editing via the existing HID logic) → **frame it** (the OT-37 `frame_turn_ids` path) → `sys_generate` → **render the reply to the framebuffer console**, then prompt again. OBSERVE first what `arthur_console` (005b980) already does — extend it, don't duplicate.
- **ISC-2 — Framebuffer rendering of the reply.** Decode the reply token-ids → text (the real vocab) and render to the fb console (line-wrap, scroll). Streaming-per-token rendering is a plus (the operator sees Arthur "type") but a single final render is acceptable.
- **ISC-3 — The `arthur_console` QEMU gate (interactive).** Drive the scenario end-to-end in QEMU: **inject keystrokes** (the HID/PS2 input path QEMU supports) for a fixed prompt → assert the **framebuffer** contains a coherent reply (reuse the OT-37 deterministic small-model/fixture path so the assertion is stable). At `NEO_SMP=4`, hermetic.
- **ISC-4 — Kill-switch.** Ship behind `console-interactive` (or reuse the existing console mode flag); **disabled ≡ the current serial/hardcoded-prompt behavior, bit-for-bit** — the OT-37 boot reply path stays available (the bare-metal session and the serial sink still work). Equality regression.
- **ISC-5 — standards.** SPDX; `cargo fmt --all --check`; clippy `-D warnings` (kernel blocking) via Docker. No new cross-cutting invariant. Any new `unsafe` (fb MMIO already exists) minimal + justified.

## 3. Governance fault-containment (I-14)
- **(a) Lock isolation.** The **USB-HID keyboard IRQ/poll path** must take **no** AI-governance lock — assert it (the HID input feeds a line buffer; the governance locks are untouched).
- **(c) Memory isolation.** The HID transfers ride the **already-fenced** xHCI/USB path (I-14c, W1/V proven); the framebuffer is **MMIO writes** (no device-initiated DMA). State both — no new unfenced DMA.
- **(b) Liveness.** Inherited; note that **OT-40** (Jarvis, in flight) makes `sys_generate` preemptible — until it lands, a long reply blocks the calling CPU (the console will feel frozen mid-generation; that's the known OT-40 behavior, not a console bug).

## 4. Blind-hardware note (the real OptiPlex display + keyboard)
The on-silicon payoff — a real USB keyboard + the OptiPlex's GOP framebuffer — is **device-less-different** from QEMU. Light discriminators for the bare-metal confirmation (which folds into Cerebro's session or a follow-on): real **USB keyboard enumeration** (does the dongle/keyboard bind via the real xHCI?) and the **GOP framebuffer mode** (resolution/stride differ from QEMU's std-vga). Emit a console breadcrumb at init (`fb_console res=… stride=…`, `hid_keyboard bound`) so a blank screen / dead keyboard self-localizes.

## 5. Scope guard + MERGE coordination
- **Scope: the interactive console loop ONLY** — do NOT touch mt7601u/xHCI-WiFi/DHCP (the live bare-metal connectivity subsystem), the forward arithmetic, the chat template, or `.neom`/vocab.
- **MERGE coordination (shared Arthur boot path):** ISC-1 touches `neo-arthur/main.rs` (the Arthur I/O loop), which **OT-40 (Jarvis)** and the **bare-metal session** also touch. Develop in parallel off `main`; the PM sequences the merges (rebase discipline) so the three land cleanly — flag any `neo-arthur/main.rs` overlap in the RETURN.

## 6. Gate + close (PM)
- **`arthur_console` interactive QEMU gate green** at `NEO_SMP=4` (keystroke-injection → coherent framebuffer reply) + the kill-switch equality. **Kernel-touching ⇒ AAG** (lenses: concurrency on the line-buffer/keyboard-IRQ vs the REPL; interrupt/I-14 on the HID path; completeness — does the gate prove the *framebuffer* shows the reply, not just that generate ran?). **Stagger the `workflow_dispatch` against CB-3/OT-40** (runner cap).
- **Bare-metal "talk to it on screen" = a confirmation run** (folds into Cerebro's bare-metal session or a follow-on) made self-diagnosing by §4 breadcrumbs.
- **Close-gate `scripts/worker-close-gate.sh`** (rebase LAST; full set; SPDX; self-stat). RETURN green push-CI + the 4-shard QEMU URL + the I-14 statements + the kill-switch-equality evidence. **Disk reclaim** after merge per CLAUDE.md.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-24-phase-8-arthur-physical-console-janus.md` (+ `fb/console.rs`/`console_logic.rs`, `usb/hid_keyboard.rs`/`hid_logic.rs`, the `arthur_console` scenario in `test_harness.rs`, `neo-arthur/main.rs`). **Branch:** `codex/phase-8-arthur-physical-console-janus` off `origin/main` HEAD.
- **One-line:** **Make the box conversational on its own screen — type on the OptiPlex keyboard, read Arthur on the OptiPlex display.** Complete-and-wire: the fb console (`eacb7f7`), USB-HID keyboard, and the `arthur_console` scenario (`005b980`) are landed — **OBSERVE what `arthur_console` already drives**, then wire the **interactive REPL**: read a line from USB-HID → `frame_turn_ids` (the OT-37 path) → `sys_generate` → render the reply to the framebuffer → loop. **Gate:** the `arthur_console` QEMU scenario, now **interactive** — inject keystrokes → assert the **framebuffer** shows a coherent reply (reuse the OT-37 deterministic fixture) at `NEO_SMP=4`. **Kill-switch** `console-interactive` off ≡ current serial/hardcoded behavior bit-for-bit (keeps the bare-metal serial sink working). **I-14:** the HID IRQ path takes no governance lock; HID rides the already-fenced xHCI, framebuffer is MMIO (no new DMA) — state each. Note **OT-40** (Jarvis) makes `sys_generate` preemptible — until it lands the console feels frozen mid-reply (known, not a bug). **Light bare-metal breadcrumbs** (`fb_console res=…/stride=…`, `hid_keyboard bound`) for the real-display/real-keyboard confirmation. **Scope: the console loop ONLY — no mt7601u/WiFi/DHCP, no forward/template change.** **MERGE coordinates with OT-40 + the bare-metal session on `neo-arthur/main.rs` — flag overlap; stagger your QEMU gate against CB-3/OT-40.** **Close-gate `scripts/worker-close-gate.sh`; RETURN green push-CI + QEMU URL + I-14 + kill-switch equality.**
