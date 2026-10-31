// SPDX-License-Identifier: MPL-2.0
//! Modern virtio-blk driver for Phase 3 BUILD-1 sub-commit 1B.
//!
//! BUILD-3 keeps the device policy deliberately narrow: QEMU must offer
//! `F_FLUSH`, and the driver negotiates `VIRTIO_F_VERSION_1 | F_FLUSH` while
//! leaving `CONFIG_WCE` disabled. Read/write requests use one data descriptor;
//! FLUSH requests use only header and status descriptors.

extern crate alloc;

// The `virtio-blk-io-gate-off` kill-switch disables the OT-59 #2 device I/O
// gate, restoring the pre-fix single-in-flight race (a KNOWN hang/panic). It
// exists ONLY to arm the falsifiable `virtio_blk_io_gate_contention` regression;
// it must NEVER reach a release build, where its disabled mode is not a safe
// fallback. Gate it behind `neo-test` at compile time.
#[cfg(all(feature = "virtio-blk-io-gate-off", not(feature = "neo-test")))]
compile_error!(
    "virtio-blk-io-gate-off is a TEST-ONLY kill-switch (its disabled mode is a known \
     deadlock); it requires the `neo-test` feature and must not ship in a release build"
);

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp;
use core::hint::spin_loop;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering, fence};

use spin::Lazy;
use x86_64::VirtAddr;

use crate::drivers::block::{
    self, BlockDevice, BlockDeviceIdentity, BlockError, BlockGeometry, BlockOp, BlockRequest,
    BlockResult, DmaBuffer,
};
use crate::drivers::pci::{self, Device, VirtioPciCap};
use crate::sched::{self, ThreadWakeToken, WakeTokenResult};
use crate::serial_println;
use crate::sync::{IrqSafeMutex, SwitchMutex};

pub const VIRTIO_VENDOR_ID: u16 = 0x1af4;
pub const VIRTIO_BLK_LEGACY_DEVICE_ID: u16 = 0x1001;
pub const VIRTIO_BLK_MODERN_DEVICE_ID: u16 = 0x1042;
#[cfg(feature = "virtio-blk-irq")]
pub const VIRTIO_BLK_MSI_VECTOR: u8 = 0xf4;

pub const VIRTIO_BLK_F_FLUSH: u64 = 1 << 9;
pub const VIRTIO_BLK_F_CONFIG_WCE: u64 = 1 << 11;
/// OT-87 R4.3: `blk_size` in the device config is valid (virtio 1.2
/// §5.2.3).
#[cfg(feature = "virtio-blk-size")]
pub const VIRTIO_BLK_F_BLK_SIZE: u64 = 1 << 6;
/// The device config's `blk_size` (u32; virtio 1.2 §5.2.4).
#[cfg(feature = "virtio-blk-size")]
const DEVICE_CONFIG_BLK_SIZE: usize = 20;

const VIRTIO_F_VERSION_1: u64 = 1 << 32;
#[cfg(feature = "virtio-blk-irq")]
const VIRTIO_F_EVENT_IDX: u64 = 1 << 29;

const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;
const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;

const COMMON_DEVICE_FEATURE_SELECT: usize = 0x00;
const COMMON_DEVICE_FEATURE: usize = 0x04;
const COMMON_DRIVER_FEATURE_SELECT: usize = 0x08;
const COMMON_DRIVER_FEATURE: usize = 0x0c;
#[cfg(feature = "virtio-blk-irq")]
const COMMON_CONFIG_MSIX_VECTOR: usize = 0x10;
const COMMON_DEVICE_STATUS: usize = 0x14;
const COMMON_QUEUE_SELECT: usize = 0x16;
const COMMON_QUEUE_SIZE: usize = 0x18;
#[cfg(feature = "virtio-blk-irq")]
const COMMON_QUEUE_MSIX_VECTOR: usize = 0x1a;
const COMMON_QUEUE_ENABLE: usize = 0x1c;
const COMMON_QUEUE_NOTIFY_OFF: usize = 0x1e;
const COMMON_QUEUE_DESC: usize = 0x20;
const COMMON_QUEUE_DRIVER: usize = 0x28;
const COMMON_QUEUE_DEVICE: usize = 0x30;

const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;
const STATUS_FAILED: u8 = 128;

const VIRTQUEUE_SIZE: u16 = 8;
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;
const VIRTIO_BLK_T_FLUSH: u32 = 4;
/// OT-87 R4.1: the device's serial, 20 bytes (virtio 1.2 §5.2.6).
#[cfg(feature = "installer")]
const VIRTIO_BLK_T_GET_ID: u32 = 8;
/// OT-87 R4.1: the common config's `config_generation` byte (virtio 1.2
/// §4.1.4.3): it changes whenever the device changes its config (a resize).
#[cfg(feature = "installer")]
const COMMON_CONFIG_GENERATION: usize = 0x15;
/// OT-87 R4.1: DEVICE_NEEDS_RESET (virtio 1.2 §2.1): the device hit an error
/// it cannot recover from.
#[cfg(feature = "installer")]
const STATUS_NEEDS_RESET: u8 = 0x40;
const VIRTIO_BLK_S_OK: u8 = 0;

const SECTOR_SIZE: u32 = 512;
const QUEUE_INDEX: u16 = 0;
const BLOCK_RET_OK: i64 = 0;
const BLOCK_RET_IO: i64 = -5;
const BLOCK_RET_UNSUPPORTED: i64 = -95;
const INLINE_COMPLETION_POLL_ATTEMPTS: usize = 64;

const WAKE_RESULT_NONE: u32 = 0;
const WAKE_RESULT_WOKEN: u32 = 1;
const WAKE_RESULT_GONE: u32 = 2;
const WAKE_RESULT_STALE: u32 = 3;
const WAKE_RESULT_INLINE: u32 = 4;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ProbeError {
    MissingCommonConfig,
    MissingNotifyConfig,
    MissingIsrConfig,
    MissingDeviceConfig,
    FlushNotOffered,
    #[cfg(feature = "virtio-blk-irq")]
    InterruptUnavailable,
    ModernFeatureNotOffered,
    FeatureNegotiationRejected,
    QueueUnavailable,
    Dma,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct ProbeSnapshot {
    pub seen: bool,
    pub probe_ok: bool,
    pub common_config: bool,
    pub notify_config: bool,
    pub isr_config: bool,
    pub device_config: bool,
    pub msix_configured: bool,
    pub msi_configured: bool,
    pub offered_features: u64,
    pub driver_features: u64,
    pub flush_offered: bool,
    pub flush_negotiated: bool,
    pub config_wce_negotiated: bool,
    pub event_idx_negotiated: bool,
    pub queue_ready: bool,
    pub last_bdf: u32,
    pub interrupt_count: u64,
    pub irq_completion_count: u64,
}

static SEEN: AtomicBool = AtomicBool::new(false);
static PROBE_OK: AtomicBool = AtomicBool::new(false);
static COMMON_CONFIG: AtomicBool = AtomicBool::new(false);
static NOTIFY_CONFIG: AtomicBool = AtomicBool::new(false);
static ISR_CONFIG: AtomicBool = AtomicBool::new(false);
static DEVICE_CONFIG: AtomicBool = AtomicBool::new(false);
static MSIX_CONFIGURED: AtomicBool = AtomicBool::new(false);
static MSI_CONFIGURED: AtomicBool = AtomicBool::new(false);
static OFFERED_FEATURES: AtomicU64 = AtomicU64::new(0);
static DRIVER_FEATURES: AtomicU64 = AtomicU64::new(0);
static FLUSH_OFFERED: AtomicBool = AtomicBool::new(false);
static EVENT_IDX_NEGOTIATED: AtomicBool = AtomicBool::new(false);
static QUEUE_READY: AtomicBool = AtomicBool::new(false);
static LAST_BDF: AtomicU32 = AtomicU32::new(0);
static INTERRUPTS: AtomicU64 = AtomicU64::new(0);
static IRQ_COMPLETIONS: AtomicU64 = AtomicU64::new(0);

static DESCRIPTOR_PUBLISH_BARRIERS: AtomicU64 = AtomicU64::new(0);
static NOTIFICATION_SUPPRESSION_BARRIERS: AtomicU64 = AtomicU64::new(0);
static COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static IN_FLIGHT_DMA_BUFFERS: AtomicU32 = AtomicU32::new(0);
static LAST_WAKE_RESULT: AtomicU32 = AtomicU32::new(WAKE_RESULT_NONE);
static LAST_COMPLETION_CPU: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_COMPLETION_TOKEN_CPU: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_COMPLETION_TOKEN_SLOT: AtomicU32 = AtomicU32::new(u32::MAX);
#[cfg(any(
    not(feature = "virtio-blk-irq"),
    feature = "neo-test-phase3-storage-completion-cross-cpu-wake"
))]
static COMPLETION_WORKER_STARTED: AtomicBool = AtomicBool::new(false);
static FLUSH_COMMANDS: AtomicU64 = AtomicU64::new(0);
static FLUSH_COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static LAST_COMPLETED_REQUEST_TYPE: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_COMPLETED_DMA_BUFFERS: AtomicU32 = AtomicU32::new(0);
static LAST_COMPLETED_HAS_DATA_BUFFER: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-arthur-memory-io-trace")]
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// Bounded per-completion serial trace (CI deflake).
///
/// One serial line per virtio-blk completion is a self-inflicted amplifier: the
/// QEMU serial console is synchronous, so under single-threaded TCG the guest
/// pays console time on every completion. Measured on `arthur_reversibility`,
/// 1,164 of its 1,400 serial lines (83%) came from this one statement, and that
/// scenario is a NeoFS-backed undo loop that times out in CI.
///
/// Keep the first [`COMPLETION_TRACE_LIMIT`] lines — enough to prove completions
/// are flowing when diagnosing a hang — then emit exactly one suppression notice
/// and stay quiet. Nothing parses these lines: the machine-readable channel is
/// [`debug_completion_count`] plus the `LAST_COMPLETION_*` atomics, which are
/// stored *before* this trace and are what `test_harness` actually asserts on.
/// Full per-completion detail remains available on demand via the
/// `neo-test-arthur-memory-io-trace` feature.
const COMPLETION_TRACE_LIMIT: u64 = 32;
static COMPLETION_TRACE_EMITTED: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "neo-test")]
static DEBUG_HOLD_COMPLETIONS: AtomicBool = AtomicBool::new(false);

