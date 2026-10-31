// SPDX-License-Identifier: CC-BY-4.0
// Gen9.5 BUILD-4a proof kernel: elementwise f32[2] add. Real finalizer source
// for ADD_F32X2_KERNEL_GEN9. The vISA form in add_f32x2.visa documents intent;
// the embedded blob is this OpenCL-C source compiled by the pinned ocloc,
// because no standalone vISA finalizer ships in prebuilt IGC (see ../README.md).
__kernel void add_f32x2(__global const float* a,
                        __global const float* b,
                        __global float* o) {
    int i = get_global_id(0);
    o[i] = a[i] + b[i];
}
