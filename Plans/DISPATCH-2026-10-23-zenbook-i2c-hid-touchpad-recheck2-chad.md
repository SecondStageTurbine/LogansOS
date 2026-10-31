<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #40): re-check 2 of PR #100 F9 → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This narrowly re-checks your remaining open item from relay #37 (RETURN
`201b9a91`: F9 still open because `INFLIGHT`/`TABLE`/`RECENCY` were wrongly described as not compiled). F16 is
already closed.

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `425c140f` (pinned).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-i2c-hid-touchpad-recheck2-chad
          git fetch origin && git checkout -B codex/zenbook-i2c-hid-touchpad-recheck2-chad 425c140f

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-zenbook-i2c-hid-touchpad-recheck2-chad.md   # this file
git show 201b9a91:Plans/RETURN-2026-10-23-zenbook-i2c-hid-touchpad-recheck-chad.md          # your last RETURN
git diff f7a176c2 425c140f -- kernel                                                         # the fix
```

`dd16761c` holds the fix; `425c140f` adds a docs-only merge of `main` (`kernel/` is unchanged by it).

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input or sequence.

1. **F9 closed.** Check that the ISR body now runs at IF=0 while all nine are held: `ARENA`, `RESERVATIONS`,
   `INFLIGHT`, `TABLE`, `RECENCY`, `DELIVERY`, `STATE`, `BOOT_LINES`, `BUS_OWNER`. Check that the "not compiled"
   claim is gone, and that the comments, the PASS witness and the findings list name exactly those nine.
2. **Order and seam.** Check that the new `accounting::debug_with_accounting_held`
   (`kernel/src/drivers/accel/accounting.rs`) exists only under `neo-test-i2c-hid-absent`, and that it takes
   `INFLIGHT` → `TABLE` → `RECENCY` in an order the lockdep levels allow. Check that the nine-lock nesting has no
   inversion or self-reacquisition.
3. **Nothing else moved.** Check that `git diff f7a176c2 425c140f -- kernel` touches only `accounting.rs`,
   `touchpad.rs` and `test_harness.rs`, and only for F9.

A FAIL is reported only after you have re-checked it yourself.

## Close condition

- Commit `Plans/RETURN-2026-10-23-zenbook-i2c-hid-touchpad-recheck2-chad.md` on your branch and **push it**, with
  the verdict (CLOSED or STILL OPEN). Paste the pushed SHA to the PL.
- No build is needed. **Never write the Zenbook's disk.**
