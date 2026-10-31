<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-027: Phase 5 AI Subsystem Foundation — Design Substrate

| | |
|---|---|
| **Status** | ✅ Accepted (PL sign-off 2026-05-31) |
| **Date** | 2026-05-31 |
| **Authority** | PM + PL |
| **Supersedes** | None |
| **Superseded by** | - |
| **Related** | Bible Articles 2.1–2.5 (AI-aware kernel), 6.1–6.6 (AI subsystem), 7.5 (tensor memory class), 8.2 (inference scheduling class), 19 (what changes fast/slow), 20 (Tensor Object), 21 (Model Object), 12.1 (capabilities); ADR-021 (Phase 3 design substrate, precedent); ANSPEC-002 (LogitPort / cascade revocation); `docs/phases/PHASE-4.md` (foundation complete); Bible v1.10.0 syscall-range amendment |

---

## Context

Phases 0–4 are complete: SMP, capabilities, allocators, NeoFS + real disk I/O,
and a full network stack with TLS over the native ABI. That was all foundation.
Phase 5 begins the AI-native phases that, per Article 13, *justify the project's
existence*: **tensor primitive, model object, CPU inference backend**.

`kernel/src/ai/` does not exist yet — a clean slate. The Bible specifies the
design in unusual normative detail (Articles 6, 20, 21), so Phase 5 is less about
inventing the architecture than about *faithfully realizing what is already
ratified* while resisting scope creep into Phases 6–7.

This ADR records the design substrate for the phase and the BUILD-1 design; the
phase/build plan and Implementation Success Criteria live in
`Plans/PM-DECISION-2026-05-31-phase-5-opening.md`.

## Decisions

### D1 — Layering (Article 6.1)

Three layers, built in sequence, not at once:

- **Layer 0 — Tensor primitive** (Article 20). `BUILD-1`. The kernel-managed
  tensor object. Everything else builds on it.
- **Layer 1 — Model object** (Article 21). `BUILD-2+`. Graph of tensor ops +
  immutable weights, loaded from `.neom`.
- **Layer 2 — Inference context** (Article 6.1, 8.2). `BUILD-N`. CPU inference
  backend (Article 6.5): correct and slow, no GPU required to run.

### D2 — The Tensor Object (Article 20)

A tensor is `(shape, dtype, device, storage)`:

- **shape** — fixed-rank descriptor, rank ≤ 8 (Article 20.1).
- **dtype** — enumerated, initial set: `F32, F16, BF16, I32, I8, U8, Bool`
  (Article 20.1), extensible by accretion (Article 19.2).
- **device** — `Cpu` for Phase 5. `Accel(AccelId)` is a **reserved enum variant
  with no driver code** until Phase 6 (Article 6.4, anti-scope below). No
  speculative accelerator machinery ships in Phase 5 (Article 15.7).
- **storage** — refcounted physical-memory region (Article 20.2). Sharing a
  tensor shares storage; the kernel tracks per-process handles.

Tensors are **immutable by default** (Article 20.3). Mutable tensors exist for
activations and carry an explicit marker; weight tensors are immutable, which
makes dedup and safe sharing trivial.

### D3 — Tensor memory is its own allocation class (Article 7.5)

Tensor storage is **not** subject to the slab allocator. It is a distinct
allocation class managed by the AI subsystem, backed by the page/buddy allocator,
with awareness of (eventual) accelerator residency. A large weight buffer is not a
normal kernel allocation. BUILD-1 implements the CPU-resident case only.

### D4 — Tensors are capabilities (Articles 12.1, 20.2)

A handle to a tensor is a capability (`TensorCap`) integrated with the Phase 2
capability table. Holding the handle is permission; there is no ambient authority.
Per-process handle tracking; refcount on the underlying storage. This is the same
capability discipline every other kernel object already follows.

### D5 — Syscall range (Bible v1.10.0)

The AI syscall family occupies **`0x100..0x17F`** (Article 6.1, amended
2026-05-31). The originally-reserved `0x100..0x1FF` overlapped the network ABI
that shipped at `0x180..0x18E` in Phase 4; rather than churn a just-ratified ABI,
the AI reservation was narrowed to the free lower half. BUILD-1 allocates the
tensor sub-family at the bottom of the range (concrete numbers fixed in the
opening doc's PLAN).

### D6 — Native format only (Article 6.2, 6.3)

The kernel reads exactly one model format, `.neom`. ONNX/GGUF/SafeTensors
importers are **userspace tools** (anti-scope for the kernel). The kernel knows
ops and tensors; NN-architecture semantics (transformer, CNN, diffusion) are the
model file's business (Article 6.3) and never appear in kernel code.

### D7 — Governance is hooks, not policy (Article 6.6)

Phase 5 ships **no** governance policy, safety filter, or content classifier. The
LogitPort *mechanism* (ANSPEC-002 §7) is a later-layer (inference) concern; it is
out of scope until Layer 2. Article 6.6 / 23.1: the kernel exposes the seam;
userspace builds policy on it.

### D8 — Deferred THINK item: cascade revocation on unmount (ANSPEC-002 §16.3)

ANSPEC-002 §16.3 deferred the mount-interaction shape of cascade revocation to
"Phase 5 THINK" (`Plans/archive/PHASE-3-BUILD-5/PM-NOTE-2026-05-20-anspec-002-q6-deferral-to-phase-5-think.md`).
Disposition now that BUILD-4B mount semantics exist:

- **Rule:** when a tensor's backing NeoFS inode is destroyed, or the inode
  generation advances so the underlying `FileCap` is stale, every derived
  `TensorCap` returns `ESTALE` on next use (cascade). Unmount **invalidates**
  outstanding caps (cascade-revocation targets) rather than blocking on `EBUSY`.
- **When it bites:** only once tensors/models are loaded *from disk*. **BUILD-1
  tensors are memory-only** (created from userspace-supplied bytes), so cascade-
  on-unmount has no surface in BUILD-1. Implementation lands in the build that
  first loads a tensor/model from NeoFS (Layer 1, model load). Recorded here so it
  is not re-lost.

## Anti-scope (what Phase 5 must NOT do)

- **No accelerator drivers.** `Device::Accel` is a reserved enum variant only.
  Real GPU/NPU/virtio-gpu compute is Phase 6 (Article 6.4, ANSPEC-001).
- **No inference scheduler / multi-tenant serving.** That is Phase 7.
- **No `.neom` importers in the kernel.** Userspace converts (Article 6.2).
- **No NN-architecture logic in the kernel** (Article 6.3).
- **No governance policy** — hooks only, and not until Layer 2 (Article 6.6).
- **No aarch64 bringup** — the foundation is portable by design, but ARM boot
  validation is a separate effort (Article 9 note, "Phase 5 or later").

## Consequences

- BUILD-1 delivers a usable, capability-gated, refcounted tensor primitive that
  the model object and CPU backend will sit on, with the syscall range coherent
  before the first AI syscall number is assigned.
- The reserved-but-empty `Device::Accel` variant keeps the type honest without
  shipping driver bloat, satisfying Article 15.7.
- The cascade-revocation rule is settled in design but deferred in code to the
  build where it first matters, avoiding speculative machinery.
