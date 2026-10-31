// SPDX-License-Identifier: MPL-2.0
//! Modern PCI virtio-net driver for Phase 4 BUILD-1a.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering, fence};

use spin::Lazy;
use x86_64::VirtAddr;

use crate::drivers::block::DmaBuffer;
use crate::drivers::net;
use crate::drivers::pci::{self, Device, VirtioPciCap};
use crate::net::{LinkState, NetDevice, NetDeviceIdentity, NetError, NetResult, RxFrame, TxFrame};
use crate::serial_println;
use crate::sync::{IrqSafeMutex, SwitchMutex};

pub const VIRTIO_VENDOR_ID: u16 = 0x1af4;
pub const VIRTIO_NET_LEGACY_DEVICE_ID: u16 = 0x1000;
pub const VIRTIO_NET_MODERN_DEVICE_ID: u16 = 0x1041;

const VIRTIO_NET_F_MAC: u64 = 1 << 5;
const VIRTIO_NET_F_STATUS: u64 = 1 << 16;
const VIRTIO_F_VERSION_1: u64 = 1 << 32;

const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;
const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;

const COMMON_DEVICE_FEATURE_SELECT: usize = 0x00;
const COMMON_DEVICE_FEATURE: usize = 0x04;
const COMMON_DRIVER_FEATURE_SELECT: usize = 0x08;
const COMMON_DRIVER_FEATURE: usize = 0x0c;
const COMMON_DEVICE_STATUS: usize = 0x14;
const COMMON_QUEUE_SELECT: usize = 0x16;
const COMMON_QUEUE_SIZE: usize = 0x18;
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

const QUEUE_SIZE: u16 = 64;
const RX_QUEUE_INDEX: u16 = 0;
const TX_QUEUE_INDEX: u16 = 1;
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

const VIRTIO_NET_HDR_LEN: usize = 12;
const ETHERNET_MTU: usize = 1500;
const ETHERNET_MAX_FRAME: usize = 1514;
const RX_BUFFER_LEN: usize = 2048;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ProbeError {
    MissingCommonConfig,
    MissingNotifyConfig,
    MissingIsrConfig,
    MissingDeviceConfig,
    RequiredFeatureNotOffered,
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
    pub offered_features: u64,
    pub driver_features: u64,
    pub mac_negotiated: bool,
    pub status_negotiated: bool,
    pub rx_queue_ready: bool,
    pub tx_queue_ready: bool,
    pub link_up: bool,
    pub last_bdf: u32,
}

static SEEN: AtomicBool = AtomicBool::new(false);
static PROBE_OK: AtomicBool = AtomicBool::new(false);
static COMMON_CONFIG: AtomicBool = AtomicBool::new(false);
static NOTIFY_CONFIG: AtomicBool = AtomicBool::new(false);
static ISR_CONFIG: AtomicBool = AtomicBool::new(false);
static DEVICE_CONFIG: AtomicBool = AtomicBool::new(false);
static OFFERED_FEATURES: AtomicU64 = AtomicU64::new(0);
static DRIVER_FEATURES: AtomicU64 = AtomicU64::new(0);
static RX_QUEUE_READY: AtomicBool = AtomicBool::new(false);
static TX_QUEUE_READY: AtomicBool = AtomicBool::new(false);
static LINK_UP: AtomicBool = AtomicBool::new(false);
static LAST_BDF: AtomicU32 = AtomicU32::new(0);
static TX_SUBMISSIONS: AtomicU64 = AtomicU64::new(0);
static RX_COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static DESCRIPTOR_PUBLISH_BARRIERS: AtomicU64 = AtomicU64::new(0);
static NOTIFICATION_SUPPRESSION_BARRIERS: AtomicU64 = AtomicU64::new(0);
static USED_RING_ACQUIRE_BARRIERS: AtomicU64 = AtomicU64::new(0);

