<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Phase 6 BUILD-4b-2 OptiPlex Evidence — GPU GELU + forward-pass-equivalence + migration **PASS** (closes Phase 6)

Date: 2026-06-07 (evening) -04:00

Branch: `codex/phase-6-build-4b2-intel-gelu-and-close-jarvis`

Commit: `0a061be` (`phase6: BUILD-4b-2 fix - GELU full SIMD32 dispatch (Stage-B iter)`)

Scenario: `accel_intel_add_bringup` (extended for 4b-2)

Hardware: Dell OptiPlex 7080, Intel UHD 630 / Gen9.5, PCI `00:02.0`, device ID `0x9bc8`

Dispatch: `Plans/DISPATCH-2026-06-07-phase-6-build-4b2-intel-gelu-and-close-jarvis.md`

## Result — **PASS**: GELU on the GPU + the integration close retire on silicon. **Phase 6 is closed.**

BUILD-4b-2 landed one transcendental (GELU) on the iGPU and did the integration close
(forward-pass-equivalence + tensor migration) on the proven 4a/4b-1 substrate. The
OptiPlex run retired Add (regression) → two matmuls (4b-1 regression) → **GELU on 8
spread inputs vs host-f32 (non-vacuous)** → the mixed transformer forward (within the
accumulated bound of host-f32, pure-CPU byte-exact) → cpu↔intel tensor migration.

## Two-run iteration (the blind-hardware loop)

- **Run #1 (commit `713f489`): FAIL** at GELU. The Add/matmul passed; GELU produced only
  lane 0 (`output[0]=gelu(-2)` correct to ~16 ULP, `output[1..7]=0`). Diagnosis from the
  gelu/matmul disasm: GELU's payload was correct (count@`r8.4`, loaded by cross=2); GELU was
  simply the **first >4-lane GPU dispatch ever run on this silicon** (Add=2, matmul=4 were
  the only lane-counts ever proven), and the hand-trimmed walker execution mask
  (`lane_mask(8)=0xFF`) dispatched only lane 0.
- **Run #2 (commit `0a061be`): PASS.** Fix: dispatch GELU as a **full SIMD32 thread**
  (`GELU_DISPATCH_LANES=32` → local_id `0..31`, walker mask `0xFFFFFFFF`) and rely on the
  GELU kernel's **own bounds check** (`if (idx >= count) return;`, count = N in the
  cross-thread payload) to mask the partial — what real NEO does. Idle lanes return before
  any A64 access, so it is safe (the bounds-check-less Add keeps its trimmed mask).

## Extraction

- Runtime log sink: Intel NVMe `/dev/nvme0n1` (INTEL SSDPEKKR256G8), NEO_LOG_v1 at offset `0x10000000`
- Pre-boot staging proof: device-prefix SHA256 `79a9ac85947cc141dec3284a82f59b21ae2b882db43b3d22bc29657e5b998282` == staging image (header-only); re-staged clean before this run
- Extracted via `BUILD-5b-staging/neo-extract-log --device /dev/nvme0n1`
- Extracted log: `Plans/evidence/accel_intel_add_bringup.optiplex.build4b2.extracted.log`
- Extracted log SHA256: `35ff3a780b69dec302eb28bfab468f6d19e76349e726f46635fd561286d75b7d`
- Captured bytes: `144384`; no degradation marker

## Decisive extracted-log markers

