# BLOCKER — Phase 3 BUILD-5b NVMe Lane — AHCI log-sink write path lands at wrong LBA and DMAs kernel `.rodata` to disk

**Date filed:** 2026-05-28
**Phase:** 3
**Build:** BUILD-5b
**Lane affected:** NVMe (and any future scenario that uses the AHCI driver as a log-sink target)
**Severity:** BLOCKING for Phase 3 BUILD-5b formal close; ALSO a security concern (kernel memory leak to disk)
**Kernel commit at time of discovery:** `72a0a41c8d6ef6d8bb6e7981dbc928c8cface40c` (with the prior Jarvis APIC fix applied)

## Summary

When the NeoPlutarch kernel uses the AHCI driver as a bare-metal log-sink target, log writes:

1. Land at the **wrong byte offset** on the destination disk (observed: `0x434A51` instead of the declared `0x10000000`)
2. **Over-DMA** — copy hundreds of bytes of adjacent kernel `.rodata` (Rust source paths, panic/assert format strings, crate paths, toolchain paths) instead of the intended log-header structure

The kernel reports success (`bare_metal_log_sink_selftest=ok cursor=246` and end-of-scenario `NEO_TEST/1 event=PASS`), but `neo-extract-log` against the destination disk at the default offset fails with `NEO_LOG_v1 magic not found at requested offset`. Investigation reveals the magic IS on disk, just at the wrong LBA, followed by kernel binary contents.

## Why this is separate from the APIC panic Jarvis already fixed

- The APIC panic (`kernel/src/arch/x86_64/apic.rs:125`) was an IRQ-handling race: storage probe enabled MSI/MSI-X before APIC base was initialized. Jarvis closed it in `72a0a41` by initializing the BSP APIC before `drivers::block::init()`. **That fix holds** — the AHCI lane re-run completed without the panic and extracted 9202 clean bytes (see [phase3-build5b-optiplex-7080-20260528-ahci-lane-rerun-PASS.md](evidence/phase3-build5b-optiplex-7080-20260528-ahci-lane-rerun-PASS.md)).
- This BLOCKER is a different defect in a different code path: the AHCI driver's log-sink **write submission** (DMA source/destination computation). It has nothing to do with interrupts.

## Forensic evidence

Full forensic chain is in [phase3-build5b-optiplex-7080-20260528-nvme-lane-rerun-PARTIAL.md](evidence/phase3-build5b-optiplex-7080-20260528-nvme-lane-rerun-PARTIAL.md). Key data:

**Kernel declared write destination** (serial line 435):
```
NEO_TEST/1 event=INFO name=phase3_nvme_neofs_mount_and_readwrite bare_metal_log_sink=installed target_driver=ahci offset=0x10000000 size=16777216
```

**Extractor at declared offset:** `Error: "NEO_LOG_v1 magic not found at requested offset"`

**Magic-string scan over `/dev/sda` (Samsung SATA = AHCI-driver target):**
```
4399074:NEOFSv1     (= 0x4321A2)
4409489:NEO_LOG_v1  (= 0x434A51)
```

**Same scan over the raw log-sink file `$STAGING/nvme/nvme-log-sink-disk.raw`:**
```
0:NEOPLUTARCH_PHASE
```
The raw contains ONLY the offset-0 sentinel; both unexpected on-disk magics are kernel-written during this run.

**Hexdump of `/dev/sda` at `0x434A51`:**
```
NEO_LOG_v1\0kernel/src/arch/x86_64/syscall.rs\0
#tlb_shootdown_slot_busy target_cpu=
[ kernel ] paging: GLOBAL fixup complete - 4K leaves:
CapacityError:
/workspace/.cargo-home/registry/src/index.crates.io-.../linked_list_allocator-0.10.6/src/hole.rs
/workspace/.cargo-home/registry/src/index.crates.io-.../bit_field-0.10.3/src/lib.rs
kernel/src/arch/x86_64/idt.rs
/home/neo/.rustup/toolchains/nightly-2026-04-16-x86_64-unknown-linux-gnu/lib/rustlib/
```

These are verbatim kernel `.rodata` strings — panic/assert source paths, crate registry paths, format strings.

## Scope analysis

Four bare-metal storage code paths exist; only one is broken.

| Code path | Status | Proof |
|---|---|---|
| AHCI driver — NeoFS scenario disk (read+write+fsync) | ✅ WORKS | AHCI lane PASS: `create_write_fsync_readback=ok bytes=30` on Samsung SATA |
| NVMe driver — NeoFS scenario disk (read+write+fsync) | ✅ WORKS | NVMe lane scenario PASS: `create_write_fsync_readback=ok bytes=30` on Intel NVMe |
| NVMe driver — log sink (write+read by neo-extract-log) | ✅ WORKS | AHCI lane extraction: 9202 bytes clean, no Degradation, magic at correct offset |
| **AHCI driver — log sink (write+read by neo-extract-log)** | ❌ **BROKEN** | NVMe lane: magic at wrong LBA, kernel `.rodata` leaked, extractor fails |

