#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# batch-assemble.sh — build an OT-58 batch tree (docs/THINK-2026-11-01-ot58-tiered-gate.md,
# section 2.4): `main` at a fixed base, then each member's head merged in order
# with `git merge --no-ff`, under a fixed committer, author and date. The same
# inputs give the same tree, so every CI job of a batch tests one tree, and the
# PM re-assembles it locally before landing and compares the hash.
#
# Usage:
#   scripts/batch-assemble.sh BASE_SHA HEAD_SHA [HEAD_SHA ...]
#
# Every SHA must already be in the local object store (fetch the PR heads first,
# e.g. `git fetch origin pull/N/head`). Checks out a detached HEAD at the result.
#
# Output (stdout, one `key=value` per line, for $GITHUB_OUTPUT):
#   tree=<the batch tree hash>
#   commit=<the batch tip commit>
#
# Exit codes:
#   0  assembled
#   1  a member did not merge cleanly (stderr names it); the work tree is reset
#   2  usage error, or an unknown SHA

set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: scripts/batch-assemble.sh BASE_SHA HEAD_SHA [HEAD_SHA ...]" >&2
  exit 2
fi

base="$1"
shift

for sha in "$base" "$@"; do
  if ! git cat-file -e "${sha}^{commit}" 2>/dev/null; then
    echo "batch-assemble: unknown commit ${sha} (fetch it first)" >&2
    exit 2
  fi
done

# Fixed identity and date: the merge commits are then the same on every
# machine, not only their trees.
export GIT_AUTHOR_NAME="OT-58 batch"
export GIT_AUTHOR_EMAIL="batch@invalid"
export GIT_COMMITTER_NAME="OT-58 batch"
export GIT_COMMITTER_EMAIL="batch@invalid"
export GIT_AUTHOR_DATE="2000-01-01T00:00:00Z"
export GIT_COMMITTER_DATE="2000-01-01T00:00:00Z"

git -c advice.detachedHead=false checkout --quiet --detach "$base"

n=0
for head in "$@"; do
  n=$((n + 1))
  if ! git merge --quiet --no-ff --no-edit -m "batch: member ${n} ${head}" "$head" >/dev/null 2>&1; then
    echo "batch-assemble: member ${n} (${head}) does not merge cleanly onto the batch so far" >&2
    git merge --abort 2>/dev/null || git reset --quiet --hard
    exit 1
  fi
done

echo "tree=$(git rev-parse 'HEAD^{tree}')"
echo "commit=$(git rev-parse HEAD)"
