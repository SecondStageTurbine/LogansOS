<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 5 BUILD-2 (track 2-tool) — `.neom` Authoring Tool → Prometheus

**Date:** 2026-05-31
**From:** PM/Cerebro
**To:** Prometheus (worker tree, feature branch)
**Status:** DISPATCHED — ready for BUILD
**Runs in parallel with:** track 2-kernel (Jarvis, `DISPATCH-2026-05-31-phase-5-build-2-kernel-model-object-jarvis.md`)
**Format contract:** `docs/ADR-028-neom-v1-format-and-model-object.md` — **the byte layout, alignment, validation rules, and opcode set are normative. Your output must be byte-exact to ADR-028 D1/D3; the kernel validator (track 2-kernel) is unforgiving.**
**Why this is userspace:** ADR-027 §D6 — `.neom` authoring/conversion is a userspace concern, never kernel.

---

## 1. Objective

A host-side Rust tool that **emits valid `.neom` v1 files** per ADR-028, plus the
**test fixtures** (valid + deliberately-malformed) that track 2-kernel's
`model_load_cascade` QEMU scenario loads. This is what makes `.neom` files exist
in the world; without it the kernel has nothing real to load.

## 2. Scope

**In scope:**
- A `.neom` v1 **writer**: a programmatic builder (add weight tensors with
  shape/dtype/data, add op-graph nodes with input/output value refs, declare
  model inputs/outputs) that serializes to spec-compliant bytes — correct
  offsets, **page-aligned weight blob, 64-byte-aligned weights**, little-endian,
  string heap, all per ADR-028 D1.
- A **self-validator** mirroring ADR-028 D2 that the tool runs on its own output
  before writing — encoder bugs caught on the host, not at kernel load.
- **Fixtures:** one realistic-but-small valid model (e.g. a 2-layer MLP: 2–3
  weight tensors, nodes `MatMul`/`Add`/`Relu`, one input + one output) and a set
  of **negative** files each violating exactly one rule (bad magic, bad version,
  non-zero flags, weight-len≠shape×dtype, offset overflow, out-of-range value
  ref, **cyclic graph**, unknown opcode). These feed track 2-kernel ISC-10/11.

**Anti-scope:**
- **No real ONNX/GGUF/SafeTensors import.** BUILD-2 ships the `.neom` writer and
  a *documented stub/roadmap* for importers; actual converters are later work.
- **No kernel code.** No inference. No model *execution* — you produce files,
  you don't run them.

## 3. Form factor & placement

- A host tool crate under `tools/` (std Rust, runs on the dev host / CI), e.g.
  `tools/neomc` (binary) + a small library module for the writer/validator so the
  host unit tests can exercise it directly. SPDX `// SPDX-License-Identifier:
  MPL-2.0` on every file. Wire it into the workspace as the other `tools/*`
  crates are (see `tools/xtask`, and BUILD-1's `tools/tensor-logic-tests`).
- Output: a CLI that writes a named `.neom` to a path, and a fixture-emit mode
  that drops the valid + negative corpus into a staging dir the kernel scenario’s
  disk-provisioning consumes (coordinate the path with track 2-kernel; default to
  a `target/`-relative staging dir documented in your close report).

## 4. Format-drift guard (read this)

Both tracks encode ADR-028 **independently** to preserve parallelism — that is
deliberate. The contract is the spec; the **drift detector** is track 2-kernel
loading *your* fixtures in its integration test. To make that succeed:
- Hard-code the magic (`NEOM`), `version=1`, header_size (96), struct sizes
  (TensorEntry 96, NodeEntry 48, IoEntry 80), dtype ABI (`F32=0..BOOL=6`), and
  opcode set (ADR-028 D3) **exactly** from the ADR.
- Match the dtype ABI numbering to the kernel's `ai/tensor_logic.rs`
  (`F32_ABI=0 .. BOOL_ABI=6`) — same encoding, by ADR-028 D1.
- (Optional, encouraged, not required for BUILD-2: factor the `#[repr(C)]` layout
  structs + constants into a tiny `no_std` `neom-format` crate both tracks could
  later share. Note it as a consolidation if you do; do not let it block.)

## 5. Acceptance gate (ISC)

- **ISC-1 — Writer.** Programmatic builder → byte-exact ADR-028 D1 layout:
  header, tensor/node/io tables, string heap, page-aligned weight blob,
  64-byte-aligned weights, little-endian throughout. `total_size` == file length.
- **ISC-2 — Self-validator.** Implements ADR-028 D2 over a byte buffer; the tool
  refuses to emit (non-zero exit, clear error) a file that fails. Returns
  `Result`, never panics on bad input.
- **ISC-3 — Valid fixture.** Emits the small MLP model described in §2; loads
  clean in the tool's own validator; intended to load clean in the kernel.
- **ISC-4 — Negative corpus.** One file per violation in §2, each failing exactly
  the intended rule (and *passing* every other rule, so the kernel's rejection is
  attributable to the right cause).
- **ISC-5 — Host unit tests.** Round-trip (build → serialize → re-parse →
  fields match); alignment assertions (weight blob % 4096 == 0, each weight % 64
  == 0); each negative case is rejected by the self-validator for the right
  reason.
- **ISC-6 — CLI.** `neomc write <spec> <out.neom>` and a fixture-emit mode;
  documented usage in `--help` and the close report.
- **ISC-7 — Standards + lean.** rustfmt; clippy `-D warnings`; SPDX headers; no
  `unwrap`/`expect` on user/file input paths (return `Result`); `/simplify`
  before BUILD. Anti-scope (no real importers, no kernel code, no inference) holds.

## 6. Coordination with track 2-kernel

- Jarvis builds his scenario against a minimal self-built fixture first, then
  switches to **your** fixtures for the drift-detecting integration test. Agree on
  the staging path + filenames early (a one-line note in each close report is
  enough). Neither track blocks the other to *start*; they meet at the
  integration test.

## 7. Workflow

Branch off current `main` (e.g. `prometheus/phase-5-build-2-neom-tool`).
`/simplify` gate before BUILD. Single squashed commit; close report
(`Plans/CLOSE-REPORT-...`) listing ISC pass/fail, the fixture corpus + staging
path, and the importer roadmap stub. Cerebro reviews + merges to `main`.
