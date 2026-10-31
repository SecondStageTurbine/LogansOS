<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #97): A9-B repetition guard (#156)

**Reviewer:** Jarvis (Codex)
**Model / effort:** GPT-6-Luna / high
**Review branch:** `codex/a9-repetition-guard-review-jarvis`
**Reviewed commit:** `500606e9dded67b333b844c8a7d9907160fe88a4`
**Overall verdict: FAIL**

## Checklist

1. **FAIL — penalty rule.** The distinct-token scan and positive/negative formula are implemented as specified, and only the generated-token window is passed by the decode core. The prompt exclusion is an explicit policy choice for avoiding penalty on the chat prompt; Hugging Face Transformers applies its default penalty over `input_ids` (prompt plus output) and offers `prompt_ignore_length` to exclude the prompt. At `p=1.0`, finite logits and signed zero remain unchanged, but the function has no identity fast path: with `logits=[f32::from_bits(0x7f800001)]`, `generated=[0]`, `window=64`, and `p=1.0`, this host turns signaling NaN `0x7f800001` into quiet NaN `0x7fc00001`. The existing KAT covers signed zero and finite values but no NaN. (`kernel/src/ai/repetition_logic.rs:36-52,82-94`; `kernel/src/arch/x86_64/syscall.rs:3585-3592`; [Transformers repetition-penalty docs](https://huggingface.co/docs/transformers/main/internal/generation_utils#transformers.RepetitionPenaltyLogitsProcessor).)

2. **FAIL — contract mismatch for overlapping runs.** The detector correctly finds equal 16-token windows, but permits them to overlap. Concrete input: `[0,1,2,3,4,5,6,7]` repeated three times, with `ngram=16`, returns `Some(8)`. That is a repeated 8-token phrase, yet the overlapping 16-token windows compare equal and the guard truncates to the first eight tokens, inside the earlier 16-token window. This conflicts with the checklist guarantee that a legitimate phrase shorter than 16 tokens is never cut. Bounds are safe for short inputs and zero n-gram because the function returns before subtraction/slicing. (`kernel/src/ai/repetition_logic.rs:58-68,113-126`; decode truncation at `kernel/src/arch/x86_64/syscall.rs:3598-3607`.)

3. **PASS — wiring and returned length.** `generate_inventory_greedy` applies the penalty before the greedy pick and checks the loop stop after appending each non-EOS token. `sys_generate` and the KV-persistence equivalence check both reach this shared core. The stop truncates the vector, and the syscall copies/returns only `generated.len()`, so no stale tail is exposed. (`kernel/src/arch/x86_64/syscall.rs:3533-3618,3688-3689,3725-3789,3824-3954`.)

4. **PASS — oracle is independent.** `inventory_ref.py` computes the float64 guarded decode directly from the rule, without importing or invoking the Rust module; the committed oracle records `[11,0,3,4,10,3]`, and the guard scenario compares the shared core's output against that file. The Python `--check` rerun was blocked in this environment because NumPy is not installed; the existing gate run passed. (`tools/real-lm-oracle/inventory_ref.py:161-194,228-249`; `tools/real-lm-oracle/expected_guard_tokens.txt:1-7`; `kernel/src/test_harness.rs:37453-37501`.)

5. **PASS — kill-switch and baseline.** The `repetition_logic` module itself and both guard call sites are gated by `decode-repetition-guard`, so they compile out when the feature is off. The guard-off real-LM scenario still asserts the prior `[11,0,3,3,3,3,3,3]` vector, and the host KAT compares guarded-at-1.0 output with today's output when no loop fires. (`kernel/src/ai/mod.rs:68-69`; `kernel/src/arch/x86_64/syscall.rs:3584-3607`; `kernel/src/ai/repetition_logic.rs:190-201`; `tools/xtask/src/main.rs:14091-14114`.)

6. **PASS — prior-art survey is present.** PR #156's survey includes llama.cpp, Transformers and vLLM, with each license, head date/SHA, behavior comparison, and a statement that no code was copied. ([PR #156](https://github.com/SecondStageTurbine/LogansOS/pull/156), “Prior-art survey”.)

## Verification

- Reviewer rerun: `cargo test -p ai-ops-tests repetition` — 7 passed, 0 failed. The existing tests miss the signaling-NaN and short-phrase overlap inputs above.
- Reviewer probe against the pinned Rust module: signaling NaN `7f800001 -> 7fc00001`; an 8-token phrase repeated three times returns `Some(8)` for `ngram=16`.
- `python -I tools/real-lm-oracle/inventory_ref.py --check` could not run because this environment lacks `numpy`.
- Existing gate run `40446328993` was independently queried: completed successfully, all 22 jobs green, including all four QEMU shards. I did not rerun QEMU locally. [Gate run](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40446328993).
- RAG health check passed; graft was used for discovery, then the pinned source files were read directly.

### Reproducer

Save this as a temporary file such as `%TEMP%\a9-review-probe.rs`:

```rust
#[path = "E:/AI Automation/workers/jarvis/kernel/src/ai/repetition_logic.rs"]
mod repetition_logic;

fn main() {
    let bits = std::hint::black_box(0x7f800001u32);
    let penalty = std::hint::black_box(1.0f32);
    let mut logits = [f32::from_bits(bits)];
    repetition_logic::penalize(&mut logits, &[0], 64, penalty);
    let phrase: Vec<u32> = (0..24).map(|i| (i % 8) as u32).collect();
    println!("{:08x}", logits[0].to_bits());
    println!("{:?}", repetition_logic::loop_end(&phrase, 16));
}
```

From PowerShell, run `rustc +stable --edition=2024 $env:TEMP\a9-review-probe.rs -o $env:TEMP\a9-review-probe.exe; & $env:TEMP\a9-review-probe.exe`; output is `7fc00001` then `Some(8)`.

Disposition: review only; no feature or main-branch files were changed. These findings concern signaling-NaN bit preservation and the short-phrase cutoff contract; I did not observe or claim changed picks in a real model run.
