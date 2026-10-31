<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Atrium glass amendment — Codex cross-check FINDINGS (Janus)

**Date:** 2026-07-19

**Author:** Janus (Codex; third-family cross-check)

**Dispatch:** `Plans/DISPATCH-2026-07-19-glass-amendment-crosscheck-janus.md`

**Status:** FINDINGS — independent design/architecture review; docs-only, no gate

**Branch:** `codex/glass-crosscheck-janus`

## 0. Verdict in one breath

Ratify the **direction**, after amendment. A compositor-owned retained scene plus a compositor-owned Vello/wgpu blur pipeline is the right load-bearing architecture; do not put iced in that layer. However, keep a deliberately narrow seam for permissively licensed widget machinery in applications, because rebuilding layout, focus, keyboard navigation, text editing, accessibility semantics, and controls is a different and much larger job than building the compositor scene. The browser-sharing claim is useful but narrower than stated.

Trust expressed through material is strong, but **frost density is not a sufficient hue-independent redundant channel**. It is backdrop-dependent, not reliably ordinal, and disappears in the solid fallback. Preserve the existing Cap Halo's pattern/shape/edge/label vocabulary in every rendering mode; let tint and frost reinforce it. “WCAG AAA” constrains content contrast, not whether trust states are mutually distinguishable.

Reduce Transparency is a sound invariant if it means **identical semantics and geometry through a no-translucency render path**, not merely “replace glass with a solid trust color.” Glass, ≥70% negative space, and neon-as-meaning compose only if glass is sparse and neon is crisp foreground information. Pervasive frost is occupied visual texture, not negative space.

## 1. Method and confidence

I first formed the position above from the Atrium design language, the G1 compositor constraints, and the actual semantic requirements of backdrop filtering, state differentiation, and fallback rendering. I then pressure-tested it against the Qwen research and Claude amendment.

| Finding | Confidence | Limitation / proof still needed |
|---|---|---|
| Compositor owns retained scene and backdrop-filter scheduling | **High** | Architecture follows scene ordering and cap-native protocol boundaries |
| Vello + custom wgpu passes is the best current renderer candidate | **Medium-high** | Require a Gen9 spike; Vello does not by itself prove backdrop capture/blur integration or browser completeness |
| iced should not own compositor surfaces, but may save substantial app-toolkit work | **High** | Exact reusable subset depends on iced version/API and a no-winit integration spike |
| Frost density alone is not a safe redundant trust channel | **High** | Human testing should tune the complete redundant vocabulary |
| Solid trust-color substitution does not itself establish AAA or state distinguishability | **High** | Token pairs must be measured in all themes/states |
| Sparse glass can compose with Atrium calm; pervasive glass cannot | **Medium-high** | A motion/static mockup on target displays should validate the visual budget |

Licensing boundary: only permissive/MPL-compatible code may be adapted. iced (MIT), egui (MIT/Apache), Vello (MIT), and wgpu (MIT/Apache) are eligible subject to dependency audit. GPL projects remain reference-only; no code is proposed for copying from them.

## 2. Scope A — renderer

### 2.1 Independent position

There are three different systems that should not be collapsed into “one renderer”:

1. The **compositor scene** knows ordered cap-surfaces, damage, occlusion, backdrop dependencies, trust material, and present timing.
2. An **application widget system** knows layout, focus traversal, pointer/keyboard behavior, text editing, accessibility semantics, controls, and state/update conventions.
3. The **browser engine** knows CSS layout and stacking contexts, glyphs, images, clips, filters, scrolling, and a large display-list vocabulary.

The compositor must own (1). Backdrop blur is an order-dependent scene operation: render the backdrop region, downsample/cache it, filter it, then composite the surface and its sharp foreground. That belongs at the compositor's render-graph boundary, not inside an ordinary widget.

