<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 410 — PR #108, A3p re-check under ruling (A)

Reviewer: Janus.
Dispatch: `Plans/DISPATCH-2026-11-12-relays-410-411.md`, Section 410.
Reviewed head: `690eb95dfac378be2c8dafff87a673225c94ab8e`.
RETURN branch: `codex/pr108-recheck3-janus`, based on `origin/main`.
All source coordinates below refer to that reviewed head. No implementation changes.

**Verdict: FAIL — item 3's two added guards are not complete at this head.**
Items 2 and 4, commit 8's resume fix, and the relay-408 item-1 kernel
registration witness pass. Item 3's original two conditions under option
(A) pass; Guard 1 is missing, and Guard 2's specified unit-test fixture is
missing, although the oracle already rejects that input.

Applied the [PM's item-3 ruling](https://github.com/SecondStageTurbine/LogansOS/pull/108#issuecomment-6837972242)
and read [Seat I's preceding report](https://github.com/SecondStageTurbine/LogansOS/pull/108#issuecomment-6836832288).
This does not reinstate a requirement that gen 2's own request have
`parked=1`, nor request a `sys_send` scheduling hook.

## Item 3, as ruled: FAIL

1. **Gen 2's registered key/cap fires between its first key and idle: PASS.**
   `tools/xtask/src/main.rs:9729` gets gen 2's key/caps and `:9730` rejects
   old caps. `:9737` and `:9738` bound the request window; `:9739` searches
   it for the surface key, and `:9743` checks the latest registration's
   handle/cap. The fixture's cap is 44 (`:29983`). The oracle derives the
   cap from the registration rather than hard-coding its number.
2. **The WM's next wake names `c1g2f`: PASS.**
   `tools/xtask/src/main.rs:9752` selects that wake and `:9753` requires the
   surface bit before idle. This is a readiness scan, not a tick inferred
   from elapsed time: `kernel/src/arch/x86_64/syscall.rs:7205` scans,
   `:7209` returns the ready mask, and `:7263` loops to rescan after either
   kind of wake. `userland/neo-init/src/desktop_session.rs:2215` passes the
   actual return mask to `witness_wake`; `:3434` rejects nonpositive returns
   and `:3443` checks each ready bit before naming the client's surface.
3. **Guard 1, an Arthur `parked=1 why=1` channel fire in this boot: FAIL.**
   There is no enforcing check. `tools/xtask/src/main.rs:9606` through
   `:9759` never parses or requires `parked` or `why`; the scenario calls
   this oracle at `:22120` without adding the guard. **Concrete failing
   input:** the unchanged `CP61_LOG` at `:29968` contains three fires, all
   `parked=0 why=2`, and no Arthur fire. Nevertheless the upstream test
   `cp61_oracle_accepts_death_while_parked_and_reuse` at `:29991` requires
   `Ok(())`. I reproduced that acceptance and separately asserted that
   this input contains no `parked=1`. An otherwise valid scenario log
   with every Arthur fire removed likewise encounters no such gate.
   Add the required same-boot Arthur channel-fire check and a refusal test
   with that evidence removed; retain acceptance for gen 2's timer-first
   fire when the separate Arthur guard is satisfied.
4. **Guard 2, tick-only/no-gen-2-request fixture refused with a unit test:
   FAIL as delivered; refusal behavior PASS.**
   The existing key gate at `tools/xtask/src/main.rs:9739` rejects the
   no-request input. However, the full upstream cases at `:29996` through
   `:30035` do not contain the specified fixture. The closest case,
   `:30021`, substitutes a session-key fire while leaving `ready=c1g2f`
   intact; that is not the PM's negative control.

   **Concrete missing test input:** take `CP61_LOG`, delete the entire
   `WAITANY witness woke ... ticket=2598 ...` line (`:29985`), and replace
   `DESKTOP park woke ms=490 ready=c1g2f,` (`:29986`) with
   `DESKTOP park woke ms=490 ready=`. Keep the cap-44 registration and all
   earlier gen-1 events. There is now no cap-44 fire and no `c1g2f` in any
   ready set. The oracle returns `Err` containing `no kernel wake on gen
   2's surface key`, as independently tested. Commit this explicit
   negative-control fixture/assertion to satisfy the unit-test requirement.
   A pure tick without a channel fire emits no kernel `woke` witness
   (`kernel/src/sched/wait_any.rs:274`), hence deleting that line models
   the absence of the request's fire.

**Diagnostics retained: PASS.** `kernel/src/sched/wait_any.rs:280` prints
`parked`, `why`, `key`, `fire_tick`, `deadline` and `now`; `:342` records
the key, parked state and tick on the matching entry. No removal of those
fields is needed to implement (A).

## Item 2, WM-generation negative checks: PASS

- `tools/xtask/src/main.rs:9706` rejects any subsequent `DESKTOP park`
  set or wake naming `c1g1`, starting after the death's WM wake.
- `:9752` and `:9753` require the WM's wake after gen 2's request to name
  `c1g2f`. The kernel-cap negatives also remain at `:9712` and `:9730`.
- `cp61_oracle_refuses_each_wrong_ordering` (`:29996`) tests both specified
  refusals: `ready=c1g2f` to `ready=c1g1f` at `:30026`, and a gen-1 set
  after death at `:30028`; `:30033` asserts rejection. Both passed.
- An additional independent control replaced gen 2's readiness with an
  empty ready set, without introducing any `c1g1`. That also failed at
  the gen-2 readiness gate, confirming it is independently enforced.

## Item 4, CP-61 citation repair: PASS

Read the current [PR body](https://github.com/SecondStageTurbine/LogansOS/pull/108)
and checked each of the row's five coordinates at the pinned head:

| `userland/neo-init/src/desktop_session.rs` | What the cited line does |
|---|---|
| `:2106` | Calls `apps::turns` before building the next wait. |
| `:2170` | Allocates a fresh wait-item array. |
| `:2198` | Populates it from the current client table. |
| `:3351` | Defines `apps::wait_items`. |
| `:3354` | Iterates the current clients. |

All five point at the claimed operations. This citation pass does not
supply item 3's missing guards.

## Commit 8, unlock resume and latch audit: PASS by source inspection

- `userland/neo-init/src/desktop_session/lock.rs:567` publishes `RESUMED`
  with Release after restoring the client route. `:275` reads it with
  Acquire and `:276` returns deadline zero before the idle-off branch.
  `desktop_session.rs:458` folds that deadline into `Scene::park` and
  `:467` passes it to the wait. `syscall.rs:7253` refuses to park after
  the deadline; ready work may return even earlier. The next turn takes
  the latch at `desktop_session.rs:4033` and resumes the front client at
  `:4034`; `lock.rs:194` consumes it with an AcqRel swap. This removes the
  reported park-between-unlock-and-resume hang.
- The unarmed early return at `lock.rs:266` is consistent: `service`
  cannot enter the lock/unlock path unarmed (`:305`), and `arm` sets
  `ARMED` at `:163`; no disarming store exists in this source.
- Compared directly with `775f5178`: the same pending-action/zero-deadline
  pattern remains for `REQUESTED` (`lock.rs:269`) and `next_client`
  (`desktop_session.rs:462`). The new check extends that pattern to resume.
- Exact-symbol audit of `desktop_session/`: its only `take_*` definitions
  are `lock::take_resumed` and `settings::take_token`. The latter is a
  synchronous write-rate bucket (`settings.rs:229`, consumed at `:565`);
  exhaustion returns busy and queues no action awaiting a future WM turn.
  It cannot strand a parked WM. Also checked the containing file's
  `pointer::take_sak` (`desktop_session.rs:7479`): it hands lock bits to
  the already guarded `REQUESTED` path (`:7487`), and the deferred
  next-client action has the existing `:462` guard. `take_client` is a
  pixel-copy operation, not a pending-action latch.

## Still true, relay-408 item 1: PASS

`kernel/src/arch/x86_64/syscall.rs:7216` registers, `:7227` rescans,
`:7245` retries identity changes, and `:7258` emits the kernel set before
parking at `:7259`. `kernel/src/sched/wait_any.rs:342` records a matching
registered key; `:250` reads that evidence before removal and `:260`
emits it. `tools/xtask/src/main.rs:9678` obtains gen-1 keys/caps,
`:9683` requires the matching registered fire between idle and end, and
`:9696` requires the following WM wake to name gen 1. Removing the death
fire (`ticket=1907`) from the fixture is still refused. A death before
registration that merely returns readiness from `syscall.rs:7209` cannot
substitute for this kernel witness. This is registration/fire/return
evidence, within option (A)'s scope.

## Validation and limits

Extracted the exact-head `require_cp61_parked_death`, `CP61_LOG` and both
upstream unit tests into a temporary Rust harness, changing only the
upstream tests' `super::` qualification. Added four review controls:
missing Guard 1 accepted; Guard 2's tick-only/no-request input refused;
missing gen-2 ready bit refused independently; missing gen-1 death fire
refused. Compiled with `rustc --edition 2024 --test`: **6 tests passed**.
The Guard-1 test intentionally confirms the defect rather than asserting
the desired behavior.

Read the pinned sources, commit 8, `775f5178`, previous RETURN, dispatch,
PR row and both PM comments. Known paths and exact-symbol searches were
sufficient; no semantic discovery was needed. No local QEMU executable
is available, so no independent boot run or reproduction of Seat I's
40-boot measurements or terminal scenario is claimed. No full workspace
build was run. This RETURN reviews the requested head, not a later fix.
