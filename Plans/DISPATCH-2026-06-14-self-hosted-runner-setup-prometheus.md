<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Stand up the GitHub Actions self-hosted runner → Prometheus

**Date:** 2026-06-14
**From:** PM/Cerebro
**To:** Prometheus (ops/infra task on the PL's main PC; under `E:\AI Automation\AltimitNeo` per PL direction)
**Status:** DISPATCHED. **Goal: a GitHub Actions self-hosted runner for `SecondStageTurbine/NeoPlutarch`, registered, persistent, and showing "Idle" — so CI runs on PL hardware at zero GitHub-hosted minutes** (the hosted minutes are exhausted). When it's Idle, the staged `ci.yml` change (`Plans/DISPATCH-2026-06-14-ci-self-hosted-jarvis.md`, Jarvis) flips the workflow onto it.

## 0. The two hard constraints (do not skip — they're the whole reason this is a dispatch, not a one-liner)
1. **The runner MUST be a Linux runner in WSL2 Ubuntu, NOT Windows-native.** Our CI builds a **Linux dev container and runs QEMU inside Docker** (`ci.yml`: "Build dev container", "Run QEMU integration tests"). A Windows-native runner cannot run that workflow without a full rewrite. So: install/run the runner **inside WSL2 Ubuntu** on the main PC. **Docker (with `buildx`) must be reachable from that WSL2 distro** — verify `docker buildx version` succeeds in the runner's shell before declaring done (Docker Desktop → Settings → Resources → WSL Integration → enable for the distro, or a native docker-engine in WSL).
2. **The runner must NOT pollute the git tree.** `E:\AI Automation\AltimitNeo` is the repo working tree — dropping `actions-runner/` directly inside it makes the runner show up as untracked *and* the runner would check the repo out into its own `_work` subdir (nested-repo mess). Honor "under `E:\AI Automation\AltimitNeo`" **safely** via ONE of:
   - **(preferred)** install at `E:\AI Automation\AltimitNeo\actions-runner` (in WSL2: `/mnt/e/AI\ Automation/AltimitNeo/actions-runner`) **and add `/actions-runner/` to the repo `.gitignore`** (a legit 1-line repo commit) so it stays clean; OR
   - a **sibling** dir `E:\AI Automation\AltimitNeo-runner` (organizationally adjacent, zero git-pollution risk).
   - **Performance note:** `/mnt/e/...` (Windows FS via WSL 9p) is slow for cargo/Docker I/O. Strongly consider pointing the runner's **work directory** at the WSL **native** filesystem with `./config.sh --work /home/<user>/altimitneo-runner-work` even if the runner package lives under the E: path. Your call on the actual machine — optimize for build speed.

## 1. The setup (inside WSL2 Ubuntu)
1. **Get a FRESH registration token.** The one shared earlier is expired (registration tokens last ~1 hour). The PL generates a new one: GitHub.com → `SecondStageTurbine/NeoPlutarch` → Settings → Actions → Runners → **New self-hosted runner** → Linux x64. **The PL hands you the fresh token at run time — do NOT write any token into a committed file, this dispatch, or a RETURN.**
2. **Install** (the GitHub-provided block, run in WSL2 at the chosen path):
   ```
   mkdir actions-runner && cd actions-runner
   curl -o actions-runner-linux-x64-2.335.1.tar.gz -L https://github.com/actions/runner/releases/download/v2.335.1/actions-runner-linux-x64-2.335.1.tar.gz
   echo "4ef2f25285f0ae4477f1fe1e346db76d2f3ebf03824e2ddd1973a2819bf6c8cf  actions-runner-linux-x64-2.335.1.tar.gz" | shasum -a 256 -c
   tar xzf ./actions-runner-linux-x64-2.335.1.tar.gz
   ./config.sh --url https://github.com/SecondStageTurbine/NeoPlutarch --token <FRESH-TOKEN-FROM-PL> [--work /home/<user>/altimitneo-runner-work]
   ```
3. **Make it persistent** (survives reboots / terminal close) — install as a service rather than `./run.sh`:
   ```
   sudo ./svc.sh install
   sudo ./svc.sh start
   ```
4. **Verify Docker** in the runner's environment: `docker buildx version` succeeds.

## 2. Done = (report all four in the RETURN)
1. The runner shows **"Idle"** in repo Settings → Actions → Runners.
2. Its **labels** (default `self-hosted, Linux, X64` — the `ci.yml` change keys on `self-hosted`).
3. `docker buildx version` works in the runner's shell.
4. The chosen **location** + whether `.gitignore` was updated (and the `--work` dir, if set).

A self-hosted run consumes **zero** GitHub-hosted minutes, so once it's Idle, Jarvis's `ci.yml` push will exercise it for free — that first run is the real test.

## 3. Scope guard
Ops only: stand up the runner. **Do NOT** edit `ci.yml` (that's Jarvis's staged dispatch, fired after you report Idle). **Do NOT** commit any token or the runner binaries/`_work` to the repo (only a `.gitignore` line if you chose the in-tree path). **Do NOT** register the runner to any other repo. **Security:** a self-hosted runner executes whatever the workflow runs — fine for this private repo with trusted authors; do not later wire it to run untrusted external PRs. If WSL2/Docker isn't available on the machine, **BLOCKER-to-PM** with the specifics rather than forcing a Windows-native runner.

## 4. Handback
On your RETURN (runner Idle + labels + Docker-ok), the PM relays the staged `Plans/DISPATCH-2026-06-14-ci-self-hosted-jarvis.md` to Jarvis, who flips `ci.yml` to `runs-on: self-hosted` + the cheap/expensive split. First green run on your runner = CI is a live, free gate again.

---

## Relay — what to tell Prometheus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-14-self-hosted-runner-setup-prometheus.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-14-self-hosted-runner-setup-prometheus.md
  ```
- **One-line:** Stand up a GitHub Actions **self-hosted runner** for `SecondStageTurbine/NeoPlutarch`, **in WSL2 Ubuntu** (NOT Windows-native — CI is Linux+Docker), located **under `E:\AI Automation\AltimitNeo`** but kept out of the git tree (gitignored `actions-runner/`, or a sibling `AltimitNeo-runner`; prefer a WSL-native `--work` dir for speed). Use a **fresh registration token from the PL** (never commit it). Install as a service (`svc.sh`) for persistence; verify `docker buildx version` works in the runner shell. Done = runner **Idle** in Settings → Actions → Runners. Report: Idle + labels + Docker-ok + the location/.gitignore/`--work` choices. Don't touch `ci.yml` (Jarvis's job, after you report Idle). BLOCKER-to-PM if WSL2/Docker isn't available.
