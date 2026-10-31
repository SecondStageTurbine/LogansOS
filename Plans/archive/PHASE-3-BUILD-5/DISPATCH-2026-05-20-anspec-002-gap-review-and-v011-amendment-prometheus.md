# Dispatch - ANSPEC-002 Gap Review And v0.1.1 Amendment Draft

Date: 2026-05-20
Agent: Prometheus
Status: AUTHORIZED DRAFT (REPLACES RESCINDED 2026-05-20 SKELETON DISPATCH)

Authority:

- `docs/bible.md` v1.6.0 (current ratified Bible; especially Articles 6, 6.6,
  12, 18, 20–23, 27)
- `docs/specs/neoplutarch-ai-subsystem-spec.md` (ANSPEC-001, current)
- `docs/specs/neoplutarch-ai-capability-semantics-spec.md` (ANSPEC-002 v0.1.0,
  existing — this is the artifact being reviewed and amended)
- `docs/research/ALTIMIT-real-world-implementation.md` (research report; the
  artifact whose recommendations are being checked against ANSPEC-002 coverage)
- `docs/ADR-010-capability-system-foundation.md` (two-tier rights model
  ANSPEC-002 composes with)
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q8 (cap-table SMP composition)
- `docs/ADR-021-phase-3-design-substrate.md` Q9 (reserved NeoFS inode type IDs
  for Model, Tensor, InferenceContext)
- `Plans/PM-NOTE-2026-05-20-prometheus-dispatch-rescission-stale-context-error.md`
  (context for why the earlier dispatch was rescinded)
- `Plans/DISPATCH-2026-05-20-phase-3-build-4B-mount-table-and-mount-syscalls-cerebro.md`
  (BUILD-4B in flight; ANSPEC-002 amendments must not pre-commit to BUILD-4B-
  dependent rules)

Substrate:

```text
0682d24 test/wake-token: wrap remaining WAKE_TOKEN_STATE writes in without_interrupts
```

## 1. Mission

ANSPEC-002 v0.1.0 already exists. Prometheus reviews it against the ALTIMIT
research report and identifies gaps where the existing spec is silent on
constraints the research report recommends or the BUILD-4B-anticipated
work suggests should be captured.

Output is a **v0.1.1 amendment delta** to the existing spec, not a new spec.
The amendment may add sections, refine wording in existing sections, and add
to §12 Open Questions, but it does not rewrite or replace existing content.

This is informational doc work per Bible Article 18.1. PL signoff on any
material change to a normative spec follows Article 27's standard
ADR + bible-amendment process, applied to ANSPEC-002. PM expects the v0.1.1
output to be a proposal for PL ratification, not a fait accompli.

## 2. PM-Suspected Gaps (Starting Points, Non-Exhaustive)

PM's first-pass comparison of the existing spec against the research report
suggests three gaps. Prometheus is NOT constrained to only these three;
Prometheus does his own gap analysis and reports everything he finds.

### 2.1 Suspected Gap A: Capability derivation depth

The research report §G (composition safety) and §C (the five primitives)
imply a derivation chain of depth ≥4:

```text
Process -> ModelCap -> [TensorCaps...] -> [NeoFS InodeCaps...]
```

with `InferenceContext` introducing orthogonal fan-in:

```text
InferenceContext -> {ModelCap, TensorCaps, AccelTask, LogitPort}
```

The existing spec §8.2 (transitive-closure invariant) addresses the rights-
correctness side via ADR-010 §Q5 bit-subset. It does NOT name a derivation-
depth requirement on the capability table implementation.

If the cap-table implementation has any built-in depth limit (e.g. a
fixed-size derivation-log array, a recursion-bounded walk in `is_subset_of`,
a per-cap parent-chain truncation), Phase 5+ AI workflows will hit that
limit. The amendment should name the depth-≥4 requirement explicitly so
the cap-table design choices Cerebro makes in current and future builds
do not foreclose it.

### 2.2 Suspected Gap B: Cascade revocation on underlying-storage destruction

ADR-021 Q9 reserves NeoFS inode type IDs for `Model`, `Tensor`, and
`InferenceContext` and ratifies the FileCap stale-generation discipline.
BUILD-4B is adding directory-capability mount/umount with live-handle
accounting.

When AI objects are backed by NeoFS storage (the Phase 5+ direction), the
question arises: if a Model's backing NeoFS inode is destroyed, OR the
inode generation advances such that an outstanding FileCap is stale, what
happens to derived `ModelCap`s and to `TensorCap`s for the model's weight
tensors?

The existing spec §9 (provenance) names the derivation-log chain that
makes the answer derivable (walk parent links until the storage backing is
reached). It does not state the cascade-revocation discipline as a
normative constraint.

PM suspects the right constraint is:

> Capability revocation on underlying-object destruction MUST cascade.
> If a Tensor's backing NeoFS inode is destroyed, or the inode
> generation advances such that the underlying FileCap is stale, every
> derived TensorCap referencing it MUST return ESTALE on next use. Same
> discipline as BUILD-2 FileCap generation check, generalized to AI
> objects.

But this is BUILD-4B-adjacent. The exact shape of cascade revocation may
depend on whether BUILD-4B's mount-table semantics treat outstanding
ModelCaps as live handles (forcing EBUSY on unmount) or as cascade-
revocation targets (allowing unmount by invalidating caps). That choice
is PL's call after BUILD-4B closes.

The amendment should therefore name the cascade-revocation requirement at
the AI-object level but defer the mount-interaction specifics with an
explicit `[DEFERRED PENDING BUILD-4B CLOSURE]` marker.

### 2.3 Suspected Gap C: No-ambient-AI-authority constraint, explicit

