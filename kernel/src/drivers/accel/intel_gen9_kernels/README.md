<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Intel Gen9 EU Kernel Provenance (finalizer-built)

BUILD-4a embeds two Gen9.5 EU kernels for the OptiPlex UHD 630 bring-up:
`ADD_F32X2_KERNEL_GEN9` (elementwise `f32[2]` add) and `NOOP_KERNEL_GEN9`
(EOT-only no-op, the iter5/iter6 dispatch bisector). Both are now **emitted by a
pinned, reproducible Intel finalizer** — they are no longer the earlier
un-finalized blobs (an ASCII placeholder Add, a hand-laid no-op).

The finalizer toolchain lives at `tools/intel-gen9-finalizer/` (a containerized
build tool, not a kernel runtime dependency).

## Pinned toolchain

| Component | Pin | Notes |
|-----------|-----|-------|
| Intel Graphics Compiler | **`igc-1.0.17193.4`** | the README's historical `igc-1.0.17193` line, exact patch (2024-07-24) |
| compute-runtime (ships `ocloc`) | **`24.26.30049.6`** | rates **Skylake / Kaby Lake = Production** (24.35+ demotes Gen9 to legacy) |
| libigdgmm12 | `22.3.20` | GMM, the CR 24.26 dependency |
| base image | `ubuntu:22.04` | what CR 24.26 debs target |

Pinned IGC deb sha256 (verified in the Dockerfile):
- `intel-igc-core_1.0.17193.4_amd64.deb` → `98012c235ca1a8c953bc847eaf92d061e84a0f0c67a94b9375fcbb5f1a9ae8c0`
- `intel-igc-opencl_1.0.17193.4_amd64.deb` → `020d9ea61a27f0d86a2665ca56ee6f4695ddee02579b6d636995cd19f9e96ba1`

## Substitutions vs. the original ADR-032 D4 recipe (FLAGGED)

The exact tag and the *form* of the finalizer differ from the original aspiration;
each is a deliberate, recorded substitution, not silent drift:

1. **`.visa` → OpenCL-C + `ocloc`.** No standalone vISA finalizer (`GenX_IR`) and
   no `iga64` ship in any prebuilt Intel deb — both only exist by building IGC
   from source. The available finalizer is `ocloc` (the *same* igc-1.0.17193.4
   backend, driven from OpenCL-C). So the embedded blobs are compiled from
   `*.cl` sources, not the `.visa`/`.asm` + `iga64` path. `add_f32x2.visa` is
   retained as intent documentation only.
