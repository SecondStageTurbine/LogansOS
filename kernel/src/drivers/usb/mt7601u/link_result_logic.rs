// SPDX-License-Identifier: MPL-2.0
//! Pure terminal-link policy for the MT7601U acceptance scenario.
//!
//! Association is necessary but not LINK-UP. A PASS also requires the group
//! key, a DHCP ACK that leaves the client Bound, a non-zero assigned IPv4
//! address, and the durable LINK-UP bit derived from that evidence.

#![allow(dead_code)] // host-oracle'd pure core; bare-metal harness is the consumer

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DhcpTerminal {
    NotRun = 0,
    AssociationFailed = 1,
    GtkMissing = 2,
    NetDeviceFailed = 3,
    Discovering = 4,
    TimeoutSelecting = 5,
    TimeoutRequesting = 6,
    InvalidLease = 7,
    BoundLinkUp = 8,
}

impl DhcpTerminal {
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::AssociationFailed,
            2 => Self::GtkMissing,
            3 => Self::NetDeviceFailed,
            4 => Self::Discovering,
            5 => Self::TimeoutSelecting,
            6 => Self::TimeoutRequesting,
            7 => Self::InvalidLease,
            8 => Self::BoundLinkUp,
            _ => Self::NotRun,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::AssociationFailed => "association_failed",
            Self::GtkMissing => "gtk_missing",
            Self::NetDeviceFailed => "netdev_failed",
            Self::Discovering => "discovering",
            Self::TimeoutSelecting => "timeout_selecting",
            Self::TimeoutRequesting => "timeout_requesting",
            Self::InvalidLease => "invalid_lease",
            Self::BoundLinkUp => "bound_link_up",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkEvidence {
    pub bound: bool,
    pub connected: bool,
    pub gtk_installed: bool,
    pub dhcp: DhcpTerminal,
    pub assigned_ip: [u8; 4],
    pub link_up: bool,
}

pub const fn nonzero_ip(ip: [u8; 4]) -> bool {
    ip[0] != 0 || ip[1] != 0 || ip[2] != 0 || ip[3] != 0
}

pub const fn ack_terminal(ip: [u8; 4], client_bound: bool) -> DhcpTerminal {
    if client_bound && nonzero_ip(ip) {
        DhcpTerminal::BoundLinkUp
    } else {
        DhcpTerminal::InvalidLease
    }
}

pub const fn timeout_terminal(saw_offer: bool) -> DhcpTerminal {
    if saw_offer {
        DhcpTerminal::TimeoutRequesting
    } else {
        DhcpTerminal::TimeoutSelecting
    }
}

pub const fn pass_eligible(evidence: LinkEvidence) -> bool {
    evidence.bound
        && evidence.connected
        && evidence.gtk_installed
        && matches!(evidence.dhcp, DhcpTerminal::BoundLinkUp)
        && nonzero_ip(evidence.assigned_ip)
        && evidence.link_up
}

pub const fn pack_ip(ip: [u8; 4]) -> u32 {
    u32::from_be_bytes(ip)
}

pub const fn unpack_ip(packed: u32) -> [u8; 4] {
    packed.to_be_bytes()
}
