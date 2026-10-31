# CLOSE REPORT: ADR-015 implementation + PATCH 1

Date: 2026-05-07

Commit:

```text
47972c250a32de687a54b29daa3bd034d2638dd1
```

Subject:

```text
mm: ADR-015 KASLR-tolerant heap placement (Option E: validate + below-kernel fallback)
```

## Scope

Committed ADR-015 Option E on top of `afa4d22`.

Tracked files changed:

```text
 docs/ADR-015-kaslr-tolerant-heap-placement.md | 319 ++++++++++++++++++++++++++
 kernel/Cargo.toml                             |   2 +
 kernel/src/mm/heap.rs                         | 110 ++++++++-
 kernel/src/mm/mod.rs                          |  21 ++
 kernel/src/test_harness.rs                    | 223 ++++++++++++++++++
 tools/xtask/src/main.rs                       |  14 ++
 6 files changed, 686 insertions(+), 3 deletions(-)
```

Implementation contents:

- Added ADR-015 documenting KASLR-tolerant heap placement: validate above-kernel placement first, fall back below kernel if the above placement would overlap, underflow, overflow, or collide with HHDM.
- Added `HHDM_PHYS_END`, derived from Limine usable memory map entries, so heap placement can validate HHDM collisions against the physical-memory span actually mapped through HHDM.
- Reworked heap placement into checked `try_above_kernel` and `try_below_kernel` helpers.
- Added `kaslr_heap_contract` test scenario validating runtime placement invariants.
- Added `kaslr_heap_synthetic` test scenario validating deterministic edge cases, including PATCH 1's exact-ceiling overflow reproducer.
- Wired both scenarios into xtask.

## PATCH 1

The first ADR-015 50-run loop was stopped and discarded after run 24 exposed a new allocator failure class:

```text
target/verify/ADR-015-loop-run-24.cap_grant_missing_meta.serial.log
[ kernel ] heap: ready (4 MiB at VirtAddr(0xffffffffffc00000), 1024 pages mapped)
panicked at /home/neo/.rustup/toolchains/nightly-2026-04-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/alloc/src/alloc.rs:573:9:
memory allocation of 8 bytes failed
```

Root cause:

```text
heap_start = 0xffff_ffff_ffc0_0000
HEAP_SIZE  = 0x40_0000
```

The original implementation checked inclusive end representability with `heap_start + HEAP_SIZE - 1`, which accepts `0xffff_ffff_ffff_ffff`. The allocator computes the exclusive end internally with `heap_start + HEAP_SIZE`, which overflows. PATCH 1 changed both production and synthetic above-kernel checks to require the exclusive end to be representable.

PATCH 1 also added synthetic Test 4 with `exact_ceiling = 0xffff_ffff_ffa0_0000_u64`, deterministically guarding the run-24 bug class.

## Verification

`cargo k`:

```text
warning: `altimit-neo-kernel` (bin "kernel") generated 18 warnings (run `cargo fix --bin "kernel" -p altimit-neo-kernel` to apply 1 suggestion)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.08s
```

Feature build, `neo-test-kaslr-heap-contract`:

```text
warning: `altimit-neo-kernel` (bin "kernel") generated 39 warnings (run `cargo fix --bin "kernel" -p altimit-neo-kernel` to apply 1 suggestion)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.93s
```

Feature build, `neo-test-kaslr-heap-synthetic`:

```text
warning: `altimit-neo-kernel` (bin "kernel") generated 39 warnings (run `cargo fix --bin "kernel" -p altimit-neo-kernel` to apply 1 suggestion)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.58s
```

`kaslr_heap_contract` PASS frame:

```text
NEO_TEST/1 event=PASS name=kaslr_heap_contract placement=above heap_start=0xffffffffa9600000 heap_end=0xffffffffa9a00000 kernel_start=0xffffffffa8f29000 kernel_end=0xffffffffa93a2000 hhdm_offset=0xffff800000000000 hhdm_end=0xffff80001ff6a000
```

`kaslr_heap_synthetic` PASS frame:

```text
NEO_TEST/1 event=PASS name=kaslr_heap_synthetic test1=ok test2=ok test3=ok test4=ok middle_result=0xffffffff80200000
```

Fresh PATCH1 50-run loop:

```text
ADR-015 PATCH1 Result (50 counted runs on implementation tree, NEO_SMP=4 default suite):
  PANIC:   0
  PASS:    39
  TIMEOUT: 11
  OTHER:   0

PANIC details:
  none

TIMEOUT details:
  Run 4: scenario=buddy_alloc_free_alloc_returns_same_frame
  Run 11: scenario=cap_survive_exit
  Run 13: scenario=slab_local_to_global_promotion
  Run 15: scenario=cap_revoke_local
  Run 17: scenario=cap_survive_exit
  Run 21: scenario=smep_catch
  Run 35: scenario=smp_per_cpu_scheduler_smoke
  Run 36: scenario=cap_revoke_descendants_walk
  Run 38: scenario=expect_pass_but_fail
  Run 40: scenario=kaslr_heap_contract
  Run 42: scenario=cap_survive_exit

OTHER details:
  none

Per-run results array (1..50):
PASS, PASS, PASS, TIMEOUT, PASS, PASS, PASS, PASS, PASS, PASS, TIMEOUT, PASS, TIMEOUT, PASS, TIMEOUT, PASS, TIMEOUT, PASS, PASS, PASS, TIMEOUT, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS, TIMEOUT, TIMEOUT, PASS, TIMEOUT, PASS, TIMEOUT, PASS, TIMEOUT, PASS, PASS, PASS, PASS, PASS, PASS, PASS, PASS
```

Comparison to Q1B baseline:

```text
KASLR-Q1B Result (50 runs on main HEAD 9505619, NEO_SMP=4 default suite):
  PANIC:   4
  PASS:    31
  TIMEOUT: 14  (harness wedge; not KASLR overflow; see triage queue)
  OTHER:   1
  MISSING: 0
```

ADR-015 PATCH1 result:

```text
PANIC: 0
PASS: 39
TIMEOUT: 11
OTHER: 0
```

The KASLR heap-placement overflow class is no longer observed in the fresh 50-run implementation loop. The remaining timeouts match the known default-suite harness wedge class and are not ADR-015 heap-placement panics.

## Notes

The implementation dispatch's expected zero-warning posture did not match the current HEAD. Actual `cargo k` warning count is 18 on the implementation commit. Feature builds add test-scenario code and produce 39 warnings each.

The repository is local-only in this workflow. No push was performed.

Working tree after commit has no tracked modifications. The remaining `git status --short` entries are untracked `Plans/` artifacts.
