<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 335: #14 check (Enter-only confirm; the non-Enter scenario)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §335,
running §317 and §324 of `Plans/DISPATCH-2026-11-08-relays-315-321.md` in one pass.

**Reviewed:** PR #14 head `ec3de81c6` ("Merge remote-tracking branch 'origin/main' into s14"), whose #14 commits are
`7e2c50878` ("only Enter confirms the power card") and `3d55c8563` ("a non-Enter key on the card goes back").
Merge-base with `origin/main` is `91f7062ef`; current `origin/main` is `93dee0672`.

**Overall: PASS.** Only Enter (or the drawn Go button) can reach `SYS_POWER`; every other key (and the drawn Back
button) goes back to Home with no power call, and the QEMU scenario now presses `a` on the first card, requires
`DESKTOP power back to=home`, and only then types the command again and confirms the second card with Enter. The old
code fails that step. No blocking finding. Two non-blocking notes: the module doc's key sentence is now understated,
and §324 item 2's parenthetical describes the old failure mode imprecisely (it was swallow-and-wait, not go-ahead).

## Checklist, in the dispatch's order

### §317 — the Enter-only confirm

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | Only Enter (or the drawn button) confirms | **PASS** | `userland/neo-init/src/power_view.rs:137`; `userland/neo-atrium/src/power.rs:83-85`; `userland/neo-init/src/power_view.rs:138-142,155` |
| 2 | Esc/Back and every other key leave the card without powering off | **PASS** | `userland/neo-init/src/power_view.rs:137,145-148`; `userland/neo-atrium/src/keys.rs:34-40`; `userland/neo-init/src/power_view.rs:63-74` |
| 3 | The scenario covers a non-Enter key | **PASS** | `tools/xtask/src/main.rs:1236-1239,11864-11899` |
| 4 | Nothing else changed since `73453e31` | **PASS** | diff identity, below |

### §324 — test commit `3d55c856`

| # | Dispatch item | Verdict | Enforcing refs |
|---|---|---|---|
| 1 | The steps press a non-Enter key on the confirm card and then check the menu went back with nothing shut down | **PASS** | `tools/xtask/src/main.rs:1236-1239,11864-11899` |
| 2 | The witness is one the old code would fail | **PASS**, wording note | `tools/xtask/src/main.rs:1237`; pre-fix `userland/neo-init/src/power_view.rs:137-139` at `73453e31` |
| 3 | Nothing else in `3d55c856` changes behaviour | **PASS** | `git show --stat 3d55c856` (one file, +21/−0); `cargo check -p xtask` green |

## §317 — only Enter confirms; every other key goes back

- **One decision point.** The card's key arm is a single expression:
  `Input::Key(key) => break power::confirm_key_goes_ahead(key)` (`userland/neo-init/src/power_view.rs:137`), and
  `confirm_key_goes_ahead` is `key == crate::keys::Key::Enter` (`userland/neo-atrium/src/power.rs:83-85`). There is
  no other key arm left in the loop.
- **The drawn button still works and is bounded.** `button()` returns `Some(true)` only for the first drawn label on
  the button row and `Some(false)` for the second; any other press is ignored and the card stays up
  (`userland/neo-init/src/power_view.rs:63-74,138-142`). So the only ways to `go` are Enter and the drawn Go button.
- **The power call is behind `go`.** `if !go { log("DESKTOP power back to=home"); return show(scene, Vec::new()); }`
  (`power_view.rs:145-148`); `libneo::sys_power(cmd.action())` is at `power_view.rs:155`, after that early return.
  The sole userland call site of `sys_power` is this line (`grep -rn sys_power userland`: definition
  `userland/libneo/src/lib.rs:1486`, call `userland/neo-init/src/power_view.rs:155`).
- **Esc/Back and every other key.** Escape decodes to `Key::Escape`, Backspace to `Key::Backspace`, Tab/arrows to
  their variants, and all printable keys to `Key::Char` (`userland/neo-atrium/src/keys.rs:34-40`). Every one except
  `Key::Enter` makes `confirm_key_goes_ahead` false, so the loop breaks false, logs `back to=home`, and repaints an
  empty card. Releases and non-key events are dropped by `decode` (`keys.rs:25-26`), so they never reach the arm; the
  card staying up for a key the Home view does not use cannot power off.
- **The scenario covers a non-Enter key.** `OT90_POWER_MENU_STEPS` now sends `a` after the first confirm is shown,
  waits for `DESKTOP power back to=home`, sends `shut down` again, then Enter on the second card
  (`tools/xtask/src/main.rs:1233-1240`, the `a` at :1236, the `back` wait at :1237). The ordered oracle requires
  confirm → back → confirm → request → shutdown → K1 → K4 → S5 (`:11864-11881`). The script runner clears its
  accumulated `seen` buffer after every command (`tools/xtask/src/main.rs:15327,15347`), so the repeated
  `DESKTOP home keystroke ascii=110` prompt in step 5 cannot match the first typing's output; it waits for the
  second typing.
