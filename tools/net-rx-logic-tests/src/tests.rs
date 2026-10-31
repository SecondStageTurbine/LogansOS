// SPDX-License-Identifier: MPL-2.0
use crate::egress_logic::{EgressPass, egress_pass, push_capped, round};
use crate::frames::*;
use crate::rx_logic::*;

const US: LocalV4 = LocalV4 {
    addr: Some([10, 0, 0, 10]),
    mask: [255, 255, 255, 0],
};
const NO_ADDR: LocalV4 = LocalV4 {
    addr: None,
    mask: [0, 0, 0, 0],
};

/// The test's own RFC 1071 checksum, independent of the parser's.
fn rfc1071(bytes: &[u8]) -> u16 {
    let mut s: u32 = 0;
    for c in bytes.chunks(2) {
        let w = if c.len() == 2 {
            u32::from(c[0]) << 8 | u32::from(c[1])
        } else {
            u32::from(c[0]) << 8
        };
        s += w;
    }
    while s >> 16 != 0 {
        s = (s & 0xffff) + (s >> 16);
    }
    !(s as u16)
}

/// Re-seal the IPv4 header checksum after editing a header field (so the
/// case reaches the check it targets).
fn reseal_ip(f: &mut [u8]) {
    let ihl = usize::from(f[14] & 0x0f) * 4;
    f[24] = 0;
    f[25] = 0;
    let c = rfc1071(&f[14..14 + ihl]);
    f[24..26].copy_from_slice(&c.to_be_bytes());
}

fn drop_of(v: Verdict<'_>) -> Option<DropReason> {
    match v {
        Verdict::Drop(d) => Some(d),
        _ => None,
    }
}

// --- the accepted paths -------------------------------------------------------------

#[test]
fn udp_datagram_reaches_its_port() {
    match classify(&UDP, &US) {
        Verdict::Udp {
            ip,
            src_port,
            dst_port,
            payload,
        } => {
            assert_eq!(ip.src, [10, 0, 0, 2]);
            assert_eq!(ip.dst, [10, 0, 0, 10]);
            assert_eq!(ip.src_mac, [0x52, 0x54, 0x00, 0x12, 0x34, 0x57]);
            assert_eq!((src_port, dst_port), (40000, 7001));
            assert_eq!(payload, b"hello");
        }
        v => panic!("{v:?}"),
    }
}

#[test]
fn tcp_segment_is_bounded_by_total_length() {
    match classify(&TCP_SYN, &US) {
        Verdict::Tcp {
            src_port,
            dst_port,
            segment,
            ..
        } => {
            assert_eq!((src_port, dst_port), (40001, 7002));
            assert_eq!(segment, &TCP_SYN[34..]);
            assert_eq!(segment.len(), 20);
        }
        v => panic!("{v:?}"),
    }
}

#[test]
fn icmp_echo_request_and_its_reply_bytes() {
    let message = match classify(&ICMP_ECHO, &US) {
        Verdict::IcmpEcho { message, .. } => message,
        v => panic!("{v:?}"),
    };
    let mut out = [0u8; 64];
    let n = icmp_echo_reply(message, &mut out).expect("reply");
    // The hand-built reply message: type 0, same id/seq/data, its checksum.
    assert_eq!(&out[..n], &ICMP_REPLY_MSG[..]);
}

#[test]
fn dhcp_reply_goes_to_the_client_even_with_no_address() {
    assert_eq!(classify(&DHCP, &NO_ADDR), Verdict::Dhcp);
    assert_eq!(classify(&DHCP, &US), Verdict::Dhcp);
}

#[test]
fn arp_and_ipv6_are_handed_on_whole() {
    let mut arp = UDP;
    arp[12..14].copy_from_slice(&ETHERTYPE_ARP.to_be_bytes());
    assert_eq!(classify(&arp, &US), Verdict::Arp);
    let mut v6 = UDP;
    v6[12..14].copy_from_slice(&ETHERTYPE_IPV6.to_be_bytes());
    assert_eq!(classify(&v6, &US), Verdict::Ipv6);
}

