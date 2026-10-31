# Phase 3 BUILD-5b — AHCI Lane Bare-Metal Re-run — PASS

**Date filed:** 2026-05-28 (rerun executed 2026-05-27 23:15–23:29 EDT)
**Hardware:** Dell OptiPlex 7080 Micro, Service Tag D4SHKD3, BIOS 1.37.0, SATA mode AHCI
**Scenario disk:** Samsung SSD 860 EVO 250 GB (`/dev/sda`)
**Log sink:** Intel SSDPEKKR256G8 NVMe (`/dev/nvme0n1`)
**Kernel commit under test:** `72a0a41c8d6ef6d8bb6e7981dbc928c8cface40c` — `kernel: initialize bsp apic before storage probe` (Jarvis fix for APIC base race; see [PM-DISPOSITION-2026-05-28-phase3-build5b-apic-fix-accepted.md](../PM-DISPOSITION-2026-05-28-phase3-build5b-apic-fix-accepted.md))

## Result — ALL FOUR ACCEPTANCE GATES GREEN

| Gate | Status | Evidence |
|------|--------|----------|
| 1. Framebuffer PASS (green) | ✅ | PL observed and confirmed verbally; framebuffer photographed |
| 2. Extracted PASS marker present | ✅ | `NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30` |
| 3. Captured bytes > 0 | ✅ | **9202 bytes** captured (compare to prior failure: 0 bytes) |
| 4. No Degradation detected block | ✅ | Extractor stdout contains no `Degradation detected:` block |

## Provisioning chain-of-custody (Ubuntu Live, prior to scenario boot)

- Stick C (data, label `Win122h2`) mounted at `/run/media/ubuntu/Win122h2/` with BUILD-5b-staging folder
- Samsung SATA confirmed at `/dev/sda` (`lsblk` → `Samsung SSD 860 EVO 250GB`)
- Intel NVMe confirmed at `/dev/nvme0n1` (`lsblk` → `INTEL SSDPEKKR256G8`)
- `sudo dd if=$STAGING/ahci/ahci-scenario-disk.raw of=/dev/sda bs=4M conv=fsync` — 128 MiB at 107 MB/s
- `sudo dd if=$STAGING/ahci/ahci-log-sink-disk.raw of=/dev/nvme0n1 bs=4M conv=fsync` — 288 MiB at 119 MB/s
- `sudo sync`
- Magic-byte check: `/dev/sda` → `NEOFSv1` at offset 0; `/dev/nvme0n1` → `NEOPLUTARCH_PHAS...` (pre-init sentinel — per procedure correction, NEO_LOG_v1 is NOT expected in the runtime region preboot)
- Byte-for-byte cmp against both raw files: clean (both raws compared to their full length with zero byte differences before EOF on source)

## Staging artifact SHA256 (Jarvis-reported, PM-independently-verified via `Get-FileHash`)

```
f6a1f44f84cc7e7a1c270f1392d106cde8e8c648f2fd952764961c2602a4a411  BUILD-5b-staging/ahci/ahci-scenario.iso
77c4ef29e7184d49deec91db8a47588bdcdde1297d0372f13824efffe0db1083  BUILD-5b-staging/ahci/ahci-scenario-disk.raw
79a9ac85947cc141dec3284a82f59b21ae2b882db43b3d22bc29657e5b998282  BUILD-5b-staging/ahci/ahci-log-sink-disk.raw
dd75e1335c722d0cfbf078061244e8037111e7273d095947d14d4803d00ae9c7  BUILD-5b-staging/neo-extract-log
```

## Scenario kernel boot evidence (serial)

**Source PuTTY log (canonical):** `phase2-bare-metal-optiplex-7080-20260527-191028-12-irqsafe_mutex_bench.serial.log` in this same `Plans/evidence/` directory.
*Naming note: PuTTY session began with the `irqsafe_mutex_bench` scenario at 19:11 EDT and remained open across subsequent scenario reboots, appending all kernel serial output to the same file. The file therefore contains the smoke-test scenarios AND the Phase 3 BUILD-5b old-kernel failure AND both NEW-kernel reruns. Filename was retained for chain-of-custody continuity rather than renamed midstream.*

