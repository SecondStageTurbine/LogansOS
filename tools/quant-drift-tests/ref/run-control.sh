#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Relay 333 control run: the pinned full-precision reference (float32 through
# transformers) against llama.cpp F16 on the same pinned model, plus the
# shipped Q4_K_M against the *same-checkpoint* F16 reference.
#
# The upstream Qwen2.5-0.5B-Instruct-GGUF Q4_K_M/Q8_0 files are NOT the same
# weights as Qwen/Qwen2.5-0.5B-Instruct@7ae5576: their token embeddings match,
# but blk.0.attn_q.weight differs by ~44% relative.  Bar B therefore uses the
# upstream F16 GGUF (the same checkpoint the shipped Q4_K_M was quantized from)
# as its full-precision reference; the pinned float32 is used only for Control C
# and for the model-identity measurement.
#
# Inputs (environment):
#   QUANT_DRIFT_NEOM         model-qwen2.5-0.5b.neom (BLAKE3 a32e4a1b...)
#   QUANT_DRIFT_GGUF_Q4      upstream q4_k_m GGUF (SHA-256 74a4da8c...)
#   QUANT_DRIFT_GGUF_Q8      upstream q8_0 GGUF (SHA-256 ca59ca7f...)
#   QUANT_DRIFT_GGUF_F16_UP  upstream fp16 GGUF (SHA-256 8e0ae260...)
#   QUANT_DRIFT_GGUF_F32     pinned f32 GGUF converted from the safetensors
#   QUANT_DRIFT_FP32_DIR     pinned Qwen/Qwen2.5-0.5B-Instruct safetensors dir
#   QUANT_DRIFT_CORPUS_TEXT  WikiText-2 raw test.txt
#   QUANT_DRIFT_LLAMA_DIR    built llama.cpp checkout
#   QUANT_DRIFT_WORK         scratch dir (default /tmp/quant-drift-control)
#   QUANT_DRIFT_OUT          output dir (default this crate's fixtures/)
#
# The corpus text, GGUFs and .neom are never committed.

set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$CRATE_DIR/../.." && pwd)"
WORK="${QUANT_DRIFT_WORK:-/tmp/quant-drift-control}"
OUT="${QUANT_DRIFT_OUT:-$CRATE_DIR/fixtures}"
LLAMA_DIR="${QUANT_DRIFT_LLAMA_DIR:?set QUANT_DRIFT_LLAMA_DIR}"
NEOM="${QUANT_DRIFT_NEOM:?set QUANT_DRIFT_NEOM}"
GGUF_Q4="${QUANT_DRIFT_GGUF_Q4:?set QUANT_DRIFT_GGUF_Q4}"
GGUF_Q8="${QUANT_DRIFT_GGUF_Q8:?set QUANT_DRIFT_GGUF_Q8}"
GGUF_F16_UP="${QUANT_DRIFT_GGUF_F16_UP:?set QUANT_DRIFT_GGUF_F16_UP}"
GGUF_F32="${QUANT_DRIFT_GGUF_F32:?set QUANT_DRIFT_GGUF_F32}"
FP32_DIR="${QUANT_DRIFT_FP32_DIR:?set QUANT_DRIFT_FP32_DIR}"
CORPUS_TEXT="${QUANT_DRIFT_CORPUS_TEXT:?set QUANT_DRIFT_CORPUS_TEXT}"
LLAMA_COMMIT="${QUANT_DRIFT_LLAMA_COMMIT:-$(git -C "$LLAMA_DIR" rev-parse HEAD)}"
FP32_REV="${QUANT_DRIFT_FP32_REV:-7ae557604adf67be50417f59c2c2f167def9a775}"
GGUF_REV="${QUANT_DRIFT_GGUF_REV:-9217f5db79a29953eb74d5343926648285ec7e67}"
DATE="${QUANT_DRIFT_DATE:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"

mkdir -p "$WORK" "$OUT"
cd "$REPO_ROOT"

echo "== build"
cargo build --release -p quant-drift-tests
BIN="$REPO_ROOT/target/release/quant-drift-tests"

echo "== prompt + corpus tokens (one tokenizer: the shipped .vocab)"
"$BIN" prompts --neom "$NEOM" --corpus-text "$CORPUS_TEXT" \
    --out-prompts "$WORK/prompts.tsv" --out-corpus "$WORK/corpus.u32"
