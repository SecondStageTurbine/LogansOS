<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-8 — Arthur agentic A3 design THINK (the destructive / irreversible-external tier) → Janus

**Date:** 2026-07-02
**From:** PM (Claude)
**To:** Janus (Phase-8 Arthur-application lane — landed the console, A1, A2). New branch off `origin/main` HEAD (`db61c7f`): `codex/agentic-a3-design-think-janus`.
**Status:** DISPATCHED — **design only, no code, no gate, no hardware.** A2 (`a8a48e3`) shipped the mutating tier and *restricted* the destructive/irreversible-external actions (delete/move/kill/spawn/send) explicitly deferring them to A3. This THINK designs A3 so the eventual build is turnkey. Output is a decision memo (`THINK-…`), not a build. **Non-conflicting** with everything in flight (design work).

## 0. The decision this THINK makes
A1 = read/nav (safe by construction). A2 = mutating-but-reversible (confirm + OT-19 snapshot-undo). **A3 = the tier where the action is destructive or reaches outside the machine, and reversibility is not guaranteed** — `Delete`, a truncating/overwriting `Move`, `Kill`, `Spawn`, and **external/network sends**. The A2 reversibility trick (snapshot-before-write, restore on undo) **does not generalize** here: you cannot un-send a network packet, un-kill a process's side effects, or cheaply snapshot an arbitrary delete. So A3 needs a *different* safety model than "make it undoable." This THINK decides what that model is — **before** any code, so we don't discover the gap on a destructive action that already fired.

## 1. References
```
git fetch origin
git show origin/main:Plans/THINK-2026-06-21-arthur-agentic-action-design.md   # OT-26 — D1 grammar, D4 confirm, D5 blast-radius-by-action-class (the A3 seam)
git show origin/main:Plans/DISPATCH-2026-06-29-phase-8-arthur-agentic-a2-janus.md   # A2 — what "restricted at A2" means + the confirm/undo model A3 extends
git show origin/main:docs/OPEN-THREADS.md    # OT-19 (reversibility), OT-18 (grant lifecycle / E5 Process/Network/Model grants — A3's cap surface)
git show origin/main:docs/bible.md           # Article 29.3 (model proposes, capability disposes) — the keystone A3 must not soften
```

## 2. What the THINK must decide (each: recommendation / rationale / rejected alternative)
- **A3-D1 — The action set + a taxonomy of irreversibility.** Enumerate the A3 actions and classify each: *reversible-with-effort* (delete → trash-not-unlink, recoverable), *irreversible-local* (a true destroy/overwrite), *irreversible-external* (a network send, a spawn with outside effect). The gate strength should key off this class (D5 blast-radius split), not the action name.
- **A3-D2 — The gate model when undo is impossible.** A2's answer was snapshot+undo. For A3, decide the escalation: e.g. **stronger/typed confirmation** (the user must confirm the *specific* irreversible act, not a generic "yes"), a **dry-run/preview** (show exactly what will be destroyed/sent before executing), a **trash-tier** for deletes (convert "irreversible" → "reversible-with-effort" where possible so it drops to A2's model), and an **outright-refuse** tier for the genuinely unsafe (external sends without an explicit standing grant). Decide which actions land in which tier.
- **A3-D3 — The keystone holds harder, not softer (29.3).** The cap-check still disposes. A3's new surface is **Process/Network/Model grants** (OT-18 E5, currently "banked only on real need") — A3 is that real need. Decide how an A3 action acquires its cap: does the user pre-grant a Network/Process scope (front-loaded, D1-style), or is every external act a per-action grant? State how a confused/compromised model is bounded — an ungranted destroy/send must be *impossible*, not merely un-confirmed.
- **A3-D4 — Reversibility ceiling + the "no silent destruction" invariant.** State plainly what A3 will and won't make reversible, and the hard rule: **no A3 action executes without either (a) a recovery path or (b) an explicit typed confirmation of the irreversible act** — never a silent destroy/send. Decide whether this needs a new ledger invariant or rides I-15 (file-caps) + the existing confirmation machinery.
- **A3-D5 — The kill-switch + build decomposition.** Sketch how A3 slots under the `agentic-actions` switch (off ≡ A2, bit-for-bit) and propose the build's ISC split (grammar + tiered-confirm + grant-acquisition + refuse-tier + the QEMU failure-mode scenarios) so the follow-on build dispatch is a straight lift from this THINK.

## 3. Scope guard
**A THINK doc only.** Do NOT write kernel/userland code, add a grammar production, touch the cap-check, or add a QEMU scenario — this decides the design so the *next* dispatch builds it. If a decision needs a PL ruling (e.g. "external sends: pre-granted scope vs per-action prompt vs refuse-by-default"), surface it as an open ratification point, don't pick unilaterally. Output: `Plans/THINK-2026-07-02-arthur-agentic-a3-design.md`.

## 4. Gate + close
- **Docs-only:** `cargo fmt`/SPDX clean on the added doc; **no QEMU, no AAG** (design artifact; paths-ignored CI). Close-gate `scripts/worker-close-gate.sh` (host set; SPDX; self-stat).
- **RETURN** the THINK path + the A3-D1…D5 decisions + any open PL-ratification points. PM reviews; the A3 *build* is a later dispatch synthesized from it.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-02-phase-8-arthur-agentic-a3-design-janus.md` (+ the OT-26 design THINK, the A2 dispatch, OT-18/OT-19, Article 29.3). **Branch:** `codex/agentic-a3-design-think-janus` off `origin/main` HEAD (`db61c7f`).
- **One-line:** **Design the A3 destructive/irreversible-external tier — the one where A2's snapshot-undo doesn't generalize.** Produce `Plans/THINK-2026-07-02-arthur-agentic-a3-design.md` deciding: the A3 action set + an irreversibility taxonomy (A3-D1); the gate model when undo is impossible — typed confirmation / dry-run preview / trash-tier / outright-refuse, keyed by blast-radius class (A3-D2); how A3 acquires its cap via OT-18 E5 Process/Network/Model grants with the 29.3 keystone holding harder (A3-D3); the "no silent destruction/send" rule + whether it needs a new invariant (A3-D4); and the kill-switch + ISC decomposition for the follow-on build (A3-D5). **Design only — no code, no gate.** Surface genuine forks (esp. external-send policy) as open PL-ratification points. Close-gate; RETURN the THINK + decisions + open points.
