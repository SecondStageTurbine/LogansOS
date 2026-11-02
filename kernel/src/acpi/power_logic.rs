// SPDX-License-Identifier: MPL-2.0
//! OT-90 P1: the pure logic of power-off and restart. Shared with the host KATs
//! in `tools/acpi-power-logic-tests`. Nothing here touches hardware: it reads
//! table bytes and composes the register values P2 writes.
//! Design: `docs/THINK-2026-10-27-ot90-shutdown-restart.md` §3-§4.
//!
//! Sources, read for principle (ACPICA `e85aa3be`, dual BSD/GPL, no code taken;
//! ACPI 6.5):
//! - the FADT layout: `actbl.h` `ACPI_TABLE_FADT` and the flag bits
//!   (`ACPI_FADT_RESET_REGISTER` bit 10, `ACPI_FADT_HW_REDUCED` bit 20,
//!   `ACPI_FADT_LOW_POWER_S0` bit 21);
//! - a register's address: the X_ GAS when its address is non-zero, else the
//!   32-bit block as SystemIO with `LEN * 8` bits (`tbfadt.c` `AcpiTbConvertFadt`,
//!   with `AcpiGbl_Use32BitFadtAddresses` false);
//! - the legacy S5 sequence: `hwsleep.c` `AcpiHwLegacySleep`; the hardware-reduced
//!   one: `hwesleep.c` `AcpiHwExtendedSleep`; the PM1 bit masks: `aclocal.h`;
//! - the reset register: `hwxface.c` `AcpiReset` (an 8-bit write to SystemIO,
//!   whatever the GAS width says).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// The FADT
// ---------------------------------------------------------------------------

/// FADT flags: the PM timer counts 32 bits (else 24).
pub const TMR_VAL_EXT: u32 = 1 << 8;
/// FADT flags: the RESET_REG is supported.
pub const RESET_REG_SUP: u32 = 1 << 10;
/// FADT flags: hardware-reduced ACPI (no PM1 blocks).
pub const HW_REDUCED_ACPI: u32 = 1 << 20;
/// FADT flags: low-power S0 idle is at least as good as S3.
pub const LOW_POWER_S0: u32 = 1 << 21;

/// GAS address space IDs (ACPI 6.5 §5.2.3.2).
pub const SPACE_SYSTEM_MEMORY: u8 = 0;
pub const SPACE_SYSTEM_IO: u8 = 1;
pub const SPACE_PCI_CONFIG: u8 = 2;

/// A Generic Address Structure (ACPI 6.5 §5.2.3.2), 12 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gas {
    pub space: u8,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

impl Gas {
    /// A SystemIO register of `bits` bits at `port`.
    pub const fn io(port: u64, bits: u8) -> Gas {
        Gas {
            space: SPACE_SYSTEM_IO,
            bit_width: bits,
            bit_offset: 0,
            access_size: 0,
            address: port,
        }
    }
}

/// What the FADT says about power-off and restart. A register is `None` when
/// the FADT names none (address 0 in both its 32-bit and X_ forms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FadtPower {
    pub revision: u8,
    pub flags: u32,
    pub smi_cmd: u32,
    pub acpi_enable: u8,
    pub pm1a_evt: Option<Gas>,
    pub pm1b_evt: Option<Gas>,
    pub pm1_evt_len: u8,
    pub pm1a_cnt: Option<Gas>,
    pub pm1b_cnt: Option<Gas>,
    pub pm1_cnt_len: u8,
    pub gpe0: Option<Gas>,
    pub gpe0_len: u8,
    pub reset_reg: Option<Gas>,
    pub reset_value: u8,
    pub sleep_control: Option<Gas>,
    pub sleep_status: Option<Gas>,
    /// The ACPI PM timer (3.579545 MHz): its register, when it is in SystemIO,
    /// and whether it counts 32 bits (`TMR_VAL_EXT`) rather than 24.
    pub pm_timer: Option<(u64, bool)>,
}

/// The ACPI PM timer's frequency (ACPI 6.5 §4.8.3.3).
pub const PM_TIMER_HZ: u64 = 3_579_545;

impl FadtPower {
    pub fn hw_reduced(&self) -> bool {
        self.flags & HW_REDUCED_ACPI != 0
    }
}

/// The FADT byte offsets (ACPI 6.5 Table 5.9; ACPICA `ACPI_TABLE_FADT`).
mod off {
    pub const SMI_CMD: usize = 48;
    pub const ACPI_ENABLE: usize = 52;
    pub const PM1A_EVT: usize = 56;
    pub const PM1B_EVT: usize = 60;
    pub const PM1A_CNT: usize = 64;
    pub const PM1B_CNT: usize = 68;
    pub const PM_TMR: usize = 76;
    pub const GPE0: usize = 80;
    pub const PM1_EVT_LEN: usize = 88;
    pub const PM1_CNT_LEN: usize = 89;
    pub const PM_TMR_LEN: usize = 91;
    pub const GPE0_LEN: usize = 92;
    pub const FLAGS: usize = 112;
    pub const RESET_REG: usize = 116;
    pub const RESET_VALUE: usize = 128;
    pub const X_PM1A_EVT: usize = 148;
    pub const X_PM1B_EVT: usize = 160;
    pub const X_PM1A_CNT: usize = 172;
    pub const X_PM1B_CNT: usize = 184;
    pub const X_PM_TMR: usize = 208;
    pub const X_GPE0: usize = 220;
    pub const SLEEP_CONTROL: usize = 244;
    pub const SLEEP_STATUS: usize = 256;
}

