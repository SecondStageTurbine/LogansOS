# ANSPEC-002 v0.1.1 Amendment — Ratified 2026-05-28

**Document ID:** ANSPEC-002
**Amendment version:** 0.1.1-RATIFIED
**Base version:** 0.1.0 (2026-05-05)
**Date filed:** 2026-05-20
**Date ratified:** 2026-05-28
**Status:** RATIFIED per Bible Article 27
**Authority:** PL (2026-05-28 ratification)
**Format:** Additive amendment; new sections §17–§18, additions to §12 and §14

---

## Ratification Memo

PL ratified Option A (conservative): accept §17, §18, and §12/§14 additions only. Defer §15, §16, §19 to Phase 5/6 THINK when context is complete.

The two ratified sections are essential architectural constraints:
- **§17 — No Ambient AI Authority:** Core discipline (Bible Article 23, research §J)
- **§18 — Content-Addressed Model Identity:** Essential for dedup strategy (research §E, ADR-021 Phase 5 direction)

---

## Amendment §17 — No Ambient AI Authority

### 17.1 The constraint

There is no global "model registry capability" that confers authority to invoke any loaded model. Every model invocation requires a ModelCap derived from a process-explicit load or grant.

Formally:

> The kernel MUST NOT maintain a capability, handle, or authority token that is implicitly available to all processes and that would permit model invocation without an explicit derivation chain. No kernel-global ModelCap exists. No ambient model authority exists.

### 17.2 Bible alignment

Bible Article 6.6 establishes that the kernel exposes inference-governance hooks but does not implement governance policy. Bible Article 23 forecloses a system AI assistant. A complementary cap-side constraint follows: if the kernel held a global ModelCap accessible to all processes, it would effectively be an ambient model authority — a mechanism by which any process could invoke any model without explicit capability derivation. This violates the spirit of Article 23 by introducing ambient AI authority, even if technically implementable.

This is the ALTIMIT-canon parallel from the research report §J: ALTIMIT is substrate, not a holder of model authority. The OS that embedded authority (Morganna) is the one that failed.

### 17.3 Implementation implication

The `sys_model_load` mint site (§4.2) is the only origin of ModelCaps. A ModelCap exists only if:

1. A process explicitly called `sys_model_load` and received the minted handle, or
2. A process received a ModelCap via `sys_grant` from a holder that had one through (1).

No kernel-internal "default" ModelCap. No boot-time model preload with implicit authority. The kernel's inference scheduler (§6.4 AccelTask composition) accesses models through the issuing process's caps, not through a kernel-owned handle.

### 17.4 What this does not prevent

A supervisor process (e.g., neo-init) may load a model and grant attenuated ModelCaps to child processes at boot. This is explicit derivation, not ambient authority. The constraint only forbids implicit availability — the cap must have a traceable derivation chain from an explicit load or grant.

---

## Amendment §18 — Content-Addressed Model Identity

### 18.1 The discipline

A `Model` object in the kernel is identified by the hash of (tensor lineage, quantization, layout). This is the content-addressing discipline recommended by the ALTIMIT research report §E, which references the Hugging Face Xet and ZipLLM patterns.

Specifically:

- A Model's canonical identity is a hash derived from its weight tensor contents, quantization parameters, and memory layout.
- Loading the same model (by content) twice produces handles to the same kernel object, not two independent copies.
- The deduplication is achieved at allocation time: the second load recognizes the content address and increments the reference count rather than allocating new storage.

### 18.2 Composition with capability discipline

Content-addressed identity is orthogonal to capability discipline. Two processes may hold ModelCaps for the same underlying model (same content address) but with different rights, different derivation chains, and different grant histories. The kernel deduplicates the storage; the capability system manages the authority.

This is consistent with Bible Article 10.2's direction: NeoFS is content-addressed at the block layer, models are deduplicated by content hash. ANSPEC-002 names the discipline at the AI-object level; ADR-021 implements it at the filesystem level.

### 18.3 LoRA adapters and fine-tunes

LoRA adapters and fine-tunes are deltas keyed off a base model's content address. The derived model's identity is hash of (base content address + delta parameters). This enables BitX-style delta compression (ZipLLM pattern) at the storage layer without changing the capability semantics: a fine-tuned model is a distinct ModelCap with its own derivation chain, even though its storage is a compressed delta against a base.

### 18.4 What this does not specify

This section does not prescribe the hash algorithm, chunking strategy, or compression format. Those are ADR-021 and Phase 5 implementation decisions. ANSPEC-002 names the discipline: Model identity is content-addressed, not path-addressed.

---

## Amendment: §12 Open Questions — Additions (ratified)

**Q6.** How does cascade revocation interact with BUILD-4B's mount-table live-handle accounting? **[DEFERRED TO PHASE 5 THINK]**

**Q7.** Should vAccel caps be grantable to userspace in Phase 6, or remain kernel-internal? **[OPEN FOR PHASE 6 THINK]**

---

## Amendment: §14 References — Additions (ratified)

- ALTIMIT research report (`docs/research/ALTIMIT-real-world-implementation.md`), §§C, E, J (references for content-addressing, no ambient authority, and AI-as-substrate)
- ADR-021 — Phase 3 NeoFS design substrate, Q9 (typed inode discriminants for AI objects)

---

## Deferred Sections (§15, §16, §19)

Sections on capability derivation depth (§15), cascade revocation (§16), and vAccel capability surface (§19) are **deferred to Phase 5/6 THINK** when design context is complete.

---

**Amendment end.**

*This amendment ratifies §17, §18, and §12/§14 additions to ANSPEC-002 v0.1.0. Sections §15, §16, §19 (from the v0.1.1 draft) are deferred and will be revisited in Phase 5/6 design cycles.*