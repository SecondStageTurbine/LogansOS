#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Reproduce the NOTE's tables: the production kernel inference code at
# Qwen2.5-0.5B shapes, shipped (opt-level 0, AVX2 decided per dot product,
# lm_head on every prompt token) vs fixed (opt-level 3, AVX2 decided once,
# lm_head on the last prompt token only).
set -euo pipefail
cd "$(dirname "$0")"
F=phase9-avx2-matmul,phase9-quantized-matmul,phase9-real-quant
cargo build -q --features "$F" --bins
cargo build -q --release --features "$F" --bins
echo "== shipped kernels (opt 0, per-call decision)"; ./target/debug/inference-latency-bench
echo "== fixed kernels (opt 3, boot decision)";        REPS=5 ./target/release/inference-latency-bench fixed
echo "== shipped layer";                                POS=8 ./target/debug/layer
echo "== fixed layer";                                  POS=32 ./target/release/layer fixed
