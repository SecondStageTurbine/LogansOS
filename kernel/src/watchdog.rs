// SPDX-License-Identifier: MPL-2.0
//! The freeze watchdog overlay (feature `freeze-watchdog`, product ISO only;
//! `Plans/NOTE-2026-10-22-zenbook-ask-freeze.md`).
//!
//! After the desktop starts, the boot console is gone, so a freeze leaves no
//! evidence. This draws a small status block in the bottom-left corner of the
//! screen, once a second, **from an AP**: the highest-numbered online CPU,
//! from its idle loop (`run_ap_bootstrap`). It never takes a lock and never
//! needs CPU 0, so it keeps drawing when CPU 0 is stuck with interrupts
//! masked, when the WM is blocked, or when a lock is held forever. What it
//! shows:
//!
//! - `t=` seconds since the overlay started, and which CPU drew it (a stale
//!   `t` in a photo means the drawing AP itself stopped);
//! - CPU 0's timer ticks in the last second (`ticks/s=0` ⇒ CPU 0 is running
//!   with interrupts masked, or its timer stopped);
//! - how many APs went round their idle loop in the last second;
//! - per process (up to 8): the last syscall it entered and how long ago, so
//!   a process stuck in one syscall shows a large age;
//! - the model load and generation phase, and the decode step;
//! - forward-matmul jobs and whether the BSP is waiting on a partition;
//! - USB input: xHCI interrupts, keyboard and mouse reports;
//! - the startup timeline (`watchdog_timeline_logic`): in timer ticks since
//!   boot, when the keyboard route came up, when keys were produced, dropped
//!   and first taken, when Arthur was spawned and got its model handle.
//!
//! The pixels are written straight into the framebuffer (the Software
//! present backend's scanout). The compositor may overwrite the block; it is
//! redrawn every second. Feature off: none of this is compiled.

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::arch::x86_64::per_cpu::MAX_CPUS;

// --- Framebuffer, published once at init (no lock on the drawing path) ----------

static FB_BASE: AtomicUsize = AtomicUsize::new(0);
static FB_WIDTH: AtomicUsize = AtomicUsize::new(0);
static FB_HEIGHT: AtomicUsize = AtomicUsize::new(0);
static FB_PITCH: AtomicUsize = AtomicUsize::new(0);

/// Called by `drivers::fb::init` with the Limine framebuffer geometry.
pub fn publish_framebuffer(base: usize, width: usize, height: usize, pitch_bytes: usize) {
    FB_WIDTH.store(width, Ordering::Relaxed);
    FB_HEIGHT.store(height, Ordering::Relaxed);
    FB_PITCH.store(pitch_bytes, Ordering::Relaxed);
    FB_BASE.store(base, Ordering::Release);
}

// --- Probes (each a relaxed atomic store at the call site) ------------------------

/// Per-CPU idle-loop iterations (APs) — the heartbeat.
static BEATS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// The last syscall each process entered, by `pid % PROCS`.
const PROCS: usize = 8;
static PROC_PID: [AtomicU64; PROCS] = [const { AtomicU64::new(0) }; PROCS];
static PROC_NR: [AtomicU64; PROCS] = [const { AtomicU64::new(0) }; PROCS];
static PROC_TSC: [AtomicU64; PROCS] = [const { AtomicU64::new(0) }; PROCS];
#[cfg(feature = "prefill-perf")]
static PROC_CPU: [AtomicU32; PROCS] = [const { AtomicU32::new(0) }; PROCS];

/// Model load / generation phase (see `Phase`), the decode step, and the
/// weight being built.
static PHASE: AtomicU32 = AtomicU32::new(0);
static STEP: AtomicU64 = AtomicU64::new(0);
static STEP_MAX: AtomicU64 = AtomicU64::new(0);
static WEIGHT: AtomicU64 = AtomicU64::new(0);
static WEIGHTS: AtomicU64 = AtomicU64::new(0);
static GENERATIONS: AtomicU64 = AtomicU64::new(0);

/// Forward-matmul fork-join jobs, and the AP slot the BSP is waiting on (0:
/// not waiting).
static PAR_JOBS: AtomicU64 = AtomicU64::new(0);
static PAR_WAIT: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum Phase {
    Idle = 0,
    LoadHash = 1,
    LoadBuild = 2,
    LoadDone = 3,
    Prefill = 4,
    Decode = 5,
    GenDone = 6,
}

