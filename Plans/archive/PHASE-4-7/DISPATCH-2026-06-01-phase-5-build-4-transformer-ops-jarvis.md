<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 5 BUILD-4 — Transcendental Transformer Ops → Jarvis

**Date:** 2026-06-01
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch)
**Status:** DISPATCHED — ready for BUILD
**Design contract:** `docs/ADR-029-phase-5-layer-2-cpu-inference-design.md` — D1 (soft-float), D3 (match dispatch), D9 (slice), **D10 (dual oracle), D11 (no attr mechanism)** are normative.
**Builds on:** BUILD-3 CPU inference executor (merged `806756f`).
**Format reference:** `docs/ADR-028` — note the **authoritative worked byte example** and the absolute-`weight_off` defect note; honor them.

---

## 1. Objective

Add the transcendental transformer ops to the CPU inference backend: **`Softmax`
(6), `Gelu` (5), `LayerNorm` (7), `RmsNorm` (8)**. This requires extending the
soft-float core with the transcendentals these ops need, then wiring four op
kernels into the existing executor. Milestone: an **end-to-end forward pass on a
new transformer-block fixture** exercising all four ops, accuracy-verified against
host hardware float. Correct and slow (Article 6.5).

**`Embedding` (9) is NOT in this build** — it's an integer gather, deferred to
BUILD-5 (ADR-029 D9). Do not implement it; the validator already accepts opcode 9
format-wise, but the executor should continue to reject it cleanly as unsupported.

## 2. Decisions already locked (ADR-029 — do not re-litigate)

- **D1 soft-float, no hardware FP.** The new transcendentals (`div`, `exp`,
  `sqrt`/`rsqrt`, `tanh`, compare/`max`) are implemented in `ai/ops/soft_f32.rs`
  on `u32` IEEE-754 bit patterns — **no SSE/AVX/XMM**. Same no-XMM disassembly
  gate as BUILD-3 applies to the new code.
- **D10 dual-oracle testing.** Each transcendental is tested (1) for **determinism**
  (recorded bit pattern, stable run-to-run) and (2) for **accuracy vs host `std`
  f32** within **relative epsilon ≤ 2^-12 (~2.4e-4)**. Host float is the
  independent oracle (satisfies the CLAUDE.md byte-oracle rule for numerics).
- **D11 no attribute mechanism.** Softmax/LayerNorm/RmsNorm reduce over the
  **last axis (`rank-1`)**; epsilon is a **fixed `1e-5`**. Do **not** add
  `.neom` attribute parsing or change the format — that's deferred until a model
  needs a non-default (Article 15.7; and we just fixed an ADR-028 defect, don't
  churn the format).
- **Op set is exactly these four.** No Embedding, no new non-transformer ops.

## 3. OBSERVE map (verified by Cerebro 2026-06-01)

### soft-float core — `kernel/src/ai/ops/soft_f32.rs`
- Today: `zero()`, `relu(bits)`, `add(a,b)`, `mul(a,b)` on `u32` patterns;
  internal `Finite{sign,mant,exp2}` decomposition (`from_bits`), `pack`,
  `round_shift_right`, `is_nan`/`is_inf`. **Missing (you add):** `div`, `exp`,
  `sqrt` or `rsqrt`, `tanh` (or `erf`), and a `max`/compare for softmax stability.
  Build them from the existing `Finite` decomposition + add/mul. Suggested:
  `exp` via range-reduction (`x = k·ln2 + r`, polynomial on `r`); `sqrt`/`rsqrt`
  via Newton iteration seeded from exponent bit manipulation; `tanh` via
  `exp`-based formula or a clamped polynomial; document the method + error bound
  per function (it feeds the D10 accuracy test).

### op kernels + dispatch — `kernel/src/ai/ops/mod.rs`, `kernel/src/ai/exec.rs`
- Op kernel convention: `pub fn NAME(input: &TensorView, out: &mut [u8]) ->
  Result<(), OpError>` (binary ops take two `TensorView`). `TensorView{dtype,
  shape:&Shape, bytes:&[u8]}` (ops/mod.rs:9). Helpers `read_bits(bytes,idx)`,
  `write_bits(bytes,idx,v)`, `element_count(shape)`, `require_f32`,
  `require_output_len`. Row-major flat indexing. `matmul` (ops/mod.rs:78) is the
  end-to-end template.
- **Shape/axis is available** to a kernel via `input.shape.rank()` /
  `.dim(idx)` — so last-axis reductions (Softmax/LayerNorm/RmsNorm) read the row
  width as `dim(rank-1)` and iterate rows. (No attrs needed — D11.)
- Executor: `exec.rs:14-20` op-const block (add `OP_GELU=5, OP_SOFTMAX=6,
  OP_LAYERNORM=7, OP_RMSNORM=8`); dispatch `match` in `execute_node()`
  (exec.rs:84-180) — add four arms; the `_ =>` catch-all (exec.rs:170) keeps
  rejecting Embedding/unknown as `InferenceError::UnsupportedOp`. Per-node flow
  (allocate_writable → bytes_mut → kernel → freeze → store) is unchanged.
- Validation `model_logic.rs:393` already accepts opcodes ≤ 11; **no validation
  change needed** — only executor dispatch + kernels.

