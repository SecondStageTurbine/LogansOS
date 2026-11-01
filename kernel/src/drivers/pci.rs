// SPDX-License-Identifier: MPL-2.0
//! Minimal x86 PCI configuration-space walker for Phase 3 storage bring-up.
//!
//! BUILD-1 deliberately supports the conventional CF8/CFC mechanism only.
//! That is enough for QEMU q35 virtio-blk smoke coverage and leaves clean
//! dispatch points for AHCI and NVMe in BUILD-5.

// aarch64/riscv64 tripwire: this module is x86-only by construction — PCI config
// goes through the CF8/CFC PIO mechanism and MSI/MSI-X targets the LAPIC doorbell
// (0xFEE00000). A weak-memory target needs an ECAM (MMIO) config path and a
// GIC-ITS doorbell instead. This is a deliberate, tracked deferral (post-Phase-5),
// not silent debt: see ADR-025 "aarch64 portability debt inventory". The guard
// turns a future non-x86 build into a clear message instead of an opaque
// x86_64-crate import failure.
#[cfg(not(target_arch = "x86_64"))]
compile_error!(
    "drivers::pci is x86-only (CF8/CFC PIO PCI config + LAPIC MSI 0xFEE00000); \
     aarch64/riscv64 need an ECAM + GIC-ITS path — see ADR-025 aarch64 portability debt inventory"
);

use core::ptr::write_volatile;

use alloc::vec::Vec;
use x86_64::instructions::port::Port;
use x86_64::{PhysAddr, VirtAddr};

use crate::serial_println;

const CONFIG_ADDRESS: u16 = 0x0cf8;
const CONFIG_DATA: u16 = 0x0cfc;

const VENDOR_ID_NONE: u16 = 0xffff;
const HEADER_TYPE_MULTI_FUNCTION: u8 = 0x80;
const STATUS_CAPABILITIES_LIST: u16 = 1 << 4;

const PCI_CAP_ID_VENDOR_SPECIFIC: u8 = 0x09;
const PCI_CAP_ID_MSI: u8 = 0x05;
const PCI_CAP_ID_MSIX: u8 = 0x11;

const CLASS_MASS_STORAGE: u8 = 0x01;
const CLASS_NETWORK: u8 = 0x02;
#[cfg(feature = "accel-intel")]
const CLASS_DISPLAY: u8 = 0x03;
const SUBCLASS_ETHERNET: u8 = 0x00;
const SUBCLASS_SATA: u8 = 0x06;
const PROGIF_AHCI: u8 = 0x01;
const SUBCLASS_NVM: u8 = 0x08;
const PROGIF_NVME: u8 = 0x02;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Device {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub revision_id: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_code: u8,
    pub header_type: u8,
}

impl Device {
    pub fn bdf(self) -> (u8, u8, u8) {
        (self.bus, self.device, self.function)
    }

    pub fn class_triplet(self) -> u32 {
        ((self.class_code as u32) << 16) | ((self.subclass as u32) << 8) | self.prog_if as u32
    }
}

/// VT-d domain class for a device (Phase 7.5 BUILD-V). The substrate
/// (GPU/NVMe/AHCI/virtio/host bridges) goes into an identity domain (GPA==HPA,
/// no regression); connectivity devices (USB host controllers + network) go
/// into a fence domain bounded to the DMA arena.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum IommuClass {
    Substrate,
    Connectivity,
    /// OT-63 Slice 5: display/graphics controllers (the iGPU). Placed in the
    /// identity domain at boot (no regression) like the rest of the substrate,
    /// then narrowed to the graphics domain (`narrow_graphics`) when Slice-5
    /// narrowing is enabled — its own translated domain over the graphics arena.
    Display,
}

/// Classify a PCI device for VT-d domain assignment. Connectivity = USB host
/// controllers (class 0x0C subclass 0x03 — xHCI/EHCI/UHCI) and network devices
/// (class 0x02, incl. virtio-net). Display = graphics controllers (class 0x03 —
/// the iGPU). Everything else is substrate.
pub fn iommu_class(dev: Device) -> IommuClass {
    const CLASS_SERIAL_BUS: u8 = 0x0c;
    const SUBCLASS_USB: u8 = 0x03;
    const CLASS_DISPLAY: u8 = 0x03;
    if dev.class_code == CLASS_SERIAL_BUS && dev.subclass == SUBCLASS_USB {
        return IommuClass::Connectivity;
    }
    if dev.class_code == CLASS_NETWORK {
        return IommuClass::Connectivity;
    }
    if dev.class_code == CLASS_DISPLAY {
        return IommuClass::Display;
    }
    IommuClass::Substrate
}

