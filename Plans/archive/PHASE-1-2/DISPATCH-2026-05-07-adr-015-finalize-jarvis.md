# Dispatch: Finalize ADR-015 text from calibrated draft + ratification decisions

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-07
**Type:** Doc-only finalization. NOT a commit. Output is a `Plans/` artifact (the polished ADR text) ready to be committed to `docs/ADR-015-kaslr-tolerant-heap-placement.md` as part of a future implementation dispatch.
**Substrate:** `Plans/ADR-015-DRAFT-2026-05-07-kaslr-tolerant-heap-placement.md` is the calibrated draft; the five ratification decisions below are PM's authoritative resolutions of the open review points. Sub-commit 2 working tree is currently applied (clean state per Cerebro's KASLR-Q1B restore); do NOT touch tracked files.

---

## 0. Standing instructions

1. **Do NOT commit anything.** Output is a single `Plans/` file.
2. **Do NOT modify any tracked file** under `kernel/`, `boot/`, `userland/`, `docs/`, `tools/`, `tests/`, or anywhere else outside `Plans/`.
3. **Do NOT touch any stash.** Both stashes (`stash@{0}: kaslr-q1-higher-n-pre-diagnostic`, `stash@{1}: sub-commit-2-close-attempt-blocked-on-kaslr`) stay exactly as they are.
4. **Do NOT run any build/test command.** Cerebro has the docker box queued for sub-commit 2 closure when he's rested.
5. Output: `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md`. This is the polished ADR text, ready to be committed to `docs/ADR-015-kaslr-tolerant-heap-placement.md` later by the implementation dispatch.

---

## 1. The task

Take the calibrated draft (`Plans/ADR-015-DRAFT-2026-05-07-kaslr-tolerant-heap-placement.md`) and produce the polished, ratification-final ADR text. Apply the five ratification decisions in §3 below. Strip "draft" framing. Add the standard ADR header that matches the format of `docs/ADR-014-kernel-observability.md`.

The output is the file that will be committed to `docs/` later. It is in `Plans/` only because we don't commit tracked files while sub-commit 2 is uncommitted.

---

## 2. Reference shape — match `docs/ADR-014-kernel-observability.md`

Read `docs/ADR-014-kernel-observability.md` for the standard ADR header format. Specifically:

- Title: `# ADR-015: KASLR-tolerant kernel heap placement` (no "DRAFT" suffix)
- Header table with: Status, Date, Authority, Supersedes, Superseded by, Related
- Status: `Accepted (ratified 2026-05-07)`
- Authority: `Project Lead (review delegated to PM as subject-matter expert)`

The body sections preserve the draft's structure (Context, Constraints, Options, Decision, Implementation sketch, Test additions, Anti-scope, Cross-references, Notes) but with the five ratification decisions woven in.

---

## 3. The five ratification decisions

These are PM's authoritative resolutions of the draft's five open review points. Apply them in the order below; each maps to a specific section of the ADR.

### Decision 1 — Option E (validate + below-kernel fallback)

**Recommended in draft; ratified.**

Rationale to weave into §Decision (replacing or augmenting the existing rationale bullets):

- Option E's fallback is *exercised regularly, not rarely*. At calibrated 8% rolls hitting the danger zone, the fallback path runs in roughly 1-in-12 boots. A regularly-exercised path is more likely to be correct than a rarely-exercised one — code that runs gets tested in production by definition. The original draft framing of "fallback adds a second strategy" was the weak version; the correct framing is "fallback is a routine code path that runs whenever the primary fails, and the primary fails routinely."

- Option E **incidentally addresses the alloc.rs:573 sibling failure**. The sibling failure (KASLR-Q1B run 32) occurred when heap placement *succeeded* but the allocator's internal arithmetic on a near-ceiling heap then failed. Option E's `try_below_kernel` fallback places the heap in the below-kernel range (far from canonical ceiling) whenever the above-kernel placement would be near the ceiling. That moves the heap out of the alloc.rs:573 danger zone too. Option D leaves the heap in the canonical-ceiling neighborhood whenever it doesn't outright overflow, so D doesn't address the alloc.rs:573 case at all. **This is the deciding point.**

