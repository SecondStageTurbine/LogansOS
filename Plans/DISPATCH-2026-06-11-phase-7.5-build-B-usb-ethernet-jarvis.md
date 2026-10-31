<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7.5 BUILD-B — USB-Ethernet (CDC-ECM → the Phase-4 stack over silicon) → Jarvis

**Date:** 2026-06-11
**From:** PM/Cerebro
**To:** Jarvis (new branch off local `main` `7741e75`: `codex/phase-7.5-build-B-usb-ethernet-jarvis`)
**Status:** DISPATCHED. **QEMU-first (`usb-net`), then OptiPlex blind with a real CDC-ECM adapter. Both standing rules apply (I-14 analysis + blind-hardware section below).**
**Builds on:** 7.5-A (`12b64de`, the USB core + the `UsbClassDriver` probe seam + `FencedDmaAddr`, ADR-046) + 7.5-0 (the arena) + the Phase-4 network stack (`NetDevice` trait `net/device.rs:63`; `interface.rs`/`dhcpv4.rs`/`arp.rs`/`dns.rs`/`tcp`/`tls` — all device-agnostic, all waiting behind `NetDevice`).
**Silicon lesson bank (from 7.5-A, carry-in):** xECP-at-0x8000 window sizing, FS Evaluate Context, real-MSI-vs-poll, PLS discrimination.

## 0. The decision this build makes
A **CDC-ECM class driver** binds to a USB-Ethernet adapter through 7.5-A's `UsbClassDriver` seam and **implements the existing `NetDevice` trait** — at which point the *entire Phase-4 control plane lights up over USB on real silicon*: DHCP gets a lease, ARP resolves, DNS resolves, TCP/TLS connect — **no new networking code, only the driver binding.** This is the payoff of Phase 4 having built the stack device-agnostic: 7.5-B is a *driver*, not a network stack. The gate is an **end-to-end exchange over the real adapter** (DHCP lease + at minimum an ARP/ping or UDP round-trip), QEMU first, then the OptiPlex blind.

## 1. Scope
- **ISC-1 — The CDC-ECM class driver + probe.** A `UsbClassDriver` whose `probe` matches the **CDC-ECM** function (comm class `0x02` / subclass `0x06` ECM, with the CDC Union + Ethernet Networking functional descriptors). On match: claim the comm + data interfaces, parse the ECM functional descriptor (MAC via the `iMACAddress` string descriptor; `wMaxSegmentSize` → MTU), locate the bulk IN/OUT + interrupt endpoints.
- **ISC-2 — `NetDevice` implementation.** `mac_address` (parsed), `mtu` (ECM descriptor), `link_state` (driven by the interrupt-endpoint `NETWORK_CONNECTION`/`CONNECTION_SPEED_CHANGE` notifications), `tx_reserve`/`tx_submit` (Ethernet frame → bulk OUT; **ECM framing is raw frames, no header**), `rx_take`/`rx_recycle` (bulk IN → frame), `service` (poll/drain). Register the device so `net::` picks it up.
- **ISC-3 — The ECM bring-up sequence (the gotchas, pre-named).** `SET_CONFIGURATION`; **`SET_INTERFACE` to the data interface's *alt 1*** (ECM's alt 0 has no endpoints — frames will not flow until alt 1 is selected: E4); `SET_ETHERNET_PACKET_FILTER` (PROMISCUOUS/DIRECTED/BROADCAST) so the device passes frames up (E5); arm the interrupt endpoint for link notifications before expecting traffic.
- **ISC-4 — DMA stays fenced (I-14(c1) continues onto the data path).** Every bulk transfer buffer is arena-allocated; every address programmed into a transfer is a `FencedDmaAddr` — the data path inherits 7.5-A's structural fence (grep-assertable: no raw `PhysAddr` on the USB-Ether programming path). This is the first build where the fence guards *high-rate streaming* DMA, not just control descriptors — assert the fence counters stay at 0 refusals across a sustained transfer.
- **ISC-5 — Kill-switch (Rule 1).** No adapter present / driver compiled-out ⇒ the system + the existing net stack (virtio-net) are bit-for-bit today. The CDC-ECM `NetDevice` is purely additive: virtio-net behavior unchanged.
- **ISC-6 — Standards.** SPDX; new `unsafe` confined to the existing MMIO/USB-transfer classes (justify each); `cargo fmt`; host suite (ECM descriptor parse + framing logic in the pure tier where expressible + the Phase-4 net regressions unchanged) green; `shard-coverage`; `clippy -D warnings`; builds on+off. **ANSPEC §10.7 (USB-Ethernet)** + **ADR-047**. (Note for the record, not this build: §10.5 is duplicated in the spec — flag for the next Rule-4 consolidation.)
- **RNDIS:** **out of scope as a fallback for now** — CDC-ECM is the standard-class path and the cleanest gate; RNDIS (phone-tether/Windows protocol, message-framed) is a labelled follow-on if the target hardware needs it. Do not build both.

