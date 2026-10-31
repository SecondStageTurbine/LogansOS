# DISPATCH: F-006 Cerebro diagnosis — bare-metal HHDM coverage short of module placement

- From: PM
- To: Cerebro (Claude — deep research + light coding lane)
- Date: 2026-05-25
- Status: ACTIVE
- Defect ID: F-006
- Prior gate: this is a fresh diagnosis dispatch, not gated on anything beyond F-006 NOTES being filed (`Plans/NOTES-2026-05-25-F006-bare-metal-kernel-hhdm-coverage-short-of-module-placement.md`, commit `83e8112`).
- Time bound: 90 minutes. No checkpoint needed within bound; surface checkpoint if exceeded.
- Output target: `Plans/CEREBRO-OUTPUT-2026-05-25-F006-diagnosis.md`.

---

## 1. Mission

F-006 is a real-bare-metal-only defect. On the OptiPlex 7080, the kernel attempts to ELF-parse the neo-init module bytes through its HHDM mapping, but the HHDM does not cover the physical page where Limine placed the module. Page fault, kernel hang. Phase 2 formal close is now gated on F-006.

Confirm the hypothesis by reading source. Evaluate the three fix alternatives in the NOTES doc (and any fourth alternative Cerebro identifies). Recommend one with reasoning + alternatives-ruled-out.

## 2. Authoritative inputs

Read these before recommending:

- `Plans/NOTES-2026-05-25-F006-bare-metal-kernel-hhdm-coverage-short-of-module-placement.md` (`83e8112`) — defect statement, hypothesis, three preliminary fix alternatives.
- `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-production-smoke-BLOCKER.serial.log` (kept in the private repo) (`8aad054`) — full failed-boot serial transcript. Confirm symptom matches NOTES §1.
- `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-183600-12-serial-chain-baseline.serial.log` (if present in evidence dir, otherwise the Ubuntu dmesg transcript captured in the PROCEDURE doc Appendix A) — for the OptiPlex's actual e820 memmap shape.

**Live source to read (use RAG `:8404` for navigation if useful, then read the files directly):**

- `kernel/src/mm/*` — entire memory-management subsystem; in particular the HHDM-build logic. The kernel's serial output names the symbol as `mm:`-prefixed (`hhdm_offset=…`, `hhdm_phys_end=…`, `memmap_entries=70`). Find where these are computed.
- `kernel/src/main.rs` or whichever entry point loads `neo-init` — confirm exactly how the module's virtual address is computed (likely `hhdm_offset + module_phys_base`) and how it's read.
- Anywhere Limine's `MemmapResponse` / `ModuleResponse` / `ExecutableAddressResponse` is consumed.
- `kernel/src/arch/x86_64/paging.rs` (or equivalent) — page-table construction at HHDM-build time; relevant to alternative (C) which would extend the HHDM.

Use RAG semantic search (`:8404/search?q=...` with queries like "hhdm phys end", "memmap iteration", "module load", "limine memmap usable") to navigate.

## 3. Sub-questions to answer

- **Q1 — Hypothesis confirmation.** Does the kernel's HHDM-build logic iterate Limine's memmap by `LIMINE_MEMMAP_USABLE` (or similar `usable` filter) only? If so, that's the bug. If it iterates a broader set, the hypothesis needs refinement — what's the actual reason `hhdm_phys_end < neo-init phys`?
- **Q2 — Module placement reality.** Confirm Limine places modules in `KERNEL_AND_MODULES`-typed memmap entries (per the Limine spec), distinct from `USABLE`. Quote the Limine spec section if uncertain.
- **Q3 — Fix alternative evaluation.** For each of (A) extend HHDM-build to all-usable-types, (B) compute `hhdm_phys_end = max(memmap_end, exec_end, max(module_end))`, and (C) defensive full-phys-up-to-highest-memmap-entry:
  - Page-table cost (how many extra L2/L3/L4 entries; bytes consumed)
  - Code surface (lines changed; risk of touching adjacent invariants)
  - `unsafe` budget impact (Bible 4.2)
  - Interaction with ADR-016 (does the fix allocate page tables under the scheduler/mm lock?)
  - Bible 7.2 memory model compliance (kernel-half / user-half split preserved)
  - Whether the fix is testable in QEMU (i.e., can a synthetic memmap make QEMU exercise the bug path so a CI scenario can catch regressions?)