/// Read the FADT bytes `fadt` (header included). `None` when it isn't a FADT
/// or is too short to hold the flags. Bytes past the header's length, or past
/// the slice, read as zero, as ACPICA's copy into a zeroed FADT does.
pub fn parse_fadt(fadt: &[u8]) -> Option<FadtPower> {
    if fadt.get(..4)? != b"FACP" {
        return None;
    }
    let declared = u32::from_le_bytes(fadt.get(4..8)?.try_into().ok()?) as usize;
    let t = fadt.get(..declared.min(fadt.len()))?;
    if t.len() < off::FLAGS + 4 {
        return None;
    }
    let u8_at = |o: usize| t.get(o).copied().unwrap_or(0);
    let u32_at = |o: usize| match t.get(o..o + 4) {
        Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        None => 0,
    };
    let gas_at = |o: usize| -> Option<Gas> {
        let b = t.get(o..o + 12)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(&b[4..12]);
        let g = Gas {
            space: b[0],
            bit_width: b[1],
            bit_offset: b[2],
            access_size: b[3],
            address: u64::from_le_bytes(a),
        };
        (g.address != 0).then_some(g)
    };
    // The X_ form when its address is non-zero, else the 32-bit block as SystemIO.
    let reg = |o32: usize, len: u8, ox: usize| -> Option<Gas> {
        gas_at(ox).or_else(|| {
            let a = u32_at(o32);
            (a != 0).then(|| Gas::io(a as u64, len.saturating_mul(8)))
        })
    };
    let pm1_evt_len = u8_at(off::PM1_EVT_LEN);
    let pm1_cnt_len = u8_at(off::PM1_CNT_LEN);
    let gpe0_len = u8_at(off::GPE0_LEN);
    let flags = u32_at(off::FLAGS);
    let pm_timer = reg(off::PM_TMR, u8_at(off::PM_TMR_LEN), off::X_PM_TMR)
        .filter(|g| g.space == SPACE_SYSTEM_IO && io_range_ok(g.address, 4))
        .map(|g| (g.address, flags & TMR_VAL_EXT != 0));
    Some(FadtPower {
        revision: u8_at(8),
        flags,
        smi_cmd: u32_at(off::SMI_CMD),
        acpi_enable: u8_at(off::ACPI_ENABLE),
        pm1a_evt: reg(off::PM1A_EVT, pm1_evt_len, off::X_PM1A_EVT),
        pm1b_evt: reg(off::PM1B_EVT, pm1_evt_len, off::X_PM1B_EVT),
        pm1_evt_len,
        pm1a_cnt: reg(off::PM1A_CNT, pm1_cnt_len, off::X_PM1A_CNT),
        pm1b_cnt: reg(off::PM1B_CNT, pm1_cnt_len, off::X_PM1B_CNT),
        pm1_cnt_len,
        gpe0: reg(off::GPE0, gpe0_len, off::X_GPE0),
        gpe0_len,
        reset_reg: gas_at(off::RESET_REG),
        reset_value: u8_at(off::RESET_VALUE),
        sleep_control: gas_at(off::SLEEP_CONTROL),
        sleep_status: gas_at(off::SLEEP_STATUS),
        pm_timer,
    })
}

/// `bytes` bytes of I/O port space at `addr` fit the 16-bit port range. Every
/// SystemIO register a plan names passes this, so the port I/O that runs a
/// plan never truncates an address.
pub fn io_range_ok(addr: u64, bytes: u64) -> bool {
    addr.checked_add(bytes).is_some_and(|end| end <= 0x1_0000)
}

/// A PCI-configuration GAS address names segment 0, bus 0, a device (bits
/// 32-47) below 32, a function (16-31) below 8, and an offset (0-15) in the
/// 256-byte conventional space; bits 48-63 are zero (ACPI 6.5 §5.2.3.2).
pub fn pci_config_ok(addr: u64) -> bool {
    addr >> 48 == 0
        && (addr >> 32) & 0xffff < 32
        && (addr >> 16) & 0xffff < 8
        && addr & 0xffff < 0x100
}

/// How many PM timer ticks make `us` microseconds.
pub fn pm_timer_ticks(us: u64) -> u64 {
    us.saturating_mul(PM_TIMER_HZ) / 1_000_000
}

/// Ticks elapsed between two PM timer reads, for a 24- or 32-bit counter that
/// wraps (it may wrap once between reads; the caller polls far faster).
pub fn pm_timer_elapsed(start: u32, now: u32, ext32: bool) -> u64 {
    let mask: u32 = if ext32 { u32::MAX } else { 0x00ff_ffff };
    (now.wrapping_sub(start) & mask) as u64
}

/// A running total of PM timer ticks across many reads. One start-to-now
/// difference can't measure a wait longer than one wrap (a 24-bit counter
/// wraps every 4.69 s, so a 10 s wait would never end); summing each step
/// between consecutive reads can, as long as the caller reads at least once
/// per wrap.
#[derive(Clone, Copy, Debug)]
pub struct PmTicks {
    last: u32,
    total: u64,
    ext32: bool,
}

impl PmTicks {
    pub fn start(first_read: u32, ext32: bool) -> PmTicks {
        PmTicks {
            last: first_read,
            total: 0,
            ext32,
        }
    }

    /// Add the ticks since the last read and return the total so far.
    pub fn advance(&mut self, read: u32) -> u64 {
        self.total = self
            .total
            .saturating_add(pm_timer_elapsed(self.last, read, self.ext32));
        self.last = read;
        self.total
    }
}

/// Wait until `done` holds or `us` microseconds pass on the PM timer that
/// `read` reads; returns whether `done` held. The kernel's delays are this loop
/// over the real port, so the host KAT runs the same code against a model timer.
pub fn pm_wait(
    us: u64,
    ext32: bool,
    mut read: impl FnMut() -> u32,
    mut done: impl FnMut() -> bool,
) -> bool {
    let want = pm_timer_ticks(us);
    let mut ticks = PmTicks::start(read(), ext32);
    loop {
        if done() {
            return true;
        }
        if ticks.advance(read()) >= want {
            return done();
        }
    }
}

/// The highest physical address a SystemMemory register may name: 52 bits,
/// the x86-64 architectural MAXPHYADDR limit (so `PhysAddr::new` accepts it),
/// less the 8-bit register itself.
pub const PHYS_ADDR_LIMIT: u64 = 1 << 52;

/// The lowest canonical higher-half address with 4-level paging (48-bit
/// virtual addresses): bits 47-63 all set. The kernel requests no other
/// paging mode from Limine, and `VirtAddr::new` takes 48-bit addresses.
pub const HIGHER_HALF_START: u64 = 0xffff_8000_0000_0000;

