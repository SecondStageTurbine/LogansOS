# DISPATCH: F-006 implementation (Jarvis)

- Defect: F-006, bare-metal HHDM coverage short of Limine module placement
- Authority: PM-DISPOSITION `d53c7c2` (`Plans/PM-DISPOSITION-2026-05-25-F006-cerebro-diagnosis-close.md`) accepting Cerebro option D
- Inputs: NOTES `e2dedf9`, CEREBRO-OUTPUT `c6dc59a`, PM-DISPOSITION `d53c7c2`
- Worker: Jarvis (Codex, heavy coding, kernel source)
- Time bound: 4 hours (code + tests + push)
- Output: one commit (or two: code + tests) on `origin/main`. No PR. CI green.

## 1. Goal

Make F-006 structurally impossible: derive HHDM coverage requirements from authoritative Limine responses (executable + modules), repair any missing leaf mappings during `mm::init`, assert post-repair coverage before any boot code can dereference module bytes. Bug class closes; placement variation tolerated.

## 2. File targets

Use the RAG at `http://127.0.0.1:8404` for navigation. Don't brute-enumerate.

- `kernel/src/main.rs` — boot-info collection of executable + module spans before `mm::init` call.
- `kernel/src/mm/mod.rs` — extend `init` signature with `BootHhdmRequirements`; compute required end; call repair helper; emit serial diagnostic line; assert coverage.
- `kernel/src/mm/paging.rs` (or new `kernel/src/mm/hhdm_repair.rs` if Jarvis prefers module separation) — repair helper that maps missing HHDM leaves using local boot frame allocator.
- New: `BootHhdmRequirements` and `BootModuleExtent` types — place in `kernel/src/mm/mod.rs` or a small `kernel/src/boot_info.rs` if Jarvis judges it cleaner.
- Tests: pure helper test in `kernel/src/mm/` (cfg(test) module).
- New `neo-test` scenarios: `bare_metal_hhdm_covers_modules` and `bare_metal_hhdm_repair_path`. Wire into existing harness — match the pattern of the most recent Phase 2 scenarios (e.g. `q7c_shootdown_timeout_panic`).

## 3. Signature sketches

PM-DISPOSITION §3.1 sketch is non-binding shape, not final form. Jarvis owns the final API. Constraints:

- `BootHhdmRequirements` is a value passed by reference into `mm::init` — not a global, not a `OnceCell`. Pre-heap, single-threaded boot context.
- Module names captured for diagnostics (`&'static str` from the Limine response is fine; lifetime tied to bootloader-reclaimable, which is still live during `mm::init`).
- Sizing executable span: use `ExecutableAddressResponse.physical_base` plus image size from linker symbols. RAG-search for existing `__kernel_*` symbol references; if none exist, Jarvis introduces the linker-symbol pair as part of this work (small, local change).

## 4. Repair helper contract

- Inputs: `hhdm_offset`, required spans (exec + modules), local boot frame allocator.
- For each required span:
  - Walk page tables via the active `OffsetPageTable` (`paging.rs:34`).
  - Check translation at every 4 KiB or 2 MiB boundary covering the span.
  - For each missing leaf, install an HHDM mapping using the boot frame allocator. Prefer 2 MiB leaves where alignment permits; fall back to 4 KiB otherwise.
- Post-repair: every required span's first and last byte must translate. Assert this. Panic on failure with diagnostic naming the failing span.

**Hard constraints (Bible / ADR):**
- All `unsafe` blocks narrowly scoped, each with SAFETY comment naming the invariant.
- Use the **local boot frame allocator** passed into `mm::init`, not `FRAME_ALLOC`. ADR-016 substrate discipline — heap and scheduler are not yet live.
- All page-table work runs single-threaded, pre-AP-bringup, pre-scheduler-start.
- No heap allocation in the repair path.
- Upper-half HHDM only; never touch lower-half user space.
- No changes to Bible 7.2 split, no changes to ADR-016 substrate, no new ADR.

## 5. Serial diagnostic

Emit one line per boot, before the assertion runs:

```
[hhdm] required_end=0x47d800000 exec=[0x66b5f000,0x66c80000) modules=[neo-init=[0x47d4ba000,0x47d7ffbe8),neo-worker=[...]] repair_pages=N
```

Format is illustrative; concrete fields:
- `required_end` — final `HHDM_PHYS_END` value.
- `exec` — half-open phys range.
- `modules` — comma-separated `name=[start,end)` for every module.
- `repair_pages` — count of leaf mappings installed by the repair helper this boot (0 if everything was already mapped).

