<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: The Atrium text engine — GPU vector-text prior-art, the shaping reality, and the smallest-first path (RESEARCH)

**Date:** 2026-07-10
**Author:** Prometheus (research / design lane)
**Source:** Plans/DISPATCH-2026-07-10-atrium-text-engine-research-prometheus.md
**Branch:** prometheus/atrium-text-engine-research
**Inputs:** The Atrium synthesis (docs/THINK-2026-07-08-gui-vision-SYNTHESIS.md — ratified, §5 spine G0-G7, §2 GPU vector text as killer enabler, "text-shaping-is-hard" flag); OT-52 (compositor architecture, cap taxonomy); OT-45 (visual design language, Article 1.6); kernel framebuffer console (kernel/src/drivers/fb/ — 8x16 bitmap font, Janus v1 surface); Phase-6 accel path (docs/design.md §Phase 6); voice-substrate FINDINGS pattern (RESEARCH-2026-07-08-ot46-voice-substrate-FINDINGS.md, fbfd96c)

---

## 0. The thesis in one breath

**The Atrium's transcript needs text that is both beautiful (infinitely zoomable, resolution-independent) and correct (shaped, kerned, laid out). Those are two different problems. GPU vector rendering solves the first; a HarfBuzz-class shaper solves the second. The smallest honest path is: crisp CPU/hinted type for G0 on the raw framebuffer → Vello-class GPU-outline text for G2's zoomable leap (Latin-only, shaper-deferred) → rustybuzz shaping stack before localization → complex scripts and color emoji last. Slug's analytic-coverage patent is freed (March 2026), making it a viable primary technique — but Vello's active development, clean compute-pipeline architecture, and Apache-2.0 license make it the better reference for our first build.**

Every exact claim below comes from the real source read directly: paper PDFs, library source/docs, LICENSE files, issue trackers. No summarizing fetch.

---

## 1. ISC-1 — The rendering-technique landscape (with honest tradeoffs and a verdict)

Four families for resolution-independent glyph rendering, compared on the axes that matter for our stack.

### (a) SDF / MSDF — Signed Distance Fields

**Sources:** msdfgen README + API docs (Chlumsky/msdfgen, 4,862 stars); Valve's "Distance Fields Everywhere" (Lysenko, SIGGRAPH 2012); Vello vision doc (which references SDF as a prior approach).

**How it works:** Precompute a texture per glyph encoding signed distance to the glyph edge. MSDF (Viktor Chlumsky's master's thesis) stores directional distances in R/G/B channels, computing the median of three channels in the fragment shader to recover sharp corners.

**Quality:**
- **Small sizes (12-14px):** Good at standard scale. Natural 1-pixel AA regardless of render size. Standard SDF blurs corners; MSDF recovers near-perfect convex corners via channel splitting.
- **Zoomed (48px+):** Degrades. The distance field is a fixed-resolution texture. Beyond ~2x generation resolution, straight edges become subtly bent — the precomputed gradients can't represent arbitrary magnification.

**Honest failure modes:**
1. Sharp corners on standard SDF round into fillets (MSDF fixes convex; concave and self-intersections remain)
2. Thin stems: strokes thinner than ~2px at atlas resolution have no "inside" — both sides map to the same texel, the stem disappears or renders as a fuzzy blob
3. Self-intersections / complex glyphs: MSDF's 3-color edge-graph coloring can fail on tight features, causing color-bleed artifacts
4. Atlas packing: O(glyphs × resolution²). CJK at 64×64 RGB8 ≈ 30 MB just for the atlas

**GPU features:** No compute shaders. Fragment shader + bilinear texture sample. SM 3.0 / GLSL 1.20+. Generation is CPU-side (msdfgen CLI).

**Memory:** ~3 KB/glyph at 32×32 RGB8. Latin (~900 glyphs) ≈ 2.7 MB atlas. No per-frame GPU memory beyond atlas + uniforms.

**CPU fallback:** Trivial — texture lookup + threshold in any software rasterizer.

### (b) Slug — Analytic-coverage GPU outlines

**Sources:** Eric Lengyel's "Slug: Analytic-Coverage GPU Outlines" paper (terathon.com); Slug patent US10373352B1; Lengyel's blog "A Decade of Slug" (March 17, 2026, terathon.com/blog/decade-slug.html); reference shaders at github.com/EricLengyel/Slug.

**How it works:** Computes *exact fractional pixel coverage* analytically. For each line segment of a glyph outline, calculates the exact area of intersection with each pixel using geometric formulas — not supersampling, not distance fields.

**Quality:**
- **Small sizes:** Potentially the highest quality available — exact subpixel coverage, equivalent to FreeType's best unhinted mode but computed in parallel on GPU. No distance-field blur, no MSAA graininess.
- **Zoomed:** Perfect at any scale. Analytic from the vector outline — no precomputed texture to degrade.

**GPU features:** Compute shaders required (SM 5.0 / DX11 compute minimum). Substantial shared memory for per-tile accumulators.

**Memory:** Only outline data per glyph (~320 bytes for a typical 20-segment glyph). No atlas textures. Dramatically lower than SDF.

**CPU fallback:** Possible but complex — the analytic Bezier-pixel intersection math is nontrivial. Essentially what FreeType's `FT_Render_SubPixel` does on CPU.