/// A one-byte SystemMemory register at `addr` is a valid physical address,
/// and the kernel can reach its page through the direct map at
/// `hhdm_offset` (`mm::phys_to_virt` is `phys + hhdm_offset`). #154 F2
/// (Jarvis): a 52-bit physical address can still overflow that sum, or land
/// on a non-canonical address, so the sum is checked: it must not overflow
/// and must be a canonical higher-half address, for the page's first and last
/// byte. Anything else is refused here, so `init`'s premap never computes it.
pub fn phys_ok(addr: u64, hhdm_offset: u64) -> bool {
    if addr == 0 || addr >= PHYS_ADDR_LIMIT {
        return false;
    }
    let page = addr & !0xfff;
    match (
        page.checked_add(hhdm_offset),
        (page | 0xfff).checked_add(hhdm_offset),
    ) {
        (Some(first), Some(last)) => first >= HIGHER_HALF_START && last >= HIGHER_HALF_START,
        _ => false,
    }
}

/// #140 F1 (Janus): a hardware-reduced SLEEP_CONTROL or SLEEP_STATUS register
/// has the geometry ACPI 6.5 §4.8.3.7 gives it: 8 bits wide at bit offset 0,
/// accessed a byte at a time (access size 0, undefined, or 1, byte). The plan
/// writes it with one raw byte, which can't honour a wider or shifted field,
/// so any other geometry is refused, not written.
pub fn sleep_register_geometry_ok(g: &Gas) -> bool {
    g.bit_width == 8 && g.bit_offset == 0 && g.access_size <= 1
}

// ---------------------------------------------------------------------------
// PM1 and sleep-control values
// ---------------------------------------------------------------------------

/// PM1 control: SCI_EN (bit 0).
pub const SCI_EN: u16 = 0x0001;
/// PM1 control: SLP_TYP (bits 10-12).
pub const SLP_TYP_MASK: u16 = 0x1C00;
pub const SLP_TYP_SHIFT: u32 = 10;
/// PM1 control: SLP_EN (bit 13).
pub const SLP_EN: u16 = 0x2000;
/// PM1 status: every fixed status bit, written to clear them (ACPICA
/// `ACPI_BITMASK_ALL_FIXED_STATUS`). Bit 11 is never written (it's preserved).
pub const PM1_ALL_FIXED_STATUS: u16 = 0xC731;
/// PM1 status: WAK_STS (bit 15).
pub const WAK_STS: u16 = 0x8000;
/// The largest SLP_TYP (three bits; ACPICA `ACPI_SLEEP_TYPE_MAX`).
pub const SLEEP_TYPE_MAX: u8 = 7;

/// The two PM1 control writes for a legacy sleep, from the register's current
/// value: first SLP_TYP alone, then SLP_TYP with SLP_EN. Every other bit of
/// `current` (SCI_EN among them) is kept, as `AcpiHwLegacySleep` keeps them.
///
/// If the machine is still running 10 s later, P2 repeats the second value.
/// That differs on purpose from ACPICA's retry (`hwsleep.c`), which writes
/// SLP_EN through `AcpiHwRegisterWrite`: that keeps only
/// `ACPI_PM1_CONTROL_PRESERVED_BITS` (0xC3F8), so it clears SCI_EN and writes
/// SLP_TYP 0.
pub fn pm1_sleep_values(current: u16, slp_typ: u8) -> [u16; 2] {
    let typ = ((slp_typ & SLEEP_TYPE_MAX) as u16) << SLP_TYP_SHIFT;
    let first = (current & !(SLP_TYP_MASK | SLP_EN)) | typ;
    [first, first | SLP_EN]
}

/// The hardware-reduced SLEEP_CONTROL value: SLP_TYP at bits 2-4, SLP_EN bit 5
/// (ACPICA `ACPI_X_SLEEP_TYPE_MASK`, `ACPI_X_SLEEP_ENABLE`).
pub fn sleep_control_value(slp_typ: u8) -> u8 {
    ((slp_typ << 2) & 0x1C) | 0x20
}

/// SLEEP_STATUS: WAK_STS (bit 7), written to clear it.
pub const X_WAK_STS: u8 = 0x80;

/// The SMI command that puts the machine in ACPI mode, if it isn't and the
/// FADT says how: `(SMI_CMD port, ACPI_ENABLE value)`.
pub fn acpi_enable_command(fadt: &FadtPower, pm1a_cnt_now: u16) -> Option<(u32, u8)> {
    (pm1a_cnt_now & SCI_EN == 0
        && fadt.smi_cmd != 0
        && io_range_ok(fadt.smi_cmd as u64, 1)
        && fadt.acpi_enable != 0)
        .then_some((fadt.smi_cmd, fadt.acpi_enable))
}

/// A register block split into its status and enable halves (PM1 event, GPE0):
/// the status half at the block's address, the enable half after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusEnable {
    pub status: Gas,
    pub enable: Gas,
    /// Each half's length in bytes.
    pub half_len: u8,
}

/// Split a SystemIO event block of `len` bytes. `None` for an absent or empty
/// block, an odd length, or a block outside SystemIO (the MVP writes only ports).
pub fn split_block(block: Option<Gas>, len: u8) -> Option<StatusEnable> {
    let b = block?;
    if b.space != SPACE_SYSTEM_IO
        || len == 0
        || !len.is_multiple_of(2)
        || !io_range_ok(b.address, len as u64)
    {
        return None;
    }
    let half = len / 2;
    Some(StatusEnable {
        status: Gas::io(b.address, half.saturating_mul(8)),
        enable: Gas::io(b.address + half as u64, half.saturating_mul(8)),
        half_len: half,
    })
}

// ---------------------------------------------------------------------------
// The power-off plan
// ---------------------------------------------------------------------------

/// `\_S5`'s two SLP_TYP values: for PM1a (and SLEEP_CONTROL), and for PM1b.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepType {
    pub a: u8,
    pub b: u8,
}

/// Why the plan is the "safe to turn off" screen instead of a write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoPowerOff {
    /// The static `\_S5` decode failed closed.
    S5(S5Error),
    /// Legacy: no PM1a control block.
    NoPm1a,
    /// Legacy: a PM1 control block outside SystemIO, or not 16 bits wide.
    UnsupportedPm1,
    /// Hardware-reduced: SLEEP_CONTROL or SLEEP_STATUS is missing, or outside
    /// SystemIO and SystemMemory (or, for SystemMemory, not reachable through
    /// the direct map, #154 F2).
    NoSleepRegisters,
    /// Hardware-reduced: SLEEP_CONTROL or SLEEP_STATUS is present, but not an
    /// 8-bit, offset-0, byte-accessed register (#140 F1).
    UnsupportedSleepRegisters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerOffPlan {
    /// PM1: write `pm1_sleep_values(read, typ.a)` to PM1a (and `typ.b` to PM1b).
    Legacy {
        pm1a: Gas,
        pm1b: Option<Gas>,
        typ: SleepType,
        pm1a_evt: Option<StatusEnable>,
        pm1b_evt: Option<StatusEnable>,
        gpe0: Option<StatusEnable>,
    },
    /// Clear SLEEP_STATUS.WAK_STS, then write `value` to SLEEP_CONTROL.
    HwReduced {
        control: Gas,
        status: Gas,
        value: u8,
    },
    SafeToTurnOff(NoPowerOff),
}

