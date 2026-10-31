// SPDX-License-Identifier: MPL-2.0
//! OT-96 R1: the wire receive demux, as pure logic.
//!
//! [`classify`] takes one Ethernet frame from the wire (already copied out of
//! DMA memory) and says where it goes: ARP, IPv6 (NDP and RA only), the DHCP
//! client, a UDP socket, TCP, an ICMP echo reply, or a drop with its reason.
//! Every length or offset the frame claims is checked against the bytes
//! actually present before any slice is taken, so no input can index out of
//! bounds or panic. Kernel-free, so `tools/net-rx-logic-tests` runs its
//! byte-oracle KATs and the mutation sweep on the host.
//!
//! MVP scope (PM, 2026-10-29): IPv4 data only. IPv4 fragments are dropped,
//! as are 802.1Q-tagged frames. No ICMP unreachable is sent: TCP to a closed
//! port gets TCP's own RST, UDP to a closed port is dropped silently.

/// The Ethernet header: destination MAC, source MAC, EtherType.
pub const ETH_HEADER_LEN: usize = 14;
/// The longest untagged frame without FCS.
pub const MAX_FRAME: usize = 1514;

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
pub const ETHERTYPE_VLAN: u16 = 0x8100;
pub const ETHERTYPE_IPV6: u16 = 0x86dd;

pub const IPV4_MIN_HEADER: usize = 20;
pub const UDP_HEADER_LEN: usize = 8;
pub const TCP_MIN_HEADER: usize = 20;
pub const ICMP_HEADER_LEN: usize = 8;

pub const PROTO_ICMP: u8 = 1;
pub const PROTO_TCP: u8 = 6;
pub const PROTO_UDP: u8 = 17;

pub const DHCP_SERVER_PORT: u16 = 67;
pub const DHCP_CLIENT_PORT: u16 = 68;

pub const ICMP_ECHO_REPLY: u8 = 0;
pub const ICMP_ECHO_REQUEST: u8 = 8;

/// The receiving interface's IPv4 configuration: its address, if it has one,
/// and its netmask.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LocalV4 {
    pub addr: Option<[u8; 4]>,
    pub mask: [u8; 4],
}

/// Why a frame was dropped. The discriminant indexes the drop counters.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DropReason {
    /// Shorter than the Ethernet header.
    TooShort = 0,
    /// Longer than [`MAX_FRAME`].
    TooLong,
    /// An 802.1Q tag (dropped in the MVP).
    Vlan,
    /// An EtherType this stack doesn't handle.
    UnknownEthertype,
    /// IPv4 version field is not 4.
    IpVersion,
    /// IHL below 5, or the header runs past the packet.
    IpHeaderLen,
    /// Total length below the header or past the frame.
    IpTotalLen,
    /// IPv4 header checksum fails.
    IpChecksum,
    /// A fragment (MF set or a non-zero offset): dropped in the MVP.
    Fragment,
    /// The destination is not this interface's address or a broadcast.
    NotForUs,
    /// The source is broadcast, multicast, loopback or unspecified.
    BadSource,
    /// UDP length below its header or past the IPv4 payload.
    UdpLength,
    /// A non-zero UDP checksum fails.
    UdpChecksum,
    /// A TCP segment shorter than its minimum header.
    TcpShort,
    /// An ICMP message shorter than its header.
    IcmpShort,
    /// ICMP checksum fails.
    IcmpChecksum,
    /// An ICMP message other than an echo request.
    IcmpNotEcho,
    /// An IPv4 protocol this stack doesn't handle.
    UnknownProtocol,
    /// A group (multicast or broadcast) or all-zero source MAC: no station
    /// sends one, and a reply to it would go to the whole link.
    BadSourceMac,
    /// An ARP sender not learned or answered: our own address, unspecified,
    /// loopback, multicast or reserved, or a broadcast.
    ArpSender,
    /// IPv6: the version, a payload length past the frame, or not ICMPv6
    /// without extension headers (IPv6 data is not in the MVP).
    Ipv6Invalid,
    /// NDP or an RA with a hop limit other than 255 (RFC 4861 §6.1, §7.1).
    Ipv6HopLimit,
    /// IPv6 to an address that is not ours, all-nodes or our solicited-node group.
    Ipv6NotForUs,
    /// IPv6 from a multicast source.
    Ipv6BadSource,
    /// The ICMPv6 checksum (with its pseudo-header) fails.
    Icmpv6Checksum,
    /// An ICMPv6 message other than an RA, NS or NA, or a non-zero code.
    Icmpv6Unsupported,
    /// A Router Advertisement from a source that is not link-local (RFC 4861 §6.1.2).
    RaSource,
}

