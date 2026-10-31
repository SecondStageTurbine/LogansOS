<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: `.neom` Layer-Schedule Section — Draft Byte-Spec + Hand-Laid Oracle

| | |
|---|---|
| **Date** | 2026-07-05 (rev. 2 — bounce fix: header size, config_off, magic) |
| **Status** | DRAFT — B1 prep, not an ADR yet |
| **Related** | `THINK-2026-07-05-ot16-forward-graph-design.md` (ratified D1–D6), ADR-028 (`.neom` v1/v2 format) |
| **Lifted by** | B1 into numbered ADR when walker + parity code lands |

---

## 0. Normative principle (the ADR-028 lesson)

The worked examples in §4 are the normative core. Prose serves the example. If an ambiguity survives the byte-level hex layout, it's a spec bug.

---

## 1. Current format established from source (ISC-1)

Read from `tools/neomc/src/format.rs` and ADR-028:

**Version:** `NEOM_VERSION = 2` (v2 is current; v1 = 96-byte header, v2 = 108-byte header)

**Header v2 layout (108 bytes):**
```
offset  size  field
0x00    4     magic          = b"NEOM"
0x04    4     version        = 2
0x08    4     header_size    = 108
0x0C    4     flags          = 0 (reserved)
0x10    4     tensor_count
0x14    4     node_count
0x18    4     input_count
0x1C    4     output_count
0x20    8     tensor_table_off
0x28    8     node_table_off
0x30    8     io_table_off
0x38    8     string_off
0x40    8     string_len
0x48    8     weight_blob_off
0x50    8     weight_blob_len
0x58    8     total_size
0x60    4     layer_count    (v2 extension)
0x64    4     rope_theta     (v2 extension, f32)
0x68    4     rms_norm_eps   (v2 extension, f32)
```

**Endianness:** All multi-byte fields are **little-endian**.

**Alignment rules:** Tables are 8-byte aligned. Weight blob is 4096-byte (page) aligned. Each weight within the blob is 64-byte aligned. `weight_off` is an absolute file offset.

**DRIFT CHECK vs THINK assumptions:** The ratified THINK assumed v2 header with `layer_count` at `0x60`. Source confirms: correct, no drift. The schedule section will be additive to v2 — a v2 `.neom` with no schedule section loads exactly as today.

---

## 2. Schedule section byte layout (ISC-2)

### 2.1 Discovery

The schedule section is discovered from the v3 header extension. When `version == 3`, `header_size == 120` (108 + 12 bytes of schedule pointer). The 12-byte extension:

```
offset  size  field
0x6C    8     schedule_off   (absolute file offset of ScheduleHeader, or 0 if absent)
0x74    4     schedule_len   (byte length of entire schedule section, or 0 if absent)
```

If `schedule_off == 0`, the kernel uses the hardcoded Qwen2 forward (D3 kill-switch OFF path). The file is otherwise a valid v3 `.neom` — the schedule is optional.

**Version bump:** v2 → v3 adds only these 12 bytes past the existing header. First 108 bytes are identical. v1/v2 files remain valid. A v3 reader accepts v1/v2 (treats missing schedule as hardcoded forward).

### 2.2 ScheduleHeader (32 bytes)

```
offset  size  field
0x00    2     magic          = 0x53, 0x43 (raw on-disk bytes: 'S' then 'C')
0x02    2     version        = 1 (schedule section version, independent of .neom version)
0x04    4     layer_count    (must match header.layer_count)
0x08    4     entry_count    (== layer_count; one ScheduleEntry per layer)
0x0C    8     entry_array_off  (absolute file offset of [ScheduleEntry; entry_count])
0x14    8     config_data_off  (absolute file offset of tagged-union config payloads)
0x1C    4     reserved       (must be 0)
```

Size verification: 2 + 2 + 4 + 4 + 8 + 8 + 4 = **32 bytes**. The struct is tightly packed with no internal padding. When placed at an 8-byte-aligned file offset, the trailing edge is also 8-byte aligned (32 % 8 == 0), so the entry array requires no inter-region padding.