/// Plan the S5 entry from the FADT and the `\_S5` decode. `hhdm_offset` is the
/// direct map's offset (`mm::hhdm_offset`), for the SystemMemory check.
pub fn plan_power_off(
    fadt: &FadtPower,
    s5: Result<SleepType, S5Error>,
    hhdm_offset: u64,
) -> PowerOffPlan {
    let typ = match s5 {
        Ok(t) => t,
        Err(e) => return PowerOffPlan::SafeToTurnOff(NoPowerOff::S5(e)),
    };
    if fadt.hw_reduced() {
        // AcpiHwExtendedSleep needs both registers.
        let usable = |g: Option<Gas>| {
            g.filter(|g| {
                (g.space == SPACE_SYSTEM_IO && io_range_ok(g.address, 1))
                    || (g.space == SPACE_SYSTEM_MEMORY && phys_ok(g.address, hhdm_offset))
            })
        };
        let (Some(control), Some(status)) = (usable(fadt.sleep_control), usable(fadt.sleep_status))
        else {
            return PowerOffPlan::SafeToTurnOff(NoPowerOff::NoSleepRegisters);
        };
        if !sleep_register_geometry_ok(&control) || !sleep_register_geometry_ok(&status) {
            return PowerOffPlan::SafeToTurnOff(NoPowerOff::UnsupportedSleepRegisters);
        }
        return PowerOffPlan::HwReduced {
            control,
            status,
            value: sleep_control_value(typ.a),
        };
    }
    let Some(pm1a) = fadt.pm1a_cnt else {
        return PowerOffPlan::SafeToTurnOff(NoPowerOff::NoPm1a);
    };
    // A GAS width of 0 falls back to PM1_CNT_LEN (the HP's and the Zenbook's X_GPE0
    // carry width 0 too).
    let width = |g: &Gas| match g.bit_width {
        0 => fadt.pm1_cnt_len.saturating_mul(8),
        w => w,
    };
    let ok16 = |g: &Gas| g.space == SPACE_SYSTEM_IO && width(g) == 16 && io_range_ok(g.address, 2);
    if !ok16(&pm1a) || fadt.pm1b_cnt.is_some_and(|g| !ok16(&g)) {
        return PowerOffPlan::SafeToTurnOff(NoPowerOff::UnsupportedPm1);
    }
    PowerOffPlan::Legacy {
        pm1a,
        pm1b: fadt.pm1b_cnt,
        typ,
        pm1a_evt: split_block(fadt.pm1a_evt, fadt.pm1_evt_len),
        pm1b_evt: split_block(fadt.pm1b_evt, fadt.pm1_evt_len),
        gpe0: split_block(fadt.gpe0, fadt.gpe0_len),
    }
}

// ---------------------------------------------------------------------------
// The restart chain (THINK §4)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetStep {
    /// Write `value` to the FADT reset register; an 8-bit write for SystemIO.
    FadtRegister { reg: Gas, value: u8 },
    /// `0xFE` to the i8042 command port `0x64`.
    Keyboard,
    /// Port `0xCF9`: `cf9_values(read)`.
    Cf9,
    /// A zero-limit IDT, then `int3`.
    TripleFault,
}

/// OT-90 P3b (Janus's #6 re-check, item 2): whether `step` uses a register
/// pair another CPU could be in the middle of using: a FADT reset register
/// in PCI configuration space goes through 0xCF8 (address) then 0xCFC (data),
/// and a CPU still running between the two writes redirects the reset byte
/// to its own function. With a K4 holdout such a step is skipped and the
/// chain goes on (the keyboard pulse, 0xCF9 and the triple fault are single
/// port writes, or no port at all).
// Used by the kernel only with `power-stop-cpus`.
#[allow(dead_code)]
pub fn step_uses_shared_ports(step: &ResetStep) -> bool {
    matches!(step, ResetStep::FadtRegister { reg, .. } if reg.space == SPACE_PCI_CONFIG)
}

/// The restart chain, in order. At most four steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResetChain {
    steps: [ResetStep; 4],
    len: usize,
}

impl ResetChain {
    pub fn steps(&self) -> &[ResetStep] {
        &self.steps[..self.len]
    }
}

pub const CF9_PORT: u64 = 0xCF9;

/// Plan the restart chain: the FADT reset register when it is supported and in
/// a space we can write (a SystemMemory one reachable through the direct map
/// at `hhdm_offset`), then the keyboard controller, then 0xCF9 (unless the
/// FADT register already was 0xCF9), then a triple fault.
pub fn reset_chain(fadt: &FadtPower, hhdm_offset: u64) -> ResetChain {
    let mut steps = [ResetStep::TripleFault; 4];
    let mut len = 0;
    let fadt_reg = fadt
        .reset_reg
        .filter(|_| fadt.flags & RESET_REG_SUP != 0)
        .filter(|g| match g.space {
            SPACE_SYSTEM_MEMORY => phys_ok(g.address, hhdm_offset),
            SPACE_SYSTEM_IO => io_range_ok(g.address, 1),
            SPACE_PCI_CONFIG => pci_config_ok(g.address),
            _ => false,
        });
    let mut cf9_done = false;
    if let Some(reg) = fadt_reg {
        cf9_done = reg.space == SPACE_SYSTEM_IO && reg.address == CF9_PORT;
        steps[len] = ResetStep::FadtRegister {
            reg,
            value: fadt.reset_value,
        };
        len += 1;
    }
    steps[len] = ResetStep::Keyboard;
    len += 1;
    if !cf9_done {
        steps[len] = ResetStep::Cf9;
        len += 1;
    }
    steps[len] = ResetStep::TripleFault;
    len += 1;
    ResetChain { steps, len }
}

