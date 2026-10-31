// SPDX-License-Identifier: MPL-2.0
//! Phase 4 BUILD-1b UDP socket objects.

extern crate alloc;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use alloc::{vec, vec::Vec};

use spin::{Lazy, Mutex};

use crate::drivers::net;
use crate::net::arp;
use crate::net::interface;
use crate::net::ipv4;
use crate::net::ndp;
use crate::sched::process::ProcessId;

pub const AF_INET: u32 = 2;
pub const AF_INET6: u32 = 10;
pub const SOCK_DGRAM: u32 = 2;
pub const MAX_DATAGRAM: usize = 2048;
pub(crate) const RX_QUEUE_LIMIT: usize = 16;
const IPV6_HEADER_LEN: usize = 40;
const DEFAULT_HOP_LIMIT: u8 = 64;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SocketError {
    Invalid,
    Busy,
    WouldBlock,
    NotConnected,
    NoRoute,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub enum AddressFamily {
    Inet4,
    Inet6,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct SocketAddr {
    pub family: AddressFamily,
    pub port: u16,
    pub addr: [u8; 16],
}

impl SocketAddr {
    pub fn parse(buf: &[u8]) -> Result<Self, SocketError> {
        if buf.len() >= 8 && u16::from_le_bytes([buf[0], buf[1]]) as u32 == AF_INET {
            let port = u16::from_be_bytes([buf[2], buf[3]]);
            let mut addr = [0u8; 16];
            addr[..4].copy_from_slice(&buf[4..8]);
            return Ok(Self {
                family: AddressFamily::Inet4,
                port,
                addr,
            });
        }
        if buf.len() >= 20 && u16::from_le_bytes([buf[0], buf[1]]) as u32 == AF_INET6 {
            let port = u16::from_be_bytes([buf[2], buf[3]]);
            let mut addr = [0u8; 16];
            addr.copy_from_slice(&buf[4..20]);
            return Ok(Self {
                family: AddressFamily::Inet6,
                port,
                addr,
            });
        }
        Err(SocketError::Invalid)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
struct BindKey {
    family: AddressFamily,
    port: u16,
    addr: [u8; 16],
}

static BOUND_SOCKETS: Lazy<Mutex<BTreeMap<BindKey, Arc<Socket>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
static LAST_EGRESS: Lazy<Mutex<Option<EgressCapture>>> = Lazy::new(|| Mutex::new(None));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum SocketState {
    Unbound,
    Bound(SocketAddr),
    Closed,
}

pub struct Socket {
    family: AddressFamily,
    inner: Mutex<SocketInner>,
    pub handles: crate::net::HandleCount,
}

struct SocketInner {
    state: SocketState,
    rx_queue: VecDeque<Datagram>,
    stats: SocketStats,
}

struct Datagram {
    src: SocketAddr,
    data: Vec<u8>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct SocketStats {
    pub packets_in: u64,
    pub packets_out: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct EgressCapture {
    pub src: SocketAddr,
    pub dst: SocketAddr,
    pub len: usize,
}

impl Socket {
    pub fn new(_owner: ProcessId, family: u32, socktype: u32) -> Result<Self, SocketError> {
        let family = match family {
            AF_INET => AddressFamily::Inet4,
            AF_INET6 => AddressFamily::Inet6,
            _ => return Err(SocketError::Invalid),
        };
        if socktype != SOCK_DGRAM {
            return Err(SocketError::Invalid);
        }
        Ok(Self {
            family,
            inner: Mutex::new(SocketInner {
                state: SocketState::Unbound,
                rx_queue: VecDeque::new(),
                stats: SocketStats::default(),
            }),
            handles: crate::net::HandleCount::default(),
        })
    }

    pub fn bind(self: &Arc<Self>, addr: SocketAddr) -> Result<(), SocketError> {
        if addr.family != self.family || addr.port == 0 {
            return Err(SocketError::Invalid);
        }
        let mut inner = self.inner.lock();
        match inner.state {
            SocketState::Unbound => {}
            SocketState::Bound(_) => return Err(SocketError::Busy),
            SocketState::Closed => return Err(SocketError::Invalid),
        }
        let key = BindKey {
            family: addr.family,
            port: addr.port,
            addr: addr.addr,
        };
        let mut bound = BOUND_SOCKETS.lock();
        if bound.contains_key(&key) {
            return Err(SocketError::Busy);
        }
        bound.insert(key, self.clone());
        inner.state = SocketState::Bound(addr);
        Ok(())
    }

    pub fn send(&self, _data: &[u8]) -> Result<usize, SocketError> {
        Err(SocketError::NotConnected)
    }

    pub fn send_to(&self, dst: SocketAddr, data: &[u8]) -> Result<usize, SocketError> {
        if data.is_empty() || data.len() > MAX_DATAGRAM {
            return Err(SocketError::Invalid);
        }
        let bound_src = match self.inner.lock().state {
            SocketState::Bound(src) => src,
            SocketState::Unbound => return Err(SocketError::NotConnected),
            SocketState::Closed => return Err(SocketError::Invalid),
        };
        if dst.family != bound_src.family {
            return Err(SocketError::Invalid);
        }
        let src = if is_unspecified(&bound_src) {
            if is_loopback(&dst) {
                loopback_source(bound_src, dst)
            } else {
                let mut selected = interface::select_source(dst).ok_or(SocketError::NoRoute)?;
                selected.port = bound_src.port;
                selected
            }
        } else {
            bound_src
        };
        let mut inner = self.inner.lock();
        inner.stats.packets_out = inner.stats.packets_out.saturating_add(1);
        inner.stats.bytes_out = inner.stats.bytes_out.saturating_add(data.len() as u64);
        drop(inner);

        if !is_loopback(&dst) {
            *LAST_EGRESS.lock() = Some(EgressCapture {
                src,
                dst,
                len: data.len(),
            });
            if dst.family == AddressFamily::Inet4 {
                transmit_ipv4_udp(src, dst, data)?;
            } else {
                transmit_ipv6_udp(src, dst, data)?;
            }
            return Ok(data.len());
        }

        let key = BindKey {
            family: dst.family,
            port: dst.port,
            addr: dst.addr,
        };
        let peer = BOUND_SOCKETS
            .lock()
            .get(&key)
            .cloned()
            .ok_or(SocketError::NoRoute)?;
        peer.enqueue(src, data)?;
        Ok(data.len())
    }

    pub(crate) fn enqueue(&self, src: SocketAddr, data: &[u8]) -> Result<(), SocketError> {
        let mut inner = self.inner.lock();
        if !matches!(inner.state, SocketState::Bound(_)) {
            return Err(SocketError::NotConnected);
        }
        if inner.rx_queue.len() >= RX_QUEUE_LIMIT {
            return Err(SocketError::WouldBlock);
        }
        let mut datagram = Vec::with_capacity(data.len());
        datagram.extend_from_slice(data);
        inner.rx_queue.push_back(Datagram {
            src,
            data: datagram,
        });
        inner.stats.packets_in = inner.stats.packets_in.saturating_add(1);
        inner.stats.bytes_in = inner.stats.bytes_in.saturating_add(data.len() as u64);
        Ok(())
    }

    pub fn recv(&self, out: &mut [u8]) -> Result<usize, SocketError> {
        self.recv_from(out).map(|(n, _src)| n)
    }

    pub fn recv_from(&self, out: &mut [u8]) -> Result<(usize, SocketAddr), SocketError> {
        if out.is_empty() {
            return Err(SocketError::Invalid);
        }
        let mut inner = self.inner.lock();
        match inner.rx_queue.pop_front() {
            Some(datagram) => {
                let n = core::cmp::min(out.len(), datagram.data.len());
                out[..n].copy_from_slice(&datagram.data[..n]);
                Ok((n, datagram.src))
            }
            None => Err(SocketError::WouldBlock),
        }
    }

    pub fn stats(&self) -> SocketStats {
        self.inner.lock().stats
    }

    pub fn close(&self) {
        let mut inner = self.inner.lock();
        if let SocketState::Bound(addr) = inner.state {
            BOUND_SOCKETS.lock().remove(&BindKey {
                family: addr.family,
                port: addr.port,
                addr: addr.addr,
            });
        }
        inner.rx_queue.clear();
        inner.state = SocketState::Closed;
    }
}

/// OT-96: deliver a datagram from the wire to the socket bound to `dst`, or
/// to the one bound to its port on the unspecified address. The socket's own
/// queue bound applies (`RX_QUEUE_LIMIT`). No bound socket: `NoRoute`, and
/// the caller drops it (no ICMP unreachable in the MVP).
#[cfg(feature = "wire-rx")]
pub fn deliver_inbound(src: SocketAddr, dst: SocketAddr, data: &[u8]) -> Result<(), SocketError> {
    if data.len() > MAX_DATAGRAM {
        return Err(SocketError::Invalid);
    }
    let exact = BindKey {
        family: dst.family,
        port: dst.port,
        addr: dst.addr,
    };
    let wildcard = BindKey {
        family: dst.family,
        port: dst.port,
        addr: [0; 16],
    };
    let socket = {
        let bound = BOUND_SOCKETS.lock();
        bound.get(&exact).or_else(|| bound.get(&wildcard)).cloned()
    };
    socket.ok_or(SocketError::NoRoute)?.enqueue(src, data)
}

#[cfg(feature = "neo-test")]
pub fn debug_bound_count() -> usize {
    BOUND_SOCKETS.lock().len()
}

#[cfg(feature = "neo-test")]
pub fn debug_last_egress() -> Option<EgressCapture> {
    *LAST_EGRESS.lock()
}

fn is_loopback(addr: &SocketAddr) -> bool {
    match addr.family {
        AddressFamily::Inet4 => addr.addr[..4] == [127, 0, 0, 1],
        AddressFamily::Inet6 => addr.addr == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    }
}

fn is_unspecified(addr: &SocketAddr) -> bool {
    match addr.family {
        AddressFamily::Inet4 => addr.addr[..4] == [0, 0, 0, 0],
        AddressFamily::Inet6 => addr.addr == [0; 16],
    }
}

fn loopback_source(mut src: SocketAddr, dst: SocketAddr) -> SocketAddr {
    match dst.family {
        AddressFamily::Inet4 => src.addr[..4].copy_from_slice(&[127, 0, 0, 1]),
        AddressFamily::Inet6 => src.addr = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    }
    src
}

fn transmit_ipv4_udp(src: SocketAddr, dst: SocketAddr, data: &[u8]) -> Result<(), SocketError> {
    // ISC-3: egress on the interface that owns the selected source (interface 0 when
    // `interface-identity` is off — bit-for-bit the old `get_device(0)`).
    let iface = crate::net::interface::egress_interface_for_source(&src);
    let device = net::get_device(iface as usize).ok_or(SocketError::NoRoute)?;
    let src_ip = ipv4_addr(src);
    let dst_ip = ipv4_addr(dst);
    let src_mac = device.mac_address();
    let dst_mac = if is_ipv4_broadcast_or_multicast(dst_ip) {
        [0xff; 6]
    } else {
        [0; 6]
    };
    let frame_len = ipv4::ETHERNET_HEADER_LEN
        .saturating_add(ipv4::IPV4_HEADER_LEN)
        .saturating_add(ipv4::UDP_HEADER_LEN)
        .saturating_add(data.len());
    let mut bytes = vec![0u8; frame_len];
    let len = ipv4::build_ethernet_udp_frame(
        src_mac, dst_mac, src_ip, dst_ip, src.port, dst.port, data, &mut bytes,
    )
    .map_err(|_| SocketError::Invalid)?;
    bytes.truncate(len);
    if is_ipv4_broadcast_or_multicast(dst_ip) {
        let mut frame = device.tx_reserve().ok_or(SocketError::NoRoute)?;
        if bytes.len() > frame.len() {
            return Err(SocketError::Invalid);
        }
        frame.as_mut_slice()[..bytes.len()].copy_from_slice(&bytes);
        device
            .tx_submit(frame, bytes.len())
            .map_err(|_| SocketError::NoRoute)?;
        return Ok(());
    }
    arp::submit_or_queue_ipv4(iface, src_ip, dst_ip, bytes).map_err(|_| SocketError::NoRoute)
}

fn transmit_ipv6_udp(src: SocketAddr, dst: SocketAddr, data: &[u8]) -> Result<(), SocketError> {
    // ISC-3: egress on the interface that owns the selected source (interface 0 when
    // `interface-identity` is off — bit-for-bit the old `get_device(0)`).
    let iface = crate::net::interface::egress_interface_for_source(&src);
    let device = net::get_device(iface as usize).ok_or(SocketError::NoRoute)?;
    let udp_len = ipv4::UDP_HEADER_LEN
        .checked_add(data.len())
        .ok_or(SocketError::Invalid)?;
    if udp_len > u16::MAX as usize {
        return Err(SocketError::Invalid);
    }
    let frame_len = ndp::ETHERNET_HEADER_LEN
        .checked_add(IPV6_HEADER_LEN)
        .and_then(|len| len.checked_add(udp_len))
        .ok_or(SocketError::Invalid)?;
    let mut bytes = vec![0u8; frame_len];
    bytes[6..12].copy_from_slice(&device.mac_address());
    bytes[12..14].copy_from_slice(&ndp::ETHERNET_IPV6.to_be_bytes());

    let ip = ndp::ETHERNET_HEADER_LEN;
    bytes[ip] = 0x60;
    bytes[ip + 4..ip + 6].copy_from_slice(&(udp_len as u16).to_be_bytes());
    bytes[ip + 6] = ipv4::IPV4_PROTOCOL_UDP;
    bytes[ip + 7] = DEFAULT_HOP_LIMIT;
    bytes[ip + 8..ip + 24].copy_from_slice(&src.addr);
    bytes[ip + 24..ip + 40].copy_from_slice(&dst.addr);

    let udp = ip + IPV6_HEADER_LEN;
    bytes[udp..udp + 2].copy_from_slice(&src.port.to_be_bytes());
    bytes[udp + 2..udp + 4].copy_from_slice(&dst.port.to_be_bytes());
    bytes[udp + 4..udp + 6].copy_from_slice(&(udp_len as u16).to_be_bytes());
    bytes[udp + 6..udp + 8].copy_from_slice(&0u16.to_be_bytes());
    bytes[udp + ipv4::UDP_HEADER_LEN..udp + ipv4::UDP_HEADER_LEN + data.len()]
        .copy_from_slice(data);
    let checksum = udp_ipv6_checksum(src.addr, dst.addr, &bytes[udp..udp + udp_len]);
    bytes[udp + 6..udp + 8].copy_from_slice(&checksum.to_be_bytes());

    ndp::submit_or_queue_ipv6(iface, src.addr, dst.addr, bytes).map_err(|_| SocketError::NoRoute)
}

fn ipv4_addr(addr: SocketAddr) -> [u8; 4] {
    let mut out = [0u8; 4];
    out.copy_from_slice(&addr.addr[..4]);
    out
}

fn is_ipv4_broadcast_or_multicast(ip: [u8; 4]) -> bool {
    ip == [255, 255, 255, 255] || (224..=239).contains(&ip[0])
}

fn udp_ipv6_checksum(src: [u8; 16], dst: [u8; 16], udp_packet: &[u8]) -> u16 {
    let mut checksum = Checksum::new();
    checksum.add(&src);
    checksum.add(&dst);
    checksum.add(&(udp_packet.len() as u32).to_be_bytes());
    checksum.add(&[0, 0, 0, ipv4::IPV4_PROTOCOL_UDP]);
    checksum.add(udp_packet);
    let checksum = checksum.finish();
    if checksum == 0 { 0xffff } else { checksum }
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

impl Drop for Socket {
    fn drop(&mut self) {
        self.close();
    }
}
