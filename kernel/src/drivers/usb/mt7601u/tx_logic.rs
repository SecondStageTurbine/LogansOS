// SPDX-License-Identifier: MPL-2.0
//! ISC-1 (TX path): the MT7601U TX descriptor + USB DMA-wrap byte layouts for
//! sending an 802.11 management/data frame.
//!
//! W1 only sent MCU commands (`CPU_TX_PORT`/`DMA_COMMAND`, via `mcu_logic`) and
//! received beacons; W2 must transmit 802.11 frames (auth/assoc, then data). The
//! on-wire USB TX buffer is (dma.h `mt7601u_dma_skb_wrap`):
//!
//! ```text
//!   |  4B    |        xfer len          | pad |  4B  |
//!   | TXINFO | TXWI(20B) | 802.11 frame | →4B | zero |
//! ```
//!
//! `TXINFO.LEN = round_up(TXWI + frame, 4)`; `D_PORT = WLAN_PORT`,
//! `TYPE = DMA_PACKET` (vs `CPU_TX_PORT`/`DMA_COMMAND` for MCU). Byte-faithful to
//! the Linux mt7601u driver: `mac.h` (`struct mt76_txwi`, the `MT_TXWI_*` masks),
//! `tx.c` (`mt7601u_push_txwi`), `dma.h` (the wrap + `MT_TXD_*` masks). Pure
//! `no_std`; host-oracle'd in `tools/ai-ops-tests` against hand-laid fixtures
//! (I-12) — the builders here are never the source of the oracle's bytes.

#![allow(dead_code)] // the TX surface is consumed by the ISC-1 driver wiring

use super::regs;

/// `struct mt76_txwi` is 20 bytes (mac.h:109-124).
pub const TXWI_LEN: usize = 20;

// ── TXWI field masks (mac.h:126-153) ────────────────────────────────────────
/// ack_ctl: request an ACK for this (unicast) frame.
pub const TXWI_ACK_CTL_REQ: u8 = 1 << 0;
/// ack_ctl: no-sequence (broadcast/multicast, no ACK expected).
pub const TXWI_ACK_CTL_NSEQ: u8 = 1 << 1;
/// len_ctl: MPDU byte count, GENMASK(11, 0).
pub const TXWI_LEN_BYTE_CNT: u16 = 0x0fff;
/// len_ctl: packet-id, GENMASK(15, 12).
pub const TXWI_LEN_PKTID_SHIFT: u16 = 12;

// ── TXD packet-info flags (dma.h:40-52), for the WLAN_PORT/DMA_PACKET wrap ───
/// The frame is raw 802.11 (not 802.3 for the HW to convert).
pub const TXD_PKT_INFO_80211: u32 = 1 << 19;
/// QSEL field, GENMASK(26, 25).
pub const TXD_PKT_INFO_QSEL_SHIFT: u32 = 25;
/// QSEL values (enum mt76_qsel, dma.h:47-52).
pub const QSEL_MGMT: u32 = 0;
pub const QSEL_HCCA: u32 = 1;
pub const QSEL_EDCA: u32 = 2;
/// WCID-is-invalid: dma.c sets this for an unencrypted frame (the wcid has no
/// hardware key, `hw_key_idx == 0xff`) — our pre-assoc mgmt case. (dma.h:44)
pub const TXD_PKT_INFO_WIV: u32 = 1 << 24;

/// The TXD flags for an unencrypted management frame on the data EP, per dma.c
/// `mt7601u_dma_enqueue_tx`: the 802.11 bit, WIV (no key), QSEL_EDCA.
pub fn mgmt_tx_flags() -> u32 {
    pkt_flags(QSEL_EDCA, true) | TXD_PKT_INFO_WIV
}

/// The TXD flags for an encrypted DATA frame (post-handshake): 802.11 + QSEL_EDCA,
/// NO WIV — the WCID has a key (`hw_key_idx != 0xff`), so the firmware encrypts
/// (CCMP) using the installed pairwise key (dma.c sets WIV only when keyless).
pub fn data_tx_flags() -> u32 {
    pkt_flags(QSEL_EDCA, true)
}

#[inline]
fn round_up4(n: usize) -> usize {
    (n + 3) & !3
}

/// The fields a caller sets in the TXWI for one frame. `rate_ctl` is the encoded
/// MT_TXWI_RATE word (0 = the lowest basic rate, fine for mgmt); `wcid` selects
/// the hardware client/key slot; `pkt_id` tags the frame for TX-status matching.
pub struct TxwiParams {
    pub mpdu_len: u16,
    pub wcid: u8,
    pub rate_ctl: u16,
    pub ack: bool,
    pub pkt_id: u8,
}