/// Process-context primary-device cache: PCI probe initializes it and debug
/// helpers read it; the IRQ/completion path uses `VIRTIO_DEVICES` instead.
static PRIMARY_DEVICE: SwitchMutex<Option<Arc<VirtioBlkDevice>>> = SwitchMutex::new(None);
/// Mixed process/interrupt context: PCI probe mutates it, while the completion
/// worker and virtio interrupt hook poll device completions from IF=0 paths.
static VIRTIO_DEVICES: Lazy<IrqSafeMutex<Vec<Arc<VirtioBlkDevice>>>> =
    Lazy::new(|| IrqSafeMutex::new(Vec::new()));

#[derive(Copy, Clone)]
struct Region {
    cap: VirtioPciCap,
    virt: VirtAddr,
}

#[derive(Default)]
struct Regions {
    common: Option<Region>,
    notify: Option<Region>,
    isr: Option<Region>,
    device: Option<Region>,
}

pub fn probe_pci(dev: Device) -> Result<(), ProbeError> {
    reset_probe_state(dev);

    let mut regions = Regions::default();
    pci::for_each_virtio_vendor_cap(dev, |cap| {
        serial_println!(
            "[ kernel ] virtio-blk: cap bdf={:02x}:{:02x}.{} type={} bar={} offset=0x{:x} length=0x{:x}",
            dev.bus,
            dev.device,
            dev.function,
            cap.cfg_type,
            cap.bar,
            cap.offset,
            cap.length
        );
        let Some(virt) = pci::map_bar_region(dev, cap.bar, cap.offset, cap.length) else {
            return;
        };
        let region = Region { cap, virt };
        match cap.cfg_type {
            VIRTIO_PCI_CAP_COMMON_CFG => {
                COMMON_CONFIG.store(true, Ordering::Release);
                regions.common = Some(region);
            }
            VIRTIO_PCI_CAP_NOTIFY_CFG => {
                NOTIFY_CONFIG.store(true, Ordering::Release);
                regions.notify = Some(region);
            }
            VIRTIO_PCI_CAP_ISR_CFG => {
                ISR_CONFIG.store(true, Ordering::Release);
                regions.isr = Some(region);
            }
            VIRTIO_PCI_CAP_DEVICE_CFG => {
                DEVICE_CONFIG.store(true, Ordering::Release);
                regions.device = Some(region);
            }
            _ => {}
        }
    });

    let common = regions.common.ok_or(ProbeError::MissingCommonConfig)?;
    let notify = regions.notify.ok_or(ProbeError::MissingNotifyConfig)?;
    let isr = regions.isr.ok_or(ProbeError::MissingIsrConfig)?;
    let device_config = regions.device.ok_or(ProbeError::MissingDeviceConfig)?;

    pci::enable_memory_and_bus_mastering(dev);
    reset_device(common.virt);

    let offered = read_device_features(common.virt);
    let flush_offered = offered & VIRTIO_BLK_F_FLUSH != 0;
    OFFERED_FEATURES.store(offered, Ordering::Release);
    FLUSH_OFFERED.store(flush_offered, Ordering::Release);

    if !flush_offered {
        serial_println!(
            "[ kernel ] virtio-blk: refusing attach bdf={:02x}:{:02x}.{} reason=F_FLUSH_not_offered offered=0x{:016x}",
            dev.bus,
            dev.device,
            dev.function,
            offered
        );
        return Err(ProbeError::FlushNotOffered);
    }
    if offered & VIRTIO_F_VERSION_1 == 0 {
        fail_device(common.virt);
        return Err(ProbeError::ModernFeatureNotOffered);
    }

    add_status(common.virt, STATUS_ACKNOWLEDGE);
    add_status(common.virt, STATUS_DRIVER);

    #[cfg(feature = "virtio-blk-irq")]
    let event_idx = offered & VIRTIO_F_EVENT_IDX != 0;
    #[cfg(not(feature = "virtio-blk-irq"))]
    let event_idx = false;
    #[cfg(feature = "virtio-blk-irq")]
    let mut driver_features = VIRTIO_F_VERSION_1 | VIRTIO_BLK_F_FLUSH;
    #[cfg(feature = "virtio-blk-irq")]
    {
        if event_idx {
            driver_features |= VIRTIO_F_EVENT_IDX;
        }
    }
    #[cfg(not(feature = "virtio-blk-irq"))]
    let driver_features = VIRTIO_F_VERSION_1 | VIRTIO_BLK_F_FLUSH;
    // OT-87 R4.3: accept the device's logical block size when it offers one.
    #[cfg(feature = "virtio-blk-size")]
    let driver_features = driver_features | (offered & VIRTIO_BLK_F_BLK_SIZE);
    write_driver_features(common.virt, driver_features);
    DRIVER_FEATURES.store(driver_features, Ordering::Release);
    EVENT_IDX_NEGOTIATED.store(event_idx, Ordering::Release);
    add_status(common.virt, STATUS_FEATURES_OK);
    if unsafe { mmio_read8(common.virt, COMMON_DEVICE_STATUS) } & STATUS_FEATURES_OK == 0 {
        fail_device(common.virt);
        return Err(ProbeError::FeatureNegotiationRejected);
    }

    let queue = setup_queue(common.virt, notify, isr)?;
    let (msix, msi) = configure_completion_interrupt(dev, common.virt)?;
    let queue_size = queue.size;
    let capacity_sectors = unsafe { mmio_read64(device_config.virt, 0) };
    #[cfg(not(feature = "virtio-blk-size"))]
    let geometry = BlockGeometry {
        logical_block_size: SECTOR_SIZE,
        physical_block_size: SECTOR_SIZE,
        block_count: capacity_sectors,
    };
    // OT-87 R4.3: a 4096-byte logical block when the device reports one.
    // `capacity` stays in 512-byte sectors (§5.2.4), and requests keep
    // addressing 512-byte sectors (`PendingIo::new_data` scales the LBA).
    // Any other size is treated as 512, as before.
    #[cfg(feature = "virtio-blk-size")]
    let geometry = {
        let block = logical_block_size(driver_features, device_config.virt);
        BlockGeometry {
            logical_block_size: block,
            physical_block_size: block,
            block_count: capacity_sectors / (block / SECTOR_SIZE) as u64,
        }
    };
    #[cfg(feature = "virtio-blk-size")]
    serial_println!(
        "[ kernel ] virtio-blk: block-size bdf={:02x}:{:02x}.{} F_BLK_SIZE={} logical_block_size={} blocks={}",
        dev.bus,
        dev.device,
        dev.function,
        driver_features & VIRTIO_BLK_F_BLK_SIZE != 0,
        geometry.logical_block_size,
        geometry.block_count
    );

    let bdf = pack_bdf(dev);
    let device = Arc::new(VirtioBlkDevice {
        identity: BlockDeviceIdentity {
            driver: "virtio-blk",
            name: "virtio-blk0",
            pci_bdf: Some(dev.bdf()),
        },
        geometry,
        // `inner` is deliberately UNORDERED: it is acquired in the completion ISR
        // (`claim_completion`), and lockdep is invalid in interrupt context (it
        // keys by thread-id), so an ordered `inner` fires a FALSE inversion panic
        // when a completion IRQ preempts a thread holding an AI-governance lock.
        // Matches AHCI/NVMe (ISR inner unordered). The io_gate — thread-context
        // only — carries the io_gate→inner discipline on its own.
        inner: SwitchMutex::new(VirtioBlkInner {
            common: common.virt,
            notify: notify.virt,
            isr: isr.virt,
            notify_multiplier: notify.cap.notify_off_multiplier,
            queue,
            next_avail_idx: 0,
            last_used_idx: 0,
            in_flight: None,
            retired_completion: None,
            event_idx,
            bdf,
        }),
        io_gate: SwitchMutex::new_ordered((), crate::sync::lock_order::BLOCK_IO_GATE),
        #[cfg(feature = "installer")]
        common: common.virt,
        #[cfg(feature = "installer")]
        device_config: device_config.virt,
        #[cfg(feature = "installer")]
        config_generation: core::sync::atomic::AtomicU8::new(unsafe {
            mmio_read8(common.virt, COMMON_CONFIG_GENERATION)
        }),
        #[cfg(feature = "installer")]
        media: block::MediaTracker::new(),
    });

    add_status(common.virt, STATUS_DRIVER_OK);
    QUEUE_READY.store(true, Ordering::Release);
    PROBE_OK.store(true, Ordering::Release);

    {
        let mut primary = PRIMARY_DEVICE.lock();
        if primary.is_none() {
            *primary = Some(device.clone());
        }
    }
    VIRTIO_DEVICES.lock().push(device.clone());
    let trait_device: Arc<dyn BlockDevice> = device.clone();
    block::register_device(trait_device);

    serial_println!(
        "[ kernel ] virtio-blk: feature-policy bdf={:02x}:{:02x}.{} offered=0x{:016x} driver=0x{:016x} F_FLUSH_offered={} F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false F_EVENT_IDX_negotiated={} queue_size={} capacity_sectors={} msix={} msi={} attach=rw",
        dev.bus,
        dev.device,
        dev.function,
        offered,
        driver_features,
        flush_offered,
        event_idx,
        queue_size,
        capacity_sectors,
        msix,
        msi
    );

    Ok(())
}

