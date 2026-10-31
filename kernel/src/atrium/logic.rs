// SPDX-License-Identifier: MPL-2.0
//! Pure, host-testable logic for the G1 compositor slice-1 (Rung 0).
//!
//! This module holds every decision the atrium kernel objects make that can
//! be expressed without kernel state: buffer-descriptor validation, the
//! descriptor wire codec (byte-oracle target), present-fence epoch
//! satisfaction, the fence outcome codes, the capability rights bits, and
//! the console routing selector for the kill-switch equality KAT.
//!
//! Mounted on the host by `tools/atrium-logic-tests` via `#[path]` — keep it
//! free of `crate::` paths and `cfg(feature = ...)` internals (the mounted
//! copy compiles under the host crate's feature set).

/// DRM fourcc 'XR24': little-endian XRGB8888 — u32 pixel `0x??RRGGBB`,
/// memory bytes `[B, G, R, X]`. Byte-identical to the framebuffer's native
/// 32-bit layout (`fb::Color`), so the present copy is a u32 row copy.
pub const FORMAT_XRGB8888_LE: u32 = 0x3432_5258;

/// Hard ceiling on a single shared pixel buffer (G1 design, chad §4.1).
pub const MAX_BUFFER_BYTES: u64 = 64 * 1024 * 1024;

/// Size of the `BufferDescV1` wire encoding in bytes (4 × u32 + 1 × u64).
pub const BUFFER_DESC_V1_LEN: usize = 24;

