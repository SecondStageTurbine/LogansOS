<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# LogansOS app icons

The first app-icon set, chosen by J.c. on 2026-10-29 (`Plans/SYNTHESIS-2026-10-29-icons.md`). The design record is
`docs/design/icons-2026-10-29/`; the rules are `docs/design-system.md` §8.

| Icon | Shape designed by | Notes |
|---|---|---|
| Arthur | S1 (Claude) | A four-point spark beside one open arc, with a live dot. |
| Lens | S1 (Claude) | A magnifier. |
| Terminal | S1 (Claude) | The bare `neo>` chevron and a cursor; no window frame. |
| Files | S1 (Claude) | A tabbed folder. |
| Settings | S1 (Claude) | Three sliders. |
| Install | **Jarvis (Codex)** | A thin arrow into a shallow tray on a wider base. Redrawn by S1 at the set's weights and colour rule. |
| Browser | **Jarvis (Codex)** | A globe with an equator and two meridians. Redrawn by S1 at the set's weights and colour rule. |

Jarvis drew Install and Browser with all-blue outlines. Those are 2.8:1 on the frosted Spine chip, under the 3:1 floor,
so the redraws keep his shapes but put Silver or White on the shape-carrying lines and Electric Blue on one accent.
J.c. can ask for the all-blue versions back (`docs/design/icons-2026-10-29/jarvis/svg/`).

## Files

For each icon `<name>`:
- `<name>.svg`: the 40×40 master (the Spine chip's size).
- `<name>-24.svg`, `<name>-16.svg`: hand-hinted small sizes, which drop detail rather than shrink it.
- `<name>-mono.svg`: the one-colour master. Electric Blue and White become Silver, and dark fills become the ground.

Plain SVG, written by hand: paths, circles and rects, solid colours. No filters, gradients, masks, text, images or
transforms. Licensed MPL-2.0, like the rest of the repository's assets.

## Rasterised blobs (generated; do not hand-edit)

The OS has no SVG renderer. `cargo xtask icons` runs the host-only `tools/icon-raster` (resvg, default features off),
which rasterises each SVG at its own size to a raw blob:
- **Where:** `userland/neo-atrium/assets/icons/<name>-<size>.rgba`.
- **Format:** `size * size * 4` bytes, row-major RGBA8888, straight (not premultiplied) alpha, no header.

The blobs are committed, like the busy emblem's layers. `cargo xtask icons --check` fails if any is stale, and
icon-raster's tests pin each blob's BLAKE3, so a renderer or SVG change can't move a pixel silently. After changing an
SVG: run `cargo xtask icons`, then update the pins in `tools/icon-raster/src/tests.rs` from
`cargo run -p icon-raster -- check`'s output.

**Where they're used:** neo-atrium's `app-icons` feature (neo-init's `atrium-app-icons`) puts the 40 px Arthur and
Lens icons on their Spine chips.
