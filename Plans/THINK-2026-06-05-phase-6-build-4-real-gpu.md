<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 6 BUILD-4 — the real GPU (design pass)

**Date:** 2026-06-05
**From:** PM/Cerebro
**Status:** THINK (design investigation — no code). Feeds a substrate ADR + PLAN once target + slice are ratified.
**References:** ANSPEC-001 §9.2 ("one real GPU… cleanest open documentation… likely Intel or AMD; NVIDIA lower priority"), §9.5 goal milestone; ADR-030 (Phase 6 substrate), ADR-023 (Linux-personality DRM-expressibility forward-marker), ADR-025 (typed MMIO/PIO wrappers — "new drivers first, Phase 6 GPU"); bible Articles 2.2/2.3 (kernel-managed accelerators), 6.4, 7.5.

---

## 0. Why this needs a THINK pass

BUILD-4 is unlike BUILD-1–3 in three structural ways, and each changes how we plan it:
1. **It needs real hardware** — QEMU has no emulated GPU compute device and the CI container has no GPU/passthrough, so **BUILD-4 cannot be CI-tested.** Its gate is bare-metal evidence, not a green scenario.
2. **The GPU choice is load-bearing** (driver tractability varies enormously by vendor), and ANSPEC §9.2 ties it to *open documentation*.
3. **It's the real-hardware payoff** the whole synthetic ladder (accel_cpu → accel_sim → virtio-accel) was de-risking — so the *transport/abstraction* is proven; what's new is vendor-specific GPU bring-up.

## 1. Findings (grounded)

### F1 — The available hardware is an Intel Gen9.5 iGPU
The project's bare-metal target is a **Dell OptiPlex 7080 Micro** (the box used for Phase 3 AHCI and Phase 4 net bare-metal evidence). That's a Comet Lake machine whose integrated GPU is **Intel UHD Graphics 630 — Gen9.5 (Gen9 LP)**. *We already have the target GPU in hand* — this resolves the "is the hardware available?" question. (Confirm the exact PCI device-id by enumeration at BUILD; some F-series CPU configs disable the iGPU — verify the specific unit has it.)

### F2 — Intel Gen9 is also the cleanest-documented, and an iGPU is the simplest memory model
- Intel publishes the **open-source Programmer's Reference Manuals** (the x.org-hosted PRM set) and **"The Compute Architecture of Intel Processor Graphics Gen9"** (full EU/threading/ISA detail), plus the **open EU/vISA ISA** via `intel-graphics-compiler`. This is the best public GPU documentation that exists.
- As an **iGPU**, tensor buffers live in **system RAM mapped through the GTT** (graphics translation table) — *no discrete VRAM / PCIe-BAR device-memory allocator* to build (the hard part of a discrete-GPU driver). Gen9 shares the LLC, so there are coherent CPU↔GPU paths.
- **Gen9 supports legacy ring submission without GuC firmware** for basic execution — so a first compute path can avoid the GuC/HuC firmware-load dependency that later gens force.

So: **the hardware we have and the best-documented, most-from-scratch-tractable GPU are the same chip.** That is a rare and decisive alignment.

### F3 — AMD and NVIDIA are worse fits here
- **AMD:** open ISA (GPUOpen machine-readable ISA) but a **heavy required-firmware story** — multiple microcode blobs (SMC/CP/RLC/MC/SDMA, MES on RDNA3, PSP on modern parts) loaded at boot, plus complex PSP/SMU init. Much larger bring-up, and **we have no AMD card in the loop.**
- **NVIDIA:** opaque, signed firmware; deprioritized by §9.2. Correct to skip.

### F4 — No CI; the gate is bare-metal evidence + a tolerance oracle
BUILD-4 validation runs on the OptiPlex via the existing **`--bare-metal-mode`** evidence flow (serial / block-device log sink, the forensic capture used for AHCI/net). The CI suite stays green because the GPU driver is **feature-gated, default-off** (CI never compiles/loads it) — same pattern as `virtio-accel`.

### F5 — Equivalence is within-tolerance, NOT byte-exact
`accel_cpu` computes in deterministic **soft-float**; the GPU uses **hardware FP**. For transcendentals these differ in the low bits, so BUILD-4's oracle is **`real_gpu ≈ accel_cpu` within an f32 tolerance** (§9.5 already says "within numerical tolerance for non-bit-exact dtypes"), not the byte-exact equality used for accel_sim/virtio-accel. (Exact-integer ops like Embedding gather can still be byte-exact.)

## 2. Recommendation

**Target: the Intel Gen9.5 iGPU (UHD 630) on the OptiPlex 7080.** Driver name candidate `accel_intel` (or `accel_gen9`). Slice BUILD-4 **bring-up-first**, exactly as BUILD-3 was sliced transport-first — the first contact with real GPU silicon should prove the *pipeline*, not the op set:

- **BUILD-4a — minimal compute bring-up (one EU kernel).** PCI-probe the iGPU, map its MMIO/GTT, map a system-RAM input+output buffer through the GTT, upload one tiny compute kernel (an elementwise `Add`), submit it on the render/compute ring, detect completion, read back, and assert the result **≈ the accel_cpu oracle within tolerance** — on bare metal, captured. This proves the whole GPU compute path end-to-end with the smallest possible surface.
- **BUILD-4b — full op set + driver integration.** Author the remaining ops as EU kernels, register `accel_intel` as a full `AccelDriver` on `Device::Accel(n)`, prove forward-pass equivalence (within tolerance) + tensor migration to GPU (GTT) memory, and land the DRM-expressibility (ADR-023) + ANSPEC §9 amendments.

