// SPDX-License-Identifier: MPL-2.0
//! Pure logic for the Realtek RTL8125 2.5GbE driver (rung 1).
//!
//! Kernel-free so the host crate `tools/rtl8125-logic-tests` can test it
//! against hand-laid byte oracles and a register model: the XID decode, the
//! descriptor format, the TX/RX ring state machines, the PHY status decode, the
//! OCP access encoding, the MAC-address choice, the interrupt-storm detector
//! and the DMA-containment decision table. The kernel side
//! (`rtl8125.rs`) only does MMIO and PCI config around these.
//!
//! Sources, read for facts only (no code copied): OpenBSD `rge(4)`
//! (`if_rge.c`/`if_rgereg.h`, ISC) and Linux `r8169_main.c` (GPL, principle
//! and traps only). Where the two disagree the choice is noted.

/// MMIO register offsets (BAR2, 64 KiB on RTL8125). Byte offsets.
pub mod reg {
    /// Station address bytes 0..3 (programmed by firmware/our driver).
    pub const MAC0: usize = 0x00;
    /// Station address bytes 4..5.
    pub const MAC4: usize = 0x04;
    /// Multicast hash filter, 64 bits.
    pub const MAR0: usize = 0x08;
    pub const MAR4: usize = 0x0c;
    /// TX descriptor ring base (normal priority queue), low/high dwords.
    pub const TX_DESC_LO: usize = 0x20;
    pub const TX_DESC_HI: usize = 0x24;
    /// RTL8125 interrupt config (8-bit). Written 0 for the legacy single vector.
    pub const INT_CFG0: usize = 0x34;
    /// Chip command (8-bit): reset, TX/RX enable, stop request.
    pub const CMD: usize = 0x37;
    /// RTL8125 interrupt mask, 32-bit. The 8168 had a 16-bit IMR at 0x3c;
    /// the 8125 moved it here and widened both (dispatch trap 2).
    pub const IMR: usize = 0x38;
    /// RTL8125 interrupt status, 32-bit, write-1-to-clear.
    pub const ISR: usize = 0x3c;
    /// TX config. Bits 30..20 (masked 0x7cf) carry the XID.
    pub const TX_CONFIG: usize = 0x40;
    /// RX config: accept filters, DMA burst, fetch count.
    pub const RX_CONFIG: usize = 0x44;
    /// Config-register write lock (93C46 command, 8-bit).
    pub const CFG9346: usize = 0x50;
    pub const CONFIG1: usize = 0x52;
    pub const CONFIG2: usize = 0x53;
    pub const CONFIG5: usize = 0x56;
    /// PHY status (16-bit): link, duplex, speed.
    pub const PHY_STATUS: usize = 0x6c;
    /// RTL8125 interrupt config 1 (16-bit).
    pub const INT_CFG1: usize = 0x7a;
    /// RTL8125 TX doorbell (16-bit, bit 0 = normal priority queue). The 8168
    /// used an 8-bit `TxPoll` at 0x38.
    pub const TX_POLL: usize = 0x90;
    /// MAC OCP access window (the 8125's internal MAC registers).
    pub const MAC_OCP: usize = 0xb0;
    /// PHY OCP access window (the integrated PHY's registers).
    pub const PHY_OCP: usize = 0xb8;
    /// MCU status (8-bit): TX/RX FIFO empty, OOB.
    pub const MCU: usize = 0xd3;
    /// RX max frame size (16-bit).
    pub const RX_MAX_SIZE: usize = 0xda;
    /// C+ command (16-bit).
    pub const CPLUS_CMD: usize = 0xe0;
    /// RTL8125: interrupt mitigation/status (16-bit); 0x0103 = idle.
    pub const INTR_MITIGATE: usize = 0xe2;
    /// RX descriptor ring base, low/high dwords.
    pub const RX_DESC_LO: usize = 0xe4;
    pub const RX_DESC_HI: usize = 0xe8;
    /// MISC (32-bit); bit 19 is the RX DV gate.
    pub const MISC: usize = 0xf0;
    /// RSS control (32-bit); zero for one queue.
    pub const RSS_CTRL: usize = 0x4500;
    /// Factory station address, the backup copy (bytes 0..5). Both prior-art
    /// drivers treat this as the authoritative MAC on the 8125.
    pub const MAC0_BKP: usize = 0x19e0;
    /// Highest register offset the driver touches, plus its width. The BAR
    /// mapping must cover at least this.
    pub const MAP_LEN: u32 = 0x4600;
}

/// Bits of [`reg::CMD`].
pub mod cmd {
    pub const TX_ENABLE: u8 = 0x04;
    pub const RX_ENABLE: u8 = 0x08;
    pub const RESET: u8 = 0x10;
    pub const STOP_REQ: u8 = 0x80;
}

