# PM-DECISION: ADR-015 exclusive-end check correction (PATCH 1)

**Date:** 2026-05-07
**Decision-maker:** PM
**Caught by:** Cerebro, mid-verification at run 24 of the in-flight 50-run loop
**Type:** Bug fix in the ADR-015 implementation. Cerebro stopped before committing per the dispatch's "any OTHER classification → STOP and surface" rule. Right call — the bug is structurally adjacent to the ADR-015-target bug (one byte difference in the overflow check) and committing without fixing it would let an adjacent failure mode through.

---

## What happened

Cerebro ran the ADR-015 verification loop. Run 24 (`cap_grant_missing_meta`) produced an OTHER classification:

```
[ kernel ] heap: ready (4 MiB at VirtAddr(0xffffffffffc00000), 1024 pages mapped)
panicked at .../alloc/src/alloc.rs:573:9: memory allocation of 8 bytes failed
NEO_TEST/1 event=FAIL name=cap_grant_missing_meta reason=kernel_panic
```

He proposed: change the fit check from `HEAP_SIZE - 1` to `HEAP_SIZE`, add a synthetic exact-ceiling test for `aligned = 0xffff_ffff_ffc0_0000`. Stopped before committing per dispatch.

Cerebro is right.

## The math, verified

`try_above_kernel` does, simplified:

```rust
let unaligned = kernel_end.checked_add(HEAP_GUARD_SIZE)?;       // step 1
let aligned = align_up_checked(unaligned, HEAP_ALIGN)?;          // step 2
let _heap_end = aligned.checked_add(HEAP_SIZE as u64 - 1)?;     // step 3 (BUG)
Some(VirtAddr::new(aligned))
```

Step 3 verifies `aligned + (HEAP_SIZE - 1)` is representable — i.e., the *inclusive* last byte of the heap region fits in u64. But:

- `linked_list_allocator::Heap::init(start, size)` computes `end = start + size` internally for bounds-checking allocations.
- That computation is `aligned + HEAP_SIZE`, not `aligned + (HEAP_SIZE - 1)`.
- One byte difference.

With constants from `kernel/src/mm/heap.rs`:
- `HEAP_GUARD_SIZE = 0x20_0000` (2 MiB)
- `HEAP_ALIGN = 0x20_0000` (2 MiB)
- `HEAP_SIZE = 0x40_0000` (4 MiB)

Run 24's actual heap placement was `0xffff_ffff_ffc0_0000`. Working through both checks at that aligned value:

| check | computation | result | passes? |
|---|---|---|---|
| inclusive (`HEAP_SIZE - 1`, current) | `0xffff_ffff_ffc0_0000 + 0x3f_ffff` | `0xffff_ffff_ffff_ffff` | yes (BUG: should reject) |
| exclusive (`HEAP_SIZE`, fixed) | `0xffff_ffff_ffc0_0000 + 0x40_0000` | overflows u64 (`0x1_0000_0000_0000_0000`) | no (correct: rejects) |

Under the fix, `try_above_kernel` returns `None` for this aligned value, falls back to `try_below_kernel`, and the heap gets placed safely below the kernel image with abundant headroom from the canonical ceiling.

## Verification that the fix doesn't break benign cases

Three benign cases checked by hand:

| aligned | check (`+ HEAP_SIZE`) | result |
|---|---|---|
| `0xffff_ffff_ff80_0000` (the original wrong constant) | `0xffff_ffff_ffc0_0000` | fits, accepted ✓ |
| `0xffff_ffff_86c0_0000` (run 1 actual) | `0xffff_ffff_8700_0000` | fits, accepted ✓ |
| `0xffff_ffff_e300_0000` (run 11 actual) | `0xffff_ffff_e340_0000` | fits, accepted ✓ |

The fix is a strict tightening — it rejects exactly one more case (the exact-ceiling 0xffc0_0000 case) than the current check. No benign placement gets newly rejected.

## Verification that the existing synthetic Test 1 still triggers None

The synthetic Test 1's input `near_ceiling = 0xffff_ffff_ffc0_0000_u64` was chosen during R2 to overflow at `aligned + (HEAP_SIZE - 1) = 0xffff_ffff_ffe0_0000 + 0x3f_ffff`. Under the patched check (`+ HEAP_SIZE`), the chain still overflows — at `aligned + HEAP_SIZE`. Both inclusive and exclusive checks reject this input; the test still passes.

