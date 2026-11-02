// SPDX-License-Identifier: MPL-2.0
//! OT-66 L1 (`login`): the first-boot account screen, the login screen and
//! the recovery code, as WM chrome on Home (THINK-2026-10-27-ot66 §5, §7).
//!
//! [`gate`] runs before Home is first shown:
//! - no `/neofs/.system` (Try, or a volume nothing seeded): straight on, as
//!   today;
//! - a record that only opens read-only (Try with an installed disk
//!   attached, OT-87 §6.3): straight on; Try never offers to sign in;
//! - no valid record on a writable volume: the first-boot screen;
//! - else the login screen.
//!
//! Every screen is Home's view with a card above the field; a password field
//! paints a dot per character, and its bytes leave the field only into a
//! wiped buffer. The verifier is the WM itself (§3.2). The failure count is
//! persisted (`NEOFAIL1`, login-core `attempts`; Dexter #186 item 2), counted
//! before each check, and reset only by a successful sign-in, unlock or
//! recovery (§4.4). Its files, like the record's, open only for the WM (the
//! kernel's `system-store`, Dexter #186 item 4).

use super::home::{HomeFocus, MAX_INTENT};
use super::{Line, Rect, Scene, wm_rect};
use alloc::string::String;
use alloc::vec::Vec;
use libneo::{
    CLOCK_MONOTONIC, EAGAIN, FileRights, INPUT_EVENT_V1_LEN, OPEN_CREATE_REGULAR,
    OPEN_ROOT_TEMPORARY, sys_clock_gettime, sys_close, sys_fsync, sys_ftruncate, sys_getrandom,
    sys_input_focus_recv, sys_open, sys_read, sys_write,
};
use login_core::{
    self as lc, CODE_LEN, Current, File, KdfParams, SALT_LEN, Slot, SlotRandom, Zeroizing,
};
use neo_atrium::keys::{self, Key};

/// The parameters a new record is written with: the floor (§4.2). The
/// Zenbook and OptiPlex timing lines decide whether a later build raises
/// them; a record keeps its own, so raising them invalidates nothing.
pub const PARAMS: KdfParams = lc::FLOOR;

/// How the session starts.
enum Mode {
    Skip(&'static [u8]),
    FirstBoot,
    SignIn(Current),
}

/// Before Home: decide, then run the screen the boot needs. `Ok` when the
/// session may start.
#[cfg(not(any(feature = "login-kdf-timing", feature = "login-cut-test")))]
pub fn gate(scene: &mut Scene, focus: &HomeFocus) -> Result<(), i32> {
    #[cfg(feature = "login-below-floor-test")]
    seed_below_floor()?;
    match mode() {
        Mode::Skip(reason) => {
            Line::new()
                .s(b"LOGIN skipped reason=")
                .s(reason)
                .s(b"\n")
                .emit();
            Ok(())
        }
        Mode::FirstBoot => {
            begin(scene)?;
            first_boot(scene, focus)?;
            end(scene)?;
            super::lock::arm(focus);
            Ok(())
        }
        Mode::SignIn(current) => {
            begin(scene)?;
            sign_in(scene, focus, current)?;
            end(scene)?;
            super::lock::arm(focus);
            Ok(())
        }
    }
}

/// The timing boot (`login-kdf-timing`) measures and shows, and never
/// signs in. `Err(0)` ends the QEMU check's session.
#[cfg(feature = "login-kdf-timing")]
pub fn gate(scene: &mut Scene, focus: &HomeFocus) -> Result<(), i32> {
    timing::run(scene, focus)
}

/// The cut scenario's driver (`login-cut-test`): no screens. `Err(0)` ends
/// the boot.
#[cfg(all(
    feature = "login-cut-test",
    not(feature = "login-kdf-timing"),
    not(feature = "settings-cut-test")
))]
pub fn gate(_scene: &mut Scene, _focus: &HomeFocus) -> Result<(), i32> {
    cut::run()
}

/// OT-88 S1's cut scenario driver (`settings-cut-test`): no screens; it
/// cuts a settings write instead of a password change.
#[cfg(feature = "settings-cut-test")]
pub fn gate(_scene: &mut Scene, _focus: &HomeFocus) -> Result<(), i32> {
    super::settings::cut::run()
}