pub fn last_probe_snapshot() -> ProbeSnapshot {
    ProbeSnapshot {
        seen: SEEN.load(Ordering::Acquire),
        probe_ok: PROBE_OK.load(Ordering::Acquire),
        common_config: COMMON_CONFIG.load(Ordering::Acquire),
        notify_config: NOTIFY_CONFIG.load(Ordering::Acquire),
        isr_config: ISR_CONFIG.load(Ordering::Acquire),
        device_config: DEVICE_CONFIG.load(Ordering::Acquire),
        msix_configured: MSIX_CONFIGURED.load(Ordering::Acquire),
        msi_configured: MSI_CONFIGURED.load(Ordering::Acquire),
        offered_features: OFFERED_FEATURES.load(Ordering::Acquire),
        driver_features: DRIVER_FEATURES.load(Ordering::Acquire),
        flush_offered: FLUSH_OFFERED.load(Ordering::Acquire),
        flush_negotiated: DRIVER_FEATURES.load(Ordering::Acquire) & VIRTIO_BLK_F_FLUSH != 0,
        config_wce_negotiated: DRIVER_FEATURES.load(Ordering::Acquire) & VIRTIO_BLK_F_CONFIG_WCE
            != 0,
        event_idx_negotiated: EVENT_IDX_NEGOTIATED.load(Ordering::Acquire),
        queue_ready: QUEUE_READY.load(Ordering::Acquire),
        last_bdf: LAST_BDF.load(Ordering::Acquire),
        interrupt_count: INTERRUPTS.load(Ordering::Acquire),
        irq_completion_count: IRQ_COMPLETIONS.load(Ordering::Acquire),
    }
}

#[allow(dead_code)]
pub fn handle_interrupt() {
    INTERRUPTS.fetch_add(1, Ordering::AcqRel);
    poll_all_completions(CompletionClaim::Irq);
}

