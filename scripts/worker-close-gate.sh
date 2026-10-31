#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# worker-close-gate.sh — the close-gate ritual every coder runs BEFORE declaring
# a build done. The worker-side mirror of pm-merge-check.sh: pm-merge-check is
# the PM's backstop at merge time; this is the worker's self-check that makes the
# branch arrive ff-clean and CI-green so the merge is trivial.
#
# It exists because three failure classes bounced builds this session, every one
# of them catchable here instead of at PM-merge time:
#   1. STALE BASE   — branch cut from an old main, never rebased (cherry-pick tax,
#                     phantom-deletion squashes). Step 1 catches it.
#   2. partial gate — ran clippy locally but not fmt, or ran on the pre-rebase
#                     tree. Step 2 runs the EXACT CI host-check set, in order.
#   3. "green" with no proof — reported done while branch CI was red. Step 4
#                     prints the RETURN-evidence block you must paste, including
#                     the green CI run URL(s). "Green" without a CI URL is NOT done.
#
# Usage:
#   scripts/worker-close-gate.sh
#
# Run it from the worker tree, on your feature branch, after your last commit.
# It does NOT push and does NOT rebase for you (rebasing rewrites your tree — do
# it deliberately). It tells you exactly what to do, runs the gates, and fails
# loudly if any step is not satisfied.
#
# Exit codes:
#   0  close-gate satisfied: rebased on current main + all host checks green.
#      Proceed: push, run the workflow_dispatch QEMU gate if kernel-touching,
#      then file the RETURN with the evidence block this script printed.
#   1  NOT satisfied: a step failed. Fix it and re-run. Do NOT declare done.
#   2  usage / environment error.

set -uo pipefail

MAIN_REF="${CLOSE_GATE_MAIN_REF:-origin/main}"
DC="${CLOSE_GATE_DOCKER:-docker compose run --rm neo}"

