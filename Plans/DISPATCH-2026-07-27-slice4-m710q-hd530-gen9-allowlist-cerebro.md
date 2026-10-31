<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Slice-4 EU — accept the M710q HD 530 (Gen9) in the Intel accel probe → Cerebro

**Date:** 2026-07-27
**From:** PM (Claude)
**To:** Cerebro (Claude, main-tier — GPU-p1 background; freed from 2(b))
**Status:** DISPATCHED. Slice-4 EU bare-metal iteration N+1. Janus's M710q run 1 was a **conclusive FAIL for a clean reason — a PCI device-ID allowlist miss, not an EU/compute bug.** The scenario never reached the EU.

## Run-1 evidence (Janus, M710q, `0eae1fc9`)
- `accel: intel scan complete functions=13 candidates=0 probe_ok=0` → `FAIL reason=blur_silicon_accel_intel_missing`. Never reached `BLUR_WALKER_KICKS` / byte-equality / `fence_refusals`.
- **M710q GPU = Intel HD 530 `[8086:1912]`** (Skylake, **Gen9**, GT2). Branch allowlist accepts only `0x9bc8 | 0x9bc5` (Comet Lake UHD 630) + `0x3e92 | 0x3e98` (Coffee Lake UHD 630) — all **Gen9.5**, the OptiPlex family. `0x1912` isn't in it → no accepted candidate.
- Boot hygiene was correct: WWN-guarded write to Samsung `0x5002538d4070972f`, USB protected; AHCI sink valid (`region_size=16777216, cursor=3930, degradation_reason=0`), extracted to `…/accel_intel_blur_silicon.m710q.run1.ahci-log-region.raw`. ISO SHA-256 `097eb19b…`.

## Scope
1. **Add `0x1912` (HD 530, Skylake Gen9 GT2) to the Intel accel probe allowlist.** Base on Chad's gate-green Slice-4 tip `0eae1fc9` (`codex/gpu-p1-slice4-gen9-eu-silicon-gate-chad`) — coordinate with Chad (his rebase is done; take the branch or stack a followup on it, your call, but don't fork the EU work).
2. **Verify Gen9 (Skylake HD 530) vs Gen9.5 (UHD 630) EU-path compatibility — the real work, not the one-liner.** The allowlist add lets the probe *accept* the GPU; it does NOT guarantee the bring-up path (written/tested for UHD 630) runs on Skylake. Same EU compute ISA (Gen9 graphics), so byte-equality should transfer — but check the deltas that bite: GGTT/PPGTT setup, MMIO/register offsets, the ring/execlist submission, any per-SKU workarounds, and the GT2 EU/thread counts if the blur kernel assumes them. Determine: is the accept-ID sufficient, or does Skylake need adaptation?
3. **Prior-art trap-mine (driver-dispatch rule):** mine i915 for **SKL (Skylake) vs CFL/CML (Coffee/Comet Lake)** differences on the compute/EU submission path — read for principle, no GPL copy, raw source directly (not WebFetch). Seed any SKL-specific trap into the discriminator bank before the next boot.

## Standing rules
- **Kill-switch:** EU-accel OFF ≡ CPU floor, bit-for-bit (unchanged). Adding a device ID must not alter the OFF path.
- **I-14(c):** every EU DMA buffer through `dma_arena::fence(...)` under IOMMU; `fence_refusals=0`. HD 530's IOMMU is the same VT-d generation — confirm.
- **Non-vacuous proof:** PASS requires `BLUR_WALKER_KICKS>0` (the EU actually executed on HD 530) AND byte-equality vs the CPU floor AND `fence_refusals=0`. A green that never kicked the EU is vacuous.

## Blind-hardware discriminator bank (next M710q boot must be conclusive)
| Observation | Meaning | Discriminator |
|---|---|---|
| `candidates>=1 probe_ok=1`, EU kicks, byte-equal | HD 530 Gen9 path works — **Slice-4 PASS** | `BLUR_WALKER_KICKS>0` + equality + `fence_refusals=0` |
| Probe accepts, but bring-up fails before EU | Skylake-specific delta (GTT/MMIO/ring) | capture the failing stage + the SKL register/offset it died on |
| EU kicks but byte-mismatch | real Gen9 EU compute divergence | first divergent byte offset |
| `fence_refusals>0` | HD 530 DMA bypassed the fence | I-14(c) containment signal, not an EU bug |

## Close-gate + merge
`scripts/worker-close-gate.sh`, green push-CI, 4-shard `workflow_dispatch` (kernel/GPU). Device-less QEMU can't exercise HD 530 — the **real proof is the next JOINT M710q boot** (Janus drives, per `docs/runbook-baremetal-m710q-gen9-eu.md`). **No auto-merge** — PM verify + **AAG (DMA-fence / mem-safety lens)** before it lands. Disk-reclaim after.

## Relay — Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-27-slice4-m710q-hd530-gen9-allowlist-cerebro.md`. Base on Chad's `0eae1fc9`.
- **One-line:** Janus's M710q Slice-4 run FAILED cleanly — **device-ID allowlist miss, not a compute bug**: the M710q is HD 530 `[8086:1912]` (Skylake Gen9), the allowlist only accepts Coffee/Comet Lake UHD 630 (Gen9.5), so the probe rejected it (`candidates=0`) and never reached the EU. Add `0x1912` to the Intel accel allowlist, **then verify the Gen9-vs-Gen9.5 EU-path actually runs on Skylake** (same EU ISA so byte-equality should transfer, but check GGTT/MMIO/ring/workaround deltas — trap-mine i915 SKL-vs-CFL/CML, raw source not WebFetch). Keep EU-OFF≡CPU-floor bit-identical, `fence_refusals=0`, non-vacuous (`BLUR_WALKER_KICKS>0`). Coordinate with Chad on his branch. Close-gate + 4-shard QEMU; real proof is the next JOINT M710q boot; no auto-merge (PM + DMA-fence AAG).
- **RAG** (PowerShell) health-check first; e.g. `intel accel probe allowlist device id 0x1912 gen9`, `gpu ggtt ppgtt skylake coffee lake mmio`, `blur walker kicks eu dispatch execlist`. If down, note it + grep/Read.