### 2.3 ScheduleEntry (8 bytes each)

One entry per layer, in layer order (layer 0 first).

```
offset  size  field
0x00    1     kind           (LayerKind discriminant — see §2.4)
0x01    1     config_tag     (matches kind; validates tagged-union)
0x02    2     config_off     (region-relative u16: offset FROM config_data_off)
0x04    2     config_len     (byte length of this layer's config payload)
0x06    2     reserved       (must be 0)
```

**config_off convention:** The value is a region-relative offset. The absolute file offset of a layer's config payload is `ScheduleHeader.config_data_off + ScheduleEntry.config_off`. This keeps `config_off` as a compact u16 (max 65 535 bytes of config data per section — sufficient for any practical model) while avoiding the overflow that would occur with absolute file offsets in multi-MB `.neom` files. For Qwen2 layers with no config, `config_off == 0` and `config_len == 0`.

### 2.4 LayerKind enum (stable discriminants)

```
value  name                 meaning
0x00   QWEN2_ATTENTION_FFN  Standard Qwen2/MoE transformer block (MSA + SwiGLU FFN)
0x01   GATED_DELTANET_LINEAR  Gated DeltaNet recurrence + linear projection (Qwen 3.5 linear-attention)
0x02..0x0F  RESERVED          (must be rejected by validator if encountered)
0x10..0xFF  EXTENSIBLE        (future kinds; validator accepts if in supported set)
```

The discriminant is a `u8`. Values 0x02–0x0F are explicitly reserved to catch encoding errors. Unknown values > 0x0F are rejected until the kernel adds support — extensibility requires an explicit opt-in, not silent acceptance.

### 2.5 Tagged-union LayerConfig payloads

Each layer's config payload lives in the `config_data` region. The absolute file offset is `config_data_off + config_off` (region-relative).

**Qwen2LayerConfig (`kind == 0x00`, 0 bytes — empty payload):**
The Qwen2 layer has no per-layer config beyond what's in the global header (`rope_theta`, `rms_norm_eps`) and tensor shapes. `config_len == 0`, `config_off == 0`.

**GDNLayerConfig (`kind == 0x01`, 20 bytes):**
```
offset  size  field
0x00    4     state_size       (u32; recurrent state dimension; e.g. 16)
0x04    4     conv_kernel      (u32; convolution kernel size; e.g. 4)
0x08    4     n_heads          (u32; attention head count for this layer)
0x0C    4     key_head_dim     (u32; per-head key dimension for linear attention)
0x10    4     value_head_dim   (u32; per-head value dimension for linear attention)
```

Total: 20 bytes. No padding needed — all u32 fields, naturally 4-byte aligned.

### 2.6 Alignment and padding rules

- `schedule_off` is 8-byte aligned (`schedule_off % 8 == 0`).
- ScheduleHeader is 32 bytes; when placed at an 8-byte-aligned offset, its trailing edge is also 8-byte aligned (32 % 8 == 0). **No post-header padding is needed.**
- The `ScheduleEntry` array starts at `entry_array_off` (which equals `schedule_off + 32`). Each entry is 8 bytes; the array is naturally 8-byte aligned throughout.
- The config data region starts immediately after the entry array: `config_data_off = entry_array_off + 8 * entry_count`. Since entries are 8 bytes and the array starts 8-aligned, the config region start is always 8-aligned (and therefore 4-aligned).
- Config payloads are packed contiguously with no inter-config gaps. Each GDN payload is 20 bytes (not a multiple of 8), but since they're packed sequentially within the config region, no alignment padding between configs is needed — the reader uses `config_off` to find each payload's start.
- The schedule section occupies `[schedule_off, schedule_off + schedule_len)`. No part overlaps other file regions.

**Section layout (fixed order, no gaps):**
```
ScheduleHeader (32 bytes)
  |
  v
ScheduleEntry array (8 × N bytes)
  |
  v
Config data region (20 × G bytes, packed contiguously)
```

