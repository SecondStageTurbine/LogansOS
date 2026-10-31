<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: The integrated showcase — Arthur-on-the-compositor session (design + build) → Jarvis

**Date:** 2026-07-21
**From:** PM (Claude)
**To:** Jarvis (main-tier; Milestone-A capstone owner — knows the compositor/input/bare-metal path cold)
**Status:** DISPATCHED — **design + build in one shot** (PL-ratified: Arthur renders **as a G1 compositor surface**, one integrated boot session, OptiPlex bare-metal capstone). Kernel-touching build + a **JOINT PL+coder bare-metal Arthur run**. No separate design-ratification gate — the load-bearing decision is already made; you author the session spec inline and build to it.

## What this is (the milestone)
The **first real "sit down and use the AI OS" session** — the fusion of **Milestone A** (G1 compositor + kernel-routed USB-keyboard input, proven on silicon `3ec1aba`) with **Arthur** (on-device inference + conversation, proven bare-metal OT-28) into **one boot session**:

> Boot → the G1 compositor comes up → **Arthur greets you in a conversation surface** → you type a question on the USB keyboard → keystrokes route via kernel **InputFocus** to Arthur's surface → Arthur runs a real forward pass → **answers into its composited surface.**

Every piece is proven **individually** on silicon. The new work is the **integration seam**, not new capability — so scope it as an integration build, honestly (in cycles/gates/dependencies, **never calendar time** — per the standing no-human-time rule).

## The ratified design decision (build to this)
**Arthur is a G1 compositor CLIENT surface, NOT a direct framebuffer console.** This is the honest integrated stack:
- Arthur is a compositor client using the surface protocol (ADR-073) — its conversation renders into a `SharedPixelBuffer` surface that **G1 composites**, not raw framebuffer writes.
- The USB keyboard routes to Arthur via the kernel **InputFocus** path (ADR-074, the no-snoop kernel-routed input from Milestone A) — Arthur's surface holds input focus; keystrokes reach Arthur's stdin.
- Arthur's inference output is drawn into its surface and committed via the damage/commit path.
- The `framebuffer-console` direct-draw path (what `neo-test-arthur-console` used) is **superseded** for this session — that was Arthur owning the raw framebuffer; here Arthur is a well-behaved compositor client.

## Part A — the session spec (author it as you build; it's the doc deliverable)
Write a design doc (`docs/THINK-2026-07-21-integrated-showcase-arthur-compositor.md`, CC-BY-4.0) capturing:
1. **The session flow** + what renders where (the compositor owns the screen; Arthur is a surface; any greeting/status is composited).
2. **The feature configuration** — ONE build with `framebuffer` + `atrium-compositor-g1` + `usb-hid` + Arthur inference (the model path) all resident. **This combo has likely never been compiled together** — enumerate the feature interactions and flag any that fight (e.g. framebuffer-console vs G1 owning the FB; scenario-gating collisions).
3. **Memory footprint (the thing to watch).** The 485 MB Qwen2.5-0.5B `.neom` + vocab + compositor buffers + USB structures + inference scratch **resident simultaneously** on the OptiPlex's RAM. OT-28 proved the model loads; the compositor is light; but confirm the *combination* fits and doesn't OOM — this is a named discriminator, not an afterthought.
4. **Model-blob staging (ISC-2, out-of-band).** The `.neom` + vocab are **not in git** — staged as a Limine module / NeoFS volume the same disciplined way as OT-28 (blob-less checkout omits it + fails closed). The showcase ISO carries the model like the OT-28 bare-metal ISO.
5. **The Arthur↔compositor render seam** — the main unknown. How Arthur's text output becomes a `SharedPixelBuffer` surface (glyph raster into the RW mapping), how input focus is granted to Arthur's surface, how the conversation scrolls/commits damage. This is the seam that may not exist yet; designing it is the core of Part A.

## Part B — the build ladder (honest; cycles/gates, no calendar)
1. **Slice A — Arthur greets as a G1 surface (no input yet).** Arthur's greeting renders into a compositor surface that G1 composites onto the framebuffer. Proves the **render seam** (the primary unknown). QEMU-gate the surface render; bare-metal shows the greeting *composited* (not raw-framebuffer).
2. **Slice B — the full loop (the capstone).** Keyboard → InputFocus → Arthur's stdin → a real forward pass → the answer composited into Arthur's surface. The **JOINT bare-metal OptiPlex run**: you drive serial analysis, the PL drives the hardware — sit down, type a question, Arthur answers on the composited framebuffer.
- Each slice ships a **kill-switch** (showcase-mode disabled ≡ the prior layer / the standalone scenarios still pass bit-for-bit) and its own gate.

