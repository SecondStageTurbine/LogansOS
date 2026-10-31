<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-077: GPU P1 — the minimal cap-native Gen9 compute service + one project-authored glass-blur kernel

| | |
|---|---|
| **Status** | **v2 — Accepted (RATIFY-AFTER-AMENDMENT applied).** Chad's cross-model cross-check (`docs/RESEARCH-2026-07-21-gpu-p1-adr-crosscheck-FINDINGS.md`) ratified the architectural cut and required 7 bounded amendments before Slice 1; **this v2 folds all 7** (§12 changelog). Slice 1 (the CPU blur floor) builds against this document. |
| **Date** | 2026-07-21 (v1); **v2 2026-07-21** (folds the cross-check amendments) |
| **Author** | Cerebro (main-tier; Claude family; OT-63 GPU owner). v2 amendments per Chad (Codex; cross-model). |
| **Dispatch** | `Plans/DISPATCH-2026-07-21-gpu-p1-compute-service-build-adr-cerebro.md` |
| **Synthesizes** | `docs/RESEARCH-2026-07-20-gpu-graphics-compute-api-design-FINDINGS.md` (Prometheus/Qwen) **as amended by** `docs/RESEARCH-2026-07-20-gpu-graphics-api-crosscheck-FINDINGS.md` (Chad/Codex — the amendment is load-bearing) |
| **Builds on** | ADR-032 (Phase-6 Gen9 compute driver + the proven `ocloc`/IGC path), ADR-045 (fenced DMA arena = I-14(c1)), ADR-050 (VT-d/IOMMU = I-14(c2)), ADR-076 (arena-backed scanout + present-epoch fence), ADR-024 (the frosted-glass design direction) |
| **Ledger** | Preserves I-1…I-16. Adds **no** new cross-cutting invariant. Names one new *sequencing obligation* (the graphics VT-d narrowing precedes any userland compute queue) that is discharged by an existing planned build (OT-63 Slice 5), not a new invariant. |
| **Related** | OT-63 (the GPU the glass needs), `GpuSubmitCap` (Slice-2, merged), Janus's GFX ladder (`docs/THINK-2026-07-12-gpu-accel-path-design-janus.md`) |

---

## 0. Decision

