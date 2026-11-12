<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 400 — PR #108, A3p, AAG cross-family review

Reviewer: Janus (Codex).
Dispatch: `Plans/DISPATCH-2026-11-12-relays-400-402.md`, Section 400.
Reviewed head: `259feac29778164d814ad9ac029da22a6a20f5f0`
(`claude/a3p-parked-loop`), against its base `3ed6c040`.
RETURN branch: `codex/pr108-review-janus`, based on `origin/main`.
All coordinates below refer to the reviewed head. No implementation changes.

**Verdict: FAIL the requested CP-61 scenario gate (P2, evidence gap).
The implementation review passes; no functional lost-wake or stale-handle
counterexample was found.** The existing reuse scenario does not establish
the dispatched death/park/reuse ordering. The distinction matters: a green
ordinary reuse test is not evidence that the specified race was exercised.

## 1. The bound

- **PASS — `ITEMS_MAX = 16`, with bounded arrays.**
  `kernel/src/sched/wait_any.rs:53` sets the bound; libneo agrees at
  `userland/libneo/src/wait_abi.rs:8`. The static waiter's keys are fixed at
  `wait_any.rs:72`; registration clamps the key count before copying at
  line 158. Syscall identity keys/caps are fixed at
  `kernel/src/arch/x86_64/syscall.rs:6967`; each channel or pipe item adds
  at most one identity (`:7022`, `:7054`). The userspace entry bounds `n`
  before decoding fixed arrays (`:7078`, `:7085`, `:7095`), and passes only
  `items[..n]` (`:7100`). The readiness mask is `u64` (`:7016`), so bit 15
  fits. The test harness's enlarged arrays are also fixed
  (`kernel/src/test_harness.rs:28078`, `:28088`).
- **PASS — 17 items returns `EINVAL` before any validation or copy-in.**
  `syscall.rs:7078` returns at line 7079; user-buffer validation and copying
  occur only at lines 7087 and 7091. `wait_any_core` leg 8 supplies
  `0xffff800000000000` with 17 and 16 items and requires `-22` and `-14`
  respectively (`kernel/src/test_harness.rs:28632`, `:28636`).
- **PASS — fixed per-wait array cost at 16 is 912 bytes, +448 versus 8.**
  On x86-64: raw copy-in 128 B (`syscall.rs:7085`), decoded items 128 B
  (`:6954`, `:7095`), two identities of 264 B each (two 128 B arrays plus
  `usize`, `:6966`, `:7121`), and registration's key copy 128 B
  (`wait_any.rs:157`). Sum: `128 + 128 + 2*264 + 128 = 912` B; the same
  inventory at 8 is 464 B. This is a source-level array inventory, not a
  measured maximum stack depth: scalar locals, waiter/guard temporaries,
  nested calls and compiler layout add overhead or permit reuse. The PR's
  "about 1 KiB" is reasonable for that inventory. The enlargement adds no
  heap allocation. Static key capacity is `16 waiters * 128 = 2048` B,
  excluding each waiter's other fields (`wait_any.rs:56`, `:69`).

## 2. CP-14: last try-receive to park

- **PASS — a send in the gap cannot strand the WM.** The last Arthur try
  is `userland/neo-init/src/desktop_session.rs:2116`; the rebuilt wait enters
  the kernel at `:2184` / `:467`. A sender already present is seen by the
  first scan (`syscall.rs:7141`). Registration occurs at `:7152`; the exact
  post-registration recheck is **`syscall.rs:7163`**. A send after that scan
  sets `fired` under the wait-table lock (`wait_any.rs:284`), and park checks
  it at `:208` / `:213`. If the token is already installed, fire takes it
  and wakes it after dropping the table guard (`:287`, `:304`). The send
  holds the channel lock through readiness publication and its own park
  (`kernel/src/ipc/mod.rs:215`, `:216`), so a recheck cannot see an
  unpublished sender after observing its fire.
