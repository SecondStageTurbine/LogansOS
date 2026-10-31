// SPDX-License-Identifier: MPL-2.0
//! The shared pixel-buffer object (G1 kernel object #1) and its embedded
//! software present fence.
//!
//! Ownership model (G1 SYNTHESIS §3, chad §4.2):
//! - the `SharedPixelBuffer` owns its physical frames and frees them in
//!   `Drop` — which can only run once every mapping's PTEs are gone,
//!   because each mapping holds a clone of the buffer `Arc` inside the
//!   mapping `Pmap` (`Pmap::map_borrowed_region` pins it; the pin drops
//!   after the Pmap's Drop body has removed PTEs and shot down TLBs);
//! - a `Pmap` mapping of these frames is a **borrowed** leaf (PTE `BIT_9`):
//!   `Pmap::Drop` removes the PTE but never frees the frame — the frame
//!   has exactly one owner (this object), so no double-free at exit;
//! - the compositor-role mapping is hardware RO + NX; the client-role
//!   mapping is RW + NX (W^X holds for both).
//!
//! The present fence is one epoch pair + a result record embedded in the
//! object — deliberately NOT a third kernel object kind (the synthesis
//! ratifies exactly two). Publication order is the I-16 discipline
//! (CP-8/CP-17): the result is stamped and the completed epoch is
//! `Release`-stored only AFTER the framebuffer rows are written and the
//! framebuffer lock is released; waiters `Acquire`-load the epoch. The
//! fence is an explicit signal, never a visibility proxy.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

use super::logic::{self, BufferDescV1, DamageRectV1};
use crate::drivers::block::DmaBuffer;
use crate::drivers::fb;
use crate::errno::{EAGAIN, EINVAL, ENOMEM, EPERM, ETIMEDOUT};
use crate::mm::{phys_to_virt, with_frame_alloc};

// `PresentResultV1` (the repr(C) POD + its 32-byte byte codec + LEN) lives in
// `logic.rs` so the host suite can mount and byte-oracle it (ADR-028), exactly
// like `BufferDescV1`. Re-exported here so existing `buffer::PresentResultV1` /
// `buffer::PRESENT_RESULT_V1_LEN` paths keep resolving.
pub use super::logic::{PRESENT_RESULT_V1_LEN, PresentResultV1};

/// Ceiling on a single fence wait (~10 min at the 100 Hz tick) — "bounded"
/// stays bounded even for a caller passing u64::MAX.
pub const MAX_FENCE_WAIT_TICKS: u64 = 60_000;
/// Yield-iteration backstop factor: with a live timer the tick deadline
/// always fires first (yield round-trips are ~microseconds, ticks ~10 ms);
/// with a frozen tick source the iteration cap terminates the wait.
const FENCE_YIELDS_PER_TICK: u64 = 100_000;

/// Shared-XOR-mutable state for a contiguous DMA pixel span.  Zero is free,
/// `1..CONTIGUOUS_WRITER` is a shared reader count, and the terminal value is
/// the exclusive writer marker.  Reader-count exhaustion fails closed too.
const CONTIGUOUS_FREE: u32 = 0;
const CONTIGUOUS_WRITER: u32 = u32::MAX;

/// The shared pixel-buffer kernel object. All fence/descriptor state lives
/// HERE, in kernel memory — never in the client-mappable frames (ISC-41):
/// a client scribble can corrupt only its own pixels, never the descriptor,
/// the epochs, or the result record.
pub struct SharedPixelBuffer {
    /// Immutable after construction (ISC-1).
    desc: BufferDescV1,
    /// The owned physical frames backing `desc.bytes` (page-rounded).
    frames: Vec<PhysFrame<Size4KiB>>,
    /// Present only for scanout-fencing-on buffers. Owns one contiguous span
    /// from the fenced DMA arena; `frames` remains the stable mapping/CPU view.
    arena_backing: Option<Arc<DmaBuffer>>,
    /// A shared-XOR-mutable borrow state for the contiguous-DMA escape hatches.
    /// `DmaBuffer::{as_slice,as_mut_slice}(&self)` cannot express this itself:
    /// grantable compute caps can otherwise let crossed submits manufacture an
    /// `&[u8]` and an `&mut [u8]` to one buffer before `COMPUTE_IO_GATE` runs.
    contiguous_borrow_state: AtomicU32,
    /// Commit gate (slice-1 damage model: full-repaint, commit-dirty).
    /// `Release`-stored by commit, `AcqRel`-swapped by present.
    committed: AtomicBool,
    /// The single bounded damage region published by the next commit.
    /// `present_serial` prevents replacement while a present consumes it.
    pending_damage: Mutex<Option<DamageRectV1>>,
    /// Serializes presents on this buffer (leaf lock: nothing else is
    /// acquired while held except the framebuffer's own internal lock).
    present_serial: Mutex<()>,
    /// Last present epoch handed out. Monotonic from 1; never reused.
    issued_epoch: AtomicU64,
    /// Last COMPLETED present epoch. `Release`-stored after the blit —
    /// the I-16 publication the fence waiter `Acquire`-loads.
    completed_epoch: AtomicU64,
    /// Result record for the most recent completed present. Guarded for
    /// torn-read freedom (CP-13); never held across the blit or a wait.
    result: Mutex<PresentResultV1>,
    /// OT-61 C2.2 (I-18): the creator's charge for these frames. A field, so
    /// it drops (releasing the charge) after `Drop::drop` has freed them.
    /// `None` for a compositor-internal surface (`create_gpu`).
    #[cfg(feature = "mem-quota")]
    _charge: Option<crate::mm::mem_account_logic::ChargeLedger>,
}

