// SPDX-License-Identifier: MPL-2.0
//! Hand-rolled RFC 1035 stub resolver core for Phase 4 BUILD-5a.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, AtomicU64, Ordering};

use spin::{Lazy, Mutex};

use crate::net::dhcpv4;
use crate::net::icmpv6;
use crate::net::socket::{AF_INET, AF_INET6, AddressFamily, SOCK_DGRAM, Socket, SocketAddr};
use crate::sched::process::ProcessId;

pub const DNS_PORT: u16 = 53;
pub const TYPE_A: u16 = 1;
pub const TYPE_AAAA: u16 = 28;
pub const CLASS_IN: u16 = 1;
pub const TYPE_OPT: u16 = 41;
pub const EDNS_UDP_PAYLOAD: u16 = 1232;
pub const MAX_NAME_LEN: usize = 253;
pub const MAX_DNS_PACKET: usize = 1232;
pub const MAX_CANDIDATES: usize = 8;
pub const MAX_NAME_SERVERS: usize = 8;

static CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);
static NEXT_TXID: AtomicU16 = AtomicU16::new(0x4e50);
static CONFIG: Lazy<Mutex<ResolverConfig>> = Lazy::new(|| Mutex::new(ResolverConfig::new()));
static CACHE: Lazy<Mutex<BTreeMap<CacheKey, Vec<CachedAnswer>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DnsError {
    Invalid,
    BufferTooSmall,
    Unsupported,
    NotFound,
    NoResolver,
    WouldBlock,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub enum IpAddr {
    V4([u8; 4]),
    V6([u8; 16]),
}

impl IpAddr {
    pub fn family(self) -> AddressFamily {
        match self {
            IpAddr::V4(_) => AddressFamily::Inet4,
            IpAddr::V6(_) => AddressFamily::Inet6,
        }
    }

    pub fn into_socket_addr(self, port: u16) -> SocketAddr {
        let mut addr = [0u8; 16];
        match self {
            IpAddr::V4(v4) => addr[..4].copy_from_slice(&v4),
            IpAddr::V6(v6) => addr = v6,
        }
        SocketAddr {
            family: self.family(),
            port,
            addr,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct NetCandidate {
    pub family: u16,
    pub reserved: u16,
    pub ttl: u32,
    pub addr: [u8; 16],
}

impl NetCandidate {
    pub fn from_ip(ip: IpAddr, ttl: u32) -> Self {
        let mut addr = [0u8; 16];
        let family = match ip {
            IpAddr::V4(v4) => {
                addr[..4].copy_from_slice(&v4);
                AF_INET as u16
            }
            IpAddr::V6(v6) => {
                addr = v6;
                AF_INET6 as u16
            }
        };
        Self {
            family,
            reserved: 0,
            ttl,
            addr,
        }
    }

    pub fn socket_addr(self, port: u16) -> Result<SocketAddr, DnsError> {
        let family = match self.family as u32 {
            AF_INET => AddressFamily::Inet4,
            AF_INET6 => AddressFamily::Inet6,
            _ => return Err(DnsError::Invalid),
        };
        Ok(SocketAddr {
            family,
            port,
            addr: self.addr,
        })
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum FamilyHint {
    Any,
    Inet4,
    Inet6,
}

impl FamilyHint {
    pub fn from_abi(raw: u32) -> Result<Self, DnsError> {
        match raw {
            0 => Ok(Self::Any),
            AF_INET => Ok(Self::Inet4),
            AF_INET6 => Ok(Self::Inet6),
            _ => Err(DnsError::Invalid),
        }
    }

    fn accepts(self, rr_type: u16) -> bool {
        matches!(
            (self, rr_type),
            (FamilyHint::Any, TYPE_A)
                | (FamilyHint::Any, TYPE_AAAA)
                | (FamilyHint::Inet4, TYPE_A)
                | (FamilyHint::Inet6, TYPE_AAAA)
        )
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct NameServer {
    pub addr: IpAddr,
    pub valid_until: u64,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ResolverConfig {
    pub servers: Vec<NameServer>,
}

impl ResolverConfig {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    pub fn insert(&mut self, addr: IpAddr, lifetime: u32, now: u64) {
        if lifetime == 0 {
            self.servers.retain(|server| server.addr != addr);
            return;
        }
        let valid_until = now.saturating_add(lifetime as u64);
        if let Some(server) = self.servers.iter_mut().find(|server| server.addr == addr) {
            server.valid_until = valid_until;
            return;
        }
        if self.servers.len() < MAX_NAME_SERVERS {
            self.servers.push(NameServer { addr, valid_until });
        }
    }

    pub fn refresh(&mut self, now: u64) {
        self.servers.retain(|server| now < server.valid_until);
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
struct CacheKey {
    name: String,
    rr_type: u16,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct CachedAnswer {
    addr: IpAddr,
    expires_at: u64,
    ttl: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Question<'a> {
    pub name: &'a str,
    pub rr_type: u16,
}

pub fn configure_from_dhcp_lease(lease: Option<dhcpv4::Lease>) {
    if let Some(lease) = lease
        && let Some(dns) = lease.dns
    {
        let lifetime = lease
            .expiry
            .saturating_sub(dhcpv4::now())
            .min(u32::MAX as u64) as u32;
        CONFIG.lock().insert(IpAddr::V4(dns), lifetime, now());
    }
}

pub fn configure_from_ra(ra: &icmpv6::RouterAdvertisement) {
    let now = now();
    let mut config = CONFIG.lock();
    for server in &ra.rdnss {
        for addr in &server.servers {
            config.insert(IpAddr::V6(*addr), server.lifetime, now);
        }
    }
}

pub fn resolver_config() -> ResolverConfig {
    let now = now();
    let mut config = CONFIG.lock();
    config.refresh(now);
    config.clone()
}

pub fn build_query(name: &str, rr_type: u16, out: &mut [u8]) -> Result<usize, DnsError> {
    build_query_with_txid(name, rr_type, NEXT_TXID.fetch_add(1, Ordering::AcqRel), out)
}

pub fn build_query_with_txid(
    name: &str,
    rr_type: u16,
    txid: u16,
    out: &mut [u8],
) -> Result<usize, DnsError> {
    if rr_type != TYPE_A && rr_type != TYPE_AAAA {
        return Err(DnsError::Unsupported);
    }
    if out.len() < 12 {
        return Err(DnsError::BufferTooSmall);
    }
    out[..12].fill(0);
    out[0..2].copy_from_slice(&txid.to_be_bytes());
    out[2..4].copy_from_slice(&0x0100u16.to_be_bytes());
    out[4..6].copy_from_slice(&1u16.to_be_bytes());
    out[10..12].copy_from_slice(&1u16.to_be_bytes());
    let mut off = 12;
    off = encode_name(name, out, off)?;
    if off + 15 > out.len() {
        return Err(DnsError::BufferTooSmall);
    }
    out[off..off + 2].copy_from_slice(&rr_type.to_be_bytes());
    out[off + 2..off + 4].copy_from_slice(&CLASS_IN.to_be_bytes());
    off += 4;
    out[off] = 0;
    out[off + 1..off + 3].copy_from_slice(&TYPE_OPT.to_be_bytes());
    out[off + 3..off + 5].copy_from_slice(&EDNS_UDP_PAYLOAD.to_be_bytes());
    out[off + 5..off + 9].fill(0);
    out[off + 9..off + 11].copy_from_slice(&0u16.to_be_bytes());
    Ok(off + 11)
}

pub fn inject(response: &[u8]) -> Result<usize, DnsError> {
    let parsed = parse_response(response)?;
    let now = now();
    let mut inserted = 0usize;
    let mut cache = CACHE.lock();
    for answer in parsed.answers {
        let key = CacheKey {
            name: parsed.question.name.to_ascii_lowercase(),
            rr_type: answer.rr_type,
        };
        let entry = CachedAnswer {
            addr: answer.addr,
            expires_at: now.saturating_add(answer.ttl as u64),
            ttl: answer.ttl,
        };
        let entries = cache.entry(key).or_default();
        if let Some(existing) = entries.iter_mut().find(|item| item.addr == entry.addr) {
            *existing = entry;
        } else {
            entries.push(entry);
        }
        inserted = inserted.saturating_add(1);
    }
    Ok(inserted)
}

pub fn resolve(name: &str, hint: FamilyHint) -> Result<Vec<NetCandidate>, DnsError> {
    validate_name(name)?;
    let now = now();
    let mut out = Vec::new();
    {
        let mut cache = CACHE.lock();
        for rr_type in [TYPE_AAAA, TYPE_A] {
            if !hint.accepts(rr_type) {
                continue;
            }
            let key = CacheKey {
                name: name.to_ascii_lowercase(),
                rr_type,
            };
            if let Some(entries) = cache.get_mut(&key) {
                entries.retain(|answer| now < answer.expires_at);
                for answer in entries.iter().take(MAX_CANDIDATES - out.len()) {
                    out.push(NetCandidate::from_ip(answer.addr, answer.ttl));
                }
            }
            if out.len() >= MAX_CANDIDATES {
                break;
            }
        }
    }
    if !out.is_empty() {
        return Ok(out);
    }
    query_configured_servers(name, hint)?;
    Err(DnsError::WouldBlock)
}

pub fn tick() {
    let now = CLOCK_TICKS.fetch_add(1, Ordering::AcqRel).saturating_add(1);
    CONFIG.lock().refresh(now);
    let mut cache = CACHE.lock();
    cache.retain(|_key, entries| {
        entries.retain(|answer| now < answer.expires_at);
        !entries.is_empty()
    });
}

pub fn age_out(ticks: u64) {
    for _ in 0..ticks {
        tick();
    }
}

fn now() -> u64 {
    CLOCK_TICKS.load(Ordering::Acquire)
}

fn query_configured_servers(name: &str, hint: FamilyHint) -> Result<(), DnsError> {
    let config = resolver_config();
    if config.servers.is_empty() {
        return Err(DnsError::NoResolver);
    }
    let mut sent = false;
    for rr_type in [TYPE_AAAA, TYPE_A] {
        if !hint.accepts(rr_type) {
            continue;
        }
        let mut packet = [0u8; MAX_DNS_PACKET];
        let len = build_query(name, rr_type, &mut packet)?;
        for server in &config.servers {
            let socket = Socket::new(ProcessId(0), family_abi(server.addr), SOCK_DGRAM)
                .map_err(|_| DnsError::Unsupported)?;
            let socket = alloc::sync::Arc::new(socket);
            let bind = unspecified(server.addr.family(), ephemeral_dns_port());
            if socket.bind(bind).is_err() {
                continue;
            }
            if socket
                .send_to(server.addr.into_socket_addr(DNS_PORT), &packet[..len])
                .is_ok()
            {
                sent = true;
            }
        }
    }
    if sent {
        Ok(())
    } else {
        Err(DnsError::NoResolver)
    }
}

fn family_abi(addr: IpAddr) -> u32 {
    match addr {
        IpAddr::V4(_) => AF_INET,
        IpAddr::V6(_) => AF_INET6,
    }
}

fn unspecified(family: AddressFamily, port: u16) -> SocketAddr {
    SocketAddr {
        family,
        port,
        addr: [0u8; 16],
    }
}

fn ephemeral_dns_port() -> u16 {
    let raw = NEXT_TXID.fetch_add(1, Ordering::AcqRel);
    40000u16.saturating_add(raw % 20000)
}

struct ParsedResponse {
    question: ParsedQuestion,
    answers: Vec<ParsedAnswer>,
}

struct ParsedQuestion {
    name: String,
}

struct ParsedAnswer {
    rr_type: u16,
    ttl: u32,
    addr: IpAddr,
}

fn parse_response(packet: &[u8]) -> Result<ParsedResponse, DnsError> {
    if packet.len() < 12 {
        return Err(DnsError::Invalid);
    }
    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    if flags & 0x8000 == 0 || flags & 0x000f != 0 {
        return Err(DnsError::Invalid);
    }
    let qd = u16::from_be_bytes([packet[4], packet[5]]) as usize;
    let an = u16::from_be_bytes([packet[6], packet[7]]) as usize;
    if qd != 1 {
        return Err(DnsError::Unsupported);
    }
    let (name, mut off) = read_name(packet, 12)?;
    if off + 4 > packet.len() {
        return Err(DnsError::Invalid);
    }
    let _qtype = u16::from_be_bytes([packet[off], packet[off + 1]]);
    let qclass = u16::from_be_bytes([packet[off + 2], packet[off + 3]]);
    if qclass != CLASS_IN {
        return Err(DnsError::Unsupported);
    }
    off += 4;
    let mut answers = Vec::new();
    for _ in 0..an {
        let (_rr_name, next) = read_name(packet, off)?;
        off = next;
        if off + 10 > packet.len() {
            return Err(DnsError::Invalid);
        }
        let rr_type = u16::from_be_bytes([packet[off], packet[off + 1]]);
        let class = u16::from_be_bytes([packet[off + 2], packet[off + 3]]);
        let ttl = u32::from_be_bytes([
            packet[off + 4],
            packet[off + 5],
            packet[off + 6],
            packet[off + 7],
        ]);
        let rdlen = u16::from_be_bytes([packet[off + 8], packet[off + 9]]) as usize;
        off += 10;
        if off + rdlen > packet.len() {
            return Err(DnsError::Invalid);
        }
        if class == CLASS_IN && ttl > 0 {
            match (rr_type, rdlen) {
                (TYPE_A, 4) => answers.push(ParsedAnswer {
                    rr_type,
                    ttl,
                    addr: IpAddr::V4([
                        packet[off],
                        packet[off + 1],
                        packet[off + 2],
                        packet[off + 3],
                    ]),
                }),
                (TYPE_AAAA, 16) => {
                    let mut addr = [0u8; 16];
                    addr.copy_from_slice(&packet[off..off + 16]);
                    answers.push(ParsedAnswer {
                        rr_type,
                        ttl,
                        addr: IpAddr::V6(addr),
                    });
                }
                _ => {}
            }
        }
        off += rdlen;
    }
    if answers.is_empty() {
        return Err(DnsError::NotFound);
    }
    Ok(ParsedResponse {
        question: ParsedQuestion { name },
        answers,
    })
}

fn validate_name(name: &str) -> Result<(), DnsError> {
    if name.is_empty() || name.len() > MAX_NAME_LEN || name.as_bytes().contains(&0) {
        return Err(DnsError::Invalid);
    }
    for label in name.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(DnsError::Invalid);
        }
    }
    Ok(())
}

fn encode_name(name: &str, out: &mut [u8], mut off: usize) -> Result<usize, DnsError> {
    validate_name(name)?;
    for label in name.split('.') {
        if off + 1 + label.len() > out.len() {
            return Err(DnsError::BufferTooSmall);
        }
        out[off] = label.len() as u8;
        off += 1;
        out[off..off + label.len()].copy_from_slice(label.as_bytes());
        off += label.len();
    }
    if off >= out.len() {
        return Err(DnsError::BufferTooSmall);
    }
    out[off] = 0;
    Ok(off + 1)
}

fn read_name(packet: &[u8], mut off: usize) -> Result<(String, usize), DnsError> {
    let mut labels: Vec<String> = Vec::new();
    let mut end = None;
    let mut visited = [usize::MAX; 16];
    let mut depth = 0usize;
    loop {
        if off >= packet.len() || depth >= visited.len() {
            return Err(DnsError::Invalid);
        }
        if visited[..depth].contains(&off) {
            return Err(DnsError::Invalid);
        }
        visited[depth] = off;
        depth += 1;
        let len = packet[off];
        if len & 0xc0 == 0xc0 {
            if off + 1 >= packet.len() {
                return Err(DnsError::Invalid);
            }
            let ptr = (((len & 0x3f) as usize) << 8) | packet[off + 1] as usize;
            if ptr >= packet.len() {
                return Err(DnsError::Invalid);
            }
            if end.is_none() {
                end = Some(off + 2);
            }
            off = ptr;
            continue;
        }
        if len & 0xc0 != 0 {
            return Err(DnsError::Invalid);
        }
        off += 1;
        if len == 0 {
            let name = labels.join(".");
            validate_name(&name)?;
            return Ok((name, end.unwrap_or(off)));
        }
        let len = len as usize;
        if len > 63 || off + len > packet.len() {
            return Err(DnsError::Invalid);
        }
        let label = core::str::from_utf8(&packet[off..off + len]).map_err(|_| DnsError::Invalid)?;
        labels.push(String::from(label));
        off += len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(rr_type: u16, rdata: &[u8]) -> Vec<u8> {
        let mut packet = Vec::new();
        packet.extend_from_slice(&0x1234u16.to_be_bytes());
        packet.extend_from_slice(&0x8180u16.to_be_bytes());
        packet.extend_from_slice(&1u16.to_be_bytes());
        packet.extend_from_slice(&1u16.to_be_bytes());
        packet.extend_from_slice(&0u16.to_be_bytes());
        packet.extend_from_slice(&0u16.to_be_bytes());
        packet.extend_from_slice(&[
            3, b'w', b'w', b'w', 7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 0,
        ]);
        packet.extend_from_slice(&rr_type.to_be_bytes());
        packet.extend_from_slice(&CLASS_IN.to_be_bytes());
        packet.extend_from_slice(&[0xc0, 0x0c]);
        packet.extend_from_slice(&rr_type.to_be_bytes());
        packet.extend_from_slice(&CLASS_IN.to_be_bytes());
        packet.extend_from_slice(&4u32.to_be_bytes());
        packet.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
        packet.extend_from_slice(rdata);
        packet
    }

    #[test]
    fn query_contains_edns_opt() {
        let mut packet = [0u8; 128];
        let len = build_query_with_txid("www.example", TYPE_AAAA, 0x1234, &mut packet);
        assert!(matches!(len, Ok(_)));
        if let Ok(len) = len {
            assert_eq!(&packet[0..2], &0x1234u16.to_be_bytes());
            assert_eq!(&packet[10..12], &1u16.to_be_bytes());
            assert_eq!(
                u16::from_be_bytes([packet[len - 10], packet[len - 9]]),
                TYPE_OPT
            );
            assert_eq!(
                u16::from_be_bytes([packet[len - 8], packet[len - 7]]),
                EDNS_UDP_PAYLOAD
            );
        }
    }

    #[test]
    fn injected_a_and_aaaa_resolve_then_age_out() {
        let v4 = response(TYPE_A, &[192, 0, 2, 10]);
        assert_eq!(inject(&v4), Ok(1));
        let v6_addr = [0x20, 1, 0xd, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let v6 = response(TYPE_AAAA, &v6_addr);
        assert_eq!(inject(&v6), Ok(1));
        let found = resolve("www.example", FamilyHint::Any);
        assert!(matches!(found, Ok(list) if list.len() >= 2 && list[0].family == AF_INET6 as u16));
        age_out(5);
        assert!(matches!(
            resolve("www.example", FamilyHint::Any),
            Err(DnsError::NoResolver) | Err(DnsError::WouldBlock)
        ));
    }

    #[test]
    fn compression_loop_is_rejected() {
        let packet = [0u8, 0, 0x81, 0x80, 0, 1, 0, 0, 0, 0, 0, 0, 0xc0, 0x0c];
        assert_eq!(parse_response(&packet), Err(DnsError::Invalid));
    }
}
