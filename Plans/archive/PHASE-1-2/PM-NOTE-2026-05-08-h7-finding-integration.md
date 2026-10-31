# PM-NOTE: H7 finding integrated — historical wedge baseline reinterpreted

**Date:** 2026-05-08
**Author:** PM
**Type:** Project-state record. Captures the integration of Jarvis's H7 investigation findings into the project's wedge-investigation context. Names the reinterpreted historical wedge rates, the coordination decision to defer the H7 config fix verification, the classifier reconciliation between Cerebro's dispatch and Jarvis's report, and the strategic implications for the in-flight WEDGE-INV-3 analysis. Doc-only artifact; does not change project commitments. Lands before Cerebro completes WEDGE-INV-3 so his findings report can reference the H7 reinterpretation.
**Authority:** PM. Reinterpretation of historical numbers is bookkeeping, not governance; it does not require ADR or amendment. The wedge-fix-before-SC-3B commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` remains active and unchanged in form.

---

## 1. What the H7 investigation produced

Jarvis ran `Plans/DISPATCH-2026-05-08-h7-investigation-jarvis.md` against the full corpus of 2,504 historical serial logs. Findings in `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md`. Key results:

- **9 H7 instances catalogued across 5 campaigns**, 0.36% of total logs. All identical 318-byte signatures: Limine boot menu rendered, countdown started, no kernel load.
- **Diagnosed cause:** Limine boot menu countdown × QEMU emulated CD-ROM I/O race. Fix: `timeout: 0` in `boot/limine.conf`.
- **No commit on main yet.** Jarvis correctly held off because verification of the fix (30-boot loop with `timeout: 0`) would conflict with Cerebro's in-flight WEDGE-INV-3 baseline measurement (50 boots, in progress at the time of his ping).
- **Canonical three-bucket classifier proposed** in §5.1 of the H7 findings, including a small but meaningful improvement over Cerebro's dispatch §3.2: a negative check for `limine: Loading executable` to avoid potential edge-case misclassification of boots that load the kernel but stall before `_kernel_start`.

## 2. The reinterpreted historical baseline

The most strategically important output of the H7 investigation is the corrected per-campaign in-kernel wedge rates after subtracting H7 instances:

| Campaign | Substrate | Reported TIMEOUT rate | H7 count | Adjusted in-kernel wedge rate |
|---|---|---:|---:|---:|
| KASLR-Q1 (initial, 10 runs) | sub-commit 2 | ~30% | 1 | ~20% |
| KASLR-Q1B (50 runs) | sub-commit 2 | 28% | 0 | 28% |
| ADR-015 PATCH1 (50 runs) | post-ADR-015 | 22% | 4 | **14%** |
| H1 experiment (20 runs, instrumented) | post-ADR-015 | 5% | 0 | 5% |
| WEDGE-INV-2 (30 runs, instrumented) | post-ADR-015 | 3.3% | 1 | **0%** |

Two findings stand out:

**Finding 1: ADR-015 PATCH1's adjusted in-kernel rate is 14%, not 22%.** The historical baseline we've been treating as "the wedge rate on post-ADR-015 substrate" is overstated by ~8 percentage points. The actual rate is meaningfully lower than the headline number, but still well above zero.

**Finding 2: WEDGE-INV-2's adjusted in-kernel rate is 0%.** All 30 runs produced no in-kernel TIMEOUT — the single TIMEOUT in that campaign was H7. Combined with the H1 experiment's 5% rate, this means instrumentation took the wedge rate from 14% to **0%**, not from 22% to 3.3%. The Heisenbug interpretation is sharper now: instrumentation appears to *completely* suppress the in-kernel wedge in this scenario, not merely reduce it.

This is the kind of revision that changes how the next investigation should be framed. The signature-search scope shifts from "find a bug that fires at moderate frequency under any conditions" to "find a bug that fires at moderate frequency on un-instrumented substrate but disappears entirely under any logging-bearing change." The latter is a much more constrained and informative target.

## 3. Implications for WEDGE-INV-3 (in-flight)

Cerebro's WEDGE-INV-3 dispatch (`Plans/DISPATCH-2026-05-08-wedge-investigation-3-baseline-cerebro.md`) is currently in flight — at the time of writing, run 40 of 50 was just produced. He will produce a `Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md` shortly.

His dispatch's §5 decision matrix uses the un-revised numbers as reference. After the H7 reinterpretation:

- **Case A threshold** (IN_KERNEL_SMOKE rate ≥15%): unchanged in form. The post-ADR-015 historical baseline is now ~14%, so an observed rate of 15%+ would be at-or-above historical baseline.
- **Case B threshold** (IN_KERNEL_SMOKE rate ≤5%): unchanged in form, but more meaningful. Going from 14% historical to ≤5% measured would be a stronger signal that the bug is genuinely reduced or gone, because the historical floor is lower than we previously thought.
- **Case C threshold** (5-15%): now spans the historical baseline rather than sitting below it. An intermediate rate is now harder to interpret cleanly — it could be "rate unchanged from pre-investigation baseline" rather than "rate reduced but not eliminated."

Cerebro should be told to use Jarvis's improved classifier (§5.1 of the H7 findings) rather than the original dispatch's §3.2 classifier. The improvement is small for current data (Cerebro's classifier ordering protects against the most obvious misclassification), but adopting the canonical version going forward is project hygiene.

His findings report should also explicitly cross-reference the H7 reinterpretation when comparing his measured rate to the historical baseline — comparing against 14% rather than 22% is the truthful framing.

## 4. The Limine config fix — coordination disposition

The proposed `timeout: 0` change to `boot/limine.conf` is small, well-motivated, and verified-design (§4.3 of the H7 findings shows the mechanism). Verification requires a 30-boot loop. The right coordination posture:

**Bundle verification with Cerebro's WEDGE-INV-3 follow-up.** When Cerebro completes WEDGE-INV-3, his data gives us the pre-fix H7 rate as a side effect (we'll see N H7 instances out of 50 boots on `46984a1` un-modified). The natural next step is a small dispatch that:

1. Applies the `timeout: 0` change to `boot/limine.conf`.
2. Runs 30 boots using a simple scenario (e.g., `kaslr_smoke` per Jarvis's recommendation) and counts H7 instances post-fix. Expected: 0.
3. If verified, lands the change as its own small commit on main with the message Jarvis drafted in §6.1 of the H7 findings.

This produces a clean before/after comparison on the same host, same day, same conditions — much cleaner than verifying the fix as a standalone campaign.

Jarvis remains available for documentation work in the meantime. The H7 fix is unblocked the moment Cerebro pings closure on WEDGE-INV-3.

## 5. The canonical classifier — adoption disposition

Going forward, every verification campaign should use the canonical three-bucket classifier from §5.1 of the H7 findings:

- **PASS** — `NEO_TEST/1 event=PASS`
- **FAIL** — `NEO_TEST/1 event=FAIL` or `panicked at`
- **TIMEOUT_PRE_KERNEL** — Limine menu visible, no `limine: Loading executable` (H7 class; harness/config concern)
- **TIMEOUT_IN_KERNEL_SMOKE** — `sched: spawned 'ping'` reached but no PASS frame (kernel concern; original wedge target)
- **TIMEOUT_BOOT_KERNEL_PRE_SMOKE** — kernel started but didn't reach smoke test (third TIMEOUT class; previously unseen)
- **TIMEOUT_UNCLASSIFIED** — residual

The canonical implementation is the PowerShell function in §5.1 of the H7 findings. The xtask test classifier or any harness wrappers should be updated to match.

This is a separate dispatch — when Jarvis next has bandwidth (probably after the H7 config fix lands), a small Jarvis dispatch updates `tools/xtask/src/main.rs` (and any other classification surfaces) to use the canonical scheme. Code change, single small commit.

## 6. Implications for the wedge-fix-before-SC-3B commitment

The commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` remains active. What changes is the *target*: we need to drive the in-kernel TIMEOUT rate below 5% per the de-escalation rule, which is now relative to a baseline of 14% (post-ADR-015) rather than 22% (the pre-H7-reinterpretation number).

