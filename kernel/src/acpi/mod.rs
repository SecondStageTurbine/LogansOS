// SPDX-License-Identifier: MPL-2.0
//! Minimal ACPI table discovery — Phase 7.5 BUILD-V (ISC-1), the entry point
//! for the DMAR (DMA Remapping Reporting) table the VT-d driver consumes.
//!
//! Scope is deliberately narrow: walk RSDP → XSDT/RSDT, validate checksums,
//! locate the **DMAR** table, hand its bytes to the pure `dmar_logic` parser,
//! and cache the result (`DmarInfo`). No FADT/MADT/etc. — those are not needed
//! by BUILD-V and stay unbuilt (scope-guard). The whole module compiles only
//! under the `vtd` feature (the kill-switch: `vtd`-off ⇒ this never runs and
//! the kernel is bit-for-bit pre-VT-d).
//!
//! **Physical-memory reads.** Limine's HHDM direct-maps physical RAM (including
//! the ACPI-reclaimable regions the tables live in), so `mm::phys_to_virt`
//! suffices to read the RSDP/XSDT/DMAR — no explicit MMIO mapping (that is for
//! the DRHD register block, which the `iommu` driver maps uncacheable). The
//! RSDP address from Limine is **physical** because we boot base revision 3
//! (see `RsdpRespData` — "for base revision 3 only, this is a physical
//! address").

use x86_64::PhysAddr;

use crate::mm;

pub mod dmar;

#[path = "dmar_logic.rs"]
pub mod dmar_logic;

/// CP-69: the FADT PM timer and HPET locations (`lapic-timer` calibration).
#[cfg(feature = "lapic-timer")]
#[path = "timers_logic.rs"]
pub mod timers_logic;

/// i8042 lane: the MADT's I/O APICs and ISA interrupt source overrides.
#[cfg(any(feature = "ps2-keyboard", feature = "i2c-hid-touchpad"))]
#[path = "madt_logic.rs"]
pub mod madt_logic;

pub use dmar::DmarInfo;

use crate::sync::SwitchMutex;

/// The parsed DMAR, set once at boot. Leaf lock (set-once, read-mostly) — never
/// held across any other acquisition, matching the arena's `SPANS` discipline.
static DMAR: SwitchMutex<Option<DmarInfo>> = SwitchMutex::new(None);

/// ACPI signatures we look for (4-byte table signatures / 8-byte RSDP).
const SIG_RSDP: &[u8; 8] = b"RSD PTR ";
const SIG_XSDT: &[u8; 4] = b"XSDT";
const SIG_RSDT: &[u8; 4] = b"RSDT";
const SIG_DMAR: &[u8; 4] = b"DMAR";
#[cfg(any(
    feature = "lapic-timer",
    feature = "ps2-keyboard",
    feature = "i2c-hid-touchpad",
    feature = "power-control"
))]
const SIG_FADT: &[u8; 4] = b"FACP";
#[cfg(any(feature = "ps2-keyboard", feature = "i2c-hid-touchpad"))]
const SIG_MADT: &[u8; 4] = b"APIC";
#[cfg(feature = "lapic-timer")]
const SIG_HPET: &[u8; 4] = b"HPET";

/// Read `len` bytes of physical memory as a slice via the HHDM direct map.
///
/// # Safety
/// `phys..phys+len` must be valid, HHDM-mapped physical memory (true for ACPI
/// tables under Limine). The returned slice is only read, never written.
unsafe fn phys_slice(phys: u64, len: usize) -> &'static [u8] {
    // ACPI tables can live in reserved/BIOS memory the kernel's direct map does
    // NOT cover (QEMU puts the RSDP in 0xE0000-0xFFFFF). Map every page we touch
    // that isn't already mapped — skipping huge-page-covered RAM, which
    // `map_kernel_mmio_page` would refuse (it asserts on a huge-page crossing).
    let start = phys & !0xfff;
    let end = (phys.saturating_add(len as u64).saturating_add(0xfff)) & !0xfff;
    let mut page = start;
    while page < end {
        let virt = mm::phys_to_virt(PhysAddr::new(page));
        if !mm::kernel_virt_mapped(virt) {
            mm::map_kernel_mmio_page(PhysAddr::new(page));
        }
        page += 0x1000;
    }
    let virt = mm::phys_to_virt(PhysAddr::new(phys));
    unsafe { core::slice::from_raw_parts(virt.as_ptr::<u8>(), len) }
}

