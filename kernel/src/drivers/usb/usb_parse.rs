// SPDX-License-Identifier: MPL-2.0
//! Pure USB descriptor + CDC-ECM parsing (Phase 7.5 BUILD-B, ANSPEC §10.8,
//! ADR-047) — kernel-free and host-testable (path-`mod`'d by
//! `tools/ai-ops-tests`). This file IS the E1/E2 discriminator arithmetic:
//! if the configuration walk, the alt-setting selection, or the ECM
//! functional-descriptor extraction is wrong, it is wrong HERE, under a
//! truth table and a hand-laid byte oracle, not in a blind bare-metal run.
//!
//! USB 2.0 §9.6 (standard descriptors), CDC 1.2 §5.2.3 (functional
//! descriptors), ECM 1.2 §5 (Ethernet Networking functional descriptor,
//! subtype 0x0F) + §3.2 (the alt-setting protocol: the data interface's
//! default alt 0 has NO endpoints; the bulk pair lives on alt 1 — selecting
//! it is mandatory and forgetting it is THE classic ECM bring-up trap).

extern crate alloc;

use alloc::vec::Vec;

/// One parsed endpoint descriptor (USB 2.0 §9.6.6), plus the bMaxBurst of
/// a trailing SuperSpeed Endpoint Companion descriptor (USB 3.2 §9.6.7)
/// when one follows — 0 otherwise. A SuperSpeed endpoint programmed with
/// the wrong Max Burst runs degraded or is rejected by strict silicon
/// (Forge M1): the companion is parsed so the EP context can carry it.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct UsbEndpoint {
    /// bEndpointAddress: number in [3:0], direction IN = bit 7.
    pub address: u8,
    /// bmAttributes: transfer type in [1:0] (0 ctrl, 1 isoch, 2 bulk, 3 int).
    pub attributes: u8,
    pub max_packet: u16,
    pub interval: u8,
    /// SuperSpeed Endpoint Companion bMaxBurst (0..15 = 1..16 packets per
    /// burst); 0 when no companion descriptor followed (FS/HS devices).
    pub max_burst: u8,
}

impl UsbEndpoint {
    pub fn number(&self) -> u8 {
        self.address & 0x0F
    }
    pub fn is_in(&self) -> bool {
        self.address & 0x80 != 0
    }
    pub fn is_bulk(&self) -> bool {
        self.attributes & 0x03 == 2
    }
    pub fn is_interrupt(&self) -> bool {
        self.attributes & 0x03 == 3
    }
}

/// The CDC Ethernet Networking functional descriptor's payload (ECM 1.2
/// §5.4): where the MAC string lives and how big a segment may be.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct EcmDescriptor {
    pub mac_string_index: u8,
    pub max_segment_size: u16,
}

/// One parsed interface descriptor — one (number, alt_setting) pair with its
/// endpoints and any CDC-ECM functional descriptor that followed it.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct UsbInterface {
    pub number: u8,
    pub alt_setting: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub endpoints: Vec<UsbEndpoint>,
    pub ecm: Option<EcmDescriptor>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct UsbConfiguration {
    pub value: u8,
    pub interfaces: Vec<UsbInterface>,
}

