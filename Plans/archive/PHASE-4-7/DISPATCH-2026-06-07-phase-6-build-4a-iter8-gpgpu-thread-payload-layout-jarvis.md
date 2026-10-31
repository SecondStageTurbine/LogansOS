<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a iteration 8 — Gen9 GPGPU thread-payload (per-thread local_id) layout → Jarvis

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Jarvis (continue on `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`, new commit on top of iter7 Stage-B `4b682ed`)
**Status:** DISPATCHED — iter7 Stage-B conclusive (payload reaches GPU, addr_space=GGTT, **GRF placement wrong**). This is the BUILD-4a→main trigger build.
**Builds on:** iter7 (`4b682ed`, cross-thread CURBE + fault-address capture) + the finalizer Add (`072e0f7`).

## 0. What iter7 proved on silicon (the bisection advanced, not stalled)
The ISC-5a fault-address capture made iter7 conclusive instead of a fourth blind hang:
- **Payload reaches the GPU** — CURBE mapped at `0x134000`, MEDIA_CURBE_LOAD raised no fault, EU spun up.
- **Address space is NOT the bug** — `addr_space=GGTT` proves A64 dataport translates through GGTT (the ISC-5b discriminator fired: payload-side, not address-side).
- **The bug is GRF placement** — the Add read its pointer from `r8.0` and got garbage `0xfb1ce9bb…`, not `0x130000`.

## 1. Substrate survey (PM, done before this BUILD) — the authoritative model

### 1a. The GRF contract is fixed by the kernel itself (the oracle is in-repo)
`intel_gen9_kernels/add_f32x2.gen9.asm` (IGC disasm) + `add_f32x2.gen9.zeinfo` define the
thread payload the kernel *reads*. This is non-negotiable ABI, not a choice:

| GRF | Role | Evidence in the disasm |
|---|---|---|
| `r0` | dispatch header | `mov r3, r0`; `mov r127, r3` (→ EOT) |
| `r1`,`r2` | per-thread `local_id.x` (SIMD32 lo/hi, `:uw`) | `add r5 … r1.0<16;16,1>:uw` / `add r10 … r2.0<16;16,1>:uw` |
| `r3`–`r6` | per-thread `local_id.y/.z` (`:uw`, zero for 1D) | `shl r46 … r6.0:d` (per-thread block spans r1..r6) |
| `r7` | cross-thread `global_id_offset` / `local_size` | `add r5 … r7.0<0;1,0>:d` (broadcast) |
| **`r8.0/.1/.2`** | **cross-thread A64 pointers a/b/o** | **`add r12 r44 r8.0:q` … `r8.1:q` … `r8.2:q`** |
| `r9` | cross-thread `enqueued_local_size` | `mul r4 r9.0:d r3.1:d` |

So: **r0 header → r1–r6 per-thread local_id (6 GRF / 192 B) → r7–r9 cross-thread (3 GRF / 96 B).**
The kernel's pointers live at **r8**, i.e. cross-thread GRF #1, which only lands there if a
full 6-GRF per-thread block precedes it. (192 B = SIMD32 × 3 dims × 2 B `uw` = 6 GRF; matches
the zeinfo `per_thread_payload_arguments local_id size 192`.)

### 1b. The toolchain ABI is NEO/IGC, **not** i965/Mesa (the reframe)
The blob was built by `ocloc` (IGC `igc-1.0.17193.4` / compute-runtime `24.26.30049.6`), so its
payload convention is **NEO's**, not i965's — and the two order the payload differently. The
driver encoder is currently commented against i965 / `gen9.xml`; that mismatch is the deeper
root. iter8 follows the **NEO Gen9 dispatch convention**, with the in-repo disasm/zeinfo as the
ABI oracle. NEO computes `perThreadDataSize = perThreadDataSizeForWholeThreadGroup /
numThreadsPerThreadGroup`; Gen8/9/11 encoders live on compute-runtime branch `releases/24.35`
(same lineage as our pinned CR 24.26 — Gen9 = production there). See Sources.

