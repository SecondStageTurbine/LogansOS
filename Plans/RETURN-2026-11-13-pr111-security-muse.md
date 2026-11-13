<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Muse security read — PR #111 (A2, launching apps from the Spine and Home)

> **PM note (2026-11-13):** pasted into the PM chat by J.c. Muse has read-only access and can't push. The reviewed
> head, `372d9a74`, is still #111's head when this was filed. Overall **PASS**. The observation under item 3 (no
> `compile_error!` belt for `lock-switch-test`) is a non-blocking follow-up, not a merge condition.

- **PR:** #111, A2 — launching apps from the Spine and Home
- **Branch / reviewed commit:** `claude/a2-launching` @ `372d9a74`. This matches the head named in the relay, with no discrepancy. All files below were read at this commit, not from a summary.
- **Checklist:** the 3 items in the relay of 2026-11-12. This is the follow-up to the relay-392 / PR #98 observation: gate the `next_client` switch on the lock.
- **Reviewer:** Muse (Gwen), a different model from the writer (Claude), so this review counts under the model-difference rule.
- **Filed by the PM as:** `Plans/RETURN-2026-11-13-pr111-security-muse.md`
- **Overall verdict: PASS, all 3 items.** No FAIL. Two precision notes are recorded inside items 1 and 2, and one observation after item 3; none is inflated into a fail.

## Item 1 — `lock::locking()` covers the whole lock: true from the moment Super+L is taken until the unlock, including the same keyboard read that takes it — **PASS**

- **What `locking()` is.** `locking()` is `LOCKED || REQUESTED` (`userland/neo-init/src/desktop_session/lock.rs:174-176`). `REQUESTED` is the latch bit for "Super+L taken, lock not yet engaged". It was added for exactly this item, and its doc comment names the #98 case.
- **The take sets it, inside the same read.**
  - Super+L reaches the WM only through `pointer::take_sak` (`userland/neo-init/src/desktop_session.rs:7524`). It reads the kernel SAK latch (`sys_input_sak_take`, :7529-7532), and under `login` it calls `lock::note_sak(pending)` at :7536. That call comes *before* `take_sak` derives or returns any action from the same take's bits (:7537-7548).
  - `note_sak` sets `REQUESTED` when the take carries `SAK_LOCK` (`lock.rs:184-186`).
  - Every take path funnels through this one function:
    - the pointer tick's read (`desktop_session.rs:657-665`), where the returned action is applied only after `take_sak` returns;
    - `lock::service`'s no-pointer fallback (`lock.rs:276`);
    - `lock::guard_input` (`lock.rs:299`).
  - So by the time the read that took Super+L has finished, `locking()` is already true. That is before anything the read produced is applied, and before any `apps::turns` can run.
- **The combined-take case (the one `login_switch_lock` forces).**
  - One take can carry `SAK_TAB | SAK_LOCK` together, and `note_sak` runs first.
  - The take then returns `Action::NextClient`. Applying it only records `scene.next_client = Some(via)` (`desktop_session.rs:7667-7670`).
  - The switch itself is deferred to `turns`, which sees `locking()` true and drops it (item 2).
- **No false gap at engagement.**
  - `service()` (`lock.rs:268`) calls `due()`, which consumes `REQUESTED` (`REQUESTED.swap(false)`, `lock.rs:238` / `:253`). It then immediately calls `lock_input` (`lock.rs:281`), whose first statement sets `LOCKED` (`lock.rs:311-312`).
  - The two calls are consecutive in one function on the WM's single thread. No `turns`, keyboard read or client signalling can run between them, so `locking()` is never observably false between the take and the engagement.
  - From engagement on, `LOCKED` holds `locking()` true for the whole locked period: `service`'s locked branch never re-consults `due()`.
- **The clearing end.**
  - `LOCKED` and `REQUESTED` are cleared inside `unlock()` (`lock.rs:480-481`), which is reached only from `attempt()` after the password verifies.
  - Precision note (not a fail): the flags clear at the start of `unlock()`'s body, before its `HANDOFF` swap, discard and present steps complete. This leaves no gap:
    - `unlock()` runs synchronously inside the lock's own `tick`, so no `turns` or input read can interleave;
    - the one client signal an unlock causes (the resume turn) is only *stored* after the handoff completes (`RESUMED.store`, `lock.rs:531`);
    - a later `turns` acts on it, once the unlock is done and `locking()` is false.
  - So no path signals a client, or gives it input, while `locking()` should still be true.

## Item 2 — No client is signalled while the screen is locked: while `lock::locking()` is true, `apps::turns` sends no message and grants nothing to any client — a pending Super+Tab switch, and a Spine chip click (launch or switch) — **PASS**

Both named cases are gated inside `apps::turns` (`userland/neo-init/src/desktop_session.rs:3820`). That matters because the session loop runs `turns` *before* the `lock::service` that engages a just-taken lock: `desktop_session.rs:2099` runs in the poll loop ahead of the service at :2116. This is the exact ordering the #98 observation flagged.

- **A pending Super+Tab switch is dropped, never signalled.**
  - At `desktop_session.rs:3934-3943` (under `login`), if `lock::locking()` is true, `scene.next_client` is taken and dropped with a log line.
  - The ungated switch block that follows (:3944-3949) then finds `None`. So `switch` (`desktop_session.rs:4161`) never runs, and it is the only place the turn signal exists: the `K` send / `D` receive discard exchange at :4181-4187.
  - This covers both ways the pending switch can meet the lock:
    - taken in the same chord read as Super+L (`REQUESTED` true, not yet engaged);
    - left pending from before the lock engaged (`LOCKED` true).
  - The switch is dropped, not deferred: it doesn't fire after unlock.
