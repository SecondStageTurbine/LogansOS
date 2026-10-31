<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #112): cross-family review of K1a, the root-directory capability (#168) → Janus

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. It changes the capability model, with new derivations from a root cap,
widened spawn inheritance and two new syscalls, on top of K1a-0's `CAP_GRAPH` lock (#166). Claude-authored (S4), so it
needs a cross-family lens. Muse reviews security separately (relay #111). **Do this right after relay #109** (K1a-0),
which it builds on.

**Type:** review, post-merge (merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** one pushed RETURN. **Reviewed SHA:** `2922bf0c`, #168's squash on main (PR head `bfb8f37b` plus a merge of
main). Gate: batch 10, run `40500773798`, all jobs green.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/k1a-rootdircap-review-janus
          git fetch origin && git checkout -B codex/k1a-rootdircap-review-janus 2922bf0c

git show origin/main:Plans/DISPATCH-2026-10-27-k1a-rootdircap-review-janus.md   # this file
git diff 2922bf0c^ 2922bf0c
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='RootDirCap cap-rooted-open root form open spawn inheritance directory cap'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For any FAIL, give a concrete input or interleaving.

1. **Every new install goes through K1a-0's path.** Files opened through the cap, and caps derived for Arthur and the
   shell, are installed via `install_cap` with the cap as parent, under `CAP_GRAPH`. The edge gate
   (`cap-graph-logic-tests`) still passes and covers the new sites.
2. **Open vs revoke.** An open through the cap racing a revoke of that cap on another CPU either fails, or yields a file
   that the revoke then reaches. No file survives the revoke.
3. **Spawn inheritance of a directory cap.** The child is registered before it runs (K1a-0). The inherited cap is a
   child of the parent's cap. A spawn racing a revoke of the parent's cap refuses or starts without it.
4. **The root resolver.** Every caller of the root-form resolver checks the cap before resolving: open, readdir,
   mount, umount, `sys_model_load`. Grep for any caller that skips it.
5. **`SYS_HANDLE_INFO`.**
   - It reads the caller's table under the handles lock and is consistent with a concurrent close: the old or new
     occupant, never torn.
   - The byte layout matches its hand-laid oracle.
6. **The falsifiers.** Gate removed ⇒ FAIL 213. Opened file not parented ⇒ FAIL 214. Check that each would bite and that
   no leg can pass for the wrong reason.
7. **The pitfall bank.** The handle-table locking and cross-process revoke classes are stated, with discriminators.
   Nothing new is taken in IRQ context.
8. **Feature off** is byte-identical (`.text` and `.rodata`), and the 7 Arthur and shell scenarios pass with it on.

## Close condition

- Commit `Plans/RETURN-2026-10-27-k1a-rootdircap-review-janus.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
