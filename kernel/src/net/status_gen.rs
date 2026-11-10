// SPDX-License-Identifier: MPL-2.0
//! F11 S4 (Janus, #100 relay 382): the network-status generation, so
//! `SYS_NET_IF_STATUS` returns one coherent record.
//!
//! Every writer of a source the record reads calls [`bump`] **inside its own
//! lock, after the write**: the DHCP client (its state, lease or interface),
//! the device registry (a registration), the identity registry (a
//! registration, or a link-state change) and the IPv4 table (an address
//! added, or one expired). The reader takes the same locks, so a write whose
//! data it saw had bumped before the reader's read, and any write between the
//! reader's two loads of the generation changes it (`if_status_logic::
//! read_consistent` then retries). The driver's own carrier has no lock: the
//! reader takes every carrier first, right after its first load, so the record
//! is the state at that instant.
//!
//! Without `net-info` nothing reads it, and [`bump`] is empty: the writers are
//! as they were (the kill-switch).

#[cfg(feature = "net-info")]
use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "net-info")]
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// A source of the network status changed. Call it inside the writer's lock,
/// after the write.
#[inline]
pub fn bump() {
    #[cfg(feature = "net-info")]
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// The generation now.
#[cfg(feature = "net-info")]
pub fn read() -> u64 {
    GENERATION.load(Ordering::SeqCst)
}
