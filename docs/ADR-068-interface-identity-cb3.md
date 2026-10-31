<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-068: Interface identity — a runtime InterfaceId registry + a `/net/if/<name>` capability (Phase 7.5 CB-3, the 7.5-D layer)

**Status:** Accepted
**Date:** 2026-06-24
**Dispatch:** `Plans/DISPATCH-2026-06-24-phase-7.5-cb-3-identity-oracle.md`
**Design context:** Oracle's Charter-B **Option 3** (interface identity as a runtime registry + a `Handle::File` cap, both default-off)
**Scope:** Replace the baked interface identity (`NetDeviceIdentity.name`/`driver` `&'static str`, ambient `get_device(0)`) with a runtime `InterfaceId` registry above `NetDevice` — runtime `eth0`/`wlan0` names + hotplug link-state, multi-interface RFC-6724 `select_source`, and a `/net/if/<name>` `Handle::File` capability — under a **two-layer default-off kill-switch**. Does **not** touch the mt7601u/xHCI driver or the DHCP/wlan0 RX path (Cerebro's live bare-metal subsystem), change the I-15 cap graph, add a new `Handle` kind or invariant, or enable either switch by default. No new `unsafe`.

## Problem

The Phase-4 net stack treats "interface 0" as **the** interface in two unrelated places, and names are compile-time literals:

- **Identity is baked.** `NetDeviceIdentity { driver: &'static str, name: &'static str }` — every device's name is a fixed literal (`"virtio-net0"`), with no runtime, per-device, hotplug-aware naming.
- **Selection is hardcoded.** `interface::select_source` calls `ipv4::preferred_address(0)` / `ipv6::preferred_global_address(0)`, and the egress path (`socket.rs`/`tcp.rs`) independently calls `net::get_device(0)` and passes literal `0` to `arp`/`ndp`. The address-layer `INTERFACES` tables and the device registry both key on an `interface_id`/index that is only ever `0` — there is no binding between them and no multi-interface routing.
- **No interface is addressable as an object.** There is no way to name, gate, or delegate access to a specific interface; a connection-management layer (and the eventual user-facing "which network am I on?" surface) needs interfaces to be first-class, capability-addressable nodes.

## Decision

Add a thin **runtime identity layer above `NetDevice`**, entirely behind a default-off kill-switch, in four parts:

1. **The `InterfaceId` registry (`net/identity.rs`, ISC-1/2).** A leaf-locked `Vec` keyed by the device-registry index, populated from the generic `drivers::net::register_device` (no driver edits). Each entry carries a runtime `id`, a runtime name (`<class><ordinal>` — `eth0`, `wlan0` — with the class inferred from the existing identity strings, so no driver is touched), and a hotplug-tracked `LinkState`. The registry is the **single source of truth** for "is `wlan0` up?"; its link-state is refreshed from the generic `NetDevice::link_state()` in `net::tick` (so assoc/disassoc/attach/detach — already maintained inside each driver — reach it without editing any driver). The **binding invariant** is established and asserted by every consumer: *registry index `i` ≡ `ipv4`/`ipv6` `interface_id` `i` ≡ `get_device(i)`*.

2. **Multi-interface `select_source` (`net/interface.rs`, ISC-3).** `select_source` picks across **all UP interfaces** — the lowest-index interface that is UP (per the registry) and has a usable source for the destination family — instead of hardcoding interface 0. The egress sites replace `get_device(0)` / `submit_or_queue(0, …)` with `egress_interface_for_source(src)`, the reverse-lookup of the interface that owns the selected source. The cross-interface decision is a pure function (`identity_logic::chosen_interface`), host-KAT'd.

3. **The `/net/if/<name>` capability (`fs/mod.rs`, ISC-4).** A synthetic `NetIfFs` mounted as the root-child `net`; `lookup("if/<name>")` resolves to a small `InterfaceInode` (kind `Regular`, reads the `<name> <link>` status). Opening `/net/if/wlan0` via the **unchanged** `sys_open` path mints a `Handle::File` capability that enters the I-15 derivation/grant graph exactly like any file cap — **zero new `Handle` kind, zero new syscall, zero change to `derive_handle`/`revoke_handle`**. (A *new `Handle` variant* was explicitly rejected: it would force edits to every exhaustive match and add cap-graph surface for no benefit; a `Handle::File` over a synthetic inode reuses the entire OT-18 file-cap machinery.)

4. **The two-layer kill-switch (`net/identity_logic.rs`, ISC-5).** `interface-identity` (default-off) gates everything above; with it off, `chosen_interface` is the constant `0`, the registry is inert, and `/net` is never mounted — so the net stack is bit-for-bit **7.5-C2** (index semantics, ambient `get_device(0)`, the `&'static str` names), an I-2-form collapse to the proven core. `interface-caps` (default-off, depends on `interface-identity`) is a **second** sub-switch gating whether egress *requires* presenting an interface cap (`egress_requires_cap`) — so identity/naming lands and bakes before access is ever gated. Neither switch changes observable behavior until flipped.

### Why a `Handle::File` and not a new cap kind

The interface is addressed by `sys_open("/net/if/<name>")`, which already wraps the returned `Arc<OpenFile>` in `Handle::File` and inserts the single `(cap_id, parent=None)` derivation-graph leaf that **every** file open inserts. Because `Handle::File` is already a first-class cap (I-15, ADR-061), `sys_derive`/`sys_grant`/`sys_revoke` work on the interface cap with no new generic code. This is the literal reading of the dispatch's "zero new Handle kind" and the minimal-surface choice.

## The kill-switch and the equality proof (Rule 1)

`interface-identity` is read once as `INTERFACE_IDENTITY_ENABLED = cfg!(feature = "interface-identity")` and threaded as an explicit `enabled: bool` into the pure logic, so the host KAT (`tools/interface-identity-logic-tests`) proves **disabled ≡ interface-0-only (7.5-C2), bit-for-bit** by sweeping every `(up, has_source)` configuration of four interfaces (the exhaustive `4^4` sweep), plus a contrapositive proving the enabled path genuinely differs (eth0 down ⇒ eth1 wins). The QEMU `interface_identity_select` scenario re-asserts the ON behavior (names, registry link-state as source of truth, multi-interface pick) and the in-kernel mirror of the disabled collapse; `interface_cap_derive_revoke` proves the cap (open `/net/if/eth0` → derive an attenuated read-only child → revoke the parent → the descendant dies).

## Governance (I-5 / I-14) — no new invariant, no new ordered lock

- **No new invariant.** The registry is metadata; the cap is I-15 (already in the ledger). The §1 page-fit alarm is RAISED (it blocks I-16 until the I-14/I-15 prose is compressed) — CB-3 preserves all 15 rows (Rule 2 default).
- **I-5 (lock order) unchanged.** The registry lock is a **plain leaf `SwitchMutex`** (the VT-d-state-lock precedent), NOT `new_ordered` — it adds zero machine-checked ordering call sites, the ordered band stays at 13 sites / max level 31 (`USB_CORE`). It is taken **alone** (snapshot-then-release) so it never nests under the device-table or ipv4/ipv6 table locks.
- **I-14(a) lock isolation — N/A by construction.** The registry has no ISR path (it is touched only from the device-management/control path and `net::tick`) and imports no AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/the reservation/`TABLE`) — grep-checkable.
- **I-14(c) memory isolation — N/A by construction.** The registry holds names/ids/link-state, not device buffers; no DMA.
- **I-14(b) liveness — inherited.** The registry lock is a leaf; it cannot stall governance.

