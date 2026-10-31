<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-072: `.neom` v3 Layer-Kind Schedule Section (OT-16 B1)

| | |
|---|---|
| **Status** | ✅ Accepted (OT-16 B1, 2026-07-07) |
| **Date** | 2026-07-07 |
| **Authority** | PM + PL |
| **Supersedes** | None |
| **Superseded by** | - |
| **Related** | ADR-028 (`.neom` v1 format), the v2 config-metadata extension; ANSPEC §19.2 (extension by accretion; op set is substrate-stable, the model *graph* is data); Bible Article 5.7 (no per-architecture kernel branches); `docs/THINK-2026-07-05-ot16-forward-graph-design.md` (the ratified OT-16 design); ADR-070 (OT-34 threaded matmul — the walker dispatches to it unchanged) |

---

## Context

`kernel/src/ai/forward_logic.rs` + `qwen_inventory.rs` hardcode the Qwen2
model-graph orchestration in ring 0 (the per-layer `0..layer_count` loop). A new
architecture is currently a *kernel patch* with `if qwen / if delta_net`
branches — the Article 5.7 anti-pattern applied to model graphs. OT-16 makes the
forward *graph* **data-driven from the `.neom`**: a new architecture becomes a
new `.neom` schedule, not a kernel branch (ANSPEC §19.2 — the op *primitives*
stay compiled-in and proven; only the *wiring* becomes data).

B1 defines the on-disk schedule and proves the data-driven walker reproduces
today's hardcoded Qwen2 forward bit-for-bit. This ADR names the `.neom`
format-version bump (an ADR-028-family change) and ships the mandatory
independent hand-laid byte oracle for the new section.

## Decision 1 — v3 is an additive, version-gated header extension

The `.neom` container gains **version 3** (`NEOM_VERSION_V3 = 3`), header size
**124 bytes** = the v2 header (108) + a **16-byte schedule descriptor**. This
follows the exact v1→v2 idiom (v2 appended a 12-byte config block). All fields
little-endian, all offsets absolute from file start (ADR-028 invariant).

`NEOM_VERSION` (the *default* written version) stays **2**: v3 is emitted only
when a layer-kind schedule is requested. Existing v1/v2 files are unaffected, and
the walker **synthesizes** a homogeneous `n × Qwen2AttentionFFN` schedule for a
v1/v2 file — so the shipped 0.5B/1.5B/3B `.neom` files run unchanged.

## Decision 2 — the v3 schedule descriptor + LayerDescriptor table

**v3 header schedule descriptor** (offsets relative to file start):

| offset | field | type | notes |
|---|---|---|---|
| 108 | `schedule_layer_count` | u32 LE | number of LayerDescriptor entries; must equal the config `layer_count` |
| 112 | `schedule_version` | u32 LE | schedule format version (independent of the container version); currently `1` |
| 116 | `schedule_off` | u64 LE | absolute offset to the LayerDescriptor table; 8-aligned, ≥ header_size |

**LayerDescriptor** table at `schedule_off`, `schedule_layer_count` × **8 bytes**:

| offset (within entry) | field | type | notes |
|---|---|---|---|
| +0 | `kind` | u32 LE | `0` = `Qwen2AttentionFFN`, `1` = `GatedDeltaNetLinear` |
| +4 | `reserved` | u32 LE | must be `0` in B1 (reserved for a B2 per-layer config-ref) |

`LayerKind` value `0` (`Qwen2AttentionFFN`) is the only kind B1 **executes**. `1`
(`GatedDeltaNetLinear`) is declared so the format is stable for B2, but B1
**rejects it at load** (`ModelFormatError::LayerKindUnimplemented`) — never
stub-executed. For Qwen2 the per-layer config is global (the v2 config block), so
B1's descriptor needs only `kind`; per-layer config is B2's use of `reserved`.

## Decision 3 — the 7 load-time validation invariants (rejected as data)

A malformed or hostile schedule is rejected at `.neom` load, before any forward
executes (`validate_neom` → `parse_schedule`), with a typed `ModelFormatError` —
it never reaches ring-0 execution. The invariants: (1) `schedule_version` in the
supported range; (2) `0 < layer_count ≤ MAX_LAYERS (128)`; (3) known kinds only;
(4/5) `layer_count` equals the model config's `layer_count` (with the existing
per-tensor loader validation covering weight presence for B1's homogeneous
Qwen2); (6) state budget — B1's only kind reuses the already-budgeted KV cache
and declares no extra recurrent state; (7) the table is a static in-file array
(8-aligned, within bounds, after the header — no dynamic control flow). `reserved
≠ 0` is also rejected.

## Decision 4 — the double kill-switch

- **Kill-switch 1:** with the `ot16-graph-walker` feature **off** (default), the
  ring-0 forward is the exact hardcoded per-layer loop, bit-for-bit; the v3
  schedule field is not even compiled into the model object.
