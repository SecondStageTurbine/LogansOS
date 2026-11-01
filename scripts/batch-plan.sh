#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# batch-plan.sh — the OT-58 batch plan (docs/THINK-2026-11-01-ot58-tiered-gate.md,
# section 2.4), run by CI's plan job for a `batch` dispatch. For each member PR,
# in order:
#   1. resolve its head SHA, once;
#   2. require it green ON THAT SHA: a completed, successful `CI` run whose
#      `QEMU smoke (quick tier)` job succeeded, or, for a PR gate-plan puts in
#      full-alone (its own run is the 4 shards), all four shard jobs
#      succeeded. Skipped, neutral, cancelled or missing are not green;
#   3. require `gate-plan` to say `tier=quick` (a `full-alone` PR may only be a
#      batch of one);
# then assemble the batch tree and print the plan.
#
# Environment:
#   BATCH      space-separated PR numbers, in landing order (required)
#   BASE_SHA   the batch base; default: origin/main
#   PREFIX     run only the first N members (a bisect leg; 0 = the base alone)
#   GH_TOKEN   for the GitHub API (`gh api`)
#   REPO       owner/name
#
# Output (stdout, `key=value`, for $GITHUB_OUTPUT): base=, members= (PR:SHA
# pairs), tree=. Human-readable detail goes to stderr.

set -euo pipefail

: "${BATCH:?BATCH is required}"
: "${REPO:?REPO is required}"

git fetch --quiet --no-tags origin main
base="${BASE_SHA:-$(git rev-parse origin/main)}"
git fetch --quiet --no-tags origin "$base"
base="$(git rev-parse "${base}^{commit}")"

read -r -a prs <<<"$BATCH"
if [[ -n "${PREFIX:-}" ]]; then
  prs=("${prs[@]:0:${PREFIX}}")
fi
echo "batch-plan: base ${base}; members: ${prs[*]:-(none: the base alone)}" >&2

members=()
heads=()
for pr in "${prs[@]}"; do
  sha="$(gh api "repos/${REPO}/pulls/${pr}" --jq .head.sha)"
  git fetch --quiet --no-tags origin "+refs/pull/${pr}/head:refs/batch/pr-${pr}"

  # 2. Quick-green on this exact SHA.
  green_run=""
  for run in $(gh api "repos/${REPO}/actions/runs?head_sha=${sha}&per_page=50" \
      --jq '.workflow_runs[] | select(.name == "CI" and .status == "completed" and .conclusion == "success") | .id'); do
    # The PR's own run is green on this SHA in either tier: its smoke job
    # passed, or (a full-alone PR) all four shards did.
    qemu="$(gh api "repos/${REPO}/actions/runs/${run}/jobs?per_page=100" \
      --jq '[.jobs[] | select(.conclusion == "success") | .name]
            | ((map(select(. == "QEMU smoke (quick tier)")) | length) > 0)
              or ((map(select(startswith("QEMU Integration Tests (shard "))) | length) == 4)')"
    if [[ "$qemu" == true ]]; then
      green_run="$run"
      break
    fi
  done
  if [[ -z "$green_run" ]]; then
    echo "::error::PR #${pr} is not green on its head ${sha}: no successful CI run with a passing 'QEMU smoke (quick tier)' job or 4 passing shards"
    exit 1
  fi
  echo "batch-plan: PR #${pr} @ ${sha}: quick-green in run ${green_run}" >&2

  # 3. The tier gate-plan gives this PR on its own.
  tier="$(cargo run -q -p xtask -- gate-plan --base "$base" --head "$sha" 2>/dev/null | sed -n 's/^tier=//p')"
  if [[ "$tier" != quick && ${#prs[@]} -gt 1 ]]; then
    echo "::error::PR #${pr} is ${tier:-unplanned} by gate-plan: it can only be gated alone (a batch of one)"
    exit 1
  fi

  members+=("${pr}:${sha}")
  heads+=("$sha")
done

if [[ ${#heads[@]} -eq 0 ]]; then
  tree="$(git rev-parse "${base}^{tree}")"
else
  tree="$(scripts/batch-assemble.sh "$base" "${heads[@]}" | sed -n 's/^tree=//p')"
fi
echo "batch-plan: tree ${tree}" >&2

echo "base=${base}"
echo "members=${members[*]:-}"
echo "tree=${tree}"