Build **P1**: a minimal, **manifest-only, cap-native Gen9 compute service** layered on the Phase-6 proven substrate, proven end-to-end by **one project-authored kernel** — a **separable Gaussian backdrop blur over a linear XRGB8888 pixel buffer** (the glass effect the Atrium's frosted-glass material needs, ADR-024). The kernel is hand-authored in **OpenCL C** and offline-compiled through the **already-proven `ocloc`/IGC finalizer** (ADR-032 D4: `OpenCL C → pinned ocloc → device binary + zeinfo → extracted EU ABI → in-kernel pinned blob`). **No shader compiler is built. No WGSL. No Naga. No wgpu. No Vello.**

Three decisions carry the design:

1. **The blur is exact fixed-point integer arithmetic, not float.** This is what makes the kill-switch falsifiable: *compute-service-disabled ≡ the CPU blur floor, bit-for-bit* — and the GPU kernel ≡ the CPU floor, bit-for-bit — are only *provable equalities* if the arithmetic is integer. Float would make "bit-for-bit" a lie (GPU/CPU rounding differ). Integer also makes the independent hand-computed byte oracle exact. This mirrors the in-tree `fb::blit_rgba` alpha path, which is already integer `(v*a + old*inv_a + 127)/255`.

2. **The physical DMA fence for the iGPU precedes any *userland* compute queue** (Chad's load-bearing catch, §3). The iGPU is today in the VT-d **identity domain** (GPA==HPA — *not* a fence; ADR-050, ADR-076). A manifest-only **kernel-trusted** blur service may run *before* that narrowing as an explicitly risk-accepted bring-up stage (c1 contains our own programming), but **granting a userland compute queue, or calling I-14(c) *closed* for the compute path, requires the graphics VT-d narrowing** — which is the already-planned **OT-63 Slice 5**.

3. **P1 deliberately touches none of the hard Gen9 compute shapes** (no SLM, no workgroup barriers, no atomics, no storage images, no surface state, no samplers). A linear-buffer separable blur can avoid all of them, so P1's *new* silicon surface over Phase-6 is small and enumerable (§4) — exactly the opposite of Vello's shader closure, which needs all of them (Chad §3).

### 0.1 What this ADR is explicitly NOT (the P2 boundary)

Vello / WGSL / Naga-SPIR-V / wgpu are **out of P1** and are **not on its critical path**. The "Vello backdrop-blur is a short hop" claim in the reviewed research **does not survive** Chad's cross-check: there is no demonstrated WGSL/SPIR-V→Gen9 executable path, Vello's `backdrop.wgsl` is a winding-number prefix-sum (not a Gaussian backdrop blur), and Vello's public renderer is wgpu-specific. (The reviewed research quoted that hop in person-months; per the standing no-human-labor-time rule the figure is not passed through — the real bound is **dependency depth**: three unmet closure spikes gate it, listed below.) Those are a **separate P2 project** — a compiler/backend-closure spike that must, *before* Vello is allowed onto any committed schedule, demonstrate: (a) an offline WGSL/SPIR-V→Gen9 artifact with pinned source/compiler/flags/disassembly; (b) execution of Vello's actual `Recording` closure without wgpu; and (c) a byte-or-image oracle on the real UHD 630 for a representative shader using workgroup memory, barriers, atomics, and image output. **P2 is named here only to fence it out of P1.** This ADR designs P1 only.

---

## 1. The exact glass effect + kernel (replacement gate 1 — effect closure)

Chad's gate 1: *name the precise effect and the exact kernel — not Vello's winding backdrop.* Here it is, closed.

### 1.1 The effect

The Atrium's frosted-glass material (ADR-024 amendment, 2026-07-19) is the classic-Mac `NSVisualEffectView` idiom: **capture backdrop → downsample → blur → vibrancy tint → composite.** P1 proves the **blur** stage — the one GPU-gated, compute-bound step. The other stages are either CPU-cheap (tint, composite) or an optional pre-pass (downsample) and are *not* what P1 must prove on silicon.

Concretely, when the compositor is about to draw a glass surface:

1. **Backdrop capture (damage = radius-expanded output + source halo — v2 amendment 3).** The already-composited pixels under the glass surface's bounding rect are copied into an **arena-backed source buffer** (`DmaBuffer::alloc_in_arena`, XRGB8888 LE, matching `SharedPixelBuffer`'s format). A blur of radius `r` **spreads**: a changed source pixel affects output pixels up to `r` away, and each affected output pixel itself gathers taps `±r`. So the damage model is **not** "blur `surface_bounds ∩ DamageRectV1`" — it is:
   - **committed output damage = the changed rect expanded by `r`** (`blur::expand_output_rect`), clamped to the surface;
   - **source read = the changed rect expanded by `2r`** — the "source halo" (`blur::source_halo_rect`), clamped to the surface, since recomputing the `⊕r` output needs a further `±r` of taps.

   Backdrop damage **behind** a glass surface invalidates that surface even if the glass client submitted no damage (a scene-level rule for the compositor slice; the halo math itself is pinned + host-tested in `blur.rs` now). **There is no occlusion model in-tree** (confirmed: damage is a single validated rect). **Slice 1's CPU floor blurs the full surface on damage** — the conservative, correct case, whose read is a *superset* of the `2r` halo — and commits the radius-expanded rect; the bounded halo-only incremental recompute is the named later optimization. Occlusion-culling is likewise explicitly out of P1.
2. **Blur (the P1 kernel).** A **separable Gaussian blur**, two passes (§1.3), source → intermediate → destination, all arena-backed linear buffers.
3. **Vibrancy tint + composite.** Trust-state tint (ADR-024 §2) and the composite into the `SharedPixelBuffer` are the existing integer paths (`fb::with_present`/`copy_row_px`), unchanged. Not P1-silicon work.

### 1.2 Pixel format, radius, edge rules (all pinned for P1)

| Property | P1 value | Why pinned |
|---|---|---|
| **Format** | XRGB8888 LE (native fb word). Blur operates on R,G,B independently; the X byte is read-ignored, written `0xFF`. | Matches `SharedPixelBuffer`; no format conversion, no surface state. Translucency is the *tint* stage's job, not the blur's. |
| **Radius** | Fixed `r = 8` → a 17-tap 1-D kernel per pass (manifest constant). | One kernel, one radius = one thing to prove. Variable radius is later. |
| **Weights** | **Q16 fixed-point** Gaussian, `Σ w[k] = 65536` exactly (the discretization forces the sum to unity by adjusting the center tap). σ derived from r (`σ = r/2 = 4.0`), weights computed **offline** and checked in as a `const [i32; 17]` with the generator script + its output pinned (provenance, same discipline as the EU blob). | Integer weights summing to exactly 65536 → exact, CPU/GPU-identical, oracle-able. |
| **Accumulation** | per channel: 32-bit signed accumulator, `acc += w[k] * src_channel`; result `= (acc + 32768) >> 16`, then clamp to `[0, 255]`. | Exact, deterministic rounding (round-half-up), no float. `2r+1` taps × 255 × 65536 fits in i32 with margin. |
| **Edge** | **clamp-to-edge**: out-of-bounds sample index is clamped to `[0, dim-1]` (border pixels replicate). | Deterministic, no branchy border kernels, oracle-able at edges. |

### 1.3 The two-pass separable structure (the whole point of "separable")

- **Pass H (horizontal):** `mid[y][x] = clamp( ( Σ_{k=-r..r} w[k+r] · src[y][clampX(x+k)] + 32768 ) >> 16 )`, per channel. Reads `src`, writes `mid` (intermediate buffer).
- **Pass V (vertical):** `dst[y][x] = clamp( ( Σ_{k=-r..r} w[k+r] · mid[clampY(y+k)][x] + 32768 ) >> 16 )`, per channel. Reads `mid`, writes `dst`.

Separable = `2·(2r+1) = 34` taps/pixel instead of `(2r+1)² = 289`. **Pass V must observe Pass H's writes** — the inter-pass memory hazard is a real, load-bearing silicon concern (§4.3, §7).

### 1.4 Workgroup shape (designed to avoid new hardware shape)

- **Work-item = one output pixel.** Each item independently gathers its `2r+1` taps from the *linear source* via **stateless A64** (the exact access path Phase-6 proved — no surface state, no sampler). **No SLM, no shared-tile caching** in P1: an item re-reads overlapping taps rather than cooperating through workgroup memory. This trades bandwidth for *zero new silicon shape from SLM/barriers* — the correct trade for a first proof. SLM-tiling is a named later optimization.
- **Group shape:** `32 × 8` work-items (256/group) — one SIMD32 row × 8 rows. Global size = `(width, height)` rounded up to group multiples; **edge groups are partial**.
- **Explicit kernel bounds check (v2 amendment 2 — the memory-safety boundary).** A `GPGPU_WALKER` execution mask is an **ABI/SIMD** concern, **not** a per-workgroup rectangle that can independently suppress every right-edge lane / bottom-edge row — Intel's model launches one work-item per grid point grouped into *equal-sized* workgroups. So the kernel MUST begin, **before computing or dereferencing any source/intermediate/destination address**, with an unconditional guard:
  ```c
  uint x = get_global_id(0);
  uint y = get_global_id(1);
  if (x >= width || y >= height) return;
  ```
  Clamp-to-edge protects tap **reads** for a valid output pixel; it does **not** protect an out-of-range output **write** or a padded work-item's row-base arithmetic — only the explicit `x/y` guard does. The **CPU floor carries the identical discipline**: `blur.rs`'s `validate_blur` is the single up-front gate (dims, stride, weight length, buffer lengths — the checked `(height-1)·stride + width·4` bound), after which every index is provably in range. The same rule holds for rounded/partial edge groups on the GPU.
- **Dispatch geometry:** 2-D `GPGPU_WALKER` (X = ⌈width/32⌉ groups, Y = ⌈height/8⌉ groups). Phase-6 was effectively 1-D/single-group; **2-D geometry is new shape** (§4).

### 1.5 The independent byte oracle — the literal 8×8 fixture (v2 amendment 1)

Chad's load-bearing catch: v1 *named* an oracle but supplied no bytes, so it was un-recomputable — an intention, not a fixture. v2 ships the literal fixture. It is asserted independent of *both* the CPU floor and the GPU kernel (round-trip alone is insufficient — a shared-spec writer+reader agree even when both are wrong, the ADR-028 `.neom` lesson).

**Parameters.** `r=1`, Q16 weights `w = [16384, 32768, 16384]` (Σ = 65536). **8×8 XRGB8888 LE, tight stride = 32 bytes.** **Byte order in memory is `BB GG RR FF`** — the little-endian u32 `0x??RRGGBB` lands as bytes `[B, G, R, X]`. The X byte is **read-ignored** and written `0xFF`.

**Arithmetic (per pass, per channel, after edge replication):** `out = clamp((w[0]·left + w[1]·center + w[2]·right + 32768) >> 16, 0, 255)`. With these weights that is exactly `round_half_up((left + 2·center + right) / 4)`.

**Provenance.** Computed by an **independent Python reference** (`tools/atrium-logic-tests/oracle_gen/blur_oracle.py`) implementing the integer formula above — *not* the Rust under test — then asserted byte-for-byte by both the host KATs (`tools/atrium-logic-tests`, `blur_tests`) and the kernel scenario (`atrium_glass_blur`). The Rust `blur.rs` and the Python reference **agree byte-for-byte** (two independent implementations). The acquisition rule holds: the bytes come from the integer formula, never a summarizing fetch. **The PM independently recomputes this fixture before merge** — that independent recomputation *is* the oracle's value.

**Coverage** (each required case is present and asserted):

| Required case (v2 amendment 1) | Where in the fixture |
|---|---|
| all four clamp edges + corners | the 8×8 border rows/cols; TL/TR/BL/BR corners replicate two borders each |
| **odd quarter-sums → round-half-up** (`+32768>>16`, not only divisible) | seeded in **B channel row y=3** (`src B[3] = [40,50,50,52,60,60,62,70]`): H `B[3][2]` L=50 C=50 R=52 → S=202 → `(202+2)/4 = 51 = 0x33` (a floor would give 50); H `B[3][5]` → S=242 → 61 = 0x3D. The discriminator between round-half-up and naive truncation. |
| unequal R/G/B (channel-order bugs) | R = horizontal ramp, G = vertical ramp, B = 2-D pattern — every pixel has distinct channels, so a BGR/RGB swap changes the bytes |
| nontrivial X input proven ignored → `0xFF` | src X = `0x40 + 3x + 5y` (nontrivial); mid + dst X all `0xFF`; a second KAT XORs every source X and asserts the blurred RGB is unchanged |
| 255 clamp | R tail hits 255 (right column); blurred R saturates to 254/255 near the right edge |
| **H separately asserted** (H and V cannot share a compensating error) | the H-intermediate is a separate literal fixture, asserted on its own before the V-pass |

**The literal fixture** (memory order `[B, G, R, X]`, one 8-pixel row per line; the authoritative copy lives in `tools/atrium-logic-tests/src/lib.rs::blur_tests`, generated by `tools/atrium-logic-tests/oracle_gen/blur_oracle.py`):

<details><summary>SOURCE (256 bytes) → H-INTERMEDIATE (256 bytes) → DEST (256 bytes)</summary>

```
SOURCE  row0: 1E 05 0A 40  1F 05 2D 43  21 05 50 46  24 05 82 49  28 05 B4 4C  2D 05 E6 4F  33 05 FC 52  3A 05 FF 55
SOURCE  row1: 35 23 0A 45  36 23 2D 48  38 23 50 4B  3B 23 82 4E  3F 23 B4 51  44 23 E6 54  4A 23 FC 57  51 23 FF 5A
SOURCE  row2: 4C 42 0A 4A  4D 42 2D 4D  4F 42 50 50  52 42 82 53  56 42 B4 56  5B 42 E6 59  61 42 FC 5C  68 42 FF 5F
SOURCE  row3: 28 78 0A 4F  32 78 2D 52  32 78 50 55  34 78 82 58  3C 78 B4 5B  3C 78 E6 5E  3E 78 FC 61  46 78 FF 64
SOURCE  row4: 7A AF 0A 54  7B AF 2D 57  7D AF 50 5A  80 AF 82 5D  84 AF B4 60  89 AF E6 63  8F AF FC 66  96 AF FF 69
SOURCE  row5: 91 CD 0A 59  92 CD 2D 5C  94 CD 50 5F  97 CD 82 62  9B CD B4 65  A0 CD E6 68  A6 CD FC 6B  AD CD FF 6E
SOURCE  row6: A8 F0 0A 5E  A9 F0 2D 61  AB F0 50 64  AE F0 82 67  B2 F0 B4 6A  B7 F0 E6 6D  BD F0 FC 70  C4 F0 FF 73
SOURCE  row7: BF FF 0A 63  C0 FF 2D 66  C2 FF 50 69  C5 FF 82 6C  C9 FF B4 6F  CE FF E6 72  D4 FF FC 75  DB FF FF 78

H-MID   row0: 1E 05 13 FF  1F 05 2D FF  21 05 54 FF  24 05 82 FF  28 05 B4 FF  2D 05 DF FF  33 05 F7 FF  38 05 FE FF
H-MID   row1: 35 23 13 FF  36 23 2D FF  38 23 54 FF  3B 23 82 FF  3F 23 B4 FF  44 23 DF FF  4A 23 F7 FF  4F 23 FE FF
H-MID   row2: 4C 42 13 FF  4D 42 2D FF  4F 42 54 FF  52 42 82 FF  56 42 B4 FF  5B 42 DF FF  61 42 F7 FF  66 42 FE FF
H-MID   row3: 2B 78 13 FF  30 78 2D FF  33 78 54 FF  36 78 82 FF  3A 78 B4 FF  3D 78 DF FF  40 78 F7 FF  44 78 FE FF
H-MID   row4: 7A AF 13 FF  7B AF 2D FF  7D AF 54 FF  80 AF 82 FF  84 AF B4 FF  89 AF DF FF  8F AF F7 FF  94 AF FE FF
H-MID   row5: 91 CD 13 FF  92 CD 2D FF  94 CD 54 FF  97 CD 82 FF  9B CD B4 FF  A0 CD DF FF  A6 CD F7 FF  AB CD FE FF
H-MID   row6: A8 F0 13 FF  A9 F0 2D FF  AB F0 54 FF  AE F0 82 FF  B2 F0 B4 FF  B7 F0 DF FF  BD F0 F7 FF  C2 F0 FE FF
H-MID   row7: BF FF 13 FF  C0 FF 2D FF  C2 FF 54 FF  C5 FF 82 FF  C9 FF B4 FF  CE FF DF FF  D4 FF F7 FF  D9 FF FE FF

DEST    row0: 24 0D 13 FF  25 0D 2D FF  27 0D 54 FF  2A 0D 82 FF  2E 0D B4 FF  33 0D DF FF  39 0D F7 FF  3E 0D FE FF
DEST    row1: 35 23 13 FF  36 23 2D FF  38 23 54 FF  3B 23 82 FF  3F 23 B4 FF  44 23 DF FF  4A 23 F7 FF  4F 23 FE FF
DEST    row2: 3E 48 13 FF  40 48 2D FF  42 48 54 FF  45 48 82 FF  49 48 B4 FF  4E 48 DF FF  53 48 F7 FF  58 48 FE FF
DEST    row3: 47 78 13 FF  4A 78 2D FF  4D 78 54 FF  50 78 82 FF  54 78 B4 FF  58 78 DF FF  5C 78 F7 FF  61 78 FE FF
DEST    row4: 6C A9 13 FF  6E A9 2D FF  70 A9 54 FF  73 A9 82 FF  77 A9 B4 FF  7C A9 DF FF  81 A9 F7 FF  86 A9 FE FF
DEST    row5: 91 CE 13 FF  92 CE 2D FF  94 CE 54 FF  97 CE 82 FF  9B CE B4 FF  A0 CE DF FF  A6 CE F7 FF  AB CE FE FF
DEST    row6: A8 EB 13 FF  A9 EB 2D FF  AB EB 54 FF  AE EB 82 FF  B2 EB B4 FF  B7 EB DF FF  BD EB F7 FF  C2 EB FE FF
DEST    row7: B9 FB 13 FF  BA FB 2D FF  BC FB 54 FF  BF FB 82 FF  C3 FB B4 FF  C8 FB DF FF  CE FB F7 FF  D3 FB FE FF
```

Worked spot-check (independently recomputable): H `R[0][0]` (top-left, clamp-left): left = src R clamped = 10, center = 10, right = 45 → `acc = 16384·10 + 32768·10 + 16384·45 = 1,228,800`; `(1,228,800 + 32768) >> 16 = 19 = 0x13` — matches H-MID row0 pixel0's R byte. V `G[0][0]` on the intermediate: up = 5 (clamp), center = 5, down = 35 → `(16384·5 + 32768·5 + 16384·35 + 32768) >> 16 = 13 = 0x0D` — matches DEST row0 pixel0's G byte.

</details>

The fixture is a required Slice-1 deliverable (shipped in this build) and both the CPU floor (now) and the GPU kernel (from Slice 3) are checked against it.

---

## 2. The minimal cap-native compute ABI (gate 2)

**Design principle: reuse, don't reinvent.** The Phase-6 substrate already provides the mechanics (alloc, submit, poll, wait, seqno completion); OT-63 already provides the capability *pattern* (`Handle::GpuSubmit` + a pure `authority_allows` toggle) and the *fence* (present-epoch `fetch_max`, deadline-bounded yield). P1's ABI is the **smallest cap-native skin** over those, sized so its userland-facing shape (Slice 5) is a scoping change, not a rewrite.

> **v2 amendment 4 — the pre-VT-d service must be WHOLLY kernel-internal.** Chad's catch: a `Handle::GpuCompute` + a service syscall **is already a userland compute queue**, regardless of whether the kernel is manifest-only — a user process that can invoke that syscall before the graphics VT-d fence (Slice 5) has a queue into an identity-domained iGPU. That contradicts the normative VT-d ordering (§3). So the re-sequencing is now **binding**: Slices 2–4 expose the blur as a **feature-gated kernel-internal call path ONLY** — no `Handle`, no syscall, no process-controlled dimensions/buffers. `Handle::GpuCompute` + its syscall + the three-cap userland split are **deferred behind Slice 5** (the VT-d narrowing). A kernel-internal caller needs a **feature gate, not a process capability**. The tables below therefore describe the *forward* (post-Slice-5) userland shape; the P1 realization column and §2.2 are strictly kernel-internal.

### 2.1 The forward three-cap model vs. the P1 realization

The dispatch asks for `GpuBufferCap`/`GpuQueueCap`/`GpuFenceCap` **or a justified smaller set**. P1 is **manifest-only and kernel-trusted** (no userland principal touches the GPU until Slice 5), so P1 *realizes* a smaller set by reusing existing types, while *defining* the three-cap surface as the Slice-5 target so the grant is later a scoping change:

| Forward cap (**post-Slice-5** userland shape) | Authority | **P1 realization (pre-Slice-5, kernel-internal ONLY)** |
|---|---|---|
| `GpuQueueCap` | Submit one manifest pipeline within byte/time quotas | **A feature-gated kernel-internal submit path** — NOT a `Handle` or syscall (v2 amendment 4). `GPU_COMPUTE_ENABLE` defaults off; there is **no process capability and no syscall** in P1. Any future `Handle::GpuCompute`-like authority remains a post-fence Slice-6 design target, not a type introduced here. |
| `GpuBufferCap` | A typed arena buffer with MAP/READ/WRITE | **Trusted kernel slices bound through `ComputeBindings`, staged into driver-owned `DmaBuffer::alloc_gpu` allocations.** Under Slice 5 narrowing, `alloc_gpu` routes every backing allocation through the graphics arena; no new user-visible buffer handle exists. |
| `GpuFenceCap` | Await/query one generation-tagged completion; no submit right | **Existing `DispatchToken { device, sequence }` + deadline-bounded `AccelDriver::wait`** (compute side) and the present-epoch `completed_epoch.fetch_max`/`fence_wait` (present side). No new fence primitive. |

**Justification for the smaller set:** with one iGPU, one manifest, one trusted holder, and no userland submitter, a per-buffer/per-fence *handle* type buys nothing in P1 — it would be dead cap-plumbing. The scarce submit right is the default-off kernel feature/runtime gate; P1 deliberately introduces no `Handle::GpuCompute`. The three-cap split becomes load-bearing only when a *userland* queue is granted — which §3 sequences behind Slice 5 anyway.

### 2.2 The service interface (kernel-internal, manifest-only — no syscall pre-Slice-5)

```
// Pre-Slice-5 (P1): a feature-gated KERNEL-INTERNAL function. No syscall, no
// Handle, no process-supplied dimensions/buffers (v2 amendment 4).
gpu_compute::blur_backdrop(
    driver: &Arc<dyn AccelDriver>,
    src:     &[u8],             // trusted kernel XRGB8888 bytes
    mid:     &mut [u8],         // H-only/two-pass intermediate readback
    dst:     &mut [u8],         // backend-owned on a successful fence
    dims:    BlurDims,          // checked w, h, stride, radius
    weights: &[i32],            // pinned Q16 table
) -> Result<DispatchToken, AccelError>
// Slice-6+ ONLY (after the VT-d fence): a future queue/buffer/fence authority
// and syscall wrapper may be added; P1 defines no userland compute handle.
```

The CPU-floor analogue that ships in **Slice 1** is exactly this shape, kernel-internal, with an explicit `radius: usize, weights: &[i32]` so the same entry is asserted against the r=1 oracle and driven with the r=8 table: `SharedPixelBuffer::blur_backdrop(&self, dst, radius, weights, changed) -> Result<(), i64>` (`kernel/src/atrium/buffer.rs`), gated by the runtime `GLASS_BLUR_ENABLE` kill-switch. No syscall; the compositor slice calls it internally.

- **Manifest-only:** the pipeline is a fixed, pinned set — exactly one entry in P1 (`BLUR_H_KERNEL_GEN9`, `BLUR_V_KERNEL_GEN9`, or one kernel invoked twice with an axis param). Userland cannot supply a kernel; there is no `create_pipeline(user_bytes)`. The kernel blob's hash is checked in the manifest.
- **Submission** uses the parallel, kernel-internal `AccelDriver::submit_compute_pipeline(&ComputePipeline, &mut ComputeBindings)` seam — never the syscall-reachable tensor `OpKind`. `accel-intel` stages the bindings, constructs a fresh 48-bit-PPGTT ELSP/LRC context, and submits the encoded batch; `accel-sim` mirrors the bound-buffer result only for QEMU wiring evidence.
- **Completion:** the primary batch executes H → the raw PRM-oracled `PIPE_CONTROL` → V untouched, then posts one final seqno. The service waits that token with a deadline. A separate H-only batch/token exists solely for the non-perturbing `mid` diagnostic. No park or completion IRQ is introduced; `AccelDriver::wait(timeout)` is the I-14(b) backstop.

### 2.3 What the kernel validates on every submit

The manifest geometry is non-zero, stride/buffer lengths cover every live pixel, radius/weight count match the pinned kernel, and this Slice-4 VA layout limits each operand to one mapped page. The driver allocates/stages every positive working-set object, maps only its fixed command/data VAs in the context PPGTT, verifies the kernel EOT and command/LRC byte oracles, and clears operand/weights GGTT aliases. No raw batch bytes or addresses come from a non-kernel caller — in P1 there is no non-kernel caller at all.

---

## 3. The I-14(c) boundary — the physical fence precedes any userland compute queue (gate 5 + the amendment)

This is Chad's load-bearing finding and the one the ADR must get exactly right.

### 3.1 The catch: PPGTT isolates *contexts*, not *host physical memory*

Phase-6 proved (BUILD-4X-3) that per-context PPGTT stops context B from addressing context A's tensor — a **real intra-GPU boundary**. But PPGTT is **not** the boundary against the *device* reaching arbitrary host physical memory. That second boundary is VT-d, and **the iGPU is today in the VT-d identity domain (GPA==HPA)** — every physical address maps to itself, which is *no fence at all* (ADR-050: identity domains for the proven GPU/NVMe/AHCI substrate; ADR-076: "moving the real Gen9 iGPU out of the identity VT-d domain into an arena-only translated domain … is Slice 5 bare-metal. No c2 hardware-containment claim is made here"). Prometheus's research contradicted itself here (executive table said "PPGTT+VT-d contains it"; §5.3 admitted the GPU is identity-domained). Chad is right; the amendment stands.

### 3.2 The two threats, the two guards, the sequencing rule

I-14(c) for the compute path splits exactly as ADR-045 split it for connectivity:

- **(c1) OUR errant programming** — a mis-computed blur buffer address. **Contained for this kernel-trusted manifest:** checked geometry/length/weight invariants, fixed kernel-selected GPU VAs, a fresh PPGTT containing only the staged command/data leaves, cleared operand/weights GGTT aliases, and complete second-page kernel mapping. This compute seam does **not** consume `FencedDmaAddr`; that earlier design aspiration is superseded by the actual PPGTT + graphics-arena construction. Under Slice 5 narrowing, every backing allocation comes from `alloc_gpu` in the named graphics arena. **Guards:** command/LRC byte oracles, logged roots/leaves/VAs, admit/dispatch/degrade counters, and the out-of-arena weights negative.
- **(c2) a MISBEHAVING device** — errant iGPU firmware/DMA engine that invents an address. **Not closed for the iGPU today** (identity-domained). No software check contains it; only the IOMMU does. Closing it = **narrowing the iGPU from the identity domain to an arena-only translated domain, with RMRR + integrated-GPU-aperture + active-scanout handling = OT-63 Slice 5.**
  > **v2 amendment 5 — the graphics domain must cover ALL GPU-reachable allocations, not just the 3 pixel buffers.** Chad's catch: a live Gen9 path allocates far more than `src`/`mid`/`dst` with `DmaBuffer::alloc_zeroed` — the **ring, LRC (logical ring context), HWSP, state, kernel blob, CURBE, seqno, and PPGTT page-table** buffers are all either GPU-reachable *or contain addresses the GPU consumes*. A fault reaches governance memory through the **context/ring structures**, not only the pixel data. So requiring `src/mid/dst` to use `alloc_in_arena` does **not** discharge (c1)/(c2). The current arena is also a *connectivity* arena, not a typed graphics arena. Slice 5 must therefore: **(i)** inventory and migrate **every** GPU-reachable allocation into a **named graphics arena/domain** (so granting the iGPU its working set does not expose NIC/USB DMA buffers); **(ii)** its VT-d domain cannot execute until every required page is mapped in that domain (or is an explicitly justified privileged mapping); **(iii)** the negative DMA test must probe an address **outside that complete mapping**. Until Slice 5, describe c1 as constraining *our-programmed* pixel addresses — **not** as making every *device-programmed* address a `FencedDmaAddr`.

**The sequencing rule (the amendment, made normative):**

> A **manifest-only, kernel-trusted** blur service (kernel-authored pinned kernel; no userland code ever executes on the GPU; c1 fully contained) **may** be brought up *before* Slice 5 as an **explicitly risk-accepted** stage — the risk being solely a hardware/firmware fault in the iGPU itself, the same residual (c2) risk every identity-domained substrate device (NVMe/AHCI) already carries today. But:
>
> **No userland compute queue may be granted, and I-14(c) may not be called *closed* for the compute path, until the graphics VT-d narrowing (OT-63 Slice 5) lands.** The build ladder (§8) places the userland-queue grant strictly after Slice 5.

This is not a new invariant — it is a **sequencing obligation** discharged by an already-planned build. The ledger's I-14 row is unchanged; P1 adds a note that the compute path's (c2) is closed at OT-63 Slice 5, exactly as ADR-076 already records.

### 3.3 Full governance fault-containment analysis (I-14 a/b/c) for the compute path

Required for every driver/connectivity-class dispatch; stated for the compute service:

- **(a) Lock isolation.** The compute submit/config path runs in **process context** (the trusted glass service's syscall/kernel call) and acquires **no** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`-reservation/`TABLE`). **P1 stays poll-only** — it adds *no new GPU interrupt* — so the IF-off ISR hazard class (CP-2/CP-3) is **N-A for P1's compute path**; completion is a deadline-bounded poll in process context. (The existing synthetic-vblank present ISR `on_present_vblank` already takes no governance lock; a real GPU compute-completion IRQ is a *later* slice and inherits the ADR-050/ADR-076 leaf-lock discipline — the `IrqSafeMutex<IntelGpuInner>` split Prometheus flagged is that later slice's obligation, not P1's.) **Guard: grep-assert + runtime assertion that `submit_compute`'s blur path holds no governance lock.**
- **(b) Liveness degradation.** Every compute poll/`wait` carries a deadline (`AccelDriver::wait(timeout)`). A hung blur retains the possibly live DMA working set, refuses future submit, fails the fence, and **falls back to the CPU blur floor** (§8 Slice 1); Slice 4 does not attempt an unproven engine reset. Governance scheduling, capability revocation, and the filesystem **never await GPU recovery**. The terminal observer-timeout scenario proves floor output plus quarantine.
- **(c) Memory isolation.** As §3.2: (c1) is the fixed-manifest bounds/PPGTT construction; (c2) is the Slice-5 graphics VT-d narrowing over every `alloc_gpu` working-set page, proven with a separate real-iGPU positive/negative run — and required before any userland compute queue.

---

## 4. The silicon-shape gate, scoped to P1's ONE kernel (gate 4)

Phase-6 proved simple 1-D, pinned-ABI, stateless kernels — but the honest record is narrow: `thread_count = 1`, `SIMD width` effectively **2 live lanes** (walker masks trimmed to `0x3`), **one thread-group**, effectively 1-D geometry (ADR-032 iters 6–8). P1's blur is deliberately simple, but it is **not** zero-delta. Here is exactly what is new, what is reused, and what is avoided.

### 4.1 Reused from Phase-6 (no new proof needed)
Stateless A64 linear-buffer access; GGTT/PPGTT mapping; ELSP/execlist submit; `PIPE_CONTROL` post-sync seqno completion; the `GPGPU_WALKER` indirect-data (NEO-convention) payload delivery; the pinned-blob provenance model.

### 4.2 New shape #1 — 2-D dispatch geometry + the 2-D local-id GRF payload
Phase-6 was 1-D/single-group; the blur is a 2-D `GPGPU_WALKER` over many groups. The **iter7 root cause** (ADR-032) was a GRF-layout mismatch: the kernel expects `r0` header + per-thread `local_id` GRFs + cross-thread GRFs, and if the walker generates the wrong per-thread `local_id` block the cross-thread pointers land in the wrong GRF and the EU reads garbage. **In 2-D this must be re-derived** (the `local_id.x`/`.y` block sizing changes). **Discriminator:** a `RING_FAULT` VA decode (already in-tree) plus a known-input readback — a wrong payload faults at a garbage VA or produces a shifted/garbage output.

### 4.3 New shape #2 — real SIMD32 occupancy across many thread-groups
Phase-6 never ran full-width: 2 live lanes, one group. The blur runs **full SIMD32** lanes across hundreds of groups (real occupancy). A bad execution mask or a wrong per-lane address computation shows as **periodic stripes/garbage** at a 32-pixel or group-boundary period. **Discriminator:** compare a known-blur output against the CPU-floor reference (§1.5) — a lane bug is spatially periodic and localizes immediately; edge groups (partial) are the highest-risk region (compare right/bottom edge rows against interior).

### 4.4 New shape #3 — the dependent two-pass memory hazard (inter-dispatch coherence)
Pass V must read Pass H's writes to `mid`. On Gen9 this needs a `PIPE_CONTROL` with the correct flush (DC-flush + CS-stall) between the two dispatches, plus the correct **MOCS** on both mappings.

> **v2 amendment 6 — a PRM-derived command-byte oracle, and a non-perturbing coherence diagnostic.** Chad's catch: **`MOCS` is a cacheability policy/index, not a barrier** — "correct MOCS" is *not* a sufficient executable spec; `PIPE_CONTROL` ordering + flush bits provide the transition. So **Slice 3 must transcribe the applicable Kaby Lake PRM requirements into a hand-laid command-byte oracle** (the byte-oracle discipline, applied to commands): the exact `PIPE_CONTROL` DWord(s) placed **between H and V** — the required CS-stall companion bits, the DC/L3-flush/visibility bits — **and** the exact MOCS value programmed for both the H-write and V-read mappings. Per the exact-bytes rule, the PRM bytes are pulled from **raw source** (the Intel PRM / cross-referenced against a permissively-licensed decoder's tables), **never `WebFetch`** (a summarizing fetch mangles exact bytes and poisons the oracle). Slice 4 proves it on silicon.
>
> **The diagnostic must not perturb the hazard it observes.** Do **not** read `mid` back on the CPU *between* H and V in the primary coherence test — that readback can wait-for-completion or introduce CPU cache maintenance and mask the very hazard. Use **two runs**: (a) an **H-only** run + readback to prove the H math; (b) an **untouched H→`PIPE_CONTROL`→V** run + final readback only. If H-only is correct but the untouched two-pass `dst` is stale/non-deterministic, the coherence discriminator is conclusive.

**Discriminator (Slice 4):** the `mid` readback oracle after Pass H exists *before* the run (capture-before-you-need-it), run in the **H-only** configuration (amendment 6): if `mid` is correct but the untouched two-pass `dst` is wrong, the fault is the inter-pass flush/MOCS, not the kernel math; if the same input yields non-deterministic `dst` across runs, it is a coherence bug specifically.

### 4.5 Deliberately AVOIDED (design choice — keeps the delta small)
**No SLM / workgroup memory, no `workgroupBarrier`, no atomics, no storage images, no surface state, no samplers.** These are precisely the shapes Chad flagged as unproven for Vello (§3 of the cross-check). P1's linear-buffer separable blur touches **none** of them. That is the point: P1's new-silicon surface is exactly the three items above (2-D geometry, real occupancy, inter-pass coherence) — each with a discriminator — and nothing more.

---

## 5. Kill-switch (Rule-1) + the byte oracle

### 5.1 The kill-switch and its regression
`GPU_COMPUTE_ENABLE: AtomicBool = false` (default off), mirroring `GPU_SUBMIT_ENFORCE`. The pure toggle logic follows `authority_allows`'s shape (host-KAT-shared).

- **Rule-1 substrate equivalence (service off ≡ no service):** with the service disabled, no compute submit occurs and the system behaves **bit-for-bit as before the service existed** — the glass backdrop is produced by the CPU blur floor (Slice 1). **Regression:** the CPU-floor present output is byte-identical with the service compiled out vs. compiled-in-but-disabled.
- **The correctness oracle (GPU on ≡ CPU floor, bit-for-bit):** because the blur is exact fixed-point integer (§1.2), the GPU kernel's output **equals** the CPU floor's output **bit-for-bit** on any input. This is a *unit-test-expressible equality* (the strongest kill-switch form) and simultaneously the correctness gate: the CPU floor is the reference, the GPU path a provably-equal re-implementation.

### 5.2 The byte oracle
The hand-laid-out 8×8 `r=1` fixture of §1.5 — computed independently of both implementations — asserts the exact bytes at the clamp edges and the round-half-up boundary. Required Slice-1 deliverable; both the CPU floor and (from Slice 3) the GPU kernel are checked against it.

---

## 6. Concurrency-pitfall consultation (per the SMP-touching standing rule)

P1 adds a GPU submit that touches rings/execlists, so the bank (`docs/concurrency-pitfalls.md`, CP-1…CP-29) is consulted; per touched class:

| Class | P1 touch | Guard / status |
|---|---|---|
| **CP-2 / CP-3** (IF-off spin / IRQ-safe lock across cross-CPU wait) | **N-A for P1** — P1 adds **no new GPU IRQ**; the compute completion is a deadline-bounded **poll in process context**. | The later real-GPU-compute-IRQ slice inherits the ADR-050/076 leaf-`SwitchMutex` + lock-free `publish` discipline; not P1's seam. |
| **CP-7** (ACK/EOI ordering) | **N-A for P1** (no new interrupt handler). | Deferred with the IRQ-completion slice. |
| **CP-11** (wrong `Ordering` on an atomic) | The seqno/epoch atomics (`DispatchToken.sequence`, `completed_epoch`). | **Reuse** the proven `fetch_max(…, Release)` / `load(Acquire)` (I-16) — no hand-rolled lock-free added. Grep-gate on the new toggle's ordering. |
| **CP-14/15/16** (lost wakeup / park-unpark) | **Structurally absent** — the fence wait is a **deadline-bounded yield loop, no park** (existing `fence_wait`; `AccelDriver::wait(timeout)`). | Reused as-is. |
| **CP-29** (AP per-CPU init buddy-lock/shootdown) | **N-A** — arena allocation happens at buffer creation on a normal CPU, never in an AP-init/ISR flip-arm path (ADR-076 established this). | Unchanged. |

No new concurrency pitfall is introduced by P1's poll-only compute submit. If the later IRQ-completion slice learns a new trap, it is added back to the bank.

---

## 7. Prior-art trap-mining (Gen9 compute-shader path — read-for-principle, no GPL lift)

**License correction, raw-verified 2026-07-22.** The earlier blanket statement “Mesa/i915 are GPL” was false. At Mesa commit `d49a00bdf15fd48b31af93aaf5feed3eebcbda12`, raw `docs/license.rst` says most Mesa code is MIT and both raw `src/gallium/drivers/iris/iris_state.c` and `iris_pipe_control.c` carry `SPDX-License-Identifier: MIT`. Intel NEO commit `cedeb04702f0a9db5338d0f0926f8104c113c2f0` has an MIT `LICENSE.md` and its raw local-id helper is MIT. The exact Linux i915 LRC source consulted (`drivers/gpu/drm/i915/gt/intel_lrc.c`, Linux master `248951ddc14de84de3910f9b13f51491a8cd91df`) is also SPDX MIT. The Linux kernel as a combined distribution has its own GPL-2.0-only licensing rules, but that does not change these file-level identifiers. We keep the stricter **principle-only, no code lift** rule for this project regardless of source license.

Raw Intel authority was the Kaby Lake PRM Vol. 2a (`IHD-OS-KBL-Vol 2a-1.17`, SHA-256 `0F2BD4D5FDCDB502606BDA01D91247A4101A076C8F1FFEB620FC42BEB3F141A4`) and Vol. 7 (`IHD-OS-KBL-Vol 7-1.17`, SHA-256 `22AA547D0B634AF217917D71642791F522ACE6A9567AED74451C040FB99A7BEE`). The command bytes and field encodings below were checked from those raw PDFs, not a web summary.

Traps found and pre-seeded into the Slice-4 implementation/bank:

1. **Eight hardware threads, not one.** A 32×8 SIMD32 work-group needs eight 192-byte local-ID blocks. NEO's raw `local_id_gen` advances one `activeChannels * 64-byte` block per hardware thread. The walker therefore carries 96 cross-thread bytes + `8 × 192` per-thread bytes = 1,632 bytes, aligned to a 1,664-byte indirect length. `local_id.x` is lanes 0…31 and `local_id.y` is the hardware-thread row 0…7.
2. **IDD/walker thread counts must agree.** PRM Vol. 2a requires `(depth+1) × (height+1) × (width+1)` to equal IDD `Number of Threads in GPGPU Thread Group`. Mesa iris packs `dispatch.threads - 1` in walker width. Slice 4 therefore emits width-max 7, height/depth 0, and IDD DW6 = 8. The inherited IDD value 1 was a real first-silicon hang/partial-output defect and is now byte-oracled.
3. **VFE URB entry allocation must cover the whole indirect payload.** PRM Vol. 2a says a `GPGPU_WALKER` using indirect data needs a sufficient URB entry allocation. DW5 is U16-1 in 32-byte registers: 1,664 bytes needs 52 registers, raw value 51. The inherited raw value 2 covered only three registers and was corrected with a host invariant.
4. **A stalling `PIPE_CONTROL` precedes `MEDIA_VFE_STATE`.** Mesa iris repeats the PRM restriction. The existing post-SBA command has CS-stall set; the host command oracle keeps it from drifting.
5. **Dependent A64 writes need an actual barrier.** The H→V packet remains exactly `PIPE_CONTROL` DW0 `0x7a000004`, DW1 `0x00100020` (CS-stall bit 20 + DC-flush bit 5). MOCS is cache policy, not ordering; stateless MOCS remains 0 for this pinned path.
6. **PPGTT is an LRC property on this silicon.** ADR-032 already falsified a legacy-ring PDP load for EU A64 addressing. Slice 4 refuses that path and constructs a 48-bit PML4→PDP→PD→PT root in a fresh ELSP/LRC context. Its leaf table maps the command/data virtual-address set: ring, state, both 4,736-byte kernel pages, operands, seqno, both CURBEs, and weights. LRC/HWSP are reached through their required GGTT context mappings and the page-table hierarchy is walked physically; all three classes still use graphics-arena-backed allocations so the separate narrowed VT-d run covers the complete physical working set.
7. **Repeated context identity is a trap.** The H-only run, two fixed-oracle untouched two-pass runs, the multi-group untouched run, and terminal negative use distinct non-zero 21-bit context IDs; re-presenting the same LRCA/tail as one context can be treated as an empty resubmit.
8. **Edges are guarded in the kernel.** Right/bottom walker masks remain full because the pinned kernel returns before any address calculation when `x >= width || y >= height`. This is the memory-safety boundary; the gate fixture still checks all edges byte-for-byte.
9. **Kernel tail/EOT must be reachable.** The 4,736-byte blob crosses a page. The backing allocation and GGTT/PPGTT mappings cover two pages, including the EOT-bearing tail; a one-page mapping localizes as instruction-fetch stall/no seqno.

---

## 8. The build ladder (each slice: kill-switch, gate, discriminator)

Ordered from here to glass blur on the iGPU. Honest effort per slice; **no single number hides a compiler** (there is no compiler on this path). Bare-metal slices reference `docs/runbook-baremetal-arthur-optiplex.md`.

### Slice 1 — CPU blur floor (the honest MVP; **BUILT in this dispatch**)
Land the **CPU** separable-Gaussian blur (§1) over the arena-backed backdrop as `blur::apply_blur_or_floor` (pure, host-mounted) + `SharedPixelBuffer::blur_backdrop` (kernel-internal, no syscall — amendment 4). Produces real glass on any machine, GPU or not; **is** the ADR-024 degraded/Reduce-Transparency performance floor and the I-14(b) fallback. **Kill-switch:** blur-off ≡ the solid Reduce-Transparency floor (the backdrop passed through unblurred), bit-for-bit (`GLASS_BLUR_ENABLE`, default off). **Damage-halo:** commits the radius-expanded rect (amendment 3). **Explicit bounds checks** (amendment 2) via `blur::validate_blur`. **Gate:** host KATs + the §1.5 literal byte oracle (`tools/atrium-logic-tests::blur_tests`) — **and**, because the code compiles into the kernel (kernel-touching), the **`workflow_dispatch` 4-shard QEMU gate**, including the new `atrium_glass_blur` scenario that asserts the *kernel* `blur_backdrop` path produces the literal oracle bytes + the kill-switch passthrough. **No new silicon** (no GPU dispatch). *(Prometheus Compute-0 / Janus GFX-0 analogue, blur-specialized.)*

### Slice 2 — the compute ABI + service (kill-switch default off; **kernel-internal, no syscall**) — **BUILT in `codex/gpu-p1-slice2-compute-device-cerebro`**
Built the **kernel-internal** `GPU_COMPUTE_ENABLE` + `gpu_compute::blur_backdrop` (§2, amendment 4 — **no `Handle`, no syscall, no on-wire struct**) *above* the Phase-6 legacy-ring/PPGTT substrate (`accel_intel::submit_compute` + the `PIPE_CONTROL` post-sync **seqno fence**; inventoried, not re-derived). The compute submit is a new **defaulted** `AccelDriver::submit_compute_pipeline(&ComputePipeline)` (a `ComputePipeline` manifest — NOT an `OpKind`, which is the syscall-reachable *tensor* ABI: routing the blur through `OpKind` would risk a userland-reachable compute op, exactly what amendment 4 forbids). **No GPU kernel yet** — the submit path is validated with a **no-op/echo pipeline** (`ComputePipeline::Noop`); `ComputePipeline::SeparableBlur` is defined as the **Slice-3 target** (drivers return `Unsupported`). **Completion is a fence, not a present:** the `DispatchToken` seqno path is entirely separate from `present`/`arm_flip`/`completed_epoch`/`PLANE_SURF` — the compute path is not a second road to scanout (the compositor alone holds the `GpuSubmit` present authority). **Kill-switch (Rule-1):** service-disabled ≡ Slice-1 CPU-floor state, bit-for-bit (the `accel_gpu_compute` scenario asserts the r=1 oracle bytes for both toggle states + that the submit path is exercised iff enabled: `compute_admits` 0 vs 1). **CP-18:** the whole submit→wait→complete is serialized under a leaf `COMPUTE_IO_GATE` (the single-slot substrate); CP-2/3 N-A (poll completion, no new IRQ). **I-14:** (a) no governance lock, (b) deadline-bounded wait degrades to the CPU floor, (c) no new userland DMA — the physical VT-d fence over all GPU-reachable allocs is Slice 5, before any userland queue. **Gate:** host KATs (the `gpu_compute_cap_logic` authority table, mirrored byte-for-byte with the present-submit table) + **QEMU on the `accel-sim` substrate** for the submit→fence→wait plumbing. **Silicon-only (deferred, labeled):** `accel-intel` is bare-metal-only (real UHD 630 PCI IDs; QEMU cannot probe it) — the **real Gen9 EU dispatch + "the seqno lands after execution" is Slice 4** (M710q), not claimed by the QEMU pass. **No on-wire byte-oracle:** the manifest is a kernel-internal Rust enum (amendment 4 — no syscall/wire); the userland wire ABI is Slice 6. *(Prometheus Compute-2 skeleton, kernel-only.)*

### Slice 3 — the one pinned blur kernel via ocloc (**named Linux/amd64 finalizer host — amendment 7**)
Author `blur.cl` (OpenCL C, §1), compile through the pinned `tools/intel-gen9-finalizer` (`ocloc` + `ocloc disasm` byte oracle, ADR-032 D4), extract the EU blob + zeinfo ABI, pin as a `const` blob with full provenance. Wire it as the manifest pipeline. **The finalizer container is Linux/amd64-only** (`ubuntu:22.04`; `build-kernels.sh` needs Bash + `readelf`/`dd`/`awk`/`sha256sum`) — it does **not** run natively on the Windows PowerShell host. **Name the concrete runner: the `<runner-host>` Docker-Linux runner Janus is standing up** (`docs`/`Plans` for `codex/ci-runner-host-janus`), or a tested WSL2/Docker-Linux path; build the image there, record image/source/output hashes, and make generation **plus a second reproducibility run** a gate. Also produce the **PRM command-byte oracle** (amendment 6): the `PIPE_CONTROL`/MOCS bytes between H and V, from raw PRM source (no `WebFetch`). **Kill-switch:** GPU-on ≡ CPU floor bit-for-bit (§5.1, the fixed-point equality). **Gate:** host KATs (the equality) + the §1.5 byte oracle + the command-byte oracle; **QEMU** for the dispatch plumbing. **New silicon shape is validated on real hardware in Slice 4** (QEMU's `-cpu`/software-EU does not prove Gen9 EU execution). *(Prometheus Compute-2.)*

### Slice 4 — the silicon gate on the real UHD 630 (bare-metal, blind)
Run the two-pass blur on the OptiPlex 7080 UHD 630. **Failure-mode / discriminator bank** (blind hardware — conclusive on pass *or* fail; per the standing rule; channels per `docs/runbook-baremetal-arthur-optiplex.md` — serial `REPLY:` log + NVMe sink):

| Expected failure (even if the primary fix is right) | Discriminator (capture must pre-exist) |
|---|---|
| 2-D `local_id` GRF mis-place (§4.2) | `RING_FAULT` **VA** decode + H-only known-input mismatch; wrong row placement gives a periodic row signature |
| Walker/IDD thread-count mismatch | seqno stuck or partial rows; serial records walker width-max 7 and IDD thread count 8; host byte oracle pins both |
| VFE URB entry too small for the 1,664-byte indirect heap | seqno stuck/partial rows; host oracle decodes DW5 U16-1 as 52 × 32 bytes before silicon |
| SIMD32 lane / edge guard bug (§4.3) | vs. CPU-floor reference; periodic 32-pixel stripes localize; edge rows vs. interior; full masks are safe only because the kernel bounds-returns before address math |
| Inter-pass flush/MOCS (§4.4) | **`mid` readback oracle after Pass H** (`copy_from_device` — must exist before the run); `mid` OK + `dst` wrong ⇒ flush/MOCS; non-deterministic `dst` ⇒ coherence |
| PPGTT/A64 binds the wrong address space | `RING_FAULT` VA classified against every logged operand/kernel/CURBE VA and the non-zero PML4 root; legacy PDP-load is refused |
| Kernel second page/EOT unreachable | instruction-fetch `INSTDONE` stall, no seqno, kernel-pages breadcrumb ≠ 2 |
| Repeat dispatch is mistaken for an empty same-tail context | distinct context-id breadcrumbs on H-only, equality runs 1/2, the multi-group run, and the negative run |
| Degrade-to-floor masks a dead EU | correct bytes are insufficient: `walker_kicks`, `compute_dispatches`, `blur_completions`, real token, command/LRC oracles, and `compute_degrades == 0` must all agree |
| Enabled EU timeout | terminal forced-observer-timeout run must return the exact CPU floor, increment degrade, retain all potentially live DMA allocations, and quarantine future submit |
| Narrowed VT-d working set is incomplete | separate run 2 first repeats byte equality under the translated domain and drains the hardware fault records with `positive_vtd_faults=0`; only then may an intentionally out-of-arena weights read fault with the iGPU SID and external physical address |

**Stage A built (2026-07-22; not the silicon close):** `accel-intel` now owns the output on success, submits the real batch through a 48-bit-PPGTT ELSP/LRC context, captures H-only `mid`, preserves untouched H→V, emits the fault-VA/context/evidence breadcrumbs, and quarantines timed-out DMA. The silicon scenarios retain the literal 8×8 oracle and add a 33×31 exact CPU-oracle surface: its 2×4 walker grid executes eight partial-edge workgroups / 256 SIMD32 hardware threads, so the gate observes group-ID packing and both edge guards instead of accepting a one-group result. The narrowed scenario establishes a clean pre-run VT-d baseline and requires a post-positive hardware drain with `positive_vtd_faults=0` before arming its deliberate fault, preventing an earlier containment error from being misattributed to the negative. The QEMU `accel_gpu_compute` scenario proves only bound-buffer wiring, kill-switch equality, and unconditional degrade. Two bare-metal scenarios/bundles exist: `accel_intel_blur_silicon` (identity domain plus terminal observer-timeout floor) and `accel_intel_blur_silicon_narrowed` (translated domain plus terminal real out-of-domain weights fault). Their exact JOINT procedure is `docs/runbook-gpu-p1-slice4-optiplex.md`.

**Gate remains OPEN pending hardware:** the identity-domain run must report H-only `mid`, two repeatable untouched 8×8 `dst` outputs, and the untouched 33×31 multi-group `dst` equal to the CPU floor byte-for-byte with zero degrade; the separate narrowed run must repeat all of that equality, prove `positive_vtd_faults=0`, and then record the real iGPU VT-d fault. QEMU cannot execute a Gen9 EU, so no QEMU result closes Slice 4. A self-localizing hardware FAIL is a valid return, not a pass. *(Silicon proof of P1's kernel.)*

### Slice 5 — the **complete** graphics VT-d narrowing (the I-14(c2) close) — **precedes any userland compute queue** — **BUILT (QEMU-proven) in `codex/gpu-p1-slice5-graphics-vtd-domain-cerebro` (`4b92366`)**
Narrow the iGPU from the identity VT-d domain to an **arena-only translated domain**, generalizing ADR-050's fence-domain to graphics. **Amendment 5:** inventory and migrate **every GPU-reachable allocation** — ring, LRC, HWSP, state, kernel blob, CURBE, seqno, PPGTT page tables, *and* the pixel buffers — into a **named graphics arena/domain** distinct from the connectivity arena; the domain cannot execute until every required page is mapped in it, and the negative test probes an address **outside the complete mapping**. **This is OT-63 Slice 5** (already planned; ADR-076 names it). **Kill-switch:** graphics-narrowing-off ≡ Slice-4 identity-domain state, bit-for-bit. **Gate:** the ADR-050 no-regression oracle + a deliberate out-of-domain GPU DMA faulting.

**BUILT (`4b92366`, Cerebro):** `kernel/src/drivers/graphics_arena.rs` — a **named second DMA arena** (disjoint-by-construction from the connectivity `dma_arena`, separate buddy blocks) reusing the pure span/validate logic; holds the `GRAPHICS_NARROWING_ENABLED` kill-switch (**default OFF ≡ identity-domained Slice-4, bit-for-bit**). `DmaBuffer::alloc_gpu` is the **GPU-reachable-allocation chokepoint** (ON → graphics arena; OFF → `alloc_zeroed` buddy). The `DID_GRAPHICS` domain is built via the generalized `build_translated_domain` (**same 2nd-level page-table object as the proven connectivity fence domain — no new byte layout**); `fence_graphics_source_id` repoints the iGPU; `narrow_graphics` is the production entry (a no-op while the kill-switch is OFF). `set_narrowing(true)` is **refused when the arena is unset** (fail-safe, amendment 5(ii)). Still **kernel-internal (amendment 4)** — no userland queue.
- **QEMU-proven (the merged gate, `workflow_dispatch` #29862408777):** the domain narrowing + the domain covering all routed GPU-reachable allocs + the **out-of-domain fault negative control** (a real NVMe/AHCI DMA-master proxy repointed into `DID_GRAPHICS` takes a **genuine VT-d hardware fault** — real FRCD registers, correct source-id, out-of-arena fault addr, governance sentinel intact; the same proxy-validity the merged `vtd_fence` accepts, since QEMU cannot emulate the Gen9 iGPU as a DMA master) + arena disjointness + the kill-switch equality. PM AAG clean — 3 lenses (I-14(c2)/mem-safety, concurrency/IOTLB-invalidation ordering, completeness/fault-genuineness).
- **Silicon-only (Slice-4-adjacent, deferred, labeled `real_gpu=silicon_slice4_adjacent`):** the **real Gen9 iGPU DMA under the narrowed domain on hardware** — pairs with the Slice-4 bare-metal EU run on the **same JOINT session** (the real GPU executing the blur, now under the narrowed VT-d domain, containment proven on silicon).
- **Fail-closed completeness note (carried to Slice 4):** the domain maps **only** the arena, so any GPU-reachable buffer NOT drawn from it — the tensor-residency `AccelDriver::alloc` path + the `PendingSmoke`/`PendingNoop`/`PendingExeclist` diagnostic sites (all still `alloc_zeroed` today) — would **fault** if run under narrowing (a functional limitation, **never** a mapped-wide containment hole). They auto-join the `alloc_gpu` chokepoint as they land; the enable-flag is the fail-safe. Route them through `alloc_gpu` when the live paths that use them land.
- **Gaming through-line (note):** this graphics-VT-d-domain-for-a-GPU pattern is exactly what the ratified gaming passthrough-microVM extends at scale (VT-d-passing a discrete GPU to a confined guest) — proving it on the Gen9 iGPU de-risks the gaming Phase-0 spike.

### Slice 6 — the userland three-cap ABI + wire the blur into the G1 backdrop path (**userland queue lands HERE, after the fence**) — **BUILT (QEMU gate; no hardware claim) in `codex/gpu-p1-slice6-glass-into-g1-chad`**
**Only now** (post-Slice-5, amendment 4) introduce `Handle::GpuCompute` + the service syscall + the concrete three-cap userland split (`GpuBufferCap`/`GpuQueueCap`/`GpuFenceCap` — generation-safe buffer handles with ranges/rights, a quota-bound manifest queue/context, fence handles that convey wait/query but not submit). The implementation has no `sys_gpu_compute_create`: the kernel mints the three roots only after the graphics-narrowing gate, userspace may only derive/grant attenuated descendants, and a normal G1 `Buffer` is rejected as `EBADF`. `gpu_compute::blur_backdrop` now drives the compositor-owned G1 glass sequence **capture → blur/floor → integer vibrancy tint → composite**, using three distinct GPU-reachable buffers and committing radius-expanded damage. The `gpu_glass_slice6` QEMU scenario asserts a literal 8×8 composited/tinted byte oracle; GPU-off and forced GPU-degrade equal the same CPU-floor composite, while enabled `accel-sim` records a completed compute token. That token is not a present: an unprivileged `SharedPixelBuffer::present` still fails the existing `scanout_authorized()` gate, and only a separately authorized present reaches the framebuffer. The QEMU result is plumbing/byte-equivalence evidence only; it makes no EU or hardware-pixel claim.

**Honest critical path to GPU glass blur** (dependency depth, not calendar): Slice 1 (floor — **built**) → 2 (kernel-internal ABI) → 3 (kernel + Linux finalizer + command-byte oracle) → 4 (silicon) → **5 (complete VT-d narrowing) → 6 (userland queue + wire)**. A **kernel-trusted** service can reach silicon glass at Slice 4 as the risk-accepted stage of §3.2, but **no userland compute queue exists before Slice 5** (amendment 4). This ADR fixes the *order and the gates*; per the no-human-labor-time rule, effort is expressed as dependency depth + iteration cycles + gate cost at each slice's dispatch, never a roll-up calendar number.

---

## 9. Contradiction check

| Decision / prior art | Status | Note |
|---|---|---|
| OT-63 `GpuSubmitCap` / present-epoch fence | **Compatible** | P1 reuses the default-off authority-toggle pattern and seqno wait semantics without introducing a compute handle; a future userland queue can follow the generation-safe cap pattern after the fence. |
| ADR-076 (arena-backed scanout, GPU identity-domained, Slice-5 VT-d narrowing) | **Compatible — and the anchor** | P1's (c2) sequencing *is* ADR-076's Slice-5; the backdrop buffer is already `alloc_in_arena`. |
| ADR-045/050 (fenced arena / VT-d) | **Compatible** | c1 uses checked fixed VAs plus per-context PPGTT; c2 reuses the graphics-arena fence-domain mechanism applied to every `alloc_gpu` working-set page. |
| ADR-032 (Phase-6 compute + ocloc) | **Compatible** | P1 is a vertical skin; the ocloc path and stateless-A64 substrate are reused verbatim. |
| ADR-024 (frosted glass) | **Compatible — P1 serves it** | P1 proves the blur stage; the degraded opaque path is P1's kill-switch floor. |
| Vello / wgpu / WGSL | **Out of scope (P2)** | Explicitly fenced out (§0.1); no P1 dependency. |
| Chad's cross-check (REFUTE the Vello "short hop" claim; require the physical fence) | **Adopted + 7 amendments folded (§12)** | §0.1 (no compiler on P1), §1.5 (literal oracle), §2/§3 (kernel-internal before the fence; complete graphics domain), §4 (honest silicon delta + command-byte oracle). |

No architectural contradiction. The only tension the research carried — "PPGTT+VT-d contains the GPU" — is resolved in Chad's favor (§3) and matches the already-ratified OT-63 ladder.

---

## 10. Consequences

- P1 delivers **glass blur without waiting for a shader compiler or a Vello backend**, on the proven Phase-6 substrate, with a CPU floor that ships first and a GPU path provably equal to it.
- The **I-14 ledger is unchanged**; P1 adds a *sequencing obligation* (userland compute queue ⟵ OT-63 Slice-5 VT-d narrowing) already discharged by a planned build, and records the compute path's (c2) as closed-at-Slice-5 exactly as ADR-076 does for scanout.
- The **cap ABI is sized for its future**: pre-fence, the blur is a **kernel-internal** call path (no `Handle`, no syscall — amendment 4); `Handle::GpuCompute` + the three-cap userland split land at Slice 6, only after Slice 5 makes a userland queue safe.
- **P2 (Vello / compiler closure) remains gated** behind the three closure spikes (§0.1) and attaches to no schedule until they run.
- The complexity budget is respected: **no new invariant**, one kill-switch, one no-feature reference (the CPU floor doubles as the correctness oracle), and every new silicon shape is enumerated with a discriminator.

---

## 11. Sources and confidence

**Internal, read directly this session:** `docs/RESEARCH-2026-07-20-gpu-graphics-compute-api-design-FINDINGS.md` (Prometheus), `docs/RESEARCH-2026-07-20-gpu-graphics-api-crosscheck-FINDINGS.md` (Chad — the load-bearing amendment), `docs/ADR-032-…-gen9-gpu-compute-driver.md` (ocloc path, iter6–8 GRF/SIMD/occupancy reality), `docs/ADR-045-…-fenced-dma-arena.md` (I-14(c1) `FencedDmaAddr` single-gate), `docs/ADR-050-…-vtd-iommu.md` (I-14(c2) identity vs fence domains), `docs/ADR-076-scanout-dma-fence.md` (arena-backed `SharedPixelBuffer`, present-epoch fence, the Slice-5 GPU-narrowing anchor), `docs/ADR-024-…-design-system.md` (the frosted-glass direction + Reduce-Transparency invariant), `docs/concurrency-pitfalls.md` (CP-1…CP-29). In-tree signatures verified against `kernel/src/{sched/process.rs, sched/gpu_submit_cap_logic.rs, atrium/present_backend.rs, atrium/buffer.rs, drivers/fb/mod.rs, drivers/accel/{mod.rs,accel_intel.rs,intel_gen9_execlist.rs,intel_gen9_4a.rs}, drivers/block/dma.rs, drivers/dma_arena.rs}`.

**External, exact-source:** Intel Kaby Lake PRM Vol. 2a/7 raw PDFs and hashes recorded in §7; Mesa `d49a00bdf15fd48b31af93aaf5feed3eebcbda12` (`docs/license.rst`, `iris_state.c`, `iris_pipe_control.c`); Intel NEO `cedeb04702f0a9db5338d0f0926f8104c113c2f0` (`LICENSE.md`, `local_id_gen.h/.inl`); Linux `248951ddc14de84de3910f9b13f51491a8cd91df` (`intel_lrc.c`). License identifiers were read from raw files; §7 corrects the prior blanket GPL claim. All were mined for principle only, with no code lift. Vello at commit `53df8b970588…` remains a P2 boundary only, not a P1 dependency.

**Confidence:**
- P1 ABI reuse (cap pattern, arena fence, seqno completion) — **[High]**; all pieces are in-tree and cited.
- The I-14(c2)/Slice-5 sequencing — **[High]**; directly corroborated by ADR-050 + ADR-076 + Chad.
- Fixed-point kill-switch/oracle equality — **[High]**; integer arithmetic makes it a provable equality.
- The silicon-shape delta (2-D geometry, real occupancy, inter-pass coherence) — **[High that these are the new shapes; silicon result still unknown]** — Stage A found and repaired three pre-silicon defects (eight local-ID blocks, IDD thread count, VFE URB size), but only the JOINT run can validate the resulting command stream.
- Per-slice effort — **[deliberately not quoted as a roll-up]**; estimated at each slice's dispatch.

**RAG health:** `healthy` (`ollama:true, qdrant:true`, 2026-07-21). Used for discovery only; all load-bearing files read directly.

---

## 12. v2 changelog — the 7 folded amendments (Chad cross-check → applied)

Chad's cross-model cross-check (`docs/RESEARCH-2026-07-21-gpu-p1-adr-crosscheck-FINDINGS.md`) verdict was **RATIFY-AFTER-AMENDMENT**: the architectural cut (one project-authored OpenCL-C linear-buffer blur through the proven ocloc path; CPU floor first; no Vello/WGSL/wgpu; no userland GPU queue before a real graphics VT-d domain) is correct; 7 bounded amendments were required before Slice 1. All are folded:

1. **Literal 8×8 byte oracle (§1.5).** v1 named an oracle but shipped no bytes. v2 ships the literal source → H-intermediate → dest fixture (memory order `BB GG RR FF`), exercising all four clamp edges + corners, **odd quarter-sums that hit round-half-up** (seeded B row y=3), unequal R/G/B, a nontrivial X byte proven ignored → `0xFF`, a 255 clamp, and the **H-pass separately asserted**. Generated by an independent Python reference; the Rust `blur.rs` agrees byte-for-byte; the PM independently recomputes it.
2. **Explicit kernel edge bounds checks (§1.4).** Walker execution masks are an ABI/SIMD concern, **not** the memory-safety boundary — the kernel MUST `if (x>=width||y>=height) return;` before any address math; the CPU floor carries the identical `validate_blur` up-front gate.
3. **Damage = radius-expanded output + source halo (§1.1).** Committed output damage = changed rect ⊕ `r`; source read = changed rect ⊕ `2r`; both surface-clamped. Pinned + host-tested (`blur::expand_output_rect`/`source_halo_rect`); Slice 1 commits the radius-expanded rect.
4. **Pre-VT-d wholly kernel-internal (§2, §8).** A `Handle::GpuCompute` + syscall *is already a userland queue*; it is **deferred behind Slice 5**. Slices 2–4 are a feature-gated kernel-internal path only; the userland three-cap ABI lands at Slice 6.
5. **VT-d domain covers ALL GPU-reachable allocations (§3.2, Slice 5).** Ring/LRC/HWSP/state/kernel/CURBE/seqno/PPGTT page tables — not merely the 3 pixel buffers — migrated into a **named graphics arena/domain**; the negative test probes outside the complete mapping.
6. **PRM-derived command-byte oracle + non-perturbing diagnostic (§4.4, Slice 3).** MOCS is a cacheability index, not a barrier; `PIPE_CONTROL` bytes between H and V come from **raw PRM source (no WebFetch)** as a hand-laid command-byte oracle; the coherence diagnostic uses two runs (H-only readback; untouched H→V) so it cannot perturb the hazard.
7. **Named Linux/amd64 ocloc host (Slice 3).** The finalizer is Linux-only; Slice 3 names the **`<runner-host>` Docker-Linux runner** (Janus) or a tested WSL2/Docker-Linux path, with generation + a second reproducibility run gated.

Corroboration (Chad §7.5): the no-SLM Q16 separable blur + the pinned OpenCL-C/ocloc route are sound; the CPU-first, VT-d-before-userland, P2-excluded direction proceeds. Also applied: the person-month figures the reviewed research carried are **restated as dependency depth / iteration cycles / gate cost** per the 2026-07-21 no-human-labor-time rule.

---

*v1 was one NO-GATE documentation artifact, itself cross-checked (Codex, cross-model) before Slice 1. v2 folds that cross-check's 7 amendments and is the document Slice 1 (the CPU blur floor, built in `codex/gpu-p1-adr-v2-slice1-cerebro`) is asserted against.*
