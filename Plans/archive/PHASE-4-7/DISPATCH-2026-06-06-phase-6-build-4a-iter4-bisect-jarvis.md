<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a iteration 4 — bisect ring-execution vs compute dispatch → Jarvis

**Date:** 2026-06-06
**From:** PM/Cerebro
**To:** Jarvis (continue on `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`, new commit on top of `6ef5cb4`)
**Status:** DISPATCHED — iter3 localized the wall; iter4 bisects it. Two-stage gate (Stage B PL-driven).
**Builds on:** BUILD-4a iter3 (`6ef5cb4`). Evidence: `Plans/evidence/accel_intel_add_bringup.optiplex.extracted.log` (sha256 `F63ED169…`).

---

## 0. Where iter3 got us (real progress)

Full telemetry, no hang. The hardware submission stack now **works**: forcewake awake (`FORCEWAKE_ACK=1`, `GT_FORCEWAKE_ACK=1`), **legacy ring mode active** (`RING_MODE=0`, `execlist_enabled=false`), **no fault** (`RING_FAULT=0`), ring valid (`RING_CTL=0x1`), `RING_TAIL=0xa8` (full 168-byte batch written), GGTT PTEs written for kernel/inputs/output/seqno. But: **`RING_HEAD=0x50` and `ACTHD=0x50`, identical at submit-status *and* wait-timeout** (zero progress during the wait), `seqno=0` (never written), `INSTDONE=0xffdeffff` (a unit not idle). `HEAD (0x50) ≠ TAIL (0xa8)`.

**The ambiguity to resolve:**
- **(A)** engine consumed to `0x50`, dispatched the GPGPU compute, and the **EU threads never retired** (no End-Of-Thread / wrong kernel pointer) → engine parks, seqno never written.
- **(B)** engine **never consumed the batch** (`HEAD=0x50` was the starting head) → a ring-execution/setup problem, compute never reached.

`INSTDONE` not-idle + GGTT mapped lean toward (A), but it is not conclusive. iter4 settles it.

## 1. Objective

**Bisect ring-execution from compute dispatch**, and either PASS or definitively localize to one side. Center on a no-compute smoke test; fix the prime compute suspect (EU-kernel EOT) opportunistically.

## 2. Implementation Success Criteria

- **ISC-1 — Ring smoke test (the bisector).** Add a path (selected by the scenario / a sub-mode) that submits to the same ring **only a `PIPE_CONTROL` post-sync seqno store — no `MEDIA_VFE_STATE`/`INTERFACE_DESCRIPTOR`/`GPGPU_WALKER`, no EU kernel.** Run it before/instead of the compute batch and log its result distinctly (`smoke-seqno=0x…`).
  - **Seqno lands** → the ring/forcewake/submission path executes correctly; the bug is **isolated to the compute dispatch** (→ ISC-3/4 are the fix).
  - **Seqno does not land** → **ring-execution setup** is the bug (HEAD/CTL/ring-start/wraparound) — that becomes iter5's focus.
- **ISC-2 — Disambiguate HEAD progress.** Log `RING_HEAD`/`ACTHD` **before** the kick and the ring byte-offset the batch is written at; sample `RING_HEAD`/`ACTHD`/`INSTDONE` at ≥3 points across the wait (post-kick, mid, timeout) so "static vs advancing" is unambiguous in the log.
- **ISC-3 — Log values, not just offsets.** Emit the **GGTT PTE values** written (physical address + flags) for the kernel, each input, output, and seqno page; and the **interface-descriptor contents** (kernel start pointer, thread count, SIMD mode). Assert/log that the descriptor's kernel pointer **equals** the GGTT address the kernel binary is mapped at.
- **ISC-4 — Verify the EU kernel terminates.** Inspect the compiled `ADD_F32X2_KERNEL_GEN9` (from `add_f32x2.visa`): confirm it ends with a proper **End-Of-Thread** send (the textbook cause of "threads dispatch, never retire, engine parks, no fault, seqno never written"). If the EOT is missing/wrong, recompile per the pinned recipe and update the embedded blob + the provenance README + the command-byte oracle if the bytes shift. Document the EOT instruction in the kernel README.
- **ISC-5 — Standards / gate.** `accel-intel` default-off; SPDX; no new kernel dep; SAFETY+PRM citations on new MMIO; `cargo fmt --all --check`; host-checks + `shard-coverage 4` green; full sharded suite green with `accel_intel_add_bringup` hardware-only; the command-byte oracle stays green (re-laid if the EU kernel/bytes change). Record the iter3 readout + the iter4 bisect in ADR-032.

## 3. Close gate (two-stage)

- **Stage A (you):** builds with `accel-intel` on; oracle green; CI suite + host-checks green feature-off; fmt clean. Push; PM reviews the smoke-test path + EU-kernel/EOT + value-logging.
- **Stage B (PL):** fresh OptiPlex run. The NVMe log shows **either** `accel_intel_add_bringup` PASS (readback `== [4.0,6.0]`) — best case, the EOT fix landed it — **or** the smoke-test result that bisects ring-vs-compute plus the richer readout. **Merge only on PASS.**

## 4. Scope guard

Still the single `Add` bring-up. The smoke test is a **diagnostic sub-path**, not a new feature; no op-set breadth, **no ELSP** (legacy ring is proven to *load*; the question is execution/compute), no display. The deliverable: the next run either passes or tells us, unambiguously, whether to fix **ring execution** or **the EU compute dispatch** — no more guessing between the two.
