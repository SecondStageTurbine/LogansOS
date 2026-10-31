// SPDX-License-Identifier: MPL-2.0
//! Intel LPSS (Serial-IO) private registers and the Synopsys DesignWare
//! `DW_apb_i2c` master, polled (PIO). Pure: every register access goes
//! through [`Regs`], so the host tests run this code against a register model
//! (`tools/i2c-hid-logic-tests`).
//!
//! **Register map.** Synopsys DW_apb_i2c databook (the register offsets are
//! the same in every Intel LPSS I2C since Sunrise Point): `IC_CON` 0x00,
//! `IC_TAR` 0x04, `IC_DATA_CMD` 0x10, `IC_SS_SCL_HCNT`/`LCNT` 0x14/0x18,
//! `IC_FS_SCL_HCNT`/`LCNT` 0x1C/0x20, `IC_INTR_MASK` 0x30,
//! `IC_RAW_INTR_STAT` 0x34, `IC_RX_TL`/`IC_TX_TL` 0x38/0x3C, `IC_CLR_INTR`
//! 0x40, `IC_CLR_TX_ABRT` 0x54, `IC_CLR_STOP_DET` 0x60, `IC_ENABLE` 0x6C,
//! `IC_STATUS` 0x70, `IC_TXFLR`/`IC_RXFLR` 0x74/0x78, `IC_SDA_HOLD` 0x7C,
//! `IC_TX_ABRT_SOURCE` 0x80, `IC_ENABLE_STATUS` 0x9C, `IC_COMP_PARAM_1` 0xF4,
//! `IC_COMP_VERSION` 0xF8, `IC_COMP_TYPE` 0xFC (reads `0x44570140`, "DW").
//!
//! **LPSS private block** at BAR0 + 0x200: clock parameters (+0x00), resets
//! (+0x04: bits 1:0 the function, bit 2 the integrated DMA; 1 = out of
//! reset), capabilities (+0xFC: bit 8 "no iDMA", bits 7:4 the function
//! type, 0 = I2C). The I2C function's input clock is fixed (100 MHz on
//! Panther Lake, [`LPSS_I2C_CLOCK_HZ`]); its clock-parameter divider is not
//! used for I2C, so this driver reads it for the console line and never
//! writes it.
//!
//! **PIO only (I-14(c)).** The resets write takes the function out of reset
//! and leaves the iDMA bit 0, so the LPSS DMA engine stays in reset for the
//! life of the driver; [`lpss_dma_in_reset`] is asserted after every write.
//! Nothing here programs a DMA register or a DesignWare DMA control
//! (`IC_DMA_CR` stays 0).
//!
//! **Transfers** are one write-then-read (or write-only, or read-only)
//! message: the bytes to write, then read commands, the first read carrying
//! `RESTART` (a repeated start, never STOP+START: some devices reject the
//! latter) and the last command carrying `STOP`. The TX FIFO is fed as it has
//! room, at most an RX FIFO's worth of reads outstanding, so neither FIFO
//! overflows. Every wait is bounded by a deadline in the caller's clock;
//! a timed-out transfer is aborted (`IC_ENABLE.ABORT`) and the controller
//! disabled, so a wedged bus is reported, never spun on.

pub const IC_CON: usize = 0x00;
pub const IC_TAR: usize = 0x04;
pub const IC_DATA_CMD: usize = 0x10;
pub const IC_SS_SCL_HCNT: usize = 0x14;
pub const IC_SS_SCL_LCNT: usize = 0x18;
pub const IC_FS_SCL_HCNT: usize = 0x1C;
pub const IC_FS_SCL_LCNT: usize = 0x20;
pub const IC_INTR_MASK: usize = 0x30;
pub const IC_RAW_INTR_STAT: usize = 0x34;
pub const IC_RX_TL: usize = 0x38;
pub const IC_TX_TL: usize = 0x3C;
pub const IC_CLR_INTR: usize = 0x40;
pub const IC_CLR_TX_ABRT: usize = 0x54;
pub const IC_CLR_STOP_DET: usize = 0x60;
pub const IC_ENABLE: usize = 0x6C;
pub const IC_STATUS: usize = 0x70;
pub const IC_TXFLR: usize = 0x74;
pub const IC_RXFLR: usize = 0x78;
pub const IC_SDA_HOLD: usize = 0x7C;
pub const IC_TX_ABRT_SOURCE: usize = 0x80;
pub const IC_DMA_CR: usize = 0x88;
pub const IC_ENABLE_STATUS: usize = 0x9C;
pub const IC_COMP_PARAM_1: usize = 0xF4;
pub const IC_COMP_VERSION: usize = 0xF8;
pub const IC_COMP_TYPE: usize = 0xFC;

