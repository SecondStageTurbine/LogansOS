// SPDX-License-Identifier: MPL-2.0
//! OT-66 L2 (`login`): lock and unlock (THINK-2026-10-27-ot66 §5.2).
//!
//! A signed-in session locks on Super+L (the kernel's `sak-lock` chord, so
//! no client can read or intercept it) or when no key or pointer event has
//! been produced for [`IDLE_NS`] (the latch's activity bit, set by the kernel
//! for every event, whoever's route it went to: no client can feign or hide
//! input). Locked is an exclusive state:
//!
//! - **Input.** Locking runs [`HANDOFF`]: the WM's own focus becomes the
//!   route (one kernel swap; every client's receive then fails the kernel's
//!   id check), its queue is discarded, then the lock screen is presented.
//!   While locked, [`defer_route`] keeps every other route change the WM's
//!   code asks for (a launch's handoff, a hold, a refocus) as the one to
//!   restore; none is made. A pointer grab in progress is consumed, so the
//!   rest of the press reaches no client.
//! - **Pixels.** Only the lock's own frames reach the screen: `compose` and
//!   `commit_present` do nothing else while locked. Everything under the lock
//!   goes on as usual: clients' commits are accepted into the WM's copy, an
//!   `OP_BUSY` starts the emblem, a client spawned meanwhile completes its
//!   handshake and commits; none of it is composited until unlock.
//! - **Unlock.** The account's record, re-read, verifies the password (the
//!   same KDF as sign-in, with the same backoff). Then [`HANDOFF`] again: the
//!   route goes to the owner restored, the WM's queue (whatever was typed
//!   while locked) is discarded, and the whole current scene is presented.
//!
//! The lock screen is Home's view with a card and a password field, painted
//! into the lock's own plane, never into the scene's base.
//!
//! **L2b, the kernel's lock route** (kernel `sak-lock-route`). At arm the WM
//! registers its own focus as the kernel's lock focus. Super+L then locks the
//! route in the kernel's latch itself, before the WM sees the chord: every
//! client's receive fails at once and every event goes to the WM's focus,
//! even while the WM is parked in a send to a client that never receives
//! (CP-57). The idle lock asks the kernel to lock, and unlock is the kernel's
//! release, which makes the restored owner the route as it unlocks. Without
//! the kernel's half (an older kernel's take ignores the op) the WM's own
//! swaps above do it all, as in L2.

use super::home::{self, HomeFocus, HomeState, MAX_INTENT};
use super::{Line, Plane, Rect, Scene, login};
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use libneo::atrium_handoff::{HANDOFF, Steps, hand_over};
use libneo::{
    CLOCK_MONOTONIC, EAGAIN, Handle, INPUT_EVENT_V1_LEN, SAK_ACTIVITY, SAK_ESCAPE, SAK_LOCK,
    SAK_SPACE, SAK_TAB, sys_clock_gettime, sys_input_focus_activate, sys_input_focus_recv,
};
use login_core::{self as lc, Zeroizing};
use neo_atrium::keys::{self, Key};

/// The idle time that locks: no key or pointer event for this long. 10
/// minutes (§5.2); the settings app sets it (L4).
#[cfg(not(any(feature = "lock-idle-test", feature = "arthur-actions-test")))]
pub const IDLE_NS: u64 = 600_000_000_000;
/// Boot test (`login_arthur_permission`): 30 seconds, so a stalled
/// permission request (#248 bounce F1) must not keep the idle lock off.
#[cfg(all(feature = "arthur-actions-test", not(feature = "lock-idle-test")))]
pub const IDLE_NS: u64 = 30_000_000_000;
/// Boot test (`lock_idle`): 6 seconds.
#[cfg(feature = "lock-idle-test")]
pub const IDLE_NS: u64 = 6_000_000_000;

