# DISPATCH: Phase 2 OBSERVE — landed-work inventory + remaining-surface map (Cerebro)

- Author: PM
- Date: 2026-05-24
- Agent: Cerebro (Claude — deep research + synthesis lane)
- PL ratification: this session, "yes" to the OBSERVE → THINK → PL-ratification → PLAN → /simplify → BUILD-N cadence with OBSERVE as a single Cerebro dispatch.
- Scope class: worker execution under PM dispatch; PM consumes output, ratifies, then dispatches THINK as a separate step.
- Bible amendment: none anticipated. OBSERVE is purely inventory + question-surfacing per Bible Article 14.
- Inputs available:
  - The just-closed pre-Phase-2 audit chain:
    - `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md` (`5da364d`)
    - `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md` (Day 1, `c2a4411`)
    - `Plans/CEREBRO-OUTPUT-2026-05-24-audit-day-2-findings.md` (Day 2, `bb1b679`)
    - `Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md` (`199fb51`)
    - `Plans/PM-DISPOSITION-2026-05-24-f005-close.md` (`5884ffd`)
  - `Plans/CLOSE-REPORT-2026-05-24-f005-forced-kill-zombify.md` (F-005 fix close, `69df60b`)
  - The RAG semantic search service at `http://127.0.0.1:8404` (verified healthy this session, 7,513 points indexed; documented in `CLAUDE.md`).

## 1. The task

Per Bible Article 14, OBSERVE is *"gather context, read prior code, identify constraints."* For Phase 2 in current state, that resolves to four concrete inventory questions plus one design-question-surfacing pass:

1. **What Phase 2 work has already landed.** SMP retrofit (ADR-013 SC-3B sub-commits 1-5) and capability system foundation (ADR-010 BUILD-1) are known closed; the audit confirmed both are foundation-ready. But the full inventory of Phase 2 commits, BUILD subphases, and their dispositions is not currently consolidated in one place.

2. **What Phase 2 work remains, per Bible Article 13's enumeration.** Article 13 names Phase 2 as *"SMP, capability system, buddy+slab allocators, advanced TLB management."* SMP is largely landed; capability system foundation is landed but the full buildout is not; buddy+slab allocators are pending; advanced TLB management is pending beyond ADR-013 Q7's SMP-side closures. Each of these needs decomposition into the actual remaining work surface.

3. **How the pre-Phase-2 audit closure trail integrates as foundation characterization.** The audit produced 5 PASS findings, 4 (b) BUILD-N test items, 2 (d) ADR-tracked deferrals, and 4 closed DEFECTs. The (b) items are explicit absorption candidates for Phase 2 BUILD; the (d) items are explicit deferrals that may or may not surface in Phase 2 BUILD; the PASS findings are the substrate Phase 2 builds on. OBSERVE should consume this trail and surface it as a single inventory entry per item: where in Phase 2 it lands or stays deferred.

4. **What deferred ADRs need revisiting in Phase 2 scope.** Known candidates: ADR-019 sub-phases B/C (IrqSafeMutex migration of mixed-context call sites), ADR-022 fault recovery forward marker (informational; no architectural commitment yet — Phase 2 may or may not promote it), ADR-007 Phase 4 self-kill (deferred, but Phase 2 may add cap-passing-over-IPC that enables it). RAG-search for "deferred", "Phase 2", "Phase 4" should surface others.

5. **Design questions for THINK** — enumerated, *not answered*. Each workstream (cap buildout, allocators, TLB advanced, plus any cross-cutting question) gets a question-batch seed. Goal is to identify what Phase 2 PLAN needs to ratify before BUILD subphases can be enumerated.

## 2. Sources to scan