say()  { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
ok()   { printf '  \033[32mOK\033[0m   %s\n' "$*"; }
bad()  { printf '  \033[31mFAIL\033[0m %s\n' "$*"; }
note() { printf '       %s\n' "$*"; }

cd "$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "not in a git repo" >&2; exit 2; }

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" == "main" || "$BRANCH" == "HEAD" ]]; then
  echo "Run this on your feature branch, not '$BRANCH'." >&2; exit 2
fi

FAILED=0

# ── Step 1 — rebase freshness (the staleness fix at the source) ───────────────
say "Step 1/4 — branch is rebased on current $MAIN_REF"
git fetch origin --quiet 2>/dev/null || git fetch origin
BASE="$(git merge-base HEAD "$MAIN_REF" 2>/dev/null || true)"
MAIN_TIP="$(git rev-parse "$MAIN_REF" 2>/dev/null || true)"
if [[ -z "$MAIN_TIP" ]]; then
  bad "cannot resolve $MAIN_REF — fetch failed?"; FAILED=1
elif [[ "$BASE" == "$MAIN_TIP" ]]; then
  ok "branch base == $MAIN_REF tip (${MAIN_TIP:0:7}) — ff-clean"
else
  bad "STALE BASE: your base ${BASE:0:7} is behind $MAIN_REF tip ${MAIN_TIP:0:7}"
  note "Rebase before continuing (this is the close-gate's first requirement):"
  note "    git rebase $MAIN_REF"
  note "Then re-run this script — the gates below must pass on the REBASED tree."
  FAILED=1
fi

# ── Step 2 — the exact CI host-check set, in CI order, via Docker ─────────────
# Skipped if Step 1 is stale: gates must run on the rebased tree, not this one.
if [[ "$FAILED" -eq 0 ]]; then
  say "Step 2/4 — the exact CI host checks (via: $DC)"
  run_check() { # $1=label  $2..=cmd
    local label="$1"; shift
    printf '  ... %s\n' "$label"
    if $DC "$@" >/tmp/wcg.$$.log 2>&1; then ok "$label"; else
      bad "$label — tail:"; tail -15 /tmp/wcg.$$.log | sed 's/^/         /'; FAILED=1
    fi
  }
  run_check "cargo fmt --all --check"                         cargo fmt --all --check
  run_check "cargo test --workspace (host crates)"            cargo test --workspace --exclude neoplutarch-kernel
  run_check "cargo test -p ai-ops-tests (real-quant + AVX2)"  cargo test -p ai-ops-tests --features phase9-real-quant,phase9-avx2-matmul
  run_check "cargo test -p ai-ops-tests (soft-f32-only)"       cargo test -p ai-ops-tests --features soft-f32-only
  run_check "cargo test -p neo-atrium (app-icons)"             cargo test -p neo-atrium --features app-icons
  run_check "cargo test -p neo-atrium (install-chip)"          cargo test -p neo-atrium --features install-chip
  run_check "cargo test -p neo-atrium (install-chip,app-icons)" cargo test -p neo-atrium --features install-chip,app-icons
  run_check "cargo clippy --workspace --all-targets (-D)"     cargo clippy --workspace --all-targets -- -D warnings
  # kernel clippy is BLOCKING (ADR-056). Run it the way CI does, from kernel/.
  printf '  ... cargo clippy kernel (-D, blocking)\n'
  if $DC bash -c "cd kernel && cargo clippy -Zjson-target-spec -- -D warnings" >/tmp/wcg.$$.log 2>&1; then
    ok "cargo clippy kernel"
  else bad "cargo clippy kernel — tail:"; tail -15 /tmp/wcg.$$.log | sed 's/^/         /'; FAILED=1; fi
  # MT7601U kill-switch KAT: the shared xHCI host must remain independently
  # buildable when the WiFi feature (and its operation ledger) is absent.
  run_check "usb-host + !mt7601u compile KAT" bash -c \
    "cd kernel && cargo check -Zjson-target-spec --no-default-features --features usb-host"
  # The check above never assembles global_asm!, so it cannot see the
  # !xsave-fpu (FXSAVE) context-switch/syscall/timer asm. Build it, as CI does.
  run_check "!xsave-fpu kernel build (assembles the FXSAVE asm)" bash -c \
    "cd kernel && cargo build -Zjson-target-spec --bin kernel --no-default-features"
  # CI's "Product Feature Build" job builds and lints userland crates under
  # non-default features (the shipped desktop WMs and their boot tests). The
  # workspace clippy above sees only default features, so a warning under one
  # of those (an import left unused, #168) passed this script and failed CI.
  # Read the commands from ci.yml itself so the two cannot drift. Only lines
  # of the exact `(cd userland/<crate> && cargo build|clippy ...)` shape run;
  # any other `cd userland/` line fails the step, so a new shape is noticed.
  CI_YML=.github/workflows/ci.yml
  SHAPE='^\(cd userland/[a-z0-9_-]+ && cargo (build|clippy) -Zjson-target-spec --bin [a-z0-9_-]+ --features [a-z0-9_,-]+( -- -D warnings)?\)$'
  N=0
  while IFS= read -r line; do
    cmd="$(sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//' <<<"$line")"
    if [[ ! "$cmd" =~ $SHAPE ]]; then
      bad "unrecognised userland line in $CI_YML: $cmd"
      note "Teach this step the new shape (scripts/worker-close-gate.sh)."
      FAILED=1; continue
    fi
    N=$((N + 1))
    run_check "ci.yml: ${cmd:1:${#cmd}-2}" bash -c "$cmd" </dev/null
  done < <(grep -E '^[[:space:]]+\(cd userland/' "$CI_YML")
  if [[ "$N" -eq 0 ]]; then
    bad "found no product-feature userland commands in $CI_YML"; FAILED=1
  fi
  rm -f /tmp/wcg.$$.log
else
  say "Step 2/4 — SKIPPED (rebase first; gates must run on the rebased tree)"
fi

# ── Step 3 — SPDX headers on your new/changed source files ───────────────────
say "Step 3/4 — SPDX headers on changed source files"
MISSING=0
while IFS= read -r f; do
  [[ -z "$f" || ! -f "$f" ]] && continue
  case "$f" in
    # A pristine `cargo vendor` tree (third_party/<name>/vendor/<crate>-<ver>/):
    # its files are verified against crates.io by .cargo-checksum.json, so
    # they stay byte-identical to upstream (licences in third_party/<name>/
    # VENDORED.md). A vendored crate we PATCH lives outside vendor/ and is
    # checked like first-party code.
    third_party/*/vendor/*) ;;
    *.rs) head -5 "$f" | grep -q 'SPDX-License-Identifier' || { bad "missing SPDX: $f"; MISSING=1; } ;;
    *.toml|*.sh|*.ps1) head -3 "$f" | grep -q 'SPDX-License-Identifier' || { bad "missing SPDX: $f"; MISSING=1; } ;;
  esac
done < <(git diff --name-only "$MAIN_REF"...HEAD 2>/dev/null | grep -E '\.(rs|toml|sh|ps1)$' || true)
[[ "$MISSING" -eq 0 ]] && ok "all changed source files carry SPDX headers" || FAILED=1

# ── Step 4 — self-inspect + the RETURN-evidence block ────────────────────────
say "Step 4/4 — self-inspection (inspect your OWN diff, like the PM will)"
note "Your commit(s) vs $MAIN_REF — confirm this is what you intend (no phantom deletions):"
git diff --stat "$MAIN_REF"...HEAD 2>/dev/null | sed 's/^/    /'
KERNEL_TOUCHING=no
if git diff --name-only "$MAIN_REF"...HEAD 2>/dev/null | grep -qE '^(kernel/|boot/|tools/xtask/)'; then KERNEL_TOUCHING=yes; fi

echo
if [[ "$FAILED" -ne 0 ]]; then
  printf '\033[31m✗ CLOSE-GATE NOT SATISFIED — fix the FAILs above and re-run. Do NOT declare done.\033[0m\n'
  exit 1
fi
printf '\033[32m✓ CLOSE-GATE SATISFIED (host side).\033[0m Now, in order:\n'
note "1. git push -u origin $BRANCH"
if [[ "$KERNEL_TOUCHING" == "yes" ]]; then
  note "2. KERNEL-TOUCHING: trigger the pre-merge workflow_dispatch QEMU gate on this branch"
  note "   and confirm all 4 shards + host + SPDX are GREEN on CI."
else
  note "2. host-only change: no workflow_dispatch QEMU gate required (confirm the push CI is green)."
fi
note "3. File your RETURN with this evidence block — 'green' without a CI URL is NOT done:"
echo
echo "   ---- RETURN evidence (paste, filled in) ----"
echo "   Branch:        $BRANCH @ $(git rev-parse --short HEAD)"
echo "   Rebased on:    $MAIN_REF @ ${MAIN_TIP:0:7}  (ff-clean)"
echo "   Kernel-touching: $KERNEL_TOUCHING"
echo "   Push CI:       <paste green run URL>"
[[ "$KERNEL_TOUCHING" == "yes" ]] && echo "   QEMU gate:     <paste green workflow_dispatch run URL — all 4 shards>"
echo "   Self-stat:     $(git diff --stat "$MAIN_REF"...HEAD 2>/dev/null | tail -1 | sed 's/^ *//')"
echo "   ---------------------------------------------"
exit 0
