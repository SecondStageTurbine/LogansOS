<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: GPU graphics/compute API design — FINDINGS (Prometheus)

**Date:** 2026-07-20
**Author:** Prometheus (research seat, cross-model from the Claude PM)
**Dispatch:** `Plans/DISPATCH-2026-07-20-gpu-graphics-compute-api-design-research-prometheus.md`
**Status:** FINDINGS — doc-only, no gate. Feeds Cerebro's build ADR. Cross-check follows.

---

## 0. Executive recommendation

**Minimal-compute-first, not full wgpu-class.** Build a minimal Gen9 compute device that serves Vello's backdrop-blur now (glass), with a clean extension path to the WebRender GL-profile adapter later (browser). The API is the layer above the Phase-6 foundation — it does not start from scratch.

**Why minimal-compute-first:**

| Criterion | Minimal-compute-first | Full wgpu-class device |
|-----------|----------------------|----------------------|
| **First deliverable** | Glass backdrop-blur (Vello compute pass) | Nothing usable until full graphics stack complete |
| **Builds on Phase-6** | Directly: execlist, PPGTT, ring, EU dispatch already proven | Requires new 3D pipeline, textures, samplers, render targets on top |
| **Effort to first pixels** | ~2-4 person-months (compute device + Vello compute backend) | ~9-14 person-months (full wgpu HAL + all GFX slices through GFX-6 per Janus) |
| **Risk** | Low: Gen9 EU dispatch proven on silicon (Add, Matmul, GELU). Compute shaders are the same hardware path. | High: Gen9 3D pipeline unproven; requires offline shader compiler, GL object semantics, surface state, MOCS, blend/scissor |
| **Extension to graphics** | Clean: add render targets, textures, samplers, 3D pipeline as GFX-5 equivalent on same resource/queue/fence model | Already there but takes longer to deliver anything useful |
| **I-14 preservation** | Same containment model (PPGTT + VT-d) applies equally | Same containment model, more surface area for faults during bring-up |

**The extension path:** The minimal compute device establishes the cap-native resource model (`GpuBufferCap`, `GpuQueueCap`, `GpuFenceCap`, per-context PPGTT), command validation, and submission/fence infrastructure. Adding graphics (textures, render targets, samplers, 3D pipeline) is a vertical extension on the same substrate — not a rewrite. Janus's GFX-1 through GFX-6 ladder confirms this: GFX-3 (fixed copy/clear) builds directly on GFX-1/GFX-2's resource/queue/fence model, and GFX-5 (WebRender GL) is the large slice that adds 3D state on top.

**License verdicts (raw source, fetched 2026-07-20):**
| Component | License | Adoptable? |
|-----------|---------|------------|
| Vello | MIT + Apache-2.0 | **Yes** |
| wgpu | MIT + Apache-2.0 | **Yes** |
| Naga (WGSL validator/lowerer) | MIT + Apache-2.0 | **Yes** |

These are permissive and compatible with our MPL-2.0 tree. However, we're not adopting wgpu as a whole — we're building a minimal compute device that Vello can target directly. wgpu adoption is the later path for WebGPU/WebGL support (GFX-7 per Janus).

---

## 1. Phase-6 Gen9 compute foundation inventory

### 1.1 What's in-tree and silicon-proven

The `kernel/src/drivers/accel/` tree contains a comprehensive, silicon-proven Gen9 compute stack:

