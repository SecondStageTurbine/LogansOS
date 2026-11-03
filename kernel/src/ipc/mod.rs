// SPDX-License-Identifier: MPL-2.0
//! Phase 1.5b: synchronous-rendezvous IPC channels.
//!
//! Per ADR-004 (anonymous handles, explicit pass-at-spawn, sync rendezvous).
//! See that ADR for the design rationale; this module implements it.
//!
//! Shape:
//!   - A `Channel` has two `Endpoint`s (sides 0 and 1).
//!   - A `Process` holds `Handle { Arc<Channel>, side }` entries in its
//!     handle table; the matching peer holds the other side.
//!   - `sys_send(handle, buf, len)` and `sys_recv(handle, buf, max_len)`
//!     drive the rendezvous. Either operation can arrive first; the
//!     second arrival completes both with a single user→user copy.
//!
//! Lifetime invariants (load-bearing):
//!   - The blocked thread's `Box<Thread>` is parked INSIDE the endpoint's
//!     state variant for the duration of the wait. Reaper never sees it
//!     (reaper iterates the zombie queue; Blocked threads are invisible).
//!   - On process exit, every handle the exiting process holds is
//!     enumerated and its PEER endpoint is signalled (`close_peer`).
//!     A blocked peer is woken with `-EPIPE`; an idle peer transitions
//!     to `Closed` so its next operation returns `-EPIPE`. If the exiting
//!     process itself has a thread parked on that same channel side, the
//!     parked thread is zombified rather than woken.
//!
//! SMP retrofit: the per-channel `IrqSafeMutex<[EndpointState; 2]>` is held
//! through the block path and released by `context_switch_with_release2` after
//! the blocked thread has switched stacks. The mutex enforces IF-off acquisition
//! by type while retaining the release token needed for atomic parked-thread
//! publication against a peer completing the rendezvous on another CPU.

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::cmp::min;

use x86_64::VirtAddr;

use crate::errno::{EFAULT, EPERM, EPIPE};
use crate::mm::pmap::Pmap;
use crate::sched;
use crate::sched::process::{self, CapId, CapRights, GrantInstallError, ProcessId};
use crate::sched::thread::Thread;
use crate::sync::IrqSafeMutex;

// === Endpoint side =======================================================

/// Which side of a channel a handle owns. `Other` of `Side(0)` is `Side(1)`,
/// and vice versa.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct EndpointSide(pub u8);

impl EndpointSide {
    pub const A: EndpointSide = EndpointSide(0);
    pub const B: EndpointSide = EndpointSide(1);

    pub fn other(self) -> EndpointSide {
        EndpointSide(1 - self.0)
    }

    pub fn idx(self) -> usize {
        self.0 as usize
    }
}

// === Endpoint state machine ==============================================

/// One endpoint of a `Channel`. Either:
///   - `Idle` — no thread waiting; next arriving op (send or recv) will
///     transition to the matching `*Blocked` variant if the *peer* is
///     also Idle, OR will rendezvous with the peer's blocked op.
///   - `SendBlocked` — a sender parked here, waiting for a recv on the
///     peer side.
///   - `RecvBlocked` — a receiver parked here, waiting for a send on the
///     peer side.
///   - `Closed` — peer process exited. Any operation returns `-EPIPE`.
#[derive(Default)]
enum EndpointState {
    #[default]
    Idle,
    SendBlocked {
        buf_user_va: u64,
        len: usize,
        sender_pmap: Arc<Pmap>,
        waiter: Box<Thread>,
    },
    RecvBlocked {
        buf_user_va: u64,
        max_len: usize,
        receiver_pmap: Arc<Pmap>,
        waiter: Box<Thread>,
    },
    GrantBlocked {
        payload: GrantPayload,
        waiter: Box<Thread>,
    },
    RecvGrantBlocked {
        recipient_pid: ProcessId,
        max_rights: CapRights,
        waiter: Box<Thread>,
    },
    Closed,
}

#[derive(Copy, Clone)]
pub struct GrantPayload {
    pub grantor_pid: ProcessId,
    pub source_cap_id: CapId,
    pub offered: CapRights,
}

// === Channel =============================================================

pub struct Channel {
    /// Both endpoints' state. Indexed by `EndpointSide.idx()`. IRQ-safe by
    /// type because syscalls, process teardown, and test debug helpers all
    /// acquire this mixed-context rendezvous lock.
    state: IrqSafeMutex<[EndpointState; 2]>,
}

impl Channel {
    pub fn new() -> Arc<Self> {
        // Channel allocation is Arc-backed by design. Sub-commit 3 (commit
        // 13d7a25) attempted to migrate it to slab via Arc::from(slab_box);
        // that was half-broken (alloc traversed the slab, but Arc's drop
        // machinery returned the storage to the global allocator, not the
        // slab's free() path — the cache silently bled entries) and was
        // reverted. OT-8 (2026-07-03) resolved the fork the revert left open:
        // CHANNEL_CACHE was removed from mm/slab.rs as dead scaffolding —
        // every correct Arc-aware route is a cross-cutting change nothing
        // today justifies. See ADR-012's OT-8 amendment.
        Arc::new(Self {
            state: IrqSafeMutex::new([EndpointState::Idle, EndpointState::Idle]),
        })
    }