- **Kill-switch 2:** with the feature **on** and a Qwen2 schedule, the walker
  produces **byte-identical** output to the hardcoded loop — same op helper, same
  args, same order. Placement changes; the computed bytes do not.

## Verification scope (B1a / B1b — honest, post-AAG 2026-07-08)

KS2's **executed** proof is a host KAT that runs the *real* dispatch
(`model_logic::walk_qwen2_schedule`, the exact function the ring-0 walker calls)
with **distinct per-layer weights** and asserts exact-bit equality to the
hardcoded loop, plus a reversed-order guard proving the weights are distinct (so
a `layer_idx`/order mis-map would fail — not the tautology of identical weights).
The QEMU scenario `arthur_real_brain_boot_ot16_walker` builds + boots the
walker-**ON** kernel at NEO_SMP=4 but runs the hermetic **graph fixture**
(`run_graph_infer`), so it proves the walker-on kernel builds/boots/KS1-holds —
**not** walker execution. **Deferred to B3 (explicit gate item):** end-to-end
walker execution on a real inventory model (routed through
`sys_generate`/`qwen_inventory`) with OT-30 top-1 parity, and the walker → OT-34
threaded-matmul path exercised on real cores at NEO_SMP=4.

## Authoritative worked example (the byte oracle)

Independent of the writer and reader, the exact on-disk bytes for a **2-layer
schedule `[Qwen2AttentionFFN, GatedDeltaNetLinear]`** whose table sits at file
offset `128` (`0x80`). Round-trip (`write_then_read == input`) is **not**
sufficient — a writer and reader built from the same off-spec understanding agree
even when both are wrong (the ADR-028 v1 defect). This hand-computed fixture is
asserted directly in `tools/neomc` (writer) and `tools/model-logic-tests`
(reader).

Header schedule descriptor (offsets 108..124):

```
108:  02 00 00 00            schedule_layer_count = 2
112:  01 00 00 00            schedule_version     = 1
116:  80 00 00 00 00 00 00 00  schedule_off       = 128 (0x80)
```

LayerDescriptor table at `schedule_off` = 128 (`0x80`), 2 × 8 bytes = 16 bytes
(addresses decimal, matching the header offsets above; hex in parens):

```
128 (0x80):  00 00 00 00        entry[0].kind     = 0  (Qwen2AttentionFFN)
132 (0x84):  00 00 00 00        entry[0].reserved = 0
136 (0x88):  01 00 00 00        entry[1].kind     = 1  (GatedDeltaNetLinear)
140 (0x8C):  00 00 00 00        entry[1].reserved = 0
```

(A homogeneous Qwen2 schedule is the same with `entry[1].kind = 00 00 00 00`.
`GatedDeltaNetLinear` is representable in the format but rejected at load in B1.)

## Consequences

- A second model architecture becomes a new `.neom` schedule + new ops (B2/B3),
  not a ring-0 `if qwen / if delta_net` branch.
- The change is additive and default-off: no risk to shipped models or the
  hardcoded forward, which remains the always-available fallback.
- The schedule is a **typed enum-dispatch table**, not a graph VM — no ring-0 NN
  compiler (ANSPEC §19.2 / Article 6.6).

---

## OT-16 B3 amendment — v4 partial-RoPE geometry (2026-07-11)

Qwen3.5 decouples its 256-element attention head from its 64-element rotary
subspace. Deriving RoPE width from `head_dim` would make the model family a
kernel branch and rotates 192 channels that the reference leaves untouched.
The GGUF source carries the authoritative value as
`qwen35.rope.dimension_count`.

`.neom` **v4** preserves v3 byte-for-byte and appends one field:

| offset | field | type | notes |
|---|---|---|---|
| 124 | `rope_dimension_count` | u32 LE | non-zero, even, and no greater than the derived attention `head_dim` |

The v4 header is therefore **128 bytes**. The schedule descriptor remains at
108..124 and its table retains the v3 `LayerDescriptor` ABI. v1/v2/v3 remain
accepted; because those artifacts predate partial-RoPE models, their runtime
value is derived as `rope_dimension_count = head_dim`. A builder emits v4 only
when this field is explicitly supplied alongside a schedule.

For the B3 text-only scope, Qwen3.5's four MRoPE position axes are equal. The
reference's interleaved section selector therefore produces the same split-half
rotation as the existing NeoX op over the declared 64 dimensions; channels
64..255 are copied unchanged. Vision positions and the vision tower remain out
of scope and would require carrying the four axis sections as additional data.

Independent byte oracle for `rope_dimension_count = 64`:

```
124 (0x7C):  40 00 00 00
```

Writer and reader KATs assert those literal bytes independently, and the
attention KAT asserts both the rotated prefix and bit-identical unrotated tail.
