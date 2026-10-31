// SPDX-License-Identifier: MPL-2.0
//! Boot-time eth0 product bring-up.
//!
//! Feature-gated by `boot-eth0` (default-off). The disabled build compiles no
//! USB host path or boot DHCP code into production boot. The enabled flow reuses
//! the proven CDC-ECM/AX88179 -> `NetDevice` -> DHCPv4 path: USB enumeration
//! has just run in `drivers::usb::init`; this module finds the bound CDC-ECM
//! device, settles link state without trusting silent interrupt endpoints, runs
//! bounded DHCP attempts through the production RX demux, and records a
//! serial/boot-frame status.

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use core::fmt::Write as _;
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use crate::net::{LinkState, NetDevice};

use super::boot_eth0_logic::{self, BootEth0Status, PreDhcpDecision};

const CDC_ECM_DRIVER: &str = "cdc-ecm";
const LINK_SETTLE_PASSES: u64 = 256;
const DHCP_ATTEMPTS: u32 = 4;
const DHCP_PUMP_PASSES_PER_ATTEMPT: u64 = 750_000;

static STATUS: AtomicU8 = AtomicU8::new(BootEth0Status::NotStarted.code());
static IFACE_ID: AtomicU32 = AtomicU32::new(0);
static LEASE_IP: AtomicU32 = AtomicU32::new(0);

#[derive(Default)]
struct PumpStats {
    demuxed: u64,
    frames: u64,
    short: u64,
    arp: u64,
    ipv4: u64,
    dhcp_offer: u64,
    dhcp_ack: u64,
    other: u64,
}

pub fn bring_up() {
    crate::serial_println!(
        "[ kernel ] eth0: boot bring-up enabled (usb enumerate -> cdc-ecm bind -> dhcpv4)"
    );
    crate::log::flush();

    let Some((iface_id, device)) = find_cdc_ecm_device() else {
        publish(BootEth0Status::NoDevice, 0, [0; 4]);
        crate::serial_println!(
            "[ kernel ] eth0: no device (no CDC-ECM/AX88179 bound); boot continues"
        );
        crate::log::flush();
        return;
    };
    IFACE_ID.store(iface_id, Ordering::Release);

    let name = interface_name(iface_id);
    crate::serial_println!(
        "[ kernel ] {}: CDC-ECM device found mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} mtu={} iface_id={}",
        name.as_str(),
        device.mac_address()[0],
        device.mac_address()[1],
        device.mac_address()[2],
        device.mac_address()[3],
        device.mac_address()[4],
        device.mac_address()[5],
        device.mtu(),
        iface_id,
    );
    crate::log::flush();

    let mut stats = PumpStats::default();
    for _ in 0..LINK_SETTLE_PASSES {
        pump_once(&device, &mut stats);
        if stats.frames > 0 || device.link_state() == LinkState::Up {
            break;
        }
        core::hint::spin_loop();
    }

    match boot_eth0_logic::pre_dhcp_decision(
        true,
        device.link_state() == LinkState::Down,
        stats.frames > 0,
    ) {
        PreDhcpDecision::NoDevice => {
            publish(BootEth0Status::NoDevice, iface_id, [0; 4]);
            crate::serial_println!("[ kernel ] {}: no device; boot continues", name.as_str());
            return;
        }
        PreDhcpDecision::NoLink => {
            publish(BootEth0Status::NoLink, iface_id, [0; 4]);
            crate::serial_println!(
                "[ kernel ] {}: no link after bounded settle; boot continues",
                name.as_str()
            );
            crate::log::flush();
            return;
        }
        PreDhcpDecision::AttemptDhcp => {}
    }

    crate::net::dhcpv4::set_interface_id(iface_id);
    let mut bound = false;
    for attempt in 1..=DHCP_ATTEMPTS {
        let xid = crate::net::dhcpv4::start_discover();
        crate::serial_println!(
            "[ kernel ] {}: dhcp discover attempt={}/{} xid={:#010x}",
            name.as_str(),
            attempt,
            DHCP_ATTEMPTS,
            xid
        );
        crate::log::flush();

        for _ in 0..DHCP_PUMP_PASSES_PER_ATTEMPT {
            pump_once(&device, &mut stats);
            crate::net::tick();
            if crate::net::dhcpv4::snapshot().state == crate::net::dhcpv4::DhcpState::Bound {
                bound = true;
                break;
            }
            core::hint::spin_loop();
        }
        if bound {
            break;
        }
        let snap = crate::net::dhcpv4::snapshot();
        crate::serial_println!(
            "[ kernel ] {}: dhcp attempt={} ended state={:?} frames={} demuxed={} offer={} ack={}",
            name.as_str(),
            attempt,
            snap.state,
            stats.frames,
            stats.demuxed,
            stats.dhcp_offer,
            stats.dhcp_ack
        );
        crate::log::flush();
    }

    if bound && let Some(lease) = crate::net::dhcpv4::snapshot().lease {
        publish(BootEth0Status::Lease, iface_id, lease.ip);
        log_cdc_io(name.as_str(), &stats);
        crate::serial_println!(
            "[ kernel ] {}: link up ip={}.{}.{}.{} router={:?} dns={:?} fence_refusals={}",
            name.as_str(),
            lease.ip[0],
            lease.ip[1],
            lease.ip[2],
            lease.ip[3],
            lease.router,
            lease.dns,
            crate::drivers::dma_arena::fence_refusals()
        );
        crate::log::flush();
        return;
    }

    publish(boot_eth0_logic::final_dhcp_status(false), iface_id, [0; 4]);
    log_cdc_io(name.as_str(), &stats);
    crate::serial_println!(
        "[ kernel ] {}: no lease after {} bounded attempts; boot continues (frames={} demuxed={} short={} arp={} ipv4={} offer={} ack={} other={} fence_refusals={})",
        name.as_str(),
        DHCP_ATTEMPTS,
        stats.frames,
        stats.demuxed,
        stats.short,
        stats.arp,
        stats.ipv4,
        stats.dhcp_offer,
        stats.dhcp_ack,
        stats.other,
        crate::drivers::dma_arena::fence_refusals()
    );
    crate::log::flush();
}