- Option D was a defensible stopgap when the panic rate was guessed at 0.5%. At calibrated 8%, with a known sibling failure that D doesn't address, D is no longer adequate.

### Decision 2 — Memmap walk for HHDM end, not a constant

**Decisive, not "not load-bearing" as the draft hedged.**

Rationale to weave into §Implementation sketch (replacing the `MAX_PHYSICAL_RAM_ASSUMPTION` paragraph):

- The constant approach has a real failure mode: a system with more physical RAM than the hardcoded constant could place HHDM mappings beyond `hhdm_offset + MAX_PHYSICAL_RAM_ASSUMPTION`, and `try_below_kernel`'s lower bound check would let the heap collide with HHDM. Silent corruption — strictly worse than the bug we're fixing.

- The memmap walk pulls the actual largest physical address from the Limine memmap. Bounded cost — memmap entries are O(tens). One-time at heap_start() invocation, or once at `mm::init` time and stashed. **There is no defensible reason to prefer the constant.**

Implementation note (add to §Implementation sketch): the cleanest shape is to stash the value at `mm::init` time alongside `HHDM_OFFSET`. Add a new `static HHDM_PHYS_END: Once<u64>` to `kernel/src/mm/mod.rs`, set during `mm::init` from the memmap walk:

```rust
let max_phys_end: u64 = memmap
    .iter()
    .filter(|e| matches!(e.type_, limine::memmap::MEMMAP_USABLE))
    .map(|e| e.base + e.length)
    .max()
    .unwrap_or(0);
HHDM_PHYS_END.call_once(|| max_phys_end);
```

`heap_start()`'s `try_below_kernel` reads `HHDM_PHYS_END` for the lower-bound check.

### Decision 3 — Both test approaches: contract smoke + helper unit tests

**Both, not either/or.** Replace §Test additions entirely with:

#### Test 1 — `kaslr_heap_contract` (default suite, runs every boot)

A new test_harness scenario that runs every boot in the default suite. Captures:
1. `heap_start()`'s actual return value.
2. `__kernel_image_end` and `__kernel_image_start`.
3. The HHDM offset and HHDM phys-end.

Verifies:
- The heap region (`heap_start..heap_start + HEAP_SIZE`) does not overlap with the kernel image, HHDM region, or extend past the canonical-space ceiling at `0xffffffffffffffff`.
- The heap region is either above the kernel image with a 2 MiB guard (preferred placement) OR below the kernel image with a 2 MiB guard (fallback placement).
- `heap_start` is `HEAP_ALIGN`-aligned.

The contract test surfaces regressions automatically. Cost: ~10 lines of test code.

#### Test 2 — `kaslr_heap_synthetic` (test-harness only, deterministic branch coverage)

A new test_harness scenario behind `#[cfg(test_harness)]` that exercises both `try_above_kernel` and `try_below_kernel` deterministically by passing synthetic `kernel_end` values:

- Synthetic `kernel_end` within ~10 MiB of `0xffffffffffffffff`: verifies `try_above_kernel` returns `None` (overflow path) and `try_below_kernel` returns `Some(_)` at a sane address.
- Synthetic `kernel_end` near `0xffff_8000_0040_0000` (just above canonical-half boundary): verifies `try_below_kernel` returns `None` (no room below) and that the panic message in the both-fail case is clear.
- Synthetic `kernel_end` in middle range: verifies `try_above_kernel` returns `Some(_)` at the expected address.

The synthetic test requires `try_above_kernel` and `try_below_kernel` to be `pub(super)` or `pub(crate)` so the test_harness module can call them. This is a standard pattern; production code outside `mm/heap.rs` doesn't see them.

