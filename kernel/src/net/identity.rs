// SPDX-License-Identifier: MPL-2.0
//! The runtime `InterfaceId` registry (Phase 7.5 CB-3, ISC-1/ISC-2) — the layer
//! above `NetDevice` that gives each device a runtime `InterfaceId`, a runtime name
//! (`eth0`/`wlan0`), and a hotplug-tracked link-state, replacing the baked
//! `NetDeviceIdentity.name`/`driver` `&'static str` literals as the *identity*
//! surface.
//!
//! ## The ISC-1 binding invariant
//!
//! A registry entry's `id` is the **device-registry index** it was registered with
//! (`drivers::net::register_device` returns it), so **registry index `i` ≡
//! `ipv4`/`ipv6` `interface_id` `i` ≡ `get_device(i)`**. Source selection and egress
//! both key on this single integer; the binding is asserted by every consumer that
//! looks an interface up by `id` and then calls `get_device(id)`.
//!
//! ## Lock discipline (I-5 / I-14(a))
//!
//! [`REGISTRY`] is a **leaf `IrqSafeMutex`** (CP-37: the syscalls and the net
//! tick take it IF-off, USB hotplug and `/net/if` reads IF-on; a plain spin lock
//! held by a preempted IF-on taker would deadlock an IF-off one on the same CPU;
//! nothing prints under it). It is NOT `new_ordered`, so it
//! adds zero machine-checked lock-ordering call sites (the VT-d-state-lock
//! precedent; the I-5 lockdep stays at its prior count, the USB band ceiling
//! unchanged). Every public fn here takes the lock **alone**: it snapshots or
//! mutates the small `Vec` and releases before any other subsystem lock is taken, so
//! the registry can never nest under or over another lock. It imports **no**
//! AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/the reservation/`TABLE`) and is
//! never acquired in a driver IRQ/ISR path (it is touched only from the
//! device-management/control path and `net::tick`) — I-14(a) lock isolation, by
//! construction (grep-checkable: this file references no governance lock).
//!
//! ## Kill-switch (ISC-5)
//!
//! Every entry point is gated by [`identity_logic::INTERFACE_IDENTITY_ENABLED`]
//! (`interface-identity`, default-off): with it off, [`register`] is inert, the
//! registry stays empty, and no consumer ever finds an interface here — so the net
//! stack is bit-for-bit 7.5-C2.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use spin::Lazy;

use crate::net::identity_logic::{self, IfClass};
use crate::net::{LinkState, NetDeviceIdentity};
use crate::sync::IrqSafeMutex;

/// One interface as the registry knows it.
#[derive(Clone, Eq, PartialEq, Debug)]
struct InterfaceEntry {
    /// Device-registry index (the ISC-1 binding: `id` == `get_device(id)` index ==
    /// `ipv4`/`ipv6` `interface_id`).
    id: u32,
    /// Runtime name (`eth0`, `wlan0`, …) assigned by class + per-class ordinal.
    name: String,
    /// Interface class, inferred from the device's existing identity strings.
    class: IfClass,
    /// Hotplug-tracked link-state — the ISC-2 single source of truth for
    /// "is `wlan0` up?". Seeded at registration, refreshed from the generic
    /// `NetDevice::link_state()` by [`refresh_link_states`] and updated by
    /// [`set_link_state`] on hotplug events.
    link: LinkState,
}

/// The registry — a leaf lock over a small `Vec` keyed by `id`.
///
/// #175 item 2: what runs under it on the hot paths (the net tick's
/// [`refresh_link_states`], source selection's [`snapshot_links`], [`name`])
/// allocates nothing. Registration ([`register`], boot and hotplug only) does
/// allocate under it: the name needs the ordinal the lock serializes. That is
/// safe IF-off: the kernel heap is a fixed range behind its own IF-off spin
/// lock that takes no other lock and never parks or grows (`mm::heap`), so
/// the order is this registry → the heap's lock, a leaf, with no inverse.
static REGISTRY: Lazy<IrqSafeMutex<Vec<InterfaceEntry>>> =
    Lazy::new(|| IrqSafeMutex::new(Vec::new()));

/// Register a freshly-attached device at registry index `id`, seeding its link-state.
/// Called from the generic `drivers::net::register_device` (NOT a driver) right after
/// the device joins the device table, so `id` is the device-registry index.
///
/// Inert when `interface-identity` is off (the registry stays empty — 7.5-C2
/// bit-for-bit). The runtime name is `<class><ordinal>` where the ordinal is the
/// count of already-registered devices of the same class, and the class is inferred
/// from the existing identity strings (no driver edits — the mt7601u/xHCI scope
/// guard).
pub fn register(id: u32, identity: &NetDeviceIdentity, initial_link: LinkState) {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return;
    }
    let class = identity_logic::classify(identity.driver, identity.name);
    let name = {
        let mut registry = REGISTRY.lock();
        if registry.iter().any(|e| e.id == id) {
            return;
        }
        let ordinal = registry.iter().filter(|e| e.class == class).count();
        let mut name = String::from(class.prefix());
        push_u32(&mut name, ordinal as u32);
        registry.push(InterfaceEntry {
            id,
            name: name.clone(),
            class,
            link: initial_link,
        });
        // F11 S4: a status source changed (inside the lock, after the write).
        crate::net::status_gen::bump();
        name
    };
    crate::serial_println!(
        "[ kernel ] net: interface registered id={} name={} class={:?} link={:?}",
        id,
        name.as_str(),
        class,
        initial_link
    );
}

