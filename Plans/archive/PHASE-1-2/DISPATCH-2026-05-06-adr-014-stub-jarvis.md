# Dispatch: ADR-014 stub — kernel observability deferred to Phase 8+

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-06
**Parent commit:** `3d1b908` (currently HEAD on `main`)
**Type:** Doc-only. New file. Single commit on `main`.
**Estimated surface:** ~80–150 lines in a new `docs/ADR-014-kernel-observability.md`. No code, no Bible, no spec changes.
**Triage origin:** `ALTIMIT-real-world-implementation.md` §"Triage queue" item "ADR-014: userspace agent architecture (deferred, mark intent now)" — note: that line in the research artifact says "userspace agent architecture" but the substantive item is observability per the surrounding context and the artifact §K (`KernelAttestation`). The research artifact has two near-adjacent triage items confused; this dispatch produces the observability stub. The agent-architecture item is a separate triage entry that stays open.

---

## 0. Standing instructions

1. **Commit on `main`, not detached HEAD.** Verify with `git status` and `git branch --show-current` before committing.
2. **Run `git pull --ff-only origin main`** before starting in case Cerebro's sub-commit 2 lands while you're working. If it does, rebase and proceed — this dispatch doesn't touch any of Cerebro's territory.
3. **Doc-only.** No code. No Bible amendment. No ADR amendments to existing ADRs. New ADR file only.
4. **Match the existing ADR voice.** ADR-013 is the most recent and substantial; ADR-009 is the closest stylistic match for a "deferral / govern-the-slot" ADR. Read both as references.
5. **OBSERVE before writing.** §3 of this dispatch is the mandatory pre-implementation reading list.

---

## 1. What ADR-014 is

ADR-014 is a deferral ADR. It does NOT design kernel observability. It NAMES the slot, ENUMERATES the design questions a future ADR would need to answer, EXPLICITLY DEFERS the work to Phase 8+, and CROSS-REFERENCES the existing observability surface that already lives in the kernel and specs.

Same shape as ADR-009 §3 (the "bare-metal verification" deferral). The point is to capture the project's intent in a place future PRs measure themselves against, so a year from now nobody has to re-derive "did we deliberately defer kernel observability or did we forget about it?"

The ADR is small on purpose. ~80–150 lines. Not 500. The substance is the deferral, not a design.

---

## 2. What ADR-014 is NOT

- **Not a design.** It does not propose a `/proc`-equivalent. It does not specify event-log formats. It does not enumerate proc-stats fields. Those are Phase 8+ concerns.
- **Not a competitor to ANSPEC-001 §11.** ANSPEC-001 §11 is **AI-subsystem-specific** observability (model_load events, inf_run events, the `[ ai ]` log prefix, per-process AI accounting). ADR-014 is general-kernel observability — memory pressure, scheduler stats, capability-table introspection, process registry queries, IPC channel counts, the things a future system monitor needs that aren't AI-specific. The two are siblings; ADR-014's existence does NOT imply rework to ANSPEC-001 §11.
- **Not a chartering document for Phase 8.** Phase 8 will have its own plan; ADR-014 is just one input to it.
- **Not a `KernelAttestation` design.** Attestation is named in the research artifact §K as a Stage-2 (~18-36mo) item and shares a "trustworthy introspection" shape with observability, but it's a separable concern. ADR-014 names attestation as a related thread under §"Cross-references" but does not design it.
- **Not a `LogitPort` ADR.** That's Article 6.6 + ANSPEC-002 §7, already in tree.

---

## 3. Pre-implementation OBSERVE pass

Before writing, read in full:

1. **`docs/ADR-013-phase-2-smp-retrofit.md`** — for the substantial-ADR shape: front-matter table, Context section, Q1/Q2/Q3 forward-looking format, Anti-scope, Cross-references, Notes. ADR-014 is much smaller but should match the structural conventions (front-matter table, `## Context`, etc.).

2. **`docs/ADR-009-phase-1.5c-governance-and-bare-metal-deferral.md`** — for the deferral-shaped voice. ADR-009 names something the project deliberately does NOT do, explains why, and gives forward conditions for revisiting. ADR-014 is the same shape applied to kernel observability.

3. **`docs/specs/altimitneo-ai-subsystem-spec.md` §11** (lines ~718–842) — to know exactly what AI-specific observability already exists. The §11.2 event taxonomy is spec-stable; ADR-014 must explicitly state it does NOT touch this surface.

