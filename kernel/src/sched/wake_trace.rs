// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.1 — cross-CPU wake-**delivery** tracer: the `threads_futex` hang
//! discriminator.
//!
//! # Why this exists
//!
//! The 4-shard `workflow_dispatch` gate reddens ~2-in-3 at `NEO_SMP=4` on
//! `threads_futex` check 3: threads 15/16 park and are never reaped, with **no
//! panic and total silence** — the worst possible evidence shape, and one the
//! CI retry then erases. Both layers that would *obviously* explain it have
//! been proven correct rather than assumed:
//!
//!   - the futex park is atomic w.r.t. a racing wake — the waiter re-checks
//!     under the `FUTEX_QUEUE` guard and that guard is released on the new
//!     stack (cross-family verified by the Codex refute-lens; all three
//!     wake-before / wake-under / wake-after windows are closed), and
//!   - the idle loop's `sti; hlt` cannot lose a reschedule IPI (the ready-queue
//!     check runs at IF=0 and the halt is atomic with re-enabling).
//!
//! So the defect is **downstream**, in cross-CPU wake *delivery*
//! (`docs/concurrency-pitfalls.md`, the CP-19…CP-28 AP-idle family) — not in
//! the futex condition logic. Guessing a "fix" against a positive proof is how
//! iterations get burned; this module makes the hang **name its own seam**.
//!
//! # What it discriminates
//!
//! The decisive question that today's silent hang cannot answer is: **were
//! threads 15/16 never woken, or woken-but-never-dispatched?** The two are
//! indistinguishable from userland and have opposite fixes:
//!
//!   - **Never woken** ⇒ at hang time they are *still in* the futex queue.
//!     `dump_waiters` lists them with their key and token CPU.
//!   - **Woken but stranded** ⇒ they are *gone* from the queue, `TOKEN_WOKEN`
//!     counted them, `READY_LEN` on their token CPU is non-zero, and that
//!     CPU's `ROTATE` count has **stopped advancing**. That is CP-15/CP-21
//!     exactly: the reschedule IPI is EOI-only, so it cannot force a
//!     reschedule on a busy target — and on this kernel **only the BSP takes
//!     timer ticks** (the timer is PIC IRQ0 vector 32; there is no LAPIC
//!     timer), so an AP running a ring-3 thread that does not syscall has *no
//!     preemption point at all*. `TICKS_SEEN` proves that per CPU rather than
//!     leaving it inferred.
//!
//! A third possibility the ring settles: a wake that was **issued but hit
//! `Stale`/`Gone`** (counted separately from `Woken`), versus a waiter that was
//! **never targeted by any wake** at all.
//!
//! # Cost, and why there is no per-event logging
//!
//! Hot paths take only `Relaxed` per-CPU counter bumps plus one packed `u64`
//! store into a fixed ring. No locks, no allocation, and deliberately **no
//! `serial_println!` per event**: at thousands of contention rounds that would
//! flood the log *and* perturb the very race being hunted (a Heisenbug we would
//! then fail to reproduce). Everything expensive happens in [`dump`], which
//! fires only when the watchdog observes the futex queue **stop changing** —
//! i.e. only on a hang.
//!
//! Fully `neo-test`-gated, so the production kernel keeps its exact previous
//! shape — the same discipline as OT-49's `IDLE_ENTERED`.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::arch::x86_64::per_cpu;

/// A per-CPU counter bank: one `AtomicU64` per CPU id, so a dump can attribute
/// every number to a CPU instead of reporting a global sum. That attribution is
/// the gap the cross-family lens flagged — today's `IRQ_RING3_ENTRIES` is global
/// *and* ring-3-only, so it cannot answer "did CPU 2 receive the IPI?".
type CpuCounters = [AtomicU64; per_cpu::MAX_CPUS];
/// Zero-initialiser template for the banks below.
///
/// `clippy::declare_interior_mutable_const` fires because a `const` holding
/// `AtomicU64` yields a *fresh* value at each use, so mutating "the const" would
/// silently mutate a temporary. That is a real trap — but not here: this is used
/// **only** to initialise the `static`s below, which is precisely the sanctioned
/// use. Each `static` gets its own copy at compile time, and every runtime access
/// goes through the `static`, never through this const.
#[allow(clippy::declare_interior_mutable_const)]
const ZERO_COUNTERS: CpuCounters = [const { AtomicU64::new(0) }; per_cpu::MAX_CPUS];

