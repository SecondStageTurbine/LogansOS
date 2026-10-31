<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Consolidation build (the overdue cadence) → Oracle

**Date:** 2026-07-22
**From:** PM (Claude)
**To:** Oracle (main-tier; free after the perf lane)
**Status:** DISPATCHED — **no-feature consolidation, kernel-touching-clean, QEMU-gated (NO hardware).** The consolidation cadence (CLAUDE.md rule 3: one no-feature consolidation per ~3-4 feature builds) is **overdue** — this session shipped glass Slices 1/2/3/4-wiring/5, showcase A/B, C0/C0.5, WiFi ×2, the perf lane, vault Phase B, NetDialCap, and more. Pay the debt down before it compounds.

## Scope — collapse, delete, reconcile, re-verify (NO new features)
1. **Collapse duplication.** Find + unify duplicated logic across the recent builds (e.g. the two DMA arenas' shared span/validate logic, the mt7601u targeted-vs-survey CAL decision, any copy-pasted scenario scaffolding, repeated cap-authority patterns). Extract to one home; keep behavior bit-for-bit.
2. **Delete dead scaffolding.** Remove genuinely-dead code/flags/fixtures left by the churn — BUT **gate discriminators and armed negative-controls are NEVER "test-only weight"** (CLAUDE.md): they are paid-down debt and **stay armed permanently**. Do not remove a discriminator, a kill-switch equality, or a negative-control. When unsure whether something is dead, leave it + note it.
3. **Reconcile docs/ADR/ledger pointers.** Fix stale cross-references (ADR ↔ ADR, OT-row ↔ commit, README/INDEX). Verify the **invariant ledger** (`docs/complexity-governance.md`) still reflects reality (I-1…I-16 + any this session added) and **fits one page** — if it's overgrown, flag it (feature work pauses for consolidation, but that's a PM call).
4. **Re-measure + re-verify the ledger.** Confirm the kill-switches still hold (disabled ≡ prior), the discriminators still fire, and the fresh-reader legibility test passes (a new reader can navigate the accel/browser/wifi subsystems from the docs).

## Standing rules
- **Behavior-preserving.** This is a consolidation, not a refactor-with-changes: every collapse/delete must be provably behavior-neutral (unit-test equality where expressible; the QEMU gate proves the scenarios still pass unchanged). If a "consolidation" would change any observable behavior, it's out of scope — note it as a follow-up.
- **No feature, no new invariant.** If you find a genuine bug while consolidating, note it (don't fix-and-expand here — a bug fix is a separate gated build).
- **Kill-switch:** default build bit-for-bit unchanged.

## Close-gate + merge
- `scripts/worker-close-gate.sh` (rebase-fresh), green 4-shard `workflow_dispatch` QEMU URL (proves the collapses/deletes didn't change any scenario), PM AAG (behavior-neutrality: nothing observable changed; no discriminator/kill-switch/negative-control removed; ledger reconciled). Branch `codex/consolidation-cadence-oracle`; PM merges after gate + AAG. **The RETURN should list exactly what was collapsed/deleted/reconciled** so the behavior-neutrality is auditable.

## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-22-consolidation-cadence-oracle.md` + `…:docs/complexity-governance.md` (the invariant ledger + the consolidation rule). Work in `Workers\oracle`; branch `codex/consolidation-cadence-oracle`.
- **One-line:** **Run the overdue consolidation build (no features), QEMU-gated, no hardware.** Collapse duplication (shared DMA-arena/CAL/scenario/cap logic → one home, behavior bit-for-bit), delete genuinely-dead scaffolding (**never** a gate discriminator / kill-switch equality / armed negative-control — those stay armed permanently), reconcile stale doc/ADR/OT-row/README pointers, and re-verify the invariant ledger (`docs/complexity-governance.md`) reflects reality + fits one page + passes the fresh-reader legibility test. **Behavior-preserving only** — any change to observable behavior is out of scope (note it); a bug you find is a separate build. Kill-switch: default build bit-for-bit unchanged. RETURN lists exactly what was collapsed/deleted/reconciled (auditable neutrality). Close-gate + green 4-shard QEMU URL + PM AAG.
- **RAG** (PowerShell): use it to find duplication — `dma_arena graphics_arena span validate`, `scenario harness scaffolding`, `cap authority grant derive revoke`, `invariant ledger I-14`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read.
