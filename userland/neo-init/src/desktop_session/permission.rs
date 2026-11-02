// SPDX-License-Identifier: MPL-2.0
//! L1.3a (`arthur-actions`; THINK-2026-10-30-l1.3): Arthur's actions behind
//! the WM's permission prompt. The WM is the grant authority on the desktop.
//!
//! - **The record** is `/neofs/.system/arthur-grants.json` (grants.json v1,
//!   `libneo::grant_store`), in the WM-only store: Arthur can't write the
//!   record that governs it (D2). A missing or malformed record grants
//!   nothing.
//! - **On demand.** Arthur holds no grant until it asks: `OP_PERMISSION`
//!   names the action and (in a following raw message) the path. A path the
//!   record already grants is lent without a prompt; otherwise the WM shows
//!   its own prompt (the lock screen's kind of view: WM focus, the client
//!   held) and lends the cap only on Allow once or Always (D4, D5).
//! - **The cap is the authority.** The WM derives it from its own open of the
//!   path and lends it over the client's channel for that one action, and
//!   revokes it at `OP_PERMISSION_DONE` through the cap graph, whatever the
//!   client does with its copy. "Always" is recorded, so the next request for
//!   it is lent without a prompt (a fresh cap each time: a file handle has
//!   no seek, so a cap kept between reads would read on from its end). A
//!   denial is remembered for the session, without another prompt.
//! - **Read-only by default** (D7): a write is refused before any prompt
//!   unless the session is built with `arthur-actions-write`.

use super::{Line, Plane, Scene, grant_over, home::HomeFocus, lock, login};
use alloc::string::String;
use alloc::vec::Vec;
use libneo::atrium_protocol::{
    PERMISSION_LIST, PERMISSION_PATH_MAX, PERMISSION_READ, PERMISSION_WRITE, Permission,
    STATUS_PERMISSION_DENIED, STATUS_PERMISSION_GRANTED, STATUS_PERMISSION_GRANTED_ALWAYS,
    STATUS_PERMISSION_PATH, STATUS_PERMISSION_READ_ONLY,
};
use libneo::grant_store::{self, GrantEntry, ScopeKind};
use libneo::{
    CapRights, EAGAIN, FileRights, Handle, INPUT_EVENT_V1_LEN, OPEN_ROOT_TEMPORARY, meta_rights,
    sys_close, sys_derive, sys_input_focus_activate, sys_input_focus_recv, sys_open, sys_revoke,
};
use neo_atrium::keys::{self, Key};

/// The grant record (D2).
pub const RECORD: &str = "/neofs/.system/arthur-grants.json";
/// The largest record the WM reads or writes: one kernel file I/O (a record
/// that would outgrow it isn't written, and the grant lasts the session).
const RECORD_MAX: usize = 4096;
/// Denials remembered per session (more are refused unprompted anyway).
const DENIALS_MAX: usize = 32;

/// The prompt's three answers, and the lock's: a prompt the lock took down
/// is a deny the user didn't give (not remembered for the session).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice {
    Once,
    Always,
    Deny,
    Locked,
}

/// L2.1: one action-log record. `false`: it couldn't be appended, and the
/// caller lends nothing on it (#253 bounce: no action runs unrecorded).
/// Without `arthur-action-log` there is no log and every note succeeds.
#[allow(unused_variables)]
fn note(kind: u8, action: u16, detail: u32, path: &str, intent: &str) -> bool {
    // Boot test: the append of an Always for this one path fails (as a log
    // over its read limit, or a full disk, would make it).
    #[cfg(feature = "arthur-actions-test")]
    if kind == ALLOWED_ALWAYS && path == NOLOG_PATH {
        Line::new().s(b"ACTIONLOG append=failed\n").emit();
        return false;
    }
    #[cfg(feature = "arthur-action-log")]
    if let Some(kind) = action_log_core::Kind::from_u8(kind) {
        return super::action_log::append(kind, action, detail, path, intent);
    }
    true
}

/// Boot test: the path whose Always can't be logged. It is no longer than
/// todo.txt, so it fits the seeded record's one free slot.
#[cfg(feature = "arthur-actions-test")]
const NOLOG_PATH: &str = "/neofs/nolg.txt";

