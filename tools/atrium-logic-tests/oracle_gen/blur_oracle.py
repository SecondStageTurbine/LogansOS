# SPDX-License-Identifier: MPL-2.0
#!/usr/bin/env python3
"""Independent reference generator for the ADR-077 8x8 r=1 Q16 blur byte oracle.

Implements EXACTLY the integer formula from ADR-077 v2 §1.2-1.3, r=1:
  weights w = [16384, 32768, 16384]  (Q16, sum = 65536)
  per pass, per channel:
    acc = w[0]*left + w[1]*center + w[2]*right         (i32-domain)
    out = clamp((acc + 32768) >> 16, 0, 255)           (round-half-up)
  Pass H: reads src, writes mid, clamp-to-edge in X.
  Pass V: reads mid, writes dst, clamp-to-edge in Y.
  X byte: read-ignored on input, written 0xFF on output.
Memory byte order (XRGB8888 LE): [B, G, R, X].

This is an INDEPENDENT reference (Python) vs the CPU floor (Rust) — the whole
point of the oracle (ADR-028 discipline). The Rust KAT asserts the literal bytes
this prints; the PM independently recomputes them.
"""

W = [16384, 32768, 16384]
N = 8
R_RADIUS = 1


def clampi(v, lo, hi):
    return lo if v < lo else (hi if v > hi else v)


def tap(arr, i, n):
    # clamp-to-edge sample index
    return arr[clampi(i, 0, n - 1)]


def pass1d(row):
    """One 1-D pass over a length-N list of channel values."""
    out = []
    for x in range(len(row)):
        left = tap(row, x - 1, len(row))
        center = row[x]
        right = tap(row, x + 1, len(row))
        acc = W[0] * left + W[1] * center + W[2] * right
        out.append(clampi((acc + 32768) >> 16, 0, 255))
    return out


# ---- Source design (hand-chosen to hit every required case) ----
# Per-channel 8x8 matrices. Values chosen so that, per the amendment, the
# fixture exercises: all four clamp edges + corners, ODD quarter-sums that hit
# round_half_up, UNEQUAL R/G/B (channel-order bugs), a nontrivial X byte, and a
# 255-clamp. R/G/B use DIFFERENT generators so a channel swap is detectable.

def src_R(x, y):
    # horizontal ramp with a high tail -> forces 255-clamp near right edge
    base = [10, 45, 80, 130, 180, 230, 252, 255]
    return base[x]

def src_G(x, y):
    # vertical ramp (independent of x) -> exercises V-pass, distinct from R
    base = [5, 35, 66, 120, 175, 205, 240, 255]
    return base[y]

def src_B(x, y):
    # A pattern deliberately engineered so the H-pass quarter-sum S=L+2C+R hits
    # every residue mod 4 -- crucially S%4==2 (the exact-half round_half_up
    # boundary that distinguishes +32768>>16 from a plain floor). Constant
    # within a row (per-y offset) makes S=4*C for interior pixels EXCEPT where
    # the +/-1 step lands, so we add a per-x sawtooth to force odd sums.
    # base sawtooth over x: 0,1,3,6,10,15,21,28  (triangular numbers -> varied
    # consecutive differences => varied S residues), plus a per-row lift.
    tri = [0, 1, 3, 6, 10, 15, 21, 28][x]
    return (30 + tri + 23 * y) % 256

def src_X(x, y):
    # nontrivial, per-pixel X byte -> must be IGNORED, output 0xFF
    return (0x40 + 3 * x + 5 * y) & 0xFF


def build_channel(fn):
    return [[fn(x, y) for x in range(N)] for y in range(N)]


srcR = build_channel(src_R)
srcG = build_channel(src_G)
srcB = build_channel(src_B)
srcX = build_channel(src_X)

# Force the round-half-up boundary (S = L+2C+R with S % 4 == 2) at documented
# interior pixels in the B channel, row y=3. A plain floor gives out-1 here;
# round_half_up (+32768 >> 16) gives out. These two pixels are the load-bearing
# discriminator between the spec's round-half-up and a naive truncation.
#   x=2: L=50 C=50 R=52 -> S=202 (%4==2) -> round_half_up=51, floor=50
#   x=5: L=60 C=60 R=62 -> S=242 (%4==2) -> round_half_up=61, floor=60
srcB[3] = [40, 50, 50, 52, 60, 60, 62, 70]

# ---- Pass H: horizontal, per row ----
midR = [pass1d(srcR[y]) for y in range(N)]
midG = [pass1d(srcG[y]) for y in range(N)]
midB = [pass1d(srcB[y]) for y in range(N)]

# ---- Pass V: vertical, per column (operate on mid columns) ----
def col(m, x):
    return [m[y][x] for y in range(N)]

dstR = [[0] * N for _ in range(N)]
dstG = [[0] * N for _ in range(N)]
dstB = [[0] * N for _ in range(N)]
for x in range(N):
    cR = pass1d(col(midR, x))
    cG = pass1d(col(midG, x))
    cB = pass1d(col(midB, x))
    for y in range(N):
        dstR[y][x] = cR[y]
        dstG[y][x] = cG[y]
        dstB[y][x] = cB[y]


def to_bytes(R, G, B, Xval=None):
    """Emit memory bytes [B,G,R,X] per pixel, row-major."""
    out = []
    for y in range(N):
        for x in range(N):
            xb = 0xFF if Xval is None else Xval[y][x]
            out += [B[y][x] & 0xFF, G[y][x] & 0xFF, R[y][x] & 0xFF, xb & 0xFF]
    return out


