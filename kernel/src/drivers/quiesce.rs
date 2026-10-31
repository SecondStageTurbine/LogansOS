// SPDX-License-Identifier: MPL-2.0
//! OT-90 P3b-2, K6 (`power-quiesce`): stop device DMA before power-off or
//! restart (`docs/THINK-2026-10-27-ot90-shutdown-restart.md` §5.1).
//!
//! [`Plan::prepare`] runs before K4, while every CPU still runs: it may take
//! driver locks, allocate and scan PCI. [`Plan::run`] runs after K5, alone
//! on one CPU with interrupts off: registers and config space only, every
//! wait bounded, nothing allocated or freed, no lock taken (a parked CPU may
//! hold any driver's lock).
//!
//! K6, in order:
//! 1. **The disks the write guard armed** (decision D1): each controller's
//!    own stop. NVMe gets a normal shutdown notification, so it writes its
//!    volatile cache; virtio-blk a reset; AHCI its port's engines stopped. A
//!    disk the guard hasn't armed (the Zenbook's Windows NVMe) is not
//!    touched here.
//! 2. **Every bus master:** Bus Master Enable cleared, so no function starts
//!    another DMA.
//! 3. **The IOMMU** (`vtd`): translation off, last, once nothing does DMA.
//!
//! **Resume** (THINK §10). None of it frees driver state: a resume path
//! would re-initialize each controller from its probe state.

use alloc::vec::Vec;

use crate::drivers::block::{self, ShutdownStep};
use crate::drivers::pci;

/// What K6 stops, read before K4.
pub struct Plan {
    /// Guard-armed disks' controller steps (one per controller), with the
    /// device index that named it first.
    disks: Vec<(usize, ShutdownStep)>,
    /// How many disks were left alone (not armed by the write guard).
    unarmed: usize,
    bus_masters: Vec<pci::Device>,
    #[cfg(feature = "vtd")]
    iommu: Vec<(crate::drivers::iommu::RemapRegs, u32)>,
}

impl Plan {
    /// Read everything K6 needs. Before K4.
    pub fn prepare() -> Self {
        let mut disks: Vec<(usize, ShutdownStep)> = Vec::new();
        let mut unarmed = 0;
        for index in 0..block::device_count() {
            // Never waits on a guard judgement: one stuck on a disk that
            // stopped answering would hold K6 past the shutdown's deadline.
            if block::armed_now(index) != Some(true) {
                unarmed += 1;
                continue;
            }
            let Some(step) = block::get_device(index).and_then(|d| d.shutdown_step()) else {
                continue;
            };
            if !disks.iter().any(|(_, s)| *s == step) {
                disks.push((index, step));
            }
        }
        Self {
            disks,
            unarmed,
            bus_masters: pci::bus_masters(),
            #[cfg(feature = "vtd")]
            iommu: crate::drivers::iommu::shutdown_plan(),
        }
    }

    /// K6. After K5; `wait(us, done)` bounds each wait without a lock.
    pub fn run(&self, wait: &dyn Fn(u64, &mut dyn FnMut() -> bool) -> bool) {
        for &(index, step) in &self.disks {
            let outcome = match step {
                ShutdownStep::Nvme { mmio } => {
                    block::nvme::shutdown_notify(mmio, wait).map(|()| "nvme-shutdown")
                }
                ShutdownStep::VirtioBlk { common } => {
                    block::virtio_blk::shutdown_reset(common);
                    Ok("virtio-reset")
                }
                ShutdownStep::AhciPort { port_base } => {
                    block::ahci::shutdown_stop_port(port_base);
                    Ok("ahci-port-stopped")
                }
            };
            match outcome {
                Ok(what) => {
                    crate::serial_println!("[ power ] K6 disk={} {} ok", index, what);
                }
                Err(why) => {
                    crate::serial_println!("[ power ] K6 disk={} FAILED {}", index, why);
                }
            }
        }
        for dev in &self.bus_masters {
            pci::clear_bus_master(*dev);
        }
        #[cfg(feature = "vtd")]
        let iommu_off = crate::drivers::iommu::shutdown_disable(&self.iommu);
        #[cfg(not(feature = "vtd"))]
        let iommu_off = 0usize;
        crate::serial_println!(
            "[ power ] K6 disks_stopped={} disks_left_alone={} bus_masters_cleared={} iommu_units_off={}",
            self.disks.len(),
            self.unarmed,
            self.bus_masters.len(),
            iommu_off
        );
    }
}
