<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #53): steal-grace fix-forward re-check

Reviewed SHA: `ae6231a086047b883b8db88471fd2aee3451ea02` (PR #117 squash).
Branch: `codex/steal-grace-recheck-chad`.
Scope: relay #52 items 3, 4 and 6 only; review only, no source edits or build/test execution.

| Item | Verdict | Evidence at reviewed SHA |
|---|---|---|
| R3 | **CLOSED / PASS** | `kernel/src/ai/par_matmul_logic.rs:96` multiplies two u64 inputs in u128, rounds upward with `div_ceil(1_000_000)` at `:98`, and saturates at `:100`. For grace_us=50, every positive rate yields at least one cycle; 500 kHz yields 25, 2,399,999,999 Hz yields 120,000, 3 GHz yields 150,000, and u64::MAX yields 922,337,203,685,478. Even the product of two u64::MAX inputs fits u128. `past_steal_grace` preserves the exact zero-rate spin fallback at `:85`. The shared-source host KAT (`tools/ai-ops-tests/src/lib.rs:69`, `:9906`) covers all four required rates, sub-MHz boundaries, saturation, and 1 Hz; the fallback KAT is at `:9898`. |
| R4 | **CLOSED / PASS** | `kernel/src/ai/par_matmul.rs:122` emits MFENCE, LFENCE, RDTSC in that order (`:123` through `:125`), in one asm block without `nomem`/`readonly`. Every active AP slot's pending Release store occurs inside the publication loop at `:504`; the sole publish-stamp call is after that loop at `:507`, before BSP work and outside all polling loops. This supplies the store-visibility ordering missing in relay #52, once per parallel job. |
| R6 | **STILL OPEN / FAIL (timing discriminator)** | Feature-on CI execution, forced BSP stealing, parity, and scenario-only overrides are present (details below). However, `kernel/src/ai/par_matmul.rs:585` timestamps **after** the stolen work executes at `:577`; `kernel/src/test_harness.rs:23484` compares this completion timestamp with grace. It does not assert that the steal/claim happened at or after grace. |

## R6: verified improvements and remaining gap

CI selection is now explicit through the default scenario registry:
`tools/xtask/src/main.rs:3112` names `threaded_matmul_parity_grace_time`, maps it to its scenario
feature at `:3113`, and sets ignored=false at `:3115`. Default selection includes nonignored
scenarios at `:7074`; build features include scenario.feature at `:4983`.
`.github/workflows/ci.yml:343` supplies NEO_SMP=4 and `:345` invokes the four-way sharded suite.
`kernel/Cargo.toml:1049` makes the scenario pull in both the existing parity scenario and
`matmul-steal-grace-time` (plus lapic-timer).

I inspected [run 40163944426, QEMU shard 3/4](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40163944426/job/121081438135).
Its recorded PASS line is:

```text
2026-10-25T05:49:36.0150234Z [xtask] neo-test: `threaded_matmul_parity_grace_time` PASS
```

The run metadata reports success for all 23 jobs on
`d0265cded695c71f95d1d14c93cf08f1a492ec78`. I fetched that object and verified there is no diff
against the reviewed squash in `kernel`, `tools`, or `.github`.

The scenario sets 1 GHz / 500,000 us at `kernel/src/test_harness.rs:23408`, builds a whole-matrix
oracle at `:23430`, holds AP slot 1 at `:23444`, resets counters at `:23445`, and runs the
parallel executor at `:23448`. The AP skips the held slot at `kernel/src/ai/par_matmul.rs:638`.
Output f32 bits are compared at `kernel/src/test_harness.rs:23464`, with failure at `:23478`;
a zero steal count fails at `:23481`. TEST_TSC_HZ, TEST_GRACE_US, the setter and the override
read are all gated by `neo-test-threaded-matmul-parity-grace-time` at
`kernel/src/ai/par_matmul.rs:156`, `:158`, `:161`, and `:172`. Builds without that scenario
feature have no override storage/setter/read path.

**Remaining concrete false-pass sequence:** the requested grace is 500,000,000 cycles. Suppose
an early-steal regression allows the held slot's successful claim CAS (`par_matmul.rs:569`)
at elapsed 499,999,999 cycles. Its inline computation (`:577`) completes at elapsed
510,000,000 cycles, with correct output, and the other AP slots have already completed.
The value recorded at `:585` is then 510,000,000; steals is positive, output matches, and
`test_harness.rs:23484` passes despite the claim occurring before grace. This is a discriminator
counterexample, not an observed early steal in the corrected production decision at
`par_matmul_logic.rs:88`. A later stolen slot can also overwrite the earlier timestamp.

To close the remaining R6 requirement, capture the elapsed value used for the eligibility
decision (before the claim and before computation), and assert the held slot's value, or the
minimum across all successful BSP steals, against the expected grace. Recording only after
execution cannot establish the requested lower bound on the steal itself. The gate should
also assert the independent expected 500,000,000-cycle threshold: its current expected grace
comes from the same production helper (`test_harness.rs:23409`), so a helper regression to zero
would make the `waited < grace` check vacuous.

## Validation and workspace

Read the dispatch, prior RETURN, requested diff from `0df36d09`, relevant executor/helper/KAT,
scenario, feature, and CI selection code, and the existing CI metadata/logs. No build or tests
were run locally. RAG health was healthy (ollama=true, qdrant=true); all reviewed paths were
already supplied by the dispatch/diff, so direct reads and exact-symbol searches sufficed.

Preexisting untracked graft cache files were preserved in the named stash
`chad preexisting graft cache before relay 53 recheck`; they are not part of this commit.
This branch adds only this RETURN. Final handoff checks require an empty
`git status --porcelain` and no commits in `git log '@{u}..'` after push.
