// SPDX-License-Identifier: MPL-2.0
//! OT-66 L1: the sign-in scenarios on the R1 VM layout (OVMF, `boot.raw`,
//! `data.raw` seeded with an empty `.system/`), one QEMU boot after another on
//! the same data disk (THINK-2026-10-27-ot66 §8, the L1 close gate):
//! - `login_two_boot`: the first boot creates the account; the second signs
//!   in after three wrong passwords and the backoff; the third resets the
//!   password with the recovery code; the fourth refuses the old password
//!   and the old code and signs in with the new password.
//! - `login_below_floor`: a record under the KDF floor is refused, and the
//!   first-boot screen shows instead.
//! - `login_record_guard` (Dexter #186 item 4): first boot creates the
//!   account; the WM opens its record, and a no-caps process gets EPERM for
//!   the record (read and write), a `..` path, the directory and a new file
//!   in it, while `/neofs` itself still opens for it.
//! - `login_backoff_reboot` (Dexter #186 item 2, J.c.): the first boot
//!   creates the account; the second ends after four wrong passwords; the
//!   third starts from that count, waits out its backoff (2 s) and signs in.
//! - `login_cut_steps`: a power cut after each device write and flush of a
//!   password change leaves exactly the old or the new password, and the
//!   file the change doesn't write keeps its bytes (BLAKE3).
//!
//! OT-66 L2, lock and unlock (THINK §5.2, the L2 close gate), one boot each:
//! - `login_lock_session`: Super+L locks Home; Arthur is spawned under the
//!   lock, draws a full-surface picture and asks for its busy emblem, and
//!   the screen doesn't change (two screendumps, byte-equal); keys typed
//!   while locked reach no client; a wrong password is refused, the right one
//!   unlocks to Arthur, whose picture now shows, and whose first key is the
//!   one typed after unlock. Then Arthur holds the focus and never yields,
//!   committing a new picture every 300 ms: Super+L locks it anyway, and the
//!   screen doesn't change while it commits.
//! - `login_lock_idle`: keys 3 s apart keep the session unlocked; 6 s
//!   after the last one it locks (`lock-idle-test`'s idle time).
//! - `login_lock_stall` (AAG F3): Super+L while Arthur stalls in its
//!   handshake locks during the stall.
//! - `login_lock_park` (L2b, kernel `sak-lock-route`): Super+L while the WM
//!   is parked in a send to Arthur locks the kernel's route at once; the key
//!   typed after it never reaches Arthur, and the lock screen follows.
//! - `login_terminal_lock` (L1a.3, `terminal_lock_park` in
//!   THINK-2026-10-31-l1a §6): in the terminal window, cooked mode, the child
//!   reporting what it reads: `ab`, Super+L, `x` and Enter (the lock's
//!   password field: a wrong password), the password, unlock, `c` and Enter.
//!   The child receives exactly `abc\n`.

use super::*;

pub const LOGIN_TWO_BOOT_SCENARIO: &str = "login_two_boot";
pub const LOGIN_RECORD_GUARD_SCENARIO: &str = "login_record_guard";
pub const LOGIN_BACKOFF_REBOOT_SCENARIO: &str = "login_backoff_reboot";
pub const LOGIN_BELOW_FLOOR_SCENARIO: &str = "login_below_floor";
pub const LOGIN_LOCK_SESSION_SCENARIO: &str = "login_lock_session";
pub const LOGIN_LOCK_IDLE_SCENARIO: &str = "login_lock_idle";
pub const LOGIN_LOCK_STALL_SCENARIO: &str = "login_lock_stall";
pub const LOGIN_LOCK_PARK_SCENARIO: &str = "login_lock_park";
pub const LOGIN_TERMINAL_LOCK_SCENARIO: &str = "login_terminal_lock";
pub const LOGIN_SWITCH_LOCK_SCENARIO: &str = "login_switch_lock";
pub const SETTINGS_STORE_SCENARIO: &str = "login_settings_store";
pub const SETTINGS_CUT_SCENARIO: &str = "login_settings_cut";
pub const SETTINGS_REFUSED_SCENARIO: &str = "login_settings_refused";
pub const ARTHUR_PERMISSION_SCENARIO: &str = "login_arthur_permission";
pub const LOGIN_SETUP_STORE_SCENARIO: &str = "login_setup_store";
pub const LOGIN_SETUP_TZ_SCENARIO: &str = "login_setup_tz";
pub const LOGIN_SETUP_TZ_SKIP_SCENARIO: &str = "login_setup_tz_skip";
pub const LOGIN_SETUP_TZ_REFUSED_SCENARIO: &str = "login_setup_tz_refused";
pub const LOGIN_SETUP_TZ_WRITE_FAIL_SCENARIO: &str = "login_setup_tz_write_fail";
pub const LOGIN_SETUP_TZ_FSYNC_FAIL_SCENARIO: &str = "login_setup_tz_fsync_fail";
pub const LOGIN_SETUP_TZ_ROLLBACK_FAIL_SCENARIO: &str = "login_setup_tz_rollback_fail";
pub const LOGIN_SETUP_TZ_REREAD_FAIL_SCENARIO: &str = "login_setup_tz_reread_fail";

/// F11 S3's RTC (`-rtc base=`): 01:57 local time in New York on the
/// fall-back day, inside the repeated hour (01:00-01:59 happens twice), three
/// minutes before 02:00 so the boot and the walk stay inside it. Read as
/// local time it is the earlier instant, 05:57Z.
pub const SETUP_TZ_RTC_BASE: &str = "2026-11-01T01:57:00";
/// That reading's UTC instant (EDT, the earlier one): 2026-11-01T05:57:00Z.
const SETUP_TZ_UTC_S: u64 = 1_793_512_620;
/// The same reading taken as UTC (four hours behind): what a record made
/// before the confirm would carry if the wizard didn't correct it.
const SETUP_TZ_RTC_AS_UTC_S: u64 = 1_793_498_220;
/// `America/New_York`'s id in libneo's tz table (tz-logic-tests pins it).
const NEW_YORK_ID: u16 = 105;

pub fn is_login_vm_scenario(name: &str) -> bool {
    matches!(
        name,
        LOGIN_TWO_BOOT_SCENARIO
            | LOGIN_RECORD_GUARD_SCENARIO
            | LOGIN_BACKOFF_REBOOT_SCENARIO
            | LOGIN_BELOW_FLOOR_SCENARIO
            | LOGIN_CUT_SCENARIO
            | LOGIN_LOCK_SESSION_SCENARIO
            | LOGIN_LOCK_IDLE_SCENARIO
            | LOGIN_LOCK_STALL_SCENARIO
            | LOGIN_LOCK_PARK_SCENARIO
            | LOGIN_TERMINAL_LOCK_SCENARIO
            | LOGIN_SWITCH_LOCK_SCENARIO
            | SETTINGS_STORE_SCENARIO
            | SETTINGS_CUT_SCENARIO
            | SETTINGS_REFUSED_SCENARIO
            | ARTHUR_PERMISSION_SCENARIO
            | LOGIN_SETUP_STORE_SCENARIO
            | LOGIN_SETUP_TZ_SCENARIO
            | LOGIN_SETUP_TZ_SKIP_SCENARIO
            | LOGIN_SETUP_TZ_REFUSED_SCENARIO
            | LOGIN_SETUP_TZ_WRITE_FAIL_SCENARIO
            | LOGIN_SETUP_TZ_FSYNC_FAIL_SCENARIO
            | LOGIN_SETUP_TZ_ROLLBACK_FAIL_SCENARIO
            | LOGIN_SETUP_TZ_REREAD_FAIL_SCENARIO
    )
}

/// The key script of the next QEMU boot (`serial_script_for_scenario` reads
/// it first), so each boot of one scenario types its own.
pub static SCRIPT_OVERRIDE: std::sync::Mutex<Option<&'static [SerialScriptStep]>> =
    std::sync::Mutex::new(None);

const P1: &[u8] = b"first-pass\n";
const P2: &[u8] = b"second-pass\n";

/// Boot 1: the first-boot screen. A mismatched second entry is caught, then
/// the account is made and the recovery code acknowledged.
const BOOT1: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", b"first-pazz\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
];

/// `login_record_guard`: first boot, as boot 1 without the mismatch.
const RECORD_GUARD: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
];

/// `login_backoff_reboot`, boot 2: four wrong passwords (the test build ends
/// the boot at the fourth).
const BACKOFF_BOOT2: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=password", b"wrong-one\n"),
    step(b"LOGIN prompt=password", b"wrong-two\n"),
    step(b"LOGIN prompt=password", b"wrong-three\n"),
    step(b"LOGIN prompt=password", b"wrong-four\n"),
];

/// Boot 3: the right password, which must wait out the persisted backoff.
const BACKOFF_BOOT3: &[SerialScriptStep] = &[step(b"LOGIN prompt=password", P1)];

/// Boot 2: three wrong passwords, then the right one, which waits out the
/// backoff (1 s after the third failure) before it is checked.
const BOOT2: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=password", b"wrong-one\n"),
    step(b"LOGIN prompt=password", b"wrong-two\n"),
    step(b"LOGIN prompt=password", b"wrong-three\n"),
    step(b"LOGIN prompt=password", P1),
];

/// Boot 3: Tab to the recovery code, a wrong code, the right one (typed in
/// lower case, as the parse folds it), a new password, the new code.
const BOOT3: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=password", b"\t"),
    step(b"LOGIN prompt=recovery", b"7k9qm-2xvd4-hj8rt-wb6nf\n"),
    step(b"LOGIN prompt=recovery", b"7k9qm-2xvd4-hj8rt-wb6ne\n"),
    step(b"LOGIN prompt=choose", P2),
    step(b"LOGIN prompt=again", P2),
    step(b"LOGIN prompt=code", b"\n"),
];

/// Boot 4: the used code is refused (a new one was issued), Tab back, the
/// old password is refused, the new one signs in.
const BOOT4: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=password", b"\t"),
    step(b"LOGIN prompt=recovery", b"7k9qm-2xvd4-hj8rt-wb6ne\n"),
    step(b"LOGIN prompt=recovery", b"\t"),
    step(b"LOGIN prompt=password", P1),
    step(b"LOGIN prompt=password", P2),
];

const fn cap(n: u8) -> u8 {
    HMP_CAPTURE_0 + n
}
const PAUSE: u8 = HMP_PAUSE_1S;

/// `login_lock_session`: BOOT1's account, then the two locks.
const LOCK_SESSION: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
    // Lock Home (the WM's own focus has the keyboard).
    step(b"LOGIN session started view=home", &[PAUSE, HMP_SUPER_L]),
    // The lock screen, before Arthur (spawned under the lock) draws.
    step(b"LOCK prompt=password", &[cap(0)]),
    // Arthur drew and asked for its emblem: 2 s later the screen is the
    // same. Then a wrong password (keys no client may see).
    step(
        b"DESKTOP arthur lock probe committed picture=0",
        &[PAUSE, PAUSE, cap(1), b'n', b'o', b'p', b'e', b'\n'],
    ),
    step(b"LOCK prompt=password", P1),
    // Unlocked to Arthur: its picture shows; its first key is this one.
    step(b"LOCK unlocked", &[PAUSE, cap(2), b'q']),
    // Arthur now holds the focus and never yields, committing every 300 ms.
    step(
        b"DESKTOP arthur lock probe committed picture=1",
        &[HMP_SUPER_L],
    ),
    step(
        b"LOCK prompt=password",
        &[
            PAUSE,
            cap(3),
            PAUSE,
            PAUSE,
            cap(4),
            b'f',
            b'i',
            b'r',
            b's',
            b't',
            b'-',
            b'p',
            b'a',
            b's',
            b's',
            b'\n',
        ],
    ),
];

/// Type `line` and Enter, after a pause.
macro_rules! typed {
    ($($b:expr),*) => { &[PAUSE, $($b),*, b'\n'] };
}

/// `login_arthur_permission` (L1.3a), boot 1: the account; `hi` to Home
/// launches Arthur; then Arthur's actions and the WM's prompt: Allow once,
/// Deny, the same read again (remembered), Always, a write (read-only), and
/// Super+L while a prompt is up.
const PERMISSION1: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
    step(b"LOGIN session started view=home", typed!(b'h', b'i')),
    step(b"DESKTOP arthur line=hi", READ_NOTES),
    step(b"PERMISSION prompt action=read path=/neofs/notes.txt", b"1"),
    step(b"PERMISSION done", READ_NOTES),
    step(b"PERMISSION prompt action=read path=/neofs/notes.txt", b"3"),
    step(b"PERMISSION answer=deny", READ_NOTES),
    // PM #253 follow-up: an Always whose log entry can't be appended is
    // refused, and its record entry undone: the same read prompts again.
    step(b"PERMISSION refused reason=denied-this-session", READ_NOLG),
    step(b"PERMISSION prompt action=read path=/neofs/nolg.txt", b"2"),
    step(b"PERMISSION record undone", READ_NOLG),
    step(b"PERMISSION prompt action=read path=/neofs/nolg.txt", b"1"),
    step(b"PERMISSION done", READ_TODO),
    step(b"PERMISSION prompt action=read path=/neofs/todo.txt", b"2"),
    step(b"PERMISSION done", b"\x80write /neofs/notes.txt hello\n"),
    step(b"PERMISSION refused reason=read-only", b"\x80escapeprobe\n"),
    // #248 bounce F2: the keyboard on the WM's own focus (Super+Escape) when
    // the prompt comes up; answered, it must stay there.
    step(b"DESKTOP arthur escape probe armed", &[HMP_SUPER_ESCAPE]),
    step(
        b"PERMISSION prompt action=read path=/neofs/escape.txt",
        b"1",
    ),
    step(b"PERMISSION done", &[PAUSE, b'\n']),
    // Enter on the WM's focus gives the keyboard back to Arthur.
    step(
        b"DESKTOP pointer focus client=arthur via=enter",
        b"\x80stall\n",
    ),
    // #248 bounce F1: a header without its path; Super+L must still lock,
    // and the lock screen still unlock, while the WM waits for it.
    step(
        b"DESKTOP arthur permission stall sent",
        &[PAUSE, HMP_SUPER_L],
    ),
    step(b"LOCK prompt=password", P1),
    // The WM keeps waiting for the path after the unlock: a Backspace every
    // 8 s (to Arthur's empty line, a no-op) keeps the idle lock away until
    // it refuses the request.
    step(b"LOCK unlocked", STALL_ACTIVITY),
    step(
        b"DESKTOP arthur permission stall status=33",
        b"\x80intentstall\n",
    ),
    // Dexter 213 item 1: the path sent, the intent withheld; the same lock
    // leg must pass while the WM waits for the intent.
    step(
        b"DESKTOP arthur permission intent stall sent",
        &[PAUSE, HMP_SUPER_L],
    ),
    step(b"LOCK prompt=password", P1),
    step(b"LOCK unlocked", STALL_ACTIVITY),
    step(
        b"DESKTOP arthur permission intent stall status=33",
        b"\x80escapewait\n",
    ),
    // Item 2: Super+Escape while the WM waits for the path; the prompt then
    // comes up with the keyboard on the WM's focus, and stays there.
    step(
        b"DESKTOP arthur escape wait armed",
        &[PAUSE, HMP_SUPER_ESCAPE],
    ),
    step(
        b"PERMISSION prompt action=read path=/neofs/escape.txt",
        b"1",
    ),
    step(b"PERMISSION done", &[PAUSE, b'\n']),
    // Item 4: a request and a done naming another surface.
    step(
        b"DESKTOP pointer focus client=arthur via=enter",
        b"\x80badidentity\n",
    ),
    // Item 3: Always with the record full (the seed fills it but for
    // todo.txt): answered once, and the next read prompts again.
    step(b"DESKTOP arthur identity probe bad_done", READ_FULL),
    step(b"PERMISSION prompt action=read path=/neofs/full.txt", b"2"),
    step(b"PERMISSION done", READ_FULL),
    step(b"PERMISSION prompt action=read path=/neofs/full.txt", b"1"),
    step(b"PERMISSION done", b"\x80list /neofs/docs/\n"),
    step(b"PERMISSION prompt action=list", &[PAUSE, HMP_SUPER_L]),
    step(b"LOCK prompt=password", P1),
];

