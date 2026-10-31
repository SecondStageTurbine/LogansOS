<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: GPU P1 ADR-077 — cross-model FINDINGS

**Date:** 2026-07-21
**Author:** Chad (Codex; independent cross-check of Cerebro/Claude)
**Status:** NO-GATE design review

## 0. Verdict

**RATIFY-AFTER-AMENDMENT.** **[High confidence]**

ADR-077 makes the correct architectural cut: one project-authored OpenCL-C
linear-buffer blur through the already-proven pinned `ocloc` path; CPU floor
first; no Vello, WGSL, wgpu, or new shader compiler; and no userland GPU queue
before a real graphics VT-d domain. A no-SLM separable blur is real and is an
appropriately narrow first Gen9 workload.

It is not ready to dispatch Slice 1 unchanged. The promised 8×8 byte oracle is
not present, edge dispatch is unsafe as specified, damage-scoped blur omits the
filter halo, the cap/execution locus contradicts the VT-d sequencing rule, and
the physical-memory plan accounts for only the three pixel buffers rather than
all GPU-reachable allocations. The coherence gate also needs a command-byte
oracle and a non-perturbing diagnostic. These are bounded amendments, not a
reason to reject P1.

## 1. Blur specification and byte oracle

### 1.1 The requested raw recomputation cannot be performed

ADR-077 §1.5 names an “8×8 XRGB8888 source” and the `r=1` weights, but supplies
neither the 256 source bytes nor the 256 expected output bytes. No matching
fixture exists elsewhere on `origin/main`. There is therefore no exact oracle
to recompute. The document promises that Slice 1 will create it; it does not
itself close the effect/oracle gate as claimed. **[Certain; load-bearing
divergence]**

Before Slice 1, amend the ADR with literal rows for both source and expected
destination, plus at least the horizontal intermediate. State byte order
explicitly. For XRGB8888 little-endian words `0xFFRRGGBB`, memory bytes are
`BB GG RR FF`; “XRGB8888 LE” alone is too easy to misread. The fixture should
include:

- all four clamp edges and corners;
- odd quarter sums that exercise `+32768 >> 16` rather than only divisible
  values;
- unequal R/G/B channels to catch channel-order mistakes;
- a nontrivial X input byte proving it is ignored and output as `0xFF`;
- the separately asserted H-pass bytes, so H and V cannot share a compensating
  error.

The independent recomputation for the stated `r=1` arithmetic reduces to
`round_half_up((left + 2*center + right)/4)` per pass after edge replication.
That formula is internally sound, but it cannot yield ADR-077's missing 8×8
bytes without the source matrix. **[High confidence]**

### 1.2 No-SLM separable blur is real

Each work-item can gather 17 neighboring dwords from a linear A64 buffer and
write one independent output dword. Overlapping work-items may redundantly load
the same pixels; neither pass requires communication within a workgroup. SLM,
barriers, atomics, samplers, and surface state are performance optimizations or
alternative representations, not correctness requirements. **[High
confidence; corroboration]**

### 1.3 Edge work-items are not safely specified

Rounding an ND-range to 32×8 workgroups launches complete workgroups. A walker
execution mask selects lanes in hardware SIMD threads; it is not a per-final-
workgroup rectangle mask that can independently suppress every right-edge lane
and bottom-edge row. Intel's OpenCL model likewise defines one execution for
each point in the launched grid and groups those work-items into equal-sized
workgroups. The kernel must begin with:

```c
uint x = get_global_id(0);
uint y = get_global_id(1);
if (x >= width || y >= height) return;
```

before calculating or dereferencing any source, intermediate, or destination
address. Clamp-to-edge protects tap **reads** for a valid output pixel; it does
not protect an out-of-range output write or a padded work-item's row-base
calculation. Keep hardware execution masks as an ABI/SIMD concern, not the
memory-safety boundary. **[High confidence; load-bearing divergence]**