4. **`docs/specs/altimitneo-ai-subsystem-spec.md` §11.3** (lines ~840–845) — names "tracing hooks (likely Phase 7)" and rejects perf/eBPF. ADR-014 inherits the perf/eBPF rejection — name it explicitly.

5. **`docs/bible.md` Article 23.2** — "We will not collect telemetry, training data, or usage data from the OS." This is the Bible-level constraint ADR-014 must compose with: observability is for the LOCAL operator, never for an upstream party.

6. **`docs/bible.md` Article 12.4** — "The kernel never executes user-provided code in kernel mode... There is no eBPF equivalent in Phase 1–3. If we ever add programmable kernel hooks, they are bytecode-verified offline and signed." This is also a load-bearing constraint: ADR-014's design space cannot include programmable in-kernel observability without first amending Article 12.4.

7. **`docs/research/ALTIMIT-real-world-implementation.md` §K** ("Identity, persistent state, and 'the self of the OS'") — the `KernelAttestation` thread. ADR-014 cross-references this as a sibling/future concern but does not design it.

8. **`docs/HANDOFF.md`** if it exists, for any other observability-shaped threads I might have missed. Surface anything you find in your close report under "Observations."

If anything contradicts this dispatch, **flag the contradiction in your close report** and surface to PM rather than silently rewriting.

---

## 4. The structure ADR-014 should have

Headings and approximate content:

### 4.1 Front-matter table

Match ADR-013's table format. Specifically:

```markdown
# ADR-014: Kernel Observability Deferred to Phase 8+

| | |
|---|---|
| **Status** | Accepted (deferral; Phase 8+ implementation) |
| **Date** | 2026-05-06 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Articles 12.4 (no in-kernel programmable hooks), 23.2 (no telemetry) · ANSPEC-001 §11 (AI-subsystem observability — separate scope) · ADR-013 (per-CPU schedulers — produces stats this ADR does not yet specify how to expose) · `docs/research/ALTIMIT-real-world-implementation.md` §K (KernelAttestation, sibling concern) |
```

### 4.2 `## Context`

A few paragraphs. Topics to cover:

- The kernel's existing observability surface today is ad-hoc: serial-print log lines (`[ kernel ] sched: ready ...`), boot-time diagnostic prints, `frame: leak audit` lines, the AI event taxonomy in ANSPEC-001 §11. There is no programmatic introspection API.
- Phase 1.5+ has accumulated real per-process / per-subsystem state (process registry, capability table, slab cache stats, scheduler queues). None of it is queryable from userspace.
- A future system monitor — analogous to Linux `top`/`htop` but capability-gated and AltimitNeo-shaped — needs a programmatic surface. That surface is "kernel observability."
- This ADR does not design that surface. It names the slot, enumerates the design questions, defers the work to Phase 8+.

Frame the deferral honestly. The ALTIMIT research artifact's triage queue named "general kernel observability (deferred to Phase 8+, mark intent now)" — that's the source of the deferral. ADR-014 is the "mark intent now" act.

### 4.3 `## Why deferred, not done now`

Brief. Two-three points:

- **Phase 2 has higher-priority work.** SMP, capabilities, allocators (ADRs 010, 012, 013) all sit between us and Phase 3+. Observability that doesn't exist yet doesn't bottleneck anything.
- **Premature design risk.** The right observability shape depends on what's actually being introspected. By Phase 8 we'll have stable AI subsystem internals (Phase 5–7), real accelerator drivers, and per-CPU scheduler internals that have proven their shape under load. Designing observability before its consumers exist invites rework.
- **Engineering parsimony.** Observability is a vector for security regressions (info-leak side channels, capability-table introspection that bypasses cap discipline). It is also a vector for ABI lock-in: once a `/proc`-equivalent ships, every script in userland depends on its exact shape. Better to ship deliberately, late, than to ship by accretion.

### 4.4 `## Design questions a future ADR will answer`

The substance. List the questions Phase 8+ work has to answer, *without* answering them. Suggested set (refine during your OBSERVE pass; the list is what matters, the wording is yours):

- **Q1. What is the introspection mechanism?** Syscall (`sys_proc_stats(pid, kind)`-shaped)? Pseudo-filesystem (a `procfs`-equivalent built on top of NeoFS once it exists, Phase 3+)? An opaque IPC service hosted by a privileged userspace daemon? Each has tradeoffs. The Bible's no-eBPF stance (Article 12.4) rules out programmable in-kernel queries; everything else is open.

