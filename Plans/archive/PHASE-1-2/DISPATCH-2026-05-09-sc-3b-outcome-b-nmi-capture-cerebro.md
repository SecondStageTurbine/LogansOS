# Dispatch: SC-3B Outcome B — NMI-armed capture pass

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-09
**Type:** Investigation-stage execution dispatch. Per Bible Article 14, this is the Outcome B follow-up to `Plans/DISPATCH-2026-05-09-sc-3b-closure-gate-retry-cerebro.md` §6.2. The closure-gate retry failed at run 10/200 with the missing-terminal-frame signature — the localized window Phase B couldn't manufacture. NMI watchdog re-arms; smaller targeted loop captures stack traces. Investigation only: no commits.
**Substrate:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`. Tracked working tree clean per Cerebro's exit state. Backup with closure-gate artifacts: `Z:\AltimitNeo\2026-05-09-202525`.

Stash list at dispatch:

- `stash@{0}: On main: sc-3b-fix-6-1-attempt`
- `stash@{1}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{2}: On main: wedge-investigation-2-instrumentation`
- `stash@{3}: On main: h1-experiment-instrumentation`

The Fix 6.1 stash (`{0}`) remains untouched throughout. NMI watchdog (`{1}`) is the only stash applied during this dispatch.

This dispatch runs in parallel with `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-janus-deep-research-janus.md` (Janus deep-internet research on prior art). The two are independent; neither blocks the other.

---

## 0. Standing instructions

1. **Investigation only. No commits.** Same posture as Phase B.
2. **Re-apply the Phase B NMI patch with the same adaptation.** Cerebro already worked out the `is_locked()` → `try_lock()` adaptation; reuse it.
3. **Smaller, targeted loop.** 50 runs at NEO_SMP=4, with early-stop on first NMI capture. The failure rate is currently active (1/10 in the closure-gate retry); NMI fire within 5-10 runs is the expected outcome.
4. **No Fix 6.1 mixing.** Stash `{0}` untouched.
5. **Backup at end of dispatch.** One backup after capture is complete (whether via NMI fire or 50-run exhaustion).
6. **Stop conditions are honored** (§5).
7. **PM is on call** for trace classification or any pivot decision.

---

## 1. Goal

Capture NMI-driven per-CPU stack traces at the moment the missing-terminal-frame failure signature manifests. This is the data Phase B couldn't produce because the failure didn't reproduce in that observation window.

The closure-gate retry's run 10 failure (`target/verify/sc3b-closure-gate-clean/neo-smp4/run-010.serial.log`) confirms the failure is currently sampling on this host. Re-running with NMI armed within the same wall-clock window has high probability of capture.

The output is per-CPU stack traces classified per `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` §4.3 distinguishing table (Hn vs H1-variant vs H4 vs Hh). Cerebro's findings feed PM ranking-with-data, which together with Janus's parallel deep-internet research output drives the next dispatch (kernel fix design).

---

## 2. Substrate prep

```bash
git rev-parse HEAD                        # expect a61e318d93321c9e84732869f5de9da96abe3d5f
git status                                # expect "nothing to commit, working tree clean"
git stash list                            # expect 4 entries; stash@{0} = sc-3b-fix-6-1-attempt
docker compose run --rm neo cargo k       # expect PASS, 18 warnings (clean baseline)
```

If any diverge, halt and ping PM.

---

## 3. Apply NMI watchdog patch

```bash
git stash apply stash@{1}
```

Apply the same `is_locked()` → `try_lock()` adaptation Cerebro made in Phase B (per `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md` §Substrate "Patch adaptation"). The adaptation is local to the NMI instrumentation footprint in `kernel/src/sched/scheduler.rs`; no kernel source outside the NMI patch is modified.

Verify the patched substrate builds:

```bash
docker compose run --rm neo cargo k
```

Expected: PASS at 18 warnings + any warnings the NMI patch introduces (Phase B observed none).

NMI threshold: **10 seconds post-BEGIN serial silence** — same as Phase B. The threshold worked correctly during Phase B's 50 PASS runs (zero false positives during normal completion); reuse rather than re-tune.

---

## 4. The capture loop

### 4.1 Run shape

50 boots of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4. Early-stop on first NMI capture.

Artifact directory:
```
target/verify/sc3b-outcome-b-nmi/neo-smp4/
```

Mirror the Phase B artifact structure: per-run serial log, results.jsonl, host log per failure run, NMI-specific dump file naming (e.g., `run-NNN.nmi.serial.log` for any run that produces NMI output).

### 4.2 Early-stop discipline

**Aggressive early-stop on first NMI capture.** One good trace is enough to feed §6 classification. The current failure rate (~10% from the closure-gate retry: 1/10) suggests NMI fire within 5-10 runs is highly likely.

If the first NMI fire is partial or ambiguous (e.g., NMI fired but stack trace truncated, or only some CPUs dumped), continue the loop until either:
- A second, cleaner NMI capture, OR
- Three NMI captures total (per the Phase B dispatch §3.3 PM recommendation), OR
- 50 runs.

Whichever first.

### 4.3 What we want from the trace

Per `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` §4.3 distinguishing table:

| Signal | Hn (CPU load imbalance) | H1-variant (lost wakeup) | Hh (host-load-dependent) |
|--------|------------------------|--------------------------|--------------------------|
| Failing iteration's pair | Likely pair 1 or 3 | Any pair | Any pair |
| iter_count in endpoint_blocked | Very large (>50,000) | Small (<1000) | Either |
| NMI: orchestrator CPU | Inside yield_now | Inside iteration_done past endpoint_blocked | Inside yield_now or endpoint_blocked |
| NMI: worker CPUs | Multiple in yield loops | One in wait_for_stress_go; one in EndpointState | Multiple in yield loops |
| Channel state at hang | One side blocked, peer not signaled | One side blocked, peer signaled but returned wrong | Either |
| Pattern across captures | Same pair every time | Varies by run | Pair varies but timing tight |

The Hh column is added per `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §3 (host-load-dependent timing). It's not cleanly distinguishable from Hn at trace level alone — host-load is a meta-cause that manifests through Hn-like or H1-variant-like patterns. The host load itself isn't observable from the trace; it's inferred from cross-trace pattern (e.g., timing tight + variable-pair = Hh; same-pair + iteration-count high = Hn).

