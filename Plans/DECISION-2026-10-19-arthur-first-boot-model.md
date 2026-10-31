<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DECISION: how Arthur gets a real model on a fresh 0.1 machine (PL-ratified 2026-10-19)

**Source:** `docs/THINK-2026-10-19-arthur-first-boot-model.md` (#67, merged `cdc2f73b`). The PL chose the
recommended option on every question on 2026-10-19.
**Cross-family check:** `Plans/DISPATCH-2026-10-19-arthur-first-boot-model-codex-crosscheck.md` (Chad). If it raises a
surviving finding against a decision below, the PM brings it back to the PL before B1 merges.

## Decisions

1. **How the model gets onto the machine: the bootloader loads it as a boot module (Q1 option a).**
   - The 0.5B `.neom` and `.vocab` ship on the USB image as Limine modules, with their licence files beside them.
   - The kernel builds the model object from the module bytes, then returns the module's frames to the allocator.
   - **This amends ADR-063:10-14** (which said weights never ship as Limine modules). The amendment is part of B1.
   - Why: our kernel has no USB mass-storage driver (it has AHCI, NVMe and virtio only), so a NeoFS partition on the
     stick cannot be read after boot. Downloading at first boot needs a client and fails offline.
   - USB mass storage (option b) remains the 1.0 path, through OT-65.
2. **0.1 ships the 0.5B (Q2 option a).**
   - It is Qwen2.5-0.5B-Instruct Q4_K_M, the only brain proven on silicon (the PL lock of 2026-07-18).
   - The 1.5B joins the same image after its bare-metal run (OT-28).
   - The tier rule ("the OS auto-selects the best brain the machine can run", PL 2026-07-08) is B2.
   - ADR-063's "1.5B default" is superseded for 0.1.
3. **The 3B does not ship in 0.1 (Q3 option a).** It carries the Qwen License (attribution plus a use policy), not
   Apache-2.0. It also needs a 2.75 GiB budget and would make the image about 3.7 GB.
4. **0.1 has no separate read-only model volume (Q6 option a), on one condition:** the model's digest is checked on
   every load, failing closed with the "damaged" message (the OT-73 fix).
5. **On a USB-only machine, saving your work is honestly off in 0.1 (Q5 option a).** Arthur answers, and persistence
   arrives with the installer or USB storage (OT-65).

## Consequences

- **B1 is dispatched:** `Plans/DISPATCH-2026-10-19-b1-arthur-boot-module-model.md`.
- **The licence article (bible.md:387) and `NOTICE`** gain a model-weights entry in B1.
- **OT-65:** the installer inherits the model-source registry. USB mass storage is its first dependency.

## Amendment 1 (PL, 2026-10-19 evening): Chad's cross-check changes D1. Do it properly in B1.

Chad's cross-family RETURN (`codex/arthur-first-boot-model-crosscheck-chad` @ `eb322323`) upheld the boot-module
delivery path (the kernel has no USB mass-storage driver). It refuted the reclaim shape as written:

- **C1.** The module adds about 470.6 MiB to the load-time peak, on top of the eagerly mapped 768 MiB heap.
- **C2.** `EXECUTABLE_AND_MODULES` frames are not a legal `BuddyAllocator::free()` input: the allocator is single-zone,
  and those ranges may be unaligned or overlapping. `find_module_by_path_suffix` can also re-borrow a module after a
  reclaim.
- **C3.** The tier rule omits resident tensor memory and uses summed `USABLE` memory, not managed allocatable pages.
- **C4.** The failure bank is missing the eight-module boot limit (F7), F2 cannot time pre-kernel loading, and F6
  predicts the wrong NeoFS branch on the OptiPlex.

The PL was offered "ship the simple form now (module stays resident, published minimum-RAM floor), optimize in B2" or
"do the memory work in B1". **The PL chose to do it properly in B1.** So:

1. **D1 stands with B1's scope widened.** B1 must include:
   - **(a)** a lazily backed or right-sized heap for the module source, so the load-time peak no longer stacks the
     768 MiB eager heap, the module and the tensor copy;
   - **(b)** a checked admit-external-region (second-zone) allocator operation with its own accounting, used for module
     reclaim. Never loop the existing `free()`. Reclaim is driven from the Module feature's exact file range,
     intersected with the memmap. Edge and overlapping pages are retained, and disjointness from the executable and
     the other modules is proven;
   - **(c)** a consuming or tombstoned module registry, so the model module cannot be looked up after reclaim. Audit
     every `MODULE_REQUEST` user, and replace the "module bytes live forever" contract deliberately;
   - **(d)** the boot census Chad specified: module bytes, mapped heap capacity, heap used, managed buddy free pages,
     and tensor block capacity.
2. **D2 stands (0.5B first). The 1792 MiB threshold is withdrawn.** The tier rule is B2, and it must admit on
   managed allocatable pages with a tier-specific measured term. A host KAT must include fragmented memory maps.
   B1 publishes a measured minimum-RAM figure from the census.
3. **D3–D5 stand.**
4. **The failure bank gains** F7 (an xtask manifest-count hard failure before boot, plus kernel module-capacity proof
   and logging), an externally timed F2, the corrected F6 breadcrumb (`no valid NeoFS superblock` on the OptiPlex's
   NVMe), and F3 split in two.
5. **Integrity details** (Chad, item 4): import the same pinned raw GGUF twice and compare byte-for-byte. Pin the
   importer version and the upstream commit/LFS SHA-256. If the Limine path suffix is used, it is BLAKE2b, not BLAKE3.
6. **Review:** B1 now changes the memory/allocator core. Its AAG needs a cross-family lens on C1–C3 (Chad, who found
   them, or Jarvis).
