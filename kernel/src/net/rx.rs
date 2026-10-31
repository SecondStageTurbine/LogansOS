// SPDX-License-Identifier: MPL-2.0
//! OT-96 R1+R2 (`wire-rx`): wire receive after bring-up.
//!
//! Before this, a NIC's receive ring was polled only inside its bring-up loop
//! (the DHCP exchange), and only ARP and DHCP frames were routed. After boot
//! nothing received from the wire at all. Here one `net-rx` kernel thread
//! services every registered `NetDevice` and routes each frame through
//! [`rx_logic::classify`] (`docs/DESIGN-2026-10-29-ot96-wire-rx.md`).
//!
//! - **Copy, recycle, then parse.** Each frame is copied out of its DMA
//!   buffer into the thread's own buffer, the DMA buffer goes back to the
//!   device, and only the copy is parsed: a device that writes a buffer it no
//!   longer owns (possible with `dma=arena-only`) can't change bytes after
//!   they were checked (I-14(c), no double fetch). Checksum-offload bits are
//!   never read; checksums are verified in software.
//! - **Context and locks (I-14(a), CP-37).** Thread context. Every step that
//!   takes a net or driver lock runs with interrupts off, one device service
//!   or one frame at a time: the syscalls take the same plain spin locks
//!   with IF=0 (SYSCALL's SFMASK clears IF), so a holder preempted IF-on
//!   would deadlock a syscall spinning for it on the same CPU (CP-37; it hung
//!   `socket_teardown` 3 runs in 4 before this). The driver lock is held only
//!   inside `service`, `rx_take` and `rx_recycle`; routing runs with no driver
//!   lock held, and takes only net locks (ARP, the socket table, TCP's
//!   tables, DHCP): never a governance lock. The nap runs IF on.
//! - **Liveness (I-14(b)).** At most [`FRAME_BUDGET`] frames per device per
//!   pass, then a one-tick nap. The pass's egress is [`FLUSH_CHUNKS`]
//!   flushes, each taking at most `tcp::WIRE_FLUSH_BUDGET` segments off the
//!   front of the egress queue and touching nothing else (Chad #189: the
//!   whole call, not just its sends, is bounded), and the queue admits
//!   segments up to a threshold (`tcp::OUTBOUND_ADMIT`). A flood costs the
//!   budget, not the CPU.
//! - **Scope (MVP).** IPv4 data; IPv6 only to NDP and the RA handler. TCP to
//!   a closed port gets TCP's RST; UDP to a closed port is dropped silently;
//!   no ICMP unreachable. Fragments and 802.1Q frames are dropped.
//! - **Feature off.** Not compiled and never started: frames are received
//!   only in bring-up loops, as before.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::rx_logic::{self, DROP_KINDS, LocalV4, MAX_FRAME, Verdict, Verdict6};
use super::socket::{AddressFamily, SocketAddr};
use crate::arch::x86_64::without_interrupts;
use crate::sched::kthread::{self, KthreadExitState, KthreadStopToken};

// The harness scenarios that take the net locks IF-on can't be built with
// this thread: `wire-rx` requires `net-tick`, whose `compile_error!` refuses
// them (`net::periodic`; CP-37, design §11).

/// Frames taken from one device per pass.
pub const FRAME_BUDGET: usize = 24;
/// OT-96 F2, Chad #189: the flushes a pass makes, each IF-off, taking at
/// most `tcp::WIRE_FLUSH_BUDGET` segments off the queue's front and touching
/// nothing else (interrupts on between them). The rest wait for the next
/// pass, so a pass's egress work is bounded whatever the backlog or what
/// other CPUs queue.
pub const FLUSH_CHUNKS: usize = 4;
/// Timer ticks (10 ms) the thread naps between passes.
pub const NAP_TICKS: u64 = 1;
/// Echo replies sent per second, at most (a ping flood stays bounded).
const ECHO_REPLIES_PER_SECOND: u32 = 16;
const TIMER_HZ: u64 = 100;

/// What delivered frames were, by kind.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Kind {
    Arp = 0,
    ArpReplied,
    Ipv6,
    Dhcp,
    Udp,
    UdpNoSocket,
    Tcp,
    TcpRefused,
    Echo,
    EchoLimited,
}
const KINDS: usize = Kind::EchoLimited as usize + 1;

static DELIVERED: [AtomicU64; KINDS] = [const { AtomicU64::new(0) }; KINDS];
static DROPS: [AtomicU64; DROP_KINDS] = [const { AtomicU64::new(0) }; DROP_KINDS];
static PASSES: AtomicU64 = AtomicU64::new(0);
static ECHO_WINDOW: AtomicU64 = AtomicU64::new(0);
static ECHO_IN_WINDOW: AtomicU32 = AtomicU32::new(0);
/// Interfaces the thread leaves alone (neo-test: a harness peer NIC).
static SKIP: AtomicU32 = AtomicU32::new(0);

