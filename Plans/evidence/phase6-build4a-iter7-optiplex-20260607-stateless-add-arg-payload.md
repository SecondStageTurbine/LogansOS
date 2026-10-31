<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Phase 6 BUILD-4a Iter7 OptiPlex Evidence - accel_intel_add_bringup

Date: 2026-06-07 10:40 -04:00

Branch: `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`

Commit: `454ec84` (`phase6: intel iter7 stateless-A64 arg payload + fault-address capture`)

Scenario: `accel_intel_add_bringup`

Hardware: Dell OptiPlex 7080, Intel UHD 630 / Gen9.5, PCI `00:02.0`, device ID `0x9bc8`

Dispatch: `Plans/DISPATCH-2026-06-07-phase-6-build-4a-iter7-stateless-add-arg-payload-jarvis.md` (incl. ISC-5a/5b)

## Extraction

- Runtime log sink: Intel NVMe `/dev/disk/by-id/nvme-INTEL_SSDPEKKR256G8_BTHH82050XNA256B` (`/dev/nvme0n1`)
- Declared log offset: `0x10000000`
- Pre-boot staging proof (NVMe written + fsync'd before the Neo run): device-prefix SHA256 `79a9ac85947cc141dec3284a82f59b21ae2b882db43b3d22bc29657e5b998282` == staging image (`accel_intel_add_bringup.nvme-log.raw`, header-only; kernel writes the log at runtime)
- Pulled region: `target/neo-test/accel_intel_add_bringup.optiplex.iter7.nvme-log-region.raw` (16 MiB window, `skip=256 count=16` MiB; not committed)
- Extracted log: `Plans/evidence/accel_intel_add_bringup.optiplex.iter7.extracted.log`
- Extracted log SHA256: `7086936095742407da4352ee490eb389dd60d54e0ee5848a4323e36cc0fa785e`
- Extractor result: `Found NEO_LOG_v1 header at offset 0x0`; scenario `accel_intel_add_bringup`; captured bytes `62397`; no degradation marker

## Result — CONCLUSIVE via fault-address capture: pointers landed in the WRONG GRF (ISC-5b branch 3)

iter7 wired the stateless-A64 cross-thread argument payload (CURBE) and the
ISC-5a/5b fault-address capture in the same commit so this run is conclusive
either way. It is: the Add does **not** retire `[4,6]`, but the fault is now
**fully localized** — the payload reaches the GPU and the only remaining defect is
the GRF placement of the cross-thread pointers.

### What is confirmed WORKING on silicon (all iter7 wiring present)

- No-op retires again (dispatch path still silicon-proven):
  - `noop-kernel complete token=3 noop-kernel-seqno=0x00000003 bisect=dispatch_ok_addbody_suspect`
  - `accel_intel_bisect=noop_retired verdict=dispatch_ok suspect=B_add_eu_data_access`
- The Add carries the new 73-DWord batch: `ring-batch mode=compute token=4 ... command_len=292 tail=0x124`
- The CURBE page is mapped: `ggtt-pte label=curbe ggtt=0x134000 phys=0x47a380000 ... offset=0x8009a0`
- The IDD declares the cross-thread read: `interface-descriptor ... thread_group_threads=1 cross_thread_read_grf=3 stateless=true(no sampler/binding-table)`
- `MEDIA_CURBE_LOAD` raised no fault (the CURBE load itself is fine); the Add reaches the EU (the command streamer drains the batch: `RING_HEAD=RING_TAIL=ACTHD=0x00400120`).

### The decisive marker (ISC-5a fault-address decode)

```
ring-fault-decode wait-timeout token=4 RING_FAULT=0x000009e1 valid=true
  fault_type=0 (invalid-PTE/not-present) srcid=0x3c addr_space=GGTT
  fault_gpu_va=0xffb1ce9bb000 data0=0xfb1ce9bb data1=0x0000001f in_build4a_window=false
```

(identical across `post-kick-status`, `wait-mid-status`, `wait-timeout` — stable fault.)

- `pre-kick-status token=4 ... RING_FAULT=0x00000000 seqno=0x0` → no fault before the kick; the EU spins up (`INSTDONE=0xffddffff`, bits 17 + 21) then faults.
- `NEO_TEST/1 event=FAIL name=accel_intel_add_bringup reason=accel_intel_add_wait_failed`

## Interpretation — ISC-5b discriminator: branch 3 (wrong GRF), NOT the address-space risk

- **`fault_gpu_va=0xffb1ce9bb000`, `in_build4a_window=false`.** The faulting address is
  **garbage** — not one of our buffers (`lhs=0x130000`, `rhs=0x131000`, `output=0x132000`,
  all in `0x0013_xxxx`). The Add read its `a` base pointer from `r8.0` (per
  `add_f32x2.gen9.asm`) and got `0xfb1ce9bb…` instead of `0x130000`. The cross-thread CURBE
  pointers **did not land at `r7..r9`** where the kernel reads them.
- **This RULES OUT the iter7-prime address-space risk (ISC-5b branch 2).** Had the payload
  been placed correctly, the A64 deref of `0x0013_0xxx` would have either succeeded or
  faulted *in-window*; instead it faulted out-of-window. Moreover `addr_space=GGTT` (DATA1
  bit 4 set) confirms the A64 data port **does** translate through GGTT — the same path
  instruction fetch uses — so GGTT-vs-PPGTT is not the blocker.
- **Root cause = the Gen9 GPGPU thread-payload GRF layout.** `add_f32x2.gen9.asm` reads
  per-lane `local_id.x` from `r1/r2` and the uniform A64 pointers from `r8.0/r8.1/r8.2`
  (with `enqueued_local_size` at `r9.0`), i.e. it expects `r0` header + **6-GRF per-thread
  `local_id` (`r1..r6`)** + **3-GRF cross-thread (`r7..r9`)**. iter7 declared the
  cross-thread read length (3) and loaded the CURBE, but did **not** make the dispatcher
  emit/account for the 6-GRF per-thread `local_id` block, so the cross-thread data is placed
  at the wrong offset and `r8` holds garbage. The bug is upstream of the kernel body, in the
  payload-assembly programming (GPGPU_WALKER local-ID generation + the IDD per-thread read
  length / cross-thread placement), not in the CURBE contents (which are byte-correct, see
  the hand-laid payload oracle).
- **ISC-9 (cosmetic) confirmed again, still not load-bearing.** `noop-pre-kick-status
  token=3 seqno=0x3` recurred; the driver zeroes the seqno at construction and completion is
  gated on the EU-idle INSTDONE, so it cannot fake a retire. No fix needed.

## iter8 scope (recommended) — fix the cross-thread GRF placement

Focus on the Gen9 GPGPU thread-payload layout so the cross-thread pointers land at `r7..r9`:
make the `GPGPU_WALKER` generate the per-thread `local_id` (6 GRF for SIMD32 × 3 dims) and/or
set the IDD per-thread **Constant/Indirect URB Entry Read Length** so the dispatcher reserves
`r1..r6` for `local_id` and places the 3-GRF cross-thread CURBE at `r7..r9`. This is a
focused PRM/reference investigation (KBL PRM Vol. 2a INTERFACE_DESCRIPTOR_DATA + GPGPU_WALKER
payload layout; cross-check i965 `brw_emit_gpgpu_walker` / compute-runtime
`EncodeDispatchKernel` local-ID emit) — recommend a Cerebro substrate-survey on the Gen9
local-ID / cross-thread payload model before the iter8 BUILD. Success unchanged:
`output_base == [4.0f, 6.0f]` with the seqno landed → BUILD-4a→main. The fault-address
capture stays in place to keep the next run self-diagnosing.

No `PANIC`/`BUG`/degradation marker extracted. The scenario failed intentionally at the Add
wait gate after the fault-address decode produced its conclusive verdict.

## Gate status (per dispatch §4)

- **Stage A (Jarvis): green** — builds feature-on (ISO) + feature-off; re-laid 292-byte
  command oracle + hand-laid payload oracle + both EU-kernel-EOT tests; `shard-coverage 4`;
  fmt; host clippy. Committed `454ec84`, pushed to `origin/codex/phase-6-build-4a-intel-gpu-bringup-jarvis`.
- **Stage B (PL): conclusive, not a pass.** The Add does not compute `[4,6]`; the fault is
  localized to the cross-thread GRF placement (iter8). The branch does **not** merge.
