#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# revert-batch.sh — the OT-58 batch revert unit (docs/THINK-2026-11-01-ot58-tiered-gate.md,
# sections 2.4 and 6): undo every merge commit a batch landed on `main`, newest
# first, as ONE commit, so `main` stays one action from green.
#
# Usage (on an up-to-date `main`, clean tree):
#   scripts/revert-batch.sh BASE_SHA
#
# BASE_SHA is the batch's base (the `base_sha` its gate ran on). Every
# first-parent merge commit after it is reverted with `-m 1`; non-merge
# commits after it (docs-only commits the PM landed meanwhile) are kept. The
# result is committed locally, not pushed: check `git show --stat HEAD`, then
# push.
#
# Exit codes:
#   0  one revert commit made
#   1  a revert did not apply cleanly (the tree is reset; nothing committed)
#   2  usage error, BASE_SHA is not an ancestor of HEAD, a dirty tree, or no merges

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: scripts/revert-batch.sh BASE_SHA" >&2
  exit 2
fi
base="$1"

if ! git merge-base --is-ancestor "$base" HEAD; then
  echo "revert-batch: ${base} is not an ancestor of HEAD" >&2
  exit 2
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "revert-batch: the working tree is not clean" >&2
  exit 2
fi

# The batch's merges, newest first, along the first-parent chain.
mapfile -t merges < <(git rev-list --first-parent --merges "${base}..HEAD")
if [[ ${#merges[@]} -eq 0 ]]; then
  echo "revert-batch: no merge commits after ${base}" >&2
  exit 2
fi

for m in "${merges[@]}"; do
  if ! git revert --no-commit -m 1 "$m" >/dev/null 2>&1; then
    echo "revert-batch: reverting ${m} did not apply cleanly; nothing committed" >&2
    git revert --abort 2>/dev/null || git reset --quiet --hard
    exit 1
  fi
done

{
  echo "Revert the batch landed on ${base}"
  echo
  echo "Reverts these merges (newest first), as one commit:"
  for m in "${merges[@]}"; do
    echo "  $(git log -1 --format='%h %s' "$m")"
  done
} | git commit --quiet -F -

git log -1 --stat --format='%h %s'
