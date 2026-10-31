// SPDX-License-Identifier: MPL-2.0
//! File-capability unification — the pure `FileRights ↔ CapRights` mapping (I-15).
//!
//! Phase 7.5 OT (file-cap unification). Until this build, `Handle::File` was a
//! parallel rights system (`FileRights`) that never entered the `CapRights`
//! derivation/grant graph: `Handle::File::rights()` returned `CapRights(0,0)`, so
//! `derive_handle`/`ipc::grant`/spawn-inheritance EPERM'd on every file handle and
//! `with_cap` ignored the requested rights. This module is the keystone: a pure,
//! kernel-free, host-testable mapping that lets a file handle be **derived
//! (attenuated), granted, and inherited** exactly like a channel/process handle,
//! with `FileRights` subset-checking — while keeping `FileRights` authoritative for
//! the semantic file ops (read/write/readdir are unchanged).
//!
//! ## The mapping (the hand-written oracle — assert it, do not round-trip)
//!
//! `FileRights` is a 32-bit set; `CapRights` is `{ meta: u8, semantic: u8 }`. The
//! mapping below is the I-12-style byte oracle for this layout: it is hand-laid
//! here and asserted by the host KATs (`tools/file-cap-logic-tests`) against
//! literal expected values, never validated by a writer↔reader round-trip.
//!
//! ### Semantic (file op → `CapRights.semantic` bit) — **identity on bits 0..=5**
//! | `FileRights`            | bit  | `CapRights.semantic` bit |
//! |-------------------------|------|--------------------------|
//! | `READ`                  | 1<<0 | `S_READ`    = 1<<0        |
//! | `WRITE`                 | 1<<1 | `S_WRITE`   = 1<<1        |
//! | `LOOKUP`                | 1<<2 | `S_LOOKUP`  = 1<<2        |
//! | `READDIR`               | 1<<3 | `S_READDIR` = 1<<3        |
//! | `EXECUTE`               | 1<<4 | `S_EXECUTE` = 1<<4        |
//! | `MMAP_RESERVED`         | 1<<5 | `S_MMAP`    = 1<<5        |
//! | `MOUNT`                 | 1<<27| `S_MOUNT`   = 1<<6        |
//!
//! The bits a real handle can carry (`READ..EXECUTE`, the low ones used by every
//! file op) map to the **identical** `CapRights.semantic` bit, so the userland
//! derive/grant ABI passes `FileRights::READ.bits() as u8` directly as the
//! semantic byte — no translation table in the syscall caller. Only `MOUNT`
//! (bit 27) is relocated, to bit 6, because the semantic byte is 8 bits wide.
//!
//! ### Meta (file meta-right → `CapRights.meta` bit)
//! | `FileRights`     | bit   | `CapRights.meta`              |
//! |------------------|-------|-------------------------------|
//! | `GRANT_RESERVED` | 1<<24 | `M_GRANT` (1<<2) **and** `M_REVOKE` (1<<3) |
//! | `DERIVE_RESERVED`| 1<<25 | `M_DERIVE` (1<<1)             |
//! | *(any semantic)* | —     | `M_USE` (1<<0) when the handle has any file op |
//!
//! `M_USE` and `M_REVOKE` are **derived, not stored**: a file handle that can be
//! used at all carries `USE`, and the authority to *grant* a cap is the authority
//! to *revoke* it (the grantor is the revoker), so `GRANT_RESERVED ⇒ GRANT|REVOKE`.
//! Decoding (`cap_to_file_rights`) therefore only reconstructs the two *stored*
//! meta bits (`GRANT_RESERVED`/`DERIVE_RESERVED`); `USE`/`REVOKE` are re-derived on
//! the next `rights()` call. This keeps the encode→decode round-trip exact on every
//! bit a real handle can hold (so spawn-inheritance — which passes the parent's full
//! `rights()` back through `with_cap` — preserves the parent's rights bit-for-bit).
//!
//! ## The kill-switch (Rule 1)
//!
//! `FILE_CAPS_ENABLED` is `cfg!(feature = "file-caps")`, default **off**. With it
//! off, [`file_rights_to_cap`] returns `(0, 0)`, [`attenuate_file_rights`] ignores
//! the requested rights (returns the original), and [`root_file_handle_rights`]
//! adds no meta bits — i.e. **`Handle::File` behaves exactly as it did before this
//! build, bit-for-bit**. The host KATs prove this equality for all inputs.
//!
//! This module imports nothing from the kernel so the host KAT crate can mount it
//! via `#[path]`; the bit values are mirrored as local consts and tied back to the
//! authoritative `FileRights`/`meta_rights` definitions by `const _` assertions in
//! `sched::process` (a drift makes the kernel fail to compile).

