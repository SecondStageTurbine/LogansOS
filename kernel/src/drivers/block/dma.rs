// SPDX-License-Identifier: MPL-2.0
//! Identity-DMA buffer ownership for Phase 3 BUILD-1.
//!
//! These buffers are physically contiguous frames reachable through the
//! kernel HHDM. The abstraction is intentionally named for DMA ownership,
//! not for today's identity mapping, so later IOMMU or bounce-buffer work can
//! stay behind this boundary.

extern crate alloc;

use alloc::sync::Arc;
use core::ptr;
#[cfg(feature = "neo-test")]
use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::structures::paging::{PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

use crate::mm;

const PAGE_SIZE: usize = 4096;

#[cfg(feature = "neo-test")]
static LIVE_DMA_BUFFERS: AtomicUsize = AtomicUsize::new(0);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DmaError {
    ZeroLength,
    TooLarge,
    OutOfFrames,
    /// I-14(c): a GPU-reachable allocation was requested while a real
    /// bus-mastering accelerator is bound but its DMA is not contained by the
    /// graphics VT-d domain. Handing back unrestricted buddy memory here would
    /// let the device DMA anywhere, so the allocation fails closed instead.
    ContainmentRequired,
}

/// Where a [`DmaBuffer`]'s frames came from — chooses the Drop return path.
/// `Buddy` is every pre-7.5 caller; `Connectivity` is the fenced connectivity
/// arena (ADR-045); `Graphics` is the named graphics arena (OT-63 Slice 5,
/// ADR-077 amendment 5). The two arenas are separately reserved and disjoint,
/// so a span must return to the arena it came from — the kill-switch keeps
/// GPU-reachable allocations `Buddy` until Slice-5 narrowing is enabled.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Backing {
    Buddy,
    Connectivity,
    Graphics,
}

pub struct DmaBuffer {
    phys: PhysAddr,
    virt: VirtAddr,
    len: usize,
    allocated_pages: usize,
    /// Which allocator owns these frames (chooses the Drop return path).
    /// NOTE: for arena buffers `allocated_pages` is the exact span length
    /// and is informational only (the SpanTable owns the free-side truth);
    /// for `Buddy` buffers it is the rounded power-of-two the buddy freed.
    backing: Backing,
}

// SAFETY: the buffer owns its physical frames until Drop. Shared access is
// controlled by callers through `Arc<DmaBuffer>` and the block request
// lifetime; raw mutation remains explicit through unsafe pointer/slice APIs.
unsafe impl Send for DmaBuffer {}
unsafe impl Sync for DmaBuffer {}

impl DmaBuffer {
    pub fn alloc_zeroed(len: usize) -> Result<Arc<Self>, DmaError> {
        if len == 0 {
            return Err(DmaError::ZeroLength);
        }
        let pages = len.checked_add(PAGE_SIZE - 1).ok_or(DmaError::TooLarge)? / PAGE_SIZE;
        let order = order_for_pages(pages).ok_or(DmaError::TooLarge)?;
        let allocated_pages = 1usize << order;

        let frame =
            mm::with_frame_alloc(|fa| fa.allocate_blocks(order)).ok_or(DmaError::OutOfFrames)?;
        let phys = frame.start_address();
        let virt = mm::phys_to_virt(phys);
        let allocation_len = allocated_pages
            .checked_mul(PAGE_SIZE)
            .ok_or(DmaError::TooLarge)?;

        // SAFETY: the freshly allocated frames are exclusively owned by this
        // DmaBuffer and HHDM-mapped.
        unsafe {
            ptr::write_bytes(virt.as_mut_ptr::<u8>(), 0, allocation_len);
        }

        #[cfg(feature = "neo-test")]
        LIVE_DMA_BUFFERS.fetch_add(1, Ordering::AcqRel);

        Ok(Arc::new(Self {
            phys,
            virt,
            len,
            allocated_pages,
            backing: Backing::Buddy,
        }))
    }

    /// Phase 7.5 BUILD-0 (ADR-045): allocate from the FENCED DMA ARENA — the
    /// only allocation path connectivity drivers (7.5-A onward) may use.
    /// Fails when the arena is uninitialized (the gate fails closed) or
    /// exhausted. Existing users keep [`DmaBuffer::alloc_zeroed`] untouched.
    pub fn alloc_in_arena(len: usize) -> Result<Arc<Self>, DmaError> {
        if len == 0 {
            return Err(DmaError::ZeroLength);
        }
        let pages = len.checked_add(PAGE_SIZE - 1).ok_or(DmaError::TooLarge)? / PAGE_SIZE;
        let phys =
            crate::drivers::dma_arena::alloc_span(pages as u64).ok_or(DmaError::OutOfFrames)?;
        let virt = mm::phys_to_virt(phys);
        let allocation_len = pages.checked_mul(PAGE_SIZE).ok_or(DmaError::TooLarge)?;

        // SAFETY: the span is exclusively owned by this DmaBuffer (the arena
        // allocator hands each span to exactly one owner) and HHDM-mapped —
        // the same argument as alloc_zeroed's, over the arena's frames.
        unsafe {
            ptr::write_bytes(virt.as_mut_ptr::<u8>(), 0, allocation_len);
        }

        #[cfg(feature = "neo-test")]
        LIVE_DMA_BUFFERS.fetch_add(1, Ordering::AcqRel);

        Ok(Arc::new(Self {
            phys,
            virt,
            len,
            allocated_pages: pages,
            backing: Backing::Connectivity,
        }))
    }

