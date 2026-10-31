<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Phase 6 BUILD-4a Iter8 OptiPlex Evidence — accel_intel_add_bringup **PASS**

Date: 2026-06-07 (afternoon) -04:00

Branch: `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`

Commit: `733035d` (`phase6: intel iter8 NEO Gen9 indirect-data thread payload (per-thread local_id)`)

Scenario: `accel_intel_add_bringup`

Hardware: Dell OptiPlex 7080, Intel UHD 630 / Gen9.5, PCI `00:02.0`, device ID `0x9bc8`

Dispatch: `Plans/DISPATCH-2026-06-07-phase-6-build-4a-iter8-gpgpu-thread-payload-layout-jarvis.md`

## Result — **PASS**: the Intel Gen9 EU computed `[1,2] + [3,4] = [4,6]`

This is the **BUILD-4a→main trigger**. iter8 replaced the iter7 `MEDIA_CURBE_LOAD`
path with the NEO Gen9 **indirect-data** dispatch (the convention the IGC/ocloc blob
was built for; ADR-032 ISC-1 field map), delivering the per-thread `local_id` block
the kernel needs at `r1..r6` and the cross-thread A64 pointers at `r7..r9`. The real
UHD 630 EU executed the stateless-A64 Add and returned the oracle result.

## Extraction

- Runtime log sink: Intel NVMe `/dev/disk/by-id/nvme-INTEL_SSDPEKKR256G8_BTHH82050XNA256B` (`/dev/nvme0n1`)
- Declared log offset: `0x10000000`
- Pre-boot staging proof (NVMe written + fsync'd before the Neo run): device-prefix SHA256 `79a9ac85947cc141dec3284a82f59b21ae2b882db43b3d22bc29657e5b998282` == staging image (header-only; kernel writes the log at runtime)
- Pulled region: `target/neo-test/accel_intel_add_bringup.optiplex.iter8.nvme-log-region.raw` (16 MiB window, `skip=256 count=16` MiB; not committed)
- Extracted log: `Plans/evidence/accel_intel_add_bringup.optiplex.iter8.extracted.log`
- Extracted log SHA256: `2cf58dd46b2e9ba2d26e15634d32dfbea40d631a782e4fc2a4f02b621875075b`
- Extractor result: `Found NEO_LOG_v1 header at offset 0x0`; scenario `accel_intel_add_bringup`; captured bytes `56503`; no degradation marker

## Decisive extracted-log markers

- No-op bisector retires (dispatch path good, as every iter since iter6):
  - `noop-kernel complete token=3 noop-kernel-seqno=0x00000003 bisect=dispatch_ok_addbody_suspect`
  - no-op IDD `cross_thread_read_grf=0 per_thread_read_grf=0` (byte-identical iter6 bisector)
- The iter8 indirect-data heap is assembled correctly (new ISC-3/§3 breadcrumb):
  - `indirect-heap curbe_ggtt=0x134000 indirect_len=320 cross_a=0x0000000000130000 local_id_lane1=1 (cross[0,96) + per_thread[96,288))`
- The Add IDD now carries both read lengths (NEO `setGrfInfo`):
  - `interface-descriptor kernel_start=0x00120000 ... thread_group_threads=1 cross_thread_read_grf=3 per_thread_read_grf=6 stateless=true(no sampler/binding-table)`
- The Add batch is the 69-DWord indirect-data form:
  - `ring-batch mode=compute token=4 ... command_len=276 tail=0x114`
- **The Add EU runs clean — no fault, EU idle, seqno landed:**
  - `pre-kick-status token=4 ... RING_FAULT=0x00000000 seqno=0x0`
  - `post-kick-status token=4 RING_HEAD=0x00400110 ... INSTDONE=0xfffffffe ... RING_FAULT=0x00000000 seqno=0x0` (the iter6/7 `RING_FAULT=0x9e1` is **gone**; INSTDONE returns to idle, no stuck EU bits)
  - `wait-complete token=4 ... INSTDONE=0xfffffffe RING_FAULT=0x00000000 seqno=0x00000004`
- **The output is the oracle:**
  - `complete add token=4 output=[00, 00, 80, 40, 00, 00, c0, 40]` = `0x40800000, 0x40c00000` = **`4.0f, 6.0f`**
  - `NEO_TEST/1 event=PASS name=accel_intel_add_bringup accel_intel=ok stage=4a op=add input=[1,2]+[3,4] output_bits=[0x40800000,0x40c00000] tolerance_ulp=0 evidence=block_log_sink`

## Interpretation

- **The NEO indirect-data fix was correct and complete.** iter7 read its pointer from
  `r8` and got garbage (`fault_gpu_va=0xffb1ce9bb000`) because the 6-GRF per-thread
  `local_id` block was never built and cross-thread landed at `r1`. iter8 builds the
  Indirect Object Heap `[cross 96 B][per-thread 192 B]`, points `GPGPU_WALKER` DW2/DW3
  at it (`indirect_len=320`, start `0x134000`), and declares the IDD read lengths
  (cross=3, per-thread=6). The dispatcher then lands per-thread at `r1..r6` and
  cross-thread at `r7..r9`, so the kernel's `r8.0/.1/.2` resolve to the real
  `lhs/rhs/output` GGTT addresses — `RING_FAULT=0` and the A64 gather/scatter produce
  `[4,6]`.
- **The address-space question (iter7 ISC-5b) is closed for good:** `RING_FAULT=0`
  with the payload correctly placed confirms A64 stateless data access resolves
  through GGTT in legacy-ring mode, exactly as `addr_space=GGTT` indicated.
- The whole dispatch stack — PCI probe, forcewake, GGTT mapping, legacy ring,
  `STATE_BASE_ADDRESS` (iter6), stateless-A64 + indirect-data payload (iter8) — is
  now silicon-proven end-to-end.

## Gate status (per dispatch §4/§6) — **Stage B PASS**

- **Stage A (Jarvis): green** — builds feature-on (ISO) + feature-off; local_id +
  re-laid command + cross-thread + EU-EOT oracles; `shard-coverage 4`; fmt; host clippy.
- **Stage B (PL, OptiPlex): PASS** — `output == [4.0f, 6.0f]`, seqno landed,
  `RING_FAULT=0`. This is the BUILD-4a→main trigger.
- **Merge:** worker close-gate (rebase onto `origin/main`, re-run Stage A, push), then
  PM `scripts/pm-merge-check.sh` + `--ff-only` → `main`. **BUILD-4a closes.**
