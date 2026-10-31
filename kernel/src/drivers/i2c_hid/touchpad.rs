// SPDX-License-Identifier: MPL-2.0
//! The Zenbook S14 touchpad driver (`i2c-hid-touchpad`), rung 1: the
//! touchpad's mouse collection, through HID over I2C on Serial-IO I2C5.
//!
//! **Match first.** [`init`] reads the DSDT and walks the quirk table
//! (`quirk_logic`). Only a machine whose firmware describes this exact
//! touchpad (and whose PCI 00:19.1 is `8086:E351`) goes further. Anything
//! else prints one `i2c-hid: no match (...)` line and touches nothing: the
//! PCI function is not even read unless the DSDT conditions hold.
//!
//! **Bring-up** (boot, BSP, once; every wait bounded by the TSC):
//! 1. PCI: D0 through PMCSR (10 ms after a D3hot exit), memory decode on,
//!    bus mastering **never** granted, BAR0 mapped (validated against its
//!    size).
//! 2. LPSS: the resets register written 0 then function-out-of-reset; the
//!    iDMA bit stays 0, so the DMA engine is held in reset (I-14(c)),
//!    checked on the read-back and refused otherwise.
//! 3. DesignWare: identify (`IC_COMP_TYPE`), FIFO depths, fast mode at the
//!    100 MHz input clock, all controller interrupts masked (polled).
//! 4. HID descriptor (30 bytes from register 0x0001), validated; the report
//!    descriptor, parsed for the mouse collection.
//! 5. The GSI (0x69, level, active-low) routed MASKED to [`VECTOR`]; stale
//!    input drained while masked; SET_POWER ON, 60 ms; RESET with the line
//!    unmasked, then up to 1 s for the device's interrupt and its zero-length
//!    reset-complete read; SET_POWER ON again (not for ELAN, trap 7).
//!
//! **Runtime.** The ISR only signals: it masks the GSI, sets `PENDING` and
//! wakes the parked service thread (`WAITER`, held only for the take; no
//! governance lock, I-14(a)). The service thread (`i2c-hid`, a kernel thread)
//! parks while nothing is pending (CP-81: without `wait-multiplex` or
//! `tick-sleep` a nap is a yield loop), reads one input report
//! per assertion, decodes it, pushes a mouse event
//! into a lock-free single-producer ring, and unmasks. A level line still
//! asserted re-fires at once. After [`hid::STORM_LIMIT`] assertions in a row that
//! held no data, the line stays masked and the thread polls instead
//! (`mode=polled-storm`). [`pump_routed_events`] (syscall context, beside
//! the USB mouse's) moves ring events into the focus stage as the USB
//! mouse's pointer events. No I2C transfer runs in the ISR or under a lock
//! the ISR could want.
//!
//! **Polled fallback.** When the route fails, or the reset completed without
//! the interrupt ever arriving, the thread reads the input register every
//! tick instead (`mode=polled(...)`), and the console says so.

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};

use spin::Once;

use super::dw_logic::{self as dw, Regs};
use super::hid_logic::{
    self as hid, Action, Decoded, HidDescriptor, Input, LineGovernor, MouseReport, Outcome,
    ReportMap,
};
use super::pci_pm_logic::{self as pm, BarDecision, Bars};
use super::quirk_logic::{self as quirk, Quirk, Verdict};
use crate::arch::x86_64::ioapic::GsiPin;

/// The IDT vector the touchpad's GSI is routed to.
pub const VECTOR: u8 = 0xF6;

/// TSC cycles per millisecond on any TSC up to 10 GHz: bring-up runs before
/// the tick is calibrated, so every wait is sized for the fastest plausible
/// TSC and lasts longer, never shorter, on a slower one (the xHCI silicon
/// lane's idiom).
const TSC_PER_MS: u64 = 10_000_000;
const BUDGET: dw::Budget = dw::Budget::from_units_per_ms(TSC_PER_MS);

/// Reports one service pass handles before it naps.
const PASS_MAX: u32 = 64;
/// Stale input drained before the reset, at most.
const STALE_MAX: u32 = 8;

const RING_LEN: usize = 64;
static RING: [AtomicU32; RING_LEN] = [const { AtomicU32::new(0) }; RING_LEN];
/// Producer index (the service thread only).
static HEAD: AtomicU32 = AtomicU32::new(0);
/// Consumer index (under the pump's `STATE` lock).
static TAIL: AtomicU32 = AtomicU32::new(0);

/// The match passed and the controller path was entered.
static PROBED: AtomicBool = AtomicBool::new(false);
/// Bring-up finished; the service thread may run.
static READY: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);
static MODE: AtomicU8 = AtomicU8::new(MODE_NONE);
const MODE_NONE: u8 = 0;
const MODE_IRQ: u8 = 1;
const MODE_POLLED_NO_ROUTE: u8 = 2;
const MODE_POLLED_NO_IRQ: u8 = 3;
const MODE_POLLED_STORM: u8 = 4;
const MODE_FAILED: u8 = 5;

/// Taken (never waited for) around every transfer: only boot, then the
/// service thread, ever transfer, so it is never contended. Checked anyway.
static BUS_OWNER: AtomicBool = AtomicBool::new(false);

static IRQS: AtomicU64 = AtomicU64::new(0);
static READS: AtomicU64 = AtomicU64::new(0);
static REPORTS: AtomicU64 = AtomicU64::new(0);
static MOUSE: AtomicU64 = AtomicU64::new(0);
static UNEXPECTED: AtomicU64 = AtomicU64::new(0);
static UNKNOWN: AtomicU64 = AtomicU64::new(0);
static SHORT: AtomicU64 = AtomicU64::new(0);
static EMPTY: AtomicU64 = AtomicU64::new(0);
static BOGUS: AtomicU64 = AtomicU64::new(0);
static BAD_LENGTH: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static DELIVERED: AtomicU64 = AtomicU64::new(0);
static LAST_ID: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_ERROR: AtomicU32 = AtomicU32::new(0);
/// The watchdog's touchpad line (A5: the pointer moves, clicks do not land):
/// button-down transitions, the last decoded buttons byte, the first four
/// bytes of the last report (ID included), and the parsed button layout
/// (count << 16 | size << 8 | bit).
#[cfg(feature = "freeze-watchdog")]
static BTN_DOWNS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "freeze-watchdog")]
static LAST_BUTTONS: AtomicU8 = AtomicU8::new(0);
#[cfg(feature = "freeze-watchdog")]
static LAST_RAW: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "freeze-watchdog")]
static BUTTON_LAYOUT: AtomicU32 = AtomicU32::new(0);

/// The routed GSI: set once at bring-up, read lock-free by the ISR.
static PIN: Once<GsiPin> = Once::new();
/// What the service thread drives: set once at bring-up.
static DEVICE: Once<Device> = Once::new();

/// The bound touchpad (immutable after bring-up).
struct Device {
    base: usize,
    ctl: dw::Controller,
    address: u8,
    desc: HidDescriptor,
    map: ReportMap,
}

/// The service thread's wake token while it is parked for the interrupt
/// (IRQ mode): the ISR takes it and wakes the thread. `PENDING` is re-checked
/// under this lock before parking, so a wake is never lost (CP-8/CP-14).
/// Lock order: `WAITER` -> scheduler; the ISR drops `WAITER` before waking.
static WAITER: crate::sync::IrqSafeMutex<Option<crate::sched::ThreadWakeToken>> =
    crate::sync::IrqSafeMutex::new(None);
/// Service-loop passes (the watchdog's `loops=`: still while idle in IRQ
/// mode, since the thread is parked).
#[cfg(feature = "freeze-watchdog")]
static LOOPS: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "atrium-input-routing")]
static STATE: crate::sync::IrqSafeMutex<Option<crate::atrium::focus::Stage>> =
    crate::sync::IrqSafeMutex::new(None);

/// The DesignWare/LPSS registers through BAR0, timed by the TSC.
struct Mmio {
    base: usize,
}

impl Regs for Mmio {
    fn read(&mut self, offset: usize) -> u32 {
        // SAFETY: `base` is BAR0, mapped for `dw::BAR0_MAP_LEN` bytes, and
        // every offset the logic uses is below it.
        unsafe { core::ptr::read_volatile((self.base + offset) as *const u32) }
    }

    fn write(&mut self, offset: usize, value: u32) {
        // SAFETY: as for `read`.
        unsafe { core::ptr::write_volatile((self.base + offset) as *mut u32, value) }
    }

    fn now(&mut self) -> u64 {
        rdtsc()
    }
}

