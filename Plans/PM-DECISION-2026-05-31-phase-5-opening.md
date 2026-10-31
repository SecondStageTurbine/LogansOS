<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PM-DECISION: Phase 5 Opening — AI Subsystem Foundation

**Date:** 2026-05-31
**From:** PM/Cerebro (PL sign-off this session)
**Status:** OPEN — Phase 5 active
**References:**
- docs/bible.md Article 13 ledger (Phase 5), Articles 6, 7.5, 8.2, 20, 21; v1.10.0
- docs/ADR-027-phase-5-ai-foundation-design-substrate.md (design substrate)
- docs/phases/PHASE-4.md (foundation complete; Phase 4 shipped)

---

## 1. Phase goal (Article 13)

> Phase 5 — AI subsystem foundation: tensor primitive, model object, CPU
> inference backend.

The milestone for the phase: **a `.neom` model loads as a kernel object and runs
a forward pass on the CPU inference backend, end to end, through the native AI
syscall family — no accelerator, no GPU required.** (Article 6.5: "correct and
slow; this is acceptable.")

## 2. Scope / anti-scope

**In scope (this phase):** the three Article 6.1 layers — tensor primitive
(Layer 0), model object + `.neom` load (Layer 1), CPU inference backend
(Layer 2) — all capability-gated, all on the native ABI in `0x100..0x17F`.

**Anti-scope** (per ADR-027): no accelerator drivers (Phase 6), no inference
scheduler / multi-tenant serving (Phase 7), no `.neom` importers in the kernel
(userspace converts), no NN-architecture logic in the kernel, no governance
policy (hooks only, later), no aarch64 bringup.

## 3. Build sequence (the Algorithm — sliced, not bundled)

| Build | Layer | Deliverable |
|-------|-------|-------------|
| **BUILD-1** | Layer 0 | **Tensor primitive** (this opening's PLAN below). |
| BUILD-2+ | Layer 1 | Model object: `.neom` load path, op graph, immutable weights, cascade-revocation-on-unmount (ADR-027 §D8). |
| BUILD-N | Layer 2 | CPU inference backend: op kernels (matmul, softmax, layernorm, elementwise…), inference context, end-to-end forward pass. |

Each build OBSERVE → THINK → PLAN → BUILD/VERIFY, `/simplify` gate before BUILD,
QEMU + (where applicable) bare-metal verification per Article 13.

## 4. BUILD-1 PLAN — Tensor primitive (Article 20)

**Goal:** a capability-gated, refcounted, immutable-by-default tensor object in
`kernel/src/ai/`, created from userspace-supplied bytes, with a tensor-class
allocation backing and a native syscall surface — QEMU-verified.

### Implementation Success Criteria

- **ISC-1 — Module skeleton.** `kernel/src/ai/mod.rs` + `kernel/src/ai/tensor.rs`
  exist, wired into the kernel module tree, SPDX headers present, `//!` module
  docs (Article 15.6).
- **ISC-2 — Tensor type.** `shape` (fixed-rank descriptor, rank ≤ 8), `Dtype`
  enum `{F32,F16,BF16,I32,I8,U8,Bool}` with a `size_bytes()`, `Device`
  `{Cpu, Accel(AccelId)}` (Accel reserved, no driver code), refcounted `Storage`.
  Immutable by default; explicit `mutable` marker (ADR-027 §D2).
- **ISC-3 — Tensor memory class.** Storage is allocated from the page/buddy path,
  **not** the slab allocator; the distinct-allocation-class invariant
  (Article 7.5, ADR-027 §D3) is documented at the allocation site.
- **ISC-4 — Capability integration.** `TensorCap` handle in the Phase 2
  capability table; holding = permission, no ambient authority (Articles 12.1,
  20.2). Per-process handle tracking; sharing shares storage and bumps refcount.
- **ISC-5 — Syscall surface (`0x100..0x17F`).** Tensor sub-family registered in
  the dispatcher and the AI range documented in `kernel/src/arch/x86_64/syscall.rs`:
  - `0x100 SYS_TENSOR_CREATE` (shape, dtype, mutable flag, src bytes → TensorCap)
  - `0x101 SYS_TENSOR_QUERY` (TensorCap → shape, dtype, device, refcount)
  - `0x102 SYS_TENSOR_SHARE` (TensorCap → new TensorCap, refcount++)
  - `0x103 SYS_TENSOR_DROP`  (TensorCap → refcount--; free storage at zero)
  (Exact arg marshalling settled at BUILD time; set may shrink under `/simplify`.)
- **ISC-6 — Errno discipline.** Invalid rank/shape/dtype → `EINVAL`; bad handle →
  `EBADF`; out-of-memory → `ENOMEM`; mutate-immutable → `EPERM`. Reuse
  `kernel/src/errno.rs`.
- **ISC-7 — Lifetime correctness.** `DROP` decrements; storage freed exactly at
  refcount zero; immutable tensors cannot be mutated (guarded). No leak on
  process exit (handles reaped).
- **ISC-8 — QEMU integration test.** Test-harness scenario: create → query
  (shape/dtype match) → share (refcount up) → drop twice (storage freed at zero) →
  attempt-mutate-immutable returns `EPERM`; sentinel to serial; debug-exit.
- **ISC-9 — Host unit tests.** Pure logic on host (`cargo test`): shape/rank
  validation, `Dtype::size_bytes`, byte-length-vs-shape consistency, refcount
  arithmetic.
- **ISC-10 — Standards gate.** rustfmt clean; clippy warnings-as-errors clean;
  every `unsafe` has a `// SAFETY:` comment; no `unwrap()`/`expect()` in
  post-init paths (Article 15.2–15.5); SPDX headers on new files.
- **ISC-11 — Lean gate.** `/simplify` before BUILD. `Device::Accel` is a reserved
  variant only — **no accelerator code, no `.neom` parsing, no op kernels** ship
  in BUILD-1 (Article 15.7, ADR-027 anti-scope).

### Anti-criteria (must NOT trigger)

- Any accelerator driver code, BAR mapping, or virtio-gpu touch.
- Any model-object / `.neom` parsing or op-graph code.
- Any inference execution or op kernel.
- Any governance/LogitPort surface.
- Any NN-architecture-specific logic.

## 5. Verification

QEMU integration test (ISC-8) + host unit tests (ISC-9) are the BUILD-1 gate.
Bare-metal tensor validation rides the same hardware lane the rest of the kernel
uses; per Article 13 it is a phase-close concern, not a per-build blocker for a
memory-only primitive.