/// A signed-in session: the gate armed the lock. Try and a volume without
/// an account never lock (there is nothing to unlock with).
static ARMED: AtomicBool = AtomicBool::new(false);
static LOCKED: AtomicBool = AtomicBool::new(false);
/// Super+L was latched (taken by `pointer::take_sak`, wherever it ran).
static REQUESTED: AtomicBool = AtomicBool::new(false);
/// When the last key or pointer event was seen (monotonic ns).
static LAST_INPUT_NS: AtomicU64 = AtomicU64::new(0);
/// The keyboard route's owner: the activator the WM last made the route, or,
/// while locked, the one it asked for since (restored on unlock).
static ROUTE: AtomicU32 = AtomicU32::new(0);
/// The WM's own focus (its read side and activator), stored at arm, so a wait
/// that can't reach the scene can still lock the input ([`guard_input`]).
static FOCUS_EVENTS: AtomicU32 = AtomicU32::new(0);
static FOCUS_ACTIVATOR: AtomicU32 = AtomicU32::new(0);
/// The input is locked and the lock screen not painted yet (it was locked
/// from a wait that can't reach the scene).
static PAINT_PENDING: AtomicBool = AtomicBool::new(false);
/// The kernel's `sak-lock` activity bit has been seen in a latch take.
static SEEN_ACTIVITY: AtomicBool = AtomicBool::new(false);
/// L2b: the kernel holds the lock route (the WM's focus is registered).
static KERNEL_ROUTE: AtomicBool = AtomicBool::new(false);

/// `SYS_INPUT_SAK_TAKE`'s lock ops (kernel `sak-lock-route`).
const OP_REGISTER: u64 = 1;
const OP_ENGAGE: u64 = 2;
const OP_RELEASE: u64 = 3;

fn now_ns() -> u64 {
    sys_clock_gettime(CLOCK_MONOTONIC).unwrap_or(0)
}

/// The lock screen's state, in the scene.
#[derive(Default)]
pub struct Screen {
    /// The lock's own plane (output-sized), made at the first lock.
    plane: Option<Plane>,
    line: keys::Line<MAX_INTENT>,
    failures: u32,
    /// Keys are dropped until then (the backoff).
    wait_until: u64,
    note: Option<&'static str>,
    /// F9: the note once the backoff ends: [`WRONG`] after a wrong try
    /// here, none for a delay carried from earlier tries (Dexter's
    /// login-followups re-check, item 3).
    after_wait: Option<&'static str>,
    /// The lock's own frame is being composed and presented.
    pub(super) painting: bool,
    /// Unlocks so far (the boot test ends after its last one).
    #[cfg(feature = "lock-boot-test")]
    unlocks: u32,
}

/// The session is signed in, with the WM's own focus as the route: the lock
/// may engage from now on, and the idle time starts now.
pub fn arm(focus: &HomeFocus) {
    // Without the kernel's latch take (no `atrium-pointer-forward`, or no
    // input-system authority) the WM can see neither Super+L nor input: no
    // lock, rather than an idle lock while someone types.
    let probe = libneo::sys_input_sak_take();
    if probe < 0 {
        Line::new()
            .s(b"LOCK unavailable reason=no-sak-latch errno=")
            .n(probe.unsigned_abs())
            .s(b"\n")
            .emit();
        return;
    }
    note_sak(probe as u8);
    // Nor without the kernel's `sak-lock`: no Super+L, and no activity bit,
    // so the idle timer would fire while someone types. Signing in took keys,
    // so with `sak-lock` the bit has been seen by now (in this take, or in a
    // take while the sign-in screen ran).
    if !SEEN_ACTIVITY.load(Ordering::Acquire) {
        Line::new()
            .s(b"LOCK unavailable reason=no-activity-bit\n")
            .emit();
        return;
    }
    FOCUS_EVENTS.store(focus.events.0, Ordering::Release);
    FOCUS_ACTIVATOR.store(focus.activator.0, Ordering::Release);
    ROUTE.store(focus.activator.0, Ordering::Release);
    // L2b: the kernel's lock route. A kernel without it took the latch
    // instead (the op ignored): keep the bits it returned.
    let r = libneo::sys_input_sak_op(OP_REGISTER, focus.activator);
    let kernel = r == libneo::SAK_LOCK_OK;
    if !kernel && r >= 0 {
        note_sak(r as u8);
    }
    KERNEL_ROUTE.store(kernel, Ordering::Release);
    Line::new()
        .s(if kernel {
            b"LOCK armed route=kernel\n"
        } else {
            b"LOCK armed route=wm\n"
        })
        .emit();
    LAST_INPUT_NS.store(now_ns(), Ordering::Release);
    ARMED.store(true, Ordering::Release);
}

pub fn locked() -> bool {
    LOCKED.load(Ordering::Acquire)
}