/// Boot 2: sign in; Arthur reads the file Always allowed: no prompt. Then a
/// stalled request with no key typed after it: the idle lock (30 s in this
/// boot test) must still engage (#248 bounce F1); the same with the intent
/// withheld (Dexter 213 item 1); then Super+L.
const PERMISSION2: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=password", P1),
    step(b"LOGIN session started view=home", typed!(b'h', b'i')),
    step(b"DESKTOP arthur line=hi", READ_TODO),
    step(b"PERMISSION done", b"\x80stall\n"),
    step(b"LOCK locked reason=idle", &[PAUSE]),
    step(b"LOCK prompt=password", P1),
    step(
        b"DESKTOP arthur permission stall status=33",
        b"\x80intentstall\n",
    ),
    // Dexter 213 item 1, the idle lock's leg: the intent withheld.
    step(b"LOCK locked reason=idle", &[PAUSE]),
    step(b"LOCK prompt=password", P1),
    step(
        b"DESKTOP arthur permission intent stall status=33",
        &[PAUSE, HMP_SUPER_L],
    ),
    step(b"LOCK prompt=password", P1),
];

// `\x80` is `HMP_PAUSE_1S` (a byte string can't name the const).
const _: () = assert!(HMP_PAUSE_1S == 0x80);
/// Six Backspaces 8 s apart (`\x80` pauses, `\x08` Backspace).
const STALL_ACTIVITY: &[u8] = b"\x80\x80\x80\x80\x80\x80\x80\x80\x08\x80\x80\x80\x80\x80\x80\x80\x80\x08\x80\x80\x80\x80\x80\x80\x80\x80\x08\x80\x80\x80\x80\x80\x80\x80\x80\x08\x80\x80\x80\x80\x80\x80\x80\x80\x08\x80\x80\x80\x80\x80\x80\x80\x80\x08";
const READ_NOTES: &[u8] = b"\x80read /neofs/notes.txt\n";
const READ_TODO: &[u8] = b"\x80read /neofs/todo.txt\n";
const READ_FULL: &[u8] = b"\x80read /neofs/full.txt\n";
const READ_NOLG: &[u8] = b"\x80read /neofs/nolg.txt\n";

/// `login_lock_idle`: keys 3 s apart (each one restarts the 6 s idle time),
/// then none: the idle lock, then the password.
const LOCK_IDLE: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
    step(
        b"LOGIN session started view=home",
        &[
            PAUSE, b'a', PAUSE, PAUSE, PAUSE, b'b', PAUSE, PAUSE, PAUSE, b'c', PAUSE, PAUSE, PAUSE,
            b'd',
        ],
    ),
    step(b"LOCK prompt=password", P1),
];

/// `login_lock_stall` (AAG F3): Arthur, launched from Home, stalls 8 s
/// before its 'M'; Super+L during the stall locks at once.
const LOCK_STALL: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
    step(
        b"LOGIN session started view=home",
        &[PAUSE, b'h', b'i', b'\n'],
    ),
    step(b"DESKTOP arthur lock stall before=M", &[PAUSE, HMP_SUPER_L]),
    step(b"LOCK prompt=password", P1),
];

/// `login_lock_park` (L2b): Arthur, launched from Home, takes `p`, then
/// sends a commit whose reply it doesn't receive for 12 s, so the WM is
/// parked in that send. Super+L, then `x`, both during the park; the lock
/// screen comes after it.
const LOCK_PARK: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
    step(
        b"LOGIN session started view=home",
        &[PAUSE, b'h', b'i', b'\n'],
    ),
    step(b"DESKTOP arthur lock park ready", &[PAUSE, b'p']),
    step(
        b"DESKTOP arthur lock park begin",
        &[PAUSE, HMP_SUPER_L, PAUSE, b'x'],
    ),
    step(b"LOCK prompt=password", P1),
];

/// `login_terminal_lock` (L1a.3): the account; `open terminal` from Home
/// (Arthur opens it); `ab` into the terminal's cooked line, Super+L, then `x`
/// and Enter into the lock's password field (wrong), the password; after the
/// unlock gives the terminal its turn back, `c` and Enter. Then a second lock
/// and unlock end the boot (`UNLOCKS` = 2).
const TERMINAL_LOCK: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
    step(b"LOGIN session started view=home", b"open terminal\n"),
    step(
        b"TERM row=0 |child ready|",
        &[b'a', b'b', PAUSE, HMP_SUPER_L],
    ),
    step(b"LOCK prompt=password", b"x\n"),
    step(b"LOCK prompt=password", P1),
    step(b"DESKTOP apps client resumed client=3", b"c\n"),
    step(b"TERM row=2 |got=6162630a|", &[PAUSE, HMP_SUPER_L]),
    step(b"LOCK prompt=password", P1),
];

/// `login_switch_lock` (A2, Muse's #98 observation): the account; `open
/// terminal` from Home; while the WM holds the SAK latch (2 s from the
/// terminal's launch), Super+Tab and Super+L, so both come out of one take and
/// the switch is due in the turn the lock engages. It is dropped, not run while
/// locked. The password; the unlock gives the terminal its turn back; Super+Tab
/// then switches to Arthur as usual; a second lock and unlock end the boot.
const SWITCH_LOCK: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
    step(b"LOGIN session started view=home", b"open terminal\n"),
    step(b"DESKTOP apps sak hold ms=", &[HMP_SUPER_TAB, HMP_SUPER_L]),
    step(b"LOCK prompt=password", P1),
    step(b"DESKTOP apps client resumed client=3", &[PAUSE, HMP_SUPER_TAB]),
    step(
        b"DESKTOP apps front client=0 via=sak-tab",
        &[PAUSE, HMP_SUPER_L],
    ),
    step(b"LOCK prompt=password", P1),
];

/// `login_settings_store` (OT-88 S1), boot 1: the account, then Arthur launched
/// from Home sets its settings by itself (the boot test's stand-in for the
/// settings app); the idle lock (5, set by the WM: 20 s in the boot test)
/// fires with no key typed, and the password unlocks. `login_settings_refused`
/// types the same.
const SETTINGS1: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
    step(
        b"LOGIN session started view=home",
        &[PAUSE, b'h', b'i', b'\n'],
    ),
    step(b"LOCK prompt=password", P1),
];

/// Boot 2: sign in; Super+L, then the password.
const SETTINGS2: &[SerialScriptStep] = &[
    step(b"LOGIN prompt=password", P1),
    step(b"LOGIN session started view=home", &[PAUSE, HMP_SUPER_L]),
    step(b"LOCK prompt=password", P1),
];

/// `login_setup_store` (F11 S2), boot 1: the setup menu walked with both
/// Comfort options on (S1's walk), then the account as `login_record_guard`
/// makes it.
const SETUP_STORE1: &[SerialScriptStep] = &[
    step(b"DESKTOP setup page=welcome rows=5", b"\n"),
    step(b"DESKTOP setup page=timezone rows=8", &[HMP_UP]),
    step(b"DESKTOP setup redraw page=timezone focus=4", b"\n"),
    step(b"DESKTOP setup page=comfort rows=6", b" "),
    step(b"DESKTOP setup redraw page=comfort focus=0", &[HMP_DOWN]),
    step(b"DESKTOP setup redraw page=comfort focus=1", b" "),
    step(b"DESKTOP setup redraw page=comfort focus=1", &[HMP_DOWN]),
    step(b"DESKTOP setup redraw page=comfort focus=2", b"\n"),
    step(b"DESKTOP setup page=try rows=5", b"\n"),
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
];

/// Boot 2: no menu (the record exists); sign in.
const SETUP_STORE2: &[SerialScriptStep] = &[step(b"LOGIN prompt=password", P1)];

/// `login_setup_tz` (F11 S3), boot 1: Begin; on the Time zone page type
/// `new y` (Space types there), choose America/New_York, Up to Continue and
/// leave (the one confirm); Comfort's Continue; Try; then the account. Keys
/// arrive in order, so the Enter after the typing chooses from its matches.
const SETUP_TZ1: &[SerialScriptStep] = &[
    step(b"DESKTOP setup page=welcome rows=5", b"\n"),
    step(b"DESKTOP setup page=timezone rows=8", b"new y"),
    step(
        b"DESKTOP setup redraw page=timezone focus=0",
        &[PAUSE, b'\n', PAUSE, HMP_UP, PAUSE, b'\n'],
    ),
    // Comfort has one more row (the notice) after a failed confirm; the
    // checks pin which.
    step(b"DESKTOP setup page=comfort rows=", &[HMP_UP, PAUSE, b'\n']),
    step(b"DESKTOP setup page=try rows=5", b"\n"),
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
];

/// `login_setup_tz_skip`, the decline: Begin, type `new y` and choose New
/// York, then Esc back to Welcome without leaving the page forward (no
/// confirm), Esc again and skip; then the account.
const SETUP_TZ_SKIP: &[SerialScriptStep] = &[
    step(b"DESKTOP setup page=welcome rows=5", b"\n"),
    step(b"DESKTOP setup page=timezone rows=8", b"new y"),
    step(
        b"DESKTOP setup redraw page=timezone focus=0",
        &[PAUSE, b'\n', PAUSE, 0x1b],
    ),
    step(b"DESKTOP setup page=welcome rows=5", b"\x1b"),
    step(b"DESKTOP setup page=skip rows=5", b"\n"),
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
];

/// `login_setup_tz_reread_fail` (Chad, relay 379), boot 1: the walk to the
/// confirm; it fails and the store can't be re-read, so the page stays with
/// the notice (one more row: 6); then Esc back to Welcome, Esc and skip.
const SETUP_TZ_REREAD: &[SerialScriptStep] = &[
    step(b"DESKTOP setup page=welcome rows=5", b"\n"),
    step(b"DESKTOP setup page=timezone rows=8", b"new y"),
    step(
        b"DESKTOP setup redraw page=timezone focus=0",
        &[PAUSE, b'\n', PAUSE, HMP_UP, PAUSE, b'\n'],
    ),
    step(b"DESKTOP setup page=timezone rows=6", &[PAUSE, 0x1b]),
    step(b"DESKTOP setup page=welcome rows=5", b"\x1b"),
    step(b"DESKTOP setup page=skip rows=5", b"\n"),
    step(b"LOGIN prompt=name", b"ada\n"),
    step(b"LOGIN prompt=choose", P1),
    step(b"LOGIN prompt=again", P1),
    step(b"LOGIN prompt=code", b"\n"),
];

/// One boot of the VM layout with `steps` typed (none: no keyboard).
#[allow(clippy::too_many_arguments)]
fn boot(
    scenario: TestScenario,
    attempt: usize,
    label: &str,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
    steps: Option<&'static [SerialScriptStep]>,
) -> Result<ObservedTest, String> {
    let log = scenario_serial_log(&format!(
        "{}{}.attempt{}.{}.serial.log",
        scenario.name,
        neo_test_run_id_suffix(),
        attempt,
        label
    ));
    remove_serial_log(&log)?;
    vm_vars_copy(ovmf, vars)?;
    let args = vm_image::vm_disk_args(&ovmf.code, vars, boot_disk, data);
    *SCRIPT_OVERRIDE.lock().expect("login script override") = steps;
    let result = run_qemu_test_invocation(
        scenario,
        ScenarioBundleMode::Vmware,
        log.clone(),
        &[],
        Some(&args),
    );
    *SCRIPT_OVERRIDE.lock().expect("login script override") = None;
    let canonical = scenario_serial_log(&format!("{}.{}.serial.log", scenario.name, label));
    let _ = fs::copy(&log, &canonical);
    let observed = result?;
    require_observed_pass(scenario, &observed)?;
    require_serial_contains(
        scenario,
        &observed,
        "mounted NeoFS readwrite at /neofs device_index=1 ",
        "the kernel must mount the data disk (device 1), not the boot disk",
    )?;
    Ok(observed)
}

fn absent(
    scenario: TestScenario,
    observed: &ObservedTest,
    needle: &str,
    why: &str,
) -> Result<(), String> {
    let serial = read_serial(observed)?;
    if serial.contains(needle) {
        return Err(format!(
            "`{}` serial contains `{needle}` ({why}); serial={}",
            scenario.name,
            observed.serial_log.display()
        ));
    }
    Ok(())
}

