# PM-NOTE: PATCH1 in-kernel wedge signature analysis — at least 3 distinct signatures

**Date:** 2026-05-08
**Author:** PM
**Type:** Project-strategic finding. Reads the 7 non-H7 timeout logs from the ADR-015 PATCH1 50-boot loop and identifies their wedge signatures. Finds that they are NOT all the same bug — they show at least three distinct signatures across three different code phases. Updates the wedge-investigation framing to account for multiple-bugs-presenting-as-one.
**Authority:** PM. Doc-only finding; does not change project commitments. Lands before the next Cerebro dispatch so that dispatch's signature classification can capture all three.

---

## 1. Why this analysis exists

Cerebro's WEDGE-INV-3 baseline (`Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md`) returned 0/50 in-kernel wedges on un-instrumented `46984a1`. ADR-015 PATCH1 measured 14% in-kernel wedge rate (7/50 after H7 subtraction) on functionally identical kernel code (`47972c2`, parent of `46984a1`, identical kernel sources). The 95% CI for 0/50 has an upper bound of ~6%; the joint probability of observing 7/50 then 0/50 under independent sampling at the same true rate is uncomfortable but not impossible.

Before declaring the wedge effectively resolved, three alternative explanations needed evaluation:

- **(A) Sample variance** — true rate is 3-5%, both observations are individually plausible.
- **(B) Environmental factors** — host load, time-of-day, docker/QEMU caching state shift the timing window.
- **(C) Misattribution** — some of PATCH1's "non-H7" timeouts were also harness/environmental, not in-kernel.

PM-side verification (~30 min, doc-only) addressed (C) directly by reading each of the 7 non-H7 PATCH1 timeout logs and confirming progression markers.

## 2. What the logs show

All 7 non-H7 PATCH1 timeouts are confirmed in-kernel: each reached the smoke-test phase (ping/pong/hog spawned) and at least one reap event before timing out. (C) is refuted — Jarvis's attribution was correct.

But examination of the **terminal serial output** of each log reveals **three distinct wedge signatures**, not one:

### 2.1 Signature SMOKE-MID-PHASE (2 of 7)

Wedged mid-smoke-test, with some threads reaped and others still alive. Examples:

**Run 13 (slab_local_to_global_promotion), 4595 bytes, last lines:**
```
[ ping ] 5
[ kernel ] sched: reaped 'pong' (id=5)
[end]
```
Reaped: pong only. Not yet reaped: ping, hog.

**Run 15 (cap_revoke_local), 4595 bytes, last lines:**
```
[ pong ] 5
[ kernel ] sched: reaped 'ping' (id=4)
[end]
```
Reaped: ping only. Not yet reaped: pong, hog.

This signature matches Jarvis's "smoke-test-partial-reap" from his original investigation (`Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` §3) and H1 run-4's pattern (`reaped 'ping' (id=4)` then silence).

### 2.2 Signature SMOKE-COMPLETION (2 of 7)

All three smoke threads reach termination, but the kernel never proceeds to the neo-init / scenario dispatch phase. Examples:

**Run 21 (smep_catch), 4724 bytes, last lines:**
```
[ kernel ] sched: reaped 'pong' (id=5)
[ hog  ] preempted at least 5 ticks (start=10, now=17)
[ hog  ] HOG_TRIGGER seen, exiting
[ kernel ] sched: reaped 'ping' (id=4)
[end]
```

**Run 40 (kaslr_heap_contract), 4723 bytes, last lines:**
```
[ kernel ] sched: reaped 'pong' (id=5)
[ hog  ] preempted at least 5 ticks (start=4, now=11)
[ hog  ] HOG_TRIGGER seen, exiting
[ kernel ] sched: reaped 'ping' (id=4)
[end]
```

Note: hog has visibly received its trigger and exited in both cases. ping was reaped after hog finished. Smoke phase is structurally complete, but no neo-init spawn line appears. The transition from smoke completion to scenario dispatch never happens.

This is a **previously unrecognized signature**. Jarvis's investigation grouped these with smoke-test-partial-reap because they shared the smoke-test scenario context, but they are mechanistically distinct: the smoke test's threads have all completed; the bug is in whatever code runs between smoke completion and neo-init spawn.