/// A shared contiguous pixel view.  The reader count is held for this guard's
/// full lifetime, so a writer cannot create an `&mut [u8]` while this slice is
/// live.
pub(crate) struct ContiguousPixels<'a> {
    pixels: &'a [u8],
    borrow_state: &'a AtomicU32,
}

impl ContiguousPixels<'_> {
    pub(crate) fn as_slice(&self) -> &[u8] {
        self.pixels
    }
}

impl Drop for ContiguousPixels<'_> {
    fn drop(&mut self) {
        let previous = self.borrow_state.fetch_sub(1, Ordering::Release);
        debug_assert!(previous > CONTIGUOUS_FREE && previous < CONTIGUOUS_WRITER);
    }
}

/// An exclusive contiguous pixel view.  The writer state is held for the full
/// lifetime of this guard, so callers cannot obtain either a second `&mut [u8]`
/// or an `&[u8]` from the same [`SharedPixelBuffer`].
///
/// This is deliberately not `DerefMut`: keeping mutable-slice creation behind
/// [`as_mut_slice`](Self::as_mut_slice) makes the unsafe-DMA seam visible at
/// every use while the guard remains live.
pub(crate) struct ContiguousPixelsMut<'a> {
    pixels: &'a mut [u8],
    borrow_state: &'a AtomicU32,
}

impl ContiguousPixelsMut<'_> {
    pub(crate) fn as_mut_slice(&mut self) -> &mut [u8] {
        self.pixels
    }
}

impl Drop for ContiguousPixelsMut<'_> {
    fn drop(&mut self) {
        self.borrow_state.store(CONTIGUOUS_FREE, Ordering::Release);
    }
}

impl SharedPixelBuffer {
    /// Allocate and zero the backing frames for a validated descriptor.
    pub fn create(desc: BufferDescV1) -> Result<Arc<Self>, i64> {
        let (out_w, out_h) = fb::dimensions();
        logic::validate_desc(&desc, out_w as u32, out_h as u32).map_err(|_| EINVAL)?;

        let pages = desc.bytes.div_ceil(0x1000) as usize;
        // OT-61 C2.2 (I-18): charged to the creator before anything is
        // allocated, and held by the buffer until its last reference (a grant
        // to the compositor included) drops. An early return below drops the
        // ledger, which releases the charge.
        #[cfg(feature = "mem-quota")]
        let charge = {
            let ledger = crate::mm::mem_account_logic::ChargeLedger::new(
                crate::sched::process::caller_mem_account(),
            );
            ledger.charge(pages as u64).map_err(|_| ENOMEM)?;
            ledger
        };
        let (frames, arena_backing) = if super::present_backend::scanout_fence_enabled() {
            let backing = DmaBuffer::alloc_in_arena(desc.bytes as usize).map_err(|_| ENOMEM)?;
            let base = backing.physical_addr().as_u64();
            let frames = (0..pages)
                .map(|page| {
                    PhysFrame::<Size4KiB>::containing_address(x86_64::PhysAddr::new(
                        base + (page * 0x1000) as u64,
                    ))
                })
                .collect();
            (frames, Some(backing))
        } else {
            let mut frames = Vec::with_capacity(pages);
            for _ in 0..pages {
                let Some(frame) = with_frame_alloc(|fa| fa.allocate_frame()) else {
                    free_frames(&frames);
                    return Err(ENOMEM);
                };
                // SAFETY: freshly allocated, HHDM-mapped, exclusively owned.
                unsafe {
                    core::ptr::write_bytes(
                        phys_to_virt(frame.start_address()).as_mut_ptr::<u8>(),
                        0,
                        0x1000,
                    );
                }
                frames.push(frame);
            }
            (frames, None)
        };

        Ok(Arc::new(Self {
            desc,
            frames,
            arena_backing,
            contiguous_borrow_state: AtomicU32::new(CONTIGUOUS_FREE),
            committed: AtomicBool::new(false),
            pending_damage: Mutex::new(None),
            present_serial: Mutex::new(()),
            issued_epoch: AtomicU64::new(0),
            completed_epoch: AtomicU64::new(0),
            result: Mutex::new(PresentResultV1::default()),
            #[cfg(feature = "mem-quota")]
            _charge: Some(charge),
        }))
    }

