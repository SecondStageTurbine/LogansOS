// SPDX-License-Identifier: MPL-2.0
//! Block-device backed scenario log sink.
//!
//! This is intentionally small and synchronous. It is for bare-metal scenario
//! evidence capture where post-run extraction matters more than latency.

extern crate alloc;

use alloc::sync::Arc;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::drivers::block::{BlockDevice, BlockError, BlockResult, DmaBuffer};
use crate::serial_println;

pub type BlockDeviceHandle = Arc<dyn BlockDevice>;

/// Single source with the block write guard (`write_guard_logic`).
pub const LOG_MAGIC: &[u8; 11] = crate::drivers::block::write_guard_logic::LOG_MAGIC;
pub const HEADER_SIZE: usize = crate::drivers::block::write_guard_logic::LOG_HEADER_SIZE;
pub const DEFAULT_REGION_OFFSET: u64 = crate::drivers::block::write_guard_logic::LOG_REGION_OFFSET;
pub const DEFAULT_REGION_SIZE: u64 = 16 * 1024 * 1024;
/// v3 adds the BLAKE3 seal in the header's last 32 bytes (the write guard
/// arms the region only on a sealed header).
pub const HEADER_VERSION: u32 = crate::drivers::block::write_guard_logic::LOG_HEADER_VERSION;

const MAGIC_FIELD_LEN: usize = 16;
const BUILD_ID_LEN: usize = 32;
const SCENARIO_NAME_LEN: usize = 64;
const STAGING_SIZE: usize = 4096;
const OFFSET_REGION_SIZE: usize = 0x010;
const OFFSET_WRITE_CURSOR: usize = 0x018;
const OFFSET_BUILD_ID: usize = 0x020;
const OFFSET_SCENARIO_NAME: usize = 0x040;
const OFFSET_START_TIMESTAMP_NS: usize = 0x080;
const OFFSET_HEADER_VERSION: usize = 0x088;
const OFFSET_DEGRADATION_REASON: usize = 0x08c;
const OFFSET_DEGRADATION_OP: usize = 0x090;
const OFFSET_DEGRADATION_LBA: usize = 0x098;
const OFFSET_DEGRADATION_CURSOR: usize = 0x0a0;

static SINK: Mutex<Option<BlockDeviceLogSink>> = Mutex::new(None);
static IN_FLUSH: AtomicBool = AtomicBool::new(false);

/// OT-90: set once the clean shutdown has finished with the sink (after K3's
/// flush, or past its deadline, when an abandoned step may hold [`SINK`] or
/// wait on the sink's disk). From then on nothing here locks or writes, so
/// the power path's own prints and flushes never wait on that disk (Chad
/// #199 item 2, #267 3b); the UART still has every line.
#[cfg(feature = "power-clean")]
static CLOSED: AtomicBool = AtomicBool::new(false);

/// Close the sink for good (see [`CLOSED`]).
#[cfg(feature = "power-clean")]
pub fn close() {
    CLOSED.store(true, Ordering::Release);
}

fn closed() -> bool {
    #[cfg(feature = "power-clean")]
    return CLOSED.load(Ordering::Acquire);
    #[cfg(not(feature = "power-clean"))]
    false
}
static DROPPED_RECURSIVE_LOGS: AtomicU64 = AtomicU64::new(0);
/// Lines dropped because `SINK` was contended (CP-37: this path must never
/// spin — see `write_fmt`). Surfaced with the degradation line so a dropped
/// line is never silent loss.
static DROPPED_CONTENDED_LOGS: AtomicU64 = AtomicU64::new(0);
/// CPUs (bit = `current_cpu_id`) inside a stage-only section (OT-83 F1,
/// CP-71): their lines are staged or dropped, never waited for or flushed.
static STAGE_ONLY_CPUS: AtomicU64 = AtomicU64::new(0);
/// Lines were staged without a flush; the next `flush_pending` writes them.
static FLUSH_PENDING: AtomicBool = AtomicBool::new(false);

pub struct BlockDeviceLogSink {
    device: BlockDeviceHandle,
    region_offset: u64,
    region_size: u64,
    write_cursor: AtomicU64,
    inner: Mutex<SinkInner>,
}