Bible Article 6.6 establishes that the kernel exposes inference-governance
hooks but does not implement governance policy. Bible Article 23 forecloses
a system AI assistant.

A complementary cap-side constraint follows but is not explicitly named in
the existing ANSPEC-002:

> There is no global "model registry capability" that confers authority to
> invoke any loaded model. Every model invocation requires a ModelCap
> derived from a process-explicit load or grant.

This is the ALTIMIT-canon parallel from the research report: ALTIMIT is
substrate, not a holder of model authority. The constraint forecloses an
implementation shortcut (a kernel-global cap that all processes implicitly
hold) that would technically work but would violate Article 23's spirit by
introducing ambient AI authority.

The existing spec §4 (Model) implies this via the per-process derivation
log discipline but does not state it as a normative constraint.

## 3. Scope

In scope:

- Full read of existing ANSPEC-002 v0.1.0 against the research report
  recommendations (research report §§A–L, plus the Stage-0/1/2/3 roadmap).
- Identification of gaps where ANSPEC-002 v0.1.0 is silent on a research-
  report recommendation that the spec is the right home for.
- Drafting of section text or constraint-clause text for each gap found.
- Drafting of Open Question entries (§12) for items that are gaps but
  cannot be closed without PL decision or BUILD-4B closure.
- Producing a v0.1.1 changelog showing what changed and why.

Out of scope:

- Rewriting any existing section. v0.1.1 is additive and refining, not
  restructuring.
- Adding new AI object types beyond the five enumerated (Tensor, Model,
  InferenceContext, AccelTask, LogitPort). Research-report Stage-3
  primitives (e.g. ConfidentialInference) remain deferred per existing
  §11.3 phasing.
- Drafting Bible amendments. If a gap suggests a Bible amendment is
  needed, surface it in the close artifact and do NOT draft amendment
  text. PL owns Article 27.
- Pre-committing to BUILD-4B-dependent rules (mount-interaction
  specifics, FileCap-derivation parent/sibling/separate-table choice).
  Use `[DEFERRED PENDING BUILD-4B CLOSURE]` markers where needed.
- Reconciling against the rescinded skeleton dispatch's D1–D6
  constraints verbatim. Those were PM's first-pass formulation; the
  authoritative formulation is whatever Prometheus derives from reading
  the existing spec and the research report directly.

## 4. Required Output

### 4.1 Spec amendment

Either:

- A new file `docs/specs/neoplutarch-ai-capability-semantics-spec-v0.1.1-DRAFT.md`
  containing the proposed amended spec, OR
- A diff-style amendment proposal `docs/specs/anspec-002-v011-amendment-DRAFT.md`
  showing section-by-section additions and refinements.

Prometheus chooses the format that best preserves reviewability. PM has no
preference between the two as long as the changes are clearly attributable.

### 4.2 Close artifact

```text
Plans/PROMETHEUS-CLOSE-2026-05-20-anspec-002-gap-review.md
```

Required content:

- substrate commit hash;
- list of gaps found, with research-report section reference for each;
- for each gap: whether it was closed by the amendment, deferred with a
  marker, or surfaced for PL/Bible-amendment consideration;
- explicit confirmation that no existing section was rewritten in place
  (additive/refining only);
- explicit list of any places where the existing v0.1.0 wording was
  found incorrect or inconsistent with the current Bible v1.6.0 (e.g.
  a citation to "Bible Article X.Y" that no longer exists at that
  address after amendments); flag these but do not fix them in this
  pass — Bible-citation correction is its own focused task;
- explicit confirmation: "this artifact is a v0.1.1 amendment proposal,
  informational per Bible Article 18.1, pending PL ratification per
  Article 27."

## 5. Anti-Scope

NOT in this draft:

- Bible amendment text. If a finding suggests one is needed, surface it in
  the close artifact's "for PL consideration" list.
- Implementation guidance. ANSPEC-002 is design contract, not implementation
  manual.
- Syscall ABI changes. ANSPEC-001 §6 owns AI syscalls; cross-reference, do
  not modify.
- New AI primitives beyond the five.
- Resolution of Open Questions Q1–Q5 already in the existing §12. Those are
  intentional deferrals; the amendment may add to §12 but should not close
  existing entries without PL signoff.
- Reconciliation work on Bible-citation addresses (separate focused task per
  §4.2 close artifact requirement).

## 6. Stop Conditions

Stop and surface to PM if:

- The gap review reveals that an existing v0.1.0 normative claim is
  inconsistent with the current Bible v1.6.0 in a way that requires
  retraction, not refinement. That's a v0.2.0 task, not v0.1.1.
- BUILD-4B closes during drafting and changes the mount-capability shape in
  a way that affects the cascade-revocation deferred sections. Surface and
  wait for PM re-dispatch.
- A research-report recommendation that PM expected to be in scope turns out
  to require ANSPEC-001 changes rather than ANSPEC-002 changes (e.g. an op-
  set extension). Surface; do not modify ANSPEC-001 from this dispatch.
- The gap review identifies more than ~5 distinct gaps requiring section
  additions. That suggests v0.1.1 is too small an amendment vehicle and PM
  should consider chunking the work.

## 7. PM Posture

This is a focused doc lane. Expected turnaround: complete read-and-draft
cycle within reasonable Prometheus working time; do not expand scope to
cover everything in the research report at once.

When the draft is ready, PM consumes it, surfaces to PL for ratification
review per Article 27, and either ratifies, requests revisions, or moves
specific findings to ADR drafts or Bible-amendment candidates.

Cerebro's BUILD-4B work and Jarvis's force-kill audit proceed independently
in parallel. No coordination required.
