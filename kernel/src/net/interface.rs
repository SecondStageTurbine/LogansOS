// SPDX-License-Identifier: MPL-2.0
//! Dual-family interface view and source selection for Phase 4 BUILD-3, extended in
//! Phase 7.5 CB-3 (ISC-3) to select a source across **all UP interfaces** instead of
//! the hardcoded interface 0.
//!
//! With `interface-identity` **off**, [`select_source`] and
//! [`egress_interface_for_source`] both collapse to interface 0 — bit-for-bit the
//! pre-identity `preferred_address(0)` / `get_device(0)` behavior (the ISC-5
//! kill-switch; the disabled-equivalence is proven exhaustively by the host KAT over
//! [`identity_logic::chosen_interface`]).

extern crate alloc;

use alloc::vec::Vec;

use crate::net::identity;
use crate::net::identity_logic::{self, Candidate};
use crate::net::ipv4;
use crate::net::ipv6;
use crate::net::socket::{AddressFamily, SocketAddr};

/// Select the local source address for a destination, picking across UP interfaces.
///
/// Disabled (`interface-identity` off): exactly `preferred_address(0)` /
/// `preferred_global_address(0)` — bit-for-bit the current single-device path.
/// Enabled: the source of the interface chosen by [`identity_logic::chosen_interface`]
/// (the lowest-index UP interface with a usable source for the family).
pub fn select_source(dst: SocketAddr) -> Option<SocketAddr> {
    let iface = chosen_for_family(dst.family);
    match dst.family {
        AddressFamily::Inet4 => ipv4::preferred_address(iface).map(|addr| {
            let mut full = [0u8; 16];
            full[..4].copy_from_slice(&addr);
            SocketAddr {
                family: AddressFamily::Inet4,
                port: dst.port,
                addr: full,
            }
        }),
        AddressFamily::Inet6 => ipv6::preferred_global_address(iface).map(|addr| SocketAddr {
            family: AddressFamily::Inet6,
            port: dst.port,
            addr,
        }),
    }
}

/// The egress interface index for a frame with source address `src` — i.e. the
/// interface that owns `src`. The egress path uses this to pick the device
/// (`get_device(i)`) and the ARP/NDP `interface_id`, replacing the hardcoded
/// `get_device(0)` / `submit_or_queue(0, …)`.
///
/// Disabled: always `0` (so every egress site is bit-for-bit pre-identity). Enabled:
/// the interface whose family-preferred address equals `src`, else `0`. Because each
/// interface owns a distinct source address, this resolves to exactly the interface
/// [`select_source`] chose.
pub fn egress_interface_for_source(src: &SocketAddr) -> u32 {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return 0;
    }
    for (id, _up) in identity::snapshot_links() {
        match src.family {
            AddressFamily::Inet4 => {
                if let Some(addr) = ipv4::preferred_address(id)
                    && addr == src.addr[..4]
                {
                    return id;
                }
            }
            AddressFamily::Inet6 => {
                if let Some(addr) = ipv6::preferred_global_address(id)
                    && addr == src.addr
                {
                    return id;
                }
            }
        }
    }
    0
}

/// Choose the source interface for a destination family across UP interfaces.
/// Disabled ⇒ `0`. Enabled ⇒ [`identity_logic::chosen_interface`] over the registry's
/// UP interfaces that have a preferred source of this family.
///
/// The registry lock is taken (and dropped) inside [`identity::snapshot_links`]
/// before the per-interface ipv4/ipv6 table locks are touched, so no lock nests under
/// the registry lock.
fn chosen_for_family(family: AddressFamily) -> u32 {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return 0;
    }
    let links = identity::snapshot_links();
    let candidates: Vec<Candidate> = links
        .iter()
        .map(|&(id, up)| {
            let has_source = match family {
                AddressFamily::Inet4 => ipv4::preferred_address(id).is_some(),
                AddressFamily::Inet6 => ipv6::preferred_global_address(id).is_some(),
            };
            Candidate::new(id, up, has_source)
        })
        .collect();
    identity_logic::chosen_interface(&candidates, true)
}