- **Q2. What is the capability discipline?** Bible Article 12.1: every kernel object is referenced by a capability handle. Observability means querying state about objects the caller may not hold capabilities for. The simplest discipline: the caller queries only what it has caps for (per-process info for its own pid, per-channel info for its own handles). The most useful: a privileged "observer" capability grants system-wide read access. ADR-010's two-tier rights model is the substrate; ADR-014's future work picks the policy.

- **Q3. What is the field set?** What does the kernel expose, per subsystem? The list grows with each phase. Phase 8 starts from what exists then; ADR-014 just names the categories: scheduler stats, memory pressure, capability-table summary, process registry, IPC channel counts, slab cache stats, accelerator load. Each has its own per-field observability question.

- **Q4. What is the output format?** Structured (CBOR, MessagePack, native binary)? Line-oriented `key=value`-shaped (matching ANSPEC-001 §11.2)? Per-syscall opaque struct (matching ANSPEC-001 §5.5 `InfStats`)? Composable answer: probably native binary structs for syscalls, line-oriented `key=value` for log events, the same discipline ANSPEC-001 §11 already uses for the AI subsystem.

- **Q5. What is the tracing relationship?** ANSPEC-001 §11.3 names "tracing hooks (likely Phase 7)" for inference profiling and rejects perf/eBPF. ADR-014's general-kernel tracing surface composes with that ADR but is broader (it covers scheduler / IPC / cap operations). Question: does general-kernel tracing get its own future ADR, or is it folded into the per-subsystem tracing ADRs as they land? Either is defensible. Punt to Phase 8.

- **Q6. Identity / attestation.** How does the kernel prove its own identity to a remote verifier (or a local audit tool)? `KernelAttestation` per the research artifact §K. ADR-014 names this as related but does not design it; that's likely its own ADR-NNN at the time it ships.

- **Q7. Performance budget.** Observability that costs measurable runtime (TLB shootdowns to read stats, cache-line bouncing across CPUs to maintain monitor counters) corrupts the system it claims to observe. Phase 8 design must specify the per-query and per-event cost budget. ADR-014 names the constraint; the number is Phase 8's.

Format these as `### Q1`, `### Q2`, etc. — same convention ADR-013 uses for its design questions, just without the answers.

### 4.5 `## Anti-scope`

Things this ADR explicitly does NOT do, plus things future ADR-014-successor work explicitly does NOT include:

- Does not amend Article 12.4 (no programmable kernel hooks). If a future observability design needs eBPF-shaped programmability, that requires a Bible amendment first.
- Does not amend Article 23.2 (no telemetry). Observability is for the local operator. Anything that would phone home is out of scope at the constitutional level, not just the ADR level.
- Does not adopt perf, eBPF, journald, syslog, OpenTelemetry, or Linux audit. ANSPEC-001 §11.2 already documents the rejection for AI-events; ADR-014 inherits it system-wide.
- Does not duplicate ANSPEC-001 §11. The AI subsystem's event taxonomy is its own surface; ADR-014's future work is everything else.
- Does not specify a `/proc` filesystem. NeoFS doesn't exist yet (Phase 3); discussions of FS-mediated observability wait until then.

### 4.6 `## Cross-references`

A short bullet list of related ADRs / spec sections / Bible articles. Same shape as ADR-013's cross-references section. Items I'd include:

- Bible Article 12.1 (capability-based security) — the substrate for Q2's capability discipline.
- Bible Article 12.4 (no programmable kernel hooks) — load-bearing constraint.
- Bible Article 23.2 (no telemetry) — load-bearing constraint.
- ADR-010 (capability system foundation) — the two-tier rights model future observer caps would extend.
- ANSPEC-001 §11 (AI subsystem observability) — sibling spec; ADR-014 does not duplicate.
- ANSPEC-001 §6.3.4 (stack-frame discipline) — the same composability concern (kernel-resident state held across blocking points) that observability queries against.
- `docs/research/ALTIMIT-real-world-implementation.md` §K (KernelAttestation, identity) — sibling future ADR.
- ADR-013 §Q1 (per-CPU state shape) — produces the per-CPU stats ADR-014's future work would expose.

### 4.7 `## Notes`

Brief, optional. The kind of paragraph that ADR-013's Notes section has — meta-commentary on what this ADR commits to and what remains explicitly open. One short paragraph is plenty.

---

## 5. What you do NOT touch

- **No Bible changes.** Articles 12.4 and 23.2 are cited as constraints, not amended.
- **No spec changes.** ANSPEC-001 §11 stays exactly as it is. The cross-reference is descriptive.
- **No other ADR changes.** ADR-014 is new; existing ADRs are unaffected.
- **No code.** This is a Phase 8+ deferral; there is nothing to implement.
- **No `docs/HANDOFF.md` updates.** That's PM-side document hygiene, separate concern.

