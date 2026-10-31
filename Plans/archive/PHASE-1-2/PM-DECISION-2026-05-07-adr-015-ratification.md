# PM ratification decision: ADR-015 (KASLR-tolerant kernel heap placement)

**Date:** 2026-05-07
**Author:** PM
**Status:** Ratified per Bible Article 27 authority. Project lead delegated review to PM as subject-matter expert; PM reviewed and ratified. ADR-015 final text at `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md` is the ratified text, ready to commit to `docs/ADR-015-kaslr-tolerant-heap-placement.md` as part of the implementation dispatch.
**Substrate:** `Plans/ADR-015-DRAFT-2026-05-07-kaslr-tolerant-heap-placement.md` (calibrated draft) · `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` (calibrating evidence) · `Plans/PM-DECISION-2026-05-07-kaslr-q1-higher-n.md` (pre-existence decision)

---

## What was reviewed

ADR-015 draft as of 2026-05-07 14:02, calibrated against KASLR-Q1B's 4/50 panic rate, five virt_base data points, 712-byte binary-size delta, and the alloc.rs:573 sibling failure mode.

The five review points the draft flagged for project-lead input:

1. Option E vs Option D
2. MAX_PHYSICAL_RAM_ASSUMPTION vs memmap walk
3. Test scenario shape (refactor-for-testability vs contract-only)
4. Sibling allocator failure scope
5. Urgency / scheduling

## The five decisions

### Decision 1 — Option E (validate + below-kernel fallback)

**Recommended in draft; ratified.** Two observations sharpened the case beyond the draft's original framing:

- Option E's fallback is *exercised regularly, not rarely*. At calibrated 8% of rolls hitting the danger zone, the fallback path runs in roughly 1-in-12 boots. A regularly-exercised path is more likely to be correct than a rarely-exercised one — code that runs gets tested in production by definition. The original draft framing of "fallback adds a second strategy" was the weak version; the correct framing is "fallback is a routine code path that runs whenever the primary fails, and the primary fails routinely."

- Option E **incidentally addresses the alloc.rs:573 sibling failure**. The sibling failure (KASLR-Q1B run 32) occurred when heap placement *succeeded* but the allocator's internal arithmetic on a near-ceiling heap then failed. Option E's `try_below_kernel` fallback places the heap in the below-kernel range (far from canonical ceiling) whenever the above-kernel placement would be near the ceiling. That moves the heap out of the alloc.rs:573 danger zone too. Option D leaves the heap in the canonical-ceiling neighborhood whenever it doesn't outright overflow, so D doesn't address the alloc.rs:573 case at all. **This is the deciding point.**

Option D was a defensible stopgap when the panic rate was guessed at 0.5%. At calibrated 8%, with a known sibling failure that D doesn't address, D is no longer adequate.

### Decision 2 — Memmap walk for HHDM end, not a constant

**Decisive, not "not load-bearing" as the draft hedged.**

The constant approach has a real failure mode: a system with more physical RAM than the hardcoded constant could place HHDM mappings beyond `hhdm_offset + MAX_PHYSICAL_RAM_ASSUMPTION`, and `try_below_kernel`'s lower bound check would let the heap collide with HHDM. Silent corruption — strictly worse than the bug we're fixing.

The memmap walk pulls the actual largest physical address from the Limine memmap. Bounded cost — memmap entries are O(tens). One-time at `mm::init`. Stash the result in `mm::HHDM_PHYS_END: AtomicU64` (or `Once<u64>`; both work). `heap_start()`'s `try_below_kernel` reads the value for the lower-bound check. **There is no defensible reason to prefer the constant.**

### Decision 3 — Both test approaches: contract smoke + helper unit tests

**Both, not either/or.**

Contract smoke test (runs every boot): `kaslr_heap_contract` captures the actual heap placement, verifies non-overlap with kernel/HHDM/canonical-ceiling, verifies alignment. Surfaces regressions automatically.

Helper unit tests (deterministic branch coverage, behind `#[cfg(test_harness)]`): `kaslr_heap_synthetic` calls `try_above_kernel` and `try_below_kernel` with synthetic `kernel_end` values to exercise both branches deterministically.

The trade-off the draft flagged ("internal helpers stay private vs refactor for testability") is resolved by `pub(super)` or `pub(crate)` exposure: production code outside `mm/heap.rs` doesn't see the helpers; the test_harness module does. Standard pattern.

Both tests are required for ratification: contract is the runtime gate, synthetic is deterministic-branch-coverage defense-in-depth.

### Decision 4 — Sibling allocator failure: incidentally addressed

**Reframed.** The draft tracked the alloc.rs:573 failure as "Q4 in anti-scope" — separate work item. After Decision 1's analysis, the framing changes: Option E **incidentally addresses the most common case** of the alloc.rs:573 failure, because the fallback heap is far from canonical ceiling. The full mm/ canonical-ceiling audit remains a separate work item, but the urgent case is closed by Option E itself.

The ADR text reflects this: §Anti-scope keeps the broader audit as a separate work item, but §Decision and §Notes name Option E's incidental coverage as part of the rationale.

### Decision 5 — Schedule: between sub-commit 2 commit and sub-commit 3a checkpoint

**Confirmed.** Sequence:

1. Sub-commit 2 commits to `main` (per `Plans/DISPATCH-2026-05-07-sub-commit-2-close-cerebro.md`).
2. ADR-015 implementation dispatched to Cerebro as a small focused commit on `main`. Includes the contract smoke test (default suite) and synthetic helper tests. Verification: 50-run KASLR-Q1B-style loop on the new tree shows 0/50 panics (the upper bound on residual rate is then ~6% at 95% CI; combined with deterministic helper tests, that's strong evidence the bug class is eliminated).
3. ADR-015 finalized text commits to `docs/ADR-015-kaslr-tolerant-heap-placement.md` as part of step 2's commit.
4. Sub-commit 3a pre-implementation checkpoint conversation. Uses `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` as substrate. KASLR is no longer a noise source for 3a's verification.

The schedule pre-empts the failure mode where 3a verification produces flaky results because of KASLR-induced false failures interleaved with real 3a bugs. At 8% per-boot panic rate, 3a's verification gates would have meaningful false-failure probability without the fix. Doing ADR-015 first is honest engineering, not delay.

## Why PM has authority to ratify

Bible Article 27 specifies amendments are approved by the project lead. ADRs (per Bible Article 18.1) are mandatory for choices that "future-us might second-guess" but don't carry the same ratification ceremony as Bible amendments — they go through PR + rationale + project-lead approval.

Project lead explicitly delegated this review to PM as subject-matter expert: "you go ahead and do the ratification review. Remember you are the expert I am just project manager, I just make sure work is getting done." Recorded for project history.

Reversibility: ADRs are reversible per Bible Article 24.2. If ADR-015 turns out wrong (e.g., Option E's `try_below_kernel` exhibits an unanticipated failure mode that Option D would have avoided), it can be superseded by a follow-on ADR.

## Cross-references

- `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md` — the ratified ADR text, ready for commit to `docs/`.
- `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` — the calibrating evidence.
- `Plans/PM-DECISION-2026-05-07-kaslr-q1-higher-n.md` — the higher-N Q1B pre-committed decision tree that this ratification follows from.
- Bible Articles 12.3 (KASLR mandate), 18.1 (ADR procedure), 24.2 (reversibility), 27 (amendment authority).

---

**Decision recorded.** ADR-015 is ratified. Implementation dispatch follows once sub-commit 2 closes per Decision 5's schedule.
