// SPDX-License-Identifier: MPL-2.0
//! F11 S1: the first-boot setup menu's page machine, with no I/O
//! (`docs/THINK-2026-11-02-setup-menu.md` §2.1, §4.2, §4.5, §5 S1).
//!
//! The WM's view (`neo-init/src/desktop_session/setup.rs`) only draws the
//! rows this crate gives it, turns keys into [`Input`]s and logs. Every
//! decision is here, so the host KATs run the whole walk without QEMU:
//!
//! - the pages this build shows, in order ([`Wizard::pages`]): Welcome, then
//!   (behind `stub_pages`) Language, then Keyboard (also with
//!   `keyboard-layouts`, S5, where it is the real page), then Time zone,
//!   Comfort and Try. There is no Network page until S4 (§3.3);
//! - the key map: Up/Down and Tab move, Enter or Space chooses, Esc goes
//!   back (on Welcome it asks before skipping, §4.2);
//! - the rows: a title (with `N of M`, counting the pages after Welcome),
//!   a body, the options and a hint, never more than [`MAX_ROWS`]. The
//!   focused option is marked `>` and a chosen one `[x]`, so no state is
//!   colour-only (§4.3);
//! - the choices, held until the wizard ends. S1 writes nothing: the store
//!   (S2), the clock confirm (S3) and the Install action (S7) come later.
//!
//! **S3, the Time zone page with the one confirm** (§2.2, §3.4, §5 S3): with
//! a [`Zones`] directory attached ([`Wizard::with_zones`]) the page lists real
//! zones, searched by typing; leaving it forward with a zone chosen asks the
//! view to confirm the clock once ([`Outcome::Confirm`]), and from then on the
//! page is review-only for the session (write-once). Without a directory the
//! page is S1's short list, unchanged; [`Wizard::without_time_zone`] hides it
//! (the `rtc-offset` kill-switch: no zone the clock cannot apply).
//!
//! **S6b, the language** (§3.1, §5 S6): every row comes from `i18n-core`'s
//! catalog, in the language chosen ([`Choices::lang`]). With `i18n-catalog`
//! the Language page lists the catalog's languages, and choosing one redraws
//! the wizard in it. Without it the language is English and the page is S1's
//! stub (shown only with `stub_pages`): the rows are today's, as the host
//! KATs hold.

#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
pub use i18n_core::Lang;
use i18n_core::{LANGUAGE_COUNT, Msg, format as fill, text};

/// The card's row budget (`neo-atrium` `CARD_MAX_ROWS`).
pub const MAX_ROWS: usize = 8;

/// The Time zone page's short list: a pointer alone can pick from it
/// (§4.2). Typing a name in the field is S3's.
pub const ZONES: [&str; 4] = [
    "UTC",
    "America/New_York",
    "Europe/London",
    "Australia/Sydney",
];

/// The Language page lists the catalog's languages (S6b): with
/// `i18n-catalog`, once there is more than one.
pub const LANGUAGE_PAGE: bool = cfg!(feature = "i18n-catalog") && LANGUAGE_COUNT > 1;

/// The longest search the Time zone page keeps (a city name fits).
pub const QUERY_MAX: usize = 32;

/// The zones the Time zone page shows (S3), supplied by the view: the WM's is
/// `libneo::tz`; the host KATs use a fixed table. Ids are `settings-core`'s
/// `TimeZone` values.
pub trait Zones {
    /// The zones shown before anything is typed (pointer-pickable, §4.2).
    fn shortlist(&self) -> Vec<u16>;
    /// The zones matching what was typed, at most four.
    fn search(&self, typed: &str) -> Vec<u16>;
    /// The IANA name.
    fn name(&self, id: u16) -> &'static str;
    /// The preview after the name: abbreviation and offset now, read from
    /// the computer's clock (`EDT, UTC-4`).
    fn detail(&self, id: u16) -> String;
}

/// A page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Welcome,
    /// "Skip setup and use the defaults?" (Esc on Welcome).
    SkipConfirm,
    /// The languages (S6b, `i18n-catalog`); S1's stub without it (§3.1).
    Language,
    /// A stub until a second layout exists (§3.2).
    Keyboard,
    /// S4: the wired link's status, read from the kernel (§3.3).
    Network,
    TimeZone,
    Comfort,
    /// The summary and "Try LogansOS" (Install is S7's).
    Try,
}

