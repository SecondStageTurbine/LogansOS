<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #100): post-merge review of A9-C, scrolling Arthur's reply with the keyboard (#159) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. It's contained to neo-text's `TextFlow` scrollback and Arthur's view and keys,
plus a test scenario. No SMP-core change and no new syscall. Claude-authored (S1), so it gets a Codex-family review.
**Do this after relays #95 and #98.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** one pushed RETURN. **Reviewed SHA:** `675495c9`, #159's squash on main (PR head `78515fe1` plus a
merge of main). Gate: batch 6, run `40457391685`, all jobs green.

**What it does.**
- `TextFlow` keeps a scrollback.
- Arthur's surface shows a scrolled view:
  - PgUp/PgDn always scroll it;
  - Up/Down scroll it when the input line is empty;
  - the view follows new text unless the user has scrolled up.
- Before reserving the busy emblem's rect (#155), Arthur calls `follow()`, so the emblem never lands on old text.
- Kill-switch: `arthur-scrollback`.
- J.c. asked for this after A9. The moon answer was cut off with no way to scroll.

## Setup

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/a9-arthur-scrollback-review-chad
          git fetch origin && git checkout -B codex/a9-arthur-scrollback-review-chad 675495c9

git show origin/main:Plans/DISPATCH-2026-10-27-a9-arthur-scrollback-review-chad.md   # this file
git diff 675495c9^ 675495c9
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='TextFlow scrollback view_top follow make_room'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For any FAIL, give a concrete input.

1. **The scrollback's bounds.**
   - `view_top` can't go below 0 or past the last full page.
   - The scrollback has a fixed cap, and dropping the oldest lines keeps `view_top` pointing at the same text, or
     clamps it.
   - No index underflows when the transcript is shorter than one page.
2. **Following.**
   - New text moves the view only when it's at the bottom.
   - PgDn back to the bottom resumes following.
   - A reply that arrives while the user is scrolled up doesn't jump the view.
3. **The keys.**
   - Up/Down scroll only when the input line is empty, and otherwise behave as before.
   - PgUp/PgDn don't reach the input line.
   - Arthur's ASCII decoder behaviour is unchanged for every other key.
4. **The busy emblem.**
   - `reserve_busy` calls `follow()` first, so the emblem's rect is below the latest text even if the user was
     scrolled up.
   - The DS-1 injected-Enter path that skips `follow()` can't put the emblem on old text.
   - Check `busy_rect_while_scrolled_up_follows_first`.
5. **Redraw.** A scroll repaints exactly the view, with no stale rows and no flicker of the input line. Commits use
   the existing damage path.
6. **Kill-switch.** With `arthur-scrollback` off, Arthur's transcript and keys are today's.
7. **The scenario.** `desktop_scroll_boot`'s arithmetic (PgUp 3, Up 2, PgDn 13, then follow) would fail if a key
   were dropped or the clamp were wrong.
8. **Prior art.** The PR body's survey table (Alacritty, WezTerm) follows CLAUDE.md's "Unfamiliar features" rule:
   licenses, last commit dates and SHAs, and nothing GPL copied. It also states which of the three prior-art rules
   apply.

## Close condition

- Commit `Plans/RETURN-2026-10-27-a9-arthur-scrollback-review-chad.md` on your branch and **push it**, with PASS/FAIL
  per item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