/// Bits of [`reg::ISR`] / [`reg::IMR`] (the 8125's 32-bit layout).
pub mod isr {
    pub const RX_OK: u32 = 1 << 0;
    pub const RX_ERR: u32 = 1 << 1;
    pub const TX_OK: u32 = 1 << 2;
    pub const TX_ERR: u32 = 1 << 3;
    /// RX descriptor unavailable: the chip ran out of descriptors it owns.
    pub const RX_DESC_UNAVAIL: u32 = 1 << 4;
    pub const LINK_CHANGE: u32 = 1 << 5;
    pub const RX_FIFO_OVERFLOW: u32 = 1 << 6;
    pub const TX_DESC_UNAVAIL: u32 = 1 << 7;
    pub const SYSTEM_ERR: u32 = 1 << 15;
    /// Every source rung 1 acknowledges.
    pub const ALL: u32 = 0xffff_ffff;
    /// The "overflow" class counted as `rxovf`.
    pub const RX_OVERFLOW_CLASS: u32 = RX_DESC_UNAVAIL | RX_FIFO_OVERFLOW;
}

/// [`reg::CFG9346`] values.
pub const CFG9346_UNLOCK: u8 = 0xc0;
pub const CFG9346_LOCK: u8 = 0x00;
/// [`reg::MISC`] bit 19: RX DV gate. Set during stop/reset; the chip receives
/// nothing until it is cleared again (trap: "link up, rx=0").
pub const MISC_RXDV_GATE: u32 = 1 << 19;
/// [`reg::MCU`] bits: both FIFOs drained.
pub const MCU_FIFOS_EMPTY: u8 = 0x30;
/// [`reg::INTR_MITIGATE`] idle pattern after a stop request (8125B and later).
pub const MITIGATE_IDLE: u16 = 0x0103;
/// [`reg::CONFIG2`] bit 7: CLKREQ enable (cleared with ASPM, trap 7).
pub const CONFIG2_CLKREQ: u8 = 0x80;
/// [`reg::CONFIG1`] bit 4: "speed down" (cleared so the PHY may use 2.5G).
pub const CONFIG1_SPEED_DOWN: u8 = 0x10;
/// [`reg::TX_POLL`] bit 0: kick the normal-priority TX queue.
pub const TX_POLL_NPQ: u16 = 0x0001;

// --- chip identification ------------------------------------------------------

/// The XID: bits 30..20 of TxConfig, masked by 0x7cf. Both prior-art drivers
/// key the MAC version zoo on exactly these bits.
pub const fn xid_of(tx_config: u32) -> u16 {
    ((tx_config >> 20) & 0x7cf) as u16
}

/// The RTL8125 versions rung 1 drives. Anything else is refused by XID.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum MacVersion {
    /// XID 0x609: RTL8125A.
    A,
    /// XID 0x641: RTL8125B.
    B,
    /// XID 0x688: RTL8125D, first stepping.
    D1,
    /// XID 0x689: RTL8125D, second stepping.
    D2,
}

impl MacVersion {
    pub const fn name(self) -> &'static str {
        match self {
            Self::A => "8125A",
            Self::B => "8125B",
            Self::D1 => "8125D-1",
            Self::D2 => "8125D-2",
        }
    }

    pub const fn xid(self) -> u16 {
        match self {
            Self::A => 0x609,
            Self::B => 0x641,
            Self::D1 => 0x688,
            Self::D2 => 0x689,
        }
    }

    /// The RTL8125B and later have a stop-request drain handshake and the
    /// pause-slot RX bit; the 8125A has neither.
    pub const fn is_b_or_later(self) -> bool {
        !matches!(self, Self::A)
    }
}

/// Decode a TxConfig value. `Err(xid)` for every XID that is not an RTL8125
/// rung 1 knows: the RTL8126 (0x649/0x64a), 8125K (0x68a), 8125BP (0x681),
/// 8125CP (0x708), the 8168 family, and a dead bus (all-ones reads as 0x7cf).
/// Never "close enough" (trap 1).
pub const fn decode_xid(tx_config: u32) -> Result<MacVersion, u16> {
    match xid_of(tx_config) {
        0x609 => Ok(MacVersion::A),
        0x641 => Ok(MacVersion::B),
        0x688 => Ok(MacVersion::D1),
        0x689 => Ok(MacVersion::D2),
        other => Err(other),
    }
}

/// A register read of all-ones means the function stopped decoding (surprise
/// removal, a D3 device, or a dead link). Checked before trusting any value.
pub const fn mmio_dead(value: u32) -> bool {
    value == 0xffff_ffff
}

// --- RX / TX configuration ----------------------------------------------------

