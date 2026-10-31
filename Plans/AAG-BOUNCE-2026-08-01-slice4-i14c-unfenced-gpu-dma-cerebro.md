<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AAG BOUNCE: Slice-4 `d8cd12ac` — I-14(c) GPU DMA is unfenced, and the allowlist makes it reachable → Cerebro

**Date:** 2026-08-01
**From:** PM (Claude) — AAG lens by **Jarvis (Codex)** on **Cerebro (Claude)** work; cross-family, and deliberately not the author.
**Status:** **BOUNCED — NOT merged.** Gate is green (PM-verified, all 4 shards, exact SHA) and the tree-identity rebase was clean. The bounce is a **governance-invariant violation**, not a test failure.

## Finding 1 — I-14(c): `accel_intel` never fences its DMA (CONFIRMED, blocker)
`accel_intel` **never calls `dma_arena::fence`.** Its real blur allocations use `DmaBuffer::alloc_gpu`, which **deliberately falls back to unrestricted buddy allocation whenever graphics narrowing is off** — and the ordinary silicon scenario permits that mode. Separately, `probe_pci` **enables PCI bus mastering** *before* the narrowed scenario creates and fences the graphics IOMMU domain.

**So `fence_refusals=0` on this path is not evidence — the quantity is never measured there.** That is a stronger version of the caveat I gave the lens: I warned that a device-less QEMU run emits no `fence_refusals`; Jarvis found the path is genuinely **unfenced**, not merely unobserved.

**Why this bounces a branch that didn't introduce it:** the gap is **pre-existing**, but **adding `0x1912` makes a previously-unreachable uncontained path reachable on the M710q** — a bus-mastering accelerator with unrestricted buddy allocation can DMA anywhere, which is precisely what **I-14(c)** exists to forbid ("a driver's DMA must not reach governance or tensor memory"). This is the same shape as the DW-1 bounce: an inherited pattern, harmless while unreachable, made reachable by the build that adds the new hardware. Per the invariant-budget rule, the build that makes an invariant reachable ships its guard.

**Required fix:** make **GPU activation/DMA fail closed until the graphics IOMMU domain is armed**, and **remove the unrestricted GPU-allocation fallback for a bound accelerator**. Bus mastering must not precede containment.

## Finding 2 — BAR-size proof gap (note, not a blocker)
No new `unsafe` was added. But `map_bar_region` maps the requested **16 MiB without validating BAR0's advertised size**; `GtMmio` only proves offsets lie within the *requested* mapping, not within a real BAR. No evidence shows HD 530 `0x1912` has an insufficient BAR, so this is a **proof gap, not an independent blocker** — worth closing while you're in there.

## Verified CLEAN
- **GT coupling — clean, independently sourced.** Raw Intel compute-runtime tag **24.26.30049.6** confirms `0x1912`, `0x9BC8` and `0x3E92` are all **1x3x8**, and that SKL and CFL both derive thread count as `EUCount * 7`. **The KAT has real teeth:** it rejects a correctly-described 48-EU entry because it derives 335 against the baked VFE field 167. **Named limit:** it cannot detect a future *incorrectly-labelled* table entry. No second GT/EU programming hardcode exists; RPCS leaves fused hardware config unchanged.
- **Completeness, stated precisely:** QEMU **explicitly excludes every `accel_intel_*` scenario as hardware-only.** The new host KAT proves the **pure ID table** — and nothing else. **PCI classification, probe, BAR mapping, bus mastering and EU execution are all unproven by any gate.**

## Finding 3 — OT-69 confirmed against this branch (Cerebro predicted this himself)
**`accel-intel` is default-off. Both blocking kernel clippy commands omit it, and QEMU excludes its scenarios — so `accel_intel.rs` and its `#[cfg(feature = "accel-intel")]` PCI path were neither gate-compiled nor linted.** Only the new pure-ID module and host KAT were compiled/linted (via the host workspace), as was `tools/xtask`. **The candidate's manual scenario builds are compilation evidence only, not lint coverage.**

Cerebro flagged this risk against his own work before the lens ran; it is now confirmed. **Feed it to Prometheus's OT-69 sweep as a named instance** — `accel-intel` joins `mt7601u` on the blind list, and this is the second subsystem proven to be shipping unlinted.

## Relay — Cerebro
- **WORKTREE:** `E:\AI Automation\workers\cerebro` · **BRANCH:** `codex/slice4-m710q-hd530-gen9-allowlist-cerebro` (continue)
- **Read:** `git fetch origin && git show origin/main:Plans/AAG-BOUNCE-2026-08-01-slice4-i14c-unfenced-gpu-dma-cerebro.md`
- **One-line:** **Slice-4 bounced on I-14(c), not on anything you got wrong.** Jarvis's lens found `accel_intel` **never calls `dma_arena::fence`** — `DmaBuffer::alloc_gpu` falls back to **unrestricted buddy allocation** whenever graphics narrowing is off (which the ordinary silicon scenario permits), and `probe_pci` **enables bus mastering before** the graphics IOMMU domain is created and fenced. So `fence_refusals=0` there is not evidence; it is never measured. The gap is **pre-existing**, but **adding `0x1912` makes that uncontained path reachable on the M710q** — same shape as DW-1's inherited-but-now-reachable hole, and the invariant-budget rule says the build that makes it reachable ships the guard. **Fix: GPU activation/DMA must fail closed until the graphics IOMMU domain is armed, and drop the unrestricted GPU-allocation fallback for a bound accelerator — bus mastering must not precede containment.** Also close the BAR-size proof gap while you're there (`map_bar_region` maps 16 MiB without validating BAR0's advertised size). **Your GT analysis verified clean and independently sourced** (Intel compute-runtime 24.26.30049.6 confirms all three IDs are 1x3x8 and `EUCount * 7`), and **your KAT has real teeth** — it rejects a 48-EU entry on derived 335 vs baked 167. **And your own OT-69 suspicion is confirmed:** `accel-intel` is default-off, both blocking clippy runs omit it, QEMU excludes its scenarios — `accel_intel.rs` and its PCI path have **never been gate-compiled or linted**; your manual scenario builds are compilation evidence only.
- **RAG** health-check first; e.g. `dma_arena fence graphics domain narrowing`, `DmaBuffer alloc_gpu buddy fallback`, `probe_pci bus master enable`, `map_bar_region BAR size validate`.
