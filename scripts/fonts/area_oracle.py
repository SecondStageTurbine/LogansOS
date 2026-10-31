#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Exact-area reference renderer for the neo-text byte oracles.

Shares no code with neo-text (Rust) and no rasterizer with anything:

* outlines: fontTools `glyf` points (TrueType quadratic B-splines, implied
  on-curve midpoints expanded here), scaled by ppem/unitsPerEm, y flipped;
* curves: flattened to 256 segments per quadratic (deviation < 1e-5 px at
  the sizes used), far finer than any coverage byte can resolve;
* coverage: for every pixel, each contour is clipped to the pixel square
  (Sutherland-Hodgman, f64) and its *signed* area summed - i.e. the exact
  non-zero-winding area for non-overlapping contours - then
  round_half_up(255 * min(|area|, 1));
* advances: fontTools `hmtx`; kerning: fontTools GPOS PairPos (`kern`
  feature lookups, first *matching* subtable per lookup; a Format-1 miss falls through);
* blending: IEC 61966-2-1 sRGB <-> linear in float.

The conventions it shares with neo-text are the *specified* ones: ppem = px,
glyph origins rounded to the nearest 1/4 px, linear-light blending.

Subcommands (Rust source fragment on stdout):
  glyph  <font.ttf> <char> <px> <x_offset>
  window <font.ttf> <px> <text> <x> <baseline> <bg> <fg> <wx> <wy> <ww> <wh>
  bounds-window <same as window> <slack> [darken_px]
                per-pixel [lo, hi] for coverage +/- slack; with darken_px < 16 the
                coverage first goes through neo-text's specified small-size stem
                darkening curve c' = c^(1/(1+k)), k = min((16-px)/8, 1)*0.3
