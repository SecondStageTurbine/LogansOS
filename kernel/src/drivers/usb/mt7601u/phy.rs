// SPDX-License-Identifier: MPL-2.0
//! MT7601U PHY / RF / BBP bring-up (Phase 7.5 BUILD-W1, ISC-4) — a faithful
//! port of the Linux `mt7601u` driver `phy.c` (v6.6, GPL-2.0), scoped to what
//! a passive RX **scan** needs.
//!
//! Ported: `mt7601u_phy_init` (phy.c:1226), `mt7601u_init_cal` (phy.c:1122),
//! `__mt7601u_phy_set_channel` (phy.c:342, the BW20 path), `mt7601u_vco_cal`
//! (phy.c:263), `mt7601u_rxdc_cal` (phy.c:548), `mt7601u_set_bw_filter`
//! (phy.c:271), `mt7601u_bbp_set_bw` (phy.c:1180), `mt7601u_set_rx_path`
//! (phy.c:1211), `mt7601u_set_tx_dac` (phy.c:1221), `mt7601u_bbp_set_ctrlch`
//! (phy.c:228), `mt7601u_wait_bbp_ready` (phy.c:209). The RF/BBP register
//! access primitives live on `Mt7601uDev` (`mod.rs`); the init tables are
//! `initvals_phy`.
//!
//! **W2 TX-power (now ported)** — the open-loop path that lets the AP hear us:
//! the real per-channel EEPROM power (`mt7601u_set_channel_power`, in `eeprom_init`)
//! and the per-rate `config_tx_power_per_rate` (TX_PWR_CFG_0..4, below). W1 used a
//! hardcoded `chan_pwr` placeholder and never wrote the per-rate table at all.
//!
//! **DPD bootup temp (now ported)** — `mt7601u_read_bootup_temp` feeds the real
//! chip temperature into the DPD calibration (W1 passed 0, mis-linearizing the PA).
//!
//! **Still omitted** (TSSI closed-loop *refinement*, only for fine power accuracy
//! post-assoc — a follow-on, not needed to associate): `mt7601u_tssi_dc_gain_cal`,
//! `mt7601u_temp_comp`, the CH14 CCK-OBW fixup, and the periodic cal/freq-cal
//! work-queues.

#![allow(dead_code)]

use super::Mt7601uDev;
use super::regs;
use core::fmt;

// MCU calibration ids (mcu.h `enum mcu_calibrate`).
const MCU_CAL_R: u32 = 1;
const MCU_CAL_LOFT: u32 = 4;
const MCU_CAL_TXIQ: u32 = 5;
const MCU_CAL_BW: u32 = 6;
const MCU_CAL_DPD: u32 = 7;
const MCU_CAL_RXIQ: u32 = 8;
const MCU_CAL_TXDCOC: u32 = 9;

/// mt7601u.h `MT_EE_TEMPERATURE_SLOPE` — converts (raw_temp − ref_temp) into the
/// DPD calibration's temperature units.
const MT_EE_TEMPERATURE_SLOPE: i32 = 39;
/// phy.c BBP R47 flag mask (`GENMASK(2,0)`) + the temperature-select flag.
const BBP_R47_FLAG: u8 = 0x07;
const BBP_R47_F_TEMP: u8 = 4;

/// Captured DPD-cal inputs from the most recent `init_cal`, so the assoc dispatch
/// can include them in its terminal summary. MT7601U bare-metal lanes now install
/// the NVMe sink before USB enumeration; the retained values remain useful as
/// replay evidence. Sentinel `i32::MIN` = init_cal did not run this boot.
pub(super) static CAL_RAW_TEMP: core::sync::atomic::AtomicI32 =
    core::sync::atomic::AtomicI32::new(i32::MIN);
pub(super) static CAL_DPD_TEMP: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(0);

/// A channel-programming failure with the exact operation boundary preserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelError {
    pub phase: &'static str,
    pub reason: &'static str,
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.phase, self.reason)
    }
}

