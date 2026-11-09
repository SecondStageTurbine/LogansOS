// SPDX-License-Identifier: MPL-2.0
//! F11 S1 (`setup-menu`): the first-boot setup menu, as WM chrome on Home
//! (`docs/THINK-2026-11-02-setup-menu.md` §4.1, §5 S1).
//!
//! [`run`] runs between `clock::start` and `login::gate`, on the WM's own
//! focus, so no client can receive a key or draw over it (§4.4). The page
//! machine is `setup-core`; this view only draws its rows on Home's card,
//! turns keys into its inputs and logs each page (`DESKTOP setup page=…`),
//! each redraw and the end (`DESKTOP setup done action=…`).
//!
//! S1 writes nothing: no store write (S2), no clock confirm (S3), no
//! network call (S4), no install action (S7) and no action-log record (the
//! queue arrives with S3's confirm). The choices are logged and dropped.
//!
//! **S3** (`setup-tz`, §2.2, §3.4): the Time zone page lists `libneo::tz`'s
//! zones, searched by typing, each with its abbreviation and offset read from
//! the computer's clock. Leaving it with a zone chosen confirms the clock
//! **once**, through `clock::set_mode` (which also stores `TimeZone`); the
//! page is then review-only. A clock already confirmed when the wizard starts
//! makes it review-only from the start, and a kernel without `rtc-offset`
//! hides it (the kill-switch: the wizard doesn't offer a zone it can't apply).
//!
//! **The action log** (S3 with `arthur-action-log`, §4.4): each choice made
//! leaving a page forward, and the end, is held in a small queue with its
//! monotonic instant, and appended (`Kind::Setup`) only when the wizard ends,
//! after the confirm. Each record's time is the clock now less the time since
//! the choice, so a choice made before the confirm still carries the
//! confirmed UTC instant (never the RTC's local reading as if it were UTC);
//! without a confirm the records carry the clock as read, flagged as such.

use super::home::HomeFocus;
use super::{Line, Rect, Scene, wm_rect};
#[cfg(feature = "setup-journal")]
use alloc::string::String;
use alloc::vec::Vec;
use libneo::{EAGAIN, INPUT_EVENT_V1_LEN, sys_input_focus_recv};
use neo_atrium::keys::{self, Key};
use setup_core::{Finish, Input, Outcome, Wizard, ZONES};

/// Run the wizard to its end. `Ok` when the session may go on.
pub fn run(scene: &mut Scene, focus: &HomeFocus) -> Result<(), i32> {
    let wizard = Wizard::new(cfg!(feature = "setup-stub-pages"));
    #[cfg(feature = "setup-tz")]
    let wizard = tz_page::attach(scene, wizard);
    let mut wizard = wizard;
    #[cfg(feature = "setup-journal")]
    let mut journal = journal::Journal::default();
    begin(scene)?;
    show(scene, &wizard)?;
    log_page(&wizard);
    loop {
        let Some(input) = next_input(scene, focus)? else {
            continue;
        };
        #[cfg(feature = "setup-journal")]
        let page_before = wizard.page();
        match wizard.on(input) {
            Outcome::Redraw => {
                show(scene, &wizard)?;
                Line::new()
                    .s(b"DESKTOP setup redraw page=")
                    .s(wizard.page().name().as_bytes())
                    .s(b" focus=")
                    .n(wizard.focus() as u64)
                    .s(b"\n")
                    .emit();
            }
            Outcome::Page(_) => {
                #[cfg(feature = "setup-journal")]
                journal.left(&wizard, page_before);
                show(scene, &wizard)?;
                log_page(&wizard);
            }
            Outcome::Nothing => {}
            // S3: leaving the Time zone page with a zone chosen.
            Outcome::Confirm(zone) => {
                #[cfg(feature = "setup-tz")]
                let ok = tz_page::confirm(scene, zone)?;
                #[cfg(not(feature = "setup-tz"))]
                let ok = {
                    let _ = zone;
                    false
                };
                wizard.confirmed(ok);
                #[cfg(feature = "setup-journal")]
                journal.left(&wizard, page_before);
                show(scene, &wizard)?;
                log_page(&wizard);
            }
            Outcome::Done(finish) => {
                let c = wizard.choices();
                let line = Line::new()
                    .s(b"DESKTOP setup done action=")
                    .s(match finish {
                        Finish::Try => b"try" as &[u8],
                        Finish::Skipped => b"skip",
                    });
                let zone = wizard.zone_name().unwrap_or(if wizard.reviewed() {
                    "unknown"
                } else {
                    ZONES[c.zone]
                });
                #[cfg(feature = "setup-tz")]
                let zone = if c.tz.is_none() && !wizard.reviewed() {
                    "unset"
                } else {
                    zone
                };
                let line = match finish {
                    Finish::Try => line
                        .s(b" zone=")
                        .s(zone.as_bytes())
                        .s(b" reduce_motion=")
                        .n(u64::from(c.reduce_motion))
                        .s(b" reduce_transparency=")
                        .n(u64::from(c.reduce_transparency)),
                    Finish::Skipped => line,
                };
                line.s(b"\n").emit();
                #[cfg(feature = "setup-store")]
                if finish == Finish::Try {
                    store(scene, &c);
                }
                #[cfg(feature = "setup-journal")]
                {
                    journal.note(
                        "done",
                        String::from(match finish {
                            Finish::Try => "try",
                            Finish::Skipped => "skip",
                        }),
                    );
                    journal.flush();
                }
                return end(scene);
            }
        }
    }
}

