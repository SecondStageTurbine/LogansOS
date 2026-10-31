// SPDX-License-Identifier: MPL-2.0
//! Pure fenced-DMA-arena logic (Phase 7.5 BUILD-0, ADR-045 — the I-14(c1)
//! guard's arithmetic): the fence-bounds validation and the span allocator,
//! kernel-free and host-testable (path-`mod`'d by `tools/ai-ops-tests`).
//! The kernel glue (`dma_arena.rs`) owns the reserved region, the atomics,
//! and the [`FencedDmaAddr`](super::dma_arena::FencedDmaAddr) witness type.
//!
//! The validation is deliberately total and saturating-free: every overflow
//! is an explicit refusal, never a wrap — an address that cannot be checked
//! is an address that cannot reach a device.

extern crate alloc;

use alloc::vec::Vec;

/// Why an address was refused at the fence. `ArenaUnset` covers the
/// kill-switch state (no arena ⇒ no connectivity DMA at all — the gate
/// fails CLOSED, never open).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DmaFault {
    ZeroLength,
    ArenaUnset,
    Overflow,
    OutsideArena,
}

/// The I-14(c1) fence check: `[phys, phys+len)` must lie entirely inside
/// `[base, base+arena_len)`. Total: zero-length, unset arena, and bound
/// overflow are explicit refusals.
pub fn validate(phys: u64, len: u64, base: u64, arena_len: u64) -> Result<(), DmaFault> {
    if len == 0 {
        return Err(DmaFault::ZeroLength);
    }
    if arena_len == 0 {
        return Err(DmaFault::ArenaUnset);
    }
    let end = phys.checked_add(len).ok_or(DmaFault::Overflow)?;
    let fence_end = base.checked_add(arena_len).ok_or(DmaFault::Overflow)?;
    if phys >= base && end <= fence_end {
        Ok(())
    } else {
        Err(DmaFault::OutsideArena)
    }
}

/// One allocated span inside the arena, in pages.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct Span {
    offset_pages: u64,
    pages: u64,
    used: bool,
}

/// First-fit page-span allocator over a fixed region — the arena's
/// book-keeping, pure (offsets only; the glue owns the physical region).
/// Free coalesces with adjacent free spans so the arena never fragments
/// permanently.
pub struct SpanTable {
    total_pages: u64,
    spans: Vec<Span>,
}

impl SpanTable {
    pub fn new(total_pages: u64) -> Self {
        Self {
            total_pages,
            spans: alloc::vec![Span {
                offset_pages: 0,
                pages: total_pages,
                used: false,
            }],
        }
    }

    /// First-fit: the offset (in pages) of a fresh `pages`-long span, or
    /// `None` if no free span is large enough.
    pub fn alloc(&mut self, pages: u64) -> Option<u64> {
        if pages == 0 || pages > self.total_pages {
            return None;
        }
        let idx = self
            .spans
            .iter()
            .position(|s| !s.used && s.pages >= pages)?;
        let span = self.spans[idx];
        if span.pages == pages {
            self.spans[idx].used = true;
        } else {
            self.spans[idx] = Span {
                offset_pages: span.offset_pages,
                pages,
                used: true,
            };
            self.spans.insert(
                idx + 1,
                Span {
                    offset_pages: span.offset_pages + pages,
                    pages: span.pages - pages,
                    used: false,
                },
            );
        }
        Some(span.offset_pages)
    }

    /// Release the span starting at `offset_pages`; `false` if no used span
    /// starts there (a double free or a forged offset — refused, the I-13
    /// fail-safe discipline).
    pub fn free(&mut self, offset_pages: u64) -> bool {
        let Some(idx) = self
            .spans
            .iter()
            .position(|s| s.used && s.offset_pages == offset_pages)
        else {
            return false;
        };
        self.spans[idx].used = false;
        // Coalesce with the next, then the previous, free neighbour.
        if idx + 1 < self.spans.len() && !self.spans[idx + 1].used {
            self.spans[idx].pages += self.spans[idx + 1].pages;
            self.spans.remove(idx + 1);
        }
        if idx > 0 && !self.spans[idx - 1].used {
            self.spans[idx - 1].pages += self.spans[idx].pages;
            self.spans.remove(idx);
        }
        true
    }

    /// Pages currently allocated — the balance gauge (0 at teardown).
    pub fn used_pages(&self) -> u64 {
        self.spans.iter().filter(|s| s.used).map(|s| s.pages).sum()
    }
}
