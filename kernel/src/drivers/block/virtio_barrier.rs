// SPDX-License-Identifier: MPL-2.0
//! Human-auditable virtio split-ring ordering barriers.

use core::sync::atomic::{Ordering, fence};

/// Step-3 barrier: descriptor-table writes must be visible before the driver
/// publishes the next available-ring index.
#[inline(always)]
pub fn publish_descriptors() {
    fence(Ordering::Release);
}

/// Step-5 barrier: the available-ring index publish must be visible before
/// reading notification-suppression state.
#[inline(always)]
pub fn before_notification_check() {
    fence(Ordering::SeqCst);
}
