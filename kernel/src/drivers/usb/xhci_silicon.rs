// SPDX-License-Identifier: MPL-2.0
//! xHCI bring-up for real Intel client silicon (the Zenbook USB input lane,
//! feature `xhci-silicon`; `Plans/NOTE-2026-10-22-zenbook-usb-input.md`).
//!
//! A child module of `xhci`, so it uses the parent's private register
//! helpers. Feature off: none of this is compiled and `xhci` brings up the
//! first class-0x0C0330 function exactly as before. What it adds:
//!
//! 1. **A census of every xHCI** before anything is reset: PCI identity,
//!    power state, the Supported Protocol port ranges, and which root ports
//!    had a device connected as firmware left them. One controller is then
//!    chosen by [`logic::choose`]: Intel parts since Tiger Lake list the
//!    Type-C subsystem's SuperSpeed-only xHCI (bus 0 device 0x0d) before the
//!    south xHCI (device 0x14) that carries every USB 2 pin, including the
//!    USB-C ports' — so "the first xHCI" can never see a keyboard or mouse.
//! 2. **D0** for a controller firmware left in D3hot.
//! 3. **The BIOS handoff as Linux does it** (principle only): take ownership
//!    when firmware does not release it in time, then switch off every SMI
//!    source in USBLEGCTLSTS and clear the latched SMI events.
//! 4. **Port power and settle**: power any root port with PP clear, then wait
//!    (time-bounded, TSC) for the ports the census saw connected to show a
//!    connection again after the reset, before the boot scan reads them.
//! 5. **Hubs**: a hub slot is marked with Configure Endpoint on xHCI > 0.95
//!    (Evaluate Context does not carry the Hub fields there), and the scan of
//!    a hub's ports waits bPwrOn2PwrGood + 100 ms debounce after power-on.
//! 6. **The USB summary** ([`log_usb_summary`]): the on-screen evidence.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};

use spin::Mutex;
use x86_64::VirtAddr;

use super::super::xhci_silicon_logic::{self as logic, Candidate, ChoiceReason, ExtCaps};
use super::{
    AddressRoute, CAP_CAPLENGTH, CAP_HCCPARAMS1, CAP_HCSPARAMS1, CAP_HCSPARAMS2, CLASS_SERIAL_BUS,
    CONTROLLER, ClaimedDevice, OP_BASE, OP_PORTSC_BASE, OP_USBCMD, OP_USBSTS, PORTSC_CCS,
    PORTSC_PED, PORTSC_PP, PORTSC_RW1C, PORTSC_STRIDE, PROGIF_XHCI, SUBCLASS_USB, Trb, TrbType,
    USBCMD_RS, USBSTS_CNR, USBSTS_HCH, USBSTS_HSE, XHCI_BAR_WINDOW, control_for, ctx_stride,
    mmio_read32, mmio_write32, portsc, portsc_write, submit_command,
};
use crate::drivers::pci::{self, Device as PciDeviceId};

/// Ports tracked per controller (xHCI MaxPorts is 8 bits).
const PORT_WORDS: usize = 4;
/// Raw PORTSC values kept per controller for the summary (the first ports).
const RAW_PORTS: usize = 32;

/// One census row: what one xHCI looked like before this kernel touched it.
#[derive(Clone, Copy)]
struct Row {
    dev: PciDeviceId,
    d_before: Option<u8>,
    d_after: Option<u8>,
    readable: bool,
    version: u16,
    ports: u8,
    caps: ExtCaps,
    connected: [u64; PORT_WORDS],
    candidate: Candidate,
    // Raw registers as firmware left them (the summary's discriminators).
    bar: u64,
    hcs1: u32,
    hcs2: u32,
    hcc1: u32,
    usbcmd: u32,
    usbsts: u32,
    msi_cap: bool,
    msix_cap: bool,
    portsc: [u32; RAW_PORTS],
}