fn phase_name(raw: u32) -> &'static str {
    match raw {
        0 => "idle",
        1 => "load-hash",
        2 => "load-build",
        3 => "load-done",
        4 => "prefill",
        5 => "decode",
        6 => "gen-done",
        _ => "?",
    }
}

pub fn set_phase(phase: Phase) {
    PHASE.store(phase as u32, Ordering::Relaxed);
    if matches!(phase, Phase::Prefill) {
        GENERATIONS.fetch_add(1, Ordering::Relaxed);
        STEP.store(0, Ordering::Relaxed);
    }
}

pub fn set_step(step: u64, max: u64) {
    STEP_MAX.store(max, Ordering::Relaxed);
    STEP.store(step, Ordering::Relaxed);
}

pub fn set_weight(index: u64, count: u64) {
    WEIGHTS.store(count, Ordering::Relaxed);
    WEIGHT.store(index, Ordering::Relaxed);
}

pub fn note_par_job() {
    PAR_JOBS.fetch_add(1, Ordering::Relaxed);
}

/// `slot` is the AP partition the BSP is spinning on; 0 when it stops.
pub fn note_par_wait(slot: u32) {
    PAR_WAIT.store(slot, Ordering::Relaxed);
}

fn rdtsc() -> u64 {
    // SAFETY: RDTSC is unprivileged and side-effect free.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Syscall entry (every syscall, every CPU).
/// Returns the caller's pid, so the dispatch's other hooks need not look it
/// up again (no lock of their own).
pub fn note_syscall(nr: u64) -> u64 {
    let pid = crate::sched::current_pid().0 as u64;
    let slot = (pid as usize) % PROCS;
    PROC_PID[slot].store(pid, Ordering::Relaxed);
    PROC_NR[slot].store(nr, Ordering::Relaxed);
    PROC_TSC[slot].store(rdtsc(), Ordering::Relaxed);
    // Which CPU it was on, and the surface-commit / present timeline.
    #[cfg(feature = "prefill-perf")]
    {
        PROC_CPU[slot].store(
            crate::arch::x86_64::per_cpu::current_cpu_id(),
            Ordering::Relaxed,
        );
        crate::ai::prefill_perf::note_syscall(pid, nr);
    }
    pid
}

// --- The startup timeline (ticks since boot; 0 = not yet) ----------------------------

static ROUTES: AtomicU64 = AtomicU64::new(0);
static ROUTE_FIRST: AtomicU64 = AtomicU64::new(0);
static ROUTE_LAST: AtomicU64 = AtomicU64::new(0);
static KEYS: AtomicU64 = AtomicU64::new(0);
static KEY_FIRST: AtomicU64 = AtomicU64::new(0);
static KEYS_DROPPED: AtomicU64 = AtomicU64::new(0);
static RX_FIRST: AtomicU64 = AtomicU64::new(0);
static ARTHUR: AtomicU64 = AtomicU64::new(0);
static MODEL: AtomicU64 = AtomicU64::new(0);
/// The latest vocab open (a `/vocab/` path) and its close, with the fd and
/// pid that opened it.
static VOCAB_OPEN: AtomicU64 = AtomicU64::new(0);
static VOCAB_CLOSED: AtomicU64 = AtomicU64::new(0);
static VOCAB_FD: AtomicU64 = AtomicU64::new(u64::MAX);
static VOCAB_PID: AtomicU64 = AtomicU64::new(0);

fn now_mark() -> u64 {
    crate::watchdog_timeline_logic::mark(crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed))
}

fn mark_first(at: &AtomicU64) {
    let _ = at.compare_exchange(0, now_mark(), Ordering::Relaxed, Ordering::Relaxed);
}

#[cfg(feature = "atrium-compositor-g1")]
fn is_press(event: &crate::atrium::input_logic::InputEventV1) -> bool {
    use crate::atrium::input_logic::{INPUT_EVENT_KEY, KEY_PRESSED};
    event.kind == INPUT_EVENT_KEY && event.flags & KEY_PRESSED != 0
}

/// A keyboard route became the active one.
pub fn note_route_activated() {
    ROUTES.fetch_add(1, Ordering::Relaxed);
    mark_first(&ROUTE_FIRST);
    ROUTE_LAST.store(now_mark(), Ordering::Relaxed);
}

