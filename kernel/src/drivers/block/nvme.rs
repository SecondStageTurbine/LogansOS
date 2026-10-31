// SPDX-License-Identifier: MPL-2.0
//! NVMe block backend for Phase 3 BUILD-5a.
//!
//! The initial driver targets QEMU's PCI NVMe controller. It creates one admin
//! queue pair and one I/O queue pair, issues commands synchronously through the
//! existing `BlockDevice` trait, and uses PRPs rather than SGLs.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr::{copy_nonoverlapping, read_volatile, write_bytes, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering, fence};

use spin::Lazy;
use x86_64::VirtAddr;

#[cfg(feature = "installer")]
use crate::drivers::block::nvme_event_logic as events;
use crate::drivers::block::{
    self, BlockDevice, BlockDeviceIdentity, BlockError, BlockGeometry, BlockOp, BlockRequest,
    BlockResult, DmaBuffer,
};
use crate::drivers::pci::{self, Device};
use crate::serial_println;
use crate::sync::SwitchMutex;

pub const NVME_MSI_VECTOR: u8 = 0xf1;

const NVME_BAR: u8 = 0;
const NVME_MMIO_LEN: u32 = 0x4000;
const PAGE_SIZE: usize = 4096;

const REG_CAP: usize = 0x00;
const REG_CC: usize = 0x14;
const REG_CSTS: usize = 0x1c;
const REG_AQA: usize = 0x24;
const REG_ASQ: usize = 0x28;
const REG_ACQ: usize = 0x30;
const REG_DOORBELL_BASE: usize = 0x1000;

const CC_ENABLE: u32 = 1 << 0;
const CC_CSS_NVM: u32 = 0 << 4;
const CC_MPS_4K: u32 = 0 << 7;
const CC_AMS_ROUND_ROBIN: u32 = 0 << 11;
const CC_IOSQES_64B: u32 = 6 << 16;
const CC_IOCQES_16B: u32 = 4 << 20;
const CSTS_RDY: u32 = 1 << 0;
const CAP_CSS_NVM: u64 = 1 << 37;

const ADMIN_QUEUE_ID: u16 = 0;
const IO_QUEUE_ID: u16 = 1;
const ADMIN_QUEUE_DEPTH: u16 = 8;
const IO_QUEUE_DEPTH: u16 = 16;
const SQ_ENTRY_BYTES: usize = 64;
const CQ_ENTRY_BYTES: usize = 16;

const ADMIN_IDENTIFY: u8 = 0x06;
const ADMIN_CREATE_IO_CQ: u8 = 0x05;
const ADMIN_CREATE_IO_SQ: u8 = 0x01;
const ADMIN_SET_FEATURES: u8 = 0x09;
const FEATURE_NUMBER_OF_QUEUES: u32 = 0x07;

const CNS_IDENTIFY_NAMESPACE: u32 = 0x00;
const CNS_IDENTIFY_CONTROLLER: u32 = 0x01;

const NVM_FLUSH: u8 = 0x00;
const NVM_WRITE: u8 = 0x01;
const NVM_READ: u8 = 0x02;

