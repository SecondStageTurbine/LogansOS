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
        let Some(input) = next_input(scene, focus)? else {
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
                line.s(b"\n").emit();
                return end(scene);
            }
        }
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