/// The number of [`DropReason`] reasons.
pub const DROP_KINDS: usize = DropReason::RaSource as usize + 1;

/// The IPv4 envelope of an accepted packet.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ipv4Meta {
    pub src: [u8; 4],
    pub dst: [u8; 4],
    /// The frame's source MAC (an echo reply goes straight back to it).
    pub src_mac: [u8; 6],
}

/// Where a frame goes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Verdict<'a> {
    /// An ARP frame: the whole frame to the ARP layer.
    Arp,
    /// An IPv6 frame: the whole frame to NDP and the RA handler only.
    Ipv6,
    /// A DHCP message for the client (UDP 67 to 68): the whole frame to it.
    Dhcp,
    /// A UDP datagram for a socket.
    Udp {
        ip: Ipv4Meta,
        src_port: u16,
        dst_port: u16,
        payload: &'a [u8],
    },
    /// A TCP segment (header and data, bounded by the IPv4 total length).
    Tcp {
        ip: Ipv4Meta,
        src_port: u16,
        dst_port: u16,
        segment: &'a [u8],
    },
    /// An ICMP echo request: the whole ICMP message.
    IcmpEcho {
        ip: Ipv4Meta,
        message: &'a [u8],
    },
    Drop(DropReason),
}

fn be16(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

/// The ones'-complement sum of `bytes` (odd length padded with a zero),
/// added to `acc`. Not folded.
pub fn sum16(bytes: &[u8], mut acc: u32) -> u32 {
    let mut chunks = bytes.chunks_exact(2);
    for c in &mut chunks {
        acc = acc.wrapping_add(u32::from(u16::from_be_bytes([c[0], c[1]])));
        acc = (acc & 0xffff) + (acc >> 16);
    }
    if let [last] = chunks.remainder() {
        acc = acc.wrapping_add(u32::from(*last) << 8);
        acc = (acc & 0xffff) + (acc >> 16);
    }
    acc
}

/// Fold a [`sum16`] to 16 bits.
pub fn fold(mut acc: u32) -> u16 {
    while acc > 0xffff {
        acc = (acc & 0xffff) + (acc >> 16);
    }
    acc as u16
}

/// The checksum field value for data whose [`sum16`] (with the field zeroed)
/// is `acc`.
pub fn checksum_of(acc: u32) -> u16 {
    !fold(acc)
}

/// Whether data that includes its own checksum field sums correctly.
fn verifies(acc: u32) -> bool {
    fold(acc) == 0xffff
}

/// The IPv4 pseudo-header's sum, for UDP and TCP checksums.
pub fn pseudo_sum(src: [u8; 4], dst: [u8; 4], proto: u8, len: u16) -> u32 {
    let acc = sum16(&src, 0);
    let acc = sum16(&dst, acc);
    sum16(&[0, proto], sum16(&len.to_be_bytes(), acc))
}

fn bad_source(src: [u8; 4], local: &LocalV4) -> bool {
    if src == [0; 4] || src == [255; 4] || src[0] == 127 || (224..=239).contains(&src[0]) {
        return true;
    }
    match local.addr {
        Some(addr) if local.mask != [255; 4] => {
            let bcast = subnet_broadcast(addr, local.mask);
            src == bcast
        }
        _ => false,
    }
}

fn subnet_broadcast(addr: [u8; 4], mask: [u8; 4]) -> [u8; 4] {
    [
        addr[0] | !mask[0],
        addr[1] | !mask[1],
        addr[2] | !mask[2],
        addr[3] | !mask[3],
    ]
}

fn for_us(dst: [u8; 4], local: &LocalV4) -> bool {
    if dst == [255; 4] {
        return true;
    }
    match local.addr {
        Some(addr) => {
            dst == addr || (local.mask != [255; 4] && dst == subnet_broadcast(addr, local.mask))
        }
        None => false,
    }
}

/// Classify one received Ethernet frame (FCS already stripped).
pub fn classify<'a>(frame: &'a [u8], local: &LocalV4) -> Verdict<'a> {
    if frame.len() < ETH_HEADER_LEN {
        return Verdict::Drop(DropReason::TooShort);
    }
    if frame.len() > MAX_FRAME {
        return Verdict::Drop(DropReason::TooLong);
    }
    if frame[6] & 1 != 0 || frame[6..12] == [0; 6] {
        return Verdict::Drop(DropReason::BadSourceMac);
    }
    match be16(frame, 12) {
        ETHERTYPE_ARP => Verdict::Arp,
        ETHERTYPE_IPV6 => Verdict::Ipv6,
        ETHERTYPE_IPV4 => classify_ipv4(frame, local),
        ETHERTYPE_VLAN => Verdict::Drop(DropReason::Vlan),
        _ => Verdict::Drop(DropReason::UnknownEthertype),
    }
}

