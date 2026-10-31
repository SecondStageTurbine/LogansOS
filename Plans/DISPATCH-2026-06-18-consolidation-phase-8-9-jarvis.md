<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-8/9 Rule-4 consolidation — the seven-build digest → Jarvis

**Date:** 2026-06-18
**From:** PM (Claude)
**To:** Jarvis (built the last consolidation `645fd4b` — the precedent; broad context across the ai/ops + Phase-9 lanes). New branch off `origin/main` HEAD: `codex/consolidation-phase-8-9-jarvis`.
**Status:** DISPATCHED. **No-feature consolidation (Rule 4) — overdue.** Seven feature builds since the last digest (`645fd4b`): Arthur Slice 4, Phase 9.1, 9.2, CSPRNG 10.1, the 8b input path, 9.3 AVX2, and `arthur_console`. The cadence is one consolidation per ~3–4 — we're at seven, and the tree is *quiet* (all coders free), which is the condition a consolidation needs. **Behavior-preserving by construction: zero new features, zero new invariants, zero new `unsafe`; the full family passes bit-identically.**

## 0. The decision this build makes
None functional. It pays down the accumulated debt of the last seven builds so the next reader — and the next builder — inherits a clean tree: collapse duplication, prune dead scaffolding, reconcile the doc/ADR/ANSPEC/INDEX cross-pointers, and **re-verify + re-measure the I-1…I-14 invariant ledger.** The equality proof *is* the gate: the consolidation changes no observable behavior.

## 1. References
```
git fetch origin
git show origin/main:docs/complexity-governance.md        # the ledger + Rules 1-4 + the lock census method (re-measure against this)
git show origin/main:Plans/DISPATCH-2026-06-17-phase-8-consolidation-jarvis.md   # the last consolidation — the precedent shape
git show origin/main:docs/OPEN-THREADS.md                 # OT-14 (resolved-remove-next-cycle), OT-21/23 (banked gates), the register to reconcile
```

## 2. ISCs (the consolidation surface — inspect, then act only where it earns it)
- **ISC-1 — Duplication across the seven builds.** The likely sites: the **quantized-matmul lane** (9.1 scalar / 9.2 wiring / 9.3 AVX2 in `ai/ops/qk_matmul.rs` + `forward_logic.rs` — any depack/dequant duplication between scalar and AVX2 paths that should share a helper?); the **rng** module (10.1); the **fb/usb console + neo-init** glue (input path + `arthur_console`). Collapse only real duplication — per the BUILD-K precedent, if a file is "cohesive-but-large" and the apparent duplication doesn't survive inspection, **leave it and say so** (don't churn a validated driver for a 2-line wrapper).
- **ISC-2 — Dead scaffolding + `#[allow(...)]` audit.** Every `#[allow(dead_code)]` / `#[allow(clippy::…)]` added across the seven builds: confirm each is per-item Rule-5-justified (no blanket crate-root allows — that was OT-14's whole point; verify none crept back). Remove genuinely dead scaffolding; keep gate discriminators (paid-down debt, armed permanently — never removed as "test-only weight").
- **ISC-3 — Doc/pointer reconciliation.** Reconcile the cross-references touched by the seven builds: ADR-058 (9.1/9.2/9.3 — is it coherent across all three slices, or fragmented?), ADR-057 (CSPRNG), the ANSPEC §10.x/§11.x sections, INDEX, the bible Article-13 addendum lines, and the OPEN-THREADS register (OT-14 is RESOLVED — **remove it this cycle** per its own note; verify OT-2/21/23 cross-refs are accurate). Grep-clean: no dangling by-number references.
- **ISC-4 — Re-verify + re-measure the ledger (the heart of Rule 4).** Walk I-1…I-14: each row still has a named, running guard? Re-measure the **lock census** (the documented method — static lock primitives under `kernel/src`) and the **pure-vs-glue LOC ratio** (the `*_logic.rs`/`*_policy.rs`/`ops/` tier). Report the numbers in an audit table (the ADR-041/048 precedent). Flag any drift; if the ledger outgrew one page, say so (that's the Rule-2 alarm).
- **ISC-5 — The feature graph.** Seven builds added many features (`phase9-quantized-matmul`, `phase9-avx2-matmul`, `kernel-rng`, `framebuffer-console`, `usb-hid`, the `arthur`/scenario features). Verify the graph is coherent and the **#243-class hygiene holds** (OT-21 — no re-aliased shared crypto deps; `--workspace --all-targets` clean). No new features added here.

## 3. Gate
- **Behavior-preserving equality:** the **full host + QEMU family passes unchanged** — every KAT, byte-oracle, and scenario green and bit-identical to pre-consolidation (this is the consolidation's kill-switch-equivalent: no observable change). `cargo clippy --workspace --all-targets -- -D warnings` green via Docker (incl. blocking kernel clippy).
- **Kernel-touching (if you touch any `kernel/` code) ⇒ pre-merge `workflow_dispatch` QEMU gate green (all 4 shards).** If the consolidation ends up docs/pointer-only (possible — the last one was largely additive), say so and the host gate suffices.
- **Net LOC should trend flat-or-negative** (consolidation removes more than it adds); report the `git diff --stat`.

## 4. Scope guard
Consolidation **only**. **Do NOT** add features, change behavior, add invariants/`unsafe`, or "improve" working code beyond collapsing real duplication + pruning real dead code. **Do NOT** remove gate discriminators. **Do NOT** start M1 (the Arthur memory build — that's the next build, after this lands). If you find a latent bug, do NOT silently fix it — surface it as a BLOCKER-to-PM (the CI-greening precedent: a real find gets its own analysis, not a drive-by).

## 5. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness vs current `main`, the exact CI host-check set on the rebased tree, SPDX, self-stat). The two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL + the QEMU `workflow_dispatch` URL if kernel-touching), and the PM re-inspects your commit's own `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (the consolidation digest + the re-measured numbers in an ADR, the ADR-041/048 precedent). Next: **M1** (Arthur memory dir+format+fallback — the first ratified OT-17 slice).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Jarvis
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-18-consolidation-phase-8-9-jarvis.md`. **Branch:** `codex/consolidation-phase-8-9-jarvis` off `origin/main`.
- **One-line:** Run the **Rule-4 consolidation** over the seven builds since `645fd4b` (Slice 4, 9.1, 9.2, CSPRNG 10.1, input path, 9.3, arthur_console). **No features, no new invariants/`unsafe`, behavior-preserving** — the full family passes bit-identically (that's the gate). Collapse real duplication (esp. the scalar/AVX2 quantized-matmul depack paths — share a helper only if it's genuinely duplicated; the BUILD-K rule: if "cohesive-but-large" and duplication doesn't survive inspection, leave it and say so); audit every `#[allow]` added (per-item Rule-5 justified, no blanket crate-root — verify OT-14's fix didn't regress); reconcile ADR-057/058 + ANSPEC + INDEX + the bible addendum + the OT register (**remove OT-14, it's RESOLVED**); **re-verify I-1…I-14 + re-measure the lock census + pure-vs-glue LOC** (audit table, ADR-041/048 precedent); verify the feature graph + OT-21 #243-hygiene (`--workspace --all-targets` clean). **Keep gate discriminators armed.** Net LOC flat-or-negative. Gate: full family green + clippy via Docker; kernel-touching ⇒ pre-merge QEMU gate. **Scope: consolidation only — no features, no M1, surface any latent bug as BLOCKER not a drive-by fix.** **Close-gate: run `scripts/worker-close-gate.sh`; RETURN pastes the green CI URL(s) + the re-measured ledger numbers.**
