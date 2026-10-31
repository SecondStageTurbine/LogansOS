# Dispatch - Phil-opp's "Writing an OS in Rust" OBSERVE Pass

Date: 2026-05-20
Agent: Cerebro
Status: AUTHORIZED RESEARCH (OBSERVE-CLASS; INFORMATIONAL ONLY PER BIBLE ARTICLE 18.1)

Authority:

- PL signal 2026-05-20: "lets send cerebro on a observe pass through it" — explicit
  authorization for a retrospective cross-reference of phil-opp's "Writing an OS
  in Rust" blog against NeoPlutarch's existing Phase 0–3 substrate.
- `docs/design.md` §9 References — phil-opp's blog is the project's acknowledged
  pedagogical reference for Rust kernel work, listed alongside Limine and the
  .hack universe.
- `docs/bible.md` v1.6.0 — substrate to cross-reference against.
- Bible Article 14 (OBSERVE → THINK → PLAN → BUILD/VERIFY algorithm).
- Bible Article 18.1 (informational artifacts).
- `Plans/DISPATCH-2026-05-20-phase-4-network-stack-observe-cerebro.md` — shape
  precedent (also OBSERVE-class research dispatch).

Substrate at time of dispatch:

```text
Phase 0 through Phase 3 substantively complete; BUILD-5b bare-metal validation
pending. Bible v1.6.0 ratified. ADRs 001–022 ratified. ANSPEC-001 + ANSPEC-002
v0.1.1 ratified. Three filesystems × three block-device backends substrate-
agnostic.
```

## 1. Mission

Cross-reference phil-opp's "Writing an OS in Rust" blog series (canonical URL:
https://os.phil-opp.com) against NeoPlutarch's existing substrate. Produce a
retrospective OBSERVE artifact that:

- maps phil-opp's coverage areas to NeoPlutarch's corresponding substrate;
- identifies convergent design choices (where both projects independently
  arrived at the same approach);
- identifies intentional divergences (where NeoPlutarch's Bible/ADRs explicitly
  reject phil-opp's recommendations);
- identifies genuine gaps if any (patterns phil-opp covers that NeoPlutarch's
  substrate does not address but plausibly should);
- identifies opportunistic patterns (techniques phil-opp uses that NeoPlutarch
  might benefit from cross-referencing without commitment to adopt).

The mission is to confirm that NeoPlutarch's substrate has not silently
overlooked something well-known in the Rust kernel pedagogy, and to formally
record that phil-opp has been considered as a reference. The mission is NOT
to redo phil-opp's tutorial work or to recommend retroactive design changes.

This is a smaller-scope OBSERVE than the Phase 4 network stack OBSERVE
because:

a. phil-opp's series covers approximately Phase 0 through Phase 1.5
   territory — substrate NeoPlutarch closed months ago.
b. NeoPlutarch has moved substantially beyond phil-opp's scope (SMP,
   capability-based security, three storage backends, three filesystems,
   AI-subsystem spec, etc.).
c. The cross-reference work is mostly confirmation-of-divergence rather
   than substrate-design.

## 2. Scope

### 2.1 In Scope

**A. Survey phil-opp's Edition 2 series (canonical).** Read and summarize
each post's coverage:

- A Freestanding Rust Binary
- A Minimal Rust Kernel
- VGA Text Mode
- Testing
- CPU Exceptions
- Double Faults
- Hardware Interrupts
- Introduction to Paging
- Paging Implementation
- Heap Allocation
- Allocator Designs
- Async/Await

For each post: one-paragraph summary of what phil-opp teaches, what specific
crates/techniques he recommends, what assumptions the post makes.

**B. Note Edition 1 posts that aren't superseded by Edition 2.** Phil-opp's
Edition 1 contains some material (Set Up GDT, some early paging framing, etc.)
that Edition 2 doesn't directly cover. Brief acknowledgment of what's unique
to Edition 1 where relevant.

**C. Cross-reference table.** For each phil-opp topic, classify NeoPlutarch's
relationship:

- **Convergent:** both projects arrived at the same approach (e.g.,
  `linked_list_allocator` crate, `#![no_std]` Rust kernel, x86_64 crate
  usage).
- **Divergent (intentional):** NeoPlutarch's Bible or ADR explicitly chose
  a different approach (e.g., Limine instead of bootimage, framebuffer
  instead of VGA text mode, custom test harness instead of bootimage
  runner). Cite the Bible article or ADR that authorizes the divergence.
