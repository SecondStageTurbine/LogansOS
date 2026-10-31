// SPDX-License-Identifier: MPL-2.0
//! USB HID boot-protocol report decoding (keyboard + mouse).

pub const BOOT_REPORT_LEN: usize = 8;

// ── USB HID boot-protocol MOUSE (USB HID 1.11 §B.2) ────────────────────────────
//
// The boot mouse report is `buttons, dX, dY` (3 bytes); byte 3 = wheel is a
// near-universal extension (USB HID 1.11 §B.2 is the 3-byte core; most mice add a
// signed wheel byte). Deltas are i8 two's-complement. This decoder is the byte
// oracle's subject and hid_mouse.rs's decode core — no allocation, no kernel deps.

/// Minimum boot mouse report length: buttons, dX, dY.
pub const BOOT_MOUSE_REPORT_MIN_LEN: usize = 3;
/// Maximum boot mouse report length we decode: + a signed wheel byte.
pub const BOOT_MOUSE_REPORT_MAX_LEN: usize = 4;

/// Boot mouse button bitmap (report byte 0). Bit 0 = left/primary, bit 1 =
/// right/secondary, bit 2 = middle.
#[allow(dead_code)] // consumed by hid_mouse.rs (usb-hid-mouse) + the host byte-oracle
pub const MOUSE_BTN_LEFT: u8 = 1 << 0;
#[allow(dead_code)] // see MOUSE_BTN_LEFT
pub const MOUSE_BTN_RIGHT: u8 = 1 << 1;
#[allow(dead_code)] // see MOUSE_BTN_LEFT
pub const MOUSE_BTN_MIDDLE: u8 = 1 << 2;

/// One decoded pointer event: the current button bitmap plus relative motion
/// (and wheel) since the previous report. Deltas are signed. This is the event
/// shape the input queue delivers; the OT-52 compositor accumulates the deltas
/// and tracks button transitions (surface-routing is OT-52's job, not this
/// driver's). `#[repr(C)]` so the shape is a stable, inspectable layout for the
/// compositor consumer.
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
#[allow(dead_code)] // consumed by hid_mouse.rs (feature usb-hid-mouse) + the host byte-oracle
pub struct PointerEvent {
    pub buttons: u8,
    pub dx: i8,
    pub dy: i8,
    pub wheel: i8,
}

/// Boot-protocol mouse report decoder. Stateful only in the previous button
/// bitmap, so it can suppress idle/no-change reports (the all-zero-movement trap:
/// a report identical in buttons with zero motion/wheel is not an event) while
/// still emitting a pure button *release* (buttons changed, zero motion).
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
#[allow(dead_code)] // see PointerEvent
pub struct BootMouseDecoder {
    previous_buttons: u8,
}

#[allow(dead_code)] // see PointerEvent
impl BootMouseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode one boot-protocol mouse report. `bytes` must be at least
    /// [`BOOT_MOUSE_REPORT_MIN_LEN`] (buttons, dX, dY); byte 3, when present, is
    /// the signed wheel — a shorter report decodes wheel as 0, a longer report
    /// ignores the tail (defensive against the variable-length trap; never
    /// hard-assume 3). Deltas are i8 two's-complement (the signed-delta trap).
    /// Returns `Some(event)` only when the report carries a change — motion,
    /// wheel, or a button transition — and `None` for an idle/repeat report, so
    /// a spamming or idle mouse produces no spurious events.
    pub fn decode_report(&mut self, bytes: &[u8]) -> Option<PointerEvent> {
        if bytes.len() < BOOT_MOUSE_REPORT_MIN_LEN {
            return None;
        }
        let buttons = bytes[0];
        let dx = bytes[1] as i8;
        let dy = bytes[2] as i8;
        let wheel = if bytes.len() >= BOOT_MOUSE_REPORT_MAX_LEN {
            bytes[3] as i8
        } else {
            0
        };
        let changed = dx != 0 || dy != 0 || wheel != 0 || buttons != self.previous_buttons;
        self.previous_buttons = buttons;
        if changed {
            Some(PointerEvent {
                buttons,
                dx,
                dy,
                wheel,
            })
        } else {
            None
        }
    }
}

