// SPDX-License-Identifier: MPL-2.0
//! Arthur's ADR-073 `TEXT` client surface and ADR-074 InputFocus endpoint.

#![cfg_attr(not(feature = "arthur-compositor-baremetal"), allow(dead_code))]

use libneo::atrium_protocol::{
    CreateSurface, DestroySurface, FocusRequest, Identity, ImportAttachBuffer, OP_CREATE_SURFACE,
    OP_DESTROY_SURFACE, OP_FOCUS_REQUEST, OP_IMPORT_ATTACH_BUFFER, REPLY_LEN, ROLE_TEXT, Reply,
    Request, decode_reply, encode_request,
};
use libneo::{
    ATRIUM_FOCUS_ACTIVATE, ATRIUM_MAP_READ, ATRIUM_PRESENT, ATRIUM_QUERY, CapRights, EAGAIN, EPERM,
    FORMAT_XRGB8888_LE, FULL_CHANNEL_RIGHTS, Handle, INPUT_EVENT_V1_LEN, encode_buffer_desc,
    encode_damage_rect, meta_rights, sys_buffer_commit_damage, sys_buffer_create, sys_buffer_map,
    sys_fb_present, sys_grant, sys_input_focus_create, sys_input_focus_mint_activator,
    sys_input_focus_recv, sys_recv, sys_recv_grant, sys_send,
};

#[cfg(feature = "arthur-legacy-5x7-font")]
use crate::showcase_logic::LegacyGrid;
use crate::showcase_logic::{
    ANSWER_CLIENT_SERIAL, BUFFER_BYTES, BUFFER_HEIGHT, BUFFER_STRIDE, BUFFER_WIDTH, SURFACE_HEIGHT,
    SURFACE_WIDTH, input_event_to_ascii,
};

#[cfg(all(feature = "desktop-resize", feature = "arthur-legacy-5x7-font"))]
compile_error!("desktop-resize lays out neo-text; the legacy 5x7 grid is fixed at 640x400");
#[cfg(not(feature = "arthur-legacy-5x7-font"))]
use crate::showcase_text::{GREETING, TextSurface};

pub const SESSION: Handle = Handle(0);
pub const NAMESPACE_ID: u64 = 0x4152_5448_5552_0001;
pub const BUFFER_CAP_PROVENANCE: u64 = 0x4255_4646_4152_0001;
pub const ANSWER_SERIAL: u64 = ANSWER_CLIENT_SERIAL;
const BYPASS_POISON: u32 = 0x00D1_23E4;
const BYPASS_DST: (usize, usize) = (0, 0);

pub struct ArthurSurface {
    surface: Handle,
    identity: Identity,
    buffer: Handle,
    focus: Handle,
    /// The client RW mapping: exactly BUFFER_WIDTH x BUFFER_HEIGHT pixels
    /// (the surface, `width` x `height`, is its top-left corner).
    pixels: &'static mut [u32],
    /// The surface's current size: as created until the WM configures it.
    width: usize,
    height: usize,
    /// DI-1b resize: the WM's configure mailbox (read-only), and the last
    /// configure serial applied.
    #[cfg(feature = "desktop-resize")]
    mailbox: Option<&'static [core::sync::atomic::AtomicU32; 4]>,
    #[cfg(feature = "desktop-resize")]
    configure_serial: u64,
    /// Kill-switch: the pre-S2 5x7 uppercase grid, bit-for-bit.
    #[cfg(feature = "arthur-legacy-5x7-font")]
    text: LegacyGrid,
    /// Desktop S2: neo-text proportional AA type.
    #[cfg(not(feature = "arthur-legacy-5x7-font"))]
    text: TextSurface<'static>,
    request_id: u64,
    client_serial: u64,
    /// A9-C: a scroll key `poll_ascii` read, for the session loop to apply.
    #[cfg(feature = "arthur-scrollback")]
    scroll: core::cell::Cell<Option<crate::showcase_logic::ScrollKey>>,
}