// === FileRights bit values (mirror of `crate::fs::FileRights`) ===============
// Tied to the authoritative definitions by `const _` static asserts in
// `sched::process`. Keep in sync; a divergence is a compile error there.
pub const FR_READ: u32 = 1 << 0;
pub const FR_WRITE: u32 = 1 << 1;
pub const FR_LOOKUP: u32 = 1 << 2;
pub const FR_READDIR: u32 = 1 << 3;
pub const FR_EXECUTE: u32 = 1 << 4;
pub const FR_MMAP: u32 = 1 << 5;
pub const FR_GRANT: u32 = 1 << 24;
pub const FR_DERIVE: u32 = 1 << 25;
pub const FR_MOUNT: u32 = 1 << 27;

// === CapRights.meta bit values (mirror of `crate::sched::process::meta_rights`) =
pub const M_USE: u8 = 1 << 0;
pub const M_DERIVE: u8 = 1 << 1;
pub const M_GRANT: u8 = 1 << 2;
pub const M_REVOKE: u8 = 1 << 3;

// === CapRights.semantic bit values for file ops (defined HERE — this is the map) =
pub const S_READ: u8 = 1 << 0;
pub const S_WRITE: u8 = 1 << 1;
pub const S_LOOKUP: u8 = 1 << 2;
pub const S_READDIR: u8 = 1 << 3;
pub const S_EXECUTE: u8 = 1 << 4;
pub const S_MMAP: u8 = 1 << 5;
pub const S_MOUNT: u8 = 1 << 6;

/// The kill-switch (Rule 1). `file-caps` is default-off ⇒ every entry point below
/// reproduces the pre-unification behavior bit-for-bit.
pub const FILE_CAPS_ENABLED: bool = cfg!(feature = "file-caps");

/// The meta-reserved `FileRights` bits the opener of a file is rooted with
/// (`sys_open`, ISC-3): `DERIVE` + `GRANT`. Granting authority implies revoke
/// authority (see module docs), so the rooted handle's `rights()` also surfaces
/// `REVOKE`. Never set on the `OpenFile` (which stays at the user's semantic
/// rights) — only on the capability handle.
pub const FILE_OWNER_META_RESERVED: u32 = FR_GRANT | FR_DERIVE;

/// Encode a file handle's `FileRights` into the `(meta, semantic)` of a
/// `CapRights`, so the cap graph (`has_meta`/`is_subset_of`/`intersection`) gates
/// file derivation/granting exactly as for every other handle kind.
///
/// With the kill-switch off, returns `(0, 0)` — the pre-unification `rights()`.
#[inline]
pub const fn file_rights_to_cap(file_bits: u32, enabled: bool) -> (u8, u8) {
    if !enabled {
        return (0, 0);
    }
    let mut semantic: u8 = 0;
    if file_bits & FR_READ != 0 {
        semantic |= S_READ;
    }
    if file_bits & FR_WRITE != 0 {
        semantic |= S_WRITE;
    }
    if file_bits & FR_LOOKUP != 0 {
        semantic |= S_LOOKUP;
    }
    if file_bits & FR_READDIR != 0 {
        semantic |= S_READDIR;
    }
    if file_bits & FR_EXECUTE != 0 {
        semantic |= S_EXECUTE;
    }
    if file_bits & FR_MMAP != 0 {
        semantic |= S_MMAP;
    }
    if file_bits & FR_MOUNT != 0 {
        semantic |= S_MOUNT;
    }

    let mut meta: u8 = 0;
    // Any usable file op ⇒ USE (derived, not stored).
    if semantic != 0 {
        meta |= M_USE;
    }
    if file_bits & FR_DERIVE != 0 {
        meta |= M_DERIVE;
    }
    // Grant authority is revoke authority (the grantor is the revoker).
    if file_bits & FR_GRANT != 0 {
        meta |= M_GRANT | M_REVOKE;
    }
    (meta, semantic)
}

