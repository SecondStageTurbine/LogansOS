<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a iteration 7 — stateless-A64 Add argument payload → Jarvis

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Jarvis (continue on `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`, new commit on top of iter6 Stage-B `f2a602e`)
**Status:** **DISPATCHED** (2026-06-07) — iter6 Stage-B PASSED, precondition met. This is the BUILD-4a→main trigger build.
**Builds on:** iter6 (`f2a602e`, STATE_BASE_ADDRESS dispatch-setup fix + Stage-B evidence) + the finalizer Add (`072e0f7`).

## 0. PRECONDITION — MET (iter6 Stage-B is green)

iter6's Stage-B OptiPlex run (`f2a602e`) **passed**: the finalizer no-op retired
(`noop_retired verdict=dispatch_ok`), EU INSTDONE returned to **idle `0xfffffffe`**
from iter5's stuck-at-fetch `0xffdeffff` (bits 16/21), ring drained to tail,
`RING_FAULT=0`. An EU cannot go stuck-fetch → idle without executing to `{EOT}`, so
the dispatch path (VFE / walker / descriptor / STATE_BASE_ADDRESS / Instruction Base)
is **silicon-proven sound**. iter7 only *adds data* (the arg payload) on top of it.

The same run **opened SUSPECT B exactly as predicted**: the Add fetches, spins up
the EU, then **page-faults** — `RING_FAULT=0x000009e1` (Valid, invalid-PTE /
page-not-present), `seqno=0`. That is the unloaded-pointer signature: with no
MEDIA_CURBE_LOAD, the stateless-A64 Add dereferences garbage `lhs/rhs/output`
pointers into unmapped memory. Loading the cross-thread payload below is precisely
the fix. Evidence:
`Plans/evidence/phase6-build4a-iter6-optiplex-20260607-state-base-address.md`
+ extracted log (`a8bcd0cb…`); ADR-032 iteration-6 Stage-B paragraph.

**Proceed.** This dispatch is live.

## 1. What iter6 left in place (the gap iter7 closes)
`add_batch_dwords` (`kernel/src/drivers/accel/intel_gen9_4a.rs`) now emits, in order:
PIPELINE_SELECT(GPGPU) → PIPE_CONTROL(flush/stall) → **STATE_BASE_ADDRESS** (bases=0/absolute,
bounds=max) → PIPE_CONTROL(cache invalidate) → MEDIA_VFE_STATE (**CURBE=0**) →
MEDIA_INTERFACE_DESCRIPTOR_LOAD (one IDD, **no CURBE read length**) → MEDIA_STATE_FLUSH →
GPGPU_WALKER (SIMD32, 1×1×1) → PIPE_CONTROL(seqno write) → MI_BATCH_BUFFER_END.

The kernel runs (proven by the no-op), but the Add's three pointers are never
delivered to the thread: **CURBE is empty**, the **input buffers are unfilled**,
and the **IDD declares no cross-thread constant data**. The thread would read
garbage/zero pointers. iter7 wires the arg payload.

## 2. The contract (authoritative — already checked in)
- **ABI table:** `kernel/src/drivers/accel/intel_gen9_kernels/README.md` → "Cross-thread argument ABI".
- **Independent oracle:** `kernel/src/drivers/accel/intel_gen9_kernels/add_f32x2.gen9.zeinfo` (`payload_arguments`).
- **Expected output:** `add_oracle_output()` already returns `[4.0f, 6.0f]` little-endian.
- **Add semantics:** `o[i] = a[i] + b[i]`, f32, 2 elements → inputs `a=[1.0,2.0]`, `b=[3.0,4.0]` produce `o=[4.0,6.0]`.

Cross-thread (CURBE) payload, **96 B / 3 GRF**, exact byte layout:

| Cross-thread offset | Size (B) | Field | iter7 fills with |
|---:|---:|---|---|
| 0  | 12 | `global_id_offset` | `0, 0, 0` (u32×3) |
| 12 | 12 | `local_size` | the dispatch local size (u32×3; `2,1,1`) |
| 24 | 8  | _(pad)_ | zero (alignment to the 8-byte ptr at 32) |
| **32** | **8** | **arg 0 `a`** | **A64 GPU VA of `lhs_base`** (full 64-bit, little-endian) |
| **40** | **8** | **arg 1 `b`** | **A64 GPU VA of `rhs_base`** |
| **48** | **8** | **arg 2 `o`** | **A64 GPU VA of `output_base`** |
| 56 | 8 | _(pad)_ | zero |
| 64 | 12 | `enqueued_local_size` | `2, 1, 1` (u32×3) |

