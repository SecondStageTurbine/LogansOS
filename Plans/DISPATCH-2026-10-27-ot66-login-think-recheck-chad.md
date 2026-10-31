<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #117): re-check of the OT-66 login / session THINK, revision 2 (#157 @ b555159e) → Chad

**MODEL:** GPT-5.6-Luna. **EFFORT:** medium. It's a docs-only re-check of your own five findings from relay #98, so
the cheapest model that can do it is the right one. If this goes into a bundle, run the bundle on GPT-6.1-Sol high
instead, per the bundle rule.

**Type:** narrow design re-check. **Deliverable:** one pushed RETURN, `Plans/RETURN-2026-10-27-ot66-login-think-recheck-chad.md`.
**Reviewed SHA:** `b555159e` (PR #157, branch `claude/ot66-login-think`).
- Your RETURN #98 reviewed `d49acd0f`. The SHA mapping is `d49acd0f → 9a0d525e` (rebase only) `→ 3c3939c5` (rev 2) `→ b555159e` (adds the K1 handle-rights rule to §3.4 and K1b's close gate).
- Diff: `git diff 9a0d525e b555159e -- docs/THINK-2026-10-27-ot66-login-session.md`.

## Checklist (pass/fail, plus `file:line` in the THINK and any code it cites)

1. **Item 2, Try.** §2 and §7 now say plainly that Try can read an installed disk's files. They say what K1b changes
   (Try grants no cap on the found volume). They say that only encryption at rest gives confidentiality, and that
   the installer and first boot tell the user so. Is that the complete and accurate statement?
2. **Item 3, model load.** §0, §1 and §3.4 cite K1a's gate on `model_load_path` (`kernel/src/arch/x86_64/syscall.rs`
   ~2619–2626 at `origin/main`). Check that the cited code does what the THINK says.

   Note that K1a has an open review FAIL of its own (Janus #112, `Plans/RETURN-2026-10-27-k1a-rootdircap-review-janus.md`
   F1: readdir, mount and umount check the backing `OpenFile`'s rights, not the handle's). Check whether the THINK's
   K1 scope now says that every fd-based directory operation is checked against the **handle's** rights. If it
   doesn't, that's a FAIL of this item.
3. **Item 4, NEOCRED1.** Two files, one slot each. A change writes only the older file and never opens the newer one
   for writing. Check the claim that NeoFS's copy-on-write regular write (`kernel/src/fs/neofs/inode.rs` ~371–427,
   then the fsync role order) means a cut damages only the file being written. Check that §4.3's L1 test (a cut
   after every write and flush; exactly one of the old or new password accepted; the untouched file byte-identical)
   would catch a design that breaks the guarantee. Check that the byte oracle still matches the new layout.
4. **Item 6, the delay counter.** It lives in neo-init, and neo-init can't restart: when it exits, the kernel drops
   to kernel-only mode with no respawn (`kernel/src/main.rs` ~463–480). Check that claim, that the counter survives
   a WM or login-screen restart, and that it still resets at boot.
5. **Item 5, lock.** Locked is an explicit exclusive compositor state:
   - no client pixels and no busy emblem (`userland/neo-init/src/desktop_session.rs` ~900–907);
   - OP_BUSY is deferred until after unlock;
   - a client spawned while locked gets no focus, input or pixels;
   - L2's gate adds a frame-digest check.

   Is the gate check strong enough to fail if a late client draws?
6. **§8, the chain.** K1a-0 → K1a → K1b, with L3 depending on K1b, and the MVP set is L1, L2, K1b, K2, L3. Note that
   #166 and #168 are both under post-merge fix-forward (`Plans/LEDGER-2026-07-29-logansos-release-ledger.md`, Janus
   RETURNs). Check whether the THINK's dependency claims still hold if either is reverted. One sentence is enough.

Plain checklist: for each item, what you checked, pass or fail, the enforcing `file:line`, and a concrete input for
any fail.

```
WORKTREE: E:\AI Automation\workers\chad   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot66-login-think-recheck-chad
          git fetch origin && git checkout -B codex/ot66-login-think-recheck-chad b555159e

git show origin/main:Plans/DISPATCH-2026-10-27-ot66-login-think-recheck-chad.md
git show origin/main:Plans/RETURN-2026-10-27-ot66-login-think-review-chad.md
git show origin/main:Plans/RETURN-2026-10-27-k1a-rootdircap-review-janus.md
```

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?" questions; grep
still wins for exact symbols. Health check first: `Invoke-RestMethod http://127.0.0.1:8404/health` should return
`{"status":"healthy",...}`. If it's down, note that in your RETURN and continue with direct file reads. The PL
manages the RAG, so it's never a blocker. Query example:
`$body=@{query='NEOCRED1 credential slot write'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

Commit and push the RETURN to your branch. Docs only, so no gate. Disk reclaim: none, since this is a review branch.
Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
