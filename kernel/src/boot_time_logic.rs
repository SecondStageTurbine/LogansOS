// SPDX-License-Identifier: MPL-2.0
//! L7.1: the boot-time numbers. Pure: the FPDT and FBPT parse (the firmware's
//! own boot timestamps, ACPI 6.5 §5.2.24), the `SYS_BOOT_TIMES` record (one
//! source for the kernel's encoder and libneo's decoder) and the lines shown
//! on screen. `tools/boot-time-logic-tests` checks it against hand-laid FPDT,
//! FBPT and record bytes.
//!
//! **The origin.** The firmware's FBPT timestamps are nanoseconds since
//! reset. The TSC usually counts from reset too, but nothing guarantees it
//! (firmware may write it, and a VM's starts anywhere). So a TSC time is
//! called "from power-on" only when the two agree ([`power_on_origin`]): the
//! firmware recorded its handoff (exit-boot-services) and the kernel's entry
//! TSC is not earlier than it. Otherwise the TSC times read "since TSC
//! reset", and the firmware's, on another axis, are left out of them.
//!
//! **Missing fields.** A firmware timestamp of 0 is not recorded, and is
//! never shown as 0 s. A TSC rate from CPUID leaf 0x16 (the base frequency,
//! not the TSC's) is an estimate, flagged in the record and on screen.

/// `SYS_BOOT_TIMES` record version.
pub const BOOT_TIMES_V1_VERSION: u32 = 1;
/// `SYS_BOOT_TIMES` record length in bytes.
pub const BOOT_TIMES_V1_LEN: usize = 80;
/// Record flag: the firmware timestamps are present.
pub const BOOT_TIMES_FLAG_FIRMWARE: u32 = 1;
/// Record flag: the TSC rate is an estimate (CPUID leaf 0x16).
pub const BOOT_TIMES_FLAG_RATE_ESTIMATED: u32 = 2;

/// The FBPT's boot performance record (ns since reset; 0 = not recorded).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FirmwareBoot {
    pub reset_end_ns: u64,
    pub load_image_ns: u64,
    pub start_image_ns: u64,
    pub ebs_entry_ns: u64,
    pub ebs_exit_ns: u64,
}

/// What `SYS_BOOT_TIMES` reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BootTimes {
    /// The TSC's rate, or 0 when the kernel has none.
    pub tsc_hz: u64,
    /// The TSC at the kernel's first instruction after the serial port.
    pub tsc_kernel_entry: u64,
    /// The TSC when the kernel handed the screen to the desktop, or 0.
    pub tsc_desktop_start: u64,
    /// The TSC when the boot model was built (`boot-model-preload`), or 0.
    pub tsc_model_ready: u64,
    pub firmware: Option<FirmwareBoot>,
    /// The rate is an estimate (CPUID leaf 0x16's base frequency).
    pub rate_estimated: bool,
}

/// The firmware's handoff (exit-boot-services) in ms, if it recorded one.
pub fn handoff_ms(times: &BootTimes) -> Option<u64> {
    times
        .firmware
        .map(|fw| fw.ebs_exit_ns)
        .filter(|&ns| ns != 0)
        .map(|ns| ns / 1_000_000)
}

/// Whether the TSC times are from power-on: the firmware recorded its
/// handoff and the kernel's entry TSC is not earlier than it (the TSC is
/// monotonic, so the later stamps aren't either). Without that the TSC's
/// start is unknown.
pub fn power_on_origin(times: &BootTimes) -> bool {
    match (
        handoff_ms(times),
        tsc_ms(times.tsc_kernel_entry, times.tsc_hz),
    ) {
        (Some(handoff), Some(entry)) => entry >= handoff,
        _ => false,
    }
}

/// Why the firmware timestamps are missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FpdtError {
    NoFpdt,
    BadSignature,
    BadLength,
    BadRecord,
    NoFbptPointer,
    AddressOutOfRange,
    NoBootRecord,
}

impl FpdtError {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::NoFpdt => "no-fpdt",
            Self::BadSignature => "bad-signature",
            Self::BadLength => "bad-length",
            Self::BadRecord => "bad-record",
            Self::NoFbptPointer => "no-fbpt-pointer",
            Self::AddressOutOfRange => "address-out-of-range",
            Self::NoBootRecord => "no-boot-record",
        }
    }
}

