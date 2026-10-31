# PM Note: SC-3B Phase B outcome — ranking-with-data update + closure path options

**Date:** 2026-05-09
**Author:** PM
**Type:** Investigation-stage analysis. Per Bible Article 14, this is the post-Phase-B ranking-with-data update anticipated by `Plans/DISPATCH-2026-05-09-sc-3b-phase-b-instrumentation-cerebro.md` §7. Surfaces options for PL ratification on the SC-3B closure path. Not a dispatch; no executable instructions for Cerebro until PL ratifies one of the options below.
**Substrate:** HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`. Tracked working tree clean. Stashes: `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment. Backups `Z:\AltimitNeo\2026-05-09-191624` (pre-instr) and `Z:\AltimitNeo\2026-05-09-194344` (post-Phase-B).

---

## 1. What Phase B told us

Cerebro's Phase B execution per `Plans/DISPATCH-2026-05-09-sc-3b-phase-b-instrumentation-cerebro.md`:

- **NMI-instrumented loop (50 runs at NEO_SMP=4):** 50/50 PASS, 0 NMI captures, 0 silent-timeouts.
- **Clean counterfactual loop (50 runs at NEO_SMP=4):** 50/50 PASS, 0 timeouts.

The dispatch's Outcome C (NMI does not fire) is what landed. Specifically C1: 50 runs without instrumentation also showed zero timeouts, confirming the bug did not reproduce in this observation window with or without the instrumentation perturbing timing.

Patch adaptation: the NMI stash referenced `SwitchMutex::is_locked()` which doesn't exist on this substrate. Cerebro adapted to `try_lock()` inference within the NMI footprint only. The adaptation is sound and was discarded post-run; documented honestly in `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md`. This is acceptable per Phase B dispatch §2.2 ("If the patch needs minor adaptation… make the adaptation in-tree but do NOT commit").

### 1.1 What Phase B did *not* tell us

- Hn (CPU load imbalance from Q4 migration) — **neither confirmed nor refuted.** The mechanism described in the synthesis remains plausible; we just couldn't observe it.
- H1-variant (lost wakeup or starved completer under Q4 wake-on-completer-CPU contention) — **neither confirmed nor refuted.**
- H4 (per-CPU starvation) — **neither confirmed nor refuted.**
- Whether NMI instrumentation hides the bug — **inconclusive.** The clean counterfactual showing zero failures means we can't attribute the zero-NMI-fire result to instrumentation perturbation specifically.
- Whether the bug is closed by anything we did, latent, or simply at a rate too low to sample at n=100 — **undetermined.**

---

## 2. The data we have, end-to-end

```
Original BUILD §9 Step 6 (200-run NEO_SMP=4):  35 PASS, then run 36 silent-timeout. [a61e318 unmodified]
Fix 6.1 gate (200-run NEO_SMP=4 attempt):       5 PASS, then run 6 silent-timeout.   [a61e318 + Fix 6.1 patch]
Phase B NMI loop (50-run NEO_SMP=4):           50 PASS.                              [a61e318 + NMI instrumentation]
Phase B clean counterfactual (50-run NEO_SMP=4): 50 PASS.                            [a61e318 unmodified]
```

Total observations on **clean `a61e318`** post-Fix-6.1-failure: 50 PASS, 0 fail.
Total observations on **clean `a61e318`** including Phase B clean: 50 PASS, 0 fail.
Total observations on **clean `a61e318`** including original gate: 85 PASS, 1 fail (run 36).

The original failure-rate point estimate: 1/36 ≈ 2.8%.

Cerebro's statistical note (Findings §Counterfactual Data): under unchanged 1/36 rate, P[0 failures in 50 clean runs] ≈ 24.5%. P[0 failures in 100 runs combined] ≈ 6.0%, treating the NMI half as same-rate.

**Honest reading of the combined data.** The ratio went from 1/36 to 1/6 to 0/100. None of these transitions are statistically distinguishable from the original 2.8% rate in isolation:

- 1/6 vs. 2.8% rate: P[≥1 in 6] ≈ 16%. Within ordinary variance.
- 0/50 vs. 2.8% rate: P[0 in 50] ≈ 24%. Within ordinary variance.
- 0/100 vs. 2.8% rate: P[0 in 100] ≈ 6%. Borderline; not enough to reject.

