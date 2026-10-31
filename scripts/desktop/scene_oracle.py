#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Independent reference renderer for the desktop scene (neo-atrium).

Implements the "Scene specification" (v1.8) text in userland/neo-atrium/src/lib.rs
from scratch, sharing no code with the Rust:

* numpy whole-image integer arithmetic instead of per-pixel functions;
* the box blur via padded cumulative sums (the Rust uses a sliding window);
* the corner coverage by its own closed-form area integral in Python floats;
* the WM's focus light composited with the S1 `blend_over` formula.

It renders the final base layer (and optionally the focus light) and prints a
Rust fixture with the FNV-1a digest of every visible pixel, row-major, each as
4 little-endian bytes, with the three label boxes hashed as zero (labels
are verified separately against scripts/fonts/area_oracle.py).

usage: scene_oracle.py fixture > fixtures/desktop-s3/scene_digests.rs
       scene_oracle.py home-fixture > fixtures/desktop-ds1/home_digests.rs
       scene_oracle.py lens-fixture > fixtures/desktop-ds1/lens_digests.rs
       scene_oracle.py move-fixture | resize-fixture   (fixtures/desktop-di1b/)
       scene_oracle.py dock-fixture > fixtures/desktop-dl1/dock_digests.rs
       scene_oracle.py motion-fixture > fixtures/desktop-dm1/motion_digests.rs
       scene_oracle.py png <w> <h> <glass|rt> <focused 0|1> <out.png>
"""
import math
import sys

import numpy as np

# --- spec constants (lib.rs / layout.rs / material.rs / scene.rs prose) ------
VOID = (10, 14, 39)
THRESHOLD = (18, 28, 66)
EB = (60, 142, 244)
INACTIVE = (36, 74, 122)
RT_PANEL = (26, 33, 48)
RT_RAISED = (34, 42, 60)
CONTENT = (10, 14, 39)
BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
WHITE_W = 255 * 256
BAR_H, SPINE_W, PAD, TITLE_H = 36, 84, 8, 34
WIN_R, CHIP_INSET, CHIP, CHIP_R = 12, 14, 56, 10
FIELD_R = 14  # DS-1 Home intent field radius
GLOW, GLOW_PEAK = 14, 110.0
BLUR_R, MARGIN = 8, 24
FROST, VIB, CHIP_FROST = 18, 26, 30
HAIR = 36
TICK_LEN, TICK_W = 10, 2
RING_HW = 24
# DS-1c Lens (spec v1.4 "Lens"); DL-1 (v1.6): the client is a fixed 360x400
LENS_CW, LENS_CH, LENS_GAP, HALO_A = 360, 400, 32, 110
MIN_MARGIN = 16
# DI-1b client bounds (layout.rs)
MIN_CW, MIN_CH, MAX_CW, MAX_CH = 320, 200, 1280, 800


# DI-1b move-fixture origins at 1920x1080: an interior move, and the clamp
# corner (top-left of the allowed range: workfield origin + GLOW).
MOVE_ORIGINS = ((500, 200), (98, 50))
# DI-1b resize-fixture cases at 1920x1080, (frame x, frame y, client w, client
# h): the centred frame's bottom-right corner dragged by (+120, +60); the
# largest client (1280x800) at the clamp corner; the smallest (320x200).
RESIZE_CASES = ((674, 337, 760, 460), (98, 50, 1280, 800), (674, 337, 320, 200))


def layout(W, H, cw=640, ch=400, frame_at=None):
    """frame_at: DI-1b, the window frame's top-left (already inside the
    workfield; the WM clamps). Default: centred in the workfield."""
    fw, fh = cw + 2 * PAD, TITLE_H + ch + PAD
    wx, wy, ww, wh = SPINE_W, BAR_H, W - SPINE_W, H - BAR_H
    fx, fy = frame_at if frame_at else (wx + (ww - fw) // 2, wy + (wh - fh) // 2)
    chip = (CHIP_INSET, BAR_H + CHIP_INSET, CHIP, CHIP)
    # DS-1 Home: the intent field
    field_w = min(720, ww - 2 * (GLOW + 16))
    field = (wx + (ww - field_w) // 2, wy + (wh - 56) * 38 // 100, field_w, 56)
    return {
        "bar": (0, 0, W, BAR_H),
        "spine": (0, BAR_H, SPINE_W, H - BAR_H),
        "chip": chip,
        "marker": (4, chip[1] + 12, 3, 32),
        "status": (16, 14, 8, 8),
        "frame": (fx, fy, fw, fh),
        "client": (fx + PAD, fy + TITLE_H, cw, ch),
        "labels": [
            (30, 6, 170, 24),
            (fx + 14, fy + 4, 240, 26),
            (chip[0] + 3, chip[1] + 8, CHIP - 6, CHIP - 10),
        ],
        "field": field,
        "home_labels": [
            (30, 6, 170, 24),
            (field[0] + 22, field[1] + 8, field[2] - 44, field[3] - 16),
            (chip[0] + 3, chip[1] + 8, CHIP - 6, CHIP - 10),
        ],
        # DS-1c: the second chip and (room permitting) the Lens
        "chip2": (14, 128, 56, 56),
        "chip2_label": (17, 136, 50, 46),
        # OT-87 R4.4b: the third chip (Install), below the Lens's at its gap
        "chip3": (14, 206, 56, 56),
        "chip3_label": (17, 214, 50, 46),
        "lens": lens_layout(W, H, (fx, fy, fw, fh)),
    }


def card_rect(L, rows):
    """v1.8 (desktop-intro): a card of `rows` rows under the intent field."""
    fx, fy, fw, fh = L["field"]
    return (fx, fy + fh + GLOW + 18, fw, 24 * rows + 20)


def card_text(L, rows):
    """The card's text box (masked like a label box); rows=1: the hint box."""
    x, y, w, h = card_rect(L, rows)
    return (x + 22, y + 10, w - 44, 24 * rows)


def lens_layout(W, H, frame):
    """The Lens beside the frame, top-aligned, its frame around a fixed
    LENS_CW x LENS_CH client (v1.6: no longer the window's height)."""
    fx, fy, fw, fh = frame
    lx = fx + fw + LENS_GAP
    lf = (lx, fy, LENS_CW + 2 * PAD, TITLE_H + LENS_CH + PAD)
    if lf[0] + lf[2] + GLOW + MIN_MARGIN > W or lf[1] + lf[3] + GLOW > H:
        return None
    return {
        "frame": lf,
        "client": (lx + PAD, fy + TITLE_H, LENS_CW, LENS_CH),
        "ring": (lx - 4, fy - 4, lf[2] + 8, lf[3] + 8),
        "title": (lx + 14, fy + 4, 240, 26),
    }


# --- DL-1: the docked group, written from the spec (v1.6 "Docked Lens") ------
# The group is the frame, LENS_GAP, the Lens frame and the Lens's right margin
# (MIN_MARGIN); it is as tall as the taller of the frame and the Lens frame.
# A move puts the group's top-left nearest the request inside the workfield
# inset by GLOW; a resize stops the right edge where the group still fits and
# the top where the Lens's bottom does.

def group_wh(fw, fh):
    lfw, lfh = LENS_CW + 2 * PAD, TITLE_H + LENS_CH + PAD
    return fw + LENS_GAP + lfw + MIN_MARGIN, max(fh, lfh)


def dock_move(W, H, fw, fh, x, y):
    gw, gh = group_wh(fw, fh)
    lo_x, hi_x = SPINE_W + GLOW, W - GLOW - gw
    lo_y, hi_y = BAR_H + GLOW, H - GLOW - gh
    return max(lo_x, min(hi_x, x)), max(lo_y, min(hi_y, y))