static PRIMARY_DEVICE: SwitchMutex<Option<Arc<VirtioNetDevice>>> = SwitchMutex::new(None);
static VIRTIO_NET_DEVICES: Lazy<IrqSafeMutex<Vec<Arc<VirtioNetDevice>>>> =
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
            "[ kernel ] virtio-net: cap bdf={:02x}:{:02x}.{} type={} bar={} offset=0x{:x} length=0x{:x}",
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
    OFFERED_FEATURES.store(offered, Ordering::Release);
    let required = VIRTIO_F_VERSION_1 | VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS;
    if offered & VIRTIO_F_VERSION_1 == 0 {
        fail_device(common.virt);
        return Err(ProbeError::ModernFeatureNotOffered);
    }
    if offered & (VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS)
        != (VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS)
    {
        fail_device(common.virt);
        return Err(ProbeError::RequiredFeatureNotOffered);
    }

    add_status(common.virt, STATUS_ACKNOWLEDGE);
    add_status(common.virt, STATUS_DRIVER);
    write_driver_features(common.virt, required);
    DRIVER_FEATURES.store(required, Ordering::Release);
    add_status(common.virt, STATUS_FEATURES_OK);
    if unsafe { mmio_read8(common.virt, COMMON_DEVICE_STATUS) } & STATUS_FEATURES_OK == 0 {
        fail_device(common.virt);
        return Err(ProbeError::FeatureNegotiationRejected);
    }

    let rx_queue = setup_queue(common.virt, notify, RX_QUEUE_INDEX)?;
    let tx_queue = setup_queue(common.virt, notify, TX_QUEUE_INDEX)?;
    let mac = read_mac(device_config.virt);
    let link_up = unsafe { mmio_read16(device_config.virt, 6) } & 1 != 0;
    let bdf = pack_bdf(dev);
    let mut inner = VirtioNetInner {
        common: common.virt,
        notify: notify.virt,
        isr: isr.virt,
        notify_multiplier: notify.cap.notify_off_multiplier,
        rx_queue,
        tx_queue,
        rx_last_used_idx: 0,
        tx_last_used_idx: 0,
        rx_buffers: Vec::new(),
        tx_free: Vec::new(),
        tx_in_flight: Vec::new(),
        bdf,
    };
    inner.initialize_rx_buffers()?;
    inner.initialize_tx_pool()?;

    let device = Arc::new(VirtioNetDevice {
        identity: NetDeviceIdentity {
            driver: "virtio-net",
            name: "virtio-net0",
            pci_bdf: Some(dev.bdf()),
        },
        mac,
        link_up,
        inner: SwitchMutex::new(inner),
    });

    add_status(common.virt, STATUS_DRIVER_OK);
    RX_QUEUE_READY.store(true, Ordering::Release);
    TX_QUEUE_READY.store(true, Ordering::Release);
    LINK_UP.store(link_up, Ordering::Release);
    PROBE_OK.store(true, Ordering::Release);

    {
        let mut primary = PRIMARY_DEVICE.lock();
        if primary.is_none() {
            *primary = Some(device.clone());
        }
    }
    VIRTIO_NET_DEVICES.lock().push(device.clone());
    let trait_device: Arc<dyn NetDevice> = device.clone();
    net::register_device(trait_device);

    serial_println!(
        "[ kernel ] virtio-net: feature-policy bdf={:02x}:{:02x}.{} offered=0x{:016x} driver=0x{:016x} F_MAC=true F_STATUS=true queue_size={} mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} link={}",
        dev.bus,
        dev.device,
        dev.function,
        offered,
        required,
        QUEUE_SIZE,
        mac[0],
        mac[1],
        mac[2],
        mac[3],
        mac[4],
        mac[5],
        if link_up { "up" } else { "down" }
    );

    Ok(())
}

pub fn last_probe_snapshot() -> ProbeSnapshot {
    let features = DRIVER_FEATURES.load(Ordering::Acquire);
    ProbeSnapshot {
        seen: SEEN.load(Ordering::Acquire),
        probe_ok: PROBE_OK.load(Ordering::Acquire),
        common_config: COMMON_CONFIG.load(Ordering::Acquire),
        notify_config: NOTIFY_CONFIG.load(Ordering::Acquire),
        isr_config: ISR_CONFIG.load(Ordering::Acquire),
        device_config: DEVICE_CONFIG.load(Ordering::Acquire),
        offered_features: OFFERED_FEATURES.load(Ordering::Acquire),
        driver_features: features,
        mac_negotiated: features & VIRTIO_NET_F_MAC != 0,
        status_negotiated: features & VIRTIO_NET_F_STATUS != 0,
        rx_queue_ready: RX_QUEUE_READY.load(Ordering::Acquire),
        tx_queue_ready: TX_QUEUE_READY.load(Ordering::Acquire),
        link_up: LINK_UP.load(Ordering::Acquire),
        last_bdf: LAST_BDF.load(Ordering::Acquire),
    }
}

pub fn handle_interrupt() {
    service_all();
}

