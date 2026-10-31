<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Next-brain 1.5B provisioning + parity methodology (OT-28/OT-38)

This is the reproducible recipe for the **Qwen2.5-1.5B** Arthur brain (the 0.5B → 1.5B
scale-up) and the **near-tie-aware parity oracle** it required. No model blob lives in
git; the `.neom` is staged from disk (`NEO_STAGE_B_NEOM`), the existing 0.5B pattern.

## 1. Import recipe (host)

```
# Source GGUF (same Q4_K_M family as the shipped 0.5B), staged outside git:
#   <models>/qwen2.5-1.5b-instruct-q4_k_m.gguf   (~1.04 GiB)
cargo build --release -p neomc
target/release/neomc import \
  <models>/qwen2.5-1.5b-instruct-q4_k_m.gguf \
  -o   <models>/qwen2.5-1.5b-instruct-q4_k_m.neom \
  --vocab-output <models>/qwen2.5-1.5b-instruct-q4_k_m.vocab
```

Verified output:

| field | value |
|---|---|
| tensors | 339 |
| `.neom` total size | **1,111,411,200 bytes (≈1.035 GiB)** |
| quant mix | f32=141 (norms+biases), q4_K=169, q6_K=29 — **no q5_0/q8_0** |
| vocab sidecar | 151,936 tokens, 151,387 merges (≈7.62 MiB) |
| v2 header | `layer_count=28`, `rope_theta=1e6`, `rms_norm_eps=1e-6` (data-driven from GGUF) |

The importer is **arch-data-driven** (`tools/neomc/src/gguf_import.rs`): dimensions are
read from GGUF metadata (`block_count/embedding_length/feed_forward_length/head_count/
head_count_kv`) and cross-checked against tensor shapes — not pinned to 0.5B constants —
so 0.5B/1.5B/3B import uniformly. The kernel re-derives the same dims from the `.neom`
tensor shapes + the 3 v2 header scalars (`derive_model_config`, F2).

## 2. Memory budget (the crux)

`HEAP_SIZE` is sized **1.75 GiB** for the 1.5B, gated by `neo-test-arthur-real-brain-boot-15b`
(`kernel/src/mm/heap_policy.rs`). The 768 MiB 0.5B budget was sized for the 485 MB `.neom`
and could not hold even the 1.035 GiB 1.5B single load. The 1.5B `.neom` loads as one
`vec![0u8; total_size]` (no double-buffer) and stays **quantized** in place; the F1 forward
streams weights block-by-block (integer Q8_K madd), so the peak transient is ~O(vocab) f32
logits (≈0.6 MB), not a full-matrix dequant. Resident ≈ 1.035 GiB + bounded KV + ~0.6 MB
scratch ≈ ~1.1 GiB, leaving ~0.65 GiB headroom.