- **Substrate-extended:** NeoPlutarch covers phil-opp's topic and has gone
  substantially further (e.g., paging → KASLR + per-process PML4 + TLB
  shootdown; heap → buddy + slab + IRQ-safe variants; interrupts → SMP
  + cross-CPU IPI wake primitive).
- **Not applicable:** phil-opp's topic is outside NeoPlutarch's design
  space (e.g., bootimage tooling is not relevant since NeoPlutarch uses
  Limine).
- **Gap:** phil-opp covers a pattern NeoPlutarch's substrate does not
  address. (PM expects this category to be small or empty given how far
  past phil-opp's scope NeoPlutarch has moved.)

**D. Gap analysis.** For any topic Cerebro classifies as "Gap" in §2.1.C:

- specific description of what phil-opp covers;
- specific description of what NeoPlutarch lacks;
- assessment of whether the gap is intentional (Bible/ADR would forbid
  the pattern) or oversight (no Bible/ADR speaks to it);
- assessment of severity (would addressing the gap be load-bearing for
  any current or planned Phase work?).

Cerebro does NOT propose remediation for gaps. The artifact surfaces the
gap; PM decides whether to dispatch follow-up work.

**E. Opportunistic pattern analysis.** Patterns phil-opp uses that
NeoPlutarch might benefit from cross-referencing — without commitment to
adopt. Examples might include: specific Rust idioms for `unsafe`
boundaries, particular test-harness patterns, specific patterns for
encoding hardware state in the type system, etc.

For each pattern: one-paragraph description, NeoPlutarch's current
analog if any, and a neutral note on whether the pattern is worth a
future PM examination. **No recommendation to adopt.**

**F. Explicit confirmation of intentional divergences.** The artifact
should explicitly list every place NeoPlutarch diverges from phil-opp's
recommendations, with citation to the authorizing Bible article or ADR.
This produces a durable record that the divergences are intentional, not
oversights.

Known divergences PM expects Cerebro to find and document (non-exhaustive):

- **Bootloader:** phil-opp recommends `bootimage` / `bootloader` crate;
  NeoPlutarch uses Limine (Bible Article 4.4).
- **Test framework:** phil-opp uses `bootimage runner` with custom test
  framework; NeoPlutarch uses `cargo neo-test` orchestrated through
  docker compose (project tooling).
- **VGA text mode:** phil-opp covers; NeoPlutarch uses Limine-supplied
  framebuffer (Bible Article 9.8).
- **Async/Await:** phil-opp's Edition 2 final post covers kernel-level
  async/await with cooperative scheduling; NeoPlutarch deliberately does
  not have an async runtime in the kernel (ADR-021 Q1: synchronous-blocking
  block API; single-thread-per-process invariant).

These four are illustrative; Cerebro identifies all material divergences.

### 2.2 Anti-Scope

NOT in this OBSERVE:

- Design decisions or recommendations to amend any Bible article, ADR, or
  ANSPEC.
- Implementation proposals.
- Code recommendations.
- Phase-roadmap changes.
- Reproducing phil-opp's tutorial content. The artifact surveys
  phil-opp's work; it does not re-explain his material at depth.
- Detailed code-level comparison of phil-opp's example code vs
  NeoPlutarch's actual kernel code.
- Edition-1-vs-Edition-2 deep comparison beyond noting what's unique to
  each.
- Survey of phil-opp's other writing (his crate docs, his GitHub repos,
  his RustConf talks). Scope is limited to the os.phil-opp.com blog
  series.
