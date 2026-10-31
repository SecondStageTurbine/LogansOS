# Dispatch: ADR-015 implementation — KASLR-tolerant kernel heap placement

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-07
**Revised:** 2026-05-07 (in place, twice):
  - **R1:** pre-flight expected-log text updated for the post-INDEX-swap state. Jarvis's INDEX.md sweep landed at `afa4d22` (doc-only, 1-file delta) shortly after this dispatch was first written; the sub-commit 2 hash `0b5ffb4` is now one commit deeper. §2 and §7 expected-log blocks updated to match.
  - **R2:** §3.3 Scenario 2 Test 1 synthetic constant `near_ceiling` changed from `0xffff_ffff_ff80_0000_u64` (8 MiB below `u64::MAX + 1`) to `0xffff_ffff_ffc0_0000_u64` (4 MiB below `u64::MAX + 1`). The original value did not actually trigger the overflow path: `kernel_end + GUARD (2 MiB)` aligned and `+ HEAP_SIZE - 1 (4 MiB - 1)` fits in u64 with 2 MiB to spare from `0xffff_ffff_ff80_0000`, so `try_above_kernel_synthetic` would have returned `Some` and the test's "expected None" assertion would have failed every single run. The new value `0xffff_ffff_ffc0_0000` overflows at the `aligned + (HEAP_SIZE - 1)` checked step (`0xffff_ffff_ffe0_0000 + 0x3f_ffff` overflows u64). Verified by hand. Cerebro caught this pre-flight per the dispatch's "do not change synthetic constants without surfacing" rule. Recorded in `Plans/PM-DECISION-2026-05-07-adr-015-synthetic-constant-correction.md`.
  - Substrate (sub-commit 2 closure, ADR-015 ratification, project local-only) unchanged across both revisions.
**Type:** Implementation dispatch. Code changes + test additions + ADR commit + verification loop. Single focused commit on `main`.
**Substrate:** ADR-015 ratified (`Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md`); ratification record at `Plans/PM-DECISION-2026-05-07-adr-015-ratification.md`. Sub-commit 2 closed at `0b5ffb4`. INDEX.md sweep landed at `afa4d22` (doc-only; the current `main` HEAD). Project is local-only per `Plans/PM-NOTE-2026-05-07-local-only-posture.md` (no push step).

---

## 0. Standing instructions

1. **Single commit.** All ADR-015 work — code changes, test additions, ADR commit to `docs/`, supporting docs — lands as one commit on `main`. Don't split into sub-commits; the change is small and tightly coupled.
2. **Project is local-only.** No push step. After the commit lands and verification passes, you're done.
3. **Empty `git remote -v` is expected.** Do not surface as a blocker.
4. **Stop and surface** if anything is unexpected: `git status` shows files you didn't touch, the build fails, the verification loop produces unexpected results, or you hit any decision point not specified in this dispatch.
5. **Time budget: ~3-5h.** Most of that is the 50-run verification loop at the end (~1.5-2h serial). The code changes themselves are ~30-60 min.

---

## 1. The change set

Five files modified, one ADR added:

1. **`kernel/src/mm/mod.rs`** — Add `HHDM_PHYS_END: AtomicU64` and populate it via memmap walk in `mm::init`.
2. **`kernel/src/mm/heap.rs`** — Rewrite `heap_start()` to use Option E (validate + below-kernel fallback). Expose `try_above_kernel` and `try_below_kernel` as `pub(super)` for test access.
3. **`kernel/src/test_harness.rs`** — Add two new scenarios: `kaslr_heap_contract` (default suite) and `kaslr_heap_synthetic` (deterministic helper coverage).
4. **`kernel/Cargo.toml`** — Add the two new test feature flags.
5. **`tools/xtask/src/main.rs`** — Add the two new scenarios to the suite list.
6. **`docs/ADR-015-kaslr-tolerant-heap-placement.md`** — New file. Copy verbatim from `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md`.

---

## 2. Pre-flight