/// One buffer-coordinate damage rectangle: four little-endian u16 fields.
/// Slice 5 deliberately accepts one rectangle per commit. This keeps per-frame
/// work bounded to one clipped region and makes flood behavior degrade only
/// the client's own surface.
pub const DAMAGE_RECT_V1_LEN: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DamageRectV1 {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

pub fn encode_damage(rect: DamageRectV1) -> [u8; DAMAGE_RECT_V1_LEN] {
    let mut out = [0u8; DAMAGE_RECT_V1_LEN];
    out[0..2].copy_from_slice(&rect.x.to_le_bytes());
    out[2..4].copy_from_slice(&rect.y.to_le_bytes());
    out[4..6].copy_from_slice(&rect.width.to_le_bytes());
    out[6..8].copy_from_slice(&rect.height.to_le_bytes());
    out
}

pub fn decode_damage(bytes: &[u8]) -> Option<DamageRectV1> {
    if bytes.len() != DAMAGE_RECT_V1_LEN {
        return None;
    }
    Some(DamageRectV1 {
        x: u16::from_le_bytes([bytes[0], bytes[1]]),
        y: u16::from_le_bytes([bytes[2], bytes[3]]),
        width: u16::from_le_bytes([bytes[4], bytes[5]]),
        height: u16::from_le_bytes([bytes[6], bytes[7]]),
    })
}

/// Validate without lossy clipping: a malformed/wild client cannot expand a
/// commit beyond its immutable buffer descriptor or trigger arithmetic wrap.
pub fn validate_damage(rect: DamageRectV1, desc: &BufferDescV1) -> bool {
    let x = rect.x as u32;
    let y = rect.y as u32;
    let width = rect.width as u32;
    let height = rect.height as u32;
    width != 0
        && height != 0
        && x.checked_add(width).is_some_and(|end| end <= desc.width)
        && y.checked_add(height).is_some_and(|end| end <= desc.height)
}

/// The immutable shared-pixel-buffer descriptor (G1 `BufferDescV1`).
///
/// Stored in the kernel object at create time; NEVER reachable through a
/// mapped page (ISC-1). Crosses the syscall boundary by value as the 24-byte
/// little-endian encoding below (byte-oracle: `encode`/`decode` are asserted
/// against a hand-laid fixture independent of both).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BufferDescV1 {
    pub width: u32,
    pub height: u32,
    /// Row pitch in bytes. `>= width * 4` and a multiple of 4.
    pub stride: u32,
    /// Pixel format. Slice-1 accepts exactly [`FORMAT_XRGB8888_LE`].
    pub format: u32,
    /// Total byte length: exactly `stride * height`.
    pub bytes: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DescError {
    ZeroDimension,
    StrideTooSmall,
    StrideMisaligned,
    UnsupportedFormat,
    BytesMismatch,
    TooLarge,
    ExceedsOutput,
    Overflow,
}

/// Validate a descriptor against the current output (framebuffer) size.
/// All arithmetic is checked; a `None` anywhere is `Overflow`.
pub fn validate_desc(desc: &BufferDescV1, out_w: u32, out_h: u32) -> Result<(), DescError> {
    if desc.width == 0 || desc.height == 0 {
        return Err(DescError::ZeroDimension);
    }
    if desc.format != FORMAT_XRGB8888_LE {
        return Err(DescError::UnsupportedFormat);
    }
    let min_stride = (desc.width as u64)
        .checked_mul(4)
        .ok_or(DescError::Overflow)?;
    if (desc.stride as u64) < min_stride {
        return Err(DescError::StrideTooSmall);
    }
    if !desc.stride.is_multiple_of(4) {
        return Err(DescError::StrideMisaligned);
    }
    let expected_bytes = (desc.stride as u64)
        .checked_mul(desc.height as u64)
        .ok_or(DescError::Overflow)?;
    if desc.bytes != expected_bytes {
        return Err(DescError::BytesMismatch);
    }
    if desc.bytes > MAX_BUFFER_BYTES {
        return Err(DescError::TooLarge);
    }
    if desc.width > out_w || desc.height > out_h {
        return Err(DescError::ExceedsOutput);
    }
    Ok(())
}

/// Decode the 24-byte little-endian wire form. Total function — validation
/// is separate ([`validate_desc`]) so the codec itself is byte-oracle-able.
pub fn decode_desc(bytes: &[u8; BUFFER_DESC_V1_LEN]) -> BufferDescV1 {
    let u32_at =
        |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let u64_at = |i: usize| {
        u64::from_le_bytes([
            bytes[i],
            bytes[i + 1],
            bytes[i + 2],
            bytes[i + 3],
            bytes[i + 4],
            bytes[i + 5],
            bytes[i + 6],
            bytes[i + 7],
        ])
    };
    BufferDescV1 {
        width: u32_at(0),
        height: u32_at(4),
        stride: u32_at(8),
        format: u32_at(12),
        bytes: u64_at(16),
    }
}

/// Encode to the 24-byte little-endian wire form (the inverse of
/// [`decode_desc`]; both are asserted against the hand-laid oracle).
pub fn encode_desc(desc: &BufferDescV1) -> [u8; BUFFER_DESC_V1_LEN] {
    let mut out = [0u8; BUFFER_DESC_V1_LEN];
    out[0..4].copy_from_slice(&desc.width.to_le_bytes());
    out[4..8].copy_from_slice(&desc.height.to_le_bytes());
    out[8..12].copy_from_slice(&desc.stride.to_le_bytes());
    out[12..16].copy_from_slice(&desc.format.to_le_bytes());
    out[16..24].copy_from_slice(&desc.bytes.to_le_bytes());
    out
}

// === Present-fence outcomes (G1 `PresentResultV1.outcome`) ===

pub const OUTCOME_COMPOSED_TO_FB: u32 = 1;
pub const OUTCOME_NO_DAMAGE: u32 = 2;

/// Size of the `PresentResultV1` wire encoding in bytes (2 × u64 + 2 × u32 +
/// 1 × u64 = 32). Equals `size_of::<PresentResultV1>()` under `repr(C)` — the
/// syscall copy-out writes the struct's in-memory bytes directly, so on a
/// little-endian target those bytes ARE this encoding (asserted host-side).
pub const PRESENT_RESULT_V1_LEN: usize = 32;

/// Result record for one present (G1 `PresentResultV1`, chad §5.3 subset).
///
/// `repr(C)`, all-integer POD. Crosses the syscall boundary by value as the
/// 32-byte little-endian encoding below. Slice-1 stamps only the fields the
/// software path can fill honestly; the hardware fields stay 0 and no
/// VSYNC/HW/ZERO_COPY flag is ever set. Byte-oracle: `encode`/`decode` are
/// asserted against a hand-laid 32-byte fixture independent of both, and the
/// in-memory `repr(C)` bytes are asserted equal to that fixture — so the
/// oracle guards the actual `write_unaligned` copy-out path, not a parallel
/// codec (ADR-028 discipline).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(C)]
pub struct PresentResultV1 {
    pub present_id: u64,
    pub outcome: u32,
    pub flags: u32,
    pub submit_tick: u64,
    pub complete_tick: u64,
}

// The pinned wire length MUST equal the repr(C) struct size: the syscall
// copies the struct's in-memory bytes straight out, so any layout drift
// (a field widened / reordered / padded) would desync the wire form from
// PRESENT_RESULT_V1_LEN. This static assert fails the build if they part.
const _: () = assert!(PRESENT_RESULT_V1_LEN == core::mem::size_of::<PresentResultV1>());

/// Decode the 32-byte little-endian wire form. Total function (no validation),
/// so the codec itself is byte-oracle-able.
pub fn decode_present_result(bytes: &[u8; PRESENT_RESULT_V1_LEN]) -> PresentResultV1 {
    let u32_at =
        |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let u64_at = |i: usize| {
        u64::from_le_bytes([
            bytes[i],
            bytes[i + 1],
            bytes[i + 2],
            bytes[i + 3],
            bytes[i + 4],
            bytes[i + 5],
            bytes[i + 6],
            bytes[i + 7],
        ])
    };
    PresentResultV1 {
        present_id: u64_at(0),
        outcome: u32_at(8),
        flags: u32_at(12),
        submit_tick: u64_at(16),
        complete_tick: u64_at(24),
    }
}

/// Encode to the 32-byte little-endian wire form (the inverse of
/// [`decode_present_result`]; both are asserted against the hand-laid oracle).
/// The field offsets match the `repr(C)` layout of [`PresentResultV1`]:
/// present_id\@0, outcome\@8, flags\@12, submit_tick\@16, complete_tick\@24.
pub fn encode_present_result(r: &PresentResultV1) -> [u8; PRESENT_RESULT_V1_LEN] {
    let mut out = [0u8; PRESENT_RESULT_V1_LEN];
    out[0..8].copy_from_slice(&r.present_id.to_le_bytes());
    out[8..12].copy_from_slice(&r.outcome.to_le_bytes());
    out[12..16].copy_from_slice(&r.flags.to_le_bytes());
    out[16..24].copy_from_slice(&r.submit_tick.to_le_bytes());
    out[24..32].copy_from_slice(&r.complete_tick.to_le_bytes());
    out
}

/// Epoch satisfaction for the software present fence: a waiter for present
/// `wait_epoch` is satisfied once the buffer's completed epoch has reached
/// it. Monotonic u64 epochs never reuse, so a waiter can never satisfy on a
/// PRIOR present's residual signal (the buffer-reuse ABA guard, ISC-39).
#[inline]
pub const fn fence_satisfied(completed_epoch: u64, wait_epoch: u64) -> bool {
    completed_epoch >= wait_epoch
}

// === Capability rights bits (semantic byte of `CapRights`) ===

pub mod buffer_rights {
    /// Map the buffer writable (client role).
    pub const MAP_WRITE: u8 = 1 << 0;
    /// Map the buffer read-only (compositor role).
    pub const MAP_READ: u8 = 1 << 1;
    /// Read the immutable descriptor.
    pub const QUERY: u8 = 1 << 2;
    /// Present this buffer object (`sys_fb_present`). When scanout enforcement
    /// is armed, the calling process must also hold a `GpuSubmitCap`; this bit
    /// alone cannot authorize raw framebuffer access.
    pub const PRESENT: u8 = 1 << 3;
    /// Mark pending pixel writes ready for the next present.
    pub const COMMIT: u8 = 1 << 4;

    /// The **client (owner) role** semantic set: the creator maps RW, reads
    /// the descriptor, requests present, and commits. `PRESENT` remains object
    /// authority only; it does not imply scanout authority. Equals
    /// `full_buffer_rights`'s semantic byte (kept here as pure logic so the host
    /// KATs sweep it).
    pub const CLIENT_ROLE: u8 = MAP_WRITE | MAP_READ | QUERY | PRESENT | COMMIT;

    /// The **compositor (importer) role** semantic set — the cross-process
    /// C4/C3 attenuation: a strict read-only *sibling* of the client. The
    /// compositor may map RO, read the descriptor, and composite (present),
    /// but carries **neither** `MAP_WRITE` **nor** `COMMIT`. This is the
    /// offered subset the client grants over the surface channel; the grant
    /// path intersects it with the root, so a compositor cap can never widen
    /// back to write access (Slice-3 cross-process import).
    pub const COMPOSITOR_ROLE: u8 = MAP_READ | QUERY | PRESENT;
}

/// C3 relocated-surface invariant (Slice-3): is `granted` a valid cross-process
/// compositor import of a shared pixel buffer? A valid import is a **strict
/// read-only sibling** — it can map read-only (`MAP_READ` present) but can
/// **never** write the client's pixels (`MAP_WRITE` absent) nor drive the
/// commit gate (`COMMIT` absent). This is the pure-logic half of the "the
/// compositor genuinely cannot write" proof: the kernel enforces it at map
/// time via the hardware RO+NX PTE; this predicate pins the *rights* half so
/// the host KATs assert it independently of any kernel type.
pub const fn is_readonly_compositor_import(granted: u8) -> bool {
    (granted & buffer_rights::MAP_READ != 0)
        && (granted & buffer_rights::MAP_WRITE == 0)
        && (granted & buffer_rights::COMMIT == 0)
}

/// The rights a capability actually carries after a grant: the byte-wise
/// intersection of the source's rights, the grantor's offered subset, and the
/// recipient's `max_rights` ceiling (mirrors `CapRights::intersection` on the
/// semantic byte). Pure logic so the host KATs prove the cross-process
/// attenuation — an offered `COMPOSITOR_ROLE` from a full-rights root yields
/// exactly `COMPOSITOR_ROLE`, and no combination re-introduces `MAP_WRITE`.
pub const fn granted_semantic(source: u8, offered: u8, max_rights: u8) -> u8 {
    source & offered & max_rights
}

pub mod focus_rights {
    /// Activate / transfer / deactivate this focus object (compositor root).
    pub const ACTIVATE: u8 = 1 << 0;
    /// Poll/receive through this focus object (client leaf).
    pub const RECEIVE: u8 = 1 << 1;
}

// === Console routing selector (kill-switch equality KAT target) ===

// Kernel-side dead until the takeover slice wires it into the stdout/stdin
// routing — consumed TODAY by the host equality KAT (tools/atrium-logic-tests
// mounts this file), which pins the off-branch to the v1 path forever.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConsoleRoute {
    /// The v1 framebuffer console / serial stdin paths — unchanged.
    V1Console,
}