- **Bible Articles 4, 5, 7, 8, 12, 13** — confirm phase scope and any Phase 2 claims not already captured in audit inventory.
- **ADRs in `docs/ADR-*.md`** — full enumeration; filter for "Phase 2", "BUILD", "deferred", or implementation-status-pending. Already-scanned in audit: ADR-004, 007, 010, 011, 013, 018, 019, 022. Day-1-deferred-but-now-in-scope: ADR-005, 006, 020, plus any not yet read.
- **Phase docs**: `docs/phases/PHASE-1.5b.md`, `docs/phases/PHASE-1.5c.md`, and any `PHASE-2*.md` if they exist (likely not — PLAN hasn't started yet).
- **Git log for Phase 2 BUILD commits**: `git log --oneline | Select-String "Phase 2|BUILD-|ADR-010|ADR-013"`. Identify landed subphases and their close commits.
- **Kernel source for landed Phase 2 features**: `kernel/src/cap/*` (if exists), `kernel/src/sched/scheduler.rs` (SMP-aware portions), `kernel/src/mm/*` (allocator surface), `kernel/src/arch/x86_64/tlb.rs` or equivalent (if exists).
- **Audit chain artifacts** listed in the inputs above.
- **RAG queries** for discovery — recommended starter queries:
  - "Phase 2 BUILD subphase design"
  - "buddy allocator slab allocator design"
  - "advanced TLB management invalidation shootdown"
  - "capability system full buildout cap composition"
  - "deferred Phase 2 work"
  - "open questions Phase 2"
  - The RAG is `http://127.0.0.1:8404`; use `/search/full` for full chunks; CLAUDE.md §"NeoPlutarch RAG" has the curl examples. RAG is the discovery tool; direct file reads are the deep-dive tool. Per CLAUDE.md guidance, don't over-rely on RAG for files you need to read end-to-end.

## 3. Deliverable

**Path:** `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md`

Contents:

1. **Frontmatter** — author=Cerebro, date, scope binding (this dispatch), inputs consumed (with commit hashes for any source that has changed since the dispatch was filed).
2. **Sources scanned** — table mirroring the audit's §3 shape. Every source listed in §2 above either marked COMPLETE or marked DEFERRED with reason.
3. **§1 Phase 2 landed work inventory.** Table or list: workstream → BUILD subphase or ADR closure → close commit hash → load-bearing invariants produced or confirmed. Should answer "what's done."
4. **§2 Phase 2 remaining work surface.** One sub-section per Article-13 workstream (SMP residual, capability system full buildout, buddy+slab allocators, advanced TLB management) plus a "cross-cutting" sub-section if cross-workstream items surface. Each item: scope sketch (one paragraph), known dependencies, known open design questions (catalogued, not answered).
5. **§3 Audit closure integration.** Per audit finding (PASS / (b) / (d) / closed DEFECT), one line: where in Phase 2 it lands. (b) items map to BUILD subphase candidates; (d) items map to revisit triggers; closed DEFECTs are foundation; PASS findings are foundation invariants Phase 2 must preserve.
6. **§4 Deferred ADR revisit candidates.** Each ADR with status "pending" or "informational" or "deferred": disposition for Phase 2 (revisit now / revisit in specific BUILD / leave deferred). Includes ADR-019 sub-phases B/C, ADR-022, ADR-007 Phase 4 item, and any others Cerebro surfaces.
7. **§5 Design question seeds for THINK.** Enumerated questions, grouped by workstream. Format mirrors Phase 4 THINK v0.2 Q1-Q12 shape (each question a single paragraph with the context that makes it answerable). Goal: 8-15 questions total across all workstreams. Cerebro does *not* answer; PM and PL handle THINK in the next step.
8. **§6 Recommended next BUILD subphase candidates.** 2-3 candidate first BUILD subphases (after THINK closes), ranked by dependency-cleanness and impact. PM uses these as input for PLAN.
9. **§7 Open questions for PM** — anything Cerebro hit that needs PM judgment to decide before THINK can start (e.g., scope-class call on whether a particular item is Phase 2 or Phase 3, ambiguity in Article 13's enumeration, etc.).

## 4. Acceptance criteria

- **C1.** Every source listed in §2 above has been scanned (or has a stated reason in the sources-scanned table for non-completion).
- **C2.** Deliverable sections §1-§4 (inventory + integration) are complete.
- **C3.** §5 design questions are enumerated (8-15 questions), grouped by workstream, *not answered*. Each question has enough context that PM and PL can address it in the THINK step without re-deriving the source.
- **C4.** §6 next BUILD subphase candidates: 2-3 candidates with rough dependencies and a one-paragraph scope sketch each. Cerebro recommends; PM/PL ratify ordering in PLAN.
- **C5.** Deliverable filed at the §3 path and pushed to `origin/main`.
- **C6.** Cerebro stops at deliverable file. Does NOT amend any audit artifact, does NOT write ADRs, does NOT propose bible amendments, does NOT answer the THINK questions.

## 5. Out of scope

- **THINK answers.** This dispatch only surfaces questions. Answering them is PM + PL work in the next step.
- **PLAN BUILD subphase ordering.** §6 provides candidates only. Ordering is scope-class (PL-tier).
- **New ADR drafting.** If Cerebro identifies that an ADR is needed (e.g., for a deferred-and-now-active item), flag in §7 and let PM dispatch.
- **Bible amendments.** Same pattern — flag in §7.
- **Any code changes, test additions, or fix dispatches.**
- **Audit (b)-disposition test items.** These are scoped to be absorbed into Phase 2 BUILD; the OBSERVE deliverable maps them to candidate BUILDs in §3 but does not dispatch them.

## 6. Notes for Cerebro

This is the third worker-tier load for Cerebro this session (Day 1 was PM-irregular, Day 2 was proper Cerebro dispatch, this is Phase 2 OBSERVE). The pattern is well-established: heavy thinking happens between sources, deliverable prose stays tight.

**Use the RAG aggressively for discovery.** The audit's Day 2 deliverable was built without RAG (it wasn't operational then). The OBSERVE inventory benefits much more from semantic search across the project's accumulated planning artifacts than the audit did, because OBSERVE is exactly "find the existing thinking that's not yet consolidated." RAG calls are free; use them.

**The RAG returns design context, not just symbols.** A query like "buddy allocator design questions" can surface a 2026-04 dispatch that already enumerated half the open questions. Treat those as input to §5, not as answers to extract; the questions still need to be re-derived against current state, but the existing thinking is the substrate.

**Time-bound.** Estimate 4-8 hours of focused work, similar to audit Day 2. If you cross 8 hours, checkpoint a partial at the deliverable path with a stated continuation point in §7's open questions and PM dispatches a continuation rather than press.

**If a source contradicts the bible or an ADR, that's an open question for PM**, not an ambiguity to resolve. Record in §7.

## 7. Close

When the deliverable is filed and pushed, Cerebro returns a brief note naming the commit hash. PM then:

1. Consumes the deliverable.
2. Reviews §5 design questions and §6 BUILD candidates.
3. Either:
   - **(a)** files a THINK dispatch with the question batch refined and routed to PL for ratification, OR
   - **(b)** if §7 surfaces blocking ambiguities, resolves those first (possibly via a clarifying NOTES exchange or a follow-up Cerebro dispatch) before THINK.
4. Files `Plans/PM-DISPOSITION-*-phase-2-observe-close.md` confirming OBSERVE is closed and naming the next step.

## 8. References

- Bible Articles 13 (phase ledger), 14 (algorithm), 4-8 + 12 (Phase 2 surface).
- The pre-Phase-2 audit chain (inputs section above).
- ADR-010 (cap foundation), ADR-013 (SMP retrofit), ADR-019 (IrqSafeMutex), ADR-022 (fault recovery direction).
- Phase 4 THINK v0.2 Q1-Q12 — `Plans/Plan-Phase-4-THINK-v0.2-2026-05-21.md` or similar (per session memory; Cerebro confirms exact path) — as a reference shape for THINK question format in §5.
- `CLAUDE.md` §"NeoPlutarch RAG" — RAG usage discipline.
