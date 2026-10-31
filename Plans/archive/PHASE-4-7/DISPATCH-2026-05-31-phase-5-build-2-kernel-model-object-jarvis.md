<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 5 BUILD-2 (track 2-kernel) — Model Object → Jarvis

**Date:** 2026-05-31
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch)
**Status:** DISPATCHED — ready for BUILD
**Runs in parallel with:** track 2-tool (Prometheus, `DISPATCH-2026-05-31-phase-5-build-2-neom-tool-prometheus.md`)
**Format contract:** `docs/ADR-028-neom-v1-format-and-model-object.md` — **the byte layout, validation rules, opcode set, and §D8 disposition are normative. Implement to the ADR, not to your own reading of Article 21.**
**Design substrate:** `docs/ADR-027-...` (§D6, §D8); BUILD-1 tensor primitive (merged `37075a3`).
**Syscall range:** AI = `0x100..0x17F` (Bible 6.1). Tensors own `0x100..0x103`; **models own `0x110..0x11F`.**

---

## 1. Objective

Implement the Article 21 model object in `kernel/src/ai/`: load a `.neom` v1 file
from NeoFS, **validate** it (Article 21.2 / ADR-028 D2), build **immutable weight
tensors** (snapshot) + validated op-graph metadata, expose a capability-gated
`Handle::Model`, and discharge §D8 staleness (model ops → `ESTALE` after the
backing file changes/disappears). **Validate ops; never execute them** — inference
is Layer 2 (BUILD-3), hard anti-scope here.

## 2. Decisions already locked (ADR-028 — do not re-litigate)

- **Snapshot load:** read the whole `.neom`, validate, copy each weight into an
  **immutable** `Tensor` (`mutable=0`) in the buddy-frame class, then **release
  the `OpenFile`**. The model holds no live file handle.
- **§D8 staleness:** `Model` retains `{ mount_id, inode generation }` from load;
  model syscalls re-validate generation → `ESTALE` on change/deletion/unmount.
  Models hold no `live_handles`, so they never `EBUSY`-block unmount. **Do NOT
  rewrite the global file-handle EBUSY→invalidate unmount path** — out of scope,
  unneeded (ADR-028 D5).
- **Syscalls:** `SYS_MODEL_LOAD=0x110`, `SYS_MODEL_QUERY=0x111`,
  `SYS_MODEL_DROP=0x112`. **No `SYS_MODEL_INFER`** (Layer 2).
- **Per-op shape derivation is deferred** to Layer 2 (ADR-028 D2.7). BUILD-2
  validates structure + weight integrity + acyclicity + dtype/op validity.

## 3. OBSERVE integration map (verified by Cerebro 2026-05-31)

### Parse-and-validate template — `kernel/src/loader/elf.rs`
- Mirror exactly: `#[repr(C)]` header structs (`Elf64Header` 63–91), bounds
  check **before** unsafe cast (`parse` 99–128), `checked_mul`/`checked_add` for
  table arrays (`program_headers` 134–155), per-region byte-range validation with
  `SAFETY` comments (segment load 206–238). Define a `NeomError` enum modeled on
  `ElfError` (43–61). Add a host-testable logic split: `kernel/src/ai/model_logic.rs`
  (pure parse/validate over `&[u8]`) + `kernel/src/ai/model.rs` (kernel object,
  storage, syscalls), mirroring BUILD-1's `tensor_logic.rs` / `tensor.rs`.

### Building weights — `kernel/src/ai/tensor.rs` (already kernel-callable)
- `pub fn Tensor::from_desc_and_bytes(desc: TensorCreateDesc, data: &[u8]) ->
  Result<Self, TensorError>` (60–79). Build each weight with `mutable=0`,
  `device=Cpu`, from the validated weight slice. Wrap `Arc<Tensor>`. No change to
  `tensor.rs` is required. Dtype ABI values live in `ai/tensor_logic.rs`
  (`F32_ABI=0 .. BOOL_ABI=6`); reuse them — the `.neom` `dtype` field uses the
  same encoding (ADR-028 D1).

### Kernel-space file read — `kernel/src/fs/mod.rs`
- `pub fn fs::open_root_temporary(path, flags, requested: FileRights) ->
  Result<Arc<OpenFile>, VfsError>` (817–861) — the only kernel open API.
- `OpenFile::read(&self, out: &mut [u8]) -> Result<usize, VfsError>` (236–248),
  advances the offset. **No get-size method exists** — read in a loop into a
  bounded `Vec<u8>` until a short/zero read (EOF). Enforce `MAX_NEOM_BYTES`
  (pick a sane cap, e.g. 64 MiB) → `E2BIG`/`ENOMEM` if exceeded. Capture the
  inode generation for provenance via `OpenFile::validate_current` / the inode
  `current_generation` path (fs/mod.rs 125–127, 194–203) **before** releasing.

### Capability handle — `kernel/src/sched/process.rs` (mirror `Handle::Tensor`)
- Add `Model { model: Arc<Model>, cap_id: CapId, rights: CapRights }` to the
  `Handle` enum (400–441). Add arms to `cap_id()` (444–453), `rights()`
  (456–466), `with_cap()` (475–521) — `Handle::Tensor` shows the exact shape.
  Add `tensor_rights`-style `pub mod model_rights { pub const USE: u8 = 1<<0; }`
  + `pub const fn full_model_rights() -> CapRights`. `cargo check` flags any
  other exhaustive `Handle` match.
- Cap install: `process::install_cap(&proc, handle, None)` (BUILD-1
  `sys_tensor_create_impl` 789–837 is the template). Drop mirrors
  `sys_tensor_drop_impl` (876–894), matching `Handle::Model`.

