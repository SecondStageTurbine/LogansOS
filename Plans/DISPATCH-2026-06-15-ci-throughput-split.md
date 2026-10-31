# DISPATCH: CI throughput — concurrency hygiene + a second self-hosted runner → Prometheus

**Date:** 2026-06-15 (REVISED v2 — drops the `ubuntu-latest` lever; see §0.2)
**From:** PM (Claude)
**To:** Prometheus (CI/infra lane). Continue on `prometheus/ci-throughput-split` (already pushed) —
**revert the `ubuntu-latest` change** per §0.2. Or fold into `prometheus/ci-storage-reclaim` (same lane).
**Status:** DISPATCHED. **CI-infra change (`.github/workflows/ci.yml` + runner setup) — no `[skip ci]`.**

## 0. The problem
A single serial self-hosted runner cannot serve 4 parallel coders + the reclaim dispatch: observed
0 in-progress / 4 stuck-queued, and the Docker→D: relocation froze it because the runner is on the
same host. There is also **no `concurrency:` block**, so superseded runs never auto-cancel and the
queue fills with dead runs.

## 0.2 CORRECTION — no GitHub-hosted runners (the binding constraint)
**v1 of this dispatch said to move `host-checks`/`spdx-lint` to `ubuntu-latest`. That is WRONG and must
be reverted.** The repo is **private on the free plan with 100% of the monthly Actions minutes already
consumed** — that is the entire reason we migrated to self-hosted. A `ubuntu-latest` job therefore gets
**no runner assigned** (symptom Prometheus correctly diagnosed: `runner_id 0`, empty `steps`, no
execution) and will keep failing until the minutes reset on the 1st. **Every job must stay
`runs-on: self-hosted`.** The throughput fix below uses only levers that need **zero hosted minutes**.

## 0.3 The decision this build makes
Make the single-runner jam stop recurring using only self-hosted-compatible levers: **(1) concurrency
hygiene** so the queue self-drains, and **(2) a second self-hosted runner** so the cheap gate and QEMU
stop serializing behind each other. No GitHub-hosted dependency.

## 1. ISCs
- **ISC-1 — Revert the `ubuntu-latest` change.** `host-checks` and `spdx-lint` go **back to
  `runs-on: self-hosted`** with the **Docker-`compose`-run** invocation exactly as before this branch.
  The check *set* is unchanged from `main` (fmt / `cargo test --workspace --exclude neoplutarch-kernel` /
  `xtask shard-coverage 4` / clippy `-D warnings` / the non-blocking kernel-clippy / SPDX). Net effect on
  `ci.yml` jobs vs `main`: **only ISC-2 (and any reclaim artifact-drop) differ** — no runner retargeting.
- **ISC-2 — Concurrency hygiene (the immediate, infra-free win).** Add a workflow-level
  `concurrency: { group: ci-${{ github.workflow }}-${{ github.ref }}, cancel-in-progress: true }` so a new
  push to a branch cancels only **that branch's** stale run, draining the dead-run pileup. Verify the
  reclaim `workflow_dispatch` and the branch `push` for the **same ref** sharing a group is acceptable
  (a fresh push *should* supersede an in-flight dispatch on that ref) — if you want the reclaim dispatch
  to survive branch pushes, give `workflow_dispatch` a distinct group suffix (e.g. include
  `${{ github.event_name }}`). Pick whichever and note the choice.