If your OBSERVE pass reveals stale references in OTHER files (e.g., a comment in a kernel source file that says "see ADR-014 (pending)" anywhere), do NOT fix them in this commit. Surface in your close report.

---

## 6. Verification gates

### 6.1 Doc-only confirmation

```
git diff --stat <parent>..HEAD
```

Expected: exactly one file changed (`docs/ADR-014-kernel-observability.md`), and it's a new file (only insertions, zero deletions).

### 6.2 Markdown sanity

The new file should render cleanly. Specifically:

- Front-matter table renders.
- Heading levels consistent (H1 for the ADR title, H2 for major sections, H3 for sub-questions in §4.4).
- No broken cross-reference links.

### 6.3 Cross-reference accuracy

Every reference to another ADR / spec section / Bible article in §4.6 must match real existing content. Specifically:

- Bible Article 12.1, 12.4, 23.2 — confirm their numbers in the current `bible.md` (post-6.6 amendment).
- ADR-010, ADR-013 — exist.
- ANSPEC-001 §11, §6.3.4 — both exist.
- Research artifact §K — exists at `docs/research/ALTIMIT-real-world-implementation.md`.

If any cross-reference is wrong (different section number, different ADR number), fix it before committing.

### 6.4 No premature design

Read the file you wrote and verify it does not accidentally answer Q1–Q7. The question section names design questions; the answers belong to Phase 8+. If the prose drifts into recommending an answer ("the introspection mechanism should be a syscall family..."), pull it back to the deferred-question shape ("the introspection mechanism is undecided; candidates include syscall family, FS-mediated, opaque IPC...").

This is the most common drift in deferral ADRs: the author finds themselves with opinions and bakes them in. The discipline is to NAME the question without ANSWERING it.

### 6.5 Length sanity

If your draft is over ~250 lines, you've over-engineered it. Trim. The point is to mark the slot, not to design Phase 8.

If under ~50 lines, you've under-engineered it. Likely missing one or more of: front-matter table, Anti-scope, or cross-references. Add.

### 6.6 Commit shape

Single commit on `main`. Suggested commit message:

```
docs: ADR-014 stub — kernel observability deferred to Phase 8+

ADR-014 names the kernel-observability slot, enumerates the design
questions a future ADR will need to answer (introspection mechanism,
capability discipline, field set, output format, tracing relationship,
attestation, performance budget), and explicitly defers the work to
Phase 8+. Closes a triage-queue item from the ALTIMIT research artifact.

ADR-014 does not design observability; it marks intent. ANSPEC-001 §11
remains the canonical AI-subsystem observability surface (separate
scope; not duplicated by ADR-014). Bible Articles 12.4 (no programmable
kernel hooks) and 23.2 (no telemetry) are load-bearing constraints
named in the ADR; neither is amended.

No code changes. No Bible or spec amendments.
```

---

## 7. If you spot more triage candidates

If your OBSERVE pass reveals other "marked-intent" items that should have ADR slots but don't, surface in your close report under "Observations." Do NOT create them in this commit. Each deferral ADR is its own decision.

The handoff §7 had four items still open after this one closes:

- Capability-bounded action flow ADR (research artifact Thread 2A)
- ANSPEC-001 §6.3.4 provenance investigation (likely a small spec patch, not an ADR)
- Bible Article on fault recovery (cell-swap recovery, research artifact recommendation)
- Article on scope (already landed as 4.5; this triage entry is closed)

ADR-014 closes "ADR-014: kernel observability" from that queue. Surfacing whether more triage entries are open is welcome but not required.

---

## 8. Why this matters even though it's small

Deferral ADRs prevent two failure modes:

1. **Future PM / future Claude / future Cerebro discovers the gap and re-derives whether it was deliberate.** The ADR is the answer: yes, deliberate, here's why, here's what's required to revisit.
2. **A drive-by PR adds a one-off observability hook ("just a quick stats syscall") that locks in a shape without the design conversation.** ADR-014 gives that PR a clause to argue against.

ADR-009 is the precedent: bare-metal verification was deliberately deferred at Phase 1.5c close, with conditions for revisiting recorded in the ADR. It's been cited in subsequent dispatch + close-report discussions ever since. ADR-014 is the same shape applied to a different gap.

---

**End of dispatch.**

*Standard close report shape: confirm the new file lands at the right path, verify cross-references, paste the commit hash. ~10 minutes if you're efficient.*
