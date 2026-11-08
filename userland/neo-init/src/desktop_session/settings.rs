// SPDX-License-Identifier: MPL-2.0
//! OT-88 S1 (`atrium-settings-store`): the settings store, with the WM as
//! its only writer (THINK-2026-10-29-ot88 §5.2-5.3).
//!
//! - **Load.** At boot, before the scene is built, the WM reads the two
//!   copies (`.system/settings.a` and `.b`, `settings-core`'s `NEOSET1`)
//!   and takes the newer valid one. With neither, it migrates today's
//!   presence files into a first record and writes it (the files stay).
//! - **Apply.** Reduce Transparency is the base scene's material
//!   (`glass::build`, and live through `BaseScene::set_mode`); Reduce Motion
//!   is the scene's motion profile; the idle-lock time is L2's idle timer.
//! - **`OP_SETTING {key, value}`.** Only the settings app asks (§5.3): the
//!   WM takes it from the client it launched as the settings app, and refuses
//!   any other client's (`CapRequired`, 7). It checks the key and value
//!   against `settings-core`'s table; a key that isn't client-writable (the
//!   idle lock: the PM's #222 bounce) is refused (23). Then it writes the file
//!   not holding the record, reads it back and checks it, and applies the
//!   setting live. Writes are rate-limited (24), and refused under the lock
//!   screen (`BadState`, 4). A refusal, or a write that
//!   didn't land, changes nothing; a write that landed but read back wrong is
//!   resolved by reading the store again, as the next boot will.
//! - **The WM's own keys.** [`set_wm`] is the WM-only path (S2's idle-lock
//!   view, the WM's own, will call it; the boot test calls it for the idle
//!   lock).

use super::{Line, Scene, login};
use core::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use libneo::{FileRights, OPEN_ROOT_TEMPORARY, sys_close, sys_open};
use settings_core::{self as sc, Current, File, Motion, Record, Settings};

/// The record in force, packed (see [`pack`]); its generation (0: none on
/// disk) and file (0: none, 1: a, 2: b).
static VALUES: AtomicU64 = AtomicU64::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static FILE: AtomicU8 = AtomicU8::new(0);
static LOADED: AtomicU8 = AtomicU8::new(0);
/// F11 S2 (`setup-store`): the volatile overlay (setup-menu THINK §2.2): a
/// session whose store can't be written (Try: no `/neofs/.system`, or a
/// read-only mount) starts from the defaults, never reads the installed
/// record, and keeps every write in memory.
static VOLATILE: AtomicU8 = AtomicU8::new(0);

/// Whether `load` found a valid record on disk (F11 S2: an installed
/// system's setup menu runs only until one exists, setup-menu THINK §2.4).
static FOUND: AtomicU8 = AtomicU8::new(0);

/// Whether the first-boot setup menu should run: on a Try session (the
/// volatile overlay: nothing persists, so every boot), or on an installed
/// system whose store held no valid record when this boot began.
pub fn setup_due() -> bool {
    volatile() || FOUND.load(Ordering::Acquire) == 0
}

/// Whether this session's store is the volatile overlay.
pub fn volatile() -> bool {
    VOLATILE.load(Ordering::Acquire) == 1
}

/// Until OT-87 R5's Try flag exists: the store is volatile when it can't be
/// written: no `.system/`, or the record file (or, with none, `settings.a`)
/// doesn't open for writing (how a read-only volume shows itself, as the
/// login gate tells it). The test build can force it.
#[cfg(feature = "setup-store")]
fn store_is_volatile() -> bool {
    if cfg!(feature = "setup-store-volatile-test") {
        return true;
    }
    match sys_open(sc::SYSTEM_DIR, OPEN_ROOT_TEMPORARY, FileRights::LOOKUP) {
        Ok(dir) => {
            let _ = sys_close(dir);
        }
        Err(_) => return true,
    }
    let a = login::read_small(sc::SETTINGS_A);
    let b = login::read_small(sc::SETTINGS_B);
    let path = sc::select(a.as_deref(), b.as_deref()).map_or(sc::SETTINGS_A, |c| c.file.path());
    let rw = FileRights::READ.union(FileRights::WRITE);
    let opened = match sys_open(path, OPEN_ROOT_TEMPORARY, rw) {
        Err(libneo::ENOENT) => sys_open(path, libneo::OPEN_CREATE_REGULAR, rw),
        other => other,
    };
    match opened {
        Ok(handle) => {
            let _ = sys_close(handle);
            false
        }
        Err(_) => true,
    }
}

