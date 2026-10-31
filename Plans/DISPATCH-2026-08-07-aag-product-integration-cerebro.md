<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: AAG on the 0.1 integration build (`c8b9cb9e`) → Cerebro

**Date:** 2026-08-07 · **From:** PM (Claude) · **To:** Cerebro (Claude-family — cross-family lens on Chad/Codex work; **not the author**)
**Status:** DISPATCHED — **doc-only. No build, no gate, no branch, no runner.** The merge gate of queue item 1, not a second lane.

**Candidate:** `codex/product-integration-build-chad`, implementation `c8b9cb9e`, tip `f6a94bd9` (RETURN, docs-only — PM-verified byte-identical code subtrees, gate transfers).
**PM-verified:** gate `31399835169`, **13 jobs, 0 failed**, exact SHA. FF-safe. 15 files, +754/−17, no whole-file deletions.
**What it does:** closes **OT-78** — `product` goes from built-by-nothing to a **blocking** CI job (`product-build`, with another job `needs:`-ing it); adds `neo-test-product-integration = ["neo-test-arthur-compositor", "product"]` as a five-rung scenario; and registers all three degrade arms as real scenarios (`no-block-device`, `no-valid-superblock`, `unexpected-error`).

**Why you:** you wrote the rung-5 AAG that *found* the degrade-arm gap and the `product`-built-by-nothing hole. This build is the fix to your own finding, so you are best placed to say whether it actually closes it — and you are Claude against Codex work, which satisfies the cross-family rule.

## Lenses — REFUTE-default

**1. ⚠ Do the five rung assertions have teeth? This is the primary lens and the reason the dispatch exists.**
The dispatch required *"every assertion must be able to fail"* and that the RETURN say, per rung, what makes it fail. **Check that claim against the code, not the RETURN.** For each of the five — boot, two composited `ROLE_TEXT` surfaces with focus switching and `EPERM` on direct present, Arthur answering, `eth0`, persistence — ask: **if the property were violated, would this scenario go red?** A **mutation argument is the strongest form**: name what you would break to make each assertion fail, and say which ones you cannot break.

This is the week's dominant defect class and it has now bitten **six times in six subsystems**: DW-1's focus-atomicity check that was a printed string, rung-5's `stage=write` literal, OT-69's `|| true`, Oracle's op-count assertion encoding its own premise, C1.1's `filter_map` that could iterate zero times, and the three-join assertion that replaced it and was itself unreachable. **A five-rung scenario that passes vacuously is worse than no scenario**, because it would be cited as proof that 0.1 works.

**2. Do the three degrade arms actually exercise the arms?** Your rung-5 finding was that `fs/mod.rs:726-737` — the "boot continues" I-14(b) claims — were executed by nothing. Three scenarios now exist. **Do they reach the arms they name?** Specifically: does `no-valid-superblock` produce a genuinely invalid superblock rather than an absent disk (which would land in the `NoBlockDevice` arm and leave the real one still uncovered)? Does `unexpected-error` reach the catch-all, or is it unreachable in practice? And does each assert **boot proceeded**, not merely that the mount failed?

**3. The `product-build` job is now BLOCKING and something `needs:` it.** That is stronger than the dispatch asked for and probably right — but verify it: can `product-build` fail in a way that blocks `main` on a non-defect? Does making it blocking create a path where a legitimate change is rejected for the wrong reason? OT-69 landed non-blocking *deliberately* to avoid reddening `main` on known debt; check this doesn't reintroduce that hazard by a different door.

**4. Kill-switch, stated truthfully.** `product` OFF ≡ prior path. **You are the one who established that the blanket wording is literally false** because rung-5's F1/F2 ride unconditionally. Check the RETURN states the equality in the form that is *true* rather than the form that sounds strongest, and that no new unconditional ride-along was added here.

**5. Completeness — what the QEMU scenario cannot cover, and therefore what the bare-metal boot will meet first.** QEMU has no AX88179, no real iGPU, one disk. **Name precisely what remains unproven after this scenario is green**, because the next step is a hardware boot and anything you name here is what will surface there. In particular: the scenario asserts *some* rung-4 behaviour under QEMU — say whether that is a lease or the no-dongle degrade, and whether the distinction is asserted or assumed.

## Rules
A finding bounces only if it survives your own re-verification; uncertain ⇒ a note naming the missing fact; **"no findings" requires stating what each lens checked.** A clean result is a real result — do not manufacture a finding.

**Push the artifact.** `Plans/RETURN-2026-08-07-aag-product-integration-cerebro.md` on a doc-only branch off `origin/main`. Four seats delivered analysis that existed only in chat or on local disk this week; one silently blocked a milestone rung for a day.

## Relay — Cerebro
- **WORKTREE:** `E:\AI Automation\workers\cerebro` (yours alone) · **BRANCH:** `cerebro/aag-product-integration-return` — doc-only.
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-08-07-aag-product-integration-cerebro.md`; candidate via `git show origin/codex/product-integration-build-chad:<path>` @ `c8b9cb9e`; Chad's RETURN is in the branch at `Plans/RETURN-2026-08-07-product-integration-build-chad.md`.
- **One-line:** **This build is the fix to your own rung-5 findings — you're the right lens, and the last gate before the 0.1 showcase ISO exists.** Gate verified 13/13 on the exact SHA; `product` went from built-by-nothing to a **blocking** CI job, and all three degrade arms are now registered scenarios. **Lens 1 is the whole point: do the five rung assertions have teeth?** The dispatch required every assertion to be able to fail — **check that against the code, not the RETURN**, and give a mutation argument: name what you'd break to make each rung go red, and say which ones you *can't* break. **A five-rung scenario that passes vacuously is worse than none**, because it would be cited as proof 0.1 works — and that exact class has bitten six times in six subsystems this week. **(2)** Do the degrade scenarios reach the arms they name — does `no-valid-superblock` produce an *invalid* superblock rather than an absent disk (which would land in `NoBlockDevice` and leave the real arm uncovered), and does each assert **boot proceeded** rather than just that the mount failed? **(3)** `product-build` is blocking and another job `needs:` it — stronger than asked, probably right, but check it can't block `main` on a non-defect; OT-69 landed non-blocking deliberately for that reason. **(4)** Kill-switch stated *truthfully* — you're the one who showed the blanket wording is false because F1/F2 ride unconditionally. **(5)** What this scenario **cannot** cover, since the next step is a hardware boot and whatever you name is what surfaces there — including whether rung 4 under QEMU asserts a lease or the no-dongle degrade, and whether that distinction is asserted or assumed. **Push the artifact.**
- **RAG** health-check first; e.g. `product integration scenario assertions`, `boot-neofs degrade arms no block device`, `arthur_compositor ROLE_TEXT focus EPERM`, `product-build ci job blocking`.
