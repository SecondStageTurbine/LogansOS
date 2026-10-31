<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-69 — feature-gated subsystems are invisible to CI (audit + CI coverage) → Prometheus

**Date:** 2026-08-01
**From:** PM (Claude)
**To:** Prometheus (Qwen — research/audit + bounded HOST-only tooling; **push-CI only, no QEMU gate**)
**Status:** DISPATCHED — squarely in scope: an audit, a CI-config change, and a falsification proof. **No kernel code changes** (see *Explicitly out of scope*).

## The hole (proven, not theorised — found by Oracle, 2026-08-01)
The blocking kernel clippy runs **default-features only**, and whole subsystems sit behind non-default `#[cfg(feature = …)]`. Consequences, both demonstrated:
1. `cargo clippy -Zjson-target-spec --features neo-test-mt7601u-assoc-baremetal -- -D warnings` fails with **27 pre-existing errors** (doc-continuation `mt7601u/mod.rs:242-243`, `manual_range_contains` `:1186`, `result_large_err` `:4944` and `usb/mod.rs:125`) — verified against a pristine tree, none introduced by the WiFi lane.
2. **Worse:** a plain `cargo build` in `kernel/` reports **`Finished`** while a **deliberately planted syntax error** sits in `mt7601u/wedge_logic.rs` — because the driver **is never compiled**. Oracle nearly shipped on three such fake "clean builds."

**So any error in any feature-gated subsystem is currently invisible to CI.** This is the driver-feature analogue of the neo-test-gated clippy debt already paid once (`bbf93a4`).

## Scope
1. **Audit — enumerate the blind surface.** Every non-default feature in `kernel/Cargo.toml` (and `userland/*`) that gates real code: which subsystems are *never compiled* by CI today? Report as a table: feature → what it gates → LoC → currently compiled by CI? (yes/no). This audit is the deliverable even if nothing else lands.
2. **Add CI coverage for the gated surface.** A job that builds/clippies the feature-gated code. **Respect mutually-exclusive features** — a naive `--all-features` will not work here (scenario features conflict), so a **feature matrix** or an explicit list is likely right. Keep it cheap: this is a host/compile job, **not** a QEMU shard.
3. **⚠ Land it NON-BLOCKING first.** Turning it blocking immediately would **redden `main`** on the 27 known errors, which violates the escaped-defect contract. Land it as warn-only (or `continue-on-error`) with its output clearly reported, document exactly what it currently catches, and note that a follow-on lane pays the debt down and flips it blocking.
4. **Falsification proof (the un-gameable part).** Prove the new job actually works: **plant a deliberate syntax error in a feature-gated file, show the OLD build path reports `Finished` and the NEW job catches it, then remove it.** Paste both outputs. A CI job that would not have caught Oracle's planted error is not worth landing.

## Explicitly out of scope (do NOT do these)
- **Do not fix the 27 clippy errors.** They are kernel-touching and route to a main-tier coder in a follow-on lane.
- **Do not flip the job to blocking.** That happens after the debt is paid.
- No kernel logic changes of any kind — this is CI config + an audit report.

## Close-gate + merge
`scripts/worker-close-gate.sh`, **green push-CI, and paste the URL** — *green without a CI URL is not done*. **No 4-shard `workflow_dispatch` needed** (no kernel logic changes; if you find yourself needing one, stop and tell me — it means the scope drifted). Branch: `prometheus/ot69-feature-gated-ci-coverage`. Disk-reclaim after merge.

## Relay — Prometheus
- **WORKTREE:** `E:\AI Automation\workers\prometheus` (yours alone — never another coder's folder, never the repo root)
- **BRANCH:** `prometheus/ot69-feature-gated-ci-coverage` (`git fetch origin && git checkout -B prometheus/ot69-feature-gated-ci-coverage origin/main`)
- **Read:** `git show origin/main:Plans/DISPATCH-2026-08-01-ot69-feature-gated-ci-blindspot-prometheus.md`, and OT-69 in `git show origin/main:docs/OPEN-THREADS.md`.
- **One-line:** **CI cannot see feature-gated code, and we proved it the hard way** — a deliberately planted syntax error in `mt7601u/wedge_logic.rs` survived a `cargo build` reporting **`Finished`**, because the whole driver sits behind a `cfg(feature)` the blocking clippy never enables; 27 real clippy errors are invisible for the same reason. **(1) Audit** every non-default feature that gates real code and report which subsystems CI never compiles. **(2) Add a cheap host job** that builds/clippies the gated surface — respect mutually-exclusive scenario features, so a feature **matrix**, not `--all-features`. **(3) Land it NON-BLOCKING** (warn-only) — making it blocking now would redden `main` on the 27 known errors; a follow-on lane pays those down and flips it. **(4) Prove it works by falsification:** plant a syntax error in a gated file, show the old path says `Finished` while the new job catches it, then remove it — paste both outputs. **Do NOT fix the 27** (kernel-touching, someone else's lane) and **do not flip it blocking**. Close-gate + green push-CI URL; **no QEMU gate needed** — if you think you need one, stop and tell me.
- **RAG** (PowerShell) health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `kernel Cargo.toml feature gates cfg feature driver`, `ci.yml clippy kernel blocking default features`, `neo-test scenario features mutually exclusive`. If down, note it in the RETURN + grep/Read (PL owns RAG startup — never a blocker).
