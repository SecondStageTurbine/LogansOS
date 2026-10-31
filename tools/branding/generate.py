# SPDX-License-Identifier: MPL-2.0
"""Generate the LogansOS branding asset ladder + kernel boot splash.

Pure Python standard library only (zlib + struct) so it is reproducible in any
environment with python3 — no Pillow/ImageMagick install required. This is the
authoritative, recorded generation method referenced by
``assets/branding/README.md``.

The LogansOS identity is a blue lens-flare star inside broken silver ring arcs
with a wordmark below — it is NOT a circular badge, so there is no circular
alpha mask anywhere in this pipeline (the previous Plutarch crest was masked;
that is retired).

Committed *sources* (hand-authored artwork, never regenerated here):
    assets/branding/LogansOS.png              black-bg master, 1254x1254 RGBA
    assets/branding/logansos-transparent.png  transparent master, 1254x1254 RGBA

Generated *outputs* (do not hand-edit — re-run this script):
    assets/branding/logansos-logo-512.png      512x512 RGBA, from the transparent
    assets/branding/logansos-logo-256.png      256x256 RGBA, from the transparent
    assets/branding/logansos-icon-128.png      128x128 RGBA, from the transparent
    kernel/assets/logansos_boot_logo_160.rgba  160x160 RGBA8888 raw, opaque black
                                               bg, the emblem cropped out of the
                                               black-bg master (== 160*160*4 =
                                               102400 B). The wordmark is cropped
                                               away: at 160px it is illegibly
                                               small, so the boot splash shows the
                                               ring+star emblem alone.
    userland/neo-atrium/assets/busy-emblem-star-88.rgba
    userland/neo-atrium/assets/busy-emblem-ring-88.rgba
                                               88x88 RGBA8888 raw, straight alpha,
                                               from the transparent master: the
                                               still star and the ring arcs, split
                                               apart, on a square centred on the
                                               ring. neo-atrium's build script
                                               rotates the ring over the star into
                                               the busy emblem's frames.

The boot splash keeps an opaque black background on purpose: the kernel
framebuffer is cleared to black before the blit and ``fb::blit_rgba`` writes
opaque pixels as-is, so black-on-black is seamless — no alpha needed there. The
black-bg master is composited over black so any source transparency collapses to
the intended opaque emblem-on-black.
"""

import os
import struct
import sys
import zlib

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BDIR = os.path.join(ROOT, "assets", "branding")
TRANSPARENT_SRC = os.path.join(BDIR, "logansos-transparent.png")
BLACKBG_SRC = os.path.join(BDIR, "LogansOS.png")

# Emblem crop window into the 1254x1254 black-bg master, for the 160^2 boot blob.
# Derived from the artwork: the ring+star emblem occupies x[342..964] y[229..784]
# and the "LogansOS" wordmark begins at y~823. A 630px square centered on the
# emblem at (653, 506) contains the full ring (right=968>=964, bottom=821<823)
# while excluding the wordmark band.
CROP_LEFT = 338
CROP_TOP = 191
CROP_SIDE = 630

# Busy emblem (A9-D). The ring arcs are a circle centred at (639.1, 495.4) in
# the master (least-squares fit to the silver strokes; radius 276) and no
# emblem pixel lies more than ~330 px from it. The square below is centred on
# that point, so the ring turns in place. Rows from BUSY_WORDMARK_TOP down
# belong to the wordmark and are cleared.
BUSY_LEFT = 303
BUSY_TOP = 159
BUSY_SIDE = 672
BUSY_WORDMARK_TOP = 815
BUSY_LAYER = 88
# The star: a disc round the centre plus the thin horizontal flare, which
# reaches further left than right (the lower-right ring crescent comes within
# 0.33 of the side on the right).
BUSY_STAR_DISC = 0.25
BUSY_FLARE_HALF_HEIGHT = 0.04
BUSY_FLARE_LEFT = 0.37
BUSY_FLARE_RIGHT = 0.32


def busy_is_star(side, x, y):
    """Whether square pixel (x, y) belongs to the still star."""
    dx = (x + 0.5) / side - 0.5
    dy = (y + 0.5) / side - 0.5
    r = (dx * dx + dy * dy) ** 0.5
    if r < BUSY_STAR_DISC:
        return True
    reach = BUSY_FLARE_LEFT if dx < 0 else BUSY_FLARE_RIGHT
    return abs(dy) < BUSY_FLARE_HALF_HEIGHT and r < reach


