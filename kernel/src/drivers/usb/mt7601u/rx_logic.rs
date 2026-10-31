// SPDX-License-Identifier: MPL-2.0
//! MT7601U RX/scan logic (Phase 7.5 BUILD-W1, ISC-5) — the pure,
//! host-testable core of the bulk RX path: the DMA segment walk, the RXWI
//! strip, and the 802.11 beacon/probe-response → BSSID/SSID extraction that
//! turns received management frames into a scan list.
//!
//! Byte-faithful to the Linux `mt7601u` driver (`dma.c`
//! `mt7601u_rx_next_seg_len`/`mt7601u_rx_process_seg`, `mac.h`
//! `struct mt7601u_rxwi`; v6.6 GPL-2.0) and IEEE 802.11 management-frame
//! layout. NO kernel deps — path-mod'd into `tools/ai-ops-tests` and asserted
//! against a hand-laid beacon fixture (I-12).

#![allow(dead_code)] // host-oracle'd core; the kernel RX path is the consumer

use super::regs;

/// `sizeof(struct mt7601u_rxwi)` — 28 bytes (mac.h).
pub const RXWI_SIZE: usize = 28;
/// Minimum segment length: DMA_HDR + RX_INFO + RXWI + FCE_INFO (dma.c).
pub const MIN_SEG_LEN: usize =
    regs::DMA_HDR_LEN + regs::RX_INFO_LEN + RXWI_SIZE + regs::FCE_INFO_LEN;

/// IEEE 802.11 frame-control byte-0 values (version 0, mgmt type).
pub const FC0_BEACON: u8 = 0x80;
pub const FC0_PROBE_RESP: u8 = 0x50;

/// Length of the next RX DMA segment (`mt7601u_rx_next_seg_len`): `dma_len`
/// from `le16(data[0])`, validated, returned as `DMA_HDRS + dma_len`. `None`
/// (the C's 0) when the buffer is too short or `dma_len` is malformed.
pub fn next_seg_len(data: &[u8]) -> Option<usize> {
    if data.len() < MIN_SEG_LEN {
        return None;
    }
    let dma_len = u16::from_le_bytes([data[0], data[1]]) as usize;
    if dma_len == 0
        || dma_len + regs::DMA_HDRS > data.len()
        || dma_len & 0x3 != 0
        || dma_len < MIN_SEG_LEN
    {
        return None;
    }
    Some(regs::DMA_HDRS + dma_len)
}

/// The MPDU length carried in `RXWI.ctl` (`MT_RXWI_CTL_MPDU_LEN`, bits 27:16).
pub fn rxwi_mpdu_len(seg: &[u8]) -> Option<u16> {
    if seg.len() < regs::DMA_HDR_LEN + RXWI_SIZE {
        return None;
    }
    let ctl = u32::from_le_bytes([
        seg[regs::DMA_HDR_LEN + 4],
        seg[regs::DMA_HDR_LEN + 5],
        seg[regs::DMA_HDR_LEN + 6],
        seg[regs::DMA_HDR_LEN + 7],
    ]);
    Some(((ctl >> 16) & 0xfff) as u16)
}

/// The RXWI "zero" fields (bytes 13..16 of the RXWI) must be zero
/// (`mt7601u_rx_process_seg`); a set field signals a malformed descriptor.
pub fn rxwi_zero_ok(seg: &[u8]) -> bool {
    if seg.len() < regs::DMA_HDR_LEN + RXWI_SIZE {
        return false;
    }
    let base = regs::DMA_HDR_LEN + 13;
    seg[base] == 0 && seg[base + 1] == 0 && seg[base + 2] == 0
}

/// Strip a validated segment of length `next_seg_len` to its 802.11 frame:
/// skip the 4-byte DMA header + 28-byte RXWI, drop the trailing FCE info.
/// Returns the frame slice, or `None` if the lengths don't line up.
pub fn segment_frame(seg: &[u8]) -> Option<&[u8]> {
    let total = seg.len();
    if total < regs::DMA_HDRS + RXWI_SIZE {
        return None;
    }
    // dma_len = total - DMA_HDRS; frame = seg[DMA_HDR+RXWI .. total - FCE_INFO].
    let start = regs::DMA_HDR_LEN + RXWI_SIZE; // 32
    let end = total - regs::FCE_INFO_LEN;
    if end <= start {
        return None;
    }
    Some(&seg[start..end])
}

