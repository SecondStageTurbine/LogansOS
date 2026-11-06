// SPDX-License-Identifier: MPL-2.0
//! Minimal local-APIC support for cross-CPU IPIs, in xAPIC or x2APIC mode.
//!
//! Phase 2 BUILD-3 SC-3B lands only the infrastructure needed to notify a
//! CPU that work was queued remotely. APIC timers and IOAPIC routing remain
//! out of scope.
//!
//! x2APIC (CP-68): the kernel requests it from Limine, and drives whichever
//! mode the CPUs were left in. The xAPIC path is unchanged (MMIO, the 8-bit
//! destination). In x2APIC mode every register is an MSR
//! (`apic_logic::x2apic_msr`), the ICR is one 64-bit write with a 32-bit
//! destination, and `IA32_APIC_BASE` is never rewritten: leaving x2APIC
//! except through the disabled state is illegal (#GP).

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use x86_64::PhysAddr;
use x86_64::registers::model_specific::{ApicBase, ApicBaseFlags, Msr};

use crate::arch::halt_forever;
use crate::arch::x86_64::apic_logic::{self, ApicMode};
use crate::arch::x86_64::per_cpu;
use crate::mm;
use crate::serial_println;

pub const RESCHEDULE_IPI_VECTOR: u8 = 0xfd;
pub const TLB_SHOOTDOWN_IPI_VECTOR: u8 = 0xfc;
pub const SPURIOUS_VECTOR: u8 = 0xff;
/// OT-90 P3b: the shutdown's STOP IPI (`cpu_stop`).
#[cfg(feature = "power-stop-cpus")]
pub const STOP_IPI_VECTOR: u8 = 0xfb;

const INVALID_LAPIC_ID: u32 = u32::MAX;
const APIC_BASE_MSR_ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;
const APIC_SVR: u64 = 0x0f0;
const APIC_EOI: u64 = 0x0b0;
const APIC_ICR_LOW: u64 = 0x300;
const APIC_ICR_HIGH: u64 = 0x310;
const APIC_SVR_ENABLE: u32 = 1 << 8;
const ICR_LEVEL_ASSERT: u32 = 1 << 14;
#[cfg(any(feature = "neo-test-syscall-exit-irq-window", feature = "lapic-timer"))]
const APIC_LVT_TIMER: u64 = 0x320;
#[cfg(any(feature = "neo-test-syscall-exit-irq-window", feature = "lapic-timer"))]
const APIC_TIMER_INITIAL_COUNT: u64 = 0x380;
#[cfg(any(feature = "neo-test-syscall-exit-irq-window", feature = "lapic-timer"))]
const APIC_TIMER_DIVIDE: u64 = 0x3e0;
/// Divide configuration 0b1011: count at the bus clock, undivided.
#[cfg(feature = "neo-test-syscall-exit-irq-window")]
const APIC_TIMER_DIVIDE_BY_1: u32 = 0b1011;

static APIC_BASE_PHYS: AtomicU64 = AtomicU64::new(0);
/// The mode every local APIC is driven in, fixed by the BSP in `init_bsp`
/// before any register access and checked by each AP in `init_ap`.
static X2APIC_MODE: AtomicBool = AtomicBool::new(false);
static LAPIC_IDS: [AtomicU32; per_cpu::MAX_CPUS] =
    [const { AtomicU32::new(INVALID_LAPIC_ID) }; per_cpu::MAX_CPUS];

pub fn register_lapic_id(cpu_id: u32, lapic_id: u32) {
    let idx = cpu_id as usize;
    debug_assert!(idx < per_cpu::MAX_CPUS, "apic: cpu_id exceeds MAX_CPUS");
    if idx < per_cpu::MAX_CPUS {
        LAPIC_IDS[idx].store(lapic_id, Ordering::Release);
    }
}

/// Forget `cpu_id`'s local APIC ID: an AP that was started but stayed offline
/// (it cannot run under the FPU save policy). It halts before its IDT is
/// loaded, so nothing may target it: `cpu_stop::stop_others` sends a STOP IPI
/// and then an NMI to every registered CPU (#57 re-check).
pub fn unregister_lapic_id(cpu_id: u32) {
    let idx = cpu_id as usize;
    if idx < per_cpu::MAX_CPUS {
        LAPIC_IDS[idx].store(INVALID_LAPIC_ID, Ordering::Release);
    }
}

