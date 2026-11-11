// SPDX-License-Identifier: MPL-2.0
//! L3u-2: USB mass storage (Bulk-Only Transport + SCSI) as a block device
//! (`docs/THINK-2026-10-31-l3u-usb-mass-storage.md` §4). Read-only unless
//! the kernel is built with `usb-storage-write` (OT-100 R2,
//! `docs/THINK-2026-11-10-ot100-usb-persistence.md` §3): then WRITE(10),
//! SYNCHRONIZE CACHE(10) and a MODE SENSE(6) write-protect check, every write
//! still through `GuardedDevice`, and the probe only on an armed device.
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
//! device. The write path (`usb-storage-write`) keeps all three: the same
//! lock, the same event path, a deadline in time on every wait, and its
//! data-out stage through the same fenced bounce buffer.

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
#[cfg(feature = "usb-storage-write")]
use core::sync::atomic::{AtomicU8, AtomicU64};

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

/// OT-100 R2: the write path's waits are bounded in time (THINK §3.4, CP-80):
/// a WRITE(10) chunk, and SYNCHRONIZE CACHE (flash can take seconds).
#[cfg(feature = "usb-storage-write")]
const WRITE_DEADLINE_MS: u64 = 10_000;
#[cfg(feature = "usb-storage-write")]
const SYNC_DEADLINE_MS: u64 = 30_000;
/// The timer tick's period, as `TICKS` counts it (100 Hz).
#[cfg(feature = "usb-storage-write")]
const MS_PER_TICK: u64 = 10;
/// NOT READY on a write: retries, each after about 250 ms.
#[cfg(feature = "usb-storage-write")]
const NOT_READY_TRIES: u32 = 4;

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
    /// OT-100 R2: the current command's deadline in ms, if it has one (the
    /// write path's commands); reads keep today's poll bound.
    #[cfg(feature = "usb-storage-write")]
    deadline_ms: Option<u64>,
}

/// Wait for the transfer at ring index `index` on `pipe`: `(code, residue)`,
/// or `None` on timeout, or as soon as `gone` is set (the stick was pulled:
/// #30, an in-flight read fails at once rather than after the timeout).
fn wait(pipe: &mut EndpointPipe, index: usize, gone: &AtomicBool) -> Option<(u32, u32)> {
    wait_bounded(pipe, index, gone, WAIT_POLLS)
}

/// OT-100 R2: [`wait`], but bounded by `ms` of the timer tick (time, not
/// spins: CP-80). The poll count is only a backstop for a tick that does not
/// advance (interrupts off), sized well past the deadline at any CPU speed.
#[cfg(feature = "usb-storage-write")]
fn wait_until(
    pipe: &mut EndpointPipe,
    index: usize,
    gone: &AtomicBool,
    ms: u64,
) -> Option<(u32, u32)> {
    use crate::arch::x86_64::timer::TICKS;
    let start = TICKS.load(Ordering::Relaxed);
    let end = start.saturating_add(ms.div_ceil(MS_PER_TICK) + 1);
    let backstop = (WAIT_POLLS as u64).saturating_mul(ms.div_ceil(1000).max(1) * 4);
    for _ in 0..backstop {
        if gone.load(Ordering::Acquire) {
            return None;
        }
        xhci::poll_events();
        for event in pipe.completions() {
            if pipe.index_of(event.parameter) == Some(index) {
                return Some((event.completion_code(), event.transfer_residue()));
            }
        }
        if TICKS.load(Ordering::Relaxed) >= end {
            return None;
        }
        for _ in 0..20_000 {
            core::hint::spin_loop();
        }
    }
    None
}

/// Wait for one transfer: the command's deadline in time when it has one
/// (the write path), else today's poll bound.
fn wait_for(t: &mut Transport, inbound: bool, index: usize) -> Option<(u32, u32)> {
    #[cfg(feature = "usb-storage-write")]
    if let Some(ms) = t.deadline_ms {
        let pipe = if inbound {
            &mut t.bulk_in
        } else {
            &mut t.bulk_out
        };
        return wait_until(pipe, index, &t.gone, ms);
    }
    let pipe = if inbound {
        &mut t.bulk_in
    } else {
        &mut t.bulk_out
    };
    wait(pipe, index, &t.gone)
}