### 2.7 Field bounds

| Field | Constraint |
|-------|-----------|
| `ScheduleHeader.magic` | bytes `0x53, 0x43` ('S', 'C') in that on-disk order |
| `ScheduleHeader.version` | == 1 |
| `ScheduleHeader.layer_count` | == `header.layer_count`; 1..2048 |
| `ScheduleHeader.entry_count` | == `ScheduleHeader.layer_count` |
| `ScheduleEntry.kind` | In `{0x00, 0x01}` (currently supported set) |
| `ScheduleEntry.config_tag` | == `ScheduleEntry.kind` |
| `ScheduleEntry.config_len` | 0 for QWEN2; 20 for GATED_DELTANET_LINEAR |
| `ScheduleEntry.config_off` | If `config_len > 0`: region-relative offset satisfying `config_data_off + config_off + config_len <= schedule_off + schedule_len` |
| `GDNLayerConfig.state_size` | 1..65536 |
| `GDNLayerConfig.conv_kernel` | 1..64 |
| `ScheduleEntry.reserved` | == 0 |
| `ScheduleHeader.reserved` | == 0 |

---

## 3. Validation invariants mapped to exact bytes (ISC-4)

The ratified THINK's 7 load-time validation invariants, mapped to the bytes they check:

### Invariant 1: Version gate
- **Bytes:** `header.version` (offset 0x04, 4 bytes); `ScheduleHeader.version` (schedule+0x02, 2 bytes)
- **Check:** `header.version >= 3` for schedule-present files; `ScheduleHeader.version == 1`
- **Reject:** `NeomError::InvalidVersion(v)` if `header.version < 3` and `schedule_off != 0`; `NeomError::ScheduleInvalidVersion(v)` if schedule version != 1

### Invariant 2: Layer count bounds
- **Bytes:** `ScheduleHeader.layer_count` (schedule+0x04, 4 bytes)
- **Check:** `1 <= layer_count <= 2048`; `layer_count == header.layer_count`
- **Reject:** `NeomError::ScheduleLayerCountMismatch { schedule, header }`

### Invariant 3: Known kinds
- **Bytes:** Each `ScheduleEntry.kind` (entry+0x00, 1 byte)
- **Check:** `kind in {0x00, 0x01}` (current supported set)
- **Reject:** `NeomError::ScheduleUnknownKind(kind)` at first unknown discriminant

### Invariant 4: Config consistency
- **Bytes:** `ScheduleEntry.config_tag` vs `kind`; `config_len` vs expected for kind
- **Check:** `config_tag == kind`; if `kind == 0x00` then `config_len == 0`; if `kind == 0x01` then `config_len == 20`
- **Reject:** `NeomError::ScheduleConfigTagMismatch`; `NeomError::ScheduleConfigLenMismatch { kind, expected, actual }`

### Invariant 5: Weight tensor presence
- **Bytes:** `header.tensor_count`; tensor table entries' `name_off`/`name_len` (string heap)
- **Check:** For each GDN layer, verify required tensors exist in the tensor table by name prefix (`blk.{i}.ssm_*`, `blk.{i}.attn_*`, etc.) — this is the same check as ADR-028 D2.3, extended to GDN tensor names
- **Reject:** `NeomError::ScheduleMissingTensor { layer_idx, tensor_name }`

### Invariant 6: State budget
- **Bytes:** `GDNLayerConfig.state_size` (config+0x00, 4 bytes); `header.weight_blob_len`
- **Check:** Sum of `(state_size * hidden_size * dtype_size)` across all GDN layers must not exceed a fraction of available tensor memory (e.g. 10% of `weight_blob_len` as a soft cap, configurable at compile time)
- **Reject:** `NeomError::ScheduleStateBudgetExceeded { requested, limit }`

