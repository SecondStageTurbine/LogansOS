// SPDX-License-Identifier: MPL-2.0
//! The USB port service (the Zenbook USB input lane, feature `usb-hotplug`;
//! `Plans/NOTE-2026-10-22-zenbook-usb-input.md`).
//!
//! Before this, ports were scanned once, at boot, a few milliseconds after
//! the controller reset: a device that connected later — plugged in at the
//! desktop, or simply slower to show a connection than QEMU — was never
//! enumerated, and port-change events were counted and dropped. The service
//! re-reads every root port's PORTSC and every known hub's port status on a
//! timer (`usb::start_port_service`, a kernel thread) and enumerates what is
//! connected but not yet enumerated, up to three attempts per connection.
//!
//! A child module of `xhci` (it calls the parent's private enumeration).
//! Every enumeration runs on that one thread after boot, so commands and
//! EP0 transfers are never issued from two threads at once (the boot scan
//! finishes before the thread starts). Nothing here runs in an ISR, and no
//! lock is held across an enumeration.
//!
//! Teardown is not implemented: a detached device's slot, rings and contexts
//! stay allocated (the controller may still reference them until a Disable
//! Slot, which this lane does not issue), and a hub on a detached root port
//! is moved to a retired list, never dropped. A re-plug enumerates into a
//! new slot. The controller's slot limit bounds the leak.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use spin::Mutex;
use x86_64::VirtAddr;

use super::super::xhci_silicon_logic::{self as logic, AddressedSet, PortAction, PortTrack};
use super::{
    AddressRoute, CONTROLLER, ClaimedDevice, HUBS, PORTSC_CCS, Trb, TrbType, address_and_describe,
    append_route, control_for, enumerate_port, portsc, submit_command,
};

/// Root ports, indexed by port number (index 0 unused).
static ROOT: Mutex<Vec<PortTrack>> = Mutex::new(Vec::new());

/// What the service knows about each hub: its own route, its port count, and
/// per-port attempt counts.
#[derive(Clone, Copy)]
struct HubInfo {
    slot_id: u8,
    route: AddressRoute,
    ports: u8,
    attempts: [u8; 16],
}

static HUB_INFO: Mutex<Vec<HubInfo>> = Mutex::new(Vec::new());

/// Every finished enumeration, by (root port, route string): a hub child is
/// attached when its route is here, not when its hub port is enabled.
static ADDRESSED: Mutex<AddressedSet> = Mutex::new(AddressedSet::new());

/// Hubs whose root port detached or whose status reads failed: kept alive,
/// never serviced again (see the module docs on teardown).
static RETIRED_HUBS: Mutex<Vec<ClaimedDevice>> = Mutex::new(Vec::new());

static PASSES: AtomicU64 = AtomicU64::new(0);
static ENUMERATED: AtomicU64 = AtomicU64::new(0);
static DETACHED: AtomicU64 = AtomicU64::new(0);

/// The boot scan's result for a root port.
pub(super) fn note_root_result(port: u32, ok: bool) {
    let mut root = ROOT.lock();
    let index = port as usize;
    if root.len() <= index {
        root.resize(index + 1, PortTrack::default());
    }
    root[index] = logic::port_after(PortTrack::default(), true, PortAction::Enumerate, ok);
}

/// A device finished enumerating (boot scan or service).
pub(super) fn note_addressed(root_port: u32, route: u32) {
    if !ADDRESSED.lock().insert(root_port, route) {
        crate::serial_println!(
            "usb-hotplug: addressed-device table full - port {} route {:#x} not tracked",
            root_port,
            route
        );
    }
}

/// A hub was enumerated (boot scan or service): remember how to reach its
/// ports.
pub(super) fn note_hub(slot_id: u8, route: AddressRoute, ports: u8) {
    HUB_INFO.lock().push(HubInfo {
        slot_id,
        route,
        ports: ports.min(15),
        attempts: [0; 16],
    });
}

/// (passes, enumerated, detached) — the scenario's and summary's view.
pub fn counters() -> (u64, u64, u64) {
    (
        PASSES.load(Ordering::Relaxed),
        ENUMERATED.load(Ordering::Relaxed),
        DETACHED.load(Ordering::Relaxed),
    )
}

