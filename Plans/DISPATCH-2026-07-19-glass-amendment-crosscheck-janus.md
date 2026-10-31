<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Glass amendment — Codex cross-check (cross-model review) → Janus

**Date:** 2026-07-19
**From:** PM (Claude)
**To:** Janus (Codex frontier family — the third-family lens)
**Status:** DISPATCHED — a **cross-model design cross-check** (doc output, NO gate). ⚠️ **UPDATE (2026-07-19): the PL RATIFIED the glass DIRECTION** (glass-is-the-material + trust-state-as-tint + the Reduce-Transparency invariant + the reconciliation — ADR-024 Amendment 2026-07-19). So this cross-check's job **narrows to REFINING within the ratified direction, not re-deciding it:** **Scope A (the renderer — own-scene+Vello vs adapt-iced) is FULLY OPEN and the most valuable output** (settle it); **Scope B is now "refine the palette + prove colorblind-safety (frost-density as the hue-independent channel), and catch any blind spot," NOT "should we do trust-tint at all"** (that's settled); **Scope C is a sanity check on the ratified reconciliation.** Do NOT spend effort arguing against glass/trust-tint/Reduce-Transparency — argue about the *details and the renderer*. A genuine divergence that reveals a real flaw in the ratified direction is still worth flagging (the PL can re-open), but the bar for that is high now.

## The one discipline that makes this worth doing
**Form your OWN independent position FIRST, then compare.** Read our conclusions as *one input to pressure-test*, not as the answer. Where you'd have designed it differently, say so with reasons — that divergence is the whole value. Where you independently arrive at the same place, say that too (and why) — that's the convergence signal the PL will weight. Rubber-stamping wastes the cross-check; a same-family echo is exactly what this exists to avoid.

## Read first
- `git fetch origin && git show origin/main:docs/THINK-2026-07-19-atrium-glass-frost-amendment.md` (the PM amendment — the thing under review)
- `git show origin/main:docs/RESEARCH-2026-07-19-gui-prior-art-glass-frost-FINDINGS.md` (Prometheus's research it's built on)
- Context: `docs/THINK-2026-07-10-atrium-design-language.md`, `docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md`, `docs/ADR-024-phase-8-design-system-and-tech-stack.md`

## Scope A — the renderer decision (the load-bearing TECHNICAL one)
Prometheus explicitly flagged this for you: **"is there any reason to adapt iced's widget layer despite the glass gap?"** Our position: **own retained-mode scene graph + a Vello/wgpu backdrop-blur compute pass — NOT iced** (its Elm-style widget model assumes a Wayland compositor + fights per-surface backdrop blur; and one own-renderer serves the compositor AND the browser = shared OT-63 leverage). Pressure-test it independently:
- Is own-scene+Vello genuinely right, or is there a concrete reason to **adapt a permissive toolkit** (iced MIT; egui MIT/Apache — NOT Slint GPL, NOT cosmic-comp GPL) — e.g. does iced's widget/layout/event machinery save enough to be worth the glass-integration + Wayland-assumption fight?
- What does "own retained-mode scene" actually **cost** to build vs adopt — and does the browser-sharing argument hold (one renderer for compositor + browser), or are their needs different enough that the sharing is illusory?
- License-aware throughout (adapt = permissive only; GPL = reference-only, never copied into our MPL tree). Give a clear recommendation + reasons.

## Scope B — the trust-state-as-tint aesthetic (§2 — the PL's ratification call)
Independently assess *before* reading our palette: **does expressing cap-surface trust-state as the glass's tint/vibrancy/frost-density actually work?** Then pressure-test ours (owned=clear+blue / tethered=leash / denied=heavy+magenta / untrusted=heaviest+gray):
- **Legibility + the five-year-old test:** is "clear+blue = yours, gray+opaque = stranger" genuinely pre-verbally readable, or does it need a redundant non-material cue?
- **Colorblind-safety (ties to our WCAG-AAA commitment):** the palette leans on blue/magenta/gray hues — trust MUST NOT rely on hue alone. Is **frost-density** (clear→heavy) a sufficient *redundant, hue-independent* channel so a colorblind user reads the same trust gradient? If not, what fixes it?
- **Does it survive the solid fallback (§3)?** In Reduce-Transparency mode the glass becomes SOLID trust-colors — are those solids still legible AND distinguishable from each other at the WCAG-AAA floor? (The material's meaning must survive losing the blur.)
- Any blind spot in "security is the beautiful part" — a state that reads wrong, an ambiguity between two states, an aesthetic that fights itself?

## Scope C — the invariant + the reconciliation claims
- **Reduce-Transparency invariant (§3):** does it hold as *the* rule (a11y floor + perf floor + GPU-degraded fallback, one kill-switch)? Any case where degrade-to-solid loses necessary information?
- **The reconciliation (§5):** we claim glass breaks NOTHING in the Atrium grammar (Undo-Shadow-only-drop-shadow, neon-as-meaning, ≥70% negative space, grammar-not-uniformity, classic-Mac craft, the Threshold). Pressure-test the one most likely to be glossed: **do frosted glass + ≥70%-negative-space + neon-as-meaning actually COMPOSE visually, or fight** (e.g. does pervasive frost eat the negative space's calm, or muddy the neon)? Is the "NOT Tahoe enforced-sameness" line real, or a comforting distinction?

## Deliverable + close
- A cross-check FINDINGS doc: `docs/RESEARCH-2026-07-19-glass-amendment-codex-crosscheck-FINDINGS.md` — your independent position per scope, then agree/diverge-with-reasons vs ours, a clear **renderer recommendation** (Scope A), and any **required changes** to the amendment before ratification. Honest confidence flags.
- **Push to a FEATURE BRANCH (`codex/glass-crosscheck-janus` or `janus/…`) — NOT `main`** (coders never push to `main`; only the PM does). RETURN names the branch + your load-bearing conclusions (esp. where you DIVERGED). The PM merges it and folds it into the amendment's PL ratification.
- **No gate** (doc/`docs` only). MPL hygiene: reference-only, never copy GPL/copyleft; you may reference iced/egui (permissive) freely for the renderer comparison.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-19-glass-amendment-crosscheck-janus.md` (+ the amendment + FINDINGS + Atrium design language it references). PM artifacts live on `origin/main`; `git show`, not a local open.
- **One-line:** A **Codex third-family cross-check** of the glass GUI direction (currently Qwen+Claude — form your OWN view FIRST, then agree/diverge with reasons; convergence = signal, divergence = the blind spot we need). **(A)** the renderer: pressure-test own-retained-scene+Vello/wgpu-blur vs adapt-iced (permissive only; GPL reference-only) — is there a real reason to adopt iced despite the glass gap + the Wayland assumption? recommend. **(B)** trust-state-as-glass-tint: is it pre-verbally legible, **colorblind-safe via frost-density as a hue-independent redundant channel** (WCAG-AAA), and does it survive the solid fallback? **(C)** does the Reduce-Transparency invariant hold, and do frosted-glass + ≥70%-negative-space + neon-as-meaning actually compose or fight? Deliverable = a cross-check FINDINGS doc on a **feature branch (NOT main)** + a RETURN naming the branch + where you diverged; the PM merges + folds into ratification.
- **RAG:** available for our design docs — health-check `Invoke-RestMethod http://127.0.0.1:8404/health`; e.g. `atrium material trust state`, `design-system negative space electric blue`, `glass frost vibrancy`. If down, note it + grep/Read (never a blocker; the PL owns RAG startup).
