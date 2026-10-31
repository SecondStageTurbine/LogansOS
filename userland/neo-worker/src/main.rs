// SPDX-License-Identifier: MPL-2.0
//! `neo-worker` — Phase 1.5b BUILD-6 IPC + kill-target test peer.
//!
//! Spawned by `neo-init` via `sys_spawn("worker", &[h_main, h_block])`.
//! ADR-004 §Q2 deterministic ordering: parent's first inherited handle
//! lands at our Handle(0), second at Handle(1).
//!
//! Two-phase scenario (see neo-init/main.rs and ADR-007 §test-scenario):
//!   1. recv ping on Handle(0)/MAIN
//!   2. send pong on Handle(0)/MAIN
//!   3. recv "go-block" on Handle(1)/BLOCK — synchronization
//!   4. recv on Handle(0)/MAIN — BLOCKS HERE; init kills us while we're parked.
//!      SC-4 Q8.c zombifies the parked thread, then the reaper drops our
//!      Box<Thread> and Pmap.

#![no_std]
#![no_main]
#![cfg_attr(
    any(
        feature = "cap-mint-and-use",
        feature = "cap-derive-subset",
        feature = "cap-grant-basic",
        feature = "cap-grant-missing-meta",
        feature = "cap-revoke-cross-process",
        feature = "cap-revoke-descendants-walk",
        feature = "cap-revoke-spawn-inheritance-descendants",
        feature = "cap-survive-exit",
        feature = "cap-graph-revoke",
        feature = "file-cap-derive-grant",
        feature = "cap-rooted-open-test",
        feature = "mem-quota-test",
        feature = "kill-ready-in-kernel-test",
        feature = "socket-teardown-test",
        feature = "obj-quota-test",
        feature = "exit-codes-test",
        feature = "mount-authority-test",
        feature = "atrium-surface-protocol",
        feature = "atrium-slice3",
        feature = "atrium-interactive",
        feature = "atrium-interactive-baremetal",
        feature = "dw1-desktop-wm",
        feature = "dw2-scene-policy",
        feature = "gpu-glass-slice6",
        feature = "sys-wait-blocking",
        feature = "sys-wait-wnohang",
        feature = "sys-wait-already-exited",
        feature = "sys-wait-multiple-waiters",
        feature = "sys-inspect-running",
        feature = "sys-inspect-exited",
        feature = "sys-inspect-rights-gate",
        feature = "sched-ready-queue-capacity-pressure",
        feature = "neo-shell-repl",
        feature = "arthur-action-surface",
        feature = "ipc-rendezvous-translate-fault",
        feature = "phase4-https-fetch",
        feature = "vault-phase-b",
        feature = "desktop-lens",
        feature = "installer-probe",
        feature = "record-guard-probe",
    ),
    allow(dead_code)
)]

extern crate alloc;

use alloc::vec::Vec;

use libneo::{Handle, sys_exit, sys_recv, sys_send, sys_write};

#[cfg(any(
    feature = "cap-grant-basic",
    feature = "cap-grant-missing-meta",
    feature = "cap-revoke-cross-process",
    feature = "cap-revoke-descendants-walk",
    feature = "cap-revoke-spawn-inheritance-descendants",
    feature = "file-cap-derive-grant",
))]
use libneo::{FULL_CHANNEL_RIGHTS, sys_recv_grant};

#[cfg(feature = "file-cap-derive-grant")]
use libneo::{EACCES, sys_read_fd};

#[cfg(feature = "cap-grant-missing-meta")]
use libneo::EPIPE;

#[cfg(any(
    feature = "cap-revoke-cross-process",
    feature = "cap-revoke-descendants-walk",
    feature = "cap-revoke-spawn-inheritance-descendants",
))]
use libneo::EBADF;

#[cfg(any(
    feature = "cap-survive-exit",
    feature = "cap-revoke-spawn-inheritance-descendants"
))]
use libneo::CapRights;

#[cfg(feature = "cap-survive-exit")]
use libneo::sys_grant;

#[cfg(feature = "cap-revoke-spawn-inheritance-descendants")]
use libneo::{channel_rights, meta_rights, sys_derive};

const HELLO: &[u8] = b"hello from neo-worker (phase 1.5b BUILD-6 e2e: ipc + heap + kill targets)\n";
const RECV_PING_OK: &[u8] = b"[ neo-worker ] received ping from parent\n";
const RECV_UNBLOCK_OK: &[u8] =
    b"[ neo-worker ] woke from unblock; entering spin_loop awaiting sys_kill\n";
const RECV_FAIL: &[u8] = b"[ neo-worker ] sys_recv FAILED\n";
const SEND_FAIL: &[u8] = b"[ neo-worker ] sys_send FAILED\n";

const MAIN: Handle = Handle(0);

#[cfg(feature = "installer-probe")]
mod installer_probe;
#[cfg(feature = "desktop-lens")]
mod lens;
#[cfg(feature = "record-guard-probe")]
mod record_guard;
const BLOCK: Handle = Handle(1);

#[cfg(feature = "atrium-surface-protocol")]
fn atrium_surface_protocol_worker() -> i32 {
    use libneo::atrium_protocol::{
        Header, OP_COMMIT, OP_CREATE_SURFACE, OP_DESTROY_SURFACE, OP_FOCUS_REQUEST,
        OP_IMPORT_ATTACH_BUFFER, ProtocolError, REPLY_LEN, Reply, Request, SurfaceRegistry,
        decode_request, encode_reply,
    };
    use libneo::{
        ATRIUM_MAP_READ, ATRIUM_QUERY, BUFFER_DESC_V1_LEN, CapRights, EPIPE, FULL_CHANNEL_RIGHTS,
        meta_rights, sys_buffer_query, sys_chan_create, sys_grant, sys_recv_grant,
    };

    const NS: u64 = 0x4751_0000_0000_0001;
    const SURFACE_CAP_PROVENANCE: u64 = 0x5355_5246_0000_0001;
    const BUFFER_CAP_PROVENANCE: u64 = 0x4255_4646_0000_0001;

    fn recv_request(channel: Handle) -> Result<(Header, Request), i64> {
        let mut bytes = [0u8; libneo::atrium_protocol::MAX_MESSAGE_LEN];
        let len = sys_recv(channel, &mut bytes);
        if len <= 0 {
            return Err(len);
        }
        decode_request(&bytes[..len as usize]).map_err(|_| libneo::EINVAL)
    }

    fn recv_surface_request(
        channel: Handle,
        registry: &mut SurfaceRegistry,
        cap_provenance: u64,
    ) -> Result<(Header, Request), i64> {
        match recv_request(channel) {
            Err(EPIPE) => {
                if registry.peer_died(cap_provenance) {
                    Err(EPIPE)
                } else {
                    Err(libneo::EINVAL)
                }
            }
            result => result,
        }
    }

    fn send_reply(
        channel: Handle,
        opcode: u16,
        request_id: u64,
        status: u32,
        identity: libneo::atrium_protocol::Identity,
    ) -> bool {
        let bytes = encode_reply(Reply {
            opcode,
            request_id,
            status,
            identity,
        });
        bytes.len() == REPLY_LEN && sys_send(channel, &bytes) == REPLY_LEN as i64
    }

    fn grant_surface(session: Handle, client_endpoint: Handle) -> bool {
        let mut byte = [0u8; 1];
        sys_send(session, b"O") == 1
            && sys_recv(session, &mut byte) == 1
            && byte[0] == b'R'
            && sys_grant(session, client_endpoint, FULL_CHANNEL_RIGHTS) == 0
            && sys_recv(session, &mut byte) == 1
            && byte[0] == b'A'
    }

    let session = MAIN;
    let mut registry = SurfaceRegistry::new(NS, 1920, 1080);
    let (create_header, create) = match recv_request(session) {
        Ok((header, Request::Create(create))) => (header, create),
        _ => return 30,
    };
    let (surface_server, surface_client) = match sys_chan_create() {
        Ok(pair) => pair,
        Err(_) => return 31,
    };
    let identity = match registry.create(create, SURFACE_CAP_PROVENANCE) {
        Ok(identity) => identity,
        Err(_) => return 32,
    };
    // Normal reply, then explicit bounded SurfaceCap transfer:
    // OFFER / READY / grant / ACCEPTED.
    if !send_reply(
        session,
        OP_CREATE_SURFACE,
        create_header.request_id,
        0,
        identity,
    ) || !grant_surface(session, surface_client)
    {
        return 33;
    }

    let (second_header, second) = match recv_request(session) {
        Ok((header, Request::Create(create))) => (header, create),
        _ => return 37,
    };
    if registry.create(second, SURFACE_CAP_PROVENANCE + 1) != Err(ProtocolError::Limit)
        || !send_reply(
            session,
            OP_CREATE_SURFACE,
            second_header.request_id,
            ProtocolError::Limit.status(),
            identity,
        )
    {
        return 38;
    }

    let (import_header, import) = match recv_surface_request(
        surface_server,
        &mut registry,
        identity.surface_cap_provenance,
    ) {
        Ok((header, Request::ImportAttach(import))) => (header, import),
        _ => return 39,
    };
    if sys_send(surface_server, b"R") != 1 {
        return 40;
    }
    let read_query = CapRights::new(meta_rights::USE, ATRIUM_MAP_READ | ATRIUM_QUERY);
    let buffer = match sys_recv_grant(surface_server, read_query) {
        Ok(handle) => handle,
        Err(_) => return 41,
    };
    let mut desc = [0u8; BUFFER_DESC_V1_LEN];
    if sys_buffer_query(buffer, &mut desc) != 0
        || registry
            .import_attach_on_channel(
                import,
                identity.surface_cap_provenance,
                BUFFER_CAP_PROVENANCE,
            )
            .is_err()
        || !send_reply(
            surface_server,
            OP_IMPORT_ATTACH_BUFFER,
            import_header.request_id,
            0,
            identity,
        )
    {
        return 42;
    }

    let (commit_header, commit) = match recv_surface_request(
        surface_server,
        &mut registry,
        identity.surface_cap_provenance,
    ) {
        Ok((header, Request::Commit(commit))) => (header, commit),
        _ => return 43,
    };
    if registry.commit_on_channel(commit, identity.surface_cap_provenance) != Ok(1)
        || !send_reply(
            surface_server,
            OP_COMMIT,
            commit_header.request_id,
            0,
            identity,
        )
    {
        return 44;
    }
    let (focus_header, focus) = match recv_surface_request(
        surface_server,
        &mut registry,
        identity.surface_cap_provenance,
    ) {
        Ok((header, Request::Focus(focus))) => (header, focus),
        _ => return 45,
    };
    // Slice-2 policy hook only. No InputFocus activation or HID routing here.
    if registry.focus_request_on_channel(
        focus,
        identity.surface_cap_provenance,
        |surface_id, _role| surface_id == identity.surface_id,
    ) != Ok(true)
        || !send_reply(
            surface_server,
            OP_FOCUS_REQUEST,
            focus_header.request_id,
            0,
            identity,
        )
    {
        return 46;
    }
    let (destroy_header, destroy) = match recv_surface_request(
        surface_server,
        &mut registry,
        identity.surface_cap_provenance,
    ) {
        Ok((header, Request::Destroy(destroy))) => (header, destroy),
        _ => return 47,
    };
    if registry
        .destroy_on_channel(destroy, identity.surface_cap_provenance)
        .is_err()
        || registry.is_alive(identity)
        || !send_reply(
            surface_server,
            OP_DESTROY_SURFACE,
            destroy_header.request_id,
            0,
            identity,
        )
    {
        return 48;
    }

    let (drop_header, drop_create) = match recv_request(session) {
        Ok((header, Request::Create(create))) => (header, create),
        _ => return 49,
    };
    let (drop_server, drop_client) = match sys_chan_create() {
        Ok(pair) => pair,
        Err(_) => return 50,
    };
    let dropped_identity = match registry.create(drop_create, SURFACE_CAP_PROVENANCE + 1) {
        Ok(identity) => identity,
        Err(_) => return 51,
    };
    if !send_reply(
        session,
        OP_CREATE_SURFACE,
        drop_header.request_id,
        0,
        dropped_identity,
    ) || !grant_surface(session, drop_client)
    {
        return 52;
    }

    // The client closes without DESTROY. recv_surface_request must preserve
    // EPIPE long enough to route it into the registry transition.
    if recv_surface_request(
        drop_server,
        &mut registry,
        dropped_identity.surface_cap_provenance,
    ) != Err(EPIPE)
        || registry.commit_on_channel(
            libneo::atrium_protocol::Commit {
                identity: dropped_identity,
                client_serial: 1,
            },
            dropped_identity.surface_cap_provenance,
        ) != Err(ProtocolError::SurfaceGone)
    {
        return 53;
    }

    // A fresh create after peer death must reuse the freed bounded slot.
    let (reuse_header, reuse_create) = match recv_request(session) {
        Ok((header, Request::Create(create))) => (header, create),
        _ => return 54,
    };
    let (reuse_server, reuse_client) = match sys_chan_create() {
        Ok(pair) => pair,
        Err(_) => return 55,
    };
    let reused_identity = match registry.create(reuse_create, SURFACE_CAP_PROVENANCE + 2) {
        Ok(identity) if identity.surface_id != dropped_identity.surface_id => identity,
        _ => return 56,
    };
    if !send_reply(
        session,
        OP_CREATE_SURFACE,
        reuse_header.request_id,
        0,
        reused_identity,
    ) || !grant_surface(session, reuse_client)
    {
        return 57;
    }
    let (final_header, final_destroy) = match recv_surface_request(
        reuse_server,
        &mut registry,
        reused_identity.surface_cap_provenance,
    ) {
        Ok((header, Request::Destroy(destroy))) => (header, destroy),
        _ => return 58,
    };
    if registry
        .destroy_on_channel(final_destroy, reused_identity.surface_cap_provenance)
        .is_err()
        || !send_reply(
            reuse_server,
            OP_DESTROY_SURFACE,
            final_header.request_id,
            0,
            reused_identity,
        )
    {
        return 59;
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    #[cfg(feature = "record-guard-probe")]
    {
        sys_exit(record_guard::run());
    }

    #[cfg(feature = "installer-probe")]
    {
        sys_exit(installer_probe::run());
    }

    #[cfg(feature = "desktop-lens")]
    {
        sys_exit(lens::run());
    }

    #[cfg(feature = "atrium-surface-protocol")]
    {
        sys_exit(atrium_surface_protocol_worker());
    }

    #[cfg(feature = "atrium-slice3")]
    {
        sys_exit(atrium_slice3_client());
    }

    #[cfg(any(
        feature = "atrium-interactive",
        feature = "atrium-interactive-baremetal"
    ))]
    {
        sys_exit(atrium_interactive_client());
    }

    #[cfg(any(feature = "dw1-desktop-wm", feature = "dw2-scene-policy"))]
    {
        sys_exit(dw1_desktop_client());
    }

    #[cfg(feature = "gpu-glass-slice6")]
    {
        sys_exit(gpu_glass_slice6_worker());
    }

    #[cfg(any(
        feature = "cap-mint-and-use",
        feature = "cap-derive-subset",
        feature = "cap-grant-basic",
        feature = "cap-grant-missing-meta",
        feature = "cap-revoke-cross-process",
        feature = "cap-revoke-descendants-walk",
        feature = "cap-revoke-spawn-inheritance-descendants",
        feature = "cap-survive-exit",
        feature = "cap-graph-revoke",
        feature = "file-cap-derive-grant"
    ))]
    {
        let code = cap_worker_main();
        if code == 0 {
            sys_exit(0);
        }
        loop {
            core::hint::spin_loop();
        }
    }

    #[cfg(feature = "cap-rooted-open-test")]
    {
        sys_exit(cap_rooted_open_worker());
    }

    #[cfg(feature = "mem-quota-test")]
    {
        sys_exit(mem_quota_worker());
    }

    #[cfg(feature = "kill-ready-in-kernel-test")]
    {
        sys_exit(kill_ready_in_kernel_worker());
    }

    #[cfg(feature = "socket-teardown-test")]
    {
        sys_exit(socket_teardown_worker());
    }

    #[cfg(feature = "obj-quota-test")]
    {
        sys_exit(obj_quota_worker());
    }

    #[cfg(feature = "exit-codes-test")]
    {
        sys_exit(exit_codes_worker());
    }

    #[cfg(feature = "mount-authority-test")]
    {
        sys_exit(mount_authority_worker());
    }

    #[cfg(any(
        feature = "sys-wait-blocking",
        feature = "sys-wait-wnohang",
        feature = "sys-wait-already-exited",
        feature = "sys-wait-multiple-waiters",
        feature = "sys-inspect-running",
        feature = "sys-inspect-exited",
        feature = "sys-inspect-rights-gate"
    ))]
    {
        process_cap_worker_main();
    }

    #[cfg(feature = "sched-ready-queue-capacity-pressure")]
    {
        loop {
            core::hint::spin_loop();
        }
    }

    #[cfg(feature = "neo-shell-repl")]
    {
        sys_exit(neo_shell_repl_worker_main());
    }

    #[cfg(feature = "arthur-action-surface")]
    {
        sys_exit(arthur_action_surface_worker_main());
    }

    #[cfg(feature = "ipc-rendezvous-translate-fault")]
    {
        let ret = sys_send(MAIN, b"ping");
        if ret == libneo::EFAULT {
            sys_exit(0);
        }
        sys_exit(10);
    }

    #[cfg(feature = "phase4-https-fetch")]
    {
        sys_exit(phase4_https_worker::run());
    }

    #[cfg(feature = "vault-phase-b")]
    {
        sys_exit(vault_worker::run());
    }

    #[cfg(feature = "threads-guard-fault")]
    {
        sys_exit(threads_guard_fault_worker());
    }

    // OT-61 C1.2 (threads_tls), two roles chosen by the capability probe:
    // - spawned with no handles (no C1 capability): SYS_THREAD_SET_TLS must
    //   refuse with EPERM, exit 41;
    // - spawned with the parent's capability: a thread that skipped the
    //   bootstrap (FS.base 0) sets a key, which must fault this process
    //   (the parent expects 139), not read address 0 and continue.
    #[cfg(feature = "threads-tls")]
    {
        const EPERM: i64 = -1;
        sys_exit(match libneo::sys_thread_set_tls(0) {
            Err(EPERM) => 41,
            Err(_) => 42,
            Ok(()) => threads_tls_unset_key_access(),
        });
    }

    #[cfg(feature = "threads-exit-group")]
    {
        sys_exit(threads_exit_group_worker());
    }

    #[cfg(not(any(
        feature = "cap-mint-and-use",
        feature = "cap-derive-subset",
        feature = "cap-grant-basic",
        feature = "cap-grant-missing-meta",
        feature = "cap-revoke-cross-process",
        feature = "cap-revoke-descendants-walk",
        feature = "cap-revoke-spawn-inheritance-descendants",
        feature = "cap-survive-exit",
        feature = "cap-graph-revoke",
        feature = "file-cap-derive-grant",
        feature = "cap-rooted-open-test",
        feature = "mem-quota-test",
        feature = "kill-ready-in-kernel-test",
        feature = "socket-teardown-test",
        feature = "obj-quota-test",
        feature = "exit-codes-test",
        feature = "mount-authority-test",
        feature = "atrium-surface-protocol",
        feature = "atrium-slice3",
        feature = "atrium-interactive",
        feature = "atrium-interactive-baremetal",
        feature = "dw1-desktop-wm",
        feature = "dw2-scene-policy",
        feature = "gpu-glass-slice6",
        feature = "sys-wait-blocking",
        feature = "sys-wait-wnohang",
        feature = "sys-wait-already-exited",
        feature = "sys-wait-multiple-waiters",
        feature = "sys-inspect-running",
        feature = "sys-inspect-exited",
        feature = "sys-inspect-rights-gate",
        feature = "sched-ready-queue-capacity-pressure",
        feature = "neo-shell-repl",
        feature = "arthur-action-surface",
        feature = "ipc-rendezvous-translate-fault",
        feature = "phase4-https-fetch",
        feature = "vault-phase-b",
        feature = "threads-guard-fault",
        feature = "threads-tls",
        feature = "threads-exit-group",
        feature = "desktop-lens",
        feature = "installer-probe",
        feature = "record-guard-probe"
    )))]
    {
        default_worker_main();
    }
}

