// SPDX-License-Identifier: MPL-2.0
//! MT7601U register map + USB vendor-request encodings (Phase 7.5 BUILD-W1,
//! ANSPEC §10.10, ADR-054).
//!
//! Constants transcribed byte-faithfully from the Linux `mt7601u` driver
//! (`regs.h`, `usb.h`, `mcu.h`, `eeprom.h`, `dma.h`, `mac.h`; torvalds/linux
//! v6.6, GPL-2.0). Only the subset the W1 scan bring-up touches is carried —
//! the authoritative map is the upstream header (cited per constant group).
//! These are device facts, not invention; the bring-up SEQUENCE that consumes
//! them is `RESEARCH-2026-06-14-mt7601u-bringup-trap-mine-FINDINGS.md §1`.

#![allow(dead_code)] // a register map: each constant is a documented device fact

// --- USB vendor requests (usb.h `enum mt_vendor_req`) -------------------------
/// `MT_VEND_DEV_MODE` — device-mode control (vendor reset, IVB upload).
pub const VEND_DEV_MODE: u8 = 1;
/// `MT_VEND_WRITE` — single 32-bit register write (split low16/high16).
pub const VEND_WRITE: u8 = 2;
/// `MT_VEND_MULTI_READ` — 32-bit register read (4-byte IN payload).
pub const VEND_MULTI_READ: u8 = 7;
/// `MT_VEND_WRITE_FCE` — FCE register write (firmware-DMA addr/len).
pub const VEND_WRITE_FCE: u8 = 0x42;
/// `MT_VEND_DEV_MODE_RESET` value for the vendor reset.
pub const VEND_DEV_MODE_RESET: u16 = 1;

/// `bmRequestType` for a vendor control transfer (USB 2.0 §9.3):
/// `USB_TYPE_VENDOR(0x40) | USB_RECIP_DEVICE(0x00)`, OR the direction bit.
pub const REQ_TYPE_VENDOR_OUT: u8 = 0x40; // host→device
pub const REQ_TYPE_VENDOR_IN: u8 = 0xC0; // device→host (| USB_DIR_IN 0x80)

// --- Bulk endpoint indices (usb.h `enum mt_usb_ep_in/out`) --------------------
/// IN endpoints, in descriptor order: 0 = packet RX, 1 = MCU command response.
pub const EP_IN_PKT_RX: usize = 0;
pub const EP_IN_CMD_RESP: usize = 1;
pub const N_EP_IN: usize = 2; // `__MT_EP_IN_MAX`
/// OUT endpoints, in descriptor order: 0 = in-band command (firmware + MCU),
/// 1..=4 = AC_BK/BE/VI/VO TX queues, 5 = HCCA.
pub const EP_OUT_INBAND_CMD: usize = 0;
/// W2 ISC-1: the data/mgmt TX queue. AC_BE (out_eps[2]) — the best-effort queue
/// the supplicant's auth/assoc frames and (later) data ride on. The DMA engine
/// maps this to QSEL_EDCA (dma.c `ep2dmaq`: only ep 5 is QSEL_MGMT).
pub const EP_OUT_DATA: usize = 2;
pub const N_EP_OUT: usize = 6; // `__MT_EP_OUT_MAX`

// --- Core / system registers (regs.h) -----------------------------------------
pub const ASIC_VERSION: u32 = 0x0000;
pub const CMB_CTRL: u32 = 0x0020;
pub const CMB_CTRL_XTAL_RDY: u32 = 1 << 22;
pub const CMB_CTRL_PLL_LD: u32 = 1 << 23;
pub const EFUSE_CTRL: u32 = 0x0024;
pub const EFUSE_CTRL_AOUT: u32 = 0x3f; // GENMASK(5,0)
pub const EFUSE_CTRL_MODE_SHIFT: u32 = 6; // GENMASK(7,6)
pub const EFUSE_CTRL_AIN_SHIFT: u32 = 16; // GENMASK(25,16)
pub const EFUSE_CTRL_AIN_MASK: u32 = 0x3ff << 16;
pub const EFUSE_CTRL_MODE_MASK: u32 = 0x3 << 6;
pub const EFUSE_CTRL_KICK: u32 = 1 << 30;
pub const EFUSE_CTRL_SEL: u32 = 1 << 31;
pub const EFUSE_DATA_BASE: u32 = 0x0028;
pub const fn efuse_data(n: u32) -> u32 {
    EFUSE_DATA_BASE + (n << 2)
}

