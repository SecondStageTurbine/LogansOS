// SPDX-License-Identifier: MPL-2.0
//! F11 S4 (setup-menu THINK §3.3): the snapshot behind `SYS_NET_IF_STATUS`.
//!
//! Read-only: nothing here writes kernel state. Each source is read under its
//! own lock, taken and released alone, so no lock is held while another is
//! taken (no new lock order):
//! 1. the DHCP client (`dhcpv4::bound_lease`, one hold: lease, interface and
//!    clock agree);
//! 2. the identity registry (`identity::snapshot_links`), or the device
//!    registry's count when identity is off;
//! 3. the device registry (`get_device`, an `Arc` clone), then the driver's
//!    own fields with no lock (`identity()`, `mac_address()`, `link_state()`:
//!    an atomic or a register read, never a wait on the driver's ISR);
//! 4. the IPv4 table (`ipv4::preferred_address`), only without a lease.
//!
//! Two sources read at different instants can disagree for one call (a lease
//! bound just after step 1): the record then shows no lease, and the next
//! call shows it. The lease is used only when its interface is the chosen one.

use alloc::vec::Vec;

use super::if_status_logic::{self as logic, Iface, NetIfStatusV1};
use crate::drivers::net as devices;
use crate::net::{LinkState, dhcpv4, identity, identity_logic, ipv4};

/// The default interface's record ([`logic::choose`], [`logic::build`]).
pub fn snapshot() -> NetIfStatusV1 {
    let lease = dhcpv4::bound_lease();
    let links: Vec<(u32, bool)> = if identity_logic::INTERFACE_IDENTITY_ENABLED {
        identity::snapshot_links()
    } else {
        (0..devices::device_count() as u32)
            .map(|i| (i, false))
            .collect()
    };
    let Some(ifindex) = logic::choose(&links, lease.map(|l| l.ifindex)) else {
        return logic::build(None, None, None);
    };
    let Some(device) = devices::get_device(ifindex as usize) else {
        return logic::build(None, None, None);
    };
    let carrier = device.link_state() == LinkState::Up;
    let link_up = if identity_logic::INTERFACE_IDENTITY_ENABLED {
        links.iter().any(|&(i, up)| i == ifindex && up)
    } else {
        carrier
    };
    let iface = Iface {
        ifindex,
        driver: device.identity().driver,
        mac: device.mac_address(),
        link_up,
        carrier,
    };
    let address = match lease {
        Some(l) if l.ifindex == ifindex => None,
        _ => ipv4::preferred_address(ifindex),
    };
    logic::build(Some(iface), lease, address)
}