/// The two 0xCF9 writes for a cold hard reset, from its current value: the
/// reset-type bits with SYS_RST (bit 1) alone, then with RST_CPU and FULL_RST.
pub fn cf9_values(current: u8) -> [u8; 2] {
    let base = current & !0x0E;
    [base | 0x02, base | 0x0E]
}

// ---------------------------------------------------------------------------
// The static \_S5 decode (THINK §3.2): a parse-only namespace walk
// ---------------------------------------------------------------------------

/// How the static `\_S5` decode failed closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S5Error {
    /// A table didn't walk to its end: `table` is its index, `at` the offset.
    Walk {
        table: usize,
        at: usize,
        fault: WalkFault,
    },
    /// No definition blocks to decode (no DSDT found).
    NoTables,
    /// The declaration pre-pass didn't settle within `MAX_PASSES`.
    NoSettle,
    /// The strict pass found other declarations than the pre-pass.
    PassesDisagree,
    /// The bytes `_S5_` occur this many times (not once) across the tables.
    Occurrences(usize),
    /// The walk reached this many root `Name (\_S5)` definitions (not one).
    RootDefinitions(usize),
    /// `\_S5` isn't a `Package` with a one-byte PkgLength.
    NotShortPackage,
    /// One of the first two elements isn't `ZeroOp`, `OneOp` or `BytePrefix`, or
    /// they run past the package.
    Element,
    /// NumElements < 2, or an SLP_TYP > 7.
    BadPackage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkFault {
    Truncated,
    PkgLength,
    NameSeg,
    ParentPastRoot,
    Opcode,
    StringByte,
    /// A body or field list didn't end exactly at its PkgLength.
    Overrun,
    MethodS5,
    ConditionalS5,
    /// A name used as a term resolves to no declaration.
    Unresolved,
    /// A name used as a term resolves to a definition inside a conditional
    /// body that the use isn't inside.
    ConditionalOnly,
    ConflictingArity,
    /// Nesting past `MAX_DEPTH`.
    TooDeep,
}

/// The pre-pass repeats at most this many times.
pub const MAX_PASSES: usize = 8;
/// Scope, body and expression nesting is bounded (kernel stack).
pub const MAX_DEPTH: usize = 64;
/// The definition-block header's length; AML starts after it.
const HEADER: usize = 36;

type Seg = [u8; 4];
type Path = Vec<Seg>;
/// A conditional body: (table index, offset of its If/Else/While opcode).
type Body = (usize, usize);
/// `None`: not a method. `Some(n)`: a method taking `n` arguments.
type Arity = Option<u8>;

#[derive(Clone, Default, PartialEq, Eq)]
struct Decls {
    unconditional: BTreeMap<Path, Arity>,
    conditional: BTreeMap<Path, BTreeMap<Body, Arity>>,
}

/// The root objects the OS predefines (ACPI 6.5 §5.3, §5.7).
const PREDEFINED: [(Seg, Arity); 9] = [
    (*b"_SB_", None),
    (*b"_GPE", None),
    (*b"_PR_", None),
    (*b"_TZ_", None),
    (*b"_SI_", None),
    (*b"_GL_", None),
    (*b"_OS_", None),
    (*b"_REV", None),
    (*b"_OSI", Some(1)),
];

const S5: Seg = *b"_S5_";

/// A parsed NameString: rooted, the number of `^` prefixes, and the segments.
struct NameString {
    root: bool,
    up: usize,
    segs: Path,
}

type WResult<T> = Result<T, (usize, WalkFault)>;

/// An argument kind in an opcode's shape: a TermArg, a SuperName or Target, a
/// NameString it declares, or a fixed-size immediate.
#[derive(Clone, Copy)]
enum Arg {
    T,
    S,
    N,
    B,
    W,
}

use Arg::{B, N, S, T, W};

/// The argument shapes of the one-byte Type 1 and Type 2 opcodes (ACPI 6.5 §20).
fn one_byte_shape(op: u8) -> Option<&'static [Arg]> {
    Some(match op {
        0x70 | 0x71 | 0x80 | 0x81 | 0x82 | 0x96 | 0x97 | 0x98 | 0x99 | 0x9c | 0x9d => &[T, S],
        0x72 | 0x73 | 0x74 | 0x77 | 0x79 | 0x7a | 0x7b | 0x7c | 0x7d | 0x7e | 0x7f | 0x84
        | 0x85 | 0x88 | 0x9e => &[T, T, S],
        0x75 | 0x76 | 0x87 | 0x8e => &[S],
        0x78 => &[T, T, S, S],
        0x83 | 0x92 | 0xa4 => &[T],
        0x86 => &[S, T],
        0x89 => &[T, B, T, B, T, T],
        0x90 | 0x91 | 0x93 | 0x94 | 0x95 => &[T, T],
        0x9f | 0xa3 | 0xa5 | 0xcc => &[],
        0x8a | 0x8b | 0x8c | 0x8d | 0x8f => &[T, T, N],
        _ => return None,
    })
}

/// The argument shapes of the `0x5B`-prefixed Type 1 and Type 2 opcodes.
fn ext_shape(op: u8) -> Option<&'static [Arg]> {
    Some(match op {
        0x12 | 0x1f | 0x20 | 0x24 | 0x25 | 0x26 | 0x27 => &[S],
        0x13 => &[T, T, T, N],
        0x21 | 0x22 => &[T],
        0x23 => &[S, W],
        0x28 | 0x29 => &[T, S],
        0x2a => &[T, T],
        0x30 | 0x33 => &[],
        0x88 => &[N, T, T, T],
        _ => return None,
    })
}

fn is_lead_char(c: u8) -> bool {
    c.is_ascii_uppercase() || c == b'_'
}

fn is_name_start(c: u8) -> bool {
    matches!(c, b'\\' | b'^' | 0x2e | 0x2f) || is_lead_char(c)
}

fn resolve(scope: &[Seg], ns: &NameString) -> WResult<Path> {
    let mut base: Path = if ns.root { Vec::new() } else { scope.to_vec() };
    if ns.up > base.len() {
        return Err((0, WalkFault::ParentPastRoot));
    }
    base.truncate(base.len() - ns.up);
    base.extend_from_slice(&ns.segs);
    Ok(base)
}

struct Walker<'a> {
    known: &'a Decls,
    strict: bool,
    seen: Decls,
    /// Each root `Name (\_S5)`: its table, its NameOp offset, and the offset
    /// of its object.
    found: Vec<(usize, usize, usize)>,
    table: usize,
    stack: Vec<Body>,
    depth: usize,
}