### 4.4 Per-CPU fields to extract

For each NMI fire, extract for CPU0, CPU1, CPU2, CPU3:

- IP + symbol (which function).
- Function-context interpretation (which loop, which call site, what state).
- Notable register state if the NMI patch dumps it (RFLAGS, key general-purpose registers).
- Any kernel structure state the patch dumps (per-CPU scheduler inner state if `try_lock` succeeded, channel state if accessible).

Symbol resolution from kernel binary acceptable offline.

---

## 5. Stop conditions

Halt and ping PM if any of these fire:

- Build under NMI patch fails (`cargo k` non-zero).
- Stash apply produces conflicts that don't resolve with the Phase B `try_lock` adaptation.
- Fix 6.1 stash (`{0}`) is altered in any way.
- Working tree contains kernel source modifications outside the NMI patch's footprint.
- Two consecutive runs hit silent-timeout WITHOUT NMI firing — threshold may be wrong even though Phase B established it; halt and ping PM.
- 50 runs complete with zero NMI fires AND zero silent-timeouts. The failure window has closed without capture; PM proposes next move.
- Any commit lands during the dispatch.

---

## 6. Findings log

Produce `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md` (or 2026-05-10 if calendar advances). Structure:

```
# SC-3B Outcome B NMI capture findings

## Substrate
[HEAD, stash list at start, NMI patch adaptation reused from Phase B]

## Run summary
[N runs, M PASS, K NMI captured, J silent-timeouts-without-NMI if any]

## Per-trace analysis
For each NMI capture:
  - Run number, serial log path, NMI dump path
  - Wall-clock from BEGIN to NMI fire
  - CPU0 IP + symbol + interpretation
  - CPU1 IP + symbol + interpretation
  - CPU2 IP + symbol + interpretation
  - CPU3 IP + symbol + interpretation
  - Inferred channel/scheduler state
  - Classification (Hn / H1-variant / H4 / Hh / unknown)

## Pattern across captures
[Same pair? Same orchestrator state? Cross-trace consistency?]

## Comparison to closure-gate run 010
[Does the NMI-captured failure match run 010's missing-terminal signature exactly?
Or are there observable differences that suggest the NMI changed the bug's
manifestation?]

## Recommendation
[For each captured pattern, what hypothesis is supported / refuted]
[Whether the data is sufficient to design a kernel-level fix]
[Whether a second-pass instrumentation (PROGRESS frames, cpu_id tracking) is needed]
```

Working-tree artifact. Captured by post-dispatch backup.

---

## 7. Exit handling

After findings are written:

