# CEREBRO OUTPUT: F-006 diagnosis

- Defect: F-006, bare-metal HHDM coverage short of Limine module placement
- Dispatch: `Plans/DISPATCH-2026-05-25-F006-cerebro-diagnosis.md`
- Inputs: F-006 NOTES through `e2dedf9`, dispatch through `76f6586`, OptiPlex 7080 serial evidence
- Discipline: diagnosis and recommendation only; no patch code, no ADR draft, no Bible amendment

## 1. Recommendation

Recommend **D: actual Limine-owned span repair and assertion**, a narrowed form of alternative **B**.

In PM shorthand: carry this as **"B, but map/assert the explicit executable/module spans, not a continuous max-fill just because one module landed high."**

The implementation lane should:

1. Build a boot-time required-HHDM set from the actual Limine responses:
   - module data spans from every `ModuleResponse` file address and size;
   - executable image span from `ExecutableAddressResponse` plus the kernel image size already known from linker symbols, if needed;
   - optionally bootloader response/file metadata spans if later code starts dereferencing them past current early usage.
2. During `mm::init`, after the HHDM offset and a local frame allocator are available, verify each required span translates at `hhdm_offset + phys`.
3. If a required span is missing, install HHDM leaf mappings for that explicit span.
4. Store a conservative `HHDM_PHYS_END = max(usable_end, required_span_end)` for heap-collision accounting.
5. Assert after repair that every module first/last page translates and that `HHDM_PHYS_END >= max(module_phys_end)`.

If PM wants a strict A/B/C selection only, choose **B**, with the above refinement. Do not choose A. Do not choose broad C for Phase 2 unless PL explicitly wants a Linux-style full direct map policy.

## 2. Hypothesis Confirmation

Confirmed with refinement.

The source confirms the dispatch's key observable: `mm::init` computes `HHDM_PHYS_END` from `MEMMAP_USABLE` entries only:

- `kernel/src/mm/mod.rs:66` is `mm::init(hhdm_offset, memmap)`.
- `kernel/src/mm/mod.rs:85` filters `e.type_ == limine::memmap::MEMMAP_USABLE`.
- `kernel/src/mm/mod.rs:89` stores that result in `HHDM_PHYS_END`.
- `kernel/src/mm/mod.rs:98` separately uses the same `MEMMAP_USABLE` filter for the usable-memory byte count.

But the phrase "HHDM-build logic" needs correction. This kernel does **not** build the original HHDM from scratch. `kernel/src/mm/paging.rs:3` says Limine already set up the page tables, and `kernel/src/mm/paging.rs:34` only wraps the active CR3 table in an `OffsetPageTable`. The local kernel then mutates page tables for GLOBAL (`paging.rs:49`) and heap/MMIO mappings, but it does not currently create a complete direct map.

Therefore the exact bug is:

- the kernel has no boot-time contract check that the Limine module data spans it will dereference are actually mapped in the active HHDM;
- its only stored HHDM extent is usable-only metadata, which is too low on the OptiPlex run;
- the module pointer is then dereferenced as if HHDM coverage were guaranteed.

The serial evidence matches this:

- executable phys: `0x66b5f000` (`Plans/evidence/...BLOCKER.serial.log:1088`)
- `hhdm_phys_end=0x47d175000` (`:1093`)
- `neo-init` bytes at `0xffff80047d4ba000`, size `3431400` (`:1245`)
- page fault CR2 at that exact module virtual address (`:1247`)

Subtracting `hhdm_offset=0xffff800000000000`, the module starts at physical `0x47d4ba000`, which is `0x345000` / 3.27 MiB above the recorded usable-only HHDM end. The module end is `0x47d7ffbe8`, about 6.54 MiB above that end and just below the OptiPlex high RAM end `0x47d800000` shown by the Ubuntu e820 line (`:3`).

## 3. Module Placement Reality

The current kernel requests Limine base revision 3 (`kernel/src/main.rs:66`), plus HHDM, memmap, modules, and executable-address responses (`main.rs:74`, `:78`, `:82`, `:86`). It later gets the `neo-init` module, calls `module.data()`, widens the returned slice, logs the pointer, and passes the bytes to `spawn_user` (`main.rs:132-160`). `spawn_user` immediately calls `Elf::parse(elf_bytes)` (`kernel/src/sched/process.rs:917`), and `Elf::parse` dereferences `bytes.as_ptr()` as an ELF header (`kernel/src/loader/elf.rs:98-107`). That is the faulting path.

Limine's protocol says all non-null pointers are returned with the HHDM offset already added, and for base revision 3 the HHDM maps only selected memmap types: usable, bootloader reclaimable, executable/modules, and framebuffer. The memory-map feature also defines `LIMINE_MEMMAP_EXECUTABLE_AND_MODULES`, but explicitly says those entries are illustrative and not authoritative for finding executable/module addresses; the executable and module features are authoritative instead. See the official Limine protocol at:

