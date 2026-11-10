// SPDX-License-Identifier: MPL-2.0
//! Host regression for the kernel-heap budget kill-switch.
//!
//! Mounts the *real* `kernel/src/mm/heap_policy.rs` via `#[path]` (no shims, the
//! `frame-guard-logic-tests` pattern) and proves on the host that with the 3B
//! and 1.5B real-brain-boot flags OFF the heap budget is bit-for-bit the
//! previous layer's value — 944 MiB for the 0.5B boot (768 MiB before Arthur
//! moved to Q8_0), 4 MiB otherwise — and
//! that each larger flag selects its larger budget (1.5B → 1.75 GiB, 3B → 2.75
//! GiB) and the largest set flag wins.

#![allow(dead_code)]

#[path = "../../../kernel/src/mm/heap_policy.rs"]
mod heap_policy;

#[cfg(test)]
mod tests {
    use super::heap_policy::{
        HEAP_SIZE_DEFAULT, HEAP_SIZE_REAL_BRAIN_3B, HEAP_SIZE_REAL_BRAIN_05B,
        HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE, HEAP_SIZE_REAL_BRAIN_15B,
        HEAP_SIZE_REAL_BRAIN_QWEN35, heap_size_bytes, heap_size_bytes_for_source,
    };

    /// Kill-switch (Rule 1): with the 1.5B budget OFF (and 3B OFF), the heap size
    /// equals the previous layer's value bit-for-bit — 944 MiB under the 0.5B
    /// real-brain boot, 4 MiB otherwise. This is the disabled ≡ previous-layer
    /// regression for the 1.5B tier.
    #[test]
    fn kill_switch_15b_off_is_previous_budget_bit_for_bit() {
        assert_eq!(
            heap_size_bytes(false, false, false, false),
            HEAP_SIZE_DEFAULT
        );
        assert_eq!(heap_size_bytes(false, false, false, false), 4 * 1024 * 1024);
        assert_eq!(
            heap_size_bytes(true, false, false, false),
            HEAP_SIZE_REAL_BRAIN_05B
        );
        assert_eq!(
            heap_size_bytes(true, false, false, false),
            944 * 1024 * 1024
        );
    }

    /// The 1.5B flag selects the 1.75 GiB budget and dominates the 0.5B flag
    /// when both are set (1.5B implies 0.5B via Cargo). With 3B OFF the 1.5B
    /// selection is unchanged from before the 3B tier existed.
    #[test]
    fn fifteen_b_budget_selected_and_dominant() {
        assert_eq!(
            heap_size_bytes(false, true, false, false),
            HEAP_SIZE_REAL_BRAIN_15B
        );
        assert_eq!(
            heap_size_bytes(true, true, false, false),
            HEAP_SIZE_REAL_BRAIN_15B
        );
        assert_eq!(
            heap_size_bytes(true, true, false, false),
            1792 * 1024 * 1024
        );
    }

    /// Kill-switch (Rule 1) for the 3B tier: with the 3B flag OFF the budget is
    /// bit-for-bit the previous layer's — 4 MiB / 944 MiB / 1.75 GiB depending
    /// on the lower flags — i.e. adding the 3B tier changed nothing when 3B is
    /// disabled. `heap_size_bytes(_, _, false)` never returns the 3B budget.
    #[test]
    fn kill_switch_3b_off_is_previous_budget_bit_for_bit() {
        assert_eq!(
            heap_size_bytes(false, false, false, false),
            HEAP_SIZE_DEFAULT
        );
        assert_eq!(
            heap_size_bytes(true, false, false, false),
            HEAP_SIZE_REAL_BRAIN_05B
        );
        assert_eq!(
            heap_size_bytes(false, true, false, false),
            HEAP_SIZE_REAL_BRAIN_15B
        );
        assert_eq!(
            heap_size_bytes(true, true, false, false),
            HEAP_SIZE_REAL_BRAIN_15B
        );
        assert_ne!(
            heap_size_bytes(true, true, false, false),
            HEAP_SIZE_REAL_BRAIN_3B
        );
    }

    /// The 3B flag selects the 2.75 GiB budget and dominates every lower flag.
    /// 3B is a sibling of the 1.5B flag (a 3B build has 1.5B OFF), but the
    /// selection must still win if both were somehow set — the larger tier never
    /// silently loses to a smaller one.
    #[test]
    fn three_b_budget_selected_and_dominant() {
        assert_eq!(
            heap_size_bytes(false, false, true, false),
            HEAP_SIZE_REAL_BRAIN_3B
        );
        assert_eq!(
            heap_size_bytes(true, false, true, false),
            HEAP_SIZE_REAL_BRAIN_3B
        );
        assert_eq!(
            heap_size_bytes(true, true, true, true),
            HEAP_SIZE_REAL_BRAIN_3B
        );
        assert_eq!(
            heap_size_bytes(true, false, true, false),
            2816 * 1024 * 1024
        );
    }

    #[test]
    fn qwen35_budget_covers_model_recurrence_vocab_and_logits() {
        const QWEN35_NEOM_BYTES: usize = 725_171_456;
        const GDN_RECURRENT_BYTES: usize = 18 * 16 * 128 * 128 * 4;
        const VOCAB_LOGITS_BYTES: usize = 248_320 * 4;
        assert_eq!(
            heap_size_bytes(true, false, false, true),
            HEAP_SIZE_REAL_BRAIN_QWEN35
        );
        const {
            assert!(
                HEAP_SIZE_REAL_BRAIN_QWEN35
                    > QWEN35_NEOM_BYTES + GDN_RECURRENT_BYTES + VOCAB_LOGITS_BYTES
            )
        };
        assert_eq!(HEAP_SIZE_REAL_BRAIN_QWEN35, 1024 * 1024 * 1024);
    }