2. **`ocloc` ships inside `intel-opencl-icd`**, not a standalone `intel-ocloc`
   deb (that split exists only in distro repos, not Intel's GitHub release).
3. **Disassembly oracle = `ocloc disasm`** (a decode pass, distinct from the
   `ocloc compile` emit pass), not `iga64 -d`. Both wrap the same IGA library;
   true third-party independence (e.g. Mesa `brw_disasm`) is not used here.

## Sources and blobs

| Source (`.cl`) | sha256 | → blob const | size | output `.krn` sha256 | compile options |
|----------------|--------|--------------|------|----------------------|-----------------|
| `add_f32x2.cl` | `bbba57b37f01b7fe69b03a87513ef565b8dc0ca50d824db776e005bacd84889e` | `ADD_F32X2_KERNEL_GEN9` | 640 B | `b2e3f9f154d4eddf88a81952ce6d2495366a3b423a540f1b575e5488a0b66f77` | `-cl-intel-greater-than-4GB-buffer-required` (stateless A64) |
| `noop_eot.cl` | `3d843854c8a1f81bfd8c35aa25ca513c4b15ff81c831d82fb19f515398432652` | `NOOP_KERNEL_GEN9` | 192 B | `a8237c4b3485423afd95707039c188e4d99012ed05be7aa7ed115da6243a9e13` | _(none)_ |
| `matmul_f32.cl` | `07fa1b2ba456735797c5bda102744aad5d5c28ddd7f53b64704f176cab4ed416` | `MATMUL_F32_KERNEL_GEN9` | 1216 B | `7bed26119acc7bc2f4bbb6809b757bf24a7f4e72ffb140b6b8f2c73f27e1f96c` | `-cl-intel-greater-than-4GB-buffer-required` (stateless A64) |
| `gelu_f32.cl` | `06538782cbb3c0ba9a57859ab82ae5f83c1d29a8ce62b3daab3e3fa13dcb51f5` | `GELU_F32_KERNEL_GEN9` | 4096 B | `f7ec6c94733b9ec6361ebc1c1454d3415dc09cca4f2dc03fd1571e04a0a102de` | `-cl-intel-greater-than-4GB-buffer-required` (stateless A64) |
| `blur.cl` | `af6dc6f8ba1579bf1c5bb5d1d89029259e9a14c86aff02c4438751bb7204e200` | `BLUR_KERNEL_GEN9` | 4736 B | `d40067da102c34ef8ab5ea1b79a306a63abc700a7662537264bd00924d77bfff` | `-cl-intel-greater-than-4GB-buffer-required` (stateless A64) |

The `.krn` is the bare EU instruction stream pulled from the zebin
`.text.<kernel>` section (the section is padded past the terminating EOT `send`
with `illegal` ops — the trailing zero bytes — which the thread never reaches).
The extracted-section bytes are deterministic for the pinned toolchain (the
zebin *wrapper* is not hashed; only the `.text` section is embedded).

Independent disassembly (the byte oracle): `add_f32x2.gen9.asm`,
`noop_eot.gen9.asm`. Both show a terminating spawner **End-Of-Thread send**
(`send ... 0x27 0x02000010 {EOT}`). `add_f32x2.gen9.asm` additionally shows the
real f32 add via **stateless A64** messages: 64-bit address math, `a64 dword
gathering read` of a[i]/b[i] (BTI `0xFF`), `add ...:f`, `a64 dword scattering
write` of o[i] — **no `untyped surface` / BTI surface sends**.

`gelu_f32.gen9.asm` (BUILD-4b-2) additionally shows the Gen9 **hardware
extended-math** path: `math.exp`, `math.inv`, and `math.fdiv` instructions
implementing `tanh` — not a software polynomial. This is the point of landing
GELU on silicon: the hardware-math `tanh` is a third, independent approximation
distinct from the soft-float `accel_cpu` path and from host libm, so the
`≤2⁻¹²` host-f32 tolerance fires *non-vacuously* (unlike matmul, which is
bit-exact for lack of FMA). The host gate `intel_gen9_gelu_kernel_is_finalizer_eot_with_hw_math`
asserts the committed disasm contains `math.exp`, failing CI if a re-finalize ever
loses the hardware-math lowering. GELU's cross-thread ABI (`gelu_f32.gen9.zeinfo`,
its OWN contract) is pointwise: `in`@32, `out`@40 (A64 ptrs), `count`@48 (byvalue
i32), `enqueued_local_size`@52 — span `[0,64)` = **2 cross-thread GRF**, one fewer
than the Add/matmul's 3 (their span runs to 76/80). Per-thread `local_id` (6 GRF,
SIMD32) and the indirect-data substrate are otherwise identical.

> Flag-spelling note: the working ocloc option is singular
> `-cl-intel-greater-than-4GB-buffer-required` (passed via `-options`); the plural
> `…buffers-required` form is rejected ("Unrecognized build options") by this
> ocloc. The dispatch quoted the plural; the singular is the one that compiles.

## Reproduce + diff (byte-exact, one path)

```bash
# 1. Build the pinned finalizer (downloads + verifies the pinned debs).
docker build -t neo-gen9-finalizer tools/intel-gen9-finalizer/

# 2. Finalize each source and print source+blob sha256 (run twice → same hashes).
docker run --rm -v "$PWD/kernel/src/drivers/accel/intel_gen9_kernels:/src" \
  neo-gen9-finalizer bash -lc '
    # Add ships STATELESS A64 (singular "buffer"); no-op takes no options.
    bash /opt/build-kernels.sh /src/add_f32x2.cl add_f32x2 skl /out \
      "-cl-intel-greater-than-4GB-buffer-required"
    bash /opt/build-kernels.sh /src/noop_eot.cl  noop      skl /out
    # disassembly + zeinfo oracles (prepend the SPDX header the checked-in carry):
    for k in add_f32x2 noop_eot; do
      printf "// SPDX-License-Identifier: CC-BY-4.0\n" > /src/$k.gen9.asm
      cat /out/$k.skl.asm >> /src/$k.gen9.asm
    done
    printf "# SPDX-License-Identifier: CC-BY-4.0\n" > /src/add_f32x2.gen9.zeinfo
    cat /out/add_f32x2.skl.zeinfo >> /src/add_f32x2.gen9.zeinfo'
```

