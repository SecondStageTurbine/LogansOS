<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CI artifact-storage quota — one-time prune (ops) → Oracle

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Oracle (first task — bounded host/ops, no gate). No branch/code change needed (this is a GitHub-API ops action, not a build).
**Status:** DISPATCHED. **The GitHub Actions artifact-storage quota is exhausted** — every gate's serial-log upload now errors with `Failed to CreateArtifact: Artifact storage quota has been hit` (surfaced on F3's QEMU run 27977525100, shard 2/4). It is **non-blocking** (the upload step is `continue-on-error`, so gates don't redden from it) but it means **diagnostic serial-log uploads are failing**, so post-mortems on real failures are degraded until headroom returns. The recurrence fix is already in `ci.yml` (3-day retention + upload-on-failure-only + `continue-on-error`); this is the **one-time backlog prune** the `ci.yml` comment explicitly defers to "a separate ops action."

## 0. The task
Delete the accumulated `neo-test-serial-logs` artifacts (this session's many gate runs piled them up) to reclaim the Actions artifact-storage quota. These are **pure diagnostics** (serial console logs from failed shards) — safe to drop. **Do NOT delete run *logs*** (we use those for post-mortems) and **do NOT touch any non-`neo-test-serial-logs` artifacts.**

## 1. Commands
```bash
# 1. Inventory (id · bytes · name · created) — see the bloat, confirm it's neo-test-serial-logs:
gh api --paginate /repos/SecondStageTurbine/NeoPlutarch/actions/artifacts \
  --jq '.artifacts[] | "\(.id)\t\(.size_in_bytes)\t\(.name)\t\(.created_at)"' | sort -k2 -n | tail -40
gh api /repos/SecondStageTurbine/NeoPlutarch/actions/artifacts --jq '.total_count'

# 2. Delete every neo-test-serial-logs artifact (the diagnostics bloat) — parallel, id-by-id:
gh api --paginate /repos/SecondStageTurbine/NeoPlutarch/actions/artifacts \
  --jq '.artifacts[] | select(.name=="neo-test-serial-logs") | .id' \
  | xargs -I{} -P4 gh api -X DELETE /repos/SecondStageTurbine/NeoPlutarch/actions/artifacts/{}

# 3. Verify: re-run the inventory; confirm the neo-test-serial-logs entries are gone + report the new total_count.
```
(Web-UI equivalent if `gh` is unavailable: repo → **Actions → Storage/Management**, delete the `neo-test-serial-logs` artifacts.)

## 2. Scope guard
Delete **only** `neo-test-serial-logs` artifacts via the API **only**. **Do NOT:** delete run logs (`delete_workflow_run_logs` — those are our diagnostics), delete artifacts of any other name, change `ci.yml` (the recurrence fix is already in place — do not re-touch it), touch any branch/code, or trigger any CI. No commit, no gate.

## 3. RETURN
A short report: artifacts deleted (count + approximate bytes freed), the before/after `total_count`, and a note that the quota **recalculates every 6–12 h** so upload headroom returns after that lag (not instantly). No CI URL needed (this is an ops action, not a build).

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-ci-artifact-quota-prune-oracle.md`. **No branch — this is a GitHub-API ops task, not a build.**
- **One-line:** The Actions **artifact-storage quota is full** (serial-log uploads erroring; non-blocking — `continue-on-error` — but diagnostics are degraded). **One-time prune:** `gh api --paginate .../actions/artifacts` → delete every artifact named **`neo-test-serial-logs`** (`gh api -X DELETE .../actions/artifacts/{id}`, parallel) → re-inventory to confirm. **Scope: delete ONLY `neo-test-serial-logs` artifacts; do NOT delete run logs, other artifacts, or touch `ci.yml`/any code.** RETURN: count + bytes freed + before/after `total_count` (note the 6–12 h quota-recalc lag — headroom returns after that). No gate, no commit.
