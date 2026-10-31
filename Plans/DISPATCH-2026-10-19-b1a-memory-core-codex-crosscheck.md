<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex cross-check of B1a (boot-module reclaim, second allocator zone, module registry) → Chad

**Type:** review only. **Deliverable:** one pushed RETURN.

**Why Chad:** you found C1–C4 on the Arthur first-boot THINK
(`Plans/RETURN-2026-10-19-arthur-first-boot-model-crosscheck-chad.md` on
`origin/codex/arthur-first-boot-model-crosscheck-chad`). B1a is the memory-core half of the answer to C2 and part of
C3/C4. Its author and its first AAG lens are Claude-family. This touches the frame allocator, so CLAUDE.md requires a
different-family lens before merge. If Chad is busy, Jarvis or Janus takes it unchanged.

## What is under review

The branch is `claude/b1-arthur-boot-module`. The PM names the head SHA when relaying. Review this diff:

```
git diff origin/main...origin/claude/b1-arthur-boot-module
```

The split (B1a = memory core, B1b = the model on it) is in `Plans/NOTE-2026-10-19-b1-split-proposal-s1.md`. One
change from that note: the right-sized heap and the tensor-capacity census line moved to **B1b**, because both need
the model to be measured. B1a has no model and makes no Arthur change.

What B1a adds:
- **The planner**, `kernel/src/mm/module_reclaim_logic.rs`. It is pure and host-tested (13 KATs in
  `tools/module-reclaim-logic-tests`). `plan_module_reclaim` starts from the module's *exact* file range, never from
  a memmap entry, and:
  1. rounds the range inward to whole pages;
  2. refuses if any non-type-6 entry overlaps those pages;
  3. requires type-6 coverage of every byte, even through overlapping entries;
  4. refuses if the executable or another module, each rounded outward, overlaps.

  `blocks` tiles the planned pages into naturally aligned buddy blocks. `zone_base` gives the zone's buddy base.
- **The allocator**, `kernel/src/mm/frame.rs`. The old single-zone struct is now a private `Zone`, with its
  algorithms unchanged. `BuddyAllocator` is a primary `Zone` plus up to `MAX_ADMITTED_ZONES = 2` admitted zones.
  `admit_external_region` checks alignment, checks disjointness from every zone, and pushes blocks; it never calls
  `free`. Allocation tries the primary first. A free is routed by address. A frame no zone owns is counted in
  `REFUSED_FREES` and is not freed. `FRAMES_IN_USE` is decremented only on a successful free.
- **The registry**, `kernel/src/boot_modules.rs`, under the `model-boot-module` feature. A path containing `/models/`
  is consumable:
  - `find` never returns it;
  - `take` hands it out once as a `ModuleLease`, through a CAS from PRESENT to TAKEN;
  - `ModuleLease::reclaim(self)` consumes the lease, then plans and admits.

  `find_module_by_path_suffix` and `spawn_neo_init` go through the registry under the feature.
- **The census** (`mm::boot_census`), and **F7**: the kernel logs `BOOTMOD count=` before the capacity panic, and
  xtask hard-fails when it would stage more than 8 modules.
- **The scenario** `boot_module_reclaim`. It stages a 6 MiB + 2560 B probe, which has the same unaligned tail as the
  real 0.5B `.neom`. It runs a forged-plan control, a re-borrow control, admit → alloc → write → free → realloc, and
  census deltas.
- **The pitfall bank**: CP-62 (CP-61 was taken by OT-22 rung 1, #58) in `docs/concurrency-pitfalls.md`.

## Your job: refute it

1. **The `'static` hazard (your C2).** Find any path where a slice of a consumable module outlives its reclaim. Check:
   - every `MODULE_REQUEST` user;
   - `module.data()` and `find_module_by_path_suffix` callers (process spawn, `mt7601u` firmware, `main.rs`);
   - the HHDM repair spans and the extent arrays built at early boot;
   - anything that caches a module pointer before the registry exists.

   Is there a way to get two leases, or a lease plus a `find`?
2. **Zone disjointness and coalescing.** An admitted zone's buddy base is `zone_base(start)`, which is *below* the
   admitted start. Find an input where:
   - a coalesce merges a block that reaches outside `[start_phys, end_phys)`;
   - a primary-zone free is routed to an admitted zone, or the reverse;
   - two zones end up owning the same frame.

   Check `allocate_blocks` / `deallocate_blocks` (multi-order runs) and `free` of a block whose buddy lies across a
   zone edge.
3. **The kill-switch.** With the feature off, or with zero admitted zones, is every allocator path the old code, bit
   for bit? This includes `free_block`'s early path, the accounting, `observability_free_summary`, and the debug
   hooks.
4. **The planner's inputs.** Is `executable_address()` + `kernel_image_size()` the executable's real physical extent?
   Do `limine_file_data_extent`'s extents match `module.data()` exactly? Is there a memmap Limine can produce that the
   planner accepts but that is unsafe? Think of type 6 describing bootloader structures, or an entry whose
   `base + length` overflows.
5. **Locking and IRQs.** `reclaim` calls `admit_external_region` inside `with_frame_alloc`, and logs after the lock
   is dropped. Is there a lock-order or IRQ-context hazard? Check the CP-60 placement lesson: the assert sits after
   the lock and the recheck.
6. **Coverage.** What does `boot_module_reclaim` not exercise? For example:
   - a module that spans a primary-zone edge;
   - real hardware memmaps (the OptiPlex);
   - a second admitted zone;
   - SMP allocation from the admitted zone.

   Which of these is cheap to add?

A finding bounces the branch only if it survives your own re-verification. "No findings" must state what you checked
for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/b1a-crosscheck-chad
          (git fetch origin && git checkout -B codex/b1a-crosscheck-chad origin/claude/b1-arthur-boot-module)
```

```
git fetch origin
git show origin/claude/b1-arthur-boot-module:Plans/DISPATCH-2026-10-19-b1a-memory-core-codex-crosscheck.md
git show origin/claude/b1-arthur-boot-module:Plans/NOTE-2026-10-19-b1-split-proposal-s1.md
git diff origin/main...origin/claude/b1-arthur-boot-module
```

Optional runs:
- `cargo test -p module-reclaim-logic-tests`
- `cargo test -p xtask`
- `cd kernel && cargo clippy -Zjson-target-spec --features neo-test-boot-module-reclaim -- -D warnings`
- `cargo xtask neo-test boot_module_reclaim`

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='buddy allocator free routing admitted zone coalesce'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-b1a-memory-core-codex-crosscheck-chad.md` to your branch and **push it**. It must
contain:
- a verdict per item;
- each finding with its severity, file:line, and a repro or the reasoning;
- for each item, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never delete the
worktree.
