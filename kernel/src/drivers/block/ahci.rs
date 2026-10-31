// SPDX-License-Identifier: MPL-2.0
//! AHCI 1.0 block backend for Phase 3 BUILD-5a.
//!
//! The initial driver targets QEMU's ICH9 AHCI controller. It deliberately
//! uses one command slot per port and non-NCQ ATA commands, but it registers
//! through the generic `BlockDevice` trait so existing filesystems compose
//! without knowing whether the backing disk is virtio-blk or SATA.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr::{read_volatile, write_bytes, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering, fence};

use spin::Lazy;
use x86_64::VirtAddr;

use crate::drivers::block::{
    self, BlockDevice, BlockDeviceIdentity, BlockError, BlockGeometry, BlockOp, BlockRequest,
    BlockResult, DmaBuffer,
};
use crate::drivers::pci::{self, Device};
use crate::serial_println;
use crate::sync::{IrqSafeMutex, SwitchMutex};

pub const AHCI_MSI_VECTOR: u8 = 0xf0;

const AHCI_BAR: u8 = 5;
const AHCI_MMIO_LEN: u32 = 0x2000;

const HBA_CAP: usize = 0x00;
const HBA_GHC: usize = 0x04;
const HBA_IS: usize = 0x08;
const HBA_PI: usize = 0x0c;
const HBA_GHC_AE: u32 = 1 << 31;
const HBA_GHC_IE: u32 = 1 << 1;
const HBA_PORT_BASE: usize = 0x100;
const HBA_PORT_STRIDE: usize = 0x80;

const PX_CLB: usize = 0x00;
const PX_FB: usize = 0x08;
const PX_IS: usize = 0x10;
const PX_IE: usize = 0x14;
const PX_CMD: usize = 0x18;
const PX_TFD: usize = 0x20;
const PX_SIG: usize = 0x24;
const PX_SSTS: usize = 0x28;
const PX_SERR: usize = 0x30;
const PX_CI: usize = 0x38;

const PX_CMD_ST: u32 = 1 << 0;
const PX_CMD_FRE: u32 = 1 << 4;
const PX_CMD_FR: u32 = 1 << 14;
const PX_CMD_CR: u32 = 1 << 15;

const PX_IS_TFES: u32 = 1 << 30;
const PX_IE_ERRORS_AND_COMPLETIONS: u32 = 0x7dc0_0007;
const PX_TFD_ERR: u32 = 1 << 0;
const PX_TFD_DRQ: u32 = 1 << 3;
const PX_TFD_BSY: u32 = 1 << 7;

const SATA_DET_PRESENT: u32 = 3;
/// OT-87 R4.1: PxSERR.DIAG.X (Exchanged: a COMINIT, i.e. a device was
/// connected or replaced) and PxSERR.DIAG.N (PhyRdy changed), AHCI 1.3.1
/// §3.3.12. Both stay set until written back, so a change between commands is
/// still seen at the next one.
#[cfg(feature = "installer")]
const PX_SERR_DIAG_X: u32 = 1 << 26;
#[cfg(feature = "installer")]
const PX_SERR_DIAG_N: u32 = 1 << 16;
const SATA_IPM_ACTIVE: u32 = 1;
const SATA_SIG_ATA: u32 = 0x0000_0101;

const FIS_TYPE_REG_H2D: u8 = 0x27;
const FIS_REG_H2D_COMMAND: u8 = 1 << 7;

const ATA_IDENTIFY_DEVICE: u8 = 0xec;
const ATA_READ_DMA_EXT: u8 = 0x25;
const ATA_WRITE_DMA_EXT: u8 = 0x35;
const ATA_FLUSH_CACHE_EXT: u8 = 0xea;