### Invariant 7: No dynamic control flow
- **Bytes:** Entire schedule section — structural check, not data-dependent
- **Check:** The schedule is a flat array of entries. No branching, no conditional execution, no data-dependent dispatch. The walker iterates `entry_count` sequentially. This is enforced by the format: there are no "jump" or "branch" opcodes in `ScheduleEntry`.
- **Reject:** N/A — unrepresentable by construction. The format cannot express dynamic control flow.

---

## 4. Hand-laid worked examples (ISC-3) — the normative oracle

### 4.1 Example A: 2-layer Qwen2-only schedule (B1 parity fixture)

**Model:** Minimal 2-layer Qwen2 model. `layer_count = 2`. Both layers are `QWEN2_ATTENTION_FFN` (kind 0x00, empty config). No tensor data — this oracle covers the schedule section bytes only.

**Layout computation:**
```
schedule_off       = 0x78  (header v3 ends at 0x78)
ScheduleHeader     = 32 bytes → file 0x78..0x97
entry_array_off    = 0x78 + 32 = 0x98
entry array        = 2 × 8 = 16 bytes → file 0x98..0xA7
config_data_off    = 0x98 + 16 = 0xA8
config data        = 0 bytes (no GDN layers)
section_end        = max(0xA8, 0xA8) = 0xA8
schedule_len       = 0xA8 - 0x78 = 0x30 (48 bytes)
```

```
OFFSET  BYTES (LE)                                          FIELD
──────  ──────────────────────────────────────────          ──────────────────────────────────

=== Header v3 extension (within .neom header at file 0x00) ===
0x6C    78 00 00 00 00 00 00 00                              schedule_off = 0x78
0x74    30 00 00 00                                          schedule_len = 0x30 (48)

=== ScheduleHeader (file 0x78, 32 bytes: 0x78..0x97) ===
0x78    53 43                                                magic = 'S' 'C' (raw on-disk bytes)
0x7A    01 00                                                version = 1 (u16 LE)
0x7C    02 00 00 00                                          layer_count = 2 (u32 LE)
0x80    02 00 00 00                                          entry_count = 2 (u32 LE)
0x84    98 00 00 00 00 00 00 00                              entry_array_off = 0x98 (u64 LE, absolute)
0x8C    A8 00 00 00 00 00 00 00                              config_data_off = 0xA8 (u64 LE, absolute)
0x94    00 00 00 00                                          reserved = 0 (u32)

CHECK: header size = 0x98 - 0x78 = 0x20 = 32 bytes. PASS.
CHECK: 0x98 % 8 == 0 → entry array is 8-byte aligned. PASS.

=== ScheduleEntry[0] (file 0x98, 8 bytes) — QWEN2 layer 0 ===
0x98    00                                                   kind = 0x00 (QWEN2_ATTENTION_FFN)
0x99    00                                                   config_tag = 0x00
0x9A    00 00                                                config_off = 0 (region-relative; no config)
0x9C    00 00                                                config_len = 0
0x9E    00 00                                                reserved = 0

=== ScheduleEntry[1] (file 0xA0, 8 bytes) — QWEN2 layer 1 ===
0xA0    00                                                   kind = 0x00
0xA1    00                                                   config_tag = 0x00
0xA2    00 00                                                config_off = 0
0xA4    00 00                                                config_len = 0
0xA6    00 00                                                reserved = 0

Entry array: 0x98..0xA7 (16 bytes). CHECK: 0x98 + 16 = 0xA8. PASS.
Config region: empty at 0xA8. No data bytes to emit.

Section: [0x78, 0xA8) = 0x30 bytes. CHECK: 0x78 + 0x30 = 0xA8. PASS.
```

**Example A summary:** 2 layers, both QWEN2, schedule section = **48 bytes** (0x30).

---

### 4.2 Example B: 4-layer hybrid schedule (B2/B3 fixture)

**Model:** 4-layer Qwen 3.5 model with default `full_attention_interval = 4`.
Pattern: `(i+1) % 4 == 0 → full_attention (QWEN2)`, else `linear_attention (GDN)`.

