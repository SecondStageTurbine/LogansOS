<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Next-brain 3B provisioning (OT-38)

The reproducible recipe for the **Qwen2.5-3B** Arthur brain — the config-swap that
follows the landed 1.5B (`04bc459` + `6f6486c`). Same Qwen2 architecture family, **zero new
forward math and zero model-conditional kernel branches**: a 3B is a new `.neom` + a new
config tier + a measured heap budget + a load-smoke scenario, not new kernel logic. No model
blob lives in git; the `.neom` is staged from disk (`NEO_STAGE_B_NEOM`), the existing
0.5B/1.5B pattern. The `-3b` feature is a **sibling** of `-15b` (both compose the 0.5B path +
data-driven config F2), never a replacement.

## 1. Import recipe (host)

```
# Source GGUF (same Q4_K_M family as the shipped 0.5B/1.5B), staged outside git:
#   <models>/qwen2.5-3b-instruct-q4_k_m.gguf   (2,104,932,768 bytes, from
#   huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF — pulled raw, no summarizing fetch)
cargo build --release -p neomc
target/release/neomc import \
  <models>/qwen2.5-3b-instruct-q4_k_m.gguf \
  -o   <models>/qwen2.5-3b-instruct-q4_k_m.neom \
  --vocab-output <models>/qwen2.5-3b-instruct-q4_k_m.vocab
```

Verified output (the byte-oracle record; hardcoded in `tools/xtask/src/main.rs` and
`tools/heap-policy-logic-tests`):

| field | value |
|---|---|
| tensors | 435 |
| `.neom` total size | **2,099,030,016 bytes (≈1.955 GiB)** |
| weight bytes | 2,098,976,768 |
| quant mix | f32=181 (norms+biases), q4_K=217, q6_K=37 — **no q5_0/q8_0/q8_K** |
| vocab sidecar | 151,936 tokens, 151,387 merges (**7,990,862 bytes** — byte-identical to the 0.5B/1.5B sidecar; Qwen2.5 shares one tokenizer across sizes) |
| derived config (F2, from the `.neom` header + tensor shapes) | `layer_count=36 head_dim=128 rope_theta=1e6 hidden=2048 intermediate=11008 q_heads=16 kv_heads=2 vocab=151936 context=4096 rms_norm_eps=1e-6 (bits 0x358637bd)` |

The config values were cross-checked against the GGUF metadata (`gguf-py`:
`qwen2.block_count=36`, `embedding_length=2048`, `feed_forward_length=11008`,
`head_count=16`, `head_count_kv=2`, `rope.freq_base=1e6`, `layer_norm_rms_epsilon=1e-6`);
the kernel re-derives the same via `derive_model_config` (proven equal to the GGUF-declared
values by `neom_config_*_inventory_derives_*` in `tools/ai-ops-tests`). `context` is the
model-independent `QWEN2_DEFAULT_CONTEXT = 4096` (native 32768 is clamped, same as 1.5B).

The importer is **arch-data-driven** (`tools/neomc/src/gguf_import.rs`): dimensions come from
GGUF metadata cross-checked against tensor shapes, so 0.5B/1.5B/3B import uniformly with no
3B-specific code change (none was needed).

## 2. Memory budget (the crux — measured, not guessed)

`HEAP_SIZE` is sized **2.75 GiB (2816 MiB)** for the 3B, gated by
`neo-test-arthur-real-brain-boot-3b` (`kernel/src/mm/heap_policy.rs`). Honest resident need
for a full forward:

- **Weights:** the whole `.neom` loads as one `vec![0u8; total_size]` — **2,099,030,016 B
  (2001.79 MiB)** — and stays **quantized in place** (the F1 forward streams weights
  block-by-block as integer Q8_K madd; the peak transient is ~O(vocab) f32 logits ≈ 0.6 MiB,
  not a full-matrix dequant).
- **KV cache** (full context 4096): 2 (K+V) × 36 layers × 4096 × 2 kv-heads × 128 head-dim ×
  4 (f32) = **301,989,888 B (288 MiB)**.
- **Forward scratch + kernel heap rest:** a few MiB.

So resident ≈ 2002 + 288 + ε ≈ **~2.24 GiB**, and the 2.75 GiB budget leaves **~814 MiB
headroom over the weights** (over the ≥512 MiB floor enforced by
`tools/heap-policy-logic-tests`). Budget history: 768 MiB (0.5B) → 1.75 GiB (1.5B) → 2.75 GiB
(3B).

**Kill-switch (Rule 1):** with `-3b` OFF the budget is bit-for-bit the previous layer's —
4 MiB / 768 MiB / 1.75 GiB depending on the lower flags — host-tested in
`tools/heap-policy-logic-tests` (`kill_switch_3b_off_is_previous_budget_bit_for_bit`). The
larger tier always wins (`three_b_budget_selected_and_dominant`).

**MAX_ORDER 16 (256 MiB) still suffices.** The largest single tensor is `output.weight` (3B
does **not** tie embeddings — a separate q6_K output projection): **255,252,480 B (243.43
MiB) < 256 MiB** (margin ~12.6 MiB; `token_embd.weight` is 166.92 MiB q4_K). The 256 KiB
syscall stack is inherited from the composed 0.5B feature (forward stack depth is
layer-count-independent).