- **ISC-3 — Second (and third) self-hosted runner on the one host (the throughput lever; PL infra).**
  Document + (with the PL) register **additional `self-hosted` runner instances on the single CI host**
  so >1 job runs concurrently — the cheap gate no longer waits behind QEMU/reclaim. **There is only one
  eligible machine** (the main PC = the gaming PC = the GPU host); the **OptiPlex is off-limits** (it's the
  bare-metal target, reflashed each run), so **no cross-machine redundancy is possible — accept single-host
  CI.** Sizing for this host (16 logical / ~8 physical cores, 128 GB RAM, Docker on D:):
  - **Target 3 runner instances; start with 2, add the 3rd after measuring a full main-CI run.** ~8 physical
    cores → ~2–3 concurrent heavy jobs (each QEMU shard = `NEO_SMP=4` + a Rust build, ~4–6 cores) before
    oversubscribing. RAM is not the constraint (4 jobs ≈ 32 GB / 128 GB). 4+ runners would thrash on the core
    count — don't.
  - Each instance in its **own directory** (`/opt/actions-runner`, `-runner-2`, `-runner-3`), each with its
    own `config.sh` token, **all sharing the `self-hosted` label** so the matrix fans across them. **Isolated
    work/cache dirs per runner** (the buildx-rotation race the reclaim branch fixed assumes this); they share
    one Docker daemon, which is fine.
  - **Gaming coexistence:** since this is the box the PL games on, include a **stop/start toggle**
    (`sudo ./svc.sh stop` per instance before a session, `start` after) so a 70-min QEMU run can't collide
    with gaming. CI is bursty, so idle runners cost ~nothing.
  Deliverable is the **runbook (per-instance dir/token/label/cache-isolation + the stop-while-gaming toggle)
  + the sizing note**; the PL runs `config.sh`.
- **ISC-4 — QEMU untouched.** `neo-test` (4 shards, QEMU 10.2.3 image, buildx cache, disk probes, 70-min
  timeout, gated to `dispatch/schedule/push-to-main`) stays exactly as-is on `self-hosted`. Keep
  `shard-coverage`'s N in sync with the matrix.

## 2. Gate
- All jobs `self-hosted` (zero `ubuntu-latest`); the check set identical to `main`'s; concurrency cancels
  only same-ref stale runs (demonstrate: push twice quickly, first run goes `cancelled`, second completes).
- With a 2nd runner registered, show two jobs `in_progress` simultaneously (e.g. `host-checks` +
  `spdx-lint`, or two QEMU shards) — the proof of parallelism.
- This run itself **must show real steps executing on a self-hosted runner** (the v1 `runner_id 0`
  failure must be gone).

## 3. Scope guard
`ci.yml` concurrency + the revert + the 2nd-runner runbook **only**. **Do NOT** reintroduce `ubuntu-latest`
or any GitHub-hosted job, change what the checks *are*, alter the QEMU sharding/cache/timeout, or touch
kernel/userland/tools code. Pin any action by raw-sourced SHA/version.

## 4. Close-gate & merge (PM)
Rebase onto current `main` (reconcile with the reclaim `ci.yml` edits — artifact-drop + concurrency + revert
land as one coherent change); push; confirm CI **executes** on self-hosted and goes green. PM
`pm-merge-check` → `--ff-only` → ledger. Merge before the Phase-8 branches so the queue stops piling up.

---

## Relay — what to tell Prometheus
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-15-ci-throughput-split.md` (REVISED v2)
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-15-ci-throughput-split.md
  ```
- **Branch:** continue `prometheus/ci-throughput-split` (or fold into `prometheus/ci-storage-reclaim`).
- **One-line:** **My v1 was wrong — scrap `ubuntu-latest`.** We have **zero GitHub-hosted minutes** (private/
  free, 100% used — that's why we're self-hosted), so hosted jobs get no runner (`runner_id 0`, empty steps —
  your diagnosis is right). **Revert host-checks/spdx-lint back to `self-hosted` + Docker-compose as on
  `main`.** Real fix, minutes-free: **(1)** add a workflow `concurrency` group
  (`ci-…-${{ github.ref }}`, cancel-in-progress) so stale same-branch runs auto-cancel and the queue drains;
  **(2)** stand up a **second self-hosted runner** (same-machine 2nd instance for contention relief, or a 2nd
  machine — *not* the OptiPlex — for redundancy too; PL runs `config.sh`, you write the runbook + label note).
  QEMU matrix untouched. Validate: the run shows **real steps on a self-hosted runner**, a double-push
  cancels the first, and two jobs run concurrently once the 2nd runner's up. Let CI run.
