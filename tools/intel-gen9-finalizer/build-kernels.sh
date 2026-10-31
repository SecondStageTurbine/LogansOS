#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Deterministically finalize an OpenCL-C EU kernel source into a bare Gen9
# instruction stream using the pinned ocloc (igc-1.0.17193.4 / CR 24.26.30049.6).
#
# usage: build-kernels.sh <src.cl> <kernel_name> [device] [outdir] [ocloc_opts]
# emits  <outdir>/<base>.<device>.krn     (bare EU ISA, no ELF/zebin wrapper)
#        <outdir>/<base>.<device>.asm     (ocloc disasm text — the oracle)
#        <outdir>/<base>.<device>.zeinfo  (zebin .ze_info — the arg-ABI oracle)
# prints sha256 of source and of the extracted .krn.
#
# [ocloc_opts] is passed verbatim to `ocloc compile -options "<opts>"`; pass it
# empty (default) for the plain compile. The Add ships stateless via
# `-cl-intel-greater-than-4GB-buffers-required` (forces __global access through
# stateless A64); the no-op passes no options so its bytes stay identical.
set -euo pipefail
SRC="$1"; KNAME="$2"; DEV="${3:-skl}"; OUT="${4:-$(dirname "$SRC")}"; OPTS="${5:-}"
mkdir -p "$OUT"
base="$(basename "$SRC")"; base="${base%.cl}"
work="$(mktemp -d)"
if [ -n "$OPTS" ]; then
    ocloc compile -device "$DEV" -file "$SRC" -output "$work/$base" -output_no_suffix \
        -options "$OPTS" >/dev/null
else
    ocloc compile -device "$DEV" -file "$SRC" -output "$work/$base" -output_no_suffix >/dev/null
fi
BIN="$work/$base.bin"
# Intel zebin uses a custom ELF machine type binutils objcopy rejects, so pull
# the kernel section by its readelf-reported file offset + size (format-agnostic).
read -r offhex sizehex < <(readelf -SW "$BIN" | awk -v s=".text.$KNAME" '{for(i=1;i<=NF;i++) if($i==s){print $(i+3), $(i+4); exit}}')
off=$((16#$offhex)); size=$((16#$sizehex))
dd if="$BIN" of="$OUT/$base.$DEV.krn" bs=1 skip="$off" count="$size" status=none
# Independent disassembly + zeinfo (the byte/ABI oracles): a decode pass, not emit.
dis="$(mktemp -d)"; ocloc disasm -device "$DEV" -file "$BIN" -dump "$dis" >/dev/null 2>&1 || true
cp "$dis/.text.$KNAME.asm" "$OUT/$base.$DEV.asm"
[ -f "$dis/.ze_info" ] && cp "$dis/.ze_info" "$OUT/$base.$DEV.zeinfo"
sha256sum "$SRC" "$OUT/$base.$DEV.krn"
