<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 261 — PR #15 re-check — Chad

Reviewed head: `e7bef755e645dc3397647a7b34fd5b3af14fa3e3`.
RETURN branch: `codex/pr15-recheck-chad`, from `origin/main` at
`f2210a32a`.
Earlier report: relay 231, `Plans/RETURN-2026-11-01-pr15-client-table-review-chad.md`
on `origin/codex/pr15-review-chad`. All code locations below refer to the
reviewed head. Seat notes: latest
[PM: Seat F](https://github.com/SecondStageTurbine/LogansOS/pull/15#issuecomment-6563986460).
Overall: **FAIL**; item 3's client-local failure requirement and a blocking
permission handoff remain, despite the fixes below.

- **PASS — 1. Permission-blind copy.** `kernel/src/ipc/mod.rs:350` calls
  `cross_pmap_copy`; the parked destination is accessed through
  `with_user_frame(..., true, ...)` at `:935`. `kernel/src/mm/pmap.rs:1272`
  holds the Pmap lock through the copy; `:1283` checks PRESENT/USER at every
  level and WRITABLE for a destination. A refusal wakes the receiver with
  EFAULT and returns EFAULT to the sender (`ipc/mod.rs:357`). This is the
  integrated #13 fix, not an independent PR #15 fix. The original read-only
  destination input is now rejected. The new discriminator maps USER_DATA_RO,
  checks EFAULT on both sides and checks untouched bytes
  (`kernel/src/test_harness.rs:28431`–`:28470`). It calls the Channel API from
  kernel threads, so it is not itself a ring-3 syscall test.

- **PASS — 2. The two previously identified stale-state defects.** A failed
  K/D exchange ends the client and unconditionally switches back to Arthur
  (`userland/neo-init/src/desktop_session.rs:3368`–`:3384`), independent of
  the old front. `SYS_CLOSE` now accepts Buffer/InputFocus/Process with
  `close-object-handles` (`kernel/src/arch/x86_64/syscall.rs:6990`, `:7015`),
  removes the handle under the capability graph/table locks
  (`kernel/src/sched/process.rs:3179`–`:3192`), then drops the object outside
  them. `release` closes both surface ends, buffer, activator and process,
  counting failures (`desktop_session.rs:3489`); the desktop-app test kernel
  features enable the required close feature (`kernel/Cargo.toml:1073`,
  `:1084`–`:1087`). Pending replies are cleared on death (`:3425`) and tagged
  with the slot generation (`:3144`, `:3200`). This fixes the old wrong-kind
  close and failed-first-handoff examples. See the new endpoint leak below
  before treating this as complete resource accounting.

- **FAIL — 3. Fair turns and reply parking are fixed, but Arthur's failure
  still ends the desktop; a permission handoff still parks it (P1).** The
  old continuous-commit schedule now services the other clients and pointer
  before Arthur (`desktop_session.rs:1872`–`:1888`). Surface replies use
  `defer_arthur_reply` (`:2756`, `:2909`), retry for at most five seconds
  (`:2942`), and keep the loop polling while owed (`:1908`). The original
  commit-without-receive no longer parks in SYS_SEND. Secondary cleanup now
  runs on every exit through `Session::drop` (`:3057`), and early Arthur
  cleanup runs at `:1657`.

  **Concrete input A:** with a live probe app, Arthur sends one valid commit
  and never receives its reply. After REPLY_NS, `flush_arthur` returns `hung`
  (`:2954`); its caller returns 24 (`:1868`–`:1870`), the Session drop ends
  the probe too, and the outer desktop loop returns the error (`:1246`).
  Arthur exiting/faulting similarly hits `:1888`. Cleanup is improved, but
  this still violates the original requirement to end only the failing client
  and keep the WM/other clients alive. The PM note acknowledges this gap.

  **Concrete input B:** enable `atrium-apps,arthur-actions`, request a valid
  permitted action, let the user approve it (or use its standing approval),
  receive the successful permission reply, then stop receiving. The WM
  settles the reply and calls `permission::lend` (`:2167`–`:2170`), which
  calls blocking `grant_over`
  (`userland/neo-init/src/desktop_session/permission.rs:472`–`:475`). Its
  first `sys_send(chan, b"O")` has no timeout
  (`desktop_session.rs:3847`–`:3851`). The delivered reply proves no readiness
  for this next receive. The WM parks indefinitely, so secondary turns,
  reaping and pointer service stop. Use a bounded/pending try handshake here.
  The PM's disclosure of this gap does not satisfy item 3.

  **PASS for the original failed-handshake intermediate handles:**
  `HandshakeHeld::drop` closes the two surface endpoints, imported buffer and
  activator accumulated before a failure (`:1443`, `:1458`, `:1488`, `:1521`,
  `:1602`); the later-declared mapping drops first. The bad-buffer-description
  example no longer leaks those particular handles. The session peer remains
  outside this guard, as noted below.

- **PASS — 4. Original held-button switch/death counterexamples.** Before
  forwarding a grabbed record, the WM compares the grab's activator with the
  front; a mismatch consumes the remainder of the press
  (`desktop_session.rs:6181`–`:6203`). Thus motion/release while the probe is
  front cannot queue to Arthur's old grab, and a dead probe's grab cannot
  forward after Arthur becomes front. The new scenario explicitly scripts
  both cases (`tools/xtask/src/main.rs:649`–`:667`) and checks the consumed
  records in order (`:17432`–`:17446`). Cancellation is lazy on a pointer
  record, not an explicit reset in `switch`/`end`; this PASS covers the
  original counterexamples, not every possible focus-history schedule.

- **FAIL — 6. CP material exists, but the remaining implementation claims
  still do not hold.** The current PR body adds named CP-57/54/52
  discriminators, and the design survey remains at
  `docs/THINK-2026-10-29-ot88-app-model.md:702`. The flood scenario now demands
  probe input and Super+Tab before the flood completes, with at least 20
  commits (`tools/xtask/src/main.rs:17399`–`:17428`); the pointer and read-only
  discriminators address the old gaps. However the design's every-send/grant
  is-a-try claim (`docs/THINK-2026-10-29-ot88-app-model.md:318`) and client-local
  failure requirement (`:651`) still fail on item 3's concrete inputs A/B.
  The PR body acknowledges the unchanged permission grant but labels the
  remaining blocking steps bounded. The blocking offer at
  `desktop_session.rs:3848` is not bounded. Add discriminating Arthur
  nonreceiver/permission-handoff tests and preserve the healthy client/session
  before claiming these properties.

- **New — FAIL (resource cleanup):** `connect` creates `(session, client_end)` at `desktop_session.rs:1419`, but only passes `client_end` to spawn (`:1423`); spawn clones inherited handles rather than consuming the parent's (`kernel/src/arch/x86_64/syscall.rs:1982`–`:1988`), and neither `Client` (`desktop_session.rs:116`) nor `HandshakeHeld` nor the failure path (`:1429`) retains/closes that session peer. Concrete input: repeatedly launch/exit/reap/relaunch the probe, or repeatedly fail its buffer handshake; each cycle leaves one additional parent session endpoint even when `released ... closed=5 failed=0` passes. Retain that peer through the client's lifetime and close it on teardown; close both session ends on spawn failure too (`spawn(client_end)?` currently skips cleanup).

Validation at the reviewed head: `cargo test -p libneo -p atrium-logic-tests`
passed (146 tests); `git diff --check origin/main...e7bef755` passed.
Targeted checks passed: kernel `cargo check -Zjson-target-spec --bin kernel
--features ipc-try-send,close-object-handles`; neo-init `cargo check
-Zjson-target-spec --bin neo-init --features
atrium-apps,desktop-home-boot-test,arthur-actions` (each from its package
directory). The earlier item 5 was PASS; feature-off binaries were not rebuilt
in this narrow re-check. QEMU is unavailable locally; counterexamples are
source-traced, not claimed runtime reproductions. At the final status check,
the exact-head [gate run](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41350979533)
was still in progress, so this report does not claim that gate passed.
No PR code was changed.