pub const COMP_TYPE_DW: u32 = 0x4457_0140;

pub const CON_MASTER: u32 = 1 << 0;
pub const CON_SPEED_FAST: u32 = 2 << 1;
pub const CON_RESTART_EN: u32 = 1 << 5;
pub const CON_SLAVE_DISABLE: u32 = 1 << 6;
/// Master, fast mode, 7-bit addressing, repeated start, slave off.
pub const CON_FAST_MASTER: u32 = CON_MASTER | CON_SPEED_FAST | CON_RESTART_EN | CON_SLAVE_DISABLE;

pub const DATA_CMD_READ: u32 = 1 << 8;
pub const DATA_CMD_STOP: u32 = 1 << 9;
pub const DATA_CMD_RESTART: u32 = 1 << 10;

pub const INTR_TX_ABRT: u32 = 1 << 6;
pub const INTR_STOP_DET: u32 = 1 << 9;

pub const ENABLE: u32 = 1 << 0;
pub const ENABLE_ABORT: u32 = 1 << 1;

pub const STATUS_MST_ACTIVITY: u32 = 1 << 5;

pub const ABRT_7B_ADDR_NOACK: u32 = 1 << 0;
pub const ABRT_TXDATA_NOACK: u32 = 1 << 3;
pub const ABRT_GCALL_NOACK: u32 = 1 << 4;
pub const ABRT_MASTER_DIS: u32 = 1 << 11;
pub const ABRT_ARB_LOST: u32 = 1 << 12;
pub const ABRT_USER_ABRT: u32 = 1 << 16;

/// The LPSS private block's offset in BAR0, and its registers.
pub const LPSS_PRIV: usize = 0x200;
pub const LPSS_CLOCK_PARAMS: usize = LPSS_PRIV;
pub const LPSS_RESETS: usize = LPSS_PRIV + 0x04;
pub const LPSS_CAPS: usize = LPSS_PRIV + 0xFC;
/// Resets bits 1:0: the function out of reset.
pub const LPSS_RESETS_FUNC: u32 = 0x3;
/// Resets bit 2: the integrated DMA out of reset. Never set by this driver.
pub const LPSS_RESETS_IDMA: u32 = 1 << 2;
/// BAR0 bytes the driver maps: the DesignWare block and the private block.
pub const BAR0_MAP_LEN: u32 = 0x300;

/// The Panther Lake LPSS I2C input clock (`ic_clk`): 100 MHz.
pub const LPSS_I2C_CLOCK_HZ: u64 = 100_000_000;

/// SCL fall time used for the counts: 208 ns, and the SDA hold time: 42 ns
/// (the values Intel publishes for its LPSS I2C at 100 MHz).
pub const SCL_FALL_NS: u64 = 208;
pub const SDA_HOLD_NS: u64 = 42;

/// Register access, plus the clock the deadlines are measured in.
pub trait Regs {
    fn read(&mut self, offset: usize) -> u32;
    fn write(&mut self, offset: usize, value: u32);
    /// Monotonic time, in the unit the caller's budgets use.
    fn now(&mut self) -> u64;
}

/// SCL high/low counts for one speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SclCounts {
    pub hcnt: u16,
    pub lcnt: u16,
}

/// `round(clk_hz * ns / 1e9)`: clock cycles in `ns` nanoseconds, to nearest.
pub const fn cycles(clk_hz: u64, ns: u64) -> u64 {
    (clk_hz * ns + 500_000_000) / 1_000_000_000
}

/// SCL counts from the I2C-bus specification's minimum high and low times
/// (UM10204 Table 10: fast mode tHIGH ≥ 600 ns, tLOW ≥ 1300 ns; standard
/// mode 4000 / 4700 ns), each lengthened by the SCL fall time, less the
/// DesignWare core's fixed counts (the databook's: 3 cycles on the high
/// count when the spike filter is accounted separately, 1 on the low).
pub const fn scl_counts(clk_hz: u64, t_high_ns: u64, t_low_ns: u64, fall_ns: u64) -> SclCounts {
    let h = cycles(clk_hz, t_high_ns + fall_ns).saturating_sub(3);
    let l = cycles(clk_hz, t_low_ns + fall_ns).saturating_sub(1);
    SclCounts {
        hcnt: if h > 0xFFFF { 0xFFFF } else { h as u16 },
        lcnt: if l > 0xFFFF { 0xFFFF } else { l as u16 },
    }
}

