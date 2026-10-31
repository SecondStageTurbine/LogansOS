<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Isolation-class taxonomy THINK — HongMeng IC0/IC1/IC2 as a kill-switch knob → Prometheus

**Date:** 2026-07-04
**From:** PM (Claude)
**To:** Prometheus (research / design-THINK / host-only lane). New branch off `origin/main` HEAD (`e406a83`): `prometheus/isolation-class-taxonomy-think`. **(Queued task 2 of 2 — after the DeltaNet research.)**
**Status:** DISPATCHED — the design follow-up from the HarmonyOS kernel-lessons THINK (`THINK-2026-07-03-harmonyos-hongmeng-lessons.md` §2). HongMeng makes the security/performance trade-off a **deployment-time knob** via graded isolation classes (IC0 kernel-adjacent shims / IC1 in-kernel services / IC2 address-space-isolated userspace), with a proven safe fallback ("demote everything to IC2 under threat"). That maps almost 1:1 onto our **kill-switch discipline**. Design a NeoPlutarch isolation-class taxonomy that captures the *configurability* while **rejecting IC1-style in-kernel co-location** (we are Rust/formal-verification-first; IC1 inflates the TCB and trades away memory-safety — HarmonyOS's demonstrated weak axis). **Scope: a design THINK, host-only — NO code, no QEMU. Output is a THINK doc.**

## 0. The decision this THINK makes
Define **whether and how** NeoPlutarch should represent "isolation placement" as a first-class, kill-switchable deployment parameter for its services — the way HongMeng does — and if so, the taxonomy + how it wires to our existing kill-switch regression harness. The bar: keep the *good* idea (isolation class as a knob with a proven-safe fallback = a kill-switch at the isolation level) without importing the *bad* one (validated services running in kernel space for speed).

## 1. References
```
git fetch origin
git show origin/main:docs/THINK-2026-07-03-harmonyos-hongmeng-lessons.md   # §2 (the IC0/IC1/IC2 lesson) + §4 (DarkNavy: isolation strength / memory-corruption weakness)
git show origin/main:docs/complexity-governance.md                         # the invariant ledger + the kill-switch discipline (Rule 1) this maps onto
git show origin/main:docs/bible.md                                         # Article 5.6 (userspace/personality isolation), Article 6.6 (no policy in kernel), Article 1.2 (Correctness > Coherence > Performance)
git show origin/main:docs/OPEN-THREADS.md                                  # OT-13 (lock-order band), I-14 (fault containment) — the boundaries a taxonomy must respect
```

## 2. What the THINK must decide
- **D1 — Is isolation-class-as-a-knob a real fit for us, or a HongMeng-ism we admire but don't need?** NeoPlutarch is single-node, capability-secured, Rust-first — do we actually have services that could sensibly move between isolation placements, or is our split already fixed by the capability model? Decide honestly (a "we don't need this, and here's why" is a valid, valuable outcome).
- **D2 — If yes: the taxonomy.** Define the isolation classes for *our* services (analogous to IC0/1/2 but on our terms), the safe-fallback (the IC2-equivalent = maximal isolation), and the rule that a class change is a **kill-switch-gated** move proving "the more-isolated fallback ≡ the current behavior, safety-preserving." Name which existing services (if any) would carry a class.
- **D3 — The hard reject: no IC1.** State the principle that a *validated* service is NOT co-located into kernel space for speed — the TCB-inflation / memory-safety cost is exactly HarmonyOS's demonstrated weak axis (DarkNavy), and it conflicts with our Rust/FV-first stance and Article 1.2 (Correctness first). If a service ever needs kernel-adjacency, it must be Rust-safe + AAG-audited, and even then the *default* class is the isolated one.
- **D4 — Relation to I-14 + the lock band.** How an isolation-class taxonomy interacts with I-14 fault-containment and the OT-13 lock-order band — a class change must not violate either.
- **D5 — Verdict + (if warranted) a build sketch.** Recommend adopt/defer/reject, and if adopt, sketch the ISC decomposition for a future (low-priority) build so it's a straight lift.

## 3. Scope guard
**A THINK doc only.** No code, no kill-switch harness changes, no QEMU. If a decision needs a PL ruling (e.g. "do we want isolation-placement as a configurable axis at all?"), surface it as an open ratification point. Output: `docs/THINK-2026-07-04-isolation-class-taxonomy.md`.

## 4. Gate + close
- **Host only / docs:** `cargo fmt`/SPDX clean on the added doc; **no QEMU** (paths-ignored).
- **Close-gate `scripts/worker-close-gate.sh`** (host set; SPDX; self-stat). RETURN the THINK path + the D1–D5 decisions + any open PL-ratification point. PM reviews; a build (if any) is a later dispatch.
- **Disk reclaim** after merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-04-isolation-class-taxonomy-think-prometheus.md` (+ the HarmonyOS lessons THINK §2/§4, `complexity-governance.md` kill-switch discipline, bible Art 5.6/6.6/1.2). **Branch:** `prometheus/isolation-class-taxonomy-think` off `origin/main` HEAD (`e406a83`). **Do this AFTER the DeltaNet research.**
- **One-line:** **Design a NeoPlutarch isolation-class taxonomy — HongMeng's IC0/1/2-as-a-deployment-knob mapped onto our kill-switch discipline, while rejecting IC1 (in-kernel service co-location).** Produce `docs/THINK-2026-07-04-isolation-class-taxonomy.md` deciding: is isolation-placement-as-a-configurable-knob a real fit for us or a HongMeng-ism we don't need (a well-argued "reject" is valid) (D1); if yes, the taxonomy + safe fallback + the kill-switch-gated class-change rule (D2); the hard no-IC1 principle (TCB inflation / memory-safety cost = HarmonyOS's weak axis; Rust/FV-first + Art 1.2) (D3); interaction with I-14 + the lock band (D4); adopt/defer/reject verdict + a build sketch if warranted (D5). **Design only — no code, no gate.** Surface genuine forks as open PL-ratification points. Close-gate; RETURN the THINK + decisions.