static CENSUS: Mutex<Vec<Row>> = Mutex::new(Vec::new());
static CHOICE: Mutex<Option<(usize, ChoiceReason)>> = Mutex::new(None);
static HCI_VERSION: AtomicU16 = AtomicU16::new(0);
static HANDOFF: Mutex<&'static str> = Mutex::new("not-run");
static LEGCTL_AFTER: AtomicU32 = AtomicU32::new(0);
static MSI_CONFIGURED: AtomicBool = AtomicBool::new(false);
static SETTLE: Mutex<(u16, u16, u16)> = Mutex::new((0, 0, 0));

fn set_bit(words: &mut [u64; PORT_WORDS], port: u32) {
    let index = port as usize;
    if index < PORT_WORDS * 64 {
        words[index / 64] |= 1 << (index % 64);
    }
}

fn has_bit(words: &[u64; PORT_WORDS], port: u32) -> bool {
    let index = port as usize;
    index < PORT_WORDS * 64 && words[index / 64] & (1 << (index % 64)) != 0
}

// --- Time before the timer is calibrated --------------------------------------------

fn rdtsc() -> u64 {
    // SAFETY: RDTSC is unprivileged and side-effect free.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Spin until `done` holds or at least `ms` milliseconds pass on any TSC up
/// to 10 GHz (longer on a slower one). Returns whether `done` held.
pub(super) fn wait_at_least_ms(ms: u64, mut done: impl FnMut() -> bool) -> bool {
    let start = rdtsc();
    let budget = logic::tsc_cycles_for_ms(ms);
    loop {
        if done() {
            return true;
        }
        if rdtsc().wrapping_sub(start) >= budget {
            return false;
        }
        core::hint::spin_loop();
    }
}

// --- 1 + 2: census, power state, choice ---------------------------------------------

/// Census every xHCI, choose one, and return it for bring-up.
pub(super) fn census_and_choose() -> Option<PciDeviceId> {
    let rows: Vec<Row> = pci::enumerate_all()
        .into_iter()
        .filter(|dev| {
            dev.class_code == CLASS_SERIAL_BUS
                && dev.subclass == SUBCLASS_USB
                && dev.prog_if == PROGIF_XHCI
        })
        .map(probe)
        .collect();
    let candidates: Vec<Candidate> = rows.iter().map(|row| row.candidate).collect();
    let choice = logic::choose(&candidates);
    // The control: the pre-lane rule (first in PCI scan order), so the
    // scenario's keyboard on the second controller is never seen.
    #[cfg(feature = "neo-test-usb-xhci-select-control")]
    let choice = choice.map(|(_, reason)| (0, reason));

    crate::serial_println!("xhci: census controllers={}", rows.len());
    for (index, row) in rows.iter().enumerate() {
        crate::serial_println!("xhci: census {}", census_line(index, row));
    }
    match choice {
        Some((index, reason)) => {
            let (bus, device, function) = rows[index].dev.bdf();
            crate::serial_println!(
                "xhci: chose [{}] {:02x}:{:02x}.{} reason={}{}",
                index,
                bus,
                device,
                function,
                reason.name(),
                if cfg!(feature = "neo-test-usb-xhci-select-control") {
                    " (control: first in scan order)"
                } else {
                    ""
                }
            );
        }
        None => {
            crate::serial_println!("xhci: census found no controller");
        }
    }
    crate::log::flush();
    let chosen = choice.map(|(index, _)| rows[index].dev);
    *CHOICE.lock() = choice;
    *CENSUS.lock() = rows;
    chosen
}

fn probe(dev: PciDeviceId) -> Row {
    let mut row = Row {
        dev,
        d_before: pci::power_management_csr(dev).map(logic::pmcsr_state),
        d_after: None,
        readable: false,
        version: 0,
        ports: 0,
        caps: ExtCaps::default(),
        connected: [0; PORT_WORDS],
        candidate: Candidate::default(),
        bar: 0,
        hcs1: 0,
        hcs2: 0,
        hcc1: 0,
        usbcmd: 0,
        usbsts: 0,
        msi_cap: pci::has_capability(dev, 0x05),
        msix_cap: pci::has_capability(dev, 0x11),
        portsc: [0; RAW_PORTS],
    };
    if row.d_before == Some(3) {
        // D3hot → D0, then the 10 ms D3hot→D0 recovery time (PCI PM 1.2
        // §5.6.1) before any register access.
        if let Some(pmcsr) = pci::power_management_csr(dev) {
            pci::write_power_management_csr(dev, logic::pmcsr_to_d0(pmcsr));
        }
        wait_at_least_ms(10, || false);
    }
    row.d_after = pci::power_management_csr(dev).map(logic::pmcsr_state);
    match pci::bar(dev, 0) {
        Some(pci::PciBar::Memory32 { base, .. } | pci::PciBar::Memory64 { base, .. }) => {
            row.bar = base;
        }
        _ => return row,
    }
    let Some(base) = pci::map_bar_region(dev, 0, 0, XHCI_BAR_WINDOW as u32) else {
        return row;
    };
    // Decode only: the census reads registers; bus mastering is granted to
    // the chosen controller alone, in `bring_up`.
    pci::enable_memory_decode(dev);
    let cap0 = mmio_read32(base, CAP_CAPLENGTH);
    let caplength = u64::from(cap0 & 0xFF);
    if cap0 == u32::MAX || caplength == 0 {
        return row;
    }
    let hcs1 = mmio_read32(base, CAP_HCSPARAMS1);
    let hcc1 = mmio_read32(base, CAP_HCCPARAMS1);
    row.hcs1 = hcs1;
    row.hcs2 = mmio_read32(base, CAP_HCSPARAMS2);
    row.hcc1 = hcc1;
    row.usbcmd = mmio_read32(base + caplength, OP_USBCMD);
    row.usbsts = mmio_read32(base + caplength, OP_USBSTS);
    row.readable = true;
    row.version = (cap0 >> 16) as u16;
    row.ports = (hcs1 >> 24) as u8;
    row.caps = logic::walk_ext_caps(
        &mut |offset| mmio_read32(base, offset),
        hcc1,
        XHCI_BAR_WINDOW,
    );
    let op = base + caplength;
    let mut connected = 0u16;
    let mut usb2_connected = 0u16;
    for port in 1..=u32::from(row.ports) {
        let offset = OP_PORTSC_BASE + u64::from(port - 1) * PORTSC_STRIDE;
        if offset + 4 > XHCI_BAR_WINDOW.saturating_sub(caplength) {
            break;
        }
        let sc = mmio_read32(op, offset);
        if let Some(raw) = row.portsc.get_mut(port as usize - 1) {
            *raw = sc;
        }
        if sc != u32::MAX && sc & PORTSC_CCS != 0 {
            set_bit(&mut row.connected, port);
            connected += 1;
            // The port's protocol range decides; with no range (a missing or
            // truncated capability chain), the port speed does.
            if logic::port_holds_usb2_device(
                row.caps.port_major(port as u8),
                ((sc >> 10) & 0xF) as u8,
            ) {
                usb2_connected += 1;
            }
        }
    }
    row.candidate = Candidate {
        readable: true,
        protocols_known: logic::protocols_known(&row.caps),
        usb2_ports: row.caps.usb2_ports(),
        usb2_connected,
        connected,
    };
    row
}

/// The controller's run state from USBCMD.RS / USBSTS.HCH (+ HSE, CNR).
fn run_state(usbcmd: u32, usbsts: u32) -> &'static str {
    match (usbcmd & USBCMD_RS != 0, usbsts & USBSTS_HCH != 0) {
        _ if usbsts & USBSTS_HSE != 0 => "host-system-error",
        _ if usbsts & USBSTS_CNR != 0 => "not-ready",
        (true, false) => "running",
        (false, true) => "halted",
        (true, true) => "halting",
        (false, false) => "stopping",
    }
}

