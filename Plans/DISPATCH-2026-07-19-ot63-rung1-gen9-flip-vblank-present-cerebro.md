<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-63 Rung 1 — real Gen9 present (flip + vblank + present-fence) → Cerebro

**Date:** 2026-07-19
**From:** PM (Claude)
**To:** Cerebro (main-tier — deepest bare-metal + Gen9 GPU context: the Phase-6 ring-fault/GGTT-PPGTT bring-up + the recent CP-29 bare-metal diagnosis; PL may reassign to Chad)
**Status:** DISPATCHED — kernel-touching + a **GPU interrupt** ⇒ mandatory 4-shard `workflow_dispatch` QEMU gate + concurrency AAG + a **joint bare-metal Gen9 close**. OT-63 Rung 1 of the ratified accel ladder — and now a **hard prerequisite for the beautiful glass desktop**, not just the browser (the glass/frost material needs the GPU; see `docs/RESEARCH-2026-07-19-gui-prior-art-glass-frost-FINDINGS.md`).

## Where OT-63 stands + this rung's job
Ratified architecture (`docs/THINK-2026-07-12-gpu-accel-path-SYNTHESIS.md` §1-2): one kernel-owned `IntelGt` + a renderer-agnostic **surface/fence/buffer/damage seam**. **Rung 0 shipped** (the G1 compositor slice-1 with a *software* present-fence + the swgl browser floor). **The OT-59 gate that blocked the first GPU ISR is now CLEARED** (OT-59 merged). **Rung 1 = swap a REAL hardware present in behind the same seam:** a **bounded pageflip** to a scanout buffer + **vblank interrupt** handling + a **real present-fence** signaled on flip-complete/vblank — replacing Rung 0's software fence, transparently to the compositor. **Full modeset stays DEFERRED** (inherit Limine's GOP mode per ADR-032 — flip *within* the inherited mode; do NOT modeset). This unblocks hardware compositing for the desktop, is the substrate the later backdrop-blur compute pass (Rung 2b) rides for glass/frost, and is the browser's WebRender/GL present path.

Builds on the Phase-6 Intel GPU bring-up (GGTT/PPGTT/ring — the iter4-7 work); this is display-engine + present, not 3D.

## MANDATORY — pitfall-bank consultation (OT-62; the vblank ISR is a wake-race)
Read `docs/concurrency-pitfalls.md`. The **vblank GPU ISR is the CP-14/15/16 interrupt-wake-race family** (+ the CP-19…CP-29 AP-idle/cross-CPU classes now that SMP is live). State, per class your change touches: introduce / guard / leave-N-A + the discriminator. Load-bearing: the present-fence must be signaled from the vblank ISR and woken by the waiter with a **correct wake-race-free handshake** (the OT-59 I-16 readiness discipline + the CP-2/CP-3 IF-off-spin lesson — the Rung-0 `fence_wait` `with_interrupts_enabled` fix is the in-tree pattern; do NOT reintroduce an IF-off spin waiting on vblank). Do **not** regress CP-29 (serialized bringup) / I-16 (shootdown mask).

## I-14 fault-containment (MANDATORY — the iGPU is the highest-bandwidth DMA device)
1. **Lock isolation (a).** The vblank ISR acquires **no** AI-governance lock (INFLIGHT/RECENCY/ARENA/reservation/TABLE) — it only signals the present-fence into a bounded queue. Assert.
2. **Liveness degradation (b).** A GPU hang / missed vblank cannot stall governance: the **software present-fence (Rung 0) is the degraded fallback** — a bounded vblank-wait times out to the software path, never an unbounded IRQ-masked spin. This IS the kill-switch below.
3. **Memory isolation (c).** Every **scanout buffer + GGTT/PPGTT-mapped submission** routes through `dma_arena::fence(...)` under the **iGPU's VT-d/IOMMU domain** (per the synthesis I-14(c): move the iGPU identity into a *translated* VT-d domain — NOT swiotlb bounce). **`GpuSubmitCap` gates every submit; no raw renderer access to physical memory.** A GPU DMA must not reach governance/tensor memory — the scanout region is fenced + IOMMU-contained.

