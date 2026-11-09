<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 231 — PR #15 client-table review — Chad

Reviewed head: `87bad1b7dee85e20fd2c24124ce70b6e38f2a27e`.
RETURN branch: `codex/pr15-review-chad`, based on `origin/main` at
`10e10ee99b282ae163749748a9e4427ef5132deb`.
All code locations below refer to the reviewed head. Overall: **FAIL**.

- **FAIL — 1. Syscall arguments: the new send inherits a permission-blind copy (P1).**
  `SYS_TRY_SEND` rejects zero length at `kernel/src/arch/x86_64/syscall.rs:6126`,
  checks the user-half range and overflow at `:6129` / `:1491`, and checks handle
  existence, kind and USE/SEND at `:5891`. `SYS_TRY_GRANT` checks its channel's
  USE/SEND at `:6145`, source existence at `:6152`, GRANT at `:6161`, and the
  Installer exclusion at `:6166`. `SYS_TRY_RECV_GRANT` checks USE/RECV at `:6186`.
  Both grants use `kernel/src/sched/process.rs:3825`, which rechecks the source
  by capability identity under CAP_GRAPH and intersects source, offered and
  recipient rights at `:3861`; an empty meta intersection is refused. Client
  indices are WM-generated: `index_of` returns only 1 at
  `userland/neo-init/src/desktop_session.rs:2789`; no syscall accepts a client-table
  index. The handshake checks dimensions at `:1449`, buffer description at
  `:1503`, and channel-bound identities at `:1527` and `:1560`.

  The missing check is memory permission: the new path calls
  `cross_pmap_copy` at `kernel/src/ipc/mod.rs:352`; its page walks at `:906` and
  `:907` use `Pmap::translate`, which checks only nonempty entries, not USER or
  destination WRITABLE (`kernel/src/mm/pmap.rs:1259`). The HHDM store at
  `kernel/src/ipc/mod.rs:918` bypasses those permissions. **Concrete input:** a
  receiver parks `SYS_RECV(channel, ro_shared_buffer_va, 1)` on a buffer mapped
  with MAP_READ only; the peer calls `SYS_TRY_SEND(channel, valid_byte_va, 1)`.
  The new send writes the shared buffer through HHDM rather than refusing the
  read-only destination. This helper defect also exists in the older blocking
  IPC path; the new syscall reuses it. Enforce user/read/write permissions for
  every translated page, including the parked receiver's destination.

- **FAIL — 2. Death, failed activation and reuse leave stale state (P1).**
  The normal secondary-client path does take the table entry and clear its
  pending reply (`userland/neo-init/src/desktop_session.rs:3094`), and reuse is
  withheld while `dying` is occupied (`:2813`). Two gaps remain:

  **Failed first handoff leaves the failed client's input route installed.**
  `switch` activates the new client at `:3035`, but publishes `apps.front` only
  at `:3059`, after its K/D exchange. A discard failure calls `end` at `:3054`;
  `end` restores Arthur only if the *old* `apps.front` equals the failed index
  (`:3110`). **Concrete input:** launch from Arthur, complete the probe's surface,
  buffer and activator handshake, then never receive K or never answer D.
  After the one-second deadline, slot 1 is removed while its focus remains the
  kernel route; the table still says Arthur is front. Roll back the installed
  route on every failed handoff, independently of the previous front index.

  **Reaping does not release the advertised handles.** `release` calls
  `sys_channel_close` on buffer, activator and process handles at `:3155`.
  That wrapper invokes SYS_CLOSE (`userland/libneo/src/lib.rs:1096`), whose
  accept predicate allows only File and Channel
  (`kernel/src/arch/x86_64/syscall.rs:6890`). It returns EBADF without removal
  for the other kinds (`kernel/src/sched/process.rs:3183`); all errors are
  ignored. `SYS_WAIT` does not close the process handle (`syscall.rs:7862`).
  **Concrete input:** repeatedly launch the probe, let it exit, wait for
  `client ended`, and relaunch into slot 1. The table is reused but the WM keeps
  old buffer/focus/process handles. Old focuses retain routes until their last
  Arc drops (`kernel/src/atrium/focus.rs:778`); enough cycles exhaust the 32-route
  cap (`:505`). Use an operation that actually releases each handle kind and
  check its result. The mapping guard unmaps pixels, but does not remove these
  handles.