fn at_channel_phase<T>(
    phase: &'static str,
    result: Result<T, &'static str>,
) -> Result<T, ChannelError> {
    result.map_err(|reason| ChannelError { phase, reason })
}

fn spin(iters: u64) {
    for _ in 0..iters {
        core::hint::spin_loop();
    }
}
fn udelay(us: u64) {
    spin(us * 2_000);
}
fn msleep(ms: u64) {
    spin(ms * 2_000_000);
}

/// phy.c:209 — poll BBP R0 (version) until it reads a sane value.
pub fn wait_bbp_ready(dev: &mut Mt7601uDev) -> Result<(), &'static str> {
    for _ in 0..20 {
        let val = dev.bbp_rr(regs::BBP_REG_VERSION);
        if val != 0 && val != 0xff {
            return Ok(());
        }
    }
    Err("BBP not ready")
}

/// phy.c:228 — `mt7601u_bbp_set_ctrlch` (BBP R3 bit 0x20).
fn bbp_set_ctrlch(dev: &mut Mt7601uDev, below: bool) {
    dev.bbp_rmw(3, 0x20, if below { 0x20 } else { 0 });
}

/// phy.c:1211 — `mt7601u_set_rx_path` (BBP R3 bits 0x18).
fn set_rx_path(dev: &mut Mt7601uDev, path: u8) {
    dev.bbp_rmw(3, 0x18, path << 3);
}

/// phy.c:1221 — `mt7601u_set_tx_dac` (BBP R1 bits 0x18).
fn set_tx_dac(dev: &mut Mt7601uDev, dac: u8) {
    dev.bbp_rmw(1, 0x18, dac << 3);
}

/// mac.c `mt7601u_mac_set_ctrlch` — the control-channel band bit. For the BW20
/// scan `below` is false, so this clears the upper-40M bit.
fn mac_set_ctrlch(dev: &mut Mt7601uDev, below: bool) -> Result<(), &'static str> {
    dev.rmw(
        regs::TX_BAND_CFG,
        regs::TX_BAND_CFG_UPPER_40M,
        if below {
            regs::TX_BAND_CFG_UPPER_40M
        } else {
            0
        },
    )?;
    Ok(())
}

/// phy.c:263 — `mt7601u_vco_cal`.
pub(super) fn vco_cal(dev: &mut Mt7601uDev) -> Result<(), &'static str> {
    dev.rf_wr(0, 4, 0x0a)?;
    dev.rf_wr(0, 5, 0x20)?;
    // rf_set(0, 4, BIT(7)) — read-or-bit-write.
    let v = dev.rf_rr(0, 4) | (1 << 7);
    dev.rf_wr(0, 4, v)?;
    msleep(2);
    Ok(())
}

/// THINK §3 channel-triple discriminator (mandatory on every W2 bring-up run):
/// `beacon_ds` (the AP's DS Parameter Set channel, from its beacon) vs `tuned`
/// (what we programmed via `FREQ_PLAN`) vs `tx` (the channel the RF synth is
/// ACTUALLY on — RF bank-0 regs 17..=20 reverse-matched to `FREQ_PLAN`). Any
/// divergence is a wrong-channel TX: `beacon_ds != tuned` is the DS-param mis-tag
/// bug just fixed; `tuned != tx` is the stale-RF-after-failed-MCU-switch class
/// (THINK P1). The raw RF regs are logged so a `tx=0` (no match) is debuggable.
pub(super) fn log_channel_triple(dev: &mut Mt7601uDev, beacon_ds: u8, tuned: u8) {
    let r = [
        dev.rf_rr(0, 17),
        dev.rf_rr(0, 18),
        dev.rf_rr(0, 19),
        dev.rf_rr(0, 20),
    ];
    let mut tx = 0u8; // 0 = RF regs match no channel's freq plan (stale/garbage)
    for (i, fp) in super::initvals_phy::FREQ_PLAN.iter().enumerate() {
        if fp[0] == r[0] && fp[1] == r[1] && fp[2] == r[2] && fp[3] == r[3] {
            tx = (i + 1) as u8;
            break;
        }
    }
    let aligned = tx != 0 && beacon_ds == tuned && tuned == tx;
    crate::serial_println!(
        "mt7601u: CHANNEL-TRIPLE beacon_ds={} tuned={} tx={} rf[17..20]=[{:#04x},{:#04x},{:#04x},{:#04x}] {}",
        beacon_ds,
        tuned,
        tx,
        r[0],
        r[1],
        r[2],
        r[3],
        if aligned {
            "OK(aligned)"
        } else {
            "DIVERGENCE -> wrong-channel TX"
        },
    );
    crate::log::flush();
}