pub const WLAN_FUN_CTRL: u32 = 0x0080;
pub const WLAN_FUN_CTRL_WLAN_EN: u32 = 1 << 0;
pub const WLAN_FUN_CTRL_WLAN_CLK_EN: u32 = 1 << 1;

// --- WPDMA / FCE / USB DMA ----------------------------------------------------
pub const WPDMA_GLO_CFG: u32 = 0x0208;
pub const WPDMA_GLO_CFG_TX_DMA_BUSY: u32 = 1 << 1;
pub const WPDMA_GLO_CFG_RX_DMA_BUSY: u32 = 1 << 3;
pub const FCE_DMA_ADDR: u32 = 0x0230;
pub const FCE_DMA_LEN: u32 = 0x0234;
pub const USB_DMA_CFG: u32 = 0x0238;
pub const USB_DMA_CFG_RX_BULK_EN: u32 = 1 << 22;
pub const USB_DMA_CFG_TX_BULK_EN: u32 = 1 << 23;
pub const USB_DMA_CFG_TX_CLR: u32 = 1 << 19;
pub const USB_DMA_CFG_RX_BULK_AGG_EN: u32 = 1 << 21;
pub const USB_DMA_CFG_UDMA_RX_WL_DROP: u32 = 1 << 25;
pub const US_CYC_CFG: u32 = 0x02a4;
pub const US_CYC_CNT: u32 = 0xff; // GENMASK(7,0)

pub const PBF_CFG: u32 = 0x0404;
pub const PBF_CFG_TX0Q_EN: u32 = 1 << 0;
pub const PBF_CFG_TX1Q_EN: u32 = 1 << 1;
pub const PBF_CFG_TX2Q_EN: u32 = 1 << 2;
pub const PBF_CFG_TX3Q_EN: u32 = 1 << 3;

pub const RF_CSR_CFG: u32 = 0x0500;
pub const RF_CSR_CFG_WR: u32 = 1 << 30;
pub const RF_CSR_CFG_KICK: u32 = 1 << 31;
pub const RF_BYPASS_0: u32 = 0x0504;
pub const RF_SETTING_0: u32 = 0x050c;

// BBP CSR access (phy.c `mt7601u_bbp_wr`/`mt7601u_bbp_rr`).
pub const BBP_CSR_CFG: u32 = 0x101c;
pub const BBP_CSR_CFG_BUSY: u32 = 1 << 17;
pub const BBP_CSR_CFG_READ: u32 = 1 << 16;
pub const BBP_CSR_CFG_RW_MODE: u32 = 1 << 19;
/// BBP R0 — the version register polled by `wait_bbp_ready`.
pub const BBP_REG_VERSION: u8 = 0x00;

// PHY / TX-power registers (phy.c set_channel + phy_init).
pub const TX_ALC_CFG_0: u32 = 0x13b0;
/// `MT_TX_ALC_CFG_0_LIMIT_0` (GENMASK(21,16)): the per-channel ALC power ceiling
/// the EEPROM per-channel power is validated against (eeprom.c set_channel_power).
pub const TX_ALC_CFG_0_LIMIT_0_SHIFT: u32 = 16;
pub const TX_ALC_CFG_0_LIMIT_0_MASK: u32 = 0x3f;
// Per-rate TX power (phy.c/eeprom.c): TX_PWR_CFG_0..4 hold CCK/OFDM/HT power
// (CCK 1/2 Mbps — our mgmt-frame rate — lives in _0); _7/_9 hold extended rates.
pub const TX_PWR_CFG_0: u32 = 0x1314;
pub const TX_PWR_CFG_1: u32 = 0x1318;
pub const TX_PWR_CFG_2: u32 = 0x131c;
pub const TX_PWR_CFG_3: u32 = 0x1320;
pub const TX_PWR_CFG_4: u32 = 0x1324;
pub const TX_PWR_CFG_7: u32 = 0x13d4;
pub const TX_PWR_CFG_9: u32 = 0x13dc;
/// mac.c `mt7601u_mac_fetch_tx_status` — per-frame HW TX result FIFO (0x1718).
/// Populated for a frame whose TXWI carries a nonzero packet-id. VALID=entry
/// present; SUCCESS=transmitted AND ACKed by the peer; ACKREQ=ACK was requested.
pub const TX_STAT_FIFO: u32 = 0x1718;
pub const TX_STAT_FIFO_VALID: u32 = 1 << 0;
pub const TX_STAT_FIFO_SUCCESS: u32 = 1 << 5;
pub const TX_STAT_FIFO_ACKREQ: u32 = 1 << 7;
pub const TX_BAND_CFG: u32 = 0x132c;
pub const TX_BAND_CFG_UPPER_40M: u32 = 1 << 0;
pub const RF_PA_MODE_CFG0: u32 = 0x121c;
pub const RF_PA_MODE_CFG1: u32 = 0x1220;
/// MCU RF memory map (mcu.h) — the RF reg-pair address base.
pub const MCU_MEMMAP_RF: u32 = 0x80000000;