pub fn run_login_vm_scenario(scenario: TestScenario, attempt: usize) -> Result<(), String> {
    build_test_artifacts_for_scenario(scenario, ScenarioBundleMode::Vmware)?;
    let ovmf = resolve_ovmf(true)?;
    let dir = workspace_root().join(TEST_OUTPUT_REL).join(scenario.name);
    let (boot_disk, data) = write_vm_disks(&dir, true)?;
    let boot_digest = blake3_file_hex(&boot_disk)?;
    let vars = dir.join("vars.fd");
    match scenario.name {
        LOGIN_TWO_BOOT_SCENARIO => two_boot(scenario, attempt, &ovmf, &vars, &boot_disk, &data)?,
        LOGIN_BACKOFF_REBOOT_SCENARIO => {
            boot(
                scenario,
                attempt,
                "boot1",
                &ovmf,
                &vars,
                &boot_disk,
                &data,
                Some(RECORD_GUARD),
            )?;
            let second = boot(
                scenario,
                attempt,
                "boot2",
                &ovmf,
                &vars,
                &boot_disk,
                &data,
                Some(BACKOFF_BOOT2),
            )?;
            require_serial_sequence(
                scenario,
                &second,
                &[
                    "LOGIN screen shown failures=0",
                    "LOGIN password refused failures=3 next_delay_s=1",
                    "LOGIN password refused failures=4 next_delay_s=2",
                    "LOGIN backoff test stop",
                ],
                "four wrong passwords count to four",
            )?;
            let third = boot(
                scenario,
                attempt,
                "boot3",
                &ovmf,
                &vars,
                &boot_disk,
                &data,
                Some(BACKOFF_BOOT3),
            )?;
            require_serial_sequence(
                scenario,
                &third,
                &[
                    "LOGIN screen shown failures=4",
                    "LOGIN backoff wait_s=2",
                    "LOGIN signed in failures=4",
                    "LOGIN session started view=home",
                ],
                "a reboot must not reset the wrong-password count: the next attempt still waits out the backoff the four failures earned, and only the successful sign-in resets it",
            )?;
        }
        LOGIN_RECORD_GUARD_SCENARIO => {
            let observed = boot(
                scenario,
                attempt,
                "boot1",
                &ovmf,
                &vars,
                &boot_disk,
                &data,
                Some(RECORD_GUARD),
            )?;
            require_serial_sequence(
                scenario,
                &observed,
                &[
                    "LOGIN account created generation=1",
                    "LOGIN record guard wm_open=ok",
                    "RECORDGUARD read=-1 write=-1 dotdot=-1 dir=-1 create=-1 slashes=-1 dot=-1 relative=-22 root=-1 control=ok model=-1",
                    "LOGIN record guard probe exit=0",
                    "LOGIN record guard unchanged=1",
                    "LOGIN session started view=home",
                ],
                "the sign-in record must open only for the WM: every other process gets EPERM for every `.system/` open (read, write, a `..` path, the directory, a new file, `//` and `./` spellings, the root mount's own `/.system`, `SYS_MODEL_LOAD`), a relative path is EINVAL, it can still open `/neofs`, and both copies of the record are byte for byte as before",
            )?;
        }
        LOGIN_BELOW_FLOOR_SCENARIO => {
            let observed = boot(
                scenario, attempt, "boot1", &ovmf, &vars, &boot_disk, &data, None,
            )?;
            require_serial_sequence(
                scenario,
                &observed,
                &[
                    "LOGIN test seeded below-floor m=32768 files=a,b",
                    "LOGIN record refused file=a reason=below-floor",
                    "LOGIN record refused file=b reason=below-floor",
                    "LOGIN first-boot shown",
                ],
                "a record under the KDF floor must be refused, never honoured: the first-boot screen shows instead of the login screen",
            )?;
            absent(
                scenario,
                &observed,
                "LOGIN screen shown",
                "a below-floor record must never reach the login screen",
            )?;
            // A writable volume whose files hold no valid record is first
            // boot; skipping the gate there would be a sign-in bypass.
            absent(
                scenario,
                &observed,
                "LOGIN skipped",
                "an invalid record on a writable volume must never skip sign-in",
            )?;
        }
        LOGIN_LOCK_SESSION_SCENARIO
        | LOGIN_LOCK_IDLE_SCENARIO
        | LOGIN_LOCK_STALL_SCENARIO
        | LOGIN_LOCK_PARK_SCENARIO
        | LOGIN_TERMINAL_LOCK_SCENARIO
        | LOGIN_SWITCH_LOCK_SCENARIO => {
            lock(scenario, attempt, &dir, &ovmf, &vars, &boot_disk, &data)?
        }
        SETTINGS_STORE_SCENARIO => {
            settings_store(scenario, attempt, &ovmf, &vars, &boot_disk, &data)?
        }
        SETTINGS_CUT_SCENARIO => settings_cut(scenario, attempt, &ovmf, &vars, &boot_disk, &data)?,
        SETTINGS_REFUSED_SCENARIO => {
            settings_refused(scenario, attempt, &ovmf, &vars, &boot_disk, &data)?
        }
        ARTHUR_PERMISSION_SCENARIO => {
            arthur_permission(scenario, attempt, &ovmf, &vars, &boot_disk, &data)?
        }
        LOGIN_SETUP_STORE_SCENARIO => {
            setup_store(scenario, attempt, &ovmf, &vars, &boot_disk, &data)?
        }
        LOGIN_SETUP_TZ_SCENARIO
        | LOGIN_SETUP_TZ_SKIP_SCENARIO
        | LOGIN_SETUP_TZ_REFUSED_SCENARIO
        | LOGIN_SETUP_TZ_WRITE_FAIL_SCENARIO
        | LOGIN_SETUP_TZ_FSYNC_FAIL_SCENARIO
        | LOGIN_SETUP_TZ_ROLLBACK_FAIL_SCENARIO
        | LOGIN_SETUP_TZ_REREAD_FAIL_SCENARIO => {
            setup_tz(scenario, attempt, &ovmf, &vars, &boot_disk, &data)?
        }
        _ => cut_steps(scenario, attempt, &ovmf, &vars, &boot_disk, &data)?,
    }
    let after = blake3_file_hex(&boot_disk)?;
    if after != boot_digest {
        return Err(format!(
            "`{}`: the boot disk changed across the boots (blake3 {boot_digest} -> {after}); nothing may write it",
            scenario.name
        ));
    }
    eprintln!(
        "[xtask] neo-test: `{}` PASS (boot disk unchanged)",
        scenario.name
    );
    Ok(())
}

fn two_boot(
    scenario: TestScenario,
    attempt: usize,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
) -> Result<(), String> {
    let first = boot(
        scenario,
        attempt,
        "boot1",
        ovmf,
        vars,
        boot_disk,
        data,
        Some(BOOT1),
    )?;
    require_serial_sequence(
        scenario,
        &first,
        &[
            "LOGIN first-boot shown",
            "LOGIN password mismatch",
            "LOGIN recovery code shown",
            "LOGIN kdf params=m:65536,t:3,p:1 elapsed_ms=",
            "LOGIN account created generation=1 files=a,b,meta",
            "LOGIN session started view=home",
        ],
        "boot 1 (an empty .system/) must show the first-boot screen, catch the mismatched password, and create the account before the session starts",
    )?;
    absent(
        scenario,
        &first,
        "LOGIN screen shown",
        "boot 1 has no account to sign in to",
    )?;
    absent(
        scenario,
        &first,
        "LOGIN skipped",
        "the VM image's .system/ is writable: sign-in is never skipped",
    )?;

    let second = boot(
        scenario,
        attempt,
        "boot2",
        ovmf,
        vars,
        boot_disk,
        data,
        Some(BOOT2),
    )?;
    require_serial_sequence(
        scenario,
        &second,
        &[
            "LOGIN screen shown",
            "LOGIN password refused failures=1 next_delay_s=0",
            // F9 (J.c.): the wrong try is said, on the row nearest the field.
            "LOGIN note=\"Incorrect password. Try again.\" row=last",
            "LOGIN password refused failures=2 next_delay_s=0",
            "LOGIN note=\"Incorrect password. Try again.\" row=last",
            "LOGIN password refused failures=3 next_delay_s=1",
            "LOGIN backoff wait_s=1",
            "LOGIN signed in failures=3",
            "LOGIN session started view=home",
        ],
        "boot 2 must show the login screen, refuse three wrong passwords, wait out the 1 s backoff before checking the fourth try, and start the session on the right one",
    )?;
    absent(
        scenario,
        &second,
        "LOGIN first-boot shown",
        "boot 2 has an account",
    )?;

    let third = boot(
        scenario,
        attempt,
        "boot3",
        ovmf,
        vars,
        boot_disk,
        data,
        Some(BOOT3),
    )?;
    require_serial_sequence(
        scenario,
        &third,
        &[
            "LOGIN screen shown",
            "LOGIN recovery refused failures=1",
            "LOGIN recovery accepted",
            "LOGIN recovery code shown",
            "LOGIN password reset generation=2 file=b",
            "LOGIN session started view=home",
        ],
        "boot 3 must refuse a wrong recovery code, accept the right one, take a new password, issue a new code and write the new record to the file not holding the old one",
    )?;

    let fourth = boot(
        scenario,
        attempt,
        "boot4",
        ovmf,
        vars,
        boot_disk,
        data,
        Some(BOOT4),
    )?;
    require_serial_sequence(
        scenario,
        &fourth,
        &[
            "LOGIN screen shown",
            "LOGIN recovery refused failures=1",
            "LOGIN password refused failures=2",
            "LOGIN signed in failures=2",
            "LOGIN session started view=home",
        ],
        "boot 4 must refuse the used recovery code (a new one was issued) and the old password, and sign in with the new one",
    )?;
    Ok(())
}

/// The cut step a boot runs at: the four digits of `LOGINCUT step=NNNN` in
/// the boot disk (the ISO's FAT and ISO 9660 copies of `/modules/login-cut`
/// both), rewritten in place. Step 0 is a verify boot.
fn set_cut_step(boot_disk: &Path, step: u32) -> Result<(), String> {
    let mut bytes =
        fs::read(boot_disk).map_err(|e| format!("read {}: {e}", boot_disk.display()))?;
    let marker = b"LOGINCUT step=";
    let mut hits = 0;
    let mut at = 0;
    while let Some(pos) = bytes[at..].windows(marker.len()).position(|w| w == marker) {
        let digits = at + pos + marker.len();
        let new = format!("{step:04}");
        bytes[digits..digits + 4].copy_from_slice(new.as_bytes());
        hits += 1;
        at = digits + 4;
    }
    if hits == 0 {
        return Err(format!(
            "{}: no `LOGINCUT step=` module bytes to rewrite",
            boot_disk.display()
        ));
    }
    fs::write(boot_disk, &bytes).map_err(|e| format!("write {}: {e}", boot_disk.display()))
}

/// What a verify boot reported.
#[derive(Debug, PartialEq, Eq)]
struct Verify {
    record: String,
    generation: u64,
    old: bool,
    new: bool,
    a: String,
    b: String,
}

fn verify_line(scenario: TestScenario, observed: &ObservedTest) -> Result<Verify, String> {
    let serial = read_serial(observed)?;
    let line = serial
        .lines()
        .find_map(|l| l.split("LOGIN cut verify ").nth(1))
        .ok_or_else(|| {
            format!(
                "`{}` verify boot logged no `LOGIN cut verify`; serial={}",
                scenario.name,
                observed.serial_log.display()
            )
        })?;
    let field = |key: &str| -> Result<String, String> {
        line.split_whitespace()
            .find_map(|kv| kv.strip_prefix(key))
            .map(str::to_string)
            .ok_or_else(|| format!("`{}` verify line lacks `{key}`: {line}", scenario.name))
    };
    Ok(Verify {
        record: field("record=")?,
        generation: field("generation=")?
            .parse()
            .map_err(|e| format!("generation: {e}"))?,
        old: field("old=")? == "1",
        new: field("new=")? == "1",
        a: field("a_blake3=")?,
        b: field("b_blake3=")?,
    })
}

fn copy_disk(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))
}

