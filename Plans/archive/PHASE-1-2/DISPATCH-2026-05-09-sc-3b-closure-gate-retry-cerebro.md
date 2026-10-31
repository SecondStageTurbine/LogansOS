# Dispatch: SC-3B closure-gate retry on clean a61e318

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-09
**Type:** VERIFY-stage execution dispatch. Per Bible Article 14, this re-runs the BUILD dispatch §9 Step 6 + Step 7 closure gate on clean `a61e318` following Phase B's null outcome. No code changes. No commits during the gate run itself. The gate-as-specified is the closure standard; if it passes, SC-3B closes via Sub-step 6 (docs + close, separate dispatch). If it fails, NMI watchdog re-arms for a localized capture pass.
**Substrate:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`. Tracked working tree clean per Phase B exit state. Backup: `Z:\AltimitNeo\2026-05-09-194344` (post-Phase-B).

Stash list at dispatch:

- `stash@{0}: On main: sc-3b-fix-6-1-attempt`
- `stash@{1}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{2}: On main: wedge-investigation-2-instrumentation`
- `stash@{3}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this dispatch's primary path. `stash@{1}` (NMI watchdog) is reserved for the Outcome B fallback path (§6.2). The Fix 6.1 stash (`{0}`) remains untouched throughout; its disposition is deferred to post-closure decision.

Authority for this dispatch: PM ratification of option (a) per `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §4.5. Project-lead delegated PM authority for closure-path option choice within the four surfaced options.

---

## 0. Standing instructions

1. **Single primary action: re-run BUILD §9 Steps 6 + 7 on clean `a61e318`.** No code changes during the run. No commits during the run.
2. **Substrate stays clean.** `git status` reports clean throughout the gate. If a hand-edit slips into the working tree, halt and ping PM.
3. **No NMI instrumentation on the primary run.** The closure gate runs on the substrate that ships. Stash `{1}` (NMI) is on standby only for Outcome B (§6.2).
4. **Fix 6.1 stash (`{0}`) is untouched.** Do not apply, do not modify, do not stash drop. Hands off entirely.
5. **Backup runs at the end of the dispatch.** One backup after Step 6 + Step 7 complete (whether passing or failing). With four stashes preserved, the 2026-05-08-refined script handles N stashes correctly per Phase B's verification (`Restored stash refs: 4 (source had 4)`).
6. **Stop conditions are honored** (§5).
7. **PM is on call** for any clarification, classification of failure signatures, or pivot decisions.

---

## 1. Goal

Satisfy the BUILD-3 sub-commit 3 closure gate on clean `a61e318`. Specifically:

- **Step 6:** 200-run loop of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4. **200/200 PASS, zero classifier-signature timeouts.**
- **Step 7:** 25-run loop of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=2. **25/25 PASS.**

The closure criterion is unchanged from BUILD dispatch §9. We are re-running the original gate, not a modified one. The PASS frame schema is unchanged. The stop discipline is unchanged.

If the gate passes: SC-3B closes via the standard criterion. PM follows with a Sub-step 6 dispatch covering ADR-013 §Q4/§Q6 amendments, PHASE-2.md update, close report (with honest framing per `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §5), and INDEX sweep.

If the gate fails: the failure run is the localized investigation window we did not have during Phase B. PM produces a follow-up NMI-armed capture dispatch.

---

## 2. Pre-run sanity

Before launching the gate loops:

```bash
git rev-parse HEAD                        # expect a61e318d93321c9e84732869f5de9da96abe3d5f
git status                                # expect "nothing to commit, working tree clean"
git stash list                            # expect 4 entries; sc-3b-fix-6-1-attempt at {0}
docker compose run --rm neo cargo k       # expect PASS, 18 warnings
```

If any diverge from expected, halt and ping PM.

Single-run sanity at NEO_SMP=4 and NEO_SMP=2:

```bash
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress
docker compose run --rm -e NEO_SMP=2 neo cargo neo-test smp_cross_cpu_rendezvous_stress
```

Both expected PASS. PASS frame format:

```
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=<N>
```

If either single-run sanity fails, halt — that is itself a failure signature worth investigation, not just a sample of the rate.

---

## 3. Step 6 — 200-run NEO_SMP=4 loop

### 3.1 Run shape

200 boots of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4. Stop on first failure of any classifier signature per BUILD dispatch §9.4 (lost wakeup, deadlock-with-waiter-unpublished, deadlock-with-waiter-not-woken, wrong-CPU-migration, duplicate-terminal-frame, missing-terminal-frame, panic-or-bug, frame-leak, rights-mismatch, EBADF/EPIPE).

The original-gate signature (the one we have observed twice and could not capture at Phase B) is **missing-terminal-frame**: BEGIN frame followed by silence, no PASS/FAIL/BUG/PANIC, QEMU 180s timeout. If the gate fails with this signature, it's the localized capture opportunity §6.2 anticipates.

### 3.2 Artifact directory

```
target/verify/sc3b-closure-gate-clean/neo-smp4/
```

