# DISPATCH — Phase 4 BUILD-1a Implementation (NetDevice + virtio-net + smoltcp wrap) → Jarvis

**Date:** 2026-05-28
**From:** PM
**To:** Jarvis (Codex, heavy kernel coding)
**Authority:** `Plans/PL-RATIFICATION-2026-05-28-phase-4-build-1-pm-decision.md` ratifying `Plans/PM-DECISION-2026-05-28-phase-4-build-1-opening.md`.
**Substrate (must be on disk before you start):**
- `Plans/PM-DECISION-2026-05-28-phase-4-build-1-opening.md` (the PLAN; especially §5 BUILD-1a and §6 `/simplify` discipline).
- `Plans/PM-NOTE-2026-05-28-phase-4-build-1-speculative-tcp-shape-sketch.md` (informational; FM-2 affects BUILD-1c not 1a, but read it).
- **`Plans/RESEARCH-2026-05-28-phase-4-build-1a-smoltcp-and-virtio-net-cerebro.md`** (Cerebro's return — recommended smoltcp version, NetDevice trait shape, publication-fence verdict).
- **`Plans/RESEARCH-2026-05-28-phase-4-build-1a-rust-virtio-net-ecosystem-prior-art-prometheus.md`** (Prometheus's ecosystem prior-art survey — comparative table across six Rust OSes + concrete BUILD-1a recommendations).
- **`Plans/PM-DISPOSITION-2026-05-28-phase-4-build-1a-prometheus-consumption.md`** (PM consumption of Prometheus's research; dispositions OQ-1..OQ-4; encodes the additional directives below).

## GATE

**Do not start implementation until BOTH `RESEARCH-...-cerebro.md` AND `PM-DISPOSITION-...-prometheus-consumption.md` are filed on `main`.** Cerebro's RESEARCH specifies the smoltcp version, the recommended `NetDevice` trait shape, and the publication-fence discipline. The PM-DISPOSITION encodes the OQ-1..OQ-4 dispositions (PCI transport, `QUEUE_SIZE = 64`, no control queue, no indirect descriptors), the rejection of the external `virtio-drivers` crate, the feature-negotiation set (`VIRTIO_NET_F_MAC` + `VIRTIO_NET_F_STATUS` only — no checksum offload, TSO, GSO, LRO, multiqueue, or MRG_RXBUF), the pooled-DMA pre-allocation pattern, the softirq bottom-half IRQ pattern, and the `phy::Device`-against-`NetDevice` smoltcp integration shape. Starting before either lands risks expensive rework.

PM will signal the gate is open by linking the Cerebro return in a follow-up note.

## 1. Scope (per PM-DECISION §5 BUILD-1a)

Create the network substrate. Three concrete code drops in one commit (or sequenced if the kernel build is cleaner that way — your call, document in commit message):

1. **`kernel/src/net/` module tree created.** Declare `mod net;` in `kernel/src/lib.rs`. Initial contents: `device.rs` (the `NetDevice` trait), `mod.rs` (re-exports), and any minimal scaffold needed to compile.
2. **`kernel/src/drivers/net/virtio_net.rs`** — virtio-net PCI driver implementing the `NetDevice` trait. Reuse the Phase 3 PCI / MSI-X / DMA-buffer infrastructure (`kernel/src/drivers/block/virtio_blk.rs` is the closest pattern; copy the discipline, not the file).
3. **smoltcp wrap shim.** Cargo dependency on `smoltcp` at the version Cerebro specifies. Kernel-owned scheduling shim integrating smoltcp's `Device` trait with our `NetDevice`. The shim is **kernel-internal only** at BUILD-1a — no userspace surface yet.

## 2. ISC Criteria (must all hold for BUILD-1a close)

Per PM-DECISION §5 BUILD-1a. Reproduced here with implementation guidance:

- **ISC-1a-1.** `kernel/src/drivers/net/virtio_net.rs` exists; PCI vendor `0x1AF4` / device `0x1000` (legacy) and `0x1041` (modern transitional) probed and bound; link state observed in serial log via `klog!`. *Implementation note:* re-use the PCI probe machinery from virtio-blk; do not invent new PCI scanning. virtio-net feature negotiation should match the modern device spec — Cerebro's research return covers the exact features required for BUILD-1a (likely `VIRTIO_NET_F_MAC`, basic ring layout; not `VIRTIO_NET_F_MRG_RXBUF` or offload features at BUILD-1a).
- **ISC-1a-2.** `NetDevice` trait defined in `kernel/src/net/device.rs`, parallel to `BlockDevice`. Shape per Cerebro's research return Q4. *Implementation note:* descriptor publication discipline must match the AHCI/NVMe pattern from commit `2fdb7cb` — `core::sync::atomic::fence(Ordering::Release)` between descriptor writes and the doorbell MMIO write; `fence(Ordering::Acquire)` after completion. This is non-negotiable; the BUILD-5b bug recurring in virtio-net would be worse than not having a net stack at all.
- **ISC-1a-3.** smoltcp wrap smoke test: an Ethernet frame submitted via the wrap reaches virtio-net's TX queue and emerges on QEMU's net backend; a frame received on RX is delivered up through the wrap to a kernel test consumer. *Implementation note:* this is the early-gate for R1 (smoltcp poll-loop fit). If the smoke test reveals friction (the wrap exceeds ~200 lines, polling shape doesn't compose cleanly with our scheduler, etc.), **stop and surface to PM** — do not work around. Q1 reopens.
- **ISC-1a-4.** Phase 3 sweep stays green: `NEO_SMP=4 --no-meta` passes without regression. Run before pushing.
- **ISC-1a-5.** No syscall surface exposed to userspace. The whole BUILD-1a output is kernel-internal. No new entries in the syscall table.

## 3. Code-Location Hints

- `kernel/src/lib.rs` — add `mod net;` and `mod drivers::net;` (or wherever the driver tree is mounted).
- `kernel/src/net/mod.rs` — module entry; re-exports.
- `kernel/src/net/device.rs` — `NetDevice` trait + supporting types.
- `kernel/src/net/smoltcp_wrap.rs` (or similar) — the scheduling shim. Keep this file small.
- `kernel/src/drivers/net/mod.rs` — driver tree mount.
- `kernel/src/drivers/net/virtio_net.rs` — the driver itself.
- `Cargo.toml` (kernel crate) — add the smoltcp dependency.
- `kernel/src/drivers/block/virtio_blk.rs` — **read this first** for the PCI/MSI-X/DMA pattern to mirror. Do not modify it.
- `kernel/src/drivers/block/ahci.rs` (commit `2fdb7cb`, lines ~423 and ~447) — **read this** for the publication-fence pattern.

## 4. `/simplify` Discipline (per PM-DECISION §6)

- No machinery beyond what each ISC requires. If a feature isn't required by an ISC, it doesn't ship in BUILD-1a.
- **No IP layer code.** That's BUILD-1b. The smoltcp wrap's smoke test is at the *Ethernet frame* level, not the IP-packet level.
- **No syscalls.** That's BUILD-1c.
- **No capability gating.** That's BUILD-1c.
- **No multi-queue virtio-net.** Single RX queue + single TX queue at BUILD-1a. Multi-queue is a later sub-commit if it ever becomes needed.
- **No offload feature negotiation** (checksum offload, TSO, GSO, LRO). Plain frames.
- **No interrupt coalescing.** Defer.
- **Wrap shim line-count gate:** if your smoltcp wrap shim grows past ~200 lines, **stop and surface to PM**. That is the signal that the wrap posture (Q1) may need revisiting. Do not fight the line count by compressing readability; surface the issue.

## 5. Anti-Scope

- IP layer, ICMPv6, ND, SLAAC, RA processing, RDNSS lift — all BUILD-1b.
- UDP, capability gating, syscalls (`sys_net_*`) — all BUILD-1c.
- Loopback short-circuit, `sys_net_stats`, per-process counters — all BUILD-1d.
- Address rotation, deprecation events — BUILD-1.1.
- IPv6 extension-header processing — BUILD-1b/1f (one fused dispatch).
- Real-hardware verification (bare-metal) — BUILD-1e.
- e1000e, USB Ethernet, any non-virtio driver — Phase 4 later BUILDs.
- TCP scaffolding of any kind — BUILD-3. The speculative TCP sketch is informational only; do not implement against it.

## 6. Return Contract

Match the Phase 3 BUILD-5b Round-2 return format (the one that proved correct on hardware):

```
Root cause / scope summary: <one paragraph describing what BUILD-1a delivered and any non-obvious decisions>

Commit(s):
<full-sha> <subject>   (name the BUILD-1a sub-commit; reference PM-DECISION in body)

Validation:
- cargo k: PASS (or appropriate equivalent for net-module kernel build)
- cargo test: any new unit tests pass
- QEMU smoke test (ISC-1a-3): PASS — Ethernet frame round-trip observed
- NEO_SMP=4 --no-meta: PASS — no Phase 3 regression
- smoltcp wrap shim line count: <N> lines (under ~200-line gate)

Confidence basis (required, since BUILD-1a is substrate-only):
<why you believe the NetDevice trait shape and the wrap shim are right — reference Cerebro's RESEARCH artifact + virtio-blk pattern>

ISC verification:
- ISC-1a-1: <evidence — serial-log excerpt of virtio-net probe>
- ISC-1a-2: <evidence — trait file path + fence placement line numbers>
- ISC-1a-3: <evidence — smoke test output / log path>
- ISC-1a-4: <evidence — sweep pass>
- ISC-1a-5: <evidence — confirmation no new syscalls>

Open questions / forward markers for BUILD-1b:
<anything you discovered during 1a that BUILD-1b should know>
```

If any ISC cannot be met, **stop and surface to PM** rather than committing a partial pass. Do not declare BUILD-1a closed unless all five ISCs hold.

## 7. Do NOT

- Start before Cerebro's RESEARCH return is on `main`.
- Push to `origin/main` if any ISC fails or the publication-fence pattern is not in place. PL push authorization is conditional on the return contract being clean.
- Implement against the speculative TCP sketch. It's a hypothesis-validation artifact, not a commitment.
- Modify `kernel/src/drivers/block/virtio_blk.rs` or any Phase 3 code, *except* if Cerebro's research surfaces a publication-fence bug in virtio-blk — in which case stop, surface to PM, do NOT bundle the fix into BUILD-1a (separate BLOCKER artifact).
- Add anything outside the ISC scope on grounds of "while I'm here" — Article 15.7 lean-patch rule.

## 8. Acceptance Gates PM Will Check

Same cadence that proved out Phase 3 BUILD-5b:

1. Commit on `origin/main`, branch `main`, not detached.
2. Body names the BUILD-1a sub-commit and references PM-DECISION.
3. Diff matches stated root cause — PM inspects every changed file; deltas only in `kernel/src/net/`, `kernel/src/drivers/net/`, `Cargo.toml`, and minimal scaffold edits.
4. `NetDevice` trait method bodies use the `fence(Release)` / `fence(Acquire)` discipline matching `2fdb7cb`.
5. smoltcp wrap shim is under ~200 lines OR you have surfaced the line-count concern to PM as a Q1 re-open.
6. QEMU smoke test (ISC-1a-3) log present and parseable.
7. `NEO_SMP=4 --no-meta` clean.
8. No syscall additions.

Bare-metal verification is **not** required for BUILD-1a close — that's BUILD-1e's job. QEMU smoke is the BUILD-1a gate.

---

**End of dispatch.**

*Jarvis: BUILD-1a substrate — `NetDevice` trait, virtio-net driver, smoltcp wrap shim, frame round-trip smoke test in QEMU. Five ISCs. Publication-fence discipline inherited from `2fdb7cb`. ~200-line wrap-shim gate. Gated on Cerebro's RESEARCH return. PM stands by for your report.*
