<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7.5 opening — the connectivity lane (DMA isolation first, then the drivers)

**Date:** 2026-06-11
**Seat:** PM/Cerebro (deep think + prior art; phase-opening pass)
**Context:** Phase 7 CLOSED at `db88652` (`Plans/CLOSE-REPORT-2026-06-11-phase-7.md`). Phase 7.5 is the **committed** connectivity lane: WiFi (Intel `iwlwifi`, WPA2/WPA3 client, Article 9.10 / ADR-017), the USB host stack (Articles 9.7/9.9), and USB-Ethernet (ADR-017 §D3) — the latter two re-deferred from Phase 4 by PL decision 2026-05-30. The lane opens under the **I-14 fault-containment mandate** (PL decision 2026-06-10): every dispatch carries a governance fault-containment analysis, and **no connectivity driver may be granted unfenced DMA** until the DMA-isolation strategy is built. This THINK makes that strategy decision — task one — and slices the lane.
**Related:** `docs/complexity-governance.md` I-14 (+ the CLAUDE.md 7.5 standing rule); ADR-017 (WiFi strategy); `kernel/src/drivers/block/dma.rs` (`DmaBuffer` — the single existing DMA abstraction); `kernel/src/net/device.rs` (`NetDevice` — the Phase-4 trait new drivers implement); the Phase-4 control plane (`interface.rs` RFC-6724 `select_source`, `dhcpv4.rs`, `arp.rs`, `dns.rs`).

---

## 1. Task one — the DMA-isolation fork (I-14(c))

### The threat model, split honestly into two tiers
I-14(c) says a driver's DMA must not reach governance or tensor memory. That sentence hides **two different adversaries**:

- **(c1) The errant driver** — *our* code programs a bad descriptor (wrong address, stale buffer, off-by-one ring index) and the device faithfully DMAs where we mistakenly told it to. This is the **realistic, near-certain** failure class for a from-scratch xHCI/NIC bring-up — it is the DMA analogue of the blind-hardware base rate ("the next run reveals the next layer").
- **(c2) The misbehaving device** — the device itself (or its firmware) issues DMA to addresses *we never programmed*: a buggy controller, or a large proprietary firmware blob acting badly. **Software cannot stop (c2)** — a bus-mastering device can write any physical address it likes; only an IOMMU physically blocks it. This tier is acute for exactly one 7.5 deliverable: `iwlwifi` runs a **multi-megabyte closed firmware blob with its own DMA engines**. USB controllers are lower-risk (simple, well-understood silicon; no gigantic vendor blob between us and the bus).

