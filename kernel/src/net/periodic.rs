// SPDX-License-Identifier: MPL-2.0
//! OT-95: the periodic net tick (`net-tick`).
//!
//! `net::tick` advances every protocol clock (TCP retransmit and TIME_WAIT,
//! the ARP and NDP caches, the DHCP lease, IPv6 lifetimes, the DNS cache).
//! Before this, only driver bring-up loops and the test harness called it, so
//! on a product kernel every one of those clocks stopped once bring-up ended:
//! a TCP segment a loopback peer dropped (a full receive queue) was never
//! sent again.
//!
//! One kernel thread runs it once a second, the unit the protocol timers
//! count in (the DHCP lease and IPv6 lifetimes in seconds; TCP's 3-tick RTO
//! is RFC 1122's 3 s initial RTO). After each tick it pumps loopback egress:
//! a retransmit to a loopback peer is only queued by the tick, and is
//! delivered by a pump.
//!
//! - **Context.** Thread context, never an interrupt handler, and no
//!   governance lock (I-14(a)). The tick and the pump run with interrupts
//!   off, as the syscalls that take the same net spin locks do (SFMASK clears
//!   IF): a holder preempted IF-on would deadlock a syscall spinning for it
//!   on the same CPU (CP-37). The sleep is `tick_sleep`'s timed park, IF on.
//! - **CP-37, stated exactly (#175 items 1, 7).** The product's net-lock
//!   takers are IF-off: syscalls, this thread, `net-rx`, and the handle
//!   releases. Driver bring-up loops take them IF-on, but before this thread
//!   starts. Some *harness* scenarios take them IF-on while running; those
//!   can't be built with this feature (the `compile_error!` below). The plain
//!   net locks don't mask interrupts themselves; that is OT-97.
//! - **Bounded work (#175 item 4, Chad #189).** One tick visits at most
//!   `tcp::TICK_BUDGET` entries of each TCP table (a rotating cursor). Its
//!   flush and pump each take at most their budget (`tcp::WIRE_FLUSH_BUDGET`,
//!   `tcp::PUMP_BUDGET`) off the front of the egress queue and touch nothing
//!   else, so the whole call is bounded, not only its sends (the wire send's
//!   interface snapshot included: `identity_logic::SNAPSHOT_PASSES`); the
//!   queue admits segments up to `tcp::OUTBOUND_ADMIT` (past it one is
//!   dropped and counted, and TCP retransmits; put-backs can pass it by what
//!   was out). The ARP and NDP caches are capped. Not bounded: the syscalls'
//!   own loopback pump (`pump_loopback_egress`, unchanged), which drains
//!   until no delivery; producers on other CPUs can refill the queue during
//!   a round, so not even a round is bounded.
//! - **Feature off.** This thread, the budgets and the caps are not compiled:
//!   the protocol clocks stop after bring-up, as before. **Not everything is
//!   as before (#175 item 5):** #215's registry change is unconditional, so
//!   the device and interface registries are `IrqSafeMutex` in every kernel
//!   (desktop and image A `.text` +384 bytes, usb-host +64, default
//!   unchanged, by #215's own measurement). It fixes their mixed-IF use on
//!   `main` too (UDP-send syscalls IF-off, registration and `/net/if` IF-on).
//! - **Not in image A yet.** With the clock running the DHCP lease reaches
//!   expiry and is dropped, and nothing renews it: the client sends no
//!   renewal, and nothing receives from the wire after bring-up (OT-96). A
//!   frozen lease is better than an expired one, so image A gains this
//!   together with renewal.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::sched::kthread::{self, KthreadExitState, KthreadStopToken};

// #175 item 1 (CP-37): these harness scenarios drive TCP, DHCP, IPv6 or a NIC
// themselves with interrupts on, so with this thread ticking IF-off they can
// deadlock a CPU: the harness holds a net lock, the timer preempts it, and
// the tick spins on that lock on the same CPU. `wire-rx` requires `net-tick`,
// so this refuses those combinations for it too. Fixed by construction once
// the net locks mask interrupts themselves (OT-97).
#[cfg(any(
    feature = "neo-test-phase4-tcp-connect-stream-close",
    feature = "neo-test-phase4-dhcpv4-ipv4-udp",
    feature = "neo-test-phase4-virtio-net-smoltcp-wrap-smoke",
    feature = "neo-test-phase4-slaac-udp-ipv6-loopback",
    feature = "neo-test-phase4-ndp-ipv6-neighbor",
    feature = "neo-test-interface-identity-select",
    feature = "neo-test-dhcp-rx-real-lease"
))]
compile_error!(
    "net-tick (and wire-rx, which requires it) can't be combined with a \
     harness scenario that takes the net locks IF-on (CP-37): \
     phase4_tcp_connect_stream_close, phase4_dhcpv4_ipv4_udp, \
     phase4_virtio_net_smoltcp_wrap_smoke, phase4_slaac_udp_ipv6_loopback, \
     phase4_ndp_ipv6_neighbor, interface_identity_select, dhcp_rx_real_lease"
);

/// Timer ticks (100 Hz) between net ticks: one second.
pub const PERIOD_TIMER_TICKS: u64 = 100;

/// Net ticks this thread has run.
static TICKS_RUN: AtomicU64 = AtomicU64::new(0);

/// Test-only: a harness that drives `net::tick` itself and counts what it
/// sends (`tcp_listen_bounds`) holds the thread off meanwhile.
#[cfg(feature = "neo-test")]
static PAUSED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Test-only: hold the thread's ticks off (`true`) or let them run.
#[cfg(feature = "neo-test")]
pub fn pause_for_test(paused: bool) {
    PAUSED.store(paused, Ordering::SeqCst);
}

/// Start the net tick thread. After the scheduler is up, and after any
/// driver's bring-up loop has stopped calling `net::tick` itself.
pub fn start() {
    let id = kthread::spawn_detached("net-tick", run);
    crate::serial_println!(
        "[ net ] tick: started (kthread slot={}) every {} timer ticks",
        id.slot(),
        PERIOD_TIMER_TICKS
    );
}

fn run(stop: KthreadStopToken) -> KthreadExitState {
    while !stop.is_stop_requested() {
        crate::sched::tick_sleep::sleep_ticks(PERIOD_TIMER_TICKS);
        #[cfg(feature = "neo-test")]
        if PAUSED.load(Ordering::SeqCst) {
            continue;
        }
        // CP-37: the syscalls take these plain spin locks with IF=0
        // (SYSCALL's SFMASK clears IF). Taken IF-on here, a holder preempted
        // by the tick would deadlock a syscall spinning for it on this CPU.
        crate::arch::x86_64::without_interrupts(|| {
            super::tick();
            // #175 item 4: bounded, as the tick's own tables and flush are.
            super::tcp::pump_loopback_egress_some(super::tcp::PUMP_BUDGET);
        });
        TICKS_RUN.fetch_add(1, Ordering::Relaxed);
    }
    KthreadExitState::Success
}

/// Net ticks run so far.
pub fn ticks_run() -> u64 {
    TICKS_RUN.load(Ordering::Relaxed)
}