/// xAPIC IPIs refused for a destination above 255 (the first is logged).
static XAPIC_IPI_REFUSED: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Whether an IPI can name APIC ID `id` in the driven mode.
pub fn ipi_addressable(id: u32) -> bool {
    apic_logic::ipi_addressable(mode(), id)
}

/// The mode the local APICs are driven in.
pub fn mode() -> ApicMode {
    if X2APIC_MODE.load(Ordering::Acquire) {
        ApicMode::X2Apic
    } else {
        ApicMode::XApic
    }
}

/// `reported_x2apic` is Limine's MP response flag bit 0 (x2APIC enabled).
pub fn init_bsp(lapic_id: u32, reported_x2apic: bool) {
    let (_, flags) = ApicBase::read();
    let (mode, mismatch) = apic_logic::resolve_mode(
        reported_x2apic,
        flags.contains(ApicBaseFlags::X2APIC_ENABLE),
    );
    if mismatch {
        serial_println!(
            "[ kernel ] apic: mode MISMATCH: bootloader reports x2apic={} but IA32_APIC_BASE says {}; driving {}",
            reported_x2apic,
            mode.name(),
            mode.name()
        );
    }
    X2APIC_MODE.store(mode == ApicMode::X2Apic, Ordering::Release);
    serial_println!("[ kernel ] apic: mode={}", mode.name());
    register_lapic_id(0, lapic_id);
    init_local_apic(0, lapic_id, "BSP");
}

pub fn init_ap(cpu_id: u32) {
    let lapic_id = lapic_id_for_cpu(cpu_id);
    init_local_apic(cpu_id, lapic_id, "AP");
}

fn init_local_apic(cpu_id: u32, lapic_id: u32, role: &str) {
    // Jarvis P2: check this CPU's own mode before ANY IA32_APIC_BASE write,
    // in both directions (`apic_logic::ap_mode_check`).
    let (frame, mut flags) = ApicBase::read();
    if apic_logic::ap_mode_check(
        mode(),
        flags.contains(ApicBaseFlags::LAPIC_ENABLE),
        flags.contains(ApicBaseFlags::X2APIC_ENABLE),
    ) == apic_logic::ApInit::HaltModeMismatch
    {
        serial_println!(
            "[ kernel ] apic: cpu={} lapic_id={} ({}) mode MISMATCH: IA32_APIC_BASE flags={:?} but the kernel drives {} - halting this CPU",
            cpu_id,
            lapic_id,
            role,
            flags,
            mode().name()
        );
        halt_forever();
    }
    if mode() == ApicMode::X2Apic {
        init_local_x2apic(cpu_id, lapic_id, role);
        return;
    }
    flags.remove(ApicBaseFlags::X2APIC_ENABLE);
    flags.insert(ApicBaseFlags::LAPIC_ENABLE);

    // SAFETY: the kernel keeps the APIC base frame from the CPU's own
    // IA32_APIC_BASE MSR and only toggles architectural enable/mode flags;
    // the mode check above excludes the illegal x2APIC -> xAPIC write.
    unsafe {
        ApicBase::write(frame, flags);
    }

    let phys = frame.start_address().as_u64() & APIC_BASE_MSR_ADDR_MASK;
    let mapped_phys = APIC_BASE_PHYS.load(Ordering::Acquire);
    if mapped_phys == 0 {
        mm::map_kernel_mmio_page(PhysAddr::new(phys));
        APIC_BASE_PHYS.store(phys, Ordering::Release);
    } else {
        debug_assert_eq!(mapped_phys, phys, "apic: APIC base differs across CPUs");
    }

    let svr = read_reg(APIC_SVR);
    write_reg(
        APIC_SVR,
        (svr & !0xff) | APIC_SVR_ENABLE | SPURIOUS_VECTOR as u32,
    );

    serial_println!(
        "[ kernel ] apic: cpu={} lapic_id={} enabled xAPIC ({}, base_phys=0x{:016x})",
        cpu_id,
        lapic_id,
        role,
        phys
    );
}

/// x2APIC: Limine left this CPU enabled in x2APIC mode (checked by
/// `init_local_apic`). Nothing in `IA32_APIC_BASE` is touched, and no MMIO
/// page is mapped either: the xAPIC window is dead in this mode.
fn init_local_x2apic(cpu_id: u32, lapic_id: u32, role: &str) {
    let svr = read_reg(APIC_SVR);
    write_reg(
        APIC_SVR,
        (svr & !0xff) | APIC_SVR_ENABLE | SPURIOUS_VECTOR as u32,
    );

    serial_println!(
        "[ kernel ] apic: cpu={} lapic_id={} enabled x2APIC ({})",
        cpu_id,
        lapic_id,
        role
    );
}