// Channel bandwidth + temperature mode (mt7601u.h enums).
pub const BW_20: u8 = 0;
pub const BW_40: u8 = 1;
pub const TEMP_MODE_NORMAL: u8 = 0;

pub const FCE_PSE_CTRL: u32 = 0x0800;
pub const TX_CPU_FROM_FCE_BASE_PTR: u32 = 0x09a0;
pub const TX_CPU_FROM_FCE_MAX_COUNT: u32 = 0x09a4;
pub const TX_CPU_FROM_FCE_CPU_DESC_IDX: u32 = 0x09a8;
pub const FCE_PDMA_GLOBAL_CONF: u32 = 0x09c4;
pub const FCE_SKIP_FS: u32 = 0x0a6c;

// --- MAC registers ------------------------------------------------------------
pub const MAC_CSR0: u32 = 0x1000;
pub const MAC_SYS_CTRL: u32 = 0x1004;
pub const MAC_SYS_CTRL_RESET_CSR: u32 = 1 << 0;
pub const MAC_SYS_CTRL_RESET_BBP: u32 = 1 << 1;
pub const MAC_SYS_CTRL_ENABLE_TX: u32 = 1 << 2;
pub const MAC_SYS_CTRL_ENABLE_RX: u32 = 1 << 3;
pub const MAC_ADDR_DW0: u32 = 0x1008;
pub const MAC_ADDR_DW1: u32 = 0x100c;
/// MT_MAC_ADDR_DW1_U2ME_MASK = GENMASK(23,16): the "unicast-to-me" address-match
/// mask. Linux's `mt7601u_set_macaddr` programs it to 0xff so the HW recognises
/// directed unicast frames addressed to our MAC (without it, directed frames —
/// e.g. the AP's auth/assoc response — are dropped while broadcast beacons pass).
pub const MAC_ADDR_DW1_U2ME_MASK: u32 = 0x00ff_0000;
/// MT_MAC_BSSID_DW0/DW1 (0x1010/0x1014): the joined BSSID. `mt7601u_bss_info_changed`
/// writes these on `BSS_CHANGED_BSSID` so the HW filters/ACKs frames for this BSS.
pub const MAC_BSSID_DW0: u32 = 0x1010;
pub const MAC_BSSID_DW1: u32 = 0x1014;
pub const MAC_STATUS: u32 = 0x1200;
pub const MAC_STATUS_TX: u32 = 1 << 0;
pub const MAC_STATUS_RX: u32 = 1 << 1;
pub const AUX_CLK_CFG: u32 = 0x120c;
pub const TXOP_CTRL_CFG: u32 = 0x1340;
pub const BEACON_TIME_CFG: u32 = 0x1114;
pub const BEACON_TIME_CFG_TIMER_EN: u32 = 1 << 16;
pub const BEACON_TIME_CFG_SYNC_MODE: u32 = 0x3 << 17;
pub const BEACON_TIME_CFG_TBTT_EN: u32 = 1 << 19;
pub const BEACON_TIME_CFG_BEACON_TX: u32 = 1 << 20;

pub const RX_FILTR_CFG: u32 = 0x1400;
pub const RX_FILTR_CFG_CRC_ERR: u32 = 1 << 0;
pub const RX_FILTR_CFG_PHY_ERR: u32 = 1 << 1;
pub const RX_FILTR_CFG_PROMISC: u32 = 1 << 2;
pub const RX_FILTR_CFG_VER_ERR: u32 = 1 << 4;
pub const RX_FILTR_CFG_DUP: u32 = 1 << 7;
pub const RX_FILTR_CFG_CFACK: u32 = 1 << 8;
pub const RX_FILTR_CFG_CFEND: u32 = 1 << 9;
pub const RX_FILTR_CFG_ACK: u32 = 1 << 10;
pub const RX_FILTR_CFG_CTS: u32 = 1 << 11;
pub const RX_FILTR_CFG_RTS: u32 = 1 << 12;
pub const RX_FILTR_CFG_PSPOLL: u32 = 1 << 13;
pub const RX_FILTR_CFG_BA: u32 = 1 << 14;
pub const RX_FILTR_CFG_CTRL_RSV: u32 = 1 << 16;