/// RxConfig accept bits.
pub mod rx_accept {
    pub const ALL_PHYS: u32 = 1 << 0;
    pub const MY_PHYS: u32 = 1 << 1;
    pub const MULTICAST: u32 = 1 << 2;
    pub const BROADCAST: u32 = 1 << 3;
    pub const MASK: u32 = 0x3f;
}

/// RxConfig bit 24: switches the chip to the 32-byte "v3" RX descriptor.
/// OpenBSD sets it and lays out 32-byte RX descriptors; Linux leaves it clear
/// and uses the classic 16-byte layout shared with the 8168. Rung 1 follows
/// Linux: one descriptor format for both rings, the one the dispatch asked to
/// verify against the 8168. It must stay clear.
pub const RX_CONFIG_DESC_V3: u32 = 1 << 24;

/// The RxConfig value to program for `version`, accept filters included:
/// fetch 8 descriptors (bits 31..27), unlimited DMA burst (bits 10..8 = 7),
/// the pause-slot bit (11) on 8125B and later; my-address, broadcast and
/// multicast accepted. The descriptor-v3 bit is never set.
pub const fn rx_config(version: MacVersion) -> u32 {
    let fetch = 8u32 << 27;
    let burst = 7u32 << 8;
    let pause_slot = if version.is_b_or_later() { 1 << 11 } else { 0 };
    fetch | burst | pause_slot | rx_accept::MY_PHYS | rx_accept::BROADCAST | rx_accept::MULTICAST
}

/// TxConfig: standard inter-frame gap (bits 25..24 = 3) and unlimited DMA
/// burst (bits 10..8 = 7). The XID bits are read-only.
pub const TX_CONFIG: u32 = (3 << 24) | (7 << 8);

/// MAC OCP 0xeb58 bit 0 selects the "new" TX descriptor format. Linux clears
/// it (classic 16-byte TX descriptors); OpenBSD sets it with its 32-byte
/// layout. Rung 1 clears it, matching [`RX_CONFIG_DESC_V3`] clear.
pub const MAC_OCP_TX_DESC_FORMAT: u16 = 0xeb58;
pub const TX_DESC_NEW_FORMAT_BIT: u16 = 0x0001;

/// One MAC-OCP read-modify-write: `reg = (reg & !clear) | set`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct OcpModify {
    pub reg: u16,
    pub clear: u16,
    pub set: u16,
}

const fn ocp(reg: u16, clear: u16, set: u16) -> OcpModify {
    OcpModify { reg, clear, set }
}

/// The MAC bring-up writes both prior-art drivers make for every RTL8125 in
/// rung 1 (where they agree), in order. No RSS, one queue, EEE off. The
/// per-version entries come from [`mac_init_versioned`].
pub const MAC_INIT_COMMON: &[OcpModify] = &[
    // Disable UPS (ultra power saving).
    ocp(0xd40a, 0x0010, 0x0000),
    // Classic TX descriptor format (see MAC_OCP_TX_DESC_FORMAT).
    ocp(MAC_OCP_TX_DESC_FORMAT, TX_DESC_NEW_FORMAT_BIT, 0x0000),
    ocp(0xc140, 0xffff, 0xffff),
    ocp(0xc142, 0xffff, 0xffff),
    ocp(0xeb6a, 0x00ff, 0x0033),
    ocp(0xeb50, 0x03e0, 0x0040),
    ocp(0xe056, 0x00f0, 0x0000),
    ocp(0xe040, 0x1000, 0x0000),
    ocp(0xea1c, 0x0003, 0x0001),
    ocp(0xea1c, 0x0004, 0x0000),
    ocp(0xd430, 0x0fff, 0x047f),
    // EEE off (MAC side): rung 1 does not tune EEE; it disables it.
    ocp(0xe040, 0x0003, 0x0000),
    ocp(0xe098, 0xffff, 0xc302),
];

/// The per-version MAC-OCP writes: the TX FIFO threshold field of 0xe614 and
/// the queue field of 0xe63e (OpenBSD's 0x0020 for all four; Linux uses 0 on
/// the 8125B — OpenBSD's ISC value is the one rung 1 carries).
pub const fn mac_init_versioned(version: MacVersion) -> [OcpModify; 2] {
    let e614 = match version {
        MacVersion::B => 0x0200,
        _ => 0x0300,
    };
    [ocp(0xe614, 0x0700, e614), ocp(0xe63e, 0x0030, 0x0020)]
}

/// Apply an [`OcpModify`] to a read value.
pub const fn ocp_apply(value: u16, m: OcpModify) -> u16 {
    (value & !m.clear) | m.set
}

// --- OCP window encoding -------------------------------------------------------

/// Busy/flag bit of the OCP windows (bit 31).
pub const OCP_FLAG: u32 = 1 << 31;

