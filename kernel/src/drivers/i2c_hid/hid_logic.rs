// SPDX-License-Identifier: MPL-2.0
//! HID over I2C (Microsoft "HID over I2C Protocol Specification" v1.0) and
//! the report-descriptor walk that finds the mouse collection. Pure: no I/O.
//! Host-tested by `tools/i2c-hid-logic-tests`.
//!
//! **The HID descriptor** (spec §5.1.1) is 30 bytes, little-endian, read from
//! the descriptor register: `wHIDDescLength` (=30) @0, `bcdVersion`
//! (=0x0100) @2, `wReportDescLength` @4, `wReportDescRegister` @6,
//! `wInputRegister` @8, `wMaxInputLength` @10, `wOutputRegister` @12,
//! `wMaxOutputLength` @14, `wCommandRegister` @16, `wDataRegister` @18,
//! `wVendorID` @20, `wProductID` @22, `wVersionID` @24, reserved @26..30.
//!
//! **Commands** (§7.2) are written to the command register: the register's
//! two bytes, then the command's low byte (report type and ID, or the power
//! state for SET_POWER) and high byte (the opcode: RESET 1, SET_POWER 8).
//!
//! **Input** (§6.1.1): a read of the input register returns a 2-byte
//! little-endian length that counts itself, then the report (its ID first
//! when the report descriptor declares IDs). Length 0 is the reset-complete
//! (or "nothing to send") answer.
//!
//! **Report descriptor** (HID 1.11 §6.2.2): short items only (long items are
//! skipped). The walk keeps the global state (usage page, logical minimum,
//! report size/count/ID; push/pop not needed by mouse descriptors and
//! treated as unsupported), the local usages, and the collection stack. The
//! mouse is the Application collection Generic Desktop / Mouse (0x01/0x02);
//! within it the Button-page Input field is the buttons, Generic Desktop X,
//! Y and Wheel (0x30/0x31/0x38) are the axes.

pub const HID_DESC_LEN: usize = 30;
pub const HID_VERSION_1_0: u16 = 0x0100;

pub const OPCODE_RESET: u8 = 0x01;
pub const OPCODE_SET_POWER: u8 = 0x08;
pub const POWER_ON: u8 = 0x00;
pub const POWER_SLEEP: u8 = 0x01;

/// The ELAN vendor ID. Linux's i2c-hid notes that ELAN controllers break if
/// SET_POWER ON follows a RESET and that they may answer an interrupt with a
/// length of 0xFFFF; both are handled for this vendor (trap 7).
pub const VENDOR_ELAN: u16 = 0x04F3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HidDescriptor {
    pub length: u16,
    pub version: u16,
    pub report_desc_length: u16,
    pub report_desc_register: u16,
    pub input_register: u16,
    pub max_input_length: u16,
    pub output_register: u16,
    pub max_output_length: u16,
    pub command_register: u16,
    pub data_register: u16,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version_id: u16,
}

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

impl HidDescriptor {
    /// Decode the 30 bytes, whatever they hold (see [`Self::validate`]).
    pub fn decode(b: &[u8; HID_DESC_LEN]) -> Self {
        Self {
            length: le16(b, 0),
            version: le16(b, 2),
            report_desc_length: le16(b, 4),
            report_desc_register: le16(b, 6),
            input_register: le16(b, 8),
            max_input_length: le16(b, 10),
            output_register: le16(b, 12),
            max_output_length: le16(b, 14),
            command_register: le16(b, 16),
            data_register: le16(b, 18),
            vendor_id: le16(b, 20),
            product_id: le16(b, 22),
            version_id: le16(b, 24),
        }
    }

    /// The checks rung 1 relies on: the length and version the spec fixes,
    /// a report descriptor to read, and an input length that can hold the
    /// length prefix plus a report.
    pub fn validate(&self) -> Result<(), DescriptorError> {
        if self.length as usize != HID_DESC_LEN {
            return Err(DescriptorError::Length(self.length));
        }
        if self.version != HID_VERSION_1_0 {
            return Err(DescriptorError::Version(self.version));
        }
        if self.report_desc_length == 0 || self.report_desc_length as usize > MAX_REPORT_DESC {
            return Err(DescriptorError::ReportDescLength(self.report_desc_length));
        }
        if !(3..=MAX_INPUT as u16).contains(&self.max_input_length) {
            return Err(DescriptorError::MaxInput(self.max_input_length));
        }
        Ok(())
    }
}