pub const WCID_ADDR_BASE: u32 = 0x1800;
pub const SKEY_MODE_BASE_0: u32 = 0xb000;
pub const WCID_ATTR_BASE: u32 = 0xa800;
pub const BEACON_BASE: u32 = 0xc000;

// --- MCU registers + memory maps (mcu.h) --------------------------------------
pub const MCU_COM_REG0: u32 = 0x0730;
pub const MCU_COM_REG1: u32 = 0x0734;
pub const MCU_IVB_SIZE: usize = 0x40; // 64
pub const MCU_DLM_OFFSET: u32 = 0x80000;
pub const MCU_MEMMAP_WLAN: u32 = 0x00410000;
pub const MCU_MEMMAP_BBP: u32 = 0x40000000;
pub const INBAND_PACKET_MAX_LEN: usize = 192;

// --- DMA descriptor fields (dma.h) --------------------------------------------
pub const DMA_HDR_LEN: usize = 4; // `MT_DMA_HDR_LEN`
pub const RX_INFO_LEN: usize = 4; // `MT_RX_INFO_LEN`
pub const FCE_INFO_LEN: usize = 4; // `MT_FCE_INFO_LEN`
pub const DMA_HDRS: usize = DMA_HDR_LEN + RX_INFO_LEN; // 8

/// `enum mt76_msg_port` values used by the bring-up.
pub const WLAN_PORT: u32 = 0;
pub const CPU_TX_PORT: u32 = 2;
/// `enum mt76_info_type`.
pub const DMA_PACKET: u32 = 0;
pub const DMA_COMMAND: u32 = 1;

// TXINFO/CMD field shifts (dma.h `MT_TXD_*`).
pub const TXD_INFO_LEN_MASK: u32 = 0xffff; // GENMASK(15,0)
pub const TXD_INFO_D_PORT_SHIFT: u32 = 27; // GENMASK(29,27)
pub const TXD_INFO_TYPE_SHIFT: u32 = 30; // GENMASK(31,30)
pub const TXD_CMD_INFO_SEQ_SHIFT: u32 = 16; // GENMASK(19,16)
pub const TXD_CMD_INFO_TYPE_SHIFT: u32 = 20; // GENMASK(26,20)
/// RX command-info fields (`MT_RXD_CMD_INFO_*`) for MCU responses.
pub const RXD_CMD_INFO_CMD_SEQ_SHIFT: u32 = 16; // GENMASK(19,16)
pub const RXD_CMD_INFO_CMD_SEQ_MASK: u32 = 0xf << 16;

// ── W2 ISC-2: WCID hardware key table (CCMP offload; regs.h:572-625) ─────────
// Per-WCID register blocks; the handshake installs PTK/GTK here and the firmware
// does CCMP on the data path. Key install is plain register writes (FINDINGS §4).
pub const WCID_KEY_BASE: u32 = 0x8000; // 32 B / wcid (the key material)
pub const WCID_IV_BASE: u32 = 0xa000; // 8 B / wcid (IV/PN state)
// WCID_ATTR_BASE (0xa800) is defined above (W1 MAC-init section).
pub const fn wcid_key(n: u32) -> u32 {
    WCID_KEY_BASE + n * 32
}
pub const fn wcid_iv(n: u32) -> u32 {
    WCID_IV_BASE + n * 8
}
pub const fn wcid_attr(n: u32) -> u32 {
    WCID_ATTR_BASE + n * 4
}
pub const WCID_ATTR_PAIRWISE: u32 = 1 << 0; // BIT(0)
pub const WCID_ATTR_PKEY_MODE_SHIFT: u32 = 1; // GENMASK(3,1)
pub const WCID_ATTR_PKEY_MODE_MASK: u32 = 0x7 << 1;
pub const WCID_ATTR_PKEY_MODE_EXT: u32 = 1 << 10; // BIT(10)

/// `enum mt76_cipher_type` (regs.h:615-625) — the PKEY_MODE value.
pub const CIPHER_NONE: u8 = 0;
pub const CIPHER_TKIP: u8 = 3;
pub const CIPHER_AES_CCMP: u8 = 4;
pub const RXD_CMD_INFO_EVT_TYPE_SHIFT: u32 = 20; // GENMASK(23,20)
pub const RXD_CMD_INFO_EVT_TYPE_MASK: u32 = 0xf << 20;
/// RX packet-info TYPE (`MT_RXD_INFO_TYPE`, GENMASK(31,30)) — 0 ⇒ a data pkt.
pub const RXD_INFO_TYPE_SHIFT: u32 = 30;

