// SPDX-License-Identifier: MPL-2.0
//! Placement + dispatch glue for the executor (BUILD-C1 split of the former
//! `exec.rs` `allocate_result`). One op's journey from validated operands to
//! a frozen output tensor: the §9.9 placement decision, the §9.10 room
//! consult (with the §9.11 retain flags), the BUILD-P4b daemon need-note and
//! kick, the OOM backstops, the accounted dispatch with the transparent
//! accel_cpu fallbacks, and the §9.5 recency touches. The pass driver lives
//! in `exec.rs`; the per-op shape rules in `exec_ops.rs`.

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::drivers::accel::{self, AccelDriver, AccelError, OpBatch, accounting};

use super::evict;
use super::exec::InferenceError;
use super::ops::{OpKind, TensorView};
use super::placement::{self, OperandResidency};
use super::tensor::{Device, Dtype, Shape, Tensor, TensorError};

/// Where the executor sends an op. `Fixed` reproduces the legacy single-driver
/// dispatch (the cross-device equivalence scenarios and thrash baselines);
/// `Placed` consults the residency-aware policy per op — `lookahead` is the
/// predicted-hot window `W` (§9.11); `lookahead == 0` disables every proactive
/// decision, the §9.10 reactive path bit for bit (the strict-superset
/// guarantee, ledger I-2).
pub(super) enum Target<'a> {
    Fixed(&'a dyn AccelDriver),
    Placed { lookahead: usize },
}

/// Per-op structural prediction (§9.11): the predicted-hot and
/// demote-eligible **value-index** sets computed from the remaining op graph,
/// plus the seam controls. Built fresh per node by the pass driver; read-only
/// inputs to the synchronous decisions.
pub(super) struct OpLookahead {
    /// `lookahead > 0` — any proactive decision enabled at all.
    pub(super) active: bool,
    /// No demote seam after the final op (nothing left to bank for).
    pub(super) last_op: bool,
    /// Values consumed by an op within the next `W` ops (predicted-hot).
    pub(super) hot: BTreeSet<u32>,
    /// Values dead to the pass (no remaining op use, not a declared output) —
    /// the only safe proactive-demotion candidates (the thrash guard).
    pub(super) demote_ok: BTreeSet<u32>,
}

impl OpLookahead {
    pub(super) const INERT: Self = Self {
        active: false,
        last_op: true,
        hot: BTreeSet::new(),
        demote_ok: BTreeSet::new(),
    };
}

/// Migration accounting for the P1 gate: operands a dispatch had to stage
/// onto its run device, across a whole pass. Both the placed path and a
/// `Fixed` baseline feed these with the same rule ([`placement::migration_to`])
/// so the gate compares like for like. Reset before a measured run.
static MIGRATIONS: AtomicU64 = AtomicU64::new(0);
static MIGRATED_BYTES: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn reset_migration_counter() {
    MIGRATIONS.store(0, Ordering::Relaxed);
    MIGRATED_BYTES.store(0, Ordering::Relaxed);
}

#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn migration_count() -> u64 {
    MIGRATIONS.load(Ordering::Relaxed)
}

#[allow(dead_code)] // §9.12 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn migrated_bytes() -> u64 {
    MIGRATED_BYTES.load(Ordering::Relaxed)
}

pub(super) fn view(tensor: &Tensor) -> TensorView<'_> {
    TensorView {
        dtype: tensor.dtype(),
        shape: tensor.shape_ref(),
        bytes: tensor.bytes(),
    }
}

