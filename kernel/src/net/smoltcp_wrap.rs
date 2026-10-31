// SPDX-License-Identifier: MPL-2.0
//! Thin smoltcp phy::Device adapter over the kernel NetDevice trait.

use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

use crate::net::{NetDevice, RxFrame, TxFrame};

pub struct SmolDevice<'d> {
    dev: &'d dyn NetDevice,
}

impl<'d> SmolDevice<'d> {
    pub fn new(dev: &'d dyn NetDevice) -> Self {
        Self { dev }
    }
}

pub struct NetRxToken<'a> {
    dev: &'a dyn NetDevice,
    frame: Option<RxFrame>,
}

pub struct NetTxToken<'a> {
    dev: &'a dyn NetDevice,
    frame: Option<TxFrame>,
}

impl Device for SmolDevice<'_> {
    type RxToken<'a>
        = NetRxToken<'a>
    where
        Self: 'a;
    type TxToken<'a>
        = NetTxToken<'a>
    where
        Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let rx = self.dev.rx_take()?;
        let tx = self.dev.tx_reserve();
        Some((
            NetRxToken {
                dev: self.dev,
                frame: Some(rx),
            },
            NetTxToken {
                dev: self.dev,
                frame: tx,
            },
        ))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(NetTxToken {
            dev: self.dev,
            frame: Some(self.dev.tx_reserve()?),
        })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = self.dev.mtu();
        caps
    }
}

impl RxToken for NetRxToken<'_> {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let frame = self.frame.take().expect("rx token has frame");
        let result = f(frame.as_slice());
        self.dev.rx_recycle(frame);
        result
    }
}

impl TxToken for NetTxToken<'_> {
    fn consume<R, F>(mut self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut frame = self.frame.take().expect("tx token has frame");
        let result = f(&mut frame.as_mut_slice()[..len]);
        let _ = self.dev.tx_submit(frame, len);
        result
    }
}
