// SPDX-License-Identifier: MPL-2.0
//! Kernel-internal link-layer device trait for Phase 4 BUILD-1a.

extern crate alloc;

use alloc::sync::Arc;

use crate::drivers::block::DmaBuffer;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct NetDeviceIdentity {
    pub driver: &'static str,
    pub name: &'static str,
    pub pci_bdf: Option<(u8, u8, u8)>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum LinkState {
    Up,
    Down,
    Unknown,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum NetError {
    NoDevice,
    RingFull,
    InvalidFrame,
    Io,
}

pub type NetResult<T> = Result<T, NetError>;

pub struct TxFrame {
    pub(crate) buffer: Arc<DmaBuffer>,
}

impl TxFrame {
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: a reserved TxFrame is exclusively owned by the caller until
        // it is submitted back to the device.
        unsafe { self.buffer.as_mut_slice() }
    }
}

pub struct RxFrame {
    pub(crate) buffer: Arc<DmaBuffer>,
    pub(crate) offset: usize,
    pub(crate) len: usize,
}

impl RxFrame {
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: RxFrame owns the completed DMA buffer until Drop/recycle.
        unsafe { &self.buffer.as_slice()[self.offset..self.offset + self.len] }
    }
}

pub trait NetDevice: Send + Sync {
    fn identity(&self) -> &NetDeviceIdentity;
    fn mac_address(&self) -> [u8; 6];
    fn link_state(&self) -> LinkState;
    fn mtu(&self) -> usize;

    fn tx_reserve(&self) -> Option<TxFrame>;
    fn tx_submit(&self, frame: TxFrame, len: usize) -> NetResult<()>;
    fn rx_take(&self) -> Option<RxFrame>;
    fn rx_recycle(&self, frame: RxFrame);
    fn service(&self);
}