const SECTOR_SIZE: u32 = 512;
const COMMAND_TIMEOUT_SPINS: usize = 10_000_000;
const CONTROLLER_TIMEOUT_SPINS: usize = 10_000_000;
const MAX_PRP_LIST_ENTRIES: usize = PAGE_SIZE / core::mem::size_of::<u64>();

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ProbeError {
    MissingBar0,
    UnsupportedController,
    UnsupportedPageSize,
    ControllerNotReady,
    InterruptUnsupported,
    Dma,
    IdentifyController,
    IdentifyNamespace,
    QueueCreate,
    InvalidNamespace,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
#[allow(dead_code)]
pub struct ProbeSnapshot {
    pub seen: bool,
    pub probe_ok: bool,
    pub bar0_mapped: bool,
    pub msi_configured: bool,
    pub msix_configured: bool,
    pub admin_queue_ready: bool,
    pub io_queue_ready: bool,
    pub controller_ready: bool,
    pub identify_controller_ok: bool,
    pub identify_namespace_ok: bool,
    pub lba_format: u32,
    pub sector_size: u32,
    pub sector_count: u64,
    pub doorbell_stride: u32,
    pub last_bdf: u32,
    pub registered_index: u32,
    pub interrupt_count: u64,
}

static SEEN: AtomicBool = AtomicBool::new(false);
static PROBE_OK: AtomicBool = AtomicBool::new(false);
static BAR0_MAPPED: AtomicBool = AtomicBool::new(false);
static MSI_CONFIGURED: AtomicBool = AtomicBool::new(false);
static MSIX_CONFIGURED: AtomicBool = AtomicBool::new(false);
static ADMIN_QUEUE_READY: AtomicBool = AtomicBool::new(false);
static IO_QUEUE_READY: AtomicBool = AtomicBool::new(false);
static CONTROLLER_READY: AtomicBool = AtomicBool::new(false);
static IDENTIFY_CONTROLLER_OK: AtomicBool = AtomicBool::new(false);
static IDENTIFY_NAMESPACE_OK: AtomicBool = AtomicBool::new(false);
static LBA_FORMAT: AtomicU32 = AtomicU32::new(0);
static SECTOR_SIZE_ATOMIC: AtomicU32 = AtomicU32::new(0);
static SECTOR_COUNT: AtomicU64 = AtomicU64::new(0);
static DOORBELL_STRIDE: AtomicU32 = AtomicU32::new(0);
static LAST_BDF: AtomicU32 = AtomicU32::new(0);
static REGISTERED_INDEX: AtomicU32 = AtomicU32::new(u32::MAX);
static INTERRUPTS: AtomicU64 = AtomicU64::new(0);

/// NVMe namespace device list lock.
///
/// Process-context device list: list mutation happens during PCI probe and
/// lookup/debug paths run from process context. The MSI/MSI-X handler
/// intentionally does not take this lock; it only counts the interrupt,
/// because command completion is polled synchronously.
static NVME_DEVICES: Lazy<SwitchMutex<Vec<Arc<NvmeNamespaceDevice>>>> =
    Lazy::new(|| SwitchMutex::new(Vec::new()));

pub fn probe_pci(dev: Device) -> Result<(), ProbeError> {
    if PROBE_OK.load(Ordering::Acquire) {
        SEEN.store(true, Ordering::Release);
        return Err(ProbeError::InvalidNamespace);
    }
    reset_probe_state(dev);
    pci::enable_memory_and_bus_mastering(dev);

    let mmio =
        pci::map_bar_region(dev, NVME_BAR, 0, NVME_MMIO_LEN).ok_or(ProbeError::MissingBar0)?;
    BAR0_MAPPED.store(true, Ordering::Release);

    let cap = unsafe { mmio_read64(mmio, REG_CAP) };
    if cap & CAP_CSS_NVM == 0 {
        return Err(ProbeError::UnsupportedController);
    }
    let mps_min = ((cap >> 48) & 0x0f) as u8;
    if mps_min > 0 {
        return Err(ProbeError::UnsupportedPageSize);
    }
    let doorbell_stride = 4u32 << ((cap >> 32) & 0x0f);
    DOORBELL_STRIDE.store(doorbell_stride, Ordering::Release);

    disable_controller(mmio)?;

    let admin_sq = DmaBuffer::alloc_zeroed(ADMIN_QUEUE_DEPTH as usize * SQ_ENTRY_BYTES)
        .map_err(|_| ProbeError::Dma)?;
    let admin_cq = DmaBuffer::alloc_zeroed(ADMIN_QUEUE_DEPTH as usize * CQ_ENTRY_BYTES)
        .map_err(|_| ProbeError::Dma)?;

    unsafe {
        mmio_write32(
            mmio,
            REG_AQA,
            ((ADMIN_QUEUE_DEPTH as u32 - 1) << 16) | (ADMIN_QUEUE_DEPTH as u32 - 1),
        );
        mmio_write64(mmio, REG_ASQ, admin_sq.physical_addr().as_u64());
        mmio_write64(mmio, REG_ACQ, admin_cq.physical_addr().as_u64());
        mmio_write32(
            mmio,
            REG_CC,
            CC_ENABLE | CC_CSS_NVM | CC_MPS_4K | CC_AMS_ROUND_ROBIN | CC_IOSQES_64B | CC_IOCQES_16B,
        );
    }
    wait_controller_ready(mmio, true)?;
    CONTROLLER_READY.store(true, Ordering::Release);

    let mut admin = NvmeQueuePair::new(
        ADMIN_QUEUE_ID,
        ADMIN_QUEUE_DEPTH,
        admin_sq,
        admin_cq,
        doorbell_stride,
    );
    ADMIN_QUEUE_READY.store(true, Ordering::Release);

    let identify_controller = DmaBuffer::alloc_zeroed(PAGE_SIZE).map_err(|_| ProbeError::Dma)?;
    let mut cmd = NvmeCommand::new(ADMIN_IDENTIFY, 0);
    cmd.set_prp1(identify_controller.physical_addr().as_u64());
    cmd.set_cdw10(CNS_IDENTIFY_CONTROLLER);
    admin
        .submit(mmio, &mut cmd)
        .map_err(|_| ProbeError::IdentifyController)?;
    IDENTIFY_CONTROLLER_OK.store(true, Ordering::Release);

    let identify_namespace = DmaBuffer::alloc_zeroed(PAGE_SIZE).map_err(|_| ProbeError::Dma)?;
    let mut cmd = NvmeCommand::new(ADMIN_IDENTIFY, 1);
    cmd.set_prp1(identify_namespace.physical_addr().as_u64());
    cmd.set_cdw10(CNS_IDENTIFY_NAMESPACE);
    admin
        .submit(mmio, &mut cmd)
        .map_err(|_| ProbeError::IdentifyNamespace)?;

    let namespace =
        parse_namespace_identity(&identify_namespace).ok_or(ProbeError::InvalidNamespace)?;
    IDENTIFY_NAMESPACE_OK.store(true, Ordering::Release);
    LBA_FORMAT.store(namespace.lba_format as u32, Ordering::Release);
    SECTOR_SIZE_ATOMIC.store(namespace.sector_size, Ordering::Release);
    SECTOR_COUNT.store(namespace.sector_count, Ordering::Release);
    if namespace.sector_size != SECTOR_SIZE || namespace.sector_count == 0 {
        return Err(ProbeError::InvalidNamespace);
    }

    let mut cmd = NvmeCommand::new(ADMIN_SET_FEATURES, 0);
    cmd.set_cdw10(FEATURE_NUMBER_OF_QUEUES);
    cmd.set_cdw11(0);
    admin
        .submit(mmio, &mut cmd)
        .map_err(|_| ProbeError::QueueCreate)?;

    let io_sq = DmaBuffer::alloc_zeroed(IO_QUEUE_DEPTH as usize * SQ_ENTRY_BYTES)
        .map_err(|_| ProbeError::Dma)?;
    let io_cq = DmaBuffer::alloc_zeroed(IO_QUEUE_DEPTH as usize * CQ_ENTRY_BYTES)
        .map_err(|_| ProbeError::Dma)?;

    let mut cmd = NvmeCommand::new(ADMIN_CREATE_IO_CQ, 0);
    cmd.set_prp1(io_cq.physical_addr().as_u64());
    cmd.set_cdw10(((IO_QUEUE_DEPTH as u32 - 1) << 16) | IO_QUEUE_ID as u32);
    cmd.set_cdw11(0x3);
    admin
        .submit(mmio, &mut cmd)
        .map_err(|_| ProbeError::QueueCreate)?;

    let mut cmd = NvmeCommand::new(ADMIN_CREATE_IO_SQ, 0);
    cmd.set_prp1(io_sq.physical_addr().as_u64());
    cmd.set_cdw10(((IO_QUEUE_DEPTH as u32 - 1) << 16) | IO_QUEUE_ID as u32);
    cmd.set_cdw11((IO_QUEUE_ID as u32) << 16 | 0x1);
    admin
        .submit(mmio, &mut cmd)
        .map_err(|_| ProbeError::QueueCreate)?;
    IO_QUEUE_READY.store(true, Ordering::Release);

    let msix = pci::configure_msix_fixed_vector(dev, NVME_MSI_VECTOR, 0);
    let msi = if msix {
        false
    } else {
        pci::configure_msi_fixed_vector(dev, NVME_MSI_VECTOR)
    };
    MSIX_CONFIGURED.store(msix, Ordering::Release);
    MSI_CONFIGURED.store(msi, Ordering::Release);
    if !msix && !msi {
        return Err(ProbeError::InterruptUnsupported);
    }

    // OT-87 R4.1: namespace-attribute notices on, and one Asynchronous Event
    // Request outstanding from here on (`service_async_events`).
    #[cfg(feature = "installer")]
    let events_armed = arm_async_events(mmio, &mut admin);
    let io = NvmeQueuePair::new(IO_QUEUE_ID, IO_QUEUE_DEPTH, io_sq, io_cq, doorbell_stride);
    let prp_list = DmaBuffer::alloc_zeroed(PAGE_SIZE).map_err(|_| ProbeError::Dma)?;
    let geometry = BlockGeometry {
        logical_block_size: namespace.sector_size,
        physical_block_size: namespace.sector_size,
        block_count: namespace.sector_count,
    };
    let bdf = dev.bdf();
    let device = Arc::new(NvmeNamespaceDevice {
        identity: BlockDeviceIdentity {
            driver: "nvme",
            name: "nvme0n1",
            pci_bdf: Some(bdf),
        },
        geometry,
        inner: SwitchMutex::new(NvmeControllerInner {
            mmio,
            _admin: admin,
            io,
            prp_list,
        }),
        #[cfg(feature = "installer")]
        mmio,
        #[cfg(feature = "installer")]
        media: block::MediaTracker::new(),
        // SAFETY: registers of the mapped BAR, read once the controller is
        // set up: the values the driver programmed.
        #[cfg(feature = "installer")]
        ours: unsafe { read_regs(mmio) },
        #[cfg(feature = "installer")]
        events_armed,
    });

    let registered_index = block::register_device(device.clone());
    REGISTERED_INDEX.store(registered_index as u32, Ordering::Release);
    NVME_DEVICES.lock().push(device);
    PROBE_OK.store(true, Ordering::Release);

    serial_println!(
        "[ kernel ] nvme: attached bdf={:02x}:{:02x}.{} nsid=1 sectors={} sector_size={} doorbell_stride={} msix={} msi={}",
        bdf.0,
        bdf.1,
        bdf.2,
        geometry.block_count,
        geometry.logical_block_size,
        doorbell_stride,
        msix,
        msi
    );
    Ok(())
}

fn reset_probe_state(dev: Device) {
    SEEN.store(true, Ordering::Release);
    PROBE_OK.store(false, Ordering::Release);
    BAR0_MAPPED.store(false, Ordering::Release);
    MSI_CONFIGURED.store(false, Ordering::Release);
    MSIX_CONFIGURED.store(false, Ordering::Release);
    ADMIN_QUEUE_READY.store(false, Ordering::Release);
    IO_QUEUE_READY.store(false, Ordering::Release);
    CONTROLLER_READY.store(false, Ordering::Release);
    IDENTIFY_CONTROLLER_OK.store(false, Ordering::Release);
    IDENTIFY_NAMESPACE_OK.store(false, Ordering::Release);
    LBA_FORMAT.store(0, Ordering::Release);
    SECTOR_SIZE_ATOMIC.store(0, Ordering::Release);
    SECTOR_COUNT.store(0, Ordering::Release);
    DOORBELL_STRIDE.store(0, Ordering::Release);
    LAST_BDF.store(pack_bdf(dev), Ordering::Release);
    REGISTERED_INDEX.store(u32::MAX, Ordering::Release);
}

#[allow(dead_code)]
pub fn last_probe_snapshot() -> ProbeSnapshot {
    ProbeSnapshot {
        seen: SEEN.load(Ordering::Acquire),
        probe_ok: PROBE_OK.load(Ordering::Acquire),
        bar0_mapped: BAR0_MAPPED.load(Ordering::Acquire),
        msi_configured: MSI_CONFIGURED.load(Ordering::Acquire),
        msix_configured: MSIX_CONFIGURED.load(Ordering::Acquire),
        admin_queue_ready: ADMIN_QUEUE_READY.load(Ordering::Acquire),
        io_queue_ready: IO_QUEUE_READY.load(Ordering::Acquire),
        controller_ready: CONTROLLER_READY.load(Ordering::Acquire),
        identify_controller_ok: IDENTIFY_CONTROLLER_OK.load(Ordering::Acquire),
        identify_namespace_ok: IDENTIFY_NAMESPACE_OK.load(Ordering::Acquire),
        lba_format: LBA_FORMAT.load(Ordering::Acquire),
        sector_size: SECTOR_SIZE_ATOMIC.load(Ordering::Acquire),
        sector_count: SECTOR_COUNT.load(Ordering::Acquire),
        doorbell_stride: DOORBELL_STRIDE.load(Ordering::Acquire),
        last_bdf: LAST_BDF.load(Ordering::Acquire),
        registered_index: REGISTERED_INDEX.load(Ordering::Acquire),
        interrupt_count: INTERRUPTS.load(Ordering::Acquire),
    }
}

/// `install_claim_nvme_events`: Namespace Attachment for `nsid` on this
/// controller (attach, or detach), as another host or a management tool
/// would issue it. True when the controller accepted it.
#[cfg(feature = "neo-test-install-claim-nvme-events")]
pub fn test_namespace_attachment(attach: bool, nsid: u32) -> bool {
    let Some(device) = NVME_DEVICES.lock().first().cloned() else {
        return false;
    };
    let (Ok(identify), Ok(list)) = (
        DmaBuffer::alloc_zeroed(PAGE_SIZE),
        DmaBuffer::alloc_zeroed(PAGE_SIZE),
    ) else {
        return false;
    };
    let mut inner = device.inner.lock();
    let mmio = inner.mmio;
    let mut cmd = NvmeCommand::new(ADMIN_IDENTIFY, 0);
    cmd.set_prp1(identify.physical_addr().as_u64());
    cmd.set_cdw10(CNS_IDENTIFY_CONTROLLER);
    if inner._admin.submit(mmio, &mut cmd).is_err() {
        return false;
    }
    // SAFETY: both buffers are this function's; the command completed.
    let cntlid = unsafe { u16::from_le_bytes([identify.as_slice()[78], identify.as_slice()[79]]) };
    // The controller list: one identifier, this controller.
    unsafe {
        list.as_mut_slice()[..2].copy_from_slice(&1u16.to_le_bytes());
        list.as_mut_slice()[2..4].copy_from_slice(&cntlid.to_le_bytes());
    }
    let mut cmd = NvmeCommand::new(events::OPC_NAMESPACE_ATTACHMENT, nsid);
    cmd.set_prp1(list.physical_addr().as_u64());
    cmd.set_cdw10(if attach { 0 } else { 1 });
    let ok = inner._admin.submit(mmio, &mut cmd).is_ok();
    serial_println!(
        "[ kernel ] nvme: test namespace attachment nsid={} attach={} cntlid={} ok={}",
        nsid,
        attach,
        cntlid,
        ok
    );
    ok
}

/// `install_claim_nvme_events`: a controller reset this driver didn't make
/// (CC.EN cleared, waiting for CSTS.RDY to clear). With `reenable`, then
/// enabled again with exactly the CC the probe wrote, as another agent
/// would: AQA, ASQ and ACQ survive a controller reset, so every register
/// reads as before.
#[cfg(feature = "neo-test-install-claim-nvme-events")]
pub fn test_controller_reset(reenable: bool) -> bool {
    let Some(device) = NVME_DEVICES.lock().first().cloned() else {
        return false;
    };
    let mmio = device.mmio;
    let cc = device.ours.cc;
    // SAFETY: the mapped BAR; the scenario owns the controller now.
    unsafe { mmio_write32(mmio, REG_CC, cc & !CC_ENABLE) };
    if wait_controller_ready(mmio, false).is_err() {
        return false;
    }
    if reenable {
        unsafe { mmio_write32(mmio, REG_CC, cc) };
        if wait_controller_ready(mmio, true).is_err() {
            return false;
        }
    }
    serial_println!(
        "[ kernel ] nvme: test controller reset reenable={}",
        reenable
    );
    true
}

pub fn handle_interrupt() {
    INTERRUPTS.fetch_add(1, Ordering::AcqRel);
}

struct NvmeNamespaceIdentity {
    sector_count: u64,
    sector_size: u32,
    lba_format: u8,
}

struct NvmeNamespaceDevice {
    identity: BlockDeviceIdentity,
    geometry: BlockGeometry,
    /// Lock type: `SwitchMutex<NvmeControllerInner>`.
    /// IF discipline: synchronous block submitters may hold this lock with
    /// IF=1 while one queue command is in flight. The MSI/MSI-X handler does
    /// not take this lock or advance queue heads, so interrupt-time entry
    /// cannot self-deadlock on queue ownership.
    inner: SwitchMutex<NvmeControllerInner>,
    /// OT-87 R4.1: the controller registers, for the lock-free CSTS poll in
    /// `media_state` (a read-only MMIO register read).
    #[cfg(feature = "installer")]
    mmio: VirtAddr,
    #[cfg(feature = "installer")]
    media: block::MediaTracker,
    /// The CC, AQA, ASQ and ACQ the probe programmed (`register_verdict`).
    #[cfg(feature = "installer")]
    ours: events::Regs,
    /// Whether the probe armed namespace-attribute notices.
    #[cfg(feature = "installer")]
    events_armed: bool,
}

struct NvmeControllerInner {
    mmio: VirtAddr,
    _admin: NvmeQueuePair,
    io: NvmeQueuePair,
    prp_list: Arc<DmaBuffer>,
}

struct NvmeQueuePair {
    qid: u16,
    depth: u16,
    sq: Arc<DmaBuffer>,
    cq: Arc<DmaBuffer>,
    sq_tail: u16,
    cq_head: u16,
    cq_phase: bool,
    next_cid: u16,
    doorbell_stride: u32,
    /// OT-87 R4.1: the outstanding Asynchronous Event Request (admin queue).
    #[cfg(feature = "installer")]
    aer_cid: Option<u16>,
    /// Its completion, (status, Dword 0), seen but not yet acted on.
    #[cfg(feature = "installer")]
    aer_done: Option<(u32, u32)>,
}

struct NvmeCommand {
    bytes: [u8; SQ_ENTRY_BYTES],
}

impl BlockDevice for NvmeNamespaceDevice {
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
        let mmio = inner.mmio;
        let mut cmd = NvmeCommand::new(NVM_FLUSH, 1);
        inner.io.submit(mmio, &mut cmd).map(|_| ())
    }

    /// OT-87 R4.1: Identify Controller (CNS 01h) again, on the admin queue:
    /// SN and MN, with the namespace's size and sector size.
    #[cfg(feature = "installer")]
    fn driver_identity(&self) -> Option<block::claim_logic::DriverIdentity> {
        // Identify Controller (SN, MN), then Identify Namespace 1 (its size
        // and LBA format), both read now rather than from the probe.
        let controller = DmaBuffer::alloc_zeroed(PAGE_SIZE).ok()?;
        let namespace = DmaBuffer::alloc_zeroed(PAGE_SIZE).ok()?;
        let mut identify_controller = NvmeCommand::new(ADMIN_IDENTIFY, 0);
        identify_controller.set_prp1(controller.physical_addr().as_u64());
        identify_controller.set_cdw10(CNS_IDENTIFY_CONTROLLER);
        let mut identify_namespace = NvmeCommand::new(ADMIN_IDENTIFY, 1);
        identify_namespace.set_prp1(namespace.physical_addr().as_u64());
        identify_namespace.set_cdw10(CNS_IDENTIFY_NAMESPACE);
        {
            let mut inner = self.inner.lock();
            let mmio = inner.mmio;
            inner._admin.submit(mmio, &mut identify_controller).ok()?;
            inner._admin.submit(mmio, &mut identify_namespace).ok()?;
        }
        // SAFETY: both commands completed; nothing else holds the buffers.
        let (serial, model) =
            block::claim_logic::nvme_serial_model(unsafe { controller.as_slice() });
        let size = parse_namespace_identity(&namespace)?;
        // SAFETY: as above.
        let namespace_ids = events::namespace_ids(unsafe { namespace.as_slice() });
        Some(block::claim_logic::DriverIdentity {
            model,
            serial,
            capacity_bytes: size.sector_count.saturating_mul(size.sector_size as u64),
            sector_size: size.sector_size,
            namespace_ids,
        })
    }

    /// OT-87 R4.1: CSTS all ones (the controller is gone from the bus: a
    /// surprise removal), CFS, NSSRO, or not ready: the device is dead.
    ///
    /// Then the event queue (`service_async_events`): a namespace-attribute
    /// notice, or any other event, is a media change. And the registers
    /// against what the probe programmed (`register_verdict`): a reset the
    /// driver didn't make leaves CC, or the admin queue registers, different
    /// until the driver sets them again, which it never does.
    #[cfg(feature = "installer")]
    fn media_state(&self) -> block::MediaState {
        if !self.events_armed {
            self.media.dead("nvme_aen_unavailable");
        }
        self.service_async_events(&mut self.inner.lock());
        // SAFETY: registers of the mapped BAR, only read.
        if let Some(why) = events::register_verdict(unsafe { read_regs(self.mmio) }, self.ours) {
            self.media.dead(why);
        }
        self.media.state()
    }
}

