// SPDX-License-Identifier: MPL-2.0
//! OT-90 P2: the power-off and restart mechanism (`power-control`). It runs the
//! plans `power_logic` makes; nothing here decides anything.
//! Design: `docs/THINK-2026-10-27-ot90-shutdown-restart.md` §3.1-§3.2, §4.
//!
//! - [`init`] runs once at boot, in process context with the allocator live. It
//!   reads the FADT, the DSDT and the SSDTs, decodes `\_S5`, and caches the
//!   power-off plan and the restart chain. A failed decode is cached too, as the
//!   "safe to turn off" plan with its reason.
//! - [`power_off`] and [`restart`] use only that cache: they never parse, decode,
//!   allocate or map. `init` maps any SystemMemory register the plans name. P2 calls them from test scenarios only; the clean shutdown
//!   sequence (P3) and `SYS_POWER` (P4) come later.
//!
//! **Interrupts.** Both paths log their plan and flush the log sink with
//! interrupts as the caller had them, then mask interrupts on this CPU for each
//! run of register writes, so nothing preempts between a status clear and the
//! PM1 write that depends on it. From there they print to COM1 only
//! (`serial_println_isr!`): the normal print path also feeds the block log sink,
//! whose lock is taken with interrupts enabled (CP-37). Delays poll the ACPI PM
//! timer, so they need no interrupt. The long waits (the 10 s S5 retry, the 3 s
//! ACPI_ENABLE poll, ~100 ms per reset step) and the final halt reopen
//! interrupts when the caller had them on: the other CPUs still run in P2, and
//! a masked section longer than the TLB-shootdown deadline is CP-64.
//!
//! **Port I/O.** Every SystemIO register a plan names passed
//! `power_logic::io_range_ok` (and a PCI-config one `pci_config_ok`), so the
//! `as u16` port casts below never truncate. PM1 is read and written 16 bits
//! wide, the PM timer 32, everything else 8.
//!
//! **For the P3/P4 callers:** the long waits and the halt assume the caller has
//! interrupts on (CP-64); P3 and P4 should assert that on entry.
//!
//! **What P2 leaves out (P3):** user processes, NeoFS's drain, the other CPUs,
//! interrupt masking at the I/O APIC, and device quiesce. On S5 the machine
//! loses power with them running; on restart the reset takes them down.

use x86_64::PhysAddr;
use x86_64::instructions::interrupts;
use x86_64::instructions::port::Port;

use super::power_logic::{
    self, FadtPower, Gas, NoPowerOff, PM1_ALL_FIXED_STATUS, PowerOffPlan, ResetChain, ResetStep,
    S5Error, SCI_EN, SPACE_PCI_CONFIG, SPACE_SYSTEM_IO, SPACE_SYSTEM_MEMORY, StatusEnable,
    X_WAK_STS,
};
use crate::mm;
use crate::sync::SwitchMutex;

/// What `init` found, kept for the power paths.
#[derive(Clone, Copy)]
struct Cached {
    fadt: FadtPower,
    plan: PowerOffPlan,
    chain: ResetChain,
}

/// Set once by `init`; read by the power paths. A leaf lock, never held across
/// another acquisition (the `DMAR` discipline in `acpi::mod`).
static CACHE: SwitchMutex<Option<Cached>> = SwitchMutex::new(None);

/// The S5 retry wait (ACPICA `hwsleep.c`: 10 s).
const S5_RETRY_US: u64 = 10_000_000;
/// The wait after each reset step before trying the next (THINK §4: ~100 ms).
const RESET_STEP_US: u64 = 100_000;
/// The wait for SCI_EN after writing ACPI_ENABLE (ACPICA `hwacpi.c`: 3 s).
const ACPI_ENABLE_US: u64 = 3_000_000;
/// Between the two 0xCF9 writes (Linux's `udelay(50)`, read for principle).
const CF9_GAP_US: u64 = 50;
/// The i8042 command port, its input-buffer-full bit, and the pulse-reset command.
const I8042_STATUS: u16 = 0x64;
const I8042_IBF: u8 = 1 << 1;
const I8042_RESET: u8 = 0xFE;

