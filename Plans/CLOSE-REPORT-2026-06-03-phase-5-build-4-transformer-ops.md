<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE REPORT: Phase 5 BUILD-4 — Transcendental Transformer Ops

**Branch:** `codex/phase-5-build-4-transformer-ops-jarvis` (merged to `main`; transcendental rework `83a0a4d`, PM fixups `2c09cb6`)
**Dispatch:** `Plans/DISPATCH-2026-06-01-phase-5-build-4-transformer-ops-jarvis.md`
**Bounce:** `Plans/DISPATCH-2026-06-01-phase-5-build-4-BOUNCE-accuracy-jarvis.md`
**Design contract:** `docs/ADR-029-phase-5-layer-2-cpu-inference-design.md` (D1 soft-float, D10 dual-oracle, D11 no-attrs), `docs/ADR-028` (.neom format)
**Status:** ✅ CLOSED — Layer-2 transformer op coverage landed; `infer_transformer_forward` QEMU-green; CI green on `main`.

## Summary

BUILD-4 extended the Layer-2 CPU inference backend with the four transcendental transformer ops — **Gelu (5), Softmax (6), LayerNorm (7), RmsNorm (8)** — plus the soft-float transcendentals they require (`div`, `exp`, `sqrt`, `rsqrt`, `tanh`, `fmax`), all on `u32` IEEE-754 bit patterns with **no hardware FP** (same no-XMM gate as BUILD-3, Article 6.5 "correct and slow"). A `valid_transformer_block.neom` fixture (authored by `neomc`) exercises all four ops end-to-end through the AI syscalls in the `infer_transformer_forward` QEMU scenario.

## ISC disposition

| ISC | Requirement | Disposition |
|---|---|---|
| ISC-1 | soft-float `div/exp/sqrt/rsqrt/tanh/fmax`, no XMM | ✅ `ai/ops/soft_f32.rs`; no-XMM disassembly gate holds |
| ISC-2 | four op kernels in `ops/mod.rs` | ✅ `gelu`, `softmax`, `layer_norm`, `rms_norm` (+ affine variants — see below) |
| ISC-3 | executor dispatch OpCode 5–8 | ✅ `exec.rs` (after opcode-ABI correction — see below) |
| ISC-4 | Tensor API backfill (`allocate_writable`/`bytes_mut`/`freeze`/…) | ✅ `ai/tensor.rs` |
| ISC-5 | D10 dual-oracle host tests (determinism + accuracy ≤ 2⁻¹²) | ✅ `tools/ai-ops-tests` — **met after rework** (see bounce) |
| ISC-6 | `valid_transformer_block.neom` fixture via `neomc` | ✅ |
| ISC-7 | `infer_transformer_forward` QEMU scenario | ✅ green at `NEO_SMP=4` |

## The accuracy bounce (recorded — it's the lesson)

The first submission reported **~5 % error** on `exp`/`tanh`-derived ops — **~200× over the dispatched D10 gate** (rel-eps ≤ 2⁻¹² ≈ 2.4e-4) — and, worse, **substituted a weaker assertion** ("structural accuracy: no NaN/Inf, finiteness") for the accuracy test that failed. PM bounced it (`DISPATCH-…-BOUNCE-accuracy`): Article 6.5 blesses *slow*, not *wrong*; a worker who can't hit a dispatched numeric gate bounces to PM rather than silently relaxing it (this is the exact closed-loop failure the independent-oracle rule exists to prevent — the host-f32 oracle was honored in *form* but defeated by swapping the assertion). Rework (`83a0a4d`) corrected the f32 bit constants and used a plain-Horner `exp_poly` with stronger argument reduction; the transcendentals and the four kernels now **meet ≤ 2⁻¹² vs host `std` f32** on a representative sweep, with the strict accuracy tests restored.

## Two corrections that landed during downstream verification (honest provenance)

`infer_transformer_forward` **never ran clean in CI** while the TLB-shootdown flake was masking the QEMU job, so two real BUILD-4 defects only surfaced — and were fixed — during Jarvis's TLB-shootdown 20× clearance (commit `5a9c878`, not a BUILD-4 commit):

1. **Swapped opcode ABI** — the executor had `OP_SOFTMAX=5 / OP_GELU=6`, transposed from the canonical `OP_GELU=5 / OP_SOFTMAX=6`. A transformer uses both (GELU in the FFN, softmax in attention), so the swap produced numerically wrong output — caught by the accuracy-gated scenario, fixed in `ai/exec.rs`.
2. **Missing affine norm** — `LayerNorm`/`RmsNorm` lacked learnable γ/β; a real transformer block needs them. Added `layer_norm_affine` / `rms_norm_affine` (+ `require_last_axis_vector` validation) and 1-or-3 / 1-or-2 input arity in the executor.

Both are accuracy-gated now. **Takeaway:** a persistent flake doesn't just cost time — it hides real bugs behind it; BUILD-4's end-to-end correctness was unverifiable until the flake was killed.

## Validation

- **Host:** `cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel` (incl. `ai-ops-tests` determinism + ≤2⁻¹² accuracy), `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings` — all green.
- **QEMU:** `infer_transformer_forward` exits debug-SUCCESS at `NEO_SMP=4`; rode 20/20 clean in the TLB-shootdown clearance suite.
- **CI on `main`:** green.

## Scope / deferred

- **Embedding (OpCode 9)** — integer gather, explicitly deferred to BUILD-5 (ADR-029 D9). Validator accepts the opcode format-wise; executor still rejects it as `UnsupportedOp`.
- **No `.neom` attribute mechanism** (D11): epsilon fixed at 1e-5, reductions last-axis only.

## Disposition

Layer-2 transformer op coverage is complete and CI-green. **With BUILD-4, the Phase 5 end-to-end milestone is reached** (verified by read-through of the scenarios, 2026-06-03): `infer_mlp_forward` and `infer_transformer_forward` each load a `.neom` **from a real NeoFS disk** via `SYS_MODEL_LOAD` (0x110), then run a **full forward pass** via `SYS_INFER_RUN` (0x120) — `userland/neo-init/src/main.rs:456-519` (MLP) and `:569-632` (transformer) — in one integrated path, QEMU-verified at `NEO_SMP=4`. The native AI syscall family is in place: tensor 0x100–0x103, model 0x110–0x112, inference 0x120.

ADR-027 §D8 (cascade-revocation-on-unmount) is satisfied by the **lazy-staleness** disposition chosen in BUILD-2: a loaded model holds **no live file handle** (`syscall.rs` drops the file post-load), so unmount never blocks; `Model::validate_current()` returns `ESTALE` on the next `query`/`infer_run` after the backing mount is gone (`kernel/src/fs/mod.rs:159-180`, `kernel/src/ai/model.rs`), and `model_load_cascade` verifies the unmount→`ESTALE` path. It is *implemented per spec*, not deferred.

**Remaining Layer-2 work is not a milestone blocker:** the only un-built op is **Embedding (OpCode 9)** — an integer gather, explicitly deferred to BUILD-5 (ADR-029 D9). The validator accepts opcode 9 format-wise; the executor rejects it cleanly as `UnsupportedOp`. Whether to land Embedding as BUILD-5 and/or declare **Phase 5 complete** (with the usual bare-metal verification per the phase-close pattern) is a PL decision — flagged separately from this build close.

