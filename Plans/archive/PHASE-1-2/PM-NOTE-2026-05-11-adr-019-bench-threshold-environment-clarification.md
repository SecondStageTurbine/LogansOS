# PM-NOTE — ADR-019 sub-phase A bench threshold: environment clarification (50ns → QEMU-debug regression ceiling)

**Date filed:** 2026-05-11 (early morning, post-Jarvis kernel-rename bundle stop at sub-commit 3)
**Author:** PM
**Type:** PM-NOTE. Operational tuning decision. Doc-only artifact. **No ADR-019 text amendment** — the architectural threshold (50ns acquire/release overhead vs `spin::Mutex`) stands as the native-hardware design target. This PM-NOTE clarifies the operational interpretation of that threshold under the QEMU TCG debug-build test environment and authorizes the bench code to use an environment-appropriate regression-detector ceiling instead of the architectural target as the test gate.
**Authority:** PM (operational tuning). PM operational posture per the PL clarification 2026-05-10 memory entry: PM owns operational/technical decisions; PL-tier reserved for bible amendments, scope, major architectural pivots. Threshold tuning for a regression detector under a specific test environment is operational. PL may override if it should rise to ADR amendment level.
**Trigger:** Jarvis kernel-rename bundle stopped at sub-commit 3 per `Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-sub-commit-3.md`. Bench measured `overhead_ns=3085 threshold_ns=50` after three optimization iterations (4377 → 3139 → 3085 ns floor). Jarvis triage correctly surfaced the three options for PM resolution: (a) does 50ns threshold apply to debug QEMU numbers; (b) should bench run under optimized kernel build; (c) does sub-phase A need different API/measurement strategy.

---

## 1. The numbers

From `Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-sub-commit-3.md`, final iteration after Jarvis applied two optimizations (constant `lock_site!()` instead of runtime `Location::caller()` hashing; forced inlining on lock/owner hot path):

```text
spin_mean_ns      = 463
irqsafe_mean_ns   = 3548
switch_mean_ns    = 1837
overhead_ns       = 3085   (irqsafe_mean - spin_mean)
threshold_ns      = 50
```

Jarvis's three iterations:

| Iteration | Optimization | spin_mean | irqsafe_mean | switch_mean | overhead | Reduction |
|---|---|---|---|---|---|---|
| 1 | Initial implementation | 464 ns | 4841 ns | 2339 ns | 4377 ns | — |
| 2 | Constant `lock_site!()` (no runtime Location::caller hashing) | 486 ns | 3625 ns | 1535 ns | 3139 ns | 28% |
| 3 | + forced inlining on lock/owner hot path | 463 ns | 3548 ns | 1837 ns | 3085 ns | 1.7% additional |

Floor at ~3085ns. Two implementation optimizations exhausted the obvious code-path gains. The remaining cost is environmental, not implementation.

## 2. Why 3085ns and not 50ns

### 2.1 The 50ns target was theoretically grounded for native hardware

Native x86_64 IF-toggle theoretical cost:

- `cli` / `sti` (clear/set interrupts): ~3-5 cycles each on Skylake+ (Intel Optimization Manual)
- `pushfq` / `popfq` (save/restore RFLAGS): ~1-3 cycles each, though `popfq` can serialize the pipeline in some microarchitectures
- Memory ordering: implicit; no explicit fence required for IF-toggle on x86

Total native instruction cost: ~10-20 cycles ≈ **3-7 ns at 3 GHz**.

The ADR-019 Q4 threshold of 50ns gives **7-15x headroom** over the native theoretical minimum — appropriate engineering margin for production hardware with predictable performance characteristics. The threshold was set correctly *for its intended measurement environment* (production hardware).

### 2.2 QEMU TCG debug-build is a fundamentally different measurement environment

The test infrastructure runs under QEMU with the Tiny Code Generator (TCG) software emulation, not under KVM hardware acceleration. The kernel is built in debug mode (no inlining of `spin::Mutex` methods either; no LLVM optimization passes).

Under this configuration:

- `cli` / `sti` are software-emulated by QEMU; each instruction goes through TCG's translation cache and dispatches a helper function in QEMU's runtime. Single-digit-cycle native instructions become microsecond-scale emulated operations.
- `pushfq` / `popfq` similarly emulated; the RFLAGS register state is reconstructed in QEMU's vCPU state, not directly manipulated.
- Debug build means no function-call inlining, so every `arch::without_interrupts` or guard-drop traverses multiple frame setups.
- The benchmark loop itself runs under the same emulation/debug overhead in both baseline (`spin::Mutex`) and `IrqSafeMutex` arms — but the IF-toggle path has substantially more emulation surface than the baseline spin path.

The diagnostic signal in the numbers: `spin_mean_ns = 463` for a contention-free spin lock is itself wildly above native hardware (~5-30 ns). That's ~430ns of pure measurement/emulation overhead in *every* lock acquisition, regardless of IRQ-off semantics. The QEMU debug environment shifts the entire performance baseline up by ~2 orders of magnitude.

