// SPDX-License-Identifier: MPL-2.0
//! B1a: which pages of a consumed Limine boot module may be handed to the frame
//! allocator (Chad C2, `Plans/DECISION-2026-10-19-arthur-first-boot-model.md`
//! Amendment 1 (b)).
//!
//! Pure and allocation-free: host-tested by `tools/module-reclaim-logic-tests`
//! through `#[path]`, so keep it free of `crate::` paths and feature cfgs.
//!
//! The Limine protocol guarantees 4 KiB alignment and non-overlap only for
//! `USABLE` and `BOOTLOADER_RECLAIMABLE` entries. `EXECUTABLE_AND_MODULES`
//! (type 6) entries may be unaligned, may overlap, and may also describe the
//! kernel and other modules. So the plan is driven from the module's *exact*
//! file range, never from a memmap entry:
//!
//! 1. round the file range **inward** to whole pages, so a page the module
//!    shares with anything else (its own unaligned head or tail) is retained;
//! 2. require every byte of that page range to be described by type-6 entries,
//!    and by no entry of any other type;
//! 3. require it to be disjoint from the executable's and every other module's
//!    range, each rounded **outward** to whole pages.
//!
//! Anything else is refused and nothing is admitted (fail closed).

/// Bytes per frame.
pub const PAGE: u64 = 4096;

/// Limine `MEMMAP_EXECUTABLE_AND_MODULES`.
pub const MEMMAP_EXECUTABLE_AND_MODULES: u64 = 6;

/// A half-open physical byte range `[start, end)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u64,
    pub end: u64,
}

impl Span {
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    /// Whole pages.
    pub const fn pages(self) -> u64 {
        if self.is_empty() {
            0
        } else {
            (self.end - self.start) / PAGE
        }
    }

    const fn overlaps(self, other: Span) -> bool {
        !self.is_empty() && !other.is_empty() && self.start < other.end && other.start < self.end
    }
}

/// One memmap entry, as Limine reports it.
#[derive(Clone, Copy, Debug)]
pub struct MapEntry {
    pub base: u64,
    pub length: u64,
    pub kind: u64,
}

/// Why a module's pages may not be admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanRefusal {
    /// The module has no bytes.
    EmptyModule,
    /// An address computation overflowed.
    Overflow,
    /// The module does not contain one whole page.
    NoWholePage,
    /// Some page is not described by any type-6 entry.
    NotCovered,
    /// Some page is also described by an entry of another type.
    ForeignTypeOverlap,
    /// Some page is shared with the kernel executable.
    ExecutableOverlap,
    /// Some page is shared with another module.
    ModuleOverlap,
}

const fn align_up(value: u64) -> Option<u64> {
    match value.checked_add(PAGE - 1) {
        Some(v) => Some(v & !(PAGE - 1)),
        None => None,
    }
}

const fn align_down(value: u64) -> u64 {
    value & !(PAGE - 1)
}

/// `span` rounded outward to whole pages.
fn outward(span: Span) -> Result<Span, PlanRefusal> {
    if span.is_empty() {
        return Ok(span);
    }
    let end = align_up(span.end).ok_or(PlanRefusal::Overflow)?;
    Ok(Span::new(align_down(span.start), end))
}

fn entry_span(entry: &MapEntry) -> Result<Span, PlanRefusal> {
    let end = entry
        .base
        .checked_add(entry.length)
        .ok_or(PlanRefusal::Overflow)?;
    Ok(Span::new(entry.base, end))
}