Span `[0, 76)` → ceil(76/32) = **3 GRF (96 B)**. Execution env (zeinfo, must match what
iter6 already set / what iter7 declares): `simd_size 32`, `grf_count 128`,
`has_4gb_buffers true` (stateless A64), `eu_thread_count 7`. Per-thread `local_id` =
192 B at per-thread offset 0 (the walker generates it; iter7 only sizes it).

## 3. ISCs

- **ISC-1 — Fill the input buffers.** Before the dispatch, write `a=[1.0f,2.0f]` to
  `lhs_base` and `b=[3.0f,4.0f]` to `rhs_base` (GGTT-mapped). These are the operands the
  A64 gathering reads pull; with empty buffers the kernel adds zeros.

- **ISC-2 — Build the 96-B cross-thread payload** exactly per §2 into a CURBE GGTT
  region. Add a `curbe_base` to `BUILD4A_LAYOUT` (a new GGTT region distinct from
  state/kernel/lhs/rhs/output/seqno) and lay the three **A64 absolute** pointers at
  byte offsets 32/40/48. Stateless A64 addresses are **absolute** (not General-State-Base
  relative), so keep **General State Base = 0** as iter6 left it — do not add a GSB for the
  data path. Only Instruction Base (kernel fetch) and Dynamic State Base (IDD/CURBE read),
  both already 0/absolute, matter.

- **ISC-3 — Emit MEDIA_CURBE_LOAD** to load the 96-B payload into the CURBE area
  (Dynamic-State-relative offset = `curbe_base`; length = 96 B / 3 GRF). Insert it after
  MEDIA_VFE_STATE and before / alongside the IDD load, per the Gen9 media-state ordering
  (CURBE before the walker dispatches). Hand-verify the command DWords vs the PRM /
  Mesa `gen9.xml`.

