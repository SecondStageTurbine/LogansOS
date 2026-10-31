<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-028: `.neom` v1 Model Format & Phase 5 BUILD-2 Model Object

| | |
|---|---|
| **Status** | ✅ Accepted (PL sign-off 2026-05-31) |
| **Date** | 2026-05-31 |
| **Authority** | PM + PL |
| **Supersedes** | None |
| **Superseded by** | - |
| **Related** | Bible Articles 6.1–6.3 (AI subsystem, native format only), 7.5 (tensor memory class), 12.1 (capabilities), 19.2 (extension by accretion), 20 (Tensor), 21 (Model Object), 21.2 (load-time validation); ADR-027 (Phase 5 design substrate, §D6 native-format-only, §D8 cascade revocation); BUILD-1 tensor primitive (merged `37075a3`) |

---

## Context

Phase 5 BUILD-1 landed the tensor primitive. BUILD-2 (Layer 1) is the **model
object**: Article 21's `(graph, weights, metadata)`, loaded from a `.neom` file
and validated by the kernel (21.1, 21.2). Article 21 specifies *what* a model is
but not the file's byte layout — so this ADR defines the **`.neom` v1 format**.

This format is the single contract that gates **two parallel BUILD-2 tracks**:
the kernel model object (Jarvis) and the userspace `.neom` authoring tool
(Prometheus, per ADR-027 §D6: authoring/conversion is userspace, never kernel).
Getting it right matters double — a format error force-reworks both tracks.

The OBSERVE pass (2026-05-31) established the integration substrate: the ELF
loader (`kernel/src/loader/elf.rs`) is the parse-and-validate template
(`#[repr(C)]` headers, `checked_mul`/`checked_add` bounds, bounds-checked unsafe
casts); `Tensor::from_desc_and_bytes` is already kernel-callable so weights need
no tensor-API change; the cascade/staleness machinery already exists
(`revoke_handle` walk in `sched/process.rs`, `OpenFile::validate_current` →
`VfsError::Stale` → `ESTALE` via NeoFS inode generation).

## Decision 1 — `.neom` v1 byte layout

All multi-byte fields **little-endian** (matching x86_64 and the ELF precedent).
**All offsets are absolute from the start of the file** — including `weight_off`
in each TensorEntry and the value-ref/string offsets. There are **no
blob-relative offsets** anywhere in the format: do not add `weight_blob_off` to
`weight_off` when locating a weight; `weight_off` already points at the file
byte. (See the §"Defect note" at the end — an earlier ambiguous wording of this
clause caused BUILD-2 to implement blob-relative offsets, corrected in BUILD-3.)
All tables 8-byte aligned; the weight blob is **4096-byte (page) aligned** and
each weight within it is **64-byte aligned** — so Layer 2 can map weights
read-only without a re-copy (Article 21.3) even though BUILD-2 snapshots them.
Because the blob is page-aligned and `weight_off` is absolute, each weight's
absolute offset is `weight_blob_off + (its position within the blob)`, computed
**by the writer**; the reader treats `weight_off` as the final absolute byte.

### Header v1 (`#[repr(C)]`, 96 bytes)

```
offset size field
0x00   4    magic            = b"NEOM"
0x04   4    version          = 1
0x08   4    header_size      = 96 (sizeof header; validation anchor)
0x0C   4    flags            = 0 (reserved; must be 0 in v1)
0x10   4    tensor_count     (weight tensors)
0x14   4    node_count       (op-graph nodes)
0x18   4    input_count      (graph input values)
0x1C   4    output_count     (graph output values)
0x20   8    tensor_table_off (-> [TensorEntry; tensor_count])
0x28   8    node_table_off   (-> [NodeEntry;   node_count])
0x30   8    io_table_off     (-> [IoEntry; input_count + output_count])
0x38   8    string_off       (string heap base; names are (off,len) into it)
0x40   8    string_len
0x48   8    weight_blob_off  (page-aligned; base of raw weight bytes)
0x50   8    weight_blob_len
0x58   8    total_size       (== file length; validation anchor)
```

### Header v2 — config-metadata extension (+12 bytes, 108 bytes total)

