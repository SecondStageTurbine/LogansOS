<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Forward-path F2 — model config data-driven from `.neom` (kernel) → Janus

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Janus (main-tier kernel). **GATED ON F1** (Jarvis, `codex/forward-path-f1-mixed-quant-maincoder`) — F2 shares `forward_logic.rs` + the dtype/weights infra F1 introduces, so **branch off `origin/main` AFTER F1 merges** (`codex/forward-path-f2-config-from-neom-janus`), or coordinate with Jarvis to continue from F1. **Do not start on top of pre-F1 main** — you'll collide on `forward_logic.rs`.
**Status:** DISPATCHED (F1-gated) — **PL-ratified D2 (forward-path THINK `cf50ffb`).** The kernel Qwen2 config is **hardcoded to 0.5B consts** (`QWEN25_05B_HIDDEN_SIZE=896`, etc.) — it cannot run the 1.5B (or any variant) without recompiling. F2 makes the config **data-driven from the `.neom`**: tensor shapes from the inventory + the 3 scalar keys L1 just landed in the v2 header. **L1's dependency is satisfied** (`3a652eb`: `layer_count`/`rope_theta`/`rms_norm_eps` are in the `.neom` v2 header). Config-only — graph topology unchanged (OT-16 frozen).

## 0. The decision this build makes
Turn "the forward graph only runs 0.5B because its dimensions are `const`" into "**the forward graph reads its `Qwen2LayerConfig` from the model object**, so 0.5B and 1.5B both run from the same kernel" — by deriving shapes from the `.neom` tensor inventory and reading `layer_count`/`rope_theta`/`rms_norm_eps` from the v2 header (L1). **Values only; the graph topology does not change (OT-16 frozen).**

