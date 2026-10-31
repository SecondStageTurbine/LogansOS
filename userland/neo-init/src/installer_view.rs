// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.4: the Atrium installer (THINK §4.5; the PM's R4.4 decision: a
//! view of the WM, so the Installer cap never leaves neo-init). `install`
//! typed at Home opens it, as `help` opens the help card; it is drawn in
//! the same card under the field.
//!
//! - **The list:** a title row, one row per disk (at most [`DISK_ROWS`]),
//!   the buttons, and a hint row that says what the selected disk is or why
//!   a disk can't be chosen. Refused rows are shown and never selected.
//!   Keyboard: Up/Down move between the disks that can be chosen, Enter
//!   chooses, Esc goes back to Home. Pointer: click a disk, then Choose.
//! - **The confirmation:** one screen naming the disk (fresh installs only:
//!   no typed code, that is R4b's), then Install or Back.
//! - **The install:** `SYS_INSTALL_RUN`, which returns when it has finished
//!   or stopped; the card then says which, and any key or click ends it.
//!
//! The kernel decides every verdict (`installer_ui`); this shows its rows
//! and sends back the token of the one chosen, which the kernel re-checks.

#[cfg(feature = "desktop-session-boot-test")]
use super::Line as Log;
use super::{Rect, Scene, wm_rect};
use alloc::string::String;
use alloc::vec::Vec;
use libneo::install::{self, Row};
use libneo::{EAGAIN, Handle, INPUT_EVENT_V1_LEN, sys_input_focus_recv};
use neo_atrium::keys::{self, Key};
use neo_atrium::layout::{CARD_LINE_H, CARD_PAD_Y};

/// Disk rows on the card: with the title, the buttons and the hint, the
/// card's eight.
pub const DISK_ROWS: usize = 5;

/// The marks beside a disk: chosen, choosable, refused.
const SELECTED: &str = "> ";
const OFFERED: &str = "  ";
const REFUSED: &str = "x ";

/// What a press on the card hit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Hit {
    Disk(usize),
    /// The button row's first label (Choose, Install).
    Go,
    /// Its second label.
    Back,
    Outside,
}

/// What came in: a key, or a press on Home at (x, y).
enum Input {
    Key(Key),
    Press(usize, usize),
}

fn log(text: &[u8]) {
    #[cfg(feature = "desktop-session-boot-test")]
    Log::new().s(text).emit();
    #[cfg(not(feature = "desktop-session-boot-test"))]
    let _ = text;
}

fn slot(scene: &Scene) -> Rect {
    wm_rect(scene.chrome.layout.card_slot())
}

fn show(scene: &mut Scene, rows: Vec<String>) -> Result<(), i32> {
    // The old card's slot and the new one's: a shorter card leaves none of
    // the longer one behind.
    scene.chrome.set_card(rows);
    let r = slot(scene);
    scene.repaint_chrome(r);
    scene.present(false, r).map(|_| ()).map_err(|c| 100 + c)
}

/// The card row a press at `y` is on, if it is on the card at all.
fn card_row(scene: &Scene, x: usize, y: usize) -> Option<usize> {
    let card = scene.chrome.layout.card(scene.chrome.card_rows());
    if !card.contains(x, y) || y < card.y + CARD_PAD_Y {
        return None;
    }
    let row = (y - card.y - CARD_PAD_Y) / CARD_LINE_H;
    (row < scene.chrome.card_rows()).then_some(row)
}

/// Where a press landed on a card of `disks` disk rows (row 0 the title,
/// then the disks, then the buttons).
/// A button is hit only on its drawn label (`card_buttons`, measured with
/// the font the row is drawn in); the rest of the row hits nothing (#148).
fn hit(scene: &Scene, x: usize, y: usize, disks: usize) -> Hit {
    match card_row(scene, x, y) {
        Some(r) if r >= 1 && r <= disks => Hit::Disk(r - 1),
        Some(r) if r == disks + 1 => match scene.chrome.card_buttons(r) {
            Some([go, _]) if go.contains(x, y) => Hit::Go,
            Some([_, back]) if back.contains(x, y) => Hit::Back,
            _ => Hit::Outside,
        },
        Some(_) => Hit::Disk(usize::MAX),
        None => Hit::Outside,
    }
}

