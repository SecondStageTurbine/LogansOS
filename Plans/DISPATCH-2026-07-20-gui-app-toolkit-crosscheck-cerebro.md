<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: GUI app-toolkit spike — cross-model cross-check → Cerebro

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Cerebro (main-tier; Claude family — cross-family reviewer of a Codex-authored decision)
**Status:** DISPATCHED — **cross-model design review, NO-GATE** (a verdict + divergences doc). The app-toolkit spike FINDINGS (`docs/RESEARCH-2026-07-20-gui-app-toolkit-spike-FINDINGS.md`) was authored by **Chad (Codex)**; you (Claude) are the independent second family; the PM (Claude — same family, so your coder review is the real cross-model signal here). You own the **compositor + OT-63 GPU + the own-Vello direction**, so you're the right seat to test whether iced actually plugs into *our* rendering world.

## What to do
Read the FINDINGS + (if useful) skim the throwaway PoC crate on `origin/codex/gui-app-toolkit-spike-chad` (`tools/gui-app-toolkit-spike/`). **Independently** assess the load-bearing claims — try to find where the **hybrid** recommendation is wrong. Deliver a verdict: **RATIFY / RATIFY-AFTER-AMENDMENT / REFUTE**, with the specific load-bearing divergences.

## The claims that carry the decision (pressure-test each)
1. **The PoC feasibility (the load-bearing proof).** Chad reports: a real iced `TextInput`+`Button` laid out + rendered through an **own offscreen RGBA renderer** (own-Vello-shaped), fed by **G1-shaped generation-checked input**, without winit/wgpu/iced_wgpu. Is that PoC genuinely proving "iced_core drives an own backend over our seam," or is the offscreen renderer a stub that sidesteps the hard part (real Vello glyph/vector rendering, damage, the compositor commit)? Does the PoC's input path actually match our `InputFocus`/`InputEventV1` model, or an approximation?
2. **THE CORE QUESTION — hybrid vs own-from-scratch.** The rec is **hybrid**: own the `no_std` runtime/API/a11y seam, *adapt iced's algorithms* (layout, widget tree). You own own-Vello + the G1 compositor — is "adapt iced's algorithms" actually cheaper than own-from-scratch, or does the impedance (our `no_std` userland, our event model, our own-Vello backend, our damage/commit model) mean we rewrite so much that borrowing iced's algorithms adds *coupling cost* (tracking iced's API churn, its taffy/layout assumptions) without real saving? The 12–20 (hybrid) vs 24–40 (clean-room) person-month split — does it hold, or is the hybrid's integration tax under-counted?
3. **The a11y correction.** Chad found iced has **no AccessKit** — so accessibility is project-owned either way. Given the ratified WCAG-AAA floor, does that change the calculus (if we own a11y regardless, how much iced value remains — is it just the layout algo + widget model)?
4. **Fit with own-Vello + G1.** Does iced's rendering abstraction (its `Renderer`/`text::Renderer` traits) actually map onto *our* Vello scene + the SharedPixelBuffer damage/commit model, or is there an impedance you can see from having built the compositor that the spike glossed?
5. **No-contradiction check.** Anything here that fights the ratified own-Vello-compositor / glass / Reduce-Transparency decisions? The compositor is settled — confirm the app-toolkit choice doesn't leak back into it.

## Deliverable + close
- `docs/RESEARCH-2026-07-20-gui-app-toolkit-crosscheck-FINDINGS.md` (CC-BY-4.0): the verdict (§0), per-claim independent assessment + load-bearing divergences + corroborations, confidence-tagged.
- No-gate; push to `Workers\cerebro` on branch `codex/gui-app-toolkit-crosscheck-cerebro`; PM cherry-picks the FINDINGS + folds the verdict into the toolkit decision. RETURN: verdict + 3–5 bullet divergences/corroborations.

## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-gui-app-toolkit-crosscheck-cerebro.md`. The doc under review: `git show origin/main:docs/RESEARCH-2026-07-20-gui-app-toolkit-spike-FINDINGS.md`; the PoC crate is on `origin/codex/gui-app-toolkit-spike-chad` (`tools/gui-app-toolkit-spike/`). Work in `Workers\cerebro`.
- **One-line:** **Cross-model cross-check** (you=Claude, author=Codex/Chad) of the app-toolkit decision (**hybrid: own the no_std runtime/API/a11y seam, adapt iced's algorithms**). You own own-Vello + the G1 compositor, so test whether iced actually plugs into *our* world: is the PoC (iced TextInput+Button via an own offscreen renderer fed by G1-shaped input) proving the real thing or sidestepping the hard part (real Vello/damage/commit)? Is **hybrid actually cheaper than own-from-scratch**, or is the integration tax (iced API churn, layout assumptions, our no_std/event/own-Vello impedance) under-counted in the 12–20 vs 24–40 split? Given iced has no AccessKit (a11y project-owned regardless), how much iced value remains? Any contradiction with the settled own-Vello compositor? Verdict: **RATIFY / RATIFY-AFTER-AMENDMENT / REFUTE** with divergences. Deliverable `docs/RESEARCH-2026-07-20-gui-app-toolkit-crosscheck-FINDINGS.md`.
- **RAG:** `Vello renderer scene text`, `atrium SharedPixelBuffer damage commit`, `InputFocus InputEventV1`, `atrium compositor present`. Health-check first; note-and-fall-back if down.