/// `pointer::take_sak`'s bits: Super+L asks to lock; any chord or produced
/// event restarts the idle time.
pub fn note_sak(bits: u8) {
    if bits & SAK_ACTIVITY != 0 {
        SEEN_ACTIVITY.store(true, Ordering::Release);
    }
    if bits & SAK_LOCK != 0 {
        REQUESTED.store(true, Ordering::Release);
    }
    if bits & (SAK_ACTIVITY | SAK_LOCK | SAK_TAB | SAK_SPACE | SAK_ESCAPE) != 0 {
        LAST_INPUT_NS.store(now_ns(), Ordering::Release);
    }
}

/// L1a.3: an unlock gave the route back to a client (not Home).
#[cfg(feature = "atrium-terminal")]
static RESUMED: AtomicBool = AtomicBool::new(false);

/// L1a.3: whether an unlock gave the route back to a client since the last
/// call. A client's focus receive fails while the session is locked (the
/// kernel's lock route), so a client in front parks for its next turn's `K`;
/// unlock restores the route to it but, unlike a switch, sends no `K`. The
/// client table runs the turn's discard step for the client in front.
#[cfg(feature = "atrium-terminal")]
pub fn take_resumed() -> bool {
    RESUMED.swap(false, Ordering::AcqRel)
}

/// `home::swap_to` succeeded: `activator` is the route.
pub fn note_route(activator: Handle) {
    ROUTE.store(activator.0, Ordering::Release);
}

/// `home::swap_to`, while locked: keep `activator` as the owner to restore,
/// and make no route change (`true`). Unlocked: `false`, swap as usual.
pub fn defer_route(activator: Handle) -> bool {
    if !locked() {
        return false;
    }
    ROUTE.store(activator.0, Ordering::Release);
    #[cfg(feature = "lock-boot-test")]
    Line::new()
        .s(b"LOCK route deferred to=")
        .n(u64::from(activator.0))
        .s(b"\n")
        .emit();
    true
}

fn wm_focus() -> HomeFocus {
    HomeFocus {
        events: Handle(FOCUS_EVENTS.load(Ordering::Acquire)),
        activator: Handle(FOCUS_ACTIVATOR.load(Ordering::Acquire)),
    }
}

/// Whether the lock is asked for now (Super+L, or the idle time), and why.
#[cfg(not(feature = "atrium-settings-store"))]
fn due() -> Option<(&'static [u8], u64)> {
    let idle_ns = now_ns().saturating_sub(LAST_INPUT_NS.load(Ordering::Acquire));
    if REQUESTED.swap(false, Ordering::AcqRel) {
        Some((b"sak", idle_ns))
    } else if idle_ns >= IDLE_NS {
        Some((b"idle", idle_ns))
    } else {
        None
    }
}

/// As above, with OT-88 S1's idle time from the settings store (0: the idle
/// lock is off).
#[cfg(feature = "atrium-settings-store")]
fn due() -> Option<(&'static [u8], u64)> {
    let idle_ns = now_ns().saturating_sub(LAST_INPUT_NS.load(Ordering::Acquire));
    let idle = super::settings::idle_ns();
    if REQUESTED.swap(false, Ordering::AcqRel) {
        Some((b"sak", idle_ns))
    } else if idle != 0 && idle_ns >= idle {
        Some((b"idle", idle_ns))
    } else {
        None
    }
}

/// A3p: the latest a parked WM loop may sleep (absolute monotonic ns) and
/// still serve the lock on time. Unarmed, or with the idle lock off: no
/// bound. Armed with Super+L latched but not yet served, or an unlock's
/// resume owed to the front client: now (each is served on the next turn,
/// and nothing else would wake the park). Armed and unlocked: when the idle time runs out (input since
/// then only wakes it early, which is harmless). Locked: one tick, so the
/// lock screen reads its keys and ends its backoff without a wait item of
/// its own (the lock is rare; the cost is the pre-A3p poll's, while it
/// shows).
#[cfg(feature = "desktop-park")]
pub fn park_deadline_ns() -> u64 {
    const TICK_NS: u64 = 10_000_000;
    if !ARMED.load(Ordering::Acquire) {
        return u64::MAX;
    }
    if REQUESTED.load(Ordering::Acquire) {
        return 0;
    }
    // L1a.3: an unlock that gave the route back to a client is served on the
    // client table's next turn ([`take_resumed`]): nothing else wakes it.
    #[cfg(feature = "atrium-terminal")]
    if RESUMED.load(Ordering::Acquire) {
        return 0;
    }
    if locked() || PAINT_PENDING.load(Ordering::Acquire) {
        return now_ns().saturating_add(TICK_NS);
    }
    let idle = idle_limit_ns();
    if idle == 0 {
        return u64::MAX;
    }
    LAST_INPUT_NS.load(Ordering::Acquire).saturating_add(idle)
}