fn classify_ipv4<'a>(frame: &'a [u8], local: &LocalV4) -> Verdict<'a> {
    let packet = &frame[ETH_HEADER_LEN..];
    if packet.len() < IPV4_MIN_HEADER {
        return Verdict::Drop(DropReason::IpHeaderLen);
    }
    if packet[0] >> 4 != 4 {
        return Verdict::Drop(DropReason::IpVersion);
    }
    let ihl = usize::from(packet[0] & 0x0f) * 4;
    if ihl < IPV4_MIN_HEADER || ihl > packet.len() {
        return Verdict::Drop(DropReason::IpHeaderLen);
    }
    let total = usize::from(be16(packet, 2));
    if total < ihl || total > packet.len() {
        return Verdict::Drop(DropReason::IpTotalLen);
    }
    if !verifies(sum16(&packet[..ihl], 0)) {
        return Verdict::Drop(DropReason::IpChecksum);
    }
    let frag = be16(packet, 6);
    // MF (0x2000) or a non-zero fragment offset (0x1fff); DF (0x4000) is fine.
    if frag & 0x3fff != 0 {
        return Verdict::Drop(DropReason::Fragment);
    }
    let proto = packet[9];
    let src = [packet[12], packet[13], packet[14], packet[15]];
    let dst = [packet[16], packet[17], packet[18], packet[19]];
    let mut src_mac = [0u8; 6];
    src_mac.copy_from_slice(&frame[6..12]);
    let ip = Ipv4Meta { src, dst, src_mac };
    // Trailing Ethernet padding past the total length is not payload.
    let payload = &packet[ihl..total];

    // DHCP replies come to 68 from 67, broadcast or to an address we don't
    // hold yet: checked before the destination filter.
    if proto == PROTO_UDP && payload.len() >= UDP_HEADER_LEN {
        let (sport, dport) = (be16(payload, 0), be16(payload, 2));
        if sport == DHCP_SERVER_PORT && dport == DHCP_CLIENT_PORT {
            return match udp_payload(payload, src, dst) {
                Ok(_) => Verdict::Dhcp,
                Err(d) => Verdict::Drop(d),
            };
        }
    }

    if !for_us(dst, local) {
        return Verdict::Drop(DropReason::NotForUs);
    }
    if bad_source(src, local) {
        return Verdict::Drop(DropReason::BadSource);
    }
    match proto {
        PROTO_UDP => {
            if payload.len() < UDP_HEADER_LEN {
                return Verdict::Drop(DropReason::UdpLength);
            }
            match udp_payload(payload, src, dst) {
                Ok(data) => Verdict::Udp {
                    ip,
                    src_port: be16(payload, 0),
                    dst_port: be16(payload, 2),
                    payload: data,
                },
                Err(d) => Verdict::Drop(d),
            }
        }
        PROTO_TCP => {
            if payload.len() < TCP_MIN_HEADER {
                return Verdict::Drop(DropReason::TcpShort);
            }
            // The TCP layer checks the data offset and the checksum (with the
            // pseudo-header) itself: `tcp::parse_segment`.
            Verdict::Tcp {
                ip,
                src_port: be16(payload, 0),
                dst_port: be16(payload, 2),
                segment: payload,
            }
        }
        PROTO_ICMP => {
            if payload.len() < ICMP_HEADER_LEN {
                return Verdict::Drop(DropReason::IcmpShort);
            }
            if !verifies(sum16(payload, 0)) {
                return Verdict::Drop(DropReason::IcmpChecksum);
            }
            if payload[0] != ICMP_ECHO_REQUEST || payload[1] != 0 {
                return Verdict::Drop(DropReason::IcmpNotEcho);
            }
            Verdict::IcmpEcho {
                ip,
                message: payload,
            }
        }
        _ => Verdict::Drop(DropReason::UnknownProtocol),
    }
}