impl NvmeNamespaceDevice {
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

        let opcode = match request.op {
            BlockOp::Read => NVM_READ,
            BlockOp::Write => NVM_WRITE,
            BlockOp::Flush => unreachable!(),
        };
        let mut inner = self.inner.lock();
        let prps = build_prps(request.buffers, &inner.prp_list)?;
        let mut cmd = NvmeCommand::new(opcode, 1);
        cmd.set_prp1(prps.prp1);
        cmd.set_prp2(prps.prp2);
        cmd.set_cdw10((request.lba & 0xffff_ffff) as u32);
        cmd.set_cdw11((request.lba >> 32) as u32);
        cmd.set_cdw12((sector_count as u32) - 1);
        let mmio = inner.mmio;
        inner.io.submit(mmio, &mut cmd).map(|_| ())
    }
}

impl NvmeQueuePair {
    fn new(
        qid: u16,
        depth: u16,
        sq: Arc<DmaBuffer>,
        cq: Arc<DmaBuffer>,
        doorbell_stride: u32,
    ) -> Self {
        Self {
            qid,
            depth,
            sq,
            cq,
            sq_tail: 0,
            cq_head: 0,
            cq_phase: true,
            next_cid: 1,
            doorbell_stride,
            #[cfg(feature = "installer")]
            aer_cid: None,
            #[cfg(feature = "installer")]
            aer_done: None,
        }
    }

