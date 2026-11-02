<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DECISION: the first static page uses option A (J.c., 2026-11-02)

**J.c.:** "Go with A."

**The question** (`docs/THINK-2026-11-01-ot61-first-page-layout.md`, PR #48): Servo's `servo-layout` 0.7.0 requires
`servo-script` and through it SpiderMonkey (`mozjs_sys`); 19 layout files import script's DOM wrapper types. A static
page (HTML + CSS, no scripts) can't be laid out by unmodified Servo layout without the JavaScript engine.

**Decided: option A.** Patch `servo-layout` so its `script::layout_dom` imports go through a crate-local alias module
(or a generic over the layout DOM bundle). Upstream's script bundle stays the default; on our target a static host DOM
implements the bundle. This puts a static page on screen before SpiderMonkey (C3), through rungs L0-L4 of the THINK.

**The conditions that keep it a seam, not a fork (they are part of the decision):**
1. **Only types move.** No layout algorithm, box construction, or style semantics changes. The patch is reviewed as a
   diff of `use` lines plus the alias module, and a non-neoplutarch build of the patched crate is byte-identical in
   behaviour (the kill-switch).
2. **It converges back.** When C3 lands, our target can name `servo-script`'s bundle again; the patch then only serves
   the static-page host, and goes away if upstream makes layout generic. Offer it upstream.
3. **Cross-family review of the patch** (a Codex reviewer) checks condition 1 line by line before L0 merges.

**ADR-078 is amended, narrowly:** its "never fork layout" rule stands for layout *semantics*; a type-only seam under
the three conditions above is allowed for this one purpose. Options B (wait for C3) and C (Blitz, a second engine)
were declined; D (our own layout) was already rejected.

Fonts (L1) and the WebRender plumbing (#22, #25-#28) proceed regardless.
