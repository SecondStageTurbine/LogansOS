<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a iteration 5 — bisect dispatch-mechanics vs EU data-access → Jarvis

**Date:** 2026-06-06
**From:** PM/Cerebro
**To:** Jarvis (continue on `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`, new commit on top of `efd1101`)
**Status:** DISPATCHED — iter4 bisected ring✓/compute✗; iter5 bisects *inside* the compute dispatch. Two-stage gate.
**Builds on:** iter4 (`efd1101`). Evidence: `Plans/evidence/accel_intel_add_bringup.optiplex.iter4.extracted.log` (sha256 `8186f68a…`).

---

## 0. What iter4 proved (the eliminations)

The OptiPlex bisect was decisive. **Working:** legacy-ring execution + `PIPE_CONTROL` + GGTT seqno (`ring-smoke complete … smoke-seqno=0x2`); forcewake (both ACKs=1); the EU blob's **EOT is present** (`pinned_tail_match=true`, `ret (M1,1)`); the interface descriptor is correct (`kernel_ptr_matches=true`, `kernel_start=0x120000`, `lhs/rhs/output` set, `thread_count=1 simd_width=1`). **The wall:** the compute Add thread dispatches but **never retires** — `RING_HEAD=ACTHD=0x50` (parked past the walker), `seqno=0`, `INSTDONE` shifts `0xfffffffe` (smoke, near-idle) → `0xffdeffff` (compute, a unit stuck), `RING_FAULT=0`.

So it's **inside the EU thread execution** — narrowed to two suspects:
- **(A) dispatch mechanics** — `MEDIA_VFE_STATE` (thread pool / URB / scratch) or `GPGPU_WALKER` params dispatch a thread that can't run.
- **(B) the Add kernel's EU data access** — its first `lhs`/`rhs` read (or `output` write) faults/hangs because the **EU data port** addressing (stateless A64/A32 vs stateful surface-state; PPGTT vs GGTT; MOCS) isn't set up, even though the *command streamer* reaches GGTT fine.

## 1. Objective

Separate (A) from (B) with a **no-op EU kernel**, and decode the stuck-unit state — so the next run says which of the two to fix.

## 2. Implementation Success Criteria

- **ISC-1 — No-op (EOT-only) EU kernel bisect.** Add a minimal Gen9 EU kernel that does **only** the End-Of-Thread send — **no memory access** — dispatched through the *same* `VFE_STATE`/interface-descriptor/`GPGPU_WALKER` path as the Add kernel (a scenario sub-mode). Because the standard image has no Intel finalizer, **hand-lay this kernel** (an EOT-only thread is a tiny, fixed Gen9 `send` sequence) and ship it with a **hand-computed byte oracle** (CLAUDE.md: it's an on-device instruction blob). Distinct breadcrumb/result (`noop-kernel-seqno=…`).
  - **No-op retires** (walker completes, post-walker seqno written) → **dispatch mechanics work; the Add kernel's data access is the bug** → iter6 = EU data-port addressing (surface state / stateless mode / PPGTT / MOCS).
  - **No-op also hangs** → **dispatch setup is the bug** (VFE/walker/descriptor) → iter6 = dispatch config, independent of the kernel body.
- **ISC-2 — Decode the stuck unit.** Read + log the Gen9 EU/thread-dispatch status beyond the basic `INSTDONE`: the subslice/row INSTDONE variants (e.g. `SC_INSTDONE`, `ROW_INSTDONE`), `TD_CTL`, and any GPGPU thread-dispatch / pending-thread counters available; and **decode which `INSTDONE` bits are clear** for the `0xffdeffff` compute case against the PRM, naming the hung unit. Log to the NVMe sink.
- **ISC-3 — Keep the existing paths.** The ring smoke and the Add compute path stay; the no-op is a third sub-mode. Per-mode distinct reasons so the log is unambiguous.
- **ISC-4 — Standards.** `accel-intel` default-off; SPDX; no new kernel dep; SAFETY+PRM citations on the new EU-blob + MMIO; the new no-op kernel's byte oracle is host-tested; existing command-byte oracle stays green; `cargo fmt --all --check`; host-checks + `shard-coverage 4` green; full sharded suite green with `accel_intel_add_bringup` hardware-only.
- **ISC-5 — Record it.** ADR-032: log the iter4 eliminations (ring/forcewake/GGTT/descriptor/EOT all proven) and the iter5 A/B bisect.

## 3. Close gate (two-stage)

- **Stage A (you):** builds with `accel-intel` on; the no-op kernel byte oracle + command oracle green; CI suite + host-checks green feature-off; fmt clean. Push; PM reviews the hand-laid no-op kernel + its oracle + the EU-status decode.
- **Stage B (PL):** fresh OptiPlex run. The log shows **either** PASS (`[4,6]`) **or** the `noop-kernel-seqno` result + EU-status decode that picks (A) vs (B). **Merge only on PASS.**

## 4. Scope guard

Still the single `Add` bring-up. The no-op kernel is a **diagnostic sub-path**; no op-set breadth, no ELSP, no display. If iter5 isolates the bug to **(B) EU data access**, iter6 will likely need the **real Intel finalizer (IGC/ocloc/vISA) added to the build toolchain** to regenerate a corrected Add kernel — flag PM then and I'll dispatch that image task to Prometheus; do **not** pull the finalizer in now (the hand-laid no-op kernel is enough for this bisect).