## Kill-switch (Rule-1) — the renderer-agnostic seam makes this clean
GPU-accel **OFF ≡ the Rung-0 software present-fence path (swgl/Vello-CPU floor), bit-for-bit** — the compositor presents identically, just via CPU blit + software fence instead of hardware flip. Armed regression: with accel off, the Rung-0 `atrium_slice1` present path is byte-unchanged. This is also the I-14(b) degraded fallback and (later) the glass Reduce-Transparency floor.

## Failure-mode + discriminator bank (blind-hardware GPU — design for the hang)
Capture-before-you-need-it (the Phase-6 `RING_FAULT` lesson — capture the fault ADDRESS + type + stage, not just "it hung"):
| # | Failure | Discriminator (serial breadcrumb) |
|---|---|---|
| F1 | **Flip never happens** (wrong PLANE/scanout reg, GGTT scanout addr) | `FLIP: submitted plane=… addr=…` present but no `VBLANK:` → the flip register/scanout-address path; dump the PLANE_CTL/SURF regs |
| F2 | **vblank IRQ never fires** (DE_IIR/pipe interrupt unmasked wrong) | `FLIP: submitted` but no `VBLANK: pipe=N` → the display-engine interrupt enable/mask (DEIER/pipe IMR); breadcrumb the IIR read |
| F3 | **present-fence never signals** (ISR→waiter handshake) | `VBLANK:` fires but the compositor waits forever → the fence signal/wake path (CP-bank); the OT-59 handshake discriminator |
| F4 | **GPU hang / DMA fault** | capture the fault address+type+stage (Phase-6 pattern); the software-floor fallback engages (I-14(b)) → `ACCEL: degraded_to_software reason=…` |
| F5 | **Tearing** (flip not vblank-synced) | visible tear on the framebuffer + a `PRESENT: unsynced` breadcrumb → flip-timing vs vblank |

## Prior-art trap-mining (MANDATORY — mine before you boot)
Mine the **exact Gen9 (Skylake/Kaby Lake, UHD 630 / HD 530) display-engine** gotchas from the **Intel PRM (Gen9 Vol 2/12 — display, pageflip, vblank/DE interrupts, PLANE/SURF registers, GGTT scanout)** and **i915** (read for PRINCIPLE — the pageflip sequence, the DE interrupt topology, the GGTT-scanout requirement — **NEVER copy GPL i915 code into our MPL kernel**). Seed the bank with the documented flip-vs-vblank ordering, the DEIER/pipe-IMR enable sequence, the GGTT-must-map-scanout requirement, and the Phase-6 ring-fault/GGTT-PPGTT lessons. Each with its discriminator, before the first boot.

## Bare-metal Gen9 close (JOINT PL run — what QEMU CANNOT prove)
QEMU can prove the **present-fence protocol, the cap-gating, the DMA-fencing, and the ISR wake-race handling under `NEO_SMP`** (host-testable + a QEMU scenario for the seam + the kill-switch equality). It **cannot** prove the real **flip + real vblank + real present-fence on Gen9 silicon** — that's a blind-hardware bring-up (like CP-29 / OT-28). Close it on a **Gen9 target: the M710q (HD 530) if its PSU is up, else the OptiPlex (UHD 630)** — both Gen9. **Reference `docs/runbook-baremetal-arthur-optiplex.md`** (the standing bare-metal procedure — the M710q supplies its own IP/boot-entries/disk-layout against the same methodology; the serial log is the proof, **no photo**; JOINT PL+coder boot-select). The run proves the flip/vblank/present breadcrumbs on real silicon; a self-diagnosing FAILURE (per the bank) is a valid RETURN — blind-hardware is iterative.