struct SinkInner {
    staging: [u8; STAGING_SIZE],
    staged: usize,
    degraded: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum DegradationReason {
    None = 0,
    Io = 1,
    InvalidRequest = 2,
    Unsupported = 3,
    Other = 4,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum DegradationOp {
    Idle = 0,
    ReadBlockForRmw = 1,
    WriteBlockPayload = 2,
    WriteBlockHeaderRewrite = 3,
    DeviceFlushAfterWrite = 4,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct DegradationMarker {
    reason: DegradationReason,
    op: DegradationOp,
    lba: u64,
    cursor: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct BlockWriteFailure {
    err: BlockError,
    op: DegradationOp,
    lba: u64,
}

impl BlockDeviceLogSink {
    pub fn new(
        device: BlockDeviceHandle,
        region_offset: u64,
        region_size: u64,
    ) -> BlockResult<Self> {
        let geometry = device.geometry();
        let block_size = geometry.logical_block_size as u64;
        if region_size < HEADER_SIZE as u64
            || block_size == 0
            || !region_offset.is_multiple_of(block_size)
        {
            return Err(BlockError::InvalidRequest);
        }

        let capacity = geometry
            .block_count
            .checked_mul(block_size)
            .ok_or(BlockError::InvalidRequest)?;
        let end = region_offset
            .checked_add(region_size)
            .ok_or(BlockError::InvalidRequest)?;
        if end > capacity {
            return Err(BlockError::InvalidRequest);
        }

        Ok(Self {
            device,
            region_offset,
            region_size,
            write_cursor: AtomicU64::new(0),
            inner: Mutex::new(SinkInner {
                staging: [0; STAGING_SIZE],
                staged: 0,
                degraded: false,
            }),
        })
    }

    pub fn install(self) -> BlockResult<()> {
        if SINK.lock().is_some() {
            return Err(BlockError::Unsupported);
        }

        // Zenbook lane: the region must already carry a sealed v3 header
        // for exactly this region (`cargo xtask log-sink-header`, provisioned
        // on a disk set aside for the log) before the sink writes anything.
        // A disk without one is not ours: refuse.
        let claimed = self.provisioned_region_size();
        if claimed != Some(self.region_size) {
            serial_println!(
                "[ kernel ] block-log-sink: REFUSED: no sealed NEO_LOG_v1 header for a 0x{:x}-byte region at 0x{:x} on driver={} (found {:?}) - not writing this disk",
                self.region_size,
                self.region_offset,
                self.device.identity().driver,
                claimed
            );
            return Err(BlockError::WriteRefused);
        }

        self.rewrite_header(0).map_err(|failure| failure.err)?;
        *SINK.lock() = Some(self);
        Ok(())
    }

    /// The region size the sealed header at the region's start claims, if it
    /// validates (`write_guard_logic::log_region_bytes`). Read only.
    fn provisioned_region_size(&self) -> Option<u64> {
        let block_size = self.device.geometry().logical_block_size as usize;
        if block_size == 0 || !HEADER_SIZE.is_multiple_of(block_size) {
            return None;
        }
        let buffer = DmaBuffer::alloc_zeroed(HEADER_SIZE).ok()?;
        self.device
            .read_block(
                self.region_offset / block_size as u64,
                core::slice::from_ref(&buffer),
            )
            .ok()?;
        // SAFETY: filled by the read above; not shared with a request.
        let bytes = unsafe { buffer.as_slice() };
        crate::drivers::block::write_guard_logic::log_region_bytes(bytes.try_into().ok()?)
    }

    fn append(&self, bytes: &[u8]) {
        let mut degradation = None;
        let mut inner = self.inner.lock();
        let mut remaining = bytes;

        while !remaining.is_empty() {
            if inner.degraded {
                inner.staged = 0;
                break;
            }

            let free = STAGING_SIZE - inner.staged;
            if free == 0 {
                degradation = self.flush_locked(&mut inner);
                if degradation.is_some() || inner.degraded {
                    break;
                }
                continue;
            }

            let take = free.min(remaining.len());
            let start = inner.staged;
            let end = start + take;
            inner.staging[start..end].copy_from_slice(&remaining[..take]);
            inner.staged = end;
            remaining = &remaining[take..];

            if inner.staged == STAGING_SIZE {
                degradation = self.flush_locked(&mut inner);
                if degradation.is_some() || inner.degraded {
                    break;
                }
            }
        }

        drop(inner);
        if let Some(marker) = degradation {
            let _ = self.write_degradation_marker(marker);
        }
    }

    fn flush_locked(&self, inner: &mut SinkInner) -> Option<DegradationMarker> {
        if inner.staged == 0 {
            return None;
        }
        if inner.degraded {
            inner.staged = 0;
            return None;
        }

        let cursor = self.write_cursor.load(Ordering::Acquire);
        let payload_limit = self.region_size.saturating_sub(HEADER_SIZE as u64);
        if cursor >= payload_limit {
            inner.staged = 0;
            return self.mark_degraded_locked(
                inner,
                DegradationMarker {
                    reason: DegradationReason::InvalidRequest,
                    op: DegradationOp::Idle,
                    lba: 0,
                    cursor,
                },
            );
        }

        let writable = (payload_limit - cursor).min(inner.staged as u64) as usize;
        let payload_offset = self.region_offset + HEADER_SIZE as u64 + cursor;
        let staged = inner.staged;
        let bytes = &inner.staging[..writable];

        IN_FLUSH.store(true, Ordering::Release);
        let result =
            match self.write_bytes_at(payload_offset, bytes, DegradationOp::WriteBlockPayload) {
                Ok(()) => {
                    let new_cursor = cursor + writable as u64;
                    self.write_cursor.store(new_cursor, Ordering::Release);
                    match self.rewrite_header(new_cursor) {
                        Ok(()) => {
                            if self.device.supports_flush() {
                                self.device.flush().map_err(|err| BlockWriteFailure {
                                    err,
                                    op: DegradationOp::DeviceFlushAfterWrite,
                                    lba: 0,
                                })
                            } else {
                                Ok(())
                            }
                        }
                        Err(failure) => Err(failure),
                    }
                }
                Err(failure) => Err(failure),
            };
        IN_FLUSH.store(false, Ordering::Release);

        match result {
            Ok(()) => {
                inner.staged = 0;
                if writable < staged {
                    self.mark_degraded_locked(
                        inner,
                        DegradationMarker {
                            reason: DegradationReason::InvalidRequest,
                            op: DegradationOp::Idle,
                            lba: 0,
                            cursor: cursor + writable as u64,
                        },
                    )
                } else {
                    None
                }
            }
            Err(failure) => {
                inner.staged = 0;
                self.mark_degraded_locked(
                    inner,
                    DegradationMarker {
                        reason: degradation_reason(failure.err),
                        op: failure.op,
                        lba: failure.lba,
                        cursor: self.write_cursor.load(Ordering::Acquire),
                    },
                )
            }
        }
    }

    fn mark_degraded_locked(
        &self,
        inner: &mut SinkInner,
        marker: DegradationMarker,
    ) -> Option<DegradationMarker> {
        if inner.degraded {
            return None;
        }
        inner.degraded = true;
        let dropped = DROPPED_RECURSIVE_LOGS.load(Ordering::Acquire);
        let already_in_flush = IN_FLUSH.swap(true, Ordering::AcqRel);
        serial_println!(
            "[ kernel ] block-log-sink: degraded reason={:?} op={:?} lba={} cursor={} recursive_drops={} contended_drops={}",
            marker.reason,
            marker.op,
            marker.lba,
            marker.cursor,
            dropped,
            DROPPED_CONTENDED_LOGS.load(Ordering::Acquire),
        );
        IN_FLUSH.store(already_in_flush, Ordering::Release);
        Some(marker)
    }

    fn rewrite_header(&self, cursor: u64) -> Result<(), BlockWriteFailure> {
        let mut header = [0u8; HEADER_SIZE];
        serialize_header(
            &mut header,
            self.region_size,
            cursor,
            build_id_bytes(),
            scenario_name_bytes(),
            0,
            DegradationMarker::none(cursor),
        );
        self.write_bytes_at(
            self.region_offset,
            &header,
            DegradationOp::WriteBlockHeaderRewrite,
        )
    }

    fn write_degradation_marker(&self, marker: DegradationMarker) -> BlockResult<()> {
        let block_size = self.device.geometry().logical_block_size as usize;
        if block_size == 0
            || !HEADER_SIZE.is_multiple_of(block_size)
            || !self.region_offset.is_multiple_of(block_size as u64)
        {
            return Err(BlockError::InvalidRequest);
        }

        let buffer = DmaBuffer::alloc_zeroed(HEADER_SIZE).map_err(|_| BlockError::Io)?;
        unsafe {
            serialize_header(
                buffer
                    .as_mut_slice()
                    .try_into()
                    .map_err(|_| BlockError::InvalidRequest)?,
                self.region_size,
                marker.cursor,
                build_id_bytes(),
                scenario_name_bytes(),
                0,
                marker,
            );
        }

        let already_in_flush = IN_FLUSH.swap(true, Ordering::AcqRel);
        let result = self.device.write_block(
            self.region_offset / block_size as u64,
            core::slice::from_ref(&buffer),
        );
        IN_FLUSH.store(already_in_flush, Ordering::Release);
        result
    }

    fn write_bytes_at(
        &self,
        absolute_offset: u64,
        bytes: &[u8],
        write_op: DegradationOp,
    ) -> Result<(), BlockWriteFailure> {
        if bytes.is_empty() {
            return Ok(());
        }

        let block_size = self.device.geometry().logical_block_size as usize;
        if block_size == 0 {
            return Err(BlockWriteFailure {
                err: BlockError::InvalidRequest,
                op: DegradationOp::Idle,
                lba: 0,
            });
        }

        let mut copied = 0usize;
        while copied < bytes.len() {
            let absolute = absolute_offset
                .checked_add(copied as u64)
                .ok_or(BlockWriteFailure {
                    err: BlockError::InvalidRequest,
                    op: write_op,
                    lba: 0,
                })?;
            let lba = absolute / block_size as u64;
            let in_block = (absolute % block_size as u64) as usize;
            let take = (block_size - in_block).min(bytes.len() - copied);
            let buffer = DmaBuffer::alloc_zeroed(block_size).map_err(|_| BlockWriteFailure {
                err: BlockError::Io,
                op: write_op,
                lba,
            })?;

            if in_block != 0 || take != block_size {
                self.device
                    .read_block(lba, core::slice::from_ref(&buffer))
                    .map_err(|err| BlockWriteFailure {
                        err,
                        op: DegradationOp::ReadBlockForRmw,
                        lba,
                    })?;
            }

            unsafe {
                buffer.as_mut_slice()[in_block..in_block + take]
                    .copy_from_slice(&bytes[copied..copied + take]);
            }
            self.device
                .write_block(lba, core::slice::from_ref(&buffer))
                .map_err(|err| BlockWriteFailure {
                    err,
                    op: write_op,
                    lba,
                })?;
            copied += take;
        }

        Ok(())
    }
}

pub fn serialize_header(
    header: &mut [u8; HEADER_SIZE],
    region_size: u64,
    write_cursor: u64,
    build_id: [u8; BUILD_ID_LEN],
    scenario_name: [u8; SCENARIO_NAME_LEN],
    start_timestamp_ns: u64,
    degradation: DegradationMarker,
) {
    header[..MAGIC_FIELD_LEN].fill(0);
    header[..LOG_MAGIC.len()].copy_from_slice(LOG_MAGIC);
    header[OFFSET_REGION_SIZE..OFFSET_REGION_SIZE + 8].copy_from_slice(&region_size.to_le_bytes());
    header[OFFSET_WRITE_CURSOR..OFFSET_WRITE_CURSOR + 8]
        .copy_from_slice(&write_cursor.to_le_bytes());
    header[OFFSET_BUILD_ID..OFFSET_BUILD_ID + BUILD_ID_LEN].copy_from_slice(&build_id);
    header[OFFSET_SCENARIO_NAME..OFFSET_SCENARIO_NAME + SCENARIO_NAME_LEN]
        .copy_from_slice(&scenario_name);
    header[OFFSET_START_TIMESTAMP_NS..OFFSET_START_TIMESTAMP_NS + 8]
        .copy_from_slice(&start_timestamp_ns.to_le_bytes());
    header[OFFSET_HEADER_VERSION..OFFSET_HEADER_VERSION + 4]
        .copy_from_slice(&HEADER_VERSION.to_le_bytes());
    header[OFFSET_DEGRADATION_REASON..OFFSET_DEGRADATION_REASON + 4]
        .copy_from_slice(&(degradation.reason as u32).to_le_bytes());
    header[OFFSET_DEGRADATION_OP..OFFSET_DEGRADATION_OP + 4]
        .copy_from_slice(&(degradation.op as u32).to_le_bytes());
    header[OFFSET_DEGRADATION_LBA..OFFSET_DEGRADATION_LBA + 8]
        .copy_from_slice(&degradation.lba.to_le_bytes());
    header[OFFSET_DEGRADATION_CURSOR..OFFSET_DEGRADATION_CURSOR + 8]
        .copy_from_slice(&degradation.cursor.to_le_bytes());
    crate::drivers::block::write_guard_logic::seal_log_header(header);
}

pub fn write_fmt(args: fmt::Arguments<'_>) {
    if closed() {
        return;
    }
    if IN_FLUSH.load(Ordering::Acquire) {
        DROPPED_RECURSIVE_LOGS.fetch_add(1, Ordering::AcqRel);
        return;
    }
    if on_stage_only_cpu() {
        write_fmt_staged(args);
        return;
    }

    // NEVER block here (OT-61 C1.1, CP-37). `SINK` is a plain `spin::Mutex`, and
    // this function is reached from BOTH:
    //   - kernel context at IF=1 — every `serial_println!` (`serial::_print`
    //     calls `log::write_fmt` *outside* its `without_interrupts` block), which
    //     is therefore fully preemptible while holding `SINK`; and
    //   - a syscall body at IF=0 — `sys_write` → `serial::write_bytes` → here,
    //     where `SYSCALL` masked interrupts and nothing can preempt us.
    // That is the CP-37 asymmetry exactly: a kernel printer preempted mid-section
    // leaves `SINK` held by an off-CPU thread, and a `sys_write` on the SAME cpu
    // then spins for it with interrupts masked — so the holder can never be
    // rescheduled to release it. Unbreakable, silent, and the guest wedges until
    // the harness times out. Observed on the `threads_futex` gate scenario after
    // the pmap instance of CP-37 was fixed and the battery began getting far
    // enough to do heavy `sys_write` logging.
    //
    // Making `SINK` IRQ-safe was rejected: the flush path performs block-device
    // I/O, and holding it with interrupts masked across a completion wait would
    // trade this deadlock for a worse one (CP-2/CP-3).
    //
    // So a contended acquire DROPS the line rather than waiting — the same
    // posture this module already takes for recursion (`IN_FLUSH` above). The
    // serial UART, which is the primary channel, has already received these bytes
    // before we are called; this sink is the durable/bare-metal copy. Drops are
    // COUNTED and reported (see `note_degraded`) so loss is never silent.
    let Some(guard) = SINK.try_lock() else {
        DROPPED_CONTENDED_LOGS.fetch_add(1, Ordering::AcqRel);
        return;
    };
    let Some(sink) = guard.as_ref() else {
        return;
    };

    let mut writer = SinkWriter { sink };
    let _ = writer.write_fmt(args);
}

/// How long the panic path tries a sink lock before giving up, in spin
/// iterations (well over a millisecond; the holder may be the CPU that
/// panicked, so this can never become a wait).
#[cfg(feature = "fb-progress-console")]
const PANIC_LOCK_SPINS: u32 = 1_000_000;

/// `try_lock`, retried for a bounded number of spins.
#[cfg(feature = "fb-progress-console")]
fn try_lock_bounded<T>(lock: &Mutex<T>) -> Option<spin::MutexGuard<'_, T>> {
    for _ in 0..PANIC_LOCK_SPINS {
        if let Some(guard) = lock.try_lock() {
            return Some(guard);
        }
        core::hint::spin_loop();
    }
    None
}

/// Panic path (R1 pass 2): stage the line without ever blocking or flushing
/// inline. Both locks are tried with a bounded wait, and what does not fit in
/// the staging buffer is dropped, counted. `panic_flush` writes it out once.
#[cfg(feature = "fb-progress-console")]
pub fn write_fmt_panicking(args: fmt::Arguments<'_>) {
    if closed() {
        return;
    }
    if IN_FLUSH.load(Ordering::Acquire) {
        DROPPED_RECURSIVE_LOGS.fetch_add(1, Ordering::AcqRel);
        return;
    }
    let Some(guard) = try_lock_bounded(&SINK) else {
        DROPPED_CONTENDED_LOGS.fetch_add(1, Ordering::AcqRel);
        return;
    };
    let Some(sink) = guard.as_ref() else {
        return;
    };
    let Some(mut inner) = try_lock_bounded(&sink.inner) else {
        DROPPED_CONTENDED_LOGS.fetch_add(1, Ordering::AcqRel);
        return;
    };
    let mut stage = StageOnly(&mut inner);
    let _ = stage.write_fmt(args);
}

/// Copies into the staging buffer and never flushes.
struct StageOnly<'a>(&'a mut SinkInner);

impl Write for StageOnly<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let inner = &mut *self.0;
        if inner.degraded {
            return Ok(());
        }
        let take = (STAGING_SIZE - inner.staged).min(s.len());
        inner.staging[inner.staged..inner.staged + take].copy_from_slice(&s.as_bytes()[..take]);
        inner.staged += take;
        if take < s.len() {
            DROPPED_CONTENDED_LOGS.fetch_add(1, Ordering::AcqRel);
        }
        Ok(())
    }
}

