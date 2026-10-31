# Phase 3 BUILD-5b — NVMe Lane Bare-Metal Re-run — PARTIAL (scenario PASS, on-disk extraction FAIL)

**Date filed:** 2026-05-28 (rerun executed 2026-05-27 23:45 EDT → 2026-05-28 00:30 EDT)
**Hardware:** Dell OptiPlex 7080 Micro (same as AHCI lane)
**Scenario disk:** Intel SSDPEKKR256G8 NVMe (`/dev/nvme0n1`)
**Log sink:** Samsung SSD 860 EVO 250 GB (`/dev/sda`)
**Kernel commit under test:** `72a0a41c8d6ef6d8bb6e7981dbc928c8cface40c` (same fixed kernel as AHCI lane)

## Result — 3 of 4 ACCEPTANCE GATES GREEN; 1 GATE FAILED ON A NEW KERNEL BUG

| Gate | Status | Evidence |
|------|--------|----------|
| 1. Framebuffer PASS (green) | ✅ | PL observed PASS in green (verbal confirmation; no photograph captured this lane) |
| 2. Extracted PASS marker present | ❌ | Extraction failed — magic not at default offset |
| 3. Captured bytes > 0 | ❌ | Extractor errored before writing output |
| 4. No Degradation detected block | n/a | Extractor errored before stdout could include or omit a Degradation block |
| (bonus) Kernel-side scenario PASS via serial | ✅ | `NEO_TEST/1 event=PASS name=phase3_nvme_neofs_mount_and_readwrite fs=neofs source=device1 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30` |

## Provisioning chain-of-custody (Ubuntu Live, prior to scenario boot)

- Stick B (sdb, 238.5G Toshiba external SSD) reflashed with `nvme-scenario.iso` via `sudo dd if=$STAGING/nvme/nvme-scenario.iso of=/dev/sdb bs=4M conv=fsync` — 25 MB at 97 MB/s
- `sudo dd if=$STAGING/nvme/nvme-scenario-disk.raw of=/dev/nvme0n1 bs=4M conv=fsync` — 128 MiB at 112 MB/s (scenario disk now on Intel NVMe)
- `sudo dd if=$STAGING/nvme/nvme-log-sink-disk.raw of=/dev/sda bs=4M conv=fsync` — 288 MiB at 108 MB/s (log sink now on Samsung SATA — the inversion of the AHCI lane)
- `sudo sync`
- Magic-byte check: `/dev/nvme0n1` → `NEOFSv1` at offset 0; `/dev/sda` → `NEOPLUTARCH_PHAS...` (pre-init sentinel)
- Byte-for-byte cmp against both raw files: clean (EOF on raw with zero byte differences)

## Scenario kernel boot evidence (serial)

**Source PuTTY log:** `phase2-bare-metal-optiplex-7080-20260527-191028-12-irqsafe_mutex_bench.serial.log` (same canonical file as the AHCI-lane evidence; the NVMe-lane boot is appended further into the same file).

**Key lines:**

| Line | Content | Significance |
|------|---------|--------------|
| 435 | `NEO_TEST/1 event=INFO name=phase3_nvme_neofs_mount_and_readwrite bare_metal_log_sink=installed target_driver=ahci offset=0x10000000 size=16777216` | Kernel declares it will write log to AHCI driver at offset `0x10000000` |
| 436 | `NEO_TEST/1 event=INFO name=phase3_nvme_neofs_mount_and_readwrite bare_metal_log_sink_selftest=begin` | Sink selftest starts |
| 437 | `NEO_TEST/1 event=INFO name=phase3_nvme_neofs_mount_and_readwrite bare_metal_log_sink_selftest=ok cursor=246` | Kernel reports 246-byte selftest write+readback succeeded |
| 572 | `NEO_TEST/1 event=BEGIN name=phase3_nvme_neofs_mount_and_readwrite phase=dispatch` | Scenario starts |
| 574 | `NEO_TEST/1 event=PASS name=phase3_nvme_neofs_mount_and_readwrite fs=neofs source=device1 mount=rw initial_read=ok create_write_fsync_readback=ok bytes=30` | **NVMe scenario PASS on bare metal — NeoFS read/write/fsync/readback cycle on Intel NVMe is proven** |

APIC-init ordering is correct in this boot as well (`apic: cpu=0 ... enabled xAPIC (BSP)` precedes `block: init`).

## Extraction evidence — FAILED at default offset

```
$ sudo $STAGING/neo-extract-log --device /dev/sda --output /tmp/nvme-run-log.txt
Error: "NEO_LOG_v1 magic not found at requested offset"
```

The extractor's default offset (`0x10000000`) does not contain the NEO_LOG_v1 magic on the Samsung SATA after the run.

## Forensic investigation (Ubuntu Live, post-extraction-failure)

**Hexdump at offset 0** (expected: pre-init sentinel `NEOPLUTARCH_PHAS...` — actual: stale boot-sector debris):
```
00000000: 4552 0800 eb02 ffff 0000 0000 0000 0000  ER..............
```

**Hexdump at offset 0x10000000** (expected: NEO_LOG_v1 header — actual: high-entropy data, neither zeros nor magic):
```
00000000: 8ee1 4558 ff00 85bd f10e d977 5c7c 38bb  ..EX.......w\|8.
00000010: 5880 ff00 9f5b 84c7 e254 d75f 79f1 2e2f  X....[...T._y../
00000020: 0dfc 44bf f0ff 008a 0c76 5a7c a91c da65  ..D......vZ|...e
... (continues, no recognizable structure)
```