/// Decode `\_S5`, plan power-off and restart, and cache both. Logs the outcome.
/// Idempotent: a second call leaves the first result.
pub fn init(rsdp_phys: u64) {
    if CACHE.lock().is_some() {
        return;
    }
    #[cfg(feature = "power-stop-cpus")]
    RSDP.store(rsdp_phys, core::sync::atomic::Ordering::Release);
    let Some(fadt) = super::fadt(rsdp_phys).and_then(power_logic::parse_fadt) else {
        crate::serial_println!("[ kernel ] power: no FADT; power-off and restart unavailable");
        return;
    };
    let s5 = match super::definition_blocks(rsdp_phys) {
        Some(blocks) => {
            let (r, per) = power_logic::decode_s5_report(&blocks);
            let walked = per.iter().filter(|t| t.is_ok()).count();
            crate::serial_println!(
                "[ kernel ] power: \\_S5 walk tables={} walked={} result={:?}",
                blocks.len(),
                walked,
                r
            );
            r.map(|f| f.typ)
        }
        None => {
            crate::serial_println!("[ kernel ] power: no DSDT; \\_S5 not decoded");
            Err(S5Error::NoTables)
        }
    };
    let hhdm = mm::hhdm_offset();
    let plan = power_logic::plan_power_off(&fadt, s5, hhdm);
    let chain = power_logic::reset_chain(&fadt, hhdm);
    crate::serial_println!(
        "[ kernel ] power: plan={:?} reset_chain={:?} pm_timer={:x?}",
        plan,
        chain.steps(),
        fadt.pm_timer
    );
    // Map every SystemMemory register the plans name now, while the allocator
    // is live, so the S5 and reset paths only write (they never map, never
    // allocate a page-table frame).
    if let PowerOffPlan::HwReduced {
        control, status, ..
    } = plan
    {
        premap(control);
        premap(status);
    }
    for step in chain.steps() {
        if let ResetStep::FadtRegister { reg, .. } = *step {
            premap(reg);
        }
    }
    *CACHE.lock() = Some(Cached { fadt, plan, chain });
}

/// OT-90 P3b: the RSDP, for K5's MADT read.
#[cfg(feature = "power-stop-cpus")]
static RSDP: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// OT-90 P3b: the RSDP `init` was given (0 if none).
#[cfg(feature = "power-stop-cpus")]
pub fn rsdp() -> u64 {
    RSDP.load(core::sync::atomic::Ordering::Acquire)
}

/// OT-90 P3b: K4's wait, with interrupts off and no lock taken: poll `done`
/// for up to `us` microseconds on the ACPI PM timer (port 0x80 writes
/// without one). `fadt` is read before K4 ([`stop_timer`]).
#[cfg(feature = "power-stop-cpus")]
pub fn spin_us(fadt: Option<&FadtPower>, us: u64, done: &mut dyn FnMut() -> bool) -> bool {
    match fadt {
        Some(fadt) => poll_us(fadt, us, done),
        None => {
            for _ in 0..us {
                if done() {
                    return true;
                }
                outb(0x80, 0);
            }
            done()
        }
    }
}

/// OT-90 P3b: the power plan, copied before K4 (review #6 item 5: K7 took
/// `CACHE`'s lock after K4, which a parked CPU could hold). `SNAPSHOT_SET`
/// is published after `SNAPSHOT`, by the one shutdown CPU.
#[cfg(feature = "power-stop-cpus")]
static mut SNAPSHOT: Option<Cached> = None;
#[cfg(feature = "power-stop-cpus")]
static SNAPSHOT_SET: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// OT-90 P3b (Janus's #6 re-check, item 2): K4 left CPUs running; K7 then
/// skips any step that needs the shared PCI configuration ports.
#[cfg(feature = "power-stop-cpus")]
static HOLDOUTS: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// OT-90 P3b: K4's outcome for K7 (set once, by the shutdown CPU).
#[cfg(feature = "power-stop-cpus")]
pub fn note_holdouts() {
    HOLDOUTS.store(true, core::sync::atomic::Ordering::Release);
}

