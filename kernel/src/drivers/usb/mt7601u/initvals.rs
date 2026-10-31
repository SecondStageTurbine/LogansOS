// SPDX-License-Identifier: MPL-2.0
//! MT7601U MAC/BBP init-value tables (Phase 7.5 BUILD-W1) — transcribed
//! byte-faithfully from the Linux `mt7601u` driver `initvals.h` (v6.6,
//! GPL-2.0). Each `(reg, val)` is written via MCU RANDOM_WRITE to the WLAN
//! (MAC) or BBP memory map (`mod.rs::write_mac_initvals`/`init_bbp`). These are
//! device-programming facts copied verbatim; the comments preserve the upstream
//! `MT_*` register names so the transcription is auditable against the header.

#![allow(dead_code)]

/// `mac_common_vals[]` — written to `MT_MCU_MEMMAP_WLAN`. `(MT_* offset, val)`.
pub const MAC_COMMON_VALS: &[(u32, u32)] = &[
    (0x1408, 0x0000_013f), // MT_LEGACY_BASIC_RATE
    (0x140c, 0x0000_8003), // MT_HT_BASIC_RATE
    (0x1004, 0x0000_0000), // MT_MAC_SYS_CTRL
    (0x1400, 0x0001_7f97), // MT_RX_FILTR_CFG
    (0x1104, 0x0000_0209), // MT_BKOFF_SLOT_CFG
    (0x1330, 0x0000_0000), // MT_TX_SW_CFG0
    (0x1334, 0x0008_0606), // MT_TX_SW_CFG1
    (0x1350, 0x0000_1020), // MT_TX_LINK_CFG
    (0x1348, 0x000a_2090), // MT_TX_TIMEOUT_CFG
    (0x1018, 0x0000_3fff), // MT_MAX_LEN_CFG
    (0x0408, 0x1fbf_1f1f), // MT_PBF_TX_MAX_PCNT
    (0x040c, 0x0000_009f), // MT_PBF_RX_MAX_PCNT
    (0x134c, 0x47d0_1f0f), // MT_TX_RETRY_CFG
    (0x1404, 0x0000_0013), // MT_AUTO_RSP_CFG
    (0x1364, 0x0574_0003), // MT_CCK_PROT_CFG
    (0x1368, 0x0574_0003), // MT_OFDM_PROT_CFG
    (0x1370, 0x03f4_4084), // MT_MM40_PROT_CFG
    (0x1374, 0x0174_4004), // MT_GF20_PROT_CFG
    (0x1378, 0x03f4_4084), // MT_GF40_PROT_CFG
    (0x136c, 0x0174_4004), // MT_MM20_PROT_CFG
    (0x1340, 0x0000_583f), // MT_TXOP_CTRL_CFG
    (0x1344, 0x0109_2b20), // MT_TX_RTS_CFG
    (0x1380, 0x0024_00ca), // MT_EXP_ACK_TIME
    (0x1608, 0x0000_0002), // MT_TXOP_HLDR_ET
    (0x1100, 0x33a4_1010), // MT_XIFS_TIME_CFG
    (0x1204, 0x0000_0000), // MT_PWR_PIN_CFG
    (0x150c, 0x0000_0001), // MT_PN_PAD_MODE
];

/// `mac_chip_vals[]` — written to `MT_MCU_MEMMAP_WLAN`.
pub const MAC_CHIP_VALS: &[(u32, u32)] = &[
    (0x0250, 0x0000_6050), // MT_TSO_CTRL
    (0x041c, 0x1810_0800), // MT_BCN_OFFSET(0)
    (0x0420, 0x3830_2820), // MT_BCN_OFFSET(1)
    (0x0400, 0x0008_0c00), // MT_PBF_SYS_CTRL
    (0x0404, 0x7f72_3c1f), // MT_PBF_CFG
    (0x0800, 0x0000_0001), // MT_FCE_PSE_CTRL
    (0x0a38, 0x0000_0000), // MT_PAUSE_ENABLE_CONTROL1
    (0x13a0, 0x003b_0005), // MT_TX0_RF_GAIN_CORR
    (0x13a8, 0x0000_6900), // MT_TX0_RF_GAIN_ATTEN
    (0x13c0, 0x0000_0400), // MT_TX0_BB_GAIN_ATTEN
    (0x13c8, 0x0006_0006), // MT_TX_ALC_VGA3
    (0x1330, 0x0000_0402), // MT_TX_SW_CFG0
    (0x1334, 0x0000_0000), // MT_TX_SW_CFG1
    (0x1338, 0x0000_0000), // MT_TX_SW_CFG2
    (0x0260, 0x0000_0000), // MT_HEADER_TRANS_CTRL_REG
    (0x0808, 0x0000_030f), // MT_FCE_CSO
    (0x0804, 0x0025_6f0f), // MT_FCE_PARAMETERS
];

