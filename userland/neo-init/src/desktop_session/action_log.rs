// SPDX-License-Identifier: MPL-2.0
//! L2.1 (`arthur-action-log`; THINK-2026-10-30-l2): the WM's append-only,
//! hash-chained action log, `NAL1` records (`action-log-core`) in
//! `/neofs/.system/actions.log`, the WM-only store. Every request, answer,
//! lend, refusal and done that passes the permission prompt is appended,
//! with the intent Arthur sent.
//!
//! **Appending** (no append flag, no seek): open, read to the end (verifying
//! the chain on the way; it also leaves the offset at the end), write the
//! record there, fsync. The record is written before the WM lends anything
//! for it: an append that fails refuses the lend (see `permission`).
//! - **A broken chain** is cut back to its last good record; the bytes cut
//!   off are kept in [`alc::BROKEN_PATH`], and a `Refused` record with detail
//!   [`alc::REFUSED_CHAIN_BROKEN`] goes first, so the break is on the record.
//! - **Records dropped off the end** while the WM runs: the file's head is
//!   checked against the one the WM last left, and a mismatch is recorded
//!   ([`alc::REFUSED_TRUNCATED`]). Across a reboot only an anchor outside the
//!   file could tell (THINK §2).
//! - **Past [`CAP`]** the oldest half is dropped behind a rotation marker
//!   ([`alc::REFUSED_ROTATED`]), outside the chain, naming the sequence and
//!   link the first kept record carries: its bytes and every later link are
//!   unchanged.
//! - **Rewrites** (a repair, a rotation) go through `login::write_whole`:
//!   4 KiB writes (the kernel's `MAX_FILE_IO`), then a truncate and an fsync.

use super::{Line, login};
use action_log_core::{self as alc, Kind, Record};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use libneo::{
    FileRights, OPEN_CREATE_REGULAR, OPEN_ROOT_TEMPORARY, sys_close, sys_open, sys_read, sys_write,
};

/// The most the log holds before it rotates.
const CAP: usize = if cfg!(feature = "arthur-actions-test") {
    // The boot test rotates a small log (the seed passes it).
    16 * 1024
} else {
    1 << 20
};
/// The most a repair keeps of the bytes it cuts off.
const BROKEN_CAP: usize = 64 * 1024;
/// One kernel file I/O.
const IO: usize = 4096;

/// The head the WM last left this boot: the next sequence number plus one (0:
/// none yet), and the link, in two halves.
static LAST_NEXT: AtomicU64 = AtomicU64::new(0);
static LAST_LINK_LO: AtomicU64 = AtomicU64::new(0);
static LAST_LINK_HI: AtomicU64 = AtomicU64::new(0);

fn last() -> Option<(u64, [u8; alc::PREV_LEN])> {
    let next = LAST_NEXT.load(Ordering::Acquire);
    if next == 0 {
        return None;
    }
    let mut link = [0u8; alc::PREV_LEN];
    link[..8].copy_from_slice(&LAST_LINK_LO.load(Ordering::Acquire).to_le_bytes());
    link[8..].copy_from_slice(&LAST_LINK_HI.load(Ordering::Acquire).to_le_bytes());
    Some((next - 1, link))
}

fn set_last(next: u64, link: [u8; alc::PREV_LEN]) {
    LAST_LINK_LO.store(
        u64::from_le_bytes(link[..8].try_into().unwrap_or_default()),
        Ordering::Release,
    );
    LAST_LINK_HI.store(
        u64::from_le_bytes(link[8..].try_into().unwrap_or_default()),
        Ordering::Release,
    );
    LAST_NEXT.store(next + 1, Ordering::Release);
}

/// The wall clock in seconds (0 when unknown) and the record's flags: with
/// [`alc::TIME_UTC`] only if the kernel had confirmed realtime as UTC
/// (THINK-2026-11-02 §2.7). Before that, it's the computer's clock as read,
/// which may be local wall time. The status is read before and after the
/// clock: a confirm in between (the base moved) reads again, so a flagged
/// time is always a confirmed one. The boot test pins 0, so its records'
/// bytes are fixed.
fn now_s() -> (u64, u8) {
    if cfg!(feature = "arthur-actions-test") {
        return (0, 0);
    }
    let utc = || libneo::sys_clock_realtime_status() == Ok(true);
    let read =
        || libneo::sys_clock_gettime(libneo::CLOCK_REALTIME).map_or(0, |ns| ns / 1_000_000_000);
    for _ in 0..4 {
        let before = utc();
        let s = read();
        if utc() == before {
            return (s, if before { alc::TIME_UTC } else { 0 });
        }
    }
    // Confirmed while reading: it stays confirmed (write-once).
    (read(), alc::TIME_UTC)
}