/// `enum mt76_evt_type` — `CMD_DONE` is the MCU success event.
pub const EVT_CMD_DONE: u32 = 0;

// --- MCU commands (mcu.h `enum mcu_cmd`) --------------------------------------
pub const CMD_FUN_SET_OP: u8 = 1;
pub const CMD_BURST_WRITE: u8 = 8;
pub const CMD_RANDOM_WRITE: u8 = 12;
pub const CMD_CALIBRATION_OP: u8 = 31;
pub const CMD_SWITCH_CHANNEL_OP: u8 = 30;
/// `enum mcu_function` — `Q_SELECT` is the post-firmware function select.
pub const FUNC_Q_SELECT: u32 = 1;

// --- Firmware (mcu.c) ---------------------------------------------------------
/// `MCU_FW_URB_MAX_PAYLOAD` — the firmware-chunk ceiling (14,336 bytes).
pub const MCU_FW_URB_MAX_PAYLOAD: usize = 0x3800;
/// The IVB upload's vendor-request `wValue` (mcu.c: `MT_VEND_DEV_MODE`, 0x12).
pub const FW_IVB_DEV_MODE_VALUE: u16 = 0x12;
pub const FW_HEADER_LEN: usize = 32; // sizeof(struct mt76_fw_header)

// --- eFUSE / EEPROM (eeprom.h) ------------------------------------------------
pub const EE_VERSION_FAE: usize = 0x02;
pub const EE_VERSION_EE: usize = 0x03;
pub const EE_MAC_ADDR: usize = 0x04;
pub const EE_NIC_CONF_0: usize = 0x34;
pub const EE_NIC_CONF_1: usize = 0x36;
pub const EE_COUNTRY_REGION: usize = 0x39;
pub const EE_FREQ_OFFSET: usize = 0x3a;
/// eeprom.h `MT_EE_TX_POWER_OFFSET`: 14 per-channel TX power bytes (0x52..0x5f).
pub const EE_TX_POWER_OFFSET: usize = 0x52;
/// eeprom.h `MT_EE_TX_POWER_BYRATE_BASE`: 5 little-endian u32 per-rate power
/// words, written verbatim to TX_PWR_CFG_0..4 (eeprom.c config_tx_power_per_rate).
pub const EE_TX_POWER_BYRATE_BASE: usize = 0xde;
/// eeprom.h `MT7601U_DEFAULT_TX_POWER` — the fallback when a per-channel EEPROM
/// power byte is invalid/out-of-range.
pub const EE_DEFAULT_TX_POWER: u8 = 6;
/// eeprom.h `MT_EE_TX_TSSI_TARGET_POWER` — when TSSI is enabled, the per-channel
/// ALC target is this (the closed loop's target), NOT the 0x52 per-channel value.
pub const EE_TX_TSSI_TARGET_POWER: usize = 0xd0;
/// eeprom.h nic_conf1 (0x36) bits: `TX_ALC_EN` gates TSSI; `TEMP_TX_ALC` = temp-comp.
pub const EE_NIC_CONF_1_TX_ALC_EN: u16 = 1 << 13;
pub const EE_NIC_CONF_1_TEMP_TX_ALC: u16 = 1 << 1;
/// eeprom.c `mt7601u_set_channel_target_power` default when 0xd0 is invalid.
pub const EE_TSSI_TARGET_DEFAULT: u8 = 0x20;
pub const EE_USAGE_MAP_START: usize = 0x1e0;
pub const EE_USAGE_MAP_END: usize = 0x1fc;
pub const EFUSE_USAGE_MAP_SIZE: usize = EE_USAGE_MAP_END - EE_USAGE_MAP_START + 1; // 29
pub const EEPROM_SIZE: usize = 256;
pub const EE_MAX_VER: u8 = 0x0d;
/// eFUSE read modes (`enum mt7601u_eeprom_access_modes`).
pub const EE_READ: u32 = 0;
pub const EE_PHYSICAL_READ: u32 = 1;

// --- Known-good discriminator targets for THIS adapter (DISPATCH §2) ----------
/// USB id of the confirmed dongle (`148f:7601`) + the upstream clone table.
pub const VID_RALINK: u16 = 0x148f;
pub const PID_MT7601U: u16 = 0x7601;
/// `(asic_rev >> 16)` must equal this (usb.c probe check).
pub const ASIC_REV_MT7601: u16 = 0x7601;
