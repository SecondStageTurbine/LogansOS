<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7.5 BUILD-K — connectivity-lane consolidation (the second Rule-4 digest) → Jarvis

**Date:** 2026-06-13
**From:** PM/Cerebro
**To:** Jarvis (branch `codex/phase-7.5-build-K-consolidation-jarvis`, already created off `origin/main` `c611960`)
**Status:** DISPATCHED. **Authored after a ground-truth audit (Jarvis's + PM re-verification @ `c611960`) — this REPLACES the earlier from-memory chat preview, which was wrong on three items.** `[skip ci]` for the doc-only path; the full build+boot gate applies ONLY if the conditional `xhci` refactor (ISC-4) is taken.
**Authority:** `docs/complexity-governance.md` **Rule 4**, second invocation: the connectivity lane is four feature builds deep (7.5-0 / A / B + branding). C1 was the first (policy layer); this is its connectivity counterpart.

## 0. What this build is — and the honest, *small* scope
The lane was built disciplined, so the **code-collapse portion is small**: the `dead_code` allows are already per-item Rule-5-labeled (zero blanket), and I-14 is already split into its three vectors. So this consolidation is **primarily the mandated doc-hygiene + ledger re-audit + the fresh-reader test** — not a big refactor. Zero new features/invariants/`unsafe`; zero behavior change.

**Audit scorecard (tree-verified @ `c611960` — what the preview got wrong, corrected here):**
- The "four blanket `dead_code` allows" do **not** exist — all 20 are per-item, Rule-5-labeled (ISC-3 is verify-only, not a removal).
- `xhci.rs` is **1787 lines** (the preview's "1787" was right; a later audit's "1676" was wrong — PM re-verified three ways on a clean tree).
- I-14 is **already split** (a)/(b)/(c1)/(c2) — ISC-5 is a wording refinement, not a re-structure.
- The "31-vs-25 lock drift" was a grep artifact (docs say **31** consistently; ISC-6 re-measures to the canonical method, no doc inconsistency exists).

## 1. ISCs

### The real work — spec/doc hygiene
- **ISC-1 — Fix the `§10.5` collision + renumber (REQUIRED; the one non-trivial item).** `neoplutarch-ai-subsystem-spec.md` has **two `### 10.5`** — L1042 "USB host" (BUILD-A) and L1054 "Model-as-attack-vector" (the original security §) — with 10.6/10.7 wedged between, so the security threat-model block and the connectivity block are interleaved. Renumber to a single clean scheme: keep the **threat-model block** (threat model / capability / integrity / side-channels / model-as-attack-vector) contiguous and the **connectivity block** (DMA isolation / USB host / USB-Ethernet) contiguous. **Cross-ref blast radius (verified):** `docs/specs/neoplutarch-ai-capability-semantics-spec.md` §10.3 references *"ANSPEC-001 §10.5 (model-as-attack-vector)"* **by number** (L410/L412) — update it to the new number. Then grep-prove zero dangling `§10.x` pointers across `docs/` + the ADR "Spec:" lines + `bible.md` + `INDEX.md`.
- **ISC-2 — Delete the stray root artifact.** `latest-screenshot.png` (267 KB) is tracked at the repo root. Grep for references, then delete (ephemeral "latest" capture; a keepable boot shot belongs at `Plans/evidence/<named>.png`).
- **ISC-3 — `dead_code` sweep = VERIFY (already clean).** The 20 `#[allow(dead_code)]` in `usb/` + `dma_arena.rs` are already per-item with Rule-5 labels ("discriminator surface — gate-consumed", "kill-switch gate seam", "I-14(b) storm-gate seam"). Confirm they stay narrow + labeled; delete only anything `clippy` newly surfaces as genuinely dead. **Do not strip a labeled discriminator/seam (Rule 5).** This is a checkbox, not a removal pass.

### The ledger re-audit (the mandated Rule-4 step)
- **ISC-4 — Assess `xhci.rs` (1787 lines) — refactor ONLY if it's a real win.** It's the lane's largest file. **First verify whether genuine duplication exists** (descriptor-walk / fence-consumption shared with `cdc_ecm.rs` / `usb_parse.rs`) or a clean concern-split (registers / rings / contexts / enumeration / class-offer) that *materially* improves legibility. **If yes:** do it (and the full gate in §2 applies). **If it is cohesive-but-large:** leave it and say so — do NOT churn a just-silicon-validated driver for cosmetics. BLOCKER-to-PM if unsure. (The preview asserted duplication without verifying; do not inherit that assumption.)
- **ISC-5 — I-14 wording refinement (light).** The row already splits (a)/(b)/(c1)/(c2). Update it: (a)/(b)/(c1) are now **silicon-PROVEN** (BUILD-A/B exercised the fenced DMA on a real AX88179 — 47 admits / 0 refusals across the bare-metal runs; the ISR cap held under real MSI; no AI-governance lock on any USB ISR path) — promote the wording from "guarded (QEMU)" to "silicon-proven." (c2) stays **mandated-not-yet-proven** (guard ships at 7.5-V/VT-d). Update the audit-log line.
- **ISC-6 — Re-audit I-1…I-14 + re-measure structural facts to the canonical method.** Walk every row, name its running guard, confirm it runs. Re-measure the two structural facts using the **C1 canonical method** (which yielded 31 static locks) and record that method so future audits are comparable — the preview's "25" was a narrower-grep artifact, not a doc drift. **And verify the lane fits the lockdep:** the ordered-lock LEVELS now include the USB band (`USB_DEVICE` etc.) — confirm they still occupy the lockdep's documented `1..=31` domain (ADR-039); if 7.5 pushed the level count past 31, that is a REAL finding → BLOCKER-to-PM, do not silently exceed the domain. The audit table lands in **ADR-048**.

### Legibility + standards
- **ISC-7 — Fresh-reader test + kill-switch re-prove.** A cold read of `kernel/src/drivers/usb/mod.rs` + the module doc-comments must reconstruct the lane (arena → xHCI → USB core → class-driver seam → CDC-ECM → NetDevice) without `git log`; make `usb/mod.rs` the table of contents. Re-prove the kill-switches: arena-off ≡ pre-arena, USB-off ≡ pre-USB, `cdc_ecm` disabled ≡ BUILD-A. The RETURN states the cold read was deliberate and what it forced.
- **ISC-8 — Standards / proof of zero behavior change.** SPDX; **no new `unsafe`/invariants**; `cargo fmt`; `clippy -D warnings`; line endings clean (`git add --renormalize .` per `.gitattributes`); **ADR-048** records the consolidation (renumber map + what was collapsed/deleted/verified/re-measured — the precedent log, like ADR-041 for C1). **Gate scales with ISC-4:** if no `xhci` refactor, the changes are docs + a file delete + line-ending normalization → the gate is the `§10.x` grep-clean + the ledger audit table + builds-still-green; `[skip ci]`. If the refactor IS taken, add the full family + a boot screenshot (splash byte-unchanged) + the kill-switch equalities. **The diff is a deliverable:** report net LOC delta, the renumber map, pointers fixed, ledger 14/14, the lock-census method + figure, and the fresh-reader findings.

## 2. Gate
- **Stage A:** builds (on+off; + cdc_ecm/USB/arena-disabled for the kill-switch legs); `clippy`/`fmt`/`shard-coverage`; the `§10.x` grep clean; the ledger audit table; renormalized line endings; **if ISC-4 taken:** the full scenario family.
- **Stage B (only if ISC-4 taken):** the family green + a boot screenshot (splash unchanged). **If doc-only:** no QEMU run needed beyond "builds green"; the doc-consistency greps ARE the gate.

## 3. Scope guard
Zero new features. No drivers/ops/policies/locks/invariant rows; no VT-d (that's **7.5-V**); do NOT touch `kernel/src/ai/**`; do NOT remove/weaken any discriminator/fence-counter/blind-bank breadcrumb (Rule 5); do NOT change any gate-parsed trace line; do NOT refactor `xhci` cosmetically (ISC-4 is conditional on a real win). A defect surfaced mid-digest is its own commit + a PM note, never folded in silently.

## 4. Merge (PM, on PASS)
Close-gate: rebase onto current `main`, re-run Stage-A (+ Stage-B if ISC-4), push. PM `pm-merge-check` → `--ff-only`. The second Rule-4 cycle completes; **7.5-V (VT-d, the I-14(c2) guard)** opens next on a clean, re-audited lane.

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-13-phase-7.5-build-K-consolidation-jarvis.md`
- **Authority:** `docs/complexity-governance.md` Rule 4 (Rules 1/2/5 bind it)
- **Branch:** `codex/phase-7.5-build-K-consolidation-jarvis` off `origin/main` `c611960` (you already created it).
- **One-line:** Corrected, tree-verified — supersedes the from-memory preview. The lane's code is already clean (allows labeled, I-14 split), so this is mostly doc-hygiene + the mandated ledger re-audit: **fix the duplicate `§10.5` + renumber** (update the ANSPEC-002 §10.3 by-number cross-ref + all pointers, grep-clean); **delete root `latest-screenshot.png`**; **verify** (not strip) the Rule-5-labeled allows; **assess `xhci.rs` (1787) — refactor only if a genuine dedup/legibility win, else leave it** (don't churn a silicon-validated driver); **I-14 wording → (a)/(b)/(c1) silicon-PROVEN, (c2) deferred to 7.5-V**; **re-audit I-1…I-14 + re-measure locks to the C1 method (31), verify the lane fits lockdep 1..=31**; fresh-reader pass; re-prove the kill-switches. ADR-048 records it. Gate scales: doc-only ⇒ grep-clean + builds-green + `[skip ci]`; if you take the `xhci` split ⇒ full family + boot screenshot.
