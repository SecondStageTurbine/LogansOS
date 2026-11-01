#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# The preview VM's build, run inside the dev container by
# .github/workflows/preview-image.yml: import Arthur's model (fetched and
# licence-checked by the workflow into target/preview/model), build the
# product image's disks with `cargo xtask vm-image --with-model`, convert them
# to qcow2, package the Windows zip and boot-check the zip.
#
# Environment: MODEL_FILE MODEL_REPO MODEL_REV SHA REF SHORT BUILD_DATE.
set -euo pipefail

out=target/preview
model=$out/model
mkdir -p "$out"

echo "== neomc import (B1-1's tool) =="
cargo build -p neomc --release
target/release/neomc import "$model/$MODEL_FILE" \
    -o "$model/model-qwen2.5-0.5b.neom" \
    --vocab-output "$model/model-qwen2.5-0.5b.vocab"

echo "== cargo xtask vm-image --with-model (the product image) =="
# xtask holds the .neom and the vocab to the kernel's own BLAKE3 pins and
# refuses to write an image the kernel would refuse at boot.
cargo xtask vm-image --with-model "$model/model-qwen2.5-0.5b.neom" --fresh-data

echo "== qcow2 =="
qemu-img convert -f raw -O qcow2 target/vm/boot.raw "$out/boot.qcow2"
qemu-img convert -f raw -O qcow2 target/vm/data.raw "$out/data.qcow2"
qemu-img info "$out/boot.qcow2"
qemu-img info "$out/data.qcow2"

limine_license=$(ls "${LIMINE_DIR:-/opt/limine}"/LICENSE* | head -1)
neom_sha=$(sha256sum "$model/model-qwen2.5-0.5b.neom" | cut -d' ' -f1)
gguf_sha=$(sha256sum "$model/$MODEL_FILE" | cut -d' ' -f1)
cat > "$out/MODEL-NOTICE.txt" <<EOF
Arthur's language model in this preview
=======================================

Model:    Qwen2.5-0.5B-Instruct, Q4_K_M quantization (GGUF)
Source:   https://huggingface.co/${MODEL_REPO}
Revision: ${MODEL_REV}
File:     ${MODEL_FILE}
          SHA-256 ${gguf_sha} (equal to the upstream LFS oid at that revision)
Author:   Copyright (c) Alibaba Cloud (the Qwen team)
Licence:  Apache License 2.0; the upstream LICENSE file at that revision is
          MODEL-LICENSE-Apache-2.0.txt next to this file. The upstream
          repository has no NOTICE file at that revision.

Changes made by LogansOS: the GGUF was converted into LogansOS's own .neom
container and a tokenizer vocabulary file (no retraining), by the neomc tool (tools/neomc in
https://github.com/SecondStageTurbine/LogansOS at ${SHA}):
  neomc import ${MODEL_FILE} -o model-qwen2.5-0.5b.neom
               --vocab-output model-qwen2.5-0.5b.vocab
  model-qwen2.5-0.5b.neom  SHA-256 ${neom_sha}
The converted files are inside disks/boot.qcow2 (/models and /vocab).
EOF

echo "== package =="
zip="$out/LogansOS-preview-${SHORT}.zip"
rm -f "$out"/LogansOS-preview-*.zip
python3 scripts/preview/preview.py package \
    --zip "$zip" \
    --boot "$out/boot.qcow2" \
    --data "$out/data.qcow2" \
    --ovmf-dir /usr/local/share/qemu \
    --license LogansOS-LICENSE-MPL-2.0.txt=LICENSE \
    --license LogansOS-NOTICE.txt=NOTICE \
    --license OFL-Inter.txt=assets/fonts/OFL-Inter.txt \
    --license OFL-IBM-Plex.txt=assets/fonts/OFL-IBM-Plex.txt \
    --license Limine-LICENSE.txt="$limine_license" \
    --license OVMF-edk2-licenses.txt="$out/edk2-licenses.txt" \
    --license MODEL-LICENSE-Apache-2.0.txt="$model/LICENSE" \
    --license MODEL-NOTICE.txt="$out/MODEL-NOTICE.txt" \
    --sha "$SHA" \
    --ref "$REF" \
    --date "$BUILD_DATE" \
    --with-model yes
echo yes > "$out/with-model.txt"

echo "== boot check (the zip, booted as Start LogansOS.bat boots it) =="
python3 scripts/preview/preview.py boot-check \
    --zip "$zip" \
    --log-out "$out/boot-check-serial.log" \
    --summary "$out/boot-check.txt"