    fn submit(&mut self, mmio: VirtAddr, command: &mut NvmeCommand) -> BlockResult<u32> {
        let cid = self.next_cid;
        self.next_cid = self.next_cid.wrapping_add(1).max(1);
        command.set_cid(cid);

        unsafe {
            let dst = self
                .sq
                .as_mut_ptr()
                .add(self.sq_tail as usize * SQ_ENTRY_BYTES);
            copy_nonoverlapping(command.bytes.as_ptr(), dst, SQ_ENTRY_BYTES);
        }
        fence(Ordering::Release);

        self.sq_tail = (self.sq_tail + 1) % self.depth;
        unsafe {
            mmio_write32(
                mmio,
                doorbell_offset(self.qid, false, self.doorbell_stride),
                self.sq_tail as u32,
            );
        }

        for _ in 0..COMMAND_TIMEOUT_SPINS {
            let entry = unsafe { self.cq.as_ptr().add(self.cq_head as usize * CQ_ENTRY_BYTES) };
            let dw3 = unsafe { u32::from_le(read_volatile(entry.add(12) as *const u32)) };
            let phase = ((dw3 >> 16) & 1) != 0;
            if phase == self.cq_phase {
                fence(Ordering::Acquire);
                let completion_cid = (dw3 & 0xffff) as u16;
                // OT-87 R4.1: the outstanding event request may complete
                // while an admin command waits; keep it for
                // `service_async_events` and wait on.
                #[cfg(feature = "installer")]
                if completion_cid != cid && self.aer_cid == Some(completion_cid) {
                    let dw0 = unsafe { u32::from_le(read_volatile(entry as *const u32)) };
                    self.aer_cid = None;
                    self.aer_done = Some(((dw3 >> 17) & 0x7fff, dw0));
                    self.advance(mmio);
                    continue;
                }
                if completion_cid != cid {
                    return Err(BlockError::Io);
                }
                let status = (dw3 >> 17) & 0x7fff;
                let dw0 = unsafe { u32::from_le(read_volatile(entry as *const u32)) };
                self.cq_head += 1;
                if self.cq_head == self.depth {
                    self.cq_head = 0;
                    self.cq_phase = !self.cq_phase;
                }
                unsafe {
                    mmio_write32(
                        mmio,
                        doorbell_offset(self.qid, true, self.doorbell_stride),
                        self.cq_head as u32,
                    );
                }
                if status != 0 {
                    return Err(BlockError::Io);
                }
                return Ok(dw0);
            }
            core::hint::spin_loop();
        }

        Err(BlockError::Io)
    }
}

