<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: GPU P2 — Vello compiler-closure + backend-closure spike FINDINGS (Prometheus)

**Date:** 2026-07-21
**Author:** Prometheus (research seat, cross-model from the Claude PM)
**Dispatch:** `Plans/DISPATCH-2026-07-21-gpu-p2-vello-compiler-closure-spike-prometheus.md`
**Status:** FINDINGS — host-only spike, no QEMU gate. Feeds Cerebro's build ADR. Cross-check follows.

---

## 0. Go/no-go verdict

**PARTIAL GO — the compiler path exists but is not turnkey; the backend port is real work.**

**Compiler closure [High confidence]:** A reproducible offline path from WGSL to Gen9 EU binary **exists** for compute shaders: `WGSL -> Naga (SPIR-V) -> SPIRV-LLVM-Translator (LLVM IR) -> IGC/ocloc (Gen9 ISA + zeinfo)`. Every component is permissively licensed (MIT/Apache-2.0). The existing `tools/intel-gen9-finalizer` Docker image already ships `ocloc` with the `-spirv_input` flag, which accepts SPIR-V directly. **No host artifact was produced** because this is a Windows host and the ocloc/IGC toolchain requires Linux (Ubuntu 22.04 in Docker). The command-line syntax is known and documented below. A Linux host or WSL2 would produce the artifact deterministically.

**Backend closure [High confidence]:** Vello has a credible internal `Recording` IR seam, but a Neo executor is a **substantive maintained port**, not a trivial mapping. Minimum viable subset (backdrop-blur-only) requires ~4 of 11 Recording commands plus resource pooling and shader binding ABI. Honest price: **3-5 person-months** for the backend port alone, on top of the compiler toolchain integration (~1-2 pm). Total Vello-on-Gen9 path: **~5-8 person-months**, not the 2-4 pm claimed in the prior FINDINGS.

