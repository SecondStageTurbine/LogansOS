<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 394: Janus re-checks #104 (OT-100 R2) for F1-F3 (2026-11-12)

**Reviewer:** Janus. **MODEL:** GPT-6.1-Sol. **EFFORT:** high.
Why high, not the usual medium for a narrow re-check: the F1 fix adds deadline-bounded `_until` variants to the shared
xHCI wait and recovery functions. Every other USB caller goes through those functions, so the "unchanged when `None`"
claim needs a careful read.

**PR:** #104, branch `claude/ot100-r2-msc-write`. **New head:** `c7235a49`.
**Your previous RETURN:** `Plans/RETURN-2026-11-12-pr104-review-janus.md`, which found F1-F3 at `2cf42002`.
**What changed:**
- the branch was rebased onto main;
- `git range-diff` shows the five reviewed commits unchanged (`=`);
- the one new commit is `c7235a49`. Review that commit, at that head, from the repo.

Answer each item pass or fail with the enforcing `file:line`. For a fail, give a concrete input.

## F1: the time bounds

- [ ] Each write attempt (WRITE chunk, SYNC, and the probe) has one absolute tick deadline, `msc_logic::Deadline`.
  Every wait in that attempt stops at it:
  - the CBW, data and CSW waits;
  - the REQUEST SENSE after a failed WRITE or SYNC;
  - the probe's TEST UNIT READY;
  - stall and reset recovery.
- [ ] Your two schedules now time out under one deadline: 9+9+9 s for a WRITE, and 29+29 s for a SYNC. The host test
  drives them with a controlled clock (`policy_tests.rs`).
- [ ] The xHCI `_until` variants behave exactly as today's calls when the deadline is `None`, for every existing
  caller:
  - `recover_endpoint_until`
  - `clear_endpoint_halt_until`
  - `control_no_data_until`
  - `submit_command_until`
  - `wait_event_until`

  Show that the `None` path is identical, or name the difference.
- [ ] The retry policy has explicit bounds: `WRITE_MAX_ATTEMPTS` is 7 and `SYNC_MAX_ATTEMPTS` is 3.
- [ ] Once a deadline has passed, no recovery starts.
- [ ] The worst allowed sequence stays within `write_chunk_bound_ms`.
- [ ] The poll-count backstop is still there.
- [ ] The read-only `command()` path is unchanged.
- [ ] Moving the policy behind the `WritePort` trait (`write_chunk`, `sync_once`, `flush`, `write_probe`,
  `WriteLatches`) changes no behaviour the first review passed. In particular:
  - the allowlist still applies;
  - the guard is still in front of every write;
  - the bounce buffer is still used;
  - the fence still comes before the TRB.
- [ ] The REQUEST SENSE now sent "through the write allowlist" is in today's opcode set, with direction In.

## F2: DATA PROTECT on SYNC

- [ ] A failed SYNC with your sense bytes `70 00 07 00 00 00 00 0a 00 00 00 00 27 00 00 00 00 00` latches
  write-protect.
- [ ] After that, `write_protected()` is `Some(true)` and a later WRITE is refused.
- [ ] A later MODE SENSE never clears the latch.

## F3: the discriminators

- [ ] A retried UNIT ATTENTION prints its `sense=6/…` line.
- [ ] A passed deadline prints `deadline N ms passed at <stage>` with the chunk's block count. It is no longer
  collapsed into a generic transport error.
- [ ] The PR body's trap-bank table now matches the code.

**RETURN:** `Plans/RETURN-2026-11-12-pr104-recheck-janus.md` on `codex/pr104-recheck-janus`, pushed.