    /// Allocate a compositor-internal, GPU-reachable surface.  Unlike the
    /// legacy scanout-fence selection in [`create`](Self::create), this always
    /// goes through the Slice-5 `alloc_gpu` chokepoint: when graphics narrowing
    /// is armed it is therefore in the named graphics arena/domain; when the
    /// kill-switch is off it preserves the buddy-backed Slice-4 baseline.
    ///
    /// This is for capture/scratch/composite surfaces owned by the trusted G1
    /// compositor.  It is intentionally not a user syscall and does not mint a
    /// mapping capability.
    pub(crate) fn create_gpu(desc: BufferDescV1) -> Result<Arc<Self>, i64> {
        let (out_w, out_h) = fb::dimensions();
        logic::validate_desc(&desc, out_w as u32, out_h as u32).map_err(|_| EINVAL)?;
        let pages = desc.bytes.div_ceil(0x1000) as usize;
        let backing = DmaBuffer::alloc_gpu(desc.bytes as usize).map_err(|_| ENOMEM)?;
        let base = backing.physical_addr().as_u64();
        let frames = (0..pages)
            .map(|page| {
                PhysFrame::<Size4KiB>::containing_address(x86_64::PhysAddr::new(
                    base + (page * 0x1000) as u64,
                ))
            })
            .collect();
        Ok(Arc::new(Self {
            desc,
            frames,
            arena_backing: Some(backing),
            contiguous_borrow_state: AtomicU32::new(CONTIGUOUS_FREE),
            committed: AtomicBool::new(false),
            pending_damage: Mutex::new(None),
            present_serial: Mutex::new(()),
            issued_epoch: AtomicU64::new(0),
            completed_epoch: AtomicU64::new(0),
            result: Mutex::new(PresentResultV1::default()),
            #[cfg(feature = "mem-quota")]
            _charge: None,
        }))
    }

    pub fn desc(&self) -> BufferDescV1 {
        self.desc
    }

    pub fn frames(&self) -> &[PhysFrame<Size4KiB>] {
        &self.frames
    }

