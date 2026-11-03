// SPDX-License-Identifier: MPL-2.0
//! The desktop session: `neo-init` resident as the compositor / window manager.
//!
//! `neo-init` stays resident as the trusted userland WM on Atrium G1
//! (desktop-UX synthesis points 1, 5, 6, 8): it owns one full-screen output
//! buffer, paints the chrome itself, lays its clients' ADR-073 surfaces out
//! (a client never learns its position), and composes the WHOLE scene into
//! that buffer before every present. Only the damaged rectangle is recomposed
//! and pushed, but the WM's base layer covers every pixel, so nothing an
//! earlier owner drew (the splash, a scenario's indicator, a stale client
//! rectangle) can survive the first frame. The WM keeps its own copy of each
//! client's committed pixels (S1-F2) and composites only that copy.
//!
//! **Input** is ADR-074 kernel routing. A client's keys reach only the focus
//! it holds; the WM holds Arthur's and the Lens's focus handles
//! ACTIVATE-only and never reads their keys (the anti-keylogger property).
//! Since DS-1 the WM has a focus of its own, which Home (the intent field,
//! the Spine chips) reads only while Home has the keyboard; every change of
//! owner is a `HANDOFF`: the route moves first, the new owner's queue is
//! emptied, and only then is the view presented. With `desktop-pointer` the
//! WM also claims the pointer route and forwards clicks in a client's area.
//!
//! **Layers.** Each feature adds one layer; each off is the layer below, bit
//! for bit (the kill-switches, held by the desktop scenarios):
//! - `desktop-session` (S1): flat chrome, Arthur as the one client;
//! - `desktop-glass` (S3): the `neo-atrium` scene — the Threshold backdrop,
//!   cached frosted glass, Cap-Halo chrome, Plex labels — or its
//!   Reduce-Transparency floor (`reduce-transparency`, or the preference
//!   file);
//! - `desktop-home` (DS-1a/b): Home and its intent field, Arthur held and
//!   resumed (`OP_YIELD_FOCUS`), the LaunchCap broker (`OP_LAUNCH`);
//! - `desktop-pointer` (DI-1a/b): the sprite, click-to-focus, the SAK, and
//!   moving the window by its title band;
//! - `desktop-lens` (DS-1c): the read-only Lens bloomed from Arthur's
//!   `list /`, switched to by its Spine chip;
//! - `desktop-resize` (DI-1b): edge and corner resizes through the client's
//!   configure mailbox;
//! - `desktop-full` (DL-1): all of the above, the Lens docked to the window;
//! - `desktop-motion` (DM-1): every view, Lens and focus change a short
//!   transition ending on its unchanged present (Reduce Motion:
//!   `reduce-motion`, or the preference file).
//!
//! Design: `docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md`, then per layer
//! the THINKs `atrium-glass-frost-amendment` (S3), `ds1-home-spine-shell`
//! (DS-1), `desktop-di1-pointer-decision` (DI-1), `dl1-docked-lens` and
//! `dm1-motion`; the scene's normative spec is `neo_atrium`'s crate doc.

#[cfg(not(feature = "desktop-glass"))]
use libneo::atrium_desktop::{self, DesktopLayout};
use libneo::atrium_protocol::{
    Header, OP_COMMIT, OP_COMMIT_DAMAGE, OP_CREATE_SURFACE, OP_DESTROY_SURFACE, OP_FOCUS_REQUEST,
    OP_IMPORT_ATTACH_BUFFER, REPLY_LEN, ROLE_TEXT, Reply, Request, SurfaceRegistry, decode_request,
    encode_reply,
};
use libneo::atrium_wm::{Blend, Layer, Rect, SharedPixels, compose_layers_xrgb8888};
use libneo::{
    ATRIUM_FOCUS_ACTIVATE, ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, BUFFER_DESC_V1_LEN,
    CapRights, EPERM, FORMAT_XRGB8888_LE, FULL_CHANNEL_RIGHTS, FileRights, Handle,
    INPUT_EVENT_V1_LEN, OPEN_ROOT_TEMPORARY, PRESENT_RESULT_V1_LEN, encode_buffer_desc,
    encode_damage_rect, meta_rights, sys_buffer_commit_damage, sys_buffer_create, sys_buffer_map,
    sys_buffer_query, sys_chan_create, sys_close, sys_fb_output_query, sys_fb_present, sys_grant,
    sys_input_focus_recv, sys_open, sys_present_fence_wait, sys_recv, sys_recv_grant, sys_send,
    sys_wait, sys_write,
};
#[cfg(feature = "desktop-home")]
use neo_atrium::motion::Kind;

/// The surface namespace Arthur's client names in `OP_CREATE_SURFACE`
/// (`neo-arthur` `showcase_surface::NAMESPACE_ID`).
const NS: u64 = 0x4152_5448_5552_0001;
const SURFACE_CAP_PROVENANCE: u64 = 0x5355_5246_4445_0001;
/// Arthur's buffer provenance is fixed by its client (`showcase_surface`).
const BUFFER_CAP_PROVENANCE: u64 = 0x4255_4646_4152_0001;
/// Arthur's TEXT surface geometry (`neo-arthur` `showcase_logic`).
const CLIENT_W: usize = 640;
const CLIENT_H: usize = 400;
/// Arthur's footer rule, this far above the client's bottom edge
/// (`neo-arthur` `showcase_text`): below it is only the footer line.
#[cfg(feature = "desktop-session-boot-test")]
const ARTHUR_FOOTER_RULE_FROM_BOTTOM: usize = 36;
/// The buffer Arthur shares: exactly its surface, or (DI-1b resize) one
/// sized for the largest client a resize can configure, created 640x400.
#[cfg(not(feature = "desktop-resize"))]
const BUFFER_W: usize = CLIENT_W;
#[cfg(not(feature = "desktop-resize"))]
const BUFFER_H: usize = CLIENT_H;
#[cfg(feature = "desktop-resize")]
const BUFFER_W: usize = neo_atrium::layout::MAX_CLIENT_W;
#[cfg(feature = "desktop-resize")]
const BUFFER_H: usize = neo_atrium::layout::MAX_CLIENT_H;
const BUFFER_STRIDE: usize = BUFFER_W * 4;
/// `desktop_home_cycle`'s silent rounds: past the test's pin cap of 8.
#[cfg(feature = "desktop-home-cycle-test")]
const DESKTOP_HOME_CYCLE_ROUNDS: u32 = 10;
const BUFFER_BYTES: u64 = (BUFFER_STRIDE * BUFFER_H) as u64;
/// DI-1b resize: the focus-light plane, sized for the largest frame.
#[cfg(feature = "desktop-resize")]
const GLOW_PLANE_W: usize = BUFFER_W + 2 * neo_atrium::layout::PAD + 2 * neo_atrium::layout::GLOW;
#[cfg(feature = "desktop-resize")]
const GLOW_PLANE_H: usize =
    BUFFER_H + neo_atrium::layout::TITLE_H + neo_atrium::layout::PAD + 2 * neo_atrium::layout::GLOW;
#[cfg(feature = "desktop-session-boot-test")]
const ANSWER_SERIAL: u64 = 0x4152_5448_5552_0001;
const MEMORY_DIR: &str = "/neofs/.arthur/memory";

/// A WM-owned pixel store: a G1 buffer the WM maps RW and never presents.
struct Plane {
    pixels: *mut u32,
    len: usize,
}

impl Plane {
    fn create(width: usize, height: usize) -> Result<(Handle, Self), i32> {
        let stride = width.checked_mul(4).ok_or(1)?;
        let bytes = stride.checked_mul(height).ok_or(1)?;
        let desc = encode_buffer_desc(
            width as u32,
            height as u32,
            stride as u32,
            FORMAT_XRGB8888_LE,
            bytes as u64,
        );
        let handle = sys_buffer_create(&desc).map_err(|_| 2)?;
        let pixels = sys_buffer_map(handle, 0).map_err(|_| 3)? as *mut u32;
        Ok((
            handle,
            Self {
                pixels,
                len: width * height,
            },
        ))
    }

    fn slice(&self) -> &[u32] {
        // SAFETY: the client-role mapping covers exactly `len` u32 pixels
        // (immutable descriptor, tightly packed) for the life of the process.
        unsafe { core::slice::from_raw_parts(self.pixels, self.len) }
    }

    fn slice_mut(&mut self) -> &mut [u32] {
        // SAFETY: as `slice`; the WM is the only writer of its own planes.
        unsafe { core::slice::from_raw_parts_mut(self.pixels, self.len) }
    }
}

/// DI-1b: frame re-blurs done by completed window moves and resizes (one
/// each, Glass; DL-1: two with the Lens docked, and the one a Lens shown
/// after a move costs).
#[cfg(feature = "desktop-pointer")]
static MOVE_BLURS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// DI-1b / DL-1: run a frame change, counting the blurs it costs as move
/// blurs (never a re-blur during typing).
#[cfg(feature = "desktop-pointer")]
fn count_move_blurs<T>(change: impl FnOnce() -> T) -> T {
    let before = neo_atrium::blur::blur_calls();
    let out = change();
    let spent = neo_atrium::blur::blur_calls() - before;
    MOVE_BLURS.fetch_add(spent, core::sync::atomic::Ordering::Relaxed);
    out
}

#[cfg(feature = "desktop-glass")]
fn move_blurs() -> u32 {
    #[cfg(feature = "desktop-pointer")]
    return MOVE_BLURS.load(core::sync::atomic::Ordering::Relaxed);
    #[cfg(not(feature = "desktop-pointer"))]
    0
}

/// What the session loop needs to know about the scene, whatever paints it.
#[derive(Clone, Copy)]
struct Geometry {
    screen: Rect,
    client: Rect,
    glow: Rect,
    /// Repainted (and re-presented) when focus changes.
    focus_damage: Rect,
}

/// The chrome painter: S1's flat chrome, or S3's glass (kill-switch).
#[cfg(not(feature = "desktop-glass"))]
type Chrome = DesktopLayout;
#[cfg(feature = "desktop-glass")]
type Chrome = neo_atrium::BaseScene;

#[cfg(not(feature = "desktop-glass"))]
fn geometry(layout: &DesktopLayout) -> Geometry {
    Geometry {
        screen: layout.screen,
        client: layout.client,
        glow: layout.glow,
        focus_damage: layout.glow.union(&layout.frame).union(&layout.spine),
    }
}

/// A neo-atrium layout rect as the WM's damage rect (the same four fields).
#[cfg(feature = "desktop-glass")]
fn wm_rect(r: neo_atrium::Rect) -> Rect {
    Rect::new(r.x, r.y, r.w, r.h)
}

/// The WM's damage rect as a neo-atrium layout rect.
#[cfg(feature = "desktop-glass")]
fn atrium_rect(r: Rect) -> neo_atrium::Rect {
    neo_atrium::Rect::new(r.x, r.y, r.w, r.h)
}

#[cfg(feature = "desktop-glass")]
fn geometry(scene: &neo_atrium::BaseScene) -> Geometry {
    let r = wm_rect;
    let l = &scene.layout;
    Geometry {
        screen: r(l.screen),
        client: r(l.client),
        glow: r(l.glow),
        focus_damage: r(l.focus_damage()),
    }
}

struct Scene {
    output: Handle,
    out: Plane,
    base: Plane,
    /// The WM's own copy of the client's committed pixels (S1-F2): the only
    /// client pixels a layer ever points at. Filled from the client's shared
    /// mapping by [`SharedPixels::snapshot`], one committed rect at a time.
    client_copy: Plane,
    glow: Option<Plane>,
    /// The focus-light plane's stride (its own width).
    glow_stride: usize,
    chrome: Chrome,
    geo: Geometry,
    focused: bool,
    /// DS-1: `Some` while the base shows the Home view.
    #[cfg(feature = "desktop-home")]
    home: Option<home::HomeState>,
    /// DI-1a: the WM's pointer (`None`: no route, ADR-074 mouse).
    #[cfg(feature = "desktop-pointer")]
    pointer: Option<pointer::Pointer>,
    /// Where the sprite is drawn on the output (window view only), and
    /// which one.
    #[cfg(feature = "desktop-pointer")]
    sprite_at: Option<(i32, i32)>,
    #[cfg(feature = "desktop-pointer")]
    sprite_shape: pointer::Shape,
    /// DI-1b: the drag outline (where a title-band drag would drop the
    /// frame), drawn over the composite like the sprite.
    #[cfg(feature = "desktop-pointer")]
    outline: Option<Rect>,
    /// Pixels the next present must recompose (a sprite being erased).
    #[cfg(feature = "desktop-pointer")]
    extra_damage: Rect,
    /// DI-1b resize: this session's configure mailbox and the configure it
    /// waits on (`None` until a client is up).
    #[cfg(feature = "desktop-resize")]
    configure: Option<resize::Configure>,
    /// DS-1c: the WM's copy of the Lens's committed pixels (copied through
    /// `SharedPixels`, as for Arthur: S1-F2), composited while the Lens is
    /// shown. Chad DS-1c F1: with the buffer the WM lends every Lens, made
    /// once at the first bloom and kept — a bloom, failed or not, maps and
    /// keeps nothing new.
    #[cfg(feature = "desktop-lens")]
    lens: Option<lens::LensStore>,
    /// DM-1: the transition's two ends, `F` (what was on screen) and `T`
    /// (the state change composed), output-sized, made once at boot.
    #[cfg(feature = "desktop-motion")]
    motion_from: Plane,
    #[cfg(feature = "desktop-motion")]
    motion_to: Plane,
    #[cfg(feature = "desktop-motion")]
    motion_profile: neo_atrium::motion::Profile,
    /// Boot test: the Home focus the next Home ↔ window transition holds on,
    /// at its mid frame, until a key arrives (the scenario's capture).
    #[cfg(feature = "desktop-motion-boot-test")]
    motion_hold: Option<Handle>,
    /// `desktop-intro`: which card Home shows, and whether NeoFS persists.
    #[cfg(feature = "desktop-intro")]
    intro: intro::State,
    /// `desktop-clock` (OT-85): the readout's minute and the next refresh.
    #[cfg(feature = "desktop-clock")]
    clock: clock::State,
    /// A9-D: the busy emblem Arthur asked for (`OP_BUSY`), until its next
    /// commit.
    #[cfg(feature = "desktop-busy")]
    busy: Option<busy::State>,
    /// OT-66 L2: the lock screen (its plane, field and backoff).
    #[cfg(feature = "login")]
    lock: lock::Screen,
}

impl Scene {
    fn new(chrome: Chrome) -> Result<Self, i32> {
        let geo = geometry(&chrome);
        let (output, out) = Plane::create(geo.screen.w, geo.screen.h).map_err(|c| 10 + c)?;
        let (_base_handle, base) = Plane::create(geo.screen.w, geo.screen.h).map_err(|c| 20 + c)?;
        let (_copy_handle, client_copy) = Plane::create(BUFFER_W, BUFFER_H).map_err(|c| 50 + c)?;
        // DI-1b resize: one plane for the largest frame's focus light.
        #[cfg(feature = "desktop-resize")]
        let (glow_w, glow_h) = (GLOW_PLANE_W, GLOW_PLANE_H);
        #[cfg(not(feature = "desktop-resize"))]
        let (glow_w, glow_h) = (geo.glow.w, geo.glow.h);
        let glow = if geo.glow.is_empty() {
            None
        } else {
            Some(Plane::create(glow_w, glow_h).map_err(|c| 30 + c)?.1)
        };
        let mut scene = Self {
            output,
            out,
            base,
            client_copy,
            glow,
            glow_stride: glow_w,
            chrome,
            geo,
            focused: false,
            #[cfg(feature = "desktop-home")]
            home: None,
            #[cfg(feature = "desktop-pointer")]
            pointer: None,
            #[cfg(feature = "desktop-pointer")]
            sprite_at: None,
            #[cfg(feature = "desktop-pointer")]
            sprite_shape: pointer::Shape::Arrow,
            #[cfg(feature = "desktop-pointer")]
            outline: None,
            #[cfg(feature = "desktop-pointer")]
            extra_damage: Rect::default(),
            #[cfg(feature = "desktop-resize")]
            configure: None,
            #[cfg(feature = "desktop-lens")]
            lens: Some(lens::LensStore::new()?),
            #[cfg(feature = "desktop-motion")]
            motion_from: Plane::create(geo.screen.w, geo.screen.h)
                .map_err(|c| 60 + c)?
                .1,
            #[cfg(feature = "desktop-motion")]
            motion_to: Plane::create(geo.screen.w, geo.screen.h)
                .map_err(|c| 64 + c)?
                .1,
            #[cfg(feature = "desktop-motion")]
            motion_profile: motion::profile(),
            #[cfg(feature = "desktop-motion-boot-test")]
            motion_hold: None,
            #[cfg(feature = "desktop-intro")]
            intro: intro::State::default(),
            #[cfg(feature = "desktop-clock")]
            clock: clock::State::default(),
            #[cfg(feature = "desktop-busy")]
            busy: None,
            #[cfg(feature = "login")]
            lock: lock::Screen::default(),
        };
        scene.repaint_chrome(geo.screen);
        Ok(scene)
    }

    /// DI-1a: drain the pointer route in the Home view (no sprite there:
    /// Home is keyboard-driven). Every record is still consumed once.
    #[cfg(feature = "desktop-pointer")]
    fn pointer_idle(&mut self) {
        let Some(pointer) = self.pointer.as_mut() else {
            return;
        };
        let _ = pointer.poll(&pointer::Ctx::default());
        let _ = pointer::take_sak(false);
    }

    /// OT-22: instead of one spin iteration, park until one of `extra`
    /// (channels or input leaves), the pointer route, or the SAK latch is
    /// ready, or `deadline_ns` (absolute monotonic) passes. The caller then
    /// polls exactly as before (the wait is level-triggered, its mask
    /// unused). A failed wait (EAGAIN: no park slot) costs one spin.
    #[cfg(feature = "desktop-park")]
    fn park(&self, extra: &[libneo::WaitItem], deadline_ns: u64) {
        use libneo::{WAIT_ITEMS_MAX, WAIT_KIND_INPUT, WAIT_KIND_SAK, WaitItem};
        let mut items = [WaitItem::default(); WAIT_ITEMS_MAX];
        let mut n = 0;
        for item in extra.iter().take(WAIT_ITEMS_MAX - 2) {
            items[n] = *item;
            n += 1;
        }
        if let Some(pointer) = self.pointer.as_ref() {
            items[n] = WaitItem {
                handle: pointer.route().0,
                kind: WAIT_KIND_INPUT,
            };
            items[n + 1] = WaitItem {
                handle: 0,
                kind: WAIT_KIND_SAK,
            };
            n += 2;
        }
        if libneo::sys_wait_any(&items[..n], deadline_ns) == libneo::EAGAIN {
            core::hint::spin_loop();
        }
    }

    /// DI-1a: hide the sprite; the next present recomposes where it was.
    #[cfg(feature = "desktop-pointer")]
    fn hide_sprite(&mut self) {
        if let Some(at) = self.sprite_at.take() {
            self.extra_damage = pointer::sprite_rect(at, self.sprite_shape, self.geo.screen);
        }
        if let Some(outline) = self.outline.take() {
            self.extra_damage = self.extra_damage.union(&outline);
        }
    }

    /// DI-1a, window view: one pointer/SAK tick. Moves the sprite (damage =
    /// old rect ∪ new rect) and applies a press's or chord's action.
    #[cfg(feature = "desktop-pointer")]
    fn pointer_tick(
        &mut self,
        arthur: Handle,
        wm_focus: &home::HomeFocus,
        escaped: &mut bool,
    ) -> Result<(), i32> {
        // OT-66 L2: locked (or locking now), the lock screen has the turn.
        #[cfg(feature = "login")]
        if lock::service(self)? {
            return Ok(());
        }
        let Some(pointer_state) = self.pointer.as_mut() else {
            return Ok(());
        };
        let layout = self.chrome.layout;
        let r = wm_rect;
        // DL-1 (`desktop-full`): a shown Lens is docked to the frame and
        // moves and resizes with it (`Layout::clamp_frame` / `resized` keep
        // the group inside), so the title band and the handles stay live.
        // Kill-switch: without resize (`desktop-lens-pointer` alone) the
        // window stays put while a Lens is up, exactly as DS-1c x DI-1b.
        let window_fixed = cfg!(all(
            feature = "desktop-lens",
            not(feature = "desktop-resize")
        )) && self.chrome.lens_shown();
        let ctx = pointer::Ctx {
            client: self.geo.client,
            frame: r(layout.frame),
            spine: r(layout.spine),
            title: if window_fixed {
                Rect::default()
            } else {
                r(layout.title_band())
            },
            arthur_focused: !*escaped,
            activator: (!*escaped).then_some(arthur),
            #[cfg(feature = "desktop-resize")]
            resize: (!window_fixed && self.configure.as_ref().is_none_or(|c| c.pending.is_none()))
                .then_some(layout),
            #[cfg(feature = "desktop-lens")]
            chip: r(layout.chip),
            #[cfg(feature = "desktop-lens")]
            lens_chip: if self.chrome.lens_shown() && !self.chrome.lens_selected() {
                r(layout.chip2)
            } else {
                Rect::default()
            },
            #[cfg(feature = "desktop-intro")]
            home: false,
        };
        let (moved, press) = pointer_state.poll(&ctx);
        let at = pointer_state.position();
        let shape = pointer_state.shape(&ctx);
        // DI-1b: the outline follows a held drag (where the frame would
        // land); a release keeps it until the move itself repaints (no
        // erase-then-move flicker), or (resize) until the client acks.
        #[cfg(feature = "desktop-resize")]
        let keep = matches!(
            press,
            Some(pointer::Action::Move(..) | pointer::Action::Resize(..))
        ) || self.configure.as_ref().is_some_and(|c| c.pending.is_some());
        #[cfg(not(feature = "desktop-resize"))]
        let keep = matches!(press, Some(pointer::Action::Move(..)));
        let outline = match pointer_state.drag() {
            _ if keep => self.outline,
            Some(pointer::Drag::Move(dx, dy)) if (dx, dy) != (0, 0) => {
                let f = layout.frame;
                let (x, y) = layout.clamp_frame(f.x as i64 + dx as i64, f.y as i64 + dy as i64);
                Some(Rect::new(x, y, f.w, f.h))
            }
            #[cfg(feature = "desktop-resize")]
            Some(pointer::Drag::Resize(edges, dx, dy)) if (dx, dy) != (0, 0) => {
                let (x, y, cw, ch) = layout.resized(edges, dx as i64, dy as i64);
                let (fw, fh) = (
                    cw + 2 * neo_atrium::layout::PAD,
                    ch + neo_atrium::layout::TITLE_H + neo_atrium::layout::PAD,
                );
                Some(Rect::new(x, y, fw, fh))
            }
            _ => None,
        };
        let screen = self.geo.screen;
        let mut damage = Rect::default();
        if moved || shape != self.sprite_shape || self.sprite_at.is_none() {
            damage = pointer::sprite_rect(at, shape, screen);
            if let Some(old) = self.sprite_at {
                damage = damage.union(&pointer::sprite_rect(old, self.sprite_shape, screen));
            }
            self.sprite_at = Some(at);
            self.sprite_shape = shape;
        }
        if outline != self.outline {
            for edge in [self.outline, outline].into_iter().flatten() {
                damage = damage.union(&edge);
            }
            self.outline = outline;
        }
        if !damage.is_empty() {
            self.present(true, damage).map_err(|c| 100 + c)?;
        }
        let action = press.or_else(|| pointer::take_sak(!*escaped)).or_else(|| {
            if *escaped {
                pointer::escaped_keys(wm_focus.events)
            } else {
                None
            }
        });
        if let Some(action) = action {
            pointer::apply(self, action, arthur, wm_focus, escaped)?;
        }
        Ok(())
    }

    /// DI-1b: move the window by a title-band drag of `(dx, dy)` (clamped
    /// to the workfield). The client is not told: its surface and pixels are
    /// window-relative, the coordinates it receives are window-relative and
    /// clamped to its surface (so even a grab dragged off-window cannot reveal
    /// where the window is), and nothing else it is sent carries a position.
    /// Repaints the place the frame left and the
    /// place it lands from the base (one blur, in `move_frame`) and presents
    /// both with the drag outline erased.
    #[cfg(feature = "desktop-pointer")]
    fn move_window(&mut self, dx: i32, dy: i32) -> Result<(), i32> {
        let l = self.chrome.layout;
        let (x, y) = l.clamp_frame(l.frame.x as i64 + dx as i64, l.frame.y as i64 + dy as i64);
        let old = self.reach();
        let mut damage = self.outline.take().unwrap_or_default();
        let moved = count_move_blurs(|| self.chrome.move_frame(x, y));
        if moved {
            self.geo = geometry(&self.chrome);
            damage = damage.union(&self.repaint_reframed(old));
        }
        if !damage.is_empty() {
            self.present(true, damage).map_err(|c| 100 + c)?;
        }
        // After the present: the witness reads what was shown.
        if moved {
            glass::log_move(self, (l.frame.x, l.frame.y), old);
        }
        Ok(())
    }

    /// DI-1b resize: a drag of the frame's `edges` released `(dx, dy)`
    /// from its press. The WM decides the size (bounded, see
    /// [`neo_atrium::Layout::resized`]) and publishes it in the client's
    /// configure mailbox; the outline stays until the client acks
    /// ([`Self::apply_configure`]). Nothing the client is told carries a
    /// position. A drag that changes nothing just erases the outline.
    #[cfg(feature = "desktop-resize")]
    fn request_resize(&mut self, edges: u8, dx: i32, dy: i32) -> Result<(), i32> {
        let l = self.chrome.layout;
        let (x, y, w, h) = l.resized(edges, dx as i64, dy as i64);
        let unchanged = (x, y, w, h) == (l.frame.x, l.frame.y, l.client.w, l.client.h);
        match self.configure.as_mut() {
            Some(configure) if !unchanged && configure.pending.is_none() => {
                configure.publish(x, y, w, h);
                resize::log_request(edges, w, h, configure.pending.map_or(0, |p| p.serial));
                Ok(())
            }
            _ => {
                let damage = self.outline.take().unwrap_or_default();
                if damage.is_empty() {
                    return Ok(());
                }
                self.present(true, damage).map(|_| ()).map_err(|c| 100 + c)
            }
        }
    }

    /// DI-1b resize: the client acked a configure (`OP_CONFIGURE` on its own
    /// surface channel): its buffer now holds that size's picture. Honoured
    /// only if it is exactly the outstanding configure (serial and size — a
    /// client cannot size itself); then the registry bounds its damage by the
    /// new size, the frame takes it (one region blur, as a move), the base is
    /// repainted where the frame was and is, the new picture is copied and
    /// everything is presented with the outline erased.
    #[cfg(feature = "desktop-resize")]
    fn apply_configure(
        &mut self,
        ack: libneo::atrium_protocol::ConfigureSurface,
        registry: &mut SurfaceRegistry,
        identity: libneo::atrium_protocol::Identity,
        client: &SharedPixels,
    ) -> Result<(), Fault> {
        use libneo::atrium_protocol::ProtocolError;
        // An ack with nothing published, or not of what was published, is
        // the client's error: refused, the configure still pending. Past
        // the acks check every failure is the WM's own.
        let pending = self
            .configure
            .as_ref()
            .and_then(|c| c.pending)
            .ok_or(Fault::Client(ProtocolError::BadState.status()))?;
        let published = libneo::atrium_configure::Published {
            serial: pending.serial,
            width: pending.w as u32,
            height: pending.h as u32,
        };
        if ack.identity != identity {
            return Err(Fault::Client(ProtocolError::CapRequired.status()));
        }
        if !libneo::atrium_configure::acks(published, ack.width, ack.height, ack.configure_serial) {
            return Err(Fault::Client(ProtocolError::BadField.status()));
        }
        if let Some(configure) = self.configure.as_mut() {
            configure.pending = None;
        }
        let from = (self.geo.client.w, self.geo.client.h);
        let old = self.reach();
        let mut damage = self.outline.take().unwrap_or_default();
        // The frame first, then the registry's damage bound, so they never
        // disagree (a size the frame cannot take is refused, not half-applied).
        // DL-1 AAG: a configure published before a Lens bloomed (its origin
        // computed with no Lens docked, and maybe before the bloom's slide)
        // is placed where the docked group fits at the acked size; the size
        // is the client's, the place is only ever the WM's.
        let (x, y) = if self.chrome.lens_shown() {
            self.chrome.layout.clamp_frame_for(
                pending.w,
                pending.h,
                pending.x as i64,
                pending.y as i64,
            )
        } else {
            (pending.x, pending.y)
        };
        // DL1-X1 fix-forward: a refit to the docked maximum can be exactly the
        // frame as it is (Arthur already at that size, at the clamp); the ack
        // then changes nothing to place, where `place_frame` would refuse.
        let l = self.chrome.layout;
        let unchanged = (x, y) == (l.frame.x, l.frame.y) && (pending.w, pending.h) == from;
        if !unchanged && !count_move_blurs(|| self.chrome.place_frame(x, y, pending.w, pending.h)) {
            return Err(Fault::Wm(92));
        }
        registry
            .configure_on_channel(ack, identity.surface_cap_provenance)
            .map_err(|_| Fault::Wm(92))?;
        self.geo = geometry(&self.chrome);
        damage = damage.union(&self.repaint_reframed(old));
        if let Some(glow) = self.glow.as_mut() {
            self.chrome
                .fill_glow_strided(glow.slice_mut(), self.glow_stride, self.focused);
        }
        let c = self.geo.client;
        damage = damage.union(&self.take_client(client, Rect::new(0, 0, c.w, c.h)));
        self.present(true, damage).map_err(|c| Fault::Wm(100 + c))?;
        resize::log_applied(self, from, pending.serial, old, client);
        Ok(())
    }

    /// DI-1b resize: a new client is created at CLIENT_W x CLIENT_H, so a
    /// window resized in an earlier session goes back to that size (same
    /// place) before the client's first commit. In the Home view only the
    /// geometry changes (the view switch repaints the window's regions).
    #[cfg(feature = "desktop-resize")]
    fn reset_client_size(&mut self) -> Result<(), i32> {
        self.configure = None;
        let l = self.chrome.layout;
        let old = self.reach();
        if !count_move_blurs(|| {
            self.chrome
                .place_frame(l.frame.x, l.frame.y, CLIENT_W, CLIENT_H)
        }) {
            return Ok(());
        }
        self.geo = geometry(&self.chrome);
        if self.home.is_some() {
            return Ok(());
        }
        let damage = self.repaint_reframed(old);
        if let Some(glow) = self.glow.as_mut() {
            self.chrome
                .fill_glow_strided(glow.slice_mut(), self.glow_stride, self.focused);
        }
        self.present(false, damage).map(|_| ()).map_err(|c| 100 + c)
    }

