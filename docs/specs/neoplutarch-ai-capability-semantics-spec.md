# NeoPlutarch AI Capability Semantics

## Specification of Per-Object Capability Rights for AI Subsystem Primitives

---

**Document ID:** ANSPEC-002
**Status:** Draft (forward-looking; Phase 5+ implementation)
**Authority:** Derived from NeoPlutarch Bible Articles 6, 12, 20–23, and ADR-010 §"Future Phase 5+ extension"
**Target phases:** 5 (Tensor, Model, InferenceContext caps), 6 (AccelTask reservation), 7+ (LogitPort, conjunctive-safety analysis)
**Version:** 0.1.0
**Last revised:** 2026-05-05
**Composes with:** ANSPEC-001 (AI subsystem design), ADR-010 (capability system foundation), ADR-013 §Q8 (SMP cap-table composition)

---

## 0. Document Purpose

This specification enumerates capability rights for the five AI-subsystem primitives named in the ALTIMIT research artifact §G — Tensor, Model, InferenceContext, AccelTask, and LogitPort — following ADR-010's two-tier rights model: a uniform meta byte (`USE | DERIVE | GRANT | REVOKE`) governing universal capability operations, plus a per-variant semantic byte governing operations specific to each primitive. It specifies derivation patterns, attenuation patterns, provenance discipline, and the transitive-closure invariant that Phase 5+ enforces automatically via ADR-010 §Q5's bit-subset.

This specification does not implement anything. It does not specify the conjunctive-safety analysis algorithm (research-frontier deferred to Phase 7+). It does not amend the Bible — that is a separate process per Article 27. It does not change ADR-010 or ADR-013; it composes with both.

This document is normative. Code that contradicts this spec is wrong; either the code changes or this spec is amended via the standard ADR + bible-amendment process.

---

## 1. Rationale

### 1.1 Why capability rights need their own spec

ADR-010 establishes the capability machinery: mint, derive, grant, revoke, the two-tier rights model (meta + semantic bytes), the three named principles (Defense in depth, Reserved-bit pattern, Errors at causing actor), and the per-process derivation log. ANSPEC-001 establishes the AI subsystem objects — Tensor, Model, InferenceContext — but specifies capability rights only for Tensor (§3.7), and that specification predates ADR-010's two-tier model. ANSPEC-001 §3.7 names three permissions (`READ`, `WRITE`, `SHARE`) with no separation between meta-rights (universal cap operations like passing) and semantic-rights (tensor-specific operations). The vocabulary was correct for a pre-ADR-010 world; it is stale in ADR-010's.

ANSPEC-002 fills the gap: per-object semantic-rights enumerations for all five AI primitives, derivation patterns specific to AI workflows, provenance discipline at mint sites, and explicit composition rules with ADR-010 and ADR-013. ANSPEC-001 §3.7 is amended atomically with this spec's introduction (see §3.4 and Artifact 2 in the originating dispatch).

### 1.2 Why five primitives, not three

ANSPEC-001 names three layers: Tensor (Layer 0), Model (Layer 1), InferenceContext (Layer 2). The ALTIMIT research artifact §G adds two more — AccelTask and LogitPort — derived from the kernel-arbitration argument (Bible Article 6.4: "Accelerators are exposed via the accel driver class... The kernel's inference scheduler chooses where to run each op") and the "kernel-mediated inference governance" research-frontier direction named in §G's Recommendation 9.

ANSPEC-002 specifies all five, with explicit phase markers:

- **Tensor, Model, InferenceContext:** Phase 5 implementation.
- **AccelTask:** Phase 6+ kernel-internal; userspace surfacing deferred to Phase 7+ if a use case justifies.
- **LogitPort:** Phase 7+ research-frontier.

---

## 2. Scope

### In scope

- Per-primitive semantic-rights bit enumerations (§3 through §7).
- Derivation patterns: which attenuated handles are first-class (e.g., model handle without weight-read; inference context observer-only).
- Provenance/lineage rules: how parent-cap-id records identify lineage chains for AI objects (§9).
- The conjunctive-safety problem statement and the transitive-closure invariant the kernel enforces in Phase 5+ (§8).
- Cross-process capability flow under SMP — cites ADR-013 §Q8; does not duplicate it.

### Out of scope