/// Reschedule IPIs **sent to** this CPU (indexed by target).
static IPI_SENT: CpuCounters = ZERO_COUNTERS;
/// Reschedule IPIs **received on** this CPU (indexed by receiver). A
/// sent-but-not-received divergence localises the seam to APIC delivery
/// (suspect 1); equality moves the seam past delivery.
static IPI_RECV: CpuCounters = ZERO_COUNTERS;
/// Timer ticks taken on this CPU. Expected to stay **0 on every AP** — the timer
/// is PIC IRQ0, delivered to the BSP only, and there is no LAPIC timer.
/// Recording it makes "APs are non-preemptible" an observed fact in the capture
/// rather than a claim in a comment.
static TICKS_SEEN: CpuCounters = ZERO_COUNTERS;
/// `rotate()` calls on this CPU — the scheduler heartbeat. A CPU with queued
/// ready work and a frozen `ROTATE` is a stranded-wake CPU.
static ROTATE: CpuCounters = ZERO_COUNTERS;
/// `sti; hlt` executions in the idle thread on this CPU.
static IDLE_HALT: CpuCounters = ZERO_COUNTERS;
/// Token parks (thread left the CPU and became wake-token-owned).
static TOKEN_PARK: CpuCounters = ZERO_COUNTERS;
/// Token wakes that found and re-readied a thread (indexed by token CPU).
static TOKEN_WOKEN: CpuCounters = ZERO_COUNTERS;
/// Token wakes that found a stale generation (indexed by token CPU).
static TOKEN_STALE: CpuCounters = ZERO_COUNTERS;
/// Token wakes that found no thread at all (indexed by token CPU).
static TOKEN_GONE: CpuCounters = ZERO_COUNTERS;
/// Ready-queue length on this CPU as of its last `rotate` — an atomic mirror,
/// stamped where the scheduler lock is already held, so a dump can read every
/// CPU's depth **without taking a remote scheduler lock** (no deadlock surface,
/// no lockdep interaction from an ISR).
static READY_LEN: CpuCounters = ZERO_COUNTERS;
/// `id.0` of the thread this CPU last dispatched.
static CURRENT_TID: CpuCounters = ZERO_COUNTERS;
/// 1 when the thread this CPU last dispatched belongs to a *user* process
/// (ring-3 capable). A busy user thread on a tickless AP is the CP-21 stranding
/// precondition.
static CURRENT_IS_USER: CpuCounters = ZERO_COUNTERS;

/// Packed event ring. Fixed size, lock-free, overwrite-oldest: it holds the
/// last `RING` park/wake events, which is the *sequence* immediately before the
/// hang — the part a counter snapshot cannot show. Sized so the whole ring is
/// one serial burst at dump time.
const RING: usize = 96;
static EVENTS: [AtomicU64; RING] = [const { AtomicU64::new(0) }; RING];
/// Monotonic event sequence. Also tells the dump how many events were dropped
/// (`SEQ - RING`) so a truncated window is never mistaken for a quiet one.
static SEQ: AtomicUsize = AtomicUsize::new(0);

/// Event kinds. Kept to the wake/park lifecycle: IPI, tick and rotate volumes
/// are far too high for a 96-entry ring and are covered by the counters.
pub mod kind {
    pub const PARK: u64 = 1;
    /// A `futex_wake`/join-wake selected this waiter under the queue guard.
    pub const WAKE_PICK: u64 = 2;
    pub const WOKEN: u64 = 3;
    pub const STALE: u64 = 4;
    pub const GONE: u64 = 5;
    /// A timer-driven deadline expiry woke this waiter.
    pub const TIMEOUT: u64 = 6;

    pub fn name(k: u64) -> &'static str {
        match k {
            PARK => "park",
            WAKE_PICK => "wake-pick",
            WOKEN => "woken",
            STALE => "stale",
            GONE => "gone",
            TIMEOUT => "timeout",
            _ => "?",
        }
    }
}