/// The `NAL1` kinds and refusal reasons the prompt logs (action-log-core's).
const ASKED: u8 = 1;
const ALLOWED_ONCE: u8 = 2;
const ALLOWED_ALWAYS: u8 = 3;
const DENIED: u8 = 4;
const LENT_FROM_RECORD: u8 = 5;
const REFUSED: u8 = 6;
const DONE: u8 = 7;
const REASON_PATH: u32 = 1;
const REASON_READ_ONLY: u32 = 2;
const REASON_DENIED_THIS_SESSION: u32 = 3;
const REASON_LOCKED: u32 = 4;
const REASON_OPEN: u32 = 5;
/// A `Done` the client never reported (its cap revoked at its next request
/// or the session's end).
const DONE_UNREPORTED: u32 = 0xFFFF_FFFF;

/// One Arthur session's permission state.
#[derive(Default)]
pub struct Session {
    /// Paths and actions denied this session.
    denied: Vec<(String, u16)>,
    /// The cap lent for the action in progress, revoked at
    /// `OP_PERMISSION_DONE` (or the next request, or the session's end).
    lent: Option<Handle>,
    /// That action, for the log's done record: the action, path and intent.
    lent_for: Option<(u16, String, String)>,
    /// The prompt's plane.
    plane: Option<Plane>,
}

impl Session {
    /// A cap still lent is revoked, and its action logged as done without a
    /// report from the client.
    fn revoke_lent(&mut self) {
        if let Some(cap) = self.lent.take() {
            let _ = sys_revoke(cap);
        }
        if let Some((action, path, intent)) = self.lent_for.take() {
            note(DONE, action, DONE_UNREPORTED, &path, &intent);
        }
    }
}

/// However the session ends (its normal end, or an early error return in
/// `arthur_session`), a cap still lent is revoked (#248 follow-up).
impl Drop for Session {
    fn drop(&mut self) {
        self.revoke_lent();
    }
}

/// What a request comes to: the reply's status, and the cap to lend after
/// it (with the rights the client may hold).
pub struct Answer {
    pub status: u32,
    lend: Option<(Handle, CapRights)>,
}

fn verb(action: u16) -> &'static str {
    match action {
        PERMISSION_READ => "read",
        PERMISSION_LIST => "list",
        _ => "change",
    }
}

/// The record's scope and the file rights the action needs.
fn scope_rights(action: u16) -> (ScopeKind, FileRights) {
    match action {
        PERMISSION_READ => (ScopeKind::FileRead, FileRights::READ),
        PERMISSION_LIST => (
            ScopeKind::FileRead,
            FileRights::LOOKUP.union(FileRights::READDIR),
        ),
        _ => (
            ScopeKind::FileWrite,
            FileRights::READ.union(FileRights::WRITE),
        ),
    }
}

/// The path the WM will prompt for: absolute, under `/neofs/`, no empty,
/// `.` or `..` component, not under `.system/`, and only characters the
/// record can hold (printable ASCII, no `"` or `\`).
fn valid_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/neofs/") else {
        return false;
    };
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    !rest.is_empty()
        && path
            .bytes()
            .all(|b| (0x20..0x7f).contains(&b) && b != b'"' && b != b'\\')
        && rest
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && rest.split('/').next() != Some(".system")
}

fn log(what: &[u8], action: u16, path: &str) {
    Line::new()
        .s(b"PERMISSION ")
        .s(what)
        .s(b" action=")
        .s(verb(action).as_bytes())
        .s(b" path=")
        .s(path.as_bytes())
        .s(b"\n")
        .emit();
}

/// The record's bytes (`None`: absent or unreadable).
fn read_record() -> Option<Vec<u8>> {
    login::read_small_up_to(RECORD, RECORD_MAX)
}

/// Whether the record grants `action` on `path`.
fn recorded(action: u16, path: &str) -> bool {
    let Some(bytes) = read_record() else {
        return false;
    };
    let mut entries = [GrantEntry::EMPTY; grant_store::MAX_GRANTS];
    let Ok(n) = grant_store::parse_grants(&bytes, &mut entries) else {
        return false;
    };
    let (scope, rights) = scope_rights(action);
    entries[..n].iter().any(|e| {
        e.scope == scope && e.resource == path && e.rights.bits() & rights.bits() == rights.bits()
    })
}

