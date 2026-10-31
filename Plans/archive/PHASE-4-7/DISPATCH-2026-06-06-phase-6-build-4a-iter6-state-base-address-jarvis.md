<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a iteration 6 — STATE_BASE_ADDRESS / dispatch-setup fix → Jarvis

**Date:** 2026-06-06
**From:** PM/Cerebro
**To:** Jarvis (continue on `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`, new commit on top of `3c7a9d7`)
**Status:** DISPATCHED — iter5 confirmed SUSPECT A; iter6 fixes it. Two-stage gate.
**Builds on:** iter5 (`3c7a9d7`). Evidence: `Plans/evidence/...iter5...` (extracted-log sha256 `3652128c…`).

## 0. What iter5 proved
The EOT-only **no-op kernel (zero memory access) hung byte-identically to the Add** — `RING_HEAD=ACTHD=0x50`, `seqno=0`, `bisect=dispatch_setup_suspect`. Kills suspect B (EU data access). The widened decode localizes it: **a thread was dispatched to EU row-0 (INSTDONE bit 16) but instruction fetch is stuck (bit 21)**; `SC_INSTDONE`/`SAMPLER_INSTDONE` idle; `TD_CTL=0`; `RING_FAULT=0`. → **the EU cannot fetch the kernel's instructions.**

## 1. Prime hypothesis (high confidence): missing STATE_BASE_ADDRESS
On Gen9 the EU fetches code from **Instruction Base Address + kernel-start-pointer**, and the interface descriptor is read from **Dynamic State Base Address**. If `STATE_BASE_ADDRESS` is never issued before `GPGPU_WALKER`, those bases are unset → the kernel pointer resolves to nowhere → the thread dispatches but stalls at instruction fetch (precisely the bit-16-busy / bit-21-stuck signature). The descriptor we logged is what the *driver wrote*, not what the *GPU read*, so a correct-looking descriptor is consistent with this.

## 2. ISCs
- **ISC-1 — Emit `STATE_BASE_ADDRESS`** before the walker, programming Instruction / Dynamic / General / Surface State Base (and their upper-bound/size fields) to the GGTT regions where the kernel blob + state heap live, so both the descriptor read and the kernel fetch resolve. Hand-verify the command bytes against the PRM.
- **ISC-2 — Re-verify the rest of the dispatch path** now that fetch can resolve: `MEDIA_VFE_STATE` (URB/scratch/max-threads), `GPGPU_WALKER` dims/enables, and the interface-descriptor offset relative to Dynamic State Base.
- **ISC-3 — Re-lay the command-byte oracle.** The command buffer grows (new `STATE_BASE_ADDRESS`); update the hand-computed oracle in `tools/ai-ops-tests` to the new bytes.
- **ISC-4 — Validate via the existing hand-laid no-op.** Success metric for iter6 = **`noop-kernel-seqno` lands** (the no-op thread fetches+runs+EOTs → dispatch fixed). If it *still* stalls at instruction fetch, escalate to PM — likely the hand-laid no-op is itself malformed and we need Prometheus's finalizer-built known-good no-op (dispatched in parallel).
- **ISC-5 — Standards.** Default-off; SPDX; SAFETY+PRM citations; oracle + host-checks + `shard-coverage 4` green; suite green with the scenario hardware-only; ADR-032 records the iter5 result + the STATE_BASE_ADDRESS fix.

## 3. Gate (two-stage) — note the partial-success shape
- **Stage A (you):** builds; re-laid oracle green; CI/host-checks green feature-off; fmt.
- **Stage B (PL):** OptiPlex run. **iter6's win = the no-op retires** (`noop-kernel-seqno` lands). The scenario will still **FAIL on the Add** because that blob is an ASCII placeholder — a real `[4,6]` PASS needs the finalizer-built Add kernel (Prometheus lane → iter7). **So iter6 does NOT merge on its own; the branch merges when the real Add computes `[4,6]`.** Report the no-op result + the EU-decode.

## 4. Scope guard
Dispatch-setup / state-base only (driver-side, `accel_intel` + the command encoder). **Do NOT regenerate the EU kernel blobs** — that's the Prometheus finalizer lane (avoids colliding with the blob consts). No ELSP, no data-path changes (suspect B is ruled out), no display.

## Addendum — 2026-06-07 (convergence: finalizer landed on your branch)

The Prometheus finalizer lane closed and PM **fast-forwarded your branch** to
include it: `codex/phase-6-build-4a-intel-gpu-bringup-jarvis` is now at
`072e0f7` (was `3c7a9d7`). `git fetch origin && git rebase origin/codex/phase-6-build-4a-intel-gpu-bringup-jarvis`
(or branch off the new head) before you build iter6. Three changes to how you run this:

1. **Validate against the FINALIZER no-op, not the hand-laid one.** `NOOP_KERNEL_GEN9`
   is now an Intel-ocloc-finalized, disassembly-confirmed EOT-only kernel
   (`intel_gen9_kernels/noop_eot.gen9.asm` shows the terminating spawner
   `{EOT}` send). The iter5 hand-laid no-op (known to hang) is superseded. So
   ISC-4's "if it still stalls, the no-op may be malformed" escape hatch is
   **closed** — the no-op is now trusted ground truth. If the finalizer no-op
   *still* doesn't retire after STATE_BASE_ADDRESS, the dispatch setup itself
   (VFE/walker/descriptor) is the remaining bug, not the kernel bytes.
2. **iter6 success metric is unchanged** — `noop-kernel-seqno` lands → dispatch
   is fixed. iter6 still does **not** merge alone.
3. **Forward note for iter7 (the Add), so you can shape STATE_BASE_ADDRESS now):**
   the real Add ships **stateless A64** (no BTI/binding-table/surface-state —
   so STATE_BASE_ADDRESS needs Instruction + General State Base, *not* a
   Surface State heap for the args). Its three buffer pointers come from the
   **cross-thread payload** at offsets **a=32, b=40, o=48** (8 B A64 each;
   96 B / 3 GRF cross-thread, 192 B per-thread local-id), and the kernel is
   **SIMD32, 128 GRF** — your `MEDIA_VFE_STATE` GRF/threads and `GPGPU_WALKER`
   SIMD-size encoding must match SIMD32. Full table: `intel_gen9_kernels/README.md`
   "Cross-thread argument ABI" (oracle `add_f32x2.gen9.zeinfo`). iter7 = fill
   that payload → real Add retires `[4,6]`. You don't need the payload for the
   no-op (it takes no args); just don't encode STATE_BASE_ADDRESS in a way that
   blocks adding General State Base + cross-thread data in iter7.