fn cut_steps(
    scenario: TestScenario,
    attempt: usize,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
) -> Result<(), String> {
    let dir = data.parent().expect("data disk has a directory");
    // The ISO's own bytes stay untouched; the cut boots run from a copy whose
    // module digits are rewritten per boot.
    let cut_boot = dir.join("boot-cut.raw");
    copy_disk(boot_disk, &cut_boot)?;

    set_cut_step(&cut_boot, 0)?;
    let setup = boot(
        scenario, attempt, "setup", ovmf, vars, &cut_boot, data, None,
    )?;
    require_serial_contains(
        scenario,
        &setup,
        "LOGIN cut setup created generation=1",
        "the setup boot must create the account (both files, generation 1)",
    )?;
    let verify = |label: &str, disk: &Path| -> Result<Verify, String> {
        set_cut_step(&cut_boot, 0)?;
        let observed = boot(scenario, attempt, label, ovmf, vars, &cut_boot, disk, None)?;
        verify_line(scenario, &observed)
    };
    let case = dir.join("data-case.raw");
    copy_disk(data, &case)?;
    let before = verify("before", &case)?;
    if before
        != (Verify {
            record: "a".into(),
            generation: 1,
            old: true,
            new: false,
            a: before.a.clone(),
            b: before.b.clone(),
        })
    {
        return Err(format!("`{}` before the change: {before:?}", scenario.name));
    }

    // Calibration: the whole change, uncut, and how many ops it takes.
    copy_disk(data, &case)?;
    set_cut_step(&cut_boot, 9999)?;
    let full = boot(
        scenario, attempt, "full", ovmf, vars, &cut_boot, &case, None,
    )?;
    let serial = read_serial(&full)?;
    let ops: u32 = serial
        .lines()
        .find_map(|l| l.split("LOGIN cut not-reached step=9999 ops=").nth(1))
        .and_then(|n| n.trim().parse().ok())
        .ok_or_else(|| {
            format!(
                "`{}` the uncut change logged no op count; serial={}",
                scenario.name,
                full.serial_log.display()
            )
        })?;
    if ops == 0 || ops > 200 {
        return Err(format!("`{}` the change took {ops} ops", scenario.name));
    }
    let after = verify("after", &case)?;
    if !(after.record == "b"
        && after.generation == 2
        && !after.old
        && after.new
        && after.a == before.a)
    {
        return Err(format!(
            "`{}` the uncut change must leave the new password in b, generation 2, with a's bytes unchanged: {after:?} (a before {})",
            scenario.name, before.a
        ));
    }
    eprintln!(
        "[xtask] neo-test: `{}` the change takes {ops} device writes and flushes",
        scenario.name
    );

    // The steps to cut: every one with NEO_LOGIN_CUT_ALL=1 (the full sweep);
    // else, from the calibration's op list, each flush and, for each run of
    // consecutive sectors (one 4 KiB block or more), the cut after its first
    // sector (a torn block) and after its last (the whole block).
    let listed = cut_ops(&serial);
    if listed.len() != ops as usize {
        return Err(format!(
            "`{}` the calibration listed {} ops but counted {ops}",
            scenario.name,
            listed.len()
        ));
    }
    let steps: Vec<u32> = if env::var("NEO_LOGIN_CUT_ALL").is_ok_and(|v| v == "1") {
        (1..=ops).collect()
    } else {
        sampled_steps(&listed)
    };
    eprintln!(
        "[xtask] neo-test: `{}` cutting at {} of the {ops} steps: {steps:?}",
        scenario.name,
        steps.len()
    );

    // Every cut: exactly one password, and a (the record) byte-identical.
    let mut table = Vec::new();
    let mut seen_new = false;
    for step in steps {
        copy_disk(data, &case)?;
        set_cut_step(&cut_boot, step)?;
        let cut = boot(
            scenario,
            attempt,
            &format!("cut{step:02}"),
            ovmf,
            vars,
            &cut_boot,
            &case,
            None,
        )?;
        let serial = read_serial(&cut)?;
        let fired = serial
            .lines()
            .find(|l| l.contains(&format!("stage=cut step={step} ")))
            .ok_or_else(|| {
                format!(
                    "`{}` step {step}: the cut did not fire; serial={}",
                    scenario.name,
                    cut.serial_log.display()
                )
            })?;
        let op = fired
            .split_whitespace()
            .filter(|kv| kv.starts_with("op=") || kv.starts_with("lba="))
            .collect::<Vec<_>>()
            .join(" ");
        let v = verify(&format!("cut{step:02}-verify"), &case)?;
        let outcome = match (v.old, v.new) {
            (true, false) => "old",
            (false, true) => "new",
            _ => {
                return Err(format!(
                    "`{}` step {step} ({op}): exactly one of the old and new passwords must be accepted, got {v:?}",
                    scenario.name
                ));
            }
        };
        if v.a != before.a {
            return Err(format!(
                "`{}` step {step} ({op}): the file the change doesn't write (a) changed: blake3 {} -> {}",
                scenario.name, before.a, v.a
            ));
        }
        if seen_new && outcome == "old" {
            return Err(format!(
                "`{}` step {step} ({op}): the old password came back after an earlier cut already kept the new one",
                scenario.name
            ));
        }
        seen_new |= outcome == "new";
        eprintln!(
            "[xtask] neo-test: `{}` cut after step {step}/{ops} ({op}): {outcome} password, record {} generation {}, a unchanged",
            scenario.name, v.record, v.generation
        );
        table.push(format!(
            "{step} {op} {outcome} record={} gen={}",
            v.record, v.generation
        ));
    }
    if !seen_new {
        return Err(format!(
            "`{}` no cut kept the new password, though the full change did",
            scenario.name
        ));
    }
    println!("LOGIN_CUT/1 ops={ops} a_blake3={}", before.a);
    for row in table {
        println!("LOGIN_CUT/1 step {row}");
    }
    Ok(())
}

/// The calibration boot's ops, in order: `(step, is_flush, lba)`.
fn cut_ops(serial: &str) -> Vec<(u32, bool, u64)> {
    serial
        .lines()
        .filter_map(|l| l.split("LOGIN-CUT op ").nth(1))
        .filter_map(|rest| {
            let field = |key: &str| {
                rest.split_whitespace()
                    .find_map(|kv| kv.strip_prefix(key))
                    .map(str::to_string)
            };
            Some((
                field("step=")?.parse().ok()?,
                field("op=")? == "flush",
                field("lba=")?.parse().ok()?,
            ))
        })
        .collect()
}

/// The data disk's 512-byte sectors per 4 KiB NeoFS block.
const SECTORS_PER_BLOCK: u64 = 8;

/// Each flush, and the first and last write of each run of consecutive LBAs
/// within one 4 KiB block (a run that crosses a block boundary is split).
fn sampled_steps(ops: &[(u32, bool, u64)]) -> Vec<u32> {
    let mut steps = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        let (step, flush, lba) = ops[i];
        if flush {
            steps.push(step);
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < ops.len()
            && !ops[j + 1].1
            && ops[j + 1].2 == ops[j].2 + 1
            && ops[j + 1].2 / SECTORS_PER_BLOCK == lba / SECTORS_PER_BLOCK
        {
            j += 1;
        }
        steps.push(step);
        if j > i {
            steps.push(ops[j].0);
        }
        i = j + 1;
    }
    steps
}

/// OT-66 L2: one boot, its captures in `dir`.
fn lock(
    scenario: TestScenario,
    attempt: usize,
    dir: &Path,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
) -> Result<(), String> {
    let session = scenario.name == LOGIN_LOCK_SESSION_SCENARIO;
    let stall = scenario.name == LOGIN_LOCK_STALL_SCENARIO;
    let park = scenario.name == LOGIN_LOCK_PARK_SCENARIO;
    let terminal = scenario.name == LOGIN_TERMINAL_LOCK_SCENARIO;
    let switch = scenario.name == LOGIN_SWITCH_LOCK_SCENARIO;
    *HMP_CAPTURE_DIR.lock().expect("capture dir") = Some(dir.to_path_buf());
    let observed = boot(
        scenario,
        attempt,
        "boot1",
        ovmf,
        vars,
        boot_disk,
        data,
        Some(if session {
            LOCK_SESSION
        } else if stall {
            LOCK_STALL
        } else if park {
            LOCK_PARK
        } else if terminal {
            TERMINAL_LOCK
        } else if switch {
            SWITCH_LOCK
        } else {
            LOCK_IDLE
        }),
    );
    *HMP_CAPTURE_DIR.lock().expect("capture dir") = None;
    let observed = observed?;
    if session {
        lock_session_checks(scenario, &observed, dir)
    } else if stall {
        lock_stall_checks(scenario, &observed)
    } else if park {
        lock_park_checks(scenario, &observed)
    } else if terminal {
        terminal_lock_checks(scenario, &observed)
    } else if switch {
        switch_lock_checks(scenario, &observed)
    } else {
        lock_idle_checks(scenario, &observed)
    }
}

/// L2.1: the action log a scenario expects, built with action-log-core.
#[derive(Default)]
struct ActionLog {
    records: u64,
    link: [u8; action_log_core::PREV_LEN],
}

impl ActionLog {
    fn push(
        &mut self,
        kind: action_log_core::Kind,
        action: u8,
        detail: u32,
        path: &str,
        intent: &str,
    ) {
        let bytes = action_log_core::encode(&action_log_core::Record {
            kind,
            action,
            seq: self.records,
            time_s: 0,
            detail,
            path,
            intent,
            prev: self.link,
            flags: 0,
        })
        .expect("a scenario record fits");
        self.link = action_log_core::link(&bytes);
        self.records += 1;
    }

    /// The WM's `ACTIONLOG verify` line for this log (`records` is the next
    /// sequence number: a rotation drops records but not the count).
    fn verify_line(&self) -> String {
        let head: String = self.link.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "ACTIONLOG verify records={} broken=none head={head}",
            self.records
        )
    }
}

/// L1.3a: the two boots of `login_arthur_permission`.
fn arthur_permission(
    scenario: TestScenario,
    attempt: usize,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
) -> Result<(), String> {
    let first = boot(
        scenario,
        attempt,
        "boot1",
        ovmf,
        vars,
        boot_disk,
        data,
        Some(PERMISSION1),
    )?;
    const NOTES: &str = "action=read path=/neofs/notes.txt";
    const TODO: &str = "action=read path=/neofs/todo.txt";
    let notes = |what: &str| format!("{what} {NOTES}");
    let todo = |what: &str| format!("{what} {TODO}");
    require_serial_sequence(
        scenario,
        &first,
        &[
            "LOGIN account created generation=1",
            "LOGIN session started view=home",
            "DESKTOP arthur line=hi",
            // Allow once: the cap is lent, used, and revoked after.
            &notes("PERMISSION prompt"),
            "PERMISSION route restored to=client",
            &notes("PERMISSION answer=once"),
            &notes("DESKTOP arthur permission"),
            "DESKTOP arthur action outcome=8",
            "PERMISSION done outcome=0 revoked=1",
            "DESKTOP arthur lent cap after done read=-",
            // Asked again: a prompt again (once is once), denied.
            &notes("PERMISSION prompt"),
            &notes("PERMISSION answer=deny"),
            "DESKTOP arthur permission action=read path=/neofs/notes.txt status=31",
            // The denial is remembered: no prompt.
            &notes("PERMISSION refused reason=denied-this-session"),
            "DESKTOP arthur permission action=read path=/neofs/notes.txt status=31",
            // An Always the log can't hold: refused, and the record put back
            // (so todo.txt's Always below still finds the record's free slot).
            "PERMISSION prompt action=read path=/neofs/nolg.txt",
            "PERMISSION answer=always action=read path=/neofs/nolg.txt",
            "ACTIONLOG append=failed",
            "PERMISSION refused reason=log action=read path=/neofs/nolg.txt",
            "PERMISSION record undone action=read path=/neofs/nolg.txt",
            "DESKTOP arthur permission action=read path=/neofs/nolg.txt status=31",
            "PERMISSION prompt action=read path=/neofs/nolg.txt",
            "PERMISSION answer=once action=read path=/neofs/nolg.txt",
            "PERMISSION done outcome=0 revoked=1",
            // Always: recorded, lent, revoked after the action as well.
            &todo("PERMISSION prompt"),
            &todo("PERMISSION answer=always"),
            "DESKTOP arthur permission action=read path=/neofs/todo.txt status=30 always",
            "DESKTOP arthur action outcome=8",
            "PERMISSION done outcome=0 revoked=1",
            // Read-only: a write is refused before any prompt.
            "PERMISSION refused reason=read-only action=change path=/neofs/notes.txt",
            "DESKTOP arthur permission action=change path=/neofs/notes.txt status=32",
            // #248 bounce F2: escaped (Super+Escape), an answered prompt
            // leaves the keyboard on the WM's own focus; Enter gives it back.
            "DESKTOP arthur escape probe armed",
            "DESKTOP pointer escape focus=spine via=sak",
            "PERMISSION prompt action=read path=/neofs/escape.txt",
            "PERMISSION route restored to=wm",
            "PERMISSION answer=once action=read path=/neofs/escape.txt",
            "PERMISSION done outcome=0 revoked=1",
            "DESKTOP pointer focus client=arthur via=enter",
            // #248 bounce F1: a header without its path; the lock engages
            // and unlocks while the WM waits, then the request is refused.
            "DESKTOP arthur permission stall sent",
            "LOCK locked reason=sak",
            "LOCK unlocked failures=0",
            "PERMISSION refused reason=incomplete",
            "DESKTOP arthur permission stall status=33",
            // Dexter 213 item 1: the same, waiting for the intent.
            "DESKTOP arthur permission intent stall sent",
            "LOCK locked reason=sak",
            "LOCK unlocked failures=0",
            "PERMISSION refused reason=incomplete",
            "DESKTOP arthur permission intent stall status=33",
            // Item 2: Super+Escape during the path wait takes the keyboard,
            // and the answered prompt leaves it on the WM.
            "DESKTOP arthur escape wait armed",
            "DESKTOP pointer escape focus=spine via=sak",
            "PERMISSION prompt action=read path=/neofs/escape.txt",
            "PERMISSION route restored to=wm",
            "PERMISSION answer=once action=read path=/neofs/escape.txt",
            "PERMISSION done outcome=0 revoked=1",
            "DESKTOP pointer focus client=arthur via=enter",
            // Item 4: another surface's request is refused (CapRequired),
            // nothing lent; its done is refused and leaves the cap lent;
            // the right done revokes it.
            "PERMISSION refused reason=identity",
            "DESKTOP arthur identity probe request=7",
            "PERMISSION lent from record action=read path=/neofs/todo.txt",
            "PERMISSION done refused reason=identity",
            "PERMISSION done outcome=0 revoked=1",
            "DESKTOP arthur identity probe bad_done=7 read=1 good_done=0 read=-",
            // Item 3: an Always the full record can't hold is a once.
            "PERMISSION prompt action=read path=/neofs/full.txt",
            "PERMISSION answer=always action=read path=/neofs/full.txt",
            "PERMISSION record=failed action=read path=/neofs/full.txt",
            "DESKTOP arthur permission action=read path=/neofs/full.txt status=0",
            "PERMISSION done outcome=0 revoked=1",
            "PERMISSION prompt action=read path=/neofs/full.txt",
            "PERMISSION answer=once action=read path=/neofs/full.txt",
            "PERMISSION done outcome=0 revoked=1",
            // The lock outranks a prompt: it is denied.
            "PERMISSION prompt action=list path=/neofs/docs/",
            "LOCK locked reason=sak",
            "PERMISSION answer=deny reason=locked action=list path=/neofs/docs/",
            "DESKTOP arthur permission action=list path=/neofs/docs/ status=31",
            "LOCK unlocked failures=0",
            "LOCK boot test done",
        ],
        "boot 1: Allow once lends a cap the WM revokes after the action, Deny is remembered, Always is recorded, a write is refused in read-only mode, an escaped prompt keeps the keyboard on the WM (escaped before the request or while the WM waits for its path), a request stalled on its path or its intent can't keep the lock out, another surface's request and done are refused, an Always the full record can't hold is a once, and the lock denies a prompt",
    )?;
    // L2.1: the WM's action log, recomputed record by record (the boot test
    // pins the clock to 0, so the bytes are fixed).
    let mut log = ActionLog::default();
    // The boot test's seed (#253 bounce): 190 good records over 4 KiB and a
    // torn 191st. The first append cuts the torn tail and marks the break.
    for _ in 0..190 {
        log.push(
            action_log_core::Kind::Asked,
            1,
            0,
            "/neofs/seed.txt",
            "seed",
        );
    }
    log.push(
        action_log_core::Kind::Refused,
        0,
        action_log_core::REFUSED_CHAIN_BROKEN,
        "",
        "",
    );
    let (n, t, l, w) = (
        "/neofs/notes.txt",
        "/neofs/todo.txt",
        "/neofs/docs/",
        "/neofs/notes.txt",
    );
    let (rn, rt) = ("read /neofs/notes.txt", "read /neofs/todo.txt");
    use action_log_core::Kind::*;
    log.push(Asked, 1, 0, n, rn);
    log.push(AllowedOnce, 1, 0, n, rn);
    log.push(Done, 1, 0, n, rn);
    log.push(Asked, 1, 0, n, rn);
    log.push(Denied, 1, 0, n, rn);
    log.push(
        Refused,
        1,
        action_log_core::REFUSED_DENIED_THIS_SESSION,
        n,
        rn,
    );
    let (g, rg) = ("/neofs/nolg.txt", "read /neofs/nolg.txt");
    // Asked; the Always isn't appended (the test fails it).
    log.push(Asked, 1, 0, g, rg);
    log.push(Asked, 1, 0, g, rg);
    log.push(AllowedOnce, 1, 0, g, rg);
    log.push(Done, 1, 0, g, rg);
    log.push(Asked, 1, 0, t, rt);
    log.push(AllowedAlways, 1, 0, t, rt);
    log.push(Done, 1, 0, t, rt);
    log.push(
        Refused,
        3,
        action_log_core::REFUSED_READ_ONLY,
        w,
        "write /neofs/notes.txt hello",
    );
    // #248 bounce F2's escaped read (the stall, refused before its path is
    // read, adds nothing).
    let (e, re) = ("/neofs/escape.txt", "escapeprobe");
    log.push(Asked, 1, 0, e, re);
    log.push(AllowedOnce, 1, 0, e, re);
    log.push(Done, 1, 0, e, re);
    log.push(Asked, 1, 0, e, "escapewait");
    log.push(AllowedOnce, 1, 0, e, "escapewait");
    log.push(Done, 1, 0, e, "escapewait");
    // The other surface's request adds nothing; its done neither.
    log.push(LentFromRecord, 1, 0, t, "badidentity");
    log.push(Done, 1, 0, t, "badidentity");
    let (f, rf) = ("/neofs/full.txt", "read /neofs/full.txt");
    for _ in 0..2 {
        log.push(Asked, 1, 0, f, rf);
        log.push(AllowedOnce, 1, 0, f, rf);
        log.push(Done, 1, 0, f, rf);
    }
    log.push(Asked, 2, 0, l, "list /neofs/docs/");
    log.push(
        Denied,
        2,
        action_log_core::REFUSED_LOCKED,
        l,
        "list /neofs/docs/",
    );
    require_serial_sequence(
        scenario,
        &first,
        &[
            "ACTIONLOG chain broken at=190",
            "ACTIONLOG rotated dropped_bytes=",
            &log.verify_line(),
        ],
        "boot 1's action log: the seeded torn tail repaired (a rewrite over 4 KiB), rotated past the cap with the chain intact, then exactly its records",
    )?;
    let serial = read_serial(&first)?;
    if serial.matches("ACTIONLOG append=failed").count() != 1 {
        return Err(format!(
            "`{}` boot 1: every append but the test's failed Always must land",
            scenario.name
        ));
    }
    if serial.matches("PERMISSION prompt").count() != 10 {
        return Err(format!(
            "`{}` boot 1: ten prompts expected (once, deny, the unlogged Always and its retry, always, the two escaped reads, the full record's two, the locked list)",
            scenario.name
        ));
    }
    let second = boot(
        scenario,
        attempt,
        "boot2",
        ovmf,
        vars,
        boot_disk,
        data,
        Some(PERMISSION2),
    )?;
    require_serial_sequence(
        scenario,
        &second,
        &[
            "LOGIN signed in",
            &todo("PERMISSION recorded"),
            "DESKTOP arthur permission action=read path=/neofs/todo.txt status=30 always",
            "DESKTOP arthur action outcome=8",
            "PERMISSION done outcome=0 revoked=1",
            // #248 bounce F1: the idle lock engages while a request stalls.
            "DESKTOP arthur permission stall sent",
            "LOCK locked reason=idle",
            "LOCK unlocked failures=0",
            "PERMISSION refused reason=incomplete",
            "DESKTOP arthur permission stall status=33",
            // Dexter 213 item 1: the same, waiting for the intent.
            "DESKTOP arthur permission intent stall sent",
            "LOCK locked reason=idle",
            "LOCK unlocked failures=0",
            "PERMISSION refused reason=incomplete",
            "DESKTOP arthur permission intent stall status=33",
            "LOCK locked reason=sak",
            "LOCK boot test done",
        ],
        "boot 2: the path allowed Always is lent from the record, with no prompt; a request stalled on its path or its intent can't keep the idle lock out",
    )?;
    log.push(LentFromRecord, 1, 0, t, rt);
    log.push(Done, 1, 0, t, rt);
    require_serial_sequence(
        scenario,
        &second,
        &[&log.verify_line()],
        "boot 2's action log must carry on boot 1's chain",
    )?;
    absent(
        scenario,
        &second,
        "ACTIONLOG chain broken",
        "boot 2 finds boot 1's log whole",
    )?;
    absent(
        scenario,
        &second,
        "PERMISSION prompt",
        "a recorded grant must not prompt again",
    )?;
    Ok(())
}

