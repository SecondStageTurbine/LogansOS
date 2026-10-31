# PM-DISPOSITION: pre-Phase-2 audit closure

- Author: PM
- Date: 2026-05-24
- Status: Audit CLOSED with one blocking item (F-005).
- Scope: `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md`
- Inputs consumed:
  - `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md` (Day 1 partial, commit `c2a4411`)
  - `Plans/CEREBRO-OUTPUT-2026-05-24-audit-day-2-findings.md` (Day 2, commit `bb1b679`)
- Outputs filed: this disposition + `Plans/DISPATCH-2026-05-24-f005-cross-process-zombify-forced-kill-jarvis.md`

---

## 1. Net ratified findings

PM has ratified Cerebro's Day 2 dispositions with two specific revisions (§2). The final findings table:

| Disposition | Count | Findings |
|---|---|---|
| PASS | 5 | INV-IPC-11/12 (REMOVED mechanism, dependent test survives); INV-CAP-12 (deferred-correctly, sys_close not yet shipped); INV-CAP-13 (structural exemption confirmed); INV-NEG-3 / INV-PROC-7 *lift* (TESTED same-process); INV-SEC-1 (Phase-1 closed, TESTED) |
| PARTIAL → (a) immediate fix | 1 | **F-003** (INV-IPC-8 lockstep — broken at Phase 2 BUILD-1 design time; restored by F-005 fix as a single unified helper) |
| PARTIAL → (b) BUILD-N test work | 4 | F-004 (INV-PROC-5 reaper double-cleanup under SMP); INV-IPC-5 (translate-fault injection coverage); INV-CAP-11 (spawn-inheritance descendants in revoke walk); INV-SYNC-5 (ready-queue capacity-bound release-build behavior) |
| PARTIAL → (d) documented deferral | 2 | INV-PROC-2 (Phase 4 self-kill-via-passed-handle, ADR-007 Implementation note 1); INV-SYNC-2 (ADR-019 sub-phases B/C — A landed, mixed-context migration pending) |
| **DEFECT → (a) immediate fix** | **1** | **F-005** (cross-process zombify in forced-kill blocked-walk) — **BLOCKING for Phase 2 PLAN** |
| DEFECT → (a) COMPLETE | 2 | F-001 (INV-IPC-7), F-002 (INV-CAP-6) — fixed in PR #6, commit `4dfb13f` |
| (c) bible amendment | 0 | none |

**Load-bearing result for Phase 2 PLAN:** gated on exactly one item — F-005's (a) fix landing. Once that fix is on `origin/main` with CI green, Phase 2 PLAN may start. The four (b) items absorb into Phase 2's BUILD work; the two (d) items are pre-existing ADR-tracked deferrals that do not block.

---

## 2. Revisions to Cerebro's dispositions

Two changes from Cerebro's Day 2 net table:

**R1 — Day 1 F-003 downgraded from PASS to PARTIAL → (a).** Cerebro's F-005 analysis showed the INV-IPC-8 "lockstep between natural exit and forced kill" invariant was actually violated at Phase 2 BUILD-1 design time, when `extract_my_side_if_blocked` (ipc/mod.rs:468) landed as a parallel extraction path that was not unified with `close_peer`'s `extracted_my` branch. PR #6 made `close_peer` pid-aware without recognizing the parallel path existed — restoring "lockstep" in the wrong sense (lockstep of behavior on one path; divergence on the other). The current state is INV-IPC-8 broken. The F-005 fix restores it by unifying. F-003's disposition is therefore the same fix as F-005's, just named differently for the two invariants it restores.

**R2 — INV-SYNC-5 confirmed in-scope.** Cerebro flagged this as discovered in Day 2 and asked whether it belonged to this audit or a fresh Phase-2-OBSERVE. PM call: in-scope. It is a 1.5b/1.5c scheduler invariant claim (bible 8.5.3 "no heap allocation under scheduler lock") with a known release-build latency hazard (`debug_assert!` only catches it in debug). The test work is in scope; the growth-policy decision (grow-with-alloc-outside-lock vs larger fixed cap vs intrusive list) is Phase-2-architectural and properly deferred until workload surfaces it.