/// The next key or press on Home. The pointer is polled as Home's
/// (`intro::pointer_tick`): the sprite follows it, a press is reported.
fn next_input(scene: &mut Scene, events: Handle) -> Result<Input, i32> {
    use super::pointer::{self, Action, Ctx, Shape};
    loop {
        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        match sys_input_focus_recv(events, &mut event) {
            0 => {
                if let Some(key) = keys::decode(event) {
                    return Ok(Input::Key(key));
                }
                continue;
            }
            EAGAIN => {}
            _ => return Err(73),
        }
        if let Some(p) = scene.pointer.as_mut() {
            let (moved, press) = p.poll(&Ctx {
                home: true,
                ..Ctx::default()
            });
            let at = p.position();
            let _ = pointer::take_sak(false);
            if moved {
                let screen = scene.geo.screen;
                let mut damage = pointer::sprite_rect(at, Shape::Arrow, screen);
                if let Some(old) = scene.sprite_at {
                    damage = damage.union(&pointer::sprite_rect(old, scene.sprite_shape, screen));
                }
                scene.sprite_at = Some(at);
                scene.sprite_shape = Shape::Arrow;
                scene.present(false, damage).map_err(|c| 100 + c)?;
            }
            if let Some(Action::HomePress(x, y)) = press {
                return Ok(Input::Press(x, y));
            }
        }
        #[cfg(feature = "desktop-park")]
        scene.park(
            &[libneo::WaitItem {
                handle: events.0,
                kind: libneo::WAIT_KIND_INPUT,
            }],
            u64::MAX,
        );
        #[cfg(not(feature = "desktop-park"))]
        core::hint::spin_loop();
    }
}

fn disk_line(row: &Row, selected: bool) -> String {
    let mark = if !row.offered() {
        REFUSED
    } else if selected {
        SELECTED
    } else {
        OFFERED
    };
    let verdict = if row.offered() {
        install::action_text(row.action)
    } else {
        "not available"
    };
    alloc::format!(
        "{mark}{}  {}  {verdict}",
        install::name_text(row),
        install::size_text(row.capacity)
    )
}

/// The list card: the title, the disks, the buttons, the hint.
fn list_card(rows: &[Row], selected: Option<usize>) -> Vec<String> {
    let mut card = Vec::new();
    card.push(String::from("Install LogansOS: choose a disk"));
    for (i, row) in rows.iter().take(DISK_ROWS).enumerate() {
        card.push(disk_line(row, selected == Some(i)));
    }
    card.push(neo_atrium::intro::button_row("Choose"));
    card.push(match selected.map(|i| &rows[i]) {
        Some(row) => install::detail_text(row),
        None => String::from("No disk here can take a fresh install."),
    });
    card
}

/// The disks that can be chosen, in list order.
fn offered(rows: &[Row]) -> Vec<usize> {
    rows.iter()
        .take(DISK_ROWS)
        .enumerate()
        .filter(|(_, r)| r.offered())
        .map(|(i, _)| i)
        .collect()
}

/// Move the selection to the next (`down`) or previous choosable disk.
fn step(offered: &[usize], selected: Option<usize>, down: bool) -> Option<usize> {
    let at = selected.and_then(|s| offered.iter().position(|&o| o == s))?;
    let next = if down {
        (at + 1).min(offered.len() - 1)
    } else {
        at.saturating_sub(1)
    };
    Some(offered[next])
}

fn log_list(scene: &Scene, rows: &[Row], offered: &[usize], selected: Option<usize>) {
    // The card's place, for a scenario's pointer moves.
    #[cfg(feature = "desktop-session-boot-test")]
    {
        let card = scene.chrome.layout.card(scene.chrome.card_rows());
        Log::new()
            .s(b"DESKTOP installer card at=")
            .n(card.x as u64)
            .s(b",")
            .n(card.y as u64)
            .s(b" size=")
            .n(card.w as u64)
            .s(b"x")
            .n(card.h as u64)
            .s(b" row_h=")
            .n(CARD_LINE_H as u64)
            .s(b" pad=")
            .n(CARD_PAD_Y as u64)
            .s(b"\n")
            .emit();
    }
    #[cfg(feature = "desktop-session-boot-test")]
    Log::new()
        .s(b"DESKTOP installer list shown rows=")
        .n(rows.len().min(DISK_ROWS) as u64)
        .s(b" offered=")
        .n(offered.len() as u64)
        .s(b" selected=")
        .i(selected.map_or(-1, |s| s as i64))
        .s(b"\n")
        .emit();
    #[cfg(not(feature = "desktop-session-boot-test"))]
    let _ = (scene, rows, offered, selected);
}

fn log_selected(selected: usize, via: &[u8]) {
    #[cfg(feature = "desktop-session-boot-test")]
    Log::new()
        .s(b"DESKTOP installer selected row=")
        .n(selected as u64)
        .s(b" via=")
        .s(via)
        .s(b"\n")
        .emit();
    #[cfg(not(feature = "desktop-session-boot-test"))]
    let _ = (selected, via);
}

/// OT-87 R4.4b: at session start, show the Install chip where this boot
/// shows it ([`chip_shown`]).
pub fn start(scene: &mut Scene) {
    let shown = chip_shown();
    scene.chrome.set_install(shown);
    #[cfg(feature = "desktop-session-boot-test")]
    Log::new()
        .s(b"DESKTOP installer chip shown=")
        .n(u64::from(shown))
        .s(b"\n")
        .emit();
}

/// Whether the Install chip shows: the one place R5 changes. Until the Try
/// boot exists (THINK §6.1), it shows where this boot can install: the
/// list names a disk that isn't refused for the medium (the bundle present
/// and held to its pin). R5 makes it the Try boot's flag, and the chip's
/// action §6.1.2's explain-and-restart.
///
/// R5 rewire point. One LIST, once per session (from [`start`]): a
/// preflight per disk, each claim dropped unwritten. Not re-run when a disk
/// is added; the view's own LIST is.
fn chip_shown() -> bool {
    install::sys_install_list()
        .map(|rows| rows.iter().any(|r| r.refusal != install::refusal::MEDIUM))
        .unwrap_or(false)
}

