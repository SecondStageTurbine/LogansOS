// SPDX-License-Identifier: MPL-2.0
//! OT-63 Rung 1 Slice 1 — the `present()` fence-completion backend.
//!
//! [`SharedPixelBuffer::present`](super::buffer::SharedPixelBuffer::present)
//! completes its epoch fence one of two ways, chosen by [`mode`] (the Rule-1
//! kill-switch):
//!
//! - **[`PresentMode::Software`]** (default / accel-off) — the fence's
//!   `completed_epoch` is `Release`-stored synchronously inside `present`,
//!   exactly the Rung-0 path, **bit-for-bit**.
//! - **[`PresentMode::HardwareFlip`]** — `present` arms the flip into a bounded
//!   ring and returns; `completed_epoch` is `Release`-stored **later**, by the
//!   present-completion ISR ([`on_present_vblank`]). In Slice 1 the timer ISR
//!   drives it — a *synthetic vblank*, QEMU-provable with no GPU. Slice 4 swaps
//!   the trigger for the real Gen9 vblank/flip-done ISR + the `PLANE_SURF` flip
//!   register write, leaving this seam unchanged.
//!
//! ## Concurrency (CP-2 / CP-14 / CP-16, I-14a/b)
//! - The ISR path is **lock-free per buffer**: [`on_present_vblank`] calls only
//!   [`SharedPixelBuffer::publish_complete`](super::buffer::SharedPixelBuffer::publish_complete)
//!   — an `SFENCE` + one `Release` atomic store. It takes **no** per-buffer
//!   `spin::Mutex` (so it can never self-deadlock against a `fence_wait` holding
//!   `result` with interrupts enabled, CP-2) and **no** AI-governance lock
//!   (I-14a). The only lock is the ring's `IrqSafeMutex` leaf (IF masked while
//!   held; bounded critical section, CP-3-safe), released *before* each publish.
//! - The waiter is the unchanged `fence_wait` `with_interrupts_enabled`
//!   yield-loop: no park to lose (CP-14/15 structurally absent), the vblank IRQ
//!   can fire on the waiting CPU (CP-2), and the yield-loop's `Acquire` observes
//!   the ISR's `Release`. There is no explicit wake path (CP-16 N/A).
//! - The ring's Release-on-unlock / Acquire-on-lock handoff orders `present`'s
//!   `result` stamp (on the arming CPU) before the ISR's publish (on any CPU).
//! - A **full ring** falls back to synchronous completion in `present` (the
//!   I-14(b) bounded-degradation path): the present still completes, just
//!   synchronously — never dropped, never an unbounded wait.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use crate::drivers::dma_arena::{self, DmaFault, FencedDmaAddr};
use crate::sync::IrqSafeMutex;

use super::buffer::SharedPixelBuffer;

/// Fence-completion backend for `present()`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PresentMode {
    /// Rung-0: fence `Release`-stored synchronously in `present` (default,
    /// accel-off — the kill-switch).
    Software,
    /// Rung-1: fence completed asynchronously by the present-completion ISR.
    HardwareFlip,
}

const MODE_SOFTWARE: u8 = 0;
const MODE_HARDWARE_FLIP: u8 = 1;

/// Default `Software` = the Rule-1 kill-switch / accel-off state.
static PRESENT_MODE: AtomicU8 = AtomicU8::new(MODE_SOFTWARE);

/// OT-63 Slice 3 scanout-DMA fence. Disabled is the exact Slice-2 path: buffer
/// allocation and flip arming do not consult the DMA arena. Accel-on enables
/// this before creating scanout targets, making those buffers arena-backed.
static SCANOUT_FENCE_ENABLED: AtomicBool = AtomicBool::new(false);
static SCANOUT_FENCE_ADMITS: AtomicU64 = AtomicU64::new(0);
static SCANOUT_FENCE_REFUSALS: AtomicU64 = AtomicU64::new(0);

pub fn set_scanout_fence_enabled(enabled: bool) {
    SCANOUT_FENCE_ENABLED.store(enabled, Ordering::Release);
}

pub fn scanout_fence_enabled() -> bool {
    SCANOUT_FENCE_ENABLED.load(Ordering::Acquire)
}

