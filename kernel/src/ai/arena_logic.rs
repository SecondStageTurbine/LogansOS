// SPDX-License-Identifier: MPL-2.0
//! Pure slot-arena logic (ANSPEC-001 §9.12 ownership, BUILD-O1a): generation
//! arithmetic, key matching, free-list selection, and the handle-count
//! lifecycle — kernel-free and host-testable (path-`mod`'d by
//! `tools/ai-ops-tests`). The kernel glue (`arena.rs`) owns the `Arc`s,
//! the `SwitchMutex`, and the P4a reservation integration; every decision
//! it makes about *which* slot, *which* generation, and *when to free*
//! lives here as a total function.
//!
//! ABA note (invariant I-13): a key carries the generation its slot had at
//! alloc; `free` bumps the generation (wrapping), so a stale key fails
//! [`key_matches`] on every later occupant. The wrap is benign in practice:
//! a stale key aliases only after exactly 2³² frees of the *same* slot index
//! while the key is held — documented and bounded in ADR-042.

extern crate alloc;

use alloc::vec::Vec;

/// A generational slot name: the only thing a holder keeps. `index` addresses
/// the table; `generation` must match the slot's current generation or every
/// arena operation refuses (stale ⇒ clean failure, never the new occupant).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct SlotKey {
    pub index: u32,
    pub generation: u32,
}

/// Where `alloc` puts the next tensor.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum AllocPick {
    /// Reuse a freed slot (LIFO off the free list).
    Reuse(u32),
    /// Grow the table by one; the new slot's index.
    Grow(u32),
}

/// Stale-or-current test — the I-13 gate every keyed operation runs first.
pub fn key_matches(slot_generation: u32, key: SlotKey, slot_index: u32) -> bool {
    slot_index == key.index && slot_generation == key.generation
}

/// The generation bump a `free` applies. Wrapping: u32 wrap is benign (see
/// the module head); `wrapping_add` keeps the arithmetic total.
pub fn bump_generation(generation: u32) -> u32 {
    generation.wrapping_add(1)
}

/// Pick the slot for a new allocation: reuse the most recently freed slot if
/// one exists, else grow. LIFO reuse maximizes generation churn on hot
/// indices — deliberately adversarial for I-13 (the reuse-attack tests want
/// stale keys to *collide* with live indices, not dodge them).
pub fn alloc_pick(free_list: &mut Vec<u32>, table_len: usize) -> AllocPick {
    match free_list.pop() {
        Some(index) => AllocPick::Reuse(index),
        None => AllocPick::Grow(table_len as u32),
    }
}

/// Return a freed slot's index to the free list — unless reuse is disabled
/// (the Rule-1 kill-switch): with reuse off the index is never reissued and
/// the arena is append-only, bit-for-bit the pre-O1a daemon registry shape.
pub fn recycle_slot(free_list: &mut Vec<u32>, index: u32, reuse_enabled: bool) {
    if reuse_enabled {
        free_list.push(index);
    }
}

/// `retain`: one more handle names the slot. `None` on overflow (a caller
/// that minted 2³² handles is broken; the glue treats it as a refused
/// retain rather than wrapping into a premature free).
pub fn retain_count(handles: u32) -> Option<u32> {
    handles.checked_add(1)
}

/// `release`: one handle dropped. Returns the new count and whether this
/// release was the last one (the slot frees now — the GEM `handle_count`
/// analogue). `None` if the count was already zero (a double release is a
/// caller bug; the glue refuses it instead of underflowing).
pub fn release_count(handles: u32) -> Option<(u32, bool)> {
    let next = handles.checked_sub(1)?;
    Some((next, next == 0))
}
