<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (RESEARCH/AUDIT THINK): Phase-7.5 closeout — the formal phase-close audit (landed-vs-planned, criteria, consolidation, tag) → Janus

**Date:** 2026-06-23
**From:** PM (Claude)
**To:** Janus (free main-tier). **Research/audit only — produce a THINK/audit doc, NO code, NO QEMU.** New branch off `origin/main` HEAD: `codex/phase-7.5-closeout-audit-janus`.
**Status:** DISPATCHED. One of **four parallel closeout-research lanes**; the PM synthesizes all four THINKs into the Phase-7.5-closeout PLAN. **Host-only — must not open a QEMU gate** (Stage-B owns the runners).

## Why this lane
The other three lanes resolve the open *technical* tail (ch6 wedge, GTK/DHCP, SMP hang, 7.5-D). This lane answers the orthogonal question the PM needs to *declare the phase closed honestly*: **what does the bible actually require to close Phase 7.5, and where is the gap between what landed and what the lane plan promised?** This is the gap-analysis + close-criteria audit, not a build.

### Charter A — landed-vs-planned gap analysis against the lane order
The ratified lane order is `0→A(xHCI)→B(USB-Ethernet)→V(VT-d)→C1(iwlwifi)→C2(WPA2/3 control plane)→D(identity wiring)`, Rule-4 consolidation after V. Produce a **table**: each lane item → what landed (commit/ADR/silicon-evidence) → what's still open → its owner. Surface every divergence honestly, e.g.:
- **C1 was planned as `iwlwifi`; the lane executed `MT7601U`** (ADR-054) — flag this as the open scope fork (Cerebro's THINK gives the decision-grade analysis; you record it as an open close-criterion the PL must rule).
- The Rule-4 consolidation "after V" — did it happen? Is there a *second* consolidation owed before close (the banked ~33-warning mt7601u clippy-debt, any dead scaffolding, doc/ADR pointer reconciliation)?

### Charter B — the bible's close criteria + the honest "what blocks close" list
The bible's phase-close definition: all criteria pass, no anti-criteria triggered, **real boot verification on QEMU AND on ≥1 bare-metal target**, a single squashed commit lands on `main` with the phase tag. Map 7.5 against each:
- **Bare-metal verification** — what is silicon-proven (W1 scan, W2 association on the OptiPlex 7080) vs what the close criterion still needs (full cold end-to-end association? DHCP-over-`wlan0`?). State the *minimum* bare-metal bar to call WiFi closed honestly — neither inflated nor deflated.
- **Anti-scope** — re-read the Phase-7.5 anti-scope; confirm nothing in the lane violated it.
- The **ordered close checklist**: the exact remaining gates (technical items from the other 3 lanes + consolidation + the squash/tag mechanics) in dependency order, so the PM's synthesized PLAN can sequence the closeout builds.

### Charter C — the honesty pass on the public-facing status
Cross-check that the bible Art. 13 ledger line, the README status, and OPEN-THREADS OT-27 all state the **same** reality (the PL's standing "always honest, both directions" rule). Flag any drift to fix at close.

## References
```
git fetch origin
git show origin/main:docs/bible.md                                       # Article 13 ledger + the phase-close definition (criteria/anti-criteria/boot-verification/squash+tag)
git show origin/main:Plans/THINK-2026-06-11-phase-7.5-opening-connectivity-lane.md   # the ratified lane order + Rule-4-after-V plan
git show origin/main:docs/OPEN-THREADS.md                                # OT-27 (the WiFi tail) + the banked mt7601u clippy-debt note
git show origin/main:docs/complexity-governance.md                       # Rule-4 consolidation cadence; the invariant ledger state to re-verify at close
git show origin/main:README.md                                           # the public status line (honesty cross-check)
git show origin/main:docs/ADR-054-phase-7.5-mt7601u-driver.md            # what's silicon-proven (the bare-metal evidence)
# docs/ADR-045/046/047/050/057 — the landed 7.5 lane ADRs (0/A/B/V/W1/W2) for the gap table
```

## Deliverable
An audit THINK on your branch: `Plans/THINK-2026-06-23-phase-7.5-closeout-phase-close-audit-janus.md` (SPDX `CC-BY-4.0`) — Charter A (the landed-vs-planned gap table), Charter B (the bible-criteria map + minimum honest bare-metal bar + ordered close checklist), Charter C (the public-status honesty cross-check). RETURN the path.

## Scope guard
**Research/audit only.** No code, no QEMU, no gate, no doc *edits* to the bible/README in this lane (you produce the *findings*; the PM lands the actual status edits at close, after synthesis). Read the landed evidence and report the gap honestly — do not assert "closeable" or "blocked" beyond what the evidence supports; where a criterion needs a PL ruling (the scope fork), say so.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-closeout-phase-close-audit-janus.md` (+ references). **Branch:** `codex/phase-7.5-closeout-audit-janus` off `origin/main`.
- **One-line:** **Research/audit THINK, no code/QEMU** — one of four Phase-7.5-closeout brains. **(A)** Build the **landed-vs-planned gap table** against the lane order `0→A→B→V→C1→C2→D` (commit/ADR/silicon-evidence per item; flag C1-was-iwlwifi-but-ran-MT7601U as the open scope fork; check the Rule-4 consolidation + the banked mt7601u clippy-debt). **(B)** Map 7.5 against the **bible's close criteria** (criteria/anti-criteria/QEMU+bare-metal verification/squash+tag), state the **minimum honest bare-metal bar** for WiFi, and produce the **ordered close checklist** in dependency order. **(C)** Honesty cross-check that bible Art. 13, README, and OT-27 state the same reality. **Deliver `Plans/THINK-2026-06-23-phase-7.5-closeout-phase-close-audit-janus.md`; RETURN the path.** Host-only — no QEMU gate; produce findings, the PM lands the status edits at close.