- **Add (token 4) — regression, byte-exact:** `complete add token=4 output=[00,00,80,40, 00,00,c0,40]` = `[4.0, 6.0]`.
- **Exact matmul (token 5):** `[19,22,43,50]`, `matmul-exact … max_rel_e24=0 within_tol=true`.
- **Non-exact matmul (token 6):** `[0.19,0.22,0.43,0.50]`, `matmul-nonexact … max_rel_e24=0 tol_e24=4096 within_tol=true`.
- **GELU op (token 7) — the NON-VACUOUS tolerance proof:** inputs `[-2,-1,-0.5,0,0.5,1,2,3]`.
  - All 8 lanes computed (not just lane 0). IDD `cross_thread_read_grf=2 per_thread_read_grf=6`; `work_items=32` (full SIMD32 dispatch).
  - `gelu-op gpu=[b0,f7,39,bd, 90,9e,22,be, …] host_f32=[c0,f7,39,bd, 90,9e,22,be, …] max_rel_e24=1 tol_e24=4096 within_tol=true`.
  - **`max_rel_e24=1` > 0**: the Gen9 hardware extended-math `tanh` (math.exp/inv/fdiv) diverges from libm by ~1 unit of 2⁻²⁴ at `x=-2` (element 0: GPU `0xbd39f7b0` vs host `0xbd39f7c0`); elements 1–7 match libm bit-for-bit. The ≤2⁻¹² bound fires **non-vacuously** — exactly the point of landing GELU on silicon.
- **Forward-pass-equivalence (valid_transformer_block, embedded in the kernel image):**
  - `forward-equiv cpu=[ff,ff,7f,3e, 00,00,80,3e, fe,ff,7f,3e, fe,ff,7f,3e] mixed=[ff,ff,7f,3e, 00,00,80,3e, ff,ff,7f,3e, ff,ff,7f,3e] host_f32=[…] cpu_rel_e24=0 mixed_rel_e24=0 bound_e24=32768 ops=matmul+gelu_gpu/layernorm+rmsnorm+softmax_cpu`.
  - MatMul + Gelu ran on the GPU; LayerNorm/RmsNorm/Softmax fell back to `accel_cpu` (transparent). The pure-CPU forward is byte-exact to the Phase-5 oracle; both pure-CPU and mixed land within `B = 2⁻⁹` (`bound_e24=32768`) of host-f32 (the terminal softmax flattens output to `~[0.25;4]`, so the forward bound is satisfied with the op-level GELU case carrying the tolerance proof).
- **Tensor migration on real GPU:** `migrate copy=non_identity tensor_migrate=cpu_gpu_cpu host_allocs=17 host_frees=17 storage_freed=ok` (copy_to/from_device non-identity transfers, cpu→intel→cpu round-trip byte-exact, no leak).
- **Every dispatch:** `RING_FAULT=0x00000000`, `INSTDONE=0xfffffffe` (EU idle), seqno landed.
- **Verdict:** `NEO_TEST/1 event=PASS name=accel_intel_add_bringup accel_intel=ok stage=4b2 op=add+matmul+gelu add=[4,6] matmul_exact=[19,22,43,50] gelu_op=tanh_hw_vs_host_f32_nonvacuous forward=transformer_mixed_gpu_cpu_within_2^-9_of_host_f32 cpu_transparency=byte_exact migrate=cpu_intel_cpu_roundtrip fallback=unsupported_routes_cpu phase6=closed evidence=block_log_sink`.

## Gate status — **Stage B PASS, Phase 6 closed**

- **Stage A (Jarvis): green** — builds feature-on (ISO) + feature-off; 42 host tests (GELU
  payload + kernel-EOT + hardware-math disasm scan + tolerance + the embedded forward refs +
  Add/matmul regressions); `shard-coverage 4`; fmt; host clippy.
- **Stage B (PL, OptiPlex): PASS** — Add `[4,6]` → matmul (exact + non-exact) → GELU on 8
  inputs vs host-f32 (`max_rel_e24=1`, non-vacuous) → mixed transformer forward within `2⁻⁹`
  of host-f32 (pure-CPU byte-exact) → cpu↔intel migration round-trip. The GPU now runs
  matmul/add/gelu (the AI-characteristic math) with mixed-precision CPU fallback + migration,
  bare-metal verified.
- **Phase-6 close:** the accel driver class (CPU / sim / virtio + the real Intel GPU) is
  complete on silicon. The stateful-BTI / DRM-expressibility path remains a documented
  post-Phase-6 hardening marker (ADR-023). One latent follow-up surfaced: matmul's GPU slice
  is only *proven* at ≤4 lanes (it still uses the hand-trimmed mask); GELU's full-SIMD32 +
  bounds-check pattern should be back-ported to matmul/add to make their ≤32 slice real.
