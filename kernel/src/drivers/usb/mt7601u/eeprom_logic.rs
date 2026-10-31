// SPDX-License-Identifier: MPL-2.0
//! MT7601U eFUSE/EEPROM logic (Phase 7.5 BUILD-W1, ISC-4) — the pure,
//! host-testable core of the eFUSE reader: the `MT_EFUSE_CTRL` kick-word
//! composition, the usage-map physical-size hard-fail (Trap 2 / T-011), and the
//! EEPROM field decode (version, MAC, country).
//!
//! Byte-faithful to the Linux `mt7601u` driver (`eeprom.c`
//! `mt7601u_efuse_read`/`mt7601u_efuse_physical_size_check`/
//! `mt7601u_eeprom_init`; v6.6 GPL-2.0). The HARD-FAIL is ported faithfully:
//! a blank-eFUSE clone (usage map < 5 programmed words) returns `Err` and
//! aborts init — for the confirmed adapter (programmed MAC + EEPROM ver 0x0d)
//! the check passes (DISPATCH §2). NO kernel deps — path-mod'd into
//! `tools/ai-ops-tests` and asserted against a hand-laid byte fixture (I-12).

#![allow(dead_code)] // host-oracle'd core; the kernel eFUSE path is the consumer

use super::regs;

/// Compose the `MT_EFUSE_CTRL` kick-word for a read (`mt7601u_efuse_read`):
/// preserve everything but AIN+MODE, set `AIN=(addr & ~0xf)`, `MODE=mode`,
/// `KICK`. `cur` is the current register value.
pub fn efuse_ctrl_kick(cur: u32, addr: u16, mode: u32) -> u32 {
    let mut val = cur & !(regs::EFUSE_CTRL_AIN_MASK | regs::EFUSE_CTRL_MODE_MASK);
    val |= (((addr as u32) & !0xf) << regs::EFUSE_CTRL_AIN_SHIFT) & regs::EFUSE_CTRL_AIN_MASK;
    val |= (mode << regs::EFUSE_CTRL_MODE_SHIFT) & regs::EFUSE_CTRL_MODE_MASK;
    val |= regs::EFUSE_CTRL_KICK;
    val
}

/// Whether the read fell OUTSIDE the usage map — `(ctrl & AOUT) == AOUT`
/// (all-ones) ⇒ the 16-byte block is filled 0xff, not real data.
pub fn efuse_out_of_map(ctrl_after: u32) -> bool {
    (ctrl_after & regs::EFUSE_CTRL_AOUT) == regs::EFUSE_CTRL_AOUT
}

/// The eFUSE usage-map physical-size check (`mt7601u_efuse_physical_size_check`).
/// `map` is the bytes read PHYSICALLY from `MT_EE_USAGE_MAP_START` (≥ 29).
/// Returns `Ok(())` if the device is programmed, `Err(())` for the HARD-FAIL
/// (the blank-clone "needs default EEPROM file" abort). Faithful to the C:
/// `cnt_free = last_zero_addr - first_zero_addr + 1`; fail if `SIZE - cnt_free
/// < 5`.
pub fn usage_map_ok(map: &[u8]) -> Result<(), ()> {
    if map.len() < regs::EFUSE_USAGE_MAP_SIZE {
        return Err(());
    }
    let mut start: u32 = 0;
    let mut end: u32 = 0;
    for (i, &byte) in map.iter().take(regs::EFUSE_USAGE_MAP_SIZE).enumerate() {
        if byte == 0 {
            let addr = (regs::EE_USAGE_MAP_START + i) as u32;
            if start == 0 {
                start = addr;
            }
            end = addr;
        }
    }
    let cnt_free = end - start + 1;
    if (regs::EFUSE_USAGE_MAP_SIZE as u32).saturating_sub(cnt_free) < 5 {
        return Err(());
    }
    Ok(())
}

