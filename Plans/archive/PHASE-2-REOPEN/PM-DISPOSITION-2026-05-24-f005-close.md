# PM-DISPOSITION: F-005 close — Phase 2 PLAN unblocked

- Author: PM
- Date: 2026-05-24
- Status: F-005 CLOSED. Pre-Phase-2 audit's one blocking item resolved.
  **Phase 2 PLAN may now proceed.**
- Inputs consumed:
  - `Plans/CLOSE-REPORT-2026-05-24-f005-forced-kill-zombify.md` (Jarvis close report, commit `69df60b`)
  - Source fix at commit `9a282f2`
  - GitHub Actions CI run `26373931452` (conclusion: success, headSha matches)

---

## 1. Acceptance-criteria verification

Per `Plans/DISPATCH-2026-05-24-f005-cross-process-zombify-forced-kill-jarvis.md` §2:

| Criterion | Status | Evidence |
|---|---|---|
| **C1** F-005 not reachable | ✓ | `q8c_blocked_kill_foreign_parked` PASS: `sys_kill(B) == 0`, C wakes with -EPIPE, C not zombified, B reaped cleanly, post-reap frame audit clean |
| **C2** INV-IPC-8 lockstep restored by construction | ✓ | Unified-helper shape chosen (preferred). `Channel::extract_parked_if_owned(side, expected_pid)` at `kernel/src/ipc/mod.rs:472` is the ONLY public extraction primitive. Old `extract_my_side_if_blocked` removed from public surface. A third extraction site cannot land without inheriting the pid check by construction. |
| **C3** Test coverage for foreign-parked forced-kill | ✓ | `q8c_blocked_kill_foreign_parked` added |
| **C4** No regression | ✓ | Local full suite `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test` exit 0. CI run `26373931452` success. Adjacent gates `q8c_blocked_kill_walk`, `q8c_close_peer_my_side_extract`, `cap_survive_exit`, `cap_grant_basic`, `cap_grant_missing_meta` all PASS. |
| **C5** Stale comment rewritten | ✓ | `process.rs:952-958` diff confirmed. Old text "first parked thread found through its channel handles is the target" deleted; replacement names `INV-IPC-8/F-005` and the pid-aware discipline by reference. |
| **C6** Close report filed at specified path | ✓ | `Plans/CLOSE-REPORT-2026-05-24-f005-forced-kill-zombify.md` filed at commit `69df60b` |

All six criteria met. No deviations from the dispatch.

## 2. Net findings net

Updates to the audit's net-findings table from
`Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md` §1:

| Disposition | Was | Now |
|---|---|---|
| PARTIAL → (a) immediate fix | 1 (F-003) | 0 — F-003 closed by this fix |
| DEFECT → (a) immediate fix | 1 (F-005) | 0 — F-005 closed |
| DEFECT → (a) COMPLETE | 2 (F-001, F-002) | **4 (+ F-003, F-005)** |
| PARTIAL → (b) BUILD-N test | 4 | 4 (unchanged) |
| PARTIAL → (d) deferral | 2 | 2 (unchanged) |
| PASS | 5 | 5 (unchanged) |

**Open blocking items for Phase 2 PLAN: zero.**

The four (b)-disposition test items remain for absorption into Phase 2
BUILD work (per audit-close §5). The two (d) deferrals remain
ADR-tracked (ADR-007 Phase 4 self-kill, ADR-019 sub-phases B/C). No (c)
bible amendments emerged from the full audit cycle.

## 3. Phase 2 PLAN status

Per `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md`
§10: *"Phase 2 PLAN does not start until the audit's primary deliverable
is filed and any (a)-disposition immediate fixes have landed."*

Both gates satisfied:
- Audit primary deliverable: filed at `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md`
  (Day 1, `c2a4411`) + `Plans/CEREBRO-OUTPUT-2026-05-24-audit-day-2-findings.md`
  (Day 2, `bb1b679`). Closed at `Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md`
  (`199fb51`).
- All (a)-disposition immediate fixes: F-001/F-002 closed in PR #6
  (`4dfb13f`); F-003/F-005 closed in `9a282f2`.

