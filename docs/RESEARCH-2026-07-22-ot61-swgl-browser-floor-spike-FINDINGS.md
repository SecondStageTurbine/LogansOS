<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-61 SWGL browser floor spike — FINDINGS

**Date:** 2026-07-22
**Lane:** Janus
**Verdict:** **PROCEED after the host-closure C0-C2 precursor.** SWGL can satisfy WebRender's real gleam GL contract in software and can render directly into an Atrium client-RW `SharedPixelBuffer`. It adds no new kernel host-surface class beyond the companion host-closure FINDINGS. The one required Servo seam is an `AtriumSwglRenderingContext` plus an honest split between page-paint gleam and optional WebGL/glow.

## 0. Build ladder and first milestone

| Rung | Dependency increment | Close gate |
|---|---|---|
| S0 | host-closure C0-C2; cross C++20 runtime; pinned SWGL | `swgl` and its generated shader C++ compile/link for `x86_64-unknown-neoplutarch`; context create/make-current/destroy has no unresolved host symbols |
| S1 | SWGL only, external target buffer | rasterize an asymmetric color/clip pattern into a supplied padded-stride byte buffer; compare every byte and canary; negative dimensions/stride fail without overwrite |
| S2 | WebRender 0.70 + SWGL | create one WebRender document, submit a display list containing solid, clipped, transformed, gradient, and text/image fixtures, render into SWGL, flush, and match a checked-in pixel oracle |
| S3 | `AtriumSwglRenderingContext` + surface protocol | WebRender writes the client-RW mapping directly, commits bounded damage, and the compositor presents from its RO+NX mapping; QEMU oracle proves pixels and client direct-present refusal |
| S4 | html5ever + Stylo + Servo layout | controlled HTML/CSS produces the expected display list and pixel oracle through the same path |
| S5 | SpiderMonkey closure | scripted DOM/style mutation changes only expected display-list and pixel regions; content crash remains confined |

### First prototype milestone

Run **S1 first**, not a full page: a target-native `swgl-atrium-smoke` binds the mapped pointer and real stride of one `SharedPixelBuffer` with `Context::init_default_framebuffer`, draws an asymmetric pattern through gleam calls, invokes `get_color_buffer(0, true)` to resolve delayed clears, commits damage, and hands the buffer to the compositor. The oracle checks both raw XRGB bytes and final framebuffer pixels.

**Prototype status: blocked by an exact precursor, not by SWGL ambiguity.** The repository still lacks the host-closure target PAL, C++ runtime, user threads/TLS/waits, monotonic clock, and general VM operations identified in `RESEARCH-2026-07-22-ot61-servo-host-closure-spike-FINDINGS.md` C0-C2. A host-platform SWGL demo would not falsify the target risk, so no substitute artifact was produced in this NO-GATE spike.

## 1. Source pins and discovery

All upstream claims were checked in Git source worktrees:

| Source | Exact revision | Pin basis |
|---|---|---|
| Servo | `25b89d879cecef6e63203f70993ba22b5e7cd9ef` | OT-61 source pin |
| WebRender + SWGL | `2e341fa14234aa6f1dfb8d3309e7eae0f5d1d7f7` | head of upstream `0.70` release branch, “Prepare WebRender for crates.io release”; Servo locks WebRender 0.70.0 |
| gleam | `e7b3f3296c70093f35357e644dc8711c1f79fc3d` | peeled `v0.15.1` tag; exact Servo/WebRender lock |
| surfman | `205778f497327c573929c7b471194390e15f331d` | `v0.13.0` tag; exact Servo lock |

Servo's `Cargo.lock` contains WebRender 0.70.0, gleam 0.15.1, and surfman 0.13.0, but **does not contain `swgl`**. Upstream WebRender keeps SWGL as its own 0.70 workspace crate and an optional `sw_compositor` dependency (`webrender/Cargo.toml:22,57`); wrench's `software` feature enables it (`wrench/Cargo.toml:36,51`). The LogansOS integration is therefore an additive provider patch, not an existing Servo feature toggle.

