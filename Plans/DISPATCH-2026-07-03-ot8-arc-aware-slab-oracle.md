<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-8 — Arc-aware slab integration + dead-cache cleanup (oldest `mm` debt) → Oracle

**Date:** 2026-07-03
**From:** PM (Claude)
**To:** Oracle (main-tier — `mm`/SMP surface). New branch off `origin/main` HEAD (`8d3e128`): `codex/ot8-arc-aware-slab-oracle`.
**Status:** DISPATCHED — the **oldest coder-actionable `mm` debt** (OT-8, banked at the 2026-06-17 audit; ADR-012 amendments). Paying down oldest-first per PL direction. **Non-conflicting** with the WiFi (Cerebro/CB-5b), Arthur-agentic (Janus/A3), and bare-metal surfaces — this is `kernel/src/mm/slab.rs` internals. **Kernel-touching ⇒ 4-shard QEMU gate + AAG.**

## 0. The decision this build makes
`PROCESS_CACHE`/`CHANNEL_CACHE` are **declared + initialized but not actually allocated through** (slab.rs:23 + ipc/mod.rs:132 note them "wired" but processes/channels don't route allocation through the Arc-aware slab). ADR-012's amendment envisioned the Arc-aware slab integration. So OT-8 is a fork the build resolves: **wire them in** (route process/channel allocation through the Arc-aware slab, realizing the ADR-012 amendment) **OR remove them** as dead scaffolding (if the integration buys nothing today). Decide on evidence, not preference.

## 1. References
```
git fetch origin
git show origin/main:docs/ADR-012-buddy-slab-allocator-foundation.md   # the Arc-aware slab amendment OT-8 tracks
git show origin/main:kernel/src/mm/slab.rs                             # PROCESS_CACHE/CHANNEL_CACHE (726/740/780), the Magazine impl + Drop (359-366)
git show origin/main:kernel/src/ipc/mod.rs                             # :132 the CHANNEL_CACHE "declared but…" note
git show origin/main:tools/slab-magazine-tests/                        # the host KAT that `mem::forget`s a Magazine to dodge OT-32
git show origin/main:docs/OPEN-THREADS.md                              # OT-8 + OT-32 rows
```

## 2. ISCs
- **ISC-1 — OBSERVE + decide (wire-in vs remove).** Establish whether routing `Process`/`Channel` allocation through the Arc-aware slab (PROCESS_CACHE/CHANNEL_CACHE) is a real win today (allocation-pressure / fragmentation / the Arc-aware reclaim ADR-012 describes) or dead forward-scaffolding. **State the decision as the build's load-bearing call** with the evidence. Either path is legitimate; a defensible removal is as good as a defensible integration.
- **ISC-2 — Implement the decision.** *If wire-in:* route the allocations through the slab caches, realizing the ADR-012 amendment, with the equality discipline (behavior of the allocated objects unchanged — only the allocator path moves). *If remove:* delete `PROCESS_CACHE`/`CHANNEL_CACHE` + their `*_CONFIG`s + the `call_once` init + the `Magazine` fields that only they use, and reconcile the slab.rs:23 / ipc/mod.rs:132 comments. Nothing else changes.
- **ISC-3 — Verify + close OT-32 (likely already fixed).** `slab.rs:366` currently reads `dealloc(*self.buffer.get() as *mut u8, …)` — the **correct dereferenced form** (with the 359-360 comment). So the OT-32 `Magazine::Drop` bug appears **already fixed on `main`**. Confirm it (and check every `dealloc`/pointer site in the Magazine), and if fixed: **prune OT-32 from OPEN-THREADS** (closing evidence in the commit message) and, if `tools/slab-magazine-tests` still `mem::forget`s the Magazine only to dodge the old bug, drop the forget so the test actually exercises `Drop`. If somehow not fixed, apply the one-char fix. Either way the host KAT must exercise `Magazine::Drop` for real.
- **ISC-4 — Kill-switch / equality proof.** An `mm` allocator-path change ⇒ prove **disabled/removal ≡ current behavior bit-for-bit** (unit-test equality where expressible; the object lifecycle and the buddy/slab invariants unchanged). No behavior change to any allocated `Process`/`Channel`.

## 3. Standing-rule sections
- **Kill-switch (Rule 1):** see ISC-4 — the equality regression is the artifact (a wire-in ships a default-off switch collapsing to today's path; a removal is inert by construction and proven so).
- **Invariant budget (Rule 2):** preserve the ledger; **no new invariant** (allocator plumbing, not a cross-cutting property). I-13 (stale-handle / slot-generation) and the buddy/slab structural facts must be re-confirmed unchanged.
- **I-14 / locks:** no new lock; the slab caches are leaf allocators below the AI band. If wire-in touches a lock, keep it a leaf (no ordering above the existing slab lock) — re-measure the lock census if it changes (coordinate with Jarvis's OT-9, which reports memory-pressure; don't double-count).
- **Scope guard:** `mm/slab` + the OT-32 verify + the two comment reconciles ONLY. **Do NOT:** touch WiFi/agentic/forward surfaces; add an invariant; change the buddy allocator or the heap; or expand into OT-9's observability counters (Jarvis's lane).

## 4. Gate + close
- **Kernel-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` + AAG.** Push-CI green (host slab-magazine KAT incl. the now-real `Drop` + the equality). **Green without the CI run URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase onto `origin/main` — A3/OT-9 may land in parallel; re-run after rebase; host set; SPDX; self-stat).
- **RETURN** with the QEMU shard URLs + SHA, the ISC-1 decision + evidence, the equality proof, the OT-32 verification (fixed→pruned, or fixed-here), and confirmation nothing outside `mm/slab` changed. **Disk reclaim** after merge.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-03-ot8-arc-aware-slab-oracle.md` (+ ADR-012, `mm/slab.rs`, `slab-magazine-tests`, the OT-8/OT-32 rows). **Branch:** `codex/ot8-arc-aware-slab-oracle` off `origin/main` HEAD (`8d3e128`).
- **One-line:** **The oldest `mm` debt (OT-8).** OBSERVE whether the Arc-aware slab integration (`PROCESS_CACHE`/`CHANNEL_CACHE`, declared+init'd but not allocated-through) is a real win today or dead scaffolding, then **wire it in OR remove it** — either with an equality proof (disabled/removal ≡ today bit-for-bit). Fold OT-32: the `Magazine::Drop` bug **looks already fixed on main** (`slab.rs:366` = `*self.buffer.get()`) — confirm, prune the OT entry, and make `slab-magazine-tests` exercise `Drop` for real instead of `mem::forget`-ing around it. **No new invariant; `mm/slab` surface only; don't touch WiFi/agentic/forward or OT-9's observability counters.** **Gate:** 4-shard QEMU + AAG; rebase before close. RETURN shard URLs + SHA + the decision + equality proof + the OT-32 verdict.
