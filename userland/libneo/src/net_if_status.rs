// SPDX-License-Identifier: MPL-2.0
//! F11 S4: decoding the `SYS_NET_IF_STATUS` record (the kernel's layout is
//! `kernel/src/net/if_status_logic.rs`). Written independently of the
//! kernel's struct, from the byte offsets; `tools/net-if-status-abi-tests`
//! holds both to hand-laid bytes.

/// The v1 record's length.
pub const RECORD_LEN: usize = 56;
/// The v1 record's `version`.
pub const VERSION: u32 = 1;

/// `flags` bits.
pub const FLAG_LINK_UP: u32 = 1 << 0;
pub const FLAG_LEASE_BOUND: u32 = 1 << 1;
pub const FLAG_CARRIER: u32 = 1 << 2;
pub const FLAG_PRESENT: u32 = 1 << 3;
/// The network kept changing while the kernel read it: the record may mix
/// two states.
pub const FLAG_STALE: u32 = 1 << 4;

/// `lease_secs_left` with no lease.
const LEASE_NONE: u64 = u64::MAX;

/// The default network interface, as the kernel reported it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetIfStatus {
    pub flags: u32,
    pub ifindex: u32,
    driver: [u8; 12],
    pub mac: [u8; 6],
    /// 0.0.0.0: no address.
    pub ipv4: [u8; 4],
    pub prefix: u8,
    /// 0.0.0.0: none.
    pub router: [u8; 4],
    /// 0.0.0.0: none.
    pub dns: [u8; 4],
    lease_secs_left: u64,
}

impl NetIfStatus {
    /// A network device exists.
    pub fn present(&self) -> bool {
        self.flags & FLAG_PRESENT != 0
    }

    /// Its link is up.
    pub fn link_up(&self) -> bool {
        self.flags & FLAG_LINK_UP != 0
    }

    /// The network kept changing as the kernel read it; don't trust the
    /// record's parts together.
    pub fn stale(&self) -> bool {
        self.flags & FLAG_STALE != 0
    }

    /// A DHCP lease is bound on it.
    pub fn lease_bound(&self) -> bool {
        self.flags & FLAG_LEASE_BOUND != 0
    }

    /// The driver's name (`cdc-ecm`, `rtl8125`, ...), empty if none.
    pub fn driver(&self) -> &str {
        let n = self.driver.iter().position(|&b| b == 0).unwrap_or(12);
        core::str::from_utf8(&self.driver[..n]).unwrap_or("")
    }

    /// The address, if it has one.
    pub fn address(&self) -> Option<[u8; 4]> {
        (self.ipv4 != [0; 4]).then_some(self.ipv4)
    }

    /// The lease's remaining seconds, if a lease is bound.
    pub fn lease_secs_left(&self) -> Option<u64> {
        (self.lease_bound() && self.lease_secs_left != LEASE_NONE).then_some(self.lease_secs_left)
    }
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16 | u32::from(b[3]) << 24
}

fn le_u64(b: &[u8]) -> u64 {
    u64::from(le_u32(&b[..4])) | u64::from(le_u32(&b[4..8])) << 32
}

fn four(b: &[u8]) -> [u8; 4] {
    [b[0], b[1], b[2], b[3]]
}

/// The record in `rec`, or `None` if its version isn't 1 or a padding byte
/// isn't zero.
pub fn decode(rec: &[u8; RECORD_LEN]) -> Option<NetIfStatus> {
    if le_u32(&rec[0..4]) != VERSION || rec[35] != 0 || rec[44..48] != [0; 4] {
        return None;
    }
    let mut driver = [0u8; 12];
    driver.copy_from_slice(&rec[12..24]);
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&rec[24..30]);
    Some(NetIfStatus {
        flags: le_u32(&rec[4..8]),
        ifindex: le_u32(&rec[8..12]),
        driver,
        mac,
        ipv4: four(&rec[30..34]),
        prefix: rec[34],
        router: four(&rec[36..40]),
        dns: four(&rec[40..44]),
        lease_secs_left: le_u64(&rec[48..56]),
    })
}
