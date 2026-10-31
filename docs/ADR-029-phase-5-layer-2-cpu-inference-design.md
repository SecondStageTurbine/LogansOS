<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-029: Phase 5 Layer 2 — CPU Inference Backend Design

| | |
|---|---|
| **Status** | ✅ Accepted (PM decision; PL may reverse Q1 — see D1) |
| **Date** | 2026-06-01 |
| **Authority** | PM (Q1 flagged for PL; PL delegated "continue") |
| **Supersedes** | None |
| **Superseded by** | - |
| **Related** | Bible Articles 6.1, 6.5 ("correct and slow is acceptable"), 7.5, 8.2 (inference scheduling class), 15.7 (lean patch rule), 20–22 (Tensor/Model/Inference Context); ADR-027 (§D layering), ADR-028 (`.neom` v1, op-graph + opcode set); BUILD-1 tensor primitive (`37075a3`); BUILD-2 model object (`2f4c941`); `Plans/PM-NOTE-2026-05-31-phase-5-build-3-layer-2-recon.md` (OBSERVE bank, Q1–Q8) |

---

## Context

Layers 0 (tensor) and 1 (model object + `.neom` load) are merged. Layer 2 is the
**CPU inference backend**: *execute* the ADR-028 op-graph to produce output
tensors — the Phase 5 goal milestone (Article 6.5: "a `.neom` model runs a
forward pass on the CPU, correct and slow"). The PM-NOTE OBSERVE pass mapped the
hooks and surfaced eight design questions (Q1–Q8) and one hard constraint. This
ADR resolves them and slices Layer 2 into two builds.

## Decisions

### D1 — Floating-point strategy: **soft-float** (Q1, the pivotal one)

**Hard constraint (from recon):** the kernel enables FPU/SSE at boot but
`context_switch` saves **no** XMM/MXCSR state (`arch/x86_64/context.rs`), and a
CPU-bound op is preempted every ~10 ms. Hardware FP in the inference path would be
corrupted by any preemption mid-op.

**Decision:** Layer 2 op kernels use **no hardware floating-point** — no SSE/AVX
in the inference code path. f32 arithmetic is done in software (soft-float). This
is preemption-safe *by construction* (no XMM to clobber), confines the entire
change to `kernel/src/ai/` (Article 15.7 — zero scheduler blast radius), and is
exactly the "correct and slow" Article 6.5 sanctions. Hardware-FP + FXSAVE/FXRSTOR
(or lazy-FP) is the **deferred performance path**, revisited when a perf pass or
accelerator work (Phase 6) justifies touching the context switch.

**Recommended mechanism (worker THINK finalizes):** confine all op arithmetic to
an `ai/ops/` module whose compiled output emits **no XMM instructions** — a small
software-f32 core (`add`, `mul`, and bit-pattern `max` for Relu) operating on
`u32` IEEE-754 bit patterns. Verifiable: disassemble the ai ops objects and assert
no `xmm`/`movss`/`mulss`/`addss` (ISC). Transcendentals (`exp`, `sqrt`, `tanh`,
`rsqrt`) are **not** needed in BUILD-3 (see D9 slice) — they arrive with the ops
that need them in BUILD-4, where soft-float transcendentals are their own scoped
problem.

**Reversibility:** if PL prefers hardware FP, the alternative is FXSAVE/FXRSTOR (or
lazy-FP via the FPU-not-present trap) in `context_switch`. That replaces D1's op
core but not the rest of this ADR. Recorded so the choice stays open.

### D2 — Execution model: synchronous (Q2)

`inf_run` runs **synchronously in the caller's kernel-mode syscall context** and
returns when outputs are populated (Article 22.1's signature implies this). No
kthread offload, no async path. Best-effort; ordinary 10 ms preemption applies
and is safe under D1 (no FP state to lose). Scheduled/async multi-tenant inference
is **Phase 7**.

### D3 — Op-kernel dispatch: `match` (Q3)

Op execution dispatches on the ADR-028 `OpCode` via a `match` in the graph
executor. No device registry, no function-pointer table, no `DeviceOps` trait —
there is one device (CPU) and accelerator dispatch is Phase 6 (Article 15.7).

### D4 — Intermediate tensors + writable API (Q4)

Layer 2 adds the writable-tensor API BUILD-1 deferred:
- `Tensor::allocate_writable(shape, dtype) -> Result<Tensor, TensorError>` —
  buddy-frame storage (Article 7.5), `mutable = true`, zero-initialized.
- `Tensor::bytes_mut(&self) -> Result<&mut [u8], TensorError>` — guarded:
  returns `EPERM`/error for immutable tensors. **This is where BUILD-1's deferred
  immutability *enforcement* lands** — weights (immutable) reject `bytes_mut`;
  only activations (writable) accept it.

**Lifetime:** intermediates are `Arc<Tensor>` held in a per-`inf_run` value table
indexed by graph value-id; each is dropped when the executor finishes (the whole
table drops at call end — simple, bounded by the model's value count). Weights are
shared `Arc<Tensor>` from the `Model` (never copied, never mutated).

### D5 — `inf_run` ABI (Q5)

Article 21.3: "inference produces per-call output tensors owned by the calling
process." So the **kernel allocates outputs, installs them as `TensorCap`s, and
returns handles** — the caller does not pre-allocate output storage.

```
sys_infer_run(model_h: u32,
              inputs_ptr: *const u32, input_count: u32,   // array of input TensorCap handles
              outputs_ptr: *mut u32,  output_cap: u32,    // kernel writes output handles here
              flags: u32)                                 // reserved, must be 0
        -> i64   // >=0: number of output handles written; <0: errno
```

Inputs are validated against the model's declared input count/dtypes/shapes;
outputs are created as fresh immutable `TensorCap`s in the caller's table and
their handles copied to `outputs_ptr` (up to `output_cap`; `E2BIG` if too small).

### D6 — No Article 8.2 scheduling class (Q6)

Layer 2 inference is **best-effort** — no inference scheduling class, no priority,
no fairness. Article 8.2's class + multi-tenant fairness is **Phase 7**. ADR states
this boundary explicitly so we do not half-build a scheduler.

### D7 — Op-kernel testing: hand-computed oracles (Q7)

Host crate `tools/ai-ops-tests` (mirrors `tensor-logic-tests`), pure-logic, tested
against **hand-computed known values** (zero external deps, fully deterministic) —
soft-float `add`/`mul`/`max`, a 2×2 `MatMul`, `Add` broadcast, `Relu`, and a full
small-MLP forward pass with a precomputed expected output. Determinism is a
requirement (bit-exact across runs).

### D8 — Syscalls + Bible (Q8)

Inference sub-block **`0x120..0x12F`** of the AI range (Bible 6.1):
`SYS_INFER_RUN = 0x120` (BUILD-3). A range doc-note is added like the tensor/model
blocks; this realizes Article 22 (Inference Context). Bundle the one-line Bible
6.1 range note with the BUILD-3 close (ledger-only).

### D9 — The slice: BUILD-3 then BUILD-4

Layer 2's full opcode set is too large for one lean build. Slice on the
soft-float frontier:

- **BUILD-3 — inference engine + MLP-sufficient ops (this dispatch).** The graph
  executor (topological walk, per-value table), the writable-tensor API (D4),
  `sys_infer_run` (D5), and the ops whose soft-float is just `add`/`mul`/bit-`max`:
  **`Identity`, `Reshape`, `Transpose`, `Add`, `Mul`, `MatMul`, `Relu`.** Proves
  an **end-to-end forward pass on the `valid_mlp.neom` fixture** (the Phase 5 goal
  milestone). No transcendentals.
- **BUILD-4 — transcendental transformer ops.** `Softmax`, `Gelu`, `LayerNorm`,
  `RmsNorm` — the ops needing `exp`/`sqrt`/`tanh` in soft-float. Scoped separately
  because soft-float transcendentals are their own correctness problem.
- **BUILD-5 — `Embedding` + (if needed) attribute mechanism.** Embedding is an
  **integer-index gather**, not a transcendental elementwise op — a different
  dtype path (I32 indices into an F32 table). Separated from BUILD-4 so the
  transcendental work stays one coherent unit. (Amendment 2026-06-01, narrowing
  the original D9 BUILD-4 set; see D10/D11.)

### D10 — Transcendental testing: dual oracle (amendment 2026-06-01)

BUILD-3 op tests asserted **bit-exact** against hand-computed values. Soft-float
`exp`/`sqrt`/`tanh` cannot be bit-exact against a hand oracle, so BUILD-4 tests
each transcendental two ways: **(1) determinism** — a recorded bit pattern that
must not change run-to-run (locks reproducibility); **(2) accuracy** — compared
against the host's **hardware `std` f32** (`f32::exp`, `f32::sqrt`, etc.) within a
documented **relative epsilon** (target ≤ 2^-12 ≈ 2.4e-4). Host hardware float is a
genuinely *independent* oracle of our soft-float implementation — satisfying the
independent-byte-oracle rule (CLAUDE.md) for numeric kernels. The accuracy target
is "useful for inference," not IEEE-correct rounding (Article 6.5).

### D11 — No attribute mechanism in BUILD-4 (amendment 2026-06-01)

The `.neom` NodeEntry has `attr_off`/`attr_len`, but BUILD-3 never wired attribute
parsing into the executor. BUILD-4 ops that *could* be parameterized
(Softmax axis, LayerNorm/RmsNorm epsilon) instead use **fixed conventions**:
softmax/normalization reduce over the **last axis** (`rank-1`); epsilon is a
**fixed `1e-5`**. No model in scope varies these, so building a string-attr parser
+ format-coupling now would be unearned machinery (Article 15.7) right after
ADR-028's offset-defect fix. A real attribute mechanism arrives only when a model
needs a non-default — deferred to the build that first requires it.

### D12 — Transcendental implementation methods (BUILD-4)

**exp**: argument reduction `x = k*ln(2) + r` via `round(x / ln(2))`, then a
minimax polynomial for `e^r` on the reduced range, scaled by `2^k`. The polynomial
is evaluated in soft-float `mul`/`add` (no hardware FP).

**sqrt**: bit-bang initial guess refined by Newton iterations in soft-float space
(`x_new = 0.5 * (x + n/x)` using soft-float `div`, `mul`, `add`).

**tanh**: `(e^(2x) - 1) / (e^(2x) + 1)` derived from the improved `exp`, with a
small-`x` Taylor branch to avoid catastrophic cancellation near zero.

**div**: 128-bit integer division of mantissas with round-to-nearest-even.

**fmax**: IEEE-754 signed maximum with NaN propagation.

All operate on `u32` IEEE-754 bit patterns. Disassembly gate: no XMM/SSE in
`ai::ops` objects (same as BUILD-3).

## Anti-scope (Layer 2 overall, enforced per build)

- No hardware FP in the inference path (D1) — no SSE/AVX, no FXSAVE machinery.
- No accelerator/`Device::Accel` execution (Phase 6).
- No inference scheduler / 8.2 class / multi-tenant serving (Phase 7).
- No new model formats; executes ADR-028 `.neom` graphs only.
- No governance/LogitPort.
- BUILD-3 specifically: **no transcendental ops** (deferred to BUILD-4).

## Consequences

- Phase 5's goal milestone — "a `.neom` model runs a CPU forward pass" — is
  reachable at **BUILD-3 close** on the MLP fixture, with the transformer op
  vocabulary following in BUILD-4.
- The soft-float decision keeps the scheduler untouched and the change auditable
  inside `kernel/src/ai/`, at the cost of speed — acceptable per 6.5, recoverable
  via the deferred hardware-FP path if PL chooses.
- The writable-tensor API finally gives BUILD-1's immutability flag teeth: weights
  reject mutation, activations permit it.
