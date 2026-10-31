# HANDOFF — 2026-05-08, mid-day, wedge investigation campaign in flight

**Purpose:** Continuation document for the new chat picking up after this one closes. Bridges from the previous chat's compacted transcript (Synology backup + ADR-015 + early wedge investigation) through today's full investigation pivot, to the in-flight WEDGE-INV-4 dispatch Cerebro is currently running.

**Read first:** This document, then the priority artifacts in §6, then act on §3 when Cerebro returns.

---

## 1. Project context (for the new chat)

**Project:** AltimitNeo — clean-slate Rust kernel, AI-native by Phase 5. Bible is `TheAltimitNeoBible.md` (project file, in context). Currently Phase 2 BUILD-3, sub-commit 2 closed, sub-commit 3a not yet started.

**Project lead:** the PL. Human. The user of this Claude.ai chat. **Has explicitly delegated PM role to Claude** and intends to keep doing so. Per Bible Article 24.1 he is the only authority for project-shape decisions and re-deferrals; PM (you, the new Claude) advises and writes.

**Coder agents:**
- **Cerebro** — kernel/correctness work. Substrate-touching. Writes commits when authorized. Currently in flight (see §3).
- **Jarvis** — docs/hygiene/investigation-prep. Doc-only work primarily. Not in flight as of this handoff.

**Time/date posture:** **Don't worry about clock time.** Project lead explicitly said "lets not worry about time anymore" earlier today. The handoff is dated 2026-05-08 because that's when the work is happening, but there is no urgency framing. Cerebro and Jarvis are AI; they don't sleep. PM (you) and project lead work when work happens.

**Backup posture:** Synology NAS at `\\SynologyServer\` mapped as `Z:\`. Run `Plans\backup-to-synology.ps1` after any commit. Last backup: `Z:\AltimitNeo\2026-05-08-091323\` (HEAD `46984a1`). **Run a fresh backup after Cerebro's Limine commit lands.**

---

## 2. Where the substrate stands RIGHT NOW

```
HEAD: 46984a1 docs: Plans/INDEX.md — post-ADR-015 sweep + open commitments section
HEAD~1: 47972c2 mm: ADR-015 KASLR-tolerant heap placement (Option E: validate + below-kernel fallback)
HEAD~2: afa4d22 docs: Plans/INDEX.md — sweep through sub-commit 2 closure
HEAD~3: 0b5ffb4 sched: Phase 2 BUILD-3 sub-commit 2 — per-CPU schedulers + §Q5 restore + AP GS-base rewire; §Q9 deferred

Working tree: clean on tracked files
Stashes (preserved, do NOT pop or drop):
  stash@{0}: On main: wedge-investigation-2-instrumentation
  stash@{1}: On main: h1-experiment-instrumentation