**Key lines in that log (574 lines total, 39533 bytes):**

| Line | Content | Significance |
|------|---------|--------------|
| 215 | `panicked at kernel/src/arch/x86_64/apic.rs:125:5: assertion 'left != right' failed: apic: local APIC base not initialized` | OLD-kernel boot reproduces the Jarvis-fixed failure on real hardware |
| 220 | `NEO_TEST/1 event=FAIL name=phase3_ahci_neofs_mount_and_readwrite reason=kernel_panic` | OLD-kernel failure event |
| 395 | `NEO_TEST/1 event=BEGIN name=phase3_ahci_neofs_mount_and_readwrite phase=dispatch` | NEW kernel (commit 72a0a41) starts AHCI scenario |
| 397 | `NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30` | **AHCI lane scenario PASS on bare metal** |

**APIC-init ordering proof (new-kernel boot):** the new boot emits, in order:

```
[ kernel ] slab: initialized per-CPU state cpu=0
[ kernel ] apic: cpu=0 lapic_id=0 enabled xAPIC (BSP, base_phys=0x00000000fee00000)
[ kernel ] block: init (generic block-device substrate)
[ kernel ] ahci: attached bdf=00:17.0 port=0 sectors=488397168 sector_size=512 msi_configured=true
[ kernel ] nvme: attached bdf=01:00.0 nsid=1 sectors=500118192 sector_size=512 doorbell_stride=4 msix=true msi=false
```

APIC base is initialized BEFORE `block::init`, so MSI/MSI-X delivered during AHCI/NVMe probe can issue EOI safely. This is precisely the ordering Jarvis's fix established.

## Extraction evidence

```
$ sudo $STAGING/neo-extract-log --device /dev/nvme0n1 --output /tmp/ahci-run-log.txt
Found NEO_LOG_v1 header at offset 0x10000000
Scenario: phase3_ahci_neofs_mount_and_readwrite
Start timestamp ns: 0
Captured bytes: 9202
Wrote /tmp/ahci-run-log.txt
```

- Magic header found at the **default** offset (`0x10000000`) — no offset override needed
- 9202 bytes captured (prior failure was 0)
- No `Degradation detected:` block in extractor stdout

**Saved log on disk:** `Plans/evidence/phase3-build5b-optiplex-7080-20260528-ahci-run-log.txt` (9202 bytes, copied from Stick C `\BUILD-5b-staging\ahci-run-log.txt`).

Tail of extracted log ends with:
```
NEO_TEST/1 event=BEGIN name=phase3_ahci_neofs_mount_and_readwrite phase=dispatch
[ kernel ] sched: enqueued kthread 'neofs-writeback' (id=15)
NEO_TEST/1 event=PASS name=phase3_ahci_neofs_mount_and_readwrite fs=neofs source=device0 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30
```

## Closure note

AHCI lane is formally GREEN on bare metal. The full mount → read → write → fsync → readback cycle executed against the AHCI-driver-backed Samsung SATA. The NVMe-driver-backed Intel NVMe served as log sink and proved write-persistence through the extraction.

The companion **NVMe lane** (Phase 3's second required scenario) surfaced a SEPARATE new kernel bug — see [phase3-build5b-optiplex-7080-20260528-nvme-lane-rerun-PARTIAL.md](phase3-build5b-optiplex-7080-20260528-nvme-lane-rerun-PARTIAL.md) and [BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md](../BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md). That bug lives specifically in the AHCI driver's *log-sink write path* (a code path the AHCI lane never exercises, because the AHCI lane writes its log to NVMe). The NVMe-lane discovery does NOT regress this AHCI-lane result.