#[test]
fn broadcast_destinations_are_for_us() {
    for dst in [[255, 255, 255, 255], [10, 0, 0, 255]] {
        let mut f = UDP;
        f[30..34].copy_from_slice(&dst);
        reseal_ip(&mut f);
        // The UDP checksum covers the destination: zero it (optional in IPv4).
        f[40] = 0;
        f[41] = 0;
        assert!(matches!(classify(&f, &US), Verdict::Udp { .. }), "{dst:?}");
    }
}

#[test]
fn ethernet_padding_is_not_payload() {
    let mut f = [0u8; 60];
    f[..UDP.len()].copy_from_slice(&UDP);
    match classify(&f, &US) {
        Verdict::Udp { payload, .. } => assert_eq!(payload, b"hello"),
        v => panic!("{v:?}"),
    }
}

#[test]
fn zero_udp_checksum_means_none() {
    let mut f = UDP;
    f[40] = 0;
    f[41] = 0;
    assert!(matches!(classify(&f, &US), Verdict::Udp { .. }));
}

#[test]
fn dont_fragment_alone_is_accepted() {
    let mut f = UDP;
    f[20] = 0x40;
    reseal_ip(&mut f);
    assert!(matches!(classify(&f, &US), Verdict::Udp { .. }));
}

// --- the malformed corpus: one case per bound ---------------------------------------