| Component | File | Status | Silicon-proven? |
|-----------|------|--------|----------------|
| **AccelDriver trait** | `mod.rs` | Complete registry, dispatch, poll, wait, alloc/free, copy to/from device | Yes (CPU, sim, Intel backends) |
| **PCI probe + forcewake** | `accel_intel.rs` | BAR0/MMIO mapping, GTTMMADR 16M GGTT, render/GT forcewake with ACK polling | Yes |
| **GGTT programming** | `accel_intel.rs` | PTE present+LLC mapping, ring/state/kernel/data/seqno/CURBE at fixed GGTT bases | Yes (BUILD-4a) |
| **Legacy ring submission** | `accel_intel.rs` | Ring start/tail/control, MI_NOOP smoke test, PIPE_CONTROL seqno, poll completion | Yes (iter1-3) |
| **EU compute dispatch** | `accel_intel.rs`, `intel_gen9_4a.rs` | GPGPU_WALKER + INTERFACE_DESCRIPTOR_DATA + VFE_STATE + batch, indirect-data payload (NEO convention), local_id generation | Yes (iter8: `[1,2]+[3,4]=[4,6]`) |
| **Op dispatcher** | `accel_intel.rs` | Kernel-table dispatch: Add, Matmul (exact + non-exact with tolerance), GELU (hardware tanh, pointwise) | Yes (BUILD-4b-1/4b-2) |
| **CPU fallback** | `accel_intel.rs`, `accel_cpu.rs` | Unsupported ops route to `accel_cpu` transparently; mixed GPU/CPU forward pass within accumulated tolerance | Yes (BUILD-4b-2) |
| **Tensor migration** | `accel_intel.rs` | `copy_to_device`/`copy_from_device` proven real non-identity transfers; cpu->gpu->cpu round-trip byte-exact | Yes (BUILD-4b-2) |
| **Execlist/LRC submission** | `intel_gen9_execlist.rs`, `accel_intel.rs` | GuC-free ELSP submission, CSB poll completion, 22-page LRC context image with PPGTT root | Yes (BUILD-4X-1: no-op retired) |
| **Per-context PPGTT** | `intel_gen9_ppgtt.rs`, `accel_intel.rs` | 48-bit 4-level page tables (PML4->PDP->PD->PT), leaf/dir entry construction, byte oracles | Yes (BUILD-4X-2: data resolved via PPGTT) |
| **Cross-tenant isolation** | `accel_intel.rs` | Cross-context negative test: B's access to A's tensor faults; RO enforcement on write | Yes (BUILD-4X-3) |
| **Fault decode** | `accel_intel.rs` | RING_FAULT capture, fault VA reconstruction, GTT/PPGTT select bit | Yes (iter7/4X-3) |

### 1.2 What Phase-6 proved on silicon (the self-proving gates)

The Phase-6/7 arc ran on real Dell OptiPlex 7080 UHD 630 (Gen9.5, PCI `00:02.0`, device ID `0x9bc8`). Key results:

1. **BUILD-4a (iter8):** `[1,2]+[3,4]=[4,6]` on EU with NEO indirect-data dispatch, GGTT-resolved. `RING_FAULT=0`, `INSTDONE=0xfffffffe`.
2. **BUILD-4b-1:** Matmul exact (`[19,22;43,50]`) and non-exact (within tolerance) on EU. Oversize matmul routes to CPU fallback.
3. **BUILD-4b-2:** GELU on 8 spread inputs vs host-f32: `max_rel_e24=1` (non-vacuous). Mixed forward pass (MatMul+Gelu GPU, LayerNorm/RmsNorm/Softmax CPU) within accumulated bound. Tensor migration cpu->gpu->cpu byte-exact.
4. **BUILD-4X-1:** GuC-free execlist context switches in and retires via ELSP/CSB. PPGTT engaged (opposite of legacy ring's `select=GGTT`).
5. **BUILD-4X-2:** Tensor data resolved through per-context PPGTT only (buffers cleared from GGTT). `[4,6]` = self-proving PPGTT engagement.
6. **BUILD-4X-3:** Cross-tenant negative: B's read of A's VA faults (`RING_FAULT=0x1e1`, `select=PPGTT`, A's VA, B's root active). RO-read succeeds, RO-write blocks (Gen9 finding: permission fault does not surface through `RING_FAULT` — EU wedges, proof is unchanged output frame).

### 1.3 What Phase-6 does NOT provide (the gap this API fills)

The existing driver is an AI compute accelerator, not a graphics/compute device:
- No typed buffers/textures/render targets
- No shader compilation path beyond pre-compiled EU kernels
- No GL context or wgpu HAL backend
- No command encoder/queue abstraction for userland
- No per-context resource namespace or quota
- No hang/reset/quarantine beyond seqno timeout
- No graphics IRQ completion (poll-only)
- No VT-d domain (identity-mapped GPU)
- No display/modeset/page-flip (OT-63 present backend is synthetic vblank, Slice 4 pending real Gen9 flip)

**The API we design sits above the proven substrate and fills these gaps for glass first.**

---

## 2. Vello's minimal compute-backend requirements

### 2.1 How Vello renders

Vello (linebender/vello) is a GPU-first 2D renderer that uses **compute shaders**, not fixed-function rasterization. It renders a scene by:
1. Building a scene graph in CPU memory
2. Recording commands into a `wgpu::CommandEncoder`
3. Dispatching compute shaders that tile the scene and render primitives (rectangles, paths, text, images) to a render target
4. The backdrop-blur effect is a **compute pass** that reads from a source texture, applies a Gaussian blur kernel in two passes (horizontal then vertical), and writes to a destination texture

Vello's `Renderer` trait abstracts over the GPU backend through wgpu. It requires:
- A `wgpu::Device` and `wgpu::Queue`
- Shader modules (WGSL) compiled at device creation time
- Bind groups binding buffers/textures to shader stages
- Compute pipeline creation from shader modules
- Command encoding: bind group sets, compute passes, dispatch calls
- Texture creation with formats (Bgra8Unorm for targets)
- Buffer creation for uniform data (render params, scene commands)

### 2.2 Minimal device contract for Vello's blur

To run Vello's backdrop-blur as a GPU compute pass on Gen9, we need:

| Requirement | What it means | Phase-6 equivalent |
|------------|---------------|-------------------|
| **Compute pipeline** | Shader module + bind group layout -> hardware dispatch state | EU kernel blob + CURBE payload + GPGPU_WALKER (already proven) |
| **Storage buffers** | GPU-readable/writable buffers for scene data, render params | GGTT/PPGTT-mapped DmaBuffers (already proven) |
| **Storage textures** | GPU-readable/writable 2D surfaces for source/destination pixels | Not yet: need surface state or stateless A64 with 2D tiling |
| **Dispatch** | `dispatch_workgroups(x, y, z)` -> EU thread launch | GPGPU_WALKER execution masks + work-size (already proven) |
| **Bind groups** | Group buffers/textures for a shader invocation | CURBE indirect-data payload (already proven) |
| **Fence/completion** | Wait for dispatch to complete before reading output | PIPE_CONTROL seqno + poll/CSB completion (already proven) |
| **Synchronization** | Order multiple dispatches (e.g., blur H then blur V passes) | Sequential dispatch with seqno wait between them |

### 2.3 The delta: storage textures are the missing piece

The existing driver uses **stateless A64** pointers for buffer access (proven by BUILD-4a). For backdrop-blur, we need to read/write 2D pixel data. Two paths:

1. **Stateless A64 with linear textures (minimal):** Treat the texture as a linear buffer with known stride. The compute shader calculates 2D coordinates from 1D offsets. This is what Vello does internally — it doesn't use sampler-based texture reads for its own render targets, just raw memory access with known layout. **This works with the existing stateless A64 path.**

2. **Surface state (proper):** Gen9 surface state objects provide 2D tiling, format conversion, and sampling. Required for sampled textures (images, fonts), but not strictly necessary for render targets that are linear memory.

**For the MVP (backdrop-blur only):** Path 1 suffices. The blur shader reads/writes to linear buffers representing pixel rows. No surface state needed. This is a **zero-hardware-delta** over the proven BUILD-4a substrate — just different kernel code and payload layout.

For later (WebRender, image sampling): Path 2 adds surface state objects. This is GFX-5 territory per Janus.

### 2.4 Shader compilation path

Vello ships WGSL shaders. Gen9 needs EU machine code (the `.gen9.asm` format). Compilation options:

| Option | Effort | Risk | Notes |
|--------|--------|------|-------|
| **Mesa's ANV compiler (build-time)** | Medium | Low | Mesa's Vulkan driver compiles GLSL->Gen9 ISA. GPL for kernel use, but build-time tool is acceptable if we only use it offline to pre-compile shaders. Output is pinned with provenance. |
| **Intel IGC/ocloc (build-time)** | Medium | Low | Already used in Phase-6 for AI kernels. Pinned ocloc toolchain compiles OpenCL->Gen9 ISA. Same provenance model. Vello's WGSL would need to be translated to OpenCL first, or we use Naga to lower WGSL->SPIR-V then hand-lower SPIR-V->Gen9. |
| **Naga + custom lowering** | High | Medium | Naga (MIT) validates and lowers WGSL to an IR. We'd need a Gen9 backend for the IR. This is significant work but gives us runtime compilation later. |
| **Pre-compile Vello's shaders once, pin output** | Low | Low | Compile Vello's fixed shader set at build time using Mesa/ocloc, pin the `.gen9.asm` output with provenance (source hash, compiler version, output hash). This is what Phase-6 does for AI kernels. **Recommended for MVP.** |

**Recommendation:** Pre-compile Vello's backdrop-blur shaders at build time using the existing ocloc toolchain (or Mesa ANV if ocloc can't handle the shader features). Pin the output with provenance. No runtime compiler needed for MVP. Later, when we need arbitrary shader compilation (WebGL/WebGPU), we invest in Naga + a Gen9 backend or a sandboxed Mesa compiler service.

