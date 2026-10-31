// SPDX-License-Identifier: MPL-2.0
//! L1.3a (`desktop-actions`; THINK-2026-10-30-l1.3): Arthur's actions on the
//! desktop. Arthur holds no file grant of its own: for each action it asks
//! the WM (`OP_PERMISSION`), which answers from its record or its prompt and
//! lends a cap for that one action; Arthur acts through that cap alone, then
//! reports (`OP_PERMISSION_DONE`) and the WM revokes it.

use crate::desktop_action_logic::{DesktopAction, LIST, READ};
use crate::showcase_surface::ArthurSurface;
use alloc::vec::Vec;
use libneo::atrium_protocol::{
    STATUS_PERMISSION_DENIED, STATUS_PERMISSION_GRANTED_ALWAYS, STATUS_PERMISSION_PATH,
    STATUS_PERMISSION_READ_ONLY,
};
use libneo::{CapRights, FileHandle, FileRights, meta_rights};

/// The most a read shows, and a listing's buffer.
const READ_MAX: usize = 256;
const LIST_BUF: usize = 1024;

fn rights(kind: u16) -> CapRights {
    let file = match kind {
        READ => FileRights::READ,
        LIST => FileRights::LOOKUP.union(FileRights::READDIR),
        _ => FileRights::READ.union(FileRights::WRITE),
    };
    CapRights::new(meta_rights::USE, (file.bits() & 0x7f) as u8)
}

fn verb(kind: u16) -> &'static [u8] {
    match kind {
        READ => b"read",
        LIST => b"list",
        _ => b"change",
    }
}

fn push_num(reply: &mut Vec<u8>, mut v: u64) {
    let mut digits = [0u8; 20];
    let mut n = 0;
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    reply.extend(digits[..n].iter().rev());
}

/// Do `action` through the WM: the reply line goes into `reply`. `Err`: the
/// session channel failed.
pub fn respond(
    surface: &mut ArthurSurface,
    action: DesktopAction<'_>,
    intent: &str,
    reply: &mut Vec<u8>,
) -> Result<(), i32> {
    let (kind, path) = (action.kind(), action.path());
    reply.clear();
    let (status, cap) = surface.permission(kind, path, intent, rights(kind))?;
    log_answer(kind, path, status);
    let Some(cap) = cap else {
        reply.extend_from_slice(match status {
            STATUS_PERMISSION_READ_ONLY => b"I can only read for now, so I didn't change ",
            STATUS_PERMISSION_DENIED => b"You didn't allow that, so I left ",
            STATUS_PERMISSION_PATH => b"I can't use ",
            _ => b"I couldn't get access to ",
        });
        reply.extend_from_slice(path.as_bytes());
        reply.push(b'.');
        return Ok(());
    };
    let file = FileHandle(cap.0);
    let outcome = match action {
        DesktopAction::Read { .. } => {
            let mut buf = [0u8; READ_MAX];
            let n = libneo::sys_read(file, &mut buf);
            if n >= 0 {
                reply.extend_from_slice(path.as_bytes());
                reply.extend_from_slice(b": ");
                reply.extend(
                    buf[..n as usize]
                        .iter()
                        .map(|&b| if (0x20..0x7f).contains(&b) { b } else { b' ' }),
                );
            }
            n
        }
        DesktopAction::List { .. } => {
            let mut buf = [0u8; LIST_BUF];
            let mut cursor = 0u64;
            let n = libneo::sys_readdir(file, &mut buf, &mut cursor);
            if n >= 0 {
                reply.extend_from_slice(path.as_bytes());
                reply.push(b':');
                let mut at = 0usize;
                while at < n as usize {
                    let Ok(entry) = libneo::parse_dirent_v1(&buf[at..n as usize]) else {
                        break;
                    };
                    reply.push(b' ');
                    reply.extend_from_slice(entry.name);
                    at += entry.record_len as usize;
                }
            }
            n
        }
        DesktopAction::Write { text, .. } => {
            let n = libneo::sys_write(file.0, text.as_ptr(), text.len());
            if n >= 0 {
                reply.extend_from_slice(b"Wrote ");
                push_num(reply, n as u64);
                reply.extend_from_slice(b" bytes to ");
                reply.extend_from_slice(path.as_bytes());
                reply.push(b'.');
            }
            n
        }
    };
    crate::stage_b_log_bytes(b"DESKTOP arthur action outcome=");
    crate::stage_b_log_i64(outcome);
    crate::stage_b_log_newline();
    if outcome < 0 {
        reply.extend_from_slice(b"I couldn't ");
        reply.extend_from_slice(verb(kind));
        reply.push(b' ');
        reply.extend_from_slice(path.as_bytes());
        reply.extend_from_slice(b" (error ");
        push_num(reply, outcome.unsigned_abs());
        reply.extend_from_slice(b").");
    }
    let _ = surface.permission_done(outcome.min(0).unsigned_abs() as u32)?;
    // The boot test's proof that the WM revoked the lent cap: this copy no
    // longer reads.
    #[cfg(feature = "desktop-actions-probe")]
    {
        let mut byte = [0u8; 1];
        let after = libneo::sys_read(file, &mut byte);
        crate::stage_b_log_bytes(b"DESKTOP arthur lent cap after done read=");
        crate::stage_b_log_i64(after);
        crate::stage_b_log_newline();
    }
    let _ = libneo::sys_close(file);
    Ok(())
}

fn log_answer(kind: u16, path: &str, status: u32) {
    crate::stage_b_log_bytes(b"DESKTOP arthur permission action=");
    crate::stage_b_log_bytes(verb(kind));
    crate::stage_b_log_bytes(b" path=");
    crate::stage_b_log_bytes(path.as_bytes());
    crate::stage_b_log_bytes(b" status=");
    crate::stage_b_log_u64(u64::from(status));
    crate::stage_b_log_bytes(if status == STATUS_PERMISSION_GRANTED_ALWAYS {
        b" always"
    } else {
        b""
    });
    crate::stage_b_log_newline();
}

