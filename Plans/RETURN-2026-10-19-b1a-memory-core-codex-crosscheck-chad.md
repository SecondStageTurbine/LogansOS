<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: B1a memory-core cross-check — Chad

**Date:** 2026-10-19

**Branch:** `codex/b1a-crosscheck-chad`

**Reviewed:** `claude/b1-arthur-boot-module` at `edb620bcb6e30d382b62c178a76e82d15540ea68`

**RAG:** healthy (`ollama=true`, `qdrant=true`). I used it for conceptual discovery, then verified every path below in the checked-out source.

## Verdict

**APPROVE — no merge-blocking findings.**

- **C2: CLOSED.** Reclaim starts from the exact Limine file range, admits only inward-rounded whole pages with complete type-6 coverage and no foreign-type/executable/other-module overlap, and places them in an isolated allocator zone. The registry gives consumable modules one non-cloneable lease; `ModuleLease::bytes` borrows from that lease and `reclaim(self)` cannot run while such a borrow is live. Current direct `MODULE_REQUEST` and `File::data()` users do not retain a consumable-module slice.
- **F7: CLOSED.** The kernel emits `BOOTMOD count=` and each module's path/size before its eight-entry extent check, while the single xtask Limine-config writer rejects more than eight `module_path:` lines after all optional firmware, smoke, and model injections.

I found no input that lets a valid admitted block coalesce outside its owned interval, route between zones, or overlap another zone. I also found no current path to obtain two leases or a lease plus a registry `find` result.

## Dispatch-item verdicts

### 1. `'static` hazard — **PASS; C2 CLOSED**

Checked every exact `MODULE_REQUEST`, `File::data()`, and `find_module_by_path_suffix` occurrence. With `model-boot-module` enabled, process spawn (`kernel/src/sched/process.rs:2278`), MT7601U firmware (`kernel/src/drivers/usb/mt7601u/mod.rs:93`), neo-init, and the std smoke all go through `boot_modules::find`, which refuses any path containing `/models/` (`kernel/src/boot_modules.rs:65-73`). The only remaining `File::data()` for consumables is private `module_bytes`, used to construct the sole lease after the per-index PRESENT→TAKEN CAS (`kernel/src/boot_modules.rs:56-63,75-93`). `ModuleLease` is neither `Copy` nor `Clone`; its byte accessor returns a borrow tied to `&self`, so safe Rust cannot move it into `reclaim(self)` while that borrow is live (`kernel/src/boot_modules.rs:108-128`). A failed plan/admission keeps the entry tombstoned and the physical pages retained.

The early HHDM structures retain only copied physical start/end integers. `limine_file_data_extent` reads the `File` prefix without constructing a slice, `BootModuleExtent` stores a `PhysExtent`, and the repair walk consumes those numbers before the registry can reclaim anything (`kernel/src/main.rs:180-190,600-628`; `kernel/src/mm/mod.rs:235-304`). The checked-out tree has no pre-registry cache of a model pointer.

### 2. Zone disjointness and coalescing — **PASS**

`admit_external_region` rejects empty/unaligned ranges and intersection with the primary or any admitted owned interval before writing a free-list header (`kernel/src/mm/frame.rs:535-581`). Each zone has private free-list heads. Its `base_phys` may precede `start_phys`, but initial blocks tile only `[start_phys,end_phys)` and are naturally aligned; a lower buddy cannot merge because it is not present on that zone's list. The upper-bound check stops an upper buddy at `end_phys`, and induction over successful buddy removal keeps every merged block within the original tiling (`kernel/src/mm/frame.rs:306-343`; `kernel/src/mm/module_reclaim_logic.rs:167-220`).

Allocation tries the primary and then each admitted zone; free routing first tests the primary's owned range and then admitted owned ranges (`kernel/src/mm/frame.rs:456-490`). Disjoint admission prevents two owners. `allocate_blocks` / `deallocate_blocks` retain the order and count `2^order` pages; the scenario covers order 0 and order 8 reuse. I also checked adjacent disjoint admitted ranges with the same rounded-down buddy base: separate free lists prevent cross-zone discovery/coalescing.

### 3. Kill-switch — **PASS (behavioral equivalence)**

