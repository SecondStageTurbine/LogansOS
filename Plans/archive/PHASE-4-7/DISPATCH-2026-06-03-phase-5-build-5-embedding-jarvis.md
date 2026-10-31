<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 5 BUILD-5 — Embedding (integer-index gather) → Jarvis

**Date:** 2026-06-03
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — e.g. `codex/phase-5-build-5-embedding-jarvis`)
**Status:** DISPATCHED — ready for BUILD
**Design contract:** `docs/ADR-029-phase-5-layer-2-cpu-inference-design.md` — **D9 (build slicing: Embedding is BUILD-5), D3 (match dispatch), D11 (no attr mechanism)** are normative.
**Format reference:** `docs/ADR-028` (.neom) — honor the authoritative worked byte example.
**Builds on:** BUILD-4 transformer ops (closed `Plans/CLOSE-REPORT-2026-06-03-phase-5-build-4-transformer-ops.md`). Current `main` tip when this was written: `094eb35`.

---

## 1. Objective

Implement the last Layer-2 op: **`Embedding` (OpCode 9)** — an **integer-index gather** of rows from an F32 weight table. This completes the Phase 5 op set (0–11). Milestone: an **end-to-end forward pass on a new `valid_embedding.neom` fixture**, output **bit-exact** against a hand-laid byte oracle, QEMU-verified at `NEO_SMP=4`.

Embedding is **not** a transcendental/elementwise op — it is the **first op that consumes a non-F32 input** (I32 indices). Per ADR-029 D9 it was split from BUILD-4 precisely because it is "a different dtype path (I32 indices into an F32 table)." That dtype path is the whole point of this build; treat it as the central correctness concern.

## 2. Decisions already locked (do not re-litigate)