/// Which console path serves `sys_write(1)` / `sys_read(0)`.
///
/// Slice-1 ships NO output/input takeover: the selector returns
/// [`ConsoleRoute::V1Console`] for every input — including both switches on.
/// The host KAT sweeps all four combinations to prove the off-branches (and,
/// in slice-1, the on-branch) are the v1 path. Takeover arrives with the G1
/// protocol slice and will extend this enum; the KAT then pins the
/// off-branch forever (compositor-off ≡ v1 console).
#[allow(dead_code)]
#[inline]
pub const fn select_console_route(_feature_on: bool, _runtime_on: bool) -> ConsoleRoute {
    ConsoleRoute::V1Console
}

/// Whether the atrium object syscalls are reachable: requires both the
/// compile-time feature (caller passes `cfg!(...)`) and the runtime switch.
/// Off ⇒ every atrium syscall returns `ENOSYS` and no object is ever minted.
#[inline]
pub const fn atrium_syscalls_enabled(feature_on: bool, runtime_on: bool) -> bool {
    feature_on && runtime_on
}

/// Desktop S1 — `SYS_FB_OUTPUT_QUERY` return encoding. The output (scanout)
/// size travels in the one return register: `width` in bits 63..32, `height`
/// in bits 31..0. Both are clamped to 31 bits so a success is never negative
/// (negative returns are errnos). `None` for an absent/zero-sized output.
/// libneo's `sys_fb_output_query` is the inverse; the host KAT pins the bytes.
#[allow(dead_code)]
#[inline]
pub const fn pack_output_size(width: usize, height: usize) -> Option<i64> {
    if width == 0 || height == 0 || width > i32::MAX as usize || height > i32::MAX as usize {
        return None;
    }
    Some(((width as i64) << 32) | height as i64)
}