RAG was healthy (`ollama=true`, `qdrant=true`) and the dispatched queries were run:

- `atrium surface present scanout_authorized compositor`
- `SharedPixelBuffer present seam`
- `swgl webrender gl contract`
- `no_std target libneo host surface`

RAG found ADR-073, the current scanout seam, the browser architecture, and target specs. Source claims below were verified directly.

## 2. Feasibility result: SWGL's host demand

SWGL is substantially smaller than the whole browser host closure. It does **not** introduce filesystem, network, process, dynamic-loader, or GPU runtime authority.

### 2.1 Runtime demand

| SWGL demand | Source evidence | Mapping to host-closure surface | Delta |
|---|---|---|---|
| C/C++ allocation and basic runtime | `gl.cc:5-10` includes C allocation/string/assert/stdio/math; buffers use `realloc/free` at `:317-342,536-563`; objects use `new/delete` at `:465-475,714-766,3084-3105` | companion C/C++ runtime row | no new class; strengthens allocator/libm/libc acceptance gate |
| monotonic clock | GL time queries use `clock_gettime(CLOCK_MONOTONIC)` on the non-Apple/non-Windows path (`gl.cc:1548-1570`) | companion time/clock row | no new class; add nanosecond monotonic adapter |
| SIMD | x86 SSE2 and ARM NEON are selected in `vector_type.h:6-12`; hot raster paths are conditional on `USE_SSE2` | x86_64 target already implies SSE2 in the architecture baseline | no syscall; require compiler-feature and scalar-oracle equivalence gate |
| heap-backed textures plus external buffers | `InitDefaultFramebuffer` and `SetTextureBuffer` accept caller storage (`gl.cc:2554-2625`); internal textures allocate when pointer is null | heap/VM row | no new class; target buffer can avoid the final color allocation/copy |
| GL context currentness | one process-global `static Context *ctx` (`gl.cc:898`) is changed by `MakeCurrent` (`:3076-3084`) | browser paint thread from thread row | no new primitive, but initial adapter must serialize one current paint context |
| optional parallel software composition | WebRender's SW compositor uses atomics, `Mutex`, `Condvar`, and a spawned thread (`webrender/src/compositor/sw_compositor.rs:10-12,881-924`) | threads/sync/waits rows | no new class; keep off in S1-S2, enable only after C1 contention gate |

SWGL's core does not spawn a thread. Its locked-resource API is explicitly shareable (`swgl_fns.rs:2350-2500`); WebRender supplies any composite worker. The global current-context pointer is not TLS. That is acceptable for the first one-paint-thread process, but multi-context use must either remain serialized or patch it to target TLS after C1.

### 2.2 Build-host demand is not target runtime demand

`swgl/build.rs` reads shader sources and writes generated C/C++ in `OUT_DIR` (`:34-51,91-136`), then compiles C++20 with exceptions and RTTI disabled (`:178,205-212`). This runs in the Linux CI build container. It needs a cross C++20 compiler, `cc`, and the in-tree `glsl-to-cxx` translator. The generated shaders and `gl.cc` are statically linked into the target module; there is no runtime shader compiler, file read, or environment lookup.

That distinction is capability-positive: target web content cannot supply a shader to this path. SWGL executes the fixed WebRender shader set generated at build time.

### 2.3 Panic/unsupported entry points

`swgl::Context` implements `gleam::gl::Gl` (`swgl/src/swgl_fns.rs:504`), but some entry points deliberately call `unimplemented!` (`:978,1423,1498-1512,2292-2344`). WebRender's SWGL configuration is the supported profile; arbitrary WebGL is not. S2 must enable `panic_on_gl_error` and exercise every pinned WebRender shader/operation variant used by the browser fixture set. Any new WebRender call reaching an unsupported entry is a release-refresh gate failure.

### Fold-back to the host-closure table

SWGL adds **zero new rows** to the companion gap table. Fold its requirements into three existing rows:

- **C/C++ runtime:** add C++20, `malloc/realloc/free`, `new/delete`, string/memory, assert, and libm subset;
- **time:** add monotonic nanosecond clock for GL queries;
- **threads/sync:** WebRender software-compositor worker is optional until the existing native wait gate passes.

