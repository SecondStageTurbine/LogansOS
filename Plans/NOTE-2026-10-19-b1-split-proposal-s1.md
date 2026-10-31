<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PM: B1 split proposal, and one environment blocker (S1, 2026-10-19)

**In reply to:** the PM trigger of 2026-10-19 23:20 UTC (B1 widened by Amendment 1 after Chad's C1–C4).
**Branch:** `claude/b1-arthur-boot-module`.

## PM: I propose splitting B1 into two PRs

After the widening, B1 touches the memory core (the allocator and the heap) and also the model path. Gating them as
one PR would put two different blast radii behind one AAG. I propose:

### B1a — memory core: no model, no Arthur change

1. **Boot census (Amendment 1 (d)).** One serial block before and after each step:
   - Limine module count, plus each module's path, base and length;
   - mapped heap capacity and heap used;
   - managed buddy free pages, per zone;
   - admitted-zone pages;
   - tensor block capacity.
2. **Module range planner.** Pure, host-tested, `*_logic.rs`. Its inputs are the Module feature's exact file range
   (the existing `limine_file_data_extent` / `mm::BootModuleExtent`, `kernel/src/main.rs:575-595`), the memmap, the
   executable extent, and every other module's extent. Its output is the set of whole pages that are:
   - inside a type-6 (`EXECUTABLE_AND_MODULES`) entry;
   - page-aligned *inside* the module range, so edge pages are retained;
   - disjoint from the executable and from every other module.

   Checked arithmetic throughout. Host KATs cover:
   - unaligned module ranges (the real 0.5B is `485,489,152 % 4096 = 2560`);
   - overlapping or merged type-6 entries;
   - a module adjacent to the executable;
   - fragmented memmaps;
   - overflow at the address-space top.
3. **Second-zone allocator admission (Amendment 1 (b)).**
   - `BuddyAllocator` becomes a primary zone (today's struct, unchanged) plus up to *N* admitted zones. Each admitted
     zone has its own `base_phys`, free lists and counters.
   - `admit_external_region(plan)` decomposes the planned pages into an admitted zone. It **never** calls `free()`.
   - Allocation tries the primary zone first, then the admitted zones. A free is routed by address to the owning zone.
     A frame owned by no zone is a checked refusal, not a coalesce.
   - Accounting is separate: `ADMITTED_FRAMES` / admitted-free. `FRAMES_IN_USE` cannot underflow from admitted pages.
   - **Kill-switch:** with zero admitted zones (no `model-boot-module`), every path is today's primary-zone code,
     bit for bit.
4. **Consuming / tombstoned module registry (Amendment 1 (c)).**
   - It replaces direct `MODULE_REQUEST` use. The users are `kernel/src/main.rs:198, 261, 575`, through
     `find_module_by_path_suffix` (callers at `kernel/src/sched/process.rs:2256`,
     `kernel/src/drivers/usb/mt7601u/mod.rs:93` and `kernel/src/main.rs:221`).
   - `take_module(path)` hands out the bytes once and tombstones the entry. After that, every lookup of that path
     returns `None`.
   - Only a tombstoned, fully dropped module may be planned for admission.
   - The "module bytes live for the kernel's lifetime" comment is replaced by the new contract: they live forever
     unless consumed.
5. **Right-sized heap (Amendment 1 (a)).**
   - Under `model-boot-module`, the 0.5B heap tier drops the file-staging buffer, because the module replaces it
     (`kernel/src/mm/heap_policy.rs:11, 26`).
   - The new size comes from the census's heap high-water mark in the scenario, plus a stated margin.
   - I prefer right-sizing over a lazily backed heap. Lazy backing changes `heap::init`'s eager mapping
     (`kernel/src/mm/heap.rs:116-170`) for every build. Right-sizing only changes one feature's constant.
6. **F7.**
   - xtask hard-fails when staged modules exceed `MAX_BOOT_MODULE_EXTENTS` (`kernel/src/main.rs:136`, today 8).
   - Licence files ship as plain ISO files under `/licenses/`, not as modules, so they never count against the limit.
   - The kernel logs the module count before it fills the array.
7. **Discriminator.** QEMU `boot_module_reclaim`:
   - xtask stages a deterministic multi-MiB dummy module with a deliberately unaligned length;
   - the kernel takes it and admits its planned pages;
   - it allocates, writes, frees and reallocates from the admitted zone, then asserts the census deltas;
   - lookups of the tombstoned path return `None`.

   Controls:
   - **re-borrow:** a lookup after `take_module` must fail;
   - **type mismatch:** a forged plan that is not type 6 must be refused, with zero pages admitted.

   The pitfall bank (CP-60 and the memory/lifetime entries) is consulted in the PR.

### B1b — the model on that substrate

The 0.5B `.neom` and `.vocab` arrive as modules, through `xtask iso --with-model`. Arthur reads the vocab, and the
model is loaded from the module (no heap `Vec`):
- the hash is computed in the single pass and checked against the pinned digest, failing closed;
- the module is then taken and admitted through B1a.

B1b also carries:
- Arthur's absent and damaged messages;
- the licence texts and `NOTICE` / Bible Article 26;
- the ADR-063 amendment;
- `arthur_boot_module_answers` with its flipped-byte and missing-module controls;
- the measured minimum-RAM figure;
- the F1–F7 bank, with F2 timed externally, F3 split in two, and F6 expecting `no valid NeoFS superblock`.

**Order:** B1a first, then B1b on top. Each gets its own gate and its own cross-family AAG lens. Chad or Jarvis covers
C1–C3 on B1a.

## PM: one environment blocker for B1b's integrity prerequisite

The import-twice check needs the raw upstream GGUF. From this session, `huggingface.co` is **denied by the
environment's network policy**: `curl` to
`huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf` returns HTTP 403 from
the egress proxy. There are two ways forward.

- **Option 1.** The PL adds `huggingface.co` (and its LFS CDN host) to this environment's allowed domains. The setting
  is under Network access in the cloud environment's settings.
- **Option 2.** A seat with access runs the two imports and pushes the result: Chad already pinned the upstream heads.
  The result needs:
  - the GGUF SHA-256;
  - both `.neom` and `.vocab` blake3s;
  - the `cmp` result;
  - the `neomc` commit.

Until then, B1b pins the `.neom` blake3 already recorded from the real model file,
`a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe` (`docs/OPEN-THREADS.md:83`), and marks
upstream reproducibility as pending. **B1a does not depend on this**, so I am starting B1a now.