---

## 3. The minimal Gen9 compute device API

### 3.1 Cap-native resource model

Building on `GpuSubmitCap` (OT-63 present gate) and Janus's capability design:

| Capability | Authority | Holder |
|-----------|-----------|--------|
| `GpuDeviceRoot` | Enumerate profiles, create contexts/queues, quarantine/reset | Kernel + trusted graphics service only |
| `GpuContextCap` | One PPGTT, reset epoch, resource namespace and budget | Compositor or disposable paint service |
| `GpuBufferCap` | A typed buffer with MAP/READ/WRITE/IMPORT rights | Derived to the minimum consumer |
| `GpuQueueCap` | Submit compute commands within byte/time/resource quotas | Compositor or glass-blur service; never script/layout clients |
| `GpuFenceCap` | Await/query one generation-tagged completion; no submit right | Any party needing completion feedback |

This preserves I-14: a GPU shader fault cannot reach governance/tensor memory because the PPGTT isolates contexts and VT-d confines DMA to graphics-owned pages.

### 3.2 Minimal compute device interface

```
GpuComputeDevice
  create_context() -> GpuContextCap        // alloc LRC + PPGTT root
  create_buffer(size, usage) -> GpuBufferCap // GGTT/PPGTT-mapped DmaBuffer
  create_pipeline(shader_hash) -> PipelineId // look up pre-compiled kernel
  create_queue(context, profile) -> GpuQueueCap // bind context + command profile

GpuComputeQueue
  submit(dispatch { pipeline, bind_group, workgroups }) -> GpuFenceCap
  wait(fence, timeout) -> Result<(), Timeout>

GpuBindGroup
  add_buffer(slot, buffer, offset, size, access) // maps into CURBE indirect-data
  add_texture(slot, texture, format, width, height, stride, access)
```

The kernel validates every submission: handles are live and generation-correct, ranges are within buffer descriptors, shader hash is in the manifest, GPU VAs are kernel-selected inside context PPGTT. No raw batch submission from userland.

### 3.3 How this maps to Phase-6 hardware

| API call | Hardware operation | Proven? |
|----------|-------------------|---------|
| `create_context()` | Allocate LRC (22 pages) + PML4 root, map in GGTT | Yes (BUILD-4X-1) |
| `create_buffer()` | Allocate DmaBuffer, map in context PPGTT | Yes (BUILD-4X-2) |
| `create_pipeline()` | Look up pre-compiled EU kernel blob + CURBE layout | Yes (BUILD-4a/4b) |
| `create_queue()` | Bind context ID to ELSP submission path | Yes (BUILD-4X-1) |
| `submit(dispatch)` | Build GPGPU_WALKER + IDD + VFE_STATE + indirect-data payload, submit via ELSP | Yes (BUILD-4a/4b/4X) |
| `wait(fence)` | Poll CSB for ACTIVE_IDLE or PIPE_CONTROL seqno | Yes (all builds) |