    /// `sys_send` body. Syscall callers keep the whole IPC handoff under
    /// `without_interrupts`; `Channel.state` itself is IRQ-safe by type.
    /// Returns bytes copied on success, negative errno on failure.
    pub fn send(
        self: &Arc<Self>,
        my_side: EndpointSide,
        buf_user_va: u64,
        len: usize,
        my_pmap: Arc<Pmap>,
    ) -> i64 {
        // `kill_ready_in_kernel` (OT-93): the held worker sends only once init
        // is parked in its recv, so the send takes the rendezvous handoff.
        #[cfg(feature = "neo-test-kill-ready-in-kernel")]
        while crate::sched::process::kill_hold_waiting_worker()
            && !matches!(
                self.state.lock()[my_side.other().idx()],
                EndpointState::RecvBlocked { .. }
            )
        {
            sched::yield_now();
        }
        let mut guard = self.state.lock();

        // First check the peer side: is a receiver already waiting?
        let peer_idx = my_side.other().idx();
        match core::mem::take(&mut guard[peer_idx]) {
            EndpointState::RecvBlocked {
                buf_user_va: dst_va,
                max_len,
                receiver_pmap,
                waiter: recv_thread,
            } => {
                let n = min(len, max_len);
                let copy_result = cross_pmap_copy(&my_pmap, buf_user_va, &receiver_pmap, dst_va, n);
                drop(guard);
                match copy_result {
                    Ok(()) => {
                        // Receiver wakes with bytes copied.
                        handoff_to_rendezvous_peer(recv_thread, n as i64);
                        // Sender (us) returns bytes sent.
                        n as i64
                    }
                    Err(()) => {
                        // Both sides see EFAULT. Receiver wakes with error.
                        handoff_to_rendezvous_peer(recv_thread, EFAULT);
                        EFAULT
                    }
                }
            }
            EndpointState::Closed => {
                // Restore Closed (mem::take left Idle).
                guard[peer_idx] = EndpointState::Closed;
                EPIPE
            }
            existing @ (EndpointState::SendBlocked { .. }
            | EndpointState::GrantBlocked { .. }
            | EndpointState::RecvGrantBlocked { .. }
            | EndpointState::Idle) => {
                // Restore peer's state (we didn't actually consume it).
                guard[peer_idx] = existing;
                // Peer is Idle or already SendBlocked (peer also waiting to
                // send — that's "two senders, no receiver", a deadlock-
                // shaped configuration; we still allow the second sender
                // to block on its OWN side and wait for a future receiver).
                //
                // We block on OUR side. Keep the channel guard held through
                // the scheduler block; Q6 asm releases channel first, then
                // scheduler, after the old thread is safely parked.
                // OT-22: a peer parked in SYS_WAIT_ANY on its side is woken
                // first, still under this guard; its re-check takes the
                // guard and so sees this sender once it has parked.
                #[cfg(feature = "wait-multiplex")]
                self.fire_waiters(my_side.other());
                sched::block_current_into_with_guard(guard, |state, prev_thread| {
                    state[my_side.idx()] = EndpointState::SendBlocked {
                        buf_user_va,
                        len,
                        sender_pmap: my_pmap,
                        waiter: prev_thread,
                    };
                })
            }
        }
    }

    /// `sys_recv` body. Syscall callers keep the whole IPC handoff under
    /// `without_interrupts`; `Channel.state` itself is IRQ-safe by type.
    /// Returns bytes copied on success, negative errno on failure.
    pub fn recv(
        self: &Arc<Self>,
        my_side: EndpointSide,
        buf_user_va: u64,
        max_len: usize,
        my_pmap: Arc<Pmap>,
    ) -> i64 {
        let mut guard = self.state.lock();

        let peer_idx = my_side.other().idx();
        match core::mem::take(&mut guard[peer_idx]) {
            EndpointState::SendBlocked {
                buf_user_va: src_va,
                len,
                sender_pmap,
                waiter: send_thread,
            } => {
                let n = min(len, max_len);
                let copy_result = cross_pmap_copy(&sender_pmap, src_va, &my_pmap, buf_user_va, n);
                drop(guard);
                match copy_result {
                    Ok(()) => {
                        // Sender wakes with bytes sent.
                        continue_current_after_receive(send_thread, n as i64);
                        // Receiver (us) returns bytes copied.
                        n as i64
                    }
                    Err(()) => {
                        continue_current_after_receive(send_thread, EFAULT);
                        EFAULT
                    }
                }
            }
            EndpointState::Closed => {
                guard[peer_idx] = EndpointState::Closed;
                EPIPE
            }
            existing @ (EndpointState::RecvBlocked { .. }
            | EndpointState::GrantBlocked { .. }
            | EndpointState::RecvGrantBlocked { .. }
            | EndpointState::Idle) => {
                guard[peer_idx] = existing;
                sched::block_current_into_with_guard(guard, |state, prev_thread| {
                    state[my_side.idx()] = EndpointState::RecvBlocked {
                        buf_user_va,
                        max_len,
                        receiver_pmap: my_pmap,
                        waiter: prev_thread,
                    };
                })
            }
        }
    }