## 3. The actual GL contract and the Servo integration seam

### 3.1 WebRender consumes gleam, and SWGL supplies it

gleam defines the object-safe `Gl` trait in `gleam/src/gl.rs:84` with buffer, texture, shader/program, framebuffer, blend, draw, query, sync, and readback operations. WebRender's constructor takes `Rc<dyn gl::Gl>` (`webrender/src/renderer/init.rs:303-304`) and immediately wraps it in its `Device` (`:338`). A rendered frame reaches this device through `Renderer::render` (`webrender/src/renderer/mod.rs:1327`).

SWGL's Rust wrapper:

- creates and makes current a native software context (`swgl_fns.rs:329-350`);
- implements the gleam trait (`:504`);
- binds an external buffer as framebuffer zero (`:354-366`);
- exposes and flushes its color buffer (`:368-382`);
- can bind external storage to texture objects (`:407-430`).

Wrench proves the intended composition: it creates/makes-current a SWGL context (`wrench/src/main.rs:355-359`), returns it as `Rc<dyn gl::Gl>` (`:301-309`), initializes framebuffer zero (`:320`), and obtains the flushed pixels with `get_color_buffer(0, true)` (`:200`). We can reuse this contract without its native-window upload.

### 3.2 Servo's current “software” context is not SWGL

Servo's `SoftwareRenderingContext` calls `surfman::Connection::create_software_adapter()` (`components/shared/paint/rendering_context.rs:289-315`). It is the surfman/OSMesa-like path, not the `swgl` crate, which is absent from Servo's lockfile.

The same file's trait requires:

- prepare, resize, present, and make-current (`:35-62`);
- `Rc<dyn gleam::gl::Gl>` for page paint (`:64`);
- a separate `Arc<glow::Context>` (`:66`) used by WebGL/offscreen paths.

SWGL supplies gleam but not a glow procedure loader. A fake glow object would create false WebGL claims and unsafe missing entry points. The bounded patch is:

1. add an `AtriumSwglRenderingContext` for page paint;
2. split the optional WebGL provider from the mandatory WebRender gleam provider, for example `fn glow_gl_api(&self) -> Option<Arc<glow::Context>>`;
3. make WebGL canvas creation return “unavailable” on the software floor;
4. leave surfman texture/connection methods at their existing `None` defaults;
5. later install `surfman-neo`/`libneogl` as the hardware provider behind the same rendering-context selection.

This is a narrow maintained provider patch. It does not touch WebRender display lists, Stylo, layout, DOM, or SWGL raster code.

## 4. End-to-end LogansOS pipeline

```text
Servo layout/paint
      |
      | WebRender display list / transaction
      v
WebRender Renderer -- Rc<dyn gleam::Gl> --> SWGL Context
                                              |
                                              | framebuffer 0 = client RW+NX mapping
                                              v
                              Atrium SharedPixelBuffer (XRGB8888_LE)
                                              |
                         flush -> commit_damage -> Surface Commit message
                                              v
                       compositor RO+NX mapping + GpuSubmitCap
                                              |
                         sys_fb_present / fence / chosen backend
                                              v
                                    software blit or fenced flip
```

### 4.1 Buffer construction and direct raster

The browser surface client creates a validated XRGB8888 buffer, maps role 0, and grants the compositor only `MAP_READ | QUERY | PRESENT`, matching the live pattern in `userland/neo-worker/src/main.rs:537-615` and `userland/neo-init/src/arthur_compositor.rs:80-110`. Kernel invariants make client mappings RW+NX and compositor mappings RO+NX (`kernel/src/atrium/buffer.rs:6-15`).

`AtriumSwglRenderingContext::prepare_for_rendering` makes its SWGL context current and calls:

```text
init_default_framebuffer(0, 0, width, height, atrium_stride, client_mapping)
```

SWGL treats non-null external storage as caller-owned and does not free it. Atrium accepts stride at least `width * 4`, aligned to four bytes, and SWGL accepts an explicit stride. This eliminates the wrench path's upload/readback copy.