/// Boot test (`login_below_floor`): both files hold a slot whose Argon2
/// parameters are under the floor (32 MiB), checksummed and otherwise
/// valid. The gate must refuse both and show the first-boot screen.
#[cfg(feature = "login-below-floor-test")]
fn seed_below_floor() -> Result<(), i32> {
    let slot = Slot {
        flags: lc::FLAG_RECOVERY,
        params: KdfParams {
            memory_kib: 32 * 1024,
            iterations: 3,
            parallelism: 1,
        },
        account_id: [0x10; 16],
        salt: [0x20; SALT_LEN],
        verifier: [0x30; lc::VERIFIER_LEN],
        recovery_salt: [0x40; SALT_LEN],
        recovery_verifier: [0x50; lc::RECOVERY_VERIFIER_LEN],
        generation: 7,
    };
    if !(write_slot(File::A, &slot) && write_slot(File::B, &slot)) {
        return Err(189);
    }
    Line::new()
        .s(b"LOGIN test seeded below-floor m=32768 files=a,b\n")
        .emit();
    Ok(())
}

fn mode() -> Mode {
    match sys_open(lc::SYSTEM_DIR, OPEN_ROOT_TEMPORARY, FileRights::LOOKUP) {
        Ok(dir) => {
            let _ = sys_close(dir);
        }
        Err(_) => return Mode::Skip(b"no-system-dir"),
    }
    let (a, b) = (read_small(lc::CRED_A), read_small(lc::CRED_B));
    for (file, bytes) in [(File::A, &a), (File::B, &b)] {
        if let Some(bytes) = bytes
            && let Err(error) = lc::decode(bytes)
        {
            log_refused(file, error);
        }
    }
    // Opening a record file for writing (or, with none there, creating the
    // first one) is how a read-only volume shows itself, so Try never signs
    // in. On a writable volume a file that holds no valid record never
    // skips the gate: it is first boot.
    let path = match lc::select(a.as_deref(), b.as_deref()) {
        Some(current) => current.file.path(),
        None => lc::CRED_A,
    };
    match open_rw(path) {
        Ok(handle) => {
            let _ = sys_close(handle);
        }
        Err(_) => return Mode::Skip(b"read-only"),
    }
    match lc::select(a.as_deref(), b.as_deref()) {
        Some(current) => Mode::SignIn(current),
        None => Mode::FirstBoot,
    }
}

/// `path` for reading and writing: the file there, or (`ENOENT`) a new one.
/// `OPEN_CREATE_REGULAR` refuses a name that exists.
fn open_rw(path: &str) -> Result<libneo::FileHandle, i64> {
    let rw = FileRights::READ.union(FileRights::WRITE);
    match sys_open(path, OPEN_ROOT_TEMPORARY, rw) {
        Err(libneo::ENOENT) => sys_open(path, OPEN_CREATE_REGULAR, rw),
        other => other,
    }
}

fn file_name(file: File) -> &'static [u8] {
    match file {
        File::A => b"a",
        File::B => b"b",
    }
}

fn log_refused(file: File, error: lc::SlotError) {
    let reason: &[u8] = match error {
        lc::SlotError::Length => b"length",
        lc::SlotError::Magic => b"magic",
        lc::SlotError::Version => b"version",
        lc::SlotError::Checksum => b"checksum",
        lc::SlotError::Reserved => b"reserved",
        lc::SlotError::Flags => b"flags",
        lc::SlotError::Generation => b"generation",
        lc::SlotError::BelowFloor => b"below-floor",
    };
    Line::new()
        .s(b"LOGIN record refused file=")
        .s(file_name(file))
        .s(b" reason=")
        .s(reason)
        .s(b"\n")
        .emit();
}

/// A small file's bytes (up to 512, which no file here reaches), or `None`.
pub(super) fn read_small(path: &str) -> Option<Vec<u8>> {
    read_small_up_to(path, 512)
}

/// A file's first `max` bytes, or `None` (L1.3's grant record is larger
/// than a credential).
pub(super) fn read_small_up_to(path: &str, max: usize) -> Option<Vec<u8>> {
    let handle = sys_open(path, OPEN_ROOT_TEMPORARY, FileRights::READ).ok()?;
    let mut buf = alloc::vec![0u8; max];
    let mut len = 0;
    while len < buf.len() {
        // A read is at most the kernel's MAX_FILE_IO (4 KiB); a larger one
        // is refused whole.
        let end = buf.len().min(len + 4096);
        let n = sys_read(handle, &mut buf[len..end]);
        if n <= 0 {
            break;
        }
        len += n as usize;
    }
    let _ = sys_close(handle);
    buf.truncate(len);
    Some(buf)
}

/// Write `bytes` as the whole of `path` (created if absent) and `fsync`.
pub(super) fn write_whole(path: &str, bytes: &[u8]) -> bool {
    let Ok(handle) = open_rw(path) else {
        return false;
    };
    // A write is at most the kernel's MAX_FILE_IO (4 KiB); a larger one is
    // refused whole (#253 bounce). Then the file ends where the bytes do,
    // whatever its old length.
    let mut ok = bytes
        .chunks(4096)
        .all(|chunk| sys_write(handle.0, chunk.as_ptr(), chunk.len()) == chunk.len() as i64);
    ok = ok && sys_ftruncate(handle, bytes.len() as u64) == 0;
    ok = ok && sys_fsync(handle) == 0;
    let _ = sys_close(handle);
    ok
}