impl<'a> Walker<'a> {
    fn new(known: &'a Decls, strict: bool) -> Self {
        Walker {
            known,
            strict,
            seen: Decls::default(),
            found: Vec::new(),
            table: 0,
            stack: Vec::new(),
            depth: 0,
        }
    }

    fn byte(b: &[u8], i: usize) -> WResult<u8> {
        b.get(i).copied().ok_or((i, WalkFault::Truncated))
    }

    /// PkgLength at `i`: its value, and the offset after its encoding.
    fn pkglen(b: &[u8], i: usize) -> WResult<(usize, usize)> {
        let lead = Self::byte(b, i)?;
        let n = (lead >> 6) as usize;
        if n == 0 {
            return Ok(((lead & 0x3f) as usize, i + 1));
        }
        if lead & 0x30 != 0 {
            return Err((i, WalkFault::PkgLength));
        }
        let mut v = (lead & 0x0f) as usize;
        for k in 0..n {
            v |= (Self::byte(b, i + 1 + k)? as usize) << (4 + 8 * k);
        }
        Ok((v, i + 1 + n))
    }

    /// The end of the PkgLength-delimited object whose PkgLength is at `i`.
    fn pkg_end(b: &[u8], i: usize) -> WResult<(usize, usize)> {
        let (l, body) = Self::pkglen(b, i)?;
        let end = i.checked_add(l).ok_or((i, WalkFault::PkgLength))?;
        if l == 0 || end > b.len() || body > end {
            return Err((i, WalkFault::PkgLength));
        }
        Ok((end, body))
    }