---

## 3. Resolutions to Cerebro's open questions

**Q1 — F-005 fix shape (a1 vs a2).** Choose neither a1 nor a2 verbatim; **choose the unified-helper direction implied by Q4.**

Rationale: ADR-004 §"Lessons" cites *"Shared cleanup paths prevent drift. When two distinct entry points need the same cleanup machinery, extract the shared helper at the moment the second caller appears — not later."* Phase 2 BUILD-1's `extract_my_side_if_blocked` was the second extraction-from-parked-side site, parallel to `close_peer`'s `extracted_my` branch. It should have been unified at landing. It was not. PR #6 fixed one site without recognizing the other as the same pattern. Adding `expected_pid` to only `extract_my_side_if_blocked` (option a1) preserves the bug class — a third extraction site in a future BUILD reproduces the failure. The right fix unifies.

Dispatch acceptance criteria require the bug class be addressed by construction (no caller can extract from a parked side without naming the expected pid), not just F-005 specifically. Implementation details (exact helper signature, refactoring shape) are Jarvis's call.

**Q2 — INV-CAP-11 spawn-inheritance descendants coverage.** Leave PARTIAL → (b). Cerebro's uncertainty about whether `cap_revoke_descendants_walk`'s userland body covers a spawn-inheritance chain is itself the evidence gap. The (b) work is "add the test if absent, or confirm-and-mark-as-PASS if present." Cheap insurance either way; no PM decision needed beyond accepting the disposition.

**Q3 — INV-SYNC-5 in-scope decision.** In-scope, PARTIAL → (b). See R2 above for rationale. Recommended test name: `sched_ready_queue_capacity_pressure` per Cerebro.

**Q4 — unify both extraction paths into one pid-aware helper.** YES. Folded into Q1.

---

## 4. F-005 fix dispatch — filed alongside this disposition

**Path:** `Plans/DISPATCH-2026-05-24-f005-cross-process-zombify-forced-kill-jarvis.md`
**Worker:** Jarvis (Codex, kernel source lane)
**Scope:** fix F-005 (DEFECT) and F-003 (PARTIAL — same fix) by unifying the two parallel extraction sites. Add foreign-parked-grantee forced-kill test coverage.

Dispatch acceptance criteria require:
1. F-005's bug not reachable: forced-kill of a process with a foreign grantee parked on its channel side leaves the grantee correctly woken with -EPIPE, and reaps the donor's own thread.
2. INV-IPC-8 lockstep restored: a single helper or otherwise structurally equivalent guarantee that both natural-exit and forced-kill extraction paths share the same pid-aware semantics. No third extraction site can land without inheriting the pid check by construction.
3. Test coverage for the foreign-parked forced-kill scenario.
4. All existing tests PASS — no regression to F-001, F-002, or the 1.5b/1.5c suite.

See the dispatch file for full criteria, scope binding, and out-of-scope items.

---

## 5. BUILD-N test items (four (b) dispositions)

For absorption into the next available BUILD subphase. Listed here for the next BUILD's PLAN to pick up.

| Source finding | Recommended test name | Scope summary |
|---|---|---|
| F-004 (INV-PROC-5 under SMP) | `smp_concurrent_reap_after_terminate` | Assert no double `close_peer` when `terminate(CPU0)` races `on_thread_reap(CPU1)` on the same exiting process |
| INV-IPC-5 (page-translate fault) | `ipc_rendezvous_translate_fault` | Assert -EFAULT on both arrival orders (first-arrival woken with EFAULT; second-arrival returns EFAULT directly); assert no partial copy to destination buffer |
| INV-CAP-11 (spawn-inheritance descendants) | `cap_revoke_spawn_inheritance_descendants` (OR confirm `cap_revoke_descendants_walk` already covers) | Assert revoke of a parent's root cap reaches a grandchild cap inherited via `sys_spawn` (cross-process derivation chain) |
| INV-SYNC-5 (ready-queue capacity-bound) | `sched_ready_queue_capacity_pressure` | Assert behavior at/above the fixed `READY_INITIAL_CAPACITY=8` boundary; documents current release-build hazard if heap-alloc-under-lock fires |