src_bytes = to_bytes(srcR, srcG, srcB, srcX)
mid_bytes = to_bytes(midR, midG, midB, None)  # intermediate: X=0xFF written by H
dst_bytes = to_bytes(dstR, dstG, dstB, None)


def fmt_rows(b, label):
    print(f"// {label} ({len(b)} bytes), memory order [B,G,R,X] per pixel:")
    for y in range(N):
        row = b[y * N * 4:(y + 1) * N * 4]
        hexs = ", ".join(f"0x{v:02X}" for v in row)
        print(f"    {hexs}, // row {y}")
    print()


fmt_rows(src_bytes, "SOURCE")
fmt_rows(mid_bytes, "H-INTERMEDIATE (mid): X written 0xFF")
fmt_rows(dst_bytes, "DEST")

# ---- Coverage report: prove the required cases are present ----
print("=== COVERAGE REPORT ===")

# 1) round-half-up: quarter-sum S with S mod 4 == 2 (the exact .5 case)
half_up_hits = []
for y in range(N):
    for x in range(N):
        for name, m in (("R", srcR), ("G", srcG), ("B", srcB)):
            left = tap(m[y], x - 1, N)
            c = m[y][x]
            right = tap(m[y], x + 1, N)
            S = left + 2 * c + right
            if S % 4 == 2:
                half_up_hits.append((name, x, y, S))
print(f"H-pass round-half-up (S%4==2) hits: {len(half_up_hits)} e.g. {half_up_hits[:5]}")

# 1b) V-pass round-half-up too (on the mid buffer, per column)
v_half = []
for x in range(N):
    for name, m in (("R", midR), ("G", midG), ("B", midB)):
        c = col(m, x)
        for y in range(N):
            up = tap(c, y - 1, N); ctr = c[y]; dn = tap(c, y + 1, N)
            S = up + 2 * ctr + dn
            if S % 4 == 2:
                v_half.append((name, x, y, S))
print(f"V-pass round-half-up (S%4==2) hits: {len(v_half)} e.g. {v_half[:5]}")

# 2) 255-clamp in output
clamp_hits = 0
for y in range(N):
    for x in range(N):
        if dstR[y][x] == 255 or srcR[y][x] == 255:
            clamp_hits += 1
print(f"255-value pixels (dstR or srcR): {clamp_hits}")

# 3) channels differ somewhere
diff = any(srcR[y][x] != srcG[y][x] or srcG[y][x] != srcB[y][x]
           for y in range(N) for x in range(N))
print(f"R/G/B differ across the matrix: {diff}")

# 4) corners (clamp both axes)
print(f"src corners RGB: TL={srcR[0][0],srcG[0][0],srcB[0][0]} "
      f"TR={srcR[0][7],srcG[0][7],srcB[0][7]} "
      f"BL={srcR[7][0],srcG[7][0],srcB[7][0]} "
      f"BR={srcR[7][7],srcG[7][7],srcB[7][7]}")
print(f"dst corners RGB: TL={dstR[0][0],dstG[0][0],dstB[0][0]} "
      f"BR={dstR[7][7],dstG[7][7],dstB[7][7]}")

# ---- Hand-checkable spot examples (documented in the KAT) ----
print("\n=== SPOT CHECKS (for hand annotation) ===")
# Top-left R after H: left=clamp(src[0][-1])=src[0][0]=10, c=10, right=45
# acc = 16384*10 + 32768*10 + 16384*45 ; out=(acc+32768)>>16
def show_h(name, m, x, y):
    left = tap(m[y], x-1, N); c = m[y][x]; right = tap(m[y], x+1, N)
    acc = W[0]*left + W[1]*c + W[2]*right
    out = clampi((acc+32768)>>16, 0, 255)
    S = left+2*c+right
    print(f"H {name}[{y}][{x}]: left={left} c={c} right={right} S={S} "
          f"acc={acc} out=(acc+32768)>>16={out}  (round_half_up((L+2C+R)/4)={ (S+2)//4 })")
show_h("R", srcR, 0, 0)   # TL corner, clamp-left
show_h("R", srcR, 7, 0)   # TR corner, clamp-right, near 255
show_h("B", srcB, 3, 2)   # interior odd
show_h("B", srcB, 0, 5)   # left edge

def show_v(name, m, x, y):
    left = tap(col(m,x), y-1, N); c = m[y][x]; right = tap(col(m,x), y+1, N)
    acc = W[0]*left + W[1]*c + W[2]*right
    out = clampi((acc+32768)>>16, 0, 255)
    S = left+2*c+right
    print(f"V {name}[{y}][{x}]: up={left} c={c} down={right} S={S} out={out}")
show_v("G", midG, 0, 0)   # TL corner clamp-up on mid
show_v("G", midG, 0, 7)   # bottom edge clamp-down

# ---- Emit Rust const arrays for the KAT ----
def emit_rust(name, b):
    print(f"    const {name}: [u8; 256] = [")
    for y in range(N):
        row = b[y*N*4:(y+1)*N*4]
        print("        " + ", ".join(f"0x{v:02X}" for v in row) + ",")
    print("    ];")

print("\n// ====== RUST CONSTS ======")
emit_rust("ORACLE_SRC", src_bytes)
emit_rust("ORACLE_MID", mid_bytes)
emit_rust("ORACLE_DST", dst_bytes)
