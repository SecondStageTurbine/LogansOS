#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""OT-61 L1's independent font oracle: read glyph ids, advances and face
metrics straight from a TrueType file's tables (no font library), so the
smoke's expected values don't come from skrifa, the code under test.

Usage: font_tables.py FONT.ttf TEXT
Prints upem, hhea ascender/descender/lineGap/advanceWidthMax, post underline,
OS/2 sxHeight/xAvgCharWidth/strikeout, then each character's glyph id (cmap
format 4) and advance width (hmtx), in font units.
"""
import struct
import sys


def tables(data):
    num = struct.unpack_from(">H", data, 4)[0]
    out = {}
    for i in range(num):
        tag, _cs, off, length = struct.unpack_from(">4sIII", data, 12 + 16 * i)
        out[tag.decode("latin-1")] = (off, length)
    return out


def cmap4(data, t):
    off, _ = t["cmap"]
    _ver, n = struct.unpack_from(">HH", data, off)
    for i in range(n):
        pid, eid, sub = struct.unpack_from(">HHI", data, off + 4 + 8 * i)
        if (pid, eid) in ((3, 1), (0, 3)):
            base = off + sub
            if struct.unpack_from(">H", data, base)[0] == 4:
                return base
    raise SystemExit("no cmap format 4 (3,1) subtable")


def glyph_id(data, base, ch):
    seg2 = struct.unpack_from(">H", data, base + 6)[0]
    ends = base + 14
    starts = ends + seg2 + 2
    deltas = starts + seg2
    ranges = deltas + seg2
    for i in range(seg2 // 2):
        end = struct.unpack_from(">H", data, ends + 2 * i)[0]
        if ch > end:
            continue
        start = struct.unpack_from(">H", data, starts + 2 * i)[0]
        if ch < start:
            return 0
        delta = struct.unpack_from(">h", data, deltas + 2 * i)[0]
        ro = struct.unpack_from(">H", data, ranges + 2 * i)[0]
        if ro == 0:
            return (ch + delta) & 0xFFFF
        g = struct.unpack_from(">H", data, ranges + 2 * i + ro + 2 * (ch - start))[0]
        return 0 if g == 0 else (g + delta) & 0xFFFF
    return 0


def main():
    path, text = sys.argv[1], sys.argv[2]
    data = open(path, "rb").read()
    t = tables(data)
    upem = struct.unpack_from(">H", data, t["head"][0] + 18)[0]
    h = t["hhea"][0]
    asc, desc, gap = struct.unpack_from(">hhh", data, h + 4)
    adv_max = struct.unpack_from(">H", data, h + 10)[0]
    n_hmetrics = struct.unpack_from(">H", data, h + 34)[0]
    p = t["post"][0]
    ul_pos, ul_thick = struct.unpack_from(">hh", data, p + 8)
    o = t["OS/2"][0]
    os2_ver = struct.unpack_from(">H", data, o)[0]
    x_avg = struct.unpack_from(">h", data, o + 2)[0]
    st_size, st_pos = struct.unpack_from(">hh", data, o + 26)
    sx_height = struct.unpack_from(">h", data, o + 86)[0] if os2_ver >= 2 else 0
    print(f"upem={upem} ascender={asc} descender={desc} lineGap={gap} advanceWidthMax={adv_max}")
    print(f"underlinePosition={ul_pos} underlineThickness={ul_thick}")
    print(f"xAvgCharWidth={x_avg} yStrikeoutSize={st_size} yStrikeoutPosition={st_pos} sxHeight={sx_height}")
    base = cmap4(data, t)
    hmtx = t["hmtx"][0]
    for ch in text:
        g = glyph_id(data, base, ord(ch))
        aw = struct.unpack_from(">H", data, hmtx + 4 * min(g, n_hmetrics - 1))[0]
        print(f"U+{ord(ch):04X} {ch!r} glyph={g} advance={aw}")


main()
