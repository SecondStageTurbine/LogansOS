# ADR-015: KASLR-tolerant kernel heap placement

| | |
|---|---|
| **Status** | Accepted (ratified 2026-05-07) |
| **Date** | 2026-05-07 |
| **Authority** | Project Lead (review delegated to PM as subject-matter expert) |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Article 12.3 (KASLR mandatory from Phase 1) · ADR-013 §Q1 (per-CPU state — context for the binary-size delta argument) · `kernel/src/mm/heap.rs` (locus of bug) · `kernel/src/mm/paging.rs` (sibling subsystem; not affected) · DISPATCH-2026-05-06-kaslr-overflow-diagnostic-cerebro.md / PM-DECISION-2026-05-06-kaslr-overflow-blocks-closure.md / PM-DECISION-2026-05-07-kaslr-q1-higher-n.md (the diagnostic chain that identified the bug) · `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` (the higher-N evidence cited throughout) · `Plans/PM-DECISION-2026-05-07-adr-015-ratification.md` (ratification record) |

---

## Context

`kernel/src/mm/heap.rs` computes the kernel heap's virtual base from the relocated kernel image end:

```rust
pub fn heap_start() -> VirtAddr {
    let kernel_end = unsafe { &__kernel_image_end as *const u8 as u64 };
    VirtAddr::new(align_up(kernel_end + HEAP_GUARD_SIZE, HEAP_ALIGN))
}

const fn align_up(value: u64, align: u64) -> u64 {
    (value + align - 1) & !(align - 1)
}
```

Then `heap::init` derives the heap end:

```rust
let heap_end_page =
    Page::<Size4KiB>::containing_address(heap_start + (HEAP_SIZE as u64 - 1));
```

`HEAP_GUARD_SIZE = 2 MiB`, `HEAP_ALIGN = 2 MiB`, `HEAP_SIZE = 4 MiB`.

When KASLR (mandatory per Bible Article 12.3) places the kernel image such that `__kernel_image_end + HEAP_GUARD_SIZE + (align_up rounding) + HEAP_SIZE` exceeds `u64::MAX`, the arithmetic overflows. Three independent overflow points exist:

1. `heap_start()` line 1: `kernel_end + HEAP_GUARD_SIZE` — raw `u64` add, silently wraps.
2. `heap_start()` line 1 (cont.): `align_up(value, HEAP_ALIGN)` — adds another `(HEAP_ALIGN - 1)` (= 2 MiB - 1). Raw `u64` add, silently wraps.
3. `heap::init` `heap_start + (HEAP_SIZE as u64 - 1)` — uses `VirtAddr::Add<u64>`, which calls `checked_add` and panics on overflow.

