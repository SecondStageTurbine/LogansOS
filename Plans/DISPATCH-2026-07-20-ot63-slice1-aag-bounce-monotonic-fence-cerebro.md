<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-63 Slice 1 AAG bounce — monotonic present-fence + test the full-ring path → Cerebro

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Cerebro (branch `codex/ot63-rung1-gen9-present-cerebro` @ `2dcceb29`)
**Status:** BOUNCE — Slice 1's load-bearing paths are CLEAN (the async fence is lost-wakeup-free; the kill-switch is armed bit-for-bit; no Rung-0 regression), but the AAG confirmed **one correctness defect on the I-14(b) overload path**, plus a completeness gap (that path is untested). Cheap fix; re-gate + re-AAG-delta.

## The finding (CONFIRMED, PM-verified in the code)
`publish_complete` (`buffer.rs:251`) does an **unconditional** `completed_epoch.store(epoch, Ordering::Release)`. But `fence_satisfied` (`logic.rs:209`) is `completed_epoch >= wait_epoch`, which **relies on `completed_epoch` being monotonic-non-decreasing** (the ISC-39 buffer-reuse ABA guard). The normal FIFO-drain path preserves monotonicity — **the full-ring I-14(b) fallback does not.**

**Concrete repro (ring saturation, `RING_CAP=16`):**
1. 17 serialized presents assign epochs e1 < … < e17.
2. e1..e16 arm the ring; e17 finds the ring **full** → `present` **synchronously** `publish_complete(e17)` → `completed_epoch = e17`.
3. Waiter W17 = `fence_wait(e17, …)`.
4. The vblank ISR drains the ring in FIFO order: pops e1 → `store(e1)` → **`completed_epoch` REGRESSES e17 → e1**; … drains to e16 → `completed_epoch = e16`.
5. e17 is never in the ring, so it is never re-published. `e16 < e17` forever → **W17 strands until `ETIMEDOUT`** despite e17 having completed. (Transient: step-2's momentary e17 also lets an earlier waiter, e.g. on e10, false-satisfy and read the single-slot `result`.)

This is precisely the **I-14(b) overload/degradation path the design touts as correct** ("full ring → synchronous fallback, never a drop, never an unbounded wait") — but under saturation it produces exactly an unbounded-until-timeout wait. Not hit at 60fps (per-tick drain, one present in flight), but **reachable under load and MORE reachable at Slice 4** (real vblank + higher present rates).

## The completeness gap (why the green gate missed it)
`atrium_hw_present` only ever has **one present in flight** — it **never fills the ring**, so the full-ring fallback path (`sw_sync`/`hw_deferred`/`hw_bounded_timeout`/`hw_async_isr` all test the non-saturated path) is **claimed but never exercised**. A green 4-shard gate does not cover this — so the gate result (still queued) doesn't change the bounce.

## The fix (both parts)
1. **Make the publish monotonic (the real fix):** `completed_epoch.fetch_max(epoch, Ordering::Release)` instead of the unconditional `store` in `publish_complete`. This preserves the ISC-39 monotonic invariant regardless of publish order (FIFO drain or full-ring sync fallback), is cheap, and is robust to any future out-of-order publish. Keep the `SFENCE`-before-publish. *(Alternatively/additionally: drain the ring before the sync-fallback publish — but `fetch_max` is the robust primitive and I recommend it as the fix.)*
2. **Arm the untested I-14(b) path (mandatory — close the completeness gap):** add a **ring-saturation case** to `atrium_hw_present` (≥ `RING_CAP+1` in-flight presents) that (a) triggers the full-ring sync fallback, then (b) asserts the sync-published waiter (W17) **completes, does not strand to `ETIMEDOUT`**, and (c) asserts no earlier waiter false-satisfies (the transient). This is the falsifiable proof the I-14(b) fallback is actually correct — currently it's asserted, not proven.

## Close-gate + merge
- Run `scripts/worker-close-gate.sh` (rebase-fresh — main advanced by docs since your base; the full host-check set; SPDX; Step-4 self-stat). Kernel-touching ⇒ re-fire the 4-shard `workflow_dispatch` QEMU gate on the re-spun head; RETURN the green push-CI + 4-shard URLs. The new ring-saturation scenario assertion + the `fetch_max` change are the delta.
- PM re-AAG-delta: confirm the publish is monotonic (`fetch_max`), the ring-saturation case genuinely exercises the full-ring fallback + is armed (hard-fail if W17 strands), and nothing else changed. Then `--ff-only` merge.
- **Disk reclaim** after PM-confirmed merge (as usual).

## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-ot63-slice1-aag-bounce-monotonic-fence-cerebro.md`.
- **One-line:** Slice 1 is almost there — the async fence + kill-switch are AAG-clean, but the **I-14(b) full-ring fallback has a real bug**: `publish_complete`'s unconditional `completed_epoch.store` **regresses the epoch** when the FIFO ISR drain runs after a full-ring *sync* publish (out of order), so a saturated-ring waiter strands to `ETIMEDOUT` — the exact degradation path the design promises is safe. **Fix = `completed_epoch.fetch_max(epoch, Release)` instead of `store`** (monotonic, cheap, robust), **and add a ring-saturation case (≥17 in-flight) to `atrium_hw_present`** proving the sync-published waiter completes + no earlier-waiter false-satisfy (that path is currently never tested — one present in flight — which is why the green gate missed it). Re-gate 4-shard + I'll re-AAG-delta + merge. Everything else stands — this is one one-liner + one scenario case.
- **RAG:** available for the fence/ring seams (`publish_complete completed_epoch`, `fence_satisfied monotonic`, `present_backend ring full fallback`). Health-check first; if down, note it + grep/Read.