**The minimal compute device is almost entirely proven hardware.** The delta is the cap-native resource management, validation, and userland ABI — not new hardware paths.

---

## 4. Gen9 hardware surface + shader path

### 4.1 Compute engine (EUs)

Gen9's compute runs on the Render Command Streamer (RCS) engine, which dispatches to Execution Units (EUs). UHD 630 has 24 EUs in one slice, one subslice per EU, 8 threads per EU = 192 concurrent threads. Each EU has a SIMD32 width (32 lanes per thread).

The compute path:
1. Command streamer fetches MI commands from ring buffer (GGTT)
2. GPGPU_WALKER tells the EU how many threads to dispatch, execution masks for live lanes
3. INTERFACE_DESCRIPTOR_DATA tells the dispatcher how many GRFs to load for cross-thread and per-thread data
4. VFE_STATE sets URB entry size and count
5. EU threads execute the kernel, reading/writing through stateless A64 pointers (resolved via PPGTT under execlist)
6. PIPE_CONTROL with `Flush = 1` + seqno write signals completion

This is exactly what Phase-6 proved. No new hardware path needed for compute shaders.

### 4.2 Shader compilation (principle, no GPL lift)

**Mesa/i915 are GPL — learn the Gen9 compute principle, never copy into our MPL tree.** The shader compilation chain:

```
WGSL/CL source -> Naga/SPIR-V -> Mesa ANV / ocloc -> Gen9 ISA (.gen9.asm) -> pinned blob
```

For MVP: pre-compile at build time, pin with provenance (source hash, compiler version/options, output hash). Runtime accepts only canonical hashes from the manifest. This is what Phase-6 does for AI kernels and works identically for Vello's fixed shader set.

**Key Gen9 ISA constraints:**
- Stateless A64 dword gathering/scattering for buffer access (proven by BUILD-4a)
- GRF layout: r0 header, r1-r6 per-thread local_id, r7+ cross-thread data (NEO convention, proven by iter8)
- SIMD32 width, 128 GRF per thread
- Hardware extended-math unit for transcendental functions (`math.exp`, `math.inv`, etc.) — proven by GELU

### 4.3 Surface state (for later graphics)

When we add textures/render targets for WebRender:
- Gen9 surface state objects encode 2D tiling (Y/Yf tiling for cache efficiency), format, dimensions
- MOCS (Memory Optimization Control Structure) programming required for dataport caching correctness (flagged in BUILD-4X-1 spec, needed for compute with textures)
- Sampler state for filtered/mipped texture reads

This is GFX-5 territory. Not needed for MVP backdrop-blur with linear memory access.

---

## 5. Cap-native/I-14 device model

### 5.1 I-14(a) — lock/IRQ isolation

GPU interrupt/fault paths acquire **no AI governance lock**. The current `IrqSafeMutex<IntelGpuInner>` must be split before adding graphics IRQs: process-context submission/configuration uses a leaf `SwitchMutex`; the hard-IRQ path reads/clears bounded device status, writes completion records, updates atomics, and schedules deferred work. No forcewake poll, seqno wait, reset, buffer free, or PPGTT teardown runs under IF-off GPU lock.

### 5.2 I-14(b) — liveness degradation

Every poll/wait has a deadline. A hang marks the guilty context, stops new submit, fails fences, attempts bounded engine reset, and quarantines hardware acceleration if reset doesn't prove clean. The compositor switches to `CpuRasterBackend`. Governance scheduling, capability revocation, filesystems do not await GPU recovery. Queue quotas prevent an untrusted paint service from turning legal work into indefinite denial of display.

### 5.3 I-14(c) — memory isolation

