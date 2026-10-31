<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Phase 6 BUILD-4a Iter5 OptiPlex Evidence - accel_intel_add_bringup

Date: 2026-06-06 21:51 -04:00

Branch: `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`

Commit: `bd15863` (`phase6: add intel iter5 no-op EOT kernel A/B bisect`)

Scenario: `accel_intel_add_bringup`

Hardware: Dell OptiPlex 7080, Intel UHD 630 / Gen9.5, PCI `00:02.0`, device ID `0x9bc8`

## Extraction

- Runtime log sink: Intel NVMe `/dev/disk/by-id/nvme-INTEL_SSDPEKKR256G8_BTHH82050XNA256B`
- Declared log offset: `0x10000000`
- Magic scan: `268435456:NEO_LOG_v1`
- Pulled region: `target/neo-test/accel_intel_add_bringup.optiplex.iter5.nvme-log-region.raw` (16 MiB window, `skip=256 count=16` MiB; not committed)
- Region size: `16777216` bytes
- Region SHA256: `07296750c1500db502901688d0fa8079fccd5f75b9dbda1ee23cc42da63ec047`
- Extracted log: `Plans/evidence/accel_intel_add_bringup.optiplex.iter5.extracted.log`
- Extracted log SHA256: `3652128cc0f7efc656005b768bad4e3d31e46f308d4f75c63ae00fecbf07eaef`
- Extractor result: `Found NEO_LOG_v1 header at offset 0x0`; scenario `accel_intel_add_bringup`; captured bytes `61895`
- Pre-boot staging proof (NVMe written before the Neo run): `dd` + `cmp=ok`, device-prefix SHA256 `79a9ac85947cc141dec3284a82f59b21ae2b882db43b3d22bc29657e5b998282`

## Result — bisect verdict: SUSPECT A (dispatch setup), NOT EU data access

Iter5 separated the two iter4 suspects with a hand-laid EOT-only no-op kernel
(no memory access) dispatched through the same VFE/interface-descriptor/
GPGPU_WALKER path as Add. **The no-op also hung**, identically to Add — which
eliminates suspect B (EU data-port addressing) and localizes the failure to the
dispatch setup itself.

Decisive extracted-log markers:

- Ring-only smoke submitted and completed (legacy ring still good):
  - `ring-smoke complete token=2 smoke-seqno=0x00000002`
  - `smoke-complete-status token=2 RING_HEAD=0x18 ... INSTDONE=0xfffffffe ... SC_INSTDONE=0xffffffff SAMPLER_INSTDONE=0xffffffff seqno=0x2`
- No-op EOT-only kernel hung (the bisector):
  - `noop-kernel-eot source='send(8) null g127 EOT thread_spawner mlen 1 rlen 0' eot_present=true`
  - `noop-pre-kick-status token=3 RING_HEAD=0x50 ... INSTDONE=0xffdeffff ACTHD=0x50 ... SC_INSTDONE=0xffffffff SAMPLER_INSTDONE=0xffffffff ROW_INSTDONE=0x00008040 TD_CTL=0x0 seqno=0x0`
  - `instdone-decode noop-pre-kick-status token=3 INSTDONE=0xffdeffff busy_mask=0x00210000 compute_ref_match=true`
  - `instdone-busy-bit ... bit=16 unit=eu-thread-dispatch(row0)`
  - `instdone-busy-bit ... bit=21 unit=eu-instruction-fetch(row0)`
  - `noop-kernel timeout token=3 noop-kernel-seqno=0x00000000 bisect=dispatch_setup_suspect`
  - `accel_intel_bisect=noop_hung verdict=dispatch_setup suspect=A_vfe_walker_descriptor`
- Add compute also did not retire (unchanged from iter4):
  - `wait-timeout token=4 RING_HEAD=0x50 RING_TAIL=0xa8 ... INSTDONE=0xffdeffff ACTHD=0x50 ... seqno=0x0`
  - `NEO_TEST/1 event=FAIL name=accel_intel_add_bringup reason=accel_intel_add_wait_failed`

## Interpretation

- **(B) eliminated.** The no-op performs no `lhs`/`rhs`/`output` access, yet parks
  at the byte-identical signature as Add (`RING_HEAD=ACTHD=0x50`, `INSTDONE=0xffdeffff`,
  `seqno=0`). The EU data port / surface state / stateless A64 / PPGTT / MOCS path is
  not the cause.
- **(A) localized.** New ISC-2 EU decode: `SC_INSTDONE=0xffffffff` and
  `SAMPLER_INSTDONE=0xffffffff` (slice-common + sampler/data-port **idle**); the only
  stuck units are row-0 EU **thread-dispatch** (`INSTDONE` bit 16) and **instruction
  fetch** (bit 21). `TD_CTL=0`, `RING_FAULT=0`. The thread reaches the EU but its
  instruction fetch never completes → the bug is in `MEDIA_VFE_STATE` (thread pool /
  URB / scratch), the `GPGPU_WALKER` params, and/or the interface-descriptor kernel
  pointer — upstream of the kernel body.
- **Residual caveat.** Two interpretations could each yield "EU cannot execute": (A)
  dispatch setup is wrong, or (A') both embedded blobs are malformed. The Add blob is
  the ASCII-string placeholder; the no-op is a structured hand-laid EOT — two very
  different blobs producing a byte-identical stuck signature argues strongly for (A),
  upstream of instruction decode. The `bit=16/21` unit *names* are best-effort
  annotations; the bit *positions* and `busy_mask=0x210000` are mechanical fact. iter6
  can fully disambiguate with a finalizer-built known-good EOT.

No extracted `PANIC`, `BUG`, or degradation marker was observed. The scenario failed
intentionally at the Add wait gate after the no-op bisect produced its A/B verdict.

## iter6 scope (recommended)

Focus on dispatch setup, not data access: `MEDIA_VFE_STATE` (thread count / URB
allocation / scratch), `GPGPU_WALKER` dimensions/enables, and the interface
descriptor's kernel-pointer/format. Likely needs Prometheus to add the real Intel
finalizer (IGC/ocloc/vISA) to the toolchain so a known-good EOT kernel confirms (A)
definitively and rules out (A').