/// A UDP datagram's payload, after its length and (non-zero) checksum check.
fn udp_payload(udp: &[u8], src: [u8; 4], dst: [u8; 4]) -> Result<&[u8], DropReason> {
    let len = usize::from(be16(udp, 4));
    if len < UDP_HEADER_LEN || len > udp.len() {
        return Err(DropReason::UdpLength);
    }
    let datagram = &udp[..len];
    if be16(udp, 6) != 0 {
        let acc = sum16(datagram, pseudo_sum(src, dst, PROTO_UDP, len as u16));
        if !verifies(acc) {
            return Err(DropReason::UdpChecksum);
        }
    }
    Ok(&datagram[UDP_HEADER_LEN..])
}

/// Write the ICMP echo reply to `request` (a message [`classify`] accepted as
/// [`Verdict::IcmpEcho`]) into `out`: the same identifier, sequence and data,
/// type 0, a fresh checksum. `None` if `out` is too small or `request` is not
/// an echo request.
pub fn icmp_echo_reply(request: &[u8], out: &mut [u8]) -> Option<usize> {
    if request.len() < ICMP_HEADER_LEN
        || request[0] != ICMP_ECHO_REQUEST
        || request[1] != 0
        || out.len() < request.len()
    {
        return None;
    }
    let reply = &mut out[..request.len()];
    reply.copy_from_slice(request);
    reply[0] = ICMP_ECHO_REPLY;
    reply[2] = 0;
    reply[3] = 0;
    let csum = checksum_of(sum16(reply, 0));
    reply[2..4].copy_from_slice(&csum.to_be_bytes());
    Some(request.len())
}

/// An ARP frame: Ethernet header plus the 28-byte IPv4-over-Ethernet body.
pub const ARP_FRAME_LEN: usize = ETH_HEADER_LEN + 28;
const ARP_OP_REQUEST: u16 = 1;
const ARP_OP_REPLY: u16 = 2;

/// If `frame` is a well-formed ARP request (Ethernet, IPv4, op 1) asking for
/// `our_ip`, the requester's MAC and IPv4 address. RFC 826: a host answers a
/// request for its own address, and learns the requester's mapping.
pub fn arp_request_for(frame: &[u8], our_ip: [u8; 4]) -> Option<([u8; 6], [u8; 4])> {
    if frame.len() < ARP_FRAME_LEN || be16(frame, 12) != ETHERTYPE_ARP {
        return None;
    }
    let a = &frame[ETH_HEADER_LEN..ARP_FRAME_LEN];
    // htype 1 (Ethernet), ptype IPv4, hlen 6, plen 4.
    if be16(a, 0) != 1 || be16(a, 2) != ETHERTYPE_IPV4 || a[4] != 6 || a[5] != 4 {
        return None;
    }
    if be16(a, 6) != ARP_OP_REQUEST || a[24..28] != our_ip {
        return None;
    }
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&a[8..14]);
    let ip = [a[14], a[15], a[16], a[17]];
    // A requester claiming a broadcast, multicast or zero MAC, or an
    // unusable IPv4 source, gets no answer (and is not learned).
    if mac == [0xff; 6] || mac == [0; 6] || mac[0] & 1 != 0 || ip == [0; 4] || ip == [255; 4] {
        return None;
    }
    Some((mac, ip))
}

