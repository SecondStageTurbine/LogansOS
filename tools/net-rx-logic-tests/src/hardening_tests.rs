// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the #218 hardening (the PM's pre-merge AAG, 2026-10-29):
//! forged DHCP replies, RA/NDP validation, the ARP sender filter, group
//! source MACs, and a no-panic sweep over all of them.

use crate::frames::{ARP_REQUEST, ICMP_ECHO, UDP};
use crate::hardening_frames::*;
use crate::rx_logic::*;

const US_MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
const LOCAL: LocalV4 = LocalV4 {
    addr: Some([10, 0, 0, 10]),
    mask: [255, 255, 255, 0],
};

fn expect(awaiting: DhcpAwaiting, server: Option<[u8; 4]>) -> DhcpExpect {
    DhcpExpect {
        awaiting,
        xid: 0x4e45_4f30,
        mac: US_MAC,
        server,
    }
}

#[test]
fn a_group_or_zero_source_mac_is_dropped() {
    for src in [
        [0xff; 6],
        [0x01, 0x00, 0x5e, 0, 0, 1],
        [0x33, 0x33, 0, 0, 0, 1],
        [0; 6],
    ] {
        let mut f = ICMP_ECHO;
        f[6..12].copy_from_slice(&src);
        assert_eq!(
            classify(&f, &LOCAL),
            Verdict::Drop(DropReason::BadSourceMac),
            "{src:x?}"
        );
    }
    assert!(matches!(
        classify(&ICMP_ECHO, &LOCAL),
        Verdict::IcmpEcho { .. }
    ));
}

#[test]
fn dhcp_replies_must_match_the_exchange() {
    let s = Some([10, 0, 0, 2]);
    assert!(dhcp_reply_ok(
        &DHCP_OFFER,
        &expect(DhcpAwaiting::Offer, None)
    ));
    assert!(dhcp_reply_ok(&DHCP_ACK, &expect(DhcpAwaiting::Reply, s)));
    assert!(dhcp_reply_ok(&DHCP_NAK, &expect(DhcpAwaiting::Reply, s)));
    // When BOUND (or INIT), nothing is accepted: no forged ACK, no reflected OFFER.
    assert!(!dhcp_reply_ok(
        &DHCP_ACK,
        &expect(DhcpAwaiting::Nothing, None)
    ));
    assert!(!dhcp_reply_ok(
        &DHCP_OFFER,
        &expect(DhcpAwaiting::Nothing, None)
    ));
    // An ACK while SELECTING, an OFFER while REQUESTING.
    assert!(!dhcp_reply_ok(
        &DHCP_ACK,
        &expect(DhcpAwaiting::Offer, None)
    ));
    assert!(!dhcp_reply_ok(&DHCP_OFFER, &expect(DhcpAwaiting::Reply, s)));
    // Another server than the one requested.
    assert!(!dhcp_reply_ok(
        &DHCP_ACK,
        &expect(DhcpAwaiting::Reply, Some([10, 0, 0, 3]))
    ));
    // Wrong xid, another client's chaddr, a BOOTREQUEST, a bad cookie.
    let mut m = DHCP_ACK;
    m[7] ^= 1;
    assert!(!dhcp_reply_ok(&m, &expect(DhcpAwaiting::Reply, s)));
    let mut m = DHCP_ACK;
    m[33] ^= 1;
    assert!(!dhcp_reply_ok(&m, &expect(DhcpAwaiting::Reply, s)));
    let mut m = DHCP_ACK;
    m[0] = 1;
    assert!(!dhcp_reply_ok(&m, &expect(DhcpAwaiting::Reply, s)));
    let mut m = DHCP_ACK;
    m[236] = 0;
    assert!(!dhcp_reply_ok(&m, &expect(DhcpAwaiting::Reply, s)));
    // Truncations are refused, never read past the end.
    for n in 0..DHCP_ACK.len() {
        let _ = dhcp_reply_ok(&DHCP_ACK[..n], &expect(DhcpAwaiting::Reply, s));
    }
}

#[test]
fn the_dhcp_message_is_found_in_its_frame() {
    // The R1 oracle's DHCP frame carries a 4-byte BOOTP stub.
    assert_eq!(classify(&crate::frames::DHCP, &LOCAL), Verdict::Dhcp);
    assert_eq!(
        dhcp_message(&crate::frames::DHCP),
        Some(&[2u8, 1, 6, 0][..])
    );
    assert_eq!(dhcp_message(&UDP[..20]), None);
}

#[test]
fn arp_senders_are_filtered() {
    assert!(arp_sender_ok(&ARP_REQUEST, &LOCAL));
    let with_ip = |ip: [u8; 4]| {
        let mut f = ARP_REQUEST;
        f[28..32].copy_from_slice(&ip);
        f
    };
    for ip in [
        [10, 0, 0, 10],
        [0; 4],
        [127, 0, 0, 1],
        [224, 0, 0, 1],
        [255; 4],
        [10, 0, 0, 255],
        [240, 0, 0, 1],
    ] {
        assert!(!arp_sender_ok(&with_ip(ip), &LOCAL), "{ip:?}");
    }
    let mut f = ARP_REQUEST;
    f[22] = 0x01; // group sender MAC
    assert!(!arp_sender_ok(&f, &LOCAL));
    // Without an address, our subnet's broadcast isn't known: only the rest applies.
    let none = LocalV4 {
        addr: None,
        mask: [0; 4],
    };
    assert!(arp_sender_ok(&with_ip([10, 0, 0, 255]), &none));
    assert!(!arp_sender_ok(&with_ip([127, 0, 0, 1]), &none));
}

