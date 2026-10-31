# AltimitNeo Design System v1.0

**Status:** Foundation spec for Phase 8  
**Authority:** Design Lead + PM  
**Last updated:** 2026-06-03  
**Governed by:** ADR-024 (Phase 8 Design System & Tech Stack)

---

## Overview

AltimitNeo's visual identity is **cyberpunk:** neon on dark, information-dense, real-time metrics visible. The design language celebrates the OS's unique mission (AI-native kernel, multi-tenant inference scheduling, tensor operations) rather than mimicking generic desktop conventions.

This document is the source of truth for all UI/UX work in Phase 8+.

---

## 1. Color Palette

### Primary Colors

| Name | Hex | RGB | Use | Notes |
|------|-----|-----|-----|-------|
| **Void** | `#0a0e27` | 10, 14, 39 | Background (primary) | Deep blue-black, non-pure black (avoids burn-in on OLED) |
| **Electric Blue** | `#3c8ef4` | 60, 142, 244 | System info, primary accent | Primary; sampled from the LogansOS flare-star glow. CPU, memory, system status |
| **Silver** | `#a8adb2` | 168, 173, 178 | Structure, secondary text, borders | Sampled from the ring arcs + wordmark; the calm structural tone |
| **Lime** | `#00ff41` | 0, 255, 65 | Success, KV cache, available | "Go" state; tensor store, dedup hits |
| **Amber** | `#ffaa00` | 255, 170, 0 | Network/I/O, warnings | Caution state; I/O activity, network traffic |
| **Magenta** | `#ff00ff` | 255, 0, 255 | Alert / fatal **only** | Reserved for missed deadlines, OOM, driver faults, kernel panic — never decorative |
| **White** | `#ffffff` | 255, 255, 255 | Text primary | Used for high-importance text and focus indicators |

### Neutral Colors

| Name | Hex | RGB | Use |
|------|-----|-----|-----|
| **Dark 1** | `#1a2332` | 26, 35, 50 | Panel backgrounds (slightly lighter than Void) |
| **Dark 2** | `#0f1219` | 15, 18, 25 | Nested panel backgrounds |
| **Gray 1** | `#4a5568` | 74, 85, 104 | Disabled text, borders |
| **Gray 2** | `#2d3748` | 45, 55, 72 | Hover states, subtle separation |

### Semantic Colors

| Name | Color | Use |
|------|-------|-----|
| **Success** | Lime (`#00ff41`) | Model loaded, inference complete, dedup hit |
| **Warning** | Amber (`#ffaa00`) | High memory usage, slow I/O, network latency |
| **Alert** | Magenta (`#ff00ff`) | Inference missed deadline, GPU OOM, driver error, kernel panic |
| **Info** | Electric Blue (`#3c8ef4`) | System status, active jobs, real-time metrics |
| **Structure** | Silver (`#a8adb2`) | Panel borders, secondary labels, separators |
| **Disabled** | Gray 1 (`#4a5568`) | Inactive buttons, unavailable features |

### Implementation

**CSS Variables:**
```css
:root {
  --neo-bg-primary: #0a0e27;
  --neo-bg-panel: #1a2332;
  --neo-bg-nested: #0f1219;
  
  --neo-color-electric-blue: #3c8ef4;
  --neo-color-silver: #a8adb2;
  --neo-color-magenta: #ff00ff;
  --neo-color-lime: #00ff41;
  --neo-color-amber: #ffaa00;
  --neo-color-white: #ffffff;
  
  --neo-color-success: var(--neo-color-lime);
  --neo-color-warning: var(--neo-color-amber);
  --neo-color-alert: var(--neo-color-magenta);
  --neo-color-info: var(--neo-color-electric-blue);
  
  --neo-text-primary: #ffffff;
  --neo-text-secondary: var(--neo-color-silver);
  --neo-text-disabled: #4a5568;
  
  --neo-border: 1px solid var(--neo-color-electric-blue);
}
```

**Rust (egui):**
```rust
pub struct NeoTheme {
    pub bg_primary: Color32,        // #0a0e27
    pub bg_panel: Color32,          // #1a2332
    pub color_electric_blue: Color32, // #3c8ef4  (primary accent)
    pub color_silver: Color32,      // #a8adb2  (structure / secondary)
    pub color_magenta: Color32,     // #ff00ff  (alert / fatal only)
    pub color_lime: Color32,        // #00ff41
    pub color_amber: Color32,       // #ffaa00
}
```

