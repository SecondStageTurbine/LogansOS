<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: design THINK — Arthur grant ergonomics (OT-18) → Prometheus

**Date:** 2026-06-18
**From:** PM (Claude)
**To:** Prometheus (the research/design seat — the OT-17 memory THINK is the immediate precedent and the sibling of this one). New branch off `origin/main` HEAD: `prometheus/arthur-grant-ergonomics-design`.
**Status:** DISPATCHED. **Design only — a ratifiable THINK, no code.** Non-colliding with Jarvis's in-flight consolidation (this touches no code — a `Plans/` doc). The companion-vs-compliance-officer problem: "model proposes, **capability disposes**" (29.3) is the safety keystone, but if every Arthur action triggers a grant prompt, Arthur is a compliance officer, not a companion. This designs **how a human grants, scopes, audits, and revokes Arthur's authority** so the keystone holds *and* Arthur stays usable.

## 0. The decision this build makes
None in code. It produces a **ratifiable design THINK** (options → tradeoffs → recommended D1…Dn, the OT-17/CSPRNG/Phase-9 THINK shape) for the *grant ergonomics* over Arthur's whole action surface — so the keystone (the kernel cap-check) is preserved while the *granting* is humane. **No implementation, no syscalls, no new capability primitive** (the primitives exist — `derive_handle`/`grant`/`revoke_handle`/revoke-through-the-derivation-graph). A design artifact.

## 1. The problem (the unresolved tradeoff)
The capability system answers **CAN** (does Arthur hold the authority); it says nothing about **how the human grants it ergonomically.** Two poles, both wrong:
- **Fine per-action grants** — prompt before every action. Small blast radius, but Arthur becomes a compliance officer; unusable.
- **Coarse durable grants** — grant a broad scope once, Arthur acts silently within it. Good UX, but a **larger blast radius** when Arthur is compromised or *confidently wrong* — the "safe to be wrong" property (the 2026-06-18 red-team concession) degrades as grants widen.
The design must find the humane middle: **front-loaded, scoped, durable grants** where the cap-check is *silent and deterministic within scope* and only a *missing* capability ever surfaces to the user — **without** letting the blast radius become unbounded.

