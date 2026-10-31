<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Slice-4 EU silicon gate — rebase-fresh + re-gate onto current `main` → Chad

**Date:** 2026-07-26
**From:** PM (Claude)
**To:** Chad (Codex family — the Slice-4 Gen9 EU silicon-gate author)
**Status:** DISPATCHED — **software half of Slice-4 EU readiness.** The hardware half (a powered M710q Gen9 target) is a separate PL gate; this lane makes the *branch* ready so hardware is the only remaining blocker. **No bare-metal run in this dispatch** — QEMU/host gate only.

## Why now
The PL wants Slice-4 EU byte-equality (Gen9 EU compute == CPU floor, byte-for-byte; the `BLUR_WALKER_KICKS`/`BLUR_COMPLETIONS` discriminator) staged for the next bare-metal session. The candidate branch is stale and never merged:
- `codex/gpu-p1-slice4-gen9-eu-silicon-gate-chad` @ `493dbd5` (2026-07-22) is based on `6b23974` — **82 commits behind current `main`** (pre-C1.0 / pre-Slice-6 / pre-consolidation).
- `BLUR_WALKER_KICKS` is **not on `main`** — it lives only on this stale branch.
A silicon build off a 4-day-old base is a stale-base merge tax waiting to happen, and it can't pick up the C1.0/Slice-6/consolidation fixes. Refresh first, then it's one rebase away from mergeable when the M710q is up.

## Scope
1. **Rebase-fresh** `codex/gpu-p1-slice4-gen9-eu-silicon-gate-chad` onto current `origin/main`. Resolve conflicts against the merged C1.0 / Slice-6 / Oracle-consolidation state (the GPU/EU path may have moved under you — reconcile, don't clobber).
2. **Re-verify the byte-equality proof still holds** post-rebase: the EU compute path must produce **byte-identical** output to the CPU floor for the covered kernels, with the `BLUR_WALKER_KICKS` / `BLUR_COMPLETIONS` discriminator instrumented (the run must be conclusive whether EU actually executed vs silently fell back to CPU — a green byte-equality that never kicked the EU is a vacuous pass).
3. **Kill-switch:** EU-accel OFF ≡ CPU floor, bit-for-bit (the standing substrate-build invariant). Prove disabled ≡ prior layer.
4. **M710q log-sink disk role → AHCI/SATA (do this in the rebase).** The Gen9 target is now up (M710q, Ubuntu, `<m710q-ip>`) — see `docs/runbook-baremetal-m710q-gen9-eu.md` (`git show origin/main:docs/runbook-baremetal-m710q-gen9-eu.md`). It has **no serial and no NVMe**, so the durable capture channel is the **disk log sink on the SATA SSD `/dev/sda`** (offset 256 MiB), read back from the live USB. The Slice-4 scenario's **log-sink disk role must target the AHCI/SATA disk**, not NVMe, for the M710q ISO — confirm the build manifest emits `log_sink_offset=0x10000000, size=16777216` on the AHCI disk role. Wire this now so the M710q ISO is capture-ready without a second build round. (The framebuffer is the secondary glass read.)
5. **No hardware run in THIS dispatch.** It ends at a green **4-shard `workflow_dispatch` QEMU gate + host** on the rebased SHA. The silicon bring-up on the M710q is a later JOINT session, run per `docs/runbook-baremetal-m710q-gen9-eu.md` (note the **INVERTED device-name danger** there: on the M710q `/dev/sda` IS the dd target and `/dev/sdb`/USB is protected — the opposite of the OptiPlex; identity-check by model/WWN).

## Blind-hardware discriminator note (for the FUTURE silicon session, not this build)
The bare-metal EU run is blind-hardware (MMIO/breadcrumb only). Pre-seed the bank now so the future session is diagnostic on failure:
- **EU never kicks (silent CPU fallback):** `BLUR_WALKER_KICKS==0` while output still byte-matches → the proof is vacuous; discriminator = the kick counter must be nonzero AND equality must hold.
- **EU kicks but diverges:** `BLUR_WALKER_KICKS>0` + byte-mismatch → real EU compute bug; capture the first divergent byte offset.
- **EU hang / no completion:** `BLUR_WALKER_KICKS>0`, `BLUR_COMPLETIONS<KICKS` → EU dispatched but didn't retire; capture the walker/DMA-fence state.
- **DMA-fence (I-14c):** every EU DMA buffer routes through `dma_arena::fence(...)` under IOMMU containment — a fence refusal is a governance-containment signal, not an EU bug. `fence_refusals` must be 0.

## Close-gate + merge
`scripts/worker-close-gate.sh` (rebase-fresh on `origin/main` is step 1 — this whole dispatch is that step made explicit), green push-CI, **and the mandatory 4-shard `workflow_dispatch` QEMU gate on the exact rebased SHA** (kernel/GPU-touching). Paste both green CI URLs in the RETURN. Branch: same (`codex/gpu-p1-slice4-gen9-eu-silicon-gate-chad`), force-with-lease after rebase is fine (it carries only un-merged work). Disk-reclaim after PM confirms merge.

**Do NOT auto-merge** — this is a substrate/GPU-touching branch; it goes through PM verify-before-merge + the adversarial-audit gate (GPU DMA-fence / memory-safety lens) before it lands.

## Relay — what to tell Chad
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-26-slice4-eu-rebase-regate-chad.md`. Your branch `codex/gpu-p1-slice4-gen9-eu-silicon-gate-chad` @ `493dbd5` is 82 commits behind `main`.
- **One-line:** **Rebase your Slice-4 Gen9 EU silicon-gate branch onto current `main` (82 commits fresh: C1.0 + Slice-6 + consolidation landed) and re-gate it — QEMU 4-shard + host only, no bare-metal this round.** Reconcile the GPU/EU path against the merged state, re-prove byte-equality (EU==CPU floor) with `BLUR_WALKER_KICKS`/`BLUR_COMPLETIONS` proving the EU actually executed (a green that never kicked the EU is vacuous), keep the EU-OFF≡CPU-floor kill-switch bit-identical. Close-gate + push-CI green + the `workflow_dispatch` QEMU gate on the rebased SHA; paste both URLs. No auto-merge — PM verify + AAG (DMA-fence/mem-safety lens) before it lands. The M710q silicon run is a later JOINT session once the box has a PSU.
- **RAG** (PowerShell): health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `gpu EU execution unit walker kick blur completion byte equality`, `dma_arena fence IOMMU gpu buffer`, `eu accel kill switch cpu floor fallback`. If down, note it in the RETURN + grep/Read (never a blocker; the PL owns RAG startup).
