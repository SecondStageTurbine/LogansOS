<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-4a iteration 2 — Gen9.5 forcewake + legacy-ring mode + submit instrumentation → Jarvis

**Date:** 2026-06-05
**From:** PM/Cerebro
**To:** Jarvis (continue on the existing branch `codex/phase-6-build-4a-intel-gpu-bringup-jarvis`, new commit on top of `655b8f4`)
**Status:** DISPATCHED — Stage-B failure follow-up; two-stage gate again (Stage B PL-driven)
**Design contract:** ADR-032 (C1/C2 get updated here), the Stage-A dispatch `Plans/DISPATCH-2026-06-05-phase-6-build-4a-intel-gpu-bringup-jarvis.md`.
**Builds on:** BUILD-4a Stage A (`655b8f4`).

---

## 0. The Stage-B result (authoritative bare-metal evidence)

On the OptiPlex 7080: `event=FAIL name=accel_intel_add_bringup reason=accel_intel_add_wait_failed`.
- **Worked:** NVMe log sink; iGPU probed at `00:02.0` `device_id=0x9bc8` (Comet Lake UHD 630, a candidate ID ✓); `accel_intel` registered; Add batch **submitted** (`ring=0x100000`, `seqno_ggtt=0x133000`).
- **Failed:** the **completion seqno never arrived** before the wait timeout. **No panic, no `cr2=`, no `BUG:`, no GPU fault/reset.**
- Evidence: `target/neo-test/accel_intel_add_bringup.optiplex.extracted.log` (sha256 `97bec2d5…`), raw NVMe `…optiplex.nvme-log.raw` (sha256 `2db478d7…`).

**Diagnosis: this is the C1 risk (legacy-ring submission on Gen9.5).** "Submitted, never completed, *cleanly*" means the **engine never started executing** — not that it executed garbage (that would fault/reset). On Gen9 that is almost always one of two missing requirements:
1. **Forcewake not held** — Gen9 GT/MMIO (the ring registers) require forcewake or the GT power-gates (RC6) and **drops the register writes**, so `RING_TAIL` never reaches a running engine.
2. **Engine still in execlist mode** — Gen9 RCS **defaults to execlist (ELSP)**; a legacy `RING_TAIL` write does nothing unless execlists are explicitly **disabled** and the engine is in ring-buffer mode.

## 1. Objective

Either get the Add to **complete** on the OptiPlex, or **precisely localize** the remaining failure. Add the two near-certain Gen9 requirements *and* the instrumentation that discriminates the failure mode in one bare-metal cycle (each cycle costs PL hardware time).

## 2. Implementation Success Criteria

- **ISC-1 — Hold forcewake across all GT MMIO.** Acquire the render forcewake domain before touching GT registers (ring setup, tail write, status reads), poll/verify `FORCEWAKE_ACK`, release after. Log the ACK state to the NVMe sink.
- **ISC-2 — Put RCS in legacy ring-buffer mode.** Disable execlist submission on the render engine (the `GFX_MODE`/`RING_MODE` masked-write that clears Execlist-Enable), and confirm `RING_CTL` is valid with the correct length and the ring is actually in ring-buffer mode before submit.
- **ISC-3 — Instrument the submit→complete path (the bisect).** After the tail write and after the wait (pass or timeout), log to the NVMe sink: **`RING_HEAD`** (did the engine consume the ring?), `RING_TAIL`, `FORCEWAKE_ACK`, the RCS status/fault registers (`RING_CTL`, engine fault / instdone / RC6 status), and the **seqno value** read back. These let the next run read off the failure mode unambiguously:
  - `HEAD == 0` (didn't advance) → engine never started → forcewake/execlist-mode still wrong (or legacy ring genuinely unsupported → iteration 3 = ELSP).
  - `HEAD == TAIL` but no seqno → engine consumed the ring but the seqno write isn't CPU-visible → GTT/coherency/`PIPE_CONTROL` flush (C2).
  - fault register set → the command stream faulted → command-byte / GTT-mapping bug.
- **ISC-4 — Oracle stays green.** If the command stream or GTT setup changes the emitted bytes, re-assert the host command-byte oracle (`intel_gen9_add_batch_matches_hand_byte_oracle`) against the new hand-laid bytes; otherwise it stays as-is.
- **ISC-5 — Clean bounded wait.** Keep the timeout bounded and clean (no tight unyielding spin, no panic) so a non-completion still produces capturable evidence, not a hang.
- **ISC-6 — Record the finding in ADR-032.** Update C1 (forcewake + legacy-ring-mode are required on Gen9.5; execlist/ELSP is the iteration-3 fallback if HEAD still won't advance) and C2 (whatever the coherency instrumentation shows) with the Stage-B evidence.
- **ISC-7 — Standards.** `accel-intel` stays default-off; SPDX on any new file; no new kernel dependency; SAFETY on new MMIO/forcewake unsafe with PRM citations; `cargo fmt --all --check`; host-checks + `shard-coverage 4` green; the full sharded suite green with `accel_intel_add_bringup` still hardware-only (excluded from QEMU).

## 3. Close gate (two-stage, unchanged shape)

- **Stage A (you):** builds with `accel-intel` on; the command-byte oracle green; CI suite + host-checks green with the feature off; fmt clean. Push to the branch; PM reviews the forcewake/ring-mode/instrumentation diff + the ADR-032 update.
- **Stage B (PL):** a fresh OptiPlex run; the NVMe log shows **either** `accel_intel_add_bringup` PASS (readback `== [4.0, 6.0]`) **or** the instrumented `RING_HEAD`/status that localizes the next step. **Merge only on Stage-B PASS.** A localized failure comes back to PM for iteration 3.

## 4. Scope guard

Still the single `Add` bring-up — **no op-set breadth, no execlist/ELSP submission yet** (that is iteration 3, gated on the instrumentation showing legacy-ring is genuinely rejected), no display/modeset, no multi-engine. The deliverable is forcewake + legacy-ring-mode + the discriminating instrumentation, so the next bare-metal cycle either passes or tells us exactly where to dig.