/// Dexter #186 item 2 (J.c., 2026-10-30): the wrong-attempt count, as the
/// `NEOFAIL1` copies hold it for `account` (login-core `attempts`).
pub(super) struct Attempts {
    account: [u8; lc::ACCOUNT_ID_LEN],
    state: lc::attempts::State,
}

impl Attempts {
    pub(super) fn load(account: [u8; lc::ACCOUNT_ID_LEN]) -> Self {
        let (a, b) = (
            read_small(lc::attempts::PATH_A),
            read_small(lc::attempts::PATH_B),
        );
        let state = lc::attempts::select(a.as_deref(), b.as_deref(), &account);
        #[cfg(not(feature = "login-attempts-per-boot-control"))]
        return Self { account, state };
        // Test only: the pre-fix shape, a count that starts at 0 each boot.
        #[cfg(feature = "login-attempts-per-boot-control")]
        Self {
            account,
            state: lc::attempts::State {
                failures: 0,
                ..state
            },
        }
    }

    pub(super) fn failures(&self) -> u32 {
        self.state.failures
    }

    /// Make the count `failures`, durably (written and flushed to the copy
    /// not holding the current one). `false`: it couldn't be, and nothing
    /// may be checked (fail closed).
    pub(super) fn set(&mut self, failures: u32) -> bool {
        let Some(rec) = lc::attempts::next(&self.state, &self.account, failures) else {
            return false;
        };
        let target = self.state.next;
        if !write_whole(lc::attempts::path(target), &lc::attempts::encode(&rec)) {
            Line::new().s(b"LOGIN attempts write=failed\n").emit();
            return false;
        }
        self.state = lc::attempts::State {
            failures,
            generation: rec.generation,
            next: target.other(),
        };
        true
    }

    /// The reset after a successful sign-in, unlock or recovery (Dexter #187
    /// G1). It fails safe (a count left on disk only charges a delay), but
    /// the user doesn't owe that delay: retry, and log a reset that still
    /// failed, with where it happened.
    pub(super) fn reset(&mut self, at: &[u8]) {
        for _ in 0..RESET_TRIES {
            if self.set(0) {
                return;
            }
        }
        Line::new()
            .s(b"LOGIN attempts reset=failed at=")
            .s(at)
            .s(b" failures=")
            .n(u64::from(self.failures()))
            .s(b"\n")
            .emit();
    }

    /// Count an attempt before checking it: a cut after this write costs the
    /// guess, and one before it means nothing was checked.
    pub(super) fn begin(&mut self) -> bool {
        self.set(self.state.failures.saturating_add(1))
    }
}

/// How many times [`Attempts::reset`] writes before it logs a failure.
const RESET_TRIES: u32 = 3;

fn now_ns() -> u64 {
    sys_clock_gettime(CLOCK_MONOTONIC).unwrap_or(0)
}

/// `N` bytes from `sys_getrandom` (it may return fewer per call).
fn random<const N: usize>() -> Result<[u8; N], i32> {
    let mut out = [0u8; N];
    let mut filled = 0;
    let mut tries = 0;
    while filled < N {
        let n = sys_getrandom(&mut out[filled..]);
        if n > 0 {
            filled += n as usize;
        } else {
            tries += 1;
            if tries > 64 {
                return Err(180);
            }
        }
    }
    Ok(out)
}

/// One Argon2id derivation's cost, logged (§4.2's tuning line).
fn log_kdf(params: KdfParams, started: u64) {
    let mut line = Vec::new();
    lc::kdf_line(
        params,
        now_ns().saturating_sub(started) / 1_000_000,
        &mut line,
    );
    let _ = sys_write(1, line.as_ptr(), line.len());
}

fn verify_password(slot: &Slot, password: &[u8]) -> bool {
    let started = now_ns();
    let ok = lc::verify_password(slot, password);
    log_kdf(slot.params, started);
    ok
}

fn verify_recovery(slot: &Slot, code: &[u8; CODE_LEN]) -> bool {
    let started = now_ns();
    let ok = lc::verify_recovery(slot, code);
    log_kdf(slot.params, started);
    ok
}

/// A slot's two derivations, each timed.
fn new_slot(
    account_id: [u8; 16],
    password: &[u8],
    code: &[u8; CODE_LEN],
    generation: u64,
) -> Result<Slot, i32> {
    let random = SlotRandom {
        salt: random::<SALT_LEN>()?,
        recovery_salt: random::<SALT_LEN>()?,
    };
    let started = now_ns();
    let slot = lc::new_slot(account_id, password, code, &random, PARAMS, generation).ok_or(181)?;
    log_kdf(PARAMS, started);
    Ok(slot)
}