/// A keyboard driver produced `event` (presses are counted).
#[cfg(feature = "atrium-compositor-g1")]
pub fn note_key_produced(event: &crate::atrium::input_logic::InputEventV1) {
    if is_press(event) {
        KEYS.fetch_add(1, Ordering::Relaxed);
        mark_first(&KEY_FIRST);
    }
}

/// `event` was dropped before any route had it (presses are counted).
#[cfg(feature = "atrium-compositor-g1")]
pub fn note_key_dropped(event: &crate::atrium::input_logic::InputEventV1) {
    if is_press(event) {
        KEYS_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

/// A client took `event` from its route.
#[cfg(feature = "atrium-compositor-g1")]
pub fn note_key_received(event: &crate::atrium::input_logic::InputEventV1) {
    if is_press(event) {
        mark_first(&RX_FIRST);
    }
}

/// Arthur was spawned.
pub fn note_arthur_spawned() {
    ARTHUR.store(now_mark(), Ordering::Relaxed);
}

/// Process `pid` opened a vocab (`/vocab/` path) as `fd`. Lock-free: the
/// dispatch passes the pid [`note_syscall`] returned.
pub fn note_vocab_open(pid: u64, fd: u64) {
    VOCAB_PID.store(pid, Ordering::Relaxed);
    VOCAB_FD.store(fd, Ordering::Relaxed);
    VOCAB_CLOSED.store(0, Ordering::Relaxed);
    VOCAB_OPEN.store(now_mark(), Ordering::Relaxed);
}

/// Process `pid` is closing `fd`: if it is the open vocab, its read is done.
/// Lock-free, like [`note_vocab_open`].
pub fn note_close(pid: u64, fd: u64) {
    if VOCAB_FD.load(Ordering::Relaxed) == fd
        && VOCAB_PID.load(Ordering::Relaxed) == pid
        && VOCAB_CLOSED.load(Ordering::Relaxed) == 0
    {
        VOCAB_CLOSED.store(now_mark(), Ordering::Relaxed);
    }
}

/// A process got a model handle (the latest, beside Arthur's latest spawn).
pub fn note_model_handle() {
    MODEL.store(now_mark(), Ordering::Relaxed);
}

fn timeline(now: u64) -> crate::watchdog_timeline_logic::Timeline {
    let get = |at: &AtomicU64| at.load(Ordering::Relaxed);
    crate::watchdog_timeline_logic::Timeline {
        now,
        routes: get(&ROUTES),
        route_first: get(&ROUTE_FIRST),
        route_last: get(&ROUTE_LAST),
        keys: get(&KEYS),
        key_first: get(&KEY_FIRST),
        keys_dropped: get(&KEYS_DROPPED),
        rx_first: get(&RX_FIRST),
        arthur: get(&ARTHUR),
        vocab_open: get(&VOCAB_OPEN),
        vocab_closed: get(&VOCAB_CLOSED),
        model: get(&MODEL),
    }
}

// --- The drawing AP ---------------------------------------------------------------

static NEXT_DRAW_TSC: AtomicU64 = AtomicU64::new(0);
static START_TSC: AtomicU64 = AtomicU64::new(0);
static LAST_TICKS: AtomicU64 = AtomicU64::new(0);
static LAST_BEATS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

fn tsc_hz() -> u64 {
    #[cfg(feature = "lapic-timer")]
    {
        let hz = crate::arch::x86_64::lapic_timer::tsc_hz();
        if hz != 0 {
            return hz;
        }
    }
    // Uncalibrated: assume 2 GHz; the overlay's seconds are then approximate
    // (and printed with `~`).
    2_000_000_000
}

fn calibrated() -> bool {
    #[cfg(feature = "lapic-timer")]
    {
        crate::arch::x86_64::lapic_timer::tsc_hz() != 0
    }
    #[cfg(not(feature = "lapic-timer"))]
    {
        false
    }
}

/// From each AP's idle loop: count the beat, and on the drawing AP redraw the
/// block once a second.
pub fn ap_tick(cpu_id: u32) {
    let index = cpu_id as usize;
    if index < MAX_CPUS {
        BEATS[index].fetch_add(1, Ordering::Relaxed);
    }
    let online = crate::arch::x86_64::smp::online_cpu_count() as u32;
    if online < 2 || cpu_id != online - 1 || FB_BASE.load(Ordering::Acquire) == 0 {
        return;
    }
    let now = rdtsc();
    if START_TSC.load(Ordering::Relaxed) == 0 {
        START_TSC.store(now, Ordering::Relaxed);
    }
    if now < NEXT_DRAW_TSC.load(Ordering::Relaxed) {
        return;
    }
    NEXT_DRAW_TSC.store(now + tsc_hz(), Ordering::Relaxed);
    draw(cpu_id, online, now);
}

/// A fixed-capacity line buffer (no allocation on the drawing path).
struct Line {
    buf: [u8; 192],
    len: usize,
}

impl Line {
    const fn new() -> Self {
        Self {
            buf: [0; 192],
            len: 0,
        }
    }
}

impl core::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            if self.len < self.buf.len() {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
        Ok(())
    }
}

fn draw(cpu_id: u32, online: u32, now: u64) {
    use core::fmt::Write as _;
    let hz = tsc_hz();
    let secs = now.saturating_sub(START_TSC.load(Ordering::Relaxed)) / hz.max(1);
    let approx = if calibrated() { "" } else { "~" };

    let ticks = crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed);
    let ticks_per_s = ticks.saturating_sub(LAST_TICKS.swap(ticks, Ordering::Relaxed));
    let mut alive = 0u32;
    for cpu in 1..(online as usize).min(MAX_CPUS) {
        let beats = BEATS[cpu].load(Ordering::Relaxed);
        if beats != LAST_BEATS[cpu].swap(beats, Ordering::Relaxed) {
            alive += 1;
        }
    }

    let mut line1 = Line::new();
    let _ = write!(
        line1,
        "WATCHDOG t={}{}s cpu{} | cpu0 ticks/s={} | aps alive={}/{} | matmul jobs={} wait={}",
        approx,
        secs,
        cpu_id,
        ticks_per_s,
        alive,
        online - 1,
        PAR_JOBS.load(Ordering::Relaxed),
        PAR_WAIT.load(Ordering::Relaxed)
    );
    // CP-81: kernel-thread sleeps that parked, that found the table full,
    // and the yield passes those made (stays 0 while every sleep parks), and
    // the USB port service's passes (about 4 a second).
    #[cfg(feature = "tick-sleep")]
    {
        let (parked, full, spins) = crate::sched::tick_sleep::counters();
        let _ = write!(
            line1,
            " | sleep parked={} full={} spins={}",
            parked, full, spins
        );
        #[cfg(feature = "usb-hotplug")]
        let _ = write!(
            line1,
            " usb loops={}",
            crate::drivers::usb::port_service_loops()
        );
    }

    let mut line2 = Line::new();
    let _ = write!(
        line2,
        "model {} weight {}/{} | gen#{} step {}/{}",
        phase_name(PHASE.load(Ordering::Relaxed)),
        WEIGHT.load(Ordering::Relaxed),
        WEIGHTS.load(Ordering::Relaxed),
        GENERATIONS.load(Ordering::Relaxed),
        STEP.load(Ordering::Relaxed),
        STEP_MAX.load(Ordering::Relaxed)
    );
    let (irqs, kbd, mouse) = input_counters();
    let _ = write!(
        line2,
        " | in: xhci_irq={} kbd={} mouse={}",
        irqs, kbd, mouse
    );
    // The built-in keyboard's keys, and key auto-repeat: repeats generated
    // (`rep`) and the keyboard's own typematic makes, dropped (`typematic`).
    #[cfg(all(feature = "ps2-keyboard", feature = "key-autorepeat"))]
    {
        let ps2 = crate::drivers::ps2::i8042::Counters::now();
        // `pre=`: bytes typed before the first keyboard route, dropped.
        let _ = write!(line2, " ps2={} pre={}", ps2.keys, ps2.boot_skipped);
    }
    #[cfg(feature = "key-autorepeat")]
    {
        let (repeats, typematic) = crate::atrium::key_repeat::counters();
        let _ = write!(line2, " rep={} typematic={}", repeats, typematic);
    }

    let mut line3 = Line::new();
    let _ = write!(line3, "sys:");
    for slot in 0..PROCS {
        let pid = PROC_PID[slot].load(Ordering::Relaxed);
        if pid == 0 {
            continue;
        }
        let age = now.saturating_sub(PROC_TSC[slot].load(Ordering::Relaxed)) / hz.max(1);
        let _ = write!(
            line3,
            " p{}={:#x}@{}{}s",
            pid,
            PROC_NR[slot].load(Ordering::Relaxed),
            approx,
            age
        );
        #[cfg(feature = "prefill-perf")]
        let _ = write!(line3, "/c{}", PROC_CPU[slot].load(Ordering::Relaxed));
    }

    let mut start = Line::new();
    let _ = crate::watchdog_timeline_logic::write_line(&mut start, &timeline(ticks));
    // The touchpad's buttons and service loop (A5/A6: clicks, the park).
    #[cfg(feature = "i2c-hid-touchpad")]
    let mut tp = Line::new();
    #[cfg(feature = "i2c-hid-touchpad")]
    let _ = write!(tp, "{}", crate::drivers::i2c_hid::touchpad::WatchdogLine);

    #[cfg(feature = "prefill-perf")]
    {
        let mut line4 = Line::new();
        let _ = crate::ai::prefill_perf::write_line(&mut line4);
        let mut line5 = Line::new();
        let _ =
            crate::ai::prefill_perf::write_timeline(&mut line5, START_TSC.load(Ordering::Relaxed));
        let mut line6 = Line::new();
        let _ = crate::ai::prefill_perf::write_cpus(&mut line6, online as usize);
        #[cfg(feature = "i2c-hid-touchpad")]
        paint(&[&line1, &line2, &line3, &start, &line4, &line5, &line6, &tp]);
        #[cfg(not(feature = "i2c-hid-touchpad"))]
        paint(&[&line1, &line2, &line3, &start, &line4, &line5, &line6]);
    }
    #[cfg(not(feature = "prefill-perf"))]
    {
        #[cfg(feature = "i2c-hid-touchpad")]
        paint(&[&line1, &line2, &line3, &start, &tp]);
        #[cfg(not(feature = "i2c-hid-touchpad"))]
        paint(&[&line1, &line2, &line3, &start]);
    }
}

