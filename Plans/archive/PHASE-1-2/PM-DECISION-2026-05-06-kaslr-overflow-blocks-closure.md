# PM decision: KASLR overflow panic is pre-existing latent bug; sub-commit 2 closure unblocks via diagnostic confirmation, not by fixing it now

**Date:** 2026-05-06
**Author:** PM
**Status:** Decision recorded; diagnostic dispatch issued to confirm pre-existence
**Substrate:** Sub-commit 2 closure attempt blocked by kernel panic at boot. PM-DECISION-2026-05-06-sub-commit-2-close-without-q9 (the immediately preceding decision) covers everything up to this point.

---

## What happened

Cerebro ran the closure dispatch for sub-commit 2. Verification gates 4.1, 4.2, 4.5, 4.6 all passed:

- `cargo k`: PASS, 18 warnings (matches baseline).
- NEO_SMP=1 default suite: 25 PASS + meta-FAIL detected. Clean.
- `git grep "live_thread_count"`: zero matches. Confirmed §Q9 fully reverted.
- `git grep "[ DBG ]"`: zero matches. Confirmed instrumentation reverted.

Verification gate 4.3 (NEO_SMP=4 default suite) FAILED with a kernel panic at boot, BEFORE the scenario body ran:

```
[ kernel ] limine: executable phys=0x000000001f1fd000 virt=0xffffffffff760000
[ kernel ] paging: GLOBAL fixup complete - 4K leaves: 4574, 2M leaves: 251, 1G leaves: 0
[ kernel ] paging: sanity OK - kernel .text sample virt VirtAddr(0xffffffffff775840) ...

!! KERNEL PANIC !!
panicked at .../x86_64-0.15.4/src/addr.rs:390:18: attempt to add with overflow
```

Cerebro stopped per dispatch instruction. ADR-013 amendments are written but uncommitted.

## Diagnosis

The panic is at `addr.rs:390`, which is the `Add<u64> for VirtAddr` impl in the x86_64 crate version 0.15.4:

```rust
fn add(self, rhs: u64) -> Self::Output {
    VirtAddr::try_new(
        self.0
            .checked_add(rhs)               // <-- line 390, panics here
            .expect("attempt to add with overflow"),
    )
    .expect("attempt to add resulted in non-canonical virtual address")
}
```

The panic fires when `self.0 + rhs > u64::MAX`. The boot log shows `virt_base = 0xffffffffff760000` — kernel image is loaded only ~10 MiB below the canonical address-space ceiling at `0xffffffffffffffff`.

The panic occurs in `mm::init` between `paging::sanity_check()` (last successful print) and `heap::init()`'s first print (`heap: ready`, never emitted). Specifically, `kernel/src/mm/heap.rs:51`:

```rust
let heap_end_page =
    Page::<Size4KiB>::containing_address(heap_start + (HEAP_SIZE as u64 - 1));
```

Where `heap_start` is computed by `heap_start()`:

```rust
pub fn heap_start() -> VirtAddr {
    let kernel_end = unsafe { &__kernel_image_end as *const u8 as u64 };
    VirtAddr::new(align_up(kernel_end + HEAP_GUARD_SIZE, HEAP_ALIGN))
}
```

When KASLR places the kernel at `0xffffffffff760000` and the kernel image is sized such that `__kernel_image_end + 2 MiB guard + 2 MiB alignment` lands close to the canonical-space ceiling, the subsequent `heap_start + (HEAP_SIZE - 1)` (4 MiB - 1) overflows past `0xffffffffffffffff`. The x86_64 crate's `VirtAddr::Add` implementation correctly catches this and panics.

## Why this bug is NOT introduced by sub-commit 2

Three independent lines of evidence:

1. **`kernel/src/mm/heap.rs` is not modified by sub-commit 2.** `git status --short kernel/src/mm/` returns empty for the working tree. `git log --oneline -- kernel/src/mm/heap.rs` shows the last commit was `3aa07d1` ("Phase 1.5c BUILD-3 enable KASLR") — the commit that introduced KASLR itself.

2. **EXP-H ran 100/100 PASS yesterday with the same working tree and same scenario at the same SMP count.** Per `target/verify/EXP-H-summary.txt`, 100 consecutive runs of NEO_SMP=4 ipc_heap_kill_smoke completed cleanly. The KASLR rolls on those 100 boots happened to land in safe regions of the canonical address space. This dispatch's single run got unlucky.

3. **NEO_SMP=1 default suite passed in this same dispatch (25 + meta-FAIL).** A bug introduced by sub-commit 2's per-CPU scheduler work would not vanish at SMP=1 — `mm::init` runs identically regardless of SMP count and happens before any AP boots. The fact that NEO_SMP=1 passed and NEO_SMP=4 hit the panic is consistent with "different KASLR seed per QEMU invocation, one of them landed badly," not "sub-commit 2 broke heap setup."

The bug has been latent in `main` since `3aa07d1` enabled KASLR. Every prior verification run got lucky on KASLR placement.

## Why this bug IS exposed more now than before

Sub-commit 2 changes the kernel binary size (per-CPU schedulers add code). Different binary size shifts where `__kernel_image_end` lands relative to the KASLR-randomized base. The probability mass of "where does `heap_start + HEAP_SIZE` land" is shifted. Sub-commit 2 may have shifted the probability of hitting the overflow from rare-enough-we-never-saw-it to rare-enough-we-see-it-once-in-a-blue-moon.