## Consequences

- The net stack gains runtime interface identity, multi-interface source selection, and capability-addressable interfaces, with the entire surface provably switch-off-able to the proven single-device core.
- **Deferred (flagged, not silently assumed):** full cross-interface RFC-6724 longest-prefix-match source ordering (today: lowest-index UP-with-source), IPv6 RA-per-interface population (`ipv6::inject_router_advertisement` still writes only table 0, so non-zero IPv6 interfaces have no global address yet), and administrative interface up/down via the cap (the `InterfaceInode` is read-only). None are needed for the single-device equality gate or the multi-interface (IPv4) selection proof.
- The `interface-caps` enforcement is wired as a default-off decision point (`egress_requires_cap`); the actual gating of egress on cap-possession is intentionally left to bake behind that second switch.

## References

- `kernel/src/net/identity.rs` (the registry), `kernel/src/net/identity_logic.rs` (the pure kill-switch + selection logic), `kernel/src/net/interface.rs` (multi-interface `select_source` + `egress_interface_for_source`), `kernel/src/fs/mod.rs` (`NetIfFs`/`InterfaceInode`/`mount_net_interface_fs`).
- `tools/interface-identity-logic-tests` (the exhaustive OFF==C2 equality KAT).
- ADR-061 (I-15, the file-cap graph the interface cap rides); ANSPEC §10.12.
- `docs/complexity-governance.md` audit-log entry (2026-06-24).