What the data does *not* support: the strong claim that "Fix 6.1 made it worse" or "the bug is closed." What the data does support: "we have not yet collected enough samples to discriminate among rate-unchanged / rate-shifted-down / rate-now-zero."

### 2.1 The host-load hypothesis surfaces

The synthesis's primary mechanism (Hn) and runner-up (H1-variant) both relied on QEMU SMP scheduling timing being slow enough to push the orchestrator's finite polls toward the 180s timeout. If the bug is **host-load-dependent** (manifests when host CPU pressure makes QEMU scheduling slow), then:

- The original failure (run 36) and the Fix 6.1 failure (run 6) occurred during periods of unknown host load.
- The Phase B runs occurred during what may be a more idle host state.
- Sampling on idle host gives systematically lower observed rate than sampling on loaded host.

This is consistent with the Hn mechanism but adds a meta-cause: it's not just deterministic Q4 migration; it's deterministic Q4 migration *under specific host scheduling conditions*. We have no instrumentation that captures host load at run time; this is a hypothesis, not a confirmed cause.

If true, it implies:
1. The bug is real and reproducible — but only under host pressure.
2. A 200/200 closure gate run on idle host doesn't strongly evidence the bug is closed; it only evidences the bug isn't currently sampling.
3. A closure gate that includes deliberate host load is a stronger test.

---

## 3. Ranking-with-data

Updates to the synthesis's hypothesis ranking, given Phase B's null outcome:

| Rank | Hypothesis | Pre-Phase-B confidence | Post-Phase-B confidence | Notes |
|------|-----------|------------------------|-------------------------|-------|
| 1 | Hn (deterministic Q4 migration accumulation) | 50% | 35% | Plausible structural defect; no NMI evidence; Fix 6.1 didn't close gate |
| 2 | H1-variant (lost wakeup / starved completer under Q4 contention) | 25% | 25% | Unchanged; Phase B couldn't discriminate |
| 3 | H4 (per-CPU starvation under Q4 accumulation) | 15% | 15% | Unchanged |
| **NEW** | **Hh (host-load-dependent timing)** | (not present) | 20% | Promoted; explains Phase B null + prior 2/42 |
| 4 | H3 (test-scenario coordination edge case) | 8% | 5% | Lower; Phase B null marginally weakens |

Hn drops because Fix 6.1 didn't close the gate; the constant-direction defect alone wasn't sufficient. It's not zero — Fix 6.1 ran for only 6 iterations before failing; insufficient evidence to refute Hn as a contributor. The Hh promotion absorbs the lost confidence from Hn.

Confidence on "the bug is fully closed" (no fix needed): low. ~10%. The bug was observed twice with the original-signature pattern; that's not a fluke. Even if it's currently unsamplable, "currently unsamplable" is not the same as "closed."

Confidence on "the bug is real but rate-low or host-load-dependent enough to not block SC-3B closure on the gate-as-specified": **higher** — 50-60%. The 200/200 closure gate was the standard agreed in the BUILD dispatch. If the bug doesn't manifest in the gate as specified, the gate is satisfied; the bug becomes a known-quirk-of-future-investigation rather than a closure blocker.

---

## 4. Closure-path options for PL

Four options. PM recommendation is option (a). Surfacing all four because the framing matters and PL may want to raise the bar.

### 4.1 Option (a) — Standard closure gate retry

**Action:** Re-run BUILD dispatch §9 Step 6 (200-run stress NEO_SMP=4) + Step 7 (25-run stress NEO_SMP=2) on clean `a61e318`. NMI NOT armed. Stop on first failure.

**If passes:** SC-3B closes via the original gate criterion. Close report acknowledges the prior observations honestly: failure was observed at run 36 of the original loop and run 6 of the Fix 6.1 loop; Phase B was inconclusive; the 200/200 closure gate was satisfied on clean substrate; the bug is logged as "observed but unreproducible at sample size n=100+ post-Fix-6.1; flagged for re-investigation if it manifests in future regression."

**If fails:** the failure run preserved. PM produces a follow-up dispatch re-applying NMI watchdog (`stash@{1}`) for capture pass, focused at the iteration where failure manifested. This time we have a localized window.

**Pro:** Honors the gate-as-specified. Surgical. Doesn't move the goalposts post-hoc.

