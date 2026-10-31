// SPDX-License-Identifier: MPL-2.0
//! OT-63 GPU P1 Slice 4 — cap-native Gen9 blur submission and its CPU floor.
//!
//! [`blur_backdrop`] submits the pinned `BLUR_KERNEL_GEN9` through the real
//! Gen9 ELSP/LRC path when the runtime switch is enabled. The LRC supplies a
//! per-context 48-bit PPGTT; the horizontal walker writes `mid`, the vertical
//! walker writes `dst`, and `PIPE_CONTROL` posts the completion seqno. The
//! backend owns those bytes on success: the service does not overwrite a GPU
//! result with a CPU calculation.
//!
//! QEMU's `accel-sim` checks service wiring and the exact bound-buffer contract.
//! It cannot prove EU execution. The `accel_intel_blur_silicon` and
//! `accel_intel_blur_silicon_narrowed` bare-metal scenarios are the acceptance
//! gates for real UHD 630 execution and the VT-d-domain negative control.
//!
//! ## Amendment 4 — wholly kernel-internal, no userland queue
//! There is **no `Handle::GpuCompute`, no syscall, no process-controlled
//! dimensions/buffers** here — a userland compute queue before the graphics VT-d
//! fence is exactly what I-14(c) forbids (ADR-077 v2 §3, amendment 4). The only
//! gate is the feature/runtime kill-switch [`GPU_COMPUTE_ENABLE`]; the process
//! capability ([`gpu_compute_cap_logic::authority_allows`]) is separate from
//! this kernel-internal gate; no process-controlled compute queue exists here.
//!
//! [`gpu_compute_cap_logic::authority_allows`]: crate::sched::gpu_compute_cap_logic::authority_allows
//!
//! ## Completion is a fence, NOT a present (no second scanout path)
//! [`blur_backdrop`] writes the destination buffer and stops. Its completion
//! token conveys "the blur finished," never "a frame reached scanout." It never
//! touches `present`/`arm_flip`/`completed_epoch`/`PLANE_SURF` — the compositor
//! alone holds the `GpuSubmit` present authority (ADR-073 `COMPOSITOR_ROLE`), and
//! the compositor separately calls `present` on the blurred result. The compute
//! path must not become a second road to the display (ADR-077 v2 amendment 4).
//!
//! ## Kill-switch (Rule-1)
//! `GPU_COMPUTE_ENABLE` default-**off** ⇒ the compute submit path is **inert**
//! (no submit occurs, [`compute_admits`] does not advance) and the blur resolves
//! to the Slice-1 CPU floor, **bit-for-bit** the `blur::blur_pass_h`/`_v` output.
//! Enabled ⇒ the backend must produce the bytes. A rejected or over-deadline
//! submit degrades to that same CPU floor and increments [`compute_degrades`].
//! A correct destination accompanied by a degrade is not GPU evidence.
//!
//! ## I-14 fault-containment (compute submit path)
//! - **(a) Lock isolation.** The submit path acquires only the compute I/O gate
//!   ([`COMPUTE_IO_GATE`], a leaf `SwitchMutex`) and, inside the driver, the
//!   driver's own device lock — **no** AI-governance lock (`INFLIGHT`/`RECENCY`/
//!   `ARENA`-reservation/`TABLE`). Grep-assertable.
//! - **(b) Liveness degradation.** The fence wait is deadline-bounded
//!   ([`AccelDriver::wait`] with a timeout); a hung compute cannot stall
//!   governance — the caller computes the CPU floor only after the failure.
//!   Governance never awaits the GPU without a deadline.
//! - **(c) Memory isolation.** Buffers are kernel-internal and every Gen9 A64
//!   operand is mapped only in the submission's 48-bit PPGTT. In the narrowed
//!   scenario every GPU-reachable allocation (ring/LRC/HWSP/state/kernel/CURBE/
//!   seqno/page tables) comes from the graphics VT-d arena; the weights negative
//!   deliberately escapes it and must fault. No userland queue exists.
//!
//! ## Concurrency (CP-18 — single-in-flight submit→wait→complete)
//! The Gen9 substrate is single-slot (`IntelGpuInner::in_flight: Option<_>`); a
//! second caller interleaving submit/wait on that one slot is the CP-18 class.
//! [`blur_backdrop`] holds [`COMPUTE_IO_GATE`] across the **entire**
//! submit→wait→complete sequence (a *separate* leaf lock from any driver queue
//! lock, acquired interruptible), so two kernel callers serialize. CP-2/CP-3 are
//! **N-A**: completion is a deadline-bounded **poll** ([`AccelDriver::wait`]), not
//! a new GPU interrupt, so there is no IF-off-across-cross-CPU-wait hazard.
//! CP-11 (atomic ordering): the toggle uses Release/Acquire like `GPU_SUBMIT_ENFORCE`.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::time::Duration;