### op semantics (last-axis, fixed eps=1e-5, all F32)
- **Gelu(x)** elementwise: use the tanh approximation
  `0.5·x·(1 + tanh(√(2/π)·(x + 0.044715·x³)))` (standard; document it). Needs `tanh`.
- **Softmax** over last axis: per row, `m = max(row)`; `e_i = exp(x_i − m)`;
  `out_i = e_i / Σe` (subtract-max for stability). Needs `exp`, `div`, `max`.
- **LayerNorm** over last axis: `mean`, `var` across the row; `out_i = (x_i −
  mean)/sqrt(var + 1e-5)`. (No affine γ/β weights in this build unless the fixture
  provides them as separate Mul/Add nodes — keep the op itself parameter-free.)
  Needs `sqrt`/`rsqrt`, `div`.
- **RmsNorm** over last axis: `out_i = x_i / sqrt(mean(x²) + 1e-5)`. Needs
  `sqrt`/`rsqrt`.

### tensor API (from BUILD-3, use as-is) — `kernel/src/ai/tensor.rs`
- `Tensor::allocate_writable(shape,dtype)` (zeroed), `bytes_mut()` (guarded;
  immutable → err), `freeze()`. SAFETY comments on any new unsafe.

### OpCode parity — already correct
- `tools/neomc/src/format.rs:66` OpCode enum already defines Gelu=5…Transpose=11,
  matching ADR-028 D3. neomc can already *emit* these ops — you mainly add the
  **fixture** (below) and the kernel side.

### host tests + fixture — `tools/ai-ops-tests/`, `tools/neomc/src/fixtures.rs`
- ai-ops-tests pattern: `bytes_from_bits`/`bits_from_bytes`, `view`, `shape2`,
  exact-bit asserts. **Add a tolerance helper** (relative-eps compare vs `f32`
  host ops) for D10. neomc has only `valid_mlp.neom`; **add a
  `valid_transformer_block.neom` fixture** exercising the four ops (e.g.
  input → LayerNorm → MatMul → Softmax → … → Gelu → output), with a hand/host-
  computed expected output for the QEMU end-to-end test.

## 4. Acceptance gate (ISC)

- **ISC-1 — soft-float transcendentals.** `div`, `exp`, `sqrt`/`rsqrt`, `tanh`,
  `max`/compare added to `soft_f32.rs`; each documents its method + error bound;
  no XMM (disassembly gate).
- **ISC-2 — four op kernels.** `gelu`, `softmax`, `layernorm`, `rmsnorm` in
  `ops/mod.rs`, last-axis reductions via `shape`, fixed eps=1e-5, F32; clean
  `OpError` on rank/shape/dtype violations, no panics.
- **ISC-3 — executor dispatch.** `OP_GELU/SOFTMAX/LAYERNORM/RMSNORM` arms added;
  Embedding (9) + unknown still cleanly rejected (`UnsupportedOp` → `ENOSYS`).
- **ISC-4 — D10 dual-oracle host tests** (`tools/ai-ops-tests`): each
  transcendental + each op has a **determinism** test (recorded bits) and an
  **accuracy** test vs host `std` f32 within rel-eps ≤ 2^-12; plus a full
  transformer-block forward-pass test.
- **ISC-5 — fixture.** `valid_transformer_block.neom` generated by neomc,
  exercises all four ops, self-validates, and is the QEMU scenario's input.
- **ISC-6 — QEMU scenario** `infer_transformer_forward` at `NEO_SMP=4`: stage the
  fixture → `SYS_MODEL_LOAD` → `SYS_INFER_RUN` → assert output within rel-eps of
  the host-computed oracle → drop. Sentinel PASS. Full `neo-test all --no-meta`
  at SMP=4 green (capture the **exit code**, not just serial — BUILD-3's close
  flagged the outer-timeout gap; run the target scenario discretely so its exit
  is recorded).
- **ISC-7 — Standards + lean + gate.** rustfmt; SAFETY on new unsafe; no post-init
  unwrap/expect; SPDX; `/simplify` before BUILD. **Host gate green**
  (`cargo test --workspace --exclude kernel`, `cargo fmt --all --check`, `cargo
  clippy --workspace --exclude neoplutarch-kernel -- -D warnings`) — the
  host-checks CI job now enforces this, so it must pass. `--locked` clean if a
  member/dep changes. Anti-scope: no hardware FP, no Embedding, no `.neom` attr
  mechanism (D11), no accel, no scheduler/8.2 class.

## 5. Workflow

Branch off **current `main`** (`git fetch && git checkout -b
codex/phase-5-build-4-transformer-ops-jarvis origin/main`). `/simplify` before
BUILD.

**Rebase-before-close gate (mandatory; CLAUDE.md):** immediately before close,
`git fetch origin && git rebase origin/main`, re-run the full ISC gate after the
rebase, then push the branch. **Push to the feature branch, NOT to `main`** — the
PM seat merges via `scripts/pm-merge-check.sh` + `--ff-only` after review.
(BUILD-3 was pushed direct-to-main; don't repeat that.) Single squashed commit;
close report (`Plans/CLOSE-REPORT-…`) with ISC pass/fail, per-transcendental
error bounds, the no-XMM disassembly evidence, the transformer oracle values, and
honest gate disposition. Cerebro reviews + merges.