```powershell
cd "E:\AI Automation\AltimitNeo"

# Confirm the expected commit chain is on HEAD.
git log --oneline -3
# Expected:
#   afa4d22 docs: Plans/INDEX.md — sweep through sub-commit 2 closure
#   0b5ffb4 sched: Phase 2 BUILD-3 sub-commit 2 ...
#   9505619 chore: gitignore ...

# Confirm working tree clean (only Plans/ untracked).
git status --short
# Expected: only `??` entries under Plans/. No `M`, no `A`, no `D`.

# Confirm the FINAL ADR exists.
Test-Path "Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md"
# Expected: True
```

If anything is unexpected, STOP and surface.

---

## 3. Implementation — file by file

### 3.1 `kernel/src/mm/mod.rs` — add `HHDM_PHYS_END`

The current `mm::init(hhdm_offset: u64, memmap: &'static [&'static Entry])` already receives the memmap. Add:

1. Near the existing `HHDM_OFFSET: Once<u64>` declaration (around line 46), add:

```rust
/// Largest physical address (exclusive end) seen in the Limine memmap.
/// Used by `heap::try_below_kernel` to compute the lower bound of the
/// fallback heap range without colliding with HHDM-mapped physical RAM.
/// Set once at `mm::init` time alongside `HHDM_OFFSET`.
pub static HHDM_PHYS_END: AtomicU64 = AtomicU64::new(0);
```

You'll need to add `use core::sync::atomic::{AtomicU64, Ordering};` if not already imported (check the existing imports at top of file).

2. In `mm::init`, after the existing `HHDM_OFFSET.call_once(|| hhdm_offset);` (around line 66), add the memmap walk:

```rust
// Compute the largest physical end-address from usable memmap entries.
// Used by heap::try_below_kernel for HHDM-collision avoidance.
// Bounded cost — memmap entries are O(tens) on real hardware.
let max_phys_end: u64 = memmap
    .iter()
    .filter(|e| matches!(e.entry_type, limine::memory_map::EntryType::USABLE))
    .map(|e| e.base + e.length)
    .max()
    .unwrap_or(0);
HHDM_PHYS_END.store(max_phys_end, Ordering::Release);
serial_println!(
    "[ kernel ] mm: hhdm_phys_end=0x{:016x}",
    max_phys_end
);
```

**Note on Limine API:** the exact field/variant names (`entry_type` vs `type_`, `EntryType::USABLE` vs `MEMMAP_USABLE`, etc.) depend on the Limine crate version in `kernel/Cargo.toml`. Check the existing memmap-iterating code in `mm::init` or elsewhere in `mm/` for the actual idiom and copy that. The conceptual operation is unchanged regardless of the API surface.

### 3.2 `kernel/src/mm/heap.rs` — rewrite `heap_start()`

The full rewrite. Replace the existing `heap_start()` (around line 110) with:

```rust
unsafe extern "C" {
    static __kernel_image_start: u8;
    static __kernel_image_end: u8;
}

pub fn heap_start() -> VirtAddr {
    // Try preferred placement: above kernel image with guard.
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
    // This should never happen on real hardware; if it does, the bug
    // is in Limine's KASLR or in this kernel's mm::init wiring.
    panic!(
        "heap: no viable placement (kernel near ceiling AND HHDM/kernel \
         layout leaves no room below); Limine placement is degenerate"
    );
}

/// Try to place the heap above the kernel image with a 2 MiB guard.
/// Returns `None` if the arithmetic overflows (KASLR placed kernel near
/// canonical-space ceiling) — caller falls back to `try_below_kernel`.
///
/// Exposed `pub(super)` for `test_harness::run_kaslr_heap_synthetic`.
pub(super) fn try_above_kernel() -> Option<VirtAddr> {
    let kernel_end = unsafe { &__kernel_image_end as *const u8 as u64 };
    let unaligned = kernel_end.checked_add(HEAP_GUARD_SIZE)?;
    let aligned = align_up_checked(unaligned, HEAP_ALIGN)?;
    // Verify the heap range itself doesn't overflow.
    let _heap_end = aligned.checked_add(HEAP_SIZE as u64 - 1)?;
    Some(VirtAddr::new(aligned))
}

/// Try to place the heap below the kernel image, above HHDM end.
/// Returns `None` if there's no room between HHDM end (plus guard) and
/// kernel image start (minus guard) for a `HEAP_SIZE` allocation.
///
/// Exposed `pub(super)` for `test_harness::run_kaslr_heap_synthetic`.
pub(super) fn try_below_kernel() -> Option<VirtAddr> {
    let kernel_start = unsafe { &__kernel_image_start as *const u8 as u64 };

    // Upper bound: kernel_start - guard - heap_size, aligned down.
    let upper_bound = kernel_start.checked_sub(HEAP_GUARD_SIZE)?;
    let candidate = upper_bound.checked_sub(HEAP_SIZE as u64)?;
    let aligned = candidate & !(HEAP_ALIGN - 1);

    // Lower bound: must not collide with HHDM region.
    // HHDM_OFFSET + HHDM_PHYS_END + guard < aligned, otherwise return None.
    let hhdm_offset = crate::mm::hhdm_offset();
    let hhdm_phys_end = crate::mm::HHDM_PHYS_END.load(core::sync::atomic::Ordering::Acquire);
    let hhdm_end = hhdm_offset
        .checked_add(hhdm_phys_end)?
        .checked_add(HEAP_GUARD_SIZE)?;
    if aligned <= hhdm_end {
        return None;
    }

    Some(VirtAddr::new(aligned))
}

/// Synthetic-input variant of `try_above_kernel` for test purposes.
/// Takes a synthetic `kernel_end` instead of reading the linker symbol.
///
/// Exposed `pub(super)` for `test_harness::run_kaslr_heap_synthetic` only.
/// Production code uses `try_above_kernel`.
#[cfg(any(test, feature = "neo-test-kaslr-heap-synthetic"))]
pub(super) fn try_above_kernel_synthetic(synthetic_kernel_end: u64) -> Option<VirtAddr> {
    let unaligned = synthetic_kernel_end.checked_add(HEAP_GUARD_SIZE)?;
    let aligned = align_up_checked(unaligned, HEAP_ALIGN)?;
    let _heap_end = aligned.checked_add(HEAP_SIZE as u64 - 1)?;
    Some(VirtAddr::new(aligned))
}

/// Synthetic-input variant of `try_below_kernel` for test purposes.
/// Takes synthetic `kernel_start` and `hhdm_end` instead of reading
/// linker symbol and `HHDM_PHYS_END`.
///
/// Exposed `pub(super)` for `test_harness::run_kaslr_heap_synthetic` only.
#[cfg(any(test, feature = "neo-test-kaslr-heap-synthetic"))]
pub(super) fn try_below_kernel_synthetic(
    synthetic_kernel_start: u64,
    synthetic_hhdm_end: u64,
) -> Option<VirtAddr> {
    let upper_bound = synthetic_kernel_start.checked_sub(HEAP_GUARD_SIZE)?;
    let candidate = upper_bound.checked_sub(HEAP_SIZE as u64)?;
    let aligned = candidate & !(HEAP_ALIGN - 1);
    let hhdm_end = synthetic_hhdm_end.checked_add(HEAP_GUARD_SIZE)?;
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
```

Replace the existing `align_up` (which silently wraps) with `align_up_checked`. The old `align_up` is no longer called anywhere; verify with `grep -n "align_up" kernel/src/mm/heap.rs` and remove the dead version if it's still there after the rewrite.

The existing `__kernel_image_end` declaration around line 28 needs to grow to include `__kernel_image_start`. Both symbols already exist in `kernel/linker.ld` (verified pre-dispatch), so the change is just the `extern "C"` block.

### 3.3 `kernel/src/test_harness.rs` — add two new scenarios

Follow the existing pattern. For each scenario:

1. Add `#[cfg(feature = "neo-test-<name>")] const SCENARIO: &str = "<name>";` near the existing block (around line 233).
2. Add the feature flag to the `compile_error!` mutual-exclusion check arms (around line 290).
3. Add the dispatch arm in `run_selected_scenario` (around line 187).
4. Add the actual `fn run_<name>() -> !` body.

#### Scenario 1: `kaslr_heap_contract`

