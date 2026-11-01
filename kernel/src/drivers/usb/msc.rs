// SPDX-License-Identifier: MPL-2.0
//! L3u-2: USB mass storage (Bulk-Only Transport + SCSI), read-only, as a
//! block device (`docs/THINK-2026-10-31-l3u-usb-mass-storage.md` §4).
//!
//! The protocol is `msc_logic` (pure, host-tested); this file is the binding
//! and the transport: a [`Pipe`] over the xHCI bulk pipes, and a
//! [`BlockDevice`] registered through the block registry, so the stick sits
//! behind `GuardedDevice` like every other disk (it carries no sealed marker,
//! so the guard refuses writes; this device refuses them first anyway).
//!
//! I-14: (a) commands run in the reading thread; the xHCI event path they
//! poll takes no governance lock. (b) every transfer wait is bounded
//! ([`WAIT_POLLS`]); a wedged stick fails its own reads, nothing else.
//! (c) every transfer goes through one 64 KiB bounce buffer and one wrapper
//! buffer allocated in the DMA arena and fenced (`dma_arena::fence`), under
//! the existing IOMMU containment; caller buffers are never given to the
//! device.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::drivers::block::{
    BlockDevice, BlockDeviceIdentity, BlockError, BlockGeometry, BlockResult, DmaBuffer,
};
use crate::drivers::dma_arena::{self, FencedDmaAddr};
use crate::sync::SwitchMutex;

use super::msc_logic::{self, Capacity10, Outcome, Pipe, ReadyAction, Xfer};
use super::usb_parse::UsbEndpoint;
use super::xhci::{self, ClaimedDevice, EndpointPipe};
use super::{UsbClassDriver, UsbDevice};

/// The bounce buffer: one READ(10) at most (`MAX_READ_BLOCKS` × 512).
const BOUNCE_LEN: usize = msc_logic::MAX_READ_BLOCKS as usize * 512;
/// Polls of the event ring per transfer before it counts as timed out
/// (U18). Each poll spins ~10 µs, so the bound is a few seconds.
const WAIT_POLLS: u32 = 400_000;
/// TEST UNIT READY attempts while the device reports UNIT ATTENTION (U9).
const READY_TRIES: u32 = 5;
/// Spin-up (Chad's #20 review): while the device says NOT READY, becoming
/// ready (04/01), TEST UNIT READY is asked again every [`SPIN_UP_PAUSE_POLLS`]
/// of ~10 µs (about 250 ms), at most this many times (about 10 s in all,
/// on top of the UNIT ATTENTION retries).
const SPIN_UP_WAITS: u32 = 40;
const SPIN_UP_PAUSE_POLLS: u32 = 25_000;

/// xHCI completion codes (xHCI §6.4.5).
const CC_SUCCESS: u32 = 1;
const CC_STALL: u32 = 6;
const CC_SHORT_PACKET: u32 = 13;

pub struct MscDriver;

pub fn init() {
    super::register_class_driver(Arc::new(MscDriver));
}

/// The bulk-only SCSI interface of a configuration: (configuration value,
/// interface, alt setting, bulk-in, bulk-out). UAS alternates are skipped
/// (U1).
fn find_bot(device: &UsbDevice) -> Option<(u8, u8, u8, UsbEndpoint, UsbEndpoint)> {
    for config in &device.configurations {
        for iface in &config.interfaces {
            if !msc_logic::is_bulk_only_scsi(iface.class, iface.subclass, iface.protocol) {
                continue;
            }
            let bulk_in = iface.endpoints.iter().find(|e| e.is_bulk() && e.is_in());
            let bulk_out = iface.endpoints.iter().find(|e| e.is_bulk() && !e.is_in());
            if let (Some(i), Some(o)) = (bulk_in, bulk_out) {
                return Some((config.value, iface.number, iface.alt_setting, *i, *o));
            }
        }
    }
    None
}

impl UsbClassDriver for MscDriver {
    fn name(&self) -> &'static str {
        "usb-storage"
    }

    fn probe(&self, device: &UsbDevice) -> bool {
        find_bot(device).is_some()
    }

    #[cfg(feature = "usb-hotplug")]
    fn detach(&self, device: &UsbDevice) {
        detach_disk(device.slot_id);
    }

    fn bind(&self, device: &Arc<UsbDevice>, claimed: ClaimedDevice) -> Result<(), ClaimedDevice> {
        match bind_inner(device, claimed) {
            Ok(()) => Ok(()),
            Err((claimed, reason)) => {
                crate::serial_println!(
                    "msc: bind failed vid={:04x} pid={:04x}: {}",
                    device.vendor_id,
                    device.product_id,
                    reason
                );
                Err(claimed)
            }
        }
    }
}