| Layer | (i+1)%4 | Kind |
|-------|---------|------|
| 0 | 1 | GATED_DELTANET_LINEAR (0x01) |
| 1 | 2 | GATED_DELTANET_LINEAR (0x01) |
| 2 | 3 | GATED_DELTANET_LINEAR (0x01) |
| 3 | 0 | QWEN2_ATTENTION_FFN (0x00) |

**GDN config values** (hypothetical Qwen 3.5 2B): `state_size=16`, `conv_kernel=4`, `n_heads=16`, `key_head_dim=128`, `value_head_dim=128`. Each GDN config = 20 bytes. 3 GDN layers → 60 bytes of config data.

Final layout:
```
schedule_off       = 0x78
ScheduleHeader     = 32 bytes → file 0x78..0x97
entry_array_off    = 0x98
entry array        = 32 bytes → file 0x98..0xB7
config_data_off    = 0xB8
config data        = 60 bytes → file 0xB8..0xF3 (inclusive) / [0xB8, 0xF4)
section_end        = 0xF4
schedule_len       = 0xF4 - 0x78 = 0x7C (124 bytes)
```

```
OFFSET  BYTES (LE)                                          FIELD
──────  ──────────────────────────────────────────          ──────────────────────────────────

=== Header v3 extension ===
0x6C    78 00 00 00 00 00 00 00                              schedule_off = 0x78
0x74    7C 00 00 00                                          schedule_len = 0x7C (124)

=== ScheduleHeader (file 0x78, 32 bytes: 0x78..0x97) ===
0x78    53 43                                                magic = 'S' 'C' (raw on-disk bytes)
0x7A    01 00                                                version = 1 (u16 LE)
0x7C    04 00 00 00                                          layer_count = 4 (u32 LE)
0x80    04 00 00 00                                          entry_count = 4 (u32 LE)
0x84    98 00 00 00 00 00 00 00                              entry_array_off = 0x98 (u64 LE, absolute)
0x8C    B8 00 00 00 00 00 00 00                              config_data_off = 0xB8 (u64 LE, absolute)
0x94    00 00 00 00                                          reserved = 0 (u32)

CHECK: header size = 0x98 - 0x78 = 0x20 = 32 bytes. PASS.

=== ScheduleEntry[0] (file 0x98, 8 bytes) — GDN layer 0 ===
0x98    01                                                   kind = 0x01 (GATED_DELTANET_LINEAR)
0x99    01                                                   config_tag = 0x01
0x9A    00 00                                                config_off = 0x00 (region-relative)
0x9C    14 00                                                config_len = 20
0x9E    00 00                                                reserved = 0
→ config at file: 0xB8 + 0x00 = 0xB8

=== ScheduleEntry[1] (file 0xA0, 8 bytes) — GDN layer 1 ===
0xA0    01                                                   kind = 0x01
0xA1    01                                                   config_tag = 0x01
0xA2    14 00                                                config_off = 0x14 (region-relative; 20 decimal)
0xA4    14 00                                                config_len = 20
0xA6    00 00                                                reserved = 0
→ config at file: 0xB8 + 0x14 = 0xCC

=== ScheduleEntry[2] (file 0xA8, 8 bytes) — GDN layer 2 ===
0xA8    01                                                   kind = 0x01
0xA9    01                                                   config_tag = 0x01
0xAA    28 00                                                config_off = 0x28 (region-relative; 40 decimal)
0xAC    14 00                                                config_len = 20
0xAE    00 00                                                reserved = 0
→ config at file: 0xB8 + 0x28 = 0xE0

=== ScheduleEntry[3] (file 0xB0, 8 bytes) — QWEN2 layer 3 ===
0xB0    00                                                   kind = 0x00 (QWEN2_ATTENTION_FFN)
0xB1    00                                                   config_tag = 0x00
0xB2    00 00                                                config_off = 0
0xB4    00 00                                                config_len = 0
0xB6    00 00                                                reserved = 0

Entry array: 0x98..0xB7 (32 bytes). CHECK: 0x98 + 32 = 0xB8. PASS.

=== Config data region (file 0xB8, 60 bytes) ===

Layer 0 GDN config (file 0xB8, region_off=0x00):
0xB8    10 00 00 00                                          state_size = 16
0xBC    04 00 00 00                                          conv_kernel = 4
0xC0    10 00 00 00                                          n_heads = 16
0xC4    80 00 00 00                                          key_head_dim = 128
0xC8    80 00 00 00                                          value_head_dim = 128
CHECK: occupies [0xB8, 0xB8+20) = [0xB8, 0xCC). PASS.

Layer 1 GDN config (file 0xCC, region_off=0x14):
0xCC    10 00 00 00                                          state_size = 16
0xD0    04 00 00 00                                          conv_kernel = 4
0xD4    10 00 00 00                                          n_heads = 16
0xD8    80 00 00 00                                          key_head_dim = 128
0xDC    80 00 00 00                                          value_head_dim = 128
CHECK: occupies [0xCC, 0xCC+20) = [0xCC, 0xE0). PASS.

Layer 2 GDN config (file 0xE0, region_off=0x28):
0xE0    10 00 00 00                                          state_size = 16
0xE4    04 00 00 00                                          conv_kernel = 4
0xE8    10 00 00 00                                          n_heads = 16
0xEC    80 00 00 00                                          key_head_dim = 128
0xF0    80 00 00 00                                          value_head_dim = 128
CHECK: occupies [0xE0, 0xE0+20) = [0xE0, 0xF4). PASS.

Config region total: 60 bytes at [0xB8, 0xF4). CHECK: 0xF4 - 0xB8 = 0x3C = 60. PASS.

Section: [0x78, 0xF4) = 0x7C bytes. CHECK: 0x78 + 0x7C = 0xF4. PASS.
```