```rust
#[cfg(feature = "neo-test-kaslr-heap-contract")]
fn run_kaslr_heap_contract() -> ! {
    // Capture actual heap placement and verify the contract.
    let heap_start = crate::mm::heap::heap_start();
    let heap_start_u64 = heap_start.as_u64();
    let heap_end_u64 = match heap_start_u64.checked_add(crate::mm::heap::HEAP_SIZE as u64) {
        Some(e) => e,
        None => fail_with_hex("reason=heap_end_overflow heap_start", heap_start_u64),
    };

    // Capture kernel image bounds.
    unsafe extern "C" {
        static __kernel_image_start: u8;
        static __kernel_image_end: u8;
    }
    let kernel_start = unsafe { &__kernel_image_start as *const u8 as u64 };
    let kernel_end = unsafe { &__kernel_image_end as *const u8 as u64 };

    // Capture HHDM bounds.
    let hhdm_offset = crate::mm::hhdm_offset();
    let hhdm_phys_end = crate::mm::HHDM_PHYS_END.load(core::sync::atomic::Ordering::Acquire);
    let hhdm_end = match hhdm_offset.checked_add(hhdm_phys_end) {
        Some(e) => e,
        None => fail_with_hex("reason=hhdm_end_overflow hhdm_offset", hhdm_offset),
    };

    // Contract 1: heap range does not overflow.
    // (Already verified by the heap_end_u64 computation above.)

    // Contract 2: heap is HEAP_ALIGN-aligned.
    const HEAP_ALIGN: u64 = 2 * 1024 * 1024;
    if heap_start_u64 % HEAP_ALIGN != 0 {
        fail_with_hex("reason=heap_not_aligned heap_start", heap_start_u64);
    }

    // Contract 3: heap does not overlap kernel image.
    let overlaps_kernel =
        (heap_start_u64 >= kernel_start && heap_start_u64 < kernel_end)
        || (heap_end_u64 > kernel_start && heap_end_u64 <= kernel_end)
        || (heap_start_u64 < kernel_start && heap_end_u64 > kernel_end);
    if overlaps_kernel {
        fail_with_hex(
            "reason=heap_overlaps_kernel heap_start",
            heap_start_u64,
        );
    }

    // Contract 4: heap does not overlap HHDM region.
    let overlaps_hhdm =
        (heap_start_u64 >= hhdm_offset && heap_start_u64 < hhdm_end)
        || (heap_end_u64 > hhdm_offset && heap_end_u64 <= hhdm_end)
        || (heap_start_u64 < hhdm_offset && heap_end_u64 > hhdm_end);
    if overlaps_hhdm {
        fail_with_hex(
            "reason=heap_overlaps_hhdm heap_start",
            heap_start_u64,
        );
    }

    // Contract 5: heap is either above kernel (preferred) OR below kernel
    // (fallback). Identify which.
    let placement = if heap_start_u64 >= kernel_end {
        "above"
    } else if heap_end_u64 <= kernel_start {
        "below"
    } else {
        fail_with_hex(
            "reason=heap_neither_above_nor_below_kernel heap_start",
            heap_start_u64,
        );
    };

    serial_println!(
        "NEO_TEST/1 event=PASS name={} placement={} heap_start={:#x} heap_end={:#x} kernel_start={:#x} kernel_end={:#x} hhdm_offset={:#x} hhdm_end={:#x}",
        SCENARIO,
        placement,
        heap_start_u64,
        heap_end_u64,
        kernel_start,
        kernel_end,
        hhdm_offset,
        hhdm_end
    );
    debug_exit(EXIT_SUCCESS);
}
```

#### Scenario 2: `kaslr_heap_synthetic`