const FPDT_HEADER_LEN: usize = 36;
const FPDT_RECORD_FBPT_POINTER: u16 = 0;
const FPDT_POINTER_RECORD_LEN: usize = 16;
/// The FBPT header: signature and length.
pub const FBPT_HEADER_LEN: usize = 8;
/// The most FBPT the kernel reads; a firmware length past it is refused.
pub const FBPT_MAX_LEN: usize = 4096;
const FBPT_RECORD_BOOT: u16 = 2;
const FBPT_BOOT_RECORD_LEN: usize = 48;
const RECORD_HEADER_LEN: usize = 4;

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// Walk `table[start..len]`'s records (type u16, length u8, revision u8),
/// calling `f(type, record)`; `f` returning `Some` ends the walk. A record
/// shorter than its header or past the end fails the whole walk: a length of
/// 0 would otherwise never advance.
fn walk<T>(
    table: &[u8],
    start: usize,
    len: usize,
    mut f: impl FnMut(u16, &[u8]) -> Option<T>,
) -> Result<Option<T>, FpdtError> {
    let mut at = start;
    while at < len {
        if len - at < RECORD_HEADER_LEN {
            return Err(FpdtError::BadRecord);
        }
        let kind = u16_at(table, at).ok_or(FpdtError::BadRecord)?;
        let rec_len = usize::from(table[at + 2]);
        if rec_len < RECORD_HEADER_LEN || rec_len > len - at {
            return Err(FpdtError::BadRecord);
        }
        if let Some(found) = f(kind, &table[at..at + rec_len]) {
            return Ok(Some(found));
        }
        at += rec_len;
    }
    Ok(None)
}

/// The FBPT's physical address from the FPDT (the first FBPT pointer
/// record). `phys_bits` is the CPU's physical address width: firmware has
/// shipped addresses with high bits set, which are refused.
pub fn fbpt_address(fpdt: &[u8], phys_bits: u32) -> Result<u64, FpdtError> {
    if fpdt.get(..4) != Some(b"FPDT".as_slice()) {
        return Err(FpdtError::BadSignature);
    }
    let len = u32_at(fpdt, 4).ok_or(FpdtError::BadLength)? as usize;
    if len < FPDT_HEADER_LEN || len > fpdt.len() {
        return Err(FpdtError::BadLength);
    }
    let address = walk(fpdt, FPDT_HEADER_LEN, len, |kind, rec| {
        if kind == FPDT_RECORD_FBPT_POINTER && rec.len() >= FPDT_POINTER_RECORD_LEN {
            u64_at(rec, 8)
        } else {
            None
        }
    })?
    .ok_or(FpdtError::NoFbptPointer)?;
    if address == 0 || (phys_bits < 64 && address >> phys_bits != 0) {
        return Err(FpdtError::AddressOutOfRange);
    }
    Ok(address)
}

/// The FBPT's length from its header (to size the read), refusing one
/// shorter than the header or longer than `FBPT_MAX_LEN`.
pub fn fbpt_len(header: &[u8]) -> Result<usize, FpdtError> {
    if header.get(..4) != Some(b"FBPT".as_slice()) {
        return Err(FpdtError::BadSignature);
    }
    let len = u32_at(header, 4).ok_or(FpdtError::BadLength)? as usize;
    if !(FBPT_HEADER_LEN..=FBPT_MAX_LEN).contains(&len) {
        return Err(FpdtError::BadLength);
    }
    Ok(len)
}

/// The boot performance record from a whole FBPT (the first one, as Linux
/// keeps the first of duplicates).
pub fn parse_fbpt(fbpt: &[u8]) -> Result<FirmwareBoot, FpdtError> {
    let len = fbpt_len(fbpt)?;
    if len > fbpt.len() {
        return Err(FpdtError::BadLength);
    }
    walk(fbpt, FBPT_HEADER_LEN, len, |kind, rec| {
        if kind == FBPT_RECORD_BOOT && rec.len() >= FBPT_BOOT_RECORD_LEN {
            Some(FirmwareBoot {
                reset_end_ns: u64_at(rec, 8)?,
                load_image_ns: u64_at(rec, 16)?,
                start_image_ns: u64_at(rec, 24)?,
                ebs_entry_ns: u64_at(rec, 32)?,
                ebs_exit_ns: u64_at(rec, 40)?,
            })
        } else {
            None
        }
    })?
    .ok_or(FpdtError::NoBootRecord)
}

