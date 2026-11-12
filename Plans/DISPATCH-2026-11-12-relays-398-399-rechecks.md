<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 398-399: narrow re-checks of #104 and #106 (2026-11-12)

Read the code at the stated head, from the repo. Answer each item pass or fail with the enforcing `file:line`. For a
fail, give a concrete input. Each RETURN goes on the named branch and is pushed.

## Section 398: Janus re-checks #104 (OT-100 R2) for relay 395's two items

**MODEL:** GPT-6-Astra. **EFFORT:** medium.
Why this model: round 4 of a lane that has bounced several times, and a narrow re-check.

- **Head:** `b6ee1046`, branch `claude/ot100-r2-msc-write`.
- **The diff:** after a docs-only rebase, `git range-diff` shows your seven reviewed commits unchanged (`=`). The only
  new commit is `b6ee1046`.
- **Your last RETURN:** `Plans/RETURN-2026-11-12-pr104-recheck2-janus.md`.

**Item 1: a SYNC timeout must end the flush.**
- [ ] A deadline expiry in `sync_once` is a terminal result all the way through `flush`.
- [ ] Your input must now send exactly one SYNC and not return `Flushed`. The input is: fail at tick 3000, then a
  UNIT ATTENTION sense at 3001, then a SYNC that would succeed.
- [ ] A host test drives `flush` itself, not only `sync_once`.

**Item 2: ring admission must honour the deadline.**
- [ ] After `CONTROLLER` is acquired, the deadline is checked again before the command ring is touched or the
  doorbell rung. Check `submit_command_until` and EP0 admission.
- [ ] Either the lock wait is deadline-aware, or the bound is stated to exclude it.
- [ ] A test advances the clock during admission and asserts nothing is published.

**Still true:**
- [ ] The `None` path of every `_until` variant is today's.
- [ ] The read-only `command()` path is unchanged.

**RETURN:** `Plans/RETURN-2026-11-12-pr104-recheck3-janus.md`, on branch `codex/pr104-recheck3-janus`.

## Section 399: Chad re-checks #106 (F0) for F1 and F2

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium.
Why this model: a narrow re-check of the reviewer's own findings.

- **Head:** `e7a50407`, branch `claude/f0-neofs-dir-ops`.
- **The new commit:** `b6ee94b4`, "F0: read before the namespace lock, and publish reloads newest-first", followed by
  a merge of main.
- **Your RETURN:** `Plans/RETURN-2026-11-12-pr106-review-chad.md`.
- **PM ruling:** posted on #106 on 2026-11-12. F2 is scoped to F0's own four operations. The older paths are OT-101.
  D1 needs no change.

**F1: the handle snapshot.**
- [ ] A handle's published `(inode_block, record, generation)` is one consistent snapshot.
- [ ] Your two-`mkdir_at` schedule on one shared fd now ends with readdir giving either a complete current listing
  or `ESTALE`, never a successful old listing.
- [ ] Name the protocol ("newest-first") and the line that enforces it.
- [ ] A discriminator forces your pauses.

**F2: the lock across device reads.**
- [ ] None of F0's four operations holds `entries` across a device read. The reads happen first, then the operation
  takes the lock and re-validates the version.
- [ ] A re-validation failure retries or errors cleanly.
- [ ] The discriminator holds a device completion and shows that another writer doesn't spin on `entries`.
- [ ] The CP-63 row now describes the lock correctly.

**Still true:**
- [ ] One commit per mutation.
- [ ] Rights are checked before any read.
- [ ] The kill-switch holds.

**RETURN:** `Plans/RETURN-2026-11-12-pr106-recheck-chad.md`, on branch `codex/pr106-recheck-chad`.