### Option A — VT-d/IOMMU first (hardware enforcement before any driver)
Build the full DMA-remapping stack now: DMAR ACPI parse → DRHD units → context entries + second-level page tables → per-device domains → fault reporting ([Intel VT-d architecture; DMAR/DRHD/RMRR](http://standa-note.blogspot.com/2020/05/introductory-study-of-iommu-vt-d-and.html), [ACRN VT-d design](https://projectacrn.github.io/latest/developer-guides/hld/hv-vt-d.html)).
- **For:** closes (c1) *and* (c2); the OptiPlex 7080 (Comet Lake / Q470, vPro-class) is expected to expose VT-d (verify the DMAR table on first boot — a discriminator, not an assumption).
- **Against:** it is itself a **blind-hardware mechanism build** (DMAR faults on real silicon, RMRR quirks), gating every feature in the lane behind it; and it **touches the proven Phase-6/7 substrate** — the Gen9 GPU and NVMe/AHCI do physical-address DMA today, so enabling VT-d requires identity/passthrough domains for *existing* devices, putting the 16-for-16-green accel path at regression risk before a single new feature ships. Two unproven things (a new hardware mechanism + a new driver) would then ride every dispatch — the exact bundling the discipline forbids.

### Option B — the fenced DMA arena (software containment)
A dedicated, pre-registered physical region from which **all** connectivity-driver DMA buffers are allocated, plus **descriptor validation at the programming boundary**: every address our driver writes into a ring/descriptor is checked against the arena before it reaches the device (a wrapper the driver cannot bypass — the same "one gate" pattern as `mint`). This is Linux's own answer where the IOMMU is absent or insufficient: **swiotlb bounce-buffering and restricted DMA pools** for untrusted devices ([restricted DMA, LWN](https://lwn.net/Articles/841916/), [swiotlb docs](https://docs.kernel.org/core-api/swiotlb.html), [bounce pages for untrusted devices, LWN](https://lwn.net/Articles/794595/) — which also notes that even *with* an IOMMU, sub-page transfers need bouncing because protection is per-4K-granule).
- **For:** fully contains **(c1)** — the realistic class; **host-testable** (the validation logic is pure); a **strict overlay** on the existing abstraction — every DMA user already goes through `DmaBuffer` (`dma.rs`), so the arena is `DmaBuffer::alloc_zeroed` gaining an arena-backed variant + a validation gate, not a refactor; kill-switchable (Rule 1: arena-off ≡ today, bit-for-bit); zero risk to the proven accel/storage paths (they are untouched); works on hardware without VT-d.
- **Against:** does **not** contain (c2). An arena is a fence around *our own* programming, not around the device. Claiming otherwise would be dishonest — so we won't.

### Option C — staged hybrid (RECOMMENDED)
**B now, A before the firmware.** Precisely:
1. **7.5-0 ships the fenced arena** as the lane's substrate — I-14(c1)'s guard, satisfying the "no unfenced DMA" prohibition for the USB arc, whose realistic threat *is* (c1).
2. **VT-d lands as its own dedicated mechanism build (7.5-V), sequenced before `iwlwifi` (7.5-C1)** — because WiFi's closed firmware blob is where (c2) stops being theoretical. The VT-d build's scope: remapping for **new connectivity devices only**, with identity/passthrough domains for the existing proven devices (GPU/NVMe/AHCI/virtio) so the Phase-6/7 substrate is provably unaffected (its kill-switch: VT-d-off ≡ today; its regression: the full family + the Phase-6 GPU scenarios, unchanged).
3. I-14(c)'s ledger row is upgraded in two recorded steps: **(c1) guarded at 7.5-0** (arena + validation + its gate), **(c2) guarded at 7.5-V** (hardware-enforced), each in the build that ships it — Rule 2 honored both times.
This is the same judgment the whole project runs on: the smallest sufficient mechanism first, behind an existing abstraction, with the hardware enforcement as a separately-gated upgrade — and the honest admission of what each tier does and does not stop, recorded in the ledger rather than blurred.

## 2. The lane, sliced

| Build | Content | Type | I-14 analysis (the standing-rule section) |
|---|---|---|---|
| **7.5-0** | **Fenced DMA arena**: arena-backed `DmaBuffer` variant + the descriptor-validation gate + the I-14(c1) ledger guard. Kill-switch: arena-off ≡ today. | software; host + QEMU | (c1) closed by construction; (a)/(b) untouched |
| **7.5-A** | **xHCI + USB core**: controller bring-up, command/event rings, enumeration, the kernel USB device model. All DMA via the arena. | **blind hardware** (the Phase-6 archetype) | (a) no AI locks in ISR; (b) IRQ storm → daemon backstop scenario; (c1) arena |
| **7.5-B** | **USB-Ethernet** (CDC-ECM, RNDIS fallback): a class driver on the USB core, **implementing the existing `NetDevice` trait** — DHCP/ARP/DNS/TCP light up from Phase 4. First *new-hardware* end-to-end packet. | protocol/driver | inherits A's; no new vectors |
| **7.5-V** | **VT-d DMA remapping**: DMAR/DRHD parse, per-device domains for connectivity devices, identity domains for the proven substrate, fault reporting (the (c2) discriminator). Upgrades I-14(c). | **blind hardware mechanism** | the (c2) guard itself; regression = entire family + Phase-6 GPU gates unchanged |
| **7.5-C1** | **`iwlwifi` device bring-up**: PCIe device, firmware load (under VT-d), host-command/RX/TX rings, 802.11 data path → `NetDevice`. | **blind hardware, hardest** | full three-vector analysis; firmware = (c2) adversary, now hardware-contained |
| **7.5-C2** | **WiFi control plane**: scan/auth/assoc (MLME), WPA2/WPA3 key exchange, the kernel connection-management surface (ADR-017). | software/protocol | (a)/(b); crypto in the pure tier |
| **7.5-D** | **Identity wiring**: hotplug interface lifecycle (link up/down on USB attach / WiFi associate), multi-interface extension of `select_source`, interface naming/identity. | software/policy | (a)/(b) only |
| **7.5-K** | **Rule-4 consolidation** (~after 7.5-V — 4 feature builds in). | digest | ledger re-audit incl. I-14 rows |

**Sequencing rationale (USB before WiFi):** USB-Ethernet delivers a real, second connectivity path early on the *simplest* hardware (validating arena + USB core + the Phase-4 control plane end-to-end) before the lane takes on `iwlwifi` — the largest, blindest, firmware-encumbered driver — with VT-d already standing between that firmware and governance memory. Each stage consumes the previous one's proven substrate; nothing rides on two unproven things at once.

**What is already built (so the lane doesn't rebuild it):** the entire L3+ control plane is Phase-4 complete and device-agnostic — `NetDevice` is the only seam a new driver must fill; `interface.rs`/`dhcpv4.rs`/`arp.rs`/`dns.rs`/TCP/TLS light up behind it. 7.5-D is *wiring* (hotplug + multi-interface), not stack-building.

## 3. Blind-hardware discriminators, pre-seeded (the 7.5-A failure-mode bank)
Per the standing rule, each blind dispatch ships its own failure-mode table; the research pass pre-seeds 7.5-A's known xHCI traps so the first dispatch starts from the field's scars, not ours ([osdev xHCI bring-up threads](https://forum.osdev.org/viewtopic.php?f=1&t=38652)): the **cycle-bit** protocol on command/transfer rings (forgetting it = silent no-op — discriminator: command-completion event counter); **dequeue-pointer semantics** (write the *next* TRB address, not the last processed); the **scratchpad array** some controllers require before anything works; **32-vs-64-byte context size** (read HCCPARAMS1, don't assume); the **interrupt-ack protocol** (clear USBSTS + IP bit, *then* advance the event-ring dequeue — wrong order = interrupt storm, which is also the I-14(b) test case arriving for free). Development order: QEMU's emulated xHCI first (it exists — full-speed loop in CI), OptiPlex blind runs second; QEMU-green is the entry gate for every bare-metal iteration.

## 4. Design questions for the PL
- **Q1 — Adopt the staged hybrid (Option C): fenced arena at 7.5-0 as the (c1) guard, VT-d as a dedicated 7.5-V build sequenced before `iwlwifi`, I-14(c) upgraded in two recorded steps?** *(Recommended — §1. Honest about what each tier stops; no bundling; the proven substrate untouched until VT-d's own gated build.)*
- **Q2 — Lane order USB-first (7.5-0 → A → B → V → C1 → C2 → D, consolidation after V)?** *(Recommended — §2. Earliest real connectivity on simplest hardware; WiFi lands on a proven USB-validated substrate with hardware DMA containment in place.)*
- **Q3 — QEMU-first development for every driver (QEMU xHCI / emulated NIC green before any OptiPlex blind run)?** *(Recommended — §3. The Phase-6 lesson industrialized: bare-metal iterations are for hardware-only unknowns, never for logic QEMU could have caught.)*
- **Q4 — Defer WPA3-specific crypto detail to the 7.5-C2 dispatch (its own pure-tier design + test vectors), keeping this opening lean?** *(Recommended — crypto belongs in the host-testable tier with RFC test vectors; no reason to design it before the radio exists.)*

## 5. If ratified — the first dispatch
**BUILD-7.5-0 (the fenced DMA arena):** arena region + arena-backed `DmaBuffer` allocation + the descriptor-validation gate (pure-tier validation logic, kernel glue at the programming boundary) + the I-14(c1) ledger guard + kill-switch (arena-off ≡ today, full family unchanged). Host + QEMU; no silicon; the lane's substrate. Then 7.5-A opens the blind-hardware arc on a contained footing.
