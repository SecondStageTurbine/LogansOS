#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# OT-61 C0.5 — apply the bounded `neoplutarch` std overlay to the active
# toolchain's rust-src. The logic (pristine restore, module copy, seam patches,
# fallback audit) lives in apply-overlay.py; see its header. xtask runs this
# only for the std_real_smoke scenario, never on a default build.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$HERE/apply-overlay.py"
