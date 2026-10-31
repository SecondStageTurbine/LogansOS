<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-8 — Arthur agentic A3 build (destructive / irreversible-external tier) → Janus

**Date:** 2026-07-02
**From:** PM (Claude)
**To:** Janus (Phase-8 Arthur-application lane — A1, A2, and the A3 design THINK author). New branch off `origin/main` HEAD (`ed7719e`): `codex/phase-8-arthur-agentic-a3-build-janus`.
**Status:** DISPATCHED — a **straight lift from your own A3 THINK** (`Plans/THINK-2026-07-02-arthur-agentic-a3-design.md`, `c36c403`). Builds the destructive/irreversible-external tier: the actions A2 explicitly restricted (delete/move/kill/spawn/external-send), now safe via the tiered gate the THINK designed. **Non-conflicting** with Cerebro's WiFi surface + Oracle's CB-5a (this is the Arthur sampling boundary + userland action wiring; not mt7601u/wpa, not the forward math). **Kernel-touching ⇒ 4-shard QEMU gate + AAG.**

## 0. Ratification (the THINK's one open PL point — RULED 2026-07-02)
**External-send policy: per-send typed confirmation for first A3.** No standing grant auto-sends. Every `NetSend`/external-capable act in the **X (irreversible-external)** tier requires: the Network/Process/Model grant (OT-18 E5) **AND** a dry-run preview **AND** typed confirmation of that specific send. This is the safest first cut and matches "irreversible-external = highest gate." (PL may revisit later; this is the build's ratified assumption.)

## 1. References
```
git fetch origin
git show origin/main:Plans/THINK-2026-07-02-arthur-agentic-a3-design.md   # THE SPEC — A3-D1…D5, the R/T/L/X/U taxonomy, ISC-A3-1…6
git show origin/main:Plans/DISPATCH-2026-06-29-phase-8-arthur-agentic-a2-janus.md   # A2 — the confirm/undo model + execute_action cap-check A3 extends
git show origin/main:docs/OPEN-THREADS.md   # OT-18 (E5 Process/Network/Model grants — A3's cap surface), OT-19 (reversibility is structural, not model-judged)
git show origin/main:docs/adversarial-audit-gate.md   # the AAG you'll be RETURNED against
# the kernel constrained-decode masker + execute_action_with cap-check (A1/A2, action.rs) — extend the grammar, do NOT touch the cap-check
```

## 2. ISCs (your THINK's ISC-A3-1…6; each byte-oracle'd where it defines bytes)
- **ISC-A3-1 — Grammar / action taxonomy.** A3 ASTs for `dry_run`, `confirm_typed`, `cancel`/`refuse`, and the destructive/process/network/model intents, classified by the **R/T/L/X/U** taxonomy (D1). Same-name actions route by *parameters* (a same-FS move-to-trash is R; a permanent unlink is L; `NetSend` is always X). **No new action bypass.** Byte-oracle fixtures for every new AST (continue A1/A2's).
- **ISC-A3-2 — Preview + typed-confirm state machine (D2).** Pending-action digest, typed-phrase generation, expiry, **stale-confirm rejection**, wrong-phrase no-op, and a **grant-revocation recheck before execution** (a cap revoked between preview and confirm must re-deny). The digest surfaces exactly what will be destroyed/sent (path/bytes/undo-window; dial host:port; spawn binary+inherited-handles).
- **ISC-A3-3 — OT-18 E5 grants (D3).** Process/Network/Model grant-store parse/serialize, on-miss re-request, session-vs-durable handling. **No grant+execute shortcut** (granting a scope must NOT execute the pending action as a side effect — return to a fresh dry-run/confirm). Typed confirmation is *intent, not authority* — the cap-check still disposes after confirm.
- **ISC-A3-4 — Recoverable file-destructive tier (T→R, D2/D4).** Implement delete/move-equivalent as **trash/journal** (`.arthur/undo/`) where exact restoration is available → downgrade to the A2 recovery model. If conversion fails, **escalate to L or refuse — never fall through silently.**
- **ISC-A3-5 — Refuse tier + external-send policy (D2/D5, U tier).** Wildcard/broad scopes, unpreviewable payloads, protected-`.arthur/`-state destruction, and kill-of-supervising-session are **outright refused** (no typed-confirm override in A3). External sends obey the ratified per-send-typed-confirm policy (§0).
- **ISC-A3-6 — Failure-mode scenarios (the safety contract, D4).** Host KATs + QEMU scenarios: **malformed=impossible, ungranted=denied, no-confirm=no-op, wrong-confirm=no-op, stale-preview=no-op, trash-undo restores, kill=typed-confirm-required, spawn-inherited-handle=denied, no-silent-network-send, kill-switch=A2-equivalence.** This enumeration IS the deliverable (the A2 discipline).

## 3. Standing-rule sections
- **Keystone (29.3) — holds harder, never softened.** Every A3 action routes through the **unchanged** `execute_action()` cap-check; `action.rs` not in the diff. Typed confirmation gates *whether the NL layer may cause it*; the cap gates *whether it's permitted at all*. A confused/compromised model with no cap **cannot** destroy/send — impossible, not merely un-confirmed.
- **No-silent-destruction/send invariant (application-level).** A3 never performs a silent destroy/send — it proves a recovery path first OR requires typed confirmation after a dry-run. Encode as the A3 safety contract + test matrix.
- **Invariant budget (Rule 2):** the THINK says A3 **rides existing substrate (I-15 file-caps + the confirmation machinery) — no new ledger invariant** unless the build introduces a genuinely new cross-cutting kernel guarantee or handle/cap-graph rule. If you find you need one, name+justify+guard it in-build (don't smuggle it); otherwise state "no new invariant" in the RETURN.
- **Kill-switch (Rule 1):** an A3 sub-gate under the `agentic-actions` family; **A3 disabled ⇒ A2 posture bit-for-bit** (A1/A2 work as today; delete/move/kill/spawn/external stay restricted with the A2 restricted response). The equality regression is the required artifact.
- **I-14:** the masker/state-machine is pure CPU at the sampling/userland boundary (no ISR/DMA/governance lock); inherited from A1/A2 — restate.
- **Scope guard:** A3 as the THINK scopes it. **Do NOT:** soften the cap-check; add an action that bypasses `execute_action()`; touch the forward math / `.neom` / vocab; touch mt7601u/xHCI/DHCP/wpa (Cerebro + Oracle's surface); or add a ledger invariant without justifying it. Constrained decoding makes the AST *parseable*, not the model *correct* — the tiered gate is why a wrong destructive/send is *safe*, not why it won't be proposed.

## 4. Gate + close
- **Kernel-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` workflow_dispatch + AAG.** Scenarios = the ISC-A3-6 matrix. Push-CI green (host KATs incl. byte-oracles + kill-switch A2-equivalence). **Green without the CI run URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase onto `origin/main` — CB-5a/consolidation may land in parallel; re-run the gate after rebase; host set; SPDX; self-stat).
- **RETURN** with the QEMU shard URLs + SHA, the byte-oracle pass, the **ISC-A3-6 failure-mode enumeration** (the safety argument), the kill-switch A2-equivalence proof, the 29.3/I-14 statements, and an explicit "no new invariant" (or the named+guarded one). **Disk reclaim** after merge.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-02-phase-8-arthur-agentic-a3-build-janus.md` (+ your A3 THINK — it's the spec). **Branch:** `codex/phase-8-arthur-agentic-a3-build-janus` off `origin/main` HEAD (`ed7719e`).
- **One-line:** **Build the A3 tier you designed** — the R/T/L/X/U taxonomy (ISC-A3-1), the dry-run + typed-confirm state machine with stale/wrong/revoked rejection (ISC-A3-2), OT-18 E5 Process/Network/Model grants with **no grant+execute shortcut** (ISC-A3-3), delete/move-as-trash → A2 recovery (ISC-A3-4), the refuse tier + **external-send = per-send typed confirmation (RULED)** (ISC-A3-5), and the failure-mode matrix (ISC-A3-6). **Keystone (29.3) holds harder** — typed-confirm is intent, the unchanged `execute_action()` cap-check is authority; an ungranted destroy/send is *impossible*. **Kill-switch:** A3 off ≡ A2 bit-for-bit. **No new invariant** unless the build truly earns one (name+guard in-build). **Scope: A3 only** — no cap-check softening, no connectivity/forward/`.neom` touch. **Gate:** 4-shard QEMU + AAG (the ISC-A3-6 matrix). Close-gate; RETURN shard URLs + SHA + the failure-mode enumeration + kill-switch A2-equivalence + 29.3/I-14 + the invariant statement.