- **Implementation.** ANSPEC-002 is a spec; Phase 5+ implements.
- **The conjunctive-safety analysis algorithm.** Research-frontier; deferred. ANSPEC-002 names the problem; future work specifies the algorithm.
- **LogitPort implementation details.** Phase 7+ research; ANSPEC-002 establishes the cap surface only.
- **Bible amendments.** Article 6.6 (kernel exposes inference-governance hooks without implementing governance policy) is governed by Article 27's amendment process and lives in the Bible itself; ANSPEC-002 cross-references but does not own it.
- **Modifying ADR-010 or ADR-013.** ANSPEC-002 composes; does not change either.
- **Training-related capabilities.** Per Bible Article 23.4 and ANSPEC-001 §1.3, NeoPlutarch defines its own inference framework and training is out of scope for the kernel; capability rights for training do not appear here.

---

## 3. The Tensor Capability Surface

### 3.1 Two-tier model applied

Tensor capabilities follow the two-tier model defined in ADR-010 §Q3. The meta byte is uniform across all Handle variants: `USE | DERIVE | GRANT | REVOKE`. The semantic byte carries tensor-specific rights defined in §3.2.

### 3.2 Semantic-rights bits

```rust
/// Tensor semantic-rights. Composes with the uniform meta byte from ADR-010 §Q3.
pub mod tensor_rights {
    /// Sample tensor contents into userspace (sys_tensor_read).
    pub const READ:        u8 = 1 << 0;
    /// Mutate tensor contents (sys_tensor_write); requires non-IMMUTABLE tensor.
    pub const WRITE:       u8 = 1 << 1;
    /// Migrate to a different device (sys_tensor_migrate); requires non-PINNED.
    pub const MIGRATE:     u8 = 1 << 2;
    /// Create a derived view (sys_tensor_view) sharing the same storage.
    pub const VIEW:        u8 = 1 << 3;

    // Reserved per ADR-010 Principle 2 (Reserved-bit pattern). Bits defined
    // now; gating syscalls land in later phases.
    /// Future: pin to a specific device, reject migration.
    pub const PIN:         u8 = 1 << 4;
    /// Future: declare DEDUP_CANDIDATE eligibility (currently a kernel-managed flag, not a cap right).
    pub const DEDUP_HINT:  u8 = 1 << 5;
}
```

### 3.3 Mint sites

`sys_tensor_create` mints with full default rights: `CapRights { meta: USE|DERIVE|GRANT|REVOKE, semantic: READ|WRITE|MIGRATE|VIEW }`. Weight tensors minted by `sys_model_load` mint without `WRITE` (weights are `IMMUTABLE` per ANSPEC-001 §3.6).

### 3.4 Reconciliation with ANSPEC-001 §3.7

This section supersedes ANSPEC-001 §3.7's three-rights description (`READ`/`WRITE`/`SHARE`). The mapping is:

- `SHARE` → `meta::GRANT` — sharing is a universal capability operation (meta-right), not a tensor-specific one.
- `READ` → `tensor_rights::READ`.
- `WRITE` → `tensor_rights::WRITE`.

ANSPEC-001 §3.7 is amended atomically with ANSPEC-002's introduction; the amended §3.7 adopts the two-tier vocabulary and adds `MIGRATE` and `VIEW` as semantic-rights.

### 3.5 Attenuation patterns

Two named patterns:

**Read-only handle.** Derive with `semantic = READ` only. Recipient can `sys_tensor_read` but not `sys_tensor_write`, `sys_tensor_migrate`, or `sys_tensor_view`.

**View-permitted observer.** Derive with `semantic = READ | VIEW`. Recipient can also create derived views (which inherit rights per §3.6 below).

### 3.6 View inheritance rule

When `sys_tensor_view(base, ...)` produces a derived tensor handle, the derived handle's rights are the bitwise intersection of `base`'s rights with the variant-specific defaults for views (`READ | VIEW`). Specifically:

- A view never gains rights beyond its base.
- `WRITE` on a view requires `WRITE` on the base.
- `MIGRATE` on a view is semantically meaningless — views share storage with base; the base owns migration.

---

## 4. The Model Capability Surface

### 4.1 Semantic-rights bits

