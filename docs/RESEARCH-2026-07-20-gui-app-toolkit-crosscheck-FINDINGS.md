<!-- SPDX-License-Identifier: CC-BY-4.0 -->
> **PM SUPERSEDING NOTE (2026-07-21):** Per the standing no-human-labor-time rule (CLAUDE.md), the **person-month figures in this doc (the 12–20 vs 24–40 "pm" split, the A1 amendment's numbers) are deprecated** and must not be carried forward as effort estimates — read them only as relative difficulty signal. The *finding* stands, re-expressed: "hybrid" is honestly **clean-room with iced as an executable reference**, the hybrid-vs-clean-room costs **converge**, and no effort figure is committed until the **decisive second spike** (real glyph → own-Vello → `SharedPixelBuffer` + a derived-and-committed damage rect) measures the two claims this cross-check found unproven ("adapt < own" and "own-Vello plugs into G1").

# RESEARCH: GUI application-toolkit spike — cross-model cross-check

**Date:** 2026-07-20
**Reviewer:** Cerebro (Claude family) — independent second family
**Author under review:** Chad (Codex family), `docs/RESEARCH-2026-07-20-gui-app-toolkit-spike-FINDINGS.md`
**PoC inspected:** `origin/codex/gui-app-toolkit-spike-chad:tools/gui-app-toolkit-spike/{Cargo.toml,src/lib.rs}`
**Status:** Cross-model design review — no gate. This is genuine cross-family signal (Claude reviewing a Codex-authored decision), per the CLAUDE.md convergence-diversity rule.
**Reviewer standing:** I own the G1 compositor, the OT-63 GPU ladder, and the own-Vello direction — so §4 (fit with our rendering world) is assessed from having built the seam iced would have to plug into.

---

## 0. Verdict — **RATIFY-AFTER-AMENDMENT** [High confidence]

The **direction is correct and I corroborate it cross-model:** do **not** adopt `iced_core`/`iced_widget` as shipped (they are `std`, drag a hosted cosmic-text/font graph, and evolve their traits between releases → a permanent maintained fork); **own** the `no_std` cap-native runtime, the widget API, the accessibility semantic tree, and the own-Vello backend; treat iced as **executable prior art** for narrowly-selected algorithms. The a11y correction is right (iced ships no AccessKit; a11y is project-owned regardless). Nothing here contradicts the settled own-Vello/glass compositor. Chad's §5 caveats are honest — he does not overclaim the PoC.

I withhold a clean RATIFY on **three load-bearing amendments**, all about the *economics*, not the direction:

- **A1 — the 12–20 vs 24–40 person-month split over-counts iced's savings.** The expensive parts of a production app floor — text shaping/editing (iced *is* cosmic-text, untakeable in `no_std`), accessibility, the own-Vello backend, and the `no_std`/G1 runtime — are **owned regardless of iced**. The genuine iced reuse is confined to **layout + widget-behavior state machines as a read-only reference.** Re-frame "hybrid" honestly as *"clean-room, with iced as an executable reference for layout + behavior,"* and expect the two estimates to **converge**, not sit 12–20 pm apart.
- **A2 — the PoC proves the *seam*, not the *reuse*.** Every algorithm the hybrid's cost claim rests on is stubbed to a no-op or a fixed-6px monospace approximation (the `Editor` is a literal no-op; shaping/hit-test are `x / 6.0`). So the PoC is **zero evidence** for "adapting iced's algorithms is cheaper than owning them." Gate the hybrid estimate on Chad's own proposed second spike (a `no_std+alloc` layout kernel + **one real** text-editing state machine adapted *without* iced's crate graph) before the number is load-bearing.
- **A3 — the own-Vello ↔ G1 impedance the spike glossed** (my compositor-seat contribution): iced emits **full-scene redraws, not damage**, while our commit path is **damage-region** based; and the **glyph→Vello→SharedPixelBuffer** target (CPU raster into the client RW mapping vs GPU raster + readback, tied to the OT-63 HardwareFlip/scanout path) is the real own-Vello integration question — both untouched by the PoC.

None of these flip the recommendation; they right-size the schedule and name the two seams a commit-decision needs priced first.

---

## 1. Claim 1 — the PoC feasibility (is it proving the real thing?)

**Corroborated on the narrow claim; the load-bearing "reuse" is not exercised.** [High confidence]

The PoC is a **genuine positive** on exactly one thing: iced 0.14's *real* `TextInput`, `Button`, `column`, state tree, and `UserInterface::update/draw` run against a **project-authored `iced_core::Renderer` + `text::Renderer`** with **no winit, no wgpu, no `iced_wgpu`** (verified in `Cargo.toml`: only `iced_core`/`iced_runtime`/`iced_widget` with `advanced`). The generation-checked input translation is **G1-shaped** — `TextByte{byte, generation}` is rejected unless `generation == active focus generation`, which correctly mirrors our `InputFocus` recency gate. That the widget layer belongs **client-side** (client owns the RECEIVE root + RW `SharedPixelBuffer`; compositor holds only ACTIVATE + RO/NX PRESENT) is right and matches the live G1 boundary.

But the **hard parts are all stubbed**, and stubbed precisely where the hybrid's value proposition lives:
- **`StubEditor::perform(_action)` is an empty body** — text *editing* (cursor motion, selection, insert/delete) is not implemented at all; `move_to` is a no-op.
- **`FixedParagraph` is fixed-cell monospace**: `min_bounds = chars().count() * 6.0`, `hit_test = point.x / 6.0`, `grapheme_position = index * 6.0`. It iterates `.chars()` (code points), not grapheme clusters — the exact thing Chad's own §1.2 says a real `TextInput` needs. No shaping, kerning, proportional fonts, or bidi.
- **`fill_quad` handles only `Background::Color`** (gradients dropped); `allocate_image` returns `Unsupported`; `start/end_transformation` are no-ops (any transform renders wrong).
- **Widget focus is seeded by a synthetic mouse click** at (10,10) — the PoC fakes the intra-app widget-focus seed it elsewhere (§3) identifies as an open problem; for a keyboard-only unlock there is no pointer to do this.

**Net:** the PoC proves *"iced's widget/event/layout tree can be driven over our seam"* — real and useful. It does **not** exercise a single one of the algorithms whose reuse justifies "hybrid < clean-room." Chad says as much in §5 ("proves shell and backend replacement, not production typography… accessibility, full G1 input, or `no_std`"), so this is a **corroboration of his honesty** and simultaneously the basis for **A2**: the cost claim has no empirical support yet.

## 2. Claim 2 — hybrid vs own-from-scratch (THE CORE QUESTION)

**The direction survives; the split is optimistic. → the A1 amendment.** [Medium-high confidence]

Decomposing a production app floor by who-owns-what:

| Cost center | Reusable from iced? | Owned regardless |
|---|---|---|
| Flex/row/column **layout** | **Yes — as prior-art reference** (the most portable piece) | — |
| Widget **behavior** state machines (button/focus/text_input cursor logic) | **Partly — behavior as reference**, re-written against our types | — |
| **Text shaping/editing/hit-testing** | **No** — iced delegates to cosmic-text (hosted, `std`, huge); unusable in `no_std` | **Owned** (own shaper: swash/rustybuzz-class, itself a port) |
| **Accessibility** | **No** (iced has none — §4) | **Owned** (WCAG-AAA semantic tree) |
| **Renderer backend** (own-Vello) | **No** (you replace `iced_wgpu`; the `Renderer` trait is a thin shape you satisfy) | **Owned** |
| **Runtime / event loop** | Minimal (`iced_runtime` is `std`) | **Owned** (`no_std`, G1) |

The reusable column is **layout + widget-behavior-as-reference** — real, but the *smaller* fraction. Text + a11y + backend + runtime — the parts that dominate a production floor — are owned either way, and iced buys nothing there. Crucially, "adapt iced's text editing" cannot mean *take iced's code* (that code *is* cosmic-text calls); it means *read `editor.rs` as a reference and reimplement against our shaper* — which is closer to clean-room-with-a-reference than to reuse, and carries the recurring **coupling tax** Chad's §2 names (trait churn 0.13→0.14→0.15).

So the honest framing of "hybrid" — which is exactly what Chad's §7 recommendation *already describes* ("own API/runtime/a11y; **adapt** selected behavior; own-Vello backend") — is **"clean-room with iced as an executable reference."** Under that framing the 12–20 vs 24–40 delta should **narrow** (a good reference is worth maybe a 4–10 pm discount on layout + behavior design, not a 12–20 pm structural saving). This is an **amendment to the economics, not a refutation**: reading a high-quality MIT reference *is* cheaper than pure clean-room, and the no-adopt-crates conclusion is correct. It just should not be sold as a 12–20 pm line-item until A2's spike measures it.

## 3. Claim 3 — the a11y correction

**Strongly corroborated; and it sharpens A1.** [High confidence]

iced shipping no AccessKit is consistent with everything I know of iced's upstream (a11y has been a long-standing open gap). With a **WCAG-AAA floor** and a renderer-independent semantic tree (role/name/value/state/bounds/actions/focus/relationships, cap-authorized, surviving glass/opaque rendering) **owned regardless**, iced's residual value collapses to **layout + widget behavior** — precisely the A1 point. Chad's nuance is right: AccessKit's *vocabulary/types* may be adopted where a dependency audit permits, and a *hosted* bridge could emit AccessKit nodes, but AccessKit must **not** define the kernel/G1 ABI (its platform adapters target hosted OS a11y APIs we don't have). This is one of the strongest parts of the spike.

## 4. Claim 4 — fit with own-Vello + G1 (my home turf)

**Trait mapping credible; two real impedances the spike glossed. → the A3 amendment.** [High confidence]

The `iced_core::Renderer` shape (`start_layer`/`end_layer`/`fill_quad`/`start_transformation`) **does** map onto a Vello scene (layers→clip layers, quad→fill path, transform→scene transform) — structurally credible, I agree. But from having built the compositor, two seams are unaddressed:

1. **Damage vs full-scene redraw.** `UserInterface::draw` re-emits the **entire** scene each frame; iced 0.14 has redraw-request/`mouse_interaction` signals but **no fine-grained damage output**. Our commit path is **damage-region** based — the `atrium_interactive` capstone commits *exactly one 8×16 cell and the compositor presents only that damage region*. Bridging "iced redraws everything" to "commit minimal damage" needs a **damage-derivation seam** (buffer diff, or app-owned dirty-rect tracking) that **iced does not give you**. The PoC sidesteps this entirely — it fills a full 240×100 buffer and asserts "some pixel is non-zero."
2. **Glyph→Vello→SharedPixelBuffer target.** ADR-024 settles the **compositor** as own-Vello **on wgpu (GPU; glass is GPU-gated, needs OT-63).** The app renders into its **own** client-RW `SharedPixelBuffer` (CPU RGBA, compositor sees it RO+NX). So the app-toolkit's own-Vello either (a) **CPU-rasters** directly into the RW mapping, or (b) **GPU-rasters + reads back** (or shares a GPU texture) — the latter lands squarely in the OT-63 HardwareFlip/scanout-DMA-fence path Janus is building. The PoC's `fill_paragraph` paints monospace blocks; the *real* glyph-run→Vello→buffer path (and its CPU-vs-GPU target) is **the** own-Vello integration question and is untouched. Price this before the backend estimate is trusted.

## 5. Claim 5 — no-contradiction with the settled compositor

**Corroborated — and better than "no contradiction": Chad's conclusion satisfies a criterion ADR-024 pre-set.** [High confidence]

ADR-024 already ruled the **compositor** own-project on Vello/wgpu (NOT iced) and **explicitly deferred the app toolkit**, instructing: *"adopt iced pieces only if they emit our scene/accessibility primitives without winit/Wayland ownership or a permanent fork."* Chad's finding — that `iced_core`/`iced_widget` are `std` + cosmic-text-welded + trait-churning, i.e. **a permanent fork** — **directly triggers the ADR-024 no-adopt branch.** So the recommendation is not merely non-contradictory; it is the pre-registered decision resolving as designed. The client-pixels-into-SharedPixelBuffer / compositor-owns-PRESENT separation is clean and matches G1. The **one boundary to specify** (not a contradiction, a contract): the app-toolkit's damage output must feed the compositor's damage-region commit (the A3(1) seam).

---

## 6. Corroborations & divergences (summary)

**Corroborations (cross-model agreement — Claude ← Codex):**
- No-go on adopting `iced_core`/`iced_widget` as shipped (`std`, cosmic-text graph, trait drift → maintained fork). [High]
- a11y is project-owned regardless; iced ships no AccessKit; own the semantic tree, AccessKit as vocabulary/bridge only. [High]
- The own-seam direction (own `no_std` runtime/API/a11y + own-Vello backend, iced as prior art) does not contradict — and in fact resolves — the settled ADR-024 compositor/app-toolkit split. [High]
- The PoC honestly proves shell+backend replacement over a G1-shaped, generation-checked input feed. [High]

**Divergences (load-bearing amendments):**
- **A1:** the 12–20 vs 24–40 pm split over-counts iced savings; text/a11y/backend/runtime are owned regardless, so re-frame "hybrid" as "clean-room with iced as executable reference" and expect the estimates to converge. [Medium-high]
- **A2:** the PoC is zero evidence for the *cost* claim — every expensive algorithm (editor/shaper/hit-test/transforms/images) is a stub; gate the estimate on the proposed second spike (real layout kernel + one real editing state machine, no iced graph). [High]
- **A3:** name and price the two own-Vello/G1 seams the spike glossed — (i) iced's full-scene redraw vs our damage-region commit (a damage-derivation seam iced doesn't provide); (ii) the glyph→Vello→SharedPixelBuffer CPU-raster-vs-GPU-readback target, tied to the OT-63 HardwareFlip path. [High]

## 7. Recommended next step (endorsing Chad's, with a precondition)

Adopt Chad's proposed decisive spike **as the precondition to committing any hybrid person-month number**: define the Altimit semantic-node ABI + a `no_std+alloc` flex/row/column layout kernel, and adapt **one** text-editing state machine **without** pulling iced's crate graph. Add **two** measurements A1/A3 require: (a) rasterize that one widget's text through the **real** own-Vello glyph path into a `SharedPixelBuffer` (CPU-raster target first, since it's the cap-native floor), and (b) derive a **damage rect** from that draw and commit only it. Those two are what actually test "adapt < own" and "own-Vello plugs into G1" — the two claims this cross-check found unproven.
