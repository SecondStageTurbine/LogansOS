<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Kernel thread-slab UB on heavy spawn→reap→spawn (OT-29) → main-tier kernel coder

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Main-tier kernel coder (recommend **Cerebro** — surfaced it during M2, has the repro + the sched context; PL assigns). New branch off `origin/main` HEAD (`8ef7f89` or later): `codex/sched-thread-slab-spawn-reap-spawn-ub`.
**Status:** DISPATCHED. **A pre-existing scheduler memory-corruption bug on the headline critical path.** M2's falsifiable scenario surfaced it (QEMU run `27926197328`, shard 4): a **heavy** first arthur process (M2 crypto + multiple file writes) → **reap** → a **second** spawn trips a **thread-slab UB panic** — `ptr::write` to a null/misaligned pointer in `alloc_from_slab` (`kernel/src/sched/thread.rs:135`) on the second spawn. The **lighter** M1 `arthur_memory` driver also re-spawns and does **not** hit it — so this is **not** re-spawn alone; it smells like a **UAF / double-free corrupting the slab free-list** under a heavy first process's allocation/free pattern. M2 shipped with a single-spawn workaround (in scope); **the kernel bug is live.** Stage-B is a real *heavy* Arthur process, so heavy spawn→reap→spawn **must** be sound before bare-metal.

## 0. The decision this build makes
Turn "a heavy process's reap corrupts the thread slab so the next spawn faults" into "**heavy spawn→reap→spawn is sound**" — by root-causing the free-list corruption (most likely a node freed while still linked / freed twice / freed-then-reused on reap), fixing it, and re-enabling the spawn→reap→spawn-with-heavy-first-process pattern as a permanent regression. **Bug fix — correctness restoration, no kill-switch, no new invariant** (it *restores* an existing invariant: the thread-slab free-list integrity).

## 1. References (read before code)
```
git fetch origin
git show origin/main:kernel/src/sched/thread.rs        # alloc_from_slab (line ~135 — the fault site: ptr::write null/misaligned); the thread-slab alloc/free + the free-list
git show origin/main:kernel/src/sched/process.rs       # spawn / reap (terminate→reap) — where a reaped thread's slab node is returned to the free-list (the suspected corruption point)
git show origin/main:kernel/src/test_harness.rs        # the arthur_memory (M1, re-spawns, does NOT fault) vs arthur_memory_encrypted (M2, single-spawn now) drivers — the A/B that localizes "heavy first process"
git show origin/main:tools/xtask/src/main.rs           # ARTHUR_MEMORY_ENCRYPTED_SERIAL_STEPS (M2's single-spawn) — the spawn→reap→spawn pattern was removed here; the regression re-adds a heavy two-spawn driver
# the failing evidence: QEMU run 27926197328 shard 4 log (the panic + ptr::write null/misaligned in alloc_from_slab)
```
**The A/B is the gift:** M1 re-spawns and passes; M2's heavy first process re-spawns and faults. The differential (heavy alloc/free in process #1) localizes the corruption trigger — use it.

## 2. ISCs
- **ISC-1 — Reproduce first (capture-before-you-fix).** Re-create the failing pattern as a dedicated regression scenario `sched_spawn_reap_spawn_heavy`: spawn a process that does a **heavy** allocation/file-write workload (mirror M2's first-arthur load — or a synthetic equivalent that allocates/frees enough thread-slab-adjacent state), let it exit, **reap** it, then **spawn a second** process. Confirm it panics in `alloc_from_slab` at `thread.rs:135` **before** fixing — a fix you can't see fail first is unfalsifiable. At `NEO_SMP=4` (the bug surfaced under SMP).
- **ISC-2 — Instrument the slab free-list (the discriminator).** Add debug instrumentation that makes the corruption **self-diagnosing**: on every thread-slab free, **poison** the freed node (a sentinel) and record (addr, owner-tid); on every `alloc_from_slab`, **assert** the popped node is non-null, aligned, and **poisoned** (i.e. genuinely free — not live). This converts the downstream `ptr::write` fault into a precise verdict: **double-free** (node already poisoned when freed again), **freed-while-linked / UAF** (node not poisoned at alloc = handed out live), or **free-list head corruption** (null/misaligned head). State which the repro shows.
- **ISC-3 — Root-cause + fix.** From ISC-2's verdict, fix the actual defect (likely in the reap path returning a thread-slab node that's still referenced, or a double-return on terminate+reap, or a missing barrier under SMP so the free-list head is observed torn). Name the root cause in the commit + the ADR/ledger. **Compose existing slab primitives — do not rewrite the slab allocator** unless the root cause is in the allocator itself (flag if so).
- **ISC-4 — Regression is permanent.** `sched_spawn_reap_spawn_heavy` stays in the gate (armed forever, per the "discriminators are paid-down debt, not test-only weight" rule). The ISC-2 poison/assert instrumentation: keep it behind a debug-assertion path that the regression exercises (so a future re-corruption fails loudly), or as a permanent cheap invariant check if it's not hot.
- **ISC-5 — standards.** SPDX on new files. `fmt`/`clippy --workspace --all-targets -D warnings` via Docker (kernel clippy blocking). New `unsafe`: the slab path is already `unsafe`; do not add more than the fix needs — flag any addition. **No kill-switch** (bug fix, not a policy layer). **No new invariant** (restores the existing free-list-integrity invariant — state this; if you find you need a genuinely new cross-cutting invariant, STOP and flag).

