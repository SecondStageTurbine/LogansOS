// SPDX-License-Identifier: MPL-2.0
//! Virtio accelerator transport for ADR-031 BUILD-3.
//!
//! BUILD-3a proved the transport with Identity; BUILD-3b carries the full
//! op set over the same queue/IRQ path.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering, fence};
use core::time::Duration;

use spin::Lazy;
use x86_64::VirtAddr;

use crate::ai::ops::OpKind;
use crate::ai::tensor::{Device, Dtype, PAGE_SIZE, Shape};
use crate::drivers::block::{self, DmaBuffer};
use crate::drivers::pci::{self, Device as PciDevice, VirtioPciCap};
use crate::sched;
use crate::serial_println;
use crate::sync::{IrqSafeMutex, SwitchMutex};

use super::{
    AccelDescriptor, AccelDispatchKind, AccelDriver, AccelError, AccelMemoryKind, AccelResult,
    DeviceStorage, DispatchStatus, DispatchToken, OpBatch,
};

pub const VIRTIO_VENDOR_ID: u16 = 0x1af4;
// ADR-031 left the virtio ID provisional. QEMU 10.2.3's user-creatable
// vhost-user-test-device only accepts IDs in its virtio name table; 41 is
// available in Neo's guest match table and does not collide with block/net.
pub const VIRTIO_ID_ACCEL: u16 = 41;
pub const VIRTIO_ACCEL_MODERN_DEVICE_ID: u16 = 0x1040 + VIRTIO_ID_ACCEL;
pub const VIRTIO_ACCEL_MSI_VECTOR: u8 = 0xf2;

const DEVICE_ID: Device = Device::Accel(2);
const QUEUE_INDEX: u16 = 0;
const VIRTQUEUE_SIZE: u16 = 8;
const MAX_INPUTS: usize = 3;
const MAX_TENSOR_BYTES: u64 = 1 << 20;

const VIRTIO_F_VERSION_1: u64 = 1 << 32;

const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;
const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;

const COMMON_DEVICE_FEATURE_SELECT: usize = 0x00;
const COMMON_DEVICE_FEATURE: usize = 0x04;
const COMMON_DRIVER_FEATURE_SELECT: usize = 0x08;
const COMMON_DRIVER_FEATURE: usize = 0x0c;
const COMMON_CONFIG_MSIX_VECTOR: usize = 0x10;
const COMMON_DEVICE_STATUS: usize = 0x14;
const COMMON_QUEUE_SELECT: usize = 0x16;
const COMMON_QUEUE_SIZE: usize = 0x18;
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

const CONFIG_SUPPORTED_OPS: usize = 0x00;
const CONFIG_SUPPORTED_DTYPES: usize = 0x04;
const CONFIG_MAX_INPUTS: usize = 0x08;
const CONFIG_QUEUE_SIZE: usize = 0x0c;
const CONFIG_MAX_TENSOR_BYTES: usize = 0x10;

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

const WIRE_DTYPE_F32: u32 = 0;
const WIRE_DTYPE_I32: u32 = 5;
const STATUS_OK: u32 = 0;
const STATUS_UNSUPPORTED_OP: u32 = 1;
const STATUS_UNSUPPORTED_DTYPE: u32 = 2;
const STATUS_BAD_REQUEST: u32 = 3;
const STATUS_SHAPE_MISMATCH: u32 = 4;
const STATUS_INTERNAL: u32 = 5;

