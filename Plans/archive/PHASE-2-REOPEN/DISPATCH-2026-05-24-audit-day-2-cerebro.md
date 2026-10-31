# DISPATCH: pre-Phase-2 audit Day 2 completion (Cerebro)

- Author: PM
- Date: 2026-05-24
- Agent: Cerebro (Claude — deep research + synthesis lane)
- PL ratification: implicit, this session ("lets continue with the audit"
  + reminder that PM dispatches rather than executes). Treat as standing
  authorization for the audit work whose scope was ratified earlier
  today.
- Scope binding: `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md`
  (PL-ratified). The 5-step method in §4 and 4-way disposition split in
  §6 of that scope bind this dispatch.
- Input artifacts:
  - `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md` (Day 1
    partial, commit `c2a4411`) — inventory + evidence classification +
    findings F-001..F-004 already complete. Day 2 picks up from §6
    "Findings (remaining invariants — PENDING)" and §8 "Day 2 work plan."
- Bible amendment: not anticipated from the work in this dispatch.
  Cerebro escalates to PM if a finding's natural disposition is (c)
  bible amendment rather than (a)/(b)/(d).
- Scope class: worker execution under PM dispatch; PM consumes output
  and produces PM-DISPOSITION on Cerebro's return.

## 1. The task

Complete the audit's PENDING work per the Day 1 artifact §6 + §8.
Specifically: failure-mode analysis and disposition for the 10 PENDING
invariants, with the source-side scans those analyses require.

The PENDING invariants are (verbatim from Day 1 artifact §6):

1. INV-IPC-5 — page-translate fault in rendezvous, fault-injection coverage
2. INV-IPC-11 / INV-IPC-12 — verification that `ipc_heap_kill_smoke`
   still PASSes after ADR-013 Q5's deferred-quantum removal
3. INV-PROC-2 — self-kill -EINVAL; future Phase 4 concern, may
   disposition as deferred
4. INV-PROC-5 under SMP — partial F-004 finding already exists; Day 2
   may strengthen
5. INV-PROC-7 — Blocked-target kill -EINVAL → SMP cross-CPU walk per
   ADR-013 Q8.c; source-side verification needed
6. INV-CAP-11 — spawn-inheritance derived-copy descendants in revoke walk;
   test coverage check needed
7. INV-CAP-12 — CLOSE bit enforcement; deferred until `sys_close` ships
8. INV-CAP-13 — kernel-cleanup paths exempt from rights checks;
   source-side verification needed
9. INV-SYNC-2 — IrqSafeMutex implementation status; git log needed
10. INV-NEG-3 — Blocked-thread kill lifted under SMP; source-side
    verification

