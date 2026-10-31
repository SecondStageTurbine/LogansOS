<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-047: Phase 7.5 BUILD-B — CDC-ECM USB-Ethernet → the existing `NetDevice`

**Status:** Accepted
**Date:** 2026-06-12
**Builds on:** ADR-046 (xHCI + USB core, the class-driver seam), ADR-045 (the fenced DMA arena)
**Spec:** ANSPEC §10.8 (USB Ethernet)
**Ledger:** I-14(a)/(b)/(c1) extended to the streaming (bulk/interrupt) path; no new invariant rows

## The decision

The kernel's first USB class driver is CDC-ECM, and it implements the
EXISTING `NetDevice` trait — so the whole Phase-4 network stack
(DHCP/ARP/DNS/TCP/TLS) lights up over USB with **zero new networking
code** (`kernel/src/net/` diff: empty). The gate is the wire itself: a
REAL DHCP DORA and a DNS round-trip over the bulk pipes, slirp answering
on QEMU, a physical LAN answering on the OptiPlex.

## The claim contract (the seam, completed)

BUILD-A's offer loop gains `bind`: `probe` stays a pure descriptor match;
on a match the driver receives the live `ClaimedDevice` (slot, EP0,
contexts) and either keeps it — the slot persists for the driver's
lifetime — or hands it back, in which case the device takes BUILD-A's
teardown **bit-for-bit** (DCBAA entry cleared before the backing context
drops). The kill-switch is that branch made total: `cdc_ecm::set_enabled
(false)` ⇒ probe declines ⇒ the system IS BUILD-A (gate leg: disabled
re-offer matches zero drivers; the full family is unchanged).

In scope of the same contract, BUILD-A's hub path was found returning
WITHOUT clearing DCBAA[slot] (the context dropped while the controller
still held its pointer — latent on silicon only because idle slot
contexts are never chased). Fixed: every path now clears before drop.

## What the xHCI driver gained

- **Multi-configuration enumeration:** all `bNumConfigurations` sets are
  read and parsed (the ECM function may live in any — dual-config
  RNDIS+ECM devices are selected BY CONTENT, no RNDIS code anywhere).
- **The pure descriptor walker** (`usb_parse`, host-tested): interfaces ×
  alt-settings × endpoints + the CDC Ethernet functional descriptor. Its
  byte oracle caught the first draft reading `wMaxSegmentSize` at offset
  10 instead of 8 on first run — the ADR-028 lesson demonstrated live
  (a writer-paired round-trip would have agreed with the shared mistake).
- **Configure Endpoint** (TRB 12): bulk-IN/OUT + interrupt-IN contexts in
  both CSZ layouts; per-endpoint fenced transfer rings; DCI doorbells;
  the EP-context arithmetic (DCI, type codes, FS-vs-HS periodic interval
  conversion) in the pure tier under truth tables.
- **Per-endpoint completion routing:** the bounded ISR drain routes
  Transfer Events by (slot, DCI) to hard-capped queues
  (`EP_QUEUE_CAP = 64`, drops counted) — I-14(b) extended to streaming:
  an RX flood degrades USB, never governance, and the queues are
  pre-reserved so the ISR never allocates. Unrouted events fall through
  to the BUILD-A path exactly.
- **`control_no_data` + string-descriptor reads** (SET_CONFIGURATION,
  SET_INTERFACE, class requests; a STALL is a distinct, tolerable verdict).
- **The F6 poll path made production** (`poll_events`): QEMU's qemu-xhci
  delivers no MSI (`msi_configured=false`), and BUILD-A only ever drained
  inside enumeration's wait loops — the first gate run produced a fully
  bound device whose pump saw zero frames, the poll-starvation signature.
  Class-driver service paths now drain the event ring themselves; when
  MSI does fire (the OptiPlex), the IrqSafeMutex serializes both paths.

## The class driver

`probe`: an ECM function (comm interface 02/06 + Ethernet functional
descriptor + interrupt-IN, data interface 0A whose alt setting carries
the bulk pair) anywhere in the configuration set. `bind`: MAC from the
iMACAddress string (12 UTF-16LE hex digits, refused — not guessed — when
malformed), all streaming buffers pre-allocated from the arena (failure
declines CLEANLY, before endpoints exist), then the ECM bring-up with its
classic trap explicit: **SET_INTERFACE(data, alt 1)** — alt 0 has no
endpoints, and forgetting this is the canonical dead-pipes failure (E2 in
the blind bank). MTU from `wMaxSegmentSize − 14`. Link state rides
NetworkConnection notifications with a breadcrumbed assume-up fallback
(E3: silent interrupt endpoints are common; the gate never hangs on link).

`NetDevice` is implemented frame-for-frame on the virtio-net contract:
`tx_reserve`/`tx_submit` (bulk-OUT + doorbell), `rx_take`/`rx_recycle`
(bulk-IN repost), `service` (poll + drain). The device lock is
`lock_order::USB_DEVICE` (29) — above every AI level (I-14(a): an AI
lock under it panics in the lockdep), below the controller band (calls
into xHCI nest legally).

## The gate architecture (the wire is the test)

