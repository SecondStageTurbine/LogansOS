// SPDX-License-Identifier: MPL-2.0
//! ISC-2: the MT7601U WCID hardware key-table install encoding (the pure parts).
//!
//! After the 4-way handshake, the PTK's TK (pairwise) and the GTK (group) are
//! installed into the per-WCID hardware key table so the firmware does CCMP on
//! the data path. The install is plain register writes (FINDINGS §4.2 / mac.c
//! `mt76_mac_wcid_set_key`); the byte-faithful parts — the IV/PN seed word and
//! the WCID ATTR word (cipher mode + pairwise flag) — are factored out here so
//! they can be host-oracle'd independently of the device-touching writes.

use super::regs;

/// Build the 8-byte IV/PN seed for a WCID (mac.c `mt76_mac_wcid_set_key`):
/// `iv[3] = keyidx << 6`; for TKIP/CCMP also `iv[0] |= 1` (PN starts at 1 per
/// spec) and `iv[3] |= 0x20`. `cipher` is the `mt76_cipher_type`.
pub fn build_wcid_iv(keyidx: u8, cipher: u8) -> [u8; 8] {
    let mut iv = [0u8; 8];
    iv[3] = keyidx << 6;
    if cipher >= regs::CIPHER_TKIP {
        iv[0] |= 1;
        iv[3] |= 0x20;
    }
    iv
}

/// Compute the new WCID ATTR word (mac.c): clear+set PKEY_MODE (`cipher & 7`) and
/// PKEY_MODE_EXT (`cipher >> 3`), then clear+set the PAIRWISE flag. `prev` is the
/// register's current value (the other bits — BSS index, etc. — are preserved).
pub fn wcid_attr_word(prev: u32, cipher: u8, pairwise: bool) -> u32 {
    let mut val = prev;
    val &= !regs::WCID_ATTR_PKEY_MODE_MASK & !regs::WCID_ATTR_PKEY_MODE_EXT;
    val |= ((cipher as u32 & 0x7) << regs::WCID_ATTR_PKEY_MODE_SHIFT)
        | (((cipher as u32 >> 3) & 0x1) * regs::WCID_ATTR_PKEY_MODE_EXT);
    val &= !regs::WCID_ATTR_PAIRWISE;
    if pairwise {
        val |= regs::WCID_ATTR_PAIRWISE;
    }
    val
}