For each: produce a failure-mode analysis bullet (or explicit "no
plausible failure mode identified after analysis" note) and a proposed
disposition tagged with one of {PASS, PARTIAL→(a/b/c/d), DEFECT→(a/b/c/d)}
per the scope's 4-way split.

## 2. Sources to scan

Cerebro pulls from these. Day 1 artifact §3 marks several as DEFERRED;
this dispatch closes those.

**Bible deep scan:**
- `docs/bible.md` Article 8 (Concurrency and Scheduling) — all sub-clauses,
  especially 8.5.1–8.5.5 as amended by ADR-018. Verify alignment with
  INV-SYNC-1 and look for additional scheduler invariants not captured
  in Day 1.
- `docs/bible.md` Article 12 (Security Model) — capability discipline
  mandates. Verify INV-CAP-* row coverage. Look for invariants the ADR
  layer cited but bible carries verbatim.

**Phase doc:**
- `docs/phases/PHASE-1.5c.md` — 1.5c augmented 1.5b's IPC handoff
  invariants (BUILD-1/2/4 amendments). Capture any 1.5c-specific
  invariants not already reflected in ADR-004/011 or in Day 1's
  inventory.

**Kernel source doc-comment pass:**
- `kernel/src/ipc/mod.rs` — module-level + function-level doc comments.
  Extract any invariant claims not already in ADRs.
- `kernel/src/sched/scheduler.rs` — same.
- `kernel/src/sched/process.rs` — same.
- `kernel/src/sched/thread.rs` — same.
- `kernel/src/sched/kthread.rs` — same (lower priority; sample if time
  permits, skip if not).
- `kernel/src/sched/mod.rs` — same.

**Git verification:**
- INV-SYNC-2 implementation status. Search for `IrqSafeMutex` symbol
  in `kernel/src/sync/` and elsewhere; check `git log` for commits
  touching the type's expected location. ADR-019 records status as
  PENDING with sub-phases A/B/C planned; verify which (if any) have
  landed since 2026-05-10.
- INV-PROC-7 / INV-NEG-3 implementation. Find the commit that implemented
  ADR-013 Q8.c (cross-CPU blocked-thread walk in `kill_thread_by_pid`),
  if it exists. Read the implementation and confirm whether the
  Phase-1.5b -EINVAL-on-Blocked restriction has been lifted in current
  code.

## 3. Deliverable

Cerebro produces one artifact:

**Path:** `Plans/CEREBRO-OUTPUT-2026-05-24-audit-day-2-findings.md`

(Note the `CEREBRO-OUTPUT-` prefix to distinguish worker output from
PM artifacts. PM consumes this and amends `PM-AUDIT-2026-05-24-...md`
or files a follow-on artifact.)

Contents:

1. **Frontmatter** — author=Cerebro, date, scope binding, input artifacts.
2. **Sources scanned** — table mirroring the Day 1 §3 shape, with all
   DEFERRED rows updated to COMPLETE (or with a stated reason if any
   item couldn't be completed).
3. **New invariants found** — if the bible / phase doc / source scans
   surface invariants not in the Day 1 inventory, capture them as
   numbered rows with the same shape (ID, verbatim claim, source,
   evidence classification, lens). These augment, do not replace,
   Day 1's inventory.
4. **Failure-mode analyses** — one per PENDING invariant from §1
   above, plus any new invariants from §3 of this deliverable that
   land as CLAIMED or partial-TESTED.
5. **Proposed dispositions** — one per invariant analyzed in §4,
   tagged per the scope's 4-way split. Recommend, do not decide;
   PM ratifies dispositions in the PM-DISPOSITION step.
6. **Updated net findings table** — counts by disposition class.
7. **Open questions for PM** — anything Cerebro hit that needs PM
   judgment to decide (e.g. invariant claim conflicts with source code;
   disposition unclear; bible amendment seems warranted).

## 4. Acceptance criteria

- **C1.** Every PENDING invariant from §1 above has a failure-mode
  bullet (or an explicit "no plausible failure mode identified" note).
- **C2.** Every PENDING invariant has a proposed disposition tagged
  with one of the 4-way labels (PASS / PARTIAL→(a/b/c/d) /
  DEFECT→(a/b/c/d)).
- **C3.** Every source listed in §2 has been scanned, or has a stated
  reason in the sources-scanned table for non-completion.
- **C4.** Any new invariants found receive inventory rows in the same
  shape as Day 1.
- **C5.** Deliverable filed at the path in §3 and pushed to
  `origin/main`.
- **C6.** Cerebro stops at deliverable file. Does NOT amend the Day 1
  PM-AUDIT artifact; PM does that consumption step.

## 5. Out of scope

- **PM-DISPOSITION drafting.** That's the consumption step, PM-tier.
- **Bible amendments.** If a disposition's natural choice is (c) bible
  amendment, propose it in §5/§7 of the deliverable and let PM file the
  amendment process. Do not write amendment text.
- **Fix dispatches.** If a disposition's natural choice is (a) immediate
  fix, propose it and identify the dispatch shape, but do not write the
  Codex/Jarvis dispatch itself; PM files those.
- **The 180s `cap_survive_exit` flake** from
  `NOTES-2026-05-24-cap-survive-exit-180s-flake-post-ipc-fix.md`. Out
  of scope per audit-scope §9.
- **Driver invariants** including the NVMe FLUSH durability hypothesis.
  Out of scope per audit-scope §9.
- **AI subsystem capability discipline** beyond what general cap
  invariants cite. Out of scope per audit-scope §9.

## 6. Close

When the deliverable is filed and pushed, Cerebro confirms with a brief
return note naming the commit hash. PM then:

1. Consumes the deliverable.
2. Ratifies or revises dispositions.
3. Amends the Day 1 PM-AUDIT artifact with final findings.
4. Files `Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md`
   declaring the audit closed and naming any blocking items for Phase
   2 PLAN start.
5. Files Codex/Jarvis fix dispatches for any (a)-disposition findings.
6. Files BUILD-N test-work items for any (b)-disposition findings.

## 7. Notes for Cerebro

This dispatch is the second worker-tier load this session (the first
was Day 1, which PM produced directly — process irregularity, not
repeating). The scope was PL-ratified; the method is the scope's 5-step
discipline; the input is the Day 1 inventory. Heavy thinking happens
between sources, not in the deliverable's prose — keep the writing
tight.

If a source contradicts an ADR claim, that's a finding, not an
ambiguity — record it with evidence and propose a disposition.

If the work runs longer than expected (>8 hours of focused effort),
checkpoint a partial at the deliverable path and flag in §7's open
questions; PM will dispatch a continuation rather than press.

## 8. References

- `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md`
- `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md` (Day 1
  partial, commit `c2a4411`)
- Roster update memory 2026-05-10 (worker lanes).
- PM operational posture memory 2026-05-10 (PM doesn't execute worker
  tasks; dispatch + triage discipline).
