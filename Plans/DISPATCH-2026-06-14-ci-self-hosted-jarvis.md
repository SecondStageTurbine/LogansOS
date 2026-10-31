<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Move CI to the self-hosted runner + cheap/expensive split → Jarvis

**Date:** 2026-06-14
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` HEAD: `codex/ci-self-hosted-jarvis`)
**Status:** DISPATCHED — **GATED on the self-hosted runner reporting "Idle"** in repo Settings → Actions → Runners. Do not push until the PM confirms the runner is up (the workflow's first run executes *on* that runner, which is the test).
**Why:** the GitHub-hosted Actions minutes are exhausted (100%, private repo ⇒ hosted runs blocked). PL chose a **self-hosted runner on their main PC (WSL2 Ubuntu + Docker)**. This moves CI to hardware we own — zero GitHub-hosted minutes — and keeps CI a live gate. Pairs with the standing rule: *cost discipline belongs in what CI runs, not in skipping the gate* (CLAUDE.md).

## 1. Tasks (edit `.github/workflows/ci.yml`)
1. **Flip every job `runs-on: ubuntu-24.04` → `runs-on: self-hosted`** (host job, SPDX, the 4 QEMU shards). Single runner, plain `self-hosted` label.
2. **Drop / guard the GitHub-hosted-only steps** that don't apply on a self-hosted box: the "Free disk space" action (it's an Ubuntu-GHA helper — remove it or replace with a lightweight local prune; the PC manages its own disk). Verify the Docker buildx + cache steps still work on the runner (Docker via WSL integration); `actions/cache` still functions on self-hosted (uses cache storage, not minutes) — keep it, or switch to a local cache dir if simpler.
3. **The cheap/expensive split** (the point of the exercise — don't tie the PC up ~80 min on every push):
   - **Cheap gate — every trigger:** the host job (`cargo fmt --check`, `clippy -D warnings` workspace + kernel, `cargo test --workspace`, shard-coverage) + SPDX. ~2 min. This is the fast per-push signal.
   - **Expensive gate — the 4 QEMU shards:** run on **push to `main`** (every merge is the real gate) **+ a nightly `schedule:` + `workflow_dispatch:`** (manual on-demand). Keep them OFF routine non-main activity. Use job-level `if:` conditions keyed on `github.event_name`/`github.ref` so the split is one workflow, not two.
4. **Confirm the triggers** still fire correctly for the direct-to-main flow (push to main runs both gates; `workflow_dispatch` available for a manual full run).

## 2. Gate (the terminal state)
The first workflow run **on the self-hosted runner** is GREEN — both the cheap gate and the QEMU family (on the main-push that lands this change). Zero GitHub-hosted minutes consumed. If the runner can't build the dev container or run QEMU (Docker-in-WSL issue, path issue), diagnose and BLOCKER-to-PM with the specific failure — don't paper it over.

## 3. Scope guard
CI infra only (`.github/workflows/ci.yml`, maybe a tiny helper script). **Do NOT** weaken any gate (same fmt/clippy/test/QEMU coverage, just relocated + scheduled); **do NOT** touch kernel/AI code; **do NOT** wire the runner to execute untrusted external PRs (self-hosted security). The local worker full-family run remains the gate of record regardless.

## 4. Merge (PM)
On the runner reporting Idle: Jarvis pushes the branch → it runs on the self-hosted runner → PM `pm-merge-check` → `--ff-only` → `main`, and we watch the on-`main` run go green (on the runner). Then CI is a live, free gate again, and CLAUDE.md's `[skip ci]` note gets a one-line update (the gate runs on our own hardware now).

---

## Relay — what to tell Jarvis (AFTER the runner is Idle)
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-14-ci-self-hosted-jarvis.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-14-ci-self-hosted-jarvis.md
  ```
- **Branch:** `codex/ci-self-hosted-jarvis` off current `origin/main`.
- **One-line:** GitHub-hosted minutes are out; CI now runs on a **self-hosted runner (WSL2 Ubuntu + Docker on the PL's PC)**. Edit `.github/workflows/ci.yml`: flip all jobs to `runs-on: self-hosted`; drop the GitHub-hosted-only "Free disk space" step; keep Docker buildx + cache (works on self-hosted). **Split cheap/expensive:** host gate (fmt/clippy/test/shard-coverage/SPDX, ~2 min) on every trigger; the **4 QEMU shards** only on push-to-`main` + a nightly `schedule:` + `workflow_dispatch:` (job-level `if:` on event/ref, one workflow). Gate = the first run **on the self-hosted runner** is green, zero hosted minutes. Don't weaken coverage; don't run untrusted PRs on the runner. **Wait for the PM's "runner is Idle" before pushing.**