---

## 2. Typography

### Font Stack

| Use | Font | Fallback | Notes |
|-----|------|----------|-------|
| **System/body** | IBM Plex Mono | Consolas, Courier New | Monospace; native to cyberpunk aesthetic |
| **UI labels** | Inter | Helvetica, Arial | Sans-serif; minimal, clean |
| **Headers** | IBM Plex Mono (bold) | IBM Plex Mono | Use weight differentiation, not size |

### Type Scale

| Name | Size | Weight | Line Height | Use |
|------|------|--------|-------------|-----|
| **Headline 1** | 24px | 700 (bold) | 1.2 | Main window titles, section headers |
| **Headline 2** | 18px | 700 (bold) | 1.3 | Panel titles, subsection headers |
| **Body large** | 14px | 400 (regular) | 1.5 | Primary body text, descriptions |
| **Body** | 12px | 400 (regular) | 1.5 | Default body text (BASE SIZE) |
| **Body small** | 10px | 400 (regular) | 1.4 | Secondary text, captions |
| **Label** | 11px | 600 (semibold) | 1.4 | Button text, form labels, tags |
| **Monospace** | 12px | 400 (regular) | 1.5 | Code, command output, metric values |

### Rules

- **Base size is 12px.** All other sizes are multiples/fractions thereof.
- **Line height minimum 1.4.** No compressed text; readability matters in high-density displays.
- **Body text targets contrast ratio ≥ 7:1** on background (WCAG AAA); accent and
  large text target ≥ 4.5:1 (AA). The Electric Blue primary is an *accent*, not a
  body-copy color — render long text in White or Silver.
  - White (#ffffff) on Void (#0a0e27): 14.9:1 ✅ AAA (primary body text)
  - Silver (#a8adb2) on Void: 8.4:1 ✅ AAA (secondary text, borders)
  - Electric Blue (#3c8ef4) on Void: 5.8:1 ✅ AA (accents, large text, UI borders — not body copy)
  - Gray (#4a5568) on Void: 3.1:1 ❌ (use Gray 1 only for disabled/secondary)

---

## 3. Spacing System

### Base Unit: 8px

All spacing is a multiple of 8px. This creates a predictable, composable rhythm.

| Unit | Pixels | Use |
|------|--------|-----|
| **xs** | 4px | Micro-spacing (letter-spacing, border-width) |
| **sm** | 8px | Padding (buttons), gap between tightly related items |
| **md** | 16px | Standard padding (panels), gap between items |
| **lg** | 24px | Section padding, gap between sections |
| **xl** | 32px | Major section padding, page margins |
| **2xl** | 48px | Margin between major layout blocks |

### Grid

- **Column gap:** 16px
- **Row gap:** 16px
- **Panel border-radius:** 4px (slight softness, not aggressive rounding)
- **Button padding:** 8px (horizontal) × 4px (vertical)
- **Input padding:** 8px

---

## 4. Components

### 4.1 Buttons

**Default (Electric Blue):**
```
┌─────────────────┐
│  • Load Model   │  Text color: Electric Blue, bg: transparent, border: 1px Electric Blue
└─────────────────┘
```

**Hover:**
```
┌─────────────────┐
│  • Load Model   │  Border glow: Electric Blue shadow, bg: rgba(60,142,244,0.1)
└─────────────────┘
```

**Active/Pressed:**
```
┌─────────────────┐
│  • Load Model   │  Bg: rgba(60,142,244,0.2), border: 2px Electric Blue
└─────────────────┘
```

**Disabled:**
```
┌─────────────────┐
│  • Load Model   │  Text color: Gray 1, border: 1px Gray 1, no hover
└─────────────────┘
```

**Alert (Magenta):**
```
┌──────────────────┐
│ ⚠ Delete Model   │  Text color: Magenta, border: 1px Magenta
└──────────────────┘
```

### 4.2 Inputs (Text Fields)

**Default:**
```
Model name: ___________________________
            ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔  (Electric Blue underline, 1px)
```

**Focus:**
```
Model name: ___________________________
            ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔  (Electric Blue underline, 2px, glow)
            ^ cursor (blinking Electric Blue)
```

**Disabled:**
```
Model name: ___________________________
            ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔  (Gray 1 underline, text Gray 1)
```

### 4.3 Progress Bars

**Default (Electric Blue):**
```
[████████░░░░░░░░░░░░] 52% Loading weights...
```

**Warning (Amber):**
```
[████████████████░░░░░░] 75% High memory usage
```

**Alert (Magenta):**
```
[████████████████████░░] 92% Inference missed deadline
```

**Implementation:** Solid bar with optional glow shadow.

### 4.4 Alerts / Status Messages

**Success (Lime):**
```
✓ Model loaded: llama-2-70b (hash: a1f3c9e2)
```

**Warning (Amber):**
```
⚠ GPU memory at 89%. Consider offloading models.
```

**Alert (Magenta):**
```
✗ Inference deadline missed by 23ms.
```

### 4.5 Panels / Cards

**Border:**
```
┌─ neo-shell ──────────────────┐
│                              │  Border: 1px Electric Blue
│ $ _                          │  Corner radius: 4px
│                              │
└──────────────────────────────┘
```

**Hover/Focus:**
```
┌─ neo-shell ──────────────────┐
│                              │  Border: 2px Electric Blue, glow shadow
│ $ _                          │
│                              │
└──────────────────────────────┘
```

**Nested panels:** Use Dark 2 background with slightly dimmer Electric Blue border.

### 4.6 Lists / Trees

```
├─ llama-2-70b     280 GB      Use Electric Blue for hierarchy, Lime for size indicators
├─ mistral-7b       62 GB
│  ├─ adapter        2.1 GB    Electric Blue text, Gray 2 separators
│  └─ tokenizer    ~512 KB
└─ [shared]         45.2 GB    Brackets [shared] indicate special status
```

---

## 5. Layout Rules

### Density

AltimitNeo is **information-dense** by design. This is not a "spacious" OS.

- Margins/padding are always 8px, 16px, or 24px (multiples of base).
- Whitespace should be intentional, not gratuitous.
- Panels should feel compact but not cramped.

### Alignment

- **Horizontal alignment:** Left-aligned by default (monospace text). Center for titles only.
- **Vertical alignment:** Top-aligned for lists and dashboards. Center for modals.
- **Grid alignment:** All elements snap to 8px grid. No off-grid positioning (except for dynamic content like graphs).

### Layering

Panels are layered with subtle visual separation:

```
Background (Void, #0a0e27)
└─ Panel 1 (Dark 1, #1a2332) border: Electric Blue
   └─ Nested panel (Dark 2, #0f1219) border: Electric Blue, dimmer
      └─ Content (text, metrics, controls)
```

Shadows are subtle (avoid drop-shadow design):
```css
box-shadow: 0 4px 12px rgba(60, 142, 244, 0.15);  /* Electric Blue glow */
```

---

## 6. Animation & Interaction

### Transitions

- **Fade:** 100ms (for visibility changes)
- **Slide:** 150ms (for panel transitions, drawer opens)
- **Glow:** 200ms (for focus/hover state changes)

Easing: `cubic-bezier(0.4, 0, 0.2, 1)` (standard material easing).

### Real-Time Updates

Metrics are updated every 100ms (10 Hz refresh). Graphs use line animation (data points appear smoothly).

**DO NOT** blink or flash text. High-frequency blinking is jarring and inaccessible.

### Hover States

- Buttons: +20% brightness, 2px border glow
- List items: Dark 2 background, Electric Blue left border
- Progress bars: Text appears below bar on hover

---

## 7. Dark Mode

The cyberpunk aesthetic is **inherently dark mode.** There is no "light variant" in Phase 8. 

If a light variant is needed later (Phase 8.5+), it would use:
- Background: `#f5f5f5` (light gray)
- Text: `#0a0e27` (dark blue-black)
- Accents: Saturated versions of the neon palette (less glow, more contrast)

---

## 8. Icon System

*Replaced 2026-10-29 for J.c.'s first app-icon set (`docs/design/icons-2026-10-29/`,
`Plans/SYNTHESIS-2026-10-29-icons.md`). The old 24×24 / 2 px / Electric-Blue-everything spec predates the Atrium.*

**Authority.** App icons follow the Atrium design language: classic-Mac legibility, one physics and distinct identity
(`docs/THINK-2026-07-10-atrium-design-language.md` §2). An icon is identity, never trust state. Trust is carried by
glass tint, halo geometry and labels (`docs/THINK-2026-07-19-atrium-glass-frost-amendment.md`).

**Shape.** Each icon is one nameable object with its own silhouette. There is no common tile or rounded-rect behind
icons; the Spine chip is the frame. A new icon must pass the five-year-old test against every existing one: render it
at 24 px in one colour, blur it 4 px, and it must still differ from the rest in where its mass sits, how elongated it
is or which way it points.

**Sizes and grid.**
- The master is **40×40**, for the 56×56 Spine chip: a 2 px safe margin (kept fully transparent), an 8 px grid and a
  1 px sub-grid.
- Hand-hinted **24×24** (lists and menus) and **16×16** (small) versions drop detail rather than shrink it.
- Edges land on pixel boundaries: odd-width strokes on half-pixel centrelines, even-width strokes on whole pixels.

**Stroke.** Three weights. At 40 / 24 / 16 px:

| Weight | 40 px | 24 px | 16 px | Used for |
|---|---|---|---|---|
| Structure | 3 | 2 | 1.5 | Outlines and tracks, and Install's arrow. |
| Heavy | 3.5 | 2.5 | 2 | Single-line glyphs that must hold their own, such as Terminal's chevron. A held handle may go to 5 at 40 (Lens). |
| Light | 2–2.5 | 1.25–1.5 | 1–1.25 | Light marks and fine detail: glints, lips, Browser's meridians. |

Round caps and joins throughout.

**Colour.**
- Silver `#a8adb2` for structure.
- White `#ffffff` for at most one "light or action" shape per icon (Arthur's spark, Install's arrow).
- Dark 1 `#1a2332` and Gray 2 `#2d3748` for bodies.
- Electric Blue `#3c8ef4` only as **one small light per icon**, and never on a silhouette-carrying mark. It falls to
  2.8:1 on the frosted chip, so no icon may depend on it.
- Magenta, lime and amber remain reserved for their system meanings (deny, pass, warn) and are not identity colours.
- Silhouette marks reach at least 3:1 on the Void (Silver 8.4:1), on Dark 1 (7.0:1) and on the frosted chip
  (4.1:1).

**One colour.** Every icon has a one-colour form made by a fixed rule: Electric Blue and White become Silver, and the
dark fills become the ground. The one-colour form must still pass the five-year-old test.

**Format and build.**
- Plain, hand-written SVG: paths, circles and rects, solid colours, straight alpha. No filters, gradients, masks,
  text, images or transforms.
- The OS has no SVG renderer, so `cargo xtask icons` (the host-only `tools/icon-raster`, resvg) rasterises each SVG
  to a committed straight-alpha RGBA8888 blob, pinned by BLAKE3 (`assets/icons/README.md`).
- On a Spine chip, the 40 px icon sits at `(chip.x + 8, chip.y + 6)`, blended
  `out = (s·a + d·(255 − a) + 127)/255` per channel. The chip's name moves below it (baseline `chip.y + 53`). This is
  neo-atrium's `app-icons` / neo-init's `atrium-app-icons`.

**Current set** (`assets/icons/`, J.c., 2026-10-29):

| Icon | Shape | Designed by |
|---|---|---|
| Arthur | a four-point spark beside one open arc, with a live dot | S1 |
| Lens | a magnifier | S1 |
| Terminal | the bare `neo>` chevron and a cursor | S1 |
| Files | a tabbed folder | S1 |
| Settings | three sliders | S1 |
| Install | a thin arrow into a shallow tray on a wider base | Jarvis (redrawn at the colour rule) |
| Browser | a globe with an equator and two meridians | Jarvis (redrawn at the colour rule) |

---

## 9. Accessibility

### Contrast

- All text must have ≥ 7:1 contrast ratio (WCAG AAA).
- Current palette achieves this for Electric Blue, White, Lime, Amber on Void background.
- Gray 1 (`#4a5568`) fails (3.1:1); use only for secondary/disabled text, not primary body.

### Keyboard Navigation

- All interactive elements are keyboard-accessible.
- Tab order follows visual order (left-to-right, top-to-bottom).
- Focus indicator is always visible: 2px Electric Blue border + glow.

### Motion

- No autoplay videos or animations >5s duration.
- User must opt-in to transitions via OS settings (prefers-reduced-motion).
- Real-time graphs can disable animations for users with vestibular sensitivity.

### Font Size

- Minimum 12px for body text.
- Users can increase font size via system settings (Phase 8.5+).

---

## 10. Design System Governance

### Lock-down

This design system is **locked** for Phase 8.1–8.4. No changes without a Design Review + PM approval + ADR amendment.

Rationale: Shipping a coherent visual identity matters more than pixel-perfect optimization. Iterating mid-phase causes design debt.

### Extensibility

Phase 8.5+ extensions:

- Light theme variant
- High-contrast theme (WCAG AAA on steroids)
- Custom color schemes (user preference)
- Accessibility variants (reduced motion, large text, etc.)

These are NOT part of Phase 8 MVP.

### Tooling

Design system stored as:

1. **This Markdown file** (source of truth)
2. **CSS variables** (in the compositor's stylesheet)
3. **Rust code** (egui theme struct, defined in `neo-ui/src/theme.rs`)
4. **Figma file** (design-tool source; optional, can defer to Phase 8.5)

---

## 11. Reference Implementations

### neo-shell

```
╔══════════════════════════════════════════════════════════════════╗
║  AltimitNeo 1.0.0  │  [CPU: 23%] [GPU: 67%] [MEM: 4.2GB/16GB]  ║
╠══════════════════════════════════════════════════════════════════╣
║                                                                  ║
║  $ neo-infer --model llama-2-70b --prompt "Hello"              ║
║  > Inference context bound to GPU:0                            ║
║  > Latency target: 50ms/token                                  ║
║  > [████████████░░░░░░] Token 47/100                           ║
║                                                                  ║
║  _                                                               ║
╚══════════════════════════════════════════════════════════════════╝
```

**Colors:** Electric Blue border, Electric Blue text, Amber progress bar, Green success messages.

### neo-monitor

```
┌─ neo-monitor ─────────────────────────────┐
│ CPU (8 cores)          GPU:0 (RTX 4090)   │
│ ███████░░░░░ 58%       █████████░ 91%     │
│                                            │
│ TENSOR STORE           INFERENCE QUEUE     │
│ Total: 342 GB          Active: 3           │
│ ├─ llama-2-70b   280 GB ├─ PID 1247 ⚡   │
│ ├─ mistral-7b     62 GB └─ PID 1429      │
│ └─ [shared] 45.2 GB                       │
│                                            │
│ [████████] 94.2 GB/s IO                   │
└────────────────────────────────────────────┘
```

**Colors:** Electric Blue text, Magenta GPU:1, Lime dedup hits, Amber I/O.

---

## 12. The Threshold — Login Greeter

**Status:** Accepted greeter spec (2026-06-03), extends the design system to the login surface. Net-new surface — **additive to ADR-024, no amendment required** (ADR-024 specified the *desktop*; the greeter was unspecified). Greeter-only: nothing here changes the desktop's density rules or the §5 layout laws.

### 12.1 Principle — presence, not persona

The Threshold shows the machine's intelligence as **ambient light, not a personality.** No avatar, no chat, no name that talks back — Bible Article 23.1 (no system AI persona) made visible. Where the industry bolts an assistant face onto the lock screen, NeoPlutarch shows the substrate *breathing*: intelligence as weather, not a character.

It is the one surface in the system that is **art-first.** The desktop *reports* (cyberpunk density, §5); the Threshold *is*. This fulfills ADR-024's "make the system visible" mandate in a serene register rather than contradicting it — the same substrate, shown at rest.

### 12.2 The Substrate Field (the hero)

A sparse, generative field of light — drifting particles and faint connections following a slow invisible flow. It reads as *tensor-flow* without ever drawing a literal node graph. Optionally seeded by real system entropy / idle-substrate state at session start, so it is **never identical twice and is unique to the machine** (ambient only — never a readable metric).

| Property | Law |
|---|---|
| **Palette** | Void (`#0a0e27`) base; **Electric Blue (`#3c8ef4`) dominant** as *light*, not data. Rare, low-saturation Magenta (`#ff00ff`) for depth only. **Lime/Amber are forbidden here** — they are desktop semantic colors (§ Semantic); the greeter stays monochromatic electric-blue for serenity. |
| **Motion** | Breath cycle **4–6 s** (target 5 s). **Ease-in-out only — no linear, nothing fast.** Slowness is the discipline that keeps it art, not screensaver. |
| **Density** | Sparse. **≥ ~70% of the frame stays near-Void.** Negative space is mandatory, not leftover. |
| **Awareness** | Subtle cursor gravitational bias; faint per-keystroke "stir" (brightening breath) as the passphrase is typed; idle breath when untouched; circadian dim at night. The field is *aware of you* without being a toy. |

### 12.3 The Auth Pane (the one crafted surface)

A single floating pane of **luminous frosted glass** over the field; the field glows faintly *through* it (backlit), so the pane is part of the substrate, not a chrome box on top of it.

- **Soft depth is permitted here and only here:** gentle elevation, one edge catching Electric Blue light. This is the controlled dose of the soft-material/neumorphic feel — confined to the greeter, never bleeding into the desktop.
- **Contents:** identity (name/avatar), one passphrase/passkey field, a caret rendered as a single breathing Electric Blue point. Nothing else — no button row, no secondary chrome.
- **Contrast law (non-negotiable):** all text and affordances on the pane meet the §9 contrast minimums regardless of the field behind them — the pane provides the legible backing. The soft depth must never cost readability (the one place light-neumorphism fails, we do not).

### 12.4 Typography

- **Greeting voice: IBM Plex Sans** (humanist, warm) — Plex Mono is reserved for the desktop cockpit, so the threshold *feels* different from the working OS.
- **One line only:** a quiet greeting ("hello"), or the time/date, beautifully set with generous tracking. No persona phrasing, no first person — restraint is the no-persona stance made typographic.

### 12.5 Signature moments

| Moment | Behavior | Timing |
|---|---|---|
| **Ignite** (successful unlock) | The field *blooms* — light gathers, accelerates, and resolves into the desktop. Narrative: substrate **at-rest → at-work**. This single gesture *is* the meld between the soft greeter and the dense ADR-024 desktop — one substrate waking up, not two clashing styles. | 600–900 ms, eased, one gesture |
| **Rejection** (wrong passphrase) | **No red flash.** The field *recoils* — contracts, cools, dims, and settles. Beauty even in being told no. | ~400 ms |
| **Withdrawal** (repeated failure / lockout) | The whole field cools toward dark — the machine quietly withdrawing. Lockout = near-dark sleeping field. | — |
| **Lock / idle** (returning user) | Same field, dimmer and slower — sleeping. Wakes on input with a brightening breath. | — |

### 12.6 Render tiers & fallback

The Threshold is among the first things drawn, possibly before the full GPU pipeline is up. It degrades gracefully and never blocks auth.

- **Tier 0 — static** (boot / pre-GPU / reduced-motion): a still luminous Electric-Blue-on-Void gradient with the pane. **Still beautiful, zero motion.**
- **Tier 1 — live** (compositor up): the full living field.
- **Accessibility (§9 Motion):** reduced-motion forces Tier 0 (or a near-static single breath).
- **Performance budget:** the field is best-effort and *yields to input latency* — it must never delay interactive authentication.

### 12.7 Guardrails (do / don't)

| Do | Don't |
|---|---|
| Keep motion slow and eased | Animate fast, or use linear motion |
| Stay electric-blue-monochromatic | Introduce Lime/Amber/multi-hue variety |
| Protect negative space | Fill the field |
| Let the pane carry all legibility | Let soft depth reduce text contrast |
| Show intelligence as light/weather | Add an avatar, chatbot, or system persona |
| Keep it art-first — zero metrics | Put numbers, readouts, or chrome on the threshold |

### 12.8 Implementation notes

- **Stack** (per ADR-024): the field is a wgpu shader (particle/compute or fragment flow-field); the pane is egui. No new dependencies.
- **Seeding:** system RNG / idle counters at session start for the non-repeating field.
- **Greeter motion tokens** (extends §6):

```css
:root {
  --neo-greeter-breath-cycle: 5000ms;   /* substrate breath; range 4000–6000 */
  --neo-greeter-ignite:        750ms;    /* unlock bloom → desktop; ease-in-out */
  --neo-greeter-recoil:        400ms;    /* rejection recoil; ease-out */
  --neo-greeter-easing:        cubic-bezier(0.4, 0.0, 0.2, 1.0);  /* no linear */
  --neo-greeter-field-min-void: 0.70;    /* ≥70% of frame stays near-Void */
}
```

---

## Feedback & Iteration

This document is a living specification. Feedback on the design system should be submitted as:

1. **Minor updates:** GitHub issue with label `design-system`
2. **Major changes:** ADR (triggers Bible amendment if needed per Article 27)

Phase 8 MVP ships with this spec as-is. Phase 8.5+ can evolve it.

---

**Document end.**

*Owned by: Design Lead. Governed by: ADR-024. Version: 1.1 (2026-06-03) — adds §12 The Threshold (login greeter).*
