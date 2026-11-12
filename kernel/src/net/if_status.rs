// SPDX-License-Identifier: MPL-2.0
//! F11 S4 (setup-menu THINK §3.3): the snapshot behind `SYS_NET_IF_STATUS`.
//!
//! Read-only: nothing here writes network state. **One coherent record**
//! (Janus, #100 relay 382): [`snapshot`] reads through
//! `if_status_logic::read_consistent` against the network-status generation
//! (`net::status_gen`), which every writer of a source below bumps inside its
//! lock after the write. One read, in this order:
//! 1. every device's carrier (`get_device`, an `Arc` clone, then the driver's
//!    own `link_state()`: an atomic or a register read, never a wait on the
//!    driver's ISR). These have no lock, so they come first, right after the
//!    first load of the generation: the record is the state at that instant.
//! 2. the DHCP client (`dhcpv4::bound_lease`, one hold: lease, interface and
//!    clock agree);
//! 3. the identity registry (`identity::snapshot_links`), or the device count
//!    when identity is off;
//! 4. the IPv4 table (`ipv4::preferred_address`), only without a lease.
//!
//! Each network subsystem lock is taken alone and released before the next:
//! no new lock order, and `CLIENT`'s hold is as it was.

use alloc::vec::Vec;

use super::if_status_logic::{self as logic, Iface, NetIfStatusV1};
use crate::drivers::net as devices;
use crate::net::{LinkState, dhcpv4, identity, identity_logic, ipv4, status_gen};

/// The default interface's record ([`logic::choose`], [`logic::build`]), one
/// coherent state of the network, or flagged `STALE` if it kept changing.
pub fn snapshot() -> NetIfStatusV1 {
    logic::read_consistent(status_gen::read, read_once)
}

/// One read of every source. `None`: the chosen device isn't in the carrier
/// list (registered mid-read; the generation has changed too).
fn read_once() -> Option<NetIfStatusV1> {
    let carriers: Vec<(u32, bool)> = (0..devices::device_count())
        .filter_map(|i| devices::get_device(i).map(|d| (i as u32, d.link_state() == LinkState::Up)))
        .collect();
    let lease = dhcpv4::bound_lease();
    let links: Vec<(u32, bool)> = if identity_logic::INTERFACE_IDENTITY_ENABLED {
        identity::snapshot_links()
    } else {
        carriers.iter().map(|&(i, _)| (i, false)).collect()
    };
    let Some(ifindex) = logic::choose(&links, lease.map(|l| l.ifindex)) else {
        return Some(logic::build(None, None, None));
    };
    let carrier = carriers.iter().find(|&&(i, _)| i == ifindex)?.1;
    let device = devices::get_device(ifindex as usize)?;
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
    Some(logic::build(Some(iface), lease, address))
}
