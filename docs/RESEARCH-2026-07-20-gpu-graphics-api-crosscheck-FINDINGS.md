<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: GPU graphics/compute API design — cross-model FINDINGS

**Date:** 2026-07-20
**Author:** Chad (Codex; independent cross-check of Prometheus/Qwen)
**Status:** NO-GATE design review

## 0. Verdict

**REFUTE the reviewed recommendation as written.** **[High confidence]**

The durable part should survive: keep the Phase-6 Gen9 submission/PPGTT work,
define a small cap-native compute ABI, and use pinned kernels before attempting a
full wgpu-class graphics device. What does not survive is the load-bearing claim
that this is a **2–4 person-month Vello backdrop-blur path**. The paper neither
identifies a compiler that accepts Vello's WGSL (or Naga SPIR-V) and emits the
Gen9 executable/ABI consumed by this driver, nor prices building and validating
that path. `ocloc` compiling the project's hand-authored OpenCL C kernels is not
evidence for WGSL→EU lowering; “hand-lower SPIR-V” is the missing compiler, not a
small conversion step.

The Vello premise is also misstated. In the inspected Vello source, `backdrop.wgsl`
is a path-tile **inclusive prefix sum**, not a backdrop Gaussian blur. Full Vello
rendering is a multi-stage renderer with 64/256-thread workgroups, workgroup
memory, barriers, atomics, indirect dispatch, storage images, and ordered
resource operations. Its public GPU renderer is wgpu-specific. A useful internal
`Recording` IR exists, so a new backend is feasible, but it is a maintained Vello
engine/backend port plus shader closure—not “direct mapping of Vello render
commands” already supplied by upstream. **[High confidence]**

Amended direction: separate two projects and estimate them independently:

1. a minimal, manifest-only Gen9 compute service proven first with one
   project-authored linear-buffer glass kernel; and
2. a Vello backend/compiler-closure spike that must demonstrate an offline
   WGSL/SPIR-V→Gen9 artifact, execute Vello's actual recording closure, and render
   a byte-or-image oracle on the UHD 630 before Vello appears on the committed
   schedule.

## 1. Shader-compilation gap — load-bearing divergence

### 1.1 No demonstrated WGSL→Gen9 path

The existing toolchain proves this chain:

```text
project OpenCL C -> pinned IGC/ocloc -> device binary + zeinfo -> extracted EU text
                    -> hand-read ABI/payload -> in-kernel pinned blob
```

Vello supplies WGSL. Naga can validate WGSL and emit SPIR-V, but Naga has no
Intel-EU backend. The reviewed paper's alternatives do not close that gap:

- `ocloc` was proven here for OpenCL C kernels whose ABI is captured in `.zeinfo`.
  The paper proposes WGSL→OpenCL without identifying such a translator.
- Naga→SPIR-V does not make `ocloc` or Mesa ANV an already-integrated offline
  finalizer. A Vulkan driver normally consumes SPIR-V inside a driver pipeline;
  extracting a stable Gen9 kernel blob, descriptor/surface ABI, required program
  data, and cache/barrier metadata for this own command streamer is integration
  work that must be demonstrated.
- “Hand-lower SPIR-V→Gen9” is a shader compiler backend. It is the hidden monster.

Precompiling a fixed shader set is a sound **runtime policy** and bounds accepted
programs, but it does not make the compiler exist. The 2–4 pm estimate must not
include Vello until one shader with representative workgroup memory, barriers,
atomics, and image output travels through a named reproducible compiler and runs
on silicon. **[High confidence]**

### 1.2 The claimed shader is not backdrop blur

