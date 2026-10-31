<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #68): lever 1b, counters and per-op MXCSR (#130)

Reviewed SHA: `0e66eb59dc81adabfbc4c5198bf8a2043b7a13ea`.
Branch: `codex/attn-lever1b-review-chad`.
Overall verdict: **FAIL** on the complete-entry-check and unrestricted-width requirements (items 4 and 5). Attention arithmetic and bounds pass. No attention byte divergence was found under CP-41's required MXCSR. Review only: no kernel changes or revert committed; report the confirmed failures to the PL for the dispatch's revert-first handling.

| Checklist | Verdict | Evidence at reviewed SHA |
|---|---|---|
| 1. Attention byte identity | PASS | `kernel/src/ai/ops/mod.rs:432` initializes the same negative-infinity max, then visits increasing `k_pos` and increasing `dim`, with the same zero-initialized multiply/add reduction and final scale multiplication as `:1196` / `:1229`. Each stored score at `:445` feeds max, then exp/sub at `:453`; denominator order is unchanged. The V reduction at `:459` keeps increasing `k_pos` separately for each output dimension and the same final division/write. Hoisting changes only addressing. A brace-balanced comparison confirmed the entire retained reference body at `:499` equals the parent's old last-position body at `e3e6e770`. |
| 2. KAT is real | PASS | `tools/ai-ops-tests/src/attention_lever1b_kat.rs:83` covers all eight requested positions, 14/2 heads, dimension 64, and all three scales; `:118` covers the other groupings/odd dimension; `:142` checks each short input against the old error. Local run: 3 passed. Independently reversing only the new dot's dimension addressing failed at seq=2, scale=0.5, output element 449: new `[e9 dd 7c 3d]`, reference `[ea dd 7c 3d]`, exit 101. Source restored byte-for-byte and all 3 tests passed again. |
| 3. Bounds and early errors | PASS | `kernel/src/ai/ops/mod.rs:406` / `:1154` / `:1185` reject zero dimensions, incompatible heads, dtype/rank/shape errors before indexing. `:407` checks output capacity; `:420` checks head-length multiplication; `:421`-`:423` and `:480` check all tensor byte lengths before row slices. Nonzero heads and successful full byte-size products bound every later multiplication and slice. `q_pos=seq_len-1`; all weight accesses range 0..seq_len, and the allocation at `:426` is exactly seq_len u32s, 9,216 heap bytes at 2304. For 14/2/2304/64, a head has 147,456 elements; maximum within-head index is 147,455, full Q index 2,064,383 and K/V index 294,911, each below the checked capacity. Short tensors return Input before row slices. Error output need not preserve the old partial writes, as disclosed in the PR. |
| 4. MXCSR entry coverage | FAIL | All listed `ops/mod.rs` operators have checks at `:239`, `:244`, `:249`, `:262`, `:284`, `:339`, `:405`, `:569`, `:838`, `:885`, `:932`, `:1022`; the shared norm implementations cover their wrappers. GQA forward and logits check at `kernel/src/ai/forward_logic.rs:707` / `:800`. However, `kernel/src/ai/ops/gdn.rs:336` loops through SiLU's soft helpers (`:346`), `:557` loops over soft add/mul/exp, `:457` loops over residual soft add, and `:708` loops over matvec soft add/mul, with no entry check anywhere in that file. Its public GDN core is also called from `kernel/src/ai/qwen_inventory.rs:603`. Moving the primitive assertion to cfg(test) at `soft_f32.rs:109` removes their previous non-test guard. Concrete reproduction and both results below. |
| 5. Counters and line width | FAIL (width); PASS (shares/purity/no locks) | `kernel/src/ai/forward_logic.rs:617` measures SwiGLU; `:760` measures cached attention including append/copies/conversion. Both are feature-gated. Counters reset at `kernel/src/ai/prefill_perf.rs:137`; `:228` / `:232` reach only relaxed atomic fetch_add at `:99`. `kernel/src/ai/prefill_perf_logic.rs:43` is pure and all shares divide by layers+lm_head at `:44`. Four formatter tests pass, but `tools/ai-ops-tests/src/prefill_perf_line_tests.rs:48` holds matvec/swiglu/attn/barrier cycles at 1, so it does not exercise widest percentages. Unrestricted u64 fields produce 260 bytes; watchdog's `kernel/src/watchdog.rs:330` silently keeps only the first 192. Concrete input/output below. This is an extreme/inconsistent-snapshot contract failure, not evidence of truncation in the measured product run. |
| 6. soft-f32-only | PASS | `kernel/src/ai/ops/soft_f32.rs:74` returns false for use_hardware with the feature enabled. Add/mul at `:119` / `:130` therefore use integer bodies and sub still delegates to add. The new check at `:92` is skipped too. Attention changes use the same helpers, never native arithmetic. All 3 attention KATs pass with soft-f32-only. Existing native f32 work in GDN is outside this soft_f32-helper switch and was not introduced here. |

