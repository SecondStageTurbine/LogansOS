<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Jarvis — A2 (#111) code review

**Reviewed head:** `372d9a74888903729f1d9630ef58a76b5573d05a`
**Dispatch:** `Plans/DISPATCH-2026-11-12-relays-412-413.md`, Section 413

## 1. The chips

- **PASS — Looks follow the client table, repaint only on change, and reset at Home.** `userland/neo-init/src/desktop_session.rs:4047-4085` derives the looks from `apps` and repaints the Spine only when a value changes; `:6327-6332` resets them in `return_home`.
- **PASS — Terminal launches once and subsequent clicks switch.** `:3976-3981` maps the terminal chip to its module; `:4017-4020` calls `open`. `tools/xtask/src/main.rs:25184-25212` checks the launch/switch sequence and exactly one connection.
- **PASS — Files and Settings are unavailable and spawn nothing.** WM module mapping has one unavailable fallback at `userland/neo-init/src/desktop_session.rs:3976-3981`; Arthur's corresponding intent table is `userland/neo-arthur/src/launcher_logic.rs:10-21`. The chip and Home scenarios check no extra client connection at `tools/xtask/src/main.rs:25184-25212`.

## 2. The launch path

- **PASS — Launches use `apps::open` with a module id.** The common spawn/switch path is `userland/neo-init/src/desktop_session.rs:3744-3807`; Arthur's LaunchCap broker dispatches accepted app modules there at `:4562-4568`, and chip clicks call it at `:4017-4020`.
- **PASS — A failed spawn or handshake ends only the new client.** `:3769-3805` handles `connect` failure by logging and reaping only that spawned process, returning `STATUS_SPAWN_FAILED`.
- **PASS — Home terminal intent and Home chip click go through Arthur's LaunchCap.** Arthur parses `terminal` and calls `open_terminal(surface, launch_cap)` at `userland/neo-arthur/src/main.rs:1278-1307`; the Home chip records that intent at `userland/neo-init/src/desktop_session.rs:6826-6836` and Home returns it through its input loop at `:5941-5948`.

## 3. The lock gate

- **PASS — `lock::locking()` includes LOCKED and REQUESTED.** `userland/neo-init/src/desktop_session/lock.rs:174-185` tests both flags and sets REQUESTED when the SAK take contains Super+L.
- **FAIL — A queued reply can still be sent while locking or locked.** `userland/neo-init/src/desktop_session.rs:3845-3851` retries `apps.pending[slot]` with `sys_try_send` before the `lock::locking()` gate at `:3928-3960`. Concrete input: leave a client reply pending after `sys_try_send` returns `EAGAIN`, then take Super+L before the next `apps::turns`; if the client is now receiving, that turn sends the reply while `REQUESTED` is true. The existing `login_switch_lock` check only rejects a `DESKTOP apps front` line between the hold and unlock (`tools/xtask/src/login_scenarios.rs:1833-1860`), so it does not catch this send.
- **PASS — The switch/chip test hold is test-feature gated.** `userland/neo-init/src/desktop_session.rs:4222-4224` and `:7491-7527` guard the hold code with `lock-switch-test`; xtask enables that feature for the `login_switch_lock` scenario at `tools/xtask/src/main.rs:4183-4190`.

## 4. Client slots

- **PASS — Browser and terminal have distinct slots; the six-client limit bounds the table.** `userland/neo-init/src/desktop_session.rs:110,142-156` bounds `slots`; `:3623-3636` maps browser to slot 2 and terminal to slot 3. `Apps`' pending, dying, and generation arrays are each sized `MAX_CLIENTS - 1` at `:3528-3545`.
- **PASS — The old shared-slot `compile_error!` is removed, and these module mappings do not alias.** The independent mappings are at `:3623-3636`; both features can be enabled together in `userland/neo-init/Cargo.toml:94-103`.

## 5. Kill-switch `atrium-apps`

- **PASS — Image A's `.text` size and ISO size match main's.** PR #111's description records the comparison against `63b3e02a` in run `43299016997`: `.text` 1,562,501 B and ISO excluding kernel debug sections 122,990,266 B (ISO as built 152,715,264 B). The description also identifies the `paint_labels` change responsible for the prior 144 B leak and its fix.
- **PASS — The masked Home digest uses the DS-1 oracle.** `scripts/desktop/scene_oracle.py:909-936` generates the app-chip masked Home and field values; `fixtures/desktop-ds1/app_chips_digests.rs` stores them and the client-table scenario checks the digest.
- **PASS — `paint_labels` adds no app-chip boxes when `app-chips` is off.** Its no-feature early-return checks are separately compiled at `userland/neo-atrium/src/scene.rs:1078-1093`; `app_labels` and its chained checks are behind `app-chips` at `:1070-1075,1094-1109`.

## 6. The pointer

- **FAIL — A click outside every chip can still change focus.** `userland/neo-init/src/desktop_session.rs:7371-7375` treats any click in the Spine while Arthur is focused as `Escape` after chip hit-testing. Concrete input on a 1920×1080 layout: click `(5, 100)`. The Spine begins at `x=0` below the bar (`userland/neo-atrium/src/layout.rs:249-253`), while chips start at `x=CHIP_INSET`; `(5,100)` is outside every chip, yet it escapes focus.
- **PASS — App-chip hit-testing is limited to the rectangles supplied as shown.** `userland/neo-init/src/desktop_session.rs:7481-7488` only returns a chip for a containing `ctx.app_chips` rectangle; the Home-specific pointer context separately resolves app-chip rectangles at `:6826-6836`.

## 7. Prior art

- **PASS — The PR says the design is original and no survey is needed.** PR #111 description, “Prior art”: “The launcher and chip design is our own (the THINK §8 D2, the Spine's existing chips). Nothing is borrowed, so no survey is needed.” This meets the dispatch requirement at `Plans/DISPATCH-2026-11-12-seat-b-a2-launching.md:51-52`.

**Review result:** Fail on the lock gate's queued-reply send and the blank-Spine pointer click.