/// The log's printable form of `text`: each byte, or `?` for one that isn't
/// printable ASCII (so non-UTF-8 text is kept, not dropped).
pub fn printable(text: &[u8]) -> alloc::string::String {
    text.iter()
        .take(alc::TEXT_MAX)
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '?'
            }
        })
        .collect()
}

/// Open the log for reading and writing (created when absent).
fn open() -> Option<libneo::FileHandle> {
    let rw = FileRights::READ.union(FileRights::WRITE);
    match sys_open(alc::PATH, OPEN_ROOT_TEMPORARY, rw) {
        Err(libneo::ENOENT) => sys_open(alc::PATH, OPEN_CREATE_REGULAR, rw).ok(),
        other => other.ok(),
    }
}

/// Read all of `file` from its start (leaving the offset at its end).
fn read_all(file: libneo::FileHandle) -> Option<Vec<u8>> {
    let mut log = Vec::new();
    let mut buf = [0u8; IO];
    loop {
        let n = sys_read(file, &mut buf);
        if n < 0 {
            return None;
        }
        if n == 0 {
            return Some(log);
        }
        if log.len() + n as usize > CAP + IO {
            return None;
        }
        log.extend_from_slice(&buf[..n as usize]);
    }
}

fn write_all(file: libneo::FileHandle, bytes: &[u8]) -> bool {
    bytes
        .chunks(IO)
        .all(|chunk| sys_write(file.0, chunk.as_ptr(), chunk.len()) == chunk.len() as i64)
}

/// The records being appended, chained as they are added.
struct Pending {
    bytes: Vec<u8>,
    next: u64,
    link: [u8; alc::PREV_LEN],
}

impl Pending {
    fn push(&mut self, kind: Kind, action: u8, detail: u32, path: &str, intent: &str) -> bool {
        // The control (`arthur-action-log-chain-control`) breaks every link.
        let prev = if cfg!(feature = "arthur-action-log-chain-control") {
            [0u8; alc::PREV_LEN]
        } else {
            self.link
        };
        let (time_s, flags) = now_s();
        let Some(bytes) = alc::encode(&Record {
            kind,
            action,
            seq: self.next,
            time_s,
            detail,
            path,
            intent,
            prev,
            flags,
        }) else {
            return false;
        };
        self.link = alc::link(&bytes);
        self.next += 1;
        self.bytes.extend_from_slice(&bytes);
        true
    }

    fn mark(&mut self, detail: u32) -> bool {
        self.push(Kind::Refused, 0, detail, "", "")
    }
}

/// `log`'s good part (`len` bytes) with its oldest half dropped behind a
/// rotation marker: the first kept record's sequence and link go in the
/// marker, so the kept bytes chain as they did.
fn rotated(log: &[u8], len: usize) -> Option<Vec<u8>> {
    let mut at = 0;
    // A marker already first is dropped with the rest.
    while at < len / 2 {
        let (_, n) = alc::decode(&log[at..len])?;
        at += n;
    }
    let (first, _) = alc::decode(&log[at..len])?;
    let (time_s, flags) = now_s();
    let mut out = alc::encode(&Record {
        kind: Kind::Refused,
        action: 0,
        seq: first.seq,
        time_s,
        detail: alc::REFUSED_ROTATED,
        path: "",
        intent: "",
        prev: first.prev,
        flags,
    })?;
    out.extend_from_slice(&log[at..len]);
    Line::new()
        .s(b"ACTIONLOG rotated dropped_bytes=")
        .n(at as u64)
        .s(b"\n")
        .emit();
    Some(out)
}

/// Append one record. `false`: it couldn't be (a line says so; the caller
/// decides what that means for the action).
pub fn append(kind: Kind, action: u16, detail: u32, path: &str, intent: &str) -> bool {
    let ok = append_inner(kind, action, detail, path, intent);
    if !ok {
        Line::new().s(b"ACTIONLOG append=failed\n").emit();
    }
    ok
}

