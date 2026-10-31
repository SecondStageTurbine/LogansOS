// SPDX-License-Identifier: MPL-2.0
//! Stable kernel-to-client input-event wire format.

pub const INPUT_EVENT_V1_LEN: usize = 8;
pub const INPUT_EVENT_KEY: u8 = 1;
pub const INPUT_EVENT_POINTER: u8 = 2;
pub const KEY_PRESSED: u8 = 1;

/// Two-layer input kill-switch decision, isolated for the differential KAT.
pub const fn routing_enabled(compiled_in: bool, runtime_on: bool) -> bool {
    compiled_in && runtime_on
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputEventV1 {
    pub kind: u8,
    pub flags: u8,
    pub code: u16,
    pub x: i16,
    pub y: i16,
}

impl InputEventV1 {
    pub const fn key(keycode: u8, modifiers: u8, pressed: bool) -> Self {
        Self {
            kind: INPUT_EVENT_KEY,
            flags: if pressed { KEY_PRESSED } else { 0 },
            code: keycode as u16,
            x: modifiers as i16,
            y: 0,
        }
    }

    #[allow(dead_code)] // host KATs; the kernel's drivers pass their wheel (A9-C2)
    pub const fn pointer(dx: i8, dy: i8, buttons: u8) -> Self {
        Self::pointer_wheel(dx, dy, buttons, 0)
    }

    /// A9-C2: a pointer report with its wheel. `code` carries the wheel as an
    /// i16 (LE on the wire; positive is away from the user, the HID
    /// convention); 0 is no wheel, which is what every pointer event carried
    /// before, so a report without one is byte-identical to [`Self::pointer`].
    pub const fn pointer_wheel(dx: i8, dy: i8, buttons: u8, wheel: i8) -> Self {
        Self {
            kind: INPUT_EVENT_POINTER,
            flags: buttons,
            code: wheel as i16 as u16,
            x: dx as i16,
            y: dy as i16,
        }
    }

    /// A9-C2: the wheel of a pointer event (`code` read as an i16).
    pub const fn wheel(&self) -> i16 {
        self.code as i16
    }
}

pub fn encode_event(event: InputEventV1) -> [u8; INPUT_EVENT_V1_LEN] {
    let mut out = [0; INPUT_EVENT_V1_LEN];
    out[0] = event.kind;
    out[1] = event.flags;
    out[2..4].copy_from_slice(&event.code.to_le_bytes());
    out[4..6].copy_from_slice(&event.x.to_le_bytes());
    out[6..8].copy_from_slice(&event.y.to_le_bytes());
    out
}

#[allow(dead_code)] // Host byte-oracle/user decoder; kernel is the encoder.
pub fn decode_event(bytes: [u8; INPUT_EVENT_V1_LEN]) -> InputEventV1 {
    InputEventV1 {
        kind: bytes[0],
        flags: bytes[1],
        code: u16::from_le_bytes([bytes[2], bytes[3]]),
        x: i16::from_le_bytes([bytes[4], bytes[5]]),
        y: i16::from_le_bytes([bytes[6], bytes[7]]),
    }
}
