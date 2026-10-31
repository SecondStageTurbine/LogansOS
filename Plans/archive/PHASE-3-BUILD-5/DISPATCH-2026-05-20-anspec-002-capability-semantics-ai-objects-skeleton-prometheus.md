# Dispatch - ANSPEC-002 Skeleton: Capability Semantics For AI Objects

Date: 2026-05-20
Agent: Prometheus
Status: AUTHORIZED DRAFT (INFORMATIONAL ONLY; NOT NORMATIVE)

Authority:

- `TheAltimitNeoBible.md` (Articles 2, 6, 12, 18.1, 19–23)
- `docs/specs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001)
- `ALTIMIT-real-world-implementation.md` (research report; triage-pending)
- `docs/ADR-021-phase-3-design-substrate.md` (Q9: reserved type IDs)
- `Plans/DISPATCH-2026-05-20-phase-3-build-4B-mount-table-and-mount-syscalls-cerebro.md`
  (BUILD-4B in flight — derivation rules deferred pending closure)

Substrate:

```text
0682d24 test/wake-token: wrap remaining WAKE_TOKEN_STATE writes in without_interrupts
```

## 1. Mission

Draft ANSPEC-002 as a skeleton: enumerate the AI-object capability surface
and lock in the load-bearing design constraints, with explicit
`[DEFERRED PENDING BUILD-4B CLOSURE]` markers on sections that depend on
the FileCap derivation shape that BUILD-4B will establish.

This is a Prometheus DOC lane. The artifact is informational per Bible
Article 18.1. Ratification (PL signoff on any normative claim) happens
after BUILD-4D lands and Phase 5 THINK opens. Not now.

The artifact's value at this stage is not to bind future decisions but to:

1. fix the AI-object capability surface so subsequent design work has
   a stable reference;
2. encode load-bearing constraints (notably derivation depth ≥4) so the
   capability-table design choices Cerebro makes in BUILD-4B and beyond
   do not foreclose Phase 5+ needs;
3. mark the BUILD-4B-dependent sections explicitly so the gap is visible.

## 2. Scope

### 2.1 In Scope

- Document ID block, status, authority, version, "informational only"
  preamble matching ANSPEC-001 §0 shape.
- Enumeration of capability-protected AI object types:

  - `Tensor`
  - `Model`
  - `InferenceContext`
  - `AccelTask`
  - `LogitPort`

- For each object: rights table, lifecycle, derivation parents,
  cross-reference to ANSPEC-001 section defining the underlying object.
- Cross-reference to ADR-021 Q9: reserved NeoFS inode type IDs for
  `Model`, `Tensor`, `InferenceContext`, and the implication that
  capability provenance can be anchored to NeoFS inode generation
  for content-addressed `Model` objects.
- Cross-reference to the research report's recommendations:

  - `LogitPort` as kernel-mediated inference governance primitive
    (research report §C, ProbeLogits precedent);
  - conjunctive-safety analysis on capability composition graphs
    (research report §G, arXiv 2603.15973);
  - `KernelAttestation` for verifiable substrate identity
    (research report §K).

- A normative design-constraint section. Constraints D1–D6 below are
  load-bearing and must appear in the draft as stated:

  **D1 — Capability derivation depth.** The capability table MUST
  support derivation chains of depth ≥4 without performance
  degradation or representational truncation. Reference chain:

  ```text
  Process -> ModelCap -> [TensorCaps...] -> [NeoFS InodeCaps...]
  ```

  `InferenceContext` is orthogonal and introduces fan-in, not
  additional depth:

  ```text
  InferenceContext -> {ModelCap, TensorCaps, AccelTask, LogitPort}
  ```

  **D2 — Attenuation as first-class.** Capability attenuation
  (Zircon-style "duplicate with fewer rights") MUST be a first-class
  operation on every AI capability. A `ModelCap` attenuated to
  "inference only, no weight read" MUST be kernel-enforced, not
  application convention.

  **D3 — Content-addressed Model immutability.** Content-addressed
  `Model` objects MUST be kernel-immutable. Mutation of weights
  through a `ModelCap` is rejected even with full rights; mutation
  requires creating a derived `Model`. (Aligns with ANSPEC-001 §3.6
  `IMMUTABLE | WEIGHT` flags and ADR-021 Q9 reserved type
  discriminants.)

  **D4 — Cascade revocation on underlying destruction.** Capability
  revocation on underlying object destruction MUST cascade. If a
  `Tensor`'s backing NeoFS inode is destroyed, or the inode
  generation advances such that the cap is stale, every derived
  `TensorCap` referencing it MUST return `ESTALE` on next use. Same
  discipline as BUILD-2 FileCap generation check, generalized to AI
  objects.

  **D5 — Composition safety.** The capability table SHOULD support
  enumeration of conjunctive capability sets reachable from a given
  initial distribution by message-passing composition (the "AIDA
  problem" per research report §G). This constraint is informational,
  not implementation-blocking; it flags that future capability table
  design choices should not foreclose composition analysis.

  **D6 — No ambient AI authority.** There is no global "model
  registry capability" that confers authority to invoke any loaded
  model. Every model invocation requires a `ModelCap` derived from a
  process-explicit load or grant. Aligns with Bible Article 23 and
  the ALTIMIT/Morganna distinction in the research report: the OS is
  substrate, not a holder of model authority.

- A "Lifecycle and Provenance" section noting that every AI capability
  carries provenance metadata sufficient to trace:

  - the loading process (PID at issue time);
  - the source object (NeoFS inode generation, `.neom` file hash);
  - the derivation chain back to the root capability.

### 2.2 In Scope But DEFERRED-MARKED

The following sections appear in the draft with explicit
`[DEFERRED PENDING BUILD-4B CLOSURE]` markers and the dependency stated
inline:

- FileCap derivation rules specific to `Tensor` and `Model` objects. The
  capability derivation tree (CDT) shape established by BUILD-4B's
  mount-table semantics will determine whether AI capabilities derive
  from FileCaps as siblings, parents, or via a separate AI-cap-table.
  Mark the section, state the dependency, do not pre-commit a shape.

- Cascade-revocation semantics when a mount containing `Model` storage
  is unmounted. The `EBUSY` rule from BUILD-4B applies, but whether
  outstanding `ModelCap`s are treated as live handles (forcing `EBUSY`)
  or as cascade-revocation targets (allowing unmount to proceed by
  invalidating caps) requires PL decision after BUILD-4B closes.

- Per-mount AI-rights mask composition. ANSPEC-001 §9.1 establishes
  accelerator capabilities; whether a mount's `noexec` policy
  (ADR-021 Q9) extends to "no model load from this mount" requires
  BUILD-4B mount-flags semantics first.

## 3. Anti-Scope

NOT in this draft:

- Any normative claim. The entire document carries an
  "informational; not binding per Bible Article 18.1" header.
- Ratification of any constraint. PL signoff is post-BUILD-4D and
  Phase 5 THINK opening.
- Syscall ABI specifications. ANSPEC-001 §6 owns the AI syscall
  surface; ANSPEC-002 references it, does not modify it.
- New AI object types beyond the five enumerated. The research
  report mentions e.g. `ConfidentialInference` (TEE+HE composition);
  that is Stage-3 frontier per the report and not in this skeleton.
- Detailed accelerator scheduling. ANSPEC-001 §7 owns that.
- Implementation guidance. ANSPEC-002 is design contract; it does
  not tell Cerebro or Jarvis how to write the code.
- Pre-commit to FileCap derivation parent/sibling/separate-table
  choice. Marked deferred per §2.2.
- Bible amendments. If a research-report recommendation seems to
  require one, note it in §7 (Open Questions) and surface to PM;
  do not draft amendment text in this artifact.

## 4. Required Sections

The draft MUST include, in order:

1. Document ID block (matching ANSPEC-001 §0 shape).
2. §0 Document Purpose — informational scope, Article 18.1 reminder.
3. §1 Rationale — capability discipline for AI objects; why this is
   needed at the kernel layer.
4. §2 The Five AI Object Types — one subsection per type with:

   - brief identity / cross-ref to ANSPEC-001 section;
   - rights table (`READ`, `WRITE`, `INVOKE`, `DERIVE`, `GRANT`,
     `INSPECT`, etc. as appropriate per object);
   - derivation parents (what caps can derive this cap);
   - lifecycle anchor (what destroys/invalidates the cap).

5. §3 Design Constraints — D1–D6 as stated in §2.1.
6. §4 Lifecycle and Provenance.
7. §5 Deferred Sections — explicit `[DEFERRED PENDING BUILD-4B CLOSURE]`
   markers with dependencies stated.
8. §6 References — Bible, ANSPEC-001, ADR-021, research report, external
   precedents (Zircon, seL4 CSpace, Capsicum, ProbeLogits).
9. §7 Open Questions — mirror ANSPEC-001 §13 style; surface anything that
   suggests a Bible amendment might be needed.
10. §8 Glossary.
11. Footer — version 0.0.1, status: Draft (Skeleton), informational.

## 5. Stop Conditions

Stop and surface to PM if:

- BUILD-4B closes during drafting and the deferred sections need to
  be filled in. That's a re-dispatch, not a continuation.
- The skeleton drafting reveals a load-bearing conflict between
  ANSPEC-001 (existing) and the AI-capability model being proposed.
  Surface the conflict; do not silently override ANSPEC-001.
- A research-report recommendation appears to require a Bible
  amendment to integrate cleanly. Note it in §7 Open Questions; do
  not write the amendment.
- The draft exceeds ~3000 lines. Skeleton, not full spec.

## 6. Required Close Artifact

Create:

```text
docs/specs/anspec-002-ai-capability-semantics-DRAFT.md
```

Plus:

```text
Plans/PROMETHEUS-CLOSE-2026-05-20-anspec-002-skeleton-draft.md
```

The close artifact records:

- substrate commit;
- outline of the draft (section list);
- cross-reference list (ANSPEC-001 sections touched, ADR-021 sections
  cited, research-report sections cited);
- any conflicts with existing artifacts surfaced (per §5);
- explicit confirmation: "this artifact is informational per Bible
  Article 18.1 and does not bind the project."

## 7. PM Posture

Prometheus drafts in parallel with Cerebro's BUILD-4B implementation
and Jarvis's force-kill audit. No coordination required between the
three lanes; the deferred-section markers absorb the BUILD-4B
dependency.

Expected turnaround: skeleton complete before or around BUILD-4B
closure. If Prometheus finishes early, do not start filling in the
deferred sections; surface the early completion and wait for BUILD-4B
closure plus PM re-dispatch.

When BUILD-4B closes, PM will issue a follow-up Prometheus dispatch
to consume the closure evidence and fill in the deferred sections.
That follow-up will likely also incorporate the three Bible amendment
candidates (23.4 verifiable-identity-no-narrative, Theseus-style fault
recovery, single-node scope) that PL will have ratified or rejected
by then.

## 8. Voice And Format Notes

Prometheus voice profile (per `Plans/PM-NOTE-2026-05-11-jarvis-voice-profile.md`
adjacent precedent): plain prose, no marketing register, no hedging
beyond what the deferred-section markers already encode. Match the
ANSPEC-001 register: declarative, normative where it is normative,
explicit where it defers.

Cross-references use the same style as ANSPEC-001 §15: short bullet
references with section anchors. Do not invent new citation styles.