The bug lives at the intersection: **AHCI driver × log-sink write submission**. This code path was first exercised by this BUILD-5b NVMe lane run.

## Hypothesis (for Jarvis to confirm or refute)

The AHCI log-sink write submission appears to have two co-occurring defects:

1. **Destination LBA arithmetic** — the kernel passes byte offset `0x10000000` to the AHCI command-FIS builder, which expects sector-units or some other unit, producing a wrong sector LBA that maps to byte offset `0x434A51` after AHCI/SSD translation. (Numerical relationship between the two offsets is not a clean factor; the precise arithmetic bug needs to be traced in code.)
2. **DMA source pointer / length** — the write submission uses a source address adjacent to the `NEO_LOG_v1` magic constant in `.rodata`, with a length that walks 500+ bytes into surrounding `.rodata`. The kernel intended to write a small structured log header (magic + version + length + scenario name + …), not a `.rodata` dump.

Both bugs co-occur because this path has never been validated. The NVMe-driver log-sink path (used by the AHCI lane) does the same job correctly, suggesting the right reference exists; the AHCI-driver path likely diverged from it.

## Security implication

The AHCI log-sink path leaks arbitrary kernel `.rodata` (and potentially `.text`, `.bss`, stack contents — unknown without further investigation) to a disk region readable by any subsequent OS that mounts the device. This is a confidentiality boundary violation. Even after the LBA bug is fixed, the DMA length / source bounding must also be fixed; otherwise a "fix" that just relocates the write to `0x10000000` would still leak kernel memory at the new location.

## Acceptance criteria for the fix

A fix is acceptable only when ALL of the following hold:

1. **Code change** — AHCI log-sink write submission corrects both the destination LBA arithmetic AND the DMA source pointer/length to write ONLY the intended log header structure (no kernel memory beyond the header is copied).
2. **`cargo k` PASS** — kernel build and tests green.
3. **`neo-extract-log` tests PASS** — extractor unit tests green.
4. **QEMU AHCI PASS** — phase3_ahci_neofs_mount_and_readwrite scenario passes in QEMU with valid extraction at default offset.
5. **QEMU NVMe PASS** — phase3_nvme_neofs_mount_and_readwrite scenario passes in QEMU with valid extraction from the AHCI-backed Samsung-equivalent log sink at default offset. **This is the new scenario that must be exercised** — the bug we're filing was first surfaced because QEMU's NVMe lane was passing but bare metal wasn't; either QEMU was masking it, or QEMU NVMe lane didn't actually use AHCI-as-log-sink end-to-end. Jarvis should confirm.
6. **Bare-metal build-only PASS** for both AHCI and NVMe lanes.
7. **Rebuilt BUILD-5b-staging** with fresh SHA256 hashes for the 4 changed AHCI-side artifacts (or all 7 if any test fixtures changed).
8. **Confirmation in commit message** that this BLOCKER (`Plans/BLOCKER-2026-05-28-phase3-build5b-ahci-log-sink-dma-wrong-lba-and-rodata-leak.md`) is being closed, and naming whether the security `.rodata`-leak aspect was addressed by the same patch or deferred to a separate change.
9. **PL rerun instruction** specifying NVMe lane only first (AHCI lane has already passed; rerunning it is optional).

## Reproduction

On bare-metal OptiPlex 7080 (or equivalent with Intel NVMe + SATA SSD in AHCI mode) with kernel commit `72a0a41` or later:

1. Provision Samsung SATA with `nvme-log-sink-disk.raw`, Intel NVMe with `nvme-scenario-disk.raw` per EXECUTION-GUIDE.md NVMe lane.
2. Boot the NVMe-scenario ISO.
3. Observe framebuffer PASS (kernel-side scenario passes).
4. Power off, boot Ubuntu Live.
5. Run `sudo $STAGING/neo-extract-log --device /dev/sda --output /tmp/out.txt` — expect `Error: NEO_LOG_v1 magic not found at requested offset`.
6. Run `sudo dd if=/dev/sda bs=1M count=512 2>/dev/null | grep -aob NEO_LOG_v1` — expect a hit at a non-`0x10000000` offset.
7. Hexdump 512 bytes at the hit offset — expect to see verbatim kernel source-path strings.

## Dispatch

Dispatched to Jarvis via: [HANDOFF-2026-05-28-phase3-build5b-ahci-log-sink-dma-bug-to-jarvis.md](HANDOFF-2026-05-28-phase3-build5b-ahci-log-sink-dma-bug-to-jarvis.md)
