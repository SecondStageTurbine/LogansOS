<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CI consolidation — close the QEMU-coverage gap + harden the 3-runner setup → Jarvis

**Date:** 2026-06-16
**From:** PM (Claude)
**To:** Jarvis (he diagnosed the shard-3 coverage gap — best placed to build the guard). New branch off `origin/main` HEAD: `codex/ci-consolidation-prequel-qemu-gate-jarvis`.
**Status:** DISPATCHED. **CI-infra (`.github/workflows/ci.yml` + runbook) — let CI run.** A Rule-3-style consolidation of today's CI churn (per-runner cache, non-root runners, 3-runner scale) **plus the load-bearing fix**: make kernel-touching branches QEMU-validated *before* merge, so today's regression class can't recur.

## 0. The decision this build makes
Today's shard-3 regression slipped onto `main` because **branch CI gates QEMU off — a kernel/scenario branch first exercises QEMU only *after* it lands.** Close that gap with a pre-merge QEMU path, and consolidate the rest of today's CI changes into one clean, documented state.

## 1. ISCs
- **ISC-1 — Pre-merge QEMU for kernel-touching branches (the gap-closer).** Add a mechanism so a branch that changes `kernel/`, `tools/xtask/`, scenario definitions, or `Cargo.*` gets the full QEMU shard suite run **before** the PM merges. Pick the lightest sound option and justify it:
  - **(a) documented `workflow_dispatch`-on-branch close-gate** — the coder/PM triggers a QEMU run on the branch (the `neo-test` job already runs on `workflow_dispatch`); the dispatch close-gate requires a green branch-QEMU run before declaring close. Cheap, explicit. **Recommended default.**
  - **(b) auto-trigger** — extend the `neo-test` `if:` to also run on `push` to `codex/**` branches whose diff touches `kernel/`/scenarios. Automatic but burns QEMU on every such push (×3-runner cost).
  Recommend (a) as the standing close-gate + leave (b) as an optional opt-in (e.g. a `[qemu]` commit-tag or a label). State your choice + why.
- **ISC-2 — Consolidate today's CI state.** Verify + tidy into one coherent `ci.yml`: per-runner buildx cache (`/tmp/.buildx-cache-${{ runner.name }}`), the hardware-only-scenario exclusion + its regression test (your shard-3 fix — confirm it's the canonical guard), the concurrency group. Confirm no stray shared-`/tmp` paths remain. No behavior change beyond ISC-1.
- **ISC-3 — Runbook refresh.** Update `Plans/RUNBOOK-second-self-hosted-runner.md` to the *as-built* reality: 3 runners (`altimitneo-wsl`/`gaming-pc-2`/`gaming-pc-3`), non-root services, per-runner cache isolation, the stop-while-gaming toggle, and the **new pre-merge-QEMU close-gate step**.
- **ISC-4 — Standards.** No kernel/userland code touched; no new `unsafe`/invariant; `ci.yml` stays self-hosted-only (we have **0 GitHub-hosted minutes** — do NOT reintroduce `ubuntu-latest`). Pin any action by raw-sourced SHA/version.

## 2. Gate
- The pre-merge-QEMU path **demonstrated**: trigger it on this very branch (which touches `tools/xtask`? no — `ci.yml` only; so demonstrate via a `workflow_dispatch` QEMU run on the branch going green), and document the exact command/step.
- `ci.yml` consistent (per-runner cache, concurrency, hardware-only exclusion intact); host-checks green; the check *set* unchanged except ISC-1. Runbook matches as-built.

## 3. Scope guard
`ci.yml` + the runbook + (if needed) the dispatch-template close-gate text **only**. **Do NOT** touch kernel/userland/`ai-ops-tests` code (Janus's 8.3-integration is live there), reintroduce `ubuntu-latest`, or change the QEMU suite's coverage. No new `unsafe`/invariant.

## 4. Merge (PM)
Rebase onto current `main`; push (CI runs); demonstrate the pre-merge-QEMU path; PM `pm-merge-check` → `--ff-only` → ledger (audit-log: the coverage-gap guard shipped). This makes "a kernel branch can't reach `main` QEMU-unvalidated" a mechanical guarantee, not a discipline note.

---
## Relay — what to tell Jarvis
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-16-ci-consolidation-qemu-gate-jarvis.md` (`git show origin/main:<path>`).
- **Branch:** `codex/ci-consolidation-prequel-qemu-gate-jarvis` off `origin/main`.
- **One-line:** Close the gap that bit us today — **kernel-touching branches must get a QEMU run before merge** (recommend a documented `workflow_dispatch`-on-branch close-gate; auto-trigger optional). Plus consolidate today's CI into one clean `ci.yml` (per-runner cache, concurrency, your hardware-only-scenario exclusion + regression test) and refresh the runbook to the as-built 3-runner/non-root/per-runner-cache state with the new pre-merge-QEMU step. **Self-hosted only — no `ubuntu-latest` (0 hosted minutes).** No kernel code (Janus is in `ai-ops-tests`). Demonstrate the pre-merge-QEMU path green on your branch.
