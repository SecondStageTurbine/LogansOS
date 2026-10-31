<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Phase 6 BUILD-4a Iter6 OptiPlex Evidence - accel_intel_add_bringup

Date: 2026-06-07 01:35 -04:00

Branch: `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`

Commit: `41e0f10` (`phase6: intel iter6 STATE_BASE_ADDRESS dispatch-setup fix`)

Scenario: `accel_intel_add_bringup`

Hardware: Dell OptiPlex 7080, Intel UHD 630 / Gen9.5, PCI `00:02.0`, device ID `0x9bc8`

Dispatch: `Plans/DISPATCH-2026-06-06-phase-6-build-4a-iter6-state-base-address-jarvis.md` (+ 2026-06-07 Addendum)

## Extraction

- Runtime log sink: Intel NVMe `/dev/disk/by-id/nvme-INTEL_SSDPEKKR256G8_BTHH82050XNA256B` (`/dev/nvme0n1`)
- Declared log offset: `0x10000000`
- Pre-boot staging proof (NVMe written + fsync'd before the Neo run): device-prefix SHA256 `79a9ac85947cc141dec3284a82f59b21ae2b882db43b3d22bc29657e5b998282` == staging image (`accel_intel_add_bringup.nvme-log.raw`, identical to the iter5 proven sink image; header-only, kernel writes the log at runtime)
- Post-run header at sink: `NEO_LOG_v1` present at offset `0x10000000` (kernel wrote it during the run)
- Pulled region: `target/neo-test/accel_intel_add_bringup.optiplex.iter6.nvme-log-region.raw` (16 MiB window, `skip=256 count=16` MiB; not committed)
- Region SHA256: `3949dc47f136ce20d8a6db684fc20f938d3280d49d85fabd3b6cf9ecdcde95df`
- Extracted log: `Plans/evidence/accel_intel_add_bringup.optiplex.iter6.extracted.log`
- Extracted log SHA256: `a8bcd0cb744b8ecde7ee7e70110c758ddcbed480cec133126fef9885ffd38d0a`
- Extractor result: `Found NEO_LOG_v1 header at offset 0x0`; scenario `accel_intel_add_bringup`; captured bytes `59032`; no degradation marker

## Result — bisect verdict: dispatch_ok / SUSPECT B (Add EU data access)

iter6 emitted `STATE_BASE_ADDRESS` (Instruction + General + Dynamic + Surface
State Base, sizes maxed) before `GPGPU_WALKER`, corrected the walker to SIMD32 /
15-DW Gen9 encoding, rewrote `MEDIA_VFE_STATE` and the interface descriptor, and
re-laid the 276-byte command-byte oracle. The run validates against the
**finalizer-built** `NOOP_KERNEL_GEN9` (ocloc-disassembly-confirmed EOT).

**The finalizer no-op RETIRED — dispatch is fixed.** The bisect flips from iter5's
"both no-op and Add hang → SUSPECT A (dispatch setup)" to "no-op retires / Add
faults → SUSPECT B (Add EU data access)."

### Decisive extracted-log markers

Legacy ring still good (ring-only smoke retired):
- `smoke-complete-status token=2 RING_HEAD=0x18 RING_TAIL=0x18 ... INSTDONE=0xfffffffe ... RING_FAULT=0x00000000 seqno=0x2`
- `ring-smoke complete token=2 smoke-seqno=0x00000002`

Command buffer carries the new `STATE_BASE_ADDRESS` (both no-op and Add):
- `ring-batch mode=noop token=3 ring_ggtt=0x100000 byte_offset=0x0 command_len=276 tail=0x114`
- `interface-descriptor kernel_start=0x00120000 expected_kernel_ggtt=0x00120000 kernel_ptr_matches=true thread_group_threads=1 stateless=true(no sampler/binding-table/curbe)`
- `noop-kernel-eot source='send (1|M0) null r127 0x27 0x02000010 {EOT}' eot_present=true`

No-op retires through the full dispatch path (the iter6 win):
- `noop-pre-kick-status token=3 RING_HEAD=0x00200000 RING_TAIL=0x00000000 INSTDONE=0xfffffffe ... RING_FAULT=0x00000000 ... seqno=0x00000003`
- `noop-complete-status token=3 RING_HEAD=0x00200110 RING_TAIL=0x00000110 INSTDONE=0xfffffffe ACTHD=0x0000000000200110 RING_FAULT=0x00000000 SC_INSTDONE=0xffffffff SAMPLER_INSTDONE=0xffffffff ROW_INSTDONE=0x00008040 TD_CTL=0x00000000 seqno=0x00000003`
- `instdone-decode noop-complete-status token=3 INSTDONE=0xfffffffe busy_mask=0x00000001 compute_ref_match=false` (only command-streamer bit 0 — **no stuck EU bits**)
- `noop-kernel complete token=3 noop-kernel-seqno=0x00000003 bisect=dispatch_ok_addbody_suspect`
- `accel_intel_bisect=noop_retired verdict=dispatch_ok suspect=B_add_eu_data_access`

Add still does not retire — now with a GPU **page fault** (the new signature):
- `add-kernel-eot source='send (1|M0) null r127 0x27 0x02000010 {EOT}' pinned_tail_match=true`
- `submit add token=4 bdf=0x000200 device_id=0x9bc8 ring=0x100000 seqno_ggtt=0x133000`
- `ring-batch mode=compute token=4 ... command_len=276 tail=0x114`
- `pre-kick-status token=4 RING_HEAD=0x00400000 ... INSTDONE=0xfffffffe RING_FAULT=0x00000000 seqno=0x00000000`
- `post-kick-status token=4 RING_HEAD=0x00400110 RING_TAIL=0x00000110 INSTDONE=0xffddffff ACTHD=0x0000000000400110 RING_FAULT=0x000009e1 ... seqno=0x00000000`
- `wait-timeout token=4 ... INSTDONE=0xffddffff RING_FAULT=0x000009e1 ... seqno=0x00000000`
- `instdone-busy-bit wait-timeout token=4 bit=17 unit=eu-area-bit`
- `instdone-busy-bit wait-timeout token=4 bit=21 unit=eu-instruction-fetch(row0)`
- `NEO_TEST/1 event=FAIL name=accel_intel_add_bringup reason=accel_intel_add_wait_failed`

## Interpretation

- **Dispatch setup (SUSPECT A) is fixed and hardware-proven.** The load-bearing
  proof is the EU state change, independent of any seqno read: iter5's no-op left
  `INSTDONE=0xffdeffff` with EU row-0 thread-dispatch (bit 16) and instruction
  fetch (bit 21) **stuck**; iter6's no-op returns `INSTDONE=0xfffffffe` — every EU
  unit idle, only the command-streamer bit set — with `RING_HEAD` advanced to
  `RING_TAIL` (`0x110`) and `RING_FAULT=0`. An EU cannot go from stuck-at-fetch to
  idle unless it actually fetched and executed the kernel to its `{EOT}`. The
  missing `STATE_BASE_ADDRESS` (Instruction Base unset → kernel pointer resolved to
  nowhere) was the iter5 bug; emitting it resolves the fetch. The kernel's own
  wait-loop concurs: `noop-kernel complete … bisect=dispatch_ok_addbody_suspect`.
- **The remaining failure (SUSPECT B) is the Add EU body's data access.** The Add
  fetches and starts (command streamer drains the batch, EU spins up — bits 17/21),
  then raises `RING_FAULT=0x000009e1`. Decode: bit 0 = Valid; fault-type bits[2:1]
  = `0b00` = **invalid PTE / page not present**. The Add is stateless A64 and reads
  `lhs`/`rhs` + writes `output` from pointers that arrive in the cross-thread
  payload — which iter6 does **not** load (no `MEDIA_CURBE_LOAD`, no General State
  Base data, payload offsets `a=32/b=40/o=48` unfilled). With null/garbage A64
  pointers the gather/scatter touches an unmapped VA → invalid-PTE fault. This is
  exactly the iter7 scope.
- **Caveat (flagged, not load-bearing).** The no-op `pre-kick` read already showed
  `seqno=0x00000003` (its target value) before the kick, whereas the Add `pre-kick`
  correctly read `seqno=0x0`; both target the same seqno GGTT page `0x133000`. So
  the no-op's seqno value alone is **not** treated as proof of completion — the
  retirement verdict rests on the EU-idle `INSTDONE` transition + ring drain +
  `RING_FAULT=0` above. Worth a driver-code check on whether the no-op path
  pre-seeds / skips zeroing the seqno page (cosmetic; does not change the verdict).

No `PANIC`, `BUG`, or degradation marker was extracted. The scenario failed
intentionally at the Add wait gate after the no-op's dispatch_ok verdict.

## Gate status (per dispatch §3)

- **Stage A (Jarvis): green** — builds, re-laid 276-byte oracle, host-checks +
  `shard-coverage 4`, clippy `-D warnings`, fmt; scenario hardware-only. Committed
  `41e0f10`, pushed to `origin/codex/phase-6-build-4a-intel-gpu-bringup-jarvis`.
- **Stage B (PL): the iter6 win landed** — the finalizer no-op retires
  (`noop-kernel-seqno=0x3`, dispatch_ok). Per dispatch, iter6 does **not** merge on
  its own; the branch merges when the real Add computes `[4,6]` (iter7).

## iter7 scope (next, Jarvis)

Fill the stateless-A64 cross-thread payload so the Add's A64 pointers resolve:
`MEDIA_CURBE_LOAD` carrying `lhs@32 / rhs@40 / output@48` (8 B A64 each, into the
already-mapped GGTT buffers) + General State Base data, SIMD32 / 128 GRF
consistent with the now-correct `MEDIA_VFE_STATE`/`GPGPU_WALKER`. Reference:
`kernel/src/drivers/accel/intel_gen9_kernels/README.md` "Cross-thread argument
ABI" (oracle `add_f32x2.gen9.zeinfo`). Success = Add retires `[4,6]` →
BUILD-4a → main.