const REQ_HEADER_LEN: usize = 16;
const TENSOR_DESC_LEN: usize = 80;
const STATUS_LEN: usize = 16;
const IDENTITY_DESC_BLOCK_LEN: usize = REQ_HEADER_LEN + TENSOR_DESC_LEN * 2;
const ADD_DESC_BLOCK_LEN: usize = REQ_HEADER_LEN + TENSOR_DESC_LEN * 3;
const MAX_DESC_BLOCK_LEN: usize = REQ_HEADER_LEN + TENSOR_DESC_LEN * (MAX_INPUTS + 1);
const IDENTITY_INPUT_BYTES: [u8; 8] = [0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40];
const IDENTITY_STATUS_ORACLE: [u8; STATUS_LEN] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const ADD_LHS_BYTES: [u8; 8] = [0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x40];
const ADD_RHS_BYTES: [u8; 8] = [0x00, 0x00, 0x40, 0x40, 0x00, 0x00, 0x80, 0x40];
const ADD_OUTPUT_ORACLE: [u8; 8] = [0x00, 0x00, 0x80, 0x40, 0x00, 0x00, 0xc0, 0x40];
const ADD_STATUS_ORACLE: [u8; STATUS_LEN] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const IDENTITY_DESC_BLOCK_ORACLE: [u8; IDENTITY_DESC_BLOCK_LEN] = [
    // VirtioAccelReqHeader { op_kind: Identity, input_count: 1, flags: 0, reserved: 0 }
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // input VirtioAccelTensorDesc { dtype: F32, rank: 1, dims: [2], byte_len: 8 }
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // output VirtioAccelTensorDesc { dtype: F32, rank: 1, dims: [2], byte_len: 8 }
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const ADD_DESC_BLOCK_ORACLE: [u8; ADD_DESC_BLOCK_LEN] = [
    // VirtioAccelReqHeader { op_kind: Add, input_count: 2, flags: 0, reserved: 0 }
    0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // input0 VirtioAccelTensorDesc
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // input1 VirtioAccelTensorDesc
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // output VirtioAccelTensorDesc
    0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

static SUPPORTED_OPS: &[OpKind] = &[
    OpKind::Identity,
    OpKind::Add,
    OpKind::Mul,
    OpKind::Matmul,
    OpKind::Relu,
    OpKind::Gelu,
    OpKind::Softmax,
    OpKind::LayerNorm,
    OpKind::RmsNorm,
    OpKind::Embedding,
    OpKind::Reshape,
    OpKind::Transpose,
];
static SUPPORTED_DTYPES: &[Dtype] = &[Dtype::F32, Dtype::I32];

static PRIMARY_DEVICE: SwitchMutex<Option<Arc<VirtioAccelDriver>>> = SwitchMutex::new(None);
static DEVICES: Lazy<IrqSafeMutex<Vec<Arc<VirtioAccelDriver>>>> =
    Lazy::new(|| IrqSafeMutex::new(Vec::new()));

static SEEN: AtomicBool = AtomicBool::new(false);
static PROBE_OK: AtomicBool = AtomicBool::new(false);
static COMMON_CONFIG: AtomicBool = AtomicBool::new(false);
static NOTIFY_CONFIG: AtomicBool = AtomicBool::new(false);
static ISR_CONFIG: AtomicBool = AtomicBool::new(false);
static DEVICE_CONFIG: AtomicBool = AtomicBool::new(false);
static MSIX_CONFIGURED: AtomicBool = AtomicBool::new(false);
static MSI_CONFIGURED: AtomicBool = AtomicBool::new(false);
static QUEUE_READY: AtomicBool = AtomicBool::new(false);
static OFFERED_FEATURES: AtomicU64 = AtomicU64::new(0);
static DRIVER_FEATURES: AtomicU64 = AtomicU64::new(0);
static CONFIG_SUPPORTED_OPS_SEEN: AtomicU32 = AtomicU32::new(0);
static CONFIG_SUPPORTED_DTYPES_SEEN: AtomicU32 = AtomicU32::new(0);
static CONFIG_MAX_INPUTS_SEEN: AtomicU32 = AtomicU32::new(0);
static CONFIG_QUEUE_SIZE_SEEN: AtomicU32 = AtomicU32::new(0);
static CONFIG_MAX_TENSOR_BYTES_SEEN: AtomicU64 = AtomicU64::new(0);
static LAST_BDF: AtomicU32 = AtomicU32::new(0);
static DESCRIPTOR_PUBLISH_BARRIERS: AtomicU64 = AtomicU64::new(0);
static NOTIFICATION_SUPPRESSION_BARRIERS: AtomicU64 = AtomicU64::new(0);
static COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static IRQ_COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static INTERRUPTS: AtomicU64 = AtomicU64::new(0);
static IN_FLIGHT_DMA_BUFFERS: AtomicU32 = AtomicU32::new(0);
static LAST_DESC_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);
static LAST_INPUT_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);
static LAST_STATUS_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);
static LAST_ADD_DESC_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);
static LAST_ADD_INPUT_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);
static LAST_ADD_OUTPUT_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);
static LAST_ADD_STATUS_ORACLE_MATCH: AtomicBool = AtomicBool::new(false);

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ProbeError {
    MissingCommonConfig,
    MissingNotifyConfig,
    MissingIsrConfig,
    MissingDeviceConfig,
    ModernFeatureNotOffered,
    FeatureNegotiationRejected,
    QueueUnavailable,
    Dma,
    InterruptUnavailable,
    ConfigRejected,
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
    pub queue_ready: bool,
    pub offered_features: u64,
    pub driver_features: u64,
    pub supported_ops: u32,
    pub supported_dtypes: u32,
    pub max_inputs: u32,
    pub queue_size: u32,
    pub max_tensor_bytes: u64,
    pub last_bdf: u32,
}

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

pub fn init() {
    let summary = crate::drivers::pci::scan_accel_devices();
    serial_println!(
        "[ kernel ] accel: virtio scan complete functions={} virtio_accel={} virtio_accel_ok={}",
        summary.functions_seen,
        summary.virtio_accel_seen,
        summary.virtio_accel_probe_ok
    );
}

