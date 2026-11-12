<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 408 — PR #108, A3p kernel-order re-check

Reviewer: Janus (GPT-6-Astra, medium).
Dispatch: `Plans/DISPATCH-2026-11-12-relays-407-408.md`, Section 408.
Reviewed head: `561b1a1d13e902ced9fed649406fbbc4135186e2`.
RETURN branch: `codex/pr108-recheck2-janus`, based on `origin/main`.
Coordinates refer to the reviewed head. No implementation changes.

**Verdict: FAIL (P2, item 3's causal-wake evidence remains missing).**
The kernel witness fixes the earlier preempt-before-syscall discriminator.
It now distinguishes an object registration from a userspace table log,
and the gen-2 request from gen-2 channel closure. However, the accepted
witness explicitly permits a timer to wake the WM before the request
arrives. A channel key recorded on an already-woken waiter does not show
that the request woke it. This is an evidence failure, not a demonstrated
production lost wake.

The [PM ruling](https://github.com/SecondStageTurbine/LogansOS/pull/108#issuecomment-6832650360)
relaxes reuse before the next wait. This review does not reinstate it.

## 1. Kernel registration and old-generation ordering: PASS, scoped

- **Kernel-side registration or park witness: PASS.**
  `kernel/src/arch/x86_64/syscall.rs:7216` registers; `:7227` scans again;
  `:7245` checks identity; `:7258` emits the test-only kernel set before
  `:7259` calls `park`. `wait_any_witness_set` at `:7021` records channel
  handles, keys and CapIds. The feature is declared at `kernel/Cargo.toml:614`
  and selected by the reuse scenario at `:1240`.
- **Registration → channel fire → WM return carrying dead-generation
  readiness: PASS.** `kernel/src/sched/wait_any.rs:331` records a matching
  fire on the registered entry; `:245` retrieves it before removal, and
  `:253` emits the kernel witness. `tools/xtask/src/main.rs:9659` obtains
  gen-1 keys/caps; `:9664` requires a matching kernel line between the
  probe's idle and end, under the latest cap set; `:9677` requires the
  following WM wake to name gen 1. This establishes registration/fire/
  return ordering. It does **not** establish that the channel fire caused
  the scheduler wake: `parked=0` explicitly means the tick woke it first.
  The dispatch permits a registration witness; no extra requirement of
  continuous blocking throughout the idle is imposed here.
- **Previous preempt-before-`Scene::park` counterexample rejected: PASS.**
  If death precedes the syscall, the first scan returns ready at
  `syscall.rs:7209`, without this registration or kernel wake. Removing
  the gen-1 death `WAITANY witness woke` line from the exact-head fixture
  makes the extracted oracle return `Err`. Its earlier userspace elapsed
  time and `c1g1` readiness can no longer substitute for that line.

## 3. Gen-2 first request is what wakes the WM: FAIL (P2)

The oracle at `tools/xtask/src/main.rs:9713` checks a gen-2 surface key and
cap between the key event and idle. This correctly excludes the previous
closure-only counterexample: the probe commits at
`userland/neo-worker/src/probe_app.rs:275`, logs idle at `:284`, and exits
at `:287`. Moving its only gen-2 kernel wake after idle is rejected.

But `:9715`–`:9718` never checks `parked` or `why`. The checked-in accepted
fixture itself is a concrete failing input (`xtask main.rs:29784`), with:

```text
DESKTOP park set c1g2 f=22 s=19
WAITANY witness set pid=1 items=12:0x10a8:14,22:0xb4d0:44,19:0x9cd8:40,
PROBEAPP key code=5 count=1
WAITANY witness woke pid=1 ticket=2598 parked=0 why=2 key=0xb4d0
DESKTOP park woke ms=500 ready=c1g2f,
PROBEAPP idling before exit ms=2000
```

The unchanged complete `CP61_LOG` returns `Ok(())`; the upstream acceptance
test at `:29807` expects that result. `why=2` is `WHY_TICK`
(`kernel/src/sched/wait_any.rs:63`).

**Concrete schedule:** the WM registers and blocks on gen 2. The next tick
sets `w.fired = WHY_TICK` and takes its token at `wait_any.rs:375`–`:378`,
then wakes the WM at `:388`. Before the WM runs/removes its entry, the
probe sends its first commit. `fire_key` records its surface key and
`parked=false` at `:331`–`:333`. Because `w.fired` is already nonzero,
`:335` skips the actual channel wake path. The WM resumes from its timer
wake, logs `parked=0 why=2 key=<gen-2 surface>`, and rescans the request.
The oracle accepts, although the request did not wake the WM.

Required fix: make the causal-request gate require a channel-caused wake
(e.g. `parked=1` with the appropriate IPC wake reason and gen-2 key), and
arrange a scenario that exercises it. Keep `parked=0` as honest diagnostic
information, but reject it as evidence for this requirement. Add a negative
control for exactly the existing timer-first fixture.

## 4. CP-61 row: guarantee wording PASS; exact rebuild citation FAIL (minor)

The current [PR body](https://github.com/SecondStageTurbine/LogansOS/pull/108)
now describes the narrower observation accurately: its CP-61 row says the
request **fires its surface key on a registration**, while the surrounding
text explicitly explains `parked=0`. That wording is supported by
`wait_any.rs:331` and `xtask main.rs:9713`. It does not repair item 3's
stronger PM requirement that the request cause the wake.

The row states the relaxed set-rebuild argument, as requested. However,
its literal citations `desktop_session.rs:2104`, `:2175`, and `:3328` are
stale at the stated head. The concrete input is that current CP-61 row:
`:2175` is a feature attribute, and `:3328` is an unrelated match arm.
The enforcing coordinates are `desktop_session.rs:2106` (`apps::turns`),
`:2170` (new item array each turn), `:2198` (populate from current clients),
and `:3351` (`wait_items`, iterating clients at `:3354`). Correct those
citations. This minor documentation issue is separate from the P2 verdict.

## Still true: PASS by source inspection, with a trace-check regression

| Requirement | Enforcing coordinates at `561b1a1d` |
|---|---|
| Bound | `kernel/src/sched/wait_any.rs:53` and `userland/libneo/src/wait_abi.rs:8`: 16. `syscall.rs:7142` rejects 17 before copy-in; fixed arrays at `:7149`, `:7159`. |
| CP-14 | `syscall.rs:7216` registration, `:7227` rescan, `:7245` identity retry; `wait_any.rs:225` checks fired before token installation at `:237`. |
| Fairness | `desktop_session.rs:3237` bounds each client to one request; `:4037` rotates start, `:4038` visits the clients, `:4059` enforces the bound. |
| Idle lock | `desktop_session.rs:458` minimizes the park deadline with `lock::park_deadline_ns`; `desktop_session/lock.rs:269`, `:272`, `:279` handle pending lock, locked/repaint, and idle expiry. |
| SAK latch | `lock.rs:177` records the request and `:269` forces zero deadline; `desktop_session.rs:462` also forces zero for a pending client switch. |
| Kill-switch | `desktop_session.rs:2167` gates parking; `:2225` retains polling; `tools/xtask/src/main.rs:9495` honors `desktop-poll`. Witnesses are separately feature-gated. |
| No dead-generation readiness | `desktop_session.rs:4342` removes the ended client; `:2170`/`:2198` rebuild the next set; `:3354` enumerates current clients. Kernel `syscall.rs:7245` compares identities. `xtask main.rs:9686` rejects subsequent registrations of gen-1 caps and `:9704` rejects those caps in gen 2. |

**Trace limitation:** the new oracle dropped the old explicit rejection of
later WM sets/wakes naming `c1g1`. Concrete input: replace only
`DESKTOP park woke ms=500 ready=c1g2f,` in `CP61_LOG` with
`DESKTOP park woke ms=500 ready=c1g1f,`. The extracted oracle still returns
`Ok(())`, because it only examines later kernel registration caps. Source
inspection still supports the production invariant above; do not describe
this oracle as independently rejecting every dead-generation readiness
trace. Restoring the old WM-generation negative check would retain that
coverage alongside the stronger CapId check.

## Validation and limits

- Read the dispatch, previous RETURN, PM ruling, current PR body, exact-head
  new commit, and relevant implementation paths. Repository `AGENTS.md`
  was the only tracked instruction file. All paths were known, so direct
  `git show` reads were used.
- Compiled the exact oracle and its two upstream unit tests extracted from
  `561b1a1d` using `rustc --edition 2024 --test`, adding four review controls:
  original preempt schedule rejected; gen-2 closure-only rejected;
  timer-first fixture accepted; stale WM-generation readiness accepted.
  **6 tests passed.** Extraction changed only the tests' `super::` path
  qualification to compile outside their original module.
- No local QEMU executable was available. No independent boot trace,
  scheduler fault injection, whole-workspace test run, or new binary
  equality comparison is claimed. The concrete timer schedule follows
  the exact source and matches the accepted fixture.
