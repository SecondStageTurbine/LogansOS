<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 392, Muse security read — PR #98, the terminal (`neo-terminal`)

> **Banked by the PM (2026-11-12).** Muse has read-only access, so J.c. relayed this text. **Head note:** Muse
> reviewed `bc45df45`, #98's current head. The dispatch named `76aa36d7`, the head before Seat B's rebase onto #100.
> The two differ only in the `DESKTOP_SCENARIOS` array length (PM range-diff, posted on #98), so this PASS covers
> both.

- **PR:** #98, L1a.3, `neo-terminal`: the terminal window running `neo-shell`
- **Reviewed commit:** `bc45df45` (as relayed; the dispatch on `main`, `Plans/DISPATCH-2026-11-12-relay-392-pr98-terminal-review.md`, names head `76aa36d7` — see Limitations)
- **Checklist:** the "Muse security read" section of that dispatch, read from `main`
- **Reviewer:** Muse (Gwen) — a different model from the writer (Claude, Seat B), so this review counts under the user's model-difference review rule
- **Overall verdict: PASS — all 7 checklist items pass. No FAIL.**

## 1. The lock path

### 1a. No key typed while the session is locked reaches the terminal or its child — **PASS**

- Locking takes the route for the WM first and only then serves input: `lock_input` sets `LOCKED`, engages the kernel lock route (`OP_ENGAGE`) where registered, activates the WM's own focus, and discards the WM queue — `userland/neo-init/src/desktop_session/lock.rs:303-317`.
- While locked, the only key consumer is the lock screen itself: `tick` receives on the WM's focus (`wm_focus()`) and feeds keys only to the password line (`line.edit`) or to `attempt` on Enter — nothing is forwarded anywhere — `lock.rs:368-413`. Keys typed during the backoff are dropped outright (`lock.rs:385-388`).
- The terminal client cannot receive keys while locked: its focus receive fails and it parks for its next turn — `userland/neo-worker/src/terminal_app.rs:449-451` (`EPERM => { in_front = false; break; }`). Its child's stdin is written only from focus events it actually receives (`terminal_app.rs:419-447`), so no received key means no child input.
- Whatever was typed while locked dies at unlock: the unlock `HANDOFF`'s discard step discards the WM focus queue ("Whatever was typed while locked reaches no one") — `lock.rs:502-506`.
- Belt and braces on the client side: when the terminal is given a turn (`K`), it discards up to 256 queued focus events (a focus queue holds at most 64) before answering `D` and serving any key — `terminal_app.rs:302-317`. Nothing queued from before or during the lock is replayed into the child.

### 1b. The resume step gives a client its turn back only after a successful unlock — **PASS**

- `RESUMED` is written in exactly one place: the end of `unlock()`, after the `HANDOFF` (swap route back, discard WM queue, present scene) has succeeded, and only when the route went back to a client (`!to_home`) — `lock.rs:518-523` (declaration `lock.rs:184-186`, `take_resumed` `lock.rs:190-196`).
- `unlock()` is reached only from `attempt()`, and only on the branch where `login::check_password` returned true (after the persisted attempt count was begun; a failed count write stays locked) — `lock.rs:415-435`.
- The consumer is gated the same way: `apps::turns` calls `resume_front` only under `#[cfg(all(feature = "atrium-terminal", feature = "login"))]` and only when `take_resumed()` returns true — `userland/neo-init/src/desktop_session.rs:3724-3727`. No successful unlock, no `RESUMED`, no resume turn.

### 1c. A client that doesn't answer its discard step is ended — **PASS**

- `resume_front` sends `K` and waits for `D` on a bounded link (`Link::within(DISCARD_NS)`); on success it logs `resumed` and returns — `desktop_session.rs:3980-3998`.
- On any failure of that exchange it logs `discard-failed` and calls `end(...)` — which closes the client's session channel and hands it to the reaping — then falls back to Arthur / the next live client — `desktop_session.rs:3999-4007`, `end` at `desktop_session.rs:4027`ff. This is the same treatment as a failed `switch` handoff (`desktop_session.rs:3941-3957`).

## 2. Locked input

### 2a. While the session is locked, no pointer or key event goes to any client — **PASS**

- Keys: the route is the WM's own focus for the whole locked period (item 1a); every client's focus receive fails the kernel's route/id check, and the terminal's handling of that failure is to park, not to serve — `lock.rs:303-317`, `terminal_app.rs:449-451`. Route changes the WM's code asks for while locked are deferred, not made: `home::swap_to` returns early via `lock::defer_route` — `desktop_session.rs:5958-5965`, `lock.rs:205-217`.
- Pointer: the pointer tick is the only path that applies pointer actions, and it is skipped while locked (item 2b). The lock's own `tick` also drains the pointer via `pointer_idle`, which moves nothing — `lock.rs:368-371`.
- Pixels follow the same rule, for completeness: `compose` and `commit_present_rects` return without presenting while locked unless the lock itself is painting — `desktop_session.rs:1034-1037` and `desktop_session.rs:1155-1159`.

### 2b. When `lock::service` returns `Ok(true)`, the pointer tick is skipped — **PASS**

- In the session loop for this PR's feature combination (`atrium-terminal` + `login`), the match is explicit: `Ok(true) => {}` (no tick), `Ok(false)` runs `pointer_tick`, `Err` leaves the loop — `desktop_session.rs:2093-2102`. (`lock::service` returns `Ok(locked())` — `lock.rs:260-280`.)
- The general guard also holds at the callee: `Scene::pointer_tick` itself calls `lock::service` first and returns at once when it reports locked — `desktop_session.rs:481-485` — which covers the other call sites (`serve_input`, `desktop_session.rs:3340-3341`).

## 3. The client protocol (`terminal_app.rs`)

### 3a. The child gets only the stdin read end, the stdout write end and the term page — no WM channel, no other handle — **PASS**

- `spawn_child` creates the two pipes and spawns with exactly three chosen stdio handles: `sys_spawn_stdio(CHILD, &[in_rd, out_wr, term], Some(0), Some(1), Some(2))` — stdin read end as 0, stdout write end as 1, term page as 2 — `terminal_app.rs:527-530`.
- The terminal then closes its own copies of the child's ends (`in_rd`, `out_wr`), keeping only the stdin write end and stdout read end — `terminal_app.rs:531-538`.
- The terminal's WM session channel (`MAIN`, `terminal_app.rs:62`) and its surface channel are never in the spawn list and are never granted to the child (the only `sys_grant` calls in the file give the buffer to the compositor over the surface channel and the activator to the WM over `MAIN` — `terminal_app.rs:215-218` and `terminal_app.rs:253-259`). No other handle is passed.

### 3b. The terminal writes no input of its own beyond the three answerbacks — **PASS**

- `terminal_app.rs` never constructs bytes for the child's stdin: the only sink writes go through `Session::key` (the user's own focus keys, through the line discipline), `Session::output` (child output; the only terminal-originated bytes it can emit are the parser's `answers`), and `Session::set_mode` (the child's own mode request from the term page) — `terminal_app.rs:372-382`, `terminal_app.rs:390-413`, `terminal_app.rs:419-447`; `userland/neo-terminal/src/session.rs:115-133`, `session.rs:147-180`, `session.rs:201-232` (the single `write_child` funnel).
- The answerback filter admits exactly three, and refuses everything else (any private marker or intermediate, more than one parameter, DECXCPR, secondary/tertiary DA, window reports, DECRQSS): DSR 5n → `CSI 0 n`, DSR 6n → `CSI row ; col R`, DA1 (`CSI c` / `CSI 0 c`) → `CSI ? 1 ; 2 c` — `userland/neo-term-core/src/answer.rs:29-51`, reached only via the `unhandled_csi` hook in `userland/neo-term-core/src/term.rs:34-68`.