- **PASS — the named discriminator is `wait_any_core`, leg 9.**
  `kernel/src/test_harness.rs:28643` widens the kernel check-to-park window;
  the sender waits for registration (`:28119`) and sends on the last of
  the 16 distinct channels (`:28073`, `:28648`). Eight rounds require bit
  15, and the test rejects zero `fired_before_park` (`:28659`, `:28675`,
  `:28678`). The hook runs before park takes the table lock
  (`wait_any.rs:205`). This is a kernel race discriminator, rather than
  an instrumented WM turn; it exercises the primitive the WM calls.

## 3. CP-61: death and slot reuse

- **PASS — source enforcement prevents stale registration or a missed new
  client.** Peer death closes and fires both channel sides
  (`kernel/src/ipc/mod.rs:757`, `:768`); closed channels are ready (`:628`).
  After a wait returns its registration is removed (`wait_any.rs:231`).
  The single-threaded WM removes the old client from `others` before closing
  or releasing its handles (`desktop_session.rs:4143`, `:4147`, `:4219`).
  New clients are installed with a new generation (`:3828`, `:3830`), stale
  pending replies are discarded (`:3886`), and every park rebuilds its
  handle list after `apps::turns` (`:2103`, `:2172`). `wait_items` reads
  only current live entries (`:3315`, `:3618`). No old wait array is reused.
  Independently, kernel removal notification fires the old key
  (`kernel/src/sched/process.rs:2206`), and the second scan compares both
  keys and capability identities before parking (`syscall.rs:7181`). A
  changed identity unregisters and retries, rather than sleeping on the
  old object. `wait_any_core` leg 5 checks actual numeric-handle reuse and
  a wake from the replacement channel (`test_harness.rs:28506`, `:28531`).
- **FAIL — the required WM scenario ordering is not demonstrated (P2).**
  The PR calls `desktop_apps_reuse_boot` the death-while-parked CP-61
  discriminator. Its script instead sends `a`, waits for the first client's
  **ended** log, then types `open probe app` and sends `b`
  (`tools/xtask/src/main.rs:758`, `:765`, `:767`, `:770`). The oracle checks
  client-table slot 1's generation 1 then 2, cleanup and a new key (`:21361`),
  without comparing old/new numeric channel handles or observing the WM's
  registration at death. The generic park check accepts a ready return
  `rc=1`, which need not have blocked at all (`:9379`, `:9381`). Kernel leg
  5 covers a different ordering: sibling rebind **before registration**
  (`kernel/src/test_harness.rs:28475`). It does not fill this WM timing gap.

  **Concrete input/schedule exposing the evidence failure:** run the existing
  reuse script unchanged. Let the WM service the probe's `a`-triggered commit
  and let the probe exit 7 immediately after its reply, while the WM is still
  running (`userland/neo-worker/src/probe_app.rs:275`, `:278`). Let the WM
  observe the closure, remove/reap/release the client and enter another
  `SYS_WAIT_ANY` before the subsequently injected `open probe app` command
  has completed. Relaunch, deliver `b`, and finish with `again`. Every current
  serial assertion can pass, although death was not established to occur
  during a park and reuse occurred after an intervening wait. This is a
  counterexample to the claimed discriminator, not a demonstrated production
  malfunction.

  **Required follow-up:** bind the test to an observed WM registration on
  the old client's channels, force death there, record the removal and
  actual numeric-slot reuse before the next registration, then require a
  replacement-client request to wake/service the WM with unrelated input
  quiet. Check the next registration excludes the old identity, and that
  old-object activity cannot satisfy the replacement wait. A suitable armed
  hook or an equivalent deterministic witness should make the ordering an
  assertion, rather than an assumption about scheduling.

## 4. The park loop