/// Registration-time assertion hook for the ISC-1 binding:
/// `InterfaceId i == drivers::net::get_device(i) index == ipv4/ipv6 interface_id i`.
/// Inert when identity is off; when on, the just-registered entry must occupy its
/// own numeric index so later source-selection fallbacks cannot hide drift as
/// interface 0.
pub fn registration_binding_holds(id: u32) -> bool {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return true;
    }
    let registry = REGISTRY.lock();
    match registry.get(id as usize) {
        Some(entry) => entry.id == id,
        None => false,
    }
}

/// Update an interface's link-state — the ISC-2 hotplug mutator. USB attach/detach
/// and WiFi assoc/disassoc reach the registry through this (via the generic
/// link-state refresh below), and the live-proof scenario drives it directly to make
/// an interface appear down. No-op when the interface is unknown / identity is off.
pub fn set_link_state(id: u32, link: LinkState) {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return;
    }
    let mut registry = REGISTRY.lock();
    if let Some(entry) = registry.iter_mut().find(|e| e.id == id) {
        // F11 S4: only a change is a new status (the refresh runs every tick).
        if entry.link != link {
            entry.link = link;
            crate::net::status_gen::bump();
        }
    }
}

/// Refresh every interface's cached link-state from the generic
/// `NetDevice::link_state()` trait method — the production path by which hotplug
/// (assoc/disassoc, attach/detach, already maintained inside each driver) reaches the
/// registry **without editing any driver**. Called from `net::tick`. Inert when
/// identity is off.
///
/// Takes the registry lock alone: it first snapshots the ids under the lock, drops
/// the lock, queries `get_device(id).link_state()` lock-free (a cloned `Arc`), then
/// re-takes the lock to store — so the registry lock never nests under the device
/// table lock.
pub fn refresh_link_states() {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return;
    }
    // #175 item 2: one entry at a time, so nothing is allocated under the
    // lock (the ids used to be collected into a `Vec` under it).
    // The guard must end before `set_link_state` re-takes the lock: bound in
    // its own statement, not a `while let` scrutinee (whose temporaries live
    // through the body).
    let mut i = 0;
    loop {
        let id = REGISTRY.lock().get(i).map(|e| e.id);
        let Some(id) = id else {
            break;
        };
        i += 1;
        let live = match crate::drivers::net::get_device(id as usize) {
            Some(dev) => dev.link_state(),
            None => continue,
        };
        set_link_state(id, live);
    }
}

/// The registry's link-state for `id` — the single source of truth `select_source`
/// consults. `Unknown` when the interface is not registered / identity is off.
pub fn link_state(id: u32) -> LinkState {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return LinkState::Unknown;
    }
    REGISTRY
        .lock()
        .iter()
        .find(|e| e.id == id)
        .map(|e| e.link)
        .unwrap_or(LinkState::Unknown)
}

/// The runtime name (`eth0`/`wlan0`) for `id`, or `None` if unknown / identity off.
pub fn name(id: u32) -> Option<String> {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return None;
    }
    // #175 item 2: the bytes are copied out under the lock (a name is a
    // class prefix and a `u32`, well inside `NAME_MAX`); the `String` is
    // made after it.
    let mut buf = [0u8; NAME_MAX];
    let len = REGISTRY.lock().iter().find(|e| e.id == id).map(|e| {
        let n = e.name.len().min(NAME_MAX);
        buf[..n].copy_from_slice(&e.name.as_bytes()[..n]);
        n
    })?;
    core::str::from_utf8(&buf[..len]).ok().map(String::from)
}

/// The longest runtime name [`name`] copies out (`<class><u32>`).
const NAME_MAX: usize = 32;

/// The `id` of the interface named `name` (the `/net/if/<name>` lookup), or `None`.
pub fn id_for_name(name: &str) -> Option<u32> {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return None;
    }
    REGISTRY
        .lock()
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.id)
}

/// Snapshot `(id, up)` for every registered interface, in registration order. The
/// caller (source selection) adds the family-specific `has_source` per id to build
/// the [`identity_logic::Candidate`] set, without holding the registry lock across
/// the ipv4/ipv6 table locks. Empty when identity is off.
pub fn snapshot_links() -> Vec<(u32, bool)> {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return Vec::new();
    }
    // #175 item 2: sized outside the lock, then filled under it; a
    // registration in between (it only appends) means one more pass. The
    // last pass takes the oldest entries that fit (Chad #241 1b).
    identity_logic::snapshot_sized(
        || REGISTRY.lock().len(),
        |out, last| {
            let registry = REGISTRY.lock();
            if registry.len() > out.capacity() && !last {
                return false;
            }
            let fits = out.capacity();
            out.extend(
                registry
                    .iter()
                    .take(fits)
                    .map(|e| (e.id, e.link == LinkState::Up)),
            );
            true
        },
    )
}

/// Number of registered interfaces (identity on) — 0 when identity is off.
pub fn count() -> usize {
    if !identity_logic::INTERFACE_IDENTITY_ENABLED {
        return 0;
    }
    REGISTRY.lock().len()
}

/// Append the decimal of `value` to `name` without `format!` (keeps this allocation
/// path tiny and panic-free for the registry hot spot).
fn push_u32(name: &mut String, value: u32) {
    if value == 0 {
        name.push('0');
        return;
    }
    let mut digits = [0u8; 10];
    let mut n = value;
    let mut i = digits.len();
    while n > 0 {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    for &d in &digits[i..] {
        name.push(d as char);
    }
}