/// A recovery code for the record of `generation`: 20 symbols from
/// `sys_getrandom`. The boot test types its codes, so it takes them from a
/// fixed list by generation instead (test builds only; the salts and the
/// account id stay random).
fn issue_code(generation: u64) -> Result<[u8; CODE_LEN], i32> {
    #[cfg(feature = "desktop-session-boot-test")]
    {
        const CODES: [&[u8; CODE_LEN]; 2] = [b"7K9QM2XVD4HJ8RTWB6NE", b"R8ZW4N2KXQ7HV3TMD9JP"];
        Ok(*CODES[(generation.saturating_sub(1) % 2) as usize])
    }
    #[cfg(not(feature = "desktop-session-boot-test"))]
    {
        let _ = generation;
        Ok(lc::recovery_code(&random::<CODE_LEN>()?))
    }
}

/// Write `slot` to `file` only: the change's one write (§4.3).
pub(super) fn write_slot(file: File, slot: &Slot) -> bool {
    write_whole(file.path(), &lc::encode(slot))
}

fn display_name() -> Option<String> {
    let bytes = read_small(lc::META)?;
    lc::decode_meta(&bytes).map(String::from)
}

/// OT-66 L2: the name the lock screen shows.
pub(super) fn name() -> String {
    display_name().unwrap_or_else(|| String::from("LogansOS"))
}

/// OT-66 L2: the account's record as the files hold it now (`None`: no
/// valid record), for unlock.
pub(super) fn current() -> Option<Current> {
    let (a, b) = (read_small(lc::CRED_A), read_small(lc::CRED_B));
    lc::select(a.as_deref(), b.as_deref())
}

/// OT-66 L2: the sign-in check (the KDF, and its `LOGIN kdf` line).
pub(super) fn check_password(slot: &Slot, password: &[u8]) -> bool {
    verify_password(slot, password)
}

// ---- The screens --------------------------------------------------------

fn begin(scene: &mut Scene) -> Result<(), i32> {
    scene.chrome.set_intro(true);
    scene.show_home(true, false);
    if let Some(home) = scene.home.as_mut() {
        home.masked = false;
    }
    scene
        .present(false, scene.geo.screen)
        .map(|_| ())
        .map_err(|c| 100 + c)
}

/// Leave Home as `run()` expects it: no card, an empty, unmasked field.
fn end(scene: &mut Scene) -> Result<(), i32> {
    scene.chrome.set_card(Vec::new());
    if let Some(home) = scene.home.as_mut() {
        home.line.wipe();
        home.masked = false;
    }
    let r = damage(scene);
    scene.repaint_chrome(r);
    scene.present(false, r).map(|_| ()).map_err(|c| 100 + c)
}

/// The field and every card size.
fn damage(scene: &Scene) -> Rect {
    let layout = &scene.chrome.layout;
    wm_rect(layout.card(neo_atrium::layout::CARD_MAX_ROWS)).union(&wm_rect(layout.field_glow))
}

/// Show `rows` on the card, the field empty; `masked`: a password field.
/// The boot test logs `LOGIN prompt=<id>` once it is on screen (its key
/// script waits for it).
fn show(scene: &mut Scene, id: &[u8], rows: &[&str], masked: bool) -> Result<(), i32> {
    scene
        .chrome
        .set_card(rows.iter().map(|r| String::from(*r)).collect());
    let home = scene.home.as_mut().ok_or(182)?;
    home.line.wipe();
    home.masked = masked;
    let r = damage(scene);
    scene.repaint_chrome(r);
    scene.present(false, r).map_err(|c| 100 + c)?;
    #[cfg(feature = "desktop-session-boot-test")]
    Line::new().s(b"LOGIN prompt=").s(id).s(b"\n").emit();
    #[cfg(not(feature = "desktop-session-boot-test"))]
    let _ = id;
    Ok(())
}

/// Wait for input (or `deadline_ns`), keeping the pointer route drained.
fn idle(scene: &mut Scene, focus: &HomeFocus, deadline_ns: u64) {
    #[cfg(feature = "desktop-pointer")]
    scene.pointer_idle();
    #[cfg(feature = "desktop-park")]
    scene.park(
        &[libneo::WaitItem {
            handle: focus.events.0,
            kind: libneo::WAIT_KIND_INPUT,
        }],
        deadline_ns,
    );
    #[cfg(not(feature = "desktop-park"))]
    {
        let _ = (focus, deadline_ns);
        core::hint::spin_loop();
    }
}

