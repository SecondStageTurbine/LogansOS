<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-050: Phase 7.5 BUILD-V — Intel VT-d / IOMMU (the I-14(c2) hardware DMA fence)

**Status:** Accepted — Stage A (QEMU `intel-iommu`) **and** Stage B (OptiPlex 7080 / Comet Lake silicon, blind) **both PASS 2026-06-13**; I-14(c2) fully proven
**Date:** 2026-06-13
**Design:** `Plans/THINK-2026-06-13-phase-7.5-build-V-vtd-iommu.md` (PL-ratified: arena-as-hardware-fence; identity domains for the substrate; DMA-remapping only; identity-first no-regression order)
**Dispatch:** `Plans/DISPATCH-2026-06-13-phase-7.5-build-V-vtd-iommu-jarvis.md`
**Builds on:** ADR-045 (the fenced arena = the (c1) guard this hardware-enforces)
**Ledger:** I-14(c2) mandated-not-yet-proven → **PROVEN** (Stage A QEMU + Stage B OptiPlex silicon, both 2026-06-13) — I-14 fully proven, the Phase-7.5 pre-mandate discharged

## The decision

The arena (7.5-0) restricts connectivity DMA to one physical region by
**software** validation — the (c1) guard against *our own* bad descriptor.
VT-d makes that **same restriction hardware-enforced against the device
itself** — the (c2) guard against errant firmware. (c1) and (c2) become one
fence: software-checked on the way in, hardware-bounded at the bus. VT-d does
not replace the arena; it *backs* it. This build arms the last open I-14
vector.

*(WiFi-target amendment 2026-06-13: the WiFi target is now MediaTek MT7601U, a
**USB** dongle whose DMA rides the already-arena-fenced xHCI controller, so
VT-d is no longer a hard prerequisite for the radio. VT-d stands on its own
merit as the I-14(c2) build for the USB controller and all connectivity
devices.)*

## The design — two domain classes

- **Identity domains for the proven substrate (GPU / NVMe / AHCI / virtio).**
  A second-level table mapping **every physical address to itself (GPA==HPA)**
  using **1 GiB super-pages** (3-level: the root *is* the PDPT, 512 × 1 GiB =
  512 GiB in one frame; 4-level: PML4[0] → one PDPT). The substrate keeps
  working with **zero driver changes**. Because identity covers the whole host
  space, the firmware-reserved **RMRR** regions are honored inherently (they
  fall inside the identity map) — no special RMRR carve-out needed, and the
  integrated-GPU aperture / USB legacy region (the V2 canary) never breaks.
- **Fence domains for connectivity devices (USB controller, NICs).** A
  translating domain mapping **only the arena's physical range** (4 KiB
  leaves; everything else not-present). A connectivity device — even one with
  errant firmware — **physically cannot reach memory outside the arena**; the
  IOMMU faults. This is I-14(c2)'s hardware guard.

## The mechanism

