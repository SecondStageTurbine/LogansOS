# PM-NOTE: Project commitment — test-runner wedge fix before sub-commit 3b

**Date:** 2026-05-07
**Author:** PM
**Type:** Project commitment record. Names a deadline-bearing commitment by the project to fix the test-runner intermittent wedge before a specific future milestone, with a named de-escalation rule. Doc-only; no code or governance change. Lands tonight ahead of tomorrow's post-ADR-015 planning conversation so the commitment is on disk before the convergence note is drafted.
**Authority:** PM. Per Bible Article 24.1, the project lead may override or modify any PM commitment via ADR or amendment; PM cannot unilaterally re-defer this commitment.

---

## The commitment

**The test-runner intermittent wedge will be fixed before sub-commit 3b ships.**

Concretely:

1. The verification gate for sub-commit 3a may accept the wedge as known baseline noise (~20-28% TIMEOUT rate on default-suite runs), as has been the project's posture since sub-commit 1b. Sub-commit 3a is the highest-correctness-risk piece in BUILD-3 (asm release primitive in the scheduler) and changing the noise floor underneath it mid-stream would itself be a correctness hazard.

2. **No verification gate after sub-commit 3a may accept >5% TIMEOUT rate as baseline noise.** Sub-commit 3b's gate fails on its own terms if the wedge has not been fixed by the time 3b is verified. "Wedge has been fixed" means: (a) the root cause is identified and fixed in the kernel, OR (b) the test harness is modified such that the wedge no longer manifests as a TIMEOUT classification, AND (c) a freshly-run 50-boot loop on the parent commit produces ≤2/50 TIMEOUTs.

3. **PM cannot unilaterally re-defer this commitment.** The deadline can be moved only by an explicit project-lead decision recorded as an ADR or Bible amendment. PM may surface a request to re-defer with rationale; the project lead decides.

## Why this commitment exists

The wedge has been deferred four times across four campaigns:

- **Sub-commit 1b verification:** wedge was new and uncharacterized; defensible to defer.
- **Sub-commit 2 bisection (EXP-A through EXP-H):** §Q9 was being deferred for unrelated reasons; wedge investigation deferred alongside.
- **KASLR-Q1B diagnostic:** focused on the heap-overflow panic, not the wedge; defensible to defer.
- **ADR-015 implementation loop (this campaign):** focused on landing the heap fix; defensible to defer.

Each deferral had a reason. Across all four, the common pattern is "we're working on the thing in front of us, the wedge is the thing behind it." That pattern is how projects accumulate permanent noise floors.

The wedge is now characterized. Jarvis's investigation notes (`Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md`) catalogue 5 instances across 2 distinct signatures, propose the leading hypothesis (BSP reap scan races AP zombie production via the lock-free `zombie_count == 0` early-out), and note the convergence with the §Q9 redo design notes (`Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md`). The investigation is not "we don't know yet what this is"; it is "we have a credible hypothesis and the next dispatch can test it."

The project has the substrate to fix this. It has not had a *commitment* to fix it. This note is that commitment.

## Why "before 3b" specifically

Three reasons:

1. **Sub-commit 3a is uniquely high-correctness-risk.** Asm release primitives in the scheduler are the kind of code where flaky verification noise has the highest cost — a real correctness regression could be hidden in the same TIMEOUT bucket as the wedge. But changing the noise floor mid-3a is itself a hazard; better to let 3a verify against the known baseline and then fix the wedge before 3b.

2. **Sub-commit 3b is BUILD-3's last sub-commit.** Per the BUILD-3 sub-commit ledger (sub-commits 1a, 1b, 2, 3a, 3b, 4, 5), 3b is roughly the structural midpoint. Closing BUILD-3's remaining work cleanly requires a verification substrate that produces high signal-to-noise. The wedge has to be gone before that work begins, or BUILD-3 closes with a permanently degraded gate.