Primary-source corroboration: Intel describes an ND-range as work-items divided
into equal-sized workgroups and states that the implementation runs a work-item
for every point in the grid ([Intel OpenCL basic concepts](https://www.intel.com/content/www/us/en/docs/opencl-sdk/developer-guide-processor-graphics/2019-4/basic-concepts.html)).

### 1.4 Q16 can be bit-identical, but integer alone is not the proof

For nonnegative weights summing to 65536, the maximum channel sum is
`255*65536 = 16,711,680`; after adding 32768 it remains safely in signed i32.
The ADR's conclusion is viable. Its explanatory bound should use the weight
sum, not “17 × 255 × 65536,” which would not fit i32. **[Certain]**

CPU≡GPU additionally requires identical, explicit types and operations:

- `uint32_t`/OpenCL `uint` weights and accumulator (avoid implementation
  ambiguity around signed `char` and signed overflow);
- byte extraction by shifts/masks from a defined little-endian `uint`, not a
  C struct or vector cast with layout ambiguity;
- the same per-pass rounding and clamp, including the intermediate's 8-bit
  quantization;
- identical stride, bounds, and channel packing;
- compiler/disassembly/host KATs proving no saturating or vector conversion
  changed those semantics.

With those constraints, bit equality is provable. “It is integer” alone is not.
Also keep Rule-1 **off≡CPU** distinct from the separate GPU-on correctness claim.
**[High confidence]**

### 1.5 Damage requires a blur halo

Filtering only `surface_bounds ∩ DamageRectV1` is not generally correct. A
changed source pixel affects destination pixels up to radius 8 away in X and Y;
pixels inside the damaged output also require source samples outside the damage
rectangle. Expand the affected output by the kernel radius, capture the required
source halo (with surface/backdrop clipping and clamp rules), blur that region,
then composite only the valid affected output. Backdrop damage behind a glass
surface must also invalidate that surface even if the glass client submitted no
damage. **[High confidence; load-bearing effect-closure divergence]**

## 2. Minimal ABI and execution locus

The kernel-internal function has enough conceptual arguments—three buffers,
width/height/stride, fixed manifest pipeline, and completion—but its validation
contract is incomplete. Require checked arithmetic for
`(height-1)*stride + width*4`, `stride >= width*4`, nonzero dimensions, buffer
lengths, format, and pinned radius. Reject overlap among `src`, `mid`, and `dst`
unless alias semantics are explicitly designed. The interface should return a
two-pass operation token or final status whose failure cannot be confused with
the H-pass token. **[High confidence]**

More importantly, ADR-077 alternates among:

- “kernel-internal” and “no non-kernel caller”;
- a `Handle::GpuCompute` held by a compositor/glass service; and
- “the trusted glass service's syscall/kernel call.”

If a user process can invoke that syscall before Slice 5, it has a userland
compute queue regardless of whether kernels are manifest-only. That contradicts
the normative VT-d ordering. Amend Slices 2–4 to be an entirely kernel-owned
bring-up API with no syscall, process handle, or user-controlled dimensions/
buffers. Either defer `Handle::GpuCompute` and its syscall until after Slice 5,
or move Slice 5 before introducing them. A kernel-internal caller needs a feature
gate, not a process capability. **[High confidence; load-bearing divergence]**

After Slice 5, `Arc<DmaBuffer>` and `DispatchToken` are not userland
capabilities. The proposed future three-cap split must become concrete before a
queue grant: generation-safe buffer handles with ranges/rights, a quota-bound
manifest queue/context, and fence handles that convey wait/query but not submit.
The smaller internal P1 interface is sufficient only while it remains inside
the kernel. **[High confidence]**

## 3. I-14(c) sequencing and reachable memory

The central sequencing rule is correct: PPGTT separates GPU contexts; it does
not constrain the device's invented physical DMA, and no userland queue may
exist before the iGPU is moved from its identity domain. A fixed kernel-authored
blur can be a clearly labelled, default-off, bare-metal risk-acceptance stage;
this is reversible and no broader than the existing trusted Phase-6 bring-up.
**[High confidence; corroboration]**

The implementation obligations are wider than ADR-077 states. The live Gen9
path allocates not only data but ring, state, kernel, CURBE, seqno, LRC, HWSP,
and PPGTT page-table buffers with `DmaBuffer::alloc_zeroed`. All are reachable by
the GPU or contain addresses the GPU consumes. An arena-only VT-d domain cannot
execute until every required page is allocated in and mapped by the graphics
domain (or is explicitly admitted as a narrowly justified privileged mapping).
Requiring `src/mid/dst` to use `alloc_in_arena` does not discharge this.
**[Certain; load-bearing divergence]**

The current arena is also a connectivity arena, not a typed graphics arena.
Prior design already identifies the need for named graphics extents so granting
the iGPU its working set does not expose NIC/USB DMA buffers. Amend Slice 5 to
inventory and migrate **all** GPU-reachable allocations and create a named
graphics domain/arena; its negative test must cover an address outside that
complete mapping. Until then, describe c1 as constraining project-programmed
pixel addresses, not as making every device-programmed address a
`FencedDmaAddr`. **[High confidence]**

## 4. Silicon-shape and two-pass coherence

The ADR correctly inventories the principal new execution shapes: 2-D IDs and
payload ABI, eight SIMD32 threads per 32×8 workgroup across many groups, and a
dependent dispatch. Add these missing/understated gates:

1. explicit kernel bounds checks and non-multiple dimensions;
2. global-ID construction across group IDs, not only local-ID GRF layout;
3. walker/VFE maximum-thread and workgroup programming derived from the emitted
   zeinfo, including scratch/GRF use and `SLMSize=0`;
4. checked address/stride multiplication and multi-page PPGTT mapping;
5. the exact cacheability and synchronization command bytes.

`MOCS` is a cacheability policy/index, not a barrier. `PIPE_CONTROL` ordering and
flush bits provide the transition. The ADR's proposed DC-flush + CS-stall may be
the right Gen9 sequence for stateless dataport writes, but “correct MOCS” is not
a sufficient executable specification. Slice 3 must transcribe the applicable
Kaby Lake PRM requirements into a hand-laid command-byte oracle: placement
between H and V, required CS-stall companion bits, DC/L3 visibility behavior,
and the exact MOCS value programmed for both mappings. Slice 4 then proves it on
silicon. **[Medium-high confidence; PRM-specific bit selection remains the point
of the required gate.]**

Do not read `mid` back on the CPU *between* H and V in the primary coherence
test: that diagnostic can wait for completion, introduce CPU cache maintenance,
or otherwise perturb the hazard it is intended to observe. Use two runs:

- an H-only diagnostic run followed by readback to prove H math; and
- an untouched H→`PIPE_CONTROL`→V run followed only by final readback.

If H-only is correct and the untouched two-pass result is stale or
nondeterministic, the coherence discriminator remains conclusive. **[High
confidence]**

## 5. Build ladder, toolchain, and contradiction check

The P1 ladder correctly avoids the WGSL compiler problem and uses the exact
OpenCL-C route already proven by Add/Matmul/GELU. **[High confidence;
corroboration]**

Slice 3 nevertheless omits its execution environment. The checked finalizer is
a Linux/amd64 `ubuntu:22.04` container and `build-kernels.sh` requires Bash plus
Linux utilities (`readelf`, `dd`, `awk`, `sha256sum`). Amend the slice to name a
pinned Linux CI/worker or tested WSL2/Docker-Linux route; build the existing
image there; record image/source/output hashes; and make generation plus a
second reproducibility run a gate. Do not imply the Windows PowerShell host can
execute the script natively. **[Certain]**

Reorder the milestones to remove the authority ambiguity:

```text
1 CPU floor + literal oracle
2 kernel-internal validator/service, default off (no Handle/syscall)
3 Linux-finalized pinned OpenCL-C kernels + command/ABI oracles
4 trusted-kernel UHD 630 proof (explicit risk acceptance)
5 complete graphics arena + VT-d domain + negative DMA gate
6 userland three-cap ABI, if still required
7 G1 integration at the chosen kernel/userland boundary
```

There is no contradiction with OT-63, ADR-032, or the own-glass direction once
these amendments are applied. Present submission/fencing and compute dispatch
remain distinct authorities; reuse their logic pattern without conflating their
tokens or completion semantics. **[High confidence]**

## 6. Required amendments before Slice 1

1. Put the literal 8×8 source, H-pass, and final XRGB memory bytes in ADR-077;
   independently recompute and hash them.
2. Specify an unconditional `x/y` bounds guard, exact integer/packing types,
   checked stride/range validation, and non-aliasing.
3. Add radius-expanded damage/backdrop invalidation and source-halo rules.
4. Keep Slices 2–4 kernel-internal; defer `Handle::GpuCompute`/syscall until the
   graphics VT-d gate.
5. Inventory and migrate/map every GPU-reachable allocation into a named
   graphics arena/domain, not only pixel buffers.
6. Replace “PIPE_CONTROL + correct MOCS” prose with a PRM-derived command-byte
   oracle and split H-only readback from the untouched H→V coherence run.
7. Name and gate the Linux/amd64 finalizer environment.

## 7. Divergences and corroborations

1. **Divergence:** the ADR contains no recomputable 8×8 byte oracle; only an
   intention to create one in Slice 1.
2. **Divergence:** walker masks do not make rounded edge groups memory-safe;
   explicit kernel bounds checks and a blur/damage halo are mandatory.
3. **Divergence:** `Handle::GpuCompute` plus a service syscall before Slice 5 is
   a userland queue; keep pre-fence bring-up wholly kernel-internal.
4. **Divergence:** c1/c2 must cover ring/state/kernel/CURBE/seqno/LRC/HWSP/page
   tables in a named graphics arena/domain; three arena pixel buffers are not
   the GPU working set.
5. **Corroboration after amendment:** no-SLM Q16 separable blur and the pinned
   OpenCL-C/ocloc route are sound; the CPU-first, VT-d-before-userland, and P2-
   excluded direction should proceed.

## 8. Evidence and method

- RAG health was `healthy` (`ollama=true`, `qdrant=true`). Queries covered Gen9
  walker/local IDs, GPU authority, DMA arena/VT-d, and dispatch/storage types;
  results were used only for discovery.
- Direct internal sources: `docs/ADR-077-gpu-p1-compute-service.md`, ADR-032,
  ADR-045, ADR-050, ADR-076, the P1/P2 cross-check inputs,
  `kernel/src/drivers/accel/{mod.rs,accel_intel.rs}`,
  `kernel/src/drivers/block/dma.rs`, `kernel/src/drivers/dma_arena.rs`,
  `kernel/src/sched/{process.rs,gpu_submit_cap_logic.rs}`, and
  `kernel/src/arch/x86_64/syscall.rs`.
- Exact toolchain bytes were read directly from
  `tools/intel-gen9-finalizer/{Dockerfile,build-kernels.sh}`.
- The byte-oracle search was performed over `docs/` and `Plans/` on
  `origin/main`; no literal ADR-077 fixture was found. No generated or inferred
  bytes are presented as a recomputation of a source fixture that does not
  exist.
