#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Independent reference renderer for the neo-text byte oracles.

Nothing here shares code with neo-text: glyph outlines are scaled and
rasterized by the system FreeType library (libfreetype.so.6, via ctypes,
FT_LOAD_NO_HINTING + FT_RENDER_MODE_NORMAL), advances come from FreeType's
unhinted `linearHoriAdvance`, kerning from fontTools' own GPOS parser, and
gamma-correct blending from the IEC 61966-2-1 formula in float.

The only things deliberately shared with neo-text are the *specified*
conventions: ppem = pixel size, glyph origins rounded to the nearest 1/4 px,
GPOS `kern` lookups applied first matching subtable wins (Format 1 matches only listed pairs), linear-light
blending. Those are the spec the oracle checks the implementation against.

Subcommands (each prints a Rust source fragment on stdout):
  glyph <font.ttf> <char> <px>          coverage of one glyph at origin (0,0)
  window <font.ttf> <px> <text> <x> <baseline> <bg> <fg> <wx> <wy> <ww> <wh>
                                        XRGB pixels of a text line drawn on bg

Requires fontTools on sys.path (only for GPOS kerning).
"""
import ctypes
import ctypes.util
import math
import sys

FT_LOAD_NO_HINTING = 1 << 1
FT_LOAD_NO_BITMAP = 1 << 3
FT_RENDER_MODE_NORMAL = 0


class FT_Generic(ctypes.Structure):
    _fields_ = [("data", ctypes.c_void_p), ("finalizer", ctypes.c_void_p)]


class FT_BBox(ctypes.Structure):
    _fields_ = [(n, ctypes.c_long) for n in ("xMin", "yMin", "xMax", "yMax")]


class FT_Vector(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]


class FT_Glyph_Metrics(ctypes.Structure):
    _fields_ = [(n, ctypes.c_long) for n in (
        "width", "height", "horiBearingX", "horiBearingY", "horiAdvance",
        "vertBearingX", "vertBearingY", "vertAdvance")]


class FT_Bitmap(ctypes.Structure):
    _fields_ = [
        ("rows", ctypes.c_uint), ("width", ctypes.c_uint), ("pitch", ctypes.c_int),
        ("buffer", ctypes.POINTER(ctypes.c_ubyte)), ("num_grays", ctypes.c_ushort),
        ("pixel_mode", ctypes.c_ubyte), ("palette_mode", ctypes.c_ubyte),
        ("palette", ctypes.c_void_p)]


class FT_GlyphSlotRec(ctypes.Structure):
    _fields_ = [
        ("library", ctypes.c_void_p), ("face", ctypes.c_void_p), ("next", ctypes.c_void_p),
        ("glyph_index", ctypes.c_uint), ("generic", FT_Generic),
        ("metrics", FT_Glyph_Metrics), ("linearHoriAdvance", ctypes.c_long),
        ("linearVertAdvance", ctypes.c_long), ("advance", FT_Vector),
        ("format", ctypes.c_uint), ("bitmap", FT_Bitmap),
        ("bitmap_left", ctypes.c_int), ("bitmap_top", ctypes.c_int)]


class FT_FaceRec(ctypes.Structure):
    _fields_ = [
        ("num_faces", ctypes.c_long), ("face_index", ctypes.c_long),
        ("face_flags", ctypes.c_long), ("style_flags", ctypes.c_long),
        ("num_glyphs", ctypes.c_long), ("family_name", ctypes.c_char_p),
        ("style_name", ctypes.c_char_p), ("num_fixed_sizes", ctypes.c_int),
        ("available_sizes", ctypes.c_void_p), ("num_charmaps", ctypes.c_int),
        ("charmaps", ctypes.c_void_p), ("generic", FT_Generic), ("bbox", FT_BBox),
        ("units_per_EM", ctypes.c_ushort), ("ascender", ctypes.c_short),
        ("descender", ctypes.c_short), ("height", ctypes.c_short),
        ("max_advance_width", ctypes.c_short), ("max_advance_height", ctypes.c_short),
        ("underline_position", ctypes.c_short), ("underline_thickness", ctypes.c_short),
        ("glyph", ctypes.POINTER(FT_GlyphSlotRec))]


ft = ctypes.CDLL(ctypes.util.find_library("freetype") or "libfreetype.so.6")


def check(err, what):
    if err:
        raise SystemExit("FreeType %s failed: %d" % (what, err))


class Face:
    def __init__(self, path, px):
        self.lib = ctypes.c_void_p()
        check(ft.FT_Init_FreeType(ctypes.byref(self.lib)), "init")
        self.face = ctypes.POINTER(FT_FaceRec)()
        check(ft.FT_New_Face(self.lib, path.encode(), 0, ctypes.byref(self.face)), "new_face")
        check(ft.FT_Set_Pixel_Sizes(self.face, 0, px), "set_pixel_sizes")
        self.path = path
        self.px = px

    def version(self):
        a, b, c = ctypes.c_int(), ctypes.c_int(), ctypes.c_int()
        ft.FT_Library_Version(self.lib, ctypes.byref(a), ctypes.byref(b), ctypes.byref(c))
        return "%d.%d.%d" % (a.value, b.value, c.value)

    def glyph_index(self, ch):
        return ft.FT_Get_Char_Index(self.face, ctypes.c_ulong(ord(ch)))

    def render(self, gid, x_shift):
        """Coverage of glyph `gid` with its origin moved right by x_shift px.
        Returns (left, top_down, width, rows, bytes, linear_advance_px)."""
        delta = FT_Vector(int(round(x_shift * 64)), 0)
        ft.FT_Set_Transform(self.face, None, ctypes.byref(delta))
        check(ft.FT_Load_Glyph(self.face, gid, FT_LOAD_NO_HINTING | FT_LOAD_NO_BITMAP), "load")
        slot = self.face.contents.glyph
        check(ft.FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL), "render")
        g = slot.contents
        bm = g.bitmap
        data = []
        for r in range(bm.rows):
            for c in range(bm.width):
                data.append(bm.buffer[r * bm.pitch + c])
        return (g.bitmap_left, -g.bitmap_top, bm.width, bm.rows, data,
                g.linearHoriAdvance / 65536.0)


def gpos_kern(font_path):
    sys_path_fonttools()
    from fontTools.ttLib import TTFont
    tt = TTFont(font_path)
    gpos = tt["GPOS"].table
    order = tt.getGlyphOrder()
    lookups = sorted({i for fr in gpos.FeatureList.FeatureRecord
                      if fr.FeatureTag == "kern" for i in fr.Feature.LookupListIndex})

    def kern(left_gid, right_gid):
        left, right = order[left_gid], order[right_gid]
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

    return kern, tt["head"].unitsPerEm


def sys_path_fonttools():
    try:
        import fontTools  # noqa: F401
    except ImportError:
        raise SystemExit("fontTools is required (pip install fonttools)")


def quantize(pen):
    q = math.floor(pen * 4 + 0.5)
    return q // 4, q % 4


def srgb_to_linear(c):
    c /= 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def linear_to_srgb(v):
    s = v * 12.92 if v <= 0.0031308 else 1.055 * v ** (1 / 2.4) - 0.055
    return max(0, min(255, int(math.floor(s * 255 + 0.5))))


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


def cmd_glyph(path, ch, px):
    face = Face(path, px)
    left, top, w, h, data, _ = face.render(face.glyph_index(ch), 0.0)
    print("// Generated by scripts/fonts/freetype_oracle.py glyph %s %r %d" % (path.split("/")[-1], ch, px))
    print("// FreeType %s, FT_LOAD_NO_HINTING, FT_RENDER_MODE_NORMAL." % face.version())
    print("pub const LEFT: i32 = %d;\npub const TOP: i32 = %d;" % (left, top))
    print("pub const WIDTH: usize = %d;\npub const HEIGHT: usize = %d;" % (w, h))
    print("#[rustfmt::skip]\npub const COVERAGE: [u8; %d] = [" % len(data))
    for r in range(h):
        print("    " + ", ".join("%3d" % v for v in data[r * w:(r + 1) * w]) + ",")
    print("];")


def cmd_window(path, px, text, x0, baseline, bg, fg, wx, wy, ww, wh):
    face = Face(path, px)
    kern, upem = gpos_kern(path)
    canvas = {}
    pen, prev = 0.0, None
    for ch in text:
        gid = face.glyph_index(ch)
        if prev is not None:
            pen += kern(prev, gid) * px / upem
        ix, b = quantize(x0 + pen)
        left, top, w, h, data, adv = face.render(gid, b / 4.0)
        for r in range(h):
            for c in range(w):
                a = data[r * w + c]
                if a:
                    key = (ix + left + c, baseline + top + r)
                    canvas[key] = blend(canvas.get(key, bg), fg, a)
        pen += adv
        prev = gid
    print("// Generated by scripts/fonts/freetype_oracle.py window %s %d %r %d %d %#08x %#08x %d %d %d %d"
          % (path.split("/")[-1], px, text, x0, baseline, bg, fg, wx, wy, ww, wh))
    print("// FreeType %s (unhinted) + fontTools GPOS kern + float sRGB blend." % face.version())
    print("#[rustfmt::skip]\nconst EXPECTED: [u32; %d] = [" % (ww * wh))
    for y in range(wy, wy + wh):
        print("    " + ", ".join("0x%06x" % canvas.get((x, y), bg) for x in range(wx, wx + ww)) + ",")
    print("];")


def main(argv):
    if argv[1] == "glyph":
        cmd_glyph(argv[2], argv[3], int(argv[4]))
    elif argv[1] == "window":
        a = argv[2:]
        cmd_window(a[0], int(a[1]), a[2], int(a[3]), int(a[4]), int(a[5], 16), int(a[6], 16),
                   int(a[7]), int(a[8]), int(a[9]), int(a[10]))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
