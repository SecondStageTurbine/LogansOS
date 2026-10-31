// SPDX-License-Identifier: CC-BY-4.0
// OT-63 GPU P1 Slice 3: the separable Gaussian backdrop-blur EU kernel
// (ADR-077 v2 §1.3). ONE pass per dispatch; the driver invokes it twice — Pass H
// (axis=0, src->mid, clamp X) then a PIPE_CONTROL DC-flush/CS-stall then Pass V
// (axis=1, mid->dst, clamp Y) — so a single pinned blob serves both passes and
// both the r=1 byte oracle and the r=8 production radius (weights are a pointer
// arg, not baked, so radius is data).
//
// Exact Q16 fixed-point, matching `kernel/src/atrium/blur.rs::blur_pass`
// bit-for-bit — that equality is the kill-switch (ADR-077 v2 §5.1), only provable
// because the arithmetic is integer, never float. Per channel:
//   acc(i32) += w[k] * src_channel;  out = clamp((acc + 32768) >> 16, 0, 255)
// weights sum to exactly 65536 (Q16 one), so the >>16 is an exact round-half-up.
//
// Memory order per XRGB8888 LE pixel is [B, G, R, X]; B/G/R are blurred
// independently, the X byte is read-ignored and written 0xFF (ADR-077 v2 §1.2).
// `stride` is the row pitch in BYTES (>= width*4), exactly as in blur.rs.
//
// Stateless A64 (compiled with -cl-intel-greater-than-4GB-buffer-required):
// __global access lowers to A64 messages — no SURFACE_STATE, no sampler, no SLM,
// no workgroup barrier, no atomics (ADR-077 v2 §4.5: P1's linear-buffer blur
// touches none of the hard Gen9 shapes).
//
// amendment 2 (memory-safety boundary): the explicit x/y bounds guard runs BEFORE
// any address arithmetic — a partial edge thread-group's out-of-range lanes must
// not compute or dereference a row base. Clamp-to-edge protects tap *reads* for a
// valid output pixel; only this guard protects the output *write*.
__kernel void separable_blur(__global const uchar* src,
                             __global uchar* dst,
                             __global const int* weights,
                             uint width,
                             uint height,
                             uint stride,
                             uint radius,
                             uint axis) {
    uint x = get_global_id(0);
    uint y = get_global_id(1);
    if (x >= width || y >= height) {
        return;
    }

    int r = (int)radius;
    uint base = y * stride + x * 4u;

    // Channels B=0, G=1, R=2 (memory order); X handled after the loop.
    for (uint c = 0u; c < 3u; ++c) {
        int acc = 0;
        for (int k = 0; k <= 2 * r; ++k) {
            int off = k - r;
            uint sidx;
            if (axis == 0u) {
                // Pass H: gather along X, clamp-to-edge in X; Y fixed.
                int sx = (int)x + off;
                if (sx < 0) {
                    sx = 0;
                } else if (sx >= (int)width) {
                    sx = (int)width - 1;
                }
                sidx = y * stride + (uint)sx * 4u + c;
            } else {
                // Pass V: gather along Y, clamp-to-edge in Y; X fixed.
                int sy = (int)y + off;
                if (sy < 0) {
                    sy = 0;
                } else if (sy >= (int)height) {
                    sy = (int)height - 1;
                }
                sidx = (uint)sy * stride + x * 4u + c;
            }
            acc += weights[k] * (int)src[sidx];
        }
        // Round-half-up then clamp. acc >= 0 (weights, samples both >= 0), so the
        // >>16 is a logical/arithmetic shift on a non-negative value — identical to
        // the Rust i32 `>>`. Peak acc = 65536*255 = 16_711_680, well inside i32.
        int out = (acc + 32768) >> 16;
        if (out < 0) {
            out = 0;
        } else if (out > 255) {
            out = 255;
        }
        dst[base + c] = (uchar)out;
    }
    dst[base + 3u] = (uchar)0xFF;
}
