// SPDX-License-Identifier: MPL-2.0
//! OT-87 R3: a partition of a GPT disk as a block device of its own
//! (`docs/THINK-2026-10-27-ot87-installer.md` §4.2).
//!
//! NeoFS mounts it unchanged: its offsets are device-relative, and this view
//! makes the partition's first LBA its LBA 0. Every read and write is offset
//! and bounded ([`gpt_logic::partition_lba`]): a request that would cross the
//! partition's end is refused, never truncated. Writes still reach the
//! registered device's write guard (`inner` is the `GuardedDevice`), which
//! judges them against the whole disk, so a partition the guard did not arm
//! stays unwritten.
//!
//! L3a (G7, `docs/THINK-2026-10-31-l3a-fat-readonly.md` §6a): the same view
//! over a believed MBR entry ([`PartitionDevice::from_mbr`]), so a stick's
//! FAT partition mounts as `"deviceN:pM"`.

extern crate alloc;

use alloc::sync::Arc;

use super::gpt_logic;
#[cfg(feature = "neofs-partition")]
use super::gpt_logic::{Entry, Gpt};
use super::{
    BlockDevice, BlockDeviceIdentity, BlockError, BlockGeometry, BlockOp, BlockRequest,
    BlockResult, DmaBuffer,
};

pub struct PartitionDevice {
    inner: Arc<dyn BlockDevice>,
    first_lba: u64,
    lba_count: u64,
    /// 1-based entry number, for the logs.
    index: u32,
    identity: BlockDeviceIdentity,
    /// OT-100 R3: the last failed write or flush's class
    /// (`persist_logic::BlockFailure` code; 0 none), for the drain worker.
    #[cfg(feature = "usb-persistence")]
    failure: core::sync::atomic::AtomicU8,
}

impl PartitionDevice {
    /// L3a (G7): the view of a believed MBR entry
    /// ([`super::mbr_logic::partition`] checked it against `inner`'s size).
    /// `None` when it does not fit the device.
    #[cfg(feature = "mbr-partitions")]
    pub fn from_mbr(inner: Arc<dyn BlockDevice>, entry: &super::mbr_logic::Entry) -> Option<Self> {
        let end = entry.first_lba.checked_add(entry.lba_count)?;
        if entry.lba_count == 0 || end > inner.geometry().block_count {
            return None;
        }
        let identity = inner.identity().clone();
        Some(Self {
            inner,
            first_lba: entry.first_lba,
            lba_count: entry.lba_count,
            index: entry.index,
            identity,
            #[cfg(feature = "usb-persistence")]
            failure: core::sync::atomic::AtomicU8::new(0),
        })
    }

    /// The view of `entry`, one of `gpt`'s validated entries (`gpt` came from
    /// `gpt_logic::read` of `inner`). `None` when the entry is not in the
    /// table or does not fit the device.
    #[cfg(feature = "neofs-partition")]
    pub fn new(inner: Arc<dyn BlockDevice>, gpt: &Gpt, entry: &Entry) -> Option<Self> {
        if !gpt.entries.contains(entry) {
            return None;
        }
        let lba_count = entry
            .last_lba
            .checked_sub(entry.first_lba)?
            .checked_add(1)?;
        let end = entry.first_lba.checked_add(lba_count)?;
        if end > inner.geometry().block_count {
            return None;
        }
        let identity = inner.identity().clone();
        Some(Self {
            inner,
            first_lba: entry.first_lba,
            lba_count,
            index: entry.index,
            identity,
            #[cfg(feature = "usb-persistence")]
            failure: core::sync::atomic::AtomicU8::new(0),
        })
    }

    pub fn entry_index(&self) -> u32 {
        self.index
    }

    /// OT-100 R3: note a failed write (`flush` false) or flush, and pass the
    /// result on unchanged.
    #[cfg(feature = "usb-persistence")]
    fn note(&self, flush: bool, result: BlockResult<()>) -> BlockResult<()> {
        use crate::persist_logic::BlockFailure;
        if let Err(error) = result {
            let failure = match (flush, error) {
                (_, super::BlockError::WriteRefused) => BlockFailure::WriteRefused,
                (_, super::BlockError::NoDevice) => BlockFailure::NoDevice,
                (true, _) => BlockFailure::Flush,
                (false, super::BlockError::Io) => BlockFailure::Io,
                (false, _) => BlockFailure::Other,
            };
            self.failure
                .store(failure.code(), core::sync::atomic::Ordering::Release);
        }
        result
    }

    pub fn first_lba(&self) -> u64 {
        self.first_lba
    }

    pub fn lba_count(&self) -> u64 {
        self.lba_count
    }

    /// The device LBA for a partition request at `lba` over `buffers`.
    fn map(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<u64> {
        let block = self.inner.geometry().logical_block_size as u64;
        let bytes: u64 = buffers.iter().map(|b| b.len() as u64).sum();
        if block == 0 || !bytes.is_multiple_of(block) {
            return Err(BlockError::InvalidRequest);
        }
        gpt_logic::partition_lba(self.first_lba, self.lba_count, lba, bytes / block)
            .ok_or(BlockError::InvalidRequest)
    }
}

impl BlockDevice for PartitionDevice {
    #[cfg(feature = "power-quiesce")]
    fn shutdown_step(&self) -> Option<super::ShutdownStep> {
        self.inner.shutdown_step()
    }

    fn identity(&self) -> &BlockDeviceIdentity {
        &self.identity
    }

    fn geometry(&self) -> BlockGeometry {
        BlockGeometry {
            block_count: self.lba_count,
            ..self.inner.geometry()
        }
    }

    fn submit(&self, request: BlockRequest<'_>) -> BlockResult<()> {
        if request.op == BlockOp::Flush {
            #[cfg(feature = "usb-persistence")]
            return self.note(true, self.inner.submit(request));
            #[cfg(not(feature = "usb-persistence"))]
            return self.inner.submit(request);
        }
        let lba = self.map(request.lba, request.buffers)?;
        #[cfg(feature = "usb-persistence")]
        if request.op == BlockOp::Write {
            return self.note(false, self.inner.submit(BlockRequest { lba, ..request }));
        }
        self.inner.submit(BlockRequest { lba, ..request })
    }

    fn supports_flush(&self) -> bool {
        self.inner.supports_flush()
    }

    fn flush(&self) -> BlockResult<()> {
        #[cfg(feature = "usb-persistence")]
        return self.note(true, self.inner.flush());
        #[cfg(not(feature = "usb-persistence"))]
        self.inner.flush()
    }

    #[cfg(feature = "usb-persistence")]
    fn last_failure(&self) -> Option<crate::persist_logic::BlockFailure> {
        crate::persist_logic::BlockFailure::from_code(
            self.failure.load(core::sync::atomic::Ordering::Acquire),
        )
    }

    #[cfg(feature = "usb-storage-write")]
    fn write_protected(&self) -> Option<bool> {
        self.inner.write_protected()
    }

    #[cfg(feature = "usb-storage-write")]
    fn write_probe(&self) -> Option<super::WriteProbe> {
        self.inner.write_probe()
    }

    fn read_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        let lba = self.map(lba, buffers)?;
        self.inner.read_block(lba, buffers)
    }

    fn write_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        let lba = self.map(lba, buffers)?;
        #[cfg(feature = "usb-persistence")]
        return self.note(false, self.inner.write_block(lba, buffers));
        #[cfg(not(feature = "usb-persistence"))]
        self.inner.write_block(lba, buffers)
    }
}
