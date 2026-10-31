# Phase 6 BUILD-4a Iter4 OptiPlex Evidence - accel_intel_add_bringup

Date: 2026-06-06 20:35 -04:00

Branch: `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`

Commit: `b2dfaed5587c46d777d577c8399ad72bb539d934`

Scenario: `accel_intel_add_bringup`

Hardware: Dell OptiPlex UHD 630 candidate, PCI `00:02.0`, device ID `0x9bc8`

## Extraction

- Runtime log sink: Intel NVMe `/dev/disk/by-id/nvme-INTEL_SSDPEKKR256G8_BTHH82050XNA256B`
- Declared log offset: `0x10000000`
- Magic scan: `268435456:NEO_LOG_v1`
- Pulled region: `target/neo-test/accel_intel_add_bringup.optiplex.iter4.nvme-log-region.raw`
- Region size: `16777216` bytes
- Region SHA256: `c7094633f0055dcad8c8a1c02d5c9cc9a08ff4b55ff5edb8911b4100468a947f`
- Extracted log: `Plans/evidence/accel_intel_add_bringup.optiplex.iter4.extracted.log`
- Extracted log SHA256: `8186f68aad649921987b026e3dd8461143e36f59a4b525a1012c036d5a104c82`
- Extractor result: `Found NEO_LOG_v1 header at offset 0x0`; scenario `accel_intel_add_bringup`; captured bytes `39838`

## Result

Iter4 localized the failure to compute dispatch / EU retirement, not legacy-ring execution.

Decisive extracted-log markers:

- Ring-only smoke submitted and completed:
  - `ring-batch mode=smoke token=2 ring_ggtt=0x100000 byte_offset=0x0 command_len=28 tail=0x1c`
  - `smoke-complete-status token=2 RING_HEAD=0x00000018 RING_TAIL=0x00000018 ... seqno=0x00000002`
  - `ring-smoke complete token=2 smoke-seqno=0x00000002`
- Compute Add setup was internally consistent:
  - `add-kernel-eot source='ret (M1, 1)' pinned_tail_match=true`
  - `interface-descriptor kernel_start=0x00120000 expected_kernel_ggtt=0x00120000 kernel_ptr_matches=true ...`
  - `ring-batch mode=compute token=3 ring_ggtt=0x100000 byte_offset=0x0 command_len=168 tail=0xa8`
- Compute Add did not retire:
  - `pre-kick-status token=3 RING_HEAD=0x00000050 RING_TAIL=0x00000000 ... ACTHD=0x0000000000000050 ... seqno=0x00000000`
  - `post-kick-status token=3 RING_HEAD=0x00000050 RING_TAIL=0x000000a8 ... ACTHD=0x0000000000000050 ... seqno=0x00000000`
  - `wait-mid-status token=3 RING_HEAD=0x00000050 RING_TAIL=0x000000a8 ... ACTHD=0x0000000000000050 ... seqno=0x00000000`
  - `wait-timeout token=3 RING_HEAD=0x00000050 RING_TAIL=0x000000a8 ... ACTHD=0x0000000000000050 ... seqno=0x00000000`
  - `NEO_TEST/1 event=FAIL name=accel_intel_add_bringup reason=accel_intel_add_wait_failed`

No extracted `PANIC`, `BUG`, or degradation marker was observed. The scenario failed intentionally at the Add wait gate after proving the ring-only seqno path.