/// OT-61 C1.3b (threads_exit_group): one role per spawn, read from the channel
/// at handle 1 (the C1 capability is handle 0). Each ends the process while a
/// sibling sits where the C1.1 drain could not reach it:
/// 1. a thread spinning in ring 3 on CPU 2 (and one parked in a futex wait),
///    then `SYS_EXIT(7)`;
/// 2. the same spinner, then a ring-3 page fault (the parent expects 139);
/// 3. a thread on CPU 1 armed by `SYS_TEST_FUTEX_LATE_PARK`, whose futex wait
///    reaches its park only after the retire's purge, then `SYS_EXIT(7)`;
/// 4. a thread on CPU 2 inside `SYS_TEST_KERNEL_SPIN` (ring 0) when the main
///    thread calls `SYS_EXIT(7)`; once the syscall returns it spins in ring 3
///    with no further syscall, so only the syscall-return check can end it;
/// 5. a thread on CPU 1 armed by `SYS_TEST_FUTEX_LATE_PARK` whose
///    `SYS_THREAD_JOIN` of a futex-parked sibling reaches its park only after
///    the retire's purge, then `SYS_EXIT(7)` (the kernel join loop must return
///    ESRCH instead of retrying forever in ring 0: C1.3b review finding 1);
/// 6. a thread on CPU 2 in `SYS_TEST_TAIL_HOLD`, which returns with IF=1 (as
///    `spawn_user` does) and holds in the syscall return after the final
///    retire check; the retire IPI lands there, then the thread spins in
///    ring 3 with no syscall. Only a final check run with interrupts off
///    (the IPI then waits for ring 3) ends it: C1.3b review finding 2.
/// Codes 50-59 and 60-65 report a setup failure.
#[cfg(feature = "threads-exit-group")]
fn threads_exit_group_worker() -> i32 {
    use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
    use libneo::{
        FUTEX_DEADLINE_INFINITE, sys_futex_wait, sys_sched_yield, sys_test_futex_late_park,
        sys_test_kernel_spin, sys_test_thread_create_on, sys_thread_exit,
    };

    static SPINNING: AtomicU32 = AtomicU32::new(0);
    static SPINS: AtomicU64 = AtomicU64::new(0);
    static PARK_WORD: AtomicU32 = AtomicU32::new(0);
    static ARMED: AtomicU32 = AtomicU32::new(0);
    static IN_KERNEL: AtomicU32 = AtomicU32::new(0);

    extern "C" fn spinner(_: usize) -> ! {
        SPINNING.store(1, Ordering::Release);
        // Ring 3 only: no syscall, so no syscall boundary ever checks it.
        loop {
            SPINS.fetch_add(1, Ordering::Relaxed);
            core::hint::spin_loop();
        }
    }
    extern "C" fn parker(_: usize) -> ! {
        loop {
            let _ = sys_futex_wait(PARK_WORD.as_ptr(), 0, FUTEX_DEADLINE_INFINITE);
        }
    }
    extern "C" fn late_parker(_: usize) -> ! {
        if sys_test_futex_late_park() != 0 {
            sys_thread_exit(57);
        }
        ARMED.store(1, Ordering::Release);
        // Enters the kernel now, but parks only after the retire's purge.
        loop {
            let _ = sys_futex_wait(PARK_WORD.as_ptr(), 0, FUTEX_DEADLINE_INFINITE);
        }
    }
    extern "C" fn late_joiner(target: usize) -> ! {
        if sys_test_futex_late_park() != 0 {
            sys_thread_exit(60);
        }
        ARMED.store(1, Ordering::Release);
        // Enters SYS_THREAD_JOIN now; its futex park is held until after the
        // retire's purge, so the kernel refuses it (ESRCH).
        let _ = libneo::sys_thread_join(target as u64);
        // Never reached: the syscall-return check retires this thread.
        sys_thread_exit(61)
    }
    static IN_TAIL: AtomicU32 = AtomicU32::new(0);
    extern "C" fn tail_holder(_: usize) -> ! {
        let _ = libneo::sys_test_tail_hold(&IN_TAIL);
        // Back in ring 3, never to make another syscall.
        loop {
            SPINS.fetch_add(1, Ordering::Relaxed);
            core::hint::spin_loop();
        }
    }
    extern "C" fn kernel_spinner(_: usize) -> ! {
        IN_KERNEL.store(1, Ordering::Release);
        let _ = sys_test_kernel_spin(500);
        // Back in ring 3, never to make another syscall.
        loop {
            SPINS.fetch_add(1, Ordering::Relaxed);
            core::hint::spin_loop();
        }
    }
    fn wait_for(flag: &AtomicU32) -> bool {
        (0..5_000_000u64).any(|_| {
            if flag.load(Ordering::Acquire) == 1 {
                return true;
            }
            sys_sched_yield();
            false
        })
    }

    let mut role = [0u8; 1];
    if sys_recv(Handle(1), &mut role) < 1 {
        return 50;
    }
    match role[0] {
        1 | 2 => {
            if sys_test_thread_create_on(spinner, 0, 16 * 1024, 2).is_err() {
                return 51;
            }
            if libneo::sys_thread_create(parker, 0, 16 * 1024, true).is_err() {
                return 52;
            }
            if !wait_for(&SPINNING) {
                return 53;
            }
            if role[0] == 1 {
                sys_exit(7);
            }
            // SAFETY: a deliberate ring-3 fault (unmapped address 8).
            unsafe { core::ptr::write_volatile(8 as *mut u64, 1) };
            54
        }
        3 => {
            if sys_test_thread_create_on(late_parker, 0, 16 * 1024, 1).is_err() {
                return 55;
            }
            if !wait_for(&ARMED) {
                return 56;
            }
            // Let it enter SYS_FUTEX_WAIT (it holds there until the retire).
            for _ in 0..1_000 {
                sys_sched_yield();
            }
            sys_exit(7)
        }
        4 => {
            if sys_test_thread_create_on(kernel_spinner, 0, 16 * 1024, 2).is_err() {
                return 58;
            }
            if !wait_for(&IN_KERNEL) {
                return 58;
            }
            // It is now inside its 500 ms ring-0 spin.
            for _ in 0..1_000 {
                sys_sched_yield();
            }
            sys_exit(7)
        }
        5 => {
            let Ok(target) = libneo::sys_thread_create(parker, 0, 16 * 1024, true) else {
                return 62;
            };
            if sys_test_thread_create_on(late_joiner, target as usize, 16 * 1024, 1).is_err() {
                return 63;
            }
            if !wait_for(&ARMED) {
                return 63;
            }
            // Let it enter SYS_THREAD_JOIN (it holds there until the retire).
            for _ in 0..1_000 {
                sys_sched_yield();
            }
            sys_exit(7)
        }
        6 => {
            if sys_test_thread_create_on(tail_holder, 0, 16 * 1024, 2).is_err() {
                return 64;
            }
            // The kernel sets IN_TAIL once the holder is past its final check.
            if !wait_for(&IN_TAIL) {
                return 65;
            }
            sys_exit(7)
        }
        _ => 59,
    }
}