#[cfg(feature = "neo-test")]
pub fn debug_interrupt_count() -> u64 {
    INTERRUPTS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_irq_completion_count() -> u64 {
    IRQ_COMPLETIONS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_descriptor_publish_barriers() -> u64 {
    DESCRIPTOR_PUBLISH_BARRIERS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_notification_suppression_barriers() -> u64 {
    NOTIFICATION_SUPPRESSION_BARRIERS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_completion_count() -> u64 {
    COMPLETIONS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_flush_command_count() -> u64 {
    FLUSH_COMMANDS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_flush_completion_count() -> u64 {
    FLUSH_COMPLETIONS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_completed_request_type() -> u32 {
    LAST_COMPLETED_REQUEST_TYPE.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_completed_dma_buffers() -> u32 {
    LAST_COMPLETED_DMA_BUFFERS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_completed_had_data_buffer() -> bool {
    LAST_COMPLETED_HAS_DATA_BUFFER.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_in_flight_dma_buffers() -> u32 {
    IN_FLIGHT_DMA_BUFFERS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_in_flight_waiting() -> bool {
    let Some(device) = primary_device() else {
        return false;
    };
    crate::arch::x86_64::without_interrupts(|| {
        let inner = device.inner.lock();
        inner
            .in_flight
            .as_ref()
            .is_some_and(|rendezvous| rendezvous.waiting_thread.is_some())
    })
}

#[cfg(feature = "neo-test")]
pub fn debug_last_wake_result() -> u32 {
    LAST_WAKE_RESULT.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_wake_result_woken(encoded: u32) -> bool {
    encoded == WAKE_RESULT_WOKEN
}

#[cfg(feature = "neo-test")]
pub fn debug_wake_result_gone_or_stale(encoded: u32) -> bool {
    encoded == WAKE_RESULT_GONE || encoded == WAKE_RESULT_STALE
}

#[cfg(feature = "neo-test")]
pub fn debug_wake_result_woken_or_inline(encoded: u32) -> bool {
    encoded == WAKE_RESULT_WOKEN || encoded == WAKE_RESULT_INLINE
}

#[cfg(feature = "neo-test")]
pub fn debug_last_completion_cpu() -> u32 {
    LAST_COMPLETION_CPU.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_completion_token_cpu() -> u32 {
    LAST_COMPLETION_TOKEN_CPU.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_completion_token_slot() -> u32 {
    LAST_COMPLETION_TOKEN_SLOT.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_set_completion_hold(hold: bool) {
    DEBUG_HOLD_COMPLETIONS.store(hold, Ordering::Release);
}

/// `phase3_virtio_blk_abandoned_io_no_leak`: judge the write guard for the
/// abandoned write's target now, while completions still flow (the guard's
/// first judgement reads the disk), and say whether it is armed.
#[cfg(all(feature = "neo-test", not(feature = "neo-test-bare-metal-mode")))]
pub fn debug_abandoned_write_armed(lba: u64) -> bool {
    primary_device().is_some_and(|device| {
        super::guard_allows_raw_write(
            &device.identity,
            lba,
            device.geometry.logical_block_size as u64,
        )
    })
}

/// `phase3_virtio_blk_abandoned_io_no_leak`: a write submitted below the
/// `BlockDevice` trait, so the write guard is asked directly (Janus F3), and
/// never compiled into a bare-metal build (only a QEMU virtio disk has it).
#[cfg(all(feature = "neo-test", not(feature = "neo-test-bare-metal-mode")))]
pub fn debug_submit_abandoned_write(lba: u64, fill: u8) -> BlockResult<()> {
    let Some(device) = primary_device() else {
        return Err(BlockError::NoDevice);
    };
    let block_size = device.geometry.logical_block_size as usize;
    if lba >= device.geometry.block_count {
        return Err(BlockError::InvalidRequest);
    }
    if !super::guard_allows_raw_write(&device.identity, lba, block_size as u64) {
        return Err(BlockError::WriteRefused);
    }
    let data = DmaBuffer::alloc_zeroed(block_size).map_err(|_| BlockError::Io)?;
    unsafe {
        for byte in data.as_mut_slice().iter_mut() {
            *byte = fill;
        }
    }
    let pending = PendingIo::new_data(
        BlockOp::Write,
        lba,
        data,
        device.geometry.logical_block_size,
    )?;
    device.submit_pending_and_wait(pending)
}

struct VirtioBlkDevice {
    identity: BlockDeviceIdentity,
    geometry: BlockGeometry,
    /// Scheduler/context-switch special: slow-path request submission parks the
    /// caller through `block_current_with_token_and_guard`, so this lock must
    /// keep release-token semantics for the parked I/O rendezvous.
    inner: SwitchMutex<VirtioBlkInner>,
    /// Device I/O serialization gate (OT-59 #2). Held for the whole
    /// `submit_pending_and_wait` sequence — submit → wait → completion — so two
    /// submitters (a foreground reader and the neofs-writeback worker) cannot
    /// interleave on the single in-flight/waiter slot. A `SwitchMutex` so it can
    /// be held across the owner's park.
    ///
    /// INVARIANT (load-bearing — grep for `io_gate` before adding any use): the
    /// gate is acquired ONLY at the top of `submit_pending_and_wait`, OUTER of
    /// `inner`. The completion path — `poll_used`, `handle_interrupt`,
    /// `poll_all_completions`, `virtio_blk_completion_worker` — MUST take only
    /// `inner`, NEVER this gate. The whole safety argument rests on the waker
    /// never needing the gate: it must stay free so a completion can wake the
    /// parked owner, which is what finally releases the gate. A third acquirer
    /// re-introduces the deadlock this gate closes.
    ///
    /// The parked gate-owner ALWAYS resumes to release the gate: the kernel is
    /// panic=abort (no unwinding), so a fault while the gate is held aborts the
    /// whole machine rather than dropping the guard and orphaning the gate.
    // Read only in `submit_pending_and_wait`; the `virtio-blk-io-gate-off`
    // kill-switch build cfg's that acquire out, leaving the field unread.
    #[cfg_attr(feature = "virtio-blk-io-gate-off", allow(dead_code))]
    io_gate: SwitchMutex<()>,
    /// OT-87 R4.1: the common config, for the lock-free status and
    /// config-generation poll in `media_state` (byte register reads).
    #[cfg(feature = "installer")]
    common: VirtAddr,
    /// OT-87 R4.1: the device config, for the live capacity in
    /// `driver_identity`.
    #[cfg(feature = "installer")]
    device_config: VirtAddr,
    #[cfg(feature = "installer")]
    config_generation: core::sync::atomic::AtomicU8,
    #[cfg(feature = "installer")]
    media: block::MediaTracker,
}

impl BlockDevice for VirtioBlkDevice {
    #[cfg(feature = "power-quiesce")]
    fn shutdown_step(&self) -> Option<block::ShutdownStep> {
        Some(block::ShutdownStep::VirtioBlk {
            common: self.inner.lock().common.as_u64(),
        })
    }

    fn identity(&self) -> &BlockDeviceIdentity {
        &self.identity
    }

    fn geometry(&self) -> BlockGeometry {
        self.geometry
    }

    fn submit(&self, request: BlockRequest<'_>) -> BlockResult<()> {
        if request.op == BlockOp::Flush {
            if !request.buffers.is_empty() || request.lba != 0 {
                return Err(BlockError::InvalidRequest);
            }
            return self.submit_pending_and_wait(PendingIo::new_flush()?);
        }
        if request.buffers.len() != 1 {
            return Err(BlockError::Unsupported);
        }
        let data = request.buffers[0].clone();
        let logical = self.geometry.logical_block_size as usize;
        if logical == 0 || data.len() < logical || !data.len().is_multiple_of(logical) {
            return Err(BlockError::InvalidRequest);
        }
        let requested_blocks = (data.len() / logical) as u64;
        if request.lba >= self.geometry.block_count
            || match request.lba.checked_add(requested_blocks) {
                Some(end) => end > self.geometry.block_count,
                None => true,
            }
        {
            return Err(BlockError::InvalidRequest);
        }
        let pending = PendingIo::new_data(
            request.op,
            request.lba,
            data,
            self.geometry.logical_block_size,
        )?;
        self.submit_pending_and_wait(pending)
    }

    fn supports_flush(&self) -> bool {
        true
    }

    fn flush(&self) -> BlockResult<()> {
        self.submit_pending_and_wait(PendingIo::new_flush()?)
    }

    /// OT-87 R4.1: `VIRTIO_BLK_T_GET_ID`, through the ordinary request path.
    /// A device that refuses it (or a QEMU disk without `serial=`, which
    /// answers with zeros) reports an empty serial: no identity.
    #[cfg(feature = "installer")]
    fn driver_identity(&self) -> Option<block::claim_logic::DriverIdentity> {
        let buffer = DmaBuffer::alloc_zeroed(SECTOR_SIZE as usize).ok()?;
        let mut pending =
            PendingIo::new_data(BlockOp::Read, 0, buffer.clone(), SECTOR_SIZE).ok()?;
        pending.get_id = true;
        let serial = match self.submit_pending_and_wait(pending) {
            // SAFETY: the request completed; nothing else holds the buffer.
            Ok(()) => block::claim_logic::virtio_serial(unsafe { &buffer.as_slice()[..20] }),
            Err(_) => alloc::string::String::new(),
        };
        // The capacity as the device config reports it now (virtio 1.2
        // §5.2.4), in 512-byte sectors whatever the block size. The logical
        // block size is the probe's (F_BLK_SIZE is negotiated once).
        // SAFETY: the device-config region is mapped for the device's life.
        let sectors = unsafe { mmio_read64(self.device_config, 0) };
        let sector_size = self.geometry.logical_block_size;
        Some(block::claim_logic::DriverIdentity {
            model: alloc::string::String::new(),
            serial,
            capacity_bytes: sectors.saturating_mul(SECTOR_SIZE as u64),
            sector_size,
            namespace_ids: None,
        })
    }

    /// OT-87 R4.1: DEVICE_NEEDS_RESET or FAILED in the device status is
    /// dead; a moved `config_generation` is a media change.
    #[cfg(feature = "installer")]
    fn media_state(&self) -> block::MediaState {
        // SAFETY: byte registers of the mapped common config.
        let (status, generation) = unsafe {
            (
                mmio_read8(self.common, COMMON_DEVICE_STATUS),
                mmio_read8(self.common, COMMON_CONFIG_GENERATION),
            )
        };
        if status & (STATUS_NEEDS_RESET | STATUS_FAILED) != 0 {
            self.media.dead("virtio_needs_reset");
        }
        if self.config_generation.swap(generation, Ordering::AcqRel) != generation {
            self.media.changed("virtio_config_generation");
        }
        self.media.state()
    }
}

impl VirtioBlkDevice {
    fn submit_pending_and_wait(&self, pending: PendingIo) -> BlockResult<()> {
        // Device I/O gate (OT-59 #2 root fix): the virtio-blk queue is single
        // in-flight, but this submit→wait→complete sequence has IF-enabled
        // windows (the inline-completion poll) where a concurrent submitter — a
        // foreground reader vs the neofs-writeback worker — or the completion
        // IRQ can interleave and race the one in_flight/waiter slot: a
        // lost-wakeup hang under the polling completion worker, or the
        // `attach_waiter` "already has a waiter" panic under virtio-blk-irq.
        // Serialize the whole sequence so exactly one submitter owns the device
        // from submit through completion. The guard is held across the owner's
        // park (the completion path locks only `inner`, never this gate, so it
        // can still wake us), and a *yielding* acquire cedes the CPU to the
        // parked owner rather than spin-starving it.
        // The acquire loop MUST run interrupts-enabled/preemptible (CP-2): a
        // waiter that yields IF-off can't be preempted, and the parked gate owner
        // is woken by the completion IRQ / timer — an IF-off wait would be the
        // very deadlock this gate closes, in new clothes. Force IF on for the
        // wait regardless of caller state; `with_interrupts_enabled` restores the
        // caller's prior IF on return, before we ever touch `inner`.
        // Kill-switch (OT-59 #2, complexity-governance): the `virtio-blk-io-gate-off`
        // build cfg's this acquire OUT, restoring the pre-fix racy driver — the
        // disabled mode whose regression proves the gate is load-bearing (the
        // `virtio_blk_io_gate_contention` scenario reds gate-off, greens gate-on).
        // It is TEST-ONLY (a compile_error! below forbids it in a release build):
        // its disabled state is a KNOWN hang, never a safe runtime fallback.
        #[cfg(not(feature = "virtio-blk-io-gate-off"))]
        let _io_gate = crate::arch::x86_64::with_interrupts_enabled(|| {
            loop {
                if let Some(guard) = self.io_gate.try_lock() {
                    break guard;
                }
                sched::yield_now();
            }
        });
        #[cfg(any(
            not(feature = "virtio-blk-irq"),
            feature = "neo-test-phase3-storage-completion-cross-cpu-wake"
        ))]
        ensure_completion_worker();
        #[cfg(feature = "neo-test-arthur-memory-io-trace")]
        let req_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::AcqRel);
        #[cfg(not(feature = "neo-test-arthur-memory-io-trace"))]
        let req_id = 0;
        #[cfg(feature = "neo-test-arthur-memory-io-trace")]
        serial_println!(
            "[ kernel ] virtio-blk: submit req_id={} op={:?} sector={} tick={} block_len={} has_data_buffer={}",
            req_id,
            pending.op,
            pending.sector,
            crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed),
            pending.block_len,
            pending.data.is_some()
        );
        let mut pending = Some(pending);

        #[cfg(feature = "virtio-blk-irq")]
        loop {
            let mut attempt = self.try_submit_pending(&mut pending, req_id);
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            Self::trace_submit_attempt(
                req_id,
                attempt.ret,
                attempt.drained_completion.is_some(),
                attempt.trace,
            );
            if let Some(completion) = attempt.drained_completion.take() {
                let _ = self.finish_completion(completion);
                sched::yield_now();
                continue;
            }
            if attempt.ret == BLOCK_RET_OK {
                break;
            }
            sched::yield_now();
        }

        #[cfg(not(feature = "virtio-blk-irq"))]
        {
            let attempt = self.try_submit_pending(&mut pending, req_id);
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            Self::trace_submit_attempt(
                req_id,
                attempt.ret,
                attempt.drained_completion.is_some(),
                attempt.trace,
            );
            if attempt.ret == BLOCK_RET_UNSUPPORTED
                && let Some(completion) = self.claim_completion(CompletionClaim::Worker)
            {
                let _ = self.finish_completion(completion);
                sched::yield_now();
            }
            if attempt.ret != BLOCK_RET_OK {
                return Self::block_result_from_return(attempt.ret);
            }
        }

        if inline_completion_poll_enabled() {
            for attempt in 1..=INLINE_COMPLETION_POLL_ATTEMPTS {
                if let Some(completion) = self.claim_completion(CompletionClaim::InlineSubmitter) {
                    #[cfg(not(feature = "neo-test-arthur-memory-io-trace"))]
                    let _ = attempt;
                    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
                    serial_println!(
                        "[ kernel ] virtio-blk: inline-poll req_id={} outcome=hit attempt={} tick={}",
                        req_id,
                        attempt,
                        crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed)
                    );
                    let ret = self.finish_completion(completion);
                    return Self::block_result_from_return(ret);
                }
                spin_loop();
            }
        }

        #[cfg(feature = "neo-test-arthur-memory-io-trace")]
        serial_println!(
            "[ kernel ] virtio-blk: inline-poll req_id={} outcome=miss attempts={} tick={} action=park",
            req_id,
            INLINE_COMPLETION_POLL_ATTEMPTS,
            crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed)
        );

        let mut prepark_completion = None;
        let ret = crate::arch::x86_64::without_interrupts(|| {
            let mut guard = self.inner.lock();
            if inline_completion_poll_enabled() {
                prepark_completion = guard.claim_ready(CompletionClaim::InlineSubmitter);
                if prepark_completion.is_some() {
                    return BLOCK_RET_OK;
                }
            }
            if guard.in_flight.is_none() {
                return BLOCK_RET_IO;
            }
            guard.arm_completion_interrupt(guard.last_used_idx);
            sched::block_current_with_token_and_guard(guard, move |inner, token| {
                inner.attach_waiter(token);
            })
        });
        if let Some(completion) = prepark_completion {
            let ret = self.finish_completion(completion);
            return Self::block_result_from_return(ret);
        }
        Self::block_result_from_return(ret)
    }

    fn try_submit_pending(&self, pending: &mut Option<PendingIo>, req_id: u64) -> SubmitAttempt {
        crate::arch::x86_64::without_interrupts(|| {
            let mut inner = self.inner.lock();
            #[cfg(feature = "virtio-blk-irq")]
            let mut drained_completion = None;
            #[cfg(not(feature = "virtio-blk-irq"))]
            let drained_completion = None;
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            let mut trace = None;

            #[cfg(feature = "virtio-blk-irq")]
            if inner.retired_completion.is_some() {
                drained_completion = inner.claim_ready(CompletionClaim::InlineSubmitter);
                #[cfg(feature = "neo-test-arthur-memory-io-trace")]
                {
                    trace = Some(inner.trace_queue_state());
                }
                return SubmitAttempt {
                    ret: BLOCK_RET_UNSUPPORTED,
                    drained_completion,
                    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
                    trace,
                };
            }

            let ret = if inner.in_flight.is_some() {
                BLOCK_RET_UNSUPPORTED
            } else {
                let pending = pending
                    .take()
                    .expect("virtio-blk pending request already queued");
                inner.submit_pending(pending, None, req_id);
                BLOCK_RET_OK
            };
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            {
                trace = Some(inner.trace_queue_state());
            }
            SubmitAttempt {
                ret,
                drained_completion,
                #[cfg(feature = "neo-test-arthur-memory-io-trace")]
                trace,
            }
        })
    }

    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    fn trace_submit_attempt(
        req_id: u64,
        submit_ret: i64,
        drained_completion: bool,
        trace: Option<TraceQueueState>,
    ) {
        if let Some(trace) = trace {
            serial_println!(
                "[ kernel ] virtio-blk: submit-state req_id={} outcome={} in_flight_req_id={} waiting={} retired={} next_avail_idx={} last_used_idx={} hw_used_idx={} armed_used_event={} event_idx={} tick={}",
                req_id,
                if submit_ret == BLOCK_RET_OK {
                    "queued"
                } else if drained_completion {
                    "drained"
                } else {
                    "busy"
                },
                trace.in_flight_req_id,
                trace.waiting,
                trace.retired,
                trace.next_avail_idx,
                trace.last_used_idx,
                trace.hw_used_idx,
                trace.armed_used_event,
                trace.event_idx,
                crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed)
            );
        }
    }

    fn block_result_from_return(ret: i64) -> BlockResult<()> {
        match ret {
            BLOCK_RET_OK => Ok(()),
            BLOCK_RET_UNSUPPORTED => Err(BlockError::Unsupported),
            _ => Err(BlockError::Io),
        }
    }

    fn poll_completions(&self, claim: CompletionClaim) {
        // `submit_pending_and_wait` may spin on this same lock with IF=0 while
        // parking the submitter; therefore the completion worker must not be
        // timer-preempted while holding it.
        let completion = self.claim_completion(claim);
        if let Some(completion) = completion {
            let _ = self.finish_completion(completion);
        }
    }

    fn claim_completion(&self, claim: CompletionClaim) -> Option<Completion> {
        crate::arch::x86_64::without_interrupts(|| {
            let mut inner = self.inner.lock();
            inner.claim_ready(claim)
        })
    }

    fn finish_completion(&self, completion: Completion) -> i64 {
        let completion_cpu = crate::arch::x86_64::per_cpu::current_cpu_id();
        let token = completion.token;
        let return_value = completion.return_value;
        let _status = completion.status;
        let _bdf = completion.bdf;
        #[cfg(feature = "neo-test-arthur-memory-io-trace")]
        let req_id = completion.req_id;
        #[cfg(feature = "neo-test-arthur-memory-io-trace")]
        let submit_avail_idx = completion.submit_avail_idx;
        #[cfg(feature = "neo-test-arthur-memory-io-trace")]
        let used_idx = completion.used_idx;
        #[cfg(feature = "neo-test-arthur-memory-io-trace")]
        let used_slot = completion.used_slot;
        #[cfg(feature = "neo-test-arthur-memory-io-trace")]
        let descriptor_id = completion.descriptor_id;
        let claim = completion.claim;
        drop(completion);

        LAST_COMPLETION_CPU.store(completion_cpu, Ordering::Release);
        let (token_cpu, token_slot, wake_result) = if let Some(token) = token {
            LAST_COMPLETION_TOKEN_CPU.store(token.cpu_id, Ordering::Release);
            LAST_COMPLETION_TOKEN_SLOT.store(token.slot, Ordering::Release);
            let result = sched::wake_thread_token(token, return_value);
            (token.cpu_id, token.slot, encode_wake_result(result))
        } else {
            LAST_COMPLETION_TOKEN_CPU.store(u32::MAX, Ordering::Release);
            LAST_COMPLETION_TOKEN_SLOT.store(u32::MAX, Ordering::Release);
            (u32::MAX, u32::MAX, WAKE_RESULT_INLINE)
        };
        LAST_WAKE_RESULT.store(wake_result, Ordering::Release);

        #[cfg(not(any(
            feature = "neo-test-neofs-large-file",
            feature = "neo-test-arthur-real-brain-boot"
        )))]
        #[cfg(feature = "neo-test-arthur-memory-io-trace")]
        serial_println!(
            "[ kernel ] virtio-blk: completion req_id={} bdf=0x{:06x} status={} tick={} claim={} completion_cpu={} token_cpu={} token_slot={} submit_avail_idx={} used_idx={} used_slot={} desc={} wake={}",
            req_id,
            _bdf,
            _status,
            crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed),
            completion_claim_name(claim),
            completion_cpu,
            token_cpu,
            token_slot,
            submit_avail_idx,
            used_idx,
            used_slot,
            descriptor_id,
            wake_result_name(wake_result)
        );
        // Throttled (see COMPLETION_TRACE_LIMIT). This supersedes the previous
        // per-scenario opt-outs for `neofs-large-file` / `arthur-real-brain-boot`:
        // those were ad-hoc patches around this same storm, and a bound that
        // applies to every scenario generalizes them.
        #[cfg(not(feature = "neo-test-arthur-memory-io-trace"))]
        {
            let emitted = COMPLETION_TRACE_EMITTED.fetch_add(1, Ordering::Relaxed);
            if emitted < COMPLETION_TRACE_LIMIT {
                serial_println!(
                    "[ kernel ] virtio-blk: completion bdf=0x{:06x} status={} claim={} completion_cpu={} token_cpu={} token_slot={} wake={}",
                    _bdf,
                    _status,
                    completion_claim_name(claim),
                    completion_cpu,
                    token_cpu,
                    token_slot,
                    wake_result_name(wake_result)
                );
            } else if emitted == COMPLETION_TRACE_LIMIT {
                serial_println!(
                    "[ kernel ] virtio-blk: completion trace suppressed after {} lines (per-completion serial output is throttled; running total via debug_completion_count)",
                    COMPLETION_TRACE_LIMIT
                );
            }
        }

        return_value
    }
}

struct VirtioBlkInner {
    common: VirtAddr,
    notify: VirtAddr,
    isr: VirtAddr,
    notify_multiplier: u32,
    queue: QueueMemory,
    next_avail_idx: u16,
    last_used_idx: u16,
    in_flight: Option<IoRendezvous>,
    retired_completion: Option<Completion>,
    event_idx: bool,
    bdf: u32,
}

#[cfg(feature = "neo-test-arthur-memory-io-trace")]
struct TraceQueueState {
    in_flight_req_id: u64,
    waiting: bool,
    retired: bool,
    next_avail_idx: u16,
    last_used_idx: u16,
    hw_used_idx: u16,
    armed_used_event: u16,
    event_idx: bool,
}

struct SubmitAttempt {
    ret: i64,
    drained_completion: Option<Completion>,
    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    trace: Option<TraceQueueState>,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum CompletionClaim {
    InlineSubmitter,
    Worker,
    Irq,
}

impl VirtioBlkInner {
    fn submit_pending(
        &mut self,
        pending: PendingIo,
        waiting_thread: Option<ThreadWakeToken>,
        req_id: u64,
    ) {
        #[cfg(not(feature = "neo-test-arthur-memory-io-trace"))]
        let _ = req_id;
        let PendingIo {
            op,
            sector,
            data,
            header,
            status,
            block_len,
            #[cfg(feature = "installer")]
            get_id,
        } = pending;

        write_request_header(&header, op, sector);
        #[cfg(feature = "installer")]
        if get_id {
            // SAFETY: the header buffer is this request's own, not yet posted.
            unsafe { write_volatile(header.as_mut_ptr() as *mut u32, VIRTIO_BLK_T_GET_ID.to_le()) };
        }
        unsafe {
            write_volatile(status.as_mut_ptr(), 0xff);
        }

        let head = 0u16;
        write_descriptor(
            &self.queue.desc,
            0,
            header.physical_addr().as_u64(),
            16,
            VIRTQ_DESC_F_NEXT,
            1,
        );
        let has_data_buffer = data.is_some();
        if let Some(data) = data.as_ref() {
            let data_flags = match op {
                BlockOp::Read => VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE,
                BlockOp::Write => VIRTQ_DESC_F_NEXT,
                BlockOp::Flush => unreachable!(),
            };
            write_descriptor(
                &self.queue.desc,
                1,
                data.physical_addr().as_u64(),
                block_len,
                data_flags,
                2,
            );
            write_descriptor(
                &self.queue.desc,
                2,
                status.physical_addr().as_u64(),
                1,
                VIRTQ_DESC_F_WRITE,
                0,
            );
        } else {
            write_descriptor(
                &self.queue.desc,
                1,
                status.physical_addr().as_u64(),
                1,
                VIRTQ_DESC_F_WRITE,
                0,
            );
        }

        let submit_avail_idx = self.next_avail_idx;
        let avail_slot = submit_avail_idx % self.queue.size;
        write_avail_ring(&self.queue.avail, avail_slot, head);
        descriptor_publish_barrier();
        self.arm_completion_interrupt(submit_avail_idx);
        self.next_avail_idx = self.next_avail_idx.wrapping_add(1);
        write_avail_idx(&self.queue.avail, self.next_avail_idx);
        notification_suppression_barrier();

        let mut dma_buffers = Vec::new();
        dma_buffers.push(header);
        if let Some(data) = data {
            dma_buffers.push(data);
        }
        dma_buffers.push(status.clone());
        if op == BlockOp::Flush {
            FLUSH_COMMANDS.fetch_add(1, Ordering::AcqRel);
        }
        IN_FLIGHT_DMA_BUFFERS.store(dma_buffers.len() as u32, Ordering::Release);
        self.in_flight = Some(IoRendezvous {
            done: AtomicBool::new(false),
            waiting_thread,
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            req_id,
            op,
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            submit_avail_idx,
            status,
            has_data_buffer,
            dma_buffers,
            result: IoResult::Pending,
        });

        notify_queue(self.notify, self.queue.notify_off, self.notify_multiplier);
    }

    fn attach_waiter(&mut self, token: ThreadWakeToken) {
        let rendezvous = self
            .in_flight
            .as_mut()
            .expect("virtio-blk slow-path park without in-flight request");
        assert!(
            rendezvous.waiting_thread.is_none(),
            "virtio-blk in-flight request already has a waiter"
        );
        rendezvous.waiting_thread = Some(token);
    }

    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    fn trace_queue_state(&self) -> TraceQueueState {
        TraceQueueState {
            in_flight_req_id: self
                .in_flight
                .as_ref()
                .map_or(u64::MAX, |rendezvous| rendezvous.req_id),
            waiting: self
                .in_flight
                .as_ref()
                .is_some_and(|rendezvous| rendezvous.waiting_thread.is_some()),
            retired: self.retired_completion.is_some(),
            next_avail_idx: self.next_avail_idx,
            last_used_idx: self.last_used_idx,
            hw_used_idx: read_used_idx(&self.queue.used),
            armed_used_event: read_used_event(&self.queue.avail, self.queue.size),
            event_idx: self.event_idx,
        }
    }

    fn arm_completion_interrupt(&self, idx: u16) {
        if self.event_idx {
            write_used_event(&self.queue.avail, self.queue.size, idx);
            fence(Ordering::Release);
        }
    }

    fn claim_ready(&mut self, claim: CompletionClaim) -> Option<Completion> {
        if claim == CompletionClaim::InlineSubmitter
            && let Some(completion) = self.retired_completion.take()
        {
            return Some(completion);
        }
        self.poll_used(claim)
    }

    fn poll_used(&mut self, claim: CompletionClaim) -> Option<Completion> {
        #[cfg(feature = "neo-test")]
        if DEBUG_HOLD_COMPLETIONS.load(Ordering::Acquire) {
            return None;
        }

        let used_idx = read_used_idx(&self.queue.used);
        if used_idx == self.last_used_idx {
            return None;
        }
        if claim == CompletionClaim::Worker
            && self
                .in_flight
                .as_ref()
                .is_none_or(|rendezvous| rendezvous.waiting_thread.is_none())
        {
            return None;
        }
        used_ring_acquire_barrier();

        let used_idx = self.last_used_idx;
        let used_slot = used_idx % self.queue.size;
        let descriptor_id = read_used_elem_id(&self.queue.used, used_slot);
        let _used_len = read_used_elem_len(&self.queue.used, used_slot);
        self.last_used_idx = self.last_used_idx.wrapping_add(1);
        self.arm_completion_interrupt(self.last_used_idx);

        let mut rendezvous = self.in_flight.take()?;
        let status = unsafe { read_volatile(rendezvous.status.as_ptr()) };
        let return_value = if descriptor_id == 0 && status == VIRTIO_BLK_S_OK {
            rendezvous.result = IoResult::Ok;
            BLOCK_RET_OK
        } else {
            rendezvous.result = IoResult::Io;
            BLOCK_RET_IO
        };
        rendezvous.done.store(true, Ordering::Release);
        LAST_COMPLETED_REQUEST_TYPE.store(block_request_type(rendezvous.op), Ordering::Release);
        LAST_COMPLETED_DMA_BUFFERS.store(rendezvous.dma_buffers.len() as u32, Ordering::Release);
        LAST_COMPLETED_HAS_DATA_BUFFER.store(rendezvous.has_data_buffer, Ordering::Release);
        if rendezvous.op == BlockOp::Flush && status == VIRTIO_BLK_S_OK {
            FLUSH_COMPLETIONS.fetch_add(1, Ordering::AcqRel);
        }
        IN_FLIGHT_DMA_BUFFERS.store(0, Ordering::Release);
        COMPLETIONS.fetch_add(1, Ordering::AcqRel);
        if claim == CompletionClaim::Irq {
            IRQ_COMPLETIONS.fetch_add(1, Ordering::AcqRel);
        }

        // Reading the ISR status is the virtio INTx/MSI-X acknowledgement hook.
        // BUILD-1's worker and future IRQ handler share this completion drain.
        let _isr = unsafe { mmio_read8(self.isr, 0) };

        let token = rendezvous.waiting_thread.take();
        let completion = Completion {
            token,
            return_value,
            status,
            bdf: self.bdf,
            claim,
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            req_id: rendezvous.req_id,
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            submit_avail_idx: rendezvous.submit_avail_idx,
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            used_idx,
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            used_slot,
            #[cfg(feature = "neo-test-arthur-memory-io-trace")]
            descriptor_id,
            _rendezvous: rendezvous,
        };
        if claim == CompletionClaim::Irq && completion.token.is_none() {
            assert!(
                self.retired_completion.is_none(),
                "virtio-blk retired completion slot already occupied"
            );
            self.retired_completion = Some(completion);
            return None;
        }
        Some(completion)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum IoResult {
    Pending,
    Ok,
    Io,
}

struct IoRendezvous {
    done: AtomicBool,
    waiting_thread: Option<ThreadWakeToken>,
    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    req_id: u64,
    op: BlockOp,
    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    submit_avail_idx: u16,
    status: Arc<DmaBuffer>,
    has_data_buffer: bool,
    dma_buffers: Vec<Arc<DmaBuffer>>,
    result: IoResult,
}

struct Completion {
    token: Option<ThreadWakeToken>,
    return_value: i64,
    status: u8,
    bdf: u32,
    claim: CompletionClaim,
    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    req_id: u64,
    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    submit_avail_idx: u16,
    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    used_idx: u16,
    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    used_slot: u16,
    #[cfg(feature = "neo-test-arthur-memory-io-trace")]
    descriptor_id: u32,
    _rendezvous: IoRendezvous,
}

struct PendingIo {
    op: BlockOp,
    sector: u64,
    data: Option<Arc<DmaBuffer>>,
    header: Arc<DmaBuffer>,
    status: Arc<DmaBuffer>,
    block_len: u32,
    /// OT-87 R4.1: a device-writable request sent as `GET_ID`, not `IN`.
    #[cfg(feature = "installer")]
    get_id: bool,
}

impl PendingIo {
    fn new_data(
        op: BlockOp,
        lba: u64,
        data: Arc<DmaBuffer>,
        logical_block_size: u32,
    ) -> BlockResult<Self> {
        if op == BlockOp::Flush {
            return Err(BlockError::InvalidRequest);
        }
        if logical_block_size == 0 || !logical_block_size.is_multiple_of(SECTOR_SIZE) {
            return Err(BlockError::InvalidRequest);
        }
        if data.len() < logical_block_size as usize
            || !data.len().is_multiple_of(logical_block_size as usize)
            || data.len() > u32::MAX as usize
        {
            return Err(BlockError::InvalidRequest);
        }
        let data_len = data.len() as u32;
        let sectors_per_block = logical_block_size / SECTOR_SIZE;
        let sector = lba
            .checked_mul(sectors_per_block as u64)
            .ok_or(BlockError::InvalidRequest)?;
        let header = DmaBuffer::alloc_zeroed(16).map_err(|_| BlockError::Io)?;
        let status = DmaBuffer::alloc_zeroed(1).map_err(|_| BlockError::Io)?;
        Ok(Self {
            op,
            sector,
            data: Some(data),
            header,
            status,
            block_len: data_len,
            #[cfg(feature = "installer")]
            get_id: false,
        })
    }

    fn new_flush() -> BlockResult<Self> {
        let header = DmaBuffer::alloc_zeroed(16).map_err(|_| BlockError::Io)?;
        let status = DmaBuffer::alloc_zeroed(1).map_err(|_| BlockError::Io)?;
        Ok(Self {
            op: BlockOp::Flush,
            sector: 0,
            data: None,
            header,
            status,
            block_len: 0,
            #[cfg(feature = "installer")]
            get_id: false,
        })
    }
}

struct QueueMemory {
    size: u16,
    notify_off: u16,
    desc: Arc<DmaBuffer>,
    avail: Arc<DmaBuffer>,
    used: Arc<DmaBuffer>,
}

fn setup_queue(common: VirtAddr, notify: Region, _isr: Region) -> Result<QueueMemory, ProbeError> {
    unsafe {
        mmio_write16(common, COMMON_QUEUE_SELECT, QUEUE_INDEX);
    }
    let max_size = unsafe { mmio_read16(common, COMMON_QUEUE_SIZE) };
    let queue_size = cmp::min(VIRTQUEUE_SIZE, max_size);
    let queue_size = if queue_size >= 8 {
        8
    } else if queue_size >= 4 {
        4
    } else {
        return Err(ProbeError::QueueUnavailable);
    };
    let notify_off = unsafe { mmio_read16(common, COMMON_QUEUE_NOTIFY_OFF) };
    let desc =
        DmaBuffer::alloc_zeroed(desc_table_bytes(queue_size)).map_err(|_| ProbeError::Dma)?;
    let avail =
        DmaBuffer::alloc_zeroed(avail_ring_bytes(queue_size)).map_err(|_| ProbeError::Dma)?;
    let used = DmaBuffer::alloc_zeroed(used_ring_bytes(queue_size)).map_err(|_| ProbeError::Dma)?;

    unsafe {
        mmio_write16(common, COMMON_QUEUE_SIZE, queue_size);
        mmio_write64(common, COMMON_QUEUE_DESC, desc.physical_addr().as_u64());
        mmio_write64(common, COMMON_QUEUE_DRIVER, avail.physical_addr().as_u64());
        mmio_write64(common, COMMON_QUEUE_DEVICE, used.physical_addr().as_u64());
        mmio_write16(common, COMMON_QUEUE_ENABLE, 1);
    }

    let _ = notify;
    Ok(QueueMemory {
        size: queue_size,
        notify_off,
        desc,
        avail,
        used,
    })
}

fn configure_completion_interrupt(
    dev: Device,
    common: VirtAddr,
) -> Result<(bool, bool), ProbeError> {
    #[cfg(feature = "virtio-blk-irq")]
    {
        let msix = pci::configure_msix_fixed_vector(dev, VIRTIO_BLK_MSI_VECTOR, 0);
        let msi = if msix {
            false
        } else {
            pci::configure_msi_fixed_vector(dev, VIRTIO_BLK_MSI_VECTOR)
        };
        if !msix && !msi {
            fail_device(common);
            return Err(ProbeError::InterruptUnavailable);
        }
        if msix {
            unsafe {
                mmio_write16(common, COMMON_CONFIG_MSIX_VECTOR, 0);
                mmio_write16(common, COMMON_QUEUE_MSIX_VECTOR, 0);
            }
        }
        MSIX_CONFIGURED.store(msix, Ordering::Release);
        MSI_CONFIGURED.store(msi, Ordering::Release);
        Ok((msix, msi))
    }

    #[cfg(not(feature = "virtio-blk-irq"))]
    {
        let _ = (dev, common);
        Ok((false, false))
    }
}

fn reset_probe_state(dev: Device) {
    SEEN.store(true, Ordering::Release);
    PROBE_OK.store(false, Ordering::Release);
    COMMON_CONFIG.store(false, Ordering::Release);
    NOTIFY_CONFIG.store(false, Ordering::Release);
    ISR_CONFIG.store(false, Ordering::Release);
    DEVICE_CONFIG.store(false, Ordering::Release);
    MSIX_CONFIGURED.store(false, Ordering::Release);
    MSI_CONFIGURED.store(false, Ordering::Release);
    OFFERED_FEATURES.store(0, Ordering::Release);
    DRIVER_FEATURES.store(0, Ordering::Release);
    FLUSH_OFFERED.store(false, Ordering::Release);
    EVENT_IDX_NEGOTIATED.store(false, Ordering::Release);
    QUEUE_READY.store(false, Ordering::Release);
    LAST_BDF.store(pack_bdf(dev), Ordering::Release);
    LAST_WAKE_RESULT.store(WAKE_RESULT_NONE, Ordering::Release);
    LAST_COMPLETION_CPU.store(u32::MAX, Ordering::Release);
    LAST_COMPLETION_TOKEN_CPU.store(u32::MAX, Ordering::Release);
    LAST_COMPLETION_TOKEN_SLOT.store(u32::MAX, Ordering::Release);
    FLUSH_COMMANDS.store(0, Ordering::Release);
    FLUSH_COMPLETIONS.store(0, Ordering::Release);
    LAST_COMPLETED_REQUEST_TYPE.store(u32::MAX, Ordering::Release);
    LAST_COMPLETED_DMA_BUFFERS.store(0, Ordering::Release);
    LAST_COMPLETED_HAS_DATA_BUFFER.store(false, Ordering::Release);
    INTERRUPTS.store(0, Ordering::Release);
    IRQ_COMPLETIONS.store(0, Ordering::Release);
}

/// OT-87 R4.3: the logical block size to present: the device's `blk_size`
/// when F_BLK_SIZE was negotiated and it is 4096, else 512.
#[cfg(feature = "virtio-blk-size")]
fn logical_block_size(driver_features: u64, device_config: VirtAddr) -> u32 {
    if driver_features & VIRTIO_BLK_F_BLK_SIZE == 0 {
        return SECTOR_SIZE;
    }
    // SAFETY: the device-config region is mapped for the device's life and
    // `blk_size` is in it (§5.2.4).
    match unsafe { mmio_read32(device_config, DEVICE_CONFIG_BLK_SIZE) } {
        4096 => 4096,
        _ => SECTOR_SIZE,
    }
}

fn read_device_features(common: VirtAddr) -> u64 {
    unsafe {
        mmio_write32(common, COMMON_DEVICE_FEATURE_SELECT, 0);
        let lo = mmio_read32(common, COMMON_DEVICE_FEATURE) as u64;
        mmio_write32(common, COMMON_DEVICE_FEATURE_SELECT, 1);
        let hi = mmio_read32(common, COMMON_DEVICE_FEATURE) as u64;
        (hi << 32) | lo
    }
}

fn write_driver_features(common: VirtAddr, features: u64) {
    unsafe {
        mmio_write32(common, COMMON_DRIVER_FEATURE_SELECT, 0);
        mmio_write32(common, COMMON_DRIVER_FEATURE, features as u32);
        mmio_write32(common, COMMON_DRIVER_FEATURE_SELECT, 1);
        mmio_write32(common, COMMON_DRIVER_FEATURE, (features >> 32) as u32);
    }
}

/// OT-90 P3b-2 (K6): reset the device (status 0), registers only.
#[cfg(feature = "power-quiesce")]
pub fn shutdown_reset(common: u64) {
    reset_device(VirtAddr::new(common));
}

fn reset_device(common: VirtAddr) {
    unsafe {
        mmio_write8(common, COMMON_DEVICE_STATUS, 0);
        for _ in 0..100_000 {
            if mmio_read8(common, COMMON_DEVICE_STATUS) == 0 {
                break;
            }
            core::hint::spin_loop();
        }
    }
}

fn add_status(common: VirtAddr, bit: u8) {
    unsafe {
        let status = mmio_read8(common, COMMON_DEVICE_STATUS);
        mmio_write8(common, COMMON_DEVICE_STATUS, status | bit);
    }
}

fn fail_device(common: VirtAddr) {
    unsafe {
        let status = mmio_read8(common, COMMON_DEVICE_STATUS);
        mmio_write8(common, COMMON_DEVICE_STATUS, status | STATUS_FAILED);
    }
}

#[cfg(any(
    not(feature = "virtio-blk-irq"),
    feature = "neo-test-phase3-storage-completion-cross-cpu-wake"
))]
fn ensure_completion_worker() {
    if COMPLETION_WORKER_STARTED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        sched::spawn("virtio-blk-completion", virtio_blk_completion_worker);
    }
}

#[cfg(any(
    not(feature = "virtio-blk-irq"),
    feature = "neo-test-phase3-storage-completion-cross-cpu-wake"
))]
extern "C" fn virtio_blk_completion_worker() -> ! {
    loop {
        poll_all_completions(CompletionClaim::Worker);
        sched::yield_now();
    }
}

