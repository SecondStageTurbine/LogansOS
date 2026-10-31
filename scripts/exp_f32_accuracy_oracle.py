#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""
Reference oracle: pure-f32 exp() meets the ADR-029 D10 accuracy gate (rel-eps <= 2^-12).

Context
-------
Phase 5 BUILD-4 (bounce) reported ~0.22% (2.2e-3) relative error on soft-float
`exp`, ~9x over the rel-eps <= 2^-12 (~2.44e-4) gate, and floated the possibility
that 2^-12 is infeasible for exp in pure soft-float f32. This script is the
independent check the bounce process requires before any gate is relaxed.

It emulates IEEE-754 single precision by rounding every intermediate to the
nearest f32 (via struct pack/unpack of a double -> the same round-to-nearest a
correct soft-float `mul`/`add` must produce), and evaluates the SAME algorithm
class BUILD-4 chose: range reduction x = k*ln2 + r, then a degree-6 Taylor
polynomial for exp(r), then exp(x) = 2^k * exp(r) (the 2^k is an exact
exponent-field shift).

Result (run it):
    naive reduction    max rel-err ~ 6.6e-7   PASS  (370x inside the gate)
    Cody-Waite 2-part  max rel-err ~ 2.3e-7   PASS
    polynomial-only    max rel-err ~ 1e-7     PASS  (isolates reduction vs poly)

Conclusion: 2^-12 is NOT a soft-float f32 precision floor for exp -- even *naive*
reduction with plain Horner clears it by ~370x, and exp(-5) comes out exact to
shown digits (not 0.22%). The decomposition exp(x)=2^k*exp(r) makes 2^k exact, so
the final relative error EQUALS the relative error of exp(r) with r in
[-ln2/2, ln2/2] where exp(r) in [0.71, 1.41] -- there is no relative-error
amplification for small outputs. A 0.22% result is therefore an implementation
bug (under-rounded soft-float mul/add, or a broken even/odd decomposition, or a
broken integer-scaled reduction), not a wall.

How BUILD-4 should use it
-------------------------
1. Switch the even/odd decomposition for the plain Horner here -- proven to pass.
2. Run the `polynomial-only` sweep below in-kernel: exp(r) for r in [-ln2/2,ln2/2]
   with NO reduction. If that passes, 100% of the error is in your reduction;
   drop the integer-scaled path and use simple `r = x - k*ln2` (passes) or the
   Cody-Waite split here (ln2_hi = 0.693359375 is exact in f32). The belief that
   "two-part ln2 doesn't work in f32" is false: it works because ln2_hi is chosen
   with trailing zero mantissa bits, so k*ln2_hi is exact for the k range.
3. If Horner still fails in-kernel, your soft-float mul/add are under-rounded
   (a correct f32 mul/add is ~0.5 ULP ~ 6e-8, not 1e-3) -- fix the primitives;
   that also puts the "passing" sqrt/div in doubt.

No third-party deps (struct + math only), so it runs in any CI/container.
"""

import struct
import math


def f32(x: float) -> float:
    """Round a Python double to the nearest IEEE-754 single (round-to-nearest)."""
    return struct.unpack("f", struct.pack("f", x))[0]


def mul(a: float, b: float) -> float:
    return f32(a * b)


def add(a: float, b: float) -> float:
    return f32(a + b)


def sub(a: float, b: float) -> float:
    return f32(a - b)


# Constants, all materialized in f32.
LN2 = f32(0.6931471805599453)
LN2HI = f32(0.693359375)        # 355/512: exact in f32 (trailing zero mantissa bits)
LN2LO = f32(-2.12194440e-4)     # ln2 - LN2HI, the Cody-Waite remainder
INVLN2 = f32(1.4426950408889634)

# Degree-6 Taylor coefficients for exp(r): 1, 1, 1/2, 1/6, 1/24, 1/120, 1/720.
COEFFS = [f32(1.0), f32(1.0), f32(0.5), f32(1 / 6), f32(1 / 24), f32(1 / 120), f32(1 / 720)]


def poly_exp_r(r: float) -> float:
    """exp(r) via plain degree-6 Horner, every op rounded to f32."""
    acc = COEFFS[6]
    for c in reversed(COEFFS[:6]):
        acc = add(mul(acc, r), c)
    return acc


def _k_of(x: float) -> int:
    return int(round(x * INVLN2))


def exp_naive(x: float) -> float:
    """Naive single-word range reduction r = x - k*ln2."""
    k = _k_of(x)
    r = sub(x, mul(f32(k), LN2))
    return f32(math.ldexp(poly_exp_r(r), k))


def exp_codywaite(x: float) -> float:
    """Cody-Waite two-word reduction r = (x - k*ln2_hi) - k*ln2_lo."""
    k = _k_of(x)
    r = sub(sub(x, mul(f32(k), LN2HI)), mul(f32(k), LN2LO))
    return f32(math.ldexp(poly_exp_r(r), k))


def _max_rel(fn, xs):
    worst, at = 0.0, 0.0
    for x in xs:
        ref = math.exp(x)
        rel = abs(fn(x) - ref) / abs(ref)
        if rel > worst:
            worst, at = rel, x
    return worst, at


def main() -> int:
    gate = 2.0 ** -12
    xs = [-10.0 + 20.0 * i / 4000 for i in range(4001)]          # exp sweep
    rs = [(-math.log(2) / 2) + math.log(2) * i / 2000 for i in range(2001)]  # reduced range

    rows = [
        ("naive reduction", lambda: _max_rel(exp_naive, xs)),
        ("Cody-Waite 2-part", lambda: _max_rel(exp_codywaite, xs)),
        ("polynomial-only (no reduction)", lambda: _max_rel(poly_exp_r, rs)),
    ]
    print(f"gate: rel-eps <= 2^-12 = {gate:.3e}\n")
    ok = True
    for name, run in rows:
        m, at = run()
        passed = m <= gate
        ok &= passed
        print(f"  {name:32s} max rel-err = {m:.3e} @x={at:+.4f}   {'PASS' if passed else 'FAIL'}")

    # BUILD-4's reported worst case.
    print(f"\n  exp(-5): ref={math.exp(-5):.8e}  naive={exp_naive(-5.0):.8e}  cw={exp_codywaite(-5.0):.8e}")
    print("  (BUILD-4 reported ~0.22% = 2.2e-3 at this point; correct f32 is exact to shown digits)")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