The kernel has no production ingress pump (Phase-4 scenarios pump frames
harness-side); the gate keeps that architecture over a real wire: the
scenario pumps RX frames into the EXISTING stack seams (`arp::inject`,
`dhcpv4::inject_offer/inject_ack` — `apply_offer` auto-sends REQUEST, so
DORA is the real protocol over the real pipes) and captures UDP:53
replies for the round-trip assert. Stage A (QEMU): `usb-net` on a slirp
user netdev — lease 10.0.2.15 from slirp's real DHCP server, DNS
round-trip to 10.0.2.3, plus the kill-switch leg and the RX-flood leg
(512 synthetic events through the REAL routing path: cap held, drops
counted, and a second DNS round-trip proves the pipe alive afterwards).
Stage B (OptiPlex, blind): a physical CDC-ECM adapter, the PL's LAN
answering DORA (up to 4 DISCOVER retransmits), DNS round-trip to the
leased resolver — every E1–E7 breadcrumb captured regardless of outcome,
each fail reason naming its bank row. The per-ethertype pump census is
the E4 discriminator: zero-RX (pipes dead, E2-class) vs RX-without-DHCP
(LAN/server) vs OFFER-refused (parse) are distinct verdicts.

## Scope honored

No RNDIS (grep: protocol code absent; the word appears only in selection
comments). No NCM/WiFi/VT-d ((c2) stays deferred to 7.5-V). No external
hubs. No new networking code. Zero new `unsafe` classes (MMIO/DMA-slice
idiom only, each justified).

## Audited carries (Forge — zero CRITICAL; fixed or deferred in-build)

- **M1 (fixed in-build):** SuperSpeed bulk endpoints were programmed with
  Max Burst = 0 because the SS Endpoint Companion descriptor (type 0x30)
  was never parsed — exactly the "real silicon rejects what QEMU forgave"
  class (the Stage-A device is full-speed). The parser now attaches
  `bMaxBurst` to its endpoint, the EP context carries it ([15:8]), and the
  configure breadcrumb logs per-EP `max_burst` so a SS adapter's geometry
  is in the sink before any blind run.
- **M2 (fixed in-build):** link/service state and the gate counters moved
  from module-global statics into per-device `DeviceStats`; the assume-up
  latch moved to the `service` (writer) path and `link_state` is a pure
  read — a second adapter can no longer alias the first's link state.
- **Deferred (named):** `EP_ROUTES` is append-only — no unbind/hotplug in
  scope; a future replug build adds `remove_routes(slot)` with teardown
  (m1). The CDC union descriptor is not parsed — single-function ECM
  only; a composite ECM+ACM device needs union pairing (m4). Both are
  7.5-lane follow-on obligations, not silent gaps.

## Silicon close (Stage B) — the AX88179 episode (recorded, because it bit us)

Stage B passed on bare metal: an **AX88179** adapter over CDC-ECM pulled a
real LAN lease (<optiplex-ip>, gw .1, DNS .163) and round-tripped DNS
entirely over USB; `capture5` wire-confirmed the full DISCOVER→OFFER→
REQUEST→ACK plus the post-lease gratuitous ARP (`NEO_TEST PASS:
adapter=claimed, lease=real_lan, dns_roundtrip=ok`). But it took eight
blind iterations, and the honest record is that the chip was **innocent** —
the wall was two of *our own* bugs, not the silicon:

1. **We corrupted our own TX.** A native-ASIX 8-byte TX header, added on a
   framing hypothesis, is wrong for the ECM config (which wants raw frames
   per spec). It silently broke config-3 TX and produced a confident,
   *kernel-source-cross-validated* "the AX88179's ECM-TX is dead hardware"
   conclusion across three captures (the Linux `ax88179_178a` "bind
   vendor-only" patch read as corroboration — it is not: Linux avoids the
   ECM config for its own reasons; the chip transmits over ECM fine with the
   right init). **The cheap spec-pure insurance reversed it:** sending raw
   ECM frames and capturing *before* committing to a from-scratch native-
   ASIX driver showed the chip transmitting — saving a multi-day detour. The
   disproven header path is removed and documented in `cdc_ecm.rs`.
2. **Empty DHCP `chaddr` (a latent Phase-4 defect).** Once frames hit the
   wire, the real blocker surfaced: the DHCP client sent DISCOVERs with an
   all-zero BOOTP client-hardware-address and the broadcast flag clear. A
   real server needs `chaddr` to address its reply and silently drops the
   request; **QEMU's slirp tolerated it, which is exactly why it hid until
   bare metal.** One fix in `transmit_client_message` (fill `chaddr` from the
   client MAC, set the broadcast bit) completed the full DORA.

The legitimate kernel work that mattered: the AX88179 needs its **native MAC
TX path enabled** (full PHY power-cycle + the `link_reset` TX-FIFO
handshake) in its *vendor* config before the ECM switch — that is what makes
the chip accept and emit raw frames.

**Lesson (a textbook instance of the CLAUDE.md blind-hardware rule):** a
confident, well-sourced "it's the hardware" verdict was our own bug; the
spec-pure confirmation before a big build is what caught it. Distrust the
cross-validated one-hop conclusion exactly as the standing rule warns — and
keep the cheap "confirm before you conclude" boot in the playbook for every
"the hardware can't do X" claim.

## Evidence

- Stage A: `Plans/evidence/phase75-buildB-qemu-PASS-stageA.log`
- Stage B: `Plans/evidence/phase75-buildB-optiplex-PASS-stageB.log` (commit
  `d76e37d`) — the extracted NVMe sink from the winning bare-metal run:
  AX88179 enumerate → cfg3 ECM select-by-content → cfg1 native TX bring-up
  (`physr=0xa400`, `fifo=0`) → RAW-frame DORA → `LEASE <optiplex-ip>` + DNS
  round-trip, `fence_refusals=0`. The wire proof is on-tree with the code.