## 1. References (read before code)
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-21-forward-path-real-model-integration.md  # D2 (ratified) + §3 — the two-source derivation; the exact fields
git show origin/main:kernel/src/ai/forward_logic.rs        # QWEN25_05B_* consts; Qwen2LayerConfig + qwen25_05b(); the consumers (attention_seam, swiglu_ffn, gqa_*, Qwen2KvCacheSet::new, qwen2_output_logits)
git show origin/main:kernel/src/ai/model_logic.rs          # the .neom loader — where ModelConfig is constructed from the model object at load time
git show origin/main:tools/neomc/src/format.rs             # the .neom v2 header layout (L1, 3a652eb): HEADER_SIZE=108, layer_count@96 (u32), rope_theta@100 (f32), rms_norm_eps@104 (f32) — the kernel reader must match these offsets exactly
git show origin/main:tools/neomc/src/gguf_import.rs         # the tensor inventory (dims per tensor) — the shape source: token_embd [hidden, vocab]; blk.0.attn_q [hidden, q_heads*head_dim]; etc.
# F1 (prereq): forward_logic.rs gains TensorDtype + per-tensor dtype on Qwen2LayerWeights — F2 rebases on that
```

## 2. ISCs
- **ISC-1 — `ModelConfig` struct.** Add a `ModelConfig` carrying all shape + scalar fields: `hidden_size`, `intermediate_size`, `layer_count`, `q_heads`, `kv_heads`, `head_dim`, `context`, `vocab`, `rope_theta`, `rms_norm_eps`.
- **ISC-2 — Two-source derivation (D2).** **Shapes** from the `.neom` tensor inventory: `hidden_size`/`vocab` from `token_embd.weight` dims; `q_heads*head_dim` & `kv_heads*head_dim` from the attn_q/attn_k dims; `intermediate_size` from the ffn dims. **Scalars** (`layer_count`, `rope_theta`, `rms_norm_eps`) read from the **`.neom` v2 header at the L1 offsets** (96/100/104; match `format.rs` exactly — independent byte read, not a guess). Construct `ModelConfig` in the loader (`model_logic.rs`) at model-load time.
- **ISC-3 — Replace the consts with `from_model_config()`.** `Qwen2LayerConfig` is built from `ModelConfig` (`from_model_config()`), not the hardcoded `qwen25_05b()`. **The hardcoded consts remain as the fixture default** (used when the kill-switch is off / for `arthur_boots_thinking`).
- **ISC-4 — kill-switch `phase9-data-driven-config` default-OFF ≡ hardcoded consts bit-for-bit.** Off ⇒ the existing 0.5B consts drive the graph (fixture scenario green bit-for-bit — the equality anchor). On ⇒ config comes from the `.neom`. **The equality proof is the headline KAT:** load the 0.5B `.neom`, derive `ModelConfig`, assert **every field == the hardcoded 0.5B const**. Then load a **1.5B-shaped `.neom` fixture**, assert the derived config has the correct *distinct* 1.5B values (hidden=1536, intermediate=8192, layers=28, q_heads=12, kv_heads=4, head_dim=128 — confirm against the THINK/source). **No new invariant** (OT-16 frozen — values only, topology unchanged; state this). **No new `unsafe`.**
- **ISC-5 — standards + gate.** SPDX. `cargo fmt --all --check` (**run it**), workspace test (exclude kernel), `cargo clippy --workspace --all-targets -- -D warnings`, kernel clippy blocking — via Docker. **QEMU scenario:** feature OFF ⇒ the F1 fixture/`arthur_boots_thinking` green bit-for-bit; feature ON ⇒ the real-config 0.5B (on the F1 mini mixed-quant fixture, or the real `.neom` if L2-served) derives config + produces a sane token. At `NEO_SMP=4`. ADR/ANSPEC note: config is now data-driven (D2; OT-16 still frozen — config ≠ topology).

## 3. Scope guard
Config-from-`.neom` (shape derivation + v2-header scalars + `ModelConfig` + `from_model_config`) **only**. **Do NOT**: rewrite the forward graph / change topology (OT-16 frozen — this is *values*, not a new graph); change the `.neom` format or the importer (L1 frozen — F2 *reads* the v2 header); touch the mixed-quant matmul (F1) beyond rebasing on it; touch the vocab/tokenizer (F3). No new invariant/`unsafe`.

## 4. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase onto current `main` **and onto F1** LAST; full host-checks incl. `cargo fmt --all --check`; SPDX; self-stat). Two hard rules: **green CI URL** (push-CI + the 4-shard QEMU `workflow_dispatch` URL — kernel-touching) **— paste the URL**; PM re-inspects `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**OT-28: F2 done — config data-driven, 1.5B runnable; with F1+F3 the real brain runs end-to-end → Stage-B**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-forward-path-f2-config-from-neom-janus.md` (+ the THINK D2, `forward_logic.rs`, `model_logic.rs`, `neomc/src/format.rs` for the v2 offsets). **Branch:** `codex/forward-path-f2-config-from-neom-janus` off `origin/main` **AFTER F1 (Jarvis) merges** — F2 shares `forward_logic.rs`, so rebase on F1; don't start on pre-F1 main.
- **One-line:** Build **OT-28 F2 — config data-driven from `.neom`** (PL-ratified D2): the Qwen2 config is **hardcoded 0.5B consts** so 1.5B can't run; derive a `ModelConfig` from the model object — **shapes** from the tensor inventory (`token_embd`→hidden/vocab; `attn_q/k`→q/kv_heads*head_dim; ffn→intermediate) + the **3 scalars** (`layer_count`/`rope_theta`/`rms_norm_eps`) from the **`.neom` v2 header at offsets 96/100/104** (L1 `3a652eb`, match `format.rs` exactly). Build `Qwen2LayerConfig::from_model_config()`; **keep the hardcoded consts as the fixture default**. **Kill-switch `phase9-data-driven-config` default-OFF ≡ hardcoded consts bit-for-bit** — headline KAT: load 0.5B `.neom` → derive → assert every field == the const; load a 1.5B-shaped fixture → assert correct distinct values. **OT-16 stays frozen — values only, graph topology unchanged; no new invariant/`unsafe`.** QEMU @ NEO_SMP=4 (off ≡ F1 fixture bit-for-bit; on derives config + sane token). **GATED ON F1 — rebase on Jarvis's `forward_logic.rs` dtype work; run `cargo fmt --all --check`.** **Scope: config derivation ONLY — no graph rewrite (OT-16), no `.neom`/importer change (L1), no matmul (F1) or vocab (F3) changes.** **Close-gate: `scripts/worker-close-gate.sh` (rebase on main+F1 LAST; full host set incl. fmt); RETURN the green push-CI + QEMU URLs — paste the URL.**