**Magic-string scan over first 512 MB of /dev/sda:**
```
$ sudo dd if=/dev/sda bs=1M count=512 2>/dev/null | grep -aob -E "NEO_LOG_v1|NEOPLUTARCH_PHASE|NEOFSv1"
4399074:NEOFSv1
4409489:NEO_LOG_v1
```

The `NEO_LOG_v1` magic IS present — but at byte offset **`0x434A51`** (4,409,489 bytes ≈ 4.2 MB), NOT at the declared `0x10000000` (256 MB). A `NEOFSv1` magic also appears at offset `0x4321A2` ≈ 4.3 MB.

**Cross-check against the raw log-sink file** (to determine if these magics are pre-existing in the staged raw or kernel-written):
```
$ grep -aob -E "NEO_LOG_v1|NEOPLUTARCH_PHASE|NEOFSv1" $STAGING/nvme/nvme-log-sink-disk.raw
0:NEOPLUTARCH_PHASE
```

The raw file contains ONLY `NEOPLUTARCH_PHASE` at offset 0. No `NEO_LOG_v1` and no `NEOFSv1` anywhere in the raw. **Therefore both unexpected magics on `/dev/sda` were written by the kernel during this run.**

**Hexdump around the misplaced magic** (`/dev/sda` at offset `0x434A51`, 512 bytes):
```
00000000: 4e45 4f5f 4c4f 475f 7631 00 6b 65 72 6e 65 6c 2f 73 72 63  NEO_LOG_v1.kernel/src
00000020: 2f 61 72 63 68 2f 78 38 36 5f 36 34 2f 73 79 73 63 61 6c 6c  /arch/x86_64/syscall
00000040: 2e 72 73 00 23 74 6c 62 5f 73 68 6f 6f 74 64 6f 77 6e 5f    .rs.#tlb_shootdown_
00000060: 73 6c 6f 74 5f 62 75 73 79 ... target_cpu= ...
... [ kernel ] paging: GLOBAL fixup complete - 4K leaves: ...
... CapacityError: ...
... /workspace/.cargo-home/registry/src/index.crates.io-.../linked_list_allocator-0.10.6/src/hole.rs
... /workspace/.cargo-home/registry/src/index.crates.io-.../bit_field-0.10.3/src/lib.rs
... kernel/src/arch/x86_64/idt.rs
... /home/neo/.rustup/toolchains/nightly-2026-04-16-x86_64-unknown-linux-gnu/lib/rustlib/
```

The bytes following the `NEO_LOG_v1` magic are **verbatim strings from the kernel binary's `.rodata` section** — Rust source-file paths embedded by `panic!()` / `assert!()` macros, Rust crate registry paths, the kernel's `paging` log format string, the build environment's rustup toolchain path.

This is kernel memory, written to disk. The kernel's log-sink write submission issued a DMA from the WRONG source pointer (or with the WRONG length), sending a 500+ byte chunk of kernel `.rodata` to the disk, AND to the WRONG destination LBA (`0x434A51` instead of the declared `0x10000000`). The 10-byte `NEO_LOG_v1` magic constant happens to live adjacent to those strings in `.rodata`, so the DMA "succeeded" in copying the magic and a lot of surrounding data.

**Override-offset extraction attempted:** the extractor was re-invoked with `--offset 0x434A51`. It rejected the offset — what follows the magic is `.rodata` text, not a valid `NEO_LOG_v1` header structure, so header validation fails. The data on disk is corrupt-by-construction; no extraction is possible.

## Diagnosis

A NEW kernel bug, distinct from the APIC panic that Jarvis fixed in `72a0a41`. The AHCI driver's **log-sink write path** has:

1. **Destination-LBA arithmetic bug** — writes land at `0x434A51` when the declared destination is `0x10000000`
2. **DMA source pointer + length bug** — the write copies ~500+ bytes of arbitrary kernel `.rodata` instead of the 10-byte magic + intended log header

**Scope:** the bug affects the AHCI driver's log-sink write submission specifically. Two adjacent code paths are confirmed working:

- AHCI-driver-backed NeoFS scenario writes (proven by the AHCI lane: `create_write_fsync_readback=ok` on Samsung SATA)
- NVMe-driver-backed log-sink writes (proven by the AHCI lane: 9202-byte clean extraction from Intel NVMe)

The bug lives only in the intersection: AHCI driver + log-sink write submission. This path was first exercised in this NVMe lane run.

**Security note:** the leak of kernel `.rodata` to user-accessible disk regions is a confidentiality concern beyond just the correctness bug. The fix must bound the DMA source length, not just relocate the destination.

## Follow-up artifacts

- New BLOCKER: [BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md](../BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md)
- Jarvis HANDOFF: [HANDOFF-2026-05-28-phase3-build5b-ahci-log-sink-dma-bug-to-jarvis.md](../HANDOFF-2026-05-28-phase3-build5b-ahci-log-sink-dma-bug-to-jarvis.md)

## Forensic preservation

The Samsung SATA's NAND state at conclusion of this run is itself evidence — the misplaced magic and leaked `.rodata` strings live at `/dev/sda` offsets `0x4321A2` and `0x434A51`. The OptiPlex is powered off; the disk is preserved as-is. Re-running any scenario against the Samsung will overwrite this evidence. Re-inspection of the raw bytes requires booting Ubuntu Live and `dd`-ing from /dev/sda before any further provisioning.