/// OT-61 C1.2 (threads_tls, with-cap role): set a key on a thread whose FS.base
/// was never set. Returns only if that did NOT fault (a distinct failure code).
#[cfg(feature = "threads-tls")]
fn threads_tls_unset_key_access() -> i32 {
    use core::sync::atomic::{AtomicU32, Ordering};
    static RETURNED: AtomicU32 = AtomicU32::new(0);
    static KEY: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

    extern "C" fn no_tls_thread(_: usize) -> ! {
        // FS.base is 0 here: `set` reads `fs:[0]` to find the control block.
        libneo::ntls::set(KEY.load(Ordering::Acquire), 1 as *mut u8);
        RETURNED.store(1, Ordering::Release);
        libneo::sys_thread_exit(0)
    }

    let Some(key) = libneo::ntls::create(None) else {
        return 44;
    };
    KEY.store(key, Ordering::Release);
    let Ok(tid) = libneo::sys_thread_create(no_tls_thread, 0, 16 * 1024, true) else {
        return 45;
    };
    let _ = libneo::sys_thread_join(tid);
    // Reached only if the key access did not fault the process.
    if RETURNED.load(Ordering::Acquire) == 1 {
        46
    } else {
        47
    }
}

/// OT-61 C1.1 (threads_guard_fault child). Two roles, self-selected by the
/// capability probe — the parent spawns this binary twice:
///
///   1. **No-cap spawn** (no inherited handles): `thread_create` must be
///      refused with `-EPERM` (fail-closed cap confinement, no self-mint).
///      Exit 41 confirms the refusal; any other outcome is a distinct code.
///   2. **With-cap spawn** (parent passes its browser-runtime Handle(0)):
///      exercise the refcount (spawn 3 threads, join all — the process must
///      NOT be torn down by the intermediate reaps), park one thread on a
///      futex forever (the forced-teardown-of-parked-threads witness), then
///      recurse a small guarded stack into its guard page. The ring-3 #PF
///      must fault-kill THIS process (exit 139, delivered by the kernel);
///      reaching the end alive means containment failed (exit 55).
#[cfg(feature = "threads-guard-fault")]
extern "C" fn tgf_worker_ret(arg: usize) -> i32 {
    arg as i32
}

#[cfg(feature = "threads-guard-fault")]
static TGF_PARK_WORD: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[cfg(feature = "threads-guard-fault")]
extern "C" fn tgf_park_forever(_arg: usize) -> i32 {
    // Park on a word nobody ever wakes: this thread is still parked when the
    // guard fault kills the process, so the fault-kill path must zombify a
    // token-parked sibling (the teardown witness).
    // With `threads-guard-fault-timed` the wait has a deadline an hour out, so
    // the fault-kill purges a TIMED waiter and recomputes the earliest futex
    // deadline from the #PF handler (CP-46).
    #[cfg(feature = "threads-guard-fault-timed")]
    let deadline = libneo::nsync::deadline_after_ns(3_600_000_000_000);
    #[cfg(not(feature = "threads-guard-fault-timed"))]
    let deadline = libneo::FUTEX_DEADLINE_INFINITE;
    TGF_PARK_REACHED.store(1, core::sync::atomic::Ordering::Release);
    loop {
        let v = TGF_PARK_WORD.load(core::sync::atomic::Ordering::Acquire);
        let _ = libneo::sys_futex_wait(TGF_PARK_WORD.as_ptr() as *const u32, v, deadline);
    }
}

/// Frame-per-call recursion that touches its locals (volatile, and the add
/// after the recursive call defeats tail-call collapse), so RSP descends
/// through the stack in sub-page steps and the first out-of-stack touch
/// lands in the guard page.
#[cfg(feature = "threads-guard-fault")]
fn tgf_recurse(depth: usize) -> i32 {
    let mut buf = [0u8; 256];
    // SAFETY: in-bounds volatile touch of this frame's local buffer.
    unsafe { core::ptr::write_volatile(buf.as_mut_ptr(), depth as u8) };
    if depth > 100_000_000 {
        return buf[0] as i32;
    }
    // SAFETY: in-bounds volatile read of the same local.
    tgf_recurse(depth + 1) + (unsafe { core::ptr::read_volatile(buf.as_ptr()) } as i32)
}

#[cfg(feature = "threads-guard-fault")]
extern "C" fn tgf_overflow(_arg: usize) -> i32 {
    // Take the guard-page fault with RFLAGS.AC=1 (no ring-3 effect while
    // CR0.AM is clear). The kernel's #PF handler, and the fault-kill path it
    // runs, must still see AC=0, i.e. SMAP live (CP-40).
    // SAFETY: changes only RFLAGS.AC, which this thread never relies on.
    unsafe { core::arch::asm!("pushfq", "or qword ptr [rsp], 0x40000", "popfq") };
    tgf_recurse(0)
}

// "Reached the blocking call" flags: each sibling sets its flag right before
// parking, so the orchestrator can confirm all four ownership queues are
// actually occupied before it triggers the fault (blocker C non-vacuity).
#[cfg(feature = "threads-guard-fault")]
static TGF_IPC_REACHED: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "threads-guard-fault")]
static TGF_WAIT_REACHED: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "threads-guard-fault")]
static TGF_PARK_REACHED: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// IPC-parked sibling: recv-blocks forever on the channel handle passed as
/// `arg` (the orchestrator never sends). Parked in IPC at fault time.
#[cfg(feature = "threads-guard-fault")]
extern "C" fn tgf_ipc_recv(arg: usize) -> i32 {
    let h = Handle(arg as u32);
    let mut buf = [0u8; 4];
    TGF_IPC_REACHED.store(1, core::sync::atomic::Ordering::Release);
    let _ = sys_write(
        1,
        b"[ worker ] ipc-recv sibling: about to block\n".as_ptr(),
        44,
    );
    let _ = sys_recv(h, &mut buf); // blocks (no sender) until fault-kill drains it
    0
}

/// process-wait-parked sibling: sys_wait-blocks on the grandchild process
/// handle passed as `arg`. Parked in the GRANDCHILD's wait queue at fault time.
#[cfg(feature = "threads-guard-fault")]
extern "C" fn tgf_wait_child(arg: usize) -> i32 {
    let h = libneo::ProcessHandle(arg as u32);
    TGF_WAIT_REACHED.store(1, core::sync::atomic::Ordering::Release);
    let _ = sys_write(1, b"[ worker ] wait sibling: about to block\n".as_ptr(), 40);
    let _ = libneo::sys_wait(h, 0); // blocks (grandchild alive) until drained
    0
}

#[cfg(feature = "threads-guard-fault")]
fn threads_guard_fault_worker() -> i32 {
    use libneo::nthread;
    const EPERM: i64 = -1;
    const EBADF: i64 = -9;

    // Role dispatch. Try thread_create: succeeds only with the browser-runtime
    // cap ⇒ ORCHESTRATOR. Otherwise EPERM ⇒ either the no-cap PROBER (no
    // inherited handle) or the grandchild SLEEPER (inherited a channel at
    // Handle(0)); distinguish by whether Handle(0) is a channel.
    let probe = nthread::spawn(tgf_worker_ret, 5, 16 * 1024);
    let handle = match probe {
        Ok(h) => h,
        Err(EPERM) => {
            // SLEEPER vs PROBER: recv on Handle(0). A sleeper (channel) blocks
            // until its peer (the orchestrator) closes on death → EPIPE, then
            // exits 0. A prober (no handle) gets EBADF immediately → 41.
            let mut buf = [0u8; 4];
            let r = sys_recv(Handle(0), &mut buf);
            if r == EBADF {
                return 41; // PROBER: fail-closed thread_create EPERM confirmed
            }
            return 0; // SLEEPER: released (EPIPE) → exit cleanly, reaped
        }
        Err(_) => return 42,
    };
    if handle.join() != Ok(5) {
        return 43;
    }

    // Refcount: three concurrent joined threads; every intermediate reap must
    // leave the process (and this thread) alive.
    let mut sum = 0i32;
    let h1 = match nthread::spawn(tgf_worker_ret, 10, 16 * 1024) {
        Ok(h) => h,
        Err(_) => return 44,
    };
    let h2 = match nthread::spawn(tgf_worker_ret, 20, 16 * 1024) {
        Ok(h) => h,
        Err(_) => return 44,
    };
    let h3 = match nthread::spawn(tgf_worker_ret, 30, 16 * 1024) {
        Ok(h) => h,
        Err(_) => return 44,
    };
    for h in [h1, h2, h3] {
        match h.join() {
            Ok(code) => sum += code,
            Err(_) => return 45,
        }
    }
    if sum != 60 {
        return 46;
    }

    // blocker C: leave a sibling parked in EACH non-trivial ownership queue.
    // (1) IPC-parked: a channel with no sender; a thread recv-blocks on the
    //     orchestrator's own end.
    let (ipc_local, _ipc_peer) = match libneo::sys_chan_create() {
        Ok(p) => p,
        Err(_) => return 47,
    };
    if nthread::spawn(tgf_ipc_recv, ipc_local.0 as usize, 16 * 1024).is_err() {
        return 47;
    }
    // (2) process-wait-parked: spawn a grandchild SLEEPER (it recv-blocks on
    //     the inherited channel end, staying alive), then a thread sys_waits on
    //     it — parking in the GRANDCHILD's wait queue. The orchestrator keeps
    //     the sender end, so when it dies the grandchild's recv returns EPIPE
    //     and the grandchild self-exits (no orphan leak).
    let (_gc_hold, gc_peer) = match libneo::sys_chan_create() {
        Ok(p) => p,
        Err(_) => return 48,
    };
    let (_gc_pid, gc_handle) = match libneo::sys_spawn("worker", &[gc_peer]) {
        Ok(s) => s,
        Err(_) => return 48,
    };
    if nthread::spawn(tgf_wait_child, gc_handle.0 as usize, 16 * 1024).is_err() {
        return 48;
    }
    // (3) token/futex-parked witness.
    if nthread::spawn(tgf_park_forever, 0, 16 * 1024).is_err() {
        return 49;
    }

    // Ensure all three siblings have actually reached (and entered) their
    // blocking calls before we fault — so the fault-kill genuinely drains
    // occupied IPC + wait + futex queues (non-vacuous).
    use core::sync::atomic::Ordering;
    let mut ready = false;
    for _ in 0..2_000_000u64 {
        if TGF_IPC_REACHED.load(Ordering::Acquire) == 1
            && TGF_WAIT_REACHED.load(Ordering::Acquire) == 1
            && TGF_PARK_REACHED.load(Ordering::Acquire) == 1
        {
            ready = true;
            break;
        }
        libneo::sys_sched_yield();
    }
    if !ready {
        return 50; // a sibling never reached its blocking call
    }
    // A few more yields so the siblings are past the marker and actually parked.
    for _ in 0..200 {
        libneo::sys_sched_yield();
    }
    // The timed variant needs the futex sibling *enqueued* as a timed waiter
    // when the fault hits, not just past its marker (CP-3x): barrier on the
    // kernel's queue length for the park word.
    #[cfg(feature = "threads-guard-fault-timed")]
    {
        let mut parked = false;
        for _ in 0..2_000_000u64 {
            if libneo::sys_futex_queue_len(TGF_PARK_WORD.as_ptr() as *const u32) >= 1 {
                parked = true;
                break;
            }
            libneo::sys_sched_yield();
        }
        if !parked {
            return 50;
        }
    }

    // The guard-fault trigger: a 2-page guarded stack, recursed past its end.
    // The kernel must fault-kill this whole process (exit 139), draining every
    // parked sibling exactly once.
    let overflow = match nthread::spawn(tgf_overflow, 0, 8 * 1024) {
        Ok(h) => h,
        Err(_) => return 51,
    };
    let _ = overflow.join(); // never completes — the process dies first
    55 // alive after the overflow ⇒ containment/fault-kill FAILED
}

