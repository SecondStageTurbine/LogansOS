# DISPATCH — Phase 4 BUILD-1a Pre-Implementation Research → Cerebro

**Date:** 2026-05-28
**From:** PM
**To:** Cerebro (Claude, deep internet research + light coding)
**Authority:** `Plans/PL-RATIFICATION-2026-05-28-phase-4-build-1-pm-decision.md` ratifying `Plans/PM-DECISION-2026-05-28-phase-4-build-1-opening.md` §4 Q1 (smoltcp = Hubris-style wrap).
**Gates:** none — research is the first task in the BUILD-1a chain.
**Blocks:** `DISPATCH-2026-05-28-phase-4-build-1a-netdevice-virtio-net-smoltcp-wrap-jarvis.md` (Jarvis implementation; Jarvis must not start until this research returns).

## 1. Purpose

The PM-DECISION ratified the Hubris-style smoltcp wrap posture. Before Jarvis writes a single line of kernel code, Cerebro answers four load-bearing questions whose wrong-answers would cost expensive rework deep in BUILD-1a or BUILD-1b. The output is a research report Jarvis consumes as substrate for the implementation dispatch.

## 2. Research Questions

### Q1 — smoltcp v0.13.0 surface for in-kernel embedding

- What is the **minimal** smoltcp API surface required to satisfy BUILD-1a's scope (ISC-1a-3: Ethernet frame round-trip via a wrap, no IP layer yet)?
- What is the **larger** API surface BUILD-1b will need (IPv6 stack core: ICMPv6, ND, SLAAC, RFC 8981 Privacy Extensions, RFC 8106 RDNSS option lift)?
- What changed materially in v0.13.0 (released 2025-03-20) vs the prior stable release? Identify breaking changes that affect kernel-embedded use.
- What are the **poll-loop assumptions** smoltcp makes about the embedding scheduler? Specifically: does it require periodic polling? Is it driven by interrupts? How does it surface "I have outbound traffic ready"?
- Are there any **`no_std` rough edges** still present in v0.13.0 that affect kernel embedding (allocator requirements, `core::time::Duration` use, etc.)?

### Q2 — Hubris `task/net` wrap pattern study

- How does Hubris embed smoltcp? Specifically: where is the scheduling shim, what is its rough line count (informs the PM-DECISION §6 "~200 lines" gate), and what API does it expose to the rest of Hubris?
- Does Hubris expose smoltcp's types directly (`Interface`, `Device`, `SocketSet`) to its clients, or wrap them behind a Hubris-specific abstraction? If wrapped, what shape does the wrapper take?
- Where in Hubris's source tree do the relevant pieces live? Cite specific files / paths so Jarvis can read them directly when implementing the NeoPlutarch wrap.
- What does Hubris's wrap **not** do that NeoPlutarch's wrap will need to do? Hubris is service-processor scope (controlled environment, fixed deployment). NeoPlutarch is general-purpose. Identify the deltas.

### Q3 — virtio-net publication-fence verification

Phase 3 BUILD-5b exposed a missing-publication-fence bug in the AHCI driver (commit `2fdb7cb`): descriptor writes were not published to the device before the doorbell, causing wrong-LBA + `.rodata` over-DMA on real hardware (invisible in QEMU). The fix matched the NVMe submit/complete `fence(Release)`/`fence(Acquire)` discipline.

- Does virtio-net descriptor submission have the **same memory-ordering hazard** as the AHCI command table? Specifically: are virtio queue descriptors + the available-ring write to the virtqueue notify register a publication-fence-required pair on real hardware (Intel chipset + a USB Ethernet adapter or PCIe NIC backing the virtio device, if applicable)?
- Confirm the existing virtio-blk driver's publication discipline (read `kernel/src/drivers/block/virtio_blk.rs`) and verify it uses the same release/acquire pattern. If it doesn't, that is a finding to surface separately (it may have been masked by QEMU just like AHCI was).
- Recommend the **exact publication discipline** virtio-net's TX submission path should use, matching virtio-blk's pattern.

### Q4 — Recommended `NetDevice` trait shape