fn append_inner(kind: Kind, action: u16, detail: u32, path: &str, intent: &str) -> bool {
    let Some(file) = open() else {
        return false;
    };
    let Some(log) = read_all(file) else {
        let _ = sys_close(file);
        return false;
    };
    let head = alc::verify(&log);
    let mut pending = Pending {
        bytes: Vec::new(),
        next: head.records,
        link: head.link,
    };
    // The good part, rewritten (rather than appended to) after a repair.
    let mut rewrite = false;
    if let Some(at) = head.broken_at {
        if !login::write_whole(
            alc::BROKEN_PATH,
            &log[head.len..log.len().min(head.len + BROKEN_CAP)],
        ) {
            Line::new().s(b"ACTIONLOG broken-tail keep=failed\n").emit();
        }
        rewrite = true;
        Line::new()
            .s(b"ACTIONLOG chain broken at=")
            .n(at)
            .s(b"\n")
            .emit();
        if !pending.mark(alc::REFUSED_CHAIN_BROKEN) {
            let _ = sys_close(file);
            return false;
        }
    } else if last().is_some_and(|(next, link)| next != head.records || link != head.link) {
        Line::new().s(b"ACTIONLOG truncated\n").emit();
        if !pending.mark(alc::REFUSED_TRUNCATED) {
            let _ = sys_close(file);
            return false;
        }
    }
    let (path, intent) = (printable(path.as_bytes()), printable(intent.as_bytes()));
    if !pending.push(kind, action as u8, detail, &path, &intent) {
        let _ = sys_close(file);
        return false;
    }
    let mut whole = None;
    if head.len + pending.bytes.len() > CAP {
        let Some(mut kept) = rotated(&log, head.len) else {
            let _ = sys_close(file);
            return false;
        };
        kept.extend_from_slice(&pending.bytes);
        whole = Some(kept);
    } else if rewrite {
        let mut good = log[..head.len].to_vec();
        good.extend_from_slice(&pending.bytes);
        whole = Some(good);
    }
    let ok = match whole {
        Some(bytes) => {
            let _ = sys_close(file);
            login::write_whole(alc::PATH, &bytes)
        }
        // The file is read to its end: the write lands there.
        None => {
            let ok = write_all(file, &pending.bytes) && libneo::sys_fsync(file) == 0;
            let _ = sys_close(file);
            ok
        }
    };
    if ok {
        set_last(pending.next, pending.link);
    }
    ok
}

/// Boot test (#253 bounce): a first boot finds a log over 4 KiB with a
/// torn tail: [`SEED_RECORDS`] good records, then part of one more (as a
/// power cut mid-append leaves it). The first append must repair it (a
/// rewrite over one kernel write), and a later one rotate it.
#[cfg(feature = "arthur-actions-test")]
pub const SEED_RECORDS: u64 = 190;

#[cfg(feature = "arthur-actions-test")]
pub fn seed() {
    if login::read_small_up_to(alc::PATH, 1).is_some() {
        return;
    }
    let mut pending = Pending {
        bytes: Vec::new(),
        next: 0,
        link: [0u8; alc::PREV_LEN],
    };
    for _ in 0..=SEED_RECORDS {
        let _ = pending.push(Kind::Asked, 1, 0, "/neofs/seed.txt", "seed");
    }
    // The last record, torn after 30 bytes.
    let torn =
        pending.bytes.len() - (alc::HEADER_LEN + "/neofs/seed.txt".len() + "seed".len()) + 30;
    if !login::write_whole(alc::PATH, &pending.bytes[..torn]) {
        Line::new().s(b"ACTIONLOG seed=failed\n").emit();
    }
}

/// Boot test: verify the log and log its state (records, a break, the head).
#[cfg(feature = "arthur-actions-test")]
pub fn report() {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let log = login::read_small_up_to(alc::PATH, CAP).unwrap_or_default();
    let head = alc::verify(&log);
    let mut hex = [0u8; 2 * alc::PREV_LEN];
    for (i, b) in head.link.iter().enumerate() {
        hex[2 * i] = HEX[usize::from(b >> 4)];
        hex[2 * i + 1] = HEX[usize::from(b & 15)];
    }
    Line::new()
        .s(b"ACTIONLOG verify records=")
        .n(head.records)
        .s(match head.broken_at {
            None => b" broken=none head=",
            Some(_) => b" broken=yes head=",
        })
        .s(&hex)
        .s(b"\n")
        .emit();
}