### 2.3 The decisive diagnostic: SwitchMutex is FASTER than IrqSafeMutex under QEMU

This is the load-bearing signal in Jarvis's data:

- `switch_mean_ns = 1837` — SwitchMutex (which does LockOwner bookkeeping but NO IF-toggle; lock used in cooperative-switch context only)
- `irqsafe_mean_ns = 3548` — IrqSafeMutex (LockOwner bookkeeping + IF-toggle)

**SwitchMutex overhead vs spin:** 1837 - 463 = **1374 ns** (this is LockOwner bookkeeping cost under QEMU debug).
**IrqSafeMutex overhead vs spin:** 3548 - 463 = **3085 ns** (LockOwner + IF-toggle cost).
**IF-toggle alone:** 3085 - 1374 = **1711 ns under QEMU TCG debug**.

So the IF-toggle path alone costs ~1711ns under QEMU TCG debug — versus ~3-7ns native theoretical. That's a ~250-500x emulation multiplier on the specific cli/sti/pushfq/popfq instructions, which is consistent with general QEMU TCG behavior for privileged instructions that QEMU treats with extra care.

**Conclusion:** the 50ns architectural threshold is unmeasurable under QEMU TCG debug build. It is not a question of needing more implementation optimization; the underlying emulation is the floor.

## 3. The operational call

**Threshold revision for the bench code:** **6000 ns** as the QEMU-debug regression ceiling.

Reasoning:

- Observed floor with all available implementation optimizations applied: 3085 ns.
- Run-to-run variance under QEMU TCG can be substantial; tail values in Jarvis's data showed irqsafe_max_cycles up to ~949774 (~26.6 µs). Mean is reasonably stable, but a single bench run might drift 10-20% on QEMU scheduling noise.
- **6000 ns gives ~95% headroom over the observed floor.** A future implementation change that doubles the IrqSafeMutex overhead (e.g., adding a serializing fence, adding an extra atomic op, breaking inlining) would push it above 6000ns and trip the bench. That's exactly the regression-detection signal we want.
- Catastrophic regressions (10x slowdown) would obviously trip. Minor regressions (~30% slowdown to ~4000ns) would not trip but would be measurable in the reported numbers — the bench output still emits `spin_mean_ns`, `irqsafe_mean_ns`, `overhead_ns` for human review even when it passes.

**This is a regression detector, not an absolute-performance gate.** The naming/framing in the bench code should reflect that.

**The architectural 50ns target stays in ADR-019 as the native-hardware design intent** — that's what the implementation should aim for on bare-metal production hardware once we have bare-metal CI. The bench under QEMU debug is a stand-in that detects implementation regressions; absolute performance characterization is deferred to Phase 2+ when bare-metal CI exists.

### 3.1 Why not the other two options Jarvis triaged

**Option (b) — run bench under optimized kernel build.** Would partially close the gap (debug → release inlining recovers some perf), but the underlying QEMU TCG emulation of cli/sti/pushfq/popfq stays expensive regardless of kernel build mode. Adding release-mode QEMU runs to the test infrastructure is significant work (longer build times, separate test target, separate verification log path) for marginal benefit. **Defer to Phase 2+ if bench fidelity becomes blocking; not justified now.**

**Option (c) — different API/measurement strategy.** Could split the bench into "lock overhead" + "IRQ-save/restore overhead" components separately. Could use rdtsc-based cycle counts with statistical filtering. Both add complexity for marginal benefit; the current bench correctly distinguishes IrqSafeMutex from spin and from SwitchMutex, which is all the regression detector needs to do. **Defer indefinitely; not justified now.**

**Option (a) chosen** — threshold revision to environment-appropriate value.

### 3.2 Why this is PM-tier, not PL-tier

- ADR-019's text (the 50ns architectural target) is **unchanged** by this PM-NOTE. The architectural intent stands.
- The bench code's threshold constant is operational tuning of a test-infrastructure parameter, comparable to deciding how many iterations a bench runs or what timeout a test scenario uses.
- If PL believes the ADR-019 text should be amended to add a "measurement environment clarification" subsection (e.g., "Q4 measurement environment: native production hardware; the QEMU debug test environment uses a regression-detector ceiling per `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md`"), that's a follow-up PL-tier amendment and is welcome but does not gate sub-commit 3 landing.

## 4. Sub-commit 3 path forward

**Jarvis-next dispatch:** `Plans/DISPATCH-2026-05-11-sub-commit-3-irqsafe-mutex-finalize-jarvis.md` (filed alongside this PM-NOTE).

Scope of Jarvis-next dispatch:

1. Apply the threshold revision in the bench code: 50ns → 6000ns ceiling. Update the threshold comment in `kernel/src/test_harness.rs` (or wherever the `irqsafe_mutex_bench` threshold is encoded) to cite this PM-NOTE and explain the regression-detector framing.
2. Re-run the verification gates: `cargo k`, `cargo k --no-default-features`, `cargo neo-test irqsafe_mutex_if_restore`, `cargo neo-test irqsafe_mutex_bench`, `cargo neo-test all`.
3. Commit sub-commit 3 — title and message should reflect the revised threshold and cite ADR-019 + this PM-NOTE.
4. Push to origin/main (fast-forward).
5. Report back to PL via the standard closure protocol.