fn input_counters() -> (u64, u64, u64) {
    #[cfg(feature = "usb-host")]
    let irqs = crate::drivers::usb::xhci::debug_counters().2;
    #[cfg(not(feature = "usb-host"))]
    let irqs = 0;
    #[cfg(feature = "xhci-silicon")]
    let (kbd, mouse) = crate::drivers::usb::xhci::report_counts();
    #[cfg(not(feature = "xhci-silicon"))]
    let (kbd, mouse) = (0, 0);
    (irqs, kbd, mouse)
}

const FG: u32 = 0x00ff_ff00; // yellow
const BG: u32 = 0x0000_0000;

/// Draw the lines in the bottom-left corner, glyph by glyph, straight into
/// the framebuffer.
fn paint(lines: &[&Line]) {
    let base = FB_BASE.load(Ordering::Acquire);
    let width = FB_WIDTH.load(Ordering::Relaxed);
    let height = FB_HEIGHT.load(Ordering::Relaxed);
    let pitch = FB_PITCH.load(Ordering::Relaxed);
    if base == 0 || width == 0 || height == 0 {
        return;
    }
    let scale = if height >= 1600 { 2 } else { 1 };
    let glyph_w = crate::drivers::fb::font::GLYPH_WIDTH * scale;
    let glyph_h = crate::drivers::fb::font::GLYPH_HEIGHT * scale;
    let block_h = glyph_h * lines.len();
    if block_h + 4 > height {
        return;
    }
    let top = height - block_h - 4;
    for (row, line) in lines.iter().enumerate() {
        let y0 = top + row * glyph_h;
        let columns = (width / glyph_w).min(line.len);
        for column in 0..(width / glyph_w).min(line.buf.len()) {
            let byte = if column < columns {
                line.buf[column]
            } else {
                b' '
            };
            let bitmap = crate::drivers::fb::font::glyph(byte as char);
            let x0 = column * glyph_w;
            for (gy, bits) in bitmap.iter().enumerate() {
                for sy in 0..scale {
                    let y = y0 + gy * scale + sy;
                    // SAFETY: `y < height` and every `x < width` below, so
                    // the address is inside Limine's framebuffer
                    // (base + y * pitch + x * 4), which stays mapped for the
                    // kernel's life. Racing the compositor costs only pixels.
                    let row_ptr = (base + y * pitch) as *mut u32;
                    for gx in 0..crate::drivers::fb::font::GLYPH_WIDTH {
                        let on = bits & (0x80 >> gx) != 0;
                        for sx in 0..scale {
                            let x = x0 + gx * scale + sx;
                            if x < width {
                                unsafe {
                                    core::ptr::write_volatile(
                                        row_ptr.add(x),
                                        if on { FG } else { BG },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