## Scope guard
Flip + vblank + present-fence behind the existing seam ONLY. **No full modeset** (inherit Limine's GOP). No 3D. No backdrop-blur compute (that's Rung 2b — this rung is the substrate it will ride). Do not regress Rung 0, CP-29, or I-16.

## AAG (PM, pre-merge — GPU/SMP)
Concurrency (the vblank ISR wake-race + present-fence handshake, CP-bank, no IF-off spin, no CP-29/I-16 regression — the load-bearing lens), I-14/DMA-fencing (scanout+submit fenced under VT-d, GpuSubmitCap-gated, no unfenced GPU DMA), memory-safety, completeness (the QEMU-proven set vs the bare-metal-only set stated honestly).

## Gate + close
- `scripts/worker-close-gate.sh` (rebase-fresh; full host-check set; SPDX; Step-4 self-stat). Kernel-touching ⇒ the 4-shard `workflow_dispatch` QEMU gate is mandatory (the QEMU-provable pieces). **Runner note:** stagger behind any concurrent kernel gate (one 4-shard wave at a time). RETURN pastes the green push-CI + 4-shard URLs, the CP-bank table, the I-14 analysis, and states the QEMU-proven vs bare-metal-only split. The **bare-metal Gen9 run** is the silicon close (its serial log RETURN'd, joint).
- **Disk reclaim (after PM confirms merge):** delete this build's tree once on `origin/main` + PM-confirmed.

---
## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-19-ot63-rung1-gen9-flip-vblank-present-cerebro.md`. Context: `git show origin/main:docs/THINK-2026-07-12-gpu-accel-path-SYNTHESIS.md` (the ratified accel design + Rung ladder), `docs/concurrency-pitfalls.md` (CP-14/15/16 + CP-19…29), `docs/runbook-baremetal-arthur-optiplex.md` (the bare-metal procedure), and the Phase-6 GPU bring-up + the Rung-0 compositor (`daa5332`). PM artifacts live on `origin/main`; `git show`, not a local open.
- **One-line:** OT-63 Rung 1 — swap a REAL Gen9 hardware present (bounded **flip + vblank interrupt + real present-fence**) in behind Rung 0's renderer-agnostic seam, replacing the software fence transparently to the compositor. **No modeset** (inherit Limine's GOP, ADR-032); builds on Phase-6 GGTT/PPGTT. **Consult CP-14/15/16 + CP-19…29** (the vblank ISR is a wake-race; use the OT-59 `with_interrupts_enabled` handshake, no IF-off spin; don't regress CP-29/I-16). **I-14:** scanout+submit fenced through `dma_arena` under the iGPU VT-d domain, `GpuSubmitCap`-gated, no raw renderer. **Kill-switch:** accel-off ≡ the Rung-0 software-fence path bit-for-bit (also the I-14(b) degraded fallback + the glass Reduce-Transparency floor). Trap-mine the exact Gen9 display-engine (Intel PRM + i915 for **principle only, no GPL copy**) into the failure bank (flip / vblank / present-fence / GPU-hang / tearing — each with a serial discriminator + fault-address capture). QEMU proves the protocol/cap/DMA/wake-race; the **real flip+vblank on Gen9 silicon is a joint bare-metal close** (M710q if PSU up, else OptiPlex; runbook; serial log, no photo). 4-shard QEMU gate + concurrency+I-14 AAG. A self-diagnosing FAILURE is a good RETURN — blind-hardware is iterative.
- **RAG:** use it for the GPU/present/DMA seams — health-check `Invoke-RestMethod http://127.0.0.1:8404/health`; e.g. `IntelGt GGTT scanout`, `present fence seam compositor`, `dma_arena fence VT-d IOMMU gpu submit cap`, `phase-6 ring fault bring-up`, `vblank pipe interrupt`. If down, note it + grep/Read (never a blocker; the PL owns RAG startup).
