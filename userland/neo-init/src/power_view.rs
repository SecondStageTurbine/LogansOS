// SPDX-License-Identifier: MPL-2.0
//! OT-90 P5 (L1.4, `power-menu`): Shut down and Restart in the WM, behind a
//! confirm. `shut down` or `restart` typed at Home (`neo_atrium::power`)
//! opens it, as `help` opens the help card; it is drawn in the same card
//! under the field.
//!
//! - **The confirm card** names the action and its cost (open apps close),
//!   then the buttons. Enter or the first button goes ahead; Esc or Back
//!   returns to Home.
//! - **Going ahead** calls `SYS_POWER`, which only the session root's
//!   `PowerControl` cap allows. On 0 the kernel's clean shutdown has
//!   started (it closes the apps, drains NeoFS, stops the CPUs); the card
//!   says so and the WM waits for the power to go. On a refusal the card
//!   says why, and any key or click returns to Home.
//!
//! The card helpers are the installer's (`installer_view`), copied: that
//! view exists only with `installer`.

#[cfg(feature = "desktop-session-boot-test")]
use super::Line as Log;
use super::{Rect, Scene, wm_rect};
use alloc::string::String;
use alloc::vec::Vec;
use libneo::{EAGAIN, Handle, INPUT_EVENT_V1_LEN, sys_input_focus_recv};
use neo_atrium::keys::{self, Key};
use neo_atrium::layout::{CARD_LINE_H, CARD_PAD_Y};
use neo_atrium::power::{self, PowerCommand};

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

/// Whether a press hit the confirm card's first (`Some(true)`) or second
/// (`Some(false)`) button; only on the drawn labels (#148).
fn button(scene: &Scene, x: usize, y: usize) -> Option<bool> {
    if card_row(scene, x, y) != Some(power::CONFIRM_BUTTON_ROW) {
        return None;
    }
    match scene.chrome.card_buttons(power::CONFIRM_BUTTON_ROW) {
        Some([go, _]) if go.contains(x, y) => Some(true),
        Some([_, back]) if back.contains(x, y) => Some(false),
        _ => None,
    }
}

/// The next key or press on Home, the pointer polled as Home's.
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

/// The confirm card, then the call. Returns to Home on Back, any key but
/// Enter, or a refusal; on success it never returns (the power goes).
pub fn run(scene: &mut Scene, events: Handle, cmd: PowerCommand) -> Result<(), i32> {
    show(scene, power::confirm_lines(cmd))?;
    #[cfg(feature = "desktop-session-boot-test")]
    Log::new()
        .s(b"DESKTOP power confirm shown action=")
        .s(cmd.tag().as_bytes())
        .s(b"\n")
        .emit();
    let go = loop {
        match next_input(scene, events)? {
            Input::Key(key) => break power::confirm_key_goes_ahead(key),
            Input::Press(x, y) => {
                if let Some(go) = button(scene, x, y) {
                    break go;
                }
            }
        }
    };
    if !go {
        log(b"DESKTOP power back to=home\n");
        return show(scene, Vec::new());
    }
    show(scene, power::going_lines(cmd))?;
    // The harness's power judge looks for the scenario's request marker.
    #[cfg(feature = "desktop-session-boot-test")]
    if cmd == PowerCommand::ShutDown {
        log(b"DESKTOP power stage=power_off_requested\n");
    }
    let status = libneo::sys_power(cmd.action());
    #[cfg(feature = "desktop-session-boot-test")]
    Log::new()
        .s(b"DESKTOP power requested action=")
        .s(cmd.tag().as_bytes())
        .s(b" status=")
        .i(status)
        .s(b"\n")
        .emit();
    if status == 0 {
        // The kernel spares this process in K1 and stops its CPU in K4; the
        // card stays until the power goes.
        loop {
            core::hint::spin_loop();
        }
    }
    show(scene, power::refused_lines(status))?;
    next_input(scene, events)?;
    show(scene, Vec::new())
}