/// Add `action` on `path` to the record (Always). `false`: it couldn't be
/// written (the grant still holds for this session).
fn record(action: u16, path: &str) -> bool {
    let existing = read_record().unwrap_or_default();
    let mut entries = [GrantEntry::EMPTY; grant_store::MAX_GRANTS];
    // A malformed record is replaced (it granted nothing anyway).
    let n = grant_store::parse_grants(&existing, &mut entries).unwrap_or(0);
    if n >= entries.len() {
        return false;
    }
    let (scope, rights) = scope_rights(action);
    entries[n] = GrantEntry {
        scope,
        resource: path,
        rights,
        granted_by: "wm-prompt",
    };
    let mut out = alloc::vec![0u8; RECORD_MAX];
    let Some(len) = grant_store::serialize_grants(&entries[..=n], &mut out) else {
        return false;
    };
    login::write_whole(RECORD, &out[..len])
}

/// The WM's own cap on `path` for `action`, to lend.
fn derive(action: u16, path: &str) -> Option<(Handle, CapRights)> {
    let (_, rights) = scope_rights(action);
    let file = sys_open(path, OPEN_ROOT_TEMPORARY, rights).ok()?;
    let bits = (rights.bits() & 0x7f) as u8;
    let derived = sys_derive(
        Handle(file.0),
        CapRights::new(meta_rights::USE | meta_rights::GRANT, bits),
    );
    let _ = sys_close(file);
    derived
        .ok()
        .map(|cap| (cap, CapRights::new(meta_rights::USE, bits)))
}

/// How long the WM waits for a request's path after its header (the boot
/// test's stall legs need the lock, and the idle lock, to engage meanwhile).
const PART_NS: u64 = if cfg!(feature = "arthur-actions-test") {
    45_000_000_000
} else {
    2_000_000_000
};

fn now_ns() -> u64 {
    libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC).unwrap_or(0)
}

/// The next message on `surface`, exactly `buf.len()` bytes, polled (#248
/// bounce F1, CP-57/CP-97): a client that sends a header and withholds the
/// rest can't park the WM, so the lock keeps engaging and its screen keeps
/// being served meanwhile. `false`: it didn't come in [`PART_NS`], or came
/// with another length (the request is refused). The pointer and the SAK
/// chords are served as in the session loop, so a Super+Escape pressed
/// meanwhile takes the keyboard (and `escaped` says so; Dexter 213 item 2).
fn recv_part(
    scene: &mut Scene,
    client: &mut Client<'_>,
    focus: &HomeFocus,
    buf: &mut [u8],
) -> Result<bool, i32> {
    let deadline = now_ns().saturating_add(PART_NS);
    loop {
        match libneo::sys_try_recv(client.surface, buf) {
            n if n >= 0 => return Ok(n as usize == buf.len()),
            libneo::EAGAIN => {
                #[cfg(feature = "desktop-pointer")]
                scene.pointer_tick(client.activator, focus, client.escaped)?;
                #[cfg(not(feature = "desktop-pointer"))]
                {
                    let _ = focus;
                    lock::service(scene)?;
                }
                if now_ns() >= deadline {
                    return Ok(false);
                }
                libneo::sys_sched_yield();
            }
            _ => return Ok(false),
        }
    }
}

/// The client side of one request: its channel, its input activator (given
/// back after a prompt unless the keyboard is on the WM's own focus,
/// `escaped`, Super+Escape: the session loop's own flag, kept up to date
/// while the request is read), and whether the request named this channel's
/// own surface (`ours`).
pub struct Client<'a> {
    pub surface: Handle,
    pub activator: Handle,
    pub escaped: &'a mut bool,
    pub ours: bool,
}

