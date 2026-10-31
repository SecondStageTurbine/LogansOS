# DISPATCH: Phase 2 doc reconciliation (Prometheus)

- Author: PM
- Date: 2026-05-24
- Agent: Prometheus (Qwen3.6-27B — documenting, amending, document cleanup lane)
- PL ratification: not required — pure doc-reconciliation against established commits and ADR amendments. PM-tier operational.
- Scope class: small-scope documentation cleanup. Three discrete header/note amendments. No code, no design.
- Relationship to other workstreams: parallel to Phase 2 PLAN's THINK step (which is currently gated on PL's response to `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` §4). This dispatch does not block THINK and is not blocked by it.

---

## 1. The task

Three documents are stale or carry pre-amendment text and may mislead future PLAN/BUILD work. The Phase 2 OBSERVE pass flagged them in §7.7; per OBSERVE dispatch C6 Cerebro did not amend. This dispatch authorizes the amendments.

### 1.1 `docs/phases/PHASE-2.md` — stale status header

The status line reads (effectively) *"In progress (BUILD-3 active...)"* though:

- BUILD-3 was declared substantially complete on 2026-05-11.
- Phase 3 has been in active development since (BUILD-1 through BUILD-5b).
- Phase 4 PLAN substrate has been ratified (Q1-Q12 batch).
- The pre-Phase-2 audit has closed (PM-DISPOSITION at `199fb51` + F-005 close at `5884ffd`).
- Phase 2 PLAN OBSERVE has closed (this session, `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md`).

The header should reflect Phase 2's actual current status accurately.

**Reframe target:** *"BUILD-1/2/3 substantially complete (2026-05-11). Phase 2 formal close pending — see `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` for remaining work and scope-class items currently with PL."* Exact phrasing is Prometheus's call; the principle is "describes current state, points readers at the live disposition artifact."

### 1.2 `docs/ADR-019-switchmutex-irq-off-by-construction.md` — stale implementation-status header

Header reads *"Implementation status: PENDING"* — but sub-phase A landed at commit `27d1b99`.

**Amendment target:** update the status line to reflect that:
- Sub-phase A: landed at commit `27d1b99` (`IrqSafeMutex` type-enforcement at the wrapper layer)
- Sub-phase B: pending (mixed-context `SwitchMutex` caller migration)
- Sub-phase C: pending (`Channel.state` `without_interrupts` site migration, manual-wrapper removal)

Exact phrasing is Prometheus's call. Format consistent with other ADR status conventions in the project. Both B and C are queued for the in-flight Phase 2 close work per `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` §4.1 / §6 (which is itself currently in PL ratification).

### 1.3 `docs/ADR-013-phase-2-smp-retrofit.md` — Q8.a supersession note

ADR-013 Q8.a's design selection was `RwLock` for the PROCESSES table. Implementation at SC-4 D5 superseded this — PROCESSES stayed on a single `Mutex` because `Thread` was refactored to carry `Arc<Pmap>` directly, taking PROCESSES off the SMP hot path entirely.

The supersession is recorded in SC-4's FM-D5.4 inline note. The Q8.a section itself does not carry the supersession marker; a reader of Q8.a in isolation gets the wrong picture of current implementation.

**Amendment target:** add a one-paragraph note inside (or immediately after) Q8.a stating that Q8.a's RwLock selection was superseded at SC-4 by FM-D5.4's single-Mutex approach, with a reference to the SC-4 close commit `57f3cb1` and to FM-D5.4. Do not re-derive the FM-D5.4 rationale; the note's job is to redirect a Q8.a-only reader to the implementation truth.

---

## 2. Deliverables

Three discrete edits to three files. Single commit acceptable; three separate commits also acceptable per Prometheus's preference.

- `docs/phases/PHASE-2.md` — status header reframe per §1.1
- `docs/ADR-019-switchmutex-irq-off-by-construction.md` — status header amendment per §1.2
- `docs/ADR-013-phase-2-smp-retrofit.md` — Q8.a supersession note per §1.3

## 3. Acceptance criteria

- **C1.** PHASE-2.md status header no longer reads "In progress (BUILD-3 active)"; new wording accurately reflects post-2026-05-11 state and references the PM-DISPOSITION OBSERVE-close artifact.
- **C2.** ADR-019 status header reflects sub-phase A landed at `27d1b99`; B and C still pending.
- **C3.** ADR-013 Q8.a carries a supersession marker pointing to FM-D5.4 and commit `57f3cb1`.
- **C4.** No semantic content beyond the three amendments. No retitling of sections, no reordering, no re-derivation of ratified content. Header + status amendments + one short note in Q8.a only.
- **C5.** Push to `origin/main` with CI green (CI's `paths-ignore` filter from `a434992` should skip the run for docs-only changes; if CI does run for any reason, verify it passes).
- **C6.** Close report filed at `Plans/CLOSE-REPORT-2026-05-24-phase-2-doc-reconciliation.md` listing the commit(s), the verbatim before/after for each header amendment, and the supersession note text added to ADR-013.

## 4. Out of scope

- Any change to ADR-013 Q-item *content* beyond the Q8.a supersession note.
- Any change to ADR-019 sub-phase descriptions, only the status header.
- Any change to PHASE-2.md sections other than the status header line.
- Filing the queued `PM-NOTE-…-phase-2-build-3-substantially-complete.md` referenced in `Q9-EXPLORATION` §7.2 — that's gated on PL's §7.2 ledger-semantics decision in the OBSERVE close PM-DISPOSITION.
- Drafting a Phase 2 close artifact — that's downstream of Phase 2 PLAN's last BUILD per the PL-pending scope decisions.
- Any reformatting of existing prose for style.

## 5. Notes for Prometheus

- This is the second small-scope dispatch under your roster. The first (CI caching + path filter) was PR #1 / `a434992` — clean execution, the path filter from that work should mean this PR doesn't even trigger CI.
- For PHASE-2.md, read the existing status header before writing the replacement. The point isn't to declare "Phase 2 closed" (PL hasn't ratified that yet per the OBSERVE close §7.2) — it's to stop misleading readers about BUILD-3 being "active" when it's substantially complete.
- For ADR-019 and ADR-013, preserve existing prose surrounding the status line / Q8.a content. Surgical edits only.
- If you discover that one of the documents has *additional* staleness beyond §1.1–§1.3 (e.g., another section also referring to BUILD-3 as active), flag it in §7-style open questions in the close report; do not amend beyond the three specified targets without PM acknowledgment.
- `git push` returning status 1 is cosmetic stderr decoration, not an error.

## 6. References

- `Plans/PM-DISPOSITION-2026-05-24-phase-2-observe-close.md` — origin of this dispatch
- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` §7.7 — Cerebro's flagging of these items
- `docs/ADR-013-phase-2-smp-retrofit.md` §FM-D5.4 — the supersession note's referent
- Commits: `27d1b99` (ADR-019 A landed), `57f3cb1` (SC-4 close, FM-D5.4), `e9675c4` (Phase 1.5b close — for date anchoring of the Phase-2 vs Phase-3 timeline)
