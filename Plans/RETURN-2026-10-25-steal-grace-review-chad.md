<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #52): time-based matmul steal grace (#116)

**Overall verdict: FAIL.** Checklist items **3, 4, and 6 fail**; items **1, 2, 5, 7, 8, and 9 pass**.

Reviewed commit: `2817bbaa2338f910c8787162a5309e0220b1e543`.
Branch: `codex/steal-grace-review-chad`.
Scope: review only; no source changes, build, or new test execution.
Read the full dispatch from `origin/main`, CP-20/CP-25/CP-26/CP-80, the requested diff from
`f450f4f9`, and the surrounding executor, timer calibration, instrumentation, and CI paths.
RAG health was **healthy**, with Ollama and Qdrant both true; used it for discovery and checked
the actual files at the reviewed commit for all conclusions.

The single-winner claim and lifetime barrier remain intact. The confirmed failures concern
the grace's arithmetic domain, its publication ordering guarantee, and missing runtime coverage.
No duplicate executor, output corruption, or claimed-slot abandonment was found in this change.
Per the dispatch's post-merge policy, this is a confirmed FAIL for the PL's revert-first handling;
this review branch contains only this RETURN.

## Checklist

| # | Verdict | Enforcing code / evidence at the reviewed commit |
|---|---|---|
| 1 | **PASS** | `kernel/src/ai/par_matmul_logic.rs:53` permits Steal only for pending slots after grace; `kernel/src/ai/par_matmul.rs:481` (BSP) and `kernel/src/ai/par_matmul.rs:554` (AP) use the same `compare_exchange(true, false, AcqRel, Acquire)`. Only a successful CAS reaches either execution path (`:486`, `:567`). Stale pending observations cannot grant execution. |
| 2 | **PASS** | `kernel/src/ai/par_matmul_logic.rs:62` routes claimed, unfinished slots to Spin or Halt, independently of grace. `kernel/src/ai/par_matmul.rs:471` supplies `spins >= BARRIER_LIMIT`; `:516` wires Halt to panic, rather than a return. `:459` acquires completion before reuse, and `:535` releases PAR_BUSY only after every slot drains. |
| 3 | **FAIL** | `kernel/src/ai/par_matmul_logic.rs:88` divides before multiplying and has no positive minimum threshold. The rate producer has no lower bound: `kernel/src/arch/x86_64/lapic_timer_logic.rs:23`, `:203`, `:238`; the accepted rate is published without clamping at `kernel/src/arch/x86_64/lapic_timer.rs:399`, `:424`. Concrete input and the passing overflow/wrap/fallback subchecks are below. |
| 4 | **FAIL** | `kernel/src/ai/par_matmul.rs:419` precedes `:422` in source, but the helper at `:102` is a bare `_rdtsc()` (`:104`), with no store visibility fence or instruction serialization. A Release store does not establish that the subsequent timestamp follows its global visibility. Concrete CPU sequence below. |
| 5 | **PASS** | For an absent/busy AP with an advancing calibrated TSC, finite grace eventually produces Steal (`kernel/src/ai/par_matmul_logic.rs:57`); unknown rate uses finite spin grace (`:85`). BSP wins the CAS and computes inline at `kernel/src/ai/par_matmul.rs:481`, `:489`, then retires at `:496`. A late AP loses its CAS at `:554` and returns at `:557`. If AP wins first, BSP waits for its completion or fail-stops. No new circular wait or retry-only cycle is introduced by shortening grace. |
| 6 | **FAIL** | Output computation is structurally unchanged: BSP/AP use `run_disjoint` (`kernel/src/ai/par_matmul.rs:489`, `:571`), with independent columns (`kernel/src/ai/forward_logic.rs:1310`, `:1411`, `:1455`) and Release/Acquire completion (`par_matmul.rs:575`, `:459`). However, the required parity run **with the feature on is absent**: `kernel/Cargo.toml:1044`, `tools/xtask/src/main.rs:4973`, `.github/workflows/ci.yml:343`. Scenario and actual CI evidence below. |
| 7 | **PASS** | `kernel/src/ai/prefill_perf.rs:231` / `:241` read CPU/TSC/MSRs and update atomic per-CPU counters; `:92` uses relaxed fetch_add, `:101` uses a per-CPU atomic capability cache. There is no lock, yield, or park. Hooks bracket BSP partition 0 (`kernel/src/ai/par_matmul.rs:429`, `:433`), stolen partitions (`:488`, `:491`), and the AP path **after successful claim and before done Release** (`:570`, `:573`, `:575`). Their elapsed overhead does not change ownership or the join's synchronization protocol. |
| 8 | **PASS** | Local CPUID is obtained at `kernel/src/arch/x86_64/cpufreq.rs:28`; masks are decoded at `kernel/src/arch/x86_64/cpufreq_logic.rs:32`. APERF/MPERF reads are conditional (`cpufreq.rs:47`), PERF_STATUS is gated by EIST (`:64`), and PM_ENABLE by HWP (`:69`, `:75`). Diagnostic HWP_CAPABILITIES/HWP_REQUEST reads require PM_ENABLE bit 0 (`:79`). The per-CPU APERF cache originates from that CPU's CPUID (`prefill_perf.rs:104`), including the AP partition path. Both writes are inside the `hwp-performance` function cfg (`cpufreq.rs:110`, `:121`, `:128`); the opt-in path establishes HWP enablement before reading capabilities. The per-CPU startup call is cfg-gated (`kernel/src/arch/x86_64/mod.rs:216`). Image A's list (`tools/xtask/src/main.rs:4869`) omits HWP; only the explicit flag appends it (`:4930`). |
| 9 | **PASS** | `kernel/src/ai/par_matmul.rs:469` selects exactly `spins >= STEAL_GRACE_SPINS` when the feature is off (`:470`), matching the old argument in the requested diff. The timestamp and time helper call are separately cfg-gated (`:421`, `:461`); no time-based decision remains on that branch. |

