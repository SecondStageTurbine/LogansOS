// SPDX-License-Identifier: MPL-2.0
//! Ethernet/IPv4 ARP parsing and cache for Phase 4 BUILD-3.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::{vec, vec::Vec};
use core::sync::atomic::{AtomicU64, Ordering};

use spin::{Lazy, Mutex};

use crate::drivers::net;

pub const ETHERNET_ARP: u16 = 0x0806;
pub const ETHERNET_IPV4: u16 = 0x0800;
pub const ETHERNET_HEADER_LEN: usize = 14;
pub const ARP_PACKET_LEN: usize = 28;
pub const ARP_REQUEST: u16 = 1;
pub const ARP_REPLY: u16 = 2;
const CACHE_TTL_TICKS: u64 = 60;
const PENDING_LIMIT: usize = 16;
/// OT-96: the cache's bound with `net-tick` (which `wire-rx` requires): the
/// tick ages the whole map (#175 item 4), and wire receive lets any LAN sender
/// add to it.
const MAX_ENTRIES: usize = 128;

static CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);
static CACHE: Lazy<Mutex<ArpCache>> = Lazy::new(|| Mutex::new(ArpCache::new()));
static PENDING: Lazy<Mutex<Vec<PendingFrame>>> = Lazy::new(|| Mutex::new(Vec::new()));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ArpError {
    Invalid,
    Unsupported,
    BufferTooSmall,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ArpPacket {
    pub opcode: u16,
    pub sender_mac: [u8; 6],
    pub sender_ip: [u8; 4],
    pub target_mac: [u8; 6],
    pub target_ip: [u8; 4],
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ArpCache {
    entries: BTreeMap<[u8; 4], ArpEntry>,
}

struct PendingFrame {
    interface_id: u32,
    target_ip: [u8; 4],
    frame: Vec<u8>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct ArpEntry {
    mac: [u8; 6],
    expires_at: u64,
}

impl ArpCache {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn lookup(&self, ip: [u8; 4], now: u64) -> Option<[u8; 6]> {
        self.entries
            .get(&ip)
            .filter(|entry| now < entry.expires_at)
            .map(|entry| entry.mac)
    }

    pub fn insert(&mut self, ip: [u8; 4], mac: [u8; 6], now: u64) {
        // OT-96: with `wire-rx` every LAN sender reaches here; keep the cache
        // bounded (expired entries go first, then the new one is refused).
        // With `net-tick` alone too: the tick ages the whole map (#175).
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
            ArpEntry {
                mac,
                expires_at: now.saturating_add(CACHE_TTL_TICKS),
            },
        );
    }

    pub fn age_out(&mut self, now: u64) {
        self.entries.retain(|_, entry| now < entry.expires_at);
    }
}

pub fn parse(frame: &[u8]) -> Result<ArpPacket, ArpError> {
    let packet = if frame.len() >= ETHERNET_HEADER_LEN + ARP_PACKET_LEN
        && u16::from_be_bytes([frame[12], frame[13]]) == ETHERNET_ARP
    {
        &frame[ETHERNET_HEADER_LEN..ETHERNET_HEADER_LEN + ARP_PACKET_LEN]
    } else {
        frame
    };
    if packet.len() < ARP_PACKET_LEN {
        return Err(ArpError::Invalid);
    }
    let htype = u16::from_be_bytes([packet[0], packet[1]]);
    let ptype = u16::from_be_bytes([packet[2], packet[3]]);
    if htype != 1 || ptype != ETHERNET_IPV4 || packet[4] != 6 || packet[5] != 4 {
        return Err(ArpError::Unsupported);
    }
    let mut sender_mac = [0u8; 6];
    let mut sender_ip = [0u8; 4];
    let mut target_mac = [0u8; 6];
    let mut target_ip = [0u8; 4];
    sender_mac.copy_from_slice(&packet[8..14]);
    sender_ip.copy_from_slice(&packet[14..18]);
    target_mac.copy_from_slice(&packet[18..24]);
    target_ip.copy_from_slice(&packet[24..28]);
    Ok(ArpPacket {
        opcode: u16::from_be_bytes([packet[6], packet[7]]),
        sender_mac,
        sender_ip,
        target_mac,
        target_ip,
    })
}

pub fn build(packet: ArpPacket, out: &mut [u8]) -> Result<usize, ArpError> {
    if out.len() < ARP_PACKET_LEN {
        return Err(ArpError::BufferTooSmall);
    }
    out[0..2].copy_from_slice(&1u16.to_be_bytes());
    out[2..4].copy_from_slice(&ETHERNET_IPV4.to_be_bytes());
    out[4] = 6;
    out[5] = 4;
    out[6..8].copy_from_slice(&packet.opcode.to_be_bytes());
    out[8..14].copy_from_slice(&packet.sender_mac);
    out[14..18].copy_from_slice(&packet.sender_ip);
    out[18..24].copy_from_slice(&packet.target_mac);
    out[24..28].copy_from_slice(&packet.target_ip);
    Ok(ARP_PACKET_LEN)
}

pub fn build_ethernet_request(
    src_mac: [u8; 6],
    src_ip: [u8; 4],
    target_ip: [u8; 4],
    out: &mut [u8],
) -> Result<usize, ArpError> {
    if out.len() < ETHERNET_HEADER_LEN + ARP_PACKET_LEN {
        return Err(ArpError::BufferTooSmall);
    }
    out[..6].copy_from_slice(&[0xff; 6]);
    out[6..12].copy_from_slice(&src_mac);
    out[12..14].copy_from_slice(&ETHERNET_ARP.to_be_bytes());
    build(
        ArpPacket {
            opcode: ARP_REQUEST,
            sender_mac: src_mac,
            sender_ip: src_ip,
            target_mac: [0; 6],
            target_ip,
        },
        &mut out[ETHERNET_HEADER_LEN..],
    )?;
    Ok(ETHERNET_HEADER_LEN + ARP_PACKET_LEN)
}

pub fn inject(frame: &[u8]) -> Result<(), ArpError> {
    let packet = parse(frame)?;
    if packet.opcode != ARP_REPLY {
        return Err(ArpError::Unsupported);
    }
    insert(packet.sender_ip, packet.sender_mac);
    Ok(())
}

pub fn lookup(ip: [u8; 4]) -> Option<[u8; 6]> {
    CACHE.lock().lookup(ip, now())
}

pub fn insert(ip: [u8; 4], mac: [u8; 6]) {
    CACHE.lock().insert(ip, mac, now());
    flush_pending(ip, mac);
}

pub fn submit_or_queue_ipv4(
    interface_id: u32,
    src_ip: [u8; 4],
    target_ip: [u8; 4],
    mut frame: Vec<u8>,
) -> Result<(), ArpError> {
    if let Some(mac) = lookup(target_ip) {
        if frame.len() < 6 {
            return Err(ArpError::Invalid);
        }
        frame[..6].copy_from_slice(&mac);
        return submit_frame(interface_id, &frame);
    }

    {
        let mut pending = PENDING.lock();
        if pending.len() >= PENDING_LIMIT {
            return Err(ArpError::Unsupported);
        }
        pending.push(PendingFrame {
            interface_id,
            target_ip,
            frame,
        });
    }
    send_request(interface_id, src_ip, target_ip)
}

/// Entries in the cache (the OT-96 flood leg checks its bound).
#[cfg(feature = "neo-test")]
pub fn debug_cache_len() -> usize {
    CACHE.lock().entries.len()
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

fn flush_pending(ip: [u8; 4], mac: [u8; 6]) {
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
        if pending.frame.len() >= 6 {
            pending.frame[..6].copy_from_slice(&mac);
            let _ = submit_frame(pending.interface_id, &pending.frame);
        }
    }
}

fn send_request(interface_id: u32, src_ip: [u8; 4], target_ip: [u8; 4]) -> Result<(), ArpError> {
    let device = net::get_device(interface_id as usize).ok_or(ArpError::Unsupported)?;
    let mut frame = vec![0u8; ETHERNET_HEADER_LEN + ARP_PACKET_LEN];
    let len = build_ethernet_request(device.mac_address(), src_ip, target_ip, &mut frame)?;
    submit_frame(interface_id, &frame[..len])
}

fn submit_frame(interface_id: u32, bytes: &[u8]) -> Result<(), ArpError> {
    let device = net::get_device(interface_id as usize).ok_or(ArpError::Unsupported)?;
    let mut frame = device.tx_reserve().ok_or(ArpError::Unsupported)?;
    if bytes.len() > frame.len() {
        return Err(ArpError::BufferTooSmall);
    }
    frame.as_mut_slice()[..bytes.len()].copy_from_slice(bytes);
    device
        .tx_submit(frame, bytes.len())
        .map_err(|_| ArpError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arp_reply_populates_cache() {
        let packet = ArpPacket {
            opcode: ARP_REPLY,
            sender_mac: [2, 3, 4, 5, 6, 7],
            sender_ip: [192, 0, 2, 1],
            target_mac: [0; 6],
            target_ip: [192, 0, 2, 2],
        };
        let mut bytes = [0u8; ARP_PACKET_LEN];
        assert!(build(packet, &mut bytes).is_ok());
        assert!(inject(&bytes).is_ok());
        assert_eq!(lookup([192, 0, 2, 1]), Some([2, 3, 4, 5, 6, 7]));
    }
}