fn poll_all_completions(claim: CompletionClaim) {
    let devices = VIRTIO_DEVICES.lock();
    for device in devices.iter() {
        device.poll_completions(claim);
    }
}

#[allow(dead_code)]
fn primary_device() -> Option<Arc<VirtioBlkDevice>> {
    PRIMARY_DEVICE.lock().as_ref().cloned()
}

fn write_request_header(header: &DmaBuffer, op: BlockOp, sector: u64) {
    let request_type = block_request_type(op);
    unsafe {
        let ptr = header.as_mut_ptr();
        write_volatile(ptr as *mut u32, request_type.to_le());
        write_volatile(ptr.add(4) as *mut u32, 0);
        write_volatile(ptr.add(8) as *mut u64, sector.to_le());
    }
}

fn block_request_type(op: BlockOp) -> u32 {
    match op {
        BlockOp::Read => VIRTIO_BLK_T_IN,
        BlockOp::Write => VIRTIO_BLK_T_OUT,
        BlockOp::Flush => VIRTIO_BLK_T_FLUSH,
    }
}

fn write_descriptor(buffer: &DmaBuffer, index: u16, addr: u64, len: u32, flags: u16, next: u16) {
    let offset = index as usize * 16;
    unsafe {
        let ptr = buffer.as_mut_ptr().add(offset);
        write_volatile(ptr as *mut u64, addr.to_le());
        write_volatile(ptr.add(8) as *mut u32, len.to_le());
        write_volatile(ptr.add(12) as *mut u16, flags.to_le());
        write_volatile(ptr.add(14) as *mut u16, next.to_le());
    }
}

