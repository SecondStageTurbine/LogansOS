<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: where Arthur's forward time goes outside the matvecs (speed lane, before building)

**Seat:** S4. **Status:** measurement and proposal, no kernel code. The PM asked for an overlay split
(soft_f32 time, small serial matvecs, lm_head) and a written NOTE before any fix is built.

## Answer first

- **Soft-float element-wise work dominates, not the matvecs.** On A6-hwp the quantized matvecs are
  `mv=17%` of forward cycles. That includes fork-join. The other ~83% runs serially on c0, and most of it
  is `soft_f32` arithmetic: the SwiGLU element-wise step, and the cached attention core (Q·K dot products,
  softmax, weighted V sum).
- **The big lever is hardware f32 for `add`/`sub`/`mul`, with the bytes unchanged.** A byte-oracle shows
  that soft `add`, `sub` and `mul` match SSE bit-for-bit, NaN canonicalized, on 20M operand pairs each.
  The one exception, found later by the edge sweep (lever 1 section), is `+0 + -0`. A
  prototype `exp` that uses the same algorithm with hardware add/sub/mul is **18.7× faster (11.7 ns vs
  219 ns)**. An exhaustive check found **0 mismatches against soft `exp` over all 2.24G inputs that reach the
  polynomial**.
- **`div` and `sqrt` must stay soft.** Soft `div` isn't correctly rounded and soft `sqrt` is Newton's
  method, so hardware `divss`/`sqrtss` would change bytes. Neither is the expensive op.
- **Fork-join tuning (fix 4b) stays a later, smaller lever.** It can only shrink the 17%.

## The split (host, one thread, opt 3, the production `forward_logic.rs` code)

`tools/inference-latency-bench`, `split` bin, features `phase9-avx2-matmul,phase9-quantized-matmul,phase9-real-quant`,
`fixed` (the boot-time AVX2 decision). Qwen2.5-0.5B shapes (h 896, kv 128, ff 4864, vocab 151936),
Q5_0/Q8_0 attention, Q4_K FFN. Three runs each, so the figures are ranges.

| piece (per layer) | POS=1 | POS=25 | what it is |
|---|---|---|---|
| whole layer, mean over positions 0..POS | 5.9–6.0 ms | 7.2 ms | |
| `swiglu_ffn` | 4.9–5.2 | 4.8–4.9 | norm + gate/up/down matvecs + SwiGLU + residual |
| ↳ `ops::swiglu` alone | **1.18–1.26** | **1.18–1.21** | 4864 × soft (`exp`, `add`, `div`, `mul`) |
| `attention_seam_with_norms` | 0.41–0.45 | 0.53–0.54 | q/k/v matvecs (~0.37) + norm, bias, RoPE in soft f32 |
| `gqa_attention_cached_decode`, last position | 0.07 | **2.09–2.12** | soft dot products + softmax + V sum; grows with position |
| `rms_norm_vec` / `add_vec` | 0.04 / 0.01 | 0.04 / 0.01 | |
| lm_head + final norm (`layer` bin) | | 32.0 per token | Q8_0, 151936 rows |

Split per layer at a decode position near 25–30:
- **Matvecs:** about 4.2 ms single-thread (FFN ≈ 3.6, q/k/v ≈ 0.37, o ≈ 0.2).
- **Serial soft f32:** about 3.5–4.0 ms (SwiGLU 1.2, attention core 2.1 and rising, seam about 0.16).

**Consistency check against the Zenbook.**
- In the kernel, the matvecs are split across 4 CPUs. The soft-f32 work is not.
- With 4.2 ms of matvec work over 4 CPUs plus some overhead, against about 3.8 ms serial, the matvec share
  comes out near 1.1 / (1.1 + 3.8) ≈ 22%.
- A6-hwp measured `mv=17%`. The two agree, so the serial soft-f32 path is what A6-hwp's decode
  (304 ms/token) is waiting on. Across 24 layers that's roughly 90 ms of host-speed soft f32 per decode
  token, before scaling to c0.
- The `part us 92..216 (c0)` spread, c0 slowest, fits as well. c0 is the CPU that also does all the serial
  work.

## Where the time goes inside the soft ops

Per-op cost on the host (`soft_f32_oracle` bin):

| op | soft | hardware-op prototype |
|---|---|---|
| `exp` | 211–219 ns | **11.7 ns** (same algorithm) |
| `div` | 18.3 ns | must stay soft (below) |
| `add` | 9.8 ns | 1 instruction |
| `mul` | 9.0 ns | 1 instruction |
| SwiGLU element `x · (x / (1 + e^-x))` | 257–278 ns | about 31 ns projected (hw exp + soft div) |

**`exp` is about 80% of a SwiGLU element.** Its algorithm only uses `mul`, `add`, `sub`, exact integer
steps (`round_f32_to_int`, `make_power_of_two`) and `int_to_f32` on |k| < 128, which is exact. So it can
be rebuilt from hardware ops with the bytes unchanged, provided `add`/`sub`/`mul` are bit-identical.

