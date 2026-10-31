<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur ships "yolo-within-guardrails" by default (the front-loaded-trust posture) → Oracle

**Date:** 2026-07-03
**From:** PM (Claude)
**To:** Oracle (main-tier). New branch off `origin/main` HEAD **once the OT-8/OT-9/A3 batch lands (`08fcb95`)** — confirm `origin/main == 08fcb95` before branching: `codex/arthur-yolo-posture-oracle`.
**Status:** DISPATCHED — **the PL-requested headline: Arthur comes out of the box acting freely inside granted scopes, with the guardrails intact.** This is a **composition/policy build**, not new mechanism — it assembles the *already-ratified* OT-18 front-loaded-grant model (D1: "coarse, durable, front-loaded scopes; silent within scope; only a *missing* cap surfaces") + the A2/A3 action tiers + reversibility into a **shipped default posture**, with the kernel cap-check (Article 29.3) as the unbypassable floor. **Non-conflicting:** Janus is routed to the NeoFS-truncate lane and Jarvis to CI, so this Arthur-agentic surface is yours alone for the build. **Kernel/userland-touching ⇒ 4-shard QEMU + AAG.**

## 0. The ratified posture (PL decision 2026-07-03)
**Default = yolo-within-scope.** Arthur executes **reversible** actions **inside its granted scopes** with **no per-action confirmation** — the friction drops to exactly two cases: **(a) a missing cap** (surfaces as a grant request, never a silent failure — OT-18 D3 "on-miss"), and **(b) an irreversible-external act** (NetSend / permanent-wipe / kill-of-session — the A3 **X/U tiers stay gated or refused even in yolo**; you cannot un-send a packet). Everything reversible-within-scope just happens, recoverable via the A2/A3 undo/trash net. The **two-layer separation is the whole point**: this turns *down layer 1 (confirmation friction)*, never *layer 2 (the kernel capability floor)*.

## 1. References
```
git fetch origin
git show origin/main:docs/OPEN-THREADS.md   # OT-18 (D1-D7, RATIFIED — the front-loaded-grant posture this ships) + OT-19 (reversibility)
git show origin/main:Plans/THINK-2026-07-02-arthur-agentic-a3-design.md   # the R/T/L/X/U tiers + which stay gated (X/U) vs flow (R/T)
git show origin/main:Plans/DISPATCH-2026-06-29-phase-8-arthur-agentic-a2-janus.md   # the A2 confirm/undo machinery the posture reconfigures
git show origin/main:Plans/DISPATCH-2026-07-02-phase-8-arthur-agentic-a3-build-janus.md   # A3 tiers + the execute_action cap-check keystone (DO NOT touch)
git show origin/main:docs/bible.md   # Article 29.3 (model proposes, capability disposes) — the floor this must not soften
# the landed A2/A3 code: userland/neo-arthur (conversation_logic, main.rs agentic handlers) + the grant store
```

## 2. ISCs
- **ISC-1 — The default posture config.** A boot-default "trust-within-scope" mode: reversible actions (A3 R/T tiers) inside a **held grant** execute **without** the per-action typed-confirm A3 currently requires. The confirmation surfaces ONLY on (a) missing-cap → a grant request (OT-18 D3) and (b) an X/U-tier act (§0). Front-loaded grants (OT-18 D1) are how scope is set once, up front.
- **ISC-2 — The guardrail is provably intact (Article 29.3).** This is a **posture** change — the `execute_action()` cap-check is **UNTOUCHED** (`action.rs` not in the diff). Prove the floor holds *in yolo*: an action needing a cap Arthur **doesn't hold is DENIED** by the kernel even with all confirmation skipped. A confused/hijacked yolo-Arthur cannot exceed its held caps. This is the deliverable that makes "yolo" safe.
- **ISC-3 — The reversibility net + the irreversible gate.** A wrong-but-**reversible** yolo action is recoverable via the A2/A3 undo/trash (prove undo restores). The **irreversible-external (X) and unsafe (U) tiers stay gated/refused even in yolo** — NetSend still requires its per-send typed confirm; a permanent wipe with no trash still refuses. Prove an X-tier act does NOT flow silently under the yolo default.
- **ISC-4 — Kill-switch (three postures, provably reversible complexity).** A `default-posture` selector: **yolo-within-scope (new default)** / **cautious (A3 confirm-every-risky)** / **off (pre-agentic)**. Prove **cautious ≡ today's A3 posture bit-for-bit** and **off ≡ pre-agentic bit-for-bit** — the new default is switch-off-able to the exact previously-proven layers (an I-2-form collapse). The complexity you can provably switch off is bounded.
- **ISC-5 — The safety-contract failure matrix (host KATs + QEMU).** `yolo_reversible_granted=executes_silently`, `yolo_missing_cap=DENIED`, `yolo_irreversible_external=still_gated`, `yolo_wrong_reversible=undo_recovers`, `posture_cautious=a3_equiv`, `posture_off=pre_agentic_equiv`. This enumeration IS the safety argument.