/// Walk one full configuration-descriptor set (the wTotalLength blob from
/// GET_DESCRIPTOR(CONFIGURATION)). Total: a descriptor whose bLength is 0 or
/// runs past the buffer terminates the walk; unknown descriptor types are
/// skipped (class-specific blobs other than ECM included) — the walk never
/// indexes out of bounds and never loops.
pub fn parse_configuration_set(bytes: &[u8]) -> UsbConfiguration {
    let mut config_value = 0u8;
    let mut interfaces: Vec<UsbInterface> = Vec::new();
    let mut offset = 0usize;
    while offset + 2 <= bytes.len() {
        let len = bytes[offset] as usize;
        let dtype = bytes[offset + 1];
        if len < 2 || offset + len > bytes.len() {
            break;
        }
        let body = &bytes[offset..offset + len];
        match dtype {
            // CONFIGURATION: bConfigurationValue at byte 5.
            0x02 if len >= 6 => {
                config_value = body[5];
            }
            // INTERFACE: number, alt, num_eps, class, subclass, protocol.
            0x04 if len >= 9 => {
                interfaces.push(UsbInterface {
                    number: body[2],
                    alt_setting: body[3],
                    class: body[5],
                    subclass: body[6],
                    protocol: body[7],
                    endpoints: Vec::new(),
                    ecm: None,
                });
            }
            // ENDPOINT: attaches to the most recent interface.
            0x05 if len >= 7 => {
                if let Some(iface) = interfaces.last_mut() {
                    iface.endpoints.push(UsbEndpoint {
                        address: body[2],
                        attributes: body[3],
                        max_packet: u16::from_le_bytes([body[4], body[5]]),
                        interval: body[6],
                        max_burst: 0,
                    });
                }
            }
            // SUPERSPEED ENDPOINT COMPANION (USB 3.2 §9.6.7): bMaxBurst at
            // byte 2; attaches to the endpoint it immediately follows.
            0x30 if len >= 6 => {
                if let Some(ep) = interfaces
                    .last_mut()
                    .and_then(|iface| iface.endpoints.last_mut())
                {
                    ep.max_burst = body[2];
                }
            }
            // CS_INTERFACE: CDC functional descriptors; we retain only the
            // Ethernet Networking one (ECM 1.2 §5.4 layout: subtype 0x0F,
            // iMACAddress at byte 3, bmEthernetStatistics at 4..8,
            // wMaxSegmentSize at 8..10 — the byte oracle caught the first
            // draft reading 10..12).
            0x24 if len >= 13 && body[2] == 0x0F => {
                if let Some(iface) = interfaces.last_mut() {
                    iface.ecm = Some(EcmDescriptor {
                        mac_string_index: body[3],
                        max_segment_size: u16::from_le_bytes([body[8], body[9]]),
                    });
                }
            }
            _ => {}
        }
        offset += len;
    }
    UsbConfiguration {
        value: config_value,
        interfaces,
    }
}

/// The complete ECM function located inside a device's configuration set —
/// everything the class driver needs to bind, found BY CONTENT (so a
/// dual-configuration RNDIS+ECM device binds on its ECM configuration with
/// zero RNDIS knowledge anywhere).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct EcmTopology {
    pub config_value: u8,
    pub comm_interface: u8,
    pub data_interface: u8,
    /// The data-interface alternate setting carrying the bulk pair (the ECM
    /// trap: alt 0 has no endpoints; this is almost always 1).
    pub data_alt_setting: u8,
    pub int_ep: UsbEndpoint,
    pub bulk_in: UsbEndpoint,
    pub bulk_out: UsbEndpoint,
    pub mac_string_index: u8,
    pub max_segment_size: u16,
}

