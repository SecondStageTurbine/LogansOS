<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-8 — Arthur agentic actions A2 (the mutating tier: confirm + reversibility) → Janus

**Date:** 2026-06-29
**From:** PM (Claude)
**To:** Janus (Phase-8 Arthur-application lane — landed the physical console + agentic A1). New branch off `origin/main` HEAD: `codex/phase-8-arthur-agentic-a2-janus`.
**Status:** DISPATCHED — **the A2 slice of the agentic-action design** (`THINK-2026-06-21-arthur-agentic-action-design.md`, OT-26): extend Arthur from the read/nav tier (A1, `a037f17`) to the **mutating tier** (`WriteFile`, `Move`, `Delete`-class `ActionKind`s) — the tier A1 explicitly deferred because it needs **D4 confirmation + OT-19 reversibility**. **Non-conflicting** with Cerebro's WiFi surface (this is the sampling boundary + userland action wiring, not mt7601u/xHCI/DHCP, not the forward math). **Kernel-touching ⇒ 4-shard QEMU gate + AAG.**

## 0. The decision this build makes
Make a fallible 0.5B model safely perform **mutating** OS actions. A1 proved read/nav is safe-by-construction (reversible, low-stakes). A2's mutating actions are **not** reversible-by-construction, so two design pillars from the THINK become load-bearing:
- **D4 confirmation** — a mutating action the model proposes is **confirmed before execution** (the blast-radius-by-action-class split: D5). The grammar already has the `clarify` branch (A1); A2 adds the **confirm** gate for the mutating `ActionKind`s.
- **OT-19 reversibility** — where feasible, a mutating action is **undoable** (snapshot/trash-not-unlink semantics), so "wrong but confirmed" is recoverable, not destructive.

The keystone (Article 29.3) is **never softened**: NL is UNTRUSTED — it *proposes*; the kernel cap-check (`execute_action()`, unchanged) *disposes*. A2 adds confirmation/reversibility **on top of**, never **instead of**, the cap-check.

## 1. References
```
git fetch origin
git show origin/main:Plans/DISPATCH-2026-06-25-phase-8-arthur-agentic-a1-janus.md   # A1 — the read/nav slice this extends (grammar, execute_action, byte-oracle, the BOUNCE→re-fix on model-driven selection)
git show origin/main:Plans/THINK-2026-06-21-arthur-agentic-action-design.md         # OT-26 — D1 grammar, D2 execute, D4 confirm, D5 blast-radius split
git show origin/main:docs/OPEN-THREADS.md                                            # OT-19 (reversibility), OT-18 (grant lifecycle — the cap surface A2 mutating actions need)
git show origin/main:userland/neo-arthur/src/main.rs                                 # the agentic loop + execute_action wiring
# the kernel constrained-decode masker + execute_action_with cap-check (A1, action.rs) — extend the grammar, do NOT touch the cap-check
```

## 2. ISCs
- **ISC-1 — Grammar: the mutating `ActionKind`s + the confirm branch.** Extend the A1 constrained-decode grammar (logit-mask at the sampling boundary — **engine math FROZEN**, OT-40/OT-37 untouched) to cover the mutating actions, each parameterized by held grants. Add the **confirm** production (D4): a mutating AST is not executable until confirmed. Hand-laid **byte-oracle** for the new AST encodings (continuing A1's). Model-driven selection (not heuristic — A1's BOUNCE lesson: the proof fixture must be non-vacuous, the model distinguishes candidates by real logits).
- **ISC-2 — Confirmation gate (D4/D5).** Wire the confirm step: a model-proposed mutating action surfaces to the user (console/serial) and executes **only** on explicit confirmation. D5 blast-radius split: reversible-mutating (e.g. `WriteFile` to a trashable path) may use a lighter confirm; irreversible-external is restricted/blocked at A2 (defer to A3). **Failure-mode enumeration is the deliverable** (the A1 discipline): malformed AST = impossible (grammar); ungranted = DENIED with the missing cap; granted-but-unconfirmed = no-op; confirmed-wrong-but-reversible = recoverable (ISC-3).
- **ISC-3 — Reversibility (OT-19).** For the mutating actions in scope, implement undoable semantics (trash-not-unlink / pre-write snapshot) so a confirmed-but-wrong mutation is recoverable. State precisely which actions are reversible and which are restricted-at-A2.
- **ISC-4 — Kill-switch + the keystone proof.** `agentic-actions` (the existing A1 switch) default-off ≡ today's behavior bit-for-bit; the A2 mutating grammar is gated within it (off ⇒ read/nav-A1 only, or the Hermes path). Prove the cap-check keystone is untouched: every model-selected mutating AST routes through the **unchanged** `execute_action()` — `action.rs` cap-check not in the diff.