Cost: helper-exposure refactor (~5 lines) + ~30 lines of test code.

Both tests are required for ratification; the contract test is the runtime gate, the synthetic test is the deterministic-branch-coverage defense-in-depth.

### Decision 4 — Sibling allocator failure: incidentally addressed

**Reframed.** Replace the §Anti-scope bullet on the alloc.rs:573 audit with:

- **Does not exhaustively audit overflow in other `mm` arithmetic.** Option E **incidentally addresses the most common case** of the alloc.rs:573 sibling failure observed in KASLR-Q1B run 32, because the fallback heap is placed far from the canonical ceiling whenever the above-kernel placement would be near it. A separate audit pass for `paging.rs`, `frame.rs`, `slab.rs`, `pmap.rs`, and the `linked_list_allocator` integration is desirable for full canonical-ceiling robustness, but the urgent case is closed by Option E. The broader audit is tracked as a separate work item, not a blocker for ADR-015.

Also weave a sentence into §Decision noting the incidental coverage as part of Option E's rationale (it's not just about heap_start; E fixes the immediate sibling case too).

### Decision 5 — Schedule: between SC2 commit and SC3a checkpoint

**Confirmed.** Replace §Notes' urgency bullet with:

- **Schedule:** ratification + implementation lands between sub-commit 2 commit and sub-commit 3a's pre-implementation checkpoint conversation. Sequence:
  1. Sub-commit 2 commits to `main` (per `Plans/DISPATCH-2026-05-07-sub-commit-2-close-cerebro.md`).
  2. ADR-015 implementation dispatched to Cerebro as a small focused commit on `main`. Includes the contract smoke test (default suite) and synthetic helper tests. Verification: 50-run KASLR-Q1B-style loop on the new tree shows 0/50 panics.
  3. ADR-015 finalized text commits to `docs/ADR-015-kaslr-tolerant-heap-placement.md` as part of step 2's commit.
  4. Sub-commit 3a pre-implementation checkpoint conversation. Uses `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` as substrate. KASLR is no longer a noise source.

The schedule pre-empts the failure mode where 3a verification produces flaky results because of KASLR-induced false failures interleaved with real 3a bugs. At 8% per-boot panic rate, 3a's verification gates would have meaningful false-failure probability without the fix. Doing ADR-015 first is honest engineering, not delay.

---

## 4. Other edits to apply (mechanical)

### 4.1 Strip "draft" language

