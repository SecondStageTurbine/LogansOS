# LogansOS Branding

The LogansOS identity is a **blue lens-flare star** centered inside **broken
silver ring arcs** (solid + dotted segments), with the **LogansOS** wordmark and
a thin flare divider below. It is *not* a circular badge — there is no circular
crop or alpha mask anywhere in the pipeline. It follows the project design
language in `docs/design-system.md` (Electric Blue `#3c8ef4` primary, Silver
`#a8adb2` structure, Void background).

## Sources (hand-authored — never regenerated)

Two committed masters, both 1254×1254 8-bit RGBA:

- `LogansOS.png`: the **black-background master** — the full emblem + wordmark on
  opaque black. Source for the kernel boot splash.
- `logansos-transparent.png`: the **transparent master** — the same artwork with a
  transparent background. Source for the documentation/UI PNG ladder.

## Generated variants (do not hand-edit — re-run the generator)

Derived from the transparent master by alpha-weighted box downscale (no mask):

- `logansos-logo-512.png`: 512×512 RGBA, README and documentation display.
- `logansos-logo-256.png`: 256×256 RGBA, compact documentation and UI use.
- `logansos-icon-128.png`: 128×128 RGBA, small icon use.

Kernel framebuffer splash derivative (from the black-background master):

- `../../kernel/assets/logansos_boot_logo_160.rgba`: a 160×160 RGBA8888 raw blob
  (no header, exactly `160 * 160 * 4 = 102400` bytes) embedded by
  `kernel/src/init/boot_logo.rs`. The kernel does not decode PNG during early
  boot, so this is a preprocessed raw asset. It is the **ring+star emblem cropped
  out of the black-bg master** — the wordmark is excluded on purpose: at 160px the
  "LogansOS" text is illegibly small, so the boot splash shows the emblem alone.
  Its background is kept **opaque black**: the framebuffer is cleared to black
  before the blit and `fb::blit_rgba` writes opaque pixels as-is, so black-on-black
  is seamless. The black-bg master is composited over black so any source
  transparency collapses to the intended opaque emblem-on-black.

Busy emblem layers (A9-D), from the transparent master:

- `../../userland/neo-atrium/assets/busy-emblem-star-88.rgba` and
  `busy-emblem-ring-88.rgba`: 88×88 RGBA8888 raw, straight alpha (`88 * 88 * 4 =
  30976` bytes each). A 672 px square centred on the ring (a least-squares circle
  fit to its silver strokes, so the ring turns in place) is split into the still
  star (a central disc plus the horizontal flare) and the ring arcs, with the
  wordmark rows cleared, then downscaled. `userland/neo-atrium/build.rs` rotates
  the ring under the star into the 36 frames, 44×44, that the WM shows while
  Arthur thinks.

## Regenerating (authoritative method)

All ladder variants and the boot blob are produced by `tools/branding/generate.py`,
which uses only the Python standard library (`zlib` + `struct`) — no Pillow or
ImageMagick install required:

```sh
python3 tools/branding/generate.py
```

It re-derives the generated files above from the two source masters and
asserts the boot blob is exactly 102400 bytes. The two `*.png` masters are
**inputs** and are left untouched. `boot_logo.rs` carries a `const` assertion
(`WIDTH * HEIGHT * 4 == RGBA.len()`) so a wrong-sized boot blob fails to compile
rather than rendering garbage.

## Lineage

The project was renamed AltimitNeo → NeoPlutarch → LogansOS (the LogansOS rename
is brand-only; internal codenames such as the `neoplutarch-kernel` crate and the
`.neom` model format are intentionally kept — see Bible Article 25.2). The first
LogansOS crest was a cyan/magenta circular neon badge; the 2026-06 refresh
replaced it with the current blue lens-flare star + silver ring identity and
retired the circular-mask pipeline along with it.