Determinism is load-bearing: building any blob twice yields an identical `.krn`
sha256 (verified). A non-reproducible toolchain is a defect, not an annoyance.

## The hand-laid no-op diff (what the finalizer settled)

iter5 shipped a *hand-laid* 32-byte no-op (`mov g127,g0` + SIMD8 EOT `send`),
flagging its register-region DWords as "the residual uncertainty… if the no-op
also hangs, iter6's real IGA/IGC finalizer regenerates a confirmed blob." iter5's
Stage-B run *was* that hang, so this is the regeneration. Hand-laid bytes:

```
00600001 20fe0208 008d0000 00000000   mov(8)  g127, g0
07600031 200e1ffd 00000000 82000010   send(8) EOT (SIMD8)
```

The finalizer's verdict on "were the hand-laid DWords right?":
- **The EOT send descriptor `0x82000010` was correct** (EOT bit 31 | mlen 1 |
  spawner ctrl 0x10) — it appears verbatim in the finalizer blob.
- It **differs** by: adding a `cr0` setup (`or cr0.0 0x4C0 {Switch}`), using a
  *compacted* header `mov`, and an **ExecSize-1** EOT send (DW0 `0x07000031`),
  not the hand-laid SIMD8 `0x07600031`.

## Cross-thread argument ABI (the contract Jarvis fills — iter7)

The stateless Add reads its three buffer base pointers as raw **A64 addresses**
from the GPGPU **cross-thread (indirect/CURBE) payload**. The driver
(command-encoder region, iter7) must lay this payload out exactly as below — it
is the kernel's ABI. Authoritative source: `add_f32x2.gen9.zeinfo`
(`payload_arguments`), the independent oracle for this table.

| Cross-thread offset | Size (B) | Field | Meaning / who fills it |
|---:|---:|---|---|
| 0  | 12 | `global_id_offset` | 3×u32 global-id offset (0,0,0 for this dispatch) |
| 12 | 12 | `local_size` | 3×u32 local size |
| 24 | 8  | _(pad)_ | alignment to the 8-byte pointer at 32 |
| **32** | **8** | **arg 0 `a`** | **stateless A64 base pointer** of input `a` (readonly) |
| **40** | **8** | **arg 1 `b`** | **stateless A64 base pointer** of input `b` (readonly) |
| **48** | **8** | **arg 2 `o`** | **stateless A64 base pointer** of output `o` (readwrite) |
| 64 | 12 | `enqueued_local_size` | 3×u32 enqueued local size |

- **Total cross-thread span:** `[0, 76)` bytes; GRF-aligned, so the driver
  programs ceil(76/32) = **3 GRFs (96 B)** of cross-thread data
  (`MEDIA_VFE_STATE` / `GPGPU_WALKER` indirect-data length).
- **Per-thread payload:** `local_id` at per-thread offset 0, size 192 B (the
  walker fills this; the driver sizes it).
- **Execution env** (from zeinfo): `simd_size 32`, `grf_count 128`,
  `has_4gb_buffers: true` (the stateless marker), `eu_thread_count 7`.
- The three `a`/`b`/`o` pointers are GPU virtual addresses resolved through the
  **General State Base** (stateless A64) — **no binding table, no `SURFACE_STATE`,
  no Surface State Base**. That deletes the entire stateful-surface burden the
  earlier BTI build would have imposed.

The vestigial `arg_bypointer … addrmode: stateful … size: 0` entries and
`binding_table_indices 0/1/2` in the zeinfo are unused in stateless mode — the
A64 pointers at 32/40/48 are the live contract.

### Sequencing
`NOOP_KERNEL_GEN9` (no args) proves **dispatch** once Jarvis's iter6 lands
`STATE_BASE_ADDRESS`. This stateless Add + the ABI table above are the inputs to
**iter7**: the driver fills the three A64 pointers + General State Base, then the
real Add retires `[4,6]` — the BUILD-4a→main trigger. Stateful BTI remains the
eventual production path (ADR-023 DRM-expressibility), deferred to a hardening
build.

## OT-63 GPU P1 Slice 3 — the separable-blur kernel (`blur.cl` → `BLUR_KERNEL_GEN9`)