/// Boot test probes (`desktop-actions-probe`, #248 bounce): `stall` sends a
/// permission header and withholds its path (the WM must keep the lock
/// working and refuse it); `escapeprobe` asks, after 4 s (time for the
/// script's Super+Escape), to read `/neofs/escape.txt`, so the prompt comes
/// up while the keyboard is on the WM's own focus. Dexter 213: `intentstall`
/// sends the path and withholds the intent; `escapewait` holds the path 4 s
/// after the header (the Super+Escape lands while the WM waits for it);
/// `badidentity` names another surface (see [`bad_identity`]). `Some(0)`: handled (the
/// reply in `reply`); `Some(code)`: the channel failed; `None`: not a probe.
#[cfg(feature = "desktop-actions-probe")]
pub fn probe(surface: &mut ArthurSurface, line: &str, reply: &mut Vec<u8>) -> Option<i32> {
    reply.clear();
    let line = line.trim();
    match line {
        "stall" | "intentstall" => {
            let intent = line == "intentstall";
            let what: &[u8] = if intent { b"intent stall" } else { b"stall" };
            crate::stage_b_log_bytes(b"DESKTOP arthur permission ");
            crate::stage_b_log_bytes(what);
            crate::stage_b_log_bytes(b" sent");
            crate::stage_b_log_newline();
            match surface.permission_stall(intent) {
                Ok(status) => {
                    crate::stage_b_log_bytes(b"DESKTOP arthur permission ");
                    crate::stage_b_log_bytes(what);
                    crate::stage_b_log_bytes(b" status=");
                    crate::stage_b_log_u64(u64::from(status));
                    crate::stage_b_log_newline();
                    reply.extend_from_slice(b"(stall probe answered)");
                    Some(0)
                }
                Err(code) => Some(code),
            }
        }
        "escapeprobe" => {
            crate::stage_b_log_bytes(b"DESKTOP arthur escape probe armed");
            crate::stage_b_log_newline();
            let until = libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC)
                .unwrap_or(0)
                .saturating_add(4_000_000_000);
            while libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC).unwrap_or(u64::MAX) < until {
                libneo::sys_sched_yield();
            }
            let action = DesktopAction::Read {
                path: "/neofs/escape.txt",
            };
            match respond(surface, action, "escapeprobe", reply) {
                Ok(()) => Some(0),
                Err(code) => Some(code),
            }
        }
        // Dexter 213 item 2: Super+Escape pressed while the WM waits for the
        // path (held 4 s after the header).
        "escapewait" => {
            crate::stage_b_log_bytes(b"DESKTOP arthur escape wait armed");
            crate::stage_b_log_newline();
            crate::showcase_surface::HOLD_PATH_NS
                .store(4_000_000_000, core::sync::atomic::Ordering::Relaxed);
            let action = DesktopAction::Read {
                path: "/neofs/escape.txt",
            };
            match respond(surface, action, "escapewait", reply) {
                Ok(()) => Some(0),
                Err(code) => Some(code),
            }
        }
        "badidentity" => match bad_identity(surface) {
            Ok(()) => {
                reply.extend_from_slice(b"(identity probe answered)");
                Some(0)
            }
            Err(code) => Some(code),
        },
        _ => None,
    }
}

/// Dexter 213 item 4: a request, then a done, naming another surface. The
/// request is refused with nothing lent; with a cap lent (the recorded
/// todo.txt), a wrong done is refused and leaves it lent, and the right one
/// revokes it. Logs `DESKTOP arthur identity probe request=S` and
/// `... bad_done=S read=N good_done=S read=N`.
#[cfg(feature = "desktop-actions-probe")]
fn bad_identity(surface: &mut ArthurSurface) -> Result<(), i32> {
    use core::sync::atomic::Ordering;
    let flip = &crate::showcase_surface::FLIP_IDENTITY;
    let path = "/neofs/todo.txt";
    flip.store(true, Ordering::Relaxed);
    let (status, cap) = surface.permission(READ, path, "badidentity", rights(READ))?;
    crate::stage_b_log_bytes(b"DESKTOP arthur identity probe request=");
    crate::stage_b_log_u64(u64::from(status));
    crate::stage_b_log_bytes(if cap.is_some() { b" lent" } else { b"" });
    crate::stage_b_log_newline();
    let (status, cap) = surface.permission(READ, path, "badidentity", rights(READ))?;
    let Some(cap) = cap else {
        crate::stage_b_log_bytes(b"DESKTOP arthur identity probe lend status=");
        crate::stage_b_log_u64(u64::from(status));
        crate::stage_b_log_newline();
        return Ok(());
    };
    let file = FileHandle(cap.0);
    let mut byte = [0u8; 1];
    flip.store(true, Ordering::Relaxed);
    let bad = surface.permission_done(0)?;
    let bad_read = libneo::sys_read(file, &mut byte);
    let good = surface.permission_done(0)?;
    let good_read = libneo::sys_read(file, &mut byte);
    let _ = libneo::sys_close(file);
    crate::stage_b_log_bytes(b"DESKTOP arthur identity probe bad_done=");
    crate::stage_b_log_u64(u64::from(bad));
    crate::stage_b_log_bytes(b" read=");
    crate::stage_b_log_i64(bad_read);
    crate::stage_b_log_bytes(b" good_done=");
    crate::stage_b_log_u64(u64::from(good));
    crate::stage_b_log_bytes(b" read=");
    crate::stage_b_log_i64(good_read);
    crate::stage_b_log_newline();
    Ok(())
}
