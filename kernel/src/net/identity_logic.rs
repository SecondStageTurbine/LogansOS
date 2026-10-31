// SPDX-License-Identifier: MPL-2.0
//! Interface-identity pure logic — the kill-switch + multi-interface source pick
//! (Phase 7.5 CB-3, the 7.5-D identity layer). I-2-form kill-switch, mirrored on
//! `sched::file_cap_logic`.
//!
//! This module holds the *decisions* of the interface-identity layer with **zero**
//! kernel dependencies, so the host KAT crate (`tools/interface-identity-logic-tests`)
//! can `#[path]`-mount it and prove "disabled ≡ the pre-identity behavior, bit-for-bit"
//! exhaustively, in one binary, by sweeping every representable candidate set against
//! both `enabled` states.
//!
//! ## The two switches (ISC-5)
//!
//! - [`INTERFACE_IDENTITY_ENABLED`] (`interface-identity`, default-off) gates the
//!   runtime `InterfaceId` registry, the runtime names, hotplug link-state, the
//!   multi-interface `select_source`, and the `/net/if/<name>` capability path.
//!   With it **off**, [`chosen_interface`] returns `0` for every candidate set, so
//!   the net stack collapses to the proven 7.5-C2 single-device behavior
//!   (`get_device(0)` / `preferred_address(0)`) bit-for-bit — an I-2-form collapse
//!   to the already-proven core.
//! - [`INTERFACE_CAPS_ENABLED`] (`interface-caps`, default-off, a *second*
//!   sub-switch that depends on `interface-identity`) gates whether network egress
//!   *requires* presenting an interface capability. Default-off ⇒ access is ambient
//!   exactly as today, so naming/identity can land and bake before access is gated.
//!   [`egress_requires_cap`] is the whole of that decision.
//!
//! ## The selection (ISC-3)
//!
//! [`chosen_interface`] is the multi-interface extension of the Phase-4 source pick.
//! Disabled, it is the constant `0` (interface 0 only — the current hardcode).
//! Enabled, it considers every candidate and returns the lowest-index interface that
//! is **UP and has a usable source address** for the destination family, falling
//! back to interface `0` when none qualifies (so a single-device system is
//! unchanged). The kernel builds the candidate list per family (a candidate's
//! `has_source` is `ipv4::preferred_address(i).is_some()` / the v6 analogue) and the
//! per-interface RFC-6724 preferred-address rule still runs inside each interface;
//! this function only chooses *which* UP interface's source wins. (Full cross-
//! interface RFC-6724 longest-prefix-match + the IPv6 RA-per-interface population
//! are flagged follow-ons — see ADR-068; the single-device equality gate does not
//! depend on them.)

/// `interface-identity` (default-off): the runtime InterfaceId registry, runtime
/// names, hotplug link-state, multi-interface `select_source`, and the
/// `/net/if/<name>` cap path. Off ⇒ the net stack is bit-for-bit 7.5-C2.
pub const INTERFACE_IDENTITY_ENABLED: bool = cfg!(feature = "interface-identity");

/// `interface-caps` (default-off; depends on `interface-identity`): whether network
/// egress *requires* an interface capability. Off ⇒ ambient access, as today.
pub const INTERFACE_CAPS_ENABLED: bool = cfg!(feature = "interface-caps");

/// A candidate interface for source selection. The kernel fills one per registered
/// device, with `up` from the registry's link-state (the ISC-2 single source of
/// truth) and `has_source` from the family-specific preferred-address lookup.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Candidate {
    /// Registry index == ipv4/ipv6 `interface_id` (the ISC-1 binding invariant).
    pub index: u32,
    /// Link-state is `LinkState::Up` per the registry.
    pub up: bool,
    /// The interface has a preferred source address for the destination family.
    pub has_source: bool,
}

impl Candidate {
    pub const fn new(index: u32, up: bool, has_source: bool) -> Self {
        Self {
            index,
            up,
            has_source,
        }
    }
}

