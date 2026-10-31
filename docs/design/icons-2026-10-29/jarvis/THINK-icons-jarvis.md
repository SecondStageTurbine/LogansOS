<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: LogansOS app icons — Jarvis

**Date:** 2026-10-29  
**Seat:** Jarvis (Codex family), independent design pass  
**Icon license:** MPL-2.0  
**Scope:** Seven app identities and their 40, 24, and 16 px vector masters. No application code.

## Idea

This set gives each app a compact object silhouette, with silver carrying structure and Electric Blue marking its defining action or inner light. Arthur is a small faceted light prism with a few rays, so it suggests liveness without a face and stays separate from the OS flare-ring. The icons keep a calm, crafted outline at rest; hue supports recognition but every icon retains its shape in one-color silver.

## The seven silhouettes

| App | Silhouette | Recognition without label or color |
|---|---|---|
| **Arthur** | A tall faceted diamond prism, crossed by a narrow light axis, with three short rays and a grounded stem. | The only upright crystalline core with rays; it has no face, enclosing ring, or OS emblem. |
| **Lens** | A circular glass with a short diagonal handle and a small focus point. | The only circle with an attached handle; the browser has a complete globe and no handle. |
| **Terminal** | An open command rail: framed horizontal window, chevron prompt, and cursor bar. | The only rectangular instrument with a prompt and baseline. |
| **Files** | A broad tabbed folder with a separate shelf line. | The only silhouette with a lifted tab and wide base. |
| **Settings** | An eight-lobed adjustment wheel with a clear center opening. | The only toothed circular perimeter; its center hole contrasts with the Lens glass and globe grid. |
| **Install** | A descending arrow that enters an open tray, with the tray’s grounded bottom edge. | The only vertical action arrow meeting a horizontal receiver. |
| **Browser** | A full globe with an equator and two curved meridians. | The only enclosed sphere divided by continuous geographic lines. |

The 16 px drawings remove fine interior detail where it would crowd the silhouette: Arthur keeps its core and rays; Lens its glass and handle; Terminal its prompt and cursor; Files its tab; Settings its wheel and bore; Install its arrow and tray; Browser its globe and axes. The 24 px files retain the 40 px construction with a stroke that scales with the artboard. Each 16 px file uses a direct 16-unit drawing with simplified, pixel-aligned geometry.

## Construction rules

- **Grid:** the 40 px master uses an 8 px base grid, with 4 px subdivisions and optical exceptions for the diagonal arrow, lens handle, and curved globe. Shapes stay inside a 2 px safe margin.
- **Stroke:** 2 px on the 40 px master. The 24 px artwork uses the same 40-unit construction with a 2 px source stroke, which scales to 1.2 px at output. The 16 px drawings use a 1 px source stroke on a 16-unit artboard. This preserves a shared outline voice while preventing hairlines at small sizes.
- **Color:** Electric Blue `#3c8ef4` is a small action/light accent; Silver `#a8adb2` carries the main form. No identity hue depends on blue. There are no large flat blue fills, gradients, masks, filters, or text in the SVGs.
- **Backgrounds:** SVGs are transparent. The sheet and Spine mock composite a 7% white frost and 10% Electric Blue tint over Dark 1 (`#1a2332`), on Void (`#0a0e27`). Neither background is part of the icon art.
- **Raster renderer:** Google Chrome headless, at device scale factor 1, renders the sheet and Spine HTML previews. The blurred row uses CSS `blur(4px)` on the 24 px art in the sheet only. The SVG files themselves stay within the OS-supported paths, circles, fills, and strokes.

## Prior art survey

Checked 2026-10-29. SHAs are the fetched `HEAD` commit IDs checked with `git ls-remote`; dates are the corresponding commit dates. Study informed only construction principles; no paths were copied.

