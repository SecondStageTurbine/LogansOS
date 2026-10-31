<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-45 — the Atrium visual design language (the "beautiful" made concrete)

**Date:** 2026-07-10
**Author:** Cerebro (main-tier coder)
**Source:** Plans/DISPATCH-2026-07-10-ot45-design-language-think-cerebro.md
**Branch:** codex/ot45-design-language-think-cerebro
**Scope:** Design THINK — research + design only. No code, no QEMU, host-only. Makes the *ratified* reconciliation concrete; does **not** re-open settled forks (the name "Atrium", own-protocol, intent-additive, Workfield-first, the *direction* to reconcile — all ratified in the synthesis). Recommend; **the PL ratifies the aesthetic — it is theirs to feel.** Produces the **ADR-024 amendment** (§3).

**Inputs (read firsthand):**
- Ratified paradigm — `docs/THINK-2026-07-08-gui-vision-SYNTHESIS.md` (room=Atrium / unit=Workfield / atom=cap-surface; the named primitives Cap Halo / Undo Shadow / Liveness Thread / Command Graph; §1 aesthetic fork; the §12-Threshold reconciliation).
- What I ground / consume (compose, not collide): `docs/THINK-2026-07-10-atrium-g0-buildable-spec.md` (Chad's G0 — §2.3 grammar, §2.4 craft bar + Threshold resting, **§2.5 Cap Halo material table**, §2.6 motion rules, §1.3 cap-surfaces, §1.5 Liveness Thread, §1.6 Undo Shadow); `docs/THINK-2026-07-10-atrium-text-engine-research.md` (Prometheus — the **T0 type seam**: FreeType/OFL, Inter/Plex Sans, mono-for-`neo>`/proportional-for-Workfields; the R1/R2/R4 legibility traps).
- The doc I amend — `docs/ADR-024-phase-8-design-system-and-tech-stack.md` + `docs/design-system.md` (v1.1, incl. §12 The Threshold — the resting-state proof).
- Bible **Article 1.6** (beauty-as-craft, grammar-not-uniformity, the Tahoe cautionary tale) + **1.5** (five-year-old) + **29** (Arthur felt-not-avatared).
- External craft cited firsthand in Sources (Kare/classic Mac, Aqua, the Big Sur→Tahoe arc, Rams/Braun, iOS/visionOS materials, Core Animation motion). OT-1 discipline: cite what I reference, import nothing, no summarizing fetch for exact claims.
- RAG healthy (`{"status":"healthy",…}`); used for the boot-frame (`kernel/src/init/boot_frame.rs`) + framebuffer-palette facts.

---

## 0. The one idea

**Beauty here is not decoration bolted onto plumbing — it is the capability model made visible.** The Atrium's signature is that **a surface's material carries its trust state** (owned / tethered / untrusted / denied / undo-armed), so *security becomes the beauty* — the thing every other OS hides in nagging dialogs, we render as the aesthetic itself. This is the one idea both independent visions named (synthesis §1.3), and it is the load-bearing test for this whole document: every rule below either makes capability and reversibility *legible and lovely*, or it is cut.

The governing constraint (Article 1.6): **grammar, not uniformity.** One consistent system of spacing / light / material / motion / type / color that *preserves distinct per-thing identity* — because distinct silhouettes are recognition, and recognition is the five-year-old test applied visually. The documented failure mode to design against is the macOS Tahoe / Liquid-Glass episode: Apple forced every icon into one rounded-rect and made a once-beautiful system **blander AND harder to use.** Enforced sameness is anti-beauty *and* anti-usability. We refuse it structurally (§2).

---

## 1. ISC-1 — The grammar across its axes (concrete rules, not vibes)

Six axes. Materials (§4), motion (§5), type + color (§6) get their own sections; **spacing/layout** and **light/depth** are specified here in full. Each rule states *why it serves beauty AND legibility* — in this system those are the same axis (1.6: beauty is usability made visible; §5: latency is beauty).

### 1.1 Spacing / layout — bands and columns on an 8px grid; density is a state, not a default

The structure is **full-width bands** (small screens) or **fixed columns** (large), never floating dashboard panels or nested cards (Chad G0 §2.3):

```
craft bar     ATRIUM · Clean Folder · /home/downloads · caps: read ok, write missing · neo>
Intent Spine  (left)   transcript turns, active-turn marker
Workfield     (center) candidates · command graph · action preview
Thread        (right)  caps · liveness · undo shadow
neo> rail     (bottom) the exact deterministic equivalent + the direct prompt
```

- **8px base grid** (ADR-024 §1) — every margin/gap/pad is a multiple of 8; predictable rhythm is recognition.
- **Density is on demand, not the wallpaper.** The single most important correction to ADR-024's "information-dense by default": at *rest*, the screen is calm — the Threshold's rule of **≥70% near-Void negative space** (design-system §12.2) governs the idle Atrium, not the dense cockpit. Density *blooms* when you ask a surface to report (a Workfield opens, `neo> status` runs). Calm-at-rest, dense-on-demand. *(Beauty: Rams restraint — "as little design as possible." Legibility: no hunt; the eye lands on what changed.)*
- **No nested cards.** Repeated candidates are compact cap-surfaces in a band, not floating panels (Chad G0 §2.3). *(The Tahoe lesson at the layout level: fewer, more-distinct regions beat many identical ones.)*

### 1.2 Light / depth — depth is light and layering, never gratuitous shadow

On a flat 32-bpp framebuffer, depth must read without skeuomorphic drop-shadows (Aqua's later restraint; visionOS depth-via-material; Rams "unobtrusive"). The rules:

- **Electric Blue is *light*, not fill.** It is the substrate's glow — used for focus, owned authority, and the active edge — never as a flat background wash (design-system §12.2). *(Beauty: honest light. Legibility: blue = "this is live/owned," a learned constant.)*
- **The only shadow is the Undo Shadow.** Depth is spent *semantically*: the one offset shadow in the system is the Undo Shadow — a band offset 1 row / 4 px *behind* the Action surface (Chad G0 §2.5/§1.6), because the past is literally behind the present (my time-as-axis, ratified synthesis §3). Shadow is never ornament; when you see depth, it *means* reversibility is armed. *(Beauty: depth with meaning. Legibility: the shadow IS the "safe to be wrong" signal.)*
- **One crafted material for the one crafted surface.** The Threshold's luminous frosted pane, *backlit by the substrate* (design-system §12.3), is the single place soft depth is permitted — generalized here to "the surface currently holding your intent." Everything else is flatter, quieter structure. *(Grammar-not-uniformity: not every surface gets the hero material — distinctness by restraint.)*

### 1.3 Materials → §4 · Motion → §5 · Typography + Color → §6

The remaining three axes are specified in their dedicated sections. The through-line: **materials** carry trust (the signature), **motion** is honest and measured, **type/color** are a semantic token set where neon means something.

---

## 2. ISC-2 — Grammar-not-uniformity: the distinctness rule (load-bearing)

This is the axis that keeps the system from going bland — the anti-Tahoe. The rule, stated so a builder can apply it:

> **Shared grammar + mandated per-thing distinctness.** Everything obeys ONE physics — the 8px grid (§1.1), one light model (§1.2), one material family (§4), one motion physics (§5), one token set (§6). Within that physics, every *kind* of thing MUST carry a distinct **silhouette + accent + material signature**, and that distinctness may never be sanded off for tidiness.

Concretely, distinctness lives at three levels:

1. **Cap-surface kind** (Chad G0 §1.3: Intent / Folder / Candidate / Action / Grant / Undo / Command-Graph / Liveness). Each is recognizable by *shape + position + material*, not by reading its label — the five-year-old reads shape and color before words (1.6). A Grant surface (magenta left-edge + "grant required" chip) can never be mistaken for an Action surface (solid owned material) at a glance.
2. **Workfield kind.** A "clean folder" Workfield and a "find a file" Workfield share the band grammar but carry distinct silhouettes/accents so the room is legible from across it — you know *what kind of work* is open without reading.
3. **App identity** (future, OT-44). Apps get distinct silhouettes (classic-Mac / Kare iconography — legible shape, not a forced rounded-rect). The Tahoe collapse is forbidden by construction: **there is no "normalize every surface into one shape" pass**; the design system *supplies* the grammar and *protects* identity.

The test for any new surface: *could a five-year-old tell it apart from its neighbours with the sound off and the labels blurred?* If enforced sameness would make the answer "no," the sameness is the false trade (1.6), rejected the way 1.5 rejects usability-vs-security trades.

---

## 3. ISC-3 — The ADR-024 reconciliation, made concrete + THE AMENDMENT

ADR-024 (ratified 2026-05-28) committed to **cyberpunk maximalism** — "information-dense by default," "make the system visible," "piloting a real machine" — and, in passing, cited Bible **23.1 "no system AI persona"** and proposed a **Smithay/Wayland** compositor. Three of those are now superseded, and the synthesis ratified *reconciling* the aesthetic. This section makes the reconciliation concrete and gives the amendment.

**The reconciliation in three moves:**
- **Resting state = "intelligence as weather" (the §12 Threshold), not a cockpit.** When Arthur is idle, the Atrium shows ambient light and quiet — a dim Intent Spine, a quiet active-turn marker, a dormant Liveness Thread (Chad G0 §2.4). **No avatar, face, or mascot** — Arthur is *felt, not avatared* (Article 29; the surviving-and-correct half of §12's "presence not persona"). This is the calm-at-rest of §1.1.
- **Craft bar = classic-Mac restraint for structure/chrome.** The persistent chrome (the craft bar, borders, rails) is quiet, legible, timeless — Rams/Aqua restraint — not neon-drenched. Structure recedes so content and capability read.
- **Neon = meaning, not decoration.** The palette is *retained* (it is good and already semantic) but re-scoped: every accent *means* something (§6). Electric Blue = active/owned authority; Magenta = denial/alert/fatal ONLY; Lime/Red = functional pass/fail; Amber = warning/IO. Color is never sprinkled for cyberpunk flavour; the "make the system visible" honesty is preserved but delivered as *semantic density on demand* (§1.1), not a permanent neon grid.

### The ADR-024 amendment (shape + text)

> **ADR-024 AMENDMENT (2026-07-10, via OT-45 design-language THINK; status: proposed → PL-ratify).** ADR-024 §1 "Visual Identity" is amended from cyberpunk-maximalism to the **Atrium reconciliation**:
> 1. **Resting state is calm ("intelligence as weather," §12 Threshold); density is on-demand, not the default.** The "make the system visible" mandate is preserved as *semantic density when a surface reports*, governed by the ≥70%-negative-space-at-rest rule. Supersedes "information-dense by default."
> 2. **The palette and 8px grid are RETAINED, re-scoped as semantic** (neon = meaning, §6). The craft bar / chrome adopt classic-Mac restraint.
> 3. **Persona: superseded.** ADR-024's reliance on Bible 23.1 ("no system AI persona") is void — 23.1 was reversed (Article 29). Arthur is a constitutional, *felt* presence (ambient light + honest liveness), **never an avatar/face/mascot.**
> 4. **Compositor + UI framework: superseded by OT-52 / the synthesis.** Smithay/Wayland → **our own cap-native compositor**; immediate-mode egui → a **retained-mode** cap-native scene (Core-Animation-class). ADR-024's tech-stack §2 rows for Compositor/UI-framework are informational-historical only.
> 5. **Accessibility is promoted to first-class, NOT deferred.** ADR-024 §Anti-Decisions deferred accessibility ("cyberpunk can be harsh on low-vision"); the reconciliation makes legibility non-negotiable (the five-year-old test IS a contrast/reduced-motion requirement — §6). Contrast floors, reduced-motion, and high-contrast are grammar, not Phase-8.5 polish.
> The concrete grammar/token set (this THINK, §4/§5/§6) folds into **`design-system.md` v2** as the GUI-phase design system; ADR-024 remains the historical Phase-8 dashboard record.

---

## 4. ISC-4 — The cap-surface material system (the Atrium's signature)

This is where "security becomes the beauty" is real or hand-waving. A surface's **material renders its trust/grant state**, and the state vocabulary is **stable across the software-primitive form (today's raw framebuffer) and the future GPU form** — the look gets richer, the *meaning* never changes. This grounds Chad's G0 §2.5 Cap-Halo table and fuses it with the ratified "materials carry capability" (Cerebro) + "Cap Halo" (Jarvis): the **Cap Halo is the instrument; the material is what fills it.**

| State | Means | Software material (G0 / raw framebuffer) | Future GPU material (G5+) | Why it reads |
|---|---|---|---|---|
| **Owned / granted** | You hold the authority; activatable | Solid **Silver** border + **Electric-Blue** corner ticks | Rooted, solid, catches the substrate light; sits *in* the space | The default "yours" — solid = trust |
| **Session / tethered** | Granted, revocably; on loan | **Dashed** border + a thin **tether line** to the grant surface | Lighter/provisional material + a luminous tether to its owner; **cutting the tether = revoke** (OT-18 D6 made physical) | You can *see* it's on loan; the leash is the revoke affordance |
| **Missing / denied** | Authority not held; blocked | **Alert-Magenta** left edge + "grant required" chip (+ the `neo> grant` equivalent) | Cool recoil, magenta edge; the Grant surface offered, never a modal | Denial is honest and points at the fix (butler register + `neo>`), never consent-theater |
| **Untrusted / deferred** | Bounded, held at arm's length (OT-44 WASM cap-bundle) | Muted **stipple** / dim border; **not activatable** | Distinct "arm's-length" material — visibly sandboxed | "Safe to install by construction" made visible; the boundary is seen, not read |
| **Undo-armed** | A reversible mutation is staged/done | **Shadow band** offset 1 row / 4 px behind the Action surface | Soft offset depth (§1.2) — the past behind the present | Depth = reversibility; the one semantic shadow |

For the current 8×16 bitmap console (before T0), the same vocabulary uses ASCII-safe halos (Chad G0 §2.5): `+----+` owned · `+ - -+` session · `!----+` denied · `::::` undo-armed. **The state vocabulary is identical in bitmap, T0, and GPU forms** — only the fidelity changes. That stability is what lets G0 ship the *meaning* now and the *beauty* later without a re-teach.

Design rules for the material system:
- **Trust state is always visible on any activatable surface** — there is no un-halo'd cap-surface; you never guess what you hold.
- **Revoke is a gesture on the tether, not a settings trip** (§ the Hold verb, ratified). The material *is* the control.
- **Distinct materials per state** (grammar-not-uniformity, §2): owned ≠ tethered ≠ denied ≠ untrusted at a glance, sound-off, labels-blurred.

---

## 5. ISC-5 — Motion + latency-is-beauty

Motion in the Atrium is **honest, measured, and interruptible** — it communicates state, never delights for its own sake. This grounds Chad's G0 §2.6 and generalizes it.

**Permitted motion (all tied to a real event):**
- **Row-reveal** as records arrive (a candidate appears when the directory read returns it).
- **Border/material state-change** on a cap-check result (owned→denied the instant `execute_action` checks the cap).
- **Liveness-Thread segments** lighting only when a *measured* phase begins/ends (IntentTokens / ActionCandidate / CapCheck / DeviceWait / UndoWrite / PresentComplete — Chad G0 §1.5). Liveness is a channel, never a spinner.
- **Undo-Shadow arming** — the shadow appears only *after* the OT-19 artifact is actually written (§4, Chad G0 §1.6).

**Forbidden motion (Chad G0 §2.6 — hard rules):** fake spinners; continuous ambient idle animation (the resting state is *still*, §3); physics that implies a compositor we don't have yet; smooth zoom/transform (reserved for G2's GPU text). *(The Threshold's slow 4–6 s breath is the ONE permitted ambient motion, and only on the Threshold/greeter surface — design-system §12.2 — not the working Atrium.)*

**Interruptibility (the GPU future, G5+):** when we have the retained-mode compositor, every animation is a live, redirectable spring — Core Animation's model-tree/presentation-tree split, where a new intent mid-animation redirects from the current pixels; you never wait out a transition. Motion always yields to your next move.

**Latency IS beauty ("instant or it's not beautiful").** 1.6 says beauty is usability made visible; a janking surface is unusable *in that instant*, therefore ugly. The unfair advantage (synthesis §9): we already run inference-deadline/QoS governance (Phase-7/OT-15), so the *same* deadline machinery guarantees the compositor's frame deadline — on Linux the compositor is just another process. Target sub-frame input-to-photon (< 8.33 ms at 120 Hz, toward the single-digit-ms perceptual floor) with present-pacing + predictive input at G5. **For G0 this means: truthful, immediate state changes on the raw framebuffer** — no fake progress, no lag between a cap-check and its border change. Honesty of motion *is* the beauty budget until the GPU path exists.

---

## 6. ISC-6 — Typography + color as a semantic token set

### 6.1 Type — consume the T0 seam; the type-duality carries the arthur>/neo> split

I **consume** Prometheus's text-engine recommendation (cite, don't re-derive): **FreeType (FTL) userland service, an OFL face, AA glyph bitmaps blit through `blit_rgba()`, autohint < 16 px, no-hint above** (text-engine T0). The roles:

| Role | Face | Rationale |
|---|---|---|
| **Arthur's voice + Workfield surfaces** | **Proportional: Inter** (Regular, static, OFL; IBM Plex Sans until 2026-10-26) | Warm, readable, "feels different from the working OS" (design-system §12.4); the human register |
| **`neo>` deterministic rail + structured data** | **Monospace** (IBM Plex Mono) | Alignment for the command rail + tabular output; the precision register (text-engine Fork 5) |
| **Craft-bar labels / chips** | Proportional, Label weight (11 px / 600) | Quiet chrome |

The **type-duality mirrors the arthur>/neo> duality** — you feel which register you're in before you read a word (my GUI-vision grammar, ratified). Type scale from design-system §2 (12 px base, **weight-not-size** differentiation, line-height ≥ 1.4). Legibility floor (also the five-year-old test): apply **stem-darkening below 16 px** (text-engine trap R4) and gamma-correct AA compositing (R1/R2) so small text stays crisp, not fuzzy/thin.

**Decided 2026-10-26 (J.c.):** Arthur's proportional voice is **Inter** Regular (static TTF, rsms/inter release v4.1; `assets/fonts/README.md`), replacing IBM Plex Sans on Arthur's surface, Atrium/Home and the Lens. IBM Plex Mono stays the `neo>` rail and structured-data face. The text-engine traps still apply to Inter: stem darkening below 16 px (R4) and gamma-correct AA compositing (R1/R2). The fork below is kept as the record of how the choice was framed.

**Fork flagged (composes with text-engine Fork 4):** Prometheus recommends **Inter** for T0 but notes it is "generic — doesn't carry the Atrium's aesthetic identity." Our grammar's *soul* is distinctness (§2), and the Threshold already uses **IBM Plex Sans** ("distinctive silhouettes," humanist warmth). So: Inter is the pragmatic, screen-optimized ship-now default for G0, **but the design-language target is a distinctive face** (Plex Sans interim; a custom Atrium face post-G2 at the G6 design pass). The bundled-face choice is an aesthetic call for the PL (§7).

### 6.2 Color — the semantic token set (neon = meaning)

Retain the ADR-024/design-system palette, re-scoped as **semantic tokens** with the neon-as-meaning rule and Chad's G0 role mapping:

| Token | Hex | Semantic meaning (never decorative) |
|---|---|---|
| `ground` (Void) | `#0a0e27` | Near-black canvas; the resting field (§12) |
| `authority-active` (Electric Blue) | `#3c8ef4` | Focus + owned/live capability — used as **light**, not fill (§1.2) |
| `structure` (Silver) | `#a8adb2` | Borders, secondary text, calm chrome (the classic-Mac restraint tone) |
| `structure-inactive` (muted blue) | dimmed `#3c8ef4` | Inactive structure (Chad G0 §2.4) — a derived low-value blue |
| `denial` (Magenta) | `#ff00ff` | Denied/missing authority, alert, fatal — **ONLY**; never decoration |
| `pass` / `fail` (Lime / Red) | `#00ff41` / red | Functional success/failure of an operation — not ornament |
| `warning` (Amber) | `#ffaa00` | Warning / I/O activity |
| `text-primary` (White) | `#ffffff` | High-importance text, focus |

**Contrast floors (non-negotiable — the five-year-old test; from design-system §9):** body text ≥ 7:1 (WCAG AAA) — White-on-Void 14.9:1, Silver-on-Void 8.4:1 (both AAA); Electric Blue-on-Void 5.8:1 is AA, so **Electric Blue is an accent/large-text/border color, never body copy.** Beauty that isn't legible fails 1.5. Reduced-motion and high-contrast profiles are grammar (§3 amendment), not deferred.

---

## 7. ISC-7 — Apply it to the real surfaces + honest forks

### 7.1 The grammar on the surfaces that exist / are next

- **Boot frame (OT-36 honesty; `kernel/src/init/boot_frame.rs`).** The centered wordmark + subtitle already follow an "H1-over-blockquote" hierarchy. Grammar applied: Void ground, Silver wordmark, Electric-Blue as light (a restrained glow, not a neon wash), generous negative space (≥70%). It is the *first* instance of "intelligence as weather" — quiet, crafted, honest (no fake progress bars). Kare/Aqua restraint, not cyberpunk maximalism.
- **v1 framebuffer console / transcript (Janus `4858ac6`).** Today's 8×16 bitmap console becomes the **Intent Spine** (§1.1). Grammar applied now, pre-T0: Silver body on Void, Electric-Blue active-turn marker, the shipped liveness indicator ("forward pass running / complete tokens=N") is the honest Liveness Thread seed (Chad G0 §1.5). ASCII-safe cap-halos (§4) where authority is shown. The console is already partly compliant — the grammar formalizes it.
- **G0 (Chad's spec — I make its visuals concrete).** The craft bar renders as the quiet status strip (`ATRIUM · Clean Folder · /home/downloads · caps: read ok, write missing · neo>`, Chad G0 §2.4) in `structure`/`authority-active` tokens; the Workfield bands (Intent Spine / Workfield / Caps-Liveness-Undo / `neo>`) on the 8px grid; **Cap Halos rendered per §4** (owned/tethered/denied/untrusted/undo-armed, software form); Threshold resting state (calm, no avatar) when idle; motion per §5 (row-reveal, border-on-cap-check, measured liveness, undo-shadow arming — no fake spinners). This turns Chad's "gestures at a craft bar / cap-halo / palette it can't fully specify" into a renderable grammar.

### 7.2 Honest forks (the aesthetic is the PL's to feel)

The synthesis made the *direction* (reconcile) the PL's ratified fork #1; these are the concrete choices within it, surfaced for the PL to react to — **not a fait accompli**:

1. **How far to soften cyberpunk.** *My rec:* the reconciliation as specified — retain the palette as *semantic*, calm-at-rest / dense-on-demand, craft-bar restraint. The PL feels whether this is still "their" machine or too quiet; the dial (how much neon, how dense the resting state) is theirs.
2. **Bundled UI face: Inter vs IBM Plex Sans vs a custom Atrium face** (composes with text-engine Fork 4). *My rec:* Inter to ship G0 (screen-optimized), **Plex Sans as the identity-carrying interim** (distinctness, §2), a custom face at G6. Genuine aesthetic call — Inter's neutrality is in mild tension with the grammar's distinctness soul.
3. **Accessibility promoted to first-class now** (reverses ADR-024's deferral). *My rec:* yes — contrast floors + reduced-motion + high-contrast are grammar (§3/§6), because they *are* the five-year-old/legibility test, not polish. Low downside; ask the PL to ratify the promotion.
4. **`structure-inactive` muted-blue token — exact value.** A derived low-value Electric Blue (Chad G0 §2.4 "muted blue"); the precise hex is a `design-system.md` v2 detail, flagged so it's chosen deliberately, not ad hoc.

Everything here composes with (does not collide with) Chad's G0 forks and Prometheus's text-engine forks; where they overlap (the bundled face) I state the seam and defer the feel to the PL.

---

## RETURN-relevant summary (for the relay)

- **The grammar (six axes):** spacing = bands/columns on 8px, **calm-at-rest / dense-on-demand**; light/depth = Electric-Blue-as-light + the **only shadow is the Undo Shadow** (depth = reversibility); materials = §4; motion = §5; type/color = §6.
- **Grammar-not-uniformity (§2):** shared physics + mandated per-thing distinct silhouette/accent/material; the sound-off/labels-blurred five-year-old test; the Tahoe collapse forbidden by construction (no normalize-everything pass).
- **The cap-surface material system (the signature, §4):** owned / tethered / denied / untrusted / undo-armed — a **stable vocabulary across bitmap → T0 → GPU**, grounding Chad's G0 §2.5; revoke = cut the tether; denial points at the grant, never a modal. Security *is* the beauty.
- **The ADR-024 amendment (§3):** cyberpunk-maximalism → the reconciliation (calm resting "weather," craft-bar restraint, neon-as-meaning); persona ref (23.1) void → Article 29 felt-not-avatared; compositor/egui superseded by OT-52/retained-mode; **accessibility promoted to first-class**; concrete grammar folds into `design-system.md` v2.
- **Type/color token set (§6):** consume the T0 seam (FreeType/OFL; proportional humanist for Arthur/Workfields, mono for `neo>`; type-duality = the arthur>/neo> duality); semantic color tokens with neon-as-meaning + WCAG-AAA contrast floors (Electric Blue = accent-only, never body).
- **Applied to boot-frame / console / G0 (§7.1);** forks for the PL (§7.2): how-far-to-soften, bundled face (Inter vs Plex Sans vs custom), accessibility-now, the muted-blue value — the aesthetic is the PL's to feel.
- **Did not re-open** settled forks (name/protocol/intent-additive/Workfield-first/reconcile-direction). Composed with Chad's G0 + Prometheus's T0; did not collide.

---

## Sources (craft cited firsthand; OT-1 discipline)

**Internal:** `docs/THINK-2026-07-08-gui-vision-SYNTHESIS.md`; `docs/THINK-2026-07-10-atrium-g0-buildable-spec.md` (Chad, §1.3/§1.5/§1.6/§2.3/§2.4/§2.5/§2.6); `docs/THINK-2026-07-10-atrium-text-engine-research.md` (Prometheus, T0 + forks 4/5 + traps R1/R2/R4); `docs/ADR-024-…` + `docs/design-system.md` (palette, §2 type, §9 contrast, §12 Threshold); Bible Articles 1.5 / 1.6 / 29; `kernel/src/init/boot_frame.rs` (boot-frame).

**External craft:**
- Susan Kare / classic Mac OS iconography — distinct silhouettes, recognition-by-shape (the anti-Tahoe) — kare.com; folklore.org.
- Apple Aqua & the Big Sur→Tahoe materials arc, incl. the **Liquid-Glass / Tahoe enforced-sameness episode** (the 1.6 cautionary tale) — Apple HIG (materials); the Rogue Amoeba/OSNews icon plea (banked in OT-45).
- Dieter Rams / Braun — **Ten Principles for Good Design** ("good design is as little design as possible," unobtrusive, honest, long-lasting) — vitsoe.com/us/about/good-design.
- Apple **Core Animation** — retained-mode + model-tree/presentation-tree interruptible motion — developer.apple.com/…/CoreAnimation_guide.
- Apple **visionOS / iOS materials** — depth & vibrancy via light and translucency, not drop-shadows — Apple HIG (materials/vibrancy).

---

## Gate + close

- **Docs-only ⇒ no CI run exists** (paths-ignored: `docs/**`, `**.md`). No push CI, no QEMU — correct for a research/design THINK.
- Ran the `scripts/worker-close-gate.sh` host-set intent locally: adds exactly one file (`docs/THINK-2026-07-10-atrium-design-language.md`), no source touched — fmt/clippy/host-test legs are no-ops (skip-with-note, docs-only carve-out); SPDX header present (CC-BY-4.0, first line). Self-stat below is the Step-4 self-inspection.
- **RETURN evidence:** `git diff --stat origin/main` = 1 file added, docs-only, no phantom deletions. Diffed the artifact against the RETURN summary before sending (accuracy axis).
- **Compose, not collide:** grounded Chad's G0 + consumed Prometheus's T0 (cited, not re-derived); did not re-open ratified forks; the aesthetic is surfaced for PL ratification, not decided.
- **Disk reclaim:** after the PM merges (or declines), delete the isolated worktree `E:/AI Automation/AltimitNeo-ot45` (`git worktree remove`) — docs-only, no `target/`; reclaim once the work is on `main`.