fn rdtsc() -> u64 {
    // SAFETY: RDTSC has no side effect.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Wait at least `ms` (longer on a TSC slower than 10 GHz).
fn spin_ms(ms: u64) {
    let start = rdtsc();
    while rdtsc().wrapping_sub(start) < ms * TSC_PER_MS {
        core::hint::spin_loop();
    }
}

/// One message to the touchpad, under `BUS_OWNER`. Never from the ISR.
fn transfer(dev: &Device, write: &[u8], read: &mut [u8]) -> Result<(), dw::Error> {
    if BUS_OWNER
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        // Two transferring contexts would be a driver bug.
        return Err(dw::Error::Timeout(dw::Stage::Transfer));
    }
    let mut regs = Mmio { base: dev.base };
    let r = dw::transfer(&mut regs, &dev.ctl, &BUDGET, dev.address, write, read);
    BUS_OWNER.store(false, Ordering::Release);
    r
}

/// A transfer retried once after 1 ms when the address is not acknowledged:
/// a device waking from deep sleep may NAK the first request (trap 6).
fn transfer_waking(dev: &Device, write: &[u8], read: &mut [u8]) -> Result<(), dw::Error> {
    match transfer(dev, write, read) {
        Err(dw::Error::Abort(source)) if source & dw::ABRT_7B_ADDR_NOACK != 0 => {
            spin_ms(1);
            transfer(dev, write, read)
        }
        other => other,
    }
}

/// The error as the console prints it.
struct ErrorText(dw::Error);

impl core::fmt::Display for ErrorText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            dw::Error::AllOnes => f.write_str("all-ones (D3/reset/not decoding)"),
            dw::Error::NotDesignWare(t) => write!(f, "comp-type={:#010x} (not DesignWare)", t),
            dw::Error::Timeout(stage) => write!(f, "timeout ({:?})", stage),
            dw::Error::Abort(source) => {
                write!(f, "abort={:#010x} ({})", source, dw::abort_reason(source))
            }
            dw::Error::TooLong => f.write_str("too-long"),
        }
    }
}

fn error_code(e: dw::Error) -> u32 {
    match e {
        dw::Error::AllOnes => 1,
        dw::Error::NotDesignWare(_) => 2,
        dw::Error::Timeout(_) => 3,
        dw::Error::Abort(source) => 0x8000_0000 | (source & 0x7FFF_FFFF),
        dw::Error::TooLong => 4,
    }
}

/// Every bring-up line, kept so image A's touchpad check can print them again
/// as the last block on the boot console (they scroll off above the USB
/// summary otherwise). Boot and that check only; never the ISR.
static BOOT_LINES: spin::Mutex<alloc::vec::Vec<alloc::string::String>> =
    spin::Mutex::new(alloc::vec::Vec::new());

/// A bring-up line: printed, and kept for the touchpad check.
macro_rules! say {
    ($($arg:tt)*) => {{
        let line = alloc::format!($($arg)*);
        crate::serial_println!("{}", line);
        BOOT_LINES.lock().push(line);
    }};
}

/// The verdict the boot run reached (test diagnostic).
static VERDICT: Once<Verdict> = Once::new();

/// Match, then bring up. Boot-time, BSP, after the USB stack. Returns
/// whether the touchpad is live.
pub fn init(rsdp_phys: u64) -> bool {
    let Some(dsdt) = crate::acpi::dsdt(rsdp_phys) else {
        VERDICT.call_once(|| Verdict::NoDsdt);
        say!("i2c-hid: no match (no dsdt) - touchpad not present");
        crate::log::flush();
        return false;
    };
    let (selected, finding) = quirk::select(dsdt);
    let quirk = selected.unwrap_or(quirk::QUIRKS[0]);
    // The PCI function is read only once the DSDT conditions hold.
    let forced = cfg!(feature = "neo-test-i2c-hid-absent-control");
    let pci = if selected.is_some() || forced {
        crate::drivers::pci::device_at(quirk.pci_bus, quirk.pci_dev, quirk.pci_function)
    } else {
        None
    };
    let mut verdict = quirk::verdict(&finding, quirk, pci.map(|d| (d.vendor_id, d.device_id)));
    // The control forces the match on any machine: the bring-up must then
    // fail closed (no controller, bounded waits, no hang).
    if forced && !verdict.is_match() {
        say!("i2c-hid: control: match FORCED (verdict was {:?})", verdict);
        verdict = Verdict::Match;
    }
    VERDICT.call_once(|| verdict);
    let oem = quirk::printable(&finding.oem_id);
    let table = quirk::printable(&finding.oem_table_id);
    let oem = core::str::from_utf8(&oem).unwrap_or("-");
    let table = core::str::from_utf8(&table).unwrap_or("-");
    if !verdict.is_match() {
        say!(
            "i2c-hid: no match (oem={}/{} dev={}) reason={} - touchpad not present",
            oem,
            table,
            PciText(pci),
            VerdictText(verdict)
        );
        crate::log::flush();
        return false;
    }
    PROBED.store(true, Ordering::Release);
    say!(
        "i2c-hid: quirk={} match (oem={}/{} crs={:#x} hiddesc-reg={:#06x} dev={})",
        quirk.name,
        oem,
        table,
        finding.template_at.unwrap_or(0),
        finding.hid_desc_register.unwrap_or(quirk.hid_desc_register),
        PciText(pci)
    );
    crate::log::flush();
    let live = match pci {
        Some(pci) => bring_up(quirk, pci, rsdp_phys),
        None => {
            say!(
                "i2c-hid: pci {:04x}:{:04x} absent at {:02x}:{:02x}.{} - touchpad not present",
                quirk.pci_vendor,
                quirk.pci_device,
                quirk.pci_bus,
                quirk.pci_dev,
                quirk.pci_function
            );
            false
        }
    };
    if !live {
        MODE.store(MODE_FAILED, Ordering::Release);
    }
    crate::log::flush();
    live
}

struct PciText(Option<crate::drivers::pci::Device>);

impl core::fmt::Display for PciText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Some(d) => write!(f, "{:04x}:{:04x}", d.vendor_id, d.device_id),
            None => f.write_str("not-read-or-absent"),
        }
    }
}

struct VerdictText(Verdict);

impl core::fmt::Display for VerdictText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Verdict::NoDsdt => f.write_str("no-dsdt"),
            Verdict::OemMismatch { .. } => f.write_str("oem"),
            Verdict::NoTemplate => f.write_str("no-touchpad-crs"),
            Verdict::TemplateOutsideDevice => f.write_str("crs-outside-device"),
            Verdict::DescriptorRegister(Some(r)) => write!(f, "hiddesc-reg={:#06x}", r),
            Verdict::DescriptorRegister(None) => f.write_str("hiddesc-reg-unreadable"),
            Verdict::PciAbsent => f.write_str("pci-absent"),
            Verdict::PciMismatch { vendor, device } => {
                write!(f, "pci-is-{:04x}:{:04x}", vendor, device)
            }
            Verdict::Match => f.write_str("match"),
        }
    }
}