/// OT-96 F1 discriminator: set once `net-rx` starts (neo-test builds).
#[cfg(feature = "neo-test")]
static STARTED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// OT-96 F1 (Chad #173, CP-37), neo-test builds: once `net-rx` runs, a NIC's
/// device lock (plain spin) is one it takes IF-off, so a taker with IF on can
/// deadlock it on the same CPU. The virtio and wlan0 device methods call this
/// before they lock; a taker with IF on panics, naming `what`.
#[cfg(feature = "neo-test")]
pub fn assert_if_off(what: &str) {
    if STARTED.load(Ordering::Acquire) && x86_64::instructions::interrupts::are_enabled() {
        panic!(
            "OT-96 F1: {} taken with IF on while net-rx runs (CP-37)",
            what
        );
    }
}

/// Start the receive thread. After the scheduler is up, and after every
/// driver's bring-up loop has stopped servicing its rings itself.
pub fn start() {
    #[cfg(feature = "neo-test")]
    STARTED.store(true, Ordering::Release);
    let id = kthread::spawn_detached("net-rx", run);
    crate::serial_println!(
        "[ net ] rx: started (kthread slot={}) devices={} budget={} nap={} ticks",
        id.slot(),
        crate::drivers::net::device_count(),
        FRAME_BUDGET,
        NAP_TICKS
    );
}

fn run(stop: KthreadStopToken) -> KthreadExitState {
    let mut buf = [0u8; MAX_FRAME];
    while !stop.is_stop_requested() {
        pass(&mut buf);
        crate::sched::tick_sleep::sleep_ticks(NAP_TICKS);
    }
    KthreadExitState::Success
}

/// One pass over every device: service it, then route up to
/// [`FRAME_BUDGET`] of its frames.
fn pass(buf: &mut [u8; MAX_FRAME]) {
    let skip = SKIP.load(Ordering::Acquire);
    for index in 0..crate::drivers::net::device_count() {
        if index < 32 && skip & (1 << index) != 0 {
            continue;
        }
        let Some(dev) = crate::drivers::net::get_device(index) else {
            continue;
        };
        without_interrupts(|| dev.service());
        for _ in 0..FRAME_BUDGET {
            let more = without_interrupts(|| {
                let Some(frame) = dev.rx_take() else {
                    return false;
                };
                let bytes = frame.as_slice();
                let len = bytes.len();
                let fits = len <= MAX_FRAME;
                if fits {
                    buf[..len].copy_from_slice(bytes);
                }
                dev.rx_recycle(frame);
                if fits {
                    deliver(index as u32, &buf[..len], dev.as_ref());
                } else {
                    DROPS[rx_logic::DropReason::TooLong as usize].fetch_add(1, Ordering::Relaxed);
                }
                true
            });
            if !more {
                break;
            }
        }
    }
    // A TCP reply to a wire peer (a SYN-ACK, an ACK) is queued by the TCP
    // layer and sent by this flush: bounded (OT-96 F2, Chad #173), a chunk
    // IF-off at a time.
    for _ in 0..FLUSH_CHUNKS {
        if !without_interrupts(|| super::tcp::flush_wire_egress_some(super::tcp::WIRE_FLUSH_BUDGET))
        {
            break;
        }
    }
    PASSES.fetch_add(1, Ordering::Relaxed);
}

fn count(kind: Kind) {
    DELIVERED[kind as usize].fetch_add(1, Ordering::Relaxed);
}

fn local_v4(iface: u32) -> LocalV4 {
    match super::ipv4::primary(iface) {
        Some((addr, mask)) => LocalV4 {
            addr: Some(addr),
            mask,
        },
        None => LocalV4 {
            addr: None,
            mask: [0; 4],
        },
    }
}

fn v4(addr: [u8; 4], port: u16) -> SocketAddr {
    let mut full = [0u8; 16];
    full[..4].copy_from_slice(&addr);
    SocketAddr {
        family: AddressFamily::Inet4,
        port,
        addr: full,
    }
}