- **Exact, not approximate.** Gather is a memory copy of table rows — there is no arithmetic and no epsilon. Test it **bit-exact** against a hand-computed byte oracle (BUILD-3 style, *not* the BUILD-4 dual-oracle — there are no transcendentals here). The CLAUDE.md byte-oracle rule applies: ship a hand-laid expected-bytes fixture, asserted independently of the kernel.
- **No attribute mechanism (D11).** No `padding_idx`, no `max_norm`, no scale. Plain gather. Do **not** add `.neom` attribute parsing or change the format (Article 15.7; ADR-028 was just stabilized — don't churn it).
- **Lean shape slice.** Support the shape the milestone needs and reject the rest cleanly:
  - **indices:** rank-1, dtype **I32**, shape `[seq]`.
  - **table (weights):** rank-2, dtype **F32**, shape `[vocab, dim]`.
  - **output:** rank-2, dtype **F32**, shape `[seq, dim]`; `out[i] = table[indices[i]]`.
  - Higher-rank index tensors are a future widening, not this build.
- **Op set stops here.** No new ops beyond Embedding.

## 3. OBSERVE map (verified by Cerebro 2026-06-03)

### the dtype-path wrinkle (read first)
Every existing op kernel goes through `require_f32_view` (`kernel/src/ai/exec.rs:201`) and `require_f32` in `ops/mod.rs`. **Embedding must NOT apply `require_f32_view` to the indices input** — the indices are I32. The executor arm validates each input's dtype *individually* (indices = I32, table = F32) and the kernel re-checks. The `Dtype` enum already has `I32` (`kernel/src/ai/tensor_logic.rs:17`) with `size_bytes()` (`:61`), and the `.neom`→kernel dtype map already covers it (`exec.rs:283-289`, `NeomDtype::I32 => Dtype::I32`). So the plumbing exists; you are wiring an I32 *consumer* for the first time.

### op kernel — `kernel/src/ai/ops/mod.rs`
- Convention: binary kernels take two `TensorView`s (see `matmul`, `ops/mod.rs:78`). Add:
  `pub fn embedding(indices: &TensorView<'_>, table: &TensorView<'_>, out: &mut [u8]) -> Result<(), OpError>`.
- Read I32 index values from `indices.bytes` (little-endian `i32`), read F32 rows from `table.bytes` via the existing `read_bits`/byte helpers, write the gathered row into `out`. Row-major: `table[idx]` is `dim` consecutive f32 starting at `idx*dim`.
- Validate: `indices` rank-1 & I32; `table` rank-2 & F32; output length == `seq*dim*4`. Use the existing `OpError::{Shape,Dtype}` (and add a variant if you need a distinct "index out of range" — see safety below).

### executor dispatch — `kernel/src/ai/exec.rs`
- Add `const OP_EMBEDDING: u32 = 9;` (`exec.rs:14-19` op-const block).
- Add an `OP_EMBEDDING =>` arm to `execute_node()` (the `match`, around `exec.rs:107-170`). Two inputs (indices=input[0], table=input[1]); validate arities and **per-input dtypes** (do not blanket-`require_f32_view`); `allocate_result(out_shape, Dtype::F32, |out| ops::embedding(&indices, &table, out))`. The `_ => UnsupportedOp` catch-all (`exec.rs:170`) stops rejecting opcode 9 once the arm exists.
- Output shape is `[seq, dim]` derived from `indices.dim(0)` and `table.dim(1)`.

### format / validator — NO CHANGE
`tools/neomc/src/format.rs` already has `Embedding = 9` + `from_abi` (`:78,:95`); the kernel validator accepts opcode 9 format-wise. **Do not touch the format or validator** — only executor dispatch + kernel.

### fixture + scenario
- `valid_embedding.neom` authored by `neomc` (mirror the `valid_transformer_block.neom` / `valid_mlp.neom` fixture-build path in `tools/xtask/src/main.rs` — search `prepare_valid_*_fixture` and the NeoFS disk staging).
- Register an `infer_embedding_forward` QEMU scenario alongside `infer_mlp_forward` / `infer_transformer_forward` (`tools/xtask/src/main.rs` scenario table + `userland/neo-init/src/main.rs` runner: load `/neofs/model.neom` via `SYS_MODEL_LOAD`, create the I32 index input via `SYS_TENSOR_CREATE`, `SYS_INFER_RUN`, check output bytes).

## 4. Safety — bounds-check every index (correctness-critical)

An out-of-range index makes the gather read **out-of-bounds kernel memory** (`table[idx]` with `idx >= vocab` or `idx < 0`). This is the one genuinely dangerous part of the op. The kernel **must** bounds-check **every** index against `vocab = table.dim(0)` and reject (`OpError`, mapped to `EINVAL`) on any `idx < 0 || idx >= vocab` — *before* the read. A negative/oob index must never produce a read; it returns an error. Add an explicit negative test for it (see ISC-3).

## 5. Implementation Success Criteria

- **ISC-1 — kernel.** `ops::embedding` gathers rank-1 I32 indices into a rank-2 F32 table → rank-2 F32 output; row-major; uses existing byte helpers; no XMM/hardware-FP introduced (gather is integer/copy, but keep the no-XMM disassembly gate clean).
- **ISC-2 — executor dispatch + I32 path.** `OP_EMBEDDING` arm added; per-input dtype validation (I32 indices, F32 table) *without* blanket `require_f32_view`; output shape `[seq,dim]`.
- **ISC-3 — bounds safety + negative tests.** Every index bounds-checked against `vocab`; oob/negative index → `EINVAL`, no OOB read. Negative tests: oob index rejected; wrong index dtype (e.g. F32 indices) rejected; non-rank-1 indices or non-rank-2 table rejected.
- **ISC-4 — host tests (bit-exact oracle).** `tools/ai-ops-tests`: gather correctness asserted **bit-exact** against a **hand-laid byte oracle** (independent of the kernel — CLAUDE.md byte-oracle rule), plus the ISC-3 negative cases.
- **ISC-5 — fixture.** `valid_embedding.neom` generated by `neomc`.
- **ISC-6 — QEMU scenario** `infer_embedding_forward` at `NEO_SMP=4`: load from NeoFS, `SYS_INFER_RUN`, output bytes match the oracle; exits debug-SUCCESS.
- **ISC-7 — standards + lean + gate.** rustfmt; SAFETY comments on any new unsafe; no post-init heap growth surprises; no attribute mechanism; no format change.

## 6. Close gate

- `git fetch origin && git rebase origin/main` before close (rebase onto current `main`), re-run the gate after the rebase.
- `infer_embedding_forward` **exits debug SUCCESS** at `NEO_SMP=4`, and the full `cargo neo-test` suite passes.
- Host-checks green (`cargo fmt --all --check`, `cargo test --workspace --exclude neoplutarch-kernel`, `cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings`).
- The **hand-laid byte oracle** for the gather is in the test suite (not just a writer/reader round-trip).
- The **bounds-check negative test** (oob index → EINVAL, no OOB read) is present and passing.
- Single squashed commit; **push to the feature branch, NOT `main`** — the PM seat merges via `scripts/pm-merge-check.sh` + `--ff-only` after review. Close report (`Plans/CLOSE-REPORT-…`) with ISC pass/fail, the byte-oracle values, the no-XMM evidence, and the bounds-safety disposition.

## 7. After this build

Embedding completes the Layer-2 op set (0–11) and Phase 5's op coverage. Once it merges green, the remaining Phase 5 items are **bare-metal verification of the inference path** and the **formal Phase 5 close** (PL decision, per the Phase 2/3 close precedent) — both tracked separately, not part of this dispatch.