def dock_resize(W, H, frame, client, edges, dx, dy):
    """(x, y, cw, ch) after dragging `edges` ('L', 'R', 'T', 'B' letters) of
    a docked frame by (dx, dy)."""
    fx, fy = frame
    cw, ch = client
    left, right, top, bottom = fx, fx + cw + 2 * PAD, fy, fy + TITLE_H + ch + PAD
    max_cw = min(MAX_CW, W - SPINE_W - 2 * (GLOW + MIN_MARGIN) - 2 * PAD)
    max_ch = min(MAX_CH, H - BAR_H - 2 * (GLOW + MIN_MARGIN) - TITLE_H - PAD)
    lfw, lfh = LENS_CW + 2 * PAD, TITLE_H + LENS_CH + PAD
    if "L" in edges:
        left = max(right - (max_cw + 2 * PAD), SPINE_W + GLOW, min(left + dx, right - (MIN_CW + 2 * PAD)))
    elif "R" in edges:
        stop = min(left + max_cw + 2 * PAD, W - GLOW, W - GLOW - MIN_MARGIN - lfw - LENS_GAP)
        right = min(stop, max(right + dx, left + MIN_CW + 2 * PAD))
    if "T" in edges:
        top_lo = max(bottom - (max_ch + TITLE_H + PAD), BAR_H + GLOW)
        top_hi = min(bottom - (MIN_CH + TITLE_H + PAD), H - GLOW - lfh)
        top = max(top_lo, min(top_hi, top + dy))
    elif "B" in edges:
        stop = min(top + max_ch + TITLE_H + PAD, H - GLOW)
        bottom = min(stop, max(bottom + dy, top + MIN_CH + TITLE_H + PAD))
    return left, top, right - left - 2 * PAD, bottom - top - TITLE_H - PAD


def bloom_origin(W, H, frame, client):
    """None when the workfield cannot hold the group at all (no bloom)."""
    fw, fh = client[0] + 2 * PAD, client[1] + TITLE_H + PAD
    gw, gh = group_wh(fw, fh)
    if W - SPINE_W < gw + 2 * GLOW or H - BAR_H < gh + 2 * GLOW:
        return None
    return dock_move(W, H, fw, fh, frame[0], frame[1])