/// Choose the egress/source interface for a destination of a given family, given
/// the per-interface candidate set.
///
/// - **Disabled** (`enabled == false`): always `0` — bit-for-bit the pre-identity
///   `get_device(0)` / `preferred_address(0)` hardcode. This is the I-2 baseline the
///   host KAT proves for every candidate set.
/// - **Enabled**: the lowest `index` among candidates that are `up && has_source`,
///   else `0` (the single-device system, and any system with no usable non-zero
///   interface, is unchanged).
#[inline]
pub fn chosen_interface(candidates: &[Candidate], enabled: bool) -> u32 {
    if !enabled {
        return 0;
    }
    let mut best: Option<u32> = None;
    for c in candidates {
        if c.up && c.has_source {
            best = Some(match best {
                Some(b) if b <= c.index => b,
                _ => c.index,
            });
        }
    }
    best.unwrap_or(0)
}

/// ISC-5(b): does network egress require presenting an interface capability? The
/// entirety of the second sub-switch. Disabled ⇒ `false` (ambient access, as today).
#[inline]
pub const fn egress_requires_cap(caps_enabled: bool) -> bool {
    caps_enabled
}

/// The interface-class prefix a runtime name is built from. Inferred from the
/// existing `NetDeviceIdentity` (driver/name `&'static str`) so no driver is edited
/// (the mt7601u/xHCI scope guard). WiFi-class devices become `wlanN`; everything
/// else `ethN`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum IfClass {
    Ethernet,
    Wireless,
}

impl IfClass {
    /// The name prefix (`"eth"` / `"wlan"`).
    pub const fn prefix(self) -> &'static str {
        match self {
            IfClass::Ethernet => "eth",
            IfClass::Wireless => "wlan",
        }
    }
}

/// Classify a device from its existing identity strings (no driver edits). A device
/// is wireless if its driver or name names a WiFi part/family; otherwise ethernet.
/// Pure + case-sensitive on the known lowercase identity literals in-tree.
#[inline]
pub fn classify(driver: &str, name: &str) -> IfClass {
    const WIFI_MARKERS: &[&str] = &["mt7601", "wlan", "wifi", "80211", "wireless"];
    for marker in WIFI_MARKERS {
        if contains(driver, marker) || contains(name, marker) {
            return IfClass::Wireless;
        }
    }
    IfClass::Ethernet
}

/// `str::contains` is available in core, but keep a tiny ASCII substring check here
/// so this module has no surprises under the host `#[path]` mount and stays trivially
/// auditable.
#[inline]
fn contains(haystack: &str, needle: &str) -> bool {
    let (h, n) = (haystack.as_bytes(), needle.as_bytes());
    if n.is_empty() {
        return true;
    }
    if n.len() > h.len() {
        return false;
    }
    let mut i = 0;
    while i + n.len() <= h.len() {
        if &h[i..i + n.len()] == n {
            return true;
        }
        i += 1;
    }
    false
}

/// How many sized passes [`snapshot_sized`] makes before its last one takes
/// what fits (Chad #241 1b).
pub const SNAPSHOT_PASSES: usize = 4;

/// #175 item 2 (#218 bounce): build a snapshot without allocating under the
/// table's lock. Each pass sizes a fresh `Vec` for `len()` (read under the
/// lock, then released), and `fill(out, last)` re-takes the lock and fills it
/// only if the table still fits that capacity; a table that grew in between
/// (it only appends) costs one more pass. The fresh `Vec` per pass is the
/// point: a `reserve_exact` on an empty `Vec` reserves relative to its length
/// (0), so once the table outgrew the first capacity the old retry reserved
/// nothing and spun forever.
///
/// Bounded (Chad #241 1b): the [`SNAPSHOT_PASSES`]th pass is `last`, and
/// `fill` then takes the entries that fit, the table's oldest (it only
/// appends), and succeeds. A table growing between every sizing and fill
/// therefore costs at most that many passes, and the snapshot misses only
/// interfaces registered during the call.
pub fn snapshot_sized<T>(
    mut len: impl FnMut() -> usize,
    mut fill: impl FnMut(&mut alloc::vec::Vec<T>, bool) -> bool,
) -> alloc::vec::Vec<T> {
    let mut pass = 0;
    loop {
        pass += 1;
        let mut out = alloc::vec::Vec::with_capacity(len());
        if fill(&mut out, pass >= SNAPSHOT_PASSES) {
            return out;
        }
    }
}