pub fn probe_pci(dev: PciDevice) -> Result<(), ProbeError> {
    reset_probe_state(dev);

    let mut regions = Regions::default();
    pci::for_each_virtio_vendor_cap(dev, |cap| {
        serial_println!(
            "[ kernel ] virtio-accel: cap bdf={:02x}:{:02x}.{} type={} bar={} offset=0x{:x} length=0x{:x}",
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
    let msix = pci::configure_msix_fixed_vector(dev, VIRTIO_ACCEL_MSI_VECTOR, 0);
    let msi = if msix {
        false
    } else {
        pci::configure_msi_fixed_vector(dev, VIRTIO_ACCEL_MSI_VECTOR)
    };
    MSIX_CONFIGURED.store(msix, Ordering::Release);
    MSI_CONFIGURED.store(msi, Ordering::Release);
    if !msix && !msi {
        return Err(ProbeError::InterruptUnavailable);
    }

    reset_device(common.virt);
    let offered = read_device_features(common.virt);
    OFFERED_FEATURES.store(offered, Ordering::Release);
    if offered & VIRTIO_F_VERSION_1 == 0 {
        fail_device(common.virt);
        return Err(ProbeError::ModernFeatureNotOffered);
    }

    add_status(common.virt, STATUS_ACKNOWLEDGE);
    add_status(common.virt, STATUS_DRIVER);
    write_driver_features(common.virt, VIRTIO_F_VERSION_1);
    DRIVER_FEATURES.store(VIRTIO_F_VERSION_1, Ordering::Release);
    add_status(common.virt, STATUS_FEATURES_OK);
    if unsafe { mmio_read8(common.virt, COMMON_DEVICE_STATUS) } & STATUS_FEATURES_OK == 0 {
        fail_device(common.virt);
        return Err(ProbeError::FeatureNegotiationRejected);
    }

    let config = read_config(device_config.virt);
    store_config_snapshot(config);
    if !config.supports_required_ops_f32_i32() || config.max_inputs < MAX_INPUTS as u32 {
        fail_device(common.virt);
        return Err(ProbeError::ConfigRejected);
    }

    let queue = setup_queue(common.virt, notify, msix)?;
    let queue_size = queue.size;
    let bdf = pack_bdf(dev);
    let device = Arc::new(VirtioAccelDriver {
        inner: IrqSafeMutex::new(VirtioAccelInner {
            common: common.virt,
            notify: notify.virt,
            isr: isr.virt,
            notify_multiplier: notify.cap.notify_off_multiplier,
            queue,
            next_avail_idx: 0,
            last_used_idx: 0,
            in_flight: None,
            last_completion: None,
            bdf,
        }),
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
    DEVICES.lock().push(device.clone());
    let trait_device: Arc<dyn AccelDriver> = device.clone();
    super::register_device(trait_device);

    serial_println!(
        "[ kernel ] virtio-accel: attached bdf={:02x}:{:02x}.{} offered=0x{:016x} driver=0x{:016x} queue_size={} supported_ops=0x{:08x} supported_dtypes=0x{:08x} max_inputs={} max_tensor_bytes={} msix={} msi={}",
        dev.bus,
        dev.device,
        dev.function,
        offered,
        VIRTIO_F_VERSION_1,
        queue_size,
        config.supported_ops,
        config.supported_dtypes,
        config.max_inputs,
        config.max_tensor_bytes,
        msix,
        msi
    );

    Ok(())
}

pub fn handle_interrupt() {
    INTERRUPTS.fetch_add(1, Ordering::AcqRel);
    poll_all_completions(true);
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
        queue_ready: QUEUE_READY.load(Ordering::Acquire),
        offered_features: OFFERED_FEATURES.load(Ordering::Acquire),
        driver_features: DRIVER_FEATURES.load(Ordering::Acquire),
        supported_ops: CONFIG_SUPPORTED_OPS_SEEN.load(Ordering::Acquire),
        supported_dtypes: CONFIG_SUPPORTED_DTYPES_SEEN.load(Ordering::Acquire),
        max_inputs: CONFIG_MAX_INPUTS_SEEN.load(Ordering::Acquire),
        queue_size: CONFIG_QUEUE_SIZE_SEEN.load(Ordering::Acquire),
        max_tensor_bytes: CONFIG_MAX_TENSOR_BYTES_SEEN.load(Ordering::Acquire),
        last_bdf: LAST_BDF.load(Ordering::Acquire),
    }
}

#[cfg(feature = "neo-test")]
pub fn debug_interrupt_count() -> u64 {
    INTERRUPTS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_completion_count() -> u64 {
    COMPLETIONS.load(Ordering::Acquire)
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
pub fn debug_in_flight_dma_buffers() -> u32 {
    IN_FLIGHT_DMA_BUFFERS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_desc_oracle_match() -> bool {
    LAST_DESC_ORACLE_MATCH.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_input_oracle_match() -> bool {
    LAST_INPUT_ORACLE_MATCH.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_status_oracle_match() -> bool {
    LAST_STATUS_ORACLE_MATCH.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_add_desc_oracle_match() -> bool {
    LAST_ADD_DESC_ORACLE_MATCH.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_add_input_oracle_match() -> bool {
    LAST_ADD_INPUT_ORACLE_MATCH.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_add_output_oracle_match() -> bool {
    LAST_ADD_OUTPUT_ORACLE_MATCH.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_last_add_status_oracle_match() -> bool {
    LAST_ADD_STATUS_ORACLE_MATCH.load(Ordering::Acquire)
}

struct VirtioAccelDriver {
    inner: IrqSafeMutex<VirtioAccelInner>,
}

impl AccelDriver for VirtioAccelDriver {
    fn descriptor(&self) -> AccelDescriptor {
        AccelDescriptor {
            device: DEVICE_ID,
            driver: "virtio_accel",
            name: "virtio_accel0",
            memory: AccelMemoryKind::DmaBuffer,
            dispatch: AccelDispatchKind::Asynchronous,
        }
    }

    fn supported_ops(&self) -> &'static [OpKind] {
        SUPPORTED_OPS
    }

    fn supported_dtypes(&self) -> &'static [Dtype] {
        SUPPORTED_DTYPES
    }

    fn alloc(&self, size: usize, align: usize) -> AccelResult<DeviceStorage> {
        if size == 0 || align == 0 || align > PAGE_SIZE || !align.is_power_of_two() {
            return Err(AccelError::InvalidRequest);
        }
        let buffer = DmaBuffer::alloc_zeroed(size).map_err(|_| AccelError::OutOfMemory)?;
        Ok(DeviceStorage::dma(DEVICE_ID, buffer))
    }

    fn free(&self, storage: DeviceStorage) {
        drop(storage);
    }

    fn copy_to_device(&self, dst: &mut DeviceStorage, src: &[u8]) -> AccelResult<()> {
        if dst.device() != DEVICE_ID || dst.len() != src.len() {
            return Err(AccelError::InvalidRequest);
        }
        dst.as_bytes_mut().copy_from_slice(src);
        Ok(())
    }

    fn copy_from_device(&self, dst: &mut [u8], src: &DeviceStorage) -> AccelResult<()> {
        if src.device() != DEVICE_ID || src.len() != dst.len() {
            return Err(AccelError::InvalidRequest);
        }
        dst.copy_from_slice(src.as_bytes());
        Ok(())
    }

    fn dispatch(&self, batch: &mut OpBatch<'_, '_>) -> AccelResult<DispatchToken> {
        if !SUPPORTED_OPS.contains(&batch.op)
            || batch.inputs.is_empty()
            || batch.inputs.len() > MAX_INPUTS
        {
            return Err(AccelError::Unsupported);
        }

        let output_dtype = output_dtype(batch.op, batch.inputs)?;
        let output_len = batch
            .output_shape
            .byte_len(output_dtype)
            .map_err(AccelError::from)?;
        if output_len != batch.output.len() || output_len as u64 > MAX_TENSOR_BYTES {
            return Err(AccelError::InvalidRequest);
        }

        let desc_block_len = REQ_HEADER_LEN + TENSOR_DESC_LEN * (batch.inputs.len() + 1);
        if desc_block_len > MAX_DESC_BLOCK_LEN || batch.inputs.len() + 3 > VIRTQUEUE_SIZE as usize {
            return Err(AccelError::Unsupported);
        }
        let desc_block =
            DmaBuffer::alloc_zeroed(desc_block_len).map_err(|_| AccelError::OutOfMemory)?;
        let mut inputs = Vec::with_capacity(batch.inputs.len());
        for input in batch.inputs {
            if !SUPPORTED_DTYPES.contains(&input.dtype) || wire_dtype(input.dtype).is_none() {
                return Err(AccelError::Unsupported);
            }
            let expected_len = input
                .shape
                .byte_len(input.dtype)
                .map_err(AccelError::from)?;
            if expected_len != input.bytes.len() || expected_len as u64 > MAX_TENSOR_BYTES {
                return Err(AccelError::InvalidRequest);
            }
            let input_dma =
                DmaBuffer::alloc_zeroed(input.bytes.len()).map_err(|_| AccelError::OutOfMemory)?;
            unsafe {
                input_dma.as_mut_slice().copy_from_slice(input.bytes);
            }
            inputs.push(input_dma);
        }
        let output_dma =
            DmaBuffer::alloc_zeroed(output_len).map_err(|_| AccelError::OutOfMemory)?;
        let status_dma =
            DmaBuffer::alloc_zeroed(STATUS_LEN).map_err(|_| AccelError::OutOfMemory)?;

        unsafe {
            status_dma.as_mut_slice().fill(0xff);
        }
        write_request_block(
            &desc_block,
            batch.op,
            batch.inputs,
            batch.output_shape,
            output_dtype,
        )?;
        record_request_oracles(batch.op, batch.inputs, &desc_block);

        let token = super::next_dispatch_token(DEVICE_ID);
        let pending = PendingRequest {
            token,
            op: batch.op,
            output_ptr: batch.output.as_mut_ptr() as usize,
            output_len,
            desc_block_len,
            desc_block,
            inputs,
            output: output_dma,
            status: status_dma,
        };

        let mut inner = self.inner.lock();
        if inner.in_flight.is_some() {
            return Err(AccelError::Unsupported);
        }
        inner.submit_pending(pending);
        Ok(token)
    }

    fn poll(&self, token: DispatchToken) -> DispatchStatus {
        if token.device != DEVICE_ID {
            return DispatchStatus::Failed(AccelError::InvalidRequest);
        }
        let inner = self.inner.lock();
        if let Some(completion) = inner.last_completion
            && completion.token == token
        {
            return completion.status;
        }
        if inner
            .in_flight
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            return DispatchStatus::Pending;
        }
        DispatchStatus::Failed(AccelError::InvalidRequest)
    }

    fn wait(&self, token: DispatchToken, timeout: Option<Duration>) -> AccelResult<()> {
        let start_tick = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
        let max_ticks = timeout.map(|duration| {
            let millis = duration.as_millis();
            let ticks = millis.div_ceil(10).max(1);
            ticks.min(u64::MAX as u128) as u64
        });
        loop {
            match self.poll(token) {
                DispatchStatus::Complete => return Ok(()),
                DispatchStatus::Failed(err) => return Err(err),
                DispatchStatus::Pending => {
                    if let Some(max_ticks) = max_ticks {
                        let now = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
                        if now.saturating_sub(start_tick) > max_ticks {
                            return Err(AccelError::InvalidRequest);
                        }
                    }
                    sched::yield_now();
                }
            }
        }
    }

    fn preemptible(&self) -> bool {
        false
    }

    fn preempt(&self, _token: DispatchToken) -> AccelResult<()> {
        Err(AccelError::Unsupported)
    }
}

struct VirtioAccelInner {
    common: VirtAddr,
    notify: VirtAddr,
    isr: VirtAddr,
    notify_multiplier: u32,
    queue: QueueMemory,
    next_avail_idx: u16,
    last_used_idx: u16,
    in_flight: Option<PendingRequest>,
    last_completion: Option<CompletionRecord>,
    bdf: u32,
}

impl VirtioAccelInner {
    fn submit_pending(&mut self, pending: PendingRequest) {
        let head = 0u16;
        let input_count = pending.inputs.len();
        let output_desc = input_count + 1;
        let status_desc = input_count + 2;
        write_descriptor(
            &self.queue.desc,
            0,
            pending.desc_block.physical_addr().as_u64(),
            pending.desc_block_len as u32,
            VIRTQ_DESC_F_NEXT,
            1,
        );
        for (idx, input) in pending.inputs.iter().enumerate() {
            let desc = idx + 1;
            write_descriptor(
                &self.queue.desc,
                desc as u16,
                input.physical_addr().as_u64(),
                input.len() as u32,
                VIRTQ_DESC_F_NEXT,
                (desc + 1) as u16,
            );
        }
        write_descriptor(
            &self.queue.desc,
            output_desc as u16,
            pending.output.physical_addr().as_u64(),
            pending.output_len as u32,
            VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE,
            status_desc as u16,
        );
        write_descriptor(
            &self.queue.desc,
            status_desc as u16,
            pending.status.physical_addr().as_u64(),
            STATUS_LEN as u32,
            VIRTQ_DESC_F_WRITE,
            0,
        );

        let avail_slot = self.next_avail_idx % self.queue.size;
        write_avail_ring(&self.queue.avail, avail_slot, head);
        descriptor_publish_barrier();
        self.next_avail_idx = self.next_avail_idx.wrapping_add(1);
        write_avail_idx(&self.queue.avail, self.next_avail_idx);
        notification_suppression_barrier();
        IN_FLIGHT_DMA_BUFFERS.store((input_count + 3) as u32, Ordering::Release);
        self.in_flight = Some(pending);
        notify_queue(self.notify, self.queue.notify_off, self.notify_multiplier);
    }

    fn poll_used(&mut self, from_irq: bool) {
        let used_idx = read_used_idx(&self.queue.used);
        if used_idx == self.last_used_idx {
            return;
        }
        used_ring_acquire_barrier();

        let used_slot = self.last_used_idx % self.queue.size;
        let descriptor_id = read_used_elem_id(&self.queue.used, used_slot);
        let _used_len = read_used_elem_len(&self.queue.used, used_slot);
        self.last_used_idx = self.last_used_idx.wrapping_add(1);

        let Some(pending) = self.in_flight.take() else {
            return;
        };
        let mut status = [0u8; STATUS_LEN];
        let output = unsafe { pending.output.as_slice() };
        unsafe {
            status.copy_from_slice(pending.status.as_slice());
        }
        let status_code = read_le_u32(&status, 0);
        let output_len = read_le_u64(&status, 8);
        let ok = descriptor_id == 0
            && status_code == STATUS_OK
            && output_len == pending.output_len as u64
            && output.len() == pending.output_len;
        record_completion_oracles(pending.op, output, &status);

        let dispatch_status = if ok {
            unsafe {
                let dst = core::slice::from_raw_parts_mut(
                    pending.output_ptr as *mut u8,
                    pending.output_len,
                );
                dst.copy_from_slice(output);
            }
            DispatchStatus::Complete
        } else {
            DispatchStatus::Failed(status_to_error(status_code))
        };

        self.last_completion = Some(CompletionRecord {
            token: pending.token,
            status: dispatch_status,
        });
        IN_FLIGHT_DMA_BUFFERS.store(0, Ordering::Release);
        COMPLETIONS.fetch_add(1, Ordering::AcqRel);
        if from_irq {
            IRQ_COMPLETIONS.fetch_add(1, Ordering::AcqRel);
        }
        let _isr = unsafe { mmio_read8(self.isr, 0) };
        serial_println!(
            "[ kernel ] virtio-accel: completion bdf=0x{:06x} token={} status={} output_len={}",
            self.bdf,
            pending.token.sequence,
            status_code,
            output_len
        );
    }
}

struct PendingRequest {
    token: DispatchToken,
    op: OpKind,
    output_ptr: usize,
    output_len: usize,
    desc_block_len: usize,
    desc_block: Arc<DmaBuffer>,
    inputs: Vec<Arc<DmaBuffer>>,
    output: Arc<DmaBuffer>,
    status: Arc<DmaBuffer>,
}

#[derive(Copy, Clone)]
struct CompletionRecord {
    token: DispatchToken,
    status: DispatchStatus,
}

struct QueueMemory {
    size: u16,
    notify_off: u16,
    desc: Arc<DmaBuffer>,
    avail: Arc<DmaBuffer>,
    used: Arc<DmaBuffer>,
}

#[derive(Copy, Clone)]
struct VirtioAccelConfig {
    supported_ops: u32,
    supported_dtypes: u32,
    max_inputs: u32,
    queue_size: u32,
    max_tensor_bytes: u64,
}

impl VirtioAccelConfig {
    fn supports_required_ops_f32_i32(self) -> bool {
        let f32 = 1u32 << WIRE_DTYPE_F32;
        let i32 = 1u32 << WIRE_DTYPE_I32;
        self.supported_ops & supported_ops_mask() == supported_ops_mask()
            && self.supported_dtypes & f32 != 0
            && self.supported_dtypes & i32 != 0
            && self.queue_size >= VIRTQUEUE_SIZE as u32
            && self.max_tensor_bytes >= MAX_TENSOR_BYTES
    }
}

fn poll_all_completions(from_irq: bool) {
    let devices = DEVICES.lock();
    for device in devices.iter() {
        device.inner.lock().poll_used(from_irq);
    }
}

fn setup_queue(common: VirtAddr, _notify: Region, msix: bool) -> Result<QueueMemory, ProbeError> {
    unsafe {
        mmio_write16(common, COMMON_QUEUE_SELECT, QUEUE_INDEX);
    }
    let max_size = unsafe { mmio_read16(common, COMMON_QUEUE_SIZE) };
    let queue_size = cmp::min(VIRTQUEUE_SIZE, max_size);
    if queue_size < 4 {
        return Err(ProbeError::QueueUnavailable);
    }
    let notify_off = unsafe { mmio_read16(common, COMMON_QUEUE_NOTIFY_OFF) };
    let desc =
        DmaBuffer::alloc_zeroed(desc_table_bytes(queue_size)).map_err(|_| ProbeError::Dma)?;
    let avail =
        DmaBuffer::alloc_zeroed(avail_ring_bytes(queue_size)).map_err(|_| ProbeError::Dma)?;
    let used = DmaBuffer::alloc_zeroed(used_ring_bytes(queue_size)).map_err(|_| ProbeError::Dma)?;

    unsafe {
        mmio_write16(common, COMMON_QUEUE_SIZE, queue_size);
        if msix {
            mmio_write16(common, COMMON_CONFIG_MSIX_VECTOR, 0);
            mmio_write16(common, COMMON_QUEUE_MSIX_VECTOR, 0);
        }
        mmio_write64(common, COMMON_QUEUE_DESC, desc.physical_addr().as_u64());
        mmio_write64(common, COMMON_QUEUE_DRIVER, avail.physical_addr().as_u64());
        mmio_write64(common, COMMON_QUEUE_DEVICE, used.physical_addr().as_u64());
        mmio_write16(common, COMMON_QUEUE_ENABLE, 1);
    }
    Ok(QueueMemory {
        size: queue_size,
        notify_off,
        desc,
        avail,
        used,
    })
}

fn reset_probe_state(dev: PciDevice) {
    SEEN.store(true, Ordering::Release);
    PROBE_OK.store(false, Ordering::Release);
    COMMON_CONFIG.store(false, Ordering::Release);
    NOTIFY_CONFIG.store(false, Ordering::Release);
    ISR_CONFIG.store(false, Ordering::Release);
    DEVICE_CONFIG.store(false, Ordering::Release);
    MSIX_CONFIGURED.store(false, Ordering::Release);
    MSI_CONFIGURED.store(false, Ordering::Release);
    QUEUE_READY.store(false, Ordering::Release);
    OFFERED_FEATURES.store(0, Ordering::Release);
    DRIVER_FEATURES.store(0, Ordering::Release);
    CONFIG_SUPPORTED_OPS_SEEN.store(0, Ordering::Release);
    CONFIG_SUPPORTED_DTYPES_SEEN.store(0, Ordering::Release);
    CONFIG_MAX_INPUTS_SEEN.store(0, Ordering::Release);
    CONFIG_QUEUE_SIZE_SEEN.store(0, Ordering::Release);
    CONFIG_MAX_TENSOR_BYTES_SEEN.store(0, Ordering::Release);
    LAST_BDF.store(pack_bdf(dev), Ordering::Release);
    LAST_DESC_ORACLE_MATCH.store(false, Ordering::Release);
    LAST_INPUT_ORACLE_MATCH.store(false, Ordering::Release);
    LAST_STATUS_ORACLE_MATCH.store(false, Ordering::Release);
    LAST_ADD_DESC_ORACLE_MATCH.store(false, Ordering::Release);
    LAST_ADD_INPUT_ORACLE_MATCH.store(false, Ordering::Release);
    LAST_ADD_OUTPUT_ORACLE_MATCH.store(false, Ordering::Release);
    LAST_ADD_STATUS_ORACLE_MATCH.store(false, Ordering::Release);
}

fn read_config(device: VirtAddr) -> VirtioAccelConfig {
    unsafe {
        VirtioAccelConfig {
            supported_ops: mmio_read32(device, CONFIG_SUPPORTED_OPS),
            supported_dtypes: mmio_read32(device, CONFIG_SUPPORTED_DTYPES),
            max_inputs: mmio_read32(device, CONFIG_MAX_INPUTS),
            queue_size: mmio_read32(device, CONFIG_QUEUE_SIZE),
            max_tensor_bytes: mmio_read64(device, CONFIG_MAX_TENSOR_BYTES),
        }
    }
}

fn store_config_snapshot(config: VirtioAccelConfig) {
    CONFIG_SUPPORTED_OPS_SEEN.store(config.supported_ops, Ordering::Release);
    CONFIG_SUPPORTED_DTYPES_SEEN.store(config.supported_dtypes, Ordering::Release);
    CONFIG_MAX_INPUTS_SEEN.store(config.max_inputs, Ordering::Release);
    CONFIG_QUEUE_SIZE_SEEN.store(config.queue_size, Ordering::Release);
    CONFIG_MAX_TENSOR_BYTES_SEEN.store(config.max_tensor_bytes, Ordering::Release);
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

fn write_request_block(
    buffer: &DmaBuffer,
    op: OpKind,
    inputs: &[crate::ai::ops::TensorView<'_>],
    output_shape: &Shape,
    output_dtype: Dtype,
) -> AccelResult<()> {
    if inputs.is_empty() || inputs.len() > MAX_INPUTS {
        return Err(AccelError::InvalidRequest);
    }
    let bytes = unsafe { buffer.as_mut_slice() };
    bytes.fill(0);
    write_le_u32(bytes, 0, op as u32);
    write_le_u32(bytes, 4, inputs.len() as u32);
    for (idx, input) in inputs.iter().enumerate() {
        let dtype = wire_dtype(input.dtype).ok_or(AccelError::Unsupported)?;
        write_tensor_desc(
            bytes,
            REQ_HEADER_LEN + idx * TENSOR_DESC_LEN,
            dtype,
            input.shape,
            input.bytes.len(),
        )?;
    }
    let output_byte_len = output_shape
        .byte_len(output_dtype)
        .map_err(AccelError::from)?;
    let output_wire_dtype = wire_dtype(output_dtype).ok_or(AccelError::Unsupported)?;
    write_tensor_desc(
        bytes,
        REQ_HEADER_LEN + inputs.len() * TENSOR_DESC_LEN,
        output_wire_dtype,
        output_shape,
        output_byte_len,
    )?;
    Ok(())
}

fn write_tensor_desc(
    bytes: &mut [u8],
    base: usize,
    dtype: u32,
    shape: &Shape,
    byte_len: usize,
) -> AccelResult<()> {
    write_le_u32(bytes, base, dtype);
    write_le_u32(bytes, base + 4, shape.rank() as u32);
    let dims = shape.dims();
    for (idx, dim) in dims.iter().enumerate() {
        write_le_u64(bytes, base + 8 + idx * 8, *dim);
    }
    write_le_u64(bytes, base + 72, byte_len as u64);
    Ok(())
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
    block::virtio_barrier::publish_descriptors();
}

fn notification_suppression_barrier() {
    NOTIFICATION_SUPPRESSION_BARRIERS.fetch_add(1, Ordering::AcqRel);
    block::virtio_barrier::before_notification_check();
}

fn used_ring_acquire_barrier() {
    fence(Ordering::Acquire);
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

fn supported_ops_mask() -> u32 {
    let mut mask = 0u32;
    for op in SUPPORTED_OPS {
        mask |= 1u32 << (*op as u32);
    }
    mask
}

fn output_dtype(op: OpKind, inputs: &[crate::ai::ops::TensorView<'_>]) -> AccelResult<Dtype> {
    match op {
        OpKind::Identity
        | OpKind::Reshape
        | OpKind::Transpose
        | OpKind::Relu
        | OpKind::Gelu
        | OpKind::Softmax => {
            if inputs.len() == 1 {
                Ok(inputs[0].dtype)
            } else {
                Err(AccelError::Unsupported)
            }
        }
        OpKind::Add | OpKind::Mul | OpKind::Matmul | OpKind::Embedding => {
            if inputs.len() == 2 {
                Ok(Dtype::F32)
            } else {
                Err(AccelError::Unsupported)
            }
        }
        OpKind::LayerNorm => {
            if inputs.len() == 1 || inputs.len() == 3 {
                Ok(inputs[0].dtype)
            } else {
                Err(AccelError::Unsupported)
            }
        }
        OpKind::RmsNorm => {
            if inputs.len() == 1 || inputs.len() == 2 {
                Ok(inputs[0].dtype)
            } else {
                Err(AccelError::Unsupported)
            }
        }
        OpKind::Silu | OpKind::Swiglu | OpKind::Rope | OpKind::GqaAttention => {
            Err(AccelError::Unsupported)
        }
    }
}

fn wire_dtype(dtype: Dtype) -> Option<u32> {
    match dtype {
        Dtype::F32 => Some(WIRE_DTYPE_F32),
        Dtype::I32 => Some(WIRE_DTYPE_I32),
        _ => None,
    }
}

fn record_request_oracles(
    op: OpKind,
    inputs: &[crate::ai::ops::TensorView<'_>],
    desc_block: &DmaBuffer,
) {
    let desc = unsafe { desc_block.as_slice() };
    LAST_DESC_ORACLE_MATCH.store(
        op == OpKind::Identity && desc == IDENTITY_DESC_BLOCK_ORACLE,
        Ordering::Release,
    );
    LAST_INPUT_ORACLE_MATCH.store(
        op == OpKind::Identity && inputs.len() == 1 && inputs[0].bytes == IDENTITY_INPUT_BYTES,
        Ordering::Release,
    );
    LAST_STATUS_ORACLE_MATCH.store(false, Ordering::Release);

    LAST_ADD_DESC_ORACLE_MATCH.store(
        op == OpKind::Add && desc == ADD_DESC_BLOCK_ORACLE,
        Ordering::Release,
    );
    LAST_ADD_INPUT_ORACLE_MATCH.store(
        op == OpKind::Add
            && inputs.len() == 2
            && inputs[0].bytes == ADD_LHS_BYTES
            && inputs[1].bytes == ADD_RHS_BYTES,
        Ordering::Release,
    );
    LAST_ADD_OUTPUT_ORACLE_MATCH.store(false, Ordering::Release);
    LAST_ADD_STATUS_ORACLE_MATCH.store(false, Ordering::Release);
}

fn record_completion_oracles(op: OpKind, output: &[u8], status: &[u8; STATUS_LEN]) {
    LAST_STATUS_ORACLE_MATCH.store(
        op == OpKind::Identity && *status == IDENTITY_STATUS_ORACLE,
        Ordering::Release,
    );
    LAST_ADD_OUTPUT_ORACLE_MATCH.store(
        op == OpKind::Add && output == ADD_OUTPUT_ORACLE,
        Ordering::Release,
    );
    LAST_ADD_STATUS_ORACLE_MATCH.store(
        op == OpKind::Add && *status == ADD_STATUS_ORACLE,
        Ordering::Release,
    );
}

fn status_to_error(status: u32) -> AccelError {
    match status {
        STATUS_UNSUPPORTED_OP | STATUS_UNSUPPORTED_DTYPE => AccelError::Unsupported,
        STATUS_SHAPE_MISMATCH => AccelError::Op(crate::ai::ops::OpError::Shape),
        STATUS_BAD_REQUEST | STATUS_INTERNAL => AccelError::InvalidRequest,
        _ => AccelError::InvalidRequest,
    }
}

fn pack_bdf(dev: PciDevice) -> u32 {
    ((dev.bus as u32) << 16) | ((dev.device as u32) << 8) | dev.function as u32
}

fn write_le_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_le_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .unwrap_or([STATUS_BAD_REQUEST as u8; 4]),
    )
}

fn read_le_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap_or([0xff; 8]))
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
