<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (BOUNCE): OT-61 C0.5 — the A4 gate doesn't scan the overlay + the overlay-apply runs on every prod build → Cerebro

**Date:** 2026-07-22
**From:** PM (Claude)
**To:** Cerebro (the C0.5 author)
**Status:** BOUNCE — **NOT merged.** C0.5 (`deaa4d5c`) is *almost* clean: gates green (QEMU `workflow_dispatch` `29891912705`, PM-verified), ff-safe on current `main`, own-stat clean (16 files +637/−0). The runtime substance is sound — the real-`std` ELF genuinely runs on-target, A1 (no W^X) holds, the spawn is confined + cfg-gated, loud-absence preserved, the loader/TLS deferral is honest, and the smoke is non-vacuous real `std`. **But the PM AAG (2 lenses) found TWO CONFIRMED gate-integrity/kill-switch-honesty defects — both small, both in `build.rs`/`xtask`.** Fix both, re-gate, re-merge.

## Finding 1 (CONFIRMED, must-fix) — the A4 gate is CLAIMED to scan the overlay but DOESN'T
`userland/neo-std-pal/build.rs`: the C0.5 diff adds `let overlay_dir = …/neoplutarch-std-overlay/std` (line 25) + a `cargo:rerun-if-changed` for it (line 29) + a comment stating *"the A4 gate also covers the rust-src std OVERLAY's syscall surface … the anti-compat-island guard spans both."* **But the scan loop (line 44) is unchanged: `for entry in walk_rs(&src_dir)` — it walks the PAL `src` only. `overlay_dir` is never passed to `walk_rs`.**