/// Line 2 of a census row: raw capability registers, BAR, interrupt
/// capabilities, and the run state firmware left.
fn census_regs_line(index: usize, row: &Row) -> String {
    let mut line = String::new();
    let _ = write!(
        line,
        "[{}] bar={:#x} hcs1={:#010x} hcs2={:#010x} hcc1={:#010x} csz={} ppc={} msi={} msix={} fw-state={} usbcmd={:#x} usbsts={:#x}",
        index,
        row.bar,
        row.hcs1,
        row.hcs2,
        row.hcc1,
        if row.hcc1 & (1 << 2) != 0 { 64 } else { 32 },
        u8::from(row.hcc1 & (1 << 3) != 0),
        if row.msi_cap { "y" } else { "n" },
        if row.msix_cap { "y" } else { "n" },
        run_state(row.usbcmd, row.usbsts),
        row.usbcmd,
        row.usbsts
    );
    line
}

/// Line 3 of a census row: the raw PORTSC of every port firmware left with a
/// connection (at most eight), so the speed and enable state show per port.
fn census_ports_line(index: usize, row: &Row) -> String {
    let mut line = String::new();
    let _ = write!(line, "[{}] fw-ports", index);
    let mut shown = 0;
    for port in 1..=u32::from(row.ports).min(RAW_PORTS as u32) {
        let sc = row.portsc[port as usize - 1];
        if sc & PORTSC_CCS == 0 || sc == u32::MAX {
            continue;
        }
        if shown == 8 {
            let _ = write!(line, " ...");
            break;
        }
        let _ = write!(
            line,
            " p{}=usb{}:{}:{}:{:#x}",
            port,
            row.caps.port_major(port as u8).unwrap_or(0),
            speed_name((sc >> 10) & 0xF),
            if sc & PORTSC_PED != 0 { "en" } else { "dis" },
            sc
        );
        shown += 1;
    }
    if shown == 0 {
        let _ = write!(line, " none-connected");
    }
    line
}