fn lock_session_checks(
    scenario: TestScenario,
    observed: &ObservedTest,
    dir: &Path,
) -> Result<(), String> {
    require_serial_sequence(
        scenario,
        observed,
        &[
            "LOGIN account created generation=1",
            "LOGIN session started view=home",
            "LOCK locked reason=sak",
            "LOCK prompt=password",
            "LOCK route deferred to=",
            "DESKTOP arthur lock probe committed picture=0 busy=sent",
            "LOCK password refused failures=1 next_delay_s=0",
            // F9 (J.c.): the lock screen says so too.
            "LOCK note=\"Incorrect password. Try again.\" row=last",
            "LOCK unlocked failures=1 to=client",
            "DESKTOP arthur keystroke ascii=113",
            "DESKTOP arthur lock probe committed picture=1 busy=sent",
            "LOCK locked reason=sak",
            "LOCK unlocked failures=0 to=client",
            "LOCK boot test done",
        ],
        "Super+L must lock Home, Arthur spawned under the lock must get no route until unlock, a wrong password must be refused, the right one must unlock to Arthur, and Super+L must lock Arthur while it holds the focus and never yields",
    )?;
    // AAG F2: Arthur flooded its channel with undecodable bytes the whole time
    // around the second lock (the lock engaged anyway: the sequence above).
    require_serial_contains(
        scenario,
        observed,
        "DESKTOP arthur lock flood sent=",
        "Arthur's flood must have run during the second lock",
    )?;
    // Every key typed while locked (`nope`, the password twice) went to the
    // lock screen: the only key Arthur ever received is the one typed after
    // the first unlock.
    let serial = read_serial(observed)?;
    let keys: Vec<&str> = serial
        .lines()
        .filter(|l| l.contains("DESKTOP arthur keystroke ascii="))
        .collect();
    if keys != ["DESKTOP arthur keystroke ascii=113"] {
        return Err(format!(
            "`{}`: a key typed while locked reached Arthur, or a stale key did after unlock: {keys:?}; serial={}",
            scenario.name,
            observed.serial_log.display()
        ));
    }
    let capture = |n: usize| -> Result<Vec<u8>, String> {
        let path = dir.join(format!("cap{n}.ppm"));
        fs::read(&path).map_err(|e| format!("`{}`: capture {}: {e}", scenario.name, path.display()))
    };
    let caps = (0..5).map(capture).collect::<Result<Vec<_>, _>>()?;
    for (a, b, what) in [
        (
            0,
            1,
            "Arthur, spawned under the lock, drew and asked for its emblem",
        ),
        (
            3,
            4,
            "Arthur committed a new picture every 300 ms and asked for its emblem",
        ),
    ] {
        if caps[a] != caps[b] {
            return Err(format!(
                "`{}`: the locked screen changed (cap{a} != cap{b}) while {what}: only the lock may be composited",
                scenario.name
            ));
        }
    }
    let probe = [(255u8, 0u8, 255u8), (0, 255, 255), (255, 255, 0)];
    let count = |bytes: &[u8], rgb: (u8, u8, u8)| -> Result<usize, String> {
        let (_, _, raster) = ppm_raster(bytes)
            .ok_or_else(|| format!("`{}`: a capture is not a PPM", scenario.name))?;
        Ok(raster
            .chunks_exact(3)
            .filter(|p| (p[0], p[1], p[2]) == rgb)
            .count())
    };
    for n in [0, 1, 3, 4] {
        for rgb in probe {
            let seen = count(&caps[n], rgb)?;
            if seen > 100 {
                return Err(format!(
                    "`{}`: the locked screen (cap{n}) shows {seen} pixels of Arthur's picture {rgb:?}",
                    scenario.name
                ));
            }
        }
    }
    let shown = count(&caps[2], probe[0])?;
    if shown < 100_000 {
        return Err(format!(
            "`{}`: after unlock (cap2) Arthur's picture, drawn under the lock, must show: {shown} of its pixels",
            scenario.name
        ));
    }
    eprintln!(
        "[xtask] neo-test: `{}` locked frames unchanged (cap0 = cap1, cap3 = cap4), no probe pixels while locked, {shown} after unlock",
        scenario.name
    );
    Ok(())
}

fn lock_stall_checks(scenario: TestScenario, observed: &ObservedTest) -> Result<(), String> {
    require_serial_sequence(
        scenario,
        observed,
        &[
            "LOGIN session started view=home",
            "DESKTOP arthur lock stall before=M",
            "LOCK locked reason=sak",
            "LOCK prompt=password",
            "LOCK unlocked failures=0",
            "LOCK boot test done",
        ],
        "Super+L while Arthur stalls in its handshake must lock and paint the lock screen",
    )?;
    // During the stall, not after it: the WM waits on Arthur's 'M' meanwhile.
    let serial = read_serial(observed)?;
    let locked = serial.find("LOCK locked reason=sak");
    let over = serial.find("DESKTOP arthur lock stall over");
    if let (Some(locked), Some(over)) = (locked, over)
        && over < locked
    {
        return Err(format!(
            "`{}`: the lock engaged only after Arthur's stall ended: a stalled client held it off",
            scenario.name
        ));
    }
    Ok(())
}

fn lock_park_checks(scenario: TestScenario, observed: &ObservedTest) -> Result<(), String> {
    // First the property: `x`, typed after Super+L while the WM was parked,
    // never reached Arthur (its only key is the `p` that began the park).
    let serial = read_serial(observed)?;
    let keys: Vec<&str> = serial
        .lines()
        .filter(|l| l.contains("DESKTOP arthur keystroke ascii="))
        .collect();
    if keys != ["DESKTOP arthur keystroke ascii=112"] {
        return Err(format!(
            "`{}`: a key typed after Super+L reached Arthur while the WM was parked in a send: {keys:?}; serial={}",
            scenario.name,
            observed.serial_log.display()
        ));
    }
    require_serial_sequence(
        scenario,
        observed,
        &[
            "LOCK armed route=kernel",
            "LOGIN session started view=home",
            "DESKTOP arthur keystroke ascii=112",
            "DESKTOP arthur lock park begin",
            "DESKTOP arthur lock park over",
            "LOCK locked reason=sak",
            "LOCK prompt=password",
            "LOCK unlocked failures=0 to=client",
            "LOCK boot test done",
        ],
        "with the WM parked in a send to Arthur, Super+L must lock the kernel's route; the lock screen follows the park, then unlock returns the focus to Arthur",
    )
}

/// L1a.3 (`terminal_lock_park`, THINK-2026-10-31-l1a §6): no key typed while
/// locked reached the terminal or its child. The child reports each line it
/// reads as `got=` and its bytes in hex; it read exactly one line, `abc\n`:
/// `ab` from before the lock (kept in the terminal's cooked line), `c` and
/// Enter from after the unlock, and nothing of the `x` typed while locked.
fn terminal_lock_checks(scenario: TestScenario, observed: &ObservedTest) -> Result<(), String> {
    let serial = read_serial(observed)?;
    let lines: Vec<&str> = serial
        .lines()
        .filter_map(|l| l.split_once("|got=").map(|(_, rest)| rest))
        .collect();
    if lines != ["6162630a|"] {
        return Err(format!(
            "`{}`: the terminal's child must read exactly `abc\\n` (got=6162630a), not a key typed while locked: {lines:?}; serial={}",
            scenario.name,
            observed.serial_log.display()
        ));
    }
    require_serial_sequence(
        scenario,
        observed,
        &[
            "LOCK armed route=kernel",
            "LOGIN session started view=home",
            "DESKTOP apps front client=3 via=launch",
            "TERM row=1 |ab|",
            "LOCK locked reason=sak",
            "LOCK prompt=password",
            "LOCK unlocked failures=1 to=client",
            "DESKTOP apps client resumed client=3",
            "TERM row=1 |abc|",
            "TERM row=2 |got=6162630a|",
            "LOCK unlocked failures=0 to=client",
            "LOCK boot test done",
        ],
        "L1a.3: what is typed while the session is locked never reaches the terminal or its child; the unlock gives the terminal its turn back",
    )
}

/// A2 (Muse, #98): no client is signalled while the session is locked. The
/// Super+Tab taken with Super+L is dropped once the lock is up, so no switch
/// (whose discard step sends the new front its `K`) happens between the lock
/// and the unlock; the terminal keeps the front through the unlock, and
/// Super+Tab works again after it.
fn switch_lock_checks(scenario: TestScenario, observed: &ObservedTest) -> Result<(), String> {
    let serial = read_serial(observed)?;
    let locked_at = serial.find("LOCK locked reason=sak");
    let unlocked_at = serial.find("LOCK unlocked failures=0 to=client");
    if let (Some(from), Some(to)) = (locked_at, unlocked_at) {
        if let Some(line) = serial[from..to]
            .lines()
            .find(|l| l.contains("DESKTOP apps front client="))
        {
            return Err(format!(
                "`{}`: a client was switched to (and sent its `K`) while the session was locked: `{line}`; serial={}",
                scenario.name,
                observed.serial_log.display()
            ));
        }
    }
    require_serial_sequence(
        scenario,
        observed,
        &[
            "LOCK armed route=kernel",
            "LOGIN session started view=home",
            "DESKTOP apps front client=3 via=launch",
            "DESKTOP apps sak hold ms=2000",
            "LOCK locked reason=sak",
            "DESKTOP apps switch dropped locked via=sak-tab",
            "LOCK prompt=password",
            "LOCK unlocked failures=0 to=client",
            "DESKTOP apps client resumed client=3",
            "DESKTOP apps front client=0 via=sak-tab",
            "LOCK locked reason=sak",
            "LOCK unlocked failures=0",
            "LOCK boot test done",
        ],
        "A2 (Muse, #98): a Super+Tab due in the turn the lock engages is dropped; no client is signalled while locked",
    )
}

