// SPDX-License-Identifier: MPL-2.0
//! Pure kernel-heap budget policy.
//!
//! Factored out of `heap.rs` (which pulls in the allocator, paging, and APIC
//! deps) so the kill-switch equality — *with the 1.5B path OFF the heap budget
//! is bit-for-bit the previous layer's* — is host-testable with zero shims via
//! `tools/heap-policy-logic-tests`, mirroring `frame-guard-logic-tests`.
//!
//! Budgets (selected by the real-brain scenario features):
//! - default: 4 MiB.
//! - 0.5B Stage-B real-brain-boot: 944 MiB (the 669,799,936-byte Q8_0 `.neom`
//!   staging buffer, 639 MiB rounded up, plus the 305 MiB the tier has always
//!   kept beside it).
//! - Qwen3.5-0.8B: 1 GiB (725,171,456-byte `.neom`, 18 recurrent GDN
//!   states, six attention KV caches, 248,320 logits, and forward scratch).
//! - 1.5B Stage-B real-brain-boot: 1.75 GiB (the ~1.04 GiB `.neom`, loaded once
//!   and kept quantized — the F1 forward streams weights block-by-block so the
//!   peak transient is ~O(vocab) f32 logits, not a full-matrix dequant — plus
//!   the KV cache, activation/FFN scratch, and the rest of the kernel heap).
//! - 3B Stage-B real-brain-boot: 2.75 GiB (the ~1.955 GiB `.neom` — measured
//!   2,099,030,016 B — resident-quantized like the 1.5B, plus the larger KV
//!   cache (ctx 4096 × 36 layers × 2 kv-heads × 128 head-dim, f32 ≈ 288 MiB),
//!   the same ~O(vocab) forward scratch, and the rest of the kernel heap; the
//!   ~814 MiB headroom over the weights exceeds the 512 MiB floor).

/// Default kernel heap: 4 MiB.
pub const HEAP_SIZE_DEFAULT: usize = 4 * 1024 * 1024;
/// The pinned 0.5B `.neom`, rounded up to whole MiB: 669,799,936 B (Q8_0,
/// `boot_model_logic::QWEN25_05B`) is 638.77 MiB, so 639 MiB. It
/// was 463 MiB for the Q4_K_M's 485,489,152 B.
pub const REAL_BRAIN_05B_STAGING_MIB: usize = 639;
/// What the 0.5B tier keeps beside the staging buffer: 305 MiB, the headroom
/// the 0.5B load has had since B1 (768 MiB - 463 MiB under Q4_K_M).
pub const REAL_BRAIN_05B_HEADROOM_MIB: usize = 305;
/// 0.5B Stage-B real-brain-boot: the staging buffer plus the headroom, 639 +
/// 305 = 944 MiB (was 463 + 305 = 768 MiB under Q4_K_M). Still below the
/// Qwen3.5-0.8B tier, so the tier order is unchanged.
pub const HEAP_SIZE_REAL_BRAIN_05B: usize =
    (REAL_BRAIN_05B_STAGING_MIB + REAL_BRAIN_05B_HEADROOM_MIB) * 1024 * 1024;
/// B1b: the 0.5B tier when the model arrives as a boot module. There is no
/// staging `Vec` (the model is built straight from the module's bytes), so
/// this is the tier minus that buffer: 944 - 639 = 305 MiB, the same headroom
/// as under Q4_K_M (768 - 463). The forward's kernel heap use doesn't grow
/// with the quantization: the Q8_0 weights, like the Q4_K_M ones, are never
/// dequantized to f32 buffers (they live in their own buddy blocks, not the
/// heap); the KV cache is ~24 KiB per token, ~13 MiB at Arthur's 512+32
/// tokens; logits ~0.6 MiB. The census's heap high-water on a real-model run
/// is the measurement a later tightening (B2) starts from.
pub const HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE: usize =
    HEAP_SIZE_REAL_BRAIN_05B - REAL_BRAIN_05B_STAGING_MIB * 1024 * 1024;
/// Qwen3.5-0.8B hybrid: the measured 725,171,456-byte model plus recurrent
/// state, KV cache, 248,320-vocabulary logits, and forward scratch.
pub const HEAP_SIZE_REAL_BRAIN_QWEN35: usize = 1024 * 1024 * 1024;
/// 1.5B Stage-B real-brain-boot: 1.75 GiB.
pub const HEAP_SIZE_REAL_BRAIN_15B: usize = 1792 * 1024 * 1024;
/// 3B Stage-B real-brain-boot: 2.75 GiB (holds the measured ~1.955 GiB 3B
/// `.neom` Vec with ~814 MiB headroom for the KV cache + forward scratch + the
/// rest of the kernel heap).
pub const HEAP_SIZE_REAL_BRAIN_3B: usize = 2816 * 1024 * 1024;

/// Select the heap budget from the real-brain-boot feature flags. A bigger tier
/// dominates when several flags are set, so a build never silently inherits a
/// smaller budget: 3B wins over 1.5B, which wins over Qwen3.5-0.8B, which wins
/// over the 0.5B base tier.
pub const fn heap_size_bytes(
    real_brain_boot: bool,
    real_brain_boot_15b: bool,
    real_brain_boot_3b: bool,
    real_brain_boot_qwen35: bool,
) -> usize {
    if real_brain_boot_3b {
        HEAP_SIZE_REAL_BRAIN_3B
    } else if real_brain_boot_15b {
        HEAP_SIZE_REAL_BRAIN_15B
    } else if real_brain_boot_qwen35 {
        HEAP_SIZE_REAL_BRAIN_QWEN35
    } else if real_brain_boot {
        HEAP_SIZE_REAL_BRAIN_05B
    } else {
        HEAP_SIZE_DEFAULT
    }
}

/// B1b: the heap budget when the 0.5B may come from a boot module. Only the
/// 0.5B tier changes; every larger tier, and every build with
/// `model_from_boot_module` off, is exactly [`heap_size_bytes`] (the
/// kill-switch).
pub const fn heap_size_bytes_for_source(
    model_from_boot_module: bool,
    real_brain_boot: bool,
    real_brain_boot_15b: bool,
    real_brain_boot_3b: bool,
    real_brain_boot_qwen35: bool,
) -> usize {
    let tier = heap_size_bytes(
        real_brain_boot,
        real_brain_boot_15b,
        real_brain_boot_3b,
        real_brain_boot_qwen35,
    );
    if model_from_boot_module && tier == HEAP_SIZE_REAL_BRAIN_05B {
        HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE
    } else {
        tier
    }
}