/// Encode an OCP write: flag | (reg/2) << 16 | data. `None` for an odd or
/// out-of-range register (the window addresses 16-bit words below 0x10000).
pub const fn ocp_write_word(reg: u16, data: u16) -> Option<u32> {
    if reg & 1 != 0 {
        return None;
    }
    Some(OCP_FLAG | (((reg >> 1) as u32) << 16) | data as u32)
}

/// Encode an OCP read request (flag clear). The PHY window then sets the flag
/// when data is valid; the MAC window returns data immediately.
pub const fn ocp_read_request(reg: u16) -> Option<u32> {
    if reg & 1 != 0 {
        return None;
    }
    Some(((reg >> 1) as u32) << 16)
}

/// The PHY-OCP address of standard MII register `mii` (0..=15): the 8125
/// exposes the clause-22 page at 0xa400.
pub const fn mii_ocp(mii: u8) -> u16 {
    0xa400 + (mii as u16) * 2
}

/// MII registers and bits rung 1 uses.
pub mod mii {
    pub const BMCR: u8 = 0;
    pub const BMSR: u8 = 1;
    pub const PHYID1: u8 = 2;
    pub const PHYID2: u8 = 3;
    pub const ANAR: u8 = 4;
    pub const GBCR: u8 = 9;
    pub const BMCR_RESET: u16 = 1 << 15;
    pub const BMCR_ANENABLE: u16 = 1 << 12;
    pub const BMCR_POWERDOWN: u16 = 1 << 11;
    pub const BMCR_ANRESTART: u16 = 1 << 9;
    pub const BMSR_ANCOMPLETE: u16 = 1 << 5;
    /// Selector 802.3 | 10H | 10F | 100H | 100F | pause | asym pause.
    pub const ANAR_ALL: u16 = 0x0001 | 0x0020 | 0x0040 | 0x0080 | 0x0100 | 0x0400 | 0x0800;
    pub const GBCR_1000FULL: u16 = 1 << 9;
    pub const GBCR_1000HALF: u16 = 1 << 8;
}

/// PHY-OCP 0xa5d4 bit 7: advertise 2.5GBASE-T.
pub const PHY_OCP_ADV_2500: u16 = 0xa5d4;
pub const ADV_2500_FULL: u16 = 0x0080;
/// PHY-OCP 0xa420 bits 2..0: the PHY state machine; 3 = LAN on (ready).
pub const PHY_OCP_STATE: u16 = 0xa420;
pub const PHY_STATE_LAN_ON: u16 = 3;
/// PHY-OCP indirect pair used to read the PHY RAM-code (patch) version.
pub const PHY_OCP_INDIRECT_ADDR: u16 = 0xa436;
pub const PHY_OCP_INDIRECT_DATA: u16 = 0xa438;
pub const PHY_RAMCODE_VERSION_SLOT: u16 = 0x801e;

/// The PHY-side EEE disables (OpenBSD's list, ISC), applied after the MII
/// advertisement is set. Each is a clear-mask on a PHY-OCP register.
pub const PHY_EEE_OFF: &[OcpModify] = &[
    ocp(0xa5d0, 0x0006, 0x0000),
    ocp(0xa6d4, 0x0001, 0x0000),
    ocp(0xa6d8, 0x0010, 0x0000),
    ocp(0xa428, 0x0080, 0x0000),
    ocp(0xa4a2, 0x0200, 0x0000),
];

/// Decoded [`reg::PHY_STATUS`].
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct LinkReport {
    pub up: bool,
    pub full_duplex: bool,
    /// Mbit/s; 0 when down or unrecognised.
    pub speed_mbps: u32,
}

pub const fn decode_phy_status(status: u16) -> LinkReport {
    let up = status & 0x0002 != 0;
    let speed_mbps = if !up {
        0
    } else if status & 0x0400 != 0 {
        2500
    } else if status & 0x0010 != 0 {
        1000
    } else if status & 0x0008 != 0 {
        100
    } else if status & 0x0004 != 0 {
        10
    } else {
        0
    };
    LinkReport {
        up,
        full_duplex: up && status & 0x0001 != 0,
        speed_mbps,
    }
}

// --- MAC address ----------------------------------------------------------------

/// A station address the driver may use: not all-zero, not all-ones, unicast
/// (I/G bit clear). Zero/ones are what an unfinished EEPROM autoload reads as
/// (trap 8).
pub fn mac_valid(mac: [u8; 6]) -> bool {
    mac != [0; 6] && mac != [0xff; 6] && mac[0] & 0x01 == 0
}

/// Where the station address came from.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum MacSource {
    /// `MAC0`/`MAC4`, as firmware left it.
    Mac0,
    /// The backup register at 0x19e0.
    Backup,
    /// Neither held a valid address (reported as `autoload=timeout`).
    None,
}

