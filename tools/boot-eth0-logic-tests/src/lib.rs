// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the boot-time eth0 status decisions.

#![allow(dead_code)]

#[path = "../../../kernel/src/drivers/usb/boot_eth0_logic.rs"]
mod boot_eth0_logic;

#[cfg(test)]
mod tests {
    use super::boot_eth0_logic::*;

    #[test]
    fn no_device_stops_before_dhcp() {
        assert_eq!(
            pre_dhcp_decision(false, false, false),
            PreDhcpDecision::NoDevice
        );
    }

    #[test]
    fn explicit_down_without_rx_is_no_link() {
        assert_eq!(
            pre_dhcp_decision(true, true, false),
            PreDhcpDecision::NoLink
        );
    }

    #[test]
    fn explicit_down_with_rx_still_attempts_dhcp() {
        assert_eq!(
            pre_dhcp_decision(true, true, true),
            PreDhcpDecision::AttemptDhcp
        );
    }

    #[test]
    fn silent_or_unknown_link_attempts_dhcp() {
        assert_eq!(
            pre_dhcp_decision(true, false, false),
            PreDhcpDecision::AttemptDhcp
        );
    }

    #[test]
    fn final_status_is_lease_or_no_lease_only() {
        assert_eq!(final_dhcp_status(true), BootEth0Status::Lease);
        assert_eq!(final_dhcp_status(false), BootEth0Status::NoLease);
    }

    #[test]
    fn status_codes_round_trip() {
        for status in [
            BootEth0Status::NotStarted,
            BootEth0Status::NoDevice,
            BootEth0Status::NoLink,
            BootEth0Status::NoLease,
            BootEth0Status::Lease,
        ] {
            assert_eq!(BootEth0Status::from_code(status.code()), status);
        }
        assert_eq!(BootEth0Status::from_code(99), BootEth0Status::NotStarted);
    }
}
