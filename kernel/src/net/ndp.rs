// SPDX-License-Identifier: MPL-2.0
//! Minimal IPv6 Neighbor Discovery parsing and cache for Phase 4 BUILD-4.5.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::{vec, vec::Vec};
use core::sync::atomic::{AtomicU64, Ordering};

use spin::{Lazy, Mutex};

use crate::drivers::net;
use crate::net::icmpv6::IPV6_NEXT_HEADER_ICMPV6;

pub const ETHERNET_IPV6: u16 = 0x86dd;
pub const ETHERNET_HEADER_LEN: usize = 14;
pub const IPV6_HEADER_LEN: usize = 40;
pub const ICMPV6_NEIGHBOR_SOLICITATION: u8 = 135;
pub const ICMPV6_NEIGHBOR_ADVERTISEMENT: u8 = 136;
pub const ND_OPT_SOURCE_LINK_LAYER_ADDRESS: u8 = 1;
pub const ND_OPT_TARGET_LINK_LAYER_ADDRESS: u8 = 2;
pub const NDP_BASE_LEN: usize = 24;
pub const NDP_LINK_LAYER_OPTION_LEN: usize = 8;
const CACHE_TTL_TICKS: u64 = 60;
const PENDING_LIMIT: usize = 16;
/// OT-96: the cache's bound with `net-tick` (which `wire-rx` requires), as
/// the ARP cache.
const MAX_ENTRIES: usize = 128;
const DEFAULT_HOP_LIMIT: u8 = 255;

static CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);
static CACHE: Lazy<Mutex<NeighborCache>> = Lazy::new(|| Mutex::new(NeighborCache::new()));
static PENDING: Lazy<Mutex<Vec<PendingFrame>>> = Lazy::new(|| Mutex::new(Vec::new()));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum NdpError {
    Invalid,
    Unsupported,
    BufferTooSmall,
    Checksum,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct NeighborSolicitation {
    pub target_ip: [u8; 16],
    pub source_mac: Option<[u8; 6]>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct NeighborAdvertisement {
    pub flags: u32,
    pub target_ip: [u8; 16],
    pub target_mac: Option<[u8; 6]>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum NeighborMessage {
    Solicitation(NeighborSolicitation),
    Advertisement(NeighborAdvertisement),
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct NeighborCache {
    entries: BTreeMap<[u8; 16], NeighborEntry>,
}

struct PendingFrame {
    interface_id: u32,
    target_ip: [u8; 16],
    frame: Vec<u8>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct NeighborEntry {
    mac: [u8; 6],
    expires_at: u64,
}

impl NeighborCache {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn lookup(&self, ip: [u8; 16], now: u64) -> Option<[u8; 6]> {
        self.entries
            .get(&ip)
            .filter(|entry| now < entry.expires_at)
            .map(|entry| entry.mac)
    }

    pub fn insert(&mut self, ip: [u8; 16], mac: [u8; 6], now: u64) {
        // OT-96: bounded with `net-tick` (and so `wire-rx`), as the ARP cache.
        if cfg!(feature = "net-tick")
            && !self.entries.contains_key(&ip)
            && self.entries.len() >= MAX_ENTRIES
        {
            self.age_out(now);
            if self.entries.len() >= MAX_ENTRIES {
                return;
            }
        }
        self.entries.insert(
            ip,
            NeighborEntry {
                mac,
                expires_at: now.saturating_add(CACHE_TTL_TICKS),
            },
        );
    }

    pub fn age_out(&mut self, now: u64) {
        self.entries.retain(|_, entry| now < entry.expires_at);
    }
}

pub fn build_neighbor_solicitation(
    src_ip: [u8; 16],
    dst_ip: [u8; 16],
    solicitation: NeighborSolicitation,
    out: &mut [u8],
) -> Result<usize, NdpError> {
    let total_len = NDP_BASE_LEN
        + if solicitation.source_mac.is_some() {
            NDP_LINK_LAYER_OPTION_LEN
        } else {
            0
        };
    if out.len() < total_len {
        return Err(NdpError::BufferTooSmall);
    }

    out[..total_len].fill(0);
    out[0] = ICMPV6_NEIGHBOR_SOLICITATION;
    out[8..24].copy_from_slice(&solicitation.target_ip);
    if let Some(mac) = solicitation.source_mac {
        write_link_layer_option(
            &mut out[NDP_BASE_LEN..NDP_BASE_LEN + NDP_LINK_LAYER_OPTION_LEN],
            ND_OPT_SOURCE_LINK_LAYER_ADDRESS,
            mac,
        )?;
    }
    let checksum = checksum_for_addresses(src_ip, dst_ip, &out[..total_len])?;
    out[2..4].copy_from_slice(&checksum.to_be_bytes());
    Ok(total_len)
}

pub fn build_neighbor_advertisement(
    src_ip: [u8; 16],
    dst_ip: [u8; 16],
    advertisement: NeighborAdvertisement,
    out: &mut [u8],
) -> Result<usize, NdpError> {
    let total_len = NDP_BASE_LEN
        + if advertisement.target_mac.is_some() {
            NDP_LINK_LAYER_OPTION_LEN
        } else {
            0
        };
    if out.len() < total_len {
        return Err(NdpError::BufferTooSmall);
    }

    out[..total_len].fill(0);
    out[0] = ICMPV6_NEIGHBOR_ADVERTISEMENT;
    out[4..8].copy_from_slice(&advertisement.flags.to_be_bytes());
    out[8..24].copy_from_slice(&advertisement.target_ip);
    if let Some(mac) = advertisement.target_mac {
        write_link_layer_option(
            &mut out[NDP_BASE_LEN..NDP_BASE_LEN + NDP_LINK_LAYER_OPTION_LEN],
            ND_OPT_TARGET_LINK_LAYER_ADDRESS,
            mac,
        )?;
    }
    let checksum = checksum_for_addresses(src_ip, dst_ip, &out[..total_len])?;
    out[2..4].copy_from_slice(&checksum.to_be_bytes());
    Ok(total_len)
}

pub fn parse_neighbor_solicitation(
    src_ip: [u8; 16],
    dst_ip: [u8; 16],
    packet: &[u8],
) -> Result<NeighborSolicitation, NdpError> {
    verify_checksum(src_ip, dst_ip, packet)?;
    match parse_message(packet)? {
        NeighborMessage::Solicitation(solicitation) => Ok(solicitation),
        NeighborMessage::Advertisement(_) => Err(NdpError::Unsupported),
    }
}

pub fn parse_neighbor_advertisement(
    src_ip: [u8; 16],
    dst_ip: [u8; 16],
    packet: &[u8],
) -> Result<NeighborAdvertisement, NdpError> {
    verify_checksum(src_ip, dst_ip, packet)?;
    match parse_message(packet)? {
        NeighborMessage::Advertisement(advertisement) => Ok(advertisement),
        NeighborMessage::Solicitation(_) => Err(NdpError::Unsupported),
    }
}

pub fn parse(frame: &[u8]) -> Result<NeighborMessage, NdpError> {
    if let Ok((src_ip, dst_ip, payload)) = ipv6_payload(frame) {
        verify_checksum(src_ip, dst_ip, payload)?;
        parse_message(payload)
    } else if cfg!(feature = "wire-rx") {
        // OT-96: with frames from the wire, never parse bytes that carried
        // no IPv6 header (and so no checksum).
        Err(NdpError::Invalid)
    } else {
        parse_message(frame)
    }
}

pub fn inject(frame: &[u8]) -> Result<(), NdpError> {
    match parse(frame)? {
        NeighborMessage::Advertisement(advertisement) => {
            let Some(mac) = advertisement.target_mac else {
                return Err(NdpError::Unsupported);
            };
            insert(advertisement.target_ip, mac);
            Ok(())
        }
        NeighborMessage::Solicitation(_) => Err(NdpError::Unsupported),
    }
}

pub fn lookup(ip: [u8; 16]) -> Option<[u8; 6]> {
    CACHE.lock().lookup(ip, now())
}

pub fn insert(ip: [u8; 16], mac: [u8; 6]) {
    CACHE.lock().insert(ip, mac, now());
    trace_cache_populated(ip, mac);
    flush_pending(ip, mac);
}

pub fn submit_or_queue_ipv6(
    interface_id: u32,
    src_ip: [u8; 16],
    target_ip: [u8; 16],
    mut frame: Vec<u8>,
) -> Result<(), NdpError> {
    if frame.len() < ETHERNET_HEADER_LEN {
        return Err(NdpError::Invalid);
    }

    if target_ip[0] == 0xff {
        frame[..6].copy_from_slice(&ipv6_multicast_mac(target_ip));
        return submit_frame(interface_id, &frame);
    }

    if src_ip == target_ip {
        return Ok(());
    }

    if let Some(mac) = lookup(target_ip) {
        frame[..6].copy_from_slice(&mac);
        trace_frame_egress(target_ip, mac);
        return submit_frame(interface_id, &frame);
    }

    {
        let mut pending = PENDING.lock();
        if pending.len() >= PENDING_LIMIT {
            return Err(NdpError::Unsupported);
        }
        pending.push(PendingFrame {
            interface_id,
            target_ip,
            frame,
        });
    }
    send_request(interface_id, src_ip, target_ip)
}

#[cfg(feature = "neo-test")]
pub fn debug_pending_count() -> usize {
    PENDING.lock().len()
}

pub fn tick() {
    let now = CLOCK_TICKS.fetch_add(1, Ordering::AcqRel) + 1;
    CACHE.lock().age_out(now);
}

pub fn now() -> u64 {
    CLOCK_TICKS.load(Ordering::Acquire)
}

pub fn solicited_node_multicast(target_ip: [u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[0] = 0xff;
    out[1] = 0x02;
    out[11] = 0x01;
    out[12] = 0xff;
    out[13] = target_ip[13];
    out[14] = target_ip[14];
    out[15] = target_ip[15];
    out
}

pub fn solicited_node_multicast_mac(target_ip: [u8; 16]) -> [u8; 6] {
    [
        0x33,
        0x33,
        0xff,
        target_ip[13],
        target_ip[14],
        target_ip[15],
    ]
}

pub fn ipv6_multicast_mac(ip: [u8; 16]) -> [u8; 6] {
    [0x33, 0x33, ip[12], ip[13], ip[14], ip[15]]
}

pub fn checksum_for_addresses(
    src_ip: [u8; 16],
    dst_ip: [u8; 16],
    packet: &[u8],
) -> Result<u16, NdpError> {
    if packet.len() > u32::MAX as usize {
        return Err(NdpError::BufferTooSmall);
    }
    let mut checksum = Checksum::new();
    checksum.add(&src_ip);
    checksum.add(&dst_ip);
    checksum.add(&(packet.len() as u32).to_be_bytes());
    checksum.add(&[0, 0, 0, IPV6_NEXT_HEADER_ICMPV6]);
    checksum.add(packet);
    Ok(checksum.finish())
}

fn parse_message(packet: &[u8]) -> Result<NeighborMessage, NdpError> {
    if packet.len() < NDP_BASE_LEN || packet[1] != 0 {
        return Err(NdpError::Invalid);
    }
    let mut target_ip = [0u8; 16];
    target_ip.copy_from_slice(&packet[8..24]);
    match packet[0] {
        ICMPV6_NEIGHBOR_SOLICITATION => Ok(NeighborMessage::Solicitation(NeighborSolicitation {
            target_ip,
            source_mac: find_link_layer_option(
                &packet[NDP_BASE_LEN..],
                ND_OPT_SOURCE_LINK_LAYER_ADDRESS,
            )?,
        })),
        ICMPV6_NEIGHBOR_ADVERTISEMENT => {
            Ok(NeighborMessage::Advertisement(NeighborAdvertisement {
                flags: u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]),
                target_ip,
                target_mac: find_link_layer_option(
                    &packet[NDP_BASE_LEN..],
                    ND_OPT_TARGET_LINK_LAYER_ADDRESS,
                )?,
            }))
        }
        _ => Err(NdpError::Unsupported),
    }
}

fn flush_pending(ip: [u8; 16], mac: [u8; 6]) {
    let mut ready = Vec::new();
    {
        let mut pending = PENDING.lock();
        let mut idx = 0;
        while idx < pending.len() {
            if pending[idx].target_ip == ip {
                ready.push(pending.swap_remove(idx));
            } else {
                idx += 1;
            }
        }
    }
    for mut pending in ready {
        if pending.frame.len() >= ETHERNET_HEADER_LEN {
            pending.frame[..6].copy_from_slice(&mac);
            trace_frame_egress(ip, mac);
            let _ = submit_frame(pending.interface_id, &pending.frame);
        }
    }
}

fn send_request(interface_id: u32, src_ip: [u8; 16], target_ip: [u8; 16]) -> Result<(), NdpError> {
    let device = net::get_device(interface_id as usize).ok_or(NdpError::Unsupported)?;
    let dst_ip = solicited_node_multicast(target_ip);
    let src_mac = device.mac_address();
    let dst_mac = solicited_node_multicast_mac(target_ip);
    let payload_len = NDP_BASE_LEN + NDP_LINK_LAYER_OPTION_LEN;
    let frame_len = ETHERNET_HEADER_LEN
        .checked_add(IPV6_HEADER_LEN)
        .and_then(|len| len.checked_add(payload_len))
        .ok_or(NdpError::BufferTooSmall)?;
    let mut frame = vec![0u8; frame_len];
    frame[..6].copy_from_slice(&dst_mac);
    frame[6..12].copy_from_slice(&src_mac);
    frame[12..14].copy_from_slice(&ETHERNET_IPV6.to_be_bytes());
    let ip = ETHERNET_HEADER_LEN;
    frame[ip] = 0x60;
    frame[ip + 4..ip + 6].copy_from_slice(&(payload_len as u16).to_be_bytes());
    frame[ip + 6] = IPV6_NEXT_HEADER_ICMPV6;
    frame[ip + 7] = DEFAULT_HOP_LIMIT;
    frame[ip + 8..ip + 24].copy_from_slice(&src_ip);
    frame[ip + 24..ip + 40].copy_from_slice(&dst_ip);
    build_neighbor_solicitation(
        src_ip,
        dst_ip,
        NeighborSolicitation {
            target_ip,
            source_mac: Some(src_mac),
        },
        &mut frame[ip + IPV6_HEADER_LEN..],
    )?;
    trace_ns_out(target_ip, dst_mac);
    submit_frame(interface_id, &frame)
}

fn submit_frame(interface_id: u32, bytes: &[u8]) -> Result<(), NdpError> {
    let device = net::get_device(interface_id as usize).ok_or(NdpError::Unsupported)?;
    let mut frame = device.tx_reserve().ok_or(NdpError::Unsupported)?;
    if bytes.len() > frame.len() {
        return Err(NdpError::BufferTooSmall);
    }
    frame.as_mut_slice()[..bytes.len()].copy_from_slice(bytes);
    device
        .tx_submit(frame, bytes.len())
        .map_err(|_| NdpError::Unsupported)
}

fn find_link_layer_option(options: &[u8], expected_type: u8) -> Result<Option<[u8; 6]>, NdpError> {
    let mut off = 0;
    while off < options.len() {
        if options.len() - off < 2 {
            return Err(NdpError::Invalid);
        }
        let option_type = options[off];
        let option_len = options[off + 1] as usize * 8;
        if option_len == 0 || off + option_len > options.len() {
            return Err(NdpError::Invalid);
        }
        if option_type == expected_type {
            if option_len < NDP_LINK_LAYER_OPTION_LEN {
                return Err(NdpError::Invalid);
            }
            let mut mac = [0u8; 6];
            mac.copy_from_slice(&options[off + 2..off + 8]);
            return Ok(Some(mac));
        }
        off += option_len;
    }
    Ok(None)
}

fn write_link_layer_option(out: &mut [u8], option_type: u8, mac: [u8; 6]) -> Result<(), NdpError> {
    if out.len() < NDP_LINK_LAYER_OPTION_LEN {
        return Err(NdpError::BufferTooSmall);
    }
    out[0] = option_type;
    out[1] = 1;
    out[2..8].copy_from_slice(&mac);
    Ok(())
}

fn verify_checksum(src_ip: [u8; 16], dst_ip: [u8; 16], packet: &[u8]) -> Result<(), NdpError> {
    if checksum_for_addresses(src_ip, dst_ip, packet)? == 0 {
        Ok(())
    } else {
        Err(NdpError::Checksum)
    }
}

type Ipv6PayloadResult<'a> = Result<([u8; 16], [u8; 16], &'a [u8]), NdpError>;

fn ipv6_payload(frame: &[u8]) -> Ipv6PayloadResult<'_> {
    let ipv6 = if frame.len() >= ETHERNET_HEADER_LEN + IPV6_HEADER_LEN
        && u16::from_be_bytes([frame[12], frame[13]]) == ETHERNET_IPV6
    {
        &frame[ETHERNET_HEADER_LEN..]
    } else {
        frame
    };
    if ipv6.len() < IPV6_HEADER_LEN || (ipv6[0] >> 4) != 6 {
        return Err(NdpError::Invalid);
    }
    if ipv6[6] != IPV6_NEXT_HEADER_ICMPV6 {
        return Err(NdpError::Unsupported);
    }
    let payload_len = u16::from_be_bytes([ipv6[4], ipv6[5]]) as usize;
    if ipv6.len() < IPV6_HEADER_LEN + payload_len {
        return Err(NdpError::Invalid);
    }
    let mut src = [0u8; 16];
    let mut dst = [0u8; 16];
    src.copy_from_slice(&ipv6[8..24]);
    dst.copy_from_slice(&ipv6[24..40]);
    Ok((
        src,
        dst,
        &ipv6[IPV6_HEADER_LEN..IPV6_HEADER_LEN + payload_len],
    ))
}

struct Checksum {
    sum: u32,
}

impl Checksum {
    fn new() -> Self {
        Self { sum: 0 }
    }

    fn add(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(2);
        for chunk in &mut chunks {
            self.sum = self
                .sum
                .wrapping_add(u16::from_be_bytes([chunk[0], chunk[1]]) as u32);
        }
        if let Some(&byte) = chunks.remainder().first() {
            self.sum = self.sum.wrapping_add((byte as u32) << 8);
        }
    }

    fn finish(mut self) -> u16 {
        while self.sum >> 16 != 0 {
            self.sum = (self.sum & 0xffff) + (self.sum >> 16);
        }
        !(self.sum as u16)
    }
}

#[cfg(feature = "neo-test")]
fn trace_ns_out(target_ip: [u8; 16], dst_mac: [u8; 6]) {
    crate::serial_println!(
        "[ kernel ] ndp: ns_out target={:02x}{:02x}:{:02x}{:02x}:...:{:02x}{:02x}:{:02x}{:02x} dst_mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        target_ip[0],
        target_ip[1],
        target_ip[2],
        target_ip[3],
        target_ip[12],
        target_ip[13],
        target_ip[14],
        target_ip[15],
        dst_mac[0],
        dst_mac[1],
        dst_mac[2],
        dst_mac[3],
        dst_mac[4],
        dst_mac[5]
    );
}

#[cfg(not(feature = "neo-test"))]
fn trace_ns_out(_target_ip: [u8; 16], _dst_mac: [u8; 6]) {}

#[cfg(feature = "neo-test")]
fn trace_cache_populated(ip: [u8; 16], mac: [u8; 6]) {
    crate::serial_println!(
        "[ kernel ] ndp: na_in cache_populated target={:02x}{:02x}:{:02x}{:02x}:...:{:02x}{:02x}:{:02x}{:02x} mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        ip[0],
        ip[1],
        ip[2],
        ip[3],
        ip[12],
        ip[13],
        ip[14],
        ip[15],
        mac[0],
        mac[1],
        mac[2],
        mac[3],
        mac[4],
        mac[5]
    );
}

#[cfg(not(feature = "neo-test"))]
fn trace_cache_populated(_ip: [u8; 16], _mac: [u8; 6]) {}

#[cfg(feature = "neo-test")]
fn trace_frame_egress(ip: [u8; 16], mac: [u8; 6]) {
    crate::serial_println!(
        "[ kernel ] ndp: frame_egress resolved target={:02x}{:02x}:{:02x}{:02x}:...:{:02x}{:02x}:{:02x}{:02x} dst_mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        ip[0],
        ip[1],
        ip[2],
        ip[3],
        ip[12],
        ip[13],
        ip[14],
        ip[15],
        mac[0],
        mac[1],
        mac[2],
        mac[3],
        mac[4],
        mac[5]
    );
}

#[cfg(not(feature = "neo-test"))]
fn trace_frame_egress(_ip: [u8; 16], _mac: [u8; 6]) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ns_round_trip_checksum_and_multicast() {
        let src = [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let target = [
            0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0x12, 0x34, 0x56, 0x78,
        ];
        let dst = solicited_node_multicast(target);
        let mut bytes = [0u8; NDP_BASE_LEN + NDP_LINK_LAYER_OPTION_LEN];
        let built = build_neighbor_solicitation(
            src,
            dst,
            NeighborSolicitation {
                target_ip: target,
                source_mac: Some([2, 3, 4, 5, 6, 7]),
            },
            &mut bytes,
        );
        assert_eq!(built, Ok(NDP_BASE_LEN + NDP_LINK_LAYER_OPTION_LEN));
        let Ok(len) = built else {
            return;
        };
        assert_eq!(checksum_for_addresses(src, dst, &bytes[..len]), Ok(0));
        assert_eq!(
            parse_neighbor_solicitation(src, dst, &bytes[..len]),
            Ok(NeighborSolicitation {
                target_ip: target,
                source_mac: Some([2, 3, 4, 5, 6, 7]),
            })
        );
        assert_eq!(dst[12..16], [0xff, 0x34, 0x56, 0x78]);
        assert_eq!(
            solicited_node_multicast_mac(target),
            [0x33, 0x33, 0xff, 0x34, 0x56, 0x78]
        );
    }

    #[test]
    fn na_inject_populates_and_tick_expires_cache() {
        let src = [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];
        let dst = [0xff, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let mut bytes = [0u8; NDP_BASE_LEN + NDP_LINK_LAYER_OPTION_LEN];
        let built = build_neighbor_advertisement(
            src,
            dst,
            NeighborAdvertisement {
                flags: 0x6000_0000,
                target_ip: src,
                target_mac: Some([8, 7, 6, 5, 4, 3]),
            },
            &mut bytes,
        );
        assert_eq!(built, Ok(NDP_BASE_LEN + NDP_LINK_LAYER_OPTION_LEN));
        let Ok(len) = built else {
            return;
        };
        assert!(inject(&bytes[..len]).is_ok());
        assert_eq!(lookup(src), Some([8, 7, 6, 5, 4, 3]));
        for _ in 0..=CACHE_TTL_TICKS {
            tick();
        }
        assert_eq!(lookup(src), None);
    }
}