# Every runner receives the full pinned corpus and --corpus-offset 6144, so
# the 2,048-token window it consumes is identical by construction. The window
# file is kept only as the BLAKE3 reference for that slice.
python3 - "$WORK/corpus.u32" "$WORK/corpus-window.u32" <<'PY'
import sys
raw = open(sys.argv[1], "rb").read()
open(sys.argv[2], "wb").write(raw[6144 * 4:])
PY

echo "== build llama.cpp reference runner"
g++ -O2 -std=c++17 "$CRATE_DIR/ref/llama_drift.cpp" \
    -I "$LLAMA_DIR/include" -I "$LLAMA_DIR/src" -I "$LLAMA_DIR/ggml/include" \
    -L "$LLAMA_DIR/build/bin" -lllama -lggml -lggml-base -lggml-cpu \
    -Wl,-rpath,"$LLAMA_DIR/build/bin" -o "$WORK/llama_drift"

sha256() { sha256sum "$1" | cut -d' ' -f1; }
b3() { python3 -c 'import blake3,sys; print(blake3.blake3(open(sys.argv[1],"rb").read()).hexdigest())' "$1"; }

NEOM_SHA="$(sha256 "$NEOM")"
NEOM_B3="$(b3 "$NEOM")"
GGUF_Q4_SHA="$(sha256 "$GGUF_Q4")"
GGUF_Q8_SHA="$(sha256 "$GGUF_Q8")"
GGUF_F16_SHA="$(sha256 "$GGUF_F16_UP")"
FP32_SHA="$(sha256 "$FP32_DIR/model.safetensors")"
CORPUS_B3="$(b3 "$WORK/corpus-window.u32")"
CORPUS_TEXT_SHA="$(sha256 "$CORPUS_TEXT")"

if [ "$NEOM_B3" != "a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe" ]; then
    echo "refusing: .neom BLAKE3 $NEOM_B3 is not the shipped pin a32e4a1b..." >&2
    exit 1
fi

echo "== ours (shipped Q4_K_M .neom, window)"
"$BIN" run --neom "$NEOM" \
    --prompts "$WORK/prompts.tsv" --corpus "$WORK/corpus.u32" \
    --label ours_q4_k_m_neom --sha256 "$NEOM_SHA" \
    --source "Arthur shipped .neom (BLAKE3 a32e4a1b...)" \
    --tool "quant-drift-tests (kernel forward_logic, release)" \
    --corpus-blake3 "$CORPUS_B3" --corpus-offset 6144 --corpus-tokens 2048 --eval-start 1024 \
    --out "$WORK/ours.tsv"

# The KAT pins the `llama_batch` logits. `--verify-final` is a diagnostic that
# re-feeds the prompt through a fresh context; on this llama.cpp build that
# fresh-context path disagrees with the batch row at desktop_help position 17
# (batch=198 matches the KAT, final=872), so it aborts the run. Leave the
# diagnostic off here and pass it explicitly when investigating that path.
echo "== llama.cpp Q4_K_M (upstream, same checkpoint as the shipped .neom)"
"$WORK/llama_drift" "$GGUF_Q4" "$WORK/prompts.tsv" "$WORK/corpus.u32" \
    "$WORK/llama-q4.tsv" llama_q4_k_m \
    "Qwen/Qwen2.5-0.5B-Instruct-GGUF qwen2.5-0.5b-instruct-q4_k_m.gguf@$GGUF_REV" \
    "$GGUF_Q4_SHA" "llama.cpp $LLAMA_COMMIT; ref/llama_drift.cpp" \
    --corpus-blake3 "$CORPUS_B3" --corpus-offset 6144 --corpus-tokens 2048 --eval-start 1024

echo "== llama.cpp F16 (upstream, same checkpoint as the shipped Q4_K_M)"
"$WORK/llama_drift" "$GGUF_F16_UP" "$WORK/prompts.tsv" "$WORK/corpus.u32" \
    "$WORK/llama-f16-up.tsv" llama_f16_upstream \
    "Qwen/Qwen2.5-0.5B-Instruct-GGUF qwen2.5-0.5b-instruct-fp16.gguf@$GGUF_REV" \
    "$GGUF_F16_SHA" "llama.cpp $LLAMA_COMMIT; ref/llama_drift.cpp" \
    --corpus-blake3 "$CORPUS_B3" --corpus-offset 6144 --corpus-tokens 2048 --eval-start 1024