/// The ARP reply telling `their_mac` (at `their_ip`) that `our_ip` is at
/// `our_mac`.
pub fn arp_reply(
    our_mac: [u8; 6],
    our_ip: [u8; 4],
    their_mac: [u8; 6],
    their_ip: [u8; 4],
) -> [u8; ARP_FRAME_LEN] {
    let mut f = [0u8; ARP_FRAME_LEN];
    f[0..6].copy_from_slice(&their_mac);
    f[6..12].copy_from_slice(&our_mac);
    f[12..14].copy_from_slice(&ETHERTYPE_ARP.to_be_bytes());
    f[14..16].copy_from_slice(&1u16.to_be_bytes());
    f[16..18].copy_from_slice(&ETHERTYPE_IPV4.to_be_bytes());
    f[18] = 6;
    f[19] = 4;
    f[20..22].copy_from_slice(&ARP_OP_REPLY.to_be_bytes());
    f[22..28].copy_from_slice(&our_mac);
    f[28..32].copy_from_slice(&our_ip);
    f[32..38].copy_from_slice(&their_mac);
    f[38..42].copy_from_slice(&their_ip);
    f
}

/// Whether an ARP frame's sender may be learned (a request's or a reply's):
/// a unicast MAC, and an address that is not ours, unspecified, loopback,
/// multicast or reserved, the limited broadcast, or our subnet's broadcast.
pub fn arp_sender_ok(frame: &[u8], local: &LocalV4) -> bool {
    if frame.len() < ARP_FRAME_LEN || be16(frame, 12) != ETHERTYPE_ARP {
        return false;
    }
    let a = &frame[ETH_HEADER_LEN..ARP_FRAME_LEN];
    if be16(a, 0) != 1 || be16(a, 2) != ETHERTYPE_IPV4 || a[4] != 6 || a[5] != 4 {
        return false;
    }
    let mac = &a[8..14];
    if mac[0] & 1 != 0 || mac == [0; 6] {
        return false;
    }
    let ip = [a[14], a[15], a[16], a[17]];
    if Some(ip) == local.addr || ip == [0; 4] || ip[0] == 127 || ip[0] >= 224 {
        return false;
    }
    match local.addr {
        Some(ours) if local.mask != [255; 4] => ip != subnet_broadcast(ours, local.mask),
        _ => true,
    }
}

/// The BOOTP/DHCP message in a frame [`classify`] called [`Verdict::Dhcp`].
pub fn dhcp_message(frame: &[u8]) -> Option<&[u8]> {
    let packet = frame.get(ETH_HEADER_LEN..)?;
    let ihl = usize::from(*packet.first()? & 0x0f) * 4;
    let total = usize::from(be16(packet.get(..4)?, 2));
    let udp = packet.get(ihl..total)?;
    let len = usize::from(be16(udp.get(..6)?, 4));
    udp.get(UDP_HEADER_LEN..len)
}

/// What the DHCP client is waiting for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DhcpAwaiting {
    /// SELECTING: an OFFER.
    Offer,
    /// REQUESTING (and, with renewal, RENEWING or REBINDING): an ACK or a NAK.
    Reply,
    /// INIT or BOUND: nothing.
    Nothing,
}

/// What a DHCP reply must match, from the client's side of the exchange.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DhcpExpect {
    pub awaiting: DhcpAwaiting,
    pub xid: u32,
    /// Our MAC: the reply's `chaddr`.
    pub mac: [u8; 6],
    /// When REQUESTING, the server whose OFFER we took: its ACK or NAK must
    /// carry that server identifier (option 54).
    pub server: Option<[u8; 4]>,
}

/// Whether a DHCP message is a reply for this client (RFC 2131 §4.1, §4.3):
/// a BOOTREPLY with our xid and our `chaddr`, of a type the client waits for,
/// and from the server we requested, when we requested one.
pub fn dhcp_reply_ok(message: &[u8], expect: &DhcpExpect) -> bool {
    const FIXED: usize = 240;
    if message.len() < FIXED || message[0] != 2 || message[236..240] != [99, 130, 83, 99] {
        return false;
    }
    if message[4..8] != expect.xid.to_be_bytes() || message[28..34] != expect.mac {
        return false;
    }
    let (mut kind, mut server) = (None, None);
    let mut off = FIXED;
    while off < message.len() {
        let code = message[off];
        if code == 255 {
            break;
        }
        if code == 0 {
            off += 1;
            continue;
        }
        let Some(&len) = message.get(off + 1) else {
            return false;
        };
        let Some(value) = message.get(off + 2..off + 2 + usize::from(len)) else {
            return false;
        };
        match (code, len) {
            (53, 1) => kind = Some(value[0]),
            (54, 4) => server = Some([value[0], value[1], value[2], value[3]]),
            _ => {}
        }
        off += 2 + usize::from(len);
    }
    match expect.awaiting {
        DhcpAwaiting::Offer => kind == Some(2),
        DhcpAwaiting::Reply => {
            (kind == Some(5) || kind == Some(6))
                && (expect.server.is_none() || server == expect.server)
        }
        DhcpAwaiting::Nothing => false,
    }
}

