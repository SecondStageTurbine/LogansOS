// SPDX-License-Identifier: MPL-2.0
//! Framebuffer progress console (`fb-progress-console`): every kernel log
//! line (`serial_println!`) is also drawn on screen, at a scale legible on a
//! high-resolution panel, from early boot until the desktop session takes the
//! framebuffer. On a target without a serial port the screen, photographed,
//! is the boot log. Layout rules: `progress_logic`.
//!
//! Never blocks: every draw only *tries* the framebuffer lock and drops the
//! glyph on contention, so a print from a context that already holds it (or
//! from another CPU mid-draw) cannot deadlock. Feature off: none of this is
//! compiled and `serial::_print` is unchanged.

use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use spin::Mutex;

use super::progress_logic::{Cursor, GLYPH_H, GLYPH_W, Step, grid, scale_for, text_height};
use super::{Color, font, try_with_fb};

const FG: Color = Color::TEXT;
const BG: Color = Color::BLACK;

struct Console {
    cursor: Cursor,
    scale: usize,
    /// A row whose clear lost the framebuffer lock; retried before the next
    /// glyph on it, so the blank row under the newest line survives contention.
    pending_clear: Option<usize>,
}

static ACTIVE: AtomicBool = AtomicBool::new(false);
static CONSOLE: Mutex<Option<Console>> = Mutex::new(None);
/// Newlines in writes dropped because another CPU held the console: the next
/// writer applies them first, so a lost line never glues two lines together.
static LOST_NEWLINES: AtomicUsize = AtomicUsize::new(0);

/// Start mirroring onto the (already initialized) framebuffer.
pub fn start() {
    let (width, height) = super::dimensions();
    if width == 0 || height == 0 {
        return;
    }
    let scale = scale_for(height);
    let (cols, rows) = grid(width, text_height(height), scale);
    *CONSOLE.lock() = Some(Console {
        cursor: Cursor::new(cols, rows),
        scale,
        pending_clear: None,
    });
    ACTIVE.store(true, Ordering::Release);
    crate::serial_println!(
        "[ kernel ] fb-progress: on {}x{} scale={} grid={}x{}",
        width,
        height,
        scale,
        cols,
        rows
    );
}

/// Stop mirroring (another drawer, or the desktop session, owns the
/// framebuffer from here). On the transition it waits for a console draw in
/// flight to finish, so no console pixel lands after this returns. After a
/// panic it does nothing: the panic owns the screen (R1-3).
pub fn stop() {
    if panicking() && !cfg!(feature = "neo-test-fb-panic-other-cpu-control") {
        return;
    }
    if ACTIVE.swap(false, Ordering::AcqRel) {
        super::console_barrier();
    }
}

/// No panic yet (`PANIC_CPU`).
const NO_PANIC: u32 = u32::MAX;
/// The APIC ID of the CPU that panicked first, or `NO_PANIC`.
static PANIC_CPU: AtomicU32 = AtomicU32::new(NO_PANIC);
/// How long the takeover waits for a live holder on another CPU to finish
/// its draw and release the console's locks, in spin iterations (well over
/// a millisecond, well under a second).
const LOCK_WAIT_SPINS: u32 = 2_000_000;

/// This CPU's APIC ID from CPUID: no per-CPU data, no APIC registers, so it
/// works at any point of boot. Only called once a panic has happened.
fn cpu_apic_id() -> u32 {
    use core::arch::x86_64::{__cpuid, __cpuid_count};
    let max_leaf = __cpuid(0).eax;
    let leaf_b = if max_leaf >= 0xb {
        __cpuid_count(0xb, 0)
    } else {
        __cpuid(0)
    };
    super::progress_logic::panic_cpu_id(max_leaf, leaf_b.ebx, leaf_b.edx, __cpuid(1).ebx)
}

/// Whether a CPU has panicked.
pub fn panicking() -> bool {
    PANIC_CPU.load(Ordering::Acquire) != NO_PANIC
}

/// Whether a CPU other than this one has panicked.
fn panicked_elsewhere() -> bool {
    let cpu = PANIC_CPU.load(Ordering::Acquire);
    cpu != NO_PANIC && cpu != cpu_apic_id()
}

/// Called before any draw that takes the screen: once another CPU has
/// panicked, this CPU never draws again and halts here (R1-3: nothing else
/// stops the other CPUs, and a live one would switch the mirror off and
/// paint over the panic).
pub fn halt_if_panicked_elsewhere() {
    // `fb_panic_holding_serial` control: the pre-R1-3 behaviour, other CPUs
    // keep drawing after a panic.
    if cfg!(feature = "neo-test-fb-panic-other-cpu-control") {
        return;
    }
    if panicked_elsewhere() {
        crate::arch::halt_forever();
    }
}