- **A Spine chip click, launch or switch, is dropped, never signalled.**
  - `scene.chip_click` is taken at `desktop_session.rs:3952`, and `locked` is computed as `lock::locking()` (:3954-3957, under `login`). When it's true, the click is logged and dropped, and `chip_clicked` (:3992) is never called.
  - So neither of `chip_clicked`'s signalling paths can run while `locking()` is true:
    - the switch to Arthur (`chip_clicked`'s `switch` call, :4002-4011);
    - the app path `open` (`desktop_session.rs:3744`), which either switches to the running app (:3757-3765) or runs the spawn/connect handshake and then switches (:3771-3790).
  - `chip_click` is set only by `pointer::apply`'s `Action::Chip` arm (`desktop_session.rs:7671-7674`). So a click recorded before the lock is exactly what this gate drops.
- **Grants.** No grant is reachable from `turns` while `locking()` is true.
  - The grant-carrying work sits behind the two gates above: the connect handshake inside `open`, and the route swap inside `switch`.
  - `turns` contains no other `sys_grant`/`sys_try_grant` path. The apps module's one `sys_try_grant` is in `lend_arthur`'s lend machinery, which `turns` never calls.
- **The `K` at the head of `turns` is not a counterexample.**
  - `resume_front`'s `K`/`D` exchange (`desktop_session.rs:4237`, send at :4252) runs from `turns` :3827-3830 only when `lock::take_resumed()` returns true.
  - `RESUMED` is set in exactly one place: the end of `unlock()`, after the handoff, with `LOCKED` already cleared (`lock.rs:531`). So when that `K` is sent, the unlock has completed and `locking()` is false.
  - It is the post-unlock resume graded in #98 (items 1b/1c), not a signal under the lock.
- **Scope precision, stated so the PM banks it knowingly.**
  - `turns`' per-slot servicing loop (`desktop_session.rs:3831-3927`) is *not* gated by `locking()`. While locked, it still services clients' `Commit`/`CommitDamage` requests and retries owed replies (`sys_try_send` in the pending-reply retry and in `reply`).
  - That is the lock's documented design. `lock.rs`'s module header says: "Everything under the lock goes on as usual: clients' commits are accepted into the WM's copy… none of it is composited until unlock".
  - It is neither of this item's two named cases: no switch, no chip launch or switch, no `K`/`D` turn exchange, and no grant.
  - If the PM meant the item's first sentence to also stop commit replies under the lock, that is a different and larger change than the #98 observation asked for. On the item as scoped by its own second sentence, it passes.

## Item 3 — The test-only hold can't ship: the `lock-switch-test` SAK hold (`hold_sak` / `take_sak`) can't be compiled into a product image — **PASS**

- **Every piece of the hold is under `#[cfg(feature = "lock-switch-test")]`.** All of them are in `userland/neo-init/src/desktop_session.rs`:
  - the `SAK_HOLD_UNTIL` static (:7494-7495) and `SAK_HOLD_NS` (:7496-7497);
  - `hold_sak()` (:7500-7514) and `sak_held()` (:7516-7520);
  - the hold check at the head of `take_sak`, which returns `None` without taking the latch (:7525-7528);
  - the sole `hold_sak()` call site, in `switch` on a `via == b"launch"` fronting (:4222-4225).

  With the feature off, `take_sak` (:7524) is the ordinary latch read, with no hold logic in it. Precision: `take_sak` itself is the normal, ungated latch read. What the feature gates is the *hold* inside it, which is the test-only part the item names.
- **The feature can't enter a product build by combination.**
  - `lock-switch-test = ["lock-terminal-test"]` (`userland/neo-init/Cargo.toml:340`). The chain it pulls is test-only end to end:
    - `lock-terminal-test = ["lock-boot-test", "atrium-terminal"]` (:336);
    - `lock-boot-test = ["login-boot-test"]` (:314).
  - neo-init's defaults are empty (`default = []`, :35).
  - `lock-switch-test` appears in no other feature's dependency list: the only occurrence in `Cargo.toml` is its own definition.
  - The product features (`product-integration = ["arthur-memory"]`, :54-55) pull in nothing of the chain.
  - The feature enters a build only by being named explicitly, which is the boot-test scenario's job (`login_switch_lock`).

## Observation (not a FAIL)

Item 3's guarantee rests on the feature graph alone. The block write guard (relay 393, item 4) also has a `compile_error!` belt that makes it unbuildable alongside a product feature. `lock-switch-test` has no such belt, so a future feature list could wire it in. The graph as it stands at `372d9a74` is clean, so the item passes. The note is only that this protection is one feature-list edit deep, where the guard's is two.

## Limitations

- **How it was read.** Read at `372d9a74` through the GitHub connector, serially; no 429 was hit. Files read in full:
  - `userland/neo-init/src/desktop_session/lock.rs`;
  - `userland/neo-init/src/desktop_session.rs` (9,163 lines, read at all relevant sites);
  - `userland/neo-init/Cargo.toml`;
  - `tools/xtask/src/login_scenarios.rs`: the `login_switch_lock` script, read for the intended sequence. The verdicts grade the product code, not the scenario.
- **No negative claim rests on code search.** GitHub code search's index covers the default branch, not this commit, and it proved incomplete in earlier relays. The "only occurrence" and "sole call site" claims rest on the full files fetched at `372d9a74`. The hold lives entirely in `desktop_session.rs`'s pointer module and the one `switch` call site.
- **Not re-read.** The kernel's half of L2b (the latch and the in-kernel lock route) was not re-read in this relay. Item 1 grades the WM-side flag discipline the checklist names, and `pointer::take_sak` itself evidences the latch-take behaviour.
