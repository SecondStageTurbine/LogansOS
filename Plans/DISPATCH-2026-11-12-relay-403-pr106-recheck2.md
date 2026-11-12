<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 403: Chad re-checks #106 (F0) for F2 (2026-11-12)

**MODEL:** GPT-6-Astra. **EFFORT:** medium.
Why: this is the third round of the lane's GPT-6.1-Sol review (relays 396 and 399 both failed). The CLAUDE.md table
routes a lane that has bounced twice to GPT-6-Astra, and a narrow re-check takes medium effort.

**Head:** `09414118`, on branch `claude/f0-neofs-dir-ops`. That is commit `8149e858` plus a merge of main `01eab8cc`.
**Your last RETURN:** `Plans/RETURN-2026-11-12-pr106-recheck-chad.md`. F1 passed in it; only F2 is open.
**The PM ruling (on #106):** after the bounded optimistic attempts fail, return `EAGAIN`. No device read may happen
under `entries`, on any path through F0's four operations.

Read `8149e858` at that head, from the repo. Answer each item pass or fail with the enforcing `file:line`, and give a
concrete input for any fail.

## F2

- [ ] `after_reads` no longer takes `entries` to run the read closure. After the bounded attempts it returns
  `NeoFsError::Contended`, which maps to `VfsError::Again` and then to `EAGAIN`.
- [ ] No path through `mkdir_at`, `unlink_at`, `rename_at` or `stat_at` reads the device while holding `entries`.
  Check every closure and every call made under the guard.
- [ ] An `EAGAIN` refusal has written nothing. Check that there is no partial cache edit, no commit, and no handle
  publish.
- [ ] Re-run your earlier schedule: 8 interleaved commits, a cold cache, a held read, and a third writer. It now
  either returns `EAGAIN` with no held-lock read, or completes.
- [ ] The discriminator forces the mismatches with interleaved commits. It asserts `EAGAIN`, and it asserts that the
  held-lock read counter stays 0. It also shows that a concurrent writer doesn't spin on `entries` while a device read
  is held.
- [ ] The CP-63 row in the PR body now states the real guarantee: optimistic reads outside the lock, only edits and
  cache writes under it, and `EAGAIN` past N attempts.
- [ ] `EAGAIN` for rows 26 to 29 is recorded in `docs/SDK-ABI.md`, and libneo's docs mention it.

## Still true

- [ ] F1's newest-first publish is unchanged.
- [ ] Rights are checked before any read.
- [ ] Each mutation makes one commit.
- [ ] The kill-switch still holds.

**RETURN:** `Plans/RETURN-2026-11-12-pr106-recheck2-chad.md` on `codex/pr106-recheck2-chad`, pushed.