1. **DMAR/ACPI parse (ISC-1).** `acpi::init` walks RSDP → XSDT/RSDT (checksum-
   validated), finds the DMAR table, and the pure host-oracle'd `dmar_logic`
   yields the DRHD register bases, the RMRR `[base,limit]` regions, and the
   host address width. "No DMAR" is the conclusive **V1** verdict ("enable
   VT-d in BIOS").
2. **Translation structures (ISC-2).** Root table (by bus) → Context table (by
   dev/func) → second-level page tables (PML4/PDPT/PD/PT). All on-hardware
   layouts are built by `vtd_logic` `const fn`s pinned **byte-for-byte** by an
   independent host oracle (I-12): root entry (§9.1), context entry (§9.3,
   T=0 legacy second-level, FPD=0 so faults record), second-level entry (§9.7,
   validity = R|W, **no x86 present bit**), AGAW from CAP.SAGAW (the **V7**
   width discriminator).
3. **Identity-first, then narrow (ISC-3, the no-regression order).** Every
   enumerated PCI device is placed in the identity domain and translation is
   enabled (RTADDR → SRTP → context-cache + IOTLB invalidate → TE), with
   GSTS.TES read back (the **V3** discriminator). Only then are connectivity
   devices narrowed to the fence domain. **The substrate is byte-identical to
   VT-d-off** — the no-regression oracle.
4. **Fence the connectivity devices (ISC-4).** `fence_connectivity()` re-points
   the classifier-selected (USB/NIC) source-ids to the fence domain, with a
   context-cache + IOTLB invalidation after every change (the **V6**
   discriminator's reason to exist; required under CAP.CM, which QEMU sets).
5. **Fault reporting (ISC-5, the (c2) discriminator + live guard).** The DMAR
   fault-recording registers capture the faulting address + source-id of any
   out-of-domain DMA. `process_faults` drains them — **bounded** per call
   (`MAX_FAULTS_PER_DRAIN`, I-14(b)) and acquiring **no AI-governance lock**
   (I-14(a); the VT-d state lock is a leaf `SwitchMutex`). This is both the
   *proof* (the gate deliberately programs an out-of-arena DMA and asserts a
   fault fires) and the *production guard* (a real misbehaving driver faults in
   hardware instead of scribbling the ledger).
6. **Kill-switch (ISC-6).** Compiled only under the `vtd` feature — `vtd`-off ⇒
   none of this exists ⇒ **bit-for-bit pre-VT-d**. Within a `vtd` build,
   `disable()` clears GCMD.TE on every unit, returning to physical DMA (the
   runtime "translation never enabled" form). The driver also **fails safe**:
   no DMAR, no 1 GiB super-pages, or an unsupported AGAW logs a conclusive
   verdict and leaves DMA physical — the substrate is never regressed by a
   capability gap.

## Scope honored

DMA remapping only. **No interrupt remapping** (its own future concern). No new
drivers, no WiFi driver, no AI-subsystem change. Legacy (non-scalable)
root/context mode, second-level translation, requests-without-PASID.

## Prior-art traps pre-seeded (the trap-mining pass)

Before the first run, the documented field gotchas for VT-d + QEMU
`intel-iommu` are seeded into the failure bank with their discriminators:

- **V1** DMAR absent → "enable in BIOS" (verify the 7080 exposes VT-d).
- **V2** RMRR not honored → a proven device breaks on enable (identity covers
  RMRRs, so this is closed by construction; the GPU is the canary).
- **V3** translation not engaging → read GSTS.TES.
- **V4** identity domain incomplete → a substrate device faults on legit DMA
  (the fault-record address + source-id localizes it).
- **V5** fence too permissive → the deliberate out-of-arena DMA MUST fault.
- **V6** stale IOTLB → invalidate after every change.
- **V7** AGAW/SAGAW/MGAW width mismatch → read CAP, fail safe if unsupported.
- **Non-coherent tables (ECAP.C=0):** software must `clflush` edited table
  lines or the unit reads stale entries — handled (`write_slot` flushes when
  `!coherent`); a classic client-part trap pre-seeded for the OptiPlex.
- **QEMU virtio bypass:** QEMU's `intel-iommu` does **not** translate a virtio
  device's DMA unless `iommu_platform=on` + the guest negotiates
  `VIRTIO_F_ACCESS_PLATFORM`. So the (c2) fault proof uses a **real emulated
  DMA device (NVMe/AHCI)**, whose DMA QEMU always routes through the vIOMMU —
  never virtio-blk.

## The proof (status)

- **Host byte oracles (I-12) — DONE (9/9):** root entry, context entry,
  second-level PTE (incl. the no-implicit-present-bit case), 4-level IOVA
  decomposition, CAP/ECAP decode + AGAW selection, source-id, fault-record
  decode, and a full hand-laid 96-byte DMAR table + parse-totality on
  malformed input.
- **Stage A (QEMU `intel-iommu`) — PASS (2026-06-13):** in 3 conclusive
  iterations (a BIOS-region RSDP page-fault, then a missing test disk, then
  green — each self-localized by the scenario discriminators). DMAR parsed
  (DRHD at `0xfed90000`, `sagaw=0x6`, `sllps=0x3`, `cm=true`, `coherent=false`);
  AGAW 48-bit/4-level; 11 devices identity-domained; `TES=true` (V3); the
  kernel booted clean under VT-d-on (no-regression). The fenced NVMe
  (dev 00:07.0) DMA'd its submission queue at `iova=0x1a07f040`, OUTSIDE the
  arena `[0x1a0c0000,0x1a100000)` → QEMU `vtd_iommu_translate` reported the
  translation failure → the kernel recorded `sid=0x0038` (= source_id(0,7,0)),
  `fault_addr=0x1a07f000` (out-of-arena), `faults=1`, governance sentinel
  intact, kill-switch cleared TES — the full (c2) proof. The driver correctly
  exercised the `coherent=false` clflush path and the `caching-mode=true`
  invalidation path.
- **Stage B (OptiPlex 7080 / Comet Lake Q470, blind) — PASS (2026-06-13):**
  staged by `ssh`-`dd` (kernel ISO → `nvme0n1`, AHCI log-sink → `sda`, both
  sha-verified); the PL booted blind; the AHCI log-sink (captured under
  VT-d-on, itself a no-regression signal) recorded the run, dd-extracted +
  `neo-extract-log`'d back. Result: DMAR parsed, TES set, no-regression read
  OK, the fenced NVMe at **BDF 01:00.0 / sid 0x0100** DMA'd to **0x47a372000**
  just below the arena `[0x47a380000,0x47a3c0000)` — **Comet Lake's IOMMU
  faulted it** (`faults=1`), governance intact, kill-switch cleared TES.
  `i14c2=proven`. Evidence:
  `Plans/archive/evidence/phase7.5-build-V-vtd-optiplex-7080-20260613-stageB-silicon.log`.
  (The driver's `vtd:` capability breadcrumbs log before the log-sink installs,
  so they are not on the disk; the PASS frame is the conclusive proof.)

## Consequences

- On Stage A/B PASS, **I-14 is fully proven** ((a)/(b)/(c1) silicon-proven at
  7.5-A/B; (c2) hardware-proven here) — the pre-mandate discharged, and the
  connectivity lane (USB-Ethernet now, MT7601U WiFi next) runs on a
  hardware-contained footing.
- New leaf lock (the VT-d state `SwitchMutex`) — unordered by design, never
  nested under the AI band (whose ceiling is `USB_CORE=31`); recorded for the
  next consolidation's lock census.
