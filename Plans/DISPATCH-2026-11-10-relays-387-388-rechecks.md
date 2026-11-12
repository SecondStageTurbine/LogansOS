<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relays 387-388 (and re-runs 389, 391): narrow re-checks of #100 and #103 (2026-11-10)

**Written after the fact (2026-11-12).** `Plans/RELAY-QUEUE.md` pointed relays 387, 388, 389 and 391 at this file,
but it was never pushed to `main`. That is the likely reason Janus's relays 387 and 389 ran and pushed no RETURN:
the seat couldn't read its task. Dexter ran 391 from the queue line and the PM's PR #100 comment of 2026-11-10 01:02
UTC instead. This file records the task as it was meant.

Answer each item pass/fail with the enforcing `file:line`, and a concrete input for any fail. Read the code at the
stated head from the repo.

## Section 387: #100 (setup S4, `SYS_NET_IF_STATUS`) narrow re-check at `8414cc5f`

Re-check of `Plans/RETURN-2026-11-09-pr100-review-janus.md` (FAIL at `67be4ba2`).
- [ ] **F1, the torn record:** the snapshot is one consistent state, or is flagged when it can't be. Show the
  writer's line for every source and the reader's retry bound.
- [ ] **Item 7, CP-13:** the PR body's pitfall table names the real protocol and its discriminator tests, and its
  lock claims match the code.
- [ ] The five items that passed at `67be4ba2` still pass at `8414cc5f`.

Relay 387 and re-run 389 went to Janus; 391 rerouted it to Dexter. RETURN:
`Plans/RETURN-2026-11-12-pr100-recheck-dexter.md` on `dexter/pr100-recheck` (**PASS**, banked).

## Section 388: #103 (the OT-100 THINK) narrow re-check at `9a2c97dd`

Dexter. RETURN `Plans/RETURN-2026-11-10-pr103-recheck-dexter.md` (**PASS**, banked; #103 merged as `b3d18cf1`).