## The byte-oracle (determinism argument)

`soft_f32` is the bit-exact CPU reference (ai-subsystem spec: "Deterministic — asserted exactly"). A
hardware path is acceptable only if it yields the **same bits**, not just within tolerance.

| op | samples | result |
|---|---|---|
| `add`, `sub`, `mul` | 20M pairs each: half uniform over all 2^32 bit patterns (NaN, inf, subnormal included), half in the activation range | **0 mismatches**, after canonicalizing hardware NaN to soft's `0x7fc0_0000`. The random sample never hits zeros; the edge sweep in the lever 1 section does, and finds one class |
| `div` | 20M pairs | 427,540 mismatches on subnormal or zero results, plus 12,938 normal-range mismatches (max 40 ulp). With both operands in [2^-27, 2^33): 14,905 / 20M. **Not correctly rounded: keep soft** |
| `sqrt` | every 97th normal input | 5.49M within 1 ulp (Newton, 8 iterations). **Keep soft** |
| `exp` (hw-op prototype vs soft) | exhaustive: every input with \|x\| ≤ 88.72, both signs: 2,237,850,676 | **0 mismatches** (81 s on 4 threads) |

**Exhaustive `exp` result: 0 mismatches over 2,237,850,676 inputs.** Every other input (NaN, ±inf,
±0, \|x\| > 88.72) takes an early return that has no arithmetic, and the prototype shares that code. So the
hardware-op `exp` equals soft `exp` on all 2^32 bit patterns, on this host's SSE.

**What has to hold in the kernel for the add/sub/mul identity to transfer:**
1. **MXCSR = `0x1F80`** (round-to-nearest-even, no FTZ/DAZ, exceptions masked). The kernel already loads
   `KERNEL_MXCSR = 0x1F80` at syscall, timer and switch boundaries (CP-41), and inference runs below a
   syscall.
2. **No FMA contraction.** Rust never fuses `a * b + c` without an explicit `mul_add`, and the target has
   no `fma` feature. The rewrite must keep each soft call's order of operations exactly. Reordering a sum,
   or vectorizing a reduction, changes the bits.
3. **SSE state saved across preemption.** Already true: FXSAVE/XSAVE at the scheduler, syscall and timer
   boundaries (`fpu.rs`). The AVX2 matvecs already run hardware FP on the inference stack.
4. **NaN canonicalization.** Every hardware result that can be NaN is mapped to `0x7fc0_0000`, as soft does.
   That's one compare per op. It can be hoisted to the function result where every intermediate NaN
   propagates, which the oracle has to cover too.

## Levers, in order

| | lever | expected effect | byte-identical? | cost |
|---|---|---|---|---|
| **1** | Hardware `add`/`sub`/`mul` inside `soft_f32` (keeping the `u32` API), so `exp`, `tanh`, the dot products and RoPE all speed up; `div`/`sqrt` stay soft | SwiGLU 1.2 → about 0.15 ms per layer (measured per op). Attention core: most of its 2.1 ms is soft mul/add dot products, but the split hasn't been measured yet (step 2 below) | Yes, if the oracle and the four conditions hold | One file plus the oracle bin; kill-switch `soft-f32-only` restores the integer path |
| **2** | Parallelize the per-element soft work (SwiGLU elements, attention heads) across the fork-join pool | Divides the serial part by the CPU count | Yes: per-element results don't depend on the split | Touches the fork-join path; worth doing after lever 1, because lever 1 shrinks what's left to split |
| **3** | Fix 4b (dynamic chunking of matvec partitions) | ≤ 7% (earlier measurement) | Yes | Small |

**Recommendation: build lever 1 first.**
- Gate it with:
  - the exhaustive `exp` oracle;
  - a host KAT showing `ops::swiglu`, `softmax`, `rms_norm` and `gqa_attention_cached_decode` produce
    identical bytes with lever 1 on and off;
  - `sys_generate_kv_persist_equivalence` plus an Arthur transcript byte-compare on QEMU.
- Then re-split. If the attention core is still large, lever 2 is next.

## Lever 1: built (hardware add/sub/mul in `soft_f32`)

