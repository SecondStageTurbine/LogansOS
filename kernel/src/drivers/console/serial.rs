// SPDX-License-Identifier: MPL-2.0
//! 16550 UART serial driver.
//!
//! Used for kernel debug output before — and after — the framebuffer is
//! available. QEMU forwards COM1 to stdout via `-serial stdio`, so every
//! `serial_println!` shows up in your terminal. Hardware boards typically
//! expose a serial console too; this is the same code path.
//!
//! Phase 0 uses this for boot diagnostics. Phase 1 will keep it as the
//! kernel debug log even after we have a graphical console, the same way
//! Linux keeps `dmesg` going forever.

use core::fmt::{self, Write};
#[cfg(any(feature = "fb-progress-console", feature = "power-stop-cpus"))]
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use uart_16550::SerialPort;

/// COM1 base I/O port.
const COM1: u16 = 0x3F8;

/// The single serial port the kernel uses. Wrapped in a spin-mutex so
/// we can call `serial_println!` from anywhere — including interrupt
/// handlers — without tearing.
static SERIAL: Mutex<Option<SerialPort>> = Mutex::new(None);

/// OT-90 P3b: set once K4 has stopped the other CPUs. Printing then goes to
/// the UART (and the fb console, which only tries its locks) and never to
/// the log sink, whose lock a parked CPU may hold.
#[cfg(feature = "power-stop-cpus")]
static STOPPED_WORLD: AtomicBool = AtomicBool::new(false);

/// OT-90 P3b: after K4. `nmi_parked`: a CPU was parked by NMI, so it may
/// hold the UART lock too; the lock is then forced (its holder never runs
/// again).
#[cfg(feature = "power-stop-cpus")]
pub fn enter_stopped_world(nmi_parked: bool) {
    STOPPED_WORLD.store(true, Ordering::Release);
    if nmi_parked && SERIAL.is_locked() {
        // SAFETY: the holder is a CPU halted for good (K4's NMI park); no
        // other CPU runs.
        unsafe { SERIAL.force_unlock() };
    }
}

/// Whether K4 has stopped the other CPUs (`log::flush` skips the sink then).
#[cfg(feature = "power-stop-cpus")]
pub fn stopped_world() -> bool {
    STOPPED_WORLD.load(Ordering::Acquire)
}

/// Set by the panic handler (`enter_panic`): from then on `_print` never
/// waits for `SERIAL`.
#[cfg(feature = "fb-progress-console")]
static PANICKING: AtomicBool = AtomicBool::new(false);
/// The UART has been initialized (raw panic writes need that).
#[cfg(feature = "fb-progress-console")]
static READY: AtomicBool = AtomicBool::new(false);

/// Initialize the serial port. Must be called exactly once, very early
/// in `_start`, before any `serial_println!`.
pub fn init() {
    let mut port = unsafe { SerialPort::new(COM1) };
    port.init();
    *SERIAL.lock() = Some(port);
    #[cfg(feature = "fb-progress-console")]
    READY.store(true, Ordering::Release);
}

/// Panic path (Janus F4): from here on every print goes to the screen FIRST
/// and never blocks on `SERIAL`, whose holder may be the CPU that panicked.
/// Called once by the panic handler, after the console takeover.
#[cfg(feature = "fb-progress-console")]
pub fn enter_panic() {
    PANICKING.store(true, Ordering::Release);
}

/// A print on the panic path: the screen mirror first, then the UART through
/// `SERIAL` if it is free, else directly (an interleaved byte with another
/// CPU is the price; waiting could be forever). The log sink only ever
/// try-locks.
#[cfg(feature = "fb-progress-console")]
fn print_panicking(args: fmt::Arguments) {
    crate::drivers::fb::progress::write_fmt(args);
    crate::arch::x86_64::without_interrupts(|| {
        let guard = if panic_serial_waits() {
            Some(SERIAL.lock())
        } else {
            SERIAL.try_lock()
        };
        match guard {
            Some(mut guard) => {
                if let Some(port) = guard.as_mut() {
                    let _ = port.write_fmt(args);
                }
            }
            None if READY.load(Ordering::Acquire) => {
                // SAFETY: COM1 was initialized by `init`; on the panic path a
                // second, lock-free handle only writes the transmit register.
                let mut port = unsafe { SerialPort::new(COM1) };
                let _ = port.write_fmt(args);
            }
            None => {}
        }
    });
    crate::log::write_fmt_panicking(args);
}

/// `fb_panic_holding_serial` control: the panic path waits for `SERIAL` (the
/// pre-fix behaviour, which hangs when the panicking CPU holds it).
#[cfg(feature = "fb-progress-console")]
const fn panic_serial_waits() -> bool {
    cfg!(feature = "neo-test-fb-panic-holding-serial-control")
}

/// `fb_panic_holding_serial`: panic while this CPU holds `SERIAL`, as a CPU
/// that panicked mid-print does. The guard is never released.
#[cfg(feature = "neo-test-fb-panic-holding-serial")]
pub fn panic_holding_lock_for_test() -> ! {
    let guard = SERIAL.lock();
    core::mem::forget(guard);
    panic!("fb_panic_holding_serial: panicked holding SERIAL");
}

/// Whether `SERIAL` is held (by anyone).
#[cfg(feature = "neo-test-fb-panic-holding-serial")]
pub fn locked_for_test() -> bool {
    SERIAL.is_locked()
}