This line is the per-boot evidence PM needs for the ≥10-boot acceptance evaluation.

## 6. Tests

### 6.1 Pure helper (cfg(test))

```rust
fn required_hhdm_phys_end(usable_end: u64, exec: PhysExtent, modules: &[PhysExtent]) -> u64
```

Cases:
- empty modules → `max(usable_end, exec.end)`.
- one module above usable → `max(exec.end, module.end)` (the F-006 captured shape).
- modules below usable → `usable_end` (no change).
- multiple modules, one straddling usable boundary → correct max.
- exec span equal to usable end → no over-count.

### 6.2 `neo-test` scenario: `bare_metal_hhdm_covers_modules`

Default path. Boots, runs the assertion-only path (no forced unmap), confirms `repair_pages=0` is acceptable when modules are already mapped. Acts as regression guard for "someone removed exec/module from the required set" (A-style regression).

Pass condition: scenario boots to scheduler-idle, serial contains `[hhdm]` line, no PAGE_FAULT.

### 6.3 `neo-test` scenario: `bare_metal_hhdm_repair_path`

With forced-unmap cfg hook enabled. Boots, the hook unmaps one module's first HHDM leaf right before `mm::init`'s repair step, repair runs, post-repair assertion passes, scenario reaches scheduler-idle.

Pass condition: scenario boots to scheduler-idle, serial contains `[hhdm]` line with `repair_pages >= 1`, no PAGE_FAULT.

### 6.4 Forced-unmap cfg hook

Place behind a feature flag or `cfg(neo_test_force_unmap_module)` attribute. Default-off in normal builds. Only the `bare_metal_hhdm_repair_path` scenario enables it. Document the gate in the cfg attribute's adjacent comment.

## 7. Acceptance criteria (Jarvis-facing)

- `cargo k` green.
- All existing `neo-test` scenarios green.
- Two new `neo-test` scenarios green.
- Pure helper unit test green.
- Per-commit perf harness proxies (Q4 thresholds) green.
- One push to `origin/main` (status 1 cosmetic).
- Output: brief commit messages naming F-006 and PM-DISPOSITION `d53c7c2`.

PM and PL then carry the bare-metal ≥10-boot re-verification — not Jarvis's responsibility.

## 8. Discipline

- Recommend the simplest implementation that satisfies the contract. Don't over-engineer the struct shape.
- No ADR draft. No Bible amendment. No scope-class escalation.
- If during implementation Jarvis finds the recommendation cannot be cleanly implemented under the constraints (e.g. the local boot frame allocator isn't actually available where needed), STOP, file a brief blocker note under `Plans/JARVIS-BLOCKER-2026-05-25-F006-implementation-<topic>.md`, push, and surface back to PM. Do not change scope unilaterally.
- If a fourth open dependency surfaces (e.g. linker symbol for kernel image end doesn't exist and adding it is non-trivial), same blocker pattern — don't expand the dispatch.
- Status 1 on `git push` is cosmetic. Don't retry.

## 9. Tell-Jarvis text

Jarvis — F-006 implementation dispatch. Code-bearing. PM accepted Cerebro option D in `Plans/PM-DISPOSITION-2026-05-25-F006-cerebro-diagnosis-close.md` (`d53c7c2`); full brief in `Plans/DISPATCH-2026-05-25-F006-jarvis-implementation.md`. Goal: derive required HHDM coverage from Limine executable + module responses, repair missing leaves in `mm::init`, assert coverage before any boot code dereferences module bytes. Three open dependencies resolved in PM-DISPOSITION §3 — `BootHhdmRequirements` struct, executable span included, forced-unmap cfg hook for QEMU repair-path coverage. Use the RAG at `http://127.0.0.1:8404` for source navigation. Two new `neo-test` scenarios: `bare_metal_hhdm_covers_modules` (default path) and `bare_metal_hhdm_repair_path` (with forced-unmap hook). Plus pure helper unit test. Hard constraints: Bible 4.2 narrow unsafe with SAFETY comments, ADR-016 substrate (local boot frame allocator, NOT `FRAME_ALLOC`), pre-AP-bringup single-threaded context. Acceptance §7. Discipline §8 — blocker pattern for surprises. Time bound 4 hours. Push to `origin/main`. Status 1 cosmetic.

---

**End of dispatch.**