/// Pack one event into a `u64`:
/// `kind:4 | cpu:8 | tcpu:8 | slot:8 | tid:16 | key:20`.
///
/// `tid` and `key` are truncated to their low bits — ample to tell 14/15/16
/// apart and to distinguish futex keys from `join_key`s, while keeping the
/// hot-path cost to a single store.
fn pack(k: u64, cpu: u32, tcpu: u32, slot: u32, tid: u64, key: u64) -> u64 {
    ((k & 0xf) << 60)
        | ((cpu as u64 & 0xff) << 52)
        | ((tcpu as u64 & 0xff) << 44)
        | ((slot as u64 & 0xff) << 36)
        | ((tid & 0xffff) << 20)
        | (key & 0xf_ffff)
}

/// Record one wake/park lifecycle event. Single relaxed fetch_add plus one
/// relaxed store — safe from any context, including an ISR.
pub fn note_event(k: u64, tcpu: u32, slot: u32, tid: u64, key: u64) {
    let cpu = per_cpu::current_cpu_id();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    EVENTS[seq % RING].store(pack(k, cpu, tcpu, slot, tid, key), Ordering::Relaxed);
}

#[inline]
fn bump(bank: &CpuCounters, cpu: u32) {
    if let Some(c) = bank.get(cpu as usize) {
        c.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn note_ipi_sent(target_cpu: u32) {
    bump(&IPI_SENT, target_cpu);
}

pub fn note_ipi_recv() {
    bump(&IPI_RECV, per_cpu::current_cpu_id());
}

/// CP-68 discriminator: reschedule IPIs received on `cpu` so far.
#[cfg(feature = "neo-test-x2apic-smp")]
pub fn ipi_recv_count(cpu: u32) -> u64 {
    IPI_RECV
        .get(cpu as usize)
        .map_or(0, |c| c.load(Ordering::Relaxed))
}

pub fn note_tick() {
    bump(&TICKS_SEEN, per_cpu::current_cpu_id());
}

pub fn note_idle_halt() {
    bump(&IDLE_HALT, per_cpu::current_cpu_id());
}

pub fn note_token_park(token_cpu: u32) {
    bump(&TOKEN_PARK, token_cpu);
}

pub fn note_token_woken(token_cpu: u32) {
    bump(&TOKEN_WOKEN, token_cpu);
}

pub fn note_token_stale(token_cpu: u32) {
    bump(&TOKEN_STALE, token_cpu);
}

pub fn note_token_gone(token_cpu: u32) {
    bump(&TOKEN_GONE, token_cpu);
}

/// Stamp this CPU's dispatch state. Called from `rotate_compute`, which already
/// holds the per-CPU scheduler lock, so these mirrors are consistent with the
/// queue they describe and readable later without any lock.
pub fn note_rotate(cpu: u32, ready_len: usize, current_tid: u64, current_is_user: bool) {
    bump(&ROTATE, cpu);
    let idx = cpu as usize;
    if idx >= per_cpu::MAX_CPUS {
        return;
    }
    READY_LEN[idx].store(ready_len as u64, Ordering::Relaxed);
    CURRENT_TID[idx].store(current_tid, Ordering::Relaxed);
    CURRENT_IS_USER[idx].store(u64::from(current_is_user), Ordering::Relaxed);
}

/// A CPU is considered present for dump purposes once it has shown any activity
/// at all. Avoids printing 64 rows of zeros for a 4-CPU gate run.
fn cpu_active(idx: usize) -> bool {
    ROTATE[idx].load(Ordering::Relaxed) != 0
        || TICKS_SEEN[idx].load(Ordering::Relaxed) != 0
        || IPI_RECV[idx].load(Ordering::Relaxed) != 0
        || IPI_SENT[idx].load(Ordering::Relaxed) != 0
        || TOKEN_PARK[idx].load(Ordering::Relaxed) != 0
}

/// Dump the full cross-CPU wake-delivery state. Serial-only (see
/// [`crate::drivers::console::serial::_print_isr`]) so it is safe from the
/// timer ISR: the block-device log sink takes a plain spin lock with interrupts
/// *enabled* and would self-deadlock if a tick landed while the BSP held it.
pub fn dump(tag: &str) {
    let seq = SEQ.load(Ordering::Relaxed);
    crate::serial_println_isr!("[wake-trace] ==== {} (events={}) ====", tag, seq);
    crate::serial_println_isr!(
        "[wake-trace] cpu rotate ticks ipi_s ipi_r halt park woken stale gone rdy cur user"
    );
    for idx in 0..per_cpu::MAX_CPUS {
        if !cpu_active(idx) {
            continue;
        }
        crate::serial_println_isr!(
            "[wake-trace] {:>3} {:>6} {:>5} {:>5} {:>5} {:>4} {:>4} {:>5} {:>5} {:>4} {:>3} {:>3} {:>4}",
            idx,
            ROTATE[idx].load(Ordering::Relaxed),
            TICKS_SEEN[idx].load(Ordering::Relaxed),
            IPI_SENT[idx].load(Ordering::Relaxed),
            IPI_RECV[idx].load(Ordering::Relaxed),
            IDLE_HALT[idx].load(Ordering::Relaxed),
            TOKEN_PARK[idx].load(Ordering::Relaxed),
            TOKEN_WOKEN[idx].load(Ordering::Relaxed),
            TOKEN_STALE[idx].load(Ordering::Relaxed),
            TOKEN_GONE[idx].load(Ordering::Relaxed),
            READY_LEN[idx].load(Ordering::Relaxed),
            CURRENT_TID[idx].load(Ordering::Relaxed),
            CURRENT_IS_USER[idx].load(Ordering::Relaxed),
        );
    }

    // The event window, oldest first. `dropped` is reported explicitly so a
    // truncated ring is never read as "nothing else happened".
    let dropped = seq.saturating_sub(RING);
    crate::serial_println_isr!(
        "[wake-trace] last {} events (dropped {}): kind cpu->tcpu slot tid key",
        seq.min(RING),
        dropped
    );
    let start = seq.saturating_sub(RING);
    for i in start..seq {
        let e = EVENTS[i % RING].load(Ordering::Relaxed);
        let k = (e >> 60) & 0xf;
        if k == 0 {
            continue;
        }
        crate::serial_println_isr!(
            "[wake-trace] #{} {} cpu{}->cpu{} slot{} tid{} key{:#x}",
            i,
            kind::name(k),
            (e >> 52) & 0xff,
            (e >> 44) & 0xff,
            (e >> 36) & 0xff,
            (e >> 20) & 0xffff,
            e & 0xf_ffff,
        );
    }

    // Who is still parked? This is the single most decisive line in the dump:
    // a waiter STILL LISTED here was never woken; an absent waiter was woken
    // and is stranded somewhere in the rows above.
    #[cfg(feature = "browser-runtime")]
    super::futex::dump_waiters();

    crate::serial_println_isr!("[wake-trace] ==== end {} ====", tag);
}

/// Stagnation watchdog. Ticks are BSP-only, so this runs on the BSP.
///
/// Two independent triggers, because the first reproduction proved one is not
/// enough:
///
///  1. **Silence** — no serial output for `SILENCE_TICKS`. This is the *symptom
///     as actually observed* ("no panic, total silence"), so it catches a freeze
///     wherever it happens. The first local reproduction froze in the lifecycle
///     check with an **empty** futex queue, where trigger 2 alone stays blind —
///     which is exactly why a symptom-shaped trigger has to exist. Because the
///     dump itself writes to serial, this self-spaces: the next dump needs
///     another full silent window.
///  2. **Frozen futex queue** — non-empty and unchanged across several checks.
///     Narrower but stronger: it fires while output is still flowing elsewhere,
///     catching a waiter stuck behind a live foreground thread.
///
/// Both are bounded by `MAX_DUMPS` so nothing can flood the serial log, and both
/// are spaced so successive dumps form a *time series*: whether `ROTATE` /
/// `IPI_RECV` keep advancing across dumps is itself the discriminator between
/// "one CPU is stranded" and "the whole machine stopped".
///
/// The silence trigger is `browser-runtime`-gated on purpose. Long legitimate
/// silence is normal in other scenarios (an Arthur forward pass computes for
/// seconds), and `browser-runtime` is precisely the C1.1 family this hunt covers
/// — every one of whose scenarios emits output continuously. So this adds
/// evidence where it is needed and zero noise anywhere else.
const CHECK_INTERVAL_TICKS: u64 = 256; // ~2.56 s at 100 Hz
const STAGNANT_CHECKS_TO_DUMP: u32 = 3; // ~7.7 s frozen before the first dump
const SILENCE_TICKS: u64 = 500; // ~5 s with no serial output at all
const MAX_DUMPS: u32 = 24;

static LAST_CHECK_TICK: AtomicU64 = AtomicU64::new(0);
static LAST_FINGERPRINT: AtomicU64 = AtomicU64::new(0);
static STAGNANT: AtomicU64 = AtomicU64::new(0);
static DUMPS: AtomicU64 = AtomicU64::new(0);

/// Tick at which serial output was last produced. Stamped by the serial writers
/// (see `crate::drivers::console::serial`), read here to detect silence.
static LAST_OUTPUT_TICK: AtomicU64 = AtomicU64::new(0);

/// Called from every serial write path. One relaxed store; safe anywhere,
/// including before the scheduler exists.
pub fn note_output() {
    LAST_OUTPUT_TICK.store(
        crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
}

/// Take a dump slot if the budget allows. Keeps the two triggers from
/// collectively exceeding `MAX_DUMPS`.
fn claim_dump_slot() -> bool {
    DUMPS.fetch_add(1, Ordering::Relaxed) < u64::from(MAX_DUMPS)
}

/// OT-tick-1 step 1: the per-CPU counters and where every thread is, every
/// [`TICK_DIAG_TICKS`] (test only, `neo-test-tick-diag`).
#[cfg(feature = "neo-test-tick-diag")]
const TICK_DIAG_TICKS: u64 = 200;
#[cfg(feature = "neo-test-tick-diag")]
static TICK_DIAG_LAST: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "neo-test-tick-diag")]
fn tick_diag(now: u64) {
    if now.wrapping_sub(TICK_DIAG_LAST.load(Ordering::Relaxed)) < TICK_DIAG_TICKS {
        return;
    }
    TICK_DIAG_LAST.store(now, Ordering::Relaxed);
    for idx in 0..per_cpu::MAX_CPUS {
        if !cpu_active(idx) {
            continue;
        }
        crate::serial_println_isr!(
            "TICKDIAG t={} cpu={} ticks={} rotate={} ready_len={} cur_tid={} cur_user={}",
            now,
            idx,
            TICKS_SEEN[idx].load(Ordering::Relaxed),
            ROTATE[idx].load(Ordering::Relaxed),
            READY_LEN[idx].load(Ordering::Relaxed),
            CURRENT_TID[idx].load(Ordering::Relaxed),
            CURRENT_IS_USER[idx].load(Ordering::Relaxed),
        );
    }
    crate::sched::scheduler::tick_diag_dump(now);
}

pub fn on_tick_watchdog(now: u64) {
    #[cfg(feature = "neo-test-tick-diag")]
    tick_diag(now);
    if now.wrapping_sub(LAST_CHECK_TICK.load(Ordering::Relaxed)) < CHECK_INTERVAL_TICKS {
        return;
    }
    LAST_CHECK_TICK.store(now, Ordering::Relaxed);

    // Trigger 1: total silence. Deliberately checked FIRST — it is the symptom
    // the gate actually reports, and it subsumes the frozen-queue case whenever
    // the guest has also stopped talking.
    #[cfg(feature = "browser-runtime")]
    if now.wrapping_sub(LAST_OUTPUT_TICK.load(Ordering::Relaxed)) >= SILENCE_TICKS
        && claim_dump_slot()
    {
        dump("no serial output");
        return;
    }

    // Trigger 2: the futex queue froze while output continued elsewhere.
    #[cfg(feature = "browser-runtime")]
    let (waiters, fp) = super::futex::waiter_fingerprint();
    #[cfg(not(feature = "browser-runtime"))]
    let (waiters, fp): (usize, u64) = (0, 0);

    if waiters == 0 {
        STAGNANT.store(0, Ordering::Relaxed);
        LAST_FINGERPRINT.store(fp, Ordering::Relaxed);
        return;
    }

    if fp == LAST_FINGERPRINT.load(Ordering::Relaxed) {
        let n = STAGNANT.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= u64::from(STAGNANT_CHECKS_TO_DUMP) && claim_dump_slot() {
            dump("futex queue frozen");
        }
    } else {
        STAGNANT.store(0, Ordering::Relaxed);
        LAST_FINGERPRINT.store(fp, Ordering::Relaxed);
    }
}
