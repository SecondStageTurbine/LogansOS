# PM-DECISION: ADR-015 dispatch synthetic test constant correction

**Date:** 2026-05-07
**Decision-maker:** PM
**Caught by:** Cerebro, pre-flight on the ADR-015 implementation dispatch
**Type:** Bug fix in a dispatch I (PM) wrote. Cerebro stopped before any file change per the dispatch's "do not change synthetic constants without surfacing" rule. Right call — the bug would have caused the synthetic test to fail on its first run, every run.

---

## What happened

After the R1 in-place revision (pre-flight expected-log update), Cerebro proceeded to read the dispatch's §3.3 Scenario 2 implementation block and surfaced before editing any source file:

> The dispatch's synthetic test constant `near_ceiling = 0xffff_ffff_ff80_0000` is described as "~8 MiB below ceiling" and expected to make `try_above_kernel_synthetic()` return `None`, but the arithmetic does not overflow with a 2 MiB guard plus 4 MiB heap. That constant still leaves about 8 MiB of address space; the checked range only needs about 6 MiB. A value within roughly 6 MiB of `u64::MAX`, e.g. `0xffff_ffff_ffc0_0000`, would exercise the intended overflow path.

Cerebro is right.

## The math, verified

The `try_above_kernel_synthetic` checked-arithmetic chain is:

1. `unaligned = synthetic_kernel_end.checked_add(HEAP_GUARD_SIZE)?`
2. `aligned = align_up_checked(unaligned, HEAP_ALIGN)?` — which internally is `value.checked_add(align - 1)?` then `& !(align - 1)`
3. `_heap_end = aligned.checked_add(HEAP_SIZE - 1)?`

With constants from `kernel/src/mm/heap.rs`:
- `HEAP_GUARD_SIZE = 2 MiB = 0x20_0000`
- `HEAP_ALIGN = 2 MiB = 0x20_0000`
- `HEAP_SIZE = 4 MiB = 0x40_0000`, so `HEAP_SIZE - 1 = 0x3f_ffff`

### Original constant `0xffff_ffff_ff80_0000` — does NOT overflow

| step | computation | result | overflow? |
|---|---|---|---|
| 1 | `0xffff_ffff_ff80_0000 + 0x20_0000` | `0xffff_ffff_ffa0_0000` | no |
| 2a | `0xffff_ffff_ffa0_0000 + 0x1f_ffff` | `0xffff_ffff_ffbf_ffff` | no |
| 2b | `& !0x1f_ffff` | `0xffff_ffff_ffa0_0000` | no |
| 3 | `0xffff_ffff_ffa0_0000 + 0x3f_ffff` | `0xffff_ffff_ffdf_ffff` | no (2 MiB headroom remaining) |

`try_above_kernel_synthetic(0xffff_ffff_ff80_0000)` returns `Some(VirtAddr::new(0xffff_ffff_ffa0_0000))`. The test's `if try_above_kernel_synthetic(near_ceiling).is_some() { fail... }` would fail every run.

### Cerebro's proposed constant `0xffff_ffff_ffc0_0000` — DOES overflow

| step | computation | result | overflow? |
|---|---|---|---|
| 1 | `0xffff_ffff_ffc0_0000 + 0x20_0000` | `0xffff_ffff_ffe0_0000` | no |
| 2a | `0xffff_ffff_ffe0_0000 + 0x1f_ffff` | `0xffff_ffff_ffff_ffff` | no (exactly `u64::MAX`) |
| 2b | `& !0x1f_ffff` | `0xffff_ffff_ffe0_0000` | no |
| 3 | `0xffff_ffff_ffe0_0000 + 0x3f_ffff` | `0x1_0000_0000_001f_ffff` (truncates) | **yes — checked_add returns None** |

`try_above_kernel_synthetic(0xffff_ffff_ffc0_0000)` returns `None`. The test's "expected None" assertion now passes.

## Decision

Adopt Cerebro's proposed constant `0xffff_ffff_ffc0_0000` exactly. Update the dispatch's §3.3 Scenario 2 Test 1 in place. Update the comment from "~8 MiB below ceiling" to "4 MiB below `u64::MAX + 1`" so the arithmetic intent is documented in the source.

The derived `synthetic_kernel_start = near_ceiling - 9 * 1024 * 1024` shifts to `0xffff_ffff_ff70_0000` with the new constant. That is still well above the synthetic HHDM end (`0xffff_8000_0000_0000 + 16 GiB = 0xffff_8004_0000_0000`), so `try_below_kernel_synthetic` still returns `Some` for Test 1's second branch. No further correction needed.

Test 2 and Test 3 of Scenario 2 are unchanged — they don't reference `near_ceiling`.

## Why this happened

I wrote the synthetic test constants in the dispatch by reasoning about "what value is near the ceiling" without working through the full checked-arithmetic chain. The miscalibration is by 2 MiB (the chain needs `GUARD + HEAP_SIZE = 6 MiB` of headroom-plus-1 to overflow; I left 8 MiB). It's a mental-math error, not a design error.

The dispatch's §3.3 explicit instruction "do not change synthetic constants without surfacing first" was written precisely because this kind of miscalibration is easy to make and hard to catch in code review. Cerebro followed the instruction; the instruction did its job.

## What it costs to NOT fix this

If we shipped the dispatch as-written:

1. Cerebro implements §3 verbatim (synthetic test included with `0xffff_ffff_ff80_0000`).
2. Build passes.
3. §5 single-run of `kaslr_heap_synthetic` fails on Test 1 with `reason=try_above_succeeded_near_ceiling synthetic_end=0xffff_ffff_ff80_0000`.
4. Cerebro surfaces the failure, I diagnose it as a mental-math error in the dispatch, we revise the constant, Cerebro rebuilds and re-runs.

Cost: one rebuild cycle (~5-10 min) plus diagnostic time. Catching it pre-flight saves that round-trip.

## Lesson, mild

When writing synthetic test constants in a dispatch, I should compute the chain explicitly and put the computation in the comment. The dispatch now does this for the new constant ("`aligned + (HEAP_SIZE - 1) = 0xffff_ffff_ffe0_0000 + 0x3f_ffff` overflows u64"). Future synthetic constants in future dispatches should follow the same pattern.

This is also the second consecutive in-place revision to this dispatch (R1 was the post-INDEX-swap pre-flight text, R2 is this constant fix). Two revisions in 30 minutes is a sign that the original dispatch needed more careful review before going out. Worth noting in the post-mortem when ADR-015 closes.

## Cross-references

- `Plans/DISPATCH-2026-05-07-adr-015-implementation-cerebro.md` — revised dispatch (R2 in place).
- `Plans/PM-NOTE-2026-05-07-adr-015-dispatch-preflight-revision.md` — R1 (the pre-flight expected-log fix).
- `Plans/PM-DECISION-2026-05-07-adr-015-ratification.md` — the five ratification decisions; Decision 3 mandates the synthetic test, which made this constant load-bearing.
- `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md` — the ratified ADR; doesn't change as a result of this correction (the correction is in the dispatch's test scaffolding, not in ADR-015's design).

---

**Decision recorded.** Constant `0xffff_ffff_ffc0_0000` adopted; dispatch revised; Cerebro can resume from §2 pre-flight on the revised dispatch.