fn bring_up(quirk: &'static Quirk, pci: crate::drivers::pci::Device, rsdp_phys: u64) -> bool {
    use crate::drivers::pci;

    // 1. PCI power and decode (no bus master: PIO only). The raw BAR dwords,
    // the command register and PMCSR are captured before PMCSR is touched:
    // with No_Soft_Reset clear, D3hot -> D0 resets the function's config
    // (`pci_pm_logic`), and the saved BAR is written back.
    let read_bars = || Bars {
        bar0: pci::config_read32(pci, 0x10),
        bar1: pci::config_read32(pci, 0x14),
    };
    let bars_before = read_bars();
    let command_before = pci::command_register_at(pci.bus, pci.device, pci.function);
    let pmcsr_before = pci::power_management_csr(pci);
    say!(
        "i2c-hid: cfg before bar0={:#010x} bar1={:#010x} cmd={:#06x} pmcsr={} nsr={}",
        bars_before.bar0,
        bars_before.bar1,
        command_before,
        PmRaw(pmcsr_before),
        NsrText(pmcsr_before)
    );
    crate::log::flush();
    if let Some(pm) = pmcsr_before
        && pm & 0x3 != 0
    {
        pci::write_power_management_csr(pci, pm & !0x3);
        // PCI PM 1.2 §5.6.1: 10 ms from D3hot to D0 before any access.
        spin_ms(10);
    }
    let pmcsr_after = pci::power_management_csr(pci);
    let bars_after = read_bars();
    say!(
        "i2c-hid: cfg after bar0={:#010x} bar1={:#010x} cmd={:#06x} pmcsr={} nsr={}",
        bars_after.bar0,
        bars_after.bar1,
        pci::command_register_at(pci.bus, pci.device, pci.function),
        PmRaw(pmcsr_after),
        NsrText(pmcsr_after)
    );
    crate::log::flush();
    match pm::decide(bars_before, bars_after) {
        BarDecision::Ok => {}
        BarDecision::Restore => {
            // Decode off while the BAR is written (a soft reset leaves it
            // off; make sure), then low dword before high.
            pci::config_write16(pci, 0x04, command_before & !0x3);
            pci::config_write32(pci, 0x10, bars_before.bar0);
            if bars_before.is_64bit() {
                pci::config_write32(pci, 0x14, bars_before.bar1);
            }
            say!(
                "i2c-hid: bar restored after D3->D0 reset bar0={:#010x} bar1={:#010x}",
                pci::config_read32(pci, 0x10),
                pci::config_read32(pci, 0x14)
            );
            crate::log::flush();
        }
        BarDecision::Unassigned => {
            // `i2c-hid-bar-assign` off: stop here, as before.
            #[cfg(not(feature = "i2c-hid-bar-assign"))]
            {
                say!("i2c-hid: bar0 unassigned by firmware - stopping");
                return false;
            }
            #[cfg(feature = "i2c-hid-bar-assign")]
            {
                say!("i2c-hid: bar0 unassigned by firmware - assigning");
                crate::log::flush();
                if !assign_bar0(pci, bars_before, command_before) {
                    return false;
                }
            }
        }
        BarDecision::NotMemory => {
            say!("i2c-hid: bar0 is an I/O BAR - stopping");
            return false;
        }
        BarDecision::Changed => {
            say!("i2c-hid: bar0 changed across D3->D0 - stopping");
            return false;
        }
    }
    pci::enable_memory_decode(pci);
    // PIO only: whatever firmware left, the function may not master the bus.
    pci::disable_bus_mastering(pci);
    let command = pci::command_register_at(pci.bus, pci.device, pci.function);
    let bar0 = match pci::bar(pci, 0) {
        Some(pci::PciBar::Memory32 { base, .. } | pci::PciBar::Memory64 { base, .. }) => base,
        _ => 0,
    };
    say!(
        "i2c-hid: pci {:02x}:{:02x}.{} {:04x}:{:04x} pmcsr={}->{} cmd={:#06x} bar0={:#x}",
        pci.bus,
        pci.device,
        pci.function,
        pci.vendor_id,
        pci.device_id,
        PmText(pmcsr_before),
        PmText(pmcsr_after),
        command,
        bar0
    );
    crate::log::flush();
    if bar0 == 0 {
        say!("i2c-hid: no usable BAR0 - stopping");
        return false;
    }
    if command & pci::COMMAND_BUS_MASTER != 0 {
        say!("i2c-hid: bus mastering would not clear - refusing (I-14(c))");
        return false;
    }
    let Some(virt) = pci::map_bar_region_validated(pci, 0, 0, dw::BAR0_MAP_LEN) else {
        say!(
            "i2c-hid: BAR0 smaller than {:#x} - stopping",
            dw::BAR0_MAP_LEN
        );
        return false;
    };
    let mut regs = Mmio {
        base: virt.as_u64() as usize,
    };

    // 2. LPSS out of reset, iDMA held in reset (I-14(c)).
    let resets_before = regs.read(dw::LPSS_RESETS);
    let caps = regs.read(dw::LPSS_CAPS);
    let clock = regs.read(dw::LPSS_CLOCK_PARAMS);
    if resets_before == u32::MAX {
        say!("i2c-hid: lpss bar0=0xffffffff (reads all-ones: D3 or not decoding) - stopping");
        return false;
    }
    regs.write(dw::LPSS_RESETS, 0);
    regs.write(dw::LPSS_RESETS, dw::lpss_resets_value());
    let resets = regs.read(dw::LPSS_RESETS);
    let dma_in_reset = dw::lpss_dma_in_reset(resets);
    say!(
        "i2c-hid: lpss reset={:#010x}->{:#010x} dma={} caps={:#010x} clk={:#010x}",
        resets_before,
        resets,
        if dma_in_reset {
            "in-reset"
        } else {
            "OUT-OF-RESET"
        },
        caps,
        clock
    );
    crate::log::flush();
    if !dma_in_reset {
        // I-14(c): never run with the iDMA live.
        regs.write(dw::LPSS_RESETS, 0);
        say!("i2c-hid: lpss iDMA left reset - refusing (I-14(c))");
        return false;
    }

    // 3. The DesignWare master.
    let ctl = match dw::configure(&mut regs, dw::LPSS_I2C_CLOCK_HZ, &BUDGET) {
        Ok(ctl) => ctl,
        Err(e) => {
            say!("i2c-hid: dw configure FAILED: {}", ErrorText(e));
            return false;
        }
    };
    say!(
        "i2c-hid: dw ver={:#010x} fifo=tx{}/rx{} fs-hcnt={} fs-lcnt={} sda-hold={:#x} addr={:#04x} clk-hz={}",
        ctl.version,
        ctl.tx_depth,
        ctl.rx_depth,
        ctl.fast.hcnt,
        ctl.fast.lcnt,
        ctl.sda_hold,
        quirk.slave_address,
        dw::LPSS_I2C_CLOCK_HZ
    );
    crate::log::flush();
    let mut dev = Device {
        base: regs.base,
        ctl,
        address: quirk.slave_address,
        desc: HidDescriptor::default(),
        map: ReportMap::default(),
    };

    // 4. The HID descriptor, then the report descriptor.
    let mut raw = [0u8; hid::HID_DESC_LEN];
    if let Err(e) = transfer_waking(&dev, &hid::register(quirk.hid_desc_register), &mut raw) {
        say!("i2c-hid: hiddesc read FAILED: {}", ErrorText(e));
        return false;
    }
    let desc = HidDescriptor::decode(&raw);
    say!(
        "i2c-hid: hiddesc len={} ver={:#06x} vid={:#06x} pid={:#06x} rdesc-len={} rdesc-reg={:#06x} in-reg={:#06x} max-in={} cmd-reg={:#06x}",
        desc.length,
        desc.version,
        desc.vendor_id,
        desc.product_id,
        desc.report_desc_length,
        desc.report_desc_register,
        desc.input_register,
        desc.max_input_length,
        desc.command_register
    );
    crate::log::flush();
    if let Err(e) = desc.validate() {
        say!(
            "i2c-hid: hiddesc INVALID: {:?} (wrong descriptor register or device)",
            e
        );
        return false;
    }
    dev.desc = desc;
    let mut rdesc = alloc::vec![0u8; desc.report_desc_length as usize];
    if let Err(e) = transfer(&dev, &hid::register(desc.report_desc_register), &mut rdesc) {
        say!("i2c-hid: rdesc read FAILED: {}", ErrorText(e));
        return false;
    }
    match hid::parse_report_descriptor(&rdesc) {
        Ok(map) => {
            let m = map.mouse.unwrap_or_default();
            say!(
                "i2c-hid: rdesc bytes={} items={} ids={} mouse=id{} buttons={}x{}/bit{} x={}b/bit{} y={}b/bit{} wheel={} report-bits={}",
                rdesc.len(),
                map.items,
                IdsText(&map),
                m.report_id,
                m.button_count,
                m.buttons.size,
                m.buttons.bit,
                m.x.size,
                m.x.bit,
                m.y.size,
                m.y.bit,
                if m.wheel.is_some() { "yes" } else { "no" },
                m.report_bits
            );
            #[cfg(feature = "freeze-watchdog")]
            BUTTON_LAYOUT.store(
                (m.button_count.min(0xFF) << 16)
                    | (m.buttons.size.min(0xFF) << 8)
                    | m.buttons.bit.min(0xFF),
                Ordering::Relaxed,
            );
            dev.map = map;
        }
        Err(e) => {
            say!(
                "i2c-hid: rdesc bytes={} parse FAILED: {:?} first={:02x?}",
                rdesc.len(),
                e,
                &rdesc[..rdesc.len().min(12)]
            );
            return false;
        }
    }
    crate::log::flush();

    // 5. The interrupt (masked), power, reset.
    let route = match crate::acpi::madt(rsdp_phys) {
        Some(madt) => crate::arch::x86_64::ioapic::route_gsi(
            &madt,
            quirk.gsi,
            VECTOR,
            quirk.active_low,
            quirk.level,
        )
        .map_err(Some),
        None => Err(None),
    };
    if let Ok(pin) = route {
        PIN.call_once(|| pin);
    }
    let dev = DEVICE.call_once(|| dev);
    // Firmware may have left a report pending (the line then asserts as
    // soon as it is unmasked, and would pass for the reset's interrupt).
    let mut buf = [0u8; hid::MAX_INPUT];
    let mut stale = 0;
    for _ in 0..STALE_MAX {
        match read_input(dev, &mut buf) {
            Ok(Input::Report(_)) | Ok(Input::Bogus) => stale += 1,
            _ => break,
        }
    }
    let power = transfer_waking(
        dev,
        &hid::set_power_command(desc.command_register, true),
        &mut [],
    );
    spin_ms(60);
    PENDING.store(false, Ordering::Release);
    unmask();
    let reset = transfer(dev, &hid::reset_command(desc.command_register), &mut []);
    let ack = wait_reset_ack(dev, reset.is_ok(), &mut buf);
    if desc.vendor_id != hid::VENDOR_ELAN {
        let _ = transfer_waking(
            dev,
            &hid::set_power_command(desc.command_register, true),
            &mut [],
        );
        spin_ms(60);
    }
    let mode = match (route, ack) {
        (Err(_), _) => MODE_POLLED_NO_ROUTE,
        (Ok(_), ResetAck::Irq) => MODE_IRQ,
        (Ok(_), _) => MODE_POLLED_NO_IRQ,
    };
    MODE.store(mode, Ordering::Release);
    if mode != MODE_IRQ
        && let Some(pin) = PIN.get()
    {
        // Polled: the line stays masked (the ISR may already have masked it).
        crate::arch::x86_64::without_interrupts(|| pin.set_masked(true));
    }
    let rirr = PIN.get().map(|p| (p.read_low() >> 14) & 1);
    let eoi_suppressed = crate::arch::x86_64::apic::spurious_vector_register() >> 12 & 1;
    say!(
        "i2c-hid: power-on={} reset={} reset-ack={} stale={} irq={} vector={:#x} rirr={:?} eoi-suppress={} mode={}",
        if power.is_ok() { "ok" } else { "FAILED" },
        match reset {
            Ok(()) => "sent",
            Err(_) => "FAILED",
        },
        ack.text(),
        stale,
        RouteText(route),
        VECTOR,
        rirr,
        eoi_suppressed,
        mode_text(mode)
    );
    if let Err(e) = power.and(reset) {
        say!("i2c-hid: power/reset error: {}", ErrorText(e));
    }
    #[cfg(feature = "atrium-input-routing")]
    crate::arch::x86_64::without_interrupts(|| {
        *STATE.lock() = Some(crate::atrium::focus::Stage::with_capacity(
            crate::atrium::focus::INPUT_STAGE_CAPACITY,
        ));
    });
    READY.store(true, Ordering::Release);
    // In IRQ mode the line is masked by the reset's interrupt; the service
    // thread unmasks it once running. In polled mode it stays masked.
    true
}