fn fence(buffer: &DmaBuffer, len: usize) -> Result<FencedDmaAddr, &'static str> {
    if len > buffer.len() {
        return Err("transfer longer than its buffer");
    }
    dma_arena::fence(buffer.physical_addr().as_u64(), len as u64)
        .map_err(|_| "buffer outside the DMA fence")
}

/// The device's transport: its claimed slot, both bulk pipes, and the two
/// fenced buffers every transfer uses.
struct Transport {
    claimed: ClaimedDevice,
    bulk_in: EndpointPipe,
    bulk_out: EndpointPipe,
    interface: u8,
    /// Wrappers (CBW out, CSW in).
    small: Arc<DmaBuffer>,
    /// Data stages.
    bounce: Arc<DmaBuffer>,
    tag: u32,
    lun: u8,
    /// Set when the device is unplugged (#30); shared with its `MscDisk`.
    gone: Arc<AtomicBool>,
}

/// Wait for the transfer at ring index `index` on `pipe`: `(code, residue)`,
/// or `None` on timeout, or as soon as `gone` is set (the stick was pulled:
/// #30, an in-flight read fails at once rather than after the timeout).
fn wait(pipe: &mut EndpointPipe, index: usize, gone: &AtomicBool) -> Option<(u32, u32)> {
    for _ in 0..WAIT_POLLS {
        if gone.load(Ordering::Acquire) {
            return None;
        }
        xhci::poll_events();
        for event in pipe.completions() {
            if pipe.index_of(event.parameter) == Some(index) {
                return Some((event.completion_code(), event.transfer_residue()));
            }
        }
        for _ in 0..20_000 {
            core::hint::spin_loop();
        }
    }
    None
}

fn classify(done: Option<(u32, u32)>, asked: usize) -> Xfer {
    match done {
        Some((CC_SUCCESS | CC_SHORT_PACKET, residue)) => {
            Xfer::Done(asked.saturating_sub(residue as usize))
        }
        Some((CC_STALL, _)) => Xfer::Stall,
        _ => Xfer::Error,
    }
}

impl Transport {
    /// Clear a halted endpoint on both sides: the host ring (Reset Endpoint
    /// + Set TR Dequeue) and the device (ClearFeature(ENDPOINT_HALT)).
    fn clear(&mut self, inbound: bool) -> bool {
        let pipe = if inbound {
            &mut self.bulk_in
        } else {
            &mut self.bulk_out
        };
        let host = self.claimed.recover_endpoint(pipe, true);
        let dci = pipe.dci;
        let device = self.claimed.clear_endpoint_halt(dci).is_ok();
        host.set_dequeue_ok && device
    }

    /// One command; a data-in stage of `data.len()` bytes (at most the
    /// bounce buffer) is copied into `data`.
    fn run(&mut self, cdb: &[u8], data: &mut [u8]) -> Outcome {
        if data.len() > BOUNCE_LEN {
            return Outcome::Reset {
                reason: msc_logic::ResetReason::Data,
                recovered: true,
            };
        }
        self.tag = self.tag.wrapping_add(1);
        let (tag, lun) = (self.tag, self.lun);
        msc_logic::command(&mut PipeView(self), tag, lun, cdb, data)
    }

    /// REQUEST SENSE after a failed command (U9).
    fn sense(&mut self) -> Option<msc_logic::Sense> {
        let mut buf = [0u8; msc_logic::SENSE_LEN as usize];
        match self.run(&msc_logic::cdb_request_sense(), &mut buf) {
            Outcome::Passed { len } => msc_logic::decode_sense(&buf[..len]),
            _ => None,
        }
    }
}

struct PipeView<'a>(&'a mut Transport);