/// Choose the station address: `MAC0` when valid, else the backup copy, else
/// none (the caller reports the zero address and refuses the bind).
pub fn choose_mac(mac0: [u8; 6], backup: [u8; 6]) -> ([u8; 6], MacSource) {
    if mac_valid(mac0) {
        (mac0, MacSource::Mac0)
    } else if mac_valid(backup) {
        (backup, MacSource::Backup)
    } else {
        (mac0, MacSource::None)
    }
}

/// The two dwords that program `MAC0`/`MAC4` (little-endian byte order).
pub const fn mac_registers(mac: [u8; 6]) -> (u32, u32) {
    (
        u32::from_le_bytes([mac[0], mac[1], mac[2], mac[3]]),
        u32::from_le_bytes([mac[4], mac[5], 0, 0]),
    )
}

// --- descriptors ------------------------------------------------------------------

/// Descriptor size: the classic 16-byte layout, both rings.
pub const DESC_LEN: usize = 16;
/// Ring base alignment the chip requires (256 bytes; rungs allocate a page).
pub const RING_ALIGN: u64 = 256;

/// opts1 bits shared by TX and RX descriptors.
pub const DESC_OWN: u32 = 1 << 31;
pub const DESC_EOR: u32 = 1 << 30;
/// First / last segment (FS/LS).
pub const DESC_FS: u32 = 1 << 29;
pub const DESC_LS: u32 = 1 << 28;
/// RX opts1: receive error summary.
pub const RX_ERR_SUMMARY: u32 = 1 << 21;
/// RX opts1: frame length including FCS, bits 13..0.
pub const RX_LEN_MASK: u32 = 0x3fff;
/// TX opts1: frame length, bits 15..0 (rung 1 never exceeds 1514).
pub const TX_LEN_MASK: u32 = 0xffff;
/// Ethernet frame check sequence, which RX lengths include.
pub const FCS_LEN: usize = 4;

/// Lay out one descriptor: opts1, opts2, then the 64-bit buffer address, all
/// little-endian.
pub const fn descriptor_bytes(opts1: u32, opts2: u32, addr: u64) -> [u8; DESC_LEN] {
    let a = opts1.to_le_bytes();
    let b = opts2.to_le_bytes();
    let c = addr.to_le_bytes();
    [
        a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3], c[0], c[1], c[2], c[3], c[4], c[5], c[6],
        c[7],
    ]
}

/// TX opts1 for a single-segment frame of `len` bytes handed to the chip.
pub const fn tx_opts1(len: u16, end_of_ring: bool) -> u32 {
    let eor = if end_of_ring { DESC_EOR } else { 0 };
    DESC_OWN | eor | DESC_FS | DESC_LS | (len as u32 & TX_LEN_MASK)
}

/// RX opts1 handing an empty buffer of `buf_len` bytes to the chip.
pub const fn rx_opts1(buf_len: u16, end_of_ring: bool) -> u32 {
    let eor = if end_of_ring { DESC_EOR } else { 0 };
    DESC_OWN | eor | (buf_len as u32 & RX_LEN_MASK)
}

/// What a returned RX descriptor holds.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum RxCompletion {
    /// Still owned by the chip.
    Owned,
    /// A whole frame of `len` bytes (FCS stripped).
    Frame { len: usize },
    /// Error summary set, or a frame shorter than its FCS.
    Error { opts1: u32 },
    /// Not both FS and LS: a frame larger than one buffer. Rung 1 drops it.
    Fragment { opts1: u32 },
    /// The device reported a length the armed buffer cannot hold (or shorter
    /// than an Ethernet header). The length field is device-written metadata:
    /// the DMA fence bounds where the chip writes, not what it claims, so it
    /// is checked before any slice is built from it (I-14(c)).
    BadLength { opts1: u32 },
}

/// The shortest frame handed to the stack: an Ethernet header.
pub const MIN_RX_FRAME: usize = 14;

/// Classify an RX opts1 the chip handed back. Only the opts1 word is read
/// before the OWN bit is known clear; the caller must issue an acquire
/// barrier between this read and any read of the buffer (trap 5).
pub const fn rx_classify(opts1: u32) -> RxCompletion {
    rx_classify_bounded(opts1, RX_LEN_MASK as usize)
}