/// Panic path: one attempt to write the staged lines out, taking each lock
/// only by a bounded try. Skipped if the panic came from inside a flush
/// (its device I/O may be what failed, or hold the driver's lock). No
/// degradation marker is written: that would be a second I/O.
#[cfg(feature = "fb-progress-console")]
pub fn panic_flush() {
    if closed() {
        return;
    }
    if IN_FLUSH.load(Ordering::Acquire) {
        return;
    }
    let Some(guard) = try_lock_bounded(&SINK) else {
        return;
    };
    let Some(sink) = guard.as_ref() else {
        return;
    };
    let Some(mut inner) = try_lock_bounded(&sink.inner) else {
        return;
    };
    let _ = sink.flush_locked(&mut inner);
}

/// Enter a stage-only section on this CPU (OT-83 F1, CP-71). Until
/// [`leave_stage_only`], every line this CPU logs is staged or dropped
/// (counted) without waiting for a lock or doing block I/O, and the durable
/// write is left to [`flush_pending`] in normal context. For paths that run
/// at IF=0 where a `SINK` holder may be preempted on this same CPU (the ring-3
/// exception kill). Call with interrupts disabled. Returns whether this CPU
/// was already in a section.
pub fn enter_stage_only() -> bool {
    let bit = cpu_bit();
    STAGE_ONLY_CPUS.fetch_or(bit, Ordering::AcqRel) & bit != 0
}

