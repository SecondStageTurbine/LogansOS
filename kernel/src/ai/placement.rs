// SPDX-License-Identifier: MPL-2.0
//! Phase 7 BUILD-P1: residency-aware op placement — the kernel glue.
//!
//! The executor consults this before dispatching each op: from inputs only the
//! kernel sees globally — the operands' residency (`DeviceId` per tensor), the
//! AccelDriver registry (which devices cover this op), and the §9.5 accounting
//! table (the principal's per-device dispatch quota, §9.6) — it chooses *where the
//! op runs* and **logs the decision**. The pure decision logic lives in
//! [`placement_policy`] (host-testable, kernel-free); this file snapshots the live
//! kernel state into that policy's inputs, converts the result back to a `Device`,
//! and emits the `placement:` trace.
//!
//! ANSPEC-001 §9.9. Result-invariant (ISC-4): placement changes the run device,
//! never the bytes computed. It does **not** move tensors proactively (that is
//! BUILD-P2/P3) — it chooses the device that *minimizes* the migration a dispatch
//! implies, and the executor measures that migration.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::ai::ops::OpKind;
use crate::ai::tensor::Device;
use crate::drivers::accel::{self, accounting};
use crate::sched::process::ProcessId;
use crate::serial_println;

#[path = "placement_policy.rs"]
mod placement_policy;
pub use placement_policy::{DeviceCandidate, Placement, PlacementReason, decide};

/// One operand's residency, as the executor reads it off the operand tensor.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct OperandResidency {
    pub device: Device,
    pub bytes: u64,
}

/// The host-memory fallback device (accel_cpu) — the safety floor and the host
/// residency class. `None` only before the accel registry is initialized.
pub fn cpu_fallback_device() -> Option<Device> {
    accel::find_device_by_driver("accel_cpu").map(|(_, d)| d.descriptor().device)
}

/// Fold host residency: a `Device::Cpu` tensor lives in host RAM — the same memory
/// class as accel_cpu, with no migration between them — so it counts toward the
/// host fallback candidate.
fn residency_device(dev: Device, fallback: Device) -> Device {
    match dev {
        Device::Cpu => fallback,
        other => other,
    }
}

fn device_from_abi(abi: u32, fallback: Device) -> Device {
    if abi & Device::ACCEL_ABI_BASE != 0 {
        Device::Accel(abi & !Device::ACCEL_ABI_BASE)
    } else {
        // CPU_ABI (or anything host-class) resolves to the registered host fallback.
        fallback
    }
}

/// Decide where `op` runs given its operands' residency. Snapshots the registry
/// (device coverage) and §9.5 accounting (per-principal, per-device quota), runs
/// the pure policy, and returns the chosen `Device` plus the decision record.
/// `None` only if the accel registry has no host fallback (never, post-init).
pub fn decide_placement(op: OpKind, operands: &[OperandResidency]) -> Option<(Device, Placement)> {
    let fallback = cpu_fallback_device()?;

    // Aggregate operand bytes per residency device (host-folded), and the total.
    let mut resident: BTreeMap<u32, u64> = BTreeMap::new();
    let mut total: u64 = 0;
    for o in operands {
        let dev = residency_device(o.device, fallback);
        let entry = resident.entry(dev.as_abi()).or_insert(0);
        *entry = entry.saturating_add(o.bytes);
        total = total.saturating_add(o.bytes);
    }

    // One candidate per registered device: its resident operand bytes, op coverage,
    // and the principal's quota state there (§9.5/§9.6, read — never recomputed).
    let mut candidates: Vec<DeviceCandidate> = Vec::new();
    for idx in 0..accel::device_count() {
        let Some(driver) = accel::get_device(idx) else {
            continue;
        };
        let device = driver.descriptor().device;
        let abi = device.as_abi();
        let principal = accounting::current_principal(device);
        let over_quota = accounting::usage(device, principal).dispatch_count
            >= crate::ai::admission::DISPATCH_QUOTA;
        candidates.push(DeviceCandidate {
            device_abi: abi,
            resident_bytes: resident.get(&abi).copied().unwrap_or(0),
            covers_op: driver.supported_ops().contains(&op),
            over_quota,
            is_cpu_fallback: device == fallback,
        });
    }

    let placement = decide(&candidates, total);
    Some((device_from_abi(placement.device_abi, fallback), placement))
}

/// Count the operands (and their bytes) **not** resident on `run_device`,
/// host-folding `Device::Cpu` into the fallback — the migration a dispatch to
/// `run_device` implies. One rule measures both the policy path and a fixed-device
/// baseline, so the gate's comparison is apples-to-apples.
pub fn migration_to(operands: &[OperandResidency], run_device: Device) -> (u64, u64) {
    let fallback = cpu_fallback_device().unwrap_or(Device::Cpu);
    let run = residency_device(run_device, fallback);
    let mut count = 0u64;
    let mut bytes = 0u64;
    for o in operands {
        if residency_device(o.device, fallback) != run {
            count += 1;
            bytes = bytes.saturating_add(o.bytes);
        }
    }
    (count, bytes)
}

fn reason_label(reason: PlacementReason) -> &'static str {
    match reason {
        PlacementReason::OperandsResident => "operands-resident",
        PlacementReason::MinMigration => "min-migration",
        PlacementReason::CpuFallback => "cpu-fallback",
        PlacementReason::OverQuota => "over-quota",
    }
}

/// Emit the decision trace (ISC-3) — the governance line: the kernel reasoning
/// about placement the application did not request.
pub fn trace_decision(op: OpKind, principal: ProcessId, chosen: Device, placement: Placement) {
    let fallback = cpu_fallback_device();
    let reason = reason_label(placement.reason);
    if Some(chosen) == fallback {
        serial_println!(
            "placement: op={:?} principal=PID{} → device=cpu reason={} migrated={}",
            op,
            principal.0,
            reason,
            placement.migrated_bytes,
        );
    } else {
        serial_println!(
            "placement: op={:?} principal=PID{} → device={:?} reason={} migrated={}",
            op,
            principal.0,
            chosen,
            reason,
            placement.migrated_bytes,
        );
    }
    crate::log::flush();
}
