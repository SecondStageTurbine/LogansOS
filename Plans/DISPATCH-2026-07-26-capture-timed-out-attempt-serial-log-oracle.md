<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Capture the serial log of a TIMED-OUT attempt (even when a later attempt passes) → Oracle

**Date:** 2026-07-26
**From:** PM (Claude)
**To:** Oracle (offered to start immediately; owns the finding)
**Status:** DISPATCHED — **host/xtask-touching, gate-verified. The prerequisite to hunting the arthur hang.** Oracle's deflake return proved `arthur_reversibility` is a **live intermittent kernel hang** (bimodal ~8s-or-never; the 600s timeout lever is disproven), which the built-in retry masks. **The reason it has survived: a hang that passes on retry destroys its own evidence** — artifacts upload only on job *failure*, so the attempt-1 serial tail (showing exactly where the guest stopped) is written on the runner and discarded when a later attempt passes. Fix that first.

## Scope — make the timed-out attempt's evidence survive
1. **Preserve the serial log of any timed-out attempt**, even when a subsequent retry attempt of the same scenario PASSES (so the job succeeds). Today `neo-test`'s per-attempt serial is written on the runner but only *uploaded* on job failure; a retry-recovered job succeeds → the hang's serial tail is discarded. Make xtask **retain + surface** the timed-out attempt's serial (name it distinctly, e.g. `<scenario>.attempt<N>.timeout.serial.log`) and have the CI job **upload it as an artifact regardless of final job conclusion** (a hung attempt is worth capturing even on a green run).
2. **Small + self-contained.** This is xtask + the CI artifact-upload step — no kernel change. Do not alter the retry logic or the scenario; only *retain and upload* what the timed-out attempt already produced.
3. **The evidence must show where it hung** — ensure the captured serial is the *tail* at timeout (the last breadcrumbs before the guest stopped: the `agentic_model_ast_kind=write_file` / `ipc_freq … syscalls=` region Oracle's return identified), so the next occurrence is diagnosable.

## Why this is the right first step (not the hang itself)
The NeoFS-writeback-vs-arthur-I/O race (the actual hang) is a genuine intermittent kernel bug on a merge-blocking scenario — but hunting it blind is expensive; every occurrence currently erases the one artifact needed. This lane converts the **next** hang into a diagnosable event. It unblocks the hunt; it is not the hunt.

## Standing rules
- Behavior-preserving (retain/upload only; no retry-logic or scenario change). No new invariant. The artifact-upload-on-success addition must not break the existing on-failure upload or the artifact-quota handling (the quota-exhaustion incident: keep the upload `continue-on-error` so a full quota can't redden a passing job).
- **Promotion note:** this IS the escaped-defect promotion for the hang class — instead of the retry silently erasing evidence, the gate now preserves it.

## Close-gate + merge
`scripts/worker-close-gate.sh` (rebase-fresh on `origin/main`), green push-CI, and a **demonstration**: a run where an attempt times out and a later attempt passes → the timed-out serial log **is** in the artifacts (green-run capture proven). A 4-shard `workflow_dispatch` is only needed if a gated path changes (it shouldn't — xtask + CI yaml). Branch `codex/capture-timeout-serial-oracle`. Disk-reclaim after merge.

## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-26-capture-timed-out-attempt-serial-log-oracle.md`. Files: `tools/xtask/src/main.rs` (the neo-test attempt/serial handling), `.github/workflows/ci.yml` (the "Upload neo-test serial logs on failure" step). This is your rec #1 — greenlit; go.
- **One-line:** **Make a timed-out attempt's serial log survive even when a later retry attempt passes** — the arthur hang keeps erasing its own evidence because artifacts upload only on job failure, and a retry-recovered job succeeds. Retain the timed-out attempt's serial tail (where it hung) + upload it as an artifact **regardless of final job conclusion** (keep `continue-on-error` so a full quota can't red a passing job). xtask + CI yaml only, no kernel/retry/scenario change. Prove it: a run that times out an attempt then passes → the timeout serial log IS in the artifacts. This unblocks the actual hang-hunt (the NeoFS-writeback-vs-arthur-I/O race). Close-gate + green push-CI + the capture demonstration.
- **RAG** (PowerShell): health-check first; e.g. `neo-test attempt serial log upload artifact xtask`, `ci.yml upload neo-test serial logs on failure`, `TEST_SCENARIO_ATTEMPTS retry`. If down, note it + grep/Read.
