<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: fix the Muse FAILs on OT-87 R3 (#160) and R4.1 (#163), plus three Codex FAILs (PR C) → S3

**Why.** (PR C, added later the same day, is below; it fixes three Codex-review FAILs.) Muse reviewed R3 and R4.1 after they merged. Each review FAILs on one item:
- R3 FAILs on GPT entries that overlap: `Plans/RETURN-2026-10-27-ot87-r3-guard-review-muse.md`, item 5.
- R4.1 FAILs because NVMe never reports a namespace change or a controller reset:
  `Plans/RETURN-2026-10-27-ot87-r4.1-claim-review-muse.md`, item 4.

The PM re-checked both against `origin/main` at `8e80abde`, and both hold:
- `gpt_logic::validate` (`kernel/src/drivers/block/gpt_logic.rs:125ff`) checks each entry against the usable range
  only. Nothing compares two entries.
- `nvme.rs` has no Async Event Request and no AEN handling. The interrupt handler only counts interrupts.

**Exposure, and why this is fix-forward rather than revert.** Neither `neofs-partition` nor `installer` is in
`PRODUCT_IMAGE_KERNEL_FEATURES` (`tools/xtask/src/main.rs`) at `8e80abde`, so no product image carries either defect.
No installer writes exist on `main` yet. Reverting #160 and #163 would also unwind R4.2 (#165) and block R4.3 (#170),
without making any image safer. The escaped-defect contract's fix-forward window applies:
- **PR A (the GPT overlap)** is small. It should be pushed within the window, by **17:40 UTC (1:40 pm EDT)** today.
  If it isn't, the PM reverts #160 and the work re-lands with the fix.
- **PR B (NVMe reporting)** is real driver work, so it isn't held to the window. It must merge **before R6** (the first
  bare-metal install) and before `installer` goes into any image that boots on hardware. It does not block #170,
  which stays VM-only behind `installer`.

The PM's call on R4.1 is **fix, not sign the gap into the stated limit.** THINK §2a item 5 makes driver reporting a
prerequisite of the claim, so the claim is built on it, not around it.

## PR A — reject overlapping GPT entries (`claude/ot87-r3-gpt-overlap`)

1. In `gpt_logic::validate`, after the entries are collected, reject the copy (`None`) when any two non-empty
   entries overlap (`a.first_lba <= b.last_lba && b.first_lba <= a.last_lba`). Rejecting the whole copy is the
   simpler rule and matches how every other malformed field is treated. If the backup copy validates on its own, it
   is used, exactly as today.

   This is a parser change, and it reaches the feature-off `judge()` path too. `judge()` reads the table through
   `read_table` (`write_guard_logic.rs:255ff`). When a GPT header is present but neither copy validates, that
   returns `Table::Opaque("opaque-gpt-invalid")`, and `judge()` clears every claim. So an overlapping table should
   make the whole device arm nothing, on both paths. That is the fail-closed result. Show it with a KAT on
   `judge()` as well as `judge_with_partitions()`.
2. KATs in `tools/block-guard-logic-tests`:
   - Muse's layout exactly: ESP and LogansOS entries both `[2048, 1050623]`, both GPT copies re-sealed, a sealed
     NEOFSv1 superblock at 2048 that fits. It arms nothing, and a write to LBA 2048 is refused.
   - Overlap by one sector at either end, which arms nothing.
   - Entries that touch but don't overlap (`a.last + 1 == b.first`), which still arm normally.
   - A primary copy with overlap plus a clean backup, where the backup's decision is taken.
   - The differential test against `legacy`. The corpus has no overlapping table, so it should stay green
     unchanged. Add the overlap layout to it as an **expected divergence**, named in the test, because the legacy
     parser accepts that table and the new one refuses it (`opaque-gpt-invalid`, nothing armed).
3. Ride-along: fix the stale "four conditions" comment in `kernel/src/drivers/block/mod.rs` (the code checks five).
4. Kill-switch: this change is in the shared parser, so it isn't behind `neofs-partition`, and `.text` will
   differ. Give both hashes, and say in the PR that the feature-off guard now refuses one more malformed table. That
   change only removes write permission; it never adds any.
