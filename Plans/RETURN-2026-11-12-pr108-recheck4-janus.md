<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Relay 412 — PR #108, A3p item 3 guards

Reviewer: Janus.
Dispatch: `Plans/DISPATCH-2026-11-12-relays-412-413.md`, Section 412.
Reviewed head: `d276e74642e8885e36157f66475394a68cc27abd`.
RETURN branch: `codex/pr108-recheck4-janus`, based on `origin/main`.
All source coordinates below refer to the reviewed head. No implementation changes.

**Verdict: FAIL — Guard 2's committed test does not retain the specified WM tick wake.**
Guard 1 and the accepted timer-first fixture pass. The oracle rejects the
precise Guard 2 input, but the committed test deletes the WM wake instead
of retaining it with no `c1g2f`.

## Guard 1: PASS

`tools/xtask/src/main.rs:9955` through `:9963` requires a kernel
`WAITANY witness woke pid=` line containing ` parked=1 why=1 ` anywhere
in the supplied boot log, returning `Err` if none exists. The accepted
fixture now contains Arthur's key `0x10a8` fire with `parked=1 why=1`
at `:30420`. The negative case at `:30489` through `:30493` changes that
fire to `parked=0 why=2`, and `:30503` asserts refusal. This case passes.
Independently deleting the entire Arthur fire (the relay-410 input)
returns the specific missing-parked-waiter error.

## Guard 2: FAIL as a specified test; refusal behavior PASS

`cp61_oracle_refuses_a_tick_wake_without_the_request` at
`tools/xtask/src/main.rs:30448` removes a two-line string at `:30450`
through `:30452`: both the `ticket=2598` kernel fire and
`DESKTOP park woke ms=490 ready=c1g2f,`. It asserts `Err` at `:30457`,
but leaves **no gen-2 WM wake** in the fixture. Consequently it is not
the dispatch's negative control in which the tick wakes the WM and that
next wake has no `c1g2f`.

Concrete required input: take `CP61_LOG` (`:30416`), delete only the
`WAITANY witness woke ... ticket=2598 ...` line (`:30434`), and replace
`DESKTOP park woke ms=490 ready=c1g2f,` (`:30435`) with
`DESKTOP park woke ms=490 ready=`. Preserve the cap-44 registration,
Arthur parked fire, and every earlier gen-1 event. The committed test
currently deletes that empty-ready WM wake too.

I independently ran the exact required input: it returns `Err` containing
`no kernel wake on gen 2's surface key`, enforced by the request search
at `:9941` through `:9953`. Thus this is a test-fixture mismatch, not an
observed oracle acceptance defect. To complete Guard 2, change the
replacement at `:30452` from the empty string to
`"DESKTOP park woke ms=490 ready=\n"`, retaining the existing refusal
assertion (and preferably asserting that this WM wake remains).

## Still true: PASS

The accepted fixture's gen-2 fire remains timer-first (`parked=0 why=2`,
`fire_tick=deadline=1894`) at `tools/xtask/src/main.rs:30434` and its next
WM wake names `c1g2f` at `:30435`. The acceptance test at `:30440`
through `:30441` passes with the separate Arthur guard satisfied.

Item 3's original points 1 and 2 remain enforced and unchanged by this
commit: `:9939` and `:9940` bound the fire between gen 2's first key and
idle; `:9943` through `:9946` require its surface key and the latest
registration's surface handle/cap. `:9965` selects the next WM wake,
and `:9966` requires it before idle with `c1g2f`. An independent control
retaining the request fire but removing only the ready bit returns the
specific gen-2 surface-readiness error. Gen 2's own fire is not required
to have `parked=1`, consistent with ruling (A).

## Validation and limits

Read the full pinned oracle, fixture and all three upstream CP-61 tests,
the new commit's diff, and the relay-410 RETURN. Extracted the exact
pinned function and fixture/tests into a temporary Rust harness, keeping
the upstream `super::` qualification by wrapping tests in a module.
Added three independent controls: old fixture without Arthur refused;
required tick wake retained with no request refused; request present
but gen-2 ready bit absent refused. `rustc --edition 2024 --test`:
**6 tests passed**, including all three upstream tests and their mutation
cases. Compared the gen-2 request block with parent `d641726d`; unchanged.
No full workspace build or QEMU boot was run. This is the requested
narrow source/oracle re-check at `d276e746`.
