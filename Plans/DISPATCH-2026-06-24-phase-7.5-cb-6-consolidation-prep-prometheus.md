<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-7.5 CB-6 consolidation PREP (host-only/research) — the close audit, ahead of the tag → Prometheus

**Date:** 2026-06-24
**From:** PM (Claude)
**To:** Prometheus (research / consolidation-support / host-only lane; just closed CB-prep). New branch off `origin/main` HEAD: `prometheus/cb-6-consolidation-prep`.
**Status:** DISPATCHED — **runner-free CB-6 groundwork while the kernel/QEMU lanes run** (Cerebro on bare-metal, Oracle on CB-3). CB-6 is the Phase-7.5 close (Rule-4 consolidation + status-repair + integrated proof + tag) and is PM-driven on the *final* tree — but the **analysis** can be done now so the close is turnkey. **Scope: research/docs/host-only — NO kernel code, no QEMU, no behavior change. The output is a THINK (a decision memo), not a build.**

## 0. The decision this prep makes
Produce the **CB-6 consolidation audit** so that when the technical tail (CB-2/CB-3 + the bare-metal session) is green, the close is mechanical: a clear pay-or-bank call on the clippy-debt, a dead-scaffolding inventory, a verified invariant-ledger page-fit, and the OPEN-THREADS prune list.

## 1. References
```
git fetch origin
git show origin/main:docs/complexity-governance.md   # the I-1..I-15 invariant ledger — NOTE the doc already says the section "exceeds a printed page" (:17-18): the consolidation-cadence alarm is LIVE
git show origin/main:docs/OPEN-THREADS.md            # OT-1..OT-40 — the RESOLVED ones (OT-2, OT-30, OT-37, …) are pruned at phase close per the file's own maintenance rule
git show origin/main:Plans/PLAN-2026-06-23-phase-7.5-closeout.md   # §CB-6 (Janus checklist steps 8-10) — the authoritative CB-6 scope
git show origin/main:Plans/THINK-2026-06-23-phase-7.5-closeout-phase-close-audit-janus.md  # Janus's prior close audit (the starting inventory)
# The mt7601u clippy-debt: run `cargo clippy -p neoplutarch-kernel --all-targets` over the driver (host-readable) to enumerate the banked ~33-warning set referenced in CLAUDE.md's CI-hygiene note
```

## 2. ISCs (all produce ANALYSIS, not code)
- **ISC-1 — Clippy-debt inventory + pay-or-bank recommendation.** Enumerate the banked mt7601u (and any V/W0–W2) clippy warnings that BUILD-K consolidated *before* and that were never folded in (CLAUDE.md CI-hygiene note: "the banked ~33-warning mt7601u clippy-debt"). For each cluster, recommend **pay** (fix in CB-6) or **bank** (justify, with a one-line rationale). A clean number + a recommendation — the actual fixing is CB-6's coder step.
- **ISC-2 — Dead-scaffolding scan.** Inventory scaffolding that the closed builds made obsolete (e.g. superseded fixtures, one-off helpers) — but **explicitly EXCLUDE gate discriminators**: those are paid-down debt and stay armed permanently (CLAUDE.md). List candidates with "safe to remove / keep" + reason. Do NOT remove anything; just the list.
- **ISC-3 — Invariant-ledger page-fit verdict.** `complexity-governance.md` already states the I-1..I-15 section *exceeds a printed page* (I-14/I-15 guard prose). Per the consolidation cadence, feature work pauses for consolidation when the ledger outgrows a page — assess: does CB-6 need to **re-fit** it (tighten guard prose / move detail to the ADRs) to restore the one-page legibility test? Recommend the specific trims.
- **ISC-4 — OPEN-THREADS prune list.** List the **RESOLVED** OTs to remove at the close audit (OT-2, OT-30, OT-37, and any other RESOLVED), each with its closing evidence preserved in the commit message, per the register's own maintenance rule. Flag any OT marked RESOLVED that *shouldn't* be pruned yet (closing evidence not actually on `main`).
- **ISC-5 — The fresh-reader legibility pass.** A short "what would confuse a fresh reader of the close tree" list (stale pointers, doc/ADR cross-refs that moved) — the Rule-4 legibility test, as input to CB-6's status-repair.

## 3. Scope guard
**Host-only + a THINK doc.** **Do NOT:** modify kernel/driver code, fix the clippy warnings (that's CB-6's coder step on the final tree), remove any scaffolding or OT entries (just *list* them), add a QEMU scenario, or touch a Cargo build feature. If a task seems to need a code change, **stop and note it** as a CB-6 line-item. Output: `Plans/THINK-2026-06-24-phase-7.5-cb-6-consolidation-audit.md`.

## 4. Gate + close (PM)
- **Host only:** if any host tooling is run (`cargo clippy` for the inventory), no code changes ship — the deliverable is the THINK. `cargo fmt`/SPDX clean if any file is added. **No QEMU gate** (research/docs; paths-ignored CI).
- **Close-gate `scripts/worker-close-gate.sh`** (host set; SPDX; self-stat). RETURN the THINK path + the four lists (clippy pay/bank, dead-scaffolding, ledger-refit verdict, OT-prune). PM folds it into CB-6.
- **Disk reclaim** after merge per CLAUDE.md.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-24-phase-7.5-cb-6-consolidation-prep-prometheus.md` (+ `complexity-governance.md`, `OPEN-THREADS.md`, the PLAN §CB-6, Janus's close-audit THINK). **Branch:** `prometheus/cb-6-consolidation-prep` off `origin/main` HEAD.
- **One-line:** **Runner-free CB-6 groundwork — a close-audit THINK, no code.** Produce `Plans/THINK-2026-06-24-phase-7.5-cb-6-consolidation-audit.md` with four lists: **(1)** the banked **mt7601u clippy-debt** (run `cargo clippy` to enumerate the ~33-warning set) + a **pay-or-bank** call per cluster; **(2)** a **dead-scaffolding** inventory (EXCLUDE gate discriminators — they stay armed); **(3)** an **invariant-ledger page-fit verdict** (the doc already says I-1..I-15 *exceeds a page* — recommend the specific trims to restore one-page legibility); **(4)** the **OPEN-THREADS prune list** (RESOLVED OTs to remove at close — OT-2/OT-30/OT-37/… — with closing evidence preserved). Plus a short fresh-reader legibility list. **Scope: host-only + the THINK — do NOT fix clippy, remove scaffolding/OTs, or touch kernel code (just LIST); if something needs code, note it as a CB-6 line-item.** **Close-gate `scripts/worker-close-gate.sh`; RETURN the THINK path + the four lists.**
