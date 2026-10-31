<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# logansos.com

A static landing page in the Atrium design system (`docs/design-system.md`). No build step; deploy this folder as-is.

- `emblem-ring.png` and `emblem-star.png` are the logo's two layers, split exactly as `tools/branding/generate.py` splits
  Arthur's busy emblem, but at 336 px. The ring turns clockwise round the still star once every 3 s, as in the OS
  (`userland/neo-atrium/src/busy.rs`), and stops under Reduce Motion.
- Deploy with Cloudflare Pages: connect the repo (build command: none; output directory: `site`), or run
  `npx wrangler pages deploy site --project-name logansos`. Then add `logansos.com` as the project's custom domain.
