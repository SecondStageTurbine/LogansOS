<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: eth0 at boot — turn the proven wired stack into a PRODUCT flow → Chad

**Date:** 2026-07-29
**From:** PM (Claude)
**To:** Chad (Codex, main-tier — free after the Slice-4 rebase/re-gate)
**Status:** DISPATCHED — **product lane, highest value-per-risk on the board.** No new driver work: every layer already exists, is merged, and is **silicon-proven**. This is *wiring*, not bring-up.

## Why this is the cheapest big win we have
`kernel/src/drivers/usb/cdc_ecm.rs` (844 L, ADR-047 Accepted, **merged**) implements CDC-ECM **plus** a real AX88179/178A ASIX vendor bring-up over the merged xHCI + DHCP + ARP + DNS + IPv4/6 + TCP + TLS stack. **Proven on real hardware** (README §Status, 2026-06-13: real DHCP lease `<optiplex-ip>` + DNS over a USB-Ethernet dongle on the OptiPlex 7080, "zero new networking code").

**But it is not a product.** `usb-host` is a **default-off** kernel feature (`kernel/Cargo.toml` — `default = ["kernel-diagnostic","accel-sim","inference-preemptible"]`), and USB-Ethernet is reachable only through *test scenarios* (`neo-test-usb-ethernet{,-baremetal}`). There is **no boot-time "bring up eth0 and get a lease" flow** in `kernel/src/main.rs` / `kernel/src/init/`. So the OS that can demonstrably get on the network in a test cannot get on the network when a user boots it.

**Consequence of fixing this: WiFi stops being a release blocker and becomes a feature.** ADR-017 §D3-C already ratifies wired posture as the supported path for users without WiFi.

## Scope
1. **Boot-time network bring-up as a product path.** On boot (behind the kill-switch below), enumerate USB, bind a CDC-ECM/AX88179 device if present, register `eth0` (ADR-068 identity), run DHCPv4 to a lease, and make the result available to userland — the same sequence the proven scenario performs, moved out of test-only wiring into the init path.
2. **Absent/failed hardware must degrade, never block boot.** No dongle, no link, or no DHCP answer ⇒ the OS boots normally with `eth0` down and a clear status line. **Boot must never hang or panic on a networking failure** — a user with no dongle is the common case.
3. **Bounded, observable, honest.** Bounded DHCP attempts (no infinite retry), a status breadcrumb on serial *and* on the boot frame (`link up ip=<a.b.c.d>` / `no link` / `no device`), and no silent partial state.
4. **Feature posture.** Decide and justify the default: the goal is that a *product* build has networking on by default. If flipping `usb-host` on by default has cost (boot time, code size, risk), state it and propose the product feature-set explicitly (e.g. a `product` feature that composes `usb-host` + the boot flow) rather than silently changing `default`.

## Standing rules
- **Kill-switch (mandatory):** boot-time network bring-up behind a feature/flag; **OFF ≡ the current boot path, bit-for-bit** (regression proving equality). This is the governance requirement *and* the safety property for bare metal.
- **I-14 fault containment (connectivity dispatch rule — required section):** (a) the driver/ISR path takes **no** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`) — assert it; (b) a driver stall must degrade liveness, never stall governance — show boot completing with the NIC wedged; (c) **every DMA buffer through `dma_arena::fence(...)`** under IOMMU containment — `fence_refusals=0`. Unfenced connectivity DMA remains forbidden.
- **No new invariant** expected; if boot-order changes require one, name it and ship its guard.

## Discriminators (the bare-metal run must be conclusive either way)
| Observation | Meaning | Discriminator |
|---|---|---|
| `eth0` up + nonzero IPv4 lease | product flow works | the DORA chain + `link up ip=` on serial *and* boot frame |
| No dongle attached | the common user case | boots clean, `no device`, no hang, no panic |
| Dongle present, no DHCP server | degraded | bounded attempts then `no lease`, boot completes |
| Boot hangs / panics | **regression — unacceptable** | the kill-switch OFF path must still boot |
| `fence_refusals>0` | I-14(c) breach | governance containment signal, not a NIC bug |

## Close-gate + merge
`scripts/worker-close-gate.sh` (rebase-fresh), green push-CI, **4-shard `workflow_dispatch` QEMU gate** on the exact SHA (kernel/init-touching; `virtio-net` covers the QEMU side). **Stagger the gate** — C1.1 is release-gating and DW-1 is also queued; coordinate before firing (~3 runners). The **product proof is a bare-metal boot on the OptiPlex with the USB-Ethernet dongle** (JOINT, per `docs/runbook-baremetal-arthur-optiplex.md`) showing a real lease at boot with no test scenario involved. **No auto-merge** — PM verify + I-14 containment AAG. Branch: `codex/eth0-boot-product-flow-chad`. Disk-reclaim after merge.

## Relay — Chad
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-29-eth0-boot-product-flow-chad.md`, plus `…:Plans/LEDGER-2026-07-29-logansos-release-ledger.md` (why this is the cheapest big win) and `…:docs/ADR-047-phase-7.5-usb-ethernet-cdc-ecm.md`.
- **One-line:** **Turn our proven wired networking into a product flow.** CDC-ECM + AX88179 (`kernel/src/drivers/usb/cdc_ecm.rs`) over xHCI/DHCP/DNS/TCP/TLS is **merged and silicon-proven** (real lease `<optiplex-ip>` on the OptiPlex, 2026-06-13) — but `usb-host` is **default-off** and it only runs inside *test scenarios*, so a user who boots the OS has no network. Wire boot-time bring-up into the init path: enumerate USB → bind CDC-ECM/AX88179 → register `eth0` → DHCPv4 → lease, with a status breadcrumb on serial and the boot frame. **Hard requirement: no dongle / no link / no DHCP must degrade gracefully — never hang, never panic** (booting with no dongle is the common case). Propose the product feature posture explicitly (e.g. a `product` feature composing `usb-host` + boot flow) rather than silently flipping `default`. Kill-switch OFF ≡ current boot path bit-for-bit; I-14 analysis required (no governance lock on the ISR path, `dma_arena::fence` on every buffer, `fence_refusals=0`). Close-gate + 4-shard QEMU (**stagger** — C1.1 and DW-1 are queued); the real proof is a JOINT bare-metal OptiPlex boot getting a lease with no test scenario. **This makes WiFi a feature instead of a release blocker.**
- **RAG** (PowerShell) health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `cdc_ecm bind NetDevice register eth0`, `dhcpv4 discover offer request ack lease`, `kernel main init boot sequence usb host enumerate`, `dma_arena fence usb dma buffer`. If down, note it in the RETURN + grep/Read (PL owns RAG startup — never a blocker).
