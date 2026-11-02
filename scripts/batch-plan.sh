#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# batch-plan.sh — the OT-58 batch plan (docs/THINK-2026-11-01-ot58-tiered-gate.md,
# section 2.4), run by CI's plan job for a `batch` dispatch. For each member PR,
# in order:
#   1. resolve its head SHA, once;
#   2. ask `gate-plan` for its tier on its own: `quick`, or `full-alone`, which
#      may only be a batch of one (a failed plan is full-alone);
#   3. require it green ON THAT SHA, in the evidence its tier calls for: a
#      completed, successful `CI` run whose `QEMU smoke (quick tier)` job
#      succeeded (quick), or whose four `QEMU Integration Tests (shard i/4)`
#      jobs all succeeded (full-alone: its own run was the full tier). Skipped,
#      neutral, cancelled or missing are not green. A smoke run never stands in
#      for a full-alone PR (Dexter #47 item 2.1).
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
# The base is `main`, now or earlier: never a feature branch or a typo.
if ! git merge-base --is-ancestor "$base" origin/main; then
  echo "::error::base ${base} is not on main"
  exit 1
fi

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

  # 2. The tier gate-plan gives this PR on its own.
  if plan="$(cargo run -q -p xtask -- gate-plan --base "$base" --head "$sha" 2>/dev/null)"; then
    tier="$(sed -n 's/^tier=//p' <<<"$plan" | tail -n 1)"
  else
    tier=""
  fi
  [[ "$tier" == quick ]] || tier=full-alone
  if [[ "$tier" != quick && ${#prs[@]} -gt 1 ]]; then
    echo "::error::PR #${pr} is ${tier} by gate-plan: it can only be gated alone (a batch of one)"
    exit 1
  fi

  # 3. Green on this exact SHA, in the evidence the tier calls for.
  if [[ "$tier" == quick ]]; then
    want='(map(select(. == "QEMU smoke (quick tier)")) | length) > 0'
    what="a passing 'QEMU smoke (quick tier)' job"
  else
    want='(map(select(startswith("QEMU Integration Tests (shard ") and endswith("/4)"))) | unique | length) == 4'
    what="4 passing 'QEMU Integration Tests' shards"
  fi
  green_run=""
  for run in $(gh api --paginate "repos/${REPO}/actions/runs?head_sha=${sha}&per_page=100" \
      --jq '.workflow_runs[] | select(.name == "CI" and .status == "completed" and .conclusion == "success") | .id'); do
    qemu="$(gh api --paginate "repos/${REPO}/actions/runs/${run}/jobs?per_page=100" \
      --jq '.jobs[] | select(.conclusion == "success") | .name' | jq -R . | jq -s "$want")"
    if [[ "$qemu" == true ]]; then
      green_run="$run"
      break
    fi
  done
  if [[ -z "$green_run" ]]; then
    echo "::error::PR #${pr} (${tier}) is not green on its head ${sha}: no successful CI run with ${what}"
    exit 1
  fi
  echo "batch-plan: PR #${pr} @ ${sha}: ${tier}, green in run ${green_run}" >&2

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