/// ACPI checksum: the 8-bit sum of all bytes in the structure must be zero.
fn checksum_ok(bytes: &[u8]) -> bool {
    bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b)) == 0
}

fn rd_u32(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn rd_u64(b: &[u8], off: usize) -> Option<u64> {
    let s = b.get(off..off + 8)?;
    Some(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

/// Locate the root system descriptor table (XSDT preferred, RSDT fallback) from
/// the RSDP at `rsdp_phys`. Returns `(root_table_phys, is_xsdt)`.
fn locate_root_table(rsdp_phys: u64) -> Option<(u64, bool)> {
    // RSDP is 20 bytes (rev 0) or 36 bytes (rev 2+). Read 36 and validate.
    let rsdp = unsafe { phys_slice(rsdp_phys, 36) };
    if &rsdp[0..8] != SIG_RSDP {
        crate::serial_println!(
            "[ kernel ] acpi: RSDP signature mismatch at {:#x}",
            rsdp_phys
        );
        return None;
    }
    let revision = rsdp[15];
    // Revision 0: the first 20 bytes must checksum to zero.
    if !checksum_ok(&rsdp[0..20]) {
        crate::serial_println!("[ kernel ] acpi: RSDP v1 checksum bad");
        return None;
    }
    if revision >= 2 {
        // Extended structure: full-length extended checksum, then the XSDT.
        let length = rd_u32(rsdp, 20)? as usize;
        let xsdt = rd_u64(rsdp, 24)?;
        if checksum_ok(&rsdp[0..length.min(36)]) && xsdt != 0 {
            return Some((xsdt, true));
        }
    }
    // Fall back to the 32-bit RSDT.
    let rsdt = rd_u32(rsdp, 16)? as u64;
    if rsdt != 0 { Some((rsdt, false)) } else { None }
}

/// Walk the XSDT/RSDT entries, returning the physical address of the table
/// whose signature matches `sig`, if present and checksum-valid.
fn find_table(root_phys: u64, is_xsdt: bool, sig: &[u8; 4]) -> Option<u64> {
    find_table_in(root_phys, is_xsdt, sig, &|_, _| true)
}

/// The largest root table (XSDT/RSDT) read: 1 MiB, as for any table
/// (`read_table`). A real XSDT is a few hundred bytes; the cap keeps a
/// damaged length from asking for gigabytes before the checksum is seen.
const MAX_ROOT_LEN: usize = 1 << 20;

/// [`find_table`], reading a physical range only after `readable(phys, len)`
/// accepts it (L7.1's firmware-memory check; `find_table` accepts all).
fn find_table_in(
    root_phys: u64,
    is_xsdt: bool,
    sig: &[u8; 4],
    readable: &dyn Fn(u64, usize) -> bool,
) -> Option<u64> {
    if !readable(root_phys, 36) {
        return None;
    }
    // SDT header is 36 bytes; the length field is at offset 4.
    let header = unsafe { phys_slice(root_phys, 36) };
    let length = rd_u32(header, 4)? as usize;
    if !(36..=MAX_ROOT_LEN).contains(&length) || !readable(root_phys, length) {
        return None;
    }
    // Validate the root table's own signature matches the kind the RSDP claimed.
    let expect_sig: &[u8; 4] = if is_xsdt { SIG_XSDT } else { SIG_RSDT };
    if &header[0..4] != expect_sig {
        crate::serial_println!("[ kernel ] acpi: root table signature mismatch");
        return None;
    }
    let table = unsafe { phys_slice(root_phys, length) };
    if !checksum_ok(table) {
        crate::serial_println!("[ kernel ] acpi: root table checksum bad");
        return None;
    }
    let entry_size = if is_xsdt { 8 } else { 4 };
    let entries = (length - 36) / entry_size;
    for i in 0..entries {
        let off = 36 + i * entry_size;
        let entry_phys = if is_xsdt {
            rd_u64(table, off)?
        } else {
            rd_u32(table, off)? as u64
        };
        if !readable(entry_phys, 4) {
            continue;
        }
        // Read just the 4-byte signature of the pointed-to table.
        let sub = unsafe { phys_slice(entry_phys, 4) };
        if sub == sig {
            return Some(entry_phys);
        }
    }
    None
}

/// Parse the ACPI tables starting from the RSDP and cache the DMAR. Idempotent
/// (a second call is a no-op). Logs the V1 breadcrumb (DMAR present/absent) —
/// "no DMAR" is the conclusive "enable VT-d in BIOS" verdict on real silicon.
pub fn init(rsdp_phys: u64) {
    if DMAR.lock().is_some() {
        return;
    }
    if rsdp_phys == 0 {
        crate::serial_println!("[ kernel ] acpi: no RSDP from bootloader");
        return;
    }
    let Some((root_phys, is_xsdt)) = locate_root_table(rsdp_phys) else {
        crate::serial_println!("[ kernel ] acpi: RSDP/root table parse failed");
        return;
    };
    crate::serial_println!(
        "[ kernel ] acpi: root table {} at {:#x}",
        if is_xsdt { "XSDT" } else { "RSDT" },
        root_phys
    );
    let Some(dmar_phys) = find_table(root_phys, is_xsdt, SIG_DMAR) else {
        // V1: no DMAR ⇒ VT-d not exposed (BIOS) — conclusive, distinct code.
        crate::serial_println!(
            "[ kernel ] acpi: no DMAR table (VT-d not exposed — enable in BIOS)"
        );
        return;
    };
    // Read the full DMAR table and validate its checksum.
    let header = unsafe { phys_slice(dmar_phys, 36) };
    let Some(length) = rd_u32(header, 4) else {
        return;
    };
    let table = unsafe { phys_slice(dmar_phys, length as usize) };
    if !checksum_ok(table) {
        crate::serial_println!("[ kernel ] acpi: DMAR checksum bad");
        return;
    }
    match dmar::DmarInfo::parse(table) {
        Some(info) => {
            crate::serial_println!(
                "[ kernel ] acpi: DMAR parsed — haw={} flags={:#x} drhd={} rmrr={}",
                info.host_addr_width,
                info.flags,
                info.drhd_bases.len(),
                info.rmrrs.len()
            );
            for (i, base) in info.drhd_bases.iter().enumerate() {
                crate::serial_println!(
                    "[ kernel ] acpi: DRHD[{}] reg_base={:#x} include_pci_all={}",
                    i,
                    base,
                    info.drhd_include_all.get(i).copied().unwrap_or(false)
                );
            }
            for (i, (b, l)) in info.rmrrs.iter().enumerate() {
                crate::serial_println!("[ kernel ] acpi: RMRR[{}] [{:#x}, {:#x}]", i, b, l);
            }
            *DMAR.lock() = Some(info);
        }
        None => {
            crate::serial_println!("[ kernel ] acpi: DMAR parse yielded no remapping units");
        }
    }
}

/// Run `f` with the cached DMAR info, if a DMAR was parsed. Returns `None` if
/// VT-d is not available (no DMAR table). The closure form avoids handing out a
/// reference past the (leaf) lock.
pub fn with_dmar<R>(f: impl FnOnce(&DmarInfo) -> R) -> Option<R> {
    DMAR.lock().as_ref().map(f)
}

/// Whether a usable DMAR table was found (VT-d is exposed by the platform).
pub fn dmar_available() -> bool {
    DMAR.lock().is_some()
}

/// The platform clocks the ACPI tables name (CP-69): the HPET block's
/// physical address and the ACPI PM timer. Either may be absent.
#[cfg(feature = "lapic-timer")]
#[derive(Clone, Copy, Debug, Default)]
pub struct TimerHardware {
    pub hpet_phys: Option<u64>,
    pub pm_timer: Option<timers_logic::PmTimer>,
}

/// A checksum-valid table found under the root, whole.
#[cfg(any(
    feature = "lapic-timer",
    feature = "ps2-keyboard",
    feature = "i2c-hid-touchpad",
    feature = "power-control"
))]
fn read_table(root_phys: u64, is_xsdt: bool, sig: &[u8; 4]) -> Option<&'static [u8]> {
    read_table_in(root_phys, is_xsdt, sig, &|_, _| true)
}