fn d_state_text(state: Option<u8>) -> &'static str {
    match state {
        None => "nopm",
        Some(0) => "D0",
        Some(1) => "D1",
        Some(2) => "D2",
        _ => "D3",
    }
}

fn census_line(index: usize, row: &Row) -> String {
    let (bus, device, function) = row.dev.bdf();
    let mut line = String::new();
    let _ = write!(
        line,
        "[{}] {:02x}:{:02x}.{} {:04x}:{:04x} {}",
        index,
        bus,
        device,
        function,
        row.dev.vendor_id,
        row.dev.device_id,
        d_state_text(row.d_before)
    );
    if row.d_after != row.d_before {
        let _ = write!(line, "->{}", d_state_text(row.d_after));
    }
    if !row.readable {
        let _ = write!(line, " unreadable");
        return line;
    }
    let _ = write!(
        line,
        " v{:x}.{:02x} ports={}",
        row.version >> 8,
        row.version & 0xFF,
        row.ports
    );
    for range in row.caps.protocols() {
        let last = u16::from(range.first_port) + u16::from(range.count).saturating_sub(1);
        let _ = write!(line, " usb{}={}-{}", range.major, range.first_port, last);
    }
    let _ = write!(line, " ccs=");
    let mut any = false;
    for port in 1..=u32::from(row.ports) {
        if has_bit(&row.connected, port) {
            let _ = write!(line, "{}{}", if any { "," } else { "" }, port);
            any = true;
        }
    }
    if !any {
        let _ = write!(line, "-");
    }
    if row.caps.stop.is_some() {
        let _ = write!(line, " xecp-walk-stopped");
    }
    if row.caps.protocols_dropped != 0 {
        let _ = write!(line, " protocols-dropped={}", row.caps.protocols_dropped);
    }
    if !logic::protocols_known(&row.caps) {
        let _ = write!(line, " protocols=UNKNOWN");
    }
    line
}

