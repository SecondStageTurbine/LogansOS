<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Rule-4 consolidation — the post-A2/1.5B/OT-41 clean-tree pass → Jarvis

**Date:** 2026-07-02
**From:** PM (Claude)
**To:** Jarvis (main-tier — landed OT-41 + the 1.5B boot). New branch off `origin/main` HEAD (`ed7719e`): `codex/rule4-consolidation-2026-07-02-jarvis`.
**Status:** DISPATCHED — the **consolidation-cadence build** (complexity-governance Rule 4: one no-feature consolidation per ~3–4 feature builds). Since CB-6 (`d958f8f`) we've landed **A2** (`a8a48e3`), **1.5B** (`5316abd`), **OT-41** (`1fec219`), the **CB-5 trap-mine** (docs), and the **1.5B boot** (`6f6486c`) — the cadence says a clean-tree pass is due. **No feature work.** **Kernel-touching (dead-code removal / ledger) ⇒ 4-shard QEMU gate + AAG.**

## 0. The one hard exclusion
**Do NOT touch the WiFi / wpa / mt7601u / xHCI / DHCP surface** — Oracle's **CB-5a is active there right now** and Cerebro owns it. Any consolidation of that surface waits until CB-5a/CB-5b close. Your lane is the **Phase-8 / brain / Arthur / ledger / docs** surface that just accreted.

## 1. References
```
git fetch origin
git show origin/main:docs/complexity-governance.md   # the I-1..I-15 ledger — re-verify + re-measure; page-fit was cleared by CB-6, keep it clear
git show origin/main:docs/OPEN-THREADS.md            # prune RESOLVED OTs; reconcile status text
git show origin/main:Plans/DISPATCH-2026-06-29-phase-7.5-cb-6-consolidation-janus.md   # the CB-6 consolidation pattern to mirror (the four lists + the two hard rules)
git show origin/main:Plans/THINK-2026-06-24-phase-7.5-cb-6-consolidation-audit.md      # the audit shape (clippy pay/bank, dead-scaffolding, ledger, OT-prune, legibility)
```

## 2. ISCs (mirror the CB-6 pattern; ANALYSIS → then the mechanical pass)
- **ISC-1 — Duplication / dead-scaffolding scan (EXCLUDE gate discriminators + oracles).** Inventory scaffolding the recent builds made obsolete — superseded fixtures, one-off helpers, unused feature seams — on the Phase-8/brain/Arthur surface. **Rule-5 exclusion is absolute:** gate discriminators, KAT oracles (esp. the rng KAT / byte-oracles), and blind-hardware capture breadcrumbs (the 1.5B-boot `-15b` discriminators Jarvis just seeded) **stay armed permanently** — never remove them as "test-only weight." List "safe to remove / keep" with reason; remove only the confirmed-dead.
- **ISC-2 — Clippy hygiene on the accreted surface.** `cargo clippy --all-targets` over the Phase-8/brain/Arthur crates; pay the trivial/auto-fixable, bank the structural (one-line `// clippy-bank:` rationale). Do NOT run the mt7601u-feature clippy (that's CB-5's/Cerebro's surface).
- **ISC-3 — Invariant-ledger re-verify + re-measure.** Confirm A2/1.5B/1.5B-boot added **no** invariant (they claimed none — verify). Re-measure the lock census + the pure-policy LOC % if they moved (1.5B added `heap_policy`; A2 added agentic userland — neither is a new *kernel* lock, confirm). Keep the I-1..I-15 table **within one printed page** (CB-6 cleared the alarm — don't let it re-raise). If a build silently changed a row's guard, flag it.
- **ISC-4 — OPEN-THREADS prune + status reconcile.** Remove the newly-RESOLVED entries with closing evidence preserved in the commit message: **OT-41 (2)+(3)** (closed by `1fec219`), the **1.5B follow-on** status (boot config-swap landed `6f6486c` — the bare-metal 1.5B run is the remaining open item), and any other RESOLVED since CB-6. Do NOT prune multi-item OTs still partly open. Reconcile stale status text (e.g. OT-28 next-brain: 1.5B now boots).
- **ISC-5 — Fresh-reader legibility pass.** The Rule-4 legibility test: stale cross-refs, doc/ADR pointers that moved, the `complexity-governance.md` LOC-number pair I flagged (header says 61.7%/5,724 vs the structural-facts paragraph 60.9%/5,518 — reconcile to the true current count), and any dispatch/INDEX drift.

## 3. Scope guard
- **In scope:** dead-scaffolding removal (non-discriminator), clippy hygiene (non-WiFi), ledger re-verify/re-measure, OT prune + status reconcile, legibility fixes.
- **Out of scope / STOP:** any feature/behavior change; **any WiFi/wpa/mt7601u/xHCI/DHCP touch** (CB-5a active); removing a gate discriminator, KAT oracle, or blind-hardware breadcrumb; changing an invariant's *guarantee*; a new invariant. If a cleanup needs real logic surgery, note it as a follow-on line-item, don't expand the pass.

## 4. Gate + close
- **Kernel-touching (ISC-1/ISC-3 remove/verify compiled code) ⇒ mandatory 4-shard QEMU `NEO_SMP=4` + AAG.** Push-CI green. **Green without the CI URLs is not done.** (If the pass turns out docs-only after the scan, say so and the QEMU gate is moot — but any code removal re-gates.)
- **Close-gate `scripts/worker-close-gate.sh`** (rebase onto `origin/main` — A3-build/CB-5a may land in parallel; re-run after rebase; host set; SPDX; self-stat).
- **RETURN** with the QEMU shard URLs + SHA (or the docs-only note), the removed-scaffolding list, the ledger re-verify verdict (page-fit still clear; no invariant changed), the OT-prune list with preserved evidence, and an explicit **"no gate discriminator / KAT oracle / blind-hw breadcrumb removed"** line. **Disk reclaim** after merge.

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-02-rule4-consolidation-jarvis.md` (+ `complexity-governance.md`, `OPEN-THREADS.md`, the CB-6 consolidation dispatch/audit as the pattern). **Branch:** `codex/rule4-consolidation-2026-07-02-jarvis` off `origin/main` HEAD (`ed7719e`).
- **One-line:** **The Rule-4 clean-tree pass for the post-CB-6 wave (A2/1.5B/OT-41/1.5B-boot)** — dead-scaffolding removal (ISC-1), clippy hygiene (ISC-2), invariant-ledger re-verify + re-measure keeping the I-1..I-15 table one-page (ISC-3), OPEN-THREADS prune + status reconcile (ISC-4, incl. OT-41(2)+(3) and the 1.5B-boot status), fresh-reader legibility incl. the stale LOC-pair in `complexity-governance.md` (ISC-5). **HARD EXCLUSION: do NOT touch the WiFi/wpa/mt7601u surface — Oracle's CB-5a is live there.** **Never remove a gate discriminator, KAT oracle, or blind-hardware breadcrumb** (the CB-6 rng-KAT lesson; the 1.5B `-15b` captures stay). **Gate:** any code removal ⇒ 4-shard QEMU + AAG (my AAG centers on "nothing live/armed was removed"). Rebase before close. RETURN shard URLs + SHA + the removed list + ledger verdict + OT-prune + the "no discriminator/oracle removed" line.