/// [`read_table`] with [`find_table_in`]'s `readable` check on every range.
fn read_table_in(
    root_phys: u64,
    is_xsdt: bool,
    sig: &[u8; 4],
    readable: &dyn Fn(u64, usize) -> bool,
) -> Option<&'static [u8]> {
    let phys = find_table_in(root_phys, is_xsdt, sig, readable)?;
    if !readable(phys, 36) {
        return None;
    }
    // SAFETY: an ACPI table the root table points at (HHDM-readable).
    let header = unsafe { phys_slice(phys, 36) };
    let length = rd_u32(header, 4)? as usize;
    if !(36..=1 << 20).contains(&length) || !readable(phys, length) {
        return None;
    }
    // SAFETY: as above, for the length the header declares.
    let table = unsafe { phys_slice(phys, length) };
    checksum_ok(table).then_some(table)
}

/// Locate the HPET and the ACPI PM timer (read only; logs what it finds).
#[cfg(feature = "lapic-timer")]
pub fn timer_hardware(rsdp_phys: u64) -> TimerHardware {
    let mut hw = TimerHardware::default();
    if rsdp_phys == 0 {
        crate::serial_println!("[ kernel ] acpi: no RSDP from bootloader - no HPET / PM timer");
        return hw;
    }
    let Some((root_phys, is_xsdt)) = locate_root_table(rsdp_phys) else {
        return hw;
    };
    hw.hpet_phys = read_table(root_phys, is_xsdt, SIG_HPET).and_then(timers_logic::hpet_base);
    hw.pm_timer = read_table(root_phys, is_xsdt, SIG_FADT).and_then(timers_logic::fadt_pm_timer);
    crate::serial_println!(
        "[ kernel ] acpi: clocks hpet={:x?} pm_timer={:x?}",
        hw.hpet_phys,
        hw.pm_timer
    );
    hw
}

