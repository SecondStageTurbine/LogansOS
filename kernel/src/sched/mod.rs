// SPDX-License-Identifier: MPL-2.0
//! Kernel-thread scheduling.
//!
//! Phase 1.2 shipped `thread` + cooperative `scheduler`.
//! Phase 1.3 adds: `on_tick` (preemption entry), `exit`/`reap` (lifetime),
//! and a smoke test that exercises both cooperative and preemptive paths.
//!
//! Phase 1.4 will add userspace + ring transitions.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::arch::x86_64::timer::TICKS;
use crate::arch::x86_64::without_interrupts;
use crate::serial_println;

pub mod cap_graph_logic;
pub mod file_cap_logic;
#[cfg(feature = "browser-runtime")]
pub mod futex;
pub mod gpu_compute_cap_logic;
pub mod gpu_submit_cap_logic;
#[cfg(any(
    feature = "browser-runtime",
    feature = "cap-rooted-open",
    feature = "installer"
))]
pub mod handle_info_logic;
pub mod kthread;
pub mod net_dial_cap_logic;
pub mod process;
pub mod scheduler;
pub mod thread;
#[cfg(feature = "tick-sleep")]
pub mod tick_sleep;
#[cfg(feature = "wait-multiplex")]
pub mod wait_any;
#[cfg(feature = "wait-multiplex")]
pub mod wait_any_logic;
/// OT-61 C1.1: cross-CPU wake-delivery tracer — the `threads_futex` hang
/// discriminator. `neo-test`-gated in full; stays armed permanently once it has
/// localised the seam (discriminators are paid-down debt, never removed).
#[cfg(feature = "neo-test")]
pub mod wake_trace;

// Gated on its one consumer (the preemptible-liveness probe in syscall.rs),
// not on `neo-test-instrumentation`: any other instrumented scenario left the
// re-export unused.
#[cfg(feature = "neo-test-inference-preemptible-liveness")]
pub use scheduler::context_switch_count_for_test;
#[cfg(feature = "neo-test-phase3-neofs-entries-park")]
pub use scheduler::debug_token_parked_kill_protected;
pub use scheduler::{
    ThreadWakeToken, WakeTokenResult, block_current_into_with_guard,
    block_current_with_token_and_guard, current_pid, defer_current_preempt_for_handoff,
    enter_syscall, exit, exit_current_if_force_killed, init_bsp, leave_syscall, on_tick, reap,
    reap_and_drain, reap_nested, run_ap_bootstrap, set_current_park_owns_state, spawn,
    try_block_current_with_token_and_guard, wake_thread, wake_thread_on_current_cpu,
    wake_thread_token, with_channel_anchor, yield_now, zombify_thread,
};

// === Smoke test: ping/pong (cooperative) + compute_hog (preempted) ===

static PING_DONE: AtomicBool = AtomicBool::new(false);
static PONG_DONE: AtomicBool = AtomicBool::new(false);
static HOG_DONE: AtomicBool = AtomicBool::new(false);
static HOG_TRIGGER: AtomicBool = AtomicBool::new(false);

const PING_PONG_ROUNDS: u32 = 5;

extern "C" fn ping_thread() -> ! {
    for n in 1..=PING_PONG_ROUNDS {
        serial_println!("[ ping ] {}", n);
        yield_now();
    }
    PING_DONE.store(true, Ordering::Release);
    exit();
}

extern "C" fn pong_thread() -> ! {
    for n in 1..=PING_PONG_ROUNDS {
        serial_println!("[ pong ] {}", n);
        yield_now();
    }
    PONG_DONE.store(true, Ordering::Release);
    exit();
}

/// Tight loop that does NOT call yield_now. The only way this thread ever
/// gives up the CPU is via the timer-driven preempt path. After
/// `HOG_TRIGGER` is set, it exits.
extern "C" fn compute_hog_thread() -> ! {
    let start_ticks = TICKS.load(Ordering::Relaxed);
    let mut announced = false;
    while !HOG_TRIGGER.load(Ordering::Acquire) {
        core::hint::spin_loop();
        if !announced {
            let cur_ticks = TICKS.load(Ordering::Relaxed);
            if cur_ticks > start_ticks + 5 {
                serial_println!(
                    "[ hog  ] preempted at least 5 ticks (start={}, now={})",
                    start_ticks,
                    cur_ticks
                );
                announced = true;
            }
        }
    }
    serial_println!("[ hog  ] HOG_TRIGGER seen, exiting");
    HOG_DONE.store(true, Ordering::Release);
    exit();
}

/// Spawn ping + pong + compute_hog, drive them to completion, reap.
/// Proves: (a) cooperative alternation still works, (b) timer preemption
/// works (the hog gives up the CPU without calling yield_now), (c) exit and
/// reap work (heap-backed thread stacks are released).
pub fn smoke_test() {
    PING_DONE.store(false, Ordering::Release);
    PONG_DONE.store(false, Ordering::Release);
    HOG_DONE.store(false, Ordering::Release);
    HOG_TRIGGER.store(false, Ordering::Release);

    // The timer is already live here. Keep setup atomic so the smoke threads
    // cannot run against a partially-spawned test cohort.
    without_interrupts(|| {
        spawn("ping", ping_thread);
        spawn("pong", pong_thread);
        spawn("hog", compute_hog_thread);
    });

    // Drive ping/pong to completion via cooperative yields. Each yield_now
    // call rotates ping/pong/hog through the queue. The hog gets preempted
    // by the timer, so this loop progresses even without it cooperating.
    while !(PING_DONE.load(Ordering::Acquire) && PONG_DONE.load(Ordering::Acquire)) {
        yield_now();
        reap();
    }

    // Tell the hog to exit, then drive it to completion.
    HOG_TRIGGER.store(true, Ordering::Release);
    while !HOG_DONE.load(Ordering::Acquire) {
        yield_now();
        reap();
    }

    // Final reap pass to catch the hog's exit.
    reap();

    let final_ticks = TICKS.load(Ordering::Relaxed);
    serial_println!(
        "[ kernel ] sched: smoke test passed ({} ping, {} pong, hog reaped, ticks={})",
        PING_PONG_ROUNDS,
        PING_PONG_ROUNDS,
        final_ticks
    );
}