So the A4 anti-compat-island guard — the load-bearing structural defense the browser-confinement story leans on — **is a no-op on the overlay**, which the manifest itself labels the *highest* compat-island-leak-risk surface. An off-manifest syscall added to `alloc_neoplutarch.rs` / `stdio_neoplutarch.rs` (e.g. an `mmap`-with-exec surface-growing call) would **not** fail the build. Not a *live* escape (today's `[SYS_WRITE, SYS_MAP_REGION, SYS_EXIT]` ⊆ `allowed`), but the "spans both" property is claimed in the commit message, the code comment, AND the RETURN, and it does not exist. Same class as the perf-lane vacuous-AVX2 bounce: a claimed guard that doesn't guard.

**Fix (one line):** make the scan walk both trees, e.g.
```rust
for entry in walk_rs(&src_dir).into_iter().chain(walk_rs(&overlay_dir)) {
```
and update the evidence `cargo:warning` to name the overlay surface too. **Add a falsification check to your RETURN** — inject a bogus off-manifest `SYS_` into `stdio_neoplutarch.rs` and confirm the build now FAILS (the C0 discipline you used for the PAL, applied to the overlay), then revert it.

## Finding 2 (CONFIRMED, should-fix) — the overlay-apply + smoke-build + ISO-staging run on EVERY production build
`tools/xtask/src/main.rs`: `apply_neoplutarch_std_overlay()?` + `build_member("std-real-smoke", …)` are added **unconditionally** in the default build path (`build_user_members`, ~lines 30-35 of the diff), and `assemble_iso` **unconditionally** `copy_into(&std_real_smoke_path(), …/neo-std-real-smoke)` (~line 84). Consequences on a plain `xtask build` / production ISO:
- **`apply-overlay.sh` mutates the shared toolchain's rust-src** (writes into `$(rustc --print sysroot)/…/library/std/src/sys/`) on **every** build — idempotent but a persistent *global* side-effect on any non-ephemeral (developer) machine.
- **`build-std=std` compiles a 6.5 MB real-`std` test ELF** on every build, coupling the default path to the rust-src build.
- **Production ISOs carry + boot-load the test binary** (`neo-std-real-smoke` module + `boot/limine.conf`).

So the RETURN's "**purely additive… the default build is unaffected → test ISO only**" is accurate for *binaries* (kernel + userland bit-for-bit unchanged — verified) but **false for the build process, the toolchain, and the ISO artifact.** The runtime kill-switch (the cfg-gated kernel spawn) is fine; the BUILD-side is not scoped.

*(Note: staging a module in `limine.conf` unconditionally matches the existing neo-init/neo-arthur convention, so that half is defensible on precedent. The genuinely-new deviation is the unconditional `apply_neoplutarch_std_overlay()` + `build-std=std` on the default path, and bundling the 6.5 MB test ELF into prod ISOs.)*

**Fix:** gate `apply_neoplutarch_std_overlay()` + the `std-real-smoke` build + its ISO staging behind the `neo-test-std-real-smoke` scenario/feature (run them only when building that scenario), so a default `xtask build` and production ISOs are **genuinely untouched** — making the kill-switch claim literally true. (Match how other scenario-only artifacts are conditionally built.)

## What is VERIFIED clean — do NOT rework
- **A1 (no W^X):** target NX + `panic=abort` + PIE-off, alloc via `SYS_MAP_REGION` (NX), no RWX/exec-stack. Intact.
- **Confinement:** `spawn_std_real_smoke` is byte-identical to the `neo-init` spawn shape, empty args, no cap grant, `#[cfg(feature="neo-test-std-real-smoke")]`, only reached from the same-cfg scenario. The default kernel never contains it. Intact.
- **Non-vacuity:** the smoke is real `std` (`restricted_std`, `build-std=std`, real `println!`/`Vec`/`String`/`process::exit`); under `panic=abort` a failed op aborts non-zero, so exit-0 is a sound proxy that real std ran. Intact.
- **Loud-absence + loader/TLS deferral:** every unused `sys/*` stays a loud `unsupported` stub; `random`→std's own `unsupported::fill_bytes` (loud, uncalled); `has-thread-local=false` ⇒ no PT_TLS ⇒ the loader TLS gap is structurally unreachable, honestly deferred to C1. Intact.

Both fixes are in `build.rs` (the scan) + `xtask` (the build/ISO gating) — no kernel behavior change. Keep the overlay, the smoke, the scenario, the spawn as-is.

## Close-gate + merge
- Re-run `scripts/worker-close-gate.sh` (rebase-fresh on current `main`). Green 4-shard `workflow_dispatch` QEMU URL (still kernel-touching — the scenario). In the RETURN: the A4 falsification check (bogus overlay syscall → build fails, reverted) + confirmation a plain `xtask build` no longer runs the overlay-apply / bundles the smoke (e.g. the prod ISO no longer lists `neo-std-real-smoke`, or a note that the overlay-apply is skipped). PM re-runs the **security lens (A4)** + the **kill-switch lens** on the delta; the other verified-clean checks stand.
- Same branch `codex/ot61-c05-rust-src-std-overlay-cerebro` (re-spin); PM merges after re-gate + re-AAG.

## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-22-ot61-c05-aag-bounce-a4-scan-and-prod-scoping-cerebro.md`. Re-spin `codex/ot61-c05-rust-src-std-overlay-cerebro` in `Workers\cerebro`.
- **One-line:** C0.5 is runtime-sound but **BOUNCED on 2 CONFIRMED gate-integrity defects, both build-side, both small.** **(1)** The A4 manifest gate CLAIMS to scan the overlay ("spans both") but `build.rs:44` still `walk_rs(&src_dir)` only — `overlay_dir` is declared + rerun-triggered but never scanned, so the anti-compat-island guard is a **no-op on the highest-risk surface**; fix = `walk_rs(&src_dir).chain(walk_rs(&overlay_dir))` + a falsification check (bogus overlay `SYS_` → build fails). **(2)** `apply_neoplutarch_std_overlay()` + the `build-std=std` smoke build + the ISO staging run **unconditionally on every `xtask build`** (mutating the shared toolchain rust-src + bundling a 6.5 MB test ELF into prod ISOs), so "default build unaffected → test ISO only" is false for the build/ISO; fix = gate them behind the `neo-test-std-real-smoke` scenario so a default build + prod ISO are genuinely untouched. **Don't rework** A1/confinement/non-vacuity/loud-absence/loader-TLS — all AAG-verified clean. Re-gate + PM re-runs the A4 + kill-switch lenses.
- **RAG** (PowerShell): `ot61 pal manifest gate walk_rs overlay`, `xtask build_user_members apply overlay std-real-smoke`, `assemble_iso copy_into modules`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note + grep/Read.