impl ArthurSurface {
    pub fn create() -> Result<Self, i32> {
        let create = CreateSurface {
            identity: Identity {
                namespace_id: NAMESPACE_ID,
                surface_id: 0,
                surface_cap_provenance: 0,
            },
            role: ROLE_TEXT,
            width: SURFACE_WIDTH as u32,
            height: SURFACE_HEIGHT as u32,
            client_tag: 0x4152_5448_5552,
        };
        if !send_request(SESSION, 1, Request::Create(create)) {
            return Err(20);
        }
        let identity = recv_reply(SESSION, OP_CREATE_SURFACE, 1)
            .filter(|reply| reply.status == 0)
            .map(|reply| reply.identity)
            .ok_or(21)?;

        let mut offer = [0u8; 1];
        if sys_recv(SESSION, &mut offer) != 1 || offer[0] != b'O' || sys_send(SESSION, b"R") != 1 {
            return Err(22);
        }
        let surface = sys_recv_grant(SESSION, FULL_CHANNEL_RIGHTS).map_err(|_| 23)?;
        if sys_send(SESSION, b"A") != 1 {
            return Err(24);
        }

        let desc = encode_buffer_desc(
            BUFFER_WIDTH as u32,
            BUFFER_HEIGHT as u32,
            BUFFER_STRIDE as u32,
            FORMAT_XRGB8888_LE,
            BUFFER_BYTES as u64,
        );
        let buffer = sys_buffer_create(&desc).map_err(|_| 25)?;
        let mapping = sys_buffer_map(buffer, 0).map_err(|_| 26)? as *mut u32;
        // SAFETY: the mapping is this client's RW view of a buffer whose
        // immutable descriptor is exactly BUFFER_WIDTH x BUFFER_HEIGHT
        // XRGB8888 pixels (stride == width, BUFFER_BYTES long). It is never
        // unmapped while the process lives and only this thread touches it;
        // the compositor reads it after the commit syscall, a compiler barrier.
        let pixels: &'static mut [u32] =
            unsafe { core::slice::from_raw_parts_mut(mapping, BUFFER_WIDTH * BUFFER_HEIGHT) };
        let focus = sys_input_focus_create().map_err(|_| 27)?;
        let activator = sys_input_focus_mint_activator(focus).map_err(|_| 28)?;

        let mut this = Self {
            surface,
            identity,
            buffer,
            focus,
            pixels,
            width: SURFACE_WIDTH,
            height: SURFACE_HEIGHT,
            #[cfg(feature = "desktop-resize")]
            mailbox: None,
            #[cfg(feature = "desktop-resize")]
            configure_serial: 0,
            #[cfg(feature = "arthur-legacy-5x7-font")]
            text: LegacyGrid::new(),
            #[cfg(not(feature = "arthur-legacy-5x7-font"))]
            text: TextSurface::with_stride(BUFFER_WIDTH).ok_or(36)?,
            request_id: 2,
            client_serial: 0,
            #[cfg(feature = "arthur-scrollback")]
            scroll: core::cell::Cell::new(None),
        };
        #[cfg(feature = "arthur-legacy-5x7-font")]
        {
            this.text.clear(this.pixels);
            this.write_text(b"ARTHUR / LOCAL\n\nARTHUR IS READY.\nASK ME ANYTHING.\n\nYOU> ");
        }
        #[cfg(not(feature = "arthur-legacy-5x7-font"))]
        {
            this.text.paint_chrome(this.pixels);
            // A9-C boot test: enough earlier transcript to scroll through.
            #[cfg(feature = "arthur-scroll-seed-test")]
            for i in 0..20u8 {
                this.write_text(b"seed line ");
                this.write_text(&[b'0' + i / 10, b'0' + i % 10, b'\n']);
            }
            this.write_text(GREETING.as_bytes());
        }

        // AAG regression: attenuation of the surface buffer alone would not be
        // enough because a compromised client can create a fresh full-rights
        // buffer. Prove that exact attack: a one-pixel poison buffer commits,
        // then raw scanout refuses it because Arthur has no GpuSubmitCap.
        prove_client_scanout_denied()?;
        if sys_send(SESSION, b"B") != 1 {
            return Err(46);
        }

        let import = ImportAttachBuffer {
            identity,
            transfer_id: 1,
            buffer_cap_provenance: BUFFER_CAP_PROVENANCE,
        };
        if !send_request(
            surface,
            this.next_request_id(),
            Request::ImportAttach(import),
        ) {
            return Err(29);
        }
        let import_request_id = this.request_id - 1;
        let mut ready = [0u8; 1];
        if sys_recv(surface, &mut ready) != 1 || ready[0] != b'R' {
            return Err(30);
        }
        let compositor_buffer = CapRights::new(
            meta_rights::USE,
            ATRIUM_MAP_READ | ATRIUM_QUERY | ATRIUM_PRESENT,
        );
        if sys_grant(surface, buffer, compositor_buffer) != 0
            || recv_reply(surface, OP_IMPORT_ATTACH_BUFFER, import_request_id)
                .is_none_or(|reply| reply.status != 0)
        {
            return Err(31);
        }

        this.commit_next().map_err(|_| 32)?;
        let focus_request_id = this.next_request_id();
        if !send_request(
            surface,
            focus_request_id,
            Request::Focus(FocusRequest {
                identity,
                reason: libneo::atrium_protocol::FOCUS_REASON_USER_ACTIVATION,
                request_serial: 1,
            }),
        ) || recv_reply(surface, OP_FOCUS_REQUEST, focus_request_id)
            .is_none_or(|reply| reply.status != 0)
        {
            return Err(33);
        }
        let activate_only = CapRights::new(meta_rights::USE, ATRIUM_FOCUS_ACTIVATE);
        if sys_grant(SESSION, activator, activate_only) != 0 {
            return Err(34);
        }
        Ok(this)
    }

