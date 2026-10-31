<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AAG BOUNCE: GPU P1 Slice-6 (glass→G1 + compute ABI) → Chad

**Date:** 2026-07-22
**From:** PM (Claude) — 5-lens cross-model adversarial-audit panel (Claude auditing Codex)
**Branch:** `codex/gpu-p1-slice6-glass-into-g1-chad` @ `5b816ca0`
**Status:** **BOUNCED — NOT merged.** CI is green (independently re-verified: exact head SHA `5b816ca0`, `workflow_dispatch`, all 4 QEMU shards + host + SPDX + identity), the diffstat is clean (no phantom deletion), and **four of five AAG lenses are clean** — present/fence separation, cap no-self-mint/algebra, memory-safety bounds, and I-14 governance isolation are all well-constructed. Two items must be closed before merge; both are small and local. The work is good — this is a release-integrity bounce, not a design bounce.

## Must-fix 1 — the `&mut`-from-`&self` aliasing seam (CONFIRMED by lenses 3 AND 4)
`SharedPixelBuffer::contiguous_pixels_mut(&self) -> &mut [u8]` (`kernel/src/atrium/buffer.rs:193-198`, via `DmaBuffer::as_mut_slice(&self)` `dma.rs:209`) hands out a `&mut [u8]` with **no in-use guard** — soundness rests entirely on caller discipline. The Slice-6 ABI widens the exposure the `unsafe` comment predates:

- **Repro (needs only preemption, not SMP):** the `Arc::ptr_eq(&src,&dst)` check in `sys_gpu_compute_submit_blur_impl` rejects `src==dst` **only within one call**. Two concurrent submits naming the **same `dst` `Arc`** (a write-cap is minted with `full_gpu_compute_write_buffer_rights()` incl. GRANT/DERIVE, so it can be granted to a second holder or shared across two contexts of one process):
  1. Ctx A: `dst.contiguous_pixels_mut()` → first `&mut` live.
  2. Ctx A enters `blur_backdrop`, takes `COMPUTE_IO_GATE`.
  3. Ctx B (same `dst` `Arc`): `dst.contiguous_pixels_mut()` → **second `&mut` to the same bytes, simultaneously live.**
  `COMPUTE_IO_GATE` serializes the *write*, but both `&mut` slices are created **before** the gate — two aliasing `&mut [u8]` coexist → UB under Rust's uniqueness model, independent of whether the stores race.
- **Latent today** (no in-tree caller grants a dst write-cap twice; user multithreading is unbuilt — C1 unmerged), which is why this is a bounce for hardening, not a red-`main` escape. But the memory-safety invariant must not rest on "the compositor won't grant the write cap" — the new grantable-cap ABI is exactly what stresses it, so **the guard ships in the same build** (invariant-budget rule).
- **Fix (pick one):** (a) give `SharedPixelBuffer` a per-buffer `AtomicBool` borrow flag; `contiguous_pixels_mut` CAS-acquires and **fails closed (`EAGAIN`)** on a live second mutable borrow, releasing on guard drop — converts caller-discipline into an enforced invariant; or (b) acquire `COMPUTE_IO_GATE` **before** obtaining the mut slice. (a) is preferred (the guard lives with the buffer, not the caller). Add a negative test: two overlapping mut-borrows of one surface → the second gets `EAGAIN`.

