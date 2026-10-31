# Prometheus Close Artifact — ANSPEC-002 Gap Review

Date: 2026-05-20
Agent: Prometheus
Dispatch: `Plans/DISPATCH-2026-05-20-anspec-002-gap-review-and-v011-amendment-prometheus.md`
Status: COMPLETE — amendment draft produced, awaiting PL ratification

---

## Substrate

```
0682d24 test/wake-token: wrap remaining WAKE_TOKEN_STATE writes in without_interrupts
```

---

## Gaps Found

Five gaps identified from reading ANSPEC-002 v0.1.0 against the ALTIMIT research report (§§A–L, Stage 0–3 roadmap):

### Gap 1: Capability derivation depth requirement (CLOSED by amendment)

**Research report reference:** §G (composition safety), §C (five primitives imply depth ≥4 derivation chains)
**Amendment:** New §15 in v0.1.1 draft
**Disposition:** Closed. The amendment names the depth ≥4 requirement explicitly so cap-table design choices do not foreclose it.

### Gap 2: Cascade revocation on underlying-storage destruction (CLOSED by amendment, mount interaction DEFERRED)

**Research report reference:** §G (composition safety), ADR-021 Q9 (NeoFS inode type IDs for AI objects)
**Amendment:** New §16 in v0.1.1 draft
**Disposition:** Closed at the AI-object level. The cascade-revocation constraint is stated normatively. Mount-interaction specifics are marked `[DEFERRED PENDING BUILD-4B CLOSURE]` and surfaced as Open Question Q6.

### Gap 3: No ambient AI authority, explicit (CLOSED by amendment)

**Research report reference:** §J (ambient AI as substrate vs system persona), Bible Article 23
**Amendment:** New §17 in v0.1.1 draft
**Disposition:** Closed. The constraint is stated normatively: no global model registry cap, no implicit model authority.

### Gap 4: Content-addressed Model identity (CLOSED by amendment)

**Research report reference:** §E (content-addressed model deduplication, Hugging Face Xet + ZipLLM pattern)
**Amendment:** New §18 in v0.1.1 draft
**Disposition:** Closed. The discipline is stated: Model identity is content-addressed (hash of tensor lineage, quantization, layout). LoRA/fine-tune delta composition is named. Hash algorithm and chunking strategy deferred to ADR-021/Phase 5 implementation.

### Gap 5: Accelerator capability surface — vAccel (CLOSED by amendment)

**Research report reference:** §D (multi-tenant accelerator scheduling, kernel-owned accel resources)
**Amendment:** New §19 in v0.1.1 draft
**Disposition:** Closed. `vAccel` capability added with reserved semantic-rights bits. Composition with AccelTask specified. Userspace surfacing deferred to Phase 7+.

---

## Items Explicitly Out of Scope for ANSPEC-002

The following research report recommendations were identified as belonging elsewhere:

| Recommendation | Research § | Why out of scope for ANSPEC-002 |
|---|---|---|
| Verifiable kernel identity, KernelAttestation primitive | §K | Belongs with Bible amendment (Article 23.4 candidate) and/or ANSPEC-001 subsystem spec |
| Theseus-style cell-swapping fault recovery | §F | Bible amendment candidate (fault recovery article); not a capability semantics concern |
| Single-node scope clarification | §H | Already in Bible as Article 4.5; PM rescission note confirms |
| OSTD-style unsafe-boundary discipline | §F | Kernel engineering policy, not AI capability semantics |
| Confidential inference (TEE+HE) primitives | §L, Stage 3 | Phase 8+ frontier; ANSPEC-002 §11.3 already defers to Phase 7+ research |

---

## Bible-Citation Inconsistencies in v0.1.0

The following citation in the existing v0.1.0 was found inconsistent with the current Bible v1.6.0:

- **ANSPEC-002 §14 References** cites "NeoPlutarch Bible v1.0.4, Articles 6 (including 6.6, inference-governance hooks), 12, 20–23." The current Bible is v1.6.0. Article 6.6 exists at that address and has not moved. The version number in the reference is stale but the article addresses are correct. **Flagged, not fixed** — Bible-citation correction is a separate focused task per dispatch §4.2.

No other Bible-citation addresses were found incorrect. All cited articles (6, 6.6, 12, 20–23) exist at their stated addresses in Bible v1.6.0.

---

## Additive-Only Confirmation

No existing section of ANSPEC-002 v0.1.0 was rewritten in place. All changes are additive:

- New §15: Capability derivation depth requirement
- New §16: Cascade revocation on underlying-storage destruction
- New §17: No ambient AI authority
- New §18: Content-addressed Model identity
- New §19: Accelerator capability surface (vAccel)
- §12 Open Questions: Added Q6, Q7 (existing Q1–Q5 untouched)
- §14 References: Added two entries (existing entries untouched)

---

## Stop-Condition Check

- No existing v0.1.0 normative claim was found inconsistent with Bible v1.6.0 in a way requiring retraction. The only inconsistency was a stale version number in the references section (flagged, not a normative claim).
- BUILD-4B did not close during drafting.
- No research-report recommendation required ANSPEC-001 changes rather than ANSPEC-002 changes. Items that belonged with ANSPEC-001 or the Bible were identified as out of scope and surfaced in the table above.
- Exactly 5 gaps were found, which is at the boundary of the "~5 gaps" stop condition. The work was not chunked because all 5 gaps are coherent (they extend the capability semantics spec with constraints the research report recommends) and the amendment draft is manageable in size (~200 lines of new content).

---

## Artifact Status

> This artifact is a v0.1.1 amendment proposal, informational per Bible Article 18.1, pending PL ratification per Article 27.

Amendment draft: `docs/specs/anspec-002-v011-amendment-DRAFT.md`

---

**End of close artifact.**