impl Page {
    /// The name the log uses (`DESKTOP setup page=<name>`).
    pub const fn name(self) -> &'static str {
        match self {
            Page::Welcome => "welcome",
            Page::SkipConfirm => "skip",
            Page::Language => "language",
            Page::Keyboard => "keyboard",
            Page::Network => "network",
            Page::TimeZone => "timezone",
            Page::Comfort => "comfort",
            Page::Try => "try",
        }
    }
}

/// What a key does here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Up,
    /// Down, and Tab.
    Down,
    /// Enter (and Space, except where it types).
    Choose,
    /// Esc.
    Back,
    /// A printable character: typed into the Time zone search (S3), and
    /// into the Keyboard page's test field (S5, decoded in the layout chosen
    /// on the page); Space chooses on every other page; any other character
    /// does nothing there.
    Char(u8),
    /// Backspace: erases from the search (S3) or the test field (S5).
    Erase,
    /// S5: a key on the Keyboard page, as the layout chosen there types it
    /// (any legend, or a dead key); the page composes it into the test field.
    #[cfg(feature = "keyboard-layouts")]
    Layout(keyboard::Typed),
}

/// How the wizard ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// Try LogansOS: the session starts with the choices.
    Try,
    /// Skip setup: the defaults.
    Skipped,
}

/// What an input did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing: a key this page doesn't use.
    Nothing,
    /// The focus moved, or a choice changed: redraw this page.
    Redraw,
    /// A new page is shown.
    Page(Page),
    /// The wizard is over.
    Done(Finish),
    /// S3: the Time zone page was left with this zone chosen. The view
    /// confirms the clock once, then calls [`Wizard::confirmed`], which shows
    /// the next page.
    Confirm(u16),
}

/// What the user chose. The defaults are the store's (`settings-core`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Choices {
    /// An index into [`ZONES`] (S1's short list, without a [`Zones`]).
    pub zone: usize,
    pub reduce_motion: bool,
    pub reduce_transparency: bool,
    /// S3: the zone chosen on the searchable page (`None`: not set).
    pub tz: Option<u16>,
    /// S6b: the language the wizard (and the session) shows; English
    /// without `i18n-catalog`.
    pub lang: Lang,
    /// S5: a keyboard layout id (`keymap_layout_logic.rs`; 0: US).
    #[cfg(feature = "keyboard-layouts")]
    pub layout: u8,
}

/// S4: what the Network page says (§3.3), from the kernel's
/// `SYS_NET_IF_STATUS`. The page only reports: nothing is chosen or stored,
/// and Continue is always there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Network {
    /// The kernel has no status call (the `net-info` kill-switch) or the call
    /// failed.
    Unavailable,
    /// No device, or a device without link (`driver`: its driver, if any).
    NoLink { driver: Option<String> },
    /// Link up. `address`: the lease's (or the interface's) IPv4 address;
    /// `lease_secs`: the lease's time left, if one is bound.
    Connected {
        driver: String,
        address: Option<[u8; 4]>,
        lease_secs: Option<u64>,
    },
}

impl Network {
    fn connected(&self) -> bool {
        matches!(
            self,
            Network::Connected {
                address: Some(_),
                ..
            }
        )
    }

    /// The page's body rows, in `lang`.
    fn rows(&self, lang: Lang) -> Vec<String> {
        let t = |m| String::from(text(lang, m));
        let mut b = Vec::new();
        match self {
            Network::Unavailable => b.push(t(Msg::NetUnavailable)),
            Network::NoLink { driver } => {
                b.push(t(Msg::NetNoLink));
                if let Some(d) = driver {
                    b.push(fill(lang, Msg::NetDevice, &[d]));
                }
            }
            Network::Connected {
                driver,
                address,
                lease_secs,
            } => {
                b.push(match address {
                    Some(a) => {
                        let dotted = format!("{}.{}.{}.{}", a[0], a[1], a[2], a[3]);
                        fill(lang, Msg::NetConnected, &[&dotted])
                    }
                    None => t(Msg::NetNoAddress),
                });
                b.push(match lease_secs {
                    Some(s) => fill(lang, Msg::NetDeviceLease, &[driver, &lease_left(lang, *s)]),
                    None => fill(lang, Msg::NetDevice, &[driver]),
                });
            }
        }
        b.push(t(Msg::NetWifiLater));
        b
    }
}