fn lock_idle_checks(scenario: TestScenario, observed: &ObservedTest) -> Result<(), String> {
    require_serial_sequence(
        scenario,
        observed,
        &[
            "LOGIN session started view=home",
            "DESKTOP home keystroke ascii=100",
            "LOCK locked reason=idle idle_ms=",
            "LOCK unlocked failures=0 to=home",
            "LOCK boot test done",
        ],
        "keys 3 s apart must keep the session unlocked, and it must lock on the idle time after the last one",
    )?;
    let serial = read_serial(observed)?;
    let locks: Vec<&str> = serial
        .lines()
        .filter(|l| l.contains("LOCK locked"))
        .collect();
    let [line] = locks.as_slice() else {
        return Err(format!(
            "`{}`: exactly one lock expected (the idle one): {locks:?}",
            scenario.name
        ));
    };
    let idle_ms: u64 = line
        .rsplit("idle_ms=")
        .next()
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| format!("`{}`: unreadable `{line}`", scenario.name))?;
    if !(6_000..=8_000).contains(&idle_ms) {
        return Err(format!(
            "`{}`: the idle lock must fire at the 6 s idle time: {line}",
            scenario.name
        ));
    }
    eprintln!(
        "[xtask] neo-test: `{}` idle lock at idle_ms={idle_ms}",
        scenario.name
    );
    Ok(())
}

/// The `SETTINGS written` line the WM logs for `settings` at `generation`
/// in `file`: the digest is BLAKE3 over the record's own encoding (the
/// store's byte oracle is `settings-core`'s), the first 16 bytes in hex.
fn settings_written(settings: &settings_core::Settings, generation: u64, file: &str) -> String {
    let bytes = settings_core::encode(settings, generation);
    let digest = blake3::hash(&bytes);
    let hex: String = digest.as_bytes()[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("SETTINGS written generation={generation} file={file} digest={hex}")
}

/// F11 S2's seven-key record (`settings-core`'s `setup-keys`), hand-laid
/// here: xtask builds `settings-core` without the feature (the five-key
/// scenarios above need its five-key `encode`), so the two new entries,
/// Language (6) and KeyboardLayout (7), both `Choice` (2) at 0, are appended
/// by hand and the count and checksum cover them. `settings-core`'s
/// `setup_keys_tests.rs` holds the byte oracle (`DEFAULTS7_GEN1`).
fn record7(settings: &settings_core::Settings, generation: u64) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&settings_core::MAGIC);
    b.extend_from_slice(&settings_core::VERSION.to_le_bytes());
    b.extend_from_slice(&7u16.to_le_bytes());
    b.extend_from_slice(&generation.to_le_bytes());
    for key in settings_core::KEYS {
        b.extend_from_slice(&key.id().to_le_bytes());
        b.push(key.kind() as u8);
        b.push(0);
        b.extend_from_slice(&settings.get(key).to_le_bytes());
    }
    // Only the keys this build's `KEYS` lacks (Dexter #71 note 4): a build
    // that unifies `setup-keys` into xtask already wrote 6 and 7 above.
    let mut count = settings_core::KEYS.len();
    for id in [6u16, 7] {
        if settings_core::KEYS.iter().any(|k| k.id() == id) {
            continue;
        }
        b.extend_from_slice(&id.to_le_bytes());
        b.extend_from_slice(&[2, 0]);
        b.extend_from_slice(&0u32.to_le_bytes());
        count += 1;
    }
    assert_eq!(count, 7, "record7: a seven-key record");
    let sum = blake3::hash(&b);
    b.extend_from_slice(&sum.as_bytes()[..settings_core::CHECKSUM_LEN]);
    b
}

/// [`settings_written`] for a seven-key record.
fn settings7_written(settings: &settings_core::Settings, generation: u64, file: &str) -> String {
    let digest = blake3::hash(&record7(settings, generation));
    let hex: String = digest.as_bytes()[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("SETTINGS written generation={generation} file={file} digest={hex}")
}

/// F11 S2: the two boots of `login_setup_store`.
fn setup_store(
    scenario: TestScenario,
    attempt: usize,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
) -> Result<(), String> {
    use settings_core::{Motion, Settings};
    let run = |label: &str, steps| {
        boot(
            scenario,
            attempt,
            label,
            ovmf,
            vars,
            boot_disk,
            data,
            Some(steps),
        )
    };
    let defaults = Settings::default();
    let rt = Settings {
        reduce_transparency: true,
        ..defaults
    };
    let rm = Settings {
        reduce_motion: Motion::Reduced,
        ..rt
    };
    let w1 = settings7_written(&defaults, 1, "a");
    let w2 = settings7_written(&rt, 2, "b");
    let w3 = settings7_written(&rm, 3, "a");
    let first = run("boot1", SETUP_STORE1)?;
    require_serial_sequence(
        scenario,
        &first,
        &[
            "SETTINGS migrated rt=0 rm=0 idle=10 lang=0 kb=0",
            &w1,
            "DESKTOP setup page=welcome rows=5",
            "DESKTOP setup page=try rows=5",
            "DESKTOP setup done action=try zone=UTC reduce_motion=1 reduce_transparency=1",
            &w2,
            "DESKTOP material=reduce-transparency cause=settings live=1",
            "SETTING applied key=1 value=1 status=0",
            &w3,
            "SETTING applied key=2 value=1 status=0",
            "SETTING applied key=6 value=0 status=0",
            "SETTING applied key=7 value=0 status=0",
            "DESKTOP setup stored keys=4 refused=0 volatile=0",
            "LOGIN account created generation=1",
            "LOGIN session started view=home",
        ],
        "boot 1 must find no record, run the menu, and write each changed choice through the WM's path (the record read back by digest), the two keys already at their only value writing nothing, before the account is made",
    )?;
    let serial = read_serial(&first)?;
    let writes = serial.matches("SETTINGS written").count();
    if writes != 3 {
        return Err(format!(
            "`{}`: three store writes expected (the first record, Reduce transparency, Reduce motion), saw {writes}",
            scenario.name
        ));
    }
    let second = run("boot2", SETUP_STORE2)?;
    require_serial_sequence(
        scenario,
        &second,
        &[
            "SETTINGS loaded generation=3 file=a rt=1 rm=1 idle=10 lang=0 kb=0",
            "DESKTOP material=reduce-transparency cause=settings",
            "DESKTOP setup skipped reason=stored",
            "LOGIN signed in",
            "LOGIN session started view=home",
        ],
        "boot 2 must build from the record the menu wrote (generation 3, file a) and not show the menu again",
    )?;
    for (needle, why) in [
        (
            "DESKTOP setup page=",
            "a stored record means the menu is done",
        ),
        (
            "SETTINGS written",
            "a boot that only reads the store must write nothing",
        ),
        ("SETTINGS migrated", "the record exists"),
        // Dexter #71 note 2: no write attempt at all, failed or resynced.
        (
            "SETTINGS write=",
            "a boot that only reads the store attempts no write",
        ),
        (
            "SETTINGS resync",
            "a boot that only reads the store attempts no write",
        ),
    ] {
        absent(scenario, &second, needle, why)?;
    }
    Ok(())
}

/// The `DESKTOP setup journal page=<page> ...` line's `time_s`, `utc` and
/// `ok`, or an error naming the page.
fn journal_line(
    scenario: TestScenario,
    observed: &ObservedTest,
    page: &str,
) -> Result<(u64, u64, u64), String> {
    let serial = read_serial(observed)?;
    let needle = format!("DESKTOP setup journal page={page} ");
    let line = serial
        .lines()
        .find_map(|l| l.split(needle.as_str()).nth(1))
        .ok_or_else(|| {
            format!(
                "`{}`: no `{needle}` line; serial={}",
                scenario.name,
                observed.serial_log.display()
            )
        })?;
    let num = |key: &str| -> Result<u64, String> {
        line.split_whitespace()
            .find_map(|kv| kv.strip_prefix(key))
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| format!("`{}`: journal line lacks `{key}`: {line}", scenario.name))
    };
    Ok((num("time_s=")?, num("utc=")?, num("ok=")?))
}

