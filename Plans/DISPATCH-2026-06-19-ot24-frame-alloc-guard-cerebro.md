<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-24 — per-CPU frame-allocator reentrancy guard (promote the verified A/B fix) → Cerebro

**Date:** 2026-06-19
**From:** PM (Claude)
**To:** Cerebro (unmasked OT-24 during CI-hygiene; mm is your lane; CI-hygiene now landed `add484c`). New branch off `origin/main` HEAD: `codex/ot24-frame-alloc-guard-cerebro`.
**Status:** DISPATCHED. **De-flake the kernel gate at the source.** CI-hygiene made the gate hermetic, but a *real* intermittent remains: OT-24 — the debug-build frame-allocator reentrancy guard false-positive-panics when two APs contend during SMP bring-up. The A/B OT-24 cell already produced a **PM-verified correct fix** (`ot24-kimi` `34843d0`): a **per-CPU guard array** replacing the single global `AtomicBool`. This dispatch re-homes that verified diff onto a proper branch, gates it, and merges — closing OT-24 and removing the last intermittent reddener of kernel-touching gates.

## 0. The decision this build makes
The same-CPU reentry guard becomes **per-CPU** so concurrent APs don't false-trip it. **Debug-only behavior change** (the guard lives under `#[cfg(debug_assertions)]`; release builds have no guard and are bit-for-bit unaffected). Correctness fix (I-1), not a feature. No new invariant, no AI-governance touch.

## 1. Context — the verified diagnosis (already banked)
PM-verified against `kernel/src/mm/mod.rs` (via the A/B OT-24 probe; DeepSeek's diagnosis + Kimi's fix, both PASS):
- `IN_FRAME_ALLOC` is a **single global `static AtomicBool`** under `#[cfg(debug_assertions)]`. The real allocator (`FRAME_ALLOC: Mutex<Option<BuddyAllocator>>`) is already cross-CPU safe; the guard was only meant to catch **same-CPU reentry** of `with_frame_alloc`.
- Being **global**, it false-positives when two APs call `with_frame_alloc` concurrently during bring-up → debug-build panic on `attempt 1`, absorbed by the harness's 4-attempt retry (`pass attempt 2`) → masked the bug while reddening gates intermittently (the "green CI rots" pattern; surfaced in run `27799088852` even on the green CI-hygiene run).
- **No memory corruption; release unaffected.** (My earlier "I-1/potential memory corruption" framing was corrected to debug-only false-positive per lowest-category-evidence — `955055d`.)
- **Fix (Kimi, `ot24-kimi` `34843d0`, verified):** per-CPU guard array indexed by `current_cpu_id()`. The early-bring-up caveat is **resolved**: the AP wires its GS-base in `per_cpu::init_ap` at `smp.rs:139`, *before* the `with_frame_alloc` call sites at `smp.rs:151/152`, so `current_cpu_id()` is valid at every guard use. `frame.rs` comment corrected. No deadlock.

## 2. References
```
git fetch origin
git show origin/ot24-qwen:kernel/src/mm/mod.rs     # PREFERRED verified diff (27408d9, +22/-8; adds a MAX_CPUS bounds assert)
git show origin/ot24-kimi:kernel/src/mm/mod.rs     # equivalent verified diff (34843d0) — fallback
git show origin/main:docs/OPEN-THREADS.md          # OT-24 row (severity DOWNGRADED; two candidate fixes verified)
git log --oneline -1 origin/main                   # base = add484c (CI-hygiene; gate now hermetic)
```
**Two PM-verified per-CPU-guard fixes exist** — `ot24-qwen` (`27408d9`, Qwen-27B/Prometheus) and `ot24-kimi` (`34843d0`, Kimi); same approach, both correct. **Adopt `ot24-qwen` as the base** (it adds an explicit `cpu_id < MAX_CPUS` bounds assert before indexing — fail-loud, not OOB — and a clearer doc comment). **Don't re-derive.** Inspect it, confirm it matches §1, re-home it onto your branch. (Leave any untracked A/B cruft behind — e.g. `kernel/src/mm/temp_snippet.txt`, a `linux-kernel/` dir; **do not** carry those in.)