/// Leave the section entered by [`enter_stage_only`], passing back what it
/// returned so a nested section does not end its caller's.
pub fn leave_stage_only(was_stage_only: bool) {
    if !was_stage_only {
        STAGE_ONLY_CPUS.fetch_and(!cpu_bit(), Ordering::AcqRel);
    }
}

fn cpu_bit() -> u64 {
    1u64 << (crate::arch::x86_64::per_cpu::current_cpu_id() as u64 % 64)
}

fn on_stage_only_cpu() -> bool {
    // The mask is zero outside a kill, so early boot (GS not yet wired) never
    // reads the CPU id.
    let mask = STAGE_ONLY_CPUS.load(Ordering::Acquire);
    mask != 0 && mask & cpu_bit() != 0
}

/// Stage a line without waiting or flushing: both locks are tried once, and a
/// contended lock or a full staging buffer drops the line, counted.
fn write_fmt_staged(args: fmt::Arguments<'_>) {
    let Some(guard) = SINK.try_lock() else {
        DROPPED_CONTENDED_LOGS.fetch_add(1, Ordering::AcqRel);
        return;
    };
    let Some(sink) = guard.as_ref() else {
        return;
    };
    let Some(mut inner) = sink.inner.try_lock() else {
        DROPPED_CONTENDED_LOGS.fetch_add(1, Ordering::AcqRel);
        return;
    };
    let mut stage = StageOnly(&mut inner);
    let _ = stage.write_fmt(args);
    FLUSH_PENDING.store(true, Ordering::Release);
}