    /// DI-1a `SYS_TRY_RECV` body (THINK §8.5): `recv`, except that when no
    /// sender is waiting it restores the endpoint and returns `EAGAIN`
    /// instead of parking. It only ever takes the existing send-blocked
    /// handoff, so it adds no park or wake edge.
    #[cfg(feature = "atrium-pointer-forward")]
    pub fn try_recv(
        self: &Arc<Self>,
        my_side: EndpointSide,
        buf_user_va: u64,
        max_len: usize,
        my_pmap: Arc<Pmap>,
    ) -> i64 {
        let mut guard = self.state.lock();
        let peer_idx = my_side.other().idx();
        match core::mem::take(&mut guard[peer_idx]) {
            // Exactly `recv`'s send-blocked handoff, under the same guard.
            EndpointState::SendBlocked {
                buf_user_va: src_va,
                len,
                sender_pmap,
                waiter: send_thread,
            } => {
                let n = min(len, max_len);
                let copy_result = cross_pmap_copy(&sender_pmap, src_va, &my_pmap, buf_user_va, n);
                drop(guard);
                match copy_result {
                    Ok(()) => {
                        continue_current_after_receive(send_thread, n as i64);
                        n as i64
                    }
                    Err(()) => {
                        continue_current_after_receive(send_thread, EFAULT);
                        EFAULT
                    }
                }
            }
            EndpointState::Closed => {
                guard[peer_idx] = EndpointState::Closed;
                EPIPE
            }
            existing => {
                guard[peer_idx] = existing;
                crate::errno::EAGAIN
            }
        }
    }

    /// OT-88 A0k `SYS_TRY_SEND` body: `send`, except that it completes only
    /// with a receiver already parked on the peer side. Anything else restores
    /// the peer's state and returns `EAGAIN` (`EPIPE` if closed); it never
    /// parks, so it adds no park, wait-queue or park-teardown edge. A WM's
    /// reply to a client that isn't receiving stays the WM's to retry.
    #[cfg(feature = "ipc-try-send")]
    pub fn try_send(
        self: &Arc<Self>,
        my_side: EndpointSide,
        buf_user_va: u64,
        len: usize,
        my_pmap: Arc<Pmap>,
    ) -> i64 {
        let mut guard = self.state.lock();
        let peer_idx = my_side.other().idx();
        match core::mem::take(&mut guard[peer_idx]) {
            // Exactly `send`'s receiver-parked handoff, under the same guard.
            EndpointState::RecvBlocked {
                buf_user_va: dst_va,
                max_len,
                receiver_pmap,
                waiter: recv_thread,
            } => {
                let n = min(len, max_len);
                let copy_result = cross_pmap_copy(&my_pmap, buf_user_va, &receiver_pmap, dst_va, n);
                drop(guard);
                match copy_result {
                    Ok(()) => {
                        handoff_to_rendezvous_peer(recv_thread, n as i64);
                        n as i64
                    }
                    Err(()) => {
                        handoff_to_rendezvous_peer(recv_thread, EFAULT);
                        EFAULT
                    }
                }
            }
            EndpointState::Closed => {
                guard[peer_idx] = EndpointState::Closed;
                EPIPE
            }
            existing => {
                guard[peer_idx] = existing;
                crate::errno::EAGAIN
            }
        }
    }

    /// OT-88 A0k `SYS_TRY_GRANT` body: `grant`, completing only with a
    /// `recv_grant` already parked on the peer side; otherwise `EAGAIN` (or
    /// `EPIPE`), nothing installed and nothing parked.
    #[cfg(feature = "ipc-try-send")]
    pub fn try_grant(self: &Arc<Self>, my_side: EndpointSide, payload: GrantPayload) -> i64 {
        let mut guard = self.state.lock();
        let peer_idx = my_side.other().idx();
        match core::mem::take(&mut guard[peer_idx]) {
            // Exactly `grant`'s receiver-parked handoff.
            EndpointState::RecvGrantBlocked {
                recipient_pid,
                max_rights,
                waiter: recv_thread,
            } => {
                let completion = process::complete_grant(
                    payload.grantor_pid,
                    payload.source_cap_id,
                    payload.offered,
                    recipient_pid,
                    max_rights,
                );
                drop(guard);
                match completion {
                    Ok(handle_idx) => {
                        handoff_to_rendezvous_peer(recv_thread, handle_idx as i64);
                        0
                    }
                    Err(GrantInstallError::EmptyIntersection) => {
                        handoff_to_rendezvous_peer(recv_thread, EPERM);
                        EPERM
                    }
                    Err(GrantInstallError::GrantorBadCap) => {
                        handoff_to_rendezvous_peer(recv_thread, EPIPE);
                        crate::errno::EBADF
                    }
                    Err(GrantInstallError::GrantorMissingGrant) => {
                        handoff_to_rendezvous_peer(recv_thread, EPIPE);
                        EPERM
                    }
                    Err(GrantInstallError::RecipientGone) => EPIPE,
                }
            }
            EndpointState::Closed => {
                guard[peer_idx] = EndpointState::Closed;
                EPIPE
            }
            existing => {
                guard[peer_idx] = existing;
                crate::errno::EAGAIN
            }
        }
    }

