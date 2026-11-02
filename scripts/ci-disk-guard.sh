#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# A disk bound for CI jobs on the self-hosted runners (Janus #214 on the
# OT-61 closure job: a job's Cargo/rustup state and build dirs live in the
# workspace, with no free-space floor or budget).
#
#   ci-disk-guard.sh check  <min-free-GiB>
#       Fail before building if the workspace's filesystem has less free
#       space than this. The message starts "infra:": a full runner disk is
#       a runner-capacity red (docs/runbook-ci-runner-capacity.md), not a
#       test verdict.
#   ci-disk-guard.sh budget <max-GiB> <path>...
#       After building, fail if the paths together use more than this, so a
#       build that grows past its budget is seen the day it happens.
#   ci-disk-guard.sh clean  <path>...
#       Remove the paths (run it `if: always()`). Each must resolve inside
#       the workspace; anything else is refused.
#
# The workspace is $GITHUB_WORKSPACE, or the current directory outside CI.
set -euo pipefail

ws=$(realpath "${GITHUB_WORKSPACE:-$PWD}")
cmd=${1:-}
shift || true

gib() { awk -v k="$1" 'BEGIN { printf "%.1f", k / 1048576 }'; }

case "$cmd" in
check)
    min=${1:?usage: ci-disk-guard.sh check <min-free-GiB>}
    free_k=$(df -Pk "$ws" | awk 'NR == 2 { print $4 }')
    echo "disk guard: $(gib "$free_k") GiB free on $ws's filesystem (floor $min GiB)"
    if [ "$free_k" -lt $((min * 1048576)) ]; then
        echo "infra: runner ${RUNNER_NAME:-?} has $(gib "$free_k") GiB free, under this job's $min GiB floor;" \
            "free space on it (docs/runbook-ci-runner-capacity.md) and re-run" >&2
        exit 1
    fi
    ;;
budget)
    max=${1:?usage: ci-disk-guard.sh budget <max-GiB> <path>...}
    shift
    total_k=0
    for p in "$@"; do
        [ -e "$p" ] || { echo "disk budget: $p (absent)"; continue; }
        k=$(du -sk "$p" | awk '{ print $1 }')
        echo "disk budget: $p $(gib "$k") GiB"
        total_k=$((total_k + k))
    done
    echo "disk budget: total $(gib "$total_k") GiB (budget $max GiB)"
    if [ "$total_k" -gt $((max * 1048576)) ]; then
        echo "this job wrote $(gib "$total_k") GiB, over its $max GiB budget: raise the budget in the" \
            "workflow deliberately, or find what grew" >&2
        exit 1
    fi
    ;;
clean)
    for p in "$@"; do
        [ -e "$p" ] || continue
        real=$(realpath "$p")
        case "$real" in
        "$ws"/?*) rm -rf -- "$real" && echo "disk guard: removed $p" ;;
        *)
            echo "disk guard: refusing to remove $p ($real is not inside $ws)" >&2
            exit 1
            ;;
        esac
    done
    ;;
*)
    echo "usage: ci-disk-guard.sh check <min-free-GiB> | budget <max-GiB> <path>... | clean <path>..." >&2
    exit 2
    ;;
esac
