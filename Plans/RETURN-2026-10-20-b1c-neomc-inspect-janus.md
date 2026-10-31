<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: B1c — real Arthur model memory-block inspection → Janus

**Date:** 2026-10-20
**Branch:** `codex/b1c-neomc-inspect-janus`
**neomc source commit:** `048182dd8ba76a3a87feb713275c1c1a820b9111`

## Pinned input and imported artifact

- GGUF: `qwen2.5-0.5b-instruct-q4_k_m.gguf`
- GGUF size: `491400032` bytes
- Upstream LFS SHA-256 from raw API JSON: `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db`
- Local GGUF SHA-256: `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db`
- SHA-256 verdict: **MATCH**
- Imported `.neom` size: `485489152` bytes

`b3sum` output:

```text
a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe  E:/AI Automation/b1-1/run1/model-qwen2.5-0.5b.neom
```

The full BLAKE3 matches the pinned Arthur model digest
`a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe`.

## Full inspect output

Command:

```powershell
cargo run -p neomc --release -- inspect "E:\AI Automation\b1-1\run1\model-qwen2.5-0.5b.neom"
```

Output, pasted verbatim:

```text
name dtype dims bytes pages order
output.weight 5 [144643072] 144643072 35314 16
token_embd.weight 5 [93592576] 93592576 22850 15
blk.0.attn_norm.weight 0 [896] 3584 1 0
blk.0.ffn_down.weight 5 [3575040] 3575040 873 10
blk.0.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.0.ffn_up.weight 5 [2996224] 2996224 732 10
blk.0.ffn_norm.weight 0 [896] 3584 1 0
blk.0.attn_k.bias 0 [128] 512 1 0
blk.0.attn_k.weight 5 [78848] 78848 20 5
blk.0.attn_output.weight 5 [551936] 551936 135 8
blk.0.attn_q.bias 0 [896] 3584 1 0
blk.0.attn_q.weight 5 [551936] 551936 135 8
blk.0.attn_v.bias 0 [128] 512 1 0
blk.0.attn_v.weight 5 [121856] 121856 30 5
blk.1.attn_norm.weight 0 [896] 3584 1 0
blk.1.ffn_down.weight 5 [3575040] 3575040 873 10
blk.1.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.1.ffn_up.weight 5 [2996224] 2996224 732 10
blk.1.ffn_norm.weight 0 [896] 3584 1 0
blk.1.attn_k.bias 0 [128] 512 1 0
blk.1.attn_k.weight 5 [78848] 78848 20 5
blk.1.attn_output.weight 5 [551936] 551936 135 8
blk.1.attn_q.bias 0 [896] 3584 1 0
blk.1.attn_q.weight 5 [551936] 551936 135 8
blk.1.attn_v.bias 0 [128] 512 1 0
blk.1.attn_v.weight 5 [121856] 121856 30 5
blk.10.attn_norm.weight 0 [896] 3584 1 0
blk.10.ffn_down.weight 5 [3575040] 3575040 873 10
blk.10.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.10.ffn_up.weight 5 [2996224] 2996224 732 10
blk.10.ffn_norm.weight 0 [896] 3584 1 0
blk.10.attn_k.bias 0 [128] 512 1 0
blk.10.attn_k.weight 5 [78848] 78848 20 5
blk.10.attn_output.weight 5 [551936] 551936 135 8
blk.10.attn_q.bias 0 [896] 3584 1 0
blk.10.attn_q.weight 5 [551936] 551936 135 8
blk.10.attn_v.bias 0 [128] 512 1 0
blk.10.attn_v.weight 5 [121856] 121856 30 5
blk.11.attn_norm.weight 0 [896] 3584 1 0
blk.11.ffn_down.weight 5 [2451456] 2451456 599 10
blk.11.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.11.ffn_up.weight 5 [2996224] 2996224 732 10
blk.11.ffn_norm.weight 0 [896] 3584 1 0
blk.11.attn_k.bias 0 [128] 512 1 0
blk.11.attn_k.weight 5 [78848] 78848 20 5
blk.11.attn_output.weight 5 [551936] 551936 135 8
blk.11.attn_q.bias 0 [896] 3584 1 0
blk.11.attn_q.weight 5 [551936] 551936 135 8
blk.11.attn_v.bias 0 [128] 512 1 0
blk.11.attn_v.weight 5 [78848] 78848 20 5
blk.12.attn_norm.weight 0 [896] 3584 1 0
blk.12.ffn_down.weight 5 [2451456] 2451456 599 10
blk.12.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.12.ffn_up.weight 5 [2996224] 2996224 732 10
blk.12.ffn_norm.weight 0 [896] 3584 1 0
blk.12.attn_k.bias 0 [128] 512 1 0
blk.12.attn_k.weight 5 [78848] 78848 20 5
blk.12.attn_output.weight 5 [551936] 551936 135 8
blk.12.attn_q.bias 0 [896] 3584 1 0
blk.12.attn_q.weight 5 [551936] 551936 135 8
blk.12.attn_v.bias 0 [128] 512 1 0
blk.12.attn_v.weight 5 [78848] 78848 20 5
blk.13.attn_norm.weight 0 [896] 3584 1 0
blk.13.ffn_down.weight 5 [3575040] 3575040 873 10
blk.13.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.13.ffn_up.weight 5 [2996224] 2996224 732 10
blk.13.ffn_norm.weight 0 [896] 3584 1 0
blk.13.attn_k.bias 0 [128] 512 1 0
blk.13.attn_k.weight 5 [78848] 78848 20 5
blk.13.attn_output.weight 5 [551936] 551936 135 8
blk.13.attn_q.bias 0 [896] 3584 1 0
blk.13.attn_q.weight 5 [551936] 551936 135 8
blk.13.attn_v.bias 0 [128] 512 1 0
blk.13.attn_v.weight 5 [121856] 121856 30 5
blk.14.attn_norm.weight 0 [896] 3584 1 0
blk.14.ffn_down.weight 5 [2451456] 2451456 599 10
blk.14.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.14.ffn_up.weight 5 [2996224] 2996224 732 10
blk.14.ffn_norm.weight 0 [896] 3584 1 0
blk.14.attn_k.bias 0 [128] 512 1 0
blk.14.attn_k.weight 5 [78848] 78848 20 5
blk.14.attn_output.weight 5 [551936] 551936 135 8
blk.14.attn_q.bias 0 [896] 3584 1 0
blk.14.attn_q.weight 5 [551936] 551936 135 8
blk.14.attn_v.bias 0 [128] 512 1 0
blk.14.attn_v.weight 5 [78848] 78848 20 5
blk.15.attn_norm.weight 0 [896] 3584 1 0
blk.15.ffn_down.weight 5 [2451456] 2451456 599 10
blk.15.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.15.ffn_up.weight 5 [2996224] 2996224 732 10
blk.15.ffn_norm.weight 0 [896] 3584 1 0
blk.15.attn_k.bias 0 [128] 512 1 0
blk.15.attn_k.weight 5 [78848] 78848 20 5
blk.15.attn_output.weight 5 [551936] 551936 135 8
blk.15.attn_q.bias 0 [896] 3584 1 0
blk.15.attn_q.weight 5 [551936] 551936 135 8
blk.15.attn_v.bias 0 [128] 512 1 0
blk.15.attn_v.weight 5 [78848] 78848 20 5
blk.16.attn_norm.weight 0 [896] 3584 1 0
blk.16.ffn_down.weight 5 [3575040] 3575040 873 10
blk.16.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.16.ffn_up.weight 5 [2996224] 2996224 732 10
blk.16.ffn_norm.weight 0 [896] 3584 1 0
blk.16.attn_k.bias 0 [128] 512 1 0
blk.16.attn_k.weight 5 [78848] 78848 20 5
blk.16.attn_output.weight 5 [551936] 551936 135 8
blk.16.attn_q.bias 0 [896] 3584 1 0
blk.16.attn_q.weight 5 [551936] 551936 135 8
blk.16.attn_v.bias 0 [128] 512 1 0
blk.16.attn_v.weight 5 [121856] 121856 30 5
blk.17.attn_norm.weight 0 [896] 3584 1 0
blk.17.ffn_down.weight 5 [2451456] 2451456 599 10
blk.17.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.17.ffn_up.weight 5 [2996224] 2996224 732 10
blk.17.ffn_norm.weight 0 [896] 3584 1 0
blk.17.attn_k.bias 0 [128] 512 1 0
blk.17.attn_k.weight 5 [78848] 78848 20 5
blk.17.attn_output.weight 5 [551936] 551936 135 8
blk.17.attn_q.bias 0 [896] 3584 1 0
blk.17.attn_q.weight 5 [551936] 551936 135 8
blk.17.attn_v.bias 0 [128] 512 1 0
blk.17.attn_v.weight 5 [78848] 78848 20 5
blk.18.attn_norm.weight 0 [896] 3584 1 0
blk.18.ffn_down.weight 5 [2451456] 2451456 599 10
blk.18.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.18.ffn_up.weight 5 [2996224] 2996224 732 10
blk.18.ffn_norm.weight 0 [896] 3584 1 0
blk.18.attn_k.bias 0 [128] 512 1 0
blk.18.attn_k.weight 5 [78848] 78848 20 5
blk.18.attn_output.weight 5 [551936] 551936 135 8
blk.18.attn_q.bias 0 [896] 3584 1 0
blk.18.attn_q.weight 5 [551936] 551936 135 8
blk.18.attn_v.bias 0 [128] 512 1 0
blk.18.attn_v.weight 5 [78848] 78848 20 5
blk.19.attn_norm.weight 0 [896] 3584 1 0
blk.19.ffn_down.weight 5 [3575040] 3575040 873 10
blk.19.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.19.ffn_up.weight 5 [2996224] 2996224 732 10
blk.19.ffn_norm.weight 0 [896] 3584 1 0
blk.19.attn_k.bias 0 [128] 512 1 0
blk.19.attn_k.weight 5 [78848] 78848 20 5
blk.19.attn_output.weight 5 [551936] 551936 135 8
blk.19.attn_q.bias 0 [896] 3584 1 0
blk.19.attn_q.weight 5 [551936] 551936 135 8
blk.19.attn_v.bias 0 [128] 512 1 0
blk.19.attn_v.weight 5 [121856] 121856 30 5
blk.2.attn_norm.weight 0 [896] 3584 1 0
blk.2.ffn_down.weight 5 [2451456] 2451456 599 10
blk.2.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.2.ffn_up.weight 5 [2996224] 2996224 732 10
blk.2.ffn_norm.weight 0 [896] 3584 1 0
blk.2.attn_k.bias 0 [128] 512 1 0
blk.2.attn_k.weight 5 [78848] 78848 20 5
blk.2.attn_output.weight 5 [551936] 551936 135 8
blk.2.attn_q.bias 0 [896] 3584 1 0
blk.2.attn_q.weight 5 [551936] 551936 135 8
blk.2.attn_v.bias 0 [128] 512 1 0
blk.2.attn_v.weight 5 [78848] 78848 20 5
blk.20.attn_norm.weight 0 [896] 3584 1 0
blk.20.ffn_down.weight 5 [2451456] 2451456 599 10
blk.20.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.20.ffn_up.weight 5 [2996224] 2996224 732 10
blk.20.ffn_norm.weight 0 [896] 3584 1 0
blk.20.attn_k.bias 0 [128] 512 1 0
blk.20.attn_k.weight 5 [78848] 78848 20 5
blk.20.attn_output.weight 5 [551936] 551936 135 8
blk.20.attn_q.bias 0 [896] 3584 1 0
blk.20.attn_q.weight 5 [551936] 551936 135 8
blk.20.attn_v.bias 0 [128] 512 1 0
blk.20.attn_v.weight 5 [78848] 78848 20 5
blk.21.attn_norm.weight 0 [896] 3584 1 0
blk.21.ffn_down.weight 5 [3575040] 3575040 873 10
blk.21.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.21.ffn_up.weight 5 [2996224] 2996224 732 10
blk.21.ffn_norm.weight 0 [896] 3584 1 0
blk.21.attn_k.bias 0 [128] 512 1 0
blk.21.attn_k.weight 5 [78848] 78848 20 5
blk.21.attn_output.weight 5 [551936] 551936 135 8
blk.21.attn_q.bias 0 [896] 3584 1 0
blk.21.attn_q.weight 5 [551936] 551936 135 8
blk.21.attn_v.bias 0 [128] 512 1 0
blk.21.attn_v.weight 5 [121856] 121856 30 5
blk.22.attn_norm.weight 0 [896] 3584 1 0
blk.22.ffn_down.weight 5 [2451456] 2451456 599 10
blk.22.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.22.ffn_up.weight 5 [2996224] 2996224 732 10
blk.22.ffn_norm.weight 0 [896] 3584 1 0
blk.22.attn_k.bias 0 [128] 512 1 0
blk.22.attn_k.weight 5 [78848] 78848 20 5
blk.22.attn_output.weight 5 [551936] 551936 135 8
blk.22.attn_q.bias 0 [896] 3584 1 0
blk.22.attn_q.weight 5 [551936] 551936 135 8
blk.22.attn_v.bias 0 [128] 512 1 0
blk.22.attn_v.weight 5 [78848] 78848 20 5
blk.23.attn_norm.weight 0 [896] 3584 1 0
blk.23.ffn_down.weight 5 [2451456] 2451456 599 10
blk.23.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.23.ffn_up.weight 5 [2996224] 2996224 732 10
blk.23.ffn_norm.weight 0 [896] 3584 1 0
blk.23.attn_k.bias 0 [128] 512 1 0
blk.23.attn_k.weight 5 [78848] 78848 20 5
blk.23.attn_output.weight 5 [551936] 551936 135 8
blk.23.attn_q.bias 0 [896] 3584 1 0
blk.23.attn_q.weight 5 [551936] 551936 135 8
blk.23.attn_v.bias 0 [128] 512 1 0
blk.23.attn_v.weight 5 [78848] 78848 20 5
blk.3.attn_norm.weight 0 [896] 3584 1 0
blk.3.ffn_down.weight 5 [3575040] 3575040 873 10
blk.3.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.3.ffn_up.weight 5 [2996224] 2996224 732 10
blk.3.ffn_norm.weight 0 [896] 3584 1 0
blk.3.attn_k.bias 0 [128] 512 1 0
blk.3.attn_k.weight 5 [78848] 78848 20 5
blk.3.attn_output.weight 5 [551936] 551936 135 8
blk.3.attn_q.bias 0 [896] 3584 1 0
blk.3.attn_q.weight 5 [551936] 551936 135 8
blk.3.attn_v.bias 0 [128] 512 1 0
blk.3.attn_v.weight 5 [121856] 121856 30 5
blk.4.attn_norm.weight 0 [896] 3584 1 0
blk.4.ffn_down.weight 5 [2451456] 2451456 599 10
blk.4.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.4.ffn_up.weight 5 [2996224] 2996224 732 10
blk.4.ffn_norm.weight 0 [896] 3584 1 0
blk.4.attn_k.bias 0 [128] 512 1 0
blk.4.attn_k.weight 5 [78848] 78848 20 5
blk.4.attn_output.weight 5 [551936] 551936 135 8
blk.4.attn_q.bias 0 [896] 3584 1 0
blk.4.attn_q.weight 5 [551936] 551936 135 8
blk.4.attn_v.bias 0 [128] 512 1 0
blk.4.attn_v.weight 5 [78848] 78848 20 5
blk.5.attn_norm.weight 0 [896] 3584 1 0
blk.5.ffn_down.weight 5 [2451456] 2451456 599 10
blk.5.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.5.ffn_up.weight 5 [2996224] 2996224 732 10
blk.5.ffn_norm.weight 0 [896] 3584 1 0
blk.5.attn_k.bias 0 [128] 512 1 0
blk.5.attn_k.weight 5 [78848] 78848 20 5
blk.5.attn_output.weight 5 [551936] 551936 135 8
blk.5.attn_q.bias 0 [896] 3584 1 0
blk.5.attn_q.weight 5 [551936] 551936 135 8
blk.5.attn_v.bias 0 [128] 512 1 0
blk.5.attn_v.weight 5 [78848] 78848 20 5
blk.6.attn_norm.weight 0 [896] 3584 1 0
blk.6.ffn_down.weight 5 [3575040] 3575040 873 10
blk.6.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.6.ffn_up.weight 5 [2996224] 2996224 732 10
blk.6.ffn_norm.weight 0 [896] 3584 1 0
blk.6.attn_k.bias 0 [128] 512 1 0
blk.6.attn_k.weight 5 [78848] 78848 20 5
blk.6.attn_output.weight 5 [551936] 551936 135 8
blk.6.attn_q.bias 0 [896] 3584 1 0
blk.6.attn_q.weight 5 [551936] 551936 135 8
blk.6.attn_v.bias 0 [128] 512 1 0
blk.6.attn_v.weight 5 [121856] 121856 30 5
blk.7.attn_norm.weight 0 [896] 3584 1 0
blk.7.ffn_down.weight 5 [3575040] 3575040 873 10
blk.7.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.7.ffn_up.weight 5 [2996224] 2996224 732 10
blk.7.ffn_norm.weight 0 [896] 3584 1 0
blk.7.attn_k.bias 0 [128] 512 1 0
blk.7.attn_k.weight 5 [78848] 78848 20 5
blk.7.attn_output.weight 5 [551936] 551936 135 8
blk.7.attn_q.bias 0 [896] 3584 1 0
blk.7.attn_q.weight 5 [551936] 551936 135 8
blk.7.attn_v.bias 0 [128] 512 1 0
blk.7.attn_v.weight 5 [121856] 121856 30 5
blk.8.attn_norm.weight 0 [896] 3584 1 0
blk.8.ffn_down.weight 5 [3575040] 3575040 873 10
blk.8.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.8.ffn_up.weight 5 [2996224] 2996224 732 10
blk.8.ffn_norm.weight 0 [896] 3584 1 0
blk.8.attn_k.bias 0 [128] 512 1 0
blk.8.attn_k.weight 5 [78848] 78848 20 5
blk.8.attn_output.weight 5 [551936] 551936 135 8
blk.8.attn_q.bias 0 [896] 3584 1 0
blk.8.attn_q.weight 5 [551936] 551936 135 8
blk.8.attn_v.bias 0 [128] 512 1 0
blk.8.attn_v.weight 5 [121856] 121856 30 5
blk.9.attn_norm.weight 0 [896] 3584 1 0
blk.9.ffn_down.weight 5 [3575040] 3575040 873 10
blk.9.ffn_gate.weight 5 [2996224] 2996224 732 10
blk.9.ffn_up.weight 5 [2996224] 2996224 732 10
blk.9.ffn_norm.weight 0 [896] 3584 1 0
blk.9.attn_k.bias 0 [128] 512 1 0
blk.9.attn_k.weight 5 [78848] 78848 20 5
blk.9.attn_output.weight 5 [551936] 551936 135 8
blk.9.attn_q.bias 0 [896] 3584 1 0
blk.9.attn_q.weight 5 [551936] 551936 135 8
blk.9.attn_v.bias 0 [128] 512 1 0
blk.9.attn_v.weight 5 [121856] 121856 30 5
output_norm.weight 0 [896] 3584 1 0
tensors=291 weight_bytes=485452288 buddy_bytes=761761792 largest=output.weight largest_bytes=144643072 min_max_order=16
```

## Verdict

**PASS — PINNED MODEL CONFIRMED.** The real imported Arthur model requires
`761761792` buddy bytes; its largest tensor is `output.weight` at
`144643072` bytes, and the minimum allocator maximum order is `16`.
