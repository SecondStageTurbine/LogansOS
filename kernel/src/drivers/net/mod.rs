// SPDX-License-Identifier: MPL-2.0
//! Phase 4 network drivers.

extern crate alloc;

/// The Realtek RTL8125 2.5GbE driver (rung 1). Feature-gated: off ⇒ absent.
#[cfg(feature = "rtl8125")]
pub mod rtl8125;
pub mod virtio_net;

use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Lazy;

use crate::net::NetDevice;
use crate::serial_println;
use crate::sync::IrqSafeMutex;

struct Registry {
    devices: Vec<Arc<dyn NetDevice>>,
}

/// IRQ-safe (CP-37): the syscalls (IF=0) and the net-tick / net-rx threads
/// read it with interrupts off, while device registration (boot, USB hotplug)
/// runs IF-on; a plain spin lock held by a preempted IF-on taker would
/// deadlock an IF-off taker on the same CPU. Nothing prints under it.
///
/// #175 item 2: only [`register_device`] (boot and hotplug) allocates under it,
/// the `Vec` growing by one. Safe IF-off: the kernel heap is a fixed range
/// behind its own IF-off spin lock that takes no other lock and never parks
/// or grows (`mm::heap`), so the order is this table → the heap's lock, a leaf.
/// The readers (`get_device`, `device_count`) clone an `Arc` or read a length.
static REGISTRY: Lazy<IrqSafeMutex<Registry>> = Lazy::new(|| {
    IrqSafeMutex::new(Registry {
        devices: Vec::new(),
    })
});

pub fn init() {
    serial_println!("[ kernel ] net: init (generic link-device substrate)");
    let summary = crate::drivers::pci::scan_network_devices();
    serial_println!(
        "[ kernel ] net: pci scan complete functions={} virtio_net={} virtio_net_ok={} attached_devices={}",
        summary.functions_seen,
        summary.virtio_net_seen,
        summary.virtio_net_probe_ok,
        device_count()
    );
}

pub fn register_device(device: Arc<dyn NetDevice>) -> usize {
    // Snapshot the identity + link-state off the device before it moves into the
    // table (lock-free reads on the `Arc`), so the interface-identity registry can be
    // populated *after* this lock is dropped — the registry is a leaf lock and must
    // never nest under the device-table lock (I-5 / I-14(a)).
    let identity = device.identity().clone();
    let link = device.link_state();
    let idx = {
        let mut registry = REGISTRY.lock();
        let idx = registry.devices.len();
        registry.devices.push(device);
        // F11 S4: a status source changed (inside the lock, after the write).
        crate::net::status_gen::bump();
        idx
    };
    serial_println!(
        "[ kernel ] net: registered device index={} driver={} name={} pci_bdf={:?}",
        idx,
        identity.driver,
        identity.name,
        identity.pci_bdf
    );
    assert!(
        idx <= u32::MAX as usize,
        "net device index must fit InterfaceId"
    );
    let interface_id = idx as u32;
    // ISC-1: mirror the device into the interface-identity registry (runtime
    // InterfaceId + name + seed link-state). Inert when `interface-identity` is off.
    crate::net::identity::register(interface_id, &identity, link);
    assert!(
        crate::net::identity::registration_binding_holds(interface_id),
        "InterfaceId registry index must match net device index"
    );
    idx
}

pub fn get_device(index: usize) -> Option<Arc<dyn NetDevice>> {
    REGISTRY.lock().devices.get(index).cloned()
}

pub fn device_count() -> usize {
    REGISTRY.lock().devices.len()
}
