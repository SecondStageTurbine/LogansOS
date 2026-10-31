// SPDX-License-Identifier: CC-BY-4.0
// Gen9.5 BUILD-4b-2 activation kernel: pointwise GELU (tanh approximation), the
// transformer MLP nonlinearity. Computes the SAME tanh-approx form as the host
// reference `ai::ops::gelu`:
//   gelu(x) = x * (0.5 * (1 + tanh( sqrt(2/pi) * (x + 0.044715 * x^3) )))
// One work-item per element over a 1-D NDRange of `count` (the same 1-D walker
// shape the Add/matmul proved in 4a/4b-1); the bounds guard covers the partial
// last SIMD32 thread. The OpenCL `tanh()` lowers to the Gen9 hardware
// extended-math unit (a `math` instruction in the disasm — the whole point of
// landing GELU on silicon: GPU hardware-tanh != soft-float != host libm, three
// independent approximations, so the <=2^-12 host-f32 tolerance fires
// non-vacuously). The constants bit-match `ai::ops::gelu` (0x3f4c4229 sqrt(2/pi),
// 0x3d372713 = 0.044715, 0x3f000000 = 0.5, 0x3f800000 = 1.0) and the operation
// association mirrors the host (x*(0.5*(1+tanh(scaled)))). Real finalizer source
// for GELU_F32_KERNEL_GEN9 — compiled stateless-A64 by the pinned ocloc
// (igc-1.0.17193.4 / CR 24.26.30049.6) via
// -cl-intel-greater-than-4GB-buffer-required; see ../README.md.
__kernel void gelu_f32(__global const float* in,
                       __global float* out,
                       const int count) {
    int idx = get_global_id(0);
    if (idx >= count) {
        return;
    }
    float x = in[idx];
    float x3 = x * x * x;
    float scaled = 0.7978845608f * (x + 0.044715f * x3);
    out[idx] = x * (0.5f * (1.0f + tanh(scaled)));
}