/// i8042 lane: the MADT's I/O APICs and ISA overrides (`None` without one).
#[cfg(any(feature = "ps2-keyboard", feature = "i2c-hid-touchpad"))]
pub fn madt(rsdp_phys: u64) -> Option<madt_logic::Madt> {
    if rsdp_phys == 0 {
        return None;
    }
    let (root_phys, is_xsdt) = locate_root_table(rsdp_phys)?;
    madt_logic::parse(read_table(root_phys, is_xsdt, SIG_MADT)?)
}

/// i8042 lane: the FADT's `IAPC_BOOT_ARCH` flags (u16 at offset 109, ACPI
/// 2.0+). Logged only: bit 1 ("8042 present") is often wrong on laptops, so
/// the driver probes the ports whatever it says.
#[cfg(feature = "ps2-keyboard")]
pub fn fadt_boot_arch(rsdp_phys: u64) -> Option<u16> {
    if rsdp_phys == 0 {
        return None;
    }
    let (root_phys, is_xsdt) = locate_root_table(rsdp_phys)?;
    let fadt = read_table(root_phys, is_xsdt, SIG_FADT)?;
    Some(u16::from_le_bytes(fadt.get(109..111)?.try_into().ok()?))
}

/// I2C-HID touchpad lane: the whole DSDT, through the FADT's `X_DSDT`
/// (u64 at 140, ACPI 2.0+) or, when that is zero, `DSDT` (u32 at 40).
/// Checksum-valid, signature `DSDT`, at most 4 MiB; `None` otherwise.
#[cfg(any(feature = "i2c-hid-touchpad", feature = "power-control"))]
pub fn dsdt(rsdp_phys: u64) -> Option<&'static [u8]> {
    if rsdp_phys == 0 {
        return None;
    }
    let (root_phys, is_xsdt) = locate_root_table(rsdp_phys)?;
    let fadt = read_table(root_phys, is_xsdt, SIG_FADT)?;
    let x_dsdt = if fadt.len() >= 148 {
        rd_u64(fadt, 140)?
    } else {
        0
    };
    let phys = if x_dsdt != 0 {
        x_dsdt
    } else {
        rd_u32(fadt, 40)? as u64
    };
    if phys == 0 {
        return None;
    }
    // SAFETY: the table the FADT points at (mapped page by page).
    let header = unsafe { phys_slice(phys, 36) };
    if &header[0..4] != b"DSDT" {
        return None;
    }
    let length = rd_u32(header, 4)? as usize;
    if !(36..=4 << 20).contains(&length) {
        return None;
    }
    // SAFETY: as above, for the length the header declares.
    let table = unsafe { phys_slice(phys, length) };
    checksum_ok(table).then_some(table)
}

