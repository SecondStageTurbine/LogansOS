<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-1 — Accel driver class + `accel_cpu` → Jarvis

**Date:** 2026-06-04
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — e.g. `codex/phase-6-build-1-accel-driver-class-jarvis`)
**Status:** DISPATCHED — ready for BUILD
**Design contract:** **ANSPEC-001 §9.1** (the `AccelDriver` trait — normative), `docs/ADR-030-phase-6-accelerator-driver-class-substrate.md` (D1–D5), `Plans/PM-DECISION-2026-06-04-phase-6-opening.md` (BUILD-1 PLAN ISC-1…9).
**Builds on:** Phase 5 (closed). Current `main` tip: `af13678`.

---

## 1. Objective

Introduce the `accel` driver class and retrofit the Phase-5 CPU inference path as the `accel_cpu` reference driver, with `ai::exec` dispatching **through** the `AccelDriver` trait instead of calling `ops::*` directly. **The forward-pass output must be byte-identical to Phase 5** — this build is an abstraction + refactor, not a behavior change. QEMU- and bare-metal-testable; zero new hardware.

The win condition is the **transparency proof**: the existing `infer_mlp_forward` / `infer_transformer_forward` / `infer_embedding_forward` oracles pass **unchanged** once routed through `accel_cpu`.

## 2. Decisions locked (do not re-litigate)

- **Build to ANSPEC-001 §9.1, don't redesign it.** Implement the trait as specified (`descriptor`, `supported_ops/dtypes`, `alloc/free`, `copy_to/from_device`, `dispatch/poll/wait`, `preemptible/preempt`). If a signature genuinely can't work, flag an ANSPEC amendment — don't silently drift.
- **`accel_cpu` is synchronous and simplest-correct.** Device memory *is* host RAM; `copy_*` are memcpys; `dispatch` runs the op synchronously and returns an already-complete token; `poll`/`wait` trivially succeed; `preemptible() == false`. **Do NOT build speculative async/preemption/queue machinery** (Article 15.7 lean rule) — implement the full trait *surface* with the trivial *body*. BUILD-2 (virtio-gpu-compute) is where real async/copies arrive.
- **Single-device dispatch only.** The whole forward pass runs on one chosen device (default `accel_cpu`). Per-op placement across devices is **Phase 7** — not here.
- **No inference-output change.** If any oracle byte shifts, the refactor is wrong.

## 3. OBSERVE map (verified by Cerebro 2026-06-04)

- **Residency already scaffolded:** `kernel/src/ai/tensor_logic.rs:71` already has `enum Device { Cpu, Accel(u32) }` with ABI encoding (ADR-027 D2 reserved it). BUILD-1 makes it *functional* (carry it on tensors, honor it in dispatch/migration) — you are not inventing it.
- **Executor to reroute:** `kernel/src/ai/exec.rs` — `pub fn run(model, inputs) -> Vec<Arc<Tensor>>` (`:36`) calls `execute_node` (`:89`), whose `match` dispatches each op via `allocate_result(shape, dtype, |out| ops::FOO(...))` (`:117`+). Replace the direct `ops::*` calls with a call **through** the selected `AccelDriver` (which, for `accel_cpu`, calls the same `ops::*` internally). The per-node allocate→compute→freeze→store flow stays; only the *compute* step goes through the trait.
- **Op kernels to wrap:** `kernel/src/ai/ops/mod.rs` (matmul/add/mul/relu/gelu/softmax/layer_norm/rms_norm/embedding/identity/reshape/transpose). `accel_cpu::dispatch` maps an op to the right `ops::*` call.
- **Registry pattern to mirror:** `kernel/src/drivers/block/mod.rs` — `BlockDeviceIdentity` + `trait BlockDevice` + `static REGISTRY: Lazy<SwitchMutex<Registry>>` + `register_device`/`get_device`/`find_device_by_driver` (`:58`–`:146`). Build the **accel** equivalent the same way.
- **Module wiring:** `kernel/src/drivers/mod.rs` (`:13`–`:17` has `block/console/fb/net/pci`) — add `pub mod accel;`.
- **Storage class:** tensor storage is the buddy/tensor allocation class (Article 7.5), **not** slab. `accel_cpu` owns its allocations via that path (ANSPEC-001 §9.3), not the general kernel heap.

