// SPDX-License-Identifier: MPL-2.0
//! Pure i8042 / PS/2 keyboard logic: the controller's command set, the
//! configuration-byte transform, status decoding, and the scancode decoder.
//! No port I/O; host-tested through `tools/xtask/src/i8042_tests.rs`.
//!
//! **Scancode set.** The controller is left in its firmware-default
//! *translating* mode (config bit 6), and the decoder reads **set 1**. The
//! keyboard itself keeps sending set 2; the controller translates. This is
//! the choice that needs nothing from the keyboard: an EC-emulated keyboard
//! (the Zenbook's `ATK3001`) is not obliged to honour the `0xF0` set-select
//! command, and some ignore or garble it, while every PC firmware boots with
//! translation on and every BIOS-era OS has read set 1 through it. Turning
//! translation off and selecting set 2 would add a keyboard command whose
//! failure modes are vendor-specific, for no gain: set 1 through translation
//! covers every key a laptop keyboard has.
//!
//! Sources, read for the principle only (no code copied): the IBM PC/AT
//! technical reference and OSDev's "8042 PS/2 Controller" and "PS/2 Keyboard"
//! pages for the command set and scancode tables; the USB HID Usage Tables
//! (Keyboard/Keypad page 0x07) for the output codes; Linux's `i8042` and
//! `atkbd` drivers for the field traps
//! (`docs/RESEARCH-2026-11-02-i8042-atkbd-traps-FINDINGS.md`).

/// Data port (read: output buffer; write: input buffer to the device).
pub const DATA_PORT: u16 = 0x60;
/// Status register (read) / command register (write).
pub const STATUS_PORT: u16 = 0x64;

/// Status bit 0: output buffer full (a byte is waiting at 0x60).
pub const STATUS_OBF: u8 = 1 << 0;
/// Status bit 1: input buffer full (the controller has not taken our byte).
pub const STATUS_IBF: u8 = 1 << 1;
/// Status bit 5: the waiting byte came from the AUX (mouse) port.
pub const STATUS_AUX: u8 = 1 << 5;
/// Status bit 6: receive/transmit timeout from the device.
pub const STATUS_TIMEOUT: u8 = 1 << 6;
/// Status bit 7: parity error from the device.
pub const STATUS_PARITY: u8 = 1 << 7;

pub const CMD_READ_CONFIG: u8 = 0x20;
pub const CMD_WRITE_CONFIG: u8 = 0x60;
pub const CMD_DISABLE_PORT2: u8 = 0xA7;
pub const CMD_SELF_TEST: u8 = 0xAA;
pub const CMD_TEST_PORT1: u8 = 0xAB;
pub const CMD_DISABLE_PORT1: u8 = 0xAD;
pub const CMD_ENABLE_PORT1: u8 = 0xAE;

pub const SELF_TEST_OK: u8 = 0x55;
pub const PORT_TEST_OK: u8 = 0x00;

/// Keyboard commands and replies.
pub const KBD_ENABLE_SCANNING: u8 = 0xF4;
pub const KBD_ACK: u8 = 0xFA;
pub const KBD_RESEND: u8 = 0xFE;

/// Configuration byte bits.
pub const CONFIG_PORT1_IRQ: u8 = 1 << 0;
pub const CONFIG_PORT2_IRQ: u8 = 1 << 1;
pub const CONFIG_PORT1_CLOCK_OFF: u8 = 1 << 4;
pub const CONFIG_PORT2_CLOCK_OFF: u8 = 1 << 5;
pub const CONFIG_TRANSLATE: u8 = 1 << 6;

/// The configuration byte written once both ports are disabled: no
/// interrupts yet, port 1 clocked, translation on, port 2 left alone except
/// that its interrupt is off (a missing or unused AUX port must never raise
/// IRQ12). Bits 2 (system flag) and 3/7 (reserved) are kept as read.
pub const fn config_quiesced(read: u8) -> u8 {
    (read & !(CONFIG_PORT1_IRQ | CONFIG_PORT2_IRQ | CONFIG_PORT1_CLOCK_OFF)) | CONFIG_TRANSLATE
}