/// OT-85 (`rtc-epoch`): the FADT's CMOS-clock fields (century index, "CMOS
/// RTC Not Present") for the boot-time RTC epoch seed.
#[cfg(feature = "rtc-epoch")]
pub mod rtc_fadt;
#[cfg(feature = "rtc-epoch")]
#[path = "rtc_fadt_logic.rs"]
pub mod rtc_fadt_logic;

/// OT-90 (`power-control`): power-off and restart. P1's pure logic, and the
/// P2 mechanism that runs it.
#[cfg(feature = "power-control")]
pub mod power;
#[cfg(feature = "power-control")]
#[path = "power_logic.rs"]
pub mod power_logic;
#[cfg(feature = "power-clean")]
pub mod shutdown;

/// OT-90: the definition blocks the static `\_S5` decode reads, in the order it
/// expects: the DSDT, then every XSDT-listed SSDT in root-table order. An SSDT
/// that isn't checksum-valid, or is longer than 4 MiB, is left out and logged.
#[cfg(feature = "power-control")]
pub fn definition_blocks(rsdp_phys: u64) -> Option<alloc::vec::Vec<&'static [u8]>> {
    if rsdp_phys == 0 {
        return None;
    }
    let (root_phys, is_xsdt) = locate_root_table(rsdp_phys)?;
    let mut blocks = alloc::vec![dsdt(rsdp_phys)?];
    // SAFETY: the root table, validated by `locate_root_table`'s caller path
    // (signature and checksum are re-checked by `find_table`; re-read here).
    let header = unsafe { phys_slice(root_phys, 36) };
    let length = rd_u32(header, 4)? as usize;
    if !(36..=1 << 20).contains(&length) {
        return None;
    }
    // SAFETY: as above, for the length the header declares.
    let root = unsafe { phys_slice(root_phys, length) };
    if !checksum_ok(root) {
        return None;
    }
    let entry_size = if is_xsdt { 8 } else { 4 };
    for i in 0..(length - 36) / entry_size {
        let off = 36 + i * entry_size;
        let phys = if is_xsdt {
            rd_u64(root, off)?
        } else {
            rd_u32(root, off)? as u64
        };
        if phys == 0 {
            continue;
        }
        // SAFETY: a table the root table points at (mapped page by page).
        let sub = unsafe { phys_slice(phys, 36) };
        if &sub[0..4] != b"SSDT" {
            continue;
        }
        let len = rd_u32(sub, 4)? as usize;
        if !(36..=4 << 20).contains(&len) {
            crate::serial_println!(
                "[ kernel ] acpi: SSDT at {:#x} length {} skipped",
                phys,
                len
            );
            continue;
        }
        // SAFETY: as above, for the length the header declares.
        let table = unsafe { phys_slice(phys, len) };
        if !checksum_ok(table) {
            crate::serial_println!("[ kernel ] acpi: SSDT at {:#x} checksum bad, skipped", phys);
            continue;
        }
        blocks.push(table);
    }
    Some(blocks)
}

