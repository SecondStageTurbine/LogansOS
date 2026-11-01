#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Independent PNG -> RGBA8 reference used to derive the Lane 2b KAT hashes.

This is deliberately not the vendored `png` crate: it follows the PNG spec's
chunk layout and per-row filters directly, using only Python's `zlib`. It is a
provenance tool, not part of the build; run it on the fixtures and compare its
hashes with `tests/data/PROVENANCE.md` and the KAT assertions.

Only the features those fixtures use are implemented: non-interlaced 8- and
16-bit grayscale, RGB, palette, gray+alpha and RGBA. For 16-bit samples the
high byte is kept, matching `png::Transformations::STRIP_16`.
"""

import hashlib
import struct
import sys
import zlib
from pathlib import Path


def decode(path):
    data = Path(path).read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    pos = 8
    idat = b""
    palette = None
    transparency = None
    info = None
    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, color, compression, filter_method, interlace = (
                struct.unpack(">IIBBBBB", body)
            )
            assert compression == 0 and filter_method == 0 and interlace == 0, path
            info = (width, height, depth, color)
        elif kind == b"IDAT":
            idat += body
        elif kind == b"PLTE":
            palette = [tuple(body[i : i + 3]) for i in range(0, len(body), 3)]
        elif kind == b"tRNS":
            transparency = body
        elif kind == b"IEND":
            break
    width, height, depth, color = info
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color]
    raw = zlib.decompress(idat)
    bpp = max(1, (channels * depth + 7) // 8)
    stride = (width * channels * depth + 7) // 8
    out = bytearray()
    previous = bytearray(stride)
    offset = 0
    for _y in range(height):
        filter_type = raw[offset]
        offset += 1
        row = bytearray(raw[offset : offset + stride])
        offset += stride
        for i in range(stride):
            a = row[i - bpp] if i >= bpp else 0
            b = previous[i]
            c = previous[i - bpp] if i >= bpp else 0
            if filter_type == 0:
                pass
            elif filter_type == 1:
                row[i] = (row[i] + a) & 0xFF
            elif filter_type == 2:
                row[i] = (row[i] + b) & 0xFF
            elif filter_type == 3:
                row[i] = (row[i] + ((a + b) >> 1)) & 0xFF
            elif filter_type == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                predictor = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                row[i] = (row[i] + predictor) & 0xFF
            else:
                raise ValueError(f"filter {filter_type}")
        previous = row
        samples = (
            [row[i] for i in range(0, len(row), 2)]
            if depth == 16
            else list(row)
        )
        if color == 0:
            for g in samples:
                out += bytes([g, g, g, 255])
        elif color == 4:
            for i in range(0, len(samples), 2):
                g, a = samples[i], samples[i + 1]
                out += bytes([g, g, g, a])
        elif color == 2:
            for i in range(0, len(samples), 3):
                r, g, b = samples[i : i + 3]
                out += bytes([r, g, b, 255])
        elif color == 6:
            for i in range(0, len(samples), 4):
                out += bytes(samples[i : i + 4])
        elif color == 3:
            for index in samples:
                r, g, b = palette[index]
                a = (
                    transparency[index]
                    if transparency is not None and index < len(transparency)
                    else 255
                )
                out += bytes([r, g, b, a])
        else:
            raise ValueError(f"colour type {color}")
    return width, height, bytes(out)


def main(paths):
    for path in paths:
        width, height, rgba = decode(path)
        print(
            Path(path).name,
            f"{width}x{height}",
            len(rgba),
            hashlib.sha256(rgba).hexdigest(),
        )


if __name__ == "__main__":
    main(sys.argv[1:])
