<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Phase 6 BUILD-4b-1 OptiPlex Evidence — accel_intel op dispatcher + matmul **PASS**

Date: 2026-06-07 (evening) -04:00

Branch: `codex/phase-6-build-4b1-intel-op-dispatcher-jarvis`

Commit: `f5c80e6` (`phase6: BUILD-4b-1 add non-exact matmul Stage-B case (make 2^-12 non-vacuous)`)

Scenario: `accel_intel_add_bringup` (extended)

Hardware: Dell OptiPlex 7080, Intel UHD 630 / Gen9.5, PCI `00:02.0`, device ID `0x9bc8`

Dispatch: `Plans/DISPATCH-2026-06-07-phase-6-build-4b1-intel-op-dispatcher-matmul-jarvis.md` (+ §5 refinement d92e385)

## Result — **PASS**: the generalized op dispatcher + matmul retire on silicon

BUILD-4b-1 turned `IntelAccelDriver::dispatch` from Add-only into a kernel-table op
dispatcher (Add | Matmul) on the proven 4a substrate, with `accel_cpu` fallback for
uncovered ops. The OptiPlex run retired the Add (regression), two matmuls (exact +
non-exact) within tolerance, and routed an oversize matmul to `Unsupported`.

## Extraction

- Runtime log sink: Intel NVMe `/dev/disk/by-id/nvme-INTEL_SSDPEKKR256G8_BTHH82050XNA256B` (`/dev/nvme0n1`)
- Pre-boot staging proof: device-prefix SHA256 `79a9ac85947cc141dec3284a82f59b21ae2b882db43b3d22bc29657e5b998282` == staging image (header-only)
- Pulled region: `target/neo-test/accel_intel_add_bringup.optiplex.build4b1.nvme-log-region.raw` (16 MiB; not committed)
- Extracted log: `Plans/evidence/accel_intel_add_bringup.optiplex.build4b1.extracted.log`
- Extracted log SHA256: `7c16fab4e63685b9248f0d368ada538c7a372dd8f7ba37a691ef610de47c3b57`
- Captured bytes: `91538`; no degradation marker

## Decisive extracted-log markers

- **Add (token 4) — regression, byte-exact:** `complete add token=4 output=[00,00,80,40, 00,00,c0,40]` = `[4.0, 6.0]`.
- **Exact-integer matmul (token 5) — mode-B dim/stride isolation:**
  - `indirect-heap op=matmul … work_items=4`; `kernel-eot op=matmul eot_present=true`; IDD `cross_thread_read_grf=3 per_thread_read_grf=6`.
  - `complete add token=5 output=[00,00,98,41, 00,00,b0,41, 00,00,2c,42, 00,00,48,42]` = `0x41980000,0x41b00000,0x422c0000,0x42480000` = **`[19, 22, 43, 50]`**.
  - `matmul-exact gpu=[…] host=[…] max_rel_e24=0 within_tol=true`.
- **Non-exact matmul (token 6) — mode-C, the ≤2⁻¹² path fires on rounding inputs:**
  - `A=[0.1,0.2;0.3,0.4]·B=[0.5,0.6;0.7,0.8]`; reference is **native hardware f32** (separate mul+add), not accel_cpu soft-float.
  - `complete add token=6 output=[5c,8f,42,3e, af,47,61,3e, f6,28,dc,3e, 00,00,00,3f]` = `0x3e428f5c,0x3e6147af,0x3edc28f6,0x3f000000` = **`[0.19, 0.22, 0.43, 0.50]`**.
  - `matmul-nonexact gpu=[…] host=[…] max_rel_e24=0 tol_e24=4096 within_tol=true`.
- **Every dispatch:** `RING_FAULT=0x00000000`, `INSTDONE=0xfffffffe` (EU idle), seqno landed (4/5/6).
- **Verdict:** `NEO_TEST/1 event=PASS name=accel_intel_add_bringup accel_intel=ok stage=4b1 op=add+matmul add=[4,6] matmul_exact=[19,22,43,50] matmul_nonexact=[0.1,0.2;0.3,0.4]*[0.5,0.6;0.7,0.8] tol=2^-12_vs_host_f32 fallback=unsupported_routes_cpu evidence=block_log_sink`.

## Interpretation

- **The op dispatcher generalization works on silicon.** The same 4a substrate (legacy
  ring, GGTT, `STATE_BASE_ADDRESS`, NEO indirect-data payload) dispatches a *different*
  kernel (matmul) with a *different* cross-thread payload (a/b/c + m/n/k dims) and a
  *different* walker work-size (m·n=4 vs the Add's 2) — only the blob, payload bytes, and
  execution masks change, exactly as designed. The Add stays byte-exact (regression intact).
- **The tolerance claim is now real, and the delta is 0.** `max_rel_e24=0` on the non-exact
  case means the GPU f32 matched the native host-f32 reference bit-for-bit. The non-exact
  inputs (`0.1, 0.2, …`) are not exactly representable, so the dot-products genuinely round —
  the tolerance check ran where rounding occurs (non-vacuous) and the GPU tracked host f32
  exactly. This is consistent with `matmul_f32.gen9.asm`'s separate `add …:f` accumulation
  (no FMA), so GPU-f32 == host-f32 (separate mul+add). The `≤2⁻¹²` bound is validated with
  headroom, not assumed.
- **Fallback routing is correct.** The oversize matmul (m·n=64 > the slice's 32) returned
  `Unsupported`, so the executor routes it to `accel_cpu` (transparent fallback) rather than
  failing the batch.

## Gate status (per dispatch §5/§7) — **Stage B PASS**

- **Stage A (Jarvis): green** — builds feature-on (ISO) + feature-off; 38 host tests (matmul
  payload + kernel-EOT + tolerance oracles + Add regression); `shard-coverage 4`; fmt; host clippy.
- **Stage B (PL, OptiPlex): PASS** — Add `[4,6]` → exact matmul `[19,22,43,50]` → non-exact
  matmul within 2⁻¹² (`max_rel_e24=0`) → oversize → `Unsupported`. The dispatcher + matmul are
  silicon-proven.
- **Merge:** worker close-gate (rebase onto `origin/main`, re-run Stage A, push), then PM
  `scripts/pm-merge-check.sh` + `--ff-only` → `main`. BUILD-4b-1 closes; 4b-2 (remaining ops +
  forward-pass-equivalence + the model-disk transparency oracle + migration close) follows.
