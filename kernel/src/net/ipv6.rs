// SPDX-License-Identifier: MPL-2.0
//! IPv6 SLAAC address state for Phase 4 BUILD-2.

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::{Lazy, Mutex};

use crate::net::dns;
use crate::net::icmpv6::{self, PrefixInformation, RouterAdvertisement};
use crate::serial_println;

pub const IPV6_LOOPBACK: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
pub const DEFAULT_RA_TIMEOUT_SECS: u64 = 30;
const DAD_TIMEOUT_TICKS: u64 = 1;
const TEMP_VALID_LIFETIME_SECS: u32 = 86_400;
const TEMP_PREFERRED_LIFETIME_SECS: u32 = 43_200;
/// OT-96: with `wire-rx`, RAs come from the wire, so the table is bounded:
/// addresses, routes, and the prefixes one RA may apply.
const MAX_ADDRESSES: usize = 16;
const MAX_ROUTES: usize = 16;
const MAX_PREFIXES_PER_RA: usize = 4;

static CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);
static INTERFACES: Lazy<Mutex<Vec<InterfaceAddressTable>>> =
    Lazy::new(|| Mutex::new(alloc::vec![InterfaceAddressTable::new(0)]));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Ipv6Error {
    Invalid,
    Unsupported,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum AddressState {
    Tentative,
    Preferred,
    Deprecated,
    Invalid,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct InterfaceAddress {
    pub addr: [u8; 16],
    pub prefix_len: u8,
    pub state: AddressState,
    pub valid_until: u64,
    pub preferred_until: u64,
    pub dad_until: u64,
    pub temporary: bool,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct InterfaceAddressTable {
    pub interface_id: u32,
    pub addresses: Vec<InterfaceAddress>,
    pub routes: Vec<RouteEntry>,
    pub last_ra_tick: Option<u64>,
    pub ra_timeout_tick: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RouteEntry {
    pub prefix: [u8; 16],
    pub prefix_len: u8,
    pub valid_until: u64,
}

impl InterfaceAddressTable {
    pub fn new(interface_id: u32) -> Self {
        Self {
            interface_id,
            addresses: Vec::new(),
            routes: Vec::new(),
            last_ra_tick: None,
            ra_timeout_tick: DEFAULT_RA_TIMEOUT_SECS,
        }
    }

    pub fn apply_ra(&mut self, ra: &RouterAdvertisement) {
        let now = now();
        self.last_ra_tick = Some(now);
        self.ra_timeout_tick = now + DEFAULT_RA_TIMEOUT_SECS;
        let wire = cfg!(feature = "wire-rx");
        let prefixes = if wire {
            MAX_PREFIXES_PER_RA
        } else {
            usize::MAX
        };
        for prefix in ra.prefixes.iter().take(prefixes) {
            self.apply_prefix(*prefix, now);
        }
        for route in &ra.routes {
            let entry = RouteEntry {
                prefix: route.prefix,
                prefix_len: route.prefix_len,
                valid_until: now + route.route_lifetime as u64,
            };
            if !wire {
                self.routes.push(entry);
            } else if let Some(r) = self
                .routes
                .iter_mut()
                .find(|r| r.prefix == entry.prefix && r.prefix_len == entry.prefix_len)
            {
                // OT-96: the same route again refreshes it.
                *r = entry;
            } else if self.routes.len() < MAX_ROUTES {
                self.routes.push(entry);
            }
        }
        dns::configure_from_ra(ra);
        self.refresh(now);
    }

    pub fn refresh(&mut self, now: u64) {
        for addr in &mut self.addresses {
            if addr.state == AddressState::Tentative && now >= addr.dad_until {
                addr.state = AddressState::Preferred;
            }
            if addr.state == AddressState::Preferred && now >= addr.preferred_until {
                addr.state = AddressState::Deprecated;
            }
            if now >= addr.valid_until {
                addr.state = AddressState::Invalid;
            }
        }
        self.addresses
            .retain(|addr| addr.state != AddressState::Invalid);
        self.routes.retain(|route| now < route.valid_until);
    }

    fn apply_prefix(&mut self, prefix: PrefixInformation, now: u64) {
        if !prefix.autonomous()
            || prefix.prefix_len != 64
            || prefix.valid_lifetime == 0
            || prefix.preferred_lifetime > prefix.valid_lifetime
        {
            return;
        }
        let stable = derive_stable_address(prefix.prefix, self.interface_id);
        self.upsert_address(InterfaceAddress {
            addr: stable,
            prefix_len: prefix.prefix_len,
            state: AddressState::Tentative,
            valid_until: now + prefix.valid_lifetime as u64,
            preferred_until: now + prefix.preferred_lifetime as u64,
            dad_until: now + DAD_TIMEOUT_TICKS,
            temporary: false,
        });

        // OT-96: with `wire-rx`, one temporary address per prefix: an RA for
        // a prefix that already has one refreshes it instead of adding another.
        if cfg!(feature = "wire-rx")
            && let Some(t) = self
                .addresses
                .iter_mut()
                .find(|a| a.temporary && a.addr[..8] == prefix.prefix[..8])
        {
            t.valid_until =
                now + core::cmp::min(prefix.valid_lifetime, TEMP_VALID_LIFETIME_SECS) as u64;
            t.preferred_until = now
                + core::cmp::min(prefix.preferred_lifetime, TEMP_PREFERRED_LIFETIME_SECS) as u64;
            return;
        }
        let temporary = derive_temporary_address(prefix.prefix);
        self.upsert_address(InterfaceAddress {
            addr: temporary,
            prefix_len: prefix.prefix_len,
            state: AddressState::Tentative,
            valid_until: now
                + core::cmp::min(prefix.valid_lifetime, TEMP_VALID_LIFETIME_SECS) as u64,
            preferred_until: now
                + core::cmp::min(prefix.preferred_lifetime, TEMP_PREFERRED_LIFETIME_SECS) as u64,
            dad_until: now + DAD_TIMEOUT_TICKS,
            temporary: true,
        });
    }

    fn upsert_address(&mut self, new_addr: InterfaceAddress) {
        if let Some(addr) = self
            .addresses
            .iter_mut()
            .find(|addr| addr.addr == new_addr.addr)
        {
            *addr = new_addr;
        } else if !cfg!(feature = "wire-rx") || self.addresses.len() < MAX_ADDRESSES {
            self.addresses.push(new_addr);
        }
    }
}

pub fn inject_router_advertisement(frame: &[u8]) -> Result<(), Ipv6Error> {
    let ra = icmpv6::parse_router_advertisement(frame).map_err(|e| match e {
        icmpv6::Icmpv6Error::Invalid => Ipv6Error::Invalid,
        icmpv6::Icmpv6Error::Unsupported => Ipv6Error::Unsupported,
    })?;
    let mut tables = INTERFACES.lock();
    let Some(table) = tables.get_mut(0) else {
        return Err(Ipv6Error::Invalid);
    };
    let before = table.addresses.len();
    table.apply_ra(&ra);
    // OT-96: from the wire, print only when the table changed (not once per RA).
    if cfg!(feature = "wire-rx") && table.addresses.len() == before {
        return Ok(());
    }
    serial_println!(
        "[ kernel ] ipv6: ra_injected prefixes={} routes={} addresses={}",
        ra.prefixes.len(),
        ra.routes.len(),
        // Keep the established diagnostic shape; RDNSS is reflected through dns config.
        table.addresses.len()
    );
    Ok(())
}

pub fn tick() {
    let now = CLOCK_TICKS.fetch_add(1, Ordering::AcqRel) + 1;
    let mut tables = INTERFACES.lock();
    for table in tables.iter_mut() {
        table.refresh(now);
    }
}

pub fn snapshot(interface_id: u32) -> Option<InterfaceAddressTable> {
    INTERFACES
        .lock()
        .iter()
        .find(|table| table.interface_id == interface_id)
        .cloned()
}

/// OT-96: this interface's IPv6 addresses (the receive thread's destination
/// filter), at most [`MAX_ADDRESSES`].
#[cfg(feature = "wire-rx")]
pub fn addresses(interface_id: u32) -> alloc::vec::Vec<[u8; 16]> {
    INTERFACES
        .lock()
        .iter()
        .find(|t| t.interface_id == interface_id)
        .map(|t| t.addresses.iter().map(|a| a.addr).collect())
        .unwrap_or_default()
}

pub fn preferred_global_address(interface_id: u32) -> Option<[u8; 16]> {
    snapshot(interface_id).and_then(|table| {
        table
            .addresses
            .iter()
            .find(|addr| addr.state == AddressState::Preferred && !addr.temporary)
            .map(|addr| addr.addr)
    })
}

fn now() -> u64 {
    CLOCK_TICKS.load(Ordering::Acquire)
}

fn derive_stable_address(mut prefix: [u8; 16], interface_id: u32) -> [u8; 16] {
    let iid = 0x0200_0000_0000_0000u64 | interface_id as u64;
    prefix[8..16].copy_from_slice(&iid.to_be_bytes());
    prefix
}

fn derive_temporary_address(mut prefix: [u8; 16]) -> [u8; 16] {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::AcqRel);
    let mut input = [0u8; 24];
    input[..16].copy_from_slice(&prefix);
    input[16..].copy_from_slice(&counter.to_be_bytes());
    let hash = blake3::hash(&input);
    prefix[8..16].copy_from_slice(&hash.as_bytes()[..8]);
    prefix[8] &= 0xfd;
    prefix
}