```rust
/// Model semantic-rights. Composes with the uniform meta byte from ADR-010 §Q3.
pub mod model_rights {
    /// Run inference on this model (sys_inf_run / sys_inf_submit / sys_inf_stream_open).
    pub const INVOKE:        u8 = 1 << 0;
    /// Read model metadata (sys_model_info): name, shape descriptors, op count, etc.
    pub const INSPECT:       u8 = 1 << 1;
    /// Bulk-read weight tensors. Rare and expensive; intended for export and audit, not inference.
    pub const READ_WEIGHTS:  u8 = 1 << 2;

    // Reserved per ADR-010 Principle 2.
    /// Future: produce a fine-tuned derived model (Phase 7+ if/when fine-tuning is supported).
    pub const FINE_TUNE:     u8 = 1 << 3;
    /// Future: serialize a loaded model back to .neom on disk.
    pub const EXPORT:        u8 = 1 << 4;
}
```

### 4.2 Mint sites

`sys_model_load` mints with full default rights for the loading process: `CapRights { meta: USE|DERIVE|GRANT|REVOKE, semantic: INVOKE|INSPECT|READ_WEIGHTS }`.

### 4.3 The inference-only attenuation

This is the canonical use case named in the ALTIMIT research artifact §G: "a model handle attenuated to 'inference only, no weight read' should be a kernel-enforced fact, not an application convention."

The pattern: a model owner derives a handle with `semantic = INVOKE | INSPECT` (no `READ_WEIGHTS`, no `FINE_TUNE`, no `EXPORT`) and grants it to inference clients. Clients can run inference and inspect metadata; they cannot extract weights.

### 4.4 Composition with weight-tensor caps

A subtle point that matters: when `sys_inf_run` executes, the kernel reads the model's weight tensors as part of forward-pass computation. Those reads happen against the kernel's internal handles to the weight tensors, not against the calling process's model handle. Therefore: a process holding `INVOKE` without `READ_WEIGHTS` can indirectly cause weight reads (via inference) but cannot extract weight values to userspace.

This is the kernel-enforced version of "model weights are data, never code" (Bible Article 12.5): the calling process can use the model's weights for what they were intended for (inference) but cannot extract them.

### 4.5 Sharing

Cross-process model sharing uses the universal `sys_grant` / `sys_recv_grant` per ADR-010 §Q2. The three-way intersection from ADR-010 Principle 1 applies:

```
actual = grantor_actual ∩ rights_subset ∩ max_rights
```

A grantor offering `INVOKE | INSPECT | READ_WEIGHTS` can be clipped by a recipient declaring `max_rights = INVOKE | INSPECT` — recipient gets only what they accepted.

### 4.6 Reconciliation with ANSPEC-001 §4.6

ANSPEC-001 §4.6's description — "A grants a model capability handle to B... The handle's permissions include `INVOKE`" — is consistent with `model_rights::INVOKE` in §4.1 above. No amendment needed; §4.6 is unaffected.

---

## 5. The InferenceContext Capability Surface

### 5.1 Object identity

ANSPEC-001 §5 names two surface forms: `InfTicket` (returned by `sys_inf_submit`, used by `sys_inf_wait`/`sys_inf_cancel`) and `StreamHandle` (returned by `sys_inf_stream_open`, used by `sys_inf_stream_next`/`sys_inf_stream_close`). Both are capability handles to the same underlying kernel object — an `InferenceContext`. ANSPEC-002 names this object explicitly so the capability rights have one home.

### 5.2 Semantic-rights bits

```rust
/// InferenceContext semantic-rights. Applies to InfTicket and StreamHandle alike.
pub mod inference_context_rights {
    /// Check status / await completion (sys_inf_wait).
    pub const POLL:         u8 = 1 << 0;
    /// Cancel before completion (sys_inf_cancel).
    pub const CANCEL:       u8 = 1 << 1;
    /// Advance streaming inference (sys_inf_stream_next). Stream-only; meaningless on non-streaming tickets.
    pub const STREAM_NEXT:  u8 = 1 << 2;
    /// Close a streaming inference (sys_inf_stream_close). Stream-only.
    pub const STREAM_CLOSE: u8 = 1 << 3;

    // Reserved per ADR-010 Principle 2.
    /// Future: ProbeLogits-style checkpoint/fork (research-frontier; arXiv 2604.11943).
    pub const FORK:               u8 = 1 << 4;
    /// Future: attach a LogitPort to this inference context (see §7).
    pub const ATTACH_LOGIT_PORT:  u8 = 1 << 5;
}
```

### 5.3 Mint sites