/// phy.c:271 — `mt7601u_set_bw_filter` (MCU CAL_BW, TX then RX). Scan is BW20,
/// so the `0x100` bw bit is never set.
fn set_bw_filter(
    dev: &mut Mt7601uDev,
    cal: bool,
    trace_commands: bool,
) -> Result<(), &'static str> {
    let mut filter = 0u32;
    if !cal {
        filter |= 0x10000;
    }
    if trace_commands {
        crate::serial_println!(
            "mt7601u: MCU-CMD before op=CAL_BW phase=tx val={:#x}",
            filter | 1
        );
    }
    dev.mcu_calibrate(MCU_CAL_BW, filter | 1)?; // TX
    if trace_commands {
        crate::serial_println!("mt7601u: MCU-CMD after op=CAL_BW phase=tx result=OK");
        crate::serial_println!(
            "mt7601u: MCU-CMD before op=CAL_BW phase=rx val={:#x}",
            filter
        );
    }
    dev.mcu_calibrate(MCU_CAL_BW, filter)?; // RX
    if trace_commands {
        crate::serial_println!("mt7601u: MCU-CMD after op=CAL_BW phase=rx result=OK");
    }
    Ok(())
}

/// Deferred operating-channel BW calibration. Survey hops and the first
/// targeted tune omit this MCU round-trip; association calls it once the AP
/// accepted the join on the selected operating channel.
pub(super) fn calibrate_bw(dev: &mut Mt7601uDev) -> Result<(), &'static str> {
    set_bw_filter(dev, false, true)
}

/// phy.c:1180 — `mt7601u_bbp_set_bw`. When the bandwidth is UNCHANGED (the scan
/// case — always BW20), the faithful driver only touches BBP R4 and returns; it
/// does NOT stop/restart the MAC or reload the BBP table. Doing the full path on
/// every channel hop (the prior bug) churned the MAC and destabilized the chip
/// by ch6 on silicon — so this early-return is the fix, not just an optimization.
fn bbp_set_bw(dev: &mut Mt7601uDev, bw: u8) -> Result<(), &'static str> {
    if bw == dev.bw {
        dev.bbp_rmw(4, 0x18, if bw == regs::BW_20 { 0 } else { 0x10 });
        return Ok(());
    }
    dev.bw = bw;
    // Bandwidth actually changed: stop MAC, set the bits, restart, load table.
    let old = dev.rr(regs::MAC_SYS_CTRL);
    let val = old & !(regs::MAC_SYS_CTRL_ENABLE_TX | regs::MAC_SYS_CTRL_ENABLE_RX);
    dev.wr(regs::MAC_SYS_CTRL, val)?;
    dev.poll(
        regs::MAC_STATUS,
        regs::MAC_STATUS_TX | regs::MAC_STATUS_RX,
        0,
        500_000,
    );
    dev.bbp_rmw(4, 0x18, if bw == regs::BW_20 { 0 } else { 0x10 });
    dev.wr(regs::MAC_SYS_CTRL, old)?;
    // load_bbp_temp_table_bw: bbp_mode_table[NORMAL][BW20].
    dev.mcu_write_reg_pairs(
        regs::MCU_MEMMAP_BBP,
        super::initvals_phy::BBP_NORMAL_TEMP_BW20,
    )
}

