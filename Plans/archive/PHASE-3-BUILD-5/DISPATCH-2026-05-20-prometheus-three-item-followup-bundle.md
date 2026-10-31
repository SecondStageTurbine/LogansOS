# Dispatch - Prometheus Three-Item Post-BUILD-4B Follow-Up Bundle

Date: 2026-05-20
Agent: Prometheus
Status: AUTHORIZED DRAFT (INFORMATIONAL DOC WORK; PER-ITEM RATIFICATION VARIES)

Authority:

- `Plans/PM-NOTE-2026-05-20-bible-amendment-triage-packet-post-build-4B.md` §5
  (PL signaled all three optional follow-up artifacts authorized)
- `docs/research/ALTIMIT-real-world-implementation.md` §F (Theseus consideration),
  §K (KernelAttestation), Recommendations
- `docs/bible.md` v1.6.0 Articles 9.2, 18.1, 27
- `docs/specs/neoplutarch-ai-subsystem-spec.md` §13 (existing Open Questions
  pattern)
- `docs/specs/neoplutarch-ai-capability-semantics-spec.md` §14, §16.3
- `docs/specs/anspec-002-v011-amendment-DRAFT.md`
- `Plans/PHASE-3-BUILD-4B-CLOSE-EVIDENCE-2026-05-20.md` §1

Substrate:

```text
3097126 build-4B: add mount table and directory-capability mount syscalls
```

## 1. Mission

Three small documentation artifacts post-BUILD-4B closure:

a. **ADR-022** — Fault-recovery direction forward marker (Theseus cell-swap
   consideration recorded as forward marker; no architectural commitment).
b. **ANSPEC-001 §13 Q6** — Add KernelAttestation as an Open Question
   referencing research report §K.
c. **Doc hygiene sweep** — Targeted fixes to BUILD-4B close artifact §1
   (stale status + diff-stat) and ANSPEC-002 §14/§16.3 marker text;
   broader sweep of ANSPEC-001/ANSPEC-002/ADR-* for stale Bible version
   references.

All three are doc-only. None modify normative claims. None propose Bible
amendments. None modify ADR-021's substrate or BUILD plan.

This is a single dispatch with one close artifact. Prometheus may choose
the order in which items are drafted; PM has no preference.

## 2. Item 1 — ADR-022 Fault-Recovery Direction Forward Marker

### 2.1 Output

```text
docs/ADR-022-fault-recovery-direction-forward-marker.md
```

### 2.2 What the ADR must capture

Following the existing ADR shape (see ADR-021 for the closest stylistic
reference):

**Context section.** Brief summary of why fault recovery becomes relevant
for NeoPlutarch:

- Bible Article 1.2 (correctness > coherence > performance) and Article
  15.2 (`unsafe` discipline) currently rely on "make the bug impossible"
  as the primary fault-elimination strategy;
- Phase 4+ (network stack) and Phase 6+ (accelerator drivers) introduce
  fault categories where "graceful component restart" is meaningfully
  better than panic;
- ALTIMIT research report §F recommends Theseus-style cell-swap as the
  pattern;
- Bible Article 9.2 currently forbids loadable kernel modules in a phrasing
  broad enough to read as forbidding Theseus-style cell-swap too.

**Decision section.** The decision being recorded:

> AT THIS TIME (2026-05-20): NeoPlutarch does NOT commit to Theseus-style
> cell-swap fault recovery. No Bible amendment to Article 9.2 is proposed.
> No engineering work toward cell-swap is on the phase roadmap.
>
> This ADR records the consideration as a forward marker so the decision
> to defer is explicit and discoverable, rather than implicit and
> rediscovered.

**Conditions for revisit.** The ADR should name the concrete conditions
under which the project would revisit the cell-swap question:

a. A Phase 4+ network-stack failure mode is identified where driver
   panic costs significantly more (lost connection state, in-flight
   request loss) than a graceful driver restart would.
b. A Phase 6+ accelerator driver fault mode is identified where the
   alternative to graceful restart is killing in-flight inference
   contexts owned by other processes.
c. An equivalent pattern (Asterinas OSTD framekernel, or another
   intralingual fault-isolation design) becomes substantially easier
   to adopt than Theseus cell-swap and produces similar guarantees.

**Anti-decision section.** Explicitly NOT decided by this ADR:

- Whether the project will EVER adopt cell-swap. It might never.
- Whether Article 9.2 will eventually be amended. That's a separate
  Bible-amendment process per Article 27.
- The specific architectural shape of any future fault-recovery work.
- Whether Asterinas OSTD framekernel discipline (separate from cell-
  swap; concerns `unsafe`-boundary segregation) is also adopted. The
  research report names both; ADR-022 names only the cell-swap question.

**Related ADRs section.** Reference ADR-019 (`SwitchMutex` IRQ-off-by-
construction) as a related example of how the project handles
substrate-level discipline without architectural commitment.

