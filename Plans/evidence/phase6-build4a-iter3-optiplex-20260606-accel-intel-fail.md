# Phase 6 BUILD-4a Iter3 OptiPlex Evidence - accel_intel_add_bringup FAIL

Date: 2026-06-06
Scenario: `accel_intel_add_bringup`
Hardware: OptiPlex 7080, Intel UHD 630 / Gen9.5 candidate `00:02.0`, NVMe log sink

## Artifacts

- Raw NVMe readback: `target/neo-test/accel_intel_add_bringup.optiplex.nvme-log.raw`
  - SHA256: `364D653633E468683CBAF893E29F83FD70FFD065C50252CF85C1827D2C51D32C`
- Extracted NEO_LOG: `Plans/evidence/accel_intel_add_bringup.optiplex.extracted.log`
  - SHA256: `F63ED1694D2A42D7A3AA4E660C8F606F66F641EC9204402C55DE2356E640CD32`

## Extraction

Command:

```text
docker compose run --rm neo cargo run -p neo-extract-log -- --device target/neo-test/accel_intel_add_bringup.optiplex.nvme-log.raw --output Plans/evidence/accel_intel_add_bringup.optiplex.extracted.log
```

Extractor result:

```text
Found NEO_LOG_v1 header at offset 0x10000000
Scenario: accel_intel_add_bringup
Captured bytes: 23412
Wrote Plans/evidence/accel_intel_add_bringup.optiplex.extracted.log
```

## Result

The scenario reached the accel-intel dispatch path and failed cleanly:

```text
NEO_TEST/1 event=FAIL name=accel_intel_add_bringup reason=accel_intel_add_wait_failed
```

Key submit/wait state:

```text
submit-status token=2 RING_HEAD=0x00000050 RING_TAIL=0x000000a8 FORCEWAKE_ACK=0x00000001 GT_FORCEWAKE_ACK=0x00000001 RING_CTL=0x00000001 RING_MODE=0x00000000 execlist_enabled=false INSTDONE=0xffdeffff ACTHD=0x0000000000000050 R_PWR_CLK_STATE=0x00000000 RING_FAULT=0x00000000 GT_CORE_STATUS=0x00800000 seqno=0x00000000
wait-timeout token=2 RING_HEAD=0x00000050 RING_TAIL=0x000000a8 FORCEWAKE_ACK=0x00000001 GT_FORCEWAKE_ACK=0x00000001 RING_CTL=0x00000001 RING_MODE=0x00000000 execlist_enabled=false INSTDONE=0xffdeffff ACTHD=0x0000000000000050 R_PWR_CLK_STATE=0x00000000 RING_FAULT=0x00000000 GT_CORE_STATUS=0x00800000 seqno=0x00000000
```

Interpretation: forcewake acknowledges, GGTT PTE writes happen, legacy ring mode is selected, the ring is enabled, and the tail write is visible. The engine does not complete the seqno write before timeout.
