# Dispatch: β bookkeeping — ADR-016 amendment + INDEX.md sweep

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-08
**Type:** Two-commit bookkeeping dispatch implementing the β outcome (wedge discharged on L+A1, A2 reframed to Phase 2). Per project lead's "proceed with bookkeeping" decision in this conversation.
**Substrate:** Project HEAD `2fe5a70` (`docs: Plans/INDEX.md — sweep through 2026-05-08 governance commits (8a2311d)`). Three preserved stashes (unchanged):

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this dispatch.

**Parallelism note.** Cerebro is currently free (A1 landed at `aac0c11`). Both commits in this dispatch are docs-only and well-suited to Jarvis given the prior INDEX sweep work. PM expects this dispatch to be the only outstanding work until project lead opens SC-3B planning.

---

## 0. Standing instructions

1. **Two focused commits in sequence.** Commit X touches only `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`. Commit Y touches only `Plans/INDEX.md`. No other files, no commingling.
2. **Ordering matters.** Commit X (ADR amendment) lands first; Commit Y (INDEX sweep) lands second. Commit Y references Commit X's SHA in some entry text per §3.
3. **No stash interactions.** Stashes remain untouched.
4. **Build verification before each commit.** `docker compose run --rm neo cargo k` must pass with the existing 18-warning baseline (no new warnings). Docs-only changes should not affect the build, but verify per project pattern.
5. **Backup runs after BOTH commits land.** `Plans/backup-to-synology.ps1`. One backup run captures both commits.

---

## 1. COMMIT X — docs/ADR-016 amendment

### 1.1 Goal

Append a new amendment section at the bottom of `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`. The original ADR body stays unchanged. The amendment records that L's empirical verification supersedes A2's pre-SC-3B framing, A2 is reframed to Phase 2 conformance work, and the wedge-fix-before-SC-3B commitment discharges on L+A1.

### 1.2 Insertion point

The current file ends with this line (verified via `read_text_file --tail 30`):

```
*Three coordinated changes: A2 (scheduler decoupling), A1 (IRQ-safe heap wrapper), Bible amendment 8.5 (encoded discipline). Co-ratified together; implementation lands in stages; wedge-fix-before-SC-3B commitment discharges on Phase 6 verification. Trust but Verify.*
```

Insert the amendment block in §1.3 below **after** that closing italic line. Add one blank line of separation between the original closing line and the new `---` separator that opens the amendment block. The original closing line is preserved as-is — it is the closing of the original ADR body, and the amendment starts fresh below it.

### 1.3 Exact text to append

Append the following block verbatim (including the leading separator and headers):