Given the size and uncertainty (see §4), **expect BUILD-4 to expand into a multi-build sub-arc**, not a single dispatch.

## 3. The minimal-compute path (BUILD-4a), concretely

1. **Probe + MMIO:** find the iGPU (vendor `0x8086`, UHD 630 device-id) on PCI; map GTTMMADR (registers + GTT) and the aperture via the typed MMIO wrappers (ADR-025).
2. **GTT setup:** program GTT entries to map a kernel-owned system-RAM region (the compute kernel binary + input/output tensor buffers) into GPU graphics address space.
3. **Compute kernel:** a tiny Gen9 EU program for `Add`. **We do not hand-assemble Gen ISA** — compile it *offline* with the open `intel-graphics-compiler`/vISA finalizer and embed the resulting Gen9 binary as a `const` blob (a new *offline build-time* toolchain dependency — see §4).
4. **Command submission:** build a minimal GPGPU pipeline (`MEDIA_VFE_STATE` / `MEDIA_INTERFACE_DESCRIPTOR_LOAD` / `GPGPU_WALKER` on Gen9) in a ring buffer; submit via legacy ring (no GuC).
5. **Completion:** signal via a pipe-control store + seqno in memory; poll or take the GPU interrupt.
6. **Readback + oracle:** compare the GPU output to `accel_cpu` within tolerance; emit the result on the bare-metal evidence channel.

## 4. Risks & unknowns (this is the longest pole in Phase 6)

1. **Framebuffer/console conflict (sharp).** The iGPU *is* driving the Limine framebuffer console — which is also part of our evidence channel. Doing compute on the same engine risks disturbing or blanking the display mid-run. Mitigation: the block-device log sink (already used for bare-metal) is the resilient evidence path; treat the framebuffer as expendable during a GPU run. **Must be designed for, not discovered.**
2. **No CI / single physical machine.** All validation is manual, serialized on one OptiPlex, slow to iterate (cf. the careful scheduled bare-metal handoffs in Phase 3). Regression-guarding is weak — the CI suite can't protect this driver.
3. **EU-kernel toolchain.** Producing Gen9 compute binaries needs an offline compiler (`intel-graphics-compiler`/IGA/vISA). New build-time dependency + reproducibility/pinning concern; the embedded kernel blobs need their own provenance.
4. **Firmware.** Gen9 *should* allow ring submission without GuC — confirm early. If GuC turns out necessary, that's a firmware blob (Article 4.1 acknowledges accelerator firmware as a reality, but it's a dependency to surface, not bury).
5. **GTT / cache coherency / fences.** Getting CPU↔GPU memory visibility and completion ordering right on Gen9 (LLC-coherent paths, pipe-control flushes) is fiddly and a classic source of "works sometimes" bugs.
6. **Tolerance definition.** §F5 — we must pick and justify the f32 tolerance per op (and keep exact-integer ops exact), or risk a meaningless oracle.

## 5. Governance impact
- **A substrate ADR** for the Intel driver (the Gen9 bring-up decisions: ring vs GuC, GTT model, the EU-kernel toolchain, tolerance policy).
- **ADR-023 DRM-expressibility:** BUILD-4b is where the real-hardware driver must demonstrate it stays *expressible* as a Linux-DRM-compatible char device for the future personality (interface constraint).
- **ANSPEC-001 §9 amendment** for the real-driver surface (§9.2 "second deliverable"/now BUILD-4, and any §9.1 trait reality the hardware forces).

## 6. Open questions for the ADR/PLAN
1. Confirm the OptiPlex unit's exact iGPU device-id (and that the iGPU is enabled).
2. Compute-only, or must we cooperate with / tear down the Limine framebuffer first?
3. EU-kernel toolchain: which compiler, pinned how, blobs stored where (provenance)?
4. Ring vs execlist submission on Gen9.5; GuC required or not (confirm).
5. Tolerance policy per op (and which ops stay byte-exact).
6. Does BUILD-4 formally become a sub-arc (4a/4b/4c…)? Likely yes.

## 7. Scope guard
THINK only — **no code.** Next gate: PL ratifies the target (Intel Gen9.5 on the OptiPlex) and the bring-up-first slice; then a substrate ADR + the BUILD-4a PLAN. This is the most hardware-dependent, least-CI-protected build in the project to date — plan it as such.

---

**Sources (F2/F3):**
- Intel Open Source HD Graphics PRMs: https://www.intel.com/content/www/us/en/docs/graphics-for-linux/developer-reference/1-0/overview.html
- The Compute Architecture of Intel Processor Graphics Gen9: https://www.intel.com/content/dam/develop/external/us/en/documents/the-compute-architecture-of-intel-processor-graphics-gen9-v1d0.pdf
- Intel Graphics Compiler / vISA ISA: https://github.com/intel/intel-graphics-compiler
- AMD machine-readable GPU ISA: https://gpuopen.com/machine-readable-isa/
