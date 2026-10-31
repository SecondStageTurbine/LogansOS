<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CI storage reclaim — stop using GitHub-metered storage on the self-hosted runner → Prometheus

**Date:** 2026-06-15
**From:** PM (Claude)
**To:** Prometheus (new branch off `origin/main` HEAD: `prometheus/ci-storage-reclaim`) — you **stood up the self-hosted runner**, so you already know its filesystem layout (`/opt/actions-runner`, the work dir) — ideal for placing the local cache. Light-coding/ops lane.
**Status:** DISPATCHED. **GitHub Actions *storage* is at 90%.** The self-hosted migration saved *minutes* but left the *storage* leak: even on your runner, `actions/cache` uploads the multi-GB cargo/target caches to **GitHub's cache backend**, and the `neo-test-serial-logs` artifacts accumulate (14-day retention). Close it — same principle as the minutes fix: **now that CI runs on your hardware, nothing uses GitHub's metered resources; caches + logs live on the runner's local disk.** At 100% it blocks uploads, so reclaim before then.

## Part A — Reclaim now (get back under quota)
1. **Delete the existing Actions caches:** `gh cache list` then `gh cache delete --all` (or per-key) for `SecondStageTurbine/NeoPlutarch`. These are the stale multi-GB cargo/registry/target/docker-layer caches from all the CI runs — the bulk of the 90%.
2. **Delete old artifacts:** the `neo-test-serial-logs` uploads (and any others) via `gh api -X DELETE /repos/SecondStageTurbine/NeoPlutarch/actions/artifacts/<id>` (list with `gh api .../actions/artifacts`). 
3. Confirm the storage gauge drops well under quota.

## Part B — `ci.yml`: go local-disk, stop the growth
> **⚠ C: DRIVE CONSTRAINT (PL, 2026-06-15):** the PL's **C: drive is filling from the WSL2 + Docker vhdx** (this runner + the dev container + build caches all default to `C:\…\AppData`). So the local cache **must NOT live on the C: WSL vhdx** (e.g. a bare `/opt/...` path is on C: and would make it worse). Land the cache — and ideally the runner/distro itself — on **D:** (the PL's chosen target, 2026-06-15: a SATA SSD, ~1.7 TB free — fast enough for build/Docker I/O, most headroom, off the gaming NVMe). Options, your call on the runner: (a) **relocate the whole WSL2 distro** to a roomy drive (`wsl --shutdown` → `wsl --export <distro> <file>` → **verify the export file exists/size** → `wsl --unregister <distro>` → `wsl --import <distro> <dir-on-roomy-drive> <file>`; re-confirm the runner service after) so `/opt` + cache are natively on that drive; or (b) point the cache dir at a roomy location Docker/WSL can reach fast. **Coordinate timing with the PL** — do the relocation when the runner is idle, not mid-build. The Docker Desktop *disk-image* move (Settings → Resources → Disk image location → a roomy drive) is the PL's GUI step; flag it in the RETURN.

1. **Drop GitHub-backed caching.** Remove the `actions/cache@v4` steps (cargo registry/git/target, the rustup-home, **and the Docker-layer cache**). On a self-hosted runner you don't need GitHub's cache backend — **persist `CARGO_HOME` + the `target` dir (and the buildx cache) on a stable LOCAL path on the runner** (e.g. a fixed `/opt/altimitneo-ci-cache/{cargo,target,buildx}` bind-mounted into the dev container) that survives between runs. Result: caching is free (your disk) and faster (no up/download), and **zero GitHub cache storage**. You judge the exact mechanism on the runner — the goal is *local persistence, no GitHub cache uploads*, while keeping builds fast (a warm local cargo/target).
2. **Drop the serial-log artifact upload.** Remove the `actions/upload-artifact` step (and the "Dump … on failure" can stay as in-log output, which doesn't cost storage). Rationale: failures are diagnosable from the **job logs** (which the PM reads directly via the API — that's what I've used) + on the runner itself; the uploaded artifact is redundant storage. *(If you find a reason to keep it, use 2–3 day retention, not 14 — but default to dropping it.)*
3. **Keep the gate intact:** same fmt/clippy/test/SPDX cheap gate every push, same 4 QEMU shards on push-to-main/nightly/`workflow_dispatch`, same coverage. This is storage hygiene, not a gate change.

## Gate
- Part A done: Actions storage well under quota.
- Part B: a green run on the runner with **no `actions/cache` and no artifact upload**, the local cargo/target cache warm (build not dramatically slower than before), all gates intact. That run *is* the test (zero hosted minutes, zero hosted storage).
- `actionlint` + `git diff --check` clean.
- **⚠ PUSH the branch (`git push -u origin prometheus/ci-storage-reclaim`) before reporting done** — end the lane with the push (standing habit; don't make the PM chase a local-only branch).

## Scope guard
CI-infra only (`ci.yml` + the reclaim). **Do NOT** weaken any gate (same fmt/clippy/test/QEMU coverage); **do NOT** touch kernel/AI code; **do NOT** expose the runner to untrusted PRs. If local-disk persistence needs a runner-side setup step (a fixed cache dir / bind mount) beyond `ci.yml`, do it and note it in the RETURN. If `gh` lacks delete perms, BLOCKER-to-PM (the PL can delete via the UI).

## Merge (PM)
On the green no-cache-upload/no-artifact run: PM `pm-merge-check` → `--ff-only` → `main`; this closes the **last GitHub-metered-resource leak** — CI is then fully free (minutes *and* storage) on the PL's hardware. Update the CLAUDE.md CI note if the caching mechanism changes materially.

---

## Relay — what to tell Prometheus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-15-ci-storage-reclaim-prometheus.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-15-ci-storage-reclaim-prometheus.md
  ```
- **Branch:** `prometheus/ci-storage-reclaim` off `origin/main` HEAD.
- **One-line:** Actions **storage** at 90% — close the leak. **(A) Reclaim:** `gh cache delete --all` + delete the `neo-test-serial-logs` artifacts (API) → back under quota. **(B) `ci.yml`:** drop the GitHub-backed `actions/cache` steps (cargo/target/rustup + docker-layer) and **persist cargo/target/buildx on a stable LOCAL path on the runner** (your disk — free + faster, zero GitHub cache storage); **drop the `upload-artifact` serial-log step** (failures are diagnosable from job logs + the runner; default to dropping, ≤3-day retention only if you must keep it). Keep every gate intact (cheap gate per push, QEMU shards on push-to-main/nightly). Gate = a green runner run with **no cache uploads, no artifacts**, local cache warm. `actionlint` clean. Let CI run (no `[skip ci]`). This closes the **last** GitHub-metered-resource leak — CI fully free on your hardware after.