    /// OT-88 A0k `SYS_TRY_RECV_GRANT` body: `recv_grant`, completing only
    /// with a grantor already parked on the peer side; otherwise `EAGAIN` (or
    /// `EPIPE`), nothing installed and nothing parked. The WM receives a
    /// client's buffer and activator this way, so a client that never grants
    /// can't park it.
    #[cfg(feature = "ipc-try-send")]
    pub fn try_recv_grant(
        self: &Arc<Self>,
        my_side: EndpointSide,
        recipient_pid: ProcessId,
        max_rights: CapRights,
    ) -> i64 {
        let mut guard = self.state.lock();
        let peer_idx = my_side.other().idx();
        match core::mem::take(&mut guard[peer_idx]) {
            // Exactly `recv_grant`'s grantor-parked handoff.
            EndpointState::GrantBlocked {
                payload,
                waiter: grant_thread,
            } => {
                let completion = process::complete_grant(
                    payload.grantor_pid,
                    payload.source_cap_id,
                    payload.offered,
                    recipient_pid,
                    max_rights,
                );
                drop(guard);
                match completion {
                    Ok(handle_idx) => {
                        continue_current_after_receive(grant_thread, 0);
                        handle_idx as i64
                    }
                    Err(GrantInstallError::EmptyIntersection) => {
                        continue_current_after_receive(grant_thread, EPERM);
                        EPERM
                    }
                    Err(GrantInstallError::GrantorBadCap) => {
                        continue_current_after_receive(grant_thread, crate::errno::EBADF);
                        EPIPE
                    }
                    Err(GrantInstallError::GrantorMissingGrant) => {
                        continue_current_after_receive(grant_thread, EPERM);
                        EPIPE
                    }
                    Err(GrantInstallError::RecipientGone) => {
                        continue_current_after_receive(grant_thread, EPIPE);
                        EPIPE
                    }
                }
            }
            EndpointState::Closed => {
                guard[peer_idx] = EndpointState::Closed;
                EPIPE
            }
            existing => {
                guard[peer_idx] = existing;
                crate::errno::EAGAIN
            }
        }
    }

    /// `sys_grant` body. Syscall callers keep the whole IPC handoff under
    /// `without_interrupts`; `Channel.state` itself is IRQ-safe by type.
    /// Completes with a waiting `sys_recv_grant` or parks the grantor on its
    /// endpoint. Grant is derived-copy: donor keeps source, recipient receives
    /// a fresh CapId linked to the donor's source CapId.
    pub fn grant(self: &Arc<Self>, my_side: EndpointSide, payload: GrantPayload) -> i64 {
        let mut guard = self.state.lock();
        let peer_idx = my_side.other().idx();
        match core::mem::take(&mut guard[peer_idx]) {
            EndpointState::RecvGrantBlocked {
                recipient_pid,
                max_rights,
                waiter: recv_thread,
            } => {
                let completion = process::complete_grant(
                    payload.grantor_pid,
                    payload.source_cap_id,
                    payload.offered,
                    recipient_pid,
                    max_rights,
                );
                drop(guard);
                match completion {
                    Ok(handle_idx) => {
                        handoff_to_rendezvous_peer(recv_thread, handle_idx as i64);
                        0
                    }
                    Err(GrantInstallError::EmptyIntersection) => {
                        handoff_to_rendezvous_peer(recv_thread, EPERM);
                        EPERM
                    }
                    Err(GrantInstallError::GrantorBadCap) => {
                        handoff_to_rendezvous_peer(recv_thread, EPIPE);
                        crate::errno::EBADF
                    }
                    Err(GrantInstallError::GrantorMissingGrant) => {
                        handoff_to_rendezvous_peer(recv_thread, EPIPE);
                        EPERM
                    }
                    Err(GrantInstallError::RecipientGone) => EPIPE,
                }
            }
            EndpointState::Closed => {
                guard[peer_idx] = EndpointState::Closed;
                EPIPE
            }
            existing @ (EndpointState::SendBlocked { .. }
            | EndpointState::RecvBlocked { .. }
            | EndpointState::GrantBlocked { .. }
            | EndpointState::Idle) => {
                guard[peer_idx] = existing;
                sched::block_current_into_with_guard(guard, |state, prev_thread| {
                    state[my_side.idx()] = EndpointState::GrantBlocked {
                        payload,
                        waiter: prev_thread,
                    };
                })
            }
        }
    }

