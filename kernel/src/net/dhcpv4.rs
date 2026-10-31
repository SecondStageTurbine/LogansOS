// SPDX-License-Identifier: MPL-2.0
//! Minimal DHCPv4 state machine for Phase 4 BUILD-3.

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use spin::{Lazy, Mutex};

use crate::drivers::net;
use crate::net::dns;
use crate::net::ipv4;

pub const DHCP_CLIENT_PORT: u16 = 68;
pub const DHCP_SERVER_PORT: u16 = 67;
const DHCP_FIXED_LEN: usize = 240;
const DHCP_MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99];
const DEFAULT_LEASE_TICKS: u64 = 3600;

static CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);
static XID_COUNTER: AtomicU32 = AtomicU32::new(0x4e45_4f30);
static CLIENT: Lazy<Mutex<DhcpClient>> = Lazy::new(|| Mutex::new(DhcpClient::new(0)));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DhcpError {
    Invalid,
    Unsupported,
    BufferTooSmall,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DhcpMessageType {
    Discover = 1,
    Offer = 2,
    Request = 3,
    Ack = 5,
    Nak = 6,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DhcpState {
    Init,
    Selecting,
    Requesting,
    Bound,
    Renewing,
    Rebinding,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Lease {
    pub ip: [u8; 4],
    pub mask: [u8; 4],
    pub router: Option<[u8; 4]>,
    pub dns: Option<[u8; 4]>,
    pub server_id: Option<[u8; 4]>,
    pub t1: u64,
    pub t2: u64,
    pub expiry: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct DhcpMessage {
    pub message_type: DhcpMessageType,
    pub xid: u32,
    pub yiaddr: [u8; 4],
    pub requested_ip: Option<[u8; 4]>,
    pub server_id: Option<[u8; 4]>,
    pub lease_time: Option<u32>,
    pub subnet_mask: Option<[u8; 4]>,
    pub router: Option<[u8; 4]>,
    pub dns: Option<[u8; 4]>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct DhcpSnapshot {
    pub state: DhcpState,
    pub xid: u32,
    pub offered: Option<Lease>,
    pub lease: Option<Lease>,
    pub acd: AcdState,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct DhcpClient {
    interface_id: u32,
    state: DhcpState,
    xid: u32,
    offered: Option<Lease>,
    lease: Option<Lease>,
    acd: AcdState,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum AcdState {
    Idle,
    Probing,
    Announcing,
    Conflict,
}

impl DhcpClient {
    pub const fn new(interface_id: u32) -> Self {
        Self {
            interface_id,
            state: DhcpState::Init,
            xid: 0,
            offered: None,
            lease: None,
            acd: AcdState::Idle,
        }
    }

    fn start(&mut self) {
        self.xid = XID_COUNTER.fetch_add(1, Ordering::AcqRel);
        self.state = DhcpState::Selecting;
        self.offered = None;
        self.lease = None;
        self.acd = AcdState::Idle;
        let _ = transmit_client_message(
            self.interface_id,
            DhcpMessage {
                message_type: DhcpMessageType::Discover,
                xid: self.xid,
                yiaddr: [0; 4],
                requested_ip: None,
                server_id: None,
                lease_time: None,
                subnet_mask: None,
                router: None,
                dns: None,
            },
        );
    }

    fn apply_offer(&mut self, message: DhcpMessage, now: u64) -> Result<(), DhcpError> {
        if message.message_type != DhcpMessageType::Offer {
            return Err(DhcpError::Unsupported);
        }
        if self.state == DhcpState::Init {
            self.start();
        }
        self.offered = Some(message_to_lease(message, now));
        self.state = DhcpState::Requesting;
        crate::serial_println!("dhcp: request_tx iface={}", self.interface_id);
        let _ = transmit_client_message(
            self.interface_id,
            DhcpMessage {
                message_type: DhcpMessageType::Request,
                xid: self.xid,
                yiaddr: [0; 4],
                requested_ip: Some(message.yiaddr),
                server_id: message.server_id,
                lease_time: None,
                subnet_mask: None,
                router: None,
                dns: None,
            },
        );
        Ok(())
    }

    fn apply_ack(&mut self, message: DhcpMessage, now: u64) -> Result<(), DhcpError> {
        if message.message_type != DhcpMessageType::Ack {
            return Err(DhcpError::Unsupported);
        }
        let lease = message_to_lease(message, now);
        self.lease = Some(lease);
        self.offered = None;
        self.acd = AcdState::Probing;
        self.acd = AcdState::Announcing;
        self.state = DhcpState::Bound;
        ipv4::apply_lease(
            self.interface_id,
            lease.ip,
            lease.mask,
            lease.router,
            Some(lease.expiry.saturating_sub(now)),
        );
        Ok(())
    }

    fn tick(&mut self, now: u64) {
        if let Some(lease) = self.lease {
            if self.state == DhcpState::Bound && now >= lease.t1 {
                self.state = DhcpState::Renewing;
            }
            if self.state == DhcpState::Renewing && now >= lease.t2 {
                self.state = DhcpState::Rebinding;
            }
            if now >= lease.expiry {
                self.state = DhcpState::Init;
                self.lease = None;
            }
        }
    }
}

pub fn inject_acd_conflict() {
    let mut client = CLIENT.lock();
    client.acd = AcdState::Conflict;
    client.state = DhcpState::Init;
    client.lease = None;
}

pub fn start_discover() -> u32 {
    let mut client = CLIENT.lock();
    client.start();
    client.xid
}

pub fn inject_offer(packet: &[u8]) -> Result<(), DhcpError> {
    let message = parse_message(packet)?;
    CLIENT.lock().apply_offer(message, now())
}

pub fn inject_ack(packet: &[u8]) -> Result<(), DhcpError> {
    let message = parse_message(packet)?;
    let result = CLIENT.lock().apply_ack(message, now());
    if result.is_ok() {
        dns::configure_from_dhcp_lease(snapshot().lease);
    }
    result
}

/// CB-1b kill-switch: gates the production wire-RX→DHCP demux. Off ⇒ the
/// pre-CB-1b inject-only baseline (received DHCP is not auto-routed to the
/// client), bit-for-bit. Default ON.
static RX_DEMUX_ENABLED: AtomicBool = AtomicBool::new(true);

/// CB-1b ISC-5 kill-switch toggle.
#[allow(dead_code)]
pub fn set_rx_demux_enabled(enabled: bool) {
    RX_DEMUX_ENABLED.store(enabled, Ordering::Release);
}

/// CB-1b: bind the DHCP client to a specific interface index. The
/// Discover/Request are transmitted on this device (`transmit_client_message`
/// selects `net::get_device(interface_id)`), so wlan0 — which may not be index 0
/// — must set this to its registered index, else DHCP never leaves wlan0.
#[allow(dead_code)]
pub fn set_interface_id(id: u32) {
    CLIENT.lock().interface_id = id;
}

/// CB-1b ISC-1 — the production wire-RX→DHCP bridge (the piece CB-1 lacked).
/// Given a received ETHERNET frame, if it is a UDP datagram to the DHCP client
/// port (68), parse it and feed the state machine: an Offer auto-sends the
/// Request inside `apply_offer` (full DORA over the wire), an Ack binds the
/// lease. Returns the consumed message type, or `None`. This promotes the proven
/// test-harness `classify_ipv4` demux into a path the bare-metal wlan0 loop (and
/// every device-RX consumer) can call. Gated by the ISC-5 kill-switch.
pub fn demux_eth_rx(eth: &[u8]) -> Option<DhcpMessageType> {
    // OT-39(2): Acquire pairs with `set_rx_demux_enabled`'s Release store so an
    // enable is fully published before this read observes it (the write side was
    // already Release; the read was Relaxed, giving no ordering across the pair).
    if !RX_DEMUX_ENABLED.load(Ordering::Acquire) {
        return None;
    }
    // Ethernet(14) + IPv4(ihl*4) + UDP(8); EtherType IPv4 = 0x0800, proto = UDP.
    if eth.len() < 14 + 20 + 8 || u16::from_be_bytes([eth[12], eth[13]]) != 0x0800 {
        return None;
    }
    // OT-39(3): RFC 791/768 length validation — hygiene (parse_message already
    // rejects a short DHCP body, so this is not a bounds fix), but respect the
    // declared header lengths: IHL >= 5 (min IPv4 header), the IP Total-Length
    // covers IP+UDP and fits the frame, and the UDP-Length is >= 8 and in-frame.
    // The payload is then bounded by the UDP datagram, not trailing frame bytes.
    if (eth[14] & 0x0f) < 5 {
        return None;
    }
    let ihl = ((eth[14] & 0x0f) as usize) * 4;
    let ip_total = u16::from_be_bytes([eth[16], eth[17]]) as usize;
    if ip_total < ihl + 8 || 14 + ip_total > eth.len() {
        return None;
    }
    let udp = 14 + ihl;
    if eth.len() < udp + 8 || eth[14 + 9] != ipv4::IPV4_PROTOCOL_UDP {
        return None;
    }
    let udp_len = u16::from_be_bytes([eth[udp + 4], eth[udp + 5]]) as usize;
    if udp_len < 8 || udp + udp_len > eth.len() {
        return None;
    }
    if u16::from_be_bytes([eth[udp + 2], eth[udp + 3]]) != DHCP_CLIENT_PORT {
        return None;
    }
    let payload = &eth[udp + 8..udp + udp_len];
    let message = parse_message(payload).ok()?;
    // OT-96: with wire receive any LAN host's DHCP traffic reaches here for
    // the whole uptime. Accept only a reply to this client's exchange: its
    // xid and MAC, a type its state waits for, and the server it requested.
    #[cfg(feature = "wire-rx")]
    if !reply_for_us(payload) {
        DHCP_STRAYS.fetch_add(1, Ordering::Relaxed);
        return None;
    }
    match message.message_type {
        DhcpMessageType::Offer => {
            let ip = message.yiaddr;
            crate::serial_println!(
                "dhcp: offer_rx ip={}.{}.{}.{} (request_tx follows)",
                ip[0],
                ip[1],
                ip[2],
                ip[3]
            );
            let _ = inject_offer(payload);
        }
        DhcpMessageType::Ack => {
            let ip = message.yiaddr;
            crate::serial_println!("dhcp: ack_rx ip={}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
            let _ = inject_ack(payload);
        }
        _ => return None,
    }
    Some(message.message_type)
}

/// OT-96: replies dropped as not for this client's exchange.
#[cfg(feature = "wire-rx")]
static DHCP_STRAYS: AtomicU64 = AtomicU64::new(0);

/// OT-96: DHCP replies dropped as not for this client's exchange.
#[cfg(feature = "wire-rx")]
pub fn strays() -> u64 {
    DHCP_STRAYS.load(Ordering::Relaxed)
}

#[cfg(feature = "wire-rx")]
fn reply_for_us(message: &[u8]) -> bool {
    use crate::net::rx_logic::{self, DhcpAwaiting, DhcpExpect};
    let (state, xid, interface_id, server) = {
        let client = CLIENT.lock();
        (
            client.state,
            client.xid,
            client.interface_id,
            client.offered.and_then(|o| o.server_id),
        )
    };
    let awaiting = match state {
        DhcpState::Selecting => DhcpAwaiting::Offer,
        DhcpState::Requesting | DhcpState::Renewing | DhcpState::Rebinding => DhcpAwaiting::Reply,
        DhcpState::Init | DhcpState::Bound => DhcpAwaiting::Nothing,
    };
    let Some(device) = net::get_device(interface_id as usize) else {
        return false;
    };
    let expect = DhcpExpect {
        awaiting,
        xid,
        mac: device.mac_address(),
        server: server.filter(|_| state == DhcpState::Requesting),
    };
    rx_logic::dhcp_reply_ok(message, &expect)
}

pub fn tick() {
    let now = CLOCK_TICKS.fetch_add(1, Ordering::AcqRel) + 1;
    CLIENT.lock().tick(now);
}

pub fn snapshot() -> DhcpSnapshot {
    let client = CLIENT.lock();
    DhcpSnapshot {
        state: client.state,
        xid: client.xid,
        offered: client.offered,
        lease: client.lease,
        acd: client.acd,
    }
}

pub fn now() -> u64 {
    CLOCK_TICKS.load(Ordering::Acquire)
}

pub fn parse_message(packet: &[u8]) -> Result<DhcpMessage, DhcpError> {
    if packet.len() < DHCP_FIXED_LEN {
        return Err(DhcpError::Invalid);
    }
    if packet[236..240] != DHCP_MAGIC_COOKIE[..] {
        return Err(DhcpError::Invalid);
    }
    let mut yiaddr = [0u8; 4];
    yiaddr.copy_from_slice(&packet[16..20]);
    let xid = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
    let mut message_type = None;
    let mut requested_ip = None;
    let mut server_id = None;
    let mut lease_time = None;
    let mut subnet_mask = None;
    let mut router = None;
    let mut dns = None;
    parse_options(
        &packet[240..],
        &mut message_type,
        &mut requested_ip,
        &mut server_id,
        &mut lease_time,
        &mut subnet_mask,
        &mut router,
        &mut dns,
    )?;
    let Some(message_type) = message_type else {
        return Err(DhcpError::Invalid);
    };
    Ok(DhcpMessage {
        message_type,
        xid,
        yiaddr,
        requested_ip,
        server_id,
        lease_time,
        subnet_mask,
        router,
        dns,
    })
}

pub fn build_message(message: DhcpMessage, out: &mut [u8]) -> Result<usize, DhcpError> {
    if out.len() < DHCP_FIXED_LEN {
        return Err(DhcpError::BufferTooSmall);
    }
    out[..DHCP_FIXED_LEN].fill(0);
    out[0] = 1;
    out[1] = 1;
    out[2] = 6;
    out[4..8].copy_from_slice(&message.xid.to_be_bytes());
    out[16..20].copy_from_slice(&message.yiaddr);
    out[236..240].copy_from_slice(&DHCP_MAGIC_COOKIE);
    let mut options = Vec::new();
    push_option(&mut options, 53, &[message.message_type as u8]);
    if let Some(ip) = message.requested_ip {
        push_option(&mut options, 50, &ip);
    }
    if let Some(server) = message.server_id {
        push_option(&mut options, 54, &server);
    }
    if let Some(lease) = message.lease_time {
        push_option(&mut options, 51, &lease.to_be_bytes());
    }
    if let Some(mask) = message.subnet_mask {
        push_option(&mut options, 1, &mask);
    }
    if let Some(router) = message.router {
        push_option(&mut options, 3, &router);
    }
    if let Some(dns) = message.dns {
        push_option(&mut options, 6, &dns);
    }
    options.push(255);
    if out.len() < DHCP_FIXED_LEN + options.len() {
        return Err(DhcpError::BufferTooSmall);
    }
    out[DHCP_FIXED_LEN..DHCP_FIXED_LEN + options.len()].copy_from_slice(&options);
    Ok(DHCP_FIXED_LEN + options.len())
}

fn transmit_client_message(interface_id: u32, message: DhcpMessage) -> Result<(), DhcpError> {
    let device = net::get_device(interface_id as usize).ok_or(DhcpError::Unsupported)?;
    let mut dhcp = vec![0u8; 576];
    let dhcp_len = build_message(message, &mut dhcp)?;
    // build_message has no MAC, so it leaves the BOOTP chaddr + flags zero. A
    // real DHCP server needs chaddr to address its reply and silently drops a
    // request with an empty chaddr (QEMU/slirp tolerated it, which masked this
    // until the bare-metal AX88179 run: DISCOVERs reached the wire but drew no
    // offer). Fill chaddr with the client MAC and set the broadcast flag so the
    // server broadcasts the offer/ack (the client holds no IP yet to receive a
    // unicast reply).
    let mac = device.mac_address();
    dhcp[28..34].copy_from_slice(&mac); // chaddr (first 6 of the 16-byte field)
    dhcp[10] |= 0x80; // flags: BOOTP broadcast bit (big-endian high byte)
    dhcp.truncate(dhcp_len);

    let frame_len = ipv4::ETHERNET_HEADER_LEN
        .saturating_add(ipv4::IPV4_HEADER_LEN)
        .saturating_add(ipv4::UDP_HEADER_LEN)
        .saturating_add(dhcp.len());
    let mut bytes = vec![0u8; frame_len];
    let len = ipv4::build_ethernet_udp_frame(
        device.mac_address(),
        [0xff; 6],
        [0, 0, 0, 0],
        [255, 255, 255, 255],
        DHCP_CLIENT_PORT,
        DHCP_SERVER_PORT,
        &dhcp,
        &mut bytes,
    )
    .map_err(|_| DhcpError::BufferTooSmall)?;
    let mut frame = device.tx_reserve().ok_or(DhcpError::Unsupported)?;
    if len > frame.len() {
        return Err(DhcpError::BufferTooSmall);
    }
    frame.as_mut_slice()[..len].copy_from_slice(&bytes[..len]);
    device
        .tx_submit(frame, len)
        .map_err(|_| DhcpError::Unsupported)
}

#[allow(clippy::too_many_arguments)] // parses DHCP options into caller-owned output slots without allocation
fn parse_options(
    options: &[u8],
    message_type: &mut Option<DhcpMessageType>,
    requested_ip: &mut Option<[u8; 4]>,
    server_id: &mut Option<[u8; 4]>,
    lease_time: &mut Option<u32>,
    subnet_mask: &mut Option<[u8; 4]>,
    router: &mut Option<[u8; 4]>,
    dns: &mut Option<[u8; 4]>,
) -> Result<(), DhcpError> {
    let mut off = 0;
    while off < options.len() {
        let code = options[off];
        off += 1;
        if code == 0 {
            continue;
        }
        if code == 255 {
            return Ok(());
        }
        if off >= options.len() {
            return Err(DhcpError::Invalid);
        }
        let len = options[off] as usize;
        off += 1;
        if off + len > options.len() {
            return Err(DhcpError::Invalid);
        }
        let value = &options[off..off + len];
        match code {
            53 if len == 1 => {
                *message_type = match value[0] {
                    1 => Some(DhcpMessageType::Discover),
                    2 => Some(DhcpMessageType::Offer),
                    3 => Some(DhcpMessageType::Request),
                    5 => Some(DhcpMessageType::Ack),
                    6 => Some(DhcpMessageType::Nak),
                    _ => return Err(DhcpError::Unsupported),
                };
            }
            50 if len == 4 => *requested_ip = Some(copy_ipv4(value)),
            54 if len == 4 => *server_id = Some(copy_ipv4(value)),
            51 if len == 4 => {
                *lease_time = Some(u32::from_be_bytes([value[0], value[1], value[2], value[3]]));
            }
            1 if len == 4 => *subnet_mask = Some(copy_ipv4(value)),
            3 if len >= 4 => *router = Some(copy_ipv4(&value[..4])),
            6 if len >= 4 => *dns = Some(copy_ipv4(&value[..4])),
            _ => {}
        }
        off += len;
    }
    Err(DhcpError::Invalid)
}

fn message_to_lease(message: DhcpMessage, now: u64) -> Lease {
    let lease_ticks = message.lease_time.unwrap_or(DEFAULT_LEASE_TICKS as u32) as u64;
    Lease {
        ip: if message.yiaddr == [0; 4] {
            message.requested_ip.unwrap_or([0; 4])
        } else {
            message.yiaddr
        },
        mask: message.subnet_mask.unwrap_or([255, 255, 255, 0]),
        router: message.router,
        dns: message.dns,
        server_id: message.server_id,
        t1: now.saturating_add(lease_ticks / 2),
        t2: now.saturating_add(lease_ticks.saturating_mul(7) / 8),
        expiry: now.saturating_add(lease_ticks),
    }
}

fn push_option(options: &mut Vec<u8>, code: u8, value: &[u8]) {
    options.push(code);
    options.push(value.len() as u8);
    options.extend_from_slice(value);
}

fn copy_ipv4(value: &[u8]) -> [u8; 4] {
    let mut out = [0u8; 4];
    out.copy_from_slice(&value[..4]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_and_ack_advance_state() {
        let xid = start_discover();
        let offer = DhcpMessage {
            message_type: DhcpMessageType::Offer,
            xid,
            yiaddr: [192, 0, 2, 44],
            requested_ip: None,
            server_id: Some([192, 0, 2, 1]),
            lease_time: Some(8),
            subnet_mask: Some([255, 255, 255, 0]),
            router: Some([192, 0, 2, 1]),
            dns: Some([192, 0, 2, 53]),
        };
        let mut bytes = [0u8; 320];
        let n = match build_message(offer, &mut bytes) {
            Ok(n) => n,
            Err(_) => {
                assert!(false);
                return;
            }
        };
        assert!(inject_offer(&bytes[..n]).is_ok());
        assert_eq!(snapshot().state, DhcpState::Requesting);

        let mut ack = offer;
        ack.message_type = DhcpMessageType::Ack;
        let n = match build_message(ack, &mut bytes) {
            Ok(n) => n,
            Err(_) => {
                assert!(false);
                return;
            }
        };
        assert!(inject_ack(&bytes[..n]).is_ok());
        assert_eq!(snapshot().state, DhcpState::Bound);
        for _ in 0..4 {
            tick();
        }
        assert_eq!(snapshot().state, DhcpState::Renewing);
    }
}