#[test]
fn malformed_corpus() {
    type Edit = fn(&mut Vec<u8>);
    let cases: &[(&str, &[u8], Edit, DropReason)] = &[
        ("13 bytes", &UDP, |f| f.truncate(13), DropReason::TooShort),
        (
            "1515 bytes",
            &UDP,
            |f| f.resize(1515, 0),
            DropReason::TooLong,
        ),
        (
            "802.1Q tag",
            &UDP,
            |f| f[12..14].copy_from_slice(&[0x81, 0x00]),
            DropReason::Vlan,
        ),
        (
            "LLDP",
            &UDP,
            |f| f[12..14].copy_from_slice(&[0x88, 0xcc]),
            DropReason::UnknownEthertype,
        ),
        (
            "IPv4 header cut at 19",
            &UDP,
            |f| f.truncate(14 + 19),
            DropReason::IpHeaderLen,
        ),
        (
            "version 6 in an IPv4 EtherType",
            &UDP,
            |f| f[14] = 0x65,
            DropReason::IpVersion,
        ),
        ("IHL 4", &UDP, |f| f[14] = 0x44, DropReason::IpHeaderLen),
        ("IHL 0", &UDP, |f| f[14] = 0x40, DropReason::IpHeaderLen),
        (
            "IHL 15 past the packet",
            &UDP,
            |f| f[14] = 0x4f,
            DropReason::IpHeaderLen,
        ),
        (
            "total length 19",
            &UDP,
            |f| f[16..18].copy_from_slice(&19u16.to_be_bytes()),
            DropReason::IpTotalLen,
        ),
        (
            "total length past the frame",
            &UDP,
            |f| f[16..18].copy_from_slice(&34u16.to_be_bytes()),
            DropReason::IpTotalLen,
        ),
        (
            "total length 0xffff",
            &UDP,
            |f| f[16..18].copy_from_slice(&[0xff, 0xff]),
            DropReason::IpTotalLen,
        ),
        (
            "frame cut inside the payload",
            &UDP,
            |f| f.truncate(40),
            DropReason::IpTotalLen,
        ),
        (
            "header checksum off by one",
            &UDP,
            |f| f[25] ^= 1,
            DropReason::IpChecksum,
        ),
        (
            "more fragments",
            &UDP,
            |f| {
                f[20] = 0x20;
                reseal_ip(f)
            },
            DropReason::Fragment,
        ),
        (
            "fragment offset 1",
            &UDP,
            |f| {
                f[21] = 0x01;
                reseal_ip(f)
            },
            DropReason::Fragment,
        ),
        (
            "another host's address",
            &UDP,
            |f| {
                f[33] = 11;
                reseal_ip(f)
            },
            DropReason::NotForUs,
        ),
        (
            "source broadcast",
            &UDP,
            |f| {
                f[26..30].copy_from_slice(&[255; 4]);
                reseal_ip(f)
            },
            DropReason::BadSource,
        ),
        (
            "source subnet broadcast",
            &UDP,
            |f| {
                f[26..30].copy_from_slice(&[10, 0, 0, 255]);
                reseal_ip(f)
            },
            DropReason::BadSource,
        ),
        (
            "source loopback",
            &UDP,
            |f| {
                f[26..30].copy_from_slice(&[127, 0, 0, 1]);
                reseal_ip(f)
            },
            DropReason::BadSource,
        ),
        (
            "source multicast",
            &UDP,
            |f| {
                f[26..30].copy_from_slice(&[224, 0, 0, 1]);
                reseal_ip(f)
            },
            DropReason::BadSource,
        ),
        (
            "source unspecified",
            &UDP,
            |f| {
                f[26..30].copy_from_slice(&[0; 4]);
                reseal_ip(f)
            },
            DropReason::BadSource,
        ),
        (
            "UDP length 7",
            &UDP,
            |f| f[38..40].copy_from_slice(&7u16.to_be_bytes()),
            DropReason::UdpLength,
        ),
        (
            "UDP length past the packet",
            &UDP,
            |f| f[38..40].copy_from_slice(&14u16.to_be_bytes()),
            DropReason::UdpLength,
        ),
        (
            "UDP header cut",
            &UDP,
            |f| {
                f.truncate(14 + 20 + 7);
                f[16..18].copy_from_slice(&27u16.to_be_bytes());
                reseal_ip(f)
            },
            DropReason::UdpLength,
        ),
        (
            "UDP checksum wrong",
            &UDP,
            |f| f[41] ^= 0x80,
            DropReason::UdpChecksum,
        ),
        (
            "UDP payload flipped",
            &UDP,
            |f| f[44] ^= 0x01,
            DropReason::UdpChecksum,
        ),
        (
            "TCP 19 bytes",
            &TCP_SYN,
            |f| {
                f.truncate(14 + 20 + 19);
                f[16..18].copy_from_slice(&39u16.to_be_bytes());
                reseal_ip(f)
            },
            DropReason::TcpShort,
        ),
        (
            "ICMP 7 bytes",
            &ICMP_ECHO,
            |f| {
                f.truncate(14 + 20 + 7);
                f[16..18].copy_from_slice(&27u16.to_be_bytes());
                reseal_ip(f)
            },
            DropReason::IcmpShort,
        ),
        (
            "ICMP checksum wrong",
            &ICMP_ECHO,
            |f| f[36] ^= 1,
            DropReason::IcmpChecksum,
        ),
        (
            "ICMP timestamp request",
            &ICMP_ECHO,
            |f| {
                f[34] = 13;
                let c = rfc1071(&{
                    let mut m = f[34..].to_vec();
                    m[2] = 0;
                    m[3] = 0;
                    m
                });
                f[36..38].copy_from_slice(&c.to_be_bytes())
            },
            DropReason::IcmpNotEcho,
        ),
        (
            "GRE",
            &UDP,
            |f| {
                f[23] = 47;
                reseal_ip(f)
            },
            DropReason::UnknownProtocol,
        ),
        (
            "DHCP with a bad UDP length",
            &DHCP,
            |f| f[38..40].copy_from_slice(&200u16.to_be_bytes()),
            DropReason::UdpLength,
        ),
        (
            "DHCP with a bad UDP checksum",
            &DHCP,
            |f| f[41] ^= 1,
            DropReason::UdpChecksum,
        ),
    ];
    for (name, base, edit, want) in cases {
        let mut f = base.to_vec();
        edit(&mut f);
        assert_eq!(drop_of(classify(&f, &US)), Some(*want), "{name}");
    }
}

#[test]
fn no_address_drops_unicast() {
    assert_eq!(
        drop_of(classify(&UDP, &NO_ADDR)),
        Some(DropReason::NotForUs)
    );
}

#[test]
fn echo_reply_refuses_non_requests() {
    let mut out = [0u8; 64];
    assert!(icmp_echo_reply(&ICMP_REPLY_MSG, &mut out).is_none());
    assert!(icmp_echo_reply(&ICMP_REPLY_MSG[..7], &mut out).is_none());
    let request = &ICMP_ECHO[34..];
    assert!(icmp_echo_reply(request, &mut out[..request.len() - 1]).is_none());
}

#[test]
fn checksum_helpers_match_rfc1071() {
    for data in [&b""[..], b"\x01", b"\xff\xff", b"hello world", &UDP[..]] {
        assert_eq!(checksum_of(sum16(data, 0)), rfc1071(data), "{data:?}");
    }
}