pub fn scanout_fence_admits() -> u64 {
    SCANOUT_FENCE_ADMITS.load(Ordering::Relaxed)
}

pub fn scanout_fence_refusals() -> u64 {
    SCANOUT_FENCE_REFUSALS.load(Ordering::Relaxed)
}

#[cfg(feature = "neo-test")]
pub fn reset_scanout_fence_counters() {
    SCANOUT_FENCE_ADMITS.store(0, Ordering::Relaxed);
    SCANOUT_FENCE_REFUSALS.store(0, Ordering::Relaxed);
}

/// Mint the only address currency accepted by [`arm_flip`]. The DMA arena gate
/// is lock-free and fails closed for unset, overflowing, or out-of-arena spans.
pub fn fence_scanout(phys: u64, len: u64) -> Result<FencedDmaAddr, DmaFault> {
    match dma_arena::fence(phys, len) {
        Ok(fenced) => {
            SCANOUT_FENCE_ADMITS.fetch_add(1, Ordering::Relaxed);
            Ok(fenced)
        }
        Err(fault) => {
            SCANOUT_FENCE_REFUSALS.fetch_add(1, Ordering::Relaxed);
            Err(fault)
        }
    }
}

/// When false, [`on_present_vblank`] does not drain — used only by the Slice-1
/// scenario to make the async-deferral and the I-14(b) timeout paths
/// deterministic (present + non-blocking probe = pending; short-timeout wait =
/// `ETIMEDOUT`). Production never touches it; the real vblank ISR always drains.
static DRAIN_ENABLED: AtomicBool = AtomicBool::new(true);

/// The current present backend.
pub fn mode() -> PresentMode {
    match PRESENT_MODE.load(Ordering::Acquire) {
        MODE_HARDWARE_FLIP => PresentMode::HardwareFlip,
        _ => PresentMode::Software,
    }
}

/// Select the present backend. `Software` is the kill-switch default; the
/// HardwareFlip path is armed only by an accel-on build (Slice 1: the
/// `atrium_hw_present` scenario) — never in the accel-off / kill-switch build.
pub fn set_mode(m: PresentMode) {
    PRESENT_MODE.store(
        match m {
            PresentMode::HardwareFlip => MODE_HARDWARE_FLIP,
            PresentMode::Software => MODE_SOFTWARE,
        },
        Ordering::Release,
    );
}

/// Scenario-only: gate the synthetic vblank drain (see [`DRAIN_ENABLED`]).
pub fn set_drain_enabled(enabled: bool) {
    DRAIN_ENABLED.store(enabled, Ordering::Release);
}

/// OT-63 Rung 1 Slice 2 + showcase AAG hardening — the `GpuSubmitCap`
/// enforcement toggle (the Rule-1 kill-switch for scanout authority). Default
/// **false** preserves the pre-feature ambient-present behavior. When armed,
/// every raw-scanout present requires the cap: both a Software framebuffer blit
/// and a HardwareFlip submission refuse an un-capped caller with `EPERM` before
/// mutating scanout or present state.
static GPU_SUBMIT_ENFORCE: AtomicBool = AtomicBool::new(false);

/// Arm/disarm `GpuSubmitCap` enforcement (see [`GPU_SUBMIT_ENFORCE`]).
pub fn set_gpu_submit_enforce(enabled: bool) {
    GPU_SUBMIT_ENFORCE.store(enabled, Ordering::Release);
}

/// Whether enforcement is currently armed (kill-switch state).
pub fn gpu_submit_enforced() -> bool {
    GPU_SUBMIT_ENFORCE.load(Ordering::Acquire)
}

/// The authority decision for one raw-scanout present: `true` ⇒ the present may
/// proceed on either backend. Delegates to the host-tested truth table
/// [`crate::sched::gpu_submit_cap_logic::authority_allows`] so kernel and host
/// KAT share one definition: enforcement-off ⇒ always allowed (kill-switch);
/// enforcement-on ⇒ allowed iff the caller holds a matching `GpuSubmit` cap.
#[inline]
pub fn scanout_authorized(caller_holds_gpu_submit: bool) -> bool {
    crate::sched::gpu_submit_cap_logic::authority_allows(
        gpu_submit_enforced(),
        caller_holds_gpu_submit,
    )
}

