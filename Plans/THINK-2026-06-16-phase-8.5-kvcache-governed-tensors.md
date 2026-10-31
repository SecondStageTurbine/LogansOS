<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 8.5 — KV cache as governed tensors

**Date:** 2026-06-16
**Author:** Prometheus (design lane)
**Status:** DRAFT — open decisions below require PM/PL ratification before dispatch
**Branches on:** ADR-037 (eviction-under-pressure), ADR-040 (residency-daemon), Phase 7 governance arc
**Prerequisite:** Phase 8.3 (f32 forward path with dequant→matmul) + 8.4a/8.5a (RoPE/SwiGLU, standalone GQA attention)

---

## 1. The KV cache — what it is and how big it grows

### 1.1 Shape per layer

For Qwen2.5-0.5B:
- `n_layer = 12`, `n_head = 6`, `n_head_kv = 2` (GQA grouping factor = 3)
- `head_dim = 128` (`n_embd / n_head = 768 / 6`)

Per layer, the KV cache holds:
- **K:** `[n_head_kv=2, head_dim=128, seq_len]` — grows by one position per generated token
- **V:** `[n_head_kv=2, head_dim=128, seq_len]` — same shape

Per-token growth per layer (f32):
```
K_delta = 2 × 128 × 4 B = 1 KiB
V_delta = 2 × 128 × 4 B = 1 KiB
Total per layer per token = 2 KiB
All layers per token = 12 × 2 KiB = 24 KiB
```

### 1.2 Worst-case sizes at context length

Qwen2.5-0.5B default context: **4096 tokens**.

| Metric | Value |
|--------|-------|
| Per-layer K at max ctx | 2 × 128 × 4096 × 4 B = **4 MiB** |
| Per-layer V at max ctx | **4 MiB** (same) |
| Per-layer KV total | **8 MiB** |
| All 12 layers | **96 MiB** |

This is the f32 baseline. If Phase 9 quantized weights land first and we store cached K/V in a lower precision (e.g., f16 or q8), the numbers halve or quarter — but that's a Phase-9 concern. For 8.5, plan for f32.

### 1.3 Growth pattern during decode

The KV cache is **monotonically growing**: each generated token appends one position to every layer's K and V. Unlike weights (fixed at load), the cache has no upper bound until the model's context limit. During a single generation session:
- Token 0 (prefill): all prompt tokens fill positions [0..prompt_len) in one batch
- Tokens 1..N (decode): each step appends position `t`, making it visible to all future attention heads

---

## 2. The growth-vs-governance tension — options analysis

The governance substrate (arena + reservation + residency daemon) is built for **fixed-size production tensors**: mint once, dedup-aware, slot-keyed, immutable byte footprint. A growing tensor violates the fundamental assumption that `tensor.byte_len()` is constant after alloc.

### Option A: Pre-reserve max-context capacity (RECOMMENDED)

Mint each layer's K and V as a single arena-managed tensor sized for the full context length at generation start. Track `used_len` internally; the arena sees a fixed size.

**Pros:**
- **Zero invariant violation.** The arena, reservation, and daemon all see a fixed-size tensor. SlotKey remains valid across the entire decode session. Liveness probeable via I-13.
- **Natural eviction granularity.** The residency daemon can evict the entire layer-KV pair as one unit under pressure — consistent with ADR-037's "no partial eviction" decision (decision 3).
- **Simplest for 8.5.** No new arena operations, no segmented handle tracking, no mid-decode reallocation. The attention op just writes at position `used_len` and increments the counter.
- **llama.cpp alignment.** This is exactly what llama.cpp does (`llama-kv-cache.cpp:245-246`: allocates `[n_embd_gqa, kv_size, n_stream]` at context creation). The pre-allocation pattern is proven for this workload.

**Cons:**
- **Wastes space on short generations.** A 50-token generation reserves space for 4096 — but 96 MiB is not catastrophic on a system with 128 GB RAM and the arena's buddy-frame allocator.
- **Eviction is coarse-grained.** Can't evict just the cold prefix of a 4096-token cache; it's all-or-nothing per layer. This is acceptable for 8.5 (see §3 residency scope call).