    /// DI-1b / DL-1: what a frame change repaints, captured before it: the
    /// focus light's reach and (docked) the Lens halo's.
    #[cfg(feature = "desktop-pointer")]
    fn reach(&self) -> [Rect; 2] {
        [self.geo.glow, wm_rect(self.chrome.layout.docked_reach())]
    }

    /// Repaint the base where the frame (and a docked Lens) was and is, in
    /// the window view (never while Home shows: moves, resizes and slides
    /// happen only in the window view); returns the union to present.
    #[cfg(feature = "desktop-pointer")]
    fn repaint_reframed(&mut self, old: [Rect; 2]) -> Rect {
        debug_assert!(self.home.is_none(), "a frame change painted over Home");
        let stride = self.geo.screen.w;
        let mut damage = Rect::default();
        for region in old.into_iter().chain(self.reach()) {
            if region.is_empty() {
                continue;
            }
            self.chrome.paint_rect(
                self.base.slice_mut(),
                stride,
                self.focused,
                atrium_rect(region),
            );
            damage = damage.union(&region);
        }
        damage
    }

    /// DL-1: before a bloom, slide the window (one move) to where the Lens
    /// fits beside it, if it does not where it is. `Ok` either way: no room
    /// at all is the bloom's own refusal.
    #[cfg(all(feature = "desktop-lens", feature = "desktop-resize"))]
    fn slide_for_lens(&mut self) -> Result<(), i32> {
        let l = self.chrome.layout;
        if l.lens.is_some() {
            return Ok(());
        }
        let Some((x, y)) = l.bloom_origin() else {
            return Ok(());
        };
        self.move_window(x as i32 - l.frame.x as i32, y as i32 - l.frame.y as i32)?;
        lens::log_slide((l.frame.x, l.frame.y), &self.chrome.layout);
        Ok(())
    }

    /// DL-1 fix-forward (Janus DL1-X1): after a bloom, a configure published
    /// while the Lens proof was polled was sized with no Lens docked; a size
    /// the docked group cannot hold is re-published at the docked maximum
    /// (a new serial). Arthur is still parked on its `OP_LAUNCH` reply, so it
    /// reads only the refit, and [`Self::apply_configure`] can always place
    /// the ack inside the group (the DL-1 THINK's stale-configure closure).
    #[cfg(all(feature = "desktop-lens", feature = "desktop-resize"))]
    fn refit_pending_for_dock(&mut self) {
        let (max_w, max_h) = self.chrome.layout.docked_max_client();
        let Some(configure) = self.configure.as_mut() else {
            return;
        };
        let Some(p) = configure.pending else {
            return;
        };
        if p.w <= max_w && p.h <= max_h {
            return;
        }
        let (w, h) = (p.w.min(max_w), p.h.min(max_h));
        configure.publish(p.x, p.y, w, h);
        let serial = configure.pending.map_or(0, |n| n.serial);
        resize::log_refit((p.w, p.h), (w, h), serial);
    }

    /// DM-1: make a state change's present(s) — `finals`, exactly as
    /// without motion — the end frame of a transition over `rects` (what the
    /// change repainted; each is one frame's damage, never the screen).
    /// `anchor`: the iris's centre for Grow / Shrink. Motion off (the
    /// kill-switch): the presents alone. Errors are mapped like the
    /// callers' (`100 +` a present's code).
    #[cfg(feature = "desktop-home")]
    fn transition(
        &mut self,
        kind: neo_atrium::motion::Kind,
        anchor: Rect,
        rects: &[Rect],
        client: bool,
        finals: &[Rect],
    ) -> Result<(), i32> {
        #[cfg(feature = "desktop-motion")]
        let run = self.animate(kind, anchor, rects, client)?;
        #[cfg(not(feature = "desktop-motion"))]
        let _ = (kind, anchor, rects);
        for &f in finals {
            self.present(client, f).map_err(|c| 100 + c)?;
        }
        #[cfg(all(feature = "desktop-motion", feature = "desktop-session-boot-test"))]
        if let Some(run) = run {
            self.log_motion(&run, finals);
        }
        #[cfg(all(feature = "desktop-motion", not(feature = "desktop-session-boot-test")))]
        let _ = run;
        Ok(())
    }

    fn set_focused(&mut self, focused: bool) {
        if self.focused != focused {
            self.focused = focused;
            self.repaint_chrome(self.geo.focus_damage);
        }
    }

    /// S1 repaints its whole (cheap, flat) base on any change.
    #[cfg(not(feature = "desktop-glass"))]
    fn repaint_chrome(&mut self, _damage: Rect) {
        let layout = self.chrome;
        atrium_desktop::paint_base(
            self.base.slice_mut(),
            layout.screen.w,
            &layout,
            self.focused,
        );
        if let Some(glow) = self.glow.as_mut() {
            atrium_desktop::fill_glow(glow.slice_mut(), &layout, self.focused);
        }
    }

    /// S3 repaints only `damage` of the base, from the cached blurs (no blur
    /// runs here — a scene's blurs are made when it is built, or when a move,
    /// resize or docked Lens changes what lies under its glass).
    #[cfg(feature = "desktop-glass")]
    fn repaint_chrome(&mut self, damage: Rect) {
        let stride = self.geo.screen.w;
        let damage = atrium_rect(damage);
        // DS-1: the Home view is repainted exactly where asked (its callers
        // pass the intent field or the view-switch regions, never a box).
        #[cfg(feature = "desktop-home")]
        if let Some(home) = &self.home {
            // OT-66 L1: a password field paints a dot per byte.
            #[cfg(feature = "login")]
            if home.masked {
                let dots = "\u{2022}".repeat(home.line.as_bytes().len());
                self.chrome.paint_home_view(
                    self.base.slice_mut(),
                    stride,
                    home.view(),
                    &dots,
                    damage,
                );
                return;
            }
            self.chrome.paint_home_view(
                self.base.slice_mut(),
                stride,
                home.view(),
                home.line.as_str(),
                damage,
            );
            return;
        }
        // A focus change touches only disjoint regions whose bounding box
        // spans most of the screen; repaint those, not the box.
        let regions = self.chrome.layout.focus_regions();
        let full = [self.chrome.layout.screen];
        let regions: &[neo_atrium::Rect] = if damage == full[0] { &full } else { &regions };
        for region in regions {
            let part = region.intersect(&damage);
            if !part.is_empty() {
                self.chrome
                    .paint_rect(self.base.slice_mut(), stride, self.focused, part);
            }
        }
        if let Some(glow) = self.glow.as_mut() {
            self.chrome
                .fill_glow_strided(glow.slice_mut(), self.glow_stride, self.focused);
        }
    }

    /// Copy `rect` (client coordinates) of the client's committed pixels
    /// into the WM's own copy; returns it in output coordinates (clipped).
    fn take_client(&mut self, client: &SharedPixels, rect: Rect) -> Rect {
        let r = client.snapshot(rect, self.client_copy.slice_mut());
        Rect::new(self.geo.client.x + r.x, self.geo.client.y + r.y, r.w, r.h)
    }

    /// Recompose `damage` from base + focus light + (optionally) the WM's
    /// copy of the client, then commit exactly that rectangle and present
    /// the full-screen output. Returns the rectangle actually written.
    fn present(&mut self, client: bool, damage: Rect) -> Result<Rect, i32> {
        let written = self.compose(client, damage)?;
        if written.is_empty() {
            return Ok(written);
        }
        self.commit_present(written)?;
        Ok(written)
    }

    /// The composing half of [`Self::present`]: recompose `damage` (and a
    /// pending sprite erase) into the output plane, the drag outline and the
    /// sprite on top. Nothing reaches the screen until [`Self::commit_present`].
    fn compose(&mut self, client: bool, damage: Rect) -> Result<Rect, i32> {
        // OT-66 L2: while locked only the lock's own frame is composed.
        #[cfg(feature = "login")]
        if lock::locked() && !self.lock.painting {
            return Ok(Rect::default());
        }
        let l = self.geo;
        #[cfg(feature = "desktop-pointer")]
        let damage = if self.extra_damage.is_empty() {
            damage
        } else {
            damage.union(&core::mem::take(&mut self.extra_damage))
        };
        let mut layers = [Layer {
            pixels: self.base.slice(),
            width: l.screen.w,
            height: l.screen.h,
            stride: l.screen.w,
            x: 0,
            y: 0,
            blend: Blend::Opaque,
        }; 5];
        let mut count = 1;
        if let Some(glow) = self.glow.as_ref() {
            layers[count] = Layer {
                pixels: glow.slice(),
                width: l.glow.w,
                height: l.glow.h,
                stride: self.glow_stride,
                x: l.glow.x,
                y: l.glow.y,
                blend: Blend::Alpha,
            };
            count += 1;
        }
        if client {
            layers[count] = Layer {
                pixels: self.client_copy.slice(),
                width: l.client.w,
                height: l.client.h,
                stride: BUFFER_W,
                x: l.client.x,
                y: l.client.y,
                blend: Blend::Opaque,
            };
            count += 1;
        }
        // A9-D: the busy emblem, over the client it belongs to.
        #[cfg(feature = "desktop-busy")]
        if client && let Some((dest, pixels)) = self.busy_layer() {
            layers[count] = Layer {
                pixels,
                width: dest.w,
                height: dest.h,
                stride: neo_atrium::busy::SIDE,
                x: dest.x,
                y: dest.y,
                blend: Blend::Alpha,
            };
            count += 1;
        }
        // DS-1c: the Lens shows in the window view only.
        #[cfg(feature = "desktop-lens")]
        if let (Some(store), None, Some(lens), true) = (
            self.lens.as_ref(),
            &self.home,
            self.chrome.layout.lens,
            self.chrome.lens_shown(),
        ) {
            layers[count] = Layer {
                pixels: store.copy.slice(),
                width: lens.client.w,
                height: lens.client.h,
                stride: lens.client.w,
                x: lens.client.x,
                y: lens.client.y,
                blend: Blend::Opaque,
            };
            count += 1;
        }
        let written = compose_layers_xrgb8888(
            self.out.slice_mut(),
            l.screen.w,
            l.screen.h,
            l.screen.w,
            &layers[..count],
            damage,
        )
        .map_err(|_| 40)?;
        if written.is_empty() {
            return Ok(written);
        }
        // DI-1b: the drag outline, then (DI-1a) the sprite, ride on top,
        // only where this present recomposed.
        #[cfg(feature = "desktop-pointer")]
        if let Some(frame) = self.outline {
            pointer::draw_outline(self.out.slice_mut(), l.screen.w, frame, written);
        }
        #[cfg(feature = "desktop-pointer")]
        if let Some(at) = self.sprite_at {
            pointer::draw(
                self.out.slice_mut(),
                l.screen.w,
                at,
                self.sprite_shape,
                written,
            );
        }
        Ok(written)
    }

    /// Commit exactly `written` of the output plane and present it; wait
    /// for the present's fence.
    fn commit_present(&self, written: Rect) -> Result<(), i32> {
        // OT-66 L2: and only the lock's own frame reaches the screen.
        #[cfg(feature = "login")]
        if lock::locked() && !self.lock.painting {
            return Ok(());
        }
        let rect = encode_damage_rect(
            written.x as u16,
            written.y as u16,
            written.w as u16,
            written.h as u16,
        );
        if sys_buffer_commit_damage(self.output, &rect) != 0 {
            return Err(41);
        }
        let epoch = sys_fb_present(self.output, 0, 0);
        if epoch < 1 {
            return Err(42);
        }
        let mut result = [0u8; PRESENT_RESULT_V1_LEN];
        if sys_present_fence_wait(self.output, epoch as u64, &mut result, 1000) != 0
            || u32::from_le_bytes([result[8], result[9], result[10], result[11]]) != 1
        {
            return Err(43);
        }
        Ok(())
    }
}

pub fn run() -> i32 {
    log(b"DESKTOP session boot compositor=neo-init policy=userland-wm\n");
    let (width, height) = match sys_fb_output_query() {
        Ok(size) => size,
        Err(_) => return 1,
    };
    // Damage rectangles travel as u16 on the wire (DAMAGE_RECT_V1).
    if width > u16::MAX as usize || height > u16::MAX as usize {
        return 2;
    }
    #[cfg(not(feature = "desktop-glass"))]
    let Some(chrome) = atrium_desktop::layout(width, height, CLIENT_W, CLIENT_H) else {
        log(b"DESKTOP output too small for the Arthur surface\n");
        return 3;
    };
    // OT-88 S1: the settings, before the scene that shows them.
    #[cfg(feature = "atrium-settings-store")]
    settings::load();
    #[cfg(feature = "desktop-glass")]
    let build_start = perf::now();
    #[cfg(feature = "desktop-glass")]
    let Some(chrome) = glass::build(width, height) else {
        log(b"DESKTOP output too small for the Arthur surface\n");
        return 3;
    };
    #[cfg(not(feature = "desktop-glass"))]
    let layout = chrome;
    let mut scene = match Scene::new(chrome) {
        Ok(scene) => scene,
        Err(code) => return code,
    };
    let geo = scene.geo;
    #[cfg(feature = "desktop-pointer")]
    {
        scene.pointer = pointer::Pointer::claim(geo.screen);
    }
    #[cfg(feature = "desktop-home")]
    let home_focus = match home::claim_focus() {
        Ok(focus) => focus,
        Err(code) => return code,
    };
    // OT-85: the readout (and the intro's time-aware greeting) only when the
    // kernel has a wall-clock epoch.
    #[cfg(feature = "desktop-clock")]
    clock::start(&mut scene);
    // OT-66 L1: sign-in comes first (it shows Home with its own card).
    #[cfg(feature = "login")]
    if let Err(code) = login::gate(&mut scene, &home_focus) {
        return code;
    }
    #[cfg(feature = "arthur-actions-test")]
    permission::seed();
    #[cfg(feature = "desktop-intro")]
    intro::start(&mut scene);
    // OT-87 R4.4b: the Install chip, where this boot shows it.
    #[cfg(all(feature = "installer", feature = "desktop-intro"))]
    installer_view::start(&mut scene);
    #[cfg(feature = "desktop-home")]
    scene.show_home(true, false);
    // First frame: the whole output, before any client exists.
    #[cfg(feature = "desktop-glass")]
    let frame_start = perf::now();
    if let Err(code) = scene.present(false, geo.screen) {
        return 100 + code;
    }
    #[cfg(not(feature = "desktop-glass"))]
    log_layout(&layout);
    #[cfg(feature = "desktop-glass")]
    glass::log_scene(
        &scene.chrome,
        frame_start.wrapping_sub(build_start),
        perf::now().wrapping_sub(frame_start),
    );
    #[cfg(feature = "desktop-glass")]
    let mut frames = perf::FrameStats::default();
    // DS-1b: the session broker's LaunchCap table (ADVISORY — the kernel
    // still lets any process spawn by name; OT-66 gates it later).
    #[cfg(feature = "desktop-home")]
    let mut broker = libneo::atrium_launch::LaunchBroker::new();
    // Chad #177 item 4 (`desktop_home_cycle`): Home -> Arthur -> Home rounds
    // before the scripted one, each mapping a new Arthur buffer into this
    // long-lived WM (an Arthur built for the test ends its session after
    // answering the intent "cycle"). The kernel's borrowed-pin cap is cut to
    // 8 for it, so a mapping kept past its round fails a later one.
    #[cfg(feature = "desktop-home-cycle-test")]
    for round in 0..DESKTOP_HOME_CYCLE_ROUNDS {
        let code = arthur_session(
            &mut scene,
            width,
            height,
            #[cfg(feature = "desktop-glass")]
            &mut frames,
            b"cycle",
            &home_focus,
            &mut broker,
        );
        if code != 0 {
            log(b"DESKTOP home cycle round failed\n");
            let _ = round;
            return code;
        }
        if let Err(code) = home::return_home(&mut scene, &home_focus) {
            return code;
        }
    }
    #[cfg(feature = "desktop-home-cycle-test")]
    log(b"DESKTOP home cycle rounds=10 ok\n");
    // Logged only once Home is on screen: typing may start from here.
    #[cfg(feature = "desktop-home")]
    log(b"DESKTOP home ready focus=wm-home input=adr074-own field=intent\n");
    // OT-66 L1's boot tests end once sign-in has started the session (Home,
    // the WM's own focus); DS-1's own scenarios cover what follows.
    #[cfg(feature = "login-record-guard-test")]
    login::record_guard_test();
    #[cfg(feature = "login")]
    if cfg!(all(
        feature = "login-boot-test",
        not(feature = "lock-boot-test")
    )) {
        log(b"LOGIN session started view=home\n");
        return 0;
    }
    // OT-66 L2: the lock scenarios go on into the session.
    #[cfg(feature = "lock-boot-test")]
    log(b"LOGIN session started view=home\n");
    #[cfg(feature = "desktop-intro")]
    intro::log_start(&scene);

    #[cfg(not(feature = "desktop-home"))]
    return arthur_session(
        &mut scene,
        width,
        height,
        #[cfg(feature = "desktop-glass")]
        &mut frames,
        &[],
    );

    // DS-1: Home holds the keyboard until the user names an intent; Enter
    // launches Arthur with it, and Arthur's exit hands the keyboard back.
    // (The boot test runs one Home -> Arthur -> Home round, then ends.)
    #[cfg(feature = "desktop-home")]
    #[cfg_attr(feature = "desktop-session-boot-test", allow(clippy::never_loop))]
    loop {
        let intent = match home::read_intent(&mut scene, &home_focus) {
            Ok(intent) => intent,
            Err(code) => return code,
        };
        // Chad DS-1ab F1 (re-check): Home stays on screen — and its route
        // live — while Arthur is spawned and handshaken; the window view is
        // presented only inside the launch's `HANDOFF`, after the route moved
        // to Arthur and Arthur discarded its queue. A key typed once the
        // window shows is Arthur's; one typed while Home still shows is Home's
        // (and Home's next turn discards it), never lost between the two.
        let code = arthur_session(
            &mut scene,
            width,
            height,
            &mut frames,
            intent.as_bytes(),
            &home_focus,
            &mut broker,
        );
        if code != 0 {
            return code;
        }
        if let Err(code) = home::return_home(&mut scene, &home_focus) {
            return code;
        }
        #[cfg(feature = "desktop-session-boot-test")]
        return 0;
    }
}

/// Chad #177 item 4: a compositor-role buffer mapping, unmapped (its pin
/// and charge released) when this drops.
struct BufferMapping(*const u8);

impl Drop for BufferMapping {
    fn drop(&mut self) {
        let _ = libneo::sys_buffer_unmap(self.0);
    }
}

/// One Arthur session: spawn, the ADR-073 surface handshake, ACTIVATE-only
/// focus, then commits until Arthur destroys its surface and exits. `intent`
/// (DS-1) is handed to Arthur after the handshake as its first input line.
/// The module the WM launches Arthur from.
const ARTHUR_MODULE: &str = "arthur";

