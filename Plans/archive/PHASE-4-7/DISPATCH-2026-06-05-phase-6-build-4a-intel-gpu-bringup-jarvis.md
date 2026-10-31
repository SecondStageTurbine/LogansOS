<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a — Intel Gen9.5 minimal compute bring-up → Jarvis

**Date:** 2026-06-05
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — e.g. `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`)
**Status:** DISPATCHED — code may proceed; **final acceptance is a bare-metal run (PL-driven), not CI**
**Design contract:** **ADR-032** (Gen9 bring-up decisions + the C1–C4 confirm-at-BUILD list), `Plans/THINK-2026-06-05-phase-6-build-4-real-gpu.md`, ADR-030 (AccelDriver trait), ADR-025 (typed MMIO wrappers).
**Builds on:** BUILD-3 (complete). Target: Intel UHD 630 (Gen9.5) on the OptiPlex 7080.

---

## 0. The unusual shape of this build (read first)

BUILD-4a **cannot be validated in CI** — there's no emulated GPU. So the work splits cleanly:
- **What you can prove on your own (no hardware):** it builds, the **command-byte oracle** matches the PRM, and the CI suite stays green with the driver off.
- **What needs the PL at the OptiPlex:** the actual probe→submit→complete→readback run, captured via `--bare-metal-mode`.

**Do not declare 4a done, and PM will not merge it, until the bare-metal evidence is captured** — landing unverified GPU code as "complete" is false progress (we will keep it on the feature branch until the run exists). Build it so the PL can run it with minimal ceremony.

## 1. Objective

The smallest real-GPU proof: run **one `Add` compute kernel** on the Gen9.5 iGPU and read back `[1,2]+[3,4] ≈ [4,6]`, matching the `accel_cpu` oracle within tolerance — proving probe + GTT + GuC-free submission + completion + readback end-to-end on real silicon. **No op-set breadth, no migration** (that's 4b).

## 2. Decisions locked (ADR-032) — settle the C-items, don't redesign

GuC-free submission (D2/C1), GTT-mapped system RAM (D3/C2), offline-compiled EU kernel embedded with pinned provenance (D4), poll a `PIPE_CONTROL` seqno (D5), within-tolerance oracle (D6/C3), command bytes asserted vs PRM (D7), framebuffer expendable + block-device log is the evidence channel (D8/C4), default-off feature `accel-intel`, compute-only. The four **Confirm-at-BUILD** items (C1 submission mechanism, C2 GTT/flush sequence, C3 tolerance, C4 scanout-quiesce + exact device-id) get settled here and written back into ADR-032.

## 3. Implementation Success Criteria

- **ISC-1 — Probe + MMIO.** PCI-probe the iGPU (vendor `0x8086`, confirm UHD 630 device-id — C4); map GTTMMADR (registers + GTT) via the ADR-025 typed wrappers. Feature `accel-intel` **default-off**; register on `Device::Accel(n)` **only when present** (absent/disabled iGPU → no register, no panic).
- **ISC-2 — GTT.** Map a kernel-owned system-RAM region (the EU-kernel blob + input/output buffers) into GPU graphics address space; document the C2 coherency/flush sequence in code with PRM citations.
- **ISC-3 — EU kernel (offline, provenance-pinned, D4).** A Gen9 `Add` compute kernel: checked-in **source** + the **pinned compiler/recipe** + the embedded `const` Gen9 binary. No hand-assembled ISA; no opaque blob (the recipe must reproduce the bytes).
- **ISC-4 — Command submission (C1).** Emit the Gen9 GPGPU pipeline (`MEDIA_VFE_STATE`/`MEDIA_INTERFACE_DESCRIPTOR_LOAD`/[`MEDIA_CURBE_LOAD`]/`GPGPU_WALKER`) into a command buffer; submit GuC-free (legacy ring or direct execlist — pick the simplest that works; record the choice in ADR-032).
- **ISC-5 — Completion (D5).** `PIPE_CONTROL` post-sync seqno store; `wait()` polls to completion, **bounded** (timeout → clean error, never an infinite/tight unyielding spin — heed the TLB-shootdown TCG-spin lesson).
- **ISC-6 — Command-byte oracle (D7, host-testable).** A **host** test asserts the command-buffer bytes the driver emits for this `Add` dispatch match **hand-computed, PRM-cited** expected values. This is the off-hardware correctness check — it must pass without a GPU.
- **ISC-7 — Tolerance oracle (D6).** On hardware, the readback `≈ accel_cpu` within the C3 tolerance (`Add` is IEEE-exact, so this should be tight/byte-exact — define and justify the bound).
- **ISC-8 — Bare-metal scenario + capture.** A `--bare-metal-mode` path that runs the bring-up and emits the result + a pass/fail line to the **block-device log sink** (the resilient evidence channel; the framebuffer may die — D8), so the PL can run it and pull the evidence with minimal fuss. Document the exact PL run/capture steps.
- **ISC-9 — Standards/gate.** No new **kernel** dependency; SAFETY on every new MMIO/GTT/DMA `unsafe` with PRM citations; SPDX on new files; the EU-kernel toolchain recipe documented; **CI suite + host-checks green with `accel-intel` OFF**, default build unaffected; `cargo fmt --all --check`.

## 4. Close gate (two stages)

**Stage A — worker-provable (push to feature branch when green):**
- Builds with `accel-intel` on (kernel target); the **command-byte oracle (ISC-6) passes on host**; CI suite + host-checks green with the feature **off**; fmt clean. Push; PM reviews the code, the oracle, and the C1–C4 write-backs.

**Stage B — bare-metal acceptance (PL-driven, gates merge):**
- The PL runs the `--bare-metal-mode` scenario on the OptiPlex; the captured block-device/serial log shows probe → submit → completion → readback `≈` oracle. PM/PL review the evidence. **Only then** is 4a merged to `main` and the C-items finalized in ADR-032.

Close report: ISC pass/fail, the command-byte oracle evidence, the C1–C4 resolutions, and (Stage B) the captured bare-metal evidence reference.

## 5. Scope guard

Minimal `Add` bring-up **only**. No full op set, no `AccelDriver` op breadth, no tensor migration (all 4b); no display/modeset/3D, no GuC, no multi-engine, no preemption. If GTT/submission bring-up reveals the Gen9.5 reality diverges from ADR-032's assumptions (entirely possible — that's what C1–C4 are for), **document it and adjust ADR-032**, don't silently work around it. If something looks like it needs GuC/firmware after all, stop and flag PM before taking that dependency.
