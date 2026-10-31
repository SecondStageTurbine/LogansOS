// SPDX-License-Identifier: MPL-2.0
//! Pure status decisions for the boot-time eth0 product flow.
//!
//! Kept kernel-free so the kill-switch/status edge cases can be host-tested.

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BootEth0Status {
    NotStarted,
    NoDevice,
    NoLink,
    NoLease,
    Lease,
}

impl BootEth0Status {
    pub const fn code(self) -> u8 {
        match self {
            Self::NotStarted => 0,
            Self::NoDevice => 1,
            Self::NoLink => 2,
            Self::NoLease => 3,
            Self::Lease => 4,
        }
    }

    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::NoDevice,
            2 => Self::NoLink,
            3 => Self::NoLease,
            4 => Self::Lease,
            _ => Self::NotStarted,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PreDhcpDecision {
    NoDevice,
    NoLink,
    AttemptDhcp,
}

/// Decide whether the boot flow should attempt DHCP after USB enumeration and
/// bounded link settling.
///
/// An explicit link-down notification with zero RX evidence is reported as
/// `no link`; unknown/silent link state still attempts DHCP because CDC-ECM
/// hardware often omits link notifications while data works.
pub const fn pre_dhcp_decision(
    device_present: bool,
    explicit_link_down: bool,
    rx_frames_seen: bool,
) -> PreDhcpDecision {
    if !device_present {
        return PreDhcpDecision::NoDevice;
    }
    if explicit_link_down && !rx_frames_seen {
        return PreDhcpDecision::NoLink;
    }
    PreDhcpDecision::AttemptDhcp
}

pub const fn final_dhcp_status(bound: bool) -> BootEth0Status {
    if bound {
        BootEth0Status::Lease
    } else {
        BootEth0Status::NoLease
    }
}
