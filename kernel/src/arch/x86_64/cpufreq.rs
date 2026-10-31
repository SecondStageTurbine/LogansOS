// SPDX-License-Identifier: MPL-2.0
//! CPU frequency evidence (feature `prefill-perf`) and the HWP performance
//! request (feature `hwp-performance`, in image A since the Zenbook A/B of
//! 2026-10-25: A6 against A6-hwp, about 3.2x on Arthur's answer). The
//! decoding is in `cpufreq_logic.rs`.
//!
//! Every MSR here is read or written only after CPUID reports it, so a CPU or
//! hypervisor without it never takes a `#GP`. Nothing here takes a lock, and
//! nothing is written unless `hwp-performance` is built in.

use core::sync::atomic::{AtomicU32, Ordering};

use x86_64::registers::model_specific::Msr;

#[cfg(feature = "hwp-performance")]
use super::cpufreq_logic::HwpCaps;
pub use super::cpufreq_logic::PowerCaps;
#[cfg(feature = "prefill-perf")]
pub use super::cpufreq_logic::effective_mhz;

const IA32_MPERF: u32 = 0xE7;
const IA32_APERF: u32 = 0xE8;
const IA32_PERF_STATUS: u32 = 0x198;
const IA32_PM_ENABLE: u32 = 0x770;
const IA32_HWP_CAPABILITIES: u32 = 0x771;
const IA32_HWP_REQUEST: u32 = 0x774;

/// This CPU's power-management features.
pub fn caps() -> PowerCaps {
    // Leaf 0 gives the maximum basic leaf; leaves 1 and 6 exist when it is
    // at least 6.
    let max = core::arch::x86_64::__cpuid(0).eax;
    if max < 6 {
        return PowerCaps::default();
    }
    let leaf1 = core::arch::x86_64::__cpuid(1);
    let leaf6 = core::arch::x86_64::__cpuid(6);
    PowerCaps::from_cpuid(leaf6.eax, leaf6.ecx, leaf1.ecx)
}

fn rdmsr(msr: u32) -> u64 {
    // SAFETY: every caller checked the CPUID bit that makes `msr` exist.
    unsafe { Msr::new(msr).read() }
}

/// This CPU's (APERF, MPERF), when it has them.
#[inline]
pub fn aperf_mperf(caps: PowerCaps) -> Option<(u64, u64)> {
    caps.aperf_mperf
        .then(|| (rdmsr(IA32_APERF), rdmsr(IA32_MPERF)))
}

/// What this CPU's HWP registers say.
#[derive(Copy, Clone, Debug, Default)]
pub struct HwpSnapshot {
    pub supported: bool,
    pub enabled: bool,
    pub caps: u64,
    pub request: u64,
    /// IA32_PERF_STATUS[15:8], the current ratio (0 if EIST is absent).
    pub ratio: u8,
}

pub fn hwp_snapshot(caps: PowerCaps) -> HwpSnapshot {
    let ratio = if caps.eist {
        (rdmsr(IA32_PERF_STATUS) >> 8) as u8
    } else {
        0
    };
    if !caps.hwp {
        return HwpSnapshot {
            ratio,
            ..HwpSnapshot::default()
        };
    }
    let enabled = rdmsr(IA32_PM_ENABLE) & 1 != 0;
    // The other HWP registers are read only once HWP is enabled: before that,
    // only IA32_PM_ENABLE is certain to be accessible, and a #GP here would be
    // a boot-time fault for a diagnostic.
    let (caps, request) = if enabled {
        (rdmsr(IA32_HWP_CAPABILITIES), rdmsr(IA32_HWP_REQUEST))
    } else {
        (0, 0)
    };
    HwpSnapshot {
        supported: true,
        enabled,
        caps,
        request,
        ratio,
    }
}

/// CPUs on which `hwp-performance` found HWP already enabled by firmware, and
/// on which it enabled it.
static HWP_WAS_ON: AtomicU32 = AtomicU32::new(0);
static HWP_TURNED_ON: AtomicU32 = AtomicU32::new(0);

pub fn hwp_counts() -> (u32, u32) {
    (
        HWP_WAS_ON.load(Ordering::Relaxed),
        HWP_TURNED_ON.load(Ordering::Relaxed),
    )
}

/// `hwp-performance`, on the calling CPU (each CPU's `enable_sse`): enable HWP
/// if the CPU has it, then request its full range with a performance
/// preference. A CPU without HWP is left alone. IA32_PM_ENABLE can only be
/// cleared by a reset, so this is one-way for the boot; the request itself is
/// what the OS normally sets.
#[cfg(feature = "hwp-performance")]
pub fn hwp_performance() {
    let caps = caps();
    if !caps.hwp {
        return;
    }
    if rdmsr(IA32_PM_ENABLE) & 1 != 0 {
        HWP_WAS_ON.fetch_add(1, Ordering::Relaxed);
    } else {
        // SAFETY: CPUID.06H:EAX[7] reports HWP, so IA32_PM_ENABLE exists;
        // bit 0 enables it.
        unsafe { Msr::new(IA32_PM_ENABLE).write(1) };
        HWP_TURNED_ON.fetch_add(1, Ordering::Relaxed);
    }
    let request = super::cpufreq_logic::hwp_performance_request(HwpCaps::from_msr(rdmsr(
        IA32_HWP_CAPABILITIES,
    )));
    // SAFETY: HWP is enabled on this CPU; IA32_HWP_REQUEST exists with it.
    unsafe { Msr::new(IA32_HWP_REQUEST).write(request) };
}
