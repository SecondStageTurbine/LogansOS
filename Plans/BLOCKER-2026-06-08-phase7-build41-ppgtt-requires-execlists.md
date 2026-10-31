<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# BLOCKER / ESCALATION: Phase 7 BUILD-4-1 — legacy-ring PPGTT does not engage on Gen9; PPGTT requires LRC/execlists

**Date:** 2026-06-08
**From:** Jarvis (worker, OptiPlex bare-metal loop)
**To:** Cerebro / PM — THINK-level scope decision required
**Branch:** `codex/phase-7-build-4-1-ppgtt-positive-jarvis` (Stage-A green; **Stage-B blocked** — not mergeable)
**Dispatch:** `Plans/DISPATCH-2026-06-07-phase-7-build-4-1-ppgtt-positive-jarvis.md`
**THINK:** `Plans/THINK-2026-06-07-phase-7-build-4-ppgtt-spatial-isolation.md` (reframe (A) PPGTT-on-legacy-ring vs (B) execlists — this blocker says (A) is infeasible)

## TL;DR

BUILD-4-1's premise — **"PPGTT on the proven legacy ring, no execlists"** — is **not achievable on this Gen9 (UHD 630) silicon.** Across **three** bare-metal iterations exercising **every** PPGTT mechanism the legacy ring can express, the EU's A64 data-port access read **`select=GGTT`** — i.e. it never resolved through PPGTT. The driver's own fault decode (the discriminator the dispatch mandated) makes this conclusive. Engaging PPGTT on Gen9 requires the **LRC / execlist context** (the PPGTT descriptor lives in the per-context image), which the dispatch's scope guard explicitly forbids me from building. This needs a PM call.

## The self-proving gate worked exactly as designed

The dispatch built the run to be conclusive on failure: data buffers mapped only in PPGTT and **cleared** from GGTT, so a non-engaging access **faults** at the buffer VA, and the fault decode reports the GTT-vs-PPGTT `select` bit + the fault address. Every iteration produced a clean, self-diagnosing fault:

```
ring-fault-decode … RING_FAULT=0x000009e1 fault_type=0(not-present)
                    fault_gpu_va=0x000000130000  addr_space=GGTT  data1=0x10
```

`fault_gpu_va=0x130000` is the lhs input VA; `addr_space=GGTT` is the `select` bit. The EU went to **GGTT** for its data, found the (now-cleared) slot empty, and faulted. INSTDONE flipped to `0xffddffff` (EU active: area bit 17 + instruction-fetch row0 bit 21), so the EU genuinely ran — it just used the wrong address space.

## Evidence — three iterations, every legacy-ring mechanism, same answer

| iter | mechanism | programming | RING_MODE (read-back) | data-access result |
|---|---|---|---|---|
| 2 | **48-bit PPGTT** (4-level PML4 → PDP0) | `GFX_MODE` bit7 **set**; PML4 root → PDP0 via LRI | `0x00000080` (48b on) | fault @`0x130000`, **`select=GGTT`** |
| 3 | **32-bit legacy PPGTT** (3-level, 4 PDP regs → PD) | `GFX_MODE` bit7 **cleared**; 4 PDP regs via LRI(8) | `0x00000000` (48b off) | fault @`0x130000`, **`select=GGTT`** |
| 4 | **Gen7 aliasing PPGTT** (`PP_DIR_BASE`) | GGTT-mapped PD; `PP_DIR_DCLV=0xffffffff`; `PP_DIR_BASE=PD GGTT offset` | `0x00000000` | fault @`0x130000`, **`select=GGTT`** |

In all three, the tables were built correctly (PTE byte-oracle green, on-silicon table dumps match), the root was loaded, the command streamer + EU ran (seqno path / INSTDONE), and `RING_FAULT=0` until the EU's first data access — which always went to GGTT. There is no remaining legacy-ring lever: 48-bit PDP, 32-bit PDP, and the Gen7 `PP_DIR_BASE` are the complete set of non-execlist PPGTT-root mechanisms, and all three are ruled out.

Durable block-sink evidence (extracted off the OptiPlex NVMe):
- `Plans/evidence/phase7-build41-iter2-optiplex-PPGTT-not-engaging-select-GGTT.log`
- `Plans/evidence/phase7-build41-iter3-optiplex-32bit-legacy-PPGTT-also-select-GGTT.log`
- iter-4 (`PP_DIR_BASE`) — serial captured; durable log pending next OptiPlex→Ubuntu reboot.

## Root cause

On Gen8+, the render engine's address space (GGTT vs a per-process PPGTT) is selected by the **logical ring context (LRC)** that execlist submission loads — the PPGTT descriptor is a field in the context image. A pure **legacy ring** (ring-buffer submission, no LRC) has no per-process context, so the engine uses the **global GTT** for all accesses regardless of the PDP / `GFX_MODE` / `PP_DIR_BASE` registers. This matches the Linux i915 architecture: Gen8+ uses execlists for every PPGTT path; the legacy-ring PPGTT registers exist but are not honored for data without a context. BUILD-4a deliberately avoided execlists (the larger lift), and that is exactly why the data port stays on GGTT.

This was anticipated as a *possibility* in both the dispatch failure-mode table (row: `select=GGTT` → "PPGTT never took effect") and the THINK (mode 2). We have now hit it definitively, across every mechanism.

## The decision (PM / Cerebro)

BUILD-4 — the device-boundary governance line — cannot be made real on this hardware without execlists. Options:

1. **Reopen BUILD-4 to include a minimal execlist/LRC bring-up** — construct a Logical Ring Context with the PPGTT descriptor, submit via ELSP, handle context-status. This is the THINK's deferred "(B)": a second bring-up arc (4a took eight iterations; this is comparable). It is the **only** path to a hardware-enforced PPGTT boundary on Gen9.
2. **Defer the device-boundary line** — the governance thesis is already real on the temporal substrate (§9.5 measure, §9.6 arbitrate-enforced, §9.7 own). Mark the page-table-enforced device boundary a documented, hardware-blocked seam (PPGTT-needs-LRC) and close Phase 7 at three of four lines.
3. **Re-scope to a non-PPGTT enforcement** — none exists short of execlists on this silicon (kernel-mediated op-boundary enforcement already shipped in BUILD-3; the device-side boundary is specifically what needs PPGTT/LRC).

**Recommendation:** decide between (1) and (2) at the THINK level. The temporal-substrate thesis is intact regardless; (1) buys the concurrent/hardware-enforced boundary at the cost of an execlist bring-up arc.

## What's on the branch (keep — diagnostic value, not mergeable)

iter-2/3/4 land the reusable diagnostic infrastructure even though Stage-B failed: the PPGTT PTE + PDP-load byte oracles (host-green), the GGTT-clear (so non-engagement faults instead of silently reading stale frames), the GFX_MODE/PDP/PP_DIR_BASE programming, the per-batch PDP-root capture, and the block-sink log extraction. If (1) is chosen, the execlist build reuses the page-table construction + capture wholesale; only the submission path changes. The branch is **not** mergeable (the gate is red by design — PPGTT does not engage), but it should be retained as the conclusive record.

## Stage-A status (unchanged, green)

Builds on+off; PTE + PDP-load byte oracles + 4a Add/matmul/GELU command oracles + §9.5/§9.7 regressions; `shard-coverage`; fmt; clippy. The blocker is purely the Stage-B silicon finding above.
