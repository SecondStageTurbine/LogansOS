<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: GPU P1 Slice 2 — minimal cap-native Gen9 compute device + ABI → Cerebro

**Date:** 2026-07-21
**From:** PM (Claude)
**To:** Cerebro (main-tier; OT-63 GPU owner, ADR-077 author)
**Status:** DISPATCHED — **kernel-touching build.** Slice 1 (CPU blur floor) is merged (`34eea2a`, oracle PM-verified, AAG-clean). Slice 2 is the **compute ABI + service** rung of the ADR-077 v2 ladder: the minimal cap-native Gen9 compute *device* the blur kernel will run on — the layer above the Phase-6 execlist/PPGTT foundation. **Build to ADR-077 v2's Part-B/§ladder as the authoritative spec; this dispatch highlights the load-bearing constraints + standing rules.**

## Scope — what Slice 2 IS (and is NOT)
**IS:** the **minimal, manifest-only, cap-native Gen9 compute device + submit/queue/buffer/fence ABI**, kernel-internal, that a pinned kernel will dispatch through — with the kill-switch off ≡ the Slice-1 CPU floor bit-for-bit. **Inventory the Phase-6 Gen9 compute foundation FIRST** (execlists/LRC, PPGTT, ring submission in `kernel/src/drivers/accel/` — ADR-032) and build the device ABI *above* it; do not re-derive submission.
**IS NOT (deferred, name them):** the ocloc-compiled blur EU kernel = **Slice 3**; the real UHD 630 silicon proof = **Slice 4**; the graphics VT-d fence = **Slice 5**; the G1 wiring = **Slice 6**. Slice 2 stands up the ABI + submit path; the *actual EU dispatch of the real blur kernel* is Slice 3/4.