/// Normal context: write out lines a stage-only section left staged. Never
/// waits — if either sink lock is held the flag stays set for the next call (the holder
/// is running and will itself flush when its staging fills).
pub fn flush_pending() {
    if closed() {
        return;
    }
    if !FLUSH_PENDING.load(Ordering::Acquire) || IN_FLUSH.load(Ordering::Acquire) {
        return;
    }
    let Some(guard) = SINK.try_lock() else {
        return;
    };
    if let Some(sink) = guard.as_ref() {
        // Try-lock too: a held `inner` leaves the flag set for the next call.
        let Some(mut inner) = sink.inner.try_lock() else {
            return;
        };
        FLUSH_PENDING.store(false, Ordering::Release);
        let degradation = sink.flush_locked(&mut inner);
        drop(inner);
        if let Some(marker) = degradation {
            let _ = sink.write_degradation_marker(marker);
        }
    }
}

/// Test-only: hold `SINK` as a preempted logger would (the OT-83 F1
/// discriminator).
#[cfg(feature = "neo-test-admission-park-kill-sink-held")]
pub fn hold_sink_for_test() -> spin::MutexGuard<'static, Option<BlockDeviceLogSink>> {
    SINK.lock()
}

pub fn flush() {
    if closed() {
        return;
    }
    if IN_FLUSH.load(Ordering::Acquire) {
        DROPPED_RECURSIVE_LOGS.fetch_add(1, Ordering::AcqRel);
        return;
    }

    let mut degradation = None;
    let guard = SINK.lock();
    if let Some(sink) = guard.as_ref() {
        let mut inner = sink.inner.lock();
        degradation = sink.flush_locked(&mut inner);
    }
    drop(guard);

    if let Some(marker) = degradation {
        let guard = SINK.lock();
        if let Some(sink) = guard.as_ref() {
            let _ = sink.write_degradation_marker(marker);
        }
    }
}

