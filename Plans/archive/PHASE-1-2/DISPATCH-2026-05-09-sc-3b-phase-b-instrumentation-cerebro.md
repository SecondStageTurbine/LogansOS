# Dispatch: SC-3B Phase B instrumentation — NMI watchdog capture

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-09
**Type:** Investigation-stage dispatch. Per Bible Article 14, this opens a Phase B instrumentation pass following Fix 6.1's failure to close the SC-3B stress gate. Investigation-only: no commits, no kernel source changes, instrumentation applied via stash and discarded after data capture. Goal is data — specifically NMI-captured per-CPU stack traces at hang time — that distinguishes among Hn / H1-variant / H4 in the synthesis ranking.
**Substrate at dispatch:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` (SC-3B Sub-step 5 commit). Backup: `Z:\AltimitNeo\2026-05-09-144314`.

Substrate prep is the first action of this dispatch. The Fix 6.1 patch is currently in the working tree but did not close the gate (5/6 PASS at NEO_SMP=4, run 6 silent-timeout matching the original run-36 signature). Per project-lead ratification 2026-05-09 (option (ii) of PM's substrate-state options), the patch is preserved as a stash, then the working tree is restored to clean `a61e318` for Phase B execution. Phase B runs on uncontaminated substrate.

Stash state at dispatch:

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

After substrate prep (§1) the stash list grows to four:

- `stash@{0}: On main: sc-3b-fix-6-1-attempt` (NEW; bumped to top)
- `stash@{1}: On main: wedge-investigation-8-nmi-watchdog` (was {0})
- `stash@{2}: On main: wedge-investigation-2-instrumentation` (was {1})
- `stash@{3}: On main: h1-experiment-instrumentation` (was {2})

Note the stash-numbering shift. Subsequent §-references in this dispatch use the post-prep numbering. The NMI watchdog stash that Phase B applies is `stash@{1}` post-prep (was `stash@{0}`).

---

## 0. Standing instructions

1. **Investigation only. No commits.** No source code is committed during this dispatch. Working-tree changes from instrumentation are discarded after data capture.
2. **No kernel source changes outside instrumentation.** The only modifications come from applying `stash@{1}` (NMI watchdog). Do not author new kernel code.
3. **No Fix 6.1 mixing.** The Fix 6.1 stash (`stash@{0}` post-prep) is preserved untouched throughout this dispatch. It is not applied. Phase B data must reflect the unmodified `a61e318` substrate plus only the NMI watchdog.
4. **Backup script handling.** With four stashes after §1's prep, confirm `Plans/backup-to-synology.ps1` (the 2026-05-08-refined mechanism) handles four. The mechanism was specified to handle three; if it generalizes to N, fine. If not, raise to PM before proceeding to §2 — PM will defer backup runs during the dispatch and resolve the script gap separately.
5. **Stop conditions are honored** (§6). When data is sufficient or a hard stop fires, halt and report.
6. **PM is on call.** Ping for any clarification, especially on NMI threshold choice and stack-trace classification.

---

## 1. Substrate prep — preserve Fix 6.1, restore clean substrate

### 1.1 Stash the Fix 6.1 patch

Command:

```bash
git stash push -m "sc-3b-fix-6-1-attempt" -- kernel/src/test_harness.rs
```

The `-- kernel/src/test_harness.rs` pathspec scopes the stash to exactly the Fix 6.1 patch (5 insertions, 1 deletion in that one file per the verification log). If `git status` shows any other modified files at dispatch open, halt and ping PM — the working tree should contain only the Fix 6.1 changes.

After the push:

- `git status` should report a clean working tree.
- `git stash list` should show four entries with `stash@{0}` carrying the message `On main: sc-3b-fix-6-1-attempt` and SHA matching the patch's tree.

### 1.2 Verify clean substrate

```bash
git rev-parse HEAD                        # expect a61e318d93321c9e84732869f5de9da96abe3d5f
git status                                # expect "nothing to commit, working tree clean"
git stash list                            # expect 4 entries; sc-3b-fix-6-1-attempt at {0}
docker compose run --rm neo cargo k       # expect PASS, 18 warnings
```

If any of these diverge, halt and ping PM. The Phase B baseline is `a61e318` clean + 18 warnings.

### 1.3 Backup before applying any instrumentation

Run `Plans/backup-to-synology.ps1`. This snapshot captures the four-stash state. Per §0 standing instruction 4, if the script does not handle four stashes correctly, raise to PM before proceeding.

If backup succeeds, proceed to §2.

---

## 2. Apply NMI watchdog instrumentation

### 2.1 Apply stash@{1}

```bash
git stash apply stash@{1}
```

This applies the wedge-investigation-8 NMI watchdog patch on top of the clean working tree. The stash itself is preserved (`apply`, not `pop`); after Phase B, the patch is removed via `git restore`/`git clean`, and the stash remains available for any future investigation needs.

### 2.2 Familiarize with the NMI watchdog mechanism

Cerebro authored this stash during the wedge investigation arc. Before running Phase B, re-read the patch and verify:

- What CPU(s) the NMI watchdog arms.
- The threshold mechanism (timer interrupt counter, RDTSC, or other).
- The NMI handler's stack-trace dump format and target (serial assumed).
- Whether the watchdog terminates the run (calls `debug_exit`) or simply dumps and continues. Either is fine for Phase B's purposes; the data of value is the stack trace, not the post-NMI behavior.
- Any global state the watchdog touches that might alter scheduler timing in a way that hides the bug. If the watchdog itself measurably perturbs timing under non-failure runs, note this for §5 and consider a counterfactual run.

If the patch needs minor adaptation to apply cleanly to the post-Sub-step-5 substrate (e.g., function-name drift, new lines around insertion points), make the adaptation in-tree but do NOT commit. Document the adaptation in the Phase B findings log (§5).

### 2.3 Threshold choice

The synthesis offered two reference thresholds:

- 5 seconds (Claude-3 §3.1) — short enough to fire well before QEMU's 180s timeout; tight enough to localize the hang to within a few-second window.
- 10 seconds (Cerebro §3.7) — gives more headroom for ordinary scheduling latency under heavy QEMU SMP load; reduces false positives on slow but progressing iterations.

PM defers the choice to Cerebro. The decision criterion: choose the threshold that minimizes false-positive NMI fires during a successful 1000-iteration run while still firing within the 180s window when a hang occurs. Cerebro's deeper familiarity with typical iteration timing under NEO_SMP=4 in QEMU is the authority.

If Cerebro is uncertain, default to **10 seconds**. False positives waste a run; missing the hang loses the diagnostic. The conservative choice is the longer threshold.

Document the chosen threshold in the findings log (§5).

### 2.4 Verify the patched substrate builds

```bash
docker compose run --rm neo cargo k
```

Expected: PASS with the 18-warning baseline plus any warnings the NMI watchdog patch introduces. Note any new warnings in the findings log; do not attempt to suppress them.

If the build fails, halt and ping PM. The stash apply may have hit a merge conflict or function-signature drift; PM will help resolve.

---

## 3. The instrumentation run

### 3.1 Run shape

50 boots of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4. Single scenario, single CPU configuration. No supplementary scenarios — Phase B is targeted data collection, not regression coverage.

Run host artifact directory:

```text
target/verify/sc3b-phase-b-nmi/neo-smp4/run-NNN.serial.log
```

Per-run timeout: keep the existing xtask 180s. The NMI watchdog should fire well before 180s if a hang occurs, capturing the trace and (if the watchdog `debug_exit`s) terminating the run. If the watchdog does not `debug_exit`, the post-trace behavior is up to the watchdog's design; the trace itself is the prize.

### 3.2 Per-run capture

For every run that hits the silent-timeout signature OR fires the NMI watchdog (the two should coincide if the watchdog is correctly armed):

- Full serial log preserved (default xtask behavior).
- Identify the watchdog-fire line(s) in the log.
- Identify per-CPU stack trace blocks following the watchdog fire.
- Note timing: how many iterations the orchestrator emitted before silence. This is currently bounded by the absence of per-iteration PROGRESS frames — without them, the iteration count at hang is not directly observable from serial alone, but boot-relative timestamps + the watchdog fire time + the threshold give a rough window.

For runs that PASS:

- Standard PASS frame captured by xtask.
- No special handling needed.

### 3.3 Loop early-stop discipline

Unlike Fix 6.1's gate, Phase B is data collection. The synthesis (§3.1) recommends 50 runs with the expectation that "very high probability (>95%) of capturing a hung state if a failure occurs in 50 runs."

Cerebro decides the early-stop criterion:

- **Aggressive:** stop on the FIRST NMI-watchdog-captured trace. One good trace may be enough to discriminate Hn from H1-variant.
- **Conservative:** continue to 50 runs even after capturing one trace, for two reasons: (a) confirms the failure-pattern is consistent across multiple instances, not idiosyncratic to the first one captured; (b) gives a rough rate estimate (e.g., 3/50 vs. 1/50) usable for ranking.

PM recommendation: **stop at the third NMI trace OR 50 runs, whichever comes first**. Three traces gives enough data to confirm pattern consistency without burning the full 50-run budget if the failure is at ~5–10% rate. If at the originally-observed ~3% rate, three traces in 50 runs is roughly the binomial expectation; running the full 50 is not wasted. Cerebro is empowered to override this if a single trace is unambiguously discriminating.

### 3.4 Counterfactual control (optional)

If at the end of the 50-run loop zero NMI watchdog fires were observed (i.e., the watchdog perturbed timing enough to hide the bug), this is itself diagnostic data. The hang has a fragile timing signature. In that case, run an additional 50-boot loop on **clean `a61e318`** (no NMI watchdog) to confirm the bug still reproduces in absence of instrumentation. This is the heisenbug-risk mitigation per synthesis §3.5 (deferred PROGRESS frames) and §7 (PM's heisenbug concern).

This counterfactual is optional and only triggered by the zero-fire outcome. Do not run it pre-emptively.

---

## 4. Data classification

For each NMI-captured trace, classify the orchestrator and worker states per the synthesis §4.3 distinguishing table. Reproduced for convenience:

| Signal | Hn (CPU load imbalance) | H1-variant (lost wakeup) |
|--------|------------------------|--------------------------|
| Failing iteration's pair | Likely pair 1 or 3 (always recv_first, more migrations) | Any pair |
| iter_count in endpoint_blocked | Very large (>50,000) | Small (<1000) |
| NMI: orchestrator CPU | Inside yield_now in endpoint_blocked OR iteration_done | Inside iteration_done (past endpoint_blocked) |
| NMI: worker CPUs | Multiple workers in yield_now loops | One worker in wait_for_stress_go; one in EndpointState |
| Channel state at time of hang | One side blocked, peer has NOT been signaled | One side blocked, peer HAS been signaled but returned wrong |
| Pattern across failures | Always fails for the same pair | Varies by run |

Per-CPU-trace fields to extract per captured fire:

- **CPU0 (orchestrator):** which function is the IP in. Specifically: is it inside `yield_now`, inside `wait_for_stress_endpoint_blocked`, inside `wait_for_stress_iteration_done`, or somewhere else.
- **CPU1, CPU2, CPU3 (workers):** for each, is the IP in `wait_for_stress_go`, in `block_current_into_with_guard`, in `context_switch_with_release2` asm, in IPC code (`Channel::send`/`recv` body), or in `yield_now`. Note any thread that is parked in EndpointState (visible via channel debug helpers that already exist).
- **Channel state per pair:** if the watchdog can dump channel state (or if it can be inferred from IPC code paths visible in the trace), note which pair has a blocked endpoint and which side.

Note that some of these fields require source-position resolution from the IP — symbol resolution is fine to do offline against the kernel binary. The findings log should include the raw trace and the resolved interpretation side by side.

---

## 5. Findings log

Produce `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md` (or 2026-05-10 if calendar advances). Structure:

```
# SC-3B Phase B NMI watchdog findings