impl Pipe for PipeView<'_> {
    fn bulk_out(&mut self, data: &[u8]) -> Xfer {
        let t = &mut *self.0;
        // SAFETY: no transfer is in flight on `small`; the device reads it
        // only after the doorbell below.
        unsafe { t.small.as_mut_slice()[..data.len()].copy_from_slice(data) };
        let Ok(addr) = fence(&t.small, data.len()) else {
            return Xfer::Error;
        };
        let index = t.bulk_out.submit(addr, data.len() as u32);
        classify(wait(&mut t.bulk_out, index, &t.gone), data.len())
    }

    fn bulk_in(&mut self, buf: &mut [u8]) -> Xfer {
        let t = &mut *self.0;
        let target = if buf.len() == msc_logic::CSW_LEN {
            &t.small
        } else {
            &t.bounce
        };
        let Ok(addr) = fence(target, buf.len()) else {
            return Xfer::Error;
        };
        let target = target.clone();
        let index = t.bulk_in.submit(addr, buf.len() as u32);
        let got = classify(wait(&mut t.bulk_in, index, &t.gone), buf.len());
        if let Xfer::Done(n) = got {
            // SAFETY: the transfer completed; nothing else uses the buffer.
            buf[..n].copy_from_slice(unsafe { &target.as_slice()[..n] });
        }
        got
    }

    fn clear_halt_in(&mut self) -> bool {
        if self.0.gone.load(Ordering::Acquire) {
            return false;
        }
        self.0.clear(true)
    }

    fn clear_halt_out(&mut self) -> bool {
        if self.0.gone.load(Ordering::Acquire) {
            return false;
        }
        self.0.clear(false)
    }

    fn mass_storage_reset(&mut self) -> bool {
        if self.0.gone.load(Ordering::Acquire) {
            return false;
        }
        let iface = u16::from(self.0.interface);
        // bmRequestType 0x21: host-to-device, class, interface.
        self.0
            .claimed
            .control_no_data(0x21, msc_logic::REQ_MASS_STORAGE_RESET, 0, iface)
            .is_ok()
    }
}

/// The registered block device: the transport behind one lock, held for
/// one block read.
struct MscDisk {
    identity: BlockDeviceIdentity,
    geometry: BlockGeometry,
    transport: SwitchMutex<Transport>,
    /// The stick was unplugged (#30): every read from now on, and the one in
    /// flight, fails `NoDevice`.
    gone: Arc<AtomicBool>,
}

impl BlockDevice for MscDisk {
    fn identity(&self) -> &BlockDeviceIdentity {
        &self.identity
    }

    fn geometry(&self) -> BlockGeometry {
        self.geometry
    }

    fn read_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        let block = u64::from(self.geometry.logical_block_size);
        let bytes: u64 = buffers.iter().map(|b| b.len() as u64).sum();
        if block == 0 || bytes == 0 || !bytes.is_multiple_of(block) {
            return Err(BlockError::InvalidRequest);
        }
        let count = bytes / block;
        if lba
            .checked_add(count)
            .is_none_or(|end| end > self.geometry.block_count)
        {
            return Err(BlockError::InvalidRequest);
        }
        if self.gone.load(Ordering::Acquire) {
            return Err(BlockError::NoDevice);
        }
        let mut t = self.transport.lock();
        let mut out = alloc::vec::Vec::with_capacity(bytes as usize);
        let mut done = 0;
        let mut chunk = alloc::vec![0u8; BOUNCE_LEN];
        // `next_read` caps a command at MAX_READ_BLOCKS; with blocks larger
        // than 512 bytes the bounce buffer caps it lower (U13, U14).
        let per_command = (BOUNCE_LEN as u64 / block).max(1);
        while let Some((at, blocks)) =
            msc_logic::next_read(lba, count.min(done + per_command), done)
        {
            let len = blocks as usize * block as usize;
            if self.gone.load(Ordering::Acquire) {
                return Err(BlockError::NoDevice);
            }
            match t.run(&msc_logic::cdb_read_10(at, blocks), &mut chunk[..len]) {
                Outcome::Passed { len: got } if got == len => out.extend_from_slice(&chunk[..len]),
                _ if self.gone.load(Ordering::Acquire) => return Err(BlockError::NoDevice),
                other => {
                    crate::serial_println!(
                        "msc: read failed lba={} blocks={} outcome={:?}{}",
                        at,
                        blocks,
                        other,
                        // U12: the reported last block failing is the
                        // capacity-off-by-one quirk's signature.
                        if u64::from(at) + u64::from(blocks) == self.geometry.block_count {
                            " (last-lba read failed)"
                        } else {
                            ""
                        }
                    );
                    return Err(BlockError::Io);
                }
            }
            done += u64::from(blocks);
        }
        drop(t);
        // Pulled after the last command: the data may be whole, but the
        // device is gone, so the read reports it rather than racing it.
        if self.gone.load(Ordering::Acquire) {
            return Err(BlockError::NoDevice);
        }
        let mut at = 0;
        for buffer in buffers {
            // SAFETY: the caller's buffers are not part of any device request
            // (the device only ever sees the bounce buffer).
            unsafe {
                buffer
                    .as_mut_slice()
                    .copy_from_slice(&out[at..at + buffer.len()])
            };
            at += buffer.len();
        }
        Ok(())
    }

    fn write_block(&self, _lba: u64, _buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        // L3u is read-only; the write guard would refuse an unmarked stick too.
        Err(BlockError::Unsupported)
    }
}