struct SinkWriter<'a> {
    sink: &'a BlockDeviceLogSink,
}

impl Write for SinkWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.sink.append(s.as_bytes());
        Ok(())
    }
}

fn build_id_bytes() -> [u8; BUILD_ID_LEN] {
    let mut out = [0u8; BUILD_ID_LEN];
    copy_ascii_prefix(&mut out, env!("NEO_BUILD_ID").as_bytes());
    out
}

#[cfg(feature = "neo-test")]
fn scenario_name_bytes() -> [u8; SCENARIO_NAME_LEN] {
    let mut out = [0u8; SCENARIO_NAME_LEN];
    copy_ascii_prefix(&mut out, crate::test_harness::scenario_name().as_bytes());
    out
}

#[cfg(not(feature = "neo-test"))]
fn scenario_name_bytes() -> [u8; SCENARIO_NAME_LEN] {
    let mut out = [0u8; SCENARIO_NAME_LEN];
    copy_ascii_prefix(&mut out, b"kernel");
    out
}

fn copy_ascii_prefix(out: &mut [u8], input: &[u8]) {
    for (dst, src) in out.iter_mut().zip(input.iter().copied()) {
        *dst = if src.is_ascii() && !src.is_ascii_control() {
            src
        } else {
            b'?'
        };
    }
}

impl DegradationMarker {
    const fn none(cursor: u64) -> Self {
        Self {
            reason: DegradationReason::None,
            op: DegradationOp::Idle,
            lba: 0,
            cursor,
        }
    }
}

fn degradation_reason(err: BlockError) -> DegradationReason {
    match err {
        BlockError::Io => DegradationReason::Io,
        BlockError::InvalidRequest => DegradationReason::InvalidRequest,
        BlockError::Unsupported => DegradationReason::Unsupported,
        BlockError::NoDevice | BlockError::WriteRefused => DegradationReason::Other,
    }
}