/// Serve one `OP_PERMISSION` on `client.surface` (its path is the next
/// message there).
pub fn serve(
    scene: &mut Scene,
    session: &mut Session,
    request: Permission,
    focus: &HomeFocus,
    mut client: Client<'_>,
) -> Result<Answer, i32> {
    let refuse = |status| Answer { status, lend: None };
    let mut buf = [0u8; PERMISSION_PATH_MAX];
    let len = usize::from(request.path_len);
    if !recv_part(scene, &mut client, focus, &mut buf[..len])? {
        Line::new()
            .s(b"PERMISSION refused reason=incomplete\n")
            .emit();
        return Ok(refuse(STATUS_PERMISSION_PATH));
    }
    // L2.1: the line the action came from follows the path, read the same
    // polled way (#248 bounce F1).
    let mut intent_buf = [0u8; PERMISSION_PATH_MAX];
    let intent_len = usize::from(request.intent_len);
    if intent_len != 0 && !recv_part(scene, &mut client, focus, &mut intent_buf[..intent_len])? {
        Line::new()
            .s(b"PERMISSION refused reason=incomplete\n")
            .emit();
        return Ok(refuse(STATUS_PERMISSION_PATH));
    }
    // Kept byte for byte, `?` for anything that isn't printable ASCII (a
    // non-UTF-8 intent is still logged, #253 note).
    let intent: String = intent_buf[..intent_len]
        .iter()
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '?'
            }
        })
        .collect();
    let intent = intent.as_str();
    // The request must name this channel's own surface (Dexter 203 note);
    // its path is read first, so nothing of it is left on the channel.
    let (activator, escaped) = (client.activator, *client.escaped);
    if !client.ours {
        Line::new()
            .s(b"PERMISSION refused reason=identity\n")
            .emit();
        return Ok(refuse(
            libneo::atrium_protocol::ProtocolError::CapRequired.status(),
        ));
    }
    let Ok(path) = core::str::from_utf8(&buf[..len]) else {
        return Ok(refuse(STATUS_PERMISSION_PATH));
    };
    let action = request.action;
    if !valid_path(path) {
        log(b"refused reason=path", action, path);
        note(REFUSED, action, REASON_PATH, path, intent);
        return Ok(refuse(STATUS_PERMISSION_PATH));
    }
    if action == PERMISSION_WRITE && !cfg!(feature = "arthur-actions-write") {
        log(b"refused reason=read-only", action, path);
        note(REFUSED, action, REASON_READ_ONLY, path, intent);
        return Ok(refuse(STATUS_PERMISSION_READ_ONLY));
    }
    if session
        .denied
        .iter()
        .any(|(p, a)| p == path && *a == action)
    {
        log(b"refused reason=denied-this-session", action, path);
        note(REFUSED, action, REASON_DENIED_THIS_SESSION, path, intent);
        return Ok(refuse(STATUS_PERMISSION_DENIED));
    }
    // A request while a cap is still out: that action is over.
    session.revoke_lent();
    // Nothing is lent behind the lock, a standing grant included (#248
    // follow-up).
    if lock::locked() {
        log(b"answer=deny reason=locked", action, path);
        note(DENIED, action, REASON_LOCKED, path, intent);
        return Ok(refuse(STATUS_PERMISSION_DENIED));
    }
    let from_record = recorded(action, path);
    let choice = if from_record {
        log(b"recorded", action, path);
        Choice::Always
    } else {
        note(ASKED, action, 0, path, intent);
        ask(scene, session, focus, activator, escaped, action, path)?
    };
    if matches!(choice, Choice::Deny | Choice::Locked) {
        let locked = choice == Choice::Locked;
        if !locked && session.denied.len() < DENIALS_MAX {
            session.denied.push((String::from(path), action));
        }
        log(b"answer=deny", action, path);
        note(
            DENIED,
            action,
            if locked { REASON_LOCKED } else { 0 },
            path,
            intent,
        );
        return Ok(refuse(STATUS_PERMISSION_DENIED));
    }
    // The cap first: an Always is saved, and an answer logged, only for a
    // path the WM could open (#253 bounce item 2).
    let Some((cap, rights)) = derive(action, path) else {
        log(b"refused reason=open", action, path);
        note(REFUSED, action, REASON_OPEN, path, intent);
        return Ok(refuse(STATUS_PERMISSION_PATH));
    };
    // The record as it was before an Always is saved (put back if the
    // Always can't be logged).
    let mut before = None;
    let (status, kind) = if from_record {
        log(b"lent from record", action, path);
        (STATUS_PERMISSION_GRANTED_ALWAYS, LENT_FROM_RECORD)
    } else if choice == Choice::Once {
        log(b"answer=once", action, path);
        (STATUS_PERMISSION_GRANTED, ALLOWED_ONCE)
    } else {
        log(b"answer=always", action, path);
        before = Some(read_record().unwrap_or_default());
        // An Always that couldn't be saved is answered, and logged, as what
        // it is: a grant for this action only (Dexter 203 note).
        if record(action, path) {
            (STATUS_PERMISSION_GRANTED_ALWAYS, ALLOWED_ALWAYS)
        } else {
            log(b"record=failed", action, path);
            (STATUS_PERMISSION_GRANTED, ALLOWED_ONCE)
        }
    };
    // No action runs unrecorded: a lend the log can't hold isn't made.
    if !note(kind, action, 0, path, intent) {
        let _ = sys_revoke(cap);
        log(b"refused reason=log", action, path);
        // Nor is a grant saved that the log doesn't hold (PM, #253 AAG).
        if kind == ALLOWED_ALWAYS
            && let Some(before) = before
        {
            if login::write_whole(RECORD, &before) {
                log(b"record undone", action, path);
            } else {
                log(b"record undo=failed", action, path);
            }
        }
        return Ok(refuse(STATUS_PERMISSION_DENIED));
    }
    session.lent = Some(cap);
    session.lent_for = Some((action, String::from(path), String::from(intent)));
    Ok(Answer {
        status,
        lend: Some((cap, rights)),
    })
}