pub fn write_status_line(out: &mut String) {
    let iface_id = IFACE_ID.load(Ordering::Acquire);
    let name = interface_name(iface_id);
    match BootEth0Status::from_code(STATUS.load(Ordering::Acquire)) {
        BootEth0Status::NotStarted => {
            let _ = write!(out, "{}: pending", name.as_str());
        }
        BootEth0Status::NoDevice => {
            let _ = write!(out, "eth0: no device");
        }
        BootEth0Status::NoLink => {
            let _ = write!(out, "{}: no link", name.as_str());
        }
        BootEth0Status::NoLease => {
            let _ = write!(out, "{}: no lease", name.as_str());
        }
        BootEth0Status::Lease => {
            let ip = LEASE_IP.load(Ordering::Acquire).to_be_bytes();
            let _ = write!(
                out,
                "{}: link up ip={}.{}.{}.{}",
                name.as_str(),
                ip[0],
                ip[1],
                ip[2],
                ip[3]
            );
        }
    }
}

/// OT-78 runtime oracle. The product scenario reads the status published by
/// the real boot flow; it does not infer success from the serial sentence.
#[cfg(feature = "neo-test-product-integration")]
pub fn debug_status() -> BootEth0Status {
    BootEth0Status::from_code(STATUS.load(Ordering::Acquire))
}

fn publish(status: BootEth0Status, iface_id: u32, ip: [u8; 4]) {
    IFACE_ID.store(iface_id, Ordering::Release);
    LEASE_IP.store(u32::from_be_bytes(ip), Ordering::Release);
    STATUS.store(status.code(), Ordering::Release);
}

fn find_cdc_ecm_device() -> Option<(u32, Arc<dyn NetDevice>)> {
    for idx in 0..crate::drivers::net::device_count() {
        let device = crate::drivers::net::get_device(idx)?;
        if device.identity().driver == CDC_ECM_DRIVER {
            return Some((idx as u32, device));
        }
    }
    None
}

fn interface_name(iface_id: u32) -> String {
    crate::net::identity::name(iface_id).unwrap_or_else(|| {
        let mut fallback = String::from("eth");
        push_u32(&mut fallback, iface_id);
        fallback
    })
}

fn pump_once(device: &Arc<dyn NetDevice>, stats: &mut PumpStats) {
    device.service();
    while let Some(frame) = device.rx_take() {
        stats.demuxed += 1;
        let bytes = frame.as_slice();
        if bytes.len() >= 14 {
            match u16::from_be_bytes([bytes[12], bytes[13]]) {
                0x0806 => {
                    stats.frames += 1;
                    stats.arp += 1;
                    let _ = crate::net::arp::inject(bytes);
                }
                0x0800 => {
                    stats.frames += 1;
                    stats.ipv4 += 1;
                    match crate::net::dhcpv4::demux_eth_rx(bytes) {
                        Some(crate::net::dhcpv4::DhcpMessageType::Offer) => {
                            stats.dhcp_offer += 1;
                        }
                        Some(crate::net::dhcpv4::DhcpMessageType::Ack) => {
                            stats.dhcp_ack += 1;
                        }
                        _ => {}
                    }
                }
                _ => {
                    stats.frames += 1;
                    stats.other += 1;
                }
            }
        } else {
            stats.short += 1;
            stats.other += 1;
        }
        device.rx_recycle(frame);
    }
}

fn log_cdc_io(name: &str, stats: &PumpStats) {
    if let Some(io) = super::cdc_ecm::debug_primary_io() {
        crate::serial_println!(
            "[ kernel ] {}: cdc-ecm io urb_completed={} frames_parsed={} frames_demuxed={} rx_zero_len={} rx_last_len={} rx_last_code={} tx_submit={} tx_complete={} tx_bytes={} tx_last_len={} tx_last_code={} ep_errors={}",
            name,
            io.rx_urb_completed,
            io.rx_frames_parsed,
            stats.demuxed,
            io.rx_zero_len,
            io.rx_last_len,
            fmt_code(io.rx_last_completion_code),
            io.tx_submissions,
            io.tx_completions,
            io.tx_bytes,
            io.tx_last_len,
            fmt_code(io.tx_last_completion_code),
            io.ep_errors
        );
        crate::log::flush();
    }
}

fn fmt_code(code: u64) -> u64 {
    if code == u64::MAX { 0xffff_ffff } else { code }
}

fn push_u32(name: &mut String, value: u32) {
    if value == 0 {
        name.push('0');
        return;
    }
    let mut digits = [0u8; 10];
    let mut n = value;
    let mut i = digits.len();
    while n > 0 {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    for &d in &digits[i..] {
        name.push(d as char);
    }
}