/// The configuration byte that turns on the keyboard interrupt (IRQ1).
pub const fn config_running(quiesced: u8) -> u8 {
    quiesced | CONFIG_PORT1_IRQ
}

/// What a byte read from the output buffer is, by the status read with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteSource {
    /// From the keyboard, clean.
    Keyboard,
    /// From the AUX port: not ours, discarded.
    Aux,
    /// A parity or timeout error: discarded.
    Error,
}

pub const fn classify(status: u8) -> ByteSource {
    if status & (STATUS_TIMEOUT | STATUS_PARITY) != 0 {
        ByteSource::Error
    } else if status & STATUS_AUX != 0 {
        ByteSource::Aux
    } else {
        ByteSource::Keyboard
    }
}

/// The ring's read index once the bytes received before the first keyboard
/// route are skipped. `tail`/`head` are the pump's free-running ring indices
/// and `cut` is `head` as it was when the route came up. A `cut` outside
/// `tail..=head` (wrapping) skips nothing.
///
/// Why: keys typed during the boot screens wait in the ring until the
/// desktop's first pump. Delivered then, they reached Home's intent field
/// (an Enter among them submitted it), and a break lost when a held key's
/// typematic filled the ring left the key, or a modifier, held. Those bytes
/// belong to no one; the pump drops them and starts the decoder afresh.
pub const fn tail_after_cut(tail: u32, head: u32, cut: u32) -> u32 {
    if cut.wrapping_sub(tail) <= head.wrapping_sub(tail) {
        cut
    } else {
        tail
    }
}

/// A decoded key transition, as a USB HID Keyboard/Keypad usage (page 0x07).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub usage: u8,
    pub pressed: bool,
}

/// Set-1 make code (no prefix) → HID usage. 0 = no key.
const SET1: [u8; 0x59] = [
    0x00, 0x29, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, // 00-07: -, Esc, 1..6
    0x24, 0x25, 0x26, 0x27, 0x2D, 0x2E, 0x2A, 0x2B, // 08-0F: 7..0, -, =, Bksp, Tab
    0x14, 0x1A, 0x08, 0x15, 0x17, 0x1C, 0x18, 0x0C, // 10-17: Q W E R T Y U I
    0x12, 0x13, 0x2F, 0x30, 0x28, 0xE0, 0x04, 0x16, // 18-1F: O P [ ] Enter LCtrl A S
    0x07, 0x09, 0x0A, 0x0B, 0x0D, 0x0E, 0x0F, 0x33, // 20-27: D F G H J K L ;
    0x34, 0x35, 0xE1, 0x31, 0x1D, 0x1B, 0x06, 0x19, // 28-2F: ' ` LShift \ Z X C V
    0x05, 0x11, 0x10, 0x36, 0x37, 0x38, 0xE5, 0x55, // 30-37: B N M , . / RShift KP*
    0xE2, 0x2C, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, // 38-3F: LAlt Space Caps F1..F5
    0x3F, 0x40, 0x41, 0x42, 0x43, 0x53, 0x47, 0x5F, // 40-47: F6..F10 NumLk ScrLk KP7
    0x60, 0x61, 0x56, 0x5C, 0x5D, 0x5E, 0x57, 0x59, // 48-4F: KP8 KP9 KP- KP4 KP5 KP6 KP+ KP1
    0x5A, 0x5B, 0x62, 0x63, 0x00, 0x00, 0x64, 0x44, // 50-57: KP2 KP3 KP0 KP. - - NonUS\ F11
    0x45, // 58: F12
];

/// Set-1 `E0`-prefixed make code → HID usage. 0 = no key.
const fn set1_extended(code: u8) -> u8 {
    match code {
        0x1C => 0x58, // keypad Enter
        0x1D => 0xE4, // right Ctrl
        0x35 => 0x54, // keypad /
        0x37 => 0x46, // Print Screen
        0x38 => 0xE6, // right Alt
        0x47 => 0x4A, // Home
        0x48 => 0x52, // Up
        0x49 => 0x4B, // Page Up
        0x4B => 0x50, // Left
        0x4D => 0x4F, // Right
        0x4F => 0x4D, // End
        0x50 => 0x51, // Down
        0x51 => 0x4E, // Page Down
        0x52 => 0x49, // Insert
        0x53 => 0x4C, // Delete
        0x5B => 0xE3, // left GUI
        0x5C => 0xE7, // right GUI
        0x5D => 0x65, // Application
        _ => 0,
    }
}