- Title: `# ADR-015: KASLR-tolerant kernel heap placement` (remove "(DRAFT)").
- Status row: `Accepted (ratified 2026-05-07)`.
- Remove the "Draft status note" blockquote at the top.
- Remove "(DRAFT)" markings throughout.
- Final line: `**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR.` (matching ADR-014's closing).
- Remove the `*Draft status: ready for project-lead ratification review...*` italic line at the very end.

### 4.2 Header table

Match ADR-014's format exactly:

```markdown
| | |
|---|---|
| **Status** | Accepted (ratified 2026-05-07) |
| **Date** | 2026-05-07 |
| **Authority** | Project Lead (review delegated to PM as subject-matter expert) |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | [keep the existing related list, but reword "draft" references — see 4.3] |
```

### 4.3 Update the Related row

Current Related row references "the diagnostic chain that identified the bug" and `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` as "the higher-N evidence cited throughout." Keep these. Also add:

- `Plans/PM-DECISION-2026-05-07-adr-015-ratification.md` (forthcoming PM ratification record — PM will commit this separately; reference it for project history).

### 4.4 Clean up forward-looking language

The draft contains language like "the higher-N Q1B diagnostic running concurrent with this ADR draft will tighten the rate estimate." That was true at draft time; it's not now. Strip such phrases. Replace with past-tense statements of what the diagnostic produced.

### 4.5 Cross-check the Implementation sketch

The draft's implementation sketch references `MAX_PHYSICAL_RAM_ASSUMPTION`. Replace that section with the memmap-walk approach from Decision 2. The sketch should now reference `mm::HHDM_PHYS_END` (the new `Once<u64>`) instead of a constant.

### 4.6 Linker symbol export

The draft's implementation sketch declares `static __kernel_image_start: u8` as a new symbol. Verify this symbol exists in the current `kernel/linker.ld` — if not, the implementation will need to add it. The linker script already defines `__kernel_image_start = .;` at the top, so the symbol exists; just confirm the `extern "C"` declaration is correct.

(You don't need to verify by reading the linker.ld for this dispatch; just trust PM's claim. The implementation dispatch will verify and adjust if needed.)

---

## 5. Output structure

`Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md`:

```
# ADR-015: KASLR-tolerant kernel heap placement

[header table per §4.2]

---

## Context

[copy from draft, with calibrated KASLR-Q1B evidence woven in as in current draft;
strip "draft" forward-looking language per §4.4]

## Constraints

[copy from draft]

## Options

[copy from draft, all six options A-F]

## Decision

[draft's decision section, augmented with Decision 1's rationale per §3,
including the alloc.rs:573 incidental-coverage point]

## Implementation sketch (informational, not normative)

[draft's sketch, with MAX_PHYSICAL_RAM_ASSUMPTION replaced by HHDM_PHYS_END
memmap-walk approach per Decision 2]

## Test additions

[replaced entirely per Decision 3 — both contract smoke test and synthetic
helper tests]

## Anti-scope

[draft's anti-scope, with the alloc.rs:573 bullet reframed per Decision 4]

## Cross-references

[copy from draft, plus add the PM-DECISION-2026-05-07-adr-015-ratification.md reference]

## Notes

[draft's notes, with urgency bullet replaced per Decision 5; remove
"pending project-lead review" language since it's now ratified]

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR.
```

Total target length: 350-450 lines (the draft is ~280; the additions and replacements grow it modestly).

---

## 6. What you do NOT do

- Do not change the option list (A through F). The five options stay; only the rationale around Option E is sharpened.
- Do not delete §Options. Future readers benefit from seeing the rejected alternatives.
- Do not invent new technical content. The five decisions are PM's authoritative resolutions; everything else stays close to the draft.
- Do not modify the calibrated evidence in §Context (KASLR-Q1B numbers, virt_base data points, binary-size delta). Those are facts.
- Do not add a new "Decision rationale" section. The rationale lives within §Decision; don't bifurcate.

---

## 7. Time budget

- 5 min: read this dispatch and the draft to load context.
- 10 min: read `docs/ADR-014-kernel-observability.md` for the header format.
- 30 min: write the new §Test additions section per Decision 3.
- 15 min: rewrite §Decision per Decision 1 (Option E rationale + alloc.rs:573 incidental coverage).
- 15 min: rewrite §Implementation sketch's HHDM section per Decision 2.
- 10 min: §Anti-scope reframe per Decision 4 + §Notes urgency bullet per Decision 5.
- 10 min: header table, status, supersedes language, draft-stripping per §4.
- 15 min: full read-through for flow, citation discipline (one quote per source max if any quotes survive), and tone.

**Total: ~1.5h.** If you hit 2h and aren't done, surface — PM picks up the rest.

---

## 8. Close

When done, ping PM with: "ADR-015 finalized: `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md`, <line count> lines, any notable deviations from the five decisions or the §4 mechanical edits."

PM does a final review pass before queueing the C dispatch (INDEX.md sweep pre-draft).

---

## 9. Why this matters

ADR-015's finalized text is the artifact that commits to `docs/` as part of the implementation dispatch. Producing it now while context is fresh saves a future-PM from re-deriving the polish from a draft + a half-written ratification note. The five decisions are PM's authoritative resolutions and shouldn't change between now and the implementation commit; locking them into the ADR text now is correct.

---

**End of dispatch.**