**Note on Test 1's `near_ceiling` constant:** the value `0xffff_ffff_ffc0_0000` is chosen so that the checked-arithmetic chain in `try_above_kernel_synthetic` overflows. With `GUARD = 2 MiB`, `ALIGN = 2 MiB`, `HEAP_SIZE = 4 MiB`: `near_ceiling + GUARD = 0xffff_ffff_ffe0_0000` (no overflow), `align_up_checked` is already aligned (no overflow), but `aligned + (HEAP_SIZE - 1) = 0xffff_ffff_ffe0_0000 + 0x3f_ffff` overflows u64 — `checked_add` returns `None` and `try_above_kernel_synthetic` returns `None`. The earlier draft of this dispatch used `0xffff_ffff_ff80_0000` which leaves enough headroom that no step overflows; that constant was wrong (Cerebro caught it pre-flight, see PM-DECISION-2026-05-07-adr-015-synthetic-constant-correction.md).

```rust
#[cfg(feature = "neo-test-kaslr-heap-synthetic")]
fn run_kaslr_heap_synthetic() -> ! {
    use crate::mm::heap::{try_above_kernel_synthetic, try_below_kernel_synthetic};

    // Test 1: synthetic kernel_end ~4 MiB below u64 ceiling.
    // try_above should return None (overflow at aligned + HEAP_SIZE - 1);
    // try_below with reasonable HHDM end should return Some.
    let near_ceiling = 0xffff_ffff_ffc0_0000_u64; // 4 MiB below u64::MAX + 1
    if try_above_kernel_synthetic(near_ceiling).is_some() {
        fail_with_hex(
            "reason=try_above_succeeded_near_ceiling synthetic_end",
            near_ceiling,
        );
    }

    // For try_below: synthetic kernel_start matching the near-ceiling kernel
    // (image is ~9 MiB, so kernel_start ≈ kernel_end - 9 MiB), HHDM end
    // ~16 GiB (well below kernel start).
    let synthetic_kernel_start = near_ceiling - 9 * 1024 * 1024;
    let synthetic_hhdm_end = 0xffff_8000_0000_0000_u64 + 16 * 1024 * 1024 * 1024;
    if try_below_kernel_synthetic(synthetic_kernel_start, synthetic_hhdm_end).is_none() {
        fail("reason=try_below_returned_none_with_room_available");
    }

    // Test 2: synthetic kernel_start near canonical-half boundary
    // (no room below for the heap). try_below should return None.
    let near_half_boundary = 0xffff_8000_0040_0000_u64; // 4 MiB above canonical half base
    let synthetic_hhdm_end_low = 0xffff_8000_0010_0000_u64;
    if try_below_kernel_synthetic(near_half_boundary, synthetic_hhdm_end_low).is_some() {
        fail_with_hex(
            "reason=try_below_succeeded_near_half_boundary synthetic_start",
            near_half_boundary,
        );
    }

    // Test 3: middle-range synthetic. try_above should return Some at
    // the expected aligned address.
    let middle = 0xffff_ffff_8000_0000_u64; // -2 GiB, the link-time base
    let result = match try_above_kernel_synthetic(middle) {
        Some(v) => v,
        None => fail_with_hex(
            "reason=try_above_returned_none_in_middle_range synthetic_end",
            middle,
        ),
    };
    let result_u64 = result.as_u64();
    // Verify alignment.
    const HEAP_ALIGN: u64 = 2 * 1024 * 1024;
    if result_u64 % HEAP_ALIGN != 0 {
        fail_with_hex(
            "reason=middle_range_result_not_aligned result",
            result_u64,
        );
    }
    // Verify the result is at least middle + HEAP_GUARD_SIZE.
    const HEAP_GUARD_SIZE: u64 = 2 * 1024 * 1024;
    if result_u64 < middle + HEAP_GUARD_SIZE {
        fail_with_hex(
            "reason=middle_range_result_below_kernel_end_plus_guard result",
            result_u64,
        );
    }

    serial_println!(
        "NEO_TEST/1 event=PASS name={} test1=ok test2=ok test3=ok middle_result={:#x}",
        SCENARIO,
        result_u64
    );
    debug_exit(EXIT_SUCCESS);
}
```

### 3.4 `kernel/Cargo.toml` — add feature flags

In the `[features]` section, add:

```toml
neo-test-kaslr-heap-contract = []
neo-test-kaslr-heap-synthetic = []
```

### 3.5 `tools/xtask/src/main.rs` — add suite entries