**Con:** If passes, the close report has to be honest about the situation, which is uncomfortable but accurate. We close on empirical evidence the bug isn't sampling, not on having mechanism-level closure.

**Estimated wall time:** ~1.5–2.5 hours (200 runs at ~10–15s each + 25 NEO_SMP=2 runs).

### 4.2 Option (b) — Extended sample closure gate

**Action:** Same as (a) but 500/500 NEO_SMP=4 + 100/100 NEO_SMP=2.

**If passes 500/500:** P[500/500 PASS | rate=2.8%] ≈ 7×10⁻⁷. Effectively rules out rate at the original observation level. The close-report framing is much stronger: "This sample provides empirical evidence the bug, if still present, is at a rate below what the gate-as-specified would catch."

**Con:** Wall time scales linearly. 500 runs ≈ 4–6 hours. Doesn't address host-load-dependence; if Hh is true, even 500/500 on idle host doesn't tell us about loaded runs.

**Estimated wall time:** ~4–6 hours.

### 4.3 Option (c) — Under-load closure gate

**Action:** Run 200/200 NEO_SMP=4 + 25/25 NEO_SMP=2 with a deliberate host CPU stressor in parallel (e.g., `stress --cpu N` on the host machine, where N is roughly the number of host cores not consumed by QEMU).