fn write_avail_ring(buffer: &DmaBuffer, slot: u16, head: u16) {
    unsafe {
        let ptr = buffer.as_mut_ptr().add(4 + slot as usize * 2) as *mut u16;
        write_volatile(ptr, head.to_le());
    }
}

fn write_avail_idx(buffer: &DmaBuffer, idx: u16) {
    unsafe {
        write_volatile(buffer.as_mut_ptr().add(2) as *mut u16, idx.to_le());
    }
}

// EVENT_IDX stores the driver-to-device completion interrupt threshold in the
// split avail ring's trailing `used_event` field.
fn write_used_event(buffer: &DmaBuffer, queue_size: u16, idx: u16) {
    unsafe {
        let offset = 4 + queue_size as usize * 2;
        write_volatile(buffer.as_mut_ptr().add(offset) as *mut u16, idx.to_le());
    }
}

#[cfg(feature = "neo-test-arthur-memory-io-trace")]
fn read_used_event(buffer: &DmaBuffer, queue_size: u16) -> u16 {
    unsafe {
        let offset = 4 + queue_size as usize * 2;
        u16::from_le(read_volatile(buffer.as_ptr().add(offset) as *const u16))
    }
}

fn read_used_idx(buffer: &DmaBuffer) -> u16 {
    unsafe { u16::from_le(read_volatile(buffer.as_ptr().add(2) as *const u16)) }
}