/// ASCII-only, trimmed, for the log line (INQUIRY's vendor/product).
fn ascii(bytes: &[u8]) -> String {
    let s: String = bytes
        .iter()
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '?'
            }
        })
        .collect();
    String::from(s.trim())
}

// clippy-bank: oversized ClaimedDevice errors are a CB-4 USB error-type refactor.
#[allow(clippy::result_large_err)]
fn bind_inner(
    device: &Arc<UsbDevice>,
    mut claimed: ClaimedDevice,
) -> Result<(), (ClaimedDevice, &'static str)> {
    let Some((config, interface, alt, ep_in, ep_out)) = find_bot(device) else {
        return Err((claimed, "no bulk-only SCSI interface"));
    };
    crate::serial_println!(
        "msc: bind vid={:04x} pid={:04x} speed={} config={} if={} alt={} proto=0x50 in=0x{:02x}/{} out=0x{:02x}/{}",
        device.vendor_id,
        device.product_id,
        device.speed.name(),
        config,
        interface,
        alt,
        ep_in.address,
        ep_in.max_packet,
        ep_out.address,
        ep_out.max_packet
    );
    // Buffers before endpoints: a failed allocation declines cleanly.
    let Ok(small) = DmaBuffer::alloc_in_arena(512) else {
        return Err((claimed, "wrapper buffer alloc (arena)"));
    };
    let Ok(bounce) = DmaBuffer::alloc_in_arena(BOUNCE_LEN) else {
        return Err((claimed, "bounce buffer alloc (arena)"));
    };
    let mut pipes = match claimed.configure_endpoints(&[ep_in, ep_out]) {
        Ok(pipes) => pipes,
        Err(reason) => return Err((claimed, reason)),
    };
    let bulk_out = pipes.pop().expect("two pipes requested");
    let bulk_in = pipes.pop().expect("two pipes requested");
    if let Err(reason) = claimed.control_no_data(0x00, 9, u16::from(config), 0) {
        return Err((claimed, reason));
    }
    // U1: select the BOT alternate when it is not the default one.
    if alt != 0
        && let Err(reason) = claimed.control_no_data(0x01, 11, u16::from(alt), u16::from(interface))
    {
        return Err((claimed, reason));
    }
    // U7: GET MAX LUN; a stall means one LUN.
    let mut max_lun = [0u8; 1];
    let max_lun = match claimed.control_read(
        0xa1,
        msc_logic::REQ_GET_MAX_LUN,
        0,
        u16::from(interface),
        1,
        &mut max_lun,
    ) {
        Ok(()) => max_lun[0],
        Err(_) => 0,
    };
    let mut t = Transport {
        claimed,
        bulk_in,
        bulk_out,
        interface,
        small,
        bounce,
        tag: 0,
        lun: 0,
        gone: Arc::new(AtomicBool::new(false)),
    };
    crate::serial_println!("msc: maxlun={} using lun=0", max_lun);

    let mut inquiry = [0u8; msc_logic::INQUIRY_LEN as usize];
    match t.run(&msc_logic::cdb_inquiry(), &mut inquiry) {
        Outcome::Passed { len } if len >= 36 => {
            crate::serial_println!(
                "msc: inquiry type=0x{:02x} removable={} vendor=\"{}\" product=\"{}\"",
                inquiry[0] & 0x1f,
                (inquiry[1] & 0x80 != 0) as u8,
                ascii(&inquiry[8..16]),
                ascii(&inquiry[16..32])
            );
        }
        other => {
            crate::serial_println!("msc: inquiry failed outcome={:?}", other);
            return Err((t.claimed, "inquiry"));
        }
    }

    // U9: TEST UNIT READY, through UNIT ATTENTION (asked again at once) and
    // spin-up (asked again after a pause, within a budget).
    let mut ready = false;
    let (mut attentions, mut waits) = (0u32, 0u32);
    while attentions < READY_TRIES && waits <= SPIN_UP_WAITS {
        match t.run(&msc_logic::cdb_test_unit_ready(), &mut []) {
            Outcome::Passed { .. } => {
                ready = true;
                break;
            }
            Outcome::Failed { .. } => match t.sense().map(msc_logic::ready_action) {
                Some(ReadyAction::Retry) => attentions += 1,
                Some(ReadyAction::Wait) => {
                    waits += 1;
                    for _ in 0..SPIN_UP_PAUSE_POLLS {
                        for _ in 0..20_000 {
                            core::hint::spin_loop();
                        }
                    }
                }
                Some(ReadyAction::NoMedium) => {
                    crate::serial_println!("msc: no medium");
                    return Err((t.claimed, "no medium"));
                }
                Some(ReadyAction::NeedsStart) => {
                    crate::serial_println!("msc: needs START UNIT (not sent: read-only allowlist)");
                    return Err((t.claimed, "needs start unit"));
                }
                _ => break,
            },
            Outcome::Reset { .. } => break,
        }
    }
    if waits > 0 {
        crate::serial_println!("msc: spin-up waits={} ready={}", waits, u8::from(ready));
    }
    if !ready {
        return Err((t.claimed, "not ready"));
    }

    let mut cap10 = [0u8; 8];
    let capacity = match t.run(&msc_logic::cdb_read_capacity_10(), &mut cap10) {
        Outcome::Passed { len: 8 } => match msc_logic::decode_capacity_10(&cap10) {
            Capacity10::Known(c) => Some(c),
            Capacity10::Needs16 => {
                let mut cap16 = [0u8; 32];
                match t.run(&msc_logic::cdb_read_capacity_16(), &mut cap16) {
                    Outcome::Passed { len } if len >= 12 => msc_logic::decode_capacity_16(&cap16),
                    _ => None,
                }
            }
            Capacity10::Invalid => None,
        },
        _ => None,
    };
    let Some(capacity) = capacity else {
        return Err((t.claimed, "read capacity"));
    };
    // A block must fit the bounce buffer whole, and be a sane power of two
    // (U13): anything else would make every read fail or misindex.
    if !capacity.block_len.is_power_of_two()
        || capacity.block_len < 512
        || capacity.block_len as usize > BOUNCE_LEN
    {
        crate::serial_println!("msc: block_len={} refused", capacity.block_len);
        return Err((t.claimed, "unsupported block size"));
    }
    crate::serial_println!(
        "msc: capacity blocks={} block_len={}",
        capacity.blocks,
        capacity.block_len
    );
    let name: &'static str = alloc::boxed::Box::leak(
        format!(
            "usb{}-{:04x}:{:04x}",
            device.slot_id, device.vendor_id, device.product_id
        )
        .into_boxed_str(),
    );
    let gone = t.gone.clone();
    let disk = MscDisk {
        identity: BlockDeviceIdentity {
            driver: "usb-storage",
            name,
            pci_bdf: None,
        },
        geometry: BlockGeometry {
            logical_block_size: capacity.block_len,
            physical_block_size: capacity.block_len,
            block_count: capacity.blocks,
        },
        transport: SwitchMutex::new(t),
        gone,
    };
    let disk = Arc::new(disk);
    let index = crate::drivers::block::register_device(disk.clone());
    crate::serial_println!("msc: registered block device index={}", index);
    #[cfg(feature = "usb-hotplug")]
    BOUND_DISKS.lock().push((device.slot_id, index, disk));
    #[cfg(not(feature = "usb-hotplug"))]
    let _ = disk;
    Ok(())
}