    /// OT-22: instead of one spin iteration, park until the focus has an
    /// event. A configure mailbox is a memory word with no waker (CP-57), so
    /// with one mapped the park also ends at the next tick, the rate at
    /// which a published size is taken. EAGAIN (no park slot) costs a spin.
    #[cfg(feature = "desktop-park")]
    pub fn park(&self) {
        use libneo::{WAIT_KIND_INPUT, WaitItem};
        #[cfg(feature = "desktop-resize")]
        let deadline = if self.mailbox.is_some() {
            libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC)
                .map_or(0, |now| now.saturating_add(10_000_000))
        } else {
            u64::MAX
        };
        #[cfg(not(feature = "desktop-resize"))]
        let deadline = u64::MAX;
        let item = [WaitItem {
            handle: self.focus.0,
            kind: WAIT_KIND_INPUT,
        }];
        if libneo::sys_wait_any(&item, deadline) == libneo::EAGAIN {
            core::hint::spin_loop();
        }
    }

    pub fn poll_ascii(&self) -> Result<Option<u8>, i32> {
        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        match sys_input_focus_recv(self.focus, &mut event) {
            0 => {
                #[cfg(feature = "desktop-pointer-probe")]
                log_pointer_event(event);
                #[cfg(feature = "arthur-scrollback")]
                if let Some(key) = crate::showcase_logic::input_event_to_scroll(event) {
                    self.scroll.set(Some(key));
                    return Ok(None);
                }
                Ok(input_event_to_ascii(event))
            }
            EAGAIN => Ok(None),
            // DI-1a: the WM moved the keyboard away (Super+Escape, a Spine
            // click). Not focused is "no input yet", not a failure: the
            // events resume when the WM gives the window focus back.
            EPERM => Ok(None),
            _ => Err(40),
        }
    }

    /// Chad DS-1a F1 (`libneo::atrium_handoff`), the discard step on the
    /// client side: called when a message hands Arthur a turn (the WM has
    /// already made Arthur's focus the route), it drops whatever the queue
    /// kept from before — keys typed while Arthur was held, or in the
    /// instant of the swap. Bounded (a focus queue holds at most 64 events).
    #[cfg(feature = "desktop-home")]
    pub fn discard_queued(&self) -> usize {
        let mut event = [0u8; INPUT_EVENT_V1_LEN];
        let mut discarded = 0;
        while discarded < 256 && sys_input_focus_recv(self.focus, &mut event) == 0 {
            discarded += 1;
        }
        discarded
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.text.write_byte(self.pixels, byte);
    }

    /// What the footer says about the model. Committed with the next frame.
    /// The legacy grid has no footer.
    pub fn set_footer(&mut self, model: crate::showcase_logic::FooterModel) {
        #[cfg(not(feature = "arthur-legacy-5x7-font"))]
        self.text.set_footer(self.pixels, model);
        #[cfg(feature = "arthur-legacy-5x7-font")]
        let _ = model;
    }

    /// A9-C: the scroll key the last [`Self::poll_ascii`] read, if any.
    #[cfg(feature = "arthur-scrollback")]
    pub fn take_scroll(&self) -> Option<crate::showcase_logic::ScrollKey> {
        self.scroll.take()
    }

    /// A9-C: scroll the transcript (committed with the next frame).
    #[cfg(feature = "arthur-scrollback")]
    pub fn scroll(&mut self, key: crate::showcase_logic::ScrollKey) -> bool {
        self.text.scroll(self.pixels, key)
    }

    /// A9-C: follow new text again (the user typed).
    #[cfg(feature = "arthur-scrollback")]
    pub fn follow(&mut self) {
        self.text.follow(self.pixels);
    }

    /// A9-C: the transcript line at the top of a scrolled-up view.
    #[cfg(all(feature = "arthur-scrollback", feature = "desktop-session-boot-test"))]
    pub fn view_top(&self) -> Option<u64> {
        self.text.view_top()
    }

    pub fn write_text(&mut self, text: &[u8]) {
        for &byte in text {
            self.write_byte(byte);
        }
    }

    pub fn commit_next(&mut self) -> Result<u64, i32> {
        let serial = self.client_serial.checked_add(1).ok_or(41)?;
        self.commit_serial(serial)
    }

    pub fn commit_answer(&mut self) -> Result<u64, i32> {
        self.commit_serial(ANSWER_SERIAL)
    }

    pub fn destroy(&mut self) -> Result<(), i32> {
        let request_id = self.next_request_id();
        if !send_request(
            self.surface,
            request_id,
            Request::Destroy(DestroySurface {
                identity: self.identity,
                final_client_serial: self.client_serial,
            }),
        ) || recv_reply(self.surface, OP_DESTROY_SURFACE, request_id)
            .is_none_or(|reply| reply.status != 0)
        {
            return Err(42);
        }
        Ok(())
    }

    /// DS-1: hand the keyboard back to Home (`OP_YIELD_FOCUS`). The WM
    /// replies, then holds this session until it sends a resume message.
    #[cfg(feature = "desktop-home")]
    pub fn yield_focus(&mut self) -> Result<(), i32> {
        let request_id = self.next_request_id();
        let serial = self.client_serial.max(1);
        libneo::atrium_client::yield_focus(self.surface, request_id, self.identity, serial)
            .map_err(|_| 46)
    }

    /// A9-D: show the WM's busy emblem in the header until the next commit:
    /// put the quiet "…" on the reply line, commit it and the text before
    /// it, then send `OP_BUSY` for the header's emblem rect. A WM that
    /// refuses the rect shows no emblem; the generation goes on without one.
    #[cfg(feature = "arthur-busy-emblem")]
    pub fn show_busy(&mut self) -> Result<(), i32> {
        let rect = self.text.reserve_busy(self.pixels);
        let serial = self.commit_next()?;
        let Some(rect) = rect else {
            return Ok(());
        };
        let accepted = self.busy(self.identity, serial, rect);
        #[cfg(feature = "arthur-busy-refusal-test")]
        self.probe_busy_refusals(&accepted, serial, rect)?;
        match accepted {
            Ok(()) | Err(libneo::atrium_client::RequestError::Refused(_)) => Ok(()),
            Err(_) => Err(47),
        }
    }

    #[cfg(feature = "arthur-busy-emblem")]
    fn busy(
        &mut self,
        identity: Identity,
        serial: u64,
        rect: (u16, u16, u16, u16),
    ) -> Result<(), libneo::atrium_client::RequestError> {
        let request_id = self.next_request_id();
        libneo::atrium_client::busy(self.surface, request_id, identity, serial, rect)
    }

    /// Janus #96 boot-test probe, on the first generation only: after
    /// `show_busy`'s valid `OP_BUSY` (its emblem now turning), one whose rect
    /// ends a pixel past the 640-wide surface (x=639, width=2) and one
    /// naming another surface's channel (the identity's cap provenance is
    /// not this channel's). The WM must refuse both in the reply's status
    /// and keep serving: the first emblem must keep turning through them,
    /// and the second answer's `OP_BUSY` must still be accepted.
    /// `DESKTOP arthur busy probe valid=S overflow=S wrong_channel=S`, each
    /// `S` the reply status (0 accepted), logged after every reply is in.
    #[cfg(feature = "arthur-busy-refusal-test")]
    fn probe_busy_refusals(
        &mut self,
        accepted: &Result<(), libneo::atrium_client::RequestError>,
        serial: u64,
        rect: (u16, u16, u16, u16),
    ) -> Result<(), i32> {
        use core::sync::atomic::{AtomicBool, Ordering};
        use libneo::atrium_client::RequestError;
        static PROBED: AtomicBool = AtomicBool::new(false);
        if PROBED.swap(true, Ordering::Relaxed) {
            return Ok(());
        }
        let status = |result: &Result<(), RequestError>| match *result {
            Ok(()) => Ok(0),
            Err(RequestError::Refused(status)) => Ok(status),
            Err(_) => Err(47),
        };
        let valid = status(accepted)?;
        let overflow_rect = (SURFACE_WIDTH as u16 - 1, rect.1, 2, rect.3);
        let overflow = status(&self.busy(self.identity, serial, overflow_rect))?;
        let mut other = self.identity;
        other.surface_cap_provenance ^= 1;
        let wrong_channel = status(&self.busy(other, serial, rect))?;
        let mut line = [0u8; 96];
        let mut n = 0;
        let mut put = |part: &[u8]| {
            line[n..n + part.len()].copy_from_slice(part);
            n += part.len();
        };
        for (label, value) in [
            (&b"DESKTOP arthur busy probe valid="[..], valid),
            (b" overflow=", overflow),
            (b" wrong_channel=", wrong_channel),
        ] {
            put(label);
            let mut digits = [0u8; 10];
            let mut d = 0;
            let mut v = value;
            loop {
                digits[d] = b'0' + (v % 10) as u8;
                d += 1;
                v /= 10;
                if v == 0 {
                    break;
                }
            }
            digits[..d].reverse();
            put(&digits[..d]);
        }
        put(b"\n");
        let _ = libneo::sys_write(1, line.as_ptr(), n);
        Ok(())
    }

    /// DS-1b: ask the broker to launch `module_id` through the LaunchCap
    /// `cap`: `token` rides in the request, `proof` on the cap (honest callers
    /// pass the same value). ADVISORY until OT-66 gates `sys_spawn`.
    #[cfg(feature = "desktop-home")]
    pub fn launch(
        &mut self,
        cap: Handle,
        module_id: u32,
        token: u64,
        proof: Option<u64>,
    ) -> Result<(), libneo::atrium_client::RequestError> {
        let request_id = self.next_request_id();
        libneo::atrium_client::launch(
            self.surface,
            cap,
            request_id,
            self.identity,
            module_id,
            token,
            proof,
        )
    }

    /// DI-1b resize: map the configure mailbox the WM grants on the session
    /// channel (the same offer / ready / grant / ack exchange as the
    /// surface), read-only: Arthur can read what the WM publishes and never
    /// write it.
    #[cfg(feature = "desktop-resize")]
    pub fn recv_configure_mailbox(&mut self, session: Handle) -> Result<(), i32> {
        use libneo::atrium_configure::{
            MAILBOX_BYTES, MAILBOX_HEIGHT, MAILBOX_STRIDE, MAILBOX_WIDTH, words,
        };
        let mut offer = [0u8; 1];
        if sys_recv(session, &mut offer) != 1 || offer[0] != b'O' || sys_send(session, b"R") != 1 {
            return Err(70);
        }
        let read_only = CapRights::new(meta_rights::USE, ATRIUM_MAP_READ | ATRIUM_QUERY);
        let mailbox = sys_recv_grant(session, read_only).map_err(|_| 71)?;
        if sys_send(session, b"A") != 1 {
            return Err(72);
        }
        let mut desc = [0u8; libneo::BUFFER_DESC_V1_LEN];
        let expected = encode_buffer_desc(
            MAILBOX_WIDTH,
            MAILBOX_HEIGHT,
            MAILBOX_STRIDE,
            FORMAT_XRGB8888_LE,
            MAILBOX_BYTES,
        );
        if libneo::sys_buffer_query(mailbox, &mut desc) != 0 || desc != expected {
            return Err(73);
        }
        let base = sys_buffer_map(mailbox, 1).map_err(|_| 74)?;
        // SAFETY: a read-only mapping of the validated 16-byte mailbox, page
        // aligned and never unmapped; the WM writes it, all accesses atomic.
        self.mailbox = Some(unsafe { words(base) });
        Ok(())
    }

    /// DI-1b resize: apply the WM's latest configure, if it published a new
    /// one. Size only (no position: nothing Arthur is sent ever carries one).
    /// Arthur lays its surface out at that size and replays its transcript,
    /// then acks with `OP_CONFIGURE` on its surface channel — "my buffer now
    /// holds this size's picture" — and the WM shows it before replying. A
    /// size outside [MIN, buffer] is a protocol violation (`Err`). Returns
    /// whether a configure was applied.
    #[cfg(feature = "desktop-resize")]
    pub fn poll_configure(&mut self) -> Result<bool, i32> {
        use crate::showcase_logic::{MIN_HEIGHT, MIN_WIDTH};
        use libneo::atrium_protocol::{ConfigureSurface, OP_CONFIGURE};
        let Some(published) = self.mailbox.and_then(libneo::atrium_configure::read) else {
            return Ok(false);
        };
        // A changed serial, not a larger one: serials wrap (#53 nit).
        if !libneo::atrium_configure::is_new(published, self.configure_serial) {
            return Ok(false);
        }
        let (w, h) = (published.width as usize, published.height as usize);
        if !(MIN_WIDTH..=BUFFER_WIDTH).contains(&w) || !(MIN_HEIGHT..=BUFFER_HEIGHT).contains(&h) {
            return Err(75);
        }
        self.text.resize(self.pixels, w, h);
        let _ = self.text.take_damage();
        (self.width, self.height) = (w, h);
        self.configure_serial = u64::from(published.serial);
        let damage = encode_damage_rect(0, 0, w as u16, h as u16);
        if sys_buffer_commit_damage(self.buffer, &damage) != 0 {
            return Err(76);
        }
        // Boot-test probe: an ack of another serial first, which the WM must
        // refuse with the configure still pending for the real ack below.
        #[cfg(feature = "arthur-refusal-probe-test")]
        {
            use core::sync::atomic::{AtomicBool, Ordering};
            static PROBED: AtomicBool = AtomicBool::new(false);
            if !PROBED.swap(true, Ordering::Relaxed) {
                let wrong = ConfigureSurface {
                    identity: self.identity,
                    width: published.width,
                    height: published.height,
                    configure_serial: self.configure_serial.wrapping_add(1).max(1),
                };
                let status = self.ask(Request::Configure(wrong), OP_CONFIGURE)?;
                let mut line = ProbeLine::new();
                line.put(b"DESKTOP arthur refusal probe configure_mismatch=");
                line.num(u64::from(status));
                line.put(b"\n");
                line.emit();
            }
        }
        let request_id = self.next_request_id();
        let ack = ConfigureSurface {
            identity: self.identity,
            width: published.width,
            height: published.height,
            configure_serial: self.configure_serial,
        };
        if !send_request(self.surface, request_id, Request::Configure(ack))
            || recv_reply(self.surface, OP_CONFIGURE, request_id)
                .is_none_or(|reply| reply.status != 0)
        {
            return Err(77);
        }
        log_configure(w, h, self.configure_serial);
        Ok(true)
    }

    /// Commit the footer's change alone (the transcript's damage waits for
    /// the next commit). Nothing to commit (the legacy grid has no footer):
    /// no commit.
    #[cfg(feature = "desktop-session")]
    pub fn commit_footer(&mut self) -> Result<(), i32> {
        #[cfg(not(feature = "arthur-legacy-5x7-font"))]
        if let Some(rect) = self.text.take_footer_damage() {
            let serial = self.client_serial.checked_add(1).ok_or(41)?;
            self.commit_rect(serial, rect)?;
        }
        Ok(())
    }

    fn commit_serial(&mut self, serial: u64) -> Result<u64, i32> {
        #[cfg(feature = "arthur-scrollback")]
        self.text.settle(self.pixels);
        let rect = self.damage();
        self.commit_rect(serial, rect)
    }

    fn commit_rect(&mut self, serial: u64, rect: (u16, u16, u16, u16)) -> Result<u64, i32> {
        if serial <= self.client_serial {
            return Err(43);
        }
        let (x, y, width, height) = rect;
        let damage = encode_damage_rect(x, y, width, height);
        if sys_buffer_commit_damage(self.buffer, &damage) != 0 {
            return Err(44);
        }
        let request_id = self.next_request_id();
        // S2-F2: the desktop WM copies Arthur's pixels instead of presenting
        // this buffer, so it is told the band the commit changed (the
        // standalone showcase presents the buffer and keeps `OP_COMMIT`).
        #[cfg(feature = "desktop-session")]
        let (request, opcode) = (
            Request::CommitDamage(libneo::atrium_protocol::CommitDamage {
                identity: self.identity,
                client_serial: serial,
                x,
                y,
                width,
                height,
            }),
            libneo::atrium_protocol::OP_COMMIT_DAMAGE,
        );
        #[cfg(not(feature = "desktop-session"))]
        let (request, opcode) = (
            Request::Commit(libneo::atrium_protocol::Commit {
                identity: self.identity,
                client_serial: serial,
            }),
            libneo::atrium_protocol::OP_COMMIT,
        );
        if !send_request(self.surface, request_id, request)
            || recv_reply(self.surface, opcode, request_id).is_none_or(|reply| reply.status != 0)
        {
            return Err(45);
        }
        self.client_serial = serial;
        Ok(serial)
    }

    /// Boot-test probe (desktop_resize_boot): one request the WM must refuse
    /// for each of its session loop's client-error arms, each answered in
    /// its reply's status while the session goes on. None of them changes
    /// Arthur's own state (its serial, its picture); the session after them
    /// is the scenario's ordinary one. Logs
    /// `DESKTOP arthur refusal probe commit_stale=S damage_outside=S
    /// destroy_wrong_channel=S yield_wrong_channel=S unexpected=S
    /// configure_unpublished=S truncated=S garbage=dropped`.
    #[cfg(feature = "arthur-refusal-probe-test")]
    pub fn probe_refusals(&mut self) -> Result<(), i32> {
        use libneo::atrium_protocol::{
            CommitDamage, ConfigureSurface, FOCUS_REASON_USER_ACTIVATION, OP_COMMIT_DAMAGE,
            OP_CONFIGURE, OP_YIELD_FOCUS, YieldFocus,
        };
        let identity = self.identity;
        let mut other = identity;
        other.surface_cap_provenance ^= 1;
        let serial = self.client_serial;
        let (w, h) = (self.width as u16, self.height as u16);
        // A commit of a serial already committed.
        let commit_stale = self.ask(
            Request::CommitDamage(CommitDamage {
                identity,
                client_serial: serial,
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }),
            OP_COMMIT_DAMAGE,
        )?;
        // Damage a pixel past the surface's right edge.
        let damage_outside = self.ask(
            Request::CommitDamage(CommitDamage {
                identity,
                client_serial: serial + 1,
                x: w - 1,
                y: 0,
                width: 2,
                height: 1,
            }),
            OP_COMMIT_DAMAGE,
        )?;
        let destroy_wrong_channel = self.ask(
            Request::Destroy(DestroySurface {
                identity: other,
                final_client_serial: serial,
            }),
            OP_DESTROY_SURFACE,
        )?;
        let yield_wrong_channel = self.ask(
            Request::Yield(YieldFocus {
                identity: other,
                yield_serial: serial.max(1),
            }),
            OP_YIELD_FOCUS,
        )?;
        // A request the protocol knows but the session loop does not take.
        let unexpected = self.ask(
            Request::Focus(FocusRequest {
                identity,
                reason: FOCUS_REASON_USER_ACTIVATION,
                request_serial: 1,
            }),
            OP_FOCUS_REQUEST,
        )?;
        // An ack before the WM has published any configure.
        let configure_unpublished = self.ask(
            Request::Configure(ConfigureSurface {
                identity,
                width: u32::from(w),
                height: u32::from(h),
                configure_serial: 1,
            }),
            OP_CONFIGURE,
        )?;
        // A whole header naming a commit, its payload cut short.
        let request_id = self.next_request_id();
        let (bytes, len) = encode_request(
            request_id,
            Request::CommitDamage(CommitDamage {
                identity,
                client_serial: serial + 1,
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }),
        );
        if sys_send(self.surface, &bytes[..len - 8]) != (len - 8) as i64 {
            return Err(47);
        }
        let truncated = recv_reply(self.surface, OP_COMMIT_DAMAGE, request_id)
            .ok_or(47)?
            .status;
        // Not even a header: nothing to reply to, so none is awaited (the
        // next request's reply shows the WM went on).
        if sys_send(self.surface, b"GARBAGE!") != 8 {
            return Err(47);
        }
        let mut line = ProbeLine::new();
        line.put(b"DESKTOP arthur refusal probe");
        for (label, status) in [
            (&b" commit_stale="[..], commit_stale),
            (b" damage_outside=", damage_outside),
            (b" destroy_wrong_channel=", destroy_wrong_channel),
            (b" yield_wrong_channel=", yield_wrong_channel),
            (b" unexpected=", unexpected),
            (b" configure_unpublished=", configure_unpublished),
            (b" truncated=", truncated),
        ] {
            line.put(label);
            line.num(u64::from(status));
        }
        line.put(b" garbage=dropped\n");
        line.emit();
        Ok(())
    }

    /// Send `request`, wait for its reply: the reply's status.
    #[cfg(feature = "arthur-refusal-probe-test")]
    fn ask(&mut self, request: Request, opcode: u16) -> Result<u32, i32> {
        let request_id = self.next_request_id();
        if !send_request(self.surface, request_id, request) {
            return Err(47);
        }
        recv_reply(self.surface, opcode, request_id)
            .map(|reply| reply.status)
            .ok_or(47)
    }

    fn next_request_id(&mut self) -> u64 {
        let request_id = self.request_id;
        self.request_id = self.request_id.saturating_add(1);
        request_id
    }

    /// Buffer-coordinate damage for the next commit. The legacy grid keeps its
    /// original full-surface damage; the text engine reports what it drew.
    fn damage(&mut self) -> (u16, u16, u16, u16) {
        let full = (0, 0, self.width as u16, self.height as u16);
        #[cfg(feature = "arthur-legacy-5x7-font")]
        {
            full
        }
        #[cfg(not(feature = "arthur-legacy-5x7-font"))]
        {
            self.text.take_damage().unwrap_or(full)
        }
    }
}

