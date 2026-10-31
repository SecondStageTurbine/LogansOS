# ANSPEC-002 v0.1.1 Amendment Draft

**Document ID:** ANSPEC-002
**Amendment version:** 0.1.1-DRAFT
**Base version:** 0.1.0 (2026-05-05)
**Date:** 2026-05-20
**Status:** Amendment proposal — informational per Bible Article 18.1, pending PL ratification per Article 27
**Amended by:** Prometheus (gap review against ALTIMIT research report)
**Format:** Diff-style amendment proposal showing section-by-section additions and refinements

---

## Changelog v0.1.0 → v0.1.1

| Section | Change | Rationale |
|---|---|---|
| New §15 | Capability derivation depth requirement | Research report §G implies depth ≥4; v0.1.0 was silent on implementation depth constraints |
| New §16 | Cascade revocation on underlying-storage destruction | ADR-021 Q9 reserves NeoFS inode type IDs for AI objects; v0.1.0 provenance (§9) makes cascade derivable but did not state it as a normative constraint |
| New §17 | No ambient AI authority | Research report §J + Bible Article 23 foreclose global model registry; v0.1.0 implied but did not state explicitly |
| New §18 | Content-addressed Model identity | Research report §E recommends content-addressed storage; v0.1.0 named immutability but not content-addressing discipline |
| New §19 | Accelerator capability surface (vAccel) | Research report §D recommends capability-gated multi-tenant accel scheduling; v0.1.0 reserved AccelTask but not the accelerator resource cap itself |
| §12 Open Questions | Added Q6, Q7 | Open questions for items that cannot be closed without PL decision or BUILD-4B closure |

---

## Amendment: New §15 — Capability Derivation Depth Requirement

### 15.1 The constraint

The ALTIMIT research report §G implies a derivation chain of depth ≥4 for Phase 5+ AI workflows:

```
Process -> ModelCap -> [TensorCaps...] -> [NeoFS InodeCaps...]
```

with `InferenceContext` introducing orthogonal fan-in:

```
InferenceContext -> {ModelCap, TensorCaps, AccelTask, LogitPort}
```

The capability table implementation MUST support derivation chains of depth at least 4 without truncation, recursion-bounded walk failure, or fixed-size derivation-log overflow. This includes:

- The per-process derivation log (ADR-010 Q1) must not impose a depth limit that would prevent a cap at depth 4 from being recorded.
- The `is_subset_of` walk (ADR-010 Q5) must not have a recursion bound that terminates before reaching depth 4.
- Any fixed-size derivation-log array, parent-chain buffer, or similar implementation detail must accommodate depth ≥4.

### 15.2 Why this matters

Phase 5+ AI workflows create deep derivation chains naturally: a process loads a Model from NeoFS storage (depth 1: InodeCap), the Model mint creates ModelCap (depth 2), inference produces output Tensors derived from the ModelCap (depth 3), and each Tensor's backing storage is a NeoFS inode with its own InodeCap (depth 4). A cap-table implementation with a depth limit of 2 or 3 would silently truncate or reject valid AI-object derivation chains.

This constraint is informational for the cap-table design choices Cerebro makes in current and future builds. It does not prescribe a specific implementation; it names the minimum depth the implementation must support.

### 15.3 What this does not constrain

This section does not prescribe the cap-table data structure (flat log, hierarchical CSpace, parent→children index, etc.). ADR-010 Q1 already decided flat per-process derivation log for Phase 2; Phase 5+ may change the data structure if cap counts require. The depth constraint applies regardless of data structure choice.

---

## Amendment: New §16 — Cascade Revocation on Underlying-Storage Destruction

### 16.1 The constraint

Capability revocation on underlying-object destruction MUST cascade. If a Tensor's backing NeoFS inode is destroyed, or the inode generation advances such that the underlying FileCap is stale, every derived TensorCap referencing it MUST return ESTALE on next use. Same discipline as BUILD-2 FileCap generation check, generalized to AI objects.

Formally:

> If cap C was derived (directly or transitively) from storage-backed cap S, and S becomes stale (generation advance, inode destruction, or equivalent), then C becomes stale. The staleness is detected lazily: C remains a valid handle until next invocation, at which point the kernel returns ESTALE.

### 16.2 Derivation chain for cascade

The provenance discipline from §9 makes the cascade walk derivable:

1. Walk parent links in the derivation log from the AI cap to the storage backing.
2. At each hop, check whether the underlying storage cap is stale (generation mismatch, destroyed inode, etc.).
3. If any link in the chain is stale, the derived cap is stale.

This is the same pattern ADR-010 §Q1's derivation log enables for Channel and Process caps, generalized to AI objects with storage backing.

### 16.3 Mount-interaction specifics — deferred

The exact shape of cascade revocation when a mounted filesystem is unmounted depends on BUILD-4B's mount-table semantics:

- Does the mount table treat outstanding ModelCaps as live handles (forcing EBUSY on unmount)?
- Or does it allow unmount by invalidating caps (cascade-revocation targets)?

This choice is PL's call after BUILD-4B closes. The cascade-revocation requirement at the AI-object level stands; the mount-interaction specifics are deferred.

**[DEFERRED TO PHASE 5 THINK]** — Mount-table semantics are now visible per BUILD-4B commit 3097126; resolution of this question belongs with the Phase 5 AI-object capability design per PM-NOTE-2026-05-20-anspec-002-q6-deferral-to-phase-5-think.md.

### 16.4 What this does not change

This section does not modify ADR-010's revoke discipline. ADR-010 §Q2's `sys_revoke` is an explicit, userspace-initiated operation. Cascade revocation is implicit and kernel-initiated, triggered by storage-layer events. The two are complementary: explicit revoke invalidates caps immediately; cascade revocation marks caps as stale on next use.

---

## Amendment: New §17 — No Ambient AI Authority

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

## Amendment: New §18 — Content-Addressed Model Identity

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

## Amendment: New §19 — Accelerator Capability Surface (vAccel)

### 19.1 Object identity

The ALTIMIT research report §D recommends that the kernel — not a userspace daemon — owns the accelerator's compute and memory units as schedulable resources, virtualized as `vAccel` capabilities. ANSPEC-002 §6 names `AccelTask` as the unit of accelerator scheduling but does not name the capability representing the accelerator resource itself.

This section adds the `vAccel` capability: a handle representing access to a partitioned accelerator resource (e.g., an NVIDIA MIG instance, a vNPU slice, a CXL memory region).

### 19.2 Semantic-rights bits (reserved)

```rust
/// vAccel semantic-rights. Phase 6+ kernel-internal; userspace surfacing
/// deferred to Phase 7+ if a use case justifies.
pub mod vaccel_rights {
    /// Dispatch compute work to this accelerator partition.
    pub const DISPATCH:   u8 = 1 << 0;
    /// Allocate memory in this accelerator's address space.
    pub const ALLOC_MEM:  u8 = 1 << 1;
    /// Query accelerator state (load, temperature, available memory).
    pub const INSPECT:    u8 = 1 << 2;

    // Reserved per ADR-010 Principle 2.
    /// Future: migrate a running task to this accelerator partition.
    pub const MIGRATE_IN: u8 = 1 << 3;
    /// Future: release accelerator resources back to the pool.
    pub const RELEASE:    u8 = 1 << 4;
}
```

### 19.3 Composition with AccelTask

When the scheduler creates an `AccelTask`, the task's permission to use a specific accelerator partition is derived from the issuing process's `vAccel` cap on that partition. An `AccelTask` cannot dispatch to an accelerator the calling process doesn't have a `vAccel` cap for. This extends the defense-in-depth pattern from §6.4 to the accelerator resource itself, not just the model and tensor caps.

### 19.4 Phase target

`vAccel` is Phase 6+ (accelerator drivers). Userspace surfacing is deferred to Phase 7+ if a use case justifies direct accelerator partition management by userspace. Until then, `vAccel` caps are kernel-internal and the inference scheduler manages partition assignment.

---

## Amendment: §12 Open Questions — Additions

**Q6.** How does cascade revocation (§16) interact with BUILD-4B's mount-table live-handle accounting? Specifically: if a process holds a ModelCap derived from a model stored on a mounted filesystem, and the filesystem is unmounted, does the ModelCap become stale (cascade revocation) or does the unmount block (live handle)? **[DEFERRED TO PHASE 5 THINK]** — Mount-table semantics are now visible per BUILD-4B commit 3097126; resolution of this question belongs with the Phase 5 AI-object capability design per PM-NOTE-2026-05-20-anspec-002-q6-deferral-to-phase-5-think.md.

**Q7.** Should `vAccel` caps (§19) be grantable to userspace in Phase 6, or remain kernel-internal until Phase 7+? The research report §D recommends kernel-owned accelerator resources with capability-gated admission, but doesn't specify whether the cap should cross the syscall boundary. If `vAccel` is kernel-internal, the scheduler's partition assignment is opaque to userspace; if grantable, userspace can manage accelerator partitions directly. This affects the `AccelTask` composition model in §6.4.

---

## Amendment: §14 References — Addition

- ALTIMIT research report (`docs/research/ALTIMIT-real-world-implementation.md`), §§C, D, E, G, J, K, Recommendations (Stage 0–3).
- ADR-021 — Phase 3 NeoFS design substrate, Q9 (typed inode discriminants for Model, Tensor, InferenceContext).

---

**Amendment end.**

*This is a v0.1.1 amendment proposal, informational per Bible Article 18.1, pending PL ratification per Article 27. No existing section was rewritten in place; all changes are additive (new sections §15–§19, additions to §12 and §14).*
