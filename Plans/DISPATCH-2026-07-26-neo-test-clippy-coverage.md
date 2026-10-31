<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Make `neo-test`-gated clippy visible to CI + clean the backlog → (main-tier, when a slot frees)

**Date:** 2026-07-26
**From:** PM (Claude)
**Status:** DISPATCHED-QUEUED — **kernel-touching hygiene, QEMU-gated. LOW priority — behind the WiFi + browser critical path.** Closes the invisible-clippy-debt pocket Oracle surfaced (`Plans/PM-NOTE-2026-07-26-toolchain-hazards-from-oracle-return.md`, hazard 2).

## The gap
CI's blocking kernel clippy runs `cd kernel && cargo clippy -Zjson-target-spec -- -D warnings` on **default features**. All `#[cfg(feature = "neo-test")]` (and scenario-feature) code is not compiled, so clippy never sees it. **~70 errors have accumulated invisibly** (e.g. `kernel/src/fs/mod.rs:787` `collapsible_if`) — the same rot-class as the pre-`[skip ci]` debt: real lint failures that a green CI hides because the gate structurally can't compile that code.

## Scope — pay down + gate it so it can't rot again
1. **Enumerate the backlog.** Build the kernel clippy under the `neo-test` feature set that compiles the scenario scaffolding (the base `neo-test` umbrella; if scenarios are mutually exclusive, use a representative set / small matrix that covers the shared `#[cfg(feature="neo-test")]` code). Capture the full list (~70).
2. **Fix them all** — they're mechanical (collapsible_if, needless_return, etc.). Behavior-preserving lint fixes only; no logic change. If a fix would change behavior, it's a real bug → note it separately, don't fold it in.
3. **Add the CI pass so it stays visible.** In `.github/workflows/ci.yml`, add a **blocking** kernel clippy step under the `neo-test` feature(s): `cd kernel && cargo clippy -Zjson-target-spec --features <neo-test-umbrella> -- -D warnings` (mirror the existing default-feature step's toolchain/target flags). It runs in the host-checks job (ubuntu-latest now). Pick the feature set that maximizes coverage of the `#[cfg(neo-test)]` surface without a mutually-exclusive-scenario conflict — document which set and why.
4. **Prove it fails-closed:** inject one clippy violation into `#[cfg(neo-test)]` code → the new step REDs → revert. Confirms the pass actually sees the previously-invisible code.

## Standing rules
- Behavior-preserving (lint fixes only); no new invariant; no kill-switch needed (hygiene). Do NOT touch scenario *logic* or discriminators — only the lint form.
- Coordinate with the file-sync-hazard rule: `git diff`-verify each edit landed (many small edits across files is exactly where the sync-corruption tool bites).

## Close-gate + merge
`scripts/worker-close-gate.sh` (rebase-fresh), green push-CI + the new `neo-test` clippy step green, the falsification (injected violation REDs). A 4-shard QEMU gate is only needed if any fix touches gated runtime behavior (lint fixes shouldn't) — otherwise push-CI covers it. Branch `codex/neo-test-clippy-coverage`. PM verify + light review (behavior-neutrality of each fix + the new step actually compiles the cfg(neo-test) code).

## Relay — what to tell the coder
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-26-neo-test-clippy-coverage.md` + `…:Plans/PM-NOTE-2026-07-26-toolchain-hazards-from-oracle-return.md` (hazard 2). Files: `.github/workflows/ci.yml` (the kernel clippy step), the ~70 sites (start `kernel/src/fs/mod.rs:787`), `kernel/Cargo.toml` (the `neo-test` feature umbrella).
- **One-line:** **Make CI lint the `#[cfg(neo-test)]` code it currently can't see, and clean the ~70 backlog.** CI's blocking kernel clippy runs default features, so scenario-gated code is never compiled/linted (~70 hidden errors — pre-`[skip ci]`-class rot). Enumerate them under the `neo-test` umbrella, fix them (mechanical, behavior-preserving lint only — a fix that changes behavior is a real bug, flag it), add a **blocking** `cargo clippy --features <neo-test-umbrella> -Zjson-target-spec -- -D warnings` step to `ci.yml` (mirror the existing step's flags), and prove it fails-closed (inject a violation → RED → revert). Behind WiFi/browser priority. Close-gate + green push-CI + the falsification.
- **RAG** (PowerShell): health-check first; e.g. `cfg neo-test feature kernel Cargo.toml umbrella`, `ci.yml kernel clippy json-target-spec`. If down, note it + grep/Read.