**Example B summary:** 4 layers (3 GDN + 1 QWEN2), schedule section = **124 bytes** (0x7C).

---

## 5. GGUF/HF config-keys-to-layer-interleave mapping (ISC-5)

### 5.1 HF transformers source

**File:** `src/transformers/models/qwen3_5/configuration_qwen3_5.py`
**Class:** `Qwen3_5TextConfig`
**Key field:** `layer_types: list[str] | None`
**Default generation** (in `__post_init__`):
```python
if self.layer_types is None:
    interval_pattern = kwargs.pop("full_attention_interval", 4)
    self.layer_types = [
        "linear_attention" if bool((i + 1) % interval_pattern) else "full_attention"
        for i in range(self.num_hidden_layers)
    ]
```

**Mapping rule:** For layer index `i` (0-based):
- If `(i + 1) % full_attention_interval == 0` → `"full_attention"` → `LayerKind::QWEN2_ATTENTION_FFN` (0x00)
- Else → `"linear_attention"` → `LayerKind::GATED_DELTANET_LINEAR` (0x01)

Default `full_attention_interval = 4`, meaning every 4th layer (indices 3, 7, 11, ...) is full attention; the rest are linear.

### 5.2 GGUF metadata keys (llama.cpp source)

**File:** `src/llama-arch.h` / `src/llama-arch.cpp`
**Arch:** `LLM_ARCH_QWEN35` → GGUF key `"qwen35"`
**Interval key:** `LLM_KV_FULL_ATTENTION_INTERVAL` → `"qwen35.full_attention_interval"` (u32)

The GGUF stores the interval as a single scalar. The importer derives the layer-type list from this interval using the same `(i+1) % interval` rule as HF.

### 5.3 Importer mapping algorithm

