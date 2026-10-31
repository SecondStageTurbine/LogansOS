# SPDX-License-Identifier: MPL-2.0
#!/usr/bin/env python3
"""Generate the pinned r=8, sigma=4.0 Q16 Gaussian weights (17 taps), summing to
exactly 65536. Center tap absorbs the rounding residual so the sum is exact.
This is the offline generator whose output is pinned as WEIGHTS_R8_Q16 in
kernel/src/atrium/blur.rs (ADR-077 v2 §1.2 provenance discipline)."""
import math

R = 8
SIGMA = R / 2.0  # 4.0
Q = 65536

# Continuous Gaussian samples at integer offsets -R..R
raw = [math.exp(-(k * k) / (2 * SIGMA * SIGMA)) for k in range(-R, R + 1)]
s = sum(raw)
# Normalize to Q16 and round each tap
w = [round(v / s * Q) for v in raw]
# Fix the sum to exactly Q by adjusting the center tap (index R)
residual = Q - sum(w)
w[R] += residual

assert sum(w) == Q, sum(w)
# symmetric?
sym = all(w[k] == w[2 * R - k] for k in range(R))
assert sym, "not symmetric"
assert all(x > 0 for x in w), w

print("// WEIGHTS_R8_Q16: sigma=4.0, 17 taps, sum == 65536 (center absorbs residual)")
print("pub const WEIGHTS_R8_Q16: [i32; 17] = [")
for i in range(0, 17):
    print(f"    {w[i]},  // k={i-R:+d}")
print("];")
print(f"// sum = {sum(w)}, center(k=0) = {w[R]}, residual added to center = {residual}")
print(f"// raw weights: {w}")