use crate::atrium::blur::{self, BlurDims};
use crate::sync::SwitchMutex;

use super::{
    AccelDriver, AccelError, AccelResult, BlurDispatchMode, ComputeBindings, ComputePipeline,
    DispatchToken,
};

/// Runtime half of the compute kill-switch (ADR-077 v2 §8 Slice 2), mirroring
/// `present_backend`'s `GPU_SUBMIT_ENFORCE`. Default **off**: the compute submit
/// path is inert and the blur is the Slice-1 CPU floor, bit-for-bit. Flipped on
/// only by the `accel_gpu_compute` scenario (and, later, the ratified accel-on
/// build).
static GPU_COMPUTE_ENABLE: AtomicBool = AtomicBool::new(false);

/// Count of successful compute-pipeline submits that completed their fence — the
/// falsifiable evidence the submit path was exercised (advances only when
/// enabled). The kill-switch equality asserts this stays 0 when disabled.
static COMPUTE_ADMITS: AtomicU64 = AtomicU64::new(0);

/// Backend-accepted submissions (before the fence wait). This distinguishes a
/// dispatched-but-hung EU from a run in which only the CPU floor executed.
static COMPUTE_DISPATCHES: AtomicU64 = AtomicU64::new(0);

/// Enabled-path failures hidden from the caller by the mandatory CPU floor.
/// A correct destination with this counter non-zero is a degraded run, never
/// Slice-4 EU evidence.
static COMPUTE_DEGRADES: AtomicU64 = AtomicU64::new(0);

/// Serializes the whole submit→wait→complete per the (single-slot) compute
/// engine — the CP-18 gate. A *leaf* lock, separate from the driver's queue
/// lock, acquired interruptible; the driver's completion poll never needs it.
static COMPUTE_IO_GATE: SwitchMutex<()> = SwitchMutex::new(());

/// Bound on a single compute fence wait — "bounded" even if a driver misbehaves.
/// `accel_sim` completes synchronously (ignores the timeout); `accel_intel`'s
/// `wait` honors it (the I-14(b) liveness backstop).
const COMPUTE_WAIT: Duration = Duration::from_secs(2);

/// A returned token with `sequence == 0` means "computed synchronously on the CPU
/// floor; already complete, no fence to wait." (The compute substrate never mints
/// sequence 0 — `next_dispatch_token` starts at 1.)
const FLOOR_SEQUENCE: u64 = 0;

/// Arm/disarm the compute service (see [`GPU_COMPUTE_ENABLE`]).
pub fn set_gpu_compute_enable(enabled: bool) {
    GPU_COMPUTE_ENABLE.store(enabled, Ordering::Release);
}

/// Whether the compute submit path runs. Off ⇒ [`blur_backdrop`] is the pure
/// Slice-1 CPU floor with no submit.
#[inline]
pub fn gpu_compute_enabled() -> bool {
    GPU_COMPUTE_ENABLE.load(Ordering::Acquire)
}

/// Successful compute submits observed (test/among the kill-switch evidence).
pub fn compute_admits() -> u64 {
    COMPUTE_ADMITS.load(Ordering::Relaxed)
}

pub fn compute_dispatches() -> u64 {
    COMPUTE_DISPATCHES.load(Ordering::Relaxed)
}

pub fn compute_degrades() -> u64 {
    COMPUTE_DEGRADES.load(Ordering::Relaxed)
}

/// Scenario-only: reset the submit counter between phases.
#[cfg(feature = "neo-test")]
pub fn reset_compute_admits() {
    COMPUTE_ADMITS.store(0, Ordering::Relaxed);
    COMPUTE_DISPATCHES.store(0, Ordering::Relaxed);
    COMPUTE_DEGRADES.store(0, Ordering::Relaxed);
}