The glass-blur kernel (ADR-077 v2 §1): a two-pass separable Gaussian blur over a
linear XRGB8888 buffer, hand-authored to match `kernel/src/atrium/blur.rs`'s Q16
fixed-point arithmetic **bit-for-bit** (the kill-switch equality, §5.1). ONE
`.cl`/one blob serves both passes — the driver invokes it twice (H `axis=0`
`src→mid`, then a `PIPE_CONTROL` DC-flush/CS-stall, then V `axis=1` `mid→dst`) —
and both the r=1 byte oracle and the r=8 production radius, because the Q16
weights are a `__global const int*` pointer arg, not baked (radius is data).

Compiled by the same pinned `ocloc` (igc-1.0.17193.4 / CR 24.26.30049.6) with
`-cl-intel-greater-than-4GB-buffer-required` (stateless A64). **Reproducible:**
building twice yields the identical `.krn` sha256 `d40067da…` (verified). The blob
bytes embedded in `intel_gen9_4a.rs::BLUR_KERNEL_GEN9` are the literal extracted
`.text.separable_blur` section (4736 B) — never a summarizing fetch.

`blur.gen9.asm` (the disassembly oracle) shows the **stateless A64** data path —
`a64 byte gathering read 8b` (src), `a64 dword gathering read` (the Q16 weights),
`a64 byte scattering write 8b` (dst), BTI `0xFF` — plus the 2-D global-id compute
(`local_id.x` r1/r2 + `local_id.y` r3/r4) and the terminating spawner EOT `send`.
It contains **no SLM, no barrier, no sampler, no BTI-surface** message
(ADR-077 v2 §4.5): P1's linear-buffer blur touches none of the hard Gen9 shapes.
`blur.gen9.zeinfo` (asm sha `23fc9acf…`, zeinfo sha `d1ecb578…`) is the ABI oracle.

### Cross-thread argument ABI (oracle: `blur.gen9.zeinfo`)

| Cross-thread offset | Size (B) | Field | Kind |
|---:|---:|---|---|
| 0  | 12 | `global_id_offset` | 3×u32 |
| 12 | 12 | `local_size` | 3×u32 (width, height, 1) |
| **32** | **8** | **arg 0 `src`** | stateless A64 ptr (readonly) |
| **40** | **8** | **arg 1 `dst`** | stateless A64 ptr (readwrite) |
| **48** | **8** | **arg 2 `weights`** | stateless A64 ptr (readonly, Q16 `int*`) |
| **56** | **4** | **arg 3 `width`** | byvalue u32 |
| **60** | **4** | **arg 4 `height`** | byvalue u32 |
| **64** | **4** | **arg 5 `stride`** | byvalue u32 (row pitch, BYTES) |
| **68** | **4** | **arg 6 `radius`** | byvalue u32 |
| **72** | **4** | **arg 7 `axis`** | byvalue u32 (0 = H, 1 = V) |
| 76 | 12 | `enqueued_local_size` | 3×u32 |

- **Cross-thread span** `[0, 88)` → ceil(88/32) = **3 GRF** (same as Add/matmul).
- **Per-thread `local_id`** 192 B = **6 GRF**, SIMD32, **2-D** (`.x` r1/r2, `.y` r3/r4).
- **Execution env:** `simd_size 32`, `grf_count 128`, `has_4gb_buffers true`
  (stateless), `eu_thread_count 7`; **no `slm_size`, no `barrier_count`** ⇒ 0/0.
- Host oracle: `ai-ops-tests::blur_cross_thread_abi_matches_zeinfo` +
  `blur_curbe_payload_byte_layout`; blob provenance: `blur_kernel_blob_provenance`
  (asserts the sha256 above + the EOT). The amendment-6 inter-pass `PIPE_CONTROL`
  command-byte oracle: `blur_interpass_pipe_control_matches_prm_byte_oracle`.

### Reproduce the blur blob

Same one-command path as the Add (on a Linux/amd64 Docker host — the `M75q`
`<runner-host>` WSL2 runner, or any tested Docker-Linux), the finalizer image
already built:

```bash
docker run --rm -v "$PWD/kernel/src/drivers/accel/intel_gen9_kernels:/src" \
  -v /tmp/out:/out neo-gen9-finalizer \
  bash /opt/build-kernels.sh /src/blur.cl separable_blur skl /out \
    "-cl-intel-greater-than-4GB-buffer-required"
# → /tmp/out/blur.skl.krn sha256 == d40067da… (run twice → identical)
```