#[cfg(feature = "installer")]
impl NvmeQueuePair {
    /// Consume the completion at the head: the head moves on, and the
    /// controller is told.
    fn advance(&mut self, mmio: VirtAddr) {
        self.cq_head += 1;
        if self.cq_head == self.depth {
            self.cq_head = 0;
            self.cq_phase = !self.cq_phase;
        }
        unsafe {
            mmio_write32(
                mmio,
                doorbell_offset(self.qid, true, self.doorbell_stride),
                self.cq_head as u32,
            );
        }
    }

    /// Submit without waiting (the Asynchronous Event Request, which
    /// completes only when the controller has an event). Returns its CID.
    fn post(&mut self, mmio: VirtAddr, command: &mut NvmeCommand) -> u16 {
        let cid = self.next_cid;
        self.next_cid = self.next_cid.wrapping_add(1).max(1);
        command.set_cid(cid);
        unsafe {
            let dst = self
                .sq
                .as_mut_ptr()
                .add(self.sq_tail as usize * SQ_ENTRY_BYTES);
            copy_nonoverlapping(command.bytes.as_ptr(), dst, SQ_ENTRY_BYTES);
        }
        fence(Ordering::Release);
        self.sq_tail = (self.sq_tail + 1) % self.depth;
        unsafe {
            mmio_write32(
                mmio,
                doorbell_offset(self.qid, false, self.doorbell_stride),
                self.sq_tail as u32,
            );
        }
        cid
    }

