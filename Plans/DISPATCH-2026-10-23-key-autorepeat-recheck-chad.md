<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #45): narrow re-check of PR #106 item 11 → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your one failed item from relay #42
(RETURN `cb2971f3`: the scenario's timing oracle had one tick of slack).

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `d59bab14` (pinned). New commits on the
`b17cb2f2` you reviewed, plus a merge of `main` `87ca8c9a` (Plans only).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/key-autorepeat-recheck-chad
          git fetch origin && git checkout -B codex/key-autorepeat-recheck-chad d59bab14

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-key-autorepeat-recheck-chad.md   # this file
git show cb2971f3:Plans/RETURN-2026-10-23-key-autorepeat-review-chad.md         # your RETURN
git diff b17cb2f2 d59bab14 -- kernel tools Plans/NOTE-2026-10-23-key-autorepeat.md
```

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input.

1. **Item 11 closed.** In `run_ps2_key_repeat` (`kernel/src/test_harness.rs`), using the stamps from
   `drain_events_stamped` (`kernel/src/drivers/ps2/i8042.rs`), check that repeat k is rejected unless it comes at
   or after press + 500 ms + k/30 s, compared exactly with no slack. Check that your inputs fail: first repeat at
   490 ms, and repeats 30 ms apart.
2. **Honest claim.** The author does not assert a 33 ms gap between each adjacent pair, because on a 10 ms tick
   correct gaps are 30 or 40 ms. Check that the NOTE §6 item 11 wording and a KAT state exactly this bound and
   nothing stronger.
3. **Test-only.** Check that the new stamping hook is test-only (under a test feature), and that
   `git diff b17cb2f2 d59bab14 -- kernel` changes no production repeat logic.

A FAIL is reported only after you have re-checked it yourself.

## Close condition

- Commit `Plans/RETURN-2026-10-23-key-autorepeat-recheck-chad.md` on your branch and **push it**, with the verdict
  (CLOSED or STILL OPEN). Paste the pushed SHA to the PL.
- No build is needed. **Never write the Zenbook's disk.**