## 3. Failure-mode + discriminator (the regression must be conclusive on pass AND fail)
- **Double-free on reap** ⇒ ISC-2 flags "node already poisoned at free." Most likely given the A/B (heavy process #1's reap returns a node twice).
- **UAF (freed-while-linked)** ⇒ ISC-2 flags "node not poisoned at alloc" (handed out a live node).
- **SMP free-list tear** ⇒ null/misaligned head only under `NEO_SMP>1`; add the missing barrier/lock. (The bug surfaced at NEO_SMP=4 — keep the regression there.)
- **Heap exhaustion masquerading** (heavy process #1 OOMs the slab) ⇒ the alloc path returns null legitimately; if so the fix is graceful-fail not corruption — distinguish via the poison check (a poisoned-null vs a corrupted-non-null).

## 4. Scope guard
Root-cause + fix the thread-slab free-list corruption on heavy spawn→reap→spawn + the regression + the discriminator instrumentation **only**. **Do NOT**: rewrite the slab allocator or the scheduler (compose/fix); change spawn/reap semantics beyond the corruption fix; re-architect thread lifetime; widen to unrelated sched cleanups. No kill-switch, no new invariant.

## 5. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST — `main` moves fast; the exact CI host-checks; SPDX; self-stat). Two hard rules: green CI URL (push-CI + the 4-shard QEMU `workflow_dispatch` URL — **kernel-touching ⇒ QEMU gate mandatory**, and the new `sched_spawn_reap_spawn_heavy` must be in it), and the PM re-inspects your `git show --stat`. PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**OT-29 CLOSED — heavy spawn→reap→spawn sound; Stage-B unblocked for a real heavy Arthur**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell the coder
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-sched-thread-slab-spawn-reap-spawn-ub.md` (+ `kernel/src/sched/{thread.rs,process.rs}`, `test_harness.rs` M1-vs-M2 drivers, QEMU run 27926197328 shard-4 log). **Branch:** `codex/sched-thread-slab-spawn-reap-spawn-ub` off `origin/main`.
- **One-line:** Root-cause + fix a **pre-existing kernel thread-slab UB** (OT-29): a **heavy** first process → reap → **second** spawn faults in `alloc_from_slab` (`sched/thread.rs:135`, `ptr::write` null/misaligned) at NEO_SMP=4; the **lighter** M1 arthur re-spawns fine, so it's **not** re-spawn alone — a **UAF/double-free corrupting the slab free-list** on the heavy process's reap. **ISC-1** reproduce first as `sched_spawn_reap_spawn_heavy` (heavy proc → reap → spawn, confirm the panic before fixing). **ISC-2 discriminator:** poison freed thread-slab nodes + assert-poisoned-on-alloc ⇒ verdict = double-free (already poisoned at free) / UAF (not poisoned at alloc) / SMP head-tear (null head only multi-CPU). **ISC-3** fix the root cause (likely reap returning a node twice or while still linked, or a missing SMP barrier on the free-list head); compose the slab primitives, don't rewrite. **ISC-4** the regression stays armed permanently. **No kill-switch, no new invariant** (restores free-list integrity). **Scope: the corruption fix + regression + instrumentation ONLY — no slab/scheduler rewrite, no spawn/reap semantic changes.** **Close-gate: `scripts/worker-close-gate.sh` (rebase LAST); RETURN the green push-CI + 4-shard QEMU URLs with `sched_spawn_reap_spawn_heavy` in the gate.**