/// Receives a least-rights GpuBufferCap over the ordinary grant mechanism.
/// It has no destination or queue.  After the donor revokes the root, even the
/// handle it already received must resolve EBADF (not a stale usable object).
#[cfg(feature = "gpu-glass-slice6")]
fn gpu_glass_slice6_worker() -> i32 {
    use libneo::{
        CapRights, EBADF, gpu_compute_rights, meta_rights, sys_gpu_compute_submit_blur,
        sys_recv_grant,
    };

    let mut command = [0u8; 8];
    if sys_recv(MAIN, &mut command) != 2 || &command[..2] != b"go" {
        return 1;
    }
    let granted = match sys_recv_grant(
        MAIN,
        CapRights::new(meta_rights::USE, gpu_compute_rights::BUFFER_READ),
    ) {
        Ok(handle) => handle,
        Err(_) => return 2,
    };
    if sys_send(MAIN, b"grant") < 0 {
        return 3;
    }
    if sys_recv(MAIN, &mut command) != 6 || &command[..6] != b"revoke" {
        return 4;
    }
    if sys_gpu_compute_submit_blur(granted, granted, granted) != Err(EBADF) {
        return 5;
    }
    if sys_send(MAIN, b"revoked") < 0 {
        return 6;
    }
    0
}

/// Slice-5 text/workfield client. It alone holds RECEIVE and writes pixels;
/// the compositor receives only attenuated PRESENT and ACTIVATE siblings.
#[cfg(any(
    feature = "atrium-interactive",
    feature = "atrium-interactive-baremetal"
))]
fn atrium_interactive_client() -> i32 {
    use libneo::{
        ATRIUM_FOCUS_ACTIVATE, ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, CapRights, EAGAIN,
        FORMAT_XRGB8888_LE, INPUT_EVENT_V1_LEN, encode_buffer_desc, encode_damage_rect,
        meta_rights, sys_buffer_commit, sys_buffer_commit_damage, sys_buffer_create,
        sys_buffer_map, sys_grant, sys_input_focus_create, sys_input_focus_mint_activator,
        sys_input_focus_recv, sys_write,
    };

    const WIDTH: usize = 64;
    const HEIGHT: usize = 32;
    const BG: u32 = 0x0020_2226;
    const FG: u32 = 0x00E6_F2FF;
    const CELL_X: usize = 8;
    const CELL_Y: usize = 8;
    const GLYPH_A: [u8; 16] = [
        0x18, 0x18, 0x24, 0x24, 0x42, 0x42, 0x7e, 0x7e, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x00,
        0x00,
    ];

    let desc = encode_buffer_desc(64, 32, 256, FORMAT_XRGB8888_LE, 8192);
    let buffer = match sys_buffer_create(&desc) {
        Ok(h) => h,
        Err(_) => return 80,
    };
    let pixels = match sys_buffer_map(buffer, 0) {
        Ok(p) => p as *mut u32,
        Err(_) => return 81,
    };
    unsafe {
        for index in 0..WIDTH * HEIGHT {
            core::ptr::write_volatile(pixels.add(index), BG);
        }
    }
    if sys_buffer_commit(buffer) != 0 {
        return 82;
    }
    let compositor_buffer = CapRights::new(
        meta_rights::USE,
        ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
    );
    if sys_grant(MAIN, buffer, compositor_buffer) != 0 {
        return 83;
    }

    let focus = match sys_input_focus_create() {
        Ok(h) => h,
        Err(_) => return 84,
    };
    let activator = match sys_input_focus_mint_activator(focus) {
        Ok(h) => h,
        Err(_) => return 85,
    };
    let activate_only = CapRights::new(meta_rights::USE, ATRIUM_FOCUS_ACTIVATE);
    if sys_grant(MAIN, activator, activate_only) != 0 {
        return 86;
    }

    let mut go = [0u8; 1];
    if sys_recv(MAIN, &mut go) != 1 || go[0] != b'K' {
        return 87;
    }
    let mut event = [0u8; INPUT_EVENT_V1_LEN];
    let mut attempts = 0usize;
    loop {
        let rc = sys_input_focus_recv(focus, &mut event);
        if rc == 0 {
            break;
        }
        #[cfg(feature = "atrium-interactive")]
        let exhausted = attempts == 4096;
        #[cfg(feature = "atrium-interactive-baremetal")]
        let exhausted = attempts == 200_000_000;
        if rc != EAGAIN || exhausted {
            return 88;
        }
        attempts += 1;
        core::hint::spin_loop();
    }
    // InputEventV1: kind=key, pressed=1, HID usage 0x04 ('a').
    if event[0] != 1 || event[1] != 1 || event[2] != 0x04 {
        return 89;
    }
    let route = b"SLICE5 route key=0x04 focus=active delivered=1 dropped=0\n";
    let _ = sys_write(1, route.as_ptr(), route.len());

    unsafe {
        // A wild write outside the committed cell is intentional: the QEMU
        // proof asserts it never escapes this client's buffer.
        core::ptr::write_volatile(pixels, 0x00FF_00FF);
        for (row, bits) in GLYPH_A.iter().copied().enumerate() {
            for col in 0..8 {
                let color = if bits & (0x80 >> col) != 0 { FG } else { BG };
                core::ptr::write_volatile(pixels.add((CELL_Y + row) * WIDTH + CELL_X + col), color);
            }
        }
    }
    let damage = encode_damage_rect(CELL_X as u16, CELL_Y as u16, 8, 16);
    if sys_buffer_commit_damage(buffer, &damage) != 0 {
        return 90;
    }
    let commit = b"SLICE5 commit surface=workfield epoch=2 glyph=a\n";
    let _ = sys_write(1, commit.as_ptr(), commit.len());
    let damage = b"SLICE5 damage submitted=8,8,8,16 composited=pending\n";
    let _ = sys_write(1, damage.as_ptr(), damage.len());
    if sys_send(MAIN, b"D") != 1 {
        return 91;
    }
    0
}

/// DW-1 text client. It can create and commit only its own ROLE_TEXT surface,
/// cannot present to scanout directly, and only receives keyboard events while
/// its own InputFocus object is active.
#[cfg(any(feature = "dw1-desktop-wm", feature = "dw2-scene-policy"))]
fn dw1_desktop_client() -> i32 {
    use libneo::atrium_protocol::{
        Commit, CreateSurface, DestroySurface, FocusRequest, Identity, ImportAttachBuffer,
        OP_COMMIT, OP_CREATE_SURFACE, OP_DESTROY_SURFACE, OP_FOCUS_REQUEST,
        OP_IMPORT_ATTACH_BUFFER, REPLY_LEN, ROLE_TEXT, Request, decode_reply, encode_request,
    };
    #[cfg(feature = "dw2-scene-policy")]
    use libneo::atrium_protocol::{OP_CONFIGURE, Reply, decode_request, encode_reply};
    use libneo::{
        ATRIUM_FOCUS_ACTIVATE, ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, CapRights, EAGAIN,
        EPERM, FORMAT_XRGB8888_LE, INPUT_EVENT_V1_LEN, encode_buffer_desc, encode_damage_rect,
        meta_rights, sys_buffer_commit_damage, sys_buffer_create, sys_buffer_map, sys_fb_present,
        sys_grant, sys_input_focus_create, sys_input_focus_mint_activator, sys_input_focus_recv,
    };

    const WIDTH: usize = 32;
    const HEIGHT: usize = 32;
    const STRIDE_BYTES: usize = WIDTH * 4;
    const BUFFER_BYTES: u64 = (WIDTH * HEIGHT * 4) as u64;
    const NS: u64 = 0x4457_3100_0000_0001;
    const BUFFER_CAP_PROVENANCE: u64 = 0x4457_3142_5546_0001;

    fn send_request(channel: Handle, request_id: u64, request: Request) -> bool {
        let (bytes, len) = encode_request(request_id, request);
        sys_send(channel, &bytes[..len]) == len as i64
    }

    fn recv_reply(channel: Handle, opcode: u16, request_id: u64) -> Result<Identity, i32> {
        let mut bytes = [0u8; REPLY_LEN];
        if sys_recv(channel, &mut bytes) != REPLY_LEN as i64 {
            return Err(1);
        }
        let reply = decode_reply(&bytes).map_err(|_| 2)?;
        if reply.opcode != opcode || reply.request_id != request_id || reply.status != 0 {
            return Err(3);
        }
        Ok(reply.identity)
    }

    fn recv_surface_cap(session: Handle) -> Result<Handle, i32> {
        let mut byte = [0u8; 1];
        if sys_recv(session, &mut byte) != 1 || byte[0] != b'O' {
            return Err(1);
        }
        if sys_send(session, b"R") != 1 {
            return Err(2);
        }
        let surface =
            libneo::sys_recv_grant(session, libneo::FULL_CHANNEL_RIGHTS).map_err(|_| 3)?;
        if sys_send(session, b"A") != 1 {
            return Err(4);
        }
        Ok(surface)
    }

    let mut tag = [0u8; 1];
    if sys_recv(MAIN, &mut tag) != 1 || (tag[0] != b'A' && tag[0] != b'B' && tag[0] != b'C') {
        return 100;
    }
    let (fill, event_pixel, keycode) = if tag[0] == b'A' {
        (0x00D1_3344, 0x00F0_A011, 0x04)
    } else if tag[0] == b'B' {
        (0x0044_88D1, 0x000B_EEF1, 0x05)
    } else {
        (0x0062_C878, 0x00C0_ffee, 0x06)
    };

    let create_identity = Identity {
        namespace_id: NS,
        surface_id: 0,
        surface_cap_provenance: 0,
    };
    if !send_request(
        MAIN,
        1,
        Request::Create(CreateSurface {
            identity: create_identity,
            role: ROLE_TEXT,
            width: WIDTH as u32,
            height: HEIGHT as u32,
            client_tag: tag[0] as u64,
        }),
    ) {
        return 101;
    }
    let identity = match recv_reply(MAIN, OP_CREATE_SURFACE, 1) {
        Ok(identity) => identity,
        Err(code) => return 102 + code,
    };
    let surface = match recv_surface_cap(MAIN) {
        Ok(surface) => surface,
        Err(code) => return 106 + code,
    };

    let desc = encode_buffer_desc(
        WIDTH as u32,
        HEIGHT as u32,
        STRIDE_BYTES as u32,
        FORMAT_XRGB8888_LE,
        BUFFER_BYTES,
    );
    let buffer = match sys_buffer_create(&desc) {
        Ok(buffer) => buffer,
        Err(_) => return 111,
    };
    let pixels = match sys_buffer_map(buffer, 0) {
        Ok(ptr) => ptr as *mut u32,
        Err(_) => return 112,
    };
    unsafe {
        for index in 0..WIDTH * HEIGHT {
            core::ptr::write_volatile(pixels.add(index), fill);
        }
    }
    let full_damage = encode_damage_rect(0, 0, WIDTH as u16, HEIGHT as u16);
    if sys_buffer_commit_damage(buffer, &full_damage) != 0 {
        return 113;
    }
    if sys_fb_present(buffer, 220, 16) != EPERM || sys_send(MAIN, b"P") != 1 {
        return 114;
    }

    if !send_request(
        surface,
        2,
        Request::ImportAttach(ImportAttachBuffer {
            identity,
            transfer_id: 1,
            buffer_cap_provenance: BUFFER_CAP_PROVENANCE,
        }),
    ) {
        return 115;
    }
    let mut ready = [0u8; 1];
    if sys_recv(surface, &mut ready) != 1 || ready[0] != b'R' {
        return 116;
    }
    let compositor_buffer = CapRights::new(
        meta_rights::USE,
        ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
    );
    if sys_grant(surface, buffer, compositor_buffer) != 0 {
        return 117;
    }
    if recv_reply(surface, OP_IMPORT_ATTACH_BUFFER, 2).is_err() {
        return 118;
    }

    if !send_request(
        surface,
        3,
        Request::Commit(Commit {
            identity,
            client_serial: 1,
        }),
    ) || recv_reply(surface, OP_COMMIT, 3).is_err()
    {
        return 119;
    }

    let focus = match sys_input_focus_create() {
        Ok(focus) => focus,
        Err(_) => return 120,
    };
    let activator = match sys_input_focus_mint_activator(focus) {
        Ok(activator) => activator,
        Err(_) => return 121,
    };
    if !send_request(
        surface,
        4,
        Request::Focus(FocusRequest {
            identity,
            reason: 1,
            request_serial: 1,
        }),
    ) || recv_reply(surface, OP_FOCUS_REQUEST, 4).is_err()
    {
        return 122;
    }
    let activate_only = CapRights::new(meta_rights::USE, ATRIUM_FOCUS_ACTIVATE);
    if sys_grant(MAIN, activator, activate_only) != 0 || sys_send(MAIN, b"R") != 1 {
        return 123;
    }

    #[cfg(feature = "dw2-scene-policy")]
    let mut client_serial = 1u64;
    loop {
        let mut command = [0u8; 1];
        if sys_recv(MAIN, &mut command) != 1 {
            return 124;
        }
        match command[0] {
            b'K' => {
                let mut event = [0u8; INPUT_EVENT_V1_LEN];
                let mut attempts = 0usize;
                loop {
                    let rc = sys_input_focus_recv(focus, &mut event);
                    if rc == 0 {
                        break;
                    }
                    if rc != EAGAIN || attempts == 4096 {
                        return 125;
                    }
                    attempts += 1;
                    core::hint::spin_loop();
                }
                if event[0] != 1 || event[1] != 1 || event[2] != keycode {
                    return 126;
                }
                if sys_input_focus_recv(focus, &mut event) != EAGAIN {
                    return 126;
                }
                unsafe {
                    core::ptr::write_volatile(pixels, event_pixel);
                }
                #[cfg(feature = "dw2-scene-policy")]
                {
                    client_serial += 1;
                }
                #[cfg(feature = "dw2-scene-policy")]
                let commit_serial = client_serial;
                #[cfg(not(feature = "dw2-scene-policy"))]
                let commit_serial = 2;
                let one_pixel = encode_damage_rect(0, 0, 1, 1);
                if sys_buffer_commit_damage(buffer, &one_pixel) != 0
                    || !send_request(
                        surface,
                        5,
                        Request::Commit(Commit {
                            identity,
                            client_serial: commit_serial,
                        }),
                    )
                    || recv_reply(surface, OP_COMMIT, 5).is_err()
                    || sys_send(MAIN, b"D") != 1
                {
                    return 127;
                }
            }
            b'N' => {
                let mut event = [0xA5u8; INPUT_EVENT_V1_LEN];
                if sys_input_focus_recv(focus, &mut event) != EPERM
                    || event != [0xA5u8; INPUT_EVENT_V1_LEN]
                    || sys_send(MAIN, b"N") != 1
                {
                    return 128;
                }
            }
            #[cfg(feature = "dw2-scene-policy")]
            b'C' => {
                let mut bytes = [0u8; libneo::atrium_protocol::MAX_MESSAGE_LEN];
                let len = sys_recv(surface, &mut bytes);
                let (header, configure) = match if len > 0 {
                    decode_request(&bytes[..len as usize])
                } else {
                    Err(libneo::atrium_protocol::ProtocolError::BadLength)
                } {
                    Ok((header, Request::Configure(configure))) => (header, configure),
                    _ => return 131,
                };
                if header.opcode != OP_CONFIGURE
                    || header.request_id != 7
                    || configure.identity != identity
                    || configure.width != 24
                    || configure.height != 24
                    || configure.configure_serial != 1
                {
                    return 132;
                }
                let reply = encode_reply(Reply {
                    opcode: OP_CONFIGURE,
                    request_id: header.request_id,
                    status: 0,
                    identity,
                });
                if sys_send(surface, &reply) != REPLY_LEN as i64 || sys_send(MAIN, b"C") != 1 {
                    return 133;
                }
            }
            b'X' => {
                #[cfg(feature = "dw2-scene-policy")]
                let final_client_serial = client_serial;
                #[cfg(not(feature = "dw2-scene-policy"))]
                let final_client_serial = 2;
                if !send_request(
                    surface,
                    6,
                    Request::Destroy(DestroySurface {
                        identity,
                        final_client_serial,
                    }),
                ) || recv_reply(surface, OP_DESTROY_SURFACE, 6).is_err()
                {
                    return 129;
                }
                return 0;
            }
            _ => return 130,
        }
    }
}