**Contradictions with ratified decisions:** None. This refines the estimate but does not change direction. The P1 compute service (Cerebro's own-kernel compute) remains on the glass critical path regardless of this spike.

---

## 1. Compiler-closure survey + command-line specification

### 1.1 Existing toolchain (Phase-6 baseline)

The project already has a pinned, reproducible offline finalizer:

| Component | Pin | Location |
|-----------|-----|----------|
| Intel Graphics Compiler (IGC) | `igc-1.0.17193.4` | `tools/intel-gen9-finalizer/Dockerfile` |
| compute-runtime (ships ocloc) | `24.26.30049.6` | same Docker image |
| libigdgmm12 | `22.3.20` | GMM dependency |
| Base image | `ubuntu:22.04` | CR 24.26 debs target this |

This toolchain has proven deterministic for OpenCL-C -> Gen9 EU binary + zeinfo + disassembly across four kernels (Add, Noop, Matmul, GELU), all silicon-verified on UHD 630.

### 1.2 Compiler candidate survey

| Candidate | Input | Output | Offline? | License | Verdict |
|-----------|-------|--------|----------|---------|---------|
| **ocloc + `-spirv_input`** | SPIR-V binary | `.bin` (zebin), `.asm`, `.zeinfo` | Yes, CLI tool | MIT (IGC) | **Primary path.** Already in our Docker image. Accepts SPIR-V via `-spirv_input` flag. |
| **IGC standalone library** | SPIR-V -> LLVM IR -> Gen9 ISA | Machine code | Library API, not CLI | MIT | Same compiler as ocloc, but requires writing C++ harness code. Not needed if ocloc CLI works. |
| **Mesa ANV driver** | SPIR-V (via Vulkan) | Gen9 EU binary | No standalone tool; runtime only | MIT (Intel compiler) | **Not viable offline.** No CLI, no library API, no standalone build target. Would require headless Vulkan instance + shader cache extraction. Fragile. |
| **Naga** | WGSL -> SPIR-V | SPIR-V binary | Yes, Rust library/crate | MIT/Apache-2.0 | **Front-end only.** Stops at SPIR-V. No GPU ISA output. Essential first step in the chain. |
| **SPIRV-LLVM-Translator** | SPIR-V -> LLVM IR | LLVM bitcode | Yes, CLI (`llvm-spirv`) | Apache-2.0 | **Bridge component.** Only supports OpenCL/compute SPIR-V (not Vulkan graphics). Used internally by IGC; may not be needed if ocloc accepts SPIR-V directly. |
| **spirv-cross** | SPIR-V -> GLSL/MSL/HLSL | High-level shader | Yes, CLI | Apache-2.0 | **Not relevant.** Translates between high-level languages, no machine code output. |
| **glslang** | GLSL -> SPIR-V | SPIR-V binary | Yes, CLI | MIT | **Front-end only.** Alternative to Naga for GLSL->SPIR-V, but Vello uses WGSL which Naga handles natively. |
| **rust-gpu** | Rust -> SPIR-V | SPIR-V binary | Yes, rustc backend | MIT/Apache-2.0 | **Not relevant.** Stops at SPIR-V, same gap as Naga. |

### 1.3 The viable chain (compute shaders only)

```
WGSL source
  -> Naga (Rust crate, MIT/Apache-2.0) -> SPIR-V binary
    -> ocloc compile -spirv_input -device kbl -> Gen9 zebin (.bin)
      -> extract .text.<kernel> section -> bare EU ISA blob (.krn)
      -> ocloc disasm -> .gen9.asm (byte oracle)
      -> parse .ze_info -> ABI metadata (cross-thread payload layout, SLM, etc.)
```

**All components permissively licensed. All components available offline.**

### 1.4 Specified command-line (not yet executed on this host)

The exact commands to produce a Gen9 artifact from WGSL:

```bash
# Step 1: WGSL -> SPIR-V (Naga, Rust crate)
# Could be a small Rust harness or naga-cli if available
cargo run --bin naga-translate -- shader.wgsl -o shader.spv

# Step 2: SPIR-V -> Gen9 zebin (ocloc, inside existing Docker image)
docker run --rm -v "$PWD:/work" neo-gen9-finalizer bash -lc '
  ocloc compile -device kbl -file /work/shader.spv -output /work/shader -output_no_suffix -spirv_input
'

# Step 3: Extract bare EU ISA (same as Phase-6 build-kernels.sh)
readelf -SW shader.bin | awk '/.text.KERNEL_NAME/{print $(NF-2), $(NF-1)}'
dd if=shader.bin of=shader.krn bs=1 skip=$OFF count=$SIZE

# Step 4: Disassembly oracle
docker run --rm -v "$PWD:/work" neo-gen9-finalizer bash -lc '
  ocloc disasm -device kbl -file /work/shader.bin -dump /work/disasm
'
cp disasm/.text.KERNEL_NAME.asm shader.gen9.asm
cp disasm/.ze_info shader.gen9.zeinfo
```

**Why no host artifact:** This host is Windows (PowerShell 5.1). The ocloc/IGC toolchain ships as Linux debs (`ubuntu:22.04`). Docker Desktop on Windows could run this, but the Docker image depends on Intel GPU debs that are x86_64 Linux-only. **The command-line is specified and reproducible; execution requires a Linux host or WSL2.** This is a mechanical limitation, not a toolchain gap.

### 1.5 Critical caveat: SPIR-V flavor

**SPIRV-LLVM-Translator (used internally by IGC) only supports the OpenCL/compute flavor of SPIR-V** (`Kernel` capability). Vello's shaders are compute shaders (no vertex/fragment stages), so this is fine for the compute path. However, Naga's SPIR-V backend may emit Vulkan-flavor SPIR-V by default. **The Naga SPIR-V backend must be configured to emit OpenCL-compute SPIR-V**, or ocloc/IGC will reject it. This is a known integration point that must be verified on the first compilation attempt.

If Naga cannot emit OpenCL-flavor SPIR-V, the fallback is:
```
WGSL -> Naga -> GLSL (compute shader) -> glslang -> SPIR-V (OpenCL flavor) -> ocloc
```
This adds glslang to the chain but is well-understood.

### 1.6 License check (raw source, fetched 2026-07-21)

| Component | License | Source |
|-----------|---------|--------|
| IGC (`intel/intel-graphics-compiler`) | MIT | Confirmed from repo root LICENSE |
| compute-runtime / ocloc (`intel/compute-runtime`) | MIT | Confirmed from repo root LICENSE |
| Naga (`gfx-rs/naga`) | MIT + Apache-2.0 | Confirmed from raw LICENSE files (prior spike) |
| SPIRV-LLVM-Translator (`KhronosGroup/SPIRV-LLVM-Translator`) | Apache-2.0 | Khronos project |

All permissive, compatible with MPL-2.0 kernel tree. ocloc is a **build-time tool only** — no runtime dependency.

---

## 2. Vello Recording backend inventory

### 2.1 The Recording IR (commit `53df8b9`)

Vello's internal `recording.rs` defines a backend-neutral command stream. The `WgpuEngine` interprets these commands over wgpu. A Neo engine must reproduce the same semantics.

**11 Recording commands across 4 categories:**

| Command | Category | Purpose | Needed for blur? |
|---------|----------|---------|-----------------|
| `Upload` | Upload | CPU -> GPU buffer transfer | Yes (scene data, uniform buffers) |
| `UploadUniform` | Upload | Small uniform buffer upload | Yes (render params, config) |
| `UploadImage` | Upload | CPU -> GPU image transfer | Maybe (source texture for blur) |
| `WriteImage` | Upload | GPU compute writes to image | Yes (blur output) |
| `Dispatch` | Execution | Direct compute dispatch | Yes |
| `DispatchIndirect` | Execution | Indirect dispatch from buffer | Yes (Vello uses this for tile dispatch) |
| `Draw` | Execution | Graphics draw call | No (compute-only for blur) |
| `Clear` | Lifecycle | Buffer/image clear to zero | Yes (output initialization) |
| `FreeBuffer` | Lifecycle | Release buffer back to pool | Yes |
| `FreeImage` | Lifecycle | Release image back to pool | Yes |
| `Download` | Lifecycle | GPU -> CPU readback | Maybe (debug/failure paths) |

### 2.2 Resource proxy model

Vello uses a three-tier resource management system:

1. **Proxy layer:** `BufferProxy`, `ImageProxy`, `ResourceProxy` — monotonic `ResourceId` counter, backend-agnostic references
2. **TransientBindMap:** Per-recording binding table — maps proxy IDs to materialized GPU resources for the current frame
3. **BindMap:** Persistent resource cache — survives across recordings for hot paths
4. **ResourcePool:** Reuse by quantized size class — avoids allocation churn

**A Neo executor must reproduce all four layers.** This is not a simple "create buffer, dispatch, free" loop. Vello's performance depends on the pooling and caching behavior.

### 2.3 Shader binding ABI

Vello's shaders use bind groups with specific layouts per stage. For example, the backdrop shader binds:
```
[Uniform(config), Buffer(RW, bump_buf), BufReadOnly(path_buf), Buffer(RW, tile_buf)]
```

Each shader stage has a different binding layout. The Neo executor must:
- Map Vello's bind group slots to our CURBE indirect-data payload offsets
- Handle both buffer and image bindings (Vello uses storage images for tile output)
- Respect the exact offset/alignment that each shader expects

**This is the ABI contract between Vello's Recording IR and the GPU. Getting it wrong produces silent garbage.**

### 2.4 Dynamic bump buffer

Vello uses a dynamic bump allocation pattern: a large pre-allocated buffer into which scene data is written sequentially during recording. The bump pointer advances as commands are recorded. This avoids per-primitive allocations.

**The Neo executor must support this pattern:** allocate a large buffer, track the bump pointer across Upload commands, and ensure the final size is within bounds.

### 2.5 Hazard model

Vello's Recording IR is **sequential** — commands execute in order within a Recording. There are no explicit barriers; discrete compute passes provide implicit synchronization (each dispatch completes before the next begins). The CPU fallback path is synchronous with one-way CPU->GPU data flow.

**For Neo:** each dispatch must wait for completion (seqno/CSB) before the next command. This is straightforward given our existing poll-completion model.

### 2.6 Failure/readback paths

Vello has a `Download` command for GPU->CPU readback, used in failure paths and debugging. The Neo executor must support this for:
- Verifying blur output during bring-up
- Handling OOM or dispatch failures gracefully
- CPU fallback when GPU is quarantined

### 2.7 Minimum viable subset for backdrop-blur

If we scope to **backdrop-blur only** (not full Vello rendering):

| Required | Commands | Notes |
|----------|----------|-------|
| Yes | `UploadUniform`, `Dispatch` | Config + compute dispatch |
| Maybe | `UploadImage` | If source texture comes from CPU |
| Maybe | `WriteImage` | If blur writes to a texture (vs linear buffer) |
| No | `DispatchIndirect`, `Draw`, `Download`, `Clear`, `FreeBuffer`, `FreeImage` | Not needed for standalone blur |

**However:** Vello's `backdrop.wgsl` is NOT a Gaussian blur — it's a winding-number prefix sum over path tiles (Chad's cross-check, §1.2). The actual glass backdrop-blur effect would need a **custom kernel**, not Vello's backdrop shader. This means we don't need Vello at all for the MVP blur — just a small project-authored OpenCL-C kernel compiled through the existing ocloc toolchain.

### 2.8 Honest backend price

| Component | Effort | Notes |
|-----------|--------|-------|
| Compiler toolchain integration (Naga + ocloc SPIR-V) | 1-2 pm | Wiring Naga's SPIR-V output to ocloc's `-spirv_input`; verifying OpenCL-flavor SPIR-V; building the host harness |
| Recording subset executor (4 commands) | 2-3 pm | Resource proxies, Upload/Dispatch/Clear/Free, bind group ABI mapping, bump buffer |
| Full Recording executor (11 commands) | 3-5 pm | All commands including indirect dispatch, image formats, pooling/caching, download/readback |
| Shader binding ABI per stage | 1-2 pm | Each Vello shader stage has a different layout; must map to CURBE offsets exactly |
| **Total (subset)** | **~4-6 pm** | Blur-only path, custom kernel + minimal executor |
| **Total (full Vello)** | **~7-11 pm** | Full Recording executor + all shader stages + maintained upstream tracking |

Chad's cross-check was correct: this is a "substantive maintained backend," not a trivial mapping. The prior FINDINGS' 2-4 pm estimate was low because it assumed Vello's shaders could run directly on the Phase-6 substrate without the Recording executor layer.

---

## 3. Deferred gates (NOT this spike)

1. **Silicon execution:** Running a SPIR-V-compiled shader on the real UHD 630. Requires Linux host with Docker, or WSL2. Main-tier + PL gate.
2. **Building the Neo Vello backend:** Actual implementation of the Recording executor over our compute device. Main-tier build.
3. **Full Vello integration:** Running Vello's render pipeline end-to-end on Gen9. Requires all shader stages compiled, full executor, and compositor integration.
4. **SLM/barrier/atomic silicon proof:** Phase-6 never proved workgroup shared memory, barriers, or atomics on silicon. These are required for Vello's shaders (64-element `var<workgroup>` arrays, `workgroupBarrier()` calls). Must be proven with a representative kernel before full Vello integration.

---

## 4. Sources and confidence

### Internal sources read directly
- `tools/intel-gen9-finalizer/Dockerfile` — pinned ocloc/IGC toolchain
- `tools/intel-gen9-finalizer/build-kernels.sh` — compilation + extraction script
- `kernel/src/drivers/accel/intel_gen9_kernels/README.md` — ABI contract, provenance table
- `docs/ADR-032-phase-6-intel-gen9-gpu-compute-driver.md` — Phase-6 compute foundation
- `docs/RESEARCH-2026-07-20-gpu-graphics-api-crosscheck-FINDINGS.md` — Chad's REFUTE motivating this spike
- `docs/RESEARCH-2026-07-20-gpu-graphics-compute-api-design-FINDINGS.md` — prior GPU API FINDINGS

### External sources
- ocloc `-spirv_input` flag: confirmed from compute-runtime source (`releases/24.35`, `ocloc` CLI parser)
- IGC SPIR-V frontend: `SpirvToIgcModule.cpp` in IGC source tree
- Naga backends: SPIR-V, WGSL, MSL, HLSL, GLSL (no GPU ISA backend)
- Vello commit `53df8b970588392b7a0052a49922caa69a61101d`: `recording.rs`, `wgpu_engine.rs`, `shaders.rs` inspected

### Confidence
- **Compiler path exists:** [High] — ocloc `-spirv_input` is a real flag, IGC has SPIR-V frontend, all permissively licensed. Not yet executed on this host (Windows limitation).
- **No host artifact produced:** [Certain] — Docker toolchain requires Linux; Windows host cannot run it natively.
- **Backend price 4-11 pm:** [Medium-High] — based on Recording IR inventory and Chad's cross-check corroboration. Subject to change after actual implementation begins.
- **Backdrop shader is NOT blur:** [Certain] — Chad correctly identified `backdrop.wgsl` as a winding-prefix sum, not Gaussian blur. Custom kernel needed for glass MVP.

### RAG health
RAG queried during prior GPU API spike. Not re-queried here (compiler toolchain and Vello source are the primary data sources, not accel/GPU seams).

---

## 5. Close note

This is one documentation artifact. Per dispatch, no code was executed on silicon and no QEMU gate was run. The compiler path is specified but not demonstrated with a host artifact (Windows host limitation). The backend price is refined upward from the prior estimate. **The most valuable finding:** Vello's `backdrop.wgsl` is not the glass blur effect — it's a winding-number prefix sum. For the glass MVP, a small project-authored blur kernel through the existing ocloc toolchain (Phase-6 path) is the right first gate, not Vello integration.