    /// The completion at the head, if the controller has posted one:
    /// (CID, status, Dword 0), consumed. Does not wait.
    fn poll(&mut self, mmio: VirtAddr) -> Option<(u16, u32, u32)> {
        let entry = unsafe { self.cq.as_ptr().add(self.cq_head as usize * CQ_ENTRY_BYTES) };
        let dw3 = unsafe { u32::from_le(read_volatile(entry.add(12) as *const u32)) };
        if (((dw3 >> 16) & 1) != 0) != self.cq_phase {
            return None;
        }
        fence(Ordering::Acquire);
        let dw0 = unsafe { u32::from_le(read_volatile(entry as *const u32)) };
        self.advance(mmio);
        Some(((dw3 & 0xffff) as u16, (dw3 >> 17) & 0x7fff, dw0))
    }
}

/// Turn on namespace-attribute notices (Set Features, Asynchronous Event
/// Configuration) and post the first Asynchronous Event Request. False if
/// the controller refused the feature: the device is then never claimable
/// (`media_state` reports it dead), since a namespace change would go
/// unreported.
#[cfg(feature = "installer")]
fn arm_async_events(mmio: VirtAddr, admin: &mut NvmeQueuePair) -> bool {
    let mut cmd = NvmeCommand::new(ADMIN_SET_FEATURES, 0);
    cmd.set_cdw10(events::FEATURE_ASYNC_EVENT_CONFIG);
    cmd.set_cdw11(events::AEC_NS_ATTR_NOTICES);
    if admin.submit(mmio, &mut cmd).is_err() {
        serial_println!("[ kernel ] nvme: async event configuration refused; never claimable");
        return false;
    }
    let mut aer = NvmeCommand::new(events::OPC_ASYNC_EVENT_REQUEST, 0);
    admin.aer_cid = Some(admin.post(mmio, &mut aer));
    serial_println!("[ kernel ] nvme: namespace-attribute notices armed");
    true
}