/// One service pass over root ports, then hub ports. Called only from the
/// port-service thread.
pub fn service_ports() {
    let Some((op, max_ports)) = CONTROLLER
        .lock()
        .as_ref()
        .map(|ctrl| (ctrl.op, ctrl.max_ports))
    else {
        return;
    };
    PASSES.fetch_add(1, Ordering::Relaxed);
    let mut tracks = ROOT.lock().clone();
    if tracks.len() <= max_ports as usize {
        tracks.resize(max_ports as usize + 1, PortTrack::default());
    }
    for port in 1..=max_ports {
        let sc = portsc(op, port);
        let connected = sc & PORTSC_CCS != 0;
        let track = tracks[port as usize];
        let action = logic::port_action(track, connected);
        let ok = match action {
            PortAction::Enumerate => {
                crate::serial_println!(
                    "usb-hotplug: port {} connected (portsc={:#010x}) - enumerating, attempt {}",
                    port,
                    sc,
                    track.attempts + 1
                );
                match enumerate_port(op, port) {
                    Ok(()) => {
                        ENUMERATED.fetch_add(1, Ordering::Relaxed);
                        true
                    }
                    Err(reason) => {
                        crate::serial_println!(
                            "usb-hotplug: port {} enumeration failed: {}",
                            port,
                            reason
                        );
                        false
                    }
                }
            }
            PortAction::Detached => {
                crate::serial_println!("usb-hotplug: port {} disconnected", port);
                DETACHED.fetch_add(1, Ordering::Relaxed);
                ADDRESSED.lock().remove_subtree(port, 0);
                retire_hubs_on(port);
                false
            }
            PortAction::Nothing | PortAction::GaveUp => false,
        };
        let after = logic::port_after(track, connected, action, ok);
        if action == PortAction::Enumerate && !ok && after.attempts == logic::MAX_ATTEMPTS {
            crate::serial_println!(
                "usb-hotplug: port {} failed {} times - left alone until it disconnects",
                port,
                logic::MAX_ATTEMPTS
            );
        }
        tracks[port as usize] = after;
        if action != PortAction::Nothing {
            crate::log::flush();
        }
    }
    *ROOT.lock() = tracks;
    service_hubs(op);
}

fn retire_hubs_on(root_port: u32) {
    let mut retired = Vec::new();
    {
        let mut hubs = HUBS.lock();
        let mut index = 0;
        while index < hubs.len() {
            if hubs[index].port == root_port {
                retired.push(hubs.swap_remove(index));
            } else {
                index += 1;
            }
        }
    }
    if retired.is_empty() {
        return;
    }
    let slots: Vec<u8> = retired.iter().map(|hub| hub.slot_id).collect();
    HUB_INFO
        .lock()
        .retain(|info| !slots.contains(&info.slot_id));
    crate::serial_println!(
        "usb-hotplug: retired {} hub(s) behind port {}",
        retired.len(),
        root_port
    );
    RETIRED_HUBS.lock().extend(retired);
}