```bash
git restore kernel/src/                    # discard NMI watchdog patch
git status                                 # expect clean
git stash list                             # expect 4 entries unchanged
git rev-parse HEAD                         # expect a61e318
docker compose run --rm neo cargo k        # confirm 18-warning baseline restored
```

Both stashes preserved. NMI watchdog (`{1}`) available for further captures if needed. Fix 6.1 (`{0}`) untouched.

Run backup. Notify PM.

---

## 8. Outcomes and forward path

### 8.1 Outcome A — Clean NMI capture(s) classify against §4.3 table

This is the expected outcome.

PM:
1. Reads findings.
2. Cross-references with Janus's parallel deep-internet research output (`Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` per the parallel dispatch).
3. Produces ranking-with-data update — likely promoting one hypothesis to high confidence based on combined trace + prior-art evidence.
4. Drafts kernel fix design dispatch for Cerebro based on the converged hypothesis.

### 8.2 Outcome B — NMI fires but traces are ambiguous

Multiple captures don't cleanly classify. PM produces a second-pass instrumentation dispatch (PROGRESS frames + cpu_id tracking per synthesis §3.2-3.4).

This was Phase B's anticipated Outcome B and didn't fire then because nothing reproduced. May fire here if traces show novel state.

### 8.3 Outcome C — 50 runs, zero NMI fires, AND silent-timeouts still occur

NMI threshold needs lowering. Halt at second silent-timeout-without-NMI run per §5. PM tunes threshold and re-dispatches a smaller follow-up loop.

### 8.4 Outcome D — 50 runs, zero NMI, zero silent-timeouts

The failure window has closed (host-load-dependent or otherwise). Combined with the closure-gate retry's 1/10 rate, this would be informative — the bug's sampling rate is high enough to fire one in every 10 closure attempts but low enough that 50 instrumented runs miss it. PM produces a Hh-favoring ranking update and proposes either accepting the closure-gate failure as host-load-conditional and proceeding under Bible 1.3 caveat, or escalating to under-load gate (option (c) from `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §4.3).

---

## 9. Estimated wall time

- Substrate prep + NMI patch apply (§2-3): ~15-20 minutes.
- Capture loop (§4): if NMI fires within 5-10 runs as expected, ~5-15 minutes total. If 50-run exhaustion (Outcome D), ~30-60 minutes.
- Findings log + classification (§6): ~30-60 minutes.
- Restore + backup (§7): ~10 minutes.

Total: ~1-2 hours from dispatch acceptance to handoff if NMI fires; ~2-3 hours if 50-run exhaustion.

---

## 10. Cross-references

**Authority for this dispatch:**
- `Plans/DISPATCH-2026-05-09-sc-3b-closure-gate-retry-cerebro.md` §6.2 — the Outcome B branch this dispatch executes.
- `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §4.3 — Hh hypothesis context.

**The failure that necessitated this:**
- `target/verify/sc3b-closure-gate-clean/neo-smp4/run-010.serial.log` — failure artifact.
- Closure-gate retry verification log (Cerebro to land at `Plans/SC-3B-CLOSURE-GATE-VERIFICATION-2026-05-09.md` per closure-gate dispatch §6.2; if not yet on disk, this dispatch's findings reference the artifact directly).

**Phase B precedent for the NMI infrastructure:**
- `Plans/DISPATCH-2026-05-09-sc-3b-phase-b-instrumentation-cerebro.md` — Phase B dispatch.
- `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md` — Phase B findings, including the `is_locked()` → `try_lock()` adaptation Cerebro reuses here.

**Parallel running dispatch:**
- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-janus-deep-research-janus.md` — Janus deep-internet research on the same recurrence pattern.

**Hypothesis classification:**
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` §4.3 — distinguishing table (extended in §4.3 of this dispatch with Hh column).

**Substrate:**
- HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`.
- Backup `Z:\AltimitNeo\2026-05-09-202525` (post-closure-gate-retry, with run 010 failure preserved).
- Stashes `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment.

**Governing posture:**
- `docs/bible.md` Articles 1.2, 14, 15.2.
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6.

---

**End of dispatch.**

*NMI re-arm on clean a61e318. 50-run NEO_SMP=4 with early-stop on first capture. Reuse Phase B's `try_lock` adaptation. 10s threshold reused. Failure rate active (1/10 in closure-gate retry); NMI fire expected within 5-10 runs. Findings log → PM ranking-with-data → kernel fix design. Runs in parallel with Janus deep-internet research dispatch. Estimated 1-2 hours. PM is on call.*