## 3. Standing-rule sections
- **Keystone (29.3):** the cap-check is **untouched** (ISC-2). Yolo changes *when Arthur asks the user*, never *what the kernel permits*. Prove it.
- **Invariant budget (Rule 2):** **no new invariant** — this rides I-15 (file-caps) + the OT-18 grant graph + the existing confirmation machinery; it is policy/posture, not a new cross-cutting guarantee. State "no new invariant" (or name+guard one if the build truly earns it).
- **Kill-switch (Rule 1):** ISC-4 — the three-posture selector with both equalities proven.
- **I-14:** the posture logic is pure userland/sampling (no ISR/DMA/governance lock); inherited from A1/A2/A3 — restate.
- **Byte-oracle:** any new config/policy format (e.g. the default-posture descriptor) ships a hand-laid fixture.
- **Scope guard:** the default-posture + the three-way selector ONLY. **Do NOT:** modify `execute_action()`/the cap-check; add a new destructive action (that's A3's set); weaken the X/U gating; touch mt7601u/wpa/forward/`mm-slab`/observability (other coders' surfaces); or ship yolo without the cautious/off fallbacks proven equal.

## 4. Gate + close
- **Kernel/userland-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` + AAG.** Scenarios = the ISC-5 matrix (esp. `yolo_missing_cap=DENIED` and `yolo_irreversible_external=still_gated` — the two that prove the guardrail). Push-CI green. **Green without the CI run URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase onto `origin/main`; re-run after rebase; host set; SPDX; self-stat).
- **RETURN** with the QEMU shard URLs + SHA, the failure matrix, the **29.3 proof** (missing-cap denied in yolo; `action.rs` untouched), both kill-switch equalities (cautious≡A3, off≡pre-agentic), and the "no new invariant" statement. **Disk reclaim** after merge. **On merge, Arthur ships yolo-within-guardrails by default** — the headline capability.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-03-arthur-yolo-within-guardrails-oracle.md` (+ OT-18 D1-D7, the A3 THINK's R/T/L/X/U tiers, the A2/A3 dispatches, Article 29.3). **Branch:** `codex/arthur-yolo-posture-oracle` off `origin/main` **after the OT-8/OT-9/A3 batch lands (`08fcb95`)** — confirm `main == 08fcb95` first.
- **One-line:** **Make Arthur ship "yolo-within-guardrails" by default** — reversible actions inside granted scopes execute with **no per-action confirmation** (OT-18 D1 silent-within-scope); friction surfaces ONLY on a **missing cap** (→ grant request) or an **irreversible-external act** (X/U tiers stay gated/refused even in yolo). **The guardrail is the deliverable:** the `execute_action()` cap-check is **UNTOUCHED** and you *prove* a missing-cap action is DENIED even with confirmation skipped (Article 29.3 — a hijacked yolo-Arthur still can't exceed its held caps). Reversible-wrong is undo-recoverable; irreversible-external never flows silently. **Kill-switch:** a 3-posture selector — yolo (default) / cautious (≡ today's A3 bit-for-bit) / off (≡ pre-agentic) — both equalities proven. **No new invariant; posture/policy only; do NOT touch the cap-check or other coders' surfaces.** **Gate:** 4-shard QEMU + AAG (the `missing_cap=DENIED` + `irreversible_external=still_gated` scenarios are the ones that prove it's safe). Close-gate; RETURN shard URLs + SHA + the failure matrix + the 29.3 proof + both kill-switch equalities.