// --- the mutation sweep -------------------------------------------------------------

/// xorshift64*, seeded through splitmix64 (the OT-86 fuzzer's generator).
struct Rng(u64);

impl Rng {
    fn new(seed: u64, index: u64) -> Rng {
        let mut z = seed ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Rng((z ^ (z >> 31)) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Check what an accepted verdict promises about its slices.
fn check_invariants(frame: &[u8], v: Verdict<'_>) {
    let within = |s: &[u8]| {
        let (b, e) = (
            frame.as_ptr() as usize,
            frame.as_ptr() as usize + frame.len(),
        );
        let (sb, se) = (s.as_ptr() as usize, s.as_ptr() as usize + s.len());
        assert!(sb >= b && se <= e, "slice outside the frame");
    };
    match v {
        Verdict::Udp { payload, .. } => {
            within(payload);
            assert!(
                payload.len() + ETH_HEADER_LEN + IPV4_MIN_HEADER + UDP_HEADER_LEN <= frame.len()
            );
        }
        Verdict::Tcp { segment, .. } => {
            within(segment);
            assert!(segment.len() >= TCP_MIN_HEADER);
        }
        Verdict::IcmpEcho { message, .. } => {
            within(message);
            let mut out = [0u8; MAX_FRAME];
            let n = icmp_echo_reply(message, &mut out).expect("an accepted echo has a reply");
            assert_eq!(rfc1071(&out[..n]), 0, "the reply's checksum verifies");
        }
        Verdict::Arp | Verdict::Ipv6 | Verdict::Dhcp | Verdict::Drop(_) => {}
    }
}

#[test]
fn mutation_sweep_never_panics() {
    const SEED: u64 = 0x0796_0a1c;
    const CASES: u64 = 200_000;
    let pool: [&[u8]; 4] = [&UDP, &TCP_SYN, &ICMP_ECHO, &DHCP];
    let mut accepted = 0u64;
    for index in 0..CASES {
        let mut rng = Rng::new(SEED, index);
        let mut f = pool[rng.below(pool.len())].to_vec();
        for _ in 0..1 + rng.below(4) {
            match rng.below(7) {
                0 if !f.is_empty() => {
                    let i = rng.below(f.len());
                    f[i] ^= 1 << rng.below(8);
                }
                1 if !f.is_empty() => {
                    let i = rng.below(f.len());
                    f[i] = rng.next() as u8;
                }
                2 => f.truncate(rng.below(f.len() + 1)),
                3 => {
                    for _ in 0..rng.below(64) {
                        f.push(rng.next() as u8);
                    }
                }
                // The length fields: IHL, total length, UDP length.
                4 if f.len() > 14 => f[14] = (f[14] & 0xf0) | (rng.next() as u8 & 0x0f),
                5 if f.len() > 17 => f[16..18].copy_from_slice(&(rng.next() as u16).to_be_bytes()),
                6 if f.len() > 39 => f[38..40].copy_from_slice(&(rng.next() as u16).to_be_bytes()),
                _ => {}
            }
        }
        if index % 5 == 0 && f.len() >= 34 && f[14] >> 4 == 4 {
            // Reseal some, so mutated packets also get past the checksum.
            let ihl = usize::from(f[14] & 0x0f) * 4;
            if ihl >= 20 && 14 + ihl <= f.len() {
                reseal_ip(&mut f);
            }
        }
        for local in [US, NO_ADDR] {
            let v = classify(&f, &local);
            if !matches!(v, Verdict::Drop(_)) {
                accepted += 1;
            }
            check_invariants(&f, v);
        }
    }
    // The sweep reaches the accepting paths, not only the early drops.
    assert!(accepted > CASES / 50, "accepted={accepted}");
}

// --- ARP ------------------------------------------------------------------------------

const PEER_MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x57];
const OUR_MAC: [u8; 6] = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];

#[test]
fn arp_request_for_us_and_the_reply_bytes() {
    assert_eq!(classify(&ARP_REQUEST, &US), Verdict::Arp);
    let (mac, ip) = arp_request_for(&ARP_REQUEST, [10, 0, 0, 10]).expect("a request for us");
    assert_eq!((mac, ip), (PEER_MAC, [10, 0, 0, 2]));
    assert_eq!(arp_reply(OUR_MAC, [10, 0, 0, 10], mac, ip), ARP_REPLY);
}

#[test]
fn arp_requests_not_answered() {
    // For another address.
    assert!(arp_request_for(&ARP_REQUEST, [10, 0, 0, 11]).is_none());
    // A reply, not a request.
    assert!(arp_request_for(&ARP_REPLY, [10, 0, 0, 2]).is_none());
    // Truncated.
    assert!(arp_request_for(&ARP_REQUEST[..41], [10, 0, 0, 10]).is_none());
    // Wrong hardware type, protocol type, lengths.
    for (at, v) in [(15usize, 6u8), (16, 0x86), (18, 8), (19, 16)] {
        let mut f = ARP_REQUEST;
        f[at] = v;
        assert!(arp_request_for(&f, [10, 0, 0, 10]).is_none(), "byte {at}");
    }
    // A broadcast or multicast sender MAC, or a broadcast sender IP.
    for (range, bytes) in [
        (22..28, &[0xff; 6][..]),
        (22..28, &[0x01, 0, 0x5e, 0, 0, 1][..]),
        (28..32, &[255; 4][..]),
    ] {
        let mut f = ARP_REQUEST;
        f[range].copy_from_slice(bytes);
        assert!(arp_request_for(&f, [10, 0, 0, 10]).is_none());
    }
}

#[test]
fn arp_sweep_never_panics() {
    for index in 0..50_000u64 {
        let mut rng = Rng::new(0xa4b, index);
        let mut f = ARP_REQUEST.to_vec();
        for _ in 0..1 + rng.below(3) {
            match rng.below(3) {
                0 => {
                    let i = rng.below(f.len());
                    f[i] = rng.next() as u8;
                }
                1 => f.truncate(rng.below(f.len() + 1)),
                _ => f.push(rng.next() as u8),
            }
            if f.is_empty() {
                break;
            }
        }
        let _ = arp_request_for(&f, [10, 0, 0, 10]);
        let _ = classify(&f, &US);
    }
}

/// Chad #189: one egress pass's whole work is its budget's. With a backlog
/// of 1,000 and of 100,000 segments (a kept one first, then wire ones), a
/// pass with budget 16 allocates the same bytes, takes 16, sends 15, and
/// leaves every untouched segment where it was. The first bounded flush
/// snapshot the queue and rebuilt it under the lock, so its allocations
/// grew with the backlog (Chad measured 63,040 bytes at 1,000 and
/// 6,399,040 at 100,000).
#[test]
fn an_egress_pass_does_the_same_work_whatever_the_backlog() {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    let mut costs = Vec::new();
    for backlog in [1_000usize, 100_000] {
        // As the kernel makes it: room for the backlog plus a put-back.
        let mut q: VecDeque<(u32, bool)> = VecDeque::with_capacity(backlog + 128);
        q.push_back((0, true)); // kept (loopback, for a flush)
        for n in 1..backlog as u32 {
            q.push_back((n, false));
        }
        let q = RefCell::new(q);
        let mut sent = Vec::with_capacity(16);
        let before = crate::alloc_count::bytes();
        let pass = egress_pass(
            |f| f(&mut q.borrow_mut()),
            16,
            |seg: &(u32, bool)| seg.1,
            |seg| sent.push(seg.0),
        );
        let cost = crate::alloc_count::bytes() - before;
        costs.push(cost);
        assert_eq!(
            pass,
            EgressPass {
                taken: 16,
                sent: 15,
                more: true
            }
        );
        assert_eq!(sent, (1..16).collect::<Vec<_>>());
        let q = q.into_inner();
        assert_eq!(q.len(), backlog - 15);
        assert_eq!(q[0], (0, true), "the kept one is back at the front");
        assert_eq!(q[1], (16, false), "the rest untouched, in order");
        assert!(q.capacity() >= backlog + 128, "never reallocated");
    }
    assert_eq!(
        costs[0], costs[1],
        "per-pass allocation independent of backlog"
    );
    // Two scratch vectors of at most 16 entries each.
    assert!(
        costs[0] <= 2 * 16 * core::mem::size_of::<(u32, bool)>(),
        "{costs:?}"
    );
}

/// A pass that takes what's there: a short queue empties; a zero budget
/// takes nothing; segments queued between its two lock sections stay behind
/// the kept ones.
#[test]
fn an_egress_pass_keeps_order_and_stops_at_the_queue_end() {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    let q = RefCell::new(VecDeque::from([(1u32, false), (2, true), (3, false)]));
    let mut sent = Vec::new();
    let mut sections = 0;
    let pass = egress_pass(
        |f| {
            sections += 1;
            if sections == 2 {
                // Another CPU queued one between the take and the put-back.
                q.borrow_mut().push_back((9, false));
            }
            f(&mut q.borrow_mut())
        },
        16,
        |seg: &(u32, bool)| seg.1,
        |seg| sent.push(seg.0),
    );
    assert_eq!(
        pass,
        EgressPass {
            taken: 3,
            sent: 2,
            more: true
        }
    );
    assert_eq!(sent, [1, 3]);
    assert_eq!(q.into_inner(), VecDeque::from([(2, true), (9, false)]));
    let q = RefCell::new(VecDeque::from([(1u32, false)]));
    let pass = egress_pass(
        |f| f(&mut q.borrow_mut()),
        0,
        |_| false,
        |_: (u32, bool)| panic!(),
    );
    assert_eq!(
        pass,
        EgressPass {
            taken: 0,
            sent: 0,
            more: true
        }
    );
}

/// The cap: refused at `cap`, accepted below it.
#[test]
fn the_egress_queue_is_capped() {
    let mut q = std::collections::VecDeque::new();
    for n in 0..4 {
        assert!(push_capped(&mut q, 4, n));
    }
    assert!(!push_capped(&mut q, 4, 4));
    assert_eq!(q.len(), 4);
    q.pop_front();
    assert!(push_capped(&mut q, 4, 5));
}

/// Chad #241 1c: `push_capped` is an admission threshold, not a hard cap. A
/// flush takes 16 of 512 and keeps them; meanwhile another CPU's enqueues
/// fill the queue back to the threshold; the put-back then makes 528. The
/// overshoot is what the pass had out, never more.
#[test]
fn the_admission_threshold_is_not_a_hard_cap() {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    const ADMIT: usize = 512;
    let q: RefCell<VecDeque<u32>> = RefCell::new((0..ADMIT as u32).collect());
    let mut other_cpu_ran = false;
    let pass = egress_pass(
        |f| f(&mut q.borrow_mut()),
        16,
        |_| {
            // Outside the lock: another CPU's enqueues, refused at the
            // threshold as usual.
            if !other_cpu_ran {
                other_cpu_ran = true;
                let mut q = q.borrow_mut();
                let mut n = 10_000;
                while push_capped(&mut q, ADMIT, n) {
                    n += 1;
                }
            }
            true
        },
        |_| unreachable!("every segment is kept"),
    );
    assert_eq!((pass.taken, pass.sent), (16, 0));
    assert_eq!(q.borrow().len(), ADMIT + 16);
    // Admission refuses past the threshold all the same.
    assert!(!push_capped(&mut q.borrow_mut(), ADMIT, 0));
}

/// #175 item 4: a tick visits at most its budget of a table, and successive
/// ticks sweep it all, each entry once, then start again.
#[test]
fn a_tick_round_is_bounded_and_a_sweep_covers_every_entry_once() {
    let map: std::collections::BTreeMap<u32, ()> = (0..200).map(|k| (k * 3, ())).collect();
    let mut cursor = None;
    let mut seen = Vec::new();
    let mut sizes = Vec::new();
    for _ in 0..4 {
        let got = round(&map, &mut cursor, 64, |k, _| *k);
        sizes.push(got.len());
        seen.extend(got);
    }
    assert_eq!(sizes, [64, 64, 64, 8]);
    assert_eq!(seen, map.keys().copied().collect::<Vec<_>>());
    assert_eq!(cursor, None, "the sweep ended: the next starts over");
    assert_eq!(round(&map, &mut cursor, 64, |k, _| *k)[0], 0);
    // A key removed under the cursor doesn't stop the round.
    let mut map = map;
    let mut cursor = Some(63 * 3);
    map.remove(&(63 * 3));
    assert_eq!(round(&map, &mut cursor, 64, |k, _| *k)[0], 64 * 3);
}