fn arthur_session(
    scene: &mut Scene,
    width: usize,
    height: usize,
    #[cfg(feature = "desktop-glass")] frames: &mut perf::FrameStats,
    intent: &[u8],
    #[cfg(feature = "desktop-home")] home_focus: &home::HomeFocus,
    #[cfg(feature = "desktop-home")] broker: &mut libneo::atrium_launch::LaunchBroker,
) -> i32 {
    // DI-1b resize: a new client starts at its created size.
    #[cfg(feature = "desktop-resize")]
    if let Err(code) = scene.reset_client_size() {
        return code;
    }
    let (session, client_end) = match sys_chan_create() {
        Ok(pair) => pair,
        Err(_) => return 4,
    };
    let process = match spawn_arthur(client_end) {
        Ok(process) => process,
        Err(code) => return code,
    };
    // OT-88 S1 (§5.3, the PM's #222 bounce): whether this client may send
    // `OP_SETTING`, by the module the WM launched it from.
    #[cfg(feature = "atrium-settings-store")]
    let settings_app = settings::is_settings_app(ARTHUR_MODULE);
    // The boot test sets the idle lock (WM-only) to 5 the WM's way, as S2's
    // idle-lock view will.
    #[cfg(feature = "settings-standin-test")]
    let _ = settings::set_wm(scene, settings_core::Key::IdleLockMinutes.id(), 5);
    #[cfg(feature = "arthur-actions-test")]
    permission::note_client(process);

    let mut registry = SurfaceRegistry::new(NS, width as u32, height as u32);
    let (create_header, create) = match recv_request(session) {
        // The surface must be exactly the buffer it will attach: the
        // registry bounds a damage rect by the size declared here.
        Ok((header, Request::Create(create)))
            if create.role == ROLE_TEXT
                && create.width == CLIENT_W as u32
                && create.height == CLIENT_H as u32 =>
        {
            (header, create)
        }
        _ => return 5,
    };
    let (surface, surface_client) = match sys_chan_create() {
        Ok(pair) => pair,
        Err(_) => return 6,
    };
    let identity = match registry.create(create, SURFACE_CAP_PROVENANCE) {
        Ok(identity) => identity,
        Err(_) => return 7,
    };
    if !send_reply(
        session,
        OP_CREATE_SURFACE,
        create_header.request_id,
        identity,
    ) || !grant_surface(session, surface_client)
    {
        return 8;
    }
    // Arthur proves its own scanout bypass is refused before attaching.
    if !recv_control(session, b'B') {
        return 9;
    }

    let (import_header, import) = match recv_request(surface) {
        Ok((header, Request::ImportAttach(import))) => (header, import),
        _ => return 10,
    };
    if sys_send(surface, b"R") != 1 {
        return 11;
    }
    let read_only = CapRights::new(
        meta_rights::USE,
        ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
    );
    let buffer = match sys_recv_grant(surface, read_only) {
        Ok(buffer) => buffer,
        Err(_) => return 12,
    };
    let mut desc = [0u8; BUFFER_DESC_V1_LEN];
    let expected = encode_buffer_desc(
        BUFFER_W as u32,
        BUFFER_H as u32,
        BUFFER_STRIDE as u32,
        FORMAT_XRGB8888_LE,
        BUFFER_BYTES,
    );
    if sys_buffer_query(buffer, &mut desc) != 0 || desc != expected {
        return 13;
    }
    // The compositor role may only map Arthur's pixels read-only.
    if sys_buffer_map(buffer, 0).is_ok() {
        return 14;
    }
    let client_pixels = match sys_buffer_map(buffer, 1) {
        Ok(ptr) => ptr as *const u32,
        Err(_) => return 15,
    };
    // Chad #177 item 4: unmapped when this session returns, on every path,
    // so its pin (and charge) don't outlive it in this long-lived WM. Declared
    // before `client`, so `client` (the only view of the pages) drops first.
    let _client_mapping = BufferMapping(client_pixels as *const u8);
    // SAFETY: the RO compositor mapping covers the validated descriptor
    // (exactly BUFFER_W * BUFFER_H tightly packed pixels) and stays mapped
    // until `_client_mapping` drops, after `client`. Arthur keeps writing
    // those pages, so the WM never forms a reference over them: it reads them
    // only through `SharedPixels` (S1-F2).
    let client = unsafe { SharedPixels::new(client_pixels, BUFFER_W, BUFFER_H) };
    if registry
        .import_attach_on_channel(
            import,
            identity.surface_cap_provenance,
            BUFFER_CAP_PROVENANCE,
        )
        .is_err()
        || !send_reply(
            surface,
            OP_IMPORT_ATTACH_BUFFER,
            import_header.request_id,
            identity,
        )
    {
        return 16;
    }

    // Arthur's first commit. Its damage must cover the whole client (Arthur
    // paints its chrome first): the WM's copy starts as zeroed pages, so a
    // smaller first rect would show zeros in Arthur's own window only.
    if let Err(code) = handle_commit(surface, &mut registry, identity, scene, &client) {
        return code;
    }

    let (focus_header, focus) = match recv_request(surface) {
        Ok((header, Request::Focus(focus))) => (header, focus),
        _ => return 17,
    };
    if registry.focus_request_on_channel(
        focus,
        identity.surface_cap_provenance,
        |surface_id, role| surface_id == identity.surface_id && role == ROLE_TEXT,
    ) != Ok(true)
        || !send_reply(surface, OP_FOCUS_REQUEST, focus_header.request_id, identity)
    {
        return 18;
    }
    let activator = match sys_recv_grant(
        session,
        CapRights::new(meta_rights::USE, ATRIUM_FOCUS_ACTIVATE),
    ) {
        Ok(activator) => activator,
        Err(_) => return 19,
    };
    // The WM's handle can only ACTIVATE: it can never read a keystroke.
    let mut poison = [0xA5; INPUT_EVENT_V1_LEN];
    if sys_input_focus_recv(activator, &mut poison) != EPERM || poison != [0xA5; INPUT_EVENT_V1_LEN]
    {
        return 20;
    }
    // Zenbook ask-freeze: between 'G' and 'M' Arthur loads its vocab and
    // model, so the WM polls for 'M' and keeps the pointer live meanwhile.
    if !recv_control(session, b'G') {
        return 21;
    }
    match poll_control(session, b'M', scene) {
        Ok(true) => {}
        Ok(false) => return 21,
        Err(code) => return code,
    }
    // F8: Arthur's `K` carries the user's zone (`[K, lo, hi]`) so he can
    // answer the time in it; a bare `K` without `desktop-clock`.
    #[cfg(feature = "desktop-clock")]
    let k_msg = {
        let z = scene.clock.mode().zone.to_le_bytes();
        [b'K', z[0], z[1]]
    };
    #[cfg(not(feature = "desktop-clock"))]
    let k_msg = [b'K'];
    // DS-1 (Chad DS-1a F1, `HANDOFF`): the route moves to Arthur, then
    // Arthur's first turn begins with its intent (it discards anything its
    // queue held before reading a key), then the focus light is shown.
    #[cfg(feature = "desktop-home")]
    if let Err(code) = libneo::atrium_handoff::hand_over(
        &mut libneo::atrium_handoff::Steps {
            swap: || home::swap_to(activator, 22),
            discard: || {
                if sys_send(session, &k_msg) != k_msg.len() as i64 {
                    return Err(23);
                }
                if !home::send_intent(session, intent) {
                    return Err(31);
                }
                // Arthur has discarded its queue (AAG: ordered before the
                // present, not left to the scheduler).
                if !recv_control(session, b'D') {
                    return Err(35);
                }
                Ok(())
            },
            present: || {
                // The whole switch (the field is wider than the window, so
                // the client's own damage would leave slivers of it), then
                // the focus light.
                scene.show_window();
                scene.view_transition(Kind::Open, true)?;
                #[cfg(feature = "desktop-session-boot-test")]
                {
                    home::log_stale_outside(scene, b"window", &[scene.geo.client]);
                    home::log_residue(
                        scene,
                        neo_atrium::View::Window { focused: false },
                        b"window",
                    );
                }
                scene.set_focused(true);
                scene.focus_transition(true)
            },
        },
        &libneo::atrium_handoff::HANDOFF,
    ) {
        return code;
    }
    #[cfg(not(feature = "desktop-home"))]
    {
        if libneo::sys_input_focus_activate(activator) != 0 {
            return 22;
        }
        scene.set_focused(true);
        if let Err(code) = scene.present(true, scene.geo.focus_damage) {
            return 100 + code;
        }
        if sys_send(session, &k_msg) != k_msg.len() as i64 {
            return 23;
        }
    }
    // DS-1b: Arthur's LaunchCap — allowlisted to the probe module (DS-1c: and
    // the Lens).
    #[cfg(feature = "desktop-home")]
    let (mut launch_cap, launch_cap_client) =
        match launch::mint(broker, session, identity.surface_cap_provenance) {
            Ok((cap, client)) => (Some(cap), client),
            Err(code) => return code,
        };
    #[cfg(feature = "desktop-lens")]
    let mut lens = lens::Slot::default();
    #[cfg(not(feature = "desktop-home"))]
    let _ = intent;
    // DI-1b resize: the client's configure mailbox (read-only to it).
    #[cfg(feature = "desktop-resize")]
    let mailbox = match resize::mint(session) {
        Ok((configure, handle)) => {
            scene.configure = Some(configure);
            handle
        }
        Err(code) => return code,
    };
    log(b"DESKTOP session ready focus=arthur input=adr074-live wm_event_bytes=0\n");
    #[cfg(feature = "desktop-session-boot-test")]
    let mut damage_logged = false;
    #[cfg(feature = "desktop-session-boot-test")]
    let mut footer_logged = false;

    // DI-1a: Super+Escape (or a click on the Spine) moved the keyboard to
    // the WM's own focus; Arthur's window stays, unfocused.
    #[cfg(feature = "desktop-pointer")]
    let mut escaped = false;
    // L1.3a: this session's prompts, denials and lent caps.
    #[cfg(feature = "arthur-actions")]
    let mut permissions = permission::Session::default();
    #[cfg(feature = "desktop-idle-boot-test")]
    let mut idle_window = idle_witness::Window::start();
    loop {
        // OT-66 L2 (AAG F2): the lock is served every turn, not only when
        // Arthur's channel is empty: a client flooding requests can't keep
        // it from engaging.
        #[cfg(feature = "login")]
        if let Err(code) = lock::service(scene) {
            return code;
        }
        // DI-1a: poll instead of parking, so the pointer and the SAK latch
        // stay live while Arthur is quiet.
        #[cfg(feature = "desktop-pointer")]
        let incoming = loop {
            match try_recv_request(surface) {
                Ok(Some(incoming)) => break incoming,
                Ok(None) => {}
                Err(_) => return 24,
            }
            if let Err(code) = scene.pointer_tick(activator, home_focus, &mut escaped) {
                return code;
            }
            // A9-D: the emblem turns on the WM's clock while Arthur is
            // blocked in a generation.
            #[cfg(feature = "desktop-busy")]
            if let Err(code) = scene.busy_tick() {
                return code;
            }
            // OT-22: park on Arthur's surface channel, the route, the SAK
            // latch, and (escaped) the WM's own focus.
            #[cfg(feature = "desktop-park")]
            {
                use libneo::{WAIT_KIND_CHANNEL, WAIT_KIND_INPUT, WaitItem};
                let items = [
                    WaitItem {
                        handle: surface.0,
                        kind: WAIT_KIND_CHANNEL,
                    },
                    WaitItem {
                        handle: home_focus.events.0,
                        kind: WAIT_KIND_INPUT,
                    },
                ];
                #[cfg(feature = "desktop-busy")]
                let deadline = scene.busy_deadline();
                #[cfg(not(feature = "desktop-busy"))]
                let deadline = u64::MAX;
                scene.park(&items[..if escaped { 2 } else { 1 }], deadline);
            }
            #[cfg(not(feature = "desktop-park"))]
            core::hint::spin_loop();
        };
        #[cfg(feature = "desktop-idle-boot-test")]
        idle_window.note_request();
        #[cfg(not(feature = "desktop-pointer"))]
        let incoming = match recv_incoming(surface) {
            Ok(incoming) => incoming,
            Err(_) => return 24,
        };
        // Bytes that do not decode are the client's error: refused in a
        // reply when the header names the request, dropped when it does not.
        let (header, request) = match incoming {
            Incoming::Request(header, request) => (header, request),
            Incoming::Undecodable {
                target,
                status,
                len,
            } => {
                match target {
                    Some((opcode, request_id)) => {
                        if !refuse(surface, opcode, request_id, identity, status) {
                            return 24;
                        }
                    }
                    None => log_dropped(len),
                }
                continue;
            }
        };
        match request {
            Request::Commit(_) | Request::CommitDamage(_) => {
                #[cfg(feature = "desktop-glass")]
                let frame_start = perf::now();
                let (serial, opcode, damage) =
                    match commit(&request, &mut registry, identity, scene, &client) {
                        Ok(v) => v,
                        Err(Fault::Wm(code)) => return code,
                        Err(Fault::Client(status)) => {
                            if !refuse(surface, header.opcode, header.request_id, identity, status)
                            {
                                return 26;
                            }
                            continue;
                        }
                    };
                #[cfg(feature = "desktop-glass")]
                frames.record(perf::now().wrapping_sub(frame_start));
                // Logged before the reply: Arthur logs (in several writes)
                // only once it has the reply, so the two processes' lines can
                // never interleave on the serial console.
                // S2-F2 witness: a keystroke's commit presents only the band
                // Arthur redrew, translated to output coordinates.
                // Arthur's footer naming its model is committed on its own,
                // before any keystroke; it is witnessed separately so `first`
                // stays the first keystroke's band.
                #[cfg(feature = "desktop-session-boot-test")]
                if opcode == OP_COMMIT_DAMAGE && is_footer_band(damage, scene.geo.client) {
                    if !footer_logged {
                        footer_logged = true;
                        log_footer_damage(damage, scene.geo.client);
                    }
                } else if !damage_logged && opcode == OP_COMMIT_DAMAGE {
                    damage_logged = true;
                    log_damage(damage, scene.geo.client);
                }
                #[cfg(not(feature = "desktop-session-boot-test"))]
                let _ = damage;
                #[cfg(feature = "desktop-session-boot-test")]
                if serial == ANSWER_SERIAL {
                    log(b"DESKTOP answer composited surface=arthur full_scene=wm_owned\n");
                }
                if !send_reply(surface, opcode, header.request_id, identity) {
                    return 26;
                }
                #[cfg(not(feature = "desktop-session-boot-test"))]
                let _ = serial;
            }
            // A9-D: Arthur is about to block in a generation; the WM shows
            // the emblem in the rect it reserved until its next commit. A
            // request the registry refuses (a rect outside the surface, or
            // one named on another surface's channel) is the client's error:
            // its reply carries the protocol status, the scene is left as it
            // was, and the session goes on (Janus #96). Only the WM's own
            // present failing is fatal.
            #[cfg(feature = "desktop-busy")]
            Request::Busy(request) => {
                let status =
                    match registry.busy_on_channel(request, identity.surface_cap_provenance) {
                        Ok(busy) => {
                            if let Err(code) = scene.start_busy(busy) {
                                return code;
                            }
                            0
                        }
                        Err(refused) => refused.status(),
                    };
                #[cfg(feature = "desktop-session-boot-test")]
                if status != 0 {
                    Line::new()
                        .s(b"DESKTOP busy refused status=")
                        .n(u64::from(status))
                        .s(b" scene=unchanged\n")
                        .emit();
                }
                if !send_reply_status(
                    surface,
                    libneo::atrium_protocol::OP_BUSY,
                    header.request_id,
                    identity,
                    status,
                ) {
                    return 121;
                }
            }
            // DI-1b resize: the client laid itself out at the published size.
            #[cfg(feature = "desktop-resize")]
            Request::Configure(ack) => {
                match scene.apply_configure(ack, &mut registry, identity, &client) {
                    Ok(()) => {}
                    Err(Fault::Wm(code)) => return code,
                    Err(Fault::Client(status)) => {
                        if !refuse(
                            surface,
                            libneo::atrium_protocol::OP_CONFIGURE,
                            header.request_id,
                            identity,
                            status,
                        ) {
                            return 95;
                        }
                        continue;
                    }
                }
                if !send_reply(
                    surface,
                    libneo::atrium_protocol::OP_CONFIGURE,
                    header.request_id,
                    identity,
                ) {
                    return 95;
                }
            }
            Request::Destroy(destroy) => {
                if let Err(refused) =
                    registry.destroy_on_channel(destroy, identity.surface_cap_provenance)
                {
                    if !refuse(
                        surface,
                        OP_DESTROY_SURFACE,
                        header.request_id,
                        identity,
                        refused.status(),
                    ) {
                        return 28;
                    }
                    continue;
                }
                // The channel names one surface: destroyed is gone.
                if registry.is_alive(identity) {
                    return 27;
                }
                // DI-1b resize: an outstanding configure dies with the client,
                // and its outline is erased with the window.
                #[cfg(feature = "desktop-resize")]
                {
                    scene.configure = None;
                    if let Some(outline) = scene.outline.take() {
                        scene.extra_damage = scene.extra_damage.union(&outline);
                    }
                }
                // The client's pixels are no longer the WM's to show: repaint
                // the client rect from the base with focus withdrawn.
                scene.set_focused(false);
                let focus_damage = scene.geo.focus_damage;
                // DM-1: the light and the client's last picture fade out.
                #[cfg(feature = "desktop-home")]
                {
                    let mut rects = [Rect::default(); 7];
                    rects[..6].copy_from_slice(&scene.focus_rects());
                    rects[6] = scene.geo.client;
                    let glow = scene.geo.glow;
                    if let Err(code) =
                        scene.transition(Kind::Focus, glow, &rects, false, &[focus_damage])
                    {
                        return code;
                    }
                }
                #[cfg(not(feature = "desktop-home"))]
                if let Err(code) = scene.present(false, focus_damage) {
                    return 100 + code;
                }
                #[cfg(feature = "desktop-glass")]
                frames.log();
                log(b"DESKTOP surface destroyed client=arthur registry_alive=false\n");
                if !send_reply(surface, OP_DESTROY_SURFACE, header.request_id, identity) {
                    return 28;
                }
                break;
            }
            // L1.3a: Arthur asks for one action on one path. The WM answers
            // from its record or its own prompt, then lends the cap.
            #[cfg(feature = "arthur-actions")]
            Request::Permission(request) => {
                #[cfg(not(feature = "desktop-pointer"))]
                let mut escaped = false;
                let answer = match permission::serve(
                    scene,
                    &mut permissions,
                    request,
                    home_focus,
                    permission::Client {
                        surface,
                        activator,
                        escaped: &mut escaped,
                        ours: request.identity == identity,
                    },
                ) {
                    Ok(answer) => answer,
                    Err(code) => return code,
                };
                if !send_reply_status(
                    surface,
                    libneo::atrium_protocol::OP_PERMISSION,
                    header.request_id,
                    identity,
                    answer.status,
                ) || !permission::lend(surface, &answer)
                {
                    return 35;
                }
            }
            #[cfg(feature = "arthur-actions")]
            Request::PermissionDone(done) => {
                // It must name this channel's own surface (Dexter 203 note).
                let status = if done.identity == identity {
                    permission::done(&mut permissions, done.outcome);
                    0
                } else {
                    log(b"PERMISSION done refused reason=identity\n");
                    libneo::atrium_protocol::ProtocolError::CapRequired.status()
                };
                if !send_reply_status(
                    surface,
                    libneo::atrium_protocol::OP_PERMISSION_DONE,
                    header.request_id,
                    identity,
                    status,
                ) {
                    return 36;
                }
            }
            // DS-1: Arthur hands the keyboard back (Esc). Home takes over
            // while Arthur stays alive, blocked on its session channel; the
            // next intent (or Enter on its chip) resumes this same Arthur.
            #[cfg(feature = "desktop-home")]
            Request::Launch(request) => {
                // While the broker waits for the proof, the WM keeps serving
                // the SAK and the pointer (Janus DS-1b).
                #[cfg(feature = "desktop-pointer")]
                let mut idle =
                    |scene: &mut Scene| scene.pointer_tick(activator, home_focus, &mut escaped);
                #[cfg(not(feature = "desktop-pointer"))]
                let mut idle = |_: &mut Scene| Ok(());
                let status = match launch::serve(
                    broker,
                    &mut launch_cap,
                    request,
                    identity,
                    scene,
                    #[cfg(feature = "desktop-lens")]
                    &mut lens,
                    &mut idle,
                ) {
                    Ok(status) => status,
                    Err(code) => return code,
                };
                if !send_status(surface, header.request_id, identity, status) {
                    return 34;
                }
            }
            #[cfg(feature = "desktop-home")]
            Request::Yield(yielded) => {
                if let Err(refused) =
                    registry.yield_focus_on_channel(yielded, identity.surface_cap_provenance)
                {
                    if !refuse(
                        surface,
                        libneo::atrium_protocol::OP_YIELD_FOCUS,
                        header.request_id,
                        identity,
                        refused.status(),
                    ) {
                        return 32;
                    }
                    continue;
                }
                if !send_reply(
                    surface,
                    libneo::atrium_protocol::OP_YIELD_FOCUS,
                    header.request_id,
                    identity,
                ) {
                    return 32;
                }
                if let Err(code) = home::hold(scene, home_focus) {
                    return code;
                }
                // DS-1c: Home's choice may be the Lens's chip: the WM shows
                // the Lens selected until the keyboard goes back to Arthur
                // (the empty intent) or to Home.
                #[cfg(feature = "desktop-lens")]
                let intent = match lens::choose(scene, home_focus) {
                    Ok(intent) => intent,
                    Err(code) => return code,
                };
                #[cfg(not(feature = "desktop-lens"))]
                let intent = match home::read_intent(scene, home_focus) {
                    Ok(intent) => intent,
                    Err(code) => return code,
                };
                if let Err(code) = home::resume(scene, activator, session, intent.as_bytes()) {
                    return code;
                }
                #[cfg(feature = "desktop-pointer")]
                {
                    escaped = false;
                }
            }
            // OT-88 S1: a client asks to set one setting. Only the settings
            // app may, and only a client-writable key; the WM checks it,
            // writes the store and applies it live. A refusal is in the
            // reply's status, and the session goes on. It must name this
            // channel's own surface.
            #[cfg(feature = "atrium-settings-store")]
            Request::Setting(request) => {
                let status = if request.identity == identity {
                    settings::request(scene, settings_app, request.key, request.value)
                } else {
                    libneo::atrium_protocol::ProtocolError::CapRequired.status()
                };
                if !send_reply_status(
                    surface,
                    libneo::atrium_protocol::OP_SETTING,
                    header.request_id,
                    identity,
                    status,
                ) {
                    return 122;
                }
            }
            // A request the protocol knows but this session does not take
            // here (a second create, an attach, a focus request).
            _ => {
                if !refuse(
                    surface,
                    header.opcode,
                    header.request_id,
                    identity,
                    libneo::atrium_protocol::ProtocolError::BadState.status(),
                ) {
                    return 29;
                }
            }
        }
    }

    let exit = sys_wait(process, 0);
    #[cfg(feature = "desktop-home")]
    {
        // DS-1c: the Lens belongs to this session's intent; it ends with it.
        #[cfg(feature = "desktop-lens")]
        if let Some(session) = lens.session.take()
            && let Err(code) = lens::end(scene, session)
        {
            return code;
        }
        broker.revoke(identity.surface_cap_provenance);
        // (Already closed if a late proof revoked it.)
        if let Some(cap) = launch_cap {
            let _ = libneo::sys_channel_close(cap);
        }
        let _ = libneo::sys_channel_close(launch_cap_client);
    }
    // DS-1: Home relaunches Arthur, so each round releases its handles
    // (the session and surface channels, the buffer, the dead client's focus
    // activator and the process). The read-only client mapping has no unmap
    // syscall yet; it is one 1 MiB view per round (4 MiB under
    // `desktop-resize`, plus the configure mailbox's page).
    // L1.3a: every cap lent this session is revoked with it.
    #[cfg(feature = "arthur-actions")]
    permission::end(&mut permissions);
    #[cfg(feature = "desktop-home")]
    for handle in [session, surface, buffer, activator, Handle(process.0)] {
        let _ = libneo::sys_channel_close(handle);
    }
    #[cfg(feature = "desktop-resize")]
    {
        scene.configure = None;
        let _ = libneo::sys_channel_close(mailbox);
    }
    match exit {
        Ok(0) => {
            log(b"DESKTOP session end client=arthur exit=0\n");
            0
        }
        _ => 30,
    }
}

fn handle_commit(
    surface: Handle,
    registry: &mut SurfaceRegistry,
    identity: libneo::atrium_protocol::Identity,
    scene: &mut Scene,
    client: &SharedPixels,
) -> Result<(), i32> {
    let (header, request) = recv_request(surface).map_err(|_| 50)?;
    let (_, opcode, _) =
        commit(&request, registry, identity, scene, client).map_err(|fault| match fault {
            Fault::Wm(code) => code,
            Fault::Client(_) => 51,
        })?;
    if !send_reply(surface, opcode, header.request_id, identity) {
        return Err(52);
    }
    Ok(())
}

/// `DESKTOP client damage first=WxH at=X,Y band=yes|no`: what the present of
/// the first damage commit actually wrote; `band=yes` when it lies inside the
/// client and is shorter than it. (xtask also bounds its height: one text
/// line for a keystroke in S1/S3.)
#[cfg(feature = "desktop-session-boot-test")]
fn log_damage(damage: Rect, client: Rect) {
    let band = client.covers(&damage) && damage.h < client.h;
    Line::new()
        .s(b"DESKTOP client damage first=")
        .n(damage.w as u64)
        .s(b"x")
        .n(damage.h as u64)
        .s(b" at=")
        .n(damage.x as u64)
        .s(b",")
        .n(damage.y as u64)
        .s(if band { b" band=yes\n" } else { b" band=no\n" })
        .emit();
}

/// A damage commit that lies wholly below Arthur's footer rule: the footer
/// line alone, not the transcript.
#[cfg(feature = "desktop-session-boot-test")]
fn is_footer_band(damage: Rect, client: Rect) -> bool {
    let rule = client
        .bottom()
        .saturating_sub(ARTHUR_FOOTER_RULE_FROM_BOTTOM);
    !damage.is_empty() && damage.y > rule && client.covers(&damage)
}

/// `DESKTOP client footer damage=WxH at=X,Y band=yes|no`: the present of
/// Arthur's footer commit, which must also be a band.
#[cfg(feature = "desktop-session-boot-test")]
fn log_footer_damage(damage: Rect, client: Rect) {
    let band = client.covers(&damage) && damage.h < client.h;
    Line::new()
        .s(b"DESKTOP client footer damage=")
        .n(damage.w as u64)
        .s(b"x")
        .n(damage.h as u64)
        .s(b" at=")
        .n(damage.x as u64)
        .s(b",")
        .n(damage.y as u64)
        .s(if band { b" band=yes\n" } else { b" band=no\n" })
        .emit();
}

/// Accept a client commit (`OP_COMMIT`: the whole surface; S2-F2's
/// `OP_COMMIT_DAMAGE`: the rect it names, validated against the surface),
/// copy exactly that rect of its pixels into the WM's copy (S1-F2) and
/// present it. Returns the serial, the reply opcode and the rect the present
/// actually wrote (output coordinates). A commit the registry refuses (a
/// stale serial, damage outside the surface, another channel's identity)
/// is the client's error and changes nothing; anything else is not a
/// commit (`Wm(50)`).
fn commit(
    request: &Request,
    registry: &mut SurfaceRegistry,
    identity: libneo::atrium_protocol::Identity,
    scene: &mut Scene,
    client: &SharedPixels,
) -> Result<(u64, u16, Rect), Fault> {
    let provenance = identity.surface_cap_provenance;
    let refused = |e: libneo::atrium_protocol::ProtocolError| Fault::Client(e.status());
    let (serial, opcode, rect) = match *request {
        Request::Commit(commit) => {
            let serial = registry
                .commit_on_channel(commit, provenance)
                .map_err(refused)?;
            let c = scene.geo.client;
            (serial, OP_COMMIT, Rect::new(0, 0, c.w, c.h))
        }
        Request::CommitDamage(commit) => {
            let (serial, d) = registry
                .commit_damage_on_channel(commit, provenance)
                .map_err(refused)?;
            let rect = Rect::new(
                usize::from(d.x),
                usize::from(d.y),
                usize::from(d.width),
                usize::from(d.height),
            );
            (serial, OP_COMMIT_DAMAGE, rect)
        }
        _ => return Err(Fault::Wm(50)),
    };
    let damage = scene.take_client(client, rect);
    // A9-D: a commit ends the busy emblem; its pixels go back to the client's.
    #[cfg(feature = "desktop-busy")]
    let damage = damage.union(&scene.end_busy());
    // DS-1: while Home is shown (Arthur launching, or held) a commit only
    // updates the WM's copy; the window view presents it when it is shown.
    #[cfg(feature = "desktop-home")]
    if scene.home.is_some() {
        return Ok((serial, opcode, Rect::default()));
    }
    let written = scene
        .present(true, damage)
        .map_err(|code| Fault::Wm(100 + code))?;
    Ok((serial, opcode, written))
}

/// How a session request fails: the client's error, refused in its reply
/// with this protocol status while the session goes on, or the WM's own
/// failure (the session's exit code).
enum Fault {
    Client(u32),
    Wm(i32),
}

/// Refuse a client's request: its reply carries `status`, nothing in the
/// scene or the registry changed, and the session goes on. False if the
/// reply could not be sent.
fn refuse(
    channel: Handle,
    opcode: u16,
    request_id: u64,
    identity: libneo::atrium_protocol::Identity,
    status: u32,
) -> bool {
    #[cfg(feature = "desktop-session-boot-test")]
    Line::new()
        .s(b"DESKTOP request refused op=")
        .n(u64::from(opcode))
        .s(b" status=")
        .n(u64::from(status))
        .s(b"\n")
        .emit();
    send_reply_status(channel, opcode, request_id, identity, status)
}

/// A message too short for a header, or not this protocol's: there is no
/// request to reply to, so it is dropped.
fn log_dropped(len: usize) {
    Line::new()
        .s(b"DESKTOP request dropped len=")
        .n(len as u64)
        .s(b" reason=no-header\n")
        .emit();
}

/// Spawn Arthur with its session channel, plus its NeoFS memory directory
/// when one is provisioned (ephemeral otherwise — exactly the showcase rule).
/// OT-66 K1b: with `cap-rooted-open`, his caps are `ARTHUR_SCOPES` (his own
/// state and the boot modules), not a RootDirCap for all of `/`.
fn spawn_arthur(session_client: Handle) -> Result<libneo::ProcessHandle, i32> {
    match sys_open(
        MEMORY_DIR,
        OPEN_ROOT_TEMPORARY,
        FileRights::LOOKUP.union(FileRights::READDIR),
    ) {
        Ok(memory) => {
            #[cfg(feature = "arthur-actions")]
            let memory = plain_dir(memory);
            let slots = (sys_chan_create(), sys_chan_create());
            let (Ok((_p1, slot1)), Ok((_p2, slot2))) = slots else {
                let _ = sys_close(memory);
                return Err(60);
            };
            let inherited = [session_client, slot1, slot2, Handle(memory.0)];
            let spawned =
                libneo::sys_spawn_scoped(ARTHUR_MODULE, &inherited, libneo::ARTHUR_SCOPES);
            let _ = sys_close(memory);
            spawned.map(|(_pid, process)| process).map_err(|_| 61)
        }
        Err(_) => {
            log(b"DESKTOP arthur memory=ephemeral reason=no-neofs-arthur-memory\n");
            libneo::sys_spawn_scoped(ARTHUR_MODULE, &[session_client], libneo::ARTHUR_SCOPES)
                .map(|(_pid, process)| process)
                .map_err(|_| 61)
        }
    }
}

/// L1.3a: under `file-caps` (the WM's lent caps) a fresh open also carries
/// the owner's meta bits, and spawn passes on only a plain LOOKUP|READDIR
/// directory without `cap-rooted-open`. So Arthur is handed a derived child
/// with exactly those rights. A kernel without `file-caps` refuses the derive,
/// and the open is passed as before.
#[cfg(feature = "arthur-actions")]
fn plain_dir(dir: libneo::FileHandle) -> libneo::FileHandle {
    let rights = FileRights::LOOKUP.union(FileRights::READDIR);
    match libneo::sys_derive(
        Handle(dir.0),
        libneo::CapRights::new(libneo::meta_rights::USE, rights.bits() as u8),
    ) {
        Ok(child) => {
            let _ = sys_close(dir);
            libneo::FileHandle(child.0)
        }
        Err(_) => dir,
    }
}

#[cfg(not(feature = "desktop-glass"))]
fn log_layout(layout: &DesktopLayout) {
    let tier: &[u8] = match layout.tier {
        atrium_desktop::Tier::Full => b" tier=full client_at=",
        atrium_desktop::Tier::Framed => b" tier=framed client_at=",
        atrium_desktop::Tier::Bare => b" tier=bare client_at=",
    };
    Line::new()
        .s(b"DESKTOP scene full_screen=")
        .n(layout.screen.w as u64)
        .s(b"x")
        .n(layout.screen.h as u64)
        .s(tier)
        .n(layout.client.x as u64)
        .s(b",")
        .n(layout.client.y as u64)
        .s(b" owner=wm stale_pixels=0\n")
        .emit();
}

