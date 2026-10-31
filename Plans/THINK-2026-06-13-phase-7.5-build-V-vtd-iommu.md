<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7.5 BUILD-V — VT-d / IOMMU (arming I-14(c2), the hardware DMA fence)

**Date:** 2026-06-13
**Seat:** PM/Cerebro (design + deep-research pass; the dispatch follows)
**Context:** The connectivity lane's USB/Ethernet sub-arc is silicon-proven (0/A/B). I-14 has its (a)/(b)/(c1) guards proven on real hardware; **(c2) — a *misbehaving device* DMAing where we never programmed — remains mandated-not-yet-proven**, by deliberate design: the staged-hybrid decision (lane-opening THINK Q1) shipped the software fence first and reserved hardware enforcement for a dedicated build *before* `iwlwifi`, because WiFi's multi-MB closed firmware blob is where (c2) stops being theoretical. **7.5-V is that build.** It must arm (c2) without regressing the silicon-proven GPU/NVMe/AHCI/virtio substrate.
**Related:** `docs/complexity-governance.md` I-14 (the (c2) deferral); ADR-045 (the fenced arena = the (c1) guard); the lane-opening THINK §1/§5 (staged-hybrid, VT-d-before-iwlwifi); ANSPEC §10.6 (DMA isolation, renumbered from §10.4 by BUILD-K). Blind-hardware + I-14 fault-containment standing rules both apply.

> **Amendment (2026-06-13, Article 24.1):** the original "VT-d *before* `iwlwifi`" rationale (below) assumed a **PCIe** WiFi chip with its own DMA-mastering firmware blob. The WiFi target is now **MediaTek MT7601U — a USB dongle** (PL-DECISION-2026-06-13; ADR-017 Amendment). A USB device's DMA rides the **xHCI controller**, which is already arena-fenced (7.5-0) and gets VT-d's USB fence domain here — so **VT-d is no longer a hard prerequisite for WiFi; the two builds are independent.** VT-d still stands fully on its own merit as the I-14(c2) build for the USB controller and all connectivity devices. The `iwlwifi` references below are the original design context, retained as history.

## 1. The load-bearing insight: the arena and VT-d are the *same fence*, software then hardware
7.5-0's fenced arena already restricts every connectivity-driver DMA address to one physical region, by **software validation** (`FencedDmaAddr`, the (c1) guard against *our own* bad descriptor). VT-d's job is to make that **same restriction hardware-enforced** against the *device itself*: put each connectivity device into a VT-d **translating domain whose second-level page tables map only the arena's physical range**. Then a connectivity device — even one with errant firmware issuing DMA we never programmed — **physically cannot reach memory outside the arena**; the IOMMU faults. (c1) and (c2) become one fence: software-checked on the way in, hardware-bounded at the bus. This is the minimal, composable design — VT-d doesn't replace the arena, it *backs* it.

## 2. The design