enum Answer {
    Text(Zeroizing<Vec<u8>>),
    Tab,
}

/// Edit the field until Enter (on a non-empty field, unless `allow_empty`),
/// or Tab when `allow_tab`. The text leaves the field into a wiped buffer.
fn answer(
    scene: &mut Scene,
    focus: &HomeFocus,
    allow_empty: bool,
    allow_tab: bool,
) -> Result<Answer, i32> {
    let field = wm_rect(scene.chrome.layout.field);
    loop {
        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        match sys_input_focus_recv(focus.events, &mut event) {
            0 => {}
            EAGAIN => {
                idle(scene, focus, u64::MAX);
                continue;
            }
            _ => return Err(183),
        }
        let Some(key) = keys::decode(event) else {
            continue;
        };
        let home = scene.home.as_mut().ok_or(184)?;
        match key {
            Key::Enter if allow_empty || !home.line.is_empty() => {
                let text = Zeroizing::new(home.line.as_bytes().to_vec());
                home.line.wipe();
                scene.repaint_chrome(field);
                scene.present(false, field).map_err(|c| 100 + c)?;
                return Ok(Answer::Text(text));
            }
            Key::Tab if allow_tab => return Ok(Answer::Tab),
            _ => {
                if home.line.edit(key) {
                    scene.repaint_chrome(field);
                    scene.present(false, field).map_err(|c| 100 + c)?;
                }
            }
        }
    }
}

/// The backoff before a try is checked: the card says so, and keys typed
/// meanwhile are dropped.
fn wait_out(scene: &mut Scene, focus: &HomeFocus, secs: u32) -> Result<(), i32> {
    if secs == 0 {
        return Ok(());
    }
    let mut row = String::from("Too many tries. Wait ");
    push_num(&mut row, u64::from(secs));
    row.push_str(if secs == 1 { " second." } else { " seconds." });
    show(scene, b"wait", &[row.as_str()], true)?;
    Line::new()
        .s(b"LOGIN backoff wait_s=")
        .n(u64::from(secs))
        .s(b"\n")
        .emit();
    let until = now_ns().saturating_add(u64::from(secs) * 1_000_000_000);
    while now_ns() < until {
        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        if sys_input_focus_recv(focus.events, &mut event) == EAGAIN {
            idle(scene, focus, until);
        }
    }
    Ok(())
}

fn push_num(out: &mut String, mut v: u64) {
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
    for &d in digits[..n].iter().rev() {
        out.push(d as char);
    }
}

/// §2: what the password does and doesn't protect, said where it's chosen.
const PROTECTS: [&str; 3] = [
    "It protects your files while LogansOS runs.",
    "Anyone who starts this computer from another",
    "USB stick or system can still read them.",
];

/// Ask for a new password twice, until both match.
fn choose_password(scene: &mut Scene, focus: &HomeFocus) -> Result<Zeroizing<Vec<u8>>, i32> {
    let mut note: Option<&str> = None;
    loop {
        let mut rows = alloc::vec!["Choose a password for this computer."];
        rows.extend(PROTECTS);
        rows.extend(note);
        show(scene, b"choose", &rows, true)?;
        let Answer::Text(first) = answer(scene, focus, false, false)? else {
            continue;
        };
        show(scene, b"again", &["Type the same password again."], true)?;
        let Answer::Text(second) = answer(scene, focus, false, false)? else {
            continue;
        };
        if lc::constant_time_eq(&first, &second) {
            return Ok(first);
        }
        Line::new().s(b"LOGIN password mismatch\n").emit();
        note = Some("Those didn't match. Try again.");
    }
}

/// Show a new recovery code until the user says it's written down.
fn show_code(scene: &mut Scene, focus: &HomeFocus, code: &[u8; CODE_LEN]) -> Result<(), i32> {
    let shown = lc::format_recovery(code);
    let shown = core::str::from_utf8(&shown).unwrap_or("");
    show(
        scene,
        b"code",
        &[
            "Your recovery code:",
            shown,
            "Write it down and keep it somewhere safe.",
            "It's the only way back in if you forget",
            "your password. Press Enter when it's written down.",
        ],
        false,
    )?;
    Line::new().s(b"LOGIN recovery code shown\n").emit();
    let _ = answer(scene, focus, true, false)?;
    Ok(())
}