#[test]
fn a_valid_ra_ns_and_na_pass() {
    assert_eq!(classify(&RA, &LOCAL), Verdict::Ipv6);
    assert_eq!(classify_ipv6(&RA, &[]), Verdict6::Ra);
    assert_eq!(classify_ipv6(&NS, &[OURS]), Verdict6::Ndp);
    assert_eq!(classify_ipv6(&NA, &[OURS]), Verdict6::Ndp);
}

#[test]
fn ra_and_ndp_need_hop_limit_255_link_local_and_a_checksum() {
    assert_eq!(
        classify_ipv6(&RA_HOP_64, &[]),
        Verdict6::Drop(DropReason::Ipv6HopLimit)
    );
    assert_eq!(
        classify_ipv6(&RA_GLOBAL_SRC, &[]),
        Verdict6::Drop(DropReason::RaSource)
    );
    let mut f = RA;
    let last = f.len() - 1;
    f[last] ^= 1;
    assert_eq!(
        classify_ipv6(&f, &[]),
        Verdict6::Drop(DropReason::Icmpv6Checksum)
    );
    // A non-zero code.
    let mut f = RA;
    f[14 + 40 + 1] = 1;
    assert!(matches!(classify_ipv6(&f, &[]), Verdict6::Drop(_)));
    // A multicast source.
    let mut f = RA;
    f[14 + 8] = 0xff;
    assert_eq!(
        classify_ipv6(&f, &[]),
        Verdict6::Drop(DropReason::Ipv6BadSource)
    );
    // An extension header (hop-by-hop) or another upper layer.
    let mut f = RA;
    f[14 + 6] = 0;
    assert_eq!(
        classify_ipv6(&f, &[]),
        Verdict6::Drop(DropReason::Ipv6Invalid)
    );
    // A payload length past the frame.
    let mut f = RA;
    f[14 + 4] = 0xff;
    assert_eq!(
        classify_ipv6(&f, &[]),
        Verdict6::Drop(DropReason::Ipv6Invalid)
    );
}

#[test]
fn ipv6_destinations_are_filtered() {
    // The NS is for our solicited-node group, the NA for our address: neither
    // is for us without that address.
    assert_eq!(
        classify_ipv6(&NS, &[]),
        Verdict6::Drop(DropReason::Ipv6NotForUs)
    );
    assert_eq!(
        classify_ipv6(&NA, &[]),
        Verdict6::Drop(DropReason::Ipv6NotForUs)
    );
    let other = [0x20, 1, 0xd, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9];
    assert_eq!(
        classify_ipv6(&NA, &[other]),
        Verdict6::Drop(DropReason::Ipv6NotForUs)
    );
    assert_eq!(
        solicited_node(OURS)[..13],
        [0xff, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0xff]
    );
}

#[test]
fn hardening_sweep_never_panics_and_accepts_only_valid() {
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let seeds: [&[u8]; 6] = [&RA, &NS, &NA, &DHCP_ACK, &DHCP_OFFER, &ARP_REQUEST];
    for _ in 0..200_000 {
        let seed = seeds[(next() % seeds.len() as u64) as usize];
        let mut f = seed.to_vec();
        for _ in 0..1 + next() % 4 {
            match next() % 4 {
                0 if !f.is_empty() => {
                    let i = (next() % f.len() as u64) as usize;
                    f[i] ^= 1 << (next() % 8);
                }
                1 if !f.is_empty() => {
                    let i = (next() % f.len() as u64) as usize;
                    f[i] = next() as u8;
                }
                2 => f.truncate((next() % (f.len() as u64 + 1)) as usize),
                _ => f.extend((0..next() % 16).map(|_| next() as u8)),
            }
        }
        let _ = classify(&f, &LOCAL);
        match classify_ipv6(&f, &[OURS]) {
            Verdict6::Ra => {
                assert_eq!(f[14 + 7], 255);
                assert!(f[14 + 8] == 0xfe && f[14 + 9] & 0xc0 == 0x80);
            }
            Verdict6::Ndp => assert_eq!(f[14 + 7], 255),
            Verdict6::Drop(_) => {}
        }
        let _ = dhcp_message(&f);
        if dhcp_reply_ok(&f, &expect(DhcpAwaiting::Reply, Some([10, 0, 0, 2]))) {
            assert_eq!(f[0], 2);
            assert_eq!(f[28..34], US_MAC);
        }
        if arp_sender_ok(&f, &LOCAL) {
            assert!(f[22] & 1 == 0 && f[28..32] != [10, 0, 0, 10]);
        }
    }
}