/// phy.c:548 — `mt7601u_rxdc_cal` (RX DC-offset calibration via BBP 158/159).
fn rxdc_cal(dev: &mut Mt7601uDev) -> Result<(), &'static str> {
    let mac_ctrl = dev.rr(regs::MAC_SYS_CTRL);
    dev.wr(regs::MAC_SYS_CTRL, regs::MAC_SYS_CTRL_ENABLE_RX)?;
    dev.mcu_write_reg_pairs(
        regs::MCU_MEMMAP_BBP,
        &[(158, 0x8d), (159, 0xfc), (158, 0x8c), (159, 0x4c)],
    )?;
    for _ in 0..20 {
        udelay(400);
        dev.bbp_wr(158, 0x8c)?;
        if dev.bbp_rr(159) == 0x0c {
            break;
        }
    }
    dev.wr(regs::MAC_SYS_CTRL, 0)?;
    dev.mcu_write_reg_pairs(regs::MCU_MEMMAP_BBP, &[(158, 0x8d), (159, 0xe0)])?;
    dev.wr(regs::MAC_SYS_CTRL, mac_ctrl)?;
    Ok(())
}

/// phy.c:484 — `mt7601u_bbp_r47_get`: read a value through the BBP R47/R49 pair.
/// `reg` is the caller's cached R47 value; the low 3 bits select the source.
fn bbp_r47_get(dev: &mut Mt7601uDev, reg: u8, flag: u8) -> u8 {
    let f = flag | (reg & !BBP_R47_FLAG);
    let _ = dev.bbp_wr(47, f);
    udelay(600); // usleep_range(500, 700)
    dev.bbp_rr(49)
}

/// phy.c:492 — `mt7601u_read_bootup_temp`: sample the chip's boot temperature via
/// the BBP R47/R49 temperature path (saving/restoring RF_SETTING_0 / RF_BYPASS_0).
/// W1 omitted this, so DPD ran at temp 0; the real value linearizes the PA.
fn read_bootup_temp(dev: &mut Mt7601uDev) -> Result<i8, &'static str> {
    let rf_set = dev.rr(regs::RF_SETTING_0);
    let rf_bp = dev.rr(regs::RF_BYPASS_0);

    dev.wr(regs::RF_BYPASS_0, 0)?;
    dev.wr(regs::RF_SETTING_0, 0x10)?;
    dev.wr(regs::RF_BYPASS_0, 0x10)?;

    let mut bbp_val = dev.bbp_rmw(47, 0, 0x10);
    dev.bbp_wr(22, 0x40)?;
    let mut i = 100;
    while i > 0 && (bbp_val & 0x10) != 0 {
        bbp_val = dev.bbp_rr(47);
        i -= 1;
    }
    let temp = bbp_r47_get(dev, bbp_val, BBP_R47_F_TEMP);
    dev.bbp_wr(22, 0)?;

    let mut v = dev.bbp_rr(21);
    v |= 0x02;
    dev.bbp_wr(21, v)?;
    v &= !0x02;
    dev.bbp_wr(21, v)?;

    dev.wr(regs::RF_BYPASS_0, 0)?;
    dev.wr(regs::RF_SETTING_0, rf_set)?;
    dev.wr(regs::RF_BYPASS_0, rf_bp)?;
    Ok(temp as i8)
}

