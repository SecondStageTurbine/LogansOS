<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #145) → Janus: the OT-67 editor THINK (#192)

**MODEL:** GPT-6-Luna. **EFFORT:** high. A design doc, docs only. It's a cross-family read of a Claude-authored THINK
before it merges.

The THINK is `docs/THINK-2026-10-29-ot67-editor.md`, merged to `main` (PR #192), including W1 (§5.2). It
sits on the OT-88 app model (`docs/THINK-2026-10-29-ot88-app-model.md`, PR #191, which Chad reviews separately as relay
#144). Check every claim it makes about today's code against the code itself.

One pushed RETURN, a plain checklist (pass or fail, the enforcing `file:line`, a concrete input for any fail):
`Plans/RETURN-2026-10-29-ot67-think-review-janus.md`.

1. **The text engine.** Is it right that neo-text T0 is enough for a plain-text, left-to-right editor, and that T2
   gates only localization? Is the `TextLayer` trait enough to swap T2 in later without touching the buffer?
2. **The buffer.** A `no_std` gap buffer with a line index, a linear undo list bounded at 1 MiB, cursor stops from
   `unicode-segmentation`, and a 4 MiB file cap. Are the bounds enforced, or only stated? What happens to undo at the
   bound?
3. **The text test.** A file counts as text if it's UTF-8 with no NUL, the same test Files uses. Does a file that isn't
   UTF-8 open read-only, or is it refused?
4. **Save on NeoFS today.** Is it right that a save is two commits today (truncate, then write) and so isn't atomic?
   Check `kernel/src/fs/neofs/inode.rs` on main after #180 and #187.
5. **Rung W1, `SYS_FILE_REPLACE(fd, ptr, len)`: one commit through NFS-W1's indirect writer.**
   - Does it make a save atomic against a power cut: the old content or the new, never a mix? Use #180's barrier and
     deferred reuse.
   - Does it need a byte oracle, and a pitfall-bank pass for the syscall path?
   - Does it replace #191 §6.1's temp-file-plus-`rename_at` plan cleanly?
6. **The rungs.** E1 (`neo-edit-core`, host KATs only) and W1 can start now, and neither depends on #191's D1–D5. Is
   that true? C1 (the WM clipboard) needs A1. Does E2 need anything else?
7. **Prior art.** Every row gives its licence, date and SHA. GPL rows (zed, amp) are principle only. Is the use of
   helix-core (MPL-2.0) for the undo design stated as design or as code?
8. **Decisions D1–D5** (plain text, Inter, new files from Files, the 4 MiB cap, no autosave). Is each a real product
   call? Is anything missing that J.c. should decide?

```
MODEL: GPT-6-Luna. EFFORT: high. (A design doc: a cross-family read.)
WORKTREE: E:\AI Automation\workers\janus   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot67-think-review-janus
          git fetch origin && git checkout -B codex/ot67-think-review-janus origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-ot67-think-review-janus.md
git show origin/main:docs/THINK-2026-10-29-ot67-editor.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push the RETURN. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