/// Largest report descriptor read (a precision touchpad's is under 1 KiB).
pub const MAX_REPORT_DESC: usize = 2048;
/// Largest input read (the length prefix plus a report).
pub const MAX_INPUT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorError {
    Length(u16),
    Version(u16),
    ReportDescLength(u16),
    MaxInput(u16),
}

/// The register-address prefix of every read and command.
pub const fn register(reg: u16) -> [u8; 2] {
    reg.to_le_bytes()
}

/// RESET: `[cmd lo, cmd hi, 0x00, 0x01]`.
pub const fn reset_command(command_register: u16) -> [u8; 4] {
    let r = command_register.to_le_bytes();
    [r[0], r[1], 0x00, OPCODE_RESET]
}

/// SET_POWER: `[cmd lo, cmd hi, state, 0x08]`.
pub const fn set_power_command(command_register: u16, on: bool) -> [u8; 4] {
    let r = command_register.to_le_bytes();
    [
        r[0],
        r[1],
        if on { POWER_ON } else { POWER_SLEEP },
        OPCODE_SET_POWER,
    ]
}

/// What one input read held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input<'a> {
    /// Length 0: reset complete, or nothing to send.
    Empty,
    /// Length 0xFFFF: an interrupt with no data (ELAN's bogus IRQ).
    Bogus,
    /// A length shorter than its own prefix or longer than the read.
    BadLength(u16),
    /// The report bytes after the length prefix (ID first, if IDs are used).
    Report(&'a [u8]),
}

pub fn parse_input(buf: &[u8]) -> Input<'_> {
    if buf.len() < 2 {
        return Input::BadLength(0);
    }
    let len = le16(buf, 0);
    match len {
        0 => Input::Empty,
        0xFFFF => Input::Bogus,
        n if (n as usize) < 2 || n as usize > buf.len() => Input::BadLength(n),
        n => Input::Report(&buf[2..n as usize]),
    }
}

// --- Report descriptor ---------------------------------------------------------

pub const PAGE_GENERIC_DESKTOP: u16 = 0x01;
pub const PAGE_BUTTON: u16 = 0x09;
pub const PAGE_DIGITIZER: u16 = 0x0D;
pub const USAGE_MOUSE: u16 = 0x02;
pub const USAGE_POINTER: u16 = 0x01;
pub const USAGE_X: u16 = 0x30;
pub const USAGE_Y: u16 = 0x31;
pub const USAGE_WHEEL: u16 = 0x38;
pub const USAGE_TOUCH_PAD: u16 = 0x05;

/// One field of a report: its bit position (after the report ID byte),
/// width, and whether its logical range is signed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Field {
    pub bit: u32,
    pub size: u32,
    pub signed: bool,
}

/// The mouse collection's input report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseLayout {
    /// 0 when the descriptor declares no report IDs.
    pub report_id: u8,
    /// One bit each, `count` buttons from `bit`.
    pub buttons: Field,
    pub button_count: u32,
    pub x: Field,
    pub y: Field,
    pub wheel: Option<Field>,
    /// The report's payload size in bits (excluding the ID byte).
    pub report_bits: u32,
}

/// What a report ID is, for counting unexpected reports (trap 10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReportKind {
    #[default]
    Other,
    Mouse,
    /// A Digitizer-page Application collection (the precision touchpad).
    Digitizer,
}

pub const MAX_REPORT_IDS: usize = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReportMap {
    pub uses_ids: bool,
    pub mouse: Option<MouseLayout>,
    /// Input report IDs seen, with their collection's kind.
    pub ids: [(u8, ReportKind); MAX_REPORT_IDS],
    pub id_count: usize,
    pub items: u32,
}

impl ReportMap {
    pub fn kind_of(&self, id: u8) -> Option<ReportKind> {
        self.ids[..self.id_count]
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, k)| *k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// A long item or Push/Pop: not needed for rung 1, refused rather than
    /// guessed.
    Unsupported(u8),
    Truncated,
    /// End Collection without a collection, or collections nested too deep.
    Collections,
    /// No Generic Desktop / Mouse collection with X and Y.
    NoMouse,
}