5. Pitfall bank: N/A (no atomics, no locks). Say so in one line.

## PR A, second finding (added 14:25 UTC) — the partition may not cover GPT metadata (Janus #102 item 7)

`Plans/RETURN-2026-10-27-ot87-r3-review-janus.md` item 7. **Put it in #172, the same PR, under the same 18:00 UTC
fix-forward** (it also fails #160).

- **The finding.** `gpt_logic::validate` accepts `last_usable == last_lba` and never checks `alternate_lba`. It checks
  only the primary's own entry array against the usable range. `read()` trusts a valid primary without comparing it
  to the backup.
- **Concrete case.** A CRC-valid primary with `last_usable = L` (the last LBA) and a LogansOS entry spanning
  `2048..L`, with a sealed superblock. That arms a claim covering the backup array and the backup header, so a write
  to the partition's last sector overwrites the backup GPT. This breaks I-17.
- **Requirement.** A partition claim can never cover GPT metadata. Make both rules hold:
  - `validate` refuses a primary whose `alternate_lba != last_lba`, and a backup whose `alternate_lba != 1`. It refuses
    a usable range that reaches the other copy's array or header. The primary's `last_usable` must be below the
    backup array's first LBA, which is `last_lba - ceil(array/ss)`.
  - As defence in depth, `partition_claims` drops any claim that intersects either header or either entry array.
- **KATs.** Janus's exact layout arms nothing, and a write to `L` is refused. Add `alternate_lba` wrong, and
  `last_usable` one past the backup array start. Re-run the differential test: any divergence must be a named
  expected one.

## PR B — NVMe namespace-change and reset reporting (`claude/ot87-r4.1-nvme-events`)

The requirement is from THINK §2a item 5. **A namespace change or a controller reset must be visible to the claim's
pre-write check before the next write is submitted.** At the latest it must be visible at the next step boundary, and
only for a case the PR names and justifies.

1. **Namespace identity.** Add the namespace's NGUID and EUI64 (Identify Namespace) to the NVMe `DriverIdentity`
   and to its live re-read. A twin namespace with the same serial, model and capacity then fails at claim time and at
   every step boundary. All-zero NGUID and EUI64 together mean "not reported". Decide whether that makes a destructive
   claim refuse (`IdentityUnavailable`) or only fall back to today's fields. State which you chose and why. Fail-closed
   is the default.
2. **Namespace-change notices.** Arm an Async Event Request with Namespace Attribute Notices enabled (Set Features,
   Asynchronous Event Configuration). Any namespace-attribute completion, and any AER completion the driver doesn't
   recognise, bumps the media generation. Read the Changed Namespace List log if you need to know which namespace
   changed. Bumping on any notice is acceptable, and it is fail-closed.

   The claim checks synchronously, so how a completion is noticed matters. A lock-free peek at the admin completion
   queue's phase bit in `media_state()` is one way, an interrupt-side bump is another. Pick one and justify it against
   the CP bank (it's a new cross-context path).

   If QEMU's NVMe emulation can't raise the notice in the gate, give a host KAT with a model controller instead, and
   name the gap.
3. **Reset.** Show from the NVMe spec (raw text, cite the section) which registers a controller reset, a function-level
   reset and an NVM subsystem reset change, and which of those `media_state()` reads. Then either:
   - show that a reset our driver didn't make always leaves something `media_state()` sees as dead or changed (for
     example CC.EN or CSTS.RDY low, or a queue doorbell or register state our driver set and nothing else restores),
     including a reset that completes between two polls; or
   - add the check that makes it so.

   Muse's case to answer: "a reset that completes between two polls leaves no trace in the generation." Close it, or
   prove it can't happen on bare metal and name the VM case as the stated limit.
4. Ride-alongs from the same review. Each is a small fix or a one-line answer in the PR:
   - **Item 1:** the enumeration token can claim again after a `ClaimedWriter` drops. If #170 already consumes it,
     say where. If not, consume it here.
   - **Item 9:** virtio `config_generation` is a u8 and can wrap undetected. Say whether any path compares across
     more than one poll; if so, widen it.
   - **Item 9:** a transient NVMe RDY-down makes the disk unclaimable for the rest of the boot. That is fail-closed,
     so keep it and state it as the limit.
5. KATs: the claim refuses the next write after (a) a namespace notice, (b) a twin namespace (NGUID differs) at
   claim and at a boundary, and (c) each reset kind your argument covers. Muse's FAIL sequence exactly (detach ns 1,
   attach a same-serial, same-capacity twin, controller ready throughout) must now stop the claim before the first
   write lands on the twin, or at the boundary if you justified that case.
