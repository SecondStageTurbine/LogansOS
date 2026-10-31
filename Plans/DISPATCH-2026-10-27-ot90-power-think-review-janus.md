<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #93): cross-family read of the OT-90 shutdown/restart THINK (PR #153) → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** high. A design doc for ACPI power-off and reset, which shut down the CPUs, DMA and
the filesystem on the way out. Claude-authored (S5), so it gets a Codex-family read before rung P1. **Do this after
relay #91.**

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `06ea9d75` (PR #153, branch
`claude/ot90-power-think`), `docs/THINK-2026-10-27-ot90-shutdown-restart.md`.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot90-power-think-review-janus
          git fetch origin && git checkout -B codex/ot90-power-think-review-janus 06ea9d75

git show origin/main:Plans/DISPATCH-2026-10-27-ot90-power-think-review-janus.md   # this file
git show origin/main:Plans/DISPATCH-2026-10-27-ot90-power-think-s5.md             # the dispatch
git show 06ea9d75:docs/THINK-2026-10-27-ot90-shutdown-restart.md
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line` (in the THINK, the code and the `fixtures/acpi/` corpus). For a FAIL, say
what's wrong and what it should say.

1. **The corpus claims.**
   - None of the 10 sets is hardware-reduced.
   - `\_S5` is a static Name/Package exactly once per set, with the SLP_TYP values stated.
   - The OptiPlex, M75q and MSI reset through SMI port 0xB2.
   - `_PTS(5)` touches the SMI port, GPEs and NVS on every real machine, plus the EC and TPM on the Zenbook.
   Spot-check at least four sets.
2. **Power-off without in-kernel AML is sound.** Decoding the static `\_S5` package, then writing SLP_TYP|SLP_EN to
   PM1a/PM1b_CNT in the right order, works on every corpus set. What would break on a machine whose `\_S5` isn't
   static?
3. **Restart.** The reset order (FADT reset register, then the fallbacks) is right, and it handles the 0xB2 SMI
   machines.
4. **"Clean."** The order (userland, then the NeoFS flush and superblock, then the APs, DMA and IOMMU quiesced, then
   power-off) is right, has a bounded wait, and can't leave NeoFS half-written. The flush follows the same durability
   rules as the OT-87 THINK (flush errors, read-back).
5. **Who can ask.** The capability for shutdown and restart fits the capability model. An app without it gets EPERM.
6. **D1 and D2 (decisions for J.c.).** For each, say whether the recommended choice is safe:
   - D1: touch only disks the write guard armed, never the Zenbook's internal disk;
   - D2: ship the MVP without `_PTS(5)`.
7. **The ladder and tests.** QEMU power-off and restart scenarios; the bare-metal failure bank for the OptiPlex and
   the Zenbook; a kill-switch per policy rung; nothing that blocks sleep and wake later.
8. **Prior art.** The survey table (rust-osdev/acpi, Hermit, Redox, Theseus, Asterinas, SerenityOS, Linux, ACPICA)
   has licenses and last commit dates. Spot-check three. Nothing is vendored, and GPL is read for principle only.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot90-power-think-review-janus.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
