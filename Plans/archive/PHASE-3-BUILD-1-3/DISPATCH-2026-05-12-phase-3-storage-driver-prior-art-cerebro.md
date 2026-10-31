# DISPATCH - Phase 3 storage-driver prior-art research (Cerebro #2)

**Date filed:** 2026-05-12
**From:** PM (NeoPlutarch Phase 3 cadence)
**To:** Cerebro (deep-research lane)
**Ferry:** PL ferries this dispatch to a fresh Cerebro session. Status: ready to ferry.
**Type:** Question-level deep-research dispatch within the Phase 3 frame established by Cerebro #1 and ADR-021.
**Estimated budget:** 4-8 hours.
**Expected output:** `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md`

---

## 1. Context

Phase 3 builds NeoFS, block layer, and real disk I/O. ADR-021 has now ratified the substrate frame:

- BUILD-1: block layer, identity-mapped DMA, virtio-blk write-through, minimal VFS.
- BUILD-2: NeoFS native v1, single-tier physical-block-addressed CoW.
- BUILD-3: content-addressing, buffer cache, FLUSH, and GC if measurement-gated in.
- BUILD-4: FAT32/ext4 read-only and userspace polish.
- BUILD-5: SATA AHCI, NVMe, and bare-metal validation.

Cerebro #1 already performed the structural challenge for Phase 3 and reframed Q5. This dispatch is **not** asking Cerebro to reopen the Phase 3 frame. It asks Cerebro to answer storage-driver questions inside the now-ratified frame, with the usual caveat: if production prior art directly contradicts the frame, surface that contradiction explicitly.

## 2. What this dispatch is NOT asking

Do not re-survey:

- NeoFS CAS vs single-tier CoW. Cerebro #1 already answered this.
- Typed inodes, VFS FileCap, SELinux/Capsicum/9P synthesis. That is Cerebro #3.
- FAT32/ext4 implementation. That is BUILD-4 and may get its own implementation pass later.
- AI model storage systems. Covered by ALTIMIT research and Phase 5 substrate work.
- Full IOMMU architecture. Identity-mapped DMA is the Phase 3 decision; IOMMU is Phase 5+.

## 3. Research questions

### Question 1 - virtio-blk BUILD-1 write-through path

ADR-021 chooses virtio-blk first and relies on write-through correctness before FLUSH negotiation lands.

Research:

- In virtio v1.2 and production drivers, what are the exact semantics when `VIRTIO_BLK_F_WCE` is not negotiated?
- Is "guest does not negotiate write-back cache, therefore writes are write-through" sufficient for BUILD-1 correctness?
- How do Linux, Redox, Theseus, and other small kernels stage initial virtio-blk support before cache-control/FLUSH features?
- What minimal feature bits should NeoPlutarch negotiate in BUILD-1?
- What feature bits should be explicitly rejected or deferred?

Expected output: a minimal safe BUILD-1 virtio-blk feature-negotiation profile and a BUILD-3 FLUSH upgrade profile.

### Question 2 - virtio-blk request queue and descriptor discipline

NeoPlutarch needs a first virtqueue implementation that is correct under `no_std`, no heap allocation in interrupt context, and single-thread-per-process.

Research:

- How do production virtio-blk drivers structure descriptor tables, available rings, used rings, and DMA ownership?
- What descriptor-chain shape is sufficient for read/write requests?
- What memory barriers are required between descriptor writes, avail-ring update, device notify, used-ring observation, and completion?
- How should NeoPlutarch avoid allocation in IRQ context?
- What tests catch malformed descriptor ownership or lost used-ring notifications?

Expected output: implementation-shape notes suitable for Jarvis BUILD-1 dispatch.

### Question 3 - DMA infrastructure for small Rust kernels

ADR-021 chooses identity-mapped DMA first with a small kernel-internal DMA abstraction.

Research:

- How do Redox, Theseus, Asterinas, Tock, or other Rust kernels represent DMA buffers?
- What safe wrapper patterns prevent use-after-free while the device owns a buffer?
- What is the smallest useful abstraction: contiguous frame allocation, typed DMA buffer, scatter-gather list, or all three?
- For virtio-blk BUILD-1, is a contiguous DMA buffer pool enough?
- For AHCI/NVMe BUILD-5, what needs to be reserved in the abstraction now to avoid a rewrite?

Expected output: recommended BUILD-1 DMA API sketch plus forward-compat notes for AHCI/NVMe.

### Question 4 - storage IRQ completion and SC-3C wake composition

ADR-021 ratifies NeoFS as the first non-scheduler client of SC-3C cross-CPU wake.

Research:

- How do Linux completions, Plan 9 rendezvous, FreeBSD sleep/wakeup, and small kernels structure block-I/O completion?
- What does the IRQ handler do, and what work is deferred?
- Is the ADR-021 rendezvous shape (`done`, `waiting_thread`, result under IRQ-safe lock) sufficient?
- What memory-ordering and lock-order pitfalls should the BUILD-1 implementation avoid?
- How do drivers handle "issuer killed while I/O is outstanding"?

Expected output: confirm/extend the ADR-021 completion model and list implementation pitfalls.