## Substrate
[HEAD SHA, stash list at start, NMI threshold chosen, any patch adaptation notes]

## Run summary
[N runs total, M PASS, K NMI-captured, J counterfactual confirmations if run]

## Per-trace analysis
For each captured trace:
  - Run number and serial log path
  - Wall-clock time from BEGIN frame to NMI fire
  - Watchdog-fire line(s)
  - CPU0 IP + symbol + interpretation (which function, which line region)
  - CPU1-3 IPs + symbols + interpretations
  - Channel state inferences if any
  - Classification per §4 table (Hn / H1-variant / H4 / unknown)

## Pattern across traces
[Same pair? Different pair? Same orchestrator state? Different? Channel states consistent?]

## Counterfactual data (if §3.4 triggered)
[Clean-a61e318 50-run rate, comparison to instrumented loop's NMI-fire rate]

## Recommendation
[For each captured pattern, what hypothesis is supported / refuted]
[Whether a second instrumentation pass (PROGRESS frames + cpu_id tracking per synthesis §3.2-3.4) is needed]
[Whether the data is sufficient to design a kernel-level fix vs. needing another data pass]
```

This is a working-tree artifact. Captured by post-dispatch backup.

---

## 6. Stop conditions and exit handling

### 6.1 Hard stops

Halt and ping PM if:

- The Fix 6.1 stash content is altered (any operation that modifies `stash@{0}` post-prep).
- A commit lands on any branch during the dispatch.
- The build under the NMI patch fails (`cargo k` non-zero exit).
- Backup script reports failure for two consecutive attempts.
- Any kernel source file outside the NMI watchdog patch's footprint is modified.

### 6.2 Data sufficiency stops

Halt the run loop and proceed to findings when:

- Three NMI-captured traces have been collected (per §3.3 PM recommendation), OR
- 50 runs have completed, OR
- 50 runs have completed with zero NMI fires (triggers §3.4 counterfactual), OR
- Two consecutive runs hit silent-timeout WITHOUT NMI firing (NMI threshold may be set too high; halt and ping PM rather than tuning blind).

### 6.3 Exit handling — restore clean substrate after data capture

After findings are written and backed up:

```bash
git restore kernel/src/                    # discard NMI watchdog patch
git status                                 # expect clean
git stash list                             # expect 4 entries unchanged
git rev-parse HEAD                         # expect a61e318
docker compose run --rm neo cargo k        # confirm 18-warning baseline restored
```

The NMI watchdog stash (`stash@{1}` post-prep) is preserved for any subsequent passes. The Fix 6.1 stash (`stash@{0}` post-prep) is preserved for the eventual decision (re-apply, discard, or land separately) after PM's ranking-with-data analysis.

---

## 7. Outcomes and forward path

### 7.1 Outcome A — NMI captures discriminating traces

This is the expected outcome. Two or three traces clearly classify as Hn, H1-variant, H4, or some combination.

Forward path:
1. Cerebro lands findings log.
2. PM reads findings, produces ranking-with-data update to the synthesis (or a fresh PM-NOTE if the ranking shifts substantially).
3. PL reviews ranking-with-data, ratifies the next fix design.
4. PM produces a follow-up dispatch — could be a kernel fix (Cerebro implements), additional instrumentation (synthesis §3.2-3.4 second pass), or a combination.
5. Sub-step 6 (docs + close) remains blocked until the gate closes 200/200.

### 7.2 Outcome B — NMI fires but traces are ambiguous

Multiple traces captured but the per-CPU IPs do not cleanly classify. The hang involves a state the §4 table did not anticipate.

Forward path: PM produces a second-pass instrumentation dispatch adding PROGRESS frames + cpu_id tracking per synthesis §3.2-3.4. Cerebro runs another data pass. PM analyzes combined data. The Hn-vs-H1-variant decision becomes more nuanced; an Hn-prime hypothesis may emerge.

### 7.3 Outcome C — NMI does not fire (heisenbug or threshold too high)

Two sub-cases:

- **C1: 50 runs, zero NMI fires, zero silent-timeouts.** Instrumentation hid the bug. §3.4 counterfactual confirms whether the bug still exists on clean substrate. If yes, the bug has a fragile timing signature; PM produces a follow-up dispatch with less-perturbing instrumentation (likely PROGRESS frames over a longer threshold, or a fundamentally different approach like RDTSC sampling).
- **C2: 50 runs, zero NMI fires, BUT silent-timeouts still occur (per §6.2 last bullet).** NMI threshold is set too high to fire before QEMU's 180s. Halt at the second silent-timeout-without-NMI run, ping PM. Likely fix: lower the threshold and re-run a shorter loop.

### 7.4 Outcome D — Investigation reveals Fix 6.1 was directionally right

If Phase B data shows the constant-direction issue was a contributor but not the only mechanism, PM may recommend re-applying Fix 6.1 alongside a kernel-level fix as a combined commit. The Fix 6.1 stash is preserved for exactly this case.

If Phase B data refutes Fix 6.1's mechanism entirely, the stash is discarded after PM's ranking-with-data closes.

---

## 8. Estimated wall time

- Substrate prep (§1): ~10 minutes including build verification + backup.
- NMI patch apply + verification (§2): ~20–40 minutes including stash review and any adaptation.
- 50-run loop (§3): ~30–90 minutes depending on per-run wall time and how many failures occur (failures take up to 180s each pre-NMI; after NMI fires + dumps + exits, ~10–20s per failure).
- Findings log + classification (§4–5): ~30–60 minutes.
- Restore clean substrate + backup (§6.3): ~10 minutes.

Total: ~1.5–3.5 hours from dispatch acceptance to findings handoff.

---

## 9. Cross-references

**Authority for this dispatch:**
- Project-lead ratification 2026-05-09 of substrate-state option (ii) (stash Fix 6.1; Phase B on clean `a61e318`).
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` §3 (instrumentation strategy; NMI watchdog as primary), §4.3 (distinguishing table), §6 (fix-design framework Phase B feeds into).

**Result that necessitated this dispatch:**
- `Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md` — Fix 6.1 gate run; 5/6 PASS at NEO_SMP=4, run 6 silent-timeout matching original run-36 signature.
- `Plans/DISPATCH-2026-05-09-sc-3b-fix-6-1-cerebro.md` — the executed Fix 6.1 dispatch.

**The substrate this dispatch operates on:**
- HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` — SC-3B Sub-step 5 commit.
- `stash@{1}` post-prep — wedge-investigation-8 NMI watchdog (originally `stash@{0}` pre-prep).

**Investigation reports informing the instrumentation design:**
- `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md` §3.7 — Cerebro's 10s threshold recommendation.
- `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md` §3.1 — Claude-3's 5s threshold recommendation.
- `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md` — Prometheus's investigation (calibration; H7 ruled out by SwitchMutex audit).

**Statistical framing:**
- The post-Fix-6.1 data (1 failure in 6 runs vs. 1 in 36 originally) does NOT establish a rate increase — the 6-run sample is too small. With underlying rate ~3%, P[≥1 failure in 6 runs] ≈ 17%, well within ordinary variance. Phase B does not assume Fix 6.1 made things worse.
- Phase B does not gate on a 200/200 closure standard; it gates on data sufficiency for hypothesis discrimination.

**Governing posture:**
- `docs/bible.md` Articles 1.2 (correctness > coherence > performance), 1.3 (every commit is intended to ship — note: this dispatch produces no commits, so 1.3 doesn't apply directly but informs why Fix 6.1 was correctly not committed), 14 (OBSERVE → THINK → PLAN → BUILD/VERIFY; this is investigation between BUILD and VERIFY), 15.2 (SAFETY comments — applies to any kernel code Cerebro writes during NMI patch adaptation).

**Forward continuity:**
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §4.2 Outcome B — handoff anticipated this exact path.
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` §9 — the verification gate that remains the eventual closure standard for SC-3B.

---

**End of dispatch.**

*Investigation only. No commits. Substrate prep stashes Fix 6.1 as `stash@{0}`. NMI watchdog applied from `stash@{1}` (post-prep numbering). 50-run NEO_SMP=4 loop or 3-NMI-fire early stop, whichever first. Counterfactual on zero-fire outcome. Findings log → PM ranking-with-data → next dispatch. Sub-step 6 remains blocked. Estimated 1.5–3.5 hours.*