pub const fn fast_mode_counts(clk_hz: u64) -> SclCounts {
    scl_counts(clk_hz, 600, 1300, SCL_FALL_NS)
}

pub const fn standard_mode_counts(clk_hz: u64) -> SclCounts {
    scl_counts(clk_hz, 4000, 4700, SCL_FALL_NS)
}

/// The FIFO depths `IC_COMP_PARAM_1` reports (bits 23:16 TX − 1, 15:8
/// RX − 1).
pub const fn fifo_depths(comp_param_1: u32) -> (u32, u32) {
    (
        ((comp_param_1 >> 16) & 0xFF) + 1,
        ((comp_param_1 >> 8) & 0xFF) + 1,
    )
}

/// The LPSS resets value after the driver's write: the function out of
/// reset, the iDMA held in it.
pub const fn lpss_resets_value() -> u32 {
    LPSS_RESETS_FUNC
}

/// I-14(c): the iDMA is in reset (resets bit 2 clear).
pub const fn lpss_dma_in_reset(resets: u32) -> bool {
    resets & LPSS_RESETS_IDMA == 0
}

/// Which wait ran out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Disable,
    Enable,
    Transfer,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Reads all-ones: the function is in D3, reset, or not decoding.
    AllOnes,
    /// `IC_COMP_TYPE` is not DesignWare's.
    NotDesignWare(u32),
    Timeout(Stage),
    /// `IC_TX_ABRT_SOURCE` at the abort.
    Abort(u32),
    /// More bytes asked for than one message can carry.
    TooLong,
}

/// The decoded abort cause, for the console line.
pub fn abort_reason(source: u32) -> &'static str {
    if source & ABRT_7B_ADDR_NOACK != 0 {
        "addr-nak"
    } else if source & ABRT_TXDATA_NOACK != 0 {
        "data-nak"
    } else if source & ABRT_ARB_LOST != 0 {
        "arb-lost"
    } else if source & ABRT_MASTER_DIS != 0 {
        "master-disabled"
    } else if source & ABRT_GCALL_NOACK != 0 {
        "gcall-nak"
    } else if source & ABRT_USER_ABRT != 0 {
        "user-abort"
    } else {
        "other"
    }
}

/// Deadlines, in the [`Regs::now`] unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// One enable/disable handshake.
    pub handshake: u64,
    /// A transfer's fixed part (address, start/stop).
    pub transfer_base: u64,
    /// Added per byte moved.
    pub per_byte: u64,
}

impl Budget {
    /// From a rate: the handshake gets 25 ms, a transfer 10 ms plus 1 ms a
    /// byte (a byte at 400 kHz takes about 23 µs; 1 ms leaves room for clock
    /// stretching).
    pub const fn from_units_per_ms(units_per_ms: u64) -> Self {
        Self {
            handshake: 25 * units_per_ms,
            transfer_base: 10 * units_per_ms,
            per_byte: units_per_ms,
        }
    }
}

/// What the controller was configured with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controller {
    pub tx_depth: u32,
    pub rx_depth: u32,
    pub version: u32,
    pub fast: SclCounts,
    pub sda_hold: u32,
}

/// Disable and wait for `IC_ENABLE_STATUS.IC_EN` to clear (configuration
/// registers are writable only while it is clear).
pub fn disable(regs: &mut impl Regs, budget: &Budget) -> Result<(), Error> {
    regs.write(IC_ENABLE, 0);
    wait_enable_status(regs, budget, false)
}

pub fn enable(regs: &mut impl Regs, budget: &Budget) -> Result<(), Error> {
    regs.write(IC_ENABLE, ENABLE);
    wait_enable_status(regs, budget, true)
}

fn wait_enable_status(regs: &mut impl Regs, budget: &Budget, on: bool) -> Result<(), Error> {
    let start = regs.now();
    loop {
        let status = regs.read(IC_ENABLE_STATUS);
        if status == u32::MAX {
            return Err(Error::AllOnes);
        }
        if (status & 1 != 0) == on {
            return Ok(());
        }
        if regs.now().wrapping_sub(start) >= budget.handshake {
            return Err(Error::Timeout(if on {
                Stage::Enable
            } else {
                Stage::Disable
            }));
        }
    }
}

