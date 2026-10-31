<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE REPORT — Phase 6: AI Accelerator Driver Class

**Date:** 2026-06-07
**Status:** **COMPLETE.** Opened 2026-06-04, closed 2026-06-07. Bible Article 13 marked Complete; README roadmap flipped Phase 6 → Complete, Phase 7 → next.
**Closing commit:** `fff256d` (BUILD-4b-2 Stage-B PASS) on `main`.
**Authoritative ADRs:** ADR-030 (driver-class substrate), ADR-031 (virtio-accel protocol), ADR-032 (Intel Gen9 GPU compute driver). ANSPEC-001 §9 (incl. the §9.4 mixed-precision reference rule added this phase).

## 1. What Phase 6 delivered
A kernel-managed **`accel` driver class** — AI accelerators as first-class kernel devices (the `AccelDriver` trait + registry, mirroring the block-device registry; `DeviceId` residency with `DeviceStorage`; explicit `tensor_migrate`) — with four backends behind one trait, and inference (`SYS_INFER_RUN`) dispatching ops through it with CPU fallback for uncovered ops:

| Build | Backend / deliverable | Verified |
|---|---|---|
| BUILD-1 (`9d8c6cf`) | `accel_cpu` reference driver; trait + registry + residency + `tensor_migrate` | QEMU SMP=4 |
| BUILD-2 (`f5076d6`) | `accel_sim` — in-kernel async multi-device de-risk | QEMU SMP=4 |
| BUILD-3a/3b (`305875e`/`34e3291`) | `virtio-accel` — custom vhost-user device, full op set over real virtqueue/DMA/IRQ | CI (pinned QEMU) |
| BUILD-4a (`ab8fa0d`) | **Real Intel UHD 630 (Gen9.5) GPGPU** — `[1,2]+[3,4]=[4,6]` byte-exact | **Bare metal (OptiPlex 7080)** |
| BUILD-4b-1 (`dc27459`) | Generalized op dispatcher (kernel-table) + **matmul** on the GPU | **Bare metal** |
| BUILD-4b-2 (`fff256d`) | **GELU** on the GPU + forward-pass-equivalence + **tensor migration** | **Bare metal** |

**Net:** a from-scratch monolithic kernel runs transformer math — matmul, add, and a hardware-tanh GELU — on a real Intel iGPU's execution units, with transparent `accel_cpu` fallback for the rest of the op set (layernorm/rmsnorm/softmax/embedding) and cpu↔device tensor migration, all bare-metal verified. The compute path is owned end-to-end (driver, GGTT memory, command submission, dispatch ABI, op kernels); vendor tooling (IGC/`ocloc`) is used only as an **offline** compiler — there is no runtime framework, no userspace driver, no OpenCL/CUDA/Vulkan dependency on the running system.

## 2. The Intel GPU bring-up (the hard part)
ADR-032 holds the full log. Highlights: a GuC-free **legacy render ring** path; GGTT-mapped `DmaBuffer`s via GTTMMADR; `STATE_BASE_ADDRESS`; the **NEO/IGC indirect-data thread payload** (per-thread local_id r1–r6, cross-thread r7–r9 — the convention the `ocloc` blob actually requires, *not* i965). Eight bare-metal iterations to first compute; the decisive unlocks were iter6 (STATE_BASE_ADDRESS — the EU could finally fetch the kernel), iter7's **GPU fault-address capture** (turned a fourth blind hang into a precise "wrong-GRF" verdict; codified as a CLAUDE.md standing rule for blind-hardware dispatches), and iter8 (the NEO indirect-data reframe).

## 3. Silicon results at close (BUILD-4b-2, OptiPlex 7080, UHD 630, dev 0x9bc8)
- **Add** `[4,6]` byte-exact; **matmul** exact + non-exact, `max_rel=0` (Gen9 EU has no FMA → tracks host f32 bit-for-bit).
- **GELU** `[-2,-1,-0.5,0,0.5,1,2,3]` vs host-f32: `max_rel = 1/2²⁴`, within tol — **all 8 lanes, non-vacuous.** The first op whose GPU result diverges from host-f32 (Gen9 hardware-tanh ≠ libm), which is what finally validated the tolerance apparatus (host-f32 reference, ≤2⁻¹² op-level bound, ANSPEC §9.4) end-to-end. GELU was chosen for the close precisely to make the tolerance non-vacuous after matmul came back bit-exact.
- **Mixed forward** (`valid_transformer_block`, matmul/add→GPU, layernorm/rmsnorm/softmax→`accel_cpu`): within bound `B=8·2⁻¹²`; pure-CPU run byte-exact to the Phase-5 oracle.
- **Migration** cpu→intel→cpu: round-trip clean, `allocs==frees`, no leak.
- Every dispatch `RING_FAULT=0`, `INSTDONE=0xfffffffe` (EU idle).
- **2 hardware iterations:** run #1 caught a real defect — GELU was the first >4-lane GPU dispatch (Add=2, matmul=4) and the hand-trimmed execution mask dispatched only lane 0; fixed with full SIMD32 + the kernel's own bounds check (NEO-correct).

## 4. Honest limitations (carried, not hidden)
1. **Forward-equivalence has low diagnostic sensitivity.** The model's terminal softmax flattens the output, so the forward bound `B` is satisfied *trivially* — the **op-level GELU test is the load-bearing tolerance validation**; the forward test proves mixed-routing *mechanism*, not divergence-robustness. A grossly-wrong GELU would still pass the forward test. (Sharpening: assert forward-equiv at the pre-softmax block output.)
2. **matmul/add still use the hand-trimmed walker mask** — proven only ≤4 lanes. GELU's full-SIMD32 + bounds-check pattern should be back-ported to make their ≤32-element slice real.
3. **Single-device dispatch only.** The kernel *owns* residency (`DeviceId`/`tensor_migrate`) but does not yet *reason* about it — placement is explicit, not policy. That is Phase 7.
4. **Integrated, coherent memory only.** The residency/copy model is host↔GGTT-mapped memcpy (no discrete VRAM). A discrete-memory backend is the real stress test of whether the abstraction is general or unified-memory-shaped.

## 5. Post-Phase-6 hardening backlog (tracked, non-blocking)
- **Stateful-BTI / DRM-expressibility** (ADR-023) — the production surface-state path; deferred per design.
- **Full-SIMD32 mask back-port** to matmul/add (limitation #2).
- **Pre-softmax forward-equivalence assertion** (limitation #1).
- These are engineering hardening, not conceptual gaps; Phase 6's thesis is proven without them.

## 6. What this proves, and what it doesn't
Phase 6 demonstrates **sovereignty of execution**: the kernel owns dispatch, residency, movement, and inference invocation on real accelerator hardware, with no userspace runtime in the path. It does **not** yet demonstrate **sovereignty of decision-making** — placement policy, sharing policy, multi-tenant arbitration, dedup. That is the Phase 7 research question, and it is where the project's distinctive claim ("intelligence as a kernel-governed resource, not just kernel-run") will be tested: whether kernel ownership of the tensor *and* the device boundary enables cross-tenant governance a userspace+vendor-runtime stack structurally cannot. The substrate for that question now exists and is bare-metal-real. Building the substrate first was the correct order — mechanism cannot be faked.

## 7. Next
**Phase 7 — inference scheduler / multi-tenant model serving.** The first milestones that would test the governance thesis (not throughput): a policy-driven placement decision; a cross-tenant arbitration case (one principal granted, another deferred/denied, over a shared kernel-owned device); cross-process capability-mediated tensor sharing/dedup. A THINK pass opens Phase 7.