/// Candidate slots tried before giving up.
#[cfg(feature = "i2c-hid-bar-assign")]
const BAR_CANDIDATES: usize = 4;
/// Ranges collected at most (functions' BARs, bridge windows, memmap).
#[cfg(feature = "i2c-hid-bar-assign")]
const OCCUPIED_MAX: usize = 1024;

/// Firmware left BAR0 unassigned: size it, choose a slot clear of every
/// range in use, read the slot with nothing claimed (it must read all-ones),
/// and program it (`pci_pm_logic`). Bus mastering stays off (I-14(c)); only
/// this function's own BAR is ever written. Returns whether BAR0 is assigned.
#[cfg(feature = "i2c-hid-bar-assign")]
fn assign_bar0(dev: crate::drivers::pci::Device, before: Bars, command: u16) -> bool {
    use crate::drivers::pci;

    // 1. Size, with decode and bus mastering off.
    pci::config_write16(dev, 0x04, command & !0x7);
    let is_64bit = before.is_64bit();
    pci::config_write32(dev, 0x10, u32::MAX);
    if is_64bit {
        pci::config_write32(dev, 0x14, u32::MAX);
    }
    let low = pci::config_read32(dev, 0x10);
    let high = if is_64bit {
        pci::config_read32(dev, 0x14)
    } else {
        0
    };
    pci::config_write32(dev, 0x10, before.bar0);
    if is_64bit {
        pci::config_write32(dev, 0x14, before.bar1);
    }
    let Some(size) = pm::bar_size_from_mask(low, high, is_64bit) else {
        say!(
            "i2c-hid: bar0 size probe read {:#010x}/{:#010x} - stopping",
            low,
            high
        );
        return false;
    };
    say!("i2c-hid: bar0 size={:#x} 64bit={}", size, is_64bit);
    if !is_64bit {
        // The 32-bit window below 4 GiB holds the fixed platform ranges;
        // assigning there is not this lane.
        say!("i2c-hid: bar0 is a 32-bit BAR - not assigned here - stopping");
        return false;
    }

    // 2. Everything already in use on segment 0.
    let mut occupied: alloc::vec::Vec<pm::Range> = alloc::vec::Vec::new();
    let (mut bars, mut windows, mut memmap, mut vmd) = (0u32, 0u32, 0u32, 0u32);
    // The lowest firmware 64-bit BAR at or above 4 GiB, and the end of their
    // span.
    let mut floor = u64::MAX;
    let mut span_end = 0u64;
    for f in pci::enumerate_all() {
        if f.bdf() == dev.bdf() {
            continue;
        }
        let bridge = f.header_type & 0x7F == 1;
        let bar_count = if bridge { 2 } else { 6 };
        let mut i = 0u8;
        while i < bar_count {
            let offset = 0x10 + i * 4;
            let low = pci::config_read32(f, offset);
            i += 1;
            if low == 0 || low == u32::MAX || low & 1 != 0 {
                continue;
            }
            let wide = (low >> 1) & 0x3 == 0x2;
            let mut base = (low & 0xFFFF_FFF0) as u64;
            if wide && i < bar_count {
                base |= (pci::config_read32(f, offset + 4) as u64) << 32;
                i += 1;
            }
            if base == 0 {
                continue;
            }
            let range = pm::assigned_bar_bound(base);
            occupied.push(range);
            bars += 1;
            if f.bus == 0 && f.device == 0x0E {
                vmd += 1;
            }
            if wide && base >= 1 << 32 {
                floor = floor.min(base);
                span_end = span_end.max(range.end);
            }
        }
        if bridge {
            // Memory window (0x20) and prefetchable window (0x24, upper
            // halves at 0x28/0x2C when it is 64-bit).
            let mem = pci::config_read32(f, 0x20);
            let (base, limit) = (
                ((mem & 0xFFF0) as u64) << 16,
                (((mem >> 16) & 0xFFF0) as u64) << 16 | 0xF_FFFF,
            );
            if base <= limit {
                occupied.push(pm::Range {
                    start: base,
                    end: limit + 1,
                });
                windows += 1;
            }
            let pref = pci::config_read32(f, 0x24);
            let mut base = ((pref & 0xFFF0) as u64) << 16;
            let mut limit = (((pref >> 16) & 0xFFF0) as u64) << 16 | 0xF_FFFF;
            if pref & 0xF == 1 {
                base |= (pci::config_read32(f, 0x28) as u64) << 32;
                limit |= (pci::config_read32(f, 0x2C) as u64) << 32;
            }
            if base <= limit && limit != 0xF_FFFF {
                occupied.push(pm::Range {
                    start: base,
                    end: limit.saturating_add(1),
                });
                windows += 1;
            }
        }
        if occupied.len() >= OCCUPIED_MAX {
            say!("i2c-hid: bar0 too many ranges in use - stopping");
            return false;
        }
    }
    // Every memory-map entry, whatever its type (RAM included).
    if let Some(response) = crate::MEMMAP_REQUEST.response() {
        for e in response.entries() {
            occupied.push(pm::Range {
                start: e.base,
                end: e.base.saturating_add(e.length),
            });
            memmap += 1;
        }
    }
    let phys_bits = core::arch::x86_64::__cpuid(0x8000_0008).eax & 0xFF;
    let ceiling = 1u64 << phys_bits.clamp(36, 52);
    say!(
        "i2c-hid: bar0 excluded ranges={} (bars={} windows={} memmap={} vmd={}) fw64=[{:#x},{:#x}) ceiling={:#x}",
        occupied.len(),
        bars,
        windows,
        memmap,
        vmd,
        floor,
        span_end,
        ceiling
    );
    crate::log::flush();
    if floor == u64::MAX {
        say!("i2c-hid: bar0 no firmware 64-bit BAR above 4 GiB to place it near - stopping");
        return false;
    }

    // 3. Probe each candidate with nothing claimed, then 4. claim it.
    let mut slots = [0u64; BAR_CANDIDATES];
    let n = pm::candidates(&occupied, size, floor, ceiling, &mut slots);
    for &slot in &slots[..n] {
        say!(
            "i2c-hid: bar0 candidate={:#x} from={}",
            slot,
            if slot.saturating_add(size) <= span_end {
                "fw-64bit-span"
            } else {
                "above-fw-span"
            }
        );
        crate::log::flush();
        if let Some((at, value)) = probe_unclaimed(slot, size) {
            say!(
                "i2c-hid: bar0 candidate {:#x} answered {:#010x} at +{:#x} - rejected",
                slot,
                value,
                at
            );
            continue;
        }
        pci::config_write32(dev, 0x10, (slot as u32 & 0xFFFF_FFF0) | (before.bar0 & 0xF));
        pci::config_write32(dev, 0x14, (slot >> 32) as u32);
        let now = Bars {
            bar0: pci::config_read32(dev, 0x10),
            bar1: pci::config_read32(dev, 0x14),
        };
        if now.address() != slot {
            say!(
                "i2c-hid: bar0 write did not take (read {:#010x}/{:#010x}) - stopping",
                now.bar0,
                now.bar1
            );
            return false;
        }
        say!("i2c-hid: bar0 assigned={:#x} by os", slot);
        crate::log::flush();
        return true;
    }
    say!(
        "i2c-hid: bar0 no candidate cleared the probe ({} tried) - stopping",
        n
    );
    false
}

