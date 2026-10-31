<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PM-DECISION: Phase 6 Opening — AI Accelerator Driver Class

**Date:** 2026-06-04
**From:** PM/Cerebro (PL sign-off this session)
**Status:** OPEN — Phase 6 active
**References:** bible Article 13 ledger (Phase 6), Articles 2.2/2.3, 6.4, 6.5, 7.5, 9.1; ANSPEC-001 §9 (accel driver interface); `docs/ADR-030-phase-6-accelerator-driver-class-substrate.md`; Phase 5 close (`Plans/CLOSE-REPORT-2026-06-04-phase-5.md`)

---

## 1. Phase goal (Article 13)

> Phase 6 — AI accelerators: a kernel-managed `accel` driver class that executes tensor ops on hardware, with device residency and explicit tensor migration, so inference dispatches to an accelerator instead of the CPU. The CPU backend remains the always-present fallback (Article 6.5).

Milestone: **a `.neom` forward pass runs through the `AccelDriver` trait on a non-CPU device** (the `virtio-gpu-compute` accelerator), CI-verified, with results bit-matching the CPU oracle.

## 2. Scope / anti-scope

**In scope:** the ANSPEC-001 §9.1 `AccelDriver` trait; `DeviceId::Cpu/Accel(n)` residency; `tensor_migrate()`; capability registration; the `accel_cpu` reference driver (CPU path behind the trait); the `virtio-gpu-compute` paravirtual accelerator; first-pass driver memory ownership (§9.3).

**Anti-scope** (per ADR-030): no graphics/DRM/KMS rendering (Phase 8+); no accel-aware inference *scheduler* / multi-tenant placement (Phase 7 — Phase 6 is single-device dispatch); no NVIDIA-first, no exotic NPU/TPU; no `.neom` format changes. The Linux-DRM-expressibility forward-marker (ADR-023) is honored at the interface, implemented when a real GPU driver ships.

## 3. Build sequence (sliced, not bundled)

| Build | Deliverable |
|-------|-------------|
| **BUILD-1** | **The `accel` driver class + `accel_cpu`** (this opening's PLAN below). The trait, `DeviceId`/`DeviceStorage`, `tensor_migrate`, capability registration, and the CPU reference driver; executor dispatches through the trait. |
| BUILD-2 | **`virtio-gpu-compute`** — paravirtual compute accelerator; validates the multi-device path + cross-device migration in CI (ANSPEC-001 §9.2 "Phase 6 first deliverable"). |
| BUILD-3+ | **One real GPU** (Intel/AMD; NVIDIA lower priority). DRM-compat surface + ANSPEC-001 §9 amendment land here. |

Each build OBSERVE → THINK → PLAN → BUILD/VERIFY, `/simplify` before BUILD, QEMU + (where applicable) bare-metal verification per Article 13.

## 4. BUILD-1 PLAN — The accel driver class + `accel_cpu`

**Goal:** introduce the `AccelDriver` abstraction and retrofit the Phase-5 CPU inference path as the `accel_cpu` reference driver, with the executor dispatching **through** the trait — **no change to inference output** (Phase-5 byte oracles must still pass). QEMU- and bare-metal-testable; zero new hardware.

### Implementation Success Criteria

- **ISC-1 — Trait + module.** `kernel/src/drivers/accel/mod.rs` defines `AccelDriver` per ANSPEC-001 §9.1 (descriptor, supported_ops/dtypes, alloc/free, copy_to/from_device, dispatch/poll/wait, preemptible/preempt). SPDX + `//!` docs. Any forced divergence from §9.1 is flagged for an ANSPEC amendment, not silently changed.
- **ISC-2 — Device residency.** `DeviceId { Cpu, Accel(u32) }` and `DeviceStorage` (driver-owned handle). Tensors carry their `DeviceId` (extends the Phase-5 `Dtype/Device` already reserved in `tensor.rs`). Exactly-one-device invariant documented at the allocation site.
- **ISC-3 — `accel_cpu` driver.** A concrete `AccelDriver` wrapping the existing `ai::ops` kernels. `alloc/free` over host RAM (the buddy/tensor class, Article 7.5 — driver owns it, not the slab); `copy_to/from_device` are host memcpys; `dispatch` runs the op synchronously; `poll`/`wait` trivially complete. `preemptible() == false` for now.
- **ISC-4 — Executor dispatches through the trait.** `ai::exec` runs each op via the selected `AccelDriver` (default `accel_cpu`) instead of calling `ops::*` directly. **The forward-pass output is byte-identical** to Phase 5 — `infer_mlp_forward` / `infer_transformer_forward` / `infer_embedding_forward` oracles unchanged.
- **ISC-5 — `tensor_migrate` plumbing.** `tensor_migrate(handle, DeviceId)` exists and is exercised (Cpu→Accel(cpu)→Cpu is a no-op/trivial copy for `accel_cpu`, but the *path* — residency change, driver alloc, copy, free — is real and tested). This shakes out the migration machinery before a real device makes copies expensive/async.
- **ISC-6 — Capability registration.** A driver registry: `accel_cpu` registers at init with its `AccelDescriptor` (supported ops/dtypes, "memory" = host RAM, dispatch latency). Queryable (foundation for Phase-7 placement, but no placement policy here).
- **ISC-7 — Errno + lifetime.** Unsupported op/dtype on a device → clean error (no panic); device storage freed at refcount zero; no leak on process exit. Reuse `errno.rs`.
- **ISC-8 — QEMU scenario + oracle.** A scenario that runs a forward pass **through `accel_cpu`** and asserts the same output bytes as the direct-dispatch oracle (proves the trait is transparent). Plus a `tensor_migrate` round-trip test.
- **ISC-9 — Standards/lean/gate.** rustfmt; SAFETY on new unsafe; no XMM regressions; host-checks green; `--bare-metal-mode` parity unaffected.

### Close gate
- Rebase onto current `main`; the **Phase-5 inference oracles pass unchanged through the trait** (the transparency proof); host-checks green; QEMU scenario green at `NEO_SMP=4`; byte-oracle for any new `DeviceStorage` layout; push to feature branch, PM merges via `pm-merge-check` + `--ff-only`; close report.

## 5. Governance / housekeeping at open

- **Bible Article 13:** add the Phase 6 ledger line (Active, opened 2026-06-04).
- **ANSPEC-001 §9.2 amendment** (small): the CPU accel driver (`accel_cpu`) lands in **Phase 6 BUILD-1**, not Phase 5 (Phase 5 built the executor with direct dispatch). To be made when BUILD-1 opens.

## 6. Next phase (recorded, not opened)
After Phase 6, **Phase 7** is the inference scheduler / multi-tenant serving — the accel-aware per-op placement Phase 6 deliberately defers. The capability-registration foundation (ISC-6) is laid here for it.