/// After the reply: lend the answer's cap over `surface`. `false`: the
/// client didn't take it (its session is over).
pub fn lend(surface: Handle, answer: &Answer) -> bool {
    match answer.lend {
        Some((cap, rights)) => grant_over(surface, cap, rights),
        None => true,
    }
}

/// `OP_PERMISSION_DONE`: the lent cap is revoked, through the cap graph, so
/// the client's copy is gone whatever it does with it. The control
/// (`arthur-actions-revoke-control`) leaves it lent.
pub fn done(session: &mut Session, outcome: u32) {
    let revoked = session
        .lent
        .take()
        .filter(|_| !cfg!(feature = "arthur-actions-revoke-control"))
        .map(|cap| sys_revoke(cap) == 0);
    if let Some((action, path, intent)) = session.lent_for.take() {
        note(DONE, action, outcome, &path, &intent);
    }
    Line::new()
        .s(b"PERMISSION done outcome=")
        .n(u64::from(outcome))
        .s(match revoked {
            Some(true) => b" revoked=1\n",
            Some(false) => b" revoked=failed\n",
            None => b" revoked=none\n",
        })
        .emit();
}

/// The session is over: a cap still lent is revoked (the `Drop` does the
/// same on an early return).
pub fn end(session: &mut Session) {
    session.revoke_lent();
}

/// Boot test: the session's Arthur, ended when the test is (the product's
/// Arthur never exits by itself).
#[cfg(feature = "arthur-actions-test")]
static CLIENT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(u32::MAX);

#[cfg(feature = "arthur-actions-test")]
pub fn note_client(process: libneo::ProcessHandle) {
    CLIENT.store(process.0, core::sync::atomic::Ordering::Release);
}

#[cfg(feature = "arthur-actions-test")]
pub fn end_client() {
    let handle = CLIENT.load(core::sync::atomic::Ordering::Acquire);
    if handle != u32::MAX {
        let process = libneo::ProcessHandle(handle);
        let _ = libneo::sys_kill(process);
        let _ = libneo::sys_wait(process, 0);
    }
}

/// Boot test (`arthur-actions-test`): the files Arthur is asked to act on.
#[cfg(feature = "arthur-actions-test")]
pub fn seed() {
    super::action_log::seed();
    seed_record();
    for (path, text) in [
        ("/neofs/notes.txt", &b"buy milk"[..]),
        ("/neofs/todo.txt", &b"call ada"[..]),
        ("/neofs/escape.txt", &b"esc"[..]),
        ("/neofs/full.txt", &b"full"[..]),
        (NOLOG_PATH, &b"nolog"[..]),
    ] {
        if login::read_small_up_to(path, 64).is_none() && !login::write_whole(path, text) {
            Line::new()
                .s(b"PERMISSION seed failed path=")
                .s(path.as_bytes())
                .s(b"\n")
                .emit();
        }
    }
}