At Vello commit `53df8b970588392b7a0052a49922caa69a61101d`,
[`backdrop.wgsl`](https://github.com/linebender/vello/blob/53df8b970588392b7a0052a49922caa69a61101d/vello_shaders/shader/backdrop.wgsl)
states that each workgroup computes an inclusive prefix sum of path-tile
backdrops. It uses a 64-element `var<workgroup>` array and two barriers per scan
step. “Backdrop” here is the winding-number backdrop used by vector
rasterization; it is not a compositor backdrop-filter.

Vello's `fine.wgsl` contains an analytic blurred-rounded-rectangle primitive,
but that is not the reviewed paper's two-pass Gaussian read-source/write-
destination backdrop blur. The decision must first name the exact effect and
exact shader source. If the glass MVP merely needs a separable blur over an
already-composited linear pixel buffer, a small project-owned pinned kernel is a
far better first gate than porting Vello. **[High confidence]**

## 2. Vello ↔ wgpu coupling — divergence with a bounded seam

### 2.1 Public production path is wgpu-specific

Vello's default features enable wgpu. Its public `Renderer` owns a `WgpuEngine`;
construction takes a `wgpu::Device`, shader creation calls
`create_shader_module`/`create_compute_pipeline`, and rendering takes
`wgpu::Device`, `wgpu::Queue`, and `wgpu::TextureView`. The engine owns wgpu
buffers, textures, bind groups, pipeline cache, command encoders, copies, clears,
and dispatches. See pinned
[`lib.rs`](https://github.com/linebender/vello/blob/53df8b970588392b7a0052a49922caa69a61101d/vello/src/lib.rs)
and
[`wgpu_engine.rs`](https://github.com/linebender/vello/blob/53df8b970588392b7a0052a49922caa69a61101d/vello/src/wgpu_engine.rs).
There is no public generic `Renderer<Device>` backend trait comparable to iced's
generic renderer trait. **[High confidence]**

### 2.2 There is a real internal command seam

The paper is not wholly wrong about bypassing wgpu. Vello's internal
[`recording.rs`](https://github.com/linebender/vello/blob/53df8b970588392b7a0052a49922caa69a61101d/vello/src/recording.rs)
defines backend-neutral resource proxies and ordered commands: upload, image
write, download, clear, free, direct/indirect dispatch, plus buffer ranges and
read/write images. `render_full` produces this `Recording`, which `WgpuEngine`
interprets.

That is a credible extraction point, but it is internal and its executor is much
larger than the proposed `submit { pipeline, bind_group, workgroups }`. A Neo
engine must reproduce resource lifetime/pooling, zeroing, uploads, dynamic bump
buffers, indirect dispatch, image formats, hazards between commands, failure
readback, and every shader's binding ABI. It also has to track upstream changes.
Therefore Vello is **not inseparable from wgpu in principle**, but Vello as
shipped is not directly drivable by the proposed four-operation device. Require a
bounded backend spike before estimating it. **[High confidence]**

### 2.3 CPU path does not erase the decision

Current classic Vello contains CPU implementations of many shader stages and a
`use_cpu` option, but its production `Renderer` remains behind the wgpu feature
and the option describes running stages **up to fine rasterization** on CPU. It is
not evidence of a complete cap-native software compositor or a drop-in
`vello_cpu` glass path. A project-owned CPU blur/raster floor remains the honest
MVP fallback; it should land before GPU acceleration, but it does not validate
the Vello GPU estimate. **[High confidence]**

## 3. Phase-6 reuse — substrate corroborated, workload equivalence refuted

Phase 6 genuinely proves valuable mechanics: Gen9 kernel fetch/dispatch,
stateless A64 buffer access, per-context PPGTT, execlist submission, completion,
and negative cross-context mappings. Those should be reused. **[High confidence]**

It does not prove the Vello workload shape. The silicon gates exercised simple
1-D Add/Matmul/GELU kernels, pinned OpenCL ABIs, and stateless buffers. Vello's
shader closure includes:

- workgroups of 64 and 256 (and a 4×16 fine pass);
- workgroup arrays and repeated `workgroupBarrier()` calls;
- storage and workgroup atomics;
- direct and indirect multi-stage dispatch;
- storage-image output and image upload/copy behavior;
- cross-dispatch visibility and cache/flush correctness.

No cited Phase-6 gate proves SLM allocation/encoding, barrier correctness,
workgroup scheduling at those sizes, image/surface state, or the required memory
hazards. “Same RCS/EU hardware path” is true only at a coarse block-diagram level;
“zero hardware delta” and “almost entirely proven hardware” are unsupported.
Create representative gates in that order before full Vello. **[High confidence]**

## 4. WebRender extension — ordering corroborated, savings claim rejected

A cap-native allocation/context/queue/fence model is reusable, so compute-first
need not be thrown away. It is reasonable ordering if the first compute use is
independently valuable. **[High confidence]**

It does not reduce WebRender's principal graphics closure: textures and sampling,
render targets, surface state/tiling/formats, cache policy, vertex/fragment
programs, blending/scissor, GL object/state semantics, and the WebRender adapter.
The earlier Servo cross-check correctly found that WebRender's page-paint path is
GL, not Servo's wgpu WebGPU path. Thus graphics remains the separately priced
~9–14 pm-class wall (subject to a fresh closure spike). “Vertical extension, not
a rewrite” is fair only for the resource/submission substrate; it is misleading
as a statement about the total browser renderer. **[High confidence]**

## 5. I-14 and no-contradiction

PPGTT is sufficient to isolate a **correctly constrained shader access** between
GPU contexts when all reachable page tables and command state are kernel-owned;
BUILD-4X-3 directly supports that narrower claim. It is not the second I-14(c)
boundary against the device reaching arbitrary host physical memory. A graphics
VT-d domain or an equivalently proven fenced physical DMA arena is still required
before claiming that a GPU fault/compromise cannot reach governance or tensor
memory. **[High confidence]**

The reviewed paper contradicts itself: the executive table says the same
“PPGTT + VT-d” containment applies, §3.1 says that combination preserves I-14,
then §5.3 admits the GPU is identity-domained and defers the VT-d graphics domain.
Amend the ladder so the GPU physical-memory fence precedes granting a userland
compute queue or calling I-14(c) closed. A tiny trusted, manifest-only kernel
service may be an explicitly risk-accepted bring-up stage, but it is not the
final containment claim. **[High confidence]**

No architectural contradiction was found with OT-63's `GpuSubmitCap`/scanout
fence, own-Vello as a long-term glass direction, or ADR-032's Gen9 accelerator.
The contradiction is one of readiness and schedule: OT-63 present control is not
compute execution; ADR-032 is not a Vello shader/runtime; and “own Vello” does not
make the missing backend/compiler free. **[High confidence]**

## 6. Required replacement gates

1. **Effect closure:** specify backdrop capture, damage/occlusion semantics,
   radius/edge rules, format, and the exact kernel—not Vello's winding backdrop.
2. **Compiler closure:** pin source, compiler, flags, device target, executable,
   ABI metadata, and disassembly; run a representative SLM/barrier/atomic/image
   kernel on the UHD 630.
3. **Backend closure:** execute a frozen Vello `Recording` subset without wgpu,
   including upload, clear, indirect dispatch, ordered hazards, image output, and
   failure/bump readback; inventory every excluded command/shader.
4. **Silicon shape:** prove 256-thread workgroups, SLM, barriers, atomics,
   multi-dispatch visibility, and storage-image writes independently.
5. **Containment:** establish the graphics VT-d/fenced-physical-memory boundary,
   bounded waits/reset/quarantine, then adversarially submit the manifest-only
   queue from a disposable render service.

Only after gates 1–3 should the project quote a Vello person-month estimate. The
small compute service and one own blur kernel may still plausibly fit a bounded
slice; the evidence does not support attaching Vello to the current 2–4 pm
number. **[High confidence on scope correction; low confidence on any replacement
number until the spikes run.]**

## 7. Evidence and exact-source record

- RAG health was `healthy` (`ollama=true`, `qdrant=true`). Query used for
  discovery: `Vello compute shader WGSL wgpu renderer backend blur`; all
  load-bearing conclusions were checked against direct source.
- Vello was inspected at commit
  [`53df8b970588392b7a0052a49922caa69a61101d`](https://github.com/linebender/vello/tree/53df8b970588392b7a0052a49922caa69a61101d).
  The source files linked above are exact commit-pinned bytes, not rendered API
  summaries. Relevant additional source:
  [`shaders.rs`](https://github.com/linebender/vello/blob/53df8b970588392b7a0052a49922caa69a61101d/vello/src/shaders.rs) and
  [`fine.wgsl`](https://github.com/linebender/vello/blob/53df8b970588392b7a0052a49922caa69a61101d/vello_shaders/shader/fine.wgsl).
- Internal ground truth read directly: `docs/ADR-032-phase-6-intel-gen9-gpu-compute-driver.md`,
  `docs/ADR-045-phase-7.5-fenced-dma-arena.md`,
  `docs/ADR-050-phase-7.5-vtd-iommu.md`,
  `docs/RESEARCH-2026-07-20-ot61-browser-crosscheck-FINDINGS.md`, and the
  reviewed GPU-API FINDINGS.
- License compatibility was not load-bearing to this divergence. This review
  does not overturn the prior exact-byte MIT/Apache-2.0 classifications; any
  adopted frozen Vello subset still requires its normal locked transitive audit.

## 8. Divergences and corroborations

1. **Divergence:** no demonstrated WGSL/SPIR-V→Gen9 executable path exists in the
   reviewed design; offline precompilation bounds runtime policy but does not
   remove the compiler project.
2. **Divergence:** Vello's `backdrop.wgsl` is a winding-prefix shader, not a
   two-pass Gaussian backdrop blur; the proposed first deliverable lacks an exact
   effect/shader closure.
3. **Divergence:** Vello has a useful internal recording seam, but its public GPU
   renderer is wgpu-specific and a Neo executor is a substantive maintained
   backend, not a trivial mapping.
4. **Divergence:** Phase 6 proves submission/A64/PPGTT, not Vello's SLM,
   barriers, atomics, 256-thread workgroups, indirect stages, images, or hazards;
   WebRender's graphics wall is deferred rather than solved.
5. **Corroboration with amendment:** compute-first and pinned manifest kernels are
   sensible, and PPGTT is a real intra-GPU-context boundary; require a separate
   physical DMA fence for I-14(c) and prove one own blur kernel before Vello.
