#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# pm-merge-check.sh — pre-merge safety check for the Cerebro (PM) seat.
#
# Guards against the "stale branch base" trap: a worker branch cut from an old
# main can't fast-forward, and a plain merge would drag in phantom deletions of
# everything main has gained since the worker branched. This script reports the
# exact relationship and tells you whether a clean ff-merge is possible — or
# whether you must cherry-pick the worker's new commits onto current main.
#
# Usage:
#   scripts/pm-merge-check.sh <worker-branch>
#   scripts/pm-merge-check.sh origin/codex/phase-5-build-3-cpu-inference-jarvis
#
# Exit codes:
#   0  ff-safe: worker branch is ahead of main with main as its ancestor — a
#      clean `git push origin <branch>:main` (or --ff-only merge) will work.
#   1  STALE: worker branch does NOT contain current main's tip. Do NOT merge
#      naively. Cherry-pick the listed worker-only commits onto main instead.
#   2  usage / git error.

set -euo pipefail

BRANCH="${1:-}"
MAIN_REF="${PM_MAIN_REF:-origin/main}"

if [[ -z "$BRANCH" ]]; then
  echo "usage: $0 <worker-branch>   (e.g. origin/codex/phase-5-build-3-...)" >&2
  exit 2
fi

echo "==> fetching latest refs"
git fetch origin --quiet || { echo "ERROR: git fetch failed" >&2; exit 2; }

# Resolve refs; allow either 'origin/foo' or bare 'foo' (tries origin/ prefix).
if ! git rev-parse --verify --quiet "$BRANCH^{commit}" >/dev/null; then
  # Try, in order: origin/<name>, and (if the arg already had origin/) the
  # bare name. Fetch the specific branch first so a not-yet-local ref resolves.
  bare="${BRANCH#origin/}"
  git fetch origin "$bare" >/dev/null 2>&1 || true
  if git rev-parse --verify --quiet "origin/$bare^{commit}" >/dev/null; then
    BRANCH="origin/$bare"
  elif git rev-parse --verify --quiet "$bare^{commit}" >/dev/null; then
    BRANCH="$bare"
  else
    echo "ERROR: cannot resolve branch '$BRANCH' (tried 'origin/$bare' and '$bare'). Is it pushed to origin?" >&2
    exit 2
  fi
fi

MAIN_SHA=$(git rev-parse --short "$MAIN_REF")
BR_SHA=$(git rev-parse --short "$BRANCH")
BASE=$(git merge-base "$MAIN_REF" "$BRANCH")
BASE_SHA=$(git rev-parse --short "$BASE")

echo
echo "  main ($MAIN_REF):   $MAIN_SHA"
echo "  branch ($BRANCH):   $BR_SHA"
echo "  merge-base:         $BASE_SHA"
echo

# The decisive test: is current main an ancestor of the worker branch?
if git merge-base --is-ancestor "$MAIN_REF" "$BRANCH"; then
  echo "RESULT: ff-safe ✅"
  echo "  '$MAIN_REF' is an ancestor of '$BRANCH'. A clean fast-forward merge works:"
  echo "    git push origin $BRANCH:main"
  echo
  echo "  Worker-only commits that will land on main:"
  git log --oneline "$MAIN_REF..$BRANCH" | sed 's/^/    /'
  exit 0
fi

# Stale: main has advanced past the branch's base.
echo "RESULT: STALE BASE ❌  — DO NOT merge naively."
echo
echo "  '$BRANCH' was cut from $BASE_SHA but main is now $MAIN_SHA."
echo "  A naive merge/reset would drop everything main gained since the branch base."
echo
echo "  Commits on main the branch is MISSING (would be clobbered by a reset):"
git log --oneline "$BRANCH..$MAIN_REF" | sed 's/^/    /'
echo
echo "  The worker's NEW commits to cherry-pick onto current main instead:"
git log --oneline "$BASE..$BRANCH" | sed 's/^/    /'
echo
echo "  Recommended recovery (review each first):"
echo "    git checkout -B <pm-branch> $MAIN_REF"
echo "    git cherry-pick $BASE_SHA..$BR_SHA      # the worker-only range above"
echo "    # resolve any conflicts, verify build/tests, then: git push origin HEAD:main"
exit 1