**Amendment 2026-06-22.** The v2 header is an **additive extension** of v1: the
first 96 bytes are identical. Three scalar config-metadata fields are appended at
offsets `0x60..0x6C`. The `version` field is `2` and `header_size` is `108`.
Existing v1 readers reject v2 files (they check `version == 1`). Existing v1
files remain valid — the validator accepts both versions.

The 3 fields carry config scalars **not derivable from tensor shapes** that the
kernel's forward-path loader needs to construct `ModelConfig` from the `.neom`
alone (no recompilation for a different variant):

```
offset size field         (v2 extension, after v1 header)
0x60   4    layer_count   (u32 LE; transformer block count; e.g. 24 for Qwen2.5-0.5B)
0x64   4    rope_theta    (f32 LE; RoPE frequency base; e.g. 10000.0 or scaled value)
0x68   4    rms_norm_eps  (f32 LE; RMS norm epsilon; e.g. 1e-6)
```

The values are sourced from the GGUF metadata at import time:
- `layer_count` ← `qwen2.block_count` (u32)
- `rope_theta` ← `qwen2.rope.freq_base` (f32)
- `rms_norm_eps` ← `qwen2.attention.layer_norm_rms_epsilon` (f32)

If **any** of the 3 keys is absent in the source GGUF, the import fails with a
clear error — a wrong `rope_theta` is a silently-wrong model. The writer emits
all three together or none; partial sets are rejected by construction
(`NeomBuilder` requires all three to be set for v2 emission).

### TensorEntry (`#[repr(C)]`, 96 bytes) — a weight tensor descriptor

### TensorEntry (`#[repr(C)]`, 96 bytes) — a weight tensor descriptor

```
0x00   4    dtype       (Dtype ABI value, see ai/tensor_logic.rs: F32=0..BOOL=6)
0x04   4    rank        (≤ 8)
0x08   64   dims        ([u64; 8])
0x48   8    weight_off  (ABSOLUTE file offset of this weight; 64-byte aligned;
                         lies within [weight_blob_off, weight_blob_off+len))
0x50   8    weight_len  (== product(dims[..rank]) * dtype.size_bytes())
0x58   4    name_off    (into string heap)
0x5C   4    name_len
```

### NodeEntry (`#[repr(C)]`, 48 bytes) — an op-graph node (validated, not executed in Layer 1)

```
0x00   4    op           (OpCode, see Decision 3)
0x04   4    input_count
0x08   4    output_count
0x0C   4    inputs_off   (into the u32 value-ref array region of the node table)
0x10   4    outputs_off
0x14   4    attr_off     (into string heap; opaque op attributes, v1)
0x18   4    attr_len
0x1C   4    name_off
0x20   4    name_len
0x24   12   reserved     (must be 0)
```

Value refs are `u32` indices into a unified value space: `[0, tensor_count)`
are weight tensors; `[tensor_count, ...)` are intermediate/IO graph values. The
ref array region follows the `NodeEntry` array within the node table.

### IoEntry (`#[repr(C)]`, 80 bytes) — declares input/output value shapes

```
0x00   4    value_index  (the graph value this declares)
0x04   4    dtype
0x08   4    rank
0x0C   4    is_output    (0 = input, 1 = output)
0x10   64   dims         ([u64; 8])
```

## Decision 2 — Load-time validation (Article 21.2)

A `.neom` that fails *any* check does not load (`SYS_MODEL_LOAD` → error). The
kernel validates, in order, with `checked_*` arithmetic throughout (ELF pattern):

1. **Header:** `magic == "NEOM"`, `version == 1`, `flags == 0`,
   `header_size == 96`, `total_size == file length`.
2. **Table bounds:** every `*_off + count*sizeof` is within file; tables don't
   overlap the header or run past `total_size`; weight blob is page-aligned and
   within file.
3. **Tensor integrity:** each `dtype` valid; `rank ≤ 8`; dims non-zero;
   `weight_len == product(dims) * dtype.size_bytes()`; `weight_off` 64-aligned
   and **absolute** (file-start); `[weight_off, weight_off+weight_len)` lies
   within `[weight_blob_off, weight_blob_off+weight_blob_len)`.
4. **Op support (21.2):** every node `op` is in the supported `OpCode` set
   (Decision 3) — "op support on at least one available device."
5. **Ref validity:** every input/output value ref is in range; string/attr
   offsets in the heap.
