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

extern crate alloc;

use alloc::sync::Arc;

use super::gpt_logic::{self, Entry, Gpt};
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
}

impl PartitionDevice {
    /// The view of `entry`, one of `gpt`'s validated entries (`gpt` came from
    /// `gpt_logic::read` of `inner`). `None` when the entry is not in the
    /// table or does not fit the device.
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
        })
    }

    pub fn entry_index(&self) -> u32 {
        self.index
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
            return self.inner.submit(request);
        }
        let lba = self.map(request.lba, request.buffers)?;
        self.inner.submit(BlockRequest { lba, ..request })
    }

    fn supports_flush(&self) -> bool {
        self.inner.supports_flush()
    }

    fn flush(&self) -> BlockResult<()> {
        self.inner.flush()
    }

    fn read_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        let lba = self.map(lba, buffers)?;
        self.inner.read_block(lba, buffers)
    }

    fn write_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        let lba = self.map(lba, buffers)?;
        self.inner.write_block(lba, buffers)
    }
}
