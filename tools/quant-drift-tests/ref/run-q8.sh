#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# The drift check on Arthur's Q8_0 model (#90), with the bars unchanged:
#   bar A: ours vs llama.cpp on the same Q8_0 file;
#   bar B: Q8_0 vs the same-checkpoint upstream F16 (llama.cpp).
# Same prompts, same pinned WikiText-2 window (offset 6,144, 2,048 tokens,
# eval from 1,024, BLAKE3 1782216d...) and same token IDs as
# ref/run-control.sh. Control C does not depend on Arthur's file and is not
# re-run here: its fixtures stay as they are.
#
# Inputs (environment):
#   QUANT_DRIFT_NEOM_Q8      model-qwen2.5-0.5b.neom imported from the Q8_0 GGUF
#                            (BLAKE3 1cfd06dc..., the #90 pin), .vocab beside it
#   QUANT_DRIFT_GGUF_Q8      upstream q8_0 GGUF (SHA-256 ca59ca7f...)
#   QUANT_DRIFT_GGUF_F16_UP  upstream fp16 GGUF (SHA-256 8e0ae260...)
#   QUANT_DRIFT_CORPUS_TEXT  WikiText-2 raw test.txt (SHA-256 bbf94c53...)
#   QUANT_DRIFT_LLAMA_DIR    built llama.cpp checkout (pinned a2b30183...)
#   QUANT_DRIFT_WORK         scratch dir (default /tmp/quant-drift-q8)
#   QUANT_DRIFT_OUT          output dir (default this crate's fixtures/)
#
# The scalar host forward pass runs about 9 s/token on Q8_0 on a 4-core cloud
# host (2026-11-09), so the window alone is several hours. Every prompt and the
# window are independent, so they may be run as separate `run` processes and
# their TSVs merged with ref/merge-split.py; the committed KAT was produced
# that way, and its four pre-merge runs are committed in
# fixtures/quant-drift-kat-2026-11-09-q8-split/. The GGUFs, .neom and corpus
# text are never committed.
#
# Refuses: a .neom, Q8_0 GGUF or window that isn't the pin, a llama.cpp
# checkout other than a2b30183, and prompts whose text or token IDs differ from
# the pinned Q4_K_M KAT's.

set -euo pipefail

CRATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$CRATE_DIR/../.." && pwd)"
WORK="${QUANT_DRIFT_WORK:-/tmp/quant-drift-q8}"
OUT="${QUANT_DRIFT_OUT:-$CRATE_DIR/fixtures}"
LLAMA_DIR="${QUANT_DRIFT_LLAMA_DIR:?set QUANT_DRIFT_LLAMA_DIR}"
NEOM="${QUANT_DRIFT_NEOM_Q8:?set QUANT_DRIFT_NEOM_Q8}"
GGUF_Q8="${QUANT_DRIFT_GGUF_Q8:?set QUANT_DRIFT_GGUF_Q8}"
GGUF_F16_UP="${QUANT_DRIFT_GGUF_F16_UP:?set QUANT_DRIFT_GGUF_F16_UP}"
CORPUS_TEXT="${QUANT_DRIFT_CORPUS_TEXT:?set QUANT_DRIFT_CORPUS_TEXT}"
# The llama.cpp commit is pinned, not taken from whatever is checked out.
LLAMA_PIN=a2b301831d05b07860de508d229d430207127802
LLAMA_COMMIT="$(git -C "$LLAMA_DIR" rev-parse HEAD)"
if [ "$LLAMA_COMMIT" != "$LLAMA_PIN" ]; then
    echo "refusing: llama.cpp checkout is $LLAMA_COMMIT, not the pinned $LLAMA_PIN" >&2
    exit 1
fi
GGUF_REV="${QUANT_DRIFT_GGUF_REV:-9217f5db79a29953eb74d5343926648285ec7e67}"
DATE="${QUANT_DRIFT_DATE:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
WINDOW=(--corpus-offset 6144 --corpus-tokens 2048 --eval-start 1024)

mkdir -p "$WORK" "$OUT"
cd "$REPO_ROOT"

echo "== build"
cargo build --release -p quant-drift-tests
BIN="$REPO_ROOT/target/release/quant-drift-tests"

echo "== prompt + corpus tokens (one tokenizer: the shipped .vocab)"
"$BIN" prompts --neom "$NEOM" --corpus-text "$CORPUS_TEXT" \
    --out-prompts "$WORK/prompts.tsv" --out-corpus "$WORK/corpus.u32"
python3 - "$WORK/corpus.u32" "$WORK/corpus-window.u32" <<'PY'
import sys
raw = open(sys.argv[1], "rb").read()
open(sys.argv[2], "wb").write(raw[6144 * 4:])
PY
# The prompts must be the SAME ones, with the SAME token IDs, as the pinned
# Q4_K_M run: compare every prompt's text and IDs with that KAT's, and refuse
# any difference (a changed prompts.rs or a different .vocab).
python3 - "$WORK/prompts.tsv" "$CRATE_DIR/fixtures/quant-drift-kat-2026-11-08-control.ours.tsv" <<'PY'
import sys
def prompts(path):
    rows = {}
    for line in open(path):
        f = line.rstrip("\n").split("\t")
        if f[0] in ("prompt", "prompt_ids"):
            rows.setdefault(f[1], {})[f[0]] = f[2]
    return rows
