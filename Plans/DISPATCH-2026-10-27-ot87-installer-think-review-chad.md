<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #86): cross-family read of the OT-87 installer THINK (PR #149) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. A design doc, but it sets the layout for installs and widens what the disk
write guard arms. It's Claude-authored (S3), so it gets a Codex-family read before rung 1 is dispatched.

**Type:** review only. No code. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `f1cec9c2` (PR #149, branch
`claude/ot87-installer-think`), `docs/THINK-2026-10-27-ot87-installer.md`, with code cited at main `0b39255b`.

**What J.c. asked for.** LogansOS installed on a real computer or a VM, not run off the USB. One USB with **Try
LogansOS** (writes nothing) and **Install LogansOS** (onto a disk the user picks), "just like an Ubuntu ISO". The
Zenbook's internal NVMe is never written.

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot87-installer-think-review-chad
          git fetch origin && git checkout -B codex/ot87-installer-think-review-chad f1cec9c2

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-installer-think-review-chad.md   # this file
git show f1cec9c2:docs/THINK-2026-10-27-ot87-installer.md
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='write_guard_logic NEOFS claim GPT partition arm boot-neofs mount'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line` (in the THINK and in the code it cites). For a FAIL, say what's wrong and
what the THINK should say instead.

1. **The claims about today's code are true.** Spot-check at least eight citations, including:
   - the hybrid ISO's GPT/ESP (`xtask`);
   - `boot-neofs` mounting the first valid NeoFS volume read-write (`kernel/src/fs/mod.rs`);
   - NeoFS being whole-device only (`fs/neofs/format.rs`, `inode.rs`);
   - the guard dropping claims that overlap a GPT partition (`drivers/block/write_guard_logic.rs`);
   - the Zenbook NVMe behind VMD being unregistered (`drivers/pci.rs`);
   - the model pins (`boot_model_logic.rs`).
2. **The Zenbook stays unwritten.** The four layers in §2 hold, and none depends on another. In particular, the new
   partition rule (§4) can't arm anything on a Windows GPT disk, and `windows_gpt_disk_never_arms` stays as a KAT.
3. **The §6.3 defect is real.** Today, booting the USB on a machine with an installed or OptiPlex-style NeoFS volume
   mounts it read-write and writes to it. Confirm or refute from the code.
4. **R1 needs no kernel change.** A boot disk holding image A plus a NeoFS data disk boots under OVMF, and persistence
   works across two boots with the code as it is. Name anything that would stop it (boot-neofs device order, the
   guard on a virtio disk, the ISO's GPT on a disk).
5. **The image-based installer is sound.** Writing a prebuilt GPT/ESP/NeoFS image block for block, then fixing the
   backup GPT and growing NeoFS:
   - is the backup-GPT move and resize described correctly for both 512 B and 4 KiB sectors?
   - does the install claim (block-device index plus enumerated identity, one shot) close the wrong-disk and
     changed-disk cases?
6. **The ladder.** Dependencies are right: R1 and R2 parallel, R3 independent, R4 and R5 after R2 and R3. Every policy
   rung has a kill-switch with off ≡ today. R6's failure bank covers the likely bare-metal failures.
7. **A/B fit.** Reserving `\deploy\A` and `\deploy\B` on the ESP from the first install matches
   `docs/THINK-2026-07-10-ot51-update-rollback-model.md`, and nothing in the layout blocks OT-51 slice 1.
8. **What's missing.** Anything an installer needs that the THINK doesn't cover. For example:
   - the user picking a disk with the touchpad and keyboard;
   - the install failing partway through;
   - the USB being removed mid-install;
   - a target disk smaller than the image.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot87-installer-think-review-chad.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it against the code yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