/// §7: the account, created on the installed system's first boot.
fn first_boot(scene: &mut Scene, focus: &HomeFocus) -> Result<(), i32> {
    Line::new().s(b"LOGIN first-boot shown\n").emit();
    // The below-floor check ends once the record has been refused.
    if cfg!(feature = "login-below-floor-test") {
        return Err(0);
    }
    let name = loop {
        show(
            scene,
            b"name",
            &[
                "Welcome to LogansOS.",
                "What should I call you?",
                "Type your name, then press Enter.",
            ],
            false,
        )?;
        if let Answer::Text(text) = answer(scene, focus, false, false)?
            && let Ok(name) = core::str::from_utf8(&text)
            && let Some(meta) = lc::encode_meta(name)
        {
            break meta;
        }
    };
    let password = choose_password(scene, focus)?;
    let code = issue_code(1)?;
    show_code(scene, focus, &code)?;
    show(scene, b"setup", &["Setting up your account..."], false)?;
    let slot = new_slot(random::<16>()?, &password, &code, 1)?;
    drop(password);
    // Both files hold the first record; a change then writes only the
    // other one (§4.3).
    if !(write_slot(File::A, &slot) && write_slot(File::B, &slot) && write_whole(lc::META, &name)) {
        Line::new().s(b"LOGIN account write=failed\n").emit();
        return Err(185);
    }
    Line::new()
        .s(b"LOGIN account created generation=1 files=a,b,meta\n")
        .emit();
    Ok(())
}

/// §5.1: the password, or Tab for the recovery code.
/// F9 (J.c., 2026-11-02): what a wrong password shows, at sign-in and on the
/// lock screen. It says nothing about which part was wrong.
pub(super) const WRONG_PASSWORD: &str = "Incorrect password. Try again.";

fn sign_in(scene: &mut Scene, focus: &HomeFocus, mut current: Current) -> Result<(), i32> {
    let name = display_name().unwrap_or_else(|| String::from("LogansOS"));
    // Dexter #186 item 2: the count a reboot doesn't reset.
    let mut attempts = Attempts::load(current.slot.account_id);
    Line::new()
        .s(b"LOGIN screen shown failures=")
        .n(u64::from(attempts.failures()))
        .s(b"\n")
        .emit();
    let mut note: Option<&str> = None;
    loop {
        let mut rows = alloc::vec![
            name.as_str(),
            "Enter your password.",
            "Forgot it? Press Tab for your recovery code."
        ];
        // F9: the note is the last row, the one nearest the field.
        rows.extend(note);
        show(scene, b"password", &rows, true)?;
        match answer(scene, focus, false, true)? {
            Answer::Tab => {
                if recover(scene, focus, &mut current, &mut attempts)? {
                    return Ok(());
                }
                note = None;
            }
            Answer::Text(password) => {
                let failures = attempts.failures();
                wait_out(scene, focus, lc::delay_secs(failures))?;
                if !attempts.begin() {
                    return Err(188);
                }
                show(scene, b"checking", &["Checking..."], true)?;
                if verify_password(&current.slot, &password) {
                    // Only a successful sign-in (or recovery) resets it.
                    attempts.reset(b"sign-in");
                    Line::new()
                        .s(b"LOGIN signed in failures=")
                        .n(u64::from(failures))
                        .s(b"\n")
                        .emit();
                    return Ok(());
                }
                log_failure(b"password", attempts.failures());
                note = Some(WRONG_PASSWORD);
                #[cfg(feature = "desktop-session-boot-test")]
                Line::new()
                    .s(b"LOGIN note=\"")
                    .s(WRONG_PASSWORD.as_bytes())
                    .s(b"\" row=last\n")
                    .emit();
                #[cfg(feature = "login-backoff-test")]
                if attempts.failures() == BACKOFF_TEST_STOP {
                    Line::new().s(b"LOGIN backoff test stop\n").emit();
                    libneo::sys_exit(0);
                }
            }
        }
    }
}

fn log_failure(what: &[u8], failures: u32) {
    Line::new()
        .s(b"LOGIN ")
        .s(what)
        .s(b" refused failures=")
        .n(u64::from(failures))
        .s(b" next_delay_s=")
        .n(u64::from(lc::delay_secs(failures)))
        .s(b"\n")
        .emit();
}

/// §4.4: the recovery code, then a new password and a new code. `true`
/// once the password is reset (the session may start); `false` for Tab
/// (back to the password).
fn recover(
    scene: &mut Scene,
    focus: &HomeFocus,
    current: &mut Current,
    attempts: &mut Attempts,
) -> Result<bool, i32> {
    let mut note: Option<&str> = None;
    loop {
        let mut rows = alloc::vec!["Type your recovery code.", "Press Tab to go back."];
        rows.extend(note);
        show(scene, b"recovery", &rows, false)?;
        let Answer::Text(typed) = answer(scene, focus, false, true)? else {
            return Ok(false);
        };
        wait_out(scene, focus, lc::delay_secs(attempts.failures()))?;
        if !attempts.begin() {
            return Err(188);
        }
        let accepted = match lc::parse_recovery(&typed) {
            Some(code) => {
                show(scene, b"checking", &["Checking..."], false)?;
                verify_recovery(&current.slot, &code)
            }
            None => false,
        };
        if !accepted {
            log_failure(b"recovery", attempts.failures());
            note = Some("Incorrect recovery code. Try again.");
            continue;
        }
        attempts.reset(b"recovery");
        Line::new().s(b"LOGIN recovery accepted\n").emit();
        let password = choose_password(scene, focus)?;
        let code = issue_code(current.slot.generation + 1)?;
        show_code(scene, focus, &code)?;
        show(scene, b"saving", &["Saving your new password..."], false)?;
        let changed = change(current, &password, &code)?;
        *current = changed;
        return Ok(true);
    }
}

