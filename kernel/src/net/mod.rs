// SPDX-License-Identifier: MPL-2.0
//! Phase 4 BUILD-1a network substrate.

pub mod arp;
pub mod device;
pub mod dhcpv4;
pub mod dns;
#[cfg(feature = "net-tick")]
pub mod egress_logic;
pub mod happy_eyeballs;
pub mod icmpv6;
/// Phase 7.5 CB-3: the runtime `InterfaceId` registry (ISC-1/ISC-2 — runtime names
/// + hotplug link-state) above `NetDevice`. Inert when `interface-identity` is off.
pub mod identity;
/// Phase 7.5 CB-3: the kernel-free interface-identity kill-switch + selection logic
/// (ISC-3/ISC-5), host-KAT'd against the disabled-equivalence baseline.
pub mod identity_logic;
#[cfg(feature = "net-info")]
pub mod if_status;
/// F11 S4: `SYS_NET_IF_STATUS`, the record and its rules (kernel-free; the host
/// byte oracle includes this file) and the snapshot. Off ⇒ not compiled, and the
/// number is `ENOSYS`.
#[cfg(feature = "net-info")]
pub mod if_status_logic;
pub mod interface;
pub mod ipv4;
pub mod ipv6;
pub mod ndp;
#[cfg(feature = "net-tick")]
pub mod periodic;
#[cfg(feature = "wire-rx")]
pub mod rx;
#[cfg(feature = "wire-rx")]
pub mod rx_logic;
pub mod smoltcp_wrap;
pub mod socket;
pub mod tcp;
pub mod tls_record;
/// Phase 7.5 BUILD-W0: the WPA2/3 supplicant crypto library. Compiled only
/// under the `wifi` feature (the kill-switch); pure host-tested crypto, no glue.
#[cfg(feature = "wifi")]
pub mod wpa;

/// Phase 7.5 BUILD-W2: the in-kernel 802.11 STA supplicant (MLME + handshake
/// wiring + CCMP + NetDevice seam). Same `wifi` kill-switch as `wpa`.
#[cfg(feature = "wifi")]
pub mod wifi;

pub use device::{LinkState, NetDevice, NetDeviceIdentity, NetError, NetResult, RxFrame, TxFrame};

/// How many handle-table entries name a socket, TCP connection or listener.
/// `HandleTable` install and `Detached` release are the only callers, so it
/// counts table entries, not the `Arc`s a lookup clones. The release that
/// takes it to zero closes the object: a process that exits, is killed or
/// has the handle revoked no longer leaves its port bound.
#[derive(Default)]
pub struct HandleCount(core::sync::atomic::AtomicUsize);

impl HandleCount {
    pub fn retain(&self) {
        self.0.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
    }

    /// True for the release that took the count to zero.
    pub fn release(&self) -> bool {
        let prev = self.0.fetch_sub(1, core::sync::atomic::Ordering::AcqRel);
        debug_assert!(prev > 0, "released a socket handle count that was zero");
        prev == 1
    }

    pub fn get(&self) -> usize {
        self.0.load(core::sync::atomic::Ordering::Acquire)
    }
}

pub fn tick() {
    ipv6::tick();
    ipv4::tick();
    arp::tick();
    ndp::tick();
    dhcpv4::tick();
    dns::tick();
    tcp::tick();
    // ISC-2: keep the InterfaceId registry's link-state current from the generic
    // `NetDevice::link_state()` (so hotplug — assoc/disassoc, attach/detach, already
    // maintained inside each driver — reaches the registry without editing any
    // driver). Inert when `interface-identity` is off.
    identity::refresh_link_states();
}