/// Atrium G1 slice-3 CLIENT (buffer owner). Spawned by neo-init (the
/// compositor) with the surface channel at Handle(0). Creates a shared pixel
/// buffer, paints a known pattern, commits, grants a strict read-only sibling
/// cap to the compositor (the cross-process import), then EXITS — proving the
/// buffer's frames outlive the client process (the compositor still maps them,
/// no UAF). MUST stay in sync with kernel `test_harness::atrium_slice3_pixels`
/// and neo-init's `atrium_slice3_compositor_tests`.
#[cfg(feature = "atrium-slice3")]
fn atrium_slice3_client() -> i32 {
    use libneo::{
        ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, CapRights, FORMAT_XRGB8888_LE,
        encode_buffer_desc, meta_rights, sys_buffer_commit, sys_buffer_create, sys_buffer_map,
        sys_grant,
    };

    const FILL: u32 = 0x0033_9966;
    const CORNER: u32 = 0x0055_4433;

    // 64x32, stride 256 == width*4 (exactly 2 pages), so row-major u32
    // indexing is exact.
    let desc = encode_buffer_desc(64, 32, 256, FORMAT_XRGB8888_LE, 8192);
    let buffer = match sys_buffer_create(&desc) {
        Ok(h) => h,
        Err(_) => return 80,
    };
    // Map RW (client role) and paint the known pattern.
    let va = match sys_buffer_map(buffer, 0) {
        Ok(p) => p as *mut u32,
        Err(_) => return 81,
    };
    unsafe {
        for px in 0..(64 * 32) {
            *va.add(px) = FILL;
        }
        *va = CORNER;
    }
    if sys_buffer_commit(buffer) != 0 {
        return 82;
    }
    // Grant the compositor a strict read-only sibling cap — MAP_READ|QUERY|
    // PRESENT, never MAP_WRITE/COMMIT (the C4/C3 attenuation). USE meta only,
    // so the compositor cannot re-grant. Blocks until neo-init recv_grants.
    let comp_role = CapRights::new(
        meta_rights::USE,
        ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
    );
    if sys_grant(MAIN, buffer, comp_role) != 0 {
        return 83;
    }
    // Exit: tears down our RW mapping (Pmap::Drop removes PTEs, TLB-shoots
    // down, releases our buffer Arc pin) and drops our buffer handle. The
    // compositor's cap + its mapping pin keep the frames alive; our exit is
    // the peer-death (EPIPE) signal it waits on before compositing.
    0
}

#[cfg(feature = "neo-shell-repl")]
fn neo_shell_repl_worker_main() -> i32 {
    let mut buf = [0u8; 64];
    let n = sys_recv(MAIN, &mut buf);
    if n < 0 {
        return 1;
    }
    let n = n as usize;
    if &buf[..n] != b"neo-shell:ping" {
        return 2;
    }
    if sys_send(MAIN, b"neo-shell:pong") < 0 {
        return 3;
    }
    0
}

#[cfg(feature = "arthur-action-surface")]
fn arthur_action_surface_worker_main() -> i32 {
    let mut buf = [0u8; 32];
    let n = sys_recv(MAIN, &mut buf);
    if n < 0 || &buf[..n as usize] != b"arthur-action:ping" {
        return 1;
    }
    if sys_send(MAIN, b"arthur-action:pong") < 0 {
        return 2;
    }
    0
}

#[cfg(any(
    feature = "cap-mint-and-use",
    feature = "cap-derive-subset",
    feature = "cap-grant-basic",
    feature = "cap-grant-missing-meta",
    feature = "cap-revoke-cross-process",
    feature = "cap-revoke-descendants-walk",
    feature = "cap-revoke-spawn-inheritance-descendants",
    feature = "cap-survive-exit",
    feature = "cap-graph-revoke",
    feature = "file-cap-derive-grant"
))]
fn cap_worker_main() -> i32 {
    #[cfg(any(feature = "cap-mint-and-use", feature = "cap-derive-subset"))]
    {
        return cap_worker_echo();
    }

    #[cfg(feature = "cap-grant-basic")]
    {
        return cap_worker_grant_basic();
    }

    #[cfg(feature = "cap-grant-missing-meta")]
    {
        return cap_worker_grant_missing_meta();
    }

    #[cfg(feature = "cap-revoke-cross-process")]
    {
        return cap_worker_revoke_cross_process(false);
    }

    #[cfg(feature = "cap-revoke-descendants-walk")]
    {
        return cap_worker_revoke_cross_process(true);
    }

    #[cfg(feature = "cap-revoke-spawn-inheritance-descendants")]
    {
        return cap_worker_revoke_spawn_inheritance_descendants();
    }

    #[cfg(feature = "cap-survive-exit")]
    {
        return cap_worker_survive_exit_donor();
    }

    #[cfg(feature = "cap-graph-revoke")]
    {
        return cap_worker_cap_graph_exit_launder();
    }

    #[cfg(feature = "file-cap-derive-grant")]
    {
        return cap_worker_file_derive_grant();
    }
}

/// OT-61 C2.2 (`mem_quota`). One role per spawn, read from the channel at
/// handle 0; each report is one byte, 0 or the failing step.
/// 1. Budgeted: map 1 MiB regions until the account refuses. The refusal is
///    `ENOMEM`, `used` never passes `limit`, and nearly the whole budget was
///    granted first. Report, then hold the memory until init says "go".
/// 2. Unbudgeted, while role 1 holds its budget: map and touch 16 MiB.
/// 3. Grow a heap `Vec` to 8 MiB. Budgeted at 4 MiB this ends in libneo's
///    allocation-failure exit (137) and only this process dies; the
///    unbudgeted control reaches 8 MiB and exits 0.
/// 4. Budgeted at 12 MiB: spawn two role-5 grandchildren at 8 MiB each. The
///    first trips at its own limit; the second trips at this account's,
///    below its own. Report 0 when both say so.
/// 5. (Grandchild.) Map until refused; report 1 if the trip came at this
///    account's own limit, 2 if below it (an ancestor's); hold until "go".
/// 6. Budgeted: tensor storage is refused (`EPERM`).
#[cfg(feature = "mem-quota-test")]
fn mem_quota_worker() -> i32 {
    use libneo::{
        ENOMEM, EPERM, TENSOR_DTYPE_F32, TENSOR_MAX_RANK, TensorCreateDesc, sys_chan_create,
        sys_map_region, sys_mem_info, sys_spawn_budgeted, sys_tensor_create, sys_wait,
    };
    const MIB: usize = 1 << 20;
    let report = |code: u8| {
        sys_send(MAIN, &[code]);
    };
    let mut buf = [0u8; 8];
    if sys_recv(MAIN, &mut buf) < 1 {
        return 90;
    }
    // Map 1 MiB regions until refused; returns (MiB granted, the error).
    let fill = || -> (usize, i64) {
        let mut got = 0usize;
        loop {
            match sys_map_region(MIB) {
                Ok(p) => {
                    // Touch every page: the frames are real.
                    for off in (0..MIB).step_by(4096) {
                        // SAFETY: `p` is a fresh RW mapping of MIB bytes.
                        unsafe { p.add(off).write_volatile(1) };
                    }
                    got += 1;
                }
                Err(e) => return (got, e),
            }
            if got > 64 {
                return (got, 0);
            }
        }
    };
    let hold = || {
        let mut b = [0u8; 8];
        sys_recv(MAIN, &mut b);
    };
    match buf[0] {
        1 => {
            let Ok(before) = sys_mem_info() else {
                report(1);
                return 1;
            };
            if before.flags & libneo::mem_info::FLAG_OWN_BUDGET == 0 {
                report(2);
                return 2;
            }
            let room = before.limit_frames - before.used_frames;
            let (got, err) = fill();
            if err != ENOMEM {
                report(3);
                return 3;
            }
            let Ok(after) = sys_mem_info() else {
                report(4);
                return 4;
            };
            if after.used_frames > after.limit_frames {
                report(5);
                return 5;
            }
            // Granted all but less than one region (plus a few table frames).
            if (got as u64) * 256 + 256 + 16 < room {
                report(6);
                return 6;
            }
            report(0);
            hold();
            0
        }
        2 => {
            let (got, err) = fill();
            let _ = err;
            if got < 16 {
                report(10);
                return 10;
            }
            report(0);
            0
        }
        3 => {
            let mut v: Vec<u8> = Vec::new();
            for step in 1..=32 {
                v.resize(step * 256 * 1024, 1);
            }
            if v.len() != 8 * MIB {
                return 20;
            }
            0
        }
        4 => {
            let mut kids = [(Handle(0), libneo::ProcessHandle(0)); 2];
            let mut verdicts = [0u8; 2];
            for (i, kid) in kids.iter_mut().enumerate() {
                let Ok((mine, theirs)) = sys_chan_create() else {
                    report(30);
                    return 30;
                };
                let Ok((_, ph)) = sys_spawn_budgeted("worker", &[theirs], (8 * MIB) as u64) else {
                    report(31);
                    return 31;
                };
                sys_send(mine, &[5]);
                let mut b = [0u8; 8];
                if sys_recv(mine, &mut b) < 1 {
                    report(32);
                    return 32;
                }
                verdicts[i] = b[0];
                *kid = (mine, ph);
            }
            for (mine, ph) in kids {
                sys_send(mine, b"go");
                if sys_wait(ph, 0) != Ok(0) {
                    report(33);
                    return 33;
                }
            }
            // First: its own limit. Second: the parent's, below its own.
            if verdicts != [1, 2] {
                report(34 + verdicts[1]);
                return 34;
            }
            report(0);
            0
        }
        5 => {
            let (_, err) = fill();
            let Ok(info) = sys_mem_info() else {
                report(40);
                return 40;
            };
            if err != ENOMEM {
                report(41);
                return 41;
            }
            // Own-limit trip: the next 1 MiB region did not fit this limit.
            report(if info.used_frames + 256 > info.limit_frames {
                1
            } else {
                2
            });
            hold();
            0
        }
        6 => {
            let data = [0u8; 16];
            let mut dims = [0u64; TENSOR_MAX_RANK];
            dims[0] = 4;
            let desc = TensorCreateDesc::new(1, TENSOR_DTYPE_F32, false, dims, &data);
            match sys_tensor_create(&desc) {
                Err(e) if e == EPERM => report(0),
                _ => report(50),
            }
            0
        }
        _ => 91,
    }
}