### 2.3 Signature POST-SMOKE-USER-PROCESS (3 of 7)

Smoke completes, neo-init spawns, neo-init forks/spawns a worker, the worker exits cleanly, and then progress halts mid-cleanup. Examples:

**Run 11 (cap_survive_exit), 5851 bytes, last lines:**
```
[ kernel ] proc: user thread exited with code 0
[ kernel ] sched: reaped 'worker' (id=8)
[ kernel ] pmap: dropped — freed 11 frames + 1 PML4
[ kernel ] proc: deregistered 'worker' (pid=2)
[ frame ] phase=post-natural-exit used=1039 delta=-12
[end]
```

**Run 17 (cap_survive_exit), 5744 bytes, last lines:**
```
[ kernel ] proc: spawned 'worker' (pid=2) ...
[ frame ] phase=post-spawn used=1051 delta=+12
[ kernel ] proc: user thread exited with code 0
[ kernel ] sched: reaped 'worker' (id=8)
[ kernel ] proc: user thread exited with code 0
[end]
```

**Run 42 (cap_survive_exit), 5654 bytes, last lines:**
```
[ kernel ] sched: enqueued user thread (id=8)
[ kernel ] proc: spawned 'worker' (pid=2) ...
[ frame ] phase=post-spawn used=1051 delta=+12
[ kernel ] sched: reaped 'hog' (id=6)
[ kernel ] proc: user thread exited with code 0
[end]
```

These are **categorically different** from the other signatures. The smoke test has completed, the user-space neo-init scenario has run, and the bug is somewhere in user-process lifecycle code (fork/exit/wait/cleanup). The smoke test is not implicated in these wedges at all.

