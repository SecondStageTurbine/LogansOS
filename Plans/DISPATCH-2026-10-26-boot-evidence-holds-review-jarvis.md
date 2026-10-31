<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #78): post-merge review of image A without the bring-up evidence holds (#141) → Jarvis

**MODEL:** GPT-6-Luna. **EFFORT:** high. One subsystem (the Zenbook boot path's USB and touchpad evidence), no SMP
core, no new syscall. Claude-authored (S4), so it gets a Codex-family review. **Do this after relay #76.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `39f561ab` (main; the squash of PR #141, merged from the batch-1 gate `2da79dd3`,
run `40363142380`, green).

**Why it exists.** J.c. asked to drop the boot waits that only served bring-up evidence (photos of the boot
console): a 5 s wait for a USB keyboard or mouse to bind, a USB summary hold of up to 30 s, a 6 s input check, and the
touchpad's 6 s check and 30 s hold. They now sit behind a new feature, `boot-evidence-holds`, which image A doesn't
enable. In QEMU, boot-to-desktop went from 72.6 s to 7.4 s.

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/boot-evidence-holds-review-jarvis
          git fetch origin && git checkout -B codex/boot-evidence-holds-review-jarvis 39f561abd187fac31ad824c695fd7f059e9a9fe4

git show origin/main:Plans/DISPATCH-2026-10-26-boot-evidence-holds-review-jarvis.md   # this file
git diff c3aae11c 39f561ab -- kernel tools
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='boot-evidence-holds input_evidence_summary evidence_lines evidence_check hold'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete build (features) or boot sequence and the
wrong result.

1. **Kill-switch.** With `boot-evidence-holds` on, the boot calls exactly what it called before #141
   (`input_evidence_hold_for(500, 3000, 600)`, `evidence_check(600, 3000)`), in the same order, with the same arguments.
   Off, it calls `input_evidence_summary()` and `evidence_lines()` instead. Exactly one of each pair is compiled for
   every feature combination (no combination gets both or neither).
2. **Image A has it off.** `PRODUCT_IMAGE_KERNEL_FEATURES` doesn't list it, and nothing else turns it on for
   `iso --with-model` (bundles, feature implications, `NEO_TEST_EXTRA_FEATURES`). The xtask test
   `product_image_has_no_boot_evidence_holds` holds.
3. **No wait left.** `input_evidence_summary()` and `evidence_lines()` don't wait, poll, sleep or hold. The two neo-test
   checks bound them at 5 ticks; check the bound is real (it fails if the function waits).
4. **Input still works without the wait.** A USB keyboard or mouse that binds after the summary is still taken by the
   port service (`usb_hotplug`). A touchpad that isn't ready yet at the summary still becomes usable later. Say what
   the user sees on the Zenbook in each case.
5. **Queued events.** `evidence_lines()` drains the touchpad's queued events under `atrium-input-routing`, as the
   check path did, so the desktop doesn't start with stale events. Its lock use matches the existing hold path.
6. **The neo-test scenarios.** The USB and `i2c_hid_absent` scenarios still run the hold path where it's built, and
   now also check the no-hold path. No assertion was removed or weakened; the PASS lines only gained fields.
7. **Bring-up still possible.** A bring-up image can turn the feature back on (`--features boot-evidence-holds` or the
   bundle), and the comments say how.

## Close condition

- Commit `Plans/RETURN-2026-10-26-boot-evidence-holds-review-jarvis.md` on your branch and **push it**, with PASS/FAIL
  per item and an overall verdict. Paste the SHA to the PL.
- You may run `cargo test -p xtask product_image` and a QEMU scenario if you want. A FAIL is reported only after you've
  checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
