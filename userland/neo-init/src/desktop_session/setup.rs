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

use super::home::HomeFocus;
use super::{Line, Rect, Scene, wm_rect};
use alloc::vec::Vec;
use libneo::{EAGAIN, INPUT_EVENT_V1_LEN, sys_input_focus_recv};
use neo_atrium::keys::{self, Key};
use setup_core::{Finish, Input, Outcome, Wizard, ZONES};

/// Run the wizard to its end. `Ok` when the session may go on.
pub fn run(scene: &mut Scene, focus: &HomeFocus) -> Result<(), i32> {
    let mut wizard = Wizard::new(cfg!(feature = "setup-stub-pages"));
    begin(scene)?;
    show(scene, &wizard)?;
    log_page(&wizard);
    loop {
        let Some(input) = next_input(scene, focus, &wizard)? else {
            continue;
        };
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
                #[cfg(feature = "keyboard-layouts")]
                keyboard::log_typed(&wizard, input);
            }
            Outcome::Page(_) => {
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
                let line = match finish {
                    Finish::Try => line
                        .s(b" zone=")
                        .s(ZONES[c.zone].as_bytes())
                        .s(b" reduce_motion=")
                        .n(u64::from(c.reduce_motion))
                        .s(b" reduce_transparency=")
                        .n(u64::from(c.reduce_transparency)),
                    Finish::Skipped => line,
                };
                #[cfg(feature = "keyboard-layouts")]
                let line = match finish {
                    Finish::Try => line.s(b" layout=").n(u64::from(c.layout)),
                    Finish::Skipped => line,
                };
                line.s(b"\n").emit();
                #[cfg(feature = "setup-store")]
                if finish == Finish::Try {
                    store(scene, &c);
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
        // One language exists (S6 adds the rest); S5's layouts are chosen.
        (Key::Language, 0),
        (Key::KeyboardLayout, layout(c)),
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

fn log_page(wizard: &Wizard) {
    Line::new()
        .s(b"DESKTOP setup page=")
        .s(wizard.page().name().as_bytes())
        .s(b" rows=")
        .n(wizard.rows().len() as u64)
        .s(b"\n")
        .emit();
}

/// The layout the wizard chose (S5), or US.
#[cfg(feature = "setup-store")]
fn layout(c: &setup_core::Choices) -> u32 {
    #[cfg(feature = "keyboard-layouts")]
    return u32::from(c.layout);
    #[cfg(not(feature = "keyboard-layouts"))]
    {
        let _ = c;
        0
    }
}

/// The next key the wizard uses, as its input (`None`: a key it ignores).
fn next_input(
    scene: &mut Scene,
    focus: &HomeFocus,
    #[cfg_attr(not(feature = "keyboard-layouts"), allow(unused_variables))] wizard: &Wizard,
) -> Result<Option<Input>, i32> {
    let mut event = [0u8; INPUT_EVENT_V1_LEN];
    match sys_input_focus_recv(focus.events, &mut event) {
        0 => {}
        EAGAIN => {
            idle(scene, focus);
            return Ok(None);
        }
        _ => return Err(190),
    }
    // S5: on the Keyboard page, keys are decoded in the layout chosen there,
    // and the field takes characters.
    #[cfg(feature = "keyboard-layouts")]
    if wizard.page() == setup_core::Page::Keyboard {
        return Ok(keyboard::input(wizard, event));
    }
    Ok(match keys::decode(event) {
        Some(Key::Up) => Some(Input::Up),
        Some(Key::Down | Key::Tab) => Some(Input::Down),
        Some(Key::Enter | Key::Char(b' ')) => Some(Input::Choose),
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

/// F11 S5: the Keyboard page's keys and its log.
#[cfg(feature = "keyboard-layouts")]
mod keyboard {
    use super::Line;
    use neo_atrium::keys::{self, Key};
    use setup_core::{Input, Wizard};

    /// A key on the Keyboard page, decoded in the layout chosen on the page:
    /// the moves and Enter/Space/Esc as on every page, Backspace and the
    /// other printable characters for the test field.
    pub fn input(wizard: &Wizard, event: [u8; libneo::INPUT_EVENT_V1_LEN]) -> Option<Input> {
        match keys::decode_in(wizard.choices().layout, event)? {
            Key::Up => Some(Input::Up),
            Key::Down | Key::Tab => Some(Input::Down),
            Key::Enter | Key::Char(b' ') => Some(Input::Choose),
            Key::Escape => Some(Input::Back),
            Key::Backspace => Some(Input::Erase),
            Key::Char(c) => Some(Input::Char(c)),
        }
    }

    /// After a character or Backspace: what the field holds, and in which
    /// layout the key was typed (the gate reads it).
    pub fn log_typed(wizard: &Wizard, input: Input) {
        let ascii = match input {
            Input::Char(c) => u64::from(c),
            Input::Erase => 8,
            _ => return,
        };
        Line::new()
            .s(b"DESKTOP setup typed layout=")
            .n(u64::from(wizard.choices().layout))
            .s(b" ascii=")
            .n(ascii)
            .s(b" field=")
            .s(wizard.field().as_bytes())
            .s(b"\n")
            .emit();
    }
}