def busy_layers(w, h, rgba):
    """The star and ring layers, each BUSY_LAYER^2 straight-alpha RGBA."""
    square = crop_rgba(w, h, rgba, BUSY_LEFT, BUSY_TOP, BUSY_SIDE)
    star = bytearray(square)
    ring = bytearray(square)
    for y in range(BUSY_SIDE):
        for x in range(BUSY_SIDE):
            alpha = (y * BUSY_SIDE + x) * 4 + 3
            if BUSY_TOP + y >= BUSY_WORDMARK_TOP:
                star[alpha] = 0
                ring[alpha] = 0
            elif busy_is_star(BUSY_SIDE, x, y):
                ring[alpha] = 0
            else:
                star[alpha] = 0
    return tuple(
        box_downscale_rgba(BUSY_SIDE, BUSY_SIDE, layer, BUSY_LAYER, BUSY_LAYER)
        for layer in (star, ring)
    )


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def decode_png(path):
    """Decode an 8-bit, non-interlaced PNG (colortype 2 RGB or 6 RGBA).

    Returns (width, height, channels, bytearray of raw samples).
    """
    raw = open(path, "rb").read()
    assert raw[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
    pos = 8
    width = height = bitdepth = colortype = None
    idat = bytearray()
    while pos < len(raw):
        (length,) = struct.unpack(">I", raw[pos : pos + 4])
        ctype = raw[pos + 4 : pos + 8]
        data = raw[pos + 8 : pos + 8 + length]
        pos += 12 + length  # length + type + data + crc
        if ctype == b"IHDR":
            width, height, bitdepth, colortype, comp, filt, interlace = struct.unpack(
                ">IIBBBBB", data
            )
            assert bitdepth == 8, f"only 8-bit supported, got {bitdepth}"
            assert colortype in (2, 6), f"only RGB/RGBA supported, got {colortype}"
            assert interlace == 0, "interlaced PNG not supported"
        elif ctype == b"IDAT":
            idat += data
        elif ctype == b"IEND":
            break
    channels = 3 if colortype == 2 else 4
    decompressed = zlib.decompress(bytes(idat))
    stride = width * channels
    out = bytearray(width * height * channels)
    prev = bytearray(stride)
    src = 0
    for y in range(height):
        ftype = decompressed[src]
        src += 1
        line = bytearray(decompressed[src : src + stride])
        src += stride
        if ftype == 1:  # Sub
            for i in range(channels, stride):
                line[i] = (line[i] + line[i - channels]) & 0xFF
        elif ftype == 2:  # Up
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif ftype == 3:  # Average
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif ftype == 4:  # Paeth
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                c = prev[i - channels] if i >= channels else 0
                line[i] = (line[i] + _paeth(a, prev[i], c)) & 0xFF
        elif ftype != 0:
            raise ValueError(f"bad filter type {ftype}")
        out[y * stride : (y + 1) * stride] = line
        prev = line
    return width, height, channels, out


def to_rgba(w, h, ch, samples):
    """Promote RGB->RGBA (alpha 255). RGBA passes through unchanged."""
    if ch == 4:
        return bytearray(samples)
    out = bytearray(w * h * 4)
    for i in range(w * h):
        out[i * 4] = samples[i * 3]
        out[i * 4 + 1] = samples[i * 3 + 1]
        out[i * 4 + 2] = samples[i * 3 + 2]
        out[i * 4 + 3] = 255
    return out


def box_downscale_rgba(w, h, rgba, tw, th):
    """Alpha-weighted box-average downscale RGBA -> RGBA at (tw, th).

    RGB is averaged weighted by source alpha so semi-transparent edge pixels do
    not drag toward black (straight-alpha box averaging fringes; weighting by
    coverage fixes it). Alpha is a plain area average.
    """
    out = bytearray(tw * th * 4)
    for ty in range(th):
        y0 = ty * h // th
        y1 = max(y0 + 1, (ty + 1) * h // th)
        for tx in range(tw):
            x0 = tx * w // tw
            x1 = max(x0 + 1, (tx + 1) * w // tw)
            sr = sg = sb = aw = 0  # alpha-weighted color sums + weight
            sa = n = 0             # plain alpha sum + sample count
            for sy in range(y0, y1):
                base = (sy * w + x0) * 4
                for _ in range(x0, x1):
                    a = rgba[base + 3]
                    sr += rgba[base] * a
                    sg += rgba[base + 1] * a
                    sb += rgba[base + 2] * a
                    aw += a
                    sa += a
                    n += 1
                    base += 4
            o = (ty * tw + tx) * 4
            if aw > 0:
                out[o] = sr // aw
                out[o + 1] = sg // aw
                out[o + 2] = sb // aw
            out[o + 3] = sa // n
    return out


def crop_rgba(w, h, rgba, left, top, side):
    """Extract a side x side RGBA window at (left, top)."""
    assert left >= 0 and top >= 0 and left + side <= w and top + side <= h, "crop OOB"
    out = bytearray(side * side * 4)
    for y in range(side):
        srow = ((top + y) * w + left) * 4
        out[y * side * 4 : (y + 1) * side * 4] = rgba[srow : srow + side * 4]
    return out


def composite_over_black(n, rgba):
    """Flatten RGBA onto opaque black -> opaque RGBA8888 (alpha forced 255)."""
    out = bytearray(len(rgba))
    for i in range(n):
        a = rgba[i * 4 + 3]
        out[i * 4] = rgba[i * 4] * a // 255
        out[i * 4 + 1] = rgba[i * 4 + 1] * a // 255
        out[i * 4 + 2] = rgba[i * 4 + 2] * a // 255
        out[i * 4 + 3] = 255
    return out


def encode_png(width, height, channels, samples):
    """Encode raw 8-bit samples (channels 3=RGB or 4=RGBA) to PNG bytes."""
    colortype = 2 if channels == 3 else 6
    stride = width * channels
    rawb = bytearray()
    for y in range(height):
        rawb.append(0)  # filter None
        rawb += samples[y * stride : (y + 1) * stride]
    comp = zlib.compress(bytes(rawb), 9)

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, colortype, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", comp) + chunk(
        b"IEND", b""
    )


def write_rgba_raw(path, w, h, rgba):
    with open(path, "wb") as f:
        f.write(bytes(rgba))
    size = os.path.getsize(path)
    expected = w * h * 4
    assert size == expected, f"{path}: {size} bytes, expected {expected}"
    return size


def main():
    # --- PNG ladder: alpha-weighted downscale of the transparent master ---
    tw_, th_, tch, tsamp = decode_png(TRANSPARENT_SRC)
    print(f"transparent source: {tw_}x{th_} channels={tch}")
    trgba = to_rgba(tw_, th_, tch, tsamp)
    for name, size in (
        ("logansos-logo-512.png", 512),
        ("logansos-logo-256.png", 256),
        ("logansos-icon-128.png", 128),
    ):
        scaled = box_downscale_rgba(tw_, th_, trgba, size, size)
        png = encode_png(size, size, 4, scaled)
        open(os.path.join(BDIR, name), "wb").write(png)
        print(f"wrote {name}: {size}x{size} RGBA, {len(png)} bytes")

    # --- Kernel boot splash: emblem cropped from the black-bg master, 160^2 ---
    bw, bh, bch, bsamp = decode_png(BLACKBG_SRC)
    print(f"black-bg source: {bw}x{bh} channels={bch}")
    brgba = to_rgba(bw, bh, bch, bsamp)
    emblem = crop_rgba(bw, bh, brgba, CROP_LEFT, CROP_TOP, CROP_SIDE)
    boot = box_downscale_rgba(CROP_SIDE, CROP_SIDE, emblem, 160, 160)
    boot = composite_over_black(160 * 160, boot)
    out = os.path.join(ROOT, "kernel", "assets", "logansos_boot_logo_160.rgba")
    size = write_rgba_raw(out, 160, 160, boot)
    print(f"wrote kernel/assets/logansos_boot_logo_160.rgba: {size} bytes (oracle 102400)")

    # --- Busy emblem layers: star and ring, from the transparent master ---
    adir = os.path.join(ROOT, "userland", "neo-atrium", "assets")
    os.makedirs(adir, exist_ok=True)
    for name, layer in zip(("star", "ring"), busy_layers(tw_, th_, trgba)):
        rel = f"busy-emblem-{name}-{BUSY_LAYER}.rgba"
        size = write_rgba_raw(os.path.join(adir, rel), BUSY_LAYER, BUSY_LAYER, layer)
        print(f"wrote userland/neo-atrium/assets/{rel}: {size} bytes")


if __name__ == "__main__":
    sys.exit(main())
