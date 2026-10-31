<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH #1: the 0.1 integration build — make `product` real, prove it boots → Chad

**Date:** 2026-08-07 · **From:** PM (Claude) · **To:** Chad (Codex)
**Status:** DISPATCHED — kernel-touching, **full 4-shard QEMU gate required**. First lane of the single-seat queue (`Plans/QUEUE-2026-08-07-single-lane-priority-order.md`).
**Closes:** **OT-78**. **Produces:** the ISO the 0.1 bare-metal showcase will boot.

## Why this is item 1

All five rungs of 0.1 are merged. **No single boot has run them together, and no build system anywhere assembles the configuration that would.** `product = ["boot-neofs", "boot-eth0"]` exists in `kernel/Cargo.toml` **and nowhere else** — no xtask scenario selects it, no CI job compiles it. So *the configuration that actually ships is the one configuration nothing builds*, which is OT-69's shape one level up: not a feature-gated subsystem invisible to CI, but the **product profile itself**.

That has a second consequence Cerebro measured: the rung-5 gate covers only the `Ok` arm. **All three `boot-neofs` degrade arms — the "boot continues" I-14(b) claims — are executed by nothing**, and `find_neofs_block_device`'s probe loop never runs its discriminating path because the single-disk scenario always hits index 0. This build fixes that by construction: it is the first thing that ever compiles and boots `product`.

**You are the seat** because you own eth0 (rung 4) and rung 5, and you composed `product` when `boot-neofs` and `boot-eth0` collided on the rebase. Least re-derivation.

## Scope

**1. Build `product` — and make CI build it.**
Add a job or matrix entry that **compiles `--features product`**. Cheap host job; it does not need a QEMU shard. Today nothing catches a `product`-only compile break, and one already shipped this way once (`accel-intel` failed to compile on `main` in the product configuration for weeks).

**2. A scenario that boots `product` end-to-end and observes all five rungs.**
One QEMU scenario, product-shaped, asserting each rung is *observed* rather than assumed:
- **rung 1** boots — framebuffer up, no panic
- **rung 2** desktop — ≥2 `ROLE_TEXT` surfaces composited, focus switch A↔B, only the focused client sees input, direct-present → `EPERM`
- **rung 3** Arthur answers — typed question → coherent reply, `provenance=` recorded
- **rung 4** `eth0` — link up + a lease, or the **clean no-dongle degrade** under QEMU (state which you are asserting; QEMU has no AX88179)
- **rung 5** persistence — something written survives, readable after

**Each assertion must be able to fail.** This is the week's dominant defect class and it has bitten six times: a printed string, a hardcoded literal, a loop that can iterate zero times, a check that can't fire. **Say in the RETURN, per rung, what makes that assertion fail if the property is violated** — and prefer a counter or a read-back over a log line.

**3. Exercise the three degrade arms.**
`fs/mod.rs:726-737` — `NoBlockDevice`, `NoValidSuperblock`, and the catch-all all claim *"boot continues"* and none is executed by any test. Add coverage: boot `boot-neofs` against **no disk** and against a **disk with no valid superblock**, and assert the boot proceeds. This is the I-14(b) liveness claim actually being tested rather than asserted.

**4. Kill-switch, as always.** `product` OFF ≡ the previous boot path, bit-for-bit. Note honestly that F1/F2 from rung 5 ride unconditionally (they change all builds), so state the equality claim in the form that is *true* rather than the form that sounds strongest — Cerebro flagged that the blanket wording was literally false.

## Explicitly out of scope
- **No bare-metal run.** That is queue item 3, after the disk-prep decision.
- **No disk re-`mkfs`, no touching the OptiPlex.** The physical volume question is item 2 and is PL-joint.
- **No OT-73 model-verification change.** It must be settled *after* the offline hash check, not inside this build.
- **Do not raise any scenario budget.**

## Close-gate + merge
`scripts/worker-close-gate.sh`, then the **4-shard `workflow_dispatch` on the exact SHA** — paste the URL; *green without a CI URL is not done*. Rebase onto `origin/main` immediately before declaring close. **The RETURN is a pushed artifact** at `Plans/RETURN-2026-08-07-product-integration-build-chad.md` — four seats delivered analysis that existed only in chat or on local disk this week, and one silently blocked a milestone rung for a day. Disk reclaim (`cargo clean`, don't delete) after I confirm the merge.

## Relay — Chad
- **WORKTREE:** `E:\AI Automation\workers\chad` (yours alone — sibling outside the repo, never a nested `Workers/`)
- **BRANCH:** `codex/product-integration-build-chad` (`git fetch origin && git checkout -B codex/product-integration-build-chad origin/main`)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-07-01-product-integration-build-chad.md`, the queue at `git show origin/main:Plans/QUEUE-2026-08-07-single-lane-priority-order.md`, and **OT-78** in `git show origin/main:docs/OPEN-THREADS.md`.
- **One-line:** **We're back online, one seat at a time, and you're first.** All five 0.1 rungs are merged but **no build system anywhere assembles the configuration that would run them together** — `product = ["boot-neofs","boot-eth0"]` exists in `kernel/Cargo.toml` and **nowhere else**: no xtask scenario, no CI job. The configuration that ships is the one configuration nothing builds (OT-78), which is also why **all three `boot-neofs` degrade arms are executed by nothing** and why `find_neofs_block_device`'s probe loop never runs its discriminating path. **Four things: (1)** make CI **compile `--features product`** — a cheap host job, no QEMU shard; nothing catches a product-only break today and one already shipped that way (`accel-intel` didn't compile on `main` in the product config for weeks). **(2)** One QEMU scenario that boots `product` and **observes all five rungs** — boot, two composited surfaces with focus switching and `EPERM` on direct present, Arthur answering, `eth0` (say whether you're asserting a lease or the clean no-dongle degrade, since QEMU has no AX88179), and persistence surviving. **Every assertion must be able to fail — say per rung what makes it fail**, and prefer a counter or read-back over a log line; that exact class bit us six times this week. **(3)** Exercise the three degrade arms at `fs/mod.rs:726-737` — boot with no disk and with an invalid superblock, assert the boot proceeds; that's the I-14(b) claim being *tested* instead of *asserted*. **(4)** Kill-switch OFF ≡ prior path, stated in the form that's true — Cerebro showed the blanket wording is literally false because F1/F2 ride unconditionally. **Out of scope:** no bare-metal run, no touching the OptiPlex disk, no OT-73 change, no budget raises. Close-gate + 4-shard on the exact SHA with the URL, rebase first, and **push the RETURN**.
- **RAG** health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `product feature boot-neofs boot-eth0`, `mount_boot_neofs degrade arms boot continues`, `xtask scenario registration features`, `arthur_compositor ROLE_TEXT focus`. If down, note it in the RETURN and use grep/Read (PL owns RAG startup — never a blocker).
