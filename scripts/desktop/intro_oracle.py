#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Independent oracle for the desktop-intro cards (scene spec v1.8).

Written from the spec text in userland/neo-atrium/src/lib.rs ("Intro card")
and the wording the PL approved (Plans/NOTE-2026-10-20-arthur-first-boot-
intro-s2.md and its PM decision), typed here again rather than read from the
Rust: the host test compares the two.

* The scene: scripts/desktop/scene_oracle.py renders Home with the card
  panel (numpy, no neo-atrium code). Digests mask the Home label boxes plus
  the state's text box (the card's, or the hint's).
* The words: per text row, a column pattern over the text box from exact-area
  glyph coverage (scripts/fonts/area_oracle: fontTools outlines, neo-text's
  specified stem darkening and 1/4-px origins). A column is `#` when some
  pixel of the row's band is certainly at least half covered, `.` when none
  can be, `?` near the threshold. A capture's column is inked when a band
  pixel's green reaches the row's threshold (the half-coverage blend over the
  row's background). A glyph whose pen lies exactly on a half bin (a float
  pen may round either way there) is taken at both bins, and only columns
  both agree on are certain. Different words, a missing row or a shifted row
  fail it;
  the tables stay small (one character per column) where per-pixel bounds
  for thirteen rows would not.
* The field check with the hint shown: the DS-1a field digest (placeholder
  and caret window, scripts/desktop/field_oracle.py) with the hint box
  masked too.