Two translation boundaries, two threats:
```
process handles/rights
  -> per-context PPGTT: context A cannot address context B (proven by BUILD-4X-3)
    -> VT-d graphics domain: the iGPU cannot DMA outside graphics-owned physical pages
      -> system RAM
```

Currently the GPU is identity-domained. The minimal compute device must at minimum establish per-context PPGTT isolation (proven). VT-d graphics domain is GFX-2 per Janus — a later slice that generalizes `dma_arena` to named/tagged graphics extents.

### 5.4 Capability containment

No ordinary Atrium app needs a GPU queue. It receives the G1 shared pixel buffer, draws in software, and commits through its SurfaceCap channel. Only the compositor and purpose-built render services receive queue authority. Browser script/layout processes hold no GPU queue, MMIO, scanout, or physical-memory authority.

---

## 6. Build ladder to glass-blur MVP

### Compute-0 — software floor (first composited pixels)
Land the CPU `RasterBackend`/present seam over G1's shared buffer and `fb::blit_rgba`. Full-screen and damage-rect composition, real present events. No GPU probe, no DMA, no new Intel code. **Unblocks:** Atrium composited first pixels. Estimate: 2-3 person-months. (Janus GFX-0)

### Compute-1 — one Intel GT owner
Extract MMIO/forcewake/GGTT/LRC/PPGTT/CSB/fault/seqno primitives under `IntelGt`. Existing `AccelDriver` delegates through it; emitted bytes and Phase-6/7 behavior remain identical. Add deterministic arbitration skeleton, but no graphics command. **Gate:** every current Intel host oracle plus exact command/PTE hashes. Estimate: 2-4 person-months. (Janus GFX-1)

### Compute-2 — minimal compute device + Vello compute backend
Build the cap-native resource model (`GpuContextCap`, `GpuBufferCap`, `GpuQueueCap`, `GpuFenceCap`), command validation, and submission over the proven execlist/PPGTT substrate. Pre-compile Vello's backdrop-blur shaders. Implement a minimal Vello compute backend that targets our device (bypassing wgpu — direct mapping of Vello's render commands to our dispatch API). **Unblocks:** GPU-accelerated glass backdrop-blur. Estimate: 2-4 person-months.

### Compute-3 — graphics DMA/VT-d domain
Generalize named DMA arenas/witnesses; allocate graphics buffers only from `Graphics` arena. Inventory IGD DMAR scope, stolen/reserved memory and active Limine scanout. Build the GPU-specific VT-d domain. **Unblocks:** full I-14(c) enforcement for GPU. Estimate: 3-5 person-months. (Janus GFX-2)

### Compute-4 — IRQ completion, hang/reset/quarantine
Add bounded GPU interrupt capture and deferred completion/wakeup. Add generation/epoch fences, hangcheck, guilty-context accounting, reset/rebuild and permanent quarantine fallback. **Mandatory before any hostile browser content reaches an accelerated path.** Estimate: 2-4 person-months. (Janus GFX-4)

### Compute-5 — Gen9 3D/WebRender-profile GL offscreen
Inventory the pinned Servo/WebRender GL calls and shader variants. Land PRM-derived Gen9 3D state, textures/samplers/render targets/blend/scissor/draw for that profile. Implement `libneogl` and `surfman-neo`; render a WebRender scene offscreen to `GpuBufferCap`. **Unblocks:** accelerated WebRender raster. Estimate: 7-12 person-months. (Janus GFX-5)

### Compute-6 — GPU composite/present
Let the trusted compositor target/import the current boot framebuffer through `ScanoutTargetCap` or perform a GPU blit into it. Add double-buffered shadow surfaces, damage, frame pacing and honest RenderDone/CompositeDone/Presented events. **Unblocks:** credible accelerated Atrium and Servo page paint on existing mode. Estimate: 3-5 person-months. (Janus GFX-6)

### Honest critical path
- **Glass backdrop-blur MVP:** Compute-0 + Compute-1 + Compute-2 = ~6-11 person-months
- **Desktop first hardware acceleration:** Compute-3; credible smooth presentation: Compute-6
- **Servo accelerated page rendering:** Compute-1 through Compute-6 = ~20-33 person-months (per Janus)