```

Project root: `E:\AI Automation\AltimitNeo`.

---

## 3. What Cerebro is doing right now (in flight)

**Dispatch:** `Plans/DISPATCH-2026-05-08-wedge-investigation-4-extended-baseline-cerebro.md` (~3-4h work)

**Three components, one commit:**

1. **Apply Limine `timeout: 0`** to `boot/limine.conf`. (Single-line config change.)
2. **Measurement 1:** 30 boots of `kaslr_smoke`. Expect 30/30 PASS, 0 H7.
3. **Measurement 2:** 100 boots of `smp_per_cpu_scheduler_smoke` at NEO_SMP=4 with canonical classifier and signature sub-classification.

**Single small commit lands on main:** the Limine config change only. Commit message drafted in dispatch §6.

**Findings report output:** `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md`.

### 3.1 Decision matrix when Cerebro returns

The dispatch's §5 names three cases. **Combined with WEDGE-INV-3's existing 50/50 PASS, the total post-fix observation count is 150.**

| Case | M2 result (100 runs) | Combined 150 rate | Disposition |
|---|---|---|---|
| **I** | 0/100 IN_KERNEL_SMOKE | 0/150 (CI upper ~2%) | **Likely-discharge path.** Draft regression-guard scenario dispatch + draft commitment amendment. PM does both. |
| **II** | 1-3/100 IN_KERNEL_SMOKE | 1-3/150 (1-2%) | **Signature-dependent.** Sub-classify the timeouts. If all three SMOKE/POST-SMOKE signatures from PATCH1 are present at residual rates, we have a low-rate multi-bug situation; commitment amendment with residual-rate threshold. If only one signature persists, that's the targeted next investigation. PM and project lead decide. |
| **III** | 4+/100 IN_KERNEL_SMOKE | 4+/150 (≥3%) | **Wedge meaningfully present.** WEDGE-INV-3's 0/50 was lucky sampling. Investigation continues; signature breakdown directs the next dispatch. Out-of-band observation likely needed. |

### 3.2 What the new chat does as a first action

When Cerebro pings closure:

1. Read `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` carefully.
2. Verify Case I/II/III classification matches the data (don't take the verdict at face value; check the numbers and any signature breakdown).
3. Run `git log` and `git status` to confirm the Limine commit landed cleanly and nothing else changed.
4. Run a backup (`powershell -ExecutionPolicy Bypass -File "Plans\backup-to-synology.ps1"`) capturing the new substrate.
5. Reach out to project lead with case verdict and recommended next step.

---

## 4. Today's investigation cascade (chronological context)

Substrate evolution today (all commits already on disk):

```
Yesterday:  47972c2  ADR-015 + PATCH 1 (50-boot loop: 0/50 PANIC, 11/50 TIMEOUT)
Today AM:   46984a1  INDEX.md sweep + Open Project Commitments section
```

### 4.1 The pivot

This morning's plan: validate H1 (Jarvis's leading wedge hypothesis: BSP reap lock-free `zombie_count == 0` short-circuit races AP zombie production). Cerebro's H1 experiment instrumented `reap()`, ran 20 boots, refuted H1 in narrow form (0 divergences across 88 active zombie observations).

**Surprise from H1 experiment:** `smp_per_cpu_scheduler_smoke` is misnamed — its `sched::spawn()` enqueues on `current_scheduler()` (the BSP), so its workers are BSP-local. APs never produced zombies during the experiment. H1's race window cannot fire in this scenario as currently implemented.

But the H1 experiment still produced one timeout (run 4), with the smoke test reaching `[ pong ] 5` and reaping `ping`, then silence. The wedge happened in code outside `reap()`.

### 4.2 The signature finding

WEDGE-INV-2 instrumented smoke-test/thread/scheduler paths (NOT reap), ran 30 boots: 29 PASS, 1 TIMEOUT — but the TIMEOUT was H7 (pre-kernel boot/menu stall, 318-byte log). In-kernel wedge: 0/30.

This sharpened the picture: instrumentation appeared to drop the in-kernel wedge from PATCH1's 14% to 0%. Heisenbug interpretation seemed strong.

### 4.3 The H7 finding

Jarvis's H7 investigation swept 2,504 historical serial logs:
- **9 H7 instances catalogued.** All identical 318-byte signature.
- **Diagnosed cause:** Limine boot menu countdown × QEMU emulated CD-ROM I/O race.
- **Reinterpreted historical baselines:** ADR-015 PATCH1's 22% becomes 14% in-kernel (after subtracting 4 H7); WEDGE-INV-2's 3.3% becomes 0%.
- **Canonical three-bucket classifier** proposed (PRE_KERNEL / BOOT_KERNEL_PRE_SMOKE / IN_KERNEL_SMOKE).
- **Held off on commit:** verification would conflict with Cerebro's WEDGE-INV-3.

### 4.4 The clean-baseline finding

WEDGE-INV-3 ran 50 un-instrumented boots of `smp_per_cpu_scheduler_smoke` on `46984a1`. **All 50 PASS.** 0/50 in-kernel wedges. 95% CI upper bound ~6%.

This broke the Heisenbug interpretation: the wedge isn't visible on un-instrumented substrate either. The bug appears effectively absent on `46984a1`.

### 4.5 The signature analysis (the most recent finding, ~30 min before this handoff)

PM read all 7 non-H7 PATCH1 timeout logs to verify Jarvis's attribution. Result:
- **All 7 confirmed in-kernel.** Attribution holds.
- **Three distinct signatures discovered**, NOT one:
  - **SMOKE-MID-PHASE** (runs 13, 15): partial reap during smoke. Matches H1 run-4.
  - **SMOKE-COMPLETION** (runs 21, 40): all smoke threads complete, then silence. Previously unrecognized.
  - **POST-SMOKE-USER-PROCESS** (runs 11, 17, 42): smoke done, neo-init/worker run, wedge during user-process lifecycle. Doesn't involve smoke test at all.

**Implication:** the "wedge" was probably never one bug. It was 2-3 distinct bugs all manifesting as TIMEOUTs. ADR-015 (or some related substrate change) appears to have addressed all of them, but we don't know mechanism.

This is recorded in `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`.

---

## 5. Open project work (queue)

### 5.1 Active commitments (in INDEX.md "Open Project Commitments")

**1. Wedge-fix-before-SC-3B.** Active. Deadline: before sub-commit 3b ships. PM cannot re-defer; project lead can re-defer only via ADR/amendment. Source: `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md`.

**Status as of this handoff:** the de-escalation rule was written assuming the wedge was reproducible and fixable. Current evidence (0/50 on `46984a1`) suggests it's effectively absent by mechanism we don't understand. **The commitment likely needs an amendment** when WEDGE-INV-4 confirms the rate is durable. Amendment is queued — not drafted yet — see §5.2.

### 5.2 Pending work post-WEDGE-INV-4

**If Case I (0/150):**
- Draft `Plans/PM-DECISION-2026-05-08-wedge-commitment-disposition.md` recording the "wedge appears effectively absent by substrate-shift, regression guard discharges the commitment" disposition. Project-lead decision needed.
- Draft regression-guard scenario dispatch for Cerebro: a scenario that tests for all three signatures (SMOKE-MID-PHASE, SMOKE-COMPLETION, POST-SMOKE-USER-PROCESS) at a sample size large enough to catch ~2% rates with high confidence.
- Draft commitment amendment to `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` adding a third de-escalation path: "addressed-by-substrate-shift-plus-regression-guard."

**If Case II (1-3/150):**
- Sub-classify the residual timeouts.
- Decide signature-targeted investigation OR amendment-with-residual-threshold. Project-lead call.

**If Case III (4+/150):**
- Investigation continues. Out-of-band observation methods (QEMU GDB stub, hardware breakpoints, post-mortem state dumps) are the leading candidates per WEDGE-INV-3's Case A guidance.
- The wedge is real and reproducible; we go back to looking for mechanism.

### 5.3 Standing pending work (independent of WEDGE-INV-4 outcome)

**Canonical classifier adoption in `tools/xtask/src/main.rs`.** Per `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1. Small Jarvis dispatch when bandwidth allows.

