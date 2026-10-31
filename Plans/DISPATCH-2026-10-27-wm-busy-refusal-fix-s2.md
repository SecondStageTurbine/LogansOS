<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: the WM must survive a refused client request (A9-D #155, Janus #96) → S2

**Why.** Janus FAILs #155, the busy emblem: `Plans/RETURN-2026-10-27-a9-busy-emblem-review-janus.md` item 1.
`desktop-busy` is in the product image. The PM checked it at `origin/main`:
`userland/neo-init/src/desktop_session.rs:1453–1457` returns 120 when `busy_on_channel` refuses. That return unwinds
`arthur_session` and `run()`, so neo-init exits and the whole desktop is gone.

One bad `OP_BUSY` is enough, for example a 640×400 surface with `x=639, width=2`, or a request on the wrong channel.
Any client with a surface channel can send one.

**Fix-forward under the escaped-defect contract. Push a gate-ready PR by 18:30 UTC (2:30 pm EDT) today.** #155 can't
simply be reverted, because the scrollback and wheel PRs build on `reserve_busy`.

S1 wrote #155 but has four fixes in flight. You know the WM from the freeze watchdog, so it comes to you.

## The fix (`claude/wm-refusal-survives`)

1. **A refused request ends that request, not the session.** For `OP_BUSY`, a validation failure:
   - sends the request's reply with an error status, so the client sees it refused;
   - leaves the scene unchanged (no busy state starts);
   - keeps the WM running.

   Check the other request arms in the same loop: `Configure` and anything else that does `return code` on a client
   input error. Split client errors (refuse and continue) from WM-internal failures (still fatal). List each arm and
   which side it falls on.
2. **A client whose channel closes** (Janus item 2: `try_recv_request` returns 24 and the whole session unwinds).
   Say whether that's intended for the MVP, where Arthur is the only client. If it isn't, make a client's close end
   that client and keep the WM. Either way, write the decision in the PR.
3. **Protocol.** If the refusal reply needs a status field the ABI doesn't have, add it with a hand-laid byte oracle
   (the byte-oracle rule). Otherwise say which existing field carries it.
4. **Discriminators (QEMU):** Janus's case (a valid `OP_BUSY`, then `x=639, width=2`) gets a refusal, and the WM then
   still presents frames and still answers a valid `OP_BUSY`. A wrong-channel `OP_BUSY` gets the same treatment.
   Show the scenario fails on today's code.
5. **Kill-switch:** `desktop-busy` off ⇒ the userland binary is unchanged for item 1. Item 2, if you change it, is
   ungated, so give the binary diff and justify it.
6. **Pitfall bank:** N/A for userland single-threaded WM code. State that in one line.

`scripts/worker-close-gate.sh`; send the PM the head SHA from `git ls-remote`. Review after merge: Janus narrow re-check
(GPT-6-Luna medium).

```
WORKTREE: cloud session S2 (your own clone)
BRANCH:   claude/wm-refusal-survives
          git fetch origin && git checkout -B claude/wm-refusal-survives origin/main

git show origin/main:Plans/DISPATCH-2026-10-27-wm-busy-refusal-fix-s2.md
git show origin/main:Plans/RETURN-2026-10-27-a9-busy-emblem-review-janus.md
```

RAG isn't available in the cloud session. Cite code by `file:line`. Before ending, `git status --porcelain` and
`git log @{u}..` are both empty.