const SECTOR_SIZE: u32 = 512;
const COMMAND_SLOT: u32 = 0;
const COMMAND_HEADER_BYTES: usize = 32;
const COMMAND_LIST_BYTES: usize = COMMAND_HEADER_BYTES * 32;
const FIS_BYTES: usize = 256;
const COMMAND_TABLE_BYTES: usize = 4096;
const COMMAND_TABLE_CFIS: usize = 0x00;
const COMMAND_TABLE_PRDT: usize = 0x80;
const PRDT_BYTES: usize = 16;
const MAX_PRDT_ENTRIES: usize = 8;
const COMMAND_TIMEOUT_SPINS: usize = 10_000_000;
const PORT_IDLE_TIMEOUT_SPINS: usize = 1_000_000;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ProbeError {
    MissingAbar,
    NoImplementedPorts,
    NoDevicePort,
    Dma,
    Identify,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
#[allow(dead_code)]
pub struct ProbeSnapshot {
    pub seen: bool,
    pub probe_ok: bool,
    pub abar_mapped: bool,
    pub msi_configured: bool,
    pub ports_implemented: u32,
    pub port_index: u8,
    pub identify_ok: bool,
    pub sector_size: u32,
    pub sector_count: u64,
    pub last_bdf: u32,
    pub registered_index: u32,
    pub interrupt_count: u64,
}

static SEEN: AtomicBool = AtomicBool::new(false);
static PROBE_OK: AtomicBool = AtomicBool::new(false);
static ABAR_MAPPED: AtomicBool = AtomicBool::new(false);
static MSI_CONFIGURED: AtomicBool = AtomicBool::new(false);
static IDENTIFY_OK: AtomicBool = AtomicBool::new(false);
static PORTS_IMPLEMENTED: AtomicU32 = AtomicU32::new(0);
static PORT_INDEX: AtomicU32 = AtomicU32::new(u32::MAX);
static SECTOR_COUNT: AtomicU64 = AtomicU64::new(0);
static LAST_BDF: AtomicU32 = AtomicU32::new(0);
static REGISTERED_INDEX: AtomicU32 = AtomicU32::new(u32::MAX);
static INTERRUPTS: AtomicU64 = AtomicU64::new(0);

/// AHCI device list lock.
///
/// Mixed process/interrupt context: PCI probe mutates it from process context,
/// and the MSI handler reads it after IF=0 interrupt-gate entry. Use
/// `IrqSafeMutex` so the process-side acquisition cannot be preempted by the
/// same lock's handler-side acquisition.
static AHCI_DEVICES: Lazy<IrqSafeMutex<Vec<Arc<AhciDevice>>>> =
    Lazy::new(|| IrqSafeMutex::new(Vec::new()));

pub fn probe_pci(dev: Device) -> Result<(), ProbeError> {
    if PROBE_OK.load(Ordering::Acquire) {
        SEEN.store(true, Ordering::Release);
        return Err(ProbeError::NoDevicePort);
    }
    reset_probe_state(dev);
    pci::enable_memory_and_bus_mastering(dev);

    let abar =
        pci::map_bar_region(dev, AHCI_BAR, 0, AHCI_MMIO_LEN).ok_or(ProbeError::MissingAbar)?;
    ABAR_MAPPED.store(true, Ordering::Release);

    let cap = unsafe { mmio_read32(abar, HBA_CAP) };
    let pi = unsafe { mmio_read32(abar, HBA_PI) };
    PORTS_IMPLEMENTED.store(pi, Ordering::Release);
    if pi == 0 {
        return Err(ProbeError::NoImplementedPorts);
    }

    unsafe {
        let ghc = mmio_read32(abar, HBA_GHC);
        mmio_write32(abar, HBA_GHC, ghc | HBA_GHC_AE);
    }

    let msi = pci::configure_msi_fixed_vector(dev, AHCI_MSI_VECTOR);
    MSI_CONFIGURED.store(msi, Ordering::Release);

    for port in 0..32u8 {
        if pi & (1u32 << port) == 0 {
            continue;
        }
        let port_base = port_base(abar, port);
        if !port_has_sata_device(port_base) {
            continue;
        }
        let device = init_port_device(dev, abar, port_base, port, cap, msi)?;
        let registered_index = block::register_device(device.clone());
        REGISTERED_INDEX.store(registered_index as u32, Ordering::Release);
        AHCI_DEVICES.lock().push(device);
        PROBE_OK.store(true, Ordering::Release);
        return Ok(());
    }

    Err(ProbeError::NoDevicePort)
}

fn reset_probe_state(dev: Device) {
    SEEN.store(true, Ordering::Release);
    PROBE_OK.store(false, Ordering::Release);
    ABAR_MAPPED.store(false, Ordering::Release);
    MSI_CONFIGURED.store(false, Ordering::Release);
    IDENTIFY_OK.store(false, Ordering::Release);
    PORTS_IMPLEMENTED.store(0, Ordering::Release);
    PORT_INDEX.store(u32::MAX, Ordering::Release);
    SECTOR_COUNT.store(0, Ordering::Release);
    LAST_BDF.store(pack_bdf(dev), Ordering::Release);
    REGISTERED_INDEX.store(u32::MAX, Ordering::Release);
}

fn init_port_device(
    dev: Device,
    abar: VirtAddr,
    port_base: VirtAddr,
    port: u8,
    _cap: u32,
    msi_configured: bool,
) -> Result<Arc<AhciDevice>, ProbeError> {
    stop_port(port_base);

    let command_list = DmaBuffer::alloc_zeroed(COMMAND_LIST_BYTES).map_err(|_| ProbeError::Dma)?;
    let fis = DmaBuffer::alloc_zeroed(FIS_BYTES).map_err(|_| ProbeError::Dma)?;
    let command_table =
        DmaBuffer::alloc_zeroed(COMMAND_TABLE_BYTES).map_err(|_| ProbeError::Dma)?;

    unsafe {
        write_u64_split(port_base, PX_CLB, command_list.physical_addr().as_u64());
        write_u64_split(port_base, PX_FB, fis.physical_addr().as_u64());
        mmio_write32(port_base, PX_SERR, u32::MAX);
        mmio_write32(port_base, PX_IS, u32::MAX);
        mmio_write32(port_base, PX_IE, 0);
    }

    start_port(port_base);

    let geometry = identify_port(port_base, &command_list, &command_table)
        .map_err(|_| ProbeError::Identify)?;

    if msi_configured {
        unsafe {
            mmio_write32(port_base, PX_IE, PX_IE_ERRORS_AND_COMPLETIONS);
            let ghc = mmio_read32(abar, HBA_GHC);
            mmio_write32(abar, HBA_GHC, ghc | HBA_GHC_AE | HBA_GHC_IE);
        }
    }

    PORT_INDEX.store(port as u32, Ordering::Release);
    IDENTIFY_OK.store(true, Ordering::Release);
    SECTOR_COUNT.store(geometry.block_count, Ordering::Release);

    #[cfg(feature = "installer")]
    let media = Arc::new(block::MediaTracker::new());
    let bdf = dev.bdf();
    let identity = BlockDeviceIdentity {
        driver: "ahci",
        name: "ahci0p0",
        pci_bdf: Some(bdf),
    };
    serial_println!(
        "[ kernel ] ahci: attached bdf={:02x}:{:02x}.{} port={} sectors={} sector_size={} msi_configured={}",
        bdf.0,
        bdf.1,
        bdf.2,
        port,
        geometry.block_count,
        geometry.logical_block_size,
        msi_configured
    );

    Ok(Arc::new(AhciDevice {
        identity,
        geometry,
        abar,
        port_base,
        port_index: port,
        inner: SwitchMutex::new(AhciPortInner {
            command_list,
            _fis: fis,
            command_table,
            #[cfg(feature = "installer")]
            media: media.clone(),
        }),
        #[cfg(feature = "installer")]
        media,
    }))
}

#[allow(dead_code)]
pub fn last_probe_snapshot() -> ProbeSnapshot {
    ProbeSnapshot {
        seen: SEEN.load(Ordering::Acquire),
        probe_ok: PROBE_OK.load(Ordering::Acquire),
        abar_mapped: ABAR_MAPPED.load(Ordering::Acquire),
        msi_configured: MSI_CONFIGURED.load(Ordering::Acquire),
        ports_implemented: PORTS_IMPLEMENTED.load(Ordering::Acquire),
        port_index: PORT_INDEX.load(Ordering::Acquire) as u8,
        identify_ok: IDENTIFY_OK.load(Ordering::Acquire),
        sector_size: SECTOR_SIZE,
        sector_count: SECTOR_COUNT.load(Ordering::Acquire),
        last_bdf: LAST_BDF.load(Ordering::Acquire),
        registered_index: REGISTERED_INDEX.load(Ordering::Acquire),
        interrupt_count: INTERRUPTS.load(Ordering::Acquire),
    }
}

pub fn handle_interrupt() {
    INTERRUPTS.fetch_add(1, Ordering::AcqRel);
    let devices = AHCI_DEVICES.lock();
    for device in devices.iter() {
        device.ack_interrupts();
    }
}

struct AhciDevice {
    identity: BlockDeviceIdentity,
    geometry: BlockGeometry,
    abar: VirtAddr,
    port_base: VirtAddr,
    port_index: u8,
    /// Lock type: `SwitchMutex<AhciPortInner>`.
    /// IF discipline: submitters may hold this lock with IF=1 while a
    /// single non-NCQ command is in flight. The MSI handler does not take
    /// this lock; it only acknowledges interrupt status from immutable
    /// register addresses, avoiding interrupt-time self-deadlock.
    inner: SwitchMutex<AhciPortInner>,
    #[cfg(feature = "installer")]
    media: Arc<block::MediaTracker>,
}

struct AhciPortInner {
    command_list: Arc<DmaBuffer>,
    _fis: Arc<DmaBuffer>,
    command_table: Arc<DmaBuffer>,
    /// OT-87 R4.1: the device's tracker, so a command notes PxSERR.DIAG.X/N
    /// before it clears PxSERR.
    #[cfg(feature = "installer")]
    media: Arc<block::MediaTracker>,
}

impl BlockDevice for AhciDevice {
    fn identity(&self) -> &BlockDeviceIdentity {
        &self.identity
    }

    fn geometry(&self) -> BlockGeometry {
        self.geometry
    }

    fn submit(&self, request: BlockRequest<'_>) -> BlockResult<()> {
        match request.op {
            BlockOp::Read | BlockOp::Write => self.submit_data(request),
            BlockOp::Flush => {
                if !request.buffers.is_empty() || request.lba != 0 {
                    return Err(BlockError::InvalidRequest);
                }
                self.flush()
            }
        }
    }

    fn supports_flush(&self) -> bool {
        true
    }

    fn flush(&self) -> BlockResult<()> {
        let mut inner = self.inner.lock();
        inner.issue_command(self.port_base, ATA_FLUSH_CACHE_EXT, false, 0, 0, &[])
    }

    /// OT-87 R4.1: IDENTIFY DEVICE again: the serial (words 10–19) and model
    /// (words 27–46).
    #[cfg(feature = "installer")]
    fn driver_identity(&self) -> Option<block::claim_logic::DriverIdentity> {
        let identify = DmaBuffer::alloc_zeroed(SECTOR_SIZE as usize).ok()?;
        {
            let mut inner = self.inner.lock();
            inner
                .issue_command(
                    self.port_base,
                    ATA_IDENTIFY_DEVICE,
                    false,
                    0,
                    0,
                    core::slice::from_ref(&identify),
                )
                .ok()?;
        }
        // SAFETY: the command completed; nothing else holds the buffer.
        let words = unsafe { identify.as_slice() };
        let (serial, model) = block::claim_logic::ata_serial_model(words);
        let (sectors, sector_size) = block::claim_logic::ata_capacity(words);
        Some(block::claim_logic::DriverIdentity {
            model,
            serial,
            capacity_bytes: sectors.saturating_mul(sector_size as u64),
            sector_size,
            // ATA has a World Wide Name (words 108..111); the claim does not
            // use it yet.
            namespace_ids: None,
        })
    }

    /// OT-87 R4.1: no device on the port (PxSSTS.DET is not "present, PHY
    /// established") is dead; PxSERR.DIAG.X or N (a device exchanged, PhyRdy
    /// changed) is a media change, cleared once counted.
    #[cfg(feature = "installer")]
    fn media_state(&self) -> block::MediaState {
        // SAFETY: status registers of this mapped port; X and N are
        // write-1-to-clear and only these two bits are written.
        unsafe {
            if mmio_read32(self.port_base, PX_SSTS) & 0x0f != SATA_DET_PRESENT {
                self.media.dead("ahci_no_device");
            }
            let serr = mmio_read32(self.port_base, PX_SERR);
            let change = serr & (PX_SERR_DIAG_X | PX_SERR_DIAG_N);
            if change != 0 {
                mmio_write32(self.port_base, PX_SERR, change);
                self.media.changed("ahci_serr_exchanged_or_phyrdy");
            }
        }
        self.media.state()
    }
}

impl AhciDevice {
    fn submit_data(&self, request: BlockRequest<'_>) -> BlockResult<()> {
        let sector_count = request_sector_count(request.buffers)?;
        if sector_count == 0 {
            return Err(BlockError::InvalidRequest);
        }
        if request.lba >= self.geometry.block_count
            || match request.lba.checked_add(sector_count as u64) {
                Some(end) => end > self.geometry.block_count,
                None => true,
            }
        {
            return Err(BlockError::InvalidRequest);
        }

        let command = match request.op {
            BlockOp::Read => ATA_READ_DMA_EXT,
            BlockOp::Write => ATA_WRITE_DMA_EXT,
            BlockOp::Flush => unreachable!(),
        };
        let write = request.op == BlockOp::Write;
        let mut inner = self.inner.lock();
        inner.issue_command(
            self.port_base,
            command,
            write,
            request.lba,
            sector_count,
            request.buffers,
        )
    }

    fn ack_interrupts(&self) {
        unsafe {
            let port_bit = 1u32 << self.port_index;
            let hba_is = mmio_read32(self.abar, HBA_IS);
            if hba_is & port_bit != 0 {
                let port_is = mmio_read32(self.port_base, PX_IS);
                if port_is != 0 {
                    mmio_write32(self.port_base, PX_IS, port_is);
                }
                mmio_write32(self.abar, HBA_IS, port_bit);
            }
        }
    }
}

impl AhciPortInner {
    fn issue_command(
        &mut self,
        port_base: VirtAddr,
        command: u8,
        write: bool,
        lba: u64,
        sector_count: u16,
        buffers: &[Arc<DmaBuffer>],
    ) -> BlockResult<()> {
        if buffers.len() > MAX_PRDT_ENTRIES {
            return Err(BlockError::Unsupported);
        }
        wait_port_ready(port_base)?;

        #[cfg(feature = "installer")]
        {
            // SAFETY: PxSERR is a status register of this mapped port.
            let serr = unsafe { mmio_read32(port_base, PX_SERR) };
            if serr & (PX_SERR_DIAG_X | PX_SERR_DIAG_N) != 0 {
                self.media.changed("ahci_serr_exchanged_or_phyrdy");
            }
        }
        unsafe {
            mmio_write32(port_base, PX_IS, u32::MAX);
            mmio_write32(port_base, PX_SERR, u32::MAX);
            clear_buffer(&self.command_table);
            clear_buffer(&self.command_list);
        }

        write_command_fis(&self.command_table, command, lba, sector_count);
        write_command_header(
            &self.command_list,
            &self.command_table,
            write,
            buffers.len() as u16,
        );
        write_prdt(&self.command_table, buffers)?;

        // The HBA fetches the command list/table and PRDT asynchronously after
        // PX_CI is written. Match the NVMe doorbell path's release publication
        // discipline so real hardware cannot observe stale LBA/PRDT/source
        // fields from the command table.
        fence(Ordering::Release);
        unsafe {
            mmio_write32(port_base, PX_CI, 1 << COMMAND_SLOT);
        }

        for _ in 0..COMMAND_TIMEOUT_SPINS {
            let ci = unsafe { mmio_read32(port_base, PX_CI) };
            let is = unsafe { mmio_read32(port_base, PX_IS) };
            if is & PX_IS_TFES != 0 {
                unsafe {
                    mmio_write32(port_base, PX_IS, is);
                }
                return Err(BlockError::Io);
            }
            if ci & (1 << COMMAND_SLOT) == 0 {
                let tfd = unsafe { mmio_read32(port_base, PX_TFD) };
                if tfd & PX_TFD_ERR != 0 {
                    return Err(BlockError::Io);
                }
                unsafe {
                    if is != 0 {
                        mmio_write32(port_base, PX_IS, is);
                    }
                }
                fence(Ordering::Acquire);
                return Ok(());
            }
            core::hint::spin_loop();
        }

        Err(BlockError::Io)
    }
}

fn identify_port(
    port_base: VirtAddr,
    command_list: &Arc<DmaBuffer>,
    command_table: &Arc<DmaBuffer>,
) -> BlockResult<BlockGeometry> {
    let identify = DmaBuffer::alloc_zeroed(SECTOR_SIZE as usize).map_err(|_| BlockError::Io)?;
    let mut temp = AhciPortInner {
        command_list: command_list.clone(),
        _fis: DmaBuffer::alloc_zeroed(FIS_BYTES).map_err(|_| BlockError::Io)?,
        command_table: command_table.clone(),
        #[cfg(feature = "installer")]
        media: Arc::new(block::MediaTracker::new()),
    };
    temp.issue_command(
        port_base,
        ATA_IDENTIFY_DEVICE,
        false,
        0,
        0,
        core::slice::from_ref(&identify),
    )?;

    let words = unsafe { identify.as_slice() };
    let mut total = read_identify_u64(words, 100);
    if total == 0 {
        total = read_identify_u32(words, 60) as u64;
    }
    if total == 0 {
        return Err(BlockError::Io);
    }

    Ok(BlockGeometry {
        logical_block_size: SECTOR_SIZE,
        physical_block_size: SECTOR_SIZE,
        block_count: total,
    })
}

fn request_sector_count(buffers: &[Arc<DmaBuffer>]) -> BlockResult<u16> {
    if buffers.is_empty() {
        return Err(BlockError::InvalidRequest);
    }
    let mut bytes = 0usize;
    for buffer in buffers {
        if buffer.is_empty() || !buffer.len().is_multiple_of(SECTOR_SIZE as usize) {
            return Err(BlockError::InvalidRequest);
        }
        bytes = bytes
            .checked_add(buffer.len())
            .ok_or(BlockError::InvalidRequest)?;
    }
    let sectors = bytes / SECTOR_SIZE as usize;
    u16::try_from(sectors).map_err(|_| BlockError::Unsupported)
}

fn stop_port(port_base: VirtAddr) {
    unsafe {
        let cmd = mmio_read32(port_base, PX_CMD);
        mmio_write32(port_base, PX_CMD, cmd & !PX_CMD_ST);
        for _ in 0..PORT_IDLE_TIMEOUT_SPINS {
            if mmio_read32(port_base, PX_CMD) & PX_CMD_CR == 0 {
                break;
            }
            core::hint::spin_loop();
        }
        let cmd = mmio_read32(port_base, PX_CMD);
        mmio_write32(port_base, PX_CMD, cmd & !PX_CMD_FRE);
        for _ in 0..PORT_IDLE_TIMEOUT_SPINS {
            if mmio_read32(port_base, PX_CMD) & PX_CMD_FR == 0 {
                break;
            }
            core::hint::spin_loop();
        }
    }
}

fn start_port(port_base: VirtAddr) {
    unsafe {
        let cmd = mmio_read32(port_base, PX_CMD);
        mmio_write32(port_base, PX_CMD, cmd | PX_CMD_FRE);
        let cmd = mmio_read32(port_base, PX_CMD);
        mmio_write32(port_base, PX_CMD, cmd | PX_CMD_ST);
    }
}

fn wait_port_ready(port_base: VirtAddr) -> BlockResult<()> {
    for _ in 0..PORT_IDLE_TIMEOUT_SPINS {
        let tfd = unsafe { mmio_read32(port_base, PX_TFD) };
        if tfd & (PX_TFD_BSY | PX_TFD_DRQ) == 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(BlockError::Io)
}

fn port_has_sata_device(port_base: VirtAddr) -> bool {
    let ssts = unsafe { mmio_read32(port_base, PX_SSTS) };
    let det = ssts & 0x0f;
    let ipm = (ssts >> 8) & 0x0f;
    let sig = unsafe { mmio_read32(port_base, PX_SIG) };
    det == SATA_DET_PRESENT && ipm == SATA_IPM_ACTIVE && sig == SATA_SIG_ATA
}

fn write_command_header(
    command_list: &DmaBuffer,
    command_table: &DmaBuffer,
    write: bool,
    prdt_entries: u16,
) {
    let flags = 5u16 | if write { 1 << 6 } else { 0 };
    unsafe {
        let ptr = command_list.as_mut_ptr();
        write_volatile(ptr as *mut u16, flags.to_le());
        write_volatile(ptr.add(2) as *mut u16, prdt_entries.to_le());
        write_volatile(ptr.add(4) as *mut u32, 0);
        write_volatile(
            ptr.add(8) as *mut u64,
            command_table.physical_addr().as_u64().to_le(),
        );
    }
}

fn write_command_fis(command_table: &DmaBuffer, command: u8, lba: u64, sector_count: u16) {
    unsafe {
        let fis = command_table.as_mut_ptr().add(COMMAND_TABLE_CFIS);
        write_volatile(fis, FIS_TYPE_REG_H2D);
        write_volatile(fis.add(1), FIS_REG_H2D_COMMAND);
        write_volatile(fis.add(2), command);
        write_volatile(fis.add(3), 0);
        write_volatile(fis.add(4), (lba & 0xff) as u8);
        write_volatile(fis.add(5), ((lba >> 8) & 0xff) as u8);
        write_volatile(fis.add(6), ((lba >> 16) & 0xff) as u8);
        write_volatile(fis.add(7), 1 << 6);
        write_volatile(fis.add(8), ((lba >> 24) & 0xff) as u8);
        write_volatile(fis.add(9), ((lba >> 32) & 0xff) as u8);
        write_volatile(fis.add(10), ((lba >> 40) & 0xff) as u8);
        write_volatile(fis.add(11), 0);
        write_volatile(fis.add(12), (sector_count & 0xff) as u8);
        write_volatile(fis.add(13), (sector_count >> 8) as u8);
        write_volatile(fis.add(14), 0);
        write_volatile(fis.add(15), 0);
    }
}

fn write_prdt(command_table: &DmaBuffer, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
    for (idx, buffer) in buffers.iter().enumerate() {
        let len = buffer.len();
        if len == 0 || len > 4 * 1024 * 1024 || len % SECTOR_SIZE as usize != 0 {
            return Err(BlockError::InvalidRequest);
        }
        let phys = buffer.physical_addr().as_u64();
        let dbc = u32::try_from(len - 1).map_err(|_| BlockError::Unsupported)? | (1 << 31);
        unsafe {
            let entry = command_table
                .as_mut_ptr()
                .add(COMMAND_TABLE_PRDT + idx * PRDT_BYTES);
            write_volatile(entry as *mut u32, (phys & 0xffff_ffff) as u32);
            write_volatile(entry.add(4) as *mut u32, (phys >> 32) as u32);
            write_volatile(entry.add(8) as *mut u32, 0);
            write_volatile(entry.add(12) as *mut u32, dbc.to_le());
        }
    }
    Ok(())
}

fn clear_buffer(buffer: &DmaBuffer) {
    unsafe {
        write_bytes(buffer.as_mut_ptr(), 0, buffer.len());
    }
}

fn port_base(abar: VirtAddr, port: u8) -> VirtAddr {
    VirtAddr::new(abar.as_u64() + HBA_PORT_BASE as u64 + (port as u64 * HBA_PORT_STRIDE as u64))
}

fn pack_bdf(dev: Device) -> u32 {
    ((dev.bus as u32) << 16) | ((dev.device as u32) << 8) | dev.function as u32
}

fn read_identify_u16(bytes: &[u8], word: usize) -> u16 {
    let offset = word * 2;
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_identify_u32(bytes: &[u8], word: usize) -> u32 {
    let lo = read_identify_u16(bytes, word) as u32;
    let hi = read_identify_u16(bytes, word + 1) as u32;
    lo | (hi << 16)
}

fn read_identify_u64(bytes: &[u8], word: usize) -> u64 {
    let lo = read_identify_u32(bytes, word) as u64;
    let hi = read_identify_u32(bytes, word + 2) as u64;
    lo | (hi << 32)
}

unsafe fn write_u64_split(base: VirtAddr, offset: usize, value: u64) {
    unsafe {
        mmio_write32(base, offset, (value & 0xffff_ffff) as u32);
        mmio_write32(base, offset + 4, (value >> 32) as u32);
    }
}

unsafe fn mmio_read32(base: VirtAddr, offset: usize) -> u32 {
    unsafe {
        u32::from_le(read_volatile(
            (base.as_u64() as usize + offset) as *const u32,
        ))
    }
}

unsafe fn mmio_write32(base: VirtAddr, offset: usize, value: u32) {
    unsafe {
        write_volatile((base.as_u64() as usize + offset) as *mut u32, value.to_le());
    }
}