// --- 3: BIOS handoff -----------------------------------------------------------------

/// Record the controller version (the hub update uses it).
pub(super) fn note_version(version: u16) {
    HCI_VERSION.store(version, Ordering::Relaxed);
}

/// Record whether MSI was configured (the summary prints it).
pub(super) fn note_msi(configured: bool) {
    MSI_CONFIGURED.store(configured, Ordering::Relaxed);
}

/// USBLEGSUP handoff: request, wait up to 500 ms (TSC-bounded, so at least
/// that long), force on timeout, then disable every firmware SMI source.
pub(super) fn legacy_handoff(base: VirtAddr, hcc1: u32) {
    let caps = logic::walk_ext_caps(
        &mut |offset| mmio_read32(base, offset),
        hcc1,
        XHCI_BAR_WINDOW,
    );
    if let Some(stop) = caps.stop {
        crate::serial_println!("xhci: xecp walk stopped early: {:?}", stop);
    }
    let Some(ecp) = caps.legacy else {
        crate::serial_println!("xhci: no usblegsup capability (no legacy bios) - os-owned");
        *HANDOFF.lock() = "no-legacy-cap";
        return;
    };
    let before = mmio_read32(base, ecp);
    crate::serial_println!("xhci: usblegsup before={:#010x}", before);
    mmio_write32(base, ecp, logic::legsup_request(before));
    let released = wait_at_least_ms(500, || {
        let now = mmio_read32(base, ecp);
        now & logic::LEGSUP_BIOS_OWNED == 0 && now & logic::LEGSUP_OS_OWNED != 0
    });
    let verdict = if released {
        "os-owned"
    } else {
        mmio_write32(base, ecp, logic::legsup_force(mmio_read32(base, ecp)));
        "forced"
    };
    let ctl_before = mmio_read32(base, ecp + 4);
    mmio_write32(base, ecp + 4, logic::legctl_disable_smis(ctl_before));
    let ctl_after = mmio_read32(base, ecp + 4);
    LEGCTL_AFTER.store(ctl_after, Ordering::Relaxed);
    *HANDOFF.lock() = verdict;
    crate::serial_println!(
        "xhci: usblegsup after={:#010x} verdict={} usblegctlsts before={:#010x} after={:#010x}",
        mmio_read32(base, ecp),
        verdict,
        ctl_before,
        ctl_after
    );
}

// --- 4: port power + settle -----------------------------------------------------------

/// After Run: power every root port whose PP is clear, wait the 20 ms the
/// spec gives power to stabilise, then wait (at most 300 ms) for each port
/// the census saw connected to report a connection again.
pub(super) fn power_and_settle_ports(op: VirtAddr, max_ports: u32) {
    let mut powered = 0u16;
    for port in 1..=max_ports {
        let sc = portsc(op, port);
        if sc & PORTSC_PP == 0 {
            portsc_write(op, port, (sc & !PORTSC_RW1C & !PORTSC_PED) | PORTSC_PP);
            powered += 1;
        }
    }
    if powered > 0 {
        wait_at_least_ms(20, || false);
    }
    let expected = {
        let census = CENSUS.lock();
        let choice = *CHOICE.lock();
        choice
            .and_then(|(index, _)| census.get(index).map(|row| row.connected))
            .unwrap_or([0; PORT_WORDS])
    };
    let want: u16 = (1..=max_ports).filter(|&p| has_bit(&expected, p)).count() as u16;
    let back = || {
        (1..=max_ports)
            .filter(|&p| has_bit(&expected, p) && portsc(op, p) & PORTSC_CCS != 0)
            .count() as u16
    };
    let settled = wait_at_least_ms(300, || back() == want);
    let reconnected = back();
    *SETTLE.lock() = (powered, want, reconnected);
    crate::serial_println!(
        "xhci: ports powered={} census_connected={} reconnected={} settled={}",
        powered,
        want,
        reconnected,
        settled
    );
}