const MOD_LEFT_SHIFT: u8 = 1 << 1;
const MOD_RIGHT_SHIFT: u8 = 1 << 5;

#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct BootReport {
    modifiers: u8,
    keys: [u8; 6],
}

impl BootReport {
    pub fn from_bytes(bytes: &[u8; BOOT_REPORT_LEN]) -> Self {
        Self {
            modifiers: bytes[0],
            keys: [bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]],
        }
    }

    fn shifted(self) -> bool {
        self.modifiers & (MOD_LEFT_SHIFT | MOD_RIGHT_SHIFT) != 0
    }

    fn contains(self, keycode: u8) -> bool {
        keycode != 0 && self.keys.contains(&keycode)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct BootKeyboardDecoder {
    previous: BootReport,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[allow(dead_code)] // Live under atrium-input-routing; host KAT covers lean builds.
pub struct KeyEvent {
    pub keycode: u8,
    pub modifiers: u8,
    pub pressed: bool,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
#[allow(dead_code)] // Live under atrium-input-routing; host KAT covers lean builds.
pub struct BootKeyEventDecoder {
    previous: BootReport,
}

#[allow(dead_code)] // See BootKeyEventDecoder.
impl BootKeyEventDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn decode_report(&mut self, bytes: &[u8; BOOT_REPORT_LEN], out: &mut [KeyEvent]) -> usize {
        let report = BootReport::from_bytes(bytes);
        let mut written = 0;
        for &keycode in &self.previous.keys {
            if keycode != 0 && !report.contains(keycode) && written < out.len() {
                out[written] = KeyEvent {
                    keycode,
                    modifiers: report.modifiers,
                    pressed: false,
                };
                written += 1;
            }
        }
        for &keycode in &report.keys {
            if keycode != 0 && !self.previous.contains(keycode) && written < out.len() {
                out[written] = KeyEvent {
                    keycode,
                    modifiers: report.modifiers,
                    pressed: true,
                };
                written += 1;
            }
        }
        self.previous = report;
        written
    }
}

impl BootKeyboardDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn decode_report(&mut self, bytes: &[u8; BOOT_REPORT_LEN], out: &mut [u8]) -> usize {
        let report = BootReport::from_bytes(bytes);
        let mut written = 0;
        for &keycode in &report.keys {
            if keycode == 0 || self.previous.contains(keycode) {
                continue;
            }
            let Some(byte) = keycode_to_ascii(keycode, report.shifted()) else {
                continue;
            };
            if written >= out.len() {
                break;
            }
            out[written] = byte;
            written += 1;
        }
        self.previous = report;
        written
    }
}

pub fn keycode_to_ascii(keycode: u8, shifted: bool) -> Option<u8> {
    match keycode {
        0x04..=0x1d => {
            let base = b'a' + (keycode - 0x04);
            Some(if shifted { base - 32 } else { base })
        }
        0x1e..=0x26 => {
            let unshifted = b'1' + (keycode - 0x1e);
            let shifted_digits = b"!@#$%^&*(";
            Some(if shifted {
                shifted_digits[(keycode - 0x1e) as usize]
            } else {
                unshifted
            })
        }
        0x27 => Some(if shifted { b')' } else { b'0' }),
        0x28 => Some(b'\n'),
        0x2a => Some(0x08),
        0x2b => Some(b'\t'),
        0x2c => Some(b' '),
        0x2d => Some(if shifted { b'_' } else { b'-' }),
        0x2e => Some(if shifted { b'+' } else { b'=' }),
        0x2f => Some(if shifted { b'{' } else { b'[' }),
        0x30 => Some(if shifted { b'}' } else { b']' }),
        0x31 => Some(if shifted { b'|' } else { b'\\' }),
        0x33 => Some(if shifted { b':' } else { b';' }),
        0x34 => Some(if shifted { b'"' } else { b'\'' }),
        0x35 => Some(if shifted { b'~' } else { b'`' }),
        0x36 => Some(if shifted { b'<' } else { b',' }),
        0x37 => Some(if shifted { b'>' } else { b'.' }),
        0x38 => Some(if shifted { b'?' } else { b'/' }),
        _ => None,
    }
}