```
---

## Amendment 2026-05-08 — A2 reframed to Phase 2 conformance work

| | |
|---|---|
| **Status** | Accepted (post-L empirical verification) |
| **Date** | 2026-05-08 |
| **Authority** | Project Lead |
| **Affects** | Decision 1 (A2 — scheduler heap-decoupling) phasing only. Decisions 2 and 3 unchanged. |

### Background

This ADR was originally ratified earlier on 2026-05-08 with three coordinated decisions: A2 (scheduler heap-decoupling) as pre-SC-3B work, A1 (IRQ-safe heap wrapper) as defense-in-depth, and co-ratification of Bible amendment 8.5. The ADR's original "Skip L" framing was issued before Cerebro had reported on the L localized fix dispatch. Cerebro had already begun L by the time the supersession landed in conversation; L was completed and committed at `4c6abf4` (`sched: prevent heap-lock re-entry in reap zombie drain`).

L verified 300/300: 200/200 across `smp_per_cpu_scheduler_smoke` plus 50/50 each across `cap_revoke_local` and `cap_survive_exit` at NEO_SMP=4. Zero historical-signature timeouts. Verification report at `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md`.

A1 implemented in `kernel/src/mm/heap.rs` and committed at `aac0c11` (`mm: ADR-016 IRQ-safe global heap allocator (A1 defense-in-depth)`). The wrapper uses `spin::Mutex` (already a direct dep) around `linked_list_allocator::Heap`, with `without_interrupts(...)` around init/alloc/dealloc. The global heap is now universally IRQ-safe. Bible 8.5.1 conforms structurally for the heap layer.

### What this amendment changes

A2 (scheduler heap-decoupling) is reframed from pre-SC-3B work to **Phase 2 conformance work**, scheduled alongside the buddy+slab allocator subsystem rework already on Phase 2's plate. The technical content of the A2 design (bounded or pre-allocated scheduler storage replacing `VecDeque<Box<Thread>>`) stands as the Phase 2 target; only the timeline shifts.

### Why

- L's 300/300 verification is empirically conclusive for the proven WEDGE-INV-J1 mechanism (`POST_SMOKE_SMP_SCENARIO`). The bug is closed.
- A1 closes the broader heap-lock-re-entry class structurally at the heap layer. Same-CPU deadlock from any process/ISR contention on the global heap is impossible by construction.
- A2 was originally framed as bug fix; with the bug fixed, A2's value is now Bible 8.5.3 structural conformance plus architectural cleanliness, not bug closure. Both naturally align with Phase 2's allocator subsystem work.
- Bible has well-established phased-conformance precedent: Article 7.5 (tensor allocation class, Phase 5+), Article 8.1 (inference scheduler class, Phase 5+) describe goal states reached over phases. Article 8.5.3 conforms at Phase 2; the bible itself does not require immediate conformance.
- Doing A2 now would spend ~2-4 weeks on architectural conformance with no improvement to bug-closure (300/300 doesn't get more conclusive with more verification). Phase 2 is the right phase boundary for the work.

### What does NOT change

- **Decision 2 (A1 IRQ-safe heap wrapper):** implemented at `aac0c11` and in `main`. Status unchanged.
- **Decision 3 (Bible amendment 8.5 co-ratification):** Bible v1.0.5 in `main`. Status unchanged.
- **The architectural goal of Decision 1 (A2):** unchanged. Scheduler ready/zombie queues will use bounded or pre-allocated storage; spawn becomes fallible at the per-CPU bound; the heap-independence invariant becomes structural rather than empirical. Only the *when* shifts.
- **Bible 8.5.3 binding status:** unchanged. Article 8.5.3 stands as project policy. A2 is the conformance milestone; conformance lands at Phase 2.

### Updated phasing of the original Implementation plan

The original "Implementation plan" section's Phases 1-3 stand as written (and have already executed). Phases 4-7 are reframed:

- **Original Phase 4 (A2 design dispatch):** moved to Phase 2 kickoff. PM drafts at Phase 2 launch, not now.
- **Original Phase 5 (A2 implementation):** moved to Phase 2. Implementation lands as part of Phase 2's allocator subsystem rework.
- **Original Phase 6 (verification):** moved to Phase 2 closeout. Verification re-runs against the post-A2 substrate at Phase 2 close.
- **Original Phase 7 (close):** moved to Phase 2 closeout.

### Wedge-fix-before-SC-3B commitment discharge

The commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` (test-runner wedge fixed before sub-commit 3b ships) is **discharged 2026-05-08** by L (`4c6abf4`) with A1 (`aac0c11`) as defense-in-depth. Discharge record at `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md`. SC-3B work is unblocked.

### Cross-references

- Bible Article 8.5 (Critical-section discipline, ratified 2026-05-08, v1.0.5) — A2 implements 8.5.3 conformance at Phase 2.
- Bible Articles 7.5, 8.1 — phased-conformance precedents.
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — formal discharge record.
- `Plans/PM-NOTE-2026-05-08-beta-outcome.md` — project-state record of the β execution.
- `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md` — the 300/300 verification supporting the discharge.
- Phase 2 plan (forthcoming `docs/phases/PHASE-2.md` or equivalent) — receives the A2 work at Phase 2 kickoff.

---

*Amendment closes. Three decisions of original ADR-016 stand; Decision 1 (A2) phasing shifts to Phase 2; Decisions 2 and 3 are in main; wedge discharged on L+A1.*
```

### 1.4 Commit X message

Subject:

```
docs: ADR-016 — Amendment 2026-05-08, A2 reframed to Phase 2 conformance work
```

Body:

```
Appends an Amendment section to docs/ADR-016 recording that the ADR's
Decision 1 (A2 — scheduler heap-decoupling) is reframed from pre-SC-3B
work to Phase 2 conformance work. The shift is driven by L's 300/300
empirical verification (commit 4c6abf4) plus A1's structural backstop
at the heap layer (commit aac0c11) — together they discharge the
wedge-fix-before-SC-3B commitment, leaving A2 as Bible 8.5.3
structural conformance work that aligns naturally with Phase 2's
buddy+slab allocator subsystem rework.

Decisions 2 (A1) and 3 (Bible amendment 8.5) unchanged; both in main.
Decision 1's architectural goal unchanged; only the timeline shifts to
Phase 2.

Bible 8.5.3 stands as binding policy with A2 as the conformance
milestone, following the phased-conformance precedent set by Articles
7.5 and 8.1.

Wedge-fix-before-SC-3B commitment discharged. Discharge record at
Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md. SC-3B
work unblocked.

Per Bible Article 24, this amendment was approved by the project lead
in conversation on 2026-05-08 ("proceed with bookkeeping").