/// [`rx_classify`] against the capacity of the armed buffer: a returned
/// frame is accepted only when its wire length (FCS included) fits
/// `capacity` and the frame is at least [`MIN_RX_FRAME`] bytes.
pub const fn rx_classify_bounded(opts1: u32, capacity: usize) -> RxCompletion {
    if opts1 & DESC_OWN != 0 {
        return RxCompletion::Owned;
    }
    if opts1 & RX_ERR_SUMMARY != 0 {
        return RxCompletion::Error { opts1 };
    }
    if opts1 & (DESC_FS | DESC_LS) != (DESC_FS | DESC_LS) {
        return RxCompletion::Fragment { opts1 };
    }
    let wire = (opts1 & RX_LEN_MASK) as usize;
    if wire <= FCS_LEN {
        return RxCompletion::Error { opts1 };
    }
    if wire > capacity || wire - FCS_LEN < MIN_RX_FRAME {
        return RxCompletion::BadLength { opts1 };
    }
    RxCompletion::Frame {
        len: wire - FCS_LEN,
    }
}

// --- ring state machines ------------------------------------------------------------

/// Descriptor memory as the rings see it. The kernel implements this over
/// the arena page with volatile accesses and the barriers; the host tests
/// implement it over an array driven by a fake chip.
pub trait DescMem {
    /// Read a descriptor's opts1 (the status word the chip writes back).
    fn read_opts1(&self, index: usize) -> u32;
    /// Publish a descriptor: write `addr` and `opts2`, then a release barrier,
    /// then `opts1`. opts1 carries OWN, so it must be the last store the chip
    /// can observe.
    fn publish(&mut self, index: usize, opts1: u32, opts2: u32, addr: u64);
}

/// TX ring: a producer index, an in-order completion index and a count.
/// Completions are reclaimed strictly in order, so a descriptor the chip
/// reports done ahead of an older still-owned one is held until the older
/// one completes (it is never double-reclaimed or reclaimed while owned).
#[derive(Clone, Debug)]
pub struct TxRing {
    len: usize,
    produce: usize,
    complete: usize,
    in_flight: usize,
}

impl TxRing {
    pub const fn new(len: usize) -> Self {
        Self {
            len,
            produce: 0,
            complete: 0,
            in_flight: 0,
        }
    }

    pub const fn in_flight(&self) -> usize {
        self.in_flight
    }

    pub const fn is_full(&self) -> bool {
        self.in_flight == self.len
    }

    /// Post one frame. Returns the slot used, or `None` when the ring is full.
    /// The caller rings the doorbell after this returns (and after its own
    /// write barrier, trap 6).
    pub fn post<M: DescMem>(&mut self, mem: &mut M, addr: u64, len: u16) -> Option<usize> {
        if self.is_full() {
            return None;
        }
        let slot = self.produce;
        mem.publish(slot, tx_opts1(len, slot == self.len - 1), 0, addr);
        self.produce = (slot + 1) % self.len;
        self.in_flight += 1;
        Some(slot)
    }

    /// Reclaim completed slots in order, up to `budget`. Calls `done(slot)`
    /// for each so the caller can recycle that slot's buffer.
    pub fn reap<M: DescMem>(
        &mut self,
        mem: &M,
        budget: usize,
        mut done: impl FnMut(usize),
    ) -> usize {
        let mut reaped = 0;
        while self.in_flight > 0 && reaped < budget {
            if mem.read_opts1(self.complete) & DESC_OWN != 0 {
                break;
            }
            done(self.complete);
            self.complete = (self.complete + 1) % self.len;
            self.in_flight -= 1;
            reaped += 1;
        }
        reaped
    }
}

/// What one RX reap step did with a slot. Either way the slot has left the
/// chip and is now empty: the caller takes its buffer (to the stack for a
/// frame, back to the free pool for a drop) and later [`RxRing::refill`]s.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum RxEvent {
    /// A frame of `len` bytes is in `slot`'s buffer.
    Frame { slot: usize, len: usize },
    /// An error or fragment descriptor in `slot`: nothing handed out, the
    /// buffer goes straight back to the pool.
    Dropped { slot: usize, opts1: u32 },
    /// A completion whose length does not fit the armed buffer (see
    /// [`RxCompletion::BadLength`]): nothing handed out, buffer to the pool,
    /// counted separately.
    BadLength { slot: usize, opts1: u32 },
}

/// RX ring. The chip consumes slots in ring order; the driver reaps them in
/// the same order (`next`) and re-arms them in the same order (`fill`), so
/// the slots the chip owns are always the contiguous run `[next, fill)` and
/// the empty ones `[fill, next)`, with `next == fill + empty (mod len)`.
/// A slot whose buffer is still out with the stack is never examined: the
/// reap stops when every slot is empty, and the chip cannot fill a slot it
/// does not own (it raises RX_DESC_UNAVAIL instead, the overflow the driver
/// counts; reception resumes on refill).
#[derive(Clone, Debug)]
pub struct RxRing {
    len: usize,
    buf_len: u16,
    next: usize,
    fill: usize,
    empty: usize,
}

