<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat B, A2 — launching apps from the Spine and Home (2026-11-12)

**Seat:** Seat B (cloud), now that #98 (the terminal) has landed as `93f73bb7`.
**Branch:** `claude/a2-launching`, cut from `origin/main`.
**Design:** `docs/THINK-2026-10-29-ot88-app-model.md`:
- §7, the A2 row;
- §8, D2: Arthur, Terminal, Files and Settings go on the Spine, and Files takes the Lens's place.

**J.c.'s decision (2026-11-11), `Plans/DECISION-2026-11-11-one-window-first.md`:** apps open **one window at a time**
first. So A2 drops its A1c dependency: one app is front at a time, as in A1a's model. Side-by-side windows (A1b/A1c)
come after E2.
**Critical path:** A2 → F1 (Files) → E2 (the editor). F0 runs in parallel as #106.

## Build A2's row, one window at a time

- **Spine chips** for Arthur, Terminal, Files and Settings. Each chip has three looks: not running, running and in
  front, and running in the background.
  - Clicking the chip of an app that isn't running launches it.
  - Clicking the chip of an app that is running switches to it.
  - Files and Settings have no module yet; F1 and S1 add them. Until then their chips are present, and a click says
    "not available yet" without launching anything. Each chip's not-available state must be one switch that F1 or S1
    flips.
- **Home intents:** `terminal`, `files` and `settings` launch or switch the same way the chips do.
- **The launch path** uses `apps::open` with the module id, which #98 generalised. A failed spawn or handshake ends
  only the new client, and the reply says so.

## From the reviews (these must be in A2)

1. **Muse's #98 observation** (`Plans/RETURN-2026-11-12-pr98-security-muse.md`): gate the `next_client` switch in
   `apps::turns` on `!lock::locked()`. Then no client, not even one waiting on a pending Super+Tab, is sent any
   message while the screen is locked. Prove it with a scenario: press Super+Tab, lock in the same turn, and assert
   that the client receives no `K` until after unlock.
2. **Slot 2 is shared by the browser and the terminal.** #98 added a `compile_error!` so that no image builds both.
   A2 is the place to settle this. Do one of the following:
   - (a) give each app its own client slot, so one image can launch both;
   - (b) keep the exclusivity and state why in the PR. This is acceptable only if no MVP image needs both.

   The MVP needs the terminal. The browser is OT-61's separate lane. Choose, and say why.

## Gate

- **Scenarios** (4-shard full tier, `scripts/worker-close-gate.sh`):
  - each launch path: chip, Home intent and Arthur's `open …`;
  - a chip for an app that isn't running launches it, and a chip for a running app switches to it;
  - the three chip looks are checked by frame digest;
  - the Files and Settings chips say "not available" and spawn nothing;
  - the lock scenario from item 1.
- **Kill-switch `atrium-apps`:** with it off, the Spine and Home are byte-identical to today's, checked by frame
  digests and image size.
- **Prior art:** the launcher and chip patterns are our own design (the THINK). Say that the survey is not needed, or
  cite one if you borrow something.

## Review

This is the WM and neo-atrium, with no kernel changes. When the PR is green, the PM queues a cross-family review
(GPT-6-Luna, high). The lock-path change also gets a short Muse read.

## Close

Commit and push at every checkpoint. Post PM: on the PR with the run URL.

**Disk reclaim:** once the PM confirms the merge, run `cargo clean` in the worktree and remove it.
