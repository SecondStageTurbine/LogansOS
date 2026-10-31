// SPDX-License-Identifier: MPL-2.0
//! MT7601U MCU framing logic (Phase 7.5 BUILD-W1, ISC-3) — the pure,
//! host-testable core of the in-band command interface: the DMA command-wrap
//! header, the RANDOM_WRITE reg-pair / BURST_WRITE payload builders, and the
//! MCU-response (seq + event-type) decode.
//!
//! Byte-faithful to the Linux `mt7601u` driver (`dma.h` `mt7601u_dma_skb_wrap`,
//! `mcu.c` `mt7601u_dma_skb_wrap_cmd`/`mt7601u_write_reg_pairs`/
//! `mt7601u_burst_write_regs`/`mt7601u_mcu_wait_resp`; v6.6 GPL-2.0). NO kernel
//! deps — path-mod'd into `tools/ai-ops-tests` and asserted against hand-laid
//! byte fixtures (I-12).

#![allow(dead_code)] // host-oracle'd core; the kernel MCU path is the consumer

use super::fw_logic::roundup4;
use super::regs;

/// Max reg pairs per RANDOM_WRITE command (`INBAND_PACKET_MAX_LEN / 8`).
pub const MAX_REG_PAIRS_PER_CMD: usize = regs::INBAND_PACKET_MAX_LEN / 8; // 24
/// Max registers per BURST_WRITE command (`INBAND_PACKET_MAX_LEN / 4 - 1`).
pub const MAX_BURST_REGS_PER_CMD: usize = regs::INBAND_PACKET_MAX_LEN / 4 - 1; // 47

/// The 4-byte DMA command-info word (`mt7601u_dma_skb_wrap` for a command):
/// `SEQ | cmd-TYPE | LEN=roundup(payload,4) | D_PORT=CPU_TX_PORT |
/// TYPE=DMA_COMMAND`. `payload_len` is the pre-wrap payload length.
pub fn dma_cmd_info(seq: u8, cmd: u8, payload_len: usize) -> u32 {
    ((seq as u32) << regs::TXD_CMD_INFO_SEQ_SHIFT)
        | ((cmd as u32) << regs::TXD_CMD_INFO_TYPE_SHIFT)
        | ((roundup4(payload_len) as u32) & regs::TXD_INFO_LEN_MASK)
        | (regs::CPU_TX_PORT << regs::TXD_INFO_D_PORT_SHIFT)
        | (regs::DMA_COMMAND << regs::TXD_INFO_TYPE_SHIFT)
}

/// Frame an MCU command into `dst`: `[4B info][payload][zero pad]`, total
/// `roundup(payload+4, 4) + 4` bytes (`skb_push` then `skb_put_padto`). Returns
/// the framed length, or `None` if `dst` is too small. `payload` is the
/// command body (already ≤ `INBAND_PACKET_MAX_LEN`).
pub fn build_mcu_frame(dst: &mut [u8], payload: &[u8], seq: u8, cmd: u8) -> Option<usize> {
    let p = payload.len();
    let total = roundup4(p + regs::DMA_HDR_LEN) + 4;
    if dst.len() < total {
        return None;
    }
    for b in dst[..total].iter_mut() {
        *b = 0;
    }
    dst[0..4].copy_from_slice(&dma_cmd_info(seq, cmd, p).to_le_bytes());
    dst[4..4 + p].copy_from_slice(payload);
    Some(total)
}

/// Build a RANDOM_WRITE payload: `[le32(base+reg)][le32(val)]` per pair. Returns
/// bytes written (`8 * pairs.len()`), or `None` if too small / over the per-cmd
/// cap. The caller chunks > `MAX_REG_PAIRS_PER_CMD`.
pub fn build_reg_pair_payload(dst: &mut [u8], base: u32, pairs: &[(u32, u32)]) -> Option<usize> {
    if pairs.len() > MAX_REG_PAIRS_PER_CMD || dst.len() < pairs.len() * 8 {
        return None;
    }
    for (i, &(reg, val)) in pairs.iter().enumerate() {
        dst[i * 8..i * 8 + 4].copy_from_slice(&base.wrapping_add(reg).to_le_bytes());
        dst[i * 8 + 4..i * 8 + 8].copy_from_slice(&val.to_le_bytes());
    }
    Some(pairs.len() * 8)
}