## Must-fix 2 — state (and if needed, fix) the byte-oracle's provenance (lens 5)
Because `accel_sim`'s Q16 math (`accel_sim.rs:97`) is **byte-identical** to the CPU floor (`blur.rs:240`), all three tested paths (CPU floor, accel_sim "GPU", forced-degrade) share arithmetic — so the "GPU==CPU parity" leg is scalar==scalar (fine, and honestly scoped to the silicon gate), and **`ORACLE_COMPOSITE` (`test_harness.rs:4888`, `[u8;256]`) is the ONLY independent check** standing between "correct" and "consistently wrong."
- **State its provenance** in the test + the AAG doc. Per the project byte-oracle rule (CLAUDE.md: "hand-laid-out byte example… independent of the writer and reader code; round-trip tests are NOT sufficient"): if `ORACLE_COMPOSITE` was **hand-derived / computed by an independent reference routine**, cite that and you're done. If it was **snapshotted from a run of the implementation**, it's a golden-master self-check — the battery then proves determinism, not spec-correctness — and you must **add an independently-derived fixture** (hand-computed for the fixed 8×8 r=1 input, or a separate small reference computation not sharing the production blur/tint code) so the oracle catches shared-arithmetic spec-wrongness.
- This is the one item that bears on whether the green gate proves correctness at all — hence a merge-blocker, not a note.

## Minor (fix if cheap; not independently blocking)
- **RT floor uncovered:** the Reduce-Transparency `copy_backdrop` arm (`glass.rs`) is only toggled off at cleanup with **no draw+assert** in `run_gpu_glass_slice6`. If "RT ≡ solid floor, bit-for-bit" is claimed for Slice 6, add the draw+byte-assert; otherwise drop the claim from the RETURN for this slice.
- **FYI (no action):** a legitimate queue-cap holder can loop submits to consume handle-table slots (bounded resource use under valid authority, not escalation). And the `as_mut_slice(&self)→&mut` seam will need the I-14(c) DMA fence when a later slice introduces device-initiated DMA into these surfaces — noted for that build, nothing here.

## Close-gate + merge (re-submit)
- Fix must-fix 1 + 2 (+ RT assert if cheap), `scripts/worker-close-gate.sh` (rebase-fresh on `origin/main`), re-run the **4-shard `workflow_dispatch` QEMU gate** (the new negative test + any oracle-fixture change is kernel-touching), paste the green URL. PM re-runs the two affected lenses (concurrency-aliasing + completeness/oracle) on the fix, then `--ff-only`.
- **Disk reclaim (after PM confirms merge):** once merged and on `origin/main`, delete this build's tree to reclaim `target/` (verify the work is on `origin/main` first).

## Relay — what to tell Chad
- **Read:** `git fetch origin && git show origin/main:Plans/AAG-BOUNCE-2026-07-22-slice6-glass-into-g1-chad.md`. Work on the same branch `codex/gpu-p1-slice6-glass-into-g1-chad` (rebase-fresh first).
- **One-line:** **Slice-6 AAG: 4/5 lenses clean, strong work — two release-integrity fixes before merge.** (1) `contiguous_pixels_mut(&self)→&mut` has no in-use guard; two concurrent submits naming the same `dst` (write-cap is grantable) make two aliasing `&mut [u8]` (both created before `COMPUTE_IO_GATE`) → UB. Latent today but the new cap ABI widens it — add a per-buffer `AtomicBool` borrow-flag CAS that **fails closed `EAGAIN`** on a live second mut-borrow (preferred) or take `COMPUTE_IO_GATE` before the mut slice; + a negative test. (2) `ORACLE_COMPOSITE` is the only independent check (accel_sim Q16 == CPU-floor Q16, so the GPU==CPU parity is scalar==scalar) — **state its provenance**; if it's a golden-master snapshot of the implementation, add an **independently-derived** fixture per the byte-oracle rule. Minor: add the RT `copy_backdrop` draw+byte-assert or drop the RT≡floor claim. Re-run the 4-shard QEMU gate, paste the green URL; PM re-audits the two lenses then merges.
- **RAG** (PowerShell): `Invoke-RestMethod http://127.0.0.1:8404/health` first; e.g. `SharedPixelBuffer contiguous_pixels_mut DmaBuffer as_mut_slice`, `ORACLE_COMPOSITE run_gpu_glass_slice6 test_harness`, `COMPUTE_IO_GATE blur_backdrop`. If down, note it + grep/Read (PL owns RAG startup; never a blocker).