**The change.**
- `soft_f32::add` and `mul` use SSE; `sub` stays `add(a, b ^ SIGN)`.
- The integer bodies stay as `add_int` and `mul_int`, the reference.
- Two adjustments make the bytes identical:
  - **NaN:** every NaN result becomes `QUIET_NAN`. x86 gives a negative default NaN for `0 × inf`, and
    propagates an operand's payload otherwise.
  - **Signed zero:** when both addends are zero, the result is `b`. The edge sweep found that `add_int`
    returns `-0` for `+0 + -0` (and so `sub_int(+0, +0)` is `-0`), where IEEE gives `+0`. That was the
    only mismatch in 6.86M structured pairs (the scratch sweep; the KAT's set is 6.85M). The 20M random sample couldn't see it, because it almost
    never draws a zero.
- Everything built on these ops speeds up with them: `exp`, `tanh`, the dot products, RoPE and the norms.
  `div` and `sqrt` keep their integer bodies.
- **Kill-switch:** `soft-f32-only` compiles the integer path only, bit-for-bit the previous layer.
- **Debug assertion:** under `debug_assertions` there's a check of the MXCSR control bits (`& 0xFFC0 ==
  0x1F80`). Bits 0–5 are sticky flags that arithmetic sets, so they're excluded. The first draft compared
  all 16 bits and tripped on the DE flag (`0x1F82`).

**Host KATs (`tools/ai-ops-tests/src/soft_f32_hw_kat.rs`).** Each one runs the same input through both
paths in one binary. `with_integer_ops`, test-only, forces the integer path on the calling thread.

| KAT | coverage | result |
|---|---|---|
| edge pairs | add, sub, mul over 2,618² (6.85M) pairs: zeros, subnormals, boundaries, max, inf, NaN, both signs, five mantissas at every exponent | pass |
| uniform + activation pairs | 2M pairs per op | pass |
| `exp`, sampled | every 1021st input up to the polynomial limit, both signs, plus the edge values | pass |
| `exp`, exhaustive (`#[ignore]`, run in release) | all 2,237,850,676 inputs that reach the polynomial | **pass, 102 s** |
| element-wise and norm ops | `swiglu`, `silu`, `gelu`, `rms_norm`, `layer_norm`, `softmax` at width 4864, at scales 4 and 120 (the latter reaches exp under- and overflow) | pass |
| attention core | `gqa_attention_cached_decode`, 0.5B shapes, 12 positions | pass |
| full layer (real-quant) | `forward_layer_with_gqa_attention_with_norms`, 0.5B shapes, A6 quant mix, 6 positions; every intermediate compared | pass |

**Falsification of the KATs themselves.** With the signed-zero adjustment removed, the edge KAT fails on
`add(0x00000000, 0x80000000): hw 0x00000000 int 0x80000000`. So `with_integer_ops` does switch paths, and
the edge KAT sees a one-bit difference.

**Host split, after vs before (`split` bin, POS=25; before = `--features soft-f32-only`):**

| piece per layer | integer | hardware | |
|---|---|---|---|
| `ops::swiglu` | 1.18–1.19 ms | 0.38–0.39 ms | 3.1× (`div` is now most of it) |
| attention core, last position | 2.17–2.23 ms | 0.34–0.37 ms | 6.2× |
| whole layer, mean over 0..25 | 7.3–7.6 ms | 4.9–5.6 ms | |

The serial soft-f32 work per layer goes from about 3.5 ms to about 0.9 ms. What's left is mostly matvecs,
which the kernel splits across CPUs.

**Kernel codegen.** The kernel target (`"features": ""`, x86-64 baseline SSE2) compiles the hardware
path to `addss`/`mulss`. The scenario kernel holds no `__addsf3`/`__mulsf3` soft-float libcalls, and its
debug build carries the `stmxcsr` assertion.

**QEMU (local, `NEO_SMP=4`), all PASS on the first attempt:**

| scenario | hardware (default) | `soft-f32-only` |
|---|---|---|
| `sys_generate_kv_persist_equivalence` | PASS. Forward cycles 169.7G | PASS. Forward cycles 200.9G |
| `real_lm_generate_boot` (tokens checked against the Python oracle) | PASS | PASS |
| `ai_generate_decode` | PASS | PASS |
| `infer_transformer_forward` | PASS | PASS |
| `infer_mlp_forward`, `inference_stack_kill`, `boot_model_load_ticks` | PASS | |

- QEMU TCG emulates SSE in software, so its 15.5% cycle drop understates the silicon effect. The A8
  overlay's `perf dec ms/t` and `mv=` are the real measure.
- **Prediction for A8:** decode ms/token falls, and `mv=` rises well above 17%, because the serial share
  shrinks.

## Falsification

- **Lever 1 is wrong** if any oracle row shows a mismatch, or if the KAT shows different bytes. Then the
  kernel stays on soft ops for that function.
- **Lever 1 doesn't pay** if the A7 overlay shows `mv=` rising to about 60% or more (the soft path is gone)
  while decode ms/token falls by less than about 40%. In that case the remaining time is something this
  split doesn't see: c0 preemption, or the core-type question below.

## Still open

- **c0's slow partitions.** Two hypotheses:
  - c0 is preempted by user processes;
  - c0 is a different core type.
- **Discriminators for the overlay:**
  - per-CPU median and max partition time;
  - context switches during c0's partitions;
  - CPUID leaf 0x1A core type per CPU.
- These host figures are on the container's CPU. The Zenbook's absolute numbers differ; the ratios are
  what carry over.

## Reproduce

```
cd tools/inference-latency-bench
F=phase9-avx2-matmul,phase9-quantized-matmul,phase9-real-quant
cargo build --release --features $F --bins
POS=1 ./target/release/split fixed; POS=25 ./target/release/split fixed
./target/release/soft_f32_oracle                 # byte-oracle + per-op cost
./target/release/soft_f32_oracle exhaustive-exp  # all |x| <= 88.72, both signs
```