## 3. ISCs
- **ISC-1 — Per-CPU guard.** Replace the global `IN_FRAME_ALLOC: AtomicBool` with a per-CPU array (size = `MAX_CPUS`), indexed by `current_cpu_id()`, still under `#[cfg(debug_assertions)]`. Same-CPU reentry still panics (the guard's real purpose); cross-CPU concurrency does not. Confirm `current_cpu_id()` validity at every `with_frame_alloc` call site (the `smp.rs:139`-before-`151/152` ordering — assert it holds; if any call site can run before GS-base is wired, guard that path).
- **ISC-2 — The regression proof (the kill-switch-equivalent).** Release build: **bit-for-bit unaffected** (no guard compiled). Debug build: add a host/unit test that simulates two distinct `current_cpu_id()` values entering `with_frame_alloc` concurrently and asserts **no false panic**, plus a same-CPU reentry case that **still** panics (the guard still does its job). This is the "disabled ≡ previous, the change is provable" discipline mapped onto a bug fix.
- **ISC-3 — Standards.** `kernel/src/mm` only. No new `unsafe` beyond what the array access needs (SAFETY-doc it). No new invariant. SPDX intact. `fmt`/`clippy --workspace --all-targets -D warnings` + blocking kernel clippy via Docker.

## 4. Gate
- **Host:** the ISC-2 regression (two-CPU no-false-panic + same-CPU still-panics), green; builds debug + release.
- **The full 4-shard `workflow_dispatch` QEMU gate green** — and this build's *point* is that the gate stops flaking: confirm the kernel-touching shards pass **without** relying on the retry absorbing an OT-24 panic (ideally run it twice to show determinism, now that the gate is also hermetic per `add484c`).
- Kernel-touching ⇒ the pre-merge `workflow_dispatch` QEMU gate is mandatory.

## 5. Scope guard
The per-CPU guard + its regression test **only**. **Do NOT** touch the real allocator (`FRAME_ALLOC`/`BuddyAllocator` — already correct), change release-build behavior, refactor `with_frame_alloc`'s callers, or carry any A/B-worktree cruft. No new invariant/`unsafe` beyond the array index.

## 6. Merge (PM)
**Close-gate: run `scripts/worker-close-gate.sh`** (rebase-freshness vs current `main`, the exact CI host-checks on the rebased tree, SPDX, self-stat). The two hard rules: **"green" without a CI URL is not done** (paste the green push-CI URL + the `workflow_dispatch` QEMU URL — kernel-touching; show the gate green deterministically), and the PM re-inspects your commit's own `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**OT-24 CLOSED**; the kernel gate is now both hermetic *and* race-free). Next: the parked pipeline re-gates clean — CSPRNG 10.3, OT-23 (YMM/XSAVE), Arthur M1.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main` and confirmed. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Cerebro
- **Dispatch (on `main`):** `Plans/DISPATCH-2026-06-19-ot24-frame-alloc-guard-cerebro.md` — read via `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-19-ot24-frame-alloc-guard-cerebro.md`. **Branch:** `codex/ot24-frame-alloc-guard-cerebro` off `origin/main` (HEAD = `add484c`, CI-hygiene landed → gate is hermetic).
- **One-line:** **Promote the verified OT-24 fix.** Adopt the `ot24-qwen` (`27408d9`) diff (preferred over the equivalent `ot24-kimi` `34843d0` — it adds a `MAX_CPUS` bounds assert) — a **per-CPU guard array** replacing the single global `IN_FRAME_ALLOC: AtomicBool` (debug-only; release is bit-for-bit unaffected) so concurrent APs stop false-tripping the same-CPU reentry guard during bring-up. The early-bring-up caveat is resolved (`current_cpu_id()` valid: GS-base wired at `smp.rs:139` before `with_frame_alloc` at `151/152` — assert it). Re-home the diff onto your branch (drop the A/B worktree cruft: `temp_snippet.txt`, `linux-kernel/`). **Regression proof:** debug-build test that two distinct CPU ids enter `with_frame_alloc` concurrently with **no false panic**, and same-CPU reentry **still** panics; release bit-for-bit. **Scope: `kernel/src/mm` only** — don't touch the real `FRAME_ALLOC`/`BuddyAllocator` (already correct) or release behavior. Gate: host regression + the full 4-shard QEMU gate green **deterministically** (run twice — the point is no more retry-absorbed flake). **Close-gate: run `scripts/worker-close-gate.sh`; RETURN pastes the green push-CI + `workflow_dispatch` QEMU URLs.** Closes OT-24.
