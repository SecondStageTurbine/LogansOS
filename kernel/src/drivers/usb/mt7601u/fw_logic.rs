// SPDX-License-Identifier: MPL-2.0
//! MT7601U firmware-image logic (Phase 7.5 BUILD-W1, ISC-2) — the pure,
//! host-testable core of the firmware loader: header parse, image validation,
//! the ILM/DLM/IVB partition, and the per-chunk FCE-DMA framing.
//!
//! Byte-faithful to the Linux `mt7601u` driver (`mcu.c`
//! `mt7601u_load_firmware`/`mt7601u_upload_firmware`/`__mt7601u_dma_fw`,
//! `struct mt76_fw_header`; v6.6 GPL-2.0). NO kernel deps — path-mod'd into
//! `tools/ai-ops-tests` and asserted against a hand-laid byte fixture (I-12,
//! the byte-oracle: a writer/reader pair built from the same spec agree even
//! when both are wrong, so only an independent hand-computed example proves the
//! bytes). The device-touching loader (`mod.rs`) calls these and only adds the
//! USB transfers.

#![allow(dead_code)] // host-oracle'd core; the kernel loader is the consumer

use super::regs;

/// Parsed `struct mt76_fw_header` (32 bytes, all little-endian).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FwHeader {
    /// ILM length INCLUDING the 64-byte IVB (header field as-is).
    pub ilm_len: u32,
    pub dlm_len: u32,
    pub build_ver: u16,
    pub fw_ver: u16,
    pub build_time: [u8; 16],
}

/// Why a firmware image was rejected — the three distinct `-ENOENT` causes
/// (mcu.c `err_inv_fw`) kept separate so a Stage-B failure self-localizes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FwError {
    /// `fw->size < sizeof(*hdr)`.
    TooSmall,
    /// `ilm_len <= MT_MCU_IVB_SIZE` (no real ILM beyond the IVB).
    IlmTooShort,
    /// `fw->size != header + ilm_len + dlm_len`.
    SizeMismatch,
}

/// The image partition the uploader walks (offsets/lengths into the blob).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FwLayout {
    /// IVB: 64 bytes at `[ivb_off, ivb_off+64)`; uploaded by the control path.
    pub ivb_off: usize,
    /// ILM data (= `ilm_len - 64`) at `[ilm_off, ilm_off+ilm_data_len)`;
    /// FCE-DMA'd to dst `MT_MCU_IVB_SIZE` (64).
    pub ilm_off: usize,
    pub ilm_data_len: usize,
    /// DLM at `[dlm_off, dlm_off+dlm_len)`; FCE-DMA'd to `MT_MCU_DLM_OFFSET`.
    pub dlm_off: usize,
    pub dlm_len: usize,
}

fn le16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}
fn le32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

/// Parse the 32-byte header. `None` if fewer than 32 bytes are present.
pub fn parse_header(bytes: &[u8]) -> Option<FwHeader> {
    if bytes.len() < regs::FW_HEADER_LEN {
        return None;
    }
    let mut build_time = [0u8; 16];
    build_time.copy_from_slice(&bytes[16..32]);
    Some(FwHeader {
        ilm_len: le32(bytes, 0),
        dlm_len: le32(bytes, 4),
        build_ver: le16(bytes, 8),
        fw_ver: le16(bytes, 10),
        build_time,
    })
}

/// Validate the image and compute its partition (mcu.c `mt7601u_load_firmware`
/// size checks + `mt7601u_upload_firmware` offsets). `total_size` is the whole
/// blob length.
pub fn validate(total_size: usize, hdr: &FwHeader) -> Result<FwLayout, FwError> {
    if total_size < regs::FW_HEADER_LEN {
        return Err(FwError::TooSmall);
    }
    if (hdr.ilm_len as usize) <= regs::MCU_IVB_SIZE {
        return Err(FwError::IlmTooShort);
    }
    let expect = regs::FW_HEADER_LEN
        .wrapping_add(hdr.ilm_len as usize)
        .wrapping_add(hdr.dlm_len as usize);
    if total_size != expect {
        return Err(FwError::SizeMismatch);
    }
    let ivb_off = regs::FW_HEADER_LEN; // 32
    let ilm_data_len = hdr.ilm_len as usize - regs::MCU_IVB_SIZE;
    let ilm_off = ivb_off + regs::MCU_IVB_SIZE; // 96
    let dlm_off = ilm_off + ilm_data_len;
    Ok(FwLayout {
        ivb_off,
        ilm_off,
        ilm_data_len,
        dlm_off,
        dlm_len: hdr.dlm_len as usize,
    })
}

/// `fw_ver` decoded as Linux logs it: "%d.%d.%02d" (major.minor.patch).
pub fn fw_version(hdr: &FwHeader) -> (u8, u8, u8) {
    let v = hdr.fw_ver;
    (
        ((v >> 12) & 0xf) as u8,
        ((v >> 8) & 0xf) as u8,
        (v & 0xf) as u8,
    )
}

/// Round `n` up to a multiple of 4 (the DMA-frame alignment, `roundup(_,4)`).
pub fn roundup4(n: usize) -> usize {
    (n + 3) & !3
}

/// The 4-byte FCE TXINFO word prepended to a firmware chunk
/// (`__mt7601u_dma_fw`): `TYPE=DMA_PACKET | D_PORT=CPU_TX_PORT | LEN=len`.
pub fn fce_txinfo(len: usize) -> u32 {
    (regs::DMA_PACKET << regs::TXD_INFO_TYPE_SHIFT)
        | (regs::CPU_TX_PORT << regs::TXD_INFO_D_PORT_SHIFT)
        | ((len as u32) & regs::TXD_INFO_LEN_MASK)
}

/// The `MT_FCE_DMA_LEN` register value for a chunk (`roundup(len,4) << 16`).
pub fn fce_dma_len_reg(len: usize) -> u32 {
    (roundup4(len) as u32) << 16
}

/// Build one firmware-chunk DMA frame into `dst`: `[4B TXINFO][data][zero pad]`.
/// Transmitted length = `MT_DMA_HDR_LEN + roundup(len,4) + 4` (`__mt7601u_dma_fw`
/// `buf.len`). Returns the transmitted length, or `None` if `dst` is too small.
pub fn build_chunk(dst: &mut [u8], data: &[u8]) -> Option<usize> {
    let len = data.len();
    let xfer = regs::DMA_HDR_LEN + roundup4(len) + 4;
    if dst.len() < xfer {
        return None;
    }
    for b in dst[..xfer].iter_mut() {
        *b = 0;
    }
    dst[0..4].copy_from_slice(&fce_txinfo(len).to_le_bytes());
    dst[4..4 + len].copy_from_slice(data);
    Some(xfer)
}

/// The number of FCE chunks an upload of `len` bytes takes
/// (`MCU_FW_URB_MAX_PAYLOAD`-sized, `mt7601u_dma_fw` recursion).
pub fn chunk_count(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    len.div_ceil(regs::MCU_FW_URB_MAX_PAYLOAD)
}