Vello is a strong candidate for vector/text scene production, with explicit wgpu render/compute passes around it. But “Vello has compute shaders” does not make backdrop blur native automatically. The build must prove texture handoff, pass ordering, partial damage, overlapping glass, cache invalidation, device loss, and the solid path on Gen9. Treat **Vello plus a project-owned render graph**, rather than Vello alone, as the decision.

### 2.2 Is there a real reason to adopt iced?

Yes, but **not as the compositor architecture**. The real reason is the cost of the application widget layer: layout, control behavior, focus, keyboard navigation, text input/editing, event capture/bubbling equivalents, async messages, testability, and eventually accessibility-tree production. Rebuilding those correctly can dominate the cost of the scene graph.

The amendment's anti-iced reasoning needs correction:

- iced itself is not intrinsically a Wayland compositor. Its common desktop shell uses winit and platform windowing, while its core/widget/renderer abstractions are separable to some degree. The relevant risk is **host-runtime and renderer integration cost**, not a blanket “Wayland assumption.” libcosmic/cosmic-comp integration should not be imputed wholesale to iced.
- A compositor-level backdrop filter does not require every widget to understand the backdrop. An application can submit opaque/transparent content and semantic regions while the compositor applies authorized material behind the surface. What fails is asking an ordinary iced custom widget to capture arbitrary compositor content behind its client surface.
- Conversely, adapting or forking iced deeply enough to own cap-surfaces, the compositor render graph, and glass scheduling would inherit upstream abstractions while losing much of upstream compatibility. That is not worth it.

**Recommendation:** build the cap-native compositor scene and material pipeline in-project using Vello/wgpu; do not adapt iced as its surface/composition model. In parallel, preserve a backend-neutral application UI seam and run a bounded spike against iced's permissive core/widget/layout/event pieces. Adopt those pieces only if they can emit project scene/display primitives and accessibility semantics without winit/Wayland ownership or a permanent fork. egui remains reasonable for diagnostics/neo-monitor, not the system semantic UI.

This is a qualified divergence from “not iced”: **not iced in the compositor, but do not pre-commit to an own widget toolkit.**

### 2.3 Cost and browser-sharing correction

An own compositor scene is bounded: surface nodes, transforms/clips, z-order, damage, backdrop dependencies, presentation, and trust material. An own production widget toolkit is not bounded that way. The amendment should name the former, not silently commit to the latter.

Browser sharing is real at the lower layers:

- wgpu device/queue ownership and recovery;
- text shaping/raster/vector paths where requirements align;
- clips, transforms, paths, images, color management, damage vocabulary;
- profiling and Gen9 backend knowledge.

It is not literally one retained scene for both. Browser layout, stacking contexts, scrolling, filters, raster images, and isolation rules remain browser-specific; compositor cap-surfaces remain security/presentation-specific. Promise a **shared GPU/rendering substrate and primitive vocabulary**, not one renderer implementation serving both without adapters.

### 2.4 Required renderer changes before ratification

1. Replace “iced assumes a Wayland compositor” with the narrower integration analysis above.
2. Define “own retained-mode scene” as compositor scene/material scheduling, not an automatic own-every-widget commitment.
3. Reword “one renderer for compositor and browser” to shared Vello/wgpu substrate/primitives, with distinct scene producers.
4. Gate final Vello adoption on a Gen9 spike proving: nested/overlapping glass, damage/caching, foreground sharpness, device-loss → solid transition, and frame budget.
5. Keep GPL implementations reference-only and record a dependency/license audit for any adapted permissive crate.

## 3. Scope B — trust state as glass tint

### 3.1 What works

The concept is coherent with the Atrium: authority is not a badge added after the fact; it changes how a surface inhabits the room. Clearer/calm material for held authority and more visually distant treatment for untrusted content can be learned quickly. Tether-as-leash and Undo Shadow are especially strong because their geometry encodes an action, not merely a mood.

The proposed material can therefore be the beautiful reinforcement of trust state. It cannot be the only carrier.