**Guest RAM (a flagged harness change, not smuggled).** The 3B load-smoke runs QEMU at
**`-m 8192M`** (vs the 1.5B's 6144M): the 2.75 GiB heap plus the 1.955 GiB heap-resident
`.neom` need more than 6 GiB; 8 GiB leaves ~5.4 GiB for the kernel image, page tables,
disk-DMA, and framebuffer, exceeding the 1.5B's proven ~4.35 GiB overhead margin. This fits
the runner-capacity floor (Oracle's prequel raised host RAM; the pre-flight guard requires
≥32 GiB per shard, comfortably covering one 8 GiB guest).

## 3. NeoFS capacity (tight — verified)

NeoFS and the kernel model loader both cap files at **2 GiB** (`fs/neofs/format.rs:22`
`MAX_FILE_SIZE`; `ai/model_logic.rs:23` `NEOM_MAX_FILE_SIZE`; ADR-064). The 3B `.neom` is
**2,099,030,016 B, i.e. 48,453,632 B (~46 MiB) under the 2 GiB cap** — it fits, but the
margin is thin: a larger quant (Q5/Q6) would **not** fit, so Q4_K_M is the right (and only
comfortable) choice at this limit. The staging disk is sized 3 GiB (`Some(3072)`) to hold the
`.neom` + the ~7.6 MiB vocab + the neofs indirect/inode overhead (direct 8 + single 512 +
double 262144 + triple 249795 data blocks) with margin.

## 4. Boot config-swap smoke

Feature-gated by `arthur-real-brain-boot-3b` (userland) and
`neo-test-arthur-real-brain-boot-3b` (kernel). The normal 0.5B Stage-B boot remains the
default when the `-3b` selector is off; the 1.5B path is unaffected (both are siblings). No
Stage-A parity oracle is in this build's scope — this is a **load** smoke that stops before
the first forward pass (the on-silicon 3B forward + parity is the OT-7 bare-metal follow-on).

Run the load-smoke with the imported 3B artifact staged from disk:

```
# The `neo` service bind-mounts only the workspace, so the model (staged outside
# git) must be mounted in and NEO_STAGE_B_NEOM must point at the *container* path
# — a bare host path passed via `-e` does not resolve inside the Linux container.
docker compose run --rm `
  -v "<models>:/models:ro" `
  -e "NEO_STAGE_B_NEOM=/models/qwen2.5-3b-instruct-q4_k_m.neom" `
  -e "NEO_SMP=4" neo `
  cargo neo-test arthur_real_brain_boot_3b --no-meta
```

`NEO_REAL_NEOM` is also accepted as an alias for the same imported `.neom`. The `.vocab`
sidecar must sit next to the `.neom` (same stem); xtask auto-stages it as
`model-qwen2.5-3b.vocab`. If neither env var is set, xtask prints a skip note and does not
substitute a fixture — the 3B proof must come from the real model bytes (so the scenario
skips in CI, where the blob is not in git; the mandatory 4-shard QEMU gate exercises the
existing scenarios, which are bit-for-bit unaffected with `-3b` off).

The QEMU smoke intentionally stops before the first forward pass. Its serial ladder is:
`neofs_mount ok` → `model_load start path=/neofs/model-qwen2.5-3b.neom` →
`vocab_load tokens=151936` → `model_open path=/neofs/model-qwen2.5-3b.neom` →
`neofs_model_read_indirect ... direct=8 single=512 double=262144 triple=...` →
`model_open path=/neofs/model-qwen2.5-3b.neom size=2099030016` →
`config layer_count=36 head_dim=128 rope_theta=1000000 hidden=2048 intermediate=11008 q_heads=16 kv_heads=2 vocab=151936 context=4096 ...` →
`model_load_resident bytes=2099030016 heap_cap=...` → `pre_forward ready` →
`arthur_real_brain_boot_3b=ok ... init_continues=true`.

**Measured run (2026-07-05, quiet host).** A clean local PASS took **`qemu elapsed=3222.8s`**
against the original 3600 s budget (**90% consumed**, ~6 min margin), with
`model_load_resident … ms=2666470` — the 2 GB `.neom` read via the NeoFS triple-indirect
tree (512,459 data blocks) alone is ~44 min under pure TCG and is the dominant cost. Every
oracle value matched (config bits `0x358637bd`, `heap_cap=2952790016` = 2.75 GiB, resident
`bytes=2099030016`). The budget is **kept at 3600 s** to mirror the 15B:
`scenario_shard_weight` in `tools/xtask` *is* this timeout, so a larger value perturbs the CI
shard-partition balancer and collides two timeout-heavy scenarios on one shard (the
runner-capacity invariant asserted by `shard_partition_spreads_timeout_heavy_scenarios`). The
90% margin therefore only bounds **local** runs — the scenario **SKIPs in CI** (blob not in
git, `NEO_STAGE_B_NEOM` unset) — so a slower or loaded box that times out should simply re-run.

## 5. Model license note

Unlike the Apache-2.0 0.5B/1.5B, **Qwen2.5-3B is under the Qwen Research License**
(non-commercial). It is used here purely as a staged test artifact for a research OS boot
smoke; no model bytes enter git. 3B is the PL-ratified OT-38 capability step — recorded here
for the audit trail, not a licensing decision made in this build.