/// Find the first CDC-ECM function across all configurations: a comm
/// interface (class 02 subclass 06) carrying the Ethernet Networking
/// functional descriptor and an interrupt-IN endpoint, plus a data interface
/// (class 0A, same function) whose non-default alt setting carries a
/// bulk-IN/bulk-OUT pair. Returns None when the device simply is not ECM —
/// the E1 verdict, distinct from every bind failure.
pub fn find_ecm(configurations: &[UsbConfiguration]) -> Option<EcmTopology> {
    for config in configurations {
        for comm in &config.interfaces {
            if comm.class != 0x02 || comm.subclass != 0x06 {
                continue;
            }
            let Some(ecm) = comm.ecm else { continue };
            let Some(int_ep) = comm
                .endpoints
                .iter()
                .find(|ep| ep.is_interrupt() && ep.is_in())
                .copied()
            else {
                continue;
            };
            // The data interface: the first class-0A interface (other than
            // the comm interface) whose alt setting carries the bulk pair.
            // The CDC union descriptor (0x24/0x06) is NOT parsed — correct
            // for single-function ECM devices (this build's scope); a
            // composite with two CDC functions would need union pairing
            // (Forge m4: deferred, recorded in ADR-047).
            for data in &config.interfaces {
                if data.class != 0x0A || data.number == comm.number {
                    continue;
                }
                let bulk_in = data
                    .endpoints
                    .iter()
                    .find(|ep| ep.is_bulk() && ep.is_in())
                    .copied();
                let bulk_out = data
                    .endpoints
                    .iter()
                    .find(|ep| ep.is_bulk() && !ep.is_in())
                    .copied();
                if let (Some(bulk_in), Some(bulk_out)) = (bulk_in, bulk_out) {
                    return Some(EcmTopology {
                        config_value: config.value,
                        comm_interface: comm.number,
                        data_interface: data.number,
                        data_alt_setting: data.alt_setting,
                        int_ep,
                        bulk_in,
                        bulk_out,
                        mac_string_index: ecm.mac_string_index,
                        max_segment_size: ecm.max_segment_size,
                    });
                }
            }
        }
    }
    None
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct HidKeyboardTopology {
    pub config_value: u8,
    pub interface: u8,
    pub int_in: UsbEndpoint,
}

/// Find a USB HID boot-protocol keyboard interface. This intentionally stops
/// at the boot subclass/protocol tuple and interrupt-IN endpoint; Phase 8b
/// does not parse report descriptors, NKRO layouts, mice, or vendor reports.
pub fn find_hid_boot_keyboard(configurations: &[UsbConfiguration]) -> Option<HidKeyboardTopology> {
    for config in configurations {
        for iface in &config.interfaces {
            if iface.class != 0x03 || iface.subclass != 0x01 || iface.protocol != 0x01 {
                continue;
            }
            let Some(int_in) = iface
                .endpoints
                .iter()
                .find(|ep| ep.is_interrupt() && ep.is_in())
                .copied()
            else {
                continue;
            };
            return Some(HidKeyboardTopology {
                config_value: config.value,
                interface: iface.number,
                int_in,
            });
        }
    }
    None
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct HidMouseTopology {
    pub config_value: u8,
    pub interface: u8,
    pub int_in: UsbEndpoint,
}

/// Find a USB HID boot-protocol **mouse** interface: HID class (0x03), boot
/// subclass (0x01), **mouse protocol (0x02)** with an interrupt-IN endpoint. The
/// exact mirror of [`find_hid_boot_keyboard`] but for `protocol == 0x02`, so a
/// composite keyboard+mouse device (very common) is claimed on the mouse
/// interface by its own protocol byte without disturbing the keyboard interface
/// (protocol 0x01), which the keyboard driver claims separately. Returns None
/// when the device simply is not a boot mouse — distinct from any bind failure.
pub fn find_hid_boot_mouse(configurations: &[UsbConfiguration]) -> Option<HidMouseTopology> {
    for config in configurations {
        for iface in &config.interfaces {
            if iface.class != 0x03 || iface.subclass != 0x01 || iface.protocol != 0x02 {
                continue;
            }
            let Some(int_in) = iface
                .endpoints
                .iter()
                .find(|ep| ep.is_interrupt() && ep.is_in())
                .copied()
            else {
                continue;
            };
            return Some(HidMouseTopology {
                config_value: config.value,
                interface: iface.number,
                int_in,
            });
        }
    }
    None
}

/// A CDC notification from the interrupt endpoint (CDC 1.2 §6.3): an
/// 8-byte request-like header. We consume NetworkConnection (0x00) for
/// link state and recognize ConnectionSpeedChange (0x2A) to discard it;
/// anything else is Other (counted, never fatal).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum EcmNotification {
    NetworkConnection { up: bool },
    ConnectionSpeedChange,
    Other(u8),
}

pub fn parse_notification(bytes: &[u8]) -> Option<EcmNotification> {
    if bytes.len() < 8 || bytes[0] & 0x80 == 0 {
        return None;
    }
    let code = bytes[1];
    let w_value = u16::from_le_bytes([bytes[2], bytes[3]]);
    Some(match code {
        0x00 => EcmNotification::NetworkConnection { up: w_value != 0 },
        0x2A => EcmNotification::ConnectionSpeedChange,
        other => EcmNotification::Other(other),
    })
}

/// Decode the iMACAddress string descriptor (ECM 1.2 §5.4: exactly 12
/// UTF-16LE hex digits) into a MAC. Total: wrong length, a non-hex code
/// unit, or a truncated descriptor returns None — never a garbage MAC.
pub fn mac_from_string_descriptor(bytes: &[u8]) -> Option<[u8; 6]> {
    // bLength, bDescriptorType(3), then UTF-16LE code units.
    if bytes.len() < 2 || bytes[1] != 0x03 {
        return None;
    }
    let blen = bytes[0] as usize;
    if blen < 2 + 12 * 2 || blen > bytes.len() {
        return None;
    }
    let mut mac = [0u8; 6];
    for (i, out) in mac.iter_mut().enumerate() {
        let hi = hex_value(code_unit(bytes, 2 + i * 4)?)?;
        let lo = hex_value(code_unit(bytes, 2 + i * 4 + 2)?)?;
        *out = (hi << 4) | lo;
    }
    Some(mac)
}

fn code_unit(bytes: &[u8], offset: usize) -> Option<u8> {
    if offset + 1 >= bytes.len() || bytes[offset + 1] != 0 {
        return None;
    }
    Some(bytes[offset])
}

fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