/// Panic path: claim the screen for this panic, then break the console's
/// and the framebuffer's locks if a holder does not release them (a CPU that
/// died holding one, or this CPU mid-draw), and mirror again, so the panic
/// message that follows reaches the screen even if the desktop owned it.
/// Returns false on any panic after the first (here or on another CPU): the
/// caller halts at once rather than recurse.
///
/// # Safety
/// Only on the panic path, where nothing else will run to completion.
pub unsafe fn panic_takeover() -> bool {
    if PANIC_CPU
        .compare_exchange(NO_PANIC, cpu_apic_id(), Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return false;
    }
    // From here every other CPU halts at its next draw and drops its log
    // lines from the screen. A draw already in flight gets a moment to end.
    for _ in 0..LOCK_WAIT_SPINS {
        if !CONSOLE.is_locked() && !super::fb_locked() {
            break;
        }
        core::hint::spin_loop();
    }
    // SAFETY: per the caller's contract.
    unsafe {
        if CONSOLE.is_locked() {
            CONSOLE.force_unlock();
        }
        super::steal_fb();
    }
    if CONSOLE.lock().is_some() {
        ACTIVE.store(true, Ordering::Release);
    }
    true
}

/// Counts the newlines of a write that could not be drawn.
struct NewlineCounter(usize);

impl Write for NewlineCounter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0 += s.bytes().filter(|&b| b == b'\n').count();
        Ok(())
    }
}

/// Mirror one formatted log write. Called by `serial::_print`.
pub fn write_fmt(args: fmt::Arguments) {
    if !ACTIVE.load(Ordering::Acquire) {
        return;
    }
    // After a panic only the panicking CPU writes to the screen.
    if panicking() && panicked_elsewhere() {
        return;
    }
    let Some(mut guard) = CONSOLE.try_lock() else {
        let mut lost = NewlineCounter(0);
        let _ = lost.write_fmt(args);
        LOST_NEWLINES.fetch_add(lost.0, Ordering::AcqRel);
        return;
    };
    let Some(console) = guard.as_mut() else {
        return;
    };
    for _ in 0..LOST_NEWLINES.swap(0, Ordering::AcqRel) {
        let _ = console.write_str("\n");
    }
    let _ = console.write_fmt(args);
}

impl Console {
    fn clear_row(&mut self, row: usize) {
        let h = GLYPH_H * self.scale;
        let drawn = try_with_fb(|fb| {
            if ACTIVE.load(Ordering::Acquire) {
                fb.fill(0, row * h, usize::MAX, h, BG);
            }
        });
        self.pending_clear = if drawn { None } else { Some(row) };
    }
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &byte in s.as_bytes() {
            loop {
                match self.cursor.step(byte) {
                    Step::Draw { row, col } => {
                        if self.pending_clear == Some(row) && col == 0 {
                            self.clear_row(row);
                        }
                        draw_cell(row, col, byte, self.scale);
                        break;
                    }
                    Step::NewLine { clear_row } => {
                        self.clear_row(clear_row);
                        if byte == b'\n' {
                            break;
                        }
                    }
                    Step::Skip => break,
                }
            }
        }
        Ok(())
    }
}

/// One glyph, drawn only while the console is still active, checked under
/// the framebuffer lock (`stop`'s barrier relies on that).
fn draw_cell(row: usize, col: usize, byte: u8, scale: usize) {
    let bitmap = font::glyph(byte as char);
    let x0 = col * GLYPH_W * scale;
    let y0 = row * GLYPH_H * scale;
    try_with_fb(|fb| {
        if !ACTIVE.load(Ordering::Acquire) {
            return;
        }
        for (gy, bits) in bitmap.iter().enumerate() {
            for gx in 0..GLYPH_W {
                let on = (bits >> (GLYPH_W - 1 - gx)) & 1 == 1;
                fb.fill(
                    x0 + gx * scale,
                    y0 + gy * scale,
                    scale,
                    scale,
                    if on { FG } else { BG },
                );
            }
        }
        #[cfg(feature = "neo-test-fb-panic-holding-serial")]
        if panicking() {
            PANIC_CELLS.fetch_add(1, Ordering::AcqRel);
        }
    });
}

/// `fb_panic_holding_serial`: glyph cells drawn after the panic took the
/// screen.
#[cfg(feature = "neo-test-fb-panic-holding-serial")]
static PANIC_CELLS: AtomicUsize = AtomicUsize::new(0);

/// `fb_panic_holding_serial`: how many glyph cells the panic drew.
#[cfg(feature = "neo-test-fb-panic-holding-serial")]
pub fn panic_cells_for_test() -> usize {
    PANIC_CELLS.load(Ordering::Acquire)
}

/// `fb_progress_console`: whether the mirror is drawing.
#[cfg(any(
    feature = "neo-test-fb-progress",
    feature = "neo-test-fb-panic-holding-serial"
))]
pub fn active_for_test() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// `fb_progress_console`: the cursor's text row and the glyph scale.
#[cfg(any(
    feature = "neo-test-fb-progress",
    feature = "neo-test-fb-panic-holding-serial"
))]
pub fn cursor_for_test() -> Option<(usize, usize)> {
    CONSOLE
        .lock()
        .as_ref()
        .map(|c| (c.cursor.position().0, c.scale))
}