If Cerebro's WEDGE-INV-3 confirms an in-kernel rate ≤5% on `46984a1`, the commitment can be discharged with:

1. The Limine `timeout: 0` config fix (which addresses H7 directly).
2. A regression-guard test that ensures the in-kernel rate stays below the threshold.
3. No kernel patch required — the wedge would have been effectively addressed by ADR-015's heap fix even though that wasn't ADR-015's stated purpose.

If the rate is intermediate (5-14%), we still need a kernel-level fix and the commitment continues to drive investigation.

If the rate is at-or-above 14%, the wedge is fully present on un-instrumented substrate and the next investigation needs out-of-band observation methods.

## 7. What this note explicitly does NOT do

- It does not commit the `timeout: 0` change. That requires verification, which is bundled with the post-WEDGE-INV-3 dispatch.
- It does not retroactively rewrite the close reports for KASLR-Q1B, ADR-015 PATCH1, or any other historical campaign. Those documents stand as written; this note adds a reinterpretation layer, not a revision.
- It does not change the wedge-fix-before-SC-3B commitment's deadline or de-escalation rule. The commitment's *target rate* is implicitly clarified (in-kernel only, below 5%), not the structural commitment.
- It does not pre-empt Cerebro's findings. WEDGE-INV-3's data is what it is; this note just sets the comparative frame.

## 8. Cross-references

- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` — Jarvis's H7 investigation; source of all numbers in §2.
- `Plans/DISPATCH-2026-05-08-h7-investigation-jarvis.md` — H7 investigation dispatch.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-3-baseline-cerebro.md` — Cerebro's in-flight baseline; this note informs his findings.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the commitment whose target this note clarifies.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` — convergence note; §10 is partially superseded by the H7 reinterpretation, but the structural three-way contradiction remains valid.
- `Plans/PM-DECISION-2026-05-08-wedge-investigation-continues.md` — investigation-continues decision; remains valid.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — H1 experiment; reinterpreted in §2 row 4.
- `Plans/WEDGE-INVESTIGATION-2-FINDINGS-2026-05-08.md` — WEDGE-INV-2; reinterpreted in §2 row 5 (now 0% in-kernel).
- `boot/limine.conf` — config fix target; not yet modified.
- `tools/xtask/src/main.rs` — classifier adoption target; not yet modified.

---

**Note recorded.** Cerebro's WEDGE-INV-3 findings can now reference the corrected historical baseline (14% post-ADR-015, not 22%). The H7 fix is queued for verification immediately after WEDGE-INV-3 completes.
