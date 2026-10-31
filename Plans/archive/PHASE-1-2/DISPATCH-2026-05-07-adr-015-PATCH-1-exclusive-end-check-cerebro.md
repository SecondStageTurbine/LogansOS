# Dispatch: ADR-015 PATCH 1 — exclusive-end check correction

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-07
**Type:** Targeted patch on top of the uncommitted ADR-015 working tree. Three small code changes + verification re-run. Single commit lands the patched ADR-015 work.
**Substrate:** Cerebro is on `Plans/DISPATCH-2026-05-07-adr-015-implementation-cerebro.md`, paused after run 24 of the 50-run loop with `cap_grant_missing_meta` triggering the alloc.rs:573 panic. ADR-015 code/docs are implemented, uncommitted, and need this patch before the load-bearing 0/50 panic claim can be made truthfully. This patch dispatch is the third correction of the day on ADR-015 work; the prior two (R1 post-INDEX-swap pre-flight, R2 synthetic constant) were textual fixes to the dispatch *before* execution. This one is a code-side patch *during* execution.

---

## 0. Standing instructions

1. **Do NOT discard or revert the existing uncommitted work.** Cerebro's working tree currently has ADR-015 code/tests/docs that are correct in structure and 95% correct in content. This patch makes three small targeted edits on top of that work, then resumes verification.
2. **Single commit.** When verification re-runs cleanly, all the ADR-015 work — the original implementation plus this patch — lands as one commit. Do not split.
3. **Discard the old loop results.** The 25 runs done before this patch are on broken code; the 0/50 panic claim cannot be made on them. Restart the verification loop from run 1 with the patched binary. Old logs can be left in place for the wedge investigation; new logs go to a fresh path (see §5 below).
4. **Time budget: ~2.5h.** ~10 min for the patch, ~5 min for build, ~5 min for single-run sanity, ~120 min for the fresh 50-run loop. The patch itself is small.
5. **Stop and surface** if anything is unexpected: build fails, single-run sanity fails, verification produces a new failure mode I haven't predicted here.

---

## 1. Why this patch exists

Run 24 (`cap_grant_missing_meta`) hit:

```
[ kernel ] heap: ready (4 MiB at VirtAddr(0xffffffffffc00000), 1024 pages mapped)
panicked at .../alloc/src/alloc.rs:573:9: memory allocation of 8 bytes failed
```

Diagnosis:

- `heap_start = 0xffff_ffff_ffc0_0000`
- `HEAP_SIZE = 4 MiB = 0x40_0000`
- `heap_start + HEAP_SIZE = 0xffff_ffff_ffc0_0000 + 0x40_0000 = 0x1_0000_0000_0000_0000` → **overflows u64**

The allocator (`linked_list_allocator`) computes `heap_end = heap_start + heap_size` as part of its bounds machinery. With this heap placement, that computation overflows; the allocator's internal end-pointer is bogus; the very first 8-byte allocation fails.

The bug is in `try_above_kernel`'s overflow check. The current check uses `aligned + (HEAP_SIZE - 1)` — it verifies the *last byte* is representable. The allocator wants `aligned + HEAP_SIZE` to be representable — the *exclusive end*.

These differ by one byte. With `aligned = 0xffff_ffff_ffc0_0000`:

- `aligned + (HEAP_SIZE - 1) = 0xffff_ffff_ffff_ffff` — fits exactly. Current check passes.
- `aligned + HEAP_SIZE = 0x1_0000_0000_0000_0000` — overflows. Allocator chokes.

The fix: change the check from `HEAP_SIZE - 1` to `HEAP_SIZE`. This makes `try_above_kernel` return `None` for `aligned = 0xffff_ffff_ffc0_0000` (and any other case where the exclusive end overflows), causing fallback to `try_below_kernel`.

This is the Cerebro-proposed fix. The math is verified by hand in `Plans/PM-DECISION-2026-05-07-adr-015-exclusive-end-check.md`.

---

## 2. Pre-flight

```powershell
cd "E:\AI Automation\AltimitNeo"

# Confirm working tree state matches what we expect.
git status --short
# Expected:
#   M kernel/Cargo.toml
#   M kernel/src/mm/heap.rs
#   M kernel/src/mm/mod.rs
#   M kernel/src/test_harness.rs
#   M tools/xtask/src/main.rs
#   ?? docs/ADR-015-kaslr-tolerant-heap-placement.md   (or staged depending on prior workflow)
#   ?? Plans/...
# The five M entries are your in-flight ADR-015 work. The ADR copy may be staged or unstaged
# depending on how you've been working; either is fine.

# Confirm HEAD has not moved.
git log --oneline -1
# Expected: afa4d22 docs: Plans/INDEX.md — sweep through sub-commit 2 closure

# Confirm the failing log is on disk for reference.
Test-Path "target/verify/ADR-015-loop-run-24.cap_grant_missing_meta.serial.log"
# Expected: True
```