**Invariant impact:** None. All existing invariants hold because the tensor is fixed-size from the arena's perspective. The `used_len` counter is an application-level concern, invisible to governance.

### Option B: Grow-and-remint per token or batch

Each time the cache grows beyond its current allocation, free the old tensor and mint a new larger one. Update all SlotKey holders.

**Pros:**
- Exact space usage — no waste for short generations.

**Cons:**
- **Violates I-9/I-13.** Every remint changes the SlotKey (generation bump on free). All attention ops holding references to the old key must re-resolve mid-decode. This is a structural break of the generational-handle invariant.
- **Daemon disruption.** The residency daemon's recency table, watermark accounting, and prefetch state are keyed by `tensor_id`. Reminting invalidates all of them mid-session.
- **Churn.** At 24 KiB/token growth, reminting every few tokens creates massive allocator churn. Even batching to powers-of-two (1→2→4→8... positions) means ~12 remints for a 4096-token generation, each requiring a full tensor copy.
- **No clear eviction story.** A newly-minted tensor has no recency history; the daemon treats it as brand-new and unprefetchable.

**Verdict:** Rejects itself. The invariant violations are structural, not fixable with bookkeeping.

### Option C: Ring buffer (fixed-size, wraparound)

Fixed-capacity cache that wraps around, overwriting oldest positions when full. Cell-level metadata tracks occupancy (like llama.cpp's `llama_kv_cells`).

**Pros:**
- Bounded memory regardless of generation length.
- llama.cpp has cell-level tracking infrastructure (`llama-kv-cells.h:32-535`).

**Cons:**
- **Attention semantics break.** Standard causal attention reads all positions [0..t). A ring buffer that wraps means position 0's data was overwritten — the attention computation would read stale K/V from a different generation. This requires either RoPE shifting (llama.cpp's `llama_memory_can_shift`) or accepting that wrapped tokens contribute garbage to attention.
- **Governance complication.** The arena sees one tensor, but the logical content is discontinuous. Eviction semantics are unclear: what does it mean to evict a ring buffer whose "active" region wraps around?
- **Overkill for 8.5.** Qwen2.5-0.5B at 4096 context fits comfortably in 96 MiB. Ring buffering is a long-context optimization that doesn't match our model's scale.

**Verdict:** Reject for 8.5. The attention semantics problem makes this fundamentally different from the standard decode pattern we're implementing. Consider for Phase-10+ with models that exceed available memory.

### Option D: Segmented tensors (each segment = arena-managed tensor)

Allocate KV storage in fixed-size segments (e.g., 256-token blocks). Each segment is its own arena-managed tensor with its own SlotKey. Growth = new segment alloc. Attention concatenates views across segments.

**Pros:**
- **Natural eviction granularity.** The residency daemon can evict cold prefix segments independently while keeping hot (recent) segments device-resident. This is the "residency showcase" at its best — fine-grained LRU on a growing structure.
- **Exact space usage.** Only allocate segments for tokens that exist.
- **Incremental growth without remint churn.** Segment size of 256 tokens = ~16 allocs for 4096 context, vs. per-token remint's thousands of copies.