SWGL's raw color buffer is consumed as BGRA bytes by wrench (`wrench/src/main.rs:200-203`). Atrium XRGB8888_LE is a little-endian `0x??RRGGBB` word (`kernel/src/atrium/logic.rs:14-17`), which is bytes B,G,R,X. The layouts align; Atrium ignores the high byte. S1 must use distinct red and blue values to falsify a channel-swap mistake and nontrivial row padding to falsify a stride mistake.

### 4.2 Resolve, damage, and commit

After `Renderer::render`, call `get_color_buffer(0, true)` even when its returned pointer is already known: `flush=true` forces delayed clears/materialization (`swgl/src/gl.cc:2574-2593`). Confirm returned pointer, geometry, and stride equal the bound Atrium storage, then publish the WebRender dirty rectangle with `sys_buffer_commit_damage` (`userland/libneo/src/lib.rs:2007-2014`). Start with full-surface damage at S2; narrow only after the dirty-region oracle passes.

The browser then sends the Atrium surface `Commit`. It never calls `sys_fb_present`. The compositor receives the read/present grant, validates protocol serials, and owns `sys_fb_present` plus the present fence, as the live compositor does at `userland/neo-init/src/arthur_compositor.rs:200-230`.

### 4.3 Present authority and scanout fencing remain structural

`SharedPixelBuffer::present` checks `scanout_authorized()` before mutating present state (`kernel/src/atrium/buffer.rs:192-214`). Only the compositor manifest has GpuSubmitCap. A browser client direct-present attempt therefore returns `EPERM`; the existing showcase already treats that refusal as a load-bearing oracle.

If the backend selects hardware flip, buffer creation uses the DMA arena while fencing is enabled (`kernel/src/atrium/buffer.rs:83-111`), and flip arming admits only a `FencedDmaAddr` (`kernel/src/atrium/buffer.rs:315-318`; `present_backend.rs:89-102,222-247`). SWGL only writes virtual pixels. It never sees physical addresses, the DMA arena, `GpuSubmitCap`, fence currency, MMIO, or scanout. On any flip refusal the existing backend degrades to software; the SWGL surface contract does not change.

This is the kill-switch property: hardware off means the identical WebRender -> SWGL -> Atrium buffer path ends in the software present backend. Hardware GL later replaces only `Rc<dyn gleam::Gl>` and its buffer-producing provider; the Atrium commit/compositor authority boundary remains.

### 4.4 Resize and lifetime rules

Resize must not mutate storage under an in-flight render/present:

1. allocate and map a new `SharedPixelBuffer`;
2. bind it to SWGL only between frames;
3. grant/import it to the compositor and receive acceptance;
4. atomically advance the surface attachment/serial;
5. retire the old client mapping only after its present fence and SWGL no longer references it.

The kernel's buffer `Arc` pins physical frames across mappings and present (`kernel/src/atrium/buffer.rs:6-14`), but that does not excuse SWGL retaining an obsolete raw pointer. S3 needs a resize-under-frame-pressure oracle.

## 5. Per-rung falsifiable gates

### S0 — target build

- Exact pinned SWGL builds with C++20, no exceptions, and no RTTI.
- Link map contains only allowlisted private CRT/PAL symbols.
- No runtime file, environment, loader, network, or process import.
- `Context::create/make_current/destroy` passes under leak accounting.

### S1 — raster-to-external-buffer

- Clear/scissor/blend pattern matches a byte oracle including asymmetric R/B and alpha ignored by XRGB.
- Padded stride rows and front/back canaries remain intact.
- `get_color_buffer(0, true)` returns the supplied pointer and expected geometry.
- invalid/overflow geometry is rejected before calling SWGL; no partial write.
- one-context serialization is asserted; concurrent make-current is refused until TLS policy exists.

### S2 — real WebRender

- Pinned WebRender takes `Rc<dyn Gl>` and renders one generated display list.
- Golden fixtures cover clip, transform, opacity/blend, border radius, gradient, image sampling, glyph, filter, and damage.
- no gleam method reaches SWGL's `unimplemented!` paths.
- output is deterministic across repeated frames and scalar-vs-SSE2 comparison samples.

