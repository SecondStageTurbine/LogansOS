<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PL-DECISION: Arthur is sovereign over its own memory — E4 user-gate dropped (OT-17 D3 stands)

**Date:** 2026-06-22
**Decider:** PL (JC), surfaced by PM from Oracle's E4 reconcile THINK.
**Article 24.1 standing:** reaffirms a ratified decision (OT-17 D3) against a conflicting later dispatch (OT-18 E4); no constitutional change.

## Context
OT-18 E4 was dispatched to extend the grant lifecycle (E1 derive/grant → E2 re-request → E3 audit/revoke) to a **Memory scope**. Oracle's reconcile-first gate (`Plans/THINK-2026-06-22-arthur-grant-memory-scope-e4-reconcile-oracle.md`) found — and **stopped on, per the dispatch's explicit "don't force-fit" clause** — a genuine, mutually-exclusive conflict:

- **OT-17 D3 (ratified):** **Arthur is sovereign over its own memory** — Arthur owns the `.arthur/memory/` dir cap (DERIVE+REVOKE) and derives caps **outward** to tools. Grantor = Arthur.
- **OT-18 E4 (as dispatched):** the **user gates** Arthur's memory — the shell derives per-category caps and grants them **inward** to Arthur, revocable. Grantor = user.

Opposite ownership/direction for the *same* store; you cannot have both. E4 as written would silently override OT-17's ratified sovereignty stance.

## Decision
**Arthur remains sovereign over its own memory — OT-17 D3 stands.** The user's control over Arthur's memory is **forget / crypto-erase** (OT-17 M-series: per-category key-burn, already built), **not** per-category capability gating.

Consequences:
1. **E4 (the user-gates Memory scope) is NOT built.** The `codex/arthur-grant-memory-scope-e4-oracle` production work stays unbuilt; the dispatch is **superseded**. Oracle's reconcile THINK was the correct deliverable (it surfaced the fork instead of force-fitting) — the discriminator/honesty discipline working as intended.
2. **The OT-18 user-grant E-series closes at E3.** E1 (spawn-time derive/grant) + E2 (on-miss re-request) + E3 (audit + revoke) are complete for **FileRead/FileWrite** — the lifecycle Arthur actually needs to act on the user's files under capability bound. Memory is governed by OT-17 sovereignty, not OT-18 user-grants.
3. **`Grant::Memory` is not a user-grant scope.** `entry_to_grant` keeps returning `None` for Memory (its current state) — that is now the *intended* end state, not a deferral. (Process/Network/Model remain banked as a hypothetical E5 *if* a concrete need arises — not assumed.)
4. **If a tool ever needs Arthur's memory** (a calendar app, a health dashboard reading Arthur's recollections), that is the **OT-17 outward direction** (Arthur derives a read-only cap to the tool) — a separate, OT-17-consistent build, dispatched only when there's a real consumer. Not E4.

## Rationale
- **Ratified + coherent.** OT-17 D3 is the standing decision; the companion model (Article 29) makes Arthur's durable, private memory-of-the-user core to *being* Arthur — not a resource the user micromanages category-by-category.
- **The real user need is already served.** "I don't want Arthur to remember that" / "wipe it" = forget + crypto-erase (OT-17 M-series), which exists. Per-category user-gating adds a control surface nobody asked for, at the cost of a constitutional flip + an `openat` precursor build + a subsystem join (`arthur-grants`+`arthur-memory` never compile together today).
- **Cheapest honest path.** No constitutional change, no precursor build, and it closes the grant lifecycle cleanly where it's actually complete (files).

## References
OT-17 D3 (`THINK-2026-06-18-arthur-memory-substrate-design.md`); OT-18 D2 (`THINK-2026-06-18-arthur-grant-ergonomics-design.md`); the E4 reconcile THINK (`THINK-2026-06-22-arthur-grant-memory-scope-e4-reconcile-oracle.md`); ADR-060 (grant ergonomics — to be amended: Memory is not a user-grant scope per this ruling).