impl RxRing {
    /// A ring whose every slot is empty; [`Self::refill`] each one to arm it.
    pub const fn new(len: usize, buf_len: u16) -> Self {
        Self {
            len,
            buf_len,
            next: 0,
            fill: 0,
            empty: len,
        }
    }

    pub const fn empty_slots(&self) -> usize {
        self.empty
    }

    /// The slot the next [`Self::refill`] will arm, if any slot is empty.
    pub const fn next_fill_slot(&self) -> Option<usize> {
        if self.empty == 0 {
            None
        } else {
            Some(self.fill)
        }
    }

    /// Hand the oldest empty slot to the chip with the buffer at `addr`.
    /// Returns the slot armed, or `None` if no slot is empty.
    pub fn refill<M: DescMem>(&mut self, mem: &mut M, addr: u64) -> Option<usize> {
        let slot = self.next_fill_slot()?;
        mem.publish(slot, rx_opts1(self.buf_len, slot == self.len - 1), 0, addr);
        self.fill = (slot + 1) % self.len;
        self.empty -= 1;
        Some(slot)
    }

    /// Arm every empty slot a pooled buffer can fill. `pop` yields pool
    /// buffers; `fence` returns the fenced address of one, or `None` when the
    /// gate refuses it, in which case the buffer goes to `reject` (kept and
    /// accounted, never handed to the chip, never silently dropped) and the
    /// next one is tried; `arm(slot, buf)` records the buffer now in `slot`.
    /// Bounded: every iteration consumes one pool buffer. Returns the slots
    /// armed.
    pub fn refill_from<M: DescMem, B>(
        &mut self,
        mem: &mut M,
        mut pop: impl FnMut() -> Option<B>,
        mut fence: impl FnMut(&B) -> Option<u64>,
        mut reject: impl FnMut(B),
        mut arm: impl FnMut(usize, B),
    ) -> usize {
        let mut armed = 0;
        while self.next_fill_slot().is_some() {
            let Some(buf) = pop() else {
                break;
            };
            match fence(&buf) {
                Some(addr) => {
                    if let Some(slot) = self.refill(mem, addr) {
                        arm(slot, buf);
                        armed += 1;
                    }
                }
                None => reject(buf),
            }
        }
        armed
    }

    /// Examine up to `budget` returned descriptors in order. `acquire()` runs
    /// after each OWN-clear opts1 read and before the event is reported, so
    /// the caller's buffer read cannot pass the status read (trap 5).
    pub fn reap<M: DescMem>(
        &mut self,
        mem: &M,
        budget: usize,
        mut acquire: impl FnMut(),
        mut event: impl FnMut(RxEvent),
    ) -> usize {
        let mut seen = 0;
        while seen < budget && self.empty < self.len {
            let slot = self.next;
            let completion = rx_classify_bounded(mem.read_opts1(slot), self.buf_len as usize);
            if completion == RxCompletion::Owned {
                break;
            }
            acquire();
            self.next = (slot + 1) % self.len;
            self.empty += 1;
            match completion {
                RxCompletion::Frame { len } => event(RxEvent::Frame { slot, len }),
                RxCompletion::Error { opts1 } | RxCompletion::Fragment { opts1 } => {
                    event(RxEvent::Dropped { slot, opts1 })
                }
                RxCompletion::BadLength { opts1 } => event(RxEvent::BadLength { slot, opts1 }),
                RxCompletion::Owned => unreachable!(),
            }
            seen += 1;
        }
        seen
    }
}

/// Frames one boot pump pass takes before it yields to its caller's
/// bookkeeping (DHCP tick, deadline). A receive stream that the chip keeps
/// replenishing must not hold the pump forever (I-14(b)).
pub const PUMP_FRAME_BUDGET: usize = 24;

/// Call `take_one` until it reports no more work or `budget` calls have
/// handled one item each. Returns the items handled (at most `budget`).
pub fn drain_bounded(budget: usize, mut take_one: impl FnMut() -> bool) -> usize {
    let mut handled = 0;
    while handled < budget && take_one() {
        handled += 1;
    }
    handled
}

// --- interrupt-status storm detection (I-14(b)) ---------------------------------------

/// Consecutive service passes on which the same error/overflow status bits
/// re-assert immediately after being acknowledged, with no ring progress,
/// before the source is declared a storm.
pub const STORM_PASSES: u32 = 64;
/// Storms after which rung 1 stops servicing the chip's status (masked, the
/// rings still polled) and reports it.
pub const STORM_LIMIT: u32 = 4;