/// F11 S3 (setup-menu THINK §5 S3): `login_setup_tz`, Jarvis's
/// discriminator, and its control `login_setup_tz_skip`.
fn setup_tz(
    scenario: TestScenario,
    attempt: usize,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
) -> Result<(), String> {
    let run = |label: &str, steps| {
        boot(
            scenario,
            attempt,
            label,
            ovmf,
            vars,
            boot_disk,
            data,
            Some(steps),
        )
    };
    use settings_core::Settings;
    let defaults = Settings::default();
    let ny = Settings {
        time_zone: NEW_YORK_ID,
        ..defaults
    };
    // Nothing may change without an accepted confirm (Chad, #92 relays 369
    // and 373): no confirmed clock, the readout still `zone not set`, the
    // zone never set key by key, and the wizard's records carrying the clock
    // as read, without TIME_UTC. `writes`: the store writes expected.
    let unchanged = |observed: &ObservedTest,
                     pages: &[&str],
                     writes: usize|
     -> Result<(), String> {
        for (needle, why) in [
            ("DESKTOP clock utc=confirmed", "no confirm was accepted"),
            ("SETTING applied key=4 ", "the zone is never set key by key"),
            (
                "SETTING applied key=5 ",
                "the RTC mode is never set key by key",
            ),
            (
                " zone=\"EDT\"",
                "the clock keeps showing the computer's clock, zone not set",
            ),
        ] {
            absent(scenario, observed, needle, why)?;
        }
        let serial = read_serial(observed)?;
        let seen = serial.matches("SETTINGS written").count();
        if seen != writes {
            return Err(format!(
                "`{}`: {writes} store write(s) expected, saw {seen}",
                scenario.name
            ));
        }
        for page in pages {
            let (t, utc, ok) = journal_line(scenario, observed, page)?;
            if utc != 0
                || ok != 1
                || !(SETUP_TZ_RTC_AS_UTC_S..SETUP_TZ_RTC_AS_UTC_S + 600).contains(&t)
            {
                return Err(format!(
                    "`{}`: the `{page}` record must carry the clock as read (near {SETUP_TZ_RTC_AS_UTC_S}), without TIME_UTC: time_s={t} utc={utc} ok={ok}",
                    scenario.name
                ));
            }
        }
        Ok(())
    };
    // The next boot reads the record back: the defaults (no zone) at
    // `generation` in `file`, and the clock still unconfirmed.
    let reboot_unchanged = |generation: u64, file: &str| -> Result<(), String> {
        let second = run("boot2", SETUP_STORE2)?;
        let loaded = format!(
            "SETTINGS loaded generation={generation} file={file} rt=0 rm=0 idle=10 lang=0 kb=0"
        );
        require_serial_sequence(
            scenario,
            &second,
            &[
                &loaded,
                "DESKTOP clock utc=unknown zone=",
                "zone=\"zone not set\"",
                "DESKTOP setup skipped reason=stored",
                "LOGIN signed in",
            ],
            "boot 2 reads the record the failed confirm left: no zone, so the clock stays unconfirmed",
        )?;
        absent(
            scenario,
            &second,
            "DESKTOP clock utc=confirmed",
            "no zone was stored, so nothing confirms the clock at the start",
        )
    };
    if scenario.name == LOGIN_SETUP_TZ_SKIP_SCENARIO {
        let first = run("boot1", SETUP_TZ_SKIP)?;
        require_serial_sequence(
            scenario,
            &first,
            &[
                "SETTINGS migrated rt=0 rm=0 idle=10 lang=0 kb=0",
                "zone=\"zone not set\"",
                "DESKTOP setup tz page=search rtc=local",
                "DESKTOP setup page=timezone rows=8",
                // New York chosen, then declined: back to Welcome, skip.
                "DESKTOP setup page=welcome rows=5",
                "DESKTOP setup page=skip rows=5",
                "DESKTOP setup done action=skip",
                "DESKTOP setup journal page=welcome choice=begin time_s=",
                "DESKTOP setup journal page=done choice=skip time_s=",
                "DESKTOP setup journal records=2 appended=2",
                "ACTIONLOG verify records=2 broken=none head=",
                "LOGIN account created generation=1",
            ],
            "the decline: New York chosen, then the page left backwards and the wizard skipped; no confirm is asked for",
        )?;
        for (needle, why) in [
            ("DESKTOP setup tz=", "a declined page makes no confirm call"),
            ("SETTINGS zone stored", "a declined page stores no zone"),
        ] {
            absent(scenario, &first, needle, why)?;
        }
        return unchanged(&first, &["welcome", "done"], 1);
    }
    if scenario.name == LOGIN_SETUP_TZ_REFUSED_SCENARIO {
        // The zone is stored first (the confirm can't be undone), the kernel
        // refuses, and the record the boot started from is written back.
        let w2 = settings7_written(&ny, 2, "b");
        let w3 = settings7_written(&defaults, 3, "a");
        let first = run("boot1", SETUP_TZ1)?;
        require_serial_sequence(
            scenario,
            &first,
            &[
                "SETTINGS migrated rt=0 rm=0 idle=10 lang=0 kb=0",
                "zone=\"zone not set\"",
                "DESKTOP setup tz page=search rtc=local",
                "DESKTOP setup page=timezone rows=8",
                &w2,
                "SETTINGS zone stored zone=105 rtc=1",
                "DESKTOP clock confirm refused errno=114",
                &w3,
                "SETTINGS zone restored",
                // The PM's rule: the record re-read is what is in force.
                "SETTINGS reread generation=3 file=a zone=0 rtc=1",
                "DESKTOP clock utc=reconciled-rtc zone=",
                "DESKTOP setup tz=America/New_York confirmed=0",
                "DESKTOP setup tz reconciled zone=not set confirmed=0",
                "DESKTOP setup page=comfort rows=7",
                "DESKTOP setup page=try rows=5",
                "DESKTOP setup done action=try zone=unset",
                "DESKTOP setup journal records=4 appended=4",
                "ACTIONLOG verify records=4 broken=none head=",
                "LOGIN account created generation=1",
            ],
            "the refused confirm: the kernel answers EALREADY, the record the boot started from is written back, and the wizard goes on with the clock as it was",
        )?;
        unchanged(&first, &["welcome", "timezone", "comfort", "done"], 3)?;
        return reboot_unchanged(3, "a");
    }
    if scenario.name == LOGIN_SETUP_TZ_WRITE_FAIL_SCENARIO {
        let first = run("boot1", SETUP_TZ1)?;
        require_serial_sequence(
            scenario,
            &first,
            &[
                "SETTINGS migrated rt=0 rm=0 idle=10 lang=0 kb=0",
                "zone=\"zone not set\"",
                "DESKTOP setup tz page=search rtc=local",
                "DESKTOP setup page=timezone rows=8",
                "SETTINGS write=failed file=b",
                "SETTINGS zone store=failed",
                "DESKTOP clock confirm skipped reason=store",
                "SETTINGS reread generation=1 file=a zone=0 rtc=1",
                "DESKTOP clock utc=reconciled-rtc zone=",
                "DESKTOP setup tz=America/New_York confirmed=0",
                "DESKTOP setup tz reconciled zone=not set confirmed=0",
                // The notice row: Comfort shows one more.
                "DESKTOP setup page=comfort rows=7",
                "DESKTOP setup page=try rows=5",
                "DESKTOP setup journal records=4 appended=4",
                "ACTIONLOG verify records=4 broken=none head=",
                "LOGIN account created generation=1",
            ],
            "the failed store write: nothing is stored, the kernel is never asked, and the wizard goes on with the clock as it was",
        )?;
        for (needle, why) in [
            ("SETTINGS zone stored", "the zone's write failed"),
            (
                "DESKTOP clock confirm refused",
                "the kernel is never asked after a failed store",
            ),
        ] {
            absent(scenario, &first, needle, why)?;
        }
        unchanged(&first, &["welcome", "timezone", "comfort", "done"], 1)?;
        return reboot_unchanged(1, "a");
    }
    // Chad's relay-377 cases: the store ends up holding New York (generation
    // 2, file b) though the confirm failed. The PM's rule: the record re-read
    // is put in force, so the clock is confirmed with New York now, exactly
    // as the next boot builds it.
    let reconciled_to_new_york = |first: &ObservedTest,
                                  how: &[&str],
                                  why: &str|
     -> Result<(), String> {
        let mut want: Vec<&str> = vec![
            "SETTINGS migrated rt=0 rm=0 idle=10 lang=0 kb=0",
            "zone=\"zone not set\"",
            "DESKTOP setup tz page=search rtc=local",
            "DESKTOP setup page=timezone rows=8",
        ];
        want.extend_from_slice(how);
        want.extend_from_slice(&[
            "SETTINGS reread generation=2 file=b zone=105 rtc=1",
            "DESKTOP clock utc=confirmed zone=America/New_York rtc=local offset_s=-14400",
            " zone=\"EDT\" realtime_s=",
            "DESKTOP clock utc=reconciled-utc zone=America/New_York",
            "DESKTOP setup tz=America/New_York confirmed=0",
            "DESKTOP setup tz reconciled zone=America/New_York confirmed=1",
            "DESKTOP setup page=comfort rows=7",
            "DESKTOP setup page=try rows=5",
            "DESKTOP setup done action=try zone=America/New_York",
            "DESKTOP setup journal records=4 appended=4",
            "ACTIONLOG verify records=4 broken=none head=",
            "LOGIN account created generation=1",
        ]);
        require_serial_sequence(scenario, first, &want, why)?;
        // The clock is confirmed now, so every record carries UTC.
        for page in ["welcome", "timezone", "comfort", "done"] {
            let (t, utc, ok) = journal_line(scenario, first, page)?;
            if utc != 1 || ok != 1 || !(SETUP_TZ_UTC_S..SETUP_TZ_UTC_S + 600).contains(&t) {
                return Err(format!(
                    "`{}`: the `{page}` record must carry the confirmed UTC time (from {SETUP_TZ_UTC_S}): time_s={t} utc={utc} ok={ok}",
                    scenario.name
                ));
            }
        }
        let second = run("boot2", SETUP_STORE2)?;
        require_serial_sequence(
            scenario,
            &second,
            &[
                "SETTINGS loaded generation=2 file=b rt=0 rm=0 idle=10 lang=0 kb=0",
                "DESKTOP clock utc=confirmed zone=America/New_York rtc=local offset_s=-14400",
                "DESKTOP setup skipped reason=stored",
                "LOGIN signed in",
            ],
            "boot 2 builds what boot 1 had in force after the failure: New York, confirmed",
        )
    };
    if scenario.name == LOGIN_SETUP_TZ_FSYNC_FAIL_SCENARIO {
        let first = run("boot1", SETUP_TZ1)?;
        reconciled_to_new_york(
            &first,
            &[
                "SETTINGS write=failed file=b",
                "SETTINGS zone store=failed",
                "DESKTOP clock confirm skipped reason=store",
            ],
            "the write that lands but reports failure (Chad, relay 377 case 1): the store holds New York, so New York is put in force and the clock confirmed with it",
        )?;
        return absent(
            scenario,
            &first,
            "DESKTOP clock confirm refused",
            "the kernel's own confirm is never asked after a failed store",
        );
    }
    if scenario.name == LOGIN_SETUP_TZ_REREAD_FAIL_SCENARIO {
        // The New York record lands but reports failure, and then neither
        // copy can be read: what the next boot selects is unknown, so nothing
        // is put in force and no stored zone is claimed.
        let first = run("boot1", SETUP_TZ_REREAD)?;
        require_serial_sequence(
            scenario,
            &first,
            &[
                "SETTINGS migrated rt=0 rm=0 idle=10 lang=0 kb=0",
                "zone=\"zone not set\"",
                "DESKTOP setup tz page=search rtc=local",
                "DESKTOP setup page=timezone rows=8",
                "SETTINGS write=failed file=b",
                "SETTINGS zone store=failed",
                "DESKTOP clock confirm skipped reason=store",
                "SETTINGS reread unresolved",
                "DESKTOP clock utc=unresolved zone=",
                "DESKTOP setup tz=America/New_York confirmed=0",
                "DESKTOP setup tz unresolved notice=\"Couldn't save the time zone. Restart to check it.\"",
                // The page stays, with the notice row.
                "DESKTOP setup page=timezone rows=6",
                "DESKTOP setup page=welcome rows=5",
                "DESKTOP setup page=skip rows=5",
                "DESKTOP setup done action=skip",
                "DESKTOP setup journal records=2 appended=2",
                "ACTIONLOG verify records=2 broken=none head=",
                "LOGIN account created generation=1",
            ],
            "the unreadable re-read (Chad, relay 379): the confirm is unresolved, the clock stays as it was, and the page stays with a notice that names no stored zone",
        )?;
        for (needle, why) in [
            (
                "SETTINGS reread generation=",
                "neither copy could be read, so no record is claimed",
            ),
            (
                "DESKTOP setup tz reconciled",
                "an unresolved confirm reconciles nothing",
            ),
            ("In use:", "the notice names no stored zone"),
            (
                "DESKTOP clock confirm refused",
                "the kernel's own confirm is never asked after a failed store",
            ),
        ] {
            absent(scenario, &first, needle, why)?;
        }
        unchanged(&first, &["welcome", "done"], 1)?;
        // Boot 2 selects what the store holds: New York, generation 2 in b
        // (the bytes landed), and confirms the clock with it.
        let second = run("boot2", SETUP_STORE2)?;
        return require_serial_sequence(
            scenario,
            &second,
            &[
                "SETTINGS loaded generation=2 file=b rt=0 rm=0 idle=10 lang=0 kb=0",
                "DESKTOP clock utc=confirmed zone=America/New_York rtc=local offset_s=-14400",
                "DESKTOP setup skipped reason=stored",
                "LOGIN signed in",
            ],
            "boot 2 selects the record the store holds: New York, generation 2",
        );
    }
    if scenario.name == LOGIN_SETUP_TZ_ROLLBACK_FAIL_SCENARIO {
        let w2 = settings7_written(&ny, 2, "b");
        let first = run("boot1", SETUP_TZ1)?;
        return reconciled_to_new_york(
            &first,
            &[
                &w2,
                "SETTINGS zone stored zone=105 rtc=1",
                "DESKTOP clock confirm refused errno=114",
                "SETTINGS write=failed file=a",
                "SETTINGS zone restore=failed",
            ],
            "the refused confirm whose rollback fails (Chad, relay 377 case 2): the store still holds New York, so New York is put in force and the clock confirmed with it",
        );
    }
    let w1 = settings7_written(&defaults, 1, "a");
    let w2 = settings7_written(&ny, 2, "b");
    let first = run("boot1", SETUP_TZ1)?;
    require_serial_sequence(
        scenario,
        &first,
        &[
            "SETTINGS migrated rt=0 rm=0 idle=10 lang=0 kb=0",
            &w1,
            "zone=\"zone not set\"",
            "DESKTOP setup tz page=search rtc=local",
            "DESKTOP setup page=timezone rows=8",
            // Leaving the page: the zone stored, the clock confirmed once.
            &w2,
            "SETTINGS zone stored zone=105 rtc=1",
            "DESKTOP clock utc=confirmed zone=America/New_York rtc=local offset_s=-14400",
            " zone=\"EDT\" realtime_s=",
            "DESKTOP setup tz=America/New_York confirmed=1",
            "DESKTOP setup page=comfort rows=6",
            "DESKTOP setup page=try rows=5",
            "DESKTOP setup done action=try zone=America/New_York reduce_motion=0 reduce_transparency=0",
            "DESKTOP setup stored keys=4 refused=0 volatile=0",
            // The queue, appended after the confirm, in order.
            "DESKTOP setup journal page=welcome choice=begin time_s=",
            "DESKTOP setup journal page=timezone choice=America/New_York time_s=",
            "DESKTOP setup journal page=comfort choice=rm=0 rt=0 time_s=",
            "DESKTOP setup journal page=done choice=try time_s=",
            "DESKTOP setup journal records=4 appended=4",
            "ACTIONLOG verify records=4 broken=none head=",
            "LOGIN account created generation=1",
            "LOGIN session started view=home",
        ],
        "boot 1: the search finds New York, leaving the page stores it and confirms the clock once (EDT, the earlier instant of the repeated hour), and the wizard's choices are appended after the confirm, the chain verifying end to end",
    )?;
    // Jarvis's discriminator: the Welcome choice was made before the
    // confirm, yet its record carries the confirmed UTC instant (05:57Z
    // plus the walk), not the RTC's 01:57 read as UTC (four hours behind).
    for page in ["welcome", "timezone", "comfort", "done"] {
        let (t, utc, ok) = journal_line(scenario, &first, page)?;
        if utc != 1 || ok != 1 || !(SETUP_TZ_UTC_S..SETUP_TZ_UTC_S + 600).contains(&t) {
            return Err(format!(
                "`{}`: the `{page}` record must carry the confirmed UTC time (from {SETUP_TZ_UTC_S}, not {SETUP_TZ_RTC_AS_UTC_S}) with TIME_UTC: time_s={t} utc={utc} ok={ok}",
                scenario.name
            ));
        }
    }
    let serial = read_serial(&first)?;
    if serial.matches("DESKTOP clock utc=confirmed").count() != 1
        || serial.matches("DESKTOP setup tz=").count() != 1
    {
        return Err(format!(
            "`{}`: exactly one confirm expected; serial={}",
            scenario.name,
            first.serial_log.display()
        ));
    }
    let writes = serial.matches("SETTINGS written").count();
    if writes != 2 {
        return Err(format!(
            "`{}`: two store writes expected (the first record, the zone), saw {writes}",
            scenario.name
        ));
    }
    let second = run("boot2", SETUP_STORE2)?;
    require_serial_sequence(
        scenario,
        &second,
        &[
            "SETTINGS loaded generation=2 file=b rt=0 rm=0 idle=10 lang=0 kb=0",
            "DESKTOP clock utc=confirmed zone=America/New_York rtc=local offset_s=-14400",
            "DESKTOP setup skipped reason=stored",
            "LOGIN signed in",
        ],
        "boot 2: the stored zone confirms the clock at the start, and the menu is done",
    )?;
    for (needle, why) in [
        (
            "DESKTOP setup page=",
            "a stored record means the menu is done",
        ),
        (
            "SETTINGS written",
            "a boot that only reads the store writes nothing",
        ),
        (
            "zone=\"zone not set\"",
            "the stored zone confirms the clock",
        ),
    ] {
        absent(scenario, &second, needle, why)?;
    }
    Ok(())
}

