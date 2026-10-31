<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase-7.5 CB-1b (P0) — the real cold DHCP lease: promote the wire-demux to production + bind the interface + virtio real-DORA gate → Cerebro

**Date:** 2026-06-23
**From:** PM (Claude)
**To:** Cerebro (owns the WiFi lane; authored CB-1 + this OBSERVE). New branch off CB-1 (`codex/phase-7.5-cb-1b-real-lease-cerebro`, stacked on `codex/phase-7.5-cb-1-wifi-link-cerebro`); **rebase onto `origin/main` after CB-1 merges.**
**Status:** DISPATCHED — **the actual "gets an IP" milestone.** Ratifies + governs Cerebro's OBSERVE: CB-1 proves the wlan0 link/DORA **composition** + ships bare-metal diagnostics, but the wire-DHCP demux lives only in a **test pump** (`classify_ipv4`, `test_harness.rs:9152`) — never wired into a production RX path or the wlan0 loop — so **a real cold lease cannot complete on hardware yet.** CB-1b is **promotion + wiring**, not a from-scratch bridge: the hard part (wire parse + client feed) is already proven. **Kernel-touching + bare-metal ⇒ carries the I-14 analysis, the blind-hardware discriminator section, and the close-gate.**

## 0. The decision this build makes
Turn "the DORA pieces compose (demonstrated via inject helpers in a test pump)" into "**a booted NeoPlutarch receives a real DHCP Offer/Ack off the wire through a production RX path and `wlan0` gets a real lease**" — proven first by a **real lease over virtio-net in QEMU** (DORA completing over the wire, **not** `inject_offer`/`inject_ack`), then the wlan0 bare-metal cold lease (the PL's run).

## 1. The OBSERVE (Cerebro) — what exists vs. what CB-1b adds
| Item | Reality (verified) | CB-1b work |
|------|--------------------|------------|
| UDP RX demux for 67/68 | exists in `classify_ipv4` (**test-only**, `test_harness.rs:9152`) | **promote to a shared production net RX-route fn** |
| parse Offer/Ack from wire | `dhcpv4::parse_message` (`:246`, done) | reuse |
| feed `inject_offer`/`inject_ack` | done in `classify_ipv4` | reuse (now driven by the production demux) |
| bind DHCP client to wlan0 index | `transmit_client_message(interface_id)` (`:328`) selects by id; client hardcodes 0 | **add `set_interface_id`, set it to wlan0's registered index** |
| test virtio first → wlan0 | `virtio_net` is in QEMU | **a real-wire virtio DORA scenario (lease, not inject)** = the QEMU "gets IP" proof |
| breadcrumb `offer_rx→request_tx→ack_rx→ip=` | — | wire into the shared demux + the wlan0 loop |

## 2. References
```
git fetch origin
git show origin/main:kernel/src/test_harness.rs           # classify_ipv4 (:9152) — the proven wire IPv4/UDP demux to PROMOTE (dst-68 → parse_message → inject_*)
git show origin/main:kernel/src/net/dhcpv4.rs             # parse_message (:246), inject_offer/ack (:212/:217), transmit_client_message(interface_id) (:328)
git show origin/main:kernel/src/drivers/usb/mt7601u/mod.rs # wlan0 NetDevice registration (:2774/:3114) — the registered interface index to bind
git show origin/main:kernel/src/net/smoltcp_wrap.rs       # the receive() path — where the production RX-route hooks
git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-cb-1-wifi-link-cerebro.md  # CB-1 (the foundation this stacks on)
```

## 3. ISCs
- **ISC-1 — Promote the demux to a production RX route.** Lift `classify_ipv4`'s logic (parse IPv4/UDP, dst-port 68 → `dhcpv4::parse_message` → `inject_offer`/`inject_ack`) into a **shared production net RX-route function** the wlan0 RX loop (and virtio) drives. The test pump keeps calling it (no behavior change for existing scenarios). This is the single load-bearing change — real RX frames now reach the DHCP client SM.
- **ISC-2 — Bind the DHCP client to the interface.** Add `set_interface_id` to the client; set it to **wlan0's registered NetDevice index** so `transmit_client_message` TXes on the right device (no more hardcoded 0). Multi-interface-safe: the lease runs on the bound interface.
- **ISC-3 — The real-lease virtio QEMU gate (the strengthening over CB-1).** A QEMU scenario where DORA **completes over the wire via virtio-net** — a *real* Offer/Ack received through the production RX route (**not** `inject_*`), ending in a real lease, asserting `ip=…`. At `NEO_SMP=4`, hermetic. **This is the gate CB-1 lacked** (CB-1's path used inject helpers); it is a genuinely stronger proof.
- **ISC-4 — Breadcrumbs + the wlan0 bare-metal cold lease (the PL's run).** Wire `offer_rx → request_tx → ack_rx → ip=…` into the shared demux + the wlan0 loop so the bare-metal cold lease is self-diagnosing.
- **ISC-5 — Kill-switch / regression.** The production RX route ships such that **the existing scenarios remain bit-for-bit** (the test pump path unchanged); if the promotion changes any existing default, gate it. No new `unsafe`/invariant.
- **ISC-6 — standards.** SPDX; `cargo fmt --all --check`; clippy `-D warnings` (kernel blocking) via Docker.

## 4. Governance fault-containment (I-14)
- **(a) Lock isolation.** The new production RX-route fn acquires **no** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`) — it parses a frame and feeds the DHCP client SM. Grep-assert it.
- **(b) Liveness degradation.** An RX flood on the demux cannot stall governance: the residency daemon degrades to the P4b synchronous backstop (I-2 kill-switch).
- **(c) Memory isolation.** No new device-initiated DMA — the RX frames arrive via the **already-fenced** wlan0/virtio RX path (W1/W2 + CB-1 under VT-d); the demux operates on already-received buffers. State this; no unfenced DMA added.

## 5. Blind-hardware failure-mode + discriminator (the wlan0 bare-metal cold lease)
ISC-4 breadcrumbs only; conclusive on pass AND fail:
- **No `offer_rx`** ⇒ the production demux isn't seeing broadcast frames ⇒ is the CB-1 broadcast-RX filter (T-047) actually up on silicon? (this is where CB-1's untested-on-silicon T-046/T-047 surface). Discriminator: dump RX-filter reg + frames-seen count.
- **`offer_rx` but no `request_tx`** ⇒ client didn't advance / wrong interface bind (ISC-2) ⇒ log the bound `interface_id` vs wlan0's index.
- **`request_tx` but no `ack_rx`** ⇒ TX went on the wrong device or the AP didn't answer ⇒ log the TX device index + the `chaddr`.
- **`ack_rx` but no `ip=`** ⇒ lease parse/apply failed ⇒ dump the parsed lease fields.
- **inject-vs-real confusion** ⇒ assert the QEMU gate path never calls `inject_*` (real wire only) so a green gate can't mean "inject worked."

## 6. Scope guard
The real cold lease **only** — promote the demux + bind the interface + the real-DORA gate. **Do NOT**: re-architect the net stack, change `dhcpv4` parse/SM internals (reuse), touch the CB-1 GTK/CCMP path, the survey/xHCI (CB-2), WPA3 (CB-5), or add a new invariant. Stacked on CB-1; **rebase onto `origin/main` after CB-1 merges** (close-gate rebase discipline).

## 7. Gate + close (PM)
- **Real-lease virtio QEMU scenario green (ISC-3)** at `NEO_SMP=4` — DORA over the wire, real Offer/Ack, `ip=…`, **no `inject_*`**. **Kernel-touching ⇒ 4-shard QEMU `workflow_dispatch` mandatory.**
- **Bare-metal wlan0 cold lease = the PL's run**, made self-diagnosing by ISC-4 + §5.
- **Close-gate: `scripts/worker-close-gate.sh`** (rebase onto current main LAST — after CB-1 lands; full set; SPDX; self-stat). RETURN the green **push-CI + 4-shard QEMU URLs** + the QEMU real-lease `ip=…` line + confirmation the gate path takes no `inject_*`.
- **Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-23-phase-7.5-cb-1b-real-lease-cerebro.md`. **Branch:** `codex/phase-7.5-cb-1b-real-lease-cerebro` off CB-1; **rebase onto `origin/main` after CB-1 merges.**
- **One-line:** **The real "gets an IP" milestone — promote, don't rebuild.** The wire-DHCP demux is proven but trapped in the test pump (`classify_ipv4`): **(1)** lift it into a **shared production net RX-route fn** the wlan0/virtio RX loop drives (test pump keeps using it — existing scenarios bit-for-bit); **(2)** add `set_interface_id`, bind the DHCP client to **wlan0's registered index** (no hardcoded 0); **(3)** a **real-lease virtio QEMU gate** — DORA completes over the wire (real Offer/Ack, **NOT `inject_*`**) → `ip=…` (the gate CB-1 lacked, a stronger proof); **(4)** breadcrumbs `offer_rx→request_tx→ack_rx→ip=` so the wlan0 bare-metal cold lease self-localizes (this is where CB-1's untested-on-silicon broadcast-RX T-046/T-047 surface). **I-14:** the RX-route fn takes no governance lock; no new DMA (frames arrive via the already-fenced wlan0/virtio path). **Scope: real lease ONLY** — no net-stack re-arch, no `dhcpv4`-internals change, no CB-1/CB-2/CB-5 overlap, no new invariant. **Close-gate `scripts/worker-close-gate.sh` (rebase onto main AFTER CB-1 merges); RETURN green push-CI + 4-shard QEMU URLs + the real-lease `ip=…` + "gate path takes no inject_*".**