#[allow(dead_code)] // production caller lands with Q7; test caller is feature-gated.
pub fn send_reschedule_ipi(target_cpu: u32) {
    // OT-61 C1.1 hang discriminator: count reschedule IPIs by TARGET cpu, so a
    // dump can compare "sent to CPU n" against "received on CPU n"
    // (`wake_trace::IPI_RECV`). A divergence pins the seam at APIC delivery;
    // equality moves it downstream to whether the target ever reschedules.
    // Today's `IRQ_RING3_ENTRIES` cannot answer this: it is global, and it only
    // counts entries taken *from ring 3*.
    #[cfg(feature = "neo-test")]
    crate::sched::wake_trace::note_ipi_sent(target_cpu);
    send_fixed_ipi(target_cpu, RESCHEDULE_IPI_VECTOR);
}

#[allow(dead_code)] // Used by 4b shootdown machinery; Pmap callers land in 4c.
pub fn send_tlb_shootdown_ipi(target_cpu: u32) {
    send_fixed_ipi(target_cpu, TLB_SHOOTDOWN_IPI_VECTOR);
}

/// OT-90 P3b: the STOP IPI to `target_cpu` (`cpu_stop`).
#[cfg(feature = "power-stop-cpus")]
pub fn send_stop_ipi(target_cpu: u32) {
    send_fixed_ipi(target_cpu, STOP_IPI_VECTOR);
}

/// OT-90 P3b: an NMI to `target_cpu`, for a CPU that never took the STOP
/// IPI (interrupts off too long). ICR delivery mode 100b; the vector field
/// is ignored.
#[cfg(feature = "power-stop-cpus")]
pub fn send_nmi_ipi(target_cpu: u32) {
    const ICR_DELIVERY_NMI: u32 = 0b100 << 8;
    send_ipi_low(target_cpu, ICR_LEVEL_ASSERT | ICR_DELIVERY_NMI);
}

/// Whether `cpu_id` has a registered local APIC (it was started).
#[cfg(any(feature = "power-stop-cpus", feature = "neo-test-xsave-ap-policy"))]
pub fn cpu_registered(cpu_id: u32) -> bool {
    (cpu_id as usize) < per_cpu::MAX_CPUS && lapic_id_for_cpu(cpu_id) != INVALID_LAPIC_ID
}

/// OT-90 P3b: this CPU's index, found from its own local APIC ID rather than
/// GS (a STOP IPI or NMI can land in ring 3, with the user's GS). `None` if
/// no registered CPU has this ID.
#[cfg(feature = "power-stop-cpus")]
pub fn cpu_from_own_lapic_id() -> Option<u32> {
    const APIC_ID: u64 = 0x020;
    let raw = read_reg(APIC_ID);
    let own = if mode() == ApicMode::X2Apic {
        raw
    } else {
        raw >> 24
    };
    (0..per_cpu::MAX_CPUS as u32).find(|&cpu| lapic_id_for_cpu(cpu) == own)
}

fn send_fixed_ipi(target_cpu: u32, vector: u8) {
    let target = lapic_id_for_cpu(target_cpu);
    debug_assert_ne!(
        target, INVALID_LAPIC_ID,
        "apic: target CPU has no registered LAPIC id"
    );
    if target == INVALID_LAPIC_ID {
        return;
    }

    let icr_low = ICR_LEVEL_ASSERT | vector as u32;
    if mode() == ApicMode::X2Apic {
        // A WRMSR to the x2APIC ICR is not serializing, unlike the xAPIC
        // ICR's uncached MMIO store: without a fence the target can take the
        // IPI before it can see the stores the sender made first (the queued
        // work, the shootdown request). `mfence; lfence` is the SDM-documented
        // ordering for x2APIC MSR writes.
        // SAFETY: fences only; no memory or register state is changed.
        unsafe {
            core::arch::asm!("mfence", "lfence", options(nostack, preserves_flags));
        }
        // SAFETY: the x2APIC ICR, written in x2APIC mode (fixed at init).
        unsafe {
            Msr::new(apic_logic::x2apic_msr(APIC_ICR_LOW))
                .write(apic_logic::x2apic_icr(target, icr_low));
        }
        return;
    }
    // An ID above 255 cannot be named in xAPIC mode (such a CPU is never
    // started: `smp::bring_up_aps`); masking it would hit another CPU.
    let Some((icr_high, icr_low)) = apic_logic::xapic_icr(target, icr_low) else {
        if XAPIC_IPI_REFUSED.fetch_add(1, Ordering::AcqRel) == 0 {
            serial_println!(
                "[ kernel ] apic: IPI REFUSED to lapic_id={} - above 255 in xAPIC mode",
                target
            );
        }
        return;
    };
    write_reg(APIC_ICR_HIGH, icr_high);
    write_reg(APIC_ICR_LOW, icr_low);
}