---

## 7. Contradiction check

| Decision | Status | Notes |
|----------|--------|-------|
| **Own G1 compositor** | Compatible | Vello is the compositor/chrome renderer; WebRender is the browser page renderer. Both target the same GPU substrate but different adapters. |
| **Vello for glass** | Compatible | Vello's compute shaders run on Gen9 EU via stateless A64 — no new hardware path needed. |
| **kTLS** | Compatible | Transport machinery, unrelated to GPU path. |
| **Rust/no_std posture** | **Tension (resolvable)** | Vello itself is `no_std`-compatible with `alloc`. Our compute device lives in the kernel (`no_std`). Userland Vello backend runs in hosted userland or can be adapted. The shader compilation is a build-time tool, not runtime. |
| **Capability model** | Compatible | `GpuSubmitCap` extends naturally to `GpuQueueCap`/`GpuContextCap`. Cap-bounded GPU access fits the existing model. |
| **Janus GFX design** | Compatible | Our minimal-compute-first is Compute-0 through Compute-2, which maps directly to Janus's GFX-0/GFX-1 + a subset of GFX-3 (compute-only, no copy/clear yet). The ladder is consistent. |

---

## 8. Sources and confidence

### Internal sources read directly
- `kernel/src/drivers/accel/{mod.rs, accel_intel.rs, intel_gen9_execlist.rs, intel_gen9_ppgtt.rs, intel_gen9_4a.rs}`
- `docs/ADR-032-phase-6-intel-gen9-gpu-compute-driver.md`
- `Plans/RESEARCH-2026-06-08-gen9-execlist-lrc-bringup.md`
- `kernel/src/atrium/present_backend.rs` (GpuSubmitCap, present-fence model)
- `docs/THINK-2026-07-12-gpu-accel-path-design-janus.md` (comprehensive GPU architecture design)
- `docs/RESEARCH-2026-07-20-ot61-browser-crosscheck-FINDINGS.md` (Chad's cross-check: WebRender GL gap, why GPU API is shared unlock)

### External sources
- Vello source (linebender/vello): MIT + Apache-2.0, confirmed from raw LICENSE files
- wgpu source (gfx-rs/wgpu): MIT + Apache-2.0, confirmed from raw LICENSE files
- Naga source (gfx-rs/naga): MIT + Apache-2.0, confirmed from raw LICENSE files
- Intel Kaby Lake PRM Vol 2a/2c/5: Gen9 compute dispatch, PPGTT, EU ISA (principle only)
- Mesa/i915: GPL — principle only, no code lift

### Confidence
- **Phase-6 inventory:** [Certain] — all silicon-proven on real UHD 630 hardware with captured evidence
- **Vello compute requirements:** [High] — based on Vello's architecture and wgpu abstraction; storage textures via stateless A64 is the key insight
- **Minimal-compute-first recommendation:** [High] — builds directly on proven substrate, delivers glass first, clean extension path to graphics
- **Shader compilation path:** [Medium-High] — pre-compile + pin is proven by Phase-6; runtime compiler deferred
- **Build ladder estimates:** [Medium] — design estimates, subject to GFX-1/GFX-5 closure inventories

### RAG health
RAG queried successfully. Results used for discovery only; cited internal files read directly. Queries: `intel gen9 gpu compute execlist ppgtt accel`.

---

## 9. Close note

This is one documentation artifact. Per dispatch, no code, QEMU or CI run exists. The recommendation is **minimal-compute-first**: build the cap-native compute device on top of Phase-6's proven substrate, serve Vello's backdrop-blur for glass first, and extend vertically to graphics later. This avoids the 9-14 month wait for a full wgpu-class device while establishing the same resource/queue/fence model that graphics will need.

The browser cross-check (Chad) confirmed that WebRender needs a GL-profile adapter on top of this substrate — not that we should build the full graphics stack first. Our recommendation aligns: compute now (glass), GL later (browser), on the same substrate.