/// Replace the record: a new slot, generation + 1, written to the file not
/// holding the record (only that file). Returns the new record.
pub(super) fn change(
    current: &Current,
    password: &[u8],
    code: &[u8; CODE_LEN],
) -> Result<Current, i32> {
    let generation = current.slot.generation.checked_add(1).ok_or(186)?;
    let slot = new_slot(current.slot.account_id, password, code, generation)?;
    let target = current.write_target();
    if !write_slot(target, &slot) {
        Line::new().s(b"LOGIN password reset write=failed\n").emit();
        return Err(187);
    }
    Line::new()
        .s(b"LOGIN password reset generation=")
        .n(generation)
        .s(b" file=")
        .s(file_name(target))
        .s(b"\n")
        .emit();
    Ok(Current { file: target, slot })
}

/// The longest field (Home's).
const _: () = assert!(MAX_INTENT >= lc::CODE_SHOWN_LEN);

/// OT-66 L1 timing (`login-kdf-timing`): one sign-in derivation at the
/// floor, three times, shown on the card and logged, for §4.2's choice of
/// shipped parameters on the Zenbook (image A) and the OptiPlex. No account
/// is made and nothing is opened for writing: the boot touches no disk.
#[cfg(feature = "login-kdf-timing")]
mod timing {
    use super::*;

    pub fn run(scene: &mut Scene, focus: &HomeFocus) -> Result<(), i32> {
        begin(scene)?;
        show(
            scene,
            b"timing",
            &["Measuring sign-in cost (no account is made)..."],
            false,
        )?;
        let salt = *b"LogansOStiming!!";
        let mut runs = [0u64; 3];
        for run in &mut runs {
            let started = now_ns();
            lc::derive_verifier(b"timing only", &salt, PARAMS).ok_or(188)?;
            *run = now_ns().saturating_sub(started) / 1_000_000;
            log_kdf(PARAMS, started);
        }
        let mut sorted = runs;
        sorted.sort_unstable();
        let mut rows: Vec<String> = Vec::new();
        rows.push(String::from("Sign-in cost at m:65536,t:3,p:1"));
        for (i, ms) in runs.iter().enumerate() {
            let mut row = String::from("run ");
            push_num(&mut row, i as u64 + 1);
            row.push_str(": ");
            push_num(&mut row, *ms);
            row.push_str(" ms");
            rows.push(row);
        }
        let mut row = String::from("median: ");
        push_num(&mut row, sorted[1]);
        row.push_str(" ms");
        rows.push(row);
        rows.push(String::from("No disk was written. Power off when read."));
        let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        show(scene, b"timing-result", &refs, false)?;
        Line::new()
            .s(b"LOGIN kdf timing median_ms=")
            .n(sorted[1])
            .s(b"\n")
            .emit();
        // The QEMU check ends here; on hardware the card stays up.
        #[cfg(feature = "desktop-session-boot-test")]
        {
            let _ = focus;
            Err(0)
        }
        #[cfg(not(feature = "desktop-session-boot-test"))]
        loop {
            idle(scene, focus, u64::MAX);
            let mut event = [0u8; INPUT_EVENT_V1_LEN];
            let _ = sys_input_focus_recv(focus.events, &mut event);
        }
    }
}

/// OT-66 L1's cut scenario driver (`login-cut-test`, `login_cut_steps`):
/// the password change §4.3 guards, with a power cut after each of its
/// device writes and flushes. One build, four kinds of boot, told apart by
/// the record and the boot's cut step (`SYS_TEST_BLOCK_CUT(0)`, from the
/// boot module xtask rewrites):
/// - **setup** (no record): the account with [`OLD`], as first boot writes
///   it (both files, generation 1);
/// - **verify** (step 0): which of [`OLD`] and [`NEW`] the record accepts,
///   and each file's BLAKE3;
/// - **change** (step k): arm the cut at the change's k-th write or flush,
///   then change [`OLD`] to [`NEW`] through [`change`], recovery's own
///   path. The kernel ends the boot at step k; reaching the end logs
///   `not-reached` and the change's op count (the calibration boot).
#[cfg(feature = "login-cut-test")]
mod cut {
    use super::*;