/// `DESKTOP arthur configure size=WxH serial=N`: Arthur laid itself out at
/// the size the WM published, and the WM accepted its ack.
#[cfg(feature = "desktop-resize")]
fn log_configure(w: usize, h: usize, serial: u64) {
    let mut line = [0u8; 80];
    let mut n = 0;
    let mut put = |part: &[u8]| {
        line[n..n + part.len()].copy_from_slice(part);
        n += part.len();
    };
    let num = |put: &mut dyn FnMut(&[u8]), mut v: u64| {
        let mut digits = [0u8; 20];
        let mut d = 0;
        loop {
            digits[d] = b'0' + (v % 10) as u8;
            d += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        digits[..d].reverse();
        put(&digits[..d]);
    };
    put(b"DESKTOP arthur configure size=");
    num(&mut put, w as u64);
    put(b"x");
    num(&mut put, h as u64);
    put(b" serial=");
    num(&mut put, serial);
    put(b"\n");
    let _ = libneo::sys_write(1, line.as_ptr(), n);
}

/// One boot-test log line from byte fragments and decimal numbers.
#[cfg(feature = "arthur-refusal-probe-test")]
struct ProbeLine {
    buf: [u8; 192],
    len: usize,
}

#[cfg(feature = "arthur-refusal-probe-test")]
impl ProbeLine {
    fn new() -> Self {
        Self {
            buf: [0; 192],
            len: 0,
        }
    }

    fn put(&mut self, part: &[u8]) {
        self.buf[self.len..self.len + part.len()].copy_from_slice(part);
        self.len += part.len();
    }

    fn num(&mut self, mut value: u64) {
        let mut digits = [0u8; 20];
        let mut d = 0;
        loop {
            digits[d] = b'0' + (value % 10) as u8;
            d += 1;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        digits[..d].reverse();
        self.put(&digits[..d]);
    }

    fn emit(&self) {
        let _ = libneo::sys_write(1, self.buf.as_ptr(), self.len);
    }
}

fn prove_client_scanout_denied() -> Result<(), i32> {
    let desc = encode_buffer_desc(1, 1, 4, FORMAT_XRGB8888_LE, 4);
    let attack = sys_buffer_create(&desc).map_err(|_| 47)?;
    let pixel = sys_buffer_map(attack, 0).map_err(|_| 48)? as *mut u32;
    // SAFETY: the validated 1x1 XRGB buffer maps exactly one u32 pixel RW.
    unsafe { core::ptr::write_volatile(pixel, BYPASS_POISON) };
    let damage = encode_damage_rect(0, 0, 1, 1);
    if sys_buffer_commit_damage(attack, &damage) != 0 {
        return Err(49);
    }
    if sys_fb_present(attack, BYPASS_DST.0, BYPASS_DST.1) != EPERM {
        return Err(50);
    }
    Ok(())
}

fn send_request(channel: Handle, request_id: u64, request: Request) -> bool {
    let (bytes, len) = encode_request(request_id, request);
    sys_send(channel, &bytes[..len]) == len as i64
}

fn recv_reply(channel: Handle, opcode: u16, request_id: u64) -> Option<Reply> {
    let mut bytes = [0u8; REPLY_LEN];
    if sys_recv(channel, &mut bytes) != REPLY_LEN as i64 {
        return None;
    }
    let reply = decode_reply(&bytes).ok()?;
    (reply.opcode == opcode && reply.request_id == request_id).then_some(reply)
}

/// DI-1a boot-test probe: the click the WM forwarded, as this client saw it
/// (kind 3: buttons, phase, surface-local coordinates).
#[cfg(feature = "desktop-pointer-probe")]
fn log_pointer_event(event: [u8; INPUT_EVENT_V1_LEN]) {
    if event[0] != libneo::INPUT_EVENT_POINTER_ABS {
        return;
    }
    let x = i16::from_le_bytes([event[4], event[5]]);
    let y = i16::from_le_bytes([event[6], event[7]]);
    let mut line = [0u8; 96];
    let mut n = 0;
    let mut put = |bytes: &[u8]| {
        line[n..n + bytes.len()].copy_from_slice(bytes);
        n += bytes.len();
    };
    put(b"DESKTOP arthur pointer buttons=");
    put(&[b'0' + (event[1] & 7)]);
    put(b" phase=");
    put(&[b'0' + (event[2] & 7)]);
    put(b" local=");
    let mut digits = [0u8; 6];
    for v in [x, y] {
        let mut v = v.max(0) as u32;
        let mut len = 0;
        loop {
            digits[len] = b'0' + (v % 10) as u8;
            len += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        for i in (0..len).rev() {
            put(&[digits[i]]);
        }
        put(b",");
    }
    n -= 1; // the trailing comma
    line[n] = b'\n';
    write_line(&line[..n + 1]);
}

#[cfg(feature = "desktop-pointer-probe")]
fn write_line(bytes: &[u8]) {
    let _ = libneo::sys_write(1, bytes.as_ptr(), bytes.len());
}

/// DI-1a boot-test probe: a client holds no input-system authority, so all
/// three DI-1 syscalls refuse it (EPERM), whatever the GPU-submit state.
#[cfg(feature = "desktop-pointer-probe")]
pub fn probe_input_system_authority() -> bool {
    let claim = libneo::sys_input_pointer_claim(false).err();
    let forward = libneo::sys_input_pointer_forward(libneo::Handle(0), 1, None, 0, 0);
    let sak = libneo::sys_input_sak_take();
    // A client's own focus, with an activator minted for it, still can't take
    // the keyboard route: activation is the input-system holder's alone. Had
    // it worked, the route would have left Arthur's real focus, and the
    // scenario's keys after this would never reach Arthur.
    let activate = match sys_input_focus_create() {
        Ok(focus) => {
            let r = match sys_input_focus_mint_activator(focus) {
                Ok(activator) => {
                    let r = libneo::sys_input_focus_activate(activator);
                    let _ = libneo::sys_revoke(activator);
                    r
                }
                Err(e) => e,
            };
            let _ = libneo::sys_revoke(focus);
            r
        }
        Err(e) => e,
    };
    let ok = claim == Some(EPERM) && forward == EPERM && sak == EPERM && activate == EPERM;
    write_line(if ok {
        b"DESKTOP arthur input_system claim=EPERM forward=EPERM sak=EPERM activate=EPERM\n"
    } else {
        b"DESKTOP arthur input_system NOT_REFUSED\n"
    });
    ok
}

/// OT-66 L2 boot test (`lock_session`, `arthur-lock-probe-test`): pictures
/// that the lock must keep off the screen. Each fills the whole surface with
/// one of [`LOCK_PROBE_COLORS`], commits it and asks for the busy emblem,
/// exactly as a client drawing and starting a generation would.
#[cfg(feature = "arthur-lock-probe-test")]
pub const LOCK_PROBE_COLORS: [u32; 3] = [0x00ff_00ff, 0x0000_ffff, 0x00ff_ff00];

#[cfg(feature = "arthur-lock-probe-test")]
impl ArthurSurface {
    /// The `n`-th picture: commit it, then `OP_BUSY`.
    pub fn lock_probe(&mut self, n: usize) -> Result<(), i32> {
        let busy = self.text.busy_rect();
        let color = LOCK_PROBE_COLORS[n % LOCK_PROBE_COLORS.len()];
        for y in 0..self.height {
            self.pixels[y * BUFFER_WIDTH..y * BUFFER_WIDTH + self.width].fill(color);
        }
        let serial = self.client_serial.checked_add(1).ok_or(41)?;
        let full = (0, 0, self.width as u16, self.height as u16);
        let serial = self.commit_rect(serial, full)?;
        let Some(rect) = busy else {
            return Err(48);
        };
        self.busy(self.identity, serial, rect).map_err(|_| 49)
    }

    /// The flood (AAG F2): one-byte messages on the surface channel, which
    /// the WM can't decode and drops without a reply, back to back until
    /// `until_ns`. The WM's channel is never empty meanwhile.
    pub fn lock_flood(&self, until_ns: u64) -> u64 {
        let mut sent = 0;
        while libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC).unwrap_or(u64::MAX) < until_ns {
            if libneo::sys_send(self.surface, &[0]) != 1 {
                break;
            }
            sent += 1;
        }
        sent
    }
}

/// OT-66 L2b boot test (`login_lock_park`, `arthur-lock-park-test`).
#[cfg(feature = "arthur-lock-park-test")]
impl ArthurSurface {
    /// Commit the current picture, then poll the keyboard for `hold_ns`
    /// before receiving the reply, so the WM stays parked in its send of it
    /// (CP-57). Every key received meanwhile goes to `on_key`.
    pub fn lock_park(&mut self, hold_ns: u64, on_key: fn(u8)) -> Result<(), i32> {
        let serial = self.client_serial.checked_add(1).ok_or(41)?;
        let (width, height) = (self.width as u16, self.height as u16);
        let damage = encode_damage_rect(0, 0, width, height);
        if sys_buffer_commit_damage(self.buffer, &damage) != 0 {
            return Err(44);
        }
        let request_id = self.next_request_id();
        let request = Request::CommitDamage(libneo::atrium_protocol::CommitDamage {
            identity: self.identity,
            client_serial: serial,
            x: 0,
            y: 0,
            width,
            height,
        });
        if !send_request(self.surface, request_id, request) {
            return Err(45);
        }
        let until = libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC)
            .unwrap_or(0)
            .saturating_add(hold_ns);
        while libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC).unwrap_or(u64::MAX) < until {
            if let Some(byte) = self.poll_ascii()? {
                on_key(byte);
            }
            libneo::sys_sched_yield();
        }
        let opcode = libneo::atrium_protocol::OP_COMMIT_DAMAGE;
        if recv_reply(self.surface, opcode, request_id).is_none_or(|reply| reply.status != 0) {
            return Err(45);
        }
        self.client_serial = serial;
        Ok(())
    }

    /// Until the WM exits (the boot test's end): one-byte messages it drops
    /// without a reply, so a send fails only once it is gone.
    pub fn until_wm_gone(&self) {
        while libneo::sys_send(self.surface, &[0]) == 1 {
            libneo::sys_sched_yield();
        }
    }
}

