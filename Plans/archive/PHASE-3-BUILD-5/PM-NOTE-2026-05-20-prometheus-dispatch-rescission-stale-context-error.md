# PM-NOTE - Prometheus Dispatch Rescission (Stale Context Error)

Date: 2026-05-20
From: PM
Status: RESCINDS `Plans/DISPATCH-2026-05-20-anspec-002-capability-semantics-ai-objects-skeleton-prometheus.md`

## 1. What Happened

PM issued a dispatch on 2026-05-20 instructing Prometheus to draft ANSPEC-002
as a skeleton. The dispatch was based on stale PM-seat handoff materials:

- The Bible version PM was operating from was **v1.0.0** (2026-05-02).
- The current ratified Bible is **v1.6.0** (last amended 2026-05-11).
- The handoff materials also predated the 2026-05-10 project rename from
  "AltimitNeo" to "NeoPlutarch" (Bible Article 25.1; trademark-driven), so
  spec file paths cited in the original dispatch used pre-rename names.

Prometheus correctly failed fast on the first cited file path
(`docs/specs/altimitneo-ai-subsystem-spec.md`) which no longer exists; the
file is now at `docs/specs/neoplutarch-ai-subsystem-spec.md`.

On checking `docs/specs/` PM discovered that ANSPEC-002 already exists:

```text
docs/specs/neoplutarch-ai-capability-semantics-spec.md
ANSPEC-002, v0.1.0, dated 2026-05-05
Status: Draft (forward-looking; Phase 5+ implementation)
Declared normative.
```

The existing spec covers all five primitives the original dispatch asked
Prometheus to draft (`Tensor`, `Model`, `InferenceContext`, `AccelTask`,
`LogitPort`). It composes with ADR-010's two-tier rights model (meta byte +
semantic byte) and with ADR-013 §Q8 (SMP cap-table composition). It is more
rigorous than the skeleton the original dispatch described.

## 2. Effect On Other Lanes

Cerebro's BUILD-4B implementation lane is unaffected. The mount-table and
mount-syscall work proceeds.

Jarvis's force-kill / FD-teardown interleaving audit dispatch
(`Plans/DISPATCH-2026-05-20-phase-3-build-4B-fd-teardown-interleaving-audit-jarvis.md`)
is unaffected. That audit is load-bearing for BUILD-4B regardless of
ANSPEC-002 state.

No code or spec content was committed by Prometheus before the rescission;
the only artifact produced was the dispatch file itself.

## 3. PM Error Classification

This is a PM process error, not an agent error. Prometheus's failure-to-find
on the cited path was correct behavior. PM should have read
`docs/specs/` before issuing the dispatch, and PM should have read the
current Bible (`docs/bible.md`) rather than relying on the version supplied
in the seat-handoff materials.

Process correction filed for future PM seat changes:

> On PM seat change, the incoming PM's FIRST action is to read the live
> repo's `docs/bible.md` for the current Bible version, and to list
> `docs/specs/` and `docs/` for current spec/ADR inventory, BEFORE acting on
> any handoff-supplied artifact. Handoff materials are starting context,
> not ground truth; the live repo is ground truth.

## 4. Forward Action

The original Prometheus dispatch is RESCINDED. The file remains in place as
historical record (do not delete; the rescission is the record).

Replacement dispatch issued:

```text
Plans/DISPATCH-2026-05-20-anspec-002-gap-review-and-v011-amendment-prometheus.md
```

Scope of the replacement:

- Prometheus reads existing ANSPEC-002 v0.1.0 against the ALTIMIT research
  report and identifies gaps.
- Output is a v0.1.1 amendment delta, not a new spec.
- Three suspected gaps named by PM as starting points (capability derivation
  depth, cascade revocation on underlying-storage destruction, no-ambient-AI-
  authority as an explicit constraint); Prometheus does his own gap analysis
  and is not constrained to only those three.

## 5. Bible Amendment Triage Status Correction

PM previously told PL that three Bible amendment candidates would be queued
for triage after BUILD-4B closure:

1. Article 23.4 — verifiable kernel identity, no narrative identity.
2. Theseus-style cell-swap fault recovery.
3. Single-node scope.

Of those three:

- (1) is partially superseded by **existing Article 6.6** (kernel exposes
  inference-governance hooks but does not implement governance policy). The
  positive framing in 6.6 is stronger than the negative framing PM was going
  to propose. A standalone Article 23.4 may still be defensible to cover
  the substrate-identity-vs-narrative-identity distinction the research
  report names, but it is no longer urgent.
- (2) is still a candidate, unchanged.
- (3) is **already in the Bible as Article 4.5** ("NeoPlutarch is
  single-node by design"). Done.

Net: the post-BUILD-4B amendment-triage packet shrinks to one item (Theseus
fault recovery) and one optional item (23.4 substrate-identity framing).
PM will surface the revised packet when BUILD-4B closes.

## 6. PM Posture Going Forward

PM accepts accountability for the stale-context error and the cost it
imposed (one wasted Prometheus dispatch cycle, one round-trip with PL to
surface the error). The error was caught before any code or spec content
was produced, which is the correct outcome for a worker-agent failure-fast
on missing references — but it should not have required the failure-fast to
catch.

Filed.
