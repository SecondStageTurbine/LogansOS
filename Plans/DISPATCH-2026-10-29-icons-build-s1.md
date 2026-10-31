<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: put J.c.'s chosen icon set into the build and onto the Spine chips → S1

**Why.** J.c. picked the set on 2026-10-29 (`Plans/SYNTHESIS-2026-10-29-icons.md`, "J.c.'s decision"):

**Arthur A · Lens A · Terminal A · Files A · Settings A (the sliders) · Install B · Browser B (the globe).**

"A" is your set, `docs/design/icons-2026-10-29/s1/`. "B" is Jarvis's, `docs/design/icons-2026-10-29/jarvis/`.

## 1. The final masters (assets, no code)

- Put the chosen set in `assets/icons/`, plus an `assets/icons/README.md`. It holds `<name>.svg` (40), `<name>-24.svg`,
  `<name>-16.svg` and `<name>-mono.svg` for all seven.
- **Install and Browser are redrawn by you**, in your set's weight and colour rule:
  - keep Jarvis's *shapes* (the thin arrow into a tray, the globe with meridians);
  - Silver (or White) carries the shape;
  - Electric Blue is only an accent.

  The reason: Jarvis's all-blue outlines are 2.8:1 on the frosted chip, under the 3:1 floor. J.c. knows this and
  can ask for the all-blue version back.

  Credit Jarvis as the shape's designer in the README and the SVG comments.
- Every SVG gets the SPDX line (`<!-- SPDX-License-Identifier: MPL-2.0 -->`) and keeps the brief's renderable subset:
  no filters, gradients, masks, text or images.
- Regenerate `sheet.png` and `spine.png` for the final set under `docs/design/icons-2026-10-29/final/`.

## 2. Rasterise at build time (xtask, host)

- The OS has no SVG renderer. Follow the busy-emblem precedent (`assets/branding/README.md`, "Busy emblem layers"):
  - xtask produces fixed-size straight-alpha RGBA8888 raw blobs at 40, 24 and 16 px;
  - they're committed, or generated, the same way the busy-emblem layers are. Say which.
- Pick a rasteriser that is host-only and has a compatible licence (`resvg`/`tiny-skia` are MPL-2.0 / BSD-3). Do the
  survey rule: licence, last commit and SHA.
- **Byte oracle.** One hand-checkable KAT: a tiny SVG, for example a 4×4 filled rect, rasterises to the exact
  expected bytes. Then a pin of each final blob's BLAKE3, so a renderer bump can't silently change the pixels.

## 3. On the Spine (userland)

- Behind a new feature, `atrium-app-icons`. **Off ⇒ today's lettered chips, bit for bit.** Give the binary `.text`
  identity for neo-init and neo-atrium.
- **On:** the Arthur and Lens chips draw their 40 px icon, centred in the 56×56 chip, instead of the letter.
  - The other five have no app yet. Ship their blobs, but don't put chips for them on the Spine.
  - Keep each chip's label (accessibility). Keep the focus glow and the selected-chip state exactly as today.
  - Under Reduce-Transparency, the icon draws on the opaque chip.
- **Scenario:** extend the desktop scenario that already reads back Home's pixels (DS-1's) so it asserts each chip's
  pixels equal the pinned blob composited over the chip. Show the assertion fails with the wrong blob.
- Don't add `atrium-app-icons` to the product image. The PM adds it once the review passes and J.c. has seen a
  screenshot from the scenario.

## 4. Docs

Replace `docs/design-system.md` §8 with your proposed replacement from `THINK-icons-s1.md`, plus Jarvis's line that
Magenta, lime and amber are never identity colours. Update the stroke and size facts for the final set.

## Rules

- `scripts/worker-close-gate.sh`, one PR, branch `claude/atrium-app-icons`. It touches userland and xtask, so the
  PM's 4-shard gate is needed.
- Prior-art table in the PR: the rasteriser candidates.
- Review after merge: a Codex read (GPT-6-Luna high, contained code) of the rasteriser KAT, the kill-switch and the
  chip compositing.
- Send the PM the head SHA from `git ls-remote`.

```
WORKTREE: cloud session S1 (your own clone)
BRANCH:   claude/atrium-app-icons
          git fetch origin && git checkout -B claude/atrium-app-icons origin/main
git show origin/main:Plans/DISPATCH-2026-10-29-icons-build-s1.md
git show origin/main:Plans/SYNTHESIS-2026-10-29-icons.md
```

RAG isn't available in the cloud session. Before ending, `git status --porcelain` and `git log @{u}..` are both empty.