/// OT-90 P3b: [`send_fixed_ipi`]'s body for any ICR low word (the NMI's
/// delivery mode). A separate copy, so the feature-off `send_fixed_ipi` is the
/// unchanged function (review #6 item 4: feature-off binary identity).
#[cfg(feature = "power-stop-cpus")]
fn send_ipi_low(target_cpu: u32, icr_low: u32) {
    let target = lapic_id_for_cpu(target_cpu);
    debug_assert_ne!(
        target, INVALID_LAPIC_ID,
        "apic: target CPU has no registered LAPIC id"
    );
    if target == INVALID_LAPIC_ID {
        return;
    }

    if mode() == ApicMode::X2Apic {
        // A WRMSR to the x2APIC ICR is not serializing, unlike the xAPIC
        // ICR's uncached MMIO store: without a fence the target can take the
        // IPI before it can see the stores the sender made first (the queued
        // work, the shootdown request). `mfence; lfence` is the SDM-documented
        // ordering for x2APIC MSR writes.
        // SAFETY: fences only; no memory or register state is changed.
        unsafe {
            core::arch::asm!("mfence", "lfence", options(nostack, preserves_flags));
        }
        // SAFETY: the x2APIC ICR, written in x2APIC mode (fixed at init).
        unsafe {
            Msr::new(apic_logic::x2apic_msr(APIC_ICR_LOW))
                .write(apic_logic::x2apic_icr(target, icr_low));
        }
        return;
    }
    // An ID above 255 cannot be named in xAPIC mode (such a CPU is never
    // started: `smp::bring_up_aps`); masking it would hit another CPU.
    let Some((icr_high, icr_low)) = apic_logic::xapic_icr(target, icr_low) else {
        if XAPIC_IPI_REFUSED.fetch_add(1, Ordering::AcqRel) == 0 {
            serial_println!(
                "[ kernel ] apic: IPI REFUSED to lapic_id={} - above 255 in xAPIC mode",
                target
            );
        }
        return;
    };
    write_reg(APIC_ICR_HIGH, icr_high);
    write_reg(APIC_ICR_LOW, icr_low);
}

pub fn end_of_interrupt() {
    write_reg(APIC_EOI, 0);
}

/// The MSI/MSI-X address that targets the BSP's local APIC, or `None` when the
/// BSP's APIC ID does not fit an MSI address's 8-bit destination (no interrupt
/// remapping here): the device must then not be programmed, because a
/// truncated ID would deliver to another CPU or to nobody. Before any BSP id
/// is registered (no MP response) this is APIC ID 0, the pre-x2APIC constant.
pub fn msi_address_for_bsp() -> Option<u32> {
    let bsp = match lapic_id_for_cpu(0) {
        INVALID_LAPIC_ID => 0,
        id => id,
    };
    let address = apic_logic::msi_address(bsp);
    if address.is_none() {
        serial_println!(
            "[ kernel ] apic: MSI REFUSED: BSP lapic_id={} exceeds the 8-bit MSI destination ({}) and interrupt remapping is off - device left without MSI",
            bsp,
            apic_logic::MSI_MAX_DEST
        );
    }
    address
}

/// Test-only (`syscall_exit_irq_window`): fire `vector` once on this CPU,
/// `count` undivided bus cycles from now. Writing the initial count restarts
/// the countdown, cancelling one still pending. Nothing else uses the LAPIC
/// timer; the kernel tick is PIC IRQ0.
#[cfg(feature = "neo-test-syscall-exit-irq-window")]
pub fn start_oneshot_timer_for_test(vector: u8, count: u32) {
    write_reg(APIC_TIMER_DIVIDE, APIC_TIMER_DIVIDE_BY_1);
    // Timer mode bits 17-18 = 0 (one-shot), mask bit 16 = 0.
    write_reg(APIC_LVT_TIMER, vector as u32);
    write_reg(APIC_TIMER_INITIAL_COUNT, count);
}

/// `lapic-timer`: the LVT timer entry (vector, mode, mask).
#[cfg(feature = "lapic-timer")]
pub fn lapic_timer_set_lvt(value: u32) {
    write_reg(APIC_LVT_TIMER, value);
}

