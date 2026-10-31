# PM-NOTE: β outcome — wedge discharged on L+A1; A2 reframed to Phase 2

**Date:** 2026-05-08
**Author:** PM
**Type:** Project-state record. Captures the project lead's decision in this conversation to discharge the wedge-fix-before-SC-3B commitment on L's 300/300 + A1's structural defense-in-depth, and to reframe A2 (scheduler heap-decoupling) as Phase 2 conformance work. Closes out the WEDGE-INV-J1 → ADR-016 → L → A1 arc.
**Authority:** Project Lead. PM executes the bookkeeping.

---

## 1. Outcome

The wedge-fix-before-SC-3B commitment from `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` is **met**. Discharge record at `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md`. SC-3B is unblocked.

The discharge basis is two commits:
- `4c6abf4` (L) — closes the proven WEDGE-INV-J1 mechanism. Verified 300/300.
- `aac0c11` (A1) — closes the broader heap-lock-re-entry class structurally at the heap layer.

ADR-016's three-decision frame survives in revised form: A1 (Decision 2) and Bible amendment 8.5 (Decision 3) are in main as ratified; A2 (Decision 1) reframed to Phase 2 conformance work via the 2026-05-08 amendment to ADR-016 (landing in this turn's bookkeeping).

## 2. The reasoning, briefly

ADR-016 was ratified before L's empirical results were in. The ratification framed A2 as bug fix because L was forbidden ("Skip L"). With L in main and verified, A2 is no longer bug fix — it's Bible 8.5.3 structural conformance plus architectural cleanliness, both naturally aligned with Phase 2's allocator subsystem work.

The earlier α framing ("proceed per ratified ADR-016 with A2 as pre-SC-3B work") would have spent ~2-4 weeks on architectural conformance with no improvement to bug-closure (300/300 doesn't get more conclusive with more verification). The β framing ("discharge wedge on L+A1, defer A2 to Phase 2") preserves the architectural goal at the right phase boundary while unblocking SC-3B immediately.

Bible has well-established phased-conformance precedent (Articles 7.5, 8.1 describe goal states reached over phases). Article 8.5.3 conforms at Phase 2; the bible itself doesn't require immediate conformance.

The project lead's question that resolved the false dichotomy was direct: "Well is the issue fixed or not? If it's fixed, why would we do α?" PM had been hedging on a ratified plan when the empirical data made the plan stale.

## 3. What's being executed in this turn

- **ADR-016 amendment** (Commit X, dispatched to Jarvis) — `docs/ADR-016` gets a "Amendment 2026-05-08: A2 reframed to Phase 2 conformance work" section appended. Captures the reframe with rationale, updated phasing, and cross-references.
- **Discharge artifact** — `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` created. Working-tree only.
- **This PM-NOTE** — `Plans/PM-NOTE-2026-05-08-beta-outcome.md`. Working-tree only.
- **INDEX.md update** (Commit Y, dispatched to Jarvis) — `Plans/INDEX.md` updated: wedge-fix-before-SC-3B row migrates from "Open Project Commitments" to a new "Met Commitments" subsection; new entries for the A1 dispatch (closed at `aac0c11`), this PM-NOTE, the discharge artifact, and the bookkeeping dispatch itself; description-update on the original wedge-fix commitment PM-NOTE.

Two commits, both docs-only, single dispatch to Jarvis at `Plans/DISPATCH-2026-05-08-beta-bookkeeping-jarvis.md`.

## 4. What this PM-NOTE supersedes

- **`Plans/PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md` §3-§4 forward-looking sections.** The "Trust but Verify" framing was right at the time of ratification; "skip L" turned out to be impossible in practice (Cerebro had already started). The empirical outcome makes β the right call. The §1-§2 ratification record (Bible amendment + ADR-016 ratified) remains canonical.
- **The original Implementation plan in `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` Phases 4-7** (A2 design + implementation + verification + close as pre-SC-3B work). The amended `docs/ADR-016` is now canonical for A2 phasing.

## 5. What's NOT changing

- L is in main. Stays.
- A1 is in main. Stays.
- Bible v1.0.5 with Article 8.5 binding. Stays.
- `docs/ADR-016` original ratified body stays; the 2026-05-08 amendment is appended, not replacing.
- The three-decision frame of ADR-016 stands; only Decision 1 (A2) phasing shifts.
- No further bible amendments needed. Bible 8.5.3's policy is unchanged; only the conformance milestone updates.

## 6. Forward state

- **SC-3B work begins.** Project lead can dispatch SC-3B planning whenever ready.
- **A2 deferred to Phase 2.** PM drafts the A2 design dispatch at Phase 2 kickoff, not now. No A2 work between now and Phase 2 launch.
- **Bible 8.5.3 stands as binding policy with phased conformance.** A2 at Phase 2 is the conformance milestone.
- **No further L/A1/A2 dispatches expected** until Phase 2 launches the A2 work.
- **Project commitment ledger updated.** "Open Project Commitments" → "Met Commitments" migration for the wedge-fix row at Commit Y.

## 7. Lessons noted

- **Plans update when data arrives.** ADR-016 was a coherent plan when ratified; L's verification was data that made part of the plan stale. PM should be quicker to flag plan-vs-data mismatches and bring options to the project lead, not hold plans as sacred against fresh evidence.
- **Empirical fixes in main beat planned architectural fixes in flight.** Once a fix lands and verifies, follow-up architectural work is conformance, not bug closure. The right phase for conformance work is the phase whose scope naturally includes it.
- **"Skip L" was right when issued and wrong by the time it was readable.** Communication lag with Cerebro made the supersession effectively unenforceable. Going forward, supersession headers on dispatches that may already be in flight should be paired with an explicit "stop signal" (notify the coder out-of-band) rather than relying solely on the dispatch artifact's metadata.

## 8. Cross-references

- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — formal discharge record.
- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` (Amendment 2026-05-08) — the reframe.
- `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md` — empirical evidence for the discharge.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — original commitment.
- `Plans/PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md` — superseded forward-looking sections.
- `Plans/DISPATCH-2026-05-08-beta-bookkeeping-jarvis.md` — the dispatch executing this turn's commits.
- Bible Articles 8.5, 7.5, 8.1 — phased-conformance precedent.

---

**End of note.**

*β outcome: wedge discharged on L+A1; A2 reframed to Phase 2; SC-3B unblocked. The architectural goal of A2 survives at the right phase boundary; immediate work proceeds.*