/// phy.c:1122 — `mt7601u_init_cal`. The DPD calibration runs at the REAL chip
/// temperature (`read_bootup_temp`), not 0 — the W1 shortcut mis-linearized the PA.
fn init_cal(dev: &mut Mt7601uDev) -> Result<(), &'static str> {
    // Bootup temperature → DPD temp (phy.c:1127). The DPD PA-linearization MUST run
    // at the real chip temperature; W1 passed 0, mis-linearizing the PA so the TX
    // signal was distorted and the AP could not decode it (silicon tx_stat
    // success=false). dpd_temp = (raw_temp − ref_temp) * MT_EE_TEMPERATURE_SLOPE.
    let raw_temp = read_bootup_temp(dev)? as i32;
    let dpd_temp = ((raw_temp - dev.ee.ref_temp as i32) * MT_EE_TEMPERATURE_SLOPE) as u32;
    // Stash for the assoc dispatch's sink discriminator (this runs pre-sink).
    CAL_RAW_TEMP.store(raw_temp, core::sync::atomic::Ordering::Relaxed);
    CAL_DPD_TEMP.store(dpd_temp, core::sync::atomic::Ordering::Relaxed);
    crate::serial_println!(
        "mt7601u: dpd cal — raw_temp={} ref_temp={} dpd_temp={:#x}",
        raw_temp,
        dev.ee.ref_temp,
        dpd_temp,
    );

    let mac_ctrl = dev.rr(regs::MAC_SYS_CTRL);

    dev.mcu_calibrate(MCU_CAL_R, 0)?;

    let v = dev.rf_rr(0, 4) | 0x80;
    dev.rf_wr(0, 4, v)?;
    msleep(2);

    dev.mcu_calibrate(MCU_CAL_TXDCOC, 0)?;
    rxdc_cal(dev)?;
    set_bw_filter(dev, true, false)?;
    dev.mcu_calibrate(MCU_CAL_LOFT, 0)?;
    dev.mcu_calibrate(MCU_CAL_TXIQ, 0)?;
    dev.mcu_calibrate(MCU_CAL_RXIQ, 0)?;
    dev.mcu_calibrate(MCU_CAL_DPD, dpd_temp)?; // real chip temp (phy.c init_cal)
    rxdc_cal(dev)?;

    dev.wr(regs::MAC_SYS_CTRL, mac_ctrl)?;
    Ok(())
}

/// eeprom.c `mt7601u_config_tx_power_per_rate` + `mt7601u_extra_power_over_mac`.
/// Writes the EEPROM per-rate power words verbatim to TX_PWR_CFG_0..4 (CCK/OFDM/HT
/// power; CCK 1/2 Mbps — our mgmt-frame rate — lives in _0), then derives the
/// extended-rate registers _7/_9. W1 (RX scan) omitted this entirely, leaving the
/// per-rate power at hardware defaults — the real cause of the AP never hearing our
/// auth/assoc frames. Must run before the first `set_channel`.
fn config_tx_power_per_rate(dev: &mut Mt7601uDev) -> Result<(), &'static str> {
    let mut cfg = dev.ee.tx_pwr_cfg;
    // Safe fallback (dispatch hard-req #2): the connect-critical word is _0
    // (CCK[0,1] + OFDM[0,1] — our 1/2 Mbps management-frame rate). A blank clone
    // can read it all-zero, which targets ~0 dBd per rate and leaves the
    // transmitter too weak for the AP to hear (the W1/Stage-B `directed_to_us=0`
    // class of failure). Trust a populated EEPROM (this dongle's byrate0 is sane);
    // only when _0 is all-zero substitute a conservative floor at
    // MT7601U_DEFAULT_TX_POWER across all four per-rate lanes, and log it. (The
    // all-ones case is handled per-word below as upstream's `if (~val)` invalid.)
    if cfg[0] == 0 {
        let floor = super::eeprom_logic::rate_power_floor_word(regs::EE_DEFAULT_TX_POWER as i32);
        crate::serial_println!(
            "mt7601u: per-rate EEPROM word _0 blank (all-zero) — using conservative floor {:#010x} (default tx power {})",
            floor,
            regs::EE_DEFAULT_TX_POWER,
        );
        cfg[0] = floor;
    }
    let bases = [
        regs::TX_PWR_CFG_0,
        regs::TX_PWR_CFG_1,
        regs::TX_PWR_CFG_2,
        regs::TX_PWR_CFG_3,
        regs::TX_PWR_CFG_4,
    ];
    for (i, &base) in bases.iter().enumerate() {
        // upstream `if (~val)`: write only when the EEPROM word is valid (not all-ones).
        if cfg[i] != 0xffff_ffff {
            dev.wr(base, cfg[i])?;
        }
    }
    // extra_power_over_mac: fold _1/_2/_4 high bytes into _7/_9 (extended rates).
    let v1 = dev.rr(regs::TX_PWR_CFG_1);
    let v2 = dev.rr(regs::TX_PWR_CFG_2);
    dev.wr(
        regs::TX_PWR_CFG_7,
        ((v1 & 0x0000_ff00) >> 8) | ((v2 & 0x0000_ff00) << 8),
    )?;
    let v4 = dev.rr(regs::TX_PWR_CFG_4);
    dev.wr(regs::TX_PWR_CFG_9, (v4 & 0x0000_ff00) >> 8)?;
    Ok(())
}