fn write_dec(out: &mut [u8], mut value: usize) -> usize {
    let mut digits = [0u8; 20];
    let mut len = 0;
    loop {
        digits[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for i in 0..len {
        out[i] = digits[len - 1 - i];
    }
    len
}

fn recv_request(channel: Handle) -> Result<(Header, Request), i64> {
    let mut bytes = [0u8; libneo::atrium_protocol::MAX_MESSAGE_LEN];
    let len = sys_recv(channel, &mut bytes);
    if len <= 0 {
        return Err(len);
    }
    decode_request(&bytes[..len as usize]).map_err(|_| libneo::EINVAL)
}

/// A message on the session's surface channel: a request, or bytes that do
/// not decode. For those, `target` is the opcode and request id the header
/// names (`None`: too short for a header, or not this protocol's), and
/// `status` why they were refused.
enum Incoming {
    Request(Header, Request),
    Undecodable {
        target: Option<(u16, u64)>,
        status: u32,
        len: usize,
    },
}

fn incoming(bytes: &[u8]) -> Incoming {
    match decode_request(bytes) {
        Ok((header, request)) => Incoming::Request(header, request),
        Err(error) => Incoming::Undecodable {
            target: libneo::atrium_protocol::request_target(bytes),
            status: error.status(),
            len: bytes.len(),
        },
    }
}

/// The session loop's blocking receive: `Err` only when the channel itself
/// fails (the client is gone).
#[cfg(not(feature = "desktop-pointer"))]
fn recv_incoming(channel: Handle) -> Result<Incoming, i64> {
    let mut bytes = [0u8; libneo::atrium_protocol::MAX_MESSAGE_LEN];
    let len = sys_recv(channel, &mut bytes);
    if len <= 0 {
        return Err(len);
    }
    Ok(incoming(&bytes[..len as usize]))
}

/// DI-1a: `recv_incoming` without parking: `Ok(None)` when nothing waits.
#[cfg(feature = "desktop-pointer")]
fn try_recv_request(channel: Handle) -> Result<Option<Incoming>, i64> {
    let mut bytes = [0u8; libneo::atrium_protocol::MAX_MESSAGE_LEN];
    let len = libneo::sys_try_recv(channel, &mut bytes);
    if len == libneo::EAGAIN {
        return Ok(None);
    }
    if len <= 0 {
        return Err(len);
    }
    Ok(Some(incoming(&bytes[..len as usize])))
}

fn send_reply(
    channel: Handle,
    opcode: u16,
    request_id: u64,
    identity: libneo::atrium_protocol::Identity,
) -> bool {
    let bytes = encode_reply(Reply {
        opcode,
        request_id,
        status: 0,
        identity,
    });
    bytes.len() == REPLY_LEN && sys_send(channel, &bytes) == REPLY_LEN as i64
}

/// A reply to `opcode` carrying a protocol status (0: accepted).
fn send_reply_status(
    channel: Handle,
    opcode: u16,
    request_id: u64,
    identity: libneo::atrium_protocol::Identity,
    status: u32,
) -> bool {
    let bytes = encode_reply(Reply {
        opcode,
        request_id,
        status,
        identity,
    });
    sys_send(channel, &bytes) == REPLY_LEN as i64
}

/// A reply carrying a protocol status (DS-1b launch refusals).
#[cfg(feature = "desktop-home")]
fn send_status(
    channel: Handle,
    request_id: u64,
    identity: libneo::atrium_protocol::Identity,
    status: u32,
) -> bool {
    let bytes = encode_reply(Reply {
        opcode: libneo::atrium_protocol::OP_LAUNCH,
        request_id,
        status,
        identity,
    });
    sys_send(channel, &bytes) == REPLY_LEN as i64
}

/// OT-22 idle witness: the first quiet gap of at least `MIN_GAP_TICKS`
/// between two of Arthur's requests is the idle window; the BSP's idle ticks
/// across it are logged once. The parked build must idle most of it, the
/// spinning control almost none (the pair is the evidence).
#[cfg(feature = "desktop-idle-boot-test")]
mod idle_witness {
    use libneo::{CLOCK_MONOTONIC, sys_clock_gettime, sys_test_idle_ticks};

    const MIN_GAP_TICKS: u64 = 200;
    const NS_PER_TICK: u64 = 10_000_000;

    pub struct Window {
        at: (u64, u64),
        logged: bool,
    }

    fn sample() -> (u64, u64) {
        let ticks = sys_clock_gettime(CLOCK_MONOTONIC).unwrap_or(0) / NS_PER_TICK;
        (ticks, sys_test_idle_ticks().max(0) as u64)
    }

    impl Window {
        pub fn start() -> Self {
            Self {
                at: sample(),
                logged: false,
            }
        }

        /// A request arrived: close the window if the gap before it was long.
        pub fn note_request(&mut self) {
            let now = sample();
            let (ticks, idle) = (now.0 - self.at.0, now.1 - self.at.1);
            if !self.logged && ticks >= MIN_GAP_TICKS {
                self.logged = true;
                let mut line = [0u8; 96];
                let mut n = 0;
                for (label, value) in [
                    (&b"DESKTOP idle window ticks="[..], ticks),
                    (b" idle=", idle),
                    (b" pct=", idle * 100 / ticks),
                ] {
                    line[n..n + label.len()].copy_from_slice(label);
                    n += label.len();
                    n += super::write_dec(&mut line[n..], value as usize);
                }
                line[n] = b'\n';
                super::log(&line[..n + 1]);
            }
            self.at = now;
        }
    }
}

/// DS-1b: the session broker. The WM mints each client a LaunchCap (a
/// channel endpoint + an allowlist); `OP_LAUNCH` carries a token that the
/// client must also send on that endpoint (rendezvous IPC: the broker
/// receives it on its own end). ADVISORY: `sys_spawn` is still ungated in
/// the kernel (OT-66), which the boot test pins.
#[cfg(feature = "desktop-home")]
mod launch {
    #[cfg(feature = "desktop-lens")]
    use super::lens;
    use super::{Scene, grant_surface, log};
    #[cfg(feature = "desktop-lens")]
    use libneo::atrium_launch::MODULE_LENS;
    use libneo::atrium_launch::{LaunchBroker, MODULE_PROBE, STATUS_SPAWN_FAILED, allow};
    use libneo::atrium_protocol::{Identity, LaunchRequest};
    use libneo::{Handle, sys_chan_create, sys_spawn, sys_wait};

    /// How long the broker waits for a client's proof before refusing it
    /// (status 7, as for no proof). Janus DS-1b: with the pointer (DI-1a's
    /// SAK and `SYS_TRY_RECV`) the wait is polled, so the WM keeps serving
    /// the SAK and the pointer meanwhile.
    #[cfg(feature = "desktop-pointer")]
    pub const PROOF_DEADLINE_NS: u64 = 3_000_000_000;

    /// Mint the client's LaunchCap and grant the client end over the
    /// session channel. Returns the broker's end and the WM's copy of the
    /// client end.
    pub fn mint(
        broker: &mut LaunchBroker,
        session: Handle,
        surface_cap: u64,
    ) -> Result<(Handle, Handle), i32> {
        let (broker_end, client_end) = sys_chan_create().map_err(|_| 80)?;
        // DS-1c widens the allowlist to the Lens.
        #[cfg(feature = "desktop-lens")]
        let allowed = allow(MODULE_PROBE) | allow(MODULE_LENS);
        #[cfg(not(feature = "desktop-lens"))]
        let allowed = allow(MODULE_PROBE);
        if !broker.mint(surface_cap, allowed) {
            return Err(81);
        }
        if !grant_surface(session, client_end) {
            return Err(82);
        }
        // The WM keeps its copy of the client end until the session ends:
        // closing a channel handle closes that endpoint side for every
        // holder (`close_peer`), so closing it here would cut Arthur's cap.
        #[cfg(feature = "desktop-lens")]
        log(b"DESKTOP launchcap minted client=arthur allow=probe,lens advisory=true\n");
        #[cfg(not(feature = "desktop-lens"))]
        log(b"DESKTOP launchcap minted client=arthur allow=probe advisory=true\n");
        Ok((broker_end, client_end))
    }

    /// Judge one `OP_LAUNCH`: receive the client's proof on the broker's
    /// own end of its LaunchCap, then check binding, token and allowlist.
    /// Returns the reply status. With the pointer the proof is polled, never
    /// awaited: `idle` runs between polls (the WM's SAK / pointer service;
    /// its error is fatal, as in the main loop), and a proof not presented
    /// within `PROOF_DEADLINE_NS` counts as none. Without it (no SAK, no
    /// `SYS_TRY_RECV`) the WM already blocks on the client's surface channel
    /// — the recorded DS-1a focus trap — and the proof is awaited the same
    /// way.
    pub fn serve(
        broker: &mut LaunchBroker,
        broker_end: &mut Option<Handle>,
        request: LaunchRequest,
        identity: Identity,
        scene: &mut Scene,
        #[cfg(feature = "desktop-lens")] lens: &mut lens::Slot,
        idle: &mut dyn FnMut(&mut Scene) -> Result<(), i32>,
    ) -> Result<u32, i32> {
        let presented = match broker_end.as_ref() {
            Some(&end) => match recv_proof(end, request.module_id, scene, idle)? {
                Proof::Presented(proof) => Some(proof),
                Proof::Absent => None,
                // A proof that missed the deadline costs the client its
                // LaunchCap for this session: the broker's end is closed, so a
                // late send on it fails (EPIPE) instead of blocking the client
                // short of receiving this refusal, which would leave the WM
                // parked in the reply's rendezvous (AAG, Janus DS-1b).
                Proof::Late => {
                    let _ = libneo::sys_channel_close(end);
                    *broker_end = None;
                    broker.revoke(identity.surface_cap_provenance);
                    log_launch(b"revoked", request.module_id, 0);
                    None
                }
            },
            None => None,
        };
        let judged = broker.judge(request, identity.surface_cap_provenance, presented);
        // DS-1c: the Lens is served here, inside Arthur's request (Arthur
        // is blocked on the reply): one Lens per session, and (Chad DS-1c
        // F1) at most `MAX_LENS_ATTEMPTS` blooms tried, a failed one spending
        // its attempt.
        #[cfg(feature = "desktop-lens")]
        if judged.is_ok() && request.module_id == MODULE_LENS {
            use libneo::atrium_launch::{LensAttempt, STATUS_ATTEMPTS_SPENT};
            let attempt = match lens.attempts.begin() {
                LensAttempt::Bloom(n) => n,
                LensAttempt::AlreadyShown => {
                    log_launch(b"spawn-failed", request.module_id, 1);
                    return Ok(STATUS_SPAWN_FAILED);
                }
                LensAttempt::Spent => {
                    log_launch(
                        b"attempts-spent",
                        request.module_id,
                        u64::from(lens.attempts.used()),
                    );
                    return Ok(STATUS_ATTEMPTS_SPENT);
                }
            };
            #[cfg(feature = "desktop-resize")]
            scene.slide_for_lens()?;
            return Ok(match lens::bloom(scene, attempt) {
                Ok(session) => {
                    #[cfg(feature = "desktop-resize")]
                    scene.refit_pending_for_dock();
                    lens.session = Some(session);
                    lens.attempts.shown();
                    log_launch(b"granted", request.module_id, 0);
                    0
                }
                Err(code) => {
                    log_launch(b"spawn-failed", request.module_id, code as u64);
                    STATUS_SPAWN_FAILED
                }
            });
        }
        Ok(match judged {
            Ok(name) => match sys_spawn(name, &[]) {
                Ok((_pid, process)) => {
                    // The probe ends at once; waiting keeps the boot test
                    // deterministic. (A launched app would not be waited on.)
                    let exit = if request.module_id == MODULE_PROBE {
                        sys_wait(process, 0).unwrap_or(-1)
                    } else {
                        0
                    };
                    log_launch(b"granted", request.module_id, exit as u64);
                    0
                }
                Err(_) => {
                    log_launch(b"spawn-failed", request.module_id, 0);
                    STATUS_SPAWN_FAILED
                }
            },
            Err(refusal) => {
                log_launch(b"refused", request.module_id, refusal.status() as u64);
                refusal.status()
            }
        })
    }

    enum Proof {
        Presented(u64),
        /// The client closed its cap (or sent something else).
        Absent,
        /// Nothing within the deadline.
        Late,
    }

    /// Poll the broker's end for the 8-byte proof until it arrives, the
    /// client closes its cap (`EPIPE`: no proof), or the deadline passes.
    #[cfg(feature = "desktop-pointer")]
    fn recv_proof(
        broker_end: Handle,
        module: u32,
        scene: &mut Scene,
        idle: &mut dyn FnMut(&mut Scene) -> Result<(), i32>,
    ) -> Result<Proof, i32> {
        use libneo::{CLOCK_MONOTONIC, EAGAIN, sys_clock_gettime, sys_try_recv};
        let now = || sys_clock_gettime(CLOCK_MONOTONIC).unwrap_or(u64::MAX);
        let deadline = now().saturating_add(PROOF_DEADLINE_NS);
        let mut waited = false;
        loop {
            let mut proof = [0u8; 8];
            match sys_try_recv(broker_end, &mut proof) {
                8 => return Ok(Proof::Presented(u64::from_le_bytes(proof))),
                EAGAIN => {}
                _ => return Ok(Proof::Absent),
            }
            if !waited {
                waited = true;
                // Boot-test witness: the broker is now polling for a proof.
                #[cfg(feature = "desktop-session-boot-test")]
                log_launch(b"awaiting-proof", module, 0);
            }
            if now() >= deadline {
                log_launch(b"proof-timeout", module, PROOF_DEADLINE_NS / 1_000_000);
                return Ok(Proof::Late);
            }
            idle(scene)?;
            // OT-22: park on the broker's end (the proof, or the client
            // closing its cap), the route and the SAK latch, until the
            // deadline. The client runs while this thread is parked.
            #[cfg(feature = "desktop-park")]
            scene.park(
                &[libneo::WaitItem {
                    handle: broker_end.0,
                    kind: libneo::WAIT_KIND_CHANNEL,
                }],
                deadline,
            );
            // Yield, not spin: receiving `OP_LAUNCH` woke the client on this
            // CPU (`wake_thread_on_current_cpu`), and the proof can only come
            // once it runs.
            #[cfg(not(feature = "desktop-park"))]
            libneo::sys_sched_yield();
        }
    }

    /// Without the pointer: await the proof (a closed cap is no proof).
    #[cfg(not(feature = "desktop-pointer"))]
    fn recv_proof(
        broker_end: Handle,
        _module: u32,
        _scene: &mut Scene,
        _idle: &mut dyn FnMut(&mut Scene) -> Result<(), i32>,
    ) -> Result<Proof, i32> {
        let mut proof = [0u8; 8];
        Ok(if libneo::sys_recv(broker_end, &mut proof) == 8 {
            Proof::Presented(u64::from_le_bytes(proof))
        } else {
            Proof::Absent
        })
    }

    fn log_launch(verdict: &[u8], module: u32, value: u64) {
        super::Line::new()
            .s(b"DESKTOP launchcap ")
            .s(verdict)
            .s(b" module=")
            .n(u64::from(module))
            .s(b" value=")
            .n(value)
            .s(b"\n")
            .emit();
    }
}

fn grant_surface(session: Handle, client_endpoint: Handle) -> bool {
    grant_over(session, client_endpoint, FULL_CHANNEL_RIGHTS)
}

/// Lend `handle` with `rights` over `chan`: offer `O`, the peer's ready
/// `R`, the grant, its ack `A` (each step only after the one before).
fn grant_over(chan: Handle, handle: Handle, rights: CapRights) -> bool {
    sys_send(chan, b"O") == 1
        && recv_control(chan, b'R')
        && sys_grant(chan, handle, rights) == 0
        && recv_control(chan, b'A')
}

#[cfg(not(feature = "login"))]
fn recv_control(session: Handle, expected: u8) -> bool {
    let mut byte = [0u8; 1];
    sys_recv(session, &mut byte) == 1 && byte[0] == expected
}

/// OT-66 L2 (AAG F3): polled, so a client that never answers can't keep the
/// lock from locking the input meanwhile (`lock::guard_input`; these waits
/// can't reach the scene, so the lock screen is painted after them). A kernel
/// without `SYS_TRY_RECV` waits as before.
#[cfg(feature = "login")]
fn recv_control(session: Handle, expected: u8) -> bool {
    let mut byte = [0u8; 1];
    loop {
        match libneo::sys_try_recv(session, &mut byte) {
            1 => return byte[0] == expected,
            libneo::EAGAIN => {
                lock::guard_input();
                core::hint::spin_loop();
            }
            libneo::ENOSYS => return sys_recv(session, &mut byte) == 1 && byte[0] == expected,
            _ => return false,
        }
    }
}

/// `recv_control` that never parks: Home's pointer ticks while the peer is
/// slow to answer (a child still starting up). Without the pointer it is
/// `recv_control`.
fn poll_control(session: Handle, expected: u8, scene: &mut Scene) -> Result<bool, i32> {
    #[cfg(not(feature = "desktop-pointer"))]
    {
        let _ = scene;
        Ok(recv_control(session, expected))
    }
    #[cfg(feature = "desktop-pointer")]
    loop {
        let mut byte = [0u8; 1];
        match libneo::sys_try_recv(session, &mut byte) {
            1 => return Ok(byte[0] == expected),
            libneo::EAGAIN => {
                // OT-66 L2 (AAG F3): a client slow to answer can't keep the
                // lock from engaging.
                #[cfg(feature = "login")]
                lock::service(scene)?;
                #[cfg(not(feature = "desktop-intro"))]
                scene.pointer_idle();
                #[cfg(feature = "desktop-intro")]
                intro::pointer_tick(scene)?;
                // OT-87 R4.4b: a press on the Install chip is not taken
                // here (no focus).
                #[cfg(all(feature = "installer", feature = "desktop-intro"))]
                {
                    scene.intro.install_press = false;
                }
                core::hint::spin_loop();
            }
            _ => return Ok(false),
        }
    }
}

fn log(message: &[u8]) {
    let _ = sys_write(1, message.as_ptr(), message.len());
}

/// One log line from byte fragments and decimal numbers.
struct Line {
    buf: [u8; 256],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Self {
            buf: [0; 256],
            len: 0,
        }
    }

    fn s(mut self, text: &[u8]) -> Self {
        let n = text.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&text[..n]);
        self.len += n;
        self
    }

    fn n(mut self, value: u64) -> Self {
        if self.buf.len() - self.len >= 20 {
            self.len += write_dec(&mut self.buf[self.len..], value as usize);
        }
        self
    }

    /// A signed decimal (`-` then the magnitude).
    fn i(self, value: i64) -> Self {
        let line = if value < 0 { self.s(b"-") } else { self };
        line.n(value.unsigned_abs())
    }

    fn emit(self) {
        log(&self.buf[..self.len]);
    }
}

/// Desktop S3: choosing the material and building the glass chrome.
#[cfg(feature = "desktop-glass")]
mod glass {
    use super::{CLIENT_H, CLIENT_W, Line};
    use libneo::{FileRights, OPEN_ROOT_TEMPORARY, sys_close, sys_open};
    use neo_atrium::{BaseScene, Mode};

    /// The persistent accessibility preference: present ⇒ Reduce
    /// Transparency at every boot. A sticky user choice, deliberately distinct
    /// from any automatic (GPU/perf) degrade (ADR-024 Amendment 2026-07-19 §3).
    pub const RT_PREFERENCE: &str = "/neofs/atrium-reduce-transparency";

    fn material() -> (Mode, &'static [u8]) {
        if cfg!(feature = "reduce-transparency") {
            return (Mode::ReduceTransparency, b"build-feature");
        }
        #[cfg(feature = "atrium-settings-store")]
        return if super::settings::current().reduce_transparency {
            (Mode::ReduceTransparency, b"settings")
        } else {
            (Mode::Glass, b"settings")
        };
        #[cfg_attr(feature = "atrium-settings-store", allow(unreachable_code))]
        match sys_open(RT_PREFERENCE, OPEN_ROOT_TEMPORARY, FileRights::READ) {
            Ok(handle) => {
                let _ = sys_close(handle);
                (Mode::ReduceTransparency, b"user-preference")
            }
            Err(_) => (Mode::Glass, b"default"),
        }
    }

    pub fn build(width: usize, height: usize) -> Option<BaseScene> {
        let (mode, cause) = material();
        let name: &[u8] = match mode {
            Mode::Glass => b"glass",
            Mode::ReduceTransparency => b"reduce-transparency",
        };
        Line::new()
            .s(b"DESKTOP material=")
            .s(name)
            .s(b" cause=")
            .s(cause)
            .s(b"\n")
            .emit();
        #[cfg(feature = "desktop-lens")]
        return BaseScene::with_lens(width, height, CLIENT_W, CLIENT_H, mode);
        #[cfg(all(feature = "desktop-home", not(feature = "desktop-lens")))]
        return BaseScene::with_home(width, height, CLIENT_W, CLIENT_H, mode);
        #[cfg(not(feature = "desktop-home"))]
        BaseScene::new(width, height, CLIENT_W, CLIENT_H, mode)
    }

    /// DI-1b: `DESKTOP pointer move from=X,Y to=X,Y client_at=X,Y
    /// blur_regions_total=N residue_pixels=R` (DL-1, a docked Lens:
    /// ` lens_at=X,Y lens_mismatch=M`). Boot-test only (production does not
    /// log where windows go). The residue witness: every base pixel of the
    /// vacated and the new focus-light rects (and docked, Lens halos; the new
    /// label boxes aside) equals the window view recomputed from the scene
    /// definition.
    #[cfg(feature = "desktop-pointer")]
    pub fn log_move(scene: &super::Scene, from: (usize, usize), old: [super::Rect; 2]) {
        #[cfg(feature = "desktop-session-boot-test")]
        {
            let l = &scene.chrome.layout;
            let line = super::Line::new()
                .s(b"DESKTOP pointer move from=")
                .n(from.0 as u64)
                .s(b",")
                .n(from.1 as u64)
                .s(b" to=")
                .n(l.frame.x as u64)
                .s(b",")
                .n(l.frame.y as u64)
                .s(b" client_at=")
                .n(l.client.x as u64)
                .s(b",")
                .n(l.client.y as u64)
                .s(b" blur_regions_total=")
                .n(u64::from(neo_atrium::blur::blur_calls()))
                .s(b" residue_pixels=")
                .n(reframe_residue(scene, old) as u64);
            docked_suffix(scene, line).s(b"\n").emit();
        }
        #[cfg(not(feature = "desktop-session-boot-test"))]
        let _ = (scene, from, old);
    }

    /// DI-1b / DL-1 boot-test witness: base pixels, where the frame (and a
    /// docked Lens's halo) was and is, that differ from the window view
    /// recomputed from the scene (the label boxes, drawn over it, aside).
    #[cfg(all(feature = "desktop-pointer", feature = "desktop-session-boot-test"))]
    pub fn reframe_residue(scene: &super::Scene, old: [super::Rect; 2]) -> usize {
        let l = &scene.chrome.layout;
        let view = neo_atrium::View::Window {
            focused: scene.focused,
        };
        let mut labels = [neo_atrium::Rect::default(); 4];
        labels[..3].copy_from_slice(&l.label_boxes());
        if let Some(lens) = l.lens.filter(|_| l.lens_docked) {
            labels[3] = lens.title_label;
        }
        let stride = l.screen.w;
        let base = scene.base.slice();
        let mut residue = 0usize;
        for r in old.into_iter().chain(scene.reach()) {
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    if labels.iter().any(|b| b.contains(x, y)) {
                        continue;
                    }
                    let want = neo_atrium::wide::dither(scene.chrome.view_pixel(x, y, view), x, y);
                    residue += usize::from(base[y * stride + x] != want);
                }
            }
        }
        residue
    }

    /// DL-1 boot-test witness: ` lens_at=X,Y lens_mismatch=M` with the Lens
    /// docked — where its frame went and how many presented pixels of its
    /// client differ from the WM's copy of its picture (sprite aside).
    #[cfg(all(feature = "desktop-pointer", feature = "desktop-session-boot-test"))]
    pub fn docked_suffix(scene: &super::Scene, line: super::Line) -> super::Line {
        #[cfg(feature = "desktop-lens")]
        if let (Some(lens), true, Some(copy)) = (
            scene.chrome.layout.lens,
            scene.chrome.layout.lens_docked,
            scene.lens.as_ref(),
        ) {
            let c = lens.client;
            let stride = scene.geo.screen.w;
            let sprite = scene
                .sprite_at
                .map(|at| super::pointer::sprite_rect(at, scene.sprite_shape, scene.geo.screen))
                .unwrap_or_default();
            let (out, picture) = (scene.out.slice(), copy.copy.slice());
            let mut mismatch = 0usize;
            for y in 0..c.h {
                for x in 0..c.w {
                    if sprite.contains(c.x + x, c.y + y) {
                        continue;
                    }
                    let shown = out[(c.y + y) * stride + c.x + x] & 0x00FF_FFFF;
                    mismatch += usize::from(shown != picture[y * c.w + x] & 0x00FF_FFFF);
                }
            }
            return line
                .s(b" lens_at=")
                .n(lens.frame.x as u64)
                .s(b",")
                .n(lens.frame.y as u64)
                .s(b" lens_mismatch=")
                .n(mismatch as u64);
        }
        #[cfg(not(feature = "desktop-lens"))]
        let _ = scene;
        line
    }

    pub fn log_scene(scene: &BaseScene, build_cycles: u64, first_frame_cycles: u64) {
        let l = &scene.layout;
        Line::new()
            .s(b"DESKTOP scene full_screen=")
            .n(l.screen.w as u64)
            .s(b"x")
            .n(l.screen.h as u64)
            .s(b" s3=atrium client_at=")
            .n(l.client.x as u64)
            .s(b",")
            .n(l.client.y as u64)
            .s(b" blur_regions=")
            .n(neo_atrium::blur::blur_calls() as u64)
            .s(b" owner=wm stale_pixels=0\n")
            .emit();
        Line::new()
            .s(b"DESKTOP perf build_cycles=")
            .n(build_cycles)
            .s(b" full_frame_cycles=")
            .n(first_frame_cycles)
            .s(b" (1 full-screen compose+present)\n")
            .emit();
    }
}

/// DI-1b resize: the WM side of the configure mailbox
/// (`libneo::atrium_configure`). IPC is a synchronous rendezvous, so the WM
/// never sends a configure (it would park until the client received, and
/// deadlock against a client parked in its own commit): it publishes one,
/// and the client acks with an `OP_CONFIGURE` request.
#[cfg(feature = "desktop-resize")]
mod resize {
    use super::{Plane, Rect, Scene, log};
    use core::sync::atomic::AtomicU32;
    use libneo::atrium_configure::{MAILBOX_WORDS, Published, publish, words};
    use libneo::{ATRIUM_MAP_READ, ATRIUM_QUERY, CapRights, Handle, meta_rights};

    /// What the WM has published and waits on.
    #[derive(Clone, Copy)]
    pub struct Pending {
        pub serial: u32,
        pub x: usize,
        pub y: usize,
        pub w: usize,
        pub h: usize,
    }

    pub struct Configure {
        words: &'static [AtomicU32; MAILBOX_WORDS],
        next: u32,
        pub pending: Option<Pending>,
    }

    impl Configure {
        /// Publish the client size `w` x `h` (the frame's new origin `(x, y)`
        /// stays the WM's: it is never published).
        pub fn publish(&mut self, x: usize, y: usize, w: usize, h: usize) {
            let serial = self.next;
            // #53 nit: wraps to 1 past SERIAL_MAX (never 0, never pinned).
            self.next = libneo::atrium_configure::next_serial(self.next);
            publish(
                self.words,
                Published {
                    serial,
                    width: w as u32,
                    height: h as u32,
                },
            );
            self.pending = Some(Pending { serial, x, y, w, h });
        }
    }

    /// Create this session's mailbox and grant it to the client read-only
    /// over the session channel (offer / ready / grant / ack). Returns it and
    /// its handle (closed with the session).
    pub fn mint(session: Handle) -> Result<(Configure, Handle), i32> {
        // A 4x1 XRGB8888 buffer is exactly the 16-byte mailbox layout.
        let (handle, plane) = Plane::create(4, 1).map_err(|_| 93)?;
        let read_only = CapRights::new(meta_rights::USE, ATRIUM_MAP_READ | ATRIUM_QUERY);
        if !super::grant_over(session, handle, read_only) {
            return Err(94);
        }
        // SAFETY: the WM's RW mapping of its own 16-byte buffer, page
        // aligned and never unmapped (one page per session: no unmap yet);
        // the client reads it, every access is atomic.
        let words = unsafe { words(plane.pixels as *const u8) };
        log(b"DESKTOP configure mailbox granted client=arthur rights=read-only\n");
        Ok((
            Configure {
                words,
                next: 1,
                pending: None,
            },
            handle,
        ))
    }

    pub fn log_request(edges: u8, w: usize, h: usize, serial: u32) {
        let mut line = super::Line::new().s(b"DESKTOP pointer resize request edges=");
        for (bit, name) in [(1u8, b'L'), (2, b'R'), (4, b'T'), (8, b'B')] {
            if edges & bit != 0 {
                line = line.s(&[name]);
            }
        }
        line.s(b" size=")
            .n(w as u64)
            .s(b"x")
            .n(h as u64)
            .s(b" serial=")
            .n(u64::from(serial))
            .s(b" published=mailbox\n")
            .emit();
    }

    /// DL-1 fix-forward: `DESKTOP pointer resize refit from=WxH to=WxH
    /// serial=N docked=lens`: a pending size the docked group cannot hold,
    /// re-published at the docked maximum.
    pub fn log_refit(from: (usize, usize), to: (usize, usize), serial: u32) {
        super::Line::new()
            .s(b"DESKTOP pointer resize refit from=")
            .n(from.0 as u64)
            .s(b"x")
            .n(from.1 as u64)
            .s(b" to=")
            .n(to.0 as u64)
            .s(b"x")
            .n(to.1 as u64)
            .s(b" serial=")
            .n(u64::from(serial))
            .s(b" docked=lens\n")
            .emit();
    }

    /// `DESKTOP pointer resize from=WxH to=WxH frame_at=X,Y serial=N
    /// blur_regions_total=B residue_pixels=R client_mismatch=M
    /// footer_rule=yes|no` (boot test): the base equals the scene's own
    /// pixels where the frame was and is (R), the presented client rect is
    /// exactly the client's new picture (M, sprite excluded), and that
    /// picture is laid out at the new size (its footer rule, an independent
    /// layout rule: row h - 36, from x = 64 to w - 24, `structure-inactive`).
    pub fn log_applied(
        scene: &Scene,
        from: (usize, usize),
        serial: u32,
        old: [Rect; 2],
        client: &super::SharedPixels,
    ) {
        #[cfg(feature = "desktop-session-boot-test")]
        {
            const STRUCTURE_INACTIVE: u32 = 0x001E_3558;
            let l = &scene.chrome.layout;
            let residue = super::glass::reframe_residue(scene, old);
            let stride = l.screen.w;
            let c = scene.geo.client;
            let sprite = scene
                .sprite_at
                .map(|at| super::pointer::sprite_rect(at, scene.sprite_shape, scene.geo.screen))
                .unwrap_or_default();
            // Against the client's own buffer, read now (it is parked for the
            // reply, so it holds the acked picture), never the WM's copy: a WM
            // that skipped the copy would show the old picture and still match
            // its copy.
            let out = scene.out.slice();
            let mut mismatch = 0usize;
            let mut footer = true;
            let rule = c.h - 36;
            for y in 0..c.h {
                for x in 0..c.w {
                    let (sx, sy) = (c.x + x, c.y + y);
                    if sprite.contains(sx, sy) {
                        continue;
                    }
                    let shown = out[sy * stride + sx] & 0x00FF_FFFF;
                    let live = client.load(x, y).map(|p| p & 0x00FF_FFFF);
                    mismatch += usize::from(live != Some(shown));
                    if y == rule && (64..c.w - 24).contains(&x) && shown != STRUCTURE_INACTIVE {
                        footer = false;
                    }
                }
            }
            let line = super::Line::new()
                .s(b"DESKTOP pointer resize from=")
                .n(from.0 as u64)
                .s(b"x")
                .n(from.1 as u64)
                .s(b" to=")
                .n(c.w as u64)
                .s(b"x")
                .n(c.h as u64)
                .s(b" frame_at=")
                .n(l.frame.x as u64)
                .s(b",")
                .n(l.frame.y as u64)
                .s(b" serial=")
                .n(u64::from(serial))
                .s(b" blur_regions_total=")
                .n(u64::from(neo_atrium::blur::blur_calls()))
                .s(b" residue_pixels=")
                .n(residue as u64)
                .s(b" client_mismatch=")
                .n(mismatch as u64)
                .s(if footer {
                    b" footer_rule=yes"
                } else {
                    b" footer_rule=no"
                });
            super::glass::docked_suffix(scene, line).s(b"\n").emit();
        }
        #[cfg(not(feature = "desktop-session-boot-test"))]
        let _ = (scene, from, serial, old, client);
    }
}

/// Desktop S3: cycle counts for the frame budget, and the re-blur witness.
#[cfg(feature = "desktop-glass")]
mod perf {
    use super::Line;

    pub fn now() -> u64 {
        // SAFETY: RDTSC is unprivileged (CR4.TSD is clear) and has no side
        // effects; it is only used for reporting.
        unsafe { core::arch::x86_64::_rdtsc() }
    }

    /// Damage frames (Arthur commits — typing) since the scene was built.
    pub struct FrameStats {
        count: u64,
        min: u64,
        max: u64,
        total: u64,
        blurs_at_start: u32,
        moves_at_start: u32,
    }

    impl Default for FrameStats {
        fn default() -> Self {
            Self {
                count: 0,
                min: u64::MAX,
                max: 0,
                total: 0,
                blurs_at_start: neo_atrium::blur::blur_calls(),
                moves_at_start: super::move_blurs(),
            }
        }
    }

    impl FrameStats {
        pub fn record(&mut self, cycles: u64) {
            self.count += 1;
            self.min = self.min.min(cycles);
            self.max = self.max.max(cycles);
            self.total += cycles;
            // A resident session never ends; report once it has some history.
            if self.count == 32 {
                self.log();
            }
        }

        pub fn log(&self) {
            // DI-1b: a completed window move re-blurs the frame exactly once;
            // those are counted apart, and anything else is a re-blur.
            let moves = super::move_blurs() - self.moves_at_start;
            let reblurs = neo_atrium::blur::blur_calls() - self.blurs_at_start - moves;
            Line::new()
                .s(b"DESKTOP perf damage_frames=")
                .n(self.count)
                .s(b" damage_frame_cycles_min=")
                .n(if self.count == 0 { 0 } else { self.min })
                .s(b" max=")
                .n(self.max)
                .s(b" mean=")
                .n(self.total.checked_div(self.count).unwrap_or(0))
                .s(b" reblurs_during_typing=")
                .n(reblurs as u64)
                .s(b" move_blurs=")
                .n(moves as u64)
                .s(b"\n")
                .emit();
        }
    }
}

/// DM-1 motion: every view, Lens and focus-light change of the window
/// manager plays as a short transition over its own damage rects
/// (`neo_atrium::motion`, spec v1.7), ending on the change's ordinary
/// present. Off (feature `desktop-motion` off) = the presents alone.
#[cfg(feature = "desktop-motion")]
mod motion {
    use super::{Line, Rect, Scene};
    use libneo::{
        CLOCK_MONOTONIC, FileRights, OPEN_ROOT_TEMPORARY, sys_clock_gettime, sys_close, sys_open,
    };
    use neo_atrium::motion::{self, FRAME_NS, Kind, Profile};

    /// The persistent accessibility preference: present ⇒ Reduce Motion at
    /// every boot (like Reduce Transparency's).
    pub const RM_PREFERENCE: &str = "/neofs/atrium-reduce-motion";

    /// The most rects one transition carries (the view switch's five, the
    /// focus light's six, seven at Arthur's exit with its client).
    const MAX_RECTS: usize = 8;

    /// The boot test's mid-transition frame (the oracle's `HOLD_MID_*_FRAME`).
    #[cfg(feature = "desktop-motion-boot-test")]
    pub const HOLD_FRAME: u32 = 3;

    pub fn profile() -> Profile {
        let (profile, cause): (Profile, &[u8]) = if cfg!(feature = "reduce-motion") {
            (Profile::Reduced, b"build-feature")
        } else if cfg!(feature = "atrium-settings-store") {
            #[cfg(feature = "atrium-settings-store")]
            let profile = super::settings::profile(super::settings::current().reduce_motion);
            #[cfg(not(feature = "atrium-settings-store"))]
            let profile = Profile::Full;
            (profile, b"settings")
        } else {
            match sys_open(RM_PREFERENCE, OPEN_ROOT_TEMPORARY, FileRights::READ) {
                Ok(handle) => {
                    let _ = sys_close(handle);
                    (Profile::Reduced, b"user-preference")
                }
                Err(_) => (Profile::Full, b"default"),
            }
        };
        Line::new()
            .s(b"DESKTOP motion profile=")
            .s(name(profile))
            .s(b" cause=")
            .s(cause)
            .s(b"\n")
            .emit();
        profile
    }