/// The registers `register_verdict` compares.
///
/// # Safety
/// `mmio` is the controller's mapped BAR 0.
#[cfg(feature = "installer")]
unsafe fn read_regs(mmio: VirtAddr) -> events::Regs {
    unsafe {
        events::Regs {
            cc: mmio_read32(mmio, REG_CC),
            csts: mmio_read32(mmio, REG_CSTS),
            aqa: mmio_read32(mmio, REG_AQA),
            asq: mmio_read64(mmio, REG_ASQ),
            acq: mmio_read64(mmio, REG_ACQ),
        }
    }
}

#[cfg(feature = "installer")]
impl NvmeNamespaceDevice {
    /// Act on the event request's completion, if the controller has posted
    /// it: every completion is a media change; read the log page it names
    /// (so the controller can report the next event) and post the next
    /// request. With no request outstanding afterwards, events would go
    /// unreported, so the device is dead from then on (fail-closed).
    ///
    /// Under the queue lock no admin command is in flight (they are
    /// synchronous under it), so a completion at the head here is the event
    /// request's; anything else is unexplained, and also dead.
    fn service_async_events(&self, inner: &mut NvmeControllerInner) {
        let mmio = inner.mmio;
        let admin = &mut inner._admin;
        if admin.aer_done.is_none()
            && let Some((cid, status, dw0)) = admin.poll(mmio)
        {
            if admin.aer_cid == Some(cid) {
                admin.aer_cid = None;
                admin.aer_done = Some((status, dw0));
            } else {
                self.media.dead("nvme_admin_stray_completion");
                return;
            }
        }
        if let Some((status, dw0)) = admin.aer_done.take() {
            let action = events::aer_action(status, dw0);
            serial_println!(
                "[ kernel ] nvme: async event status={:#x} dw0={:#010x} -> {}",
                status,
                dw0,
                action.reason
            );
            if !action.rearm {
                self.media.dead(action.reason);
                return;
            }
            self.media.changed(action.reason);
            if let Some(log) = action.read_log {
                let Ok(page) = DmaBuffer::alloc_zeroed(events::LOG_BYTES) else {
                    self.media.dead("nvme_aen_log_unreadable");
                    return;
                };
                let mut cmd = NvmeCommand::new(events::OPC_GET_LOG_PAGE, 0xffff_ffff);
                cmd.set_prp1(page.physical_addr().as_u64());
                cmd.set_cdw10(events::get_log_page_cdw10(log));
                if admin.submit(mmio, &mut cmd).is_err() {
                    self.media.dead("nvme_aen_log_unreadable");
                    return;
                }
                // SAFETY: the command completed; nothing else holds the page.
                let first = u32::from_le_bytes(
                    unsafe { page.as_slice() }[..4].try_into().unwrap_or([0; 4]),
                );
                serial_println!(
                    "[ kernel ] nvme: async event log={:#04x} first_entry={:#x}",
                    log,
                    first
                );
            }
            // The log read above may itself have seen another completion of
            // the request (it is the only one outstanding): it was not
            // re-posted yet, so there is none.
            let mut aer = NvmeCommand::new(events::OPC_ASYNC_EVENT_REQUEST, 0);
            admin.aer_cid = Some(admin.post(mmio, &mut aer));
        }
        if admin.aer_cid.is_none() {
            self.media.dead("nvme_aen_unarmed");
        }
    }
}

