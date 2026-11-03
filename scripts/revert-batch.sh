#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# revert-batch.sh — the OT-58 batch revert unit (docs/THINK-2026-11-01-ot58-tiered-gate.md,
# sections 2.4 and 6): undo the merge commits ONE batch landed on `main`,
# newest first, as ONE commit, so `main` stays one action from green.
#
# Usage (on an up-to-date `main`, clean tree):
#   scripts/revert-batch.sh BASE_SHA
#
# BASE_SHA is the batch's base (the `base_sha` its gate ran on). The batch's
# merges are found by what scripts/batch-assemble.sh made them (Dexter #47
# item 2.6): walking `main`'s first-parent chain forward from BASE_SHA, the
# first merge authored by "OT-58 batch" with the subject
# "batch: member 1 <sha>", then the merges right after it with the subjects
# "batch: member 2 <sha>", "batch: member 3 <sha>", ... Nothing else is
# reverted: not a docs commit or another PR's merge landed before or after the
# batch, and not a later batch (it starts again at "member 1"). Each member
# merge's second parent must be the head its subject names.
#
# The result is committed locally, not pushed: check `git show --stat HEAD`,
# then push.
#
# Exit codes:
#   0  one revert commit made
#   1  a revert did not apply cleanly (the tree is reset; nothing committed)
#   2  usage error, BASE_SHA is not an ancestor of HEAD, a dirty tree, or no
#      batch found after BASE_SHA

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

# The batch's merges: the first run of "member 1, 2, ..." merges on the
# first-parent chain after the base, oldest first.
batch=()
while read -r c; do
  author="$(git log -1 --format=%an "$c")"
  subject="$(git log -1 --format=%s "$c")"
  parents=($(git log -1 --format=%P "$c"))
  member=""
  if [[ "$author" == "OT-58 batch" && ${#parents[@]} -eq 2 \
        && "$subject" =~ ^batch:\ member\ ([0-9]+)\ ([0-9a-f]{40})$ \
        && "${BASH_REMATCH[2]}" == "${parents[1]}" ]]; then
    member="${BASH_REMATCH[1]}"
  fi
  if [[ ${#batch[@]} -eq 0 ]]; then
    [[ "$member" == 1 ]] && batch+=("$c")
  elif [[ "$member" == $(( ${#batch[@]} + 1 )) ]]; then
    batch+=("$c")
  else
    break
  fi
done < <(git rev-list --reverse --first-parent "${base}..HEAD")
if [[ ${#batch[@]} -eq 0 ]]; then
  echo "revert-batch: no batch merges (\"batch: member 1 ...\") after ${base}" >&2
  exit 2
fi

# Newest first.
merges=()
for (( i = ${#batch[@]} - 1; i >= 0; i-- )); do
  merges+=("${batch[$i]}")
done

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
