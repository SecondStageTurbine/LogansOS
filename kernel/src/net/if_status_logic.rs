// SPDX-License-Identifier: MPL-2.0
//! F11 S4 (setup-menu THINK §3.3): `SYS_NET_IF_STATUS`'s fixed-size record and
//! the pure rules that fill it.
//!
//! No kernel dependency: `tools/net-if-status-abi-tests` includes this file by
//! path and holds it to a hand-laid byte oracle (I-12), independent of the
//! kernel's writer and libneo's reader.
//!
//! The record describes **one** interface, the default one ([`choose`]): the
//! DHCP client's interface when it holds a lease, else the first interface
//! whose link is up, else the first registered. With none, it is all zeros but
//! `version` (no `PRESENT` flag).

/// `NetIfStatusV1::version`.
pub const NET_IF_STATUS_V1_VERSION: u32 = 1;
/// The record's size in bytes (`size_of::<NetIfStatusV1>()`).
pub const NET_IF_STATUS_V1_SIZE: usize = 56;
/// The driver name's field width; longer names are cut, shorter NUL-padded.
pub const DRIVER_LEN: usize = 12;

/// Bit 0: the interface's link is up (the identity registry's link state, or
/// the driver's own when identity is off).
pub const FLAG_LINK_UP: u32 = 1 << 0;
/// Bit 1: the DHCP client holds a bound lease on this interface.
pub const FLAG_LEASE_BOUND: u32 = 1 << 1;
/// Bit 2: the driver reports carrier (`NetDevice::link_state() == Up`).
pub const FLAG_CARRIER: u32 = 1 << 2;
/// Bit 3: an interface exists. Clear: no network device is registered.
pub const FLAG_PRESENT: u32 = 1 << 3;
/// Bit 4: the sources kept changing while they were read
/// ([`READ_ATTEMPTS`] times): this is the last record read, and it may mix
/// two states. Clear: the record is one state of the network.
pub const FLAG_STALE: u32 = 1 << 4;
/// How many times [`read_consistent`] reads before it gives up (bounded:
/// it never spins waiting for the network to settle).
pub const READ_ATTEMPTS: usize = 4;
/// `lease_secs_left` when no lease is bound.
pub const LEASE_NONE: u64 = u64::MAX;

/// The `SYS_NET_IF_STATUS` record, in memory order (the kernel copies it out
/// as is; x86-64 is little-endian). 56 bytes, 8-aligned.
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct NetIfStatusV1 {
    /// 0..4: [`NET_IF_STATUS_V1_VERSION`].
    pub version: u32,
    /// 4..8: the `FLAG_*` bits.
    pub flags: u32,
    /// 8..12: the interface's index (the device registry's), 0 without one.
    pub ifindex: u32,
    /// 12..24: the driver's name (`cdc-ecm`, `rtl8125`, ...), NUL-padded.
    pub driver: [u8; DRIVER_LEN],
    /// 24..30: the MAC address.
    pub mac: [u8; 6],
    /// 30..34: the IPv4 address (the lease's, else the interface's preferred
    /// address), 0.0.0.0 without one.
    pub ipv4: [u8; 4],
    /// 34: the prefix length of the lease's mask (0 without a lease).
    pub prefix: u8,
    /// 35: zero.
    pub _pad0: u8,
    /// 36..40: the lease's router, 0.0.0.0 without one.
    pub router: [u8; 4],
    /// 40..44: the lease's DNS server, 0.0.0.0 without one.
    pub dns: [u8; 4],
    /// 44..48: zero.
    pub _pad1: u32,
    /// 48..56: the lease's remaining time on the DHCP client's clock (its
    /// ticks are the lease's seconds), or [`LEASE_NONE`].
    pub lease_secs_left: u64,
}

impl NetIfStatusV1 {
    /// No interface: version set, everything else zero, no lease.
    pub const fn none() -> Self {
        Self {
            version: NET_IF_STATUS_V1_VERSION,
            flags: 0,
            ifindex: 0,
            driver: [0; DRIVER_LEN],
            mac: [0; 6],
            ipv4: [0; 4],
            prefix: 0,
            _pad0: 0,
            router: [0; 4],
            dns: [0; 4],
            _pad1: 0,
            lease_secs_left: LEASE_NONE,
        }
    }
}

