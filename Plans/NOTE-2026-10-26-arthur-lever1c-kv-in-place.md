<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: lever 1c: read the KV cache in place in the cached attention step

**Seat:** S4. **Status:** design only, no code. The PM asked for it after #134. Build it only once A8's
`sw=`/`at=` shares are in. Background: `Plans/NOTE-2026-10-25-arthur-lever2-design.md` (lever 1b, and
the per-step copy found while measuring it).

## What the cached step does today, per layer, per decode token

`gqa_attention_cached_decode` (`kernel/src/ai/forward_logic.rs`) runs, for a context of `seq` tokens:

| step | what it builds | size at Qwen2.5-0.5B (14 q heads, 2 kv heads, head_dim 64) |
|---|---|---|
| `cache.append_rope_kv` | writes this token's K/V into the cache, which is laid out `[head][max_context][dim]` | 2 × 128 floats; the only work that has to happen |
| `query_for_last_position` | a new `[14 × seq × 64]` f32 buffer, all zeros except the last row | 14 × 64 × `seq` floats, allocated and zeroed |
| `cache.active_key()` / `active_value()` | two new `[2 × seq × 64]` copies of the cache's active prefix | 2 × 2 × 64 × `seq` floats |
| `bytes_from_f32` (×3, in `gqa_attention_last`) | byte copies of the query, key and value, one element at a time | the same 18 × 64 × `seq` floats again |
| `f32_from_bytes` | the output back to f32 | 14 × 64 floats |

So every decode step allocates and fills about **18 × 64 × `seq` × 4 bytes twice**:
- `seq` = 25: about 230 KB, twice;
- `seq` = 256: 2.4 MB, twice;
- `seq` = 1024: **4.7 MB, twice**.

That's per layer, so 24 times per token. The attention kernel itself then reads only the last query row
(14 × 64), plus every key and value row.

**Measured** (host, one thread, opt 3, the `split` bin after #130). "Overhead" is the whole cached step
minus the ops kernel alone:

| `seq` | whole cached step | `gqa_causal_attention_last` alone | overhead |
|---|---|---|---|
| 25 | 0.26 ms | 0.15 ms | 0.11 ms (≈ 40%) |
| 256 | 2.4–2.5 ms | 1.4–1.75 ms | ≈ 0.8 ms (≈ 33%) |
| 1024 | 10.1–10.2 ms | 5.5–5.7 ms | ≈ 4.5 ms (≈ 45%) |

The overhead grows with `seq` as fast as the arithmetic does. In the kernel it also churns the heap:
three multi-MB allocations per layer per token at long contexts, so `heap_high_water` grows with the
context. (The census in `real_lm_generate_boot`, 640,697 bytes, is a short context.)

## The change

**One new ops entry that reads the cache where it is:**
- `ops::gqa_causal_attention_last_strided(q_last, key, value, head_stride, params, out)`;
- `q_last` is the 14 × 64 row as given (`attention.q_rope`);
- `key`/`value` are the cache's `Vec<f32>` storage, used directly;
- `head_stride = max_context × head_dim`;
- `seq_len = used_len`.

**Same arithmetic as lever 1b's body:**
- the same per-head loops;
- each score computed once into `weights`;
- the same summation orders (score over `dim`, denominator and outputs over `k_pos`).

Elements are read as `f32::to_bits` instead of from little-endian bytes. On x86 that's the same 32 bits.

**`gqa_attention_cached_decode`** calls it after `append_rope_kv`, and returns the f32 output directly.
`query_for_last_position`, `active_key`/`active_value` and the three `bytes_from_f32` calls leave this path.
They stay for the uncached and full-causal paths.

**Bounds.** One check up front:
- `key.len() >= kv_heads × head_stride`;
- `used_len ≤ max_context`;
- `q_last.len() == q_heads × head_dim`.

After that, rows are sliced as in lever 1b. The cache owns these invariants already (`append_rope_kv`
enforces `used_len < max_context`).

**What it removes per step:** four allocations and about 2 × 18 × 64 × `seq` float copies. **What it
adds:** nothing on the stack. The `weights` buffer (`seq` × 4 bytes) stays the only allocation.

## Byte identity and the KAT

The new entry must give the same bytes as the current path (copy + `gqa_causal_attention_last`):
- **Shapes and contexts.** Run both on the same cache at 0.5B shapes, contexts 1, 2, 17, 255, 1024 and
  2304 (the cap), at three value scales. This is lever 1b's KAT pattern, with the cache filled through
  `append_rope_kv`.
- **Partly filled cache.** Include `used_len < max_context` with garbage written past `used_len`. The new
  path must never read past it, which a stride bug would.
- **Falsification.** Off-by-one the head stride (`max_context - 1`) and the KAT must fail.
- **Regression gate:** `sys_generate_kv_persist_equivalence` and `real_lm_generate_boot`. The latter
  checks tokens against the Python oracle.
- **No kill-switch feature needed.** The old path stays in the tree for the other callers, and the KAT
  compares both in one binary (the #130 pattern).

## Expected gain, and what A8 must show

**From the table above**, the change removes roughly the overhead column:
- **about 40%** of the cached step at `seq` 25 (0.11 of 0.26 ms per layer);
- **about 45%** at 1024 (4.5 of 10.1 ms).

At Arthur's current lengths (a prompt of about 28 tokens plus a few dozen decoded) that's about 0.1 ms per
layer, roughly 2.5 ms per token on the host.

A8's overlay gives `at=`, the cached step's share of the forward: copies plus kernel. The go/no-go:
- **Build it if A8's `at=` is about 10% or more** at Arthur's lengths. Lever 1c removes roughly 40% of
  `at=`, so that's about 4% or more of the forward, on par with fix 4b, for a contained change.
- **Build it regardless if long contexts become a product path** (chat history, memory recall at hundreds
  to thousands of tokens). There it removes almost half the attention time and the multi-MB per-step heap
  churn, whatever A8 says.
- **Skip it for now if `at=` is about 5% or less** and contexts stay short. Revisit when they grow.

## Falsification

- **Lever 1c is wrong** if the KAT shows any byte difference, or the partly-filled-cache case reads past
  `used_len`.
- **It doesn't pay** if, on the host split, the whole cached step drops by less than about 30% at `seq`
  256. In that case the overhead isn't the copies, and the split needs a finer probe before any build.

## Order against the other levers

1. **A8's numbers** (`mv=`, `sw=`, `at=`, `bar=`) decide.
2. **Lever 1c** if `at=` clears the bar above. It's small, serial and byte-identical, and it shrinks what
   lever 2 would split.
3. **Lever 2** (parallel SwiGLU and attention heads) under its own thresholds in the lever 2 NOTE.
   With 1c in, lever 2's attention split works on the kernel alone, not on the copies.
4. **Fix 4b** last.
