// SPDX-License-Identifier: MPL-2.0
//! Minimal ICMPv6 Router Advertisement parsing for Phase 4 BUILD-2.

extern crate alloc;

use alloc::vec::Vec;

pub const IPV6_NEXT_HEADER_ICMPV6: u8 = 58;
pub const ICMPV6_ROUTER_ADVERTISEMENT: u8 = 134;
pub const ND_OPT_PREFIX_INFORMATION: u8 = 3;
pub const ND_OPT_ROUTE_INFORMATION: u8 = 24;
pub const ND_OPT_RDNSS: u8 = 25;

const ETHERNET_IPV6: u16 = 0x86dd;
const ETHERNET_HEADER_LEN: usize = 14;
const IPV6_HEADER_LEN: usize = 40;
const RA_HEADER_LEN: usize = 16;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Icmpv6Error {
    Invalid,
    Unsupported,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct PrefixInformation {
    pub prefix_len: u8,
    pub flags: u8,
    pub valid_lifetime: u32,
    pub preferred_lifetime: u32,
    pub prefix: [u8; 16],
}

impl PrefixInformation {
    pub const fn autonomous(self) -> bool {
        self.flags & 0x40 != 0
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RouteInformation {
    pub prefix_len: u8,
    pub preference: u8,
    pub route_lifetime: u32,
    pub prefix: [u8; 16],
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RdnssInformation {
    pub lifetime: u32,
    pub servers: Vec<[u8; 16]>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RouterAdvertisement {
    pub router_lifetime: u16,
    pub reachable_time: u32,
    pub retrans_timer: u32,
    pub prefixes: Vec<PrefixInformation>,
    pub routes: Vec<RouteInformation>,
    pub rdnss: Vec<RdnssInformation>,
}

pub fn parse_router_advertisement(frame: &[u8]) -> Result<RouterAdvertisement, Icmpv6Error> {
    let packet = ipv6_payload(frame)?;
    if packet.len() < RA_HEADER_LEN || packet[0] != ICMPV6_ROUTER_ADVERTISEMENT {
        return Err(Icmpv6Error::Unsupported);
    }

    let router_lifetime = u16::from_be_bytes([packet[6], packet[7]]);
    let reachable_time = u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]);
    let retrans_timer = u32::from_be_bytes([packet[12], packet[13], packet[14], packet[15]]);
    let mut prefixes = Vec::new();
    let mut routes = Vec::new();
    let mut rdnss = Vec::new();
    let mut off = RA_HEADER_LEN;

    while off < packet.len() {
        if packet.len() - off < 2 {
            return Err(Icmpv6Error::Invalid);
        }
        let option_type = packet[off];
        let option_len = packet[off + 1] as usize * 8;
        if option_len == 0 || off + option_len > packet.len() {
            return Err(Icmpv6Error::Invalid);
        }

        match option_type {
            ND_OPT_PREFIX_INFORMATION if option_len >= 32 => {
                let mut prefix = [0u8; 16];
                prefix.copy_from_slice(&packet[off + 16..off + 32]);
                prefixes.push(PrefixInformation {
                    prefix_len: packet[off + 2],
                    flags: packet[off + 3],
                    valid_lifetime: u32::from_be_bytes([
                        packet[off + 4],
                        packet[off + 5],
                        packet[off + 6],
                        packet[off + 7],
                    ]),
                    preferred_lifetime: u32::from_be_bytes([
                        packet[off + 8],
                        packet[off + 9],
                        packet[off + 10],
                        packet[off + 11],
                    ]),
                    prefix,
                });
            }
            ND_OPT_ROUTE_INFORMATION if option_len >= 8 => {
                let prefix_len = packet[off + 2];
                let mut prefix = [0u8; 16];
                let prefix_bytes = (prefix_len as usize).div_ceil(8);
                if prefix_bytes > option_len.saturating_sub(8) || prefix_bytes > 16 {
                    return Err(Icmpv6Error::Invalid);
                }
                prefix[..prefix_bytes].copy_from_slice(&packet[off + 8..off + 8 + prefix_bytes]);
                routes.push(RouteInformation {
                    prefix_len,
                    preference: packet[off + 3] & 0x18,
                    route_lifetime: u32::from_be_bytes([
                        packet[off + 4],
                        packet[off + 5],
                        packet[off + 6],
                        packet[off + 7],
                    ]),
                    prefix,
                });
            }
            ND_OPT_RDNSS if option_len >= 24 => {
                let payload_len = option_len.saturating_sub(8);
                if payload_len == 0 || !payload_len.is_multiple_of(16) {
                    return Err(Icmpv6Error::Invalid);
                }
                let mut servers = Vec::new();
                let mut server_off = off + 8;
                while server_off < off + option_len {
                    let mut addr = [0u8; 16];
                    addr.copy_from_slice(&packet[server_off..server_off + 16]);
                    servers.push(addr);
                    server_off += 16;
                }
                rdnss.push(RdnssInformation {
                    lifetime: u32::from_be_bytes([
                        packet[off + 4],
                        packet[off + 5],
                        packet[off + 6],
                        packet[off + 7],
                    ]),
                    servers,
                });
            }
            _ => {}
        }

        off += option_len;
    }

    Ok(RouterAdvertisement {
        router_lifetime,
        reachable_time,
        retrans_timer,
        prefixes,
        routes,
        rdnss,
    })
}

fn ipv6_payload(frame: &[u8]) -> Result<&[u8], Icmpv6Error> {
    let ipv6 = if frame.len() >= ETHERNET_HEADER_LEN + IPV6_HEADER_LEN
        && u16::from_be_bytes([frame[12], frame[13]]) == ETHERNET_IPV6
    {
        &frame[ETHERNET_HEADER_LEN..]
    } else {
        frame
    };
    if ipv6.len() < IPV6_HEADER_LEN || (ipv6[0] >> 4) != 6 {
        return Err(Icmpv6Error::Invalid);
    }
    if ipv6[6] != IPV6_NEXT_HEADER_ICMPV6 {
        return Err(Icmpv6Error::Unsupported);
    }
    let payload_len = u16::from_be_bytes([ipv6[4], ipv6[5]]) as usize;
    if ipv6.len() < IPV6_HEADER_LEN + payload_len {
        return Err(Icmpv6Error::Invalid);
    }
    Ok(&ipv6[IPV6_HEADER_LEN..IPV6_HEADER_LEN + payload_len])
}
