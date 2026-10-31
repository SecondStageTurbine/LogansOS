<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #101): security review of OT-87 R3, the write guard's partition rule and parser move (#160) → Muse (Gwen)

**MODEL:** Muse (Gwen). **Why:** this changes the disk write guard, the code that keeps LogansOS from writing a disk it
must not touch (on the Zenbook, the internal Windows NVMe). Security reviews of Claude-authored write-guard code go to
Muse (CLAUDE.md), as for PR #85, #88 and #120.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** a RETURN. Muse can't push, so the PL pastes or uploads it and the PM saves it as
`Plans/RETURN-2026-10-27-ot87-r3-guard-review-muse.md`.

**Reviewed SHA:** `7c771c0b` on `main`, #160's squash (PR head `2c342585` plus a merge of main). Gate: batch 6, run
`40457391685`, all jobs green. Read the files at that SHA from the repo, not a summary:
- `kernel/src/drivers/block/gpt_logic.rs` (new: the one GPT parser);
- `kernel/src/drivers/block/write_guard_logic.rs` (now derives its extents from `gpt_logic`);
- `kernel/src/drivers/block/partition.rs` (new: `PartitionDevice`);
- `kernel/src/drivers/block/mod.rs` and `kernel/src/fs/mod.rs` (the `boot-neofs` partition probe);
- `kernel/Cargo.toml` (`neofs-partition`);
- `tools/block-guard-logic-tests/src/{legacy_write_guard_logic.rs,lib.rs,partition_guard_tests.rs,gpt_tests.rs,gpt_oracle.rs}`,
  plus `fixtures/gpt_oracle.py`;
- `docs/THINK-2026-10-27-ot87-installer.md` §2 and §4.2 (the design), and `docs/complexity-governance.md` I-17.

**What changed, in two parts. Both need review.**
1. **Unconditional.** The guard's GPT parser moved into `gpt_logic.rs`, and the guard now derives its extents from the
   validated entries. This code path changed for **every** build, including the product image, even with
   `neofs-partition` off. A differential KAT keeps main's old guard (`legacy_write_guard_logic.rs`, frozen from
   `e0a59501`) and asserts identical decisions over:
   - the Windows fixture;
   - a protective-MBR-only disk;
   - backup/primary disagreement;
   - 4K sectors, empty disks and whole-disk NeoFS;
   - more than 2,000 one-byte mutants.
2. **Behind `neofs-partition`.** A new partition claim arms exactly `[first_lba, last_lba]` of one GPT entry when all
   five hold:
   - the GPT validates;
   - the type GUID is LogansOS NeoFS;
   - the first 4 KiB is a valid NEOFSv1 superblock;
   - `total_blocks × 4096` fits the partition;
   - the INCOMPLETE attribute (bit 48) is clear.
   `PartitionDevice` offsets and bounds every request.

## Checklist

Answer each item PASS or FAIL, with the `file:line` that enforces it. For a FAIL, give a concrete disk layout or call
sequence.

1. **The parser move changes no decision.** For any disk, the guard's armed extents with `neofs-partition` off equal
   main's. Check that the differential KAT really compares against an unmodified copy of main's guard, and that its
   corpus includes the Windows disk behind `windows_gpt_disk_never_arms`.
2. **A Windows or foreign GPT never arms.** With the feature on or off, a GPT whose entries are not the LogansOS NeoFS
   type GUID arms nothing. That includes:
   - a Windows disk with a NEOFSv1-looking block planted at a partition start;
   - a type GUID that's right but byte-swapped.
3. **All five conditions are required.** Each one alone, and each set of four, arms nothing. Check
   `partition_guard_tests.rs` and `incomplete_partition_arms_nothing`.
4. **The armed range is exact.**
   - Writes to `first_lba - 1`, `last_lba + 1`, the GPT headers, both entry arrays, the protective MBR and the ESP are
     refused.
   - `PartitionDevice` refuses a request crossing its end, never truncates it, and can't overflow on
     `first_lba + lba`.
5. **Parser robustness.** For malformed input, `gpt_logic` fails closed and arms nothing:
   - an entry count or entry size that overflows;
   - an entry array past the disk end;
   - overlapping entries;
   - `first_lba > last_lba`;
   - a header CRC that's valid but with a bad entry-array CRC;
   - a backup header pointing at the primary.
   No panic or out-of-bounds read on any input.
6. **The judgement still reads only.** The partition claim and the `boot-neofs` probe issue only reads before the
   arming decision. The #120 judgement-wait discipline (CP-82) is unchanged.
7. **The test-only items stay out of the product.** The frozen legacy guard and the oracle live only in host test
   crates. Nothing test-only is in `PRODUCT_IMAGE_KERNEL_FEATURES`.
8. **Coverage.** Name what the gate doesn't cover. For example: real NVMe/AHCI with 4Kn sectors, a disk hot-removed
   during the judgement, and a GPT written by Windows Disk Management rather than our fixtures.

## Close condition

Give the RETURN to the PL with PASS/FAIL per item, `file:line`, and an overall verdict (PASS or FAIL). The PM saves it
under `Plans/`.