- **PASS — fairness is preserved.** `TURN_REQUESTS = 1`
  (`desktop_session.rs:3198`); `apps::turns` advances the rotating start
  (`:3876`), visits each live slot once (`:3877`), and takes at most that
  many requests (`:3898`). Other clients run before Arthur's single receive
  (`:2103`, `:2116`). Parking is after the completed turn and does not
  replace this service order with wait-list order (`:2172`, `:2184`).
- **PASS — the established A1a sends remain try-only.** Arthur's armed
  replies, pending retries and other-client replies use `sys_try_send`
  (`desktop_session.rs:3277`, `:3357`, `:3891`, `:3998`). Other-client
  handshake/handoff uses bounded `Link` and its try send/grant paths
  (`:3824`, `:4068`, `:1491`, `:1543`). A3p changes none of these sends.
  The pre-existing blocking Arthur startup exception, before other clients
  exist, is explicitly allowed by the design (`THINK §2.3`, item 7's
  implementation note); it is not a new parked-loop send.
- **PASS — the park deadline includes the idle-lock deadline.**
  `Scene::park` takes the minimum at `desktop_session.rs:458`.
  `desktop_session/lock.rs:265` supplies `LAST_INPUT_NS + idle_limit`;
  unarmed/off cases are deliberately unbounded (`:252`, `:262`), and a
  locked/paint-pending screen gets a 10 ms tick (`:258`). Input polling also
  bounds the kernel's internal park to the next tick (`syscall.rs:7136`).
  Deadline delivery has the existing 100 Hz timer granularity, with rounding
  up (`kernel/src/sched/wait_any_logic.rs:32`), rather than a sub-tick wake
  guarantee. The next turn serves the lock in `pointer_tick`
  (`desktop_session.rs:511`).
- **PASS — an actionable SAK consumed this turn cannot cause a sleep before
  its next-turn service.** Super+L sets `REQUESTED` (`lock.rs:176`); armed
  `park_deadline_ns` returns 0 for it (`:255`). Super+Tab sets `next_client`
  (`desktop_session.rs:7417`), which forces deadline 0 (`:462`); the next
  `apps::turns` takes it (`:3973`). The kernel still scans ready items, but
  an empty scan with deadline 0 returns `ETIMEDOUT` before park
  (`syscall.rs:7189`). Unarmed Super+L intentionally has no lock action.

## 5. CP table and the park/wake handoff classes

- **FAIL — the table's CP-61 discriminator claim is stronger than its test.**
  See item 3 and its concrete schedule. **PASS** for the table's source
  descriptions of the guards, including CP-14's WM-side latch row.
- **PASS — CP-19…28 are left unchanged, as the PR states.** No scheduler,
  idle, IPI, bootstrap or partition implementation changes appear in the
  PR's ten-file diff. This is preservation of existing mechanisms and
  limitations, not a claim that A3p repairs every older class:

  | Classes | Enforcing unchanged seam / scope |
  |---|---|
  | CP-19, CP-20, CP-25, CP-26 | Existing work polling and partition/boot behavior; idle still polls above the IF-off check (`kernel/src/sched/scheduler.rs:1892`). A3p adds no spawn, partition or join path. |
  | CP-21, CP-24, CP-27 | Existing wake token routes to its owning CPU and sends the reschedule IPI (`scheduler.rs:2479`); bootstrap and cross-CPU enqueue are untouched. No new latency or readiness-barrier guarantee is claimed. |
  | CP-22, CP-28 | IF is disabled before rotate/check (`scheduler.rs:1903`), followed by atomic enable-and-halt (`:1927`). |
  | CP-23 | The idle path remains halt-based (`scheduler.rs:1927`); no MWAIT path is added. |

- **PASS — no new lock is held across a park.** Registration drops its
  table guard on return (`wait_any.rs:176`); each readiness scan releases
  its channel/handle references (`syscall.rs:7015`). The existing park
  deliberately passes the wait-table guard into token installation
  (`wait_any.rs:223`), and the switch releases it and the scheduler lock on
  the new stack (`scheduler.rs:2262`, `:2271`). Wakers drop the wait-table
  guard before taking the scheduler (`wait_any.rs:296`, `:304`). A3p adds
  arrays and deadline calculations, not another lock at this seam.