**Post-mortem on three ADR-015 corrections** (R1 post-INDEX-swap, R2 synthetic constant, PATCH 1 exclusive-end). Lesson worth surfacing: **arithmetic-bearing specs need worked numeric examples in the spec text**, not only in implementation comments. Both R2 and PATCH 1 would have been caught at ratification time if ADR-015 had included a worked numeric example at the canonical-ceiling boundary. PM authors when ready.

**ADR-015 amendment** clarifying exclusive-end semantics in `docs/ADR-015-kaslr-tolerant-heap-placement.md`. Small doc-only commit. Not urgent.

**Sub-commit 3a pre-implementation checkpoint conversation.** Blocked on wedge-investigation closure. Per `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` the composition argument depends on the lock-free `zombie_count == 0` short-circuit being preserved. **Good news from today's findings:** the H1 experiment showed the short-circuit is empirically safe under current workloads — SC-3A's analysis is *less* in tension with reality than the convergence note originally suggested. SC-3A can probably proceed with light addendum once wedge investigation closes.

---

## 6. Priority reading list (for the new chat)

In order of priority — read these to get on the same page:

**Tier 1 — must read before acting on Cerebro's findings:**

1. `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` — Cerebro's findings (will exist when he returns).
2. `Plans/DISPATCH-2026-05-08-wedge-investigation-4-extended-baseline-cerebro.md` — the dispatch itself, including the decision matrix.
3. `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md` — the three-signature finding from PATCH1.
4. This handoff document.

**Tier 2 — context for today's pivot:**

5. `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — H1 refutation.
6. `Plans/WEDGE-INVESTIGATION-2-FINDINGS-2026-05-08.md` — instrumented smoke-path baseline.
7. `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` — Jarvis's H7 catalog and classifier proposal.
8. `Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md` — Cerebro's clean baseline (0/50).
9. `Plans/PM-NOTE-2026-05-08-h7-finding-integration.md` — H7 integration into project state.
10. `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` — convergence note (with §10 amendment after H1 refutation).
11. `Plans/PM-DECISION-2026-05-08-wedge-investigation-continues.md` — decision to keep investigating instead of land Alpha-prime.

**Tier 3 — older context (already in compacted transcript but here for reference):**

12. `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment.
13. `Plans/PM-NOTE-2026-05-07-backup-posture.md` — Synology backup mechanism.
14. `Plans/INDEX.md` — project artifact index.
15. `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` — Jarvis's original wedge investigation (the H1 hypothesis source).