## 2. References
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md                                   # OT-18 (this), OT-17 (ratified sibling — memory), OT-19 (disagreement), OT-20 (multi-principal), OT-22 (one pillar at a time)
git show origin/main:docs/bible.md                                          # Article 29.3 (model proposes, capability disposes), 29.4 (userland), 23.x
git show origin/main:Plans/THINK-2026-06-18-arthur-memory-substrate-design.md  # the sibling THINK — D3 per-category caps is the grant model applied to memory
git show origin/main:Plans/THINK-2026-06-18-arthur-destination-red-team.md     # the "safe to be wrong" concession + the road->traveler framing
```
Built primitives to design *on top of* (do not reinvent): `process::derive_handle(pid, h, CapRights)` (attenuation), `ipc::grant(...)` (delegation), `process::revoke_handle(pid, h)` + revoke-through-the-derivation-graph. The Slice-4 action surface (`userland/libneo/src/action.rs` — the typed `Action`/`Grant`/`RequiredCapability` enum) is what's being granted.

## 3. The design questions to resolve (the THINK's spine — options → tradeoffs → recommendation each)
- **D1 — The grant model.** Coarse front-loaded scopes vs fine per-action vs a hybrid (coarse default + per-action escalation for high-stakes). Recommend; the silent-within-scope / surface-only-on-miss rule is the likely core.
- **D2 — The scope taxonomy.** What are the grantable scopes? (resource classes: files/dirs, network, process-spawn, the OT-17 memory categories, …). The **minimal-sufficient** set for Arthur v1's actual action surface (Slice 4) — not a speculative permission zoo (Article 15.7). How scopes map onto the existing cap derivation.
- **D3 — The friction model (when does the human see a prompt?).** Grant-time only; within-scope action is silent (the deterministic cap-check); a *missing* capability is the only thing that surfaces. Define exactly what surfaces and when.
- **D4 — Graceful capability re-request.** When Arthur hits `ActionError::Denied` (missing cap), how does it request the scope from the user, and how is that bounded so it's not a nag (batching, "grant for this session" vs "always", remember-the-denial)?
- **D5 — Bounding the blast radius (the load-bearing one).** Given coarse durable grants, how is the damage of a compromised / confidently-wrong Arthur bounded? Candidate levers: **reversibility** (cross-link OT-19 — destructive actions default to undoable), time-bounded / session-scoped grants, high-stakes actions requiring confirm regardless of grant, revocation. State which levers and why — this is where the "safe to be wrong" property is preserved under wide grants.
- **D6 — Audit + revoke (the human control surface).** How the user inspects *what Arthur can currently do* and revokes a scope (via revoke-through-the-derivation-graph). Single-Arthur audit here; the *multi-Arthur provenance* (OT-20) is its extension — cross-link, defer.
- **D7 — Default posture.** What does Arthur ship with — no grants (least privilege, earn each scope) vs a minimal sensible default? Recommend least-privilege-start with graceful D4 escalation.

## 4. Discipline
- **Preserve the keystone.** Whatever the ergonomics, the kernel capability check (29.3) remains the boundary — *by construction*, never softened for UX. Ergonomics live in *userland* (how Arthur/the shell request and the user grants); the enforcement stays in the kernel.
- **One pillar (OT-22).** This THINK covers **grant ergonomics across Arthur's action surface** only. It does NOT design: the memory substrate (OT-17, ratified — but its per-category caps are an *input*, cross-link), the philosophy of *disagreement* (OT-19 — D5 reversibility *connects*, but the disagreement model is its own THINK), or multi-Arthur provenance (OT-20 — D6's single-Arthur audit is the seam, defer). Name the boundaries.
- **Minimal-sufficient (Article 15.7).** The smallest grant model that keeps Arthur usable *and* bounded for a real N=1 user. Reject a speculative permission taxonomy.
- **Build on the primitives.** No new authority primitive — the THINK shows how `derive_handle`/`grant`/`revoke` compose into the ergonomics, not a parallel system.

## 5. Deliverable + close (PM/PL)
A single ratifiable **`Plans/THINK-2026-06-18-arthur-grant-ergonomics-design.md`** with §1 the tradeoff, §2 constraints + the existing primitives, §3 the design space, §4 D1–D7 options→tradeoffs→**recommendation**, §5 the OT-17/OT-19/OT-20 boundaries, and a **§10 ratification block (D1…D7 for PL sign-off)**. **No code, no CI gate** (a `Plans/` doc — paths-ignored; no close-gate ritual / QEMU gate — those are for code branches). Close: push the branch; PM reviews for honesty + minimal-sufficiency + the keystone preservation (the cap-check is never softened) + the blast-radius/reversibility soundness (the D5 scrutiny — the same rigor that caught OT-17's D4 over-claim); PL ratifies D1–D7; *then* a build slice follows, one at a time per OT-22. Until ratified, OT-18 stays OPEN.

---
## Relay — what to tell Prometheus
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-18-arthur-grant-ergonomics-design-prometheus.md`. **Branch:** `prometheus/arthur-grant-ergonomics-design` off `origin/main`.
- **One-line:** Produce a **ratifiable design THINK for Arthur grant ergonomics** (OT-18 — the companion-vs-compliance-officer problem; sibling of the just-ratified OT-17 memory THINK). **Design only, no code** (non-colliding with the consolidation). Resolve D1 *the grant model* (coarse front-loaded scopes vs per-action vs hybrid — silent-within-scope, surface-only-on-miss), D2 *the scope taxonomy* (minimal-sufficient for Slice 4's actual action surface — not a permission zoo), D3 *the friction model* (when a prompt surfaces — grant-time + missing-cap only), D4 *graceful re-request* (on `ActionError::Denied`, ask for scope without nagging), **D5 *bounding the blast radius*** (the load-bearing one — coarse durable grants + the "safe to be wrong" property: reversibility [cross-link OT-19], time/session-scoped grants, high-stakes confirm-regardless, revocation), D6 *audit + revoke* (the human control surface; revoke-through-the-derivation-graph; single-Arthur — OT-20 multi-principal is the extension, defer), D7 *default posture* (least-privilege start). **Non-negotiables:** the kernel cap-check (29.3) stays the boundary by construction, never softened for UX; ergonomics in userland; build on the existing `derive_handle`/`grant`/`revoke` primitives (no new authority primitive); minimal-sufficient (15.7). **One pillar (OT-22):** grant ergonomics only — OT-17 memory caps are an input (cross-link), OT-19 disagreement and OT-20 provenance are siblings (cross-link, defer). Deliverable: `Plans/THINK-2026-06-18-arthur-grant-ergonomics-design.md` with a §10 D1–D7 ratification block. PM reviews (same D5 rigor that caught OT-17's D4 over-claim) → PL ratifies → build slice follows.