/// `bbp_common_vals[]` — written to `MT_MCU_MEMMAP_BBP`. `(BBP reg #, val)`.
pub const BBP_COMMON_VALS: &[(u32, u32)] = &[
    (65, 0x2c),
    (66, 0x38),
    (68, 0x0b),
    (69, 0x12),
    (70, 0x0a),
    (73, 0x10),
    (81, 0x37),
    (82, 0x62),
    (83, 0x6a),
    (84, 0x99),
    (86, 0x00),
    (91, 0x04),
    (92, 0x00),
    (103, 0x00),
    (105, 0x05),
    (106, 0x35),
];

/// `bbp_chip_vals[]` — written to `MT_MCU_MEMMAP_BBP`. Order is significant
/// (the R195/R196 AGC/GLRT index/value pairs are applied sequentially).
pub const BBP_CHIP_VALS: &[(u32, u32)] = &[
    (1, 0x04),
    (4, 0x40),
    (20, 0x06),
    (31, 0x08),
    // CCK Tx Control
    (178, 0xff),
    // AGC/Sync controls
    (66, 0x14),
    (68, 0x8b),
    (69, 0x12),
    (70, 0x09),
    (73, 0x11),
    (75, 0x60),
    (76, 0x44),
    (84, 0x9a),
    (86, 0x38),
    (91, 0x07),
    (92, 0x02),
    // Rx Path Controls
    (99, 0x50),
    (101, 0x00),
    (103, 0xc0),
    (104, 0x92),
    (105, 0x3c),
    (106, 0x03),
    (128, 0x12),
    // Change RXWI content: Gain Report
    (142, 0x04),
    (143, 0x37),
    // Change RXWI content: Antenna Report
    (142, 0x03),
    (143, 0x99),
    // CCK Receiver Control
    (160, 0xeb),
    (161, 0xc4),
    (162, 0x77),
    (163, 0xf9),
    (164, 0x88),
    (165, 0x80),
    (166, 0xff),
    (167, 0xe4),
    // AGC/GLRT registers via R195 (index) / R196 (value)
    (195, 0x00),
    (196, 0x00),
    (195, 0x01),
    (196, 0x04),
    (195, 0x02),
    (196, 0x20),
    (195, 0x03),
    (196, 0x0a),
    (195, 0x06),
    (196, 0x16),
    (195, 0x07),
    (196, 0x05),
    (195, 0x08),
    (196, 0x37),
    (195, 0x0a),
    (196, 0x15),
    (195, 0x0b),
    (196, 0x17),
    (195, 0x0c),
    (196, 0x06),
    (195, 0x0d),
    (196, 0x09),
    (195, 0x0e),
    (196, 0x05),
    (195, 0x0f),
    (196, 0x09),
    (195, 0x10),
    (196, 0x20),
    (195, 0x20),
    (196, 0x17),
    (195, 0x21),
    (196, 0x06),
    (195, 0x22),
    (196, 0x09),
    (195, 0x23),
    (196, 0x17),
    (195, 0x24),
    (196, 0x06),
    (195, 0x25),
    (196, 0x09),
    (195, 0x26),
    (196, 0x17),
    (195, 0x27),
    (196, 0x06),
    (195, 0x28),
    (196, 0x09),
    (195, 0x29),
    (196, 0x05),
    (195, 0x2a),
    (196, 0x09),
    (195, 0x80),
    (196, 0x8b),
    (195, 0x81),
    (196, 0x12),
    (195, 0x82),
    (196, 0x09),
    (195, 0x83),
    (196, 0x17),
    (195, 0x84),
    (196, 0x11),
    (195, 0x85),
    (196, 0x00),
    (195, 0x86),
    (196, 0x00),
    (195, 0x87),
    (196, 0x18),
    (195, 0x88),
    (196, 0x60),
    (195, 0x89),
    (196, 0x44),
    (195, 0x8a),
    (196, 0x8b),
    (195, 0x8b),
    (196, 0x8b),
    (195, 0x8c),
    (196, 0x8b),
    (195, 0x8d),
    (196, 0x8b),
    (195, 0x8e),
    (196, 0x09),
    (195, 0x8f),
    (196, 0x09),
    (195, 0x90),
    (196, 0x09),
    (195, 0x91),
    (196, 0x09),
    (195, 0x92),
    (196, 0x11),
    (195, 0x93),
    (196, 0x11),
    (195, 0x94),
    (196, 0x11),
    (195, 0x95),
    (196, 0x11),
    // PPAD
    (47, 0x80),
    (60, 0x80),
    (150, 0xd2),
    (151, 0x32),
    (152, 0x23),
    (153, 0x41),
    (154, 0x00),
    (155, 0x4f),
    (253, 0x7e),
    (195, 0x30),
    (196, 0x32),
    (195, 0x31),
    (196, 0x23),
    (195, 0x32),
    (196, 0x45),
    (195, 0x35),
    (196, 0x4a),
    (195, 0x36),
    (196, 0x5a),
    (195, 0x37),
    (196, 0x5a),
];