6. **No cycles (21.2):** the node graph is a DAG — topological sort succeeds.
7. **Dtype/shape consistency (21.2, Layer-1 scope):** weight tensor dims match
   their declared data length (checked in 3); declared IO dtypes valid;
   value-ref dtypes self-consistent. **Per-op output-shape derivation is
   deferred to Layer 2** (it requires op execution semantics); BUILD-2 validates
   structural + weight + acyclicity consistency. Recorded as a scoped reading of
   21.2, not a waiver.

## Decision 3 — Initial OpCode set (extensible by accretion, Article 19.2)

The shared op vocabulary the tool emits and the kernel validates. v1 set covers
MLP/transformer inference:

```
0 = Identity   1 = Add        2 = Mul        3 = MatMul
4 = Relu       5 = Gelu       6 = Softmax    7 = LayerNorm
8 = RmsNorm    9 = Embedding  10 = Reshape   11 = Transpose
```

Unknown opcodes fail validation (check 4). New ops are added by extending the
enum (accretion); the format version bumps only on a layout change, not an op
addition.

## Decision 4 — The kernel Model object

```text
Model { tensors: Vec<Arc<Tensor>>,   // immutable weights, snapshotted at load
        graph:   ModelGraph,          // validated nodes + IO descriptors (metadata)
        meta:    ModelMeta,           // name, counts, source provenance
        source:  ModelProvenance }    // mount_id + inode generation snapshot (§D8)
```

- **Snapshot load, not lazy/mmap.** `SYS_MODEL_LOAD` reads the whole `.neom`
  (kernel buffer, bounded by a `MAX_NEOM_BYTES` cap → `ENOMEM`/`E2BIG` over),
  validates it, and builds each weight as an **immutable** `Tensor`
  (`Tensor::from_desc_and_bytes`, `mutable = 0`) in the buddy-frame tensor class
  (Article 7.5). The backing `OpenFile` is **released after load** — the model
  does not hold a live file handle.
- **`Handle::Model`** mirrors `Handle::Tensor` exactly (enum variant + `cap_id`/
  `rights`/`with_cap` arms; `full_model_rights()`). Capability-gated (12.1).
- **Sharing (21.3):** weights are `Arc<Tensor>`; sharing a model across processes
  shares the weight storage. BUILD-2 supports load + query + drop; broad
  cross-process model granting rides the existing cap-grant path, no new
  mechanism.

## Decision 5 — §D8 cascade revocation, realized for Layer 1

ADR-027 §D8 has two halves. BUILD-2 resolves them as follows:

- **(i) Staleness → ESTALE (implemented).** `Model` retains a `ModelProvenance`
  = `{ mount_id, inode generation snapshot }` captured at load. Model syscalls
  re-validate the source generation (the existing `current_generation` path); if
  the backing inode was destroyed, replaced, or regenerated — including because
  its mount is gone — model operations return **`ESTALE`**. This reuses the
  NeoFS generation machinery; no new staleness mechanism.
- **(ii) Unmount must not be blocked by models (achieved by construction).**
  Because the model **releases its `OpenFile` after the snapshot load**, it holds
  **no `live_handles` count** against the mount — so a loaded model never
  `EBUSY`-blocks an unmount. After unmount, model ops re-validate and return
  `ESTALE` per (i). This delivers §D8's intent (unmount not blocked; stale models
  fail closed) **without** the broad, risky rewrite of regular-file
  `live_handles` unmount semantics from EBUSY to active cap-invalidation — that
  global change is **explicitly out of BUILD-2 scope** and is not needed, since
  models don't hold live handles. If a future build introduces lazily-mapped
  (non-snapshot) model weights, half (ii) must be revisited then.

## Anti-scope (BUILD-2)

- **No inference execution / op kernels** — `SYS_MODEL_INFER` is *not* added;
  ops are validated, never run. That is Layer 2 (BUILD-3).
- **No `.neom` importers in the kernel** — ONNX/GGUF/SafeTensors → `.neom`
  conversion is the userspace tool's job (track 2-tool).
- **No per-op shape derivation** (deferred to Layer 2, Decision 2).
- **No global EBUSY→invalidate rewrite** of file-handle unmount (Decision 5).
- **No accelerator paths** (`Device::Cpu` only; Phase 6).
- **No governance/LogitPort.**