/// OT-90 P3b: copy the power plan before K4, and return the FADT timer
/// fields [`spin_us`] uses. From then on [`cached`] reads the copy and takes
/// no lock.
#[cfg(feature = "power-stop-cpus")]
pub fn stop_timer() -> Option<FadtPower> {
    let c = *CACHE.lock();
    // SAFETY: written once, here, by the shutdown CPU before K4 and before
    // `SNAPSHOT_SET` is published; read only after it is.
    unsafe { *core::ptr::addr_of_mut!(SNAPSHOT) = c };
    SNAPSHOT_SET.store(true, core::sync::atomic::Ordering::Release);
    c.map(|c| c.fadt)
}

fn cached() -> Option<Cached> {
    #[cfg(feature = "power-stop-cpus")]
    if SNAPSHOT_SET.load(core::sync::atomic::Ordering::Acquire) {
        // SAFETY: published by `stop_timer`, never written again.
        return unsafe { *core::ptr::addr_of!(SNAPSHOT) };
    }
    *CACHE.lock()
}

/// Enter S5. Never returns: the machine powers off, or it halts on the "safe to
/// turn off" message. It never restarts instead.
pub fn power_off() -> ! {
    let irqs = interrupts::are_enabled();
    let Some(c) = cached() else {
        crate::serial_println!("[ kernel ] power: off requested, no plan (init found no FADT)");
        safe_to_turn_off(None, irqs);
    };
    crate::serial_println!("[ kernel ] power: off plan={:?}", c.plan);
    crate::log::flush();
    interrupts::disable();
    match c.plan {
        PowerOffPlan::Legacy {
            pm1a,
            pm1b,
            typ,
            pm1a_evt,
            pm1b_evt,
            gpe0,
        } => legacy_sleep(&c.fadt, irqs, pm1a, pm1b, typ, [pm1a_evt, pm1b_evt], gpe0),
        PowerOffPlan::HwReduced {
            control,
            status,
            value,
        } => {
            crate::serial_println_isr!("[ kernel ] power: S5 hw-reduced control={:#x}", value);
            gas_write8(status, X_WAK_STS);
            gas_write8(control, value);
            wait(&c.fadt, irqs, S5_RETRY_US, || false);
            crate::serial_println_isr!("[ kernel ] power: still running after 10 s; retrying S5");
            gas_write8(control, value);
            wait(&c.fadt, irqs, S5_RETRY_US, || false);
            safe_to_turn_off(None, irqs);
        }
        PowerOffPlan::SafeToTurnOff(reason) => safe_to_turn_off(Some(reason), irqs),
    }
}

