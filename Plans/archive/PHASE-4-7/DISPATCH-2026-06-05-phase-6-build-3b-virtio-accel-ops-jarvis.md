<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-3b — virtio-accel real ops + equivalence + migration → Jarvis

**Date:** 2026-06-05
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — e.g. `codex/phase-6-build-3b-virtio-accel-ops-jarvis`)
**Status:** DISPATCHED — ready for BUILD
**Design contract:** **ADR-031** (protocol + the §7 **Add** byte oracle — the *data* half lands here), ADR-029 (soft-float determinism), ADR-030 (Phase 6 substrate).
**Builds on:** BUILD-3a (`305875e`) — the virtio-accel *transport* is proven (virtqueue/DMA/IRQ round-trip, `irq_completion=ok`). Current `main` tip: see origin/main.

---

## 0. What 3b is (and isn't)

3a proved the pipe works (an `Identity` echo over real DMA + IRQ). **3b makes the pipe carry real compute:** the daemon runs the full op set, `virtio_accel` becomes a full `AccelDriver`, and we prove a `.neom` forward pass over the wire is **byte-identical** to `accel_cpu`, plus real cross-device migration. **The transport (3a) is frozen** — no virtqueue/vhost-user/IRQ changes; 3b only extends what flows through it.

## 1. The load-bearing decision — read first

**D1 — The daemon MUST compute with the *same op-kernel code* as `accel_cpu`, not a reimplementation.**

`accel_cpu` runs the ops in **soft-float** (ADR-029: no hardware FP/XMM in the kernel) for *deterministic* results. The daemon runs on the host, where:
- transcendental ops (`Gelu`/`Softmax`/`LayerNorm`/`RmsNorm`) computed with host `libm` would differ from the kernel's soft-float in the low bits, **and**
- even "exact" ops diverge if the **summation order** differs (host `MatMul` accumulating in a different order than `accel_cpu` gives different f32 bits).

So a from-scratch daemon math impl will produce **1-ULP mismatches** that fail equivalence in confusing ways. **Avoid the whole class of bug by sharing code:** the daemon must run the *same* op kernels that `tools/ai-ops-tests` already exercises on the host. If those kernels need extracting into a shared (`no_std`-compatible) crate consumed by the kernel, `ai-ops-tests`, and the daemon, do that — it's correct factoring. **If that extraction turns out to be a large refactor of `kernel/src/ai/ops`, STOP and flag PM before proceeding** (we don't balloon 3b into an ops-rearchitecture). The invariant is non-negotiable: **`virtio_accel` output is byte-identical to `accel_cpu`, not a tolerance match.**

## 2. Implementation Success Criteria

- **ISC-1 — Daemon: full op set, shared math (D1).** `tools/virtio-accel-host` executes the full op set (`Identity/Add/Mul/Matmul/Relu/Gelu/Softmax/LayerNorm/RmsNorm/Embedding/Reshape/Transpose`, F32/I32) by calling the shared op kernels — byte-identical to `accel_cpu`. Unsupported op/dtype → `VirtioAccelStatus` error code (ADR-031 §D6).
- **ISC-2 — Kernel driver: full dispatch.** `virtio_accel::dispatch` handles all `SUPPORTED_OPS` (extends 3a's Identity-only), building the chain with the correct input/output descriptors per op arity (incl. the 1-or-3-input LayerNorm / 1-or-2-input RmsNorm / 2-input Embedding shapes already handled in `ai::exec`).
- **ISC-3 — §7 Add data oracle (REQUIRED, both sides).** The daemon computes `Add [1,2]+[3,4] → [4,6]`; the **output buffer** bytes (`00 00 80 40 00 00 C0 40`) and `VirtioAccelStatus` match ADR-031 §7 exactly, asserted **independently in both the kernel and the daemon** (3a did the framing; this is the *data*).
- **ISC-4 — Forward-pass equivalence over the wire.** A scenario runs `valid_mlp` through `exec::run_with_driver(&model, &inputs, virtio_accel)` and asserts output **== `accel_cpu` == the hand oracle** (`virtio_accel_forward_equiv`, SMP=4). If shared-math covers the transcendentals cleanly (D1), also run a transformer model; if not, scope to the MLP path and note transcendental-over-virtio as a flagged follow-up.
- **ISC-5 — Cross-device migration over real DMA.** `tensor_migrate` Cpu→`Accel(2)`→Cpu round-trips through the device's `DmaBuffer` storage; verify the copy is **non-identity** + byte-exact round-trip + that a forward pass consuming a migrated tensor matches the oracle (`virtio_accel_migrate_roundtrip`, SMP=4).
- **ISC-6 — Capability negotiation reflects reality.** `VirtioAccelConfig.supported_ops/supported_dtypes` advertise the real set; the driver's `AccelDescriptor` is populated from it; an op/dtype the device doesn't advertise is rejected cleanly (no panic, no hang).
- **ISC-7 — Errors / lifetime.** Daemon op failure → status error → clean `AccelError` with **no hang** (bounded wait, per 3a); `DmaBuffer`s freed; no leak across a dispatch loop.
- **ISC-8 — No regressions.** `accel_cpu_forward_migrate`, the three `accel_sim_*`, the three `infer_*`, and 3a's `virtio_accel_identity_roundtrip` all stay green; the default build (`virtio-accel` OFF) is unaffected.
- **ISC-9 — Standards/gate.** No new **kernel** dependency; SAFETY on any new `unsafe`; SPDX on new files; host deps (if any added to the daemon) justified. Run **`cargo fmt --all --check`** and confirm before close.

## 3. Close gate

- `git fetch origin && git rebase origin/main`; re-run the gate on the rebased tree.
- Green at `NEO_SMP=4`: `virtio_accel_forward_equiv` (== `accel_cpu` == oracle), `virtio_accel_migrate_roundtrip`, the §7 **Add** data oracle on **both** sides, and the full regression batch (ISC-8).
- Host-checks green **including the daemon and any shared op crate** (fmt / `test --workspace --exclude neoplutarch-kernel` / `clippy … -D warnings`).
- Single squashed commit; push to your **feature branch, not `main`** — PM merges via `pm-merge-check` + `--ff-only`. Close report: ISC pass/fail, the equivalence evidence (the byte match vs `accel_cpu`), **how D1 was satisfied** (shared-crate mechanism + which ops are byte-exact, and whether transcendentals are in or deferred), and any op-kernel factoring you did.

## 4. Scope guard

Real ops + equivalence + migration over the **existing 3a transport** only. **No transport changes** (virtqueue/vhost-user/IRQ/daemon-protocol are frozen). No multi-queue, no preemption, no dtypes beyond F32/I32, no inference scheduler (Phase 7), no `.neom`/op-math *behavior* changes (you may *relocate* op-kernel code for sharing per D1, but the math output must not change — the `infer_*`/`accel_*` oracles prove that). Landing 3b **completes BUILD-3**; BUILD-4 (real GPU) is the next and final Phase 6 build.