With no admitted zone, allocation calls the unchanged primary-zone algorithm first; free's `has_admitted()==false` branch calls the unchanged primary `free_block`; accounting decrements on that successful path exactly as before. Observability starts from the primary summary and folds an empty admitted iterator. Debug hooks explicitly read the primary heads (`kernel/src/mm/frame.rs:443-533,609-662`). With `model-boot-module` off, no admission entry point or registry is compiled.

The wrapper means the generated machine code is not literally byte-identical to pre-B1a—the zero-zone free path evaluates `has_admitted`, and OOM scans two empty slots—but allocation/free results, counters, summaries, and debug-hook observations are unchanged. I interpret the dispatch's “bit for bit” as state/behavior equivalence; there is no semantic kill-switch failure.

### 4. Planner inputs — **PASS**

`executable_address().physical_base + (__kernel_image_end - __kernel_image_start)` describes the relocated kernel's contiguous linker-image span, including aligned PT_LOAD gaps and BSS (`kernel/src/main.rs:157-173`; `kernel/linker.ld:20-102`). `limine_file_data_extent` uses the module data address and exact file size, matching `File::data()` without dereferencing it before HHDM repair (`kernel/src/main.rs:175-190`). All additions and outward alignments used by the planner are checked; an overflowing memmap entry refuses the entire plan.

The planner rejects any non-type-6 entry touching a candidate page, requires gap-free type-6 coverage even across overlapping entries, rounds the module inward, and rounds the executable/other modules outward (`kernel/src/mm/module_reclaim_logic.rs:101-165`). Thus an oversized or overlapping type-6 description does not grant adjacent pages: only exact module pages are candidates, with every live kernel/module range excluded.

### 5. Locking and IRQs — **PASS**

Planning is allocation-free and occurs before the allocator lock. Admission executes inside the existing `with_frame_alloc` interrupt-disabled, globally serialized critical section (`kernel/src/boot_modules.rs:122-128`; `kernel/src/mm/mod.rs:436-490`). The closure performs only allocator mutation; `serial_println!` and the post-result module metadata read happen after the guard and reentry marker are dropped. I found no new nested `with_frame_alloc`, reverse lock order, or IRQ-context dependency.

### 6. Coverage — **PASS WITH NON-BLOCKING GAPS**

The host KATs cover real 0.5B tail alignment, unaligned heads, no-whole-page refusal, overlapping/gapped type-6 coverage, foreign types, executable/other-module overlap, fragmented maps, overflow, exact block tiling, and rounded zone bases. The QEMU scenario covers forged-plan refusal, tombstoning, admission, order-0 write/free/reallocate identity, order-8 write/free, census deltas, and final accounting.

It does not exercise a module adjacent to the primary zone, the second admitted-zone slot, concurrent SMP allocations/frees from an admitted zone, or the OptiPlex memmap. The cheapest additions are a two-zone kernel scenario and host tests that model adjacent owned intervals; SMP and real-hardware layout remain appropriate bare-metal evidence. None exposes a source-level counterexample in this review.

## F7 verification

`boot_modules::log_boot_modules` runs before the fixed-array length panic (`kernel/src/main.rs:600-614`). `write_limine_conf` injects firmware, std-smoke, and model lines before calling `check_boot_module_count`, and all ISO assembly reaches that one writer (`tools/xtask/src/main.rs:12891-12972`). The equality KAT ties xtask's eight-module ceiling to `MAX_BOOT_MODULE_EXTENTS` and the over-limit KAT proves the hard failure.

## Commands run

- `cargo test -p module-reclaim-logic-tests -p xtask` — **PASS** (13 + 36 tests).
- `git diff --check origin/main...HEAD` — **PASS**.
- `cargo clippy -p neoplutarch-kernel -Zjson-target-spec --features neo-test-boot-module-reclaim -- -D warnings` — the requested feature compiled through the reviewed code, then the command failed on 13 pre-existing `dead_code` diagnostics outside the B1a diff (IDT test counters, boot-frame helpers, scenario phases, and one test panic helper). No B1a diagnostic was emitted.

The user-owned `.gitignore` modification and untracked `.ignore` were left untouched.
