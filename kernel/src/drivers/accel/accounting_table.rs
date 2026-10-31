// SPDX-License-Identifier: MPL-2.0
//! Pure per-`(device, principal)` accelerator-accounting arithmetic.
//!
//! Phase 7 BUILD-1 — the measurement floor. This file has **no kernel
//! dependencies** (only `alloc`) so the host suite (`tools/ai-ops-tests`) can
//! path-`mod` it and unit-test the increment/decrement/attribution edge cases.
//! The kernel glue — the global table, principal resolution, the dispatch
//! wrapper, the trace — lives in `accounting.rs`. **MEASUREMENT ONLY**
//! (ANSPEC-001 §9.5): no scheduling, quota, or sharing decision lives here.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Per-`(device, principal)` usage. `resident_bytes` and `dispatch_count` are
/// **exact** (deterministic, asserted exactly). `dispatch_time_ticks` is real but
/// **non-deterministic** (100 Hz timer ticks accumulated submit→completion);
/// asserted only as monotonic / non-negative, never exact.
#[derive(Copy, Clone, Default, Eq, PartialEq, Debug)]
pub struct AccelUsage {
    pub resident_bytes: u64,
    pub dispatch_count: u64,
    pub dispatch_time_ticks: u64,
}

/// Map key: `(device ABI, principal pid)`. `Device` is not `Ord`, but its `u32`
/// ABI (`Device::as_abi`) is, and `ProcessId` is a `u32` — so the pair is `Ord`
/// and keeps the table a tidy `BTreeMap` without a custom key type.
pub type AcctKey = (u32, u32);

/// Per-device residency across **all** principals (Phase 7 BUILD-P2): the
/// current total and its high-water mark. `current` is the eviction policy's
/// `R` (against the device's capacity `C`); `peak` is the gate's post-hoc proof
/// that `R ≤ C` held *throughout* a run, not just at the moment it was read.
#[derive(Copy, Clone, Default, Eq, PartialEq, Debug)]
pub struct DeviceResidency {
    pub current: u64,
    pub peak: u64,
}

/// The accounting map. Bounded by the live `(device, principal)` set; a
/// principal's entries are removed by [`AccountingTable::forget_principal`] on
/// process teardown, so the table does not grow without bound.
#[derive(Default)]
pub struct AccountingTable {
    map: BTreeMap<AcctKey, AccelUsage>,
    /// Per-device totals, maintained in lockstep with `map` (BUILD-P2). Not a
    /// second source of truth: always equal to the sum of the device's
    /// per-principal `resident_bytes` (asserted by the host suite).
    devices: BTreeMap<u32, DeviceResidency>,
}

impl AccountingTable {
    pub const fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            devices: BTreeMap::new(),
        }
    }

    /// Residency: `+bytes` for `(device, principal)`, on device-storage alloc /
    /// migrate-onto-device. Saturating (cannot overflow the counter).
    pub fn add_resident(&mut self, device_abi: u32, pid: u32, bytes: u64) {
        let usage = self.map.entry((device_abi, pid)).or_default();
        usage.resident_bytes = usage.resident_bytes.saturating_add(bytes);
        let device = self.devices.entry(device_abi).or_default();
        device.current = device.current.saturating_add(bytes);
        if device.current > device.peak {
            device.peak = device.current;
        }
    }

    /// Residency: `-bytes` on device-storage free/drop / migrate-off. Saturating
    /// at 0 — residency never underflows, even on a double-free bug (it would be
    /// a visible-as-0 anomaly, not a wraparound to a huge value). The device
    /// total sheds exactly the bytes the per-principal entry actually held
    /// (lockstep rule): an over-subtract that saturates the principal at 0 must
    /// not undercount the device aggregate — `current` is the eviction policy's
    /// `R`, and an undercount would let a room check place onto a full device.
    pub fn sub_resident(&mut self, device_abi: u32, pid: u32, bytes: u64) {
        if let Some(usage) = self.map.get_mut(&(device_abi, pid)) {
            let removed = usage.resident_bytes.min(bytes);
            usage.resident_bytes -= removed;
            if let Some(device) = self.devices.get_mut(&device_abi) {
                device.current = device.current.saturating_sub(removed);
            }
        }
    }

    /// Dispatch completion: `+1` count, `+elapsed` ticks for `(device, principal)`.
    pub fn record_dispatch(&mut self, device_abi: u32, pid: u32, elapsed_ticks: u64) {
        let usage = self.map.entry((device_abi, pid)).or_default();
        usage.dispatch_count = usage.dispatch_count.saturating_add(1);
        usage.dispatch_time_ticks = usage.dispatch_time_ticks.saturating_add(elapsed_ticks);
    }

    /// Read the usage for `(device, principal)` — `AccelUsage::default()` (all 0)
    /// if never seen. Used by the correctness oracles.
    pub fn usage(&self, device_abi: u32, pid: u32) -> AccelUsage {
        self.map
            .get(&(device_abi, pid))
            .copied()
            .unwrap_or_default()
    }

    /// Drop every entry for a principal (call on process teardown). Keeps the
    /// table bounded by the live-principal set. Device `current` totals shed the
    /// principal's resident bytes; `peak` is history and never decreases.
    pub fn forget_principal(&mut self, pid: u32) {
        for ((device_abi, entry_pid), usage) in &self.map {
            if *entry_pid == pid
                && let Some(device) = self.devices.get_mut(device_abi)
            {
                device.current = device.current.saturating_sub(usage.resident_bytes);
            }
        }
        self.map.retain(|(_, entry_pid), _| *entry_pid != pid);
    }

    /// Per-device residency across all principals (BUILD-P2) — all-zero if the
    /// device never held tensor storage.
    pub fn device_residency(&self, device_abi: u32) -> DeviceResidency {
        self.devices.get(&device_abi).copied().unwrap_or_default()
    }

    /// Snapshot all entries (for the governance trace).
    pub fn entries(&self) -> Vec<(AcctKey, AccelUsage)> {
        self.map.iter().map(|(key, usage)| (*key, *usage)).collect()
    }

    /// Number of live `(device, principal)` entries (boundedness checks/tests).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
