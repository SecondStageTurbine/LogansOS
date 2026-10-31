<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: CI hermeticity / de-flake — kill the contention-timeout flakes → Jarvis

**Date:** 2026-07-03
**From:** PM (Claude)
**To:** Jarvis (main-tier — just landed OT-9). New branch off `origin/main` HEAD **once the OT-8/OT-9/A3 batch lands (`08fcb95`)** — confirm `origin/main == 08fcb95` before branching: `codex/ci-hermeticity-deflake-jarvis`.
**Status:** DISPATCHED — pay down the **operational debt that's been taxing every merge.** The 4-shard QEMU gate has flaked repeatedly on **contention timeouts** (`arthur_memory_encrypted` timed out on the 1.5B-boot run and again reappears; the F3 shard-2 exit-code saga). The CLAUDE.md hermeticity invariant is explicit: *"a green gate must mean the code is correct, never that the runner cooperated."* Every flake costs a re-run or a bounce — this build makes the gate **deterministic**. **Non-conflicting:** CI/test-infra surface, no kernel-feature/WiFi/agentic/fs overlap. **Mostly `tools/xtask` + `.github/workflows` + scenario timeouts — likely NOT a QEMU-gated code change, but if it touches kernel/test-harness it re-gates.**

## 0. What this build is
Make the 4-shard QEMU gate flake-free: find the scenarios that time out under runner contention (the heavy ones — `arthur_memory_encrypted` and siblings), and either **budget them correctly** (right per-scenario/per-shard timeout), **rebalance the shard partition** so a heavy scenario doesn't stack with others on one shard, or **optimize/split the offender**. The bar: the gate's result reflects code correctness, not runner luck.

## 1. References
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md   # OT-35 (artifact-quota self-heal — the misleading error) + the hermeticity note
git show origin/main:.github/workflows/ci.yml   # the 4-shard matrix, timeout-minutes (70), the shard-coverage gate, the `-cpu max` hermeticity treatment
git show origin/main:tools/xtask/src/main.rs   # cargo neo-test --shard i/4 (round-robin partition) + the exit-code aggregation (the F3 bug lived here)
# the flake evidence: QEMU run 28602756737 attempt-1 (arthur_memory_encrypted timeout, passed on attempt-2)
```

## 2. ISCs
- **ISC-1 — Characterize the flakes.** Identify which scenarios time out under contention (start with `arthur_memory_encrypted`), and *why* — genuinely slow (heavy crypto/model), or a runner-contention artifact (a heavy scenario sharing a shard with other heavy ones). Data, not guess: per-scenario wall-clock from recent runs. Report the offenders.
- **ISC-2 — Fix the determinism.** Per the finding: (a) **partition-balance** — spread heavy scenarios across shards so no shard stacks them (the round-robin may cluster them by index); and/or (b) **per-scenario timeout budget** so a legitimately-slow scenario gets its time without the whole shard hitting the 70-min job cap; and/or (c) **optimize/split** a scenario that's slow for a fixable reason. Prefer partition-balance + budgeting over disabling anything — **never drop a scenario's coverage to "fix" a flake** (that's gaming the gate).
- **ISC-3 — The exit-code / aggregation audit.** Re-verify the F3-class bug stays fixed: a scenario that PASSES must never leak a non-zero shard exit (the `xtask neo-test` exit aggregation). Add/confirm a guard so an all-scenarios-pass shard is green, deterministically.
- **ISC-4 — Hermeticity sweep.** Confirm no gate scenario depends on a host-variable input (the RDRAND-under-TCG / live-network class already fixed) — a quick audit that the flakes are *timing*, not *hidden host-dependence*. Note any found.

## 3. Standing-rule sections
- **Kill-switch / invariant:** N/A new — this is CI/test-infra tuning; **no kernel behavior change, no invariant, no scenario coverage removed.** The shard-coverage gate (partition complete + disjoint) must stay green with any partition change.
- **Do-not-game-the-gate:** the whole point is a *more* honest gate — do NOT fix a flake by deleting/skipping a scenario, loosening a real assertion, or hiding a genuine slowness behind an unbounded timeout. If a scenario is genuinely too heavy for the in-gate budget, *reclassify it out with a stated reason* (the `mt7601u_scan_baremetal` precedent), logged — never silently dropped.
- **Scope guard:** CI hermeticity/timeout/partition + the exit-aggregation audit ONLY. **Do NOT:** touch kernel features, WiFi, the agentic surface, `mm/slab`, or fs; change what a scenario *asserts* (only when/where it runs); or remove coverage.

## 4. Gate + close
- **If the change is `.github/workflows` + `xtask` partition/timeout only** (no kernel/test-harness code): the proof is a **green 4-shard QEMU run that's green on re-run** (determinism — ideally show 2 clean runs, no retry) + push-CI. **If it touches kernel/test-harness**, the normal 4-shard QEMU + AAG.
- **Close-gate `scripts/worker-close-gate.sh`** (rebase; re-run; host set; SPDX; self-stat).
- **RETURN** with the flake characterization (offenders + wall-clock data), the fix (partition/budget/optimize), the green run URL(s) — **and, if you can, a re-run showing the previously-flaky scenario now deterministic** — plus the "no coverage removed / no assertion loosened" statement. **Disk reclaim** after merge.

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-03-ci-hermeticity-deflake-jarvis.md` (+ `.github/workflows/ci.yml`, the `xtask neo-test --shard` partition + exit aggregation, OT-35). **Branch:** `codex/ci-hermeticity-deflake-jarvis` off `origin/main` **after the batch lands (`08fcb95`)**.
- **One-line:** **Make the 4-shard QEMU gate flake-free.** The gate keeps timing out under contention (`arthur_memory_encrypted` and friends) — characterize the offenders with per-scenario wall-clock data (ISC-1), then fix determinism via **shard-partition balance + per-scenario timeout budget** (and/or optimize a fixably-slow scenario) (ISC-2), re-verify the F3-class exit-aggregation stays fixed (ISC-3), and a quick host-dependence sweep (ISC-4). **Never fix a flake by dropping coverage or loosening an assertion** — if a scenario is genuinely too heavy for the in-gate budget, reclassify it out with a stated reason, logged (the `mt7601u_scan_baremetal` precedent). **Proof:** a 4-shard QEMU that's **green on re-run** (determinism), ideally two clean runs. **CI/infra surface only — no kernel/WiFi/agentic/fs/mm touch, no coverage removed.** Close-gate; RETURN the flake characterization + the fix + the green(-on-rerun) URL(s) + the "no coverage removed" statement.
