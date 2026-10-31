<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: AAG lens on Slice-4 M710q HD 530 (`d8cd12ac`) → Jarvis

**Date:** 2026-08-01
**From:** PM (Claude)
**To:** Jarvis (Codex — the cross-family lens on Cerebro/Claude work; **not the author**, which is the point)
**Status:** DISPATCHED — **doc-only adversarial review, no build, no gate.** Last gate before Slice-4 lands on `main`.

**Candidate:** `codex/slice4-m710q-hd530-gen9-allowlist-cerebro` @ `d8cd12ac`. **PM-verified:** gate `30676307779` green on `d9d132b6` (all 4 shards + host + SPDX), code subtrees byte-identical gated→rebased, ff-safe, no deletions, self-stat 10 files +538/−23.
**What it does:** accepts the M710q's **Intel HD 530 `0x1912` (Skylake Gen9 GT2)** in the Intel accel probe; logs the iGPU PCI revision + an SKL-vs-Gen9.5 trap bank; promotes the device-ID allowlist to a host KAT; routes the Slice-4 EU log sink to AHCI (Chad's commit, carried).

## Lenses — REFUTE-default
**1. DMA-fence / I-14(c) — the primary lens.** Does every EU/GPU DMA buffer still route through `dma_arena::fence(...)` under IOMMU containment? Does accepting a *new device ID* open any path that allocates or programs a DMA address **without** the fence — e.g. a probe-time read, the PCI revision log, or a Skylake-specific init step? `fence_refusals` must be provably 0 by construction, not by observation. **Note the trap Cerebro himself flagged for the WiFi lane and which applies here too: device-less QEMU scenarios do no DMA, so a green gate emits no `fence_refusals` at all — a green run is NOT DMA-fence evidence.** Reason about the code path, not the scenario output.

**2. Memory-safety / `unsafe`.** The accel probe and `drivers/pci.rs` changes: any new `unsafe`, raw pointer, MMIO mapping, or slice-from-raw-parts? Can a device that *matches* the new ID but has different BAR sizes/layout cause an out-of-bounds MMIO access? The allowlist widens what we'll bind to — what does binding assume about the device beyond its ID?

**3. The GT-count coupling (the load-bearing claim).** Cerebro's case rests on `0x1912 → SklHw1x3x8` and `0x9BC8/0x3E92 → CflHw1x3x8`, **both 1x3x8 = 24 EU**, so our one GT-dependent hardcode holds. **Verify that independently.** Is there any *other* place that assumes a GT config, EU count, thread count, or subslice layout — and would a future ID with a different GT silently break it? His host KAT reportedly fails on a 48-EU GT3 entry; confirm that KAT actually has teeth (i.e. it would catch the wrong thing).

**4. Completeness — what the green gate structurally cannot cover.** **QEMU has no Intel iGPU**, so the probe/allowlist is *never exercised at runtime* by the gate — which is exactly how a missing device ID reached bare metal in the first place. Name precisely what is and isn't proven, and whether the host KATs genuinely close that gap or merely test the table in isolation.

**5. ⚠ OT-69 blindspot — Cerebro flagged this against his own work.** `drivers/accel/*` sits behind the **`accel-intel`** feature, and the blocking kernel clippy runs **default-features only** — so this code may be in the same CI blindspot that let 27 errors and a planted syntax error survive in the WiFi driver (OT-69). His close-gate ran the same default-features clippy, so it shares the gap. **Confirm whether `accel-intel` is default-on or default-off**, and say plainly whether this branch's code was actually compiled/linted by anything. Not necessarily a merge blocker — but if it wasn't linted, say so, and it feeds Prometheus's OT-69 sweep.

## Rules
A finding **bounces only if it survives your own re-verification**; uncertain ⇒ a note naming the missing fact, never a speculative blocker; **"no findings" requires stating what each lens checked.** Cerebro's two lenses today (DW-1's INV-WM-1 hole, rung-5's boot panic) set the standard — same bar.

## Relay — Jarvis
- **WORKTREE:** `E:\AI Automation\workers\jarvis` (yours alone) · **BRANCH:** none — doc-only.
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-01-aag-slice4-m710q-jarvis.md`; candidate via `git show origin/codex/slice4-m710q-hd530-gen9-allowlist-cerebro:<path>` @ `d8cd12ac`.
- **One-line:** **Cross-family AAG on Slice-4 — the last gate before the M710q Gen9 EU work lands.** Gate green, stat clean, small diff (10 files, +538/−23). Five REFUTE-default lenses: **(1) DMA-fence/I-14(c)** — does accepting a new device ID open any path that programs a DMA address without `dma_arena::fence`? **Reason from code, not scenario output — device-less QEMU does no DMA, so a green gate emits no `fence_refusals` at all and is NOT fence evidence.** **(2) memory-safety** — new `unsafe`/MMIO/BAR assumptions; can a device matching the new ID but with a different BAR layout cause an OOB access? **(3) the GT coupling** — the case rests on `0x1912` and `0x9BC8/0x3E92` both being 1x3x8 = 24 EU so the one GT hardcode holds; verify independently and find any *other* GT/EU/subslice assumption, and confirm the host KAT that rejects a 48-EU GT3 entry has real teeth. **(4) completeness** — QEMU has no Intel iGPU, so the probe is never exercised at runtime (exactly how a missing ID reached bare metal); say what's actually proven. **(5) OT-69 blindspot** — `drivers/accel/*` is behind `accel-intel` and the blocking clippy runs default-features only; **confirm whether this code was compiled/linted by anything at all**, and say so plainly either way. Doc-only, no build, no gate.
- **RAG** health-check first; e.g. `accel_intel probe allowlist device id`, `dma_arena fence gpu buffer graphics arena`, `pci scan bar mmio map accel`, `SklHw1x3x8 CflHw1x3x8 EU thread count`.