/// The legacy (PM1) S5 entry, THINK §3.1 steps 3-7. `typ.a` goes to PM1a and
/// `typ.b` to PM1b, never one value to both. Called with interrupts masked.
fn legacy_sleep(
    fadt: &FadtPower,
    irqs: bool,
    pm1a: Gas,
    pm1b: Option<Gas>,
    typ: power_logic::SleepType,
    pm1_evt: [Option<StatusEnable>; 2],
    gpe0: Option<StatusEnable>,
) -> ! {
    // Step 3: ACPI mode. Some firmware leaves SCI_EN clear until told.
    let now = inw(pm1a.address);
    if let Some((port, value)) = power_logic::acpi_enable_command(fadt, now) {
        crate::serial_println_isr!(
            "[ kernel ] power: SCI_EN clear; ACPI_ENABLE {:#x} to {:#x}",
            value,
            port
        );
        outb(port as u64, value);
        let enabled = wait(fadt, irqs, ACPI_ENABLE_US, || {
            inw(pm1a.address) & SCI_EN != 0
        });
        crate::serial_println_isr!("[ kernel ] power: SCI_EN after ACPI_ENABLE: {}", enabled);
    }
    // Steps 4-5: disable every GPE, then clear every fixed and GPE status bit
    // (write-1-to-clear), WAK_STS among them, as AcpiHwLegacySleep orders it.
    if let Some(g) = gpe0 {
        for i in 0..g.half_len as u64 {
            outb(g.enable.address + i, 0x00);
        }
        for i in 0..g.half_len as u64 {
            outb(g.status.address + i, 0xFF);
        }
    }
    for evt in pm1_evt.into_iter().flatten() {
        // PM1 status is 16 bits when PM1_EVT_LEN is 4 (every corpus set).
        if evt.half_len == 2 {
            outw(evt.status.address, PM1_ALL_FIXED_STATUS);
        }
    }
    // Step 6: the two writes, each register's own other bits kept.
    let a = power_logic::pm1_sleep_values(inw(pm1a.address), typ.a);
    let b = pm1b.map(|r| (r, power_logic::pm1_sleep_values(inw(r.address), typ.b)));
    crate::serial_println_isr!(
        "[ kernel ] power: S5 PM1a={:#x} write1={:#x} write2={:#x} pm1b={:x?}",
        pm1a.address,
        a[0],
        a[1],
        b.map(|(r, v)| (r.address, v))
    );
    outw(pm1a.address, a[0]);
    if let Some((r, v)) = b {
        outw(r.address, v[0]);
    }
    outw(pm1a.address, a[1]);
    if let Some((r, v)) = b {
        outw(r.address, v[1]);
    }
    // Step 7: still running. Wait, repeat the full second write (THINK §3.1:
    // not ACPICA's SLP_EN-only write), wait again, then give up safely.
    wait(fadt, irqs, S5_RETRY_US, || false);
    crate::serial_println_isr!("[ kernel ] power: still running after 10 s; retrying S5");
    outw(pm1a.address, a[1]);
    if let Some((r, v)) = b {
        outw(r.address, v[1]);
    }
    wait(fadt, irqs, S5_RETRY_US, || false);
    safe_to_turn_off(None, irqs);
}

/// Restart through the cached chain. Never returns.
pub fn restart() -> ! {
    run_reset_chain(false)
}

/// `ot90_restart_fallback_i8042`: restart with the FADT reset register step made
/// a no-op that still waits. Step 1 writes its value to the POST port 0x80
/// (harmless) instead of the register, then waits `NEUTRAL_STEP_WAIT_US` on
/// the real PM timer before the chain moves on to the keyboard controller.
/// The wait is longer than one wrap of a 24-bit PM timer (4.69 s; QEMU's is
/// 24-bit), so the run proves a long wait ends (the AAG's F1 and F5).
#[cfg(feature = "neo-test")]
pub fn restart_with_neutral_fadt_register() -> ! {
    run_reset_chain(true)
}

/// The test-only neutral step's wait: more than one 24-bit PM timer wrap.
#[cfg(feature = "neo-test")]
const NEUTRAL_STEP_WAIT_US: u64 = 6_000_000;

