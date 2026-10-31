<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #127–#130) → Janus: re-checks of your own FAILs on #166, #155, #151 and #160

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. This is a bundle, so one model throughout (CLAUDE.md bundle rule). #127 is
SMP-core, so Sol is the right model for it anyway; the other three are narrow re-checks.

All four merged after batch 13 (run `40799093977`, green at `0395982f`). Review each squash on `main`:
`git show <sha> --stat` and `git show <sha>`. One pushed RETURN per relay, each a plain checklist (pass or fail, the
enforcing `file:line`, a concrete input for any fail): `Plans/RETURN-2026-10-29-<slug>-recheck-janus.md`.

## #127: #183 `86ed5214`, the K1a-0 CAP_GRAPH fixes (your #109 F1–F3). Slug `k1a0-cap-graph`

Your RETURN: `Plans/RETURN-2026-10-27-k1a0-cap-graph-review-janus.md`. Product image, SMP core.
1. **F1.** Does any path still drop an `Arc<Process>` or `Arc<Pmap>` while `CAP_GRAPH` is held, or inside the
   caller's interrupts-off block after the unlock? Check:
   - the three scans (`retire`, `cap_is_live`, `revoke_handle`);
   - the spawn `ParentGone` return, the install-failure returns, and `terminate`.
   Is `Arc::into_inner` in `CapGraph::drop` the only way a pin is released? Is `DEFERRED_PROCESS_DROPS` drained on
   both the natural-reap and the kill paths?
2. **F2.** Does any path release a tensor count, or drop a removed `Handle`, under the lock? Is every `Detached`
   (from `remove`, `remove_cap_ids`, `drain`) dropped after its guard, including `revoke_handle`'s list?
3. **The witness.** Is `CAP_GRAPH_HOLDER` set after the lock is taken and cleared before it's released, so the
   witness can never read another CPU's hold as its own?
4. **F3.** Is `TicketMutexGuard` now `Sync` only when `T: Sync`, and still usable for `CAP_GRAPH`?
5. **The F1 leg runs on one CPU** (stated limit). Does the `Pmap::drop` witness still catch the cause, a shootdown
   under the lock, without the second CPU spinning?

## #128: #182 `bbba7381`, the WM refusal fix (your #96). Slug `wm-busy-refusal`

Your RETURN: `Plans/RETURN-2026-10-27-a9-busy-emblem-review-janus.md`.
1. A refused `OP_BUSY` is answered with its status in the reply (3 BadField, 7 CapRequired). The scene is unchanged
   and the WM goes on. Your 640×400 `x=639, width=2` case, and the wrong-provenance case.
2. Arthur treats a `Refused` reply as no emblem and keeps generating; it no longer exits 47 → 54.
3. The `arthur_session` arm table in the PR body. Is every client-error arm left fatal named there, as the follow-up
   lane says? Is any arm mislabelled WM-internal?
4. `desktop-busy` off: the neo-init and neo-arthur `.text` are identical (the PR's binary table).

## #129: #179 `de8547c1`, the A9-A pointer window fix (your #93 items 1, 2 and 5). Slug `a9-pointer-window`

Your RETURN: `Plans/RETURN-2026-10-27-a9-presents-liveness-review-janus.md`.
1. **Items 1–2.** Opening and closing are each a single store of the window word. Can a reader ever see `rx > in`?
   `rx_never_exceeds_in_at_any_store_boundary` should cover every placement.
2. A count packed with its window id (`id << 32 | count`). Can an increment that races the next open carry into
   the new window?
3. **Item 5.** The pitfall table (CP-2, CP-3, CP-21, CP-25) is present, and each N/A holds.
4. `prefill-perf` off: the product kernel `.text` is identical.

## #130: #172 `cf438ab8`, the R3 GPT fixes (your #91 item 7). Slug `ot87-r3-gpt`

Your RETURN: `Plans/RETURN-2026-10-27-ot87-r3-review-janus.md`. Muse re-checks its own item 5 on the same PR.
1. Your layout, `alternate = last_usable = L` with one LogansOS entry over `2048..L`: nothing arms
   (`janus_layout_arms_nothing`), with the backup wiped and with it left in place.
2. A copy validates only when its usable range is clear of both copies' metadata. Check `first_usable >= 2 + A`,
   `last_usable + A < alternate`, and a backup's alternate must be 1.
3. **The PM-accepted deviation.** The primary's alternate may sit below the device's last LBA (our ISO written to a
   larger disk). Does that leave any layout where a partition covers metadata and still arms?
4. `partition_claims` drops any entry touching `metadata_lbas`. Are all the spans there?

```
MODEL: GPT-6.1-Sol. EFFORT: high. (A bundle: one model throughout, per the bundle rule.)
WORKTREE: E:\AI Automation\workers\janus   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/review-bundle-2026-10-29-janus
          git fetch origin && git checkout -B codex/review-bundle-2026-10-29-janus origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-janus.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push all four RETURNs. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