    /// `sys_recv_grant` body. Syscall callers keep the whole IPC handoff under
    /// `without_interrupts`; `Channel.state` itself is IRQ-safe by type.
    /// Returns the new handle index on success.
    pub fn recv_grant(
        self: &Arc<Self>,
        my_side: EndpointSide,
        recipient_pid: ProcessId,
        max_rights: CapRights,
    ) -> i64 {
        let mut guard = self.state.lock();
        let peer_idx = my_side.other().idx();
        match core::mem::take(&mut guard[peer_idx]) {
            EndpointState::GrantBlocked {
                payload,
                waiter: grant_thread,
            } => {
                let completion = process::complete_grant(
                    payload.grantor_pid,
                    payload.source_cap_id,
                    payload.offered,
                    recipient_pid,
                    max_rights,
                );
                drop(guard);
                match completion {
                    Ok(handle_idx) => {
                        continue_current_after_receive(grant_thread, 0);
                        handle_idx as i64
                    }
                    Err(GrantInstallError::EmptyIntersection) => {
                        continue_current_after_receive(grant_thread, EPERM);
                        EPERM
                    }
                    Err(GrantInstallError::GrantorBadCap) => {
                        continue_current_after_receive(grant_thread, crate::errno::EBADF);
                        EPIPE
                    }
                    Err(GrantInstallError::GrantorMissingGrant) => {
                        continue_current_after_receive(grant_thread, EPERM);
                        EPIPE
                    }
                    Err(GrantInstallError::RecipientGone) => {
                        continue_current_after_receive(grant_thread, EPIPE);
                        EPIPE
                    }
                }
            }
            EndpointState::Closed => {
                guard[peer_idx] = EndpointState::Closed;
                EPIPE
            }
            existing @ (EndpointState::SendBlocked { .. }
            | EndpointState::RecvBlocked { .. }
            | EndpointState::RecvGrantBlocked { .. }
            | EndpointState::Idle) => {
                guard[peer_idx] = existing;
                sched::block_current_into_with_guard(guard, |state, prev_thread| {
                    state[my_side.idx()] = EndpointState::RecvGrantBlocked {
                        recipient_pid,
                        max_rights,
                        waiter: prev_thread,
                    };
                })
            }
        }
    }

    /// OT-22: would a recv (or `try_recv`) on `my_side` complete without
    /// parking? Exactly what those consume: a sender parked on the peer side,
    /// or the channel closed (EPIPE). A parked grantor is not counted: no
    /// recv consumes it, so a wait reporting it would spin its caller.
    #[cfg(feature = "wait-multiplex")]
    pub fn recv_ready(&self, my_side: EndpointSide) -> bool {
        let guard = self.state.lock();
        matches!(
            guard[my_side.other().idx()],
            EndpointState::SendBlocked { .. } | EndpointState::Closed
        )
    }

    /// OT-22: the wait key for `side` of this channel.
    #[cfg(feature = "wait-multiplex")]
    pub fn wait_key(&self, side: EndpointSide) -> u64 {
        sched::wait_any::channel_key(self as *const Self as *const (), side.idx())
    }

    /// OT-22: wake any SYS_WAIT_ANY waiter watching `side`. Called with
    /// `self.state` held (order `Channel.state` → wait table → scheduler).
    #[cfg(feature = "wait-multiplex")]
    fn fire_waiters(&self, side: EndpointSide) {
        sched::wait_any::fire_channel(self.wait_key(side));
    }

    /// Wake a peer blocked in `sys_recv_grant` when the grantor-side syscall
    /// fails before it can create a valid GrantBlocked state.
    pub fn fail_recv_grant(self: &Arc<Self>, my_side: EndpointSide) {
        let peer_idx = my_side.other().idx();
        let mut guard = self.state.lock();
        let extracted = match core::mem::take(&mut guard[peer_idx]) {
            state @ EndpointState::RecvGrantBlocked { .. } => state,
            other => {
                guard[peer_idx] = other;
                return;
            }
        };
        drop(guard);
        if let EndpointState::RecvGrantBlocked { waiter, .. } = extracted {
            crate::arch::x86_64::without_interrupts(|| {
                handoff_to_rendezvous_peer(waiter, EPIPE);
            });
        }
    }