/// Identify and configure the master for fast mode at `clk_hz`, polled: no
/// DesignWare interrupt is unmasked and DMA stays off. Leaves it disabled.
pub fn configure(regs: &mut impl Regs, clk_hz: u64, budget: &Budget) -> Result<Controller, Error> {
    let comp_type = regs.read(IC_COMP_TYPE);
    if comp_type == u32::MAX {
        return Err(Error::AllOnes);
    }
    if comp_type != COMP_TYPE_DW {
        return Err(Error::NotDesignWare(comp_type));
    }
    let (tx_depth, rx_depth) = fifo_depths(regs.read(IC_COMP_PARAM_1));
    let version = regs.read(IC_COMP_VERSION);
    disable(regs, budget)?;
    let fast = fast_mode_counts(clk_hz);
    let standard = standard_mode_counts(clk_hz);
    regs.write(IC_CON, CON_FAST_MASTER);
    regs.write(IC_SS_SCL_HCNT, standard.hcnt as u32);
    regs.write(IC_SS_SCL_LCNT, standard.lcnt as u32);
    regs.write(IC_FS_SCL_HCNT, fast.hcnt as u32);
    regs.write(IC_FS_SCL_LCNT, fast.lcnt as u32);
    // TX hold in bits 15:0; keep the RX hold firmware left in bits 23:16.
    let hold = regs.read(IC_SDA_HOLD);
    let tx_hold = cycles(clk_hz, SDA_HOLD_NS) as u32;
    let sda_hold = if hold & 0xFFFF < tx_hold {
        (hold & !0xFFFF) | tx_hold
    } else {
        hold
    };
    regs.write(IC_SDA_HOLD, sda_hold);
    regs.write(IC_INTR_MASK, 0);
    regs.write(IC_RX_TL, 0);
    regs.write(IC_TX_TL, 0);
    regs.write(IC_DMA_CR, 0);
    Ok(Controller {
        tx_depth,
        rx_depth,
        version,
        fast,
        sda_hold,
    })
}

/// Bytes one message may move (well above a HID report descriptor).
pub const MAX_MESSAGE: usize = 4096;

/// One message to 7-bit `address`: write `write`, then (repeated start) read
/// `read.len()` bytes. Either may be empty, not both.
pub fn transfer(
    regs: &mut impl Regs,
    ctl: &Controller,
    budget: &Budget,
    address: u8,
    write: &[u8],
    read: &mut [u8],
) -> Result<(), Error> {
    let total = write.len() + read.len();
    if total == 0 || total > MAX_MESSAGE {
        return Err(Error::TooLong);
    }
    disable(regs, budget)?;
    regs.write(IC_TAR, (address & 0x7F) as u32);
    enable(regs, budget)?;
    let _ = regs.read(IC_CLR_INTR);
    let _ = regs.read(IC_CLR_TX_ABRT);
    let _ = regs.read(IC_CLR_STOP_DET);

    let deadline = budget.transfer_base + budget.per_byte * total as u64;
    let start = regs.now();
    let mut issued = 0usize;
    let mut received = 0usize;
    let mut stopped = false;
    loop {
        let raw = regs.read(IC_RAW_INTR_STAT);
        if raw == u32::MAX {
            return Err(Error::AllOnes);
        }
        if raw & INTR_TX_ABRT != 0 {
            let source = regs.read(IC_TX_ABRT_SOURCE);
            let _ = regs.read(IC_CLR_TX_ABRT);
            let _ = disable(regs, budget);
            return Err(Error::Abort(source));
        }
        if raw & INTR_STOP_DET != 0 {
            stopped = true;
        }
        while received < read.len() && regs.read(IC_RXFLR) > 0 {
            read[received] = (regs.read(IC_DATA_CMD) & 0xFF) as u8;
            received += 1;
        }
        if received == read.len() && issued == total && stopped {
            // The abort check above ran after the STOP was seen in this same
            // pass, so a NAK'd message never gets here.
            let _ = regs.read(IC_CLR_STOP_DET);
            return Ok(());
        }
        while issued < total && regs.read(IC_TXFLR) < ctl.tx_depth {
            let mut cmd = if issued < write.len() {
                write[issued] as u32
            } else {
                // Reads still outstanding (issued, not yet received).
                if (issued - write.len()) - received >= ctl.rx_depth as usize {
                    break;
                }
                let mut c = DATA_CMD_READ;
                if issued == write.len() && !write.is_empty() {
                    c |= DATA_CMD_RESTART;
                }
                c
            };
            if issued + 1 == total {
                cmd |= DATA_CMD_STOP;
            }
            regs.write(IC_DATA_CMD, cmd);
            issued += 1;
        }
        if regs.now().wrapping_sub(start) >= deadline {
            regs.write(IC_ENABLE, ENABLE | ENABLE_ABORT);
            let _ = disable(regs, budget);
            return Err(Error::Timeout(
                if issued == total && received == read.len() {
                    Stage::Stop
                } else {
                    Stage::Transfer
                },
            ));
        }
    }
}