- Survey of related Rust-OS pedagogy beyond phil-opp (e.g., other
  Rust-OS tutorials, the Rust Embedded Book's OS section). Out of
  scope for this dispatch.
- Performance comparison.
- Security comparison beyond what naturally surfaces in the
  cross-reference.

## 3. Stop Conditions

Stop and surface to PM before continuing if:

- Research reveals a hard conflict between phil-opp's pedagogy and
  NeoPlutarch's existing Bible/ADR posture in a way that the Bible/ADR
  should be revisited. (PM expects this to be very rare or absent.
  NeoPlutarch's substrate is intentionally clean-slate, not Linux- or
  phil-opp-derived.)
- Research surfaces a genuine substrate gap that's load-bearing for
  current or near-term Phase work (i.e., something Phase 5+ or BUILD-5b
  would need that NeoPlutarch's substrate lacks). Surface immediately;
  do not silently fold into the artifact.
- A pattern phil-opp uses contradicts a Bible safety invariant
  (Article 8.5 critical-section discipline, Article 15.2 `unsafe`
  discipline, Article 12 capability discipline). Surface so the
  divergence can be explicitly noted in the artifact and any spillover
  considered.
- The artifact starts feeling like THINK-class work (enumerating
  decisions and choosing) rather than OBSERVE-class (mapping
  terrain).
- The artifact exceeds ~2000 lines. OBSERVE artifacts should be
  comprehensive but not exhaustive; phil-opp's series is finite and
  well-scoped, so the survey should stay tight.
- phil-opp's blog content has changed materially since PM's understanding
  was formed (e.g., new posts added, existing posts substantially
  revised). Surface so PM and PL know the survey reflects current state.

## 4. Required Close Artifact

Create:

```text
Plans/OBSERVE-2026-05-20-phil-opp-writing-an-os-in-rust-survey-cerebro.md
```

(Or later date if research spans multiple days. Cerebro can complete this
faster than the Phase 4 OBSERVE; phil-opp's series is finite and the
cross-reference work is largely confirmation-of-divergence.)

Required content sections:

1. **Substrate state at start of research.** Bible version, recent build
   closures, lane status.
2. **TL;DR.** 2-4 bullet summary of findings: how many topics phil-opp
   covers, how many are convergent / divergent-intentional /
   substrate-extended / not-applicable / gap, headline finding if any.
3. **§2.1.A Edition 2 post survey.** One-paragraph-per-post coverage
   summary.
4. **§2.1.B Edition 1 unique-material note.** Brief.
5. **§2.1.C Cross-reference table.** Tabular: phil-opp topic | classification
   | NeoPlutarch's substrate location | Bible/ADR citation if divergent.
6. **§2.1.D Gap analysis.** Per-gap detail if any gaps found; otherwise
   explicit "no gaps identified" statement.
7. **§2.1.E Opportunistic pattern analysis.** Patterns worth future
   examination, with neutral framing (no adopt-recommendation).
8. **§2.1.F Intentional divergence record.** Definitive list of
   NeoPlutarch's divergences from phil-opp with citation.
9. **References.** Specific URLs of phil-opp posts surveyed; NeoPlutarch
   artifacts cross-referenced.
10. **Verbatim closure (per dispatch convention).** Blockquoted, lowercase:

> this artifact is observe-class per bible article 14; it maps terrain and
> does not choose routes. it is informational per bible article 18.1; it
> does not bind the project. phil-opp's blog is acknowledged as a project
> reference per design.md §9; this artifact records the formal
> cross-reference against current neoplutarch substrate. no follow-on
> action is implied; pm decides whether any surfaced item warrants
> dispatch.

## 5. PM Posture

PM monitoring. Lanes after this dispatch:

- **Cerebro:** phil-opp OBSERVE (this dispatch).
- **Jarvis:** idle (BUILD-5a-NVMe closed; available for BUILD-5b when
  hardware arrives).
- **Prometheus:** idle.

The phil-opp OBSERVE runs in the same posture as the Phase 4 network
OBSERVE: parallel lane, zero shared substrate with any active
implementation work, no coordination required.

Expected turnaround: faster than the Phase 4 network OBSERVE (which was
634 lines). PM estimates 200–800 lines for the phil-opp survey given:

- finite source material (phil-opp's series is bounded);
- most cross-reference work is confirmation-of-divergence, not original
  design synthesis;
- NeoPlutarch's existing substrate is well-documented, so the
  cross-reference resolves cleanly.

No hard timeline. Quality over speed per Bible Article 1.2.

When Cerebro closes the OBSERVE artifact, PM consumes. If any gap is
surfaced, PM triages whether to dispatch follow-up (probably no);
if no gap is surfaced, the artifact stands as durable record that
phil-opp has been formally considered.

## 6. Boundaries (Reinforcement)

Cerebro should treat this dispatch as research authorization, not design
authorization. The temptation in cross-reference work is to slip into
recommendation language — "NeoPlutarch SHOULD adopt phil-opp's pattern
for X." Resist. Indicative mood for what exists in either project;
interrogative for any genuine open question; never imperative.

The artifact's value is comprehensive cross-reference with neutral framing,
not advocacy. If phil-opp's approach to topic X is interesting,
Cerebro records that it's interesting and notes NeoPlutarch's current
approach without recommending change. PM decides whether interest
warrants follow-up.

phil-opp's blog is a teaching resource for people who don't yet know how
to write a Rust OS. NeoPlutarch has been writing one for months. The
survey's purpose is **completeness of consideration**, not catching up
on basics. Treat the source material with the respect due to a
well-regarded reference, and treat NeoPlutarch's substrate with the
respect due to substantial completed engineering work. Neither dominates
the other in this artifact.