## Standing-rule sections (required)
- **Kill-switch (Rule-1):** the integrated showcase is a build config; disabled ≡ the separate proven scenarios unchanged. Ship the equality regression.
- **I-14 fault-containment (a/b/c).** This fuses already-contained pieces — assert the containment survives the combination: (a) the input→Arthur→render path acquires **no** AI-governance lock; (b) liveness — a stall in one surface degrades, never stalls governance; (c) no new unfenced DMA (USB-DMA stays fenced; Arthur's accel tensors stay under residency governance + the existing DMA arena). State each.
- **Concurrency pitfall-bank consultation (`docs/concurrency-pitfalls.md`).** The keyboard-IRQ → InputFocus → Arthur-wake → inference → surface-commit loop adds scheduling/IPC seams — for each pitfall class it touches (scheduler-wake, lock-ordering, cross-CPU handoff), state introduce/guard/N-A + the discriminator.
- **Blind-hardware failure-mode + discriminator bank** (mandatory — this has a bare-metal run). Enumerate the likely failures *beyond* the happy path and a discriminator for each: feature-combo OOM (model + compositor + USB resident); Arthur's surface fails to composite (render seam bug); input never reaches Arthur (focus-routing bug); inference hangs with the compositor resident (scheduling/memory interaction); model-load fails with the compositor already up. Serial breadcrumbs at every seam (boot → FB → G1 up → model load → Arthur surface → greeting composited → keystroke → focus → inference → answer composited). Capture-before-you-need-it.
- **Bare-metal runbook:** reference `docs/runbook-baremetal-arthur-optiplex.md` (read via `git show origin/main:...`) for SSH/boot-select/serial-capture mechanics — the run is JOINT by construction; serial log = proof, **no photo**.

## Close-gate + merge
- `scripts/worker-close-gate.sh` (rebase-fresh, the exact CI host-check set, SPDX, self-stat + evidence). The two hard rules: **green 4-shard `workflow_dispatch` QEMU URL** (kernel-touching) and the PM inspects the commit's own `--stat`.
- Slice A is QEMU-provable (compositor surface render); Slice B's full keyboard→Arthur loop is **silicon-only** on the bare-metal run (like the USB hub path — QEMU has no real USB keyboard for the Arthur loop) — label it honestly, the JOINT OptiPlex run is its proof.
- **Disk reclaim:** `Workers/janus` is your persistent per-seat tree — reused, note it in the RETURN.
- Branch `codex/integrated-showcase-arthur-compositor-jarvis`; PM cherry-picks the design doc + merges the build after gate + AAG; the JOINT bare-metal run closes the milestone.

## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-21-integrated-showcase-arthur-compositor-jarvis.md`. Base pieces: `git show origin/main:docs/ADR-073-atrium-surface-protocol-v1.md` (surface protocol Arthur becomes a client of), `…:docs/ADR-074-atrium-kernel-routed-input.md` (InputFocus you route the keyboard through), the OT-28 Arthur bare-metal record + `…:docs/runbook-baremetal-arthur-optiplex.md` (the JOINT run mechanics). Work in `Workers\janus`; branch `codex/integrated-showcase-arthur-compositor-jarvis`.
- **One-line:** **Design + build the integrated showcase** — one boot session fusing Milestone A (G1 compositor + kernel-routed USB input, yours from `3ec1aba`) with Arthur (OT-28 inference): boot → compositor up → **Arthur greets in a G1 compositor SURFACE** → you type → keystrokes route via InputFocus to Arthur → real forward pass → **answer composited into Arthur's surface**. Ratified decision: **Arthur is a G1 client surface, NOT a direct framebuffer console** (supersede `framebuffer-console`). Ladder: **Slice A** = Arthur greeting as a composited surface (proves the render seam, QEMU-gated); **Slice B** = keyboard→InputFocus→Arthur→inference→composited answer (the **JOINT bare-metal OptiPlex capstone**). Watch the never-combined feature combo (framebuffer+G1+usb-hid+model) and the **memory footprint** (485 MB model + compositor + USB + inference resident — a named discriminator). Kill-switch (showcase-off ≡ standalone scenarios bit-for-bit), I-14 a/b/c across the combo, concurrency-pitfall consult for the input→Arthur→render loop, full blind-hardware discriminator bank, runbook-referenced. Close-gate ritual + green QEMU URL; Slice B is silicon-only (label it). Design doc `docs/THINK-2026-07-21-integrated-showcase-arthur-compositor.md`.
- **RAG:** `atrium surface protocol SharedPixelBuffer client`, `InputFocus kernel-routed input focus`, `arthur console framebuffer inference forward pass`, `neom model load NeoFS module`, `compositor present damage commit`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read (PL owns RAG startup; never a blocker).