No code changes. Docs-only.
```

### 1.5 Commit X pre-commit checklist

- [ ] `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` viewed in editor; new amendment block reads correctly at the bottom; original body unchanged above the new block.
- [ ] `git diff docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` shows ONLY the appended amendment block. No incidental changes elsewhere in the file.
- [ ] `docker compose run --rm neo cargo k` passes with 18-warning baseline.
- [ ] `git status` shows only `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` modified.

---

## 2. COMMIT Y — Plans/INDEX.md sweep

### 2.1 Goal

Update `Plans/INDEX.md` to reflect:
- Wedge-fix-before-SC-3B commitment is met (move row to "Met Commitments" subsection).
- Four new artifacts added since the prior sweep at `2fe5a70`: A1 dispatch (closed), discharge artifact (historical), β-outcome PM-NOTE (historical), this dispatch (closed).
- Description-update on the original wedge-fix commitment PM-NOTE row.
- New "Met Commitments" subsection structure introduced.

### 2.2 Add new "Met Commitments" subsection

Under the existing "Open Project Commitments" section, after the existing commitments table and the "This section is updated when:" maintenance bullet list, add a new subsection. The cleanest insertion point is just before the heading line `## Artifact Table`.

Insert this block:

```
### Met Commitments

Commitments listed here have been formally discharged. The row format mirrors the open-commitments table; an additional "Met" column records the discharge date and the artifact recording the discharge.

| Commitment | Met | Discharge artifact | Discharge basis |
|---|---|---|---|
| Test-runner intermittent wedge fixed | 2026-05-08 | `COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` | Commits `4c6abf4` (L) + `aac0c11` (A1). 300/300 verification across `smp_per_cpu_scheduler_smoke` (200/200) + `cap_revoke_local` (50/50) + `cap_survive_exit` (50/50) at NEO_SMP=4. Bible 8.5.3 structural conformance scheduled for Phase 2 per ADR-016 Amendment 2026-05-08. |

```

### 2.3 Remove migrated row from "Open Project Commitments" table

The existing row in the "Open Project Commitments" table:

```
| Test-runner intermittent wedge fixed | Before sub-commit 3b ships | `PM-NOTE-2026-05-07-wedge-fix-commitment.md` | Project lead decision via ADR or amendment. PM cannot re-defer. |
```

Remove this row entirely from the open-commitments table. The table will be empty after removal — that's correct for current state. **Do not delete the table itself**; leave the headers intact so future commitments can be added without restructuring. The intent is to leave the table header rows visible so the structure is documented; if Jarvis prefers a "(no open commitments at present)" placeholder note inside the table after the header, that's acceptable.

### 2.4 Update description on existing PM-NOTE-2026-05-07 row

The existing row in the artifact table:

```
| `PM-NOTE-2026-05-07-wedge-fix-commitment.md` | historical | Project commitment: test-runner wedge fixed before sub-commit 3b. PM cannot re-defer; project lead can re-defer only via ADR or amendment. Names the de-escalation rule. Cross-referenced by the "Open Project Commitments" section. |
```

Replace with:

```
| `PM-NOTE-2026-05-07-wedge-fix-commitment.md` | historical | Project commitment: test-runner wedge fixed before sub-commit 3b. **Met 2026-05-08** by L (`4c6abf4`, 300/300 verified) + A1 (`aac0c11`, IRQ-safe heap defense-in-depth). Discharge record at `COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md`. Original commitment text describes the de-escalation rule that was in force from 2026-05-07 through 2026-05-08; preserved for project history. |
```

### 2.5 New artifact-table entries to add

Add the following four entries, alphabetically sorted into the existing table by filename:

```
| `COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` | historical | Formal discharge record for the wedge-fix-before-SC-3B commitment. Commitment met 2026-05-08 by `4c6abf4` (L, 300/300) + `aac0c11` (A1). Documents the 0.45% statistical-evidence figure for L's verification, names what the discharge does and does not cover, and references Phase 2 as the target for Bible 8.5.3 structural conformance via A2. |
| `DISPATCH-2026-05-08-adr-016-A1-irq-safe-heap-cerebro.md` | closed | A1 implementation dispatch (Decision 2 of ADR-016): IRQ-safe global heap allocator wrapper, `IrqSafeHeap` around `linked_list_allocator::Heap` with `spin::Mutex` + `without_interrupts(...)`. Defense-in-depth for Bible 8.5.1 at the heap layer. ~30-50 lines plus SAFETY comments. Committed at `aac0c11`. |
| `DISPATCH-2026-05-08-beta-bookkeeping-jarvis.md` | closed | β outcome bookkeeping dispatch: ADR-016 Amendment 2026-05-08 (A2 reframed to Phase 2) + Plans/INDEX.md sweep (wedge-fix commitment migrated to Met Commitments subsection). Two docs-only commits. Commit X (ADR amendment) at `<COMMIT_X_SHA>`; Commit Y (this INDEX update) at `<COMMIT_Y_SHA>`. |
| `PM-NOTE-2026-05-08-beta-outcome.md` | historical | Project-state record of the β outcome: wedge discharged on L+A1; A2 reframed to Phase 2; SC-3B unblocked. Closes out the WEDGE-INV-J1 → ADR-016 → L → A1 arc. Captures the reasoning, supersession of `PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md` §3-§4 forward-looking sections, and lessons noted (plans-update-when-data-arrives; supersession-headers-need-out-of-band-stop-signal). |
```