/// A lease's time left, in words: "23 h 59 min left", "5 min left",
/// "under a minute left" (in English).
pub fn lease_left(lang: Lang, secs: u64) -> String {
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    match (h, m) {
        (0, 0) => String::from(text(lang, Msg::LeaseUnderMinute)),
        (0, m) => fill(lang, Msg::LeaseMinutes, &[&format!("{m}")]),
        (h, m) => fill(lang, Msg::LeaseHours, &[&format!("{h}"), &format!("{m}")]),
    }
}

/// S3's Time zone page state.
struct TzPage {
    zones: Box<dyn Zones>,
    /// What was typed (ASCII).
    query: String,
    /// The zones listed: the short list, or the search's matches.
    listed: Vec<u16>,
    /// The clock is confirmed with [`Choices::tz`] (or was at the start):
    /// the page only shows it (write-once, §2.2).
    review: bool,
}

/// The wizard: the page shown, the focused option, the choices.
pub struct Wizard {
    stub_pages: bool,
    /// A one-time row under the next page's title (S3: the store couldn't
    /// save the zone); cleared when another page is shown.
    notice: Option<String>,
    /// The Time zone page is shown (off: the `rtc-offset` kill-switch).
    time_zone: bool,
    /// S4: the Network page, with what it says (`None`: no page).
    network: Option<Network>,
    tz: Option<TzPage>,
    page: Page,
    focus: usize,
    choices: Choices,
    /// S5: the Keyboard page's test field.
    #[cfg(feature = "keyboard-layouts")]
    field: keyboard::Field,
}

impl Wizard {
    /// A wizard on Welcome. `stub_pages` shows the Language and Keyboard
    /// stubs (the `setup-stub-pages` build).
    pub fn new(stub_pages: bool) -> Self {
        Self {
            stub_pages,
            notice: None,
            time_zone: true,
            network: None,
            tz: None,
            page: Page::Welcome,
            focus: 0,
            choices: Choices::default(),
            #[cfg(feature = "keyboard-layouts")]
            field: keyboard::Field::default(),
        }
    }

    /// S3: the Time zone page lists `zones`, searched by typing, and leaving
    /// it confirms the clock once. `confirmed`: the clock already is (by an
    /// earlier session, with this zone, or `None` if unknown): the page is
    /// review-only from the start.
    pub fn with_zones(mut self, zones: Box<dyn Zones>, confirmed: Option<Option<u16>>) -> Self {
        let listed = zones.shortlist();
        let review = confirmed.is_some();
        if let Some(zone) = confirmed {
            self.choices.tz = zone;
        }
        self.tz = Some(TzPage {
            zones,
            query: String::new(),
            listed,
            review,
        });
        self
    }

    /// S4: show the Network page, saying `network` (§3.3).
    pub fn with_network(mut self, network: Network) -> Self {
        self.network = Some(network);
        self
    }

    /// Hide the Time zone page (the kernel can't confirm a zone: §3.4).
    pub fn without_time_zone(mut self) -> Self {
        self.time_zone = false;
        self
    }

    /// S3: the search typed on the Time zone page.
    pub fn query(&self) -> &str {
        self.tz.as_ref().map_or("", |t| t.query.as_str())
    }

    /// S3: the clock is confirmed; the Time zone page is review-only.
    pub fn reviewed(&self) -> bool {
        self.tz.as_ref().is_some_and(|t| t.review)
    }

    /// S3: the view's answer to [`Outcome::Confirm`]. `ok`: the clock is now
    /// confirmed with the chosen zone; otherwise it stays unconfirmed (the
    /// zone is kept as a choice, and the page stays editable). Either way the
    /// next page is shown.
    pub fn confirmed(&mut self, ok: bool) -> Outcome {
        if let Some(t) = self.tz.as_mut() {
            t.review = ok;
        }
        self.show(self.next(Page::TimeZone))
    }

