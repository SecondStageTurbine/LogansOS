<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 293 — PR #15 bounded input responsiveness re-check — Chad

Reviewed head: `39d0d273d240223598ec4989d9895dd515b6e1a1`.
RETURN branch: `codex/pr15-recheck3-chad`, from `origin/main` at `b75c81ae8`.
Earlier: relay 279, `Plans/RETURN-2026-11-02-pr15-recheck2-chad.md`
on `origin/codex/pr15-recheck2-chad`.
Seat notes: latest [PM: Seat F](https://github.com/SecondStageTurbine/LogansOS/pull/15#issuecomment-6592932181),
and its [fix description](https://github.com/SecondStageTurbine/LogansOS/pull/15#issuecomment-6587076990).
All locations below refer to the reviewed head.

- **PASS — relay 279's remaining FAIL, bounded input responsiveness.**
  `userland/neo-init/src/desktop_session.rs:3144` calls `serve_input` between
  retries of Arthur's owed reply; `:3237` calls it in the permission lend's
  retry callback. Both retain other clients' `turns` and scheduler yield.
  The shared helper at `:3163` runs `lock::service` when login is enabled,
  then `pointer_tick` using the current front client's activator and the
  session's Home focus and escape flag. `pointer_tick` at `:474` respects
  the lock, polls and paints the pointer, and takes SAK actions (`:574`).
  Super+L reaches `lock::note_sak` at `:6719` and the following service pass;
  Super+Tab queues `scene.next_client` (`:6845`), consumed by the next
  `turns` at `:3623`, including during either retry loop. The original input
  (a live probe, Arthur takes the permission reply but refuses O, then pointer
  movement/click or Super+L during the five-second lend) now reaches the same
  input service as the ordinary session loop, before containment. The
  surviving-client loop also calls the helper (`:2450`). Reply expiry remains
  bounded at `:3112`; the lend checks its deadline and invokes the callback
  on NotReady at `userland/libneo/src/atrium_lend.rs:77`.

- **PASS — input discriminator and CP wording.**
  `userland/neo-arthur/src/showcase_surface.rs:676` sends Yield and never
  receives its reply. The HID script at `tools/xtask/src/main.rs:696` waits
  for that witness before injecting Super+Tab; the serial oracle at `:19061`
  requires `front client=0 via=sak-tab` before `client hung client=0`, then
  Arthur-only containment, the probe's input, release of six handles, exit,
  and successful return to Home. This distinguishes input service during
  settle from input taken only after expiry. The lend's callback is
  source-traced; the stall KATs at
  `tools/atrium-logic-tests/src/lend_kats.rs:96` and `:112` check retries and
  timeout at every stage, rather than integrated pointer/lock behavior.
  `docs/THINK-2026-10-29-ot88-app-model.md:318` now states the A1a wait
  qualifications requested in relay 279.

- **New — no additional finding in this narrow re-check.**

- **PASS — local validation.** `cargo test -p libneo -p atrium-logic-tests`
  passed. From the neo-init package, `cargo check -Zjson-target-spec --bin
  neo-init --features atrium-apps,desktop-home-boot-test,arthur-actions`
  passed, as did the same check with `,login` added. `git diff --check
  origin/main...pr15` passed. No PR code changed.

- **PASS — prior-head gate evidence, with limits.** GitHub's API confirms
  [run 41546440942](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41546440942)
  completed successfully at `ac7a83f745a67301a8acaacb43064a8c3dabf765`;
  the seat reports the yield-hang scenario passed there. The desktop session,
  lend helper, and Arthur's discriminator code are unchanged between that
  head and the reviewed head. QEMU is unavailable locally; no local runtime
  run or gate success at `39d0d273` is claimed here.