/// phy.c:1226 — `mt7601u_phy_init` + the init.c tail (`set_rx_path`/
/// `set_tx_dac`/`mac_set_ctrlch`/`bbp_set_ctrlch`/`bbp_set_bw`).
pub fn phy_init(dev: &mut Mt7601uDev) -> Result<(), &'static str> {
    // Read the PA-mode regs for parity (the C caches them for TX power).
    let _pa0 = dev.rr(regs::RF_PA_MODE_CFG0);
    let _pa1 = dev.rr(regs::RF_PA_MODE_CFG1);

    dev.rf_wr(0, 12, dev.ee.rf_freq_off)?;
    dev.mcu_write_reg_pairs(0, super::initvals_phy::RF_CENTRAL)?;
    dev.mcu_write_reg_pairs(0, super::initvals_phy::RF_CHANNEL)?;
    dev.mcu_write_reg_pairs(0, super::initvals_phy::RF_VGA)?;

    init_cal(dev)?;

    // init.c `mt7601u_init_hardware` tail (post phy_init).
    set_rx_path(dev, 0);
    set_tx_dac(dev, 0);
    mac_set_ctrlch(dev, false)?;
    bbp_set_ctrlch(dev, false);
    bbp_set_bw(dev, regs::BW_20)?;
    // Program the EEPROM per-rate TX power (TX_PWR_CFG_0..4) before any set_channel.
    config_tx_power_per_rate(dev)?;
    Ok(())
}