got, pinned = prompts(sys.argv[1]), prompts(sys.argv[2])
if got != pinned:
    diff = sorted(k for k in set(got) | set(pinned) if got.get(k) != pinned.get(k))
    sys.exit(f"refusing: prompts differ from the pinned Q4_K_M KAT: {diff}")
print(f"prompts: {len(got)} match the pinned KAT (text and token IDs)")
PY

echo "== build llama.cpp reference runner"
g++ -O2 -std=c++17 "$CRATE_DIR/ref/llama_drift.cpp" \
    -I "$LLAMA_DIR/include" -I "$LLAMA_DIR/src" -I "$LLAMA_DIR/ggml/include" \
    -L "$LLAMA_DIR/build/bin" -lllama -lggml -lggml-base -lggml-cpu \
    -Wl,-rpath,"$LLAMA_DIR/build/bin" -o "$WORK/llama_drift"

sha256() { sha256sum "$1" | cut -d' ' -f1; }
b3() { python3 -c 'import blake3,sys; h=blake3.blake3(); f=open(sys.argv[1],"rb"); [h.update(c) for c in iter(lambda: f.read(1<<20), b"")]; print(h.hexdigest())' "$1"; }

NEOM_SHA="$(sha256 "$NEOM")"
NEOM_B3="$(b3 "$NEOM")"
GGUF_Q8_SHA="$(sha256 "$GGUF_Q8")"
GGUF_F16_SHA="$(sha256 "$GGUF_F16_UP")"
CORPUS_B3="$(b3 "$WORK/corpus-window.u32")"
CORPUS_TEXT_SHA="$(sha256 "$CORPUS_TEXT")"

if [ "$NEOM_B3" != "1cfd06dc7fe42710f8fab7d337a681ac5527a46e5b2ba6204e929f210055cd4d" ]; then
    echo "refusing: .neom BLAKE3 $NEOM_B3 is not the Q8_0 pin 1cfd06dc..." >&2
    exit 1
fi
if [ "$GGUF_Q8_SHA" != "ca59ca7f13d0e15a8cfa77bd17e65d24f6844b554a7b6c12e07a5f89ff76844e" ]; then
    echo "refusing: Q8_0 GGUF SHA-256 $GGUF_Q8_SHA is not the pinned upstream file" >&2
    exit 1
fi
if [ "$CORPUS_B3" != "1782216d7805bdc404e80dbc34f5af707e290559711d6e8dc23b5173567cecdb" ]; then
    echo "refusing: corpus window BLAKE3 $CORPUS_B3 is not the pinned window" >&2
    exit 1
fi

echo "== ours (Q8_0 .neom, window)"
"$BIN" run --neom "$NEOM" \
    --prompts "$WORK/prompts.tsv" --corpus "$WORK/corpus.u32" \
    --label ours_q8_0_neom --sha256 "$NEOM_SHA" \
    --source "Arthur shipped .neom (Q8_0, BLAKE3 1cfd06dc...)" \
    --tool "quant-drift-tests (kernel forward_logic, release)" \
    --corpus-blake3 "$CORPUS_B3" "${WINDOW[@]}" \
    --out "$WORK/ours.tsv"

echo "== llama.cpp Q8_0 (upstream, the file the .neom was imported from)"
"$WORK/llama_drift" "$GGUF_Q8" "$WORK/prompts.tsv" "$WORK/corpus.u32" \
    "$WORK/llama-q8.tsv" llama_q8_0 \
    "Qwen/Qwen2.5-0.5B-Instruct-GGUF qwen2.5-0.5b-instruct-q8_0.gguf@$GGUF_REV" \
    "$GGUF_Q8_SHA" "llama.cpp $LLAMA_COMMIT; ref/llama_drift.cpp" \
    --corpus-blake3 "$CORPUS_B3" "${WINDOW[@]}"

echo "== llama.cpp F16 (upstream, same checkpoint)"
"$WORK/llama_drift" "$GGUF_F16_UP" "$WORK/prompts.tsv" "$WORK/corpus.u32" \
    "$WORK/llama-f16-up.tsv" llama_f16_upstream \
    "Qwen/Qwen2.5-0.5B-Instruct-GGUF qwen2.5-0.5b-instruct-fp16.gguf@$GGUF_REV" \
    "$GGUF_F16_SHA" "llama.cpp $LLAMA_COMMIT; ref/llama_drift.cpp" \
    --corpus-blake3 "$CORPUS_B3" "${WINDOW[@]}"

cat > "$WORK/meta.tsv" <<META
date=$DATE
host=$(uname -srm)
neom_sha256=$NEOM_SHA
neom_blake3=$NEOM_B3
gguf_sha256=$GGUF_Q8_SHA
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

echo "== bars A and B on Q8_0"
KAT="$OUT/quant-drift-kat-2026-11-09-q8.json"
"$BIN" compare --quant Q8_0 \
    --ours "$WORK/ours.tsv" --q4 "$WORK/llama-q8.tsv" --fp32 "$WORK/llama-f16-up.tsv" \
    --meta "$WORK/meta.tsv" --kat-out "$KAT" \
    --markdown-out "$OUT/quant-drift-kat-2026-11-09-q8.report.md" || true
cp "$WORK/ours.tsv" "$OUT/quant-drift-kat-2026-11-09-q8.ours.tsv"
echo "== wrote $KAT"