### S3 — Atrium

- the same mapped buffer reaches the compositor with no full-frame copy;
- client mapping is RW+NX; compositor mapping is RO+NX;
- client direct present is `EPERM`, compositor present succeeds;
- out-of-bounds damage and forged/stale surface serial are refused;
- software backend completes; fencing-on hardware selection cannot arm an unfenced address;
- resize and client crash preserve mapping/buffer lifetime.

### S4-S5 — document closure

- HTML/CSS fixture produces a stable layout snapshot, display-list snapshot, and pixel oracle;
- font fallback, bidi, shaping, scroll/clip, image decode, and invalidation batteries pass;
- after SpiderMonkey lands, scripted mutation and timer-driven repaint change only their oracle regions.

## 6. What the software correctness floor proves

### It can prove

- html5ever parse and DOM construction once the host closure exists;
- Stylo selector, cascade, inheritance, media-query, and invalidation behavior;
- layout geometry, fragmentation, overflow, scroll/clip, and device-scale handling;
- WebRender display-list construction and fixed shader semantics;
- software raster output for fills, borders, gradients, images, glyphs, clips, transforms, blend/filter cases supported by SWGL;
- buffer format/stride/damage correctness and the complete capability-confined Atrium handoff;
- deterministic visual regression images independent of GPU driver variance.

The battery should keep three oracles per fixture when useful: layout tree, display list, and pixels. A pixel difference can then be localized to layout/paint versus raster/present.

### It cannot prove

- JavaScript, GC, Web APIs, or DOM mutation before SpiderMonkey C3;
- network/TLS/cookie/cache correctness before the broker closure;
- WebGL or WebGPU: SWGL here is WebRender's trusted fixed-shader page renderer, not an arbitrary site shader API;
- hardware GL shader compiler, texture/cache behavior, driver isolation, reset, IOMMU, or DMA correctness;
- frame pacing, vblank, tearing, hardware throughput, power, or latency;
- hardware color-management/scanout fidelity.

Success must therefore be labeled **software page-paint correctness**, never “browser complete” or “GPU complete.”

## 7. Go/no-go and maintained seam

### Go

- host-closure C0-C2 passes;
- S0 has a finite native symbol surface;
- direct external-buffer S1 passes channel/stride/canary oracles;
- the glow provider becomes explicitly optional rather than faked;
- S2 never reaches an unsupported SWGL GL entry;
- S3 preserves compositor-only present and scanout fencing.

### Stop or redesign

- SWGL requires writable-executable target memory or a runtime shader compiler;
- an exact WebRender fixture needs unsupported GL operations that cannot be bounded to the pinned page-paint profile;
- Servo integration requires granting the browser GpuSubmitCap or direct-present rights;
- a zero-copy buffer binding cannot preserve Atrium mapping lifetime and SWGL pointer lifetime;
- making glow optional spreads beyond provider/WebGL construction seams into DOM/layout core;
- performance tuning pressures the design into bypassing commit/damage/compositor ownership.

### Refresh gate

For each WebRender/SWGL refresh, pin the Git revision, diff the generated shader set and gleam calls, scan new host imports, run S0-S3 oracles, and fail if an unsupported entry or new authority-bearing operation appears. The patch inventory should remain:

1. SWGL dependency pin;
2. `AtriumSwglRenderingContext`;
3. optional WebGL/glow provider split;
4. target build configuration;
5. oracle fixtures.

If it expands into WebRender raster internals, Servo layout/DOM, or direct present, the seam has failed.

## 8. Bottom line

The software browser floor is technically sound and unusually well matched to Atrium. SWGL already has the exact capability we need: caller-owned framebuffer memory. That allows WebRender to paint into a confined surface without a window system, GPU, copy-to-native-GL step, physical address, or scanout privilege. The difficult dependency remains the native browser host runtime from the companion spike; once that passes C0-C2, SWGL is a bounded next rung with crisp byte and authority oracles.
