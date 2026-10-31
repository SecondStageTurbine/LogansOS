# PM-DISPOSITION — Bible v1.7.1 → v1.7.2 — Article 13 phase-ledger amendment

**Date:** 2026-05-28
**Type:** Bible amendment (Article 27), ledger-only status update
**Authority chain:** PL sign-off in session 2026-05-28 ("fix bible ledger, you have my sign off"). PM (Cerebro) applied.
**Version:** 1.7.1 → **1.7.2**

## What changed

`docs/bible.md`, Article 13 "Current phase ledger" only — no article body text altered:

- **Phase 2** (SMP, capability system, buddy+slab allocators, advanced TLB management): `Planned next` → **`Complete`**. Was stale; Phase 2 closed via the 12-cpu bare-metal hardware sweep (commit `ce2cba3`, `CLOSE-REPORT-2026-05-27-phase2-12cpu-hardware-sweep.md`).
- **Phase 3** (NeoFS, block layer, real disk I/O): no status → **`Complete`**. Closed and bare-metal-verified through BUILD-5b (`CLOSE-REPORT-2026-05-28-phase-3.md`).
- **Phase 4** (network stack, IPv6, sockets-equivalent; USB stack + USB Ethernet bundled): no status → **`Active`** (opened this session — see `PM-DECISION-2026-05-28-phase-4-opening.md`).
- Footer: version bump + amendment-history entry.

## Rationale (Article 27)

The ledger had drifted from reality — it still showed Phase 2 as "Planned next" after Phase 2 closed, and carried no status for the now-closed Phase 3. An accurate ledger is the canonical phase-status reference; leaving it stale undermines that. This is a factual status reconciliation, not a design change, hence a patch-level bump (1.7.1 → 1.7.2) and no article-text edits.

## Scope notes / not done

- Article 28.5 prose still references "NeoPlutarch at Phase 2 (current planning horizon)" — also stale, but it is article body text, not the ledger, and outside the scope PL signed off ("fix bible ledger"). Flagged for a future amendment if PL wants the prose reconciled; not touched here.
- No phase tag created and no history squash (consistent with practiced precedent; the v1.0.0 Article 13 "squashed commit + phase tag" clause is not observed in this repo).

---

**Applied.** Bible at v1.7.2.
