// SPDX-License-Identifier: MPL-2.0
//! Programmable Interval Timer — Intel 8253/8254.
//!
//! Used by Phase 1.3 as the preemption time-source. Channel 0 fires IRQ0 on
//! the master PIC at the configured frequency. We program mode 3 (square
//! wave) so the IRQ fires at the divided rate continuously.

use x86_64::instructions::port::PortWriteOnly;

const CHANNEL_0: u16 = 0x40;
const COMMAND: u16 = 0x43;

/// Base frequency of the PIT crystal — fixed at 1.193182 MHz on every PC.
const PIT_BASE_HZ: u32 = 1_193_182;

/// Channel 0, lobyte/hibyte access, mode 3 (square wave generator), binary count.
/// Bits: SC=00 (channel 0) | RW=11 (lobyte then hibyte) | MODE=011 (mode 3) | BCD=0.
const COMMAND_BYTE: u8 = 0x36;

/// Program PIT channel 0 to fire IRQ0 at `hz` cycles per second. Caller is
/// responsible for unmasking IRQ0 on the PIC (see `pic::unmask_irq(0)`).
pub fn init(hz: u32) {
    let divisor: u32 = PIT_BASE_HZ / hz;
    assert!(
        divisor > 0 && divisor < 0x10000,
        "PIT divisor out of range for {} Hz",
        hz
    );
    let lo = (divisor & 0xff) as u8;
    let hi = ((divisor >> 8) & 0xff) as u8;

    let mut command = PortWriteOnly::<u8>::new(COMMAND);
    let mut channel_0 = PortWriteOnly::<u8>::new(CHANNEL_0);
    // SAFETY: PIT port I/O during single-threaded boot.
    unsafe {
        command.write(COMMAND_BYTE);
        channel_0.write(lo);
        channel_0.write(hi);
    }
}