### Question 5 - PCI/PCIe enumeration for block devices

Phase 3 needs PCI enumeration extensions for virtio-blk, AHCI, and NVMe.

Research:

- What minimum PCI config-space scanning is needed for virtio-blk BUILD-1?
- What BAR mapping and capability-walking support should land now vs later?
- What class codes and vendor/device IDs should be recognized for virtio-blk, AHCI, and NVMe?
- How should interrupt routing be represented initially (legacy INTx, MSI, MSI-X)?
- What is safe to defer until AHCI/NVMe BUILD-5?

Expected output: staged PCI/PCIe support plan for BUILD-1 vs BUILD-5.

### Question 6 - AHCI driver architecture for BUILD-5

AHCI remains a Phase 3 bare-metal requirement but is deferred to BUILD-5.

Research:

- What are the core AHCI data structures: ABAR, ports, command list, command table, PRDT, received FIS?
- What initialization sequence do production drivers follow?
- What minimum subset supports one SATA disk well enough for Phase 3 validation?
- What error handling is mandatory from day one?
- What features are safe to defer: port multipliers, hotplug, NCQ, power management?

Expected output: AHCI implementation plan and anti-scope list for BUILD-5.

### Question 7 - NVMe driver architecture for BUILD-5

NVMe is the most architecturally distinct Phase 3 storage driver.

Research:

- What minimum admin queue and I/O queue setup is needed?
- What queue-depth and per-CPU queue model is appropriate under NeoPlutarch's single-thread-per-process invariant?
- What PRP/SGL support is required for basic reads/writes?
- What MSI-X assumptions are safe, and what fallback is needed?
- What error handling and reset behavior must ship in Phase 3?

Expected output: NVMe implementation plan and anti-scope list for BUILD-5.

### Question 8 - storage-driver verification strategy

Phase 3 needs QEMU confidence before bare-metal confidence.

Research:

- What QEMU configurations best exercise virtio-blk write-through, FLUSH, AHCI, and NVMe?
- What fault-injection tests are realistic for lost completion, failed request, and device reset?
- What scenarios should BUILD-1 include for block smoke and VFS round-trip?
- What scenarios should BUILD-5 include for real hardware?
- What should the closure artifact record to satisfy Bible Article 13?

Expected output: staged verification matrix for BUILD-1, BUILD-3, and BUILD-5.

## 4. Deliverable structure

Output file: `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md`

Required sections:

1. Header: date, author, type, authority none, consumed dispatch path.
2. Executive summary: 4-8 paragraphs with the main driver sequencing findings.
3. Sources surveyed: primary sources separated from secondary/community sources.
4. Findings by question: one section per question above.
5. BUILD mapping: table mapping findings to BUILD-1, BUILD-3, BUILD-5.
6. Risks and corrections to ADR-021: list any confirmations, extensions, or contradictions.
7. Forward markers: Phase 4+ or Phase 5+ items surfaced but out of scope.
8. Calibration notes: citation confidence, single-source claims, unreachable sources, time-budget actual.
9. Cross-references and closure.

## 5. Citation rigor

Use primary sources where possible:

- OASIS virtio v1.2 spec for virtio-blk semantics.
- Linux source/docs for mature production driver behavior.
- Redox / Theseus / Asterinas / other small-kernel source for Rust or small-OS patterns.
- AHCI specification or OSDev only as a guide when primary spec access is limited; clearly mark OSDev as secondary.
- NVMe base/NVM command set specs for queue and command semantics.
- QEMU documentation for test-device behavior.

For every load-bearing claim, state whether it is primary-source verified, source-code inferred, secondary, or single-source.

## 6. Closure protocol

When complete, Cerebro reports back to PL with:

- artifact path;
- time-budget actual;
- one-line verdict on whether ADR-021 storage-driver sequencing is confirmed;
- any hard contradictions or BUILD-1 blockers;
- recommended next action for PM.

PM consumes the artifact and either:

- files a PM-NOTE confirming storage-driver sequencing and updating BUILD-1 dispatch inputs; or
- revises ADR-021 / THINK v2 if Cerebro surfaces a real contradiction.

## 7. Cross-references

Authoritative inputs:

- `docs/ADR-021-phase-3-design-substrate.md`
- `Plans/PHASE-3-THINK-V2-2026-05-11.md`
- `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md`
- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` (context only; do not re-survey Q5)
- `docs/bible.md` Articles 9.9, 10, 13
- ADR-013 and ADR-016 for SC-3C wake and IRQ-safe discipline inheritance

Output consumed by:

- PM storage-driver consumption PM-NOTE
- Cerebro #3 only if overlap emerges
- Phase 3 BUILD-1 PLAN dispatch for Jarvis
- Future BUILD-5 AHCI/NVMe PLAN dispatch

---

**End of dispatch.**

*Phase 3 Cerebro #2. Question-level storage-driver prior-art survey inside ADR-021 frame. Focus: virtio-blk write-through, DMA, IRQ completion via SC-3C wake, PCI staging, AHCI/NVMe BUILD-5 architecture, verification matrix. Ready to ferry.*