**Cons:**
- **Attention complexity.** The attention op must concatenate views across multiple segments to form the full K/V sequence. This adds a scatter-gather step before every attention call that doesn't exist with a contiguous tensor.
- **Handle management.** Each segment needs its own SlotKey; the cache controller tracks a list of keys. If a segment is evicted and later needed, it must be recomputed (from scratch — there's no "reload" for generated K/V).
- **New arena pattern.** The substrate has no precedent for a tensor-list representing one logical entity. Dedup, reservation, and the daemon all operate per-slot.

**Verdict:** Technically compelling for the residency showcase, but adds too much complexity for 8.5's first implementation. Recommend as the Phase-9+ path once pre-reserved proves the baseline.

### Recommendation: Option A (pre-reserve) for 8.5

Pre-reserve is the right choice because:
1. It preserves every existing invariant without modification to the arena, reservation, or daemon
2. The space cost (96 MiB for Qwen2.5-0.5B) is negligible on our target hardware (128 GB RAM)
3. llama.cpp uses this exact pattern — it's proven correct for the workload
4. It makes 8.5 a wiring exercise, not an invention session under integration pressure
5. The residency showcase can still happen at the layer level: evict entire cold layers under pressure, prefetch hot ones

---

## 3. Residency management scope call — 8.5 vs later

### Decision: Layer-level governance in 8.5; segment-level eviction designed-now, built-later

**In 8.5:** The KV cache participates in the residency system at **layer granularity**. Each layer's K+V pair is registered as managed slots. The daemon can:
- Evict a full layer's KV to CPU under pressure (ADR-037 LRU)
- Prefetch a published-hot layer back to device (ADR-040 prefetch)

This is the "residency showcase" — it demonstrates that inference state is kernel-governed tensor memory, not just a userspace buffer. The eviction granularity is coarse (whole layers), but sufficient for 8.5.

**Designed now, built later:** Segment-level eviction within a layer. Once pre-reserved proves correct, Phase-9+ can split each layer's KV into segments and register each as an independent managed slot. This gives the daemon fine-grained LRU: evict cold prefix positions while keeping recent ones hot. The design for this is straightforward (Option D above) and doesn't need to block 8.5.

**Rationale:** The residency daemon already handles per-slot eviction with correct recency tracking, watermark management, and migration reservation. Registering layer-KV pairs as managed slots requires no new daemon logic — just the right registration from the cache controller. Fine-grained segment eviction would require recomputation of evicted segments (no reload path for generated K/V), which is a Phase-9 concern about what to recompute and how.

---

## 4. Kill-switch shape

### Rule-1 compliance: disabled ≡ uncached recompute-all

The kill switch is a boolean config on the attention pass: `kv_cache_enabled`.

**Disabled (`kv_cache_enabled = false`):**
- Each decode step computes Q, K, V for the current token
- Attention reads K/V from the **current step's computation**, not from any cache
- This is exactly 8.5a's standalone GQA attention — recompute-all per token
- Output is byte-identical to the no-cache path (same f32 matmul, same softmax)

**Enabled (`kv_cache_enabled = true`):**
- At decode step t: compute Q_t, K_t, V_t for the new token
- Store K_t, V_t into the pre-reserved cache at position t
- Attention reads K[0..t], V[0..t] from the cache + Q_t from current computation
- Output must match the disabled path within f32 numerical tolerance

**Implementation note:** The kill switch lives in the executor's attention dispatch, not in the arena or daemon. When disabled, the cache tensors are never written to and never read from — they exist as zeroed pre-reserved slots but participate in zero attention computation. This makes the disabled path provably identical to 8.5a (no new code paths execute).

---

## 5. Correctness oracle — cached == uncached equivalence KAT

### The I-12 plan

**Test structure:** Run the same model + same prompt through both paths and compare logits at each decode step.

```
For t = 0..N_decode:
    logits_uncached[t] = forward_step(prompt, generate_t, kv_cache_enabled=false)
    logits_cached[t]   = forward_step(prompt, generate_t, kv_cache_enabled=true)
    assert |logits_cached[t] - logits_uncached[t]| < ε_f32  (per-element, max-norm)
```

**Why this works:** The uncached path recomputes K/V from scratch at every step using the same f32 matmul that produced the cached values. If the cache is correct, reading stored K/V must produce identical attention scores to recomputing them. Any divergence indicates a cache write/read bug (wrong position, wrong layer, shape mismatch).

**Tolerance:** Use the same `ε_f32` as existing Phase-7 matmul KATs (~2⁻¹² relative per ADR-032). The cached path should be **numerically closer** than the uncached path because it avoids one round of recompute accumulation — but we gate on "within tolerance," not "bit-identical," because f32 matmul order may differ (cached reads a contiguous block; uncached recomputes then attends).

**Oracle vectors:** Raw-pull from llama.cpp's test infrastructure:
1. Clone llama.cpp, build with tests enabled
2. Run `llama-cli` with the same small model + prompt in two modes:
   - Normal mode (KV cache enabled) → capture logits at each step
   - No-cache equivalent (impossible directly; use our uncached path instead)
3. Compare our cached output against llama.cpp's cached output on the same model/prompt as a secondary oracle

**Primary gate:** Our cached == our uncached within tolerance. This is self-contained and doesn't depend on external oracles.

---

## 6. Raw-sourced llama.cpp KV layout reference

**Source:** `ggerganov/llama.cpp` commit `74ade52741203e5c8f81eaf06a96cb1cfe15f2a3`

### Per-layer storage — src/llama-kv-cache.h:221-231
```cpp
struct kv_layer {
    uint32_t il;                    // model layer index
    ggml_tensor * k;               // K cache tensor
    ggml_tensor * v;               // V cache tensor
    std::vector<ggml_tensor *> k_stream;  // per-stream views of K
    std::vector<ggml_tensor *> v_stream;  // per-stream views of V
};
```

### Tensor allocation — src/llama-kv-cache.cpp:245-246
```cpp
// Full pre-allocation at context creation:
ggml_new_tensor_3d(ctx, type_k, n_embd_k_gqa, kv_size, n_stream);
ggml_new_tensor_3d(ctx, type_v, n_embd_v_gqa, kv_size, n_stream);
```

Shape: `[n_embd_gqa, kv_size, n_stream]` where `n_embd_gqa = head_dim × n_head_kv`.

### GQA view reshaping — src/llama-kv-cache.cpp:1251-1256
The 3D tensor is viewed as 4D for attention reads:
```cpp
ggml_view_4d(ctx, k, n_embd_head_k, n_head_kv, n_kv, ns, ...)
// Result: [head_dim, n_head_kv, active_positions, streams]
```

### Cell metadata — src/llama-kv-cells.h:32-535
Per-cell tracking: position, sequence bitset, RoPE shift accumulator. Used for slot finding during `find_slot()` (src/llama-kv-cache.cpp:907-1104). Linear scan with wraparound, no compaction.

### No eviction or paging
llama.cpp has **no KV cache eviction**. When the cache is full (`n_tested >= cells.size()` at h:1089), it returns failure. The user must manually call `llama_memory_seq_rm()` to free space. This confirms that pre-reservation with bounded context is the industry-standard approach for this scale.

### K/V store and read during attention — src/llama-graph.cpp:2345-2357
```cpp
// Store fresh K/V into cache at found slot indices:
ggml_build_forward_expand(gf, mctx_cur->cpy_k(ctx0, k_cur, k_idxs, il));
ggml_build_forward_expand(gf, mctx_cur->cpy_v(ctx0, v_cur, v_idxs, il));

// Read entire active cache prefix for attention:
ggml_tensor * k = mctx_cur->get_k(ctx0, il);
ggml_tensor * v = mctx_cur->get_v(ctx0, il);
```

Store uses `ggml_set_rows` (scatter-write at specific indices). Read returns a view of the active prefix `[0..n_kv)`.

---

## 7. Proposed 8.5 build slicing

### 8.5a — Pre-reserved KV cache + store/read (no governance yet)
- Allocate layer-K and layer-V tensors for full context at generation start
- Register each as arena-managed slots (SlotKey per layer-KV pair)
- Attention dispatch: after computing K_t/V_t, scatter-write to cache at position t
- Attention reads K[0..t], V[0..t] from cache views
- Kill switch: `kv_cache_enabled` — disabled ≡ 8.5a standalone attention (recompute-all)
- **Gate:** cached == uncached equivalence KAT within tolerance

### 8.5b — Residency showcase (layer-level governance)
- Register layer-KV pairs as managed slots with the residency daemon
- Daemon can evict cold layers to CPU under pressure (ADR-037 LRU)
- Prefetch hot layers back on demand (ADR-040 prefetch)
- Recompute evicted layer's K/V from stored hidden states on reload (or accept recompute-on-demand as the eviction cost)
- **Gate:** generation completes correctly with daemon active, including eviction events during decode

### 8.5c — Segment-level eviction (designed, not built)
- Split each layer's KV into N segments of M positions
- Register each segment independently; daemon evicts cold prefix segments
- Recompute evicted segments on demand from hidden states
- **Status:** Design only for 8.5; implementation deferred to Phase-9+

---

## 8. Open decisions for PM/PL ratification

| # | Decision | Options | Recommendation |
|---|----------|---------|----------------|
| OD-1 | **KV cache precision** — store K/V as f32 or lower? | f32 (safe, matches weights), f16 (half space), q8_K (matches Phase 9 path) | **f32 for 8.5.** Lower precision is a Phase-9 concern; don't compound integration risk. |
| OD-2 | **Recompute-on-eviction** — when the daemon evicts a layer's KV, how do we recover? | (a) Recompute from hidden states of that layer, (b) abort generation and restart with smaller context, (c) block eviction on KV cache tensors | **(a) for 8.5b.** Hidden states are available per-layer; recomputing one layer's K/V is O(layer_cost), not O(full_pass). Option (c) defeats the residency showcase. |
| OD-3 | **Per-stream or single-stream?** — llama.cpp supports multi-stream KV caches via dim-2 slicing. Do we need this? | Single-stream (simpler, one generation at a time), Multi-stream (future-proof for concurrent sessions) | **Single-stream for 8.5.** Multi-stream is a Phase-10 concern; the tensor shape simplification removes an entire dimension. |
| OD-4 | **Cache registration timing** — when do KV slots enter the managed-slot registry? | (a) At generation start (pre-reserve + register), (b) lazily on first cache write, (c) never (bypass governance in 8.5) | **(a).** Register at pre-reserve time; the daemon needs the full lifecycle to demonstrate governance. Option (c) makes it not a showcase. |
| OD-5 | **Segment size for future segment-level eviction** — if we build Option D later, what's the right segment granularity? | 64 tokens (~128 KiB/layer), 256 tokens (~512 KiB/layer), 1024 tokens (~2 MiB/layer) | **256 tokens.** Balances eviction granularity (not too fine to cause thrash, not too coarse to waste the showcase). Defer final decision until Phase-9+. |

---

## 9. Risk register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Pre-reserved 96 MiB exceeds device capacity on accel_sim (256 KiB arena) | High — can't fit KV cache on the test accelerator at all | 8.5 runs on CPU/accel_intel initially; accel_sim is too small for real model inference. The residency showcase works on any capacity-declaring device. |
| Scatter-write to cache position t has off-by-one → silent wrong attention | High — garbage output, no crash | KAT oracle catches this immediately: cached ≠ uncached at the first affected step. Also add position-index assertion in debug builds. |
| RoPE rotation on cached K must match rotation applied at write time | Medium — if RoPE is applied at read instead of write (or vice versa), attention scores are wrong | llama.cpp applies RoPE to K at write time (`llama-graph.cpp:2340`), then reads rotated K from cache. Follow this pattern exactly. Document the invariant: "RoPE applied before cache store, never at cache read." |
| Daemon evicts KV mid-decode → recompute path is untested | Medium — 8.5b may not have a working recompute-from-hidden-state path | Scope 8.5b conservatively: register KV slots but set `hot` flag to prevent eviction during the first implementation. Remove the protection only after recompute-on-eviction is wired. |

---

## 10. PM/PL ratification (2026-06-16)

Ratified by the PL on PM recommendation:
- **OD-1 KV precision → f32.** f16/q8_K KV deferred to Phase 9+ (keep 8.5 on the f32 path; don't compound integration risk).
- **OD-2 Recompute-on-eviction → (a) from hidden states** — but **built with eviction (8.5b), not in 8.5** (see the scope call).
- **OD-3 → single-stream** for 8.5; multi-stream is Phase-10+.
- **OD-4 → register KV at pre-reserve** (full lifecycle under governance from generation start).
- **OD-5 segment size → deferred** (256 tokens tentative; final value locked at the Phase-9+ segment-eviction build).
- **Scope call (load-bearing) — active residency eviction/prefetch is DEFERRED out of 8.5.** At Qwen2.5-0.5B / 4096 the full KV cache (~96–100 MiB) fits the arena, so eviction is *not needed for correctness* — it is the *showcase*, built when real memory pressure exists (larger model/context). **8.5 ships: pre-reserved, governed KV (Option A) + the cached forward pass + real token generation + the cached==uncached equivalence KAT + the `kv_cache_enabled` kill-switch (disabled ≡ recompute-all ≡ 8.5a).** Pre-reserved-under-governance *already* demonstrates "KV is governed kernel memory" (the VISION thesis); **8.5b** adds the active-eviction/recompute demonstration later; **8.5c** (segment eviction) stays design-only.

These feed the BUILD-8.5 dispatch (authored once 8.4 lands).