fn pack(s: &Settings) -> u64 {
    let v = u64::from(s.get(sc::Key::ReduceTransparency))
        | u64::from(s.get(sc::Key::ReduceMotion)) << 8
        | u64::from(s.get(sc::Key::IdleLockMinutes)) << 16
        | u64::from(s.get(sc::Key::TimeZone)) << 32
        | u64::from(s.get(sc::Key::RtcLocal)) << 48;
    // F11 S2: the setup menu's keys in the top byte (each count is < 16).
    #[cfg(feature = "setup-store")]
    let v = v
        | u64::from(s.get(sc::Key::Language) & 0xf) << 56
        | u64::from(s.get(sc::Key::KeyboardLayout) & 0xf) << 60;
    v
}

fn unpack(v: u64) -> Settings {
    Settings::default()
        .with(1, (v & 0xff) as u32)
        .and_then(|s| s.with(2, (v >> 8 & 0xff) as u32))
        .and_then(|s| s.with(3, (v >> 16 & 0xffff) as u32))
        .and_then(|s| s.with(4, (v >> 32 & 0xffff) as u32))
        .and_then(|s| s.with(5, (v >> 48 & 0xff) as u32))
        .and_then(|s| unpack_setup(s, v))
        .unwrap_or_default()
}

#[cfg(feature = "setup-store")]
fn unpack_setup(s: Settings, v: u64) -> Result<Settings, sc::SetError> {
    s.with(6, (v >> 56 & 0xf) as u32)
        .and_then(|s| s.with(7, (v >> 60 & 0xf) as u32))
}

#[cfg(not(feature = "setup-store"))]
fn unpack_setup(s: Settings, _v: u64) -> Result<Settings, sc::SetError> {
    Ok(s)
}

/// The settings in force (the defaults until [`load`]).
pub fn current() -> Settings {
    if LOADED.load(Ordering::Acquire) == 0 {
        return Settings::default();
    }
    unpack(VALUES.load(Ordering::Acquire))
}

fn current_record() -> Option<Current> {
    let file = match FILE.load(Ordering::Acquire) {
        1 => File::A,
        2 => File::B,
        _ => return None,
    };
    Some(Current {
        file,
        record: Record {
            generation: GENERATION.load(Ordering::Acquire),
            settings: current(),
        },
    })
}

fn store(current: Option<Current>, settings: &Settings) {
    VALUES.store(pack(settings), Ordering::Release);
    let (generation, file) = match current {
        Some(c) => (c.record.generation, if c.file == File::A { 1 } else { 2 }),
        None => (0, 0),
    };
    GENERATION.store(generation, Ordering::Release);
    FILE.store(file, Ordering::Release);
    LOADED.store(1, Ordering::Release);
}

/// The settings app's module (§5.3; the app itself is S1's next slice). The
/// WM takes `OP_SETTING` only from a client it launched from this module.
pub const SETTINGS_APP_MODULE: &str = "settings";

/// Whether the client the WM launched from `module` may send `OP_SETTING`.
/// The boot tests' stand-in (`settings-standin-test`) is Arthur, until the
/// settings app exists; the control (`settings-authority-control`) lets
/// every client.
pub fn is_settings_app(module: &str) -> bool {
    module == SETTINGS_APP_MODULE
        || (cfg!(feature = "settings-standin-test") && module == "arthur")
        || cfg!(feature = "settings-authority-control")
}

/// The rate limit on client writes: a bucket of [`BURST`] writes, one back
/// every [`REFILL_NS`]. Only a request that would write costs one; a refusal
/// or a value already in force costs nothing. The boot test refills once a
/// minute, so its burst's count is exact.
const BURST: u64 = 4;
const REFILL_NS: u64 = if cfg!(any(
    feature = "settings-boot-test",
    feature = "settings-refused-test"
)) {
    60_000_000_000
} else {
    1_000_000_000
};
/// Writes left, and when the bucket last refilled (0: not yet started).
static TOKENS: AtomicU64 = AtomicU64::new(BURST);
static REFILLED_NS: AtomicU64 = AtomicU64::new(0);

