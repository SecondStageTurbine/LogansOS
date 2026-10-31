<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: GPU P1 — fold ADR-077 amendments (v2) + build Slice 1 (CPU blur floor) → Cerebro

**Date:** 2026-07-21
**From:** PM (Claude)
**To:** Cerebro (main-tier; OT-63 GPU owner, ADR-077 author)
**Status:** DISPATCHED — **doc revision + kernel-touching build.** Chad's cross-check (`docs/RESEARCH-2026-07-21-gpu-p1-adr-crosscheck-FINDINGS.md`) = **RATIFY-AFTER-AMENDMENT**: your architectural cut is correct, 7 bounded amendments to fold before Slice 1. This dispatch folds them into **ADR-077 v2** and builds **Slice 1 = the CPU blur floor** (the bit-for-bit reference the GPU kernel must later match; no GPU, no ocloc yet).

## Part A — fold the 7 amendments into ADR-077 v2 (doc, no gate)
Read the cross-check FINDINGS in full, then amend `docs/ADR-077-gpu-p1-compute-service.md`:
1. **Ship the literal 8×8 byte oracle** (the amendment that most matters — the ADR *claims* an oracle but supplies no bytes, so it's un-recomputable). Add literal rows for **source, expected destination, AND the horizontal intermediate**. State byte order explicitly: **XRGB8888 LE → memory bytes `BB GG RR FF`**. The fixture must exercise: all four clamp edges + corners; **odd quarter-sums** that hit `round_half_up` (`+32768 >> 16`), not only divisible values; **unequal R/G/B** (catch channel-order bugs); a **nontrivial X input byte** proven ignored → output `0xFF`; and the **separately-asserted H-pass bytes** so H and V can't share a compensating error. The `r=1` arithmetic is `round_half_up((left + 2·center + right)/4)` per pass after edge replication — compute the fixture from a real source matrix, don't hand-wave.
2. **Explicit kernel edge bounds checks.** GPGPU_WALKER thread masks are NOT a memory-safety boundary — the kernel must bounds-check the neighbor gather at clamp edges explicitly (rounded/partial edge groups included).
3. **Damage = radius-expanded output region + source halo.** A blur of radius r spreads; the committed damage rect is the changed cell **expanded by r**, and the source read is a **halo** around it. Spec both.
4. **Pre-VT-d execution must be WHOLLY kernel-internal.** A `Handle::GpuCompute` + service syscall *is already a userland compute queue* — so it **cannot ship until the graphics VT-d fence is up**. Re-sequence: the pre-fence blur is an internal kernel call path only; the cap + syscall are gated behind Slice 5 (VT-d), not exposed in the risk-accepted bring-up stage.
5. **The VT-d graphics domain must cover ALL GPU-reachable allocations** — ring, LRC, HWSP, state, kernel, CURBE, seqno, page tables — not merely the 3 pixel buffers. A GPU fault reaches governance memory through the context/ring structures too.
6. **Two-pass coherence needs a PRM-derived command-byte oracle** (PIPE_CONTROL/MOCS bytes from the Intel PRM, pulled from raw source — the exact-bytes rule, no WebFetch), and the H-pass **diagnostic must not perturb the primary H→V hazard** it's checking.
7. **Slice 3 names a Linux/amd64 ocloc host.** The ocloc container is Linux-only (P2 finding). Name the concrete runner — the **<runner-host> Docker-Linux runner Janus is standing up is exactly this host**; reference it (or a tested WSL2/Docker-Linux path).

## Part B — build Slice 1: the CPU blur floor (kernel-touching, gated)
The first ladder slice: the **CPU-side separable Gaussian blur** over the composited G1 `SharedPixelBuffer` backdrop — the cap-native software floor AND the bit-for-bit reference the GPU kernel must match. Scope:
- Implement the two-pass H→V Q16 blur (per the v2 spec) in the compositor's CPU path.
- **Assert against the literal 8×8 byte oracle** from Part-A amendment 1 (source → H-intermediate → dest, all three).
- **Damage-halo (amendment 3):** commit the radius-expanded rect, read the source halo.
- **Kill-switch (Rule-1):** blur-disabled ≡ the solid Reduce-Transparency floor, bit-for-bit; ship the equality regression.
- **I-14 / governance:** CPU-only, no device DMA at this slice — state the I-14(a/b/c) analysis (c is N/A until the GPU slices; say so).
- Explicit edge bounds checks (amendment 2) in the CPU kernel too.

## Close-gate + merge
- Run `scripts/worker-close-gate.sh` (rebase-fresh, the exact CI host-check set, SPDX, self-stat + evidence block) — the two hard rules apply: **green CI URL** (kernel-touching ⇒ the `workflow_dispatch` 4-shard QEMU gate), and the PM inspects the commit's own `--stat`.
- **PM will independently recompute the 8×8 byte oracle** before merge (the oracle's whole value is independent recomputation — expect it).
- Kill-switch equality regression is mandatory (a policy/substrate build).
- **Disk reclaim:** `Workers/cerebro` is your persistent per-seat tree — reused, not deleted; note it in the RETURN.
- Push Part-A (ADR v2) + Part-B (Slice 1) on `codex/gpu-p1-adr-v2-slice1-cerebro`; PM cherry-picks the ADR + merges the build after gate + AAG.

## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-21-gpu-p1-adr-v2-and-slice1-cerebro.md`. The cross-check to fold: `git show origin/main:docs/RESEARCH-2026-07-21-gpu-p1-adr-crosscheck-FINDINGS.md`. Your ADR to amend: `…:docs/ADR-077-gpu-p1-compute-service.md`. Context: `…:docs/RESEARCH-2026-07-21-gpu-p2-vello-compiler-closure-FINDINGS.md` (ocloc/Linux-host). Work in `Workers\cerebro`; branch `codex/gpu-p1-adr-v2-slice1-cerebro`.
- **One-line:** **Fold Chad's 7 RATIFY-AFTER-AMENDMENT amendments into ADR-077 v2, then build Slice 1 (CPU blur floor).** The load-bearing amendments: **ship the actual literal 8×8 byte oracle** (source+H-intermediate+dest, memory order `BB GG RR FF`, odd quarter-sums, unequal RGB, X-ignored, H separately asserted — PM will independently recompute it); **explicit kernel edge bounds checks** (walker masks aren't a safety boundary); **damage = radius-expanded rect + source halo**; **pre-VT-d must be wholly kernel-internal** (a GpuCompute handle+syscall is already a userland queue → gate the cap behind the VT-d fence); **VT-d domain covers ALL GPU-reachable allocs** (ring/LRC/HWSP/state/kernel/CURBE/seqno/page-tables); **two-pass coherence needs a PRM command-byte oracle** (raw source, no WebFetch) + non-perturbing diagnostic; **Slice 3 names the Linux ocloc host** (the <runner-host> runner). Then Slice 1 = the CPU two-pass Q16 blur floor asserted against that oracle, damage-halo, kill-switch (blur-off ≡ solid Reduce-Transparency floor bit-for-bit). Close-gate ritual (`scripts/worker-close-gate.sh`) + green 4-shard QEMU URL.
- **RAG:** `atrium SharedPixelBuffer backdrop blur damage commit`, `Reduce Transparency floor compositor`, `accel intel_gen9 ocloc`, `dma_arena fence vt-d graphics domain`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read (PL owns RAG startup; never a blocker).