/// phy.c:342 — `__mt7601u_phy_set_channel`, the BW20 (scan) path. `ch` is the
/// 1-based 2.4 GHz channel (1..=14).
pub fn set_channel(
    dev: &mut Mt7601uDev,
    ch: u8,
    path: super::wedge_logic::TunePath,
    inline_cal_bw: bool,
) -> Result<(), ChannelError> {
    if !(1..=14).contains(&ch) {
        return Err(ChannelError {
            phase: "CONFIG",
            reason: "channel out of range",
        });
    }
    let idx = (ch - 1) as usize;

    // RF bank-0 PLL frequency plan (regs 17..20) for this channel.
    let fp = super::initvals_phy::FREQ_PLAN[idx];
    crate::serial_println!(
        "mt7601u: set_channel(ch={}, path={:?}, cal_bw={}) begin; MCU-CMD before op=RF_FREQPLAN_RF17_20_RANDOM_WRITE",
        ch,
        path,
        inline_cal_bw,
    );
    at_channel_phase(
        "RF_FREQPLAN_RF17_20_RANDOM_WRITE",
        dev.mcu_write_reg_pairs(
            regs::MCU_MEMMAP_RF,
            &[
                (17, fp[0] as u32),
                (18, fp[1] as u32),
                (19, fp[2] as u32),
                (20, fp[3] as u32),
            ],
        ),
    )?;
    crate::serial_println!("mt7601u: MCU-CMD after op=RF_FREQPLAN_RF17_20_RANDOM_WRITE result=OK");

    // Per-channel ALC power target = the REAL EEPROM channel power. W1 used a
    // hardcoded 0x0d placeholder (`EE_MAX_VER`), and the W2 stopgap forced 0x1c into
    // BOTH the ALC and the per-rate fields — overdriving the PA into a signal the AP
    // couldn't decode (directed_to_us=0). `eeprom_init` now parses the real per-
    // channel power; write it faithfully (upstream `__mt7601u_phy_set_channel`:
    // rmw(TX_ALC_CFG_0, 0x3f3f, chan_pwr & 0x3f)).
    let chan_pwr = (dev.ee.chan_pwr[idx] as u32) & 0x3f;
    crate::serial_println!(
        "mt7601u: tx_power ch_idx={} chan_pwr={:#04x} byrate0={:#010x}",
        idx,
        chan_pwr,
        dev.ee.tx_pwr_cfg[0],
    );
    at_channel_phase("TX_ALC", dev.rmw(regs::TX_ALC_CFG_0, 0x3f3f, chan_pwr))?;

    // BBP 62/63/64 = 0x37 - lna_gain.
    let bv = (0x37i32 - dev.ee.lna_gain as i32) as u8 as u32;
    crate::serial_println!("mt7601u: MCU-CMD before op=BBP_LNA");
    at_channel_phase(
        "BBP_LNA",
        dev.mcu_write_reg_pairs(regs::MCU_MEMMAP_BBP, &[(62, bv), (63, bv), (64, bv)]),
    )?;
    crate::serial_println!("mt7601u: MCU-CMD after op=BBP_LNA result=OK");

    crate::serial_println!("mt7601u: RF-CSR before op=VCO_CAL");
    at_channel_phase("VCO_CAL", vco_cal(dev))?;
    crate::serial_println!("mt7601u: RF-CSR after op=VCO_CAL result=OK");
    crate::serial_println!("mt7601u: BBP before op=SET_BW20");
    at_channel_phase("SET_BW20", bbp_set_bw(dev, regs::BW_20))?;
    crate::serial_println!("mt7601u: BBP after op=SET_BW20 result=OK");
    if inline_cal_bw {
        at_channel_phase("CAL_BW", set_bw_filter(dev, false, true))?;
    } else {
        crate::serial_println!(
            "mt7601u: MCU-CMD op=CAL_BW result=SKIP reason={}",
            match path {
                super::wedge_logic::TunePath::Survey => "disabled",
                super::wedge_logic::TunePath::Targeted => "first_targeted_tune",
            }
        );
    }
    // Re-assert the per-rate TX power for this channel (BW20). Upstream
    // `__mt7601u_phy_set_channel` writes int_to_s6(power_rate_table.bw20) here; for
    // BW20 that reconstructs exactly the EEPROM byrate-0 word that
    // `config_tx_power_per_rate` (phy_init) already programmed into TX_PWR_CFG_0.
    // Re-write it so an intervening BBP/MAC table load can't leave it stale. The
    // forced-0x1c stopgap is GONE — CCK[0,1]/OFDM[0,1] power now comes from the
    // EEPROM, which is what lets the AP actually hear us (matches Linux at 500 mA).
    // Same blank-EEPROM floor as `config_tx_power_per_rate`: an all-zero _0 word
    // would re-assert ~0 power here, so substitute the conservative floor.
    let pwr_cfg0 = if dev.ee.tx_pwr_cfg[0] == 0 {
        super::eeprom_logic::rate_power_floor_word(regs::EE_DEFAULT_TX_POWER as i32)
    } else {
        dev.ee.tx_pwr_cfg[0]
    };
    if pwr_cfg0 != 0xffff_ffff {
        at_channel_phase("TX_POWER", dev.wr(regs::TX_PWR_CFG_0, pwr_cfg0))?;
    }
    crate::serial_println!(
        "mt7601u: set_channel(ch={}, path={:?}, cal_bw={}) OK",
        ch,
        path,
        inline_cal_bw
    );
    Ok(())
}