## Consequences

- A single, precise, little-endian format pins both BUILD-2 tracks; the tool and
  the kernel can be built and tested independently against this spec.
- The model loads as a validated, capability-gated, refcounted kernel object with
  immutable snapshotted weights — the substrate Layer 2 inference will execute.
- §D8 is discharged for Layer 1 with zero new mechanism and no destabilizing
  change to existing mount semantics, by making models hold no live file handle.
- Page/64-byte weight alignment future-proofs Layer 2 read-only weight mapping.

## Authoritative worked example (offset reference)

This is the **canonical disambiguation**: a minimal 1-tensor `.neom` with one
F32 scalar weight `[1.0]`, no nodes, one input + one output. Hand-laid-out from
the spec; any writer/reader must agree with these byte offsets. (Sizes:
header 96, TensorEntry 96, NodeEntry 48, IoEntry 80.)

```
file off  field
0x0000    header (96 B): magic "NEOM", version=1, header_size=96, flags=0,
          tensor_count=1, node_count=0, input_count=1, output_count=1,
          tensor_table_off=0x60, node_table_off=0xC0, io_table_off=0xC0,
          string_off=0x160, string_len=0, weight_blob_off=0x1000,
          weight_blob_len=4, total_size=0x1004
0x0060    TensorEntry[0]: dtype=0(F32) rank=1 dims=[1,0..]
          weight_off=0x1000  <-- ABSOLUTE: the F32 lives at file byte 0x1000,
                                 NOT at 0x1000+0x1000. weight_len=4.
0x00C0    io_table: IoEntry[0]=input(value 1), IoEntry[1]=output(value 1)
0x1000    weight blob (page-aligned): the 4 bytes of f32 1.0 = 00 00 80 3F
```

The decisive rule, stated three ways so it cannot be misread:
- `weight_off` is the **final file byte offset** of the weight.
- The reader does **`file[weight_off ..]`**, never `file[weight_blob_off +
  weight_off ..]`.
- The writer computes `weight_off = weight_blob_off + position_within_blob` and
  stores that absolute value.

## Defect note (amendment 2026-06-01)

The original v1 text contained a contradiction: the global clause said "all
offsets are absolute from the start of the file," while the TensorEntry table
described `weight_off` as an "offset into weight blob" (relative). BUILD-2
(model object + neomc + neofs-mkfs) resolved the ambiguity as **blob-relative**
and was self-consistent — writer and reader agreed, so every BUILD-2 test
passed — but that did not match the absolute-offset clause. BUILD-3 re-read the
spec, implemented **absolute** offsets (the intended reading), and corrected the
kernel parser, `neomc`, the model-logic tests, and `neofs-mkfs`'s embedded model
to match. No `.neom` produced by BUILD-2 tooling escaped the repo, so there is no
on-disk-compatibility break; this note records the correction. The lesson — a
self-consistent writer+reader pair passes round-trip tests even on a wrong shared
reading — motivated the **independent-oracle rule** now standard for format/ABI
specs (see CLAUDE.md, "Format/ABI specs require an independent byte oracle").
ADR remains v1 (no on-disk layout change vs. the intended reading).

## v2 config-metadata extension (amendment 2026-06-22)

The v2 header extends v1 by 12 bytes at offsets `0x60..0x6C` to carry three
config scalars (`layer_count`, `rope_theta`, `rms_norm_eps`) not derivable from
tensor shapes. This is the **additive extension** described in Decision 1's
"Header v2" section above. The `.neom` writer emits v2 when all three config
fields are present (sourced from GGUF metadata); otherwise it emits v1 as before.

**Backward compatibility:**
- v1 files (`version=1`, `header_size=96`) remain valid and validate unchanged.
- v2 files (`version=2`, `header_size=108`) are accepted by the validator.
- The kernel's current header parse (which reads only the first 96 bytes) does
  not break on v2 — the new fields are past what it reads.
- Existing fixtures and tools that produce v1 continue to work without change.

**Consumer:** F2 (forward-path config-from-`.neom`) will read these 3 fields from
the v2 header to construct `ModelConfig` data-driven, replacing the hardcoded
`QWEN2_05B_*` constants in `forward_logic.rs`.