#[cfg(all(feature = "desktop-park", not(feature = "atrium-settings-store")))]
fn idle_limit_ns() -> u64 {
    IDLE_NS
}

#[cfg(all(feature = "desktop-park", feature = "atrium-settings-store"))]
fn idle_limit_ns() -> u64 {
    super::settings::idle_ns()
}

/// Called from every WM loop and wait that can reach the scene (the session
/// loop's every turn, the window view's tick, Home's and the Lens view's
/// loops, `poll_control`): engage the lock when it is asked for or the idle
/// time has passed, paint it if a wait locked it without painting, and while
/// locked, serve the lock screen. `Ok(true)`: locked, the caller reads no
/// input this turn.
pub fn service(scene: &mut Scene) -> Result<bool, i32> {
    if !ARMED.load(Ordering::Acquire) {
        return Ok(false);
    }
    if !locked() {
        // With no pointer route nothing else takes the latch.
        #[cfg(feature = "desktop-pointer")]
        if scene.pointer.is_none() {
            let _ = super::pointer::take_sak(false);
        }
        let Some((reason, idle_ns)) = due() else {
            return Ok(false);
        };
        lock_input(reason, idle_ns)?;
    }
    if PAINT_PENDING.swap(false, Ordering::AcqRel) {
        show(scene)?;
    }
    tick(scene)?;
    Ok(locked())
}

/// From a wait that can't reach the scene (inside a `HANDOFF` closure, or a
/// control byte from a client): lock the input now when the lock is due; the
/// screen keeps its last frame (nothing new is composited) and the lock
/// screen is painted by the next [`service`].
pub fn guard_input() {
    if !ARMED.load(Ordering::Acquire) || locked() {
        return;
    }
    #[cfg(feature = "desktop-pointer")]
    let _ = super::pointer::take_sak(false);
    if let Some((reason, idle_ns)) = due() {
        // A failed swap here is reported by the next `service` (the route
        // check there): this wait has no error path to the WM's loop.
        let _ = lock_input(reason, idle_ns);
    }
}

/// The input half of locking, `HANDOFF`'s swap and discard: the WM's own
/// focus becomes the route, its queue is discarded, and from here every
/// `swap_to` defers and nothing but the lock is composited. The present
/// (the lock screen) is [`show`], now or at the next [`service`].
fn lock_input(reason: &[u8], idle_ns: u64) -> Result<(), i32> {
    LOCKED.store(true, Ordering::Release);
    PAINT_PENDING.store(true, Ordering::Release);
    // L2b: Super+L locked the kernel's route already; the idle lock asks now.
    if KERNEL_ROUTE.load(Ordering::Acquire)
        && libneo::sys_input_sak_op(OP_ENGAGE, Handle(0)) != libneo::SAK_LOCK_OK
    {
        return Err(194);
    }
    let focus = wm_focus();
    if sys_input_focus_activate(focus.activator) != 0 {
        return Err(190);
    }
    home::discard_queued(&focus, b"lock");
    Line::new()
        .s(b"LOCK locked reason=")
        .s(reason)
        .s(b" idle_ms=")
        .n(idle_ns / 1_000_000)
        .s(b"\n")
        .emit();
    Ok(())
}