fn now_ns() -> u64 {
    libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC).unwrap_or(0)
}

/// Take one write from the bucket; `false`: none left.
fn take_token() -> bool {
    let now = now_ns();
    let last = REFILLED_NS.load(Ordering::Acquire);
    if last == 0 {
        REFILLED_NS.store(now, Ordering::Release);
    } else {
        let back = now.saturating_sub(last) / REFILL_NS;
        if back > 0 {
            let tokens = TOKENS
                .load(Ordering::Acquire)
                .saturating_add(back)
                .min(BURST);
            TOKENS.store(tokens, Ordering::Release);
            REFILLED_NS.store(last + back * REFILL_NS, Ordering::Release);
        }
    }
    let tokens = TOKENS.load(Ordering::Acquire);
    if tokens == 0 {
        return false;
    }
    TOKENS.store(tokens - 1, Ordering::Release);
    true
}

fn present(path: &str) -> bool {
    match sys_open(path, OPEN_ROOT_TEMPORARY, FileRights::READ) {
        Ok(handle) => {
            let _ = sys_close(handle);
            true
        }
        Err(_) => false,
    }
}

fn file_name(file: File) -> &'static [u8] {
    match file {
        File::A => b"a",
        File::B => b"b",
    }
}

fn log_settings(line: Line, s: &Settings) -> Line {
    let line = line
        .s(b" rt=")
        .n(u64::from(s.get(sc::Key::ReduceTransparency)))
        .s(b" rm=")
        .n(u64::from(s.get(sc::Key::ReduceMotion)))
        .s(b" idle=")
        .n(u64::from(s.idle_lock_minutes));
    // F11 S2: the setup menu's keys.
    #[cfg(feature = "setup-store")]
    let line = line
        .s(b" lang=")
        .n(u64::from(s.language))
        .s(b" kb=")
        .n(u64::from(s.keyboard_layout));
    line
}

/// Read the store at boot; with no record, migrate the presence files and
/// write the first one.
pub fn load() {
    #[cfg(feature = "setup-store")]
    if store_is_volatile() {
        VOLATILE.store(1, Ordering::Release);
        store(None, &Settings::default());
        log_settings(
            Line::new().s(b"SETTINGS volatile writes=0"),
            &Settings::default(),
        )
        .s(b"\n")
        .emit();
        return;
    }
    let a = login::read_small(sc::SETTINGS_A);
    let b = login::read_small(sc::SETTINGS_B);
    if let Some(c) = sc::select(a.as_deref(), b.as_deref()) {
        FOUND.store(1, Ordering::Release);
        store(Some(c), &c.record.settings);
        log_settings(
            Line::new()
                .s(b"SETTINGS loaded generation=")
                .n(c.record.generation)
                .s(b" file=")
                .s(file_name(c.file)),
            &c.record.settings,
        )
        .s(b"\n")
        .emit();
        return;
    }
    let migrated = sc::migrate(present(sc::RT_PRESENCE), present(sc::RM_PRESENCE));
    store(None, &migrated);
    log_settings(Line::new().s(b"SETTINGS migrated"), &migrated)
        .s(b"\n")
        .emit();
    // A first record that can't be written (no `.system/` yet) is written
    // by the first `OP_SETTING` instead; until then the migrated values hold.
    let _ = commit(&migrated);
}