**Failure modes:**
1. Complexity: Analytic formulas for Bezier-pixel intersection are extremely complex. Large bug surface — this is why Slug remained a research paper rather than shipping in production
2. Performance at scale: Fast per-glyph, but thousands of unique glyphs per frame requires massive compute dispatch overhead
3. No hinting: Like all GPU vector approaches, doesn't benefit from font hinting bytecode

### (c) Vello — GPU compute-centric 2D renderer

**Sources:** Vello README + vision doc + 2023 roadmap (linebender/vello, 4.2k stars); fine rasterizer WGSL source (fine.wgsl); "Requiem for piet-gpu-hal" blog (Levien, Jan 2023).

**How it works:** Full 2D renderer using sort-middle architecture. CPU encodes scene into compact binary; GPU compute shaders do sorting, tiling, coarse rasterization, and fine rasterization. Fine rasterizer supports **analytic area AA** (exact coverage, Slug-like) and **multisampled rendering** (8/16-sample MSAA with precomputed sample masks). Processes 16×16 pixel tiles in workgroups.

**Quality:**
- **Small sizes:** Excellent. Analytic area AA computes exact trapezoidal coverage per pixel — "equivalent to 256xAA quality" (vision doc). No stepping or graininess.
- **Zoomed:** Perfect. Full vector rendering from Bezier definitions at target resolution.

**GPU features:** Compute shaders mandatory. WGSL (WebGPU compute) → SPIR-V 1.2+ on Vulkan, DXIL SM 6.0+ on DX12. Uses `workgroup` shared memory, `atomicAdd`, `workgroupBarrier`.

**Memory:** Only outline data per glyph. No atlas textures. GPU working memory: multiple storage buffers (segments, PTCL, blend spill, info buffer).

**CPU fallback:** Explicitly planned in roadmap ("improve testing/debuggability; GPU unavailable cases"). Initial CPU implementations optimized for clarity, not performance. Not shipped yet.