/// L7.1: the firmware's boot timestamps, FPDT → FBPT → its boot record
/// (`boot_time_logic`). The FBPT is firmware memory the HHDM may not map;
/// `phys_slice` maps what it touches. Its length is checked (at most
/// `FBPT_MAX_LEN`) before the whole of it is read.
#[cfg(feature = "boot-time")]
pub fn firmware_boot_times(
    rsdp_phys: u64,
) -> Result<crate::boot_time_logic::FirmwareBoot, crate::boot_time_logic::FpdtError> {
    use crate::boot_time_logic::{FBPT_HEADER_LEN, FpdtError, fbpt_address, fbpt_len, parse_fbpt};
    if rsdp_phys == 0 {
        return Err(FpdtError::NoFpdt);
    }
    // Every range below is read only when it lies wholly inside one ACPI
    // reclaim or ACPI NVS region of the raw UEFI memory map, the only types
    // that say the OS may read them (`boot_time_logic::efi_is_firmware_ram`):
    // a damaged pointer into a hole, MMIO, reserved memory or the kernel's RAM
    // is refused before any byte of it is touched. No UEFI map (a BIOS boot),
    // or tables kept in reserved memory: the firmware time is unavailable.
    let regions = crate::boot_time::firmware_regions();
    let readable = |phys: u64, len: usize| {
        crate::boot_time_logic::in_firmware_memory(phys, len as u64, regions)
    };
    let (root_phys, is_xsdt) = locate_root_table(rsdp_phys).ok_or(FpdtError::NoFpdt)?;
    let fpdt = read_table_in(root_phys, is_xsdt, b"FPDT", &readable).ok_or(FpdtError::NoFpdt)?;
    // CPUID 0x8000_0008 EAX[7:0]: the physical address width.
    let phys_bits = core::arch::x86_64::__cpuid(0x8000_0008).eax & 0xff;
    // The FBPT's whole largest extent is inside the CPU's physical width.
    let fbpt = fbpt_address(fpdt, phys_bits)?;
    if !readable(fbpt, FBPT_HEADER_LEN) {
        return Err(FpdtError::NotFirmwareMemory);
    }
    // SAFETY: an address the firmware named, inside the CPU's physical
    // width and inside firmware-owned memory; only read.
    let len = fbpt_len(unsafe { phys_slice(fbpt, FBPT_HEADER_LEN) })?;
    if !readable(fbpt, len) {
        return Err(FpdtError::NotFirmwareMemory);
    }
    // SAFETY: as above, for the length the header declares (at most
    // `FBPT_MAX_LEN`), checked against the same region.
    parse_fbpt(unsafe { phys_slice(fbpt, len) })
}

/// OT-90: the FADT, whole and checksum-valid.
#[cfg(feature = "power-control")]
pub fn fadt(rsdp_phys: u64) -> Option<&'static [u8]> {
    if rsdp_phys == 0 {
        return None;
    }
    let (root_phys, is_xsdt) = locate_root_table(rsdp_phys)?;
    read_table(root_phys, is_xsdt, SIG_FADT)
}
