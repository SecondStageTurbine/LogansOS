// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the file-capability unification mapping (I-15).
//!
//! Mounts the *real* kernel mapping source via `#[path]` and asserts it on the
//! host — the security core of the build, the part the PM scrutinizes hardest:
//! the `FileRights ↔ CapRights` oracle, attenuation (READ-derive of READ|WRITE ⇒
//! read-only), subset enforcement (child ⊆ parent, always), meta-gating (a
//! read-only handle surfaces no DERIVE/GRANT meta), the revoke coupling, and the
//! Rule-1 **kill-switch equality** (disabled ≡ the pre-unification behavior,
//! bit-for-bit) swept over every representable input.
//!
//! The mapping table is hand-written (the I-12 byte-oracle discipline): the
//! `oracle_*` tests assert literal expected `(meta, semantic)` values, never a
//! writer↔reader round-trip.

#![allow(dead_code)]

#[path = "../../../kernel/src/sched/file_cap_logic.rs"]
mod file_cap_logic;

#[cfg(test)]
mod tests {
    use super::file_cap_logic::*;

    // The FileRights bits a real handle can carry: the user-openable semantic
    // bits (subset of KNOWN_BUILD4B) plus the kernel-rooted owner meta bits.
    // (MMAP is included for mapping totality even though `open` can't set it.)
    const REPRESENTABLE: &[u32] = &[
        FR_READ, FR_WRITE, FR_LOOKUP, FR_READDIR, FR_EXECUTE, FR_MMAP, FR_MOUNT, FR_GRANT,
        FR_DERIVE,
    ];