/// Place, make room, allocate, dispatch, and account one op — returning the
/// frozen output tensor. Byte-for-byte the behavior the arc built up:
/// P1 placement + trace, P2 room consult + eviction + OOM backstops (alloc and
/// dispatch legs), P3 retain flags, P4b need-note + daemon kick, the
/// BUILD-4b-1 transparent accel_cpu fallback, and the §9.5 recency touches.
#[expect(
    clippy::too_many_arguments,
    reason = "dispatch glue keeps the placement/allocation/accounting inputs explicit"
)]
pub(super) fn allocate_result(
    target: &Target,
    values: &mut [Option<Arc<Tensor>>],
    la: &OpLookahead,
    operands: &[Arc<Tensor>],
    op: OpKind,
    shape: Shape,
    dtype: Dtype,
    inputs: &[TensorView<'_>],
) -> Result<Arc<Tensor>, InferenceError> {
    // Snapshot operand residency for the placement decision + the migration metric.
    let residency: Vec<OperandResidency> = operands
        .iter()
        .map(|t| OperandResidency {
            device: t.device(),
            bytes: t.byte_len() as u64,
        })
        .collect();

    let fallback = placement::cpu_fallback_device();

    // §9.9: choose the run device + driver; trace the decision.
    let placed_driver;
    let (mut run_device, mut driver): (Device, &dyn AccelDriver) = match target {
        Target::Fixed(driver) => (driver.descriptor().device, *driver),
        Target::Placed { .. } => {
            let (device, placement) = placement::decide_placement(op, &residency)
                .ok_or(InferenceError::Accel(AccelError::NoDevice))?;
            let principal = accounting::current_principal(device);
            placement::trace_decision(op, principal, device, placement);
            placed_driver = accel::get_device_by_id(device)
                .ok_or(InferenceError::Accel(AccelError::NoDevice))?;
            (device, placed_driver.as_ref())
        }
    };

    // §9.10/§9.11: placement chose a capacity-bounded accelerator — make room
    // (LRU victims to CPU, predicted-hot retained) or fall back to the host.
    // `Fixed` never consults; capacity-free devices return Ready immediately.
    let room_driver;
    if matches!(target, Target::Placed { .. }) && Some(run_device) != fallback {
        let output_len = shape.byte_len(dtype).map_err(InferenceError::Tensor)? as u64;
        let operand_total = residency
            .iter()
            .fold(0u64, |acc, o| acc.saturating_add(o.bytes));
        let need = evict::room_needed(driver.descriptor().memory, operand_total, output_len);
        // §9.12 (P4b): note the per-device need (the watermark H_dev) and wake
        // the daemon below low — the synchronous path proceeds regardless.
        super::residency_daemon::note_need(run_device, need);
        if let Some(capacity) = driver.capacity_bytes() {
            let free = capacity.saturating_sub(accounting::device_resident_total(run_device));
            if free < super::residency_daemon::need_hint(run_device) {
                super::residency_daemon::kick();
            }
        }
        let pinned: Vec<u64> = operands.iter().map(|t| t.id()).collect();
        if evict::ensure_room(values, driver, &pinned, need, &la.hot)
            == evict::RoomOutcome::CpuFallback
        {
            room_driver = accel::default_driver().ok_or(InferenceError::UnsupportedOp)?;
            run_device = room_driver.descriptor().device;
            driver = room_driver.as_ref();
        }
    }

    // Output residency: placed ops keep intermediates on the chosen
    // accelerator (thrash avoidance); host fallback yields `Device::Cpu`.
    let mut output_device = match target {
        Target::Fixed(_) => Device::Cpu,
        Target::Placed { .. } if Some(run_device) == fallback => Device::Cpu,
        Target::Placed { .. } => run_device,
    };

    // §9.10 OOM backstop, alloc leg: a placed op never fails on a device OOM
    // the room arithmetic could not see (SMP race, alignment) — it reroutes.
    let oom_driver;
    let mut tensor = match Tensor::allocate_writable_on(output_device, shape, dtype) {
        Ok(tensor) => tensor,
        Err(TensorError::OutOfMemory)
            if matches!(target, Target::Placed { .. })
                && matches!(output_device, Device::Accel(_)) =>
        {
            oom_driver = accel::default_driver().ok_or(InferenceError::UnsupportedOp)?;
            run_device = oom_driver.descriptor().device;
            driver = oom_driver.as_ref();
            output_device = Device::Cpu;
            Tensor::allocate_writable_on(Device::Cpu, shape, dtype)
                .map_err(InferenceError::Tensor)?
        }
        Err(err) => return Err(InferenceError::Tensor(err)),
    };

    // The migration this dispatch implies — the P1 gate's metric.
    let (mig_count, mig_bytes) = placement::migration_to(&residency, run_device);
    MIGRATIONS.fetch_add(mig_count, Ordering::Relaxed);
    MIGRATED_BYTES.fetch_add(mig_bytes, Ordering::Relaxed);
    {
        let output = tensor.bytes_mut().map_err(InferenceError::Tensor)?;
        let mut batch = OpBatch {
            op,
            inputs,
            output_shape: &shape,
            output,
        };
        // §9.5: dispatch through the common accounting wrapper.
        match accel::accounting::dispatch_and_wait(driver, &mut batch, None) {
            Ok(()) => {}
            Err(AccelError::Unsupported) => {
                // BUILD-4b-1: transparent accel_cpu fallback — byte-identical,
                // never failed; accounted on the device that ran it.
                let cpu = accel::default_driver().ok_or(InferenceError::UnsupportedOp)?;
                accel::accounting::dispatch_and_wait(cpu.as_ref(), &mut batch, None)
                    .map_err(InferenceError::Accel)?;
            }
            Err(AccelError::OutOfMemory) if matches!(target, Target::Placed { .. }) => {
                // §9.10 backstop, dispatch leg: arena staging OOM completes on
                // accel_cpu — same output buffer, byte-identical.
                let cpu = accel::default_driver().ok_or(InferenceError::UnsupportedOp)?;
                accel::accounting::dispatch_and_wait(cpu.as_ref(), &mut batch, None)
                    .map_err(InferenceError::Accel)?;
            }
            Err(err) => return Err(InferenceError::Accel(err)),
        }
    }

    // §9.5 recency: an op uses its accelerator-resident operands and writes
    // its output; the output touch lands last (MRU).
    for operand in operands {
        if matches!(operand.device(), Device::Accel(_)) {
            accounting::recency_touch(operand.device(), operand.id());
        }
    }
    if matches!(output_device, Device::Accel(_)) {
        accounting::recency_touch(output_device, tensor.id());
    }

    tensor.freeze();
    Ok(Arc::new(tensor))
}
