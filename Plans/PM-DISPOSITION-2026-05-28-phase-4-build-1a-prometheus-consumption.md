# PM-DISPOSITION — Phase 4 BUILD-1a Prometheus Research Consumption

**Date:** 2026-05-28
**From:** PM (Cerebro-seat)
**Consumes:** ``Plans/RESEARCH-2026-05-28-phase-4-build-1a-rust-virtio-net-ecosystem-prior-art-prometheus.md``
**Closes:** ``DISPATCH-2026-05-28-phase-4-build-1a-rust-virtio-net-prior-art-prometheus.md``
**Affects:** ``DISPATCH-2026-05-28-phase-4-build-1a-netdevice-virtio-net-smoltcp-wrap-jarvis.md`` (Jarvis substrate extended; OQs disposed; additional anti-scope and forward marker).

## 1. Consumption

Prometheus's ecosystem breadth survey landed at 21,786 bytes. Six projects surveyed (Asterinas, Theseus, Redox, Toubkal, rCore, Hubris) plus the ``virtio-drivers`` crate architecture and smoltcp examples. LionsOS confirmed as a negative result (seL4-userspace, out of comparison set). All four §10 open questions surface as PM-tier decisions and are disposed below. Headline finding accepted: **do NOT depend on the external ``virtio-drivers`` crate.** virtio-blk already owns the HAL + Transport + VirtQueue primitives at ``kernel/src/drivers/block/virtio_blk.rs``; BUILD-1a's virtio-net mirrors that discipline rather than pulling in a third-party crate.

## 2. Open-Question Dispositions

### OQ-1 — PCI vs MMIO transport priority

**Disposition: PCI transport at BUILD-1a.** Matches QEMU's default (``-device virtio-net-pci``), matches the BUILD-1a goal milestone's QEMU smoke-test substrate (ISC-1a-3), and matches the existing virtio-blk discipline. MMIO transport (cloud-host scenarios, AWS Nitro, Firecracker, virtio-mmio for aarch64) is a **forward marker** for later BUILDs, not BUILD-1a scope.

### OQ-2 — Queue size

**Disposition: ``QUEUE_SIZE = 64`` at BUILD-1a.** Universal across surveyed projects; sufficient for BUILD-1a's frame round-trip ISC; do not over-build. Throughput optimization (256 or larger) is a forward marker tied to specific measurement evidence.

### OQ-3 — Control queue (queue index 2)

**Disposition: ANTI-SCOPE at BUILD-1a.** Control queue handles VLAN configuration and offload negotiation — neither is required for ISC-1a-1 through ISC-1a-5. Defer to BUILD-1b+ if and only if concrete need surfaces. virtio-net feature negotiation at BUILD-1a uses ``VIRTIO_NET_F_MAC`` and ``VIRTIO_NET_F_STATUS`` only (matches Prometheus §9.3 / Asterinas conservative posture).

### OQ-4 — Indirect descriptors (``RING_INDIRECT_DESC``)

**Disposition: ANTI-SCOPE at BUILD-1a.** Adds a DMA allocation per chain in exchange for reduced interrupt frequency. Not required for BUILD-1a's ISCs. Defer indefinitely; revisit only if throughput evidence demands it.

## 3. Additional Directives For Jarvis (Dispatch Addendum)

The Jarvis BUILD-1a dispatch is amended (substrate list edit, see §5 below) with the following directives that must hold:

1. **Reject the ``virtio-drivers`` crate dependency.** Cargo dependency on ``smoltcp`` (already authorized per Cerebro Q1 → ``smoltcp 0.13.1``) is the only network-related external dep. Mirror the virtio-blk publication discipline (three-barrier: Release at desc→avail-idx, SeqCst at idx→notify, Acquire at completion) inside ``virtio_net.rs`` directly. Cerebro's audit of ``kernel/src/drivers/block/virtio_blk.rs`` lines 619/622/657 is the reference pattern.
2. **PCI transport, ``QUEUE_SIZE = 64``, no control queue, no indirect descriptors.** Per OQ-1 / OQ-2 / OQ-3 / OQ-4 dispositions above.
3. **virtio-net feature negotiation: MAC + STATUS only.** Specifically: ``VIRTIO_NET_F_MAC`` (negotiate to obtain device MAC) and ``VIRTIO_NET_F_STATUS`` (link state observability for ISC-1a-1). No checksum offload, no TSO/GSO/LRO, no multiqueue, no MRG_RXBUF.
4. **RX buffer strategy: pooled DMA with pre-allocation.** Pre-allocate all RX buffers at init (``QUEUE_SIZE = 64``). Pre-create the replacement buffer before popping from the used ring. TX buffers allocated on-demand from a pool. Matches Prometheus §9.4 / Asterinas pattern.
5. **IRQ handling: softirq bottom-half pattern.** Hard IRQ acknowledges and schedules a softirq; softirq handler does the per-packet work. Keeps hard-IRQ latency low and avoids reentrancy hazards. Matches Prometheus §9.5 / Asterinas pattern. NeoPlutarch's equivalent of "softirq" is the deferred-work primitive already used by the storage path; reuse it, do not invent a new mechanism.
6. **smoltcp integration: ``phy::Device`` against our ``NetDevice``.** Hubris Smol pattern per Cerebro Q4 + Prometheus §9.6. Publication fence lives in the driver (``virtio_net.rs``), NOT in the wrap shim. Wrap shim stays under the ~200-line gate from PM-DECISION §6.