/// OT-88 S1 boot tests (`login_settings_store`, `arthur-setting-test`;
/// `login_settings_refused`, `arthur-setting-refused-test`).
#[cfg(any(
    feature = "arthur-setting-test",
    feature = "arthur-setting-refused-test"
))]
impl ArthurSurface {
    /// Ask the WM to set `key` to `value` (`OP_SETTING`); the reply's status
    /// (0: set; 7: not the settings app; 20: no such key; 21: a value outside
    /// the key's range; 23: a WM-only key; 24: the rate limit).
    pub fn setting(&mut self, key: u16, value: u32) -> Result<u32, i32> {
        let request_id = self.next_request_id();
        let request = Request::Setting(libneo::atrium_protocol::Setting {
            identity: self.identity,
            key,
            value,
        });
        if !send_request(self.surface, request_id, request) {
            return Err(45);
        }
        recv_reply(
            self.surface,
            libneo::atrium_protocol::OP_SETTING,
            request_id,
        )
        .map(|reply| reply.status)
        .ok_or(45)
    }
}

/// Boot-test probes (`desktop-actions-probe`, Dexter 213): the next
/// `OP_PERMISSION` waits this long between its header and its path (a
/// Super+Escape lands meanwhile), and the next request or done names another
/// surface (`FLIP_IDENTITY`).
#[cfg(feature = "desktop-actions-probe")]
pub static HOLD_PATH_NS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
#[cfg(feature = "desktop-actions-probe")]
pub static FLIP_IDENTITY: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// This request's identity: the surface's own, or (probe) another's.
#[cfg(feature = "desktop-actions")]
fn probe_identity(identity: Identity) -> Identity {
    #[cfg(feature = "desktop-actions-probe")]
    if FLIP_IDENTITY.swap(false, core::sync::atomic::Ordering::Relaxed) {
        let mut other = identity;
        other.surface_cap_provenance ^= 1;
        return other;
    }
    identity
}

