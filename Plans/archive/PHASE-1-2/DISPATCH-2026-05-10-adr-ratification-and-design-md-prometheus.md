# DISPATCH: ADR ratifications (018/019/020) + Bible v1.5.0 + design.md B1 + ANSPEC filename rename + INDEX/PHASE-2 sweep

**Date:** 2026-05-10 (later session)
**Routed to:** Prometheus
**Type:** Six-commit docs-tier dispatch. Ratifies three ADR proposals filed at end-of-Sub-step-6, amends bible to v1.5.0, surgically updates `docs/design.md` to current identity, renames ANSPEC files in `docs/specs/` to drop the legacy project name, and sweeps INDEX.md + PHASE-2.md for the 2026-05-09/10 backlog.
**Sequencing:** Single-recipient Prometheus dispatch. No parallel agent. All six commits land on a session branch; PL fast-forwards `main` after gate verification; PL pushes to `origin/main` per hybrid git posture (this dispatch's close qualifies as a project-governance landing class, triggering a push event).
**Coordination constraint:** Negative-list — `kernel/`, `boot/`, `tools/`, `userland/`, root build files (`Cargo.toml`, `Cargo.lock`, `Dockerfile`, `docker-compose.yml`) all OFF-LIMITS. Only `docs/` and `Plans/` are touched. The forthcoming Jarvis kernel-rename bundle (separate dispatch, sequential after this one) handles everything else.
**Consumes:** `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md`, `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md`, `Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md`, `Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md` (referenced for cross-links; not modified).

---

## 0. Background

SC-3B closed 2026-05-10 (commit `e2b3f4f`) via the per-call-site `without_interrupts(...)` discipline for five `Channel.state` acquisition sites. The investigation arc named the **Hpw failure mode** (preempt-while-holding-lock + IRQ-off same-CPU waiter → single-CPU self-deadlock).

Three ADR proposals were filed in Sub-step 6 commit 1 (`9ada339`) as carry-forwards for the SC-3B closure. PL has now ratified all three (2026-05-10 evening session), with 018 + 019 linked as a single decision and 020 ratified independently. This dispatch lands the ratified ADRs in `docs/`, amends bible v1.4.0 → v1.5.0 with the Article 8.5.4 expansion, and completes adjacent doc cleanup.

ADR proposal open questions are answered per PM leans in the proposals (no PL override on individual question wording; "ratify as drafted" interpretation = PM leans become the ratified decisions). Each ratified ADR documents the decision explicitly in a "Ratification decisions" section.

---

## 1. Commit 1 — ADR-018 ratification + Bible v1.5.0 amendment

### 1.1 Scope

Convert `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md` to ratified `docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md`, and amend `docs/bible.md` Article 8.5.4 in-place per the proposal's specification. Bump bible to v1.5.0.

### 1.2 New file: `docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md`

Source content: the proposal in `Plans/`. Conversion rules:

- **Replace** `Status: Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits in Plans/ awaiting that decision.` with:
  ```
  Status: Ratified 2026-05-10 by project-lead approval.
  ADR Number: ADR-018.
  Bible amendment effected: Article 8.5.4 expanded; bible v1.4.0 → v1.5.0.
  ```
- **Replace** `Tentative ADR number on ratification: ADR-018 (next available; verify at ratification time).` with `ADR Number: ADR-018.`
- **Replace** `Bible article affected on ratification: Article 8.5.4 (amended to explicitly name the preempt-while-holding case).` with `Bible article affected: Article 8.5.4 (amended this commit).`
- **Replace** the `## Open questions for ratification` section with `## Ratification decisions` containing:
  - **Q1 (audit trigger or rely on 019):** Rely on ADR-019. Structural enforcement via `IrqSafeMutex` type makes manual audit unnecessary for the dual-context case. Plain `spin::Mutex` retention still requires SAFETY-comment-style justification at declaration site for non-dual-context uses (per ADR-019 §"Migration" guidance).
  - **Q2 (inline Plans/ citation or only commit message):** Inline. The bible's new Article 8.5.4 cites `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` §3 and §4 by path. The cross-reference is durable; if the cited file is ever superseded, the supersession marker in the target file (per Bible 24.2 reversibility discipline) handles the redirection.
  - **Q3 (combine with ADR-019):** Yes — ratified together as linked decision. ADR-018 names the discipline; ADR-019 makes it un-bypassable. Both land in this dispatch's commits 1 and 2; ratification dates match.
- **Replace** the `## Project-lead decision required` section with `## Project-lead decision (2026-05-10)` containing: `Ratified as drafted; PM leans applied for all "Open questions for ratification" without explicit override. Three-decision package (ADR-018 + ADR-019 + ADR-020) approved as one ratification cycle; 018+019 linked, 020 independent.`
- Preserve all other sections (Rationale, Proposed amendment, Compatibility with existing bible articles, Implementation discipline, Cross-references) verbatim.

### 1.3 Bible amendment

Modify `docs/bible.md` in two places:

**(a) Article 8.5.4 — replace current text:**

Locate the existing Article 8.5.4 block (currently reads):

```
8.5.4 No blocking while holding a spinlock. A spinlock may not be held across any operation that can block, sleep, yield, or wait on a condition. Operations that acquire other spinlocks are permitted only with documented lock-order discipline; operations that acquire mutexes or semaphores from spinlock-held context are forbidden. Code that needs to perform blocking operations must release all spinlocks first.
```

Replace with the proposal's `## Proposed new Article 8.5.4` text **verbatim**:

```
8.5.4 No blocking, yielding, or being preempted while holding a spinlock.

A spinlock may not be held across any operation that can block, sleep, yield, or wait on a condition. Operations that acquire other spinlocks are permitted only with documented lock-order discipline; operations that acquire mutexes or semaphores from spinlock-held context are forbidden. Code that needs to perform blocking operations must release all spinlocks first.

Additionally — and this is the discipline that SC-3B's Hpw mechanism (commit e2b3f4f, 2026-05-10) was caused by violating: a spinlock acquired in preemptible context, where the same lock may also be acquired in IF=0 (interrupts-disabled) context on the same CPU, MUST be acquired with local interrupts disabled by the preemptible-context acquirer. The kernel-provided `without_interrupts(...)` wrapper is the canonical pattern; type-level enforcement is the subject of ADR-019 (see `docs/ADR-019-switchmutex-irq-off-by-construction.md`).

Failing this discipline produces the **Hpw failure mode**: a preemptible-context holder is preempted by an interrupt; the same CPU then enters an IF=0 acquirer that spins on the lock; the preempted holder cannot resume because the IF=0 spinner monopolizes the CPU; the system is in a single-CPU self-deadlock that an SMP scheduler cannot route around. See `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` §3 and §4 for the canonical analysis and the discipline's first instance.
```

(Note: the proposal text references "ADR-019 proposal at Plans/...". This dispatch's commit 2 ratifies ADR-019 to `docs/ADR-019-...`. The bible amendment text above already uses the `docs/` path for the ratified ADR, not the Plans/ proposal path, since by ratification time ADR-019 is canonical in docs/.)

**(b) Footer — bump version and update "Last amended":**

Current footer:

```
Footer
Document: NeoPlutarch Bible
Version: 1.4.0
Status: Ratified
Ratified: 2026-05-02 (commit aa2c949)
Last amended: 2026-05-10 (Article 16.4 amended — bench-surface expanded to include scheduling fairness with Redox `schedrs` as canonical reference. Prior-art triage at Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md. Earlier same-day amendments: Article 25.1 project rename, Article 25.2 prefix clarification, Article 9 expansion + Article 28 Product Accessibility, Article 9.10 WiFi Phase 7.5 commitment, Article 17 boot/firmware/, Article 26 vendor firmware clarification.)
Authority: Project Lead
```

Replace `Version: 1.4.0` with `Version: 1.5.0`.

Replace the `Last amended:` line with:

```
Last amended: 2026-05-10 (later session) — Article 8.5.4 expanded; explicitly names the Hpw failure mode (preempt-while-holding-lock + IRQ-off same-CPU waiter, single-CPU self-deadlock) and the canonical `without_interrupts()` discipline. Companion structural enforcement ratified at docs/ADR-019-switchmutex-irq-off-by-construction.md. SC-3B closure record at Plans/CLOSE-REPORT-2026-05-10-sc-3b.md. Ratifying ADR at docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md. Earlier same-day amendments preserved from v1.4.0 footer history.
```

### 1.4 Commit message

```
docs(adr,bible): ratify ADR-018; amend Article 8.5.4 for Hpw; bible v1.5.0

Ratifies ADR-018 (Bible Article 8.5.4 clarification — preempt-while-
holding for mixed-IRQ-context locks). Lands the ratified ADR file in
docs/ and amends bible Article 8.5.4 in place with the proposal's
specified new text. Bumps bible v1.4.0 → v1.5.0.

ADR-018 is the declarative half of a two-part SC-3B carry-forward
(declarative discipline + structural enforcement). ADR-019 (structural,
type-system enforcement via IrqSafeMutex) ratifies in the next commit
of this dispatch as the linked companion.

Open questions for ratification answered per PM leans:
- Audit trigger or rely on ADR-019: rely on ADR-019.
- Inline Plans/ citation or commit-message-only: inline (durable
  cross-reference).
- Combine with ADR-019: yes, linked decision.

Cross-references:
- Plans/CLOSE-REPORT-2026-05-10-sc-3b.md (canonical SC-3B closure)
- Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md (Hpw smoking-gun)
- Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md (source proposal)
```

---

## 2. Commit 2 — ADR-019 ratification

### 2.1 Scope

Convert `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md` to ratified `docs/ADR-019-switchmutex-irq-off-by-construction.md`. No bible amendment (proposal §"Bible articles potentially affected" notes 8.5.5 may gain enforcement-clause references in a later cycle; not load-bearing for ratification).

### 2.2 New file: `docs/ADR-019-switchmutex-irq-off-by-construction.md`

Source content: the proposal in `Plans/`. Conversion rules:

- **Replace** `Status: Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits in Plans/ awaiting that decision.` with:
  ```
  Status: Ratified 2026-05-10 by project-lead approval.
  ADR Number: ADR-019.
  Linked to: ADR-018 (declarative discipline; this ADR is the structural enforcement).
  Implementation status: PENDING — sub-phases A, B, C scheduled in the forthcoming Jarvis kernel-rename bundle dispatch (sub-phase A) and follow-up Jarvis dispatches (sub-phases B and C).
  ```
- **Replace** `Tentative ADR number on ratification: ADR-019 (next available after ADR-018; verify at ratification time).` with `ADR Number: ADR-019.`
- **Replace** `Related: ADR-018 proposal at Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md (companion declarative discipline; this proposal is the structural enforcement).` with `Related: ADR-018 (companion declarative discipline; this ADR is the structural enforcement). Both ratified 2026-05-10 as a linked decision.`
- **Replace** the `## Open questions for ratification` section with `## Ratification decisions` containing:
  - **Q1 (naming — IrqSafeMutex vs rename SwitchMutex):** Keep `SwitchMutex` as established convention. Create `IrqSafeMutex` as a sibling type alongside it. Consolidation deferred — if at sub-phase C the migration audit shows `SwitchMutex` and `IrqSafeMutex` would have identical semantics for all remaining callers, a follow-up ADR proposes the merge. Until then both names coexist with distinct roles.
  - **Q2 (ban plain `spin::Mutex`):** No ban. Plain `spin::Mutex` remains available for locks that are declared to be exclusively-process-context. Declaration requires a SAFETY-comment-style justification at the lock's declaration site naming why no ISR or IF=0-process can acquire the lock. Compile-time enforcement is not added in this ADR; code-review enforcement remains the discipline.
  - **Q3 (re-entrancy):** No re-entrancy. `IrqSafeMutex::lock()` from the same thread that already holds the lock is undefined behavior (typically: spin forever, since `spin::Mutex` underlying primitive is non-re-entrant). Re-entrancy hides bugs; callers structure their critical sections to not nest.
  - **Q4 (perf bench):** Yes. Sub-phase A includes a micro-bench measuring `IrqSafeMutex::lock()` + `drop` cycle cost vs. plain `spin::Mutex::lock()` + drop. The bench is run on the project's standard QEMU configuration; numbers go into the sub-phase A verification log. If the overhead exceeds 50ns per acquire/release pair on the standard config, escalate to PM before sub-phase B.
  - **Q5 (combine with ADR-018):** Yes — ratified together as linked decision. ADR-018 names the discipline; this ADR is the un-bypassable structural enforcement. See ADR-018 §"Ratification decisions" Q3.
- **Replace** the `## Project-lead decision required` section with `## Project-lead decision (2026-05-10)` containing: `Ratified as drafted; PM leans applied for all "Open questions for ratification" without explicit override. Linked-ratification with ADR-018 (both land in this dispatch's commits 1 and 2). Implementation scheduled for the forthcoming Jarvis kernel-rename bundle: sub-phase A (introduce IrqSafeMutex type) lands alongside the kernel rename and ADR-020 LockOwner implementation; sub-phases B (migrate SwitchMutex mixed-context callers) and C (migrate Sub-step-5c-wrapped sites) follow as separate Jarvis dispatches with their own gates.`
- Preserve all other sections (Rationale, Decision context, Concrete proposal API sketch, Implementation phases, Cross-references) verbatim.
- **Update** the "Cross-references" section: replace the Plans/-relative reference to the ADR-018 proposal with the docs/-relative reference to the ratified ADR-018 (`docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md`).

### 2.3 Commit message

```
docs(adr): ratify ADR-019 — SwitchMutex IRQ-off by construction

Ratifies ADR-019 (type-level enforcement of IRQ-safe lock discipline
for mixed-IRQ-context locks). Introduces IrqSafeMutex<T> as a sibling
type to SwitchMutex; plain spin::Mutex remains available for declared
exclusively-process-context locks with SAFETY-comment justification.

Linked ratification with ADR-018 (commit prior); 018 names the discipline,
this ADR makes it un-bypassable in safe Rust.

Open questions for ratification answered per PM leans:
- Naming: keep SwitchMutex + create IrqSafeMutex sibling.
- Ban plain spin::Mutex: no ban; require justification.
- Re-entrancy: NO.
- Perf bench: yes, micro-bench in sub-phase A; escalate if >50ns/cycle.
- Combine with ADR-018: yes, linked decision.

Implementation scheduled in forthcoming Jarvis kernel-rename bundle:
sub-phase A (introduce type) + ADR-020 LockOwner + kernel rename in
one Jarvis dispatch; sub-phases B (migrate SwitchMutex) and C (migrate
Sub-step-5c-wrapped Channel.state sites) follow.

Cross-references:
- docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md
- Plans/CLOSE-REPORT-2026-05-10-sc-3b.md
- Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md
- Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md
- Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md
```

---

## 3. Commit 3 — ADR-020 ratification

### 3.1 Scope

Convert `Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md` to ratified `docs/ADR-020-owner-metadata-production-diagnostic.md`. No bible amendment.

### 3.2 New file: `docs/ADR-020-owner-metadata-production-diagnostic.md`

Source content: the proposal in `Plans/`. Conversion rules:

- **Replace** `Status: Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits in Plans/ awaiting that decision.` with:
  ```
  Status: Ratified 2026-05-10 by project-lead approval.
  ADR Number: ADR-020.
  Related: ADR-018 + ADR-019 (companion proposals; this ADR independent — ratifies on own merits).
  Implementation status: PENDING — lands in the forthcoming Jarvis kernel-rename bundle dispatch alongside ADR-019 sub-phase A.
  ```
- **Replace** `Tentative ADR number on ratification: ADR-020 (next available after ADR-019; verify at ratification time).` with `ADR Number: ADR-020.`
- **Replace** `Related: ADR-018 and ADR-019 proposals (companion). This proposal is independent: it could ratify with or without ADR-018/019.` with `Related: ADR-018 + ADR-019 (companion; declarative + structural for SC-3B Hpw). This ADR is independent — ratified on own merits regardless of 018/019. In this ratification cycle: all three ratify together for operational efficiency, but 020 does not require the other two.`
- **Replace** the `## Open questions for ratification` section with `## Ratification decisions` containing:
  - **Q1 (feature-gate or always-on):** Always-on for development and release builds. Cfg-gated only for size-optimized embedded builds (future, not currently a project target). The 16 bytes per lock is a defensible engineering trade for production-stable diagnostic.
  - **Q2 (apply to plain `spin::Mutex`):** No. Only kernel-owned lock types (`SwitchMutex` and the forthcoming `IrqSafeMutex` per ADR-019) carry `LockOwner`. Plain `spin::Mutex` is an external crate (`spin`); the project does not fork external crates per Bible 4.3 discipline.
  - **Q3 (hold-time stats):** Defer. This ADR lands the foundational owner metadata (cpu, thread_id, acquisition_site, seqlock). Hold-time tracking (timestamp at acquire, reported at NMI) is a follow-up enhancement; a future ADR proposes if needed.
  - **Q4 (seqlock vs alternatives):** Seqlock. Phase D's instrumentation used seqlock and proved the pattern at the diagnostic-readability level required. RCU and simple-ordering alternatives have either higher complexity or weaker consistency guarantees for the NMI-handler read case.
  - **Q5 (independent of ADR-018/019):** Yes, independent. ADR-020 implementation could land before ADR-019, after ADR-019, or alongside. In this ratification cycle: alongside (in the Jarvis bundle), for operational efficiency.
- **Replace** the `## Project-lead decision required` section with `## Project-lead decision (2026-05-10)` containing: `Ratified as drafted; PM leans applied for all "Open questions for ratification" without explicit override. Implementation scheduled in the forthcoming Jarvis kernel-rename bundle dispatch alongside ADR-019 sub-phase A, on the rationale that bundling the LockOwner field addition with the IrqSafeMutex introduction is efficient (both touch the same lock-type files). Cost per lock: 16 bytes plus possible cache-line padding (~32-64 bytes effective), two atomic increments per acquire/release pair.`
- Preserve all other sections (Rationale, Decision context, Concrete proposal, Cost-benefit summary, Cross-references) verbatim.

### 3.3 Commit message

```
docs(adr): ratify ADR-020 — owner metadata as production diagnostic

Ratifies ADR-020 (production-stable lock owner instrumentation on
SwitchMutex and IrqSafeMutex with seqlock-protected NMI reads).

Foundational diagnostic affordance: future deadlock investigations
can read holder identity, CPU, and acquisition site from the NMI
handler in real time, without ad-hoc instrumentation passes. SC-3B's
36-hour Phase A-through-D arc becomes ~6 hours when the metadata is
present at deadlock time.

Independent of ADR-018/019 but bundled in this ratification cycle
for operational efficiency.

Open questions for ratification answered per PM leans:
- Always-on or cfg-gated: always-on dev+release; cfg-gated embedded.
- Apply to plain spin::Mutex: no (external crate; Bible 4.3 discipline).
- Hold-time stats: defer to follow-up.
- Seqlock vs alternatives: seqlock (Phase D-proven pattern).
- Independent of 018/019: yes, ratified independently; bundled in
  Jarvis kernel-rename dispatch for operational efficiency.

Cost: 16 bytes per lock + 2 atomic incs per acquire/release pair.

Cross-references:
- Plans/CLOSE-REPORT-2026-05-10-sc-3b.md §8.1 (carry-forward declaration)
- Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md (proves diagnostic value)
- docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md
- docs/ADR-019-switchmutex-irq-off-by-construction.md
- Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md
```

---

## 4. Commit 4 — `docs/design.md` B1 surgical identity update

### 4.1 Scope

Surgical text update to `docs/design.md`. Bump from `v0.3` (ALTIMIT NEO era) to `v0.4` (NeoPlutarch identity). Preserve all vision/aesthetic narrative content. Update only: header version, §1 Identity table, the embedded boot sequence example, the §"the world is listening" boot framing (now belongs at boot, not as the project tagline — keep as boot dramatic line; do NOT promote to tagline), and any remaining `ALTIMIT NEO`/`altimit-neo` strings in body text that the prior text-replace missed.

Preserve referential mentions of `.hack` and "original ALTIMIT OS" — these are TM-defensible (referential speech for design inspiration, not derivative branding).

### 4.2 Specific edits

**(a) Header (lines ~1-4):**

Current:
```
# ALTIMIT NEO — Design Vision v0.3

> *"Built to survive the next Pluto's Kiss. Built to think alongside you."*

A from-scratch operating system written in Rust, inspired in spirit by the ALTIMIT OS of the .hack universe.
```

Target:
```
# NeoPlutarch — Design Vision v0.4

> *"Built to survive the next Pluto's Kiss. Built to think alongside you."*

A from-scratch operating system written in Rust, inspired in spirit by the ALTIMIT OS of the .hack universe.
```

(Title changes; the `.hack`/ALTIMIT OS reference stays as referential homage.)

**(b) §1 Identity table:**

Current:
```
| **Name** | ALTIMIT NEO |
| **Codename (internal)** | `altimit-neo`, `neo` |
| **Tagline** | *"The world runs on NEO."* |
| **Version scheme** | `0.MAJOR.MINOR` until first bare-metal boot, then `1.x` |
| **License** | `MIT OR Apache-2.0` per Bible Article 26 and ADR-009 |
```

Target:
```
| **Name** | NeoPlutarch |
| **Codename (internal)** | `neoplutarch`, `neo` |
| **Tagline** | *"Intelligence as a kernel primitive."* |
| **Version scheme** | `0.MAJOR.MINOR` until first bare-metal boot, then `1.x` |
| **License** | `MIT OR Apache-2.0` per Bible Article 26 and ADR-009 |
```

(Tagline is the slogan from README, canonically anchored. `neo` retained as codename per Bible 25.2 prefix convention.)

**(c) §"Design pillars" — preserve verbatim.** No edits.

**(d) §"Aesthetic identity" — preserve verbatim.** The "Modern cyberpunk reimagining of the .hack ALTIMIT visual language" line stays — referential homage to design inspiration, not derivative branding.

**(e) §2.1 boot sequence example (around line 50-65):**

Current:
```
[ 0.000 ] altimit-neo v0.5.0 — kernel online
```

Target:
```
[ 0.000 ] neoplutarch v0.5.0 — kernel online
```

(The line `[ 0.241 ] the world is listening.` stays — it's a boot drama line, not the project tagline. The new tagline "Intelligence as a kernel primitive." goes elsewhere as the project identity; "the world is listening." is the boot frame's emotional close.)

**(f) §5 The Boot Sequence — preserve verbatim. Note one phasing-fact correction:**

Current line in §5:
```
The current Phase 1.4 boot frame ends with the framebuffer logo and serial transcript.
```

Update to:
```
The current Phase 2 boot frame ends with the framebuffer wordmark + tagline byline and serial transcript.
```

(Reflects: Phase 1.4 closed, project at Phase 2; A3 boot-identity decision means wordmark + tagline byline at boot, not just logo.)

**(g) §6 "What NEO Is Not" — preserve verbatim.** No edits.

**(h) §7 Roadmap — preserve verbatim, except:** the "Phase 0–1.5: The Substrate (current)" heading is stale — Phase 0-1.5 closed; project at Phase 2. Change heading to "Phase 0–1.5: The Substrate (complete)". Body of that section preserved.

**(i) §8 Open Questions — preserve verbatim.** No edits.

**(j) §9 References:**

Current:
```
- NeoPlutarch Bible (`docs/bible.md`) — the project's constitution.
- ANSPEC-001 (`docs/specs/altimitneo-ai-subsystem-spec.md`) — AI subsystem design specification.
```

The ANSPEC-001 path will be updated in commit 5 (filename rename). For this commit, leave the path as-is; commit 5 fixes it.

Current:
```
- The .hack universe and the original ALTIMIT OS — for the spirit, not the architecture.
```

Preserve verbatim — referential homage.

**(k) Footer:**

Current:
```
*v0.3 — Restructured as a vision document. Architecture moved to Bible + ANSPEC-001 per ADR-001 (monolithic-kernel reaffirmation). The microkernel framing of v0.2 is superseded.*
```

Target:
```
*v0.4 — Identity update for the 2026-05-10 project rename (AltimitNeo → NeoPlutarch). Tagline canonicalized to "Intelligence as a kernel primitive." Vision/aesthetic narrative preserved from v0.3. Referential mentions of `.hack` and original ALTIMIT OS preserved as design-inspiration homage. v0.3 surface preserved in git history; structural carryover from v0.3 to v0.4 is otherwise identity-only.*
```

**(l) Pre-commit grep verification:** after edits, run:
```bash
grep -ci 'altimit' docs/design.md
```
Expected result: small number of hits remaining, all in the form of referential mentions (`.hack ALTIMIT visual language`, `original ALTIMIT OS`, possibly one or two others). If a hit looks like a stale ALTIMIT NEO project reference (positive descriptor, not referential), surface to PM for case-by-case decision.

The expected residual `altimit` hits in the post-edit file are:
- `inspired in spirit by the ALTIMIT OS of the .hack universe` (§"Title preamble")
- `Modern cyberpunk reimagining of the .hack ALTIMIT visual language` (§"Aesthetic identity")
- `the original ALTIMIT OS — for the spirit, not the architecture` (§9 References)

If post-edit hit count differs significantly from these 3 expected referential mentions, STOP and surface to PM.

### 4.3 Commit message

```
docs(design): v0.3 → v0.4 — identity update for 2026-05-10 rename

Surgical identity update to docs/design.md following the 2026-05-10
project rename (AltimitNeo → NeoPlutarch). Vision/aesthetic narrative
preserved unchanged; only identity surface updated:

- Header: "ALTIMIT NEO — Design Vision v0.3" → "NeoPlutarch — Design
  Vision v0.4"
- §1 Identity table: Name, Codename, Tagline updated to NeoPlutarch /
  neoplutarch / "Intelligence as a kernel primitive."
- §2.1 embedded boot sequence example: serial banner updated to
  "neoplutarch v0.5.0"
- §5 phasing-fact update: Phase 1.4 → Phase 2 boot frame description
- §7 roadmap heading: Phase 0-1.5 marked complete
- Footer: v0.3 → v0.4 changelog entry

Preserved verbatim (referential homage, TM-defensible):
- ".hack ALTIMIT visual language" (aesthetic identity)
- "the original ALTIMIT OS — for the spirit, not the architecture"
  (§9 references)
- "inspired in spirit by the ALTIMIT OS of the .hack universe" (preamble)

The ANSPEC-001 path reference (docs/specs/altimitneo-ai-subsystem-spec.md)
is intentionally NOT updated in this commit; commit 5 of this dispatch
renames the ANSPEC files in docs/specs/ and fixes the four cross-
references in docs/ atomically.

Cross-references:
- docs/bible.md Article 25 (project name authority)
- README.md (canonical home of the "Intelligence as a kernel primitive."
  slogan)
- Plans/HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md
```

---

## 5. Commit 5 — ANSPEC filename renames + docs/ cross-reference updates

### 5.1 Scope

Rename two ANSPEC files in `docs/specs/` to drop the legacy project-name prefix, and update the four cross-references to `altimitneo-ai-subsystem-spec.md` across `docs/`. The `altimitneo-ai-capability-semantics-spec.md` stub has zero cross-references at HEAD (verified via `git grep`); rename is filename-only with no reference fixes needed.

### 5.2 File renames

Use `git mv` to preserve rename detection in history:

```bash
git mv docs/specs/altimitneo-ai-subsystem-spec.md \
       docs/specs/neoplutarch-ai-subsystem-spec.md

git mv docs/specs/altimitneo-ai-capability-semantics-spec.md \
       docs/specs/neoplutarch-ai-capability-semantics-spec.md
```

### 5.3 Cross-reference updates

Update the four cross-references in `docs/`:

| File | Line | Current | Target |
|---|---|---|---|
| `docs/design.md` | ~16 | `(`docs/specs/altimitneo-ai-subsystem-spec.md`)` | `(`docs/specs/neoplutarch-ai-subsystem-spec.md`)` |
| `docs/design.md` | ~305 (§9 References) | `(`docs/specs/altimitneo-ai-subsystem-spec.md`)` | `(`docs/specs/neoplutarch-ai-subsystem-spec.md`)` |
| `docs/ADR-001-monolithic-kernel-reaffirmation.md` | ~108 | `` `docs/specs/altimitneo-ai-subsystem-spec.md` `` | `` `docs/specs/neoplutarch-ai-subsystem-spec.md` `` |
| `docs/HANDOFF.md` | ~145 | `docs/specs/altimitneo-ai-subsystem-spec.md` | `docs/specs/neoplutarch-ai-subsystem-spec.md` |

Verify via:
```bash
git grep -n -F 'altimitneo-ai' -- docs/
```
Expected result after edits: zero hits in `docs/`. (Plans/ hits — currently 1 in `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` — are intentionally preserved as historical record.)

### 5.4 ANSPEC file internal content

The ANSPEC-001 file itself contains its own document ID and title. Check post-rename:

```bash
head -20 docs/specs/neoplutarch-ai-subsystem-spec.md
```

If the file's internal title or document ID references the old project name (e.g., "AltimitNeo AI Subsystem"), update to NeoPlutarch. Check both files. Standard text-replace `AltimitNeo` → `NeoPlutarch` and `altimit-neo` → `neoplutarch` within the file content; preserve referential mentions of `.hack`/ALTIMIT OS if any.

### 5.5 Commit message

```
docs(specs): rename ANSPEC files for 2026-05-10 project rename

Renames two files in docs/specs/ to drop the legacy project-name prefix:

- docs/specs/altimitneo-ai-subsystem-spec.md
    → docs/specs/neoplutarch-ai-subsystem-spec.md (ANSPEC-001)
- docs/specs/altimitneo-ai-capability-semantics-spec.md
    → docs/specs/neoplutarch-ai-capability-semantics-spec.md (ANSPEC-002 stub)

Uses `git mv` to preserve rename detection in history.

Updates four cross-references in docs/:
- docs/design.md §"About This Document"
- docs/design.md §9 References
- docs/ADR-001-monolithic-kernel-reaffirmation.md
- docs/HANDOFF.md

Plans/ cross-references intentionally preserved as historical record
per Bible 18.1 (Plans/ artifacts are dated working records).

ANSPEC file internal content also updated where the file's own title
or document ID referenced AltimitNeo; preserved referential .hack/
ALTIMIT OS mentions as design-inspiration homage.

Cross-references:
- docs/bible.md Article 25 (project name authority)
- Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md (audit informing
  the docs-side completion of the rename surface)
```

---

## 6. Commit 6 — INDEX.md sweep + PHASE-2.md ADR ledger update

### 6.1 Scope

`Plans/INDEX.md` sweep for 2026-05-10 artifacts post-Sub-step-6 (the prior INDEX sweep in Sub-step 6 commit 5 captured 2026-05-09/10 backlog through that commit's authoring time; new artifacts since then need entries). `docs/phases/PHASE-2.md` ADR ledger update to add ADR-018/019/020 entries.

### 6.2 INDEX.md additions

Add entries for the following artifacts created since Sub-step 6 commit 5 landed:

- `Plans/DISPATCH-2026-05-10-readme-slogan-addendum-claude-3.md` — status `closed` (commit `3a2cb70`).
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` — status `historical` (operative standing-rule; supersedes 2026-05-09 roster notes).
- `Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md` — status `active` (consumed by forthcoming Jarvis kernel-rename bundle dispatch).
- `Plans/DISPATCH-2026-05-10-adr-ratification-and-design-md-prometheus.md` (this file) — status `active` (in-flight at INDEX sweep time; will be `closed` once this dispatch's commit 6 lands).

Insert each in the appropriate INDEX section (likely "Recent Artifacts" or by date; follow the existing INDEX.md structural pattern).

Also: update the "Open Project Commitments" section if any of the new artifacts introduce commitments. ADR-019 implementation (Jarvis sub-phases A/B/C) is a project commitment — add a row:

| Commitment | Deadline | Source artifact | De-escalation rule |
|---|---|---|---|
| ADR-019 sub-phase A (introduce IrqSafeMutex) | 2026-05-14 (4 days, working estimate) | `docs/ADR-019-switchmutex-irq-off-by-construction.md` | Re-deferral requires PL decision + ADR amendment per Bible 24.1 |
| ADR-019 sub-phase B (migrate SwitchMutex mixed-context callers) | 2026-05-21 (1 week post-A) | `docs/ADR-019-switchmutex-irq-off-by-construction.md` | Same |
| ADR-019 sub-phase C (migrate Sub-step-5c-wrapped Channel.state sites) | 2026-05-28 (1 week post-B) | `docs/ADR-019-switchmutex-irq-off-by-construction.md` | Same |
| ADR-020 implementation (LockOwner field on SwitchMutex + IrqSafeMutex) | 2026-05-14 (bundled with sub-phase A) | `docs/ADR-020-owner-metadata-production-diagnostic.md` | Same |

Deadlines are PM working estimates; PL may revise.

### 6.3 PHASE-2.md ADR ledger

`docs/phases/PHASE-2.md` includes an ADR ledger (or equivalent section listing relevant ADRs for the phase). Add entries for the three new ADRs:

- **ADR-018** (Bible Article 8.5.4 clarification) — declarative discipline for preempt-while-holding case. Ratified 2026-05-10.
- **ADR-019** (SwitchMutex IRQ-off by construction) — structural enforcement via `IrqSafeMutex` type. Ratified 2026-05-10. Implementation: sub-phases A/B/C in forthcoming Jarvis dispatches.
- **ADR-020** (owner metadata as production diagnostic) — `LockOwner` field on kernel-owned lock types. Ratified 2026-05-10. Implementation: alongside ADR-019 sub-phase A.

Format the entries to match the existing PHASE-2.md ADR-ledger style (Prometheus established the format in Sub-step 6 commit 5).

Also: if PHASE-2.md has a "Sub-commit status" section, mark SC-3B as fully closed (status `complete`, closure date 2026-05-10, closure commit `e2b3f4f`). If the marker is already present from Sub-step 6 commit 5, no change.

### 6.4 Commit message

```
docs(phases,index): sweep 2026-05-10 post-Sub-step-6 artifacts

Plans/INDEX.md sweep for artifacts created since Sub-step 6 commit 5
(the prior sweep). New entries:
- DISPATCH-2026-05-10-readme-slogan-addendum-claude-3.md (closed)
- PM-NOTE-2026-05-10-agent-roster-update.md (historical, operative)
- KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md (active)
- DISPATCH-2026-05-10-adr-ratification-and-design-md-prometheus.md
  (active, in-flight at sweep time)

Open Project Commitments updated with four ADR-019/020 implementation
sub-commitments (sub-phases A/B/C + LockOwner) with PM-estimated
deadlines; PL may revise.

docs/phases/PHASE-2.md ADR ledger updated with ADR-018/019/020
entries (ratified 2026-05-10).

Cross-references:
- docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md
- docs/ADR-019-switchmutex-irq-off-by-construction.md
- docs/ADR-020-owner-metadata-production-diagnostic.md
- docs/design.md (v0.4 identity update; commit 4 of this dispatch)
```

---

## 7. Pre-flight checks (before commit 1)

Run these BEFORE starting commit 1. Any failure → STOP and surface to PM.

```bash
cd /e/AI\ Automation/AltimitNeo
git status                                                # expect: clean working tree on main
git rev-parse --abbrev-ref HEAD                           # expect: main
git rev-parse HEAD                                        # capture: should be 3a2cb70260cc69ef82ad1964c9d34dea300253b3
git log --oneline -3                                      # expect: 3a2cb70 (slogan), 27f2f8f (README rewrite), 3b64723 (PHASE-2.md + INDEX.md sweep)

# Verify the three proposals exist and are readable:
test -f Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md && echo "OK: ADR-018 proposal present"
test -f Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md && echo "OK: ADR-019 proposal present"
test -f Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md && echo "OK: ADR-020 proposal present"

# Verify the audit and design.md substrate:
test -f Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md && echo "OK: audit present"
test -f docs/design.md && echo "OK: design.md present"
grep -c "^# ALTIMIT NEO — Design Vision v0.3" docs/design.md  # expect 1 (pre-edit state)

# Verify ANSPEC files exist at expected names:
test -f docs/specs/altimitneo-ai-subsystem-spec.md && echo "OK: ANSPEC-001 at legacy name"
test -f docs/specs/altimitneo-ai-capability-semantics-spec.md && echo "OK: ANSPEC-002 stub at legacy name"

# Verify bible at v1.4.0:
tail -5 docs/bible.md | grep -q "Version: 1.4.0" && echo "OK: bible v1.4.0 (pre-amend state)"

# Verify INDEX.md exists:
test -f Plans/INDEX.md && echo "OK: INDEX.md present"

# Verify PHASE-2.md exists:
test -f docs/phases/PHASE-2.md && echo "OK: PHASE-2.md present"
```

If any check fails → STOP. Do not attempt repair. File `Plans/PROMETHEUS-FAILURE-2026-05-10-adr-ratification-preflight.md` with the failing check's output and notify PM.

---

## 8. Post-commit verification (after commit 6, before push)

After commit 6 lands:

```bash
# Verify all six commits in order:
git log --oneline -8                                      # expect: 6 new commits + previous chain

# Verify the new ratified ADR files exist:
test -f docs/ADR-018-bible-8-5-4-preempt-while-holding-clarification.md && echo "OK: ADR-018 file"
test -f docs/ADR-019-switchmutex-irq-off-by-construction.md && echo "OK: ADR-019 file"
test -f docs/ADR-020-owner-metadata-production-diagnostic.md && echo "OK: ADR-020 file"

# Verify bible v1.5.0:
tail -10 docs/bible.md | grep -q "Version: 1.5.0" && echo "OK: bible bumped to v1.5.0"

# Verify Article 8.5.4 amendment:
grep -A 2 "8.5.4 No blocking" docs/bible.md | head -5     # expect: new wording starting "No blocking, yielding, or being preempted..."
grep -c "Hpw failure mode" docs/bible.md                  # expect: ≥1 (new amendment mentions Hpw)

# Verify design.md v0.4:
grep -c "^# NeoPlutarch — Design Vision v0.4" docs/design.md  # expect: 1
grep -c "Intelligence as a kernel primitive" docs/design.md   # expect: ≥1 (tagline in §1)
grep -c "ALTIMIT NEO" docs/design.md                          # expect: 0 (positive descriptor; referential mentions don't trigger this)
grep -ci "altimit" docs/design.md                             # expect: ~3 (the three referential mentions enumerated in §4.2(l))

# Verify ANSPEC file renames:
test -f docs/specs/neoplutarch-ai-subsystem-spec.md && echo "OK: ANSPEC-001 renamed"
test -f docs/specs/neoplutarch-ai-capability-semantics-spec.md && echo "OK: ANSPEC-002 renamed"
test ! -f docs/specs/altimitneo-ai-subsystem-spec.md && echo "OK: ANSPEC-001 old name absent"
test ! -f docs/specs/altimitneo-ai-capability-semantics-spec.md && echo "OK: ANSPEC-002 old name absent"
git grep -c -F 'altimitneo-ai' -- docs/                   # expect: 0
git grep -c -F 'altimitneo-ai' -- Plans/                  # expect: ≥1 (Plans/ historical record preserved)

# Verify INDEX.md sweep:
grep -c "DISPATCH-2026-05-10-readme-slogan-addendum-claude-3" Plans/INDEX.md   # expect: ≥1
grep -c "PM-NOTE-2026-05-10-agent-roster-update" Plans/INDEX.md                # expect: ≥1
grep -c "KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10" Plans/INDEX.md               # expect: ≥1

# Verify PHASE-2.md updated:
grep -c "ADR-018" docs/phases/PHASE-2.md                  # expect: ≥1
grep -c "ADR-019" docs/phases/PHASE-2.md                  # expect: ≥1
grep -c "ADR-020" docs/phases/PHASE-2.md                  # expect: ≥1
```

Any unexpected result → STOP. Do not push. File `Plans/PROMETHEUS-FAILURE-2026-05-10-adr-ratification-postcommit.md` and surface to PM.

---

## 9. Push

Per `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` push-event policy: this dispatch closes the ADR ratification cycle for the SC-3B carry-forwards and completes the docs-side surface of the 2026-05-10 rename. Both qualify as project-governance landing class, triggering a push.

After all six commits land and post-commit verification passes:

```bash
git push origin main
git rev-parse HEAD
git rev-parse origin/main                                 # expect: equal to HEAD
```

If push fails or refs diverge → STOP, file failure artifact, surface to PM.

---

## 10. Failure handling

If any step fails or surfaces unexpected state at any commit:

1. Do NOT amend, stash, `git reset`, or otherwise attempt recovery.
2. Leave the working tree as-is on the session branch.
3. File `Plans/PROMETHEUS-FAILURE-2026-05-10-adr-ratification.md` with: which commit was in progress, what was attempted, what the actual state showed, what the expected state was, verbatim output of the failing command.
4. Notify PM.
5. PM triages and dispatches either a corrective sub-step or accepts partial progress depending on the failure.

Stop conditions explicit:

- Pre-flight check failure (§7) → STOP before commit 1.
- Bible amendment producing a malformed Article 8.5.4 (missing replace, double-pasted, etc.) → STOP after commit 1; do not proceed to commit 2.
- ANSPEC `git mv` producing unexpected git status → STOP after commit 5 attempt; do not proceed to commit 6.
- Post-commit verification (§8) failure → STOP before push.
- Any push step failure → STOP, do not retry without PM direction.

---

## 11. Out of scope

- `kernel/`, `boot/`, `tools/`, `userland/`, root build files. Negative-list off-limits per §0 coordination constraint. The forthcoming Jarvis kernel-rename bundle handles these.
- Any source-code changes (kernel or userland). This is a docs-tier dispatch.
- Any `Plans/` historical-record edits. Plans/ entries are dated and preserved per Bible 18.1.
- ADR-019 implementation (the actual `IrqSafeMutex` type code). Implementation is the Jarvis bundle's territory; this dispatch only lands the ratified ADR file.
- ADR-020 implementation (the actual `LockOwner` field code). Same — Jarvis's territory.
- CLAUDE.md (out of scope per Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md §5; PL controls RAG separately).

---

## 12. Cross-references

**Source proposals (consumed, not modified):**
- `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md`
- `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md`
- `Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md`

**Substrate referents:**
- `Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md` — informs commit 5's ANSPEC rename and the Jarvis bundle that consumes ratified ADRs
- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — canonical SC-3B closure record; cited inline in bible 8.5.4 amendment
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` — Hpw smoking-gun evidence
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — the per-call-site fix verification
- `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` — push policy
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` — current operational posture (Prometheus = Qwen3.6-27B; PM voice anchoring required per §5.2)

**Forward dispatches (downstream):**
- Forthcoming Jarvis kernel-rename bundle dispatch — consumes the ratified ADRs (018/019/020 in docs/) and the kernel-bible-alignment audit (Plans/) to implement: kernel rename, ADR-019 sub-phase A (introduce `IrqSafeMutex`), ADR-020 LockOwner field. One bundle, three sub-commits with gates between.

**Authority:**
- PL ratification 2026-05-10 (evening session): all three ADRs ratified-as-drafted; 018+019 linked; 020 independent; Jarvis bundle approved.
- Bible Article 27 (amendment process) — followed.

**Operational posture references:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.2 — Document-only work → Prometheus; PM voice anchoring required for the new Prometheus backing.
- This dispatch's voice anchor: match the structural shape of `Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md` (six-commit dispatch precedent).

---

**End of dispatch.**

*Six commits, single-recipient Prometheus, docs-tier only. ADR-018 + Bible v1.5.0 amendment → ADR-019 (linked) → ADR-020 (independent) → docs/design.md v0.4 → ANSPEC filename rename + xrefs → INDEX.md + PHASE-2.md sweep. Post-commit verification gates the push. All "Open questions for ratification" answered per PM leans (documented in each ratified ADR's "Ratification decisions" section). Hand off to Prometheus when ready.*