    pub fn name(profile: Profile) -> &'static [u8] {
        match profile {
            Profile::Full => b"full",
            Profile::Reduced => b"reduced",
            Profile::Off => b"off",
        }
    }

    use super::atrium_rect as na;

    /// The monotonic clock (10 ms resolution); `None` if the kernel has
    /// none, and then nothing is paced (the boot test's witness shows it:
    /// `elapsed_ms` below `paced_ms` fails the scenario, never a hang).
    fn now() -> Option<u64> {
        sys_clock_gettime(CLOCK_MONOTONIC).ok()
    }

    /// The most yields one frame waits (AAG: a clock that stops advancing
    /// must not hang the WM inside a handoff; far more than a 16.7 ms
    /// period takes anywhere it ticks).
    const MAX_YIELDS: u32 = 100_000;

    /// Yield until the monotonic clock reaches `deadline`.
    fn wait_until(deadline: u64) {
        let mut yields = 0;
        while yields < MAX_YIELDS && now().is_some_and(|t| t < deadline) {
            libneo::sys_sched_yield();
            yields += 1;
        }
    }

    /// What one transition did (the boot test's witness line).
    pub struct Run {
        kind: Kind,
        frames: u32,
        rects: [Rect; MAX_RECTS],
        count: usize,
        start: u64,
        presented: u32,
        early: u32,
    }

    impl Scene {
        /// Play frames `1..N` of `kind` over `rects` (frame `N` is the
        /// caller's present). `None`: nothing to animate (one frame).
        pub(super) fn animate(
            &mut self,
            kind: Kind,
            anchor: Rect,
            rects: &[Rect],
            client: bool,
        ) -> Result<Option<Run>, i32> {
            let plan = motion::plan(self.motion_profile, kind);
            let screen = self.geo.screen;
            let mut set = [Rect::default(); MAX_RECTS];
            let mut count = 0;
            for r in rects {
                let r = r.intersect(&screen);
                if r.is_empty() {
                    continue;
                }
                // More rects than a transition carries: no motion at all
                // (the presents alone), never a partial one (AAG).
                if count == MAX_RECTS {
                    return Ok(None);
                }
                set[count] = r;
                count += 1;
            }
            if plan.frames <= 1 || count == 0 {
                return Ok(None);
            }
            // A sprite erase rides on the end frame, not on these — and is
            // kept for it whatever happens to them (AAG).
            #[cfg(feature = "desktop-pointer")]
            let erase = core::mem::take(&mut self.extra_damage);
            let run = self.play(kind, anchor, &set[..count], client, plan);
            #[cfg(feature = "desktop-pointer")]
            {
                self.extra_damage = self.extra_damage.union(&erase);
            }
            run.map(Some)
        }

        /// Frames `1..N` of `plan` over `set` (see [`Self::animate`]).
        fn play(
            &mut self,
            kind: Kind,
            anchor: Rect,
            set: &[Rect],
            client: bool,
            plan: motion::Plan,
        ) -> Result<Run, i32> {
            let count = set.len();
            let stride = self.geo.screen.w;
            // F: what is on screen. T: the change, composed, not presented.
            for &r in set {
                copy_rect(self.motion_from.slice_mut(), self.out.slice(), stride, r);
            }
            for &r in set {
                self.compose(client, r).map_err(|c| 100 + c)?;
                copy_rect(self.motion_to.slice_mut(), self.out.slice(), stride, r);
            }
            let bounds = set.iter().fold(Rect::default(), |b, r| b.union(r));
            let n = plan.frames;
            let start = now().unwrap_or(0);
            // Witnesses: rect presents made, and frames shown before their
            // own deadline (each frame is paced, not only the total).
            let (mut presented, mut early) = (0u32, 0u32);
            for k in 1..n {
                let deadline = start + u64::from(k - 1) * FRAME_NS;
                wait_until(deadline);
                if now().is_none_or(|t| t < deadline) {
                    early += 1;
                }
                let iris = motion::iris(plan.shape, na(anchor), na(bounds), k, n);
                for &r in set {
                    motion::frame(
                        self.out.slice_mut(),
                        self.motion_from.slice(),
                        self.motion_to.slice(),
                        stride,
                        na(r),
                        plan.shape,
                        iris,
                        k,
                        n,
                    );
                    self.commit_present(r).map_err(|c| 100 + c)?;
                    presented += 1;
                }
                #[cfg(feature = "desktop-motion-boot-test")]
                self.hold_point(kind, k, n);
            }
            let last = start + u64::from(n - 1) * FRAME_NS;
            wait_until(last);
            if now().is_none_or(|t| t < last) {
                early += 1;
            }
            let mut rects = [Rect::default(); MAX_RECTS];
            rects[..count].copy_from_slice(set);
            Ok(Run {
                kind,
                frames: n,
                rects,
                count,
                start,
                presented,
                early,
            })
        }

        /// Boot test: hold the first Home ↔ window transition armed by
        /// [`Scene::arm_motion_hold`] at frame [`HOLD_FRAME`] until a key
        /// reaches the WM's own focus (consumed: it is the scenario's
        /// "captured" signal, never Home's input).
        #[cfg(feature = "desktop-motion-boot-test")]
        fn hold_point(&mut self, kind: Kind, k: u32, n: u32) {
            if kind != Kind::View || k != HOLD_FRAME {
                return;
            }
            let Some(events) = self.motion_hold.take() else {
                return;
            };
            Line::new()
                .s(b"DESKTOP motion hold kind=view profile=")
                .s(name(self.motion_profile))
                .s(b" frame=")
                .n(u64::from(k))
                .s(b"/")
                .n(u64::from(n))
                .s(b"\n")
                .emit();
            loop {
                let mut event = [0u8; libneo::INPUT_EVENT_V1_LEN];
                match libneo::sys_input_focus_recv(events, &mut event) {
                    0 if neo_atrium::keys::decode(event).is_some() => break,
                    0 => {}
                    libneo::EAGAIN => core::hint::spin_loop(),
                    _ => break,
                }
            }
        }

        /// Boot test: hold the next Home ↔ window transition (see
        /// [`Self::hold_point`]) on `events`.
        #[cfg(feature = "desktop-motion-boot-test")]
        pub(super) fn arm_motion_hold(&mut self, events: libneo::Handle) {
            self.motion_hold = Some(events);
        }

        /// Boot test: `DESKTOP motion kind=K profile=P frames=N rects=R
        /// presented=Q frame_px=X screen_px=S elapsed_ms=E paced_ms=M early=0
        /// uncovered=0 end_mismatch=0`.
        /// * `presented`: rect presents made (`(N − 1)·R`: no frame skipped);
        /// * `frame_px`: the pixels each frame recomposes and presents (the
        ///   rects; bounded damage, never the screen);
        /// * `paced_ms`: the least the pacing allows, `(N − 1)·P`; `early`:
        ///   frames shown before their own deadline;
        /// * `uncovered`: animated rects not inside one of the end frame's
        ///   presents — a blended frame the end frame would leave on screen;
        /// * `end_mismatch`: end-frame pixels differing from the change
        ///   composed before the first frame (state drift during the frames).
        #[cfg(feature = "desktop-session-boot-test")]
        pub(super) fn log_motion(&self, run: &Run, finals: &[Rect]) {
            let elapsed = now().unwrap_or(0).saturating_sub(run.start);
            let stride = self.geo.screen.w;
            let (out, to) = (self.out.slice(), self.motion_to.slice());
            let mut frame_px = 0u64;
            let mut mismatch = 0u64;
            for r in &run.rects[..run.count] {
                frame_px += (r.w * r.h) as u64;
                for y in r.y..r.y + r.h {
                    let row = y * stride;
                    for i in row + r.x..row + r.x + r.w {
                        if out[i] != to[i] {
                            mismatch += 1;
                        }
                    }
                }
            }
            let screen = self.geo.screen;
            let inside = |r: &Rect, f: &Rect| {
                r.x >= f.x && r.y >= f.y && r.x + r.w <= f.x + f.w && r.y + r.h <= f.y + f.h
            };
            let uncovered = run.rects[..run.count]
                .iter()
                .filter(|r| !finals.iter().any(|f| inside(r, &f.intersect(&screen))))
                .count();
            Line::new()
                .s(b"DESKTOP motion kind=")
                .s(run.kind.name())
                .s(b" profile=")
                .s(name(self.motion_profile))
                .s(b" frames=")
                .n(u64::from(run.frames))
                .s(b" rects=")
                .n(run.count as u64)
                .s(b" presented=")
                .n(u64::from(run.presented))
                .s(b" frame_px=")
                .n(frame_px)
                .s(b" screen_px=")
                .n((screen.w * screen.h) as u64)
                .s(b" elapsed_ms=")
                .n(elapsed / 1_000_000)
                .s(b" paced_ms=")
                .n(u64::from(run.frames - 1) * FRAME_NS / 1_000_000)
                .s(b" early=")
                .n(u64::from(run.early))
                .s(b" uncovered=")
                .n(uncovered as u64)
                .s(b" end_mismatch=")
                .n(mismatch)
                .s(b"\n")
                .emit();
        }
    }

    fn copy_rect(dst: &mut [u32], src: &[u32], stride: usize, r: Rect) {
        for y in r.y..r.y + r.h {
            let row = y * stride + r.x;
            dst[row..row + r.w].copy_from_slice(&src[row..row + r.w]);
        }
    }
}

/// OT-66 L1: sign-in, before Home.
#[cfg(feature = "login")]
mod login;

/// OT-66 L2: lock and unlock, in a signed-in session.
#[cfg(feature = "login")]
mod lock;

/// L1.3a: Arthur's actions behind the WM's permission prompt.
#[cfg(feature = "arthur-actions")]
mod permission;
/// OT-88 S1: the settings store, the WM its only writer.
#[cfg(feature = "atrium-settings-store")]
mod settings;

/// L2.1: the WM's hash-chained action log.
#[cfg(feature = "arthur-action-log")]
mod action_log;

/// DS-1 Home: the intent field. The WM holds its OWN input focus while Home
/// is shown and reads only those keys — never a client's (the WM-blind
/// property is unchanged: Arthur's focus handle stays ACTIVATE-only here).
#[cfg(feature = "desktop-home")]
mod home {
    use super::{Rect, Scene, log};
    use libneo::atrium_handoff::{HANDOFF, Steps, hand_over};
    use libneo::{
        EAGAIN, Handle, INPUT_EVENT_V1_LEN, sys_input_focus_activate, sys_input_focus_create,
        sys_input_focus_mint_activator, sys_input_focus_recv, sys_send,
    };
    use neo_atrium::keys::{self, Key, Line};
    use neo_atrium::motion::Kind;

    /// The longest intent the field holds (one Arthur input line fits it).
    pub const MAX_INTENT: usize = 120;

    /// Boot test (DM-1): only the session's first hold stops mid-frame.
    #[cfg(feature = "desktop-motion-boot-test")]
    static MOTION_HELD: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

    pub struct HomeState {
        pub focused: bool,
        /// Arthur yielded and is held (blocked on its session channel).
        pub held: bool,
        /// The keyboard is on the Spine chip (Tab), not the field.
        pub chip_selected: bool,
        pub line: Line<MAX_INTENT>,
        /// OT-66 L1: the field holds a password; it paints a dot per byte.
        #[cfg(feature = "login")]
        pub masked: bool,
    }

    impl HomeState {
        pub fn view(&self) -> neo_atrium::View {
            neo_atrium::View::Home {
                focused: self.focused,
                held: self.held,
                chip_selected: self.chip_selected,
            }
        }
    }

    /// The WM's own focus object (read side) and its activator.
    pub struct HomeFocus {
        pub(super) events: Handle,
        pub(super) activator: Handle,
    }

    pub fn claim_focus() -> Result<HomeFocus, i32> {
        let events = sys_input_focus_create().map_err(|_| 70)?;
        let activator = sys_input_focus_mint_activator(events).map_err(|_| 71)?;
        if sys_input_focus_activate(activator) != 0 {
            return Err(72);
        }
        Ok(HomeFocus { events, activator })
    }

    use super::wm_rect as rect;

    impl Scene {
        pub fn view_damage(&self) -> Rect {
            self.chrome
                .layout
                .view_regions()
                .iter()
                .fold(Rect::default(), |acc, r| acc.union(&rect(*r)))
        }

        /// DM-1: the view switch's regions, each a transition's damage.
        pub fn view_rects(&self) -> [Rect; 7] {
            self.chrome.layout.view_regions().map(rect)
        }

        /// DM-1: the focus light's regions (the four strips around the
        /// client, the Spine, the bar label).
        pub fn focus_rects(&self) -> [Rect; 6] {
            self.chrome.layout.focus_regions().map(rect)
        }

        /// The view switch just repainted, presented as a `kind` transition
        /// over its regions (anchored on the window), ending on the one
        /// present of their union.
        pub fn view_transition(&mut self, kind: Kind, client: bool) -> Result<(), i32> {
            let (damage, rects, glow) = (self.view_damage(), self.view_rects(), self.geo.glow);
            self.transition(kind, glow, &rects, client, &[damage])
        }

        /// The focus-light change just repainted, as a `Focus` transition
        /// over its strips, ending on the one present of `focus_damage`.
        pub fn focus_transition(&mut self, client: bool) -> Result<(), i32> {
            let (damage, rects, glow) = (self.geo.focus_damage, self.focus_rects(), self.geo.glow);
            self.transition(Kind::Focus, glow, &rects, client, &[damage])
        }

        /// Switch the base to the Home view, the WM holding the keyboard;
        /// `held`: Arthur yielded and stays alive behind Home.
        pub fn show_home(&mut self, focused: bool, held: bool) {
            // DS-1c: Home starts with the selection on Arthur's chip (a
            // pointer selection made in the window view never carries over).
            #[cfg(feature = "desktop-lens")]
            self.chrome.set_lens_selected(false);
            #[cfg(feature = "desktop-pointer")]
            self.hide_sprite();
            self.set_focused(false);
            self.home = Some(HomeState {
                focused,
                held,
                chip_selected: false,
                line: Line::default(),
                #[cfg(feature = "login")]
                masked: false,
            });
            // OT-85: Home comes back showing the current minute (the
            // repaint below covers the readout's box).
            #[cfg(feature = "desktop-clock")]
            super::clock::refresh(self);
            for r in self.chrome.layout.view_regions() {
                self.repaint_chrome(rect(r));
            }
        }

        /// Switch the base to the window view (unfocused until the client's
        /// own focus is activated).
        pub fn show_window(&mut self) {
            self.home = None;
            // Painted whole, not through `repaint_chrome`: that clips window
            // repaints to the focus regions, and the field's glow reaches
            // past them (its slivers would stay in the base).
            let stride = self.geo.screen.w;
            for r in self.chrome.layout.view_regions() {
                self.chrome
                    .paint_rect(self.base.slice_mut(), stride, self.focused, r);
            }
        }
    }

    /// Edit the intent field until Enter on a non-empty line, or Enter on
    /// the Spine chip (Tab moves the keyboard between field and chip, Esc
    /// returns it to the field). Returns the intent; empty means "the chip":
    /// resume the held Arthur, or launch one with no intent.
    pub fn read_intent(scene: &mut Scene, focus: &HomeFocus) -> Result<Line<MAX_INTENT>, i32> {
        let field = rect(scene.chrome.layout.field);
        let field_glow = rect(scene.chrome.layout.field_glow);
        let spine = rect(scene.chrome.layout.spine);
        loop {
            // OT-66 L2: locked, the WM's focus is the lock screen's.
            #[cfg(feature = "login")]
            if super::lock::service(scene)? {
                #[cfg(feature = "lock-spawn-test")]
                if super::lock::spawn_under_lock() {
                    return Ok(Line::default());
                }
                continue;
            }
            let mut event = [0u8; INPUT_EVENT_V1_LEN];
            match sys_input_focus_recv(focus.events, &mut event) {
                0 => {}
                EAGAIN => {
                    #[cfg(all(feature = "desktop-pointer", not(feature = "desktop-intro")))]
                    scene.pointer_idle();
                    #[cfg(feature = "desktop-intro")]
                    super::intro::pointer_tick(scene)?;
                    // OT-87 R4.4b: a click on the Install chip opens the
                    // installer.
                    #[cfg(all(feature = "installer", feature = "desktop-intro"))]
                    if core::mem::take(&mut scene.intro.install_press) {
                        super::installer_view::run(scene, focus.events)?;
                    }
                    // OT-85: a new minute repaints the readout's box (and the
                    // intro card's time row) and nothing else.
                    #[cfg(feature = "desktop-clock")]
                    super::clock::tick(scene)?;
                    // OT-22: park on the WM's focus, the route and SAK; with
                    // the clock, until the next minute boundary at the latest.
                    #[cfg(feature = "desktop-park")]
                    scene.park(
                        &[libneo::WaitItem {
                            handle: focus.events.0,
                            kind: libneo::WAIT_KIND_INPUT,
                        }],
                        #[cfg(feature = "desktop-clock")]
                        scene.clock.deadline(),
                        #[cfg(not(feature = "desktop-clock"))]
                        u64::MAX,
                    );
                    #[cfg(not(feature = "desktop-park"))]
                    core::hint::spin_loop();
                    continue;
                }
                _ => return Err(73),
            }
            let Some(key) = keys::decode(event) else {
                continue;
            };
            // `desktop-intro`: any key ends a card, then acts; Esc only ends it.
            #[cfg(feature = "desktop-intro")]
            if scene.intro.card != super::intro::Card::None {
                let esc = key == Key::Escape;
                super::intro::close(scene, if esc { b"esc" } else { b"key" })?;
                if esc {
                    continue;
                }
            }
            let Some(home) = scene.home.as_mut() else {
                return Err(74);
            };
            if home.chip_selected {
                match key {
                    // An empty intent means "the chip": the caller resumes
                    // Arthur, or (DS-1c) switches to the Lens when the
                    // selection is on the Lens's chip.
                    Key::Enter => {
                        // OT-87 R4.4b: the Install chip opens the installer.
                        #[cfg(all(feature = "installer", feature = "desktop-intro"))]
                        if scene.chrome.install_selected() {
                            log(b"DESKTOP home chip activated target=install\n");
                            super::installer_view::run(scene, focus.events)?;
                            continue;
                        }
                        #[cfg(all(
                            feature = "desktop-session-boot-test",
                            feature = "desktop-lens"
                        ))]
                        if scene.chrome.lens_selected() {
                            log(b"DESKTOP home chip activated target=lens\n");
                            return Ok(Line::default());
                        }
                        #[cfg(feature = "desktop-session-boot-test")]
                        log(b"DESKTOP home chip activated target=arthur\n");
                        return Ok(Line::default());
                    }
                    // DS-1c: Tab moves on from Arthur's chip to the Lens's
                    // (while one is shown), then back to the field.
                    #[cfg(all(
                        feature = "desktop-lens",
                        not(all(feature = "installer", feature = "desktop-intro"))
                    ))]
                    Key::Tab if scene.chrome.lens_shown() && !scene.chrome.lens_selected() => {
                        scene.select_lens(true);
                        scene.transition(Kind::Focus, spine, &[spine], false, &[spine])?;
                        #[cfg(feature = "desktop-session-boot-test")]
                        log(b"DESKTOP home chip selected=2\n");
                    }
                    // OT-87 R4.4b: not from the Install chip.
                    #[cfg(all(
                        feature = "desktop-lens",
                        feature = "installer",
                        feature = "desktop-intro"
                    ))]
                    Key::Tab
                        if scene.chrome.lens_shown()
                            && !scene.chrome.lens_selected()
                            && !scene.chrome.install_selected() =>
                    {
                        scene.select_lens(true);
                        scene.transition(Kind::Focus, spine, &[spine], false, &[spine])?;
                        #[cfg(feature = "desktop-session-boot-test")]
                        log(b"DESKTOP home chip selected=2\n");
                    }
                    // OT-87 R4.4b: then to the Install chip, while it shows.
                    #[cfg(all(feature = "installer", feature = "desktop-intro"))]
                    Key::Tab
                        if scene.chrome.install_shown() && !scene.chrome.install_selected() =>
                    {
                        #[cfg(feature = "desktop-lens")]
                        scene.select_lens(false);
                        scene.select_install(true);
                        scene.transition(Kind::Focus, spine, &[spine], false, &[spine])?;
                        #[cfg(feature = "desktop-session-boot-test")]
                        log(b"DESKTOP home chip selected=3\n");
                    }
                    Key::Tab | Key::Escape => {
                        home.chip_selected = false;
                        #[cfg(feature = "desktop-lens")]
                        scene.select_lens(false);
                        #[cfg(all(feature = "installer", feature = "desktop-intro"))]
                        scene.select_install(false);
                        move_selection(scene, spine, field_glow)?;
                        #[cfg(feature = "desktop-session-boot-test")]
                        log(b"DESKTOP home chip selected=0\n");
                    }
                    _ => {}
                }
                continue;
            }
            if key == Key::Tab {
                home.chip_selected = true;
                move_selection(scene, spine, field_glow)?;
                #[cfg(feature = "desktop-session-boot-test")]
                log(b"DESKTOP home chip selected=1\n");
                continue;
            }
            // OT-87 R4.4: `install` is the WM's too: the installer card.
            #[cfg(all(feature = "installer", feature = "desktop-intro"))]
            if key == Key::Enter && home.line.as_bytes().trim_ascii() == b"install" {
                home.line.clear();
                scene.repaint_chrome(field);
                scene.present(false, field).map_err(|c| 100 + c)?;
                super::installer_view::run(scene, focus.events)?;
                continue;
            }
            // THINK-2026-11-02-time-zones §2.5: `timezone` and `rtc` are the
            // WM's (it owns the settings store), never Arthur's.
            #[cfg(all(feature = "desktop-clock", feature = "desktop-intro"))]
            if key == Key::Enter && libneo::tz::parse_verb(home.line.as_bytes()).is_some() {
                let line = home.line.clone();
                home.line.clear();
                scene.repaint_chrome(field);
                scene.present(false, field).map_err(|c| 100 + c)?;
                super::zone_verb::run(scene, line.as_bytes())?;
                continue;
            }
            // `desktop-intro`: `help` is the WM's, never Arthur's.
            #[cfg(feature = "desktop-intro")]
            if key == Key::Enter && neo_atrium::intro::is_help(home.line.as_bytes()) {
                home.line.clear();
                scene.repaint_chrome(field);
                scene.present(false, field).map_err(|c| 100 + c)?;
                super::intro::show_help(scene, b"typed")?;
                continue;
            }
            if key == Key::Enter && !home.line.is_empty() {
                let line = home.line.clone();
                #[cfg(feature = "desktop-session-boot-test")]
                log(b"DESKTOP home intent submitted target=arthur\n");
                return Ok(line);
            }
            if home.line.edit(key) {
                #[cfg(feature = "desktop-session-boot-test")]
                let typed_len = home.line.as_bytes().len() as u64;
                scene.repaint_chrome(field);
                scene.present(false, field).map_err(|c| 100 + c)?;
                // Logged once the keystroke is on screen (boot-test only: in
                // production the WM never logs what the user types).
                #[cfg(feature = "desktop-session-boot-test")]
                match key {
                    Key::Char(c) => log_key(c),
                    // F10: a held Backspace's repeats, one per byte removed.
                    Key::Backspace => super::Line::new()
                        .s(b"DESKTOP home keystroke backspace len=")
                        .n(typed_len)
                        .s(b"\n")
                        .emit(),
                    _ => {}
                }
            }
        }
    }

    /// Hand Arthur an intent: `b"I"` + up to [`MAX_INTENT`] printable bytes
    /// (possibly none) on the session channel.
    pub fn send_intent(session: Handle, intent: &[u8]) -> bool {
        let mut message = [0u8; 1 + MAX_INTENT];
        message[0] = b'I';
        let len = intent.len().min(MAX_INTENT);
        message[1..1 + len].copy_from_slice(&intent[..len]);
        sys_send(session, &message[..1 + len]) == (1 + len) as i64
    }

    /// Back from Home to the held Arthur (Chad DS-1a F1: `HANDOFF`): its own
    /// ACTIVATE-only focus first; then the intent, on receipt of which Arthur
    /// discards whatever its queue kept from before the hold; then the window
    /// view, its focus light and its last frame.
    pub fn resume(
        scene: &mut Scene,
        activator: Handle,
        session: Handle,
        intent: &[u8],
    ) -> Result<(), i32> {
        hand_over(
            &mut Steps {
                swap: || swap_to(activator, 77),
                discard: || {
                    // Logged before the send: the held Arthur cannot act (or
                    // log) until the intent arrives.
                    log_resume(intent.len());
                    if !send_intent(session, intent) {
                        return Err(33);
                    }
                    // Arthur has discarded its queue: only then the view.
                    if !super::recv_control(session, b'D') {
                        return Err(36);
                    }
                    Ok(())
                },
                present: || {
                    scene.show_window();
                    scene.set_focused(true);
                    #[cfg(feature = "desktop-session-boot-test")]
                    log_residue(scene, neo_atrium::View::Window { focused: true }, b"resume");
                    scene.view_transition(Kind::View, true)
                },
            },
            &HANDOFF,
        )
    }

    /// Chad DS-1a F1: make `activator` the keyboard route (the swap step).
    pub fn swap_to(activator: Handle, code: i32) -> Result<(), i32> {
        // OT-66 L2: while locked the WM's own focus keeps the route; the
        // owner asked for is the one unlock restores.
        #[cfg(feature = "login")]
        if super::lock::defer_route(activator) {
            return Ok(());
        }
        if sys_input_focus_activate(activator) != 0 {
            return Err(code);
        }
        #[cfg(feature = "login")]
        super::lock::note_route(activator);
        Ok(())
    }

    /// Chad DS-1a F1, the discard step for the WM's own focus (it must be the
    /// route): drop whatever Home's queue held — keys left from an earlier
    /// Home turn, or typed in the instant of the swap. Bounded: a focus
    /// queue holds at most 64 events.
    pub fn discard_queued(focus: &HomeFocus, via: &[u8]) {
        let mut stale = [0u8; INPUT_EVENT_V1_LEN];
        let mut discarded = 0usize;
        while discarded < 256 && sys_input_focus_recv(focus.events, &mut stale) == 0 {
            discarded += 1;
        }
        #[cfg(feature = "desktop-session-boot-test")]
        super::Line::new()
            .s(b"DESKTOP handoff to=home via=")
            .s(via)
            .s(b" discarded=")
            .n(discarded as u64)
            .s(b"\n")
            .emit();
        #[cfg(not(feature = "desktop-session-boot-test"))]
        let _ = via;
    }

    pub fn log_resume(intent_bytes: usize) {
        let mut line = *b"DESKTOP home resume client=arthur intent_bytes=000\n";
        let at = line.len() - 4;
        line[at] = b'0' + (intent_bytes / 100 % 10) as u8;
        line[at + 1] = b'0' + (intent_bytes / 10 % 10) as u8;
        line[at + 2] = b'0' + (intent_bytes % 10) as u8;
        log(&line);
    }

    /// The keyboard moved between the field and the chip: both relight.
    fn move_selection(scene: &mut Scene, spine: Rect, field_glow: Rect) -> Result<(), i32> {
        let both = [spine, field_glow];
        for r in both {
            scene.repaint_chrome(r);
        }
        scene.transition(Kind::Focus, spine, &both, false, &both)
    }

    /// Arthur yielded (`OP_YIELD_FOCUS`): Home takes the view and the
    /// keyboard while Arthur stays alive, held, blocked on its session
    /// channel until [`resume`].
    /// (Chad DS-1a F1: the keyboard moves to Home before Home is shown, and
    /// Home's queue is emptied of anything older.)
    pub fn hold(scene: &mut Scene, focus: &HomeFocus) -> Result<(), i32> {
        hand_over(
            &mut Steps {
                swap: || swap_to(focus.activator, 76),
                discard: || {
                    discard_queued(focus, b"hold");
                    Ok(())
                },
                present: || {
                    scene.show_home(true, true);
                    // Boot test: the first hold stops at its mid frame for
                    // the scenario's capture (the WM has the keyboard).
                    #[cfg(feature = "desktop-motion-boot-test")]
                    if !MOTION_HELD.swap(true, core::sync::atomic::Ordering::Relaxed) {
                        scene.arm_motion_hold(focus.events);
                    }
                    scene.view_transition(Kind::View, false)?;
                    // A hold with no frame 3 never leaves the arm for a
                    // later transition.
                    #[cfg(feature = "desktop-motion-boot-test")]
                    {
                        scene.motion_hold = None;
                    }
                    #[cfg(feature = "desktop-session-boot-test")]
                    log_stale(scene, b"hold");
                    #[cfg(feature = "desktop-session-boot-test")]
                    if let Some(home) = &scene.home {
                        log_residue(scene, home.view(), b"hold");
                    }
                    Ok(())
                },
            },
            &HANDOFF,
        )?;
        log(b"DESKTOP home hold client=arthur focus=wm-home\n");
        Ok(())
    }

    /// Arthur has exited: Home takes the view and the keyboard back.
    /// (Chad DS-1a F1: keys typed after Enter but before Arthur's focus went
    /// live landed on Home's queue; they belonged to the launch and are
    /// discarded here, before Home is shown — not replayed into the field.)
    pub fn return_home(scene: &mut Scene, focus: &HomeFocus) -> Result<(), i32> {
        hand_over(
            &mut Steps {
                swap: || swap_to(focus.activator, 75),
                discard: || {
                    discard_queued(focus, b"return");
                    Ok(())
                },
                present: || {
                    scene.show_home(true, false);
                    scene.view_transition(Kind::Close, false)?;
                    #[cfg(feature = "desktop-session-boot-test")]
                    log_stale(scene, b"home");
                    #[cfg(feature = "desktop-session-boot-test")]
                    log_residue(scene, neo_atrium::View::home(true), b"home");
                    Ok(())
                },
            },
            &HANDOFF,
        )?;
        // Whole-session witness: Home typing, the launch, Arthur's frames and
        // both view switches ran no blur beyond those built at boot (five,
        // and the Lens frame's: six; DI-1b/DL-1 add each frame change's).
        super::Line::new()
            .s(b"DESKTOP home return client=arthur exit=0 focus=wm-home blur_regions_total=")
            .n(u64::from(neo_atrium::blur::blur_calls()))
            .s(b"\n")
            .emit();
        Ok(())
    }

    /// Boot-test witness for a view switch's repaint: every base pixel of the
    /// switch regions (label boxes aside) must equal the target view
    /// recomputed from the scene definition. Catches a region the switch
    /// forgot to repaint, which `log_stale` (presented vs base) cannot.
    #[cfg(feature = "desktop-session-boot-test")]
    pub fn log_residue(scene: &Scene, view: neo_atrium::View, name: &[u8]) {
        let l = &scene.chrome.layout;
        let three = match view {
            neo_atrium::View::Home { .. } => l.home_label_boxes(),
            neo_atrium::View::Window { .. } => l.label_boxes(),
        };
        // DS-1c: the second chip's and the Lens's labels are masked too.
        #[cfg(feature = "desktop-lens")]
        let labels = {
            let lens_title = l.lens.map(|lens| lens.title_label).unwrap_or_default();
            // `desktop-intro`: the hint's or the card's text, in Home.
            #[cfg(feature = "desktop-intro")]
            let intro_text = match view {
                neo_atrium::View::Home { .. } => super::intro::text_box(scene),
                neo_atrium::View::Window { .. } => neo_atrium::Rect::default(),
            };
            #[cfg(not(feature = "desktop-intro"))]
            let intro_text = neo_atrium::Rect::default();
            // OT-85 `desktop-clock`: the readout's box, in Home.
            #[cfg(feature = "desktop-clock")]
            let clock_text = match view {
                neo_atrium::View::Home { .. } => super::clock::text_box(scene),
                neo_atrium::View::Window { .. } => neo_atrium::Rect::default(),
            };
            #[cfg(not(feature = "desktop-clock"))]
            let clock_text = neo_atrium::Rect::default();
            [
                three[0],
                three[1],
                three[2],
                l.chip2_label,
                lens_title,
                intro_text,
                clock_text,
            ]
        };
        #[cfg(not(feature = "desktop-lens"))]
        let labels = three;
        // OT-87 R4.4b: the Install chip's label, while it shows.
        #[cfg(all(feature = "installer", feature = "desktop-intro"))]
        let chip3_label = if scene.chrome.install_shown() {
            l.chip3_label
        } else {
            neo_atrium::Rect::default()
        };
        let stride = l.screen.w;
        let base = scene.base.slice();
        let mut residue = 0usize;
        for r in l.view_regions() {
            for y in r.y..r.bottom() {
                for x in r.x..r.right() {
                    if labels.iter().any(|b| b.contains(x, y)) {
                        continue;
                    }
                    #[cfg(all(feature = "installer", feature = "desktop-intro"))]
                    if chip3_label.contains(x, y) {
                        continue;
                    }
                    let want = neo_atrium::wide::dither(scene.chrome.view_pixel(x, y, view), x, y);
                    residue += usize::from(base[y * stride + x] != want);
                }
            }
        }
        log_count(b"DESKTOP view switch=", name, b" residue_pixels=", residue);
    }

    #[cfg(feature = "desktop-session-boot-test")]
    fn log_count(a: &[u8], b: &[u8], c: &[u8], value: usize) {
        super::Line::new()
            .s(a)
            .s(b)
            .s(c)
            .n(value as u64)
            .s(b"\n")
            .emit();
    }

    /// Boot-test witness for a view switch: with no client attached and the
    /// focus light off, every presented pixel must equal the base (a stale
    /// pixel is one the switch repainted but never presented).
    #[cfg(feature = "desktop-session-boot-test")]
    pub fn log_stale(scene: &Scene, view: &[u8]) {
        log_stale_outside(scene, view, &[]);
    }

    /// As [`log_stale`], not counting the client areas in `clients` (where
    /// the presented frame shows a client, not the base).
    #[cfg(feature = "desktop-session-boot-test")]
    pub fn log_stale_outside(scene: &Scene, view: &[u8], clients: &[Rect]) {
        let stride = scene.geo.screen.w;
        let stale = scene
            .out
            .slice()
            .iter()
            .zip(scene.base.slice())
            .enumerate()
            .filter(|(i, (o, b))| {
                o != b && !clients.iter().any(|c| c.contains(i % stride, i / stride))
            })
            .count();
        super::Line::new()
            .s(b"DESKTOP view switch=")
            .s(view)
            .s(b" stale_pixels=")
            .n(stale as u64)
            .s(b"\n")
            .emit();
    }

    #[cfg(feature = "desktop-session-boot-test")]
    fn log_key(c: u8) {
        super::Line::new()
            .s(b"DESKTOP home keystroke ascii=")
            .n(u64::from(c))
            .s(b"\n")
            .emit();
    }
}