/// F11 S2: the choices into the settings store, through the WM's own path
/// (`set_wm`: checked, written to the file not holding the record and
/// applied live; on the volatile overlay, in memory only). The zone is S3's,
/// with the clock confirm. Skip writes nothing: the defaults stand.
#[cfg(feature = "setup-store")]
fn store(scene: &mut Scene, c: &setup_core::Choices) {
    use settings_core::Key;
    let rm = if c.reduce_motion { 1 } else { 0 };
    let writes = [
        (Key::ReduceTransparency, u32::from(c.reduce_transparency)),
        (Key::ReduceMotion, rm),
        // One language and one layout exist (S6/S5 add the rest).
        (Key::Language, 0),
        (Key::KeyboardLayout, 0),
    ];
    let mut refused = 0u64;
    for (key, value) in writes {
        if super::settings::set_wm(scene, key.id(), value) != 0 {
            refused += 1;
        }
    }
    Line::new()
        .s(b"DESKTOP setup stored keys=4 refused=")
        .n(refused)
        .s(b" volatile=")
        .n(u64::from(super::settings::volatile()))
        .s(b"\n")
        .emit();
}

/// S3: the queue of choices for the action log (§4.4).
#[cfg(feature = "setup-journal")]
mod journal {
    use super::super::{Line, action_log};
    use action_log_core::{Kind, TIME_UTC};
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;
    use libneo::{CLOCK_MONOTONIC, sys_clock_gettime};
    use setup_core::{Page, Wizard};

    struct Event {
        page: &'static str,
        choice: String,
        mono_ns: u64,
    }

    /// The choices, in order, each with the monotonic instant it was made.
    #[derive(Default)]
    pub struct Journal(Vec<Event>);

    impl Journal {
        pub fn note(&mut self, page: &'static str, choice: String) {
            let mono_ns = sys_clock_gettime(CLOCK_MONOTONIC).unwrap_or(0);
            self.0.push(Event {
                page,
                choice,
                mono_ns,
            });
        }

        /// `wizard` now shows a page after `before`: note the choice
        /// `before` was left with. Back, and the skip question, note nothing.
        pub fn left(&mut self, wizard: &Wizard, before: Page) {
            let pages = wizard.pages();
            let at = |p: Page| pages.iter().position(|&q| q == p);
            let (Some(from), Some(to)) = (at(before), at(wizard.page())) else {
                return;
            };
            if to <= from {
                return;
            }
            let c = wizard.choices();
            let choice = match before {
                Page::Welcome => String::from("begin"),
                Page::TimeZone => String::from(wizard.zone_name().unwrap_or("unset")),
                Page::Comfort => format!(
                    "rm={} rt={}",
                    u8::from(c.reduce_motion),
                    u8::from(c.reduce_transparency)
                ),
                _ => String::from("continue"),
            };
            self.note(before.name(), choice);
        }