/// The installer, from Home. Returns to Home (the card ended) when the user
/// goes back, or once the install's outcome has been read.
pub fn run(scene: &mut Scene, events: Handle) -> Result<(), i32> {
    let rows = match install::sys_install_list() {
        Ok(rows) => rows,
        Err(e) => {
            show(
                scene,
                alloc::vec![
                    alloc::format!("The installer isn't available here (error {e})."),
                    String::from("Press any key."),
                ],
            )?;
            log(b"DESKTOP installer unavailable\n");
            next_input(scene, events)?;
            return show(scene, Vec::new());
        }
    };
    let choosable = offered(&rows);
    let mut selected = choosable.first().copied();
    loop {
        show(scene, list_card(&rows, selected))?;
        log_list(scene, &rows, &choosable, selected);
        // The list: move, choose, or go back.
        let chosen = loop {
            match next_input(scene, events)? {
                Input::Key(Key::Escape) => {
                    log(b"DESKTOP installer back to=home via=esc\n");
                    return show(scene, Vec::new());
                }
                Input::Key(Key::Enter) => {
                    if let Some(s) = selected {
                        break s;
                    }
                }
                Input::Key(key @ (Key::Up | Key::Down | Key::Tab)) => {
                    let moved = step(&choosable, selected, key != Key::Up);
                    if moved != selected {
                        selected = moved;
                        show(scene, list_card(&rows, selected))?;
                    }
                    if let Some(s) = selected {
                        log_selected(s, b"key");
                    }
                }
                Input::Key(_) => {}
                Input::Press(x, y) => match hit(scene, x, y, rows.len().min(DISK_ROWS)) {
                    Hit::Disk(i) if choosable.contains(&i) => {
                        selected = Some(i);
                        show(scene, list_card(&rows, selected))?;
                        log_selected(i, b"click");
                    }
                    Hit::Disk(i) if i < rows.len() => {
                        // A refused row is shown, never chosen.
                        #[cfg(feature = "desktop-session-boot-test")]
                        Log::new()
                            .s(b"DESKTOP installer refused row=")
                            .n(i as u64)
                            .s(b" not selectable reason=\"")
                            .s(install::refusal_text(rows[i].refusal).as_bytes())
                            .s(b"\"\n")
                            .emit();
                        show(scene, {
                            let mut card = list_card(&rows, selected);
                            if let Some(last) = card.last_mut() {
                                *last = String::from(install::refusal_text(rows[i].refusal));
                            }
                            card
                        })?;
                    }
                    Hit::Go => {
                        if let Some(s) = selected {
                            break s;
                        }
                    }
                    Hit::Back => {
                        log(b"DESKTOP installer back to=home via=click\n");
                        return show(scene, Vec::new());
                    }
                    Hit::Disk(_) | Hit::Outside => {}
                },
            }
        };
        // The confirmation: Install or Back.
        let row = &rows[chosen];
        let action = install::action_text(row.action);
        show(
            scene,
            alloc::vec![
                alloc::format!("{action} LogansOS on {}?", install::name_text(row)),
                install::detail_text(row),
                String::new(),
                neo_atrium::intro::button_row(action),
                String::from("Enter goes ahead. Esc goes back."),
            ],
        )?;
        #[cfg(feature = "desktop-session-boot-test")]
        Log::new()
            .s(b"DESKTOP installer confirm shown row=")
            .n(chosen as u64)
            .s(b" action=")
            .s(action.as_bytes())
            .s(b"\n")
            .emit();
        let go = loop {
            match next_input(scene, events)? {
                Input::Key(Key::Enter) => break true,
                Input::Key(Key::Escape) => break false,
                Input::Key(_) => {}
                // The button row is the card's fourth.
                Input::Press(x, y) => match (card_row(scene, x, y), hit(scene, x, y, 2)) {
                    (Some(3), Hit::Go) => break true,
                    (Some(3), Hit::Back) => break false,
                    _ => {}
                },
            }
        };
        if !go {
            log(b"DESKTOP installer back to=list\n");
            continue;
        }
        show(
            scene,
            alloc::vec![
                alloc::format!("Installing LogansOS on {}.", install::name_text(row)),
                String::from("Don't remove the disk or the USB until it says it is done."),
            ],
        )?;
        log(b"DESKTOP installer installing\n");
        let status = install::sys_install_run(row);
        #[cfg(feature = "desktop-session-boot-test")]
        Log::new()
            .s(b"DESKTOP installer status=")
            .i(status)
            .s(b"\n")
            .emit();
        show(
            scene,
            alloc::vec![install::status_text(status), String::from("Press any key.")],
        )?;
        next_input(scene, events)?;
        return show(scene, Vec::new());
    }
}
