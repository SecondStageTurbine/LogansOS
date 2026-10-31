<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 6 BUILD-4b — Intel GPU op-set coverage + equivalence (the final Phase 6 build)

**Date:** 2026-06-07
**Seat:** PM/Cerebro (design-questions pass; not a dispatch)
**Context:** BUILD-4a closed 2026-06-07 (`ab8fa0d`) — a real UHD 630 EU computed `[1,2]+[3,4]=[4,6]` byte-exact on bare metal. This THINK frames BUILD-4b so we dispatch the right questions instead of guessing.
**Related:** ADR-032 (the Intel Gen9 compute driver + the 8-iteration log), ADR-030 (AccelDriver trait), ADR-023 (DRM-expressibility forward-marker), ANSPEC-001 §9, bible Article 13 / 6.5.

## 1. Goal
Make `accel_intel` a **full `AccelDriver`** that runs the real inference op set on the iGPU, so a `.neom` forward pass can route through hardware and match the `accel_cpu` reference — mirroring how BUILD-3b completed BUILD-3 (virtio_accel: full op set + `forward_equiv == accel_cpu == oracle` + migration). BUILD-4b is the **final Phase 6 build**.

## 2. What BUILD-4a already settled (do NOT re-litigate)
The expensive part — bringing up the dispatch path on blind silicon — is done and proven:
- **C1 dispatch mechanism:** GuC-free legacy render ring. ✔ silicon-proven.
- **C2 memory:** GGTT-mapped DmaBuffers via GTTMMADR, LLC + pipe-control coherence. ✔
- **C4 evidence/scanout:** framebuffer expendable, NVMe block-log sink authoritative. ✔
- **The payload ABI:** NEO/IGC indirect-data convention (per-thread local_id r1–r6, cross-thread r7–r9), stateless A64. ✔
- **The toolchain:** `tools/intel-gen9-finalizer` (pinned IGC/ocloc) emits `.text` EU streams with `ocloc disasm` + zeinfo byte oracles. ✔

So each new op in 4b is **"a new finalizer-built kernel + its arg payload on the proven dispatch path,"** not a fresh bring-up. That reframing matters for the iteration-cost risk (Q6).

## 3. The design questions

### Q1 — Op coverage: which ops run on the GPU, and what's the fallback?
The inference op set is `ai::ops` opcodes 0..11 (matmul, elementwise add/mul, gelu, softmax, layernorm, rmsnorm, embedding gather, …). Two honest options:
- **(a) Full coverage** — a Gen9 EU kernel for every op. Maximal, but every op is a finalizer kernel + byte oracle + a bare-metal validation, and transcendentals are the hard ones.
- **(b) Representative coverage + CPU fallback** — GPU kernels for the ops that prove the *class* (a matmul, an elementwise, one transcendental, the integer gather), and `accel_cpu` fallback (Article 6.5 explicitly retains it) for the rest, with per-op dispatch deciding device vs fallback.
- **Recommendation:** **(b).** The Phase-6 thesis is "inference *can* dispatch to hardware," not "every op has a hand-tuned Gen9 kernel." Prove matmul (the workhorse), one elementwise, one transcendental (softmax or gelu — exercises the hardware-FP-tolerance question), and the integer gather (exercises byte-exact). A full `.neom` forward pass then runs with GPU where a kernel exists and CPU fallback elsewhere, still producing the oracle result. Full per-op GPU coverage becomes an optimization backlog, not a Phase-6 gate.