    fn seg(b: &[u8], i: usize) -> WResult<(Seg, usize)> {
        let s = b.get(i..i + 4).ok_or((i, WalkFault::Truncated))?;
        let ok = is_lead_char(s[0])
            && s[1..]
                .iter()
                .all(|&c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_');
        if !ok {
            return Err((i, WalkFault::NameSeg));
        }
        Ok(([s[0], s[1], s[2], s[3]], i + 4))
    }

    fn namestring(b: &[u8], mut i: usize) -> WResult<(NameString, usize)> {
        let mut root = false;
        let mut up = 0;
        if Self::byte(b, i)? == b'\\' {
            root = true;
            i += 1;
        } else {
            while Self::byte(b, i)? == b'^' {
                up += 1;
                i += 1;
            }
        }
        let mut segs = Vec::new();
        match Self::byte(b, i)? {
            0x00 => i += 1,
            0x2e => {
                for _ in 0..2 {
                    let (s, j) = Self::seg(b, if segs.is_empty() { i + 1 } else { i })?;
                    segs.push(s);
                    i = j;
                }
            }
            0x2f => {
                let n = Self::byte(b, i + 1)?;
                i += 2;
                for _ in 0..n {
                    let (s, j) = Self::seg(b, i)?;
                    segs.push(s);
                    i = j;
                }
            }
            _ => {
                let (s, j) = Self::seg(b, i)?;
                segs.push(s);
                i = j;
            }
        }
        Ok((NameString { root, up, segs }, i))
    }

    fn resolve_at(scope: &[Seg], ns: &NameString, at: usize) -> WResult<Path> {
        resolve(scope, ns).map_err(|(_, f)| (at, f))
    }

    fn enter(&mut self, at: usize) -> WResult<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err((at, WalkFault::TooDeep));
        }
        Ok(())
    }

    fn declare(&mut self, path: Path, arity: Arity, external: bool, at: usize) -> WResult<()> {
        let slot = match self.stack.last() {
            Some(&body) if !external => self
                .seen
                .conditional
                .entry(path)
                .or_default()
                .entry(body)
                .or_insert(None),
            _ => self.seen.unconditional.entry(path).or_insert(None),
        };
        match (*slot, arity) {
            (Some(old), Some(new)) if old != new => Err((at, WalkFault::ConflictingArity)),
            (None, a) => {
                *slot = a;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// The argument count of a name used as a term, by the ACPI 6.5 §5.3 search
    /// rules (a lone NameSeg searches up toward the root).
    fn arity(&self, scope: &[Seg], ns: &NameString, at: usize) -> WResult<usize> {
        let mut cands: Vec<Path> = Vec::new();
        if ns.root || ns.up > 0 || ns.segs.len() != 1 {
            cands.push(Self::resolve_at(scope, ns, at)?);
        } else {
            for k in (0..=scope.len()).rev() {
                let mut p = scope[..k].to_vec();
                p.push(ns.segs[0]);
                cands.push(p);
            }
        }
        for p in &cands {
            if let Some(a) = self.known.unconditional.get(p) {
                return Ok(a.unwrap_or(0) as usize);
            }
            if p.len() == 1
                && let Some((_, a)) = PREDEFINED.iter().find(|(s, _)| *s == p[0])
            {
                return Ok(a.unwrap_or(0) as usize);
            }
            if let Some(bodies) = self.known.conditional.get(p) {
                if let Some(a) = self.stack.iter().rev().find_map(|bd| bodies.get(bd)) {
                    return Ok(a.unwrap_or(0) as usize);
                }
                if self.strict {
                    return Err((at, WalkFault::ConditionalOnly));
                }
                return Ok(0);
            }
        }
        if self.strict {
            return Err((at, WalkFault::Unresolved));
        }
        Ok(0)
    }

    fn termarg(&mut self, b: &[u8], i: usize, scope: &[Seg]) -> WResult<usize> {
        self.enter(i)?;
        let r = self.termarg_inner(b, i, scope);
        self.depth -= 1;
        r
    }

    fn termarg_inner(&mut self, b: &[u8], i: usize, scope: &[Seg]) -> WResult<usize> {
        let c = Self::byte(b, i)?;
        let next = Self::byte(b, i + 1).ok();
        match c {
            0x00 | 0x01 | 0xff => Ok(i + 1),
            0x0a => Ok(i + 2),
            0x0b => Ok(i + 3),
            0x0c => Ok(i + 5),
            0x0e => Ok(i + 9),
            0x0d => {
                let mut j = i + 1;
                loop {
                    match Self::byte(b, j)? {
                        0 => return Ok(j + 1),
                        x if x > 0x7f => return Err((j, WalkFault::StringByte)),
                        _ => j += 1,
                    }
                }
            }
            0x11..=0x13 => Ok(Self::pkg_end(b, i + 1)?.0),
            0x60..=0x6e => Ok(i + 1),
            0x5b if matches!(next, Some(0x30 | 0x31)) => Ok(i + 2), // Revision, Debug
            _ if is_name_start(c) => {
                let (ns, mut j) = Self::namestring(b, i)?;
                for _ in 0..self.arity(scope, &ns, i)? {
                    j = self.termarg(b, j, scope)?;
                }
                Ok(j)
            }
            _ => {
                if let Some(shape) = one_byte_shape(c) {
                    return self.args(b, i + 1, shape, scope);
                }
                if c == 0x5b
                    && let Some(shape) = next.and_then(ext_shape)
                {
                    return self.args(b, i + 2, shape, scope);
                }
                Err((i, WalkFault::Opcode))
            }
        }
    }

    fn supername(&mut self, b: &[u8], i: usize, scope: &[Seg]) -> WResult<usize> {
        let c = Self::byte(b, i)?;
        match c {
            0x00 | 0x60..=0x6e => Ok(i + 1),
            0x5b if Self::byte(b, i + 1)? == 0x31 => Ok(i + 2),
            _ if is_name_start(c) => Ok(Self::namestring(b, i)?.1),
            _ => self.termarg(b, i, scope), // RefOf, DerefOf, Index
        }
    }

    fn args(&mut self, b: &[u8], mut i: usize, shape: &[Arg], scope: &[Seg]) -> WResult<usize> {
        for a in shape {
            i = match a {
                T => self.termarg(b, i, scope)?,
                S => self.supername(b, i, scope)?,
                N => {
                    let (ns, j) = Self::namestring(b, i)?;
                    let p = Self::resolve_at(scope, &ns, i)?;
                    self.declare(p, None, false, i)?;
                    j
                }
                B => i + 1,
                W => i + 2,
            };
        }
        Ok(i)
    }

    fn fieldlist(&mut self, b: &[u8], mut i: usize, end: usize, scope: &[Seg]) -> WResult<()> {
        while i < end {
            i = match Self::byte(b, i)? {
                0x00 => Self::pkglen(b, i + 1)?.1,
                0x01 => i + 3,
                0x02 => {
                    if Self::byte(b, i + 1)? == 0x11 {
                        Self::pkg_end(b, i + 2)?.0
                    } else {
                        Self::namestring(b, i + 1)?.1
                    }
                }
                0x03 => i + 4,
                _ => {
                    let (s, j) = Self::seg(b, i)?;
                    let mut p = scope.to_vec();
                    p.push(s);
                    self.declare(p, None, false, i)?;
                    Self::pkglen(b, j)?.1
                }
            };
        }
        if i != end {
            return Err((i, WalkFault::Overrun));
        }
        Ok(())
    }

    fn walk(&mut self, b: &[u8], i: usize, end: usize, scope: &[Seg]) -> WResult<()> {
        self.enter(i)?;
        let r = self.walk_inner(b, i, end, scope);
        self.depth -= 1;
        r
    }

    fn walk_body(
        &mut self,
        b: &[u8],
        i: usize,
        end: usize,
        scope: &[Seg],
        at: usize,
    ) -> WResult<()> {
        self.stack.push((self.table, at));
        let r = self.walk(b, i, end, scope);
        self.stack.pop();
        r
    }

    fn walk_inner(&mut self, b: &[u8], mut i: usize, end: usize, scope: &[Seg]) -> WResult<()> {
        while i < end {
            let c = Self::byte(b, i)?;
            let ext = if c == 0x5b {
                Some(Self::byte(b, i + 1)?)
            } else {
                None
            };
            match (c, ext) {
                // Scope, Device, Processor, PowerResource, ThermalZone: enter.
                (0x10, _) | (0x5b, Some(0x82..=0x85)) => {
                    let j = if c == 0x10 { i + 1 } else { i + 2 };
                    let (pend, body) = Self::pkg_end(b, j)?;
                    let (ns, mut k) = Self::namestring(b, body)?;
                    let path = Self::resolve_at(scope, &ns, i)?;
                    if c == 0x5b {
                        self.declare(path.clone(), None, false, i)?;
                    }
                    match ext {
                        Some(0x83) => k += 6, // ProcID, PblkAddr, PblkLen
                        Some(0x84) => k += 3, // SystemLevel, ResourceOrder
                        _ => {}
                    }
                    self.walk(b, k, pend, &path)?;
                    i = pend;
                }
                // Method: record its arity; the body is never walked.
                (0x14, _) => {
                    let (pend, body) = Self::pkg_end(b, i + 1)?;
                    let (ns, k) = Self::namestring(b, body)?;
                    let path = Self::resolve_at(scope, &ns, i)?;
                    if path == [S5] {
                        return Err((i, WalkFault::MethodS5));
                    }
                    let flags = Self::byte(b, k)?;
                    self.declare(path, Some(flags & 7), false, i)?;
                    i = pend;
                }
                // If, While: the predicate, then a conditional body.
                (0xa0 | 0xa2, _) => {
                    let (pend, body) = Self::pkg_end(b, i + 1)?;
                    let k = self.termarg(b, body, scope)?;
                    self.walk_body(b, k, pend, scope, i)?;
                    i = pend;
                }
                // Else: a conditional body.
                (0xa1, _) => {
                    let (pend, body) = Self::pkg_end(b, i + 1)?;
                    self.walk_body(b, body, pend, scope, i)?;
                    i = pend;
                }
                // Buffer, Package, VarPackage: skipped by length, never scanned.
                (0x11..=0x13, _) => i = Self::pkg_end(b, i + 1)?.0,
                // Field, IndexField, BankField: declare the field units.
                (0x5b, Some(op @ (0x81 | 0x86 | 0x87))) => {
                    let (pend, body) = Self::pkg_end(b, i + 2)?;
                    let (_, mut k) = Self::namestring(b, body)?;
                    if op != 0x81 {
                        k = Self::namestring(b, k)?.1;
                    }
                    if op == 0x87 {
                        k = self.termarg(b, k, scope)?;
                    }
                    self.fieldlist(b, k + 1, pend, scope)?;
                    i = pend;
                }
                // Name: a root, unconditional \_S5 is the one accepted.
                (0x08, _) => {
                    let (ns, k) = Self::namestring(b, i + 1)?;
                    let path = Self::resolve_at(scope, &ns, i)?;
                    if path == [S5] {
                        if !self.stack.is_empty() {
                            return Err((i, WalkFault::ConditionalS5));
                        }
                        self.found.push((self.table, i, k));
                    }
                    self.declare(path, None, false, i)?;
                    i = self.termarg(b, k, scope)?;
                }
                // External: ObjectType 8 is a method, with its argument count. It
                // creates nothing, so it is unconditional wherever it sits, and an
                // External whose name can't resolve (iASL's `^` at the root)
                // declares nothing.
                (0x15, _) => {
                    let (ns, k) = Self::namestring(b, i + 1)?;
                    let ty = Self::byte(b, k)?;
                    let argc = Self::byte(b, k + 1)?;
                    if let Ok(path) = resolve(scope, &ns) {
                        let arity = (ty == 8).then_some(argc & 7);
                        self.declare(path, arity, true, i)?;
                    }
                    i = k + 2;
                }
                // Alias: the second name is declared.
                (0x06, _) => {
                    let (_, k) = Self::namestring(b, i + 1)?;
                    let (ns, k) = Self::namestring(b, k)?;
                    let path = Self::resolve_at(scope, &ns, i)?;
                    self.declare(path, None, false, i)?;
                    i = k;
                }
                // OperationRegion: name, space byte, offset, length.
                (0x5b, Some(0x80)) => {
                    let (ns, k) = Self::namestring(b, i + 2)?;
                    let path = Self::resolve_at(scope, &ns, i)?;
                    self.declare(path, None, false, i)?;
                    let k = self.termarg(b, k + 1, scope)?;
                    i = self.termarg(b, k, scope)?;
                }
                // Mutex (a SyncFlags byte), Event.
                (0x5b, Some(op @ (0x01 | 0x02))) => {
                    let (ns, k) = Self::namestring(b, i + 2)?;
                    let path = Self::resolve_at(scope, &ns, i)?;
                    self.declare(path, None, false, i)?;
                    i = k + usize::from(op == 0x01);
                }
                // Any other Type 1 or Type 2 statement.
                _ => i = self.termarg(b, i, scope)?,
            }
        }
        if i != end {
            return Err((i, WalkFault::Overrun));
        }
        Ok(())
    }
}

/// One pass over every table. Returns the walker and each table's outcome.
fn pass<'a>(tables: &[&[u8]], known: &'a Decls, strict: bool) -> (Walker<'a>, Vec<WResult<()>>) {
    let mut w = Walker::new(known, strict);
    let mut per = Vec::with_capacity(tables.len());
    for (n, b) in tables.iter().enumerate() {
        w.table = n;
        w.stack.clear();
        w.depth = 0;
        let r = if b.len() < HEADER {
            Err((b.len(), WalkFault::Truncated))
        } else {
            w.walk(b, HEADER, b.len(), &[])
        };
        per.push(r);
    }
    (w, per)
}

/// The per-table outcome of the strict pass, for the KATs and the boot log.
pub type TableWalk = Result<(), (usize, WalkFault)>;

/// A decoded `\_S5`, and where its `Name` is: the table's index and the
/// NameOp's offset in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct S5Found {
    pub typ: SleepType,
    pub table: usize,
    pub at: usize,
}