/// EEPROM version (`eeprom[0x03]`) — expected `0x0d` for the confirmed adapter.
pub fn version(eeprom: &[u8]) -> u8 {
    eeprom[regs::EE_VERSION_EE]
}
/// EEPROM FAE byte (`eeprom[0x02]`) — expected `0x00`.
pub fn version_fae(eeprom: &[u8]) -> u8 {
    eeprom[regs::EE_VERSION_FAE]
}
/// Whether the version is newer than the driver supports (warn, not fatal).
pub fn version_unsupported(eeprom: &[u8]) -> bool {
    eeprom[regs::EE_VERSION_EE] > regs::EE_MAX_VER
}
/// The 6-byte MAC at `eeprom[0x04..0x0a]` — the adapter's factory MAC.
pub fn mac(eeprom: &[u8]) -> [u8; 6] {
    let mut m = [0u8; 6];
    m.copy_from_slice(&eeprom[regs::EE_MAC_ADDR..regs::EE_MAC_ADDR + 6]);
    m
}
/// Country region byte (`eeprom[0x39]`) — bounds the visible channels.
pub fn country_region(eeprom: &[u8]) -> u8 {
    eeprom[regs::EE_COUNTRY_REGION]
}

/// Whether the MAC is a usable unicast address (not all-zero, not all-ones,
/// not multicast) — else the driver substitutes a random one (`mac.c`).
pub fn mac_is_valid(m: &[u8; 6]) -> bool {
    let all_zero = m.iter().all(|&b| b == 0);
    let all_ones = m.iter().all(|&b| b == 0xff);
    let multicast = m[0] & 0x01 != 0;
    !all_zero && !all_ones && !multicast
}

/// eeprom.h `int_to_s6`: clamp a signed power delta into the 6-bit signed field
/// the per-rate TX-power registers use. Byte-exact to upstream:
/// `val < -0x20 → 0x20; val > 0x1f → 0x1f; else val & 0x3f`.
pub fn int_to_s6(val: i32) -> u32 {
    if val < -0x20 {
        return 0x20;
    }
    if val > 0x1f {
        return 0x1f;
    }
    (val & 0x3f) as u32
}

/// A conservative all-rate TX-power word (`MT_TX_PWR_CFG_x` layout: four packed
/// signed-6-bit fields) at `pwr` dBd-relative — used as the safe floor when the
/// EEPROM per-rate word is blank/all-zero (which would otherwise target ~0 power
/// and leave the transmitter too weak for the AP to hear). `pwr` is encoded via
/// `int_to_s6` into all four byte lanes, matching how upstream writes per-rate
/// power.
pub fn rate_power_floor_word(pwr: i32) -> u32 {
    let f = int_to_s6(pwr) & 0xff;
    (f << 24) | (f << 16) | (f << 8) | f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_to_s6_matches_upstream_bit_math() {
        // Positive in-range values pass through unchanged.
        assert_eq!(int_to_s6(0), 0x00);
        assert_eq!(int_to_s6(6), 0x06);
        assert_eq!(int_to_s6(0x1f), 0x1f);
        // Above the positive ceiling clamps to 0x1f.
        assert_eq!(int_to_s6(32), 0x1f);
        assert_eq!(int_to_s6(0x7fff_ffff), 0x1f);
        // Negative in-range values use the 6-bit two's-complement encoding.
        assert_eq!(int_to_s6(-1), 0x3f);
        assert_eq!(int_to_s6(-0x20), 0x20);
        // Below the negative floor clamps to 0x20.
        assert_eq!(int_to_s6(-33), 0x20);
    }

    #[test]
    fn floor_word_packs_default_power_into_all_lanes() {
        // MT7601U_DEFAULT_TX_POWER (6) in every per-rate field.
        assert_eq!(rate_power_floor_word(6), 0x0606_0606);
        // A negative floor sign-extends per lane (here -1 → 0x3f).
        assert_eq!(rate_power_floor_word(-1), 0x3f3f_3f3f);
    }
}
