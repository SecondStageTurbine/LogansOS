# PM-DISPOSITION: F-006 Cerebro diagnosis close

- Defect: F-006, bare-metal HHDM coverage short of Limine module placement
- Inputs: NOTES `e2dedf9`, DISPATCH `76f6586`, CEREBRO-OUTPUT `c6dc59a`
- Author: PM (Cerebro/Claude)
- Date: 2026-05-25

## 1. Disposition

**ACCEPT** Cerebro's recommendation: option D / refined B — actual-span HHDM repair and assertion, driven by Limine executable + module responses. PM proceeds to Jarvis implementation dispatch under operational scope class. No PL ratification required.

## 2. Cerebro recommendation (summary)

Derive a boot-time required-HHDM set from Limine's authoritative responses (executable address + module file data spans), not from memmap-type filtering. In `mm::init`, after `HHDM_OFFSET` is set and the boot frame allocator is available, verify each required span translates at `hhdm_offset + phys`; install HHDM leaf mappings for any missing span; update `HHDM_PHYS_END = max(usable_end, required_span_end)`; assert post-repair coverage. Hypothesis confirmed at source (`kernel/src/mm/mod.rs:66/85/89`); kernel does not build HHDM from scratch (Limine provides page tables, kernel wraps via `OffsetPageTable` at `paging.rs:34`). Faulting path traced: `main.rs:132-160 spawn_neo_init` → `process.rs:917 Elf::parse(elf_bytes)` → `elf.rs:98-107 bytes.as_ptr()` deref.

## 3. Open dependency resolutions

Cerebro §8 flagged three open items. PM resolves operationally:

### 3.1 BootInfo plumbing into `mm::init`

**Resolution:** introduce a small `BootHhdmRequirements` struct (or equivalently-named type) collected in `main.rs` before `mm::init`, passed in as a third arg to `mm::init(hhdm_offset, memmap, boot_hhdm_reqs)`. Rationale: keeps the contract explicit and documented; future Limine response additions (e.g. framebuffer span, RSDP page) extend the struct without re-touching every call site; aligns with Bible 4.2 reviewability (the contract is a named type, not an ad-hoc tuple).

Shape (sketch, Jarvis owns final form):
```rust
pub struct BootHhdmRequirements {
    pub executable_phys_start: u64,
    pub executable_phys_end: u64,
    pub modules: &'static [BootModuleExtent], // or ArrayVec<_, N>
}
pub struct BootModuleExtent {
    pub name: &'static str, // for assertion-failure diagnostics
    pub phys_start: u64,
    pub phys_end: u64,
}
```

### 3.2 Executable span inclusion

**Resolution:** include the executable image span in the same helper. Rationale: the bug class is "any Limine-placed object the kernel dereferences via HHDM without contract." Executable image is one such object; the marginal cost (one extent in the required set) is trivial; covering it now closes a latent variant of the same defect rather than waiting for it to surface. Use `ExecutableAddressResponse.physical_base` plus kernel image size from linker symbols (`__kernel_start` / `__kernel_end` or equivalent — Jarvis confirms the actual symbol names via RAG).

### 3.3 QEMU repair-path test coverage

**Resolution:** add a **test-only forced-unmap hook** behind a cfg gate (e.g. `cfg(feature = "test-hhdm-repair")` or `cfg(neo_test_force_unmap)`), not a synthetic page-table harness. Rationale: forced-unmap exercises the actual repair code path against the real page tables, so CI failure modes match bare-metal failure modes. Synthetic harness tests arithmetic but not interaction with the live `OffsetPageTable`. The hook unmaps one module's first HHDM leaf right before the repair/assert step runs, so the assertion-only path and the repair path both get coverage.