    /// S3: what the settings store holds after a failed confirm (the PM's
    /// rule, #92: the durable record is the truth). `zone`: its zone (`None`:
    /// not set); `confirmed`: the clock is confirmed with it. The page is
    /// review-only if confirmed, editable otherwise.
    pub fn reconciled(&mut self, zone: Option<u16>, confirmed: bool) {
        self.choices.tz = zone;
        if let Some(t) = self.tz.as_mut() {
            t.review = confirmed;
        }
    }

    /// S3: a failed confirm whose store couldn't be re-read (Chad, #92 relay
    /// 379). The page stays, editable, with the zone that was chosen; nothing
    /// is claimed as stored.
    pub fn unresolved(&mut self) -> Outcome {
        if let Some(t) = self.tz.as_mut() {
            t.review = false;
        }
        self.show(Page::TimeZone)
    }

    /// A one-time row under the title of the page shown next (S3's
    /// "Couldn't save the time zone"). Cleared when another page is shown.
    pub fn notice(&mut self, text: String) {
        self.notice = Some(text);
    }

    /// The zone's name, for the log and the Try summary (S3).
    pub fn zone_name(&self) -> Option<&'static str> {
        let t = self.tz.as_ref()?;
        Some(t.zones.name(self.choices.tz?))
    }

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn focus(&self) -> usize {
        self.focus
    }

    pub fn choices(&self) -> Choices {
        self.choices
    }

    /// S5: the Keyboard page's test field.
    #[cfg(feature = "keyboard-layouts")]
    pub fn field(&self) -> &str {
        keyboard::field(self)
    }

    /// The pages this build shows, in order (Skip's confirmation is not one).
    pub fn pages(&self) -> Vec<Page> {
        let mut pages = Vec::new();
        pages.push(Page::Welcome);
        if self.stub_pages || LANGUAGE_PAGE {
            pages.push(Page::Language);
        }
        if self.stub_pages || keyboard::SHOWN {
            pages.push(Page::Keyboard);
        }
        if self.network.is_some() {
            pages.push(Page::Network);
        }
        if self.time_zone {
            pages.push(Page::TimeZone);
        }
        pages.push(Page::Comfort);
        pages.push(Page::Try);
        pages
    }

    /// The language the rows are in (S6b).
    pub fn lang(&self) -> Lang {
        self.choices.lang
    }

    /// S6b: start in `lang` (the stored `Language`). Without `i18n-catalog`
    /// the wizard stays in English.
    pub fn with_lang(mut self, lang: Lang) -> Self {
        if cfg!(feature = "i18n-catalog") {
            self.choices.lang = lang;
        }
        self
    }

    /// `msg` in the wizard's language.
    pub(crate) fn t(&self, msg: Msg) -> String {
        String::from(text(self.choices.lang, msg))
    }

    /// `msg` in the wizard's language, with its `{}` arguments.
    pub(crate) fn f(&self, msg: Msg, args: &[&str]) -> String {
        fill(self.choices.lang, msg, args)
    }

    /// The options of the page shown, as their labels (without marks).
    pub fn options(&self) -> Vec<String> {
        let mut o = Vec::new();
        match self.page {
            Page::Welcome => {
                o.push(self.t(Msg::SetupBegin));
                o.push(self.t(Msg::SetupSkip));
            }
            Page::SkipConfirm => {
                o.push(self.t(Msg::SetupSkipDefaults));
                o.push(self.t(Msg::SetupBack));
            }
            Page::Language => {
                if LANGUAGE_PAGE {
                    for lang in Lang::ALL {
                        o.push(format!(
                            "{} {}",
                            mark(lang == self.choices.lang),
                            lang.name()
                        ));
                    }
                }
                o.push(self.t(Msg::SetupContinue));
            }
            Page::Keyboard => keyboard::options(self, &mut o),
            Page::Network => o.push(if self.network.as_ref().is_some_and(Network::connected) {
                self.t(Msg::SetupContinue)
            } else {
                self.t(Msg::SetupContinueNoNetwork)
            }),
            Page::TimeZone if self.tz.is_some() => {
                let t = self.tz.as_ref().expect("checked");
                if !t.review {
                    for &id in &t.listed {
                        o.push(format!(
                            "{} {} ({})",
                            mark(self.choices.tz == Some(id)),
                            t.zones.name(id),
                            t.zones.detail(id)
                        ));
                    }
                }
                o.push(self.t(Msg::SetupContinue));
            }
            Page::TimeZone => {
                for (i, z) in ZONES.iter().enumerate() {
                    o.push(format!("{} {}", mark(i == self.choices.zone), z));
                }
                o.push(self.t(Msg::SetupContinue));
            }
            Page::Comfort => {
                o.push(self.f(
                    Msg::SetupReduceMotionOption,
                    &[mark(self.choices.reduce_motion)],
                ));
                o.push(self.f(
                    Msg::SetupReduceTransparencyOption,
                    &[mark(self.choices.reduce_transparency)],
                ));
                o.push(self.t(Msg::SetupContinue));
            }
            Page::Try => o.push(self.t(Msg::SetupTryOption)),
        }
        o
    }

    /// The card's rows for the page shown: title, body, options (the
    /// focused one marked `>`), hint.
    pub fn rows(&self) -> Vec<String> {
        let mut rows = Vec::new();
        rows.push(self.title());
        if let Some(n) = &self.notice {
            rows.push(n.clone());
        }
        for line in self.body() {
            rows.push(line);
        }
        for (i, label) in self.options().into_iter().enumerate() {
            let focus = if i == self.focus { ">" } else { " " };
            rows.push(format!("{focus} {label}"));
        }
        rows.push(self.t(if self.searching() {
            Msg::SetupSearchHint
        } else {
            Msg::SetupHint
        }));
        rows
    }

    /// The searchable Time zone page is shown (S3, not yet confirmed).
    fn searching(&self) -> bool {
        self.page == Page::TimeZone && self.tz.as_ref().is_some_and(|t| !t.review)
    }

    fn title(&self) -> String {
        let name = match self.page {
            Page::Welcome => return self.t(Msg::SetupTitleWelcome),
            Page::SkipConfirm => return self.t(Msg::SetupTitleSkip),
            Page::Language => Msg::SetupTitleLanguage,
            Page::Keyboard => Msg::SetupTitleKeyboard,
            Page::Network => Msg::SetupTitleNetwork,
            Page::TimeZone => Msg::SetupTitleTimeZone,
            Page::Comfort => Msg::SetupTitleComfort,
            Page::Try => Msg::SetupTitleTry,
        };
        let pages = self.pages();
        let total = pages.len() - 1;
        let n = pages.iter().position(|&p| p == self.page).unwrap_or(0);
        self.f(
            Msg::SetupTitlePage,
            &[&self.t(name), &format!("{n}"), &format!("{total}")],
        )
    }

    fn body(&self) -> Vec<String> {
        let mut b = Vec::new();
        match self.page {
            Page::Welcome => b.push(self.t(Msg::SetupBodyWelcome)),
            Page::SkipConfirm => b.push(self.t(Msg::SetupBodySkip)),
            Page::Language if LANGUAGE_PAGE => {
                b.push(self.t(Msg::SetupBodyLanguageChoose));
                b.push(self.t(Msg::SetupBodyLanguageArthur));
            }
            Page::Language => b.push(self.t(Msg::SetupBodyLanguage)),
            Page::Keyboard => keyboard::body(self, &mut b),
            Page::Network => {
                if let Some(n) = &self.network {
                    b.extend(n.rows(self.choices.lang));
                }
            }
            Page::TimeZone => match self.tz.as_ref() {
                None => b.push(self.t(Msg::SetupBodyTimeZoneNoSearch)),
                Some(t) if t.review => {
                    let not_set = self.t(Msg::SetupNotSet);
                    let name = self.zone_name().unwrap_or(&not_set);
                    b.push(self.f(Msg::SetupBodyZoneSet, &[name]));
                    b.push(self.t(Msg::SetupBodyZoneLater));
                }
                Some(t) if t.query.is_empty() => b.push(self.t(Msg::SetupBodySearchEmpty)),
                Some(t) if t.listed.is_empty() => {
                    b.push(self.f(Msg::SetupBodySearchNone, &[&t.query]))
                }
                Some(t) => b.push(self.f(Msg::SetupBodySearch, &[&t.query])),
            },
            Page::Comfort => b.push(self.t(Msg::SetupBodyComfort)),
            Page::Try => {
                for line in self.summary() {
                    b.push(line);
                }
            }
        }
        b
    }

    /// The Try page's summary: what the session will start with.
    pub fn summary(&self) -> Vec<String> {
        let mut s = Vec::new();
        s.push(match self.tz.as_ref() {
            None => self.f(Msg::ZoneRow, &[ZONES[self.choices.zone]]),
            Some(t) => match self.zone_name() {
                Some(name) if t.review => self.f(Msg::ZoneRow, &[name]),
                Some(name) => self.f(Msg::SetupSummaryZoneClockNotSet, &[name]),
                None => self.t(Msg::ZoneRowNotSet),
            },
        });
        keyboard::summary(self, &mut s);
        s.push(self.f(
            Msg::SetupSummaryComfort,
            &[
                &self.t(on_off(self.choices.reduce_motion)),
                &self.t(on_off(self.choices.reduce_transparency)),
            ],
        ));
        s
    }

    /// Apply one input.
    pub fn on(&mut self, input: Input) -> Outcome {
        let count = self.options().len();
        if self.searching() {
            match input {
                Input::Char(c) => return self.type_char(c),
                Input::Erase => return self.erase(),
                _ => {}
            }
        }
        // S5: the Keyboard page's test field takes characters (not Space,
        // which chooses) and Backspace.
        #[cfg(feature = "keyboard-layouts")]
        match input {
            Input::Layout(t) if self.page == Page::Keyboard => {
                return keyboard::typed(self, Some(t));
            }
            Input::Char(c) if self.page == Page::Keyboard && c != b' ' => {
                return keyboard::typed(self, Some(keyboard::Typed::Char(c as char)));
            }
            Input::Erase if self.page == Page::Keyboard => return keyboard::typed(self, None),
            Input::Layout(_) => return Outcome::Nothing,
            _ => {}
        }
        match input {
            Input::Char(b' ') => return self.on(Input::Choose),
            Input::Char(_) | Input::Erase => return Outcome::Nothing,
            _ => {}
        }
        match input {
            Input::Up => {
                self.focus = if self.focus == 0 {
                    count - 1
                } else {
                    self.focus - 1
                };
                Outcome::Redraw
            }
            Input::Down => {
                self.focus = (self.focus + 1) % count;
                Outcome::Redraw
            }
            Input::Back => match self.page {
                Page::Welcome => self.show(Page::SkipConfirm),
                Page::SkipConfirm => self.show(Page::Welcome),
                page => self.show(self.previous(page)),
            },
            Input::Choose => self.choose(),
            Input::Char(_) | Input::Erase => Outcome::Nothing,
            #[cfg(feature = "keyboard-layouts")]
            Input::Layout(_) => Outcome::Nothing,
        }
    }

    fn type_char(&mut self, c: u8) -> Outcome {
        let t = self.tz.as_mut().expect("searching");
        if !(0x20..0x7f).contains(&c) || t.query.len() >= QUERY_MAX {
            return Outcome::Nothing;
        }
        t.query.push(c as char);
        self.research()
    }

    fn erase(&mut self) -> Outcome {
        let t = self.tz.as_mut().expect("searching");
        if t.query.pop().is_none() {
            return Outcome::Nothing;
        }
        self.research()
    }

    /// The list for the new search; the focus goes to its first row.
    fn research(&mut self) -> Outcome {
        let t = self.tz.as_mut().expect("searching");
        t.listed = if t.query.trim().is_empty() {
            t.zones.shortlist()
        } else {
            let mut found = t.zones.search(t.query.trim());
            found.truncate(4);
            found
        };
        self.focus = 0;
        Outcome::Redraw
    }

    fn choose(&mut self) -> Outcome {
        match (self.page, self.focus) {
            (Page::Welcome, 0) => self.show(self.next(Page::Welcome)),
            (Page::Welcome, _) => self.show(Page::SkipConfirm),
            (Page::SkipConfirm, 0) => Outcome::Done(Finish::Skipped),
            (Page::SkipConfirm, _) => self.show(Page::Welcome),
            (Page::Language, i) if LANGUAGE_PAGE && i < LANGUAGE_COUNT => {
                self.choices.lang = Lang::ALL[i];
                Outcome::Redraw
            }
            (Page::Language, _) => self.show(self.next(self.page)),
            (Page::Keyboard, i) => keyboard::choose(self, i),
            (Page::Network, _) => self.show(self.next(self.page)),
            (Page::TimeZone, i) if self.tz.is_some() => {
                let t = self.tz.as_ref().expect("checked");
                match t.listed.get(i) {
                    Some(&id) if !t.review => {
                        self.choices.tz = Some(id);
                        Outcome::Redraw
                    }
                    // Continue: confirm a new choice once; review, or no
                    // zone (the clock stays unconfirmed), just go on.
                    _ => match self.choices.tz {
                        Some(id) if !t.review => Outcome::Confirm(id),
                        _ => self.show(self.next(Page::TimeZone)),
                    },
                }
            }
            (Page::TimeZone, i) if i < ZONES.len() => {
                self.choices.zone = i;
                Outcome::Redraw
            }
            (Page::TimeZone, _) => self.show(self.next(Page::TimeZone)),
            (Page::Comfort, 0) => {
                self.choices.reduce_motion = !self.choices.reduce_motion;
                Outcome::Redraw
            }
            (Page::Comfort, 1) => {
                self.choices.reduce_transparency = !self.choices.reduce_transparency;
                Outcome::Redraw
            }
            (Page::Comfort, _) => self.show(self.next(Page::Comfort)),
            (Page::Try, _) => Outcome::Done(Finish::Try),
        }
    }

    fn show(&mut self, page: Page) -> Outcome {
        self.notice = None;
        self.page = page;
        self.focus = 0;
        Outcome::Page(page)
    }

    fn next(&self, page: Page) -> Page {
        let pages = self.pages();
        let i = pages.iter().position(|&p| p == page).unwrap_or(0);
        pages.get(i + 1).copied().unwrap_or(Page::Try)
    }

    fn previous(&self, page: Page) -> Page {
        let pages = self.pages();
        let i = pages.iter().position(|&p| p == page).unwrap_or(0);
        pages[i.saturating_sub(1)]
    }
}

