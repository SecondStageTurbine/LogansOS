<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: the first LogansOS app-icon set — two independent design passes → S1 (Claude) and Jarvis (Codex)

**Why.** J.c. asked for it on 2026-10-29. The Atrium's look is decided, but its app icons aren't. Today the Spine's
launcher chips are lettered tiles ("A", "L"). The only icon spec is the old Phase-8 `docs/design-system.md` §8 (a
24×24 grid, 2 px stroke, a cyberpunk list of examples), and it was never revisited for the Atrium.

This pass designs the first real set. **The look is J.c.'s call, so this is a convergence exercise:**
- two seats from **different model families** design independently;
- the PM puts both sets side by side;
- J.c. picks per icon, or picks one set and grafts from the other.

Cross-family is the point (CLAUDE.md §Cross-model diversity). Agreement between the two is the strong signal, and
where they differ, that's a real choice for J.c.

**Independence is required.** Don't read the other seat's branch or output until you've pushed your own.

## What to read first (the decided design; don't reopen it)

- `docs/THINK-2026-07-10-atrium-design-language.md`. §2 is the distinctness rule: every kind of thing gets its own
  silhouette, it's "classic-Mac / Kare iconography, legible shape, **not a forced rounded-rect**", and there's no
  normalising pass. §6 is the tokens and type.
- `docs/THINK-2026-07-19-atrium-glass-frost-amendment.md`. Trust is carried by glass tint, halo geometry and labels,
  **never** by the icon. An icon is identity, not trust state.
- `docs/THINK-2026-07-08-gui-vision-SYNTHESIS.md` §6: J.c. ratified "reconcile" on 2026-07-08, meaning a classic-Mac
  craft bar, neon only as meaning, and a calm rest state.
- `docs/THINK-2026-10-18-ds1-home-spine-shell.md`, and the Spine geometry in `userland/neo-atrium/src/lib.rs`
  (spec v1.8): the Spine is 84 px wide, chips are 56×56 with radius 10, the Void is `#0a0e27`.
- `assets/branding/README.md` and `assets/branding/logansos-logo-512.png`: the OS mark is a blue lens-flare star
  inside broken silver rings. The busy emblem (A9-D) turns that ring under the still star.
- Screenshots: `docs/screenshots/desktop-ds1-home.png`, `desktop-ds1-arthur-held.png`.

## The set: seven icons

| Icon | What it is | Note |
|---|---|---|
| **Arthur** | the AI assistant | Must read as Arthur, not as the OS. Relate it to the star-and-ring mark, but don't reuse the mark itself; the mark is the OS. Arthur is "light and liveness, never a face" (`docs/VISION-arthur-persona.md`). |
| **Lens** | the read-only inspector window | Its live window has a dashed halo. |
| **Terminal** | `neo>`, the command rail | OT-88 |
| **Files** | the file manager | OT-88 |
| **Settings** | settings | OT-88 |
| **Install** | "Install LogansOS", the OT-87 installer | Shown only on the Try boot. |
| **Browser** | the future browser (OT-61) | |

Each one must pass the five-year-old test from the design language: tell it apart from the other six with the labels
blurred and the color removed.

## Constraints

1. **Tokens only.**
   - Electric Blue `#3c8ef4` is for light and accent, never large flat fills.
   - Silver `#a8adb2` is for structure.
   - White `#ffffff`, Dark 1 `#1a2332`, Dark 2 `#0f1219`, Gray 1 `#4a5568` and Gray 2 `#2d3748` are allowed.
   - Magenta, lime and amber are **not** used in identity icons; they mean deny, pass and warn.
   - Every icon also has a **one-color version** (Silver on Void) that still passes the five-year-old test.
2. **Sizes.** Design for the Spine chip: the icon sits inside the 56×56 chip, so draw at **40×40** with a 2 px safe
   margin. Also deliver **24×24** (lists and menus) and **16×16** (small). Hint the small sizes by hand where they need
   it; don't just scale down.
3. **Grid and stroke.** Use an 8 px grid on the 40 px master, with sub-grid where you need it. You choose the stroke
   weight, but keep it consistent across the set, and say what it is.
