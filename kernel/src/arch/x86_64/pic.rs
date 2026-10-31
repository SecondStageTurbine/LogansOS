// SPDX-License-Identifier: MPL-2.0
//! Legacy 8259 PIC bring-up.
//!
//! Phase 1.3 uses the PIC (not the APIC) because it's simpler and sufficient
//! for single-CPU. Phase 2+ may switch to APIC for SMP.
//!
//! On boot Limine leaves the PIC in its BIOS-default state: master serves
//! IRQ0..7 at vectors 0x08..0x0F, slave serves IRQ8..15 at 0x70..0x77. Those
//! vectors COLLIDE with CPU exception vectors (#DE=0, #BP=3, etc.), so a
//! spurious IRQ would invoke an exception handler with the wrong context.
//!
//! `pic::init()` does the standard ICW1..ICW4 dance to remap to a contiguous
//! 32..47 range (master=32..39, slave=40..47), then masks ALL IRQs. The
//! caller (sched::init) is responsible for unmasking IRQ0 once the scheduler
//! is ready to accept ticks.

use x86_64::instructions::port::{Port, PortWriteOnly};

const MASTER_CMD: u16 = 0x20;
const MASTER_DATA: u16 = 0x21;
const SLAVE_CMD: u16 = 0xA0;
const SLAVE_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x10;
const ICW1_ICW4: u8 = 0x01;
const ICW4_8086: u8 = 0x01;

pub const PIC_VECTOR_BASE: u8 = 32; // master vectors 32..39

/// Portable I/O delay — write to an unused diagnostic port. No-op on QEMU,
/// adequate ~1µs delay on real hardware. Needed because the PIC needs settle
/// time between consecutive writes on some chipsets.
#[inline(always)]
fn io_wait() {
    // SAFETY: port 0x80 is the BIOS POST diagnostic port; writes are harmless.
    unsafe { PortWriteOnly::<u8>::new(0x80).write(0) }
}

/// Remap both PICs to vectors 32..47 and MASK ALL IRQs. Caller must unmask
/// IRQs individually after the IDT is wired up and the scheduler is ready.
pub fn init() {
    let mut master_cmd = PortWriteOnly::<u8>::new(MASTER_CMD);
    let mut master_data = PortWriteOnly::<u8>::new(MASTER_DATA);
    let mut slave_cmd = PortWriteOnly::<u8>::new(SLAVE_CMD);
    let mut slave_data = PortWriteOnly::<u8>::new(SLAVE_DATA);
    // SAFETY: PIC port I/O during single-threaded boot. Sequence matches the
    // 8259A datasheet ICW1..ICW4 init protocol.
    unsafe {
        // ICW1: start init in cascade mode, expecting ICW4.
        master_cmd.write(ICW1_INIT | ICW1_ICW4);
        io_wait();
        slave_cmd.write(ICW1_INIT | ICW1_ICW4);
        io_wait();

        // ICW2: vector base offset.
        master_data.write(PIC_VECTOR_BASE);
        io_wait(); // master 32..39
        slave_data.write(PIC_VECTOR_BASE + 8);
        io_wait(); // slave 40..47

        // ICW3: cascade wiring. Slave is on master IRQ2 → bit 2 set on master,
        // slave told its identity is 2.
        master_data.write(4);
        io_wait();
        slave_data.write(2);
        io_wait();

        // ICW4: 8086 mode (vs MCS-80/85), normal EOI.
        master_data.write(ICW4_8086);
        io_wait();
        slave_data.write(ICW4_8086);
        io_wait();

        // Mask EVERYTHING. sched::init unmasks IRQ0 (timer) at the very end of
        // its bring-up so the first tick can never fire before SCHED is forced.
        master_data.write(0xFF);
        slave_data.write(0xFF);
    }
}

/// Unmask one IRQ on the master or slave PIC. Index 0..7 → master, 8..15 → slave.
pub fn unmask_irq(irq: u8) {
    let (port_addr, bit) = if irq < 8 {
        (MASTER_DATA, irq)
    } else {
        (SLAVE_DATA, irq - 8)
    };
    let mut port = Port::<u8>::new(port_addr);
    // SAFETY: read-modify-write of the IMR for the target PIC.
    unsafe {
        let mask = port.read();
        port.write(mask & !(1 << bit));
    }
}

/// Send end-of-interrupt to the PIC(s). Must be called from any IRQ handler
/// before iretq. For IRQs >= 8, both slave AND master need EOI.
#[inline]
pub fn end_of_interrupt(irq: u8) {
    const EOI: u8 = 0x20;
    // SAFETY: single-byte write to PIC command port.
    unsafe {
        if irq >= 8 {
            PortWriteOnly::<u8>::new(SLAVE_CMD).write(EOI);
        }
        PortWriteOnly::<u8>::new(MASTER_CMD).write(EOI);
    }
}