def backdrop(W, H):
    """(H, W, 3) int64 wide values."""
    ys, xs = np.mgrid[0:H, 0:W].astype(np.int64)
    span = max(H - 1, 1)
    t = np.minimum(ys, span)
    out = np.zeros((H, W, 3), dtype=np.int64)
    for c in range(3):
        out[..., c] = (VOID[c] * 256 * (span - t) + THRESHOLD[c] * 256 * t + span // 2) // span
    cx, cy = W * 62 // 100, H * 48 // 100
    lights = [(cx, cy, H * 70 // 100, (16, 40, 86)), (W * 12 // 100, H * 96 // 100, H * 60 // 100, (26, 10, 44))]
    for lx, ly, r, peak in lights:
        d2 = (xs - lx) ** 2 + (ys - ly) ** 2
        r2 = r * r
        inside = d2 < r2
        q = np.where(inside, ((r2 - d2) << 16) // r2, 0)
        f = (q * q) >> 16
        for c in range(3):
            out[..., c] += (peak[c] * 256 * f) >> 16
    for R, peak in [(H * 58 // 100, (22, 48, 96)), (H * 80 // 100, (16, 34, 70))]:
        d2 = (xs - cx) ** 2 + (ys - cy) ** 2
        num = 8 * (d2 - R * R)
        s = np.sign(num) * (np.abs(num) // R)  # truncation toward zero
        a = np.maximum(RING_HW - np.abs(s), 0)
        f = (a << 16) // RING_HW
        for c in range(3):
            out[..., c] += (peak[c] * 256 * f) >> 16
    return np.minimum(out, 65535)


def box_axis(a, r, axis):
    """One edge-clamped box of radius r along `axis` via cumulative sums:
    pad by r with edge copies, prefix-sum with a leading zero, and take the
    window sum cs[i + 2r + 1] - cs[i] for each output i."""
    n = a.shape[axis]
    pad = [(0, 0)] * a.ndim
    pad[axis] = (r, r)
    p = np.pad(a, pad, mode="edge")
    zshape = list(p.shape)
    zshape[axis] = 1
    cs = np.concatenate([np.zeros(zshape, dtype=p.dtype), np.cumsum(p, axis=axis)], axis=axis)
    hi = cs.take(np.arange(2 * r + 1, 2 * r + 1 + n), axis=axis)
    lo = cs.take(np.arange(0, n), axis=axis)
    return (hi - lo + r) // (2 * r + 1)


def blur(bd, rect, W, H):
    x, y, w, h = rect
    sx, sy = max(x - MARGIN, 0), max(y - MARGIN, 0)
    ex, ey = min(x + w + MARGIN, W), min(y + h + MARGIN, H)
    a = bd[sy:ey, sx:ex].copy()
    for _ in range(3):
        a = box_axis(a, BLUR_R, 1)
        a = box_axis(a, BLUR_R, 0)
    return a[y - sy:y - sy + h, x - sx:x - sx + w]


def area(x0, y0, r):
    """Exact area of [x0,x0+1]x[y0,y0+1] inside the disc of radius r at 0."""
    if x0 >= r or y0 >= r:
        return 0.0

    def G(t):
        return 0.5 * (t * math.sqrt(max(r * r - t * t, 0.0)) + r * r * math.asin(min(t / r, 1.0)))

    lo_t = math.sqrt(r * r - (y0 + 1) ** 2) if y0 + 1 < r else 0.0
    hi_t = math.sqrt(r * r - y0 * y0)
    a = min(max(lo_t, x0), x0 + 1)
    b = min(max(hi_t, x0), x0 + 1)
    return min(max((a - x0) + (G(b) - G(a)) - y0 * (b - a), 0.0), 1.0)


def cov_rr(rect, r, W, H):
    """(H, W) uint coverage of a rounded rect."""
    x, y, w, h = rect
    r = min(r, w // 2, h // 2)
    cov = np.zeros((H, W), dtype=np.int64)
    cov[y:y + h, x:x + w] = 255
    if r == 0:
        return cov
    m = np.zeros((r, r), dtype=np.int64)
    for j in range(r):
        for i in range(r):
            m[j, i] = math.floor(255.0 * area(r - i - 1, r - j - 1, float(r)) + 0.5)
    cov[y:y + r, x:x + r] = m
    cov[y:y + r, x + w - r:x + w] = m[:, ::-1]
    cov[y + h - r:y + h, x:x + r] = m[::-1, :]
    cov[y + h - r:y + h, x + w - r:x + w] = m[::-1, ::-1]
    return cov


def inset1(rect, r):
    x, y, w, h = rect
    r = min(r, w // 2, h // 2)
    return (x + 1, y + 1, max(w - 2, 0), max(h - 2, 0)), max(r - 1, 0)


def tint(b, frost):
    keep = 256 - frost - VIB
    out = np.empty_like(b)
    for c in range(3):
        out[..., c] = (b[..., c] * keep + WHITE_W * frost + EB[c] * 256 * VIB + 128) >> 8
    return out


def mix(top, below, cov):
    c = cov[..., None]
    return (top * c + below * (255 - c) + 127) // 255


def lighten(px, a):
    a = np.asarray(a)[..., None] if np.ndim(a) else a
    return px + ((WHITE_W - px) * a + 127) // 255


def solid(color, shape):
    out = np.empty(shape + (3,), dtype=np.int64)
    for c in range(3):
        out[..., c] = color[c] * 256
    return out


def glow_alpha(rect, radius, xx, yy):
    fx, fy, fw, fh = rect
    r = float(radius)
    hw, hh = fw / 2.0, fh / 2.0
    px_ = xx + 0.5 - (fx + hw)
    py_ = yy + 0.5 - (fy + hh)
    qx = max(abs(px_) - (hw - r), 0.0)
    qy = max(abs(py_) - (hh - r), 0.0)
    d = math.sqrt(qx * qx + qy * qy) - r
    if d <= 0.0 or d > GLOW:
        return 0
    t = (GLOW - d) / GLOW
    return math.floor(GLOW_PEAK * t * t + 0.5)


def render(W, H, mode, focused, view="window", held=False, chip_sel=False, lens=False,
           lens_sel=False, frame_at=None, client=(640, 400), card_rows=0, install=False,
           install_sel=False):
    """view: "window" (S3) or "home" (DS-1: no window, the intent field).
    Home only: held = Arthur yielded (a dot under its chip); chip_sel = the
    keyboard is on the Spine chip (the focus light moves from field to chip).
    lens (DS-1c): the second chip, and in the window view the Lens frame,
    its dashed read-only halo and its client floor. lens_sel (spec v1.5): the
    selection is on the Lens, so the chip light is around the second chip —
    in the window view always, in Home only while chip_sel. frame_at / client
    (DI-1b): the window frame moved and/or resized. card_rows (v1.8, Home
    only): a card of that many rows under the field (0: none; the hint is
    text only, so it is the label oracle's, not drawn here). install /
    install_sel (OT-87 R4.4b): the third chip, and the selection on it (its
    chip lit in Home only, while chip_sel)."""
    L = layout(W, H, client[0], client[1], frame_at=frame_at)
    lens_win = lens and view == "window" and L["lens"] is not None
    home = view == "home"
    field_lit = home and focused and not chip_sel
    chip_lit = home and focused and chip_sel
    lens_lit = lens and lens_sel and (not home or chip_lit)
    install_lit = install and install_sel and chip_lit
    if home:
        L = dict(L, labels=L["home_labels"])
    bd = backdrop(W, H)
    out = bd.copy()
    glass = mode == "glass"

    def panel_src(rect, frost, rt_color):
        x, y, w, h = rect
        if glass:
            return tint(blur(bd, rect, W, H), frost)
        return solid(rt_color, (h, w))

    # craft bar and Spine: square panels, coverage 255, edge hairline
    for key, hair in (("bar", "bottom"), ("spine", "right")):
        x, y, w, h = L[key]
        out[y:y + h, x:x + w] = panel_src(L[key], FROST, RT_PANEL)
        if hair == "bottom":
            out[y + h - 1, x:x + w] = lighten(out[y + h - 1, x:x + w], HAIR)
        else:
            out[y:y + h, x + w - 1] = lighten(out[y:y + h, x + w - 1], HAIR)

    # rounded panels: chip, then window frame
    if chip_lit or lens_lit:
        lit = L["chip3"] if install_lit else (L["chip2"] if lens_lit else L["chip"])
        cx, cy, cw, ch = lit
        for yy in range(cy - GLOW, cy + ch + GLOW):
            for xx in range(cx - GLOW, cx + cw + GLOW):
                a = glow_alpha(lit, CHIP_R, xx, yy)
                if a:
                    for c in range(3):
                        out[yy, xx, c] = (EB[c] * 256 * a + int(out[yy, xx, c]) * (255 - a) + 127) // 255
    def draw_held_and_chip2():
        if held:
            cx, cy, cw, ch = L["chip"]
            dot = (cx + (CHIP - 8) // 2, cy + ch + 6, 8, 8)
            x, y, w, h = dot
            cov = cov_rr(dot, 4, W, H)[y:y + h, x:x + w]
            out[y:y + h, x:x + w] = mix(solid(EB, (h, w)), out[y:y + h, x:x + w], cov)
        if lens:
            # frosted from the Spine's blur, sampled at the same pixel
            sx, sy, sw, sh = L["spine"]
            x, y, w, h = L["chip2"]
            if glass:
                spine_blur = blur(bd, L["spine"], W, H)
                src = tint(spine_blur[y - sy:y - sy + h, x - sx:x - sx + w], CHIP_FROST)
            else:
                src = solid(RT_RAISED, (h, w))
            rounded_panel(L["chip2"], CHIP_R, src)
        if install:
            # as the second chip: frosted from the Spine's blur
            sx, sy, sw, sh = L["spine"]
            x, y, w, h = L["chip3"]
            if glass:
                spine_blur = blur(bd, L["spine"], W, H)
                src = tint(spine_blur[y - sy:y - sy + h, x - sx:x - sx + w], CHIP_FROST)
            else:
                src = solid(RT_RAISED, (h, w))
            rounded_panel(L["chip3"], CHIP_R, src)

    def rounded_panel(rect, radius, src):
        x, y, w, h = rect
        cov = cov_rr(rect, radius, W, H)[y:y + h, x:x + w]
        ir, irad = inset1(rect, radius)
        inner = cov_rr(ir, irad, W, H)[y:y + h, x:x + w]
        region = out[y:y + h, x:x + w]
        px = mix(src, region, cov)
        ring = np.maximum(cov - inner, 0)
        lit = lighten(px, (HAIR * ring + 127) // 255)
        px = np.where((ring > 0)[..., None], lit, px)
        out[y:y + h, x:x + w] = np.where((cov > 0)[..., None], px, region)

    panels = [("chip", CHIP_R, CHIP_FROST, RT_RAISED)]
    if not home:
        panels.append(("frame", WIN_R, FROST, RT_PANEL))
    for key, radius, frost, rt in panels:
        if key == "frame" and lens:
            # the held dot and the second chip come before the window frame
            draw_held_and_chip2()
        x, y, w, h = L[key]
        cov = cov_rr(L[key], radius, W, H)[y:y + h, x:x + w]
        ir, irad = inset1(L[key], radius)
        inner = cov_rr(ir, irad, W, H)[y:y + h, x:x + w]
        src = panel_src(L[key], frost, rt)
        region = out[y:y + h, x:x + w]
        px = mix(src, region, cov)
        ring = np.maximum(cov - inner, 0)
        a = (HAIR * ring + 127) // 255
        lit = lighten(px, a)
        px = np.where((ring > 0)[..., None], lit, px)
        out[y:y + h, x:x + w] = np.where((cov > 0)[..., None], px, region)

        if key == "frame":
            dy, dx = np.mgrid[0:h, 0:w]
            near_x = np.minimum(dx, w - 1 - dx)
            near_y = np.minimum(dy, h - 1 - dy)
            along = lambda d: (d >= WIN_R) & (d < WIN_R + TICK_LEN)
            tick = ((near_y < TICK_W) & along(near_x)) | ((near_x < TICK_W) & along(near_y))
            blk = out[y:y + h, x:x + w]
            blk[tick] = np.array([c * 256 for c in EB])

    if not (lens and not home):
        draw_held_and_chip2()
    if lens_win:
        lf = L["lens"]["frame"]
        rounded_panel(lf, WIN_R, panel_src(lf, FROST, RT_PANEL))
        rx, ry, rw, rh = L["lens"]["ring"]
        ring = np.zeros((rh, rw), dtype=bool)
        ring[0, :] = ring[rh - 1, :] = (np.arange(rw) % 10) < 6
        side = (np.arange(rh) % 10) < 6
        ring[1:rh - 1, 0] = ring[1:rh - 1, rw - 1] = side[1:rh - 1]
        blk = out[ry:ry + rh, rx:rx + rw]
        blk[ring] = lighten(blk[ring], HALO_A)
    accent = EB if (focused and not home) else INACTIVE
    x, y, w, h = L["marker"]
    out[y:y + h, x:x + w] = np.array([c * 256 for c in accent])
    x, y, w, h = L["status"]
    cov = cov_rr(L["status"], w // 2, W, H)[y:y + h, x:x + w]
    out[y:y + h, x:x + w] = mix(solid(EB, (h, w)), out[y:y + h, x:x + w], cov)
    if not home:
        x, y, w, h = L["client"]
        out[y:y + h, x:x + w] = np.array([c * 256 for c in CONTENT])
        if lens_win:
            x, y, w, h = L["lens"]["client"]
            out[y:y + h, x:x + w] = np.array([c * 256 for c in CONTENT])
    else:
        x, y, w, h = L["field"]
        if field_lit:
            # the Home focus light, in wide units before the dither
            for yy in range(y - GLOW, y + h + GLOW):
                for xx in range(x - GLOW, x + w + GLOW):
                    a = glow_alpha(L["field"], FIELD_R, xx, yy)
                    if a:
                        for c in range(3):
                            out[yy, xx, c] = (EB[c] * 256 * a + int(out[yy, xx, c]) * (255 - a) + 127) // 255
        cov = cov_rr(L["field"], FIELD_R, W, H)[y:y + h, x:x + w]
        ir, irad = inset1(L["field"], FIELD_R)
        inner = cov_rr(ir, irad, W, H)[y:y + h, x:x + w]
        src = panel_src(L["field"], FROST, RT_RAISED)
        region = out[y:y + h, x:x + w]
        px = mix(src, region, cov)
        ring = np.maximum(cov - inner, 0)
        lit = lighten(px, (HAIR * ring + 127) // 255)
        px = np.where((ring > 0)[..., None], lit, px)
        out[y:y + h, x:x + w] = np.where((cov > 0)[..., None], px, region)
        if card_rows:
            # v1.8: the intro card, opaque content floor, full hairline ring
            card = card_rect(L, card_rows)
            rounded_panel(card, FIELD_R, solid(CONTENT, (card[3], card[2])))

    # dither
    ys, xs = np.mgrid[0:H, 0:W]
    bay = np.array(BAYER, dtype=np.int64)[ys & 3, xs & 3] * 16 + 8
    rgb = np.minimum((out + bay[..., None]) >> 8, 255)

    if focused and not home:
        fx, fy, fw, fh = L["frame"]
        gx, gy, gw, gh = fx - GLOW, fy - GLOW, fw + 2 * GLOW, fh + 2 * GLOW
        r = float(WIN_R)
        hw, hh = fw / 2.0, fh / 2.0
        for yy in range(gy, gy + gh):
            for xx in range(gx, gx + gw):
                px_ = xx + 0.5 - (fx + hw)
                py_ = yy + 0.5 - (fy + hh)
                qx = max(abs(px_) - (hw - r), 0.0)
                qy = max(abs(py_) - (hh - r), 0.0)
                d = math.sqrt(qx * qx + qy * qy) - r
                if d <= 0.0 or d > GLOW:
                    continue
                t = (GLOW - d) / GLOW
                a = math.floor(GLOW_PEAK * t * t + 0.5)
                if a == 0:
                    continue
                for c in range(3):
                    rgb[yy, xx, c] = (EB[c] * a + int(rgb[yy, xx, c]) * (255 - a) + 127) // 255
    return rgb, L


def digest(rgb, L):
    H, W, _ = rgb.shape
    words = (rgb[..., 0] << 16) | (rgb[..., 1] << 8) | rgb[..., 2]
    for (x, y, w, h) in L["labels"]:
        words[y:y + h, x:x + w] = 0
    data = words.astype("<u4").tobytes()
    hsh = 0xCBF29CE484222325
    for b in data:
        hsh ^= b
        hsh = (hsh * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return hsh


# DL-1 dock-fixture cases at 1920x1080 (frame x, frame y, client w, client h):
# desktop_full_boot's end state (moved, then the bottom-right corner dragged
# +120,+60 with the Lens docked); where its bloom slid the window; the
# smallest client at the clamp corner (the Lens taller than the window).
DOCK_CASES = ((626, 357, 760, 460), (826, 397, 640, 400), (98, 50, 320, 200))
# Docked moves: (frame w, frame h, requested x, y).
DOCK_MOVES = ((656, 442, 974, 397), (656, 442, -500, -500), (656, 442, 626, 357),
              (336, 242, 1500, 900), (1016, 542, 900, 900))
# Bloom origins: (frame x, y, client w, h) of an undocked window.
BLOOM_FRAMES = ((974, 397, 640, 400), (674, 337, 640, 400), (1250, 624, 640, 400),
                (98, 50, 1280, 800))
# Docked resizes: (frame x, y, client w, h, edges, dx, dy).
DOCK_RESIZES = ((626, 357, 640, 400, "RB", 120, 60), (826, 397, 640, 400, "R", 500, 0),
                (626, 357, 640, 400, "T", 0, -500), (626, 357, 640, 400, "T", 0, 500),
                (626, 600, 640, 400, "T", 0, 500), (826, 397, 640, 400, "L", -1000, 0),
                (626, 357, 640, 400, "B", 0, 1000))
EDGE_BITS = {"L": 1, "R": 2, "T": 4, "B": 8}


def dock_fixture():
    W, H = 1920, 1080
    print("// SPDX-License-Identifier: MPL-2.0")
    print("// GENERATED by scripts/desktop/scene_oracle.py dock-fixture — do not edit.")
    print("// DL-1: the Lens docked to a moved / resized window (spec v1.6): FNV-1a over")
    print("// every visible pixel of the window view with the Lens shown and Arthur")
    print("// focused, the mask boxes (labels, both client areas) hashed as zero; and")
    print("// the docked group's clamps, written from the spec. Independent of neo-atrium.")
    print("/// (frame x, frame y, client w, client h) per case, in DIGESTS order.")
    print(f"pub const DOCK_CASES: [(usize, usize, usize, usize); {len(DOCK_CASES)}] = [")
    for c in DOCK_CASES:
        print("    (%d, %d, %d, %d)," % c)
    print("];")
    print("/// Mask boxes per case: bar, title and chip labels, the second chip's and")
    print("/// the Lens title's labels, then Arthur's and the Lens's client areas.")
    print(f"pub const DOCK_MASKS: [[(usize, usize, usize, usize); 7]; {len(DOCK_CASES)}] = [")
    for fx, fy, cw, ch in DOCK_CASES:
        L = layout(W, H, cw, ch, frame_at=(fx, fy))
        lens = L["lens"]
        boxes = L["labels"] + [L["chip2_label"], lens["title"], L["client"], lens["client"]]
        print("    [" + ", ".join("(%d, %d, %d, %d)" % b for b in boxes) + "],")
    print("];")
    for mode in ("glass", "rt"):
        print(f"/// {mode}: the window view focused, Lens docked, per case.")
        print(f"pub const DOCK_{mode.upper()}_FOCUSED: [u64; {len(DOCK_CASES)}] = [")
        for fx, fy, cw, ch in DOCK_CASES:
            L = layout(W, H, cw, ch, frame_at=(fx, fy))
            lens = L["lens"]
            boxes = L["labels"] + [L["chip2_label"], lens["title"], L["client"], lens["client"]]
            rgb, _ = render(W, H, mode, True, "window", lens=True, frame_at=(fx, fy),
                            client=(cw, ch))
            print(f"    0x{digest(rgb, dict(L, labels=boxes)):016x},")
        print("];")
    print("/// Docked moves: (frame w, frame h, requested x, y) -> where the frame lands.")
    print("pub type DockMove = ((usize, usize, i64, i64), (usize, usize));")
    print(f"pub const DOCK_MOVES: [DockMove; {len(DOCK_MOVES)}] = [")
    for fw, fh, x, y in DOCK_MOVES:
        print("    ((%d, %d, %d, %d), (%d, %d))," % ((fw, fh, x, y) + dock_move(W, H, fw, fh, x, y)))
    print("];")
    print("/// Bloom origins: (frame x, y, client w, h) -> where the frame must be for")
    print("/// the Lens to fit beside it.")
    print("/// `None`: the workfield cannot hold the group (the bloom is refused).")
    print("pub type BloomOrigin = ((usize, usize, usize, usize), Option<(usize, usize)>);")
    print(f"pub const BLOOM_ORIGINS: [BloomOrigin; {len(BLOOM_FRAMES)}] = [")
    for fx, fy, cw, ch in BLOOM_FRAMES:
        o = bloom_origin(W, H, (fx, fy), (cw, ch))
        at = "None" if o is None else "Some((%d, %d))" % o
        print("    ((%d, %d, %d, %d), %s)," % (fx, fy, cw, ch, at))
    print("];")
    print("/// Docked resizes: (frame x, y, client w, h, edge bits, dx, dy) ->")
    print("/// (frame x, y, client w, h).")
    print("pub type DockResize = ((usize, usize, usize, usize, u8, i64, i64), (usize, usize, usize, usize));")
    print(f"pub const DOCK_RESIZES: [DockResize; {len(DOCK_RESIZES)}] = [")
    for fx, fy, cw, ch, edges, dx, dy in DOCK_RESIZES:
        bits = sum(EDGE_BITS[e] for e in edges)
        r = dock_resize(W, H, (fx, fy), (cw, ch), edges, dx, dy)
        print("    ((%d, %d, %d, %d, %d, %d, %d), (%d, %d, %d, %d))," % ((fx, fy, cw, ch, bits, dx, dy) + r))
    print("];")


# --- DM-1 motion (spec v1.7 "Motion", motion.rs prose) ------------------------
# Written from the text: numpy masks for the iris, per-channel uint arithmetic
# for the blend (the Rust packs two bytes per lane).
MOTION_PLANS = {  # (profile, kind) -> (frames, shape)
    ("full", "open"): (12, "grow"), ("full", "close"): (10, "shrink"),
    ("full", "view"): (10, "dissolve"), ("full", "bloom"): (12, "grow"),
    ("full", "unbloom"): (10, "shrink"), ("full", "focus"): (8, "dissolve"),
}
for _kind in ("open", "close", "view", "bloom", "unbloom", "focus"):
    MOTION_PLANS[("reduced", _kind)] = (6, "dissolve")
    MOTION_PLANS[("off", _kind)] = (1, "dissolve")


def m_weight(k, n):
    return 256 + (-(256 * (n - k) ** 3) // (n ** 3))  # 256 - ceil(...)


def m_blend(f, t, a):
    """f, t: uint32 XRGB arrays; every byte (f(256-a) + t a + 128) >> 8."""
    out = np.zeros_like(f, dtype=np.uint64)
    for sh in (0, 8, 16, 24):
        fb = (f.astype(np.uint64) >> sh) & 0xFF
        tb = (t.astype(np.uint64) >> sh) & 0xFF
        out |= (((fb * (256 - a) + tb * a + 128) >> 8) & 0xFF) << sh
    return out.astype(np.uint32)


def m_iris(shape, anchor, bounds, k, n):
    a = m_weight(k, n)
    s = a if shape == "grow" else 256 - a
    cx, cy = anchor[0] + anchor[2] // 2, anchor[1] + anchor[3] // 2
    bx, by, bw, bh = bounds
    hw = max(cx - bx, bx + bw - cx, 0)
    hh = max(cy - by, by + bh - cy, 0)
    rx, ry = -(-hw * s // 256), -(-hh * s // 256)
    return max(cx - rx, 0), max(cy - ry, 0), cx + rx, cy + ry  # x0, y0, x1, y1


def m_frame(dst, F, T, rects, shape, anchor, k, n):
    """Frame k of n over rects (dst, F, T: H x W uint32)."""
    a = m_weight(k, n)
    xs = [r[0] for r in rects]; ys = [r[1] for r in rects]
    bounds = (min(xs), min(ys), max(r[0] + r[2] for r in rects) - min(xs),
              max(r[1] + r[3] for r in rects) - min(ys))
    H, W = dst.shape
    yy, xx = np.mgrid[0:H, 0:W]
    inside = np.zeros((H, W), bool)
    for (x, y, w, h) in rects:
        inside[y:y + h, x:x + w] = True
    mixed = m_blend(F, T, a)
    if shape == "dissolve":
        val = mixed
    else:
        x0, y0, x1, y1 = m_iris(shape, anchor, bounds, k, n)
        in_iris = (xx >= x0) & (xx < x1) & (yy >= y0) & (yy < y1)
        val = np.where(in_iris, mixed, F if shape == "grow" else T)
    dst[inside] = val[inside]


def fnv_words(words):
    hsh = 0xCBF29CE484222325
    for b in words.astype("<u4").tobytes():
        hsh ^= b
        hsh = (hsh * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return hsh


def lcg_plane(seed, W, H):
    v, out = seed, np.zeros(W * H, np.uint32)
    for i in range(W * H):
        v = (v * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
        out[i] = (v >> 32) & 0xFFFFFFFF
    return out.reshape(H, W)


# Synthetic case (64x48): two rects, an anchor inside the first.
SYN_W, SYN_H = 64, 48
SYN_RECTS = ((4, 4, 30, 20), (40, 10, 20, 30))
SYN_ANCHOR = (20, 14, 10, 8)
SYN_FRAMES = (1, 4, 6, 11, 12)  # of 12
SYN_FILL = 0xDEADBEEF
# The frame the boot test holds the first hold at (frame 3: the profiles'
# weights differ there; at half way every n gives a = 224).
HOLD_FRAME = 3


def words_of(rgb):
    return ((rgb[..., 0].astype(np.uint32) << 16) | (rgb[..., 1].astype(np.uint32) << 8)
            | rgb[..., 2].astype(np.uint32))


def motion_fixture():
    print("// SPDX-License-Identifier: MPL-2.0")
    print("// GENERATED by scripts/desktop/scene_oracle.py motion-fixture — do not edit.")
    print("// DM-1 motion (spec v1.7): the plans and weights written from the spec text;")
    print("// a synthetic two-rect case per shape; and the mid-transition frame of the")
    print("// first hold (the window view focused -> Home, Arthur held) as FNV-1a over")
    print("// every visible pixel, labels and Arthur's client hashed as zero.")
    print("// Independent of neo-atrium.")
    profiles = ("full", "reduced", "off")
    kinds = ("open", "close", "view", "bloom", "unbloom", "focus")
    shapes = {"dissolve": 0, "grow": 1, "shrink": 2}
    print("/// (profile 0 full / 1 reduced / 2 off, kind in Kind order) -> (frames, shape")
    print("/// 0 dissolve / 1 grow / 2 shrink).")
    print(f"pub const PLANS: [((u8, u8), (u32, u8)); {len(profiles) * len(kinds)}] = [")
    for pi, p in enumerate(profiles):
        for ki, k in enumerate(kinds):
            n, sh = MOTION_PLANS[(p, k)]
            print(f"    (({pi}, {ki}), ({n}, {shapes[sh]})),")
    print("];")
    for n in (6, 8, 10, 12):
        ws = ", ".join(str(m_weight(k, n)) for k in range(1, n + 1))
        print(f"/// a(k) for k = 1..={n}.")
        print(f"pub const WEIGHTS_{n}: [u32; {n}] = [{ws}];")
    F = lcg_plane(1, SYN_W, SYN_H)
    T = lcg_plane(2, SYN_W, SYN_H)
    print(f"/// Synthetic: {SYN_W}x{SYN_H} planes F = lcg(1), T = lcg(2), dst filled with")
    print(f"/// 0x{SYN_FILL:08X}; frame k of 12 over the rects, anchor {SYN_ANCHOR}.")
    print("/// (shape, k) -> FNV-1a of dst.")
    print(f"pub const SYN_RECTS: [(usize, usize, usize, usize); {len(SYN_RECTS)}] = {list(SYN_RECTS)};".replace("[(", "[(").replace("]", "]"))
    print(f"pub const SYN_ANCHOR: (usize, usize, usize, usize) = {SYN_ANCHOR};")
    cases = []
    for sh in ("dissolve", "grow", "shrink"):
        for k in SYN_FRAMES:
            dst = np.full((SYN_H, SYN_W), SYN_FILL, np.uint32)
            m_frame(dst, F, T, SYN_RECTS, sh, SYN_ANCHOR, k, 12)
            cases.append((shapes[sh], k, fnv_words(dst)))
    print(f"pub const SYN_DIGESTS: [(u8, u32, u64); {len(cases)}] = [")
    for sh, k, d in cases:
        print(f"    ({sh}, {k}, 0x{d:016x}),")
    print("];")
    # The first hold of desktop_motion_boot: F = the window view focused (no
    # Lens yet), T = Home with Arthur held; the frame the boot test holds at.
    L = layout(1920, 1080)
    masks = L["labels"] + L["home_labels"][1:2] + [L["client"]]
    print("/// The mid-hold mask boxes at 1920x1080: the window view's labels, the Home")
    print("/// field's text box, Arthur's client.")
    print(f"pub const HOLD_MASK_BOXES: [(usize, usize, usize, usize); {len(masks)}] = [")
    for b in masks:
        print("    (%d, %d, %d, %d)," % b)
    print("];")
    for mode in ("glass",):
        f_rgb, _ = render(1920, 1080, mode, True, "window")
        t_rgb, _ = render(1920, 1080, mode, True, "home", held=True)
        Fw, Tw = words_of(f_rgb), words_of(t_rgb)
        for prof in ("full", "reduced"):
            n, _ = MOTION_PLANS[(prof, "view")]
            k = HOLD_FRAME
            mid = m_blend(Fw, Tw, m_weight(k, n))
            for (x, y, w, h) in masks:
                mid[y:y + h, x:x + w] = 0
            print(f"/// {prof}: frame {k} of {n}.")
            print(f"pub const HOLD_MID_{prof.upper()}_FRAME: u32 = {k};")
            print(f"pub const HOLD_MID_{mode.upper()}_{prof.upper()}: u64 = 0x{fnv_words(mid):016x};")


def install_fixture():
    """OT-87 R4.4b: Home with the Install chip, at 1920x1080. FNV-1a over every
    visible pixel, the Home label boxes and the chips' label boxes hashed as
    zero; and with `app-icons`, the Install icon's box (the blob blended over
    this oracle's chip pixels inside the chip's label box, as icon-fixture)."""
    import pathlib
    print("// SPDX-License-Identifier: MPL-2.0")
    print("// GENERATED by scripts/desktop/scene_oracle.py install-fixture — do not edit.")
    print("// OT-87 R4.4b: Home with the Install chip (the third on the Spine): FNV-1a")
    print("// over every visible pixel, the label boxes hashed as zero. Independent of")
    print("// neo-atrium.")
    L = layout(1920, 1080)
    boxes = L["home_labels"] + [L["chip3_label"]]
    print("/// The mask boxes (x, y, w, h) at 1920x1080: the Home label boxes, then the")
    print("/// Install chip's.")
    print(f"pub const R44B_INSTALL_MASK_BOXES_1920X1080: [(usize, usize, usize, usize); {len(boxes)}] = [")
    for b in boxes:
        print("    (%d, %d, %d, %d)," % b)
    print("];")
    for mode in ("glass", "rt"):
        for tag, kw in (
            ("FOCUSED", dict()),
            ("HELD", dict(held=True)),
            ("CHIP_SELECTED", dict(held=True, chip_sel=True, install_sel=True)),
            ("ARTHUR_SELECTED", dict(held=True, chip_sel=True)),
        ):
            rgb, Lh = render(1920, 1080, mode, True, "home", install=True, **kw)
            print(f"pub const R44B_HOME_INSTALL_{mode.upper()}_1920X1080_{tag}: u64 = "
                  f"0x{digest(rgb, dict(Lh, labels=boxes)):016x};")
    # With the Lens alive too: both chips, the Lens's label box masked as well.
    lb = boxes + [L["chip2_label"]]
    print("/// With the Lens's chip too: its label box is masked as well.")
    print(f"pub const R44B_INSTALL_LENS_MASK_BOXES_1920X1080: [(usize, usize, usize, usize); {len(lb)}] = [")
    for b in lb:
        print("    (%d, %d, %d, %d)," % b)
    print("];")
    for mode in ("glass", "rt"):
        rgb, Lh = render(1920, 1080, mode, True, "home", held=True, lens=True, install=True)
        print(f"pub const R44B_HOME_INSTALL_LENS_{mode.upper()}_1920X1080_HELD: u64 = "
              f"0x{digest(rgb, dict(Lh, labels=lb)):016x};")
    # The intro session's Home end view (desktop-intro: the hint under the
    # field) with the chip: what `install_atrium`'s kernel reads back. The
    # hint's text box is masked as well, as intro_oracle.py's HINT digests.
    hb = L["home_labels"] + [card_text(L, 1), L["chip3_label"]]
    print("/// The intro session's end view with the chip: the Home label boxes, the")
    print("/// hint's text box, then the Install chip's.")
    print(f"pub const R44B_INSTALL_HINT_MASK_BOXES_1920X1080: [(usize, usize, usize, usize); {len(hb)}] = [")
    for b in hb:
        print("    (%d, %d, %d, %d)," % b)
    print("];")
    for mode in ("glass", "rt"):
        rgb, Lh = render(1920, 1080, mode, True, "home", install=True)
        print(f"pub const R44B_HOME_INSTALL_HINT_{mode.upper()}_1920X1080: u64 = "
              f"0x{digest(rgb, dict(Lh, labels=hb)):016x};")
    # And the DS-1a field check of that end view (intro_oracle.py's
    # HINT_FIELD): the field's tight window as intro.rs states it, the bar
    # and Arthur's chip labels, the hint box, and the Install chip's label.
    root = pathlib.Path(__file__).resolve().parents[2]
    import re
    intro_rs = (root / "fixtures/desktop-intro/intro.rs").read_text()
    win = tuple(int(v) for v in re.search(
        r"FIELD_WINDOW: \(usize, usize, usize, usize\) = \((\d+), (\d+), (\d+), (\d+)\)",
        intro_rs).groups())
    fb = [L["home_labels"][0], L["home_labels"][2], win, card_text(L, 1), L["chip3_label"]]
    rgb, Lh = render(1920, 1080, "glass", True, "home", install=True)
    print("/// The field check's masks with the chip: the bar and Arthur's chip labels,")
    print("/// the field's window (intro.rs's FIELD_WINDOW), the hint box, the Install chip's.")
    print(f"pub const R44B_INSTALL_HINT_FIELD_MASK_BOXES_1920X1080: [(usize, usize, usize, usize); {len(fb)}] = [")
    for b in fb:
        print("    (%d, %d, %d, %d)," % b)
    print("];")
    print(f"pub const R44B_HOME_INSTALL_HINT_FIELD_GLASS_1920X1080: u64 = "
          f"0x{digest(rgb, dict(Lh, labels=fb)):016x};")
    # `app-icons`: the Install chip's icon box.
    root = pathlib.Path(__file__).resolve().parents[2]
    raw = (root / "userland/neo-atrium/assets/icons/install-40.rgba").read_bytes()
    b = np.frombuffer(raw, dtype=np.uint8).reshape(40, 40, 4).astype(np.int64)
    cx, cy = L["chip3"][0], L["chip3"][1]
    ix, iy, rows = cx + 8, cy + 6, 39
    lx, ly, lw, lh = L["chip3_label"]
    print(f"pub const R44B_INSTALL_ICON_BOX_1920X1080: (usize, usize, usize, usize) = "
          f"({ix}, {iy}, 40, {rows});")
    for mode in ("glass", "rt"):
        rgb, _ = render(1920, 1080, mode, True, "home", install=True)
        out = rgb[iy:iy + rows, ix:ix + 40].astype(np.int64).copy()
        for r in range(rows):
            for c in range(40):
                x, y = ix + c, iy + r
                if lx <= x < lx + lw and ly <= y < ly + lh:
                    a = b[r, c, 3]
                    out[r, c] = (b[r, c, :3] * a + out[r, c] * (255 - a) + 127) // 255
        words = (out[..., 0] << 16) | (out[..., 1] << 8) | out[..., 2]
        hsh = 0xCBF29CE484222325
        for byte in words.astype("<u4").tobytes():
            hsh ^= byte
            hsh = (hsh * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
        print(f"pub const R44B_INSTALL_ICON_{mode.upper()}_1920X1080_FOCUSED: u64 = 0x{hsh:016x};")


def main():
    if len(sys.argv) >= 2 and sys.argv[1] == "fixture":
        print("// SPDX-License-Identifier: MPL-2.0")
        print("// GENERATED by scripts/desktop/scene_oracle.py fixture — do not edit.")
        print("// FNV-1a over every visible pixel (row-major, u32 little-endian), the")
        print("// three label boxes hashed as zero. Independent of neo-atrium.")
        boxes = layout(1920, 1080)["labels"]
        print("/// The label boxes (x, y, w, h) at 1920x1080, hashed as zero above.")
        print("pub const S3_LABEL_BOXES_1920X1080: [(usize, usize, usize, usize); 3] = [")
        for b in boxes:
            print("    (%d, %d, %d, %d)," % b)
        print("];")
        for (W, H) in ((1920, 1080), (1280, 800)):
            for mode in ("glass", "rt"):
                for focused in (False, True):
                    rgb, L = render(W, H, mode, focused)
                    name = f"S3_{mode.upper()}_{W}X{H}_{'FOCUSED' if focused else 'UNFOCUSED'}"
                    print(f"pub const {name}: u64 = 0x{digest(rgb, L):016x};")
        return
    if len(sys.argv) >= 2 and sys.argv[1] == "home-fixture":
        print("// SPDX-License-Identifier: MPL-2.0")
        print("// GENERATED by scripts/desktop/scene_oracle.py home-fixture — do not edit.")
        print("// DS-1 Home view (no window; the intent field): FNV-1a over every visible")
        print("// pixel, the three Home label boxes hashed as zero. Independent of neo-atrium.")
        boxes = layout(1920, 1080)["home_labels"]
        print("/// The Home label boxes (x, y, w, h) at 1920x1080, hashed as zero below.")
        print("pub const DS1_HOME_LABEL_BOXES_1920X1080: [(usize, usize, usize, usize); 3] = [")
        for b in boxes:
            print("    (%d, %d, %d, %d)," % b)
        print("];")
        for (W, H) in ((1920, 1080), (1280, 800)):
            for mode in ("glass", "rt"):
                for focused in (False, True):
                    rgb, L = render(W, H, mode, focused, "home")
                    name = f"DS1_HOME_{mode.upper()}_{W}X{H}_{'FOCUSED' if focused else 'UNFOCUSED'}"
                    print(f"pub const {name}: u64 = 0x{digest(rgb, L):016x};")
        # Arthur held (yielded): the field lit, then the chip selected (Tab).
        for mode in ("glass", "rt"):
            for chip_sel, tag in ((False, "HELD"), (True, "HELD_CHIP")):
                rgb, L = render(1920, 1080, mode, True, "home", held=True, chip_sel=chip_sel)
                print(f"pub const DS1_HOME_{mode.upper()}_1920X1080_{tag}: u64 = 0x{digest(rgb, L):016x};")
        return
    if len(sys.argv) >= 2 and sys.argv[1] == "icon-fixture":
        # `atrium-app-icons`: the DS-1 Home end view with Arthur's chip showing
        # its icon. The pinned blob (straight-alpha RGBA, rasterised on the
        # host by tools/icon-raster) is blended over this oracle's own chip
        # pixels, out = (s*a + d*(255 - a) + 127) // 255 per channel, inside
        # the chip's label box; the digest covers the icon box above the
        # chip's name (rows chip.y + 6 .. chip.y + 45), in digest()'s byte
        # format. The wrong-blob digest is printed for the negative control.
        import pathlib
        root = pathlib.Path(__file__).resolve().parents[2]

        def blob(name):
            raw = (root / "userland/neo-atrium/assets/icons" / f"{name}-40.rgba").read_bytes()
            return np.frombuffer(raw, dtype=np.uint8).reshape(40, 40, 4).astype(np.int64)

        def icon_digest(rgb, L, b):
            cx, cy = L["chip"][0], L["chip"][1]
            ix, iy, rows = cx + 8, cy + 6, 39
            lx, ly, lw, lh = L["home_labels"][2]
            out = rgb[iy:iy + rows, ix:ix + 40].astype(np.int64).copy()
            for r in range(rows):
                for c in range(40):
                    x, y = ix + c, iy + r
                    if lx <= x < lx + lw and ly <= y < ly + lh:
                        a = b[r, c, 3]
                        out[r, c] = (b[r, c, :3] * a + out[r, c] * (255 - a) + 127) // 255
            words = (out[..., 0] << 16) | (out[..., 1] << 8) | out[..., 2]
            hsh = 0xCBF29CE484222325
            for byte in words.astype("<u4").tobytes():
                hsh ^= byte
                hsh = (hsh * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
            return hsh, (ix, iy, 40, rows)

        print("// SPDX-License-Identifier: MPL-2.0")
        print("// GENERATED by scripts/desktop/scene_oracle.py icon-fixture — do not edit.")
        print("// atrium-app-icons: FNV-1a over Arthur's icon box on the DS-1 Home end view,")
        print("// the pinned blob blended over this oracle's chip pixels. Independent of neo-atrium.")
        arthur, lens = blob("arthur"), blob("lens")
        for mode in ("glass", "rt"):
            for focused in (False, True):
                rgb, L = render(1920, 1080, mode, focused, "home")
                d, box = icon_digest(rgb, L, arthur)
                name = f"ARTHUR_ICON_{mode.upper()}_1920X1080_{'FOCUSED' if focused else 'UNFOCUSED'}"
                if mode == "glass" and not focused:
                    print("/// The icon box (x, y, w, h) at 1920x1080: above the chip's name.")
                    print("pub const ICON_BOX_1920X1080: (usize, usize, usize, usize) = (%d, %d, %d, %d);" % box)
                print(f"pub const {name}: u64 = 0x{d:016x};")
        rgb, L = render(1920, 1080, "glass", True, "home")
        wrong, _ = icon_digest(rgb, L, lens)
        print("/// Negative control: the Lens blob on Arthur's chip (glass, focused).")
        print(f"pub const WRONG_BLOB_GLASS_1920X1080_FOCUSED: u64 = 0x{wrong:016x};")
        return
    if len(sys.argv) >= 2 and sys.argv[1] == "move-fixture":
        print("// SPDX-License-Identifier: MPL-2.0")
        print("// GENERATED by scripts/desktop/scene_oracle.py move-fixture — do not edit.")
        print("// DI-1b: the S3 window view with the frame moved (title-band drag): FNV-1a")
        print("// over every visible pixel, the three label boxes (which move with the")
        print("// frame) hashed as zero. Independent of neo-atrium.")
        for (fx, fy) in MOVE_ORIGINS:
            boxes = layout(1920, 1080, frame_at=(fx, fy))["labels"]
            print(f"/// The label boxes with the frame at ({fx}, {fy}), 1920x1080.")
            print(f"pub const MOVED_{fx}_{fy}_LABEL_BOXES_1920X1080: [(usize, usize, usize, usize); 3] = [")
            for b in boxes:
                print("    (%d, %d, %d, %d)," % b)
            print("];")
            for mode in ("glass", "rt"):
                for focused in (False, True):
                    rgb, L = render(1920, 1080, mode, focused, frame_at=(fx, fy))
                    name = f"MOVED_{fx}_{fy}_{mode.upper()}_1920X1080_{'FOCUSED' if focused else 'UNFOCUSED'}"
                    print(f"pub const {name}: u64 = 0x{digest(rgb, L):016x};")
        return
    if len(sys.argv) >= 2 and sys.argv[1] == "resize-fixture":
        print("// SPDX-License-Identifier: MPL-2.0")
        print("// GENERATED by scripts/desktop/scene_oracle.py resize-fixture — do not edit.")
        print("// DI-1b: the S3 window view with the frame resized (edge/corner drag):")
        print("// FNV-1a over every visible pixel, the three label boxes hashed as zero.")
        print("// Independent of neo-atrium.")
        for (fx, fy, cw, ch) in RESIZE_CASES:
            tag = f"RESIZED_{fx}_{fy}_{cw}X{ch}"
            boxes = layout(1920, 1080, cw, ch, frame_at=(fx, fy))["labels"]
            print(f"/// The label boxes with a {cw}x{ch} client, frame at ({fx}, {fy}), 1920x1080.")
            print(f"pub const {tag}_LABEL_BOXES_1920X1080: [(usize, usize, usize, usize); 3] = [")
            for b in boxes:
                print("    (%d, %d, %d, %d)," % b)
            print("];")
            for mode in ("glass", "rt"):
                for focused in (False, True):
                    rgb, L = render(1920, 1080, mode, focused, frame_at=(fx, fy), client=(cw, ch))
                    name = f"{tag}_{mode.upper()}_1920X1080_{'FOCUSED' if focused else 'UNFOCUSED'}"
                    print(f"pub const {name}: u64 = 0x{digest(rgb, L):016x};")
        return
    if len(sys.argv) >= 2 and sys.argv[1] == "lens-fixture":
        print("// SPDX-License-Identifier: MPL-2.0")
        print("// GENERATED by scripts/desktop/scene_oracle.py lens-fixture — do not edit.")
        print("// DS-1c: the window view with the Lens bloomed (focused, as the WM shows it")
        print("// while Arthur has the keyboard): FNV-1a over every visible pixel, the mask")
        print("// boxes (labels and both client areas) hashed as zero. Independent of neo-atrium.")
        L = layout(1920, 1080)
        lens = L["lens"]
        boxes = L["labels"] + [L["chip2_label"], lens["title"], L["client"], lens["client"]]
        print("/// The mask boxes (x, y, w, h) at 1920x1080: bar, title, chip, chip 2 and")
        print("/// Lens title labels, then Arthur's and the Lens's client areas.")
        print(f"pub const DS1C_LENS_MASK_BOXES_1920X1080: [(usize, usize, usize, usize); {len(boxes)}] = [")
        for b in boxes:
            print("    (%d, %d, %d, %d)," % b)
        print("];")
        for mode in ("glass", "rt"):
            rgb, _ = render(1920, 1080, mode, True, "window", lens=True)
            print(f"pub const DS1C_WINDOW_LENS_{mode.upper()}_1920X1080_FOCUSED: u64 = "
                  f"0x{digest(rgb, dict(L, labels=boxes)):016x};")
        # Home with Arthur held and the Lens alive: the second chip only.
        for mode in ("glass", "rt"):
            rgb, Lh = render(1920, 1080, mode, True, "home", held=True, lens=True)
            hb = Lh["labels"] + [L["chip2_label"]]
            print(f"pub const DS1C_HOME_LENS_{mode.upper()}_1920X1080_HELD: u64 = "
                  f"0x{digest(rgb, dict(Lh, labels=hb)):016x};")
        # v1.5: the selection switched to the Lens — in the window view
        # (Arthur unfocused, the Lens chip lit), and in Home on the Spine.
        for mode in ("glass", "rt"):
            rgb, _ = render(1920, 1080, mode, False, "window", lens=True, lens_sel=True)
            print(f"pub const DS1C_WINDOW_LENS_{mode.upper()}_1920X1080_SELECTED: u64 = "
                  f"0x{digest(rgb, dict(L, labels=boxes)):016x};")
        for mode in ("glass", "rt"):
            rgb, Lh = render(1920, 1080, mode, True, "home", held=True, chip_sel=True,
                             lens=True, lens_sel=True)
            hb = Lh["labels"] + [L["chip2_label"]]
            print(f"pub const DS1C_HOME_LENS_{mode.upper()}_1920X1080_CHIP_SELECTED: u64 = "
                  f"0x{digest(rgb, dict(Lh, labels=hb)):016x};")
        return
    if len(sys.argv) >= 2 and sys.argv[1] == "install-fixture":
        install_fixture()
        return
    if len(sys.argv) >= 2 and sys.argv[1] == "dock-fixture":
        dock_fixture()
        return
    if len(sys.argv) >= 2 and sys.argv[1] == "motion-fixture":
        motion_fixture()
        return
    if len(sys.argv) == 7 and sys.argv[1] == "png":
        from PIL import Image

        W, H = int(sys.argv[2]), int(sys.argv[3])
        rgb, _ = render(W, H, sys.argv[4], sys.argv[5] == "1")
        Image.fromarray(rgb.astype(np.uint8), "RGB").save(sys.argv[6])
        return
    print(__doc__)
    sys.exit(2)


if __name__ == "__main__":
    main()
