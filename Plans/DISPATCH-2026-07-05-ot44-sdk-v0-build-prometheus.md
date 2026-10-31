<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-44 SDK-v0 build — SDK-ABI reference + template app + quickstart (the ratified D6 plan) → Prometheus

**Date:** 2026-07-05
**From:** PM (Claude)
**To:** Prometheus (docs / host-cell lane — this is exactly your D6 plan, ratified as recommended). **Branch:** `prometheus/ot44-sdk-v0-build` off `origin/main` HEAD.
**Status:** DISPATCHED — the PL ratified all four SDK-v0 points (2026-07-05; recorded in the THINK's Status line): the ABI tier list, the versioning policy, the WASM deferral, and the **contrib-module model** as H1's honest distribution story. This build ships the D6 deliverables. **Scope: docs + ONE minimal template crate, host-CI only — no kernel, no QEMU, no new syscalls, no xtask scenario.**

## ISCs (your own D6, made binding)
- **ISC-1 — `docs/SDK-ABI.md`.** The ratified tier list (STABLE / STABILIZING / EXPERIMENTAL / BREAKS-FREELY, with the per-tier promise stated), the versioning policy as ratified (leading `version: u32` on every ABI struct; kernel validates size at entry; additions bump the constant; **removals forbidden — successor structs instead**), and the syscall + errno tables generated FROM the source of truth (cite `kernel/src/errno.rs` + the syscall table locations; numbers transcribed, each spot-checked — this document is downstream developers' contract, so a transposed number here is an ADR-028-class poison).
- **ISC-2 — `userland/template-app/`.** A minimal "Hello, LogansOS" crate: static-links `libneo`, builds with the user target spec, per-binary config following the existing pattern — **workspace-compiled** (fmt/clippy/SPDX clean in host CI) but NOT wired into the ISO or any QEMU scenario (it's a template developers copy, not a shipped feature; the guide tells them how to bake it in via the contrib-module flow). Keep it small enough to read in one sitting — it IS the documentation.
- **ISC-3 — `docs/SDK-GUIDE.md`.** The cross-development quickstart: toolchain pin, target spec, building the template, the contrib-module path to getting it into an image and run in QEMU, what's stable vs experimental (pointer to SDK-ABI.md), and the honest boundary — no install-after-boot yet (G1/G2 are the H2 kernel build), no dynamic linking, cross-compile only. Write it for the fresh-reader test: a competent Rust developer with zero repo context follows it end-to-end.
- **ISC-4 — the `NetStatsV1` version-field gap** recorded as a TODO in SDK-ABI.md (the one V1 struct missing its leading version field; fix lands on next code touch of that struct — NOT in this build).

## Scope guard
No kernel/xtask/ci.yml changes; no new syscalls (G1/G2 stay H2); no QEMU scenario. The template crate joining the workspace means **push-CI runs (host checks) — this branch is NOT docs-only**, so the close-gate bar includes the **green push-CI URL cited in the RETURN**. If the template can't compile cleanly without touching shared workspace files beyond the root `Cargo.toml` member list, flag it rather than restructure.

## Gate + close
- **Host CI only** (code = the template crate; no QEMU). Close-gate `scripts/worker-close-gate.sh`; **RETURN cites the green push-CI URL** + diff-stat, and — the standing bar — diff every RETURN claim against the committed artifact before sending.
- **Disk reclaim** after merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-05-ot44-sdk-v0-build-prometheus.md` (+ your own ratified THINK `docs/THINK-2026-07-05-ot44-sdk-v0.md`). **Branch:** `prometheus/ot44-sdk-v0-build` off `origin/main` HEAD.
- **One-line:** **Ship your ratified D6 plan: `docs/SDK-ABI.md` (tier list + versioning policy + syscall/errno tables transcribed from source with per-number spot-checks — a transposed number is developer-facing poison), `userland/template-app/` (minimal Hello-LogansOS crate, workspace-compiled and lint-clean, NOT wired into the ISO/QEMU — the guide teaches the contrib-module bake-in), `docs/SDK-GUIDE.md` (fresh-reader cross-dev quickstart with the honest boundaries: no install-after-boot, static-only, cross-compile only), and the NetStatsV1 TODO note.** No kernel/xtask/CI changes, no new syscalls. **The template crate makes this branch code-bearing: push-CI runs, and the RETURN must cite the green URL.** Close-gate; diff the RETURN against the artifact before sending.