// --- 5: hubs ------------------------------------------------------------------------

fn read_out_dword(hub: &ClaimedDevice, offset: usize) -> u32 {
    // SAFETY: the output context is a live 4096-byte arena allocation the
    // controller writes; dwords 0..=2 of the slot context are < 4096.
    unsafe { core::ptr::read_volatile(hub.out_ctx.as_mut_ptr().add(offset) as *const u32) }
}

/// Mark `hub`'s slot as a hub. On xHCI > 0.95, Configure Endpoint with only
/// the Slot Context added, the slot copied from the output context with Hub,
/// Number of Ports and (HS) TT Think Time set — the only command that carries
/// those fields there. Older controllers keep the Evaluate Context path.
pub(super) fn mark_slot_as_hub(
    hub: &mut ClaimedDevice,
    route: AddressRoute,
    ports: u8,
    characteristics: u16,
) -> Result<(), &'static str> {
    let version = HCI_VERSION.load(Ordering::Relaxed);
    if !logic::hub_needs_configure_endpoint(version) {
        return super::mark_slot_as_hub(hub, route, ports);
    }
    let stride = ctx_stride(hub.csz64);
    let high_speed = hub.speed == super::super::UsbSpeed::High;
    let multi_tt = select_multi_tt(hub, high_speed);
    let out = [
        read_out_dword(hub, 0),
        read_out_dword(hub, 4),
        read_out_dword(hub, 8),
    ];
    let slot = logic::hub_slot_context(out, ports, high_speed, multi_tt, characteristics);
    // SAFETY: the input context is the slot's exclusively owned arena
    // allocation; every offset is `< 2 * stride ≤ 128 < 4096`.
    unsafe {
        let p = hub.input.as_mut_ptr();
        core::ptr::write_bytes(p, 0, 2 * stride);
        core::ptr::write_volatile(p.add(4) as *mut u32, 1); // A0: slot context only
        let ctx = p.add(stride);
        core::ptr::write_volatile(ctx as *mut u32, slot[0]);
        core::ptr::write_volatile(ctx.add(4) as *mut u32, slot[1]);
        core::ptr::write_volatile(ctx.add(8) as *mut u32, slot[2]);
    }
    let configure = Trb {
        parameter: hub.input_addr.phys(),
        status: 0,
        control: control_for(
            TrbType::ConfigureEndpointCmd,
            false,
            (hub.slot_id as u32) << 24,
        ),
    };
    let completion = submit_command(configure).ok_or("hub configure-endpoint: no completion")?;
    crate::serial_println!(
        "xhci: hub slot={} marked by configure-endpoint (xhci {:x}.{:02x}) ports={} mtt={} code={}",
        hub.slot_id,
        version >> 8,
        version & 0xFF,
        ports,
        multi_tt,
        completion.completion_code()
    );
    if completion.completion_code() == 1 {
        Ok(())
    } else {
        Err("hub configure-endpoint refused")
    }
}

/// Read the hub's device descriptor; for a high-speed multi-TT hub
/// (bDeviceProtocol 2), select alternate setting 1 of interface 0, which
/// switches it to one TT per port (USB 2.0 §11.23.1). Returns whether the hub
/// now runs multi-TT: only then may the slot context say so.
fn select_multi_tt(hub: &mut ClaimedDevice, high_speed: bool) -> bool {
    let mut device = [0u8; 18];
    if hub
        .control_read(0x80, 6, 0x0100, 0, 18, &mut device)
        .is_err()
    {
        crate::serial_println!(
            "xhci: hub slot={} device descriptor read failed - single TT",
            hub.slot_id
        );
        return false;
    }
    let protocol = device[6];
    if !logic::wants_multi_tt(high_speed, protocol) {
        return false;
    }
    // SET_INTERFACE(interface 0, alternate 1): bmRequestType 0x01, bRequest 11.
    let switched = hub.control_no_data(0x01, 11, 1, 0).is_ok();
    crate::serial_println!(
        "xhci: hub slot={} multi-TT hub (protocol 2): alternate 1 {}",
        hub.slot_id,
        if switched {
            "selected"
        } else {
            "refused - single TT"
        }
    );
    switched
}