`sys_inf_submit` and `sys_inf_stream_open` mint with full default rights for the issuing process: `CapRights { meta: USE|DERIVE|GRANT|REVOKE, semantic: POLL|CANCEL|STREAM_NEXT|STREAM_CLOSE }`.

### 5.4 Service-architecture attenuations

Two named patterns:

**Polling worker.** A supervisor submits inference and grants the worker a derived ticket with `semantic = POLL` only. Worker awaits result; cannot cancel. Supervisor retains `CANCEL`.

**Streaming consumer.** Producer process opens a stream and grants the consumer a derived stream handle with `semantic = STREAM_NEXT` only (no `CANCEL`, no `STREAM_CLOSE`). Consumer reads tokens; cannot cancel or close. Producer retains termination control.

### 5.5 Process-exit semantics

Per ADR-010 §Q4 (survive-exit): if the inference-issuing process exits while a derived ticket is held by another process, the inference is canceled per ANSPEC-001 §5.2's existing semantics — the kernel completes the cancellation as part of process teardown. The held ticket remains a valid handle but invocation returns the appropriate errno (`-ESRCH` or a future AI-specific errno; sub-deliverable for the Phase 5 implementation).

---

## 6. The AccelTask Capability Surface

Phase target: kernel-internal in Phase 6, userspace surfacing deferred to Phase 7+ if a use case justifies. ANSPEC-002 reserves the slot.

### 6.1 Object identity

Per the ALTIMIT research artifact §C: an `AccelTask` is the unit of accelerator scheduling — a single op-batch dispatched to a specific accelerator with QoS hints, deadline, and capability-checked permission to access `{model, tensor, accel}`. Most `AccelTask`s are kernel-internal, created by `sys_inf_run`'s scheduler dispatch and freed at op-batch completion. They become userspace-visible only if a Phase 7+ use case surfaces (e.g., privileged scheduler integration, deadline-driven priority manipulation by a user-space arbiter).

### 6.2 Semantic-rights bits (reserved)

```rust
/// AccelTask semantic-rights. Reserved for Phase 7+ surfacing; not minted to userspace in Phase 5/6.
pub mod accel_task_rights {
    /// Check task state (queued, running, completed, canceled).
    pub const POLL:           u8 = 1 << 0;
    /// Cancel a queued or running task.
    pub const CANCEL:         u8 = 1 << 1;

    // Reserved per ADR-010 Principle 2.
    /// Future: bump scheduling priority within bounds.
    pub const PRIORITY_BOOST: u8 = 1 << 2;
    /// Future: request specific accelerator (overrides default placement).
    pub const PIN_DEVICE:     u8 = 1 << 3;
}
```

### 6.3 No userspace mint in Phase 5/6

Until a use case justifies, `AccelTask` handles do not cross the syscall boundary. The bits are reserved so that when surfacing happens, the discipline is established without an ABI break.

### 6.4 Composition with model and tensor caps

When the scheduler creates an `AccelTask`, the task's permission to access its bound model and tensors is derived (in the kernel) from the issuing process's caps on those objects. An `AccelTask` cannot acquire access beyond what the calling process has — this is the kernel-internal reflection of ADR-010 Principle 1 (Defense in depth).

---

## 7. The LogitPort Capability Surface

Phase target: Phase 7+ research-frontier. ANSPEC-002 establishes the cap surface; implementation is deferred to research work.

### 7.1 Concept

Per the ALTIMIT research artifact §G and Recommendation 9: a `LogitPort` is a capability granting per-step access to the logit distribution that an `InferenceContext` is about to use to sample a token. Holding a `LogitPort` lets a privileged agent (a safety policy, a tool-use governor, an alignment mechanism) inspect the distribution before sampling, and potentially veto or modify sampling.

This is "kernel-mediated inference governance" — the kernel exposes the hook, applications build the policy.

**Bible alignment:** Article 23.1 (no system AI assistant/chatbot persona) is preserved because `LogitPort` is a mechanism, not a persona; Article 23.3 (no embedded specific model in the kernel) is preserved because `LogitPort` observes a model loaded by userspace, not a kernel-resident one; Article 12.5 (AI models are a security boundary) composes — `LogitPort` is one of the mechanisms by which the kernel enforces the boundary. Article 6.6 (added in Bible v1.0.4, 2026-05-06) establishes the constitutional shape under which `LogitPort` sits: the kernel exposes governance hooks; userspace builds policy.

### 7.2 Semantic-rights bits