Given Q1–Q3, recommend the **trait shape** for `kernel/src/net/device.rs` `NetDevice`. Parallel to `BlockDevice` per PM-DECISION §3. Specifically:

- Method signatures (alloc/free of TX/RX buffers, submit-frame, completion-via-notification, descriptor publication).
- How `NetDevice` interacts with the smoltcp wrap (does smoltcp's `Device` trait sit on top of `NetDevice`, or alongside it, or does the wrap *implement* smoltcp's `Device` trait against our `NetDevice`?).
- Where the publication-fence (Q3) sits in the trait method bodies.
- Any forward markers for BUILD-2's IPv4 work, BUILD-1b's IPv6 wiring, etc., that would inform trait shape now.

## 3. Deliverables

One artifact:

**`Plans/RESEARCH-2026-05-28-phase-4-build-1a-smoltcp-and-virtio-net-cerebro.md`**

Structured as:

1. Header (date, from Cerebro, for PM/Jarvis consumption).
2. Executive summary (3–5 bullets — Jarvis should be able to start from this alone in a pinch).
3. Q1 findings — smoltcp v0.13.0 surface (minimal + larger), poll-loop posture, breaking-change deltas, `no_std` notes.
4. Q2 findings — Hubris `task/net` wrap shape, line counts, source paths, deltas-to-NeoPlutarch.
5. Q3 findings — virtio-net publication-fence verdict + virtio-blk audit confirmation + recommended discipline.
6. Q4 findings — recommended `NetDevice` trait shape + smoltcp-wrap integration recommendation + forward markers.
7. References (web sources, source tree paths, RFC citations).
8. Open questions for PM (if any surface during research that exceed the dispatch scope).

## 4. Anti-Scope

- **No kernel code.** Cerebro produces a research artifact only. Jarvis writes the code.
- **No NetDevice implementation.** Recommended trait shape (Q4) is a sketch in markdown, not a Rust file.
- **No smoltcp Cargo dependency added.** Jarvis adds it during BUILD-1a implementation, using the version Cerebro specifies in the research return.
- **No fork of smoltcp.** Ratified posture is wrap. Q1 surveys smoltcp as a dependency, not as a fork candidate.
- **No re-litigation of Q1/Q2/Q3 of the PM-DECISION** (smoltcp wrap, RDNSS placement, TCP sketch depth). Those are ratified.
- **No deep dive on Netstack3, lwIP, or scratch implementations.** Mention briefly if relevant to Q1 (ecosystem context), but the depth is on smoltcp.

## 5. Return Contract

Cerebro's return report posts to PM via PL and contains:

- Full path to the filed `Plans/RESEARCH-...-cerebro.md` artifact.
- Three-bullet executive summary of the most decision-relevant findings.
- Explicit answer per question (Q1, Q2, Q3, Q4 each get a short verdict line).
- Any open questions for PM that surfaced.
- The exact smoltcp version recommended for the Cargo dependency.

If research surfaces a finding that materially affects the PM-DECISION's ratified positions (smoltcp wrap is wrong, virtio-blk has a hidden publication bug, etc.), Cerebro flags it as a **CRITICAL** open question rather than soldiering on with a workaround. PM owns the re-decision.

## 6. Time / Depth Guidance

This is a focused pre-implementation study, not a survey paper. Target a tight, useful artifact in the 200–400 line range. Depth: enough that Jarvis can implement BUILD-1a without re-doing the smoltcp study. Breadth: bounded by the four questions above.

If Q3's virtio-blk audit surfaces a publication-fence bug in the existing virtio-blk driver, that becomes a separate finding worth a BLOCKER artifact regardless of Phase 4 scope — flag it back to PM immediately rather than rolling into Phase 4 work.

---

**End of dispatch.**

*Cerebro: read smoltcp v0.13.0, read Hubris `task/net`, audit virtio-blk publication discipline, recommend NetDevice trait shape. File `RESEARCH-2026-05-28-phase-4-build-1a-smoltcp-and-virtio-net-cerebro.md`. Jarvis is gated on your return.*
