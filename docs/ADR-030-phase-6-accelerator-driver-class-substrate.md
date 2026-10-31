<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-030: Phase 6 Accelerator Driver Class — Design Substrate

| | |
|---|---|
| **Status** | Accepted — Phase 6 shipped the accel driver class; Phase 6 closed 2026-06-07 (`Plans/CLOSE-REPORT-2026-06-07-phase-6.md`). *(Status corrected 2026-06-17: was "Proposed", stale post-close.)* |
| **Date** | 2026-06-04 |
| **Authority** | PM/Cerebro + PL |
| **Related** | Bible Articles 2.2/2.3/2.5 (kernel-arbitrated accelerators), 6.4 (accel driver class), 6.5 (CPU fallback), 7.5 (tensor memory class), 9.1 (uniform driver traits), 13 (phase ledger); **ANSPEC-001 §9 (the authoritative accel-driver spec)**; ADR-027 (Phase 5 substrate); ADR-023 (Linux-personality DRM-expressibility forward-marker) |

## Context

Phase 5 delivered the AI subsystem foundation — tensor (L0), model (L1), and a **correct-and-slow CPU inference backend** (L2): `SYS_INFER_RUN` walks a `.neom` op graph and runs each op via direct soft-float dispatch in `kernel/src/ai/exec.rs`. There is **no `AccelDriver` abstraction** today; the executor calls op kernels directly.

Phase 6 opens the **`accel` driver class** (Article 6.4): kernel-managed AI accelerators that execute tensor ops via a uniform trait, with device residency and explicit tensor migration, so inference can dispatch to hardware instead of the CPU. Article 6.5 keeps the CPU path as the always-present fallback ("we do not ship a system that requires a GPU to boot").

**The spec-vs-reality gap this ADR reconciles:** ANSPEC-001 §9.2 assigned the CPU reference driver (`accel_cpu`) to **Phase 5**. Phase 5's actual slicing built the executor with direct dispatch and never introduced the `AccelDriver` trait. So `accel_cpu` lands in **Phase 6 BUILD-1** instead — and a small ANSPEC-001 §9.2 amendment records that the CPU accel driver is Phase 6, not Phase 5.

## Decisions

### D1 — Build to ANSPEC-001 §9.1, not a new trait
The `AccelDriver` trait (`descriptor`, `supported_ops/dtypes`, `alloc/free`, `copy_to/from_device`, `dispatch/poll/wait`, `preemptible/preempt`) is already specified in ANSPEC-001 §9.1 and is normative. Phase 6 implements it under `kernel/src/drivers/accel/`; it does not redesign it. Any divergence forced at BUILD time is an ANSPEC-001 amendment, not a silent drift.

### D2 — CPU-SIMD first (`accel_cpu`), then virtio-gpu-compute, then one real GPU
The build order is the disciplined Phase-5 pattern (prove the abstraction against a testable driver before exotic hardware):
- **BUILD-1 — the trait + `accel_cpu`.** Introduce `AccelDriver`, `DeviceId`/`DeviceStorage` residency, `tensor_migrate`, capability registration, and a CPU reference driver that wraps the *existing* Phase-5 op kernels behind the trait. The executor dispatches **through** the trait. Fully QEMU- and bare-metal-testable; zero new hardware. Delivers what §9.2 expected of `accel_cpu`.
- **BUILD-2 — `virtio-gpu-compute`.** A paravirtual compute accelerator (§9.2's "Phase 6 first deliverable") that validates the **multi-device** path — a second `DeviceId::Accel`, real `copy_to/from_device`, cross-device `tensor_migrate` — in CI under QEMU/KVM.
- **BUILD-3+ — one real GPU.** Intel/AMD preferred (open docs); NVIDIA intentionally lower priority (§9.2).

### D3 — `DeviceId` residency and explicit migration (ANSPEC-001 §3.4, §6.2.4)
A tensor lives on exactly one device: `DeviceId::Cpu` (host RAM, normal memory model) or `DeviceId::Accel(n)` (driver-owned device memory). The kernel **never silently moves tensors**; migration is the explicit `tensor_migrate()` operation. For `accel_cpu`, device memory *is* host RAM, so migration is a trivial/no-op copy — which is exactly why it's the right first driver to shake out the residency + migration plumbing before a real device makes the copies expensive and asynchronous.

### D4 — Single-device dispatch only; placement policy is Phase 7
Phase 6 dispatches a model's ops to **one** chosen accelerator (or the CPU). The *inference scheduler* — choosing per-op placement across multiple accelerators by load/policy/deadline (Article 6.4 second sentence, 8.2) — is **Phase 7**. Phase 6 proves a driver can run a forward pass; Phase 7 makes the kernel choose among them.

### D5 — Drivers own their device memory; no kernel-heap tensor storage (ANSPEC-001 §9.3)
Per §9.3 and Article 7.5, an accel driver owns its device-memory allocator and serves tensor storage from it — not the general kernel heap or slab. Drivers run in kernel mode, must not block indefinitely (split long ops into pollable units), and must not hold long-lived locks across dispatch (per-queue spinlock + lock-free completion polling recommended).

## Anti-scope (Phase 6 must NOT do)

- **Graphics / DRM / KMS rendering** — separate class, Phase 8+ (Article 9.7). The `accel` class is AI compute, not graphics.
- **The accel-aware inference scheduler / multi-tenant placement** — Phase 7.
- **NVIDIA-first**, exotic NPU/TPU, or model-format changes.

## Forward-markers honored

- **ADR-023 DRM-expressibility:** accel drivers must remain *expressible* as Linux-DRM-compatible char devices for the future personality (interface constraint only). When a real GPU driver ships (BUILD-3+), expect an ANSPEC-001 §9 amendment for the DRM-compat surface; do not structure internals around DRM.

## Consequences

- BUILD-1 is a pure-refactor-plus-abstraction with no behavior change to inference output (the byte oracles from Phase 5 must still pass through the trait) — a clean, testable opening that de-risks the trait before hardware.
- The byte-oracle discipline (CLAUDE.md) applies to any new device-memory layout (`DeviceStorage`) and to `copy_to/from_device`.
- ANSPEC-001 §9.2 needs a one-line amendment (CPU accel driver = Phase 6 BUILD-1).
