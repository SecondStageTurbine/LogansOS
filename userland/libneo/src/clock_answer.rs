// SPDX-License-Identifier: MPL-2.0
//! F8 (J.c.'s feedback list): Arthur answers "what time is it" and "what's
//! the date" from the system clock, in the user's zone, before the model
//! (which has no clock and says so). Pure: no allocation, no syscall.
//! Host-tested by `tools/tz-logic-tests` (mounted with `civil` and `tz`).
//!
//! The question match is deliberately narrow: a fixed set of phrasings,
//! after lower-casing, dropping apostrophes and the end punctuation and
//! collapsing spaces; any other punctuation means it isn't one. Anything else ("what time is it in Tokyo", "what time
//! does the shop open") still goes to the model.

use super::civil::{self, Text};
use super::tz::{self, Basis, Shown, ZoneId};

/// What was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Question {
    Time,
    Date,
}

const TIME: [&str; 10] = [
    "what time is it",
    "what time is it now",
    "whats the time",
    "whats the time now",
    "what is the time",
    "what is the time now",
    "tell me the time",
    "the time",
    "time",
    "current time",
];

const DATE: [&str; 13] = [
    "whats the date",
    "whats the date today",
    "what is the date",
    "what is the date today",
    "whats todays date",
    "what is todays date",
    "todays date",
    "what day is it",
    "what day is it today",
    "what day is today",
    "tell me the date",
    "the date",
    "date",
];

/// The longest line considered (longer ones are never these questions).
const MAX: usize = 48;

/// The question `line` asks, if it is one of the phrasings above. Only
/// end punctuation (`?`, `.`, `!`, after the words) and apostrophes are
/// dropped, and runs of spaces count as one; any other punctuation, such
/// as `what,time is it`, is not one of these questions (Jarvis, #68).
pub fn question(line: &[u8]) -> Option<Question> {
    // Trim the ends: spaces, then end punctuation (and spaces among it).
    let mut end = line.len();
    while end > 0 && matches!(line[end - 1], b' ' | b'\t' | b'?' | b'.' | b'!') {
        end -= 1;
    }
    let mut buf = [0u8; MAX];
    let mut len = 0;
    let mut space = false;
    for &b in &line[..end] {
        let c = match b {
            b'A'..=b'Z' => b + 32,
            b'a'..=b'z' | b'0'..=b'9' => b,
            b'\'' => continue,
            b' ' | b'\t' => {
                space = len != 0;
                continue;
            }
            _ => return None,
        };
        if space {
            if len == MAX {
                return None;
            }
            buf[len] = b' ';
            len += 1;
            space = false;
        }
        if len == MAX {
            return None;
        }
        buf[len] = c;
        len += 1;
    }
    let text = core::str::from_utf8(&buf[..len]).ok()?;
    if TIME.contains(&text) {
        Some(Question::Time)
    } else if DATE.contains(&text) {
        Some(Question::Date)
    } else {
        None
    }
}

/// The reply (fits every zone, both questions and the longest date).
pub type Answer = Text<160>;

const NOT_SET: &str =
    " by this computer's clock. Your time zone isn't set: type timezone Area/City at Home.";

/// The answer to `q` for `realtime` (seconds since 1970) read with `basis`,
/// shown in `zone` the way Home's readout shows it (`tz::shown`).
pub fn answer(q: Question, realtime: i64, basis: Basis, zone: ZoneId) -> Answer {
    let (wall, label) = tz::shown(realtime, basis, zone);
    let c = civil::from_realtime_ns((wall.max(0) as u64).saturating_mul(civil::NS_PER_SECOND));
    let mut a = Answer::new();
    match q {
        Question::Time => {
            a.push("It's ");
            a.push(c.time_text().as_str());
            if let Shown::Zone(abbr) = label {
                a.push(" ");
                a.push(abbr);
            }
        }
        Question::Date => {
            a.push("Today is ");
            a.push(c.date_text().as_str());
        }
    }
    a.push(if label == Shown::NotSet { NOT_SET } else { "." });
    a
}