fn read_used_elem_id(buffer: &DmaBuffer, slot: u16) -> u32 {
    unsafe {
        u32::from_le(read_volatile(
            buffer.as_ptr().add(4 + slot as usize * 8) as *const u32
        ))
    }
}

fn read_used_elem_len(buffer: &DmaBuffer, slot: u16) -> u32 {
    unsafe {
        u32::from_le(read_volatile(
            buffer.as_ptr().add(4 + slot as usize * 8 + 4) as *const u32,
        ))
    }
}

fn notify_queue(notify: VirtAddr, queue_notify_off: u16, notify_multiplier: u32) {
    let offset = queue_notify_off as usize * notify_multiplier as usize;
    unsafe {
        write_volatile(
            (notify.as_u64() as usize + offset) as *mut u16,
            QUEUE_INDEX.to_le(),
        );
    }
}

fn descriptor_publish_barrier() {
    DESCRIPTOR_PUBLISH_BARRIERS.fetch_add(1, Ordering::AcqRel);
    super::virtio_barrier::publish_descriptors();
}

fn notification_suppression_barrier() {
    NOTIFICATION_SUPPRESSION_BARRIERS.fetch_add(1, Ordering::AcqRel);
    super::virtio_barrier::before_notification_check();
}

fn used_ring_acquire_barrier() {
    fence(Ordering::Acquire);
}