4. **Renderable by us.** We have no SVG renderer in the OS. The icons will be rasterised at build time to raw RGBA,
   the same way the busy emblem's layers are, so keep them to simple shapes:
   - **allowed:** paths (lines, arcs, cubic and quadratic curves), circles and rects, fills and strokes, solid colours
     and straight-alpha opacity;
   - **not allowed:** filters, blur, masks, gradients, text or embedded images.
5. **Backgrounds.** Each icon must work on the Void, and on the Dark 1 chip under owned glass: light frost plus about
   10% Electric Blue.
6. **Accessibility.** No meaning in hue alone. At 24 px the shape must read at 3:1 against Dark 1.

## Prior art (the survey rule, CLAUDE.md 2026-10-27)

Before drawing, survey the open icon sets and the Mac tradition. **Study them for principle; don't copy paths.** Our
icons are original work, licensed MPL-2.0 like the rest of the repo's assets.

In your THINK, give a table with these columns: name, license, last commit date and SHA (checked with `git`), and
what you took (principle only). At least these rows:
- Lucide
- Feather
- Phosphor
- Tabler
- Material Symbols
- the original 1984 Kare Mac icons (proprietary: principle only)
- elementary OS and GNOME HIG icon guidance

## Deliverables (one push per seat)

Under `docs/design/icons-2026-10-29/<seat>/`, where `<seat>` is `s1` or `jarvis`:
- `svg/<name>.svg`, seven files: the 40×40 master with a `viewBox="0 0 40 40"`, plus `svg/<name>-24.svg` and
  `svg/<name>-16.svg` where you hand-hinted. Plain SVG, hand-written, readable, no editor metadata.
- `sheet.png`: a contact sheet with every icon at 40, 24 and 16 px in four rows:
  - full colour on the Void;
  - full colour on the Dark 1 chip under the frost tint;
  - one-colour Silver on the Void;
  - a blurred row (a 4 px blur of the 24 px row) for the five-year-old test.
- `spine.png`: a mock of the 84 px Spine with all seven chips, on the Void, at 1:1.
- `THINK-icons-<seat>.md`, CC-BY-4.0. It gives:
  - the idea behind the set in three sentences;
  - per icon, the silhouette and why it can't be confused with the others;
  - the stroke and grid rules you used;
  - the prior-art table;
  - what you'd change in `docs/design-system.md` §8 to replace the old spec. Write it as a proposed replacement
    section.

**Rendering.** Use any tool, and name it in the THINK.
- The cloud seat has headless Chromium and Playwright preinstalled: render an HTML page of the SVGs and screenshot it.
- Jarvis can use whatever is on the PL's machine.
- The blur row may use a filter in the *sheet*, just never in the icons themselves.

## What happens next (PM)

1. The PM reads both pushes and builds `docs/design/icons-2026-10-29/compare.png`: both sets row by row, per icon.
2. The PM writes `Plans/SYNTHESIS-2026-10-29-icons.md`: where the two converge, where they differ, and a
   recommendation per icon.
3. **J.c. picks.** Nothing ships until he does.
4. Then a follow-up build lane rasterises the chosen set to RGBA in xtask, puts it on the Spine chips behind a feature
   switch, and replaces §8.

## Rules

- Docs and assets only. No code, so no gate.
- Commit and push at every checkpoint. Before ending, `git status --porcelain` and `git log @{u}..` are both empty.
- Report back with the branch head from `git ls-remote`.

### S1 (Claude family, cloud)

```
WORKTREE: cloud session S1 (your own clone)
BRANCH:   claude/icons-2026-10-29-s1
          git fetch origin && git checkout -B claude/icons-2026-10-29-s1 origin/main
git show origin/main:Plans/DISPATCH-2026-10-29-icon-design-pass.md
```

RAG isn't available in the cloud session; read the files directly.

### Jarvis (Codex family, via J.c.)

```
MODEL: GPT-6-Luna. EFFORT: high. (A design task, not a review. Luna high is enough for drawing and reasoning about
seven icons; Sol isn't needed.)
WORKTREE: E:\AI Automation\workers\jarvis   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/icons-2026-10-29-jarvis
          git fetch origin && git checkout -B codex/icons-2026-10-29-jarvis origin/main
git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-icon-design-pass.md
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's
down, note that in your THINK and read the files directly; it's never a blocker.