/// Read a slot nothing has claimed yet: its first and last dword and three in
/// between must read all-ones (no device decodes there). Returns the first
/// offset and value that did not.
#[cfg(feature = "i2c-hid-bar-assign")]
fn probe_unclaimed(base: u64, size: u64) -> Option<(u64, u32)> {
    let offsets = [0, size / 4, size / 2, size / 4 * 3, size - 4];
    for at in offsets {
        let virt = crate::drivers::pci::map_mmio_for_probe(base + at, 4);
        // SAFETY: a page just mapped uncached over an address no BAR, bridge
        // window or memory-map entry covers; a read there has no side effect
        // the driver can cause (nothing decodes it, or the probe rejects it).
        let value = unsafe { core::ptr::read_volatile(virt.as_u64() as *const u32) };
        if value != u32::MAX {
            return Some((at, value));
        }
    }
    None
}

/// PMCSR as its raw value.
struct PmRaw(Option<u16>);

impl core::fmt::Display for PmRaw {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Some(pm) => write!(f, "{:#06x}(D{})", pm, pm & 0x3),
            None => f.write_str("no-pm-cap"),
        }
    }
}

/// PMCSR's No_Soft_Reset bit.
struct NsrText(Option<u16>);

impl core::fmt::Display for NsrText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Some(pm) => f.write_str(if pm & pm::PMCSR_NO_SOFT_RESET != 0 {
                "1"
            } else {
                "0"
            }),
            None => f.write_str("-"),
        }
    }
}

struct PmText(Option<u16>);

impl core::fmt::Display for PmText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Some(pm) => write!(f, "D{}", pm & 0x3),
            None => f.write_str("no-pm-cap"),
        }
    }
}

struct IdsText<'a>(&'a ReportMap);

impl core::fmt::Display for IdsText<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.0.uses_ids {
            return f.write_str("none");
        }
        for (i, (id, kind)) in self.0.ids[..self.0.id_count].iter().enumerate() {
            let k = match kind {
                hid::ReportKind::Mouse => "m",
                hid::ReportKind::Digitizer => "d",
                hid::ReportKind::Other => "o",
            };
            write!(f, "{}{}{}", if i > 0 { "," } else { "" }, id, k)?;
        }
        Ok(())
    }
}

struct RouteText(Result<GsiPin, Option<crate::arch::x86_64::ioapic::RouteError>>);

impl core::fmt::Display for RouteText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use crate::arch::x86_64::ioapic::RouteError;
        match self.0 {
            Ok(pin) => write!(f, "gsi{}/pin{}/low/level", pin.gsi, pin.pin),
            Err(None) => f.write_str("no-madt"),
            Err(Some(RouteError::NoIoApic)) => f.write_str("no-ioapic"),
            Err(Some(RouteError::NoPin(gsi))) => write!(f, "no-pin-gsi{}", gsi),
            Err(Some(RouteError::DestinationTooWide(id))) => {
                write!(f, "dest-apic-id-{}-too-wide", id)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResetAck {
    /// The interrupt came and the read was zero-length.
    Irq,
    /// No interrupt in 1 s, but a read was zero-length.
    Polled,
    /// The interrupt came but the read held data.
    IrqWithData(u16),
    Timeout,
    NotSent,
}

impl ResetAck {
    fn text(self) -> &'static str {
        match self {
            ResetAck::Irq => "irq",
            ResetAck::Polled => "polled(no-irq)",
            ResetAck::IrqWithData(_) => "irq-with-data",
            ResetAck::Timeout => "timeout",
            ResetAck::NotSent => "not-sent",
        }
    }
}

/// Up to 1 s for the reset's interrupt (without a route, a fixed 100 ms),
/// then the read that must be zero-length.
fn wait_reset_ack(dev: &Device, sent: bool, buf: &mut [u8; hid::MAX_INPUT]) -> ResetAck {
    if !sent {
        return ResetAck::NotSent;
    }
    let irq = if PIN.get().is_some() {
        let start = rdtsc();
        loop {
            if PENDING.swap(false, Ordering::AcqRel) {
                break true;
            }
            if rdtsc().wrapping_sub(start) >= 1000 * TSC_PER_MS {
                break false;
            }
            core::hint::spin_loop();
        }
    } else {
        spin_ms(100);
        false
    };
    match read_input(dev, buf) {
        Ok(Input::Empty) if irq => ResetAck::Irq,
        Ok(Input::Empty) => ResetAck::Polled,
        Ok(Input::Report(r)) if irq => ResetAck::IrqWithData(r.len() as u16),
        _ => ResetAck::Timeout,
    }
}

const fn mode_text(mode: u8) -> &'static str {
    match mode {
        MODE_IRQ => "irq",
        MODE_POLLED_NO_ROUTE => "polled(no-route)",
        MODE_POLLED_NO_IRQ => "polled(irq-never-fired)",
        MODE_POLLED_STORM => "polled(masked-storm)",
        MODE_FAILED => "failed",
        _ => "none",
    }
}

/// Read the input register once (a plain read: the input register is not
/// addressed, spec §6.1.1), `wMaxInputLength` bytes.
fn read_input<'a>(dev: &Device, buf: &'a mut [u8; hid::MAX_INPUT]) -> Result<Input<'a>, dw::Error> {
    let n = (dev.desc.max_input_length as usize).clamp(2, hid::MAX_INPUT);
    transfer(dev, &[], &mut buf[..n])?;
    READS.fetch_add(1, Ordering::Relaxed);
    Ok(hid::parse_input(&buf[..n]))
}

/// The ISR body (IF=0, interrupt gate): mask the line, signal and wake the
/// thread. One I/O APIC write, two atomics, `WAITER` for the take only, and
/// the scheduler lock when a thread was parked; no governance lock (I-14(a)).
pub fn handle_interrupt() {
    IRQS.fetch_add(1, Ordering::Relaxed);
    if let Some(pin) = PIN.get() {
        pin.set_masked(true);
    }
    PENDING.store(true, Ordering::Release);
    // Wake the parked service thread. `WAITER` is held only for the take;
    // the wake (scheduler lock) runs after it is dropped.
    let waiter = WAITER.lock().take();
    if let Some(token) = waiter {
        let _ = crate::sched::wake_thread_token(token, 0);
    }
}

/// IRQ mode, nothing pending: park until the ISR wakes this thread. Returns
/// at once if an interrupt is already pending, or the mode changed.
fn wait_for_irq() {
    // IF off for the whole check-and-park: the guard is released by the
    // context switch, which requires an IF-off caller (as futex's are), and
    // the ISR cannot run on this CPU between the check and the install.
    crate::arch::x86_64::without_interrupts(|| {
        let guard = WAITER.lock();
        if PENDING.load(Ordering::Acquire) || MODE.load(Ordering::Acquire) != MODE_IRQ {
            return;
        }
        let _ = crate::sched::block_current_with_token_and_guard(guard, |slot, token| {
            *slot = Some(token);
        });
    });
}