## 2. I-14 governance fault-containment analysis (standing rule)
- **(a) Lock isolation:** the bulk-completion / interrupt-endpoint path touches only USB+net state — **no** AI-governance lock on any ISR/completion path (grep + runtime assertion; new locks enter `sync::lock_order`).
- **(b) Liveness:** a flooded RX bulk endpoint (a chatty or hostile link) is the I-14(b) case — **cap RX frames processed per service pass + defer**; the gate includes a sustained-RX-flood scenario asserting the governance family stays green (the residency daemon's direct-reclaim backstop unaffected).
- **(c1):** ISC-4 (fenced streaming DMA). **(c2):** out of scope (7.5-V) — a USB-Ethernet adapter is low-(c2)-risk (no large opaque firmware blob); the firmware-(c2) concern remains WiFi's, gated by VT-d before 7.5-C1.

## 3. Blind-hardware failure-mode + discriminator section (standing rule)
QEMU (`-netdev user -device usb-net,...` in CDC-ECM mode) green is the **entry gate** for every bare-metal iteration. **Hardware prerequisite (state to the PL):** the OptiPlex bare-metal run needs a **CDC-ECM-class** USB-Ethernet adapter on a root port — *most cheap dongles are vendor-specific (ASIX AX88179, Realtek RTL8153) and will NOT bind CDC-ECM*; confirm the adapter's class before the run (USB-C docks / Apple / some Plugable models are ECM). A non-ECM adapter is a **conclusive** outcome, not a failure (E1). Discriminators ship in-build:

| # | Failure mode | Discriminator (observation → cause) |
|---|---|---|
| E1 | **Vendor-specific adapter** (not CDC-ECM/RNDIS) | `probe` declines and logs class/subclass/protocol + VID/PID → "unsupported vendor-specific class; need CDC-ECM" — conclusive, no mis-bind, no hang |
| E2 | **CDC functional-descriptor parse** (Union/ECM layout varies) | dump the raw config-descriptor bytes on any parse miss → off-by-one vs genuinely-absent |
| E3 | **MAC extraction** (`iMACAddress` UTF-16 hex string) | log the raw string descriptor + parsed MAC → string-index vs decode |
| E4 | **Alt-setting not selected** — endpoints found, zero traffic | breadcrumb the `SET_INTERFACE alt=1` result; "endpoints present + 0 bulk completions" ⇒ alt-setting, the classic ECM trap |
| E5 | **Link never comes up** — no frames despite alt 1 | interrupt-endpoint notification log + `SET_ETHERNET_PACKET_FILTER` result; "link-down-forever" ⇒ packet-filter/notification |
| E6 | **Bulk framing / ZLP / short packets** | per-frame length + ZLP breadcrumbs; malformed egress vs truncated ingress separated |
| E7 | **DHCP doesn't complete on the wire** | tie into the Phase-4 net counters: TX-not-egressing vs RX-not-arriving vs frame-malformed are three distinct breadcrumbs (DISCOVER sent? OFFER seen? at L2 or L3?) |

Each bare-metal run conclusive pass **or** fail per this table.

## 4. Gate (two-stage)
- **Stage A (QEMU):** `usb-net` (CDC-ECM) enumerates + binds; **DHCP obtains a lease, ARP resolves, a UDP or ping round-trip completes** over the emulated adapter; the RX-flood I-14(b) scenario; kill-switch equivalence (virtio-net unchanged); the `FencedDmaAddr`-only grep assertion; family green.
- **Stage B (OptiPlex, blind):** with a confirmed CDC-ECM adapter on a root port — enumerate, bind, **DHCP lease on the real LAN + an ARP/ping or UDP round-trip**, fence at 0 refusals across the transfer; all E1–E7 breadcrumbs captured. Iterate per the Phase-6 loop; each iteration conclusive.

## 5. Scope guard
CDC-ECM USB-Ethernet only. **No** RNDIS (labelled follow-on), no vendor-specific chip drivers (ASIX/Realtek — separate builds if ever needed), no new networking/control-plane code (the Phase-4 stack is consumed, not modified), no VT-d (7.5-V), no WiFi. If `NetDevice` or the arena can't express something the driver needs, BLOCKER to PM — don't widen them in-flight.

## 6. Merge (PM, on Stage-B PASS)
Close-gate: rebase onto current `main`, re-run Stage-A, push. `pm-merge-check` → `--ff-only` → `main`. Then **7.5-V: VT-d/IOMMU** (the I-14(c2) guard — DMAR/DRHD parse, per-device domains for connectivity devices, identity domains for the proven substrate), sequenced **before** `iwlwifi` so the WiFi firmware blob meets hardware DMA containment. With 7.5-B, LogansOS has a *second, real* network path — and the Phase-4 stack is proven device-agnostic on hardware it never saw when it was written.

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, local `main`):** `Plans/DISPATCH-2026-06-11-phase-7.5-build-B-usb-ethernet-jarvis.md`
- **THINK:** `Plans/THINK-2026-06-11-phase-7.5-opening-connectivity-lane.md` · **USB core:** ADR-046 · **arena:** ADR-045
- **Branch:** `codex/phase-7.5-build-B-usb-ethernet-jarvis` off local `main` `7741e75`.
- **One-line:** A CDC-ECM class driver on 7.5-A's `UsbClassDriver` seam that implements the existing `NetDevice` trait — so the whole Phase-4 stack (DHCP/ARP/DNS/TCP/TLS) lights up over USB with zero new networking code. Parse the ECM functional descriptors (MAC, MTU), select **data-interface alt 1** (E4 — the classic ECM trap), `SET_ETHERNET_PACKET_FILTER` (E5), drive `link_state` off the interrupt endpoint, bulk IN/OUT for frames (raw ECM framing). DMA stays fenced — `FencedDmaAddr` on the streaming data path, 0 refusals asserted. I-14: no AI lock in the completion path; RX-flood cap + governance-green scenario; (c1) fenced, (c2) deferred. Blind bank E1–E7 in-build (esp. E1: a vendor-specific dongle declines cleanly — **PL must supply a CDC-ECM adapter** for Stage B; ASIX/Realtek won't bind). Gate = DHCP lease + round-trip over the real adapter, QEMU-first. Kill-switch: no adapter ≡ today; virtio-net untouched. ANSPEC §10.7 + ADR-047. No RNDIS, no VT-d, no WiFi.