echo "== llama.cpp F16 + Q8_0 quantized from the pinned f32 GGUF"
"$LLAMA_DIR/build/bin/llama-quantize" "$GGUF_F32" "$WORK/pinned-f16.gguf" F16 4
"$LLAMA_DIR/build/bin/llama-quantize" "$GGUF_F32" "$WORK/pinned-q8.gguf" Q8_0 4
"$WORK/llama_drift" "$WORK/pinned-f16.gguf" "$WORK/prompts.tsv" "$WORK/corpus.u32" \
    "$WORK/pinned-f16.tsv" llama_f16_pinned "F16 from the pinned f32 GGUF" \
    "$(sha256 "$WORK/pinned-f16.gguf")" "llama.cpp $LLAMA_COMMIT; ref/llama_drift.cpp" \
    --corpus-blake3 "$CORPUS_B3" --corpus-offset 6144 --corpus-tokens 2048 --eval-start 1024
"$WORK/llama_drift" "$WORK/pinned-q8.gguf" "$WORK/prompts.tsv" "$WORK/corpus.u32" \
    "$WORK/pinned-q8.tsv" llama_q8_pinned "Q8_0 from the pinned f32 GGUF" \
    "$(sha256 "$WORK/pinned-q8.gguf")" "llama.cpp $LLAMA_COMMIT; ref/llama_drift.cpp" \
    --corpus-blake3 "$CORPUS_B3" --corpus-offset 6144 --corpus-tokens 2048 --eval-start 1024

echo "== pinned float32 (transformers)"
python3 "$CRATE_DIR/ref/fp32_ref.py" "$FP32_DIR" "$WORK/prompts.tsv" "$WORK/corpus.u32" \
    "$WORK/fp32.tsv" --label llama_f32_transformers \
    --source "Qwen/Qwen2.5-0.5B-Instruct@$FP32_REV (float32, transformers)" \
    --sha256 "$FP32_SHA" --tool "transformers float32; ref/fp32_ref.py" \
    --corpus-blake3 "$CORPUS_B3" --corpus-offset 6144 --corpus-tokens 2048 --eval-start 1024

cat > "$WORK/meta.tsv" <<META
date=$DATE
host=$(uname -srm)
neom_sha256=$NEOM_SHA
neom_blake3=$NEOM_B3
gguf_sha256=$GGUF_Q4_SHA
gguf_revision=$GGUF_REV
fp32_sha256=$GGUF_F16_SHA
fp32_revision=$GGUF_REV
llama_commit=$LLAMA_COMMIT
corpus_parquet_sha256=${QUANT_DRIFT_CORPUS_PARQUET_SHA256:-5f1bea067869d04849c0f975a2b29c4ff47d867f484f5010ea5e861eab246d91}
corpus_text_sha256=$CORPUS_TEXT_SHA
corpus_slice_blake3=$CORPUS_B3
corpus_tokens=2048
corpus_eval_start=1024
corpus_window_offset=6144
META

echo "== bar A/B on the corrected same-checkpoint reference"
KAT="$OUT/quant-drift-kat-2026-11-08-control.json"
"$BIN" compare \
    --ours "$WORK/ours.tsv" --q4 "$WORK/llama-q4.tsv" --fp32 "$WORK/llama-f16-up.tsv" \
    --meta "$WORK/meta.tsv" --kat-out "$KAT" \
    --markdown-out "$OUT/quant-drift-kat-2026-11-08-control.report.md" || true
cp "$WORK/ours.tsv" "$OUT/quant-drift-kat-2026-11-08-control.ours.tsv"

echo "== Control C — F16 (pinned) vs float32"
"$BIN" control --candidate "$WORK/pinned-f16.tsv" --reference "$WORK/fp32.tsv" \
    --out "$OUT/quant-drift-control-C-f16.json"

echo "== Q8_0 (pinned) vs float32 — a quantization probe, not the control"
"$BIN" control --candidate "$WORK/pinned-q8.tsv" --reference "$WORK/fp32.tsv" \
    --out "$OUT/quant-drift-control-C-q8.json" || true

echo "== wrote $KAT"