/// One registered interface, as the snapshot found it.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Iface<'a> {
    pub ifindex: u32,
    pub driver: &'a str,
    pub mac: [u8; 6],
    /// The registry's link state is `Up`.
    pub link_up: bool,
    /// The driver's own link state is `Up`.
    pub carrier: bool,
}

/// The DHCP client's lease, as one snapshot of the client found it (taken
/// under its lock, so the fields agree with each other).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Lease {
    /// The interface the client is bound to.
    pub ifindex: u32,
    pub ip: [u8; 4],
    pub mask: [u8; 4],
    pub router: Option<[u8; 4]>,
    pub dns: Option<[u8; 4]>,
    pub expiry: u64,
    /// The client's clock when the snapshot was taken.
    pub now: u64,
}

/// The default interface's index among `links` (`(ifindex, up)`, in
/// registration order): the leased one if it is registered, else the first
/// that is up, else the first.
pub fn choose(links: &[(u32, bool)], leased: Option<u32>) -> Option<u32> {
    if let Some(id) = leased
        && links.iter().any(|&(i, _)| i == id)
    {
        return Some(id);
    }
    links
        .iter()
        .find(|&&(_, up)| up)
        .or_else(|| links.first())
        .map(|&(i, _)| i)
}

/// The prefix length of `mask`: its leading one bits (a non-contiguous mask
/// counts up to its first zero).
pub fn prefix_len(mask: [u8; 4]) -> u8 {
    u32::from_be_bytes(mask).leading_ones() as u8
}

/// The record for `iface` (`None`: no interface), with `lease` if it is bound
/// on that interface, and `address` (the interface's preferred IPv4 address)
/// when there is no lease.
pub fn build(
    iface: Option<Iface<'_>>,
    lease: Option<Lease>,
    address: Option<[u8; 4]>,
) -> NetIfStatusV1 {
    let mut out = NetIfStatusV1::none();
    let Some(iface) = iface else {
        return out;
    };
    out.flags = FLAG_PRESENT;
    if iface.link_up {
        out.flags |= FLAG_LINK_UP;
    }
    if iface.carrier {
        out.flags |= FLAG_CARRIER;
    }
    out.ifindex = iface.ifindex;
    let name = iface.driver.as_bytes();
    let n = name.len().min(DRIVER_LEN);
    out.driver[..n].copy_from_slice(&name[..n]);
    out.mac = iface.mac;
    match lease.filter(|l| l.ifindex == iface.ifindex) {
        Some(l) => {
            out.flags |= FLAG_LEASE_BOUND;
            out.ipv4 = l.ip;
            out.prefix = prefix_len(l.mask);
            out.router = l.router.unwrap_or([0; 4]);
            out.dns = l.dns.unwrap_or([0; 4]);
            out.lease_secs_left = l.expiry.saturating_sub(l.now);
        }
        None => out.ipv4 = address.unwrap_or([0; 4]),
    }
    out
}

/// One coherent record (Janus, #100 relay 382). `generation` is the
/// network-status generation, which every writer of a source bumps inside its
/// lock after the write; `read` reads every source once and builds the record
/// (`None`: a source vanished mid-read). A record read between two equal
/// loads of the generation is returned. After [`READ_ATTEMPTS`] reads that
/// each saw a change, the last record is returned with [`FLAG_STALE`] set
/// (with none, an empty record so flagged).
pub fn read_consistent(
    generation: impl Fn() -> u64,
    mut read: impl FnMut() -> Option<NetIfStatusV1>,
) -> NetIfStatusV1 {
    let mut last = None;
    for _ in 0..READ_ATTEMPTS {
        let before = generation();
        let record = read();
        // The reads above happen before the second load.
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        if let Some(r) = record {
            if generation() == before {
                return r;
            }
            last = Some(r);
        }
    }
    let mut r = last.unwrap_or(NetIfStatusV1::none());
    r.flags |= FLAG_STALE;
    r
}

/// The caller's buffer length: shorter than the record is refused (the
/// kernel answers `EINVAL`); longer is accepted, and only the record's bytes
/// are written.
pub fn len_ok(len: usize) -> bool {
    len >= NET_IF_STATUS_V1_SIZE
}

const _: () = assert!(core::mem::size_of::<NetIfStatusV1>() == NET_IF_STATUS_V1_SIZE);
const _: () = assert!(core::mem::align_of::<NetIfStatusV1>() == 8);