/// Build a BURST_WRITE payload: `[le32(MEMMAP_WLAN+offset)][le32(data)...]`.
/// Returns bytes written (`4 + 4*data.len()`), or `None` if too small / over
/// the per-cmd cap.
pub fn build_burst_payload(dst: &mut [u8], offset: u32, data: &[u32]) -> Option<usize> {
    if data.len() > MAX_BURST_REGS_PER_CMD || dst.len() < 4 + data.len() * 4 {
        return None;
    }
    dst[0..4].copy_from_slice(&regs::MCU_MEMMAP_WLAN.wrapping_add(offset).to_le_bytes());
    for (i, &v) in data.iter().enumerate() {
        let o = 4 + i * 4;
        dst[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    Some(4 + data.len() * 4)
}

/// Decoded MCU response front-word (`rxfce`, the first le32 of the resp URB).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct McuResp {
    pub seq: u8,
    pub evt_type: u8,
}

/// Decode the response front-word (`mt7601u_mcu_wait_resp`): extract CMD_SEQ +
/// EVT_TYPE.
pub fn decode_resp(rxfce: u32) -> McuResp {
    McuResp {
        seq: ((rxfce & regs::RXD_CMD_INFO_CMD_SEQ_MASK) >> regs::RXD_CMD_INFO_CMD_SEQ_SHIFT) as u8,
        evt_type: ((rxfce & regs::RXD_CMD_INFO_EVT_TYPE_MASK) >> regs::RXD_CMD_INFO_EVT_TYPE_SHIFT)
            as u8,
    }
}

/// Whether a response matches the awaited command: seq equal AND `CMD_DONE`.
pub fn resp_is_done(rxfce: u32, want_seq: u8) -> bool {
    let r = decode_resp(rxfce);
    r.seq == want_seq && r.evt_type as u32 == regs::EVT_CMD_DONE
}

/// Ownership and length verdict for one xHCI Transfer Event on the persistent
/// MCU-response pipe.
///
/// The command-response buffer is posted before a response-bearing command and
/// exactly one response TD remains outstanding. An event may be consumed only
/// when its TRB pointer maps to that outstanding ring index. This prevents a
/// late completion from an older TD from lending stale bytes to a newer command.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResponseTransfer {
    /// The event belongs to a different ring slot and must not consume or re-arm
    /// the current response buffer.
    Unowned,
    /// The owned TD completed with an error or fewer than the four bytes needed
    /// for the RXFCE response word.
    Invalid,
    /// The owned TD completed with a readable response prefix.
    Read { actual_len: usize },
}

pub fn classify_response_transfer(
    pending_index: usize,
    event_index: usize,
    completion_code: u32,
    posted_len: usize,
    transfer_residue: u32,
) -> ResponseTransfer {
    if event_index != pending_index {
        return ResponseTransfer::Unowned;
    }
    if completion_code != 1 && completion_code != 13 {
        return ResponseTransfer::Invalid;
    }
    let residue = transfer_residue as usize;
    if residue > posted_len {
        return ResponseTransfer::Invalid;
    }
    let actual_len = posted_len - residue;
    if actual_len < 4 {
        ResponseTransfer::Invalid
    } else {
        ResponseTransfer::Read { actual_len }
    }
}

/// Half-open DMA span overlap check used to prove that command TX and response
/// RX never alias the same arena bytes. Overflow is conservatively overlapping.
pub const fn dma_spans_overlap(a_start: u64, a_len: usize, b_start: u64, b_len: usize) -> bool {
    let Some(a_end) = a_start.checked_add(a_len as u64) else {
        return true;
    };
    let Some(b_end) = b_start.checked_add(b_len as u64) else {
        return true;
    };
    a_start < b_end && b_start < a_end
}

/// Mainline's chunked RANDOM_WRITE/BURST_WRITE response policy: intermediate
/// chunks use sequence zero and only the final chunk requests CMD_DONE.
pub const fn wait_response_for_chunk(index: usize, chunk_count: usize) -> bool {
    chunk_count != 0 && index + 1 == chunk_count
}
