<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH #1b: give rung 3 teeth, fix the boot-1 evidence label → Chad

**Date:** 2026-08-07 · **From:** PM (Claude) · **To:** Chad (Codex)
**Status:** DISPATCHED — small, same files, kernel-touching ⇒ **4-shard gate**. Finishes queue item 1; item 2 does not start until this merges.
**Base:** `origin/main` @ `966bdabf` (the integration build is merged).

## Why there is a 1b at all

The AAG (Cerebro, `Plans/RETURN-2026-08-07-aag-product-integration-cerebro.md`) returned **MERGE with two confirmed findings**, and the build is on `main`. Neither makes `main` worse. But both must land **before the bare-metal showcase**, because that run is where these two would be cited as proof.

**This is the seventh instance of the week's dominant defect class, and the first that survived a dispatch explicitly demanding it not happen.** The dispatch said *"every assertion must be able to fail — say per rung what makes it fail."* Rungs 1, 2, 4 and 5 have real mutation arguments and go red. **Rung 3's input half does not**, and the RETURN claimed teeth anyway. The lesson is not that anyone was careless — it is that **stating the requirement is not the same as testing for it**, and the mutation table is what caught it.

## F1 — rung 3 asserts the prompt at the sender only (the one that matters)

`bytes=12` counts twelve successful `sys_input_focus_test_inject` calls. **Arthur fires inference on any `input_len != 0` and never compares the buffer**, and the fixture graph selects token 3 deterministically regardless of input — so the reply carries **zero information about the prompt**, and `reply != input` still holds for a truncated one. Drop keystrokes mid-path: Arthur receives `hlowrd\n`, replies `Arthur is ready.`, **gate stays green** — while two strings claim the property (`typed question=hello_world`, `typed_prompt=hello_world`).

**The evidence already exists — Arthur logs every keystroke. It is simply unasserted.**

**Fix:** assert the **received** buffer, not the send count. Compare what Arthur got against what was injected, and fail on mismatch. Then state the mutation that proves it: *drop or reorder an injected keystroke ⇒ rung 3 goes red.* If the comparison cannot be made where the log is emitted, say why rather than substituting a weaker check.

**Why this one is not optional before hardware:** Cerebro notes **F1 leaves no QEMU-side fallback on the real HID path.** On the OptiPlex the typed prompt travels a different route entirely, so rung 3 arrives at the showcase with its input half unproven in *both* environments.

## F2 — boot 1's terminal line mislabels its own evidence

Boot 1 prints `rung5=two_boot_byte_readback`, but boot 1 only performed a **same-boot** readback. Boot 2 correctly says `second_boot=prior_disk_byte_readback`. **The boot-1 line is the one carrying all five rung tokens — so it is the line someone would cite.** Relabel it to what boot 1 actually did.

## Also fold in, both from the AAG and both cheap

- **Lens 3 note:** the CI chain is stricter than the stated goal. **`neo-test: needs: [host-checks, product-build]`** gets the same protection without putting a kernel build in front of `fmt`/`clippy` feedback. Make that change unless you disagree — say so if you do.
- **Lens 4 precision:** the probe counter rides **all** `neo-test` builds, not just the three degrade scenarios. Either scope it or note it accurately.

## Not in scope
Rung 5's **fsync necessity** is unprovable under QEMU (it flushes on clean shutdown, so a build with `fsync` deleted likely still passes). Cerebro flagged it as **pre-existing, not this build** — leave it; I am recording it as an open thread rather than smuggling it into a small fix.

## Close-gate + merge
`scripts/worker-close-gate.sh`, rebase onto `origin/main`, **4-shard `workflow_dispatch` on the exact SHA with the URL**. **RETURN pushed** at `Plans/RETURN-2026-08-07-1b-rung3-teeth-chad.md`.

## Relay — Chad
- **WORKTREE:** `E:\AI Automation\workers\chad` · **BRANCH:** `codex/rung3-teeth-chad` (`git fetch origin && git checkout -B codex/rung3-teeth-chad origin/main`)
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-07-1b-rung3-teeth-chad.md` and the AAG at `git show origin/main:Plans/RETURN-2026-08-07-aag-product-integration-cerebro.md`
- **One-line:** **Your build merged (`966bdabf`) — AAG verdict was MERGE with two findings, and both need closing before the hardware boot.** **F1 is the real one and it's the seventh instance of this week's class, the first to survive a dispatch that explicitly demanded it not happen:** rung 3 asserts the prompt **at the sender only**. `bytes=12` counts successful injections; **Arthur fires on any `input_len != 0` and never compares the buffer**, and the fixture picks token 3 regardless of input — so drop keystrokes mid-path and Arthur gets `hlowrd\n`, replies `Arthur is ready.`, and **the gate stays green** while two strings claim `typed_prompt=hello_world`. **The evidence is already there — Arthur logs every keystroke — it's just unasserted.** Assert the **received** buffer against what was injected, and give the mutation: *drop or reorder a keystroke ⇒ rung 3 red*. **This can't wait for hardware, because Cerebro notes F1 leaves no QEMU-side fallback on the real HID path** — rung 3 would reach the showcase unproven in both environments. **F2:** boot 1 prints `rung5=two_boot_byte_readback` but only did a same-boot readback; boot 2 is correct. **The boot-1 line carries all five rung tokens, so it's the one someone would cite** — relabel it to what boot 1 actually did. **Two cheap folds:** make it `neo-test: needs: [host-checks, product-build]` so a kernel build doesn't sit in front of fmt/clippy feedback (say so if you disagree), and either scope the probe counter or note accurately that it rides all `neo-test` builds. **Not in scope:** rung 5's fsync necessity — unprovable under QEMU, pre-existing, I'm logging it separately rather than smuggling it in here. Close-gate, rebase, 4-shard on the exact SHA with the URL, **push the RETURN**.
- **RAG** health-check first; e.g. `sys_input_focus_test_inject arthur keystroke`, `arthur inference input_len fixture token`, `product integration rung terminal line`.
