<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CI optimization — durable QEMU caching + shard the QEMU suite → Prometheus

**Date:** 2026-06-05
**From:** PM/Cerebro
**To:** Prometheus (worker tree, feature branch — e.g. `prometheus/ci-shard-and-qemu-base`)
**Status:** DISPATCHED — CI/infra hygiene, parallel to BUILD-4a (coordination note in §5)
**Context:** `main`'s `neo-test` job was being **cancelled** at `timeout-minutes: 45` for the whole Phase 6 arc (runs #127–#132). Diagnosed from the run data: ~13 min building the dev container (source-built QEMU 10.2.3) + ~30 min of tests > 45. Immediate band-aid already landed (`86d1aa6`, timeout → 90). This dispatch is the real fix.

---

## 0. The two problems (and a subtlety the run data revealed)

1. **The QEMU-10.2.3 build runs ~13 min on every CI run** — the buildx cache isn't reusing it. **Subtlety:** a cancelled job *skips the cache-save post-step* (confirmed in #132's job steps: "Post Cache Docker layers" = skipped). So every timeout cancelled the save → next run rebuilt QEMU → closer to the wall → cancelled again. A vicious cycle. **The 90-min bump may have already broken it** (the first run that *completes* will finally save the cache). Step 1 of this task is to **confirm whether the cache now self-heals**, and only do heavier work if it doesn't.
2. **The 189-scenario suite runs serially in one job (~30–40 min).** This is the durable cost and needs **sharding** regardless of the cache.

## 1. Objective

Get `main`'s CI **fast and reliably green**: container setup cheap (QEMU not rebuilt every run), and the QEMU suite **sharded across parallel jobs** so wall-clock drops from ~50 min to ~15, then bring the timeout back down from the 90-min band-aid.

## 2. Implementation Success Criteria

- **ISC-1 — Confirm/fix the QEMU build caching.**
  - First, verify empirically whether the buildx cache now persists (since runs complete under the 90-min budget): a doc-only or `ci.yml`-only commit should reuse the cached image and **not** spend ~13 min rebuilding QEMU.
  - If it still rebuilds, make the QEMU build a **prebuilt base image** the dev `Dockerfile` `FROM`s — built once and rebuilt **only when the QEMU version / base inputs change** (e.g. a separate `Dockerfile.qemu-base` published to GHCR via its own `workflow_dispatch`/path-triggered workflow, pinned by tag+digest; auth via `GITHUB_TOKEN`). The dev image then pulls the base (fast) instead of compiling QEMU.
  - Net requirement: **a kernel/doc commit must not rebuild QEMU from source.**
- **ISC-2 — `xtask neo-test --shard i/N`.** Add a deterministic shard partition to the test runner: `cargo xtask neo-test --shard i/N` runs scenario subset `i` of `N`. **Correctness is the whole point** — a unit test must assert that for the live scenario list, `⋃ shards == the full set` (nothing dropped) and `shards are pairwise disjoint` (nothing double-run). Stable ordering so the partition is reproducible.
- **ISC-3 — CI matrix.** Turn the `neo-test` job into a `strategy: matrix: shard: [1,2,3,4]` (pick N from the measured per-scenario time — target ~10–15 min/shard); each shard builds (cheap, post-ISC-1) and runs `--shard ${{ matrix.shard }}/N` at `NEO_SMP=4`. The job is green only if **all** shards pass (`fail-fast: false` so one shard's failure doesn't mask others).
- **ISC-4 — Coverage proof (no silent gaps).** A CI step (or the runner itself) emits each shard's scenario list + count, and a check confirms the shards' union equals the full registered scenario set **exactly once each** — so sharding can never produce a false green by dropping scenarios. This is the sharding analog of the byte-oracle discipline: prove the partition is complete, don't assume it.
- **ISC-5 — Bring the timeout back down.** With cheap setup + sharding, lower the per-shard `timeout-minutes` from the 90 band-aid to a tight value with margin over the measured worst shard (e.g. 30).
- **ISC-6 — Caches intact.** The cargo/target cache still works per shard (shared restore-key is fine); no regression in the `Free disk space` / disk-pressure handling that the job already does.
- **ISC-7 — Standards.** SPDX on any new Dockerfile/workflow files; document the base-image rebuild trigger; no secrets beyond `GITHUB_TOKEN`.

## 3. Verification reality (read — CI changes are awkward to test)

The workflow only triggers on `push: branches: [main]` and `pull_request`. So a push to your feature branch **won't** run CI. Verify via:
- **`xtask --shard` correctness** — fully testable locally/host (the partition unit test, ISC-2/4). Do this thoroughly; it's the part that can silently break coverage.
- **The CI matrix itself** — use **`workflow_dispatch`** on your branch ref to exercise the workflow without a PR (the workflow already has `on: workflow_dispatch`). Do **not** open a PR (standing instruction) unless I ask.
- **Final proof is the post-merge `main` run** — like the QEMU bump, the real confirmation is the first green matrix run on `main` after I merge. Report wall-clock before/after.

## 4. Close gate

- `git fetch origin && git rebase origin/main`.
- `xtask --shard` partition unit test green (complete + disjoint coverage); host-checks green (fmt/clippy/test).
- A `workflow_dispatch` run on your branch shows the matrix green, faster, and the coverage check passing (paste the per-shard counts summing to the full total).
- Single squashed commit; push to your **feature branch, not `main`** — PM merges, then we watch the first `main` matrix run together. Close report: the cache outcome (self-healed vs base-image), the shard count + per-shard timings, the coverage proof, and the new total wall-clock.

## 5. Scope guard + coordination

CI/build infra + the `xtask --shard` flag **only** — no kernel behavior changes, no scenario additions/removals (sharding must run the *exact* current set). **Coordination:** this touches `Dockerfile`, `ci.yml`, and `tools/xtask` — and **BUILD-4a (Jarvis) may also touch `Dockerfile` (EU-kernel toolchain) and `xtask` (its bare-metal scenario).** To avoid a collision, **this CI task should land first** (it's smaller and speeds everything); BUILD-4a then rebases onto the faster CI. If you find yourself editing the same `xtask` scenario-table region BUILD-4a needs, flag PM to sequence the merges.