None of these block Phase 2 PLAN. They roll into Phase 2 BUILD work as test additions. If any escalates to DEFECT during implementation, that's a fresh disposition cycle.

---

## 6. Deferred (d) items

Both are pre-existing ADR-tracked deferrals; no new artifact required.

- **INV-PROC-2 (self-kill -EINVAL runtime test):** ADR-007 Implementation note 1 records the deferral. Becomes constructible in Phase 4 when cap-passing-over-IPC lands. No action; carry forward.
- **INV-SYNC-2 (IrqSafeMutex sub-phases B/C):** ADR-019 records status as sub-phase A landed (commit `27d1b99`), B/C pending. Mixed-context call sites in `Channel.state` still use `SwitchMutex` + manual `without_interrupts(...)` (INV-SYNC-1 TESTED discipline). Not a live correctness gap, only a defense-in-depth gap. Sub-phases B and C are Phase-2-appropriate work; no immediate action.

---

## 7. Audit closure

Per scope §8 acceptance criteria:

- **C1** (every source scanned): MET — Day 1 + Day 2 sources tables complete.
- **C2** (every invariant evidence-classified): MET — 32 Day 1 + 4 Day 2 new = 36 invariants, all classified.
- **C3** (failure-mode bullets for CLAIMED + partial-TESTED): MET via Day 1 F-001..F-004 + Day 2 §4.
- **C4** (every invariant has a finding label): MET.
- **C5** (every PARTIAL/DEFECT has disposition + follow-up path): MET — F-005 fix dispatch path named in §4; (b) items named in §5; (d) items pre-tracked per §6.
- **C6** (primary deliverable filed and pushed): MET — Day 1 at `c2a4411`, Day 2 at `bb1b679`.
- **C7** (PM-DISPOSITION filed): THIS DOCUMENT.

**The pre-Phase-2 audit is CLOSED with one blocking item.**

Phase 2 PLAN start is gated on F-005's fix landing on `origin/main` with full CI green. When that lands, this disposition is the closure record; Phase 2 PLAN's OBSERVE phase can cite this audit as the foundation's characterized correctness boundary.

---

## 8. Process notes

Two process observations worth recording for future audits:

**Process observation 1 — Day 1 was PM-tier execution that should have been Cerebro-dispatched.** PL flagged this mid-session ("you are just the PM"). The Day 2 dispatch corrected the discipline; the Day 1 artifact stays on origin as already-consumed. Going forward, any audit-style work (inventory, evidence classification, failure-mode analysis) is dispatch-class, not PM-execution-class.

**Process observation 2 — Cerebro found in Day 2 exactly what PM missed in Day 1.** F-005 is a direct application of the close_peer seed bug's shape to an adjacent code path. PM read the same source ADRs in Day 1 and did not find F-005. Cerebro's framing ("apply the bug shape to every adjacent code path") was the move that surfaced it. The audit's value was therefore overwhelmingly in Cerebro's Day 2 work, not PM's Day 1 partial. Worker dispatch is the better unit of analysis work; PM consumption + ratification is the better unit of PM work. This is the discipline of process observation 1, validated by the outcome.

---

## 9. References

- `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md` — binding scope.
- `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md` — Day 1 partial (`c2a4411`).
- `Plans/CEREBRO-OUTPUT-2026-05-24-audit-day-2-findings.md` — Day 2 deliverable (`bb1b679`).
- `Plans/DISPATCH-2026-05-24-f005-cross-process-zombify-forced-kill-jarvis.md` — F-005/F-003 fix dispatch.
- `Plans/CLOSE-REPORT-2026-05-24-ci-caching-and-cap-survive-exit.md` — seed event (PR #6).
- Commits: `4dfb13f` (PR #6), `27d1b99` (IrqSafeMutex sub-phase A), `e2b3f4f` (SC-3B sub-step 5c).
- CI run `26364516256` (2026-05-24 full-suite PASS, evidence for INV-IPC-11/12, INV-PROC-7, INV-SEC-1).