fn service_hubs(op: VirtAddr) {
    // Take the hubs out so a child that is itself a hub can be pushed by
    // `enumerate_hub` without this pass holding `HUBS`.
    let mut hubs = core::mem::take(&mut *HUBS.lock());
    let mut failed = Vec::new();
    for (index, hub) in hubs.iter_mut().enumerate() {
        let Some(info) = HUB_INFO
            .lock()
            .iter()
            .find(|info| info.slot_id == hub.slot_id)
            .copied()
        else {
            continue;
        };
        let mut attempts = info.attempts;
        let mut status_failed = false;
        for child in 1..=info.ports {
            let mut status = [0u8; 4];
            if hub
                .control_read(0xA3, 0, 0, u16::from(child), 4, &mut status)
                .is_err()
            {
                status_failed = true;
                break;
            }
            let status = u16::from_le_bytes([status[0], status[1]]);
            let connected = status & 1 != 0;
            // Attached means enumerated, not PORT_ENABLE: the reset enables
            // the port before Enable Slot / Address Device can still fail.
            let child_route = append_route(info.route.route_string, child);
            let track = PortTrack {
                attached: ADDRESSED.lock().contains(info.route.root_port, child_route),
                attempts: attempts[child as usize],
            };
            let action = logic::port_action(track, connected);
            if action == PortAction::Detached {
                crate::serial_println!(
                    "usb-hotplug: hub slot={} port={} disconnected",
                    hub.slot_id,
                    child
                );
                DETACHED.fetch_add(1, Ordering::Relaxed);
                ADDRESSED
                    .lock()
                    .remove_subtree(info.route.root_port, child_route);
            }
            let ok = action == PortAction::Enumerate
                && {
                    crate::serial_println!(
                        "usb-hotplug: hub slot={} port={} connected status={:#06x} - enumerating, attempt {}",
                        hub.slot_id,
                        child,
                        status,
                        track.attempts + 1
                    );
                    match enumerate_hub_child(op, hub, info.route, child) {
                        Ok(()) => {
                            ENUMERATED.fetch_add(1, Ordering::Relaxed);
                            true
                        }
                        Err(reason) => {
                            crate::serial_println!(
                                "usb-hotplug: hub slot={} port={} enumeration failed: {}",
                                hub.slot_id,
                                child,
                                reason
                            );
                            false
                        }
                    }
                };
            attempts[child as usize] = logic::port_after(track, connected, action, ok).attempts;
        }
        if status_failed {
            crate::serial_println!(
                "usb-hotplug: hub slot={} port status read failed - hub retired",
                hub.slot_id
            );
            failed.push(index);
        }
        if let Some(stored) = HUB_INFO
            .lock()
            .iter_mut()
            .find(|stored| stored.slot_id == info.slot_id)
        {
            stored.attempts = attempts;
        }
    }
    let mut retired = Vec::new();
    for index in failed.into_iter().rev() {
        retired.push(hubs.swap_remove(index));
    }
    if !retired.is_empty() {
        let slots: Vec<u8> = retired.iter().map(|hub| hub.slot_id).collect();
        HUB_INFO
            .lock()
            .retain(|info| !slots.contains(&info.slot_id));
        RETIRED_HUBS.lock().extend(retired);
    }
    let mut guard = HUBS.lock();
    hubs.append(&mut guard);
    *guard = hubs;
}

/// Reset one connected child port of `hub` and enumerate what is behind it.
/// The same sequence as `enumerate_hub`'s per-port body (kept separate so the
/// feature-off build is unchanged): SET_FEATURE(PORT_RESET), wait for
/// connected + enabled + reset done, Enable Slot, then Address Device with
/// the child's route string and the hub as its transaction translator.
fn enumerate_hub_child(
    op: VirtAddr,
    hub: &mut ClaimedDevice,
    route: AddressRoute,
    child_port: u8,
) -> Result<(), &'static str> {
    hub.control_no_data(0x23, 3, 4, u16::from(child_port))?;
    let mut ready = None;
    let mut error = None;
    // USB 2.0 §7.1.7.5: a reset lasts 10–20 ms; allow at least 500 ms.
    super::silicon::wait_at_least_ms(500, || {
        let mut status = [0u8; 4];
        match hub.control_read(0xA3, 0, 0, u16::from(child_port), 4, &mut status) {
            Err(reason) => {
                error = Some(reason);
                true
            }
            Ok(()) => {
                let current = u16::from_le_bytes([status[0], status[1]]);
                let done = current & 1 != 0 && current & (1 << 4) == 0 && current & (1 << 1) != 0;
                if done {
                    ready = Some(current);
                }
                done
            }
        }
    });
    if let Some(reason) = error {
        return Err(reason);
    }
    let current = ready.ok_or("reset did not enable child")?;
    let speed = if current & (1 << 9) != 0 {
        super::super::UsbSpeed::Low
    } else if current & (1 << 10) != 0 {
        super::super::UsbSpeed::High
    } else {
        super::super::UsbSpeed::Full
    };
    let enable = Trb {
        parameter: 0,
        status: 0,
        control: control_for(TrbType::EnableSlotCmd, false, 0),
    };
    let completion = submit_command(enable).ok_or("enable-slot: no completion")?;
    if completion.completion_code() != 1 || completion.slot_id() == 0 {
        return Err("enable-slot refused");
    }
    let child = AddressRoute {
        root_port: route.root_port,
        route_string: append_route(route.route_string, child_port),
        parent_hub_slot: hub.slot_id,
        parent_port: child_port,
        parent_speed: hub.speed,
    };
    address_and_describe(op, child, completion.slot_id() as u8, speed)
}
