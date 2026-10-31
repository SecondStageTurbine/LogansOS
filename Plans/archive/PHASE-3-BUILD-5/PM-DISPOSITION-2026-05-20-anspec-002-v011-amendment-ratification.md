# PM Disposition - ANSPEC-002 v0.1.1 Amendment Consumption And Ratification

Date: 2026-05-20
From: PM
Status: ACCEPTED / RECOMMENDED FOR RATIFICATION / PL DELEGATION HONORED
Consumed artifacts:

```text
docs/specs/anspec-002-v011-amendment-DRAFT.md
Plans/PROMETHEUS-CLOSE-2026-05-20-anspec-002-gap-review.md
```

Authority: Bible Article 27 (amendments), Article 18.1 (informational
artifacts), PL delegation of recommendation to PM on 2026-05-20 ("all on your
recommendations").

## 1. PM Recommendation

PM recommends **OPTION (a): Ratify v0.1.1 as proposed**, including §19 vAccel.

PL has delegated the recommendation call to PM and has not requested specific
revisions. Per PL's standing delegation, this disposition stands as
ratification of v0.1.1 per Article 27. PL retains the right to override or
request revisions at any time; this disposition is reversible.

## 2. Reasoning

### 2.1 Why ratify now rather than defer

§15 (capability derivation depth ≥4) is an *active* constraint on cap-table
implementation choices Cerebro is making in BUILD-4B. Deferring ratification
until BUILD-4B closes risks Cerebro landing code that forecloses depth ≥4
derivation chains without the spec naming the requirement. The cost of
deferral is concrete; the cost of early ratification is small (Q6 deferred
properly via the mount-interaction marker in §16.3).

### 2.2 Why include §19 vAccel

Prometheus's dispatch said "no new AI object types beyond the five
enumerated." Reading the existing ANSPEC-002 §6 carefully, the spec
implicitly assumes the kernel has access discipline for accelerators (§6.4
states an AccelTask's permission "is derived (in the kernel) from the issuing
process's caps on those objects"), but the *capability representing the
accelerator resource itself* was not named. §19 vAccel fills that gap by
naming the cap whose existence the rest of §6 already presupposes.

This is the legitimate outcome of "Prometheus does his own gap analysis."
The anti-scope on new primitives was a guardrail against scope explosion,
not a hard ban; Prometheus found a real gap that requires naming a primitive,
which is the correct outcome.

The alternative (option b — ratify minus §19) leaves the spec implying
something it does not name, which is worse engineering hygiene than naming
it explicitly. §19 is well-bounded: Phase 6+ kernel-internal, userspace
surfacing deferred to Phase 7+, reserved-bit pattern per ADR-010 Principle 2.
The expansion is contained.

### 2.3 Why not revisions

Prometheus produced a high-quality amendment that meets the dispatch
contract. Asking for revisions just to ask is poor PM discipline. The two
items that *do* need PL attention later (Q6, Q7) are properly deferred via
Open Question entries, which is the right mechanism per existing §12 style.

### 2.4 What is not changing

- ADR-010, ADR-013, ADR-021: unchanged. ANSPEC-002 v0.1.1 composes with them.
- ANSPEC-001: unchanged. Cross-references only.
- Bible v1.6.0: unchanged. No amendments triggered by this ratification.
- Existing ANSPEC-002 v0.1.0 sections §0–§14: unchanged (additive only).

## 3. Ratification

Per Article 27 and PL's standing delegation:

ANSPEC-002 advances from v0.1.0 to v0.1.1.

New sections ratified:

- §15 — Capability derivation depth requirement (depth ≥4 normative).
- §16 — Cascade revocation on underlying-storage destruction (AI-object level
  normative; mount-interaction specifics deferred per §16.3 / Q6).
- §17 — No ambient AI authority (normative).
- §18 — Content-addressed Model identity (discipline normative; hash
  algorithm and chunking deferred to ADR-021 / Phase 5 implementation).
- §19 — Accelerator capability surface vAccel (Phase 6+ kernel-internal;
  reserved bits per ADR-010 Principle 2).

Additions ratified:

- §12 Open Questions: Q6 (cascade × mount interaction, deferred pending
  BUILD-4B closure), Q7 (vAccel userspace surfacing in Phase 6 vs Phase 7+,
  deferred to Phase 6 design work).
- §14 References: ALTIMIT research report, ADR-021.

## 4. Forward Items

### 4.1 Q6 — Cascade × mount interaction

Surfaces in the BUILD-4B closure adjudication pass. PM will return to it then.

### 4.2 Q7 — vAccel userspace surfacing timing

Deferrable to Phase 6 design work. No urgent action. Recorded in the open-
questions list as the canonical home for the unresolved question.

### 4.3 Bible-citation cleanup

Existing v0.1.0 §14 References cites "Bible v1.0.4" but current Bible is
v1.6.0. Article addresses are correct; only the version number is stale.
Prometheus flagged this in the close artifact and explicitly did not fix it
in this pass (out of scope per dispatch §4.2).

PM will spawn this as a small Prometheus follow-up task during a quiet
window. Not urgent. Filing as a forward marker:

```text
TODO: PM dispatch — sweep ANSPEC-001, ANSPEC-002, and ADR-* artifacts for
stale Bible version citations; correct to current Bible version. Article-
address verification is in scope; new normative claims are not.
```

### 4.4 Out-of-scope items Prometheus surfaced

Prometheus correctly identified five research-report recommendations that
belong outside ANSPEC-002:

- KernelAttestation (research-frontier; no current action).
- Theseus-style cell-swap fault recovery (still a Bible-amendment candidate
  on the post-BUILD-4B triage queue).
- Single-node scope clarification (already Bible Article 4.5; complete).
- OSTD-style unsafe-boundary discipline (kernel engineering policy; not AI
  cap concern).
- Confidential inference TEE+HE primitives (Phase 8+ frontier; already
  deferred per existing §11.3).

Of these, only Theseus fault recovery remains a live candidate for future
PL triage. PM will surface it as part of the post-BUILD-4B amendment-triage
packet.

## 5. Artifact Disposition

The amendment draft at `docs/specs/anspec-002-v011-amendment-DRAFT.md` is
the canonical record of v0.1.1 changes. PM directs that on the next quiet
window (after BUILD-4B closure), the changes be folded into the main
ANSPEC-002 file at `docs/specs/neoplutarch-ai-capability-semantics-spec.md`
with version bumped to 0.1.1 and the `-v011-amendment-DRAFT.md` file moved
to a historical archive location or deleted, per project preference.

Until that fold-in lands, both files should be considered authoritative
together: v0.1.0 base + v0.1.1 amendment.