/// Write a formatted string to the serial port. If the port hasn't been
/// initialized yet, the bytes are silently dropped — this is intentional
/// so that very-early panics don't recurse.
pub fn _print(args: fmt::Arguments) {
    #[cfg(feature = "fb-progress-console")]
    if PANICKING.load(Ordering::Acquire) {
        print_panicking(args);
        return;
    }
    #[cfg(feature = "power-stop-cpus")]
    if STOPPED_WORLD.load(Ordering::Acquire) {
        crate::arch::x86_64::without_interrupts(|| {
            if let Some(port) = SERIAL.lock().as_mut() {
                let _ = port.write_fmt(args);
            }
        });
        #[cfg(feature = "fb-progress-console")]
        crate::drivers::fb::progress::write_fmt(args);
        return;
    }
    crate::arch::x86_64::without_interrupts(|| {
        if let Some(port) = SERIAL.lock().as_mut() {
            // `write_fmt` on SerialPort is infallible in practice (writes go
            // to a hardware FIFO), so we ignore the Result.
            let _ = port.write_fmt(args);
        }
    });
    // OT-61 C1.1: feed the silence watchdog. "No panic, total silence" is the
    // shape of the threads_futex hang, so the absence of output is the most
    // general trigger available for a self-describing state dump.
    #[cfg(feature = "neo-test")]
    crate::sched::wake_trace::note_output();
    crate::log::write_fmt(args);
    // Zenbook lane: the screen is the log on a target with no serial port.
    #[cfg(feature = "fb-progress-console")]
    crate::drivers::fb::progress::write_fmt(args);
}

/// Serial-ONLY formatted write, safe to call from an interrupt handler.
///
/// `_print` also feeds `crate::log::write_fmt`, whose block-device sink takes a
/// plain spin lock with interrupts **enabled** (`log::block_sink::write_fmt` →
/// `SINK.lock()`). A diagnostic printed from the timer ISR through that path
/// would self-deadlock the moment a tick landed while this same CPU already
/// held the sink lock. This variant touches only COM1, inside
/// `without_interrupts` — the identical discipline `_print` already uses for the
/// UART itself, which is why `serial_println!` is documented as callable from
/// interrupt handlers. QEMU forwards COM1 to the captured serial log, which is
/// exactly where a gate diagnostic has to land.
///
/// `neo-test`-gated: no production kernel path prints from an ISR. OT-90's
/// power paths (`power-control`) use it too, with interrupts disabled for the
/// register writes.
#[cfg(any(feature = "neo-test", feature = "power-control"))]
pub fn _print_isr(args: fmt::Arguments) {
    crate::arch::x86_64::without_interrupts(|| {
        if let Some(port) = SERIAL.lock().as_mut() {
            let _ = port.write_fmt(args);
        }
    });
    // The dump's own output re-arms the silence window, which is what makes
    // successive dumps self-space instead of repeating every tick.
    #[cfg(feature = "neo-test")]
    crate::sched::wake_trace::note_output();
}

/// Write raw bytes to the serial port. Used by `sys_write` to forward user
/// process output verbatim — no formatting, no UTF-8 validation, just bytes.
/// Same silent-drop-before-init contract as `_print`.
pub fn write_bytes(bytes: &[u8]) {
    crate::arch::x86_64::without_interrupts(|| {
        if let Some(port) = SERIAL.lock().as_mut() {
            for &b in bytes {
                port.send(b);
            }
        }
    });
    // Userland output counts as liveness too — the battery's own progress lines
    // arrive through here, and they are what stops when it hangs.
    #[cfg(feature = "neo-test")]
    crate::sched::wake_trace::note_output();
    if let Ok(s) = core::str::from_utf8(bytes) {
        crate::log::write_fmt(format_args!("{}", s));
    }
}

/// Try to read one byte from COM1 without blocking. Used by the default-off
/// serial stdin syscall path.
#[cfg(feature = "serial-stdin")]
pub fn try_read_byte() -> Option<u8> {
    crate::arch::x86_64::without_interrupts(|| {
        let mut serial = SERIAL.lock();
        let port = serial.as_mut()?;
        // SAFETY: `uart_16550` performs the 16550 PIO/MMIO-equivalent RX read
        // only after its line-status check reports a queued byte; the serial
        // mutex keeps the port register access serialized with writers.
        port.try_receive().ok()
    })
}

/// Like `print!` but goes to the serial console.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::drivers::console::serial::_print(format_args!($($arg)*));
    };
}

/// Like `println!` but goes to the serial console.
#[macro_export]
macro_rules! serial_println {
    () => { $crate::serial_print!("\n") };
    ($($arg:tt)*) => {
        $crate::drivers::console::serial::_print(format_args!("{}\n", format_args!($($arg)*)));
    };
}

/// Like `serial_println!` but safe from an interrupt handler: COM1 only, no
/// block-device log sink (see [`crate::drivers::console::serial::_print_isr`]).
#[cfg(any(feature = "neo-test", feature = "power-control"))]
#[macro_export]
macro_rules! serial_println_isr {
    () => { $crate::drivers::console::serial::_print_isr(format_args!("\n")) };
    ($($arg:tt)*) => {
        $crate::drivers::console::serial::_print_isr(format_args!("{}\n", format_args!($($arg)*)));
    };
}
