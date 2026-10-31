<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #96): post-merge review of A9-D, the turning emblem while Arthur thinks (#155) → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** high. A new protocol verb (`OP_BUSY`), a WM overlay and Arthur's busy path, plus a
test-only kernel feature. No SMP-core change and no new syscall. Claude-authored (S1), so it gets a Codex-family
review. **Do this after relays #91 and #93.**

**Type:** review only, post-merge. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `d3603830`, #155's squash on main
(PR head `bfad0a82` plus a merge of main carrying #154). Gate: batch 4, run `40446328993`, all jobs green.

**What it does.**
- **Frames.** `build.rs` renders 36 frames of the emblem's ring turning under its still star, 44×44.
- **Protocol.** A client sends `OP_BUSY` = 10 with a rect on its own channel. It's laid out like
  `OP_COMMIT_DAMAGE`, and it consumes no commit serial.
- **WM (`desktop-busy`).** It blends the emblem as one more alpha layer and presents the rect at 12 frames/s from its
  session loop. The client's next commit ends it.
- **Arthur (`arthur-busy-emblem`).** It reserves a 48×48 rect right of the caret instead of writing `Thinking...`.
- **Product and test.** Both features are on in `desktop-with-model`. `desktop_busy_boot` measured about 12
  presents/s over two 56 s generations.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/a9-busy-emblem-review-janus
          git fetch origin && git checkout -B codex/a9-busy-emblem-review-janus d3603830

git show origin/main:Plans/DISPATCH-2026-10-27-a9-busy-emblem-review-janus.md   # this file
git diff d3603830^ d3603830
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='OP_BUSY busy_tick compose emblem'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For any FAIL, give a concrete input.

1. **`OP_BUSY` parsing.** The WM refuses a rect that's out of the surface, zero-sized or overflowing (`x + w` wrapping),
   and one sent on another client's channel. A refusal can't crash the WM or leave it drawing.
2. **The emblem ends.** Arthur's next commit ends it, and so does a client exit or a channel close mid-busy. Say whether
   anything else (a Home toggle, a second `OP_BUSY`) can leave it on screen or leak the state.
3. **Clipping.** The emblem is clipped to the client's area, never drawn while Home shows, and the rect is repainted
   from the client's own pixels when it ends. Check the compose KATs actually test this against the independent
   source-over.
4. **The pairing guard.** An `OP_BUSY` sent to a WM built without `desktop-busy` ends the session. The xtask test
   (`product_image_pairs_the_busy_emblem`) makes the product image always carry both features. Say whether any other
   image can carry one without the other.
5. **Test-only code stays test-only.**
   - `arthur-boot-model-kat` and the `real_lm_generate` prompt swap in the `desktop_busy_boot` kernel are unreachable
     in the product and image-A builds.
   - Check the cfgs on `MODEL_05B_PATH` in `userland/neo-arthur/src/main.rs`. At merge they're beside #154's
     `NEOFS_*` consts.
6. **The pace check would catch a frozen WM.** A WM that presents once, or at a fraction of the clock, fails
   `desktop_busy_boot`. Check the threshold xtask applies (two emblems, each ≥ 1 s, ≥ half the frame clock).
7. **Kill-switch.** With both features off, Arthur writes and erases `Thinking...` as before, and the WM has no busy
   state.
8. **Prior art.** The PR body's survey table follows CLAUDE.md's "Unfamiliar features" rule: candidates with license,
   last commit date and SHA, and nothing GPL copied. The PM asked S1 to add it on 2026-10-27; FAIL if it's still
   missing.

## Close condition

- Commit `Plans/RETURN-2026-10-27-a9-busy-emblem-review-janus.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
