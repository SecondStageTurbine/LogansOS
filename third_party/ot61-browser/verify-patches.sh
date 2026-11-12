#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
#
# OT-61 L0: reproduce every patched/<crate>-<version>/ from the published
# crate plus its <crate>-<version>-neoplutarch.patch, and fail on any
# difference. For each patch, it:
#   1. downloads https://static.crates.io/crates/<crate>/<crate>-<version>.crate
#      (raw bytes, curl) and checks its sha256 against this directory's
#      VENDORED.md row for "<crate> (patched)" (the last column);
#   2. unpacks it as vendor/<crate>-<version>/ in a scratch directory and
#      applies the patch there;
#   3. diffs the result against patched/<crate>-<version>/, ignoring exactly
#      what the patches were made ignoring (the SPDX lines our patched files
#      add, .cargo-checksum.json and .cargo-ok).
# It needs curl, sha256sum, tar, patch and diff, and the network to reach
# static.crates.io. It never writes outside its scratch directory.
#
# Usage: verify-patches.sh [crate-version ...]   (default: every patch)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
if [ $# -eq 0 ]; then
  set -- $(cd "$here" && ls *-neoplutarch.patch | sed 's/-neoplutarch\.patch$//')
fi
fail=0
for cv in "$@"; do
  name=${cv%-*}
  ver=${cv##*-}
  want=$(grep -E "^\| ${name} \(patched\) \| ${ver} \|" "$here/VENDORED.md" \
    | head -1 | awk -F'|' '{print $(NF-1)}' | tr -d ' `')
  case "$want" in
    [0-9a-f]*) ;;
    *) echo "FAIL $cv: no sha256 for '${name} (patched) ${ver}' in VENDORED.md"; fail=1; continue ;;
  esac
  crate="$tmp/$cv.crate"
  curl -sSfL -o "$crate" "https://static.crates.io/crates/$name/$cv.crate"
  got=$(sha256sum "$crate" | cut -d' ' -f1)
  if [ "$got" != "$want" ]; then
    echo "FAIL $cv: the .crate's sha256 is $got, VENDORED.md says $want"; fail=1; continue
  fi
  mkdir -p "$tmp/vendor"
  tar -xzf "$crate" -C "$tmp/vendor"
  (cd "$tmp" && patch -s -p0 -N --no-backup-if-mismatch < "$here/$cv-neoplutarch.patch")
  if diff -r -I SPDX-License-Identifier -x .cargo-checksum.json -x .cargo-ok \
      "$tmp/vendor/$cv" "$here/patched/$cv" > "$tmp/$cv.diff"; then
    echo "ok   $cv: published .crate ($got) + patch == patched/$cv"
  else
    echo "FAIL $cv: published .crate + patch differs from patched/$cv:"
    head -40 "$tmp/$cv.diff"
    fail=1
  fi
done
exit "$fail"