## 4. Product builds

### `end_arthur` is compiled only under `lock-terminal-test`, never in the product — **PASS**

- Everything Arthur-ending in the lock module is gated on the one feature: the `ARTHUR` static (`lock.rs:556-557`), `note_arthur` (`lock.rs:559-562`), and `end_arthur` itself (`lock.rs:564-573`) each carry `#[cfg(feature = "lock-terminal-test")]`, as does its sole call site (`lock.rs:545-546`, inside the `lock-boot-test` block of `unlock`) and the `note_arthur` call site (`desktop_session.rs:1931-1932`).
- The feature is a leaf test feature — `lock-terminal-test = ["lock-boot-test", "atrium-terminal"]` — and neo-init's defaults are empty (`default = []`), so no product feature set includes it — `userland/neo-init/Cargo.toml:35`, `userland/neo-init/Cargo.toml:331`.

## Observation (not a FAIL)

In the session loop, `apps::turns` runs before the second `lock::service` of the iteration (`desktop_session.rs:2073-2102`). If a Super+Tab switch was left pending (`scene.next_client`) in the very turn the lock engages, `turns` → `switch` performs its `K`/`D` discard exchange with that client while locked. No checklist item is violated: the swap step defers (`desktop_session.rs:5962-5964`), so the route stays with the WM; the `K` is a session control byte, not a pointer or key event, and no input event can follow it (the client's focus receive fails `EPERM` and it re-parks, `terminal_app.rs:449-451`); and its present step is suppressed while locked (`desktop_session.rs:1034-1037`, `1155-1159`). Recorded because it is the one place a client is signalled at all during a lock; if the PM wants the stricter reading — no client signalling of any kind while locked — gating the `next_client` switch on `!lock::locked()` in `turns` would close it.

## Limitations

- **Commit discrepancy:** the dispatch on `main` names PR head `76aa36d7`; this read was performed at `bc45df45` as relayed. If `bc45df45` is not the head the PM intends to bank this against, the verdict should be re-run at that head.
- **Not re-read this relay:** the kernel's `sak-lock-route` implementation and `libneo`'s `atrium_handoff::HANDOFF` step ordering (swap → discard → present) and `sys_spawn_stdio`'s chosen-stdio ABI. The verdicts rest on the call-site code in the files listed below, which is where the checklist points; the `EPERM`-on-locked-receive behaviour is evidenced by the terminal's own handling of it (`terminal_app.rs:449-451`) and the lock module's design, not by a fresh read of the kernel route code.
- No file named by the Muse section was unread: `lock.rs`, `desktop_session.rs` (8,809 lines, read at the relevant sites) and `terminal_app.rs` were all fetched in full at `bc45df45`. `test_harness.rs` is not named by the Muse section and was not needed.