Two `neo-test` scenarios:
- `bare_metal_hhdm_covers_modules` — default path, asserts coverage without forced unmap (catches the A-style regression where required spans aren't included).
- `bare_metal_hhdm_repair_path` — with the forced-unmap cfg/hook, asserts the repair path runs and post-repair translation succeeds.

Plus the pure helper test for `required_hhdm_phys_end(usable_end, exec_extent, module_extents)` per Cerebro §5.

## 4. Implementation lane (Jarvis scope)

Code-bearing change. Lands as one or two commits on `main` directly.

1. **Boot-info collection in `main.rs`** before `mm::init`:
   - Read `ExecutableAddressResponse`, compute exec span.
   - Iterate `ModuleResponse`, collect each module's `(name, phys_start, phys_end)` where `phys = file.addr() - hhdm_offset`.
   - Build `BootHhdmRequirements` and pass to `mm::init`.
2. **`mm::init` extension**:
   - Accept `BootHhdmRequirements`.
   - Compute `required_hhdm_phys_end = max(usable_end, exec_end, max(module_end))`.
   - For each required span: walk page tables, check translation at `hhdm_offset + phys`. If missing, install HHDM leaf mappings using the local boot frame allocator (NOT `FRAME_ALLOC` — ADR-016 substrate discipline).
   - Store `HHDM_PHYS_END = required_hhdm_phys_end` (conservative).
   - After repair, assert each required span's first and last page translate. Panic with diagnostic on failure (boot is unrecoverable).
   - Emit serial log line per boot: `[hhdm] required_end=0x... exec=[0x...,0x...) modules=[...] repair_pages=N`.
3. **Pure helper** `required_hhdm_phys_end(usable_end, exec_extent, module_extents)` with unit test.
4. **`neo-test` scenarios**:
   - `bare_metal_hhdm_covers_modules` (default path).
   - `bare_metal_hhdm_repair_path` (with forced-unmap cfg).
5. **Forced-unmap test hook** behind cfg gate; default-off.

Constraints:
- Bible 4.2: `unsafe` blocks for page-table mutation must be narrowly scoped with SAFETY comments naming the invariant.
- Bible 7.2: only upper-half HHDM aliases added; lower half untouched.
- ADR-016: all page-table work runs in single-threaded boot context using local boot frame allocator, before AP bring-up and scheduler start, with no heap allocation under scheduler locks.
- No changes to ADR-016 substrate or any Bible-named invariant.

## 5. Acceptance criteria

**CI:**
- `cargo k` green.
- `required_hhdm_phys_end` helper unit test passes.
- `neo-test` scenarios `bare_metal_hhdm_covers_modules` and `bare_metal_hhdm_repair_path` pass.
- All existing scenarios remain green.
- Per-commit perf harness proxies remain within Q4 thresholds.

**Bare-metal (OptiPlex 7080):**
- ≥10 consecutive boots from the same ISO, no power-cycle requirement between boots.
- Every boot reaches scheduler-idle (post-`spawn_neo_init`, hog reaped, ticks running).
- Every boot's serial log shows the `[hhdm]` line with concrete `required_end` and per-module spans.
- Zero PAGE_FAULTs.
- Boot 11+ allowed for confidence; 10 is the floor.

Single successful boot is **not** acceptance. Placement-intermittence per NOTES §11 means one success can be variance, not fix.

## 6. Scope class

**Operational.** Per Cerebro §6:
- No Bible amendment.
- No new ADR.
- No ADR-016 substrate change.
- `unsafe` budget impact is bounded and within Bible 4.2 allowed category (hardware-boundary page-table work).
- HHDM policy change is local (repair specific spans), not a project-wide direct-map policy shift.

PM proceeds. PL is courier + ratifier on the eventual F-006 close report once acceptance criteria pass.

## 7. Downstream dispatch

Next artifact: `Plans/DISPATCH-2026-05-25-F006-jarvis-implementation.md` with full Jarvis brief (file targets, signature sketches, test design, time bound). PM authors that next.

Tell-PL one-liner for the Jarvis hand-off (paste after dispatch is filed):

> Jarvis — F-006 implementation dispatch. PM accepted Cerebro's option D (`Plans/CEREBRO-OUTPUT-2026-05-25-F006-diagnosis.md`, `c6dc59a`) with open dependencies resolved in `Plans/PM-DISPOSITION-2026-05-25-F006-cerebro-diagnosis-close.md`. Brief in `Plans/DISPATCH-2026-05-25-F006-jarvis-implementation.md`. Code-bearing. Lands on `main`. Acceptance criteria §5. Status 1 on push is cosmetic.

## 8. Outstanding (not blocking this disposition)

- PROCEDURE §3.3 baud-rate correction (115200 → 38400). Separate small PM-DISPOSITION patch, deferred until after F-006 close. Not on the critical path.
- F-006 close report after re-verification passes. Files Phase 2 formal close.
- NVMe FLUSH durability diagnosis (BUILD-5b) remains paused — Phase 3 work.

## 9. Final call

Accept option D. Three open dependencies resolved: `BootHhdmRequirements` struct, executable span included, forced-unmap cfg hook for QEMU repair-path coverage. Operational scope. PM authors Jarvis dispatch next.