## Confirmed missing guard (item 4)

Compiled a standalone host binary with `rustc --edition 2024 -C opt-level=0 -C debug-assertions=yes`, **without cfg(test)**, path-importing the pinned `soft_f32.rs` and `gdn.rs` as sibling modules. This matters: the ordinary host tests retain primitive checks and would conceal the missing production entry check.

Call `gdn::gdn_rmsnorm_gated(&[1.0], &[1.0], &[1.0], 1e-6, &mut [0.0])`: one-dimensional x, weight and gate, one token, no positional/GQA parameter. Load MXCSR with inline `ldmxcsr` before each call and restore the original afterward.

- MXCSR `0x1f80`: `Ok(())`, output bits `0x3f3b26a2`.
- MXCSR `0x5f80` (round toward +infinity): `Ok(())`, output bits `0x3f3b26a0`, with no assertion.
- Independently substituting the parent's `e3e6e770` soft_f32 module, while keeping the same GDN source, confirms the removed defense: `0x1f80` returns `0x3f3b26a2`; `0x3f80` panics with `soft_f32 hardware ops need MXCSR control 0x1F80`, process exit 101. The reviewed code accepts that same `0x3f80` case and returns `0x3f3b26a2`.

The root cause is the cfg(test)-only primitive assertion plus incomplete caller coverage. This is a debug-guard regression under an invalid FP environment; CP-41 normalization still supplies the correct environment during normal kernel operation. I did not find or claim a kernel entry that violates CP-41. Restoring coverage must include public standalone GDN operations, rather than relying solely on the Qwen2 GQA forward wrapper. The previous review also identified sampling's `generate_logic.rs:603` exp loop; it has no entry assertion either, although its add/mul calls are indirect through exp.

## Confirmed width counterexample (item 5)

Direct call to the production pure formatter, in the same non-test host probe:

```rust
let m = u64::MAX;
let l = PerfLine {
    tsc_hz: 1,
    prefill_tokens: m, prefill_cycles: m,
    decode_tokens: m, decode_cycles: m,
    layers_cycles: 1, lm_head_cycles: 0,
    matvec_cycles: m, swiglu_cycles: m, attn_cycles: m,
    par_jobs: m, par_barrier_cycles: m, steals: m, ap_parts: m,
};
```

Full String output, **260 bytes**:

```text
perf pre n=18446744073709551615 ms/t=1 | dec n=18446744073709551615 ms/t=1 | lm=0% mv=18446744073709551615% sw=18446744073709551615% at=18446744073709551615% | par j=18446744073709551615 bar=18446744073709551615% st=18446744073709551615 ap=18446744073709551615
```

Same output through the 192-byte watchdog Line writer:

```text
perf pre n=18446744073709551615 ms/t=1 | dec n=18446744073709551615 ms/t=1 | lm=0% mv=18446744073709551615% sw=18446744073709551615% at=18446744073709551615% | par j=18446744073709551615 bar=1
```

No memory overrun occurs; trailing diagnostics disappear. The formatter neither clamps shares nor states/enforces part<=forward. Such a restriction would exclude this counterexample, but the relaxed snapshot at `prefill_perf.rs:335`-`:349` is not transactional, and per-piece increments precede completion of the containing forward timing. Do not call the current width test an unrestricted widest-value proof. This finding concerns the formatter's claimed input domain; it does not establish that live counters reach this extreme combination. A bounded display policy plus a boundary test, or an explicit narrower contract with justified bounds, is needed to support the claim.

## Validation and scope

- `cargo test -p ai-ops-tests attention_lever1b`: 3 passed; repeated successfully after the temporary falsification was restored.
- `cargo test -p ai-ops-tests --features soft-f32-only attention_lever1b`: 3 passed.
- `cargo test -p ai-ops-tests prefill_perf_line_tests`: 4 passed; the separate counterexample above exposes the coverage limit.
- `cargo test -p ai-ops-tests --features phase9-real-quant,phase9-avx2-matmul soft_f32_hw_kat`: 6 passed, 1 ignored, including attention and whole-layer byte comparisons. Exhaustive exp was not run.
- Temporary reversed-dot test: expected failure, exact bytes above, source restored in finally and checked against Git.
- Standalone non-test debug-assertion probe: reproduced GDN guard removal and formatter length; parent-module comparison confirmed the old assertion.
- No kernel build, QEMU rerun, silicon timing or independent stack/heap census. PR #130's reported measurements were read, not reproduced. Source confirms no new attention allocation or stack array.

RAG was healthy and used for discovery; findings were verified against this pinned worktree. The PR body and prior relay #66 RETURN were read. Only this RETURN is committed. Preexisting untracked `graft/.cache` content was preserved with `git stash push --include-untracked -- graft`, named `chad preexisting graft cache before relay 68`. Final close checks after push: empty `git status --porcelain` and empty `git log '@{u}..'`.
