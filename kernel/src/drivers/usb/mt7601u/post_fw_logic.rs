// SPDX-License-Identifier: MPL-2.0
//! Pure post-firmware WPDMA decision logic.
//!
//! Linux v6.6 `mt7601u_init_hardware()` polls `WPDMA_GLO_CFG` after
//! `mt7601u_mcu_init()` and before the second ASIC-ready check. Our register
//! transport returns `0xffff_ffff` when the USB vendor read exhausts its
//! retries. Treating that sentinel as merely "busy" multiplies the outer poll
//! by the nested EP0 retry/wait budgets, which turned TEST 3 into a pre-sink
//! multi-minute stall.
//!
//! This file expresses only the device-independent classification; the
//! device-touching loop and breadcrumbs remain in `mod.rs`. `ai-ops-tests`
//! imports this module directly as the falsifiable all-ones/busy/idle oracle.
//! Linux GPL source was consulted only for sequencing and register semantics;
//! no implementation code is copied.

#![allow(dead_code)] // host-oracle'd pure core; kernel bring-up is the consumer

pub const DEVICE_UNRESPONSIVE: u32 = 0xffff_ffff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WpdmaStep {
    /// The composed operation consumed its one non-replenishing budget.
    OperationBudgetExhausted,
    /// The USB register transport failed persistently. Stop the outer poll now.
    DeviceUnresponsive,
    /// Neither TX nor RX DMA reports busy.
    Idle,
    /// The device answered, but at least one DMA engine remains busy.
    Busy,
}

/// Classify one `WPDMA_GLO_CFG` observation.
///
/// Sentinel detection deliberately precedes the mask comparison. This makes
/// the distinction robust even if a future caller waits for a bit that is set
/// in `0xffff_ffff`.
pub const fn classify_wpdma(value: u32, busy_mask: u32) -> WpdmaStep {
    if value == DEVICE_UNRESPONSIVE {
        WpdmaStep::DeviceUnresponsive
    } else if value & busy_mask == 0 {
        WpdmaStep::Idle
    } else {
        WpdmaStep::Busy
    }
}
