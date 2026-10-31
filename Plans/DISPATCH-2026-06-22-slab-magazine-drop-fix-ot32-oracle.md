<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-32 — Magazine::Drop frees the wrong pointer (kernel slab fix) → Oracle

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Oracle (do this FIRST, before E4 — small, self-contained warm-up). New branch off `origin/main` HEAD: `codex/slab-magazine-drop-fix-oracle`.
**Status:** DISPATCHED. **A latent one-char correctness bug in the per-CPU slab magazine** (`kernel/src/mm/slab.rs`), found by Cerebro while writing the OT-29 Magazine host test. `Magazine::Drop` frees **`self.buffer.get()`** (the `UnsafeCell`'s *own* address) instead of **`*self.buffer.get()`** (the heap buffer it points at). **Kernel-harmless today** (`SlabPerCpuState` is leaked → magazines are never dropped), so it's latent — but it's a real free-of-the-wrong-pointer that would corrupt if magazines ever drop. Fix it now while the slab is fresh (post-OT-29) and add the Drop coverage the existing Magazine test lacked.

## 0. The decision this build makes
Turn "`Magazine::Drop` would free the cell address, not the buffer" into "`Magazine::Drop` frees the buffer correctly" — a one-line fix + a host test that **fails against the pre-fix code and passes after** (the byte-oracle discipline: a test that can't fail on its target bug is theatre).

## 1. ISCs
- **ISC-1 — The fix.** In `Magazine::Drop` (`kernel/src/mm/slab.rs`), free `*self.buffer.get()` (the `*mut T`-array buffer), not `self.buffer.get()` (the `*mut UnsafeCell<…>`). Match the exact alloc layout used in `Magazine::new` (same `Layout`/element type/capacity). **No new `unsafe`** (the Drop is already `unsafe`; this corrects the pointer). No behavior change anywhere else.
- **ISC-2 — Host test for Drop (extend `tools/slab-magazine-tests`).** With a red-zone/tracking allocator (the OT-29 test's pattern), construct a `Magazine`, push/pop, then **drop it** and assert it frees **the buffer allocation** (correct address + size), not the cell. **Confirm it FAILS against the pre-fix `self.buffer.get()` and PASSES after** — state that you verified the fail-against-buggy direction.
- **ISC-3 — Equality / no-regression.** The slab's runtime behavior is unchanged (magazines still aren't dropped in the kernel today) — prove no regression: the OT-29 `sched_spawn_reap_spawn_heavy` regression + the slab unit tests stay green. **No kill-switch** (a correctness fix), **no new invariant** (restores correct Drop semantics — state this). SPDX; fmt/clippy via Docker (kernel clippy blocking).

## 2. Scope guard
The `Magazine::Drop` pointer fix + the Drop host test **only**. **Do NOT** touch the magazine push/pop logic (OT-29-fixed, frozen), the slab allocator structure, or anything outside `mm/slab.rs` + the test crate. No kill-switch/invariant/new-`unsafe`.

## 3. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; full set; SPDX; self-stat). Kernel-touching ⇒ **4-shard QEMU `workflow_dispatch`** + the host Magazine test green; RETURN both CI URLs + confirm the Drop test fails-against-buggy. PM merges → ledger (**OT-32 CLOSED**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`.

---
## Relay — what to tell Oracle (do FIRST, before E4)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-slab-magazine-drop-fix-ot32-oracle.md`. **Branch:** `codex/slab-magazine-drop-fix-oracle`.
- **One-line:** **OT-32 — one-char slab fix:** `Magazine::Drop` (`kernel/src/mm/slab.rs`) frees `self.buffer.get()` (the cell address) instead of `*self.buffer.get()` (the buffer) — latent (magazines never drop today) but a real wrong-pointer free. Fix it + **extend `tools/slab-magazine-tests` with a Drop test** (tracking allocator: drop a Magazine, assert it frees the *buffer* alloc; **confirm it fails against the pre-fix code, passes after** — no theatre). No behavior change (OT-29 `sched_spawn_reap_spawn_heavy` + slab unit tests stay green); no kill-switch/invariant/new-`unsafe`. **Scope: `mm/slab.rs` Drop + the test only.** Kernel-touching ⇒ 4-shard QEMU gate. **Close-gate: `scripts/worker-close-gate.sh`; RETURN push-CI + 4-shard QEMU URLs.**