/// OT-66 K1a (`cap_rooted_open`). With one inherited handle (no RootDirCap):
/// the root form must be EPERM. With a second, a derived read-only
/// RootDirCap: it must show the ROOT_DIR flag, a read open through it works,
/// a write open is EACCES; then, after init revokes the cap, the file opened
/// through it is EBADF and the root form is EPERM. Each report is one byte,
/// 0 or the failing step.
#[cfg(feature = "cap-rooted-open-test")]
fn cap_rooted_open_worker() -> i32 {
    use libneo::{EACCES, EBADF, EPERM, FileRights, OPEN_ROOT_TEMPORARY, sys_open, sys_read_fd};
    const SCRATCH: &str = "/scratch";
    let report = |code: u8| {
        sys_send(MAIN, &[code]);
    };
    let mut buf = [0u8; 8];
    let Ok(info) = libneo::sys_handle_info(BLOCK.0) else {
        // No second handle: the role without a cap.
        report(
            match sys_open(SCRATCH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
                Err(e) if e == EPERM => 0,
                Err(_) => 1,
                Ok(_) => 2,
            },
        );
        return 0;
    };
    if info.flags & libneo::handle_info::FLAG_ROOT_DIR == 0 || libneo::root_dir_cap() != Some(BLOCK)
    {
        report(10);
        return 0;
    }
    let file = match sys_open(SCRATCH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
        Ok(f) => f,
        Err(_) => {
            report(11);
            return 0;
        }
    };
    if sys_read_fd(file.0, &mut buf) < 0 {
        report(12);
        return 0;
    }
    match sys_open(SCRATCH, OPEN_ROOT_TEMPORARY, FileRights::WRITE) {
        Err(e) if e == EACCES => {}
        _ => {
            report(13);
            return 0;
        }
    }
    report(0);
    if sys_recv(MAIN, &mut buf) < 0 {
        return 1;
    }
    if sys_read_fd(file.0, &mut buf) != EBADF {
        report(14);
        return 0;
    }
    match sys_open(SCRATCH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
        Err(e) if e == EPERM => report(0),
        _ => report(15),
    }
    0
}

/// File-capability unification (I-15) grantee. Receives a read-only file cap
/// granted by neo-init, confirms it can read but NOT write (live attenuation
/// against the real kernel — the write is denied EACCES because the granted
/// handle carries READ only), then reports "ok".
#[cfg(feature = "file-cap-derive-grant")]
fn cap_worker_file_derive_grant() -> i32 {
    if wait_for_go() != 0 {
        return 1;
    }
    let granted = match sys_recv_grant(MAIN, FULL_CHANNEL_RIGHTS) {
        Ok(h) => h,
        Err(_) => return 2,
    };
    let mut buf = [0u8; 8];
    // read is permitted (>= 0)
    if sys_read_fd(granted.0, &mut buf) < 0 {
        return 3;
    }
    // write is denied — the granted cap is read-only (EACCES)
    if sys_write(granted.0, b"x".as_ptr(), 1) != EACCES {
        return 4;
    }
    if sys_send(MAIN, b"ok") < 0 {
        return 5;
    }
    0
}

#[cfg(any(feature = "cap-mint-and-use", feature = "cap-derive-subset"))]
fn cap_worker_echo() -> i32 {
    let mut buf = [0u8; 16];
    if sys_recv(MAIN, &mut buf) < 0 {
        return 1;
    }
    if sys_send(MAIN, b"pong") < 0 {
        return 2;
    }
    0
}

#[cfg(feature = "cap-grant-basic")]
fn cap_worker_grant_basic() -> i32 {
    if wait_for_go() != 0 {
        return 1;
    }
    let granted = match sys_recv_grant(MAIN, FULL_CHANNEL_RIGHTS) {
        Ok(h) => h,
        Err(_) => return 2,
    };
    let mut buf = [0u8; 8];
    if sys_recv(granted, &mut buf) < 0 {
        return 3;
    }
    0
}

#[cfg(feature = "cap-grant-missing-meta")]
fn cap_worker_grant_missing_meta() -> i32 {
    if wait_for_go() != 0 {
        return 1;
    }
    if sys_send(MAIN, b"ready") < 0 {
        return 2;
    }
    match sys_recv_grant(MAIN, FULL_CHANNEL_RIGHTS) {
        Err(e) if e == EPIPE => 0,
        _ => 3,
    }
}

#[cfg(any(
    feature = "cap-revoke-cross-process",
    feature = "cap-revoke-descendants-walk",
    feature = "cap-revoke-spawn-inheritance-descendants"
))]
fn cap_worker_revoke_cross_process(derive_child: bool) -> i32 {
    if wait_for_go() != 0 {
        return 1;
    }
    let granted = match sys_recv_grant(MAIN, FULL_CHANNEL_RIGHTS) {
        Ok(h) => h,
        Err(_) => return 2,
    };
    let child = if derive_child {
        match libneo::sys_derive(granted, FULL_CHANNEL_RIGHTS) {
            Ok(h) => Some(h),
            Err(_) => return 3,
        }
    } else {
        None
    };
    if sys_send(MAIN, b"ready") < 0 {
        return 4;
    }
    if wait_for_go() != 0 {
        return 5;
    }
    if sys_send(granted, b"x") != EBADF {
        return 6;
    }
    if let Some(child) = child {
        if sys_send(child, b"x") != EBADF {
            return 7;
        }
    }
    0
}

#[cfg(feature = "cap-revoke-spawn-inheritance-descendants")]
fn cap_worker_revoke_spawn_inheritance_descendants() -> i32 {
    let child = match sys_derive(
        BLOCK,
        CapRights::new(meta_rights::USE, channel_rights::SEND),
    ) {
        Ok(h) => h,
        Err(_) => return 1,
    };
    if sys_send(MAIN, b"ready") < 0 {
        return 2;
    }
    if wait_for_go() != 0 {
        return 3;
    }
    if sys_send(BLOCK, b"x") != EBADF {
        return 4;
    }
    if sys_send(child, b"x") != EBADF {
        return 5;
    }
    if sys_send(MAIN, b"done") < 0 {
        return 6;
    }
    0
}

/// K1a-0 exit-laundering leg (`cap_graph_revoke`): derive a child of the
/// inherited cap (whose parent is init's), grant it back to init, then exit
/// when told to (leg 3) or wait to be killed (leg 4). The grant's edge points
/// at a cap of this process; once it is gone init revokes the root, and the
/// granted copy must die with it (CP-88).
#[cfg(feature = "cap-graph-revoke")]
fn cap_worker_cap_graph_exit_launder() -> i32 {
    let rights = libneo::CapRights::new(
        libneo::meta_rights::USE | libneo::meta_rights::DERIVE | libneo::meta_rights::GRANT,
        libneo::channel_rights::SEND,
    );
    let child = match libneo::sys_derive(BLOCK, rights) {
        Ok(h) => h,
        Err(_) => {
            // The spawn twin (one inherited handle, no BLOCK): stay alive
            // holding the inherited cap, so the kernel can see whether it
            // survived. It carries SEND only, so block on a channel of our
            // own (nobody ever sends on it).
            let mut buf = [0u8; 8];
            if let Ok((park, _keep)) = libneo::sys_chan_create() {
                let _ = sys_recv(park, &mut buf);
            }
            return 0;
        }
    };
    if libneo::sys_grant(MAIN, child, rights) != 0 {
        return 2;
    }
    // Leg 3 sends "exit"; leg 4 kills this process while it waits here.
    let mut buf = [0u8; 8];
    if sys_recv(MAIN, &mut buf) < 0 {
        return 3;
    }
    0
}

#[cfg(feature = "cap-survive-exit")]
fn cap_worker_survive_exit_donor() -> i32 {
    if wait_for_go() != 0 {
        return 1;
    }
    let offered = CapRights::new(
        libneo::meta_rights::USE | libneo::meta_rights::DERIVE | libneo::meta_rights::GRANT,
        libneo::channel_rights::SEND,
    );
    if sys_grant(MAIN, BLOCK, offered) != 0 {
        return 2;
    }
    0
}

#[cfg(any(
    feature = "cap-grant-basic",
    feature = "cap-grant-missing-meta",
    feature = "cap-revoke-cross-process",
    feature = "cap-revoke-descendants-walk",
    feature = "cap-revoke-spawn-inheritance-descendants",
    feature = "cap-survive-exit",
    feature = "file-cap-derive-grant",
    feature = "sys-wait-already-exited",
    feature = "sys-wait-multiple-waiters",
    feature = "sys-inspect-exited"
))]
fn wait_for_go() -> i32 {
    let mut buf = [0u8; 8];
    if sys_recv(MAIN, &mut buf) < 0 { 1 } else { 0 }
}

#[cfg(any(
    feature = "sys-wait-blocking",
    feature = "sys-wait-wnohang",
    feature = "sys-wait-already-exited",
    feature = "sys-wait-multiple-waiters",
    feature = "sys-inspect-running",
    feature = "sys-inspect-exited",
    feature = "sys-inspect-rights-gate"
))]
fn process_cap_worker_main() -> ! {
    #[cfg(feature = "sys-wait-blocking")]
    {
        for _ in 0..1_000_000 {
            core::hint::spin_loop();
        }
        sys_exit(41);
    }

    #[cfg(any(
        feature = "sys-wait-wnohang",
        feature = "sys-inspect-running",
        feature = "sys-inspect-rights-gate"
    ))]
    {
        loop {
            core::hint::spin_loop();
        }
    }

    #[cfg(feature = "sys-wait-already-exited")]
    {
        if wait_for_go() != 0 {
            sys_exit(100);
        }
        sys_exit(42);
    }

    #[cfg(feature = "sys-wait-multiple-waiters")]
    {
        if wait_for_go() != 0 {
            sys_exit(100);
        }
        sys_exit(43);
    }

    #[cfg(feature = "sys-inspect-exited")]
    {
        if wait_for_go() != 0 {
            sys_exit(100);
        }
        sys_exit(44);
    }
}

fn default_worker_main() -> ! {
    let _ = sys_write(1, HELLO.as_ptr(), HELLO.len());

    // Heap-allocated receive buffer. First allocation forces LibneoHeap's
    // lazy bootstrap (sys_map_region for the initial 256 KiB region +
    // Heap::init). Anti-regression for ADR-006.
    let mut recv_buf: Vec<u8> = alloc::vec![0u8; 64];
    let n = sys_recv(MAIN, &mut recv_buf);
    if n < 0 {
        let _ = sys_write(1, RECV_FAIL.as_ptr(), RECV_FAIL.len());
        sys_exit(1);
    }
    let _ = sys_write(1, RECV_PING_OK.as_ptr(), RECV_PING_OK.len());
    let _ = sys_write(1, b"[ neo-worker ] payload: ".as_ptr(), 24);
    let _ = sys_write(1, recv_buf.as_ptr(), n as usize);
    let _ = sys_write(1, b"\n".as_ptr(), 1);

    // Heap-built pong (mixed bulk + per-byte to exercise both Vec growth paths).
    let mut pong: Vec<u8> = Vec::new();
    pong.extend_from_slice(b"pong-from-heap-");
    for i in 0..8u8 {
        pong.push(b'A' + i);
    }
    let n = sys_send(MAIN, &pong);
    if n < 0 {
        let _ = sys_write(1, SEND_FAIL.as_ptr(), SEND_FAIL.len());
        sys_exit(2);
    }

    // === Synchronization: receive "go-block" on chan_block. ===
    // After this returns, transition immediately to the blocking recv on
    // chan_main. Do not put serial I/O in this window: a timer interrupt
    // between the rendezvous wake and the next recv would let the parent
    // observe us as Ready instead of RecvBlocked.
    let mut sync_buf: Vec<u8> = alloc::vec![0u8; 32];
    let n = sys_recv(BLOCK, &mut sync_buf);
    if n < 0 {
        let _ = sys_write(1, RECV_FAIL.as_ptr(), RECV_FAIL.len());
        sys_exit(3);
    }
    // === This recv is the SC-4 Q8.c blocked-kill target. ===
    // chan_main side A is Idle. We block here. Init runs sys_kill(worker_handle);
    // the kernel does not find us in the ready queue, walks our drained
    // channel handles, extracts this parked Box<Thread>, and zombifies it.
    let n = sys_recv(MAIN, &mut sync_buf);
    if n < 0 {
        let _ = sys_write(1, RECV_FAIL.as_ptr(), RECV_FAIL.len());
        sys_exit(4);
    }
    let _ = sys_write(1, RECV_UNBLOCK_OK.as_ptr(), RECV_UNBLOCK_OK.len());

    // ADR-007 §Q3: stay Ready until the parent kills us. `spin_loop`
    // emits a PAUSE — preemption keeps us visible to `sys_kill`'s
    // ready-queue scan. We never reach sys_exit; the reaper does.
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(feature = "phase4-https-fetch")]
mod phase4_https_worker {
    use libneo::{
        EAGAIN, Handle, NetDialCap, SockAddr, SockAddrIn6, sys_net_accept_tcp,
        sys_net_listen_tcp_with_cap, sys_send, sys_tcp_close, sys_write,
    };