## 4. Implementation Success Criteria

- **ISC-1 — Trait + module.** `kernel/src/drivers/accel/mod.rs` defines `AccelDriver` per ANSPEC-001 §9.1; SPDX + `//!` docs; `pub mod accel;` wired in `drivers/mod.rs`.
- **ISC-2 — Residency live.** Tensors carry their `Device`; `DeviceStorage` is a driver-owned handle; the exactly-one-device invariant is documented at the allocation site. (Reuse the existing `Device` enum.)
- **ISC-3 — `accel_cpu` driver.** Concrete `AccelDriver` over host RAM: `alloc/free` via the tensor class; `copy_to/from_device` = memcpy; `dispatch` runs the op synchronously over `ai::ops`; `poll/wait` complete trivially; `preemptible()==false`. Supports exactly the Phase-5 op/dtype set.
- **ISC-4 — Executor through the trait.** `ai::exec` runs each op via the selected driver (default `accel_cpu`). **Forward-pass output byte-identical to Phase 5.**
- **ISC-5 — `tensor_migrate`.** `tensor_migrate(handle, Device)` exists and is exercised: Cpu→Accel(cpu)→Cpu does the real residency-change + driver-alloc + copy + free path (a no-op-copy for `accel_cpu`, but the *machinery* is real and tested) — shaking it out before a real device makes copies expensive/async.
- **ISC-6 — Capability registration.** An accel registry (mirror block): `accel_cpu` registers at init with its `AccelDescriptor` (supported ops/dtypes, memory = host RAM, dispatch latency). Queryable. (Foundation for Phase-7 placement; **no placement policy here**.)
- **ISC-7 — Errno + lifetime.** Unsupported op/dtype on a device → clean error, no panic; device storage freed at refcount zero; no leak on process exit. Reuse `errno.rs`.
- **ISC-8 — QEMU scenario + oracle.** A scenario that runs a forward pass **through `accel_cpu`** and asserts the **same output bytes** as the existing direct-dispatch oracle (the transparency proof), plus a `tensor_migrate` round-trip test. Host `ai-ops-tests` coverage for the driver wrapping.
- **ISC-9 — Standards/lean/gate.** rustfmt; SAFETY on new unsafe; no XMM regressions (the soft-float discipline is unchanged); no slab use for tensor storage; `--bare-metal-mode` parity unaffected.

## 5. Close gate

- `git fetch origin && git rebase origin/main`; re-run the gate after.
- **Transparency proof (the headline):** `infer_mlp_forward`, `infer_transformer_forward`, `infer_embedding_forward` produce **byte-identical** output routed through `accel_cpu` — QEMU at `NEO_SMP=4` and the host `ai-ops-tests` oracles, unchanged.
- New `accel_cpu` forward-pass scenario + `tensor_migrate` round-trip green at `NEO_SMP=4`.
- Host-checks green (`cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings`).
- **Housekeeping (do in this build):** amend **ANSPEC-001 §9.2** — the CPU accel driver (`accel_cpu`) lands in **Phase 6 BUILD-1**, not Phase 5 (Phase 5 built the executor with direct dispatch). One-line correction; keep the rest of §9.2.
- Single squashed commit; push to your **feature branch, not `main`** — PM merges via `pm-merge-check` + `--ff-only`. Close report with ISC pass/fail and the transparency-proof evidence (the oracle bytes are unchanged).

## 6. Scope guard

Abstraction + `accel_cpu` only. **No** virtio-gpu-compute (BUILD-2), **no** real-GPU code, **no** placement/scheduler logic (Phase 7), **no** async/preemption machinery beyond the trivial trait surface, **no** `.neom`/op-logic/oracle changes. If the trait surface tempts a big async refactor, stop — the lean trivial body is the deliverable.