/// #30: the disks this driver registered, by slot, so a detach finds its own.
#[cfg(feature = "usb-hotplug")]
static BOUND_DISKS: SwitchMutex<alloc::vec::Vec<(u8, usize, Arc<MscDisk>)>> =
    SwitchMutex::new(alloc::vec::Vec::new());

/// #30: disks whose stick was pulled. Kept, never freed: the controller may
/// still hold their rings' and buffers' addresses (the retired-hubs rule,
/// `xhci_hotplug`); a stick is a few KiB plus the 64 KiB bounce buffer.
#[cfg(feature = "usb-hotplug")]
static RETIRED_DISKS: SwitchMutex<alloc::vec::Vec<Arc<MscDisk>>> =
    SwitchMutex::new(alloc::vec::Vec::new());

/// #30: the stick in `slot` was unplugged. Its reads fail from now on (the
/// one in flight stops waiting at once), its block device leaves the
/// registry, and its transport is retired once no read holds it.
#[cfg(feature = "usb-hotplug")]
fn detach_disk(slot: u8) {
    let found = {
        let mut disks = BOUND_DISKS.lock();
        disks
            .iter()
            .position(|(s, _, _)| *s == slot)
            .map(|at| disks.swap_remove(at))
    };
    let Some((_, index, disk)) = found else {
        return;
    };
    disk.gone.store(true, Ordering::Release);
    crate::drivers::block::unregister_device(index);
    // Wait out the read in flight (it sees `gone` at its next poll), so
    // nothing is using the transport after this returns.
    drop(disk.transport.lock());
    crate::serial_println!(
        "msc: slot={} detached; block device index={} gone",
        slot,
        index
    );
    RETIRED_DISKS.lock().push(disk);
}