const MAX_USAGES: usize = 16;
const MAX_DEPTH: usize = 8;

#[derive(Clone, Copy, Default)]
struct Globals {
    page: u16,
    logical_min: i32,
    size: u32,
    count: u32,
    id: u8,
}

#[derive(Clone, Copy, Default)]
struct Locals {
    usages: [(u16, u16); MAX_USAGES],
    n: usize,
    min: Option<(u16, u16)>,
    max: Option<(u16, u16)>,
}

impl Locals {
    fn push(&mut self, page: u16, usage: u32, size: usize) {
        let (p, u) = if size == 4 {
            ((usage >> 16) as u16, usage as u16)
        } else {
            (page, usage as u16)
        };
        if self.n < MAX_USAGES {
            self.usages[self.n] = (p, u);
            self.n += 1;
        }
    }

    /// The usage for the `i`th field of an Input item: the explicit list
    /// (the last one repeats), else the min..max range.
    fn usage(&self, i: u32) -> Option<(u16, u16)> {
        if self.n > 0 {
            return Some(self.usages[(i as usize).min(self.n - 1)]);
        }
        match (self.min, self.max) {
            (Some((p, lo)), Some((_, hi))) => {
                let u = lo as u32 + i;
                (u <= hi as u32).then_some((p, u as u16))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Collection {
    kind: ReportKind,
}

/// Per report ID, bits laid out so far (the next field's position).
#[derive(Clone, Copy, Default)]
struct Offsets {
    ids: [u8; MAX_REPORT_IDS],
    bits: [u32; MAX_REPORT_IDS],
    n: usize,
}

impl Offsets {
    fn slot(&mut self, id: u8) -> Option<&mut u32> {
        let i = match self.ids[..self.n].iter().position(|&x| x == id) {
            Some(i) => i,
            None if self.n < MAX_REPORT_IDS => {
                self.ids[self.n] = id;
                self.bits[self.n] = 0;
                self.n += 1;
                self.n - 1
            }
            None => return None,
        };
        Some(&mut self.bits[i])
    }
}

fn item_data(d: &[u8], size: usize) -> (u32, i32) {
    match size {
        0 => (0, 0),
        1 => (d[0] as u32, d[0] as i8 as i32),
        2 => {
            let v = u16::from_le_bytes([d[0], d[1]]);
            (v as u32, v as i16 as i32)
        }
        _ => {
            let v = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
            (v, v as i32)
        }
    }
}

/// Walk `desc` and find the mouse input report.
pub fn parse_report_descriptor(desc: &[u8]) -> Result<ReportMap, ParseError> {
    let mut map = ReportMap::default();
    let mut g = Globals::default();
    let mut l = Locals::default();
    let mut stack = [Collection::default(); MAX_DEPTH];
    let mut depth = 0usize;
    let mut offsets = Offsets::default();
    // The mouse being assembled (within the current mouse collection).
    let mut building: Option<MouseLayout> = None;
    let mut have_x = false;
    let mut have_y = false;
    let mut have_buttons = false;

    let mut i = 0usize;
    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xFE {
            return Err(ParseError::Unsupported(prefix));
        }
        let size = match prefix & 0x3 {
            3 => 4,
            s => s as usize,
        };
        if i + 1 + size > desc.len() {
            return Err(ParseError::Truncated);
        }
        let (udata, sdata) = item_data(&desc[i + 1..], size);
        let tag = prefix & 0xFC;
        map.items += 1;
        i += 1 + size;
        match tag {
            // Main items.
            0x80 => {
                // Input. Bit 0: constant (padding).
                let constant = udata & 1 != 0;
                let id = g.id;
                let in_mouse =
                    depth > 0 && stack[..depth].iter().any(|c| c.kind == ReportKind::Mouse);
                let kind = stack[..depth]
                    .iter()
                    .map(|c| c.kind)
                    .find(|k| *k != ReportKind::Other)
                    .unwrap_or_default();
                note_id(&mut map, id, kind);
                let Some(base) = offsets.slot(id) else {
                    l = Locals::default();
                    continue;
                };
                let start = *base;
                *base += g.size * g.count;
                if !constant && in_mouse {
                    let m = building.get_or_insert(MouseLayout {
                        report_id: id,
                        ..MouseLayout::default()
                    });
                    if m.report_id == id {
                        for f in 0..g.count {
                            let Some((page, usage)) = l.usage(f) else {
                                continue;
                            };
                            let field = Field {
                                bit: start + f * g.size,
                                size: g.size,
                                signed: g.logical_min < 0,
                            };
                            match (page, usage) {
                                (PAGE_BUTTON, _) if !have_buttons => {
                                    m.buttons = field;
                                    m.button_count = g.count.min(8);
                                    have_buttons = true;
                                }
                                (PAGE_GENERIC_DESKTOP, USAGE_X) => {
                                    m.x = field;
                                    have_x = true;
                                }
                                (PAGE_GENERIC_DESKTOP, USAGE_Y) => {
                                    m.y = field;
                                    have_y = true;
                                }
                                (PAGE_GENERIC_DESKTOP, USAGE_WHEEL) => m.wheel = Some(field),
                                _ => {}
                            }
                        }
                    }
                }
                l = Locals::default();
            }
            0x90 | 0xB0 => l = Locals::default(), // Output, Feature
            0xA0 => {
                // Collection. Application (1) collections carry the kind.
                if depth == MAX_DEPTH {
                    return Err(ParseError::Collections);
                }
                let first = l.usage(0);
                let kind = match (udata, first) {
                    (1, Some((PAGE_GENERIC_DESKTOP, USAGE_MOUSE))) => ReportKind::Mouse,
                    (1, Some((PAGE_DIGITIZER, _))) => ReportKind::Digitizer,
                    _ => ReportKind::Other,
                };
                stack[depth] = Collection { kind };
                depth += 1;
                l = Locals::default();
            }
            0xC0 => {
                if depth == 0 {
                    return Err(ParseError::Collections);
                }
                depth -= 1;
                if stack[depth].kind == ReportKind::Mouse
                    && map.mouse.is_none()
                    && have_x
                    && have_y
                    && let Some(mut m) = building
                {
                    let id = m.report_id;
                    m.report_bits = offsets.slot(id).map(|b| *b).unwrap_or(0);
                    map.mouse = Some(m);
                }
                if stack[depth].kind == ReportKind::Mouse && map.mouse.is_none() {
                    building = None;
                    have_x = false;
                    have_y = false;
                    have_buttons = false;
                }
                l = Locals::default();
            }
            // Global items.
            0x04 => g.page = udata as u16,
            0x14 => g.logical_min = sdata,
            0x24 | 0x34 | 0x44 | 0x54 | 0x64 => {} // logical max, physical, unit
            0x74 => g.size = udata,
            0x84 => {
                g.id = udata as u8;
                map.uses_ids = true;
            }
            0x94 => g.count = udata,
            0xA4 | 0xB4 => return Err(ParseError::Unsupported(prefix)), // Push, Pop
            // Local items.
            0x08 => l.push(g.page, udata, size),
            0x18 => l.min = Some(local_usage(g.page, udata, size)),
            0x28 => l.max = Some(local_usage(g.page, udata, size)),
            _ => {}
        }
    }
    if depth != 0 {
        return Err(ParseError::Collections);
    }
    // A report ID's mouse fields stay where they were laid out even if the
    // collection holding them ended later than the last mouse item.
    if map.mouse.is_none() {
        return Err(ParseError::NoMouse);
    }
    Ok(map)
}

fn local_usage(page: u16, usage: u32, size: usize) -> (u16, u16) {
    if size == 4 {
        ((usage >> 16) as u16, usage as u16)
    } else {
        (page, usage as u16)
    }
}

fn note_id(map: &mut ReportMap, id: u8, kind: ReportKind) {
    if let Some(slot) = map.ids[..map.id_count].iter_mut().find(|(i, _)| *i == id) {
        if slot.1 == ReportKind::Other {
            slot.1 = kind;
        }
        return;
    }
    if map.id_count < MAX_REPORT_IDS {
        map.ids[map.id_count] = (id, kind);
        map.id_count += 1;
    }
}

/// `field` read from `payload` (little-endian bit order, HID 1.11 §5.8).
pub fn extract(payload: &[u8], field: Field) -> Option<i32> {
    if field.size == 0 || field.size > 32 {
        return None;
    }
    let end = field.bit + field.size;
    if end.div_ceil(8) as usize > payload.len() {
        return None;
    }
    let mut v: u64 = 0;
    for k in 0..field.size {
        let bit = field.bit + k;
        if payload[(bit / 8) as usize] >> (bit % 8) & 1 != 0 {
            v |= 1 << k;
        }
    }
    if field.signed && field.size < 32 && v & (1 << (field.size - 1)) != 0 {
        v |= !0u64 << field.size;
    }
    Some(v as i64 as i32)
}

/// One decoded mouse report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseReport {
    pub buttons: u8,
    pub dx: i8,
    pub dy: i8,
    pub wheel: i8,
}

fn clamp_i8(v: i32) -> i8 {
    v.clamp(-127, 127) as i8
}

/// What one report was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decoded {
    Mouse(MouseReport),
    /// A report ID the map knows, of another kind (a PTP report while the
    /// device should be in mouse mode).
    Unexpected(u8, ReportKind),
    /// An ID the descriptor never declared.
    UnknownId(u8),
    /// Shorter than the mouse report.
    Short,
}

/// Decode `report` (the bytes after the length prefix).
pub fn decode(map: &ReportMap, report: &[u8]) -> Decoded {
    let Some(mouse) = map.mouse else {
        return Decoded::Short;
    };
    let payload = if map.uses_ids {
        let Some((&id, rest)) = report.split_first() else {
            return Decoded::Short;
        };
        if id != mouse.report_id {
            return match map.kind_of(id) {
                Some(kind) => Decoded::Unexpected(id, kind),
                None => Decoded::UnknownId(id),
            };
        }
        rest
    } else {
        report
    };
    let (Some(x), Some(y)) = (extract(payload, mouse.x), extract(payload, mouse.y)) else {
        return Decoded::Short;
    };
    let mut buttons = 0u8;
    for b in 0..mouse.button_count {
        let f = Field {
            bit: mouse.buttons.bit + b * mouse.buttons.size,
            size: 1,
            signed: false,
        };
        if extract(payload, f) == Some(1) {
            buttons |= 1 << b;
        }
    }
    let wheel = mouse.wheel.and_then(|w| extract(payload, w)).unwrap_or(0);
    Decoded::Mouse(MouseReport {
        buttons,
        dx: clamp_i8(x),
        dy: clamp_i8(y),
        wheel: clamp_i8(wheel),
    })
}

// --- The service thread's line policy -----------------------------------------

/// Assertions in a row that held no data before the line stays masked.
pub const STORM_LIMIT: u32 = 16;
/// Transfer errors in a row before the thread stops reading.
pub const ERROR_LIMIT: u32 = 8;

/// What one read after an assertion (or a poll) found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A report.
    Data,
    /// Length 0, 0xFFFF or a bad length: the line asserted for nothing.
    NoData,
    /// The transfer failed.
    Error,
}

