<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# BOUNCE: eth0 rung-4 run 2 — link fixed, now `frames=0` (RX never delivers) → Chad

**Date:** 2026-08-01
**From:** PM (Claude)
**Run:** JOINT bare-metal, OptiPlex, `codex/eth0-boot-product-flow-chad` @ `977591ac` (product ISO). **Gates PM-verifiable:** push-CI `30675521889`, 4-shard `workflow_dispatch` `30675825556`.
**Result: the link-precedence fix WORKED. New, deeper failure — conclusive again.**

## Layer closed ✅
```
native-init physr=0xa400 linked=true
link notification connected=false ignored; AX88179 PHY linked   ← the fix, doing exactly its job
eth0 … link=Up
DHCP attempted 4/4                                              ← DHCP is now attempted (it never was)
```
The trap-mine was right, the fix was right, and it is confirmed on silicon. `fence_refusals=0` held (I-14(c)).

## The new failure
```
eth0: no lease … frames=0 … offer=0 ack=0 … fence_refusals=0
```
**`frames=0` is the whole diagnosis.** Not "sent DISCOVER, got no OFFER" — **zero frames received, at all**, across 4 DHCP attempts × 750k pump passes each.

**Why that's decisive:** a live LAN is never silent. Even if our DISCOVER never left the wire, background broadcast (ARP, mDNS, NetBIOS…) would arrive continuously. **Receiving literally nothing means the RX path is not delivering — not that DHCP failed.** This is an RX-arming/enable problem, not a DHCP-protocol problem, and `offer=0 ack=0` is a downstream consequence, not an independent signal.

## Leading hypothesis — AX88179 RX is never enabled
The AX88179 is a **vendor** device: bulk-IN does not stream until vendor RX control is programmed. Upstream `ax88179_178a` writes **`AX_RX_CTL`** (`AX_RX_CTL_START | AX_RX_CTL_AB | …`) during bind, plus the medium-mode setup. If our native-init brings the PHY up (which we now prove: `physr=0xa400 linked=true`) but never sets RX_CTL — or never queues bulk-IN transfers — the device is linked and silent, which is **exactly** what the serial shows.

## Required trap-mine (raw upstream only, never WebFetch)
From `drivers/net/usb/ax88179_178a.c`:
1. **What exactly does bind write to enable RX?** `AX_RX_CTL` value and bit meanings; is there a required order relative to medium-mode / PHY-up?
2. **Bulk-IN setup** — URB size, `AX_RX_BULKIN_QCTRL` / burst settings; does the device require a specific queue depth or timer config before it DMAs anything?
3. **Is there a per-frame header** the AX88179 prepends (RX header/packet-count fields) that must be parsed, such that we might be receiving bytes but discarding them as malformed — which would still show `frames=0` at our net layer?

## The discriminator to add BEFORE the next boot (capture-before-you-need-it)
`frames=0` today cannot distinguish three different failures. **Split the counter:**
- **`urb_completed`** — bulk-IN transfers that completed at the **USB** layer (did the device give us *any* bytes?)
- **`frames_parsed`** — buffers that survived AX88179 header parsing
- **`frames_demuxed`** — frames handed to the net stack

Then the next run is unambiguous: `urb_completed=0` ⇒ RX never armed (fix RX_CTL/queueing); `urb_completed>0, frames_parsed=0` ⇒ we're getting bytes and mis-parsing the vendor header; `frames_parsed>0, demuxed=0` ⇒ our demux is dropping them. Also log the **TX** side (DISCOVER bytes actually submitted + completion status) so "did our DISCOVER leave?" is answered independently rather than inferred.

## Note — stale base, correctly handled
Chad did **not** rebase, because I explicitly asked him to gate and boot **exact `977591ac`**. That was the right call and he flagged it: base `0c7efaa` vs `origin/main` `566fcd6a`. **Rebase before merge, not before the boot** — a hardware proof wants the exact gated tree.

## Relay — Chad
- **WORKTREE:** `E:\AI Automation\workers\chad` · **BRANCH:** `codex/eth0-boot-product-flow-chad` (continue)
- **Read:** `git fetch origin && git show origin/main:Plans/BOUNCE-2026-08-01-eth0-rx-frames-zero-chad.md`
- **One-line:** **Your link fix worked on silicon** — `connected=false ignored; AX88179 PHY linked` → `link=Up` → **DHCP attempted 4/4**. That layer is closed. **New failure: `frames=0` — we received nothing at all**, and a live LAN is never silent (background ARP/mDNS would arrive even if our DISCOVER never left), so **this is RX never being armed, not DHCP failing**; `offer=0 ack=0` is downstream noise. Leading hypothesis: the AX88179 is a vendor device whose bulk-IN doesn't stream until **`AX_RX_CTL`** is programmed at bind (upstream writes `AX_RX_CTL_START | AX_RX_CTL_AB | …`) — we bring the PHY up but may never enable RX or queue bulk-IN. **Trap-mine `ax88179_178a` from raw source** for the RX_CTL write + ordering, the bulk-IN/burst setup, and **whether the device prepends an RX header we must parse** (we could be receiving bytes and discarding them as malformed). **Before the next boot, split `frames=0` into `urb_completed` / `frames_parsed` / `frames_demuxed`, and log the TX submit+completion separately** — that makes the next run distinguish "RX never armed" from "bytes arrive, header mis-parsed" from "parsed but not demuxed", instead of collapsing all three into one zero. Rebase before merge, not before the boot.
- **RAG** health-check first; e.g. `ax88179 rx ctl enable bulk in`, `cdc_ecm rx urb queue submit`, `dhcpv4 demux_eth_rx frames counter`.
