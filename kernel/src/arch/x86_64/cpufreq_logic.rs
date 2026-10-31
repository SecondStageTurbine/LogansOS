// SPDX-License-Identifier: MPL-2.0
//! CPU frequency evidence and HWP, the pure part (host-tested in
//! `tools/ai-ops-tests`).
//!
//! Arthur answer latency, round 2 (silicon): is the Zenbook running at its
//! minimum P-state? The kernel never touches power management, so a CPU whose
//! firmware leaves Hardware P-states (HWP) disabled and IA32_PERF_CTL low runs
//! every forward there. This decodes what CPUID and the MSRs say, and turns
//! APERF/MPERF deltas into an effective frequency:
//!
//! - CPUID.06H:EAX[7] HWP, EAX[10] HWP energy-performance preference;
//!   CPUID.06H:ECX[0] APERF/MPERF (hardware coordination feedback);
//!   CPUID.01H:ECX[7] Enhanced SpeedStep (IA32_PERF_STATUS / IA32_PERF_CTL).
//! - IA32_HWP_CAPABILITIES (0x771): highest [7:0], guaranteed [15:8], most
//!   efficient [23:16], lowest [31:24] performance.
//! - IA32_HWP_REQUEST (0x774): minimum [7:0], maximum [15:8], desired [23:16],
//!   energy-performance preference [31:24] (0 = performance).
//! - MPERF counts at the TSC rate while the CPU is in C0 and APERF at the
//!   actual clock, so `tsc_hz * dAPERF / dMPERF` is the average clock while
//!   running (Intel SDM vol. 3B, 15.2).

/// The power-management features CPUID reports.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct PowerCaps {
    pub aperf_mperf: bool,
    pub hwp: bool,
    pub hwp_epp: bool,
    pub eist: bool,
}

impl PowerCaps {
    pub const fn from_cpuid(leaf6_eax: u32, leaf6_ecx: u32, leaf1_ecx: u32) -> Self {
        Self {
            aperf_mperf: leaf6_ecx & 1 != 0,
            hwp: leaf6_eax & (1 << 7) != 0,
            hwp_epp: leaf6_eax & (1 << 10) != 0,
            eist: leaf1_ecx & (1 << 7) != 0,
        }
    }
}

/// IA32_HWP_CAPABILITIES, decoded.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct HwpCaps {
    pub highest: u8,
    pub guaranteed: u8,
    pub efficient: u8,
    pub lowest: u8,
}

impl HwpCaps {
    pub const fn from_msr(raw: u64) -> Self {
        Self {
            highest: raw as u8,
            guaranteed: (raw >> 8) as u8,
            efficient: (raw >> 16) as u8,
            lowest: (raw >> 24) as u8,
        }
    }
}

/// The IA32_HWP_REQUEST `hwp-performance` writes: the full range the CPU
/// reports (minimum = lowest, maximum = highest), no desired level (the
/// hardware chooses), energy-performance preference 0 (performance). Activity
/// window and package control 0. The hardware still clocks down when idle or
/// hot; this only removes the cap and asks it to favour speed.
pub const fn hwp_performance_request(caps: HwpCaps) -> u64 {
    (caps.lowest as u64) | ((caps.highest as u64) << 8)
}

/// Average clock in MHz over an interval: `tsc_hz * d_aperf / d_mperf`. None
/// when nothing ran (`d_mperf == 0`) or the TSC rate is unknown.
pub fn effective_mhz(tsc_hz: u64, d_aperf: u64, d_mperf: u64) -> Option<u64> {
    if d_mperf == 0 || tsc_hz == 0 {
        return None;
    }
    let hz = (tsc_hz as u128) * (d_aperf as u128) / (d_mperf as u128);
    Some((hz / 1_000_000) as u64)
}