/// The `SYS_BOOT_TIMES` record (little-endian):
///
/// | Offset | Field |
/// |---|---|
/// | 0 | version (u32) |
/// | 4 | flags (u32; bit 0 = firmware timestamps present, bit 1 = the rate is an estimate) |
/// | 8 | TSC rate, Hz |
/// | 16 | TSC at kernel entry |
/// | 24 | TSC at desktop start |
/// | 32..72 | reset end, load image, start image, EBS entry, EBS exit (ns) |
/// | 72 | TSC when the boot model was built, or 0 |
pub fn encode(times: &BootTimes) -> [u8; BOOT_TIMES_V1_LEN] {
    let mut out = [0u8; BOOT_TIMES_V1_LEN];
    out[0..4].copy_from_slice(&BOOT_TIMES_V1_VERSION.to_le_bytes());
    let mut flags = 0;
    if times.firmware.is_some() {
        flags |= BOOT_TIMES_FLAG_FIRMWARE;
    }
    if times.rate_estimated {
        flags |= BOOT_TIMES_FLAG_RATE_ESTIMATED;
    }
    out[4..8].copy_from_slice(&flags.to_le_bytes());
    let fw = times.firmware.unwrap_or_default();
    let fields = [
        times.tsc_hz,
        times.tsc_kernel_entry,
        times.tsc_desktop_start,
        fw.reset_end_ns,
        fw.load_image_ns,
        fw.start_image_ns,
        fw.ebs_entry_ns,
        fw.ebs_exit_ns,
        times.tsc_model_ready,
    ];
    for (i, value) in fields.iter().enumerate() {
        out[8 + i * 8..16 + i * 8].copy_from_slice(&value.to_le_bytes());
    }
    out
}

/// Decode a record; `None` for another length or version, or unknown
/// flags.
pub fn decode(rec: &[u8]) -> Option<BootTimes> {
    if rec.len() != BOOT_TIMES_V1_LEN || u32_at(rec, 0)? != BOOT_TIMES_V1_VERSION {
        return None;
    }
    let flags = u32_at(rec, 4)?;
    if flags & !(BOOT_TIMES_FLAG_FIRMWARE | BOOT_TIMES_FLAG_RATE_ESTIMATED) != 0 {
        return None;
    }
    let firmware = (flags & BOOT_TIMES_FLAG_FIRMWARE != 0).then(|| FirmwareBoot {
        reset_end_ns: u64_at(rec, 32).unwrap_or(0),
        load_image_ns: u64_at(rec, 40).unwrap_or(0),
        start_image_ns: u64_at(rec, 48).unwrap_or(0),
        ebs_entry_ns: u64_at(rec, 56).unwrap_or(0),
        ebs_exit_ns: u64_at(rec, 64).unwrap_or(0),
    });
    Some(BootTimes {
        tsc_hz: u64_at(rec, 8)?,
        tsc_kernel_entry: u64_at(rec, 16)?,
        tsc_desktop_start: u64_at(rec, 24)?,
        tsc_model_ready: u64_at(rec, 72)?,
        firmware,
        rate_estimated: flags & BOOT_TIMES_FLAG_RATE_ESTIMATED != 0,
    })
}

/// Milliseconds since the TSC's start for a reading, or `None` without a
/// rate (power-on only when [`power_on_origin`] holds).
pub fn tsc_ms(tsc: u64, tsc_hz: u64) -> Option<u64> {
    if tsc_hz == 0 {
        return None;
    }
    Some((u128::from(tsc) * 1000 / u128::from(tsc_hz)) as u64)
}

/// `ms` as seconds with two decimals (truncated), e.g. `12.34 s`.
pub struct Secs(pub u64);

impl core::fmt::Display for Secs {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{:02} s", self.0 / 1000, (self.0 % 1000) / 10)
    }
}

