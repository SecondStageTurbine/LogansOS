// SPDX-License-Identifier: MPL-2.0
//! Kernel panic handler.
//!
//! On panic we want to: dump the panic info to serial, paint a red bar
//! across the framebuffer if it's available, then halt forever. We
//! intentionally avoid touching anything we don't fully control here —
//! a panic during paging setup must not recurse into another panic.
//!
//! Top-level placement per ADR-002: the panic handler is not a subsystem
//! and not init-time-only (panics fire at any time), so it lives at the
//! kernel src root rather than under any subsystem directory.

#[cfg(clippy)]
use core::panic::PanicInfo;
#[cfg(not(clippy))]
use {crate::serial_println, core::panic::PanicInfo};

#[cfg(all(not(feature = "neo-test"), not(clippy)))]
use {
    crate::KERNEL_READY,
    crate::arch,
    crate::drivers::fb::{self, Color},
    core::sync::atomic::Ordering,
};

#[cfg(not(clippy))]
#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    // Zenbook lane: no serial port there, so the screen must carry the
    // panic. Break the console's locks (a CPU may have died holding them)
    // and mirror the lines below even if the desktop owned the screen.
    // R1-3: only the first panic owns the screen; a second (a recursion,
    // or another CPU) halts at once.
    #[cfg(feature = "fb-progress-console")]
    // SAFETY: this is the panic path; nothing else runs to completion.
    if !unsafe { crate::drivers::fb::progress::panic_takeover() } {
        crate::arch::halt_forever();
    }
    // Janus F4: and never wait for the serial port's lock, which this CPU
    // may hold: the screen gets each line first.
    #[cfg(feature = "fb-progress-console")]
    crate::drivers::console::serial::enter_panic();

    // Serial first — it has the simplest dependency chain and is most
    // likely to actually work.
    serial_println!();
    serial_println!("================================================");
    serial_println!("!!  KERNEL PANIC  !!");
    serial_println!("{}", info);
    serial_println!("================================================");
    // The bare-metal log sink, if any: one bounded attempt to write the
    // panic out (R1 pass 2: never a blocking lock, never a wait).
    #[cfg(feature = "fb-progress-console")]
    crate::log::panic_flush();

    #[cfg(feature = "neo-test")]
    crate::test_harness::panic_failed(info);

    #[cfg(not(feature = "neo-test"))]
    {
        // If the framebuffer is up, paint a magenta strip across the bottom
        // and the panic message inside it. If not, just halt.
        if KERNEL_READY.load(Ordering::Acquire) {
            let (w, h) = fb::dimensions();
            if w > 0 && h > 0 {
                // Magenta is reserved for fatal/critical UX in the LogansOS
                // palette — this panic bar is its only sanctioned use.
                let bar_y = h.saturating_sub(80);
                fb::fill_rect(0, bar_y, w, 80, Color::ALERT_MAGENTA);
                fb::draw_string(16, bar_y + 16, "KERNEL PANIC", Color::BLACK, None);
                // With the progress console the panic text is on screen,
                // above the bar (there may be no serial port at all).
                let hint = if cfg!(feature = "fb-progress-console") {
                    "the panic message is in the log above"
                } else {
                    "see serial console for details"
                };
                fb::draw_string(16, bar_y + 40, hint, Color::BLACK, None);
            }
        }

        arch::halt_forever();
    }
}

#[cfg(clippy)]
#[panic_handler]
fn on_panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