/// After powering a hub's ports: bPwrOn2PwrGood (descriptor byte 5, 2 ms
/// units) for power to be good, then the USB 2.0 100 ms connect debounce,
/// before the ports' connection bits mean anything.
pub(super) fn hub_power_good_wait(descriptor: &[u8; 9]) {
    let ms = u64::from(descriptor[5]) * 2 + 100;
    wait_at_least_ms(ms, || false);
    crate::serial_println!("xhci: hub power-good wait {} ms (at least)", ms);
}

// --- 6: the USB summary --------------------------------------------------------------

fn speed_name(bits: u32) -> &'static str {
    super::super::UsbSpeed::from_portsc(bits).name()
}

/// Print the USB state as one block: every xHCI from the census, the chosen
/// one's handoff and live root ports, every enumerated device and which
/// driver claimed it, and whether a keyboard and a mouse are bound. Each line
/// starts `usb-summary:` and is short enough for the framebuffer console.
pub fn log_usb_summary() {
    let census = CENSUS.lock().clone();
    let choice = *CHOICE.lock();
    crate::serial_println!("usb-summary: ---- USB input evidence ----");
    crate::serial_println!(
        "usb-summary: xhci controllers={} chosen={}",
        census.len(),
        match choice {
            Some((index, reason)) => {
                let mut text = String::new();
                let _ = write!(text, "[{}] reason={}", index, reason.name());
                text
            }
            None => String::from("none"),
        }
    );
    for (index, row) in census.iter().enumerate() {
        crate::serial_println!("usb-summary: {}", census_line(index, row));
        if row.readable {
            crate::serial_println!("usb-summary: {}", census_regs_line(index, row));
            crate::serial_println!("usb-summary: {}", census_ports_line(index, row));
        }
    }
    let (powered, want, back) = *SETTLE.lock();
    let handoff = *HANDOFF.lock();
    let (cmds_submitted, cmds_completed, irqs, events, _, _) = super::debug_counters();
    crate::serial_println!(
        "usb-summary: handoff={} legctl={:#010x} msi={} irqs={} events={} cmds={}/{}",
        handoff,
        LEGCTL_AFTER.load(Ordering::Relaxed),
        MSI_CONFIGURED.load(Ordering::Relaxed),
        irqs,
        events,
        cmds_completed,
        cmds_submitted
    );
    crate::serial_println!(
        "usb-summary: ports powered={} reconnected={}/{} running={}",
        powered,
        back,
        want,
        super::is_running()
    );
    let max_ports = CONTROLLER.lock().as_ref().map(|ctrl| ctrl.max_ports);
    let op = VirtAddr::new(OP_BASE.load(Ordering::Acquire));
    if op.as_u64() != 0 {
        let usbcmd = mmio_read32(op, OP_USBCMD);
        let usbsts = mmio_read32(op, OP_USBSTS);
        let arena = crate::drivers::dma_arena::bounds().unwrap_or((0, 0));
        crate::serial_println!(
            "usb-summary: chosen live state={} usbcmd={:#x} usbsts={:#x} irq={} dma-arena={:#x}+{:#x}",
            run_state(usbcmd, usbsts),
            usbcmd,
            usbsts,
            if MSI_CONFIGURED.load(Ordering::Relaxed) {
                "msi"
            } else {
                "polled"
            },
            arena.0,
            arena.1
        );
    }
    if let Some(max_ports) = max_ports
        && op.as_u64() != 0
    {
        let mut any = false;
        for port in 1..=max_ports {
            let sc = portsc(op, port);
            if sc & PORTSC_CCS == 0 {
                continue;
            }
            any = true;
            crate::serial_println!(
                "usb-summary: port {} connected enabled={} speed={} pls={} raw={:#010x}",
                port,
                sc & PORTSC_PED != 0,
                speed_name((sc >> 10) & 0xF),
                (sc >> 5) & 0xF,
                sc
            );
        }
        if !any {
            crate::serial_println!("usb-summary: no root port reports a connection");
        }
    }
    for hub in super::HUBS.lock().iter() {
        crate::serial_println!(
            "usb-summary: hub slot={} root-port={} {}",
            hub.slot_id,
            hub.port,
            hub.speed.name()
        );
    }
    let devices = super::super::devices();
    let claims = super::super::claims();
    for device in &devices {
        let claimed_by = claims
            .iter()
            .find(|(slot, _)| *slot == device.slot_id)
            .map(|(_, name)| *name)
            .unwrap_or("none");
        crate::serial_println!(
            "usb-summary: device port={} slot={} {} {:04x}:{:04x} class={:02x}/{:02x}/{:02x} driver={}",
            device.port,
            device.slot_id,
            device.speed.name(),
            device.vendor_id,
            device.product_id,
            device.class,
            device.subclass,
            device.protocol,
            claimed_by
        );
    }
    if devices.is_empty() {
        crate::serial_println!("usb-summary: no device enumerated");
    }
    let (kbd_reports, kbd_bytes, kbd_errors) = keyboard_counters();
    let (mouse_reports, mouse_events, mouse_errors) = mouse_counters();
    crate::serial_println!(
        "usb-summary: keyboard={} reports={} bytes={} ep-errors={} | mouse={} reports={} events={} ep-errors={}",
        if keyboard_bound() { "bound" } else { "none" },
        kbd_reports,
        kbd_bytes,
        kbd_errors,
        if mouse_bound() { "bound" } else { "none" },
        mouse_reports,
        mouse_events,
        mouse_errors
    );
    crate::log::flush();
}