/// HID usage for Pause (set 1 sends it as the one sequence `E1 1D 45 E1 9D C5`).
pub const USAGE_PAUSE: u8 = 0x48;

/// Set-1 scancode decoder. Feed every keyboard byte in order; each call
/// returns at most one key transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decoder {
    /// An `E0` prefix is pending.
    extended: bool,
    /// Bytes of the six-byte `E1` Pause sequence still to swallow.
    pause_left: u8,
    /// Held usages (bit per usage), so the keyboard's own typematic repeat
    /// (a make code repeated while held) is reported once, as the USB HID
    /// boot report reports a held key.
    held: [u64; 4],
    /// Typematic makes dropped since the last [`Decoder::take_typematic`]
    /// (`key-autorepeat` makes the repeats in software instead).
    #[cfg(feature = "key-autorepeat")]
    typematic: u32,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            extended: false,
            pause_left: 0,
            held: [0; 4],
            #[cfg(feature = "key-autorepeat")]
            typematic: 0,
        }
    }

    fn is_held(&self, usage: u8) -> bool {
        self.held[(usage >> 6) as usize] & (1 << (usage & 63)) != 0
    }

    fn set_held(&mut self, usage: u8, held: bool) {
        let word = &mut self.held[(usage >> 6) as usize];
        if held {
            *word |= 1 << (usage & 63);
        } else {
            *word &= !(1 << (usage & 63));
        }
    }

    /// Typematic makes dropped since the last call.
    #[cfg(feature = "key-autorepeat")]
    pub fn take_typematic(&mut self) -> u32 {
        core::mem::take(&mut self.typematic)
    }

    /// Whether any key is held (the driver's state line).
    pub fn any_held(&self) -> bool {
        self.held.iter().any(|w| *w != 0)
    }

    pub fn feed(&mut self, byte: u8) -> Option<KeyEvent> {
        if self.pause_left > 0 {
            // `E1 1D 45` is the make half and `E1 9D C5` the break half; the
            // key has no repeat and no separate release, so report the press
            // at the second `E1` and the release at the last byte.
            self.pause_left -= 1;
            return match self.pause_left {
                2 => Some(KeyEvent {
                    usage: USAGE_PAUSE,
                    pressed: true,
                }),
                0 => Some(KeyEvent {
                    usage: USAGE_PAUSE,
                    pressed: false,
                }),
                _ => None,
            };
        }
        match byte {
            0xE0 => {
                self.extended = true;
                return None;
            }
            0xE1 => {
                self.extended = false;
                self.pause_left = 5;
                return None;
            }
            // Keyboard replies and errors that can arrive between keys: the
            // ACK and resend of a command, echo, and the overrun/error codes.
            // None is a key. (`0xAA`, the self-test pass after a reset, is
            // also left Shift's break code; it is not filtered here. The
            // driver never resets the keyboard, and a stray one is a break
            // for a key not held, which the held check drops.)
            0x00 | 0xEE | 0xFA | 0xFC | 0xFD | 0xFE | 0xFF => {
                self.extended = false;
                return None;
            }
            _ => {}
        }
        let extended = core::mem::replace(&mut self.extended, false);
        let pressed = byte & 0x80 == 0;
        let code = byte & 0x7F;
        let usage = if extended {
            // `E0 2A` / `E0 AA` / `E0 36` / `E0 B6` are the "fake shifts" the
            // keyboard wraps around the navigation keys; they are not keys.
            if code == 0x2A || code == 0x36 {
                return None;
            }
            set1_extended(code)
        } else {
            SET1.get(code as usize).copied().unwrap_or(0)
        };
        if usage == 0 {
            return None;
        }
        if pressed == self.is_held(usage) {
            // A repeat make while held, or a break for a key never pressed.
            #[cfg(feature = "key-autorepeat")]
            if pressed {
                self.typematic = self.typematic.wrapping_add(1);
            }
            return None;
        }
        self.set_held(usage, pressed);
        Some(KeyEvent { usage, pressed })
    }
}