### Translation structure (from the research)
Root table (indexed by PCI **bus**) → Context table (indexed by **dev/func**) → **second-level page tables** (PML4/PDPT/PD/PT, the EPT-style format). The **DMAR ACPI table** yields the **DRHD** units (one per remapping hardware unit) and the **RMRR** entries (firmware-reserved regions a device must keep identity-mapped). Sources: [Intel VT-d spec](https://cdrdv2-public.intel.com/671081/vt-directed-io-spec.pdf), [Satoshi — IOMMU/VT-d study](http://standa-note.blogspot.com/2020/05/introductory-study-of-iommu-vt-d-and.html), [ACRN VT-d HLD](https://projectacrn.github.io/2.2/developer-guides/hld/hv-vt-d.html), [terenceli — intel IOMMU driver analysis](https://terenceli.github.io/%E6%8A%80%E6%9C%AF/2019/08/10/iommu-driver-analysis).

### Two domain classes (the no-regression decision)
- **Identity domains for the proven substrate (GPU / NVMe / AHCI / virtio).** These do physical-address DMA today; under VT-d they get a domain whose second-level tables map **every physical address to itself (GPA==HPA)** — they keep working with **zero driver changes**. **RMRRs MUST be honored** (the integrated graphics aperture, USB legacy regions): a device whose firmware-reserved region isn't identity-mapped *breaks the instant translation enables*. The GPU is the canary.
- **Fence domains for connectivity devices (USB controller, USB-Ethernet, later `iwlwifi`).** A translating domain mapping **only the arena's physical range**. Any DMA outside it → DMAR fault → contained. This is I-14(c2)'s hardware guard.

### Fault reporting = the (c2) discriminator AND the runtime guard
The DMAR **fault recording registers** capture the faulting address + source-id when a device DMAs outside its domain. This is both the *proof* (the gate deliberately programs an out-of-arena DMA and asserts a fault fires + governance memory is untouched) and the *production guard* (a real misbehaving driver/firmware faults in hardware instead of scribbling the ledger — I-14(c2) live).

### Kill-switch (Rule 1)
VT-d disabled (translation never enabled, or feature-off) ⇒ every device does physical DMA exactly as today ⇒ **the proven substrate is bit-for-bit unchanged.** The regression: VT-d-off → the full family + the Phase-6 GPU scenarios identical. Complexity provably switched off.

### Scope: DMA remapping only
VT-d also does **interrupt remapping** — **out of scope** (we want DMA isolation, not IR; enabling IR is its own concern, deferred). DMAR (DMA remapping) only, second-level translation, requests-without-PASID.

## 3. Blind-hardware failure-mode + discriminator section (standing rule — the OptiPlex 7080 / Comet Lake Q470)
QEMU's emulated `intel-iommu` is the entry gate; OptiPlex blind runs second. Discriminators ship in-build:

| # | Failure mode | Discriminator (observation → cause) |
|---|---|---|
| V1 | **VT-d absent / disabled in BIOS** — no DMAR table or no DRHD | parse + log the DMAR ACPI table at boot (DRHD count, register base); "no DMAR" ⇒ conclusive "enable VT-d in BIOS" verdict, distinct code — *verify, do not assume the 7080 exposes it* |
| V2 | **RMRR not honored** — a proven device (integrated GPU, USB legacy) breaks when translation enables | log every RMRR (base/limit/source-id) from DMAR; a proven device regressing under VT-d-on ⇒ its RMRR identity-map is missing (the GPU is the canary) |
| V3 | **Translation not engaging** — TES bit never sets / context malformed | read the Global Status Register **TES** bit after enable; a device that *should* fault on bad DMA doesn't ⇒ translation isn't actually on |
| V4 | **Identity domain incomplete** — a proven device faults on its *legit* DMA | the fault-recording register's faulting address + source-id: if it's a proven device's normal address, the identity domain/RMRR is incomplete |
| V5 | **Fence domain too permissive** — the (c2) guard doesn't actually contain | the deliberate out-of-arena connectivity DMA test MUST raise a DMAR fault; if it doesn't, the connectivity domain is too wide |
| V6 | **Stale IOTLB** — old translations after domain setup | register/queued invalidation after every domain change; a device using a stale mapping ⇒ missing invalidation |
| V7 | **Wrong width / level** — AGAW/SAGAW mismatch (host address width vs supported) | read CAP register SAGAW/MGAW; pick the supported address width, log it; a context-entry reject ⇒ width mismatch |

Every bare-metal run conclusive pass **or** fail per this table.

## 4. I-14 governance fault-containment analysis (standing rule — this build *is* the (c2) guard)
- **(a) lock isolation:** the DMAR fault ISR touches only VT-d state — **no** AI-governance lock; the new VT-d lock(s) enter `sync::lock_order`. Unchanged posture.
- **(b) liveness:** a DMAR fault storm (a thrashing device) is bounded — cap fault-records processed per ISR, defer; governance unaffected (the I-2 backstop).
- **(c2) — THE DELIVERABLE:** hardware-enforced for connectivity devices via the arena-bounded fence domain; the proven substrate is identity-mapped (no regression). **Ledger upgrade: I-14(c2) mandated-not-yet-proven → PROVEN** (on the bare-metal out-of-arena-DMA-faults gate). This build closes the last open I-14 vector and discharges the pre-mandate's purpose.

## 5. The risk that dominates: regressing the proven substrate
The single biggest hazard is **enabling translation before every existing device is in a working domain** — they'd fault and break. Mitigation, baked into the dispatch:
1. **Identity-domain + RMRR every existing device FIRST**, verify the full family + the Phase-6 GPU scenarios green under VT-d-on, *then* narrow the connectivity devices to fence domains.
2. **The proven-substrate-stays-green discriminator is a hard gate**, not an afterthought: the accel/NVMe/AHCI/virtio scenarios under VT-d-on are the regression oracle. Any fault from a proven device's legit DMA (V4) is a blocker.
3. **QEMU `intel-iommu` first** — the identity-domain + fence-domain logic, RMRR handling, and the fault-on-out-of-arena assertion are all exercisable in emulation before the OptiPlex.

## 6. Sequencing
7.5-V lands **after the 7.5-K consolidation** (the §10.x renumber + ledger re-audit) — one digest before the next feature, per Rule-4 cadence. The dispatch is cut off **post-K main**, and its §-references reconcile to the post-K numbering. 7.5-V is the prerequisite for **7.5-C1 (`iwlwifi`)** — the firmware blob may not be granted DMA until its fence domain exists.

## 7. Design questions for the PL
- **Q1 — Adopt the arena-as-hardware-fence design (connectivity devices in translating domains bounded to the arena's physical range; proven substrate in identity domains + RMRRs; DMA-remapping-only, no interrupt remapping)?** *(Recommended — §1/§2. Minimal, composable with 7.5-0, no substrate regression, closes I-14(c2).)*
- **Q2 — Identity-everything-first, then narrow connectivity (the no-regression order), with the proven-substrate-green-under-VT-d-on as a hard regression gate?** *(Recommended — §5.)*
- **Q3 — QEMU `intel-iommu` as the entry gate before any OptiPlex blind run, and DMAR-only (interrupt remapping deferred)?** *(Recommended — §2/§3.)*
- **Q4 — Land after 7.5-K, dispatch cut off post-K main?** *(Recommended — §6, Rule-4 cadence + the §10.x renumber dependency.)*

On ratification, the dispatch (`Plans/DISPATCH-2026-06-13-phase-7.5-build-V-vtd-iommu-jarvis.md`, drafted alongside this) builds it; ANSPEC §10.6 extends to (c2)-hardware-enforced; ADR-050 records the IOMMU design + the identity/fence domain split.