| Name | License | Last commit date | SHA | Principle taken |
|---|---|---:|---|---|
| [Lucide](https://github.com/lucide-icons/lucide) | ISC | 2026-10-28 | `be09619c1c1124ad09c19fc3e87ee3b939409bbd` | A consistent stroke vocabulary can coexist with simple, readable silhouettes. |
| [Feather](https://github.com/feathericons/feather) | MIT | 2025-03-10 | `3dc050d97405062eba78aa57115c0a15c63abdaa` | Restraint and open negative space help icons stay legible when small. |
| [Phosphor](https://github.com/phosphor-icons/core) | MIT | 2026-01-06 | `2b75f3ad12b420c9504ef05df8d2564a28f8500e` | A shared base construction can carry varied weights and sizes. |
| [Tabler](https://github.com/tabler/tabler-icons) | MIT | 2026-10-27 | `ed177a085b652fe04ec4f37ced867c94621e7840` | Precise grid placement and repeatable line weight aid small-scale clarity. |
| [Material Symbols](https://github.com/google/material-design-icons) | Apache-2.0 | 2026-10-23 | `10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7` | Familiar object metaphors should survive adjustments to visual weight. |
| [Original 1984 Kare Mac icons](https://web.stanford.edu/dept/SUL/sites/mac/primary/interviews/kare/index.html) | Proprietary; principle study only | 1984 (no source-control commit) | N/A | Distinct, economical silhouettes can make a small monochrome sign memorable. |
| [elementary OS HIG](https://github.com/elementary/hig) | No license declared in the repository | 2026-09-30 | `bdd2cdb6235c362f3d1f05fef81b9ceb7f168fdd` | App identity is allowed to use its own form and character within the desktop’s shared craft. |
| [GNOME HIG icon guidance](https://gitlab.gnome.org/Teams/Websites/developer.gnome.org-hig) | CC-BY-SA-4.0 | 2026-06-20 | `97028eac37922e787cad5796215f7e986451883a` | At small sizes, favor simple, monochrome symbolic forms and reuse only broadly recognizable conventions. |

## Proposed replacement for `docs/design-system.md` §8

### 8. Icon System

#### Identity

App icons use distinct silhouettes, not a shared rounded-rectangle container. Give each app a recognizable object or action shape that survives monochrome display and the five-year-old test with labels blurred. Structure uses Silver (`#a8adb2`); Electric Blue (`#3c8ef4`) marks light or action and is never a large flat fill. Magenta, lime, and amber remain reserved for their system meanings and are not identity colors.

Arthur represents light and liveness, never a face or persona. Its icon must not reuse the LogansOS star-and-broken-ring mark, which belongs to the operating system.

#### Sizes and geometry

Deliver a hand-built 40 × 40 px master for the 56 × 56 Spine chip, with a 2 px safe margin, plus optical 24 × 24 px and 16 × 16 px versions for menus and compact lists. Use an 8 px base grid with 4 px subdivisions and deliberate optical corrections. Use a consistent 2 px master outline; adjust small artwork to retain clear strokes and open counters rather than scaling blindly.

#### Color and backgrounds

Icons have transparent backgrounds and must read on Void (`#0a0e27`) and Dark 1 (`#1a2332`) under the Spine’s light frost and approximately 10% Electric Blue tint. Every icon must also pass in one-color Silver on Void. At 24 px, maintain at least 3:1 contrast against Dark 1. Meaning must not rely on hue alone.

#### SVG implementation limits

Use only paths, circles, rectangles, fills, strokes, solid colors, and straight-alpha opacity. Do not use filters, blur, masks, gradients, text, or embedded images in icon files. SVGs are rasterized at build time to RGBA; put optical blur or other previews only in design-sheet rendering.

#### Review

Present the full set at 40, 24, and 16 px on Void, on the frosted Dark 1 chip, and as Silver monochrome. Include a 4 px blur of the 24 px row and inspect silhouettes at the actual Spine scale. Distinctness and small-size legibility are required before an icon is accepted.