- **Q4 — Recommended alternative + reasoning.** Pick one. Alternatives-ruled-out section explicit. If a fourth alternative emerges from reading source, name it and include in the comparison. **Intermittence-aware:** per NOTES §11 (added 2026-05-25), F-006 is non-deterministic — Limine places the neo-init module at varying physical addresses across boots. Evaluate alternatives on their robustness to the FULL placement-variation surface, not just the captured failure address `0x47d4ba000`. Favor alternatives that handle worst-case placement structurally.
- **Q5 — CI-testable assertion design.** What in-kernel assertion would catch this class of bug at HHDM-build time? Likely shape: `debug_assert!(hhdm_phys_end >= max(module_phys_end))` (or `kassert!`-equivalent if the project has one). Where in the boot sequence does this go? Can a new harness scenario `bare_metal_hhdm_covers_modules` (or equivalently-named) be constructed that exercises the fix even on QEMU (e.g., synthetic memmap with a module placed near the top of a hole-punched range)? **Intermittence-aware:** because Limine placement is non-deterministic, a single successful boot does NOT prove the fix. Acceptance bar per NOTES §11 implications: ≥10 consecutive bare-metal boots all reaching scheduler-idle. Cerebro confirms or amends this acceptance bar in its recommendation.
- **Q6 — Scope-class flags.** Is the chosen fix scope-class (PL ratification needed) or operational (PM proceeds)? Default: operational unless the fix changes a Bible-named invariant or the ADR-016 substrate.

## 4. Discipline

- Cerebro recommends; PM refines; PL ratifies.
- Cerebro does NOT draft an ADR, amend a bible article, write a patch, or commit source code. Recommendations only.
- If the answer requires a Bible amendment or new ADR, flag in §9-equivalent for PM.
- Density per OBSERVE/THINK-substance precedent — committed choices, alternatives-ruled-out, no hedged option-spreads.
- Use RAG (`:8404`) before brute file enumeration.

## 5. Acceptance criteria

- **C1.** Output exists at `Plans/CEREBRO-OUTPUT-2026-05-25-F006-diagnosis.md`.
- **C2.** Hypothesis confirmation explicit: matches code OR refined-with-evidence.
- **C3.** All three (or four+) fix alternatives evaluated against the §3 Q3 axes.
- **C4.** A single recommended alternative with reasoning + alternatives-ruled-out + open-dependencies (if any).
- **C5.** CI-testable assertion design proposed (or honest framing that no in-kernel assertion is feasible, with reasoning).
- **C6.** Scope-class flagged.
- **C7.** Push to `origin/main` (CI `paths-ignore` skips Plans changes).
- **C8.** No final ratified calls; no audit-artifact amendments; no ADR drafting; no Bible amendments authored.

## 6. Out of scope

- Implementation. The fix code is Jarvis's lane after PM-DISPOSITION accepts Cerebro's recommendation (and PL ratifies if scope-class).
- Re-running bare-metal verification. That happens after the fix lands.
- F-006 close report. After re-verification passes.
- The PROCEDURE §3.3 baud-rate correction (115200 → 38400). Filing as a separate small PM-DISPOSITION patch alongside this Cerebro work; not Cerebro's concern.

## 7. Tell-Cerebro text

Cerebro — F-006 diagnosis dispatch. Bare-metal kernel HHDM coverage falls short of where Limine placed neo-init, causing PAGE_FAULT at ELF-parse time. **Intermittent across boots** per NOTES §11 (`e2dedf9`) — same ISO, same hardware, Limine's module-placement allocator picks varying physical addresses run-to-run; some land within HHDM coverage, some above. Symptom + hypothesis + intermittence finding in `Plans/NOTES-2026-05-25-F006-bare-metal-kernel-hhdm-coverage-short-of-module-placement.md` (`e2dedf9`, supersedes original `83e8112` filing); evidence in `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-production-smoke-BLOCKER.serial.log` (kept in the private repo). Read `kernel/src/mm/*` (use RAG `:8404` for navigation), confirm hypothesis against actual source, evaluate the three fix alternatives plus any fourth you identify against the FULL placement-variation surface (not just the captured `0x47d4ba000` address), recommend one. Per NOTES §11 implications, the intermittence framing favors alternatives B and C over A — confirm or amend. Acceptance criteria in dispatch §5 (this file at `Plans/DISPATCH-2026-05-25-F006-cerebro-diagnosis.md`). Discipline: recommend, don't decide; no patch code; no ADR draft. Time bound 90 minutes. Output `Plans/CEREBRO-OUTPUT-2026-05-25-F006-diagnosis.md`. Push to `origin/main`. Status 1 on push is cosmetic.

---

**End of dispatch.**