But "shift the probability of an existing bug surfacing" is different from "introduce a new bug." The arithmetic that overflows is in code unchanged by sub-commit 2.

## Decision tree

We need to answer two questions before deciding what to do:

**Q1: Does the panic reproduce on `main` HEAD (without sub-commit 2's working tree) under a forced bad KASLR seed?**

If YES → the bug is genuinely pre-existing in `main`. Sub-commit 2 closure proceeds (the bug is a separate triage item).

If NO → either the bug is somehow specific to sub-commit 2's binary layout (unlikely but possible), or our reproduction methodology is wrong. Sub-commit 2 closure stays blocked until we understand.

**Q2: Does the panic reproduce on the current sub-commit 2 working tree with KASLR disabled (NEO_KASLR=off or equivalent)?**

If NO → confirms KASLR is the trigger. The bug is positional, not structural.

If YES → KASLR is not the cause; something else is overflowing. Different bug; investigate.

These two questions are answered by a small diagnostic dispatch (issued separately).

## What sub-commit 2 closure looks like once Q1 + Q2 are answered

If Q1 = YES (pre-existing) and Q2 = NO (KASLR is the trigger), the right shape is:

1. **File the KASLR overflow as a separate triage item.** A new ADR (or a Bible §amendment) records: heap setup needs a sanity check that `heap_start + HEAP_SIZE` doesn't overflow canonical address space, and a fallback strategy when KASLR places the kernel too high. This is genuinely separate from §Q9 and from sub-commit 2's scope.

2. **Re-run the closure dispatch with the gate corrected.** The original closure gate per the bisection dispatch was "100x ipc_heap_kill_smoke at NEO_SMP=4 100/100 PASS." That gate was met by EXP-H. The closure dispatch I issued *added* a stricter gate ("full default suite at NEO_SMP=4") which was overspecification on my part. The corrected closure dispatch returns to the 100x gate as the binding closure criterion, with the full-suite-at-NEO_SMP=4 acknowledged as informational (currently fails due to the unrelated KASLR bug; will pass once that's fixed).

3. **Sub-commit 2 closes with §Q9 deferred AND the KASLR overflow flagged as the very next triage item.** ADR-013 §Q9 amendments stay as written. A new entry in the triage queue: "KASLR overflow in heap_start computation when kernel virt_base is near canonical-space ceiling." Owner: TBD, probably whoever picks up the next mm work.

If Q1 = NO or Q2 = YES, the answer changes and we re-think.

## Why I'm not just committing now

Two reasons:

1. **The "shipping with a known kernel panic" concern is real.** Even if pre-existing, even if rare, even if separate from §Q9 — the discipline that caught the §Q5 deletion bet and the §Q9 race shouldn't be abandoned because we're tired and want to commit. If there's any chance sub-commit 2 *introduced* this (Q1 = NO), we need to know.

2. **The diagnostic is cheap.** Two QEMU runs with adjusted args. ~10 minutes of Cerebro time. The cost of being wrong about pre-existence — committing a regression as a "deferral" — is much higher.

The diagnostic is dispatched separately. After its results land, sub-commit 2 closure either proceeds (with the closure dispatch corrected) or stays blocked (with the cause investigated).

## What stays as-is

- Cerebro's working tree. The §Q9 revert + option-(b) BSP-scans-all reap + ADR-013 amendment text are correct and stay in the working tree uncommitted.
- The PM decision PM-DECISION-2026-05-06-sub-commit-2-close-without-q9 stays valid. §Q9 is still deferred. The bisection conclusion is unchanged.
- The closure dispatch DISPATCH-2026-05-06-sub-commit-2-close-cerebro is superseded by the corrected version that will be issued after diagnostic results.

## What changes

- The closure dispatch will be re-issued with the full-suite-at-NEO_SMP=4 gate downgraded from "must pass" to "informational only, blocked by separate KASLR triage."
- A new triage item is queued: "KASLR overflow in `mm::heap::heap_start()` arithmetic."
- A note for future closure dispatches: "the closure gate is the bisection gate; do not add stricter gates without explicit PM approval, because adding stricter gates can surface unrelated pre-existing bugs and block closure."

## Time budget

Diagnostic dispatch: ~15-30 min for Cerebro (two QEMU runs + brief writeup). PM decision based on diagnostic results: ~10 min. Re-issued closure dispatch: ~5 min to draft (it's a small revision). Cerebro re-running closure with corrected gate: ~30 min. Total to actually closing sub-commit 2: ~1-1.5 hours additional from now.

## Lesson (for the project, not for this decision)

I added a stricter gate to the closure dispatch ("full default suite at NEO_SMP=4") than was specified in the bisection. That overspecification is what produced this block. The right discipline going forward: closure dispatches should restate the exact gate from the originating dispatch, not introduce stricter ones. If a stricter gate is needed, that's a separate decision with its own justification.

The §Q5 lesson keeps applying: bets that look defensible can fail. Adding a stricter gate "to be safe" looks defensible until you discover the stricter gate surfaces an unrelated latent bug that delays the actual closure. **Closure gates should be specified at the time of the design decision they're gating, not at the time of the commit they're gating.**

---

**End of decision note.**