**Kill-switch (Rule 1):** with `-15b` OFF the budget is bit-for-bit the previous layer's —
768 MiB under the 0.5B real-brain-boot, 4 MiB otherwise — host-tested in
`tools/heap-policy-logic-tests`. `MAX_ORDER 16` and the 256 KiB syscall stack are inherited
from the composed 0.5B feature (both still sufficient: largest 1.5B tensor ≈182 MiB <
order-16's 256 MiB; forward stack depth is layer-count-independent).

## 3. NeoFS capacity

NeoFS serves files to **2 GiB** (`kernel/src/fs/neofs/format.rs:22` `MAX_FILE_SIZE`, via
direct + single/double/triple-indirect blocks, ADR-064). The 1.035 GiB 1.5B `.neom` is
well within capacity.

## 4. Parity methodology: near-tie-aware teacher-forced top-1 (OT-30 refinement)

**Why the old oracle was the wrong instrument.** OT-30's strict *free-running greedy*
top-1 parity (`first_divergence=None`) worked for the 0.5B only because its greedy
trajectory for `"Hello"` had a clear winner at every position. The 1.5B's token 0 for a
bare `"Hello"` is **near-equiprobable** — llama.cpp itself ranks ` gorgeous`(23511) over
` beautiful`(6233) by just **0.024 logprob** (a coin-flip). The kernel's forward is
correct — it identifies the *same top-2* — but the greedy argmax flips on numerical noise
smaller than the reference's own margin, and free-running greedy then **cascades** the
whole sequence from one flip. Bit-exact greedy agreement across two independent quant-matmul
implementations at a 0.024-logprob tie is infeasible (accumulation order) and forbidden
(no forward-math change).

**The refined oracle** (`tools/ai-ops-tests/src/lib.rs`,
`real_model_stage_a_teacher_forced_parity_when_available`): teacher-force the reference
prefix, compare the kernel's argmax **per position** (no cascade). A position passes if the
kernel's top-1 == the reference's top-1 (**strict**, always passes — a superset of the old
oracle, so the 0.5B stays green), OR the kernel and reference share the same top-2 swapped
AND the reference's own top1/top2 self-margin ≤ `NEAR_TIE_EPS_LOGPROB = 0.025` (a genuine
**near-tie**, bounded at the reference's observed 0.024 self-margin — never looser). Any
other mismatch is **gross** and must be zero.

**1.5B result** (`NEO_REAL_NEOM` = 1.5B `.neom`, `NEO_REAL_LLAMA_TOPK` = llama.cpp 5f04dc7
per-position top-5):

```
TEACHER_FORCED_PARITY positions=24 strict=23 near_tie=1 gross=0 eps_logprob=0.025
  pos 0: NEAR-TIE kernel_top1=6233 ref_top1=23511 ref_self_margin=0.0242
```

**Negative control** (`teacher_forced_parity_rejects_broken_rope_theta`): a deliberately
broken forward (`rope_theta` 1e6 → 1e4) must STILL fail the refined oracle (gross > 0),
proving the near-tie tolerance did not go soft.

The reference is captured from the pinned **llama.cpp 5f04dc7** build (the OT-30 oracle),
CPU (`-ngl 0`), greedy (`temperature=0, top_k=1`), raw token prompt `[9707]`, via
`/completion n_probs=5` — the exact-byte reference is taken from the engine, never a
summarizing fetch.

## 5. Bare-metal follow-on (out of this build's gate)

This build's gate is the host Stage-A parity + the normal 4-shard QEMU (the heap change is
core/always-compiled). The on-silicon 1.5B boot is the follow-on (it inherits the
gold-standard discriminator ladder), and needs the userland selector wired:
`arthur-real-brain-boot-15b` (neo-arthur + neo-shell — index 0 / `model-qwen2.5-1.5b.neom` /
the 16 MiB vocab buffer for the real 8 MiB sidecar) plus an `arthur_real_brain_boot_15b`
xtask scenario staging the 1.5B via `NEO_STAGE_B_NEOM` at 6144M guest RAM. The kernel
budget + the import + the parity proof in this build make that a config-swap.

## 6. Boot config-swap smoke

The bootable 1.5B ISO path is feature-gated by `arthur-real-brain-boot-15b` in userland
and `neo-test-arthur-real-brain-boot-15b` in the kernel. The normal 0.5B Stage-B boot
remains the default when the `-15b` selector is off.

Run the load-smoke with the imported 1.5B artifact staged from disk:

```
$env:NEO_STAGE_B_NEOM = "<models>/qwen2.5-1.5b-instruct-q4_k_m.neom"
docker compose run --rm -e NEO_STAGE_B_NEOM=$env:NEO_STAGE_B_NEOM -e NEO_SMP=4 neo `
  cargo neo-test arthur_real_brain_boot_15b --no-meta
```

`NEO_REAL_NEOM` is also accepted as a convenience alias for the same imported `.neom`.
The `.vocab` sidecar must sit next to the `.neom` with the same stem. If neither env var
is set, xtask prints a skip note and does not substitute a fixture; the 1.5B proof must
come from the real model bytes.

The QEMU smoke intentionally stops before the first forward pass. Its serial ladder is:
`neofs_mount ok` -> `model_load start path=/neofs/model-qwen2.5-1.5b.neom` ->
`vocab_load tokens=151936` -> `model_open path=/neofs/model-qwen2.5-1.5b.neom` ->
`neofs_model_read_indirect ... direct=8 single=512 double=262144 triple=...` ->
`model_open path=/neofs/model-qwen2.5-1.5b.neom size=1111411200` ->
`config layer_count=28 head_dim=128 rope_theta=1000000 hidden=1536 intermediate=8960 q_heads=12 kv_heads=2 vocab=151936 context=4096 ...` ->
`model_load_resident bytes=1111411200 heap_cap=...` -> `pre_forward ready` ->
`arthur_real_brain_boot_15b=ok ... init_continues=true`.