/// Write the 20-byte TXWI for one frame (tx.c `mt7601u_push_txwi`, no-AMPDU
/// path). `flags`=0 (no aggregation for mgmt); iv/eiv/aid/txstream/ctl=0 (HW
/// crypto handles CCMP for data via the WCID key table — ISC-3 — not the TXWI
/// SW-IV fields, which stay 0). Returns `TXWI_LEN`.
pub fn build_txwi(buf: &mut [u8], p: &TxwiParams) -> Option<usize> {
    if buf.len() < TXWI_LEN {
        return None;
    }
    buf[0..2].copy_from_slice(&0u16.to_le_bytes()); // flags
    buf[2..4].copy_from_slice(&p.rate_ctl.to_le_bytes()); // rate_ctl
    buf[4] = if p.ack {
        TXWI_ACK_CTL_REQ
    } else {
        TXWI_ACK_CTL_NSEQ
    }; // ack_ctl
    buf[5] = p.wcid; // wcid
    let len_ctl =
        (p.mpdu_len & TXWI_LEN_BYTE_CNT) | (((p.pkt_id & 0x0f) as u16) << TXWI_LEN_PKTID_SHIFT);
    buf[6..8].copy_from_slice(&len_ctl.to_le_bytes()); // len_ctl
    buf[8..12].copy_from_slice(&0u32.to_le_bytes()); // iv
    buf[12..16].copy_from_slice(&0u32.to_le_bytes()); // eiv
    buf[16] = 0; // aid
    buf[17] = 0; // txstream
    buf[18..20].copy_from_slice(&0u16.to_le_bytes()); // ctl
    Some(TXWI_LEN)
}

/// Compose the per-packet TXD flags: the QSEL field plus (optionally) the raw
/// 802.11 bit. Management frames use `QSEL_MGMT` and the 802.11 bit.
pub fn pkt_flags(qsel: u32, raw_80211: bool) -> u32 {
    let mut f = (qsel & 0x3) << TXD_PKT_INFO_QSEL_SHIFT;
    if raw_80211 {
        f |= TXD_PKT_INFO_80211;
    }
    f
}

/// The 4-byte TXINFO word for a packet (WLAN_PORT/DMA_PACKET). `content_len` is
/// the unrounded TXWI+frame length; the LEN field is `round_up(content_len, 4)`.
/// `flags` carries the QSEL/80211 bits (`pkt_flags`).
pub fn dma_pkt_info(content_len: usize, flags: u32) -> u32 {
    ((round_up4(content_len) as u32) & regs::TXD_INFO_LEN_MASK)
        | (regs::WLAN_PORT << regs::TXD_INFO_D_PORT_SHIFT)
        | (regs::DMA_PACKET << regs::TXD_INFO_TYPE_SHIFT)
        | flags
}

/// Assemble the full USB TX buffer for an 802.11 `frame`:
/// `[TXINFO 4B][TXWI 20B][frame][pad to 4B][4B zero]`. Returns the total length.
/// `rate_ctl`/`wcid`/`ack`/`pkt_id` populate the TXWI; `flags` the TXINFO.
pub fn build_tx_frame(
    dst: &mut [u8],
    frame: &[u8],
    wcid: u8,
    rate_ctl: u16,
    ack: bool,
    pkt_id: u8,
    flags: u32,
) -> Option<usize> {
    let content = TXWI_LEN + frame.len();
    let content_padded = round_up4(content);
    let total = 4 + content_padded + 4; // TXINFO + padded content + trailing zero
    if dst.len() < total || frame.len() > TXWI_LEN_BYTE_CNT as usize {
        return None;
    }
    // Zero the whole buffer first so the content-pad and the trailing 4B are 0.
    for b in dst[..total].iter_mut() {
        *b = 0;
    }
    dst[0..4].copy_from_slice(&dma_pkt_info(content, flags).to_le_bytes());
    build_txwi(
        &mut dst[4..4 + TXWI_LEN],
        &TxwiParams {
            mpdu_len: frame.len() as u16,
            wcid,
            rate_ctl,
            ack,
            pkt_id,
        },
    )?;
    dst[4 + TXWI_LEN..4 + TXWI_LEN + frame.len()].copy_from_slice(frame);
    Some(total)
}