/// The whole pages of `module` that may be admitted, or why none may.
pub fn plan_module_reclaim(
    module: Span,
    memmap: &[MapEntry],
    executable: Span,
    other_modules: &[Span],
) -> Result<Span, PlanRefusal> {
    if module.is_empty() {
        return Err(PlanRefusal::EmptyModule);
    }
    let start = align_up(module.start).ok_or(PlanRefusal::Overflow)?;
    let end = align_down(module.end);
    if start >= end {
        return Err(PlanRefusal::NoWholePage);
    }
    let pages = Span::new(start, end);

    for entry in memmap {
        let span = entry_span(entry)?;
        if entry.kind != MEMMAP_EXECUTABLE_AND_MODULES && span.overlaps(pages) {
            return Err(PlanRefusal::ForeignTypeOverlap);
        }
    }

    // Coverage sweep over possibly overlapping type-6 entries.
    let mut cursor = pages.start;
    while cursor < pages.end {
        let mut reach = cursor;
        for entry in memmap {
            if entry.kind != MEMMAP_EXECUTABLE_AND_MODULES {
                continue;
            }
            let span = entry_span(entry)?;
            if span.start <= cursor && cursor < span.end && span.end > reach {
                reach = span.end;
            }
        }
        if reach == cursor {
            return Err(PlanRefusal::NotCovered);
        }
        cursor = reach;
    }

    if outward(executable)?.overlaps(pages) {
        return Err(PlanRefusal::ExecutableOverlap);
    }
    for other in other_modules {
        if outward(*other)?.overlaps(pages) {
            return Err(PlanRefusal::ModuleOverlap);
        }
    }
    Ok(pages)
}

/// The base an admitted zone's buddy arithmetic is relative to: `start`
/// aligned **down** to a `max_order` block. Blocks are only ever pushed inside
/// the planned pages, so a buddy below `start` is never on a free list and is
/// never merged (the coalescer merges only what it finds there).
pub const fn zone_base(start: u64, max_order: usize) -> u64 {
    let block = PAGE << max_order;
    start & !(block - 1)
}

/// Greedy decomposition of whole-page `span` into naturally aligned
/// power-of-two blocks, largest order first, never exceeding `max_order`.
/// Yields `(address, order)`; the blocks tile `span` exactly.
pub struct Blocks {
    cursor: u64,
    end: u64,
    max_order: usize,
}

pub const fn blocks(span: Span, max_order: usize) -> Blocks {
    Blocks {
        cursor: span.start,
        end: span.end,
        max_order,
    }
}

impl Iterator for Blocks {
    type Item = (u64, usize);

    fn next(&mut self) -> Option<(u64, usize)> {
        if self.cursor >= self.end || self.end - self.cursor < PAGE {
            return None;
        }
        let mut order = self.max_order;
        loop {
            let size = PAGE << order;
            let aligned = self.cursor & (size - 1) == 0;
            let fits = self.end - self.cursor >= size;
            if aligned && fits {
                break;
            }
            if order == 0 {
                // Unreachable for page-aligned spans: a page always fits.
                return None;
            }
            order -= 1;
        }
        let item = (self.cursor, order);
        self.cursor += PAGE << order;
        Some(item)
    }
}

/// B1c (F4): the largest buddy order a span of `pages` whole pages is
/// **guaranteed** to hold as one naturally aligned block, whatever the span's
/// alignment: order `k` iff `pages >= 2^(k+1) - 1` (the worst start sits one
/// page past an aligned boundary). Capped at `max_order`; `None` for no pages.
/// The Q8_0 0.5B module's 639 MiB span (163,525 whole pages) guarantees order
/// 16; the Q4_K_M module's 463 MiB guaranteed only 15. The model's tensors
/// still come from the primary zone, because the model is built before its
/// module is reclaimed.
#[allow(dead_code)] // only the reclaim scenario and the host KATs call it
pub const fn guaranteed_order(pages: u64, max_order: usize) -> Option<usize> {
    if pages == 0 {
        return None;
    }
    let mut order = 0usize;
    while order < max_order {
        let next = order + 1;
        // 2^(next+1) - 1 pages guarantee an aligned 2^next block.
        let needed = (1u64 << (next + 1)) - 1;
        if pages < needed {
            break;
        }
        order = next;
    }
    Some(order)
}