    /// OT-63 GPU P1 Slice 5 (ADR-077 amendment 5): allocate from the named
    /// GRAPHICS arena — the only allocation path a GPU-reachable buffer may use
    /// once the graphics VT-d domain is narrowed, so the domain (which maps
    /// exactly the graphics arena) contains it. Fails closed when the graphics
    /// arena is uninitialized or exhausted. Distinct from the connectivity
    /// arena so the iGPU's working set never exposes NIC/USB DMA buffers.
    pub fn alloc_in_graphics_arena(len: usize) -> Result<Arc<Self>, DmaError> {
        if len == 0 {
            return Err(DmaError::ZeroLength);
        }
        let pages = len.checked_add(PAGE_SIZE - 1).ok_or(DmaError::TooLarge)? / PAGE_SIZE;
        let phys = crate::drivers::graphics_arena::alloc_span(pages as u64)
            .ok_or(DmaError::OutOfFrames)?;
        let virt = mm::phys_to_virt(phys);
        let allocation_len = pages.checked_mul(PAGE_SIZE).ok_or(DmaError::TooLarge)?;

        // SAFETY: the span is exclusively owned by this DmaBuffer (the arena
        // allocator hands each span to exactly one owner) and HHDM-mapped —
        // the same argument as alloc_in_arena's, over the graphics region.
        unsafe {
            ptr::write_bytes(virt.as_mut_ptr::<u8>(), 0, allocation_len);
        }

        #[cfg(feature = "neo-test")]
        LIVE_DMA_BUFFERS.fetch_add(1, Ordering::AcqRel);

        Ok(Arc::new(Self {
            phys,
            virt,
            len,
            allocated_pages: pages,
            backing: Backing::Graphics,
        }))
    }

    /// OT-63 GPU P1 Slice 5: the GPU-reachable-allocation chokepoint (the
    /// kill-switch seam). When Slice-5 graphics-VT-d narrowing is enabled, a
    /// GPU-reachable buffer MUST come from the graphics arena so the narrowed
    /// domain contains it.
    ///
    /// **I-14(c) fail-closed (Slice-4 AAG bounce, 2026-08-01).** The original
    /// `else` arm fell back to unrestricted [`alloc_zeroed`] buddy memory
    /// whenever narrowing was off. For a **simulated** accelerator that is
    /// harmless and preserves the Rule-1 equality (no device, no DMA). For a
    /// **real bus-mastering** accelerator it is an uncontained DMA master with
    /// unrestricted physical reach — precisely what I-14(c) forbids, and
    /// `dma_arena::fence` is never consulted on this path, so `fence_refusals`
    /// would read 0 because the quantity is never measured. Once a hardware
    /// accelerator is bound the fallback is therefore removed and the
    /// allocation fails closed.
    pub fn alloc_gpu(len: usize) -> Result<Arc<Self>, DmaError> {
        use crate::drivers::graphics_arena;
        if graphics_arena::narrowing_enabled() {
            return Self::alloc_in_graphics_arena(len);
        }
        if graphics_arena::hardware_accelerator_bound() {
            return Err(DmaError::ContainmentRequired);
        }
        // No real DMA master is bound (simulated accelerator or none): the
        // pre-Slice-5 buddy path, bit-for-bit — the Rule-1 equality.
        Self::alloc_zeroed(len)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn physical_addr(&self) -> PhysAddr {
        self.phys
    }

    pub fn virtual_addr(&self) -> VirtAddr {
        self.virt
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.virt.as_ptr::<u8>()
    }

    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.virt.as_mut_ptr::<u8>()
    }

    /// Caller must ensure no device or CPU peer is concurrently mutating the
    /// buffer while the returned slice is live.
    #[allow(clippy::mut_from_ref)] // DMA buffer exclusively owned by kernel; &mut is safe
    pub unsafe fn as_mut_slice(&self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }

    pub unsafe fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len) }
    }
}

#[cfg(feature = "neo-test")]
pub fn debug_live_buffers() -> usize {
    LIVE_DMA_BUFFERS.load(Ordering::Acquire)
}

impl Drop for DmaBuffer {
    fn drop(&mut self) {
        #[cfg(feature = "neo-test")]
        LIVE_DMA_BUFFERS.fetch_sub(1, Ordering::AcqRel);

        match self.backing {
            Backing::Connectivity => {
                // The span returns to the connectivity arena; the region
                // itself stays reserved for the kernel's lifetime (ADR-045). A
                // refused release is counted by the arena's leaked-span gauge —
                // the signal survives outside neo-test builds (Forge MINOR-1).
                let _ = crate::drivers::dma_arena::release_span(self.phys);
                return;
            }
            Backing::Graphics => {
                // The span returns to the graphics arena (OT-63 Slice 5); same
                // discipline, its own reserved region + leaked-span gauge.
                let _ = crate::drivers::graphics_arena::release_span(self.phys);
                return;
            }
            Backing::Buddy => {}
        }
        let base = self.phys.as_u64();
        mm::with_frame_alloc(|fa| {
            for page in 0..self.allocated_pages {
                let phys = PhysAddr::new(base + (page * PAGE_SIZE) as u64);
                let frame = PhysFrame::<Size4KiB>::containing_address(phys);
                fa.free(frame);
            }
        });
    }
}

fn order_for_pages(pages: usize) -> Option<usize> {
    if pages == 0 {
        return None;
    }
    let mut order = 0usize;
    let mut covered = 1usize;
    while covered < pages {
        covered = covered.checked_shl(1)?;
        order += 1;
    }
    Some(order)
}