### Q2 — FP tolerance policy (THE central decision; ADR-032 C3 left it open for 4b)
Hardware f32 ≠ soft-float (`accel_cpu`) bit-for-bit, especially for transcendentals (hardware uses table+polynomial approximations) and fused vs split mul-add rounding. So `accel_intel == accel_cpu` byte-exact is **wrong as a universal gate**. Proposed per-class policy:
- **Byte-exact (0 ULP):** integer + data-movement ops (embedding gather, copy/migrate) and exactly-representable arithmetic — held to the 4a standard.
- **Bounded tolerance:** transcendental/accumulating ops (matmul accumulation, gelu, softmax, layernorm, rmsnorm). Phase-5 precedent is `≤ 2^-12 relative vs host f32` (bible Phase-5 ledger). **Key sub-question:** the reference. `accel_cpu` is soft-float; *hardware* f32 is usually closer to *host* f32. Proposal: validate GPU output against **host f32** within a per-op relative bound, and separately keep `accel_cpu`-vs-host bounded — i.e. the oracle is "both backends within tolerance of host f32," not "GPU within tolerance of soft-float."
- **This is the PL/ANSPEC decision to make before dispatch:** the exact per-op bounds + the reference choice. ANSPEC §9.5 already permits a tolerance; 4b must *fix the numbers* and justify them, not hand-wave (the bible's lean-patch / no-hand-wave discipline).

### Q3 — Stateless A64 vs stateful BTI for the larger ops
4a's Add used stateless A64 (3 pointers, tiny buffers). Matmul has more/larger operands. Does the op set stay stateless A64, or introduce stateful BTI / SURFACE_STATE?
- **Recommendation:** **stay stateless A64 for 4b.** It's proven, it carries arbitrary 64-bit buffers, and it avoids the entire binding-table/surface-state burden. Stateful BTI is the *eventual* ADR-023 DRM-expressible production path — defer it to a later hardening build (it's already documented as deferred in ADR-032 / the kernel README). 4b proves *correctness of the op set*, not the production surface model.

### Q4 — DRM-expressibility (D10 / ADR-023): deliverable or marker?
The driver must stay *expressible* as a Linux-DRM char device for the future personality. That's an **interface constraint, not a 4b feature**: 4b should avoid AccelDriver shapes that preclude DRM expression, but building the actual DRM/char-device interface is Phase-8+/personality work.
- **Recommendation:** 4b carries a short ADR-032/ADR-023 note affirming the AccelDriver surface stays DRM-expressible; no DRM interface is built in Phase 6.

### Q5 — Slicing
Mirror BUILD-3's transport-then-ops shape. 4a was the "transport" (dispatch path). Proposed:
- **4b-1:** register `accel_intel` as a full `AccelDriver` on `Device::Accel(n)`; implement the chosen op kernels (Q1b); per-op byte/tolerance oracles (host-side + the hand-laid payload oracles per CLAUDE.md); Stage-A green.
- **4b-2:** Stage-B bare-metal forward-pass equivalence on the OptiPlex (`infer_*` routed through `accel_intel` within tolerance) + cross-device migration (cpu↔intel), closing Phase 6.
- Open question for the PL: is 4b-1/4b-2 the right cut, or do we want a per-op Stage-B gate (slower, but each op self-diagnoses on silicon)?

### Q6 — Bare-metal iteration-cost (the real schedule risk)
4a took **8 bare-metal iterations for one Add.** The full op set on blind silicon could balloon. Mitigations, all leaning on what 4a bought us:
- The dispatch path is settled, so a new op that *faults or mis-computes* is localized to **its kernel or its payload**, not the pipeline. The iter7 fault-address capture + the failure-mode/discriminator discipline (now a CLAUDE.md standing rule) carry forward — every 4b Stage-B run must keep them.
- **Sequence by risk:** integer gather (byte-exact, no FP surprise) → elementwise → matmul (accumulation/tolerance) → one transcendental (the hardest). Land the easy, conclusive ones first.
- Each op ships its **independent byte oracle** (kernel disasm/zeinfo + hand-laid payload) so Stage-A catches ABI errors before they cost a hardware run.

## 4. Open decisions for the PL (before dispatch)
1. **Q1:** representative coverage + CPU fallback (recommended) vs full per-op GPU coverage.
2. **Q2 (the big one):** the per-op-class FP tolerance bounds and the reference (host f32, recommended) — the number policy ANSPEC §9.5 needs.
3. **Q5:** the 4b-1/4b-2 slice vs a per-op Stage-B gate.

## 5. Recommendation in one line
Slice BUILD-4b as **AccelDriver registration + representative op kernels (matmul / elementwise / one transcendental / integer gather) on the proven stateless-A64 dispatch path, with a fixed per-op-class FP tolerance against host f32 and `accel_cpu` fallback for uncovered ops**, then a bare-metal forward-pass-equivalence + migration close — keeping BTI/DRM as documented later-build deferrals. The one decision that must be made by the PL, not assumed, is **Q2 the tolerance policy**.