pub const IPV6_HEADER_LEN: usize = 40;
pub const NEXT_HEADER_ICMPV6: u8 = 58;
pub const ICMPV6_RA: u8 = 134;
pub const ICMPV6_NS: u8 = 135;
pub const ICMPV6_NA: u8 = 136;
/// ff02::1, all nodes.
pub const ALL_NODES: [u8; 16] = [0xff, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

/// Where an IPv6 frame goes, in the MVP (NDP and RA only).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Verdict6 {
    /// A Router Advertisement: to the RA handler.
    Ra,
    /// A Neighbor Solicitation or Advertisement: to NDP.
    Ndp,
    Drop(DropReason),
}

/// The solicited-node multicast group of `addr` (RFC 4291 §2.7.1).
pub fn solicited_node(addr: [u8; 16]) -> [u8; 16] {
    [
        0xff, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0xff, addr[13], addr[14], addr[15],
    ]
}

/// Classify an IPv6 frame (one [`classify`] called [`Verdict::Ipv6`]), per
/// RFC 4861 §6.1 and §7.1: ICMPv6 with no extension headers, hop limit 255,
/// a non-multicast source, addressed to one of `ours`, all-nodes or one of
/// their solicited-node groups, with a valid checksum and code 0; an RA only
/// from a link-local source.
pub fn classify_ipv6(frame: &[u8], ours: &[[u8; 16]]) -> Verdict6 {
    let Some(ip) = frame.get(ETH_HEADER_LEN..) else {
        return Verdict6::Drop(DropReason::Ipv6Invalid);
    };
    if ip.len() < IPV6_HEADER_LEN || ip[0] >> 4 != 6 {
        return Verdict6::Drop(DropReason::Ipv6Invalid);
    }
    let len = usize::from(be16(ip, 4));
    if ip[6] != NEXT_HEADER_ICMPV6 || len < 4 || IPV6_HEADER_LEN + len > ip.len() {
        return Verdict6::Drop(DropReason::Ipv6Invalid);
    }
    if ip[7] != 255 {
        return Verdict6::Drop(DropReason::Ipv6HopLimit);
    }
    let mut src = [0u8; 16];
    let mut dst = [0u8; 16];
    src.copy_from_slice(&ip[8..24]);
    dst.copy_from_slice(&ip[24..40]);
    if src[0] == 0xff {
        return Verdict6::Drop(DropReason::Ipv6BadSource);
    }
    let for_us = dst == ALL_NODES || ours.iter().any(|&a| a == dst || solicited_node(a) == dst);
    if !for_us {
        return Verdict6::Drop(DropReason::Ipv6NotForUs);
    }
    let icmp = &ip[IPV6_HEADER_LEN..IPV6_HEADER_LEN + len];
    // Pseudo-header: source, destination, upper-layer length, next header.
    let acc = sum16(&dst, sum16(&src, 0)) + len as u32 + u32::from(NEXT_HEADER_ICMPV6);
    if !verifies(sum16(icmp, acc)) {
        return Verdict6::Drop(DropReason::Icmpv6Checksum);
    }
    if icmp[1] != 0 {
        return Verdict6::Drop(DropReason::Icmpv6Unsupported);
    }
    match icmp[0] {
        ICMPV6_RA if src[0] == 0xfe && src[1] & 0xc0 == 0x80 => Verdict6::Ra,
        ICMPV6_RA => Verdict6::Drop(DropReason::RaSource),
        ICMPV6_NS | ICMPV6_NA => Verdict6::Ndp,
        _ => Verdict6::Drop(DropReason::Icmpv6Unsupported),
    }
}