---

## 7. PM working style (so the new chat preserves continuity)

- **Action over discussion** when the project lead has approved a direction. He said "your recommendations" earlier and meant "execute them."
- **Decisive but qualified** — give a clear recommendation, but name what you're not sure about and what the project-lead-only decisions are.
- **Never fake commit messages or close report claims.** The Bible's Article 1.3 ("every commit is intended to ship") is load-bearing. A commit titled "fix the wedge" must actually fix the wedge. If we don't know, the title says we don't know.
- **The convergence-note pattern works:** when an existing analysis is superseded by new evidence, AMEND it in place with a new section labeled clearly (e.g., "§10 — updated picture post-H1 experiment"), don't rewrite history. Future readers see both the original analysis and what it turned out to mean.
- **Coordination matters.** When two coder agents are in flight, ensure their work doesn't interfere (Jarvis correctly held off on H7 verification because it would have conflicted with Cerebro's WEDGE-INV-3). When this happens again, it's normal good judgment, not a failure.
- **Backups after every commit.** Z:\AltimitNeo\<timestamp>\ snapshots are cheap; running them takes ~10 seconds.

---

## 8. The strategic frame the new chat should hold

The wedge investigation has gone through a clean arc today:

1. Started with a leading hypothesis (H1) that seemed structurally compelling.
2. Built a controlled experiment to test it.
3. Refuted it cleanly.
4. Discovered the test scenario was misnamed.
5. Discovered a separate harness bug (H7) that had been polluting historical data.
6. Re-measured on un-instrumented substrate and found the wedge effectively absent.
7. Re-analyzed historical timeout logs and found the "wedge" was actually 2-3 distinct bugs.
8. Currently extending the baseline to 150 observations to confirm the absence is durable.

This is **good kernel engineering**, even though we have not yet identified or fixed any specific mechanism. The discipline of "measure before you fix" caught a wrong fix (Alpha-prime as a wedge fix) before it landed. The convergence between Jarvis's investigation and Cerebro's experiments produced a much sharper picture of the bug's nature than either could alone.

**The honest project-state framing:** we don't know what fixed the wedge. We have a credible story (ADR-015's heap shift disrupted memory-layout-sensitive timing windows for at least three different bugs) but no proof. The regression guard, when it lands, is the structural mechanism that catches the bugs' return if our story is wrong.

**This is the framing the commitment amendment will need to capture honestly.** The commitment can be discharged not by "we found and fixed the bug" but by "we have evidence the bug is effectively absent on the current substrate AND we have a regression guard that will detect its return AND we accept the residual risk of an unknown mechanism." That's defensible kernel engineering even though it's not the textbook outcome.

---

## 9. What the new chat should NOT do

- **Do not declare the wedge "fixed" without WEDGE-INV-4 data.** WEDGE-INV-3 alone is insufficient (50 observations, 6% upper bound).
- **Do not pop the stashes.** They're reference state. They may be useful for future investigations.
- **Do not draft the commitment amendment until WEDGE-INV-4 confirms.** The amendment language depends on which case we're in.
- **Do not bundle multiple work streams into one Cerebro dispatch unless they share a measurement.** WEDGE-INV-4 bundles Limine fix verification with the extended baseline because both want the same kind of multi-boot loop. Don't extend that pattern.
- **Do not assume the new chat knows context the user is holding.** Surface the context. Be explicit about what's an open question.
- **Do not treat the time-of-day as a constraint.** Project lead said don't worry about it.

---

## 10. The very next action when this handoff is read

In the new chat:

1. Read this document (you're doing that).
2. Run `git log --oneline -3` and verify HEAD is `46984a1` (or whatever Limine commit Cerebro lands on top of it).
3. Check for `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` — does it exist yet?
   - **If no:** Cerebro is still running. Standby. Maybe write the post-mortem on three corrections in the meantime, or run a backup, or other §5.3 standing work.
   - **If yes:** read it carefully, verify the case verdict, act per §3.1.
4. Don't make commitments on behalf of the project lead. Recommend; let him decide.

---

**End of handoff.**

*The work in flight: WEDGE-INV-4 measures the wedge rate on 150 un-instrumented post-Limine-fix boots. The decision matrix in §3.1 turns the result into the next move. The strategic frame in §8 is what makes the disposition honest. The commitment in §5.1 is the load-bearing project obligation everything else serves.*