/// `lapic-timer`: the timer's divide configuration.
#[cfg(feature = "lapic-timer")]
pub fn lapic_timer_set_divide(value: u32) {
    write_reg(APIC_TIMER_DIVIDE, value);
}

/// `lapic-timer`: the initial count (starts the countdown; 0 stops it).
#[cfg(feature = "lapic-timer")]
pub fn lapic_timer_set_initial(count: u32) {
    write_reg(APIC_TIMER_INITIAL_COUNT, count);
}

/// `lapic-timer`: the current count.
#[cfg(feature = "lapic-timer")]
pub fn lapic_timer_current() -> u32 {
    const APIC_TIMER_CURRENT_COUNT: u64 = 0x390;
    read_reg(APIC_TIMER_CURRENT_COUNT)
}

/// `lapic-timer`: whether `vector` is in service at this local APIC (its
/// ISR bit). An interrupt the PIC delivered as ExtINT never sets it.
#[cfg(feature = "lapic-timer")]
pub fn in_service(vector: u8) -> bool {
    const APIC_ISR_BASE: u64 = 0x100;
    let reg = APIC_ISR_BASE + (vector as u64 / 32) * 0x10;
    read_reg(reg) & (1 << (vector % 32)) != 0
}

/// Test diagnostic (`ps2_keyboard`): this local APIC's (ISR, IRR) bits for
/// `vector`.
#[cfg(feature = "neo-test-ps2-keyboard")]
pub fn isr_irr_for_test(vector: u8) -> (bool, bool) {
    let word = (vector as u64 / 32) * 0x10;
    let bit = 1 << (vector % 32);
    (
        read_reg(0x100 + word) & bit != 0,
        read_reg(0x200 + word) & bit != 0,
    )
}

/// This CPU's spurious-interrupt vector register (the touchpad lane prints
/// bit 12, "EOI broadcast suppression": with it set, a level-triggered I/O
/// APIC entry's remote IRR is never cleared by the local EOI).
#[cfg(feature = "i2c-hid-touchpad")]
pub fn spurious_vector_register() -> u32 {
    read_reg(APIC_SVR)
}

/// The local APIC ID registered for `cpu_id` (`INVALID_LAPIC_ID` if none).
#[cfg(any(
    feature = "ps2-keyboard",
    feature = "i2c-hid-touchpad",
    feature = "power-stop-cpus"
))]
pub fn lapic_id_of(cpu_id: u32) -> u32 {
    lapic_id_for_cpu(cpu_id)
}

fn lapic_id_for_cpu(cpu_id: u32) -> u32 {
    let idx = cpu_id as usize;
    debug_assert!(idx < per_cpu::MAX_CPUS, "apic: cpu_id exceeds MAX_CPUS");
    if idx >= per_cpu::MAX_CPUS {
        return INVALID_LAPIC_ID;
    }
    LAPIC_IDS[idx].load(Ordering::Acquire)
}

fn reg_ptr(offset: u64) -> *mut u32 {
    let phys = APIC_BASE_PHYS.load(Ordering::Acquire);
    debug_assert_ne!(phys, 0, "apic: local APIC base not initialized");
    let virt = mm::phys_to_virt(PhysAddr::new(phys + offset));
    virt.as_mut_ptr::<u32>()
}

fn read_reg(offset: u64) -> u32 {
    if mode() == ApicMode::X2Apic {
        // SAFETY: x2APIC mode (fixed at init); `offset` is an architectural
        // register, whose x2APIC MSR is `0x800 + (offset >> 4)`.
        return unsafe { Msr::new(apic_logic::x2apic_msr(offset)).read() } as u32;
    }
    // SAFETY: APIC_BASE_PHYS is installed from IA32_APIC_BASE after mm::init,
    // and `offset` is one of the architectural xAPIC 32-bit MMIO registers.
    unsafe { read_volatile(reg_ptr(offset)) }
}

fn write_reg(offset: u64, value: u32) {
    if mode() == ApicMode::X2Apic {
        // SAFETY: as in `read_reg`. EOI must be written 0, as callers do.
        unsafe {
            Msr::new(apic_logic::x2apic_msr(offset)).write(value as u64);
        }
        return;
    }
    // SAFETY: APIC_BASE_PHYS is installed from IA32_APIC_BASE after mm::init,
    // and `offset` is one of the architectural xAPIC 32-bit MMIO registers.
    unsafe {
        write_volatile(reg_ptr(offset), value);
    }
}
