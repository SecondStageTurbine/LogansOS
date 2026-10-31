<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-7.5 CB-3 (P1) — 7.5-D identity / interface wiring (the capstone) → Oracle

**Date:** 2026-06-24
**From:** PM (Claude)
**To:** Oracle (authored the 7.5-D identity design, Charter B; just closed OT-37). New branch off `origin/main` HEAD: `codex/phase-7.5-cb-3-identity-oracle`.
**Status:** DISPATCHED — a Phase-7.5 close gate. **Non-conflicting with the live bare-metal WiFi session** (Cerebro): CB-3 is a metadata registry above `NetDevice` with a **default-off kill-switch**, so it changes *nothing* observable until enabled — it cannot churn the mt7601u/xHCI subsystem under bare-metal test. **Kernel-touching ⇒ 4-shard QEMU gate + AAG before merge.**

## 0. The decision this build makes
Replace the baked interface identity (`NetDevice.name`/`driver` as `&'static str`, ambient `get_device(0)`) with a **runtime `InterfaceId` registry** where an interface is a named node addressable by a **`Handle::File` capability (I-15)** under a connection-management namespace (e.g. `/net/if/wlan0`) — Oracle's Charter-B **Option 3**. Identity/naming lands first; cap-*gating* of net access is a second, separately-default-off switch.

## 1. References
```
git fetch origin
git show origin/main:kernel/src/net/device.rs        # NetDevice { driver: &'static str, name: &'static str } (:12-13) — the baked literals to replace; register_device
git show origin/main:kernel/src/net/interface.rs     # select_source (:8) — extend RFC-6724 pick across UP interfaces, replacing get_device(0)
git show origin/main:kernel/src/net/mod.rs           # get_device / the device table
git show origin/main:docs/complexity-governance.md   # I-15 (Handle::File cap graph) — the cap CB-3 rides; the file-caps default-off kill-switch precedent
git show origin/main:kernel/src/sched/file_cap_logic.rs  # the I-15 FileRights↔CapRights mapping the interface cap reuses (zero new Handle kind)
# OT-18 E1b-derive / E3-revoke file-scope work (codex/arthur-grant-* on origin) — the grant/revoke pattern an interface cap follows
```

## 2. ISCs
- **ISC-1 — `InterfaceId` registry above `NetDevice`.** A small registry giving each device a runtime `InterfaceId` + a runtime name (`eth0`/`wlan0`), replacing the `&'static str` literals. The registry lock is a **leaf below the USB band** (within OT-13's lock-ordering ceiling) — assert it.
- **ISC-2 — Hotplug lifecycle → link-state.** USB attach/detach + WiFi assoc/disassoc update the registry's link-state for the interface (the registry is the single source of truth for "is `wlan0` up?").
- **ISC-3 — Multi-interface `select_source`.** Extend the Phase-4 RFC-6724 source pick across **all UP interfaces**, replacing the hardcoded `get_device(0)`. With identity OFF this must collapse to the exact current single-device behavior.
- **ISC-4 — The interface cap rides the frozen graph (zero new Handle kind).** An interface is addressed by a `Handle::File` cap (I-15) under `/net/if/<name>`; it **grants/revokes exactly like the OT-18 E1b-derive / E3-revoke file scopes** — no new `Handle` variant, no new cap kind. Reuse `file_cap_logic`'s mapping.
- **ISC-5 — Two-layer kill-switch (the discipline).** **(a)** `interface-identity` **default-off** ⇒ bit-for-bit `7.5-C2` (index semantics, ambient `get_device(0)`, the `&'static str` names) — an I-2-form collapse to the proven core, with an equality regression. **(b)** cap-*gating* of net access is a **second, separately default-off** sub-switch, so identity/naming can land and bake before access is ever gated. Neither switch changes observable behavior until flipped.
- **ISC-6 — standards.** SPDX; `cargo fmt --all --check`; clippy `-D warnings` (kernel blocking) via Docker. **No new cross-cutting invariant** (the registry is metadata; the cap is I-15, already in the ledger). No new `unsafe`.

## 3. Governance fault-containment (I-14)
- **(a) Lock isolation — N/A by construction:** the registry has **no ISR path** (metadata updated from the device-management/control path, not an interrupt). Assert no governance lock is taken.
- **(c) Memory isolation — N/A by construction:** **no DMA** — the registry holds names/ids/link-state, not device buffers.
- **(b) Liveness:** inherited — the registry lock is a leaf; it cannot stall governance.
State each explicitly in the RETURN.

## 4. Scope guard
Identity/naming/registry + the cap addressing + multi-interface source-pick **only**. **Do NOT**: touch the mt7601u/xHCI driver or the DHCP/wlan0 RX path (Cerebro's live bare-metal subsystem — keep out of it), change the I-15 cap graph itself, add a new `Handle` kind or invariant, or enable either kill-switch by default. With both switches off, `git`-diff-behavior is zero.

## 5. Gate + close (PM)
- **4-shard QEMU `workflow_dispatch` green** at `NEO_SMP=4` + a scenario proving (i) identity-OFF ≡ C2 bit-for-bit, (ii) identity-ON names/links + multi-interface `select_source`, (iii) the interface cap derive/revoke (mirroring `file_cap_derive_grant`). **Kernel-touching ⇒ AAG** (lenses: concurrency on the registry lock; ABI on the cap mapping; completeness on the kill-switch equality).
- **Close-gate `scripts/worker-close-gate.sh`** (rebase LAST; full set; SPDX; self-stat). RETURN green push-CI + 4-shard QEMU URLs + the I-14 (a)/(b)/(c) statements + the kill-switch-equality evidence.
- **Disk reclaim** after PM confirms merge, per CLAUDE.md.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-24-phase-7.5-cb-3-identity-oracle.md` (+ `net/device.rs`/`interface.rs`/`mod.rs`, I-15 in `complexity-governance.md`, `file_cap_logic.rs`). **Branch:** `codex/phase-7.5-cb-3-identity-oracle` off `origin/main` HEAD.
- **One-line:** **Your Charter-B Option 3 — interface identity as a runtime registry + a `Handle::File` cap, both default-off.** Build a small `InterfaceId` registry above `NetDevice` (runtime `eth0`/`wlan0` names replacing the `&'static str` literals + hotplug link-state), extend `select_source` (RFC-6724) across UP interfaces (replacing `get_device(0)`), and address an interface by a `Handle::File` cap (I-15) under `/net/if/<name>` that **grants/revokes exactly like the OT-18 file scopes — zero new Handle kind**. **Two-layer kill-switch:** `interface-identity` off ≡ 7.5-C2 bit-for-bit; cap-gating of net access is a *second* default-off sub-switch so naming lands before access is gated. **I-14:** (a)/(c) N/A by construction (no ISR, no DMA — metadata only), (b) inherited — state each. **No new invariant/Handle kind/`unsafe`.** **Scope: identity ONLY — do NOT touch the mt7601u/xHCI/DHCP-wlan0 path (Cerebro's live bare-metal subsystem).** **Gate:** 4-shard QEMU at `NEO_SMP=4` (OFF≡C2 / ON-names+links+multi-source / cap derive-revoke) + AAG. **Close-gate `scripts/worker-close-gate.sh`; RETURN green push-CI + QEMU URLs + the I-14 statements + kill-switch-equality evidence.**