```rust
/// LogitPort semantic-rights. Phase 7+ research-frontier.
pub mod logit_port_rights {
    /// Read the logit distribution at each sampling step.
    pub const OBSERVE:    u8 = 1 << 0;
    /// Reject the sampled token; force resampling at this step.
    pub const VETO:       u8 = 1 << 1;
    /// Mutate the logit distribution before sampling (e.g., apply a penalty mask).
    pub const REWEIGHT:   u8 = 1 << 2;

    // Reserved per ADR-010 Principle 2.
    /// Future: record a full per-step trace for audit purposes.
    pub const TRACE:      u8 = 1 << 3;
}
```

### 7.3 The pure-observer pattern

A `LogitPort` with `semantic = OBSERVE` only is a read-only watcher. It can flag inferences for higher-level review but cannot affect sampling. This is the minimum-authority shape for a safety auditor.

### 7.4 Attachment

A `LogitPort` is attached to an `InferenceContext` via a syscall surface to be specified in Phase 7+. The capability prerequisite is `inference_context_rights::ATTACH_LOGIT_PORT` on the target context (reserved in §5.2). Attachment is the moment when the cap-graph between the `LogitPort` holder and the `InferenceContext` owner is established.

### 7.5 Anti-scope

ANSPEC-002 does not specify:

- The `LogitPort` syscall surface (Phase 7+).
- The mechanism by which `LogitPort` callbacks are invoked from within accelerator dispatch (research-frontier).
- The performance contract (the research artifact targets <100µs per check; ANSPEC-002 does not bind to a specific number).
- Userspace policy frameworks built on `LogitPort` (application-layer concern; out of scope).

---

## 8. Conjunctive-Safety Analysis

### 8.1 The problem

Per the ALTIMIT research artifact §G: in capability hypergraphs with conjunctive (AND) dependencies — citing arXiv 2603.15973 ("Safety is Non-Compositional") — two agents each individually safe can compose to reach a forbidden goal. The kernel needs to reason not only about who holds what capability, but about what conjunctive capability sets are reachable by message-passing composition.

This is the AIDA pattern from canon: emergence from composition.

### 8.2 The transitive-closure invariant

ANSPEC-002 specifies one concrete invariant Phase 5+ enforces, which is the simplest non-trivial check:

> **Invariant.** A grant or derive operation that produces a cap whose rights exceed the transitive closure of the parent's rights is rejected.

This is the "no authority amplification" result from arXiv 2603.00195, applied at the cap-rights level. It is what ADR-010 §Q5 (bit-subset) already enforces locally; ANSPEC-002 names the property explicitly and specifies that the property must hold across multi-hop derivation chains.

The invariant is automatic given ADR-010 §Q5's `is_subset_of`: if A → B → C is a derivation chain, then `C.rights ⊆ B.rights ⊆ A.rights` by transitive subset, hence `C.rights ⊆ A.rights`. No additional kernel mechanism is needed for Phase 5+.

### 8.3 What this does not solve

The transitive-closure invariant does not prevent conjunctive-AND emergence. Two processes each holding individually-safe caps can still cooperate to achieve a goal neither can achieve alone, by message-passing through a shared channel.

**Example:** Process A holds `model_rights::INVOKE` on a sensitive model. Process B holds `tensor_rights::READ` on the inference output tensor. Neither alone can extract sensitive information from the model. Together, A invokes inference and grants the output to B; B reads. The conjunction of (A's `INVOKE`) AND (B's `READ` on shared output) achieves what neither has individually.

The general analysis — enumerate conjunctive-AND reachability across cross-process compositions in finite time for bounded graphs — is research-frontier deferred to Phase 7+.

### 8.4 What's deferred

Named explicitly so it is not rediscovered:

- **The conjunctive-AND reachability algorithm.** Phase 7+ research.
- **Per-process capability-flow audit.** Phase 7+ research; depends on tracing infrastructure (ANSPEC-001 §11.4).
- **Formal verification of the transitive-closure invariant.** Long-horizon; depends on the OSTD-style unsafe-boundary work the ALTIMIT research artifact §F recommends.

---

## 9. Provenance and Lineage

### 9.1 The discipline

Per the ALTIMIT research artifact Recommendation 13: every `Tensor` traces to an issuing `Model`; every `Model` traces to an attested loader; every fine-tune traces to a base.