    fn all_representable_subsets() -> impl Iterator<Item = u32> {
        // 9 representable bits → 512 subsets; trivial to sweep exhaustively.
        (0u32..(1 << REPRESENTABLE.len())).map(|mask| {
            let mut bits = 0u32;
            for (i, &bit) in REPRESENTABLE.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    bits |= bit;
                }
            }
            bits
        })
    }

    /// Returns true iff `child ⊆ parent` in the FileRights bit lattice.
    fn is_subset(child: u32, parent: u32) -> bool {
        child & !parent == 0
    }

    // === The hand-written oracle (literal expected values, not round-trip) =====

    #[test]
    fn oracle_semantic_low_bits_are_identity() {
        // READ..MMAP map to the IDENTICAL CapRights.semantic bit — this is what
        // lets the syscall ABI pass `FileRights::READ.bits() as u8` directly.
        assert_eq!(file_rights_to_cap(FR_READ, true), (M_USE, S_READ));
        assert_eq!(file_rights_to_cap(FR_WRITE, true), (M_USE, S_WRITE));
        assert_eq!(file_rights_to_cap(FR_LOOKUP, true), (M_USE, S_LOOKUP));
        assert_eq!(file_rights_to_cap(FR_READDIR, true), (M_USE, S_READDIR));
        assert_eq!(file_rights_to_cap(FR_EXECUTE, true), (M_USE, S_EXECUTE));
        assert_eq!(file_rights_to_cap(FR_MMAP, true), (M_USE, S_MMAP));
        // The low semantic bits are numerically identical between the two sets.
        assert_eq!(S_READ as u32, FR_READ);
        assert_eq!(S_WRITE as u32, FR_WRITE);
        assert_eq!(S_LOOKUP as u32, FR_LOOKUP);
        assert_eq!(S_READDIR as u32, FR_READDIR);
        assert_eq!(S_EXECUTE as u32, FR_EXECUTE);
        assert_eq!(S_MMAP as u32, FR_MMAP);
    }

    #[test]
    fn oracle_mount_relocates_to_bit6() {
        // MOUNT is the one bit that moves (1<<27 → semantic 1<<6), because the
        // semantic byte is only 8 bits wide.
        assert_eq!(file_rights_to_cap(FR_MOUNT, true), (M_USE, S_MOUNT));
        assert_eq!(S_MOUNT, 1 << 6);
    }

    #[test]
    fn oracle_meta_bits() {
        // DERIVE_RESERVED → DERIVE; no semantic ⇒ no USE.
        assert_eq!(file_rights_to_cap(FR_DERIVE, true), (M_DERIVE, 0));
        // GRANT_RESERVED → GRANT *and* REVOKE (grant authority is revoke authority).
        assert_eq!(file_rights_to_cap(FR_GRANT, true), (M_GRANT | M_REVOKE, 0));
    }

    #[test]
    fn oracle_full_owner_handle() {
        // A freshly-opened READ|WRITE file rooted by sys_open: user semantic +
        // GRANT+DERIVE owner meta.
        let rooted = root_file_handle_rights(FR_READ | FR_WRITE, true);
        assert_eq!(rooted, FR_READ | FR_WRITE | FR_GRANT | FR_DERIVE);
        // Its rights() surfaces the full file-owner meta set: USE|DERIVE|GRANT|REVOKE.
        assert_eq!(
            file_rights_to_cap(rooted, true),
            (M_USE | M_DERIVE | M_GRANT | M_REVOKE, S_READ | S_WRITE)
        );
    }

    // === ISC-2: attenuation (the bug Genesis flagged) ==========================

    #[test]
    fn attenuation_read_derive_of_read_write_is_read_only() {
        // Parent READ|WRITE (rooted). Derive requesting only READ semantic.
        let parent = root_file_handle_rights(FR_READ | FR_WRITE, true);
        let child = attenuate_file_rights(parent, M_USE, S_READ, true);
        assert_eq!(child, FR_READ); // read-only: WRITE and the owner meta are gone
        assert_eq!(child & FR_WRITE, 0);
        // The child's rights() carries no DERIVE/GRANT/REVOKE — it is a leaf.
        assert_eq!(file_rights_to_cap(child, true), (M_USE, S_READ));
    }

    #[test]
    fn attenuation_grantable_read_only_keeps_grant_loses_write() {
        // Derive a read-only-but-grantable cap (for the cross-process grant leg).
        let parent = root_file_handle_rights(FR_READ | FR_WRITE, true);
        let child = attenuate_file_rights(parent, M_USE | M_GRANT, S_READ, true);
        assert_eq!(child, FR_READ | FR_GRANT);
        assert_eq!(child & FR_WRITE, 0);
        // Grantable, read-only, revocable (GRANT⇒REVOKE), not further derivable.
        assert_eq!(
            file_rights_to_cap(child, true),
            (M_USE | M_GRANT | M_REVOKE, S_READ)
        );
    }

    // === ISC-4 / I-15: subset enforcement — child ⊆ parent, ALWAYS ============

    #[test]
    fn attenuation_never_exceeds_parent_exhaustive() {
        // For every parent and every requested (meta, semantic), the attenuated
        // child must be a subset of the parent. This is the I-15 core property.
        for parent in all_representable_subsets() {
            for req_meta in 0u8..=0x0f {
                for req_semantic in [0u8, S_READ, S_WRITE, S_READ | S_WRITE, 0xff] {
                    let child = attenuate_file_rights(parent, req_meta, req_semantic, true);
                    assert!(
                        is_subset(child, parent),
                        "child {child:#x} escaped parent {parent:#x} (req m={req_meta:#x} s={req_semantic:#x})"
                    );
                }
            }
        }
    }

    #[test]
    fn attenuation_requesting_write_from_read_only_yields_no_write() {
        // A read-only parent: no decode of a WRITE request can conjure WRITE.
        let parent = FR_READ;
        let child = attenuate_file_rights(parent, M_USE, S_READ | S_WRITE, true);
        assert_eq!(child, FR_READ);
        assert_eq!(child & FR_WRITE, 0);
    }

    // === Meta-gating: a read-only leaf surfaces no DERIVE/GRANT meta ===========

    #[test]
    fn meta_gating_read_only_leaf_has_no_derive_or_grant() {
        // The kernel's derive_handle/grant gate on has_meta(DERIVE)/has_meta(GRANT).
        // A plain READ handle reports neither ⇒ derive/grant EPERM in the kernel.
        let (meta, _semantic) = file_rights_to_cap(FR_READ, true);
        assert_eq!(meta & M_DERIVE, 0);
        assert_eq!(meta & M_GRANT, 0);
        assert_eq!(meta & M_REVOKE, 0);
        assert_eq!(meta, M_USE);
    }

    // === Round-trip: spawn inheritance preserves the parent bit-for-bit ========

    #[test]
    fn spawn_inheritance_round_trip_is_exact() {
        // Spawn passes the parent's full rights() back through with_cap; the result
        // must equal the parent for every representable handle.
        for parent in all_representable_subsets() {
            let (meta, semantic) = file_rights_to_cap(parent, true);
            let inherited = attenuate_file_rights(parent, meta, semantic, true);
            assert_eq!(inherited, parent, "round-trip lost bits for {parent:#x}");
        }
    }

    #[test]
    fn cap_to_file_rights_inverts_stored_bits() {
        // decode∘encode restores exactly the stored bits (USE/REVOKE are derived,
        // so they don't survive — but they reconstruct identically on re-encode).
        for parent in all_representable_subsets() {
            let (meta, semantic) = file_rights_to_cap(parent, true);
            assert_eq!(cap_to_file_rights(meta, semantic), parent);
        }
    }

    // === ISC-6 / Rule-1: kill-switch equality — disabled ≡ pre-unification ====

    #[test]
    fn killswitch_off_rights_are_zero_for_all_inputs() {
        for bits in all_representable_subsets() {
            assert_eq!(
                file_rights_to_cap(bits, false),
                (0, 0),
                "file-caps OFF must yield (0,0) — the pre-unification rights() — for {bits:#x}"
            );
        }
        // Also for arbitrary high-bit garbage.
        assert_eq!(file_rights_to_cap(0xffff_ffff, false), (0, 0));
    }

    #[test]
    fn killswitch_off_with_cap_ignores_requested_rights() {
        for original in all_representable_subsets() {
            for req_meta in [0u8, 0x0f, 0xff] {
                for req_semantic in [0u8, S_READ, 0xff] {
                    assert_eq!(
                        attenuate_file_rights(original, req_meta, req_semantic, false),
                        original,
                        "file-caps OFF must keep the original rights (pre-unification with_cap)"
                    );
                }
            }
        }
    }

    #[test]
    fn killswitch_off_sys_open_adds_no_meta() {
        for user in all_representable_subsets() {
            assert_eq!(
                root_file_handle_rights(user, false),
                user,
                "file-caps OFF: sys_open adds no meta bits (pre-unification rooting)"
            );
        }
    }

    #[test]
    fn killswitch_on_off_differ_only_when_expected() {
        // Sanity: with the switch ON a rooted READ handle gains owner meta; OFF it
        // does not — the equality regression's contrapositive.
        assert_ne!(
            root_file_handle_rights(FR_READ, true),
            root_file_handle_rights(FR_READ, false)
        );
        assert_eq!(root_file_handle_rights(FR_READ, false), FR_READ);
    }
}