    /// Return a shared contiguous compositor-owned pixel view.  Only internally
    /// allocated arena/GPU surfaces have one; externally created legacy buffers
    /// remain frame-vector-backed and must first be captured with
    /// [`copy_pixels_to`](Self::copy_pixels_to).  A live writer makes this fail
    /// closed with `EAGAIN`; callers must keep the returned guard live while
    /// using [`ContiguousPixels::as_slice`].
    pub(crate) fn contiguous_pixels(&self) -> Result<ContiguousPixels<'_>, i64> {
        let backing = self.arena_backing.as_ref().ok_or(EINVAL)?;
        loop {
            let state = self.contiguous_borrow_state.load(Ordering::Acquire);
            if state >= CONTIGUOUS_WRITER - 1 {
                return Err(EAGAIN);
            }
            if self
                .contiguous_borrow_state
                .compare_exchange(state, state + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // SAFETY: this reader guard holds one shared borrow until drop;
                // a writer can only acquire from CONTIGUOUS_FREE.
                return Ok(ContiguousPixels {
                    pixels: unsafe { backing.as_slice() },
                    borrow_state: &self.contiguous_borrow_state,
                });
            }
        }
    }

    /// Mutable counterpart of [`contiguous_pixels`](Self::contiguous_pixels).
    /// It is crate-private because `DmaBuffer`'s ownership alone cannot encode
    /// the compositor's draw sequencing.  It may acquire only from the free
    /// state, closing both the `&mut`-from-`&self` seam and the crossed-submit
    /// `&[u8]`/`&mut [u8]` seam.  Any live reader or writer fails closed with
    /// `EAGAIN`; the returned guard releases the writer state on drop.
    pub(crate) fn contiguous_pixels_mut(&self) -> Result<ContiguousPixelsMut<'_>, i64> {
        let backing = self.arena_backing.as_ref().ok_or(EINVAL)?;
        if self
            .contiguous_borrow_state
            .compare_exchange(
                CONTIGUOUS_FREE,
                CONTIGUOUS_WRITER,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return Err(EAGAIN);
        }
        // SAFETY: the successful Free->Writer CAS makes this guard the sole
        // view of this buffer until `ContiguousPixelsMut::drop` releases it.
        Ok(ContiguousPixelsMut {
            pixels: unsafe { backing.as_mut_slice() },
            borrow_state: &self.contiguous_borrow_state,
        })
    }

    /// Capture this G1 buffer into a distinct contiguous compositor surface.
    /// The source may be a normal page-vector-backed client buffer; capturing
    /// it before compute prevents a user mapping from becoming a GPU operand.
    pub(crate) fn copy_pixels_to(
        self: &Arc<Self>,
        dst: &Arc<SharedPixelBuffer>,
    ) -> Result<(), i64> {
        if Arc::ptr_eq(self, dst)
            || self.desc.width != dst.desc.width
            || self.desc.height != dst.desc.height
            || self.desc.stride != dst.desc.stride
            || self.desc.bytes != dst.desc.bytes
        {
            return Err(EINVAL);
        }
        let mut target = dst.contiguous_pixels_mut()?;
        let mut offset = 0usize;
        let len = self.desc.bytes as usize;
        while offset < len {
            let frame_idx = offset / 0x1000;
            let frame_off = offset % 0x1000;
            let chunk = (0x1000 - frame_off).min(len - offset);
            // SAFETY: the immutable descriptor bounds `offset < bytes`; frames
            // cover the page-rounded allocation, and `chunk` stays in one HHDM
            // page owned by this buffer.
            let src = unsafe {
                core::slice::from_raw_parts(
                    phys_to_virt(self.frames[frame_idx].start_address())
                        .as_ptr::<u8>()
                        .add(frame_off),
                    chunk,
                )
            };
            target.as_mut_slice()[offset..offset + chunk].copy_from_slice(src);
            offset += chunk;
        }
        Ok(())
    }

    /// Page-rounded physical scanout span. Fencing-on creation guarantees this
    /// is one arena-owned contiguous allocation; legacy buffers are deliberately
    /// refused by the fence before they can reach `arm_flip`.
    pub(crate) fn scanout_span(&self) -> Option<(u64, u64)> {
        let first = self.frames.first()?.start_address().as_u64();
        let len = (self.frames.len() as u64).checked_mul(0x1000)?;
        Some((first, len))
    }

    /// Mark pending client pixel writes ready for the next present
    /// (slice-1 full-repaint damage model). `Release` pairs with the
    /// present path's `AcqRel` swap so the pixels written before commit
    /// are visible to the compositing CPU.
    pub fn commit(&self) {
        let mut pending = self.pending_damage.lock();
        *pending = Some(DamageRectV1 {
            x: 0,
            y: 0,
            width: self.desc.width as u16,
            height: self.desc.height as u16,
        });
        self.committed.store(true, Ordering::Release);
    }

    /// Publish one validated buffer-coordinate damage region. Rendering must
    /// finish before this Release publication; present's AcqRel consumption
    /// then makes the pixels visible without reading a half-rendered cell.
    pub fn commit_damage(&self, rect: DamageRectV1) -> Result<(), i64> {
        if !logic::validate_damage(rect, &self.desc) {
            return Err(EINVAL);
        }
        let mut pending = self.pending_damage.lock();
        *pending = Some(rect);
        self.committed.store(true, Ordering::Release);
        Ok(())
    }

    /// Composite this buffer to the framebuffer at `(dst_x, dst_y)` and
    /// complete its fence. Returns the present epoch (>= 1).
    ///
    /// Order (I-16 / CP-17 form 1 — monotonic one-shot per epoch):
    /// 1. serialize on `present_serial` (leaf lock);
    /// 2. consume the commit gate; no commit ⇒ `NO_DAMAGE` (no blit);
    /// 3. copy rows under ONE framebuffer lock acquisition, stride-aware,
    ///    all bounds pre-validated from the immutable descriptor;
    /// 4. after the framebuffer lock is RELEASED: stamp the result, then
    ///    `Release`-store `completed_epoch` — the fence signal. A waiter
    ///    that `Acquire`-observes the epoch therefore observes the blit.
    ///
    /// OT-63 Rung 1 Slice 2 + showcase AAG hardening — the `GpuSubmitCap` gate.
    /// A present on either backend reaches raw scanout; when enforcement is armed
    /// (`present_backend::gpu_submit_enforced`) the caller must hold a matching
    /// `GpuSubmit` capability (`caller_holds_gpu_submit`) or the submit is
    /// refused with `EPERM` **before any state changes** — no epoch issued, no
    /// blit, no flip armed. Enforcement-off (the default kill-switch) makes the
    /// parameter inert, so the path is bit-for-bit the Slice-1 behavior. The
    /// The enforcement-off default remains the Rule-1 kill-switch.
    pub fn present(
        self: &Arc<Self>,
        dst_x: usize,
        dst_y: usize,
        caller_holds_gpu_submit: bool,
    ) -> Result<u64, i64> {
        let _serial = self.present_serial.lock();

        // Snapshot the backend once: the gate below and the fence-completion
        // branch must agree on one mode even if the global toggle changes.
        let backend = super::present_backend::mode();

        // AAG hardening: both Software blits and HardwareFlip submissions write
        // raw scanout. Refuse an unprivileged caller before mutating any present
        // state, issuing an epoch, touching the framebuffer, or arming a flip.
        if !super::present_backend::scanout_authorized(caller_holds_gpu_submit) {
            return Err(EPERM);
        }

        let (out_w, out_h) = fb::dimensions();
        let w = self.desc.width as usize;
        let h = self.desc.height as usize;
        if dst_x.checked_add(w).is_none_or(|end| end > out_w)
            || dst_y.checked_add(h).is_none_or(|end| end > out_h)
        {
            return Err(EINVAL);
        }

        let epoch = self.issued_epoch.load(Ordering::Relaxed) + 1;
        self.issued_epoch.store(epoch, Ordering::Relaxed);
        let submit_tick = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);

        let damage = {
            let mut pending = self.pending_damage.lock();
            if self.committed.swap(false, Ordering::AcqRel) {
                pending.take()
            } else {
                None
            }
        };
        let outcome = if let Some(damage) = damage {
            // Blit: one framebuffer lock for the whole batch; the source
            // walk is kernel arithmetic over the immutable descriptor —
            // no client-controlled validation happens under the lock.
            fb::with_present(|target| {
                let Some(target) = target else {
                    return Err(EINVAL);
                };
                let stride = self.desc.stride as usize;
                let damage_x = damage.x as usize;
                let damage_y = damage.y as usize;
                let damage_w = damage.width as usize;
                let damage_h = damage.height as usize;
                let row_bytes = damage_w * 4;
                for row in damage_y..damage_y + damage_h {
                    let mut off = row * stride + damage_x * 4;
                    let row_end = off + row_bytes;
                    let mut dst_px = 0usize;
                    while off < row_end {
                        let frame_idx = off / 0x1000;
                        let in_off = off % 0x1000;
                        let chunk = (0x1000 - in_off).min(row_end - off);
                        // SAFETY: `frame_idx < frames.len()` because
                        // `row_end <= stride * height <= desc.bytes` and the
                        // frames vec covers `bytes` page-rounded; chunk stays
                        // inside one HHDM-mapped, buffer-owned frame.
                        let src = unsafe {
                            core::slice::from_raw_parts(
                                phys_to_virt(self.frames[frame_idx].start_address())
                                    .as_ptr::<u8>()
                                    .add(in_off),
                                chunk,
                            )
                        };
                        target.copy_row_px(dst_x + damage_x + dst_px, dst_y + row, src);
                        dst_px += chunk / 4;
                        off += chunk;
                    }
                }
                Ok(())
            })?;
            logic::OUTCOME_COMPOSED_TO_FB
        } else {
            logic::OUTCOME_NO_DAMAGE
        };

        // Result record — stamped in `present` context for BOTH backends,
        // strictly after the framebuffer lock is released. It is stamped HERE,
        // never in the present-completion ISR, because the ISR must stay
        // lock-free (it cannot take `result`'s `spin::Mutex` — a `fence_wait`
        // may hold it with interrupts enabled, CP-2). For the HardwareFlip
        // backend `complete_tick` is the arm time (within a frame of the real
        // vblank); Software stamps the true synchronous completion time.
        let complete_tick = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
        {
            let mut r = self.result.lock();
            *r = PresentResultV1 {
                present_id: epoch,
                outcome,
                flags: 0,
                submit_tick,
                complete_tick,
            };
        }

        // Complete the epoch fence per the selected backend (the Rule-1 present
        // kill-switch, OT-63 Rung 1 Slice 1). `Software` runs the exact Rung-0
        // sequence synchronously (SFENCE then the Release store) — bit-for-bit
        // the accel-off path. `HardwareFlip` defers the completion to the
        // present-completion ISR (Slice 1: the synthetic timer vblank; Slice 4:
        // the real Gen9 vblank/flip-done ISR), except a full ring falls back to
        // synchronous completion (the I-14(b) bounded-degradation path — the
        // present still completes, never dropped).
        match backend {
            super::present_backend::PresentMode::Software => self.publish_complete(epoch, outcome),
            super::present_backend::PresentMode::HardwareFlip => {
                let armed = if super::present_backend::scanout_fence_enabled() {
                    self.scanout_span()
                        .and_then(|(phys, len)| {
                            super::present_backend::fence_scanout(phys, len).ok()
                        })
                        .is_some_and(|fenced| {
                            super::present_backend::arm_flip(self.clone(), epoch, outcome, fenced)
                                .is_ok()
                        })
                } else {
                    // Exact Slice-2 synthetic completion. No hardware DMA
                    // exists on this disabled branch.
                    super::present_backend::arm_synthetic_completion(self.clone(), epoch, outcome)
                        .is_ok()
                };
                if !armed {
                    self.publish_complete(epoch, outcome);
                }
            }
        }
        Ok(epoch)
    }

    /// Publish the epoch fence for `epoch`: drain the write-combining
    /// framebuffer (`SFENCE`) when this present composited — the Limine fb may
    /// be write-combining and a WB Release store does NOT drain the WC buffer —
    /// then `Release`-store `completed_epoch`, the fence signal a waiter
    /// `Acquire`-observes. **Lock-free**: takes no `spin::Mutex` and no
    /// AI-governance lock, so the present-completion ISR
    /// (`present_backend::on_present_vblank`) calls it without self-deadlocking a
    /// `fence_wait` that holds `result` (CP-2) and without touching governance
    /// state (I-14a). The `result` record is already stamped by `present`.
    pub(crate) fn publish_complete(&self, epoch: u64, outcome: u32) {
        if outcome == logic::OUTCOME_COMPOSED_TO_FB {
            // SAFETY: SFENCE has no preconditions.
            unsafe { core::arch::x86_64::_mm_sfence() };
        }
        // I-16: the explicit fence publication. Pairs with the waiter's
        // `Acquire` load in `fence_wait` — never a visibility proxy.
        //
        // MONOTONIC (`fetch_max`, not `store`): `fence_satisfied` is
        // `completed_epoch >= wait_epoch`, which relies on `completed_epoch`
        // being monotonic-non-decreasing (the ISC-39 buffer-reuse ABA guard).
        // Publishes can arrive OUT OF ORDER: the full-ring I-14(b) fallback
        // synchronously publishes the newest epoch e17 in `present`, then the
        // FIFO vblank drain publishes the older queued e1..e16 — an
        // unconditional `store` would regress the fence (e17 → e16) and strand
        // the e17 waiter to `ETIMEDOUT` on the very degradation path the design
        // promises is safe. `fetch_max` keeps the invariant regardless of
        // publish order (and is robust to any future out-of-order publish).
        self.completed_epoch.fetch_max(epoch, Ordering::Release);
    }

    /// Bounded wait for present `wait_epoch` to complete.
    ///
    /// Deliberately a deadline-bounded **yield loop**, not a park: slice-1
    /// adds zero scheduler wake machinery, so the lost/stranded-wakeup
    /// classes (CP-14/15/16) are structurally absent on this path — there
    /// is no park call to race against, no lock is held across the wait,
    /// and every iteration yields the CPU.
    ///
    /// The bound is TWO independent backstops:
    /// 1. the tick deadline — the wait loop runs inside
    ///    [`with_interrupts_enabled`](crate::arch::x86_64::with_interrupts_enabled)
    ///    (SYSCALL entry masks IF; this path *explicitly re-enables* it for
    ///    the duration of the wait, restoring the prior IF on return — the
    ///    OT-59 #2 / CP-2 pattern that `sys_read_stdin` and the NeoFS read
    ///    path use). With IF on, the timer fires *on this CPU* while the
    ///    waiter is live, so `TICKS` genuinely advances and the deadline is
    ///    reached — the bound does not depend on `yield_now()` happening to
    ///    reach the idle thread's `sti; hlt`;
    /// 2. a yield-iteration cap — if the tick source ever freezes (a boot
    ///    window, a future scheduler/timer change), the loop still
    ///    terminates deterministically instead of becoming an unbounded
    ///    spin (CP-2/CP-3).
    ///
    /// No lock is held across the yield/IF-on region: `completed_epoch` and
    /// `TICKS` are lock-free atomics, and `result` is a leaf mutex taken and
    /// released within the satisfied branch, never across `yield_now()`.
    ///
    /// `timeout_ticks == 0` is a non-blocking probe (`EAGAIN` if pending);
    /// nonzero timeouts are clamped to [`MAX_FENCE_WAIT_TICKS`].
    pub fn fence_wait(&self, wait_epoch: u64, timeout_ticks: u64) -> Result<PresentResultV1, i64> {
        if wait_epoch == 0 {
            return Err(EINVAL);
        }
        // A not-yet-issued epoch is a legitimate bounded wait (it also keeps
        // the timeout path reachable / falsifiable: slice-1 presents complete
        // synchronously, so an issued epoch is already terminal by the time
        // any single-threaded waiter can ask).
        let timeout_ticks = timeout_ticks.min(MAX_FENCE_WAIT_TICKS);
        // Run the wait with interrupts enabled so the timer fires on this CPU
        // and backstop 1 (the tick deadline) is a real, live mechanism —
        // not a claim that rests on the waiter running IF-off. IF is restored
        // to its prior (masked) state when the closure returns.
        crate::arch::x86_64::with_interrupts_enabled(|| {
            let deadline = crate::arch::x86_64::timer::TICKS
                .load(Ordering::Acquire)
                .saturating_add(timeout_ticks);
            // Backstop 2: bounds the wait even if TICKS never advances. Sized
            // so the tick deadline fires first whenever the timer is alive.
            let max_yields = timeout_ticks.saturating_mul(FENCE_YIELDS_PER_TICK);
            let mut yields: u64 = 0;
            loop {
                // Acquire pairs with `present`'s Release store (I-16):
                // observing the epoch here happens-after every framebuffer
                // row write.
                if logic::fence_satisfied(self.completed_epoch.load(Ordering::Acquire), wait_epoch)
                {
                    return Ok(*self.result.lock());
                }
                if timeout_ticks == 0 {
                    return Err(EAGAIN);
                }
                if crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire) >= deadline
                    || yields >= max_yields
                {
                    return Err(ETIMEDOUT);
                }
                yields += 1;
                crate::sched::scheduler::yield_now();
            }
        })
    }

    /// OT-63 GPU P1 Slice 1 — the CPU glass-blur floor over this backdrop.
    ///
    /// Runs the pinned two-pass Q16 separable Gaussian (ADR-077 v2 §1) over
    /// `self`'s pixels (the composited backdrop under a glass surface) and writes
    /// the result into `dst` (a same-geometry buffer), then commits the
    /// **radius-expanded** damage on `dst` (amendment 3: a blur of radius `r`
    /// spreads a changed cell by `r`). This is the cap-native software floor AND
    /// the bit-for-bit reference the later Gen9 GPU kernel must equal.
    ///
    /// **Kill-switch (Rule-1, [`super::glass_blur_enabled`]):** when the glass
    /// blur is disabled, the backdrop passes through **unblurred** — the solid
    /// Reduce-Transparency floor — bit-for-bit, and only the `changed` cell is
    /// committed (no spread, because nothing was blurred). Enabled ⇒ the two-pass
    /// blur + the radius-expanded damage.
    ///
    /// **I-14 (governance fault-containment):** CPU-only at this slice — no
    /// device, no DMA engine, no new IRQ. (a) it acquires **no** governance lock
    /// (`INFLIGHT`/`RECENCY`/`ARENA`-reservation/`TABLE`) — only the leaf pixel
    /// buffers; (b) it is a bounded, straight-line CPU pass (no wait, no park),
    /// so it cannot stall governance — a hung compositor degrades to this floor,
    /// never the reverse; (c) is **N/A** until the GPU slices introduce
    /// device-initiated DMA (ADR-077 v2 §3.3 — c is closed for the CPU floor by
    /// having no device programming at all).
    ///
    /// Slice-1 scope: both buffers are **arena-backed** (one contiguous span —
    /// the real glass path, ADR-077 v2 §1.1), so the whole buffer is a linear
    /// `&[u8]`. A non-arena buffer is refused (`EINVAL`): its per-page frames
    /// have no contiguous CPU view, a later slice's page-gather concern. The blur
    /// is computed over the **full surface** (the conservative
    /// full-bounds-on-damage of ADR-077 v2 §1.1 — a superset of the `2r` source
    /// halo, [`super::blur::source_halo_rect`]); the bounded incremental
    /// halo-only recompute is the named later optimization.
    ///
    /// `radius` + `weights` are explicit so the kernel path can be asserted
    /// against the literal `r=1` 8×8 byte oracle (the QEMU scenario) as well as
    /// driven with the production `r=8` table.
    ///
    /// Slice-1 exercises this from the `atrium_glass_blur` scenario; Slice 6
    /// wires it into the live glass compositor.  It uses the same guarded views
    /// as those live paths, so this compatibility wrapper cannot bypass the
    /// shared-XOR-mutable discipline.
    #[allow(dead_code)]
    pub fn blur_backdrop(
        self: &Arc<Self>,
        dst: &Arc<SharedPixelBuffer>,
        radius: usize,
        weights: &[i32],
        changed: DamageRectV1,
    ) -> Result<(), i64> {
        // Same immutable geometry both sides; format is XRGB8888 by construction.
        if self.desc.width != dst.desc.width
            || self.desc.height != dst.desc.height
            || self.desc.stride != dst.desc.stride
        {
            return Err(EINVAL);
        }
        let dims = super::blur::BlurDims {
            width: self.desc.width as usize,
            height: self.desc.height as usize,
            stride: self.desc.stride as usize,
            radius,
        };

        // Alias guard: &[u8] and &mut [u8] for the same backing would be UB.
        // `Arc::ptr_eq` is sufficient because each buffer owns its own arena
        // backing (`Option<Arc<DmaBuffer>>`) — distinct Arcs back disjoint
        // DMA-arena spans that cannot alias.
        if Arc::ptr_eq(self, dst) {
            return Err(EINVAL);
        }

        // Contiguous CPU views require arena backing (Slice-1 scope).  Retain
        // both guards through the complete operation: this old test wrapper is
        // deliberately routed through the same reader/writer state machine as
        // the live compositor and submit paths.
        let src = self.contiguous_pixels()?;
        let mut dst_bytes = dst.contiguous_pixels_mut()?;

        let enabled = super::glass_blur_enabled();
        // Intermediate scratch (only used when enabled); one contiguous buffer.
        let mut mid = alloc::vec![0u8; self.desc.bytes as usize];
        super::blur::apply_blur_or_floor(
            src.as_slice(),
            &mut mid,
            dst_bytes.as_mut_slice(),
            dims,
            weights,
            enabled,
        )
        .map_err(|_| EINVAL)?;

        // Commit the damage: radius-expanded when blurred (the spread), the
        // changed cell as-is when passed through (no spread). Clamp to the
        // surface, then narrow to `DamageRectV1` (u16) — the 8×8..surface range
        // always fits, but guard the conversion rather than truncate.
        let committed = if enabled {
            let changed_rect = super::blur::BlurRect {
                x: changed.x as u32,
                y: changed.y as u32,
                width: changed.width as u32,
                height: changed.height as u32,
            };
            let out = super::blur::expand_output_rect(
                changed_rect,
                radius as u32,
                self.desc.width,
                self.desc.height,
            );
            DamageRectV1 {
                x: u16::try_from(out.x).map_err(|_| EINVAL)?,
                y: u16::try_from(out.y).map_err(|_| EINVAL)?,
                width: u16::try_from(out.width).map_err(|_| EINVAL)?,
                height: u16::try_from(out.height).map_err(|_| EINVAL)?,
            }
        } else {
            changed
        };
        dst.commit_damage(committed)
    }
}

impl Drop for SharedPixelBuffer {
    /// Frees the owned frames. Reachable only once every handle AND every
    /// mapping pin (held by `Pmap`s until their PTEs are removed and shot
    /// down) has dropped — so a frame is never freed while a PTE can still
    /// reach it (ISC-8/ISC-9).
    fn drop(&mut self) {
        if self.arena_backing.is_none() {
            free_frames(&self.frames);
        }
    }
}

fn free_frames(frames: &[PhysFrame<Size4KiB>]) {
    if frames.is_empty() {
        return;
    }
    with_frame_alloc(|fa| {
        for f in frames {
            fa.free(*f);
        }
    });
}