fn mark(on: bool) -> &'static str {
    if on { "[x]" } else { "[ ]" }
}

fn on_off(on: bool) -> Msg {
    if on { Msg::On } else { Msg::Off }
}

// The KATs of S1, S3, S4 and S5 pin the build without `i18n-catalog`: the
// kill-switch, where every row is today's. `lang_tests` is the build with it.
// S4's KATs walk Begin straight to the Network page: the build without
// layouts, as S1's.
mod keyboard;
#[cfg(all(test, not(feature = "keyboard-layouts"), not(feature = "i18n-catalog")))]
mod net_tests;
#[cfg(feature = "keyboard-layouts")]
pub use keyboard::{Dead, FIELD_MAX, LAYOUT_COUNT, LAYOUT_MSGS, LAYOUT_NAMES, Typed};

#[cfg(all(test, feature = "keyboard-layouts", not(feature = "i18n-catalog")))]
mod keyboard_tests;
// S1's KATs are the build without layouts (the kill-switch: the page machine
// they pin is the one `keyboard-layouts` off ships).
#[cfg(all(test, not(feature = "keyboard-layouts"), not(feature = "i18n-catalog")))]
mod tests;
// S3's KATs walk Begin straight to the Time zone page, the build without
// layouts; `keyboard_tests` walks the Keyboard page into S3's search.
#[cfg(all(test, not(feature = "keyboard-layouts"), not(feature = "i18n-catalog")))]
mod tz_tests;
// S6b: the Language page, and the wizard in Spanish.
#[cfg(all(test, feature = "i18n-catalog"))]
mod lang_tests;