/// L1.3a (`desktop-actions`): Arthur asks the WM's prompt for one action on
/// one path, and reports when it is done.
#[cfg(feature = "desktop-actions")]
impl ArthurSurface {
    /// `OP_PERMISSION` for `action` on `path`: the reply's status and, when
    /// it allows (`STATUS_PERMISSION_GRANTED` or `_ALWAYS`), the cap the WM
    /// lends after it (offer `O`, ready `R`, the grant, ack `A`). `Err`: the
    /// session channel failed.
    pub fn permission(
        &mut self,
        action: u16,
        path: &str,
        intent: &str,
        rights: libneo::CapRights,
    ) -> Result<(u32, Option<Handle>), i32> {
        use libneo::atrium_protocol::{
            OP_PERMISSION, PERMISSION_PATH_MAX, Permission, STATUS_PERMISSION_GRANTED,
            STATUS_PERMISSION_GRANTED_ALWAYS,
        };
        if path.is_empty() || path.len() > PERMISSION_PATH_MAX {
            return Err(46);
        }
        // L2.1: the line the action came from, for the WM's log (cut, never
        // refused: the log keeps what fits).
        let intent = &intent.as_bytes()[..intent.len().min(PERMISSION_PATH_MAX)];
        let request_id = self.next_request_id();
        let request = Request::Permission(Permission {
            identity: probe_identity(self.identity),
            action,
            path_len: path.len() as u16,
            intent_len: intent.len() as u16,
        });
        if !send_request(self.surface, request_id, request) {
            return Err(46);
        }
        #[cfg(feature = "desktop-actions-probe")]
        {
            let hold = HOLD_PATH_NS.swap(0, core::sync::atomic::Ordering::Relaxed);
            let now = || libneo::sys_clock_gettime(libneo::CLOCK_MONOTONIC).unwrap_or(u64::MAX);
            let until = now().saturating_add(hold);
            while hold != 0 && now() < until {
                libneo::sys_sched_yield();
            }
        }
        if sys_send(self.surface, path.as_bytes()) != path.len() as i64
            || (!intent.is_empty() && sys_send(self.surface, intent) != intent.len() as i64)
        {
            return Err(46);
        }
        let status = recv_reply(self.surface, OP_PERMISSION, request_id)
            .ok_or(46)?
            .status;
        if status != STATUS_PERMISSION_GRANTED && status != STATUS_PERMISSION_GRANTED_ALWAYS {
            return Ok((status, None));
        }
        let mut offer = [0u8; 1];
        if sys_recv(self.surface, &mut offer) != 1
            || offer[0] != b'O'
            || sys_send(self.surface, b"R") != 1
        {
            return Err(47);
        }
        let cap = libneo::sys_recv_grant(self.surface, rights).map_err(|_| 47)?;
        if sys_send(self.surface, b"A") != 1 {
            return Err(47);
        }
        Ok((status, Some(cap)))
    }

