# PM-NOTE - ANSPEC-002 v0.1.1 Q6 Deferral To Phase 5 THINK

Date: 2026-05-20
From: PM
Status: PM RECOMMENDATION / NO BIBLE OR SPEC CHANGE REQUIRED NOW

Authority:

- `docs/specs/anspec-002-v011-amendment-DRAFT.md` §16.3, Q6
- `Plans/PM-DISPOSITION-2026-05-20-anspec-002-v011-amendment-ratification.md`
- `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4B-close-consumption.md`

## 1. Context

ANSPEC-002 v0.1.1 §16 (Cascade revocation on underlying-storage destruction)
named a normative constraint at the AI-object level:

> Capability revocation on underlying-object destruction MUST cascade. If a
> Tensor's backing NeoFS inode is destroyed, or the inode generation
> advances such that the underlying FileCap is stale, every derived
> TensorCap referencing it MUST return ESTALE on next use.

§16.3 explicitly deferred the mount-interaction specifics:

> The exact shape of cascade revocation when a mounted filesystem is
> unmounted depends on BUILD-4B's mount-table semantics:
> - Does the mount table treat outstanding ModelCaps as live handles
>   (forcing EBUSY on unmount)?
> - Or does it allow unmount by invalidating caps (cascade-revocation
>   targets)?
>
> This choice is PL's call after BUILD-4B closes.

Open Question Q6 captured the unresolved question in the spec's open-
question section.

With BUILD-4B closed (commit `3097126`), the deferral condition is
satisfied. The mount-table semantics are now visible: BUILD-4B uses live-
handle accounting on `OpenFile` construction/Drop, and `sys_umount`
returns `EBUSY` while `live_handles > 0`.

## 2. PM Recommendation

PM recommends Q6 remain deferred to Phase 5 THINK, not resolved now.

Reasoning:

**2.1 AI objects do not exist yet.**

There are no `ModelCap`s, `TensorCap`s, or `InferenceContext`s in the
current kernel. They are Phase 5+ work. ANSPEC-001 §3 (Tensor primitive),
§4 (Model object), and §5 (Inference context) are all Phase 5 scope per
the Bible's phase ledger (Article 13). Resolving Q6 now requires making
design decisions about a subsystem that has no implementation, no tests,
and no concrete usage patterns.

**2.2 The right time to resolve Q6 is in Phase 5 THINK.**

Phase 5 THINK is the natural moment to design the AI-object capability
shape, including their backing-storage relationship. Q6 will fall out of
that design as a natural sub-question alongside:

- How do `ModelCap`s relate to the NeoFS `FileCap`s that back their `.neom`
  files?
- Does loading a model copy weights to dedicated AI-tensor allocator
  memory (ANSPEC-001 §8.1), or memory-map them from NeoFS?
- If memory-mapped, what happens to the mapping when the NeoFS mount goes
  away?
- If copied, are the source `FileCap`s released after load, making the
  mount-unmount question moot for already-loaded models?

These questions are tightly coupled. Resolving Q6 in isolation now would
likely produce a decision that has to be revisited (or worse, retroactively
constrains) once the broader Phase 5 substrate is being designed.

**2.3 No current implementation lane is blocked by Q6.**

BUILD-4C (FAT32 read-only) does not touch AI objects. BUILD-4D, BUILD-4E,
BUILD-5 also do not. Q6 has zero implementation impact between now and
Phase 5 THINK opening.

**2.4 The current ANSPEC-002 §16 normative constraint stands.**

The AI-object-level cascade-revocation requirement (§16.1) is already
ratified. What's deferred is only the mount-interaction *specifics*. The
spec is consistent and complete for what it currently specifies; Q6 is
genuinely a future-design question, not an inconsistency to be repaired.

## 3. Forward Action

When Phase 5 THINK opens (post-Phase-3-close, post-Phase-4-close), Q6
becomes a design-stage question alongside the rest of the AI-object
capability shape. PM (or whoever holds the seat then) surfaces Q6 to
PL as part of Phase 5 THINK's design questions, with the BUILD-4B
mount-table semantics fully visible as input.

Between now and then: no action required on Q6.

## 4. Documentation

ANSPEC-002 v0.1.1 §16.3 already carries the
`[DEFERRED PENDING BUILD-4B CLOSURE]` marker. With BUILD-4B closed, the
marker text is mildly stale — the dependency is no longer pending
BUILD-4B closure but pending Phase 5 THINK opening.

PM directs that the marker be updated at the next ANSPEC-002 revision
window to:

```text
[DEFERRED TO PHASE 5 THINK — mount-table semantics are now visible per
BUILD-4B commit 3097126; resolution of Q6 belongs with the Phase 5
AI-object capability design]
```

This is documentation hygiene, not a normative change. Bundle into the
opportunistic Prometheus follow-up that also handles the Bible-citation
version-number sweep across ANSPEC-001/ANSPEC-002/ADR-*. Both are small
text-edit tasks.

## 5. Out Of Scope For This Note

This note does not:

- Resolve Q6 with a specific answer (live-handle-blocks-unmount vs.
  cascade-revocation-on-unmount).
- Authorize Phase 5 THINK to open.
- Authorize an ADR or Bible amendment.
- Modify ANSPEC-002 v0.1.1 itself; the recommended marker-text update
  is a documentation cleanup, not an amendment.

PL retains the option to override and direct resolution now. PM's
recommendation is deferral; PL's call.