## Confirmed findings

### F1 — P3: accepted nonzero rate can collapse the grace to zero (item 3)

Location: `kernel/src/ai/par_matmul_logic.rs:88`.

Concrete input accepted by the rate producer: CPUID leaf 0x15 `(EAX, EBX, ECX)` =
`(48, 1, 24_000_000)`. `tsc_hz_from_leaf15` returns `Some(500_000)` at
`kernel/src/arch/x86_64/lapic_timer_logic.rs:27`. With no usable independent reference-clock
measurement, `choose_rate(Some(500_000), &[], ...)` returns Calibrated at `:238`.
`lapic_timer.rs:399` / `:424` publish 500,000 as TSC_HZ. `TSC_HZ_BOUND` only clamps the
separate **budget** rate (`lapic_timer_logic.rs:84`), not `tsc_hz()`.

Then `past_steal_grace(0, 500_000, 0, 50, 2_000_000)` returns true:
`500_000 / 1_000_000 * 50 == 0`, although 50 us at that rate is 25 cycles.
Thus an unclaimed slot can be stolen on the first barrier observation without any grace.
I checked this arithmetic directly and traced the value through the calibration/publish code.
This is a rate-domain edge case; no claim that the Zenbook's measured rate is sub-MHz.
The checklist explicitly asks about any rate this producer can return, and the code enforces
no >=1 MHz precondition.

Other parts of item 3 **pass**:

- At the actual `grace_us = 50`, even `u64::MAX / 1_000_000 * 50` fits u64;
  the maximum threshold is 922,337,203,685,450 cycles. Arbitrary caller-provided grace_us
  values are outside this executor's fixed-50-us use.
- For same-CPU monotonic reads and an interval shorter than a full 64-bit counter period,
  `wrapping_sub` at `par_matmul.rs:463` correctly handles a genuine wrap. For example,
  published = `u64::MAX - 9`, now = 15 gives 25 elapsed cycles, not a huge elapsed value.
  Wrap alone cannot trigger an early steal under those premises.
- `tsc_hz == 0` takes the exact prior spin comparison at `par_matmul_logic.rs:85`.
- At ordinary nonintegral-MHz rates, division also rounds the grace downward by less than
  50 cycles; that is separate from the zero-threshold counterexample.

Suggested follow-up: form the scaled threshold in u128 before dividing, round upward for
a minimum-duration promise, and retain the zero-rate spin fallback. Cover sub-MHz and
nonintegral-MHz rates as well as the existing 3 GHz boundary KAT.

### F2 — P2: the publish timestamp does not wait for publication visibility (item 4)

Locations: `kernel/src/ai/par_matmul.rs:104`, `:419`, `:422`.

Concrete permitted sequence:

1. CPU0/BSP issues the final `pending.store(true, Release)`; its store remains buffered
   while that cache line is obtained for writing. CPU1/AP can still observe pending=false.