Notable: all three POST-SMOKE-USER-PROCESS wedges occurred in `cap_survive_exit` scenarios. That correlation may be coincidence (cap_survive_exit was over-represented in PATCH1's per-scenario rotation) or signal (something specific to this scenario triggers the bug).

## 3. What this changes about the wedge picture

### 3.1 The "wedge" was probably never one bug

The H1 hypothesis assumed a single race condition (BSP lock-free short-circuit racing AP zombie production). The H1 experiment refuted that specific hypothesis. The convergence note's amended §10 noted that "the wedge cause is genuinely elsewhere" but framed it as one bug.

The PATCH1 signature analysis shows we were looking for one bug and finding three. SMOKE-MID-PHASE and SMOKE-COMPLETION are both within the smoke-test phase but at different points; POST-SMOKE-USER-PROCESS is in entirely different code (user-process lifecycle).

A possible unifying mechanism exists: all three involve thread/process lifecycle transitions, and all three could in principle share a common substrate-sensitive race (e.g., a slab-allocator or page-table interaction that ADR-015's heap shift incidentally disrupted). But we have no direct evidence for or against that unification.

### 3.2 The 0/50 result is plausibly real but needs more samples

If there are three distinct bugs each at ~1-2% rate, an aggregate ~3-5% true rate is consistent with 7/50 in PATCH1 and 0/50 in WEDGE-INV-3 (the joint probability is moderate). A 100-boot extended baseline tightens the CI substantially: if we observe 0/100, the upper bound drops to ~3%; 0/150 drops to ~2%.

### 3.3 The regression guard needs to cover all three signatures

A regression-guard test that only validates "smoke-test completes correctly N times" will miss POST-SMOKE-USER-PROCESS wedges entirely. The guard needs to either:

- Run a scenario that exercises all three phases (smoke + scenario + neo-init lifecycle) and validate end-to-end success rate, OR
- Run separate guards for each phase, with separate thresholds.

The first is simpler and what the existing `smp_per_cpu_scheduler_smoke` already does — it just needs to be run at a sample size large enough to catch low-rate failures.

### 3.4 Mechanism understanding remains absent

We still don't know what mechanism shifted between the sub-commit 2 substrate (28% in-kernel wedge rate) and the post-ADR-015 substrate (apparent 0% on `46984a1`, 14% on `47972c2`'s PATCH1 measurement). The most parsimonious story is "ADR-015's heap shift disrupted a memory-layout-sensitive race," but this is a story, not a proof. The bugs could return if a future change recreates the trigger conditions.

This is the gap the regression guard fills. It's not a substitute for understanding; it's a structural check that catches the bugs' return if our understanding is wrong.

## 4. Implications for the next dispatch

The next Cerebro dispatch should:

1. **Apply the queued Limine `timeout: 0` fix** (eliminates the H7 class).
2. **Run an extended baseline** of 100 boots on `smp_per_cpu_scheduler_smoke` at NEO_SMP=4 with the canonical classifier and signature sub-classification.
3. **For any IN_KERNEL_SMOKE timeouts that occur**, sub-classify by signature (SMOKE-MID-PHASE / SMOKE-COMPLETION / POST-SMOKE-USER-PROCESS / new).
4. **Combined with WEDGE-INV-3's 50, give 150 total observations** post-Limine-fix on the current substrate. 95% CI upper bound at 0/150 ≈ 2%.
5. **Commit only the Limine config change**, not any kernel changes.

The dispatch (`Plans/DISPATCH-2026-05-08-wedge-investigation-4-extended-baseline-cerebro.md`) follows this PM-NOTE.

## 5. Implications for the wedge-fix-before-SC-3B commitment

The commitment's de-escalation rule requires (a) root cause identified and fixed in kernel, OR (b) test harness modified, AND (c) 50-boot loop ≤2/50. We currently have:

- (a) — root cause not identified for ANY of the three signatures.
- (b) — Limine fix is partial harness work but does not address the in-kernel signatures.
- (c) — 0/50 on WEDGE-INV-3 satisfies this for one measurement.

The honest disposition is that the commitment cannot be cleanly discharged via the existing de-escalation rule because we don't have (a) or full (b). What we have is "the wedge appears effectively absent on the current substrate by mechanism we don't understand."

This may warrant an amendment to the commitment language. The amendment would record that the de-escalation rule was written assuming the bug was reproducible and fixable; the actual situation (bug appears resolved by an unrelated substrate change) doesn't fit either branch. The amended disposition would name "addressed-by-substrate-shift-plus-regression-guard" as a third path, with the regression guard as the load-bearing structural mechanism.

PM does not draft the amendment yet. The right time is after WEDGE-INV-4 (the extended baseline) confirms the rate is durable. If the rate stays at 0% across 150 observations, the amendment is straightforward; if the rate climbs back up under different conditions, the amendment is unwarranted and we go back to investigation.

## 6. What this PM-NOTE explicitly does NOT do

- It does not retroactively rewrite Jarvis's wedge investigation notes or the convergence note. Those documents are correct relative to what was known when written; this note layers updated understanding on top.
- It does not propose a kernel fix. We don't have enough mechanism understanding to propose a targeted fix for any of the three signatures.
- It does not declare the wedge fixed. It declares that "the wedge appears effectively absent" with specific caveats.
- It does not amend the wedge-fix-before-SC-3B commitment. The commitment remains active in its original form pending WEDGE-INV-4 results.

## 7. Cross-references

- `target/verify/ADR-015-PATCH1-loop-run-{11,13,15,17,21,40,42}.*.serial.log` — the 7 non-H7 PATCH1 timeout logs analyzed in §2.
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` — Jarvis's H7 catalog identifying the 4 H7 instances; this analysis covers the remaining 7.
- `Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md` — Cerebro's 0/50 result on un-instrumented `46984a1`.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — H1 experiment; run-4 partial-reap signature matches SMOKE-MID-PHASE.
- `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` — Jarvis's original investigation; SMOKE-MID-PHASE matches his "smoke-test-partial-reap" signature, SMOKE-COMPLETION and POST-SMOKE-USER-PROCESS were not previously distinguished.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10 — convergence amendment; supersedes the original parsimonious resolution.
- `Plans/PM-NOTE-2026-05-08-h7-finding-integration.md` — integration of H7 findings; reinterpreted historical baselines.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment whose disposition is now in question.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-4-extended-baseline-cerebro.md` — next dispatch (forthcoming).

---

**Finding recorded.** The wedge has at least three distinct signatures across PATCH1's data. The next dispatch handles the H7 fix and extends the baseline; the commitment disposition is deferred until that data is in.