/// Read and decode one report; queue a mouse event if it changed anything.
fn service_one(dev: &Device, buf: &mut [u8; hid::MAX_INPUT], previous_buttons: &mut u8) -> Outcome {
    let input = match read_input(dev, buf) {
        Ok(input) => input,
        Err(e) => {
            ERRORS.fetch_add(1, Ordering::Relaxed);
            LAST_ERROR.store(error_code(e), Ordering::Relaxed);
            return Outcome::Error;
        }
    };
    let report = match input {
        Input::Empty => {
            EMPTY.fetch_add(1, Ordering::Relaxed);
            return Outcome::NoData;
        }
        Input::Bogus => {
            BOGUS.fetch_add(1, Ordering::Relaxed);
            return Outcome::NoData;
        }
        Input::BadLength(_) => {
            BAD_LENGTH.fetch_add(1, Ordering::Relaxed);
            return Outcome::NoData;
        }
        Input::Report(r) => r,
    };
    REPORTS.fetch_add(1, Ordering::Relaxed);
    if dev.map.uses_ids
        && let Some(&id) = report.first()
    {
        LAST_ID.store(id as u32, Ordering::Relaxed);
    }
    #[cfg(feature = "freeze-watchdog")]
    {
        let mut raw = [0u8; 4];
        for (dst, src) in raw.iter_mut().zip(report) {
            *dst = *src;
        }
        LAST_RAW.store(u32::from_le_bytes(raw), Ordering::Relaxed);
    }
    match hid::decode(&dev.map, report) {
        Decoded::Mouse(m) => {
            MOUSE.fetch_add(1, Ordering::Relaxed);
            #[cfg(feature = "freeze-watchdog")]
            {
                if m.buttons & !*previous_buttons != 0 {
                    BTN_DOWNS.fetch_add(1, Ordering::Relaxed);
                }
                LAST_BUTTONS.store(m.buttons, Ordering::Relaxed);
            }
            let changed = m.dx != 0 || m.dy != 0 || m.buttons != *previous_buttons;
            *previous_buttons = m.buttons;
            if changed {
                push(m);
            }
        }
        Decoded::Unexpected(..) => {
            UNEXPECTED.fetch_add(1, Ordering::Relaxed);
        }
        Decoded::UnknownId(_) => {
            UNKNOWN.fetch_add(1, Ordering::Relaxed);
        }
        Decoded::Short => {
            SHORT.fetch_add(1, Ordering::Relaxed);
        }
    }
    note_progress();
    Outcome::Data
}

fn pack(m: MouseReport) -> u32 {
    u32::from_le_bytes([m.buttons, m.dx as u8, m.dy as u8, m.wheel as u8])
}

fn unpack(v: u32) -> MouseReport {
    let b = v.to_le_bytes();
    MouseReport {
        buttons: b[0],
        dx: b[1] as i8,
        dy: b[2] as i8,
        wheel: b[3] as i8,
    }
}

/// Producer side (the service thread only).
fn push(m: MouseReport) {
    let head = HEAD.load(Ordering::Relaxed);
    if head.wrapping_sub(TAIL.load(Ordering::Acquire)) as usize >= RING_LEN {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    RING[head as usize % RING_LEN].store(pack(m), Ordering::Relaxed);
    HEAD.store(head.wrapping_add(1), Ordering::Release);
}

/// Consumer side (under `STATE`, or the boot check before any pump runs).
fn pop() -> Option<MouseReport> {
    let tail = TAIL.load(Ordering::Relaxed);
    if tail == HEAD.load(Ordering::Acquire) {
        return None;
    }
    let v = RING[tail as usize % RING_LEN].load(Ordering::Relaxed);
    TAIL.store(tail.wrapping_add(1), Ordering::Release);
    Some(unpack(v))
}

/// Start the service thread. After the scheduler is up.
pub fn start_service() {
    if !READY.load(Ordering::Acquire) {
        return;
    }
    let id = crate::sched::kthread::spawn_detached("i2c-hid", service);
    crate::serial_println!(
        "i2c-hid: service started (kthread slot={}) mode={}",
        id.slot(),
        mode_text(MODE.load(Ordering::Acquire))
    );
    crate::log::flush();
}

fn service(
    stop: crate::sched::kthread::KthreadStopToken,
) -> crate::sched::kthread::KthreadExitState {
    let Some(dev) = DEVICE.get() else {
        return crate::sched::kthread::KthreadExitState::Failed(1);
    };
    let mut buf = [0u8; hid::MAX_INPUT];
    let mut previous_buttons = 0u8;
    let mut line = LineGovernor::default();
    // The reset's interrupt left the line masked: start by unmasking it.
    if MODE.load(Ordering::Acquire) == MODE_IRQ {
        unmask();
    }
    loop {
        if stop.is_stop_requested() {
            return crate::sched::kthread::KthreadExitState::Success;
        }
        #[cfg(feature = "freeze-watchdog")]
        LOOPS.fetch_add(1, Ordering::Relaxed);
        let mode = MODE.load(Ordering::Acquire);
        if mode == MODE_IRQ {
            let mut passes = 0;
            while passes < PASS_MAX && PENDING.swap(false, Ordering::AcqRel) {
                passes += 1;
                let outcome = service_one(dev, &mut buf, &mut previous_buttons);
                match line.after_irq(outcome) {
                    Action::Continue => unmask(),
                    Action::MaskAndPoll => {
                        // I-14(b): a line that keeps asserting with nothing
                        // to read stays masked; the thread polls instead.
                        MODE.store(MODE_POLLED_STORM, Ordering::Release);
                        crate::serial_println!(
                            "i2c-hid: irq masked-storm ({} assertions without data) - polling",
                            line.spurious
                        );
                        crate::serial_println!("{}", Counters::now());
                        break;
                    }
                    Action::Stop => {
                        fail_service(line.errors);
                        break;
                    }
                }
            }
        } else if matches!(
            mode,
            MODE_POLLED_NO_ROUTE | MODE_POLLED_NO_IRQ | MODE_POLLED_STORM
        ) {
            let outcome = service_one(dev, &mut buf, &mut previous_buttons);
            if line.after_poll(outcome) == Action::Stop {
                fail_service(line.errors);
            }
        }
        // Idle without spinning. The product image has no `wait-multiplex`,
        // so `nap_ticks` there is a yield loop: an always-runnable thread.
        // IRQ mode parks until the ISR wakes it; a stopped service parks
        // until stopped; only the polled fallbacks still nap (no timed wake
        // exists without `wait-multiplex`).
        match MODE.load(Ordering::Acquire) {
            MODE_IRQ => wait_for_irq(),
            MODE_POLLED_NO_ROUTE | MODE_POLLED_NO_IRQ | MODE_POLLED_STORM => nap_ticks(1),
            _ => {
                stop.park_until_stop();
            }
        }
    }
}

fn fail_service(errors: u32) {
    MODE.store(MODE_FAILED, Ordering::Release);
    crate::serial_println!(
        "i2c-hid: {} transfer errors in a row (last {:#010x}) - stopped reading",
        errors,
        LAST_ERROR.load(Ordering::Relaxed)
    );
    crate::serial_println!("{}", Counters::now());
}

/// Unmask with interrupts off on this CPU, so the ISR cannot run between
/// the two I/O APIC writes here (see `GsiPin`).
fn unmask() {
    if let Some(pin) = PIN.get() {
        crate::arch::x86_64::without_interrupts(|| pin.set_masked(false));
    }
}

/// Wait at least `ticks` timer ticks, letting other threads run: parked
/// with `wait-multiplex` or `tick-sleep`, else a yield loop (CP-81).
fn nap_ticks(ticks: u64) {
    #[cfg(feature = "wait-multiplex")]
    crate::sched::wait_any::sleep_ticks(ticks);
    #[cfg(all(not(feature = "wait-multiplex"), feature = "tick-sleep"))]
    crate::sched::tick_sleep::sleep_ticks(ticks);
    #[cfg(all(not(feature = "wait-multiplex"), not(feature = "tick-sleep")))]
    {
        let ticks_now = || crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed);
        let deadline = ticks_now().saturating_add(ticks.max(1));
        while ticks_now() < deadline {
            crate::sched::yield_now();
            core::hint::spin_loop();
        }
    }
}

/// Deliver queued touchpad motion to the focus path as the USB mouse's
/// pointer events (`InputEventV1::pointer`, the same stage and flush).
/// Syscall context; never touches the bus.
#[cfg(feature = "atrium-input-routing")]
pub fn pump_routed_events() {
    if !READY.load(Ordering::Acquire) {
        return;
    }
    let mut guard = STATE.lock();
    let Some(stage) = guard.as_mut() else {
        return;
    };
    let routing = crate::atrium::input_routing_enabled();
    while let Some(m) = pop() {
        if !routing {
            continue;
        }
        DELIVERED.fetch_add(1, Ordering::Relaxed);
        // A9-C2: the mouse collection's wheel (when it reports one) under
        // atrium-pointer-wheel; without it, 0 (today's event).
        let event = crate::atrium::input_logic::InputEventV1::pointer_wheel(
            m.dx,
            m.dy,
            m.buttons,
            if cfg!(feature = "atrium-pointer-wheel") {
                m.wheel
            } else {
                0
            },
        );
        crate::atrium::focus::note_raw_buttons(m.buttons);
        if cfg!(feature = "atrium-input-stage-legacy") {
            crate::atrium::focus::route_event(event);
        } else {
            crate::atrium::focus::stage_event(stage, event);
        }
    }
    crate::atrium::focus::flush_pointer_staged(stage);
}