/// The kernel's line at desktop start (the fb console's last), e.g.
/// `firmware handoff 4.12 s, kernel entry 4.30 s, desktop start 7.95 s
/// (from power-on; TSC 2800000000 Hz)`. Without the firmware record, its
/// reason replaces the first part; when the TSC's start isn't tied to
/// power-on ([`power_on_origin`]), the TSC times are `since TSC reset`.
pub fn write_kernel_line(
    out: &mut impl core::fmt::Write,
    times: &BootTimes,
    firmware_missing: Option<FpdtError>,
) -> core::fmt::Result {
    match (handoff_ms(times), times.firmware, firmware_missing) {
        (Some(ms), _, _) => write!(out, "firmware handoff {}, ", Secs(ms))?,
        (None, Some(_), _) => write!(out, "firmware handoff not recorded, ")?,
        (None, None, Some(why)) => write!(out, "firmware handoff unknown ({}), ", why.tag())?,
        (None, None, None) => write!(out, "firmware handoff unknown, ")?,
    }
    match (
        tsc_ms(times.tsc_kernel_entry, times.tsc_hz),
        tsc_ms(times.tsc_desktop_start, times.tsc_hz),
    ) {
        (Some(entry), Some(desktop)) => {
            write!(out, "kernel entry {}, ", Secs(entry))?;
            if times.tsc_model_ready != 0
                && let Some(model) = tsc_ms(times.tsc_model_ready, times.tsc_hz)
            {
                write!(out, "model ready {}, ", Secs(model))?;
            }
            write!(
                out,
                "desktop start {} ({}; TSC {} Hz{})",
                Secs(desktop),
                if power_on_origin(times) {
                    "from power-on"
                } else {
                    "since TSC reset"
                },
                times.tsc_hz,
                if times.rate_estimated {
                    ", estimated"
                } else {
                    ""
                }
            )
        }
        _ => write!(
            out,
            "no TSC rate (kernel entry TSC {}, desktop start TSC {})",
            times.tsc_kernel_entry, times.tsc_desktop_start
        ),
    }
}

/// How a line names its origin: `From power-on` or `Since TSC reset`, with
/// `(estimated clock)` after it when the rate is an estimate.
fn write_origin(out: &mut impl core::fmt::Write, times: &BootTimes) -> core::fmt::Result {
    write!(
        out,
        "{}{}",
        if power_on_origin(times) {
            "From power-on"
        } else {
            "Since TSC reset"
        },
        if times.rate_estimated {
            " (estimated clock)"
        } else {
            ""
        }
    )
}

/// The summary a milestone shows, e.g. `From power-on: firmware 4.12 s,
/// model ready 6.50 s, desktop 7.95 s, Home 8.40 s.` A part with no reading
/// is left out, and so is the firmware's when the origin isn't power-on.
pub fn write_summary(
    out: &mut impl core::fmt::Write,
    what: &str,
    times: &BootTimes,
    now_tsc: u64,
) -> core::fmt::Result {
    let Some(now) = tsc_ms(now_tsc, times.tsc_hz) else {
        return write_milestone(out, what, times, now_tsc);
    };
    write_origin(out, times)?;
    write!(out, ": ")?;
    if power_on_origin(times)
        && let Some(handoff) = handoff_ms(times)
    {
        write!(out, "firmware {}, ", Secs(handoff))?;
    }
    if times.tsc_model_ready != 0
        && let Some(model) = tsc_ms(times.tsc_model_ready, times.tsc_hz)
    {
        write!(out, "model ready {}, ", Secs(model))?;
    }
    if times.tsc_desktop_start != 0
        && let Some(desktop) = tsc_ms(times.tsc_desktop_start, times.tsc_hz)
    {
        write!(out, "desktop {}, ", Secs(desktop))?;
    }
    write!(out, "{what} {}.", Secs(now))
}

/// The line a milestone shows, e.g. `Sign-in 9.87 s after power-on
/// (firmware 4.12 s).` `what` names the milestone. Off a power-on origin it
/// reads `after TSC reset`, with no firmware part.
pub fn write_milestone(
    out: &mut impl core::fmt::Write,
    what: &str,
    times: &BootTimes,
    now_tsc: u64,
) -> core::fmt::Result {
    let on = power_on_origin(times);
    match tsc_ms(now_tsc, times.tsc_hz) {
        Some(ms) => write!(
            out,
            "{what} {} after {}{}",
            Secs(ms),
            if on { "power-on" } else { "TSC reset" },
            if times.rate_estimated {
                " (estimated clock)"
            } else {
                ""
            }
        )?,
        None => write!(out, "{what} at TSC {now_tsc} (no TSC rate)")?,
    }
    match handoff_ms(times) {
        Some(handoff) if on || times.tsc_hz == 0 => write!(out, " (firmware {}).", Secs(handoff)),
        _ => write!(out, "."),
    }
}