#[cfg_attr(not(feature = "neo-test"), allow(unused_variables))]
fn run_reset_chain(neutral_fadt_register: bool) -> ! {
    let irqs = interrupts::are_enabled();
    let Some(c) = cached() else {
        crate::serial_println!("[ kernel ] power: restart requested, no plan (init found no FADT)");
        restart_failed(irqs);
    };
    crate::serial_println!("[ kernel ] power: restart chain={:?}", c.chain.steps());
    crate::log::flush();
    interrupts::disable();
    for (n, step) in c.chain.steps().iter().enumerate() {
        let n = n + 1;
        #[cfg(feature = "power-stop-cpus")]
        if HOLDOUTS.load(core::sync::atomic::Ordering::Acquire)
            && power_logic::step_uses_shared_ports(step)
        {
            crate::serial_println_isr!(
                "[ kernel ] power: reset step={} skipped: PCI config with K4 holdouts running",
                n
            );
            continue;
        }
        match *step {
            ResetStep::FadtRegister { reg, value } => {
                #[cfg(feature = "neo-test")]
                if neutral_fadt_register {
                    crate::serial_println_isr!(
                        "[ kernel ] power: reset step={} fadt_register neutral (test) value={:#x} to port 0x80",
                        n,
                        value
                    );
                    outb(0x80, value);
                    wait(&c.fadt, irqs, NEUTRAL_STEP_WAIT_US, || false);
                    crate::serial_println_isr!(
                        "[ kernel ] power: reset step={} wait ended us={}",
                        n,
                        NEUTRAL_STEP_WAIT_US
                    );
                    continue;
                }
                crate::serial_println_isr!(
                    "[ kernel ] power: reset step={} fadt_register space={} addr={:#x} value={:#x}",
                    n,
                    reg.space,
                    reg.address,
                    value
                );
                gas_write8(reg, value);
            }
            ResetStep::Keyboard => {
                crate::serial_println_isr!(
                    "[ kernel ] power: reset step={} keyboard 0xFE to 0x64",
                    n
                );
                // Bounded wait for the input buffer to empty, then the pulse.
                wait(&c.fadt, irqs, RESET_STEP_US, || {
                    inb(I8042_STATUS as u64) & I8042_IBF == 0
                });
                outb(I8042_STATUS as u64, I8042_RESET);
            }
            ResetStep::Cf9 => {
                let [first, second] = power_logic::cf9_values(inb(power_logic::CF9_PORT));
                crate::serial_println_isr!(
                    "[ kernel ] power: reset step={} cf9 {:#x} then {:#x}",
                    n,
                    first,
                    second
                );
                outb(power_logic::CF9_PORT, first);
                poll_us(&c.fadt, CF9_GAP_US, || false);
                outb(power_logic::CF9_PORT, second);
            }
            ResetStep::TripleFault => {
                crate::serial_println_isr!("[ kernel ] power: reset step={} triple fault", n);
                triple_fault();
            }
        }
        wait(&c.fadt, irqs, RESET_STEP_US, || false);
    }
    restart_failed(irqs);
}

/// A zero-limit IDT, then a breakpoint: no handler can be found, the CPU
/// double-faults, then triple-faults, and resets.
fn triple_fault() {
    let null = x86_64::structures::DescriptorTablePointer {
        limit: 0,
        base: x86_64::VirtAddr::zero(),
    };
    // SAFETY: deliberately leaves this CPU without an IDT; the next exception
    // resets the machine, which is the point.
    unsafe {
        x86_64::instructions::tables::lidt(&null);
        core::arch::asm!("int3", options(nomem, nostack));
    }
}

fn restart_failed(irqs: bool) -> ! {
    crate::serial_println_isr!(
        "[ kernel ] power: Restart failed. It's safe to turn off the computer now."
    );
    halt(irqs)
}

fn safe_to_turn_off(reason: Option<NoPowerOff>, irqs: bool) -> ! {
    crate::serial_println_isr!(
        "[ kernel ] power: It's safe to turn off the computer now. reason={:?}",
        reason
    );
    halt(irqs)
}

/// Stop here for good. With interrupts reopened when the caller had them on,
/// so this CPU still answers IPIs (CP-64); P3 stops the other CPUs first.
fn halt(irqs: bool) -> ! {
    if irqs {
        interrupts::enable();
    } else {
        interrupts::disable();
    }
    loop {
        x86_64::instructions::hlt();
    }
}

// --- delays, on the ACPI PM timer ---------------------------------------------

/// A long wait from a masked run of register writes: interrupts are reopened
/// for it when the caller had them on (CP-64), and masked again before the next
/// writes. Returns whether `done` held.
fn wait(fadt: &FadtPower, irqs: bool, us: u64, done: impl FnMut() -> bool) -> bool {
    if irqs {
        interrupts::enable();
    }
    let held = poll_us(fadt, us, done);
    interrupts::disable();
    held
}

/// Poll `done` until it holds or `us` microseconds pass. Returns whether it held.
fn poll_us(fadt: &FadtPower, us: u64, mut done: impl FnMut() -> bool) -> bool {
    match fadt.pm_timer {
        // Summed step by step between reads (`PmTicks`): a 24-bit counter wraps
        // every 4.69 s, so one start-to-now difference can't time 10 s.
        Some((port, ext32)) => power_logic::pm_wait(us, ext32, || inl(port), done),
        None => {
            // No PM timer: about 1 µs per write to the POST port 0x80, the
            // classic ISA delay. Coarse, but bounded.
            for _ in 0..us {
                if done() {
                    return true;
                }
                outb(0x80, 0);
            }
            done()
        }
    }
}