    pub const OLD: &[u8] = b"old-password";
    pub const NEW: &[u8] = b"new-password";
    const CODE: &[u8; CODE_LEN] = b"7K9QM2XVD4HJ8RTWB6NE";

    fn hex(mut line: Line, digest: &[u8; 32]) -> Line {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for &b in digest {
            line = line.s(&[HEX[usize::from(b >> 4)], HEX[usize::from(b & 15)]]);
        }
        line
    }

    pub fn run() -> Result<(), i32> {
        let step = libneo::sys_test_block_cut(0);
        let (a, b) = (read_small(lc::CRED_A), read_small(lc::CRED_B));
        let Some(current) = lc::select(a.as_deref(), b.as_deref()) else {
            let slot = new_slot([0x5a; 16], OLD, CODE, 1)?;
            if !(write_slot(File::A, &slot) && write_slot(File::B, &slot)) {
                return Err(190);
            }
            Line::new()
                .s(b"LOGIN cut setup created generation=1\n")
                .emit();
            return Err(0);
        };
        if step == 0 {
            let old = verify_password(&current.slot, OLD);
            let new = verify_password(&current.slot, NEW);
            let line = Line::new()
                .s(b"LOGIN cut verify record=")
                .s(file_name(current.file))
                .s(b" generation=")
                .n(current.slot.generation)
                .s(b" old=")
                .n(u64::from(old))
                .s(b" new=")
                .n(u64::from(new))
                .s(b" a_blake3=");
            let line = hex(line, &lc::digest(a.as_deref().unwrap_or(&[]))).s(b" b_blake3=");
            hex(line, &lc::digest(b.as_deref().unwrap_or(&[])))
                .s(b"\n")
                .emit();
            return Err(0);
        }
        if !verify_password(&current.slot, OLD) {
            return Err(191);
        }
        Line::new()
            .s(b"LOGIN cut change step=")
            .n(step as u64)
            .s(b" record=")
            .s(file_name(current.file))
            .s(b" target=")
            .s(file_name(current.write_target()))
            .s(b"\n")
            .emit();
        if libneo::sys_test_block_cut(1) != step {
            return Err(192);
        }
        change(&current, NEW, CODE)?;
        let ops = libneo::sys_test_block_cut(2);
        Line::new()
            .s(b"LOGIN cut not-reached step=")
            .n(step as u64)
            .s(b" ops=")
            .n(ops.max(0) as u64)
            .s(b"\n")
            .emit();
        Err(0)
    }
}

/// `login_backoff_reboot`: the boot ends (neo-init exits 0, the scenario's
/// pass) once this many wrong passwords are counted, so the next boot starts
/// from the persisted count.
#[cfg(feature = "login-backoff-test")]
const BACKOFF_TEST_STOP: u32 = 4;

/// `login_record_guard` (Dexter #186 item 4): the WM still reaches its record
/// (it holds the kernel's `SystemStore` cap), and a process without it gets
/// `EPERM` for every `.system/` open (its own `RECORDGUARD` line). OT-66
/// K1b: it is spawned as a client holding a RootDirCap for all of `/`, so
/// its `/neofs` control opens and only the `.system/` rule refuses it.
#[cfg(feature = "login-record-guard-test")]
pub fn record_guard_test() {
    let before = (read_small(lc::CRED_A), read_small(lc::CRED_B));
    let wm = match sys_open(lc::CRED_A, OPEN_ROOT_TEMPORARY, FileRights::READ) {
        Ok(handle) => {
            let _ = sys_close(handle);
            &b"ok"[..]
        }
        Err(_) => &b"refused"[..],
    };
    Line::new()
        .s(b"LOGIN record guard wm_open=")
        .s(wm)
        .s(b"\n")
        .emit();
    let exit = match libneo::sys_spawn_client("worker", &[]) {
        Ok((_pid, process)) => libneo::sys_wait(process, 0).unwrap_or(-1),
        Err(_) => -2,
    };
    let code: &[u8] = match exit {
        0 => b"0",
        _ => b"nonzero",
    };
    Line::new()
        .s(b"LOGIN record guard probe exit=")
        .s(code)
        .s(b"\n")
        .emit();
    // Dexter #187 qualifier B: whatever the probe managed (a write, a
    // `SYS_FILE_REPLACE`, a create), both copies are byte for byte as before.
    let after = (read_small(lc::CRED_A), read_small(lc::CRED_B));
    let unchanged = before.0.is_some() && before == after;
    Line::new()
        .s(b"LOGIN record guard unchanged=")
        .n(u64::from(unchanged))
        .s(b"\n")
        .emit();
}