    /// The 1.5B budget must exceed the measured ~1.035 GiB 1.5B `.neom` with
    /// real headroom for the KV cache + per-forward scratch.
    #[test]
    fn fifteen_b_budget_exceeds_measured_neom_with_headroom() {
        const MEASURED_15B_NEOM_BYTES: usize = 1_111_411_200; // q4_K_M import
        const { assert!(HEAP_SIZE_REAL_BRAIN_15B > MEASURED_15B_NEOM_BYTES) };
        const { assert!(HEAP_SIZE_REAL_BRAIN_15B - MEASURED_15B_NEOM_BYTES >= 512 * 1024 * 1024) };
    }

    /// The 3B budget must exceed the measured ~1.955 GiB 3B `.neom` (import:
    /// 2,099,030,016 B) with the same ≥512 MiB headroom floor for the KV cache
    /// + per-forward scratch. Measured headroom is ~814 MiB.
    #[test]
    fn three_b_budget_exceeds_measured_neom_with_headroom() {
        const MEASURED_3B_NEOM_BYTES: usize = 2_099_030_016; // q4_K_M import
        const { assert!(HEAP_SIZE_REAL_BRAIN_3B > MEASURED_3B_NEOM_BYTES) };
        const { assert!(HEAP_SIZE_REAL_BRAIN_3B - MEASURED_3B_NEOM_BYTES >= 512 * 1024 * 1024) };
    }

    /// B1b kill-switch: with `model_from_boot_module` off, every flag
    /// combination gives exactly `heap_size_bytes`.
    #[test]
    fn boot_module_off_is_the_previous_policy_for_every_combination() {
        for bits in 0u8..16 {
            let f = |i: u8| bits & (1 << i) != 0;
            assert_eq!(
                heap_size_bytes_for_source(false, f(0), f(1), f(2), f(3)),
                heap_size_bytes(f(0), f(1), f(2), f(3)),
                "flags {bits:04b}"
            );
        }
    }

    /// B1b: a boot-module source shrinks only the 0.5B tier, by the staging
    /// buffer it no longer needs, and keeps the 0.5B's old non-staging headroom.
    /// Q8_0 (2026-11-08): the staging buffer grew, the headroom did not.
    #[test]
    fn boot_module_drops_only_the_05b_staging_buffer() {
        const REAL_05B_NEOM_BYTES: usize = 669_799_936;
        assert_eq!(
            heap_size_bytes_for_source(true, true, false, false, false),
            HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE
        );
        assert_eq!(HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE, 305 * 1024 * 1024);
        const {
            assert!(
                HEAP_SIZE_REAL_BRAIN_05B - HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE
                    >= REAL_05B_NEOM_BYTES
            )
        };
        const {
            assert!(
                HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE
                    >= HEAP_SIZE_REAL_BRAIN_05B - REAL_05B_NEOM_BYTES - 1024 * 1024
            )
        };
        // Larger tiers and the default are untouched.
        for (flags, expect) in [
            ((false, false, false, false), HEAP_SIZE_DEFAULT),
            ((true, true, false, false), HEAP_SIZE_REAL_BRAIN_15B),
            ((true, false, true, false), HEAP_SIZE_REAL_BRAIN_3B),
            ((true, false, false, true), HEAP_SIZE_REAL_BRAIN_QWEN35),
        ] {
            let (a, b, c, d) = flags;
            assert_eq!(heap_size_bytes_for_source(true, a, b, c, d), expect);
        }
    }

    /// Q8_0 (`Plans/DISPATCH-2026-11-08-arthur-q8.md` step 3): the 0.5B tier is
    /// the pinned `.neom` rounded up to whole MiB plus the same 305 MiB it kept
    /// under Q4_K_M, and it stays below the Qwen3.5-0.8B tier it must lose to.
    #[test]
    fn q8_05b_tier_is_the_module_plus_the_old_headroom() {
        use super::heap_policy::{REAL_BRAIN_05B_HEADROOM_MIB, REAL_BRAIN_05B_STAGING_MIB};
        const MIB: usize = 1024 * 1024;
        const REAL_05B_NEOM_BYTES: usize = 669_799_936;
        assert_eq!(
            REAL_BRAIN_05B_STAGING_MIB,
            REAL_05B_NEOM_BYTES.div_ceil(MIB)
        );
        assert_eq!(REAL_BRAIN_05B_STAGING_MIB, 639);
        // The headroom the 0.5B tier had under Q4_K_M: 768 - ceil(485,489,152 B).
        assert_eq!(
            REAL_BRAIN_05B_HEADROOM_MIB,
            768 - 485_489_152usize.div_ceil(MIB)
        );
        assert_eq!(HEAP_SIZE_REAL_BRAIN_05B, (639 + 305) * MIB);
        assert_eq!(HEAP_SIZE_REAL_BRAIN_05B_BOOT_MODULE, 305 * MIB);
        const { assert!(HEAP_SIZE_REAL_BRAIN_05B < HEAP_SIZE_REAL_BRAIN_QWEN35) };
        assert_eq!(
            heap_size_bytes(true, false, false, true),
            HEAP_SIZE_REAL_BRAIN_QWEN35
        );
    }
}