If `git log` doesn't show `afa4d22` on top, or the M entries differ from the expected five, STOP and surface.

---

## 3. The patch — three changes

### 3.1 `kernel/src/mm/heap.rs` — fix `try_above_kernel`

Find the line:

```rust
    let _heap_end = aligned.checked_add(HEAP_SIZE as u64 - 1)?;
```

inside `try_above_kernel`. Change `HEAP_SIZE as u64 - 1` to `HEAP_SIZE as u64`:

```rust
    // Verify the exclusive end (aligned + HEAP_SIZE) is representable in u64.
    // The allocator computes heap_end = heap_start + heap_size internally; if
    // that overflows, allocation requests fail despite the heap region being
    // notionally valid. Checking the inclusive end (HEAP_SIZE - 1) is one
    // byte too lenient — it allows aligned == 0xffff_ffff_ffc0_0000 with a
    // 4 MiB heap, where exclusive end = 0x1_0000_0000_0000_0000 overflows.
    let _heap_end_exclusive = aligned.checked_add(HEAP_SIZE as u64)?;
```

The comment is required. Future readers need to know why this is `HEAP_SIZE`, not `HEAP_SIZE - 1`.

### 3.2 `kernel/src/mm/heap.rs` — same fix in `try_above_kernel_synthetic`

Find the corresponding line inside the `#[cfg(...)] pub(super) fn try_above_kernel_synthetic` body:

```rust
    let _heap_end = aligned.checked_add(HEAP_SIZE as u64 - 1)?;
```

Apply the same change:

```rust
    // Mirror try_above_kernel's exclusive-end check exactly.
    let _heap_end_exclusive = aligned.checked_add(HEAP_SIZE as u64)?;
```

The synthetic helper must mirror production behavior exactly — they exist to give the synthetic test deterministic coverage of the production logic. If they diverge, the synthetic test stops being a proxy for the production check.

### 3.3 `kernel/src/test_harness.rs` — add Test 4 to `kaslr_heap_synthetic`

In `run_kaslr_heap_synthetic`, after the existing Test 3 (middle-range) block and before the closing `serial_println!`, insert:

```rust
    // Test 4: exact-ceiling case — kernel_end + GUARD aligns up to a value
    // where exclusive end overflows but inclusive end (HEAP_SIZE - 1) does not.
    // This is the case that hit production at ADR-015 run 24 (cap_grant_missing_meta):
    //   kernel_end = 0xffff_ffff_ffa0_0000
    //   + GUARD (2 MiB) = 0xffff_ffff_ffc0_0000
    //   align_up to 2 MiB = 0xffff_ffff_ffc0_0000 (already aligned)
    //   + HEAP_SIZE (4 MiB) = 0x1_0000_0000_0000_0000 — overflows u64
    //   + HEAP_SIZE - 1 (4 MiB - 1) = 0xffff_ffff_ffff_ffff — fits (which is why the
    //   pre-patch check incorrectly accepted this case).
    // try_above must return None for this synthetic_kernel_end value.
    let exact_ceiling = 0xffff_ffff_ffa0_0000_u64;
    if try_above_kernel_synthetic(exact_ceiling).is_some() {
        fail_with_hex(
            "reason=try_above_succeeded_at_exact_ceiling synthetic_end",
            exact_ceiling,
        );
    }
```

Update the closing `serial_println!` to include `test4=ok`:

```rust
    serial_println!(
        "NEO_TEST/1 event=PASS name={} test1=ok test2=ok test3=ok test4=ok middle_result={:#x}",
        SCENARIO,
        result_u64
    );
```

This test is the regression guard for run 24's failure mode. If this test ever fails in the future, it means someone reverted the exclusive-end check (or introduced an equivalent off-by-one).

---

## 4. Build verification

```powershell
cargo k
# Expected: builds clean. Project's actual baseline is ~18 warnings on cargo k
# (despite what the original dispatch claimed; see PM-DECISION on warnings baseline).
# What matters: 0 errors, no NEW warnings vs prior ADR-015 build.

cargo build --features neo-test,neo-test-kaslr-heap-contract --target kernel/x86_64-unknown-none.json -p kernel
cargo build --features neo-test,neo-test-kaslr-heap-synthetic --target kernel/x86_64-unknown-none.json -p kernel
# Expected: both build. ~39 warnings each (test scaffolding).
```