/// What the thread does with the line next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Unmask (IRQ mode) or keep polling.
    Continue,
    /// [`STORM_LIMIT`] assertions in a row without data: leave the line
    /// masked and poll (I-14(b): a stuck-asserted level line must not hold
    /// the CPU).
    MaskAndPoll,
    /// [`ERROR_LIMIT`] failed transfers in a row: stop reading.
    Stop,
}

/// Consecutive no-data and error counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineGovernor {
    pub spurious: u32,
    pub errors: u32,
}

impl LineGovernor {
    /// After a read in IRQ mode.
    pub fn after_irq(&mut self, outcome: Outcome) -> Action {
        match outcome {
            Outcome::Data => {
                self.spurious = 0;
                self.errors = 0;
            }
            Outcome::NoData => {
                self.spurious += 1;
                self.errors = 0;
            }
            Outcome::Error => self.errors += 1,
        }
        if self.errors >= ERROR_LIMIT {
            Action::Stop
        } else if self.spurious >= STORM_LIMIT {
            Action::MaskAndPoll
        } else {
            Action::Continue
        }
    }

    /// After a read in polled mode (no data is normal there).
    pub fn after_poll(&mut self, outcome: Outcome) -> Action {
        if outcome == Outcome::Error {
            self.errors += 1;
        } else {
            self.errors = 0;
        }
        if self.errors >= ERROR_LIMIT {
            Action::Stop
        } else {
            Action::Continue
        }
    }
}