"""
import math
import sys

from fontTools.ttLib import TTFont

SEGMENTS = 256


class Face:
    def __init__(self, path):
        self.tt = TTFont(path)
        self.path = path
        self.glyf = self.tt["glyf"]
        self.order = self.tt.getGlyphOrder()
        self.cmap = self.tt.getBestCmap()
        self.upem = self.tt["head"].unitsPerEm
        self._kern = None

    def gid(self, ch):
        name = self.cmap.get(ord(ch))
        return self.order.index(name) if name else 0

    def advance(self, gid):
        return self.tt["hmtx"][self.order[gid]][0]

    def contours(self, gid):
        """Closed polylines in font units (y up)."""
        g = self.glyf[self.order[gid]]
        if g.numberOfContours == 0:
            return []
        coords, ends, flags = g.getCoordinates(self.glyf)
        out, start = [], 0
        for end in ends:
            pts = [(coords[i][0], coords[i][1], flags[i] & 1) for i in range(start, end + 1)]
            start = end + 1
            out.append(flatten(pts))
        return out

    def kern(self, left, right):
        if self._kern is None:
            self._kern = build_kern(self.tt)
        return self._kern(self.order[left], self.order[right])


def flatten(pts):
    # Expand implied on-curve points, then rotate to start on-curve.
    full = []
    n = len(pts)
    for i in range(n):
        x, y, on = pts[i]
        nx, ny, non = pts[(i + 1) % n]
        full.append((x, y, on))
        if not on and not non:
            full.append(((x + nx) / 2, (y + ny) / 2, 1))
    k = next(i for i, p in enumerate(full) if p[2])
    full = full[k:] + full[:k]
    poly = []
    m = len(full)
    i = 0
    while i < m:
        x0, y0, _ = full[i]
        x1, y1, on1 = full[(i + 1) % m]
        if on1:
            poly.append((x0, y0))
            i += 1
        else:
            x2, y2, _ = full[(i + 2) % m]
            for s in range(SEGMENTS):
                t = s / SEGMENTS
                mt = 1 - t
                poly.append((mt * mt * x0 + 2 * mt * t * x1 + t * t * x2,
                             mt * mt * y0 + 2 * mt * t * y1 + t * t * y2))
            i += 2
    return poly


def build_kern(tt):
    gpos = tt["GPOS"].table
    lookups = sorted({i for fr in gpos.FeatureList.FeatureRecord
                      if fr.FeatureTag == "kern" for i in fr.Feature.LookupListIndex})

    def kern(left, right):
        total = 0
        for li in lookups:
            lookup = gpos.LookupList.Lookup[li]
            for st in lookup.SubTable:
                if lookup.LookupType == 9:
                    st = st.ExtSubTable
                if st.LookupType != 2 or left not in st.Coverage.glyphs:
                    continue
                value = 0
                if st.Format == 1:
                    # Format 1 matches only when the exact pair is listed; a
                    # miss means "try the next subtable" (OpenType GPOS spec).
                    pset = st.PairSet[st.Coverage.glyphs.index(left)]
                    rec = next((r for r in pset.PairValueRecord if r.SecondGlyph == right), None)
                    if rec is None:
                        continue
                    value = getattr(rec.Value1, "XAdvance", 0) or 0
                else:
                    c1 = st.ClassDef1.classDefs.get(left, 0)
                    c2 = st.ClassDef2.classDefs.get(right, 0)
                    v = st.Class1Record[c1].Class2Record[c2].Value1
                    value = (getattr(v, "XAdvance", 0) or 0) if v is not None else 0
                    if c2 == 0 and value == 0:
                        continue  # unclassified + zero record: fall through (HarfBuzz)
                total += value
                break
        return total

    return kern


def clip_signed_area(poly, px, py):
    pts = poly
    for axis, bound, keep_ge in ((0, px, True), (0, px + 1, False), (1, py, True), (1, py + 1, False)):
        if not pts:
            return 0.0
        out = []
        n = len(pts)
        for i in range(n):
            a, b = pts[i], pts[(i + 1) % n]
            ia = a[axis] >= bound if keep_ge else a[axis] <= bound
            ib = b[axis] >= bound if keep_ge else b[axis] <= bound
            if ia:
                out.append(a)
            if ia != ib:
                t = (bound - a[axis]) / (b[axis] - a[axis])
                out.append((a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t))
        pts = out
    s = 0.0
    for i in range(len(pts)):
        a, b = pts[i], pts[(i + 1) % len(pts)]
        s += a[0] * b[1] - b[0] * a[1]
    return s / 2.0


def glyph_coverage(face, gid, px, ox, oy):
    """{(x, y): coverage byte} of glyph `gid` with origin at (ox, oy), y down."""
    s = px / face.upem
    polys = [[(x * s + ox, -y * s + oy) for x, y in c] for c in face.contours(gid)]
    if not polys:
        return {}
    xs = [p[0] for c in polys for p in c]
    ys = [p[1] for c in polys for p in c]
    out = {}
    for y in range(math.floor(min(ys)), math.ceil(max(ys))):
        for x in range(math.floor(min(xs)), math.ceil(max(xs))):
            # Cheap reject per contour by bbox, then exact clip.
            area = 0.0
            for c in polys:
                area += clip_signed_area(c, x, y)
            v = math.floor(min(abs(area), 1.0) * 255 + 0.5)
            if v:
                out[(x, y)] = v
    return out


def quantize(pen):
    q = math.floor(pen * 4 + 0.5)
    return q // 4, q % 4


def srgb_to_linear(c):
    c /= 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def linear_to_srgb(v):
    s = v * 12.92 if v <= 0.0031308 else 1.055 * v ** (1 / 2.4) - 0.055
    return max(0, min(255, math.floor(s * 255 + 0.5)))


def blend(bg, fg, a):
    if a == 0:
        return bg
    if a == 255:
        return fg
    out = 0
    for shift in (16, 8, 0):
        lb = srgb_to_linear((bg >> shift) & 0xFF)
        lf = srgb_to_linear((fg >> shift) & 0xFF)
        out |= linear_to_srgb(lb + (lf - lb) * a / 255.0) << shift
    return out


def line_glyphs(face, px, text, x0):
    """[(gid, integer origin x, subpixel bin)] for one line."""
    s = px / face.upem
    pen, prev, out = 0.0, None, []
    for ch in text:
        gid = face.gid(ch)
        if prev is not None:
            pen += face.kern(prev, gid) * s
        ix, b = quantize(x0 + pen)
        out.append((gid, ix, b))
        pen += face.advance(gid) * s
        prev = gid
    return out


def header(cmd, args):
    print("// Generated by scripts/fonts/area_oracle.py %s %s" % (cmd, " ".join(repr(a) if " " in a else a for a in args)))
    print("// Exact per-pixel area (fontTools outlines, f64 polygon clipping); no FreeType, no neo-text.")


def cmd_glyph(path, ch, px, x_offset):
    face = Face(path)
    cov = glyph_coverage(face, face.gid(ch), px, x_offset, 0.0)
    xs = [k[0] for k in cov]
    ys = [k[1] for k in cov]
    left, top = min(xs), min(ys)
    w, h = max(xs) - left + 1, max(ys) - top + 1
    header("glyph", sys.argv[2:])
    print("pub const LEFT: i32 = %d;\npub const TOP: i32 = %d;" % (left, top))
    print("pub const WIDTH: usize = %d;\npub const HEIGHT: usize = %d;" % (w, h))
    print("#[rustfmt::skip]\npub const COVERAGE: [u8; %d] = [" % (w * h))
    for y in range(top, top + h):
        print("    " + ", ".join("%3d" % cov.get((x, y), 0) for x in range(left, left + w)) + ",")
    print("];")


def render_window(path, px, text, x0, baseline):
    face = Face(path)
    layers = []
    for gid, ix, b in line_glyphs(face, px, text, x0):
        layers.append(glyph_coverage(face, gid, px, ix + b / 4.0, baseline))
    return layers


def cmd_window(path, px, text, x0, baseline, bg, fg, wx, wy, ww, wh):
    canvas = {}
    for cov in render_window(path, px, text, x0, baseline):
        for key, a in cov.items():
            canvas[key] = blend(canvas.get(key, bg), fg, a)
    header("window", sys.argv[2:])
    print("#[rustfmt::skip]\nconst EXPECTED: [u32; %d] = [" % (ww * wh))
    for y in range(wy, wy + wh):
        print("    " + ", ".join("0x%06x" % canvas.get((x, y), bg) for x in range(wx, wx + ww)) + ",")
    print("];")


def darkened(a, darken_px):
    """neo-text's documented stem-darkening curve (font.rs `darken_stems`)."""
    if darken_px is None or darken_px >= 16.0:
        return a
    k = min((16.0 - darken_px) / 8.0, 1.0) * 0.3
    return int((a / 255.0) ** (1.0 / (1.0 + k)) * 255.0 + 0.5)