**Phase 2 PLAN may start.** Per Bible Article 14, Phase 2 PLAN's OBSERVE
sub-step can cite this audit + closure trail as the foundation's
characterized correctness boundary. The four (b)-disposition test items
fold naturally into Phase 2's BUILD sub-phases as they encounter the
relevant invariants.

## 4. Observations for the project record

**Bonus structural improvement.** Jarvis exceeded the dispatch's
acceptance criteria slightly: rather than simply parameterizing
`extract_my_side_if_blocked` (the acceptable alternative shape in §2
C2), Jarvis chose to collapse the entire extraction surface to a single
public helper. The old function name no longer exists in the public
API. This is the stronger structural guarantee the dispatch identified
as "preferred" and is precisely what ADR-004 §"Lessons" prescribes —
*"Shared cleanup paths prevent drift. Extract the shared helper at the
moment the second caller appears."* Two appearances, now one helper.
The next appearance, whenever it lands, inherits the discipline.

**Process trajectory.** The full F-005 lifecycle in this session was:
- Day 1 audit (PM-tier, irregular execution) missed F-005.
- Day 2 audit (Cerebro dispatch, proper discipline) found F-005.
- F-005 fix dispatched to Jarvis with preferred and acceptable shapes
  specified.
- Jarvis chose the preferred shape, executed cleanly, and the close
  report self-reports unambiguously against the acceptance criteria.
- PM verification was artifact-level (commits on origin, CI green,
  close report present) plus a single spot-check (C5 comment
  rewrite + C2 surface confirmation). PM did not re-derive the fix.

This is the dispatch-then-triage discipline working as designed. The
audit deliverables, the worker dispatch, and the PM verification each
occupy distinct accountability boundaries; the artifact trail makes
each verifiable independently.

## 5. Open workstream candidates (PL chooses next)

With Phase 2 PLAN unblocked, three candidates exist; PM does not
unilaterally pick between them as ordering is scope-class (PL-tier):

1. **Phase 2 PLAN start.** Bible Article 14 OBSERVE-THINK-PLAN-BUILD/VERIFY
   cadence. OBSERVE sub-step would consume the audit closure trail as
   characterization of the foundation. THINK sub-step opens design
   questions for Phase 2's full capability system, SMP completion items,
   buddy/slab allocators, advanced TLB. Largest workstream by scope.
2. **BUILD-5b serial-console diagnosis.** Adapter arrives 2026-05-24
   (today, ETA window still open). Once installed, the NVMe FLUSH
   durability hypothesis from
   `Plans/NOTES-2026-05-23-nvme-driver-flush-and-completion-review.md`
   gets first-pass confirmation or refutation. Pre-Phase-2 unblocked
   means Phase 3 (BUILD-5b's context) can move with full attention
   instead of competing with audit work.
3. **Audit (b)-disposition test items.** Four tests recommended for
   absorption into the next BUILD subphase. Could be dispatched
   immediately if PL wants to retire the audit's remaining backlog
   before either of (1) or (2). Lowest urgency since they don't block
   anything.

PM recommendation: (2) takes priority when serial arrives, since
hardware time is the scarce resource and the adapter window is now.
(1) is the natural next major workstream after BUILD-5b's NVMe question
is characterized. (3) absorbs into (1)'s Phase 2 BUILD work in due
course.

## 6. References

- `Plans/CLOSE-REPORT-2026-05-24-f005-forced-kill-zombify.md` (Jarvis
  close report, `69df60b`).
- `Plans/DISPATCH-2026-05-24-f005-cross-process-zombify-forced-kill-jarvis.md`
  (the dispatch, `199fb51`).
- `Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md` (audit
  closure naming F-005 as blocker, `199fb51`).
- `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md`
  (binding scope, `5da364d`).
- Source fix commit `9a282f2` (`kernel/src/ipc/mod.rs` +51,
  `kernel/src/sched/process.rs` +4/-5, `kernel/src/test_harness.rs`
  +362, plus minor infrastructure).
- GitHub Actions CI run
  https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/26373931452
  (status: success, head: `9a282f2`).
- ADR-004 §"Lessons" — shared cleanup paths prevent drift.
- INV-IPC-8 (audit inventory) — now structurally guaranteed.