        /// Append every choice, in order: each at the clock now less the
        /// time since it was made, with the clock's flag now (`TIME_UTC`
        /// once confirmed). A failed append (a read-only volume) drops the
        /// rest: the wizard writes nothing it can't.
        pub fn flush(self) {
            let (now_s, flags) = action_log::now();
            let mono = sys_clock_gettime(CLOCK_MONOTONIC).unwrap_or(0);
            let mut appended = 0u64;
            for e in &self.0 {
                let ago_s = mono.saturating_sub(e.mono_ns) / 1_000_000_000;
                let time_s = now_s.saturating_sub(ago_s);
                let ok = action_log::append_at(Kind::Setup, e.page, &e.choice, time_s, flags);
                Line::new()
                    .s(b"DESKTOP setup journal page=")
                    .s(e.page.as_bytes())
                    .s(b" choice=")
                    .s(e.choice.as_bytes())
                    .s(b" time_s=")
                    .n(time_s)
                    .s(b" utc=")
                    .n(u64::from(flags & TIME_UTC))
                    .s(b" ok=")
                    .n(u64::from(ok))
                    .s(b"\n")
                    .emit();
                if !ok {
                    break;
                }
                appended += 1;
            }
            Line::new()
                .s(b"DESKTOP setup journal records=")
                .n(self.0.len() as u64)
                .s(b" appended=")
                .n(appended)
                .s(b"\n")
                .emit();
            action_log::report();
        }
    }
}

/// S3: the zone directory over `libneo::tz`, and the one confirm.
#[cfg(feature = "setup-tz")]
mod tz_page {
    use super::super::{Line, Scene, clock};
    use alloc::boxed::Box;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;
    use libneo::tz::{self, Basis, LocalResult, ZoneId};
    use setup_core::{Wizard, ZONES, Zones};

    /// The directory, with the computer's clock as read when the page was
    /// built: the preview's offsets are for that reading.
    struct Tz {
        wall_s: i64,
        rtc_local: bool,
    }

    impl Zones for Tz {
        fn shortlist(&self) -> Vec<u16> {
            ZONES.iter().filter_map(|n| tz::zone_by_name(n)).collect()
        }

        fn search(&self, typed: &str) -> Vec<u16> {
            match tz::find_zone(typed.as_bytes()) {
                Ok(id) => alloc::vec![id],
                Err(near) => near.iter().flatten().copied().collect(),
            }
        }

        fn name(&self, id: u16) -> &'static str {
            tz::zone(id).map_or("?", |z| z.name)
        }

        /// The abbreviation and offset this reading has in `id`: an RTC that
        /// holds local time reads as local time there (an ambiguous reading
        /// takes the earlier instant, as the confirm does).
        fn detail(&self, id: u16) -> String {
            let utc = if self.rtc_local {
                match tz::local_to_utc(self.wall_s, id) {
                    LocalResult::Single(u) | LocalResult::Ambiguous(u, _) => u,
                    LocalResult::Gap { offset_after, .. } => offset_after,
                }
            } else {
                self.wall_s
            };
            let (_, off) = tz::utc_to_local(utc, id);
            let abbr = if id == tz::UTC { "UTC" } else { off.abbr };
            let (sign, s) = if off.seconds < 0 {
                ('-', -off.seconds)
            } else {
                ('+', off.seconds)
            };
            match (s / 3600, s % 3600 / 60) {
                (h, 0) => format!("{abbr}, UTC{sign}{h}"),
                (h, m) => format!("{abbr}, UTC{sign}{h}:{m:02}"),
            }
        }
    }

    /// The wizard with the page as the clock allows: hidden on a kernel
    /// without `rtc-offset`, review-only when already confirmed, otherwise
    /// searchable.
    pub fn attach(scene: &Scene, wizard: Wizard) -> Wizard {
        let mode = scene.clock.mode();
        let wall_s = libneo::sys_clock_gettime(libneo::CLOCK_REALTIME)
            .map_or(0, |ns| (ns / 1_000_000_000) as i64);
        let (page, confirmed) = match mode.basis {
            Basis::Legacy => ("hidden", None),
            Basis::Utc => ("review", Some(Some(mode.zone))),
            Basis::Rtc => ("search", None),
        };
        Line::new()
            .s(b"DESKTOP setup tz page=")
            .s(page.as_bytes())
            .s(b" rtc=")
            .s(if mode.rtc_local { b"local" } else { b"utc" })
            .s(b"\n")
            .emit();
        if mode.basis == Basis::Legacy {
            return wizard.without_time_zone();
        }
        let zones = Tz {
            wall_s,
            rtc_local: mode.rtc_local,
        };
        wizard.with_zones(Box::new(zones), confirmed)
    }

    /// The one confirm: `clock::set_mode` stores the zone and, while the
    /// clock is unconfirmed, confirms it. `true`: the clock is now confirmed
    /// UTC. UTC itself is stored with an RTC that holds UTC: the same
    /// reading, and the only way UTC can be confirmed (an RTC holding local
    /// time with zone 0 means "no zone", `tz::rtc_reading_utc`).
    pub fn confirm(scene: &mut Scene, zone: ZoneId) -> Result<bool, i32> {
        let before = scene.clock.mode();
        if before.basis == Basis::Utc {
            // Confirmed meanwhile (write-once): no second call.
            log(zone, b"already");
            return Ok(true);
        }
        let rtc_local = before.rtc_local && zone != tz::UTC;
        clock::set_mode(scene, zone, rtc_local)?;
        let ok = scene.clock.mode().basis == Basis::Utc;
        log(zone, if ok { b"1" } else { b"0" });
        Ok(ok)
    }

    fn log(zone: ZoneId, confirmed: &[u8]) {
        Line::new()
            .s(b"DESKTOP setup tz=")
            .s(tz::zone(zone).map_or("?", |z| z.name).as_bytes())
            .s(b" confirmed=")
            .s(confirmed)
            .s(b"\n")
            .emit();
    }
}

