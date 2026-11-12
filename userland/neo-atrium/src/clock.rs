// SPDX-License-Identifier: MPL-2.0
//! The wall clock's words on the desktop (OT-85, feature `desktop-clock` in
//! the WM): the time-of-day greeting and the intro card's date-and-time row.
//! Pure: the WM reads `CLOCK_REALTIME`, formats it with `libneo::civil` (the
//! same function the probe uses) and hands the text here.
//!
//! Wording authority: `Plans/NOTE-2026-10-21-wallclock-intro-s4.md`, which
//! supersedes NOTE-2026-10-20's "Good day" (it stood in only because there
//! was no wall clock). Register: `docs/VISION-arthur-persona.md`. Every line
//! is checked by the persona KAT in `tests.rs`, and the greeting word by the
//! hour table there.
//!
//! The time is the RTC's local wall time with no offset (the kernel reads the
//! RTC as local time; there is no time zone), so the hour here is the hour
//! the owner's clock shows.

use alloc::string::String;
use i18n_core::{Lang, Msg, text};

/// The greeting's boundaries, in local hours: morning from 05:00, afternoon
/// from 12:00, evening from 18:00 until 04:59 (no "good night": that is a
/// farewell, not a greeting).
pub const MORNING_FROM: u8 = 5;
pub const AFTERNOON_FROM: u8 = 12;
pub const EVENING_FROM: u8 = 18;

/// The greeting for local `hour` (0..=23).
pub const fn greeting(hour: u8) -> &'static str {
    greeting_in(Lang::En, hour)
}

/// S6b: [`greeting`] in `lang`.
pub const fn greeting_in(lang: Lang, hour: u8) -> &'static str {
    text(
        lang,
        if hour >= MORNING_FROM && hour < AFTERNOON_FROM {
            Msg::GoodMorning
        } else if hour >= AFTERNOON_FROM && hour < EVENING_FROM {
            Msg::GoodAfternoon
        } else {
            Msg::GoodEvening
        },
    )
}

/// What the WM knows of now: the local hour and the readout's two texts
/// (`libneo::civil`'s `time_text` / `date_text`).
#[derive(Clone, Copy, Debug)]
pub struct Now<'a> {
    pub hour: u8,
    pub time: &'a str,
    pub date: &'a str,
}

/// The intro card's first row: the greeting and the time, e.g.
/// "Good afternoon. It's 5:48 PM on Wednesday, October 21, 2026."
pub fn greeting_row(now: &Now<'_>) -> String {
    greeting_row_in(crate::lang::get(), now)
}

/// S6b: [`greeting_row`] in `lang` (`now`'s texts already in it):
/// "Buenas tardes. Ahora: 5:48 p. m., miércoles 21 de octubre de 2026."
pub fn greeting_row_in(lang: Lang, now: &Now<'_>) -> String {
    i18n_core::format(
        lang,
        Msg::GreetingRow,
        &[greeting_in(lang, now.hour), now.time, now.date],
    )
}