// --- register access --------------------------------------------------------

/// An 8-bit write to a GAS register. SystemIO is written 8 bits wide whatever
/// the GAS says (ACPICA `AcpiReset`); SystemMemory through a mapped page; PCI
/// configuration space on segment 0, bus 0 (ACPI 6.5 §5.2.3.2: device in bits
/// 32-47, function in 16-31, offset in 0-15) by configuration mechanism #1.
fn gas_write8(reg: Gas, value: u8) {
    match reg.space {
        SPACE_SYSTEM_IO => outb(reg.address, value),
        SPACE_SYSTEM_MEMORY => {
            // `init` mapped the page (`premap`); the plan checked the address
            // (`phys_ok`). If the page isn't mapped, skip the write rather than
            // map here.
            let virt = mm::phys_to_virt(PhysAddr::new(reg.address));
            if !mm::kernel_virt_mapped(virt) {
                crate::serial_println_isr!(
                    "[ kernel ] power: register {:#x} not mapped; write skipped",
                    reg.address
                );
                return;
            }
            // SAFETY: the FADT names this register; `init` mapped its page.
            unsafe { core::ptr::write_volatile(virt.as_mut_ptr::<u8>(), value) };
        }
        #[cfg(feature = "power-stop-cpus")]
        SPACE_PCI_CONFIG if HOLDOUTS.load(core::sync::atomic::Ordering::Acquire) => {
            // The CF8/CFC pair with another CPU still running: never.
            crate::serial_println_isr!(
                "[ kernel ] power: PCI config write {:#x} skipped: K4 holdouts running",
                reg.address
            );
        }
        SPACE_PCI_CONFIG => {
            let device = (reg.address >> 32) & 0x1f;
            let function = (reg.address >> 16) & 0x7;
            let offset = reg.address & 0xff;
            let address = 0x8000_0000u32
                | (device as u32) << 11
                | (function as u32) << 8
                | (offset as u32 & 0xfc);
            // SAFETY: configuration mechanism #1, 0xCF8 address then the byte
            // within the 0xCFC dword.
            unsafe {
                Port::<u32>::new(0xCF8).write(address);
                Port::<u8>::new(0xCFC + (offset as u16 & 3)).write(value);
            }
        }
        _ => {}
    }
}

/// Map a SystemMemory register's page at `init` (process context, allocator
/// live). The planner only names addresses `phys_ok` accepts.
fn premap(reg: Gas) {
    if reg.space != SPACE_SYSTEM_MEMORY || !power_logic::phys_ok(reg.address, mm::hhdm_offset()) {
        return;
    }
    let page = PhysAddr::new(reg.address & !0xfff);
    if !mm::kernel_virt_mapped(mm::phys_to_virt(page)) {
        mm::map_kernel_mmio_page(page);
    }
}

fn inb(port: u64) -> u8 {
    // SAFETY: a port the FADT or the i8042/0xCF9 conventions name.
    unsafe { Port::<u8>::new(port as u16).read() }
}

fn inw(port: u64) -> u16 {
    // SAFETY: a PM1 register the FADT names.
    unsafe { Port::<u16>::new(port as u16).read() }
}

fn inl(port: u64) -> u32 {
    // SAFETY: the PM timer register the FADT names.
    unsafe { Port::<u32>::new(port as u16).read() }
}

fn outb(port: u64, value: u8) {
    // SAFETY: a port the FADT or the i8042/0xCF9 conventions name.
    unsafe { Port::<u8>::new(port as u16).write(value) }
}

fn outw(port: u64, value: u16) {
    // SAFETY: a PM1 register the FADT names; PM1 is written 16 bits wide (QEMU
    // only sees SLP_EN, in the high byte, on a 16-bit or high-byte write).
    unsafe { Port::<u16>::new(port as u16).write(value) }
}
