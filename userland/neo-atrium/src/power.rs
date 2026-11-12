// SPDX-License-Identifier: MPL-2.0
//! OT-90 P5 (L1.4): Shut down and Restart at Home. Pure: which typed lines
//! ask for them, and the words of the confirm card. The WM answers them (as
//! it answers `help`); they never reach the model. neo-init's `power-menu`
//! draws the card and makes the call (`SYS_POWER`).

use alloc::string::String;
use alloc::vec::Vec;
use i18n_core::{Lang, Msg, text};

/// What the line asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerCommand {
    ShutDown,
    Restart,
}

impl PowerCommand {
    /// The `SYS_POWER` action (`libneo::power_abi`): 1 powers off, 2
    /// restarts.
    pub const fn action(self) -> u64 {
        match self {
            Self::ShutDown => 1,
            Self::Restart => 2,
        }
    }

    /// For the log: `shut-down`, `restart`.
    pub const fn tag(self) -> &'static str {
        match self {
            Self::ShutDown => "shut-down",
            Self::Restart => "restart",
        }
    }
}

/// The lines that ask, compared with surrounding spaces ignored, any ASCII
/// case, and runs of inner spaces as one.
const SHUT_DOWN: &[&str] = &["shut down", "shutdown", "power off", "turn off"];
const RESTART: &[&str] = &["restart", "reboot"];

/// Whether a line typed at Home asks to shut down or restart.
pub fn power_command(line: &[u8]) -> Option<PowerCommand> {
    let mut words = line
        .split(|&b| b == b' ')
        .filter(|w| !w.is_empty())
        .peekable();
    let mut norm: Vec<u8> = Vec::with_capacity(line.len());
    while let Some(word) = words.next() {
        norm.extend(word.iter().map(u8::to_ascii_lowercase));
        if words.peek().is_some() {
            norm.push(b' ');
        }
    }
    let is = |list: &[&str]| list.iter().any(|w| w.as_bytes() == norm.as_slice());
    if is(SHUT_DOWN) {
        Some(PowerCommand::ShutDown)
    } else if is(RESTART) {
        Some(PowerCommand::Restart)
    } else {
        None
    }
}

/// The confirm card's rows; the last is the button row
/// ([`crate::intro::button_row`]), whose first label confirms.
pub fn confirm_lines(cmd: PowerCommand) -> Vec<String> {
    confirm_lines_in(crate::lang::get(), cmd)
}

/// S6b: [`confirm_lines`] in `lang`. The trigger words stay as typed.
pub fn confirm_lines_in(lang: Lang, cmd: PowerCommand) -> Vec<String> {
    let (ask, go) = match cmd {
        PowerCommand::ShutDown => (Msg::PowerAskShutDown, Msg::PowerShutDown),
        PowerCommand::Restart => (Msg::PowerAskRestart, Msg::PowerRestart),
    };
    alloc::vec![
        String::from(text(lang, ask)),
        String::from(text(lang, Msg::PowerWarning)),
        String::from(text(lang, Msg::PowerConfirmHint)),
        crate::intro::button_row_in(lang, text(lang, go)),
    ]
}

/// What a key does on the confirm card (Jarvis's #14 review, item 1): Enter
/// goes ahead and every other key goes back, so only an explicit Enter (or
/// the drawn button) reaches `SYS_POWER`. Keys Home doesn't use, and
/// releases, never arrive here (`keys::decode` drops them).
pub fn confirm_key_goes_ahead(key: crate::keys::Key) -> bool {
    key == crate::keys::Key::Enter
}

/// The button row's index on the confirm card.
pub const CONFIRM_BUTTON_ROW: usize = 3;

/// The card once the call has been made.
pub fn going_lines(cmd: PowerCommand) -> Vec<String> {
    going_lines_in(crate::lang::get(), cmd)
}

/// S6b: [`going_lines`] in `lang`.
pub fn going_lines_in(lang: Lang, cmd: PowerCommand) -> Vec<String> {
    alloc::vec![String::from(text(
        lang,
        match cmd {
            PowerCommand::ShutDown => Msg::PowerShuttingDown,
            PowerCommand::Restart => Msg::PowerRestarting,
        }
    ))]
}

/// The card when the kernel refused (`errno`, negative): this computer
/// can't be powered off from here, or a shutdown is already under way.
pub fn refused_lines(errno: i64) -> Vec<String> {
    refused_lines_in(crate::lang::get(), errno)
}

/// S6b: [`refused_lines`] in `lang`.
pub fn refused_lines_in(lang: Lang, errno: i64) -> Vec<String> {
    let why = match errno {
        -16 => Msg::PowerBusy,
        -38 => Msg::PowerUnsupported,
        _ => Msg::PowerFailed,
    };
    alloc::vec![
        String::from(text(lang, why)),
        i18n_core::format(lang, Msg::PowerError, &[&alloc::format!("{errno}")]),
    ]
}
