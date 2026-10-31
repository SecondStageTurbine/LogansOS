<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: re-check of OT-22 rung 1 after your F1 bounce → Jarvis (Codex)

**Type:** review only, short. **Deliverable:** a pushed RETURN file.

**Why you.** You bounced rung 1 at `290e7f50` on F1 (a handle changes what it names without firing the
registered key). This checks the fix against your own repro, and checks that the `origin/main` merge
(DM-1 #61, consolidation #63) changed nothing in the wait path.

## What is under review

Branch `claude/ot22-multiplex` (#58); the PM names the head SHA in the relay. Read:

```
git fetch origin
git show origin/claude/ot22-multiplex:Plans/DISPATCH-2026-10-19-ot22-rung1-recheck-jarvis.md
git diff 290e7f508804c3967b10921a862ee7dea0c15292 origin/claude/ot22-multiplex -- kernel/src/sched/wait_any.rs kernel/src/arch/x86_64/syscall.rs kernel/src/sched/process.rs kernel/src/test_harness.rs kernel/Cargo.toml
git show origin/claude/ot22-multiplex:docs/THINK-2026-10-19-ot22-readiness-multiplex.md   # §8: your design F1-F5 mapped to the build
```

The fix:

1. **Identity snapshot.** `wait_any_scan` records each channel item's key and handle `CapId` in a
   `WaitIdentity`. `wait_any_items` compares the second scan's identity, which runs after `register`, with
   the first scan's. On a difference it unregisters and starts over: it re-watches, or returns EBADF.
2. **Removal hook.** `HandleTable::notify_removed`, called at `remove`, `remove_cap_ids` and `drain` (the
   same chokepoints as the tensor release), fires the removed channel handle's key with
   `wait_any::fire_handle_gone` (`WHY_HANDLE`).
3. **New legs and controls in `wait_any_core`:**
   - **Rebind leg:** a sibling thread of the waiter's own process closes the handle and reuses the slot
     inside the scan-to-register window, using a test hook. A send on the new channel must wake the
     waiter. Control `wait-multiplex-no-identity-check` must FAIL with `wait_any_rebind_stranded`.
   - **Revoke leg:** an ancestor's `revoke_handle` takes the waiter's derived handle under an infinite,
     IPC-only wait. The wait must return EBADF, with `ipc_wakes=0`. Control `wait-multiplex-no-handle-fire`
     must FAIL with `wait_any_revoke_stranded`.
   - The existing `wait-multiplex-no-ipc-fire` control still FAILs the IPC leg.

## Refute it

1. **Is the two-sided argument complete?**
   - A change before the second scan's handle-table reads is seen by the comparison.
   - A change after them goes through a removal chokepoint, and the chain handle-table lock → `WAIT_TABLE`
     fire reaches the registration.
   - Find a handle mutation that bypasses `remove`, `remove_cap_ids` and `drain` and changes what a slot
     names. Check `install*`, `get_mut`, grant and spawn paths, and anything new from the merge.
2. **Lock order.** Firing (and waking) under a handle-table lock adds the edge handle table → `WAIT_TABLE`
   → scheduler. Does any path take a handle-table lock while holding `WAIT_TABLE`, the scheduler lock, or a
   lock held by the revoke walk (`derivations`)?
3. **Liveness.** Can the identity-retry `continue` spin unboundedly against a peer that keeps rebinding? Is
   that acceptable, given it is the caller's own process or an ancestor's revoke?
4. **Is the rebind leg vacuous?** Does the reinstall really land in the closed slot (the leg fails
   otherwise)? Does the control fail for the right reason?
5. Anything else, including the merge resolutions in `kernel/Cargo.toml` (`neo-test-desktop-idle` now
   builds on `neo-test-desktop-pointer`), `test_harness.rs` (`DESKTOP_PASS`) and xtask
   (`DESKTOP_SCENARIOS` gains the two idle scenarios).

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings" must
say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot22-rung1-recheck-jarvis   (git fetch origin && git checkout -B codex/ot22-rung1-recheck-jarvis origin/claude/ot22-multiplex)
```

Optional runs, at `NEO_SMP=4`:
- `cargo xtask neo-test wait_any_core` should PASS.
- The same with `NEO_TEST_EXTRA_FEATURES=wait-multiplex-no-identity-check`, `wait-multiplex-no-handle-fire`
  or `wait-multiplex-no-ipc-fire` should each FAIL with its own reason.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?" questions;
grep still wins for exact symbols. Health-check first with `Invoke-RestMethod http://127.0.0.1:8404/health`,
expecting `{"status":"healthy",...}`. If it is down or degraded, the PL manages it: note that in your RETURN
and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='handle table remove revoke drain chokepoint'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-ot22-rung1-recheck-jarvis.md` to `codex/ot22-rung1-recheck-jarvis` and push
it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
