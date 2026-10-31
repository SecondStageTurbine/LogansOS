<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #102): post-merge review of OT-87 R3, the GPT oracle, partition view and one-disk VM image (#160) → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** high. It's contained to the block layer and xtask image tooling, with no SMP-core
change and no new syscall. Muse covers the guard's security (relay #101). This review covers the byte oracle, the
partition view, the mount probe, the VM scenario and the ledger. Claude-authored (S3), so it gets a Codex-family review.
**Do this after relays #91, #93 and #96.**

**Type:** review only, post-merge. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `7c771c0b`, #160's squash on main
(PR head `2c342585` plus a merge of main). Gate: batch 6, run `40457391685`, all jobs green.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot87-r3-review-janus
          git fetch origin && git checkout -B codex/ot87-r3-review-janus 7c771c0b

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r3-review-janus.md          # this file
git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r3-partition-view-s3.md     # the dispatch
git show origin/main:docs/THINK-2026-10-27-ot87-installer.md                     # §4.2, §4.4
git diff 7c771c0b^ 7c771c0b
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='gpt_layout PartitionDevice boot-neofs partition probe'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For any FAIL, give a concrete input.

1. **The byte oracle is independent (I-12).**
   - `fixtures/gpt_oracle.py` lays out both 4 GiB fixtures (`S=512`, `S=4096`) from §4.4's table, with CRCs from
     `zlib.crc32`, and shares no code with `gpt_layout`.
   - The geometry numbers match §4.4 exactly: FirstUsable, LastUsable, the backup array LBA, the ESP and NeoFS ranges,
     and 916,992 blocks.
2. **Three agreeing readers.** `gpt_layout`'s output equals the fixture byte for byte, the guard's validator accepts
   it, and the `gpt` crate (MIT) reads the same partitions.
3. **`PartitionDevice`.** Offsets are exact at the first and last LBA, a crossing request is refused, and the
   arithmetic can't overflow. NeoFS mounts it with no change to NeoFS.
4. **The `boot-neofs` probe.**
   - Partitions are probed before whole devices.
   - INCOMPLETE partitions are skipped.
   - With `neofs-partition` off the probe is today's.
   - Try behaviour is unchanged in R3.
5. **`vm-image --one-disk` and `vm_one_disk_two_boot`.**
   - The disk boots under the pinned OVMF.
   - NeoFS mounts from partition 2 of the boot disk.
   - State persists across the reboot.
   - The scenario would fail if the mount fell back to a whole device or another disk.
6. **The kill-switch.** The three `.text` hash sets in the PR are consistent: main vs the parser move (differs, stated
   why); the parser move vs the branch with the feature off (equal).
7. **The ledger.** I-17 is accurate, and the consolidation of I-1/I-14/I-15/I-16 drops no guard or discriminator. The
   ledger is still one page.
8. **Prior art.** The PR body restates THINK §11's rows with licenses and last commit dates. Nothing is vendored, and
   the `gpt` crate is a host test dependency only.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot87-r3-review-janus.md` on your branch and **push it**, with PASS/FAIL per item and
  an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