So Test 1's input value remains correct and the constant doesn't need to change. Test 1 is now exercising a slightly different point (overflow at exclusive-end, with even more headroom from the inclusive-end check that's no longer present). That's fine; the test's purpose is "this synthetic_kernel_end value triggers None," and that property holds under both checks.

## The new Test 4

The patch adds a fourth sub-test to `kaslr_heap_synthetic` for the exact-ceiling case:

```rust
// Test 4: exact-ceiling case where inclusive end fits but exclusive end doesn't.
let exact_ceiling = 0xffff_ffff_ffa0_0000_u64;
if try_above_kernel_synthetic(exact_ceiling).is_some() {
    fail_with_hex("reason=try_above_succeeded_at_exact_ceiling synthetic_end", exact_ceiling);
}
```

Why `0xffff_ffff_ffa0_0000`? Working backwards from "we want aligned = 0xffff_ffff_ffc0_0000 (the run-24 reproduction)":

- We need `aligned = 0xffff_ffff_ffc0_0000` after step 2 (align_up).
- Step 1 = `synthetic_kernel_end + GUARD = synthetic_kernel_end + 0x20_0000`.
- For step 2 to land at `0xffc0_0000`, step 1 must be in `(0xffa0_0000, 0xffc0_0000]` (since align_up to 2 MiB rounds up).
- So `synthetic_kernel_end ∈ (0xff80_0000, 0xffa0_0000]`. Pick `0xffa0_0000` (the upper bound, which produces step 1 = `0xffc0_0000` exactly, no rounding up needed).

Working forward:
- step 1: `0xffa0_0000 + 0x20_0000 = 0xffc0_0000` (no overflow).
- step 2 align_up: already aligned, stays `0xffc0_0000` (no overflow).
- step 3 (patched): `0xffc0_0000 + 0x40_0000` overflows u64. `try_above` returns `None`. ✓

Test 4 deterministically reproduces the run-24 bug class. If anyone in the future reverts the exclusive-end check (or introduces an equivalent off-by-one), Test 4 fails on every CI build. This is the regression guard.

## Why a patch dispatch instead of an in-place R3 revision

Two earlier in-place revisions (R1 post-INDEX-swap pre-flight, R2 synthetic constant) corrected the original dispatch *before* execution. A hypothetical R3 would correct it *during* execution — i.e., would also need to specify what to do with the uncommitted code that already implements the wrong text.

Mixing pre-execution textual fixes with mid-execution code patches in one revision narrative would be confusing. A separate patch dispatch (`Plans/DISPATCH-2026-05-07-adr-015-PATCH-1-exclusive-end-check-cerebro.md`) is structurally cleaner: the original dispatch records "what was originally specified," and the patch dispatch records "what got patched on top of that."

The git history will still show one ADR-015 commit on top of `afa4d22`. The two-document split is for project narrative clarity, not for git structure.

## Decision

Adopt Cerebro's proposed fix exactly. Three code changes:

1. `kernel/src/mm/heap.rs` `try_above_kernel`: `HEAP_SIZE as u64 - 1` → `HEAP_SIZE as u64`.
2. `kernel/src/mm/heap.rs` `try_above_kernel_synthetic`: same change (mirror).
3. `kernel/src/test_harness.rs` `run_kaslr_heap_synthetic`: add Test 4 with `exact_ceiling = 0xffff_ffff_ffa0_0000_u64`.

Plus required documentation:

4. Comment in `try_above_kernel` explaining why `HEAP_SIZE`, not `HEAP_SIZE - 1`.
5. Comment in Test 4 documenting that this is the run-24 regression guard.
6. Commit message body section explaining PATCH 1 (the rationale).

Plus verification:

7. Discard the pre-patch 25-run loop results. Restart 50-run loop with fresh log path prefix (`ADR-015-PATCH1-loop-run-N`). Old logs preserved on disk for wedge investigation.

## Why the ADR didn't catch this

The ratified ADR-015 (`Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md`) discusses Option E in terms of "validate + below-kernel fallback" but doesn't specify whether the validation is on inclusive or exclusive end. The implementation was free to interpret. I (PM) drafted the dispatch with `HEAP_SIZE - 1` because I was thinking "verify the last byte fits" — a natural framing if you don't have the allocator's internal bounds-machinery in mind.

The fix to the *ADR text* is a one-line clarification, but the ADR is already ratified and committed at `docs/ADR-015-kaslr-tolerant-heap-placement.md` (in Cerebro's working tree, awaiting commit). The right path is:

- Land the patched implementation as planned (this dispatch).
- In the post-ADR-015 planning conversation tomorrow, draft a small ADR-015 amendment clarifying exclusive-end semantics. The amendment is a doc-only change to `docs/`; it lands as a separate small commit after ADR-015 itself is committed.

The ADR amendment is NOT this dispatch's scope. This dispatch lands the implementation; the amendment is a follow-up.

## What it costs to NOT fix this

If we shipped without PATCH 1:

- The original "0/50 panics" claim is technically true (no addr.rs:390 panics observed).
- But run 24 already produced an alloc.rs:573 OTHER. The OTHER classification is structurally the same failure mode (heap-placement overflow under KASLR), just triggered one byte differently.
- Sub-commit 3a's verification gate would inherit a ~2-4% per-boot OTHER rate from this exact-ceiling case (the bug triggers when `aligned == 0xffff_ffff_ffc0_0000`, which happens on roughly that fraction of KASLR rolls).
- Anyone running the kernel would discover the failure within ~25 boots.

That's not acceptable. The whole point of ADR-015 is "no KASLR-induced heap failures." Letting an adjacent failure class through would be technically defensible against a narrow reading of the spec but would fail the spirit of the work.

## Lesson for the post-mortem

This is the third correction on ADR-015 work today:

- **R1** (pre-execution): pre-flight expected-log text was stale because Jarvis's INDEX swap landed between dispatch authorship and execution.
- **R2** (pre-execution): synthetic test constant was miscalibrated because PM didn't work the checked-arithmetic chain explicitly.
- **PATCH 1** (mid-execution): production check used inclusive-end semantics where exclusive-end was needed.

R1 was caused by parallel-work-stream timing (not preventable by better review of this dispatch alone). R2 was caused by mental-math error during dispatch authorship (preventable by working the chain explicitly). PATCH 1 was caused by ADR ambiguity (preventable by ADR being more specific about exclusive-end semantics).

The pattern across R2 and PATCH 1 is: **arithmetic-heavy specifications need worked examples in the spec itself.** The ADR should have included a worked numeric example showing what the check does at the canonical-ceiling boundary. The dispatch should have included worked examples for every synthetic constant. Both are PM authorship failures, not Cerebro execution failures.

This goes in the post-mortem when ADR-015 closes. The post-mortem will recommend: ADR text for arithmetic-bearing specs must include at least one worked numeric example at each boundary the spec touches.

## Cross-references

- `Plans/DISPATCH-2026-05-07-adr-015-implementation-cerebro.md` — original dispatch (with R1 and R2 in place).
- `Plans/DISPATCH-2026-05-07-adr-015-PATCH-1-exclusive-end-check-cerebro.md` — this patch dispatch.
- `Plans/PM-DECISION-2026-05-07-adr-015-ratification.md` — five ratification decisions; Decision 1 (Option E adoption) and Decision 3 (synthetic test) are both touched by this patch.
- `Plans/PM-DECISION-2026-05-07-adr-015-synthetic-constant-correction.md` — R2 record.
- `Plans/PM-NOTE-2026-05-07-adr-015-dispatch-preflight-revision.md` — R1 record.
- `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md` — ratified ADR; will receive a small clarifying amendment in a follow-up commit after ADR-015 itself lands.
- `target/verify/ADR-015-loop-run-24.cap_grant_missing_meta.serial.log` — the failing run, preserved on disk for the project record.

---

**Decision recorded.** PATCH 1 dispatched; Cerebro can resume from §2 pre-flight on the patch dispatch. Post-patch verification produces the truthful "0/50 panics, 0/50 alloc.rs:573 OTHERs" claim that the ADR-015 commit needs to make.