In the suite list (around lines 60-260), add two entries matching the existing pattern:

```rust
TestScenario {
    name: "kaslr_heap_contract",
    feature: "neo-test-kaslr-heap-contract",
    // (other fields per existing TestScenario shape)
},
TestScenario {
    name: "kaslr_heap_synthetic",
    feature: "neo-test-kaslr-heap-synthetic",
    // (other fields per existing TestScenario shape)
},
```

Read the existing `TestScenario` struct to confirm the field shape; copy from a similar scenario (e.g., `kaslr_smoke`) and adjust the name and feature.

### 3.6 `docs/ADR-015-kaslr-tolerant-heap-placement.md` — copy from FINAL

```powershell
Copy-Item "Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md" "docs/ADR-015-kaslr-tolerant-heap-placement.md"
```

The FINAL is verbatim what commits to `docs/`. Do not edit during the copy.

---

## 4. Build verification

```powershell
# Build the kernel and confirm it compiles cleanly.
cargo k
# Expected: 0 errors, 0 warnings.

# Build with each new test feature to verify the feature gating.
cargo build --features neo-test,neo-test-kaslr-heap-contract --target kernel/x86_64-unknown-none.json -p kernel
cargo build --features neo-test,neo-test-kaslr-heap-synthetic --target kernel/x86_64-unknown-none.json -p kernel
# Expected: both compile cleanly.
```

If any of the above fails, surface the build errors. Don't try to fix them on the fly — surface, get specifics, then proceed.

---

## 5. Functional verification — single-run

Before the 50-run loop, verify each scenario works in a single boot.

```powershell
# Run kaslr_heap_contract — should PASS.
docker compose run --rm neo cargo neo-test --scenario kaslr_heap_contract
# Expected: NEO_TEST/1 event=PASS name=kaslr_heap_contract placement=<above|below> ...

# Run kaslr_heap_synthetic — should PASS.
docker compose run --rm neo cargo neo-test --scenario kaslr_heap_synthetic
# Expected: NEO_TEST/1 event=PASS name=kaslr_heap_synthetic test1=ok test2=ok test3=ok ...
```

If either fails, surface the failure log. The synthetic test failing on its first run almost certainly means a bug in the synthetic-input helpers — the math is straightforward; double-check the synthetic constants if so.

---

## 6. The verification loop — 50 runs

The ADR-015 ratification's Decision 5 specifies a 50-run KASLR loop showing 0/50 panics on the new tree. This is the load-bearing verification.

```powershell
# Same loop pattern as KASLR-Q1B but with the FIXED kernel.
# Runs the default suite (which includes kaslr_heap_contract automatically),
# expecting zero attempt-to-add-with-overflow panics.

# Pseudocode — adapt to your existing Q1B loop wrapper:
$results = @()
for ($i = 1; $i -le 50; $i++) {
    Write-Host "Run $i/50..."
    $log = "target/verify/ADR-015-loop-run-$i.serial.log"
    $result = docker compose run --rm neo cargo neo-test 2>&1 | Tee-Object -FilePath $log
    # Classify: PASS, PANIC, TIMEOUT, OTHER
    # Same classification logic as KASLR-Q1B's loop.
    $results += [PSCustomObject]@{
        Run = $i
        Classification = (Classify-Result $log)
    }
}

$results | Group-Object Classification | Format-Table -AutoSize
```

Adapt the loop wrapper to match what KASLR-Q1B used; the key invariants are:

1. Run all 50 boots back-to-back, no rebuild between them (KASLR re-rolls per boot via Limine).
2. Capture the serial log for every run.
3. Classify each run as PASS / PANIC (specifically `attempt to add with overflow` or `addr.rs:390`) / TIMEOUT / OTHER.
4. **Stop early** if 2 consecutive PANIC runs occur — that indicates the fix is broken and we surface immediately rather than burning the full loop.

Expected result: **0 PANIC, 50 PASS (or PASS + harness-wedge TIMEOUTs at roughly the same ~28% rate seen in KASLR-Q1B).**

If any PANIC occurs:
- Capture the failing virt_base from the serial log.
- Capture the panic site (file:line).
- STOP and surface to PM. Do not attempt to fix.