When importing a Qwen 3.5 GGUF into `.neom`:
1. Read `qwen35.block_count` → `header.layer_count`
2. Read `qwen35.full_attention_interval` (default 4 if absent)
3. For each layer `i` in `0..layer_count-1`:
   - If `(i + 1) % full_attention_interval == 0`: emit `ScheduleEntry { kind: 0x00, config_tag: 0x00, config_off: 0, config_len: 0 }`
   - Else: emit `ScheduleEntry { kind: 0x01, config_tag: 0x01, config_off: <region-relative>, config_len: 20 }` with GDN config from GGUF keys:
     - `state_size` ← `qwen35.ssm.state_size` (GGUF: `LLM_KV_SSM_STATE_SIZE`)
     - `conv_kernel` ← `qwen35.ssm.conv_kernel` (GGUF: `LLM_KV_SSM_CONV_KERNEL`)
     - `n_heads` ← `qwen35.attention.head_count` (GGUF: `LLM_KV_ATTENTION_HEAD_COUNT`)
     - `key_head_dim` ← from HF config `linear_key_head_dim` (not in GGUF directly; derive from tensor shape `blk.{i}.ssm_alpha` or hardcode 128 for known configs)
     - `value_head_dim` ← from HF config `linear_value_head_dim` (same derivation)

**OPEN-ITEM-1:** The `key_head_dim` and `value_head_dim` are not stored as explicit GGUF metadata keys in the current llama.cpp Qwen 3.5 implementation. They must be derived from tensor shapes (`blk.{i}.ssm_alpha` is `[head_dim, hidden_size]`) or from the HF config JSON. B1 should confirm the exact derivation path by inspecting a real Qwen 3.5 GGUF dump.

---

## 6. Open items (ISC-6)

| ID | Description | Impact |
|----|-------------|--------|
| **OPEN-ITEM-1** | `key_head_dim` and `value_head_dim` not in GGUF as scalar keys — must derive from tensor shapes or HF config | B3 import correctness; B1/B2 unaffected (use fixture values) |
| **OPEN-ITEM-2** | State budget invariant (§3, Invariant 6) needs a concrete cap value. Current spec says "10% of weight_blob_len as soft cap, configurable at compile time." B1 should define the actual constant. | Validation threshold; not blocking |

**Resolved in this revision:**
- ~~OPEN-ITEM-2 (old)~~: ScheduleHeader `reserved` field sizing — resolved: header is 32 bytes with u32 reserved, no post-header padding needed.
- ~~OPEN-ITEM-3 (old)~~: `config_off` absolute vs region-relative — resolved: region-relative u16 (offset from `config_data_off`).

---

## 7. Size formulas

For a model with `N` layers, `G` GDN layers, `Q` Qwen2 layers (`G + Q = N`):

```
schedule_section_size = 32          // ScheduleHeader
                      + 8 * N       // ScheduleEntry array (no padding; header is 32 = 0 mod 8)
                      + 20 * G      // GDN config payloads (packed contiguously)
```

The section layout has no gaps:
```
entry_array_off    = schedule_off + 32
config_data_off    = schedule_off + 32 + 8*N
section_end        = schedule_off + 32 + 8*N + 20*G
schedule_len       = 32 + 8*N + 20*G
```

Example A (N=2, G=0): 32 + 16 + 0 = 48 = 0x30. PASS.
Example B (N=4, G=3): 32 + 32 + 60 = 124 = 0x7C. PASS.

---

## 8. Summary

| Item | Value |
|------|-------|
| Format version | v3 header (additive to v2; 12-byte extension at 0x6C..0x77) |
| ScheduleHeader | 32 bytes (tightly packed, no internal padding) |
| ScheduleEntry | 8 bytes each, one per layer |
| config_off | Region-relative u16 (offset from `config_data_off`) |
| Qwen2 config | 0 bytes (empty) |
| GDN config | 20 bytes (5 × u32) |
| Magic | Raw on-disk bytes `0x53, 0x43` ('S', 'C') |
| Example A (2-layer Qwen2) | 48 bytes total (0x30) |
| Example B (4-layer hybrid: 3 GDN + 1 QWEN2) | 124 bytes total (0x7C) |
| Validation invariants | 7 invariants mapped to exact bytes (§3) |
| GGUF mapping | `qwen35.full_attention_interval` → `(i+1) % interval` rule |
| Open items | 2 (head dim derivation, state budget cap) |