## Load-bearing constraints (from ADR-077 v2's amendments — do not relax)
1. **Pre-VT-d execution WHOLLY kernel-internal (amendment 4).** `Handle::GpuCompute` + the compute service are **kernel-internal only** — **NO userland syscall / no userland-reachable GpuCompute handle until Slice 5 (VT-d)**. A userland compute queue before the physical fence is exactly what the I-14(c) rule forbids. The Slice-2 device is a trusted in-kernel path (the risk-accepted pre-fence stage).
2. **Kill-switch (Rule-1).** `GPU_COMPUTE_ENABLE` default-**off** ⇒ the compute path is inert and the blur resolves to the **Slice-1 CPU floor, bit-for-bit** (ship the equality regression: compute-off produces the same `ORACLE_DST`/floor bytes Slice 1 proved).
3. **I-14 containment.** Compute buffers resolve via **per-context PPGTT** (Phase-6-proven intra-context isolation) — but per Chad's amendment-5, PPGTT is NOT the host-physical-memory boundary; that graphics **VT-d / fenced-physical-DMA domain (covering ALL GPU-reachable allocs: ring/LRC/HWSP/state/kernel/CURBE/seqno/page-tables) is Slice 5, and MUST precede any userland compute queue.** Slice 2 introduces **no new userland-reachable DMA** and no unfenced path beyond the kernel-internal PPGTT-contained compute. State the I-14 (a) no-governance-lock / (b) liveness / (c) analysis for the compute submit path.
4. **Submit authority.** Reuse the Slice-2-GpuSubmit gate (`GpuSubmitCap`) + the present-fence/seqno completion already in-tree — and heed the OT-52 Slice-A finding: **scanout-present authority is compositor-only; a compute-submit path must not become a second way to reach scanout.** Compute completion is a fence/seqno, not a present.
5. **Silicon-shape delta (document, don't run yet).** Per the cross-check: the blur kernel introduces 2-D `GPGPU_WALKER`, SIMD32 multi-group occupancy, and dependent two-pass inter-dispatch coherence (`PIPE_CONTROL` flush + MOCS). Slice 2 sets up the ABI to express these; **the silicon proof is Slice 3/4** — enumerate in the ADR what each later slice must prove on the UHD 630.

## Gate coverage (honest, like the USB-hub / Slice-A precedent)
- **QEMU-provable in Slice 2:** the cap-native ABI, the `GPU_COMPUTE_ENABLE` kill-switch equality (compute-off ≡ CPU floor), the ABI byte-layout (I-12 byte-oracle if it defines an on-wire/struct layout — hand-laid-out bytes, raw source), the submit-path *wiring* up to the hardware boundary. **Silicon-only (deferred to Slice 3/4):** the real Gen9 EU dispatch. Label the split; do not overclaim a QEMU pass as silicon proof.
- The **ocloc/Linux-host dependency is Slice 3's**, but Slice 2's RETURN should confirm the **<runner-host> ocloc host is being re-stood-up** (Janus is re-adding ryzen-5/6 in parallel) so Slice 3 isn't blocked when it lands.

## Standing-rule sections (required)
- **Kill-switch ISC** (above) — the equality regression is mandatory.
- **Concurrency pitfall-bank consult (`docs/concurrency-pitfalls.md`).** The compute submit/ring/fence path touches cross-CPU/execlist/completion seams — for each pitfall class it touches (submit/wait/complete single-in-flight à la CP-18, EOI/ISR, lock-ordering), state introduce/guard/N-A + the discriminator.
- **I-14 fault-containment analysis** (above, a/b/c for the compute path).
- **No-GPL-lift:** Mesa/i915 for Gen9 compute *principle* only.
- If Slice 2 includes **any bare-metal smoke**, add the failure-mode/discriminator bank + reference `docs/runbook-baremetal-arthur-optiplex.md`; if it's QEMU+host only (recommended for the ABI rung), say so and defer silicon to Slice 4.

## Close-gate + merge
- `scripts/worker-close-gate.sh` (rebase-fresh on current `main`, the CI host-check set, SPDX, self-stat + evidence). Green 4-shard `workflow_dispatch` QEMU URL. Kernel-touching ⇒ PM AAG (concurrency + I-14 + completeness "what's silicon-only" + the no-second-scanout-path check).
- **Disk reclaim:** `Workers/cerebro` persistent — reused, note in RETURN.
- Branch `codex/gpu-p1-slice2-compute-device-cerebro`; PM cherry-picks the ADR ladder update (if any) + merges after gate + AAG.

## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-21-gpu-p1-slice2-compute-device-cerebro.md`. Authoritative spec: `git show origin/main:docs/ADR-077-gpu-p1-compute-service.md` (v2 ladder + Part B). Base: `…:docs/ADR-032-phase-6-intel-gen9-gpu-compute-driver.md` (the execlist/PPGTT foundation to inventory + build above). Containment: `…:docs/ADR-045-phase-7.5-fenced-dma-arena.md` + `…:docs/ADR-050-phase-7.5-vtd-iommu.md`. Work in `Workers\cerebro`; branch `codex/gpu-p1-slice2-compute-device-cerebro`.
- **One-line:** **Build P1 Slice 2 — the minimal cap-native Gen9 compute device + submit/queue/buffer/fence ABI** (per ADR-077 v2's ladder), the layer above the Phase-6 execlist/PPGTT foundation (inventory it first, don't re-derive submission). **Kernel-internal ONLY — no userland GpuCompute handle/syscall until Slice 5/VT-d (amendment 4).** `GPU_COMPUTE_ENABLE` default-off ⇒ **bit-for-bit the Slice-1 CPU floor** (ship the equality regression). Reuse the GpuSubmitCap gate + present-fence completion; compute completion is a fence, NOT a present (don't open a second scanout path — the OT-52 Slice-A lesson). PPGTT-contained, no new userland DMA (the VT-d physical fence over ALL GPU-reachable allocs is Slice 5, before any userland queue — amendment 5). The real EU dispatch of the blur kernel is **Slice 3 (ocloc-compiled, on the <runner-host> Linux host Janus is re-adding) + Slice 4 (UHD 630 silicon)** — Slice 2 is the ABI/service, QEMU+host-provable (label the silicon-only split honestly). Concurrency-pitfall consult (submit/wait/complete, CP-18), I-14 a/b/c, no-GPL-lift. Close-gate + green 4-shard QEMU URL; expect PM AAG.
- **RAG:** `accel intel_gen9 execlist lrc ppgtt ring submission`, `GpuSubmit present_backend fence seqno`, `GPU_COMPUTE_ENABLE compute device`, `dma_arena fence vt-d graphics domain`, `atrium blur_backdrop GLASS_BLUR_ENABLE`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read (PL owns RAG startup; never a blocker).