If 0 PANIC across all 50:
- The fix is verified at the empirical level.
- Combined with the deterministic `kaslr_heap_synthetic` test passing, this is strong evidence the bug class is eliminated. (Statistical upper bound at 95% CI with 0/50: residual rate ≤ ~6%. The synthetic test deterministically covers the branches that produce the residual, so the combined evidence is much stronger than the loop alone.)

---

## 7. Stage and commit

```powershell
# Stage the six file changes.
git add kernel/src/mm/mod.rs
git add kernel/src/mm/heap.rs
git add kernel/src/test_harness.rs
git add kernel/Cargo.toml
git add tools/xtask/src/main.rs
git add docs/ADR-015-kaslr-tolerant-heap-placement.md

# Verify the stage.
git diff --staged --stat
# Expected: 6 files changed, sizable insertion count, modest deletion count.

# Compose the commit message.
$msg = @"
mm: ADR-015 KASLR-tolerant heap placement (Option E: validate + below-kernel fallback)

Implements ADR-015. The pre-existing KASLR overflow at heap_start (latent
since 3aa07d1, surfaced 2026-05-06, calibrated 8% per-boot panic rate by
KASLR-Q1B) is fixed via Option E:

* heap_start() now tries try_above_kernel() first (the original placement,
  with checked arithmetic). On overflow, falls back to try_below_kernel()
  which places the heap between HHDM end and kernel image start. If both
  fail, panic with a clear diagnostic.

* mm::HHDM_PHYS_END (AtomicU64) is set at mm::init time via memmap walk
  for usable entries. Used by try_below_kernel for HHDM-collision
  avoidance.

* Two new test scenarios:
  - kaslr_heap_contract (default suite): captures actual heap placement
    every boot, verifies non-overlap with kernel/HHDM, alignment,
    canonical-ceiling safety. Surfaces regressions automatically.
  - kaslr_heap_synthetic: deterministic branch coverage via
    pub(super) try_above_kernel_synthetic and try_below_kernel_synthetic.
    Tests near-ceiling, near-half-boundary, and middle-range cases.

* docs/ADR-015 lands as the ratified text; matches the FINAL artifact at
  Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md.

Verification:
* 50-run KASLR loop on this tree: 0/50 panics (vs 4/50 on parent 0b5ffb4
  per Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md).
* kaslr_heap_synthetic: PASS (all three synthetic branches verified).
* kaslr_heap_contract: PASS (every default-suite boot now runs this).

Option E incidentally addresses the alloc.rs:573 sibling failure mode
(KASLR-Q1B run 32) by placing the fallback heap far from canonical ceiling
whenever the above-kernel placement would land near it. The broader mm/
canonical-ceiling audit remains a separate work item.

Refs: ADR-015, Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md,
Plans/PM-DECISION-2026-05-07-adr-015-ratification.md,
Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md, 3aa07d1 (origin of bug),
0b5ffb4 (sub-commit 2), afa4d22 (INDEX.md sweep, parent commit).
"@
$msg | Out-File -FilePath ".git/COMMIT_EDITMSG_ADR015" -Encoding utf8 -NoNewline
git commit -F ".git/COMMIT_EDITMSG_ADR015"
Remove-Item ".git/COMMIT_EDITMSG_ADR015"

# Verify the commit landed.
git log --oneline -4
# Expected:
# <new hash> mm: ADR-015 KASLR-tolerant heap placement ...
# afa4d22 docs: Plans/INDEX.md — sweep through sub-commit 2 closure
# 0b5ffb4 sched: Phase 2 BUILD-3 sub-commit 2 ...
# 9505619 chore: gitignore ...
```

---

## 8. Close report

Write `Plans/CLOSE-REPORT-2026-05-07-adr-015-implementation.md` with:

```markdown
# Close report: ADR-015 implementation

## Commit
- Hash: <new commit hash>
- Branch: main
- Lands at: 2026-05-07 ~<time>
- Parent: afa4d22 (INDEX.md sweep) → 0b5ffb4 (sub-commit 2) → 9505619
- (Project is local-only; no push step.)

## What was committed
- Option E heap_start() rewrite (validate + below-kernel fallback)
- mm::HHDM_PHYS_END atomic populated via memmap walk at mm::init
- kaslr_heap_contract default-suite scenario
- kaslr_heap_synthetic deterministic-branch-coverage scenario
- ADR-015 committed to docs/ from the FINAL artifact

## Verification gates
- Build: cargo k clean (0 errors, 0 warnings)
- Single-run kaslr_heap_contract: PASS
- Single-run kaslr_heap_synthetic: PASS (test1=ok test2=ok test3=ok)
- 50-run KASLR loop: 0/50 PANIC, <N>/50 PASS, <N>/50 TIMEOUT
  (TIMEOUT rate consistent with the known harness wedge from KASLR-Q1B)

## Comparison to KASLR-Q1B baseline
- KASLR-Q1B (parent commit 0b5ffb4): 4/50 PANIC, 31/50 PASS, 14/50 TIMEOUT
- ADR-015 implementation tree: <N>/50 PANIC, <N>/50 PASS, <N>/50 TIMEOUT
- Bug class eliminated: yes (0 panics observed)

## Sibling alloc failure
The alloc.rs:573 sibling failure observed once in KASLR-Q1B run 32 was
not reproduced in this tree's 50 runs. This is consistent with Option E
incidentally addressing the most common case by placing the fallback heap
far from the canonical ceiling. The broader mm/ canonical-ceiling audit
remains a separate work item.

## Next steps for PM
- INDEX.md update (Jarvis dispatch) flipping ADR-015-FINAL from draft to
  closed, adding the implementation commit reference, adding this close
  report.
- Sub-commit 3a pre-implementation checkpoint conversation now unblocked.
```

Ping PM with the commit hash.

---

## 9. What you do NOT do

- Do not attempt to disable KASLR. Bible Article 12.3 mandates it.
- Do not modify `kernel/linker.ld`. Both `__kernel_image_start` and `__kernel_image_end` already exist there (verified pre-dispatch); the dispatch only requires the Rust `extern "C"` declaration.
- Do not refactor unrelated `mm/` code. The dispatch scope is heap_start placement only; the broader canonical-ceiling audit is anti-scope per ADR-015 §Anti-scope.
- Do not change the synthetic test constants without surfacing first. The values are chosen to deterministically exercise specific branches; changing them changes what's being tested.
- Do not split the work into multiple commits. Single commit lands the whole change.
- Do not push or attempt to set up a remote. Project is local-only.
- Do not surface "empty git remote -v" as a blocker.

---

## 10. Time budget

- 5 min: pre-flight check (§2).
- 30-45 min: code changes (§3.1 through §3.5). Most of this is §3.2 (heap.rs rewrite) and §3.3 (test scenarios).
- 5 min: ADR copy (§3.6).
- 10-15 min: build verification + single-run tests (§4 + §5).
- 90-120 min: 50-run KASLR verification loop (§6). Bulk of total time.
- 10 min: commit (§7) and close report (§8).

**Total: ~3-4h, mostly the verification loop.** If the verification loop takes longer than 2h, that's the harness-wedge timeout rate doing what it does — not a problem unless the PANIC count goes nonzero.

If you're tired and want to do the code changes today and the loop tomorrow morning, that's acceptable — just commit a stash with a descriptive name (e.g., `git stash push -m "adr-015-implementation-pre-loop-2026-05-07"`) and resume tomorrow. Do NOT commit the kernel changes without the loop verification; the load-bearing claim of this commit is "0/50 panics" and committing without the evidence is dishonest.

---

## 11. Why this matters

ADR-015 implementation eliminates the calibrated 8% per-boot crash rate that has been polluting verification gates since `3aa07d1`. Sub-commit 3a's pre-implementation checkpoint is blocked on this — at 8% per-boot panic rate, 3a's verification would have meaningful false-failure probability without the fix.

This is also the first ADR ratified by PM under project-lead delegation. Getting it right (clean implementation, complete verification, accurate close report) sets the precedent for future delegated ratifications.

---

**End of dispatch.**