impl NvmeCommand {
    fn new(opcode: u8, nsid: u32) -> Self {
        let mut bytes = [0u8; SQ_ENTRY_BYTES];
        bytes[0] = opcode;
        write_le32(&mut bytes, 4, nsid);
        Self { bytes }
    }

    fn set_cid(&mut self, cid: u16) {
        write_le16(&mut self.bytes, 2, cid);
    }

    fn set_prp1(&mut self, value: u64) {
        write_le64(&mut self.bytes, 24, value);
    }

    fn set_prp2(&mut self, value: u64) {
        write_le64(&mut self.bytes, 32, value);
    }

    fn set_cdw10(&mut self, value: u32) {
        write_le32(&mut self.bytes, 40, value);
    }

    fn set_cdw11(&mut self, value: u32) {
        write_le32(&mut self.bytes, 44, value);
    }

    fn set_cdw12(&mut self, value: u32) {
        write_le32(&mut self.bytes, 48, value);
    }
}

struct PrpPair {
    prp1: u64,
    prp2: u64,
}

fn build_prps(buffers: &[Arc<DmaBuffer>], prp_list: &DmaBuffer) -> BlockResult<PrpPair> {
    if buffers.len() != 1 {
        return Err(BlockError::Unsupported);
    }
    let buffer = &buffers[0];
    if buffer.is_empty() || !buffer.len().is_multiple_of(SECTOR_SIZE as usize) {
        return Err(BlockError::InvalidRequest);
    }
    let phys = buffer.physical_addr().as_u64();
    let offset = (phys as usize) & (PAGE_SIZE - 1);
    let pages = (offset + buffer.len()).div_ceil(PAGE_SIZE);
    let prp1 = phys;
    let prp2 = match pages {
        0 | 1 => 0,
        2 => (phys & !(PAGE_SIZE as u64 - 1)) + PAGE_SIZE as u64,
        _ => {
            if pages - 1 > MAX_PRP_LIST_ENTRIES {
                return Err(BlockError::Unsupported);
            }
            unsafe {
                write_bytes(prp_list.as_mut_ptr(), 0, prp_list.len());
                let list = prp_list.as_mut_ptr() as *mut u64;
                let mut page_phys = (phys & !(PAGE_SIZE as u64 - 1)) + PAGE_SIZE as u64;
                for idx in 0..(pages - 1) {
                    write_volatile(list.add(idx), page_phys.to_le());
                    page_phys += PAGE_SIZE as u64;
                }
            }
            prp_list.physical_addr().as_u64()
        }
    };
    Ok(PrpPair { prp1, prp2 })
}

fn parse_namespace_identity(buffer: &DmaBuffer) -> Option<NvmeNamespaceIdentity> {
    let bytes = unsafe { buffer.as_slice() };
    let sector_count = read_le64(bytes, 0);
    let flbas = bytes[26] & 0x0f;
    let lba_format_count = (bytes[25] & 0x0f) + 1;
    if flbas >= lba_format_count {
        return None;
    }
    let lbaf_offset = 128 + flbas as usize * 4;
    let lba_data_shift = bytes[lbaf_offset + 2];
    if lba_data_shift > 31 {
        return None;
    }
    Some(NvmeNamespaceIdentity {
        sector_count,
        sector_size: 1u32 << lba_data_shift,
        lba_format: flbas,
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

fn disable_controller(mmio: VirtAddr) -> Result<(), ProbeError> {
    unsafe {
        let cc = mmio_read32(mmio, REG_CC);
        if cc & CC_ENABLE != 0 {
            mmio_write32(mmio, REG_CC, cc & !CC_ENABLE);
        }
    }
    wait_controller_ready(mmio, false)
}

fn wait_controller_ready(mmio: VirtAddr, ready: bool) -> Result<(), ProbeError> {
    for _ in 0..CONTROLLER_TIMEOUT_SPINS {
        let csts = unsafe { mmio_read32(mmio, REG_CSTS) };
        if (csts & CSTS_RDY != 0) == ready {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(ProbeError::ControllerNotReady)
}

fn doorbell_offset(qid: u16, completion_queue: bool, stride: u32) -> usize {
    REG_DOORBELL_BASE + ((2 * qid as usize + completion_queue as usize) * stride as usize)
}

fn pack_bdf(dev: Device) -> u32 {
    ((dev.bus as u32) << 16) | ((dev.device as u32) << 8) | dev.function as u32
}

fn write_le16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_le32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_le64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_le64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

unsafe fn mmio_read32(base: VirtAddr, offset: usize) -> u32 {
    unsafe {
        u32::from_le(read_volatile(
            (base.as_u64() as usize + offset) as *const u32,
        ))
    }
}

unsafe fn mmio_read64(base: VirtAddr, offset: usize) -> u64 {
    unsafe {
        u64::from_le(read_volatile(
            (base.as_u64() as usize + offset) as *const u64,
        ))
    }
}

unsafe fn mmio_write32(base: VirtAddr, offset: usize, value: u32) {
    unsafe {
        write_volatile((base.as_u64() as usize + offset) as *mut u32, value.to_le());
    }
}

unsafe fn mmio_write64(base: VirtAddr, offset: usize, value: u64) {
    unsafe {
        write_volatile((base.as_u64() as usize + offset) as *mut u64, value.to_le());
    }
}