/// A discovered BSS (the scan-list element).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Bss {
    pub bssid: [u8; 6],
    pub is_probe_resp: bool,
    /// The channel this BSS was heard on (set by the scan loop, 0 if unknown).
    /// W2 ISC-1 needs it to tune the radio before associating.
    pub channel: u8,
    /// The SSID (filled by the scan from the beacon's SSID IE; `ssid_len` bytes
    /// valid). W2 ISC-1 matches the configured target SSID against this.
    pub ssid: [u8; 32],
    pub ssid_len: u8,
}

/// Extract the BSSID from a beacon or probe-response management frame
/// (addr3 = bytes 16..22). `None` if the frame is too short or not a
/// beacon/probe-response.
pub fn frame_bssid(frame: &[u8]) -> Option<Bss> {
    if frame.len() < 24 {
        return None;
    }
    let is_beacon = frame[0] == FC0_BEACON;
    let is_probe = frame[0] == FC0_PROBE_RESP;
    if !is_beacon && !is_probe {
        return None;
    }
    let mut bssid = [0u8; 6];
    bssid.copy_from_slice(&frame[16..22]);
    Some(Bss {
        bssid,
        is_probe_resp: is_probe,
        channel: 0,      // filled by the scan loop (the frame doesn't carry it)
        ssid: [0u8; 32], // filled by the scan from frame_ssid()
        ssid_len: 0,
    })
}

/// Extract the SSID IE (tag 0) from a beacon/probe-response. The fixed body is
/// 12 bytes (timestamp 8 + beacon-interval 2 + capability 2) after the 24-byte
/// MAC header, so IEs start at offset 36. Returns the SSID bytes (may be empty
/// for a hidden SSID), or `None` if absent/malformed.
pub fn frame_ssid(frame: &[u8]) -> Option<&[u8]> {
    const IE_START: usize = 36;
    if frame.len() < IE_START {
        return None;
    }
    let mut off = IE_START;
    while off + 2 <= frame.len() {
        let tag = frame[off];
        let len = frame[off + 1] as usize;
        if off + 2 + len > frame.len() {
            return None;
        }
        if tag == 0 {
            return Some(&frame[off + 2..off + 2 + len]);
        }
        off += 2 + len;
    }
    None
}

/// Extract the AP's ACTUAL operating channel from a beacon/probe-response: the
/// DS Parameter Set IE (tag 3, 1-byte channel) is authoritative; the HT Operation
/// IE (tag 61, byte 0 = primary channel) is the fallback. Returns `None` only if
/// neither IE is present (then the scan keeps the tuned channel).
///
/// This is load-bearing: a strong, close AP on (say) ch6 is heard while the radio
/// is tuned to an OVERLAPPING channel (ch3 is 15 MHz away — both 20 MHz wide), so
/// tagging the BSS with the tuned channel points the supplicant at the WRONG
/// channel. It then transmits auth on ch3 while the AP listens on ch6 → the AP
/// never hears us (directed_to_us=0, success=false), power-independent, with a
/// byte-perfect frame and working RX. Parsing the real channel here fixes it.
pub fn frame_channel(frame: &[u8]) -> Option<u8> {
    const IE_START: usize = 36;
    if frame.len() < IE_START {
        return None;
    }
    let mut ht_primary: Option<u8> = None;
    let mut off = IE_START;
    while off + 2 <= frame.len() {
        let tag = frame[off];
        let len = frame[off + 1] as usize;
        if off + 2 + len > frame.len() {
            break;
        }
        if tag == 3 && len >= 1 {
            return Some(frame[off + 2]); // DS Parameter Set — authoritative
        }
        if tag == 61 && len >= 1 && ht_primary.is_none() {
            ht_primary = Some(frame[off + 2]); // HT Operation primary channel (fallback)
        }
        off += 2 + len;
    }
    ht_primary
}