ANSPEC-002 specifies this as a derivation-log discipline, not a separate lineage system. ADR-010 §Q1 already provides a per-process derivation log mapping `CapId → Option<CapId>` (child → parent). The discipline is: AI-syscall mints record the right parent.

### 9.2 Mint-time rules

| Cap minted by | Parent recorded as |
|---|---|
| `sys_tensor_create` | `None` (root-minted) |
| `sys_inf_run` output tensor | The model handle's `cap_id` |
| `sys_inf_stream_next` token-output tensor | The stream handle's `cap_id` |
| `sys_model_load` | `None` (root-minted; loader process is implied by the cap's home process) |
| Derived tensor view (`sys_tensor_view`) | The base tensor's `cap_id` |
| Derived model handle (via `sys_derive`) | The parent model's `cap_id` |
| Granted handle (via `sys_grant`) | The grantor's source `cap_id` |
| Future fine-tuned model | The base model's `cap_id` |

### 9.3 Audit semantics

A future audit tool walks the derivation log to answer:

- "What model produced this tensor?" — walk parent links until a model handle is reached.
- "What process loaded this model?" — the process that holds the root-minted cap (`parent = None`) for the model is the loader.
- "What is the lineage of this fine-tune?" — chain of model parent links to the base.

The audit tool itself is Phase 8+ work. ANSPEC-002 specifies the discipline that makes the audit possible.

### 9.4 What this does not specify

Cryptographic attestation of model loaders (who loaded what, signed-and-verifiable). That belongs with ANSPEC-001 Open Question Q1 (model signing); ANSPEC-002 records the cap-side discipline, signing is orthogonal.

---

## 10. Cross-References and Discipline

### 10.1 Composition with ADR-010

ANSPEC-002's per-variant semantic-rights modules (`tensor_rights`, `model_rights`, `inference_context_rights`, `accel_task_rights`, `logit_port_rights`) all compose with ADR-010's uniform meta byte (`USE | DERIVE | GRANT | REVOKE`). The `CapRights` struct from ADR-010 §"ABI summary" is reused unchanged; only the semantic byte's interpretation differs by variant.

The three named principles from ADR-010 apply to AI cap operations exactly as they apply to Channel and Process:

**Principle 1 (Defense in depth).** Three-way rights intersection on grant for Tensor, Model, InferenceContext, AccelTask, LogitPort — same shape as ADR-010 specifies for Channel. A grantor's actual rights, the grantor's offered subset, and the recipient's declared maximum are all intersected at rendezvous time.

**Principle 2 (Reserved-bit pattern).** Each AI primitive's reserved bits (`PIN`, `DEDUP_HINT` for Tensor; `FINE_TUNE`, `EXPORT` for Model; `FORK`, `ATTACH_LOGIT_PORT` for InferenceContext; `PRIORITY_BOOST`, `PIN_DEVICE` for AccelTask; `TRACE` for LogitPort) are defined now; activated when their gating syscalls land.

**Principle 3 (Errors at causing actor).** Failed grants of AI caps report errnos to the actor whose action caused the failure. Recipient sees the symmetric "rendezvous failed" signal.

### 10.2 Composition with ADR-013 §Q8

Cross-process AI capability operations — e.g., revoking a model handle granted to another process — follow ADR-013 §Q8's lock-order rule:

> Lock order for cap-table operations:
>   PROCESSES (read or write) → process.handles → process.derivations

AI cap operations are not exempt; they obey the same discipline as Channel and Process cap operations under SMP.

### 10.3 Composition with ANSPEC-001 §10.5 (model-as-attack-vector)

ANSPEC-001 §10.5 enumerates four model attack vectors: parser bugs, excess memory, infinite runtime, exfiltration. ANSPEC-002's capability discipline addresses the fourth (exfiltration) via §4.4's `INVOKE`-without-`READ_WEIGHTS` pattern: a malicious model cannot extract its own weights to userspace because the calling process's capability set bounds what reaches userspace.

### 10.4 Capability composition discipline (ADR-010 implementation note 9)

The audit-trigger rule (8 layers) applies to AI syscalls. `sys_inf_run` is the canonical case: handle lookup on model + `USE` check on model + `INVOKE` check on model + per-input handle lookup + `USE` check + `READ` check + per-output handle lookup + `USE` check + `WRITE` check + scheduler dispatch. This is well over 8 layers when there are multiple inputs and outputs. Phase 5 implementation surfaces this to PM before merging, per the rule.

---

## 11. Phasing

### 11.1 Phase 5

- Tensor cap rights (§3) implemented.
- Model cap rights (§4) implemented.
- InferenceContext cap rights (§5) implemented for the synchronous inference path. Async and stream variants depend on Phase 6 async infrastructure.
- Provenance discipline (§9) implemented at all relevant mint sites.
- Transitive-closure invariant (§8.2) implicitly enforced by ADR-010 §Q5's bit-subset.
- ANSPEC-001 §3.7 amendment lands with this spec.

### 11.2 Phase 6

- `AccelTask` kernel-internal (§6); not surfaced to userspace.
- Async/stream InferenceContext cap rights complete.

### 11.3 Phase 7

- `AccelTask` userspace surfacing if a use case justifies.
- `LogitPort` (§7) implementation — research-frontier; may slip to Phase 8+.
- Conjunctive-safety analysis algorithm (§8.3 deferred work) — research-frontier.

---

## 12. Open Questions

Following ANSPEC-001 §13's pattern: deferred items, named so they're not rediscovered.

**Q1.** Should AI-cap errnos use the POSIX numbering convention from ADR-010 §"New errnos" or define an AI-specific range? Likely: AI-specific errnos under the `EAI*` family (`EAINVAL`, `EAINOMEM` already named in ANSPEC-001 §6.1). ANSPEC-002 v0.1.0 cites ADR-010's POSIX-numbering convention but flags this for resolution at Phase 5 implementation time.

**Q2.** Is there a use case for upgrading a cap's rights (the inverse of derive)? E.g., a process holding `INVOKE`-only requests `INVOKE+INSPECT` from the model owner. Currently the answer is "the owner derives a new handle and grants it"; explicit upgrade is unnecessary. ANSPEC-002 records the answer here so it's not rediscovered.

**Q3.** Should `tensor_rights::DEDUP_HINT` be a cap right at all, or remain a kernel-managed flag (`TensorFlags::DEDUP_CANDIDATE` per ANSPEC-001 §3.6)? Reserved for now; resolved when Phase 7 deduplication ships.

**Q4.** How does `LogitPort` compose with `model_rights::INVOKE`? Specifically: does observing/vetoing/reweighting require any model-side cap, or only the `LogitPort` itself? ANSPEC-002 v0.1.0 implies `LogitPort` is sufficient; Phase 7+ implementation may surface a forced exception.

**Q5.** Does the transitive-closure invariant compose correctly with `sys_grant`'s three-way intersection across multi-hop chains? The proof sketch is straightforward (intersection at each hop preserves subset), but a formal proof is research-frontier work. Named here for completeness.

---

## 13. Glossary

Following ANSPEC-001 §14's pattern. Terms novel to this spec:

- **AccelTask** — a unit of accelerator scheduling (§6). Currently kernel-internal.
- **Conjunctive-safety analysis** — the problem of enumerating composition graphs reachable from a given cap distribution (§8). Research-frontier.
- **InferenceContext** — the kernel object underlying both `InfTicket` (async/sync) and `StreamHandle` (streaming). See §5.
- **LogitPort** — a capability granting per-step access to logit distributions during inference (§7). Phase 7+ research-frontier.
- **Provenance / lineage** — the parent-cap-id chain identifying how a cap came to exist (§9).
- **Transitive-closure invariant** — the rule that derived caps cannot exceed the rights of their parent chain (§8.2). Enforced by ADR-010 §Q5's bit-subset.

---

## 14. References

- NeoPlutarch Bible v1.0.4, Articles 6 (including 6.6, inference-governance hooks), 12, 20–23.
- ADR-010 — Capability System Foundation.
- ADR-013 — Phase 2 SMP Scheduler & IPC Retrofit, §Q8 (cap-table SMP composition).
- ANSPEC-001 — AI Subsystem Design Specification.
- ALTIMIT research artifact (`docs/research/ALTIMIT-real-world-implementation.md`), §G.
- arXiv 2603.00195 — "Formal Analysis and Supply Chain Security for Agentic AI Skills." No-authority-amplification result.
- arXiv 2603.15973 — "Safety is Non-Compositional." Conjunctive-safety result.
- arXiv 2604.11943 — ProbeLogits. Precedent for kernel-level inference governance primitives.

---

**Document end.**

*Amendments to this spec follow the standard ADR + bible-amendment process. Material changes to per-variant rights vocabularies require a major-version bump.*
