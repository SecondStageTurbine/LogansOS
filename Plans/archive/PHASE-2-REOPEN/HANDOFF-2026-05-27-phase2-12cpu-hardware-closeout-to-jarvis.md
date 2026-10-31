# HANDOFF: Phase 2 12-CPU hardware closeout to Jarvis

- **Date:** 2026-05-27
- **From:** PM
- **To:** Jarvis
- **Repo:** `github.com/SecondStageTurbine/NeoPlutarch`
- **Workspace:** `E:\AI Automation\AltimitNeo`
- **Current pushed HEAD:** `557c2936051c2ab65b023c522f01418f159bca2a`
- **Status:** 43/43 OptiPlex 7080 full-count 12-CPU hardware scenarios accepted
- **Mode:** closeout audit / publication-prep; no hardware run

---

## 1. Minimum Read Set

Read these before acting:

1. `Plans/CLOSE-REPORT-2026-05-27-phase2-12cpu-hardware-sweep.md`
2. `Plans/BLOCKER-2026-05-25-phase2-hardware-q7c-concurrent-munmap-smp.md`
3. `Plans/BLOCKER-2026-05-26-phase2-hardware-q7c-missing-shootdown-detection.md`
4. `Plans/BLOCKER-2026-05-26-phase2-hardware-q8d-phase-a-coverage-tlb-timeout.md`
5. `Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-12cpu-satisfies-minimum-tier.md`
6. `Plans/PL-RATIFICATION-2026-05-26-lean-patch-rule.md`

## 2. Current Result

PL completed the full 43-scenario OptiPlex 7080 full-count 12-CPU sweep. PM verified each accepted serial log and filed:

```text
Plans/CLOSE-REPORT-2026-05-27-phase2-12cpu-hardware-sweep.md
```

Final accepted terminal marker:

```text
NEO_TEST/1 event=PASS name=irqsafe_mutex_bench spin_mean_ns=103 irqsafe_mean_ns=675 switch_mean_ns=335 overhead_ns=572 threshold_ns=6000
```

Three hardware blockers were found during execution and all are resolved:

- `q7c_concurrent_munmap_smp` terminal handoff: fixed at `4ae4b0184071b5b8dff03836344e7a4f9184a78c`.
- `q7c_missing_shootdown_detection` terminal handoff: fixed at `87b1345639a521bd6a4f8bcabacaa91d8d64a60c`.
- `q8d_phase_a_coverage` shootdown-ready race: fixed at `557c2936051c2ab65b023c522f01418f159bca2a`.

## 3. Task

Perform closeout audit and prepare the publication package. Do not run hardware. Do not edit kernel/source code unless an audit finds a concrete documentation/evidence inconsistency requiring a narrow correction.

Required audit:

1. Confirm `HEAD` and `origin/main` are `557c2936051c2ab65b023c522f01418f159bca2a` or later.
2. Verify the close report's 43 accepted evidence paths exist.
3. For each accepted scenario, verify the expected `NEO_TEST/1 event=PASS name=<scenario>` marker exists.
4. Verify no accepted log contains a same-scenario `event=FAIL` after the accepted PASS marker.
5. Verify special markers:
   - `q7c_shootdown_timeout_panic`: expected-panic PASS.
   - `q8a_concurrent_install_revoke`: `derive_successes` is positive.
   - `irqsafe_mutex_if_restore`: variable CPU/thread/site PASS pattern.
   - `irqsafe_mutex_bench`: numeric bench PASS pattern with `threshold_ns=6000`.
6. Verify the three blocker files are marked resolved and reference their accepted rerun evidence.
7. Run `git diff --check` across touched tracked/untracked text artifacts that will be published.

## 4. Publication Shape

Prepare a commit plan, but do not publish unless PL explicitly says to publish.

Recommended split:

### Commit A - Bible lean patch rule

Files:

```text
docs/bible.md
Plans/PL-RATIFICATION-2026-05-26-lean-patch-rule.md
```

Suggested message:

```text
docs: ratify lean patch rule
```

### Commit B - Phase 2 12-CPU hardware closeout

Files:

```text
Plans/CLOSE-REPORT-2026-05-27-phase2-12cpu-hardware-sweep.md
Plans/BLOCKER-2026-05-25-phase2-hardware-q7c-concurrent-munmap-smp.md
Plans/BLOCKER-2026-05-26-phase2-hardware-q7c-missing-shootdown-detection.md
Plans/BLOCKER-2026-05-26-phase2-hardware-q8d-phase-a-coverage-tlb-timeout.md
Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-12cpu-satisfies-minimum-tier.md
Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-4cpu-pass-ambiguity.md
Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-proceed-full-count-defer-4cpu.md
Plans/PM-DISPOSITION-2026-05-25-q7c-concurrent-munmap-hardware-fail-diagnosis.md
Plans/RESEARCH-2026-05-25-phase2-4cpu-tier-without-bios.md
Plans/HANDOFF-2026-05-25-phase-2-hardware-sweep-to-jarvis.md
Plans/HANDOFF-2026-05-27-phase2-12cpu-hardware-closeout-to-jarvis.md
Plans/INDEX.md
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-*.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-*.serial.log
Plans/evidence/phase2-bare-metal-optiplex-7080-20260527-*.serial.log
```

Suggested message:

```text
plans: close phase 2 12-cpu hardware sweep
```

Do not include `ai_digest.py`.

## 5. Return To PM

Return:

- evidence audit summary;
- missing/extra evidence, if any;
- exact proposed staging list;
- whether `Plans/INDEX.md` needs a navigation update;
- if PL authorized publication, commit hash(es), push result, and final clean/dirty status.