/// Tracks a stuck status bit. Rung 1 runs polled, so a storm cannot take
/// interrupts; what it can do is make every service pass spin on the same
/// bit. The detector bounds that: after [`STORM_PASSES`] fruitless passes the
/// bits are masked out of the per-pass work for good once [`STORM_LIMIT`]
/// storms have been seen.
#[derive(Clone, Debug, Default)]
pub struct StormDetector {
    streak: u32,
    last: u32,
    storms: u32,
    masked: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum StormVerdict {
    Quiet,
    /// A storm was just declared on these bits.
    Storm {
        bits: u32,
        total: u32,
    },
}

impl StormDetector {
    pub const fn new() -> Self {
        Self {
            streak: 0,
            last: 0,
            storms: 0,
            masked: 0,
        }
    }

    pub const fn storms(&self) -> u32 {
        self.storms
    }

    /// Bits no longer acted on (after the storm limit).
    pub const fn masked(&self) -> u32 {
        self.masked
    }

    /// Feed one pass: the error/overflow bits seen (before ack) and whether
    /// any descriptor moved this pass.
    pub fn observe(&mut self, error_bits: u32, progress: bool) -> StormVerdict {
        let bits = error_bits & !self.masked;
        if bits == 0 || progress || bits != self.last {
            self.streak = if bits == 0 || progress { 0 } else { 1 };
            self.last = bits;
            return StormVerdict::Quiet;
        }
        self.streak += 1;
        if self.streak < STORM_PASSES {
            return StormVerdict::Quiet;
        }
        self.streak = 0;
        self.storms += 1;
        if self.storms >= STORM_LIMIT {
            self.masked |= bits;
        }
        StormVerdict::Storm {
            bits,
            total: self.storms,
        }
    }
}

// --- DMA containment (I-14(c)) -------------------------------------------------------------

/// How the device's DMA is contained, and so whether it may bus-master.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DmaMode {
    /// The device sits in the VT-d fence domain (arena only) and every
    /// programmed address is a fenced arena address: I-14(c1) + (c2).
    Iommu,
    /// No supported IOMMU, and the build carries the explicit
    /// `rtl8125-dma-without-iommu` opt-in: arena addresses only (c1).
    ArenaOnly,
    /// No IOMMU containment and no opt-in, or no arena: bus mastering stays
    /// off; the driver reports MAC and link and stops.
    Blocked(BlockReason),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BlockReason {
    /// No VT-d fence for the device and no opt-in.
    NoIommu,
    /// The fenced DMA arena is not available (it fails closed).
    NoArena,
}

/// The decision table (dispatch §"DMA containment"):
///
/// | arena | fenced by VT-d | opt-in | mode              | bus master |
/// |-------|----------------|--------|-------------------|------------|
/// | no    | any            | any    | Blocked(NoArena)  | off        |
/// | yes   | yes            | any    | Iommu             | on         |
/// | yes   | no             | yes    | ArenaOnly         | on         |
/// | yes   | no             | no     | Blocked(NoIommu)  | off        |
pub const fn containment(arena_ready: bool, vtd_fenced: bool, opt_in: bool) -> DmaMode {
    if !arena_ready {
        DmaMode::Blocked(BlockReason::NoArena)
    } else if vtd_fenced {
        DmaMode::Iommu
    } else if opt_in {
        DmaMode::ArenaOnly
    } else {
        DmaMode::Blocked(BlockReason::NoIommu)
    }
}

pub const fn bus_master_allowed(mode: DmaMode) -> bool {
    matches!(mode, DmaMode::Iommu | DmaMode::ArenaOnly)
}

/// The `dma=` field of the console line.
pub const fn dma_label(mode: DmaMode) -> &'static str {
    match mode {
        DmaMode::Iommu => "iommu",
        DmaMode::ArenaOnly => "arena-only",
        DmaMode::Blocked(BlockReason::NoIommu) => "blocked (no iommu)",
        DmaMode::Blocked(BlockReason::NoArena) => "blocked (no arena)",
    }
}

/// PCI COMMAND bit 2: bus master enable.
pub const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;

/// The PCI COMMAND value to write when binding: memory decode on, I/O decode
/// off, INTx disabled (rung 1 is polled, so the line can never storm), and
/// bus master off whatever firmware left (a UEFI network stack may leave it
/// set). Bus master is granted separately, after the rings are programmed
/// and only when [`bus_master_allowed`] (trap 4).
pub const fn bind_command(current: u16) -> u16 {
    const IO: u16 = 1 << 0;
    const MEMORY: u16 = 1 << 1;
    const INTX_DISABLE: u16 = 1 << 10;
    (current | MEMORY | INTX_DISABLE) & !(IO | PCI_COMMAND_BUS_MASTER)
}

/// PCI Express Link Control: ASPM L0s/L1 field (bits 1..0) and the clock
/// power management bit (8). Cleared at bind (trap 7: link flaps and TX
/// hangs with ASPM on several boards).
pub const fn link_control_aspm_off(link_control: u16) -> u16 {
    link_control & !(0x0003 | 0x0100)
}
