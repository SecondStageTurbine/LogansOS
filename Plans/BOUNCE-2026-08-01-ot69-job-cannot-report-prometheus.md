<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# BOUNCE: OT-69 `3f756daa` — the coverage job can never report a failure → Prometheus

**Date:** 2026-08-01
**From:** PM (Claude)
**Found by:** **Cerebro**, unprompted, while waiting on his own gate — he had cited `gpu-iommu = success` on his branch as a good signal, then retracted it himself on inspection. That retraction is the finding.
**Status:** **LANDED DEFECT — reopening a lane the PM closed.** `main` is **not red** (nothing is red — that is precisely the problem), so the escaped-defect contract's revert-first arm does not apply. One-token-class fix, forward.

## The defect
`.github/workflows/ci.yml`, the feature-gated clippy step, implements "non-blocking" **twice**:

```yaml
continue-on-error: true          # (1) job stays green if the step fails
run: |
  cd kernel && cargo clippy … -- -D warnings -A dead_code 2>&1 || true   # (2) step can't fail
```

`|| true` makes the shell exit 0, so the **step** conclusion is `success`; `continue-on-error` then makes the **job** `success`. The two together mean **there is no failure signal anywhere in the run** — not the run, not the job, not the step. The only evidence is text inside a collapsed `::group::` in the log.

## Verified, not accepted — `main` @ `3f756daa`, run `30680435339`
| matrix job | step-5 conclusion | actual state of that configuration |
|---|---|---|
| `wifi-usb` | `success` | 27 known pre-existing clippy errors |
| `gpu-iommu` | `success` | **`accel-intel` does not compile at all** (3 hard errors, independently reproduced by Cerebro) |
| `atrium-ai` | `success` | — |
| `misc` | `success` | — |

Four for four green on a tree independently proven broken in two of those four configurations.

## Why this matters beyond cosmetics
**OT-69's close condition is "pay down the debt, then flip the job blocking."** As written you cannot tell whether the debt is paid, because the job reports green either way — **so there is no signal on which to make the flip decision, and no way to notice regression in the meantime.** The job currently costs four runner slots per push and returns nothing machine-readable.

It is the same failure class as DW-1's focus-atomicity assert (a printed string with no counter) and rung-5's `stage=write` literal: **an assertion with nothing behind it.** Third instance today — and the only one that reached `main`.

## PM accountability — two misses, both mine
1. **I verified the wrong property.** I checked the job could not *redden* `main` (no `needs:`, no `if:`, `continue-on-error: true`) and merged on that. That is the right check for *is it safe*; I never asked the inverse — ***can it ever report anything?*** Safe and functional are different properties and I verified only the first.
2. **The falsification proof was run at the wrong layer, and I accepted it.** The dispatch demanded a falsification proof *specifically* to prevent this. Prometheus ran it at the **command** level — planted `pub fn ot69_falsification_probe( {`, old path `Finished`, new path `error: unclosed delimiter`. That proves the **cargo invocation** catches the error. The `|| true` sits *between* the command and the job's report, so the proof passed while the job stayed blind.

**Generalizable rule (worth keeping):** a falsification proof must be run against **the artifact that actually reports** — the job/step conclusion as the API returns it — not against a command nested inside it. Proving the detector fires is not proving the alarm rings.

## Required fix
Remove `|| true`. Note that dropping it alone leaves `set -e` aborting the step at the failing command, so `echo "::endgroup::"` never runs and the log group stays unclosed. This preserves both properties:

```yaml
continue-on-error: true
run: |
  echo "::group::Feature-gated clippy — ${{ matrix.name }} (NON-BLOCKING)"
  status=0
  cd kernel && cargo clippy -Zjson-target-spec \
    --features "${{ matrix.features }}" \
    -- -D warnings -A dead_code || status=$?
  echo "::endgroup::"
  exit $status
```

`|| status=$?` suppresses `set -e`; the explicit `exit` makes the **step** conclusion `failure` (visible ❌ in the UI, greppable via the API) while `continue-on-error: true` keeps the **job** and the **run** green and non-blocking.

## The close condition — falsification at the reporting layer
**Do not re-run the command-level proof; it already passed and proved the wrong thing.** The new proof is:

> Push the fix. On a tree where `accel-intel` is known not to compile — **`main` today is exactly that tree, no planting required** — show the `gpu-iommu` job's **step conclusion is `failure`** while the **job conclusion is `success`** and the **run conclusion is `success`**. Cite the run URL and quote all three conclusions.

If `wifi-usb` also flips to a failing step, that is expected and correct — those are the 27 known errors, and a job that reports them is the entire point. **The run must stay green.** If any run or job goes red, the fix is wrong and `main` blocks.

## Relay — Prometheus
- **WORKTREE:** `E:\AI Automation\workers\prometheus` (yours alone) · **BRANCH:** `prometheus/ot69-report-fix` (`git fetch origin && git checkout -B prometheus/ot69-report-fix origin/main`)
- **Read:** `git fetch origin && git show origin/main:Plans/BOUNCE-2026-08-01-ot69-job-cannot-report-prometheus.md`
- **One-line:** **Your OT-69 job merged, and it can never report a failure — one-token class fix.** The step has **both** `continue-on-error: true` **and** a trailing `|| true`, so the shell exits 0, the step conclusion is `success`, and the job is `success` — no signal at the run, job, or step level; the failure is visible only by opening a collapsed `::group::` in the log. **Proven on `main` @ `3f756daa` (run `30680435339`): all four matrix jobs report `success`, including `gpu-iommu` on a tree where `accel-intel` provably does not compile** (3 hard errors, independently reproduced). Cerebro found it unprompted. **This is not mainly your miss — I verified that the job couldn't redden `main` and merged on that, never asking whether it could report anything at all.** But the part worth internalising is the proof layer: your falsification proof ran at the **command** level (planted syntax error, old path `Finished`, new path `error: unclosed delimiter`) — which proves the *cargo invocation* catches it, while the `|| true` sits between the command and the job's report, so the proof passed and the job stayed blind. **A falsification proof has to run against the thing that actually reports.** Fix: drop `|| true`, but don't just delete it — `set -e` would then abort the step and skip your `::endgroup::`; use `status=0` / `cargo clippy … || status=$?` / `echo "::endgroup::"` / `exit $status`, which makes the **step** fail visibly while `continue-on-error` keeps the **job and run green**. **Close condition — do NOT re-run the command-level proof:** push the fix and show, on `main`'s own broken tree (no planting needed), that the `gpu-iommu` **step conclusion is `failure`** while the **job** and **run** conclusions are both `success`. Quote all three and cite the run URL. `wifi-usb` flipping to a failing step is expected and correct — those are the 27 known errors and reporting them is the whole point. **If any run or job goes red, the fix is wrong.** Why this matters: OT-69's close condition is "flip it blocking once the debt is paid," and right now the job says green either way, so there is no signal to decide on and no way to see regression.
- **RAG** health-check first; e.g. `ci.yml feature gated coverage continue-on-error`, `github actions step conclusion failure job success`. If down, note it and continue with direct file reads.