For the bookkeeping-dispatch entry, the SHA placeholders work as follows:
- `<COMMIT_X_SHA>` is the SHA of Commit X (ADR amendment) — known at the time you write Commit Y, since Commit X has already landed by then.
- `<COMMIT_Y_SHA>` is the SHA of Commit Y (this INDEX update) — same self-reference as the prior INDEX sweep dispatch's row. Use the **(Preferred)** approach from §2.6 of the prior dispatch (`DISPATCH-2026-05-08-index-sweep-jarvis.md`): commit, capture the resulting SHA, then `git commit --amend` to fill in. Single net commit on `main`.
- If you prefer the alternative ("Committed in this same INDEX commit; SHA recorded post-commit") for `<COMMIT_Y_SHA>`, that's acceptable — same precedent as the prior sweep.

### 2.6 Commit Y message

Subject:

```
docs: Plans/INDEX.md — wedge-fix discharge + β outcome artifacts
```

Body:

```
Updates Plans/INDEX.md to reflect the β outcome:

- Wedge-fix-before-SC-3B commitment migrated from "Open Project
  Commitments" to a new "Met Commitments" subsection. Met 2026-05-08
  by 4c6abf4 (L) + aac0c11 (A1).
- Description on PM-NOTE-2026-05-07-wedge-fix-commitment.md row
  updated to record the met status and discharge basis.
- Four new artifact-table entries: A1 dispatch (closed at aac0c11),
  discharge artifact (historical), β-outcome PM-NOTE (historical),
  this bookkeeping dispatch (closed at this commit).

The ADR-016 amendment that records the β outcome at the docs/ level
landed in the immediately preceding commit; this commit indexes the
working-tree records and updates the commitment ledger.

Open Project Commitments table is now empty. Table header preserved
for future commitments; "Met Commitments" subsection houses the
discharged row.

No code changes. Docs-only.
```

### 2.7 Commit Y pre-commit checklist

- [ ] `Plans/INDEX.md` viewed in editor; new "Met Commitments" subsection reads correctly under "Open Project Commitments"; the open-commitments table is empty (or has placeholder).
- [ ] PM-NOTE-2026-05-07 row description reflects "Met 2026-05-08" with both commit SHAs.
- [ ] Four new artifact-table rows are alphabetically sorted into the existing table.
- [ ] Bookkeeping-dispatch row has Commit X SHA filled in; Commit Y SHA either filled in (after `--amend`) or with placeholder per §2.5 alternative.
- [ ] `git diff Plans/INDEX.md` shows only the changes named above. No incidental whitespace churn.
- [ ] `docker compose run --rm neo cargo k` passes with 18-warning baseline.
- [ ] `git status` shows only `Plans/INDEX.md` modified.

---

## 3. Post-commit actions

After BOTH commits land:

1. `Plans/backup-to-synology.ps1` runs once. Verify the log shows `Stashes to preserve: 3` and the new HEAD SHA recorded (HEAD will be Commit Y's SHA, with Commit X as HEAD~1).
2. Notify PM. PM tracks the β bookkeeping as complete and stands by for project lead's SC-3B planning trigger.

---

## 4. Cross-references

- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` — receives the amendment in Commit X.
- `Plans/INDEX.md` — receives the sweep in Commit Y.
- `Plans/PM-NOTE-2026-05-08-beta-outcome.md` — the project-state record this dispatch implements.
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — the discharge record indexed by Commit Y.
- `Plans/DISPATCH-2026-05-08-adr-016-A1-irq-safe-heap-cerebro.md` — the A1 dispatch row added in Commit Y.
- `Plans/DISPATCH-2026-05-08-index-sweep-jarvis.md` §2.6 — the precedent for self-referential dispatch row entries.

---

**End of dispatch.**

*Two commits, both docs-only. Commit X amends `docs/ADR-016` with the Phase 2 reframe. Commit Y updates `Plans/INDEX.md` with the commitment migration and four new artifact rows. One backup run captures both. PM is on call for any drafting question Jarvis surfaces.*