- `https://github.com/limine-bootloader/limine-protocol/blob/trunk/PROTOCOL.md` lines 302-304
- lines 383-388 and 459-466 for base-revision-3 HHDM coverage
- lines 1114-1121 and 1129-1137 for memmap type definitions and the executable/modules caveat
- lines 1143-1146 for sorted/alignment/overlap limits

This makes A weaker than the NOTES originally implied: `EXECUTABLE_AND_MODULES` memmap entries are real but not authoritative. The robust source of truth is the module/executable response itself.

## 4. Alternative Evaluation

### A. Extend coverage by additional memmap types

Shape: include `BOOTLOADER_RECLAIMABLE`, `EXECUTABLE_AND_MODULES`, and perhaps `FRAMEBUFFER` when deriving HHDM coverage or repairing mappings.

- Page-table cost: low if exact memmap ranges are mapped; on the captured OptiPlex miss, the missing top interval is only about 6.54 MiB. If repaired with 4 KiB pages, that is about 1675 leaves; with 2 MiB leaves, 4 leaves cover the aligned top interval.
- Code surface: small for metadata, medium if it actually maps missing entries. Must handle non-usable entries that are not guaranteed aligned and may overlap.
- Unsafe budget: page-table mutation is allowed by Bible 4.2, but this adds unsafe page-table walking/mapping. It is reviewable if local and line-commented.
- ADR-016: acceptable if run during `mm::init` with the local boot frame allocator, before scheduler operation and without heap allocation under scheduler locks.
- Bible 7.2: preserves the upper-half HHDM and lower-half userspace split.
- QEMU testability: easy to test arithmetic with a synthetic memmap; harder to prove real Limine placement variation.
- Placement-variation robustness: incomplete. Because Limine says executable/modules memmap entries are illustrative, A can still miss the actual module address if the response and memmap disagree.

Verdict: rule out. A is better than usable-only but still uses the weaker source of truth.

### B. Compute required end from executable and module responses

Shape: derive `required_hhdm_phys_end = max(usable_end, executable_end, max(module_end))`.

- Page-table cost: low for current evidence. Filling continuously from old `hhdm_phys_end` to `neo-init` end is about 6.54 MiB. If module placement varies to the top of the 7080's high RAM range, the same order of cost applies for this captured machine; on other machines it scales with the gap from usable-max to the highest module.
- Code surface: moderate. `main.rs` or a boot-info helper must collect module extents before `mm::init`, and `mm` must accept the required extents or compute them from a boot-info structure.
- Unsafe budget: acceptable but nonzero. Mapping HHDM leaves is page-table work and needs narrow SAFETY comments.
- ADR-016: acceptable if performed in single-threaded boot using the local `frame_alloc`, not `FRAME_ALLOC` under scheduler context.
- Bible 7.2: preserved. New mappings remain in the upper-half HHDM region.
- QEMU testability: arithmetic is directly testable with synthetic module extents. Full repair-path exercise needs a forced missing-leaf test hook or bootloader support to place a module outside current HHDM coverage.
- Placement-variation robustness: high for executable/module bytes, because it follows actual Limine placements every boot.

Verdict: choose this if constrained to the original alternatives, but refine it to avoid broad continuous max-fill across holes.

### C. Full direct map up to highest memmap entry

Shape: map the whole physical address interval up to the highest memmap end.

- Page-table cost: on the OptiPlex high RAM end `0x47d800000`, the address span is about 17.96 GiB. Efficient 2 MiB leaves require 9196 leaf entries, 18 PD tables, one PDPT table, and the existing PML4 entry; roughly 76 KiB of page-table memory if built cleanly. A naive 4 KiB `map_to` loop would require about 4.7 million leaves and about 36 MiB of PT pages, and is not an attractive boot path.
- Code surface: high if done correctly with huge pages and cache/type awareness. Low only if done naively, which is the wrong trade.
- Unsafe budget: largest of the options. Full direct-map construction is page-table policy, not a small local repair.
- ADR-016: still acceptable if done at boot, but the implementation is a broader memory-manager substrate change.
- Bible 7.2: preserves the split, but changes the practical HHDM policy from Limine's restrictive base-revision-3 map toward a Linux-style direct map.
- QEMU testability: good for arithmetic and broad mapping, but QEMU success is still not proof of real firmware placement behavior.
- Placement-variation robustness: strongest. It handles any module position below the highest memmap entry.

Verdict: rule out for this dispatch. C is structurally robust, but too broad for an F-006 operational fix and risks mapping reserved/MMIO holes with the wrong memory type unless a larger policy is ratified.

### D. Actual-span HHDM repair and assertion

Shape: derive explicit required spans from the actual Limine executable/module responses; map only those spans if absent; update `HHDM_PHYS_END` conservatively; assert coverage.

