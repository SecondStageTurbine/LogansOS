<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-87 R3, the partition view, the guard's partition rule and a one-disk VM image → S3

**Why.** It's the third rung of the installer ladder (`docs/THINK-2026-10-27-ot87-installer.md` §8, the R3 row).
- **What it unblocks:** today NeoFS needs a whole disk to itself, so an installed system needs two disks (R1's
  `boot.raw` + `data.raw`). R3 lets one GPT disk carry the ESP and a NeoFS partition, and lets the write guard arm
  exactly that partition. R4 (the installer) depends on R2 (merged, #154) and on this.
- **What it touches:** the write guard (#85/#88), which keeps the Zenbook's internal Windows disk unwritten. This is a
  **safety-invariant change**: relax-and-re-prove, plus a Muse security review.

**Deliverable:** branch `claude/ot87-r3-partition-view`, a PR, and then the PM runs the 4-shard gate.

## Scope (THINK §4.2, §4.4, §6.3, §8)

1. **A shared pure GPT module.**
   - Move the guard's GPT parser (`write_guard_logic.rs`, `validated_gpt` and its CRC32) into a pure module that the
     guard, the partition view and xtask all use.
   - Moving it changes no behaviour. Every existing guard KAT passes unchanged.
2. **`PartitionDevice { inner, first_lba, lba_count }`.**
   - It offsets and bounds every read and write. A request that crosses `lba_count` is refused, never truncated.
   - It's built only from a validated GPT entry.
   - NeoFS mounts it unchanged.
3. **`boot-neofs` probes LogansOS-typed partitions before whole devices.**
   - It skips a partition with the INCOMPLETE attribute (bit 48, §4.7).
   - The Try read-only mount (§6.3) is R5, not here. Don't change Try behaviour in R3.
4. **The guard's partition claim (relax-and-re-prove).** It arms exactly `[first_lba, last_lba]` of an entry only
   when all four hold:
   - the GPT validates;
   - the type GUID is the LogansOS NeoFS GUID;
   - the first 4 KiB is a valid NEOFSv1 superblock;
   - `total_blocks × 4096` is no larger than the partition.
   Everything else behaves as today.
5. **`xtask` builds a one-disk GPT image** (ESP + NeoFS partition) for the VM, from a pure
   `gpt_layout(S, L, disk_guid, esp_guid, neofs_guid)` per §4.4.
   - Fixed size (THINK §9, decision 2: fixed images for R1–R3).
   - Add a `vm-image --one-disk` (or equivalent) and a VM scenario that boots it under the pinned OVMF.
   - The scenario mounts NeoFS from partition 2 of the boot disk, and persists across a reboot like
     `vm_installed_two_boot`.
6. **The ledger.**
   - Add **I-17** (the write guard) to `docs/complexity-governance.md`.
   - In the same change, run a consolidation pass so the ledger stays on one page (THINK §9, decision 6).

## Required proofs

- **The hand-laid GPT byte oracle (§4.4, I-12).**
  - Both 4 GiB fixtures, `S=512` and `S=4096`, list every non-zero byte of LBA 0, both headers and entries 1–2.
  - CRCs come from Python's `zlib.crc32` when the fixture is laid out, independent of the writer.
  - Check 1: xtask's `gpt_layout` output equals the fixture.
  - Check 2: the guard's `validated_gpt` accepts it.
  - Check 3: a second, independent reader agrees. That's the `gpt` crate (MIT) in a host test (§11.2), or
    `sgdisk -v` when the fixture is made.
  - R4 reuses these fixtures.
- **Guard KATs.**
  - Every existing one passes, `windows_gpt_disk_never_arms` included.
  - The oracle disk arms partition 2 only.
  - A disk with only the type GUID right, or only the superblock right, arms nothing.
  - A superblock claiming past its partition arms nothing.
  - A partition overlapping the GPT's own sectors or the backup array arms nothing.
- **The partition view.** A crossing request is refused; offsets are exact at the first and last LBA.
- **Kill-switch `neofs-partition`.** With it off, the guard and mount are bit-for-bit today's. Give `.text` SHA-256s
  for the desktop, product and image-A kernels, `main` vs your branch.

## Rules that apply

- **The pitfall bank** (`docs/concurrency-pitfalls.md`). State per class whether the partition view or the guard
  change introduces, guards or leaves N/A any lock or IF discipline in the block path (CP-56, CP-37 at minimum).
  Include the table in the PR.
- **Prior art.** THINK §11 is the survey. Restate the rows you used in the PR body, with licenses and last commit
  dates. Vendor nothing in R3.
- **The Zenbook.** Nothing in R3 runs on the Zenbook, and the guard's default for a foreign or Windows table is
  unchanged. State that in the PR with the KAT that proves it.
- **The close-gate:** `scripts/worker-close-gate.sh`, rebased on `origin/main`.
- **Reviews after the gate:** Muse (Gwen) for security, reading the files at the head SHA; then a Codex-family review
  (GPT-6-Luna high) of the GPT oracle and the partition view.

## Setup

```
WORKTREE: cloud session S3 (your own clone)
BRANCH:   claude/ot87-r3-partition-view
          git fetch origin && git checkout -B claude/ot87-r3-partition-view origin/main

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r3-partition-view-s3.md   # this file
git show origin/main:docs/THINK-2026-10-27-ot87-installer.md                   # §2, §2a, §4.2, §4.4, §8, §11
```

RAG isn't available in the cloud session. Cite code by `file:line`. Push at every checkpoint. Send the PM the PR
number when it's up. Before ending, `git status --porcelain` and `git log @{u}..` are both empty.