### 3.2 Where the five-year-old and colorblind claims overreach

“Clear + blue = yours; gray + opaque = stranger” distinguishes the endpoints under a controlled backdrop. It does not reliably distinguish the full state set:

- **owned vs tethered:** the table makes tethered lighter/clearer than owned, so frost density is not a monotonic trust scale;
- **denied vs untrusted:** both are heavily frosted and can converge when hue is weak or absent;
- **undo-armed:** it is an orthogonal transaction condition, not a point on the trust gradient, and can coexist with another authority state;
- blur radius, opacity, saturation, and backdrop detail are perceptually entangled; a busy backdrop can make “light” frost look denser than “heavy” frost over a quiet backdrop;
- reduced vision, display quality, motion, and small surface area further weaken blur-density discrimination.

Frost density is hue-independent in implementation but not a sufficiently stable redundant code in perception. WCAG AAA contrast also does **not** certify that blue, magenta, and gray states are distinguishable; it covers specified foreground/background contrast (with separate non-text and no-color-alone concerns relevant here).

### 3.3 Required redundant vocabulary

Keep the already-ratified Cap Halo vocabulary across bitmap, solid, and glass:

| State axis | Hue/material reinforcement | Mandatory hue-independent cue |
|---|---|---|
| owned | light blue vibrancy | solid border + owned corner ticks; stable silhouette |
| tethered | provisional/light glass | dashed border + physical tether to grant owner; revoke affordance |
| denied | magenta edge + heavier frost | interruption/left-edge geometry + grant-required glyph/chip; not activatable |
| untrusted | desaturated/heaviest frost | stipple/hatch or distinct arm's-length boundary + sandbox glyph; not activatable |
| undo-armed | amber optional | Undo Shadow/depth band; layer this over the underlying authority state |

Labels remain present for precise reading. The five-year-old test should be stated as: **shape/pattern/connection differentiates the state with labels blurred and color removed; tint/frost makes it faster and more beautiful.** Do not require users to infer security from blur alone.

### 3.4 Solid fallback

The fallback survives only if the redundant geometry survives. “Solid trust-state color” is underspecified and, for owned, conflicts with the design language's rule that Electric Blue is light/accent, never a body-copy fill. A token named `#3c8ef4` cannot be assumed to support AAA text merely because it means trust.

Use a stable opaque neutral/near-Void surface chosen for content contrast, then retain state through measured border/accent tokens, patterns, glyphs, tether, silhouette, and labels. Each actual foreground/background token pair must be tested; AAA is a property of a pair at a size/weight, not of a state color. High-contrast mode may further reduce the palette while preserving the same geometry.

This is the largest divergence from the amendment: **frost density is supporting atmosphere, not the accessibility redundancy, and solid fill color is not the fallback semantics.**

## 4. Scope C — invariant and visual reconciliation

### 4.1 Reduce Transparency holds, with one distinction

One no-translucency rendering path is an excellent accessibility, performance, early-boot, and GPU-failure floor. It should preserve scene geometry, content, controls, trust cues, focus, and hit testing while removing capture/blur/translucent blending. No necessary information may be encoded only in what disappears.

Do not conflate the **user preference** with the **runtime cause**. “Reduce Transparency” is user-controlled and sticky. GPU absence, thermal/performance policy, or device loss may automatically select the same solid renderer, but must not mutate or masquerade as the user's accessibility setting. Expose the active rendering mode/reason for diagnostics. A GPU fault transition must also avoid showing stale captured pixels.

The invariant should therefore be phrased as:

> Every semantic scene has a deterministic opaque rendering with identical geometry, controls, trust-state cues, and content contrast. User preference or renderer capability may select it; selection never changes semantics.

### 4.2 Glass + ≥70% negative space + neon

They compose under a budget, not by assertion:

- **Negative space must remain near-Void and materially quiet.** A large frosted pane has texture, contrast variation, and an edge; it is occupied space even when it contains no widgets. Do not count it toward the ≥70% merely because it is translucent.
- **Glass belongs to sparse semantic surfaces**, especially the active intent/Threshold surface and bounded trust-bearing objects. Persistent chrome and the empty field remain quiet/opaque. This also resolves tension with the earlier rule that only the intent-holding surface gets the hero material.
- **Neon meaning should be drawn sharp above the filtered backdrop** as edge/tick/tether/focus information. “Bleeding through the glass” can be ambient reinforcement, but must not be the sole semantic mark: blur literally muddies spatial precision and can turn a restrained accent into a decorative wash.
- Limit simultaneous blur layers and overlapping translucent panels. Nested cards remain forbidden; overlapping glass creates both visual milkiness and expensive backdrop dependencies.

With those constraints, restrained glass makes the few active/trust-bearing surfaces feel special against calm Void, while neon remains scarce and meaningful. Without them, pervasive frost fights both calm and semantic neon.

### 4.3 Tahoe distinction

“Material, not enforced shape” is a valid distinction but not a structural guarantee by itself. A universal glossy treatment can homogenize even varied silhouettes. Enforce the existing per-kind silhouette/accent/material-signature test and add a material-budget rule: a surface does not receive glass merely because the renderer supports it. Glass is conditional on semantic role, visual-rest budget, and fallback equivalence.

## 5. Convergence and divergence against Qwen + Claude

| Question | Result | Reason |
|---|---|---|
| Compositor-owned retained scene | **Agree** | Backdrop filtering and cap-surface ordering belong at the compositor boundary |
| Vello/wgpu direction | **Agree, conditional** | Best abstraction candidate, but custom render graph + Gen9 proof are still required |
| “Not iced” | **Diverge in scope** | Correct for compositor ownership; premature for application widget/layout/a11y machinery; iced is not inherently Wayland-only |
| One compositor/browser renderer | **Diverge in strength** | Share substrate/primitives, not scene model or full renderer without browser-specific adapters |
| Trust-state expressed in glass | **Agree** | Strong aesthetic reinforcement of capability state |
| Frost density is sufficient color-independent redundancy | **Diverge** | Backdrop-dependent, non-monotonic across proposed states, disappears in opaque mode |
| Solid trust-state color preserves meaning/AAA | **Diverge** | Geometry/patterns must carry state; contrast must be measured per token pair; blue-as-fill conflicts with current grammar |
| Mandatory opaque fallback | **Agree, refine** | One semantic-equivalent opaque path; distinguish user preference from automatic degraded-mode selection |
| Glass breaks nothing in Atrium grammar | **Diverge from unconditional claim** | It composes only with sparse glass, quiet true negative space, sharp foreground neon, and a material budget |

## 6. Ratification recommendation

**Ratify after required edits**, not as currently worded:

1. Ratify glass as a selective trust-bearing material, not the pervasive substrate of all calm/negative space.
2. Make Cap Halo geometry/pattern/connection/label the mandatory hue-independent trust channel in glass and solid modes; frost/tint are reinforcement.
3. Specify the opaque semantic-equivalence invariant and measured token-pair contrast; remove the implication that “solid trust color” alone establishes AAA.
4. Ratify compositor-owned retained scene + project-owned Vello/wgpu render graph, conditional on the Gen9 blur/degradation spike.
5. Reserve the application widget-layer decision; test permissive iced pieces behind the cap-native/runtime boundary instead of committing either to iced wholesale or to rebuilding every widget.
6. Narrow browser leverage to a shared GPU/rendering substrate and primitive vocabulary.
7. Add a glass/material budget: near-Void is the counted negative space; semantic neon is sharp foreground; overlapping/nested frost is exceptional and measured.

These changes preserve the amendment's best move — security made materially beautiful — while ensuring the security meaning survives color loss, blur loss, GPU loss, and the ordinary complexity of real scenes.
