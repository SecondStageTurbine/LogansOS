<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK / PROPOSED AMENDMENT: The Atrium in glass — frosted-glass material + trust-state-as-tint

**Date:** 2026-07-19
**Author:** PM (Claude)
**Status:** ✅ **RATIFIED by PL (2026-07-19)** — the glass direction is the design of record; folded into ADR-024 (Amendment 2026-07-19). Ratified = §1 (glass is the material), §2 (trust-state-as-tint approach), §3 (the Reduce-Transparency invariant), §5 (the reconciliation). **Janus Codex cross-check COMPLETE + FOLDED IN** (`docs/RESEARCH-2026-07-19-glass-amendment-codex-crosscheck-FINDINGS.md`, `708ceaaa`; three-family validated — Qwen researched → Claude drafted → Codex reviewed). Verdict: *ratify the direction, after amendment.* Refinements now normative in ADR-024 Amendment 2026-07-19: **(§2) trust redundancy** — frost-density is insufficient alone; add Cap-Halo geometry/pattern/tether/glyph/label as hue-independent, mode-surviving cues (no state relies on hue); **(§3) degraded path** = contrast-tested opaque *neutral* surface + persistent semantic cues (not solid-trust-color alone), and user-preference vs auto-GPU-degrade stay distinct causes; **(§4) renderer** — compositor scene+glass = own-project Vello/wgpu (settled, not iced); the app widget toolkit is NOT pre-committed (backend-neutral seam + a bounded spike of iced's permissive pieces); browser = shared substrate, not one renderer; **(§5) glass is SELECTIVE** (true negative space near the Void, sharp neon above the blur). Two of these correct over-claims in this doc's first draft — the cross-model rule earning its keep.
**Inputs:** the PL aesthetic steer ("a beautiful Mac-like glass/frost GUI", 2026-07-19); `docs/RESEARCH-2026-07-19-gui-prior-art-glass-frost-FINDINGS.md` (Prometheus); the ratified Atrium design language + ADR-024; OT-63 (the GPU it needs).

## 0. One breath
The Atrium already says a cap-surface's **material carries its trust/grant state**. This amendment makes that material **frosted glass**, and makes the **trust-state the glass's tint, vibrancy, and frost-density** — so you literally *read trust in the material*. It reconciles cleanly with everything already ratified (nothing in the Atrium grammar breaks), and it comes with one hard new invariant (§3, Reduce-Transparency) that is simultaneously the accessibility floor, the low-end-hardware floor, and the kill-switch.

## 1. The material — frosted glass IS the Atrium's substrate
The cap-surface material (owned / tethered / denied / untrusted / undo-armed — the Atrium's stable state vocabulary across bitmap→T0→GPU) is rendered as **frosted glass**: a translucent surface that captures and blurs the backdrop behind it (the Mac `NSVisualEffectView` idiom — capture → downsample → blur → vibrancy tint → composite). This is the classic-Mac craft material at its peak, and it is *ours* because the surface behind the glass is a cap-gated region, not another app's snoopable pixels.

## 2. Trust-state AS the glass tint (the aesthetic — PL ratifies)
The trust-state is expressed *in the glass itself* — **security becomes the beautiful part** (the Atrium's founding move, now in a material):

| Cap-surface state | Glass expression |
|---|---|
| **owned** (yours, trusted) | **clear/light frost + Electric-Blue vibrancy (~10%)** — calm, luminous, at-rest; neon-as-*meaning* bleeding through the glass |
| **tethered** (granted, revocable — OT-19) | light frost + the visible tether accent; **revoke = cut the tether** (the material shows the leash) |
| **undo-armed** (OT-19 reversible action pending) | the **Undo Shadow** — the one drop shadow; *depth means reversibility* |
| **denied** (a cap refused) | **heavy frost + a magenta edge** — the denial *points at the grant*, never a modal (Article-6.6 security-as-legible) |
| **untrusted** (unverified principal / OT-22) | **heaviest frost + desaturated gray** — trust you can *see*; the more opaque-and-gray, the less trusted |

The gradient is legible pre-verbally (the five-year-old test): *clear+blue = yours; gray+opaque = stranger.* This is §4 of Prometheus's FINDINGS (medium-high confidence, flagged as the PL's aesthetic call).

## 3. The Reduce-Transparency invariant (the load-bearing new rule — NOT optional)
**Frost is the material; legibility is the invariant.** Translucency fights contrast, so:
- A **mandatory "Reduce Transparency" mode**: the frosted glass degrades to a **solid trust-state color** (owned = a solid trust-blue panel, untrusted = a solid gray panel, etc.) — the trust-state reading is *preserved*, the blur is gone, and the **WCAG-AAA contrast floor is met**.
- The contrast floor **wins over the material, always** — a glass surface that can't meet AAA at its content falls back to solid for that content.
- This one switch does **three jobs**: (a) the **accessibility** invariant (the ratified WCAG-AAA floor), (b) the **low-end-hardware / CPU-only performance floor** (real-time backdrop blur needs the GPU — see §4; no-GPU or Rung-0 boots render solid), (c) the **OT-63-degraded fallback** (a GPU hang degrades glass→solid, never a stall).
- This is our **kill-switch discipline applied to the aesthetic**: the beautiful glass is provably switch-off-able to a legible solid, bit-for-bit deterministic.

## 4. What this costs — glass needs the GPU (the honest dependency)
Real-time backdrop blur is a per-frame GPU workload (~480M memory-ops/sec for one 1080p glass surface on CPU — pins a core; viable on the Gen9 iGPU only with 4-8× downsampling). **So the frosted-glass *material* is gated on OT-63 (the GPU accel path)** — which is why OT-63 is now a hard GUI prerequisite, and why the Gen9 bring-up (Rung 1, dispatched) runs parallel to the compositor, with the **backdrop-blur compute pass a named OT-63 deliverable** (the Vello-on-compute / Rung-2b spike). The §3 solid fallback is the always-available floor when the GPU isn't there yet or is off.

**Renderer:** own retained-mode scene graph + a **Vello/wgpu backdrop-blur compute pass** (per the FINDINGS + the already-ratified Vello text-engine verdict) — **not** iced (its widget model fights glass); the same renderer serves the compositor *and* the browser (shared OT-63 leverage). *(The own-vs-adapt-iced question gets the Codex cross-check before the G1+ build.)*

## 5. Reconciliation — nothing in the Atrium grammar breaks (confirm)
- **"The only *drop* shadow is the Undo Shadow"** — SURVIVES. Glass depth is via blur/translucency/vibrancy, not drop shadows; the Undo Shadow stays the single drop shadow (depth-means-reversibility, §2).
- **Neon-as-meaning / Electric-Blue-as-light** — SURVIVES + strengthens: Electric Blue is the *owned* vibrancy that bleeds through the frost = meaning, never decoration.
- **≥70% negative space at rest / calm-at-rest, dense-on-demand** — SURVIVES: glass is the material of the calm negative space; frost is quiet at rest.
- **Grammar-not-uniformity + the five-year-old visual test** — SURVIVES, and is the guardrail: the glass *material* is shared grammar; **per-thing distinct silhouettes/accents are still mandated.** This is explicitly **NOT** the macOS Tahoe/"Liquid-Glass" failure — Tahoe's mistake was *enforced sameness* (every icon into one rounded-rect), which we forbid. Glass material ≠ homogenized shapes.
- **Classic-Mac craft bar + the §12 "Threshold" resting state** — SURVIVES: frosted vibrancy *is* peak classic-Mac craft; the Threshold (intelligence as ambient light/weather, Arthur felt-not-avatared) renders naturally as light through glass.

## 6. What the PL ratifies
1. **§1-2 — glass as the material + trust-state-as-tint** (the aesthetic call; the tint palette in §2 is a starting proposal, tunable).
2. **§3 — the Reduce-Transparency invariant** (recommend: yes — it's the a11y floor, the perf floor, and the kill-switch in one; not really optional).
3. **§4 — glass is GPU-gated** (already implied by ratifying the aesthetic; noted for the sequencing).
On ratification this folds into ADR-024 as the Atrium material amendment; the Codex cross-check (§2 aesthetic + the renderer) runs first per the cross-model rule.