/// OT-88 S1: the two boots of `login_settings_store`.
fn settings_store(
    scenario: TestScenario,
    attempt: usize,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
) -> Result<(), String> {
    use settings_core::{Motion, Settings};
    let run = |label: &str, steps| {
        boot(
            scenario,
            attempt,
            label,
            ovmf,
            vars,
            boot_disk,
            data,
            Some(steps),
        )
    };
    let defaults = Settings::default();
    let idle = Settings {
        idle_lock_minutes: 5,
        ..defaults
    };
    let rt = Settings {
        reduce_transparency: true,
        ..idle
    };
    let rm = Settings {
        reduce_motion: Motion::Off,
        ..rt
    };
    let rm_full = Settings {
        reduce_motion: Motion::Full,
        ..rt
    };
    let first = run("boot1", SETTINGS1)?;
    let w1 = settings_written(&defaults, 1, "a");
    let w2 = settings_written(&idle, 2, "b");
    let w3 = settings_written(&rt, 3, "a");
    let w4 = settings_written(&rm, 4, "b");
    let w5 = settings_written(&rm_full, 5, "a");
    let w6 = settings_written(&rm, 6, "b");
    require_serial_sequence(
        scenario,
        &first,
        &[
            "SETTINGS migrated rt=0 rm=0 idle=10",
            &w1,
            "DESKTOP material=glass cause=settings",
            "DESKTOP motion profile=full cause=settings",
            "LOGIN account created generation=1",
            "LOGIN session started view=home",
            // The idle lock is the WM's to set (the PM's #222 bounce).
            &w2,
            "SETTING applied key=3 value=5 status=0",
            &w3,
            "DESKTOP material=reduce-transparency cause=settings live=1",
            "SETTING applied key=1 value=1 status=0",
            "DESKTOP arthur setting key=1 value=1 status=0",
            &w4,
            "SETTING applied key=2 value=2 status=0",
            "DESKTOP arthur setting key=2 value=2 status=0",
            "SETTING refused key=3 value=0 status=23",
            "DESKTOP arthur setting key=3 value=0 status=23",
            "SETTING refused key=2 value=7 status=21",
            "DESKTOP arthur setting key=2 value=7 status=21",
            "SETTING refused key=9 value=0 status=20",
            "DESKTOP arthur setting key=9 value=0 status=20",
            // The burst: the bucket's last two writes, then the rate limit
            // (a value already in force costs nothing and writes nothing).
            &w5,
            "SETTING applied key=2 value=0 status=0",
            "DESKTOP arthur setting key=2 value=0 status=0",
            &w6,
            "SETTING applied key=2 value=2 status=0",
            "DESKTOP arthur setting key=2 value=2 status=0",
            "SETTING refused key=2 value=0 status=24",
            "DESKTOP arthur setting key=2 value=0 status=24",
            "SETTING applied key=2 value=2 status=0",
            "DESKTOP arthur setting key=2 value=2 status=0",
            "SETTING refused key=2 value=0 status=24",
            "DESKTOP arthur setting key=2 value=0 status=24",
        ],
        "boot 1 must migrate and write the first record, set the idle lock the WM's way, then apply each client setting live with its store write read back by digest, refuse the WM-only idle lock, a bad value and a bad key, and rate-limit the burst",
    )?;
    // The idle lock follows the idle setting (it may come before or after
    // the client's requests, which don't change it).
    require_serial_sequence(
        scenario,
        &first,
        &[
            "SETTING applied key=3 value=5 status=0",
            "LOCK locked reason=idle idle_ms=",
            "LOCK unlocked failures=0",
            "LOCK boot test done",
        ],
        "the idle lock must fire after the idle time is set, and unlock",
    )?;
    // The idle lock at the chosen 5 (20 s in the boot test, which counts a
    // minute as 4 s), not the default 10 (40 s), and not off (the refused 0).
    // It counts from the last key (`hi`, just before the WM sets it).
    let idle_ms = idle_lock_ms(scenario, &first)?;
    if !(20_000..40_000).contains(&idle_ms) {
        return Err(format!(
            "`{}`: the idle lock must fire at the chosen 5 (20 s here), not the default (40 s): idle_ms={idle_ms}",
            scenario.name
        ));
    }
    // A refused setting writes nothing: exactly the six records.
    let serial = read_serial(&first)?;
    let writes = serial.matches("SETTINGS written").count();
    if writes != 6 {
        return Err(format!(
            "`{}`: six store writes expected (the first record, the idle lock and four client settings), saw {writes}",
            scenario.name
        ));
    }
    let second = run("boot2", SETTINGS2)?;
    require_serial_sequence(
        scenario,
        &second,
        &[
            "SETTINGS loaded generation=6 file=b rt=1 rm=2 idle=5",
            "DESKTOP material=reduce-transparency cause=settings",
            "DESKTOP motion profile=off cause=settings",
            "LOGIN signed in",
            "LOCK locked reason=sak",
            "LOCK boot test done",
        ],
        "boot 2 must build from the store's record: generation 6, in file b",
    )?;
    absent(
        scenario,
        &second,
        "SETTINGS written",
        "a boot that only reads the store must write nothing",
    )?;
    eprintln!(
        "[xtask] neo-test: `{}` idle lock at idle_ms={idle_ms}",
        scenario.name
    );
    Ok(())
}

/// The `LOCK locked reason=idle idle_ms=` value of `observed`.
fn idle_lock_ms(scenario: TestScenario, observed: &ObservedTest) -> Result<u64, String> {
    let serial = read_serial(observed)?;
    let line = serial
        .lines()
        .find(|l| l.contains("LOCK locked reason=idle"))
        .unwrap_or_default();
    line.rsplit("idle_ms=")
        .next()
        .and_then(|v| v.trim().parse().ok())
        .ok_or_else(|| format!("`{}`: unreadable `{line}`", scenario.name))
}

/// The PM's #222 bounce: Arthur (not the settings app) asks for the idle
/// lock off and Reduce Transparency on. Both are refused (`CapRequired`, 7),
/// nothing but the first record is written, and the idle lock fires at the
/// default 10 (40 s in the boot test). The control
/// (`settings-authority-control`) takes them: the store is written and the
/// idle lock never fires.
fn settings_refused(
    scenario: TestScenario,
    attempt: usize,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
) -> Result<(), String> {
    let observed = boot(
        scenario,
        attempt,
        "boot1",
        ovmf,
        vars,
        boot_disk,
        data,
        Some(SETTINGS1),
    )?;
    let w1 = settings_written(&settings_core::Settings::default(), 1, "a");
    require_serial_sequence(
        scenario,
        &observed,
        &[
            "SETTINGS migrated rt=0 rm=0 idle=10",
            &w1,
            "LOGIN account created generation=1",
            "LOGIN session started view=home",
            "SETTING refused key=3 value=0 status=7",
            "DESKTOP arthur setting key=3 value=0 status=7",
            "SETTING refused key=1 value=1 status=7",
            "DESKTOP arthur setting key=1 value=1 status=7",
            "LOCK locked reason=idle idle_ms=",
            "LOCK unlocked failures=0",
            "LOCK boot test done",
        ],
        "a client that isn't the settings app must be refused OP_SETTING, and the idle lock must still fire",
    )?;
    let serial = read_serial(&observed)?;
    let writes = serial.matches("SETTINGS written").count();
    if writes != 1 {
        return Err(format!(
            "`{}`: a refused OP_SETTING must write nothing: one store write (the first record) expected, saw {writes}",
            scenario.name
        ));
    }
    absent(
        scenario,
        &observed,
        "material=reduce-transparency",
        "a refused Reduce Transparency must not be applied",
    )?;
    // The default 10 (40 s here), unchanged.
    let idle_ms = idle_lock_ms(scenario, &observed)?;
    if idle_ms < 40_000 {
        return Err(format!(
            "`{}`: the idle lock must fire at the default 10 (40 s here): idle_ms={idle_ms}",
            scenario.name
        ));
    }
    eprintln!(
        "[xtask] neo-test: `{}` refused, idle lock at idle_ms={idle_ms}",
        scenario.name
    );
    Ok(())
}

/// `SETTINGS cut verify record=R generation=G idle=I a_blake3=A b_blake3=B`.
#[derive(Debug, PartialEq, Eq)]
struct SettingsVerify {
    record: String,
    generation: u64,
    idle: u32,
    a: String,
    b: String,
}

fn settings_verify_line(
    scenario: TestScenario,
    observed: &ObservedTest,
) -> Result<SettingsVerify, String> {
    let serial = read_serial(observed)?;
    let line = serial
        .lines()
        .find_map(|l| l.split("SETTINGS cut verify ").nth(1))
        .ok_or_else(|| {
            format!(
                "`{}` verify boot logged no `SETTINGS cut verify`; serial={}",
                scenario.name,
                observed.serial_log.display()
            )
        })?;
    let field = |key: &str| -> Result<String, String> {
        line.split_whitespace()
            .find_map(|kv| kv.strip_prefix(key))
            .map(str::to_string)
            .ok_or_else(|| format!("`{}` verify line lacks `{key}`: {line}", scenario.name))
    };
    let num = |key: &str| -> Result<u64, String> {
        field(key)?
            .parse()
            .map_err(|_| format!("`{}` unreadable `{key}`: {line}", scenario.name))
    };
    Ok(SettingsVerify {
        record: field("record=")?,
        generation: num("generation=")?,
        idle: num("idle=")? as u32,
        a: field("a_blake3=")?,
        b: field("b_blake3=")?,
    })
}

/// OT-88 S1: a power cut after each device write and flush of a settings
/// write (L1's cut). The setup boot migrates and writes the first record
/// (generation 1, file a, the idle lock at 10). Each cut boot arms the cut
/// and sets the idle lock to 30, which writes b. Every cut must leave the old
/// record (a, generation 1, idle 10) or the new one (b, generation 2, idle
/// 30), never neither, and a byte for byte as before.
fn settings_cut(
    scenario: TestScenario,
    attempt: usize,
    ovmf: &vm_image::Ovmf,
    vars: &Path,
    boot_disk: &Path,
    data: &Path,
) -> Result<(), String> {
    let dir = data.parent().expect("data disk has a directory");
    let cut_boot = dir.join("boot-cut.raw");
    copy_disk(boot_disk, &cut_boot)?;
    let verify = |label: &str, disk: &Path| -> Result<SettingsVerify, String> {
        set_cut_step(&cut_boot, 0)?;
        let observed = boot(scenario, attempt, label, ovmf, vars, &cut_boot, disk, None)?;
        settings_verify_line(scenario, &observed)
    };
    // The setup boot: the first record, then its verify.
    let before = verify("setup", data)?;
    if (before.record.as_str(), before.generation, before.idle) != ("a", 1, 10) {
        return Err(format!("`{}` after setup: {before:?}", scenario.name));
    }
    let case = dir.join("data-case.raw");
    // Calibration: the whole write, uncut, and how many ops it takes.
    copy_disk(data, &case)?;
    set_cut_step(&cut_boot, 9999)?;
    let full = boot(
        scenario, attempt, "full", ovmf, vars, &cut_boot, &case, None,
    )?;
    let serial = read_serial(&full)?;
    let ops: u32 = serial
        .lines()
        .find_map(|l| l.split("SETTINGS cut not-reached step=9999 ops=").nth(1))
        .and_then(|n| n.trim().parse().ok())
        .ok_or_else(|| {
            format!(
                "`{}` the uncut write logged no op count; serial={}",
                scenario.name,
                full.serial_log.display()
            )
        })?;
    if ops == 0 || ops > 200 {
        return Err(format!("`{}` the write took {ops} ops", scenario.name));
    }
    let after = verify("after", &case)?;
    if (after.record.as_str(), after.generation, after.idle) != ("b", 2, 30) || after.a != before.a
    {
        return Err(format!(
            "`{}` the uncut write must leave the new record in b, generation 2, with a's bytes unchanged: {after:?} (a before {})",
            scenario.name, before.a
        ));
    }
    let listed = cut_ops(&serial);
    if listed.len() != ops as usize {
        return Err(format!(
            "`{}` the calibration listed {} ops but counted {ops}",
            scenario.name,
            listed.len()
        ));
    }
    let steps: Vec<u32> = if env::var("NEO_LOGIN_CUT_ALL").is_ok_and(|v| v == "1") {
        (1..=ops).collect()
    } else {
        sampled_steps(&listed)
    };
    eprintln!(
        "[xtask] neo-test: `{}` the write takes {ops} device writes and flushes; cutting at {steps:?}",
        scenario.name
    );
    let mut seen_new = false;
    let mut table = Vec::new();
    for step in steps {
        copy_disk(data, &case)?;
        set_cut_step(&cut_boot, step)?;
        let cut = boot(
            scenario,
            attempt,
            &format!("cut{step:02}"),
            ovmf,
            vars,
            &cut_boot,
            &case,
            None,
        )?;
        let serial = read_serial(&cut)?;
        let fired = serial
            .lines()
            .find(|l| l.contains(&format!("stage=cut step={step} ")))
            .ok_or_else(|| {
                format!(
                    "`{}` step {step}: the cut did not fire; serial={}",
                    scenario.name,
                    cut.serial_log.display()
                )
            })?;
        let op = fired
            .split_whitespace()
            .filter(|kv| kv.starts_with("op=") || kv.starts_with("lba="))
            .collect::<Vec<_>>()
            .join(" ");
        let v = verify(&format!("cut{step:02}-verify"), &case)?;
        let outcome = match (v.record.as_str(), v.generation, v.idle) {
            ("a", 1, 10) => "old",
            ("b", 2, 30) => "new",
            _ => {
                return Err(format!(
                    "`{}` step {step} ({op}): the record must be the old one or the new one, got {v:?}",
                    scenario.name
                ));
            }
        };
        if v.a != before.a {
            return Err(format!(
                "`{}` step {step} ({op}): the file the write doesn't touch (a) changed: blake3 {} -> {}",
                scenario.name, before.a, v.a
            ));
        }
        if seen_new && outcome == "old" {
            return Err(format!(
                "`{}` step {step} ({op}): the old record came back after an earlier cut already kept the new one",
                scenario.name
            ));
        }
        seen_new |= outcome == "new";
        eprintln!(
            "[xtask] neo-test: `{}` cut after step {step}/{ops} ({op}): {outcome} record, a unchanged",
            scenario.name
        );
        table.push(format!("{step} {op} {outcome}"));
    }
    if !seen_new {
        return Err(format!(
            "`{}` no cut kept the new record, though the full write did",
            scenario.name
        ));
    }
    println!("SETTINGS_CUT/1 ops={ops} a_blake3={}", before.a);
    for row in table {
        println!("SETTINGS_CUT/1 step {row}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// F11 S2: the hand-laid seven-key record is `settings-core`'s
    /// `DEFAULTS7_GEN1` oracle (92 bytes, its checksum copied from
    /// `setup_keys_tests.rs`).
    #[test]
    fn record7_is_the_seven_key_oracle() {
        let r = record7(&settings_core::Settings::default(), 1);
        assert_eq!(r.len(), 92);
        assert_eq!(&r[..12], b"NEOSET1\0\x01\x00\x07\x00");
        let sum: String = r[76..].iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(sum, "1ca373c0c135b0d880ac427a1540a232");
    }

    #[test]
    fn sampled_steps_take_each_runs_ends_and_every_flush() {
        // A run crossing a block boundary (sectors 6..10: blocks 0 and 1),
        // a flush, a 1-sector run, a 3-sector run, a flush.
        let ops = [
            (1, false, 6),
            (2, false, 7),
            (3, false, 8),
            (4, false, 9),
            (5, false, 10),
            (6, true, 0),
            (7, false, 0),
            (8, false, 2048),
            (9, false, 2049),
            (10, false, 2050),
            (11, true, 0),
        ];
        assert_eq!(sampled_steps(&ops), vec![1, 2, 3, 5, 6, 7, 8, 10, 11]);
        let serial =
            "x\nLOGIN-CUT op step=1 op=write lba=100\nLOGIN-CUT op step=2 op=flush lba=0\n";
        assert_eq!(cut_ops(serial), vec![(1, false, 100), (2, true, 0)]);
    }
}