/// The watchdog overlay's touchpad line.
#[cfg(feature = "freeze-watchdog")]
pub struct WatchdogLine;

#[cfg(feature = "freeze-watchdog")]
impl core::fmt::Display for WatchdogLine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let layout = BUTTON_LAYOUT.load(Ordering::Relaxed);
        let raw = LAST_RAW.load(Ordering::Relaxed).to_le_bytes();
        write!(
            f,
            "tp: {} mode={} loops={} reports={} btn-downs={} last={:#04x} raw={:02x} {:02x} {:02x} {:02x} unexp={} buttons={}x{}/bit{}",
            if READY.load(Ordering::Acquire) {
                "bound"
            } else {
                "NOT-bound"
            },
            mode_text(MODE.load(Ordering::Relaxed)),
            LOOPS.load(Ordering::Relaxed),
            REPORTS.load(Ordering::Relaxed),
            BTN_DOWNS.load(Ordering::Relaxed),
            LAST_BUTTONS.load(Ordering::Relaxed),
            raw[0],
            raw[1],
            raw[2],
            raw[3],
            UNEXPECTED.load(Ordering::Relaxed),
            layout >> 16,
            (layout >> 8) & 0xFF,
            layout & 0xFF
        )
    }
}

/// The counter line again at the 1st, 2nd, 4th, 8th … report.
fn note_progress() {
    let reports = REPORTS.load(Ordering::Relaxed);
    if reports.is_power_of_two() {
        crate::serial_println!("{}", Counters::now());
    }
}

/// The runtime counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counters {
    pub mode: u8,
    pub irqs: u64,
    pub reads: u64,
    pub reports: u64,
    pub mouse: u64,
    pub last_id: u32,
    pub unexpected: u64,
    pub unknown: u64,
    pub short: u64,
    pub empty: u64,
    pub bogus: u64,
    pub bad_length: u64,
    pub errors: u64,
    pub dropped: u64,
    pub delivered: u64,
}

impl Counters {
    pub fn now() -> Self {
        Self {
            mode: MODE.load(Ordering::Relaxed),
            irqs: IRQS.load(Ordering::Relaxed),
            reads: READS.load(Ordering::Relaxed),
            reports: REPORTS.load(Ordering::Relaxed),
            mouse: MOUSE.load(Ordering::Relaxed),
            last_id: LAST_ID.load(Ordering::Relaxed),
            unexpected: UNEXPECTED.load(Ordering::Relaxed),
            unknown: UNKNOWN.load(Ordering::Relaxed),
            short: SHORT.load(Ordering::Relaxed),
            empty: EMPTY.load(Ordering::Relaxed),
            bogus: BOGUS.load(Ordering::Relaxed),
            bad_length: BAD_LENGTH.load(Ordering::Relaxed),
            errors: ERRORS.load(Ordering::Relaxed),
            dropped: DROPPED.load(Ordering::Relaxed),
            delivered: DELIVERED.load(Ordering::Relaxed),
        }
    }
}

impl core::fmt::Display for Counters {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "i2c-hid: reports={} id=", self.reports)?;
        if self.last_id == u32::MAX {
            f.write_str("none")?;
        } else {
            write!(f, "{}", self.last_id)?;
        }
        write!(
            f,
            " parsed={} unexpected={} unknown={} short={} empty={} bogus={} badlen={} irq={} count={} reads={} errors={} dropped={} delivered={} mode={}",
            self.mouse,
            self.unexpected,
            self.unknown,
            self.short,
            self.empty,
            self.bogus,
            self.bad_length,
            GsiText(PIN.get().map(|p| p.gsi)),
            self.irqs,
            self.reads,
            self.errors,
            self.dropped,
            self.delivered,
            mode_text(self.mode)
        )
    }
}

struct GsiText(Option<u32>);

impl core::fmt::Display for GsiText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            Some(gsi) => write!(f, "gsi{}", gsi),
            None => f.write_str("none"),
        }
    }
}

/// Print every bring-up line again, whatever the outcome (a non-match's one
/// `i2c-hid: no match (...)` line included), so the last block on the boot
/// console localises it. Returns how many lines were printed.
fn replay_boot_lines() -> usize {
    let lines = BOOT_LINES.lock();
    if lines.is_empty() {
        return 0;
    }
    crate::serial_println!("i2c-hid: ---- touchpad (again) ----");
    for line in lines.iter() {
        crate::serial_println!("{}", line);
    }
    crate::log::flush();
    lines.len()
}

/// Test hook: the replay the image A check runs, and whether its first line
/// is the no-match line.
#[cfg(feature = "neo-test")]
pub fn replay_boot_lines_for_test() -> (usize, bool) {
    let n = replay_boot_lines();
    let no_match = BOOT_LINES
        .lock()
        .first()
        .is_some_and(|l| l.starts_with("i2c-hid: no match ("));
    (n, no_match)
}

/// Image A: the touchpad's on-screen check, after the USB summary and just
/// before the desktop takes the framebuffer. With the touchpad bound, up to
/// `check_ticks` for a report (a finger moved or a click), then the counter
/// line; with nothing received, the screen is held up to `hold_ticks` (a
/// report ends the hold). The events read here are discarded, so the desktop
/// does not start with a jump.
#[cfg(all(feature = "fb-progress-console", feature = "desktop-session"))]
pub fn evidence_check(check_ticks: u64, hold_ticks: u64) {
    replay_boot_lines();
    if !READY.load(Ordering::Acquire) {
        not_working_hold(hold_ticks);
        return;
    }
    let ticks_now = || crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed);
    let wait = |ticks: u64| {
        let before = REPORTS.load(Ordering::Relaxed);
        let deadline = ticks_now().saturating_add(ticks);
        while ticks_now() < deadline {
            if REPORTS.load(Ordering::Relaxed) > before {
                return true;
            }
            nap_ticks(10);
        }
        false
    };
    crate::serial_println!(
        "i2c-hid: TOUCHPAD CHECK - move a finger on the touchpad and click now ({} s)",
        check_ticks / 100
    );
    crate::log::flush();
    let got = wait(check_ticks);
    crate::serial_println!(
        "i2c-hid: touchpad check {}: {}",
        if got { "RECEIVED" } else { "NOTHING RECEIVED" },
        Counters::now()
    );
    crate::log::flush();
    if !got {
        crate::serial_println!(
            "i2c-hid: holding this screen up to {} s (touching the touchpad ends the hold)",
            hold_ticks / 100
        );
        crate::log::flush();
        if wait(hold_ticks) {
            crate::serial_println!(
                "i2c-hid: input arrived during the hold: {}",
                Counters::now()
            );
            crate::log::flush();
        }
    }
    // Discard what the check queued (the pump has not run yet).
    #[cfg(feature = "atrium-input-routing")]
    {
        let _guard = STATE.lock();
        while pop().is_some() {}
    }
}

/// Image A without `boot-evidence-holds`: the same lines as
/// [`evidence_check`] (the bring-up lines again, then the counters, or the
/// not-working verdict), printed (serial, and the boot console), with no
/// check and no hold. Returns the lines replayed and whether the touchpad is
/// working.
#[cfg(any(
    all(feature = "fb-progress-console", feature = "desktop-session"),
    feature = "neo-test-i2c-hid-absent"
))]
pub fn evidence_lines() -> (usize, bool) {
    let replayed = replay_boot_lines();
    let ready = READY.load(Ordering::Acquire);
    if ready {
        crate::serial_println!("i2c-hid: touchpad bound: {}", Counters::now());
    } else {
        not_working_lines();
    }
    crate::log::flush();
    // As after the check: the desktop does not start with queued events.
    #[cfg(feature = "atrium-input-routing")]
    {
        let _guard = STATE.lock();
        while pop().is_some() {}
    }
    (replayed, ready)
}

/// The touchpad is not working (no match, or matched but not bound): one
/// final line with the verdict, the step it stopped at and the counters, then
/// the screen is held up to `hold_ticks` with a countdown every 5 s, so the
/// failure can be photographed before the desktop paints over it. A byte from
/// the built-in keyboard ends the hold early (the i8042 ISR counts it, no pump
/// needed; Shift is the key to press, as a modifier alone reaches no client).
/// Returns whether a key ended it.
/// The not-working verdict: which step the bring-up stopped at, and the
/// counters. The hold below prints it first; without `boot-evidence-holds`
/// it is printed alone ([`evidence_lines`]).
#[cfg(any(
    all(feature = "fb-progress-console", feature = "desktop-session"),
    feature = "neo-test-i2c-hid-absent"
))]
fn not_working_lines() {
    let verdict = VERDICT.get().copied();
    let probed = PROBED.load(Ordering::Acquire);
    {
        let lines = BOOT_LINES.lock();
        let last = lines
            .last()
            .map(|l| l.as_str())
            .unwrap_or("(no bring-up line)");
        crate::serial_println!(
            "i2c-hid: TOUCHPAD NOT WORKING - {} verdict={} - stopped at: {}",
            if probed {
                "matched but NOT bound"
            } else {
                "no match"
            },
            match verdict {
                Some(v) => alloc::format!("{}", VerdictText(v)),
                None => alloc::string::String::from("none"),
            },
            last
        );
    }
    crate::serial_println!("{}", Counters::now());
}

