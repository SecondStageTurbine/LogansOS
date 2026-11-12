#!/bin/bash
# SPDX-License-Identifier: MPL-2.0
# Regenerate every desktop fixture from its independent oracle and compare it
# with the committed file, byte for byte. Exit 1 on any difference.
#
# The consolidation builds' equivalence proof (complexity-governance Rule 4):
# a refactor that changes no pixel must leave every fixture reproducible.
# usage: scripts/desktop/check_fixtures.sh [scratch-dir]
set -u
cd "$(dirname "$0")/../.."
out="${1:-target/desktop-fixtures}"
rm -rf "$out" && mkdir -p "$out"
fail=0

check() { # committed-fixture command...
  local fixture=$1
  shift
  if ! "$@" >"$out/$(basename "$fixture")" 2>"$out/$(basename "$fixture").err"; then
    echo "FAIL  $fixture (generator error, see $out)"
    fail=1
  elif cmp -s "$out/$(basename "$fixture")" "$fixture"; then
    echo "same  $fixture"
  else
    echo "DIFF  $fixture"
    fail=1
  fi
}

# The two RT label windows carry a hand-added SPDX line above the oracle's output.
spdx() {
  echo "// SPDX-License-Identifier: MPL-2.0"
  "$@"
}

d=scripts/desktop
font=assets/fonts/Inter-Regular.ttf
check fixtures/desktop-s3/scene_digests.rs python3 $d/scene_oracle.py fixture
check fixtures/desktop-s3/labels.rs python3 $d/label_oracle.py fixture
check fixtures/desktop-s3/bar_label_rt.rs spdx python3 scripts/fonts/area_oracle.py \
  bounds-window $font 13 Atrium 32 23 1a2130 ffffff 30 6 70 24 2 13
check fixtures/desktop-s3/title_label_rt.rs spdx python3 scripts/fonts/area_oracle.py \
  bounds-window $font 14 Arthur 692 359 1a2130 ffffff 688 341 52 26 2 14
check fixtures/desktop-ds1/home_digests.rs python3 $d/scene_oracle.py home-fixture
check fixtures/desktop-ds1/field_text.rs python3 $d/field_oracle.py fixture
check fixtures/desktop-ds1/lens_digests.rs python3 $d/scene_oracle.py lens-fixture
check fixtures/desktop-ds1/lens_content.rs python3 $d/lens_content_oracle.py fixture
check fixtures/desktop-ds1/lens_labels.rs python3 $d/lens_content_oracle.py labels
check fixtures/desktop-di1b/move_digests.rs python3 $d/scene_oracle.py move-fixture
check fixtures/desktop-di1b/resize_digests.rs python3 $d/scene_oracle.py resize-fixture
check fixtures/desktop-dl1/dock_digests.rs python3 $d/scene_oracle.py dock-fixture
check fixtures/desktop-dl1/dock_labels.rs python3 $d/lens_content_oracle.py dock-labels
check fixtures/desktop-dm1/motion_digests.rs python3 $d/scene_oracle.py motion-fixture
check fixtures/desktop-intro/intro.rs python3 $d/intro_oracle.py fixture
check fixtures/desktop-clock/clock.rs python3 $d/clock_oracle.py fixture
check fixtures/desktop-setup/setup.rs python3 $d/setup_oracle.py fixture
check fixtures/desktop-setup/setup_lang_es.rs python3 $d/setup_oracle.py fixture es
check fixtures/desktop-setup/setup_lang_en.rs python3 $d/setup_oracle.py fixture en

# Every fixture under fixtures/desktop-* must be covered above.
covered=$(grep -c '^check ' "$0")
present=$(ls fixtures/desktop-*/* | wc -l)
if [ "$covered" -ne "$present" ]; then
  echo "FAIL  $present desktop fixtures, $covered checked"
  fail=1
fi
exit $fail