    const READY: Handle = Handle(0);
    const LISTEN_CAP: NetDialCap = NetDialCap(Handle(1));

    pub fn run() -> i32 {
        let addr = SockAddr::Inet6(SockAddrIn6::new_loopback(libneo::tls::PHASE4_TEST_PORT));
        let listener = match sys_net_listen_tcp_with_cap(LISTEN_CAP, &addr, 1) {
            Ok(listener) => listener,
            Err(_) => return 91,
        };
        if sys_send(READY, b"ready") != 5 {
            return 92;
        }

        let socket = match accept_one(listener) {
            Ok(socket) => socket,
            Err(code) => return code,
        };
        let result = libneo::tls::serve_https_once(
            socket,
            libneo::tls::PHASE4_TEST_CERT_CHAIN,
            libneo::tls::PHASE4_TEST_KEY_PKCS8,
            libneo::tls::PHASE4_TEST_BODY,
        );
        let _ = sys_tcp_close(socket);
        match result {
            Ok(()) => {
                let msg = b"[ neo-worker ] phase4_https_fetch served tls body\n";
                let _ = sys_write(1, msg.as_ptr(), msg.len());
                0
            }
            Err(err) => {
                let msg = b"[ neo-worker ] phase4_https_fetch tls server error=";
                let _ = sys_write(1, msg.as_ptr(), msg.len());
                let code = [err.code(), b'\n'];
                let _ = sys_write(1, code.as_ptr(), code.len());
                94
            }
        }
    }

    fn accept_one(listener: libneo::TcpListenerHandle) -> Result<libneo::TcpSocketHandle, i32> {
        // The client now runs the complete legacy-egress negative-control set
        // after READY and before its authorized dial. Keep the fixture listener
        // alive across that deliberately wider scheduling window.
        for _ in 0..4096 {
            match sys_net_accept_tcp(listener) {
                Ok(socket) => return Ok(socket),
                Err(EAGAIN) => core::hint::spin_loop(),
                Err(_) => return Err(93),
            }
        }
        Err(93)
    }
}

/// Phase-B cap-bounded vault process (ADR-075). Spawned by neo-init with a
/// single inherited handle — its operation channel at `Handle(0)` — and **no
/// network capability**. It holds the master key and the encrypted store and
/// serves `unlock`/`store`/`retrieve`/`lock` (plus scenario-only test verbs)
/// until told to shut down. A retrieved secret is written into a caller-offered
/// **send-only** least-rights grant; the master key and ciphertext store never
/// cross the boundary.
#[cfg(feature = "vault-phase-b")]
mod vault_worker {
    use alloc::vec;
    use alloc::vec::Vec;
    use libneo::{
        CapRights, EPERM, Handle, channel_rights, meta_rights, sys_channel_close, sys_getrandom,
        sys_recv, sys_recv_grant, sys_send,
    };
    use vault_core::protocol::{self, Header};
    use vault_core::record::{self, Header as RecordHeader};
    use vault_core::session::{Rng, SECRET_ID_LEN, Vault, VaultError};
    use zeroize::Zeroize;

    /// The operation channel — the vault's sole inherited handle.
    const OP: Handle = Handle(0);
    const MAX_PAYLOAD: usize = vault_core::session::MAX_SECRET_LEN;
    const MAX_MSG: usize = protocol::HEADER_LEN + SECRET_ID_LEN + MAX_PAYLOAD;
    /// The least-rights ceiling the vault accepts for a retrieve output grant:
    /// USE + SEND only — it can write the secret and nothing else (no recv, no
    /// re-derive, no re-grant, no revoke).
    const OUTPUT_GRANT_RIGHTS: CapRights = CapRights::new(meta_rights::USE, channel_rights::SEND);

    /// The vault's CSPRNG: `SYS_GETRANDOM` only, **no** RDRAND fallback
    /// (ADR-075 §3 — a getrandom failure is fatal to the vault, never silently
    /// downgraded to a weaker source).
    struct KernelRng;
    impl Rng for KernelRng {
        fn fill(&mut self, buf: &mut [u8]) -> bool {
            let n = sys_getrandom(buf);
            n >= 0 && n as usize == buf.len()
        }
    }

    fn send_response(opcode: u16, request_id: u64, status: i32, payload: &[u8]) -> bool {
        let mut msg = Vec::with_capacity(protocol::HEADER_LEN + payload.len());
        msg.extend_from_slice(
            &Header::response(opcode, request_id, status, payload.len() as u32).encode(),
        );
        msg.extend_from_slice(payload);
        sys_send(OP, &msg) >= 0
    }

    pub fn run() -> i32 {
        let mut vault = Vault::new();
        let mut rng = KernelRng;
        let mut buf = vec![0u8; MAX_MSG];

        loop {
            let n = sys_recv(OP, &mut buf);
            if n < 0 {
                return 20; // operation channel closed / peer gone
            }
            let n = n as usize;

            let header = match Header::decode(&buf[..n]) {
                Some(h) if h.kind == protocol::KIND_REQUEST => h,
                _ => {
                    send_response(0, 0, protocol::STATUS_BAD_REQUEST, &[]);
                    buf[..n].zeroize();
                    continue;
                }
            };
            let opcode = header.opcode;
            let rid = header.request_id;
            let body_start = protocol::HEADER_LEN;
            let plen = header.payload_len as usize;
            if body_start.saturating_add(plen) > n {
                // Declared payload longer than what actually arrived.
                send_response(opcode, rid, protocol::STATUS_BAD_REQUEST, &[]);
                buf[..n].zeroize();
                continue;
            }
            let body = body_start..body_start + plen;

            match opcode {
                protocol::OP_UNLOCK => {
                    let mut password = buf[body].to_vec();
                    match vault.unlock(&mut password, &mut rng) {
                        Ok(()) => {
                            let mut sid = [0u8; 8];
                            let session_id = if rng.fill(&mut sid) {
                                u64::from_le_bytes(sid) | 1
                            } else {
                                1
                            };
                            send_response(
                                opcode,
                                rid,
                                protocol::STATUS_OK,
                                &session_id.to_le_bytes(),
                            );
                        }
                        Err(e) => {
                            send_response(opcode, rid, protocol::status_for(e), &[]);
                        }
                    }
                    password.zeroize();
                }
                protocol::OP_STORE => {
                    if plen < SECRET_ID_LEN {
                        send_response(opcode, rid, protocol::STATUS_BAD_REQUEST, &[]);
                    } else {
                        let mut secret_id = [0u8; SECRET_ID_LEN];
                        secret_id.copy_from_slice(&buf[body_start..body_start + SECRET_ID_LEN]);
                        let mut plaintext =
                            buf[body_start + SECRET_ID_LEN..body_start + plen].to_vec();
                        let status = match vault.store(&secret_id, &mut plaintext, &mut rng) {
                            Ok(()) => protocol::STATUS_OK,
                            Err(e) => protocol::status_for(e),
                        };
                        plaintext.zeroize();
                        send_response(opcode, rid, status, &[]);
                    }
                }
                protocol::OP_RETRIEVE => {
                    let mut secret_id = [0u8; SECRET_ID_LEN];
                    if plen < SECRET_ID_LEN {
                        send_response(opcode, rid, protocol::STATUS_BAD_REQUEST, &[]);
                    } else {
                        secret_id.copy_from_slice(&buf[body_start..body_start + SECRET_ID_LEN]);
                        handle_retrieve(&vault, rid, &secret_id);
                    }
                }
                protocol::OP_LOCK => {
                    vault.lock();
                    send_response(opcode, rid, protocol::STATUS_OK, &[]);
                }
                protocol::OP_TEST_CORRUPT_HEADER => {
                    let status = if plen < SECRET_ID_LEN + 4 {
                        protocol::STATUS_BAD_REQUEST
                    } else {
                        let mut secret_id = [0u8; SECRET_ID_LEN];
                        secret_id.copy_from_slice(&buf[body_start..body_start + SECRET_ID_LEN]);
                        let o = body_start + SECRET_ID_LEN;
                        let off = u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]])
                            as usize;
                        match vault.corrupt_record_header_for_test(&secret_id, off) {
                            Ok(()) => protocol::STATUS_OK,
                            Err(e) => protocol::status_for(e),
                        }
                    };
                    send_response(opcode, rid, status, &[]);
                }
                protocol::OP_TEST_INJECT_WEAK => {
                    let status = if plen < SECRET_ID_LEN {
                        protocol::STATUS_BAD_REQUEST
                    } else {
                        let mut secret_id = [0u8; SECRET_ID_LEN];
                        secret_id.copy_from_slice(&buf[body_start..body_start + SECRET_ID_LEN]);
                        match inject_weak(&mut vault, &secret_id) {
                            Ok(()) => protocol::STATUS_OK,
                            Err(e) => protocol::status_for(e),
                        }
                    };
                    send_response(opcode, rid, status, &[]);
                }
                protocol::OP_TEST_SHUTDOWN => {
                    // The caller's send already rendezvoused with our recv; exit
                    // cleanly without a response.
                    buf[..n].zeroize();
                    return 0;
                }
                _ => {
                    send_response(opcode, rid, protocol::STATUS_BAD_REQUEST, &[]);
                }
            }
            buf[..n].zeroize();
        }
    }

    /// Retrieve `secret_id` into a caller-offered least-rights output grant.
    ///
    /// Protocol: the caller has already sent the RETRIEVE request and now offers
    /// a send-only endpoint over the operation channel. The vault receives it,
    /// **proves it is send-only** (a `sys_recv` on it must be denied `EPERM`),
    /// writes exactly the plaintext into it (or an empty message on any failure,
    /// so the caller's single receive always unblocks), then reports the coarse
    /// status and byte count on the operation channel.
    fn handle_retrieve(vault: &Vault, rid: u64, secret_id: &[u8; SECRET_ID_LEN]) {
        let out = match sys_recv_grant(OP, OUTPUT_GRANT_RIGHTS) {
            Ok(h) => h,
            Err(_) => {
                // No grant arrived; nothing was offered to write into.
                send_response(
                    protocol::OP_RETRIEVE,
                    rid,
                    protocol::STATUS_BAD_REQUEST,
                    &[],
                );
                return;
            }
        };

        // Least-rights witness: the delivered cap must NOT permit receive.
        let mut probe = [0u8; 1];
        if sys_recv(out, &mut probe) != EPERM {
            // The grant carried more than send authority — refuse to proceed.
            // Unblock the caller's receive with a single sentinel byte.
            let _ = sys_send(out, &[0u8]);
            send_response(protocol::OP_RETRIEVE, rid, protocol::STATUS_INTERNAL, &[]);
            let _ = sys_channel_close(out);
            return;
        }

        let mut plaintext = vec![0u8; MAX_PAYLOAD];
        match vault.retrieve(secret_id, &mut plaintext) {
            Ok(written) => {
                // Deliver the secret through the least-rights grant only.
                let sent = sys_send(out, &plaintext[..written]) >= 0;
                plaintext.zeroize();
                let status = if sent {
                    protocol::STATUS_OK
                } else {
                    protocol::STATUS_INTERNAL
                };
                send_response(
                    protocol::OP_RETRIEVE,
                    rid,
                    status,
                    &(written as u32).to_le_bytes(),
                );
            }
            Err(e) => {
                plaintext.zeroize();
                // Unblock the caller's receive with a single sentinel byte (no
                // plaintext), then report the coarse failure status.
                let _ = sys_send(out, &[0u8]);
                send_response(protocol::OP_RETRIEVE, rid, protocol::status_for(e), &[]);
            }
        }
        // Drop the one-shot output grant so handles don't accumulate across
        // retrieves (keeps the vault's live handle set at just the op channel).
        let _ = sys_channel_close(out);
    }

    /// Inject a record claiming sub-floor KDF parameters (memory 1 KiB), so the
    /// §F5 floor guard can be exercised on the live open path. Scenario-only.
    fn inject_weak(vault: &mut Vault, secret_id: &[u8; SECRET_ID_LEN]) -> Result<(), VaultError> {
        let header = RecordHeader {
            memory_kib: 1,
            iterations: 1,
            parallelism: 1,
            ciphertext_len: 3,
        };
        let mut body = vec![0xABu8, 0xCD, 0xEF];
        body.extend_from_slice(&[0u8; record::TAG_LEN]);
        let raw = record::assemble(
            header,
            &[0x11; record::SALT_LEN],
            &[0x22; record::NONCE_LEN],
            &body,
        )
        .map_err(|_| VaultError::BadRequest)?;
        vault.inject_raw_record_for_test(secret_id, raw)
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    sys_exit(254);
}