- **Nothing else changed since `73453e31`.** `git diff 73453e31 ec3de81c` over `power_view.rs`, `power.rs` and
  `tests.rs` is exactly the `7e2c5087` diff; `git diff 7e2c5087 ec3de81c` over those three files is empty. The
  `tools/xtask/src/main.rs` diff over the same range is larger only because mainline merges brought unrelated
  scenarios in; `git diff 3d55c8563 ec3de81c -- tools/xtask/src/main.rs` is empty, so the #14 scenario block at the
  reviewed head is `3d55c856`'s.

**Note (not a fail):** the module doc still says "Enter or the first button goes ahead; Esc or Back returns to Home"
(`userland/neo-init/src/power_view.rs:7-9`). The function doc below it was updated (`:125-126`); the module sentence is
now narrower than the behaviour (every key goes back). Documentation only; no enforcing path.

## §324 — the non-Enter scenario and its witness

- **It presses a non-Enter key and checks the menu went back with nothing shut down.** The first card gets `a`
  (`tools/xtask/src/main.rs:1236`); the next step blocks until `DESKTOP power back to=home` (`:1237`); then the
  command is typed again and the second card is confirmed with Enter (`:1238-1239`). The ordered oracle is
  `:11864-11881`; the explicit check at `:11883-11892` requires both markers to exist and requires
  `power_off_requested` to come **after** `back to=home`, so a power-off before the card went back fails even if the
  ordered scan were fooled. `DESKTOP power stage=power_off_requested` is emitted immediately before the `sys_power`
  call (`userland/neo-init/src/power_view.rs:151-155`), so it is the right discriminator for "nothing shut down".
- **The old code fails the witness.** At the reviewed pre-fix head `73453e31`, the key arm was
  `Input::Key(_) => {}` (`userland/neo-init/src/power_view.rs:137-139`): `a` was swallowed and the card stayed up. The
  new step waits for `DESKTOP power back to=home`, which the old code never logs, so the script times out at step 4
  and the scenario fails. The witness is discriminating.
- **Wording note (not a fail):** §324 item 2 says "a key that went ahead before the fix". The old code did not go
  ahead on `a`; it left the card waiting. The witness still fails the old code, by the step timeout rather than by an
  observed power-off. The assertion at `:11883-11892` would also catch a version that did go ahead.
- **Nothing else in `3d55c856` changes behaviour.** `git show --stat 3d55c856` is one file, `tools/xtask/src/main.rs`,
  +21/−0. The hunks are the five-line `OT90_POWER_MENU_STEPS` change (`:1229-1240`), the five-line ordered-oracle
  change (`:11866-11880`) and the eleven-line back-before-request check (`:11883-11892`), all inside the
  `ot90_power_menu` path. `cargo check -p xtask` at the reviewed head finished green.

## Verification performed

- Fetched `refs/pull/14/head` and read the two #14 commits (`7e2c50878`, `3d55c8563`) plus the pre-fix `73453e31` and
  the prior review (`Plans/RETURN-2026-11-03-pr14-power-menu-review-jarvis.md`, at commit `8bea9633f`; it is not on
  `origin/main`).
- Read the enforcing code at `ec3de81c`: `userland/neo-init/src/power_view.rs`, `userland/neo-atrium/src/power.rs`,
  `userland/neo-atrium/src/keys.rs`, `userland/neo-atrium/src/tests.rs`, `userland/neo-init/src/desktop_session.rs`
  (the power route at `:5424-5434` clears the Home line before opening the card, so the second `shut down` types into
  an empty field) and `tools/xtask/src/main.rs` (steps, oracle, script runner, HMP key mapping).
- Ran the new host KAT at `ec3de81c`: `cargo test -p neo-atrium only_enter_confirms_a_power_action` →
  `test tests::power::only_enter_confirms_a_power_action ... ok` (1 passed). Ran `cargo check -p xtask` → green.
- Did not run QEMU; this is a narrow code/scenario check, not the gate. No source change.
- RAG health at `http://127.0.0.1:8404/health` was unavailable; used `grep`/`read` throughout. Not a blocker.

## Land note (not a checklist item)

This branch contains only this RETURN. The pre-existing untracked `pr59-review/` directory in the Dexter tree is not
staged and is not part of this branch. PR #14's merge-base is `91f7062ef`; `origin/main` has since moved to
`93dee0672` with code changes outside this path, so the PM's normal rebase/merge check still applies to the PR.
