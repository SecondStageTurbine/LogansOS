// SPDX-License-Identifier: MPL-2.0
//! IPv4 header and address state for Phase 4 BUILD-3.

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::{Lazy, Mutex};

pub const IPV4_PROTOCOL_UDP: u8 = 17;
pub const ETHERNET_IPV4: u16 = 0x0800;
pub const ETHERNET_HEADER_LEN: usize = 14;
pub const IPV4_HEADER_LEN: usize = 20;
pub const UDP_HEADER_LEN: usize = 8;
pub const DEFAULT_TTL: u8 = 64;

static CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);
static INTERFACES: Lazy<Mutex<Vec<Ipv4AddressTable>>> =
    Lazy::new(|| Mutex::new(alloc::vec![Ipv4AddressTable::new(0)]));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Ipv4Error {
    Invalid,
    Unsupported,
    BufferTooSmall,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Ipv4Header {
    pub src: [u8; 4],
    pub dst: [u8; 4],
    pub protocol: u8,
    pub ttl: u8,
    pub identification: u16,
    pub payload_len: u16,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Ipv4Address {
    pub addr: [u8; 4],
    pub mask: [u8; 4],
    pub router: Option<[u8; 4]>,
    pub lease_until: Option<u64>,
    pub preferred: bool,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Ipv4AddressTable {
    pub interface_id: u32,
    pub addresses: Vec<Ipv4Address>,
}

impl Ipv4AddressTable {
    pub fn new(interface_id: u32) -> Self {
        Self {
            interface_id,
            addresses: Vec::new(),
        }
    }

    pub fn apply_lease(
        &mut self,
        addr: [u8; 4],
        mask: [u8; 4],
        router: Option<[u8; 4]>,
        lease_until: Option<u64>,
    ) {
        let leased = Ipv4Address {
            addr,
            mask,
            router,
            lease_until,
            preferred: true,
        };
        if let Some(existing) = self.addresses.iter_mut().find(|entry| entry.addr == addr) {
            *existing = leased;
        } else if !cfg!(feature = "wire-rx") || self.addresses.len() < MAX_ADDRESSES {
            self.addresses.push(leased);
        }
    }

    pub fn refresh(&mut self, now: u64) {
        self.addresses.retain(|addr| {
            addr.lease_until
                .map(|deadline| now < deadline)
                .unwrap_or(true)
        });
    }

    pub fn preferred_address(&self) -> Option<[u8; 4]> {
        self.addresses
            .iter()
            .find(|addr| addr.preferred)
            .map(|addr| addr.addr)
    }
}

impl Ipv4Header {
    pub fn total_len(self) -> u16 {
        IPV4_HEADER_LEN as u16 + self.payload_len
    }

    pub fn encode(self, out: &mut [u8]) -> Result<usize, Ipv4Error> {
        if out.len() < IPV4_HEADER_LEN {
            return Err(Ipv4Error::BufferTooSmall);
        }
        out[..IPV4_HEADER_LEN].fill(0);
        out[0] = 0x45;
        out[2..4].copy_from_slice(&self.total_len().to_be_bytes());
        out[4..6].copy_from_slice(&self.identification.to_be_bytes());
        out[6..8].copy_from_slice(&0x4000u16.to_be_bytes());
        out[8] = self.ttl;
        out[9] = self.protocol;
        out[12..16].copy_from_slice(&self.src);
        out[16..20].copy_from_slice(&self.dst);
        let checksum = checksum(&out[..IPV4_HEADER_LEN]);
        out[10..12].copy_from_slice(&checksum.to_be_bytes());
        Ok(IPV4_HEADER_LEN)
    }
}

pub fn parse_header(packet: &[u8]) -> Result<Ipv4Header, Ipv4Error> {
    if packet.len() < IPV4_HEADER_LEN {
        return Err(Ipv4Error::Invalid);
    }
    if packet[0] >> 4 != 4 {
        return Err(Ipv4Error::Invalid);
    }
    let ihl = (packet[0] & 0x0f) as usize * 4;
    if ihl < IPV4_HEADER_LEN || packet.len() < ihl {
        return Err(Ipv4Error::Invalid);
    }
    let total_len = u16::from_be_bytes([packet[2], packet[3]]) as usize;
    if total_len < ihl || packet.len() < total_len {
        return Err(Ipv4Error::Invalid);
    }
    if checksum(&packet[..ihl]) != 0 {
        return Err(Ipv4Error::Invalid);
    }
    if packet[6] & 0x20 != 0 {
        return Err(Ipv4Error::Unsupported);
    }
    let mut src = [0u8; 4];
    let mut dst = [0u8; 4];
    src.copy_from_slice(&packet[12..16]);
    dst.copy_from_slice(&packet[16..20]);
    Ok(Ipv4Header {
        src,
        dst,
        protocol: packet[9],
        ttl: packet[8],
        identification: u16::from_be_bytes([packet[4], packet[5]]),
        payload_len: (total_len - ihl) as u16,
    })
}

pub fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut chunks = bytes.chunks_exact(2);
    for chunk in &mut chunks {
        sum = sum.wrapping_add(u16::from_be_bytes([chunk[0], chunk[1]]) as u32);
    }
    if let Some(&byte) = chunks.remainder().first() {
        sum = sum.wrapping_add((byte as u32) << 8);
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

pub fn build_udp_packet(
    src: [u8; 4],
    dst: [u8; 4],
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
    out: &mut [u8],
) -> Result<usize, Ipv4Error> {
    let udp_len = UDP_HEADER_LEN
        .checked_add(payload.len())
        .ok_or(Ipv4Error::BufferTooSmall)?;
    let total_len = IPV4_HEADER_LEN
        .checked_add(udp_len)
        .ok_or(Ipv4Error::BufferTooSmall)?;
    if udp_len > u16::MAX as usize || total_len > u16::MAX as usize {
        return Err(Ipv4Error::BufferTooSmall);
    }
    if out.len() < total_len {
        return Err(Ipv4Error::BufferTooSmall);
    }
    let header = Ipv4Header {
        src,
        dst,
        protocol: IPV4_PROTOCOL_UDP,
        ttl: DEFAULT_TTL,
        identification: 0,
        payload_len: udp_len as u16,
    };
    header.encode(out)?;
    let udp = IPV4_HEADER_LEN;
    out[udp..udp + 2].copy_from_slice(&src_port.to_be_bytes());
    out[udp + 2..udp + 4].copy_from_slice(&dst_port.to_be_bytes());
    out[udp + 4..udp + 6].copy_from_slice(&(udp_len as u16).to_be_bytes());
    out[udp + 6..udp + 8].copy_from_slice(&0u16.to_be_bytes());
    out[udp + UDP_HEADER_LEN..udp + UDP_HEADER_LEN + payload.len()].copy_from_slice(payload);
    Ok(total_len)
}

#[allow(clippy::too_many_arguments)] // packet builder mirrors wire header fields explicitly
pub fn build_ethernet_udp_frame(
    src_mac: [u8; 6],
    dst_mac: [u8; 6],
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
    out: &mut [u8],
) -> Result<usize, Ipv4Error> {
    if out.len() < ETHERNET_HEADER_LEN {
        return Err(Ipv4Error::BufferTooSmall);
    }
    out[..ETHERNET_HEADER_LEN].fill(0);
    out[..6].copy_from_slice(&dst_mac);
    out[6..12].copy_from_slice(&src_mac);
    out[12..14].copy_from_slice(&ETHERNET_IPV4.to_be_bytes());
    let packet_len = build_udp_packet(
        src_ip,
        dst_ip,
        src_port,
        dst_port,
        payload,
        &mut out[ETHERNET_HEADER_LEN..],
    )?;
    Ok(ETHERNET_HEADER_LEN + packet_len)
}

pub fn apply_lease(
    interface_id: u32,
    addr: [u8; 4],
    mask: [u8; 4],
    router: Option<[u8; 4]>,
    lease_ticks: Option<u64>,
) {
    let now = now();
    let lease_until = lease_ticks.map(|ticks| now.saturating_add(ticks));
    let mut tables = INTERFACES.lock();
    let table = match tables
        .iter_mut()
        .find(|table| table.interface_id == interface_id)
    {
        Some(table) => table,
        None => {
            tables.push(Ipv4AddressTable::new(interface_id));
            let idx = tables.len().saturating_sub(1);
            &mut tables[idx]
        }
    };
    table.apply_lease(addr, mask, router, lease_until);
    // F11 S4: a status source changed (inside the lock, after the write).
    crate::net::status_gen::bump();
}

/// OT-96: with `wire-rx` the wire reaches this table, so it is capped.
/// (Declared down here so no line above moves: panic locations stay put.)
const MAX_ADDRESSES: usize = 8;

/// OT-96: this interface's preferred address (else its first) and mask,
/// without copying the table (the receive thread asks once per frame).
#[cfg(feature = "wire-rx")]
pub fn primary(interface_id: u32) -> Option<([u8; 4], [u8; 4])> {
    let tables = INTERFACES.lock();
    let table = tables.iter().find(|t| t.interface_id == interface_id)?;
    table
        .addresses
        .iter()
        .find(|a| a.preferred)
        .or(table.addresses.first())
        .map(|a| (a.addr, a.mask))
}

pub fn inject_address(interface_id: u32, addr: [u8; 4], mask: [u8; 4], router: Option<[u8; 4]>) {
    apply_lease(interface_id, addr, mask, router, None);
}

pub fn tick() {
    let now = CLOCK_TICKS.fetch_add(1, Ordering::AcqRel) + 1;
    let mut tables = INTERFACES.lock();
    let mut expired = false;
    for table in tables.iter_mut() {
        let before = table.addresses.len();
        table.refresh(now);
        expired |= table.addresses.len() != before;
    }
    // F11 S4: an expired address is a new status (inside the lock).
    if expired {
        crate::net::status_gen::bump();
    }
}

pub fn snapshot(interface_id: u32) -> Option<Ipv4AddressTable> {
    INTERFACES
        .lock()
        .iter()
        .find(|table| table.interface_id == interface_id)
        .cloned()
}

pub fn preferred_address(interface_id: u32) -> Option<[u8; 4]> {
    snapshot(interface_id).and_then(|table| table.preferred_address())
}

pub fn now() -> u64 {
    CLOCK_TICKS.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_header_round_trip_checksum() {
        let header = Ipv4Header {
            src: [192, 0, 2, 10],
            dst: [198, 51, 100, 20],
            protocol: IPV4_PROTOCOL_UDP,
            ttl: DEFAULT_TTL,
            identification: 7,
            payload_len: 4,
        };
        let mut packet = [0u8; 24];
        assert!(header.encode(&mut packet).is_ok());
        assert_eq!(parse_header(&packet), Ok(header));
        assert_eq!(checksum(&packet[..IPV4_HEADER_LEN]), 0);
    }
}
