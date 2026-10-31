// SPDX-License-Identifier: MPL-2.0
//! The Gen9 iGPU device-ID allowlist for the Intel accel probe — pure logic, no
//! kernel dependencies, so it is host-testable (`tools/ai-ops-tests`).
//!
//! **Why this is its own module.** QEMU has no Intel iGPU, so the 4-shard QEMU
//! gate structurally *cannot* exercise an allowlist: every scenario reports
//! `candidates=0` whatever the table says. That gap escaped to bare metal — the
//! M710q's first Slice-4 run burned a whole boot cycle on `candidates=0` because
//! `0x1912` was missing — so the check is promoted here, where a host KAT can
//! hold it. The table is data and the lookups are total functions; nothing in
//! this file touches MMIO, so the host crate can include it directly.

/// Threads per EU on Gen9. Intel compute-runtime computes
/// `gtSysInfo->ThreadCount = gtSysInfo->EUCount * 7u` identically in
/// `shared/source/gen9/hw_info_skl.cpp` and `hw_info_cfl.cpp`.
pub const GEN9_THREADS_PER_EU: u32 = 7;

/// The Gen9 iGPUs this driver binds: `(PCI device id, SKU label, EU count)`.
///
/// Every entry is a **`1x3x8` GT2 part** — 1 slice x 3 subslices x 8 EU = 24 EU —
/// per Intel compute-runtime `shared/source/dll/devices/devices_base.inl` at the
/// pinned finalizer version `24.26.30049.6`, the same table the toolchain that
/// compiled `BLUR_KERNEL_GEN9` consults (`0x1912 -> SklHw1x3x8`,
/// `0x9BC8`/`0x3E92 -> CflHw1x3x8`). Family labels are from Linux
/// `include/drm/i915_pciids.h`.
///
/// **The EU count is load-bearing, not documentation.** `intel_gen9_4a`'s
/// `MEDIA_VFE_STATE` DW3 bakes one Maximum-Number-of-Threads value for all
/// devices; [`vfe_max_threads_field`] derives it from this column and a host KAT
/// asserts the two agree for **every** entry. Adding a non-GT2 Gen9 part (GT1
/// `1x2x6`, GT3 `2x3x8`, GT4) therefore **fails the test** instead of silently
/// mis-programming the VFE on that SKU.
pub const SUPPORTED_GEN9_IGPUS: [(u16, &str, u32); 5] = [
    // Skylake GT2 desktop — Lenovo M710q. Gen9; also the `-device skl` target the
    // embedded EU kernels were actually compiled for.
    (0x1912, "HD530-SKL-GT2", 24),
    // Comet Lake GT2 — Dell OptiPlex 7080, the Phase-6/7 bring-up silicon. Gen9.5.
    (0x9bc8, "UHD630-CML-GT2", 24),
    (0x9bc5, "UHD630-CML-GT2", 24),
    // Coffee Lake-S GT2. Gen9.5.
    (0x3e92, "UHD630-CFL-GT2", 24),
    (0x3e98, "UHD630-CFL-GT2", 24),
];

/// `MEDIA_VFE_STATE` DW3's Maximum Number of Threads is a **bias-1** field: a
/// device that supports N threads programs `N - 1`.
pub const fn vfe_max_threads_field(eu_count: u32) -> u32 {
    eu_count * GEN9_THREADS_PER_EU - 1
}

/// The SKU label for an accepted device ID, or `None` if this driver does not
/// bind it. Naming the SKU in the bare-metal log is what tells an HD 530 run from
/// a UHD 630 run without cross-referencing the raw device ID afterwards.
pub fn gen9_sku_name(device_id: u16) -> Option<&'static str> {
    let mut i = 0;
    while i < SUPPORTED_GEN9_IGPUS.len() {
        if SUPPORTED_GEN9_IGPUS[i].0 == device_id {
            return Some(SUPPORTED_GEN9_IGPUS[i].1);
        }
        i += 1;
    }
    None
}

/// The EU count of an accepted device ID.
pub fn gen9_eu_count(device_id: u16) -> Option<u32> {
    let mut i = 0;
    while i < SUPPORTED_GEN9_IGPUS.len() {
        if SUPPORTED_GEN9_IGPUS[i].0 == device_id {
            return Some(SUPPORTED_GEN9_IGPUS[i].2);
        }
        i += 1;
    }
    None
}

/// Whether the Intel accel driver binds this PCI device ID. The caller must check
/// the vendor ID and display class separately.
pub fn is_supported_gen9_igpu_id(device_id: u16) -> bool {
    gen9_sku_name(device_id).is_some()
}
