// SPDX-License-Identifier: MPL-2.0
//! Handing SWGL a caller-owned framebuffer, checked first (#66 review,
//! Jarvis, item 2).
//!
//! SWGL's `init_default_framebuffer` passes its arguments to
//! `set_tex_storage` unchecked (`gl.cc:2564`): a zero-sized surface isn't
//! refused, `set_buffer`'s stride assert (`gl.cc:497`) accepts stride 0 when
//! the width is 0, and nothing looks at the buffer's length or alignment. So
//! every call goes through [`attach`], which refuses what SWGL would take on
//! trust. The rules live here, not as another patch to upstream SWGL.

use std::ffi::c_void;

/// Why a framebuffer was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FbError {
    /// Width or height is 0.
    Empty,
    /// The stride is less than `width * 4` bytes (SWGL writes BGRA8).
    StrideTooSmall,
    /// The buffer is shorter than `stride * height` bytes.
    TooShort,
    /// The buffer's address isn't a multiple of 4.
    Misaligned,
    /// A size doesn't fit SWGL's `i32` arguments, or `stride * height`
    /// overflows.
    TooLarge,
}

/// Check a framebuffer of `len` bytes at `addr` for a `width` x `height`
/// BGRA8 surface with `stride` bytes per row.
pub fn check_fb(
    addr: usize,
    len: usize,
    width: usize,
    height: usize,
    stride: usize,
) -> Result<(), FbError> {
    if width == 0 || height == 0 {
        return Err(FbError::Empty);
    }
    let fits = |v: usize| i32::try_from(v).is_ok();
    if !fits(width) || !fits(height) || !fits(stride) {
        return Err(FbError::TooLarge);
    }
    let row = width.checked_mul(4).ok_or(FbError::TooLarge)?;
    if stride < row {
        return Err(FbError::StrideTooSmall);
    }
    let need = stride.checked_mul(height).ok_or(FbError::TooLarge)?;
    if len < need {
        return Err(FbError::TooShort);
    }
    if addr % 4 != 0 {
        return Err(FbError::Misaligned);
    }
    Ok(())
}

/// Make `buf` SWGL's default framebuffer for `ctx`, after [`check_fb`].
/// Nothing reaches SWGL if the check fails.
///
/// # Safety
/// SWGL keeps the pointer: `buf` must stay valid, and must not be read or
/// written except through `ctx`'s `get_color_buffer`, until the context is
/// destroyed or given another framebuffer.
pub unsafe fn attach(
    ctx: &swgl::Context,
    buf: &mut [u8],
    width: usize,
    height: usize,
    stride: usize,
) -> Result<(), FbError> {
    check_fb(buf.as_ptr() as usize, buf.len(), width, height, stride)?;
    ctx.init_default_framebuffer(
        0,
        0,
        width as i32,
        height as i32,
        stride as i32,
        buf.as_mut_ptr().cast::<c_void>(),
    );
    Ok(())
}