- **ISC-4 — Wire MEDIA_VFE_STATE + the interface descriptor for the CURBE.**
  - MEDIA_VFE_STATE: set **CURBE Allocation Size** to cover 3 GRF (32-B units); confirm
    GRF/URB/threads still match the SIMD32/128-GRF kernel iter6 set.
  - Interface descriptor (IDD): set **Cross-Thread Constant Data Read Length = 3 GRF**
    (the prefix the dispatcher copies into every thread's payload from CURBE), keep
    **Number of Threads in Thread Group = 1**, kernel-start = the Add blob offset
    (`kernel_base`), no sampler/BT/SURFACE_STATE (stateless).

- **ISC-5 — GPGPU_WALKER dims for 2 elements.** Global size `2,1,1`, one threadgroup,
  SIMD32 (one thread, 2 active lanes). Right/bottom execution masks must mark exactly the
  2 active lanes live (not all-lanes-full as the no-op tolerated) so the 30 idle SIMD32
  lanes don't issue stray A64 writes past `output_base`. Re-verify the mask encoding.

- **ISC-5a — Capture the FAULT ADDRESS, not just the fault type (de-blind the next run).**
  Today the driver reads `GEN8_RING_FAULT_REG_RCS` (0x4094) → it has the iter6 `0x9e1`
  (Valid / invalid-PTE) but **no faulting address**. Before the iter7 run, extend the GT
  snapshot to also capture and log, on any non-zero `RING_FAULT`: the decoded `RING_FAULT`
  fields (GTTSEL bit, SRCID/engine, access type, fault type) **and the faulting GPU virtual
  address** (Gen8/9 exposes it via the per-engine fault registers around the RCS fault block
  — read the fault-address pair alongside 0x4094; cite the KBL PRM Vol. 2c fault-register
  layout). This is the single highest-value instrumentation add: it makes a *recurring*
  `0x9e1` self-diagnosing (see ISC-5b), instead of another blind iteration.

- **ISC-5b — Address-space risk (the most likely cause of a repeat fault) + its discriminator.**
  Every BUILD-4a buffer is **GGTT-only** (no PPGTT; legacy ring). Instruction fetch + IDD/CURBE
  reads resolve through GGTT (iter6 proved it on silicon). But the Add's data reads are
  **A64 stateless**, and A64 dataport translation is *not guaranteed* to use the GGTT path
  that instruction fetch uses. So even with a correctly loaded payload, the A64 deref of
  `0x130000`/`0x131000`/`0x132000` could fault if dataport translates through a (here unset)
  PPGTT. The ISC-5a fault address makes this a one-shot diagnosis:
  - **No fault, `output==[4,6]` →** done, BUILD-4a closes.
  - **Faults at `0x0013_0xxx` (one of our GGTT addrs) →** payload is right; the bug is the
    **A64 address space** (GGTT vs PPGTT). Next step: confirm/program the context so A64
    dataport resolves through GGTT (Gen9 GGTT-global / `PP_DCLV`/`GFX_MODE` PPGTT-disable, or
    set up a PPGTT that maps these pages). Escalate to PM with the fault addr; do not guess.
  - **Faults at a garbage / non-`0x0013_xxxx` address →** the payload/CURBE wiring (ISC-2/3/4)
    placed the pointers in the wrong GRF; fix the cross-thread read length / CURBE offset.
  Document whichever branch fires in the evidence + ADR-032.

- **ISC-6 — Post-dispatch readback + visibility.** Keep the post-walker PIPE_CONTROL with
  **DC flush + CS stall** so the A64 scattering writes to `output_base` are coherent to the
  CPU before the seqno write signals done. Read back `output_base` and compare to
  `add_oracle_output()` (`[4.0f, 6.0f]`).

- **ISC-7 — Independent byte oracle for the payload (REQUIRED, CLAUDE.md rule).** The
  cross-thread payload is an on-wire byte layout, so it needs a **hand-laid byte fixture**
  asserted independently of the encoder — round-trip is not sufficient. Add a test in
  `tools/ai-ops-tests` that hand-computes the 96-B payload for known `lhs/rhs/output`
  GGTT addresses and asserts: u32×3 zeros at 0; `local_size` at 12; the three little-endian
  A64 pointers at **byte offsets 32, 40, 48**; `enqueued_local_size` at 64; zeros in the
  pads. Cross-check the offsets against `add_f32x2.gen9.zeinfo` (the spec oracle). Also
  re-lay the **command-byte oracle** for the grown `add_batch` (now carrying MEDIA_CURBE_LOAD
  + the populated IDD) — spot-assert the MEDIA_CURBE_LOAD header + length and the IDD
  cross-thread-read-length field, the way iter6 spot-asserts the SBA header.

- **ISC-9 — Glance: no-op seqno-page seeding (cosmetic, logged in iter6 evidence).** The
  iter6 run noted the no-op pre-kick read `seqno=0x3` (the target value) *before* the kick,
  while the Add correctly read `0x0` on the same page. Not load-bearing for the iter6
  verdict (which rests on the EU-idle INSTDONE transition), but since iter7 is in the
  encoder anyway: glance at the no-op seqno-page seeding/init so a stale pre-seed can't
  ever masquerade as a retire on a future run. Fix only if it's a one-line init bug; do not
  scope-creep.

- **ISC-8 — Standards.** Default-off; SPDX headers; SAFETY + PRM/zeinfo citations on the new
  encode paths; `cargo fmt`; host suite (incl. the new payload oracle + re-laid add-batch
  oracle + both EU-kernel-EOT tests) green; `shard-coverage 4` green; workspace
  `clippy -D warnings`; kernel builds both `accel-intel` on (ISO) and feature-off. ADR-032
  records the iter6 Stage-B no-op result + the iter7 arg-payload wiring + the `[4,6]` result.

## 4. Gate (two-stage)
- **Stage A (you):** builds (feature on + off); the new hand-laid payload oracle + re-laid
  command-byte oracle green; host/CI green feature-off; clippy; fmt.
- **Stage B (PL, OptiPlex):** the real Add retires **and computes `[4,6]`** —
  `output_base == [4.0f, 6.0f]` with the completion seqno landed. **That is the
  BUILD-4a→main trigger.** Report the output bytes + seqno + (if anything is off) the
  EU-decode.

## 5. Scope guard
Command-encoder / arg-payload only (`accel_intel` + `intel_gen9_4a.rs` encoder, the
`tools/ai-ops-tests` oracles, ADR-032). **Do NOT regenerate the EU kernel blobs** (finalizer
lane, frozen) and **do NOT change STATE_BASE_ADDRESS bases** (iter6, proven) beyond the IDD
CURBE-read / VFE CURBE-alloc fields this build requires. No stateful BTI / SURFACE_STATE
(deferred hardening, ADR-032 D4), no ELSP, no display.

## 6. Merge (PM, on Stage-B `[4,6]` PASS)
Worker close-gate first: `git fetch origin && git rebase origin/main` (main carries the iter6
addendum + this dispatch the branch's base lacks — this clears the stale-base trap), re-run
the Stage-A gate after the rebase, push. Then PM runs `scripts/pm-merge-check.sh` and does the
`--ff-only` merge of `codex/phase-6-build-4a-intel-gpu-bringup-jarvis` → `main`. BUILD-4a closes.