### §D8 staleness — reuse existing machinery
- `OpenFile::validate_current` (194–203) → `VfsError::Stale`; `VfsError::Stale =>
  ESTALE` (syscall.rs ~2012; `ESTALE=-116` errno.rs 63). Inode generation:
  `Inode::current_generation` (125–127), NeoFS `current_child_generation`
  (neofs/inode.rs 137–143). The cascade walk `revoke_handle` (sched/process.rs
  769–830) exists if you need derivation-cascade, but for snapshot models a
  per-op generation re-check is sufficient.

### Dispatch — `kernel/src/arch/x86_64/syscall.rs`
- Consts after the tensor block (~line 79); add a doc note that `0x110..0x11F` is
  the model sub-block of the AI range. Dispatch arms (~349, after tensor arms):
  - `SYS_MODEL_LOAD  => sys_model_load_impl(a0 as *const u8, a1 as usize, a2 as u32)` (path ptr, path len, flags)
  - `SYS_MODEL_QUERY => sys_model_query_impl(a0 as u32, a1 as *mut u8, a2 as usize)`
  - `SYS_MODEL_DROP  => sys_model_drop_impl(a0 as u32)`
- `validate_user_buf` / `copy_user_bytes` for the path arg (BUILD-1 pattern).

### Test harness — `kernel/src/test_harness.rs`, userland `neo-init`
- Feature `neo-test-phase5-model-load-cascade`, `SCENARIO =
  "model_load_cascade"`. NEO_TEST sentinel `NEO_TEST/1 event=PASS
  name=model_load_cascade ...`. The scenario needs a `.neom` blob on NeoFS — see
  §5 fixture note.

## 4. Acceptance gate (ISC)

- **ISC-1 — Modules.** `ai/model.rs` + `ai/model_logic.rs`, wired into `ai/mod.rs`,
  SPDX + `//!` docs.
- **ISC-2 — Parser.** `.neom` v1 per ADR-028 D1, ELF-loader discipline (`#[repr(C)]`,
  bounds-before-cast, `checked_*`, `SAFETY` on every unsafe). `NeomError` enum.
- **ISC-3 — Validation (ADR-028 D2).** Header, table bounds, tensor integrity
  (`weight_len == product(dims)*dtype.size_bytes()`, alignment, in-blob), op
  support (D3 set), ref validity, **acyclicity (topo-sort)**, dtype consistency.
  Any failure → no load.
- **ISC-4 — Model object (ADR-028 D4).** `Vec<Arc<Tensor>>` immutable weights,
  graph metadata, `ModelProvenance`. `OpenFile` released after load.
- **ISC-5 — Kernel load path.** Whole-file read via `open_root_temporary` + read
  loop into bounded `Vec`; `MAX_NEOM_BYTES` cap.
- **ISC-6 — `Handle::Model`** mirrors `Handle::Tensor`; `full_model_rights`;
  capability-gated; drop frees (last `Arc<Model>` → weights drop → buddy frames
  freed).
- **ISC-7 — Syscalls** `0x110..0x112` registered + dispatched + range-documented.
  **No `SYS_MODEL_INFER`.**
- **ISC-8 — §D8 staleness.** Model retains `{mount_id, generation}`;
  `SYS_MODEL_QUERY` (and drop’s lookup) re-validate → `ESTALE` after the backing
  file's generation advances or its mount is gone. Model holds no `live_handles`
  (verify: a loaded model does not make `sys_umount` return `EBUSY`).
- **ISC-9 — Errno.** Malformed/validation-fail → `EINVAL`; oversized →
  `E2BIG`/`ENOMEM`; missing path → `ENOENT`; bad handle → `EBADF`; stale →
  `ESTALE`.
- **ISC-10 — Host unit tests** (`tools/`-style host crate, mirror
  `tensor-logic-tests`): parse a valid header; reject bad magic / bad version /
  overflowing offset / weight-len mismatch / unknown op / cyclic graph.
- **ISC-11 — QEMU scenario** `model_load_cascade` at `NEO_SMP=4`: stage a `.neom`
  on NeoFS → `LOAD` → `QUERY` (counts/shapes/dtypes match) → `DROP` (storage
  freed); then **stale path**: mutate the backing file (advance generation) or
  unmount → `QUERY` returns `ESTALE`; confirm a loaded model did **not** block the
  unmount. Sentinel PASS. Full `neo-test all --no-meta` at `NEO_SMP=4` stays green.
- **ISC-12 — Standards + lean.** rustfmt; clippy `-D warnings`; `SAFETY` on every
  unsafe; no post-init `unwrap`/`expect`; SPDX headers; `/simplify` before BUILD.
  Anti-scope (no op execution, no `SYS_MODEL_INFER`, no accel, no kernel-side
  `.neom` importers, no global unmount-semantics rewrite) — any of these fails the gate.

## 5. Fixture coordination (parallel-track note)

Track 2-tool (Prometheus) produces the canonical `.neom` authoring tool plus
valid + negative test fixtures. **To stay unblocked**, build your scenario
against a **minimal self-contained fixture** (a small hand-assembled byte blob or
a tiny `build.rs`/host helper emitting one valid `.neom` + the negative cases you
need for ISC-10/ISC-11). When track 2-tool lands, the integration test should also
load a **tool-produced** `.neom` — that cross-check is the format-drift detector
between the two tracks. Do not block BUILD on Prometheus.

## 6. Workflow

Branch off current `main` (e.g. `codex/phase-5-build-2-kernel-model-object-jarvis`).
`/simplify` gate before BUILD. Single squashed commit; close report
(`Plans/CLOSE-REPORT-...`) noting ISC pass/fail, the Layer-1 scoping of 21.2
(per-op shape derivation deferred), and the §D8 disposition with evidence
(ESTALE-after-change + unmount-not-blocked). Cerebro reviews + merges to `main`.
