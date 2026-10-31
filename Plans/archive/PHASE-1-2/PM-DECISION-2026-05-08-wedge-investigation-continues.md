# PM-DECISION: wedge investigation continues — Alpha-prime not landed as unmotivated tightening

**Date:** 2026-05-08
**Decision-maker:** Project lead (delegated to PM for documentation)
**Type:** Project-shape decision recording why we chose to continue the wedge investigation rather than land Alpha-prime opportunistically. Names the alternatives we considered and why we rejected them. Project-history record for future readers who might wonder "why didn't they just land the obvious tightening?"
**Authority:** Project lead per Bible Article 24.1.

---

## What was decided

After the H1 validation experiment (`Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md`) refuted the H1 hypothesis in its narrow form, three paths were available:

1. **Land Alpha-prime anyway** as a defensive correctness tightening. It would not fix the wedge (the experiment proved that), but the lock-free short-circuit *is* racy in principle, and future workloads (sub-commit 3b's wake-on-completer-CPU, multi-thread-per-process spawn) will produce AP-resident zombies that could trigger the race we hypothesized. Landing it now would prevent a future bug we'd otherwise have to chase.
2. **Shelve Alpha-prime entirely** until evidence specifically supports it. SC-3A's composition argument depends on the short-circuit, and the H1 experiment proved the short-circuit is empirically safe under current workloads. If future workloads need the tightening, land it then.
3. **Keep investigating the actual wedge** without landing any code change yet. The wedge has a real cause we haven't found; we look for it.

The decision is **path 3**. Investigation continues until we know what fix to write.

## Why path 3, and why explicitly

The Bible's posture (Article 1.3): "We build real operating systems engineering. Toy code, throwaway shortcuts, and 'we'll fix it later' hacks are forbidden as a matter of project culture. Every commit is intended to ship."

A commit titled "fix the test-runner wedge" that does not actually fix the test-runner wedge violates this principle, even if its content is independently defensible as a correctness tightening. The commit message is load-bearing. Future readers — including future-PM, future contributors, and anyone bisecting a regression — will read "fix the wedge" and assume the wedge was fixed. If it wasn't, they will be misled.

Landing Alpha-prime as the wedge fix when it isn't would be the kind of "fast wrong answer" Article 1.2 names as worse than a slow right one. Path 1 is therefore rejected in its "wedge fix" framing. (Path 1 is still available later, separately, as a defensive tightening with an accurately-titled commit — see §future dispositions below.)

Path 2 is also rejected. Shelving Alpha-prime entirely treats the lock-free short-circuit as a non-problem because the experiment didn't trigger it. But the experiment was on a substrate where APs produce no zombies; future substrates will. Marking the question closed because the current substrate doesn't expose the race would be premature. The right disposition is "we know the race is theoretical and we know the substrate where it would become real; we revisit when we get there."

Path 3 follows the actual evidence. We have one concrete wedge instance (run 4 of the H1 experiment), full serial logs, no exotic conditions, and a cause that must lie in code we haven't yet instrumented. The investigation has a clear next step. We do that step.

## What considerations were weighed

### Time cost

Path 1 (land Alpha-prime as wedge fix): ~3-4 hours through to commit. Wedge persists at ~22% baseline until next campaign. Loss: minor — Alpha-prime is small.
Path 2 (shelve): ~30 minutes to record the disposition. Wedge persists at ~22% baseline indefinitely until something forces the issue. Loss: significant — the wedge-fix commitment is harder to meet on time.
Path 3 (keep investigating): ~3-4 hours per investigation cycle, with possibly multiple cycles before we identify the cause. Wedge persists at ~22% baseline during the investigation. Loss: same wedge-rate persistence, but with directional progress toward an actual fix.

Path 3 has the highest information yield per hour. Each investigation cycle either narrows the hypothesis space or eliminates entire candidate explanations. Path 1 produces a defensive tightening but no investigative progress; path 2 produces no progress at all.

### The wedge-fix-before-SC-3B commitment

`Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` names the deadline. Path 3 is the only path that makes meeting the deadline likely:

- Path 1 doesn't fix the wedge; commitment unmet at SC-3B unless someone re-investigates.
- Path 2 doesn't even attempt the fix; commitment unmet by definition.
- Path 3 advances toward an actual fix, even if the timeline depends on what we find.

The commitment is not a license to hurry — it's a forcing function for actually solving the problem. Path 3 honors that distinction.

### SC-3A's pre-implementation analysis

SC-3A's pre-thinking analysis depended on the lock-free short-circuit being preserved. If we landed Alpha-prime (path 1), SC-3A's analysis would need redoing against the post-tightening reaper. If we shelve (path 2) or keep investigating (path 3), SC-3A's analysis remains valid against the current reaper.

The H1 experiment's data actually *strengthens* SC-3A's existing analysis: the short-circuit is empirically safe under current workloads, which is exactly what SC-3A's composition argument assumed. Path 3 preserves this favorable position.

## Future dispositions for Alpha-prime

When the actual wedge fix is identified and landed, Alpha-prime as a *defensive tightening* (separate from the wedge fix) becomes its own decision. The framing matters:

- **If the wedge fix involves re-architecting the reaper anyway**, Alpha-prime may be subsumed — the new reaper design either includes lock-acquisition or doesn't, and the lock-free short-circuit may not exist in the new code at all. No separate Alpha-prime needed.
- **If the wedge fix is in code outside `reap()` entirely** (the current leading hypothesis based on run 4's signature), Alpha-prime remains independently a future tightening. Project lead would decide later whether to land it preemptively or wait until a workload triggers the race.
- **If the wedge fix is something we haven't predicted**, the disposition depends on what we find. We don't pre-decide.

In all cases, an Alpha-prime that lands in the future lands with an accurate commit message — "preemptive tightening of `reap()` lock-free short-circuit against future AP-resident-zombie workloads," not "wedge fix."

## What this PM-DECISION does not do

- It does not commit to any specific time horizon for finding the actual wedge fix. The investigation takes as long as it takes.
- It does not pre-decide what the actual fix will be. Run 4's wedge has a cause; we don't know what it is yet.
- It does not change the wedge-fix-before-SC-3B commitment. The commitment, the deadline, and the de-escalation rule (project lead decision via ADR/amendment, PM cannot re-defer) are unchanged.
- It does not retract the convergence note's three-way structural finding. The contradiction between SC-3A's pre-thinking, §Q9 redo notes, and Jarvis's wedge investigation is still real. The H1 experiment refuted the parsimonious resolution; the contradiction itself remains an open structural concern.

## What this PM-DECISION authorizes

PM proceeds to write `Plans/DISPATCH-2026-05-08-wedge-investigation-2-cerebro.md` targeting run 4's mechanism. The dispatch follows directly from this decision; no additional governance is required.

## Cross-references

- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — the experiment whose results drove this decision.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` — convergence note; §10 captures the updated picture; this PM-DECISION operationalizes §10's "what the recommendation now looks like."
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the commitment this decision honors.
- `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` — Jarvis's original investigation; H1 came from §5.
- `target/verify/H1-EXPERIMENT-run-4.serial.log` — the run-4 timeout instance; substrate for the next investigation.
- Bible Article 1.2 (correctness > coherence > performance ordering).
- Bible Article 1.3 (every commit is intended to ship).
- Bible Article 24.1 (project lead authority).

---

**Decision recorded.** Path 3 (keep investigating) chosen. Alpha-prime not landed as a wedge fix. Future Alpha-prime disposition (defensive tightening or shelve) is its own decision, made later, with accurate commit framing whichever way it goes.