/// Decode `\_S5` from the definition blocks (the DSDT first, then the SSDTs in
/// XSDT order, headers included). Also returns each table's strict-pass
/// outcome; it is empty when the pre-pass didn't settle.
pub fn decode_s5_report(tables: &[&[u8]]) -> (Result<S5Found, S5Error>, Vec<TableWalk>) {
    let mut known = Decls::default();
    let mut settled = false;
    for _ in 0..MAX_PASSES {
        let (w, _) = pass(tables, &known, false);
        if w.seen == known {
            settled = true;
            break;
        }
        known = w.seen;
    }
    if !settled {
        return (Err(S5Error::NoSettle), Vec::new());
    }
    let (w, per) = pass(tables, &known, true);
    let result = finish(tables, &known, &w, &per);
    (result, per)
}

/// Decode `\_S5`; see `decode_s5_report`.
pub fn decode_s5(tables: &[&[u8]]) -> Result<SleepType, S5Error> {
    decode_s5_report(tables).0.map(|f| f.typ)
}

fn finish(
    tables: &[&[u8]],
    known: &Decls,
    w: &Walker,
    per: &[WResult<()>],
) -> Result<S5Found, S5Error> {
    if let Some((table, &Err((at, fault)))) = per.iter().enumerate().find(|(_, r)| r.is_err()) {
        return Err(S5Error::Walk { table, at, fault });
    }
    if w.seen != *known {
        return Err(S5Error::PassesDisagree);
    }
    let occurrences: usize = tables
        .iter()
        .map(|b| {
            b.get(HEADER..)
                .map_or(0, |aml| aml.windows(4).filter(|x| *x == S5).count())
        })
        .sum();
    if occurrences != 1 {
        return Err(S5Error::Occurrences(occurrences));
    }
    let &[(table, name_at, k)] = w.found.as_slice() else {
        return Err(S5Error::RootDefinitions(w.found.len()));
    };
    let b = tables[table];
    let at = |i: usize| b.get(i).copied();
    if at(k) != Some(0x12) {
        return Err(S5Error::NotShortPackage);
    }
    let len = match at(k + 1) {
        Some(l) if l >> 6 == 0 => l as usize,
        _ => return Err(S5Error::NotShortPackage),
    };
    let end = k + 1 + len;
    if at(k + 2).ok_or(S5Error::BadPackage)? < 2 {
        return Err(S5Error::BadPackage);
    }
    let mut j = k + 3;
    let mut vals = [0u8; 2];
    for v in &mut vals {
        (*v, j) = match at(j) {
            Some(0x00) => (0, j + 1),
            Some(0x01) => (1, j + 1),
            Some(0x0a) => (at(j + 1).ok_or(S5Error::Element)?, j + 2),
            _ => return Err(S5Error::Element),
        };
    }
    if j > end {
        return Err(S5Error::Element);
    }
    if vals.iter().any(|&v| v > SLEEP_TYPE_MAX) {
        return Err(S5Error::BadPackage);
    }
    Ok(S5Found {
        typ: SleepType {
            a: vals[0],
            b: vals[1],
        },
        table,
        at: name_at,
    })
}