- Page-table cost: bounded by loaded object size, not by physical address height. For captured `neo-init`, the span is 3,431,400 bytes, about 838 4 KiB pages. Because it crosses two 2 MiB windows, the missing-page-table overhead is at most a few PT pages if the containing PD table already exists. Add `neo-worker` similarly.
- Code surface: moderate and local: boot-info extent collection, one `mm` repair/check helper, post-repair assertion. Less policy surface than C.
- Unsafe budget: acceptable. It is page-table work under Bible 4.2, but targeted and auditable.
- ADR-016: clean. It runs during early `mm::init` using local boot allocator state, before AP/scheduler bring-up and without heap dependence.
- Bible 7.2: preserved. Only upper-half HHDM aliases are added; user lower half is untouched.
- QEMU testability: arithmetic helper is deterministic. A QEMU repair-path test can be made deterministic with a test-only forced-unmap/repair hook or a synthetic page-table harness; without such a hook, normal QEMU will probably only test the assertion on already-mapped modules.
- Placement-variation robustness: high. It follows whatever physical addresses Limine chose on that boot and covers the full variation surface for the actual executable/modules.

Verdict: recommend. D takes B's authoritative source of truth and avoids C's overbroad map.

## 5. Assertion and Test Design

Recommended boot assertion:

- Compute `max_module_phys_end` from the module response by subtracting `hhdm_offset` from each `module.data().as_ptr()` and adding `len`.
- After HHDM repair, assert:
  - `HHDM_PHYS_END >= max_module_phys_end`;
  - each module span's first page translates at `hhdm_offset + phys_start`;
  - each module span's last byte/page translates;
  - optionally, the executable image span translates too.

Where: inside the memory bring-up boundary, after `HHDM_OFFSET` is initialized and before `spawn_neo_init` can run. Practically this means extending the `main.rs -> mm::init(...)` handoff to pass a small boot extent list, or introducing a pre-heap boot-info collection step and letting `mm::init` own the check/repair.

CI design:

- Add a pure helper test for `required_hhdm_phys_end(usable_end, exec_extent, module_extents)` using a synthetic module above usable max. This catches the exact A-style regression cheaply.
- Add a `neo-test` scenario such as `bare_metal_hhdm_covers_modules` that logs module physical spans and asserts module first/last pages translate before `spawn_neo_init`.
- To exercise the repair path in QEMU deterministically, add either:
  - a test-only forced-unmap of one module HHDM leaf followed by the repair helper, or
  - a synthetic page-table harness for the repair helper.

Without one of those hooks, normal QEMU cannot be relied on to reproduce Limine's bare-metal high-placement behavior. QEMU can still run the assertion, but it will usually be an already-mapped success path.

Bare-metal acceptance: confirm the NOTES minimum of **10 consecutive OptiPlex boots** reaching at least post-`spawn_neo_init` / scheduler-idle / boot-complete state, with serial showing no page fault. Amend the bar by adding that the logs should include per-boot module physical span and `required_hhdm_phys_end` so PM can see that the assertion ran each time. A single successful boot remains non-evidence because the defect is placement-intermittent.

## 6. Scope Class

Recommended scope class: **operational**, PM can proceed to implementation dispatch after accepting the recommendation.

Reasoning:

- The fix does not change Bible 7.2's upper/lower-half split.
- It does not change ADR-016's scheduler/heap-lock substrate.
- It stays within Bible 4.2's allowed hardware-boundary `unsafe` category: page-table work.
- It does not require a new ADR if implemented as targeted module/executable HHDM coverage repair plus assertion.

Escalate to PL ratification only if PM chooses broad C or changes the project's named HHDM policy to a full direct-map invariant.

## 7. Alternatives Ruled Out

- A is ruled out because Limine's executable/modules memmap entries are not the authoritative address source. It may fix the captured boot, but it is weaker than the response-derived design against full placement variation.
- Unrefined B is ruled out if interpreted as continuous max-fill through holes. Response-derived extents are correct; broad continuous filling is unnecessary.
- C is ruled out because it is a larger direct-map policy decision with higher unsafe/code surface and memory-type risk.
- "Rely on Limine because the spec says modules should be HHDM-mapped" is ruled out by evidence. The kernel needs a defensive assertion/repair because the hardware boot already observed an unmapped module pointer.

## 8. Open Dependencies

- Implementation must decide whether to pass boot extents into `mm::init` directly or introduce a small `BootHhdmRequirements` structure.
- Executable span sizing needs a precise source. Module span sizing is straightforward from `File::data()` pointer and length; executable span can use linker image bounds plus `ExecutableAddressResponse.physical_base` if PM wants to include it in the same helper.
- The QEMU repair-path test needs either a test-only unmap hook or a synthetic page-table harness; otherwise QEMU only checks the assertion path.

## 9. Final Call

Recommend **D / refined B**: use actual Limine module/executable response spans, repair missing HHDM mappings only for those spans, conservatively update `HHDM_PHYS_END`, and assert module coverage before ELF parsing.

This confirms the NOTES direction that B/C are favored over A, but amends the final choice: D gives B's authoritative placement coverage with C-like robustness for loaded objects, without adopting C's broad direct-map policy.