    /// Boot test (`desktop-actions-probe`): an `OP_PERMISSION` header for a
    /// 1-byte path that is never sent, or (`path_sent`) for a 1-byte path,
    /// sent, and a 1-byte intent that isn't (Dexter 213 item 1); the reply's
    /// status.
    #[cfg(feature = "desktop-actions-probe")]
    pub fn permission_stall(&mut self, path_sent: bool) -> Result<u32, i32> {
        use libneo::atrium_protocol::{OP_PERMISSION, PERMISSION_READ, Permission};
        let request_id = self.next_request_id();
        let request = Request::Permission(Permission {
            identity: self.identity,
            action: PERMISSION_READ,
            path_len: 1,
            intent_len: u16::from(path_sent),
        });
        if !send_request(self.surface, request_id, request)
            || (path_sent && sys_send(self.surface, b"/") != 1)
        {
            return Err(49);
        }
        recv_reply(self.surface, OP_PERMISSION, request_id)
            .map(|reply| reply.status)
            .ok_or(49)
    }

    /// `OP_PERMISSION_DONE` with `outcome` (0, or a failed action's errno
    /// magnitude).
    pub fn permission_done(&mut self, outcome: u32) -> Result<u32, i32> {
        use libneo::atrium_protocol::{OP_PERMISSION_DONE, PermissionDone};
        let request_id = self.next_request_id();
        let request = Request::PermissionDone(PermissionDone {
            identity: probe_identity(self.identity),
            outcome,
        });
        if !send_request(self.surface, request_id, request) {
            return Err(48);
        }
        recv_reply(self.surface, OP_PERMISSION_DONE, request_id)
            .map(|reply| reply.status)
            .ok_or(48)
    }
}