/// Write `next` to the file not holding the record, read it back, and make
/// it the record. `false`: nothing changed (the old record stays in force).
fn commit(next: &Settings) -> bool {
    // The volatile overlay: in memory only, nothing touches the disk.
    if volatile() {
        store(None, next);
        log_settings(Line::new().s(b"SETTINGS volatile writes=0"), next)
            .s(b"\n")
            .emit();
        return true;
    }
    let current = current_record();
    let Some((file, bytes)) = sc::plan(current.as_ref(), next) else {
        return false;
    };
    let written = login::write_whole(file.path(), &bytes);
    let landed = written && login::read_small(file.path()).is_some_and(|back| back == bytes);
    if !landed {
        Line::new()
            .s(b"SETTINGS write=failed file=")
            .s(file_name(file))
            .s(b"\n")
            .emit();
        // The write may have landed though its read-back failed: take what
        // the next boot would read, so this boot and the next agree.
        return written && resync(next);
    }
    let generation = current.map_or(1, |c| c.record.generation + 1);
    let written = Current {
        file,
        record: Record {
            generation,
            settings: *next,
        },
    };
    store(Some(written), next);
    let digest = login_core::digest(&bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hex = [0u8; 32];
    for (i, byte) in digest[..16].iter().enumerate() {
        hex[2 * i] = HEX[usize::from(byte >> 4)];
        hex[2 * i + 1] = HEX[usize::from(byte & 15)];
    }
    Line::new()
        .s(b"SETTINGS written generation=")
        .n(generation)
        .s(b" file=")
        .s(file_name(file))
        .s(b" digest=")
        .s(&hex)
        .s(b"\n")
        .emit();
    true
}

/// After a write whose read-back failed: read both copies again and put
/// the record they select in force. `true`: that record is `next`.
fn resync(next: &Settings) -> bool {
    let a = login::read_small(sc::SETTINGS_A);
    let b = login::read_small(sc::SETTINGS_B);
    let Some(c) = sc::select(a.as_deref(), b.as_deref()) else {
        return false;
    };
    store(Some(c), &c.record.settings);
    Line::new()
        .s(b"SETTINGS resync generation=")
        .n(c.record.generation)
        .s(b" file=")
        .s(file_name(c.file))
        .s(b"\n")
        .emit();
    c.record.settings == *next
}

/// The idle-lock time in force, in nanoseconds (0: off). The boot test
/// counts a minute as four seconds (the 5 the WM sets is 20 s, which
/// outlasts Arthur's launch and its requests under TCG, none of which may
/// land under the lock; the default is 40 s).
pub fn idle_ns() -> u64 {
    let minutes = u64::from(current().idle_lock_minutes);
    if cfg!(any(
        feature = "settings-boot-test",
        feature = "settings-refused-test"
    )) {
        minutes * 4_000_000_000
    } else {
        minutes * 60_000_000_000
    }
}

/// A client's `OP_SETTING {key, value}`: the reply's status (0: set and
/// applied). `settings_app`: the client is the one the WM launched as the
/// settings app ([`is_settings_app`]).
pub fn request(scene: &mut Scene, settings_app: bool, key: u16, value: u32) -> u32 {
    use libneo::atrium_protocol::{
        ProtocolError, STATUS_SETTING_BUSY, STATUS_SETTING_KEY, STATUS_SETTING_WM_ONLY,
    };
    let status = if !settings_app {
        ProtocolError::CapRequired.status()
    } else if super::lock::locked() {
        // Nothing changes under the lock screen (the PM's #222 note).
        ProtocolError::BadState.status()
    } else {
        match sc::Key::from_id(key) {
            None => STATUS_SETTING_KEY,
            Some(k) if !k.client_writable() && !cfg!(feature = "settings-wm-only-control") => {
                STATUS_SETTING_WM_ONLY
            }
            Some(_) => match current().with(key, value) {
                Ok(next) if next != current() && !take_token() => STATUS_SETTING_BUSY,
                _ => return set(scene, key, value),
            },
        }
    };
    log_setting(key, value, status);
    status
}

/// Set `key` to `value`, the WM's own path (any key, no rate limit): the
/// status as [`request`]'s. A value already in force writes nothing.
pub fn set_wm(scene: &mut Scene, key: u16, value: u32) -> u32 {
    set(scene, key, value)
}

fn set(scene: &mut Scene, key: u16, value: u32) -> u32 {
    use libneo::atrium_protocol::{STATUS_SETTING_KEY, STATUS_SETTING_STORE, STATUS_SETTING_VALUE};
    let before = current();
    let status = match before.with(key, value) {
        Err(sc::SetError::Key) => STATUS_SETTING_KEY,
        Err(sc::SetError::Value) => STATUS_SETTING_VALUE,
        Ok(next) if next == before => 0,
        Ok(next) => {
            if commit(&next) {
                apply(scene, &before, &current());
                0
            } else {
                // A failed write may still have changed the record in force
                // (`resync`): apply what is in force now.
                apply(scene, &before, &current());
                STATUS_SETTING_STORE
            }
        }
    };
    log_setting(key, value, status);
    status
}

fn log_setting(key: u16, value: u32, status: u32) {
    Line::new()
        .s(if status == 0 {
            b"SETTING applied key="
        } else {
            b"SETTING refused key="
        })
        .n(u64::from(key))
        .s(b" value=")
        .n(u64::from(value))
        .s(b" status=")
        .n(u64::from(status))
        .s(b"\n")
        .emit();
}

/// Apply what changed, live.
fn apply(scene: &mut Scene, before: &Settings, next: &Settings) {
    #[cfg(feature = "desktop-glass")]
    if before.reduce_transparency != next.reduce_transparency {
        let mode = if next.reduce_transparency {
            neo_atrium::Mode::ReduceTransparency
        } else {
            neo_atrium::Mode::Glass
        };
        if scene.chrome.set_mode(mode) {
            let screen = scene.geo.screen;
            scene.repaint_chrome(screen);
            let client = {
                #[cfg(feature = "desktop-home")]
                {
                    scene.home.is_none()
                }
                #[cfg(not(feature = "desktop-home"))]
                {
                    true
                }
            };
            let _ = scene.present(client, screen);
            Line::new()
                .s(b"DESKTOP material=")
                .s(if next.reduce_transparency {
                    b"reduce-transparency"
                } else {
                    b"glass"
                })
                .s(b" cause=settings live=1\n")
                .emit();
        }
    }
    #[cfg(feature = "desktop-motion")]
    if before.reduce_motion != next.reduce_motion {
        scene.motion_profile = profile(next.reduce_motion);
    }
    let _ = (scene, before, next);
}

/// Reduce Motion's setting as the motion engine's profile.
#[cfg(feature = "desktop-motion")]
pub fn profile(m: Motion) -> neo_atrium::motion::Profile {
    use neo_atrium::motion::Profile;
    match m {
        Motion::Full => Profile::Full,
        Motion::Reduced => Profile::Reduced,
        Motion::Off => Profile::Off,
    }
}

#[cfg(not(feature = "desktop-motion"))]
#[allow(dead_code)]
fn profile(_m: Motion) {}

/// OT-88 S1's cut scenario driver (`settings-cut-test`,
/// `login_settings_cut`), on OT-66 L1's cut (`SYS_TEST_BLOCK_CUT`). Step 0
/// (no cut) verifies: which record is in force, and both files' BLAKE3. A
/// step arms the cut, then sets the idle lock to 30: the write the cut ends
/// goes to the file not holding the record, so the next boot must read the
/// old record or the new one, and the other file byte for byte as before.
#[cfg(feature = "settings-cut-test")]
pub mod cut {
    use super::*;

    fn hex(mut line: Line, digest: &[u8; 32]) -> Line {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for &b in digest {
            line = line.s(&[HEX[usize::from(b >> 4)], HEX[usize::from(b & 15)]]);
        }
        line
    }

    pub fn run() -> Result<(), i32> {
        let step = libneo::sys_test_block_cut(0);
        let Some(current) = current_record() else {
            return Err(193);
        };
        if step == 0 {
            let a = login::read_small(sc::SETTINGS_A);
            let b = login::read_small(sc::SETTINGS_B);
            let line = Line::new()
                .s(b"SETTINGS cut verify record=")
                .s(file_name(current.file))
                .s(b" generation=")
                .n(current.record.generation)
                .s(b" idle=")
                .n(u64::from(current.record.settings.idle_lock_minutes))
                .s(b" a_blake3=");
            let line = hex(line, &login_core::digest(a.as_deref().unwrap_or(&[]))).s(b" b_blake3=");
            hex(line, &login_core::digest(b.as_deref().unwrap_or(&[])))
                .s(b"\n")
                .emit();
            return Err(0);
        }
        let Ok(next) = current
            .record
            .settings
            .with(sc::Key::IdleLockMinutes.id(), 30)
        else {
            return Err(194);
        };
        Line::new()
            .s(b"SETTINGS cut change step=")
            .n(step as u64)
            .s(b" record=")
            .s(file_name(current.file))
            .s(b"\n")
            .emit();
        if libneo::sys_test_block_cut(1) != step {
            return Err(195);
        }
        let _ = commit(&next);
        let ops = libneo::sys_test_block_cut(2);
        Line::new()
            .s(b"SETTINGS cut not-reached step=")
            .n(step as u64)
            .s(b" ops=")
            .n(ops.max(0) as u64)
            .s(b"\n")
            .emit();
        Err(0)
    }
}