Include:
- Per-run serial log (xtask default behavior).
- Run summary results (jsonl format mirroring Phase B's `results.jsonl`).
- The host log line for the failure run, if any.

### 3.3 PASS criteria per run

Unchanged from BUILD dispatch §9.2:
- BEGIN frame observed.
- Exactly one terminal PASS or FAIL frame.
- Terminal frame is PASS.
- Debug-exit code matches PASS.
- No `BUG:`, `KERNEL PANIC`, timeout, or frame-leak audit failure.
- PASS frame fields present: `iterations=1000 send_first=<n> recv_first=<m> cpus=4`. The values of `n`+`m` should equal 1000.

### 3.4 Statistical context

Per `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §2:

- Original observed rate: 1/36 ≈ 2.8%.
- Combined post-Fix-6.1-failure clean observations: 100/100 PASS. P[0 in 100 | rate=2.8%] ≈ 6%.
- A successful 200/200 here adds another 200 PASS to the clean count. Combined 300/300 across multiple sample windows would mirror the L wedge-fix-discharge precedent's framing: P[300/300 | rate=2.8%] ≈ 0.02%.

If the gate passes, the empirical evidence is statistically strong but explicitly not a proof of mechanism closure. The close report (separate dispatch) frames this honestly.

If the gate fails, we are back in known territory; the failure run is the data Phase B couldn't produce, this time on idle-host conditions where Phase B happened to be running.

---

## 4. Step 7 — 25-run NEO_SMP=2 loop

After Step 6 closes 200/200, run Step 7.

### 4.1 Run shape

25 boots of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=2.

Artifact directory:
```
target/verify/sc3b-closure-gate-clean/neo-smp2/
```

PASS criteria per run: same as §3.3 but with `cpus=2`.

Step 7's purpose is coverage breadth, not statistical strength. NEO_SMP=2 exercises a different CPU layout (2 pairs sharing 2 CPUs in a different migration topology than 4 pairs across 4 CPUs). 25 runs at this size is sufficient for sanity coverage; not chasing statistical zero.

If Step 7 fails on any run with any classifier signature, halt and ping PM. Step 7 failure is more diagnostically interesting than Step 6 failure because it would refute "the bug is NEO_SMP=4-specific," shifting the hypothesis space.

---

## 5. Stop conditions

Halt and ping PM if any of these fire:

- Pre-run sanity (§2) fails.
- `git status` becomes non-clean during the gate (any tracked-file modification).
- Any commit lands during the dispatch.
- The Fix 6.1 stash content is altered in any way.
- Single-run NEO_SMP=4 or NEO_SMP=2 sanity in §2 fails.
- Step 6 (200-run NEO_SMP=4) records ANY failure run with the missing-terminal-frame signature OR any other classifier-signature failure. Stop on first failure; do not extend the loop chasing statistical zero (BUILD dispatch §9.5 discipline).
- Step 7 (25-run NEO_SMP=2) records any failure.
- Backup script reports failure for two consecutive attempts.

If a stop condition fires, the gate run is incomplete. Do not commit anything. Capture diagnostics, run backup, ping PM.

---

## 6. Outcomes and forward path

### 6.1 Outcome A — Both Step 6 and Step 7 pass cleanly

This is the expected outcome.

Cerebro:
1. Land verification log at `Plans/SC-3B-CLOSURE-GATE-VERIFICATION-2026-05-09.md` (or 2026-05-10 if calendar advances). Structure mirrors `Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md`:
   ```
   ## Substrate
   ## Pre-run sanity
   ## Step 6 (200-run NEO_SMP=4)
   ## Step 7 (25-run NEO_SMP=2)
   ## Statistical interpretation
   ## Conclusion
   ```
2. Run backup.
3. Notify PM that the gate has closed.

PM then produces the Sub-step 6 dispatch covering:
- ADR-013 §Q4 + §Q6 amendments (implementation 2026-05-09 sub-sections per BUILD dispatch §7.1–7.2; mechanically unchanged from original BUILD plan).
- ADR-011 supersession note.
- PHASE-2.md update.
- Close report at `Plans/CLOSE-REPORT-2026-05-XX-sc-3b.md` with honest framing per the closure-options note §5: failure observed twice, three-way investigation produced ranking, Fix 6.1 attempt didn't close, Phase B captured nothing, gate-as-specified passed on clean substrate, bug logged as observed-but-unreproducible at n=300+, mechanism not identified, ANSPEC-001 §10.2 cross-reference for forward concern.
- Fix 6.1 stash disposition (PM-leaning: land as standalone test-quality improvement post-closure; not part of Sub-step 6 itself).
- INDEX sweep.

### 6.2 Outcome B — Step 6 fails (any signature)

The failure run is the localized capture opportunity Phase B couldn't manufacture. The post-Phase-B environment is unchanged and Phase B's NMI infrastructure is one stash-apply away.

Cerebro:
1. Preserve the failure run's serial log and host log.
2. Do NOT delete `target/verify/sc3b-closure-gate-clean/`.
3. Run backup capturing the failure state.
4. Notify PM with the failure signature classification per BUILD dispatch §9.4.

PM then produces a follow-up NMI-armed capture dispatch:
- Apply `stash@{1}` (NMI watchdog) with the same patch adaptation Cerebro made in Phase B (`is_locked()` → `try_lock()` inference).
- Smaller loop (50 runs at NEO_SMP=4) targeted at the same scenario.
- Same threshold (10s post-BEGIN silence) per Phase B's choice.
- Stop at first NMI capture or 50 runs.

Importantly: the Outcome B follow-up is *targeted reproduction*, not exploratory. Phase B already validated the NMI infrastructure runs cleanly; the question is whether the next failure window catches it. If the failure rate is currently active (which we'd know from this gate failing), 50 runs gives high probability of capture per synthesis §3.

### 6.3 Outcome C — Step 6 passes 200/200 but Step 7 fails

Diagnostically interesting. NEO_SMP=2 failure refutes "the bug is NEO_SMP=4-specific" — the hypothesis space shifts. Hn (which depends on 4-CPU migration topology) becomes less likely; a more fundamental scheduling or rendezvous bug becomes more likely.

Cerebro: same as Outcome B, capture the failure, ping PM.

PM produces a different follow-up: NMI capture at NEO_SMP=2 (smaller hypothesis space; may discriminate better) + revised hypothesis ranking.

### 6.4 Forward path summary

```
Step 6 (200/200 NEO_SMP=4) ──pass──→ Step 7 (25/25 NEO_SMP=2) ──pass──→ Outcome A (Sub-step 6 close)
                            │                                  │
                            └──fail──→ Outcome B               └──fail──→ Outcome C
                                       (NMI capture follow-up)            (NMI@SMP=2 + rank update)
```

---

## 7. Estimated wall time

- Pre-run sanity (§2): ~10 minutes.
- Step 6 (200-run NEO_SMP=4): per Phase B timing (~10s avg per run), ~30–40 minutes if PASS-only; up to ~3 hours if all runs hit 180s timeout (extremely unlikely; gate stops on first failure).
- Step 7 (25-run NEO_SMP=2): ~5–10 minutes if PASS-only.
- Verification log + backup: ~15 minutes.

Total: ~1–1.5 hours from dispatch acceptance to handoff if gate passes cleanly. ~30–60 minutes if gate stops early on a failure.

---

## 8. What this dispatch does NOT do

- Does not commit anything. The gate-run produces a verification log; no kernel or test source changes commit.
- Does not amend Sub-step 5's commit body cosmetic literal-`\n` artifact (per standing rule).
- Does not pre-figure ADR-013 amendments. Those land in the separate Sub-step 6 dispatch only after closure.
- Does not pre-decide Fix 6.1 stash disposition. Deferred to post-closure decision.
- Does not raise the gate threshold above the BUILD §9 standard. The closure criterion is the criterion as agreed.
- Does not run NMI instrumentation on the primary path. The substrate that ships is the substrate that gates.

---

## 9. Cross-references

**Authority for this dispatch:**
- `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §4.5 — PM ratification of option (a).
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` §9 — the gate-as-specified, unchanged.

**Substrate context:**
- HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` — SC-3B Sub-step 5 commit.
- Backup `Z:\AltimitNeo\2026-05-09-194344` — post-Phase-B.

**Prior gate runs (informing the statistical context):**
- `Plans/SC-3B-VERIFICATION-2026-05-09.md` — original gate, run 36 failure.
- `Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md` — Fix 6.1 attempt, run 6 failure.
- `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md` — Phase B null capture, 100/100 clean.

**Statistical framing precedent:**
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — L's discharge framing, P[300/300 | rate>0.5%] ≈ 0.45%.
- `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md` — 300/300 evidence model.

**Outcome A close-report inputs:**
- `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §5 — pre-figured close-report framing.
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` — original synthesis.
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §2.7 — three-way investigation summary.

**Outcome B / C follow-up infrastructure:**
- `stash@{1}` (NMI watchdog) — preserved post-Phase-B; one apply away.
- `Plans/DISPATCH-2026-05-09-sc-3b-phase-b-instrumentation-cerebro.md` — Phase B dispatch as template for follow-up.

**Governing posture:**
- `docs/bible.md` Articles 1.2 (correctness > coherence > performance), 1.3 (every commit intended to ship), 14 (BUILD/VERIFY cadence; this is VERIFY of an existing build).
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6 — substrate this gate exercises.
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §10.2 — forward concern noted in close report if Outcome A.

---

**End of dispatch.**

*Re-run BUILD §9 Step 6 (200/200 NEO_SMP=4) + Step 7 (25/25 NEO_SMP=2) on clean `a61e318`. No code changes. No commits during the run. NMI stash on standby for Outcome B. Fix 6.1 stash untouched. Estimated 1–1.5 hours if gate passes. Outcome A → Sub-step 6 dispatch with honest close-report framing. Outcome B → NMI-armed capture follow-up. PM is on call.*
