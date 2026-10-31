<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REQUIRED FOLLOW-UP: complete SharedPixelBuffer borrow discipline to shared-XOR-mutable → (Chad, hard-gated to C1.1)

**Date:** 2026-07-22
**From:** PM (Claude) — Slice-6 AAG re-audit residual
**Status:** **TRACKED + HARD-GATED (not a bounce).** Slice-6 merged (`7c103ff`) — the bounced mut-vs-mut aliasing defect is genuinely closed and QEMU-tested. This records the **residual of the same class** the re-audit confirmed, and binds its fix as a **prerequisite to C1.1 (threads)** and to any build that widens the exposure.

## The residual (CONFIRMED, latent-not-reachable today)
Slice-6's `contiguous_pixels_mut(&self)` guard (`kernel/src/atrium/buffer.rs`, per-buffer `AtomicBool` CAS → `EAGAIN`) closes **mut-vs-mut** only. The immutable reader `contiguous_pixels(&self)` (`buffer.rs:212-218`) returns `unsafe { as_slice() }` and **never consults the borrow flag**. So two *crossed* concurrent submits — Submit-1 `(src=X, dst=Y)` holds `&mut Y`; Submit-2 `(src=Y, dst=X)` takes `Y.contiguous_pixels()` → `&[u8]` to Y — produce `&[u8]` + `&mut [u8]` to the same buffer → **UB under shared-XOR-mutable**, independent of whether stores race. Each submit passes its own `Arc::ptr_eq(src,dst)` (X≠Y); the mut-CAS on the dst buffers (Y then X) never collides. The guard's own comment claims "the sole mutable view" (`buffer.rs:234`) — but that invariant is not enforced against readers.

**Why not a bounce:** identical latency to the original defect — **not reachable on the current tree**: no preemptive user threads until C1.1 lands, and production mints the compute write-cap only to the single trusted (single-threaded-draw) compositor. Merging Slice-6 ships zero exploitable UB.

**Why hard-gated, not deferred-vague:** C1.1 (threads) is exactly what makes it reachable. The complete fix is a **shared-XOR-mutable (reader/writer) discipline** — a state machine (`Free` / `Readers(n)` / `Writer`), where `contiguous_pixels` also takes an RAII reader guard that (a) fails/coordinates against a live writer and (b) blocks a subsequent writer — best designed against the actual C1 scheduling model, alongside the I-14(c) DMA fence CLAUDE.md already defers for these same buffers.

## The fix (do this in/before C1.1, gate it)
1. **Reader/writer borrow state on `SharedPixelBuffer`.** Replace the mut-only `AtomicBool` with a shared-XOR-mutable guard: `contiguous_pixels` returns an RAII reader guard (readers ok concurrently), `contiguous_pixels_mut` requires zero readers **and** no writer (fail closed `EAGAIN`), each released on drop. Update all call sites of `contiguous_pixels` (glass draw `src`, the submit path `src`) to hold the reader guard for the borrow's lifetime.
2. **Negative test (QEMU):** the crossed-submit repro — a live `&mut` to Y and a concurrent `contiguous_pixels()` on Y → the reader gets `EAGAIN` (or is correctly serialized). Prove shared-XOR-mutable holds, not just mut-xor-mut.
3. **Dead-code trap:** `SharedPixelBuffer::blur_backdrop` (`buffer.rs:619`, `#[allow(dead_code)]`) calls `as_mut_slice()` directly, **bypassing the CAS guard** — dead in production (only `test_harness.rs` single-threaded scenarios call it). Either remove it or route it through `contiguous_pixels_mut` so a future slice can't wire it live and bypass the guard.

## Gate binding
**C1.1 (threads + futex) MUST NOT merge until this shared-XOR-mutable discipline is in place** (or a proof that no preemptive thread / crossed concurrent submit can reach a `SharedPixelBuffer` — unlikely once threads exist). The C1.1 dispatch's close-gate references this file; the PM merge-check blocks C1.1 on it. Same standing as the I-14(c) DMA-fence prerequisite for device-initiated DMA into these buffers.
