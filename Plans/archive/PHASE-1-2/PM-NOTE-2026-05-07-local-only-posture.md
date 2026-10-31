# PM note: project is local-only; future dispatches drop the push step

**Date:** 2026-05-07
**Author:** PM
**Status:** Project-history record (informational). Codifies project-lead clarification of 2026-05-07 ~14:50 local.
**Substrate:** Cerebro's sub-commit 2 closure attempt surfaced an empty `git remote -v`. PM raised this to project lead with four resolution options. Project lead's call: "Local-only is the project posture; revise dispatch permanently."

---

## What the project lead clarified

The AltimitNeo repository operates local-only. There is no GitHub origin, no push step, no remote-tracking branches, no offsite copy maintained via `git push`. The git history that exists is the project history.

Indicators consistent with this posture in the existing project state:
- `git remote -v` is empty.
- `git for-each-ref` shows only `refs/heads/main` and `refs/stash`. No `refs/remotes/origin/*`.
- `git log` shows no merge commits, no `(origin/main)` decorations, no upstream-tracking metadata. Every commit since project inception has landed local-only.
- `git branch -vv` shows `main` with no upstream tracking annotation.

Indicators *inconsistent* with the posture (i.e., aspirational artifacts that no longer reflect operational reality):
- `.github/workflows/ci.yml` exists. CI is configured for a remote that doesn't exist. Result: CI does not run.
- `docs/phases/PHASE-1.5c.md` line 63 references a "GitHub Actions PR/push gate" running `cargo neo-test`. That claim is stale.

These inconsistencies are NOT to be cleaned up by PM. They're historical/aspirational artifacts in closed phase documents and config files; touching them is out of scope for any current dispatch. If the project ever does set up a remote, the inconsistency resolves itself; until then, it stays as-is.

## Implications for dispatches going forward

### Drop the push step from all closure dispatches

Closure dispatches under previous PM thinking included:
1. Pre-flight check.
2. Compose commit message.
3. Stage and commit.
4. **Push to origin.**
5. Cleanup (drop stashes after push confirmed).
6. Close report.

The corrected shape is:
1. Pre-flight check.
2. Compose commit message.
3. Stage and commit.
4. Cleanup (drop stashes after `git log --oneline -3` shows the new commit on `main`).
5. Close report.

Today's `Plans/DISPATCH-2026-05-07-sub-commit-2-close-cerebro.md` was revised in place to remove §4 (push) and renumber §5–§9 to §4–§8.

### Empty `git remote -v` is expected, not a blocker

Cerebro's STOP-and-surface on the empty `git remote -v` was the right call given the dispatch's "if push fails, STOP" language. Going forward, dispatches should explicitly state that an empty `git remote -v` is expected and not surface-worthy. Today's revised closure dispatch includes this clarification in §0 standing instruction 4:

> (Note: an empty `git remote -v` is expected and is NOT a blocker. The project operates local-only; do not surface remote-related findings as blockers.)

Future dispatches that touch git operations should adopt the same framing.

### Close reports drop the "Pushed to origin/main: yes" line

Today's revised dispatch's §5 close report shape removes the push line and adds "(Project is local-only; no push step.)" for clarity.

### Stash safety-net duration

Previous dispatches kept stashes as safety nets until "the push completes successfully." That semantics is now "the commit lands on `main` (verified via `git log --oneline -3`)." Stashes drop after the commit, not after a push that doesn't happen.

### Backup posture

Local-only means a hardware failure on the development machine loses the project. The project lead has either accepted this risk or has a separate backup mechanism (file-system snapshots, manual offsite copies, etc.) that PM is not party to. **PM does not raise the backup question further.** It's been raised; the answer is "local-only is the posture"; the conversation is closed.

### What this is not

- This is not a Bible amendment. The Bible doesn't speak to remotes or distribution mechanisms; it specifies build/run/test discipline and architectural posture. Remote configuration is operational, not constitutional.
- This is not a phase plan amendment. PHASE-1.5c.md's stale "GitHub Actions" reference stays as-is; phase plans are historical records of what was planned at the time, not living documents.
- This is not a CI removal. `.github/workflows/ci.yml` stays in tree. If it ever becomes operational again, fine; if not, it's a no-op.
- This is not project-policy. The project lead made a tactical call about today's situation; it's possible a future call goes the other way (e.g., "we're setting up a remote next week"). This note records today's state, not eternal posture.

## Action items

1. **Revise today's closure dispatch.** Done. `Plans/DISPATCH-2026-05-07-sub-commit-2-close-cerebro.md` was edited in place to remove the push step and clarify the empty-remote framing.
2. **Future closure dispatches use the corrected shape.** Going forward, PM omits push steps and remote-verification steps. Implementation dispatches (e.g., the forthcoming ADR-015 implementation, sub-commit 3a, sub-commit 3b, etc.) all follow the corrected pattern.
3. **No retroactive cleanup.** Closed dispatches with push references stay as they were; only currently-active or forthcoming dispatches get the new shape.
4. **No surfacing the backup question.** The project lead has the authority and the context to make that call; it's been made.

## Cross-references

- `Plans/DISPATCH-2026-05-07-sub-commit-2-close-cerebro.md` — the dispatch revised in response to this clarification.
- `docs/phases/PHASE-1.5c.md` line 63 — stale "GitHub Actions" reference; not for cleanup.
- `.github/workflows/ci.yml` — stale CI config; not for cleanup.

---

**End of note.**

*Project-history record. Not a normative document. Future dispatches reference this note to establish that local-only is the operational posture as of 2026-05-07.*