## 3. Standing-rule sections
- **Kill-switch (Rule 1):** see ISC-4 (default-off ≡ prior; the equality regression is the artifact).
- **Invariant budget (Rule 2):** preserve the ledger; **no new invariant** (A2 is application/sampling wiring; the cap graph is I-15, already in the ledger). Article 29.3 (model proposes, capability disposes) is *upheld*, not changed.
- **Byte-oracle:** every new AST encoding ships a hand-laid fixture (continue A1's).
- **I-14:** the masker is pure CPU at the sampling boundary (no ISR/DMA/governance lock), in the OT-40 preemptible path — inherited from A1; restate.
- **Scope guard:** A2 mutating tier ONLY. **Do NOT:** implement the irreversible-external/destructive A3 tier; change the forward math or the `.neom`/vocab; soften the cap-check keystone; touch mt7601u/xHCI/DHCP; add a new invariant. Constrained decoding makes the output *parseable*, not the model *correct* — confirmation + reversibility are why a wrong mutation is *safe*, not why it won't happen.

## 4. Gate + close
- **Kernel-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` workflow_dispatch + AAG.** Scenarios: valid-mutating-AST-from-garbage + granted+confirmed-executes + granted-unconfirmed-noop + ungranted-DENIED + reversible-undo + kill-switch equality. Push-CI green (host KATs incl. byte-oracles). **Green without the CI URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase, re-run, push ff-clean; host set; SPDX; self-stat).
- **RETURN** with the QEMU shard URLs + SHA, the byte-oracle pass, the failure-mode enumeration (the safety argument), the kill-switch equality, and the 29.3/I-14 statements. **Disk reclaim** after merge.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-29-phase-8-arthur-agentic-a2-janus.md` (+ the A1 dispatch, the OT-26 design THINK, OT-19 reversibility). **Branch:** `codex/phase-8-arthur-agentic-a2-janus` off `origin/main` HEAD.
- **One-line:** **Extend agentic Arthur from read/nav (A1) to the mutating tier — safely, with confirmation + reversibility.** Add the mutating `ActionKind`s to the constrained-decode grammar with a **confirm** branch (D4) and **reversible/undoable** semantics (OT-19) so a confirmed-but-wrong mutation is recoverable; the keystone (29.3) is **never softened** — NL proposes, the unchanged `execute_action()` cap-check disposes. Model-driven AST selection (A1's BOUNCE lesson — non-vacuous proof fixture). **Failure-mode enumeration is the deliverable** (malformed=impossible / ungranted=DENIED / unconfirmed=no-op / confirmed-wrong=recoverable). **Kill-switch** `agentic-actions` off ≡ today bit-for-bit. **Scope: A2 mutating ONLY** — no A3 destructive/irreversible-external, no forward-math change, no connectivity, no cap-check edit. **Gate:** 4-shard QEMU + AAG. Close-gate; RETURN shard URLs + SHA + byte-oracle + failure-mode enumeration + kill-switch equality + the 29.3/I-14 statements.