/// One in-flight hardware present: keep the buffer `Arc` alive until the flip
/// completes (the scanout must not be freed mid-flip), plus the epoch to signal
/// and whether it composited (⇒ whether `publish_complete` must `SFENCE`).
struct PendingFlip {
    buffer: Arc<SharedPixelBuffer>,
    epoch: u64,
    outcome: u32,
    /// Construction proof: no queued hardware flip can carry a raw/unvalidated
    /// scanout address. Slice 4 consumes this witness at PLANE_SURF programming.
    _scanout: Option<FencedDmaAddr>,
}

/// Bounded pending-flip ring. Sized so several presents can be in flight
/// between vblanks; a full ring is the I-14(b) fallback trigger, never a drop.
/// `pub` so the scenario can fill it precisely (`RING_CAP + 1` in flight).
pub const RING_CAP: usize = 16;

struct PendingRing {
    slots: [Option<PendingFlip>; RING_CAP],
    head: usize,
    len: usize,
}

impl PendingRing {
    const fn new() -> Self {
        Self {
            slots: [const { None }; RING_CAP],
            head: 0,
            len: 0,
        }
    }

    fn push(&mut self, p: PendingFlip) -> Result<(), PendingFlip> {
        if self.len == RING_CAP {
            return Err(p);
        }
        let idx = (self.head + self.len) % RING_CAP;
        self.slots[idx] = Some(p);
        self.len += 1;
        Ok(())
    }

    fn pop(&mut self) -> Option<PendingFlip> {
        if self.len == 0 {
            return None;
        }
        let p = self.slots[self.head].take();
        self.head = (self.head + 1) % RING_CAP;
        self.len -= 1;
        p
    }
}

static PENDING: IrqSafeMutex<PendingRing> = IrqSafeMutex::new(PendingRing::new());

/// Arm a hardware flip: queue the present for async completion by the vblank
/// ISR. Returns `Err(())` if the ring is full (the caller then completes
/// synchronously — the I-14(b) bounded fallback). Takes only the IrqSafe ring
/// leaf; no AI-governance lock.
pub fn arm_flip(
    buffer: Arc<SharedPixelBuffer>,
    epoch: u64,
    outcome: u32,
    scanout: FencedDmaAddr,
) -> Result<(), ()> {
    let mut ring = PENDING.lock();
    ring.push(PendingFlip {
        buffer,
        epoch,
        outcome,
        _scanout: Some(scanout),
    })
    .map_err(|_| ())
}

/// Exact Slice-2 synthetic-vblank completion path for the fencing-off
/// kill-switch. This is deliberately not an arm/scanout API: no device
/// register is programmed in Slice 2. Slice 4 must call only [`arm_flip`],
/// whose witness parameter makes a raw address impossible to submit.
pub(crate) fn arm_synthetic_completion(
    buffer: Arc<SharedPixelBuffer>,
    epoch: u64,
    outcome: u32,
) -> Result<(), ()> {
    let mut ring = PENDING.lock();
    ring.push(PendingFlip {
        buffer,
        epoch,
        outcome,
        _scanout: None,
    })
    .map_err(|_| ())
}

/// Present-completion ISR body — the synthetic vblank (Slice 1: driven by the
/// timer ISR; Slice 4: the real Gen9 vblank/flip-done ISR). Drains every
/// pending flip and publishes its fence completion. **Lock-free per buffer**
/// (only the IrqSafe ring leaf + the lock-free `publish_complete`); acquires no
/// AI-governance lock (I-14a).
pub fn on_present_vblank() {
    if !DRAIN_ENABLED.load(Ordering::Acquire) {
        return;
    }
    // Pop one at a time: the ring critical section stays minimal and
    // `publish_complete` (SFENCE + Release store) runs with the ring lock
    // released, so a concurrent `arm_flip` on another CPU is never blocked by a
    // publish.
    loop {
        let pending = {
            let mut ring = PENDING.lock();
            ring.pop()
        };
        let Some(p) = pending else {
            return;
        };
        p.buffer.publish_complete(p.epoch, p.outcome);
    }
}
