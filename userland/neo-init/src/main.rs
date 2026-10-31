// SPDX-License-Identifier: MPL-2.0
//! `neo-init` — the first userspace process under NeoPlutarch.
//!
//! Phase 1.5b BUILD-6 e2e smoke: a single boot exercises the full Phase 1.5b
//! surface — spawn, IPC handshake, userspace heap, capability-mediated kill,
//! AND the kill cases (kill-on-Blocked succeeds via SC-4 Q8.c; double-kill →
//! ESRCH). Per Bible Article 18.2 this becomes the canonical Phase 1.5b
//! verification scenario; the kernel's leak detector asserts the post-reap
//! frame count equals baseline.
//!
//! Two channels:
//!   - `chan_main` — bidirectional ping/pong + the final blocked recv target.
//!     keeps `h_local_main`; worker gets `h_for_worker_main` as Handle(0).
//!   - `chan_block` — init→worker only. Used to hand worker the "go-block"
//!     synchronization message that drives worker into a deterministic
//!     RecvBlocked state on `chan_main`. Worker gets it as Handle(1).
//!
//! Trace order (single-CPU, deterministic — see ADR-007 §test-scenario):
//!   1. ping → pong on chan_main (BUILD-3 happy path)
//!   2. "go-block" on chan_block — wakes worker, worker blocks on chan_main recv
//!   3. sys_kill(worker_handle) — expect 0 (worker is RecvBlocked, zombified)
//!   4. sys_kill(worker_handle) — expect ESRCH (target gone, stale handle)

#![no_std]
#![no_main]
#![cfg_attr(
    any(
        feature = "cap-mint-and-use",
        feature = "cap-derive-subset",
        feature = "cap-derive-rights-violation",
        feature = "cap-derive-missing-meta",
        feature = "cap-grant-basic",
        feature = "cap-grant-missing-meta",
        feature = "cap-revoke-local",
        feature = "cap-graph-revoke",
        feature = "cap-revoke-cross-process",
        feature = "cap-revoke-descendants-walk",
        feature = "cap-revoke-spawn-inheritance-descendants",
        feature = "cap-survive-exit",
        feature = "file-cap-derive-grant",
        feature = "cap-rooted-open-test",
        feature = "mem-quota-test",
        feature = "kill-ready-in-kernel-test",
        feature = "obj-quota-test",
        feature = "user-copy-bounds-test",
        feature = "socket-teardown-test",
        feature = "exit-codes-test",
        feature = "mount-authority-test",
        feature = "interface-cap-derive-revoke",
        feature = "arthur-grant-revoke",
        feature = "atrium-slice1",
        feature = "atrium-surface-protocol",
        feature = "atrium-input-routing",
        feature = "dw1-desktop-wm",
        feature = "dw2-scene-policy",
        feature = "atrium-slice3",
        feature = "atrium-interactive",
        feature = "atrium-interactive-baremetal",
        feature = "gpu-glass-slice6",
        feature = "arthur-compositor",
        feature = "product-integration",
        feature = "sys-wait-blocking",
        feature = "sys-wait-wnohang",
        feature = "sys-wait-already-exited",
        feature = "sys-wait-multiple-waiters",
        feature = "sys-inspect-running",
        feature = "sys-inspect-exited",
        feature = "sys-inspect-rights-gate",
        feature = "kernel-observability",
        feature = "kernel-observability-disabled",
        feature = "sched-ready-queue-capacity-pressure",
        feature = "neo-shell-repl",
        feature = "neo-shell-builtins",
        feature = "arthur-action-surface",
        feature = "arthur-converse",
        feature = "arthur-boots-thinking",
        feature = "arthur-act",
        feature = "arthur-grants",
        feature = "arthur-console",
        feature = "v1-console-liveness",
        feature = "arthur-memory",
        feature = "boot-neofs-userland-persistence",
        feature = "arthur-memory-delegation",
        feature = "arthur-memory-forgetting",
        feature = "ipc-rendezvous-translate-fault",
        feature = "vfs-testfs-open-read-close",
        feature = "vfs-testfs-write-readback",
        feature = "vfs-type-vs-rights-errors",
        feature = "vfs-stdout-compat",
        feature = "vfs-block-smoke",
        feature = "vfs-readdir-testfs-root",
        feature = "vfs-readdir-requires-right",
        feature = "vfs-readdir-regular-is-enotdir",
        feature = "vfs-directory-read-is-eisdir",
        feature = "readdir-cursor-eof-stable",
        feature = "readdir-buffer-too-small",
        feature = "tensor-create-query-drop",
        feature = "model-load-cascade",
        feature = "infer-mlp-forward",
        feature = "infer-transformer-forward",
        feature = "inference-preemptible-liveness",
        feature = "infer-embedding-forward",
        feature = "infer-embedding-oob-rejected",
        feature = "swapgs-irq-ring3",
        feature = "syscall-mxcsr-entry",
        feature = "phase4-socket-udp-loopback",
        feature = "phase4-slaac-udp-ipv6-loopback",
        feature = "phase4-https-fetch",
        feature = "net-dial-cap-kill-switch",
        feature = "fetch-broker",
        feature = "vault-phase-b",
        feature = "desktop-session"
    ),
    allow(dead_code)
)]

extern crate alloc;

#[cfg(any(feature = "arthur-compositor", feature = "product-integration"))]
mod arthur_compositor;
#[cfg(feature = "desktop-session")]
mod desktop_session;
#[cfg(any(
    feature = "dw1-desktop-wm",
    feature = "dw2-scene-policy",
    feature = "product-integration"
))]
mod dw1_desktop_wm;

use alloc::vec::Vec;

use libneo::{
    ESRCH, sys_chan_create, sys_exit, sys_kill, sys_recv, sys_send, sys_spawn, sys_write,
};

#[cfg(feature = "net-dial-cap")]
#[allow(dead_code)]
fn shell_inherited_handles() -> &'static [libneo::Handle] {
    &[libneo::Handle(0)]
}

#[cfg(not(feature = "net-dial-cap"))]
#[allow(dead_code)]
fn shell_inherited_handles() -> &'static [libneo::Handle] {
    &[]
}

const HELLO: &[u8] = b"hello from ring 3 (i'm neo-init, pid 1)\n";
const SPAWNING: &[u8] = b"[ neo-init ] spawning worker with chan_main + chan_block\n";
const SPAWN_OK: &[u8] = b"[ neo-init ] worker spawned with ProcessHandle\n";
const SPAWN_FAIL: &[u8] = b"[ neo-init ] sys_spawn FAILED\n";
const CHAN_FAIL: &[u8] = b"[ neo-init ] sys_chan_create FAILED\n";
const SENT_PING: &[u8] = b"[ neo-init ] sent heap-built ping; waiting for pong\n";
const RECV_PONG: &[u8] = b"[ neo-init ] got pong from worker\n";
const RECV_FAIL: &[u8] = b"[ neo-init ] sys_recv FAILED\n";
const SENT_GOBLOCK: &[u8] = b"[ neo-init ] sent go-block; worker should now be RecvBlocked\n";
const SENT_GOBLOCK_FAIL: &[u8] = b"[ neo-init ] go-block sys_send FAILED\n";
const TRY_KILL_BLOCKED: &[u8] = b"[ neo-init ] attempting sys_kill on Blocked worker (expect 0)\n";
const KILL_BLOCKED_OK: &[u8] = b"[ neo-init ] sys_kill on Blocked returned 0 as expected\n";
const KILL_BLOCKED_WRONG: &[u8] = b"[ neo-init ] BUG: sys_kill on Blocked did NOT return 0\n";
const TRY_KILL_STALE: &[u8] = b"[ neo-init ] sys_kill on stale handle (expect ESRCH)\n";
const KILL_STALE_OK: &[u8] = b"[ neo-init ] sys_kill on stale returned ESRCH as expected\n";
const KILL_STALE_WRONG: &[u8] = b"[ neo-init ] BUG: sys_kill on stale did NOT return ESRCH\n";

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let code = run_selected_scenario();
    sys_exit(code);
}

#[cfg(any(
    feature = "cap-mint-and-use",
    feature = "cap-derive-subset",
    feature = "cap-derive-rights-violation",
    feature = "cap-derive-missing-meta",
    feature = "cap-grant-basic",
    feature = "cap-grant-missing-meta",
    feature = "cap-revoke-local",
    feature = "cap-graph-revoke",
    feature = "cap-revoke-cross-process",
    feature = "cap-revoke-descendants-walk",
    feature = "cap-revoke-spawn-inheritance-descendants",
    feature = "cap-survive-exit"
))]
fn run_selected_scenario() -> i32 {
    cap_tests::run()
}

#[cfg(any(
    feature = "sys-wait-blocking",
    feature = "sys-wait-wnohang",
    feature = "sys-wait-already-exited",
    feature = "sys-wait-multiple-waiters",
    feature = "sys-inspect-running",
    feature = "sys-inspect-exited",
    feature = "sys-inspect-rights-gate",
    feature = "kernel-observability",
    feature = "kernel-observability-disabled"
))]
fn run_selected_scenario() -> i32 {
    process_cap_tests::run()
}

#[cfg(feature = "file-cap-derive-grant")]
fn run_selected_scenario() -> i32 {
    file_cap_tests::run()
}

#[cfg(feature = "cap-rooted-open-test")]
fn run_selected_scenario() -> i32 {
    cap_rooted_tests::run()
}

#[cfg(feature = "mem-quota-test")]
fn run_selected_scenario() -> i32 {
    mem_quota_tests::run()
}

#[cfg(feature = "kill-ready-in-kernel-test")]
fn run_selected_scenario() -> i32 {
    kill_ready_in_kernel_tests::run()
}

#[cfg(feature = "obj-quota-test")]
fn run_selected_scenario() -> i32 {
    obj_quota_tests::run()
}

#[cfg(feature = "user-copy-bounds-test")]
fn run_selected_scenario() -> i32 {
    user_copy_bounds_tests::run()
}

#[cfg(feature = "socket-teardown-test")]
fn run_selected_scenario() -> i32 {
    socket_teardown_tests::run()
}

#[cfg(feature = "exit-codes-test")]
fn run_selected_scenario() -> i32 {
    exit_codes_tests::run()
}

#[cfg(feature = "mount-authority-test")]
fn run_selected_scenario() -> i32 {
    mount_authority_tests::run()
}

#[cfg(feature = "vault-phase-b")]
fn run_selected_scenario() -> i32 {
    vault_tests::run()
}

#[cfg(feature = "interface-cap-derive-revoke")]
fn run_selected_scenario() -> i32 {
    interface_cap_tests::run()
}

#[cfg(feature = "atrium-slice1")]
fn run_selected_scenario() -> i32 {
    atrium_tests::run()
}

#[cfg(feature = "atrium-surface-protocol")]
fn run_selected_scenario() -> i32 {
    atrium_surface_protocol_tests::run()
}

#[cfg(feature = "atrium-input-routing")]
fn run_selected_scenario() -> i32 {
    atrium_input_routing_tests::run()
}
#[cfg(feature = "dw1-desktop-wm")]
fn run_selected_scenario() -> i32 {
    dw1_desktop_wm::run()
}
#[cfg(feature = "dw2-scene-policy")]
fn run_selected_scenario() -> i32 {
    dw1_desktop_wm::run_dw2()
}
#[cfg(feature = "atrium-slice3")]
fn run_selected_scenario() -> i32 {
    atrium_slice3_compositor_tests::run()
}

#[cfg(feature = "atrium-interactive")]
fn run_selected_scenario() -> i32 {
    atrium_interactive_compositor_tests::run()
}

#[cfg(feature = "atrium-interactive-baremetal")]
fn run_selected_scenario() -> i32 {
    atrium_interactive_compositor_tests::run()
}

#[cfg(feature = "gpu-glass-slice6")]
fn run_selected_scenario() -> i32 {
    gpu_glass_slice6_tests::run()
}

#[cfg(feature = "arthur-compositor")]
fn run_selected_scenario() -> i32 {
    arthur_compositor::run()
}

#[cfg(feature = "desktop-session")]
fn run_selected_scenario() -> i32 {
    desktop_session::run()
}

#[cfg(feature = "product-integration")]
fn run_selected_scenario() -> i32 {
    if let Some(readback) = product_integration_tests::read_prior_boot() {
        return if readback == 0 { 78 } else { readback };
    }

    let desktop = dw1_desktop_wm::run();
    if desktop != 0 {
        product_integration_tests::log_failure(b"desktop", desktop);
        return 100 + desktop;
    }
    product_integration_tests::log_stage(
        b"desktop",
        b"role_text_surfaces=2 focus_switches=2 focused_deliveries=2 unfocused_deliveries=0 direct_present=eperm,eperm",
    );

    let arthur = arthur_compositor::run();
    if arthur != 0 {
        product_integration_tests::log_failure(b"arthur", arthur);
        return 200 + arthur;
    }
    product_integration_tests::log_stage(
        b"arthur",
        b"received_prompt=hello_world byte_equal=true coherent_reply=validated provenance=neofs_model_path",
    );

    product_integration_tests::write_first_boot()
}

#[cfg(feature = "arthur-grant-revoke")]
fn run_selected_scenario() -> i32 {
    arthur_grant_revoke_tests::run()
}

#[cfg(feature = "tensor-create-query-drop")]
fn run_selected_scenario() -> i32 {
    tensor_tests::run()
}

#[cfg(feature = "model-load-cascade")]
fn run_selected_scenario() -> i32 {
    model_tests::run()
}

#[cfg(feature = "infer-mlp-forward")]
fn run_selected_scenario() -> i32 {
    infer_tests::run()
}

#[cfg(feature = "infer-transformer-forward")]
fn run_selected_scenario() -> i32 {
    infer_transformer_tests::run()
}

#[cfg(feature = "infer-embedding-forward")]
fn run_selected_scenario() -> i32 {
    infer_embedding_tests::run()
}

#[cfg(feature = "infer-embedding-oob-rejected")]
fn run_selected_scenario() -> i32 {
    infer_embedding_oob_tests::run()
}

#[cfg(feature = "swapgs-irq-ring3")]
fn run_selected_scenario() -> i32 {
    swapgs_irq_ring3_tests::run()
}

#[cfg(feature = "phase4-socket-udp-loopback")]
fn run_selected_scenario() -> i32 {
    socket_tests::run()
}

#[cfg(feature = "neo-shell-repl")]
fn run_selected_scenario() -> i32 {
    neo_shell_repl_tests::run()
}

#[cfg(feature = "neo-shell-builtins")]
fn run_selected_scenario() -> i32 {
    neo_shell_builtins_tests::run()
}

#[cfg(feature = "arthur-action-surface")]
fn run_selected_scenario() -> i32 {
    arthur_action_surface_tests::run()
}

#[cfg(feature = "arthur-converse")]
fn run_selected_scenario() -> i32 {
    arthur_converse_tests::run()
}

#[cfg(feature = "arthur-boots-thinking")]
fn run_selected_scenario() -> i32 {
    arthur_boots_thinking_tests::run()
}

#[cfg(feature = "arthur-act")]
fn run_selected_scenario() -> i32 {
    arthur_act_tests::run()
}

#[cfg(feature = "arthur-grants")]
fn run_selected_scenario() -> i32 {
    arthur_grants_tests::run()
}

#[cfg(feature = "arthur-console")]
fn run_selected_scenario() -> i32 {
    arthur_console_tests::run()
}

#[cfg(feature = "v1-console-liveness")]
fn run_selected_scenario() -> i32 {
    v1_console_liveness_tests::run()
}

#[cfg(all(
    feature = "arthur-memory",
    not(feature = "arthur-compositor"),
    not(feature = "product-integration"),
    not(feature = "boot-neofs-userland-persistence"),
    not(feature = "arthur-memory-encryption"),
    not(feature = "arthur-memory-delegation"),
    not(feature = "sched-spawn-reap-spawn-heavy"),
    not(feature = "desktop-session")
))]
fn run_selected_scenario() -> i32 {
    arthur_memory_tests::run()
}

#[cfg(feature = "product-integration")]
mod product_integration_tests {
    use libneo::{
        ENOENT, FileRights, OPEN_CREATE_REGULAR, OPEN_ROOT_TEMPORARY, sys_close, sys_fsync,
        sys_open, sys_read, sys_write,
    };

    const PATH: &str = "/neofs/.arthur/memory/product-0.1.persist";
    const CONTENT: &[u8] = b"LogansOS product 0.1 persistence proof\n";

    pub fn read_prior_boot() -> Option<i32> {
        match sys_open(PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Err(ENOENT) => None,
            Ok(file) => {
                let code = verify_file(file);
                let _ = sys_close(file);
                if code != 0 {
                    return Some(code);
                }
                log_stage(
                    b"readback",
                    b"bytes=39 source=prior_boot byte_equal=true fsync_prior_boot=true",
                );
                Some(0)
            }
            Err(_) => Some(240),
        }
    }

    pub fn write_first_boot() -> i32 {
        let file = match sys_open(
            PATH,
            OPEN_CREATE_REGULAR,
            FileRights::READ.union(FileRights::WRITE),
        ) {
            Ok(file) => file,
            Err(_) => return 241,
        };
        if sys_write(file.0, CONTENT.as_ptr(), CONTENT.len()) != CONTENT.len() as i64 {
            let _ = sys_close(file);
            return 242;
        }
        if sys_fsync(file) != 0 {
            let _ = sys_close(file);
            return 243;
        }
        let _ = sys_close(file);
        let readback = match sys_open(PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(file) => file,
            Err(_) => return 244,
        };
        let code = verify_file(readback);
        let _ = sys_close(readback);
        if code != 0 {
            return code;
        }
        log_stage(
            b"write",
            b"bytes=39 fsync=ok same_boot_readback=byte_equal path=product-0.1.persist",
        );
        0
    }

    fn verify_file(file: libneo::FileHandle) -> i32 {
        let mut bytes = [0u8; CONTENT.len()];
        if sys_read(file, &mut bytes) != CONTENT.len() as i64 || bytes != CONTENT {
            return 245;
        }
        0
    }

    pub fn log_stage(stage: &[u8], detail: &[u8]) {
        write_parts(&[
            b"NEO_TEST/1 event=INFO name=product_integration stage=",
            stage,
            b" ",
            detail,
            b"\n",
        ]);
    }

    pub fn log_failure(stage: &[u8], code: i32) {
        write_parts(&[
            b"NEO_TEST/1 event=INFO name=product_integration stage=",
            stage,
            b" result=failed code=",
        ]);
        let mut digits = [0u8; 11];
        let mut value = code.unsigned_abs();
        let mut at = digits.len();
        if value == 0 {
            at -= 1;
            digits[at] = b'0';
        }
        while value != 0 {
            at -= 1;
            digits[at] = b'0' + (value % 10) as u8;
            value /= 10;
        }
        let _ = sys_write(1, digits[at..].as_ptr(), digits.len() - at);
        let _ = sys_write(1, b"\n".as_ptr(), 1);
    }

    fn write_parts(parts: &[&[u8]]) {
        for part in parts {
            let _ = sys_write(1, part.as_ptr(), part.len());
        }
    }
}

#[cfg(feature = "boot-neofs-userland-persistence")]
fn run_selected_scenario() -> i32 {
    boot_neofs_userland_persistence_tests::run()
}

#[cfg(all(
    feature = "arthur-memory-encryption",
    not(feature = "arthur-memory-delegation")
))]
fn run_selected_scenario() -> i32 {
    arthur_memory_encrypted_tests::run()
}

#[cfg(feature = "arthur-memory-delegation")]
#[cfg(not(feature = "arthur-memory-forgetting"))]
fn run_selected_scenario() -> i32 {
    arthur_memory_delegation_tests::run()
}

#[cfg(feature = "arthur-memory-forgetting")]
fn run_selected_scenario() -> i32 {
    arthur_memory_forgetting_tests::run()
}

#[cfg(feature = "sched-spawn-reap-spawn-heavy")]
fn run_selected_scenario() -> i32 {
    sched_spawn_reap_spawn_heavy_tests::run()
}

#[cfg(feature = "swapgs-irq-ring3")]
mod swapgs_irq_ring3_tests {
    pub fn run() -> i32 {
        let mut acc = 0x9e37_79b9_7f4a_7c15_u64;
        for i in 0..20_000_000_u64 {
            acc = acc.wrapping_add(i ^ acc.rotate_left(7));
            core::hint::spin_loop();
        }
        if core::hint::black_box(acc) == 0 {
            1
        } else {
            0
        }
    }
}

#[cfg(feature = "syscall-mxcsr-entry")]
fn run_selected_scenario() -> i32 {
    syscall_mxcsr_entry_tests::run()
}

/// CP-41 discriminator. Ring 3 owns MXCSR, and the kernel's SSE code must not
/// run on it. Each leg loads an MXCSR and an x87 control word and calls
/// `SYS_TEST_FLOAT_PROBE` in one asm block, so no user float op runs on the
/// hostile state, then checks what the kernel saw, what it computed, and that
/// the user's own control state came back. The kernel has no x87 code, so the
/// entry stubs leave the x87 control word alone: the kernel's view of it is
/// only logged, but the user's must still come back intact. Exit 0 iff every leg passes;
/// otherwise the first failure's code (leg N: 10*N + 1..4).
#[cfg(feature = "syscall-mxcsr-entry")]
mod syscall_mxcsr_entry_tests {
    use alloc::format;
    use core::arch::asm;

    use libneo::{SYS_TEST_FLOAT_PROBE, sys_write};

    const WORDS: usize = 16;
    const DEFAULT_MXCSR: u32 = 0x1F80;
    const DEFAULT_FCW: u16 = 0x037F;
    /// MXCSR sticky exception flags; every bit above them is control.
    const MXCSR_FLAGS: u32 = 0x3F;

    /// Probe words 2..=15 under the default MXCSR (round to nearest, all
    /// masked, no FTZ/DAZ). Worked out from IEEE 754 and checked on a host
    /// CPU, independently of the kernel's output.
    const EXPECTED: [u32; WORDS - 2] = [
        0x3EAA_AAAB, // 1/3
        0xFFC0_0000, // 0/0 = default QNaN
        0x7F80_0000, // 1/0 = +Inf
        0x7F80_0000, // MAX * 2 = +Inf (RZ: MAX)
        0x002A_AAAB, // MIN_POSITIVE / 3, denormal (FTZ: 0)
        0x0000_0001, // smallest denormal + 0 (DAZ: 0)
        0x400F_1BBD, // sqrt(5)
        0x4B80_0002, // 16777219 as f32, tie to even
        0x9999_999A, // f64 1/10, low word
        0x3FB9_9999, // f64 1/10, high word
        0x3F80_0001, // logit 0: 1 + 0.75 ulp
        0x7F80_0000, // logit 1: MAX + MAX
        0xFFC0_0000, // logit 2: Inf + -Inf
        0x0020_0000, // logit 3: MIN_POSITIVE + denormal (DAZ: MIN_POSITIVE)
    ];

    struct Leg {
        name: &'static str,
        mxcsr: u32,
        fcw: u16,
    }

    const LEGS: [Leg; 3] = [
        Leg {
            name: "default",
            mxcsr: DEFAULT_MXCSR,
            fcw: DEFAULT_FCW,
        },
        // Masked, so it cannot trap: round toward zero + FTZ + DAZ only change
        // the results. Runs before the trap leg so both failures show up.
        Leg {
            name: "masked_rz_ftz_daz",
            mxcsr: 0xFFC0,
            fcw: 0x0F7F,
        },
        // Every SSE and x87 exception unmasked, round toward zero.
        Leg {
            name: "unmasked_rz",
            mxcsr: 0x6000,
            fcw: 0x0C40,
        },
    ];

    fn say(line: &str) {
        let _ = sys_write(1, line.as_ptr(), line.len());
        let _ = sys_write(1, b"\n".as_ptr(), 1);
    }

    pub fn run() -> i32 {
        let mut first_failure = 0;
        for (i, leg) in LEGS.iter().enumerate() {
            if let Err(code) = run_leg(leg, 10 * (i as i32 + 1))
                && first_failure == 0
            {
                first_failure = code;
            }
        }
        if first_failure == 0 {
            say("[ syscall-mxcsr ] all legs passed");
        }
        first_failure
    }

    fn run_leg(leg: &Leg, base: i32) -> Result<(), i32> {
        let mut words = [0u32; WORDS];
        let (ret, mxcsr_after, fcw_after) = probe(leg.mxcsr, leg.fcw, &mut words);
        say(&format!(
            "[ syscall-mxcsr ] leg={} user_mxcsr={:#06x} user_fcw={:#06x} ret={} kernel_mxcsr={:#06x} kernel_fcw={:#06x} user_mxcsr_after={:#06x} user_fcw_after={:#06x}",
            leg.name, leg.mxcsr, leg.fcw, ret, words[0], words[1], mxcsr_after, fcw_after
        ));
        if ret != 0 {
            say(&format!(
                "[ syscall-mxcsr ] FAIL leg={} probe ret={}",
                leg.name, ret
            ));
            return Err(base + 1);
        }
        let mut result = Ok(());
        if words[0] & !MXCSR_FLAGS != DEFAULT_MXCSR {
            say(&format!(
                "[ syscall-mxcsr ] FAIL leg={} kernel float code ran on the caller's MXCSR",
                leg.name
            ));
            result = result.and(Err(base + 2));
        }
        let mut diffs = 0;
        for (k, (&got, &want)) in words[2..].iter().zip(EXPECTED.iter()).enumerate() {
            if got != want {
                say(&format!(
                    "[ syscall-mxcsr ] FAIL leg={} result[{}] got={:#010x} want={:#010x}",
                    leg.name, k, got, want
                ));
                diffs += 1;
            }
        }
        if diffs != 0 {
            result = result.and(Err(base + 3));
        }
        if mxcsr_after != leg.mxcsr || fcw_after != leg.fcw {
            say(&format!(
                "[ syscall-mxcsr ] FAIL leg={} user control state not restored",
                leg.name
            ));
            result = result.and(Err(base + 4));
        }
        result
    }

    /// Load `mxcsr`/`fcw`, issue the probe syscall, read back the user's
    /// MXCSR/FCW, and restore the defaults, all in one asm block.
    fn probe(mxcsr: u32, fcw: u16, words: &mut [u32; WORDS]) -> (i64, u32, u16) {
        let ret: i64;
        let mut mxcsr_after = 0u32;
        let mut fcw_after = 0u16;
        // SAFETY: the kernel preserves every register but RAX (return value),
        // RCX and R11 across SYSCALL, so the pointer operands stay valid. The
        // probe writes exactly `size_of_val(words)` bytes into `words`. The
        // block ends with the default MXCSR and control word loaded again.
        unsafe {
            asm!(
                "fldcw [{fcw}]",
                "ldmxcsr [{mxcsr}]",
                "syscall",
                "stmxcsr [{mxcsr_after}]",
                "fnstcw [{fcw_after}]",
                "ldmxcsr [{default_mxcsr}]",
                "fldcw [{default_fcw}]",
                fcw = in(reg) &fcw,
                mxcsr = in(reg) &mxcsr,
                mxcsr_after = in(reg) &mut mxcsr_after,
                fcw_after = in(reg) &mut fcw_after,
                default_mxcsr = in(reg) &DEFAULT_MXCSR,
                default_fcw = in(reg) &DEFAULT_FCW,
                inlateout("rax") SYS_TEST_FLOAT_PROBE as i64 => ret,
                in("rdi") words.as_mut_ptr(),
                in("rsi") core::mem::size_of_val(words),
                out("rcx") _,
                out("r11") _,
                options(nostack),
            );
        }
        (ret, mxcsr_after, fcw_after)
    }
}

#[cfg(feature = "phase4-slaac-udp-ipv6-loopback")]
fn run_selected_scenario() -> i32 {
    slaac_socket_tests::run()
}

#[cfg(feature = "phase4-https-fetch")]
fn run_selected_scenario() -> i32 {
    https_fetch_tests::run()
}

#[cfg(feature = "net-dial-cap-kill-switch")]
fn run_selected_scenario() -> i32 {
    https_fetch_kill_switch_tests::run()
}

#[cfg(feature = "fetch-broker")]
fn run_selected_scenario() -> i32 {
    fetch_broker_tests::run()
}

#[cfg(feature = "phase4-socket-udp-loopback")]
mod socket_tests {
    use libneo::{
        AF_INET6, ENOTCONN, SOCK_DGRAM, SockAddr, SockAddrIn6, sys_socket_bind_in6,
        sys_socket_close, sys_socket_create, sys_socket_recvfrom, sys_socket_send,
        sys_socket_sendto,
    };

    pub fn run() -> i32 {
        let sender = match sys_socket_create(AF_INET6, SOCK_DGRAM) {
            Ok(socket) => socket,
            Err(_) => return 21,
        };
        let receiver = match sys_socket_create(AF_INET6, SOCK_DGRAM) {
            Ok(socket) => socket,
            Err(_) => {
                let _ = sys_socket_close(sender);
                return 29;
            }
        };
        let sender_addr = SockAddrIn6::new_loopback(5555);
        let receiver_addr = SockAddrIn6::new_loopback(5556);
        if sys_socket_bind_in6(sender, &sender_addr) != 0 {
            let _ = sys_socket_close(sender);
            let _ = sys_socket_close(receiver);
            return 22;
        }
        if sys_socket_bind_in6(receiver, &receiver_addr) != 0 {
            let _ = sys_socket_close(sender);
            let _ = sys_socket_close(receiver);
            return 26;
        }
        let payload = b"phase4 socket udp loopback";
        if sys_socket_send(sender, payload) != ENOTCONN {
            let _ = sys_socket_close(sender);
            let _ = sys_socket_close(receiver);
            return 27;
        }
        if sys_socket_sendto(sender, payload, &SockAddr::Inet6(receiver_addr))
            != payload.len() as i64
        {
            let _ = sys_socket_close(sender);
            let _ = sys_socket_close(receiver);
            return 23;
        }
        let mut buf = [0u8; 64];
        let mut src = SockAddr::Inet6(SockAddrIn6::new_loopback(0));
        let n = sys_socket_recvfrom(receiver, &mut buf, &mut src);
        let _ = sys_socket_close(sender);
        let _ = sys_socket_close(receiver);
        if n != payload.len() as i64 {
            return 24;
        }
        if &buf[..payload.len()] != payload {
            return 25;
        }
        match src {
            SockAddr::Inet6(addr) if addr.port_be == sender_addr.port_be => {}
            _ => return 28,
        }
        0
    }
}

#[cfg(feature = "phase4-slaac-udp-ipv6-loopback")]
mod slaac_socket_tests {
    use libneo::{
        AF_INET6, SOCK_DGRAM, SockAddr, SockAddrIn6, sys_net_stats, sys_socket_bind_in6,
        sys_socket_close, sys_socket_create, sys_socket_sendto, sys_write,
    };

    const SLAAC_ADDR: [u8; 16] = [
        0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0x02, 0, 0, 0, 0, 0, 0, 0,
    ];
    const REMOTE_ADDR: [u8; 16] = [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];

    pub fn run() -> i32 {
        let socket = match sys_socket_create(AF_INET6, SOCK_DGRAM) {
            Ok(socket) => socket,
            Err(_) => return 31,
        };
        let local = SockAddrIn6::new(SLAAC_ADDR, 5555);
        if sys_socket_bind_in6(socket, &local) != 0 {
            let _ = sys_socket_close(socket);
            return 32;
        }
        let payload = b"phase4 slaac udp ipv6 egress";
        let remote = SockAddr::Inet6(SockAddrIn6::new(REMOTE_ADDR, 5556));
        if sys_socket_sendto(socket, payload, &remote) != payload.len() as i64 {
            let _ = sys_socket_close(socket);
            return 33;
        }
        let stats = match sys_net_stats(socket) {
            Ok(stats) => stats,
            Err(_) => {
                let _ = sys_socket_close(socket);
                return 34;
            }
        };
        let _ = sys_socket_close(socket);
        if stats.packets_out != 1 || stats.bytes_out != payload.len() as u64 {
            let _ = sys_write(
                1,
                b"[ slaac-udp ] EXIT=35 site=slaac-stats-accounting\n".as_ptr(),
                b"[ slaac-udp ] EXIT=35 site=slaac-stats-accounting\n".len(),
            );
            return 35;
        }
        0
    }
}

#[cfg(feature = "tensor-create-query-drop")]
mod tensor_tests {
    use libneo::{
        EBADF, TENSOR_DEVICE_CPU, TENSOR_DTYPE_F32, TensorCreateDesc, TensorQueryDesc,
        sys_tensor_create, sys_tensor_drop, sys_tensor_query, sys_tensor_share,
    };

    pub fn run() -> i32 {
        let data = [
            0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x40, 0x40, 0x00, 0x00,
            0x80, 0x40,
        ];
        let mut dims = [0u64; 8];
        dims[0] = 2;
        dims[1] = 2;
        let desc = TensorCreateDesc::new(2, TENSOR_DTYPE_F32, false, dims, &data);
        let tensor = match sys_tensor_create(&desc) {
            Ok(handle) => handle,
            Err(_) => return 51,
        };

        let mut query = TensorQueryDesc::empty();
        if sys_tensor_query(tensor, &mut query) != 0 {
            let _ = sys_tensor_drop(tensor);
            return 52;
        }
        if !query_matches(&query, 1, dims, data.len() as u64) {
            let _ = sys_tensor_drop(tensor);
            return 53;
        }

        let shared = match sys_tensor_share(tensor) {
            Ok(handle) => handle,
            Err(_) => {
                let _ = sys_tensor_drop(tensor);
                return 54;
            }
        };

        let mut shared_query = TensorQueryDesc::empty();
        if sys_tensor_query(tensor, &mut shared_query) != 0 {
            let _ = sys_tensor_drop(tensor);
            let _ = sys_tensor_drop(shared);
            return 55;
        }
        if !query_matches(&shared_query, 2, dims, data.len() as u64) {
            let _ = sys_tensor_drop(tensor);
            let _ = sys_tensor_drop(shared);
            return 56;
        }

        if sys_tensor_drop(tensor) != 0 {
            let _ = sys_tensor_drop(shared);
            return 57;
        }
        if sys_tensor_query(tensor, &mut query) != EBADF {
            let _ = sys_tensor_drop(shared);
            return 58;
        }

        let mut remaining_query = TensorQueryDesc::empty();
        if sys_tensor_query(shared, &mut remaining_query) != 0 {
            let _ = sys_tensor_drop(shared);
            return 59;
        }
        if !query_matches(&remaining_query, 1, dims, data.len() as u64) {
            let _ = sys_tensor_drop(shared);
            return 60;
        }

        if sys_tensor_drop(shared) != 0 {
            return 61;
        }
        if sys_tensor_query(shared, &mut remaining_query) != EBADF {
            return 62;
        }
        0
    }

    fn query_matches(
        query: &TensorQueryDesc,
        refcount: u64,
        dims: [u64; 8],
        byte_len: u64,
    ) -> bool {
        query.rank == 2
            && query.dtype == TENSOR_DTYPE_F32
            && query.device == TENSOR_DEVICE_CPU
            && query.mutable == 0
            && query.refcount == refcount
            && query.dims == dims
            && query.byte_len == byte_len
    }
}

#[cfg(feature = "model-load-cascade")]
mod model_tests {
    use libneo::{
        EACCES, EINVAL, ENOENT, ENOMEM, ESTALE, FileRights, MODEL_QUERY_VERSION, ModelQueryDesc,
        OPEN_ROOT_TEMPORARY, TENSOR_DTYPE_F32, sys_close, sys_model_drop, sys_model_load,
        sys_model_query, sys_open, sys_umount,
    };

    const MOUNT_NAME: &str = "neofs";
    const MODEL_PATH: &str = "/neofs/model.neom";

    pub fn run() -> i32 {
        let root = match sys_open("/", OPEN_ROOT_TEMPORARY, FileRights::MOUNT) {
            Ok(handle) => handle,
            Err(_) => return 71,
        };

        let model = match sys_model_load(MODEL_PATH, OPEN_ROOT_TEMPORARY) {
            Ok(handle) => handle,
            Err(err) => {
                let _ = sys_close(root);
                return match err {
                    ENOENT => 721,
                    EACCES => 722,
                    ENOMEM => 723,
                    EINVAL => 724,
                    _ => 725,
                };
            }
        };
        let mut query = ModelQueryDesc::empty();
        if sys_model_query(model, &mut query) != 0 {
            let _ = sys_model_drop(model);
            let _ = sys_close(root);
            return 73;
        }
        if !query_matches(&query) {
            let _ = sys_model_drop(model);
            let _ = sys_close(root);
            return 74;
        }

        if sys_umount(root, MOUNT_NAME, 0) != 0 {
            let _ = sys_model_drop(model);
            let _ = sys_close(root);
            return 75;
        }
        if sys_model_query(model, &mut query) != ESTALE {
            let _ = sys_model_drop(model);
            let _ = sys_close(root);
            return 78;
        }
        if sys_model_drop(model) != 0 {
            let _ = sys_close(root);
            return 80;
        }
        let _ = sys_close(root);
        0
    }

    fn query_matches(query: &ModelQueryDesc) -> bool {
        let mut dims = [0u64; 8];
        dims[0] = 1;
        query.version == MODEL_QUERY_VERSION
            && query.tensor_count == 1
            && query.node_count == 1
            && query.input_count == 1
            && query.output_count == 1
            && query.value_count == 3
            && query.first_tensor_dtype == TENSOR_DTYPE_F32
            && query.first_tensor_rank == 1
            && query.first_tensor_dims == dims
            && query.first_tensor_byte_len == 4
    }
}

#[cfg(feature = "infer-mlp-forward")]
mod infer_tests {
    use libneo::{
        E2BIG, EACCES, EINVAL, ENOENT, ENOMEM, MODEL_QUERY_VERSION, ModelQueryDesc,
        OPEN_ROOT_TEMPORARY, TENSOR_DEVICE_CPU, TENSOR_DTYPE_F32, TensorCreateDesc, TensorHandle,
        TensorQueryDesc, sys_infer_run, sys_model_drop, sys_model_load, sys_model_query,
        sys_tensor_create, sys_tensor_drop, sys_tensor_query,
    };

    const MODEL_PATH: &str = "/neofs/model.neom";
    const SUCCESS_TAG: u32 = 0x4000_0000;

    pub fn run() -> i32 {
        let model = match sys_model_load(MODEL_PATH, OPEN_ROOT_TEMPORARY) {
            Ok(handle) => handle,
            Err(err) => {
                return match err {
                    ENOENT => 911,
                    EACCES => 912,
                    ENOMEM => 913,
                    EINVAL => 914,
                    E2BIG => 915,
                    _ => 916,
                };
            }
        };
        let mut model_query = ModelQueryDesc::empty();
        if sys_model_query(model, &mut model_query) != 0 || !model_query_matches(&model_query) {
            let _ = sys_model_drop(model);
            return 92;
        }

        let input_data = [
            0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x40, 0x40, 0x00, 0x00,
            0x80, 0x40,
        ];
        let input_desc = input_desc(&input_data);
        let input = match sys_tensor_create(&input_desc) {
            Ok(handle) => handle,
            Err(_) => {
                let _ = sys_model_drop(model);
                return 93;
            }
        };

        let inputs = [input];
        let mut outputs = [TensorHandle(0)];
        let count = match sys_infer_run(model, &inputs, &mut outputs, 0) {
            Ok(count) => count,
            Err(_) => {
                let _ = sys_tensor_drop(input);
                let _ = sys_model_drop(model);
                return 94;
            }
        };
        if count != 1 {
            let _ = sys_tensor_drop(outputs[0]);
            let _ = sys_tensor_drop(input);
            let _ = sys_model_drop(model);
            return 95;
        }

        let mut output_query = TensorQueryDesc::empty();
        if sys_tensor_query(outputs[0], &mut output_query) != 0
            || !output_query_matches(&output_query)
        {
            let _ = sys_tensor_drop(outputs[0]);
            let _ = sys_tensor_drop(input);
            let _ = sys_model_drop(model);
            return 96;
        }

        let _ = sys_tensor_drop(input);
        let _ = sys_model_drop(model);
        (SUCCESS_TAG | outputs[0].0) as i32
    }

    fn input_desc(data: &[u8]) -> TensorCreateDesc {
        let mut dims = [0u64; 8];
        dims[0] = 1;
        dims[1] = 4;
        TensorCreateDesc::new(2, TENSOR_DTYPE_F32, false, dims, data)
    }

    fn model_query_matches(query: &ModelQueryDesc) -> bool {
        let mut dims = [0u64; 8];
        dims[0] = 4;
        dims[1] = 8;
        query.version == MODEL_QUERY_VERSION
            && query.tensor_count == 4
            && query.node_count == 5
            && query.input_count == 1
            && query.output_count == 1
            && query.value_count == 10
            && query.first_tensor_dtype == TENSOR_DTYPE_F32
            && query.first_tensor_rank == 2
            && query.first_tensor_dims == dims
            && query.first_tensor_byte_len == 128
    }

    fn output_query_matches(query: &TensorQueryDesc) -> bool {
        let mut dims = [0u64; 8];
        dims[0] = 1;
        dims[1] = 3;
        query.rank == 2
            && query.dtype == TENSOR_DTYPE_F32
            && query.device == TENSOR_DEVICE_CPU
            && query.mutable == 0
            && query.dims == dims
            && query.byte_len == 12
    }
}

#[cfg(feature = "infer-transformer-forward")]
mod infer_transformer_tests {
    #[cfg(feature = "inference-preemptible-liveness")]
    use libneo::sys_generate;
    use libneo::{
        E2BIG, EACCES, EINVAL, ENOENT, ENOMEM, MODEL_QUERY_VERSION, ModelQueryDesc,
        OPEN_ROOT_TEMPORARY, TENSOR_DEVICE_CPU, TENSOR_DTYPE_F32, TensorCreateDesc, TensorHandle,
        TensorQueryDesc, sys_infer_run, sys_model_drop, sys_model_load, sys_model_query,
        sys_tensor_create, sys_tensor_drop, sys_tensor_query,
    };

    const MODEL_PATH: &str = "/neofs/model.neom";
    #[cfg(feature = "inference-preemptible-liveness")]
    const GENERATE_MODEL_PATH: &str = "/neofs/generate.neom";
    const SUCCESS_TAG: u32 = 0x4000_0000;

    pub fn run() -> i32 {
        let model = match sys_model_load(MODEL_PATH, OPEN_ROOT_TEMPORARY) {
            Ok(handle) => handle,
            Err(err) => {
                return match err {
                    ENOENT => 911,
                    EACCES => 912,
                    ENOMEM => 913,
                    EINVAL => 914,
                    E2BIG => 915,
                    _ => 916,
                };
            }
        };
        let mut model_query = ModelQueryDesc::empty();
        if sys_model_query(model, &mut model_query) != 0 || !model_query_matches(&model_query) {
            let _ = sys_model_drop(model);
            return 92;
        }

        let input_data = [
            0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x40, 0x40, 0x00, 0x00,
            0x80, 0x40,
        ];
        let input_desc = input_desc(&input_data);
        let input = match sys_tensor_create(&input_desc) {
            Ok(handle) => handle,
            Err(_) => {
                let _ = sys_model_drop(model);
                return 93;
            }
        };

        let inputs = [input];
        let mut outputs = [TensorHandle(0)];
        let count = match sys_infer_run(model, &inputs, &mut outputs, 0) {
            Ok(count) => count,
            Err(_) => {
                let _ = sys_tensor_drop(input);
                let _ = sys_model_drop(model);
                return 94;
            }
        };
        if count != 1 {
            let _ = sys_tensor_drop(outputs[0]);
            let _ = sys_tensor_drop(input);
            let _ = sys_model_drop(model);
            return 95;
        }

        let mut output_query = TensorQueryDesc::empty();
        if sys_tensor_query(outputs[0], &mut output_query) != 0
            || !output_query_matches(&output_query)
        {
            let _ = sys_tensor_drop(outputs[0]);
            let _ = sys_tensor_drop(input);
            let _ = sys_model_drop(model);
            return 96;
        }

        #[cfg(feature = "inference-preemptible-liveness")]
        {
            let generate_status = run_sys_generate_identity();
            if generate_status != 0 {
                let _ = sys_tensor_drop(outputs[0]);
                let _ = sys_tensor_drop(input);
                let _ = sys_model_drop(model);
                return generate_status;
            }
        }

        let _ = sys_tensor_drop(input);
        let _ = sys_model_drop(model);
        (SUCCESS_TAG | outputs[0].0) as i32
    }

    fn input_desc(data: &[u8]) -> TensorCreateDesc {
        let mut dims = [0u64; 8];
        dims[0] = 1;
        dims[1] = 4;
        TensorCreateDesc::new(2, TENSOR_DTYPE_F32, false, dims, data)
    }

    fn model_query_matches(query: &ModelQueryDesc) -> bool {
        let mut dims = [0u64; 8];
        dims[0] = 4;
        dims[1] = 0;
        query.version == MODEL_QUERY_VERSION
            && query.tensor_count == 4
            && query.node_count == 5
            && query.input_count == 1
            && query.output_count == 1
            && query.value_count == 10
            && query.first_tensor_dtype == TENSOR_DTYPE_F32
            && query.first_tensor_rank == 1
            && query.first_tensor_dims == dims
            && query.first_tensor_byte_len == 16
    }

    fn output_query_matches(query: &TensorQueryDesc) -> bool {
        let mut dims = [0u64; 8];
        dims[0] = 1;
        dims[1] = 4;
        query.rank == 2
            && query.dtype == TENSOR_DTYPE_F32
            && query.device == TENSOR_DEVICE_CPU
            && query.mutable == 0
            && query.dims == dims
            && query.byte_len == 16
    }

    #[cfg(feature = "inference-preemptible-liveness")]
    fn run_sys_generate_identity() -> i32 {
        let model = match sys_model_load(GENERATE_MODEL_PATH, OPEN_ROOT_TEMPORARY) {
            Ok(handle) => handle,
            Err(_) => return 970,
        };
        let prompt = [1u32];
        let mut generated = [0u32; 3];
        let count = match sys_generate(model, &prompt, &mut generated, 3, Some(4)) {
            Ok(count) => count,
            Err(_) => {
                let _ = sys_model_drop(model);
                return 971;
            }
        };
        if count != 3 {
            let _ = sys_model_drop(model);
            return 972;
        }
        if generated != [2, 3, 4] {
            let _ = sys_model_drop(model);
            return 973;
        }
        if sys_model_drop(model) != 0 {
            return 974;
        }
        0
    }
}

#[cfg(feature = "infer-embedding-forward")]
mod infer_embedding_tests {
    use libneo::{
        E2BIG, EACCES, EINVAL, ENOENT, ENOMEM, MODEL_QUERY_VERSION, ModelQueryDesc,
        OPEN_ROOT_TEMPORARY, TENSOR_DEVICE_CPU, TENSOR_DTYPE_F32, TENSOR_DTYPE_I32,
        TensorCreateDesc, TensorHandle, TensorQueryDesc, sys_infer_run, sys_model_drop,
        sys_model_load, sys_model_query, sys_tensor_create, sys_tensor_drop, sys_tensor_query,
    };

    const MODEL_PATH: &str = "/neofs/model.neom";
    const SUCCESS_TAG: u32 = 0x4000_0000;

    pub fn run() -> i32 {
        let model = match sys_model_load(MODEL_PATH, OPEN_ROOT_TEMPORARY) {
            Ok(handle) => handle,
            Err(err) => {
                return match err {
                    ENOENT => 911,
                    EACCES => 912,
                    ENOMEM => 913,
                    EINVAL => 914,
                    E2BIG => 915,
                    _ => 916,
                };
            }
        };
        let mut model_query = ModelQueryDesc::empty();
        if sys_model_query(model, &mut model_query) != 0 || !model_query_matches(&model_query) {
            let _ = sys_model_drop(model);
            return 92;
        }

        let input_data = [
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
        ];
        let input_desc = input_desc(&input_data);
        let input = match sys_tensor_create(&input_desc) {
            Ok(handle) => handle,
            Err(_) => {
                let _ = sys_model_drop(model);
                return 93;
            }
        };

        let inputs = [input];
        let mut outputs = [TensorHandle(0)];
        let count = match sys_infer_run(model, &inputs, &mut outputs, 0) {
            Ok(count) => count,
            Err(_) => {
                let _ = sys_tensor_drop(input);
                let _ = sys_model_drop(model);
                return 94;
            }
        };
        if count != 1 {
            let _ = sys_tensor_drop(outputs[0]);
            let _ = sys_tensor_drop(input);
            let _ = sys_model_drop(model);
            return 95;
        }

        let mut output_query = TensorQueryDesc::empty();
        if sys_tensor_query(outputs[0], &mut output_query) != 0
            || !output_query_matches(&output_query)
        {
            let _ = sys_tensor_drop(outputs[0]);
            let _ = sys_tensor_drop(input);
            let _ = sys_model_drop(model);
            return 96;
        }

        let _ = sys_tensor_drop(input);
        let _ = sys_model_drop(model);
        (SUCCESS_TAG | outputs[0].0) as i32
    }

    fn input_desc(data: &[u8]) -> TensorCreateDesc {
        let mut dims = [0u64; 8];
        dims[0] = 3;
        TensorCreateDesc::new(1, TENSOR_DTYPE_I32, false, dims, data)
    }

    fn model_query_matches(query: &ModelQueryDesc) -> bool {
        let mut dims = [0u64; 8];
        dims[0] = 4;
        dims[1] = 3;
        query.version == MODEL_QUERY_VERSION
            && query.tensor_count == 1
            && query.node_count == 1
            && query.input_count == 1
            && query.output_count == 1
            && query.value_count == 3
            && query.first_tensor_dtype == TENSOR_DTYPE_F32
            && query.first_tensor_rank == 2
            && query.first_tensor_dims == dims
            && query.first_tensor_byte_len == 48
    }

    fn output_query_matches(query: &TensorQueryDesc) -> bool {
        let mut dims = [0u64; 8];
        dims[0] = 3;
        dims[1] = 3;
        query.rank == 2
            && query.dtype == TENSOR_DTYPE_F32
            && query.device == TENSOR_DEVICE_CPU
            && query.mutable == 0
            && query.dims == dims
            && query.byte_len == 36
    }
}

#[cfg(feature = "infer-embedding-oob-rejected")]
mod infer_embedding_oob_tests {
    use libneo::{
        E2BIG, EACCES, EINVAL, ENOENT, ENOMEM, ModelHandle, OPEN_ROOT_TEMPORARY, TENSOR_DTYPE_I32,
        TensorCreateDesc, TensorHandle, sys_infer_run, sys_model_drop, sys_model_load,
        sys_tensor_create, sys_tensor_drop,
    };

    const MODEL_PATH: &str = "/neofs/model.neom";

    pub fn run() -> i32 {
        let model = match sys_model_load(MODEL_PATH, OPEN_ROOT_TEMPORARY) {
            Ok(handle) => handle,
            Err(err) => {
                return match err {
                    ENOENT => 921,
                    EACCES => 922,
                    ENOMEM => 923,
                    EINVAL => 924,
                    E2BIG => 925,
                    _ => 926,
                };
            }
        };

        if let Err(code) = expect_einval(model, 4i32.to_le_bytes(), 930) {
            let _ = sys_model_drop(model);
            return code;
        }
        if let Err(code) = expect_einval(model, (-1i32).to_le_bytes(), 940) {
            let _ = sys_model_drop(model);
            return code;
        }

        let _ = sys_model_drop(model);
        0
    }

    fn expect_einval(model: ModelHandle, data: [u8; 4], code_base: i32) -> Result<(), i32> {
        let input_desc = input_desc(&data);
        let input = sys_tensor_create(&input_desc).map_err(|_| code_base + 1)?;
        let inputs = [input];
        let mut outputs = [TensorHandle(0)];
        let result = sys_infer_run(model, &inputs, &mut outputs, 0);
        let _ = sys_tensor_drop(input);
        match result {
            Err(EINVAL) => Ok(()),
            Err(_) => Err(code_base + 2),
            Ok(count) => {
                if count > 0 {
                    let _ = sys_tensor_drop(outputs[0]);
                }
                Err(code_base + 3)
            }
        }
    }

    fn input_desc(data: &[u8]) -> TensorCreateDesc {
        let mut dims = [0u64; 8];
        dims[0] = 1;
        TensorCreateDesc::new(1, TENSOR_DTYPE_I32, false, dims, data)
    }
}

#[cfg(any(
    feature = "sys-wait-blocking",
    feature = "sys-wait-wnohang",
    feature = "sys-wait-already-exited",
    feature = "sys-wait-multiple-waiters",
    feature = "sys-inspect-running",
    feature = "sys-inspect-exited",
    feature = "sys-inspect-rights-gate",
    feature = "kernel-observability",
    feature = "kernel-observability-disabled"
))]
mod process_cap_tests {
    use super::*;
    use libneo::{
        CapRights, EAGAIN, EPERM, PROCESS_STATE_EXITED, PROCESS_STATE_RUNNING, ProcessSnapshotV1,
        WAIT_FLAG_WNOHANG, meta_rights, process_rights, sys_derive_process, sys_inspect, sys_wait,
    };

    const WAIT_BLOCKING_CODE: i32 = 41;
    const WAIT_ALREADY_EXITED_CODE: i32 = 42;
    const WAIT_MULTIPLE_CODE: i32 = 43;
    const INSPECT_EXITED_CODE: i32 = 44;

    pub fn run() -> i32 {
        #[cfg(feature = "sys-wait-blocking")]
        {
            return sys_wait_blocking();
        }
        #[cfg(feature = "sys-wait-wnohang")]
        {
            return sys_wait_wnohang();
        }
        #[cfg(feature = "sys-wait-already-exited")]
        {
            return sys_wait_already_exited();
        }
        #[cfg(feature = "sys-wait-multiple-waiters")]
        {
            return sys_wait_multiple_waiters();
        }
        #[cfg(feature = "sys-inspect-running")]
        {
            return sys_inspect_running();
        }
        #[cfg(feature = "sys-inspect-exited")]
        {
            return sys_inspect_exited();
        }
        #[cfg(feature = "sys-inspect-rights-gate")]
        {
            return sys_inspect_rights_gate();
        }
        #[cfg(feature = "kernel-observability")]
        {
            return kernel_observability();
        }
        #[cfg(feature = "kernel-observability-disabled")]
        {
            return kernel_observability_disabled();
        }
    }

    #[cfg(feature = "kernel-observability")]
    fn kernel_observability() -> i32 {
        use libneo::{KERNEL_OBSERVABILITY_V1_VERSION, TRACE_KIND_SYSCALL, sys_observe};

        let activity = drive_kernel_observability_activity();
        if activity != 0 {
            return activity;
        }
        let stats = match sys_observe() {
            Ok(stats) => stats,
            Err(_) => return 60,
        };
        if stats.version != KERNEL_OBSERVABILITY_V1_VERSION {
            return 61;
        }
        if stats.cpu_count == 0 {
            return 62;
        }
        if !scheduler_observe_stats_sane(&stats) {
            return 63;
        }
        if !memory_observe_stats_sane(&stats) {
            return 64;
        }
        if stats.trace_count == 0 {
            return 65;
        }
        let mut saw_syscall = false;
        for event in stats.trace.iter().take(stats.trace_count as usize) {
            saw_syscall |= event.kind == TRACE_KIND_SYSCALL;
        }
        if !saw_syscall {
            return 66;
        }
        if !observe_read_only_governance_stabilizes(stats) {
            return 68;
        }
        let msg = b"[ neo-init ] observe: sys_observe ok version=3 sched=advanced memory=pressure trace=syscall read_only=governance\n";
        let _ = sys_write(1, msg.as_ptr(), msg.len());
        0
    }

    #[cfg(feature = "kernel-observability")]
    fn drive_kernel_observability_activity() -> i32 {
        let (main_local, main_worker) = match sys_chan_create() {
            Ok(handles) => handles,
            Err(_) => return 69,
        };
        let (block_local, block_worker) = match sys_chan_create() {
            Ok(handles) => handles,
            Err(_) => return 70,
        };
        let (_pid, worker) = match sys_spawn("worker", &[main_worker, block_worker]) {
            Ok(spawned) => spawned,
            Err(_) => return 71,
        };
        if sys_send(main_local, b"observe-ping") < 0 {
            return 72;
        }
        let mut pong = [0u8; 48];
        if sys_recv(main_local, &mut pong) <= 0 {
            return 73;
        }
        if sys_send(block_local, b"go-block") < 0 {
            return 74;
        }
        if sys_kill(worker) != 0 {
            return 75;
        }
        0
    }

    #[cfg(feature = "kernel-observability")]
    fn scheduler_observe_stats_sane(stats: &libneo::KernelObservabilityV1) -> bool {
        let active = observe_active_cpu_slots(stats.cpu_count);
        if active == 0 {
            return false;
        }
        let mut context_switches = 0u64;
        let mut dispatches = 0u64;
        let mut preemptions = 0u64;
        let mut ready_high_water = 0u64;
        let mut ticks = 0u64;
        for cpu in stats.sched.iter().take(active) {
            context_switches = context_switches.saturating_add(cpu.context_switches);
            dispatches = dispatches.saturating_add(cpu.dispatches);
            preemptions = preemptions
                .saturating_add(cpu.timer_preemptions)
                .saturating_add(cpu.voluntary_yields);
            ready_high_water = ready_high_water.max(cpu.ready_queue_high_water);
            ticks = ticks
                .saturating_add(cpu.run_ticks)
                .saturating_add(cpu.idle_ticks);
            if cpu.ready_queue_high_water < cpu.ready_queue_depth {
                return false;
            }
        }
        context_switches != 0
            && dispatches != 0
            && preemptions != 0
            && ready_high_water != 0
            && ticks != 0
    }

    #[cfg(feature = "kernel-observability")]
    fn memory_observe_stats_sane(stats: &libneo::KernelObservabilityV1) -> bool {
        let total_known_frames = stats.frames_used.saturating_add(stats.buddy_free_pages);
        stats.heap_high_water_bytes >= stats.heap_current_bytes
            && stats.heap_high_water_bytes != 0
            && stats.frames_used != 0
            && stats.buddy_free_pages != 0
            && stats.frames_used <= total_known_frames
    }

    #[cfg(feature = "kernel-observability")]
    fn observe_read_only_governance_stabilizes(mut before: libneo::KernelObservabilityV1) -> bool {
        for _ in 0..1000 {
            let after = match libneo::sys_observe() {
                Ok(stats) => stats,
                Err(_) => return false,
            };
            if observe_read_only_governance_stable(&before, &after) {
                return true;
            }
            before = after;
        }
        false
    }

    #[cfg(feature = "kernel-observability")]
    fn observe_read_only_governance_stable(
        before: &libneo::KernelObservabilityV1,
        after: &libneo::KernelObservabilityV1,
    ) -> bool {
        if before.cpu_count != after.cpu_count
            || before.heap_current_bytes != after.heap_current_bytes
            || before.heap_high_water_bytes != after.heap_high_water_bytes
            || before.heap_alloc_failures != after.heap_alloc_failures
            || before.frames_used != after.frames_used
            || before.buddy_free_pages != after.buddy_free_pages
            || before.buddy_largest_free_order != after.buddy_largest_free_order
        {
            return false;
        }
        let active = observe_active_cpu_slots(before.cpu_count);
        for idx in 0..active {
            let before_cpu = before.sched[idx];
            let after_cpu = after.sched[idx];
            if before_cpu.ready_queue_depth != after_cpu.ready_queue_depth
                || before_cpu.ready_queue_high_water != after_cpu.ready_queue_high_water
            {
                return false;
            }
        }
        true
    }

    #[cfg(feature = "kernel-observability")]
    fn observe_active_cpu_slots(cpu_count: u32) -> usize {
        let count = cpu_count as usize;
        if count < libneo::KERNEL_OBSERVABILITY_MAX_CPUS {
            count
        } else {
            libneo::KERNEL_OBSERVABILITY_MAX_CPUS
        }
    }

    #[cfg(feature = "kernel-observability-disabled")]
    fn kernel_observability_disabled() -> i32 {
        use libneo::{ENOSYS, sys_observe};

        match sys_observe() {
            Err(ENOSYS) => {
                let msg =
                    b"[ neo-init ] observe: sys_observe disabled errno=ENOSYS read_surface=off\n";
                let _ = sys_write(1, msg.as_ptr(), msg.len());
                0
            }
            Err(_) => 67,
            Ok(_) => 68,
        }
    }

    fn spawn_worker(handles: &[libneo::Handle]) -> Result<libneo::ProcessHandle, i32> {
        match sys_spawn("worker", handles) {
            Ok((_pid, handle)) => Ok(handle),
            Err(_) => Err(1),
        }
    }

    fn spawn_go_worker() -> Result<(libneo::Handle, libneo::ProcessHandle), i32> {
        let (ctl_local, ctl_worker) = sys_chan_create().map_err(|_| 2)?;
        let handle = spawn_worker(&[ctl_worker])?;
        Ok((ctl_local, handle))
    }

    fn release_go_worker(ctl: libneo::Handle) -> i32 {
        if sys_send(ctl, b"go") < 0 { 3 } else { 0 }
    }

    fn expect_exited(snapshot: ProcessSnapshotV1, code: i32) -> bool {
        snapshot.version == libneo::PROCESS_SNAPSHOT_V1_VERSION
            && snapshot.state == PROCESS_STATE_EXITED
            && snapshot.exit_code == code
    }

    fn wait_for_exited_snapshot(
        handle: libneo::ProcessHandle,
        code: i32,
    ) -> Result<ProcessSnapshotV1, i32> {
        for _ in 0..100_000 {
            match sys_inspect(handle) {
                Ok(snapshot) if expect_exited(snapshot, code) => return Ok(snapshot),
                Ok(snapshot) if snapshot.state == PROCESS_STATE_RUNNING => {}
                Ok(_) => return Err(7),
                Err(_) => return Err(8),
            }
        }
        Err(9)
    }

    fn sys_wait_blocking() -> i32 {
        let handle = match spawn_worker(&[]) {
            Ok(handle) => handle,
            Err(e) => return e,
        };
        match sys_wait(handle, 0) {
            Ok(code) if code == WAIT_BLOCKING_CODE => 0,
            _ => 4,
        }
    }

    fn sys_wait_wnohang() -> i32 {
        let handle = match spawn_worker(&[]) {
            Ok(handle) => handle,
            Err(e) => return e,
        };
        match sys_wait(handle, WAIT_FLAG_WNOHANG) {
            Err(e) if e == EAGAIN => {}
            _ => return 4,
        }
        if sys_kill(handle) != 0 {
            return 5;
        }
        0
    }

    fn sys_wait_already_exited() -> i32 {
        let (ctl, handle) = match spawn_go_worker() {
            Ok(v) => v,
            Err(e) => return e,
        };
        let rc = release_go_worker(ctl);
        if rc != 0 {
            return rc;
        }
        if let Err(e) = wait_for_exited_snapshot(handle, WAIT_ALREADY_EXITED_CODE) {
            return e;
        }
        match sys_wait(handle, 0) {
            Ok(code) if code == WAIT_ALREADY_EXITED_CODE => 0,
            _ => 4,
        }
    }

    fn sys_wait_multiple_waiters() -> i32 {
        let (ctl, handle) = match spawn_go_worker() {
            Ok(v) => v,
            Err(e) => return e,
        };
        let second = match sys_derive_process(
            handle,
            CapRights::new(meta_rights::USE, process_rights::WAIT),
        ) {
            Ok(handle) => handle,
            Err(_) => return 4,
        };
        let rc = release_go_worker(ctl);
        if rc != 0 {
            return rc;
        }
        if let Err(e) = wait_for_exited_snapshot(handle, WAIT_MULTIPLE_CODE) {
            return e;
        }
        match sys_wait(handle, 0) {
            Ok(code) if code == WAIT_MULTIPLE_CODE => {}
            _ => return 5,
        }
        match sys_wait(second, 0) {
            Err(e) if e == ESRCH => 0,
            _ => 6,
        }
    }

    fn sys_inspect_running() -> i32 {
        let handle = match spawn_worker(&[]) {
            Ok(handle) => handle,
            Err(e) => return e,
        };
        let snapshot = match sys_inspect(handle) {
            Ok(snapshot) => snapshot,
            Err(_) => return 4,
        };
        if snapshot.state != PROCESS_STATE_RUNNING || snapshot.thread_count != 1 {
            return 5;
        }
        if sys_kill(handle) != 0 {
            return 6;
        }
        0
    }

    fn sys_inspect_exited() -> i32 {
        let (ctl, handle) = match spawn_go_worker() {
            Ok(v) => v,
            Err(e) => return e,
        };
        let rc = release_go_worker(ctl);
        if rc != 0 {
            return rc;
        }
        let snapshot = match wait_for_exited_snapshot(handle, INSPECT_EXITED_CODE) {
            Ok(snapshot) => snapshot,
            Err(e) => return e,
        };
        if !expect_exited(snapshot, INSPECT_EXITED_CODE) {
            return 5;
        }
        match sys_wait(handle, 0) {
            Ok(code) if code == INSPECT_EXITED_CODE => 0,
            _ => 6,
        }
    }

    fn sys_inspect_rights_gate() -> i32 {
        let handle = match spawn_worker(&[]) {
            Ok(handle) => handle,
            Err(e) => return e,
        };
        let wait_only = match sys_derive_process(
            handle,
            CapRights::new(meta_rights::USE, process_rights::WAIT),
        ) {
            Ok(handle) => handle,
            Err(_) => return 4,
        };
        match sys_inspect(wait_only) {
            Err(e) if e == EPERM => {}
            _ => return 5,
        }
        if sys_kill(handle) != 0 {
            return 6;
        }
        0
    }
}

#[cfg(feature = "sched-ready-queue-capacity-pressure")]
fn run_selected_scenario() -> i32 {
    sched_capacity_tests::run()
}

#[cfg(feature = "ipc-rendezvous-translate-fault")]
fn run_selected_scenario() -> i32 {
    run_ipc_rendezvous_translate_fault()
}

#[cfg(feature = "ipc-rendezvous-translate-fault")]
fn run_ipc_rendezvous_translate_fault() -> i32 {
    const UNMAPPED_USER_RECV: u64 = 0x0000_4000_0000_0000;

    let (local, worker) = match sys_chan_create() {
        Ok(handles) => handles,
        Err(_) => return 1,
    };
    let (_pid, worker_handle) = match sys_spawn("worker", &[worker]) {
        Ok(spawned) => spawned,
        Err(_) => return 2,
    };

    let recv_ret = unsafe { libneo::sys_recv_raw(local, UNMAPPED_USER_RECV, 4) };
    if recv_ret != libneo::EFAULT {
        return 3;
    }

    match libneo::sys_wait(worker_handle, 0) {
        Ok(0) => 0,
        Ok(_) => 4,
        Err(_) => 5,
    }
}

#[cfg(feature = "sched-ready-queue-capacity-pressure")]
mod sched_capacity_tests {
    use super::*;
    use libneo::{EAGAIN, ProcessHandle};

    const MAX_ATTEMPTS: usize = 96;
    const MIN_EXPECTED_SPAWNS: usize = 60;

    pub fn run() -> i32 {
        let mut workers: Vec<ProcessHandle> = Vec::new();
        let mut saw_eagain = false;

        for _ in 0..MAX_ATTEMPTS {
            match sys_spawn("worker", &[]) {
                Ok((_pid, handle)) => workers.push(handle),
                Err(e) if e == EAGAIN => {
                    saw_eagain = true;
                    break;
                }
                Err(_) => {
                    let _ = cleanup(&workers);
                    return 1;
                }
            }
        }

        let spawned = workers.len();
        let cleanup_rc = cleanup(&workers);
        if cleanup_rc != 0 {
            return cleanup_rc;
        }
        if !saw_eagain {
            return 2;
        }
        if spawned < MIN_EXPECTED_SPAWNS {
            return 3;
        }
        0
    }

    fn cleanup(workers: &[ProcessHandle]) -> i32 {
        for handle in workers.iter().rev() {
            if sys_kill(*handle) != 0 {
                return 4;
            }
        }
        0
    }
}

#[cfg(any(
    feature = "vfs-testfs-open-read-close",
    feature = "vfs-testfs-write-readback",
    feature = "vfs-type-vs-rights-errors",
    feature = "vfs-stdout-compat",
    feature = "vfs-block-smoke",
    feature = "vfs-readdir-testfs-root",
    feature = "vfs-readdir-requires-right",
    feature = "vfs-readdir-regular-is-enotdir",
    feature = "vfs-directory-read-is-eisdir",
    feature = "readdir-cursor-eof-stable",
    feature = "readdir-buffer-too-small"
))]
fn run_selected_scenario() -> i32 {
    vfs_tests::run()
}

/// Slice-6 user ABI proof. Handles 0/1/2 are the only root compute objects
/// inherited from the kernel: READ buffer, WRITE buffer, and queue.  Neither a
/// normal G1 buffer nor an attenuated queue can be turned into submission
/// authority, and the cross-process grant dies when its root is revoked.
#[cfg(feature = "gpu-glass-slice6")]
mod gpu_glass_slice6_tests {
    use libneo::{
        CapRights, EBADF, EPERM, FORMAT_XRGB8888_LE, Handle, encode_buffer_desc,
        gpu_compute_rights, meta_rights, sys_buffer_create, sys_chan_create, sys_derive,
        sys_gpu_compute_fence_wait, sys_gpu_compute_submit_blur, sys_grant, sys_recv, sys_revoke,
        sys_send, sys_spawn,
    };

    const SRC: Handle = Handle(0);
    const DST: Handle = Handle(1);
    const QUEUE: Handle = Handle(2);

    pub fn run() -> i32 {
        // A trusted triple produces a completion capability, never a present.
        let fence = match sys_gpu_compute_submit_blur(SRC, DST, QUEUE) {
            Ok(fence) => fence,
            Err(_) => return 1,
        };
        if sys_gpu_compute_fence_wait(fence) != 0 {
            return 2;
        }

        // Dropping QUEUE_SUBMIT by derivation must be enforced at the service,
        // not merely represented in metadata.
        let no_submit = match sys_derive(QUEUE, CapRights::new(meta_rights::USE, 0)) {
            Ok(handle) => handle,
            Err(_) => return 3,
        };
        match sys_gpu_compute_submit_blur(SRC, DST, no_submit) {
            Err(e) if e == EPERM => {}
            _ => return 4,
        }

        // A user-created ordinary G1 buffer is never a GpuBufferCap. This is
        // the observable no-self-mint boundary (there is no create syscall for
        // any GpuCompute variant).
        let ordinary =
            match sys_buffer_create(&encode_buffer_desc(8, 8, 32, FORMAT_XRGB8888_LE, 256)) {
                Ok(handle) => handle,
                Err(_) => return 5,
            };
        match sys_gpu_compute_submit_blur(ordinary, DST, QUEUE) {
            Err(e) if e == EBADF => {}
            _ => return 6,
        }

        // Actual cross-process least-rights grant.  The worker receives only a
        // READ GpuBufferCap, acknowledges it, then proves revocation removes
        // that descendant rather than leaving a stale usable GPU object.
        let (ctl, worker_ctl) = match sys_chan_create() {
            Ok(handles) => handles,
            Err(_) => return 7,
        };
        if sys_spawn("worker", &[worker_ctl]).is_err() {
            return 8;
        }
        if sys_send(ctl, b"go") < 0
            || sys_grant(
                ctl,
                SRC,
                CapRights::new(meta_rights::USE, gpu_compute_rights::BUFFER_READ),
            ) != 0
        {
            return 9;
        }
        let mut ack = [0u8; 8];
        if sys_recv(ctl, &mut ack) != 5 || &ack[..5] != b"grant" {
            return 10;
        }
        if sys_revoke(SRC) != 0 {
            return 11;
        }
        if sys_send(ctl, b"revoke") < 0 {
            return 12;
        }
        if sys_recv(ctl, &mut ack) != 7 || &ack[..7] != b"revoked" {
            return 13;
        }
        match sys_gpu_compute_submit_blur(SRC, DST, QUEUE) {
            Err(e) if e == EBADF => {}
            _ => return 14,
        }

        // Fences descend from the queue. Its revoke therefore invalidates the
        // fence too: completion cannot become ambient authority.
        if sys_revoke(QUEUE) != 0 || sys_gpu_compute_fence_wait(fence) != EBADF {
            return 15;
        }
        0
    }
}

#[cfg(not(any(
    feature = "cap-mint-and-use",
    feature = "cap-derive-subset",
    feature = "cap-derive-rights-violation",
    feature = "cap-derive-missing-meta",
    feature = "cap-grant-basic",
    feature = "cap-grant-missing-meta",
    feature = "cap-revoke-local",
    feature = "cap-graph-revoke",
    feature = "cap-revoke-cross-process",
    feature = "cap-revoke-descendants-walk",
    feature = "cap-revoke-spawn-inheritance-descendants",
    feature = "cap-survive-exit",
    feature = "file-cap-derive-grant",
    feature = "cap-rooted-open-test",
    feature = "mem-quota-test",
    feature = "kill-ready-in-kernel-test",
    feature = "obj-quota-test",
    feature = "user-copy-bounds-test",
    feature = "socket-teardown-test",
    feature = "exit-codes-test",
    feature = "mount-authority-test",
    feature = "interface-cap-derive-revoke",
    feature = "arthur-grant-revoke",
    feature = "atrium-slice1",
    feature = "atrium-surface-protocol",
    feature = "atrium-input-routing",
    feature = "dw1-desktop-wm",
    feature = "dw2-scene-policy",
    feature = "atrium-slice3",
    feature = "atrium-interactive",
    feature = "atrium-interactive-baremetal",
    feature = "gpu-glass-slice6",
    feature = "arthur-compositor",
    feature = "sys-wait-blocking",
    feature = "sys-wait-wnohang",
    feature = "sys-wait-already-exited",
    feature = "sys-wait-multiple-waiters",
    feature = "sys-inspect-running",
    feature = "sys-inspect-exited",
    feature = "sys-inspect-rights-gate",
    feature = "kernel-observability",
    feature = "kernel-observability-disabled",
    feature = "sched-ready-queue-capacity-pressure",
    feature = "neo-shell-repl",
    feature = "neo-shell-builtins",
    feature = "arthur-action-surface",
    feature = "arthur-converse",
    feature = "arthur-boots-thinking",
    feature = "arthur-act",
    feature = "arthur-grants",
    feature = "arthur-console",
    feature = "v1-console-liveness",
    feature = "arthur-memory",
    feature = "boot-neofs-userland-persistence",
    feature = "ipc-rendezvous-translate-fault",
    feature = "vfs-testfs-open-read-close",
    feature = "vfs-testfs-write-readback",
    feature = "vfs-type-vs-rights-errors",
    feature = "vfs-stdout-compat",
    feature = "vfs-block-smoke",
    feature = "vfs-readdir-testfs-root",
    feature = "vfs-readdir-requires-right",
    feature = "vfs-readdir-regular-is-enotdir",
    feature = "vfs-directory-read-is-eisdir",
    feature = "readdir-cursor-eof-stable",
    feature = "readdir-buffer-too-small",
    feature = "tensor-create-query-drop",
    feature = "model-load-cascade",
    feature = "infer-mlp-forward",
    feature = "infer-transformer-forward",
    feature = "infer-embedding-forward",
    feature = "infer-embedding-oob-rejected",
    feature = "swapgs-irq-ring3",
    feature = "syscall-mxcsr-entry",
    feature = "phase4-socket-udp-loopback",
    feature = "phase4-slaac-udp-ipv6-loopback",
    feature = "phase4-https-fetch",
    feature = "net-dial-cap-kill-switch",
    feature = "fetch-broker",
    feature = "vm-primitives",
    feature = "wallclock-probe",
    feature = "threads-futex",
    feature = "threads-tls",
    feature = "threads-exit-group",
    feature = "threads-guard-fault",
    feature = "threads-cap-compose",
    feature = "vault-phase-b",
    feature = "desktop-session"
)))]
fn run_selected_scenario() -> i32 {
    run_ipc_heap_kill_smoke()
}

/// OT-61 browser C1.0 gate: the VM+clock primitive battery. Exercises the new
/// `SYS_VM_MAP/UNMAP/PROTECT` + `SYS_CLOCK_GETTIME` syscalls and returns 0 iff
/// every check passes; a distinct non-zero code localizes the first failure.
#[cfg(feature = "vm-primitives")]
fn run_selected_scenario() -> i32 {
    run_vm_primitives()
}

#[cfg(feature = "vm-primitives")]
fn run_vm_primitives() -> i32 {
    use libneo::{
        CLOCK_MONOTONIC, PROT_EXEC, PROT_READ, PROT_WRITE, sys_clock_gettime, sys_vm_map,
        sys_vm_protect, sys_vm_unmap,
    };
    const EINVAL: i64 = -22;
    const EFAULT: i64 = -14;
    let say = |m: &[u8]| {
        let _ = sys_write(1, m.as_ptr(), m.len());
    };
    say(b"[ vm-primitives ] C1.0 VM+clock battery\n");

    // 0. AAG must-fix negatives: a hostile arg must return an errno, NEVER panic
    // the kernel (panic=abort). Non-canonical va → EFAULT (validated before
    // VirtAddr); oversized size → EINVAL (bounded before the map loop).
    // 0x1234_5678_9abc_def0 is non-canonical (bits 48..63 not sign-extended).
    let noncanon = 0x1234_5678_9abc_def0u64 as *mut u8;
    if sys_vm_unmap(noncanon, 0x1000) != Err(EFAULT) {
        say(b"[ vm-primitives ] FAIL: non-canonical unmap va not EFAULT\n");
        return 5;
    }
    if sys_vm_protect(noncanon, 0x1000, PROT_READ) != Err(EFAULT) {
        say(b"[ vm-primitives ] FAIL: non-canonical protect va not EFAULT\n");
        return 6;
    }
    let oversize = 64 * 1024 * 1024; // > MAX_MAP_REGION_SIZE (16 MiB)
    if sys_vm_map(oversize, PROT_READ | PROT_WRITE, false) != Err(EINVAL) {
        say(b"[ vm-primitives ] FAIL: oversized map not EINVAL\n");
        return 7;
    }
    if sys_vm_unmap(0x1000 as *mut u8, oversize) != Err(EINVAL) {
        say(b"[ vm-primitives ] FAIL: oversized unmap not EINVAL\n");
        return 8;
    }
    if sys_vm_protect(0x1000 as *mut u8, oversize, PROT_READ) != Err(EINVAL) {
        say(b"[ vm-primitives ] FAIL: oversized protect not EINVAL\n");
        return 9;
    }
    say(b"[ vm-primitives ] boundary: non-canonical va=EFAULT, oversized size=EINVAL (no panic) ok\n");

    // 1. W^X-NEGATIVE: a W+X map / a W+X protect are REFUSED with EINVAL (A1).
    if sys_vm_map(0x1000, PROT_READ | PROT_WRITE | PROT_EXEC, false) != Err(EINVAL) {
        say(b"[ vm-primitives ] FAIL: W+X map was not refused\n");
        return 11;
    }
    say(b"[ vm-primitives ] wx_negative: W+X map refused (EINVAL) ok\n");

    // 2. map -> write -> protect(RO) -> protect(RX) -> protect(RW) -> unmap.
    let va = match sys_vm_map(0x2000, PROT_READ | PROT_WRITE, false) {
        Ok(p) => p,
        Err(_) => {
            say(b"[ vm-primitives ] FAIL: RW map\n");
            return 12;
        }
    };
    // SAFETY: `va` is a fresh 0x2000-byte RW region owned by this process.
    unsafe {
        core::ptr::write_volatile(va, 0xAB);
        core::ptr::write_volatile(va.add(0x1000), 0xCD);
        if core::ptr::read_volatile(va) != 0xAB || core::ptr::read_volatile(va.add(0x1000)) != 0xCD
        {
            say(b"[ vm-primitives ] FAIL: RW readback\n");
            return 13;
        }
    }
    if sys_vm_protect(va, 0x2000, PROT_READ).is_err() {
        say(b"[ vm-primitives ] FAIL: protect RO\n");
        return 14;
    }
    if sys_vm_protect(va, 0x2000, PROT_READ | PROT_EXEC).is_err() {
        say(b"[ vm-primitives ] FAIL: protect RX\n");
        return 15;
    }
    // W^X on protect: RX -> W+X is refused.
    if sys_vm_protect(va, 0x2000, PROT_READ | PROT_WRITE | PROT_EXEC) != Err(EINVAL) {
        say(b"[ vm-primitives ] FAIL: W+X protect was not refused\n");
        return 16;
    }
    if sys_vm_protect(va, 0x2000, PROT_READ | PROT_WRITE).is_err() {
        say(b"[ vm-primitives ] FAIL: protect back to RW\n");
        return 17;
    }
    // SAFETY: RW again — the write must succeed.
    unsafe {
        core::ptr::write_volatile(va, 0xEE);
        if core::ptr::read_volatile(va) != 0xEE {
            say(b"[ vm-primitives ] FAIL: post-reprotect write\n");
            return 18;
        }
    }
    if sys_vm_unmap(va, 0x2000).is_err() {
        say(b"[ vm-primitives ] FAIL: unmap\n");
        return 19;
    }
    // Double-unmap is fail-closed (EINVAL).
    if sys_vm_unmap(va, 0x2000) != Err(EINVAL) {
        say(b"[ vm-primitives ] FAIL: double-unmap not refused\n");
        return 20;
    }
    say(b"[ vm-primitives ] roundtrip: map/write/protect(RO/RX/RW)/unmap ok\n");

    // 3. GUARD PAGE: a guarded region's guard page (one page below base) is
    // UNMAPPED — protecting it returns EINVAL (NotMapped). Confirms the guard
    // exists without faulting (a stack-overflow fault into it is C1.1's, when
    // threads use guarded stacks).
    let gva = match sys_vm_map(0x1000, PROT_READ | PROT_WRITE, true) {
        Ok(p) => p,
        Err(_) => {
            say(b"[ vm-primitives ] FAIL: guarded map\n");
            return 21;
        }
    };
    // SAFETY: pointer arithmetic only; the guard page is not dereferenced.
    let guard = unsafe { gva.sub(0x1000) };
    if sys_vm_protect(guard, 0x1000, PROT_READ) != Err(EINVAL) {
        say(b"[ vm-primitives ] FAIL: guard page was mapped (protect succeeded)\n");
        return 22;
    }
    say(b"[ vm-primitives ] guard: page below the region is unmapped ok\n");

    // 4. CLOCK: monotonic is nondecreasing; an invalid clock id is EINVAL.
    let t0 = match sys_clock_gettime(CLOCK_MONOTONIC) {
        Ok(t) => t,
        Err(_) => {
            say(b"[ vm-primitives ] FAIL: clock read\n");
            return 23;
        }
    };
    // Busy-poll until the monotonic clock advances (the PIT fires + increments
    // TICKS during this ring-3 loop; IF=1). Bounded so a broken clock fails.
    // The GATE is nondecreasing (t_now >= t0) on every read; advancement is the
    // stronger observation we also require within the bound.
    let mut t1 = t0;
    let mut advanced = false;
    for _ in 0..2_000_000u64 {
        t1 = match sys_clock_gettime(CLOCK_MONOTONIC) {
            Ok(t) => t,
            Err(_) => return 24,
        };
        if t1 < t0 {
            say(b"[ vm-primitives ] FAIL: clock went backwards\n");
            return 25;
        }
        if t1 > t0 {
            advanced = true;
            break;
        }
    }
    if !advanced {
        say(b"[ vm-primitives ] FAIL: monotonic clock never advanced\n");
        return 27;
    }
    if sys_clock_gettime(99) != Err(EINVAL) {
        say(b"[ vm-primitives ] FAIL: invalid clock id not refused\n");
        return 26;
    }
    say(b"[ vm-primitives ] clock: monotonic nondecreasing + bad-id refused ok\n");

    // 5. OT-85 kill-switch regression: this kernel has no `rtc-epoch`, so
    // realtime is boot-relative exactly as C1.0: equal to monotonic when both
    // are read inside one tick (bracketed by two equal monotonic reads).
    match wallclock::sample() {
        Some(s) if s.base_ns == 0 => {}
        Some(_) => {
            say(b"[ vm-primitives ] FAIL: realtime != monotonic without rtc-epoch\n");
            return 28;
        }
        None => {
            say(b"[ vm-primitives ] FAIL: realtime/monotonic never read inside one tick\n");
            return 29;
        }
    }
    say(b"[ vm-primitives ] clock: realtime == monotonic (boot-relative, rtc-epoch off) ok\n");

    say(b"[ vm-primitives ] PASS: vm map/unmap/protect (W^X) + guard + clock\n");
    0
}

/// OT-85 gate: the wall-clock probe (see `wallclock`).
#[cfg(feature = "wallclock-probe")]
fn run_selected_scenario() -> i32 {
    wallclock::probe()
}

/// OT-85: reading the wall clock, shared by the probe and the C1.0 battery's
/// kill-switch check.
#[cfg(any(feature = "vm-primitives", feature = "wallclock-probe"))]
mod wallclock {
    use libneo::{CLOCK_MONOTONIC, CLOCK_REALTIME, sys_clock_gettime};

    /// One reading of both clocks inside a single tick.
    #[derive(Clone, Copy)]
    pub struct Sample {
        pub realtime_ns: u64,
        pub monotonic_ns: u64,
        /// realtime - monotonic: the boot epoch (0 = boot-relative).
        pub base_ns: u64,
    }

    /// Read monotonic, realtime, monotonic until the two monotonic reads
    /// agree (no tick landed in between), so realtime - monotonic is the
    /// kernel's base exactly. Bounded: `None` if the clocks never read.
    pub fn sample() -> Option<Sample> {
        for _ in 0..1_000 {
            let m0 = sys_clock_gettime(CLOCK_MONOTONIC).ok()?;
            let r = sys_clock_gettime(CLOCK_REALTIME).ok()?;
            let m1 = sys_clock_gettime(CLOCK_MONOTONIC).ok()?;
            if m0 == m1 && r >= m0 {
                return Some(Sample {
                    realtime_ns: r,
                    monotonic_ns: m0,
                    base_ns: r - m0,
                });
            }
        }
        None
    }

    #[cfg(feature = "wallclock-probe")]
    fn say(parts: &[&[u8]]) {
        for p in parts {
            let _ = libneo::sys_write(1, p.as_ptr(), p.len());
        }
    }

    #[cfg(feature = "wallclock-probe")]
    fn num(n: u64) -> libneo::civil::Text<20> {
        let mut t = libneo::civil::Text::<20>::new();
        // u64 in two halves (push_num takes u32 values).
        if n >= 1_000_000_000 {
            t.push_num((n / 1_000_000_000) as u32, 1);
            t.push_num((n % 1_000_000_000) as u32, 9);
        } else {
            t.push_num(n as u32, 1);
        }
        t
    }

    /// The probe. Samples the clocks across monotonic ticks: monotonic must
    /// never go back and must advance; the base (realtime - monotonic) must
    /// stay within one tick (10 ms) across every sample. Seeded: the boot
    /// instant formats, through libneo::civil (the Atrium's formatter), as
    /// the pinned QEMU RTC (Wednesday, October 21, 2026, 5:48 PM). Fallback:
    /// the base is 0 (realtime == monotonic: boot-relative).
    #[cfg(feature = "wallclock-probe")]
    pub fn probe() -> i32 {
        say(&[b"[ wallclock ] OT-85 wall-clock probe\n"]);
        let Some(first) = sample() else {
            say(&[b"[ wallclock ] FAIL: clocks never read inside one tick\n"]);
            return 2;
        };
        let (mut lo, mut hi) = (first.base_ns, first.base_ns);
        let mut last_mono = first.monotonic_ns;
        let mut ticks_seen = 0;
        let mut last = first;
        for _ in 0..5_000_000u64 {
            let Some(s) = sample() else {
                return 3;
            };
            if s.monotonic_ns < last_mono {
                say(&[b"[ wallclock ] FAIL: monotonic went backwards\n"]);
                return 4;
            }
            if s.monotonic_ns > last_mono {
                ticks_seen += 1;
            }
            last_mono = s.monotonic_ns;
            lo = lo.min(s.base_ns);
            hi = hi.max(s.base_ns);
            last = s;
            if ticks_seen >= 5 {
                break;
            }
        }
        if ticks_seen < 5 {
            say(&[b"[ wallclock ] FAIL: monotonic did not advance 5 ticks\n"]);
            return 5;
        }
        let spread = hi - lo;
        say(&[
            b"[ wallclock ] base_ns=",
            num(first.base_ns).as_bytes(),
            b" spread_ns=",
            num(spread).as_bytes(),
            b" ticks=5 monotonic=nondecreasing\n",
        ]);
        if spread > 10_000_000 {
            say(&[b"[ wallclock ] FAIL: base moved by more than one tick\n"]);
            return 6;
        }
        #[cfg(feature = "wallclock-probe-fallback")]
        {
            if hi != 0 {
                say(&[b"[ wallclock ] FAIL: realtime not boot-relative after a fallback\n"]);
                return 7;
            }
            say(&[
                b"[ wallclock ] realtime=boot-relative realtime_ns=",
                num(last.realtime_ns).as_bytes(),
                b" monotonic_ns=",
                num(last.monotonic_ns).as_bytes(),
                b"\n[ wallclock ] PASS: fallback keeps boot-relative realtime\n",
            ]);
            0
        }
        #[cfg(not(feature = "wallclock-probe-fallback"))]
        {
            use libneo::civil;
            let boot = civil::from_realtime_ns(first.base_ns);
            let now = civil::from_realtime_ns(last.realtime_ns);
            say(&[
                b"[ wallclock ] boot date=\"",
                boot.date_text().as_bytes(),
                b"\" time=\"",
                boot.time_text().as_bytes(),
                b"\"\n[ wallclock ] now realtime_ns=",
                num(last.realtime_ns).as_bytes(),
                b" date=\"",
                now.date_text().as_bytes(),
                b"\" time=\"",
                now.time_text().as_bytes(),
                b"\"\n",
            ]);
            if boot.date_text().as_str() != "Wednesday, October 21, 2026" {
                say(&[b"[ wallclock ] FAIL: boot date is not the pinned RTC's\n"]);
                return 8;
            }
            if boot.time_text().as_str() != "5:48 PM" {
                say(&[b"[ wallclock ] FAIL: boot time is not the pinned RTC's\n"]);
                return 9;
            }
            say(&[b"[ wallclock ] PASS: boot instant = pinned RTC (Wednesday, October 21, 2026, 5:48 PM)\n"]);
            0
        }
    }
}

/// OT-61 C1.2 gate: the TLS battery. Exercises `SYS_THREAD_SET_TLS`, the
/// per-thread FS.base the kernel loads on every switch-in, and the userland key
/// layer (`libneo::ntls`). Returns 0 iff every check passes; the code names the
/// first failing check (`check * 100 + detail`).
#[cfg(feature = "threads-tls")]
fn run_selected_scenario() -> i32 {
    threads_tls_battery::run()
}

/// OT-61 C1.3b (CP-86): the cross-CPU process-retire battery. Spawns three
/// `threads-exit-group` workers (with the C1 capability) and hands each its
/// role over a channel; each must end with its expected code. A worker whose
/// process never finishes hangs `sys_wait` (the scenario then times out).
/// Returns 0, or `role * 100 + detail` for the first failure.
#[cfg(feature = "threads-exit-group")]
fn run_selected_scenario() -> i32 {
    threads_exit_group_battery::run()
}

#[cfg(feature = "threads-exit-group")]
mod threads_exit_group_battery {
    use libneo::{Handle, sys_chan_create, sys_send, sys_spawn, sys_wait, sys_write};

    fn say(m: &[u8]) {
        let _ = sys_write(1, m.as_ptr(), m.len());
    }

    /// Spawn a worker with the capability (handle 0) and a channel end
    /// (handle 1), send it `role`, and wait for its exit code.
    fn child(role: u8) -> Result<i32, i32> {
        let (mine, theirs) = sys_chan_create().map_err(|_| 1)?;
        let (_, h) = sys_spawn("worker", &[Handle(0), theirs]).map_err(|_| 2)?;
        if sys_send(mine, &[role]) < 0 {
            return Err(3);
        }
        sys_wait(h, 0).map_err(|_| 4)
    }

    pub fn run() -> i32 {
        say(b"[ threads-exit-group ] C1.3b cross-CPU process retire\n");
        for (role, want, what) in [
            (
                1u8,
                7,
                &b"SYS_EXIT(7) with a ring-3 spinner on CPU 2 and a futex-parked sibling"[..],
            ),
            (
                2u8,
                139,
                &b"a ring-3 fault with a ring-3 spinner on CPU 2"[..],
            ),
            (
                3u8,
                7,
                &b"SYS_EXIT(7) with a futex waiter parking after the retire's purge"[..],
            ),
            (
                4u8,
                7,
                &b"SYS_EXIT(7) with a sibling inside a ring-0 syscall on CPU 2"[..],
            ),
            (
                5u8,
                7,
                &b"SYS_EXIT(7) with a SYS_THREAD_JOIN parking after the retire's purge"[..],
            ),
            (
                6u8,
                7,
                &b"SYS_EXIT(7) with the retire IPI landing in an IF=1 syscall return on CPU 2"[..],
            ),
        ] {
            match child(role) {
                Ok(code) if code == want => {
                    say(b"[ threads-exit-group ] ok: ");
                    say(what);
                    say(b"\n");
                }
                Ok(_) => {
                    say(b"[ threads-exit-group ] FAIL: wrong exit code: ");
                    say(what);
                    say(b"\n");
                    return i32::from(role) * 100 + 10;
                }
                Err(d) => return i32::from(role) * 100 + d,
            }
        }
        say(b"[ threads-exit-group ] PASS\n");
        0
    }
}

#[cfg(feature = "threads-tls")]
mod threads_tls_battery {
    use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

    use libneo::nthread;
    use libneo::ntls::{self, DESTRUCTOR_ITERATIONS};
    use libneo::{
        sys_sched_yield, sys_spawn, sys_test_thread_create_on, sys_test_tls_park,
        sys_test_tls_probe, sys_test_tls_sleep, sys_test_tls_unpark, sys_thread_exit,
        sys_thread_get_info, sys_thread_join, sys_thread_self, sys_thread_set_tls, sys_wait,
        sys_write,
    };

    const EFAULT: i64 = -14;
    /// Bound on a yield-wait for a sibling that is contractually about to act;
    /// running out means the sibling never did (a hang, reported, not waited).
    const SPIN_LIMIT: u64 = 20_000_000;

    fn say(m: &[u8]) {
        let _ = sys_write(1, m.as_ptr(), m.len());
    }

    /// This thread's control block address, from its stack bounds (the C1.1
    /// locator, independent of FS).
    fn own_tcb() -> Option<u64> {
        sys_thread_get_info(0).ok().map(|info| info.usable_low)
    }

    /// FS.base is right for this thread: the kernel's recorded value, the live
    /// `IA32_FS_BASE` and `fs:[0]` all equal its own control block. Returns the
    /// CPU it runs on, or a detail code: 1 no bounds, 2 probe failed, 3 recorded
    /// value wrong, 4 MSR wrong, 5 `fs:[0]` wrong.
    fn fs_ok() -> Result<u64, i32> {
        let tcb = own_tcb().ok_or(1)?;
        let [cpu, recorded, msr] = sys_test_tls_probe().map_err(|_| 2)?;
        if recorded != tcb {
            return Err(3);
        }
        if msr != tcb {
            return Err(4);
        }
        if ntls::current_tcb() as u64 != tcb {
            return Err(5);
        }
        Ok(cpu)
    }

    /// Yield until `done()` holds; `false` if it never did.
    fn wait_until(done: impl Fn() -> bool) -> bool {
        let mut spins = 0u64;
        while !done() {
            sys_sched_yield();
            spins += 1;
            if spins > SPIN_LIMIT {
                return false;
            }
        }
        true
    }

    // ── check 2: isolation, and FS.base across context switches ────────────
    const ISO_THREADS: u32 = 4;
    static ISO_KEY: AtomicUsize = AtomicUsize::new(0);
    static ISO_ARRIVED: AtomicU32 = AtomicU32::new(0);

    extern "C" fn iso_child(i: usize) -> i32 {
        let key = ISO_KEY.load(Ordering::Acquire);
        let mine = (0x1000 + i) as *mut u8;
        if !ntls::set(key, mine) {
            return 1;
        }
        if let Err(d) = fs_ok() {
            return 10 + d;
        }
        ISO_ARRIVED.fetch_add(1, Ordering::AcqRel);
        // Every child sets its value before any reads back, and each yield is a
        // switch to a sibling holding a different value for the same key.
        if !wait_until(|| ISO_ARRIVED.load(Ordering::Acquire) >= ISO_THREADS) {
            return 2;
        }
        for _ in 0..32 {
            sys_sched_yield();
            if ntls::get(key) != mine {
                return 3;
            }
        }
        if let Err(d) = fs_ok() {
            return 20 + d;
        }
        0
    }

    // ── check 3: FS.base across a cross-CPU migration ──────────────────────
    static MIG_KEY: AtomicUsize = AtomicUsize::new(0);
    static MIG_CPUS: AtomicU64 = AtomicU64::new(u64::MAX);

    extern "C" fn mig_child(_: usize) -> i32 {
        let key = MIG_KEY.load(Ordering::Acquire);
        let mine = 0x5150 as *mut u8;
        if !ntls::set(key, mine) {
            return 1;
        }
        let first = match fs_ok() {
            Ok(cpu) => cpu,
            Err(d) => return 10 + d,
        };
        // Park; the waker on another CPU wakes this thread onto ITS CPU (the
        // IPC handoff's migrating wake).
        if sys_test_tls_park() != 0 {
            return 2;
        }
        let second = match fs_ok() {
            Ok(cpu) => cpu,
            Err(d) => return 20 + d,
        };
        MIG_CPUS.store(first << 32 | second, Ordering::Release);
        if second == first {
            return 3; // not migrated: the check would be vacuous
        }
        if ntls::get(key) != mine {
            return 4;
        }
        0
    }

    extern "C" fn mig_waker(_: usize) -> ! {
        // Created on another CPU; wakes the parked child onto this one.
        let woke = wait_until(sys_test_tls_unpark);
        sys_thread_exit(if woke { 0 } else { 1 })
    }

    // ── check 8: a user selector load (Jarvis, C1.2 review) ────────────────
    /// Load the flat user data selector into FS. The CPU then takes FS.base
    /// from that descriptor: 0, behind the kernel's back.
    fn load_user_fs_selector() {
        // SAFETY: 0x23 is the user data selector (`gdt.rs` USER_DATA_SEL_RAW,
        // DPL 3): loading it into FS is legal in ring 3 and touches nothing but
        // this thread's FS.
        unsafe { core::arch::asm!("mov fs, {0:x}", in(reg) 0x23u16, options(nostack)) };
    }

    /// FS.base equals `base` everywhere: the kernel's field, the MSR and
    /// `fs:[0]`. The CPU, or a detail code 2..5.
    fn fs_is(base: u64) -> Result<u64, i32> {
        let [cpu, recorded, msr] = sys_test_tls_probe().map_err(|_| 2)?;
        if recorded != base {
            return Err(3);
        }
        if msr != base {
            return Err(4);
        }
        if ntls::current_tcb() as u64 != base {
            return Err(5);
        }
        Ok(cpu)
    }

    /// Loads tried before step 6 gives up. Every retry is a switch-in that
    /// landed between a load and its probe, so a few cover any timer rate.
    const SELECTOR_LOAD_TRIES: u32 = 16;

    fn selector_check() -> i32 {
        let mut block = alloc::boxed::Box::new([0u64; 8]);
        let base = block.as_ptr() as u64;
        block[0] = base;
        if sys_thread_set_tls(base).is_err() {
            return 1;
        }
        let cpu = match fs_is(base) {
            Ok(cpu) => cpu,
            Err(d) => return 10 + d,
        };
        // The load must really zero the hardware base, or the rest is vacuous.
        // A switch-in between the load and the probe restores FS.base (what
        // step 8 checks), so the probe then reads `base`: load again, a
        // bounded number of times (CP-83). Any other value fails at once.
        let mut zeroed = false;
        for _ in 0..SELECTOR_LOAD_TRIES {
            load_user_fs_selector();
            match sys_test_tls_probe() {
                Ok([_, _, 0]) => {
                    zeroed = true;
                    break;
                }
                Ok([_, _, msr]) if msr == base => {}
                _ => return 6,
            }
        }
        if !zeroed {
            return 6;
        }
        // The setter, with the value the kernel already recorded, loads it.
        if sys_thread_set_tls(base).is_err() {
            return 7;
        }
        if let Err(d) = fs_is(base) {
            return 20 + d;
        }
        // A switch out to the idle thread and back on this CPU restores it.
        load_user_fs_selector();
        if sys_test_tls_sleep() != 0 {
            return 8;
        }
        match fs_is(base) {
            Ok(c) if c == cpu => {}
            Ok(_) => return 9, // woke on another CPU: the check would be vacuous
            Err(d) => return 30 + d,
        }
        drop(block);
        0
    }

    extern "C" fn selector_child(_: usize) -> ! {
        sys_thread_exit(selector_check())
    }

    // ── check 4: iterated destructors, before the exit syscall ─────────────
    static REARM_KEY: AtomicUsize = AtomicUsize::new(0);
    static FOREVER_KEY: AtomicUsize = AtomicUsize::new(0);
    static DTOR_CHILD_TID: AtomicU64 = AtomicU64::new(0);
    static REARM_CALLS: AtomicU32 = AtomicU32::new(0);
    static REARM_ON_CHILD: AtomicU32 = AtomicU32::new(0);
    static REARM_SAW_VALUE: AtomicU32 = AtomicU32::new(0);
    static FOREVER_CALLS: AtomicU32 = AtomicU32::new(0);

    /// Like std's key-path cleanup: sets its key again on its first two calls,
    /// so it must run three times, in three passes.
    unsafe extern "C" fn rearm_dtor(value: *mut u8) {
        let key = REARM_KEY.load(Ordering::Acquire);
        if !ntls::get(key).is_null() {
            REARM_SAW_VALUE.fetch_add(1, Ordering::Relaxed); // slot not cleared first
        }
        if sys_thread_self() == DTOR_CHILD_TID.load(Ordering::Acquire) {
            REARM_ON_CHILD.fetch_add(1, Ordering::Relaxed);
        }
        if REARM_CALLS.fetch_add(1, Ordering::Relaxed) < 2 {
            ntls::set(key, value);
        }
    }

    /// Always sets its key again: the pass cap must stop it.
    unsafe extern "C" fn forever_dtor(value: *mut u8) {
        FOREVER_CALLS.fetch_add(1, Ordering::Relaxed);
        ntls::set(FOREVER_KEY.load(Ordering::Acquire), value);
    }

    extern "C" fn dtor_child(_: usize) -> i32 {
        DTOR_CHILD_TID.store(sys_thread_self(), Ordering::Release);
        let set_rearm = ntls::set(REARM_KEY.load(Ordering::Acquire), 0x77 as *mut u8);
        let set_forever = ntls::set(FOREVER_KEY.load(Ordering::Acquire), 0x88 as *mut u8);
        if set_rearm && set_forever { 0 } else { 1 }
    }

    // ── check 5: a deleted key's destructor never runs ──────────────────────
    static STALE_KEY: AtomicUsize = AtomicUsize::new(0);
    static STALE_STAGE: AtomicU32 = AtomicU32::new(0);
    static STALE_DTOR_CALLS: AtomicU32 = AtomicU32::new(0);

    unsafe extern "C" fn stale_dtor(_: *mut u8) {
        STALE_DTOR_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    extern "C" fn stale_child(_: usize) -> i32 {
        if !ntls::set(STALE_KEY.load(Ordering::Acquire), 0x99 as *mut u8) {
            return 1;
        }
        STALE_STAGE.store(1, Ordering::Release);
        // The main thread deletes the key now; this thread then exits with the
        // stale value still in its slot.
        if !wait_until(|| STALE_STAGE.load(Ordering::Acquire) == 2) {
            return 2;
        }
        0
    }

    // ── check 7: a thread that skipped the bootstrap has FS.base 0 ──────────
    static NB_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);

    extern "C" fn no_bootstrap_child(_: usize) -> ! {
        let r = match sys_test_tls_probe() {
            Ok([_, 0, 0]) => 0,
            Ok(_) => 1,
            Err(_) => 2,
        };
        NB_RESULT.store(r, Ordering::Release);
        sys_thread_exit(0)
    }

    pub fn run() -> i32 {
        say(b"[ threads-tls ] C1.2 TLS battery\n");

        // ── 1: the initial thread's block and FS.base ──
        if !nthread::init_main_thread_control_block() {
            say(b"[ threads-tls ] FAIL 1: init_main_thread_control_block\n");
            return 101;
        }
        let main_cpu = match fs_ok() {
            Ok(cpu) => cpu,
            Err(d) => {
                say(b"[ threads-tls ] FAIL 1: main thread FS.base\n");
                return 110 + d;
            }
        };
        // SYS_THREAD_SET_TLS on the running thread loads the MSR at once, not at
        // its next switch-in: point FS.base at a second block whose first word
        // is its own address and read `fs:[0]` with no syscall in between, then
        // put the real block back the same way.
        let tcb = own_tcb().unwrap_or(0);
        let mut alt = alloc::boxed::Box::new([0u64; 8]);
        let alt_addr = alt.as_ptr() as u64;
        alt[0] = alt_addr;
        if sys_thread_set_tls(alt_addr).is_err() {
            return 120;
        }
        let seen_alt = ntls::current_tcb() as u64;
        if sys_thread_set_tls(tcb).is_err() {
            return 121;
        }
        let seen_back = ntls::current_tcb() as u64;
        drop(alt);
        if seen_alt != alt_addr || seen_back != tcb {
            say(b"[ threads-tls ] FAIL 1: SYS_THREAD_SET_TLS did not take effect at once\n");
            return 122;
        }
        say(b"[ threads-tls ] 1 main thread: FS.base = its control block; set_tls takes effect at once ok\n");

        // ── 2: isolation + FS.base across switches ──
        let Some(key) = ntls::create(None) else {
            return 201;
        };
        ISO_KEY.store(key, Ordering::Release);
        let main_value = 0xA0 as *mut u8;
        if !ntls::set(key, main_value) {
            return 202;
        }
        let mut handles = [const { None }; ISO_THREADS as usize];
        for (i, h) in handles.iter_mut().enumerate() {
            match nthread::spawn(iso_child, i, 32 * 1024) {
                Ok(handle) => *h = Some(handle),
                Err(_) => return 203,
            }
        }
        for h in handles.iter_mut() {
            match h.take().map(|h| h.join()) {
                Some(Ok(0)) => {}
                Some(Ok(code)) => {
                    say(b"[ threads-tls ] FAIL 2: an isolation child\n");
                    return 2000 + code;
                }
                _ => return 204,
            }
        }
        if ntls::get(key) != main_value {
            say(b"[ threads-tls ] FAIL 2: main thread's value changed\n");
            return 205;
        }
        if fs_ok().is_err() {
            return 206;
        }
        say(b"[ threads-tls ] 2 isolation: 4 threads, distinct values across switches ok\n");

        // ── 3: migration ──
        let Some(mig_key) = ntls::create(None) else {
            return 301;
        };
        MIG_KEY.store(mig_key, Ordering::Release);
        let child = match nthread::spawn(mig_child, 0, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 302,
        };
        let waker_cpu = if main_cpu == 0 { 1 } else { 0 };
        let waker = match sys_test_thread_create_on(mig_waker, 0, 16 * 1024, waker_cpu) {
            Ok(tid) => tid,
            Err(_) => {
                say(b"[ threads-tls ] FAIL 3: no second CPU for the waker (needs -smp >= 2)\n");
                return 303;
            }
        };
        let child_code = child.join();
        let waker_code = sys_thread_join(waker);
        match (child_code, waker_code) {
            (Ok(0), Ok(0)) => {}
            (Ok(code), _) if code != 0 => {
                say(b"[ threads-tls ] FAIL 3: migration child\n");
                return 3000 + code;
            }
            _ => return 304,
        }
        let cpus = MIG_CPUS.load(Ordering::Acquire);
        let line = alloc::format!(
            "[ threads-tls ] 3 migration: parked on cpu {}, resumed on cpu {}, FS.base and key intact ok\n",
            cpus >> 32,
            cpus & 0xffff_ffff
        );
        say(line.as_bytes());

        // ── 8: a user selector load (C1.2 review) ──
        // On CPU 0, the timer CPU, so the tick sleep comes back on it.
        let sel = match sys_test_thread_create_on(selector_child, 0, 16 * 1024, 0) {
            Ok(tid) => tid,
            Err(_) => return 801,
        };
        match sys_thread_join(sel) {
            Ok(0) => {}
            Ok(code) => {
                say(b"[ threads-tls ] FAIL 8: FS.base after a user selector load\n");
                return 8000 + code;
            }
            Err(_) => return 802,
        }
        say(b"[ threads-tls ] 8 selector load: set_tls and a switch-in restore FS.base ok\n");

        // ── 4: iterated destructors ──
        let (Some(rearm), Some(forever)) = (
            ntls::create(Some(rearm_dtor)),
            ntls::create(Some(forever_dtor)),
        ) else {
            return 401;
        };
        REARM_KEY.store(rearm, Ordering::Release);
        FOREVER_KEY.store(forever, Ordering::Release);
        match nthread::spawn(dtor_child, 0, 32 * 1024).map(|h| h.join()) {
            Ok(Ok(0)) => {}
            _ => return 402,
        }
        if REARM_CALLS.load(Ordering::Acquire) != 3 {
            say(b"[ threads-tls ] FAIL 4: rearming destructor did not run 3 times\n");
            return 403;
        }
        if REARM_ON_CHILD.load(Ordering::Acquire) != 3 {
            say(b"[ threads-tls ] FAIL 4: destructor ran off the exiting thread\n");
            return 404;
        }
        if REARM_SAW_VALUE.load(Ordering::Acquire) != 0 {
            say(b"[ threads-tls ] FAIL 4: slot not cleared before the destructor\n");
            return 405;
        }
        if FOREVER_CALLS.load(Ordering::Acquire) != DESTRUCTOR_ITERATIONS as u32 {
            say(b"[ threads-tls ] FAIL 4: pass cap not 4\n");
            return 406;
        }
        say(b"[ threads-tls ] 4 destructors: rearm ran 3x on the exiting thread, cleared first, cap 4 ok\n");

        // ── 5: generation ──
        let Some(g) = ntls::create(None) else {
            return 501;
        };
        ntls::set(g, 0x55 as *mut u8);
        if !ntls::delete(g) || !ntls::get(g).is_null() {
            return 502;
        }
        let Some(g2) = ntls::create(None) else {
            return 503;
        };
        if g2 == g && !ntls::get(g2).is_null() {
            say(b"[ threads-tls ] FAIL 5: a deleted key's value showed through its reuse\n");
            return 504;
        }
        let Some(stale) = ntls::create(Some(stale_dtor)) else {
            return 505;
        };
        STALE_KEY.store(stale, Ordering::Release);
        let stale_child_h = match nthread::spawn(stale_child, 0, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 506,
        };
        if !wait_until(|| STALE_STAGE.load(Ordering::Acquire) == 1) {
            return 507;
        }
        if !ntls::delete(stale) {
            return 508;
        }
        STALE_STAGE.store(2, Ordering::Release);
        if stale_child_h.join() != Ok(0) {
            return 509;
        }
        if STALE_DTOR_CALLS.load(Ordering::Acquire) != 0 {
            say(b"[ threads-tls ] FAIL 5: a deleted key's destructor ran\n");
            return 510;
        }
        say(b"[ threads-tls ] 5 generation: deleted values invisible, their destructors never run ok\n");

        // ── 6: EPERM without the capability; EFAULT for a kernel address ──
        match sys_spawn("worker", &[]).map(|(_, h)| sys_wait(h, 0)) {
            Ok(Ok(41)) => {}
            _ => {
                say(b"[ threads-tls ] FAIL 6: no-cap child did not get EPERM\n");
                return 601;
            }
        }
        // A thread that touches its keys before any FS.base was set must fail
        // loudly: the with-cap child does that on a thread that skipped the
        // bootstrap, and the whole child must be fault-killed (139), never read
        // address 0 and carry on.
        match sys_spawn("worker", &[libneo::Handle(0)]).map(|(_, h)| sys_wait(h, 0)) {
            Ok(Ok(139)) => {}
            _ => {
                say(b"[ threads-tls ] FAIL 6: a key access before set_tls did not fault-kill\n");
                return 604;
            }
        }
        if sys_thread_set_tls(0xffff_8000_0000_0000) != Err(EFAULT) {
            return 602;
        }
        if fs_ok().is_err() {
            return 603; // the refused call changed FS.base
        }
        say(b"[ threads-tls ] 6 no-cap child EPERM, key access before set_tls fault-kills (139), kernel-half base EFAULT ok\n");

        // ── 7: a thread that skipped the bootstrap sees FS.base 0 ──
        let nb = match nthread::spawn_no_bootstrap(no_bootstrap_child, 0, 16 * 1024) {
            Ok(tid) => tid,
            Err(_) => return 701,
        };
        let _ = sys_thread_join(nb);
        if NB_RESULT.load(Ordering::Acquire) != 0 {
            say(b"[ threads-tls ] FAIL 7: a fresh thread inherited a FS.base\n");
            return 702;
        }
        say(b"[ threads-tls ] 7 no-bootstrap thread: FS.base 0 ok\n");

        say(b"[ threads-tls ] PASS\n");
        0
    }
}

/// OT-61 C1.1 gate: the in-process threads+futex concurrency battery.
/// Exercises the 0x44..0x4D syscalls + the userland futex sync layers
/// (`libneo::nsync`) and returns 0 iff every check passes; each failure has
/// a distinct code, so a non-zero exit localizes the first failing check.
#[cfg(feature = "threads-futex")]
fn run_selected_scenario() -> i32 {
    threads_futex_battery::run()
}

#[cfg(feature = "threads-futex")]
mod threads_futex_battery {
    use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    use libneo::nsync::{Condvar, Mutex, RwLock, deadline_after_ns};
    use libneo::nthread::{self, JoinHandle};
    use libneo::{
        CLOCK_MONOTONIC, FUTEX_DEADLINE_INFINITE, PROT_READ, PROT_WRITE, sys_clock_gettime,
        sys_futex_queue_len, sys_futex_wait, sys_futex_wake, sys_thread_get_info, sys_thread_self,
        sys_thread_set_name, sys_vm_protect, sys_write,
    };

    const EINVAL: i64 = -22;
    const ESRCH: i64 = -3;
    const EAGAIN: i64 = -11;
    const ETIMEDOUT: i64 = -110;

    /// Generous "must be woken well before this" deadline for waits that a
    /// sibling is contractually about to satisfy — TCG-timing tolerant; a
    /// timeout here means a LOST WAKEUP, the exact CP-14 failure the battery
    /// falsifies.
    const MUST_WAKE_NS: u64 = 8_000_000_000;

    fn say(m: &[u8]) {
        let _ = sys_write(1, m.as_ptr(), m.len());
    }

    fn now_ns() -> u64 {
        sys_clock_gettime(CLOCK_MONOTONIC).unwrap_or(0)
    }

    // ── check 1: create/join + arg delivery + join error surface ──────────
    extern "C" fn add_one(arg: usize) -> i32 {
        arg as i32 + 1
    }

    // ── check 2: get_info bounds (child side) ─────────────────────────────
    static CHILD_RSP_IN_BOUNDS: AtomicU32 = AtomicU32::new(0);

    extern "C" fn bounds_child(_arg: usize) -> i32 {
        let rsp: u64;
        // SAFETY: reading RSP has no side effects.
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
        match sys_thread_get_info(0) {
            Ok(info) => {
                let inside = rsp >= info.usable_low && rsp < info.usable_low + info.usable_len;
                let guard_below =
                    info.guard_len == 0x1000 && info.guard_low + info.guard_len == info.usable_low;
                CHILD_RSP_IN_BOUNDS.store(
                    u32::from(inside) | (u32::from(guard_below) << 1),
                    Ordering::Release,
                );
                0
            }
            Err(_) => 1,
        }
    }

    // ── check 3: futex-mutex contention ───────────────────────────────────
    static COUNTER_LOCK: Mutex = Mutex::new();
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    const CONTENTION_THREADS: usize = 4;
    const INCREMENTS: u64 = 5_000;

    extern "C" fn contender(_arg: usize) -> i32 {
        for _ in 0..INCREMENTS {
            COUNTER_LOCK.with(|| {
                // Deliberate load→store (NOT fetch_add): if the mutex ever
                // admits two threads, increments are lost and the sum check
                // fails — the mutex is load-bearing for this battery.
                let v = COUNTER.load(Ordering::Relaxed);
                COUNTER.store(v + 1, Ordering::Relaxed);
            });
        }
        0
    }

    // ── check 4: condvar ──────────────────────────────────────────────────
    static CV_LOCK: Mutex = Mutex::new();
    static CV: Condvar = Condvar::new();
    static CV_FLAG: AtomicU32 = AtomicU32::new(0);

    extern "C" fn cv_setter(_arg: usize) -> i32 {
        CV_LOCK.with(|| CV_FLAG.store(1, Ordering::Release));
        CV.notify_one();
        0
    }

    // ── check 5: lost-wakeup-free ping-pong ───────────────────────────────
    static PING: AtomicU32 = AtomicU32::new(0);
    static PONG: AtomicU32 = AtomicU32::new(0);
    const ROUNDS: u32 = 2_000;

    /// Wait until `word >= target`; returns false on any timeout (= a lost
    /// wakeup — the waker is contractually already committed to bumping it).
    fn wait_at_least(word: &AtomicU32, target: u32) -> bool {
        let deadline = deadline_after_ns(MUST_WAKE_NS);
        loop {
            let v = word.load(Ordering::Acquire);
            if v >= target {
                return true;
            }
            if matches!(
                sys_futex_wait(word.as_ptr() as *const u32, v, deadline),
                Err(ETIMEDOUT)
            ) {
                return word.load(Ordering::Acquire) >= target;
            }
        }
    }

    extern "C" fn ponger(_arg: usize) -> i32 {
        for i in 1..=ROUNDS {
            if !wait_at_least(&PING, i) {
                return 1; // lost wakeup on the ping side
            }
            PONG.store(i, Ordering::Release);
            let _ = sys_futex_wake(PONG.as_ptr() as *const u32, 1);
        }
        0
    }

    // ── check 7: wake-count preservation ──────────────────────────────────
    static WAKE_WORD: AtomicU32 = AtomicU32::new(0);
    static WAITERS_READY: AtomicU32 = AtomicU32::new(0);

    // C1.1 negative-control: separate futex word, never shared with check 7.
    static RACE_WORD: AtomicU32 = AtomicU32::new(0);
    // Holds negative-control waiters after their deliberately premature
    // readiness publication, making the missing queue entry observable.
    static RACE_RELEASE: AtomicU32 = AtomicU32::new(0);

    fn say_raw_u32(mut v: u32) {
        let mut buf = [0u8; 10];
        let mut i = buf.len();
        if v == 0 {
            i -= 1;
            buf[i] = b'0';
        } else {
            while v > 0 {
                i -= 1;
                buf[i] = b'0' + (v % 10) as u8;
                v /= 10;
            }
        }
        say(&buf[i..]);
    }

    extern "C" fn parked_waiter(_arg: usize) -> i32 {
        WAITERS_READY.fetch_add(1, Ordering::AcqRel);
        // One park, then exit on ANY wake; the main thread counts the wakes.
        let _ = sys_futex_wait(
            WAKE_WORD.as_ptr() as *const u32,
            0,
            deadline_after_ns(MUST_WAKE_NS),
        );
        0
    }

    /// Negative-control variant: parks on RACE_WORD (never WAKE_WORD).
    extern "C" fn race_parked_waiter(_arg: usize) -> i32 {
        WAITERS_READY.fetch_add(1, Ordering::AcqRel);
        while RACE_RELEASE.load(Ordering::Acquire) == 0 {
            libneo::sys_sched_yield();
        }
        let _ = sys_futex_wait(
            RACE_WORD.as_ptr() as *const u32,
            0,
            deadline_after_ns(MUST_WAKE_NS),
        );
        0
    }

    /// Negative-control: prove the wake-before-wait race reproduces without
    /// the queue-length barrier. Uses RACE_WORD (separate from check 7's
    /// WAKE_WORD) so it never touches check 7's futex key. It holds waiters
    /// after their pre-syscall readiness publication, making qlen==0 and two
    /// zero-pick wakes a deterministic proof of the bad barrier.
    fn race_control_check() -> i32 {
        WAITERS_READY.store(0, Ordering::Release);
        RACE_WORD.store(0, Ordering::Release);
        RACE_RELEASE.store(0, Ordering::Release);
        let mut code = 0i32;
        let mut handles = [const { None::<JoinHandle> }; 3];
        for slot in handles.iter_mut() {
            *slot = spawn_or(&mut code, race_parked_waiter, 0);
        }
        if code != 0 {
            for h in handles.into_iter().flatten() {
                let _ = h.join();
            }
            return 36;
        }
        // Spin on the deliberately pre-syscall counter. It proves only that
        // each waiter reached the bad publication point, never that it parked.
        let spin_deadline = now_ns() + MUST_WAKE_NS;
        while WAITERS_READY.load(Ordering::Acquire) < 3 && now_ns() < spin_deadline {
            libneo::sys_sched_yield();
        }
        if WAITERS_READY.load(Ordering::Acquire) < 3 {
            for h in handles.into_iter().flatten() {
                let _ = h.join();
            }
            return 37; // spin timeout — threads didn't signal readiness
        }
        // Held waiters cannot be present in RACE_WORD's queue.
        let qlen = sys_futex_queue_len(RACE_WORD.as_ptr() as *const u32);
        if qlen > 0 {
            // Race didn't manifest this run — waiters ARE enqueued.
            // Drain RACE_WORD so they exit promptly (not 8s timeout).
            RACE_WORD.store(1, Ordering::Release);
            RACE_RELEASE.store(1, Ordering::Release);
            for h in handles.into_iter().flatten() {
                let _ = h.join();
            }
            return 39;
        }
        // qlen == 0: the 3 waiters signaled "ready" but are NOT in the queue.
        // Zero picks from both finite wakes directly discriminate the old
        // barrier's lost-wakeup window.
        let first_picks = sys_futex_wake(RACE_WORD.as_ptr() as *const u32, 2).unwrap_or(u32::MAX);
        let second_picks = sys_futex_wake(RACE_WORD.as_ptr() as *const u32, 2).unwrap_or(u32::MAX);
        RACE_WORD.store(1, Ordering::Release);
        RACE_RELEASE.store(1, Ordering::Release);
        // The value change releases each held waiter via EAGAIN instead of a
        // timeout, keeping this deterministic control bounded.
        for h in handles.into_iter().flatten() {
            if h.join() != Ok(0) {
                say(b"[ threads-futex ] EXIT=40 site=check7b-race-join\n");
                return 40;
            }
        }
        if first_picks == 0 && second_picks == 0 {
            38 // race confirmed: ready count said 3 while both wakes picked 0
        } else {
            39 // held waiters were unexpectedly observed by a wake
        }
    }

    // ── check 8: rwlock torn-read falsifier ───────────────────────────────
    static RW: RwLock = RwLock::new();
    static RW_X: AtomicU64 = AtomicU64::new(0);
    static RW_Y: AtomicU64 = AtomicU64::new(0);
    const RW_WRITES: u64 = 300;

    extern "C" fn rw_writer(_arg: usize) -> i32 {
        for _ in 0..RW_WRITES {
            RW.write_lock();
            let v = RW_X.load(Ordering::Relaxed) + 1;
            RW_X.store(v, Ordering::Relaxed);
            // The X≠Y window a reader must never observe.
            core::hint::spin_loop();
            RW_Y.store(v, Ordering::Relaxed);
            RW.write_unlock();
        }
        0
    }

    extern "C" fn rw_reader(_arg: usize) -> i32 {
        loop {
            RW.read_lock();
            let x = RW_X.load(Ordering::Relaxed);
            let y = RW_Y.load(Ordering::Relaxed);
            RW.read_unlock();
            if x != y {
                return 1; // torn read: the rwlock admitted a reader mid-write
            }
            if x >= RW_WRITES {
                return 0;
            }
        }
    }

    fn spawn_or(code: &mut i32, f: extern "C" fn(usize) -> i32, arg: usize) -> Option<JoinHandle> {
        match nthread::spawn(f, arg, nthread::DEFAULT_STACK_SIZE) {
            Ok(h) => Some(h),
            Err(_) => {
                *code = 1;
                None
            }
        }
    }

    pub fn run() -> i32 {
        say(b"[ threads-futex ] C1.1 threads+futex battery\n");

        // C1.1 drain asserts: prove both futex queues are empty before check 7.
        // Any leftover waiter from a prior check would corrupt the wake-count
        // accounting. This turns a future leak into a loud failure.
        let pre_qlen = sys_futex_queue_len(WAKE_WORD.as_ptr() as *const u32);
        if pre_qlen != 0 {
            say(b"[ threads-futex ] FAIL: WAKE_WORD queue not empty before check 7\n");
            return 30;
        }
        let race_pre_qlen = sys_futex_queue_len(RACE_WORD.as_ptr() as *const u32);
        if race_pre_qlen != 0 {
            say(b"[ threads-futex ] FAIL: RACE_WORD queue not empty before check 7\n");
            return 30;
        }

        // 1. Lifecycle: create/join + arg in RDI + tid surface.
        let self_tid = sys_thread_self();
        let h = match nthread::spawn(add_one, 41, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 10,
        };
        let child_tid = h.tid();
        if child_tid == self_tid {
            return 11;
        }
        if h.join() != Ok(42) {
            say(b"[ threads-futex ] FAIL: join/arg\n");
            return 12;
        }
        if sys_thread_set_name("bat-main").is_err() {
            return 13;
        }
        say(b"[ threads-futex ] lifecycle: create/join + arg + set_name ok\n");

        // 2. Join error surface: self-join EINVAL, bogus tid ESRCH.
        if libneo::sys_thread_join(self_tid) != Err(EINVAL) {
            return 14;
        }
        if libneo::sys_thread_join(0xDEAD_BEEF) != Err(ESRCH) {
            return 15;
        }

        // 3. thread_get_info: the INITIAL thread (self) has sane bounds
        // containing the live RSP; a spawned child verifies its own.
        let rsp: u64;
        // SAFETY: reading RSP has no side effects.
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
        let info = match sys_thread_get_info(0) {
            Ok(i) => i,
            Err(_) => return 16,
        };
        if info.tid != self_tid || rsp < info.usable_low || rsp >= info.usable_low + info.usable_len
        {
            say(b"[ threads-futex ] FAIL: initial-thread bounds\n");
            return 17;
        }
        let hb = match nthread::spawn(bounds_child, 0, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 18,
        };
        let child_tid = hb.tid();
        // Parent-side view of the child's bounds (valid while it runs/exits).
        let child_info = match sys_thread_get_info(child_tid) {
            Ok(i) => i,
            Err(_) => return 19,
        };
        if child_info.usable_len != 32 * 1024 || child_info.guard_len != 0x1000 {
            return 20;
        }
        if hb.join() != Ok(0) || CHILD_RSP_IN_BOUNDS.load(Ordering::Acquire) != 0b11 {
            say(b"[ threads-futex ] FAIL: child bounds/guard view\n");
            return 21;
        }
        say(b"[ threads-futex ] get_info: initial + child bounds (guard below) ok\n");

        // 4. Futex-mutex contention: N threads × M increments, sum exact.
        let mut code = 0i32;
        let mut handles = [const { None::<JoinHandle> }; CONTENTION_THREADS];
        for slot in handles.iter_mut() {
            *slot = spawn_or(&mut code, contender, 0);
        }
        if code != 0 {
            return 22;
        }
        for h in handles.into_iter().flatten() {
            if h.join() != Ok(0) {
                return 23;
            }
        }
        if COUNTER.load(Ordering::Acquire) != CONTENTION_THREADS as u64 * INCREMENTS {
            say(b"[ threads-futex ] FAIL: contention sum (lost increments)\n");
            return 24;
        }
        say(b"[ threads-futex ] contention: futex-mutex sum exact ok\n");

        // 5. Condvar: predicate + notify arrives (deadline = lost-wakeup
        // detector, not a tuning knob).
        let hc = match nthread::spawn(cv_setter, 0, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 25,
        };
        CV_LOCK.lock();
        let mut woke = true;
        while CV_FLAG.load(Ordering::Acquire) == 0 && woke {
            woke = CV.wait_deadline(&CV_LOCK, deadline_after_ns(MUST_WAKE_NS));
        }
        let flag_set = CV_FLAG.load(Ordering::Acquire) == 1;
        CV_LOCK.unlock();
        if hc.join() != Ok(0) || !flag_set {
            say(b"[ threads-futex ] FAIL: condvar wait/notify\n");
            return 26;
        }
        say(b"[ threads-futex ] condvar: wait/notify under mutex ok\n");

        // 6. Lost-wakeup-free ping-pong: 2000 wake-races-wait rounds; any
        // timeout = a lost wakeup (CP-14 falsified).
        let hp = match nthread::spawn(ponger, 0, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 27,
        };
        for i in 1..=ROUNDS {
            PING.store(i, Ordering::Release);
            let _ = sys_futex_wake(PING.as_ptr() as *const u32, 1);
            if !wait_at_least(&PONG, i) {
                say(b"[ threads-futex ] FAIL: lost wakeup (pong side)\n");
                return 28;
            }
        }
        if hp.join() != Ok(0) {
            say(b"[ threads-futex ] FAIL: lost wakeup (ping side)\n");
            return 29;
        }
        say(b"[ threads-futex ] lost-wakeup: 2000 wake-races-wait rounds clean ok\n");

        // 7. Wake-count preservation: 3 parked waiters; wake(2)-sized calls
        // must return exactly how many they woke, never more than asked,
        // summing to exactly 3.
        //
        // CP-3x barrier: poll the kernel's real futex-queue length (ground
        // truth) until all 3 waiters are enqueued. WAITERS_READY is
        // incremented BEFORE sys_futex_wait and cannot prove parking.
        let mut whandles = [const { None::<JoinHandle> }; 3];
        for slot in whandles.iter_mut() {
            if let Ok(handle) = nthread::spawn(parked_waiter, 0, nthread::DEFAULT_STACK_SIZE) {
                *slot = Some(handle);
            }
        }
        if whandles.iter().any(Option::is_none) {
            say(b"[ threads-futex ] EXIT=35 site=check7-spawned-count\n");
            return 35;
        }
        let barrier_deadline = now_ns() + MUST_WAKE_NS;
        while sys_futex_queue_len(WAKE_WORD.as_ptr() as *const u32) < 3
            && now_ns() < barrier_deadline
        {
            libneo::sys_sched_yield();
        }
        if sys_futex_queue_len(WAKE_WORD.as_ptr() as *const u32) < 3 {
            say(b"[ threads-futex ] FAIL: barrier timeout (queue not full)\n");
            return 31;
        }
        // Give the ready-marked waiters time to actually park (bounded).
        let mut total: u32 = 0;
        let settle_deadline = now_ns() + MUST_WAKE_NS;
        while total < 3 && now_ns() < settle_deadline {
            match sys_futex_wake(WAKE_WORD.as_ptr() as *const u32, 2) {
                Ok(n) => {
                    if n > 2 {
                        return 32; // woke more than asked — count contract broken
                    }
                    total += n;
                }
                Err(_) => return 33,
            }
            if total < 3 {
                // Brief sleep so un-parked waiters reach the queue.
                let _ = sys_futex_wait(
                    WAKE_WORD.as_ptr() as *const u32,
                    1, // never matches (word is 0) → immediate EAGAIN, cheap yield
                    0,
                );
                libneo::sys_sched_yield();
            }
        }
        if total != 3 {
            say(b"[ threads-futex ] FAIL: wake-count sum\n");
            return 34;
        }
        let expected_joins = whandles.len();
        let mut joined = 0usize;
        for (idx, h) in whandles.into_iter().enumerate() {
            match h {
                Some(h) => match h.join() {
                    Ok(0) => joined += 1,
                    Ok(v) => {
                        say(b"[ threads-futex ] JOIN handle=");
                        say_raw_u32(idx as u32);
                        say(b" Ok(nonzero) val=");
                        say_raw_u32(v as u32);
                        say(b"\n");
                    }
                    Err(e) => {
                        say(b"[ threads-futex ] JOIN handle=");
                        say_raw_u32(idx as u32);
                        say(b" Err(errno=");
                        say_raw_u32(e as u32);
                        say(b")\n");
                    }
                },
                None => {
                    say(b"[ threads-futex ] JOIN handle=");
                    say_raw_u32(idx as u32);
                    say(b" missing\n");
                }
            }
        }
        if joined != expected_joins {
            say(b"[ threads-futex ] EXIT=35 site=check7-joined-count\n");
            return 35;
        }
        say(b"[ threads-futex ] wake-count: preserved (2+1=3, never >count) ok\n");

        // 7b. Negative-control: prove the wake-before-wait race reproduces
        // without the queue-length barrier. Uses the broken pre-syscall
        // WAITERS_READY counter (CP-3x), positively observes qlen==0 at
        // wake time, issues finite wakes, asserts all waiters reaped by
        // while held before sys_futex_wait. Exit 38 = race confirmed.
        let race_rc = race_control_check();
        if race_rc != 38 {
            return race_rc;
        }
        say(b"[ threads-futex ] negative-control: weak barrier race observed (zero wake picks) ok\n");

        // 8. RwLock: writer keeps X==Y outside the write section; readers
        // must never observe X!=Y (torn read ⇒ the wake-count fallback or
        // the state machine is broken).
        let hw = match nthread::spawn(rw_writer, 0, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 38,
        };
        let mut rhandles = [const { None::<JoinHandle> }; 3];
        for slot in rhandles.iter_mut() {
            *slot = spawn_or(&mut code, rw_reader, 0);
        }
        if code != 0 {
            return 39;
        }
        if hw.join() != Ok(0) {
            say(b"[ threads-futex ] EXIT=40 site=check8-rw-writer-join\n");
            return 40;
        }
        for h in rhandles.into_iter().flatten() {
            if h.join() != Ok(0) {
                say(b"[ threads-futex ] FAIL: rwlock torn read\n");
                return 41;
            }
        }
        if RW_X.load(Ordering::Acquire) != RW_WRITES {
            return 42;
        }
        say(b"[ threads-futex ] rwlock: read/write contention, no torn read ok\n");

        // 9. Timeout semantics: EAGAIN on value mismatch; ETIMEDOUT on the
        // absolute deadline (never early); deadline 0 = already expired.
        static WORD: AtomicU32 = AtomicU32::new(7);
        let wp = WORD.as_ptr() as *const u32;
        if sys_futex_wait(wp, 8, FUTEX_DEADLINE_INFINITE) != Err(EAGAIN) {
            return 43;
        }
        if sys_futex_wait(wp, 7, 0) != Err(ETIMEDOUT) {
            return 44;
        }
        let t0 = now_ns();
        let deadline = t0 + 30_000_000; // +30 ms, ABSOLUTE
        if sys_futex_wait(wp, 7, deadline) != Err(ETIMEDOUT) {
            return 46;
        }
        let t1 = now_ns();
        if t1 < deadline {
            say(b"[ threads-futex ] FAIL: woke before the absolute deadline\n");
            return 46;
        }
        say(b"[ threads-futex ] timeout: EAGAIN/expired-0/absolute-deadline (sleep) ok\n");

        // 10. Detach: a parked thread is detached → join refused (EINVAL),
        // then released; its reap is the refcount's business, not ours.
        static DETACH_WORD: AtomicU32 = AtomicU32::new(0);
        extern "C" fn detachee(_arg: usize) -> i32 {
            while DETACH_WORD.load(Ordering::Acquire) == 0 {
                let _ = sys_futex_wait(
                    DETACH_WORD.as_ptr() as *const u32,
                    0,
                    deadline_after_ns(MUST_WAKE_NS),
                );
            }
            0
        }
        let hd = match nthread::spawn(detachee, 0, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 47,
        };
        let dtid = hd.tid();
        if libneo::sys_thread_detach(dtid).is_err() {
            return 48;
        }
        if libneo::sys_thread_join(dtid) != Err(EINVAL) {
            return 49;
        }
        DETACH_WORD.store(1, Ordering::Release);
        let _ = sys_futex_wake(DETACH_WORD.as_ptr() as *const u32, 1);
        say(b"[ threads-futex ] detach: join-after-detach refused, released ok\n");

        // 11. (blocker E) BOOTSTRAP TCB: the TLS-free bootstrap must establish
        // the per-thread control block (bounds + errno seam) BEFORE the app
        // runs. The child asserts the block already exists on entry (magic +
        // RSP within bounds + an isolated errno cell); a NEGATIVE control
        // spawned without the bootstrap must observe no block.
        if run_bootstrap_check() != 0 {
            say(b"[ threads-futex ] FAIL: bootstrap TCB (bounds/errno seam)\n");
            return 50;
        }
        say(b"[ threads-futex ] bootstrap: TCB (bounds+errno) established before app; neg-control absent ok\n");

        // 12. (blocker F) DELAYED JOIN over the futex done-word: the target
        // sleeps so the joiner MUST park on the futex, exercising the wait/wake
        // path non-vacuously and surviving a wake-between-check-and-park race.
        if run_delayed_join_check() != 0 {
            say(b"[ threads-futex ] FAIL: delayed join (futex done-word)\n");
            return 51;
        }
        say(b"[ threads-futex ] join: delayed join parks on the futex done-word, exact code ok\n");

        // 13. (blocker B) STACK RECLAIM: run more than MAX_THREADS_PER_PROCESS
        // sequential spawn→join and spawn→detach cycles; each retired created
        // stack must become UNMAPPED (its base translates absent) while the
        // process is still alive — proving per-thread teardown frees the stack,
        // not just whole-process Pmap drop. A leak would keep the base mapped.
        if run_stack_reclaim_check() != 0 {
            say(b"[ threads-futex ] FAIL: created-stack reclaim (leak)\n");
            return 50;
        }
        say(b"[ threads-futex ] stack-reclaim: >MAX sequential cycles, every retired stack unmapped ok\n");

        say(b"[ threads-futex ] PASS: threads + futex + sync layers\n");
        0
    }

    // ── check 11 (blocker E): bootstrap control block established pre-app ──
    static BOOT_CHECK_RESULT: AtomicU32 = AtomicU32::new(0xFFFF_FFFF);
    static NEG_CHECK_RESULT: AtomicU32 = AtomicU32::new(0xFFFF_FFFF);

    extern "C" fn bootstrap_asserting_child(_arg: usize) -> i32 {
        // On entry, the bootstrap must ALREADY have installed the control block
        // (the test does NOT query it itself — that would hide the omission).
        let rsp: u64;
        // SAFETY: reading RSP is side-effect-free.
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
        let mut code = 0u32;
        match libneo::nthread::control_block() {
            Some(tcb) => {
                if rsp <= tcb.usable_low || rsp > tcb.usable_low + tcb.usable_len {
                    code |= 1; // live RSP not within the recorded usable bounds
                }
                // The errno cell must be an isolated per-thread datum living in
                // THIS thread's own stack block.
                let errno_addr = &tcb.errno as *const _ as u64;
                if errno_addr < tcb.usable_low || errno_addr >= tcb.usable_low + tcb.usable_len {
                    code |= 2;
                }
                tcb.errno.store(0x1234, Ordering::Release);
                if tcb.errno.load(Ordering::Acquire) != 0x1234 {
                    code |= 4;
                }
            }
            None => code |= 8, // bootstrap did NOT establish the block
        }
        BOOT_CHECK_RESULT.store(code, Ordering::Release);
        0
    }

    // Negative control: entered directly (no bootstrap) — must see NO block.
    extern "C" fn no_bootstrap_child(_arg: usize) -> ! {
        let observed_absent = libneo::nthread::control_block().is_none();
        NEG_CHECK_RESULT.store(u32::from(observed_absent), Ordering::Release);
        libneo::sys_thread_exit(0)
    }

    fn run_bootstrap_check() -> i32 {
        let h = match nthread::spawn(bootstrap_asserting_child, 0, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 1,
        };
        if h.join() != Ok(0) {
            return 2;
        }
        if BOOT_CHECK_RESULT.load(Ordering::Acquire) != 0 {
            return 3;
        }
        // Negative control: raw spawn, no bootstrap → control_block() is None.
        let ntid = match nthread::spawn_no_bootstrap(no_bootstrap_child, 0, 32 * 1024) {
            Ok(t) => t,
            Err(_) => return 4,
        };
        let _ = libneo::sys_thread_join(ntid);
        if NEG_CHECK_RESULT.load(Ordering::Acquire) != 1 {
            return 5; // the un-bootstrapped thread wrongly saw a block
        }
        0
    }

    // ── check 12 (blocker F): delayed join parks on the futex done-word ───
    static JOIN_GATE: AtomicU32 = AtomicU32::new(0);

    extern "C" fn delayed_exit_child(arg: usize) -> i32 {
        // Wait until the joiner has committed to parking (JOIN_GATE released),
        // so the join MUST go through the futex wait path (non-vacuous), then
        // exit with a distinctive code.
        while JOIN_GATE.load(Ordering::Acquire) == 0 {
            let _ = sys_futex_wait(
                JOIN_GATE.as_ptr() as *const u32,
                0,
                deadline_after_ns(MUST_WAKE_NS),
            );
        }
        arg as i32
    }

    fn run_delayed_join_check() -> i32 {
        let h = match nthread::spawn(delayed_exit_child, 0x2b, 32 * 1024) {
            Ok(h) => h,
            Err(_) => return 1,
        };
        // Release the child, then immediately join: the child is still blocked
        // (it must first observe the gate + return), so the join parks on the
        // done-word until the child exits — the wake-between-check-and-park
        // window is exercised by the concurrent release+wake here.
        JOIN_GATE.store(1, Ordering::Release);
        let _ = sys_futex_wake(JOIN_GATE.as_ptr() as *const u32, 1);
        match h.join() {
            Ok(0x2b) => 0,
            Ok(_) => 2,
            Err(_) => 3,
        }
    }

    // ── check 13 (blocker B): every retired created stack is unmapped ─────
    extern "C" fn tiny_child(arg: usize) -> i32 {
        arg as i32
    }

    /// Poll (yielding so the kernel reaper runs) until `base` translates
    /// absent — a `vm_protect` on an unmapped page is `EINVAL`. Returns true
    /// once unmapped within the bound.
    ///
    /// The probe MUST be non-destructive: a detached target may still be
    /// RUNNING on this very stack when the first poll lands. Probing with
    /// `PROT_READ` alone would strip write permission from a live stack and
    /// fault the target on its next push (a ring-3 #PF that fault-kills the
    /// whole process — observed exactly that way before this was fixed). So
    /// probe with the stack's OWN natural permissions (`PROT_READ|PROT_WRITE`):
    /// on a mapped stack it is an idempotent no-op, and on an unmapped one it
    /// still returns `EINVAL`. The probe therefore answers "is it mapped?"
    /// without perturbing a live thread — and stays falsifiable: a leaked
    /// stack keeps returning `Ok`, so this returns false and the check fails.
    fn wait_unmapped(base: u64) -> bool {
        // Bounded so a genuine leak reports a CLEAN failure ("returned 4 =
        // stack never freed") instead of silently blowing the scenario's
        // wall-clock budget — a timeout is a far worse diagnostic.
        //
        // Yields are batched BETWEEN probes on purpose. The probe's *success*
        // path (page still mapped) ends in a TLB flush inside the kernel, which
        // is expensive under TCG; probing once per yield made this check
        // dominate the whole battery's runtime. What the reaper actually needs
        // is scheduler turns, not probes — so spend the turns and probe rarely.
        for _ in 0..4_000u64 {
            if sys_vm_protect(base as *mut u8, 0x1000, PROT_READ | PROT_WRITE) == Err(EINVAL) {
                return true;
            }
            for _ in 0..64 {
                libneo::sys_sched_yield();
            }
        }
        false
    }

    /// One retire-and-reclaim cycle. `base` is the target's stack base; the
    /// caller has already retired it (join / detach). Returns true once that
    /// stack translates absent — i.e. the per-thread teardown really unmapped
    /// it, while the process is still ALIVE (the whole-`Pmap` drop cannot be
    /// what freed it, which is exactly the vacuity the AAG flagged).
    fn retired_stack_reclaimed(base: u64) -> bool {
        wait_unmapped(base)
    }

    fn run_stack_reclaim_check() -> i32 {
        // Warm the page-table overhead FIRST: the very first created stack can
        // add intermediate page-table levels that legitimately persist for the
        // life of the process. Warming means the steady-state cycles below are
        // compared against a warmed address space, not a cold one — so a
        // fixed one-time page-table cost is never mistaken for a leak.
        {
            let w = match nthread::spawn(tiny_child, 0, 16 * 1024) {
                Ok(h) => h,
                Err(_) => return 9,
            };
            let wbase = match sys_thread_get_info(w.tid()) {
                Ok(info) => info.usable_low,
                Err(_) => return 10,
            };
            if w.join() != Ok(0) {
                return 11;
            }
            if !retired_stack_reclaimed(wbase) {
                return 12;
            }
        }

        // Substantially more than MAX_THREADS_PER_PROCESS (16) sequential
        // cycles — each cycle retires THREE created stacks by a different
        // route, so 24 cycles retire 72 stacks through a 16-slot registry. If
        // created stacks leaked, the address space / frames would exhaust and
        // the per-cycle absent-check below would fail long before that.
        const CYCLES: u32 = 24;
        for i in 0..CYCLES {
            // (a) spawn→join: the joiner retires the target.
            let h = match nthread::spawn(tiny_child, i as usize, 16 * 1024) {
                Ok(h) => h,
                Err(_) => return 1,
            };
            let tid = h.tid();
            let base = match sys_thread_get_info(tid) {
                Ok(info) => info.usable_low,
                Err(_) => return 2,
            };
            if h.join() != Ok(i as i32) {
                return 3;
            }
            if !retired_stack_reclaimed(base) {
                return 4; // joined thread's stack never freed → leak
            }

            // (b) spawn→detach: nobody ever joins; the reaper alone retires it.
            let dh = match nthread::spawn(tiny_child, 0, 16 * 1024) {
                Ok(h) => h,
                Err(_) => return 5,
            };
            let dtid = dh.tid();
            let dbase = match sys_thread_get_info(dtid) {
                Ok(info) => info.usable_low,
                Err(_) => return 6,
            };
            if libneo::sys_thread_detach(dtid).is_err() {
                return 7;
            }
            if !retired_stack_reclaimed(dbase) {
                return 8; // detached thread's stack never freed → leak
            }

            // (c) ZOMBIE → LATER join/detach: let the target exit and be REAPED
            // before anyone collects it, then collect. Waiting for its stack to
            // go absent is precisely "the reap already ran", so the join below
            // is provably a zombie-record collection — the ordering the frozen
            // contract has to keep working (thread_get_info stays valid through
            // exit/reap, and the exit code survives in the zombie record).
            let zh = match nthread::spawn(tiny_child, 0x5a as usize, 16 * 1024) {
                Ok(h) => h,
                Err(_) => return 13,
            };
            let ztid = zh.tid();
            let zbase = match sys_thread_get_info(ztid) {
                Ok(info) => info.usable_low,
                Err(_) => return 14,
            };
            if !retired_stack_reclaimed(zbase) {
                return 15; // exited thread's stack never freed → leak
            }
            // The record is now a ZOMBIE (reaped, uncollected). Alternate the
            // collection route so both zombie paths are covered.
            if i % 2 == 0 {
                // Late join must still return the EXACT exit code.
                if zh.join() != Ok(0x5a) {
                    return 16;
                }
            } else {
                // Late detach must discard the zombie record cleanly.
                if libneo::sys_thread_detach(ztid).is_err() {
                    return 17;
                }
            }
        }

        // Aggregate: after 72 retired stacks the process is still healthy —
        // it can still create and join a thread. A registry or address-space
        // leak would surface here as EAGAIN/ENOMEM rather than a wrong value.
        let f = match nthread::spawn(tiny_child, 0x77, 16 * 1024) {
            Ok(h) => h,
            Err(_) => return 18,
        };
        let fbase = match sys_thread_get_info(f.tid()) {
            Ok(info) => info.usable_low,
            Err(_) => return 19,
        };
        if f.join() != Ok(0x77) {
            return 20;
        }
        if !retired_stack_reclaimed(fbase) {
            return 21;
        }
        0
    }
}

/// OT-61 C1.1 gate: cap-confinement (EPERM without the cap), per-process
/// thread refcount, and guard-page fault containment (I-14b) — via the
/// worker child, spawned twice. See `neo-worker`'s
/// `threads_guard_fault_worker` for the child-side script.
#[cfg(feature = "threads-guard-fault")]
fn run_selected_scenario() -> i32 {
    run_threads_guard_fault()
}

/// OT-61 C1.1 (blocker D): composed root-cap battery. Built with BOTH
/// `browser-runtime` AND `net-dial-cap`, so neo-init must receive BOTH root
/// caps (the bug installed only NetDial, leaving `thread_create` EPERM). Also
/// the combination-shape guard: a regression to mutually-exclusive cfg arrays
/// would either fail to compile this combined-feature build or drop a cap here.
#[cfg(feature = "threads-cap-compose")]
fn run_selected_scenario() -> i32 {
    use libneo::{Handle, NetDialCap};

    let say = |m: &[u8]| {
        let _ = sys_write(1, m.as_ptr(), m.len());
    };
    say(b"[ threads-cap-compose ] C1.1 composed root-cap battery\n");

    // 1. BrowserRuntime composed in: thread_create WORKS (the exact regression
    // — under the buggy mutually-exclusive arms this was EPERM).
    extern "C" fn compose_child(arg: usize) -> i32 {
        arg as i32
    }
    match libneo::nthread::spawn(compose_child, 7, 32 * 1024) {
        Ok(h) => {
            if h.join() != Ok(7) {
                return 1;
            }
        }
        Err(_) => {
            say(b"[ threads-cap-compose ] FAIL: thread_create EPERM under browser-runtime+net-dial-cap\n");
            return 2;
        }
    }
    say(b"[ threads-cap-compose ] browser-runtime present (thread_create+join ok)\n");

    // 2. NetDial ALSO composed in (not dropped): derive an attenuated cap from
    // the root NetDial at Handle(0). Pure capability op (no network) — succeeds
    // only if Handle(0) really is the NetDial root the compositional builder
    // installed alongside BrowserRuntime.
    match libneo::sys_net_dial_cap_derive(NetDialCap(Handle(0)), "example.com", 443) {
        Ok(_) => {}
        Err(_) => {
            say(b"[ threads-cap-compose ] FAIL: NetDial dropped by composition\n");
            return 3;
        }
    }
    say(b"[ threads-cap-compose ] netdial present (derive from Handle(0) ok)\n");

    // 3. Fail-closed / no self-mint: a child with NO delegated browser-runtime
    // cap must get EPERM from thread_create (the worker prober returns 41).
    let (_pid, w) = match sys_spawn("worker", &[]) {
        Ok(spawned) => spawned,
        Err(_) => return 4,
    };
    match libneo::sys_wait(w, 0) {
        Ok(41) => {}
        _ => {
            say(b"[ threads-cap-compose ] FAIL: no-cap child did not get thread_create EPERM\n");
            return 5;
        }
    }
    say(b"[ threads-cap-compose ] no-cap child thread_create EPERM (no self-mint) ok\n");

    say(b"[ threads-cap-compose ] PASS: browser-runtime + net-dial-cap compose; fail-closed child\n");
    0
}

#[cfg(feature = "threads-guard-fault")]
fn run_threads_guard_fault() -> i32 {
    use libneo::nthread;

    let say = |m: &[u8]| {
        let _ = sys_write(1, m.as_ptr(), m.len());
    };
    say(b"[ threads-guard-fault ] C1.1 cap/refcount/containment battery\n");

    // 1. NO-CAP child: thread authority must be fail-closed. The child
    // probes thread_create and exits 41 iff it got EPERM.
    let (_pid, h_nocap) = match sys_spawn("worker", &[]) {
        Ok(spawned) => spawned,
        Err(_) => return 1,
    };
    match libneo::sys_wait(h_nocap, 0) {
        Ok(41) => say(b"[ threads-guard-fault ] no-cap child: thread_create EPERM ok\n"),
        Ok(_) | Err(_) => {
            say(b"[ threads-guard-fault ] FAIL: no-cap child did not report EPERM\n");
            return 2;
        }
    }

    // 2. WITH-CAP child: inherit our browser-runtime root cap (Handle 0 —
    // the only kernel-installed root cap under this scenario). The child
    // joins 3 threads (refcount: intermediate reaps must not tear it down),
    // parks one thread forever, then overflows a guarded stack. The kernel
    // must fault-kill ONLY the child: sys_wait returns 139.
    let (_pid, h_cap) = match sys_spawn("worker", &[libneo::Handle(0)]) {
        Ok(spawned) => spawned,
        Err(_) => return 3,
    };
    match libneo::sys_wait(h_cap, 0) {
        Ok(139) => {
            say(b"[ threads-guard-fault ] with-cap child: guard overflow fault-killed (139) ok\n")
        }
        Ok(_) | Err(_) => {
            say(b"[ threads-guard-fault ] FAIL: child not fault-killed with 139\n");
            return 4;
        }
    }

    // 3. Containment: the fault killed only the child — this process still
    // creates, runs, and joins threads (scheduler + futex + registry live).
    extern "C" fn still_alive(arg: usize) -> i32 {
        arg as i32
    }
    match nthread::spawn(still_alive, 7, 32 * 1024) {
        Ok(h) => {
            if h.join() != Ok(7) {
                return 5;
            }
        }
        Err(_) => return 6,
    }
    say(b"[ threads-guard-fault ] containment: parent still schedules threads ok\n");

    say(b"[ threads-guard-fault ] PASS: EPERM no-cap + refcount + fault-kill 139 + containment\n");
    0
}

fn run_ipc_heap_kill_smoke() -> i32 {
    let _ = sys_write(1, HELLO.as_ptr(), HELLO.len());

    // Heap-built IPC payload (ADR-006 verification, retained from BUILD-5).
    let mut ping: Vec<u8> = Vec::with_capacity(64);
    ping.extend_from_slice(b"ping-from-heap-");
    for i in 0..16u8 {
        ping.push(b'a' + i);
    }

    // Two channels: chan_main carries ping/pong and the final blocked recv;
    // chan_block carries the synchronization message that pushes worker into
    // RecvBlocked.
    let (h_local_main, h_for_worker_main) = match sys_chan_create() {
        Ok(p) => p,
        Err(_) => {
            let _ = sys_write(1, CHAN_FAIL.as_ptr(), CHAN_FAIL.len());
            return 1;
        }
    };
    let (h_local_block, h_for_worker_block) = match sys_chan_create() {
        Ok(p) => p,
        Err(_) => {
            let _ = sys_write(1, CHAN_FAIL.as_ptr(), CHAN_FAIL.len());
            return 1;
        }
    };

    let _ = sys_write(1, SPAWNING.as_ptr(), SPAWNING.len());

    // Per ADR-004 §Q2 deterministic ordering: worker sees these as Handle(0)
    // and Handle(1) respectively.
    let (_pid, worker_handle) = match sys_spawn("worker", &[h_for_worker_main, h_for_worker_block])
    {
        Ok(p) => p,
        Err(_) => {
            let _ = sys_write(1, SPAWN_FAIL.as_ptr(), SPAWN_FAIL.len());
            return 1;
        }
    };
    let _ = sys_write(1, SPAWN_OK.as_ptr(), SPAWN_OK.len());

    // === Phase 1: happy-path ping/pong (BUILD-3 + BUILD-4 anti-regression) ===

    let n = sys_send(h_local_main, &ping);
    if n < 0 {
        let _ = sys_write(1, RECV_FAIL.as_ptr(), RECV_FAIL.len());
        return 2;
    }
    let _ = sys_write(1, SENT_PING.as_ptr(), SENT_PING.len());

    let mut recv_buf: Vec<u8> = alloc::vec![0u8; 64];
    let n = sys_recv(h_local_main, &mut recv_buf);
    if n < 0 {
        let _ = sys_write(1, RECV_FAIL.as_ptr(), RECV_FAIL.len());
        return 3;
    }
    let _ = sys_write(1, RECV_PONG.as_ptr(), RECV_PONG.len());
    let _ = sys_write(1, b"[ neo-init ] payload: ".as_ptr(), 22);
    let _ = sys_write(1, recv_buf.as_ptr(), n as usize);
    let _ = sys_write(1, b"\n".as_ptr(), 1);

    // === Phase 2: drive worker into deterministic RecvBlocked state ===
    // Single-CPU rendezvous: the worker transitions directly from receiving
    // this message into sys_recv(chan_main). There must be no worker-side
    // diagnostic syscall in between; a timer tick in that window would make
    // the blocked-kill assertion scheduler-timing-sensitive.
    let n = sys_send(h_local_block, b"go-block");
    if n < 0 {
        let _ = sys_write(1, SENT_GOBLOCK_FAIL.as_ptr(), SENT_GOBLOCK_FAIL.len());
        return 4;
    }
    let _ = sys_write(1, SENT_GOBLOCK.as_ptr(), SENT_GOBLOCK.len());

    // === Phase 3: SC-4 Q8.c — kill on Blocked succeeds via zombify ===
    let _ = sys_write(1, TRY_KILL_BLOCKED.as_ptr(), TRY_KILL_BLOCKED.len());
    let r = sys_kill(worker_handle);
    if r != 0 {
        let _ = sys_write(1, KILL_BLOCKED_WRONG.as_ptr(), KILL_BLOCKED_WRONG.len());
        return 5;
    }
    let _ = sys_write(1, KILL_BLOCKED_OK.as_ptr(), KILL_BLOCKED_OK.len());

    // === Phase 4: unhappy-path — double-kill must return ESRCH ===
    let _ = sys_write(1, TRY_KILL_STALE.as_ptr(), TRY_KILL_STALE.len());
    let r = sys_kill(worker_handle);
    if r != ESRCH {
        let _ = sys_write(1, KILL_STALE_WRONG.as_ptr(), KILL_STALE_WRONG.len());
        return 6;
    }
    let _ = sys_write(1, KILL_STALE_OK.as_ptr(), KILL_STALE_OK.len());

    0
}

#[cfg(feature = "atrium-surface-protocol")]
mod atrium_surface_protocol_tests {
    use libneo::atrium_protocol::{
        Commit, CreateSurface, DestroySurface, FocusRequest, Identity, ImportAttachBuffer,
        OP_COMMIT, OP_CREATE_SURFACE, OP_DESTROY_SURFACE, OP_FOCUS_REQUEST,
        OP_IMPORT_ATTACH_BUFFER, ProtocolError, REPLY_LEN, ROLE_TEXT, ROLE_WORKFIELD_ROOT, Reply,
        Request, decode_reply, encode_request,
    };
    use libneo::{
        ATRIUM_MAP_READ, ATRIUM_QUERY, BUFFER_DESC_V1_LEN, CapRights, FORMAT_XRGB8888_LE,
        FULL_CHANNEL_RIGHTS, Handle, encode_buffer_desc, meta_rights, sys_buffer_create,
        sys_chan_create, sys_channel_close, sys_grant, sys_recv, sys_recv_grant, sys_send,
        sys_spawn,
    };

    const NS: u64 = 0x4751_0000_0000_0001;
    const BUFFER_CAP_PROVENANCE: u64 = 0x4255_4646_0000_0001;

    fn send_request(channel: Handle, request_id: u64, request: Request) -> bool {
        let (bytes, len) = encode_request(request_id, request);
        sys_send(channel, &bytes[..len]) == len as i64
    }

    fn recv_byte(channel: Handle, expected: u8) -> bool {
        let mut byte = [0u8; 1];
        sys_recv(channel, &mut byte) == 1 && byte[0] == expected
    }

    fn recv_reply(channel: Handle, opcode: u16, request_id: u64) -> Result<Reply, ()> {
        let mut bytes = [0u8; REPLY_LEN];
        if sys_recv(channel, &mut bytes) != REPLY_LEN as i64 {
            return Err(());
        }
        let reply = decode_reply(&bytes).map_err(|_| ())?;
        if reply.opcode == opcode && reply.request_id == request_id {
            Ok(reply)
        } else {
            Err(())
        }
    }

    pub fn run() -> i32 {
        let (session_client, session_server) = match sys_chan_create() {
            Ok(pair) => pair,
            Err(_) => return 10,
        };
        if sys_spawn("worker", &[session_server]).is_err() {
            return 11;
        }

        let create = CreateSurface {
            identity: Identity {
                namespace_id: NS,
                surface_id: 0,
                surface_cap_provenance: 0,
            },
            role: ROLE_WORKFIELD_ROOT,
            width: 1920,
            height: 1080,
            client_tag: 0xA7,
        };
        if !send_request(session_client, 1, Request::Create(create)) {
            return 12;
        }
        let create_reply = match recv_reply(session_client, OP_CREATE_SURFACE, 1) {
            Ok(reply) if reply.status == 0 => reply,
            _ => return 12,
        };
        if !recv_byte(session_client, b'O') || sys_send(session_client, b"R") != 1 {
            return 12;
        }
        let surface = match sys_recv_grant(session_client, FULL_CHANNEL_RIGHTS) {
            Ok(handle) => handle,
            Err(_) => return 13,
        };
        if sys_send(session_client, b"A") != 1 {
            return 14;
        }

        // The one-full-screen-root invariant is exercised on the session
        // channel before any surface verbs. No SurfaceCap grant follows.
        if !send_request(session_client, 2, Request::Create(create)) {
            return 15;
        }
        match recv_reply(session_client, OP_CREATE_SURFACE, 2) {
            Ok(reply) if reply.status == ProtocolError::Limit.status() => {}
            _ => return 15,
        }

        let identity = create_reply.identity;
        let desc = encode_buffer_desc(64, 64, 256, FORMAT_XRGB8888_LE, 16_384);
        let buffer = match sys_buffer_create(&desc) {
            Ok(handle) => handle,
            Err(_) => return 16,
        };
        let import = ImportAttachBuffer {
            identity,
            transfer_id: 1,
            buffer_cap_provenance: BUFFER_CAP_PROVENANCE,
        };
        if !send_request(surface, 3, Request::ImportAttach(import)) || !recv_byte(surface, b'R') {
            return 17;
        }
        let read_query = CapRights::new(meta_rights::USE, ATRIUM_MAP_READ | ATRIUM_QUERY);
        if sys_grant(surface, buffer, read_query) != 0 {
            return 18;
        }
        if !matches!(recv_reply(surface, OP_IMPORT_ATTACH_BUFFER, 3), Ok(reply) if reply.status == 0)
        {
            return 18;
        }

        if !send_request(
            surface,
            4,
            Request::Commit(Commit {
                identity,
                client_serial: 1,
            }),
        ) {
            return 19;
        }
        if !matches!(recv_reply(surface, OP_COMMIT, 4), Ok(reply) if reply.status == 0) {
            return 19;
        }
        if !send_request(
            surface,
            5,
            Request::Focus(FocusRequest {
                identity,
                reason: libneo::atrium_protocol::FOCUS_REASON_USER_ACTIVATION,
                request_serial: 1,
            }),
        ) {
            return 20;
        }
        if !matches!(recv_reply(surface, OP_FOCUS_REQUEST, 5), Ok(reply) if reply.status == 0) {
            return 20;
        }
        if !send_request(
            surface,
            6,
            Request::Destroy(DestroySurface {
                identity,
                final_client_serial: 1,
            }),
        ) {
            return 21;
        }
        if !matches!(recv_reply(surface, OP_DESTROY_SURFACE, 6), Ok(reply) if reply.status == 0) {
            return 21;
        }

        // Destroy-by-drop: create a live TEXT surface, receive its dedicated
        // SurfaceCap, then close it without DESTROY. The compositor must see
        // EPIPE on its next recv, route that to peer_died, and reuse the slot.
        let text = CreateSurface {
            identity: Identity {
                namespace_id: NS,
                surface_id: 0,
                surface_cap_provenance: 0,
            },
            role: ROLE_TEXT,
            width: 320,
            height: 200,
            client_tag: 0xD0,
        };
        if !send_request(session_client, 7, Request::Create(text)) {
            return 23;
        }
        let dropped_reply = match recv_reply(session_client, OP_CREATE_SURFACE, 7) {
            Ok(reply) if reply.status == 0 => reply,
            _ => return 23,
        };
        if !recv_byte(session_client, b'O') || sys_send(session_client, b"R") != 1 {
            return 24;
        }
        let dropped_surface = match sys_recv_grant(session_client, FULL_CHANNEL_RIGHTS) {
            Ok(handle) => handle,
            Err(_) => return 25,
        };
        if sys_send(session_client, b"A") != 1 || sys_channel_close(dropped_surface) != 0 {
            return 26;
        }

        // This create is issued only after the server's blocking recv has
        // returned EPIPE and peer_died has made the prior slot reusable.
        if !send_request(session_client, 8, Request::Create(text)) {
            return 27;
        }
        let reused_reply = match recv_reply(session_client, OP_CREATE_SURFACE, 8) {
            Ok(reply)
                if reply.status == 0
                    && reply.identity.surface_id != dropped_reply.identity.surface_id =>
            {
                reply
            }
            _ => return 27,
        };
        if !recv_byte(session_client, b'O') || sys_send(session_client, b"R") != 1 {
            return 28;
        }
        let reused_surface = match sys_recv_grant(session_client, FULL_CHANNEL_RIGHTS) {
            Ok(handle) => handle,
            Err(_) => return 29,
        };
        if sys_send(session_client, b"A") != 1
            || !send_request(
                reused_surface,
                9,
                Request::Destroy(DestroySurface {
                    identity: reused_reply.identity,
                    final_client_serial: 0,
                }),
            )
            || !matches!(recv_reply(reused_surface, OP_DESTROY_SURFACE, 9), Ok(reply) if reply.status == 0)
        {
            return 30;
        }

        // Keep the descriptor length referenced in this no_std scenario: the
        // compositor peer queries exactly this immutable Rung-0 wire record.
        if desc.len() != BUFFER_DESC_V1_LEN {
            return 22;
        }
        0
    }
}

#[cfg(feature = "neo-shell-repl")]
mod neo_shell_repl_tests {
    use super::*;
    use libneo::sys_wait;

    const SPAWN_SHELL: &[u8] = b"[ neo-init ] spawning neo-shell\n";
    const SHELL_OK: &[u8] = b"[ neo-init ] neo-shell exited; init continues\n";
    const SHELL_SPAWN_FAIL: &[u8] = b"[ neo-init ] neo-shell spawn FAILED\n";
    const SHELL_WAIT_FAIL: &[u8] = b"[ neo-init ] neo-shell wait FAILED\n";
    const SHELL_EXIT_WRONG: &[u8] = b"[ neo-init ] neo-shell exit code mismatch\n";

    pub fn run() -> i32 {
        let _ = sys_write(1, SPAWN_SHELL.as_ptr(), SPAWN_SHELL.len());
        let (_pid, shell) = match libneo::sys_spawn_client("shell", shell_inherited_handles()) {
            Ok(spawned) => spawned,
            Err(_) => {
                let _ = sys_write(1, SHELL_SPAWN_FAIL.as_ptr(), SHELL_SPAWN_FAIL.len());
                return 1;
            }
        };

        match sys_wait(shell, 0) {
            Ok(0) => {
                let _ = sys_write(1, SHELL_OK.as_ptr(), SHELL_OK.len());
                0
            }
            Ok(_) => {
                let _ = sys_write(1, SHELL_EXIT_WRONG.as_ptr(), SHELL_EXIT_WRONG.len());
                2
            }
            Err(_) => {
                let _ = sys_write(1, SHELL_WAIT_FAIL.as_ptr(), SHELL_WAIT_FAIL.len());
                3
            }
        }
    }
}

#[cfg(feature = "neo-shell-builtins")]
mod neo_shell_builtins_tests {
    use super::*;
    use libneo::sys_wait;

    const SPAWN_SHELL: &[u8] = b"[ neo-init ] spawning neo-shell for fs builtins\n";
    const SHELL_OK: &[u8] = b"[ neo-init ] neo-shell builtins exited; init continues\n";
    const SHELL_SPAWN_FAIL: &[u8] = b"[ neo-init ] neo-shell builtins spawn FAILED\n";
    const SHELL_WAIT_FAIL: &[u8] = b"[ neo-init ] neo-shell builtins wait FAILED\n";
    const SHELL_EXIT_WRONG: &[u8] = b"[ neo-init ] neo-shell builtins exit code mismatch\n";

    pub fn run() -> i32 {
        let _ = sys_write(1, SPAWN_SHELL.as_ptr(), SPAWN_SHELL.len());
        let (_pid, shell) = match libneo::sys_spawn_client("shell", shell_inherited_handles()) {
            Ok(spawned) => spawned,
            Err(_) => {
                let _ = sys_write(1, SHELL_SPAWN_FAIL.as_ptr(), SHELL_SPAWN_FAIL.len());
                return 1;
            }
        };

        match sys_wait(shell, 0) {
            Ok(0) => {
                let _ = sys_write(1, SHELL_OK.as_ptr(), SHELL_OK.len());
                0
            }
            Ok(_) => {
                let _ = sys_write(1, SHELL_EXIT_WRONG.as_ptr(), SHELL_EXIT_WRONG.len());
                2
            }
            Err(_) => {
                let _ = sys_write(1, SHELL_WAIT_FAIL.as_ptr(), SHELL_WAIT_FAIL.len());
                3
            }
        }
    }
}

#[cfg(feature = "arthur-converse")]
mod arthur_converse_tests {
    use super::*;
    use libneo::sys_wait;

    const SPAWN_SHELL: &[u8] = b"[ neo-init ] spawning neo-shell for arthur\n";
    const SHELL_OK: &[u8] = b"[ neo-init ] arthur conversation shell exited; init continues\n";
    const SHELL_SPAWN_FAIL: &[u8] = b"[ neo-init ] arthur shell spawn FAILED\n";
    const SHELL_WAIT_FAIL: &[u8] = b"[ neo-init ] arthur shell wait FAILED\n";
    const SHELL_EXIT_WRONG: &[u8] = b"[ neo-init ] arthur shell exit code mismatch\n";

    pub fn run() -> i32 {
        let _ = sys_write(1, SPAWN_SHELL.as_ptr(), SPAWN_SHELL.len());
        let (_pid, shell) = match libneo::sys_spawn_client("shell", shell_inherited_handles()) {
            Ok(spawned) => spawned,
            Err(_) => {
                let _ = sys_write(1, SHELL_SPAWN_FAIL.as_ptr(), SHELL_SPAWN_FAIL.len());
                return 1;
            }
        };

        match sys_wait(shell, 0) {
            Ok(0) => {
                let _ = sys_write(1, SHELL_OK.as_ptr(), SHELL_OK.len());
                0
            }
            Ok(_) => {
                let _ = sys_write(1, SHELL_EXIT_WRONG.as_ptr(), SHELL_EXIT_WRONG.len());
                2
            }
            Err(_) => {
                let _ = sys_write(1, SHELL_WAIT_FAIL.as_ptr(), SHELL_WAIT_FAIL.len());
                3
            }
        }
    }
}

#[cfg(feature = "arthur-boots-thinking")]
mod arthur_boots_thinking_tests {
    use super::*;
    use libneo::sys_wait;

    const SPAWN_SHELL: &[u8] = b"[ neo-init ] spawning neo-shell for arthur boot thinking\n";
    const SHELL_OK: &[u8] = b"[ neo-init ] arthur boot thinking shell exited; init continues\n";
    const SHELL_SPAWN_FAIL: &[u8] = b"[ neo-init ] arthur boot thinking shell spawn FAILED\n";
    const SHELL_WAIT_FAIL: &[u8] = b"[ neo-init ] arthur boot thinking shell wait FAILED\n";
    const SHELL_EXIT_WRONG: &[u8] = b"[ neo-init ] arthur boot thinking shell exit code mismatch\n";
    const PROOF: &[u8] =
        b"NEO_TEST/1 event=INFO name=arthur_boots_thinking boot=ok default=/neofs/model-qwen2.5-1.5b.neom switch=/neofs/model-qwen2.5-0.5b.neom generate=before_after";

    pub fn run() -> i32 {
        let _ = sys_write(1, SPAWN_SHELL.as_ptr(), SPAWN_SHELL.len());
        let (_pid, shell) = match libneo::sys_spawn_client("shell", shell_inherited_handles()) {
            Ok(spawned) => spawned,
            Err(_) => {
                let _ = sys_write(1, SHELL_SPAWN_FAIL.as_ptr(), SHELL_SPAWN_FAIL.len());
                return 1;
            }
        };

        match sys_wait(shell, 0) {
            Ok(0) => {
                let _ = sys_write(1, SHELL_OK.as_ptr(), SHELL_OK.len());
                let _ = sys_write(1, PROOF.as_ptr(), PROOF.len());
                let _ = sys_write(1, b"\n".as_ptr(), 1);
                0
            }
            Ok(_) => {
                let _ = sys_write(1, SHELL_EXIT_WRONG.as_ptr(), SHELL_EXIT_WRONG.len());
                2
            }
            Err(_) => {
                let _ = sys_write(1, SHELL_WAIT_FAIL.as_ptr(), SHELL_WAIT_FAIL.len());
                3
            }
        }
    }
}

#[cfg(feature = "arthur-act")]
mod arthur_act_tests {
    use super::*;
    use libneo::sys_wait;

    const SPAWN_SHELL: &[u8] = b"[ neo-init ] spawning neo-shell for arthur act\n";
    const SHELL_OK: &[u8] = b"[ neo-init ] arthur action shell exited; init continues\n";
    const SHELL_SPAWN_FAIL: &[u8] = b"[ neo-init ] arthur action shell spawn FAILED\n";
    const SHELL_WAIT_FAIL: &[u8] = b"[ neo-init ] arthur action shell wait FAILED\n";
    const SHELL_EXIT_WRONG: &[u8] = b"[ neo-init ] arthur action shell exit code mismatch\n";

    pub fn run() -> i32 {
        let _ = sys_write(1, SPAWN_SHELL.as_ptr(), SPAWN_SHELL.len());
        let (_pid, shell) = match libneo::sys_spawn_client("shell", shell_inherited_handles()) {
            Ok(spawned) => spawned,
            Err(_) => {
                let _ = sys_write(1, SHELL_SPAWN_FAIL.as_ptr(), SHELL_SPAWN_FAIL.len());
                return 1;
            }
        };

        match sys_wait(shell, 0) {
            Ok(0) => {
                let _ = sys_write(1, SHELL_OK.as_ptr(), SHELL_OK.len());
                0
            }
            Ok(_) => {
                let _ = sys_write(1, SHELL_EXIT_WRONG.as_ptr(), SHELL_EXIT_WRONG.len());
                2
            }
            Err(_) => {
                let _ = sys_write(1, SHELL_WAIT_FAIL.as_ptr(), SHELL_WAIT_FAIL.len());
                3
            }
        }
    }
}

#[cfg(feature = "arthur-grants")]
mod arthur_grants_tests {
    use super::*;
    use libneo::{FileRights, OPEN_CREATE_REGULAR, sys_close, sys_fsync, sys_open, sys_wait};

    const GRANTS_PATH: &str = "/neofs/.arthur/grants.json";
    const SPAWN_SHELL: &[u8] = b"[ neo-init ] spawning neo-shell for arthur grants\n";
    const SHELL_OK: &[u8] = b"[ neo-init ] arthur grants shell exited; init continues\n";
    const SHELL_SPAWN_FAIL: &[u8] = b"[ neo-init ] arthur grants shell spawn FAILED\n";
    const SHELL_WAIT_FAIL: &[u8] = b"[ neo-init ] arthur grants shell wait FAILED\n";
    const SHELL_EXIT_WRONG: &[u8] = b"[ neo-init ] arthur grants shell exit code mismatch\n";
    const STORE_FAIL: &[u8] = b"[ neo-init ] arthur grants store write FAILED\n";
    const GRANTS_JSON: &[u8] = br#"{
  "version": 1,
  "grants": [
    { "scope": "FileRead", "resource": "/hello.txt", "rights": ["read"], "granted_by": "test" },
    { "scope": "FileRead", "resource": "/scratch", "rights": ["read"], "granted_by": "test" }
  ]
}"#;

    pub fn run() -> i32 {
        if write_grants_store() != 0 {
            let _ = sys_write(1, STORE_FAIL.as_ptr(), STORE_FAIL.len());
            return 10;
        }

        let _ = sys_write(1, SPAWN_SHELL.as_ptr(), SPAWN_SHELL.len());
        let (_pid, shell) = match libneo::sys_spawn_client("shell", shell_inherited_handles()) {
            Ok(spawned) => spawned,
            Err(_) => {
                let _ = sys_write(1, SHELL_SPAWN_FAIL.as_ptr(), SHELL_SPAWN_FAIL.len());
                return 1;
            }
        };

        match sys_wait(shell, 0) {
            Ok(0) => {
                let _ = sys_write(1, SHELL_OK.as_ptr(), SHELL_OK.len());
                0
            }
            Ok(_) => {
                let _ = sys_write(1, SHELL_EXIT_WRONG.as_ptr(), SHELL_EXIT_WRONG.len());
                2
            }
            Err(_) => {
                let _ = sys_write(1, SHELL_WAIT_FAIL.as_ptr(), SHELL_WAIT_FAIL.len());
                3
            }
        }
    }

    fn write_grants_store() -> i32 {
        let file = match sys_open(
            GRANTS_PATH,
            OPEN_CREATE_REGULAR,
            FileRights::READ.union(FileRights::WRITE),
        ) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        if sys_write(file.0, GRANTS_JSON.as_ptr(), GRANTS_JSON.len()) != GRANTS_JSON.len() as i64 {
            let _ = sys_close(file);
            return 2;
        }
        if sys_fsync(file) != 0 {
            let _ = sys_close(file);
            return 3;
        }
        let _ = sys_close(file);
        0
    }
}

#[cfg(feature = "arthur-console")]
mod arthur_console_tests {
    use super::*;
    use libneo::sys_wait;

    const SPAWN_SHELL: &[u8] = b"[ neo-init ] spawning neo-shell for arthur console\n";
    const SHELL_OK: &[u8] = b"[ neo-init ] arthur console shell exited; init continues\n";
    const SHELL_SPAWN_FAIL: &[u8] = b"[ neo-init ] arthur console shell spawn FAILED\n";
    const SHELL_WAIT_FAIL: &[u8] = b"[ neo-init ] arthur console shell wait FAILED\n";
    const SHELL_EXIT_WRONG: &[u8] = b"[ neo-init ] arthur console shell exit code mismatch\n";

    pub fn run() -> i32 {
        let _ = sys_write(1, SPAWN_SHELL.as_ptr(), SPAWN_SHELL.len());
        // OT-87 R4.4: the cap's negative legs, before the shell gets it.
        #[cfg(feature = "installer")]
        if let Err(code) = installer_negative_legs() {
            return code;
        }
        // OT-87 R4.4: the console installer needs the Installer cap the
        // kernel minted into this process; it is the shell's only way to it.
        #[cfg(feature = "installer")]
        let handles = {
            let mut handles = shell_inherited_handles().to_vec();
            handles.extend(libneo::install::installer_cap());
            handles
        };
        #[cfg(not(feature = "installer"))]
        let handles = shell_inherited_handles();
        let (_pid, shell) = match libneo::sys_spawn_client("shell", &handles) {
            Ok(spawned) => spawned,
            Err(_) => {
                let _ = sys_write(1, SHELL_SPAWN_FAIL.as_ptr(), SHELL_SPAWN_FAIL.len());
                return 1;
            }
        };

        match sys_wait(shell, 0) {
            Ok(0) => {
                let _ = sys_write(1, SHELL_OK.as_ptr(), SHELL_OK.len());
                0
            }
            Ok(_) => {
                let _ = sys_write(1, SHELL_EXIT_WRONG.as_ptr(), SHELL_EXIT_WRONG.len());
                2
            }
            Err(_) => {
                let _ = sys_write(1, SHELL_WAIT_FAIL.as_ptr(), SHELL_WAIT_FAIL.len());
                3
            }
        }
    }
}

/// OT-87 R4.4: a process spawned without the Installer cap gets EPERM from
/// both installer syscalls (the worker's `installer-probe`), and the cap
/// can't be handed to anything but the shell: a spawn of the worker with it
/// is refused.
#[cfg(all(feature = "arthur-console", feature = "installer"))]
fn installer_negative_legs() -> Result<(), i32> {
    const PROBE_OK: &[u8] = b"[ neo-init ] installer: no-cap probe exited 0\n";
    const PROBE_BAD: &[u8] = b"[ neo-init ] installer: no-cap probe FAILED\n";
    const SPAWN_REFUSED: &[u8] = b"[ neo-init ] installer: cap to worker refused EPERM\n";
    const SPAWN_BAD: &[u8] = b"[ neo-init ] installer: cap to worker NOT refused\n";
    const NO_CAP: &[u8] = b"[ neo-init ] installer: this process holds no Installer cap\n";
    let say = |m: &[u8]| {
        let _ = sys_write(1, m.as_ptr(), m.len());
    };
    match sys_spawn("worker", &[]).map(|(_, p)| libneo::sys_wait(p, 0)) {
        Ok(Ok(0)) => say(PROBE_OK),
        _ => {
            say(PROBE_BAD);
            return Err(4);
        }
    }
    let Some(cap) = libneo::install::installer_cap() else {
        say(NO_CAP);
        return Err(5);
    };
    match sys_spawn("worker", &[cap]) {
        Err(e) if e == libneo::EPERM => say(SPAWN_REFUSED),
        Err(_) | Ok(_) => {
            say(SPAWN_BAD);
            return Err(6);
        }
    }
    Ok(())
}

#[cfg(feature = "v1-console-liveness")]
mod v1_console_liveness_tests {
    use super::*;
    use libneo::sys_wait;

    const SPAWN_SHELL: &[u8] = b"[ neo-init ] spawning neo-shell for v1 console liveness\n";
    const SHELL_OK: &[u8] = b"[ neo-init ] v1 console liveness shell exited; init continues\n";
    const SHELL_SPAWN_FAIL: &[u8] = b"[ neo-init ] v1 console liveness shell spawn FAILED\n";
    const SHELL_WAIT_FAIL: &[u8] = b"[ neo-init ] v1 console liveness shell wait FAILED\n";
    const SHELL_EXIT_WRONG: &[u8] = b"[ neo-init ] v1 console liveness shell exit code mismatch\n";

    pub fn run() -> i32 {
        let _ = sys_write(1, SPAWN_SHELL.as_ptr(), SPAWN_SHELL.len());
        let (_pid, shell) = match libneo::sys_spawn_client("shell", shell_inherited_handles()) {
            Ok(spawned) => spawned,
            Err(_) => {
                let _ = sys_write(1, SHELL_SPAWN_FAIL.as_ptr(), SHELL_SPAWN_FAIL.len());
                return 1;
            }
        };

        match sys_wait(shell, 0) {
            Ok(0) => {
                let _ = sys_write(1, SHELL_OK.as_ptr(), SHELL_OK.len());
                0
            }
            Ok(_) => {
                let _ = sys_write(1, SHELL_EXIT_WRONG.as_ptr(), SHELL_EXIT_WRONG.len());
                2
            }
            Err(_) => {
                let _ = sys_write(1, SHELL_WAIT_FAIL.as_ptr(), SHELL_WAIT_FAIL.len());
                3
            }
        }
    }
}

#[cfg(all(feature = "arthur-memory", not(feature = "arthur-memory-encryption")))]
mod arthur_memory_tests {
    use libneo::{
        ENOENT, FileRights, Handle, OPEN_CREATE_REGULAR, OPEN_ROOT_TEMPORARY, sys_chan_create,
        sys_close, sys_fsync, sys_open, sys_recv, sys_send, sys_wait, sys_write,
    };

    const MEMORY_DIR: &str = "/neofs/.arthur/memory";
    const FACT_PATH: &str = "/neofs/.arthur/memory/fact.mem";
    const RELATIONSHIP_PATH: &str = "/neofs/.arthur/memory/relationship.mem";
    const REMEMBER_FACT: &[u8] = b"remember fact 1781433600 LogansOS M1 persists";
    const RECALL_FACT: &[u8] = b"recall fact";
    const RECALL_RELATIONSHIP: &[u8] = b"recall relationship";
    const EXIT: &[u8] = b"exit";
    const REMEMBERED: &[u8] = b"arthur: memory: remembered";
    const READBACK: &[u8] = b"arthur: memory: LogansOS M1 persists";
    const EMPTY: &[u8] = b"arthur: memory: empty";
    const OFF_ECHO: &[u8] = b"arthur: echo: remember fact 1781433600 LogansOS M1 persists";
    const OFF_RECALL_ECHO: &[u8] = b"arthur: echo: recall fact";
    const CORRUPT_RECORD: &[u8] = b"0:relationship:not-a-timestamp:SHOULD_NOT_LEAK\n";

    pub fn run() -> i32 {
        let first = match arthur_turn(REMEMBER_FACT) {
            Ok(reply) => reply,
            Err(code) => return code,
        };
        if bytes_eq(&first, REMEMBERED) {
            return run_memory_on_path();
        }
        if bytes_eq(&first, OFF_ECHO) {
            return run_memory_off_path();
        }
        write_line(b"[ neo-init ] arthur memory first reply mismatch");
        10
    }

    fn run_memory_on_path() -> i32 {
        if corrupt_relationship_file() != 0 {
            return 21;
        }

        let (readback, corrupt) = match arthur_two_turns(RECALL_FACT, RECALL_RELATIONSHIP) {
            Ok(replies) => replies,
            Err(code) => return code,
        };
        if !bytes_eq(&readback, READBACK) {
            write_line(b"[ neo-init ] arthur memory readback mismatch");
            return 20;
        }
        if contains(&corrupt, b"SHOULD_NOT_LEAK") {
            write_line(b"[ neo-init ] arthur memory corrupt content leaked");
            return 22;
        }
        if !bytes_eq(&corrupt, EMPTY) {
            write_line(b"[ neo-init ] arthur memory corrupt fallback mismatch");
            return 23;
        }

        write_line(
            b"NEO_TEST/1 event=INFO name=arthur_memory mode=on write=ok readback=ok corrupt=ephemeral",
        );
        0
    }

    fn run_memory_off_path() -> i32 {
        match sys_open(FACT_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Err(ENOENT) => {}
            Ok(file) => {
                let _ = sys_close(file);
                write_line(b"[ neo-init ] arthur memory off created fact file");
                return 30;
            }
            Err(_) => {
                write_line(b"[ neo-init ] arthur memory off fact probe failed");
                return 31;
            }
        }
        match arthur_turn(RECALL_FACT) {
            Ok(reply) if bytes_eq(&reply, OFF_RECALL_ECHO) => {}
            Ok(_) => {
                write_line(b"[ neo-init ] arthur memory off recall mismatch");
                return 32;
            }
            Err(code) => return code,
        }
        write_line(b"NEO_TEST/1 event=INFO name=arthur_memory mode=off no_persistence=true");
        0
    }

    fn arthur_turn(prompt: &[u8]) -> Result<[u8; 128], i32> {
        let mut replies = [[0u8; 128]; 1];
        arthur_session(&[prompt], &mut replies)?;
        Ok(replies[0])
    }

    fn arthur_two_turns(first: &[u8], second: &[u8]) -> Result<([u8; 128], [u8; 128]), i32> {
        let mut replies = [[0u8; 128]; 2];
        arthur_session(&[first, second], &mut replies)?;
        Ok((replies[0], replies[1]))
    }

    fn arthur_session(prompts: &[&[u8]], replies: &mut [[u8; 128]]) -> Result<(), i32> {
        if prompts.len() != replies.len() {
            return Err(39);
        }
        let (local, child) = sys_chan_create().map_err(|_| 40)?;
        let (_slot1_parent, slot1_child) = sys_chan_create().map_err(|_| 41)?;
        let (_slot2_parent, slot2_child) = sys_chan_create().map_err(|_| 42)?;
        let memory = sys_open(
            MEMORY_DIR,
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        )
        .map_err(|_| 43)?;
        let inherited = [child, slot1_child, slot2_child, Handle(memory.0)];
        let (_pid, process) = libneo::sys_spawn_client("arthur", &inherited).map_err(|_| 44)?;
        let _ = sys_close(memory);

        for idx in 0..prompts.len() {
            if sys_send(local, prompts[idx]) < 0 {
                return Err(45);
            }
            let mut reply = [0u8; 128];
            let n = sys_recv(local, &mut reply);
            if n < 0 {
                return Err(46);
            }
            replies[idx][..n as usize].copy_from_slice(&reply[..n as usize]);
        }
        if sys_send(local, EXIT) < 0 {
            return Err(47);
        }
        let mut bye = [0u8; 32];
        if sys_recv(local, &mut bye) < 0 {
            return Err(48);
        }
        match sys_wait(process, 0) {
            Ok(0) => {}
            Ok(_) => return Err(49),
            Err(_) => return Err(50),
        }
        Ok(())
    }

    fn corrupt_relationship_file() -> i32 {
        let file = match sys_open(
            RELATIONSHIP_PATH,
            OPEN_CREATE_REGULAR,
            FileRights::READ.union(FileRights::WRITE),
        ) {
            Ok(file) => file,
            Err(_) => return 60,
        };
        if sys_write(file.0, CORRUPT_RECORD.as_ptr(), CORRUPT_RECORD.len())
            != CORRUPT_RECORD.len() as i64
        {
            let _ = sys_close(file);
            return 61;
        }
        if sys_fsync(file) != 0 {
            let _ = sys_close(file);
            return 62;
        }
        let _ = sys_close(file);
        0
    }

    fn bytes_eq(reply: &[u8; 128], expected: &[u8]) -> bool {
        reply.starts_with(expected) && reply.get(expected.len()).copied().unwrap_or(0) == 0
    }

    fn contains(haystack: &[u8; 128], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    fn write_line(bytes: &[u8]) {
        let _ = sys_write(1, bytes.as_ptr(), bytes.len());
        let _ = sys_write(1, b"\n".as_ptr(), 1);
    }
}

/// M2 (encrypted) memory scenario — the FALSIFIABLE driver (ISC-5/B5).
///
/// The old M2 scenario reused the M1 driver (one record/category, no burn) so B1
/// (a 2nd-write bug), the `.key` growth, and crypto-erase were never exercised and
/// the harness banner was hardcoded. This driver returns a non-zero exit on any
/// failure, so the QEMU banner is earned. It asserts:
///   (a) two records of one category, then recall returns the LATEST — recall
///       re-reads the `.mem`, so this catches B1's second-blob `.mem` corruption;
///   (b) the on-disk `.key` is EXACTLY 48 bytes after two saves — a direct
///       falsifier for B1's `.key` growth (the bug appends 48 B/save -> 96 B);
///   (c) flip one ciphertext byte -> recall fails CLOSED (no plaintext leak);
///   (d) `forget` (crypto-erase) -> recall empty -> re-`remember` -> recall it.
///
/// SINGLE spawn by design. `spawn -> reap -> spawn` with a heavy first process (the
/// M2 arthur does crypto + multiple file writes) trips a kernel thread-slab UB
/// panic (`ptr::write` null/misaligned in `alloc_from_slab`, `sched/thread.rs`) —
/// a separate, pre-existing scheduler bug this scenario surfaced; the lighter M1
/// driver does not hit it. M2 correctness needs no second process: recall re-reads
/// the file each call (catches `.mem` corruption) and the `.key` growth is asserted
/// directly on disk. The tamper is applied while arthur idles on `sys_recv` between
/// turns (arthur closes the file between ops). See the RETURN for the kernel follow-on.
#[cfg(feature = "boot-neofs-userland-persistence")]
mod boot_neofs_userland_persistence_tests {
    use libneo::{
        ENOENT, FileRights, Handle, OPEN_ROOT_TEMPORARY, sys_chan_create, sys_close, sys_open,
        sys_recv, sys_send, sys_wait, sys_write,
    };

    const MEMORY_DIR: &str = "/neofs/.arthur/memory";
    const FACT_PATH: &str = "/neofs/.arthur/memory/fact.mem";
    const REMEMBER_FACT: &[u8] = b"remember fact 1781433605 Rung 5 Arthur remembers across reboot";
    const RECALL_FACT: &[u8] = b"recall fact";
    const EXIT: &[u8] = b"exit";
    const REMEMBERED: &[u8] = b"arthur: memory: remembered";
    const READBACK: &[u8] = b"arthur: memory: Rung 5 Arthur remembers across reboot";

    pub fn run() -> i32 {
        match sys_open(FACT_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Err(ENOENT) => write_boot(),
            Ok(file) => {
                let _ = sys_close(file);
                readback_boot()
            }
            Err(_) => {
                write_line(
                    b"NEO_TEST/1 event=INFO name=boot_neofs_userland_persistence stage=probe_failed",
                );
                10
            }
        }
    }

    fn write_boot() -> i32 {
        let (remembered, recalled) = match arthur_two_turns(REMEMBER_FACT, RECALL_FACT) {
            Ok(replies) => replies,
            Err(code) => return code,
        };
        if !bytes_eq(&remembered, REMEMBERED) {
            write_line(b"[ neo-init ] boot-neofs remember reply mismatch");
            return 20;
        }
        if !bytes_eq(&recalled, READBACK) {
            write_line(b"[ neo-init ] boot-neofs same-boot recall mismatch");
            return 21;
        }
        write_line(
            b"NEO_TEST/1 event=INFO name=boot_neofs_userland_persistence stage=write sys_open=OPEN_CREATE_REGULAR sys_write=ok fsync=ok via=arthur_memory",
        );
        0
    }

    fn readback_boot() -> i32 {
        let reply = match arthur_turn(RECALL_FACT) {
            Ok(reply) => reply,
            Err(code) => return code,
        };
        if !bytes_eq(&reply, READBACK) {
            write_line(b"[ neo-init ] boot-neofs reboot recall mismatch");
            return 30;
        }
        write_line(
            b"NEO_TEST/1 event=INFO name=boot_neofs_userland_persistence stage=readback sys_open=OPEN_ROOT_TEMPORARY sys_read=ok via=arthur_memory",
        );
        0
    }

    fn arthur_turn(prompt: &[u8]) -> Result<[u8; 128], i32> {
        let mut replies = [[0u8; 128]; 1];
        arthur_session(&[prompt], &mut replies)?;
        Ok(replies[0])
    }

    fn arthur_two_turns(first: &[u8], second: &[u8]) -> Result<([u8; 128], [u8; 128]), i32> {
        let mut replies = [[0u8; 128]; 2];
        arthur_session(&[first, second], &mut replies)?;
        Ok((replies[0], replies[1]))
    }

    fn arthur_session(prompts: &[&[u8]], replies: &mut [[u8; 128]]) -> Result<(), i32> {
        if prompts.len() != replies.len() {
            return Err(39);
        }
        let (local, child) = sys_chan_create().map_err(|_| 40)?;
        let (_slot1_parent, slot1_child) = sys_chan_create().map_err(|_| 41)?;
        let (_slot2_parent, slot2_child) = sys_chan_create().map_err(|_| 42)?;
        let memory = sys_open(
            MEMORY_DIR,
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        )
        .map_err(|_| 43)?;
        let inherited = [child, slot1_child, slot2_child, Handle(memory.0)];
        let (_pid, process) = libneo::sys_spawn_client("arthur", &inherited).map_err(|_| 44)?;
        let _ = sys_close(memory);

        for idx in 0..prompts.len() {
            if sys_send(local, prompts[idx]) < 0 {
                return Err(45);
            }
            let mut reply = [0u8; 128];
            let n = sys_recv(local, &mut reply);
            if n < 0 {
                return Err(46);
            }
            replies[idx][..n as usize].copy_from_slice(&reply[..n as usize]);
        }
        if sys_send(local, EXIT) < 0 {
            return Err(47);
        }
        let mut bye = [0u8; 32];
        if sys_recv(local, &mut bye) < 0 {
            return Err(48);
        }
        match sys_wait(process, 0) {
            Ok(0) => Ok(()),
            Ok(_) => Err(49),
            Err(_) => Err(50),
        }
    }

    fn bytes_eq(reply: &[u8; 128], expected: &[u8]) -> bool {
        reply.starts_with(expected) && reply.get(expected.len()).copied().unwrap_or(0) == 0
    }

    fn write_line(bytes: &[u8]) {
        let _ = sys_write(1, bytes.as_ptr(), bytes.len());
        let _ = sys_write(1, b"\n".as_ptr(), 1);
    }
}

#[cfg(feature = "arthur-memory-encryption")]
mod arthur_memory_encrypted_tests {
    use libneo::{
        FileRights, Handle, OPEN_ROOT_TEMPORARY, sys_chan_create, sys_close, sys_fsync, sys_open,
        sys_read, sys_recv, sys_send, sys_sysstat, sys_wait, sys_write,
    };

    const MEMORY_DIR: &str = "/neofs/.arthur/memory";
    const FACT_MEM: &str = "/neofs/.arthur/memory/fact.mem";
    const FACT_KEY: &str = "/neofs/.arthur/memory/fact.key";
    /// M2 `.key` is `key(32) || counter(16)`. The B1 bug appended 48 B/save.
    const KEY_FILE_LEN: usize = 48;

    const REMEMBER_FACT1: &[u8] = b"remember fact 1781433600 alphaone";
    const REMEMBER_FACT2: &[u8] = b"remember fact 1781433601 bravotwo";
    const REMEMBER_FACT3: &[u8] = b"remember fact 1781433602 charliethree";
    const RECALL_FACT: &[u8] = b"recall fact";
    const FORGET_FACT: &[u8] = b"forget fact";
    const EXIT: &[u8] = b"exit";

    const REMEMBERED: &[u8] = b"arthur: memory: remembered";
    const FORGOTTEN: &[u8] = b"arthur: memory: forgotten";
    const EMPTY: &[u8] = b"arthur: memory: empty";
    const READBACK2: &[u8] = b"arthur: memory: bravotwo";
    const READBACK3: &[u8] = b"arthur: memory: charliethree";

    pub fn run() -> i32 {
        ot43_step(1, b"spawn_setup_begin", 0);
        // Spawn ONE arthur process, sharing the neofs memory dir (single spawn —
        // see the module doc on the spawn-reap-spawn kernel panic).
        let (local, child) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 40,
        };
        let (_s1p, s1c) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 41,
        };
        let (_s2p, s2c) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 42,
        };
        let memory = match sys_open(
            MEMORY_DIR,
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        ) {
            Ok(m) => m,
            Err(_) => return 43,
        };
        let inherited = [child, s1c, s2c, Handle(memory.0)];
        ot43_step(2, b"spawn_call", 0);
        let (_pid, process) = match libneo::sys_spawn_client("arthur", &inherited) {
            Ok(v) => v,
            Err(_) => return 44,
        };
        ot43_step(3, b"spawn_returned", 0);
        let _ = sys_close(memory);
        ot43_step(4, b"drive_begin", 0);

        // Drive the falsifiable suite over the single conversation, then always
        // shut arthur down (EXIT + reap) regardless of the verdict.
        let mut reply = [0u8; 128];
        let code = drive(local, &mut reply);

        ot43_blocked(90, b"send_exit", 0);
        let _ = sys_send(local, EXIT);
        ot43_blocked(91, b"recv_exit_ack", 0);
        let mut bye = [0u8; 32];
        let _ = sys_recv(local, &mut bye);
        ot43_blocked(92, b"wait_reap", 0);
        let reaped = sys_wait(process, 0);
        ot43_step(93, b"wait_reap_returned", 0);

        if code != 0 {
            return code; // preserve the assertion failure
        }
        match reaped {
            Ok(0) => {
                write_line(
                    b"NEO_TEST/1 event=INFO name=arthur_memory_encrypted multi_record=ok key_no_growth=ok auth_fail=ok crypto_erase=ok",
                );
                0
            }
            Ok(_) => 49,
            Err(_) => 50,
        }
    }

    /// Run every assertion over the one conversation `local`. Returns 0 on success
    /// or a non-zero code naming the first failure. Does NOT shut arthur down —
    /// `run` does that unconditionally afterward.
    fn drive(local: Handle, reply: &mut [u8; 128]) -> i32 {
        // (a) Two records of one category.
        let mut write_attempts = 0u64;
        if turn(
            local,
            10,
            b"remember1",
            REMEMBER_FACT1,
            reply,
            &mut write_attempts,
            true,
        ) != 0
        {
            return 45;
        }
        if !bytes_eq(reply, REMEMBERED) {
            write_line(b"[ neo-init ] m2 remember#1 mismatch");
            return 70;
        }
        ot43_step(11, b"remember1_ok", write_attempts);
        if turn(
            local,
            12,
            b"remember2",
            REMEMBER_FACT2,
            reply,
            &mut write_attempts,
            true,
        ) != 0
        {
            return 45;
        }
        if !bytes_eq(reply, REMEMBERED) {
            write_line(b"[ neo-init ] m2 remember#2 mismatch");
            return 70;
        }
        ot43_step(13, b"remember2_ok", write_attempts);

        // (b) .key growth falsifier (B1): the on-disk key MUST be exactly 48 B.
        ot43_step(20, b"key_len_probe", write_attempts);
        match key_file_len() {
            Some(KEY_FILE_LEN) => {}
            Some(_) => {
                write_line(b"[ neo-init ] m2 .key grew past 48 B (B1)");
                return 71;
            }
            None => {
                write_line(b"[ neo-init ] m2 .key unreadable");
                return 72;
            }
        }
        ot43_step(21, b"key_len_ok", write_attempts);

        // (a, cont.) .mem corruption falsifier (B1): recall re-reads the file and
        // must return the LATEST record. The bug yields a 2-blob .mem -> empty.
        if turn(
            local,
            30,
            b"recall_after_two_writes",
            RECALL_FACT,
            reply,
            &mut write_attempts,
            false,
        ) != 0
        {
            return 45;
        }
        if !bytes_eq(reply, READBACK2) {
            write_line(b"[ neo-init ] m2 recall after 2 writes != latest (B1)");
            return 73;
        }
        ot43_step(31, b"recall_after_two_writes_ok", write_attempts);

        // (d) Crypto-erase -> recall empty -> re-remember -> recall returns it.
        if turn(
            local,
            40,
            b"forget_fact",
            FORGET_FACT,
            reply,
            &mut write_attempts,
            true,
        ) != 0
        {
            return 45;
        }
        if !bytes_eq(reply, FORGOTTEN) {
            write_line(b"[ neo-init ] m2 forget mismatch");
            return 74;
        }
        ot43_step(41, b"forget_fact_ok", write_attempts);
        if turn(
            local,
            42,
            b"recall_after_forget",
            RECALL_FACT,
            reply,
            &mut write_attempts,
            false,
        ) != 0
        {
            return 45;
        }
        if !bytes_eq(reply, EMPTY) {
            write_line(b"[ neo-init ] m2 recall-after-forget not empty");
            return 75;
        }
        ot43_step(43, b"recall_after_forget_ok", write_attempts);
        if turn(
            local,
            44,
            b"remember_after_forget",
            REMEMBER_FACT3,
            reply,
            &mut write_attempts,
            true,
        ) != 0
        {
            return 45;
        }
        if !bytes_eq(reply, REMEMBERED) {
            write_line(b"[ neo-init ] m2 re-remember after forget failed");
            return 76;
        }
        ot43_step(45, b"remember_after_forget_ok", write_attempts);
        if turn(
            local,
            46,
            b"recall_after_reremember",
            RECALL_FACT,
            reply,
            &mut write_attempts,
            false,
        ) != 0
        {
            return 45;
        }
        if !bytes_eq(reply, READBACK3) {
            write_line(b"[ neo-init ] m2 recall-after-re-remember mismatch");
            return 77;
        }
        ot43_step(47, b"recall_after_reremember_ok", write_attempts);

        // (c) Tamper one ciphertext byte while arthur idles on sys_recv, then a
        // fresh recall (re-reads the file) must fail CLOSED with no leak.
        ot43_step(50, b"tamper_begin", write_attempts);
        if tamper_fact_mem() != 0 {
            return 78;
        }
        write_attempts = write_attempts.saturating_add(1);
        ot43_step(51, b"tamper_write_ok", write_attempts);
        if turn(
            local,
            52,
            b"recall_after_tamper",
            RECALL_FACT,
            reply,
            &mut write_attempts,
            false,
        ) != 0
        {
            return 45;
        }
        if contains(reply, b"charliethree") {
            write_line(b"[ neo-init ] m2 tamper leaked plaintext");
            return 79;
        }
        if !bytes_eq(reply, EMPTY) {
            write_line(b"[ neo-init ] m2 tamper not fail-closed");
            return 80;
        }
        ot43_step(53, b"tamper_fail_closed_ok", write_attempts);

        0
    }

    /// One conversation turn: send `prompt`, receive arthur's reply into `reply`
    /// (zeroed first so the trailing bytes stay null for `bytes_eq`).
    fn turn(
        local: Handle,
        step: u64,
        label: &[u8],
        prompt: &[u8],
        reply: &mut [u8; 128],
        write_attempts: &mut u64,
        writes: bool,
    ) -> i32 {
        *reply = [0u8; 128];
        if writes {
            *write_attempts = (*write_attempts).saturating_add(1);
        }
        ot43_turn(step, b"send", label, *write_attempts);
        if sys_send(local, prompt) < 0 {
            return 46;
        }
        ot43_turn(step, b"recv_blocked", label, *write_attempts);
        if sys_recv(local, reply) < 0 {
            return 47;
        }
        ot43_turn(step, b"recv_done", label, *write_attempts);
        0
    }

    /// Length of the on-disk `fact.key`, or None if it can't be read.
    fn key_file_len() -> Option<usize> {
        let file = sys_open(FACT_KEY, OPEN_ROOT_TEMPORARY, FileRights::READ).ok()?;
        let mut total = 0usize;
        let mut scratch = [0u8; 64];
        loop {
            let n = sys_read(file, &mut scratch);
            if n < 0 {
                let _ = sys_close(file);
                return None;
            }
            if n == 0 {
                break;
            }
            total = total.saturating_add(n as usize);
            if total > 1024 {
                break; // sanity cap — a valid .key is 48 B
            }
        }
        let _ = sys_close(file);
        Some(total)
    }

    /// Read the current `fact.mem`, flip one byte in the AEAD region, write it
    /// back (same length). Any flipped byte (ciphertext or tag) breaks the tag.
    fn tamper_fact_mem() -> i32 {
        let mut buf = [0u8; 256];
        let mut len = 0usize;
        {
            ot43_step(50, b"tamper_read_open", 0);
            let file = match sys_open(FACT_MEM, OPEN_ROOT_TEMPORARY, FileRights::READ) {
                Ok(file) => file,
                Err(_) => return 80,
            };
            loop {
                if len >= buf.len() {
                    break;
                }
                let n = sys_read(file, &mut buf[len..]);
                if n < 0 {
                    let _ = sys_close(file);
                    return 81;
                }
                if n == 0 {
                    break;
                }
                len += n as usize;
            }
            let _ = sys_close(file);
        }
        // Need a real blob (version + nonce + tag is 29 bytes); flip a mid byte so
        // it lands in the ciphertext (small files) or the tag — both fail auth.
        if len < 30 {
            write_line(b"[ neo-init ] m2 tamper: blob too small");
            return 82;
        }
        ot43_step(50, b"tamper_read_done", 0);
        buf[len / 2] ^= 0xFF;

        ot43_step(50, b"tamper_write_open", 0);
        let file = match sys_open(
            FACT_MEM,
            OPEN_ROOT_TEMPORARY,
            FileRights::READ.union(FileRights::WRITE),
        ) {
            Ok(file) => file,
            Err(_) => return 83,
        };
        if sys_write(file.0, buf.as_ptr(), len) != len as i64 {
            let _ = sys_close(file);
            return 84;
        }
        ot43_step(50, b"tamper_fsync_blocked", 0);
        if sys_fsync(file) != 0 {
            let _ = sys_close(file);
            return 85;
        }
        let _ = sys_close(file);
        0
    }

    fn bytes_eq(reply: &[u8; 128], expected: &[u8]) -> bool {
        reply.starts_with(expected) && reply.get(expected.len()).copied().unwrap_or(0) == 0
    }

    fn contains(haystack: &[u8; 128], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    fn write_line(bytes: &[u8]) {
        let _ = sys_write(1, bytes.as_ptr(), bytes.len());
        let _ = sys_write(1, b"\n".as_ptr(), 1);
    }

    fn ot43_step(step: u64, label: &[u8], write_attempts: u64) {
        ot43_line(b"progress", step, b"label", label, write_attempts);
    }

    fn ot43_blocked(step: u64, label: &[u8], write_attempts: u64) {
        ot43_line(b"blocked", step, b"op", label, write_attempts);
    }

    fn ot43_turn(step: u64, event: &[u8], label: &[u8], write_attempts: u64) {
        ot43_line(event, step, b"turn", label, write_attempts);
    }

    fn ot43_line(event: &[u8], step: u64, key: &[u8], label: &[u8], write_attempts: u64) {
        let mut line = Ot43Line::new();
        line.push(b"[ ot43 ] src=neo-init scenario=arthur_memory_encrypted tick=");
        line.push_u64(uptime_ticks());
        line.push(b" event=");
        line.push(event);
        line.push(b" step=");
        line.push_u64(step);
        line.push(b" ");
        line.push(key);
        line.push(b"=");
        line.push(label);
        line.push(b" writes=");
        line.push_u64(write_attempts);
        line.emit();
    }

    fn uptime_ticks() -> u64 {
        sys_sysstat().map(|stats| stats.uptime_ticks).unwrap_or(0)
    }

    struct Ot43Line {
        buf: [u8; 192],
        len: usize,
    }

    impl Ot43Line {
        fn new() -> Self {
            Self {
                buf: [0u8; 192],
                len: 0,
            }
        }

        fn push(&mut self, bytes: &[u8]) {
            let remaining = self.buf.len().saturating_sub(self.len);
            let count = core::cmp::min(remaining, bytes.len());
            self.buf[self.len..self.len + count].copy_from_slice(&bytes[..count]);
            self.len += count;
        }

        fn push_u64(&mut self, mut value: u64) {
            let mut buf = [0u8; 20];
            let mut idx = buf.len();
            loop {
                idx -= 1;
                buf[idx] = b'0' + (value % 10) as u8;
                value /= 10;
                if value == 0 {
                    break;
                }
            }
            self.push(&buf[idx..]);
        }

        fn emit(&mut self) {
            self.push(b"\n");
            let _ = sys_write(1, self.buf.as_ptr(), self.len);
        }
    }
}

/// OT-17 M3 memory-cap delegation. Arthur owns the category file cap, derives an
/// attenuated child through I-15, grants a leaf cap over IPC, and revokes through
/// the existing derivation walk. The `.mem` rewrite after key-burn is exact-size:
/// fresh empty AEAD blob length, no stale ciphertext tail.
#[cfg(feature = "arthur-memory-delegation")]
mod arthur_memory_delegation_tests {
    use libneo::{
        CapRights, EACCES, EBADF, FileRights, Handle, OPEN_ROOT_TEMPORARY, meta_rights,
        sys_chan_create, sys_close, sys_derive, sys_open, sys_read, sys_recv, sys_recv_grant,
        sys_send, sys_wait, sys_write,
    };

    const MEMORY_DIR: &str = "/neofs/.arthur/memory";
    const FACT_MEM: &str = "/neofs/.arthur/memory/fact.mem";
    const FACT_KEY: &str = "/neofs/.arthur/memory/fact.key";
    const KEY_FILE_LEN: usize = 48;
    const MIN_EMPTY_AEAD_LEN: usize = 29;

    const REMEMBER_FACT1: &[u8] = b"remember fact 1781433600 alphaone";
    const REMEMBER_FACT2: &[u8] = b"remember fact 1781433601 bravotwo";
    const REMEMBER_FACT3: &[u8] = b"remember fact 1781433602 charliethree";
    const REMEMBER_RELATIONSHIP: &[u8] = b"remember relationship 1781433603 siblingcategory";
    const RECALL_FACT: &[u8] = b"recall fact";
    const RECALL_RELATIONSHIP: &[u8] = b"recall relationship";
    const DELEGATE_FACT_READ: &[u8] = b"delegate fact read";
    const REVOKE_FACT: &[u8] = b"revoke-memory fact";
    const FORGET_FACT: &[u8] = b"forget fact";
    const EXIT: &[u8] = b"exit";

    const REMEMBERED: &[u8] = b"arthur: memory: remembered";
    const DELEGATED_READ: &[u8] = b"arthur: memory: delegated fact read";
    const REVOKED: &[u8] = b"arthur: memory: revoked";
    const FORGOTTEN: &[u8] = b"arthur: memory: forgotten";
    const EMPTY: &[u8] = b"arthur: memory: empty";
    const READBACK2: &[u8] = b"arthur: memory: bravotwo";
    const READBACK3: &[u8] = b"arthur: memory: charliethree";
    const RELATIONSHIP_READBACK: &[u8] = b"arthur: memory: siblingcategory";

    pub fn run() -> i32 {
        let (local, child) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 40,
        };
        let (_s1p, s1c) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 41,
        };
        let (_s2p, s2c) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 42,
        };
        let memory = match sys_open(
            MEMORY_DIR,
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        ) {
            Ok(m) => m,
            Err(_) => return 43,
        };
        let memory_child = match sys_derive(
            Handle(memory.0),
            CapRights::new(
                meta_rights::USE,
                FileRights::LOOKUP.union(FileRights::READDIR).bits() as u8,
            ),
        ) {
            Ok(handle) => handle,
            Err(_) => return 45,
        };
        let inherited = [child, s1c, s2c, memory_child];
        let (_pid, process) = match libneo::sys_spawn_client("arthur", &inherited) {
            Ok(v) => v,
            Err(_) => return 44,
        };
        let _ = sys_close(memory);
        let _ = sys_close(libneo::FileHandle(memory_child.0));
        settle_after_spawn();

        let mut reply = [0u8; 128];
        let code = drive(local, &mut reply);

        let _ = sys_send(local, EXIT);
        let mut bye = [0u8; 32];
        let _ = sys_recv(local, &mut bye);
        let reaped = sys_wait(process, 0);

        if code != 0 {
            return code;
        }
        match reaped {
            Ok(0) => {
                write_line(
                    b"NEO_TEST/1 event=INFO name=arthur_memory_delegation delegate_ro=ok revoke=ok sibling=ok truncate_oracle=ok key_burn_revokes=ok no_new_invariant=ok cap_check_29_3=ok",
                );
                0
            }
            Ok(_) => 49,
            Err(_) => 50,
        }
    }

    fn drive(local: Handle, reply: &mut [u8; 128]) -> i32 {
        if turn_expect(local, REMEMBER_FACT1, REMEMBERED, reply, 70) != 0 {
            return 70;
        }
        if turn_expect(local, REMEMBER_FACT2, REMEMBERED, reply, 71) != 0 {
            return 71;
        }
        if turn_expect(local, REMEMBER_RELATIONSHIP, REMEMBERED, reply, 72) != 0 {
            return 72;
        }
        let old_len = match file_len(FACT_MEM) {
            Some(len) if len > MIN_EMPTY_AEAD_LEN => len,
            _ => {
                write_line(b"[ neo-init ] m3 old encrypted fact length not established");
                return 73;
            }
        };

        let ro = match delegate_read(local, reply) {
            Ok(handle) => handle,
            Err(code) => return code,
        };
        if check_delegated_read_only(ro) != 0 {
            return 81;
        }
        if turn_expect(local, REVOKE_FACT, REVOKED, reply, 82) != 0 {
            return 82;
        }
        if read_after_revoke(ro) != 0 {
            return 83;
        }
        if turn_expect(local, RECALL_FACT, READBACK2, reply, 84) != 0 {
            return 84;
        }
        if turn_expect(local, RECALL_RELATIONSHIP, RELATIONSHIP_READBACK, reply, 85) != 0 {
            return 85;
        }

        let burn_cap = match delegate_read(local, reply) {
            Ok(handle) => handle,
            Err(code) => return code,
        };
        if turn_expect(local, FORGET_FACT, FORGOTTEN, reply, 86) != 0 {
            return 86;
        }
        if read_after_revoke(burn_cap) != 0 {
            return 87;
        }
        if check_truncate_oracle(old_len) != 0 {
            return 88;
        }
        if turn_expect(local, RECALL_FACT, EMPTY, reply, 89) != 0 {
            return 89;
        }
        if turn_expect(local, RECALL_RELATIONSHIP, RELATIONSHIP_READBACK, reply, 90) != 0 {
            return 90;
        }
        if turn_expect(local, REMEMBER_FACT3, REMEMBERED, reply, 91) != 0 {
            return 91;
        }
        if turn_expect(local, RECALL_FACT, READBACK3, reply, 92) != 0 {
            return 92;
        }
        match file_len(FACT_KEY) {
            Some(KEY_FILE_LEN) => {}
            _ => {
                write_line(b"[ neo-init ] m3 key file length drifted");
                return 93;
            }
        }
        0
    }

    fn settle_after_spawn() {
        // M3 inherits a derived memory-dir cap; issue one real NeoFS read so
        // spawn/inheritance and storage startup settle before the first prompt.
        let mut scratch = [0u8; 64];
        let _ = read_file(FACT_MEM, &mut scratch);
    }

    fn delegate_read(local: Handle, reply: &mut [u8; 128]) -> Result<Handle, i32> {
        *reply = [0u8; 128];
        if sys_send(local, DELEGATE_FACT_READ) < 0 {
            return Err(74);
        }
        let handle = match sys_recv_grant(
            local,
            CapRights::new(meta_rights::USE, FileRights::READ.bits() as u8),
        ) {
            Ok(handle) => handle,
            Err(_) => return Err(75),
        };
        if sys_recv(local, reply) < 0 {
            return Err(76);
        }
        if !bytes_eq(reply, DELEGATED_READ) {
            write_line(b"[ neo-init ] m3 delegate reply mismatch");
            return Err(77);
        }
        Ok(handle)
    }

    fn check_delegated_read_only(handle: Handle) -> i32 {
        let mut buf = [0u8; 256];
        let n = sys_read(libneo::FileHandle(handle.0), &mut buf);
        if n <= 0 {
            write_line(b"[ neo-init ] m3 delegated read failed");
            return 1;
        }
        let n = n as usize;
        if contains(&buf[..n], b"bravotwo") || contains(&buf[..n], b"alphaone") {
            write_line(b"[ neo-init ] m3 delegated cap leaked plaintext");
            return 2;
        }
        if sys_write(handle.0, b"x".as_ptr(), 1) != EACCES {
            write_line(b"[ neo-init ] m3 read-only delegated cap allowed write");
            return 3;
        }
        0
    }

    fn read_after_revoke(handle: Handle) -> i32 {
        let mut one = [0u8; 1];
        if sys_read(libneo::FileHandle(handle.0), &mut one) != EBADF {
            write_line(b"[ neo-init ] m3 delegated cap survived revoke");
            return 1;
        }
        0
    }

    fn check_truncate_oracle(old_len: usize) -> i32 {
        let mut buf = [0u8; 64];
        let len = match read_file(FACT_MEM, &mut buf) {
            Some(len) => len,
            None => {
                write_line(b"[ neo-init ] m3 fact.mem unreadable after forget");
                return 1;
            }
        };
        if len != MIN_EMPTY_AEAD_LEN {
            write_line(b"[ neo-init ] m3 truncate oracle length mismatch");
            return 2;
        }
        if len >= old_len {
            write_line(b"[ neo-init ] m3 truncate oracle did not shrink");
            return 3;
        }
        if buf[0] != 1 {
            write_line(b"[ neo-init ] m3 truncate oracle bad encrypted version");
            return 4;
        }
        0
    }

    fn turn_expect(
        local: Handle,
        prompt: &[u8],
        expected: &[u8],
        reply: &mut [u8; 128],
        code: i32,
    ) -> i32 {
        if turn(local, prompt, reply) != 0 {
            return code;
        }
        if !bytes_eq(reply, expected) {
            write_line(b"[ neo-init ] m3 reply mismatch");
            return code;
        }
        0
    }

    fn turn(local: Handle, prompt: &[u8], reply: &mut [u8; 128]) -> i32 {
        *reply = [0u8; 128];
        if sys_send(local, prompt) < 0 {
            return 1;
        }
        if sys_recv(local, reply) < 0 {
            return 2;
        }
        0
    }

    fn file_len(path: &str) -> Option<usize> {
        let mut buf = [0u8; 512];
        read_file(path, &mut buf)
    }

    fn read_file(path: &str, buf: &mut [u8]) -> Option<usize> {
        let file = sys_open(path, OPEN_ROOT_TEMPORARY, FileRights::READ).ok()?;
        let mut total = 0usize;
        loop {
            if total >= buf.len() {
                break;
            }
            let n = sys_read(file, &mut buf[total..]);
            if n < 0 {
                let _ = sys_close(file);
                return None;
            }
            if n == 0 {
                break;
            }
            total = total.saturating_add(n as usize);
        }
        let _ = sys_close(file);
        Some(total)
    }

    fn bytes_eq(reply: &[u8; 128], expected: &[u8]) -> bool {
        reply.starts_with(expected) && reply.get(expected.len()).copied().unwrap_or(0) == 0
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    fn write_line(bytes: &[u8]) {
        let _ = sys_write(1, bytes.as_ptr(), bytes.len());
        let _ = sys_write(1, b"\n".as_ptr(), 1);
    }
}

/// OT-17 M4 forgetting. The QEMU oracle proves the userland ordering:
/// category forget revokes delegated caps, burns the category key, then leaves
/// the `.mem` object as a verified overwrite pattern. Total wipe burns every
/// category key before any category overwrite.
#[cfg(feature = "arthur-memory-forgetting")]
mod arthur_memory_forgetting_tests {
    use alloc::vec::Vec;
    use libneo::{
        CapRights, EACCES, EBADF, FileRights, Handle, OPEN_ROOT_TEMPORARY, meta_rights,
        sys_chan_create, sys_close, sys_derive, sys_open, sys_read, sys_recv, sys_recv_grant,
        sys_send, sys_wait, sys_write,
    };

    const MEMORY_DIR: &str = "/neofs/.arthur/memory";
    const FACT_MEM: &str = "/neofs/.arthur/memory/fact.mem";
    const PREFERENCE_MEM: &str = "/neofs/.arthur/memory/preference.mem";
    const EVENT_MEM: &str = "/neofs/.arthur/memory/event.mem";
    const RELATIONSHIP_MEM: &str = "/neofs/.arthur/memory/relationship.mem";
    const FACT_KEY: &str = "/neofs/.arthur/memory/fact.key";
    const PREFERENCE_KEY: &str = "/neofs/.arthur/memory/preference.key";
    const EVENT_KEY: &str = "/neofs/.arthur/memory/event.key";
    const RELATIONSHIP_KEY: &str = "/neofs/.arthur/memory/relationship.key";
    const KEY_FILE_LEN: usize = 48;
    const MIN_EMPTY_AEAD_LEN: usize = 29;
    const MAX_READ_FILE_BYTES: usize = 2048;
    const FORGET_OVERWRITE_BYTE: u8 = 0xa5;

    const REMEMBER_FACT4: &[u8] = b"remember fact 1781433605 deltafour";
    const RECALL_FACT: &[u8] = b"recall fact";
    const RECALL_RELATIONSHIP: &[u8] = b"recall relationship";
    const DELEGATE_FACT_READ: &[u8] = b"delegate fact read";
    const FORGET_FACT: &[u8] = b"forget fact";
    const FORGET_EVERYTHING: &[u8] = b"forget everything";
    const EXIT: &[u8] = b"exit";

    const REMEMBERED: &[u8] = b"arthur: memory: remembered";
    const DELEGATED_READ: &[u8] = b"arthur: memory: delegated fact read";
    const FORGOTTEN: &[u8] = b"arthur: memory: forgotten";
    const FORGOTTEN_EVERYTHING: &[u8] = b"arthur: memory: forgotten everything";
    const EMPTY: &[u8] = b"arthur: memory: empty";
    const READBACK4: &[u8] = b"arthur: memory: deltafour";

    pub fn run() -> i32 {
        let (local, child) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 40,
        };
        let (_s1p, s1c) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 41,
        };
        let (_s2p, s2c) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 42,
        };
        let memory = match sys_open(
            MEMORY_DIR,
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        ) {
            Ok(m) => m,
            Err(_) => return 43,
        };
        let memory_child = match sys_derive(
            Handle(memory.0),
            CapRights::new(
                meta_rights::USE,
                FileRights::LOOKUP.union(FileRights::READDIR).bits() as u8,
            ),
        ) {
            Ok(handle) => handle,
            Err(_) => return 45,
        };
        let inherited = [child, s1c, s2c, memory_child];
        let (_pid, process) = match libneo::sys_spawn_client("arthur", &inherited) {
            Ok(v) => v,
            Err(_) => return 44,
        };
        let _ = sys_close(memory);
        let _ = sys_close(libneo::FileHandle(memory_child.0));
        settle_after_spawn();

        let mut reply = [0u8; 128];
        let code = drive(local, &mut reply);

        let _ = sys_send(local, EXIT);
        let mut bye = [0u8; 32];
        let _ = sys_recv(local, &mut bye);
        let reaped = sys_wait(process, 0);

        if code != 0 {
            return code;
        }
        match reaped {
            Ok(0) => {
                write_line(
                    b"NEO_TEST/1 event=INFO name=arthur_memory_forgetting forget_endstate=ok wipe_endstate=ok cap_revoke=ok sibling_until_total=ok fresh_after_wipe=ok no_new_invariant=ok",
                );
                0
            }
            Ok(_) => 49,
            Err(_) => 50,
        }
    }

    fn drive(local: Handle, reply: &mut [u8; 128]) -> i32 {
        write_line(b"[ neo-init ] m4 step=drive_start");
        let old_fact = match read_file_vec(FACT_MEM) {
            Some(bytes) if bytes.len() > MIN_EMPTY_AEAD_LEN => bytes,
            _ => {
                write_line(b"[ neo-init ] m4 old encrypted fact length not established");
                return 73;
            }
        };
        let sibling_relationship = match read_file_vec(RELATIONSHIP_MEM) {
            Some(bytes) if bytes.len() > MIN_EMPTY_AEAD_LEN => bytes,
            _ => return 72,
        };
        write_line(b"[ neo-init ] m4 step=setup_records_ok");
        let old_fact_key = match read_file_vec(FACT_KEY) {
            Some(bytes) if bytes.len() == KEY_FILE_LEN => bytes,
            _ => {
                write_line(b"[ neo-init ] m4 old fact key length not established");
                return 74;
            }
        };
        write_line(b"[ neo-init ] m4 step=old_fact_captured");

        let burn_cap = match delegate_read(local, reply) {
            Ok(handle) => handle,
            Err(code) => return code,
        };
        if check_delegated_read_only(burn_cap) != 0 {
            return 80;
        }
        if turn_expect(local, FORGET_FACT, FORGOTTEN, reply, 81) != 0 {
            return 81;
        }
        write_line(b"[ neo-init ] m4 step=forget_fact_ok");
        if read_after_revoke(burn_cap) != 0 {
            return 82;
        }
        if check_overwrite_oracle(FACT_MEM, &old_fact) != 0 {
            return 83;
        }
        if check_key_changed(FACT_KEY, &old_fact_key) != 0 {
            return 84;
        }
        if turn_expect(local, RECALL_FACT, EMPTY, reply, 85) != 0 {
            return 85;
        }
        if check_file_unchanged(RELATIONSHIP_MEM, &sibling_relationship) != 0 {
            return 86;
        }
        write_line(b"[ neo-init ] m4 step=category_forget_oracle_ok");

        let total_fact = match read_file_vec(FACT_MEM) {
            Some(bytes) if bytes.len() > MIN_EMPTY_AEAD_LEN => bytes,
            _ => return 87,
        };
        let total_preference = match read_file_vec(PREFERENCE_MEM) {
            Some(bytes) if bytes.len() > MIN_EMPTY_AEAD_LEN => bytes,
            _ => return 88,
        };
        let total_event = match read_file_vec(EVENT_MEM) {
            Some(bytes) if bytes.len() > MIN_EMPTY_AEAD_LEN => bytes,
            _ => return 89,
        };
        let total_relationship = match read_file_vec(RELATIONSHIP_MEM) {
            Some(bytes) if bytes.len() > MIN_EMPTY_AEAD_LEN => bytes,
            _ => return 90,
        };
        let total_fact_key = match read_key_vec(FACT_KEY) {
            Some(bytes) => bytes,
            _ => return 91,
        };
        let total_preference_key = match read_key_vec(PREFERENCE_KEY) {
            Some(bytes) => bytes,
            _ => return 92,
        };
        let total_event_key = match read_key_vec(EVENT_KEY) {
            Some(bytes) => bytes,
            _ => return 93,
        };
        let total_relationship_key = match read_key_vec(RELATIONSHIP_KEY) {
            Some(bytes) => bytes,
            _ => return 94,
        };
        write_line(b"[ neo-init ] m4 step=total_wipe_preimages_captured");

        if turn_expect(local, FORGET_EVERYTHING, FORGOTTEN_EVERYTHING, reply, 95) != 0 {
            return 95;
        }
        write_line(b"[ neo-init ] m4 step=forget_everything_ok");
        if check_overwrite_pattern(FACT_MEM, total_fact.len()) != 0 {
            return 96;
        }
        if check_overwrite_oracle(PREFERENCE_MEM, &total_preference) != 0 {
            return 97;
        }
        if check_overwrite_oracle(EVENT_MEM, &total_event) != 0 {
            return 98;
        }
        if check_overwrite_oracle(RELATIONSHIP_MEM, &total_relationship) != 0 {
            return 99;
        }
        if check_key_changed(FACT_KEY, &total_fact_key) != 0 {
            return 100;
        }
        if check_key_changed(PREFERENCE_KEY, &total_preference_key) != 0 {
            return 101;
        }
        if check_key_changed(EVENT_KEY, &total_event_key) != 0 {
            return 102;
        }
        if check_key_changed(RELATIONSHIP_KEY, &total_relationship_key) != 0 {
            return 103;
        }
        if turn_expect(local, RECALL_RELATIONSHIP, EMPTY, reply, 104) != 0 {
            return 104;
        }
        write_line(b"[ neo-init ] m4 step=total_wipe_empty_ok");
        if turn_expect(local, REMEMBER_FACT4, REMEMBERED, reply, 105) != 0 {
            return 105;
        }
        if turn_expect(local, RECALL_FACT, READBACK4, reply, 106) != 0 {
            return 106;
        }
        0
    }

    fn settle_after_spawn() {
        let mut scratch = [0u8; 64];
        let _ = read_file_prefix(FACT_MEM, &mut scratch);
    }

    fn delegate_read(local: Handle, reply: &mut [u8; 128]) -> Result<Handle, i32> {
        *reply = [0u8; 128];
        if sys_send(local, DELEGATE_FACT_READ) < 0 {
            return Err(75);
        }
        let handle = match sys_recv_grant(
            local,
            CapRights::new(meta_rights::USE, FileRights::READ.bits() as u8),
        ) {
            Ok(handle) => handle,
            Err(_) => return Err(76),
        };
        if sys_recv(local, reply) < 0 {
            return Err(77);
        }
        if !bytes_eq(reply, DELEGATED_READ) {
            write_line(b"[ neo-init ] m4 delegate reply mismatch");
            return Err(78);
        }
        Ok(handle)
    }

    fn check_delegated_read_only(handle: Handle) -> i32 {
        let mut buf = [0u8; 256];
        let n = sys_read(libneo::FileHandle(handle.0), &mut buf);
        if n <= 0 {
            write_line(b"[ neo-init ] m4 delegated read failed");
            return 1;
        }
        let n = n as usize;
        if contains(&buf[..n], b"alphaone") {
            write_line(b"[ neo-init ] m4 delegated cap leaked plaintext");
            return 2;
        }
        if sys_write(handle.0, b"x".as_ptr(), 1) != EACCES {
            write_line(b"[ neo-init ] m4 read-only delegated cap allowed write");
            return 3;
        }
        0
    }

    fn read_after_revoke(handle: Handle) -> i32 {
        let mut one = [0u8; 1];
        if sys_read(libneo::FileHandle(handle.0), &mut one) != EBADF {
            write_line(b"[ neo-init ] m4 delegated cap survived forget");
            return 1;
        }
        0
    }

    fn check_key_changed(path: &str, old: &[u8]) -> i32 {
        let Some(new_bytes) = read_file_vec(path) else {
            write_line(b"[ neo-init ] m4 key unreadable after forget");
            return 1;
        };
        if new_bytes.len() != KEY_FILE_LEN {
            write_line(b"[ neo-init ] m4 key length drifted after forget");
            return 2;
        }
        if new_bytes.as_slice() == old {
            write_line(b"[ neo-init ] m4 key did not change after forget");
            return 3;
        }
        0
    }

    fn check_overwrite_oracle(path: &str, old: &[u8]) -> i32 {
        let Some(new_bytes) = read_file_vec(path) else {
            write_line(b"[ neo-init ] m4 overwritten mem unreadable");
            return 1;
        };
        if new_bytes.len() != old.len() {
            write_line(b"[ neo-init ] m4 overwrite length changed");
            return 2;
        }
        if new_bytes.as_slice() == old {
            write_line(b"[ neo-init ] m4 overwrite left prior ciphertext bytes");
            return 3;
        }
        if new_bytes.iter().any(|byte| *byte != FORGET_OVERWRITE_BYTE) {
            write_line(b"[ neo-init ] m4 overwrite pattern mismatch");
            return 4;
        }
        0
    }

    fn check_overwrite_pattern(path: &str, expected_len: usize) -> i32 {
        let Some(new_bytes) = read_file_vec(path) else {
            write_line(b"[ neo-init ] m4 overwritten mem unreadable");
            return 1;
        };
        if new_bytes.len() != expected_len {
            write_line(b"[ neo-init ] m4 overwrite length changed");
            return 2;
        }
        if new_bytes.iter().any(|byte| *byte != FORGET_OVERWRITE_BYTE) {
            write_line(b"[ neo-init ] m4 overwrite pattern mismatch");
            return 3;
        }
        0
    }

    fn check_file_unchanged(path: &str, old: &[u8]) -> i32 {
        let Some(new_bytes) = read_file_vec(path) else {
            write_line(b"[ neo-init ] m4 sibling mem unreadable");
            return 1;
        };
        if new_bytes.as_slice() != old {
            write_line(b"[ neo-init ] m4 sibling mem changed during category forget");
            return 2;
        }
        0
    }

    fn turn_expect(
        local: Handle,
        prompt: &[u8],
        expected: &[u8],
        reply: &mut [u8; 128],
        code: i32,
    ) -> i32 {
        if turn(local, prompt, reply) != 0 {
            return code;
        }
        if !bytes_eq(reply, expected) {
            write_line(b"[ neo-init ] m4 reply mismatch");
            return code;
        }
        0
    }

    fn turn(local: Handle, prompt: &[u8], reply: &mut [u8; 128]) -> i32 {
        *reply = [0u8; 128];
        write_line(b"[ neo-init ] m4 step=turn_send");
        if sys_send(local, prompt) < 0 {
            return 1;
        }
        write_line(b"[ neo-init ] m4 step=turn_send_done");
        if sys_recv(local, reply) < 0 {
            return 2;
        }
        write_line(b"[ neo-init ] m4 step=turn_recv");
        0
    }

    fn read_key_vec(path: &str) -> Option<Vec<u8>> {
        match read_file_vec(path) {
            Some(bytes) if bytes.len() == KEY_FILE_LEN => Some(bytes),
            _ => None,
        }
    }

    fn read_file_prefix(path: &str, out: &mut [u8]) -> Option<usize> {
        let file = sys_open(path, OPEN_ROOT_TEMPORARY, FileRights::READ).ok()?;
        let n = sys_read(file, out);
        let _ = sys_close(file);
        if n < 0 { None } else { Some(n as usize) }
    }

    fn read_file_vec(path: &str) -> Option<Vec<u8>> {
        let file = sys_open(path, OPEN_ROOT_TEMPORARY, FileRights::READ).ok()?;
        let mut out = Vec::new();
        let mut scratch = [0u8; 128];
        loop {
            let n = sys_read(file, &mut scratch);
            if n < 0 {
                let _ = sys_close(file);
                return None;
            }
            if n == 0 {
                break;
            }
            let n = n as usize;
            if out.len().saturating_add(n) > MAX_READ_FILE_BYTES {
                let _ = sys_close(file);
                return None;
            }
            out.extend_from_slice(&scratch[..n]);
        }
        let _ = sys_close(file);
        Some(out)
    }

    fn bytes_eq(reply: &[u8; 128], expected: &[u8]) -> bool {
        reply.starts_with(expected) && reply.get(expected.len()).copied().unwrap_or(0) == 0
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    fn write_line(bytes: &[u8]) {
        let _ = sys_write(1, bytes.as_ptr(), bytes.len());
        let _ = sys_write(1, b"\n".as_ptr(), 1);
    }
}

/// OT-29 regression — heavy spawn -> reap -> spawn. The faithful reproduce of the
/// SMP exit/reap thread-slab UAF that M2's heavy Arthur surfaced (QEMU run
/// 27926197328): a heavy first process (Arthur: ChaCha20 crypto + multiple file
/// writes — a hot, dirty FPU state that widens the `switch_context` save window)
/// EXITs, the BSP reaper frees its `Box<Thread>` while the origin AP is still in
/// the post-lock asm `mov [rdi], rsp`, and the next spawn allocates that
/// corrupted slab slot. With `slab-debug-poison` armed (kernel side), the UAF is
/// caught precisely at the corrupted alloc: a kernel stack-pointer word in a node
/// the slab believes is free.
///
/// This driver just loops `spawn heavy Arthur -> drive -> exit -> wait(reap)`
/// ROUNDS times. On a BUGGY kernel it panics (slab poison fires) — FAIL. On the
/// FIXED kernel (lock released on the new stack AFTER the rsp-write, so the
/// reaper can't free until the switch completes) every round is clean -> exit 0
/// -> PASS. The kernel `sched_spawn_reap_spawn_heavy` scenario gates on that
/// exit code, so the poison/UAF makes a FAIL reachable.
#[cfg(feature = "sched-spawn-reap-spawn-heavy")]
mod sched_spawn_reap_spawn_heavy_tests {
    use libneo::{
        FileRights, Handle, OPEN_ROOT_TEMPORARY, sys_chan_create, sys_close, sys_open, sys_recv,
        sys_send, sys_wait,
    };

    const MEMORY_DIR: &str = "/neofs/.arthur/memory";
    /// Heavy spawn->reap->spawn cycles. Each round is a fresh Arthur process; the
    /// next round's spawn allocates the prior round's reaped slab slot — the slot
    /// the SMP UAF corrupts. M2 reproduced 4/4 on a single cycle; the loop makes
    /// the regression robustly conclusive.
    const ROUNDS: usize = 16;

    // Heavy ops per round: each `remember` does a ChaCha20 encrypt + .mem/.key
    // write + fsync (dirties FPU, churns IPC + file I/O). recall re-reads.
    const REMEMBER1: &[u8] = b"remember fact 1781433600 alphaheavyworkload";
    const REMEMBER2: &[u8] = b"remember fact 1781433601 bravoheavyworkload";
    const RECALL: &[u8] = b"recall fact";
    const EXIT: &[u8] = b"exit";

    pub fn run() -> i32 {
        for round in 0..ROUNDS {
            let code = one_session();
            if code != 0 {
                write_line(b"[ neo-init ] ot29 round failed");
                return 60 + round as i32;
            }
        }
        write_line(
            b"NEO_TEST/1 event=INFO name=sched_spawn_reap_spawn_heavy heavy_spawn_reap_spawn=ok rounds=ok slab_clean=ok",
        );
        0
    }

    /// One spawn -> heavy work -> exit -> reap cycle on a fresh Arthur process.
    fn one_session() -> i32 {
        let (local, child) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 40,
        };
        let (_s1p, s1c) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 41,
        };
        let (_s2p, s2c) = match sys_chan_create() {
            Ok(v) => v,
            Err(_) => return 42,
        };
        let memory = match sys_open(
            MEMORY_DIR,
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        ) {
            Ok(m) => m,
            Err(_) => return 43,
        };
        let inherited = [child, s1c, s2c, Handle(memory.0)];
        let (_pid, process) = match libneo::sys_spawn_client("arthur", &inherited) {
            Ok(v) => v,
            Err(_) => return 44,
        };
        let _ = sys_close(memory);

        // Heavy turns (crypto + file writes), then shut Arthur down so it exits
        // and is reaped before the next round's spawn.
        let mut reply = [0u8; 128];
        for prompt in [REMEMBER1, REMEMBER2, RECALL] {
            if turn(local, prompt, &mut reply) != 0 {
                return 45;
            }
        }
        if sys_send(local, EXIT) < 0 {
            return 46;
        }
        let mut bye = [0u8; 32];
        if sys_recv(local, &mut bye) < 0 {
            return 47;
        }
        // sys_wait drives the reap of the exited Arthur thread (the UAF window).
        match sys_wait(process, 0) {
            Ok(0) => 0,
            Ok(_) => 48,
            Err(_) => 49,
        }
    }

    fn turn(local: Handle, prompt: &[u8], reply: &mut [u8; 128]) -> i32 {
        *reply = [0u8; 128];
        if sys_send(local, prompt) < 0 {
            return 50;
        }
        if sys_recv(local, reply) < 0 {
            return 51;
        }
        0
    }

    fn write_line(bytes: &[u8]) {
        let _ = libneo::sys_write(1, bytes.as_ptr(), bytes.len());
        let _ = libneo::sys_write(1, b"\n".as_ptr(), 1);
    }
}

#[cfg(feature = "arthur-action-surface")]
mod arthur_action_surface_tests {
    use libneo::action::{
        Action, ActionError, ActionKind, ActionOutcome, Grant, RequiredCapability, execute_action,
    };
    use libneo::{FileRights, sys_chan_create, sys_recv, sys_send, sys_wait, sys_write};

    const HELLO_PATH: &str = "/hello.txt";
    const SCRATCH_PATH: &str = "/scratch";
    const ROOT_PATH: &str = "/";
    const HELLO_BYTES: &[u8] = b"NeoPlutarch testfs hello\n";

    pub fn run() -> i32 {
        let file_grants = [
            Grant::File {
                path: HELLO_PATH,
                rights: FileRights::READ,
            },
            Grant::File {
                path: ROOT_PATH,
                rights: FileRights::LOOKUP.union(FileRights::READDIR),
            },
        ];

        let mut read_buf = [0u8; 64];
        let read_bytes = match execute_action(
            &file_grants,
            Action::ReadFile {
                path: HELLO_PATH,
                out: &mut read_buf,
            },
        ) {
            Ok(ActionOutcome::ReadFile { bytes }) => bytes,
            Ok(_) => return 1,
            Err(_) => return 2,
        };
        if read_bytes != HELLO_BYTES.len() || &read_buf[..read_bytes] != HELLO_BYTES {
            return 1;
        }

        match execute_action(
            &file_grants,
            Action::WriteFile {
                path: SCRATCH_PATH,
                data: b"denied",
            },
        ) {
            Err(ActionError::Denied {
                action: ActionKind::WriteFile,
                required:
                    RequiredCapability::File {
                        path: SCRATCH_PATH,
                        rights,
                    },
            }) if rights == FileRights::WRITE => {}
            _ => return 3,
        }

        let mut dir_buf = [0u8; 64];
        let mut cursor = 0u64;
        let list_bytes = match execute_action(
            &file_grants,
            Action::ListDir {
                path: ROOT_PATH,
                out: &mut dir_buf,
                cursor: &mut cursor,
            },
        ) {
            Ok(ActionOutcome::ListDir { bytes }) => bytes,
            Ok(_) => return 4,
            Err(_) => return 5,
        };
        if list_bytes == 0 || cursor == 0 {
            return 4;
        }

        let (parent, child) = match sys_chan_create() {
            Ok(handles) => handles,
            Err(_) => return 6,
        };
        let spawn_handles = [child];
        let spawn_grants = [
            Grant::SpawnProcess { name: "worker" },
            Grant::InheritHandle(child),
        ];
        let worker = match execute_action(
            &spawn_grants,
            Action::SpawnProcess {
                name: "worker",
                handles: &spawn_handles,
            },
        ) {
            Ok(ActionOutcome::SpawnProcess { handle, .. }) => handle,
            Ok(_) => return 7,
            Err(_) => return 8,
        };

        if sys_send(parent, b"arthur-action:ping") < 0 {
            return 9;
        }
        let mut pong = [0u8; 32];
        let n = sys_recv(parent, &mut pong);
        if n < 0 || &pong[..n as usize] != b"arthur-action:pong" {
            return 10;
        }
        match sys_wait(worker, 0) {
            Ok(0) => {
                let _ = sys_write(
                    1,
                    b"[ neo-init ] arthur action surface ok\n".as_ptr(),
                    b"[ neo-init ] arthur action surface ok\n".len(),
                );
                0
            }
            _ => 11,
        }
    }
}

#[cfg(any(
    feature = "cap-mint-and-use",
    feature = "cap-derive-subset",
    feature = "cap-derive-rights-violation",
    feature = "cap-derive-missing-meta",
    feature = "cap-grant-basic",
    feature = "cap-grant-missing-meta",
    feature = "cap-revoke-local",
    feature = "cap-graph-revoke",
    feature = "cap-revoke-cross-process",
    feature = "cap-revoke-descendants-walk",
    feature = "cap-revoke-spawn-inheritance-descendants",
    feature = "cap-survive-exit"
))]
mod cap_tests {
    use super::*;
    use libneo::{
        CapRights, EBADF, EPERM, EPIPE, FULL_CHANNEL_RIGHTS, channel_rights, meta_rights,
        sys_derive, sys_grant, sys_recv_grant, sys_revoke,
    };

    pub fn run() -> i32 {
        #[cfg(feature = "cap-mint-and-use")]
        {
            return cap_mint_and_use();
        }
        #[cfg(feature = "cap-derive-subset")]
        {
            return cap_derive_subset();
        }
        #[cfg(feature = "cap-derive-rights-violation")]
        {
            return cap_derive_rights_violation();
        }
        #[cfg(feature = "cap-derive-missing-meta")]
        {
            return cap_derive_missing_meta();
        }
        #[cfg(feature = "cap-grant-basic")]
        {
            return cap_grant_basic();
        }
        #[cfg(feature = "cap-grant-missing-meta")]
        {
            return cap_grant_missing_meta();
        }
        #[cfg(feature = "cap-revoke-local")]
        {
            return cap_revoke_local();
        }
        #[cfg(feature = "cap-revoke-cross-process")]
        {
            return cap_revoke_cross_process(false);
        }
        #[cfg(feature = "cap-revoke-descendants-walk")]
        {
            return cap_revoke_cross_process(true);
        }
        #[cfg(feature = "cap-revoke-spawn-inheritance-descendants")]
        {
            return cap_revoke_spawn_inheritance_descendants();
        }
        #[cfg(feature = "cap-survive-exit")]
        {
            return cap_survive_exit();
        }
        #[cfg(feature = "cap-graph-revoke")]
        {
            return cap_graph_revoke();
        }
    }

    /// K1a-0 (CP-87, CP-88): seven ways a cap used to outlive the revoke of its
    /// ancestor. The race legs place an install (derive, grant, spawn
    /// inheritance) between a revoke's walk and its removal; the launder legs
    /// close an intermediate cap, or end its holder (exit, kill) with a revoke
    /// placed inside the teardown. Survival is read with `sys_derive` (a table
    /// check: EBADF once revoked). Every leg runs and prints its verdict, so
    /// one control run shows all seven; the exit code is 0, or 10 + a bitmask
    /// of the legs whose cap survived.
    #[cfg(feature = "cap-graph-revoke")]
    fn cap_graph_revoke() -> i32 {
        use libneo::{
            sys_test_cap_graph_race_arm as arm, sys_test_cap_graph_race_result as result,
        };
        let say = |line: &[u8]| {
            sys_write(1, line.as_ptr(), line.len());
        };
        let probe = CapRights::new(meta_rights::USE | meta_rights::DERIVE, 0);
        let mid = CapRights::new(
            meta_rights::USE | meta_rights::DERIVE | meta_rights::GRANT,
            channel_rights::SEND,
        );
        let alive = |h| sys_derive(h, probe).is_ok();
        let mut leaked = 0;
        let verdict = |leaked_now: bool, ok: &[u8], bad: &[u8]| {
            say(if leaked_now { bad } else { ok });
            leaked_now
        };

        // Legs 1-3, the race: an install naming a doomed descendant lands
        // between the revoke's walk and its removal.
        for (bit, kind) in [(1, 0u32), (2, 1), (4, 2)] {
            let (root, _) = match sys_chan_create() {
                Ok(p) => p,
                Err(_) => return 1,
            };
            let d = match sys_derive(root, mid) {
                Ok(h) => h,
                Err(_) => return 2,
            };
            arm(d, kind);
            if sys_revoke(root) != 0 {
                return 3;
            }
            let r = result();
            if r == i64::MIN || r == -1000 {
                return 4; // the hook never ran, or ran with CAP_GRAPH free
            }
            let survived = match kind {
                // derive, grant: a handle index (alive?) or the errno.
                0 | 1 => r >= 0 && alive(libneo::Handle(r as u32)),
                // spawn: 1 = the child still holds the inherited cap.
                _ => r == 1,
            };
            if survived {
                leaked |= bit;
            }
            let (ok, bad): (&[u8], &[u8]) = match kind {
                0 => (
                    b"[ cap-graph ] leg=race-derive ok\n",
                    b"[ cap-graph ] leg=race-derive LEAKED (a child derived inside the revoke survived it)\n",
                ),
                1 => (
                    b"[ cap-graph ] leg=race-grant ok\n",
                    b"[ cap-graph ] leg=race-grant LEAKED (a grant completed inside the revoke survived it)\n",
                ),
                _ => (
                    b"[ cap-graph ] leg=race-spawn ok\n",
                    b"[ cap-graph ] leg=race-spawn LEAKED (a child spawned inside the revoke kept the cap)\n",
                ),
            };
            verdict(survived, ok, bad);
        }

        // Leg 4, launder by close: D' from the root, D'' from D', close D'.
        let (root, _) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 6,
        };
        let d1 = match sys_derive(root, mid) {
            Ok(h) => h,
            Err(_) => return 7,
        };
        let d2 = match sys_derive(d1, mid) {
            Ok(h) => h,
            Err(_) => return 8,
        };
        if libneo::sys_channel_close(d1) != 0 {
            return 9;
        }
        if sys_revoke(root) != 0 {
            return 10;
        }
        if verdict(
            alive(d2),
            b"[ cap-graph ] leg=close ok\n",
            b"[ cap-graph ] leg=close LEAKED (the grandchild outlived revoke(root))\n",
        ) {
            leaked |= 8;
        }

        // Legs 5 and 6, launder by exit and by kill: the worker derives from
        // its inherited cap and grants the child back here. Then it exits
        // when told (leg 5) or is killed while it waits (leg 6), and the
        // revoke of the root runs inside its teardown (the hook).
        for (bit, kill) in [(16, false), (32, true)] {
            let (ctl_local, ctl_worker) = match sys_chan_create() {
                Ok(p) => p,
                Err(_) => return 11,
            };
            let (root, _) = match sys_chan_create() {
                Ok(p) => p,
                Err(_) => return 12,
            };
            let d = match sys_derive(root, mid) {
                Ok(h) => h,
                Err(_) => return 13,
            };
            let (_pid, worker) = match sys_spawn("worker", &[ctl_worker, d]) {
                Ok(p) => p,
                Err(_) => return 14,
            };
            let granted = match sys_recv_grant(ctl_local, FULL_CHANNEL_RIGHTS) {
                Ok(h) => h,
                Err(_) => return 15,
            };
            arm(root, 3);
            if kill {
                if sys_kill(worker) != 0 {
                    return 16;
                }
            } else if sys_send(ctl_local, b"exit") < 0 || libneo::sys_wait(worker, 0) != Ok(0) {
                return 17;
            }
            match result() {
                0 => {}
                r if r == i64::MIN || r == -1000 => return 18,
                _ => return 19, // the revoke itself failed
            }
            let (ok, bad): (&[u8], &[u8]) = if kill {
                (
                    b"[ cap-graph ] leg=kill ok\n",
                    b"[ cap-graph ] leg=kill LEAKED (the granted copy outlived a revoke inside its donor's kill)\n",
                )
            } else {
                (
                    b"[ cap-graph ] leg=exit ok\n",
                    b"[ cap-graph ] leg=exit LEAKED (the granted copy outlived a revoke inside its donor's exit)\n",
                )
            };
            if verdict(alive(granted), ok, bad) {
                leaked |= bit;
            }
        }

        // Leg 7, launder without DERIVE: `sys_tensor_share` installs a child
        // of a USE-only handle (as TCP accept and the GPU fence do, through
        // the same `install_cap` path). Share T1 from T, T2 from T1, drop T1,
        // revoke T: T2 must die with it.
        let mut dims = [0u64; 8];
        dims[0] = 4;
        let data = [0u8; 16];
        let desc = libneo::TensorCreateDesc::new(1, libneo::TENSOR_DTYPE_F32, false, dims, &data);
        let t0 = match libneo::sys_tensor_create(&desc) {
            Ok(h) => h,
            Err(_) => return 20,
        };
        let t1 = match libneo::sys_tensor_share(t0) {
            Ok(h) => h,
            Err(_) => return 21,
        };
        let t2 = match libneo::sys_tensor_share(t1) {
            Ok(h) => h,
            Err(_) => return 22,
        };
        if libneo::sys_tensor_drop(t1) != 0 {
            return 23;
        }
        if sys_revoke(libneo::Handle(t0.0)) != 0 {
            return 24;
        }
        let mut q = libneo::TensorQueryDesc::empty();
        if verdict(
            libneo::sys_tensor_query(t2, &mut q) >= 0,
            b"[ cap-graph ] leg=share-close ok\n",
            b"[ cap-graph ] leg=share-close LEAKED (a shared tensor outlived revoke(root) after its parent was dropped)\n",
        ) {
            leaked |= 64;
        }

        if leaked == 0 {
            say(b"[ cap-graph ] PASS: no cap outlived the revoke of its ancestor\n");
            0
        } else {
            10 + leaked
        }
    }

    fn cap_mint_and_use() -> i32 {
        let (local, worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        if sys_spawn("worker", &[worker]).is_err() {
            return 2;
        }
        if sys_send(local, b"ping") < 0 {
            return 3;
        }
        let mut buf = [0u8; 8];
        if sys_recv(local, &mut buf) < 0 {
            return 4;
        }
        0
    }

    fn cap_derive_subset() -> i32 {
        let (local, worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let send_only = match sys_derive(
            local,
            CapRights::new(meta_rights::USE, channel_rights::SEND),
        ) {
            Ok(h) => h,
            Err(_) => return 2,
        };
        let mut buf = [0u8; 8];
        if sys_recv(send_only, &mut buf) != EPERM {
            return 3;
        }
        if sys_spawn("worker", &[worker]).is_err() {
            return 4;
        }
        if sys_send(send_only, b"ping") < 0 {
            return 5;
        }
        if sys_recv(local, &mut buf) < 0 {
            return 6;
        }
        0
    }

    fn cap_derive_rights_violation() -> i32 {
        let (local, _) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let send_only = match sys_derive(
            local,
            CapRights::new(meta_rights::USE | meta_rights::DERIVE, channel_rights::SEND),
        ) {
            Ok(h) => h,
            Err(_) => return 2,
        };
        match sys_derive(
            send_only,
            CapRights::new(
                meta_rights::USE,
                channel_rights::SEND | channel_rights::RECV,
            ),
        ) {
            Err(e) if e == EPERM => 0,
            _ => 3,
        }
    }

    fn cap_derive_missing_meta() -> i32 {
        let (local, _) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let no_derive = match sys_derive(
            local,
            CapRights::new(meta_rights::USE, channel_rights::SEND),
        ) {
            Ok(h) => h,
            Err(_) => return 2,
        };
        match sys_derive(
            no_derive,
            CapRights::new(meta_rights::USE, channel_rights::SEND),
        ) {
            Err(e) if e == EPERM => 0,
            _ => 3,
        }
    }

    fn cap_grant_basic() -> i32 {
        let (ctl_local, ctl_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let (data_local, data_grant) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 2,
        };
        if sys_spawn("worker", &[ctl_worker]).is_err() {
            return 3;
        }
        if sys_send(ctl_local, b"go") < 0 {
            return 4;
        }
        if sys_grant(ctl_local, data_grant, FULL_CHANNEL_RIGHTS) != 0 {
            return 5;
        }
        if sys_send(data_local, b"x") < 0 {
            return 6;
        }
        0
    }

    fn cap_grant_missing_meta() -> i32 {
        let (ctl_local, ctl_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let (data_local, _) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 2,
        };
        let no_grant = match sys_derive(
            data_local,
            CapRights::new(meta_rights::USE, channel_rights::SEND),
        ) {
            Ok(h) => h,
            Err(_) => return 3,
        };
        if sys_spawn("worker", &[ctl_worker]).is_err() {
            return 4;
        }
        if sys_send(ctl_local, b"go") < 0 {
            return 5;
        }
        let mut ready = [0u8; 8];
        if sys_recv(ctl_local, &mut ready) < 0 {
            return 6;
        }
        if sys_grant(ctl_local, no_grant, FULL_CHANNEL_RIGHTS) != EPERM {
            return 7;
        }
        0
    }

    fn cap_revoke_local() -> i32 {
        let (local, _) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let child = match sys_derive(
            local,
            CapRights::new(meta_rights::USE | meta_rights::REVOKE, channel_rights::SEND),
        ) {
            Ok(h) => h,
            Err(_) => return 2,
        };
        if sys_revoke(child) != 0 {
            return 3;
        }
        if sys_send(child, b"x") != EBADF {
            return 4;
        }
        match sys_derive(
            local,
            CapRights::new(meta_rights::USE, channel_rights::SEND),
        ) {
            Ok(_) => 0,
            Err(_) => 5,
        }
    }

    fn cap_revoke_cross_process(descendant: bool) -> i32 {
        let (ctl_local, ctl_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let (_, data_grant) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 2,
        };
        if sys_spawn("worker", &[ctl_worker]).is_err() {
            return 3;
        }
        if sys_send(ctl_local, b"go") < 0 {
            return 4;
        }
        if sys_grant(ctl_local, data_grant, FULL_CHANNEL_RIGHTS) != 0 {
            return 5;
        }
        let mut ready = [0u8; 8];
        if sys_recv(ctl_local, &mut ready) < 0 {
            return 6;
        }
        if sys_revoke(data_grant) != 0 {
            return 7;
        }
        if sys_send(ctl_local, b"go") < 0 {
            return 8;
        }
        let _ = descendant;
        0
    }

    fn cap_revoke_spawn_inheritance_descendants() -> i32 {
        let (ctl_local, ctl_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let (data_root, _) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 2,
        };
        let inherited_source = match sys_derive(
            data_root,
            CapRights::new(
                meta_rights::USE | meta_rights::DERIVE | meta_rights::GRANT,
                channel_rights::SEND,
            ),
        ) {
            Ok(h) => h,
            Err(_) => return 3,
        };
        if sys_spawn("worker", &[ctl_worker, inherited_source]).is_err() {
            return 4;
        }
        let mut ready = [0u8; 8];
        if sys_recv(ctl_local, &mut ready) < 0 {
            return 5;
        }
        if sys_revoke(data_root) != 0 {
            return 6;
        }
        if sys_send(ctl_local, b"go") < 0 {
            return 7;
        }
        let mut done = [0u8; 8];
        if sys_recv(ctl_local, &mut done) < 0 {
            return 8;
        }
        0
    }

    fn cap_survive_exit() -> i32 {
        let (ctl_local, ctl_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let (data_local, data_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 2,
        };
        if sys_spawn("worker", &[ctl_worker, data_worker]).is_err() {
            return 3;
        }
        if sys_send(ctl_local, b"go") < 0 {
            return 4;
        }
        let granted = match sys_recv_grant(ctl_local, FULL_CHANNEL_RIGHTS) {
            Ok(h) => h,
            Err(_) => return 5,
        };
        let mut buf = [0u8; 8];
        if sys_recv(ctl_local, &mut buf) != EPIPE {
            return 6;
        }
        if sys_send(granted, b"x") != EPIPE {
            return 7;
        }
        if sys_send(data_local, b"x") != EPIPE {
            return 8;
        }
        0
    }
}

/// OT-61 C2.2 (`mem_quota`): a budgeted process's memory trip is local.
/// Each worker role is described at `mem_quota_worker` in neo-worker.
/// Returns 0, or `step` / `role * 100 + detail` for the first failure.
#[cfg(feature = "mem-quota-test")]
mod mem_quota_tests {
    use super::*;
    use libneo::{
        ENOMEM, Handle, ProcessHandle, TENSOR_DTYPE_F32, TENSOR_MAX_RANK, TensorCreateDesc,
        mem_info::UNLIMITED, sys_mem_info, sys_spawn_budgeted, sys_tensor_create, sys_wait,
    };

    const MIB: u64 = 1 << 20;

    fn say(line: &[u8]) {
        sys_write(1, line.as_ptr(), line.len());
    }

    /// Spawn a worker (budgeted when `budget` is `Some`) and send it `role`.
    fn child(role: u8, budget: Option<u64>) -> Result<(Handle, ProcessHandle), i32> {
        let (mine, theirs) = sys_chan_create().map_err(|_| 1)?;
        let (_, ph) = match budget {
            Some(b) => sys_spawn_budgeted("worker", &[theirs], b),
            None => sys_spawn("worker", &[theirs]),
        }
        .map_err(|_| 2)?;
        if sys_send(mine, &[role]) < 0 {
            return Err(3);
        }
        Ok((mine, ph))
    }

    fn verdict(ch: Handle) -> i32 {
        let mut b = [0u8; 8];
        match sys_recv(ch, &mut b) {
            n if n >= 1 => i32::from(b[0]),
            _ => 99,
        }
    }

    pub fn run() -> i32 {
        // init: the unlimited root account.
        let Ok(start) = sys_mem_info() else {
            return 1;
        };
        if start.flags != 0 || start.limit_frames != UNLIMITED {
            return 2;
        }
        let baseline = start.used_frames;

        // CP-89: a budget too small for the child's own image is ENOMEM, and
        // what the failed spawn charged is released.
        let (_, tiny) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 3,
        };
        match sys_spawn_budgeted("worker", &[tiny], 4096) {
            Err(e) if e == ENOMEM => {}
            _ => return 4,
        }
        match sys_mem_info() {
            Ok(i) if i.used_frames == baseline => {}
            _ => return 5,
        }
        say(b"[ mem-quota ] a budget too small for the image: ENOMEM, nothing kept\n");

        // Roles 1 + 2: an 8 MiB child fills its budget and holds it; an
        // unbudgeted sibling still maps and touches 16 MiB meanwhile.
        let (r1, r1_ph) = match child(1, Some(8 * MIB)) {
            Ok(c) => c,
            Err(d) => return 100 + d,
        };
        match verdict(r1) {
            0 => say(b"[ mem-quota ] 8 MiB budget: ENOMEM at the limit, used <= limit\n"),
            v => return 110 + v,
        }
        let (r2, r2_ph) = match child(2, None) {
            Ok(c) => c,
            Err(d) => return 200 + d,
        };
        match verdict(r2) {
            0 => say(b"[ mem-quota ] neighbour mapped 16 MiB while the budget was full\n"),
            v => return 210 + v,
        }
        if sys_wait(r2_ph, 0) != Ok(0) {
            return 220;
        }
        if sys_send(r1, b"go") < 0 || sys_wait(r1_ph, 0) != Ok(0) {
            return 120;
        }

        // Role 3: the same 8 MiB heap growth, budgeted at 4 MiB, ends in the
        // allocation-failure exit (137); unbudgeted, it completes.
        match child(3, Some(4 * MIB)) {
            Ok((_, ph)) if sys_wait(ph, 0) == Ok(137) => {}
            Ok(_) => return 310,
            Err(d) => return 300 + d,
        }
        match child(3, None) {
            Ok((_, ph)) if sys_wait(ph, 0) == Ok(0) => {}
            Ok(_) => return 320,
            Err(d) => return 330 + d,
        }
        say(b"[ mem-quota ] heap growth past the budget: exit 137, only that process\n");

        // Role 4: two 8 MiB grandchildren under a 12 MiB parent.
        let (r4, r4_ph) = match child(4, Some(12 * MIB)) {
            Ok(c) => c,
            Err(d) => return 400 + d,
        };
        match verdict(r4) {
            0 => {
                say(b"[ mem-quota ] nested: the second grandchild tripped at the parent's limit\n")
            }
            v => return 410 + v,
        }
        if sys_wait(r4_ph, 0) != Ok(0) {
            return 450;
        }

        // Role 6: tensor storage is refused to a budgeted process, and still
        // works for init (the control).
        let data = [0u8; 16];
        let mut dims = [0u64; TENSOR_MAX_RANK];
        dims[0] = 4;
        let desc = TensorCreateDesc::new(1, TENSOR_DTYPE_F32, false, dims, &data);
        if sys_tensor_create(&desc).is_err() {
            return 600;
        }
        let (r6, r6_ph) = match child(6, Some(4 * MIB)) {
            Ok(c) => c,
            Err(d) => return 610 + d,
        };
        match verdict(r6) {
            0 => say(b"[ mem-quota ] budgeted tensor create: EPERM\n"),
            v => return 620 + v,
        }
        if sys_wait(r6_ph, 0) != Ok(0) {
            return 630;
        }

        // Every child is gone: init's account is back where it started.
        match sys_mem_info() {
            Ok(i) if i.used_frames == baseline => {}
            Ok(_) => return 700,
            Err(_) => return 701,
        }
        say(b"[ mem-quota ] PASS: every trip was local; the account is back to its baseline\n");
        0
    }
}

/// OT-66 K1a (OT-61 C2.1a), the `cap_rooted_open` scenario: the root form of
/// `sys_open` resolves only through a RootDirCap. Init holds the one the
/// kernel minted (found by `SYS_HANDLE_INFO`); a worker without one gets
/// EPERM; a worker given a derived read-only one can read but not write
/// through it; revoking the derived cap kills the file the worker opened
/// through it and refuses its next root-form open; a plain file handle can't
/// be spawn-inherited. Then (Janus #112 F1) read, readdir, mount and umount
/// check the rights of the handle used, not only those of the OpenFile behind
/// it, which a derive shares: a narrowed copy is refused what the original may
/// do.
#[cfg(feature = "cap-rooted-open-test")]
mod cap_rooted_tests {
    use super::*;
    use libneo::{
        CapRights, EACCES, EPERM, FileHandle, FileRights, Handle, MOUNT_FS_TYPE_FAT32_DEVICE1_RO,
        OPEN_ROOT_TEMPORARY, encode_mount_request_v1, meta_rights, sys_derive, sys_mount, sys_open,
        sys_read, sys_readdir, sys_revoke, sys_umount,
    };

    /// The client policy's RootDirCap rights (libneo `CLIENT_ROOT_CAP_RIGHTS`,
    /// built only with `libneo/cap-rooted-open`): every file right but MOUNT.
    const CLIENT_ROOT_RIGHTS: CapRights = CapRights::new(
        meta_rights::USE | meta_rights::DERIVE | meta_rights::GRANT | meta_rights::REVOKE,
        (FileRights::READ.bits()
            | FileRights::WRITE.bits()
            | FileRights::LOOKUP.bits()
            | FileRights::READDIR.bits()
            | FileRights::EXECUTE.bits()) as u8,
    );

    /// A child name under "/" that no mount uses: umount through a handle with
    /// MOUNT gets past the rights check and reports ENOENT.
    const NO_SUCH_MOUNT: &str = "k1a-no-such-mount";

    const SCRATCH: &str = "/scratch";

    fn say(line: &[u8]) {
        sys_write(1, line.as_ptr(), line.len());
    }

    /// The worker's one-byte verdict: 0, or its failing step.
    fn verdict(ctl: Handle) -> i32 {
        let mut b = [0u8; 8];
        match sys_recv(ctl, &mut b) {
            n if n >= 1 => i32::from(b[0]),
            _ => 99,
        }
    }

    pub fn run() -> i32 {
        // 1. Init holds the kernel's RootDirCap.
        let Some(root) = libneo::root_dir_cap() else {
            return 1;
        };
        say(b"[ cap-rooted ] init holds a RootDirCap\n");
        // 2. Its own root-form open works.
        let rw = match sys_open(
            SCRATCH,
            OPEN_ROOT_TEMPORARY,
            FileRights::READ.union(FileRights::WRITE),
        ) {
            Ok(f) => f,
            Err(_) => return 2,
        };
        // 3. A plain file handle can't be spawn-inherited (directory caps only).
        let (_, ctl0) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 3,
        };
        match sys_spawn("worker", &[ctl0, Handle(rw.0)]) {
            Err(e) if e == EPERM => {}
            _ => return 4,
        }
        say(b"[ cap-rooted ] a plain file handle is not inheritable\n");
        // 4. A worker without a RootDirCap: the root form is EPERM.
        let (a_local, a_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 5,
        };
        if sys_spawn("worker", &[a_worker]).is_err() {
            return 6;
        }
        match verdict(a_local) {
            0 => say(b"[ cap-rooted ] no cap: root form EPERM\n"),
            v => return 100 + v,
        }
        // 5. A derived read-only RootDirCap, inherited: read through it, not
        //    write.
        let ro = match sys_derive(
            root,
            CapRights::new(
                meta_rights::USE | meta_rights::DERIVE | meta_rights::GRANT | meta_rights::REVOKE,
                (FileRights::READ.bits() | FileRights::LOOKUP.bits() | FileRights::READDIR.bits())
                    as u8,
            ),
        ) {
            Ok(h) => h,
            Err(_) => return 7,
        };
        let (b_local, b_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 8,
        };
        if sys_spawn("worker", &[b_worker, ro]).is_err() {
            return 9;
        }
        match verdict(b_local) {
            0 => say(b"[ cap-rooted ] derived read-only cap: read ok, write EACCES\n"),
            v => return 200 + v,
        }
        // 6. Revoke it: the file opened through it dies with it, and the
        //    worker's next root-form open is EPERM.
        if sys_revoke(ro) != 0 {
            return 10;
        }
        if sys_send(b_local, b"go") < 0 {
            return 11;
        }
        match verdict(b_local) {
            0 => say(b"[ cap-rooted ] revoked: the opened file EBADF, root form EPERM\n"),
            v => return 200 + v,
        }
        // 7. readdir needs READDIR on the handle: a LOOKUP-only copy of the
        //    root is refused, the root itself reads.
        let lookup_only = match sys_derive(
            root,
            CapRights::new(meta_rights::USE, FileRights::LOOKUP.bits() as u8),
        ) {
            Ok(h) => h,
            Err(_) => return 15,
        };
        let mut buf = [0u8; 256];
        let mut cursor = 0u64;
        if sys_readdir(FileHandle(lookup_only.0), &mut buf, &mut cursor) != EACCES {
            return 16;
        }
        let mut cursor = 0u64;
        if sys_readdir(FileHandle(root.0), &mut buf, &mut cursor) < 0 {
            return 17;
        }
        say(b"[ cap-rooted ] LOOKUP-only copy: readdir EACCES, the root reads\n");
        // 8. mount and umount need MOUNT on the handle: a copy with the client
        //    policy's rights is refused both; the root gets past the rights
        //    check (ENOENT for a child that isn't mounted; a mount that isn't
        //    refused for rights).
        let client = match sys_derive(root, CLIENT_ROOT_RIGHTS) {
            Ok(h) => h,
            Err(_) => return 18,
        };
        if sys_umount(FileHandle(client.0), NO_SUCH_MOUNT, 0) != EACCES {
            return 19;
        }
        let req = encode_mount_request_v1(MOUNT_FS_TYPE_FAT32_DEVICE1_RO, "device1", NO_SUCH_MOUNT);
        if sys_mount(FileHandle(client.0), &req, 0) != EACCES {
            return 20;
        }
        if sys_umount(FileHandle(root.0), NO_SUCH_MOUNT, 0) == EACCES {
            return 21;
        }
        let mounted = sys_mount(FileHandle(root.0), &req, 0);
        if mounted == EACCES {
            return 22;
        }
        if mounted == 0 && sys_umount(FileHandle(root.0), NO_SUCH_MOUNT, 0) != 0 {
            return 23;
        }
        say(b"[ cap-rooted ] client-rights copy: mount and umount EACCES, the root passes\n");
        // 9. read needs READ on the handle: a WRITE-only copy of a read-write
        //    file is refused, the original reads.
        let write_only = match sys_derive(
            Handle(rw.0),
            CapRights::new(meta_rights::USE, FileRights::WRITE.bits() as u8),
        ) {
            Ok(h) => h,
            Err(_) => return 24,
        };
        let mut byte = [0u8; 1];
        if sys_read(FileHandle(write_only.0), &mut byte) != EACCES {
            return 25;
        }
        if sys_read(rw, &mut byte) < 0 {
            return 26;
        }
        say(b"[ cap-rooted ] WRITE-only copy: read EACCES, the original reads\n");
        say(b"[ cap-rooted ] PASS: the root form resolves only through a RootDirCap\n");
        0
    }
}

/// File-capability unification (I-15) live proof. `Handle::File` is now a
/// first-class cap: this opens `/scratch` (rooted with DERIVE+GRANT), proves
/// in-process attenuation (a read-only derived cap can read but not write),
/// meta-gating (the read-only cap can't be re-derived), subset enforcement
/// (deriving a right the parent lacks EPERMs), then grants a read-only cap to a
/// worker that confirms read-OK / write-denied, and finally proves a
/// non-grantable file cap can't be granted. Built only under `file-cap-derive-grant`.
#[cfg(feature = "file-cap-derive-grant")]
mod file_cap_tests {
    use super::*;
    use libneo::{
        CapRights, EACCES, EPERM, FileRights, Handle, OPEN_ROOT_TEMPORARY, meta_rights, sys_derive,
        sys_grant, sys_open, sys_read_fd,
    };

    const SCRATCH_PATH: &str = "/scratch";

    // File semantic ops map onto CapRights.semantic with identity on the low
    // FileRights bits (READ=1<<0, WRITE=1<<1, EXECUTE=1<<4 — see file_cap_logic),
    // so the FileRights bit value is the semantic byte directly.
    const SEM_READ: u8 = FileRights::READ.bits() as u8;
    const SEM_READ_EXEC: u8 = (FileRights::READ.bits() | FileRights::EXECUTE.bits()) as u8;

    pub fn run() -> i32 {
        file_cap_derive_grant()
    }

    fn file_cap_derive_grant() -> i32 {
        // 1. Open /scratch READ|WRITE. ISC-3: the opener is rooted with the file
        //    cap's DERIVE+GRANT meta-rights (the OpenFile keeps READ|WRITE).
        let root = match sys_open(
            SCRATCH_PATH,
            OPEN_ROOT_TEMPORARY,
            FileRights::READ.union(FileRights::WRITE),
        ) {
            Ok(f) => f,
            Err(_) => return 1,
        };
        let root_h = Handle(root.0);

        // 2. ISC-2 (attenuation, the bug Genesis flagged): derive a read-only cap.
        let ro = match sys_derive(root_h, CapRights::new(meta_rights::USE, SEM_READ)) {
            Ok(h) => h,
            Err(_) => return 2,
        };
        let mut buf = [0u8; 8];
        // read is permitted (>= 0; /scratch is empty so this is EOF=0, not an error)
        if sys_read_fd(ro.0, &mut buf) < 0 {
            return 3;
        }
        // write is DENIED — the handle carries READ only (EACCES, the file
        // rights-insufficient errno; NOT the OpenFile, which still has WRITE).
        if sys_write(ro.0, b"x".as_ptr(), 1) != EACCES {
            return 4;
        }

        // 3. Meta-gating (ISC-4): the read-only cap has no DERIVE meta ⇒ EPERM.
        match sys_derive(ro, CapRights::new(meta_rights::USE, SEM_READ)) {
            Err(e) if e == EPERM => {}
            _ => return 5,
        }

        // 4. Subset enforcement (ISC-4/I-15): root has no EXECUTE ⇒ deriving it EPERMs.
        match sys_derive(root_h, CapRights::new(meta_rights::USE, SEM_READ_EXEC)) {
            Err(e) if e == EPERM => {}
            _ => return 6,
        }

        // 5. Cross-process grant (ISC-4): grant a read-only cap to a child; the
        //    worker confirms it can read but not write.
        let (ctl_local, ctl_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 7,
        };
        if sys_spawn("worker", &[ctl_worker]).is_err() {
            return 8;
        }
        if sys_send(ctl_local, b"go") < 0 {
            return 9;
        }
        // Derive a read-only-but-grantable cap (keeps GRANT so it can be granted).
        let ro_grant = match sys_derive(
            root_h,
            CapRights::new(meta_rights::USE | meta_rights::GRANT, SEM_READ),
        ) {
            Ok(h) => h,
            Err(_) => return 10,
        };
        // Offer read-only (USE + READ, NO grant) so the child is a read-only leaf.
        if sys_grant(
            ctl_local,
            ro_grant,
            CapRights::new(meta_rights::USE, SEM_READ),
        ) != 0
        {
            return 11;
        }
        let mut verdict = [0u8; 8];
        let n = sys_recv(ctl_local, &mut verdict);
        if n < 0 {
            return 12;
        }
        if &verdict[..n as usize] != b"ok" {
            return 13;
        }

        // 6. A non-grantable file cap can't be granted (meta-gating on grant): the
        //    read-only cap below lacks GRANT, so granting it EPERMs.
        let ro_nogrant = match sys_derive(root_h, CapRights::new(meta_rights::USE, SEM_READ)) {
            Ok(h) => h,
            Err(_) => return 14,
        };
        if sys_grant(
            ctl_local,
            ro_nogrant,
            CapRights::new(meta_rights::USE, SEM_READ),
        ) != EPERM
        {
            return 15;
        }
        0
    }
}

/// Phase 7.5 CB-3 (ISC-4) — the interface capability live proof. `/net/if/eth0` is a
/// synthetic `Handle::File` minted from the CB-3 `/net` mount; this proves it is a
/// first-class file capability that enters the I-15 graph exactly like any file cap,
/// with **zero new Handle kind**: the cap is openable and readable (its status line
/// names `eth0`), an attenuated read-only child can be derived (and is correctly
/// meta-gated — it cannot re-derive), and revoking the parent kills the derived
/// descendant transitively. Mirrors `file_cap_derive_grant` on the interface path.
/// Built only under `interface-cap-derive-revoke` (pulls kernel `file-caps`).
#[cfg(feature = "interface-cap-derive-revoke")]
mod interface_cap_tests {
    use libneo::{
        CapRights, EBADF, EPERM, FileRights, Handle, OPEN_ROOT_TEMPORARY, meta_rights, sys_derive,
        sys_open, sys_read_fd, sys_revoke,
    };

    // The interface namespace node for the first ethernet interface (eth0), which
    // the two-virtio-net QEMU wiring registers at boot.
    const IFACE_PATH: &str = "/net/if/eth0";
    // File semantic ops map identity onto CapRights.semantic on the low bits
    // (READ=1<<0), so the FileRights bit value is the semantic byte directly.
    const SEM_READ: u8 = FileRights::READ.bits() as u8;

    pub fn run() -> i32 {
        interface_cap_derive_revoke()
    }

    fn interface_cap_derive_revoke() -> i32 {
        // 1. Open /net/if/eth0 READ. ISC-4: a `Handle::File` cap minted from the
        //    synthetic /net mount, rooted with DERIVE+GRANT (file-caps on) — no new
        //    Handle kind, no new syscall.
        let root = match sys_open(IFACE_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(f) => f,
            Err(_) => return 1,
        };
        let root_h = Handle(root.0);

        // 2. Derive an attenuated read-only child (I-15 attenuation works for the
        //    interface cap exactly as for any file cap). Derive BEFORE any read: a
        //    derived `Handle::File` shares the parent's backing `OpenFile` — including
        //    its read offset (dup-style) — so reading via `ro` first reads the status
        //    from offset 0 (reading `root` to EOF first would leave `ro` at EOF).
        let ro = match sys_derive(root_h, CapRights::new(meta_rights::USE, SEM_READ)) {
            Ok(h) => h,
            Err(_) => return 2,
        };

        // 3. The derived read-only cap is usable and reads real content: the
        //    interface status line, which must name eth0.
        let mut buf = [0u8; 32];
        let n = sys_read_fd(ro.0, &mut buf);
        if n <= 0 {
            return 3;
        }
        if !starts_with(&buf[..n as usize], b"eth0") {
            return 4;
        }

        // 4. Meta-gating: the read-only child has no DERIVE meta ⇒ re-deriving EPERMs.
        match sys_derive(ro, CapRights::new(meta_rights::USE, SEM_READ)) {
            Err(e) if e == EPERM => {}
            _ => return 5,
        }

        // 5. Revoke the root cap ⇒ the derived descendant dies transitively through
        //    the I-15 derivation graph (the security crux, identical to file caps).
        if sys_revoke(root_h) != 0 {
            return 6;
        }
        if sys_read_fd(ro.0, &mut buf) != EBADF {
            return 7;
        }
        // The root handle itself is gone too.
        if sys_read_fd(root.0, &mut buf) != EBADF {
            return 8;
        }
        0
    }

    fn starts_with(hay: &[u8], prefix: &[u8]) -> bool {
        hay.len() >= prefix.len() && &hay[..prefix.len()] == prefix
    }
}

/// OT-18 Slice E3-revoke (D6 revocation half) — the transitive-revoke proof.
///
/// Models the Arthur grant lifecycle at the kernel boundary by syscall: the
/// shell-role opens a file (rooted DERIVE+GRANT+REVOKE per I-15), derives the
/// per-scope cap it would hand Arthur, and from it a 2-deep chain of descendants
/// (the holders a delegated cap could itself spawn). Revoking the parent must
/// kill that cap AND every descendant through the derivation graph (the
/// `revoke_handle` Phase-2 guarantee — composed here, never modified), while a
/// separately-granted scope and the cap's own ancestor stay live.
///
/// Arthur's *real* granted cap is USE-only (it cannot sub-derive), so the
/// multi-hop transitive property is exercised at the kernel level exactly as the
/// dispatch directs: derive a child cap in the scenario, revoke the parent,
/// assert the child is dead. Every assertion is load-bearing on the exit code
/// (the kernel's pass criterion is `exit == 0`), so this cannot pass while
/// silently mis-revoking. The driver logs the discriminator (scope, cap fd,
/// descendants-walked) to serial so a missed-descendant failure is
/// self-diagnosing. Built only under `arthur-grant-revoke`.
#[cfg(feature = "arthur-grant-revoke")]
mod arthur_grant_revoke_tests {
    use super::*;
    use libneo::{
        CapRights, EBADF, FileRights, Handle, OPEN_ROOT_TEMPORARY, meta_rights, sys_derive,
        sys_open, sys_read_fd, sys_revoke,
    };

    // /hello.txt is the testfs read fixture. Two independent opens give two
    // independent derivation roots: X (the revoked scope) and Y (the survivor).
    const RESOURCE: &str = "/hello.txt";
    // File semantic ops map identity onto CapRights.semantic on the low bits
    // (READ=1<<0), so the FileRights bit value is the semantic byte directly.
    const SEM_READ: u8 = FileRights::READ.bits() as u8;

    pub fn run() -> i32 {
        arthur_grant_revoke()
    }

    fn arthur_grant_revoke() -> i32 {
        // ── Scope X: the grant that will be revoked ──────────────────────────
        // 1. Open the resource. I-15: the opener is rooted with the file cap's
        //    DERIVE+GRANT meta-rights (FR_GRANT also confers REVOKE); the backing
        //    OpenFile keeps READ.
        let root_x = match sys_open(RESOURCE, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(f) => f,
            Err(_) => return 1,
        };
        let root_x_h = Handle(root_x.0);

        // 2. Derive the per-scope cap the shell hands Arthur: read + GRANT (so it
        //    is delegatable) — FR_GRANT also confers REVOKE, so THIS is the cap
        //    the shell later `sys_revoke`s. Keep DERIVE so the scenario can build
        //    the descendant chain a delegated holder could.
        let d_x = match sys_derive(
            root_x_h,
            CapRights::new(
                meta_rights::USE | meta_rights::DERIVE | meta_rights::GRANT,
                SEM_READ,
            ),
        ) {
            Ok(h) => h,
            Err(_) => return 2,
        };
        // 3. A delegated descendant (depth 1) and its child (depth 2) — the
        //    holders a sub-derived/granted cap could itself spawn.
        let d_x_child = match sys_derive(
            d_x,
            CapRights::new(meta_rights::USE | meta_rights::DERIVE, SEM_READ),
        ) {
            Ok(h) => h,
            Err(_) => return 3,
        };
        let d_x_grandchild = match sys_derive(d_x_child, CapRights::new(meta_rights::USE, SEM_READ))
        {
            Ok(h) => h,
            Err(_) => return 4,
        };

        // ── Scope Y: an independent grant that must survive X's revoke ────────
        let root_y = match sys_open(RESOURCE, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(f) => f,
            Err(_) => return 5,
        };
        let d_y = match sys_derive(Handle(root_y.0), CapRights::new(meta_rights::USE, SEM_READ)) {
            Ok(h) => h,
            Err(_) => return 6,
        };

        // ── Pre-revoke: every cap reads (all holders have authority) ──────────
        let mut buf = [0u8; 8];
        if sys_read_fd(d_x.0, &mut buf) < 0 {
            return 7;
        }
        if sys_read_fd(d_x_child.0, &mut buf) < 0 {
            return 8;
        }
        if sys_read_fd(d_x_grandchild.0, &mut buf) < 0 {
            return 9;
        }
        if sys_read_fd(d_y.0, &mut buf) < 0 {
            return 10;
        }
        log_pre_revoke(d_x.0, d_x_child.0, d_x_grandchild.0);

        // ── Revoke scope X at the kernel (the enforcement action) ────────────
        // Composes `revoke_handle` (Phase-2, frozen). The walk starts at d_x and
        // kills d_x + d_x_child + d_x_grandchild transitively through the graph.
        if sys_revoke(d_x) != 0 {
            return 11;
        }

        // ── Post-revoke: the cap AND every descendant are dead (EBADF) ────────
        // A revoked handle is removed from the table, so the kernel refuses the
        // op with EBADF — the boundary is the cap, enforced with zero reference
        // to grants.json (store-vs-cap, ISC-3). A surviving descendant here is a
        // privilege-escalation hole, and a distinct return code localizes it.
        if sys_read_fd(d_x.0, &mut buf) != EBADF {
            return 12;
        }
        if sys_read_fd(d_x_child.0, &mut buf) != EBADF {
            return 13; // depth-1 descendant survived — the walk missed it
        }
        if sys_read_fd(d_x_grandchild.0, &mut buf) != EBADF {
            return 14; // depth-2 descendant survived — the walk stopped short
        }

        // ── Minimality + isolation ───────────────────────────────────────────
        // The ancestor (root_x) is NOT a descendant of d_x, so it survives:
        // revoke walks DOWN the graph, never up.
        if sys_read_fd(root_x_h.0, &mut buf) < 0 {
            return 15;
        }
        // The independent scope Y is a separate derivation tree → untouched.
        if sys_read_fd(d_y.0, &mut buf) < 0 {
            return 16;
        }
        log_post_revoke();
        0
    }

    // ── discriminator logging (capture-before-you-need-it) ────────────────────

    fn log_pre_revoke(cap: u32, child: u32, grandchild: u32) {
        write_str(b"arthur_grant_revoke: scope=FileRead resource=/hello.txt cap_fd=");
        write_dec(cap);
        write_str(b" descendants_walked=2 child_fd=");
        write_dec(child);
        write_str(b" grandchild_fd=");
        write_dec(grandchild);
        write_str(b"\n");
    }

    fn log_post_revoke() {
        write_str(
            b"arthur_grant_revoke: post-revoke parent=EBADF child=EBADF grandchild=EBADF isolated_y=ok ancestor=ok\n",
        );
    }

    fn write_str(bytes: &[u8]) {
        let _ = sys_write(1, bytes.as_ptr(), bytes.len());
    }

    fn write_dec(mut value: u32) {
        let mut buf = [0u8; 10];
        let mut idx = buf.len();
        loop {
            idx -= 1;
            buf[idx] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        let _ = sys_write(1, buf[idx..].as_ptr(), buf.len() - idx);
    }
}

#[cfg(any(
    feature = "vfs-testfs-open-read-close",
    feature = "vfs-testfs-write-readback",
    feature = "vfs-type-vs-rights-errors",
    feature = "vfs-stdout-compat",
    feature = "vfs-block-smoke",
    feature = "vfs-readdir-testfs-root",
    feature = "vfs-readdir-requires-right",
    feature = "vfs-readdir-regular-is-enotdir",
    feature = "vfs-directory-read-is-eisdir",
    feature = "readdir-cursor-eof-stable",
    feature = "readdir-buffer-too-small"
))]
mod vfs_tests {
    use super::*;
    use libneo::{
        EACCES, EBADF, EISDIR, ENOTDIR, FileHandle, FileRights, NEO_DIRENT_KIND_DIRECTORY,
        NEO_DIRENT_KIND_REGULAR, NEO_DIRENT_KIND_SYMLINK, parse_dirent_v1, sys_close, sys_open,
        sys_read, sys_readdir,
    };

    const HELLO_PATH: &str = "/hello.txt";
    const SCRATCH_PATH: &str = "/scratch";
    const DIR_PATH: &str = "/dir";
    const OPEN_ROOT_TEMPORARY: u32 = 0;
    const HELLO_BYTES: &[u8] = b"NeoPlutarch testfs hello\n";

    pub fn run() -> i32 {
        #[cfg(feature = "vfs-testfs-open-read-close")]
        {
            return open_read_close();
        }
        #[cfg(feature = "vfs-testfs-write-readback")]
        {
            return write_readback();
        }
        #[cfg(feature = "vfs-type-vs-rights-errors")]
        {
            return type_vs_rights_errors();
        }
        #[cfg(feature = "vfs-stdout-compat")]
        {
            return stdout_compat();
        }
        #[cfg(feature = "vfs-block-smoke")]
        {
            return block_smoke();
        }
        #[cfg(feature = "vfs-readdir-testfs-root")]
        {
            return readdir_testfs_root();
        }
        #[cfg(feature = "vfs-readdir-requires-right")]
        {
            return readdir_requires_right();
        }
        #[cfg(feature = "vfs-readdir-regular-is-enotdir")]
        {
            return readdir_regular_is_enotdir();
        }
        #[cfg(feature = "vfs-directory-read-is-eisdir")]
        {
            return directory_read_is_eisdir();
        }
        #[cfg(feature = "readdir-cursor-eof-stable")]
        {
            return readdir_cursor_eof_stable();
        }
        #[cfg(feature = "readdir-buffer-too-small")]
        {
            return readdir_buffer_too_small();
        }
    }

    fn open_read_close() -> i32 {
        let file = match sys_open(HELLO_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        let mut buf = [0u8; 64];
        let n = sys_read(file, &mut buf);
        if n != HELLO_BYTES.len() as i64 {
            return 2;
        }
        if &buf[..n as usize] != HELLO_BYTES {
            return 3;
        }
        if sys_close(file) != 0 {
            return 4;
        }
        if sys_close(file) != EBADF {
            return 5;
        }
        write_stdout(b"[ neo-init ] vfs open/read/close ok second_close=-9\n");
        0
    }

    fn write_readback() -> i32 {
        let payload = b"phase3-vfs-write-readback";
        let writer = match sys_open(SCRATCH_PATH, OPEN_ROOT_TEMPORARY, FileRights::WRITE) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        let n = sys_write(writer.0, payload.as_ptr(), payload.len());
        if n != payload.len() as i64 {
            return 2;
        }
        if sys_close(writer) != 0 {
            return 3;
        }

        let reader = match sys_open(SCRATCH_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(file) => file,
            Err(_) => return 4,
        };
        let mut buf = [0u8; 27];
        let n = sys_read(reader, &mut buf);
        if n != payload.len() as i64 {
            return 5;
        }
        if &buf[..n as usize] != payload {
            return 6;
        }
        if sys_close(reader) != 0 {
            return 7;
        }
        write_stdout(b"[ neo-init ] vfs write/readback ok reopened=true\n");
        0
    }

    fn type_vs_rights_errors() -> i32 {
        let type_err = match sys_open(DIR_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(_) => return 1,
            Err(e) => e,
        };
        let rights_err = match sys_open(HELLO_PATH, OPEN_ROOT_TEMPORARY, FileRights::WRITE) {
            Ok(_) => return 2,
            Err(e) => e,
        };
        if type_err != EISDIR {
            return 3;
        }
        if rights_err != EACCES {
            return 4;
        }
        write_stdout(b"[ neo-init ] vfs errors type=EISDIR(-21) rights=EACCES(-13)\n");
        0
    }

    fn stdout_compat() -> i32 {
        let first = match sys_open(HELLO_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        if first.0 == 1 {
            return 2;
        }
        let second = match sys_open(SCRATCH_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(file) => file,
            Err(_) => return 3,
        };
        if second.0 == 1 || first.0 == second.0 {
            return 4;
        }
        let marker = b"NEO_STDOUT_COMPAT/1 fd=1 route=serial\n";
        let n = sys_write(1, marker.as_ptr(), marker.len());
        if n != marker.len() as i64 {
            return 5;
        }
        let mut buf = [0u8; 4];
        if sys_read(FileHandle(1), &mut buf) != EBADF {
            return 6;
        }
        if sys_close(first) != 0 || sys_close(second) != 0 {
            return 7;
        }
        write_stdout(b"[ neo-init ] vfs stdout compat ok sys_open_skips_fd1=true\n");
        0
    }

    fn block_smoke() -> i32 {
        let hello = match sys_open(HELLO_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        if hello.0 == 1 {
            return 2;
        }
        let mut hello_buf = [0u8; 64];
        let n = sys_read(hello, &mut hello_buf);
        if n != HELLO_BYTES.len() as i64 {
            return 3;
        }
        if &hello_buf[..n as usize] != HELLO_BYTES {
            return 4;
        }
        if sys_close(hello) != 0 {
            return 5;
        }

        let payload = b"phase3-block-vfs-same-boot";
        let writer = match sys_open(SCRATCH_PATH, OPEN_ROOT_TEMPORARY, FileRights::WRITE) {
            Ok(file) => file,
            Err(_) => return 6,
        };
        if writer.0 == 1 {
            return 7;
        }
        let n = sys_write(writer.0, payload.as_ptr(), payload.len());
        if n != payload.len() as i64 {
            return 8;
        }
        if sys_close(writer) != 0 {
            return 9;
        }

        let reader = match sys_open(SCRATCH_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(file) => file,
            Err(_) => return 10,
        };
        if reader.0 == 1 {
            return 11;
        }
        let mut buf = [0u8; 32];
        let n = sys_read(reader, &mut buf);
        if n != payload.len() as i64 {
            return 12;
        }
        if &buf[..n as usize] != payload {
            return 13;
        }
        if sys_close(reader) != 0 {
            return 14;
        }

        let marker = b"NEO_STDOUT_COMPAT/1 namespace=serial fd=1 source=vfs-block-smoke\n";
        let n = sys_write(1, marker.as_ptr(), marker.len());
        if n != marker.len() as i64 {
            return 15;
        }
        write_stdout(
            b"[ neo-init ] vfs block smoke ok open_read_write_close=true fd1_namespace=serial\n",
        );
        0
    }

    fn dir_rights() -> FileRights {
        FileRights::LOOKUP.union(FileRights::READDIR)
    }

    fn read_one(fd: FileHandle, cursor: &mut u64, buf: &mut [u8]) -> Result<usize, i64> {
        let n = sys_readdir(fd, buf, cursor);
        if n < 0 { Err(n) } else { Ok(n as usize) }
    }

    fn expect_entry(
        fd: FileHandle,
        cursor: &mut u64,
        expected_name: &[u8],
        expected_kind: u8,
        buf: &mut [u8],
    ) -> Result<(), i32> {
        let n = read_one(fd, cursor, buf).map_err(|_| 20)?;
        if n == 0 {
            return Err(21);
        }
        let entry = parse_dirent_v1(&buf[..n]).map_err(|_| 22)?;
        if entry.record_len as usize != n || entry.name_len as usize != expected_name.len() {
            return Err(23);
        }
        if entry.kind != expected_kind || entry.flags != 0 || entry.name != expected_name {
            return Err(24);
        }
        if entry.next_cursor != *cursor {
            return Err(25);
        }
        Ok(())
    }

    fn readdir_testfs_root() -> i32 {
        let dir = match sys_open("/", OPEN_ROOT_TEMPORARY, dir_rights()) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        let mut cursor = 0u64;
        let mut buf = [0u8; 64];
        if expect_entry(
            dir,
            &mut cursor,
            b"hello.txt",
            NEO_DIRENT_KIND_REGULAR,
            &mut buf,
        )
        .is_err()
        {
            return 2;
        }
        if expect_entry(
            dir,
            &mut cursor,
            b"scratch",
            NEO_DIRENT_KIND_REGULAR,
            &mut buf,
        )
        .is_err()
        {
            return 3;
        }
        if expect_entry(
            dir,
            &mut cursor,
            b"dir",
            NEO_DIRENT_KIND_DIRECTORY,
            &mut buf,
        )
        .is_err()
        {
            return 4;
        }
        if expect_entry(dir, &mut cursor, b"link", NEO_DIRENT_KIND_SYMLINK, &mut buf).is_err() {
            return 5;
        }
        if read_one(dir, &mut cursor, &mut buf) != Ok(0) {
            return 6;
        }
        if sys_close(dir) != 0 {
            return 7;
        }
        write_stdout(b"[ neo-init ] vfs readdir testfs root ok entries=4 eof=stable\n");
        0
    }

    fn readdir_requires_right() -> i32 {
        let dir = match sys_open("/", OPEN_ROOT_TEMPORARY, FileRights::LOOKUP) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        let mut cursor = 0u64;
        let mut buf = [0u8; 64];
        if sys_readdir(dir, &mut buf, &mut cursor) != EACCES {
            return 2;
        }
        if sys_close(dir) != 0 {
            return 3;
        }
        write_stdout(b"[ neo-init ] vfs readdir requires READDIR right ok errno=-13\n");
        0
    }

    fn readdir_regular_is_enotdir() -> i32 {
        let file = match sys_open(HELLO_PATH, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        let mut cursor = 0u64;
        let mut buf = [0u8; 64];
        if sys_readdir(file, &mut buf, &mut cursor) != ENOTDIR {
            return 2;
        }
        if sys_close(file) != 0 {
            return 3;
        }
        write_stdout(b"[ neo-init ] vfs readdir regular returns ENOTDIR(-20)\n");
        0
    }

    fn directory_read_is_eisdir() -> i32 {
        for rights in [FileRights::READ, FileRights::WRITE, FileRights::EXECUTE] {
            match sys_open(DIR_PATH, OPEN_ROOT_TEMPORARY, rights) {
                Ok(_) => return 10,
                Err(EISDIR) => {}
                Err(_) => return 11,
            }
        }
        let dir = match sys_open(DIR_PATH, OPEN_ROOT_TEMPORARY, dir_rights()) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        let mut buf = [0u8; 16];
        if sys_read(dir, &mut buf) != EISDIR {
            return 2;
        }
        if sys_close(dir) != 0 {
            return 3;
        }
        write_stdout(
            b"[ neo-init ] vfs read on directory returns EISDIR(-21); wrong-right opens reject\n",
        );
        0
    }

    fn readdir_cursor_eof_stable() -> i32 {
        let dir = match sys_open(DIR_PATH, OPEN_ROOT_TEMPORARY, dir_rights()) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        let mut cursor = 0u64;
        let mut buf = [0u8; 64];
        if sys_readdir(dir, &mut buf, &mut cursor) != 0 || cursor != 0 {
            return 2;
        }
        if sys_readdir(dir, &mut buf, &mut cursor) != 0 || cursor != 0 {
            return 3;
        }
        if sys_close(dir) != 0 {
            return 4;
        }
        write_stdout(b"[ neo-init ] vfs readdir cursor EOF stable ok\n");
        0
    }

    fn readdir_buffer_too_small() -> i32 {
        let dir = match sys_open("/", OPEN_ROOT_TEMPORARY, dir_rights()) {
            Ok(file) => file,
            Err(_) => return 1,
        };
        let mut cursor = 0u64;
        let mut buf = [0u8; 8];
        if sys_readdir(dir, &mut buf, &mut cursor) != libneo::EINVAL {
            return 2;
        }
        if cursor != 0 {
            return 3;
        }
        if sys_close(dir) != 0 {
            return 4;
        }
        write_stdout(b"[ neo-init ] vfs readdir buffer-too-small returns EINVAL(-22)\n");
        0
    }

    fn write_stdout(msg: &[u8]) {
        let _ = sys_write(1, msg.as_ptr(), msg.len());
    }
}

/// OT-61 rung 5c: neo-init as the browser's content process. It hands the
/// root dial cap to the fetch broker and keeps only the channel, then checks
/// what comes back for four URLs (THINK §7.1).
#[cfg(feature = "fetch-broker")]
mod fetch_broker_tests {
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    use fetch_proto::wire::{self, ErrorCode, Method, Msg};
    use libneo::{
        Handle, sys_chan_create, sys_channel_close, sys_recv, sys_send, sys_spawn, sys_wait,
        sys_write,
    };

    const READY: &[u8] = b"ready";

    fn say(msg: &[u8]) {
        let _ = sys_write(1, msg.as_ptr(), msg.len());
    }

    /// What one request produced.
    enum Outcome {
        Body { status: u16, body: Vec<u8> },
        Error(ErrorCode),
    }

    fn fetch(chan: Handle, id: u32, url: &str) -> Result<Outcome, i32> {
        let req = wire::encode(&Msg::Fetch {
            id,
            method: Method::Get,
            url: String::from(url),
            headers: Vec::new(),
        })
        .map_err(|_| 101)?;
        if sys_send(chan, &req) < 0 {
            return Err(102);
        }
        let mut buf = vec![0u8; 8192];
        let mut status = 0;
        let mut body = Vec::new();
        loop {
            let n = sys_recv(chan, &mut buf);
            if n <= 0 {
                return Err(103);
            }
            match wire::decode(&buf[..n as usize]).map_err(|_| 104)? {
                Msg::Head {
                    id: i, status: s, ..
                } if i == id => status = s,
                Msg::Body { id: i, data } if i == id => body.extend_from_slice(&data),
                Msg::End { id: i } if i == id => return Ok(Outcome::Body { status, body }),
                Msg::Error { id: i, code } if i == id => return Ok(Outcome::Error(code)),
                _ => return Err(105),
            }
        }
    }

    pub fn run() -> i32 {
        // The HTTPS fixture, as in phase4_https_fetch.
        let (local, remote) = match sys_chan_create() {
            Ok(pair) => pair,
            Err(_) => return 81,
        };
        let (_pid, worker) = match sys_spawn("worker", &[remote, Handle(0)]) {
            Ok(spawned) => spawned,
            Err(_) => return 82,
        };
        let mut ready = [0u8; 8];
        let n = sys_recv(local, &mut ready);
        if n != READY.len() as i64 || &ready[..READY.len()] != READY {
            return 83;
        }

        // The broker: handle 0 its channel to us, handle 1 the root dial cap.
        let (content, broker_end) = match sys_chan_create() {
            Ok(pair) => pair,
            Err(_) => return 84,
        };
        let (_bpid, broker) = match sys_spawn("fetch-broker", &[broker_end, Handle(0)]) {
            Ok(spawned) => spawned,
            Err(_) => return 85,
        };

        // A private address literal: refused by the broker's policy.
        match fetch(content, 1, "https://127.0.0.1:4433/") {
            Ok(Outcome::Error(ErrorCode::PolicyRefused)) => {}
            Ok(_) => return 86,
            Err(code) => return code,
        }
        // Cleartext: refused by policy.
        match fetch(content, 2, "http://phase4.test:4433/") {
            Ok(Outcome::Error(ErrorCode::PolicyRefused)) => {}
            Ok(_) => return 87,
            Err(code) => return code,
        }
        // A name that resolves to loopback, outside the fixture allowance:
        // the broker's narrowed cap is refused by the kernel's dial (5b).
        match fetch(content, 3, "https://rebind.test:4433/") {
            Ok(Outcome::Error(ErrorCode::PolicyRefused)) => {}
            Ok(_) => return 88,
            Err(code) => return code,
        }
        // The fixture, through the broker: TLS in the broker, the body here.
        match fetch(content, 4, "https://phase4.test:4433/") {
            Ok(Outcome::Body { status: 200, body }) if body == libneo::tls::PHASE4_TEST_BODY => {}
            Ok(Outcome::Body { .. }) => return 89,
            Ok(Outcome::Error(code)) => {
                say(b"[ neo-init ] fetch_broker fixture error code=");
                let c = [b'0' + code as u8, b'\n'];
                say(&c);
                return 90;
            }
            Err(code) => return code,
        }

        // Closing our end lets the broker exit.
        let _ = sys_channel_close(content);
        match sys_wait(broker, 0) {
            Ok(0) => {}
            _ => return 91,
        }
        match sys_wait(worker, 0) {
            Ok(0) => {}
            _ => return 92,
        }
        say(b"[ neo-init ] fetch_broker private_literal=refused cleartext=refused loopback_name=kernel_refused https_body=ok\n");
        0
    }
}

#[cfg(feature = "phase4-https-fetch")]
mod https_fetch_tests {
    use libneo::{
        AF_INET, EACCES, EAGAIN, EBADF, ENOSYS, Handle, NO_NET_DIAL_CAP, NetCandidate, NetDialCap,
        SOCK_DGRAM, SYS_NET_ACCEPT_TCP, SYS_NET_CONNECT_TCP, SYS_NET_LISTEN_TCP, SYS_NET_RESOLVE,
        SYS_SOCKET_BIND, SYS_SOCKET_CREATE, SYS_SOCKET_RECVFROM, SYS_SOCKET_SEND,
        SYS_SOCKET_SENDTO, SockAddr, SockAddrIn, raw3, raw4, raw6, sys_chan_create, sys_net_dial,
        sys_net_dial_cap_derive, sys_recv, sys_revoke, sys_spawn, sys_tcp_close, sys_wait,
        sys_write,
    };

    const READY: &[u8] = b"ready";

    pub fn run() -> i32 {
        let (local, remote) = match sys_chan_create() {
            Ok(pair) => pair,
            Err(_) => return 81,
        };
        // The HTTPS fixture is a ring-3 listener too: pass the root network
        // authority explicitly so feature-on SYS_NET_LISTEN_TCP is never
        // ambient. Child handle 0 remains READY; the cap is installed at 1.
        let (_pid, worker) = match sys_spawn("worker", &[remote, Handle(0)]) {
            Ok(spawned) => spawned,
            Err(_) => return 82,
        };

        let mut ready = [0u8; 8];
        let n = sys_recv(local, &mut ready);
        if n != READY.len() as i64 || &ready[..READY.len()] != READY {
            return 83;
        }

        // AAG negative controls for the complete legacy egress surface.  The
        // connect target is the live HTTPS fixture and would have succeeded
        // before this gate.  SENDTO uses a deliberately forged handle: an
        // ungated implementation returns EBADF, while the syscall-level gate
        // must reject it before handle lookup.  The remaining legacy roots are
        // pinned for the same reason, so reopening any one path breaks this
        // scenario rather than silently weakening confinement.
        if !legacy_egress_surface_closed() {
            return 94;
        }

        // Negative control: this exact call would have succeeded before the
        // kernel authority check existed.
        if sys_net_dial(
            NO_NET_DIAL_CAP,
            libneo::tls::PHASE4_TEST_HOST,
            libneo::tls::PHASE4_TEST_PORT,
        ) != Err(EACCES)
        {
            return 89;
        }

        let cap = match sys_net_dial_cap_derive(
            NetDialCap(Handle(0)),
            libneo::tls::PHASE4_TEST_HOST,
            libneo::tls::PHASE4_TEST_PORT,
        ) {
            Ok(cap) => cap,
            Err(_) => return 90,
        };
        if sys_net_dial(
            cap,
            libneo::tls::PHASE4_TEST_HOST,
            libneo::tls::PHASE4_TEST_PORT + 1,
        ) != Err(EACCES)
        {
            return 91;
        }

        // OT-61 rung 5b: the same scope with NON_PUBLIC narrowed away must not
        // reach the fixture, which resolves only to loopback (127.0.0.1, ::1).
        // `cap` keeps the right, so the dial below still works.
        #[cfg(feature = "net-dial-public-only")]
        {
            let narrow = match libneo::sys_derive(
                cap.0,
                libneo::CapRights::new(
                    libneo::meta_rights::USE | libneo::meta_rights::DERIVE,
                    libneo::net_dial_rights::DIAL,
                ),
            ) {
                Ok(h) => NetDialCap(h),
                Err(_) => return 95,
            };
            if sys_net_dial(
                narrow,
                libneo::tls::PHASE4_TEST_HOST,
                libneo::tls::PHASE4_TEST_PORT,
            ) != Err(EACCES)
            {
                return 96;
            }
            let _ = sys_revoke(narrow.0);
            let msg = b"[ neo-init ] netdial public_only narrowed_to_loopback=denied\n";
            let _ = sys_write(1, msg.as_ptr(), msg.len());
        }

        let socket = match dial_server(cap) {
            Ok(socket) => socket,
            Err(code) => return code,
        };
        let mut body = [0u8; 64];
        let n = match libneo::tls::https_get(
            socket,
            libneo::tls::PHASE4_TEST_HOST,
            "/",
            libneo::tls::PHASE4_TEST_ROOTS,
            &mut body,
        ) {
            Ok(n) => n,
            Err(err) => {
                let msg = b"[ neo-init ] phase4_https_fetch tls client error=";
                let _ = sys_write(1, msg.as_ptr(), msg.len());
                let code = [err.code(), b'\n'];
                let _ = sys_write(1, code.as_ptr(), code.len());
                let _ = sys_tcp_close(socket);
                return 85;
            }
        };
        let _ = sys_tcp_close(socket);
        if &body[..n] != libneo::tls::PHASE4_TEST_BODY {
            return 86;
        }

        if sys_revoke(cap.0) != 0 {
            return 92;
        }
        if sys_net_dial(
            cap,
            libneo::tls::PHASE4_TEST_HOST,
            libneo::tls::PHASE4_TEST_PORT,
        ) != Err(EACCES)
        {
            return 93;
        }

        match sys_wait(worker, 0) {
            Ok(0) => {
                let msg = b"[ neo-init ] phase4_https_fetch body ok\n";
                let _ = sys_write(1, msg.as_ptr(), msg.len());
                let proof = b"[ neo-init ] netdialcap no_cap=denied in_scope=ok out_of_scope=denied post_revoke=denied legacy_connect_tcp=denied legacy_sendto=denied legacy_egress=closed\n";
                let _ = sys_write(1, proof.as_ptr(), proof.len());
                0
            }
            Ok(_) => 87,
            Err(_) => 88,
        }
    }

    fn legacy_egress_surface_closed() -> bool {
        let addr = SockAddr::Inet(SockAddrIn::new_loopback(libneo::tls::PHASE4_TEST_PORT));
        let payload = b"capless-egress-probe";
        let name = libneo::tls::PHASE4_TEST_HOST.as_bytes();
        let mut candidates = [NetCandidate::default(); 1];
        let mut recv_buf = [0u8; 1];
        let mut recv_addr = SockAddr::Inet(SockAddrIn::new_loopback(1));
        let mut recv_addr_len = recv_addr.len();
        let forged = u32::MAX as u64;

        unsafe {
            raw3(
                SYS_NET_CONNECT_TCP,
                addr.as_ptr() as u64,
                addr.len() as u64,
                0,
            ) == ENOSYS
                && raw3(SYS_SOCKET_CREATE, AF_INET as u64, SOCK_DGRAM as u64, 0) == ENOSYS
                && raw3(
                    SYS_SOCKET_BIND,
                    forged,
                    addr.as_ptr() as u64,
                    addr.len() as u64,
                ) == ENOSYS
                && raw4(
                    SYS_SOCKET_SEND,
                    forged,
                    payload.as_ptr() as u64,
                    payload.len() as u64,
                    0,
                ) == ENOSYS
                && raw6(
                    SYS_SOCKET_SENDTO,
                    forged,
                    payload.as_ptr() as u64,
                    payload.len() as u64,
                    0,
                    addr.as_ptr() as u64,
                    addr.len() as u64,
                ) == ENOSYS
                && raw6(
                    SYS_SOCKET_RECVFROM,
                    forged,
                    recv_buf.as_mut_ptr() as u64,
                    recv_buf.len() as u64,
                    0,
                    recv_addr.as_mut_ptr() as u64,
                    (&mut recv_addr_len as *mut usize) as u64,
                ) == ENOSYS
                && raw4(
                    SYS_NET_LISTEN_TCP,
                    addr.as_ptr() as u64,
                    addr.len() as u64,
                    1,
                    forged,
                ) == EACCES
                && raw3(SYS_NET_ACCEPT_TCP, forged, 0, 0) == EBADF
                && raw6(
                    SYS_NET_RESOLVE,
                    name.as_ptr() as u64,
                    name.len() as u64,
                    AF_INET as u64,
                    candidates.as_mut_ptr() as u64,
                    candidates.len() as u64,
                    0,
                ) == ENOSYS
        }
    }

    fn dial_server(cap: NetDialCap) -> Result<libneo::TcpSocketHandle, i32> {
        for _ in 0..128 {
            match sys_net_dial(
                cap,
                libneo::tls::PHASE4_TEST_HOST,
                libneo::tls::PHASE4_TEST_PORT,
            ) {
                Ok(socket) => return Ok(socket),
                Err(EAGAIN) => core::hint::spin_loop(),
                Err(_) => return Err(84),
            }
        }
        Err(84)
    }
}

#[cfg(feature = "net-dial-cap-kill-switch")]
mod https_fetch_kill_switch_tests {
    use libneo::{
        EAGAIN, NO_NET_DIAL_CAP, sys_chan_create, sys_net_dial, sys_recv, sys_spawn, sys_tcp_close,
        sys_wait, sys_write,
    };

    const READY: &[u8] = b"ready";

    pub fn run() -> i32 {
        let (local, remote) = match sys_chan_create() {
            Ok(pair) => pair,
            Err(_) => return 1,
        };
        let (_pid, worker) = match sys_spawn("worker", &[remote]) {
            Ok(spawned) => spawned,
            Err(_) => return 2,
        };
        let mut ready = [0u8; 8];
        if sys_recv(local, &mut ready) != READY.len() as i64 {
            return 3;
        }
        let socket = loop {
            match sys_net_dial(
                NO_NET_DIAL_CAP,
                libneo::tls::PHASE4_TEST_HOST,
                libneo::tls::PHASE4_TEST_PORT,
            ) {
                Ok(socket) => break socket,
                Err(EAGAIN) => core::hint::spin_loop(),
                Err(_) => return 4,
            }
        };
        let mut body = [0u8; 64];
        let n = match libneo::tls::https_get(
            socket,
            libneo::tls::PHASE4_TEST_HOST,
            "/",
            libneo::tls::PHASE4_TEST_ROOTS,
            &mut body,
        ) {
            Ok(n) => n,
            Err(_) => {
                let _ = sys_tcp_close(socket);
                return 5;
            }
        };
        let _ = sys_tcp_close(socket);
        if &body[..n] != libneo::tls::PHASE4_TEST_BODY {
            return 6;
        }
        if sys_wait(worker, 0) != Ok(0) {
            return 7;
        }
        let proof =
            b"[ neo-init ] netdialcap kill_switch=off ambient_dial=ok equality=pre_feature\n";
        let _ = sys_write(1, proof.as_ptr(), proof.len());
        0
    }
}

/// Phase-B vault caller/driver (ADR-075). Spawns the vault process (the
/// neo-worker binary built with its `vault-phase-b` role), proves the live
/// process holds **no network capability** by inspecting its handle set, then
/// drives the full operation surface: unlock / store / retrieve-into-a-
/// least-rights-grant / lock, plus the wrong-password (single AUTH_FAILED
/// class), mutated-header (AEAD auth failure), and weak-parameter (§F5 refusal)
/// negative checks. Returns 0 only if every check passes; each failure returns a
/// distinct code so the QEMU serial FAIL line names exactly which invariant
/// broke (the discriminator bank).
#[cfg(feature = "vault-phase-b")]
mod vault_tests {
    use super::*;
    use libneo::{
        CapRights, Handle, ProcessHandle, channel_rights, meta_rights, sys_grant, sys_inspect,
    };
    use vault_core::protocol::{self, Header};

    const SECRET_ID_LEN: usize = 16;
    const RECV_BUF: usize = 512;
    /// The least-rights output grant: USE + SEND only.
    const SEND_ONLY: CapRights = CapRights::new(meta_rights::USE, channel_rights::SEND);

    pub fn run() -> i32 {
        // Spawn the vault with ONLY its operation channel inherited — the I-14
        // no-network allow-list is enforced at the spawn site: we hand it a
        // single channel handle and nothing else.
        let (op, op_worker) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 90,
        };
        let vault_handle = match sys_spawn("worker", &[op_worker]) {
            Ok((_pid, h)) => h,
            Err(_) => return 91,
        };

        // No teardown: when this driver exits, the kernel test harness turns the
        // exit code into a PASS/FAIL frame and halts the VM (debug_exit), tearing
        // down the still-blocked vault. A `sys_kill` here would hit the ADR-007
        // §Q3 limitation (kill only handles Ready / parked-in-IPC targets) and
        // panic on a vault parked in `sys_recv`.
        drive(op, vault_handle)
    }

    fn drive(op: Handle, vault_handle: ProcessHandle) -> i32 {
        // ── I-14(a): the live vault holds NO network capability ──────────────
        // Inspect before any operation, so no transient retrieve grant is present.
        let snap = match sys_inspect(vault_handle) {
            Ok(s) => s,
            Err(_) => return 10,
        };
        if snap.version != libneo::PROCESS_SNAPSHOT_V1_VERSION {
            return 11;
        }
        if snap.state != libneo::PROCESS_STATE_RUNNING {
            return 12;
        }
        // The containment proof: zero network handles in the live table.
        if snap.network_handle_count != 0 {
            return 13;
        }
        // And nothing but the operation channel — no room for any other cap.
        if snap.handle_count != 1 || snap.channel_handle_count != 1 {
            return 14;
        }

        // ── unlock (correct password) ────────────────────────────────────────
        let password = b"correct horse battery staple";
        if op_status(op, protocol::OP_UNLOCK, 0x1001, password) != Some(protocol::STATUS_OK) {
            return 20;
        }

        // ── store ────────────────────────────────────────────────────────────
        let secret = b"s3cr3t-value-42";
        let id_a = [0xA1u8; SECRET_ID_LEN];
        let mut store_payload = Vec::new();
        store_payload.extend_from_slice(&id_a);
        store_payload.extend_from_slice(secret);
        if op_status(op, protocol::OP_STORE, 0x1002, &store_payload) != Some(protocol::STATUS_OK) {
            return 30;
        }

        // ── retrieve into a caller-offered send-only least-rights grant ───────
        let r = retrieve_into_grant(op, 0x2001, &id_a, Some(secret));
        if r != 0 {
            return 40 + r;
        }

        // ── lock, then wrong password: one AUTH_FAILED class, no oracle ───────
        if op_status(op, protocol::OP_LOCK, 0x1004, &[]) != Some(protocol::STATUS_OK) {
            return 50;
        }
        if op_status(op, protocol::OP_UNLOCK, 0x1005, b"the WRONG password")
            != Some(protocol::STATUS_AUTH_FAILED)
        {
            return 60;
        }
        // A retrieve while locked must leak nothing (LOCKED class).
        if retrieve_expect(op, 0x2002, &id_a, protocol::STATUS_LOCKED) != 0 {
            return 61;
        }
        // Re-unlock correctly to continue the tamper/weak-params checks.
        if op_status(op, protocol::OP_UNLOCK, 0x1006, password) != Some(protocol::STATUS_OK) {
            return 62;
        }

        // ── mutated-header record fails authentication (§F4 live path) ────────
        let id_b = [0xB2u8; SECRET_ID_LEN];
        let mut b_payload = Vec::new();
        b_payload.extend_from_slice(&id_b);
        b_payload.extend_from_slice(b"aad-bound-secret");
        if op_status(op, protocol::OP_STORE, 0x1007, &b_payload) != Some(protocol::STATUS_OK) {
            return 70;
        }
        // Flip header byte 16 (memory_kib LSB — stays >= floor, so the AEAD tag
        // is what catches the tamper).
        let mut corrupt_payload = Vec::new();
        corrupt_payload.extend_from_slice(&id_b);
        corrupt_payload.extend_from_slice(&16u32.to_le_bytes());
        if op_status(
            op,
            protocol::OP_TEST_CORRUPT_HEADER,
            0x1008,
            &corrupt_payload,
        ) != Some(protocol::STATUS_OK)
        {
            return 71;
        }
        if retrieve_expect(op, 0x2003, &id_b, protocol::STATUS_AUTH_FAILED) != 0 {
            return 72;
        }

        // ── weak-parameter record is refused, never honored (§F5 live path) ───
        let id_c = [0xC3u8; SECRET_ID_LEN];
        if op_status(op, protocol::OP_TEST_INJECT_WEAK, 0x1009, &id_c) != Some(protocol::STATUS_OK)
        {
            return 80;
        }
        if retrieve_expect(op, 0x2004, &id_c, protocol::STATUS_WEAK_PARAMS) != 0 {
            return 81;
        }

        // ── containment still holds after the whole workload ──────────────────
        match sys_inspect(vault_handle) {
            Ok(s) if s.network_handle_count == 0 => {}
            _ => return 15,
        }
        0
    }

    /// Send a request frame (header + payload) on the operation channel.
    fn send_request(op: Handle, opcode: u16, rid: u64, payload: &[u8]) -> bool {
        let mut msg = Vec::with_capacity(protocol::HEADER_LEN + payload.len());
        msg.extend_from_slice(&Header::request(opcode, rid, payload.len() as u32).encode());
        msg.extend_from_slice(payload);
        sys_send(op, &msg) >= 0
    }

    /// Receive a response frame and return `(status, payload)` if it matches the
    /// expected opcode/request id.
    fn recv_response(op: Handle, opcode: u16, rid: u64) -> Option<(i32, Vec<u8>)> {
        let mut buf = [0u8; RECV_BUF];
        let n = sys_recv(op, &mut buf);
        if n < 0 {
            return None;
        }
        let n = n as usize;
        let h = Header::decode(&buf[..n])?;
        if h.kind != protocol::KIND_RESPONSE || h.opcode != opcode || h.request_id != rid {
            return None;
        }
        let plen = h.payload_len as usize;
        let start = protocol::HEADER_LEN;
        if start + plen > n {
            return None;
        }
        Some((h.status, buf[start..start + plen].to_vec()))
    }

    /// A simple request→status round-trip (unlock/store/lock/test verbs).
    fn op_status(op: Handle, opcode: u16, rid: u64, payload: &[u8]) -> Option<i32> {
        if !send_request(op, opcode, rid, payload) {
            return None;
        }
        recv_response(op, opcode, rid).map(|(status, _)| status)
    }

    /// Drive the retrieve cap-grant handshake: offer a **send-only** endpoint,
    /// receive the secret on the retained end, and verify it. When `expect` is
    /// `Some(secret)`, requires STATUS_OK and an exact plaintext match. Returns 0
    /// on success or a small nonzero discriminator.
    fn retrieve_into_grant(
        op: Handle,
        rid: u64,
        secret_id: &[u8; SECRET_ID_LEN],
        expect: Option<&[u8]>,
    ) -> i32 {
        if !send_request(op, protocol::OP_RETRIEVE, rid, secret_id) {
            return 1;
        }
        // One-shot output channel: keep the receive end; grant the peer end
        // offering only USE|SEND. The endpoint itself carries GRANT meta (fresh
        // from chan_create) so the grant is authorized, but the offered rights —
        // intersected with the vault's USE|SEND recv ceiling — make the
        // delivered cap send-only (no recv, re-derive, re-grant, or revoke).
        let (out_recv, out_peer) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 2,
        };
        if sys_grant(op, out_peer, SEND_ONLY) != 0 {
            return 4;
        }
        let mut secret_buf = [0u8; RECV_BUF];
        let sn = sys_recv(out_recv, &mut secret_buf);
        if sn < 0 {
            return 5;
        }
        let sn = sn as usize;
        let (status, resp) = match recv_response(op, protocol::OP_RETRIEVE, rid) {
            Some(r) => r,
            None => return 6,
        };
        match expect {
            Some(want) => {
                if status != protocol::STATUS_OK {
                    return 7;
                }
                if resp.len() != 4 {
                    return 8;
                }
                let written = u32::from_le_bytes([resp[0], resp[1], resp[2], resp[3]]) as usize;
                if written != want.len() || sn != want.len() || &secret_buf[..sn] != want {
                    return 9;
                }
                0
            }
            None => 0,
        }
    }

    /// Drive the retrieve handshake but require a specific FAILURE status and no
    /// leaked plaintext (the vault sends a single sentinel byte to unblock us).
    fn retrieve_expect(
        op: Handle,
        rid: u64,
        secret_id: &[u8; SECRET_ID_LEN],
        expected_status: i32,
    ) -> i32 {
        if !send_request(op, protocol::OP_RETRIEVE, rid, secret_id) {
            return 1;
        }
        let (out_recv, out_peer) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 2,
        };
        if sys_grant(op, out_peer, SEND_ONLY) != 0 {
            return 4;
        }
        let mut sink = [0u8; RECV_BUF];
        let sn = sys_recv(out_recv, &mut sink);
        if sn < 0 {
            return 5;
        }
        match recv_response(op, protocol::OP_RETRIEVE, rid) {
            Some((status, _)) if status == expected_status => 0,
            _ => 6,
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    sys_exit(255);
}

/// Atrium G1 compositor slice-1 user side: drives the shared-pixel-buffer
/// lifecycle (validation, create/query/map, commit gate, present, the
/// epoch fence with its bounded wait) and the atomic InputFocus transfer,
/// then — as its FINAL act — writes to its compositor-role hardware-RO
/// mapping and dies by #PF (the kernel harness passes the scenario from
/// the fault handler after asserting the composition pixels). Every early
/// exit code names the failing step; exiting 0 is itself a FAILURE (the
/// RO write did not fault).
#[cfg(feature = "atrium-input-routing")]
mod atrium_input_routing_tests {
    use libneo::{
        ATRIUM_FOCUS_RECEIVE, CapRights, EBADF, EPERM, INPUT_EVENT_V1_LEN, meta_rights, sys_derive,
        sys_input_focus_activate, sys_input_focus_create, sys_input_focus_mint_activator,
        sys_input_focus_recv, sys_input_focus_test_inject, sys_revoke,
    };

    pub fn run() -> i32 {
        let client = match sys_input_focus_create() {
            Ok(handle) => handle,
            Err(_) => return 1,
        };
        let compositor = match sys_input_focus_mint_activator(client) {
            Ok(handle) => handle,
            Err(_) => return 2,
        };

        // The inversion itself is armed: the client root cannot self-activate,
        // and the compositor cap cannot derive RECEIVE (or anything: no DERIVE).
        if sys_input_focus_activate(client) != EPERM {
            return 3;
        }
        if sys_derive(
            compositor,
            CapRights::new(meta_rights::USE, ATRIUM_FOCUS_RECEIVE),
        ) != Err(EPERM)
        {
            return 4;
        }
        if sys_input_focus_activate(compositor) != 0 {
            return 5;
        }
        if sys_input_focus_test_inject(0x04, 0x02) != 0 {
            return 6;
        }

        // Crown-jewel negative control: the compositor gets EPERM and the
        // sentinel output remains byte-identical (zero event-byte disclosure).
        let mut snoop = [0xA5; INPUT_EVENT_V1_LEN];
        if sys_input_focus_recv(compositor, &mut snoop) != EPERM {
            return 7;
        }
        if snoop != [0xA5; INPUT_EVENT_V1_LEN] {
            return 8;
        }

        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        if sys_input_focus_recv(client, &mut event) != 0 {
            return 9;
        }
        if event != [0x01, 0x01, 0x04, 0x00, 0x02, 0x00, 0x00, 0x00] {
            return 10;
        }

        // Dexter #185 item 3: the ACTIVATE sibling is a child of the RECEIVE
        // cap it was minted from, so revoking that cap ends it too. Before
        // the fix it had no parent and outlived the revoke (still activating).
        let owner = match sys_input_focus_create() {
            Ok(handle) => handle,
            Err(_) => return 11,
        };
        let minted = match sys_input_focus_mint_activator(owner) {
            Ok(handle) => handle,
            Err(_) => return 12,
        };
        if sys_revoke(owner) != 0 {
            return 13;
        }
        if sys_input_focus_activate(minted) != EBADF {
            return 14;
        }
        // And a mint from a revoked cap is refused.
        if sys_input_focus_mint_activator(owner).is_ok() {
            return 15;
        }
        0
    }
}

#[cfg(feature = "atrium-slice1")]
mod atrium_tests {
    use libneo::{
        ATRIUM_FOCUS_RECEIVE, ATRIUM_MAP_READ, ATRIUM_QUERY, BUFFER_DESC_V1_LEN, CapRights, EAGAIN,
        EINVAL, EPERM, ETIMEDOUT, FORMAT_XRGB8888_LE, PRESENT_RESULT_V1_LEN, encode_buffer_desc,
        meta_rights, sys_buffer_commit, sys_buffer_create, sys_buffer_map, sys_buffer_query,
        sys_derive, sys_fb_present, sys_input_focus_activate, sys_input_focus_create,
        sys_input_focus_deactivate, sys_input_focus_mint_activator, sys_input_focus_poll,
        sys_present_fence_wait, sys_write,
    };

    // MUST match kernel `test_harness::atrium_pixels`.
    const A_CORNER: u32 = 0x0011_2233;
    const A_FILL: u32 = 0x00FF_6A00;
    const B_FILL: u32 = 0x0000_CC44;
    const A_UNCOMMITTED: u32 = 0x00AB_CDEF;
    const A_DST: (usize, usize) = (100, 100);
    const B_DST: (usize, usize) = (300, 200);

    const OUTCOME_COMPOSED_TO_FB: u32 = 1;
    const OUTCOME_NO_DAMAGE: u32 = 2;

    fn u32_at(r: &[u8; PRESENT_RESULT_V1_LEN], i: usize) -> u32 {
        u32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]])
    }
    fn u64_at(r: &[u8; PRESENT_RESULT_V1_LEN], i: usize) -> u64 {
        let mut b = [0u8; 8];
        b.copy_from_slice(&r[i..i + 8]);
        u64::from_le_bytes(b)
    }
    fn present_id(r: &[u8; PRESENT_RESULT_V1_LEN]) -> u64 {
        u64_at(r, 0)
    }
    fn outcome(r: &[u8; PRESENT_RESULT_V1_LEN]) -> u32 {
        u32_at(r, 8)
    }
    fn submit_tick(r: &[u8; PRESENT_RESULT_V1_LEN]) -> u64 {
        u64_at(r, 16)
    }
    fn complete_tick(r: &[u8; PRESENT_RESULT_V1_LEN]) -> u64 {
        u64_at(r, 24)
    }

    pub fn run() -> i32 {
        // --- descriptor validation rejections (checked math, format) ---
        if sys_buffer_create(&encode_buffer_desc(0, 32, 256, FORMAT_XRGB8888_LE, 8192)).is_ok() {
            return 10;
        }
        if sys_buffer_create(&encode_buffer_desc(64, 32, 128, FORMAT_XRGB8888_LE, 4096)).is_ok() {
            return 11; // stride < width*4
        }
        if sys_buffer_create(&encode_buffer_desc(64, 32, 256, 0xDEAD, 8192)).is_ok() {
            return 12; // unsupported format
        }
        if sys_buffer_create(&encode_buffer_desc(64, 32, 256, FORMAT_XRGB8888_LE, 4096)).is_ok() {
            return 13; // bytes != stride*height
        }

        // --- buffer A: 64x32, stride 256 (exactly 2 pages) ---
        let desc_a = encode_buffer_desc(64, 32, 256, FORMAT_XRGB8888_LE, 8192);
        let a = match sys_buffer_create(&desc_a) {
            Ok(h) => h,
            Err(_) => return 14,
        };
        let mut q = [0u8; BUFFER_DESC_V1_LEN];
        if sys_buffer_query(a, &mut q) != 0 {
            return 15;
        }
        if q != desc_a {
            return 16; // immutable descriptor round-trip
        }
        let va = match sys_buffer_map(a, 0) {
            Ok(p) => p as *mut u32,
            Err(_) => return 17,
        };

        // Present with no commit: NO_DAMAGE, nothing blitted.
        if sys_fb_present(a, A_DST.0, A_DST.1) != 1 {
            return 18;
        }
        let mut r = [0u8; PRESENT_RESULT_V1_LEN];
        if sys_present_fence_wait(a, 1, &mut r, 1000) != 0 {
            return 19;
        }
        if outcome(&r) != OUTCOME_NO_DAMAGE {
            return 20;
        }

        // Draw the pattern, commit, present: COMPOSED_TO_FB.
        // stride 256 == width*4, so row-major u32 indexing is exact.
        unsafe {
            for px in 0..(64 * 32) {
                *va.add(px) = A_FILL;
            }
            *va = A_CORNER;
        }
        if sys_buffer_commit(a) != 0 {
            return 21;
        }
        if sys_fb_present(a, A_DST.0, A_DST.1) != 2 {
            return 22;
        }
        if sys_present_fence_wait(a, 2, &mut r, 1000) != 0 {
            return 23;
        }
        if outcome(&r) != OUTCOME_COMPOSED_TO_FB {
            return 24;
        }
        if present_id(&r) != 2 {
            return 25;
        }
        if complete_tick(&r) < submit_tick(&r) {
            return 26; // fence stamped after the blit, never before
        }

        // Fence bounded-wait semantics (ISC-20/39): epoch 0 invalid; a
        // not-yet-issued epoch is a real bounded wait — EAGAIN on the
        // non-blocking probe, ETIMEDOUT at the deadline; a stale epoch
        // returns immediately with the latest result.
        if sys_present_fence_wait(a, 0, &mut r, 0) != EINVAL {
            return 27;
        }
        if sys_present_fence_wait(a, 3, &mut r, 0) != EAGAIN {
            return 28;
        }
        if sys_present_fence_wait(a, 3, &mut r, 30) != ETIMEDOUT {
            return 29;
        }
        if sys_present_fence_wait(a, 1, &mut r, 0) != 0 {
            return 30;
        }
        if present_id(&r) != 2 {
            return 31; // stale-epoch wait sees the LATEST completed present
        }

        // --- buffer B: 32x16, stride 128 — second surface ---
        let desc_b = encode_buffer_desc(32, 16, 128, FORMAT_XRGB8888_LE, 2048);
        let b = match sys_buffer_create(&desc_b) {
            Ok(h) => h,
            Err(_) => return 32,
        };
        let vb = match sys_buffer_map(b, 0) {
            Ok(p) => p as *mut u32,
            Err(_) => return 33,
        };
        unsafe {
            for px in 0..(32 * 16) {
                *vb.add(px) = B_FILL;
            }
        }
        if sys_buffer_commit(b) != 0 {
            return 34;
        }
        if sys_fb_present(b, B_DST.0, B_DST.1) != 1 {
            let _ = sys_write(
                1,
                b"[ gpu-glass-slice6 ] EXIT=35 site=buffer-b-fb-present\n".as_ptr(),
                b"[ gpu-glass-slice6 ] EXIT=35 site=buffer-b-fb-present\n".len(),
            );
            return 35;
        }
        if sys_present_fence_wait(b, 1, &mut r, 1000) != 0 || outcome(&r) != OUTCOME_COMPOSED_TO_FB
        {
            return 36;
        }

        // --- the commit gate: uncommitted writes never reach the fb ---
        unsafe {
            *va = A_UNCOMMITTED;
        }
        if sys_fb_present(a, A_DST.0, A_DST.1) != 3 {
            return 37;
        }
        if sys_present_fence_wait(a, 3, &mut r, 1000) != 0 || outcome(&r) != OUTCOME_NO_DAMAGE {
            return 38;
        }

        // --- InputFocus: atomic transfer + rights gates ---
        let f1 = match sys_input_focus_create() {
            Ok(h) => h,
            Err(_) => return 40,
        };
        let f2 = match sys_input_focus_create() {
            Ok(h) => h,
            Err(_) => return 41,
        };
        let a1 = match sys_input_focus_mint_activator(f1) {
            Ok(h) => h,
            Err(_) => return 42,
        };
        let a2 = match sys_input_focus_mint_activator(f2) {
            Ok(h) => h,
            Err(_) => return 43,
        };
        if sys_input_focus_poll(f1) != EPERM {
            return 44; // nothing active yet
        }
        if sys_input_focus_activate(a1) != 0 {
            return 45;
        }
        if sys_input_focus_poll(f1) != 0 {
            return 46;
        }
        if sys_input_focus_poll(f2) != EPERM {
            return 47; // exactly one active route
        }
        if sys_input_focus_activate(a2) != 0 {
            return 48; // the atomic transfer
        }
        if sys_input_focus_poll(f1) != EPERM {
            return 49; // old holder lost the route at the swap
        }
        if sys_input_focus_poll(f2) != 0 {
            return 50;
        }
        // Client leaf: RECEIVE-only attenuation of f2's object.
        let leaf = match sys_derive(
            f2,
            CapRights {
                meta: meta_rights::USE,
                semantic: ATRIUM_FOCUS_RECEIVE,
            },
        ) {
            Ok(h) => h,
            Err(_) => return 51,
        };
        if sys_input_focus_poll(leaf) != 0 {
            return 52; // same object, still active
        }
        if sys_input_focus_activate(leaf) != EPERM {
            return 53; // the leaf has no ACTIVATE right
        }
        if sys_input_focus_deactivate(a1) != EAGAIN {
            return 54; // a stale activator must not clobber the active holder
        }
        if sys_input_focus_poll(f2) != 0 {
            return 55;
        }
        if sys_input_focus_deactivate(a2) != 0 {
            return 56;
        }
        if sys_input_focus_poll(f2) != EPERM {
            return 57;
        }

        // --- the compositor role: attenuated handle, hardware RO/NX ---
        let comp = match sys_derive(
            a,
            CapRights {
                meta: meta_rights::USE,
                semantic: ATRIUM_MAP_READ | ATRIUM_QUERY,
            },
        ) {
            Ok(h) => h,
            Err(_) => return 56,
        };
        if sys_buffer_map(comp, 0).is_ok() {
            return 57; // no MAP_WRITE on the compositor role
        }
        let ro = match sys_buffer_map(comp, 1) {
            Ok(p) => p as *mut u32,
            Err(_) => return 58,
        };

        // FINAL ACT: this write must die by #PF (hardware RO). The kernel
        // fault handler runs the pixel asserts and PASSes the scenario.
        unsafe {
            core::ptr::write_volatile(ro, 0xDEAD_BEEF);
        }
        // Surviving the write means the RO property is broken.
        59
    }
}

/// Atrium G1 slice-3 COMPOSITOR (the importer). neo-init drives this side:
/// spawn the client (neo-worker), receive its read-only buffer grant, wait for
/// the client to EXIT, then map the client's now-orphaned frames RO+NX,
/// composite from them (no UAF — the cross-process lifetime proof), and end by
/// writing to the RO import so the process dies by #PF. The kernel fault
/// handler runs the composition-pixel asserts and PASSes. MUST stay in sync
/// with kernel `test_harness::atrium_slice3_pixels` and neo-worker's client.
#[cfg(feature = "atrium-slice3")]
mod atrium_slice3_compositor_tests {
    use libneo::{
        ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, CapRights, EPIPE, Handle,
        PRESENT_RESULT_V1_LEN, meta_rights, sys_buffer_map, sys_chan_create, sys_fb_present,
        sys_present_fence_wait, sys_recv, sys_recv_grant, sys_spawn,
    };

    // MUST match kernel test_harness::atrium_slice3_pixels AND neo-worker's client.
    const FILL: u32 = 0x0033_9966;
    const CORNER: u32 = 0x0055_4433;
    const DST: (usize, usize) = (200, 150);

    const OUTCOME_COMPOSED_TO_FB: u32 = 1;

    fn outcome(r: &[u8; PRESENT_RESULT_V1_LEN]) -> u32 {
        u32::from_le_bytes([r[8], r[9], r[10], r[11]])
    }

    pub fn run() -> i32 {
        // 1. Create the surface channel; spawn the CLIENT (neo-worker) with the
        //    far end. The client creates the buffer, paints it, commits, grants
        //    us a read-only cap, then exits.
        let (init_end, worker_end) = match sys_chan_create() {
            Ok(pair) => pair,
            Err(_) => return 60,
        };
        if sys_spawn("worker", &[worker_end]).is_err() {
            return 61;
        }

        // 2. Receive the client's read-only buffer grant — the cross-process
        //    import. Compositor role: MAP_READ|QUERY|PRESENT, never MAP_WRITE/
        //    COMMIT. The grant rendezvous blocks until the client offers it.
        let comp_role = CapRights::new(
            meta_rights::USE,
            ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
        );
        let comp = match sys_recv_grant(init_end, comp_role) {
            Ok(h) => h,
            Err(_) => return 62,
        };

        // 3. Wait for the client to EXIT. Its exit closes the channel; our next
        //    recv observes EPIPE (peer death). This is the happens-before edge
        //    that makes the lifetime proof load-bearing: the client — and its
        //    RW mapping — are gone BEFORE we map/read/composite. Only the Arc
        //    pins held by our cap + our (about-to-be) mapping keep the frames
        //    alive. If cross-process lifetime were broken, the frames would be
        //    freed here and our composite would blit garbage (or fault).
        let mut byte = [0u8; 1];
        if sys_recv(init_end, &mut byte) != EPIPE {
            return 63;
        }

        // 4. C3 relocated-surface proof — the imported cap is a strict RO
        //    sibling and there is NO ambient buffer namespace:
        //    (a) a handle that is not a granted buffer cap cannot be mapped as a
        //        buffer (a channel handle → refused);
        if sys_buffer_map(init_end, 1).is_ok() {
            return 64;
        }
        //    (a') an unallocated index names no buffer (no third principal can
        //        reach a buffer it was never granted);
        if sys_buffer_map(Handle(0x7FFF_FFFF), 1).is_ok() {
            return 65;
        }
        //    (b) no write upgrade: the granted compositor cap has no MAP_WRITE,
        //        so a request to map it writable is refused.
        if sys_buffer_map(comp, 0).is_ok() {
            return 66;
        }

        // 5. Map the client's buffer RO+NX into THIS (compositor) process. This
        //    arms the kernel RO-write-fault probe on the returned VA. We are
        //    reading frames whose owning client has already exited.
        let ro = match sys_buffer_map(comp, 1) {
            Ok(p) => p as *const u32,
            Err(_) => return 67,
        };

        // 6. Zero-copy cross-process read: the client's pixels are visible
        //    through our RO import — after the client is gone (no UAF).
        unsafe {
            if core::ptr::read_volatile(ro) != CORNER {
                return 68;
            }
            if core::ptr::read_volatile(ro.add(1)) != FILL {
                return 69;
            }
        }

        // 7. Composite the client's buffer to the framebuffer (we hold PRESENT).
        //    The client set the commit gate before exiting; the blit reads the
        //    surviving frames. The fence completes strictly AFTER the blit.
        let epoch = sys_fb_present(comp, DST.0, DST.1);
        if epoch < 1 {
            return 70;
        }
        let mut r = [0u8; PRESENT_RESULT_V1_LEN];
        if sys_present_fence_wait(comp, epoch as u64, &mut r, 1000) != 0 {
            return 71;
        }
        if outcome(&r) != OUTCOME_COMPOSED_TO_FB {
            return 72;
        }

        // 8. FINAL ACT (C3 proof 1): a compositor write to the RO import must
        //    die by #PF. The kernel fault handler verifies the composition
        //    pixels (the client's pattern reached the fb AFTER the client
        //    exited) and PASSes. Surviving this write means the hardware-RO
        //    property is broken.
        unsafe {
            core::ptr::write_volatile(ro as *mut u32, 0xDEAD_BEEF);
        }
        73
    }
}

#[cfg(any(
    feature = "atrium-interactive",
    feature = "atrium-interactive-baremetal"
))]
mod atrium_interactive_compositor_tests {
    #[cfg(feature = "atrium-interactive")]
    use libneo::sys_input_focus_test_inject;
    use libneo::{
        ATRIUM_FOCUS_ACTIVATE, ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, CapRights, EPERM,
        INPUT_EVENT_V1_LEN, PRESENT_RESULT_V1_LEN, meta_rights, sys_buffer_map, sys_chan_create,
        sys_fb_present, sys_input_focus_activate, sys_input_focus_recv, sys_present_fence_wait,
        sys_recv, sys_recv_grant, sys_send, sys_spawn, sys_write,
    };
    const DST: (usize, usize) = (200, 150);
    fn present(buffer: libneo::Handle) -> bool {
        let epoch = sys_fb_present(buffer, DST.0, DST.1);
        if epoch < 1 {
            return false;
        }
        let mut result = [0u8; PRESENT_RESULT_V1_LEN];
        sys_present_fence_wait(buffer, epoch as u64, &mut result, 1000) == 0
            && u32::from_le_bytes([result[8], result[9], result[10], result[11]]) == 1
    }
    pub fn run() -> i32 {
        let (compositor, client) = match sys_chan_create() {
            Ok(pair) => pair,
            Err(_) => return 60,
        };
        if sys_spawn("worker", &[client]).is_err() {
            return 61;
        }
        let buffer_rights = CapRights::new(
            meta_rights::USE,
            ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
        );
        let buffer = match sys_recv_grant(compositor, buffer_rights) {
            Ok(h) => h,
            Err(_) => return 62,
        };
        let activate_rights = CapRights::new(meta_rights::USE, ATRIUM_FOCUS_ACTIVATE);
        let activator = match sys_recv_grant(compositor, activate_rights) {
            Ok(h) => h,
            Err(_) => return 63,
        };
        if sys_buffer_map(buffer, 0).is_ok() || sys_buffer_map(buffer, 1).is_err() {
            return 64;
        }
        if !present(buffer) {
            return 65;
        }
        if sys_input_focus_activate(activator) != 0 {
            return 66;
        }
        let mut snoop = [0xA5u8; INPUT_EVENT_V1_LEN];
        if sys_input_focus_recv(activator, &mut snoop) != EPERM
            || snoop != [0xA5; INPUT_EVENT_V1_LEN]
        {
            return 67;
        }
        let route = b"SLICE5 route focus=active delivered=pending compositor_event_bytes=0\n";
        let _ = sys_write(1, route.as_ptr(), route.len());
        #[cfg(feature = "atrium-interactive")]
        if sys_input_focus_test_inject(0x04, 0) != 0 {
            return 68;
        }
        if sys_send(compositor, b"K") != 1 {
            return 69;
        }
        let mut done = [0u8; 1];
        if sys_recv(compositor, &mut done) != 1 || done[0] != b'D' {
            return 70;
        }
        if !present(buffer) {
            return 71;
        }
        let present = b"SLICE5 present epoch=2 outcome=1 blit_complete=1 damage=8,8,8,16\n";
        let _ = sys_write(1, present.as_ptr(), present.len());
        let reply =
            b"SLICE5 REPLY: a focus=active commit_epoch=2 damage=8,8,8,16 present=complete\n";
        let _ = sys_write(1, reply.as_ptr(), reply.len());
        0
    }
}

/// OT-93, the `kill_ready_in_kernel` scenario. Init sends the worker "go" and
/// parks in a recv; the worker yields first, so init is parked, then sends, which hands init the
/// CPU and leaves the worker Ready inside its send (the kernel's hold keeps it
/// there). Init kills it. The kernel checks the victim's pmap is freed and
/// every frame came back. Returns 0, or the step that failed.
#[cfg(feature = "kill-ready-in-kernel-test")]
mod kill_ready_in_kernel_tests {
    use super::*;
    use libneo::{sys_kill, sys_wait};

    pub fn run() -> i32 {
        let (mine, theirs) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 1,
        };
        let ph = match sys_spawn("worker", &[theirs]) {
            Ok((_, ph)) => ph,
            Err(_) => return 2,
        };
        // The worker takes "go", then yields while init parks in the recv
        // below, so its report hands init the CPU.
        if sys_send(mine, b"go") < 0 {
            return 5;
        }
        let mut b = [0u8; 8];
        if sys_recv(mine, &mut b) < 1 {
            return 3;
        }
        if sys_kill(ph) != 0 {
            return 4;
        }
        // The kill's exit status is not a natural one; only that it ended.
        let _ = sys_wait(ph, 0);
        let line = b"[ kill-ready ] killed the worker inside its send\n";
        sys_write(1, line.as_ptr(), line.len());
        0
    }
}

/// OT-61 C2.2b, the `obj_quota` scenario: a per-account budget for kernel-heap
/// objects (I-18). A 64 KiB child's channel loop and a 512 KiB child's spawn
/// loop each trip at their own object limit, a sibling (init) still creates
/// channels meanwhile, and once every process is gone init's object count is
/// back where it started. Returns 0, or the step that failed.
#[cfg(feature = "obj-quota-test")]
mod obj_quota_tests {
    use super::*;
    use libneo::{
        Handle, ProcessHandle, sys_channel_close, sys_mem_info_v2, sys_spawn_budgeted2, sys_wait,
    };

    const KIB: u64 = 1 << 10;

    fn say(line: &[u8]) {
        sys_write(1, line.as_ptr(), line.len());
    }

    fn obj_used() -> Option<u64> {
        sys_mem_info_v2().ok().map(|i| i.obj_used_bytes)
    }

    /// Spawn a role; returns init's end, the end the child inherited (init's
    /// copy of it, closed once the child is gone) and the process handle.
    fn child(role: u8, obj_budget: u64) -> Result<(Handle, Handle, ProcessHandle), i32> {
        let (mine, theirs) = sys_chan_create().map_err(|_| 1)?;
        let (_, ph) = sys_spawn_budgeted2("worker", &[theirs], 0, obj_budget).map_err(|_| 2)?;
        if sys_send(mine, &[role]) < 0 {
            return Err(3);
        }
        Ok((mine, theirs, ph))
    }

    fn verdict(ch: Handle) -> i32 {
        let mut b = [0u8; 8];
        match sys_recv(ch, &mut b) {
            n if n >= 1 => i32::from(b[0]),
            _ => 99,
        }
    }

    pub fn run() -> i32 {
        let Some(base) = obj_used() else {
            return 1;
        };
        // An object budget of 0 is refused (no budget is not a budget of 0).
        match sys_spawn_budgeted2("worker", &[], 0, 0) {
            Err(e) if e == libneo::EINVAL => {}
            _ => return 2,
        }

        // Role 1: the channel loop trips at the child's 64 KiB.
        let (r1, r1_theirs, r1_ph) = match child(1, 64 * KIB) {
            Ok(c) => c,
            Err(d) => return 100 + d,
        };
        match verdict(r1) {
            0 => say(b"[ obj-quota ] 64 KiB budget: channels refused with ENOMEM at the limit\n"),
            v => return 110 + v,
        }
        // The child holds its whole budget; init, beside it, still creates
        // and closes a channel.
        match sys_chan_create() {
            Ok((a, b)) => {
                let _ = sys_channel_close(a);
                let _ = sys_channel_close(b);
            }
            Err(_) => return 120,
        }
        if sys_send(r1, b"go") < 0 || sys_wait(r1_ph, 0) != Ok(0) {
            return 130;
        }
        say(b"[ obj-quota ] a neighbour created a channel while the budget was full\n");

        // Role 2: the spawn loop trips at the child's 512 KiB.
        let (r2, r2_theirs, r2_ph) = match child(2, 512 * KIB) {
            Ok(c) => c,
            Err(d) => return 200 + d,
        };
        match verdict(r2) {
            0 => say(b"[ obj-quota ] 512 KiB budget: spawns refused with EAGAIN at the limit\n"),
            v => return 210 + v,
        }
        if sys_wait(r2_ph, 0) != Ok(0) {
            return 220;
        }

        // Role 4 (Muse #163 item 1): buffer maps trip at the child's 64 KiB,
        // each pin charged. Its charges go when it exits: the kernel's root
        // check below counts them.
        let (r4, r4_theirs, r4_ph) = match child(4, 64 * KIB) {
            Ok(c) => c,
            Err(d) => return 400 + d,
        };
        match verdict(r4) {
            0 => {
                say(b"[ obj-quota ] 64 KiB budget: buffer maps refused with ENOMEM at the limit\n")
            }
            v => return 410 + v,
        }
        if sys_wait(r4_ph, 0) != Ok(0) {
            return 420;
        }
        for h in [r4, r4_theirs] {
            let _ = sys_channel_close(h);
        }
        for h in [r1, r1_theirs, r2, r2_theirs] {
            let _ = sys_channel_close(h);
        }

        // The kernel checks the root account's count once every process is
        // gone (init keeps its two process handles, so its own count can't
        // come back exactly while it runs).
        say(b"[ obj-quota ] PASS: every trip was local\n");
        0
    }
}

/// The `user_copy_bounds` scenario: syscalls that copy a caller-chosen length
/// into the kernel heap refuse an oversized one before copying, or fail with
/// `ENOMEM`, never panic the kernel. Returns 0, or the number of the first
/// check that failed.
#[cfg(feature = "user-copy-bounds-test")]
mod user_copy_bounds_tests {
    use super::*;
    use libneo::{
        EBADF, EINVAL, ENOMEM, EPERM, SYS_OPEN, SYS_SPAWN, TENSOR_DTYPE_U8, TENSOR_MAX_RANK,
        TensorCreateDesc, raw3, raw4, sys_tensor_create,
    };

    const SYS_TENSOR_WRITE: u64 = 0x104;
    const MIB: u64 = 1024 * 1024;
    /// A length far past any heap and any tensor (the user half admits it).
    const HUGE: u64 = 1024 * MIB;

    static BUF: [u8; 4096] = [0u8; 4096];

    fn say(line: &[u8]) {
        sys_write(1, line.as_ptr(), line.len());
    }

    fn check(code: i32, ok: bool) -> Result<(), i32> {
        if ok { Ok(()) } else { Err(code) }
    }

    fn u8_desc(len: u64, data: &[u8]) -> TensorCreateDesc {
        let mut dims = [0u64; TENSOR_MAX_RANK];
        dims[0] = len;
        TensorCreateDesc {
            rank: 1,
            dtype: TENSOR_DTYPE_U8,
            mutable: 1,
            _pad: 0,
            dims,
            data_ptr: data.as_ptr() as u64,
            data_len: len,
        }
    }

    fn checks() -> Result<(), i32> {
        let buf = BUF.as_ptr() as u64;
        // A 64-byte mutable tensor, the write target.
        let t = sys_tensor_create(&u8_desc(64, &BUF[..64])).map_err(|_| 1)?;
        let h = t.0 as u64;
        // SAFETY (all raw calls): the kernel range-checks every pointer; none
        // is dereferenced here.
        unsafe {
            // Tensor write: past every tensor, past this one, and on a bad handle.
            check(2, raw4(SYS_TENSOR_WRITE, h, 0, buf, HUGE) == EINVAL)?;
            check(3, raw4(SYS_TENSOR_WRITE, h, 0, buf, 32 * MIB) == EINVAL)?;
            check(4, raw4(SYS_TENSOR_WRITE, h, 32, buf, 64) == EINVAL)?;
            check(5, raw4(SYS_TENSOR_WRITE, 0xFFFF, 0, buf, 32 * MIB) == EBADF)?;
            // The positive control: an in-range write still works.
            check(6, raw4(SYS_TENSOR_WRITE, h, 0, buf, 64) == 0)?;
        }
        // Tensor create: a shape past the tensor limit is EINVAL; one under it
        // but larger than the kernel heap fails closed with ENOMEM.
        check(
            7,
            sys_tensor_create(&u8_desc(128 * MIB, &BUF)).err() == Some(EINVAL),
        )?;
        check(
            8,
            sys_tensor_create(&u8_desc(32 * MIB, &BUF)).err() == Some(ENOMEM),
        )?;
        unsafe {
            // Two other copy sites with a huge length: a path and a spawn name.
            check(9, raw4(SYS_OPEN, buf, HUGE, 0, 1) == EINVAL)?;
            check(10, raw4(SYS_SPAWN, buf, HUGE, 0, 0) == EINVAL)?;
            // The kernel is still serving syscalls normally.
            check(11, raw3(SYS_TENSOR_WRITE, h, 0, 0) == 0)?;
        }
        // Jarvis #153: an immutable tensor refuses a write with EPERM before
        // anything is copied, an oversized one included (it used to copy the
        // 32 MiB first and fail with ENOMEM).
        let mut frozen = u8_desc(64, &BUF[..64]);
        frozen.mutable = 0;
        let f = sys_tensor_create(&frozen).map_err(|_| 12)?.0 as u64;
        unsafe {
            check(13, raw4(SYS_TENSOR_WRITE, f, 0, buf, 32 * MIB) == EPERM)?;
            check(14, raw4(SYS_TENSOR_WRITE, f, 0, buf, 64) == EPERM)?;
        }
        Ok(())
    }

    pub fn run() -> i32 {
        match checks() {
            Ok(()) => {
                say(b"[ user-copy-bounds ] PASS: every oversized copy refused, the kernel lives\n");
                0
            }
            Err(code) => {
                say(b"[ user-copy-bounds ] FAIL\n");
                code
            }
        }
    }
}

/// OT-91, the `exit_codes_bounded` scenario: a reaped process's exit code is
/// kept only while someone can claim it. Returns 0, or the first failed step.
/// The kernel then checks the exit-code and claims maps are back to their
/// size before neo-init ran.
#[cfg(feature = "exit-codes-test")]
mod exit_codes_tests {
    use super::*;
    use libneo::{
        Handle, PROCESS_STATE_EXITED, ProcessHandle, sys_chan_create, sys_inspect, sys_kill,
        sys_revoke, sys_spawn, sys_wait,
    };

    fn say(line: &[u8]) {
        sys_write(1, line.as_ptr(), line.len());
    }

    /// Spawn a worker; its channel end and process handle.
    fn worker() -> Result<(Handle, ProcessHandle), i32> {
        let (mine, theirs) = sys_chan_create().map_err(|_| 100)?;
        let (_, ph) = sys_spawn("worker", &[theirs]).map_err(|_| 101)?;
        Ok((mine, ph))
    }

    /// `ph` has exited with code 7 and the code is kept.
    fn exited(ph: ProcessHandle) -> bool {
        for _ in 0..1_000_000 {
            match sys_inspect(ph) {
                Ok(s) if s.state == PROCESS_STATE_EXITED => return s.exit_code == 7,
                Ok(_) => libneo::sys_sched_yield(),
                Err(_) => return false,
            }
        }
        false
    }

    /// `ph` has been reaped (only its kept code is left: no threads) with
    /// code 7.
    fn reaped(ph: ProcessHandle) -> bool {
        for _ in 0..1_000_000 {
            match sys_inspect(ph) {
                Ok(s) if s.state == PROCESS_STATE_EXITED && s.thread_count == 0 => {
                    return s.exit_code == 7;
                }
                Ok(_) => libneo::sys_sched_yield(),
                Err(_) => return false,
            }
        }
        false
    }

    fn steps() -> Result<(), i32> {
        // 1. A parent that never waits on 15 of its 16 children.
        let (ch, p) = worker()?;
        if sys_send(ch, b"P") < 0 {
            return Err(1);
        }
        match sys_wait(p, 0) {
            Ok(0) => {}
            Ok(code) => return Err(10 + code),
            Err(_) => return Err(2),
        }
        // 2. A handle revoked after its process exited drops the kept code.
        let (ch, c) = worker()?;
        if sys_send(ch, b"C") < 0 || !exited(c) {
            return Err(3);
        }
        if sys_revoke(Handle(c.0)) != 0 {
            return Err(4);
        }
        // 3. A handle revoked before its process exits: the code is never kept.
        let (ch, c) = worker()?;
        if sys_revoke(Handle(c.0)) != 0 {
            return Err(5);
        }
        if sys_send(ch, b"C") < 0 {
            return Err(6);
        }
        // 4. The control: a held handle still claims its code.
        let (ch, c) = worker()?;
        if sys_send(ch, b"C") < 0 || !exited(c) {
            return Err(7);
        }
        match sys_wait(c, 0) {
            Ok(7) => {}
            _ => return Err(8),
        }
        // 5. #171 F1: a wait the handle authorized keeps its claim while the
        // handle is revoked under it (the kernel revokes it between the
        // lookup and the read, for this test flag), and still gets the code.
        // The child is reaped first, so the code the wait reads is the kept
        // one, which a lost claim drops.
        const WAIT_TEST_REVOKE_AFTER_LOOKUP: u64 = 1 << 62;
        let (ch, c) = worker()?;
        if sys_send(ch, b"C") < 0 || !reaped(c) {
            return Err(9);
        }
        match sys_wait(c, WAIT_TEST_REVOKE_AFTER_LOOKUP) {
            Ok(7) => {}
            Ok(code) => return Err(40 + code),
            Err(_) => return Err(30),
        }
        // The handle is gone, so the claim left with the wait.
        if sys_inspect(c).is_ok() {
            return Err(31);
        }
        // 6. The #238 bounce: a waiter killed while parked in `sys_wait` is
        // freed raw, so it must have parked owning no claim. The worker
        // parks on a child that lives until the worker's teardown; init
        // kills the worker. The kernel's map check after neo-init catches a
        // leaked claim (the child's pid would stay in both maps).
        let (ch, w) = worker()?;
        if sys_send(ch, b"W") < 0 {
            return Err(50);
        }
        let mut ready = [0u8; 4];
        if sys_recv(ch, &mut ready) < 1 || ready[0] != b'R' {
            return Err(51);
        }
        // The worker parks right after it reports.
        for _ in 0..10_000 {
            libneo::sys_sched_yield();
        }
        if sys_kill(w) != 0 {
            return Err(52);
        }
        // Until the worker is gone; a killed process has no exit code to
        // report, so the wait's result is not checked.
        let _ = sys_wait(w, 0);
        Ok(())
    }

    pub fn run() -> i32 {
        match steps() {
            Ok(()) => {
                say(b"[ exit-codes ] PASS: unclaimable codes dropped, a held handle claims its code\n");
                0
            }
            Err(code) => {
                say(b"[ exit-codes ] FAIL\n");
                code
            }
        }
    }
}

/// Dexter #187 item 3a, the `mount_authority` scenario. A worker with no
/// caps opens `/` asking for `MOUNT`; its `SYS_UMOUNT` of `neofs` and its
/// `SYS_MOUNT` of device 0 are both `EPERM`, and `neofs` is still mounted
/// after it. neo-init, which holds the kernel's `MountAuthority`, then
/// unmounts `neofs` and mounts it again. Returns 0, or the first failed step.
#[cfg(feature = "mount-authority-test")]
mod mount_authority_tests {
    use super::*;
    use libneo::{
        FileRights, MOUNT_FS_TYPE_NEOFS_DEVICE0_RO, OPEN_ROOT_TEMPORARY, encode_mount_request_v1,
        sys_chan_create, sys_close, sys_mount, sys_open, sys_spawn, sys_umount, sys_wait,
    };

    fn say(line: &[u8]) {
        sys_write(1, line.as_ptr(), line.len());
    }

    /// `/neofs` opens as a directory (the volume is mounted).
    fn neofs_mounted() -> bool {
        match sys_open(
            "/neofs",
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        ) {
            Ok(h) => {
                let _ = sys_close(h);
                true
            }
            Err(_) => false,
        }
    }

    fn steps() -> Result<(), i32> {
        if !neofs_mounted() {
            return Err(1);
        }
        // 1. The worker, without the authority: its exit code is its result.
        let (mine, theirs) = sys_chan_create().map_err(|_| 2)?;
        let (_, worker) = sys_spawn("worker", &[theirs]).map_err(|_| 3)?;
        if sys_send(mine, b"M") < 0 {
            return Err(4);
        }
        match sys_wait(worker, 0) {
            Ok(0) => {}
            Ok(code) => return Err(100 + code),
            Err(_) => return Err(5),
        }
        if !neofs_mounted() {
            return Err(6);
        }
        say(b"[ mount-authority ] no authority: umount and mount EPERM, neofs still mounted\n");
        // 2. neo-init, the holder: unmount `neofs`, then mount it again.
        let root = sys_open(
            "/",
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::MOUNT),
        )
        .map_err(|_| 7)?;
        if sys_umount(root, "neofs", 0) != 0 {
            return Err(8);
        }
        if neofs_mounted() {
            return Err(9);
        }
        let req = encode_mount_request_v1(MOUNT_FS_TYPE_NEOFS_DEVICE0_RO, "device0", "neofs");
        if sys_mount(root, &req, 0) != 0 {
            return Err(10);
        }
        let _ = sys_close(root);
        if !neofs_mounted() {
            return Err(11);
        }
        say(b"[ mount-authority ] the holder unmounted and mounted neofs\n");
        Ok(())
    }

    pub fn run() -> i32 {
        match steps() {
            Ok(()) => {
                say(b"[ mount-authority ] PASS\n");
                0
            }
            Err(code) => {
                say(b"[ mount-authority ] FAIL\n");
                code
            }
        }
    }
}

/// The `socket_teardown` scenario: a socket, TCP listener or TCP connection
/// is closed when its last handle leaves, whichever way it leaves. Each leg
/// takes the same three addresses (UDP 127.0.0.1:7001, a listener on
/// 127.0.0.1:7002, a connection from local port 7003 to 127.0.0.1:7009) and
/// then takes them again, which fails with `EBUSY` while anything still holds
/// them. Returns 0, or the number of the first check that failed.
#[cfg(feature = "socket-teardown-test")]
mod socket_teardown_tests {
    use super::*;
    use libneo::{
        AF_INET, CapRights, EBUSY, Handle, ProcessHandle, SOCK_DGRAM, SYS_NET_CONNECT_TCP,
        SockAddr, SockAddrIn, SocketHandle, TcpListenerHandle, TcpSocketHandle, meta_rights, raw3,
        sys_derive, sys_kill, sys_net_listen_tcp, sys_revoke, sys_socket_bind_in, sys_socket_close,
        sys_socket_create, sys_tcp_close, sys_wait,
    };

    fn say(line: &[u8]) {
        sys_write(1, line.as_ptr(), line.len());
    }

    fn udp_bind() -> Result<SocketHandle, i64> {
        let s = sys_socket_create(AF_INET, SOCK_DGRAM)?;
        match sys_socket_bind_in(s, &SockAddrIn::new_loopback(7001)) {
            0 => Ok(s),
            e => {
                let _ = sys_socket_close(s);
                Err(e)
            }
        }
    }

    fn listen() -> Result<TcpListenerHandle, i64> {
        sys_net_listen_tcp(&SockAddr::Inet(SockAddrIn::new_loopback(7002)), 4)
    }

    fn connect() -> Result<TcpSocketHandle, i64> {
        let remote = SockAddr::Inet(SockAddrIn::new_loopback(7009));
        // SAFETY: SYS_NET_CONNECT_TCP reads `len` bytes of address at `ptr`.
        let r = unsafe {
            raw3(
                SYS_NET_CONNECT_TCP,
                remote.as_ptr() as u64,
                remote.len() as u64,
                7003,
            )
        };
        if r < 0 {
            Err(r)
        } else {
            Ok(TcpSocketHandle(r as u32))
        }
    }

    fn close_all(u: SocketHandle, l: TcpListenerHandle, c: TcpSocketHandle) -> bool {
        sys_socket_close(u) == 0
            && sys_socket_close(SocketHandle(l.0)) == 0
            && sys_tcp_close(c) == 0
    }

    /// Take all three addresses (they must be free), then close them.
    fn take_all_and_close() -> Result<(), i32> {
        let u = udp_bind().map_err(|_| 1)?;
        let l = listen().map_err(|_| 2)?;
        let c = connect().map_err(|_| 3)?;
        if !close_all(u, l, c) {
            return Err(4);
        }
        Ok(())
    }

    /// Spawn a worker in `role`; it reports 0 once it holds the three.
    fn child(role: u8) -> Result<ProcessHandle, i32> {
        let (mine, theirs) = sys_chan_create().map_err(|_| 1)?;
        let (_, ph) = sys_spawn("worker", &[theirs]).map_err(|_| 2)?;
        if sys_send(mine, &[role]) < 0 {
            return Err(3);
        }
        let mut b = [0u8; 8];
        match sys_recv(mine, &mut b) {
            n if n >= 1 && b[0] == 0 => Ok(ph),
            _ => Err(4),
        }
    }

    /// Revoking a derived copy leaves the original holding the address
    /// (`retake` is `EBUSY`); revoking the original frees it.
    fn revoke_leg(original: Handle, retake: impl Fn() -> i64) -> Result<(), i32> {
        let copy = sys_derive(
            original,
            CapRights::new(meta_rights::USE | meta_rights::REVOKE, 0),
        )
        .map_err(|_| 1)?;
        if sys_revoke(copy) != 0 {
            return Err(2);
        }
        if retake() != EBUSY {
            return Err(3);
        }
        if sys_revoke(original) != 0 {
            return Err(4);
        }
        Ok(())
    }

    pub fn run() -> i32 {
        // Control: the addresses are free, a second take while they are held
        // is EBUSY (what every leg below relies on), and a close frees them.
        let (Ok(u), Ok(l), Ok(c)) = (udp_bind(), listen(), connect()) else {
            return 10;
        };
        if udp_bind().err() != Some(EBUSY)
            || listen().err() != Some(EBUSY)
            || connect().err() != Some(EBUSY)
        {
            return 11;
        }
        if !close_all(u, l, c) {
            return 12;
        }
        if let Err(e) = take_all_and_close() {
            return 20 + e;
        }
        say(b"[ socket-teardown ] control: a held address is EBUSY, a close frees it\n");

        // Exit: the worker takes the three and exits without closing them.
        let ph = match child(1) {
            Ok(ph) => ph,
            Err(e) => return 100 + e,
        };
        if sys_wait(ph, 0) != Ok(0) {
            return 110;
        }
        if let Err(e) = take_all_and_close() {
            return 120 + e;
        }
        say(b"[ socket-teardown ] exit: the child's UDP, listener and connection are freed\n");

        // Kill: the worker takes the three, takes "go" and parks in a recv;
        // they are held (EBUSY) when init kills it. The kill lands on a parked
        // thread, as the IPC kill scenarios do: a thread killed while Ready
        // inside a syscall leaks what that syscall holds (reported apart).
        let (mine, theirs) = match sys_chan_create() {
            Ok(p) => p,
            Err(_) => return 200,
        };
        let ph = match sys_spawn("worker", &[theirs]) {
            Ok((_, ph)) => ph,
            Err(_) => return 201,
        };
        if sys_send(mine, &[2]) < 0 || sys_send(mine, b"go") < 0 {
            return 202;
        }
        if udp_bind().err() != Some(EBUSY)
            || listen().err() != Some(EBUSY)
            || connect().err() != Some(EBUSY)
        {
            return 203;
        }
        if sys_kill(ph) != 0 {
            return 210;
        }
        if let Err(e) = take_all_and_close() {
            return 220 + e;
        }
        say(b"[ socket-teardown ] kill: the child's UDP, listener and connection are freed\n");

        // Revoke, per kind: the last handle, not the first, closes it.
        let Ok(u) = udp_bind() else {
            return 300;
        };
        let retake_udp = || match udp_bind() {
            Ok(s) => {
                let _ = sys_socket_close(s);
                0
            }
            Err(e) => e,
        };
        if let Err(e) = revoke_leg(Handle(u.0), retake_udp) {
            return 310 + e;
        }
        let Ok(l) = listen() else {
            return 320;
        };
        let retake_listener = || match listen() {
            Ok(h) => {
                let _ = sys_socket_close(SocketHandle(h.0));
                0
            }
            Err(e) => e,
        };
        if let Err(e) = revoke_leg(Handle(l.0), retake_listener) {
            return 330 + e;
        }
        let Ok(c) = connect() else {
            return 340;
        };
        let retake_connection = || match connect() {
            Ok(h) => {
                let _ = sys_tcp_close(h);
                0
            }
            Err(e) => e,
        };
        if let Err(e) = revoke_leg(Handle(c.0), retake_connection) {
            return 350 + e;
        }
        if let Err(e) = take_all_and_close() {
            return 360 + e;
        }
        say(b"[ socket-teardown ] revoke: a copy's revoke keeps it, the original's frees it\n");
        say(b"[ socket-teardown ] PASS\n");
        0
    }
}