#[cfg(any(
    all(feature = "fb-progress-console", feature = "desktop-session"),
    feature = "neo-test-i2c-hid-absent"
))]
fn not_working_hold(hold_ticks: u64) -> bool {
    not_working_lines();
    crate::serial_println!(
        "i2c-hid: holding this screen {} s for a photo (press Shift to continue)",
        hold_ticks / 100
    );
    crate::log::flush();
    let ticks_now = || crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed);
    #[cfg(feature = "ps2-keyboard")]
    let key_bytes = || crate::drivers::ps2::i8042::Counters::now().bytes;
    #[cfg(not(feature = "ps2-keyboard"))]
    let key_bytes = || 0u64;
    let before = key_bytes();
    let start = ticks_now();
    let deadline = start.saturating_add(hold_ticks);
    let mut next_note = start.saturating_add(500);
    while ticks_now() < deadline {
        if key_bytes() != before {
            crate::serial_println!("i2c-hid: hold ended by a key press");
            crate::log::flush();
            return true;
        }
        if ticks_now() >= next_note {
            crate::serial_println!(
                "i2c-hid: touchpad not working - {} s left",
                deadline.saturating_sub(ticks_now()).div_ceil(100)
            );
            crate::log::flush();
            next_note = next_note.saturating_add(500);
        }
        nap_ticks(10);
    }
    false
}

// --- Test hooks (`i2c_hid_absent`) ---------------------------------------------

/// CP-81's discriminator on QEMU (no touchpad, so the real service thread
/// never starts): a test thread calls `wait_for_irq` in IRQ mode. It must
/// park (its token appears in `WAITER`), stay parked for 20 ticks, and run
/// again once the ISR body is called. Then the lost-wakeup case: with
/// `PENDING` already set it must return without parking. Returns
/// `(parked, woken_early, woken_by_isr, skipped_when_pending)`.
#[cfg(feature = "neo-test-i2c-hid-absent")]
pub fn irq_wait_for_test() -> (bool, bool, bool, bool) {
    use crate::sched::kthread::{self, KthreadExitState};
    static WOKE: AtomicBool = AtomicBool::new(false);
    let ticks_now = || crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed);
    // SAFETY: RDTSC has no side effect.
    let tsc = || unsafe { core::arch::x86_64::_rdtsc() };
    // Each pass leaves interrupts on for a real window (CP-73): the check
    // and `yield_now` are both IF=0 sections, and back to back they starve
    // the tick. The TSC bound ends the wait even if the tick stops.
    let wait_until = |ticks: u64, done: &dyn Fn() -> bool| {
        let deadline = ticks_now() + ticks;
        let tsc_deadline = tsc() + ticks * 20_000_000;
        while !done() && ticks_now() < deadline && tsc() < tsc_deadline {
            for _ in 0..2_000 {
                core::hint::spin_loop();
            }
            crate::sched::yield_now();
        }
        done()
    };
    let previous_mode = MODE.swap(MODE_IRQ, Ordering::AcqRel);
    PENDING.store(false, Ordering::Release);
    WOKE.store(false, Ordering::Release);
    let job = |_stop| {
        wait_for_irq();
        WOKE.store(true, Ordering::Release);
        KthreadExitState::Success
    };
    let handle = kthread::spawn_owned("i2c-hid-wait-test", job);
    let parked = wait_until(200, &|| {
        crate::arch::x86_64::without_interrupts(|| WAITER.lock().is_some())
    });
    let woken_early = wait_until(20, &|| WOKE.load(Ordering::Acquire));
    crate::arch::x86_64::without_interrupts(handle_interrupt);
    let woken_by_isr = wait_until(200, &|| WOKE.load(Ordering::Acquire));
    let _ = kthread::join(&handle);
    // Pending already set: no park, so this returns on the calling thread.
    PENDING.store(true, Ordering::Release);
    wait_for_irq();
    let skipped_when_pending = crate::arch::x86_64::without_interrupts(|| WAITER.lock().is_none());
    PENDING.store(false, Ordering::Release);
    MODE.store(previous_mode, Ordering::Release);
    (parked, woken_early, woken_by_isr, skipped_when_pending)
}

/// The not-working hold image A runs on this path, for `hold_ticks`; returns
/// the ticks it took and whether a key ended it.
#[cfg(feature = "neo-test-i2c-hid-absent")]
pub fn not_working_hold_for_test(hold_ticks: u64) -> (u64, bool) {
    let ticks_now = || crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed);
    let start = ticks_now();
    let early = not_working_hold(hold_ticks);
    (ticks_now() - start, early)
}

/// The boot run's verdict.
#[cfg(feature = "neo-test")]
pub fn verdict_for_test() -> Option<Verdict> {
    VERDICT.get().copied()
}

/// Whether the controller path was entered.
#[cfg(feature = "neo-test")]
pub fn probed_for_test() -> bool {
    PROBED.load(Ordering::Acquire)
}

/// I-14(a), at run time: the ISR body runs to completion while this context
/// holds nine locks and flags, taken in lockdep order:
/// - the AI-governance `ARENA` (12, an `IrqSafeMutex`, so IF=0 from here on),
///   `RESERVATIONS` (20), `INFLIGHT` (22), `TABLE` (25) and `RECENCY` (28);
/// - `DELIVERY` and the driver's `STATE` (both `IrqSafeMutex`);
/// - the driver's `BOOT_LINES`;
/// - the `BUS_OWNER` flag (checked as taken).
///
/// With IF=0 nothing could release any of them, so an ISR that took one would
/// spin forever and the scenario would time out. Returns the IRQ count the
/// body added.
#[cfg(feature = "neo-test-i2c-hid-absent")]
pub fn isr_with_driver_locks_held_for_test() -> u64 {
    let before = IRQS.load(Ordering::Relaxed);
    crate::ai::arena::debug_with_arena_held(|| {
        crate::ai::reservation::debug_with_reservations_held(|| {
            crate::drivers::accel::accounting::debug_with_accounting_held(|| {
                crate::atrium::focus::debug_with_delivery_held(|| {
                    let _state = STATE.lock();
                    let _lines = BOOT_LINES.lock();
                    let taken = BUS_OWNER
                        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                        .is_ok();
                    HELD_FOR_TEST.store(taken, Ordering::Release);
                    handle_interrupt();
                    if taken {
                        BUS_OWNER.store(false, Ordering::Release);
                    }
                })
            })
        })
    });
    PENDING.store(false, Ordering::Release);
    IRQS.load(Ordering::Relaxed) - before
}

/// Whether the ISR test really held `BUS_OWNER` (nothing else had it).
#[cfg(feature = "neo-test-i2c-hid-absent")]
static HELD_FOR_TEST: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "neo-test-i2c-hid-absent")]
pub fn bus_owner_was_held_for_test() -> bool {
    HELD_FOR_TEST.load(Ordering::Acquire)
}

/// The bounded-timeout proof with no controller: the real `Mmio` register
/// access and TSC clock, aimed at a zeroed RAM page standing in for BAR0
/// (writes land, `IC_ENABLE_STATUS` never sets). Returns the enable result,
/// a whole transfer's result, and the TSC cycles both took.
#[cfg(feature = "neo-test")]
pub fn no_controller_timeout_for_test() -> (Result<(), dw::Error>, Result<(), dw::Error>, u64) {
    let page = alloc::vec![0u32; 1024];
    let mut regs = Mmio {
        base: page.as_ptr() as usize,
    };
    let start = rdtsc();
    let enable = dw::enable(&mut regs, &BUDGET);
    let ctl = dw::Controller {
        tx_depth: 64,
        rx_depth: 64,
        version: 0,
        fast: dw::fast_mode_counts(dw::LPSS_I2C_CLOCK_HZ),
        sda_hold: 0,
    };
    let mut buf = [0u8; hid::HID_DESC_LEN];
    let xfer = dw::transfer(&mut regs, &ctl, &BUDGET, 0x15, &[0x01, 0x00], &mut buf);
    let elapsed = rdtsc().wrapping_sub(start);
    drop(page);
    (enable, xfer, elapsed)
}

/// The handshake budget, in TSC cycles (the bound the proof is checked
/// against).
#[cfg(feature = "neo-test")]
pub const fn handshake_budget_for_test() -> u64 {
    BUDGET.handshake
}