## 4. Rejected (with Reasoning)

- **Refactoring virtio-blk into ``kernel/src/drivers/virtio/blk/`` to share HAL/Transport/VirtQueue.** Prometheus §9.2 recommends the consolidated layout but it requires touching ``virtio_blk.rs``, which the Jarvis dispatch §7 explicitly forbids ("Do NOT modify ``kernel/src/drivers/block/virtio_blk.rs`` or any Phase 3 code"). The /simplify discipline cited by PM-DECISION §6 keeps BUILD-1a from absorbing a refactor. Filed as **Forward Marker FM-VirtIO-1** below.
- **External ``virtio-drivers`` crate dependency** (used by Redox, Toubkal, rCore). Per Prometheus headline; per the Bible's Article 4.3 "every dependency is a liability"; per the existence of virtio-blk's already-proven in-tree HAL/Transport/VirtQueue substrate.

## 5. Jarvis Dispatch Substrate-List Edit

The dispatch's substrate-list is extended (in this commit) to include:

```
- Plans/RESEARCH-2026-05-28-phase-4-build-1a-rust-virtio-net-ecosystem-prior-art-prometheus.md
- Plans/PM-DISPOSITION-2026-05-28-phase-4-build-1a-prometheus-consumption.md (this file)
```

Jarvis must read all five substrate files before starting BUILD-1a implementation. The PM-DISPOSITION encodes the OQ dispositions and the additional directives that, combined with the original dispatch and Cerebro's RESEARCH, fully constrain BUILD-1a's design space.

## 6. Forward Markers

- **FM-VirtIO-1 — virtio subsystem directory consolidation.** Prometheus §9.2 recommends consolidating all virtio devices under ``kernel/src/drivers/virtio/{hal,transport,queue,blk,net,...}/`` with shared abstractions. This is the Asterinas ``aster-virtio`` shape. NeoPlutarch's current layout splits virtio-blk into ``drivers/block/`` and (post BUILD-1a) virtio-net into ``drivers/net/``. The consolidation is a real win when a third virtio device class appears (virtio-gpu for Phase 6+, virtio-console, etc.) but is not justifiable for two devices alone. **Revisit-trigger:** when a third virtio device driver lands, file an ADR proposing the consolidation refactor.
- **FM-VirtIO-2 — MMIO transport for cloud hosts.** Per OQ-1 disposition. Revisit-trigger: when NeoPlutarch is targeted at a vhost-user / Firecracker / AWS Nitro deployment scenario, or when aarch64 virtualization targets virtio-mmio.

## 7. What This Disposition Does NOT Do

- Does NOT modify BUILD-1a ISC criteria (ISC-1a-1 through ISC-1a-5 stand as written in PM-DECISION §5).
- Does NOT modify the BUILD-1 PLAN, the ratified THINK v0.2, or any sub-commit boundaries (1a, 1b, 1c, 1d, 1.1, 1f, 1e).
- Does NOT alter the Cerebro Q1 disposition (smoltcp 0.13.1, Hubris-style wrap, ~200-line gate).
- Does NOT require PL re-ratification. Filed as PM-DISPOSITION (operational), consistent with the project's standing pattern.

---

**End of PM-DISPOSITION.**

*Prometheus RESEARCH consumed. OQ-1..OQ-4 disposed. Jarvis dispatch substrate extended. Two forward markers (FM-VirtIO-1, FM-VirtIO-2) filed.*