    #[cfg(any(
        feature = "neo-test-smp-grant-cross-cpu",
        feature = "neo-test-ipc-try-send"
    ))]
    pub fn debug_endpoint_is_grant_blocked(&self, side: EndpointSide) -> bool {
        let guard = self.state.lock();
        matches!(guard[side.idx()], EndpointState::GrantBlocked { .. })
    }

    #[cfg(feature = "neo-test-ipc-try-send")]
    pub fn debug_endpoint_is_recv_grant_blocked(&self, side: EndpointSide) -> bool {
        let guard = self.state.lock();
        matches!(guard[side.idx()], EndpointState::RecvGrantBlocked { .. })
    }

    #[cfg(any(
        feature = "neo-test-q8c-close-peer-my-side-extract",
        feature = "neo-test-smp-cross-cpu-migration-reap",
        feature = "neo-test-smp-cross-cpu-rendezvous-stress",
        feature = "neo-test-smp-c11-migration-reservation",
        feature = "neo-test-ipc-try-send"
    ))]
    pub fn debug_endpoint_is_send_blocked(&self, side: EndpointSide) -> bool {
        let guard = self.state.lock();
        matches!(guard[side.idx()], EndpointState::SendBlocked { .. })
    }

    #[cfg(any(
        feature = "neo-test-q8c-blocked-kill-walk",
        feature = "neo-test-q8c-blocked-kill-foreign-parked",
        feature = "neo-test-smp-cross-cpu-rendezvous-stress",
        feature = "neo-test-smp-c11-migration-reservation",
        feature = "neo-test-ipc-try-send"
    ))]
    pub fn debug_endpoint_is_recv_blocked(&self, side: EndpointSide) -> bool {
        let guard = self.state.lock();
        matches!(guard[side.idx()], EndpointState::RecvBlocked { .. })
    }

    #[cfg(any(
        feature = "neo-test-q8c-blocked-kill-walk",
        feature = "neo-test-q8c-blocked-kill-foreign-parked",
        feature = "neo-test-q8c-close-peer-my-side-extract",
        feature = "neo-test-smp-concurrent-reap-after-terminate"
    ))]
    pub fn debug_endpoints_are_closed(&self) -> bool {
        let guard = self.state.lock();
        matches!(guard[EndpointSide::A.idx()], EndpointState::Closed)
            && matches!(guard[EndpointSide::B.idx()], EndpointState::Closed)
    }

    /// Forced-kill helper: extract a thread parked on the caller's own
    /// endpoint side only when it belongs to `expected_pid`. The caller
    /// zombifies the returned thread, then closes channel peers through the
    /// normal process-exit path. Foreign waiters are left parked for
    /// `close_peer`, which wakes them with EPIPE.
    pub fn extract_parked_if_owned(
        &self,
        my_side: EndpointSide,
        expected_pid: ProcessId,
    ) -> Option<Box<Thread>> {
        let my_idx = my_side.idx();
        let mut guard = self.state.lock();
        extract_parked_if_owned_from_state(&mut guard[my_idx], expected_pid)
    }

    // SC-4 forward marker (Phase 2+): illumos doors model (Cerebro D5 §6)
    // is the architectural reference for cross-process IPC at NeoPlutarch's
    // scale. Durable cross-process references via capability handles, a
    // per-target liveness flag, and short locked rendezvous sections are the
    // design language to formalize as Phase 3 approaches.
    /// Process-exit cleanup: the process owning `my_side` is exiting; signal
    /// the PEER endpoint. Two cases per ADR-004 §"Process-exit cleanup":
    ///   - Peer Idle → Closed (no thread to wake).
    ///   - Peer SendBlocked / RecvBlocked → yank parked Box, wake with EPIPE.
    ///
    /// SC-4 extends this to also extract `my_side`: if a thread from the
    /// exiting process is parked on its own endpoint, it is zombified directly
    /// because it must not run again. A waiter on `my_side` is only zombified
    /// when its `pid` matches `exiting_pid` — a foreign-process waiter parked
    /// on `my_side` via a granted derivation is woken with EPIPE instead,
    /// because that thread MUST keep running (it belongs to a live process).
    /// Zombifying the wrong process's thread reaps it without running its
    /// `sys_exit`, leaving the kernel with no recorded exit code.
    ///
    /// Caller (`process::on_thread_reap`) is on the bootstrap / reaper thread.
    /// `Channel.state` enforces IRQ-safe acquisition; the wake/zombify phase is
    /// still explicitly wrapped because it touches scheduler queues.
    pub fn close_peer(&self, my_side: EndpointSide, exiting_pid: ProcessId) {
        let peer_idx = my_side.other().idx();
        let my_idx = my_side.idx();
        let mut guard = self.state.lock();
        let extracted_peer = core::mem::replace(&mut guard[peer_idx], EndpointState::Closed);
        let extracted_my_owned =
            extract_parked_if_owned_from_state(&mut guard[my_idx], exiting_pid);
        let extracted_my_foreign = core::mem::replace(&mut guard[my_idx], EndpointState::Closed);
        // OT-22: a closed side is ready (the next recv returns EPIPE).
        #[cfg(feature = "wait-multiplex")]
        {
            self.fire_waiters(my_side.other());
            self.fire_waiters(my_side);
        }
        // OT-98 (the PM's bounce, item 3): the extracted waiters are woken (or
        // zombified) under the endpoint guard, in the endpoint → scheduler
        // order the park already uses. Woken after the guard was dropped, a
        // waiter was in no queue for a while, and a kill of its process then
        // found no thread of it. The control (`neo-test-kill-at-exit-control-g`)
        // drops the guard first, as before.
        let guard = if cfg!(feature = "neo-test-kill-at-exit-control-g") {
            drop(guard);
            None
        } else {
            Some(guard)
        };
        #[cfg(feature = "neo-test-kill-at-exit")]
        if let EndpointState::SendBlocked { waiter, .. }
        | EndpointState::RecvBlocked { waiter, .. }
        | EndpointState::GrantBlocked { waiter, .. }
        | EndpointState::RecvGrantBlocked { waiter, .. } = &extracted_peer
        {
            crate::test_harness::kae_close_peer_transit(waiter.pid());
        }

        crate::arch::x86_64::without_interrupts(|| {
            match extracted_peer {
                EndpointState::SendBlocked { waiter, .. }
                | EndpointState::RecvBlocked { waiter, .. }
                | EndpointState::GrantBlocked { waiter, .. }
                | EndpointState::RecvGrantBlocked { waiter, .. } => {
                    sched::wake_thread(waiter, EPIPE);
                }
                EndpointState::Idle | EndpointState::Closed => {}
            }
            if let Some(waiter) = extracted_my_owned {
                sched::zombify_thread(waiter);
            }
            match extracted_my_foreign {
                EndpointState::SendBlocked { waiter, .. }
                | EndpointState::RecvBlocked { waiter, .. }
                | EndpointState::GrantBlocked { waiter, .. }
                | EndpointState::RecvGrantBlocked { waiter, .. } => {
                    // Foreign-process waiter parked on my_side via a
                    // granted-derivation handle. The channel is closing,
                    // but the waiter's process is still live, so wake it
                    // with EPIPE so it can observe the closure and run
                    // its own exit path.
                    sched::wake_thread(waiter, EPIPE);
                }
                EndpointState::Idle | EndpointState::Closed => {}
            }
        });
        drop(guard);
    }
}