### 1c. iter7's exact gap (confirmed in code, disasm, AND the fault address)
iter7 set `CURBE_CROSS_THREAD_GRF=3`, raised the VFE CURBE allocation, and loaded a byte-correct
CURBE (the hand-laid oracle confirms the *contents*). But the GPGPU_WALKER emit sets only DW4
(SIMD/threadcount), DW7/10/12 (dims) and DW13/14 (masks) — **DW2 (Indirect Data Length) and DW3
(Indirect Data Start Address) are never written, and no per-thread local_id block is built at
all.** With no per-thread data, the dispatcher places cross-thread starting at r1; the kernel
reads r8 (expecting pointer `a`) and gets local-id/garbage bytes. Two things are missing, and
the disasm proves both matter: the **6-GRF per-thread local_id block** (so the kernel's
`local_id.x` at r1/r2 is real *and* cross-thread is pushed to r7) and the **delivery wiring**
that puts it at r1–r6.

## 2. ISCs

- **ISC-1 — Confirm the Gen9 delivery mechanism against the matching production source (OBSERVE,
  before encoding).** Read compute-runtime `releases/24.35` Gen9 dispatch encoding
  (`KernelCommandsHelper` / `EncodeDispatchKernel` / `HardwareCommandsHelper::sendIndirectState`
  and `generateLocalIDs`) and confirm, for a 1-thread SIMD32 group, **exactly**: (a) whether
  per-thread local_id is delivered via GPGPU_WALKER **Indirect Data** (DW2 length / DW3 start
  addr → Indirect Object Heap) or via the CURBE path; (b) the heap order (NEO writes
  crossThreadData then perThreadData into the indirect heap) and how the IDD read-length fields
  split it into the r1.. / r7.. payload; (c) the INTERFACE_DESCRIPTOR_DATA fields:
  **Cross-Thread Constant Data Read Length** (=3 GRF) and **Constant/Indirect URB Entry Read
  Length** (covers per-thread); (d) the `generateLocalIDs` channel layout for SIMD32 (per-dim
  `uw`, lo/hi SIMD16 → r1/r2 = x, r3/r4 = y, r5/r6 = z). Record the field map in ADR-032 before
  writing the encoder. **Do not guess these bits from the i965 PRM** — the running kernel is IGC.

- **ISC-2 — Build the per-thread local_id payload (6 GRF / 192 B).** For this 1D NDRange,
  global size `2,1,1`, one workgroup of 2 → **one** thread (2 of 32 lanes active). Lay
  `local_id.x = [0,1,0,0,…]` across the 32 lanes as `:uw` (r1 = lanes 0–15, r2 = lanes 16–31),
  `local_id.y/.z = 0` (r3–r6), per the ISC-1 channel layout. A single thread means the
  per-thread block is one 6-GRF blob (no per-thread replication to manage this build).

- **ISC-3 — Deliver per-thread + cross-thread so the dispatcher lands them at r1–r6 / r7–r9.**
  Per ISC-1's confirmed mechanism: set GPGPU_WALKER **DW2 Indirect Data Length** and **DW3
  Indirect Data Start Address** to the per-thread heap, and the IDD read-length fields so
  cross-thread (the existing CURBE) reads into r7–r9. Keep the cross-thread CURBE contents from
  iter7 (the byte oracle still passes); only its **placement** changes. If the confirmed NEO
  path puts both blocks in one indirect heap, migrate the cross-thread there and retire
  MEDIA_CURBE_LOAD — whichever ISC-1 says is the production Gen9 path; do not run a hybrid the
  source doesn't use.

- **ISC-4 — Keep General State Base = 0 / A64 absolute (iter7 proved GGTT resolves).** No
  address-space change. Inputs filled as iter7: `a=[1,2]`, `b=[3,4]` at lhs/rhs.

- **ISC-5 — Independent byte oracles (REQUIRED, CLAUDE.md rule).** The per-thread local_id block
  is a new on-wire byte layout → add a **hand-laid byte fixture** in `tools/ai-ops-tests`
  asserting the 192-B block: `local_id.x` lane values at the right `uw` offsets in r1/r2, zeros
  in r3–r6; cross-check against the ISC-1 `generateLocalIDs` layout. Re-lay the **command-byte
  oracle** for the grown `add_batch` — spot-assert GPGPU_WALKER **DW2/DW3** (now non-zero) and
  the IDD read-length fields. Keep the iter7 cross-thread payload oracle.