3. **Sub-commit 3a's planning will likely produce convergence with §Q9.** Tomorrow's planning conversation will draft `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` (or similar) recording the finding that the wedge investigation and §Q9 redo are very probably one work stream. If the convergence holds, "fix the wedge before 3b" and "land §Q9 redo before 3b" are the same commitment. The §Q9 redo has been on the deferred list since sub-commit 2's EXP-H bisection; tying its deadline to 3b ratifies the same posture.

If the convergence hypothesis turns out to be wrong (the wedge has a different root cause than the §Q9 stopgap's incompleteness), the commitment still holds — it just means the wedge fix is a separate piece of work from §Q9 rather than the same one. Either way, fixed before 3b.

## What this commitment does NOT specify

This note records the *deadline*, not the *implementation*. Specifically:

- It does not specify whether the fix is in the kernel (proper §Q9 redo with full per-CPU semantics + AP-side reaper) or in the test harness (e.g., a kernel-side health probe the harness can poll to distinguish "wedged" from "still working"). The convergence note tomorrow will recommend one path; the project lead decides.
- It does not specify the dispatch shape — single Cerebro dispatch, multi-stage diagnostic-then-fix, or paired Cerebro+Jarvis work. Tomorrow's planning conversation produces that.
- It does not commit to a specific calendar date. "Before 3b" is the binding constraint; the absolute calendar date depends on how 3a's verification proceeds, which is itself in flight.

The commitment is to the project ordering, not to any specific schedule. That is deliberate. A calendar deadline would either be made up (PM has no information to set one truthfully) or would itself become noise to negotiate around. An ordering deadline is verifiable: when 3b is being verified, either the wedge is fixed or it is not.

## What happens if the wedge is not fixed by 3b's verification

Sub-commit 3b's verification gate fails. The project lead chooses one of:

1. **Fix the wedge as part of 3b's scope.** Adds work to 3b but lands it cleanly.
2. **Re-defer the commitment via ADR.** Names new deadline, records reason. PM cannot do this unilaterally.
3. **Accept a degraded gate for 3b.** Bible Article 16.3 ("real boot verification") would be in tension; would require Bible amendment or a one-time deviation justified in the phase document.

Option 2 is the named escape hatch. It exists precisely so that "we made a commitment, can we keep it?" is a real question with a real process, not a quiet erosion. Each invocation of option 2 is itself a project-history artifact and accumulates: re-deferring twice in a row would be a signal the project has a planning-vs-execution mismatch worth a broader retrospective.

## Cross-references

- `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` — Jarvis's investigation; 5 wedge instances, 2 signatures, H1 hypothesis.
- `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` — §Q9 redo design notes; sub-commit 2 deferred §Q9 in favor of the BSP-scans-all-CPUs stopgap that H1 hypothesizes is incomplete.
- `Plans/CLOSE-REPORT-2026-05-07-sub-commit-2.md` — sub-commit 2 closure; references the §Q9 deferral and the wedge as known intermittent.
- `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` — KASLR-Q1B closure; documents the 14/50 (28%) TIMEOUT rate on clean main HEAD.
- ADR-015's in-flight loop logs — `target/verify/ADR-015-PATCH1-loop-run-*.serial.log`, latest sample at 32 runs showing 19% TIMEOUT rate consistent with baseline.
- Bible Article 24.1 (project lead authority over decisions, including re-deferral).
- Bible Article 16.3 (real boot verification at every phase close).
- Bible Article 27 (amendment process).

## What this note will be used for

Tomorrow's planning conversation will:

1. Reference this note as the commitment substrate when drafting `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md`.
2. Drive the sub-commit 3a sequencing decision under the constraint that wedge fix lands before 3b.
3. Drive the Jarvis dispatch to add an "Open project commitments" section to INDEX.md, with this commitment as entry 1.

The third item is the structural durability mechanism: the commitment lives in INDEX.md, not just in this PM-NOTE, so future planning conversations encounter it as part of the project's standard inventory rather than having to remember it from yesterday's stack.

---

**Commitment recorded.** Wedge fix lands before sub-commit 3b. PM cannot re-defer. Project lead can re-defer only via ADR or amendment. INDEX.md will track the commitment from tomorrow forward.