fn log_page(wizard: &Wizard) {
    Line::new()
        .s(b"DESKTOP setup page=")
        .s(wizard.page().name().as_bytes())
        .s(b" rows=")
        .n(wizard.rows().len() as u64)
        .s(b"\n")
        .emit();
}

/// The next key the wizard uses, as its input (`None`: a key it ignores).
fn next_input(scene: &mut Scene, focus: &HomeFocus) -> Result<Option<Input>, i32> {
    let mut event = [0u8; INPUT_EVENT_V1_LEN];
    match sys_input_focus_recv(focus.events, &mut event) {
        0 => {}
        EAGAIN => {
            idle(scene, focus);
            return Ok(None);
        }
        _ => return Err(190),
    }
    Ok(match keys::decode(event) {
        Some(Key::Up) => Some(Input::Up),
        Some(Key::Down | Key::Tab) => Some(Input::Down),
        Some(Key::Enter) => Some(Input::Choose),
        // Space chooses, except where it types (the S3 search).
        Some(Key::Char(c)) => Some(Input::Char(c)),
        Some(Key::Backspace) => Some(Input::Erase),
        Some(Key::Escape) => Some(Input::Back),
        _ => None,
    })
}

/// Home with the card, as the login screens show it.
fn begin(scene: &mut Scene) -> Result<(), i32> {
    scene.chrome.set_intro(true);
    scene.show_home(true, false);
    scene
        .present(false, scene.geo.screen)
        .map(|_| ())
        .map_err(|c| 100 + c)
}

/// Leave Home as `run()` expects it: no card, an empty field.
fn end(scene: &mut Scene) -> Result<(), i32> {
    scene.chrome.set_card(Vec::new());
    if let Some(home) = scene.home.as_mut() {
        home.line.wipe();
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

/// The page's rows on the card.
fn show(scene: &mut Scene, wizard: &Wizard) -> Result<(), i32> {
    scene.chrome.set_card(wizard.rows());
    let r = damage(scene);
    scene.repaint_chrome(r);
    scene.present(false, r).map(|_| ()).map_err(|c| 100 + c)
}

/// Wait for input, keeping the pointer route drained.
fn idle(scene: &mut Scene, focus: &HomeFocus) {
    #[cfg(feature = "desktop-pointer")]
    scene.pointer_idle();
    #[cfg(feature = "desktop-park")]
    scene.park(
        &[libneo::WaitItem {
            handle: focus.events.0,
            kind: libneo::WAIT_KIND_INPUT,
        }],
        u64::MAX,
    );
    #[cfg(not(feature = "desktop-park"))]
    {
        let _ = (scene, focus);
        core::hint::spin_loop();
    }
}