/// (reports, decoded bytes, endpoint errors) of the bound keyboard driver.
fn keyboard_counters() -> (u64, u64, u64) {
    #[cfg(feature = "usb-hid")]
    return super::super::hid_keyboard::debug_counters();
    #[cfg(not(feature = "usb-hid"))]
    (0, 0, 0)
}

/// (reports, pointer events, endpoint errors) of the bound mouse driver.
fn mouse_counters() -> (u64, u64, u64) {
    #[cfg(feature = "usb-hid-mouse")]
    {
        let (reports, events, errors, _dropped) = super::super::hid_mouse::debug_counters();
        (reports, events, errors)
    }
    #[cfg(not(feature = "usb-hid-mouse"))]
    (0, 0, 0)
}

/// (keyboard reports, mouse reports) received so far.
pub fn report_counts() -> (u64, u64) {
    (keyboard_counters().0, mouse_counters().0)
}

pub fn keyboard_bound() -> bool {
    #[cfg(feature = "usb-hid")]
    return super::super::hid_keyboard::is_bound();
    #[cfg(not(feature = "usb-hid"))]
    false
}

pub fn mouse_bound() -> bool {
    #[cfg(feature = "usb-hid-mouse")]
    return super::super::hid_mouse::is_bound();
    #[cfg(not(feature = "usb-hid-mouse"))]
    false
}

/// Test seam: the census size and the chosen index.
#[allow(dead_code)] // consumed by the usb_xhci_select scenario
pub fn census_for_test() -> (usize, Option<usize>) {
    (
        CENSUS.lock().len(),
        (*CHOICE.lock()).map(|(index, _)| index),
    )
}