usage: intro_oracle.py fixture > fixtures/desktop-intro/intro.rs
"""
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "fonts"))
sys.path.insert(0, HERE)

import area_oracle as ao  # noqa: E402
import field_oracle as fo  # noqa: E402
import label_oracle as lo  # noqa: E402
import scene_oracle as so  # noqa: E402

# The approved wording (variant A, USB-only, model ready: seven rows).
INTRO = [
    "Good day. I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.",
    "The household side comes later; for now, I look after this computer for you.",
    "I work entirely on this machine, and I don't send what you tell me anywhere.",
    "On this machine I can't keep anything once it's switched off, not yet.",
    "Ask in your own words: \"What does a CPU do?\", for instance.",
    "\"list /\" shows you the files on this machine.",
    "Type help for what you can do yourself. Esc always brings you back here.",
]
HELP = [
    "What you can do yourself, without asking me:",
    "Esc sets me aside and brings you back to Home.",
    "\"list /\" opens the Lens: a read-only view of the files on this machine.",
    "The chips on the left switch between me and the Lens; Tab reaches them.",
    "Drag a window by its title bar to move it, or by an edge to resize it.",
    "Start typing to ask me something, or press Esc to close this card.",
]
HINT = "Type help, or click here, for what you can do yourself."
CARD_PX, HINT_PX = 15.0, 14.0
# Certainly inked / certainly blank, as darkened coverage bytes (the 128
# threshold with a margin for the slack and, on the hint's backdrop, for the
# background varying along the row).
SURE_INK, SURE_BLANK = 150, 106


def esc(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def rows_spec(face, L, rows, hint):
    """(text, px, x0, baseline, colour, band) per row."""
    if hint:
        box = so.card_text(L, 1)
        w = lo.measure(face, HINT_PX, HINT)
        x0 = box[0] + (box[2] - w) / 2.0
        card = so.card_rect(L, 1)
        return [(HINT, HINT_PX, x0, card[1] + 30, lo.SECONDARY, box)]
    card = so.card_rect(L, len(rows))
    box = so.card_text(L, len(rows))
    out = []
    for i, text in enumerate(rows):
        band = (box[0], box[1] + 24 * i, box[2], 24)
        out.append((text, CARD_PX, float(box[0]), card[1] + 30 + 24 * i, lo.PRIMARY, band))
    return out


def origins(face, px, text, x0):
    """[(gid, [origin x, ...])] per glyph: the spec's nearest 1/4 px, and
    at an exact tie (a pen on a half bin, where a float pen may land either
    side) both neighbouring bins — both roundings are legitimate there."""
    scale = px / face.upem
    pen, prev, out = 0.0, None, []
    for ch in text:
        gid = face.gid(ch)
        if prev is not None:
            pen += face.kern(prev, gid) * scale
        q = (x0 + pen) * 4
        ix, b = ao.quantize(x0 + pen)
        xs = [ix + b / 4.0]
        if abs(q - math.floor(q) - 0.5) < 1e-3:
            xs = [math.floor(q) / 4.0, (math.floor(q) + 1) / 4.0]
        out.append((gid, xs))
        pen += face.advance(gid) * scale
        prev = gid
    return out


def column_max(face, px, glyphs, base, band, pick):
    """Each column's greatest pixel coverage in `band` with the glyphs at
    the origins `pick` chooses."""
    bx, by, bw, bh = band
    total = {}
    for gid, xs in glyphs:
        for k, a in ao.glyph_coverage(face, gid, px, pick(xs), base).items():
            if lo.inside(band, *k):
                total[k] = min(255, total.get(k, 0) + a)
    col = [0] * bw
    for (x, y), a in total.items():
        col[x - bx] = max(col[x - bx], a)
    return col


def pattern(face, rgb, spec):
    """(band, threshold green, pattern string) for one row."""
    text, px, x0, base, fg, band = spec
    bx, by, bw, bh = band
    glyphs = origins(face, px, text, x0)
    cols = [column_max(face, px, glyphs, base, band, lambda xs: xs[0]),
            column_max(face, px, glyphs, base, band, lambda xs: xs[-1])]
    chars = []
    for ms in zip(*cols):
        if all(ao.darkened(max(0, m - lo.SLACK), px) >= SURE_INK for m in ms):
            chars.append("#")
        elif all(ao.darkened(min(255, m + lo.SLACK), px) <= SURE_BLANK for m in ms):
            chars.append(".")
        else:
            chars.append("?")
    # The row's background: its band's mean green in the scene oracle's
    # render (flat content floor on a card; the backdrop under the hint).
    g = rgb[by:by + bh, bx:bx + bw, 1]
    bg = int(round(float(g.mean())))
    thr = (ao.blend(bg << 8, fg, 128) >> 8) & 0xFF
    return band, thr, "".join(chars)


def fixture():
    face = ao.Face(lo.FONT)
    W, H = 1920, 1080
    L = so.layout(W, H)
    home = L["home_labels"]
    print("// SPDX-License-Identifier: MPL-2.0")
    print("// GENERATED by scripts/desktop/intro_oracle.py fixture — do not edit.")
    print("// desktop-intro (scene spec v1.8): the Home view with the hint, the intro")
    print("// card and the help card. Digests: FNV-1a over every visible pixel, the")
    print("// three Home label boxes and the state's text box hashed as zero. Rows: per")
    print("// text row, its band, the green threshold and a column pattern from exact-")
    print("// area glyph coverage (# inked, . blank, ? either). Independent of neo-atrium")
    print("// and neo-text.")
    print("pub const INTRO_ROWS: [&str; %d] = [" % len(INTRO))
    for r in INTRO:
        print('    "%s",' % esc(r))
    print("];")
    print("pub const HELP_ROWS: [&str; %d] = [" % len(HELP))
    for r in HELP:
        print('    "%s",' % esc(r))
    print("];")
    print('pub const HINT: &str = "%s";' % esc(HINT))
    states = [("HINT", 0, so.card_text(L, 1)),
              ("INTRO", len(INTRO), so.card_text(L, len(INTRO))),
              ("HELP", len(HELP), so.card_text(L, len(HELP)))]
    for name, n, box in states:
        print("/// The %s text box at 1920x1080 (masked below)." % name.lower())
        print("pub const %s_TEXT_BOX: (usize, usize, usize, usize) = (%d, %d, %d, %d);"
              % ((name,) + box))
    for name, n in (("INTRO", len(INTRO)), ("HELP", len(HELP))):
        print("pub const %s_CARD: (usize, usize, usize, usize) = (%d, %d, %d, %d);"
              % ((name,) + so.card_rect(L, n)))
    print("/// Home, the field focused and empty, per state and material; masks: the")
    print("/// Home label boxes, then the state's text box.")
    for (Wd, Hd) in ((1920, 1080), (1280, 800)):
        Ld = so.layout(Wd, Hd)
        for name, n, _box in states:
            box = so.card_text(Ld, max(n, 1))
            for mode in ("glass", "rt"):
                rgb, Lr = so.render(Wd, Hd, mode, True, "home", card_rows=n)
                d = so.digest(rgb, dict(Lr, labels=Lr["home_labels"] + [box]))
                print("pub const %s_%s_%dX%d: u64 = 0x%016x;" % (name, mode.upper(), Wd, Hd, d))
    # Arthur held behind Home: the hint shows there too.
    for mode in ("glass", "rt"):
        rgb, Lr = so.render(W, H, mode, True, "home", held=True)
        d = so.digest(rgb, dict(Lr, labels=Lr["home_labels"] + [so.card_text(L, 1)]))
        print("pub const HINT_%s_1920X1080_HELD: u64 = 0x%016x;" % (mode.upper(), d))
    # The DS-1a field check with the hint shown (the kernel's final Home).
    layers, fg, px, caret = fo.run(face, L, "", True)
    win = fo.window(layers, caret, fo.text_box(L))
    rgb, Lr = so.render(W, H, "glass", True, "home")
    d = so.digest(rgb, dict(Lr, labels=[home[0], home[2], win, so.card_text(L, 1)]))
    print("/// The field's empty-focused window (field_oracle) and the Glass Home")
    print("/// digest with only it, the bar and chip label boxes and the hint box masked.")
    print("pub const FIELD_WINDOW: (usize, usize, usize, usize) = (%d, %d, %d, %d);" % win)
    print("pub const HINT_FIELD_GLASS_1920X1080: u64 = 0x%016x;" % d)
    # The rows' column patterns, Glass 1920x1080.
    print("/// A text row: its band (x, y, w, h), green threshold, column pattern.")
    print("pub type Row = ((usize, usize, usize, usize), u8, &'static str);")
    for name, rows, n, hint in (("HINT", [], 0, True), ("INTRO", INTRO, len(INTRO), False),
                                ("HELP", HELP, len(HELP), False)):
        rgb, _ = so.render(W, H, "glass", True, "home", card_rows=n)
        specs = rows_spec(face, L, rows, hint)
        out = [pattern(face, rgb, s) for s in specs]
        print("/// %s rows." % name)
        print("pub const %s_PATTERNS: [Row; %d] = [" % (name, len(out)))
        for band, thr, pat in out:
            print("    ((%d, %d, %d, %d), %d, \"%s\")," % (band + (thr, pat)))
        print("];")


def main():
    if len(sys.argv) == 2 and sys.argv[1] == "fixture":
        fixture()
        return
    print(__doc__)
    sys.exit(2)


if __name__ == "__main__":
    main()