/// The compute-service blur (ADR-077 v2 §2.2), kernel-internal — no syscall, no
/// `Handle` (amendment 4). Produces the two-pass Q16 separable-Gaussian blur of
/// `src` into `dst` (via the `mid` scratch), and returns the completion fence.
///
/// - **Kill-switch off:** the CPU floor produces the bytes; no submit occurs; the
///   returned token is the [`FLOOR_SEQUENCE`] sentinel (already complete). The
///   output is bit-for-bit the Slice-1 `blur::blur_pass_h`/`_v` result.
/// - **Kill-switch on (Slice 4):** the backend owns the bound source, scratch,
///   and destination buffers. A successful fence returns its real token and the
///   backend-produced destination untouched. Submit/wait failure runs the CPU
///   floor and returns the already-complete sentinel; [`compute_degrades`] makes
///   that masked failure distinguishable from real EU evidence.
///
/// `driver` is the compute substrate (injected: `accel-sim` under QEMU,
/// `accel-intel` on bare-metal; Slice 6 resolves it from the registry). The
/// whole submit→wait sequence is held under [`COMPUTE_IO_GATE`] (CP-18).
pub fn blur_backdrop(
    driver: &Arc<dyn AccelDriver>,
    src: &[u8],
    mid: &mut [u8],
    dst: &mut [u8],
    dims: BlurDims,
    weights: &[i32],
) -> AccelResult<DispatchToken> {
    // CP-18: one owner of submit→wait→complete on the single-slot engine. Leaf
    // lock, held across the whole sequence; the driver's completion poll never
    // takes it.
    let _gate = COMPUTE_IO_GATE.lock();

    if gpu_compute_enabled() {
        // Slice-3: submit the REAL blur manifest — `ComputePipeline::SeparableBlur`
        // (the ocloc-compiled Gen9 EU kernel, `BLUR_KERNEL_GEN9`). Under QEMU/
        // accel-sim this validates the submit → seqno-fence carries the pinned ABI
        // shape (SLM=0, barrier=0, the cross-thread payload); on bare-metal
        // accel-intel it dispatches the two-pass EU blur (silicon proof = Slice 4).
        let manifest = ComputePipeline::SeparableBlur {
            width: dims.width as u32,
            height: dims.height as u32,
            stride: dims.stride as u32,
            radius: dims.radius as u32,
            mode: BlurDispatchMode::TwoPass,
        };
        // I-14(b) graceful degrade — the Slice-2 carry-forward, discharged now the
        // real path exists (ADR-077 v2 §3.3(b), §5 Slice 3). A hung/failed GPU
        // submit or an OVER-DEADLINE wait must NOT error out before the
        // always-available CPU floor runs: governance never awaits the GPU. On any
        // submit/wait failure the blur degrades to the CPU floor, `COMPUTE_ADMITS`
        // does NOT advance, and the returned token is the `FLOOR_SEQUENCE`
        // sentinel. On success the real fence token is returned and admits ticks.
        let mut bindings = ComputeBindings {
            src,
            mid,
            dst,
            weights,
        };
        let completed = match driver.submit_compute_pipeline(&manifest, &mut bindings) {
            Ok(token) => {
                COMPUTE_DISPATCHES.fetch_add(1, Ordering::Relaxed);
                driver.wait(token, Some(COMPUTE_WAIT)).map(|()| token)
            }
            Err(error) => Err(error),
        };
        match completed {
            Ok(t) => {
                COMPUTE_ADMITS.fetch_add(1, Ordering::Relaxed);
                Ok(t)
            }
            Err(_) => {
                COMPUTE_DEGRADES.fetch_add(1, Ordering::Relaxed);
                blur::blur_pass_h(src, mid, dims, weights)
                    .map_err(|_| AccelError::InvalidRequest)?;
                blur::blur_pass_v(mid, dst, dims, weights)
                    .map_err(|_| AccelError::InvalidRequest)?;
                Ok(DispatchToken {
                    device: driver.descriptor().device,
                    sequence: FLOOR_SEQUENCE,
                })
            }
        }
    } else {
        // Inert: no submit. The Slice-1 CPU floor is the whole story.
        blur::blur_pass_h(src, mid, dims, weights).map_err(|_| AccelError::InvalidRequest)?;
        blur::blur_pass_v(mid, dst, dims, weights).map_err(|_| AccelError::InvalidRequest)?;
        Ok(DispatchToken {
            device: driver.descriptor().device,
            sequence: FLOOR_SEQUENCE,
        })
    }
}