#[cfg(feature = "neo-test-phase3-storage-completion-cross-cpu-wake")]
fn inline_completion_poll_enabled() -> bool {
    false
}

#[cfg(not(feature = "neo-test-phase3-storage-completion-cross-cpu-wake"))]
fn inline_completion_poll_enabled() -> bool {
    true
}

fn desc_table_bytes(queue_size: u16) -> usize {
    queue_size as usize * 16
}

fn avail_ring_bytes(queue_size: u16) -> usize {
    6 + queue_size as usize * 2
}

fn used_ring_bytes(queue_size: u16) -> usize {
    6 + queue_size as usize * 8
}

fn encode_wake_result(result: WakeTokenResult) -> u32 {
    match result {
        WakeTokenResult::Woken => WAKE_RESULT_WOKEN,
        WakeTokenResult::Gone => WAKE_RESULT_GONE,
        WakeTokenResult::Stale => WAKE_RESULT_STALE,
    }
}

fn wake_result_name(encoded: u32) -> &'static str {
    match encoded {
        WAKE_RESULT_WOKEN => "Woken",
        WAKE_RESULT_GONE => "Gone",
        WAKE_RESULT_STALE => "Stale",
        WAKE_RESULT_INLINE => "Inline",
        _ => "None",
    }
}

fn completion_claim_name(claim: CompletionClaim) -> &'static str {
    match claim {
        CompletionClaim::InlineSubmitter => "inline",
        CompletionClaim::Worker => "worker",
        CompletionClaim::Irq => "irq",
    }
}

fn pack_bdf(dev: Device) -> u32 {
    ((dev.bus as u32) << 16) | ((dev.device as u32) << 8) | dev.function as u32
}

unsafe fn mmio_read8(base: VirtAddr, offset: usize) -> u8 {
    unsafe { read_volatile((base.as_u64() as usize + offset) as *const u8) }
}

unsafe fn mmio_read16(base: VirtAddr, offset: usize) -> u16 {
    unsafe {
        u16::from_le(read_volatile(
            (base.as_u64() as usize + offset) as *const u16,
        ))
    }
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

unsafe fn mmio_write8(base: VirtAddr, offset: usize, value: u8) {
    unsafe { write_volatile((base.as_u64() as usize + offset) as *mut u8, value) }
}

unsafe fn mmio_write16(base: VirtAddr, offset: usize, value: u16) {
    unsafe { write_volatile((base.as_u64() as usize + offset) as *mut u16, value.to_le()) }
}

unsafe fn mmio_write32(base: VirtAddr, offset: usize, value: u32) {
    unsafe { write_volatile((base.as_u64() as usize + offset) as *mut u32, value.to_le()) }
}

unsafe fn mmio_write64(base: VirtAddr, offset: usize, value: u64) {
    unsafe { write_volatile((base.as_u64() as usize + offset) as *mut u64, value.to_le()) }
}