/// The present half: a pointer grab in progress is consumed, and the lock
/// screen is painted whole.
fn show(scene: &mut Scene) -> Result<(), i32> {
    #[cfg(feature = "desktop-pointer")]
    if let Some(pointer) = scene.pointer.as_mut() {
        pointer.consume_grab();
    }
    let s = &mut scene.lock;
    s.line.wipe();
    // The persisted count (Dexter #186 item 2), not a fresh 0 per lock.
    s.failures =
        login::current().map_or(0, |c| login::Attempts::load(c.slot.account_id).failures());
    // Dexter #187 3b: a saved count's delay applies from the lock's first
    // try, as sign-in's does (`wait_out` before the first check).
    let delay = lc::delay_secs(s.failures);
    s.wait_until = if delay == 0 {
        0
    } else {
        now_ns().saturating_add(u64::from(delay) * 1_000_000_000)
    };
    s.note = (delay != 0).then_some(WAIT);
    s.after_wait = None;
    if delay != 0 {
        Line::new()
            .s(b"LOCK backoff wait_s=")
            .n(u64::from(delay))
            .s(b" at=show\n")
            .emit();
    }
    let screen = scene.geo.screen;
    paint(scene, screen)?;
    // With a delay pending, `tick` asks for the password once it's over.
    if delay == 0 {
        log_prompt(b"password");
    }
    Ok(())
}

/// One turn of the lock screen: drain the pointer and the latch (the
/// pointer moves nothing, a chord does nothing but restart the idle time),
/// then the keys the WM's focus holds.
fn tick(scene: &mut Scene) -> Result<(), i32> {
    let focus = &wm_focus();
    #[cfg(feature = "desktop-pointer")]
    scene.pointer_idle();
    if scene.lock.wait_until != 0 && now_ns() >= scene.lock.wait_until {
        scene.lock.wait_until = 0;
        scene.lock.note = scene.lock.after_wait.take();
        paint(scene, card_damage(scene))?;
        log_prompt(b"password");
    }
    loop {
        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        match sys_input_focus_recv(focus.events, &mut event) {
            0 => {}
            EAGAIN => return Ok(()),
            _ => return Err(191),
        }
        // The backoff: keys typed meanwhile are dropped.
        if scene.lock.wait_until != 0 {
            continue;
        }
        let Some(key) = keys::decode(event) else {
            continue;
        };
        match key {
            Key::Enter if !scene.lock.line.is_empty() => {
                if attempt(scene, focus)? {
                    return Ok(());
                }
            }
            _ => {
                if scene.lock.line.edit(key) {
                    let field = super::wm_rect(scene.chrome.layout.field);
                    paint(scene, field)?;
                }
            }
        }
    }
}

/// F9 (J.c., 2026-11-02): his words. It says nothing about which part was
/// wrong: there is only the password here.
const WRONG: &str = login::WRONG_PASSWORD;
const WAIT: &str = "Too many tries. Wait a moment.";

/// Check the field's password against the account's record. `Ok(true)`:
/// unlocked.
fn attempt(scene: &mut Scene, focus: &HomeFocus) -> Result<bool, i32> {
    let password = Zeroizing::new(scene.lock.line.as_bytes().to_vec());
    scene.lock.line.wipe();
    scene.lock.note = Some("Checking...");
    paint(scene, card_damage(scene))?;
    // Re-read: a password changed since sign-in is the one asked for. No
    // valid record any more stays locked (fail closed).
    let Some(current) = login::current() else {
        return Ok(false);
    };
    // Dexter #186 item 2: the lock's tries count with sign-in's, persisted,
    // and before the check (fail closed if the count can't be written).
    let mut attempts = login::Attempts::load(current.slot.account_id);
    if !attempts.begin() {
        return Ok(false);
    }
    if login::check_password(&current.slot, &password) {
        attempts.reset(b"unlock");
        unlock(scene, focus)?;
        return Ok(true);
    }
    let s = &mut scene.lock;
    s.failures = attempts.failures();
    let delay = lc::delay_secs(s.failures);
    Line::new()
        .s(b"LOCK password refused failures=")
        .n(u64::from(s.failures))
        .s(b" next_delay_s=")
        .n(u64::from(delay))
        .s(b"\n")
        .emit();
    if delay == 0 {
        s.note = Some(WRONG);
        paint(scene, card_damage(scene))?;
        #[cfg(feature = "desktop-session-boot-test")]
        Line::new()
            .s(b"LOCK note=\"")
            .s(WRONG.as_bytes())
            .s(b"\" row=last\n")
            .emit();
        log_prompt(b"password");
    } else {
        s.wait_until = now_ns().saturating_add(u64::from(delay) * 1_000_000_000);
        s.note = Some(WAIT);
        s.after_wait = Some(WRONG);
        paint(scene, card_damage(scene))?;
        Line::new()
            .s(b"LOCK backoff wait_s=")
            .n(u64::from(delay))
            .s(b"\n")
            .emit();
    }
    Ok(false)
}

