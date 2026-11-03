<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 296: Janus re-checks #60's restart hint

Same rules as `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`. The seat's notes are the latest `PM:` comment on
the PR.

## Section 296: Janus, #60 narrow re-check (your relay 291's one new FAIL: the restart hint after a DST change)
**MODEL** GPT-6.1-Sol, **EFFORT** medium: a narrow re-check of your own finding.

`git fetch origin +pull/60/head:pr60 && git diff origin/claude/tz-libneo...pr60` (head `17cca97a`, stacked on #59 at
`3e5b0df0`; review the head you fetch and name it).

1. Your input: confirm New York at 2026-11-01 05:00 UTC, then `timezone` at 06:30 UTC. Home shows 01:30 EST and the
   card does NOT ask to restart (`tz::needs_restart`, and its call in `desktop_session.rs`).
2. The seat re-reads the stored confirm-time RTC reading instead of comparing zone ids. Check that a zone or RTC mode
   that would read that reading differently still asks to restart (Chicago, `rtc utc`, Sydney), and that one with the
   same rules (Toronto) doesn't.
3. The new KATs in `tools/tz-logic-tests` pin these cases, and the confirm stays write-once.
4. One line on anything new.

RETURN: `Plans/RETURN-2026-11-03-pr60-recheck2-janus.md`, branch `codex/pr60-recheck2-janus`.
