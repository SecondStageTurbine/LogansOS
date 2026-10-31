<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-44 H1 — the SDK-v0 design THINK (what we honestly offer a programmer today) → Prometheus

**Date:** 2026-07-05
**From:** PM (Claude)
**To:** Prometheus (research / design-THINK / host-only lane). **Branch:** `prometheus/ot44-sdk-v0-think` off `origin/main` HEAD.
**Status:** DISPATCHED — the near-term leg of the newly banked **OT-44 developer-platform direction** (PL, 2026-07-05). Horizon 1's claim is that the cross-development SDK *mostly already exists* because we use it daily; this THINK turns that claim into an inventory, an honest promise, and a costed formalization plan. **Scope: a design THINK, host-only — NO code, no ABI change, no QEMU. Output: `docs/THINK-2026-07-05-ot44-sdk-v0.md`.**

## What the THINK must establish (D1–D6) — from REAL repo state, not memory
- **D1 — The SDK-v0 inventory.** Enumerate exactly what a third-party programmer needs from today's repo to build a LogansOS userland program on a dev PC: the target spec JSON, `libneo` (which modules are app-facing vs internal), the toolchain pin (`rust-toolchain.toml`), the xtask subset that builds/packages/runs a userland binary under QEMU, and the minimal docs. Distinguish "exists and works" / "exists but assumes our repo layout" / "missing."
- **D2 — The ABI-stability promise we can honestly make at Phase 8.** Classify the syscall surface (numbers, arg structs, error codes — from `libneo` + the kernel syscall table): which parts have been stable in practice, which just changed (`KernelObservabilityV1` v1→v2 is the live case study — what did that bump require on both sides?), and what versioning/deprecation policy fits (the `.neom`-style version-gate precedent). The deliverable is a truthful tier list ("stable / stabilizing / experimental — expect breakage"), NOT a premature stability guarantee.
- **D3 — What IS a third-party app, mechanically, today?** Trace from source how `neo-arthur`/`neo-init` become running processes: binary format, how binaries get onto NeoFS, the spawn path, what caps a spawned process starts with, and what an app that is NOT baked into our image would need (the gap list). This is the load-bearing section — H1 is real only if a binary we didn't build into the image can actually be installed and run.
- **D4 — The capability-manifest sketch (H2 preview, design-only).** One page: what an app's declared-caps manifest could look like, how it maps onto the existing grant machinery (OT-18 authority UX, `derive_handle`/`ipc::grant`), and why it waits for the OT-22 multi-principal pillar. Name it; do not design it in depth.
- **D5 — The interpreted-substrate question.** Prior-art scan (real sources): WASM runtimes in `no_std` Rust contexts (wasmi/wasmtime-class options and their footprints), and the honest fit assessment — does a sandboxed bytecode runtime *add* anything our capability model doesn't already give a native binary, or is its value the no-native-compiler-on-box distribution story? Recommend adopt-later/defer/reject with the reasoning; this decides whether H3's early-app-substrate idea survives.
- **D6 — The formalization plan + verdict.** ISC-decompose "SDK v0" as a build (docs + template app + versioned ABI doc + whatever D3's gap list demands), size it, and state what we explicitly do NOT promise (on-box toolchain, POSIX personality — pointer to the OT-22 pillar). Surface genuine forks as PL ratification points.

## Scope guard
THINK only. No syscall/ABI change, no packaging code, no new runtime. If D3 reveals the install-a-foreign-binary path is missing entirely, that's a *finding with a build sketch*, not something to prototype here.

## Gate + close
- **Docs-only ⇒ no CI run will exist** (paths-ignored) — correct and expected; run `scripts/worker-close-gate.sh` host set locally and state "docs-only, no CI triggered" in the RETURN. If you touch anything outside docs/Plans, the green push-CI URL becomes mandatory.
- **RETURN-accuracy bar (two strikes on this axis):** before sending, diff the RETURN's claims against the committed artifact — line count, section list, and that every numeric claim appears in the doc verbatim. The RETURN describes the commit, not the intent.
- RETURN: THINK path + the D1 exists/assumes/missing table + the D2 tier list + D3's gap list + the D5 recommendation + ratification points.
- **Disk reclaim** after merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-05-ot44-sdk-v0-think-prometheus.md` (+ the OT-44 row in OPEN-THREADS). **Branch:** `prometheus/ot44-sdk-v0-think` off `origin/main` HEAD.
- **One-line:** **Scope SDK v0 — what we can honestly offer a LogansOS programmer today, from real repo state.** Inventory the cross-SDK (target JSON, app-facing `libneo`, toolchain pin, the xtask build/run subset; exists / assumes-our-repo / missing) (D1); the truthful Phase-8 ABI tier list (stable/stabilizing/experimental) using the ObservabilityV1 v2 bump as the case study, with a versioning policy (D2); **the load-bearing trace: how does a binary we did NOT bake into the image get installed and run — binary format, NeoFS placement, spawn path, starting caps, and the gap list** (D3); a one-page capability-manifest sketch deferred to OT-22 (D4); the WASM/interpreted-substrate prior-art scan + adopt-later/defer/reject recommendation (D5); and the ISC-decomposed SDK-v0 build plan + explicit non-promises (D6). **THINK only, no code. Docs-only ⇒ no CI run exists — say so in the RETURN, close-gate host set locally. And diff your RETURN against the committed artifact before sending — third time on the accuracy axis bounces on arrival.**