fn unlock(scene: &mut Scene, focus: &HomeFocus) -> Result<(), i32> {
    let failures = scene.lock.failures;
    LOCKED.store(false, Ordering::Release);
    REQUESTED.store(false, Ordering::Release);
    LAST_INPUT_NS.store(now_ns(), Ordering::Release);
    let target = Handle(ROUTE.load(Ordering::Acquire));
    let mut to_home = target.0 == focus.activator.0;
    hand_over(
        &mut Steps {
            swap: || {
                // The owner restored may be gone (its session ended while
                // locked and closed its activator): the WM keeps the keyboard.
                // L2b: the kernel's release makes it the route as it unlocks.
                let swap = |to: Handle| {
                    if KERNEL_ROUTE.load(Ordering::Acquire) {
                        libneo::sys_input_sak_op(OP_RELEASE, to) == libneo::SAK_LOCK_OK
                    } else {
                        sys_input_focus_activate(to) == 0
                    }
                };
                if !to_home && !swap(target) {
                    to_home = true;
                }
                if to_home && !swap(focus.activator) {
                    return Err(192);
                }
                ROUTE.store(
                    if to_home { focus.activator.0 } else { target.0 },
                    Ordering::Release,
                );
                Ok(())
            },
            // Whatever was typed while locked reaches no one.
            discard: || {
                home::discard_queued(focus, b"unlock");
                Ok(())
            },
            // The whole scene as it is now (a client's commits, its busy
            // emblem, a view the WM switched to underneath).
            present: || {
                let client = scene.home.is_none();
                scene
                    .present(client, scene.geo.screen)
                    .map(|_| ())
                    .map_err(|c| 100 + c)
            },
        },
        &HANDOFF,
    )?;
    // L1a.3: the route went back to a client, which parked for its turn's
    // `K` when its receive started failing at the lock. The client table runs
    // that client's discard step on its next turn ([`take_resumed`]).
    #[cfg(feature = "atrium-terminal")]
    RESUMED.store(!to_home, Ordering::Release);
    Line::new()
        .s(b"LOCK unlocked failures=")
        .n(u64::from(failures))
        .s(if to_home {
            b" to=home\n"
        } else {
            b" to=client\n"
        })
        .emit();
    #[cfg(feature = "lock-boot-test")]
    {
        scene.lock.unlocks += 1;
        if scene.lock.unlocks >= test::UNLOCKS {
            Line::new().s(b"LOCK boot test done\n").emit();
            // L1.3a: the session's Arthur is the product's, which never exits
            // by itself: end it, so the boot can.
            #[cfg(feature = "arthur-actions-test")]
            {
                super::action_log::report();
                super::permission::end_client();
            }
            #[cfg(feature = "lock-terminal-test")]
            end_arthur();
            libneo::sys_exit(0);
        }
    }
    Ok(())
}

/// `login_terminal_lock`: the session's Arthur is the product's (it opens the
/// terminal on `open terminal`), which never exits by itself, so the boot
/// test ends it, as `arthur-actions-test` does.
#[cfg(feature = "lock-terminal-test")]
static ARTHUR: AtomicU32 = AtomicU32::new(u32::MAX);

#[cfg(feature = "lock-terminal-test")]
pub fn note_arthur(process: libneo::ProcessHandle) {
    ARTHUR.store(process.0, Ordering::Release);
}

#[cfg(feature = "lock-terminal-test")]
fn end_arthur() {
    let handle = ARTHUR.load(Ordering::Acquire);
    if handle != u32::MAX {
        let process = libneo::ProcessHandle(handle);
        let _ = libneo::sys_kill(process);
        let _ = libneo::sys_wait(process, 0);
    }
}

fn card_damage(scene: &Scene) -> Rect {
    let layout = &scene.chrome.layout;
    super::wm_rect(layout.card(neo_atrium::layout::CARD_MAX_ROWS))
        .union(&super::wm_rect(layout.field_glow))
}

/// Paint `damage` of the lock screen into the lock's plane and present it.
fn paint(scene: &mut Scene, damage: Rect) -> Result<(), i32> {
    let mut rows: Vec<String> = alloc::vec![String::from("Locked"), login::name()];
    rows.push(String::from("Enter your password to unlock."));
    // F9: the note is the last row, the one nearest the field.
    rows.extend(scene.lock.note.map(String::from));
    let mut plane = scene.lock.plane.take();
    let line = core::mem::take(&mut scene.lock.line);
    let (shown, line) = paint_view(scene, &mut plane, rows, line, true, damage);
    scene.lock.plane = plane;
    scene.lock.line = line;
    shown
}