**If passes under load:** This is the strongest possible evidence. The Hh hypothesis predicts the bug manifests under host pressure; if the gate passes 200/200 under deliberate pressure, Hh is refuted (or the load-induced perturbation didn't reach the threshold needed; either way the gate is passing).

**Con:** Adds setup complexity. Requires knowing the host's free-CPU budget. May produce flakier non-bug failures (memory pressure, OOM, etc.) that aren't the SC-3B bug. Rules-of-engagement need spelling out. Not standard practice for kernel SMP gates.

**Estimated wall time:** Same as (a), ~1.5–2.5 hours, plus ~30 minutes setup.

### 4.4 Option (d) — Land Fix 6.1 on its own merit, then run closure gate

**Action:** Apply Fix 6.1 stash, commit it as a test-quality improvement (not as a closure fix; commit message clear about that), then run option (a)'s gate on the post-Fix-6.1 substrate.

**Pro:** The constant-direction issue Hn identified is a real test-design defect. Each pair locked into one direction is structurally worse coverage than alternation. Fixing it has standalone test-quality value, separate from whether it closes the bug.

**Con:** Confounds the closure gate with the Fix 6.1 substrate. If gate fails, we don't know whether Fix 6.1 was directionally right and a kernel companion fix is also needed, or whether Fix 6.1 made things worse, or whether it's irrelevant. Cleaner to land Fix 6.1 *after* SC-3B closes, as a separate sub-commit or as part of Sub-step 6.

**Estimated wall time:** Same as (a) plus ~15 minutes for the Fix 6.1 commit.

### 4.5 PM recommendation: option (a)

Reasons:
1. **Bible 1.3 + L precedent.** The L wedge-fix discharge framed 300/300 across multiple scenarios as "strong evidence the proven mechanism is closed" while explicitly noting "a latent very-low-rate remnant of the broader class is not statistically excluded." That framing applies cleanly here. We don't have a structural fix (no A1 equivalent), but we do have empirical evidence and a gate-as-specified.
2. **No goalpost movement.** The 200/200 closure gate was established at BUILD dispatch §9. If it passes, SC-3B closes by the criterion we agreed to.
3. **Phase B's null outcome doesn't justify (b) or (c).** (b) and (c) are improvements on top of (a)'s baseline. We can always escalate to them if (a) passes and we want stronger evidence; we cannot easily de-escalate from them if they fail.
4. **(d) is best deferred.** Fix 6.1 has standalone merit. It belongs in a separate commit, *after* SC-3B closes, framed as test-quality work, not as a closure fix.

If PL wants to raise the bar, my second-choice is (c) over (b). (c) tests the actual hypothesis (Hh); (b) just adds padding to the same evidence type.

---

## 5. The close-report framing if option (a) passes

This is worth pre-figuring so we don't surprise ourselves.

The close report would need to honestly state:

1. The SC-3B Sub-step 5 verification gate originally failed at run 36/200 (silent-timeout signature).
2. PM-led three-way investigation produced ranking with Hn at 50% top.
3. Fix 6.1 (per-pair direction alternation, test-only) was ratified and attempted; gate failed at run 6/200.
4. Phase B NMI watchdog instrumentation captured zero traces in 50 runs; clean counterfactual showed zero failures in 50 runs.
5. The closure gate was re-run on clean `a61e318` and passed 200/200 NEO_SMP=4 + 25/25 NEO_SMP=2.
6. The bug observed at runs 36/200 and 6/200 is logged as **observed but unreproducible at n=200+ post-Phase-B-clean**. Hypotheses Hn / H1-variant / H4 / Hh remain unrefuted; mechanism is not identified.
7. SC-3B closes on empirical-gate satisfaction. Future regression runs may surface the bug; if so, ANSPEC-001 §10.2 cross-reference is updated and a follow-up investigation opens.

The closing tone matches the L precedent: "300/300 is an upper-bound point estimate, not a proof of zero. A latent very-low-rate remnant... is not statistically excluded." We close with our eyes open.

This framing is uncomfortable but accurate. It's preferable to pretending we mechanism-closed the bug when we didn't.

---

## 6. Fix 6.1 disposition (deferred)

`stash@{0}` (`sc-3b-fix-6-1-attempt`) remains preserved.

After SC-3B closes (whichever option), Fix 6.1 disposition becomes a standalone decision:

- **Land as test-quality improvement.** The constant-direction issue is a real test-design defect even if it didn't account for the bug. Per-pair alternation gives strictly better coverage. Could land in Sub-step 6 or as a follow-up commit. Bible 1.3 is satisfied because the commit is intended to ship the test-quality improvement; it doesn't claim to fix a known bug.
- **Discard.** If post-closure analysis decides the test-design defect isn't worth a separate commit.
- **Hold for future use.** If the bug re-manifests in future regression, Fix 6.1 might combine with a kernel fix as a unified close.

PM leans toward "land as test-quality improvement" but defers the call to PL after SC-3B closes. Not a dispatch concern right now.

---

## 7. Stash hygiene note

After Phase B execution, the four-stash state is preserved. The backup script's three-stash mechanism (referenced in Phase B dispatch §0 instruction 4) handled four correctly per Cerebro's findings (`Restored stash refs: 4 (source had 4)`). Confirms the mechanism generalizes; no ADR or backup-script update needed.

---

## 8. Forward queue

Pending PL ratification:

- **Choice among options (a) / (b) / (c) / (d).** PM recommendation: (a). PM second-choice: (c) if PL wants to raise the bar.

After ratification, PM produces a follow-up dispatch with the chosen option's specific instructions. The dispatch shape mirrors the Fix 6.1 dispatch (single execution dispatch with verification gate, stop conditions, outcome handling). For options (a) and (b), no instrumentation; for (c), host-stressor setup; for (d), Fix 6.1 commit + gate.

If the chosen option's gate passes, Sub-step 6 (docs + close) opens with the close-report framing per §5.

If the chosen option's gate fails, follow-up is dispatch dependent: re-arm NMI for capture pass and treat the failure run as the localized investigation window.

---

## 9. Cross-references

**Phase B input + result:**
- `Plans/DISPATCH-2026-05-09-sc-3b-phase-b-instrumentation-cerebro.md` — the dispatch executed.
- `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md` — Cerebro's findings.

**Prior context this note builds on:**
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §3.6, §4.2 — anticipated this Outcome B path.
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` — original synthesis with Hn at 50%.
- `Plans/DISPATCH-2026-05-09-sc-3b-fix-6-1-cerebro.md` — Fix 6.1 attempt.
- `Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md` — Fix 6.1 gate failure.
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` §9 — the closure gate criterion.
- `Plans/SC-3B-VERIFICATION-2026-05-09.md` — original gate's run-36 failure.

**Statistical framing precedent:**
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — L's discharge framing applies analogously here.
- `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md` — 300/300 evidence model.

**Substrate:**
- HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`.
- Stashes `{0}`-`{3}` preserved.

**Governing posture:**
- `docs/bible.md` Articles 1.2, 1.3, 14, 15.2.
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6.

---

**End of note.**

*Phase B null. Hh promoted to 20% as host-load-dependent timing hypothesis. Four closure-path options; PM recommends (a) standard closure-gate retry on clean `a61e318`. Fix 6.1 disposition deferred to post-closure. PL ratification pending.*