- **FAIL — 3. A client can starve or park the WM and other clients (P1).**
  The secondary-client limits are real: five secondary slots, one request per
  turn, one pending reply, a five-second reply timeout and rotating order
  (`userland/neo-init/src/desktop_session.rs:2718`, `:2763`, `:2888`, `:2898`).
  They apply only when `apps::turns` is reached. Arthur's channel is checked
  first; any ready request breaks out before secondary turns or pointer service
  (`:1801` through `:1817`). Arthur's replies still use blocking sends
  (`:1925`, `:2606`). **Concrete input A:** while the probe has a commit pending,
  have Arthur continuously send valid commits and receive each reply, arranging
  the next send before the WM's next poll. Every poll selects Arthur; the probe,
  its pending-reply deadline, reaping and Super+Tab get no turn. **Concrete input
  B:** Arthur sends one valid commit and then never receives the reply. The WM
  parks in SYS_SEND indefinitely. Arthur's fault/exit also returns code 24 at
  `:1805`, bypassing `apps::end_all` at `:2224`; the wrapper clears only Arthur's
  entry (`:1620`), and the caller exits the desktop (`:1243`). Give Arthur the
  same bounded turn, pending reply and local-failure treatment as other entries,
  and clean up all entries on every session exit.

  Failed handshakes also lack resource cleanup: `connect` closes only the
  session and kills the child (`:1426`), while `handshake` can already have
  created surface endpoints (`:1458`) or received a buffer (`:1491`). The failed
  client's `Dying` record deliberately has no handles (`:2863`). **Concrete
  input C:** send a valid Create, then grant a buffer with a wrong descriptor;
  repeat after each rejected launch. WM handles accumulate despite the fixed
  client count. Track intermediate handles with cleanup guards as well as the
  existing mapping guard.

- **FAIL — 4. Pointer routing survives a focus change or client death (P1);
  the lock exclusion itself is enforced.**
  Keyboard dequeue checks the active focus under DELIVERY
  (`kernel/src/atrium/focus.rs:275`). The lock route overrides it (`:828`),
  pointer forwarding filters locked targets (`:634`), and WM route changes are
  deferred while locked (`userland/neo-init/src/desktop_session.rs:4754`).
  Those checks protect lock-screen input.

  However, the new `apps::switch` (`:3019`) and `apps::end` (`:3087`) do not
  cancel `Pointer.grab`. A grab captures an activator handle at `:5786`, and
  subsequent records keep using it at `:5821`, regardless of the current front.
  Kernel forwarding selects the named route, without checking that it is the
  current active client (`kernel/src/atrium/focus.rs:646`). **Concrete input:**
  hold a mouse button inside Arthur's surface, launch the probe with the
  keyboard while the button remains down, then move and release. The probe is
  front, but those records are queued to Arthur. Super+Tab back to Arthur does
  not discard his queue (`desktop_session.rs:3027`), so he can subsequently
  consume input produced while another client was front. Killing a grabbed
  probe likewise leaves a grab naming its old activator. Consume the grab on
  switch/death before another client's turn, and test both held-button cases.

- **PASS — 5. Feature-off selection and compile checks.**
  New syscall constants, dispatch arms and implementations are gated by
  `ipc-try-send` (`kernel/src/arch/x86_64/syscall.rs:365`, `:1326`, `:6124`);
  the channel methods have the same gate (`kernel/src/ipc/mod.rs:332`). Kernel
  defaults do not enable it (`kernel/Cargo.toml:77`). The secondary-client
  module and turn calls are gated by `atrium-apps`
  (`userland/neo-init/src/desktop_session.rs:2702`, `:1809`), with the previous
  blocking Arthur link selected at `:1646`. `atrium-apps` is opt-in
  (`userland/neo-init/Cargo.toml:88`), and `desktop-full` does not select it
  (`:113`). Local kernel checks passed with and without `ipc-try-send`; local
  neo-init checks passed for `atrium-apps,desktop-home-boot-test` and for
  feature-off `desktop-full`. This is a source/build kill-switch PASS, not a
  claim of locally rerun screenshot or binary identity.

- **FAIL — 6. Prior-art/CP material is present, but its implementation claims
  do not hold.** The PR body has a “Prior-art and pitfall-bank” section pointing
  to `docs/THINK-2026-10-29-ot88-app-model.md`. That document contains the pinned
  app-model survey at `:702`, the CP pass at `:306`, and the A0k/A1a acceptance
  rows at `:650`. The existing survey was not changed by this PR; I did not
  independently refetch its historical upstream trees. The implementation
  contradicts its CP-57 claim that every WM send to a client is a try (`:315`),
  its bounded-service claim (`:323`) and its CP-52 death-reset requirement
  (`:311`). **Concrete inputs:** checklist 3's Arthur commit-without-receive
  parks the WM; its continuous-commit schedule starves another client;
  checklist 4's held-button switch/death retains a stale grab. The new
  `ipc_try_send` test exercises Channel methods directly
  (`kernel/src/test_harness.rs:27902`), so it does not establish syscall buffer
  permission validation or the WM's fairness/cleanup properties. The missing
  counterexamples need discriminating coverage before those claims can pass.

Validation: `cargo test -p libneo -p atrium-logic-tests` passed (146 tests).
The four targeted `cargo check -Zjson-target-spec --bin ...` configurations
listed in item 5 passed. `git diff --check origin/main...pr15` passed.
GitHub reports success for host checks, product builds and all four QEMU
shards at the exact reviewed head:
[gate run](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41268541195).
QEMU is unavailable locally; the counterexamples above are source-traced
inputs/schedules, not claimed runtime reproductions. No PR code was changed.