- **PASS — the other CP rows match the change.** CP-1/16/37 retain the
  existing IRQ-safe lock/order (`wait_any.rs:85`, `:151`, `:207`); CP-30
  retains plain keys and drops scanned owners before park
  (`syscall.rs:7012`); CP-54 has the 16-item and six-client bounds
  (`syscall.rs:7078`, `desktop_session.rs:110`, `:3320`); CP-74's added
  array storage is fixed, as quantified in item 1. No driver path changes.

## 6. Kill-switch

- **PASS — with `desktop-park` off, the compiled polling code is identical.**
  The new loop and helpers are feature-gated (`desktop_session.rs:2143`,
  `:3311`); the original spin remains at `:2186`. xtask suppresses automatic
  parking when `desktop-poll` is named (`tools/xtask/src/main.rs:9329`).
  I independently built neo-init at the exact head and its actual base
  `3ed6c040`, sequentially in the same directory, with the PR's image-A
  feature list and no `desktop-park`:

  ```text
  cargo build -Zjson-target-spec --package neo-init --bin neo-init \
    --features desktop-intro,desktop-clock,desktop-busy,desktop-wheel,atrium-app-icons,login,cap-rooted-open,power-menu,atrium-settings-store
  ```

  | Section | Bytes on each side | Result / SHA-256 for identical sections |
  |---|---:|---|
  | `.text` | 768308 | Identical: `1c599966db10ff4729cd28e65a2684e58555048d7d5dd37345886c92d10a28f5` |
  | `.data` | 8 | Identical: `f0a0278e4372459cca6159cd5e71cfee638302a7b9ca9b05c34181ac0a65ac5d` |
  | `.rodata` | 926472 | 258 changed `u32` source-line values only. |

  I decoded every changed `.rodata` word's containing panic `Location`:
  240 name `desktop_session.rs`, 18 name `desktop_session/lock.rs`.
  Each old/new location names the same file and identical source line.
  Line deltas are +29 (55 words), +38 (18), +54 (16), +114 (169).
  Thus this proves executable polling-loop equality, with changed diagnostic
  line numbers; it does **not** claim whole-ELF digest equality. The PR's
  older figures (768292 text bytes, 240 line words and +17/+42/+102) are
  not the final-head figures; the independent comparison above covers the
  final idle-lock and SAK fixes and the rebased base.

## Validation and limits

- Read the exact-head implementation, dispatch/design, PR body and PM
  comments; checked the ten-file diff against `3ed6c040`.
- `cargo test -p atrium-logic-tests`: **162 passed**, zero failures,
  including the ABI/bound and deadline KATs.
- `cargo test -p xtask feature_resolver_follows_neo_init_features`:
  **1 passed**, zero failures.
- Both custom-target neo-init polling builds above succeeded. Extracted
  and compared ELF sections; independently resolved all changed diagnostic
  records back to identical source lines. Restored the review worktree to
  `259feac2`; it is clean.
- Verified via GitHub API that [run 43225069396](https://github.com/SecondStageTurbine/LogansOS/actions/runs/43225069396)
  completed successfully at predecessor `a958f3e1`. It is not an exact-head
  result. At 2026-11-12 15:51 UTC, [run 43244583909](https://github.com/SecondStageTurbine/LogansOS/actions/runs/43244583909)
  was still in progress at `259feac2`: host/build/fuzz and both production
  boot jobs had succeeded; all four integration shards were running.
- No local QEMU execution or exact-head green full-tier result is claimed.
  The remaining review request is the CP-61 discriminator described above;
  an ordinary green rerun alone does not supply that missing ordering proof.