6. Driver trap-mining: the NVMe spec for AER, AEC and log 04h, and Linux/BSD `nvme` AEN handling **for principle
   only** (no GPL code). Put the trap table in the PR body.
7. Kill-switch: `installer` off ⇒ `.text` identical to `main` with `NEO_BUILD_ID` pinned, if the AER is armed only
   under `installer`. If it is armed unconditionally, say why and give the hashes.

## PR C — three small fixes from the Codex reviews (`claude/ot87-review-fixes-c`). Added 2026-10-27, after PR A

The PM re-checked all three at `91320d01`. None of them is in a product image. Order: PR A first (it has the
deadline), then this one, then PR B.

- **#165 R4.2 (`Plans/RETURN-2026-10-27-ot87-r4.2-review-chad.md` item 1).** `slot_map` checks
  `dirent_offset + 32 <= sector_size` with an unchecked `u32` add (`kernel/src/install_bundle_logic.rs:333`). An
  offset of `0xffffffe0` wraps in release builds and is accepted.
  - Fix: use `checked_add`.
  - Add the overflow value to the slot-map KAT, expecting `BadSlotEntry`.
  - Grep the rest of `install_bundle_logic.rs` for the same pattern.
- **#167 NFS-W1 (`Plans/RETURN-2026-10-27-nfs-w1-review-chad.md` item 2).** `create_file_from_bytes`
  (`kernel/src/fs/neofs/inode.rs:279–318`) reserves `n + k` blocks, then returns through `?` on any write or flush
  error without giving them back. The next ordinary commit then publishes the dirty free map, and the blocks are lost
  for good.
  - Fix: on every error path after the reservation, release the run, or mark the mount failed so nothing more commits.
    Say which, and make sure the cache writeback that was paused is resumed on those paths too.
  - Add the KAT as Chad describes it: inject an error on the first data write, then `create_regular` succeeds, and
    after remount the free map has those blocks free.
- **#154 R2 (`Plans/RETURN-2026-10-27-ot87-r2-model-from-neofs-review-chad.md` item 4).** When the vocab falls back
  to NeoFS, it sets `model_source = Neofs` (`userland/neo-arthur/src/main.rs:2817` / `:2953`), even when the model
  itself loads from the boot module. Arthur then prints the wrong source.
  - Fix: keep a separate source for the model and for the vocab, or set `model_source` only from the model load.
  - Add a host KAT for the mixed case: boot-module model plus NeoFS vocab prints `source=boot-module`.

## All PRs

- One PR each (A, B, C), rebased onto `origin/main`. `scripts/worker-close-gate.sh` on each. Push at every checkpoint. Read
  head SHAs from `git ls-remote` and send them to the PM.
- The PM gates them on the integration branch, batched with #170 where they don't conflict.
- Reviews after merge: Muse re-checks its own item 5 and item 4 (narrow re-check).

```
WORKTREE: cloud session S3 (your own clone)
BRANCH:   claude/ot87-r3-gpt-overlap, then claude/ot87-r4.1-nvme-events
          git fetch origin && git checkout -B claude/ot87-r3-gpt-overlap origin/main

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-review-fails-fix-s3.md
git show origin/main:Plans/RETURN-2026-10-27-ot87-r3-guard-review-muse.md
git show origin/main:Plans/RETURN-2026-10-27-ot87-r4.1-claim-review-muse.md
git show origin/main:docs/THINK-2026-10-27-ot87-installer.md
```

RAG isn't available in the cloud session. Cite code by `file:line`. Before ending, `git status --porcelain` and
`git log @{u}..` are both empty.
