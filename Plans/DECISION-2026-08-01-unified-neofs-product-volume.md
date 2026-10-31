<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DECISION: the **unified NeoFS product volume** — the Arthur model volume IS the persistence volume

**Date:** 2026-08-01 · **PM (Claude)**, on Chad's rung-5 RETURN. **Ratified for 0.1.** Surfaced to the PL because it is a product property, not only an implementation detail.

## The decision, in Chad's words (authoritative)

> `boot-neofs` now selects by **NeoFS identity** instead of scan order, but it does **not** distinguish "model volume" from "persistence volume." On the OptiPlex, the SATA Arthur/model disk is the only NeoFS volume, so product boot mounts that volume **read-write** at `/neofs`. Persistence writes land there. **This is a unified NeoFS product volume design, not a separate persistence-volume design.**

## Why this is ratified rather than bounced

The AAG's Finding 2 objected to mounting the model volume **by accident** — `get_device(0)` was PCI-scan-order, bus-major, no identity check, so *which* disk got mounted was an artifact of enumeration. `find_neofs_block_device()` replaces that with a superblock probe plus a logged identity (`driver`, `name`, `pci_bdf`). The selection is now **deterministic, diagnosable, and cannot grab a non-NeoFS disk**.

What it does **not** do is make the model volume a different volume from the persistence volume — because on the target hardware **there is only one NeoFS volume**. Any selector that finds "the NeoFS volume" finds that one. Demanding otherwise would mean demanding a second disk, which is a hardware decision, not a code fix.

**Safety boundaries verified and holding** (traced in the AAG, not accepted on assertion): no superblock repair; no disk overwrite; a failed mount structurally cannot write (`select_superblock` errors before `NeoFsState::new`, so no writeback worker is ever spawned); no `mkdir`/`unlink` in this build; and **the model inode's content is neither mount-verified by full allocation nor rewritten by this path** (F1: `should_verify_child_content_at_mount(kind) = kind != NeoFsKind::Model`) — **but its content IS still verified at first range-read** via the streaming `verify_inode_content_hash_cached` (block-by-block blake3 through one 4 KiB buffer, memoised per inode), **except under `arthur-real-brain-boot`** (see OT-73). *Amended 2026-08-01 per Cerebro's AAG note (d): the unamended wording was accurate but could mislead a future reader into thinking model integrity is unverified. F1 removed the redundant-and-catastrophic check and kept the correct one — it is a strict improvement, not a deletion.*

## The risk, stated plainly — this is what the PL is being told

**The model blob and user data share one free map and one rotating superblock.** There is no isolation between them. A NeoFS allocator or writeback defect does not corrupt "some user file" — it can corrupt `model.neom`, and the model is the thing that makes Arthur answer at all. **The blast radius of any future NeoFS write bug now includes rung 3, which is already PROVEN.**

That risk is accepted for 0.1 on these grounds: durability is genuinely proven by a two-boot test with a fresh kernel and cache on boot 2; the write surface is small (no mkdir/unlink, no repair, no overwrite); and there is no second volume to move to. It is **not** accepted permanently.

## What would change the decision

If the product rule becomes **"the model volume must remain immutable / read-only,"** this branch does not implement it and would need a **volume role/label selector** — a NeoFS volume carrying a declared role, so `boot-neofs` can mount the *persistence* volume read-write and the *model* volume read-only, instead of inferring both from "it has a valid superblock." That is a real build, queued below the break gate, not smuggled into rung 5.

**Naming debt to clear when that lands:** `mount_boot_neofs_from_device0_readwrite()` and `mount_neofs_from_device0_readonly()` no longer use device 0. The names now actively mislead about the very behaviour this decision is about.

## Status
- **Ratified for 0.1.** Rung 5 merges on this basis.
- **Recorded as a follow-on:** volume role/label selector + the `device0` renames → post-break queue.
- **The ledger says what is true:** LogansOS 0.1 ships a *unified* NeoFS product volume. Model and user data are co-resident by design, not by accident.
