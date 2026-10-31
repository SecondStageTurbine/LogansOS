<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-63 GPU Slice 3 — I-14(c) scanout-DMA fencing (the glass containment leg) → Janus

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Janus (main-tier; free lane — Codex-family, cross-model from the Claude PM's AAG)
**Status:** DISPATCHED — kernel-touching (the present/flip scanout path) ⇒ 4-shard `workflow_dispatch` QEMU gate + AAG. **This is a driver/DMA-containment build ⇒ the I-14 fault-containment analysis is the CORE of the slice, not an appendix.** **Interrupt/flip-touching ⇒ CP-bank consultation.** QEMU-provable; **no bare-metal** (the hardware iGPU→translated-VT-d-domain move is Slice 5). Builds on the merged OT-63 Slice 1 (present backend) + Slice 2 (GpuSubmitCap).

## What Slice 3 is — fence the scanout DMA
Slice 2 gated **who** may submit a HardwareFlip (`GpuSubmitCap`). **Slice 3 fences what that flip's DMA can reach:** every scanout buffer that gets armed as a hardware-flip target must route through **`dma_arena::fence(phys, len)`** (the I-14(c1) gate, ADR-045) so a GPU scanout DMA is **contained** — it cannot reach AI-governance or tensor memory. This is the I-14(c) leg of the containment invariant, applied to the display path, and it is the prerequisite that makes **glass** safe to composite on the GPU later (a backdrop-blur compute pass reads the framebuffer — the scanout region must be fenced first).

Cerebro's ADR-073 §I-14(c) already staged the seam: the `SharedPixelBuffer` frames are ordinary kernel-owned page-aligned `PhysFrame`s, **fence-compatible as-is**, so the scanout path "wraps them in `dma_arena::fence(...)` without any change to the buffer object." That's your insertion point.

## The fence insertion (the mechanism)
At the flip-arm seam (`present_backend::arm_flip` / the `present()` HardwareFlip branch, `kernel/src/atrium/buffer.rs`), before a buffer is armed as a scanout/flip target, its frame span passes `dma_arena::fence(phys, len)`:
- **Admits** (returns `FencedDmaAddr`) ⇒ the flip arms with the fenced address.
- **Refuses** (`DmaFault` / fails-closed on `ArenaUnset`) ⇒ the HardwareFlip is **refused** and degrades to the Software present (I-14(b) bounded degradation — never a stall, never an unfenced flip). Bump `fence_refusals`.

**No scanout DMA is ever programmed from an unfenced address.** That is the falsifiable property.

## The load-bearing design question (resolve it, don't hand-wave)
Today the `SharedPixelBuffer` frames come from the general allocator — they are **not** arena-resident, so a naive `fence()` would **refuse every scanout** (fail-closed = safe but non-functional). You must make the scanout target fence-admissible. **Recommended:** allocate the scanout/flip buffer's frames **from the fenced DMA arena** (so `fence()` admits them and any out-of-arena address is refused), keeping the `SharedPixelBuffer` object otherwise unchanged — an arena-backed scanout allocation path, selected when the buffer is a flip target. *(Alternative: register/admit the buffer's spans into the arena at flip time. Pick one, justify it, and keep the change additive to Cerebro's buffer object — coordinate on `buffer.rs`.)* State the choice + why in the RETURN and ADR.

## Scope boundary — do NOT build here (Slice 5 bare-metal)
- **The hardware iGPU→translated VT-d domain move** (moving the iGPU out of the identity domain into a fenced/translated domain, RMRR + aperture reconciliation) is **I-14(c2) on real Gen9 silicon = Slice 5 bare-metal.** Slice 3's containment is the **software `dma_arena` fence** + the emulated `intel-iommu` under QEMU. **State the c2 seam; do not attempt the real domain move.**
- **The real Gen9 `PLANE_SURF` flip / vblank** is Slice 4-5. Slice 3 stays on the synthetic-vblank present path.

## Kill-switch (Rule-1) + byte-oracle (I-12)
- **Kill-switch:** scanout-fencing-**off** ≡ the Slice-2 HardwareFlip path bit-for-bit (armed regression — with fencing disabled, arm_flip is byte-unchanged; the shipped posture is fencing-**on** since I-14(c) is a containment invariant, but the disabled-mode equality must be provable).
- **Byte-oracle:** if Slice 3 defines a new fenced-scanout descriptor / span-record byte layout, hand-lay a fixture; if it rides the existing `dma_arena::fence` API + `FencedDmaAddr` (no new on-wire/on-register layout), **state that** — no new oracle needed. (The real Gen9 `PLANE_SURF`/GGTT register layout is Slice 5's byte-oracle, not this slice's.)

## I-14 fault-containment analysis (the CORE — required)
This slice IS I-14(c) for the display path. State + prove:
- **(a) Lock isolation:** the fence check on the arm_flip path acquires **no** AI-governance lock (`dma_arena::fence` is a lock-free span-table lookup, ADR-045); confirm.
- **(b) Liveness degradation:** a refused (unfenced) scanout degrades to the Software present — governance stays live, no stall (the I-14(b) backstop; ties to Slice-1's full-ring sync fallback).
- **(c) Memory isolation — THE POINT:** a GPU scanout DMA cannot reach governance/tensor memory. **Prove falsifiably:** a flip armed against an **out-of-arena** address (e.g. a governance/tensor physical address) is **refused by `dma_arena::fence`** (fail-closed), and the fenced scanout proceeds. This is the c1 proof; the c2 (hardware IOMMU fault on the same attempt) is the Slice-5 bare-metal companion — state it.

## CP-bank consultation (flip-touching — required)
Consult `docs/concurrency-pitfalls.md`. The fence check sits on the `arm_flip`/`on_present_vblank` seam that Slice 1 already proved (CP-2 no-IF-off-spin, the present-fence handshake, lock-free per-buffer). Confirm the fence adds no new IF-off spin, no lock held across the flip-arm, and no CP-29/I-16 regression; the fence lookup is lock-free so it doesn't change the ISR-path discipline.

## Gate + close + AAG
- Host KATs for the fence-on-scanout logic (admit / refuse / degrade) + any new oracle; a **QEMU scenario** proving: a fenced scanout admits + presents; an out-of-arena scanout is **refused** and degrades to Software (no unfenced flip, no stall); the I-14(c1) memory-isolation proof (governance/tensor address refused); the kill-switch equality (fencing-off ≡ Slice-2). `scripts/worker-close-gate.sh` (rebase-fresh on current `origin/main`; full host-check set; SPDX; Step-4 self-stat). 4-shard `workflow_dispatch` gate — **one wave at a time, stagger** behind any live Jarvis/Cerebro gate. RETURN: green push-CI + 4-shard URLs + the fence-refusal/containment evidence + the arena-allocation design choice.
- **Disk reclaim** after PM-confirmed merge (work in `Workers\janus`).
- **AAG (PM, cross-model lenses):** I-14/DMA-fencing (**no path arms an unfenced scanout DMA**; the fence genuinely refuses out-of-arena incl. governance/tensor addresses — the crown jewel), memory-safety (the frame-span→fence→flip path, no OOB, arena allocation sound), concurrency (fence on the present/flip seam, no new spin/CP regression), completeness (the QEMU-proven c1 set vs the bare-metal-only c2 VT-d-domain-move set stated **honestly** — no overclaim that the hardware IOMMU containment is proven here).

## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-ot63-slice3-scanout-dma-fence-janus.md`. Seams you build on: `git show origin/main:kernel/src/drivers/dma_arena.rs` (the `fence(phys,len)->FencedDmaAddr` gate, ADR-045), `git show origin/main:kernel/src/atrium/present_backend.rs` + `kernel/src/atrium/buffer.rs` (Cerebro's flip-arm path — coordinate, keep additive), `git show origin/main:docs/ADR-073-atrium-surface-protocol-v1.md` §I-14(c) (the staged seam). Design context: the Rung-1 plan `git show origin/main:Plans/DISPATCH-2026-07-19-ot63-rung1-gen9-flip-vblank-present-cerebro.md`. Work in `Workers\janus`. PM artifacts on `origin/main`; `git show`, not a local open.
- **One-line:** OT-63 GPU Slice 3 — **fence the scanout DMA (I-14(c)).** At the HardwareFlip arm seam, route the scanout buffer's frame span through `dma_arena::fence(phys,len)` — admits ⇒ arm the flip with the `FencedDmaAddr`; refuses (out-of-arena, fail-closed) ⇒ refuse the flip + degrade to Software (I-14(b)), **never an unfenced scanout DMA.** Resolve the load-bearing design choice: make the scanout target **arena-resident** (recommended: allocate flip buffers from the fenced arena) so `fence()` admits it and out-of-arena (incl. a governance/tensor address) is refused. **Prove falsifiably in QEMU:** fenced scanout presents; out-of-arena scanout refused; a flip against governance/tensor memory is refused. **Do NOT** build the hardware iGPU→translated-VT-d-domain move or RMRR/aperture (I-14(c2), Slice 5 bare-metal — state the seam), or the real Gen9 `PLANE_SURF`/vblank (Slice 4-5). Kill-switch: fencing-off ≡ Slice-2 bit-for-bit. Required I-14 analysis (this IS I-14(c)) + CP-bank (fence on the arm_flip seam, no new spin, no CP-29/I-16 regression). 4-shard gate (stagger) + AAG.
- **RAG:** `dma_arena fence FencedDmaAddr`, `present_backend arm_flip scanout`, `iommu identity domain igpu`, `intel_gen9_ppgtt vtd`, `SharedPixelBuffer frames PhysFrame`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it in the RETURN and fall back to grep/Read (PL owns RAG startup; never a blocker).
