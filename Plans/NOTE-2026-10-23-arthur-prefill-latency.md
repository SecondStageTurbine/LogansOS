<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: Arthur answer latency on the Zenbook — where prefill time goes, and a ranked plan

**Date:** 2026-10-23 · **Author:** S4 (Claude) · **Lane:** diagnosis (PM, 2026-10-23). The deliverable is this note,
a ranked plan, and default-off instrumentation (`prefill-perf`).

**Update (PM routing, same day): fixes 1–3 are built in the same PR (#105).** See "Fixes 1–3 as built" at the
end. Fixes 4–7 stay out: fix 4 is concurrency core, and gets measured first, on silicon, with `prefill-perf`.

## The symptom

From the PL's photos of image A (main `cb165c11`, 16 CPUs, `aps alive=15/15`):
- the question was typed at watchdog `t≈90 s`;
- prefill was still running at `t=112` and `t=216` (`gen#1 step 0/0`);
- `gen-done` (step 7/32) was showing by `t=311`.

Over that span, `matmul jobs` went 207 → 2,746 → 4,235. The OptiPlex also took 3–4 minutes per answer. The machine
stays responsive throughout; it is slow, not frozen.

**Converting the counters into forwards.** `par_matmul` forks only matmuls with ≥ 512 output columns: attn_q,
attn_o, ffn_gate, ffn_up and ffn_down per layer, plus lm_head. That is 5 × 24 + 1 = **121 jobs per forward**.
4,235 jobs ≈ **35 forwards ≈ 28 prompt tokens + 7 decoded tokens**.

The prompt is short. Arthur frames one Qwen chat turn around the user's text
(`userland/neo-arthur/src/vocab_sidecar.rs:231`, system prompt `"You are a helpful assistant."`), which is about
20 tokens of framing plus the question.

So the Zenbook spends about **5–7 s per forward pass, with 16 CPUs**. For a Q4_K_M 0.5B model, which reads about
0.4 GB of weights per token, the memory-bandwidth floor is around 10 ms per token. **We are two to three orders of
magnitude off the floor.** That means compute overhead, not data volume.

## Findings

Each finding is checked against the source at `5e40fd31`.

1. **The product kernel is built at opt-level 0.** `xtask iso --with-model` calls `build_kernel_default`, which
   builds the `dev` profile (`tools/xtask/src/main.rs`, `build_member`; the image is `target/.../debug/kernel`).
   The root `Cargo.toml` raises opt-level for a few crates (blake3, neo-atrium, the crypto crates, neo-text) but
   **not for the kernel**. Every inference loop therefore runs unoptimized, with overflow checks and debug
   assertions. The AVX2 kernels run at opt 0 too: their intrinsics are not inlined and pass through the stack.
   There is precedent for raising it: `build_member` already sets `CARGO_PROFILE_DEV_OPT_LEVEL=2` for the
   `qwen35` labels.

2. **AVX2 detection runs `CPUID` + `XGETBV` on every dot product.** `ops/qk_matmul.rs` `avx2::runtime_available()`
   is called inside each `vec_dot_*_with_kernel`, i.e. once per output column. That is about 450,000 calls per
   token: 24 layers × ~12.6k columns, plus 151,936 for the lm_head. `CPUID` is a serializing instruction, about
   100–200 cycles on bare metal. Under a hypervisor (KVM CI runners, cloud VMs) it is a VM exit; it measured
   about 7 µs here.

3. **Prefill runs token by token, and computes full logits for every prompt token.**
   `InventoryLogitsSource::prefill` (`arch/x86_64/syscall.rs`) calls `InventoryDecodeState::advance` once per
   prompt token. Each call runs the vocab-sized lm_head (896 × 151,936, Q8_0, about 145 MB) and the finiteness
   scan over 151,936 logits. Only the last prompt token's logits are used.

4. **The fork-join has no measurement on the target.** APs pick up partitions from a busy-yield idle loop
   (`sched::scheduler::run_ap_bootstrap` → `poll_ap_matmul_work` → `yield_now`). An AP that is running another
   thread (on the desktop: neo-init's compositor, neo-atrium, neo-arthur) does not poll until it returns there.
   The BSP then waits `STEAL_GRACE_SPINS` = 2,000,000 spins for that slot before stealing it. It does this slot by
   slot, and the spins themselves are unoptimized. The partitions are also equal-width across all online CPUs,
   so the job waits for the slowest core (LP-E cores included).

   Nothing today counts how often the BSP steals or how long it waits. The Zenbook evidence (16 CPUs ≈ 1 host
   thread) says the threading is buying little. **This is a hypothesis, not a measurement**; `prefill-perf` is
   what measures it.

5. **The non-matvec layer work is software float.** Attention, RoPE, SwiGLU, RMSNorm and softmax go through
   `ops::soft_f32`, integer-emulated IEEE f32 kept for determinism. At the short contexts measured below, it is a
   small share of a layer. It grows linearly with position, per token.

6. **The model never had to be dequantized per token.** The weights stay in their GGUF blocks (`Arc<Tensor>`), and
   the matvec quantizes the activation to Q8 once. There is no per-token copy of the weights.

## Measurements

**Rig.** These are host measurements on this cloud container: 4 vCPU Intel Xeon @ 2.1 GHz, a VM. The real
`qwen2.5-0.5b-instruct-q4_k_m.gguf` could not be fetched, because the environment's network policy denies
`huggingface.co`. So the code under test is the **production `kernel/src/ai` code**, `#[path]`-mounted on the
host (as `tools/ai-ops-tests` does). It runs at the real Qwen2.5-0.5B shapes, with **synthetic quantized weight
bytes**. The kernels' timing doesn't depend on the weight values; the f16 scale fields are pinned to finite
values. Everything is single-threaded.

**The dtype mix is an assumption.** Hidden size 896 is not a multiple of 256, so matrices with 896 input columns
cannot use K-quants; llama.cpp's Q4_K_M falls back to Q5_0/Q8_0 for them. The assumed mix is:
- attn_q, attn_k, attn_o, ffn_gate, ffn_up: Q5_0;
- attn_v: Q8_0;
- ffn_down (input 4864 = 19 × 256): half Q4_K, half Q6_K;
- lm_head (tied to token_embd): Q8_0.

Confirm it on the PL's machine with `neomc inspect model-qwen2.5-0.5b.neom`.

**Reproduce:** `tools/inference-latency-bench/run.sh`. It is host-only and excluded from the workspace, like
`tools/bench-harness`. It mounts the kernel's `ai` sources by path. (The tables in this section were taken before the
fixes, against a scratch copy of `ops/` with the cached decision. Since the fixes landed, `run.sh` compares the
shipped and fixed code directly: see the end of this note.)

**Kernel level: one matvec per tensor, per-token totals.** "opt 0" is the `dev` profile the image ships.
"cached" means a copy of `ops/` whose `runtime_available()` caches its result once in an atomic: fix 2.

| tensor (per token) | opt 0 | opt 0, cached | opt 3 | opt 3, cached |
|---|---|---|---|---|
| attn_q 896×896 Q5_0 ×24 | 211 ms | 238 ms | 5.6 ms | 4.6 ms |
| attn_o 896×896 Q5_0 ×24 | 290 ms | 223 ms | 5.1 ms | 4.6 ms |
| ffn_gate + ffn_up 896×4864 Q5_0 ×48 | 2,284 ms | 2,472 ms | 53.5 ms | 54.3 ms |
| ffn_down 4864×896 Q4_K/Q6_K ×24 | 1,089 ms | 731 ms | 294 ms | 30.8 ms |
| attn_k/attn_v 896×128 ×48 | 98 ms | 63 ms | 44 ms | 1.1 ms |
| lm_head 896×151,936 Q8_0 ×1 | 3,106 ms | 987 ms | 2,172 ms | 50.4 ms |
| **total** | **7,078 ms** | **4,714 ms** | **2,575 ms** | **146 ms** |

The "opt 3, uncached" column is dominated by trapped `CPUID` on this VM (about 7 µs per call). On bare metal that
column lands near the cached one.

**Layer level: the real `forward_layer_with_gqa_attention_with_norms` and `qwen2_output_logits`.** Per token
means 24 layers plus the lm_head; positions run from 0 to 8 (opt 0) or 0 to 32 (opt 3).

| build | one layer | lm_head | per token | vs shipped |
|---|---|---|---|---|
| shipped: opt 0, `CPUID` per dot | 170 ms | 3,086 ms | 7.2 s | 1× |
| opt 0, cached | 135 ms | 1,023 ms | 4.3 s | 1.7× |
| opt 3, uncached | 22 ms | 2,323 ms | 2.9 s | 2.5× (VM-inflated) |
| **opt 3, cached** | **5.5 ms** | **54 ms** | **0.19 s** | **~38×** |

At opt 3 + cached, the lm_head is **29%** of a forward. Skipping it for all but the last of 32 prompt tokens takes
that prefill from 6.0 s to 4.3 s on this rig. The layer time is within a few percent of its matvec time, so at
these short contexts the soft-float work is small.

**Scaling to the Zenbook.** The Zenbook's ~5–7 s per forward on 16 CPUs is about this rig's 4.3–7.2 s single-thread
opt-0 number. Faster cores should have made it quicker, and 16-way threading should have cut it ~10×; neither
shows. That is consistent with findings 1 and 4 together. With opt 3 + cached detection, single-thread lands at
about 0.19 s per token on this slower rig. That is **~5 s for a 28-token prefill before any threading**, against
about 150–200 s today.

## Ranked plan: expected speedup per unit of risk

| # | Fix | Expected effect | Risk / blast radius | Proof measurement |
|---|---|---|---|---|
| 1 | Build the **product image's kernel at opt-level 3**: `CARGO_PROFILE_DEV_OPT_LEVEL=3` for the `iso --with-model` kernel build only. This is the `build_member` precedent and keeps overflow checks and debug assertions on. A later step is a `release`-profile product kernel. | ~25–30× on the forward (measured 32× on the matvecs, cached) | Image-only build flag. No source change. Stack use usually shrinks at opt 3. It must pass the product scenarios (`arthur_real_brain_boot`, `product_integration`) built the same way, and the Zenbook smoke. | `prefill-perf` line `pre … ms/t=` before and after, on the Zenbook. Host: the kbench table above, opt 0 → opt 3. |
| 2 | **Cache the AVX2 decision** once: one relaxed atomic in `avx2::runtime_available()`. | 1.7× at opt 0. At opt 3 it is the difference between 2.9 s and 0.19 s per token on a VM. On bare metal, about 450k `CPUID` per token (tens of ms per token at opt 3, comparable to a whole layer stack). | Tiny and local. Output is byte-identical (same kernel chosen). The detection is CPU-invariant after boot. Needs one look at the XCR0 assumption: XCR0 is set once at boot, before any inference. | Host: kbench `cached` vs not. Kernel: `prefill-perf` `ms/t`. A host KAT: the cached and uncached choice agree. |
| 3 | **Skip the lm_head for prompt tokens except the last** (an `advance_no_logits` for prefill). | −29% of prefill compute at opt 3; for a 28-token prompt, about 27/28 of the lm_head time. | Small, in `qwen_inventory`/`syscall.rs`. Must keep `sys_generate_kv_persist_equivalence` green: same KV, same last logits. The reprefill baseline is unaffected. | `prefill-perf` `lm=` share drops to about 1/n of prefill; equivalence KAT green. |
| 4 | **Make the fork-join pay:** measure first (`prefill-perf` `bar=` and `st=`). If steals and barrier time dominate: assign partitions only to CPUs idle in their poll loop, shrink the grace window, or wake APs (IPI) instead of waiting for a `yield_now` round. | Up to ~min(cores, bandwidth) on the matvec, about 4–8× on a laptop. Unknown until measured. | **Concurrency core** (CP-19…CP-28 class). It needs the pitfall-bank consult, a cross-family AAG, and the threaded-matmul parity scenario. | `bar=%` and `st=` near 0, `ap=` ≈ jobs × (parts − 1), `ms/t` scaling with cores. |
| 5 | **Batched prefill:** all prompt tokens through each layer at once, as a matrix × matrix, with causal attention over the batch. `dispatch_q8k_kernel` already handles `lhs_rows > 1`. | Weights read once per layer per prompt instead of once per token: up to ~10× on the matvec part of prefill once memory-bound (after 1–2). | Medium–high. It touches the attention and KV-cache write path, and needs a new prefill≡token-by-token equivalence KAT. Larger refactor; route after 1–4. | `pre ms/t` vs `dec ms/t` ratio; the new equivalence KAT. |
| 6 | **Vectorize the soft-float ops**, or a hardware-f32 path behind the determinism contract. | Small at today's contexts; grows linearly with context. | Touches the determinism contract (I-1). Out of rung. | `prefill-perf`: forward − `mv`, as context grows. |
| 7 | **Shorter system prompt.** | ≤ ~35% of prefill tokens: the framing is about 10 of 28. | Changes model behaviour (Qwen's default system turn). Low value after 1–3. | Prompt token count (`prompt_tokenize n=`). |

**Recommended order:** 1 + 2 together (one build, both tiny) → measure on the Zenbook with `prefill-perf` → 3 → 4,
sized by that measurement → 5.

## Instrumentation shipped with this note: `prefill-perf` (default off)

- **What it is:** a kernel feature, `prefill-perf = ["forward-perf", "threaded-matmul"]`, in **no image by
  default**. Relaxed TSC accumulators only: no lock, no park, no change to what the forward computes.
  `sys_generate_kv_persist_equivalence` passes with it on, fast≡slow and the negative control red.
- **Output:** a fourth line on the freeze-watchdog overlay when both features are on (the Zenbook's only
  channel). There is no serial line: the first cut printed one at generation end, which took the serial sink
  lock on the generation path (review item 19, CP-71). The overlay is painted lock-free by the watchdog AP.
  ```
  perf pre n=<prompt tokens> ms/t=<ms per prefill token> | dec n=<decoded> ms/t=<ms per decode token>
       | lm=<lm_head % of forward> mv=<quantized matvec % of forward, fork-join included>
       | par j=<fork-join jobs> bar=<BSP barrier % of forward> st=<partitions stolen back> ap=<partitions APs ran>
  ```
- **Reading one photo:**
  - `bar` high with `st` near `j × (parts−1)`: the APs are not polling, and the BSP is computing everything after
    waiting out the grace window. That is finding 4, and fix 4 is the work.
  - `mv` near 100%: the matvecs dominate. Fixes 1–3 and 5.
  - `mv` well below 100% at long contexts: the soft-float attention is growing. Fix 6.
- **To use it on the next Zenbook image:** add `prefill-perf` to the `iso --with-model` feature list, beside
  `freeze-watchdog`.

## Not done / open

- Real-model measurements on QEMU or the Zenbook. The model can't be fetched in this container, and QEMU TCG
  timing would not represent the silicon anyway. The `prefill-perf` line on the next image is that measurement.
- The real `.neom`'s per-tensor dtypes (`neomc inspect`). The kernel-level totals above assume the Q4_K_M mix
  stated.

## Fixes 1–3 as built (PR #105)

1. **The product image's kernel is built at opt-level 3.**
   - `xtask` now has one list, `PRODUCT_IMAGE_KERNEL_FEATURES`, and one builder, `build_product_image_kernel`. It
     builds through the `dev` profile with `CARGO_PROFILE_DEV_OPT_LEVEL=3`, so overflow checks and debug assertions
     stay on.
   - `iso --with-model` uses that builder.
   - A new `xtask image-kernel-build [--perf-probes]` builds exactly the same kernel without the model files. CI's
     `product-build` job runs it, so CI compiles what ships.
   - Scenario kernels are unchanged (opt 0), so no scenario's timing assumptions move.
   - Smoke test: four scenarios also pass on kernels built at opt 3. They are `sys_generate_kv_persist_equivalence`,
     `threaded_matmul_parity`, `inference_preemptible_liveness` and `desktop_session_boot`.
   - Kernel ELF, what the ISO carries (debug info included): 38,918,160 B at opt 0 → 35,046,000 B at opt 3.
     `.text`: 4,251,893 B → 1,427,780 B.
2. **The AVX2 decision is made once at boot, by every CPU (review items 8/10, CP-79).**
   - The first cut cached the BSP's check for every CPU. XCR0 is per-CPU, so an AP with different CPUID or XCR0
     could run AVX2 its own check rejects. Replaced by a census, `ops::qk_matmul::AVX2_CENSUS`:
     - each CPU reports its own check at the end of its `enable_sse` (after its XCR0 write, before it can poll
       for work);
     - any "no" vetoes immediately, and "no" is always safe;
     - `publish_avx2_decision(online_cpu_count())` in `_kernel_start`, after `bring_up_aps`, sets "yes" only if
       every online CPU reported and none said no; otherwise the decision stays "no" or undecided;
     - undecided keeps the old per-call check on each CPU, so a call made before the publish, or on a boot with
       a degraded AP, sees no new answer.
   - The serial line `ai: matmul avx2=<decision> (census: <reported>/<online> cpus reported)` shows the outcome.
   - Without the AVX2 kernels compiled, every CPU reports "no". The FXSAVE / no-AVX path is the local check, so
     the census adds no new "yes".
   - Evidence:
     - Host KATs `avx2_census_requires_every_cpu`, `avx2_census_veto_wins_under_concurrency`,
       `avx2_census_global_agrees_with_local_check`.
     - QEMU `threaded_matmul_parity` (see the PR for the census line).
3. **Prefill skips the lm_head for every prompt token but the last.**
   - The skip lives in `kernel/src/ai/decode_logic.rs` (review item 18): `DecodeCursor::step` runs the layer
     stack always, the output projection only when logits are wanted, then bumps the position;
     `prefill_last_logits` asks for logits on the last prompt token only. `InventoryDecodeState::step` and the
     kernel's persistent prefill run exactly this code. The kernel adapter (`TokenForward` for
     `InventoryForward`) is the embedding + layer loop and the norm + lm_head, with no branch on logits.
   - Host KAT `prefill_lm_head_skip_is_bit_identical` mounts `decode_logic.rs` and drives it through the real
     `forward_logic` layers at 0.5B shapes, against an independent per-token reference: the last position's
     logits are bit-identical, the lm_head runs once, the position ends at the prompt length, and the next decode
     step's logits (which read every cached K/V) are bit-identical too.
   - Mutations of `decode_logic.rs`, each red: skip the layers when no logits are wanted (the review's mutation);
     skip the position bump on those tokens; run the lm_head on every token; ask for logits on the first token.
   - `sys_generate_kv_persist_equivalence` stays green: fast ≡ slow, negative control red.
- **CI:** host-checks (and the close-gate) now run `cargo test -p ai-ops-tests --features
  phase9-real-quant,phase9-avx2-matmul`. That enforces these KATs and the 18 existing feature-gated real-quant ones;
  all 258 pass.

**`run.sh` after the fixes.** This rig, synthetic weights at real shapes, single thread:

| | per token | 28-token prefill |
|---|---|---|
| shipped (opt 0, per-call AVX2 decision, lm_head every token) | 7.43 s | **208 s** |
| fixed (opt 3, boot decision, lm_head last token only) | 0.188 s | **4.3 s** |

The shipped 208 s matches the Zenbook's observed 150–200 s. The fixed number is before any gain from threading (fix
4, unmeasured). The Zenbook measurement image is `xtask iso --with-model <neom> --perf-probes`; its fourth overlay
line gives `pre ms/t` and the `bar=`/`st=` that size fix 4.


## Round 2 (2026-10-25): measuring on silicon

Image A5 on the Zenbook (main `5880e075`, J.c. timing): "What's 2 + 2?" took 35 s before Arthur's answer
surface appeared, then 22 s more for the answer, 57 s in all. That's with `avx2=Yes (census: 16/16)` and the
product kernel at opt-level 3. This round puts the breakdown on screen, so one photo taken after an answer
says where the 57 s went, and fixes the one cause the code shows.

**What gates the surface.** Arthur already writes and commits `Thinking...` before it calls `sys_generate`
(`userland/neo-arthur/src/main.rs`, `run_desktop_session`: `write_text(b"Thinking...")`, then `commit_next()`,
then `respond_to_input`). So nothing in Arthur waits for prefill before showing the surface. If the surface
appears only at 35 s, either the WM did not present for that long, or something before the commit took that
long. The timeline line below tells which.

**What the overlay shows now** (`prefill-perf`, in image A's kernel list; lines 4–6 of the freeze-watchdog
block):

```
perf pre n=<prompt tokens> ms/t=<ms per prefill token> | dec n=<decoded> ms/t=<ms per decode token>
     | lm=<lm_head %> mv=<quantized matvec %> | par j=<jobs> bar=<barrier %> st=<steals> ap=<AP partitions>
time gen@t=<overlay s> think=<Arthur's last commit, relative> | pre=<ms> dec=<ms>
     | shown=<first WM present after the start> presents=<n> gap=<longest ms without one> | answer=<first present after the end>
cpu gen=c<N> <MHz over the generation> | parts own=<n> ap=<n> on <k> cpus | ap MHz <min>/<avg>/<max>
     | part us <min>..<max> (c<slowest>) | hwp=<on|off|none> req=<HWP_REQUEST> caps=<HWP_CAPABILITIES> ratio=<PERF_STATUS>
```

Line 3 now also shows each process's CPU (`/cN`), so it says whether the WM shares a CPU with the
generation.

**Reading one photo:**
- **The 35 s is presentation:** `think` near 0, `shown` far after the start or `-`, `gap` about the whole
  prefill. The WM did not present while the generation ran. Next, `/cN` on line 3 shows whether the WM and
  Arthur share a CPU.
- **The 35 s is prefill:** `pre` near 35,000 ms with `shown` small. Then the rest of the line says why:
  - **The clock is low:** `MHz` at a few hundred, or `hwp=off` with a low `ratio`. Build the `--hwp-performance`
    image and compare.
  - **The fork-join waits:** `bar` high, `st` large. With `matmul-steal-grace-time` (in image A), each steal
    now costs at most 50 µs instead of about 0.1 s (CP-80). If it's still high, the next step is
    work-stealing chunks (fix 4b).
  - **One slow core class:** `part us` with a wide `min..max` and a slow `c<N>`. Equal partitions wait on the
    slowest core class, so fix 4b again.
  - **Soft-float attention:** `mv` well below 100%. Fix 6.

**Fixes in this round:**
1. **`matmul-steal-grace-time`** (in image A). The steal grace is 50 µs of TSC time from the job's publish,
   not 2M `PAUSE` spins: about 0.1 s per job with one busy AP on Skylake-and-later cores (CP-80). This
   changes only when an unclaimed partition may be stolen; the F1 fail-stop is unchanged.
2. **`hwp-performance`** (not in image A; `xtask iso --with-model <neom> --hwp-performance`). On every CPU
   with HWP: set IA32_PM_ENABLE, then IA32_HWP_REQUEST = the full range with energy preference 0. It's the one
   change here that writes a power MSR, so it ships only as the B side of an A/B against the plain image's
   `MHz`, `hwp=` and `ratio` fields. It can't be undone before a reset.
