<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #37): narrow re-check of PR #100 F9 and F16 → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own two findings from relay #33
(RETURN `27171a6e`).

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `f7a176c2` (pinned).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-i2c-hid-touchpad-recheck-chad
          git fetch origin && git checkout -B codex/zenbook-i2c-hid-touchpad-recheck-chad f7a176c2

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-zenbook-i2c-hid-touchpad-recheck-chad.md   # this file
git show 27171a6e:Plans/RETURN-2026-10-23-zenbook-i2c-hid-touchpad-review-chad.md         # your RETURN
git diff bd3079c7 5caac15a -- kernel tools                                                # the fixes
```

`bd3079c7` is the gated head after `main` was merged in; `5caac15a` holds the F9/F16 fixes; `f7a176c2` adds a
merge of `main` `7377a9b3`, which changes no file under `kernel/`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input or sequence.

1. **F9 closed.** Check that `isr_with_driver_locks_held_for_test` now runs the ISR body at IF=0 while holding
   `ARENA`, `RESERVATIONS`, `DELIVERY`, `STATE`, `BOOT_LINES` and `BUS_OWNER`, and that the code comment lists
   exactly the locks held. Check the stated reason that `INFLIGHT`, `TABLE` and `RECENCY` are not compiled into
   this build.
2. **Test seams are test-only.** Check that the new `debug_with_arena_held` (`kernel/src/ai/arena.rs`) and
   `debug_with_reservations_held` (`kernel/src/ai/reservation.rs`), and the widened cfg on
   `debug_with_delivery_held` (`kernel/src/atrium/focus.rs`), exist only under test features, and that holding
   them in this order cannot deadlock the test itself (the ISR body takes none of them).
3. **F16 closed.** Check that `evidence_check` replays the boot lines on every outcome, the no-match case
   included, and that `i2c_hid_absent` asserts the replayed `no match` line.
4. **Nothing else moved.** Check that `git diff bd3079c7 f7a176c2 -- kernel` touches only `touchpad.rs`,
   `test_harness.rs`, `arena.rs`, `reservation.rs` and `focus.rs`, and only for F9/F16.

A FAIL is reported only after you have re-checked it yourself.

## Close condition

- Commit `Plans/RETURN-2026-10-23-zenbook-i2c-hid-touchpad-recheck-chad.md` on your branch and **push it**, with
  the verdict (CLOSED or STILL OPEN) and each item's result. Paste the pushed SHA to the PL.
- No build is needed; if you build, `cargo clean`. **Never write the Zenbook's disk.**