/// Route one frame (a private copy) received on `iface`.
fn deliver(iface: u32, frame: &[u8], dev: &dyn super::NetDevice) {
    let local = local_v4(iface);
    match rx_logic::classify(frame, &local) {
        Verdict::Arp => {
            if !rx_logic::arp_sender_ok(frame, &local) {
                DROPS[rx_logic::DropReason::ArpSender as usize].fetch_add(1, Ordering::Relaxed);
                return;
            }
            count(Kind::Arp);
            if let Some(ours) = local.addr
                && let Some((mac, ip)) = rx_logic::arp_request_for(frame, ours)
            {
                super::arp::insert(ip, mac);
                let reply = rx_logic::arp_reply(dev.mac_address(), ours, mac, ip);
                if transmit(dev, &reply) {
                    count(Kind::ArpReplied);
                }
            } else {
                let _ = super::arp::inject(frame);
            }
        }
        Verdict::Ipv6 => {
            let ours = super::ipv6::addresses(iface);
            match rx_logic::classify_ipv6(frame, &ours) {
                Verdict6::Ra => {
                    count(Kind::Ipv6);
                    let _ = super::ipv6::inject_router_advertisement(frame);
                }
                Verdict6::Ndp => {
                    count(Kind::Ipv6);
                    let _ = super::ndp::inject(frame);
                }
                Verdict6::Drop(reason) => {
                    DROPS[reason as usize].fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        Verdict::Dhcp => {
            count(Kind::Dhcp);
            let _ = super::dhcpv4::demux_eth_rx(frame);
        }
        Verdict::Udp {
            ip,
            src_port,
            dst_port,
            payload,
        } => {
            match super::socket::deliver_inbound(
                v4(ip.src, src_port),
                v4(ip.dst, dst_port),
                payload,
            ) {
                Ok(()) => count(Kind::Udp),
                Err(_) => count(Kind::UdpNoSocket),
            }
        }
        Verdict::Tcp {
            ip,
            src_port,
            dst_port,
            segment,
        } => {
            match super::tcp::inject_segment(v4(ip.src, src_port), v4(ip.dst, dst_port), segment) {
                Ok(()) => count(Kind::Tcp),
                Err(_) => count(Kind::TcpRefused),
            }
        }
        Verdict::IcmpEcho { ip, message } => {
            // Never answer a broadcast ping (a smurf amplifier), and keep a
            // flood to the per-second budget.
            if Some(ip.dst) != local.addr || !echo_allowed() {
                count(Kind::EchoLimited);
                return;
            }
            if echo_reply(dev, ip, message) {
                count(Kind::Echo);
            }
        }
        Verdict::Drop(reason) => {
            DROPS[reason as usize].fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn echo_allowed() -> bool {
    let second = crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed) / TIMER_HZ;
    if ECHO_WINDOW.swap(second, Ordering::Relaxed) != second {
        ECHO_IN_WINDOW.store(0, Ordering::Relaxed);
    }
    ECHO_IN_WINDOW.fetch_add(1, Ordering::Relaxed) < ECHO_REPLIES_PER_SECOND
}

/// Send the echo reply straight back to the requester's MAC (no ARP needed).
fn echo_reply(dev: &dyn super::NetDevice, ip: rx_logic::Ipv4Meta, message: &[u8]) -> bool {
    let mut frame = [0u8; MAX_FRAME];
    let ip_at = rx_logic::ETH_HEADER_LEN;
    let icmp_at = ip_at + rx_logic::IPV4_MIN_HEADER;
    let Some(n) = rx_logic::icmp_echo_reply(message, &mut frame[icmp_at..]) else {
        return false;
    };
    frame[0..6].copy_from_slice(&ip.src_mac);
    frame[6..12].copy_from_slice(&dev.mac_address());
    frame[12..14].copy_from_slice(&rx_logic::ETHERTYPE_IPV4.to_be_bytes());
    let header = super::ipv4::Ipv4Header {
        src: ip.dst,
        dst: ip.src,
        protocol: rx_logic::PROTO_ICMP,
        ttl: super::ipv4::DEFAULT_TTL,
        identification: 0,
        payload_len: n as u16,
    };
    if header.encode(&mut frame[ip_at..]).is_err() {
        return false;
    }
    transmit(dev, &frame[..icmp_at + n])
}

fn transmit(dev: &dyn super::NetDevice, bytes: &[u8]) -> bool {
    let Some(mut tx) = dev.tx_reserve() else {
        return false;
    };
    if bytes.len() > tx.len() {
        return false;
    }
    tx.as_mut_slice()[..bytes.len()].copy_from_slice(bytes);
    dev.tx_submit(tx, bytes.len()).is_ok()
}

/// Delivered counts by [`Kind`], drop counts by [`rx_logic::DropReason`], and
/// the passes run.
pub fn stats() -> ([u64; KINDS], [u64; DROP_KINDS], u64) {
    let mut d = [0u64; KINDS];
    for (i, c) in DELIVERED.iter().enumerate() {
        d[i] = c.load(Ordering::Relaxed);
    }
    let mut r = [0u64; DROP_KINDS];
    for (i, c) in DROPS.iter().enumerate() {
        r[i] = c.load(Ordering::Relaxed);
    }
    (d, r, PASSES.load(Ordering::Relaxed))
}

/// Test-only: leave interface `iface` to the harness (a peer NIC).
#[cfg(feature = "neo-test")]
pub fn skip_for_test(iface: u32) {
    if iface < 32 {
        SKIP.fetch_or(1 << iface, Ordering::AcqRel);
    }
}