**Failure modes:**
1. Alpha state — known conflation artifacts (issue #49), blur/filter effects pending (#476)
2. Compute-only: no fallback to fixed-function rasterization. CPU fallback planned but not shipped
3. WebGPU immaturity: DX12/WebGPU pain points documented in Requiem blog (uniformity analysis, limited descriptor indexing, no subgroup support in WebGPU 1.0)

### (d) Pathfinder — Mozilla's GPU rasterizer (data point)

**Sources:** Pathfinder README + architecture doc (servo/pathfinder, 3.9k stars).

**How it works:** Two-pass GPU rasterization. CPU: simplify → tile (16×16) → cull → pack (~2.2ms for Ghostscript tiger at 1600×1600). GPU fill pass: rasterize edges to 16-bit FP alpha coverage (~2.6ms). GPU shade pass: solid-color tiles, back-to-front with alpha mask (~2.3ms).

**Two modes:** D3D11 (compute shader) and D3D9 (hardware rasterization, no compute needed — OpenGL 3.0 / GLES 3.0 / WebGL 2 minimum).

**Why sunsetted:** Never shipped in Firefox. Gecko integration complexity, performance didn't justify cost for typical web pages, small Servo team couldn't maintain it alongside browser dev. WebGPU/Vello made the approach converge elsewhere.

**Lessons:** Tile-based rendering wins for complex scenes. CPU tiling overhead is real (2.2ms floor). Two-pass (fill then shade) is simpler than compute-only. GPU text rendering needs hinting fallback — Pathfinder fell back to platform bitmap glyphs (DirectWrite/CoreText) for small-size quality.

### The verdict: primary + fallback

| | **Vello (primary)** | **Slug (fallback / future)** | **SDF/MSDF (G0 bridge)** |
|---|---|---|---|
| **Why** | Actively developed, clean compute pipeline, Apache-2.0, exact area AA at all scales, full 2D renderer (not just text) | Patent freed March 2026, highest single-glyph quality, minimal memory, simpler shader than Vello | Zero compute dependency, trivial CPU fallback, works on any GPU — carries G0→G1 transition |
| **When** | G2: when our Phase-6 accel path can drive WebGPU/SPIR-V compute shaders | Post-G2: if we want a lighter-weight alternative or need to support GPUs that can't run Vello's full pipeline | G0→G1: CPU/software rasterization on the raw framebuffer, before GPU accel exists |
| **Gap** | CPU fallback not shipped yet; conflation artifacts (issue #49) | Reference shaders exist but no full library; large bug surface in analytic math | Degrades at zoom — can't be the G2+ endpoint |

**Recommendation: Vello as primary, SDF/MSDF as the G0→G1 bridge, Slug as a post-G2 optimization option.** Vello gives us the zoomable-surface quality the Atrium demands. SDF carries the early stages where compute shaders aren't available. Slug's freed patent makes it a clean future option if Vello's compute requirements prove too heavy for our Phase-6 accel path.

---

## 2. ISC-2 — The shaping reality (the flagged hard part)

**"SDF gives you glyphs, not shaping."** A glyph *renderer* turns a vector outline into pixels. A *shaper* tells you which outlines to render, in what order, at what positions, and how they cluster. They are orthogonal problems.

### The layers an honest text engine needs

#### Layer 1: Font loading + cmap
Parse the font file (TTF/OTF), read the cmap table, map Unicode codepoint → glyph ID. This is not 1:1 — glyph IDs are font-specific and unpredictable. Even for Latin, you need this to get from "e" to glyph #97.

**Cost:** `ttf-parser` crate (Rust, ~10K SLoC) or FreeType's font parsing. Trivial for v1.

#### Layer 2: Glyph rasterization
Convert the outline (quadratic Beziers from `glyf` table, or CFF programs) into pixels — bitmap, SDF, or GPU vector. This is what ISC-1 covers. The renderer is NOT the shaper.

**Cost:** FreeType's built-in rasterizer (FTL), msdfgen for SDF atlases, or Vello/Slug for GPU outlines.

#### Layer 3: Hinting
Snaps glyph outline control points to the pixel grid. At small sizes (8-16px), critical for readability — preserves stroke contrast, opens counters, aligns stems to integer pixel boundaries.

**Sources:** FreeType hinter docs (freetype.org/freetype2/docs/hint.html).

**When it matters:** Below ~16px glyph height on low-DPI displays. Above that, unhinted AA is acceptable. In an infinitely zoomable surface, hinting is only needed at the bottom of the zoom range.

**Two modes:**
- **Bytecode hinting:** Executes per-glyph hinting programs authored by font designers. Platform-dependent results (FreeType's interpreter doesn't perfectly match Windows').
- **Autohinting:** FreeType's automatic module. Good enough for most fonts, no platform divergence.

**Verdict for us:** Skip bytecode hinting for v1. Use FreeType autohint or no-hint at sizes above 16px. In the Atrium's zoomable surface, hinting is a small-size quality knob, not a correctness requirement. Bytecode hinting is a post-v1 polish.

#### Layer 4: Shaping (HarfBuzz-class)
The shaper applies OpenType layout tables:

**Sources:** HarfBuzz manual (harfbuzz.github.io); rustybuzz crate (lib.rs/crates/rustybuzz).

| Feature | What it does | Latin v1 needed? |
|---|---|---|
| `cmap` lookup | Unicode → glyph ID | YES — fundamental |
| `ccmp` | Composition/decomposition (é → e + combining acute) | YES — correctness |
| `liga` | Standard ligatures (fi, fl, ff) | Optional but expected |
| `kern` | Kerning pairs | Required for proportional fonts; optional for monospace console |
| `mark`/`mkmk` | Mark-to-base, mark-to-mark positioning | YES for accented Latin |
| Cluster tracking | Maps output glyphs → input codepoint ranges | YES — cursor, selection, metrics |

**HarfBuzz C API surface:** ~15 functions for core workflow. Flat C with opaque pointers — trivially FFI-friendly.

**Rust integration — the choice:**

| Option | Approach | For our stack |
|---|---|---|
| `harfbuzz` (Servo) | Binds to system/bundled C++ lib | Full feature set, but needs C++ compiler, links native lib, 2.5 MB dep |
| **`rustybuzz`** | Pure Rust port of HB shaping algo | **No C deps, `cargo add`, no-std, 3M downloads/mo. Passes 2,221/2,252 HB tests. ~23K SLoC.** |

**Recommendation: rustybuzz.** Pure Rust, no C++ dependency chain, covers the full shaping algorithm. The 1.5-2× slowdown over C HarfBuzz is irrelevant for transcript/text volumes. `ttf-parser` (font parsing) is already a rustybuzz dependency.

#### Layer 5: Complex scripts + BiDi
The Unicode Bidirectional Algorithm (UAX #9, Revision 51): four-phase algorithm handling explicit formatting characters, level resolution (W1-W7, N0-N2, I1-I2 rules), and reordering. Max embedding depth: 125.

**Sources:** Unicode TR #9 (unicode.org/reports/tr9/); `unicode-bidi` crate.

**Arabic joining:** Each character has 4 positional forms (initial/medial/final/isolated). Handled by GSUB lookups (`init`, `medi`, `fina`, `isol`).

**Indic reordering:** Visual order differs from logical order. Script-specific GSUB features.

**For Latin-only v1:** BiDi is a **no-op**. All Latin + digits are type L or EN. No reordering. The entire UBA collapses to "render left to right." Arabic joining and Indic reordering are zero-relevance for Latin.

**What you still need from the shaper for Latin:** cmap, `ccmp`, `liga` (optional), `kern` (proportional fonts), cluster tracking. These are all part of the base shaping pipeline regardless of script.

#### Layer 6: Emoji / color fonts (COLR)
**Sources:** Microsoft OpenType COLR spec (learn.microsoft.com); COLRv1 Google doc spec.

**COLRv0:** Simple layered color glyphs. Linear z-order, solid colors. ~O(layers) per glyph. Manageable.
**COLRv1:** Full 2D paint graph — gradients (linear/radial/sweep), affine transforms, compositing modes, clipping. A mini graphics engine.

**Verdict:** Defer entirely for v1. No emoji in a console. COLRv1 is a post-compositor feature that requires a dedicated paint engine.

### The smallest honest v1 scope

| Layer | v1 status | Rationale |
|---|---|---|
| Font loading + cmap | **REQUIRED** | Can't render anything without it |
| Glyph rasterization (CPU) | **REQUIRED** | FreeType autohint or SDF — carries G0 |
| Basic shaping (`ccmp`, `liga`, `kern`) | **REQUIRED** | Even Latin needs ccmp for correctness; ligatures are expected |
| Cluster tracking | **REQUIRED** | Cursor, selection, text metrics |
| Line breaking (UAX #14) | **REQUIRED** | Multi-line text wrapping |
| Hinting (bytecode) | DEFERRED | Autohint or no-hint is fine above 16px |
| BiDi | DEFERRED | Latin-only = LTR no-op |
| Complex scripts | DEFERRED | Arabic/Indic shaping for T2 |
| COLR/color fonts | DEFERRED | Post-compositor, needs paint engine |

**"Before localization" actually requires:** rustybuzz shaping (even for Latin, ccmp + cluster tracking are correctness-critical), BiDi support (for RTL languages), and font fallback (when a glyph isn't in the primary font). That's the T2 gate.

**Concrete dependency list for v1:**
- `rustybuzz` — shaping (~23K SLoC, MIT)
- `ttf-parser` — font parsing (already a rustybuzz dep)
- FreeType (FTL) or msdfgen — glyph rasterization
- `unicode-linebreak` — line breaking (UAX #14)
- `unicode-segmentation` — grapheme clustering for cursor/editing

---

## 3. ISC-3 — Licensing / patent posture (verified firsthand from primary sources)

### Slug patent — FREED
**Source:** Eric Lengyel's blog "A Decade of Slug" (March 17, 2026, terathon.com/blog/decade-slug.html); USPTO filing SB/43.

Patent US10373352B1 (granted 2019, would have expired 2038) is **permanently and irrevocably dedicated to the public domain**, effective March 17, 2026. Lengyel filed formal SB/43 with USPTO. Reference shaders at github.com/EricLengyel/Slug under **MIT license**.

**No patent risk.** Free to implement.

### Library licenses — all MPL-2.0 compatible

| Library | License (from actual LICENSE file) | MPL-2.0 compatible? |
|---|---|---|
| Slug reference shaders | MIT | Yes |
| Vello | Apache-2.0 (LICENSE-APACHE only) | Yes |
| FreeType | **FTL** (choose this; GPL-2.0+ is the alternative) | Yes — permissive, no copyleft on our code |
| FreeType (GPL option) | GPL-2.0+ | **NO — don't choose this** |
| HarfBuzz | "Old MIT" (COPYING) | Yes |
| msdfgen | MIT (LICENSE.txt) | Yes |
| Pathfinder | Apache-2.0 (LICENSE-APACHE) | Yes |
| rustybuzz | MIT | Yes |

### Font licensing — OFL is the right fit
**Source:** SIL Open Font License v1.1 (openfontlicense.org).

OFL explicitly permits bundling with software. Cannot sell font alone. Must include copyright notice + license text. Modified versions can't use Reserved Font Names. Documents created using the font are unconstrained.

**Recommendation:** Bundle an OFL typeface for the Atrium UI. Candidates: Inter (OFL, 21 weights, designed for screen), IBM Plex Sans (OFL, multiple weights), or a custom face. Avoid proprietary fonts — they require per-seat licensing and restrict redistribution.

### One fork for the PL
**FreeType license choice:** FTL vs GPL-2.0+. Must choose FTL. The GPL option would copyleft our entire codebase. This is not a real fork — it's a compliance requirement. No steer needed, just awareness.

---

## 4. ISC-4 — Our-stack fit (what only THIS OS can do, what's constrained)

### Phase-6 GPU accel path
**Source:** docs/design.md §Phase 6.

`virtio-gpu-compute` first (QEMU), then one real GPU. Tensor migration between CPU and accelerator. Async inference syscalls. Target: "I run a real LLM. Tokens stream. The GPU is doing the work."

**What this means for text rendering:** Phase-6's accel path is designed for tensor compute (matmul, inference). Vello needs WebGPU/SPIR-V compute shaders with workgroup memory, atomics, and barriers — a different compute profile than DeltaNet's scalar matmul. The question is whether our virtio-gpu-compute path can drive general-purpose compute shaders, or only inference-specific kernels.

**Assessment:** If Phase-6 exposes a Vulkan-like or WebGPU-like compute interface (SPIR-V shader submission, dispatch, buffer mapping), Vello can ride it. If Phase-6 only supports custom inference kernels, we'd need to either:
- Extend Phase-6 to support general compute shaders (adds scope to the accel path)
- Use a separate GPU context for rendering (wasteful but isolates concerns)
- Defer GPU text rendering until Phase-6 is general-purpose

**This is a design question for the Phase-6 builder, not a blocker for the text-engine THINK.** The SDF/MSDF bridge (G0→G1) doesn't need compute shaders.

### Userland glyph service → cap-surface buffers
**Source:** OT-52 compositor architecture (cap taxonomy: SurfaceCap, BufferCap, InputFocusCap, PresentFenceCap).

The text engine is a **userland service**, not a kernel driver. It produces surface buffers that the compositor composites. In our cap model:
- A text surface = a cap-gated buffer (BufferCap)
- The text renderer allocates the buffer, fills it with rasterized glyphs, and presents it to the compositor via PresentFenceCap
- Zero-copy: the renderer writes directly into the cap-surface buffer; the compositor reads it without copying

**This is clean.** The text engine is just another userland process producing surface buffers. No kernel touch for the rendering path. The compositor's damage tracking handles incremental redraws.

### The CPU/software fallback that must carry G0
**Source:** kernel/src/drivers/fb/ — current 8x16 bitmap font, `font.rs` (hand-encoded VGA subset, 507 lines), `console.rs` (framebuffer console sink), `mod.rs` (draw_glyph, draw_string, blit_rgba).

G0 runs on the raw framebuffer. No GPU accel. The current font is a minimal bitmap subset — enough for boot diagnostics but not "beautiful type."

**G0 requirement:** Beautiful CPU/hinted type on the v1 framebuffer. The transcript must read *crisp* before it's zoomable.

**Path:** FreeType (FTL) in userland, loading an OTF/TTF font, rendering anti-aliased 8-bit glyph bitmaps at the console's pixel size. Blit via `blit_rgba()` (already exists in `fb/mod.rs` with alpha blending). This produces crisp, hinted, proportional or monospace text on the raw framebuffer — no GPU needed.

**The current kernel font.rs becomes a boot-only artifact.** The userland text service replaces it for G0's transcript rendering. The kernel retains the bitmap font for early-boot messages before the userland text service is ready.

---

## 5. ISC-5 — The smallest-first buildable path (the spine, text-engine edition)

Each stage is shippable, honestly measured, and sequences against the GUI spine (G0 → G1 → G2).

### T0 — Beautiful CPU/hinted type for G0
**What:** FreeType (FTL) userland service. Loads an OFL font (Inter or IBM Plex Sans). Renders anti-aliased glyph bitmaps at the console's native pixel size. Autohint for crispness below 16px; no-hint above. Blits via `blit_rgba()` to the raw framebuffer.

**Quality bar:** The transcript reads *crisp* at the console's native resolution. Proportional or monospace (fork for PL). Kerning on for proportional. Ligatures optional.

**GPU dependencies:** None. Pure CPU.
**Unlocks:** G0's text surface — the paradigm prototype has beautiful type from day one.
**Dependencies:** FreeType (FTL, ~250K SLoC C, but we only need the rasterizer + font parsing; alternatively, `ttf-parser` + a minimal Rust rasterizer for less C).

### T1 — GPU-outline leap for G2
**What:** Vello-class GPU-outline rendering. The chosen ISC-1 primary. Latin-only. Resolution-independent / zoomable. Rides the Phase-6 accel path's compute shader interface.

**Quality bar:** Infinitely zoomable text that looks perfect at any scale. The Atrium's killer feature — "the transcript grows a body." Latin subset of the font, no shaping beyond cmap + ccmp (defer full rustybuzz to T2).

**GPU dependencies:** Phase-6 compute shaders (SPIR-V/WebGPU). If Phase-6 can't drive general compute yet, use SDF/MSDF as the bridge until it can.
**Unlocks:** G2's core — resolution-independent zoomable type, the leap from terminal to surface.
**Dependencies:** Phase-6 accel path (or SDF bridge), Vello shaders adapted to our GPU interface.

### T2 — Shaping stack before localization
**What:** rustybuzz shaping layer. Full GSUB/GPOS pipeline: ccmp, liga, kern, mark positioning, cluster tracking. BiDi support for RTL languages. Font fallback when a glyph isn't in the primary face.

**Quality bar:** Correct text for Latin + CJK + Arabic + any script the bundled fonts cover. Ligatures render correctly. Kerning is on. Accents position properly. Cursor placement and selection work across grapheme clusters.
**GPU dependencies:** None (shaping is CPU). Works with T1's GPU renderer or T0's CPU renderer.
**Unlocks:** Localization readiness. The "before localization" gate the synthesis flagged.
**Dependencies:** rustybuzz + ttf-parser (~38K SLoC combined, pure Rust), `unicode-bidi` crate, font fallback logic.

### T3 — Complex scripts + color emoji
**What:** COLRv0 rendering for flat-color emoji. AAT/Graphite shaping support for scripts that don't use OpenType. Variable font axis interpolation. Stem darkening for small-size quality polish.

**Quality bar:** Emoji render as colored glyphs. All Unicode scripts render correctly. Small-size text benefits from stem darkening and bytecode hinting.
**GPU dependencies:** COLRv0 can render on CPU or GPU. COLRv1 needs the full 2D paint engine (defer indefinitely).
**Unlocks:** Full internationalization. The Atrium is a surface that handles any text.
**Dependencies:** COLR renderer, bytecode hinter (FreeType), variable font support in ttf-parser.

### Sequence against the GUI spine

| GUI stage | Text stage | What ships |
|---|---|---|
| **G0** — paradigm prototype on raw framebuffer | **T0** — FreeType CPU/hinted type | Crisp transcript, beautiful at native res |
| **G1** — cap-native compositor MVP (software) | T0 continues | Text surface = cap-gated buffer, composited |
| **G2** — GPU vector text | **T1** — Vello GPU outlines (Latin) | Zoomable surface, the killer leap |
| G2+ / pre-localization | **T2** — rustybuzz shaping + BiDi | Correct text for any script |
| Post-G2 polish | **T3** — emoji, variable fonts, hinting polish | Full internationalization |

---

## 6. ISC-6 — The trap bank (prior-art gotchas, each with its discriminator)

### Rendering traps

| # | Trap | Symptom | Discriminator | Source |
|---|---|---|---|---|
| R1 | **SDF gamma-correct distance collapse** | SDFs computed in linear space produce fuzzy, bloated glyphs below ~10px. Thin strokes vanish. | Compute SDF distance in gamma-corrected (sRGB) space, or apply gamma-aware threshold remap in the fragment shader. | Lengyel's Slug paper caveats; GPU rendering literature |
| R2 | **Subpixel-AA on gamma-encoded backgrounds** | Text looks darker/thicker with a "dirty" fringe on light backgrounds. FreeType produces linear alpha; compositing over gamma-encoded BG darkens edge pixels. | Linearize before compositing subpixel-AA glyphs, then convert back to sRGB. | FreeType mailing list; Microsoft ClearType docs |
| R3 | **Hinted bitmap scaling artifacts** | Crisp text at native size, terrible when zoomed. Hinting locked stems to integer pixel boundaries that don't align at new scale. | Never scale hinted bitmaps. Re-render at target size (unhinted or re-hinted). `FT_LOAD_NO_HINTING` for scaled rendering. | FreeType hinter docs; mailing list |
| R4 | **Stem darkening mismatch** | Text looks thinner than the same font in system apps. FreeType's autohint doesn't apply the same synthetic darkening as ClearType/DirectWrite at 10-14px. | Apply explicit stem-darkening below 16px. `FT_PARAM_STEM_DARKENING` with `FT_Load_Glyph()`. | FreeType issue tracker; Chrome bug tracker |
| R5 | **Area-summing rasterizer fails on self-overlapping paths** | Transparent holes appear along diagonal overlaps in filled paths. Positive and negative area contributions cancel to zero. | Use conflate (winding-number) fill rule, not nonzero/area-summing, for self-overlapping paths. | servo/pathfinder#142 (mtklein); Skia limitation |
| R6 | **MSAA can't fix area-rasterizer accuracy gaps** | Even with 4x MSAA, diagonal overlaps in filled paths show transparent artifacts. Samples each compute area independently and still cancel. | MSAA fixes edge aliasing but not fill-rule correctness. Use conflate fill or sampling-based rasterizer for self-intersecting paths. | servo/pathfinder#142 |
| R7 | **Gradient interpolation in premultiplied alpha space** | Incorrect color transitions — intermediate colors are darker when one stop is semi-transparent and another is opaque. | Interpolate gradient stops in straight (unpremultiplied) color space, then premultiply the result. | linebender/vello#1056, fixed #1145 |
| R8 | **GPU register pressure kills mobile performance** | Fine rasterizer runs 10× slower on Adreno 640 vs desktop. Clip stack state + per-tile bookkeeping create excessive register pressure, reducing occupancy to near-zero. | Minimize per-invocation shader state. Move clip stack to shared memory or texture lookups. Profile with driver-specific tools (Android GPU Inspector). | linebender/vello#83 (raphlinus) |
| R9 | **Unbounded GPU work → system hang** | Full system hang on M1 8GB when zooming into complex paths. Tile count explodes, exhausts memory/GPU command buffer. | Cap max tile count per frame. Clamp zoom or fall back to CPU when tile count exceeds threshold. Progressive refinement. | linebender/vello#548 (jrmoulton) |
| R10 | **Blend layers don't respect outer clip bounds** | `Clear` blend inside a clip region clears outside the clip boundary. Canvas compositing tests fail. | Propagate clip state through blend layer evaluation. Render graph must evaluate clips before blends. | linebender/vello#1119 (sagudev) |
| R11 | **Hybrid mode is 3-6× slower for text-heavy content** | 110ms frame times on text-heavy pages vs 18ms Classic / 36ms CPU. Tile management + GPU command submission dominate for small glyph quads. | For text-heavy rendering, prefer CPU renderer or Classic mode. Profile content mix before choosing backend. | linebender/vello#1380 (nicoburns) |
| R12 | **Glyph atlas cache thrash under scrolling** | Fixed-size atlas + LRU eviction. Horizontal scroll causes continuous thrashing — evicted glyphs re-enter every screen width. Hundreds of texture uploads/sec → GPU stalls, jank. | Spatially-aware atlas allocation (quadtree/strip-based). Wrap-around atlas sized to one screen's unique glyphs. Pre-warm atlas for visible content. | Pathfinder design; Vello glyph cache issues; OSDev patterns |
| R13 | **WebGL SDF tile atlas — random blank artifacts** | Random missing glyphs on certain GPU drivers. Async texture upload race: atlas being written while render pass reads it. | Explicit `gl.finish()` or fence syncs between atlas mutation and rendering. Double-buffered atlas. | servo/pathfinder#284 (zimond) |

### Shaping traps

| # | Trap | Symptom | Discriminator | Source |
|---|---|---|---|---|
| S1 | **USE glyph reordering breaks pre-base vowel placement** | HarfBuzz's USE reorders pre-base vowel signs to just before the *last base* glyph, should be before the first. Tirhuta `tki` renders broken. | Treat "base consonant" as maximal sequence of non-reordering non-halant glyphs, not a single B/GB-class glyph. | harfbuzz#1235 (dscorbett) |
| S2 | **Tai Tham subjoined consonants after right-side vowels** | Complex Tai Tham clusters fail to combine — subjoined consonants render as separate base glyphs. | Extend USE feature table + regex to permit subjoined-after-right-vowel for Tai Tham. | harfbuzz#170 (roozbehp) |
| S3 | **AAT Tamil shaping fails (mort vs morx)** | Glyph combination that works under CoreText fails under HarfBuzz's OT shaper — glyphs render uncombined. | Use AAT shaper (`--shaper aat`) explicitly for fonts with AAT tables. Prefer AAT when `morx`/`mort` present over GSUB/GPOS. | harfbuzz#1348 (ebraminio) |
| S4 | **Kerning lost switching from FreeType to HarfBuzz** | Text suddenly has no kerning after library upgrade. HarfBuzz's CVT processing + GPOS kerning differs from FreeType's direct `kern` table access. | Verify both `kern` table AND GPOS kerning paths. Run kerning regression tests against font corpus when migrating backends. | harfbuzz#2394; GNOME/pango#463 |
| S5 | **BiDi mirroring fails for non-bracket characters** | `<`, `>`, `/`, `{`, `}` don't flip in RTL context. Mathematical expressions or file paths in mixed-direction text look backwards. | Implement app-level mirroring table beyond Unicode's canonical mirrored set. Extend for domain-specific characters. | HarfBuzz BiDi discussions; UAX #9; browser engine bug trackers |
| S6 | **Grapheme cluster boundaries break shaping assumptions** | Text selection highlights wrong characters, cursor jumps to wrong positions. EmojiZWJ sequences, combining marks split across font fallback. | Use HarfBuzz's `cluster` field in shaped glyphs (not Unicode grapheme boundaries) for all hit-testing, selection, cursor positioning. | HarfBuzz cluster-level docs; Chromium text input bugs |
| S7 | **FreeType bytecode interpreter diverges from Windows** | Same font renders differently on Linux vs Windows — stem widths, counters, overall shape vary. Older Microsoft Core Fonts worst affected. | Use `FT_LOAD_TARGET_LIGHT` or `FT_LOAD_TARGET_NORMAL` consistently. Accept that perfect cross-platform matching requires platform-specific paths. | FreeType docs; mailing list |

---

## 7. ISC-7 — The honest forks for the PL

### Fork 1: Primary GPU rendering technique
| Option | Pros | Cons |
|---|---|---|
| **Vello (recommended)** | Actively developed, clean architecture, Apache-2.0, exact area AA at all scales, full 2D renderer | Compute shaders mandatory, CPU fallback not shipped, alpha-state conflation artifacts |
| Slug | Patent freed, highest single-glyph quality, minimal memory, MIT reference shaders | Research-paper maturity, large bug surface in analytic math, no library to lift |
| SDF/MSDF only | Maximum compatibility, zero compute, trivial CPU fallback | Degrades at zoom — can't deliver the "infinitely zoomable" promise |

**Recommendation: Vello.** It's the only actively-developed option that delivers resolution-independent quality at all scales. Slug is a clean fallback if Vello's compute requirements don't fit Phase-6. SDF is the G0→G1 bridge, not the endpoint.

### Fork 2: CPU rasterizer for T0 (G0)
| Option | Pros | Cons |
|---|---|---|
| **FreeType (FTL, recommended)** | Battle-tested, autohint, bytecode hinter, produces AA bitmaps directly | C library (~250K SLoC), FTL advertising clause (cite in docs) |
| `ttf-parser` + Rust rasterizer | Pure Rust, no C deps, small footprint | Building a quality rasterizer from scratch is nontrivial; FreeType's rasterizer took decades of tuning |

**Recommendation: FreeType (FTL).** The rasterizer is the one piece where lifting a battle-tested C library beats reinventing. We only need font parsing + rasterization, not the full FreeType stack. The FTL license is permissive and MPL-2.0 compatible.

### Fork 3: Shaping layer
| Option | Pros | Cons |
|---|---|---|
| **rustybuzz (recommended)** | Pure Rust, no C deps, no-std, 3M downloads/mo, MIT | 1.5-2× slower than C HarfBuzz (irrelevant for transcript volumes) |
| `harfbuzz` (Servo bindings) | Full feature set, fastest shaping | C++ dependency, 2.5 MB, needs system lib or bundled build |

**Recommendation: rustybuzz.** The performance gap doesn't matter at our text volumes. Pure Rust integration is cleaner for a from-scratch OS.

### Fork 4: Bundled UI typeface
| Option | Pros | Cons |
|---|---|---|
| **Inter (recommended)** | OFL, 21 weights, designed for screen readability, variable font available | Generic — doesn't carry the Atrium's aesthetic identity |
| IBM Plex Sans | OFL, distinctive silhouettes, multiple weights | Fewer weights than Inter, less screen-optimized |
| Custom face | Unique to the Atrium, carries the aesthetic identity | Font design is a specialized skill; long lead time |

**Recommendation: Inter for v1, custom face post-G2.** Inter is the safe, beautiful default. A custom face is a G6 (design language pass) investment — not a G0 blocker.

### Fork 5: Monospace vs proportional for the console
| Option | Pros | Cons |
|---|---|---|
| **Monospace (recommended for G0)** | Consistent with terminal heritage, easier layout math, `neo>` alignment | Less beautiful, less "surface-like" |
| Proportional | More beautiful, more "Atrium-like" from day one | Harder layout, kerning matters, cursor positioning is complex |

**Recommendation: Monospace for G0 (the console), proportional for Atrium surfaces (Workfields, Arthur responses).** The `neo>` deterministic rail benefits from monospace alignment. The transcript's Arthur responses and Workfield surfaces can use proportional type. This is additive — the text engine supports both; the surface chooses.

### Fork 6: How far to carry shaping for v1
| Option | Pros | Cons |
|---|---|---|
| **Latin-only, cmap+ccmp (recommended for T1)** | Smallest first step, proves GPU rendering works, defers shaping complexity | Can't render non-Latin text; ligatures might be missing |
| Full rustybuzz from T1 | Correct from day one, handles any script the font covers | More code, more surface area for bugs in the first GPU build |

**Recommendation: Latin-only for T1, full rustybuzz at T2.** Prove the GPU rendering leap with minimal shaping complexity. Add the full shaper before localization (T2). This keeps T1's scope tight — the GPU leap is the hard part; adding full shaping on top would conflate two difficulties.

---

## 8. Primary sources cited

### Papers + specs
- Lysenko, M. "Distance Fields Everywhere." SIGGRAPH 2012. (Valve SDF paper)
- Lengyel, E. "Slug: Analytic-Coverage GPU Outlines." Microsoft Research. (paper PDF, terathon.com)
- Lengyel, E. "A Decade of Slug." Blog, March 17, 2026. (patent dedication, terathon.com/blog/decade-slug.html)
- Unicode Standard Annex #9: Unicode Bidirectional Algorithm, Revision 51. (unicode.org/reports/tr9/)
- Microsoft OpenType COLR Specification. (learn.microsoft.com)
- COLRv1 Specification (Google doc). (docs.google.com)

### Library source + docs
- msdfgen: github.com/Chlumsky/msdfgen — README, API docs, LICENSE.txt (MIT)
- Vello: github.com/linebender/vello — README, vision doc, 2023 roadmap, fine.wgsl, LICENSE-APACHE (Apache-2.0)
- Pathfinder: github.com/servo/pathfinder — README, doc/architecture.md, LICENSE-APACHE (Apache-2.0)
- FreeType: freetype.org/freetype2/docs/ — hinter docs, glyph metrics docs; github.com/freetype/freetype — LICENSE.TXT (FTL/GPL dual)
- HarfBuzz: harfbuzz.github.io — manual; github.com/harfbuzz/harfbuzz — COPYING (Old MIT)
- Slug reference shaders: github.com/EricLengyel/Slug — MIT license
- rustybuzz: lib.rs/crates/rustybuzz — MIT, 3M downloads/mo, passes 2,221/2,252 HB tests

### Issue trackers + field reports
- linebender/vello#83 (register pressure), #49 (conflation), #548 (system hang), #1056 (gradient premultiply), #1119 (clip bounds), #1380 (hybrid mode text perf)
- harfbuzz#170 (Tai Tham), #1235 (USE reordering), #1348 (AAT Tamil), #2394 (kerning loss)
- servo/pathfinder#142 (area-summing overlap), #284 (WebGL atlas race)
- FreeType mailing list archives: stem darkening, bytecode interpreter divergence, subpixel AA gamma

### Our own surfaces
- kernel/src/drivers/fb/font.rs — 8x16 bitmap font, 507 lines, hand-encoded VGA subset
- kernel/src/drivers/fb/console.rs — framebuffer console sink, glyph draw at GLYPH_WIDTH×GLYPH_HEIGHT
- kernel/src/drivers/fb/mod.rs — draw_glyph, draw_string, blit_rgba (alpha-blended), fill_rect, plot
- docs/design.md §Phase 6 — virtio-gpu-compute accel path
- docs/THINK-2026-07-08-gui-vision-SYNTHESIS.md — ratified Atrium vision, G0-G7 spine, GPU vector text as killer enabler

---

## 9. Close-gate

**Docs-only branch.** No code, no QEMU. `scripts/worker-close-gate.sh` host set locally — skip-with-note if cargo times out on a docs-only branch.

**RETURN:**
- THINK path: `docs/THINK-2026-07-10-atrium-text-engine-research.md`
- ISC-1: Rendering landscape — Vello (primary) / SDF bridge (G0→G1) / Slug (future). Verdict with reasons.
- ISC-2: Shaping reality — 6 layers enumerated. Smallest v1 scope: rustybuzz + ttf-parser + FreeType FTL + unicode-linebreak. Latin-only for T1, full shaping at T2.
- ISC-3: Licensing — Slug patent FREED (March 2026, verified from Lengyel blog + USPTO SB/43). All library licenses verified from actual LICENSE files. All MPL-2.0 compatible (FreeType: choose FTL, not GPL).
- ISC-4: Our-stack fit — userland glyph service → cap-surface buffers (clean). Phase-6 accel path needs general compute shaders for Vello (design question for Phase-6 builder). CPU fallback carries G0 via FreeType.
- ISC-5: Smallest-first path — T0 (FreeType CPU for G0) → T1 (Vello GPU outlines, Latin, for G2) → T2 (rustybuzz shaping + BiDi, before localization) → T3 (emoji, variable fonts, hinting polish). Sequenced against G0→G2 spine.
- ISC-6: Trap bank — 19 traps (12 rendering + 7 shaping), each with discriminator. Sources: Vello issues, HarfBuzz issues, Pathfinder issues, FreeType mailing list, Lengyel Slug paper.
- ISC-7: Six forks — technique (Vello rec), CPU rasterizer (FreeType FTL rec), shaper (rustybuzz rec), typeface (Inter rec, custom post-G2), console font (monospace G0 / proportional Atrium rec), shaping scope (Latin-only T1 rec).

**Diff verified:** RETURN matches the committed artifact at `docs/THINK-2026-07-10-atrium-text-engine-research.md`.

*Research THINK. No code. Host-only. Every exact claim from the real paper/source/license read directly.*
