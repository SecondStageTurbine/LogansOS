<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE REPORT: Phase 5 — AI Subsystem Foundation

**Date:** 2026-06-04
**From:** PM/Cerebro
**Status:** ✅ COMPLETE — QEMU- and bare-metal-verified.
**Design substrate:** `docs/ADR-027` (phase substrate), `docs/ADR-029` (Layer-2 design)
**Bible:** Article 13 ledger; Articles 6.1/6.5, 20, 21; v1.10.0
**Close plan:** `Plans/PHASE-5-CLOSE-PLAN-2026-06-04.md`

## Milestone achieved

> A `.neom` model loads as a kernel object from disk and runs a correct CPU forward pass — MLP and transformer block — end to end through the native AI syscall family, capability-gated, no GPU. "Correct and slow" (Article 6.5).

Demonstrated by `infer_mlp_forward` and `infer_transformer_forward`: each loads a `.neom` from a real NeoFS disk (`SYS_MODEL_LOAD` 0x110), creates an input tensor (`SYS_TENSOR_CREATE` 0x100), runs the full graph (`SYS_INFER_RUN` 0x120), and reads back the output — one integrated path.

## What landed (the three layers)

| Layer | Build(s) | Deliverable | Close report |
|---|---|---|---|
| **0 — Tensor primitive** (Art. 20) | BUILD-1 | capability-gated, refcounted, immutable-by-default, buddy-frame storage; syscalls **0x100–0x103** | `…build-1-tensor-primitive` |
| **1 — Model object** (Art. 21) | BUILD-2 | `.neom` v1 parse/validate, immutable snapshot weights, `Handle::Model`, §D8 lazy-staleness (`ESTALE`, no live file handle, unmount never blocks); `neomc` authoring tool; syscalls **0x110–0x112** | `…build-2-*` |
| **2 — CPU inference backend** (Art. 6.1/6.5) | BUILD-3/4/5 | graph executor + soft-float core, **`SYS_INFER_RUN` 0x120**; full op set **0–11** (Identity, Add, Mul, MatMul, Relu, Gelu, Softmax, LayerNorm, RmsNorm, Embedding, Reshape, Transpose) | `…build-3/4/5-*` |

- **BUILD-3:** executor + matmul/add/mul/relu/reshape; MLP forward.
- **BUILD-4:** transcendental transformer ops (gelu/softmax/layernorm/rmsnorm + div/exp/sqrt/rsqrt/tanh/fmax in soft-float, **no hardware FP**, accuracy ≤ 2⁻¹² vs host `std` f32). Bounced once on accuracy (5% → reworked).
- **BUILD-5:** Embedding (OpCode 9) — I32-index gather into an F32 table, bounds-checked, bit-exact byte oracle.

## Two SMP kernel defects found & fixed en route (the phase dividend)

Exercising the inference path under SMP + the bare-metal harness flushed out two genuine, pre-existing kernel bugs the normal matrix never hit — both now fixed and pinned:

1. **TLB-shootdown timeout flake** (`5a9c878`) — a 1 ms ACK budget with no IPI re-send, marginal under QEMU/TCG. Fixed with a 1 ms re-poke + 50 ms bounded panic (382× latency margin). `Plans/CLOSE-REPORT-2026-06-03-tlb-shootdown-timeout-flake.md`. *(This flake had also masked a latent BUILD-4 opcode-ABI swap + missing affine norm, fixed in the same commit.)*
2. **Device-IRQ/IPI `swapgs`/per-CPU-GS hole** (`f5aacff`) — block-MSI and IPI vectors used plain `extern "x86-interrupt"` handlers with no `swapgs`; an IRQ in ring-3 ran `current_cpu_id` (`mov eax, gs:[24]`) with GS-base=0 → `cr2=0x18` fault. Fixed with swapgs-aware entry stubs mirroring the timer; `swapgs_irq_ring3` regression pin confirmed 125 ring-3 IPI entries handled cleanly. `Plans/DISPATCH-2026-06-04-swapgs-device-irq-page-fault-jarvis.md`.

## Verification

- **CI on `main`:** green (host-checks, SPDX, QEMU Integration Tests incl. all `infer_*` and `embedding` scenarios).
- **QEMU SMP=4:** `infer_mlp_forward`, `infer_transformer_forward`, `infer_embedding_forward`, `model_load_cascade`, `infer_embedding_oob_rejected` all PASS.
- **Bare metal (OptiPlex, 12 CPUs):**
  - *Foundation* (2026-06-03): real SMP, NeoFS disk I/O (the `.neom` load path), MM/TLB — via `phase3_ahci_neofs_mount_and_readwrite`; extracted-log sha256 `cf4d71a7…`.
  - *Inference compute* (2026-06-04): `infer_transformer_forward` PASS via physical NVMe `NEO_LOG` readback — 11 AP-ready lines, `NEO_LOG_v1` @ `0x10000000`, 9617 bytes, `softmax_sum=10000 sys_infer_run=ok storage_freed=ok`, **no FAIL/cr2/PANIC/BUG**. Evidence: `target/neo-test/infer_transformer_forward.optiplex.{nvme-log.raw,extracted.log}`; readback sha256 `A6D34715D67802F5BA30A0C68116C02805F2720B769D74A1C42DD9679E995063`, extracted sha256 `41702CDA5B5EE69FDA403CA564E1F8C1AE32EF8E675AAA6AEB41557432BDDA3D`.

## Deferred (out of Phase 5 scope; must not gate close — ADR-027 anti-scope)

- Embedding widening (higher-rank index tensors); `.neom` attribute mechanism (D11); `SYS_TENSOR_SHARE` 0x102 (stub).
- Accelerator drivers → **Phase 6**. Inference scheduler / multi-tenant serving → **Phase 7**. Governance policy → hooks-only, later.
- **Residual hardening (tracked, not blocking):** the NMI-during-`swapgs` window in the IRQ entry stubs (and the pre-existing timer stub) — a kernel-wide paranoid-entry item for later.

## Disposition

Phase 5 is **Complete and bare-metal-verified.** Intelligence is now a kernel primitive in the literal sense: a model loads from disk as a capability-gated kernel object and runs a forward pass through native syscalls on real hardware. The next phase — **Phase 6 (accelerator drivers)** vs **Phase 7 (inference scheduler / multi-tenant serving)** — is a separate PL/PM opening decision.
