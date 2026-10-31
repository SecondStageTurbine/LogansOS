<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #116 and #125) → Jarvis: the R4.3 installer, and a re-check of the repetition guard

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. This is a bundle, so one model throughout.

Both merged after batch 12b (run `40787440872`, green). One pushed RETURN per relay, each a plain checklist (pass or
fail, `file:line`, a concrete input for any fail).

## #116: #170 `d72d4470`, OT-87 R4.3, the installer's writer and state machine. RETURN `Plans/RETURN-2026-10-29-ot87-r4.3-review-jarvis.md`

The design is `docs/THINK-2026-10-27-ot87-installer.md` §4.4–§4.8.
1. **The W0–W6 state machine.**
   - Does the write-back-cache disk-model KAT cut at every write and every flush, with any subset of unflushed writes
     persisting?
   - Does every cut leave either "nothing" or a disk that Finish installing completes?
   - Are the named traces there: relay #88's header-only case, the W6 backup flush failure, the restarted Finish?
2. **The GPT writer** reuses R3's `gpt_layout` and passes the R3 validator. That includes #172's metadata-span rule
   once #172 merges, so note it if R4.3's layout would trip it.
3. **W5, the NeoFS model write.** It goes through the install claim (`ClaimedWriter`), never the plain guard path.
   NFS-W1's large write (`create_file_from_bytes`) is used with the pin re-check.
4. **virtio-blk `VIRTIO_BLK_F_BLK_SIZE`.**
   - The 4096-byte logical sector is negotiated per virtio 1.2 §5.2.4 (confirm from the raw spec).
   - With `virtio-blk-size` off, the geometry is exactly today's 512.
5. **Refusals.** The 1 GiB disk, the one-sector-short disk, the no-flush disk, a foreign GPT and a completed install
   are all refused with zero writes (xtask hashes the disks). Is any row missing from THINK §4.5?
6. **Kill-switch.** `installer` off gives the same kernel `.text` (S3 measured desktop, product and image A).

## #125: #177 `953a27d3`, the repetition-guard fix (your #97). RETURN `Plans/RETURN-2026-10-29-a9-repetition-recheck-jarvis.md`

1. **Item 1.** `penalize()` returns at `p == 1.0` before touching the logits. Do the signalling-NaN KATs
   (`0x7f800001`, `0xff800001`) keep their bits?
2. **Item 2, by contract.** The PM decided the code stays and the contract is reworded: the output is cut only after
   at least 16 tokens of exact periodic repetition, keeping one copy.
   - Is the new wording accurate to `loop_end`?
   - Do `[0..8]×3 → Some(8)` and `[0..8]×2 → None` hold?
   - Is the oracle unchanged?

```
WORKTREE: E:\AI Automation\workers\jarvis   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/review-bundle-2026-10-29-jarvis
          git fetch origin && git checkout -B codex/review-bundle-2026-10-29-jarvis origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-jarvis.md
git show origin/main:docs/THINK-2026-10-27-ot87-installer.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push both RETURNs. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
