<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #95): post-merge review of OT-87 R2, Arthur's model from the installed NeoFS volume (#154) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. It's contained to the kernel AI path (the model pin check, a new
`Inode::verified_content_hash` in NeoFS, and the preload) plus Arthur's source order. No SMP-core change and no new
syscall. Claude-authored (S3), so it gets a Codex-family review.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** one pushed RETURN. **Reviewed SHA:** `2f066957`, #154's squash on main (PR head `2d880115`). Gate:
batch 4, run `40446328993`, all jobs green.

**What it does.**
- **Kernel, `model-from-neofs`.**
  - A `/neofs/<file>` path whose name matches a pinned model or vocab is held to that pin, using NeoFS's own verified
    BLAKE3 content hash. A mismatch gives `EBADMSG`.
  - The model path re-checks the hash and generation after the read.
  - `boot_modules::preload` builds `/neofs/<pinned model>` for every pinned model that no boot module names, at IF=1
    on the inference stack.
- **Arthur, `arthur-model-from-neofs`.** Boot module first, then `/neofs/`, falling back only on `ENOENT`. The state
  line names the source, and there are disk-specific absent and damaged sentences.
- **Scope.** Neither feature is in image A. The kill-switch `.text` hashes are in the PR body.

## Setup

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot87-r2-model-from-neofs-review-chad
          git fetch origin && git checkout -B codex/ot87-r2-model-from-neofs-review-chad 2f066957

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r2-model-from-neofs-review-chad.md   # this file
git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r2-model-from-neofs-s3.md            # the dispatch
git show origin/main:docs/THINK-2026-10-27-ot87-installer.md                              # §5 and the R2 row in §8
git diff 2f066957^ 2f066957
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='verified_content_hash neofs pin model preload'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For any FAIL, give a concrete input.

1. **The pin can't be skipped by the path.**
   - Paths are matched in the VFS's normal form. Check that `/neofs//x`, `/neofs/./x`, `/neofs/a/../x` and a trailing
     slash all reach the pin check.
   - Unpinned names, nested paths and other mounts behave exactly as before.
2. **The hash compared is the pinned quantity.**
   - `verified_content_hash` returns the whole-file BLAKE3 that `boot_model_logic` pins, not a per-block or Merkle hash.
   - It's computed once per generation and cached.
   - A rewrite (new generation) invalidates the cache.
3. **The bytes built are the bytes that passed.**
   - After the read, the model path re-checks the hash and the generation.
   - A write between the check and the read can't yield a model built from unverified bytes.
   - The vocab is checked at open-for-read. Say whether a vocab rewrite after open can matter.
4. **Source order in the kernel and in Arthur.**
   - A boot module wins even when it's damaged; the disk is never consulted (`_module_wins`).
   - Arthur falls back to `/neofs/` only on `ENOENT`.
   - The `/boot/models` handle and the `/neofs/` handle are cached under their own paths, as the PR says.
5. **Locks and stacks (CP-74, CP-56, CP-82).**
   - The new `verified_content` lock is taken only inside `vfs_body_if_on` from a syscall, and on IF=1 threads at boot.
   - It's never held across the hash.
   - The preload runs in `inference_stack::run_checked`.
6. **Scenarios.** Each of the four `arthur_model_from_neofs*` scenarios would fail if its claim were false. In
   particular, would `_damaged` still pass if the pin check were skipped and the model were built anyway?
7. **Kill-switch.** With both features off, nothing new is compiled or reachable. The `.text` hashes in the PR body are
   consistent with the cfgs you see.
8. **Prior art.** The PR body's survey table follows CLAUDE.md's "Unfamiliar features" rule: candidates with license,
   last commit date and SHA, and nothing GPL copied.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot87-r2-model-from-neofs-review-chad.md` on your branch and **push it**, with
  PASS/FAIL per item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