/// Service every device. Each device's lock (a plain spin lock that
/// `net-rx` and the syscalls take IF-off) is taken under the table's
/// `IrqSafeMutex` guard, so IF-off from any caller, the kernel's IF-on
/// post-reap call included (OT-96 F1, Chad #173; `wire_rx_demux` checks it).
pub fn service_all() {
    let devices = VIRTIO_NET_DEVICES.lock();
    for device in devices.iter() {
        device.service();
    }
}

#[cfg(feature = "neo-test")]
pub fn debug_tx_submissions() -> u64 {
    TX_SUBMISSIONS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_rx_completions() -> u64 {
    RX_COMPLETIONS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_descriptor_publish_barriers() -> u64 {
    DESCRIPTOR_PUBLISH_BARRIERS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test")]
pub fn debug_notification_suppression_barriers() -> u64 {
    NOTIFICATION_SUPPRESSION_BARRIERS.load(Ordering::Acquire)
}

struct VirtioNetDevice {
    identity: NetDeviceIdentity,
    mac: [u8; 6],
    link_up: bool,
    inner: SwitchMutex<VirtioNetInner>,
}

impl NetDevice for VirtioNetDevice {
    fn identity(&self) -> &NetDeviceIdentity {
        &self.identity
    }

    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }

    fn link_state(&self) -> LinkState {
        if self.link_up {
            LinkState::Up
        } else {
            LinkState::Down
        }
    }

    fn mtu(&self) -> usize {
        ETHERNET_MTU
    }

    fn tx_reserve(&self) -> Option<TxFrame> {
        #[cfg(all(feature = "wire-rx", feature = "neo-test"))]
        crate::net::rx::assert_if_off("virtio-net inner");
        let mut inner = self.inner.lock();
        inner.service_tx_completions();
        let buffer = inner.tx_free.pop()?;
        Some(TxFrame { buffer })
    }

    fn tx_submit(&self, frame: TxFrame, len: usize) -> NetResult<()> {
        if len > ETHERNET_MAX_FRAME {
            return Err(NetError::InvalidFrame);
        }
        #[cfg(all(feature = "wire-rx", feature = "neo-test"))]
        crate::net::rx::assert_if_off("virtio-net inner");
        let mut inner = self.inner.lock();
        inner.submit_tx(frame.buffer, len)
    }

    fn rx_take(&self) -> Option<RxFrame> {
        #[cfg(all(feature = "wire-rx", feature = "neo-test"))]
        crate::net::rx::assert_if_off("virtio-net inner");
        let mut inner = self.inner.lock();
        inner.take_rx()
    }

    fn rx_recycle(&self, frame: RxFrame) {
        #[cfg(all(feature = "wire-rx", feature = "neo-test"))]
        crate::net::rx::assert_if_off("virtio-net inner");
        let mut inner = self.inner.lock();
        let _ = inner.repost_rx(frame.buffer);
    }

    fn service(&self) {
        #[cfg(all(feature = "wire-rx", feature = "neo-test"))]
        crate::net::rx::assert_if_off("virtio-net inner");
        let mut inner = self.inner.lock();
        inner.service_tx_completions();
        let _isr = unsafe { mmio_read8(inner.isr, 0) };
    }
}

struct VirtioNetInner {
    common: VirtAddr,
    notify: VirtAddr,
    isr: VirtAddr,
    notify_multiplier: u32,
    rx_queue: QueueMemory,
    tx_queue: QueueMemory,
    rx_last_used_idx: u16,
    tx_last_used_idx: u16,
    rx_buffers: Vec<Option<Arc<DmaBuffer>>>,
    tx_free: Vec<Arc<DmaBuffer>>,
    tx_in_flight: Vec<TxInFlight>,
    bdf: u32,
}

impl VirtioNetInner {
    fn initialize_rx_buffers(&mut self) -> Result<(), ProbeError> {
        for _ in 0..QUEUE_SIZE {
            self.rx_buffers.push(None);
        }
        for index in 0..QUEUE_SIZE {
            let buffer = DmaBuffer::alloc_zeroed(RX_BUFFER_LEN).map_err(|_| ProbeError::Dma)?;
            self.post_rx_at(index, buffer)?;
        }
        Ok(())
    }

    fn initialize_tx_pool(&mut self) -> Result<(), ProbeError> {
        for _ in 0..(QUEUE_SIZE / 2) {
            self.tx_free
                .push(DmaBuffer::alloc_zeroed(ETHERNET_MAX_FRAME).map_err(|_| ProbeError::Dma)?);
        }
        Ok(())
    }

    fn post_rx_at(&mut self, desc: u16, buffer: Arc<DmaBuffer>) -> Result<(), ProbeError> {
        write_descriptor(
            &self.rx_queue.desc,
            desc,
            buffer.physical_addr().as_u64(),
            RX_BUFFER_LEN as u32,
            VIRTQ_DESC_F_WRITE,
            0,
        );
        self.rx_buffers[desc as usize] = Some(buffer);
        self.publish_available(RX_QUEUE_INDEX, desc);
        Ok(())
    }

    fn repost_rx(&mut self, buffer: Arc<DmaBuffer>) -> NetResult<()> {
        let Some(desc) = self.rx_buffers.iter().position(Option::is_none) else {
            return Err(NetError::RingFull);
        };
        self.post_rx_at(desc as u16, buffer)
            .map_err(|_| NetError::Io)
    }

    fn take_rx(&mut self) -> Option<RxFrame> {
        let used_idx = read_used_idx(&self.rx_queue.used);
        if used_idx == self.rx_last_used_idx {
            return None;
        }
        used_ring_acquire_barrier();
        let used_slot = self.rx_last_used_idx % self.rx_queue.size;
        let desc = read_used_elem_id(&self.rx_queue.used, used_slot) as usize;
        let used_len = read_used_elem_len(&self.rx_queue.used, used_slot) as usize;
        self.rx_last_used_idx = self.rx_last_used_idx.wrapping_add(1);
        if desc >= self.rx_buffers.len() || used_len <= VIRTIO_NET_HDR_LEN {
            return None;
        }
        let buffer = self.rx_buffers[desc].take()?;
        let frame_len = used_len.saturating_sub(VIRTIO_NET_HDR_LEN);
        RX_COMPLETIONS.fetch_add(1, Ordering::AcqRel);
        Some(RxFrame {
            buffer,
            offset: VIRTIO_NET_HDR_LEN,
            len: cmp::min(frame_len, ETHERNET_MAX_FRAME),
        })
    }

    fn submit_tx(&mut self, data: Arc<DmaBuffer>, len: usize) -> NetResult<()> {
        self.service_tx_completions();
        let head = self.next_free_tx_head().ok_or(NetError::RingFull)?;
        let header = DmaBuffer::alloc_zeroed(VIRTIO_NET_HDR_LEN).map_err(|_| NetError::Io)?;
        write_descriptor(
            &self.tx_queue.desc,
            head,
            header.physical_addr().as_u64(),
            VIRTIO_NET_HDR_LEN as u32,
            VIRTQ_DESC_F_NEXT,
            head + 1,
        );
        write_descriptor(
            &self.tx_queue.desc,
            head + 1,
            data.physical_addr().as_u64(),
            len as u32,
            0,
            0,
        );
        self.tx_in_flight.push(TxInFlight { head, header, data });
        self.publish_available(TX_QUEUE_INDEX, head);
        TX_SUBMISSIONS.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn next_free_tx_head(&self) -> Option<u16> {
        let mut head = 0u16;
        while head + 1 < self.tx_queue.size {
            if self.tx_in_flight.iter().all(|entry| entry.head != head) {
                return Some(head);
            }
            head += 2;
        }
        None
    }

    fn service_tx_completions(&mut self) {
        // OT-96 F2 (Chad #173): at most one queue's worth of completions per
        // call. A device can have no more than that in flight, so a sane one
        // is unaffected; one that keeps advancing its used index can't keep
        // this IF-off loop running. The rest wait for the next call.
        #[cfg(feature = "wire-rx")]
        let mut budget = self.tx_queue.size;
        loop {
            #[cfg(feature = "wire-rx")]
            {
                if budget == 0 {
                    break;
                }
                budget -= 1;
            }
            let used_idx = read_used_idx(&self.tx_queue.used);
            if used_idx == self.tx_last_used_idx {
                break;
            }
            used_ring_acquire_barrier();
            let used_slot = self.tx_last_used_idx % self.tx_queue.size;
            let head = read_used_elem_id(&self.tx_queue.used, used_slot) as u16;
            self.tx_last_used_idx = self.tx_last_used_idx.wrapping_add(1);
            if let Some(pos) = self
                .tx_in_flight
                .iter()
                .position(|entry| entry.head == head)
            {
                let entry = self.tx_in_flight.swap_remove(pos);
                self.tx_free.push(entry.data);
                drop(entry.header);
            }
        }
    }

    fn publish_available(&mut self, queue_index: u16, head: u16) {
        let queue = if queue_index == RX_QUEUE_INDEX {
            &mut self.rx_queue
        } else {
            &mut self.tx_queue
        };
        let avail_slot = queue.next_avail_idx % queue.size;
        write_avail_ring(&queue.avail, avail_slot, head);
        descriptor_publish_barrier();
        queue.next_avail_idx = queue.next_avail_idx.wrapping_add(1);
        write_avail_idx(&queue.avail, queue.next_avail_idx);
        notification_suppression_barrier();
        notify_queue(
            self.notify,
            queue.notify_off,
            self.notify_multiplier,
            queue_index,
        );
    }
}

struct TxInFlight {
    head: u16,
    header: Arc<DmaBuffer>,
    data: Arc<DmaBuffer>,
}

struct QueueMemory {
    size: u16,
    notify_off: u16,
    desc: Arc<DmaBuffer>,
    avail: Arc<DmaBuffer>,
    used: Arc<DmaBuffer>,
    next_avail_idx: u16,
}

fn setup_queue(
    common: VirtAddr,
    notify: Region,
    queue_index: u16,
) -> Result<QueueMemory, ProbeError> {
    unsafe {
        mmio_write16(common, COMMON_QUEUE_SELECT, queue_index);
    }
    let max_size = unsafe { mmio_read16(common, COMMON_QUEUE_SIZE) };
    if max_size < QUEUE_SIZE {
        return Err(ProbeError::QueueUnavailable);
    }
    let notify_off = unsafe { mmio_read16(common, COMMON_QUEUE_NOTIFY_OFF) };
    let desc =
        DmaBuffer::alloc_zeroed(desc_table_bytes(QUEUE_SIZE)).map_err(|_| ProbeError::Dma)?;
    let avail =
        DmaBuffer::alloc_zeroed(avail_ring_bytes(QUEUE_SIZE)).map_err(|_| ProbeError::Dma)?;
    let used = DmaBuffer::alloc_zeroed(used_ring_bytes(QUEUE_SIZE)).map_err(|_| ProbeError::Dma)?;

    unsafe {
        mmio_write16(common, COMMON_QUEUE_SIZE, QUEUE_SIZE);
        mmio_write64(common, COMMON_QUEUE_DESC, desc.physical_addr().as_u64());
        mmio_write64(common, COMMON_QUEUE_DRIVER, avail.physical_addr().as_u64());
        mmio_write64(common, COMMON_QUEUE_DEVICE, used.physical_addr().as_u64());
        mmio_write16(common, COMMON_QUEUE_ENABLE, 1);
    }

    let _ = notify;
    Ok(QueueMemory {
        size: QUEUE_SIZE,
        notify_off,
        desc,
        avail,
        used,
        next_avail_idx: 0,
    })
}

fn reset_probe_state(dev: Device) {
    SEEN.store(true, Ordering::Release);
    PROBE_OK.store(false, Ordering::Release);
    COMMON_CONFIG.store(false, Ordering::Release);
    NOTIFY_CONFIG.store(false, Ordering::Release);
    ISR_CONFIG.store(false, Ordering::Release);
    DEVICE_CONFIG.store(false, Ordering::Release);
    OFFERED_FEATURES.store(0, Ordering::Release);
    DRIVER_FEATURES.store(0, Ordering::Release);
    RX_QUEUE_READY.store(false, Ordering::Release);
    TX_QUEUE_READY.store(false, Ordering::Release);
    LINK_UP.store(false, Ordering::Release);
    LAST_BDF.store(pack_bdf(dev), Ordering::Release);
}

fn read_mac(device_config: VirtAddr) -> [u8; 6] {
    let mut mac = [0u8; 6];
    for (idx, byte) in mac.iter_mut().enumerate() {
        *byte = unsafe { mmio_read8(device_config, idx) };
    }
    mac
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

fn notify_queue(notify: VirtAddr, queue_notify_off: u16, notify_multiplier: u32, queue: u16) {
    let offset = queue_notify_off as usize * notify_multiplier as usize;
    unsafe {
        write_volatile(
            (notify.as_u64() as usize + offset) as *mut u16,
            queue.to_le(),
        );
    }
}

fn descriptor_publish_barrier() {
    DESCRIPTOR_PUBLISH_BARRIERS.fetch_add(1, Ordering::AcqRel);
    crate::drivers::block::virtio_barrier::publish_descriptors();
}

fn notification_suppression_barrier() {
    NOTIFICATION_SUPPRESSION_BARRIERS.fetch_add(1, Ordering::AcqRel);
    crate::drivers::block::virtio_barrier::before_notification_check();
}

fn used_ring_acquire_barrier() {
    USED_RING_ACQUIRE_BARRIERS.fetch_add(1, Ordering::AcqRel);
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
