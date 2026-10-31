<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Rule-4 consolidation — pay down the six-build run (yolo → OT-34) → Oracle

**Date:** 2026-07-05
**From:** PM (Claude)
**To:** Oracle (main-tier coder). **Branch:** `codex/consolidation-rule4-oracle` off `origin/main` HEAD (`8d84e6c` or later — fetch fresh).
**Status:** DISPATCHED — the cadence is due: **six feature builds since the last consolidation** (ADR-069 yolo posture, NeoFS truncate, CI-deflake, OT-17 M3 delegation, IPC-frequency instrumentation, OT-34 threaded matmul + F1 fail-stop fix). Per `docs/complexity-governance.md` Rule 3/4: one no-feature consolidation per ~3–4 feature builds — collapse duplication, delete dead scaffolding, reconcile doc/ADR pointers, re-verify + re-measure the ledger, pass the fresh-reader legibility test. **No new features. Gate discriminators are never removed as "test-only weight."**

## ISCs
- **ISC-1 — Ledger re-verify + re-measure.** Walk `docs/complexity-governance.md`: every invariant guard still armed and green (I-1 was just extended by OT-34 — confirm the extension reads correctly and the guard actually runs); the lock census still matches reality (OT-34 added NO lock — `PAR_BUSY` is a try-flag; verify the census says so or is silent correctly); the ledger still fits one page. Re-run every ledger-cited regression/KAT and cite the run.
- **ISC-2 — OT-39 residuals (2)+(3) — explicitly tagged for "the next consolidation."** (2) `set_rx_demux_enabled` writes `Release` but `demux_eth_rx` reads `RX_DEMUX_ENABLED` with `Relaxed` (`dhcpv4.rs` ~235/256) — upgrade the read to `Acquire`. (3) `demux_eth_rx` ignores IP Total-Length / UDP-Length / IHL≥5 — add the RFC length validation (bounds-safe today via `parse_message`'s reject, so this is correctness-hygiene, not a vuln fix; keep the change minimal). Both are kernel-touching.
- **ISC-3 — OT-36 boot-frame strings — CONDITIONAL.** If Jarvis's in-flight deflake build already shipped its OT-36 rider, skip this and say so. Otherwise: update `kernel/src/init/boot_frame.rs` — phase → current (Phase 8 / Arthur), verify the `scheduler: cooperative ready` line against the live preemptive-SMP scheduler, leave the version string `v0.0.1` (PL's call to change). Coordinate via `git fetch` on his branch before touching it.
- **ISC-4 — Dead-scaffolding sweep.** Grep for superseded scaffolding from the six builds: pre-truncate whole-file-rewrite fallbacks that M3 obsoleted, deflake leftovers, any `#[allow(dead_code)]` that no longer earns its keep, TODO/FIXME markers that the landed builds resolved. Delete only what is provably dead (the kill-switch OFF-paths are NOT dead — they are the ledger's proofs; never touch them).
- **ISC-5 — Doc/ADR pointer reconciliation.** ADR-070 (threaded matmul) cross-links; OPEN-THREADS rows consistent with reality (OT-34 → DONE is in; OT-8/OT-9/OT-32 rows current; OT-17 stays open until M4 lands); the AAG record note from the F1 re-audit (the KAT guards the pure policy but not the kernel-side `Halt`-arm wiring — add a one-line `// F1: do not rewire to a return; see par_matmul_logic.rs` guard comment at the `Halt` arm if not already present, closing the N2 review-seam note mechanically).
- **ISC-6 — Fresh-reader legibility.** The Rule-4 standard: a fresh reader opens `docs/complexity-governance.md` + `kernel/src/ai/` and can reconstruct what runs when, what proves what, and where the kill-switches are. Fix what fails that test; report what you fixed.

## Scope guard
No new features, no new invariants, no behavior changes beyond ISC-2/ISC-3's specified fixes. Anything that smells like a feature goes in the RETURN as a note, not in the diff. Overlap caution: Jarvis (test harness/CI, `arthur_memory_encrypted` scenario) and Janus (neo-arthur/neo-init/memory) are in flight — do not touch their files; if an ISC-4 deletion would collide, defer it with a note.

## Gate + close
- **Kernel-touching (ISC-2 dhcpv4, possibly ISC-3) ⇒ the 4-shard QEMU `workflow_dispatch` gate is mandatory.** Stagger behind Jarvis/Janus's gates if the runners are contended.
- **Close-gate `scripts/worker-close-gate.sh`** (rebase-freshness, full host set in CI order, SPDX, self-stat). RETURN: green push-CI URL + green QEMU URL + diff-stat + the ISC-1 ledger readout + what ISC-4 deleted + the ISC-3 conditional outcome.
- **Disk reclaim** after the PM confirms merge.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-05-rule4-consolidation-oracle.md` (+ `docs/complexity-governance.md`, OT-39/OT-36 rows in OPEN-THREADS). **Branch:** `codex/consolidation-rule4-oracle` off `origin/main` HEAD.
- **One-line:** **Rule-4 consolidation after the six-build run (yolo → truncate → ci-deflake → M3 → IPC-freq → OT-34): no features, pay down the debt.** Re-verify + re-measure the whole invariant ledger incl. the fresh I-1 threading extension (cite runs) (ISC-1); fold the two OT-39 residuals — `RX_DEMUX_ENABLED` read `Relaxed`→`Acquire` + IP/UDP length validation in `demux_eth_rx` (ISC-2); OT-36 boot-frame strings ONLY if Jarvis's rider didn't ship it (check his branch first) (ISC-3); dead-scaffolding sweep — kill-switch OFF-paths are proofs, never dead (ISC-4); doc/ADR pointer reconciliation + the F1 `Halt`-arm guard comment (ISC-5); fresh-reader legibility pass (ISC-6). **Do not touch Jarvis's (harness/CI) or Janus's (neo-arthur/neo-init) in-flight files.** Kernel-touching ⇒ QEMU gate mandatory, staggered behind their gates. Close-gate; RETURN both green CI URLs + ledger readout + deletions list.