### 2.3 What the ADR must NOT do

- Propose a Bible amendment.
- Commit to any phase-roadmap change.
- Specify implementation details of any fault-recovery mechanism.
- Resolve the Article 9.2 tension (that resolution is itself the
  decision to be deferred).
- Substantively amend ADR-021 or any prior ADR.
- Use the ALTIMIT research report as authoritative; cite it as
  informational input only.

### 2.4 Status field

The ADR's `Status` field should read:

```text
Status: Accepted (forward marker; no architectural commitment)
```

Following the existing ADR-021 pattern of allowing a status that records
"this was decided" without implying ongoing implementation work.

## 3. Item 2 — ANSPEC-001 §13 Q6 KernelAttestation

### 3.1 Output

In-place addition to:

```text
docs/specs/neoplutarch-ai-subsystem-spec.md
```

Specifically, append to §13 Open Questions (currently Q1–Q5) a new Q6
following the existing brief style.

### 3.2 What Q6 must say

Drawing from research report §K and §C (verifiable kernel identity for AI
context):

**Q6.** What is the API and authority shape for a `KernelAttestation`
primitive (research report §K) that exposes verifiable kernel identity
to userspace and remote peers? Open sub-questions:

a. TPM-backed (DICE / measured-boot anchored) vs software-measured-boot
   fallback when no TPM is present;
b. How `KernelAttestation` composes with capability discipline (is it a
   capability, a syscall, or a special object class?);
c. Whether attestation tokens are revocable or one-shot;
d. Whether the primitive belongs in Phase 2 (where capability substrate
   exists) or Phase 8 (where hardware-target diversity makes
   TPM-availability variability a deployment concern);
e. Composition with `ConfidentialInference` (TEE+HE) primitives, which
   are explicitly Phase 8+ per ANSPEC-002 §11.3.

This question is informational. ANSPEC-001 does not normatively commit to
implementing `KernelAttestation`; the Open Question records that the
primitive is on the project's research horizon and that ANSPEC-002 v0.1.1
already references it as a capability-system forward marker (research
report §K, Stage 2 work).

### 3.3 What Q6 must NOT do

- Resolve any sub-question.
- Modify any other section of ANSPEC-001.
- Modify any §13 Open Question Q1–Q5 in place.
- Propose a syscall ABI.
- Cross-reference future ANSPEC-003 or other not-yet-existing artifacts.

### 3.4 Style notes

Match the existing Q1–Q5 brevity. Each existing question is roughly 2–4
sentences. Q6 can be slightly longer because the sub-questions warrant
enumeration, but the prose-to-bullets balance should match the existing
section.

## 4. Item 3 — Doc Hygiene Sweep

### 4.1 Targeted fixes

**4.1.a BUILD-4B close artifact stale status header.**

File: `Plans/PHASE-3-BUILD-4B-CLOSE-EVIDENCE-2026-05-20.md`

Current §1 header:

```text
Status: IN PROGRESS — substrate landed, all 7 required scenarios PASS,
anti-regression in flight
```

Replace with:

```text
Status: CLOSED — accepted by PM in
Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4B-close-consumption.md;
commit 3097126 ratified
```

**4.1.b BUILD-4B close artifact diff-stat reconciliation.**

File: `Plans/PHASE-3-BUILD-4B-CLOSE-EVIDENCE-2026-05-20.md`

The §1 diff stat reads "1117 insertions, 64 deletions" but the final
landed commit `3097126` carries "1322 insertions, 64 deletions" (the
delta is test/evidence additions that landed between §1 capture and
final commit).

Reconcile to the final commit's numbers. The right verification is:

```text
git show --stat 3097126
```

Update the §1 diff stat to match the actual landed commit. If the
mismatch is itself worth noting historically, add a brief one-line
parenthetical noting that the original §1 stat reflected a pre-final-
commit snapshot.

**4.1.c ANSPEC-002 §14 Bible version reference.**

File: `docs/specs/neoplutarch-ai-capability-semantics-spec.md`

Current §14 References:

```text
NeoPlutarch Bible v1.0.4, Articles 6 (including 6.6, inference-governance
hooks), 12, 20–23.
```

Replace with:

```text
NeoPlutarch Bible v1.6.0, Articles 6 (including 6.6, inference-governance
hooks), 12, 20–23.
```

Article addresses (6, 6.6, 12, 20–23) verify against current v1.6.0; only
the version number is updated.

**4.1.d ANSPEC-002 §16.3 deferred-marker text update.**

File: `docs/specs/anspec-002-v011-amendment-DRAFT.md`

Current §16.3 marker:

```text
[DEFERRED PENDING BUILD-4B CLOSURE] — Mount-table interaction with cascade
revocation will be specified in a subsequent amendment after BUILD-4B's
mount-capability semantics are finalized.
```

Replace with:

```text
[DEFERRED TO PHASE 5 THINK] — Mount-table semantics are now visible per
BUILD-4B commit 3097126; resolution of this question belongs with the
Phase 5 AI-object capability design per PM-NOTE-2026-05-20-anspec-002-q6-
deferral-to-phase-5-think.md.
```

Same update applies to the corresponding Open Question Q6 entry in the
amendment draft if its marker text is stale; verify and update in place.

### 4.2 Broader Bible-version sweep

Sweep across `docs/specs/` and `docs/ADR-*.md` for stale Bible version
references. Specifically search for:

- "Bible v1.0.4"
- "Bible v1.5"
- "Bible v1.0"
- Any other Bible version reference that does not match the current
  v1.6.0.

For each occurrence:

a. Verify that the article addresses cited (e.g. "Article 6.6", "Article
   23.1") still exist at those addresses in current v1.6.0. They should;
   the Bible amendment history has added articles but not renumbered.
b. Update the version number to v1.6.0.
c. If an article address turns out to NOT match current v1.6.0 (i.e. the
   article has moved or been renamed), do NOT fix in place. Surface in
   the close artifact as a finding for PM consumption.

The sweep is mechanical text-edit work. Do not attempt to validate the
substance of the citation; only the version number and (lightly) the
existence of the cited article.

### 4.3 Out of scope for this sweep

- No new ADRs.
- No Bible amendments.
- No normative changes to any spec.
- No edits to Plans/ artifacts other than the one BUILD-4B close artifact
  named in 4.1.a/4.1.b.
- No edits to research-report content.
- No edits to source code.

## 5. Anti-Scope (Applies To All Three Items)

NOT in this dispatch:

- Bible amendments. If something in items 1/2/3 suggests an amendment is
  needed, surface in the close artifact; do not draft amendment text.
- New normative claims in any spec.
- Implementation work of any kind.
- Modifications to ADR-021, ADR-019, or other ratified ADRs.
- Phase-roadmap changes.
- Reconciliation of any disagreement between the research report and
  current Bible/ANSPEC posture; that's for PL.
- Modifications to BUILD-4C lane (Jarvis owns that).
- Verification of substrate code (source files, kernel internals).

## 6. Stop Conditions

Stop and surface to PM if:

- The Bible-version sweep finds an article address that no longer exists
  at the cited location in current v1.6.0. Surface; do not "fix" by
  guessing a new address.
- The ADR-022 draft starts requiring engineering decisions PM didn't
  authorize (e.g. specifying exactly which fault recovery mechanism
  would be adopted). ADR-022 is a forward marker, not an implementation
  decision.
- The ANSPEC-001 Q6 addition reveals that an existing Q1–Q5 entry is
  inconsistent with current Bible or ADR posture. Flag the finding; do
  not amend Q1–Q5 in this dispatch.
- A doc-hygiene edit would change a normative claim. Stop; surface.
- The close artifact exceeds reasonable size (~150 lines) suggesting the
  sweep found more than expected. Chunk and surface partial completion.

## 7. Required Close Artifact

Create:

```text
Plans/PROMETHEUS-CLOSE-2026-05-20-three-item-followup-bundle.md
```

Required content:

- substrate commit hash;
- list of files created or modified, with brief description per file;
- for ADR-022: confirmation that no architectural commitment, Bible
  amendment, or phase-roadmap change is in the artifact;
- for ANSPEC-001 §13 Q6: confirmation that no normative content was added
  and that Q1–Q5 were not modified in place;
- for doc hygiene: enumerated list of edits made, with file/section
  references; explicit confirmation that no normative claim was altered;
- broader Bible-version sweep results: list of files where references
  were updated, with old-and-new version strings;
- any findings surfaced for PM (article-address mismatches, unexpected
  inconsistencies);
- explicit confirmation: "all three items are informational per Bible
  Article 18.1; ADR-022 ratification status is per Article 27 PL
  signoff; ANSPEC-001 Q6 is an additive Open Question and does not
  change ANSPEC-001's normative content."

## 8. PM Posture

PM monitoring. Three concurrent lanes:

- Jarvis: BUILD-4C, awaiting mtools verification per
  `Plans/PM-NOTE-2026-05-20-build-4C-mtools-stop-condition-resolution.md`,
  then resumes implementation.
- Prometheus: this dispatch (three-item bundle).
- Cerebro: idle.

These are independent lanes. No coordination required.

When Prometheus closes, PM consumes the artifact. ADR-022 then surfaces
to PL for whatever ratification disposition PL chooses (accept, request
revisions, treat as informational only); ANSPEC-001 Q6 addition surfaces
to PL for awareness but does not require ratification (Open Questions
are informational by convention); doc hygiene needs no PL signoff but
PM acknowledges completion.

Expected turnaround: skeleton complete in one Prometheus working session.
Tight scope; no design work; mostly text editing and one short ADR
authoring task.