/// A WM-owned view (the lock screen, L1.3's permission prompt): paint
/// `damage` of a full-screen card of `rows` (the field showing `line`) into
/// `plane` (created on first use) and present that plane alone. The scene's
/// base, Home state and card are borrowed for the paint and put back as they
/// were; `line` comes back.
pub(super) fn paint_view(
    scene: &mut Scene,
    plane: &mut Option<Plane>,
    rows: Vec<String>,
    line: keys::Line<MAX_INTENT>,
    masked: bool,
    damage: Rect,
) -> (Result<(), i32>, keys::Line<MAX_INTENT>) {
    let (w, h) = (scene.geo.screen.w, scene.geo.screen.h);
    let view = match plane.take() {
        Some(view) => view,
        None => match Plane::create(w, h) {
            Ok((_, view)) => view,
            Err(c) => return (Err(193 + c), line),
        },
    };
    let base = core::mem::replace(&mut scene.base, view);
    let home = scene.home.replace(HomeState {
        focused: true,
        held: false,
        chip_selected: false,
        line,
        masked,
    });
    let intro = scene.chrome.layout.intro;
    let card = scene.chrome.card().to_vec();
    scene.chrome.set_intro(true);
    scene.chrome.set_card(rows);
    scene.repaint_chrome(damage);
    // The lock's frame is the lock plane alone: no focus light, sprite or
    // drag outline over it.
    let glow = scene.glow.take();
    #[cfg(feature = "desktop-pointer")]
    let (sprite, outline) = (scene.sprite_at.take(), scene.outline.take());
    scene.lock.painting = true;
    let shown = scene.present(false, damage);
    scene.lock.painting = false;
    // Put everything back.
    scene.glow = glow;
    #[cfg(feature = "desktop-pointer")]
    {
        scene.sprite_at = sprite;
        scene.outline = outline;
    }
    scene.chrome.set_intro(intro);
    scene.chrome.set_card(card);
    let line = match core::mem::replace(&mut scene.home, home) {
        Some(HomeState { line, .. }) => line,
        None => Default::default(),
    };
    *plane = Some(core::mem::replace(&mut scene.base, base));
    (shown.map(|_| ()).map_err(|c| 100 + c), line)
}

/// Boot test: the key script waits for `LOCK prompt=<id>`.
fn log_prompt(id: &[u8]) {
    #[cfg(feature = "lock-boot-test")]
    Line::new().s(b"LOCK prompt=").s(id).s(b"\n").emit();
    #[cfg(not(feature = "lock-boot-test"))]
    let _ = id;
}

#[cfg(feature = "lock-boot-test")]
mod test {
    /// `lock_session`: the Home lock (with the client spawned under it) and
    /// the window-view lock. `lock_idle`: the idle lock.
    #[cfg(not(any(
        feature = "lock-idle-test",
        feature = "lock-stall-test",
        feature = "lock-park-test",
        feature = "settings-boot-test",
        feature = "settings-refused-test",
        feature = "arthur-actions-test"
    )))]
    pub const UNLOCKS: u32 = 2;
    /// `login_arthur_permission`, each boot: the two stall legs (the path's
    /// and the intent's), then the lock with a prompt up (boot 1) or Super+L
    /// (boot 2).
    #[cfg(feature = "arthur-actions-test")]
    pub const UNLOCKS: u32 = 3;
    #[cfg(any(
        feature = "lock-idle-test",
        feature = "lock-stall-test",
        feature = "lock-park-test",
        feature = "settings-boot-test",
        feature = "settings-refused-test"
    ))]
    pub const UNLOCKS: u32 = 1;
}

/// Boot test (`lock-spawn-test`): Home, locked, starts Arthur once, as if an
/// intent had been queued, so a client is spawned under the lock.
#[cfg(feature = "lock-spawn-test")]
pub fn spawn_under_lock() -> bool {
    static DONE: AtomicBool = AtomicBool::new(false);
    locked() && !DONE.swap(true, Ordering::AcqRel)
}