If errors or NEW warnings, surface.

---

## 5. Functional verification — single-run

```powershell
docker compose run --rm neo cargo neo-test --scenario kaslr_heap_contract
# Expected: NEO_TEST/1 event=PASS name=kaslr_heap_contract placement=above heap_start=0x... ...
# This is non-deterministic w.r.t. KASLR roll; on most rolls placement=above.

docker compose run --rm neo cargo neo-test --scenario kaslr_heap_synthetic
# Expected: NEO_TEST/1 event=PASS name=kaslr_heap_synthetic test1=ok test2=ok test3=ok test4=ok middle_result=0x...
# Note: now four sub-tests, all must be ok.
```

If the contract scenario PASSes but synthetic FAILs at `test4`, the patch isn't applied correctly — re-check §3.1 and §3.2. If synthetic PASSes but with old wording (no `test4=ok`), the test_harness change in §3.3 wasn't applied.

---

## 6. The verification loop — fresh 50 runs

The 25 runs from the prior loop are invalidated. Restart from scratch.

```powershell
# Use a fresh log path so we don't comingle pre-patch and post-patch results.
# Old: target/verify/ADR-015-loop-run-N.<scenario>.serial.log  (preserve for wedge investigation)
# New: target/verify/ADR-015-PATCH1-loop-run-N.<scenario>.serial.log

# Adapt your existing loop to the new path prefix. Keep all classification logic identical.
$results = @()
for ($i = 1; $i -le 50; $i++) {
    Write-Host "Run $i/50..."
    # ... (same shape as before, with new log path prefix)
    $results += [PSCustomObject]@{
        Run = $i
        Classification = (Classify-Result $log)
    }
}
$results | Group-Object Classification | Format-Table -AutoSize
```

Same invariants as the original §6:

1. 50 boots back-to-back, no rebuild between.
2. Capture serial log for every run.
3. Classify PASS / PANIC / TIMEOUT / OTHER.
4. Stop early if 2 consecutive PANIC runs occur — that means the patch is broken.
5. Stop early if any OTHER classification matches the alloc.rs:573 signature again — that means the exclusive-end fix was incomplete.

**Expected result:** **0 PANIC, 0 OTHER (specifically zero alloc.rs:573 panics), 50 PASS or PASS+TIMEOUT** with a TIMEOUT rate roughly comparable to the wedge baseline (~20-28% from prior data).

If the loop produces ANY OTHER classification with the alloc.rs:573 signature, STOP and surface. That would mean the exclusive-end check still has a hole somewhere, or there's a third bug in the heap-placement chain we haven't found.

If the loop produces ANY PANIC matching the original "attempt to add with overflow" / addr.rs:390 signature, STOP and surface. That would mean the patch broke the original ADR-015 fix somehow (extremely unlikely given the patch is a strict tightening, but possible).

---

## 7. Stage and commit

Same as the original dispatch §7, with three commit-message updates:

1. The bullet about `kaslr_heap_synthetic` should now say **"Tests near-ceiling, near-half-boundary, middle-range, *and exact-ceiling* cases."**
2. Add a new section to the body:

```
* PATCH 1 (during verification): the original ADR-015 implementation used
  HEAP_SIZE - 1 in the try_above_kernel overflow check (verifying the
  inclusive end is representable). The allocator computes heap_end =
  heap_start + heap_size, so the EXCLUSIVE end must be representable —
  one byte stricter. Run 24 of the in-flight verification loop produced
  an alloc.rs:573 panic on cap_grant_missing_meta with heap at
  0xffff_ffff_ffc0_0000, where inclusive end fit u64 but exclusive end
  did not. Fixed by changing the check to HEAP_SIZE (no `- 1`) in both
  production and synthetic helpers, and adding test4 to kaslr_heap_synthetic
  as the regression guard. See Plans/PM-DECISION-2026-05-07-adr-015-exclusive-end-check.md.
```

3. The verification claim should reference the patched loop:

```
Verification:
* 50-run KASLR loop on patched tree: 0/50 panics, 0/50 alloc.rs:573 OTHERs.
* kaslr_heap_synthetic: PASS (test1=ok test2=ok test3=ok test4=ok).
* kaslr_heap_contract: PASS (every default-suite boot now runs this).
* Pre-patch loop run 24 reproduced the alloc.rs:573 failure with
  heap=0xffff_ffff_ffc0_0000; post-patch verification on identical KASLR
  rolls is not directly possible (KASLR is per-boot random) but the
  exact-ceiling synthetic test (test4) deterministically covers the bug class.
```

The commit captures the original ADR-015 work AND the patch as one commit. There is no separate "fixup" commit. The git history shows one ADR-015 commit on top of `afa4d22`.

---

## 8. Close report

Same shape as the original dispatch §8, with two additions:

1. **"What was committed"** section adds: `Exclusive-end check correction (PATCH 1) — try_above_kernel and synthetic helper now check aligned + HEAP_SIZE rather than aligned + HEAP_SIZE - 1; new test4 in kaslr_heap_synthetic guards the regression class.`

2. **"Verification gates"** updates to include test4 and the patched loop result.

3. **A new section "Pre-patch failure"**:

```markdown
## Pre-patch failure (run 24 of in-flight loop)

The original ADR-015 implementation reached run 24 of the verification
loop before producing an OTHER classification on cap_grant_missing_meta
matching alloc.rs:573 (memory allocation of 8 bytes failed). Diagnosis
showed heap_start = 0xffff_ffff_ffc0_0000, where heap_start + HEAP_SIZE
overflows u64 — a case the original `aligned + (HEAP_SIZE - 1)` check
admitted because the inclusive end fits.

PATCH 1 changed the check to `aligned + HEAP_SIZE` and added test4 to
the synthetic test scenario as a deterministic regression guard. The
post-patch loop produced 0/50 panics and 0/50 OTHER classifications
matching alloc.rs:573. The pre-patch run 24 log is preserved at
target/verify/ADR-015-loop-run-24.cap_grant_missing_meta.serial.log
for the project record.
```

Ping PM with the commit hash. Note that this is patched ADR-015, not vanilla — the close report's title remains `CLOSE-REPORT-2026-05-07-adr-015-implementation.md`, but its body must mention the patch.

---

## 9. What you do NOT do

- Do not revert the original ADR-015 working-tree changes. The patch is on top of them.
- Do not split the patch from the ADR-015 base into two commits. Single commit.
- Do not modify the `kaslr_heap_contract` scenario. The contract test is already correct as written; the bug was in the production heap_start logic, not the contract assertions.
- Do not modify any code outside the three locations specified in §3. Specifically: `try_below_kernel` is correct as written (its check is on `kernel_start - HEAP_SIZE`, and underflow is bounded by `HHDM_OFFSET`, so it doesn't have the same off-by-one).
- Do not delete or rename old loop logs. They feed the wedge investigation; preserve them.

---

## 10. Time budget

- 5 min: pre-flight check (§2).
- 10 min: apply patch (§3) — three small edits.
- 5 min: build verification (§4).
- 5 min: single-run sanity (§5).
- ~120 min: fresh 50-run loop (§6).
- 10 min: commit (§7) + close report (§8).

**Total: ~2.5h.** If you'd rather do the loop tomorrow morning, that's fine — the patch and single-run sanity can land tonight, with the loop deferred. Surface a stash with descriptive name; resume tomorrow. Do NOT commit the kernel changes without the loop verification.

---

## 11. Why this matters

ADR-015's load-bearing claim is "0/50 panics on the verification loop." The pre-patch loop produced 0 PANICs — but it produced 1 OTHER (alloc.rs:573) that is structurally the same problem ADR-015 set out to fix, just one byte off. Committing without the patch would be technically defensible (the original PANIC class IS gone), but it would let an adjacent failure mode through that anyone running the kernel would discover within ~25 boots.

This is a legitimate correctness concern, not a perfectionism concern. ADR-015's purpose is to make heap placement KASLR-tolerant; an alloc.rs:573 OTHER from heap-end overflow is exactly as bad as an addr.rs:390 PANIC from heap-start overflow. The patch closes both.

This is also the third correction of the day on ADR-015 work. R1 (post-INDEX-swap pre-flight) and R2 (synthetic constant) were caught pre-execution; PATCH 1 is caught mid-verification. PM owes the project a small post-mortem when ADR-015 closes — the original dispatch could have been more carefully reviewed before going out, and the original ADR could have specified exclusive-end semantics rather than letting the implementation interpret. That post-mortem is separate from this patch and will land in the broader post-ADR-015 planning conversation tomorrow.

---

**End of dispatch.**