def cmd_bounds_window(path, px, text, x0, baseline, bg, fg, wx, wy, ww, wh, slack, darken_px=None):
    """Per-pixel [lo, hi] XRGB bounds: every glyph's coverage moved by
    -slack / +slack (clamped) before blending. Requires fg >= bg per channel so
    the blend is monotone in coverage."""
    assert all(((fg >> s) & 0xFF) >= ((bg >> s) & 0xFF) for s in (16, 8, 0))
    lo, hi, exact = {}, {}, {}
    for cov in render_window(path, px, text, x0, baseline):
        for key, a in cov.items():
            lo[key] = blend(lo.get(key, bg), fg, darkened(max(0, a - slack), darken_px))
            hi[key] = blend(hi.get(key, bg), fg, darkened(min(255, a + slack), darken_px))
            exact[key] = max(exact.get(key, 0), a)
    inside = [(x, y) for y in range(wy, wy + wh) for x in range(wx, wx + ww)]
    if darken_px is not None:
        # Small-size mode (Desktop S3 chrome labels): a pixel whose exact area
        # rounds to 0 but lies under no glyph's coverage map still gets the
        # +slack bound (a rasterizer may legitimately emit coverage 1-2 there,
        # which linear-light blending makes visible). Off by default, so the S2
        # fixtures regenerate byte-identically.
        for key in inside:
            hi.setdefault(key, blend(bg, fg, darkened(min(255, slack), darken_px)))
    ink = sum(1 for k in inside if exact.get(k, 0) >= 128)
    header("bounds-window", sys.argv[2:])
    print("// Coverage slack +/-%d/255 around the exact area; %d of %d pixels are >= 50%% ink." % (slack, ink, len(inside)))
    print("pub const WINDOW: (usize, usize, usize, usize) = (%d, %d, %d, %d);" % (wx, wy, ww, wh))
    print("pub const INK_PIXELS: usize = %d;" % ink)
    for name, table in (("LO", lo), ("HI", hi)):
        print("#[rustfmt::skip]\npub const %s: [u32; %d] = [" % (name, ww * wh))
        for y in range(wy, wy + wh):
            print("    " + ", ".join("0x%06x" % table.get((x, y), bg) for x in range(wx, wx + ww)) + ",")
        print("];")


def main(argv):
    cmd, a = argv[1], argv[2:]
    if cmd == "glyph":
        cmd_glyph(a[0], a[1], float(a[2]), float(a[3]))
    elif cmd == "window":
        cmd_window(a[0], float(a[1]), a[2], int(a[3]), int(a[4]), int(a[5], 16), int(a[6], 16),
                   int(a[7]), int(a[8]), int(a[9]), int(a[10]))
    elif cmd == "bounds-window":
        cmd_bounds_window(a[0], float(a[1]), a[2], int(a[3]), int(a[4]), int(a[5], 16), int(a[6], 16),
                          int(a[7]), int(a[8]), int(a[9]), int(a[10]), int(a[11]),
                          float(a[12]) if len(a) > 12 else None)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