/// Decode a requested `CapRights` `(meta, semantic)` back into the `FileRights`
/// bits it names. Only the two *stored* meta bits (`GRANT`/`DERIVE`) reconstruct
/// reserved bits; `USE`/`REVOKE` are derived and intentionally not decoded.
///
/// This is the inverse of [`file_rights_to_cap`] on every bit a real handle can
/// hold, which is what makes spawn-inheritance (full-rights round-trip) exact.
#[inline]
pub const fn cap_to_file_rights(meta: u8, semantic: u8) -> u32 {
    let mut bits: u32 = 0;
    if semantic & S_READ != 0 {
        bits |= FR_READ;
    }
    if semantic & S_WRITE != 0 {
        bits |= FR_WRITE;
    }
    if semantic & S_LOOKUP != 0 {
        bits |= FR_LOOKUP;
    }
    if semantic & S_READDIR != 0 {
        bits |= FR_READDIR;
    }
    if semantic & S_EXECUTE != 0 {
        bits |= FR_EXECUTE;
    }
    if semantic & S_MMAP != 0 {
        bits |= FR_MMAP;
    }
    if semantic & S_MOUNT != 0 {
        bits |= FR_MOUNT;
    }
    if meta & M_DERIVE != 0 {
        bits |= FR_DERIVE;
    }
    if meta & M_GRANT != 0 {
        bits |= FR_GRANT;
    }
    bits
}

/// Attenuate a parent file handle's `FileRights` to the child requested by a
/// `with_cap(_, CapRights{meta, semantic})` derivation/grant/inheritance.
///
/// I-15: the result is always a **subset of the parent** — the requested rights
/// are decoded and intersected with the original, so a child can never exceed its
/// parent regardless of what the requester asks for. With the kill-switch off this
/// returns `original_bits` unchanged (the pre-unification `with_cap`, which ignored
/// the requested rights).
#[inline]
pub fn attenuate_file_rights(
    original_bits: u32,
    req_meta: u8,
    req_semantic: u8,
    enabled: bool,
) -> u32 {
    if !enabled {
        return original_bits;
    }
    let requested = cap_to_file_rights(req_meta, req_semantic);
    let child = original_bits & requested;
    // I-15 runtime guard: child FileRights ⊆ parent FileRights, always.
    debug_assert!(
        child & !original_bits == 0,
        "I-15 violated: derived file rights escaped the parent set"
    );
    child
}

/// The `FileRights` a freshly-opened file handle is rooted with (`sys_open`,
/// ISC-3): the user's requested semantic rights plus the owner meta-rights
/// (`DERIVE`+`GRANT`) so the opener can derive/grant/inherit from it. With the
/// kill-switch off, no meta bits are added — the handle carries exactly the user's
/// semantic rights, as before this build.
///
/// Only the capability *handle* receives these bits; the backing `OpenFile` is
/// opened with the user's semantic rights alone (file I/O semantics unchanged).
#[inline]
pub const fn root_file_handle_rights(user_semantic_bits: u32, enabled: bool) -> u32 {
    if !enabled {
        return user_semantic_bits;
    }
    user_semantic_bits | FILE_OWNER_META_RESERVED
}