fn wait_bounded(
    pipe: &mut EndpointPipe,
    index: usize,
    gone: &AtomicBool,
    polls: u32,
) -> Option<(u32, u32)> {
    for _ in 0..polls {
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

    /// OT-100 R2: one write-path command (`msc_logic::command_dir`), with a
    /// deadline of `ms` on each of its transfers.
    #[cfg(feature = "usb-storage-write")]
    fn run_dir(&mut self, cdb: &[u8], data: msc_logic::Data<'_>, ms: u64) -> Outcome {
        let len = match &data {
            msc_logic::Data::None => 0,
            msc_logic::Data::In(b) => b.len(),
            msc_logic::Data::Out(b) => b.len(),
        };
        if len > BOUNCE_LEN {
            return Outcome::Reset {
                reason: msc_logic::ResetReason::Data,
                recovered: true,
            };
        }
        self.tag = self.tag.wrapping_add(1);
        let (tag, lun) = (self.tag, self.lun);
        self.deadline_ms = Some(ms);
        let out = msc_logic::command_dir(&mut PipeView(self), tag, lun, cdb, data);
        self.deadline_ms = None;
        out
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
        // The CBW goes from the wrapper buffer; a data-out stage (OT-100 R2,
        // `usb-storage-write` only) from the bounce buffer, never from the
        // caller's memory.
        let target = if data.len() == msc_logic::CBW_LEN {
            t.small.clone()
        } else {
            t.bounce.clone()
        };
        if data.len() > target.len() {
            return Xfer::Error;
        }
        // SAFETY: no transfer is in flight on `target` (one command at a
        // time, under the transport lock); the device reads it only after
        // the doorbell `submit` rings, and the copy is complete before it.
        unsafe { target.as_mut_slice()[..data.len()].copy_from_slice(data) };
        let Ok(addr) = fence(&target, data.len()) else {
            return Xfer::Error;
        };
        // The bounce's bytes are in memory before the TRB that names them
        // is published (CP-78; `submit` publishes the TRB, then the doorbell).
        core::sync::atomic::fence(Ordering::Release);
        let index = t.bulk_out.submit(addr, data.len() as u32);
        classify(wait_for(t, false, index), data.len())
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
        let got = classify(wait_for(t, true, index), buf.len());
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
    /// OT-100 R2: the write path's state.
    #[cfg(feature = "usb-storage-write")]
    write: WriteState,
}

/// OT-100 R2: what the write probe and the writes have learned.
#[cfg(feature = "usb-storage-write")]
struct WriteState {
    /// The probe ran (it runs once).
    probed: AtomicBool,
    /// SYNCHRONIZE CACHE worked at the probe and hasn't been refused since:
    /// `supports_flush`, and the only state in which `flush` sends anything.
    sync_ok: AtomicBool,
    /// 0 unknown, 1 not write-protected, 2 write-protected (MODE SENSE, or
    /// a DATA PROTECT sense on a write).
    protect: AtomicU8,
    /// Write-class commands this device was sent (for `usb_msc_write`'s
    /// comparison with QEMU's SCSI trace).
    writes: AtomicU64,
    syncs: AtomicU64,
    mode_senses: AtomicU64,
}

#[cfg(feature = "usb-storage-write")]
const PROTECT_UNKNOWN: u8 = 0;
#[cfg(feature = "usb-storage-write")]
const PROTECT_NO: u8 = 1;
#[cfg(feature = "usb-storage-write")]
const PROTECT_YES: u8 = 2;

#[cfg(feature = "usb-storage-write")]
impl WriteState {
    const fn new() -> Self {
        WriteState {
            probed: AtomicBool::new(false),
            sync_ok: AtomicBool::new(false),
            protect: AtomicU8::new(PROTECT_UNKNOWN),
            writes: AtomicU64::new(0),
            syncs: AtomicU64::new(0),
            mode_senses: AtomicU64::new(0),
        }
    }

    fn write_protected(&self) -> Option<bool> {
        match self.protect.load(Ordering::Acquire) {
            PROTECT_NO => Some(false),
            PROTECT_YES => Some(true),
            _ => None,
        }
    }
}

/// OT-100 R2: write-class commands sent to the usb-storage device registered
/// as `name`: (WRITE(10), SYNCHRONIZE CACHE(10), MODE SENSE(6)). Test use.
#[cfg(feature = "usb-storage-write")]
#[allow(dead_code)]
pub fn write_command_counts(device: &dyn BlockDevice) -> Option<(u64, u64, u64)> {
    let name = device.identity().name;
    let disks = ALL_DISKS.lock();
    let disk = disks.iter().find(|d| d.identity.name == name)?;
    Some((
        disk.write.writes.load(Ordering::Acquire),
        disk.write.syncs.load(Ordering::Acquire),
        disk.write.mode_senses.load(Ordering::Acquire),
    ))
}

/// OT-100 R2: every disk this driver bound, for [`write_command_counts`].
#[cfg(feature = "usb-storage-write")]
static ALL_DISKS: SwitchMutex<alloc::vec::Vec<Arc<MscDisk>>> =
    SwitchMutex::new(alloc::vec::Vec::new());

#[cfg(feature = "usb-storage-write")]
impl MscDisk {
    /// REQUEST SENSE through the write path's deadline.
    fn sense(t: &mut Transport) -> Option<msc_logic::Sense> {
        t.sense()
    }

    /// A pause of about `ms` on the timer tick (NOT READY's back-off).
    fn pause(ms: u64) {
        use crate::arch::x86_64::timer::TICKS;
        let end = TICKS
            .load(Ordering::Relaxed)
            .saturating_add(ms.div_ceil(MS_PER_TICK));
        // A backstop for a tick that does not move: about `ms` of spinning
        // at the poll loop's pace.
        for _ in 0..ms.saturating_mul(100) {
            if TICKS.load(Ordering::Relaxed) >= end {
                return;
            }
            for _ in 0..20_000 {
                core::hint::spin_loop();
            }
        }
    }

    /// One WRITE(10) chunk, with THINK §3.4's sense handling. `Ok` only when
    /// the device took every byte.
    fn write_chunk(
        &self,
        t: &mut Transport,
        at: u32,
        blocks: u16,
        bytes: &[u8],
    ) -> BlockResult<()> {
        let mut attentions = 0;
        let mut not_ready = 0;
        let mut resets = 0;
        loop {
            if self.gone.load(Ordering::Acquire) {
                crate::serial_println!("usb-msc: device gone during WRITE lba={}", at);
                return Err(BlockError::NoDevice);
            }
            self.write.writes.fetch_add(1, Ordering::AcqRel);
            let outcome = t.run_dir(
                &msc_logic::cdb_write_10(at, blocks),
                msc_logic::Data::Out(bytes),
                WRITE_DEADLINE_MS,
            );
            match outcome {
                Outcome::Passed { len } if len == bytes.len() => return Ok(()),
                Outcome::Passed { len } | Outcome::Failed { len } => {
                    let sense = Self::sense(t);
                    match sense.map(msc_logic::write_action) {
                        Some(msc_logic::WriteAction::Retry) if attentions == 0 => attentions += 1,
                        Some(msc_logic::WriteAction::Wait) if not_ready < NOT_READY_TRIES => {
                            not_ready += 1;
                            Self::pause(250);
                        }
                        Some(msc_logic::WriteAction::WriteProtected) => {
                            self.write.protect.store(PROTECT_YES, Ordering::Release);
                            crate::serial_println!(
                                "usb-msc: DATA PROTECT lba={} ; persistence read-only",
                                at
                            );
                            return Err(BlockError::WriteRefused);
                        }
                        _ => {
                            let s = sense.unwrap_or(msc_logic::Sense {
                                key: 0xff,
                                asc: 0xff,
                                ascq: 0xff,
                            });
                            crate::serial_println!(
                                "usb-msc: WRITE(10) lba={} blocks={} FAIL took={} sense={:x}/{:02x}/{:02x}",
                                at,
                                blocks,
                                len,
                                s.key,
                                s.asc,
                                s.ascq
                            );
                            return Err(BlockError::Io);
                        }
                    }
                }
                Outcome::Reset { reason, recovered } => {
                    if self.gone.load(Ordering::Acquire) {
                        crate::serial_println!("usb-msc: device gone during WRITE lba={}", at);
                        return Err(BlockError::NoDevice);
                    }
                    resets += 1;
                    crate::serial_println!(
                        "usb-msc: reset recovery #{} after WRITE(10) lba={} blocks={} reason={:?} recovered={}",
                        resets,
                        at,
                        blocks,
                        reason,
                        u8::from(recovered)
                    );
                    if reason == msc_logic::ResetReason::NotAllowed || resets > 1 || !recovered {
                        return Err(BlockError::Io);
                    }
                }
            }
        }
    }

    /// SYNCHRONIZE CACHE(10) once. ILLEGAL REQUEST latches "unsupported".
    fn sync_once(&self, t: &mut Transport) -> crate::drivers::block::SyncSupport {
        use crate::drivers::block::SyncSupport;
        self.write.syncs.fetch_add(1, Ordering::AcqRel);
        match t.run_dir(
            &msc_logic::cdb_synchronize_cache_10(),
            msc_logic::Data::None,
            SYNC_DEADLINE_MS,
        ) {
            Outcome::Passed { .. } => SyncSupport::Ok,
            Outcome::Failed { .. } => match Self::sense(t).map(msc_logic::write_action) {
                Some(msc_logic::WriteAction::Unsupported) => SyncSupport::Unsupported,
                _ => SyncSupport::Failed,
            },
            Outcome::Reset { .. } => SyncSupport::Failed,
        }
    }
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
        // OT-100 R2 (THINK §3.2): a read reaching past READ(10)'s 32-bit LBA
        // is refused before any command; `next_read` would stop short there.
        if msc_logic::read_past_32_bits(lba, count) {
            crate::serial_println!(
                "usb-msc: READ refused lba={} blocks={}: past READ(10)'s 32-bit LBA",
                lba,
                count
            );
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

    #[cfg(not(feature = "usb-storage-write"))]
    fn write_block(&self, _lba: u64, _buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        // L3u is read-only; the write guard would refuse an unmarked stick too.
        Err(BlockError::Unsupported)
    }

    /// OT-100 R2: WRITE(10) in chunks of at most the bounce buffer, the data
    /// copied into the fenced bounce, never handed over from the caller. The
    /// guard (`GuardedDevice`) has already judged the range; this device
    /// also refuses a range READ/WRITE(10) can't address, before any
    /// command.
    #[cfg(feature = "usb-storage-write")]
    fn write_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
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
        if msc_logic::read_past_32_bits(lba, count) {
            crate::serial_println!(
                "usb-msc: WRITE refused lba={} blocks={}: past WRITE(10)'s 32-bit LBA",
                lba,
                count
            );
            return Err(BlockError::InvalidRequest);
        }
        if self.write.protect.load(Ordering::Acquire) == PROTECT_YES {
            return Err(BlockError::WriteRefused);
        }
        if self.gone.load(Ordering::Acquire) {
            return Err(BlockError::NoDevice);
        }
        let mut data = alloc::vec::Vec::with_capacity(bytes as usize);
        for buffer in buffers {
            // SAFETY: the caller's buffers are not part of any device request
            // (the device only ever sees the bounce buffer).
            data.extend_from_slice(unsafe { buffer.as_slice() });
        }
        let mut t = self.transport.lock();
        let per_command =
            (BOUNCE_LEN as u64 / block).clamp(1, u64::from(msc_logic::MAX_WRITE_BLOCKS)) as u16;
        let mut done = 0u64;
        loop {
            let (at, blocks) = match msc_logic::next_write(lba, count, done, per_command) {
                Ok(Some(part)) => part,
                Ok(None) => break,
                Err(msc_logic::OutOfRange) => return Err(BlockError::InvalidRequest),
            };
            let from = (done * block) as usize;
            let len = blocks as usize * block as usize;
            self.write_chunk(&mut t, at, blocks, &data[from..from + len])?;
            done += u64::from(blocks);
        }
        drop(t);
        if self.gone.load(Ordering::Acquire) {
            return Err(BlockError::NoDevice);
        }
        Ok(())
    }

    #[cfg(feature = "usb-storage-write")]
    fn supports_flush(&self) -> bool {
        self.write.sync_ok.load(Ordering::Acquire)
    }

    /// SYNCHRONIZE CACHE(10), only once the probe has shown it works (so
    /// nothing reaches a device the guard has not armed: the probe runs only
    /// through the guard). Two retries; ILLEGAL REQUEST latches it off.
    #[cfg(feature = "usb-storage-write")]
    fn flush(&self) -> BlockResult<()> {
        use crate::drivers::block::SyncSupport;
        if !self.write.sync_ok.load(Ordering::Acquire) {
            return Err(BlockError::Unsupported);
        }
        if self.gone.load(Ordering::Acquire) {
            return Err(BlockError::NoDevice);
        }
        let mut t = self.transport.lock();
        for attempt in 0..3 {
            match self.sync_once(&mut t) {
                SyncSupport::Ok => return Ok(()),
                SyncSupport::Unsupported => {
                    self.write.sync_ok.store(false, Ordering::Release);
                    crate::serial_println!(
                        "usb-msc: SYNCHRONIZE CACHE now refused (ILLEGAL REQUEST); no durable flush"
                    );
                    return Err(BlockError::Unsupported);
                }
                SyncSupport::Failed => {
                    if self.gone.load(Ordering::Acquire) {
                        return Err(BlockError::NoDevice);
                    }
                    crate::serial_println!(
                        "usb-msc: SYNCHRONIZE CACHE failed attempt={}",
                        attempt + 1
                    );
                }
            }
        }
        Err(BlockError::Io)
    }

    #[cfg(feature = "usb-storage-write")]
    fn write_protected(&self) -> Option<bool> {
        self.write.write_protected()
    }

    /// THINK §3.5: MODE SENSE(6) header for write-protect, TEST UNIT READY,
    /// one SYNCHRONIZE CACHE(10); once, then the cached answer. Reached only
    /// through `GuardedDevice::write_probe`, which refuses an unarmed device.
    #[cfg(feature = "usb-storage-write")]
    fn write_probe(&self) -> Option<crate::drivers::block::WriteProbe> {
        use crate::drivers::block::{SyncSupport, WriteProbe};
        if self.gone.load(Ordering::Acquire) {
            return None;
        }
        let mut t = self.transport.lock();
        if self.write.probed.load(Ordering::Acquire) {
            let sync = if self.write.sync_ok.load(Ordering::Acquire) {
                SyncSupport::Ok
            } else {
                SyncSupport::Unsupported
            };
            return Some(WriteProbe {
                write_protected: self.write.write_protected(),
                sync,
            });
        }
        let mut header = [0u8; msc_logic::MODE_SENSE_HEADER_LEN as usize];
        self.write.mode_senses.fetch_add(1, Ordering::AcqRel);
        let wp = match t.run_dir(
            &msc_logic::cdb_mode_sense_6_header(),
            msc_logic::Data::In(&mut header),
            WRITE_DEADLINE_MS,
        ) {
            Outcome::Passed { len } => msc_logic::mode_sense_write_protected(&header[..len]),
            Outcome::Failed { .. } => {
                // Clear the device's pending sense; unknown is the answer.
                let _ = Self::sense(&mut t);
                None
            }
            Outcome::Reset { .. } => None,
        };
        if let Some(wp) = wp {
            self.write
                .protect
                .store(if wp { PROTECT_YES } else { PROTECT_NO }, Ordering::Release);
        }
        // A command after MODE SENSE that leaves the device ready again
        // (some report UNIT ATTENTION after a refused MODE SENSE).
        if let Outcome::Failed { .. } = t.run(&msc_logic::cdb_test_unit_ready(), &mut []) {
            let _ = Self::sense(&mut t);
        }
        let sync = self.sync_once(&mut t);
        self.write
            .sync_ok
            .store(sync == SyncSupport::Ok, Ordering::Release);
        self.write.probed.store(true, Ordering::Release);
        drop(t);
        let per_command = (BOUNCE_LEN as u64 / u64::from(self.geometry.logical_block_size))
            .clamp(1, u64::from(msc_logic::MAX_WRITE_BLOCKS));
        crate::serial_println!(
            "usb-msc: write probe dev={} wp={} sync={} max_blocks={}",
            self.identity.name,
            match self.write.write_protected() {
                Some(true) => "yes",
                Some(false) => "no",
                None => "unknown",
            },
            match sync {
                SyncSupport::Ok => "ok",
                SyncSupport::Unsupported => "unsupported",
                SyncSupport::Failed => "failed",
            },
            per_command
        );
        Some(WriteProbe {
            write_protected: self.write.write_protected(),
            sync,
        })
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
        #[cfg(feature = "usb-storage-write")]
        deadline_ms: None,
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
        #[cfg(feature = "usb-storage-write")]
        write: WriteState::new(),
    };
    let disk = Arc::new(disk);
    #[cfg(feature = "usb-storage-write")]
    ALL_DISKS.lock().push(disk.clone());
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