/// Boot test (Dexter 213 item 4): a first boot's record holds all it can
/// but two more read entries (one for `/neofs/todo.txt`, Always in boot 1),
/// so the next Always (`/neofs/full.txt`) can't be saved, by `MAX_GRANTS`
/// or the 4 KiB record, whichever binds first.
#[cfg(feature = "arthur-actions-test")]
fn seed_record() {
    if read_record().is_some() {
        return;
    }
    const FILLER: [&str; 10] = [
        "/neofs/s0.txt",
        "/neofs/s1.txt",
        "/neofs/s2.txt",
        "/neofs/s3.txt",
        "/neofs/s4.txt",
        "/neofs/s5.txt",
        "/neofs/s6.txt",
        "/neofs/s7.txt",
        "/neofs/s8.txt",
        "/neofs/s9.txt",
    ];
    let (scope, rights) = scope_rights(PERMISSION_READ);
    let entry = |resource| GrantEntry {
        scope,
        resource,
        rights,
        granted_by: "wm-prompt",
    };
    let mut entries = [GrantEntry::EMPTY; grant_store::MAX_GRANTS];
    let mut out = alloc::vec![0u8; RECORD_MAX];
    let mut n = 0;
    // A filler is no longer than `/neofs/full.txt`, so once a filler and
    // todo.txt no longer fit after `n` fillers, todo.txt and full.txt don't.
    while n + 2 <= entries.len() {
        entries[n] = entry(FILLER[n % FILLER.len()]);
        entries[n + 1] = entry("/neofs/todo.txt");
        if grant_store::serialize_grants(&entries[..n + 2], &mut out).is_none() {
            break;
        }
        n += 1;
    }
    let Some(len) = grant_store::serialize_grants(&entries[..n], &mut out) else {
        return;
    };
    if !login::write_whole(RECORD, &out[..len]) {
        Line::new()
            .s(b"PERMISSION seed failed path=record\n")
            .emit();
    }
}

/// The prompt (D4): the WM's own view and focus, the client held. The lock
/// outranks it: locked, or locking while it is up, is a Deny.
fn ask(
    scene: &mut Scene,
    session: &mut Session,
    focus: &HomeFocus,
    client: Handle,
    escaped: bool,
    action: u16,
    path: &str,
) -> Result<Choice, i32> {
    if lock::locked() {
        log(b"answer=deny reason=locked", action, path);
        return Ok(Choice::Locked);
    }
    if sys_input_focus_activate(focus.activator) != 0 {
        return Err(250);
    }
    super::home::discard_queued(focus, b"permission");
    let mut title = String::from("Arthur wants to ");
    title.push_str(verb(action));
    let rows = alloc::vec![
        title,
        String::from(path),
        String::from("1  Allow once    2  Always    3  Deny"),
    ];
    let screen = scene.geo.screen;
    let (shown, _) = lock::paint_view(
        scene,
        &mut session.plane,
        rows,
        Default::default(),
        false,
        screen,
    );
    shown?;
    // Keys typed while the prompt was drawn are dropped: an answer is a key
    // pressed with the prompt on screen (#248 follow-up).
    super::home::discard_queued(focus, b"permission-drawn");
    log(b"prompt", action, path);
    let choice = loop {
        if lock::service(scene)? {
            break Choice::Locked;
        }
        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        match sys_input_focus_recv(focus.events, &mut event) {
            0 => {}
            EAGAIN => {
                #[cfg(feature = "desktop-pointer")]
                scene.pointer_idle();
                libneo::sys_sched_yield();
                continue;
            }
            _ => return Err(251),
        }
        match keys::decode(event) {
            // Not Enter: a line typed to Arthur ends with one, and a key
            // typed ahead must not allow anything (#248 follow-up).
            Some(Key::Char(b'1')) => break Choice::Once,
            Some(Key::Char(b'2')) => break Choice::Always,
            Some(Key::Char(b'3') | Key::Escape) => break Choice::Deny,
            _ => {}
        }
    };
    // Locked meanwhile: the lock owns the route and gives it back to the
    // client at unlock; the prompt only withdraws.
    // The route goes back to its owner before the prompt (#248 bounce F2):
    // the client, or, escaped, the WM's own focus. Through `swap_to`, so the
    // lock knows the owner to restore.
    if !lock::locked() {
        let owner = if escaped { focus.activator } else { client };
        super::home::swap_to(owner, 252)?;
        Line::new()
            .s(if escaped {
                b"PERMISSION route restored to=wm\n"
            } else {
                b"PERMISSION route restored to=client\n"
            })
            .emit();
        let whole = scene.home.is_none();
        scene.present(whole, screen).map_err(|c| 100 + c)?;
    } else {
        log(b"answer=deny reason=locked", action, path);
    }
    Ok(choice)
}