/// OT-93 (`kill_ready_in_kernel`): take init's "go", yield so init is parked
/// in its recv, then send. The send hands init the CPU, and the kernel's hold keeps this thread
/// Ready inside the send until init's kill of it has returned. It never
/// returns here: the kill ends it at the send's syscall return.
#[cfg(feature = "kill-ready-in-kernel-test")]
fn kill_ready_in_kernel_worker() -> i32 {
    let mut b = [0u8; 8];
    if sys_recv(MAIN, &mut b) < 1 {
        return 90;
    }
    for _ in 0..200 {
        libneo::sys_sched_yield();
    }
    sys_send(MAIN, &[0]);
    // Reached only if the kill left this thread running.
    loop {
        libneo::sys_sched_yield();
    }
}

/// The `socket_teardown` scenario. One role per spawn, read from the channel
/// at handle 0. Both roles bind UDP 127.0.0.1:7001, listen on TCP
/// 127.0.0.1:7002 and open a TCP connection from local port 7003 to
/// 127.0.0.1:7009, and close none of them. The result is 0 or the failing
/// step.
/// 1. Then report and exit.
/// 2. Then, with no report, take init's "go" and block until init kills it.
/// OT-91, the `exit_codes_bounded` roles.
/// - `C`: exit with code 7 at once.
/// - `W`: spawn a child that lives until this process's teardown, report, and
///   park in `sys_wait` on it until init kills this process (the #238
///   bounce: the killed waiter's claim must not leak).
/// - `P`: spawn 16 `C` children; wait (by inspect) until each has exited and
///   its code is kept; claim one with `sys_wait` (7); exit 0 without waiting
///   on the other 15. The teardown of this process's table must drop their
///   codes.
#[cfg(feature = "exit-codes-test")]
fn exit_codes_worker() -> i32 {
    use libneo::{ProcessHandle, sys_chan_create, sys_spawn, sys_wait};
    let mut buf = [0u8; 8];
    if sys_recv(MAIN, &mut buf) < 1 {
        return 90;
    }
    match buf[0] {
        b'C' => 7,
        b'P' => {
            let mut kids = [ProcessHandle(0); 16];
            for kid in kids.iter_mut() {
                let Ok((mine, theirs)) = sys_chan_create() else {
                    return 1;
                };
                let Ok((_, ph)) = sys_spawn("worker", &[theirs]) else {
                    return 2;
                };
                if sys_send(mine, b"C") < 0 {
                    return 3;
                }
                *kid = ph;
            }
            if !kids.iter().all(|&ph| exit_codes_exited(ph)) {
                return 4;
            }
            match sys_wait(kids[0], 0) {
                Ok(7) => 0,
                _ => 5,
            }
        }
        b'W' => {
            // A child that lives until this process's table drops its
            // channel end; then park in `sys_wait` on it until init kills
            // this process.
            let Ok((_mine, theirs)) = sys_chan_create() else {
                return 1;
            };
            let Ok((_, held)) = sys_spawn("worker", &[theirs]) else {
                return 2;
            };
            if sys_send(MAIN, b"R") < 0 {
                return 3;
            }
            let _ = sys_wait(held, 0);
            4
        }
        _ => 91,
    }
}

/// Whether `ph`'s process exits with code 7 and the code is kept (inspect
/// reports it EXITED), polling a bounded number of times.
#[cfg(feature = "exit-codes-test")]
fn exit_codes_exited(ph: libneo::ProcessHandle) -> bool {
    for _ in 0..1_000_000 {
        match libneo::sys_inspect(ph) {
            Ok(s) if s.state == libneo::PROCESS_STATE_EXITED => return s.exit_code == 7,
            Ok(_) => libneo::sys_sched_yield(),
            Err(_) => return false,
        }
    }
    false
}

/// Dexter #187 item 3a: a process without the mount authority opens `/`
/// asking for `MOUNT`, then tries to unmount `neofs` and to mount device 0.
/// Both must be `EPERM`. 0, or which step got something else (10s: the
/// unmount, 20s: the mount; +0 when it succeeded).
#[cfg(feature = "mount-authority-test")]
fn mount_authority_worker() -> i32 {
    use libneo::{
        EPERM, FileRights, MOUNT_FS_TYPE_NEOFS_DEVICE0_RO, OPEN_ROOT_TEMPORARY,
        encode_mount_request_v1, sys_mount, sys_open, sys_umount,
    };
    let mut buf = [0u8; 8];
    if sys_recv(MAIN, &mut buf) < 1 || buf[0] != b'M' {
        return 90;
    }
    let Ok(root) = sys_open(
        "/",
        OPEN_ROOT_TEMPORARY,
        FileRights::LOOKUP.union(FileRights::MOUNT),
    ) else {
        return 1;
    };
    match sys_umount(root, "neofs", 0) {
        EPERM => {}
        0 => return 10,
        _ => return 11,
    }
    let req = encode_mount_request_v1(MOUNT_FS_TYPE_NEOFS_DEVICE0_RO, "device0", "neofs-2");
    match sys_mount(root, &req, 0) {
        EPERM => 0,
        0 => 20,
        _ => 21,
    }
}

#[cfg(feature = "socket-teardown-test")]
fn socket_teardown_worker() -> i32 {
    use libneo::{
        AF_INET, SOCK_DGRAM, SYS_NET_CONNECT_TCP, SockAddr, SockAddrIn, raw3, sys_net_listen_tcp,
        sys_socket_bind_in, sys_socket_create,
    };
    let mut buf = [0u8; 8];
    if sys_recv(MAIN, &mut buf) < 1 {
        return 90;
    }
    let role = buf[0];
    fn bind_all() -> Result<(), u8> {
        let udp = sys_socket_create(AF_INET, SOCK_DGRAM).map_err(|_| 1u8)?;
        if sys_socket_bind_in(udp, &SockAddrIn::new_loopback(7001)) != 0 {
            return Err(2);
        }
        sys_net_listen_tcp(&SockAddr::Inet(SockAddrIn::new_loopback(7002)), 4).map_err(|_| 3u8)?;
        let remote = SockAddr::Inet(SockAddrIn::new_loopback(7009));
        // SAFETY: SYS_NET_CONNECT_TCP reads `len` bytes of address at `ptr`.
        let c = unsafe {
            raw3(
                SYS_NET_CONNECT_TCP,
                remote.as_ptr() as u64,
                remote.len() as u64,
                7003,
            )
        };
        if c < 0 {
            return Err(4);
        }
        Ok(())
    }
    let code = match bind_all() {
        Ok(()) => 0,
        Err(s) => s,
    };
    if role == 2 {
        // No report: a send that hands off to a parked init leaves this
        // thread Ready inside the syscall, and a kill there would free the
        // kernel stack under the send's locals. Init's "go" arrives once the
        // binds are done; then park until killed.
        let mut b = [0u8; 8];
        sys_recv(MAIN, &mut b);
        sys_recv(MAIN, &mut b);
        return i32::from(code);
    }
    sys_send(MAIN, &[code]);
    i32::from(code)
}

/// OT-61 C2.2b (`obj_quota`). One role per spawn, read from the channel at
/// handle 0; the report is one byte, 0 or the failing step. Each role measures
/// what one object costs it from `SYS_MEM_INFO` v2, so no weight is wired in.
/// 1. Budgeted: create channels until refused. The refusal is `ENOMEM`, at
///    the limit (one more pair would not fit), never past it. Report, then
///    hold the channels until init says "go".
/// 2. Budgeted: spawn role-3 grandchildren (each given a channel) until
///    refused. The refusal is `EAGAIN`, at the object limit. Report; exiting
///    closes the channels, which ends the grandchildren.
/// 3. (Grandchild.) Block in a recv until the channel closes.
#[cfg(feature = "obj-quota-test")]
fn obj_quota_worker() -> i32 {
    use libneo::{EAGAIN, ENOMEM, sys_chan_create, sys_mem_info_v2, sys_spawn};
    let mut buf = [0u8; 8];
    if sys_recv(MAIN, &mut buf) < 1 {
        return 90;
    }
    let report = |code: u8| {
        sys_send(MAIN, &[code]);
    };
    let used = || sys_mem_info_v2().map(|i| (i.obj_used_bytes, i.obj_limit_bytes));
    match buf[0] {
        1 => {
            let Ok((start, limit)) = used() else {
                report(1);
                return 1;
            };
            if limit == libneo::mem_info::UNLIMITED || start > limit {
                report(2);
                return 2;
            }
            let mut pairs = 0u64;
            let mut cost = 0u64;
            let refused = loop {
                match sys_chan_create() {
                    Ok(_) => {
                        pairs += 1;
                        if pairs == 1 {
                            cost = used().map(|(u, _)| u - start).unwrap_or(0);
                        }
                    }
                    Err(e) => break e,
                }
                if pairs > 100_000 {
                    break 0;
                }
            };
            let Ok((now, _)) = used() else {
                report(3);
                return 3;
            };
            // Refused with ENOMEM, at the limit: within one pair of it, and
            // never over it.
            let code = if refused != ENOMEM {
                4
            } else if cost == 0 || now > limit || now + cost <= limit {
                5
            } else if now - start != pairs * cost {
                6
            } else {
                0
            };
            report(code);
            let mut b = [0u8; 8];
            sys_recv(MAIN, &mut b);
            i32::from(code)
        }
        2 => {
            let Ok((start, limit)) = used() else {
                report(1);
                return 1;
            };
            let mut kids = 0u64;
            let mut first_cost = 0u64;
            let refused = loop {
                let Ok((mine, theirs)) = sys_chan_create() else {
                    break 1;
                };
                let before = used().map(|(u, _)| u).unwrap_or(0);
                match sys_spawn("worker", &[theirs]) {
                    Ok(_) => {
                        if sys_send(mine, &[3]) < 0 {
                            break 2;
                        }
                        kids += 1;
                        if kids == 1 {
                            first_cost = used().map(|(u, _)| u - before).unwrap_or(0);
                        }
                    }
                    Err(e) => break e,
                }
                if kids > 1000 {
                    break 0;
                }
            };
            let Ok((now, _)) = used() else {
                report(3);
                return 3;
            };
            // Refused with EAGAIN at the object limit: one more spawn would
            // not have fit (anything else, a full scheduler or frames, is a
            // trip somewhere else).
            let code = if refused != EAGAIN {
                4
            } else if kids == 0 || first_cost == 0 || now > limit || now + first_cost <= limit {
                5
            } else if now < start {
                6
            } else {
                0
            };
            report(code);
            i32::from(code)
        }
        3 => {
            let mut b = [0u8; 8];
            sys_recv(MAIN, &mut b);
            0
        }
        // Muse #163 item 1: one buffer mapped in a loop (5000 times, the
        // review's input). Each map's pin is charged, so the loop is refused
        // with ENOMEM at the object limit, below the per-Pmap pin cap, and
        // each map cost the same.
        4 => {
            use libneo::{
                FORMAT_XRGB8888_LE, encode_buffer_desc, sys_buffer_create, sys_buffer_map,
            };
            let Ok((start, limit)) = used() else {
                report(1);
                return 1;
            };
            if limit == libneo::mem_info::UNLIMITED || start > limit {
                report(2);
                return 2;
            }
            let desc = encode_buffer_desc(16, 16, 64, FORMAT_XRGB8888_LE, 1024);
            let Ok(buffer) = sys_buffer_create(&desc) else {
                report(3);
                return 3;
            };
            let Ok((base, _)) = used() else {
                report(3);
                return 3;
            };
            let mut maps = 0u64;
            let mut cost = 0u64;
            let refused = loop {
                match sys_buffer_map(buffer, 0) {
                    Ok(_) => {
                        maps += 1;
                        if maps == 1 {
                            cost = used().map(|(u, _)| u - base).unwrap_or(0);
                        }
                    }
                    Err(e) => break e,
                }
                if maps == 5000 {
                    break 0;
                }
            };
            let Ok((now, _)) = used() else {
                report(3);
                return 3;
            };
            // The kernel's pin cap (MAX_BORROWED_PINS): a refusal there would
            // be the cap's, not the budget's.
            const PIN_CAP: u64 = 1024;
            let code = if refused != ENOMEM {
                4
            } else if cost == 0 || now > limit || now + cost <= limit {
                5
            } else if now - base != maps * cost {
                6
            } else if maps >= PIN_CAP {
                7
            } else {
                0
            };
            report(code);
            i32::from(code)
        }
        _ => 91,
    }
}