impl Channel {
    /// CP-67 discriminator: a thread is parked on `side`.
    #[cfg(any(feature = "neo-test-ipc-park-kill", feature = "neo-test-kill-at-exit"))]
    pub fn parked_for_test(&self, side: EndpointSide) -> bool {
        parked_waiter_pid(&self.state.lock()[side.idx()]).is_some()
    }
}

fn parked_waiter_pid(state: &EndpointState) -> Option<ProcessId> {
    match state {
        EndpointState::SendBlocked { waiter, .. }
        | EndpointState::RecvBlocked { waiter, .. }
        | EndpointState::GrantBlocked { waiter, .. }
        | EndpointState::RecvGrantBlocked { waiter, .. } => Some(waiter.pid()),
        EndpointState::Idle | EndpointState::Closed => None,
    }
}

fn extract_parked_if_owned_from_state(
    state: &mut EndpointState,
    expected_pid: ProcessId,
) -> Option<Box<Thread>> {
    if parked_waiter_pid(state) != Some(expected_pid) {
        return None;
    }

    match core::mem::take(state) {
        EndpointState::SendBlocked { waiter, .. }
        | EndpointState::RecvBlocked { waiter, .. }
        | EndpointState::GrantBlocked { waiter, .. }
        | EndpointState::RecvGrantBlocked { waiter, .. } => Some(waiter),
        other => {
            *state = other;
            None
        }
    }
}

fn handoff_to_rendezvous_peer(thread: Box<Thread>, return_value: i64) {
    // `kill_ready_in_kernel` (OT-93): the first worker sender here after the
    // harness arms the hold stays Ready inside this syscall, still owning its
    // frame's pmaps, until the receiver's `SYS_KILL` of it has returned.
    #[cfg(feature = "neo-test-kill-ready-in-kernel")]
    let hold = crate::sched::process::kill_hold_take();
    // Q4: enqueue the blocked peer on the completer's current CPU. The
    // immediate yield preserves ADR-011's consumer-first handoff behavior.
    sched::wake_thread_on_current_cpu(thread, return_value);
    sched::yield_now();
    #[cfg(feature = "neo-test-kill-ready-in-kernel")]
    if hold {
        while !crate::sched::process::kill_hold_released() {
            sched::yield_now();
        }
    }
}

fn continue_current_after_receive(thread: Box<Thread>, return_value: i64) {
    // When a receiver completes a previously-blocked sender/grantor, the
    // consumer of the payload is the current thread. Let it reach the next
    // syscall (usually a follow-on block) before the producer observes it as
    // Ready again.
    sched::wake_thread_on_current_cpu(thread, return_value);
    sched::defer_current_preempt_for_handoff();
}

// === User→user copy via two Pmap translates =============================

/// Copy `n` bytes from `src_va` in `src_pmap` to `dst_va` in `dst_pmap`.
///
/// Page-by-page: each iteration copies the smaller of (remaining,
/// src-page-tail, dst-page-tail). Returns Err(()) (EFAULT to both sides) if
/// a page can't be accessed the way ring 3 would need to: unmapped, not
/// user-accessible, or (destination) read-only. Some prefix may have been
/// copied by then.
///
/// CP-99:
/// - The side whose address space is live on this CPU (the caller's, for a
///   syscall) is read or written at its user VA through the registered
///   fault-tolerant copy (`user_copy::try_copy`): a fault there, a read-only
///   page under CR0.WP, or a sibling's unmap mid-copy (its shootdown makes
///   the access fault) is recovered.
/// - The other side is reached through the HHDM, inside
///   `Pmap::with_user_frame`: its `inner` lock is held from the
///   permission-checked walk until the chunk is copied, so a sibling of the
///   peer can't unmap and free the frame mid-copy; its unmap waits.
/// - When neither is live (a kernel-thread caller in the test harness), each
///   chunk goes through a small bounce buffer, one Pmap's lock at a time
///   (never both: two copies in opposite directions would take them in
///   opposite orders).
///
/// Lock order: the channel guard (held by every caller) → a peer Pmap's
/// `inner`. Nothing takes a channel guard under `inner`.
fn cross_pmap_copy(
    src_pmap: &Pmap,
    src_va: u64,
    dst_pmap: &Pmap,
    dst_va: u64,
    n: usize,
) -> Result<(), ()> {
    use crate::arch::x86_64::user_copy::try_copy;

    if n == 0 {
        return Ok(());
    }
    let live = x86_64::registers::control::Cr3::read().0;
    let src_live = src_pmap.cr3().0 == live;
    let dst_live = !src_live && dst_pmap.cr3().0 == live;
    let mut copied: usize = 0;
    while copied < n {
        let s = src_va + copied as u64;
        let d = dst_va + copied as u64;
        // How much of THIS iteration stays inside both pages?
        let src_page_tail = 0x1000 - (s & 0xFFF) as usize;
        let dst_page_tail = 0x1000 - (d & 0xFFF) as usize;
        let mut chunk = (n - copied).min(src_page_tail).min(dst_page_tail);
        let ok = if src_live {
            dst_pmap
                .with_user_frame(VirtAddr::new(d), true, |dst_kern| {
                    #[cfg(feature = "neo-test-user-copy-fault")]
                    copy_race_test::hold(n, s, dst_pmap, d);
                    // SAFETY: the destination is `chunk` bytes of one user
                    // frame, kept mapped by the held lock; the source is the
                    // live user VA, in the user half (the syscall validated
                    // it), and a fault on it is recovered.
                    unsafe { try_copy(dst_kern, s as *const u8, chunk) }
                })
                .ok_or(())?
        } else if dst_live {
            src_pmap
                .with_user_frame(VirtAddr::new(s), false, |src_kern| {
                    #[cfg(feature = "neo-test-user-copy-fault")]
                    copy_race_test::hold(n, d, src_pmap, s);
                    // SAFETY: as above, with the roles swapped.
                    unsafe { try_copy(d as *mut u8, src_kern, chunk) }
                })
                .ok_or(())?
        } else {
            let mut bounce = [0u8; 256];
            chunk = chunk.min(bounce.len());
            src_pmap
                .with_user_frame(VirtAddr::new(s), false, |src_kern| {
                    // SAFETY: `chunk` bytes of one user frame, kept mapped by
                    // the held lock, into the kernel stack buffer.
                    unsafe { core::ptr::copy_nonoverlapping(src_kern, bounce.as_mut_ptr(), chunk) }
                })
                .ok_or(())?;
            dst_pmap
                .with_user_frame(VirtAddr::new(d), true, |dst_kern| {
                    // SAFETY: as above, out of the kernel stack buffer.
                    unsafe { core::ptr::copy_nonoverlapping(bounce.as_ptr(), dst_kern, chunk) }
                })
                .ok_or(())?;
            true
        };
        if !ok {
            return Err(());
        }
        copied += chunk;
    }
    Ok(())
}