/// Enumerate every PCI function present (full bus/device/function scan over the
/// CF8/CFC mechanism), so the VT-d driver can place each into a translation
/// domain keyed by its source-id. Mirrors `find_by_class`'s walk but collects
/// all devices. (BUILD-V's only caller; the scan is otherwise driver-specific.)
pub fn enumerate_all() -> Vec<Device> {
    let mut devices = Vec::new();
    for bus in 0..=u8::MAX {
        for device in 0..32u8 {
            if config_read_u16(bus, device, 0, 0x00) == VENDOR_ID_NONE {
                continue;
            }
            let header_type = config_read_u8(bus, device, 0, 0x0e);
            let functions = if header_type & HEADER_TYPE_MULTI_FUNCTION != 0 {
                8
            } else {
                1
            };
            for function in 0..functions {
                if let Some(dev) = read_device(bus, device, function) {
                    devices.push(dev);
                }
            }
        }
    }
    devices
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PciBar {
    Memory32 { base: u64, prefetchable: bool },
    Memory64 { base: u64, prefetchable: bool },
    Io { base: u32 },
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct VirtioPciCap {
    pub cfg_type: u8,
    pub bar: u8,
    pub offset: u32,
    pub length: u32,
    pub notify_off_multiplier: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct StorageScanSummary {
    pub functions_seen: u32,
    pub virtio_blk_seen: u32,
    pub virtio_blk_probe_ok: u32,
    pub ahci_seen: u32,
    pub ahci_probe_ok: u32,
    pub nvme_seen: u32,
    pub nvme_probe_ok: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct NetworkScanSummary {
    pub functions_seen: u32,
    pub virtio_net_seen: u32,
    pub virtio_net_probe_ok: u32,
}

#[cfg(feature = "virtio-accel")]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct AccelScanSummary {
    pub functions_seen: u32,
    pub virtio_accel_seen: u32,
    pub virtio_accel_probe_ok: u32,
}

#[cfg(feature = "accel-intel")]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct IntelGpuScanSummary {
    pub functions_seen: u32,
    pub intel_gpu_seen: u32,
    pub intel_gpu_probe_ok: u32,
}

pub fn scan_storage_devices() -> StorageScanSummary {
    let mut summary = StorageScanSummary::default();
    for bus in 0..=u8::MAX {
        for device in 0..32u8 {
            let vendor = config_read_u16(bus, device, 0, 0x00);
            if vendor == VENDOR_ID_NONE {
                continue;
            }

            let header_type = config_read_u8(bus, device, 0, 0x0e);
            let functions = if header_type & HEADER_TYPE_MULTI_FUNCTION != 0 {
                8
            } else {
                1
            };

            for function in 0..functions {
                if let Some(dev) = read_device(bus, device, function) {
                    summary.functions_seen += 1;
                    match classify_storage(dev) {
                        StorageKind::VirtioBlkModern => {
                            summary.virtio_blk_seen += 1;
                            serial_println!(
                                "[ kernel ] pci: virtio-blk modern {:02x}:{:02x}.{} vendor=0x{:04x} device=0x{:04x}",
                                dev.bus,
                                dev.device,
                                dev.function,
                                dev.vendor_id,
                                dev.device_id
                            );
                            if crate::drivers::block::virtio_blk::probe_pci(dev).is_ok() {
                                summary.virtio_blk_probe_ok += 1;
                            }
                        }
                        StorageKind::VirtioBlkLegacy => {
                            serial_println!(
                                "[ kernel ] pci: refusing legacy virtio-blk {:02x}:{:02x}.{} device=0x{:04x}",
                                dev.bus,
                                dev.device,
                                dev.function,
                                dev.device_id
                            );
                        }
                        StorageKind::AhciForward => {
                            summary.ahci_seen += 1;
                            serial_println!(
                                "[ kernel ] pci: AHCI controller {:02x}:{:02x}.{} vendor=0x{:04x} device=0x{:04x} class=0x{:06x}",
                                dev.bus,
                                dev.device,
                                dev.function,
                                dev.vendor_id,
                                dev.device_id,
                                dev.class_triplet()
                            );
                            if crate::drivers::block::ahci::probe_pci(dev).is_ok() {
                                summary.ahci_probe_ok += 1;
                            }
                        }
                        StorageKind::NvmeForward => {
                            summary.nvme_seen += 1;
                            serial_println!(
                                "[ kernel ] pci: NVMe controller {:02x}:{:02x}.{} vendor=0x{:04x} device=0x{:04x} class=0x{:06x}",
                                dev.bus,
                                dev.device,
                                dev.function,
                                dev.vendor_id,
                                dev.device_id,
                                dev.class_triplet()
                            );
                            if crate::drivers::block::nvme::probe_pci(dev).is_ok() {
                                summary.nvme_probe_ok += 1;
                            }
                        }
                        // Zenbook lane: Intel VMD / RST presents a RAID-class
                        // (0x0104) function and hides the NVMe behind its own
                        // PCI domain. We do not drive VMD, so no disk appears;
                        // say why instead of looking like a failed probe.
                        StorageKind::Other
                            if dev.vendor_id == 0x8086
                                && dev.class_code == CLASS_MASS_STORAGE
                                && dev.subclass == 0x04 =>
                        {
                            serial_println!(
                                "[ kernel ] pci: Intel RAID/VMD controller {:02x}:{:02x}.{} device=0x{:04x} - NVMe behind VMD is not visible (VMD unsupported); no internal disk will be probed",
                                dev.bus,
                                dev.device,
                                dev.function,
                                dev.device_id
                            );
                        }
                        StorageKind::Other => {}
                    }
                }
            }
        }
    }
    summary
}

/// Find the first function matching a full class triplet (Phase 7.5: xHCI
/// discovery — class 0x0C, subclass 0x03, prog-if 0x30).
pub fn find_by_class(class_code: u8, subclass: u8, prog_if: u8) -> Option<Device> {
    for bus in 0..=u8::MAX {
        for device in 0..32u8 {
            if config_read_u16(bus, device, 0, 0x00) == VENDOR_ID_NONE {
                continue;
            }
            let header_type = config_read_u8(bus, device, 0, 0x0e);
            let functions = if header_type & HEADER_TYPE_MULTI_FUNCTION != 0 {
                8
            } else {
                1
            };
            for function in 0..functions {
                if let Some(dev) = read_device(bus, device, function)
                    && dev.class_code == class_code
                    && dev.subclass == subclass
                    && dev.prog_if == prog_if
                {
                    return Some(dev);
                }
            }
        }
    }
    None
}

pub fn scan_network_devices() -> NetworkScanSummary {
    let mut summary = NetworkScanSummary::default();
    for bus in 0..=u8::MAX {
        for device in 0..32u8 {
            let vendor = config_read_u16(bus, device, 0, 0x00);
            if vendor == VENDOR_ID_NONE {
                continue;
            }

            let header_type = config_read_u8(bus, device, 0, 0x0e);
            let functions = if header_type & HEADER_TYPE_MULTI_FUNCTION != 0 {
                8
            } else {
                1
            };

            for function in 0..functions {
                if let Some(dev) = read_device(bus, device, function) {
                    summary.functions_seen += 1;
                    match classify_network(dev) {
                        NetworkKind::VirtioNetModern => {
                            summary.virtio_net_seen += 1;
                            serial_println!(
                                "[ kernel ] pci: virtio-net {:02x}:{:02x}.{} vendor=0x{:04x} device=0x{:04x}",
                                dev.bus,
                                dev.device,
                                dev.function,
                                dev.vendor_id,
                                dev.device_id
                            );
                            if crate::drivers::net::virtio_net::probe_pci(dev).is_ok() {
                                summary.virtio_net_probe_ok += 1;
                            }
                        }
                        NetworkKind::VirtioNetLegacy => {
                            serial_println!(
                                "[ kernel ] pci: refusing legacy virtio-net {:02x}:{:02x}.{} device=0x{:04x}",
                                dev.bus,
                                dev.device,
                                dev.function,
                                dev.device_id
                            );
                        }
                        NetworkKind::Other => {}
                    }
                }
            }
        }
    }
    summary
}

#[cfg(feature = "virtio-accel")]
pub fn scan_accel_devices() -> AccelScanSummary {
    let mut summary = AccelScanSummary::default();
    for bus in 0..=u8::MAX {
        for device in 0..32u8 {
            let vendor = config_read_u16(bus, device, 0, 0x00);
            if vendor == VENDOR_ID_NONE {
                continue;
            }

            let header_type = config_read_u8(bus, device, 0, 0x0e);
            let functions = if header_type & HEADER_TYPE_MULTI_FUNCTION != 0 {
                8
            } else {
                1
            };

            for function in 0..functions {
                if let Some(dev) = read_device(bus, device, function) {
                    summary.functions_seen += 1;
                    match classify_accel(dev) {
                        AccelKind::VirtioAccelModern => {
                            summary.virtio_accel_seen += 1;
                            serial_println!(
                                "[ kernel ] pci: virtio-accel modern {:02x}:{:02x}.{} vendor=0x{:04x} device=0x{:04x}",
                                dev.bus,
                                dev.device,
                                dev.function,
                                dev.vendor_id,
                                dev.device_id
                            );
                            match crate::drivers::accel::virtio_accel::probe_pci(dev) {
                                Ok(()) => summary.virtio_accel_probe_ok += 1,
                                Err(err) => {
                                    serial_println!(
                                        "[ kernel ] pci: virtio-accel probe failed {:02x}:{:02x}.{} err={:?}",
                                        dev.bus,
                                        dev.device,
                                        dev.function,
                                        err
                                    );
                                }
                            }
                        }
                        AccelKind::Other => {}
                    }
                }
            }
        }
    }
    summary
}

#[cfg(feature = "accel-intel")]
pub fn scan_intel_gpu_devices() -> IntelGpuScanSummary {
    let mut summary = IntelGpuScanSummary::default();
    for bus in 0..=u8::MAX {
        for device in 0..32u8 {
            let vendor = config_read_u16(bus, device, 0, 0x00);
            if vendor == VENDOR_ID_NONE {
                continue;
            }

            let header_type = config_read_u8(bus, device, 0, 0x0e);
            let functions = if header_type & HEADER_TYPE_MULTI_FUNCTION != 0 {
                8
            } else {
                1
            };

            for function in 0..functions {
                if let Some(dev) = read_device(bus, device, function) {
                    summary.functions_seen += 1;
                    if classify_intel_gpu(dev) == IntelGpuKind::Gen9IgpuCandidate {
                        summary.intel_gpu_seen += 1;
                        serial_println!(
                            // `rev` matters on Skylake specifically: i915 gates
                            // several Gen9 workarounds on the graphics stepping
                            // (e.g. WaInPlaceDecompressionHang on SKL A0..H0), so a
                            // stepping-attributable failure is undiagnosable without
                            // it. Cheap to log, impossible to recover after the fact.
                            "[ kernel ] pci: Intel Gen9 iGPU candidate {:02x}:{:02x}.{} vendor=0x{:04x} device=0x{:04x} rev=0x{:02x} class=0x{:06x}",
                            dev.bus,
                            dev.device,
                            dev.function,
                            dev.vendor_id,
                            dev.device_id,
                            dev.revision_id,
                            dev.class_triplet()
                        );
                        match crate::drivers::accel::accel_intel::probe_pci(dev) {
                            Ok(()) => summary.intel_gpu_probe_ok += 1,
                            Err(err) => {
                                serial_println!(
                                    "[ kernel ] pci: Intel Gen9 probe failed {:02x}:{:02x}.{} err={:?}",
                                    dev.bus,
                                    dev.device,
                                    dev.function,
                                    err
                                );
                            }
                        }
                    } else if dev.vendor_id == 0x8086 && dev.class_code == CLASS_DISPLAY {
                        // An Intel display device the allowlist REJECTED. Without
                        // this line a scan on unfamiliar silicon reports only
                        // `candidates=0` and the run is not self-diagnosing: the
                        // M710q's first Slice-4 attempt failed exactly this way and
                        // the device ID had to be recovered by hand afterwards.
                        // Naming the rejected ID makes the next unknown-SKU box a
                        // one-line diagnosis instead of another blind iteration.
                        serial_println!(
                            "[ kernel ] pci: Intel display device NOT in the Gen9 accel allowlist {:02x}:{:02x}.{} vendor=0x{:04x} device=0x{:04x} rev=0x{:02x} class=0x{:06x}",
                            dev.bus,
                            dev.device,
                            dev.function,
                            dev.vendor_id,
                            dev.device_id,
                            dev.revision_id,
                            dev.class_triplet()
                        );
                    }
                }
            }
        }
    }
    summary
}

pub fn for_each_virtio_vendor_cap<F>(dev: Device, mut f: F)
where
    F: FnMut(VirtioPciCap),
{
    let status = config_read_u16(dev.bus, dev.device, dev.function, 0x06);
    if status & STATUS_CAPABILITIES_LIST == 0 {
        return;
    }

    let mut ptr = config_read_u8(dev.bus, dev.device, dev.function, 0x34) & !0x03;
    let mut guard = 0u8;
    while ptr >= 0x40 && guard < 48 {
        let cap_id = config_read_u8(dev.bus, dev.device, dev.function, ptr);
        let next = config_read_u8(dev.bus, dev.device, dev.function, ptr + 1) & !0x03;
        if cap_id == PCI_CAP_ID_VENDOR_SPECIFIC {
            let cap_len = config_read_u8(dev.bus, dev.device, dev.function, ptr + 2);
            if cap_len >= 16 {
                let cfg_type = config_read_u8(dev.bus, dev.device, dev.function, ptr + 3);
                let bar = config_read_u8(dev.bus, dev.device, dev.function, ptr + 4);
                let offset = config_read_u32(dev.bus, dev.device, dev.function, ptr + 8);
                let length = config_read_u32(dev.bus, dev.device, dev.function, ptr + 12);
                let notify_off_multiplier = if cap_len >= 20 {
                    config_read_u32(dev.bus, dev.device, dev.function, ptr + 16)
                } else {
                    0
                };
                f(VirtioPciCap {
                    cfg_type,
                    bar,
                    offset,
                    length,
                    notify_off_multiplier,
                });
            }
        }
        if next == 0 || next == ptr {
            break;
        }
        ptr = next;
        guard += 1;
    }
}

pub fn bar(dev: Device, index: u8) -> Option<PciBar> {
    if index >= 6 {
        return None;
    }
    let offset = 0x10 + index * 4;
    let low = config_read_u32(dev.bus, dev.device, dev.function, offset);
    if low == 0 {
        return None;
    }
    if low & 0x1 != 0 {
        return Some(PciBar::Io { base: low & !0x3 });
    }

    let prefetchable = low & 0x8 != 0;
    let mem_type = (low >> 1) & 0x3;
    let base_low = (low & 0xffff_fff0) as u64;
    match mem_type {
        0x0 => Some(PciBar::Memory32 {
            base: base_low,
            prefetchable,
        }),
        0x2 if index < 5 => {
            let high = config_read_u32(dev.bus, dev.device, dev.function, offset + 4) as u64;
            Some(PciBar::Memory64 {
                base: (high << 32) | base_low,
                prefetchable,
            })
        }
        _ => None,
    }
}

/// Probe a memory BAR's advertised size using the architectural
/// write-all-ones / read-back / restore sequence (PCI 3.0 §6.2.5.1). The size
/// is the two's complement of the masked read-back, so a BAR that decodes
/// `N` bytes reads back `!(N-1)` in its address bits.
///
/// Returns `None` for I/O BARs, absent BARs, or a read-back of 0 (BAR
/// unimplemented). 64-bit BARs are probed as their low dword only, which is
/// sufficient for every region this kernel maps (all < 4 GiB in extent).
fn bar_size(dev: Device, bar_index: u8) -> Option<u64> {
    if bar_index >= 6 {
        return None;
    }
    let offset: u8 = 0x10 + bar_index * 4;
    let original = config_read_u32(dev.bus, dev.device, dev.function, offset);
    if original & 0x1 != 0 {
        return None; // I/O BAR — not memory-mappable
    }
    // A 64-bit BAR (type bits [2:1] == 0b10) spans this dword and the next.
    let is_64bit = (original >> 1) & 0x3 == 0x2;
    if is_64bit && bar_index >= 5 {
        return None; // a 64-bit BAR cannot start at the last slot
    }
    let offset_high = offset + 4;
    let original_high = if is_64bit {
        config_read_u32(dev.bus, dev.device, dev.function, offset_high)
    } else {
        0
    };

    // Writing all-ones makes the device decode an enormous range for the
    // duration of the probe. Clear MEMORY/IO decode first so it cannot claim
    // addresses it does not own while the BAR is scratch — every caller here
    // has already enabled decode (and some, bus mastering) by this point.
    const COMMAND: u8 = 0x04;
    const DECODE_BITS: u16 = (1 << 0) | (1 << 1); // I/O | MEMORY
    let command = config_read_u16(dev.bus, dev.device, dev.function, COMMAND);
    config_write_u16(
        dev.bus,
        dev.device,
        dev.function,
        COMMAND,
        command & !DECODE_BITS,
    );

    config_write_u32(dev.bus, dev.device, dev.function, offset, u32::MAX);
    let probed_low = config_read_u32(dev.bus, dev.device, dev.function, offset);
    let probed_high = if is_64bit {
        config_write_u32(dev.bus, dev.device, dev.function, offset_high, u32::MAX);
        config_read_u32(dev.bus, dev.device, dev.function, offset_high)
    } else {
        u32::MAX
    };

    // Restore the BAR, then decode, in that order — the device must never be
    // decoding while its BAR still holds the scratch value.
    config_write_u32(dev.bus, dev.device, dev.function, offset, original);
    if is_64bit {
        config_write_u32(
            dev.bus,
            dev.device,
            dev.function,
            offset_high,
            original_high,
        );
    }
    config_write_u16(dev.bus, dev.device, dev.function, COMMAND, command);

    let mask = ((probed_high as u64) << 32) | ((probed_low & 0xffff_fff0) as u64);
    if mask == 0 {
        return None; // unimplemented BAR
    }
    Some((!mask).wrapping_add(1))
}

/// Map a sub-range of a memory BAR.
///
/// UNCHANGED behaviour, deliberately: six drivers (xhci, ahci, nvme,
/// virtio_blk, virtio_net, virtio_accel) depend on this exact mapping, and at
/// least two of them request more than QEMU's device advertises (`ahci` maps
/// 0x2000 from a 0x1000 ABAR). Adding validation here is a real change with a
/// real blast radius and belongs in its own build — see
/// [`map_bar_region_validated`] for the checked variant the Slice-4 AAG asked
/// for, and the RETURN for the over-mapping finding.
pub fn map_bar_region(dev: Device, bar_index: u8, offset: u32, length: u32) -> Option<VirtAddr> {
    let base = match bar(dev, bar_index)? {
        PciBar::Memory32 { base, .. } | PciBar::Memory64 { base, .. } => base,
        PciBar::Io { .. } => return None,
    };
    let region_phys = base.checked_add(offset as u64)?;
    let len = length.max(1) as u64;
    map_mmio_range(region_phys, len);
    Some(crate::mm::phys_to_virt(PhysAddr::new(region_phys)))
}

/// Map a sub-range of a memory BAR, **refusing** unless `offset + length` fits
/// inside the BAR's advertised size (Slice-4 AAG finding 2, 2026-08-01).
///
/// Without this, a driver mapping more than the device decodes gets MMIO the
/// device does not own, and any downstream in-range offset check proves only
/// "inside the requested mapping", not "inside a real BAR". A BAR whose size
/// cannot be determined is refused rather than assumed large enough.
///
/// Used by the Intel accel probe, where the gap was raised. Not wired into the
/// other six BAR mappings — that is a separate change with its own gate.
#[allow(dead_code)] // consumed by the accel probe
pub fn map_bar_region_validated(
    dev: Device,
    bar_index: u8,
    offset: u32,
    length: u32,
) -> Option<VirtAddr> {
    let size = bar_size(dev, bar_index)?;
    let len = length.max(1) as u64;
    let end = (offset as u64).checked_add(len)?;
    if end > size {
        crate::serial_println!(
            "pci: BAR{} region refused — offset={:#x} len={:#x} exceeds advertised size {:#x} ({:02x}:{:02x}.{})",
            bar_index,
            offset,
            len,
            size,
            dev.bus,
            dev.device,
            dev.function
        );
        return None;
    }
    map_bar_region(dev, bar_index, offset, length)
}

pub fn enable_memory_and_bus_mastering(dev: Device) {
    const PCI_COMMAND_IO: u16 = 1 << 0;
    const PCI_COMMAND_MEMORY: u16 = 1 << 1;
    const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;

    let command = config_read_u16(dev.bus, dev.device, dev.function, 0x04);
    config_write_u16(
        dev.bus,
        dev.device,
        dev.function,
        0x04,
        command | PCI_COMMAND_IO | PCI_COMMAND_MEMORY | PCI_COMMAND_BUS_MASTER,
    );
}

/// Enable I/O + memory decode **without** granting bus-master (DMA) rights.
///
/// The split half of [`enable_memory_and_bus_mastering`], for a driver that
/// must map and read registers before its DMA containment is proven. MMIO
/// access is host-initiated and cannot reach memory the device does not
/// already own, so it is safe ahead of containment; bus mastering is not.
/// (Slice-4 AAG finding 1: "bus mastering must not precede containment".)
#[allow(dead_code)] // consumed by the containment-gated accel probe
pub fn enable_memory_decode(dev: Device) {
    const PCI_COMMAND_IO: u16 = 1 << 0;
    const PCI_COMMAND_MEMORY: u16 = 1 << 1;

    let command = config_read_u16(dev.bus, dev.device, dev.function, 0x04);
    config_write_u16(
        dev.bus,
        dev.device,
        dev.function,
        0x04,
        command | PCI_COMMAND_IO | PCI_COMMAND_MEMORY,
    );
}

/// Read a device's PCI COMMAND register (offset 0x04).
///
/// Exposed so a scenario can assert **hardware state** rather than trust a
/// code path: the I-14(c) refusal discriminator checks that BUS MASTER
/// (bit 2) is genuinely clear on a device whose probe was refused, which is
/// evidence no amount of control-flow reasoning provides.
#[allow(dead_code)] // gate seam — consumed by the containment-refusal scenario
pub fn command_register_at(bus: u8, device: u8, function: u8) -> u16 {
    config_read_u16(bus, device, function, 0x04)
}

/// PCI COMMAND bit 2 — bus master (DMA) enable.
#[allow(dead_code)] // gate seam
pub const COMMAND_BUS_MASTER: u16 = 1 << 2;

/// Grant bus-master (DMA) rights. Separated from decode enabling so a caller
/// can prove DMA containment (I-14(c)) between the two steps.
#[allow(dead_code)] // consumed by the containment-gated accel probe
pub fn enable_bus_mastering(dev: Device) {
    const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;

    let command = config_read_u16(dev.bus, dev.device, dev.function, 0x04);
    config_write_u16(
        dev.bus,
        dev.device,
        dev.function,
        0x04,
        command | PCI_COMMAND_BUS_MASTER,
    );
}

pub fn configure_msi_fixed_vector(dev: Device, vector: u8) -> bool {
    let Some(cap) = find_capability(dev, PCI_CAP_ID_MSI) else {
        return false;
    };
    // CP-68: the BSP's APIC ID, not a hard-coded 0; refused if it needs more
    // than the 8-bit MSI destination.
    let Some(message_address) = crate::arch::x86_64::apic::msi_address_for_bsp() else {
        return false;
    };
    let control = config_read_u16(dev.bus, dev.device, dev.function, cap + 2);
    let is_64_bit = control & (1 << 7) != 0;
    let has_mask_bits = control & (1 << 8) != 0;

    config_write_u32(dev.bus, dev.device, dev.function, cap + 4, message_address);
    if is_64_bit {
        config_write_u32(dev.bus, dev.device, dev.function, cap + 8, 0);
        config_write_u16(dev.bus, dev.device, dev.function, cap + 12, vector as u16);
        if has_mask_bits {
            config_write_u32(dev.bus, dev.device, dev.function, cap + 16, 0);
        }
    } else {
        config_write_u16(dev.bus, dev.device, dev.function, cap + 8, vector as u16);
        if has_mask_bits {
            config_write_u32(dev.bus, dev.device, dev.function, cap + 12, 0);
        }
    }

    let enabled = (control & !(0x7 << 4)) | 1;
    config_write_u16(dev.bus, dev.device, dev.function, cap + 2, enabled);
    true
}

pub fn configure_msix_fixed_vector(dev: Device, vector: u8, table_entry: u16) -> bool {
    let Some(cap) = find_capability(dev, PCI_CAP_ID_MSIX) else {
        return false;
    };
    let control = config_read_u16(dev.bus, dev.device, dev.function, cap + 2);
    let table_size = ((control & 0x07ff) as u32) + 1;
    if table_entry as u32 >= table_size {
        return false;
    }
    let table = config_read_u32(dev.bus, dev.device, dev.function, cap + 4);
    let bir = (table & 0x7) as u8;
    let table_offset = table & !0x7;
    let Some(table_virt) = map_bar_region(dev, bir, table_offset, table_size * 16) else {
        return false;
    };
    // CP-68: as in `configure_msi_fixed_vector`.
    let Some(message_address) = crate::arch::x86_64::apic::msi_address_for_bsp() else {
        return false;
    };

    let entry_offset = table_entry as usize * 16;
    // SAFETY: the MSI-X table BAR was mapped above. The selected entry is
    // within the table size reported by PCI config space.
    unsafe {
        mmio_write32(table_virt, entry_offset + 12, 1);
        mmio_write32(table_virt, entry_offset, message_address);
        mmio_write32(table_virt, entry_offset + 4, 0);
        mmio_write32(table_virt, entry_offset + 8, vector as u32);
        mmio_write32(table_virt, entry_offset + 12, 0);
    }

    let enabled = (control | (1 << 15)) & !(1 << 14);
    config_write_u16(dev.bus, dev.device, dev.function, cap + 2, enabled);
    true
}

fn map_mmio_range(phys: u64, len: u64) {
    let start = phys & !0xfff;
    let end = (phys + len + 0xfff) & !0xfff;
    let mut cursor = start;
    while cursor < end {
        crate::mm::map_kernel_mmio_page(PhysAddr::new(cursor));
        cursor += 0x1000;
    }
}

unsafe fn mmio_write32(base: VirtAddr, offset: usize, value: u32) {
    unsafe {
        write_volatile((base.as_u64() as usize + offset) as *mut u32, value.to_le());
    }
}

/// PCI Power Management capability ID (PCI PM 1.2 §3.2).
#[cfg(any(feature = "xhci-silicon", feature = "i2c-hid-touchpad"))]
const PCI_CAP_ID_PM: u8 = 0x01;

/// The function's PMCSR (Power Management Control/Status, PM capability +
/// 4), or `None` when it has no PM capability. Zenbook USB input lane.
#[cfg(any(feature = "xhci-silicon", feature = "i2c-hid-touchpad"))]
pub fn power_management_csr(dev: Device) -> Option<u16> {
    let cap = find_capability(dev, PCI_CAP_ID_PM)?;
    Some(config_read_u16(dev.bus, dev.device, dev.function, cap + 4))
}

/// Whether the function lists capability `cap_id` (the USB summary's MSI /
/// MSI-X columns). Zenbook USB input lane.
#[cfg(feature = "xhci-silicon")]
pub fn has_capability(dev: Device, cap_id: u8) -> bool {
    find_capability(dev, cap_id).is_some()
}

/// Write the function's PMCSR (no-op without a PM capability).
#[cfg(any(feature = "xhci-silicon", feature = "i2c-hid-touchpad"))]
pub fn write_power_management_csr(dev: Device, value: u16) {
    if let Some(cap) = find_capability(dev, PCI_CAP_ID_PM) {
        config_write_u16(dev.bus, dev.device, dev.function, cap + 4, value);
    }
}

/// Read a 16-bit config register of `dev` (the RTL8125 bind: COMMAND, PMCSR,
/// PCIe Link Control). Feature-gated so `rtl8125`-off builds are unchanged.
#[cfg(feature = "rtl8125")]
pub fn config_read16(dev: Device, offset: u8) -> u16 {
    config_read_u16(dev.bus, dev.device, dev.function, offset)
}

/// Write a 16-bit config register of `dev`. See [`config_read16`]. Also the
/// I2C-HID touchpad's command register around its BAR restore.
#[cfg(any(feature = "rtl8125", feature = "i2c-hid-touchpad"))]
pub fn config_write16(dev: Device, offset: u8, value: u16) {
    config_write_u16(dev.bus, dev.device, dev.function, offset, value)
}

/// The config-space offset of `dev`'s capability `cap_id`, if listed.
#[cfg(feature = "rtl8125")]
pub fn capability(dev: Device, cap_id: u8) -> Option<u8> {
    find_capability(dev, cap_id)
}

/// Read a 32-bit config register of `dev` (the I2C-HID touchpad's raw BAR
/// dwords across its D3hot -> D0 transition).
#[cfg(feature = "i2c-hid-touchpad")]
pub fn config_read32(dev: Device, offset: u8) -> u32 {
    config_read_u32(dev.bus, dev.device, dev.function, offset)
}

/// Write a 32-bit config register of `dev`. See [`config_read32`].
#[cfg(feature = "i2c-hid-touchpad")]
pub fn config_write32(dev: Device, offset: u8, value: u32) {
    config_write_u32(dev.bus, dev.device, dev.function, offset, value)
}

/// Map `len` bytes of physical MMIO at `phys` (uncached) and return its
/// virtual address: the I2C-HID touchpad reads a candidate BAR slot before
/// claiming it.
#[cfg(feature = "i2c-hid-bar-assign")]
pub fn map_mmio_for_probe(phys: u64, len: u64) -> VirtAddr {
    map_mmio_range(phys, len);
    crate::mm::phys_to_virt(PhysAddr::new(phys))
}

/// Clear bus-master (DMA) rights (the I2C-HID touchpad is PIO only).
#[cfg(feature = "i2c-hid-touchpad")]
pub fn disable_bus_mastering(dev: Device) {
    const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;

    let command = config_read_u16(dev.bus, dev.device, dev.function, 0x04);
    config_write_u16(
        dev.bus,
        dev.device,
        dev.function,
        0x04,
        command & !PCI_COMMAND_BUS_MASTER,
    );
}

/// The function at one address, if anything answers there (the I2C-HID
/// touchpad lane reads only the function its quirk entry names).
#[cfg(feature = "i2c-hid-touchpad")]
pub fn device_at(bus: u8, device: u8, function: u8) -> Option<Device> {
    read_device(bus, device, function)
}

fn find_capability(dev: Device, cap_id: u8) -> Option<u8> {
    let status = config_read_u16(dev.bus, dev.device, dev.function, 0x06);
    if status & STATUS_CAPABILITIES_LIST == 0 {
        return None;
    }

    let mut ptr = config_read_u8(dev.bus, dev.device, dev.function, 0x34) & !0x03;
    let mut guard = 0u8;
    while ptr >= 0x40 && guard < 48 {
        let current = config_read_u8(dev.bus, dev.device, dev.function, ptr);
        let next = config_read_u8(dev.bus, dev.device, dev.function, ptr + 1) & !0x03;
        if current == cap_id {
            return Some(ptr);
        }
        if next == 0 || next == ptr {
            break;
        }
        ptr = next;
        guard += 1;
    }
    None
}

/// OT-90 K5 (Janus relay 228 item 3): disable MSI and function-mask MSI-X on
/// every PCI function, so no device interrupt bypasses the masked I/O APICs
/// to a stopped CPU. Runs after K4 with one CPU left (CF8/CFC can't
/// interleave), allocates nothing and takes no lock: a brute-force walk of
/// every bus, device and function. Returns (MSI disabled, MSI-X masked).
#[cfg(feature = "power-stop-cpus")]
pub fn mask_msi_for_shutdown() -> (u32, u32) {
    let (mut msi, mut msix) = (0u32, 0u32);
    for bus in 0..=255u8 {
        for device in 0..32u8 {
            for function in 0..8u8 {
                let Some(dev) = read_device(bus, device, function) else {
                    if function == 0 {
                        break;
                    }
                    continue;
                };
                if let Some(cap) = find_capability(dev, PCI_CAP_ID_MSI) {
                    let control = config_read_u16(bus, device, function, cap + 2);
                    if control & 1 != 0 {
                        config_write_u16(bus, device, function, cap + 2, control & !1);
                        msi += 1;
                    }
                }
                if let Some(cap) = find_capability(dev, PCI_CAP_ID_MSIX) {
                    let control = config_read_u16(bus, device, function, cap + 2);
                    if control & (1 << 15) != 0 {
                        // Function Mask (bit 14): every vector masked at once.
                        config_write_u16(bus, device, function, cap + 2, control | (1 << 14));
                        msix += 1;
                    }
                }
                if function == 0 && dev.header_type & 0x80 == 0 {
                    break;
                }
            }
        }
    }
    (msi, msix)
}

fn read_device(bus: u8, device: u8, function: u8) -> Option<Device> {
    let vendor_id = config_read_u16(bus, device, function, 0x00);
    if vendor_id == VENDOR_ID_NONE {
        return None;
    }
    let device_id = config_read_u16(bus, device, function, 0x02);
    let revision_id = config_read_u8(bus, device, function, 0x08);
    let prog_if = config_read_u8(bus, device, function, 0x09);
    let subclass = config_read_u8(bus, device, function, 0x0a);
    let class_code = config_read_u8(bus, device, function, 0x0b);
    let header_type = config_read_u8(bus, device, function, 0x0e);
    Some(Device {
        bus,
        device,
        function,
        vendor_id,
        device_id,
        revision_id,
        prog_if,
        subclass,
        class_code,
        header_type,
    })
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum StorageKind {
    VirtioBlkModern,
    VirtioBlkLegacy,
    AhciForward,
    NvmeForward,
    Other,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum NetworkKind {
    VirtioNetModern,
    VirtioNetLegacy,
    Other,
}

#[cfg(feature = "virtio-accel")]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum AccelKind {
    VirtioAccelModern,
    Other,
}

#[cfg(feature = "accel-intel")]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum IntelGpuKind {
    /// A Gen9/Gen9.5 GT2 integrated GPU this build's accel driver binds
    /// (HD 530 Skylake, or UHD 630 Coffee/Comet Lake).
    Gen9IgpuCandidate,
    Other,
}

fn classify_storage(dev: Device) -> StorageKind {
    if dev.vendor_id == crate::drivers::block::virtio_blk::VIRTIO_VENDOR_ID
        && dev.device_id == crate::drivers::block::virtio_blk::VIRTIO_BLK_MODERN_DEVICE_ID
    {
        return StorageKind::VirtioBlkModern;
    }
    if dev.vendor_id == crate::drivers::block::virtio_blk::VIRTIO_VENDOR_ID
        && dev.device_id == crate::drivers::block::virtio_blk::VIRTIO_BLK_LEGACY_DEVICE_ID
    {
        return StorageKind::VirtioBlkLegacy;
    }
    if dev.class_code == CLASS_MASS_STORAGE
        && dev.subclass == SUBCLASS_SATA
        && dev.prog_if == PROGIF_AHCI
    {
        return StorageKind::AhciForward;
    }
    if dev.class_code == CLASS_MASS_STORAGE
        && dev.subclass == SUBCLASS_NVM
        && dev.prog_if == PROGIF_NVME
    {
        return StorageKind::NvmeForward;
    }
    StorageKind::Other
}

#[cfg(feature = "virtio-accel")]
fn classify_accel(dev: Device) -> AccelKind {
    if dev.vendor_id == crate::drivers::accel::virtio_accel::VIRTIO_VENDOR_ID
        && dev.device_id == crate::drivers::accel::virtio_accel::VIRTIO_ACCEL_MODERN_DEVICE_ID
    {
        return AccelKind::VirtioAccelModern;
    }
    AccelKind::Other
}

#[cfg(feature = "accel-intel")]
fn classify_intel_gpu(dev: Device) -> IntelGpuKind {
    if dev.vendor_id == 0x8086
        && dev.class_code == CLASS_DISPLAY
        && crate::drivers::accel::accel_intel::is_supported_gen9_igpu_id(dev.device_id)
    {
        return IntelGpuKind::Gen9IgpuCandidate;
    }
    IntelGpuKind::Other
}

fn classify_network(dev: Device) -> NetworkKind {
    if dev.vendor_id == crate::drivers::net::virtio_net::VIRTIO_VENDOR_ID
        && dev.device_id == crate::drivers::net::virtio_net::VIRTIO_NET_MODERN_DEVICE_ID
    {
        return NetworkKind::VirtioNetModern;
    }
    if dev.vendor_id == crate::drivers::net::virtio_net::VIRTIO_VENDOR_ID
        && dev.device_id == crate::drivers::net::virtio_net::VIRTIO_NET_LEGACY_DEVICE_ID
    {
        return NetworkKind::VirtioNetLegacy;
    }
    if dev.class_code == CLASS_NETWORK && dev.subclass == SUBCLASS_ETHERNET {
        return NetworkKind::Other;
    }
    NetworkKind::Other
}

fn config_read_u8(bus: u8, device: u8, function: u8, offset: u8) -> u8 {
    let value = config_read_u32(bus, device, function, offset);
    ((value >> ((offset & 0x3) * 8)) & 0xff) as u8
}

fn config_read_u16(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
    let value = config_read_u32(bus, device, function, offset);
    ((value >> ((offset & 0x2) * 8)) & 0xffff) as u16
}

fn config_read_u32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xfc);
    // SAFETY: CF8/CFC config-space access is serialized by running the scan
    // during boot before concurrent PCI users exist.
    unsafe {
        let mut address_port = Port::<u32>::new(CONFIG_ADDRESS);
        let mut data_port = Port::<u32>::new(CONFIG_DATA);
        address_port.write(address);
        data_port.read()
    }
}

fn config_write_u16(bus: u8, device: u8, function: u8, offset: u8, value: u16) {
    let shift = ((offset & 0x2) * 8) as u32;
    let mask = !(0xffffu32 << shift);
    let old = config_read_u32(bus, device, function, offset);
    let new = (old & mask) | ((value as u32) << shift);
    config_write_u32(bus, device, function, offset, new);
}

fn config_write_u32(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    let address = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xfc);
    // SAFETY: BUILD-1 PCI configuration writes are limited to early device
    // enablement before concurrent PCI users exist.
    unsafe {
        let mut address_port = Port::<u32>::new(CONFIG_ADDRESS);
        let mut data_port = Port::<u32>::new(CONFIG_DATA);
        address_port.write(address);
        data_port.write(value);
    }
}