- **ISC-6 — Standards.** Default-off; SPDX; SAFETY + zeinfo/disasm/NEO-source citations on the
  new encode paths; `cargo fmt`; host suite (new local_id oracle + re-laid command oracle +
  iter7 cross-thread oracle + both EU-EOT tests) green; `shard-coverage 4`; `clippy -D warnings`;
  kernel builds `accel-intel` on (ISO) + feature-off. ADR-032 records the ISC-1 field map +
  the iter8 result.

## 3. Failure-mode + discriminator section (REQUIRED — CLAUDE.md blind-hardware rule)
The base rate here is "the next run reveals the next layer." The iter7 fault-address capture
stays in place, so iter8 is self-diagnosing. Expected modes if `[4,6]` still doesn't land:

| Mode | Most-likely cause | Discriminator (what the captured evidence shows) |
|---|---|---|
| **A. Pointer still garbage at r8** | per-thread block sized/placed wrong → cross-thread still not at r7 | fault addr is garbage (not `0x0013_xxxx`) AND the EU-decode shows the read offset; dump the assembled payload heap and compare r7/r8 bytes to the oracle |
| **B. Pointer correct but wrong element / partial result** | `local_id.x` content/layout wrong (r1/r2) → global_id mis-indexes; e.g. both lanes read element 0 | `output` lands but `!= [4,6]` (e.g. `[4,4]` or `[8,…]`); read back both elements + the local_id GRFs |
| **C. Fault at a valid `0x0013_xxxx` addr** | A64 read of a real buffer addr faults | (would reopen the GGTT-vs-PPGTT question ISC iter7 closed — capture the addr; should NOT recur, but if it does, escalate with the addr) |
| **D. Stray write past `output_base`** | walker exec-mask not trimmed to 2 lanes / per-thread replication for idle lanes | memory after `0x132000` clobbered; check the 2-lane mask (iter7 set `0x3`) and the SIMD32 idle-lane writes |

**Capture-before-you-need-it:** if ISC-1 reveals the dispatcher needs a field we don't yet log
to distinguish A from B (e.g. the assembled indirect-heap contents as the GPU sees them), add
that dump in this build — do not discover the gap on the failing run.

## 4. Gate (two-stage)
- **Stage A (you):** builds (on + off); new local_id byte oracle + re-laid command oracle green;
  host/CI green feature-off; clippy; fmt.
- **Stage B (PL, OptiPlex):** the real Add retires **and computes `[4,6]`** —
  `output_base == [4.0f, 6.0f]` with the seqno landed. **That is the BUILD-4a→main trigger.**
  Report the output bytes + seqno + (on any miss) the fault addr + the discriminator branch.

## 5. Scope guard
Thread-payload assembly only (`intel_gen9_4a.rs` encoder: per-thread local_id build + GPGPU_WALKER
DW2/DW3 + IDD read-length fields; `accel_intel` heap setup; the `tools/ai-ops-tests` oracles;
ADR-032). **Do NOT regenerate the EU blobs** (finalizer, frozen), **do NOT change
STATE_BASE_ADDRESS bases** (iter6, proven) or the address space (iter7, proven). No stateful BTI,
no ELSP, no display.

## 6. Merge (PM, on Stage-B `[4,6]` PASS)
Close-gate: `git fetch origin && git rebase origin/main` (clears the stale-base trap — main carries
the iter6/iter7/iter8 dispatches + the CLAUDE.md rule the branch base lacks), re-run Stage-A,
push. PM runs `scripts/pm-merge-check.sh`, then `--ff-only` merge → `main`. **BUILD-4a closes.**

## Sources (substrate survey)
- compute-runtime (NEO) — Gen9 dispatch encoder lives on branch `releases/24.35`:
  https://github.com/intel/compute-runtime
- i965 CURBE / local-id-via-CURBE background (contrast — *not* the convention to copy):
  https://github.com/intel/external-mesa/blob/master/src/mesa/drivers/dri/i965/brw_curbe.c
- In-repo authoritative ABI oracle: `intel_gen9_kernels/add_f32x2.gen9.asm` + `add_f32x2.gen9.zeinfo`.
