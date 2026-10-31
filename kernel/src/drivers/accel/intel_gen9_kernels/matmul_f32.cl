// SPDX-License-Identifier: CC-BY-4.0
// Gen9.5 BUILD-4b-1 workhorse kernel: row-major f32 matmul C = A x B.
// A is M x K, B is K x N, C is M x N, all row-major f32. One work-item per
// output element over a 1-D NDRange of M*N (the same 1-D walker shape the Add
// proved in 4a); the bounds guard covers the partial last SIMD32 thread.
// Semantics match the host reference `ai::ops::matmul` exactly
// (C[row*n+col] = sum_k A[row*k+k']*B[k'*n+col]). Real finalizer source for
// MATMUL_F32_KERNEL_GEN9 — compiled stateless-A64 by the pinned ocloc
// (igc-1.0.17193.4 / CR 24.26.30049.6) via -cl-intel-greater-than-4GB-buffer-required;
// see ../README.md.
__kernel void matmul_f32(__global const float* a,
                         __global const float* b,
                         __global float* c,
                         const int m,
                         const int n,
                         const int k) {
    int idx = get_global_id(0);
    if (idx >= m * n) {
        return;
    }
    int row = idx / n;
    int col = idx % n;
    float acc = 0.0f;
    for (int inner = 0; inner < k; ++inner) {
        acc += a[row * k + inner] * b[inner * n + col];
    }
    c[idx] = acc;
}