/// `user_copy_fault` test only: make a sibling's unmap of the peer's buffer
/// land in the middle of an IPC copy, and record whether the frame stayed
/// mapped. An IPC copy of exactly `RACE_LEN` bytes tells the process that
/// the copy is under way by writing the peer page's address into the first 8
/// bytes of the live side's buffer (the copy overwrites them after, if that
/// side is the destination), where an unmapping sibling on another CPU polls.
/// It then holds the peer's frame, and its `inner` lock, until that unmap is
/// waiting on the lock, or a spin bound runs out. `RESULT` then holds bit 0
/// "the hold ran", bit 1 "the unmap was seen waiting" and bit 2 "the frame
/// was still mapped".
#[cfg(feature = "neo-test-user-copy-fault")]
pub(crate) mod copy_race_test {
    use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

    use crate::mm::pmap::Pmap;

    /// The IPC length that arms the hold (neo-init's race leg sends this).
    pub const RACE_LEN: usize = 72;
    static HOLDING: AtomicBool = AtomicBool::new(false);
    static UNMAP_WAITING: AtomicBool = AtomicBool::new(false);
    pub static RESULT: AtomicU8 = AtomicU8::new(0);

    /// From `Pmap::unmap_user_range`, before it takes `inner`.
    pub fn note_unmap_waiting() {
        if HOLDING.load(Ordering::Acquire) {
            UNMAP_WAITING.store(true, Ordering::Release);
        }
    }

    pub(super) fn hold(n: usize, live_va: u64, peer_pmap: &Pmap, peer_va: u64) {
        if n != RACE_LEN || RESULT.load(Ordering::Acquire) != 0 {
            return;
        }
        UNMAP_WAITING.store(false, Ordering::Release);
        HOLDING.store(true, Ordering::Release);
        // The go for the unmapping sibling: which page to unmap.
        let _ = crate::arch::x86_64::syscall::write_user(live_va as *mut u64, peer_va & !0xFFF);
        // Both waits are bounded in TSC cycles, far inside the TLB
        // shootdown timeout (500M cycles): this CPU, and the sibling spinning
        // on `inner`, have interrupts off meanwhile, so a shootdown from a
        // third CPU waits for them.
        // SAFETY: RDTSC has no preconditions.
        let tsc = || unsafe { core::arch::x86_64::_rdtsc() };
        let start = tsc();
        let mut seen = false;
        while tsc().wrapping_sub(start) < 150_000_000 {
            if UNMAP_WAITING.load(Ordering::Acquire) {
                seen = true;
                break;
            }
            core::hint::spin_loop();
        }
        // Keep holding a while longer: an unmap that is NOT blocked by the
        // held lock clears the PTE inside this window (it flagged itself
        // just before taking the lock), so a frame that goes away here is
        // the defect.
        let peer = x86_64::VirtAddr::new(peer_va);
        let mut mapped = peer_pmap.translate_user(peer, false).is_some();
        if seen {
            let held = tsc();
            while tsc().wrapping_sub(held) < 30_000_000 {
                if peer_pmap.translate_user(peer, false).is_none() {
                    mapped = false;
                    break;
                }
                core::hint::spin_loop();
            }
        }
        HOLDING.store(false, Ordering::Release);
        RESULT.store(
            1 | u8::from(seen) << 1 | u8::from(mapped) << 2,
            Ordering::Release,
        );
    }
}