#[cfg(all(feature = "installer", feature = "desktop-intro"))]
#[path = "installer_view.rs"]
mod installer_view;

/// Arthur's first-boot introduction and the help card (`desktop-intro`,
/// NOTE-2026-10-20 as the PL approved it). The words and the show rule are
/// `neo_atrium::intro`'s (host-KAT'd); this is when they show and go:
///
/// * Session start: Home gets its card slot. The intro shows on a machine
///   that keeps nothing (no NeoFS memory directory: every session, with the
///   USB-only line) and, where NeoFS persists, until [`FLAG`] says it was
///   seen. It never takes the keyboard: the field has it as always.
/// * Any key ends a card and then does what it does; Esc ends it and is
///   consumed. A click elsewhere on Home ends it too.
/// * `help` + Enter in the field, or a click on the hint, shows the help
///   card. The WM answers: the line never reaches Arthur or the model.
/// * Model state: the WM cannot know it before Arthur loads the model (the
///   kernel checks the module at `sys_model_load`), so the card shows the
///   model-ready intro; Arthur's first reply carries B1b's state sentence,
///   the same words the no-model variants use.
#[cfg(feature = "desktop-intro")]
mod intro {
    use super::{Line, MEMORY_DIR, Rect, Scene, wm_rect};
    use libneo::{
        FileRights, OPEN_CREATE_REGULAR, OPEN_ROOT_TEMPORARY, sys_close, sys_fsync, sys_open,
    };
    use neo_atrium::intro::{self as words, ModelState};

    /// Present once the intro has been seen where NeoFS persists (beside
    /// the Reduce-Transparency preference).
    pub const FLAG: &str = "/neofs/atrium-introduced";

    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    pub enum Card {
        #[default]
        None,
        Intro,
        Help,
        /// The `timezone` / `rtc` verbs' card.
        Zone,
    }

    #[derive(Default)]
    pub struct State {
        pub card: Card,
        /// NeoFS persists here: ending the intro writes [`FLAG`].
        persistent: bool,
        /// The intro shows this session (for the start line).
        shown: bool,
        /// OT-87 R4.4b: a press on the Install chip, for Home's loop.
        #[cfg(feature = "installer")]
        pub install_press: bool,
        /// L7.1 (`boot-time`): the boot summary, read once as Home first
        /// shows, so the minute's re-wording keeps it.
        #[cfg(feature = "boot-time")]
        boot_row: Option<alloc::string::String>,
    }

    /// L7.1: the boot summary for Home, logged as `BOOT-TIME`.
    #[cfg(feature = "boot-time")]
    fn boot_row() -> Option<alloc::string::String> {
        let line = libneo::boot_summary_line("Home")?;
        Line::new()
            .s(b"BOOT-TIME ")
            .s(line.as_bytes())
            .s(b"\n")
            .emit();
        Some(line)
    }

    /// Before Home is first shown: the slot, and the intro when it is due.
    pub fn start(scene: &mut Scene) {
        #[cfg(feature = "boot-time")]
        {
            scene.intro.boot_row = boot_row();
        }
        scene.chrome.set_intro(true);
        let persistent = match sys_open(
            MEMORY_DIR,
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        ) {
            Ok(dir) => {
                let _ = sys_close(dir);
                true
            }
            Err(_) => false,
        };
        let introduced = persistent
            && match sys_open(FLAG, OPEN_ROOT_TEMPORARY, FileRights::READ) {
                Ok(flag) => {
                    let _ = sys_close(flag);
                    true
                }
                Err(_) => false,
            };
        let shown = words::show_intro(persistent, introduced);
        if shown {
            let rows = intro_rows(scene, !persistent);
            #[cfg(feature = "desktop-clock-boot-test")]
            log_clock_row(&rows);
            scene.chrome.set_card(rows);
        }
        scene.intro = State {
            card: if shown { Card::Intro } else { Card::None },
            persistent,
            shown,
            #[cfg(feature = "installer")]
            install_press: false,
            #[cfg(feature = "boot-time")]
            boot_row: scene.intro.boot_row.take(),
        };
    }

    /// The model-ready intro's rows: OT-85, with the wall clock the greeting
    /// follows the hour and carries the date and time (`desktop-clock`, when
    /// the kernel has an epoch); else NOTE-2026-10-20's "Good day".
    fn intro_rows(scene: &Scene, usb_only: bool) -> alloc::vec::Vec<alloc::string::String> {
        #[cfg(feature = "desktop-clock")]
        let rows = match scene.clock.now_words() {
            Some(now) => words::intro_lines_now(ModelState::Ready, usb_only, &now.as_now()),
            None => words::intro_lines(ModelState::Ready, usb_only),
        };
        #[cfg(not(feature = "desktop-clock"))]
        let rows = words::intro_lines(ModelState::Ready, usb_only);
        #[cfg(feature = "boot-time")]
        let rows = with_boot_row(rows, scene.intro.boot_row.as_deref());
        let _ = scene;
        rows
    }

    /// L7.1 (the `--boot-time` evidence image): the boot summary as the
    /// card's second row. The card holds `CARD_MAX_ROWS`, so the rows before
    /// the last (the way out) give way to it.
    #[cfg(feature = "boot-time")]
    fn with_boot_row(
        mut rows: alloc::vec::Vec<alloc::string::String>,
        boot: Option<&str>,
    ) -> alloc::vec::Vec<alloc::string::String> {
        let Some(boot) = boot else {
            return rows;
        };
        rows.insert(rows.len().min(1), alloc::string::String::from(boot));
        while rows.len() > neo_atrium::layout::CARD_MAX_ROWS && rows.len() >= 2 {
            rows.remove(rows.len() - 2);
        }
        rows
    }

    /// OT-85: a new minute re-words the intro while it shows (the greeting
    /// row carries the time); the slot is repainted and presented.
    #[cfg(feature = "desktop-clock")]
    pub fn refresh_clock(scene: &mut Scene) -> Result<(), i32> {
        if scene.intro.card != Card::Intro {
            return Ok(());
        }
        let rows = intro_rows(scene, !scene.intro.persistent);
        #[cfg(feature = "desktop-clock-boot-test")]
        log_clock_row(&rows);
        scene.chrome.set_card(rows);
        present_slot(scene)
    }

    /// Boot test (OT-85): the card's first row, for the host's check of the
    /// greeting word against the hour.
    #[cfg(feature = "desktop-clock-boot-test")]
    fn log_clock_row(rows: &[alloc::string::String]) {
        if let Some(row) = rows.first() {
            Line::new()
                .s(b"DESKTOP intro clock row=\"")
                .s(row.as_bytes())
                .s(b"\"\n")
                .emit();
        }
    }

    /// Once Home (and the card) is on screen.
    pub fn log_start(scene: &Scene) {
        let s = &scene.intro;
        let line = Line::new().s(b"DESKTOP intro ");
        let line = if s.shown {
            line.s(b"shown variant=ready usb=")
                .n(u64::from(!s.persistent))
                .s(b" rows=")
                .n(scene.chrome.card_rows() as u64)
        } else {
            line.s(b"skipped reason=introduced")
        };
        line.s(b" persistent=")
            .n(u64::from(s.persistent))
            .s(b"\n")
            .emit();
    }

    fn slot(scene: &Scene) -> Rect {
        wm_rect(scene.chrome.layout.card_slot())
    }

    /// Repaint and present the slot (Home only; the client is not shown).
    fn present_slot(scene: &mut Scene) -> Result<(), i32> {
        let r = slot(scene);
        scene.repaint_chrome(r);
        scene.present(false, r).map(|_| ()).map_err(|c| 100 + c)
    }

    /// End the card shown (`via`: key, esc, click). Ending the intro where
    /// NeoFS persists records it was seen.
    pub fn close(scene: &mut Scene, via: &[u8]) -> Result<(), i32> {
        let was = scene.intro.card;
        if was == Card::None {
            return Ok(());
        }
        scene.intro.card = Card::None;
        scene.chrome.set_card(alloc::vec::Vec::new());
        present_slot(scene)?;
        let name: &[u8] = match was {
            Card::Intro => b"intro",
            Card::Zone => b"zone",
            _ => b"help",
        };
        Line::new()
            .s(b"DESKTOP ")
            .s(name)
            .s(b" closed via=")
            .s(via)
            .s(b"\n")
            .emit();
        if was == Card::Intro && scene.intro.persistent {
            record_seen();
        }
        Ok(())
    }

    /// Show the help card (`via`: typed, click).
    pub fn show_help(scene: &mut Scene, via: &[u8]) -> Result<(), i32> {
        if scene.intro.card == Card::Intro {
            close(scene, via)?;
        }
        scene.intro.card = Card::Help;
        scene.chrome.set_card(words::help_lines());
        present_slot(scene)?;
        Line::new()
            .s(b"DESKTOP help shown via=")
            .s(via)
            .s(b" rows=")
            .n(scene.chrome.card_rows() as u64)
            .s(b"\n")
            .emit();
        Ok(())
    }

    /// Show the clock verbs' card (`timezone`, `rtc`).
    #[cfg(feature = "desktop-clock")]
    pub fn show_zone(
        scene: &mut Scene,
        rows: alloc::vec::Vec<alloc::string::String>,
    ) -> Result<(), i32> {
        if scene.intro.card == Card::Intro {
            close(scene, b"typed")?;
        }
        scene.intro.card = Card::Zone;
        scene.chrome.set_card(rows);
        present_slot(scene)?;
        Line::new()
            .s(b"DESKTOP zone card shown rows=")
            .n(scene.chrome.card_rows() as u64)
            .s(b"\n")
            .emit();
        Ok(())
    }

    fn record_seen() {
        let Ok(flag) = sys_open(
            FLAG,
            OPEN_CREATE_REGULAR,
            FileRights::READ.union(FileRights::WRITE),
        ) else {
            Line::new().s(b"DESKTOP intro flag write=failed\n").emit();
            return;
        };
        let synced = sys_fsync(flag) == 0;
        let _ = sys_close(flag);
        Line::new()
            .s(b"DESKTOP intro flag write=")
            .s(if synced { b"ok" } else { b"unsynced" })
            .s(b"\n")
            .emit();
    }

    /// Home's pointer (`desktop-intro` gives Home a sprite, shown once the
    /// mouse moves): a press on the hint shows the help card; one anywhere
    /// else on Home but the card ends the card shown.
    /// OT-87 R4.4b: a press on the Install chip is left in
    /// [`State::install_press`] for the caller, which opens the installer.
    pub fn pointer_tick(scene: &mut Scene) -> Result<(), i32> {
        use super::pointer::{self, Action, Ctx, Shape};
        let Some(p) = scene.pointer.as_mut() else {
            return Ok(());
        };
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
        let Some(Action::HomePress(x, y)) = press else {
            return Ok(());
        };
        let l = &scene.chrome.layout;
        let card = l.card(scene.chrome.card_rows());
        // OT-87 R4.4b: the Install chip (a card shown ends first: the
        // installer draws its own in the same slot).
        #[cfg(feature = "installer")]
        if scene.chrome.install_shown() && l.chip3.contains(x, y) {
            log_install_press(x, y);
            if scene.intro.card != Card::None {
                close(scene, b"click")?;
            }
            scene.intro.install_press = true;
            return Ok(());
        }
        if scene.intro.card == Card::None && l.hint().contains(x, y) {
            return show_help(scene, b"click");
        }
        if scene.intro.card != Card::None && !card.contains(x, y) {
            return close(scene, b"click");
        }
        Ok(())
    }

    #[cfg(feature = "installer")]
    fn log_install_press(x: usize, y: usize) {
        #[cfg(feature = "desktop-session-boot-test")]
        Line::new()
            .s(b"DESKTOP home chip activated target=install via=click at=")
            .n(x as u64)
            .s(b",")
            .n(y as u64)
            .s(b"\n")
            .emit();
        #[cfg(not(feature = "desktop-session-boot-test"))]
        let _ = (x, y);
    }

    /// Boot-test residue witness: the intro's text box is masked like the
    /// label boxes.
    pub fn text_box(scene: &Scene) -> neo_atrium::Rect {
        scene.chrome.intro_text_box()
    }
}

/// DI-1a B-ref (THINK §8): the WM's pointer. The kernel mints a record per
/// mouse report into this route; the WM integrates the cursor, draws the
/// sprite, decides what each press means (its policy), and forwards or drops
/// every record exactly once. The WM stays single-threaded: its loops poll
/// the route, the SAK latch and (`SYS_TRY_RECV`) the surface channel.
#[cfg(feature = "desktop-pointer")]
mod pointer {
    use super::{Rect, Scene, home, log};
    use libneo::atrium_handoff::{HANDOFF, Steps, hand_over};
    use libneo::{
        EAGAIN, Handle, INPUT_EVENT_POINTER_REC, INPUT_EVENT_V1_LEN, POINTER_RESYNC,
        POINTER_ROUTE_EXPIRED, SAK_ESCAPE, SAK_SPACE, SAK_TAB, sys_input_focus_recv,
        sys_input_pointer_claim, sys_input_pointer_forward, sys_input_sak_take, sys_revoke,
    };

    // The WM's record decision and the kernel's record format agree.
    const _: () = assert!(neo_atrium::pointer_grab::RESYNC == POINTER_RESYNC);

    /// The arrow: `#` fill (text-primary), `.` outline (ground), ` ` clear.
    /// Hotspot: its top-left pixel.
    const ARROW: [&[u8; 12]; 18] = [
        b".           ",
        b"..          ",
        b".#.         ",
        b".##.        ",
        b".###.       ",
        b".####.      ",
        b".#####.     ",
        b".######.    ",
        b".#######.   ",
        b".########.  ",
        b".#########. ",
        b".######.....",
        b".###.##.    ",
        b".##. .##.   ",
        b".#.  .##.   ",
        b"..    .##.  ",
        b".     .##.  ",
        b"       ..   ",
    ];
    /// DI-1b: the four-way move cursor, over the title band and while a
    /// window is dragged. Hotspot: its centre.
    const MOVE: [&[u8; 19]; 19] = [
        b"        ...        ",
        b"       ..#..       ",
        b"      ..###..      ",
        b"     ..#####..     ",
        b"     .#######.     ",
        b"   ......#......   ",
        b"  ..#.  .#.  .#..  ",
        b" ..##.  .#.  .##.. ",
        b"..###....#....###..",
        b".#################.",
        b"..###....#....###..",
        b" ..##.  .#.  .##.. ",
        b"  ..#.  .#.  .#..  ",
        b"   ......#......   ",
        b"     .#######.     ",
        b"     ..#####..     ",
        b"      ..###..      ",
        b"       ..#..       ",
        b"        ...        ",
    ];
    /// DI-1b resize: the horizontal double arrow (the vertical one is its
    /// transpose). Hotspot: its centre. `#` fill, `.` the fill's
    /// 8-neighbour ring.
    #[cfg(feature = "desktop-resize")]
    const RESIZE_H: [&[u8; 21]; 11] = [
        b"                     ",
        b"   ...         ...   ",
        b"  ..#.         .#..  ",
        b" ..##.         .##.. ",
        b"..###...........###..",
        b".###################.",
        b"..###...........###..",
        b" ..##.         .##.. ",
        b"  ..#.         .#..  ",
        b"   ...         ...   ",
        b"                     ",
    ];
    /// DI-1b resize: the NW-SE double arrow (NE-SW is its mirror). Point
    /// symmetric about its centre, the hotspot.
    #[cfg(feature = "desktop-resize")]
    const RESIZE_D: [&[u8; 17]; 17] = [
        b".......          ",
        b".#####.          ",
        b".####..          ",
        b".###...          ",
        b".##.##..         ",
        b".#..###..        ",
        b".....###..       ",
        b"    ..###..      ",
        b"     ..###..     ",
        b"      ..###..    ",
        b"       ..###.....",
        b"        ..###..#.",
        b"         ..##.##.",
        b"          ...###.",
        b"          ..####.",
        b"          .#####.",
        b"          .......",
    ];
    const FILL: u32 = 0xFFF4_F4F6;
    const OUTLINE: u32 = 0xFF0E_0F14;
    /// The drag outline (DI-1b): Electric Blue, two pixels.
    const DRAG_OUTLINE: u32 = 0xFF00_0000 | neo_atrium::material::ELECTRIC_BLUE;
    const DRAG_OUTLINE_W: usize = 2;