**Optimization work already in the working tree stays.** The two implementation optimizations Jarvis already applied (constant `lock_site!()`, forced inlining) are real improvements that should land regardless of threshold revision — they reduced overhead by ~30% and represent the actual implementation quality the production code should ship with.

**No re-architecture, no API change.** The IrqSafeMutex implementation as Jarvis wrote it is correct (functional test `irqsafe_mutex_if_restore` passes); only the bench's pass/fail criterion changes.

## 5. Forward markers

**5.1 Bare-metal CI is a Phase 2+ deferred item.** When NeoPlutarch acquires the ability to run kernel tests on actual hardware (not QEMU), the bench should be re-run there to characterize native-hardware IF-toggle cost. Expected outcome: under 50ns, validating the ADR-019 architectural target empirically. If above 50ns on bare metal, the architectural target needs amendment — that would be a PL-tier ADR-019 revision at that point.

**5.2 ADR-019 sub-phase B and beyond.** Sub-phase A (introducing IrqSafeMutex as a sibling type to SwitchMutex) is the foundation. Subsequent sub-phases (per ADR-019 §4) migrate specific call-sites to IrqSafeMutex where IRQ-off semantics are required. Those migrations should ride against the QEMU-debug regression bench at 6000ns ceiling; if a migration trips the bench, that's a real implementation regression to investigate.

**5.3 If the SwitchMutex baseline ever changes substantially.** The threshold derivation here uses `IrqSafeMutex_overhead - SwitchMutex_overhead = 1711 ns` as the IF-toggle-alone cost decomposition. If ADR-020 LockOwner work or other SwitchMutex changes substantially shift `switch_mean_ns`, the threshold may need recalibration. Currently both SwitchMutex (1837 ns) and IrqSafeMutex (3548 ns) are well below the proposed 6000 ns ceiling; threshold has natural headroom for moderate baseline drift.

**5.4 Inference scheduler will face the same environment issue.** ANSPEC §7.6 describes the inference scheduler as a sibling to the main thread scheduler operating at op-batch granularity (~10ms quanta). When that scheduler lands, its perf tests will face the same QEMU TCG measurement environment. The pattern this PM-NOTE establishes (architectural target in ADR/spec; regression detector in bench code; native characterization deferred to bare-metal CI) is reusable.

## 6. Cross-references

**Triggered by:**
- `Plans/JARVIS-FAILURE-2026-05-10-kernel-bundle-sub-commit-3.md` — Jarvis failure report with three bench iterations and triage notes.

**Architectural anchors (unchanged):**
- `docs/adr/ADR-019-switchmutex-irq-off-by-construction.md` — Q4 threshold (50 ns acquire/release overhead) stands as the native-hardware design target.
- `docs/adr/ADR-020-owner-metadata-as-production-diagnostic.md` — LockOwner bookkeeping cost is a known baseline component (1374 ns under QEMU debug, captured in §2.3 decomposition).

**Operational discipline:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.1 (Jarvis kernel lane), §3.1 (PM treats first 2-3 Jarvis kernel dispatches as calibration data).
- PL operational posture clarification (2026-05-10 memory entry): PM owns operational/technical decisions; PL-tier reserved for bible amendments, scope, major architectural pivots.

**Follow-up artifacts:**
- `Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md` — calibration record on Jarvis's stop-discipline performance under the new roster (filed alongside this PM-NOTE).
- `Plans/DISPATCH-2026-05-11-sub-commit-3-irqsafe-mutex-finalize-jarvis.md` — Jarvis-next dispatch to apply threshold revision and land sub-commit 3 (filed alongside this PM-NOTE).

**Already landed (post-rebase, on origin/main):**
- `053a5bc kernel: rename AltimitNeo → NeoPlutarch; A3 boot identity` (rebased from 33152a9).
- `de8965c kernel/sync: add LockOwner field to SwitchMutex (ADR-020 foundation)` (rebased from 481217a).

---

## 7. Closure

The 50ns architectural threshold in ADR-019 Q4 is correct for native hardware; the QEMU TCG debug measurement environment cannot resolve at that scale (IF-toggle alone costs ~1711 ns under emulation; baseline lock acquisition costs ~463 ns; both 2-3 orders of magnitude above native). Threshold revised to **6000 ns** as a regression-detector ceiling under the QEMU debug environment, with ~95% headroom over the observed floor (3085 ns) and clear catastrophic-regression detection. ADR-019 text unchanged; native-hardware absolute-performance characterization deferred to Phase 2+ bare-metal CI. Sub-commit 3 unblocked.

Jarvis-next dispatch follows.

---

**End of PM-NOTE.**

*Bench threshold revised 50 ns → 6000 ns as QEMU-debug regression ceiling. ADR-019 architectural target unchanged. Native characterization deferred to bare-metal CI. Sub-commit 3 unblocked.*