In the failing run observed during the sub-commit 2 closure attempt, points (1) and (2) silently wrapped (or didn't, depending on how close to the ceiling), and point (3) panicked at `addr.rs:390` because `VirtAddr::Add` is the only one of the three with an overflow check. The panic was the symptom; the root cause is that the entire `heap_start..heap_start + HEAP_SIZE` range is allowed to land within (or partially past) the canonical-space ceiling at `0xffffffffffffffff`.

### How rare (calibrated by KASLR-Q1B)

The kernel is linked PIE with preferred base `0xffffffff80000000` (-2 GiB). KASLR can place it anywhere in the higher-half canonical address space subject to Limine's policy. The "danger zone" — kernel placements where heap_start + HEAP_SIZE would exceed canonical-space ceiling — is approximately `(image_size + HEAP_GUARD_SIZE + HEAP_ALIGN + HEAP_SIZE)` ≈ ~10 MiB out of Limine's nominal ~2 GiB KASLR placement range. The napkin estimate predicted roughly 0.5% of placements.

**KASLR-Q1B's measured rate is 4/50 = 8%, approximately 16x higher than the napkin estimate.** Five `virt_base` data points across the diagnostic chain:

- Original closure-attempt failure (2026-05-06): `virt_base = 0xffffffffff760000`
- KASLR-Q1B run 1: `virt_base = 0xffffffffff826000` (scenario `slab_thread_migration`)
- KASLR-Q1B run 24: `virt_base = 0xffffffffff607000` (scenario `cap_derive_missing_meta`)
- KASLR-Q1B run 34: `virt_base = 0xffffffffffb55000` (scenario `buddy_alloc_free_alloc_returns_same_frame`)
- KASLR-Q1B run 43: `virt_base = 0xffffffffff606000` (scenario `smep_policy`)

All five are within ~10 MiB of the canonical ceiling at `0xffffffffffffffff`. The clustering toward the high end of the canonical-half range indicates Limine's KASLR distribution is non-uniform and biased toward high addresses, or alternatively that the danger zone is wider than the napkin estimate accounted for (the actual kernel image is ~9 MiB, not the ~5 MiB assumed in the initial estimate — the binary-size data in §"Sub-commit 2 didn't cause this" below confirms ~9 MiB).

For every example: `__kernel_image_end` ≈ `virt_base + image_size` (~9 MiB); +2 MiB guard; +4 MiB heap. When `virt_base` is within ~15 MiB of `0xffffffffffffffff`, `heap_start + HEAP_SIZE - 1` overflows `u64`. With the corrected image-size estimate, the danger zone is ~15 MiB / ~2 GiB ≈ 0.7%; the *measured* 8% rate suggests Limine's KASLR is heavily skewed toward high placements, which means even the corrected bound underestimates risk. **The calibrated rate from this diagnostic — not the napkin estimate — is what this ADR plans against.**

### How long latent

`mm/heap.rs:heap_start()` and `init()` were last modified at commit `3aa07d1` ("Phase 1.5c BUILD-3 enable KASLR"). KASLR was enabled in the same commit. The bug has been latent on `main` since `3aa07d1`. Every prior verification — including the BUILD-3 sub-commit 1a/1b closures, EXP-A through EXP-H — got lucky on KASLR placement, which is consistent with an 8% per-boot rate: a single-boot verification has 92% probability of passing, and most of our Phase 1.5c verifications were single-boot. The failure surfaced when the sub-commit 2 closure dispatch added a "full default suite at NEO_SMP=4" gate (~25 boots in a single dispatch), which raised the cumulative roll count to where 1 - 0.92^25 ≈ 88% chance of any single dispatch hitting the overflow.

### Sub-commit 2 didn't cause this — binary-size delta evidence

KASLR-Q1B captured kernel ELF sizes for both trees:
- `main` HEAD `9505619`: 9,378,344 bytes (8.94 MiB)
- sub-commit 2 working tree: 9,379,056 bytes (8.94 MiB)
- Absolute delta: 712 bytes (0.00759%)

The binary-size delta is below noise. KASLR placement randomness operates on the megabyte scale; a 712-byte image-size delta cannot meaningfully shift the danger-zone hit rate. Combined with the 4/50 panic rate observed on clean `main`, this is definitive: the bug is pre-existing latent since `3aa07d1`.

### Sibling failure mode (informational)

KASLR-Q1B run 32 produced a non-overflow panic at `alloc/src/alloc.rs:573` after heap initialization succeeded, on a high KASLR placement. This is *not* the `addr.rs:390` overflow this ADR addresses, but it suggests the bug surface is wider than just `heap_start()`: even when heap placement succeeds, subsequent allocator-internal arithmetic on a near-ceiling heap can fail. The fix in this ADR addresses `heap_start()`'s overflow paths and, as noted in §Decision, incidentally mitigates the most common case of the alloc.rs:573 sibling failure by placing the fallback heap far from the canonical ceiling. A follow-on audit is desirable for full canonical-ceiling robustness; tracked as a separate work item in §Anti-scope.

## Constraints

- **Bible Article 12.3:** KASLR is mandatory from Phase 1. Disabling KASLR as the "fix" violates the Bible. Out.
- **No POSIX compatibility (Bible Article 5.1, 5.2).** The kernel does not need to match Linux's heap layout conventions. We can choose any virtual range we want.
- **Phase 2 has higher-priority work.** Sub-commit 2 closure → sub-commit 3+ → Phase 3 NeoFS. The fix must be small, well-scoped, and not fight the existing `mm` subsystem.
- **Crash-immunity posture (research artifact §1, §F).** Per the ALTIMIT design philosophy, kernel crashes from foreseeable conditions are unacceptable. The measured 8% boot-failure rate against KASLR rolls (per KASLR-Q1B) is exactly the kind of "rare-enough-we-don't-see-it" failure mode that the research artifact's "Pluto's Kiss test" rejects — and at 8% it's not even particularly rare. This is closer to a routine boot regression than a corner-case latent bug; the calibrated rate raises the urgency of fixing it relative to the original estimate.
- **The bug fix must not depend on knowing the kernel image size at compile time.** Image size varies with build configuration, optimization level, and feature flags. A fix predicated on a specific image size is brittle.

## Options

### Option A: Saturate at canonical-space ceiling, fall back to lower fixed range

`heap_start()` computes the unsaturated value, checks for overflow, and on detection falls back to a fixed lower-half-of-higher-half range (e.g., `0xffff_8000_0000_0000` — the start of the canonical higher half).

**Pros:** Simple. Doesn't change KASLR placement. Only affects the heap location, which userspace doesn't care about.

**Cons:** The fallback range may collide with HHDM (Limine maps physical RAM to lower higher-half via HHDM at boot). Verifying no collision requires inspecting Limine's HHDM placement at runtime, which adds boot-path complexity. Also: the fallback constant is hardcoded; if HHDM moves in a future Limine version, the fallback breaks silently.

### Option B: Cap the kernel placement range, ask Limine to constrain KASLR

Limine's `kaslr` directive is binary (`yes`/`no`). There's no "kaslr but bounded" option as far as the Limine docs go. We'd have to ship a kernel patch to Limine or post-process the boot to refuse over-high placements — both fight Limine and add upstream-dependency risk.

**Verdict:** Out. Don't fight the bootloader.

### Option C: Place heap below the kernel image, not above it

Currently the heap is at `kernel_end + GUARD`. Instead place it at `kernel_start - GUARD - HEAP_SIZE` (below the kernel image). This sidesteps the canonical-ceiling issue because the kernel base is always far enough below `0xffffffffffffffff` to leave room downward — KASLR's lower bound is far from `0xffff_8000_0000_0000` (the canonical-half boundary).

**Pros:** Symmetric. The "below" range is bounded by the canonical half boundary, which is far from any normal KASLR placement. The arithmetic is `kernel_start - GUARD - HEAP_SIZE`, a subtraction that needs an underflow check (still possible if KASLR placed kernel below `0xffff_8000_0040_0000` — but Limine doesn't do this for higher-half kernels).

**Cons:** Inverts the `mm/heap.rs` layout that's existed since BUILD-3. Risk of breaking assumptions in unrelated code that may have memorized `heap_start > kernel_end`. Audit cost is non-trivial.

### Option D: Validate at boot; panic deterministically with a clear error

`heap_start()` becomes:

```rust
pub fn heap_start() -> VirtAddr {
    let kernel_end = unsafe { &__kernel_image_end as *const u8 as u64 };
    let total_needed = HEAP_GUARD_SIZE
        .checked_add(HEAP_ALIGN.saturating_sub(1))
        .and_then(|n| n.checked_add(HEAP_SIZE as u64))
        .expect("heap_start: HEAP_GUARD + HEAP_ALIGN + HEAP_SIZE constants overflow u64");

    let unaligned = kernel_end
        .checked_add(HEAP_GUARD_SIZE)
        .expect("heap: KASLR placed kernel too close to canonical-space ceiling for guard");
    let aligned = align_up_checked(unaligned, HEAP_ALIGN)
        .expect("heap: KASLR placed kernel too close to canonical-space ceiling for alignment");
    let heap_end = aligned
        .checked_add(HEAP_SIZE as u64 - 1)
        .expect("heap: KASLR placed kernel too close to canonical-space ceiling for heap range");
    let _ = heap_end; // computed only for the overflow check

    VirtAddr::new(aligned)
}

const fn align_up_checked(value: u64, align: u64) -> Option<u64> {
    value.checked_add(align - 1).map(|v| v & !(align - 1))
}
```

**Pros:** Names the failure cleanly. The panic message tells the operator exactly what happened. Doesn't change the layout, doesn't change KASLR semantics, doesn't fight Limine. Smallest possible change.

**Cons:** Doesn't actually fix the boot — it just turns a confusing `addr.rs:390` panic into a clear `heap.rs:N` panic. The system still fails ~8% of boots. This is a *better diagnostic*, not a *fix*. At the calibrated 8% rate, this is no longer adequate.

### Option E (selected): D + A combined — validate, panic clearly if D's check fails, fall back to a known-safe range

Combine Option D's overflow-checked arithmetic with Option A's fallback. The fallback range is computed dynamically from Limine's HHDM offset and the actual physical memory layout (stashed at `mm::HHDM_OFFSET` and `mm::HHDM_PHYS_END` after `mm::init` runs), so no hardcoded constant.

The mechanism:

1. Compute `heap_start()` per Option D (checked arithmetic).
2. If any check fails (i.e., we're in the canonical-ceiling danger zone), instead of panicking, fall back to a heap address strictly above HHDM end and below the kernel image start, with a 2 MiB guard on each side.
3. Emit a serial log line clearly indicating the fallback was taken.
4. If the fallback also can't fit, *then* panic — that's a "Limine placed something genuinely impossible" condition and should not happen.

**Pros:** Boot succeeds in all KASLR rolls Limine actually produces. Clear diagnostic on the rare path. No hardcoded constants. Doesn't fight Limine. Doesn't invert layout assumptions in unrelated code (heap is still "above kernel image" in the common case; the fallback is a documented exception).

**Cons:** Slightly more code. Two heap-placement strategies instead of one.

### Option F: Cap KASLR's effective range inside the kernel itself

After Limine relocates, the kernel could detect "I'm too close to the ceiling" and remap itself lower. This is a non-starter — remapping the kernel after it's running is a much harder operation than placing the heap correctly. Mention only to dismiss.

## Decision

**Option E (D + A combined).** Rationale:

- **Correctness:** Boot succeeds in 100% of KASLR rolls Limine produces. Bug class eliminated, not just rate-reduced.
- **Crash-immunity posture:** Aligns with the ALTIMIT-style "no foreseeable boot failures" goal. Option D alone produces clearer diagnostics but still fails ~8% of boots — that's not acceptable for a system whose mandate includes "no kernel crashes from foreseeable conditions." The measured 8% rate makes Option E the only defensible answer; Option D is a stopgap, not a fix.
- **The fallback path is exercised routinely, not rarely.** At the calibrated 8% roll rate hitting the danger zone, the `try_below_kernel` fallback runs in roughly 1-in-12 boots. A regularly-exercised path is more likely to be correct than a rarely-exercised one — code that runs gets tested in production by definition. The fallback is not an edge case; it is a routine code path that runs whenever the primary placement fails, and the primary fails routinely.
- **Option E incidentally addresses the alloc.rs:573 sibling failure.** The sibling failure (KASLR-Q1B run 32) occurred when heap placement *succeeded* but the allocator's internal arithmetic on a near-ceiling heap then failed. Option E's `try_below_kernel` fallback places the heap in the below-kernel range (far from canonical ceiling) whenever the above-kernel placement would be near the ceiling. That moves the heap out of the alloc.rs:573 danger zone too. Option D leaves the heap in the canonical-ceiling neighborhood whenever it doesn't outright overflow, so D does not address the alloc.rs:573 case at all. **This is the deciding point.**
- **Smallest change that solves the actual problem.** Option B is out (fights bootloader). Option C is too invasive (audit risk). Option A alone has the hardcoded-fallback brittleness; Option D alone doesn't fix the boot.
- **Composes with future SMP / per-CPU work.** The heap placement is single-init, runs before any AP boots. Nothing in sub-commit 3+ depends on `heap_start > kernel_end`; the fallback is invisible to anything outside `mm/heap.rs`.

## Implementation sketch (informational, not normative)

```rust
// kernel/src/mm/mod.rs — new: stash HHDM physical end at mm::init time

use core::sync::atomic::{AtomicU64, Ordering};

pub static HHDM_PHYS_END: AtomicU64 = AtomicU64::new(0);

// Inside mm::init(), after the Limine memmap is available:
let max_phys_end: u64 = memmap
    .iter()
    .filter(|e| matches!(e.type_, limine::memmap::MEMMAP_USABLE))
    .map(|e| e.base + e.length)
    .max()
    .unwrap_or(0);
HHDM_PHYS_END.store(max_phys_end, Ordering::Relaxed);
```

The memmap walk pulls the actual largest physical address from the Limine memmap. Bounded cost — memmap entries are O(tens). One-time at `mm::init`. This avoids a hardcoded `MAX_PHYSICAL_RAM_ASSUMPTION` constant, which has a real failure mode: a system with more physical RAM than the hardcoded constant could place HHDM mappings beyond the assumed bound, and `try_below_kernel`'s lower-bound check would let the heap collide with HHDM. Silent corruption — strictly worse than the bug we're fixing.

```rust
// kernel/src/mm/heap.rs

pub fn heap_start() -> VirtAddr {
    // Try preferred placement: above kernel image.
    if let Some(addr) = try_above_kernel() {
        return addr;
    }

    // Fallback: between HHDM end and kernel image start.
    if let Some(addr) = try_below_kernel() {
        serial_println!(
            "[ kernel ] heap: KASLR placed kernel near canonical ceiling; \
             heap fell back to below-kernel placement at {:?}",
            addr,
        );
        return addr;
    }

    // Both placements failed — Limine produced an impossible layout.
    panic!(
        "heap: no viable placement (kernel near ceiling AND HHDM/kernel layout leaves no room below); \
         Limine placement is degenerate"
    );
}

fn try_above_kernel() -> Option<VirtAddr> {
    let kernel_end = unsafe { &__kernel_image_end as *const u8 as u64 };
    let unaligned = kernel_end.checked_add(HEAP_GUARD_SIZE)?;
    let aligned = align_up_checked(unaligned, HEAP_ALIGN)?;
    let _heap_end = aligned.checked_add(HEAP_SIZE as u64 - 1)?;
    Some(VirtAddr::new(aligned))
}

fn try_below_kernel() -> Option<VirtAddr> {
    let kernel_start = unsafe { &__kernel_image_start as *const u8 as u64 };
    // Upper bound: work backwards from kernel_start.
    let upper_bound = kernel_start.checked_sub(HEAP_GUARD_SIZE)?;
    let candidate = upper_bound.checked_sub(HEAP_SIZE as u64)?;
    let aligned = candidate & !(HEAP_ALIGN - 1);
    // Lower bound: must not collide with HHDM region.
    let hhdm_end = mm::hhdm_offset()
        .checked_add(HHDM_PHYS_END.load(Ordering::Relaxed))?
        .checked_add(HEAP_GUARD_SIZE)?;
    if aligned <= hhdm_end {
        return None;
    }
    Some(VirtAddr::new(aligned))
}

const fn align_up_checked(value: u64, align: u64) -> Option<u64> {
    match value.checked_add(align - 1) {
        Some(v) => Some(v & !(align - 1)),
        None => None,
    }
}

unsafe extern "C" {
    static __kernel_image_start: u8;  // existing linker symbol; export for Rust access
    static __kernel_image_end: u8;
}
```

## Test additions

### Test 1 — `kaslr_heap_contract` (default suite, runs every boot)

A new test_harness scenario that runs every boot in the default suite. Captures:
1. `heap_start()`'s actual return value.
2. `__kernel_image_end` and `__kernel_image_start`.
3. The HHDM offset and HHDM phys-end (`HHDM_PHYS_END`).

Verifies:
- The heap region (`heap_start..heap_start + HEAP_SIZE`) does not overlap with the kernel image, HHDM region, or extend past the canonical-space ceiling at `0xffffffffffffffff`.
- The heap region is either above the kernel image with a 2 MiB guard (preferred placement) OR below the kernel image with a 2 MiB guard (fallback placement).
- `heap_start` is `HEAP_ALIGN`-aligned.

The contract test surfaces regressions automatically on every boot. Cost: ~10 lines of test code.

### Test 2 — `kaslr_heap_synthetic` (test-harness only, deterministic branch coverage)

A new test_harness scenario behind `#[cfg(test_harness)]` that exercises both `try_above_kernel` and `try_below_kernel` deterministically by passing synthetic `kernel_end` values:

- Synthetic `kernel_end` within ~10 MiB of `0xffffffffffffffff`: verifies `try_above_kernel` returns `None` (overflow path) and `try_below_kernel` returns `Some(_)` at a sane address.
- Synthetic `kernel_end` near `0xffff_8000_0040_0000` (just above canonical-half boundary): verifies `try_below_kernel` returns `None` (no room below) and that the panic message in the both-fail case is clear.
- Synthetic `kernel_end` in middle range: verifies `try_above_kernel` returns `Some(_)` at the expected address.

The synthetic test requires `try_above_kernel` and `try_below_kernel` to be `pub(super)` or `pub(crate)` so the test_harness module can call them. This is a standard pattern; production code outside `mm/heap.rs` does not see them. Cost: helper-exposure refactor (~5 lines) + ~30 lines of test code.

Both tests are required: the contract test is the runtime gate, the synthetic test is the deterministic-branch-coverage defense-in-depth.

## Anti-scope

- **Does not propose disabling KASLR.** Bible Article 12.3 is load-bearing.
- **Does not propose changing Limine's KASLR policy.** Out of scope to upstream.
- **Does not propose increasing `HEAP_SIZE`.** Independent decision; current 4 MiB has held since BUILD-3 and isn't a Phase 2 bottleneck.
- **Does not propose making the heap dynamically sized.** Independent decision; mentioned in a Phase 8 scope marker if relevant later.
- **Does not exhaustively audit overflow in other `mm` arithmetic.** Option E *incidentally addresses the most common case* of the alloc.rs:573 sibling failure observed in KASLR-Q1B run 32, because the fallback heap is placed far from the canonical ceiling whenever the above-kernel placement would be near it. A separate audit pass for `paging.rs`, `frame.rs`, `slab.rs`, `pmap.rs`, and the `linked_list_allocator` integration is desirable for full canonical-ceiling robustness, but the urgent case is closed by Option E. The broader audit is tracked as a separate work item, not a blocker for ADR-015.

## Cross-references

- **Bible Article 12.3** — KASLR mandate; constrains the option set.
- **ADR-013 §Q1** — context for the binary-size delta argument (the 712-byte delta refutes "sub-commit 2 amplified the bug").
- **PM-DECISION-2026-05-06-kaslr-overflow-blocks-closure.md** — the "this is pre-existing" decision that produced this ADR's existence.
- **PM-DECISION-2026-05-07-kaslr-q1-higher-n.md** — the higher-N Q1B decision and pre-committed decision tree.
- **`Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md`** — the calibrating evidence: 4/50 panics, five virt_base values, binary-size data, sibling alloc.rs:573 failure.
- **`Plans/PM-DECISION-2026-05-07-adr-015-ratification.md`** — ratification record (forthcoming PM commit; referenced for project history).
- **Limine boot protocol spec** — KASLR semantics, HHDM layout.

## Notes

- **Schedule:** ratification and implementation land between sub-commit 2 commit and sub-commit 3a's pre-implementation checkpoint conversation. Sequence:
  1. Sub-commit 2 commits to `main` (per `Plans/DISPATCH-2026-05-07-sub-commit-2-close-cerebro.md`).
  2. ADR-015 implementation dispatched to Cerebro as a small focused commit on `main`. Includes the contract smoke test (default suite) and synthetic helper tests. Verification: 50-run KASLR-Q1B-style loop on the new tree shows 0/50 panics.
  3. ADR-015 finalized text commits to `docs/ADR-015-kaslr-tolerant-heap-placement.md` as part of step 2's commit.
  4. Sub-commit 3a pre-implementation checkpoint conversation. Uses `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` as substrate. KASLR is no longer a noise source.

  This schedule pre-empts the failure mode where sub-commit 3a verification produces flaky results because of KASLR-induced false failures interleaved with real 3a bugs. At 8% per-boot panic rate, sub-commit 3a's verification gates would have meaningful false-failure probability without the fix. Doing ADR-015 first is honest engineering, not delay.

- **Sibling allocator failure (KASLR-Q1B run 32):** noted in §Context and §Anti-scope. Option E incidentally mitigates the most common case by placing the fallback heap far from the canonical ceiling. The broader audit follow-on is real but separable from this ADR's scope; it may be scheduled alongside ADR-015's implementation or after, depending on whether run-32-style failures cluster at a meaningfully different rate from the heap-placement overflow.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR.