    /// Which sprite the WM draws.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Shape {
        Arrow,
        Move,
        /// DI-1b resize: over (or dragging) a side edge, the top or bottom
        /// edge, and the two diagonals' corners.
        #[cfg(feature = "desktop-resize")]
        ResizeH,
        #[cfg(feature = "desktop-resize")]
        ResizeV,
        #[cfg(feature = "desktop-resize")]
        ResizeNwse,
        #[cfg(feature = "desktop-resize")]
        ResizeNesw,
    }

    impl Shape {
        /// Size and hotspot.
        fn geometry(self) -> (usize, usize, i32, i32) {
            match self {
                Shape::Arrow => (12, 18, 0, 0),
                Shape::Move => (19, 19, 9, 9),
                #[cfg(feature = "desktop-resize")]
                Shape::ResizeH => (21, 11, 10, 5),
                #[cfg(feature = "desktop-resize")]
                Shape::ResizeV => (11, 21, 5, 10),
                #[cfg(feature = "desktop-resize")]
                Shape::ResizeNwse | Shape::ResizeNesw => (17, 17, 8, 8),
            }
        }

        fn cell(self, col: usize, row: usize) -> u8 {
            match self {
                Shape::Arrow => ARROW[row][col],
                Shape::Move => MOVE[row][col],
                #[cfg(feature = "desktop-resize")]
                Shape::ResizeH => RESIZE_H[row][col],
                #[cfg(feature = "desktop-resize")]
                Shape::ResizeV => RESIZE_H[col][row],
                #[cfg(feature = "desktop-resize")]
                Shape::ResizeNwse => RESIZE_D[row][col],
                #[cfg(feature = "desktop-resize")]
                Shape::ResizeNesw => RESIZE_D[row][16 - col],
            }
        }

        /// DI-1b resize: the cursor for a drag of `edges`.
        #[cfg(feature = "desktop-resize")]
        fn for_edges(edges: u8) -> Self {
            use neo_atrium::layout::edge::{BOTTOM, LEFT, RIGHT, TOP};
            let horizontal = edges & (LEFT | RIGHT) != 0;
            let vertical = edges & (TOP | BOTTOM) != 0;
            match (horizontal, vertical) {
                (true, false) => Shape::ResizeH,
                (false, true) => Shape::ResizeV,
                _ if edges & (LEFT | TOP) == LEFT | TOP
                    || edges & (RIGHT | BOTTOM) == RIGHT | BOTTOM =>
                {
                    Shape::ResizeNwse
                }
                _ => Shape::ResizeNesw,
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Grab {
        None,
        /// Pinned at press time (Jarvis F6, THINK §8.3): the client's origin
        /// and the target then. A focus change mid-press (Super+Escape) does
        /// not retarget or drop the rest of the press: the release still
        /// reaches the client that saw the press.
        Client {
            ox: i32,
            oy: i32,
            /// The client's size at the press (the clamp for its coordinates).
            cw: i32,
            ch: i32,
            target: Handle,
        },
        /// The press was the WM's (focus change, chrome): drop until up.
        Consumed,
        /// DI-1b: a press on the title band, at `(px, py)`: the window
        /// follows (as an outline) until the release moves it.
        Move {
            px: i32,
            py: i32,
        },
        /// DI-1b resize: a press on the frame's `edges`, at `(px, py)`: the
        /// outline follows until the release asks the client for the size.
        #[cfg(feature = "desktop-resize")]
        Resize {
            edges: u8,
            px: i32,
            py: i32,
        },
    }

    pub struct Pointer {
        route: Handle,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        buttons: u8,
        grab: Grab,
    }

    pub enum Action {
        Escape(&'static [u8]),
        FocusWindow(&'static [u8]),
        /// DS-1c: a click on the Lens's Spine chip switches to the Lens.
        #[cfg(feature = "desktop-lens")]
        SelectLens(&'static [u8]),
        /// DI-1b: a title-band drag released `(dx, dy)` from its press.
        Move(i32, i32),
        /// DI-1b resize: a drag of the frame's `edges` released `(dx, dy)`
        /// from its press.
        #[cfg(feature = "desktop-resize")]
        Resize(u8, i32, i32),
        /// `desktop-intro`: a press on Home at `(x, y)` (Home's ctx only).
        #[cfg(feature = "desktop-intro")]
        HomePress(usize, usize),
    }

    /// DI-1b: what a held drag is doing, and how far it has gone.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Drag {
        Move(i32, i32),
        #[cfg(feature = "desktop-resize")]
        Resize(u8, i32, i32),
    }

    /// What the current view offers a press (`default()`: nothing — Home).
    #[derive(Default)]
    pub struct Ctx {
        pub client: Rect,
        pub frame: Rect,
        pub spine: Rect,
        /// DI-1b: the title band (a press there drags the window).
        pub title: Rect,
        pub arthur_focused: bool,
        pub activator: Option<Handle>,
        /// DI-1b resize: the window's layout (its resize handles), `None`
        /// while a configure is outstanding (one resize at a time) or when no
        /// window is shown. Moves wait for it too: a move would invalidate
        /// the outstanding size's origin.
        #[cfg(feature = "desktop-resize")]
        pub resize: Option<neo_atrium::Layout>,
        /// DS-1c: Arthur's chip, and the Lens's chip while a Lens is shown
        /// and not already selected (empty otherwise).
        #[cfg(feature = "desktop-lens")]
        pub chip: Rect,
        #[cfg(feature = "desktop-lens")]
        pub lens_chip: Rect,
        /// `desktop-intro`: Home's own press (the hint, the card): every
        /// fresh press is reported as [`Action::HomePress`].
        #[cfg(feature = "desktop-intro")]
        pub home: bool,
    }

    impl Pointer {
        /// OT-22: the route leaf, for a SYS_WAIT_ANY input item.
        #[cfg(feature = "desktop-park")]
        pub fn route(&self) -> Handle {
            self.route
        }

        /// Claim the pointer route; `None` (the ADR-074 route stays) when the
        /// kernel has no `atrium-pointer-forward` or this is not the session
        /// root.
        pub fn claim(screen: Rect) -> Option<Self> {
            match sys_input_pointer_claim(false) {
                Ok(route) => {
                    log(b"DESKTOP pointer route claimed authority=input-system policy=wm\n");
                    Some(Self {
                        route,
                        x: (screen.w / 2) as i32,
                        y: (screen.h / 2) as i32,
                        w: screen.w as i32,
                        h: screen.h as i32,
                        buttons: 0,
                        grab: Grab::None,
                    })
                }
                Err(_) => {
                    log(b"DESKTOP pointer route unavailable route=adr074\n");
                    None
                }
            }
        }

        pub fn position(&self) -> (i32, i32) {
            (self.x, self.y)
        }

        /// OT-66 L2: the screen locked. A press in progress is consumed
        /// until all buttons are up: the rest of it reaches no client.
        #[cfg(feature = "login")]
        pub fn consume_grab(&mut self) {
            if self.grab != Grab::None {
                self.grab = Grab::Consumed;
            }
        }

        /// Replace a spent route. A press in progress is consumed until all
        /// buttons are up (its records died with the old route).
        fn reclaim(&mut self, buttons: u8) {
            match sys_input_pointer_claim(true) {
                Ok(route) => {
                    let old = core::mem::replace(&mut self.route, route);
                    let _ = sys_revoke(old);
                    self.grab = if buttons == 0 {
                        Grab::None
                    } else {
                        Grab::Consumed
                    };
                    log(b"DESKTOP pointer route reclaimed reason=expired\n");
                }
                Err(_) => log(b"DESKTOP pointer route reclaim failed\n"),
            }
        }

        /// DI-1b: while a title-band (or, resize, an edge) drag is held,
        /// which, and how far the pointer is from the press.
        pub fn drag(&self) -> Option<Drag> {
            match self.grab {
                Grab::Move { px, py } => Some(Drag::Move(self.x - px, self.y - py)),
                #[cfg(feature = "desktop-resize")]
                Grab::Resize { edges, px, py } => {
                    Some(Drag::Resize(edges, self.x - px, self.y - py))
                }
                _ => None,
            }
        }

        /// DI-1b: the move cursor over the title band and while dragging it;
        /// (resize) the edge's cursor over a handle and while dragging one.
        pub fn shape(&self, ctx: &Ctx) -> Shape {
            let (x, y) = (self.x as usize, self.y as usize);
            #[cfg(feature = "desktop-resize")]
            {
                if let Some(Drag::Resize(edges, ..)) = self.drag() {
                    return Shape::for_edges(edges);
                }
                let edges = ctx.resize.map_or(0, |l| l.resize_edges(x, y));
                if edges != 0 && self.grab == Grab::None {
                    return Shape::for_edges(edges);
                }
            }
            let over_title = ctx.title.contains(x, y);
            if matches!(self.drag(), Some(Drag::Move(..)))
                || (over_title && self.grab == Grab::None)
            {
                Shape::Move
            } else {
                Shape::Arrow
            }
        }

        /// Drain the route: integrate motion and handle every record exactly
        /// once. Returns whether the cursor moved, and the press's action.
        pub fn poll(&mut self, ctx: &Ctx) -> (bool, Option<Action>) {
            let mut moved = false;
            let mut action = None;
            let mut event = [0u8; INPUT_EVENT_V1_LEN];
            while sys_input_focus_recv(self.route, &mut event) == 0 {
                if event[0] != INPUT_EVENT_POINTER_REC {
                    continue;
                }
                let flags = event[1];
                // The route is spent (Jarvis, #46 RETURN F1): take a fresh one
                // (new handle, new ring) and release the old. Its tag is never
                // valid, so there is nothing to forward or drop.
                if flags & POINTER_ROUTE_EXPIRED != 0 {
                    self.reclaim(flags & 0x1f);
                    continue;
                }
                // A9-C2: a wheel record goes to the window under the pointer
                // (hover, as Wayland's wl_pointer.axis), whatever the grab:
                // Arthur's client gets it at the pointer's local position;
                // anywhere else it is dropped. It moves nothing and changes
                // no button, so it never reaches the press decision below.
                #[cfg(feature = "desktop-wheel")]
                if flags & libneo::POINTER_WHEEL != 0 {
                    let tag = u16::from_le_bytes([event[2], event[3]]);
                    let (x, y) = (self.x as usize, self.y as usize);
                    let target = ctx.activator.filter(|_| ctx.client.contains(x, y));
                    let (lx, ly) = match target {
                        Some(_) => (self.x - ctx.client.x as i32, self.y - ctx.client.y as i32),
                        None => (0, 0),
                    };
                    let r = sys_input_pointer_forward(self.route, tag, target, lx, ly);
                    #[cfg(feature = "desktop-session-boot-test")]
                    log_wheel(
                        i16::from_le_bytes([event[4], event[5]]),
                        target.is_some() && r == 0,
                    );
                    let _ = r;
                    continue;
                }
                // Without the feature a wheel record is still never motion
                // (its `x` is the wheel): drop it, whatever kernel minted it.
                #[cfg(not(feature = "desktop-wheel"))]
                if flags & libneo::POINTER_WHEEL != 0 {
                    let tag = u16::from_le_bytes([event[2], event[3]]);
                    let _ = sys_input_pointer_forward(self.route, tag, None, 0, 0);
                    continue;
                }
                let tag = u16::from_le_bytes([event[2], event[3]]);
                let dx = i16::from_le_bytes([event[4], event[5]]) as i32;
                let dy = i16::from_le_bytes([event[6], event[7]]) as i32;
                if dx != 0 || dy != 0 {
                    self.x = (self.x + dx).clamp(0, self.w - 1);
                    self.y = (self.y + dy).clamp(0, self.h - 1);
                    moved = true;
                }
                let buttons = flags & 0x1f;
                // The press / RESYNC decision (`neo_atrium::pointer_grab`,
                // host-KAT'd): a RESYNC record is never a fresh press, and
                // with a button held it cancels the grab a lost transition
                // left behind (Jarvis, #46 RETURN F2).
                let decision =
                    neo_atrium::pointer_grab::decide(flags, self.buttons, self.grab != Grab::None);
                let pressed = decision.pressed;
                self.buttons = buttons;
                if decision.cancel_grab {
                    self.grab = Grab::Consumed;
                }
                if self.grab == Grab::None && pressed {
                    let (x, y) = (self.x as usize, self.y as usize);
                    // DS-1c boot-test discriminator: where each fresh press
                    // landed (a click that switched nothing is then
                    // self-explaining).
                    #[cfg(all(feature = "desktop-lens", feature = "desktop-session-boot-test"))]
                    log_press(x, y);
                    self.grab = Grab::Consumed;
                    // DI-1b resize: the handles first (the title band's top
                    // rows are the top edge), and nothing — not a move
                    // either — while a configure is outstanding.
                    #[cfg(feature = "desktop-resize")]
                    let (edges, window_free) = match ctx.resize {
                        Some(l) => (l.resize_edges(x, y), true),
                        None => (0, false),
                    };
                    #[cfg(not(feature = "desktop-resize"))]
                    let (edges, window_free) = (0u8, true);
                    if edges != 0 {
                        #[cfg(feature = "desktop-resize")]
                        {
                            self.grab = Grab::Resize {
                                edges,
                                px: self.x,
                                py: self.y,
                            };
                        }
                    } else if ctx.title.contains(x, y) {
                        if window_free {
                            self.grab = Grab::Move {
                                px: self.x,
                                py: self.y,
                            };
                        }
                    } else if let (true, true, true, Some(target)) = (
                        window_free,
                        ctx.client.contains(x, y),
                        ctx.arthur_focused,
                        ctx.activator,
                    ) {
                        // (Resize AAG: not while a configure is outstanding:
                        // the size and origin pinned here would go stale when
                        // the ack lands mid-press.)
                        self.grab = Grab::Client {
                            ox: ctx.client.x as i32,
                            oy: ctx.client.y as i32,
                            cw: ctx.client.w as i32,
                            ch: ctx.client.h as i32,
                            target,
                        };
                    } else if let Some(chip) = chip_action(ctx, x, y) {
                        action = Some(chip);
                    } else if ctx.frame.contains(x, y) && !ctx.arthur_focused {
                        action = Some(Action::FocusWindow(b"click"));
                    } else if ctx.spine.contains(x, y) && ctx.arthur_focused {
                        action = Some(Action::Escape(b"click"));
                    }
                    // Home's ctx holds no window, chips or Spine: nothing
                    // above matched, and the press is Home's.
                    #[cfg(feature = "desktop-intro")]
                    if ctx.home {
                        action = Some(Action::HomePress(x, y));
                    }
                }
                match self.grab {
                    Grab::Client {
                        ox,
                        oy,
                        cw,
                        ch,
                        target,
                    } => {
                        // Clamped to the client surface: a grab dragged off
                        // the window must not hand the client its screen
                        // position (`-client.x` at the screen edge) now that
                        // the user chooses where the window is (DI-1b AAG).
                        let lx = (self.x - ox).clamp(0, cw - 1);
                        let ly = (self.y - oy).clamp(0, ch - 1);
                        let r = sys_input_pointer_forward(self.route, tag, Some(target), lx, ly);
                        #[cfg(feature = "desktop-session-boot-test")]
                        if r == 0 && pressed {
                            log_forward(lx, ly);
                        }
                        let _ = r;
                    }
                    _ => {
                        let _ = sys_input_pointer_forward(self.route, tag, None, 0, 0);
                    }
                }
                // All buttons up (a plain or a RESYNC record) ends the grab;
                // a title-band drag ends by moving the window (or, held
                // still on an unfocused window, by focusing it).
                if decision.release {
                    let released = match self.drag() {
                        Some(Drag::Move(dx, dy)) if (dx, dy) != (0, 0) => {
                            Some(Action::Move(dx, dy))
                        }
                        #[cfg(feature = "desktop-resize")]
                        Some(Drag::Resize(edges, dx, dy)) if (dx, dy) != (0, 0) => {
                            Some(Action::Resize(edges, dx, dy))
                        }
                        Some(_) if !ctx.arthur_focused => Some(Action::FocusWindow(b"click")),
                        _ => None,
                    };
                    action = released.or(action);
                    self.grab = Grab::None;
                }
                // A move changes every rectangle `ctx` holds: leave the rest
                // of the queue for the next tick, which hit-tests it against
                // the moved window (DI-1b AAG). A resize request does too
                // (the next tick sees the outstanding configure).
                #[cfg(feature = "desktop-resize")]
                let changes_window = matches!(action, Some(Action::Move(..) | Action::Resize(..)));
                #[cfg(not(feature = "desktop-resize"))]
                let changes_window = matches!(action, Some(Action::Move(..)));
                if changes_window {
                    break;
                }
            }
            (moved, action)
        }
    }

    /// DS-1c: a press on a Spine chip switches: the Lens's chip to the Lens,
    /// Arthur's chip (while it does not hold the keyboard) back to Arthur.
    #[cfg(feature = "desktop-lens")]
    fn chip_action(ctx: &Ctx, x: usize, y: usize) -> Option<Action> {
        if ctx.lens_chip.contains(x, y) {
            return Some(Action::SelectLens(b"click"));
        }
        if ctx.chip.contains(x, y) && !ctx.arthur_focused {
            return Some(Action::FocusWindow(b"click"));
        }
        None
    }

    #[cfg(not(feature = "desktop-lens"))]
    fn chip_action(_ctx: &Ctx, _x: usize, _y: usize) -> Option<Action> {
        None
    }

    /// The SAK latch: Super+Escape / Super+Space escape to the Spine;
    /// Super+Tab toggles.
    pub fn take_sak(arthur_focused: bool) -> Option<Action> {
        let pending = sys_input_sak_take();
        if pending <= 0 {
            return None;
        }
        let pending = pending as u8;
        // OT-66 L2: Super+L, and the activity bit (the idle time).
        #[cfg(feature = "login")]
        super::lock::note_sak(pending);
        if arthur_focused && pending & (SAK_ESCAPE | SAK_SPACE) != 0 {
            return Some(Action::Escape(b"sak"));
        }
        if pending & SAK_TAB != 0 {
            return Some(if arthur_focused {
                Action::Escape(b"sak-tab")
            } else {
                Action::FocusWindow(b"sak-tab")
            });
        }
        None
    }

    /// The sprite's rectangle with its hotspot at `at`, clipped to the
    /// screen.
    pub fn sprite_rect(at: (i32, i32), shape: Shape, screen: Rect) -> Rect {
        let (w, h, hx, hy) = shape.geometry();
        let (x0, y0) = (at.0 - hx, at.1 - hy);
        let (x1, y1) = (x0 + w as i32, y0 + h as i32);
        let (x0, y0) = (x0.max(0) as usize, y0.max(0) as usize);
        let (x1, y1) = (x1.max(0) as usize, y1.max(0) as usize);
        if x1 <= x0 || y1 <= y0 {
            return Rect::default();
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0).intersect(&screen)
    }

    /// Overlay `shape` with its hotspot at `at`, only inside `clip` (the
    /// region just recomposed and about to be committed).
    pub fn draw(out: &mut [u32], stride: usize, at: (i32, i32), shape: Shape, clip: Rect) {
        let (w, h, hx, hy) = shape.geometry();
        for row in 0..h {
            for col in 0..w {
                let (x, y) = (at.0 - hx + col as i32, at.1 - hy + row as i32);
                if x < 0 || y < 0 || !clip.contains(x as usize, y as usize) {
                    continue;
                }
                let px = match shape.cell(col, row) {
                    b'#' => FILL,
                    b'.' => OUTLINE,
                    _ => continue,
                };
                out[y as usize * stride + x as usize] = px;
            }
        }
    }

    /// DI-1b: the drag outline (where the frame would land), only inside
    /// `clip`.
    pub fn draw_outline(out: &mut [u32], stride: usize, frame: Rect, clip: Rect) {
        let edges = [
            Rect::new(frame.x, frame.y, frame.w, DRAG_OUTLINE_W),
            Rect::new(
                frame.x,
                frame.y + frame.h - DRAG_OUTLINE_W,
                frame.w,
                DRAG_OUTLINE_W,
            ),
            Rect::new(frame.x, frame.y, DRAG_OUTLINE_W, frame.h),
            Rect::new(
                frame.x + frame.w - DRAG_OUTLINE_W,
                frame.y,
                DRAG_OUTLINE_W,
                frame.h,
            ),
        ];
        for edge in edges {
            let r = edge.intersect(&clip);
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    out[y * stride + x] = DRAG_OUTLINE;
                }
            }
        }
    }

    /// Apply an escape or refocus: the keyboard moves between Arthur and
    /// the WM's own focus (the Spine), and the window's focus light follows.
    /// Both run `HANDOFF` (Chad DS-1a F1). The escape discards what Home's
    /// queue held. The refocus discards nothing: an escape is not a turn
    /// boundary for Arthur, whose window and session stay; keys typed to it
    /// before the SAK are its own, and keys typed while escaped went to the
    /// WM's focus, never to Arthur.
    pub fn apply(
        scene: &mut Scene,
        action: Action,
        arthur: Handle,
        wm_focus: &home::HomeFocus,
        escaped: &mut bool,
    ) -> Result<(), i32> {
        // DS-1c: selecting the Lens is an escape (the keyboard goes to the
        // WM's own focus; the Lens holds none) that also lights its chip.
        // Every escape and refocus sets the selection, so none can leave a
        // stale one behind.
        let (activator, code, to_window, lens_sel, prefix, via): (_, _, _, _, &[u8], _) =
            match action {
                Action::Move(dx, dy) => return scene.move_window(dx, dy),
                #[cfg(feature = "desktop-resize")]
                Action::Resize(edges, dx, dy) => return scene.request_resize(edges, dx, dy),
                Action::Escape(via) => (
                    wm_focus.activator,
                    80,
                    false,
                    false,
                    b"DESKTOP pointer escape focus=spine via=",
                    via,
                ),
                Action::FocusWindow(via) => (
                    arthur,
                    81,
                    true,
                    false,
                    b"DESKTOP pointer focus client=arthur via=",
                    via,
                ),
                #[cfg(feature = "desktop-lens")]
                Action::SelectLens(via) => (wm_focus.activator, 82, false, true, b"", via),
                // Only Home's ctx reports these (`intro::pointer_tick`).
                #[cfg(feature = "desktop-intro")]
                Action::HomePress(..) => return Ok(()),
            };
        hand_over(
            &mut Steps {
                swap: || home::swap_to(activator, code),
                discard: || {
                    if !to_window {
                        home::discard_queued(wm_focus, b"escape");
                    }
                    Ok(())
                },
                present: || {
                    *escaped = !to_window;
                    #[cfg(feature = "desktop-lens")]
                    scene.select_lens(lens_sel);
                    scene.set_focused(to_window);
                    if !lens_sel {
                        log_action(prefix, via);
                    }
                    scene.focus_transition(true)?;
                    // Logged once on screen: the switch is what was presented.
                    #[cfg(feature = "desktop-lens")]
                    if lens_sel {
                        let at = scene.pointer.as_ref().map_or((0, 0), |p| p.position());
                        log_select(via, at);
                    }
                    Ok(())
                },
            },
            &HANDOFF,
        )
    }

    /// While escaped, the WM's own focus holds the keyboard: Enter gives it
    /// back to the window; every other key is the WM's to ignore.
    pub fn escaped_keys(wm_events: Handle) -> Option<Action> {
        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        loop {
            match sys_input_focus_recv(wm_events, &mut event) {
                0 => {
                    if neo_atrium::keys::decode(event) == Some(neo_atrium::keys::Key::Enter) {
                        return Some(Action::FocusWindow(b"enter"));
                    }
                }
                EAGAIN => return None,
                _ => return None,
            }
        }
    }

    #[cfg(all(feature = "desktop-lens", feature = "desktop-session-boot-test"))]
    fn log_press(x: usize, y: usize) {
        super::Line::new()
            .s(b"DESKTOP pointer press at=")
            .n(x as u64)
            .s(b",")
            .n(y as u64)
            .s(b"\n")
            .emit();
    }

    #[cfg(feature = "desktop-lens")]
    fn log_select(via: &[u8], at: (i32, i32)) {
        super::Line::new()
            .s(b"DESKTOP pointer select target=lens via=")
            .s(via)
            .s(b" at=")
            .n(at.0.max(0) as u64)
            .s(b",")
            .n(at.1.max(0) as u64)
            .s(b"\n")
            .emit();
    }

    fn log_action(prefix: &[u8], via: &[u8]) {
        super::Line::new().s(prefix).s(via).s(b"\n").emit();
    }

    /// A9-C2 boot-test witness: each wheel record, and whether it reached
    /// Arthur (the pointer over its client) or was dropped.
    #[cfg(all(feature = "desktop-wheel", feature = "desktop-session-boot-test"))]
    fn log_wheel(wheel: i16, to_arthur: bool) {
        super::Line::new()
            .s(b"DESKTOP pointer wheel=")
            .i(i64::from(wheel))
            .s(if to_arthur {
                b" to=arthur\n"
            } else {
                b" to=none\n"
            })
            .emit();
    }

    /// Boot-test only: production never logs where the user clicks.
    #[cfg(feature = "desktop-session-boot-test")]
    fn log_forward(x: i32, y: i32) {
        super::Line::new()
            .s(b"DESKTOP pointer forward client=arthur phase=down local=")
            .n(x.max(0) as u64)
            .s(b",")
            .n(y.max(0) as u64)
            .s(b"\n")
            .emit();
    }
}

/// DS-1c: the Lens — a read-only directory instrument bloomed from Arthur's
/// `ListDir` action, served by the broker inside Arthur's `OP_LAUNCH` (Arthur
/// is blocked on the reply, so strict alternation holds). Its authority is
/// what it is spawned with: its session channel and one directory handle
/// opened `LOOKUP | READDIR`. It commits once and then waits, static, until
/// the session ends; the WM composites that frame beside Arthur's window.
#[cfg(feature = "desktop-lens")]
mod lens {
    use super::{
        Plane, Rect, Scene, SharedPixels, grant_surface, home, log, recv_request, send_reply,
    };
    use libneo::atrium_launch::{LENS_HEIGHT, LENS_NAMESPACE, LENS_WIDTH};
    use libneo::atrium_protocol::{
        OP_COMMIT, OP_CREATE_SURFACE, OP_IMPORT_ATTACH_BUFFER, ROLE_TEXT, Request, SurfaceRegistry,
    };
    use libneo::{
        ATRIUM_COMMIT, ATRIUM_MAP_WRITE, ATRIUM_QUERY, CapRights, FORMAT_XRGB8888_LE, FileRights,
        Handle, OPEN_ROOT_TEMPORARY, ProcessHandle, encode_buffer_desc, meta_rights,
        sys_buffer_create, sys_buffer_map, sys_chan_create, sys_channel_close, sys_close, sys_kill,
        sys_open, sys_spawn, sys_wait,
    };
    use libneo::{EAGAIN, INPUT_EVENT_V1_LEN, sys_input_focus_recv};
    use neo_atrium::keys::Key;
    use neo_atrium::motion::Kind;

    const W: usize = LENS_WIDTH as usize;
    const H: usize = LENS_HEIGHT as usize;
    const SURFACE_CAP_PROVENANCE: u64 = 0x4C45_4E53_5355_0001;
    /// The Lens's buffer provenance (`neo-worker` `lens.rs`).
    const BUFFER_CAP_PROVENANCE: u64 = 0x4C45_4E53_4255_0001;
    /// The directory the Lens is lent (DS-1c: the temporary root).
    const PATH: &str = "/";
    // The scene lays the Lens out at the size its client draws.
    const _: () = assert!(W == neo_atrium::layout::LENS_W && H == super::CLIENT_H);

    /// The WM's handles on a live Lens, released by [`end`].
    pub struct LensSession {
        session: Handle,
        session_client: Handle,
        surface: Handle,
        surface_client: Handle,
        process: ProcessHandle,
    }

    impl LensSession {
        /// Close everything but `session`, which the caller closes first
        /// (that is what wakes a live Lens with `EPIPE`). Chad DS-1c F1: the
        /// process handle is a capability, not a channel (`SYS_CLOSE` refuses
        /// it): revoked, it leaves the WM's table. A slot never filled
        /// (`u32::MAX`) is skipped.
        fn release(&self) {
            for handle in [self.session_client, self.surface, self.surface_client] {
                if handle.0 != u32::MAX {
                    let _ = sys_channel_close(handle);
                }
            }
            let _ = libneo::sys_revoke(Handle(self.process.0));
        }
    }

    /// Chad DS-1c F1 discriminator (boot test): the first
    /// [`FAILED_ATTEMPTS`](leak_check::FAILED_ATTEMPTS) blooms fail late, and
    /// before the next one the WM's handle table and mappings must be exactly
    /// where they were before the first: a probe channel pair lands in the
    /// same (first-free) slots, and a probe mapping lands exactly one probe
    /// stride past the baseline's (the mmap cursor never reuses an address,
    /// so any mapping made in between would push it further).
    #[cfg(feature = "desktop-lens-fault-test")]
    pub mod leak_check {
        use core::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
        use libneo::{
            FORMAT_XRGB8888_LE, Handle, encode_buffer_desc, sys_buffer_create, sys_buffer_map,
            sys_chan_create, sys_channel_close,
        };

        pub const FAILED_ATTEMPTS: u32 = 2;
        static PROBE: AtomicU32 = AtomicU32::new(u32::MAX);
        static SLOTS: AtomicU64 = AtomicU64::new(0);
        static VA: AtomicU64 = AtomicU64::new(0);
        static STRIDE: AtomicU64 = AtomicU64::new(0);

        /// The first `2 * PAIRS` free handle slots (FNV-1a over their
        /// indices). A bloom holds ~8 handles at once, so a handle it leaked
        /// sits among these lowest free slots once the rest are closed —
        /// first-free allocation would hand that slot to a probe otherwise.
        fn slots() -> u64 {
            const PAIRS: usize = 16;
            let mut pairs = [(Handle(u32::MAX), Handle(u32::MAX)); PAIRS];
            let mut hash = 0xcbf2_9ce4_8422_2325u64;
            for pair in pairs.iter_mut() {
                let Ok((a, b)) = sys_chan_create() else {
                    hash = u64::MAX;
                    break;
                };
                *pair = (a, b);
                for index in [a.0, b.0] {
                    hash = (hash ^ u64::from(index)).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
            for (a, b) in pairs {
                if a.0 != u32::MAX {
                    let _ = sys_channel_close(a);
                    let _ = sys_channel_close(b);
                }
            }
            hash
        }

        fn map_probe() -> u64 {
            sys_buffer_map(Handle(PROBE.load(Relaxed)), 1).map_or(0, |va| va as u64)
        }

        pub fn at_attempt(attempt: u32) {
            if attempt == 1 {
                let desc = encode_buffer_desc(32, 32, 128, FORMAT_XRGB8888_LE, 4096);
                if let Ok(probe) = sys_buffer_create(&desc) {
                    PROBE.store(probe.0, Relaxed);
                }
                SLOTS.store(slots(), Relaxed);
                let first = map_probe();
                let second = map_probe();
                STRIDE.store(second.wrapping_sub(first), Relaxed);
                VA.store(second, Relaxed);
            } else if attempt == FAILED_ATTEMPTS + 1 {
                let handles_leaked = u64::from(slots() != SLOTS.load(Relaxed));
                let va = map_probe();
                let stride = STRIDE.load(Relaxed).max(1);
                let maps_leaked = va
                    .wrapping_sub(VA.load(Relaxed))
                    .checked_div(stride)
                    .map_or(u64::MAX, |n| n.wrapping_sub(1));
                super::super::Line::new()
                    .s(b"DESKTOP lens leak-check failed_blooms=")
                    .n(u64::from(FAILED_ATTEMPTS))
                    .s(b" handles_leaked=")
                    .n(handles_leaked)
                    .s(b" maps_leaked=")
                    .n(maps_leaked)
                    .s(b"\n")
                    .emit();
            }
        }
    }

    /// One client session's Lens: the live one, and its attempt budget.
    #[derive(Default)]
    pub struct Slot {
        pub session: Option<LensSession>,
        pub attempts: libneo::atrium_launch::LensAttempts,
    }

    /// Chad DS-1c F1: what the WM keeps for every Lens, made at the first
    /// bloom and never again: the buffer it lends each Lens (it keeps the
    /// handle; each Lens's derived handle and mapping go with its process),
    /// its one read-only mapping, and the WM's copy plane.
    pub struct LensStore {
        buffer: Handle,
        shared: SharedPixels,
        pub copy: Plane,
        poisoned: bool,
    }

    impl LensStore {
        /// Made once, with the WM's other planes (AAG: never retried, so a
        /// failure here cannot leak a mapping per attempt).
        pub fn new() -> Result<Self, i32> {
            let desc = encode_buffer_desc(
                LENS_WIDTH,
                LENS_HEIGHT,
                LENS_WIDTH * 4,
                FORMAT_XRGB8888_LE,
                (W * H * 4) as u64,
            );
            let buffer = sys_buffer_create(&desc).map_err(|_| 108)?;
            // Read-only: the WM never writes the pixels it lends.
            let pixels = sys_buffer_map(buffer, 1).map_err(|_| 109)? as *const u32;
            // SAFETY: the RO mapping covers the descriptor just created
            // (exactly W * H tightly packed pixels) and is kept for the life
            // of the WM. A Lens writes the pages while it lives, so they are
            // read only through `SharedPixels` (S1-F2).
            let shared = unsafe { SharedPixels::new(pixels, W, H) };
            let (_copy_handle, copy) = Plane::create(W, H).map_err(|c| 120 + c)?;
            Ok(Self {
                buffer,
                shared,
                copy,
                poisoned: false,
            })
        }
    }

    fn store(scene: &mut Scene) -> Result<&LensStore, i32> {
        match scene.lens.as_ref() {
            Some(store) if !store.poisoned => Ok(store),
            _ => Err(108),
        }
    }

    /// AAG: a Lens the WM could not reap may still hold the lent buffer
    /// writable; no later Lens is lent it (every later bloom is refused).
    /// `ESRCH` is reaped: a killed process leaves the table at once (with
    /// no exit code to collect).
    fn reaped(scene: &mut Scene, exit: Result<i32, i64>) -> i64 {
        if matches!(exit, Err(e) if e != libneo::ESRCH)
            && let Some(store) = scene.lens.as_mut()
        {
            store.poisoned = true;
        }
        exit.map_or(-1, i64::from)
    }

    /// Spawn the Lens, serve its create → import → first commit, and show
    /// it. On any failure the Lens is killed and nothing is shown.
    pub fn bloom(scene: &mut Scene, attempt: u32) -> Result<LensSession, i32> {
        if scene.chrome.layout.lens.is_none() {
            return Err(90);
        }
        store(scene)?;
        #[cfg(feature = "desktop-lens-fault-test")]
        leak_check::at_attempt(attempt);
        let (session, session_client) = sys_chan_create().map_err(|_| 91)?;
        let dir = match sys_open(
            PATH,
            OPEN_ROOT_TEMPORARY,
            FileRights::LOOKUP.union(FileRights::READDIR),
        ) {
            Ok(dir) => dir,
            Err(_) => {
                let _ = sys_channel_close(session);
                let _ = sys_channel_close(session_client);
                return Err(92);
            }
        };
        // File I/O reserves fd 1 (stdout) for file caps, so the directory
        // goes at index 2 behind a placeholder channel, closed once spawned.
        let Ok((slot_peer, slot)) = sys_chan_create() else {
            let _ = sys_close(dir);
            let _ = sys_channel_close(session);
            let _ = sys_channel_close(session_client);
            return Err(92);
        };
        let spawned = sys_spawn("worker", &[session_client, slot, Handle(dir.0)]);
        // The Lens holds its own copy; the WM keeps none.
        let _ = sys_close(dir);
        let _ = sys_channel_close(slot_peer);
        let _ = sys_channel_close(slot);
        let process = match spawned {
            Ok((_pid, process)) => process,
            Err(_) => {
                let _ = sys_channel_close(session);
                let _ = sys_channel_close(session_client);
                return Err(93);
            }
        };
        let mut lens = LensSession {
            session,
            session_client,
            surface: Handle(u32::MAX),
            surface_client: Handle(u32::MAX),
            process,
        };
        match serve(scene, &mut lens, attempt) {
            Ok(()) => {
                log(b"DESKTOP lens bloomed path=/ rights=lookup,readdir focus=none\n");
                Ok(lens)
            }
            Err(code) => {
                // The Lens may already have exited (its own failure code, the
                // discriminator for a failed bloom); else it is killed.
                let _ = sys_kill(lens.process);
                let exit = reaped(scene, sys_wait(lens.process, 0));
                let _ = sys_channel_close(lens.session);
                lens.release();
                log_failure(code, exit);
                Err(code)
            }
        }
    }

    /// DL-1: `DESKTOP lens slide from=X,Y to=X,Y lens_at=X,Y` (boot test):
    /// the window was moved so the Lens fits beside it.
    #[cfg(feature = "desktop-resize")]
    pub fn log_slide(from: (usize, usize), layout: &neo_atrium::Layout) {
        #[cfg(feature = "desktop-session-boot-test")]
        {
            let at = layout.lens.map_or((0, 0), |l| (l.frame.x, l.frame.y));
            super::Line::new()
                .s(b"DESKTOP lens slide from=")
                .n(from.0 as u64)
                .s(b",")
                .n(from.1 as u64)
                .s(b" to=")
                .n(layout.frame.x as u64)
                .s(b",")
                .n(layout.frame.y as u64)
                .s(b" lens_at=")
                .n(at.0 as u64)
                .s(b",")
                .n(at.1 as u64)
                .s(b"\n")
                .emit();
        }
        #[cfg(not(feature = "desktop-session-boot-test"))]
        let _ = (from, layout);
    }

    fn log_failure(stage: i32, exit: i64) {
        super::Line::new()
            .s(b"DESKTOP lens failed stage=")
            .i(i64::from(stage))
            .s(b" lens_exit=")
            .i(exit)
            .s(b"\n")
            .emit();
    }

    fn serve(scene: &mut Scene, lens: &mut LensSession, attempt: u32) -> Result<(), i32> {
        let screen = scene.chrome.layout.screen;
        let mut registry = SurfaceRegistry::new(LENS_NAMESPACE, screen.w as u32, screen.h as u32);
        let (create_header, create) = match recv_request(lens.session) {
            Ok((header, Request::Create(create)))
                if create.role == ROLE_TEXT
                    && create.width == LENS_WIDTH
                    && create.height == LENS_HEIGHT =>
            {
                (header, create)
            }
            _ => return Err(94),
        };
        let (surface, surface_client) = sys_chan_create().map_err(|_| 95)?;
        lens.surface = surface;
        lens.surface_client = surface_client;
        let identity = registry
            .create(create, SURFACE_CAP_PROVENANCE)
            .map_err(|_| 96)?;
        if !send_reply(
            lens.session,
            OP_CREATE_SURFACE,
            create_header.request_id,
            identity,
        ) || !grant_surface(lens.session, surface_client)
        {
            return Err(97);
        }
        // Chad DS-1c F1: lend the WM's own Lens buffer (O / R / grant / A on
        // the surface channel): write, query and commit, never grant onward.
        let buffer = store(scene)?.buffer;
        let lent = CapRights::new(
            meta_rights::USE,
            ATRIUM_MAP_WRITE | ATRIUM_QUERY | ATRIUM_COMMIT,
        );
        if !super::grant_over(surface, buffer, lent) {
            return Err(99);
        }
        let (import_header, import) = match recv_request(surface) {
            Ok((header, Request::ImportAttach(import))) => (header, import),
            _ => return Err(98),
        };
        if registry
            .import_attach_on_channel(
                import,
                identity.surface_cap_provenance,
                BUFFER_CAP_PROVENANCE,
            )
            .is_err()
            || !send_reply(
                surface,
                OP_IMPORT_ATTACH_BUFFER,
                import_header.request_id,
                identity,
            )
        {
            return Err(114);
        }
        let (commit_header, commit) = match recv_request(surface) {
            Ok((header, Request::Commit(commit))) => (header, commit),
            _ => return Err(115),
        };
        registry
            .commit_on_channel(commit, identity.surface_cap_provenance)
            .map_err(|_| 116)?;
        // Boot test (Chad DS-1c F1 discriminator): the first attempts fail
        // here, as late as a bloom can — the Lens has drawn and committed.
        #[cfg(feature = "desktop-lens-fault-test")]
        if attempt <= leak_check::FAILED_ATTEMPTS {
            return Err(130);
        }
        #[cfg(not(feature = "desktop-lens-fault-test"))]
        let _ = attempt;
        let store = scene.lens.as_mut().ok_or(108)?;
        store
            .shared
            .snapshot(Rect::new(0, 0, W, H), store.copy.slice_mut());
        scene.show_lens(true)?;
        if !send_reply(surface, OP_COMMIT, commit_header.request_id, identity) {
            scene.show_lens(false)?;
            return Err(117);
        }
        Ok(())
    }

    /// End the Lens with its session: hide it, close its session channel
    /// (it sees `EPIPE` and exits) and reap it. A Lens that had already
    /// failed is logged, never fatal: an instrument's fault must not take
    /// the shell down. Only a failed present (fatal everywhere) is an error.
    pub fn end(scene: &mut Scene, lens: LensSession) -> Result<(), i32> {
        scene.show_lens(false)?;
        let _ = sys_channel_close(lens.session);
        let exit = reaped(scene, sys_wait(lens.process, 0));
        lens.release();
        match exit {
            0 => log(b"DESKTOP lens ended exit=0\n"),
            other => log_failure(118, other),
        }
        Ok(())
    }

    /// Read Home's choice while Arthur is held. When it is the Lens's chip,
    /// switch to the Lens: the window view with Arthur unfocused (still
    /// held) and the Lens's chip lit, the keyboard on the WM's own focus.
    /// Tab or Enter switches to Arthur (returns the empty intent: resume);
    /// Esc goes back to Home.
    pub fn choose(
        scene: &mut Scene,
        focus: &home::HomeFocus,
    ) -> Result<neo_atrium::keys::Line<{ home::MAX_INTENT }>, i32> {
        loop {
            let intent = home::read_intent(scene, focus)?;
            // Only Enter on the Lens's chip (the empty intent) switches to the
            // Lens; a typed intent always goes to Arthur (AAG: never lose it
            // to a stale selection).
            if !intent.is_empty() || !scene.chrome.lens_selected() {
                return Ok(intent);
            }
            if view(scene, focus)? {
                return Ok(neo_atrium::keys::Line::default());
            }
            home::hold(scene, focus)?;
        }
    }

    /// The Lens view; `Ok(true)`: on to Arthur, `Ok(false)`: back to Home.
    fn view(scene: &mut Scene, focus: &home::HomeFocus) -> Result<bool, i32> {
        scene.show_window();
        scene.view_transition(Kind::View, true)?;
        #[cfg(feature = "desktop-session-boot-test")]
        {
            let clients = [
                scene.geo.client,
                scene
                    .chrome
                    .layout
                    .lens
                    .map(|l| super::wm_rect(l.client))
                    .unwrap_or_default(),
            ];
            home::log_stale_outside(scene, b"lens", &clients);
            home::log_residue(scene, neo_atrium::View::Window { focused: false }, b"lens");
        }
        log(b"DESKTOP lens view shown selected=lens via=chip focus=wm-home\n");
        loop {
            // OT-66 L2: locked, the WM's focus is the lock screen's.
            #[cfg(feature = "login")]
            if super::lock::service(scene)? {
                continue;
            }
            let mut event = [0u8; INPUT_EVENT_V1_LEN];
            match sys_input_focus_recv(focus.events, &mut event) {
                0 => {}
                EAGAIN => {
                    #[cfg(feature = "desktop-pointer")]
                    scene.pointer_idle();
                    // OT-22: park on the WM's focus, the route and SAK.
                    #[cfg(feature = "desktop-park")]
                    scene.park(
                        &[libneo::WaitItem {
                            handle: focus.events.0,
                            kind: libneo::WAIT_KIND_INPUT,
                        }],
                        u64::MAX,
                    );
                    #[cfg(not(feature = "desktop-park"))]
                    core::hint::spin_loop();
                    continue;
                }
                _ => return Err(78),
            }
            let to_arthur: &[u8] = match neo_atrium::keys::decode(event) {
                Some(Key::Tab) => b"tab",
                Some(Key::Enter) => b"enter",
                Some(Key::Escape) => {
                    scene.select_lens(false);
                    log(b"DESKTOP lens view left to=home via=esc\n");
                    return Ok(false);
                }
                _ => continue,
            };
            scene.select_lens(false);
            super::Line::new()
                .s(b"DESKTOP lens view left to=arthur via=")
                .s(to_arthur)
                .s(b"\n")
                .emit();
            return Ok(true);
        }
    }

    impl Scene {
        /// Put the selection on the Lens (`true`) or back: its chip wears the
        /// focus light. Both chips and their lights are inside the Spine,
        /// which is repainted (the caller presents it).
        pub fn select_lens(&mut self, on: bool) {
            if self.chrome.lens_selected() != on && self.chrome.set_lens_selected(on) {
                self.repaint_chrome(super::wm_rect(self.chrome.layout.spine));
            }
        }

        /// OT-87 R4.4b: put the selection on the Install chip (`true`) or
        /// off it: its chip wears the focus light. Inside the Spine, which is
        /// repainted (the caller presents it).
        #[cfg(all(feature = "installer", feature = "desktop-intro"))]
        pub fn select_install(&mut self, on: bool) {
            if self.chrome.install_selected() != on && self.chrome.set_install_selected(on) {
                self.repaint_chrome(super::wm_rect(self.chrome.layout.spine));
            }
        }

        /// Show (the WM's copy of the Lens's pixels) or hide the Lens:
        /// repaint its regions of the base (the frame and halo, the second
        /// chip) and present each one. Each is presented on its own: their
        /// bounding box would span Arthur's client, which this present does
        /// not carry.
        pub fn show_lens(&mut self, on: bool) -> Result<(), i32> {
            // DL-1: a Lens shown after the window moved re-blurs its glass:
            // a move's blur, not a re-blur during typing.
            #[cfg(feature = "desktop-pointer")]
            let shown_ok = super::count_move_blurs(|| self.chrome.set_lens(on));
            #[cfg(not(feature = "desktop-pointer"))]
            let shown_ok = self.chrome.set_lens(on);
            if !shown_ok {
                return Err(119);
            }
            let shown = self.repaint_lens(if on { Kind::Bloom } else { Kind::Unbloom });
            if shown.is_err() && on {
                // A bloom whose present failed is refused: leave no Lens in
                // the scene (the base keeps none either once repainted).
                self.chrome.set_lens(false);
                let _ = self.repaint_lens(Kind::Unbloom);
            }
            shown
        }

        /// Repaint the Lens's regions (the frame and halo, the second chip)
        /// and present each one — DM-1: as one bloom / close transition
        /// around the halo.
        fn repaint_lens(&mut self, kind: Kind) -> Result<(), i32> {
            let stride = self.geo.screen.w;
            let mut rects = [Rect::default(); 2];
            for (slot, r) in rects.iter_mut().zip(self.chrome.layout.lens_regions()) {
                *slot = super::wm_rect(r);
                if self.home.is_some() {
                    self.repaint_chrome(*slot);
                } else {
                    self.chrome
                        .paint_rect(self.base.slice_mut(), stride, self.focused, r);
                }
            }
            // The window view carries Arthur's client, which these regions
            // never cover (a no-op layer, but never base over it).
            self.transition(kind, rects[0], &rects, self.home.is_none(), &rects)
        }
    }
}

/// A9-D: the busy emblem. Arthur reserves a rect of its surface (`OP_BUSY`)
/// before it blocks in a generation; the WM blends `neo_atrium::busy`'s
/// frames into it, one every `FRAME_NS` by its own monotonic clock, from the
/// session loop (which spins, or parks until the next frame with
/// `desktop-park`). Arthur's next commit ends it and repaints the rect from
/// Arthur's pixels. Nothing is drawn while Home shows, and the emblem only
/// ever covers Arthur's own client area.
#[cfg(feature = "desktop-busy")]
mod busy {
    use super::{Rect, Scene, atrium_rect, wm_rect};
    use libneo::{CLOCK_MONOTONIC, sys_clock_gettime};
    use neo_atrium::busy::{FRAME_NS, frame, frame_at, place};

    pub struct State {
        /// The reserved rect, in Arthur's surface coordinates.
        area: Rect,
        start_ns: u64,
        /// The frame on screen.
        shown: usize,
        /// Presents made for the emblem (its first one included).
        #[cfg_attr(not(feature = "desktop-session-boot-test"), allow(dead_code))]
        presents: u64,
    }

    fn now() -> u64 {
        sys_clock_gettime(CLOCK_MONOTONIC).unwrap_or(0)
    }

    impl Scene {
        /// Where the emblem is on the output now, or nothing.
        fn busy_dest(&self) -> Option<(Rect, usize)> {
            #[cfg(feature = "desktop-home")]
            if self.home.is_some() {
                return None;
            }
            let b = self.busy.as_ref()?;
            let c = self.geo.client;
            let area = Rect::new(c.x + b.area.x, c.y + b.area.y, b.area.w, b.area.h);
            place(atrium_rect(area), atrium_rect(c)).map(|(dest, offset)| (wm_rect(dest), offset))
        }

        /// The emblem's layer: its rect and the shown frame's pixels from
        /// that rect's top-left (stride `SIDE`).
        pub(super) fn busy_layer(&self) -> Option<(Rect, &'static [u32])> {
            let (dest, offset) = self.busy_dest()?;
            let shown = self.busy.as_ref()?.shown;
            Some((dest, &frame(shown)[offset..]))
        }

        pub(super) fn start_busy(
            &mut self,
            request: libneo::atrium_protocol::Busy,
        ) -> Result<(), i32> {
            let area = Rect::new(
                usize::from(request.x),
                usize::from(request.y),
                usize::from(request.width),
                usize::from(request.height),
            );
            // A second request replaces the first; repaint where it was.
            let old = self.end_busy();
            self.busy = Some(State {
                area,
                start_ns: now(),
                shown: 0,
                presents: 0,
            });
            let damage = self
                .busy_dest()
                .map(|(d, _)| d)
                .unwrap_or_default()
                .union(&old);
            self.present_busy(damage)
        }

        /// End the emblem; returns the output rect it covered (to repaint).
        pub(super) fn end_busy(&mut self) -> Rect {
            let dest = self.busy_dest().map(|(d, _)| d).unwrap_or_default();
            if let Some(_b) = self.busy.take() {
                #[cfg(feature = "desktop-session-boot-test")]
                log_busy_end(&_b);
            }
            dest
        }

        /// Show the next frame if its time has come.
        pub(super) fn busy_tick(&mut self) -> Result<(), i32> {
            let Some(b) = self.busy.as_ref() else {
                return Ok(());
            };
            let next = frame_at(now().saturating_sub(b.start_ns));
            if next == b.shown {
                return Ok(());
            }
            if let Some(b) = self.busy.as_mut() {
                b.shown = next;
            }
            let damage = self.busy_dest().map(|(d, _)| d).unwrap_or_default();
            self.present_busy(damage)
        }

        /// The monotonic instant of the next frame (`u64::MAX`: none due).
        #[cfg_attr(not(feature = "desktop-park"), allow(dead_code))]
        pub(super) fn busy_deadline(&self) -> u64 {
            match self.busy.as_ref() {
                Some(b) => {
                    let elapsed = now().saturating_sub(b.start_ns);
                    b.start_ns + (elapsed / FRAME_NS + 1) * FRAME_NS
                }
                None => u64::MAX,
            }
        }

        fn present_busy(&mut self, damage: Rect) -> Result<(), i32> {
            if damage.is_empty() {
                return Ok(());
            }
            self.present(true, damage).map_err(|code| 122 + code)?;
            if let Some(b) = self.busy.as_mut() {
                b.presents += 1;
            }
            Ok(())
        }
    }

    /// Boot-test witness: how long the emblem showed, and how many frames
    /// the WM presented for it, on its own clock, while Arthur was blocked.
    #[cfg(feature = "desktop-session-boot-test")]
    fn log_busy_end(b: &State) {
        super::Line::new()
            .s(b"DESKTOP busy emblem ended ms=")
            .n(now().saturating_sub(b.start_ns) / 1_000_000)
            .s(b" presents=")
            .n(b.presents)
            .s(b" last_frame=")
            .n(b.shown as u64)
            .s(b"\n")
            .emit();
    }
}

/// THINK-2026-11-02-time-zones §2.5: Home's `timezone` and `rtc` verbs,
/// answered with a card (the help card's slot), never by Arthur.
#[cfg(all(feature = "desktop-clock", feature = "desktop-intro"))]
mod zone_verb {
    use super::clock::Basis;
    use super::{Line, Scene};
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;
    use libneo::tz::{self, ClockVerb, ZoneId};

    fn zone_name(id: ZoneId) -> &'static str {
        tz::zone(id).map_or("Etc/UTC", |z| z.name)
    }

    /// "EST, UTC-05:00" for the zone now. Only for confirmed UTC: before
    /// that, realtime isn't a UTC instant to convert.
    fn now_label(id: ZoneId) -> String {
        let now = libneo::sys_clock_gettime(libneo::CLOCK_REALTIME).unwrap_or(0) / 1_000_000_000;
        let (_, o) = tz::utc_to_local(now as i64, id);
        let sign = if o.seconds < 0 { '-' } else { '+' };
        let m = o.seconds.unsigned_abs() / 60;
        format!("{}, UTC{}{:02}:{:02}", o.abbr, sign, m / 60, m % 60)
    }

    fn status_rows(scene: &Scene, head: String) -> Vec<String> {
        let mode = scene.clock.mode();
        let mut rows = Vec::new();
        rows.push(head);
        rows.push(if mode.zone == tz::UTC && mode.rtc_local {
            String::from("Time zone: not set")
        } else if mode.basis == Basis::Utc {
            format!(
                "Time zone: {} ({})",
                zone_name(mode.zone),
                now_label(mode.zone)
            )
        } else {
            format!("Time zone: {}", zone_name(mode.zone))
        });
        rows.push(String::from(if mode.rtc_local {
            "This computer's clock keeps local time (as Windows does)."
        } else {
            "This computer's clock keeps UTC."
        }));
        match mode.basis {
            // The kill-switch kernel: the readout is the clock as read.
            Basis::Legacy => {
                rows.push(String::from(
                    "Saved. This system shows the computer's clock",
                ));
                rows.push(String::from("as it reads, without a time zone."));
                return rows;
            }
            Basis::Rtc => rows.push(String::from("Set your zone: timezone America/New_York")),
            Basis::Utc => rows.push(String::from("Change it: timezone Area/City")),
        }
        if mode.restart {
            rows.push(String::from(
                "Restart to read this computer's clock this way.",
            ));
        }
        rows.push(String::from(if mode.rtc_local {
            "Off by whole hours? Type: rtc utc"
        } else {
            "Off by whole hours? Type: rtc local"
        }));
        rows
    }

    pub fn run(scene: &mut Scene, line: &[u8]) -> Result<(), i32> {
        let Some(verb) = tz::parse_verb(line) else {
            return Ok(());
        };
        let mode = scene.clock.mode();
        let (zone, rtc_local) = (mode.zone, mode.rtc_local);
        let rows = match verb {
            ClockVerb::Show => status_rows(scene, String::from("The clock")),
            ClockVerb::Zone(typed) => match tz::find_zone(typed) {
                Ok(id) => {
                    super::clock::set_mode(scene, id, rtc_local)?;
                    Line::new()
                        .s(b"DESKTOP zone set name=")
                        .s(zone_name(id).as_bytes())
                        .s(b" id=")
                        .n(u64::from(id))
                        .s(b" restart=")
                        .n(u64::from(scene.clock.mode().restart))
                        .s(b"\n")
                        .emit();
                    status_rows(scene, format!("Time zone set to {}.", zone_name(id)))
                }
                Err(near) => {
                    let typed = core::str::from_utf8(typed).unwrap_or("?");
                    let mut rows = Vec::new();
                    rows.push(format!("There's no time zone called \"{typed}\"."));
                    let names: Vec<&str> = near.iter().flatten().map(|&i| zone_name(i)).collect();
                    if names.is_empty() {
                        rows.push(String::from("Type timezone Area/City, for example"));
                        rows.push(String::from(
                            "timezone America/New_York or timezone Europe/London.",
                        ));
                    } else {
                        rows.push(String::from("Did you mean:"));
                        for n in names {
                            rows.push(format!("  timezone {n}"));
                        }
                    }
                    Line::new().s(b"DESKTOP zone unknown\n").emit();
                    rows
                }
            },
            ClockVerb::Rtc(local) => {
                super::clock::set_mode(scene, zone, local)?;
                Line::new()
                    .s(b"DESKTOP zone rtc=")
                    .s(if local { b"local" } else { b"utc" })
                    .s(b" restart=")
                    .n(u64::from(scene.clock.mode().restart))
                    .s(b"\n")
                    .emit();
                status_rows(
                    scene,
                    String::from(if local {
                        "Saved: this computer's clock keeps local time."
                    } else {
                        "Saved: this computer's clock keeps UTC."
                    }),
                )
            }
        };
        super::intro::show_zone(scene, rows)
    }
}

/// OT-85 (`desktop-clock`): the Home readout and the wall-clock words.
///
/// * The time is `CLOCK_REALTIME`, which the kernel (`rtc-epoch`) seeded from
///   the RTC as read, formatted with `libneo::civil` (the probe's formatter).
///   THINK-2026-11-02-time-zones: once the settings make the RTC's UTC
///   instant known, the WM confirms realtime as UTC (`rtc-offset`,
///   write-once) and from then on only the display applies the zone
///   (`tz::shown`). Unconfirmed, the reading shows as it is, marked "zone
///   not set"; on a kernel without `rtc-offset`, exactly as before.
/// * Only a real epoch is shown: realtime - monotonic read inside one tick is
///   0 when the kernel has none (no `rtc-epoch`, or its fallback), and then
///   there is no readout and the intro keeps "Good day" (never a 1970 date).
/// * Refresh at minute granularity: `deadline` is the monotonic instant of
///   the next minute boundary, Home's park deadline (`desktop-park`), so the
///   WM wakes once a minute at most for the clock; `tick` then repaints the
///   readout's box (and the intro card, while it shows) only if the minute
///   changed. No thread, no timer, nothing per second. Without
///   `desktop-park` the Home loop already spins; `tick` costs it one
///   monotonic read per turn and reads realtime only past the deadline.
#[cfg(feature = "desktop-clock")]
mod clock {
    use super::{Line, Scene, wm_rect};
    use alloc::string::String;
    use libneo::civil::{self, Civil, DateText, TimeText};
    pub use libneo::tz::Basis;
    use libneo::tz::{self, Shown, ZoneId};
    use libneo::{CLOCK_MONOTONIC, CLOCK_REALTIME, sys_clock_gettime};

    const NS_PER_MINUTE: u64 = 60 * civil::NS_PER_SECOND;
    const NS: i64 = civil::NS_PER_SECOND as i64;

    pub struct State {
        /// The minute the readout shows; `None`: no epoch, no readout.
        shown: Option<Civil>,
        /// Monotonic ns of the next minute boundary (`u64::MAX`: never).
        next_ns: u64,
        /// The zone and the RTC mode in force (the settings store's, or the
        /// defaults: no zone, an RTC that holds local time).
        zone: ZoneId,
        rtc_local: bool,
        /// What realtime holds (`SYS_CLOCK_REALTIME_STATUS`). Once it is
        /// [`Basis::Utc`] the WM never re-interprets it: a later zone only
        /// changes the display, a later RTC mode waits for a restart.
        basis: Basis,
        /// The RTC reading this WM confirmed from, and the offset it
        /// confirmed, for the restart hint (`tz::needs_restart`). `None`:
        /// not confirmed by this WM.
        confirmed: Option<(i64, i64)>,
    }

    impl Default for State {
        fn default() -> Self {
            Self {
                shown: None,
                next_ns: u64::MAX,
                zone: tz::UTC,
                rtc_local: true,
                basis: Basis::Legacy,
                confirmed: None,
            }
        }
    }

    /// What the `timezone` card says about the clock.
    pub struct Mode {
        pub zone: ZoneId,
        pub rtc_local: bool,
        pub basis: Basis,
        /// The settings in force would read the computer's clock differently
        /// from the confirmed reading: only a restart applies them.
        pub restart: bool,
    }

    impl State {
        /// Home's park deadline: the next minute boundary, or never.
        pub fn deadline(&self) -> u64 {
            if self.shown.is_some() {
                self.next_ns
            } else {
                u64::MAX
            }
        }

        /// The words of the minute shown, for the intro's greeting row.
        pub fn now_words(&self) -> Option<Words> {
            self.shown.map(Words::of)
        }

        /// The zone, the RTC mode and the clock's state, for the card.
        pub fn mode(&self) -> Mode {
            let restart = match (self.basis, self.confirmed) {
                (Basis::Utc, Some((wall, offset))) => {
                    tz::needs_restart(wall, offset, self.zone, self.rtc_local)
                }
                _ => false,
            };
            Mode {
                zone: self.zone,
                rtc_local: self.rtc_local,
                basis: self.basis,
                restart,
            }
        }
    }

    /// One minute's texts (fixed buffers, no allocation).
    pub struct Words {
        hour: u8,
        time: TimeText,
        date: DateText,
    }

    impl Words {
        fn of(c: Civil) -> Self {
            Self {
                hour: c.hour,
                time: c.time_text(),
                date: c.date_text(),
            }
        }

        pub fn as_now(&self) -> neo_atrium::clock::Now<'_> {
            neo_atrium::clock::Now {
                hour: self.hour,
                time: self.time.as_str(),
                date: self.date.as_str(),
            }
        }
    }

    /// Realtime and monotonic read inside one tick (two equal monotonic
    /// reads bracket the realtime one), or `None`.
    fn sample() -> Option<(u64, u64)> {
        for _ in 0..64 {
            let m0 = sys_clock_gettime(CLOCK_MONOTONIC).ok()?;
            let r = sys_clock_gettime(CLOCK_REALTIME).ok()?;
            let m1 = sys_clock_gettime(CLOCK_MONOTONIC).ok()?;
            if m0 == m1 {
                return Some((r, m0));
            }
        }
        None
    }

    /// The wall time (ns) to show for `realtime` and its label
    /// (`tz::shown`: only confirmed UTC is converted to the zone).
    fn local(st: &State, realtime: u64) -> (u64, Shown) {
        let r = realtime as i64;
        let (wall_s, label) = tz::shown(r.div_euclid(NS), st.basis, st.zone);
        let wall = wall_s
            .saturating_mul(NS)
            .saturating_add(r.rem_euclid(NS))
            .max(0) as u64;
        (wall, label)
    }

    /// Set the readout to the minute of `realtime` (read at `mono`).
    fn set(scene: &mut Scene, realtime: u64, mono: u64) {
        let (wall, label) = local(&scene.clock, realtime);
        let c = civil::from_realtime_ns(wall);
        let w = Words::of(c);
        let mut time = String::from(w.time.as_str());
        match label {
            // The kill-switch: today's readout, no label.
            Shown::Plain => {}
            Shown::NotSet => time.push_str(" (zone not set)"),
            Shown::Zone(abbr) => {
                time.push(' ');
                time.push_str(abbr);
            }
        }
        scene
            .chrome
            .set_clock(Some((time.as_str(), w.date.as_str())));
        scene.clock.shown = Some(c);
        scene.clock.next_ns = mono.saturating_add(NS_PER_MINUTE - wall % NS_PER_MINUTE);
    }

    fn log_shown(scene: &Scene, why: &[u8], realtime: u64) {
        let (wall, label) = local(&scene.clock, realtime);
        let c = civil::from_realtime_ns(wall);
        let line = Line::new()
            .s(b"DESKTOP clock ")
            .s(why)
            .s(b" date=\"")
            .s(c.date_text().as_bytes())
            .s(b"\" time=\"")
            .s(c.time_text().as_bytes())
            .s(b"\"");
        let line = match label {
            Shown::Plain => line,
            Shown::NotSet => line.s(b" zone=\"zone not set\""),
            Shown::Zone(abbr) => line.s(b" zone=\"").s(abbr.as_bytes()).s(b"\""),
        };
        let line = line.s(b" realtime_s=").n(realtime / civil::NS_PER_SECOND);
        let line = if scene.clock.basis == Basis::Utc {
            line.s(b" utc_s=").n(realtime / civil::NS_PER_SECOND)
        } else {
            line
        };
        line.s(b"\n").emit();
    }

    /// Read the clock's state and, while realtime is the unconfirmed RTC
    /// reading, confirm it as UTC once the settings make its UTC instant
    /// known (THINK-2026-11-02 §2.1; `rtc-offset`, write-once). Once
    /// confirmed (by this WM or an earlier one) nothing is re-interpreted.
    fn resolve(st: &mut State, realtime: u64) {
        st.basis = Basis::of(libneo::sys_clock_realtime_status());
        if st.basis != Basis::Rtc {
            log_mode(
                st,
                if st.basis == Basis::Utc {
                    b"kept"
                } else {
                    b"legacy"
                },
                None,
            );
            return;
        }
        let wall_s = (realtime / civil::NS_PER_SECOND) as i64;
        let Some(offset) = tz::confirm_offset(wall_s, st.zone, st.rtc_local) else {
            log_mode(st, b"unknown", None);
            return;
        };
        match libneo::sys_clock_confirm_realtime(offset) {
            Ok(()) => {
                st.basis = Basis::Utc;
                st.confirmed = Some((wall_s, offset));
                log_mode(st, b"confirmed", Some(offset));
            }
            Err(e) => {
                Line::new()
                    .s(b"DESKTOP clock confirm refused errno=")
                    .n(e.unsigned_abs())
                    .s(b"\n")
                    .emit();
                // Another confirm won the race: it stands.
                st.basis = Basis::of(libneo::sys_clock_realtime_status());
                log_mode(st, b"unconfirmed", None);
            }
        }
    }

    fn log_mode(st: &State, how: &[u8], offset_s: Option<i64>) {
        let name = tz::zone(st.zone).map_or("Etc/UTC", |z| z.name);
        let line = Line::new()
            .s(b"DESKTOP clock utc=")
            .s(how)
            .s(b" zone=")
            .s(name.as_bytes())
            .s(b" rtc=")
            .s(if st.rtc_local { b"local" } else { b"utc" });
        let line = match offset_s {
            Some(o) => line
                .s(if o < 0 { b" offset_s=-" } else { b" offset_s=" })
                .n(o.unsigned_abs()),
            None => line,
        };
        line.s(b"\n").emit();
    }

    /// The zone and RTC mode from the settings store (defaults without it).
    fn load_mode(st: &mut State) {
        #[cfg(feature = "atrium-settings-store")]
        {
            let s = super::settings::current();
            st.zone = s.time_zone;
            st.rtc_local = s.rtc_local;
        }
        #[cfg(not(feature = "atrium-settings-store"))]
        let _ = st;
    }

    /// At session start, before Home is first shown.
    pub fn start(scene: &mut Scene) {
        match sample() {
            Some((r, m)) if r != m => {
                load_mode(&mut scene.clock);
                resolve(&mut scene.clock, r);
                // `resolve` may have moved realtime (the kernel's confirm).
                let (r, m) = sample().unwrap_or((r, m));
                set(scene, r, m);
                // The boot instant, the probe's view of the same epoch.
                #[cfg(feature = "desktop-clock-boot-test")]
                {
                    let boot = civil::from_realtime_ns(r - m);
                    Line::new()
                        .s(b"DESKTOP clock boot date=\"")
                        .s(boot.date_text().as_bytes())
                        .s(b"\" time=\"")
                        .s(boot.time_text().as_bytes())
                        .s(b"\" base_s=")
                        .n((r - m) / civil::NS_PER_SECOND)
                        .s(b"\n")
                        .emit();
                }
                log_shown(scene, b"shown", r);
            }
            _ => {
                scene.chrome.set_clock(None);
                Line::new()
                    .s(b"DESKTOP clock off reason=boot-relative\n")
                    .emit();
            }
        }
    }

    /// The `timezone` / `rtc` verbs. The settings store keeps the new zone or
    /// RTC mode when the WM has one. While realtime is unconfirmed it may now
    /// be confirmed; once confirmed, a zone changes only the display and an
    /// RTC mode waits for a restart ([`Mode::restart`]).
    pub fn set_mode(scene: &mut Scene, zone: ZoneId, rtc_local: bool) -> Result<(), i32> {
        #[cfg(feature = "atrium-settings-store")]
        {
            use settings_core::Key;
            let _ = super::settings::set_wm(scene, Key::TimeZone.id(), u32::from(zone));
            let _ = super::settings::set_wm(scene, Key::RtcLocal.id(), u32::from(rtc_local));
        }
        scene.clock.zone = zone;
        scene.clock.rtc_local = rtc_local;
        if scene.clock.shown.is_none() {
            return Ok(());
        }
        let Some((r, m)) = sample() else {
            return Ok(());
        };
        if scene.clock.basis == Basis::Rtc {
            resolve(&mut scene.clock, r);
        } else {
            let how: &[u8] = if scene.clock.basis == Basis::Utc {
                b"kept"
            } else {
                b"legacy"
            };
            log_mode(&scene.clock, how, None);
        }
        let (r, m) = sample().unwrap_or((r, m));
        set(scene, r, m);
        let b = wm_rect(scene.chrome.layout.clock_slot());
        scene.repaint_chrome(b);
        scene.present(false, b).map_err(|c| 140 + c)?;
        log_shown(scene, b"zone", r);
        Ok(())
    }

    /// Home is being shown again (`show_home` repaints the box): the text
    /// only, for the current minute.
    pub fn refresh(scene: &mut Scene) {
        if scene.clock.shown.is_none() {
            return;
        }
        if let Some((r, m)) = sample() {
            let before = scene.clock.shown;
            set(scene, r, m);
            let now = scene.clock.shown;
            if before.zip(now).is_some_and(|(b, n)| !b.same_minute(&n)) {
                log_shown(scene, b"minute", r);
            }
        }
    }

    /// Home's idle turn: past the minute boundary, repaint and present the
    /// readout's box (and re-word the intro while it shows).
    pub fn tick(scene: &mut Scene) -> Result<(), i32> {
        let Some(shown) = scene.clock.shown else {
            return Ok(());
        };
        let Ok(mono) = sys_clock_gettime(CLOCK_MONOTONIC) else {
            return Ok(());
        };
        if mono < scene.clock.next_ns {
            return Ok(());
        }
        let Some((r, m)) = sample() else {
            return Ok(());
        };
        set(scene, r, m);
        if scene.clock.shown.is_some_and(|now| shown.same_minute(&now)) {
            // Early by a tick: nothing to repaint; `set` moved the deadline.
            return Ok(());
        }
        let b = wm_rect(scene.chrome.layout.clock_slot());
        scene.repaint_chrome(b);
        scene.present(false, b).map_err(|c| 140 + c)?;
        #[cfg(feature = "desktop-intro")]
        super::intro::refresh_clock(scene)?;
        log_shown(scene, b"minute", r);
        Ok(())
    }

    /// Boot-test residue witness: the readout's box is masked like the
    /// label boxes.
    #[cfg(feature = "desktop-session-boot-test")]
    pub fn text_box(scene: &Scene) -> neo_atrium::Rect {
        scene.chrome.clock_text_box()
    }
}
