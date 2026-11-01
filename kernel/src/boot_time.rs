// SPDX-License-Identifier: MPL-2.0
//! L7.1 (`boot-time`): the boot-time numbers. The kernel stamps the TSC at
//! its entry and when it hands the screen to the desktop, reads the
//! firmware's boot timestamps from the FPDT, prints one line as the boot
//! console's last, and serves all of it through `SYS_BOOT_TIMES`, from which
//! the sign-in screen and Arthur show their own times. Pure parts:
//! `boot_time_logic`. Feature off: none of this is compiled, and
//! `SYS_BOOT_TIMES` is ENOSYS.

use core::sync::atomic::{AtomicU64, Ordering};

use spin::Once;

use crate::boot_time_logic::{self, BootTimes, FirmwareBoot, FpdtError};

static TSC_KERNEL_ENTRY: AtomicU64 = AtomicU64::new(0);
static TSC_DESKTOP_START: AtomicU64 = AtomicU64::new(0);
static TSC_MODEL_READY: AtomicU64 = AtomicU64::new(0);
static FIRMWARE: Once<Result<FirmwareBoot, FpdtError>> = Once::new();

/// The firmware-RAM regions of the UEFI memory map, captured once right
/// after `mm::init` (the map lives in bootloader-reclaimable memory, which
/// may be reused later). Empty on a BIOS boot (no UEFI map): every firmware
/// pointer is then unavailable, never read.
const FIRMWARE_REGIONS_MAX: usize = 128;
static FIRMWARE_REGIONS: Once<([(u64, u64); FIRMWARE_REGIONS_MAX], usize)> = Once::new();

/// Capture [`FIRMWARE_REGIONS`] from Limine's EFI memory map response.
/// Base revision 3: the response's map pointer is a physical address (the
/// crate's `memmap()` would treat it as virtual), so it is read raw and
/// reached through the HHDM, only if that page is mapped.
pub fn capture_firmware_regions() {
    FIRMWARE_REGIONS.call_once(|| {
        let mut out = [(0u64, 0u64); FIRMWARE_REGIONS_MAX];
        let Some(resp) = crate::EFI_MEMMAP_REQUEST.response() else {
            return (out, 0);
        };
        // SAFETY: `EfiMemmapRespData` is `repr(C)`: map pointer, map size,
        // descriptor size, descriptor version, each 8 bytes.
        let raw = resp as *const _ as *const u64;
        let (phys, size, desc_size) = unsafe { (*raw, *raw.add(1), *raw.add(2)) };
        if phys == 0 || size == 0 || size > 1 << 20 {
            return (out, 0);
        }
        let start = crate::mm::phys_to_virt(x86_64::PhysAddr::new(phys));
        let end = crate::mm::phys_to_virt(x86_64::PhysAddr::new(phys + size - 1));
        if !crate::mm::kernel_virt_mapped(start) || !crate::mm::kernel_virt_mapped(end) {
            return (out, 0);
        }
        // SAFETY: the bootloader's map, mapped through the HHDM (checked
        // above), read once before anything can reuse its memory.
        let map = unsafe { core::slice::from_raw_parts(start.as_ptr::<u8>(), size as usize) };
        let n = boot_time_logic::efi_firmware_regions(map, desc_size as usize, &mut out);
        (out, n)
    });
}

/// The captured firmware-RAM regions (empty before the capture or on BIOS).
pub fn firmware_regions() -> &'static [(u64, u64)] {
    match FIRMWARE_REGIONS.get() {
        Some((regions, n)) => &regions[..*n],
        None => &[],
    }
}

fn rdtsc() -> u64 {
    // SAFETY: RDTSC has no side effects.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// The kernel's first stamp, right after the serial port is up.
pub fn stamp_entry() {
    TSC_KERNEL_ENTRY.store(rdtsc(), Ordering::Relaxed);
}

/// The boot model was built (`boot-model-preload`), before userland.
#[allow(dead_code)] // a `boot-time` kernel without `boot-model-preload`
pub fn stamp_model_ready() {
    TSC_MODEL_READY.store(rdtsc(), Ordering::Relaxed);
}

/// The firmware timestamps, read from the FPDT once (later calls reuse it).
pub fn read_firmware(rsdp_phys: u64) -> Result<FirmwareBoot, FpdtError> {
    *FIRMWARE.call_once(|| crate::acpi::firmware_boot_times(rsdp_phys))
}

/// Stamp the desktop start and print the boot console's last line.
pub fn stamp_desktop(rsdp_phys: u64) {
    TSC_DESKTOP_START.store(rdtsc(), Ordering::Relaxed);
    let firmware = read_firmware(rsdp_phys);
    log_line(&snapshot(), firmware.err());
}

/// One `[ boot ] time:` line on serial and the fb console.
pub fn log_line(times: &BootTimes, firmware_missing: Option<FpdtError>) {
    let mut line = alloc::string::String::new();
    let _ = boot_time_logic::write_kernel_line(&mut line, times, firmware_missing);
    crate::serial_println!("[ boot ] time: {} rate={}", line, tsc_rate().1);
}

/// The TSC rate and where it came from: the LAPIC timer's calibration
/// (`calibrated`), else CPUID leaf 0x15 (`cpuid-15`), else leaf 0x16's base
/// frequency (`cpuid-16-estimate`, a specification value Intel says need
/// not match the TSC), else none. The calibration declines a rate its two
/// reference clocks dispute (seen under QEMU TCG).
fn tsc_rate() -> (u64, &'static str) {
    use crate::arch::x86_64::lapic_timer_logic::{tsc_hz_from_leaf15, tsc_hz_from_leaf16};
    let calibrated = crate::arch::x86_64::lapic_timer::tsc_hz();
    if calibrated != 0 {
        return (calibrated, "calibrated");
    }
    let max_leaf = core::arch::x86_64::__cpuid(0).eax;
    if max_leaf >= 0x15 {
        let l = core::arch::x86_64::__cpuid(0x15);
        if let Some(hz) = tsc_hz_from_leaf15(l.eax, l.ebx, l.ecx) {
            return (hz, "cpuid-15");
        }
    }
    if max_leaf >= 0x16
        && let Some(hz) = tsc_hz_from_leaf16(core::arch::x86_64::__cpuid(0x16).eax)
    {
        return (hz, "cpuid-16-estimate");
    }
    (0, "none")
}

/// What `SYS_BOOT_TIMES` reports now.
pub fn snapshot() -> BootTimes {
    let (tsc_hz, source) = tsc_rate();
    BootTimes {
        tsc_hz,
        rate_estimated: source == "cpuid-16-estimate",
        tsc_kernel_entry: TSC_KERNEL_ENTRY.load(Ordering::Relaxed),
        tsc_desktop_start: TSC_DESKTOP_START.load(Ordering::Relaxed),
        tsc_model_ready: TSC_MODEL_READY.load(Ordering::Relaxed),
        firmware: FIRMWARE.get().and_then(|r| r.ok()),
    }
}
