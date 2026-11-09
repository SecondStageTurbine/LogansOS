#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Produce the quantization-drift KAT: our Q4_K_M .neom forward pass, llama.cpp
# on the same Q4_K_M GGUF, and llama.cpp on the F32 GGUF converted from the
# pinned Qwen2.5-0.5B-Instruct safetensors. All three consume the SAME token
# IDs, produced by the shipped `.vocab` sidecar (Arthur's tokenizer).
#
# Inputs (environment):
#   QUANT_DRIFT_NEOM         model-qwen2.5-0.5b.neom (BLAKE3 a32e4a1b...)
#   QUANT_DRIFT_GGUF_Q4      the upstream q4_k_m GGUF (SHA-256 74a4da8c...)
#   QUANT_DRIFT_FP32_DIR     the pinned Qwen2.5-0.5B-Instruct safetensors directory
#   QUANT_DRIFT_CORPUS_TEXT  WikiText-2 raw test.txt (pinned revision below)
#   QUANT_DRIFT_LLAMA_DIR    llama.cpp checkout, built (`build/bin/libllama.so`)
#   QUANT_DRIFT_WORK         scratch dir (default /tmp/quant-drift)
#
# Outputs: `$QUANT_DRIFT_OUT` (default this crate's fixtures/) gets the KAT
# JSON, the exact ours-run TSV the regression test reads, and the markdown.
# The corpus text, GGUFs and `.neom` are never committed.

set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$CRATE_DIR/../.." && pwd)"
WORK="${QUANT_DRIFT_WORK:-/tmp/quant-drift}"
OUT="${QUANT_DRIFT_OUT:-$CRATE_DIR/fixtures}"
LLAMA_DIR="${QUANT_DRIFT_LLAMA_DIR:?set QUANT_DRIFT_LLAMA_DIR to a built llama.cpp checkout}"
NEOM="${QUANT_DRIFT_NEOM:?set QUANT_DRIFT_NEOM}"
GGUF_Q4="${QUANT_DRIFT_GGUF_Q4:?set QUANT_DRIFT_GGUF_Q4}"
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
"$BIN" prompts \
    --neom "$NEOM" \
    --corpus-text "$CORPUS_TEXT" \
    --out-prompts "$WORK/prompts.tsv" \
    --out-corpus "$WORK/corpus.u32"

echo "== build llama.cpp reference runner"
g++ -O2 -std=c++17 "$CRATE_DIR/ref/llama_drift.cpp" \
    -I "$LLAMA_DIR/include" -I "$LLAMA_DIR/src" -I "$LLAMA_DIR/ggml/include" \
    -L "$LLAMA_DIR/build/bin" -lllama -lggml -lggml-base -lggml-cpu \
    -Wl,-rpath,"$LLAMA_DIR/build/bin" -o "$WORK/llama_drift"

sha256() { sha256sum "$1" | cut -d' ' -f1; }
b3() { python3 -c 'import blake3,sys; print(blake3.blake3(open(sys.argv[1],"rb").read()).hexdigest())' "$1"; }

NEOM_SHA="$(sha256 "$NEOM")"
NEOM_B3="$(b3 "$NEOM")"
GGUF_SHA="$(sha256 "$GGUF_Q4")"
FP32_SHA="$(sha256 "$FP32_DIR/model.safetensors")"
CORPUS_B3="$(b3 "$WORK/corpus.u32")"
CORPUS_TEXT_SHA="$(sha256 "$CORPUS_TEXT")"

# The `.neom` must be the shipped one; a re-import that differs is a different
# model and the KAT would pin the wrong forward pass.
if [ "$NEOM_B3" != "a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe" ]; then
    echo "refusing: .neom BLAKE3 $NEOM_B3 is not the shipped pin a32e4a1b..." >&2
    exit 1
fi

echo "== ours (host forward pass)"
"$BIN" run --neom "$NEOM" \
    --prompts "$WORK/prompts.tsv" --corpus "$WORK/corpus.u32" \
    --label ours_q4_k_m_neom --sha256 "$NEOM_SHA" \
    --source "Arthur shipped .neom (BLAKE3 a32e4a1b...) imported from Qwen2.5-0.5B-Instruct Q4_K_M" \
    --tool "quant-drift-tests (kernel forward_logic, release)" \
    --corpus-blake3 "$CORPUS_B3" --out "$WORK/ours.tsv"

echo "== llama.cpp Q4_K_M"
"$WORK/llama_drift" "$GGUF_Q4" "$WORK/prompts.tsv" "$WORK/corpus.u32" \
    "$WORK/llama-q4.tsv" llama_q4_k_m \
    "Qwen/Qwen2.5-0.5B-Instruct-GGUF qwen2.5-0.5b-instruct-q4_k_m.gguf@$GGUF_REV" \
    "$GGUF_SHA" "llama.cpp $LLAMA_COMMIT; ref/llama_drift.cpp" \
    --corpus-blake3 "$CORPUS_B3"

echo "== full precision (transformers float32)"
python3 "$CRATE_DIR/ref/fp32_ref.py" "$FP32_DIR" "$WORK/prompts.tsv" "$WORK/corpus.u32" \
    "$WORK/llama-f32.tsv" --label llama_f32_transformers \
    --source "Qwen/Qwen2.5-0.5B-Instruct@$FP32_REV (float32, transformers)" \
    --sha256 "$FP32_SHA" \
    --tool "transformers float32; ref/fp32_ref.py" \
    --corpus-blake3 "$CORPUS_B3"

cat > "$WORK/meta.tsv" <<EOF
date=$DATE
host=$(uname -srm)
neom_sha256=$NEOM_SHA
neom_blake3=$NEOM_B3
gguf_sha256=$GGUF_SHA
gguf_revision=$GGUF_REV
fp32_sha256=$FP32_SHA
fp32_revision=$FP32_REV
llama_commit=$LLAMA_COMMIT
corpus_parquet_sha256=${QUANT_DRIFT_CORPUS_PARQUET_SHA256:-5f1bea067869d04849c0f975a2b29c4ff47d867f484f5010ea5e861eab246d91}
corpus_text_sha256=$CORPUS_TEXT_SHA
EOF

echo "== compare against the fixed bars"
KAT="$OUT/quant-drift-kat-2026-11-08.json"
"$BIN" compare \
    --ours "$WORK/ours.tsv" --q4 "$WORK/llama-q4.tsv" --fp32 "$WORK/llama-f32.tsv" \
    --meta "$WORK/meta.tsv" \
    --kat-out "$KAT" \
    --markdown-out "$OUT/quant-drift-kat-2026-11-08.report.md"
cp "$WORK/ours.tsv" "$OUT/quant-drift-kat-2026-11-08.ours.tsv"
echo "== wrote $KAT"
