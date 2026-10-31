<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 5 BUILD-3 — CPU Inference Executor → Jarvis

**Date:** 2026-06-01
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch)
**Status:** DISPATCHED — ready for BUILD
**Design contract:** `docs/ADR-029-phase-5-layer-2-cpu-inference-design.md` — **D1–D9 are normative.** Implement to the ADR.
**Builds on:** BUILD-1 tensor (`37075a3`), BUILD-2 model object + `tools/neomc` (`2f4c941`).
**Syscall range:** AI = `0x100..0x17F` (Bible 6.1). Tensors `0x100..0x103`, models `0x110..0x112`, **inference `0x120..0x12F`**.

---

## 1. Objective

Add the **CPU inference backend**: a graph executor that walks the validated
`.neom` op-graph and computes output tensors, exposed via `sys_infer_run`. The
build's milestone — **the Phase 5 goal milestone** — is an **end-to-end forward
pass on the `valid_mlp.neom` fixture** producing the correct `[1,3]` output, bit-
exact against a hand-computed oracle. Correct and slow (Article 6.5).

## 2. Decisions already locked (ADR-029 — do not re-litigate)

- **D1 — soft-float, no hardware FP.** Op arithmetic emits **no SSE/AVX/XMM**.
  Work on IEEE-754 f32 **bit patterns** (`u32`) via a software-f32 core
  (`add`, `mul`, bit-pattern `max` for Relu). This is preemption-safe by
  construction (the kernel doesn't save XMM across context switches). Confine all
  ops to `kernel/src/ai/ops/`. **No transcendentals in BUILD-3** (no exp/sqrt/
  tanh) — they belong to BUILD-4 ops.
- **D2 — synchronous** in the caller's syscall context. No kthread/async.
- **D3 — `match` dispatch** on `OpCode`. No device registry/trait/table.
- **D5 — kernel allocates outputs**, installs them as `TensorCap`s, returns
  handles. Caller does **not** pre-allocate output storage.
- **D9 — op set for BUILD-3:** `Identity, Reshape, Transpose, Add, Mul, MatMul,
  Relu` only. `Softmax/Gelu/LayerNorm/RmsNorm/Embedding` are **BUILD-4** —
  reject them as unsupported-in-this-build if encountered (the validator already
  accepts them format-wise; the *executor* declines them for now).
- **Syscall number is `SYS_INFER_RUN = 0x120`** — NOT `0x113`. The decade sub-block
  scheme (tensor `0x100` / model `0x110` / inference `0x120`) is intentional;
  keep it.

## 3. OBSERVE map (verified by Cerebro 2026-06-01) — incl. BUILD-2 gaps to fix

### ★ Gap 1 — `Model` must retain the graph (extend BUILD-2)
`kernel/src/ai/model.rs`: `Model` currently stores `weights: Vec<Arc<Tensor>>`,
`weight_metas`, and counts — but **drops `ValidatedNeom`** (the `nodes`/`ios`).
The executor needs the node list. **Retain `ValidatedNeom` (or at least its
`nodes: Vec<NodeMeta>` + `ios: Vec<IoMeta>` + `value_count`) in `Model`** and add
accessors: `weights() -> &[Arc<Tensor>]`, `nodes() -> &[NodeMeta]`,
`ios() -> &[IoMeta]`, `value_count()/input_count()/output_count()`. All fields are
private with no getters today.

### ★ Gap 2 — topological order is NOT stored (validator checked acyclicity only)
`model_logic::validate_neom` validates the graph is a DAG but discards order.
neomc emits nodes in topo order and the MLP fixture is sequential, but the
executor must **not blindly trust** it: walk nodes in file order and, for each
node, require every input value to be **already produced** (weight, model input,
or an earlier node's output). If an input value is not yet available →
`EINVAL`. (Defensive; do not compute a separate topo sort — file-order walk with
the produced-before-use check is sufficient and lean.)

### Validated graph structures — `kernel/src/ai/model_logic.rs`
- `validate_neom(bytes) -> Result<ValidatedNeom, ModelFormatError>` (l.221).
- `ValidatedNeom { header, tensors: Vec<TensorMeta>, nodes: Vec<NodeMeta>,
  ios: Vec<IoMeta>, value_count: u32 }` (l.177).
- `NodeMeta { op: u32, inputs: Vec<u32>, outputs: Vec<u32>, .. }` (l.158) —
  **inputs/outputs are already-resolved value indices**, range-checked by the
  validator. This is the executor's input.
- `IoMeta { value_index, dtype, shape, is_output }` (l.169).
- Value space: `[0, tensor_count)` weights, `[tensor_count, +input_count)` inputs,
  then node outputs, up to `value_count`. (MLP: 0–3 weights, 4 input, 5–9 outputs.)
- `OpCode` discriminants (neomc `tools/neomc/src/format.rs`): Identity=0, Add=1,
  Mul=2, MatMul=3, Relu=4, … Reshape=10, Transpose=11.

### Tensor surface — `kernel/src/ai/tensor.rs` (BUILD-3 adds the writable API, D4)
- Today: `from_desc_and_bytes(desc, &[u8])`, `query_desc(refcount)`. **No byte-read
  accessor, no writable path.** Storage = buddy frame via
  `mm::with_frame_alloc(|a| a.allocate_blocks(order))` + `phys_to_virt` copy.
- **Add (D4):** `Tensor::allocate_writable(shape: Shape, dtype: Dtype) ->
  Result<Tensor, TensorError>` (buddy storage, `mutable=true`, zero-init);
  `bytes(&self) -> &[u8]` (read, for any tensor); `bytes_mut(&self) ->
  Result<&mut [u8], TensorError>` **guarded: immutable tensor → error
  (`EPERM`-class)**. This is where BUILD-1's deferred immutability *enforcement*
  lands — weights reject `bytes_mut`, activations accept it. Every new `unsafe`
  gets a `// SAFETY:` comment.
- `Shape::{rank,dims,num_elements,byte_len}`, `Dtype::size_bytes` already exist.

### Syscall template — `kernel/src/arch/x86_64/syscall.rs`
- Consts l.77–84 (add `SYS_INFER_RUN: u64 = 0x120` + a `0x120..0x12F` range
  doc-note). Dispatch: add the arm.
- `model_lookup(h) -> Result<Arc<Model>, i64>` (l.937) and `tensor_lookup(h) ->
  Result<(Arc<Tensor>, CapId, CapRights), i64>` (l.796) — use both.
- Array copy in: `copy_user_u32s(src, count) -> Vec<u32>` (l.~470) for the input-
  handle array. Out: `copy_kernel_bytes_to_user` for the output-handle array.
- Output install: mirror `sys_tensor_create_impl` (l.822) `install_cap` path,
  `full_tensor_rights()`, fresh `alloc_cap_id()`, per output tensor.

### `sys_infer_run` ABI (ADR-029 D5)
```
SYS_INFER_RUN = 0x120
sys_infer_run_impl(model_h: u32,
                   inputs_ptr: *const u32, input_count: u32,
                   outputs_ptr: *mut u32,  output_cap: u32,
                   flags: u32) -> i64   // >=0: #output handles written; <0: errno
```
Flow: `model_lookup` (→ also `validate_current()` → `ESTALE` if stale) → resolve
`input_count` input `TensorCap`s, check each input's dtype/shape against the
model's `IoMeta` inputs (`EINVAL` on mismatch) → run the graph (§ below) →
allocate output tensors (immutable, from the produced output values), install as
`TensorCap`s, write up to `output_cap` handles to `outputs_ptr` (`E2BIG` if
`output_count > output_cap`) → return count. `flags != 0` → `EINVAL`.

### The executor (`kernel/src/ai/exec.rs` or similar)
Per-`inf_run` value table: `Vec<Option<Arc<Tensor>>>` indexed by value-id, length
`value_count`. Seed: weights into `[0,tensor_count)` (from `Model::weights()`),
inputs into their `IoMeta.value_index` slots. Walk `nodes` in file order; for each,
gather input tensors (produced-before-use check, Gap 2), `match op { … }` →
`ai::ops::*` soft-float kernel → write result into a freshly `allocate_writable`
tensor at each output value-id. After the walk, collect the `is_output` IoMeta
values as the results.

## 4. Op kernels (BUILD-3 set, soft-float, D1/D9)

All on f32 bit-patterns, no XMM. Row-major.
- **Identity**: copy. **Reshape**: copy bytes, new shape (same element count).
- **Transpose**: 2-D index permutation.
- **Add / Mul**: elementwise; support the MLP's bias broadcast (`[1,N]` over
  `[M,N]`). Document the broadcast rule supported (NumPy-style trailing-dim;
  keep it to what the MLP needs + clean general 2-D, no more — Article 15.7).
- **MatMul**: 2-D `[M,K]·[K,N] → [M,N]`, triple loop, soft-float `mul`+`add`
  accumulate.
- **Relu**: `x<0 → 0` via the sign bit on the f32 pattern (no FP compare needed).

## 5. Acceptance gate (ISC)

- **ISC-1 — Model graph retention (Gap 1).** `Model` retains nodes/ios/value_count
  + accessors; `cargo check` green.
- **ISC-2 — Writable tensor API (D4).** `allocate_writable`, `bytes`, guarded
  `bytes_mut`; immutable tensors reject `bytes_mut`. SAFETY comments on new unsafe.
- **ISC-3 — Soft-float ops (D1/D9).** `ai/ops/` implements the BUILD-3 op set on
  f32 bit-patterns. **Verification: disassemble the ai ops objects and assert no
  `xmm`/`movss`/`addss`/`mulss`/`divss`** (objdump grep in the test/CI step, or a
  documented build check). No transcendentals.
- **ISC-4 — Executor.** File-order graph walk with produced-before-use check
  (Gap 2); per-value table; `match` dispatch (D3); unsupported op (BUILD-4 set) →
  clean error, no panic.
- **ISC-5 — `sys_infer_run` (D5).** `0x120`, registered + dispatched + range-
  documented; ABI per §3; kernel-allocates-and-returns output handles.
- **ISC-6 — Errno.** stale model `ESTALE`; bad/again handle `EBADF`; rights `EPERM`;
  dtype/shape/flags/produced-before-use `EINVAL`; output buffer too small `E2BIG`;
  OOM `ENOMEM`; unsupported op (BUILD-4) a distinct clean errno (`ENOSYS`/`EINVAL`).
- **ISC-7 — Host unit tests** (`tools/ai-ops-tests`, `#[path=…]` like
  `tensor-logic-tests`; keep soft-float ops in a host-importable pure module):
  soft-float `add`/`mul`/`max` vs known bit patterns; 2×2 MatMul; bias-broadcast
  Add; Relu; **full MLP forward pass vs a hand-computed expected `[1,3]` output**.
- **ISC-8 — QEMU scenario `infer_mlp_forward`** at `NEO_SMP=4`: stage
  `valid_mlp.neom` (from `cargo run -p neomc -- fixtures`) on NeoFS → `SYS_MODEL_LOAD`
  → `SYS_INFER_RUN` with a known input → assert the output tensor bytes equal the
  oracle → drop. Sentinel `NEO_TEST/1 event=PASS name=infer_mlp_forward …`. Full
  `neo-test all --no-meta` at `NEO_SMP=4` stays green.
- **ISC-9 — Standards + lean.** rustfmt; `SAFETY` on every unsafe; no post-init
  `unwrap`/`expect`; SPDX headers; `/simplify` before BUILD. Anti-scope (no hardware
  FP, no transcendentals, no accel, no scheduler/8.2 class, no async) holds.
  **Clippy:** run `cargo clippy` on the **host** `tools/ai-ops-tests` crate (must
  be clean); kernel clippy is known-not-runnable in the worker container — note it,
  CI covers it. (Do not claim kernel clippy clean — see the BUILD-2 record.)

## 6. Workflow

Branch off current `main` (e.g. `codex/phase-5-build-3-cpu-inference-jarvis`).
`/simplify` before BUILD. Single squashed commit; close report
(`Plans/CLOSE-REPORT-…`) with ISC pass/fail, the soft-float no-XMM disassembly
evidence, the MLP oracle values, and honest clippy disposition. **Also: if you add
a workspace member (e.g. `tools/ai-ops-tests`), regenerate `Cargo.lock` and commit
it** — BUILD-2 missed this and `--locked` broke on main.

**Rebase-before-close gate (mandatory).** Immediately before declaring close, the
PM seat will have advanced `main` (this dispatch, ADR-029, the CI-gate merge).
Rebase onto it so the branch merges fast-forward-clean:

    git fetch origin
    git rebase origin/main
    # re-run the full ISC gate AFTER the rebase (build + host tests + the QEMU
    # scenario), then push the rebased branch

Do not close on a branch whose base is behind `origin/main` — that forces a manual
cherry-pick at merge (it bit BUILD-2 and the CI-gate merge). Cerebro verifies with
`scripts/pm-merge-check.sh` before merging; a STALE result bounces back here.
Cerebro reviews + merges (`--ff-only`).