2. CPU0 executes the bare RDTSC and records `published` before the pending store becomes
   globally visible. RDTSC does not wait for prior stores, and Release is not a store-buffer drain.
3. The pending store becomes visible and CPU1 can first see the job after the grace has
   already started. A later BSP check measures from the earlier timestamp and may claim
   after less than 50 us of actual publication visibility.

This disproves the requested ordering guarantee even though the source statements are
ordered. It affects the amount of AP opportunity; the shared claim CAS still protects output
ownership. No 50-us cache stall is required for the guarantee to fail: any delayed visibility
shortens the promised window by that delay. This is an architectural ordering counterexample,
not a claim of a measured failure on this machine.

Intel's RDTSC reference states that it is not serializing and describes MFENCE followed by
LFENCE before RDTSC when earlier loads **and stores** must be globally visible. See
[Intel SDM Volume 2, RDTSC, Vol. 2B 4-551](https://cdrdv2-public.intel.com/774492/325383-sdm-vol-2abcd.pdf).
Suggested follow-up: provide the required store-visibility and timestamp ordering in the
publish helper. Merely reusing the LFENCE-only timing reader at
`kernel/src/ai/forward_perf.rs:53` does not establish prior-store visibility.

### F3 — P2: the cited parity gate runs the old grace branch (item 6)

Locations: `kernel/Cargo.toml:1044`, `.github/workflows/ci.yml:343`,
`tools/xtask/src/main.rs:4973`.

The scenario is `threaded_matmul_parity` (`tools/xtask/src/main.rs:3102`). It exercises
1/2/4 partitions on deterministic 1024-column Q4_K and Q8_0 inputs, compares f32 bits against
the whole-matrix oracle (`kernel/src/test_harness.rs:23082`, `:23100`, `:23160`, `:23178`),
and checks that a boundary-dependent perturbation is detected and parity restored after
disarming (`:23282`, `:23296`, `:23299`). Completion acquires the AP's output writes.

However, `neo-test-threaded-matmul-parity` resolves only `neo-test` and `threaded-matmul`
at `kernel/Cargo.toml:1044`. Kernel defaults at `:77` do not add the time feature.
`build_test_kernel` starts from that scenario feature at `tools/xtask/src/main.rs:4973`;
operator additions require NEO_TEST_EXTRA_FEATURES (`:4986`). The QEMU CI invocation passes
NEO_SMP=4 but no such addition (`.github/workflows/ci.yml:336` through `:345`).

Concrete witness: the cited [CI run 40149510145, QEMU shard 4/4](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40149510145/job/121051192819)
reports `threaded_matmul_parity` PASS at 2026-10-25T02:39:19Z. I inspected its recorded command,
scenario selection, and result. Its build resolves the **feature-off** branch at
`par_matmul.rs:470`. Therefore this green result cannot prove the feature-on claim.
The run metadata confirms all 23 jobs succeeded on `0c7008232faddea9668e673430a81829583f7606`,
as cited by the dispatch.

The only CI matrix entries containing `matmul-steal-grace-time` are product-real-lm and
product-real-lm-hwp (`.github/workflows/ci.yml:819`, `:822`); their action is clippy (`:883`,
`:887`), not the parity scenario. The product build also compiles the image's feature set,
but supplies no feature-on runtime parity witness. CP-80's claim of QEMU parity with this
feature on (`docs/concurrency-pitfalls.md:3628`) is unsupported by the checked-in CI.

Suggested follow-up: add an explicit feature-on parity CI invocation with NEO_SMP=4 and
NEO_TEST_EXTRA_FEATURES=matmul-steal-grace-time, retaining feature-off coverage. Assert or
report the calibrated path as well: enabling the feature alone may still exercise the
spin fallback if the QEMU CPU has no trusted TSC calibration. Deterministically covering
both AP execution and BSP stealing would strengthen the same oracle; the current scenario
reports executor counters rather than requiring both outcomes.

## Handoff and local workspace

No source fix or revert was performed: dispatch requests one pushed review document.
The initial `.gitignore` modification and untracked `.ignore` were preserved in the named
stash `chad preexisting graft ignore files before steal grace review` before checkout.
Local generated graft cards were also preserved in a separate named stash to keep this
review branch clean; no local cache/card files are part of the RETURN commit.

Validation was source review, arithmetic inspection, RAG health/search, Intel's architectural
reference, and read-only inspection of existing CI metadata/logs. No build or test suite was run.
