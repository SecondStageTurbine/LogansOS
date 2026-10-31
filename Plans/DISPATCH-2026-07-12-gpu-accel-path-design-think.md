<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: The GPU / acceleration path — the shared browser+desktop prerequisite (DESIGN THINK, host-only, CROSS-MODEL PANEL) → Oracle AND Janus

**Date:** 2026-07-12
**From:** PM (Claude)
**To:** **Oracle AND Janus — this dispatch goes to BOTH as a cross-model judge panel** (Oracle = Claude-family, Janus = Codex-family; the convergence-signal discipline for this load-bearing seam). Each produces an **independent** design THINK — do NOT collaborate, do NOT read each other's draft; the PM synthesizes (strongest architecture wins, best ideas graft). Convergence between two families is strong evidence; divergence surfaces the blind spot.
- **Oracle:** branch `codex/gpu-accel-path-design-oracle`, output `docs/THINK-2026-07-12-gpu-accel-path-design-oracle.md`.
- **Janus:** branch `codex/gpu-accel-path-design-janus`, output `docs/THINK-2026-07-12-gpu-accel-path-design-janus.md` (Janus carries the freshest Servo/WebRender-GL context from OT-61 — use it, but design independently).
**Status:** DISPATCHED — **design THINK, HOST-ONLY, no code, no QEMU. (Prometheus is offline until the PL's post-Cerebro reboot — not on this panel; a Claude+Codex pair is the available cross-model split.)** Ratified GO (PL 2026-07-12): pull the GPU/accel path forward now. It is the **shared prerequisite for BOTH the release-gating browser (OT-61, Servo/WebRender-GL) AND the desktop GUI (OT-52, the Atrium compositor)** — so design it once, to serve both. Runs in parallel with OT-59 (host-only, no gate, no runner contention).

## Why this exists now
- **Browser (OT-61, ratified release-gating):** Servo paints via **WebRender + `surfman`/GL** (Janus's audit corrected the earlier "wgpu" assumption) — a real GPU/GL accel surface is on the browser's ship-critical path.
- **Desktop (OT-52, the Atrium):** design-complete through G1 but **unbuilt — it is gated on exactly this accel path** (no GPU path ⇒ no compositor ⇒ no windowed desktop).
- One accel substrate serves both; building it twice would be waste. This THINK designs that substrate.

## ISC-1 — establish the CURRENT GPU state (ground truth first, no assumptions)
We are NOT at zero. Survey what exists firsthand (grep/RAG + read): the Phase-6 **Intel GPU bare-metal bring-up** (the iter4–7 ring-fault / GGTT-vs-PPGTT saga — `0x9e1` RING_FAULT, address-space work). State precisely: what hardware/gen it targets (the OptiPlex's Intel graphics), where that bring-up **landed** (booting? faulting? what does it expose today — a framebuffer? a ring? GGTT/PPGTT mappings?), and what the current `fb_console` framebuffer path provides. Every later design choice traces to this real state, not a clean-slate fantasy.

## ISC-2 — the accel architecture (serve WebRender/GL first, keep a route to wgpu/Vello)
Design the layers from the driver up to what the two consumers need:
- **What Servo/WebRender needs:** a GL/context/present route (WebRender 0.69 + `surfman` + `glow`/platform-GL/ANGLE edges — per OT-61's findings). Does LogansOS implement a supported GL/context/present path, or fund a WebRender backend port? State the seam and cost.
- **What the Atrium compositor needs:** surface allocation, damage/present, the cap-native surface model (G1).
- **The software-render floor:** a CPU-rasterized path (Vello CPU / tiny-skia class) as the bring-up floor that proves the pipeline before GPU accel is real — and the honest boundary of what it can do (fine for the compositor + the Blitz reader; not enough for real-web perf).
- **The wgpu/Vello convergence route** kept open for later (WebGPU sites + a unified renderer), not built first.

## ISC-3 — the driver + capability + DMA model (I-14, mandatory — this is a device driver)
This is a connectivity/device-driver-class build, so the **governance fault-containment analysis (I-14)** is required (same standing as any driver dispatch):
- **Lock isolation (a):** the GPU IRQ/ISR path acquires no AI-governance lock.
- **Liveness (b):** a GPU fault/hang degrades (fall back to software-render / the framebuffer console), never stalls or corrupts governance.
- **Memory isolation (c):** GPU DMA (the GGTT/PPGTT command/framebuffer buffers) must route through `dma_arena::fence(...)` under IOMMU containment — unfenced GPU DMA is forbidden. The GPU is the **highest-bandwidth DMA device we'll have**; this is the stress test of I-14(c).
- **The capability-sandbox for the accel surface:** who can allocate GPU surfaces / submit command buffers? A hostile browser renderer must not get raw GPU submit — the accel surface is cap-gated (ties to OT-61's `NetDialCap`-style "kernel-enforced authority, not a UI grant" lesson, applied to GPU submit).

## ISC-4 — consult the pitfall bank + trap-mine the GPU driver (both standing rules apply)
- **Concurrency pitfall bank (OT-62, now normative):** the GPU path is cross-CPU/interrupt/DMA code — consult `docs/concurrency-pitfalls.md` and map each relevant CP-* (IRQ-safe locking, ACK/EOI ordering, DMA fencing, publication-before-ready) to introduce/guard/N-A + discriminator.
- **Driver trap-mining:** enumerate the documented Intel-GPU / i915 / GEM/GGTT field gotchas from prior art (Intel PRM, Linux i915 / Mesa, OSDev) — **read for principle, never copy GPL code into our MPL kernel** — and pre-seed the failure-mode bank (ring faults, fence/sync, stolen memory, mode-set) before any bare-metal run.

## ISC-5 — the build slices + kill-switch + honest sequencing
Smallest-first slices (host-testable where possible): the software-render floor first (proves the compositor + reader pipeline with no GPU risk), then the GL/present route, then GPU-accelerated WebRender. Kill-switch: accel-off ≡ software-render/framebuffer, provably. Honest sequencing against the roadmap: this is on the critical path to BOTH the browser (1.0 gate) and the desktop; state which slice unblocks the desktop's first pixels vs. the browser's accel needs, and flag the bare-metal (OptiPlex) GPU runs (blind-hardware ⇒ the failure-mode/discriminator-section rule applies when those are dispatched).

## Scope guard
Design THINK only — no code, no QEMU, no CI fired. Primary sources for the current-state survey (our own tree) and the prior-art (Intel PRM / Linux / OSDev — read, never copy). Don't re-open ratified decisions (the browser path, the Atrium identity, install-is-data). This THINK designs the accel substrate + its slices; the PL/PM ratify the architecture before any build opens.

## Gate + close
- **Docs-only ⇒ no CI run exists** — say so; `scripts/worker-close-gate.sh` host set locally (skip-with-note). **Diff the RETURN vs the committed artifact.**
- **RETURN:** the THINK path + the current-GPU-state survey + the accel architecture (WebRender/GL-first + software floor + wgpu/Vello route) + the I-14 driver/DMA/cap model + the pitfall-bank + trap-mining pass + the build slices/kill-switch/sequencing.
- **Disk reclaim** after merge.

---
## Relay — what to tell EACH of Oracle and Janus (send to BOTH; each works ALONE)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-12-gpu-accel-path-design-think.md` (+ `git show origin/main:docs/OPEN-THREADS.md` **OT-63** [this], **OT-61** [the browser's WebRender/GL accel needs + the ratified decisions], **OT-52** [the Atrium compositor this serves], **OT-62**/`docs/concurrency-pitfalls.md` [consult — normative]; + grep/RAG the Phase-6 Intel GPU bring-up history + `fb_console`). **Branches:** Oracle → `codex/gpu-accel-path-design-oracle` (`docs/THINK-2026-07-12-gpu-accel-path-design-oracle.md`); Janus → `codex/gpu-accel-path-design-janus` (`docs/THINK-2026-07-12-gpu-accel-path-design-janus.md`).
- **One-line:** **Design the GPU/accel path — the ONE substrate serving BOTH the release-gating browser (Servo/WebRender+`surfman`/GL) and the Atrium desktop (OT-52's compositor).** Deliver YOUR independent design: (1) survey the CURRENT GPU state first (the Phase-6 Intel bring-up — where it landed, what it exposes; no clean-slate assumptions); (2) the accel architecture — WebRender/GL-first (the GL/context/present seam Servo needs) + a **software-render floor** (Vello-CPU/tiny-skia, the bring-up floor for compositor+reader) + a wgpu/Vello route kept open; (3) the I-14 driver/DMA/cap model (GPU = highest-bandwidth DMA device — GGTT/PPGTT buffers through `dma_arena::fence` under IOMMU; GPU-submit cap-gated, no raw renderer access); (4) consult the pitfall bank (map each relevant CP-*) + trap-mine the Intel-GPU/i915 prior art (read for principle, **NEVER copy GPL into our MPL kernel**); (5) build slices (software floor → GL/present → GPU-accel WebRender) + kill-switch (accel-off ≡ software/framebuffer) + honest sequencing (which slice = desktop's first pixels vs browser's accel). **INDEPENDENT — don't read the other's THINK (cross-model pair, Claude+Codex; the PM synthesizes). Host-only, no code, NO QEMU/CI.** Close-gate host set (skip-with-note); diff RETURN vs artifact.
- **RAG:** use it for our GPU/framebuffer/DMA surfaces (`fb_console framebuffer`, `intel gpu ring GGTT PPGTT`, `dma_arena fence IOMMU`, `compositor surface cap`). Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`; if down/degraded the PL owns RAG infra — note it in the RETURN and fall back to grep/Read (never a blocker).
