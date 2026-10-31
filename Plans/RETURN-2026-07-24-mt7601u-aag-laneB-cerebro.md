<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: MT7601U candidate `00b45d4` AAG — Lane B (Cerebro)

**Date:** 2026-07-24
**From:** Cerebro
**To:** PM
**Review dispatch:** `Plans/REVIEW-DISPATCH-2026-07-24-mt7601u-candidate-00b45d4-aag.md`
**Candidate:** `00b45d418428680b0ede8d727664df27cda8c6ab`
**Branch reviewed:** `codex/mt7601u-aag-bound-dhcp-jarvis`
**Frozen base:** `33030355499fb7d270e42dd2955e12c1d1012437`
**Lane:** B — contract binding, sink, and terminal truth

## Verdict

**BOUNCE.**

One finding, on Lane B's own TOKEN-OK criterion: *"Return `TOKEN-OK` only if the hostile
oracle is **production-bound** and both evidence and LINK-UP truth are structural."*

Evidence and LINK-UP truth **are** structural — verified independently below, and they are
good work. The hostile oracle is **not bound**.

This is not a claim that the production path is wrong. My independent read of `mod.rs` says
the fail-closed structure is sound. It is a claim that the candidate's own verification does
not demonstrate that, and Lane B's gate is written about binding.

## Review method

Isolated detached worktree at the exact remote SHA; immutable commit reads; device-free
tests only. Jarvis's mutable working tree (`Workers/jarvis`) was not touched, read, or
relied on. No edits to production or test code, no PR, no merge, no `workflow_dispatch`, no
ISO build, no hardware action.

```
git fetch origin
git worktree add --detach <isolated> 00b45d418428680b0ede8d727664df27cda8c6ab
git show --stat 00b45d418428680b0ede8d727664df27cda8c6ab
git diff 33030355499fb7d270e42dd2955e12c1d1012437..00b45d418428680b0ede8d727664df27cda8c6ab
docker compose run --rm neo cargo test -p ai-ops-tests
```

Independent verification of the commit itself:

| Check | Result |
|---|---|
| Remote parity | `refs/heads/codex/mt7601u-aag-bound-dhcp-jarvis` = `00b45d41…` ✓ |
| Base relation | `33030355…00b45d4` = 0 behind / 7 ahead ✓ |
| Frozen base ancestry | `33030355` is an ancestor of `origin/main` ✓ |
| Diff scope | 17 files changed, 4926 insertions(+), 382 deletions(-) |
| Device-free tests | `ai-ops-tests` 235 passed + hostile oracle 15 passed, 0 failed |

## The finding — the approved hostile oracle is preserved but inert

```
blob tools/ai-ops-tests/tests/mt7601u_hostile_oracle.rs
  = b47fc6f5f27cb7bb991cfa1a21d37c46a8922645     byte-identical to approved  ✓
grep "impl WedgePolicy"  outside that file       → 0
grep "qualify_policy"    outside that file       → 0
```

The 15 preserved tests pass at the candidate, but they validate the reference model against
the model's own device script. They assert nothing about `Device::scan`. `qualify_policy` —
the binding seam built for exactly this purpose — is never called with a production-backed
policy. The artifact's own module documentation is explicit that this is not candidate
proof:

> Qualifying `ReferencePolicy` exercises **the harness**, not the driver. A candidate is
> qualified only when a `WedgePolicy` impl **backed by the production decision function** is
> passed to `qualify_policy` and reports `qualified()`.

### What was supplied instead splits into two unequal halves

**Genuine, and credited.** `channel_failure_action` is production-called at
`kernel/src/drivers/usb/mt7601u/mod.rs:2238` and genuinely exercised by `run_survey_control`
(`tools/ai-ops-tests/src/lib.rs:2700-2743`), which is a real loop over 14 channels driving
the real decision function. That is a proper lift, and it is what makes closing this bounce
cheap.

**Not genuine.** `terminal_channel_failure_oracle` (`kernel/src/drivers/usb/mt7601u/wedge_logic.rs:244`)
and `rejected_reload_oracle` (`wedge_logic.rs:271`) are **constant-returning functions**.
They match a hand-written trace descriptor against a fixed literal shape and return a struct
whose safety-critical fields are literals:

```rust
// wedge_logic.rs:255-256
attempted_destructive_reload: false,
commands_after_terminal: 0,
```

The test at `tools/ai-ops-tests/src/lib.rs:2780-2792` then asserts those literals equal
themselves. `channel_contract_violations` — the bitmask checker — does have a real mutation
matrix and real teeth; but its *input* is hand-declared, so the composition asserts constants
about constants. The production path could behave arbitrarily and these functions would
return identical values.

The sharpest instance: `run_survey_control` declares `destructive_reloads` at
`tools/ai-ops-tests/src/lib.rs:2697`, initialises it to `0` at `:2708`, and **never
increments it anywhere** — the simulator contains no reload branch at all. The three
assertions that `destructive_reloads == 0` (`:2756`, `:2765`, `:2774`) therefore cannot fail
for any production behaviour whatsoever.

This is the exact gap the approved artifact exists to close: `run_survey` *derives* its trace
from a recorded operation log, so "no traffic after terminal" and "no destructive reload" are
**counted from observed operations**, not declared as struct literals. It is also the failure
mode the project's own byte-oracle rule names — a closed loop that agrees with itself.

### Cost to close: small, and no production change required

The seam already exists and is production-called. Approximately 30 lines in the test file:

1. `impl WedgePolicy` over `channel_failure_action` + `channel_fault_disposition`;
2. map `ChannelFailureAction::ProbeEndpointsAndRetry` → `WedgeAction::ClearEndpointsAndRetry`
   and `ChannelFailureAction::ClassifyCommandPathUnusable` →
   `WedgeAction::ClassifyUnresponsiveAndStop`;
3. honour `disposition.attempt_destructive_reload == false` by never returning
   `WedgeAction::ForceColdReload`;
4. `assert!(qualify_policy(ProductionPolicy::default).qualified());`

## Verified sound — would be TOKEN-OK but for the above

| Lane B item | Finding |
|---|---|
| Accepted model rejects destructive reload in every accepted script | ✓ `forced_reload_is_outside_the_contract_even_when_it_would_succeed` passes at the candidate; `ForcedReloadAfterUnresponsive` is keyed to the first unresponsive observation, so a reload that *wins* the gamble is still RED |
| Early sink installed before any network/USB path | ✓ `kernel/src/main.rs:604+` — `drivers::block::init()` → `test_harness::prepare_mt7601u_log_sink_before_io()` → `drivers::net::init()` → `drivers::usb::init()`; cfg-scoped to the two MT7601U bare-metal scenarios only |
| Sink failure halts before device activity | ✓ `kernel/src/test_harness.rs:1364-1399` — `strict_bare_metal_halt` on no-device, invalid, and install-failure, plus `bare_metal_log_sink_selftest` |
| Channel terminal reaches serial **and** sink | ✓ `serial_println!` → `drivers::console::serial::_print` (`serial.rs:36-45`) tees to `crate::log::write_fmt` → installed `BlockDeviceLogSink`. Sink is live pre-USB, so the bind-time terminal is captured. This is the direct repair of TEST 3's zero-sink / non-empty-serial split |
| One truthful category with complete budget/fence data | ✓ `mod.rs:2357` emits `category=CHANNEL_COMMAND_PATH_UNUSABLE … root_cause=undistinguished`; single-shot via `CHANNEL_TERMINAL` compare-exchange 0→1 (`mod.rs:2353`); consumed / remaining / exhausted / fence_refusals in the same record |
| Association, DHCP, LINK-UP, PASS fail closed | ✓ scan `Err` → `terminalize_channel_failure` → bind returns early (`mod.rs:3977-3979`) → `BOUND`/`PRIMARY` never set → association path stores `AssociationFailed`; `mcu_send` guard (`mod.rs:1367-1370`) returns `Err` and increments `CHANNEL_COMMANDS_BLOCKED_AFTER_TERMINAL` |
| All six DHCP terminals distinguished | ✓ `test_harness.rs:12003-12019` — no-offer (`TimeoutSelecting`), offer-without-ACK (`TimeoutRequesting`), invalid/zero lease (`InvalidLease`), `NetDeviceFailed`, `GtkMissing` (`:12000`), association failure (`:11995`) |
| Human-only log interpretation required to prevent a false PASS | ✓ **Not required.** `test_harness.rs:11986-12025` is fully machine-checked: fence_refusals ≠ 0, not-enumerated, not-bound, not-connected, no-GTK, DHCP ≠ `BoundLinkUp`, and zero-IP ∨ `!link_up` ∨ `!pass_eligible()` each call `fail(...)` before `pass(...)` is reachable. `pass_eligible()` is production-derived via `link_result_logic::pass_eligible` over the live atomics |
| Evidence-neutral terminal preserved (common review contract) | ✓ No causal regression. The only "dead MCU" strings in the diff appear in the preserved oracle's own documentation *rejecting* that framing; 8 added lines carry `undistinguished` / `does not distinguish` / `not …liveness` |

The scan scenario is equally fail-closed: its gate checks `mt7601u::is_bound()`
(`test_harness.rs:11877`), and `BOUND` is only set after `scan()` returns `Ok`, so a channel
terminal cannot yield a scan PASS.

## Secondary observation — not part of the bounce

`mod.rs:2234` hardcodes `endpoint_probes_used: 0`, so production never exercises the
probes-used arm of `channel_failure_action`; only the budget arm is live. The one-probe bound
is real, but it is enforced by **control flow** — a single, non-looped call site — not by the
policy function. That is arguably more robust than a counter, but it should be recorded so
that nobody later assumes the policy is what bounds the retry. `run_survey_control` *does*
exercise the probes-used arm, which is precisely why production and the policy could drift
without a test noticing.

## Scope and limits of this return

- This lane reviewed contract binding, sink ordering, terminal truth, and DHCP/LINK-UP
  fail-closed behaviour only. Transport/response-TD ownership is Lane A; kill-switch,
  accounting, and OFF-artifact reproducibility are Lane C.
- Device-free host tests and read-only source inspection only. Nothing here substitutes for
  the four-shard gate or silicon.
- The proximate cause of the TEST 3 channel-7 timeout remains open. Nothing in this review
  identifies it, and the candidate correctly declines to claim it.

## Re-review

Available on request once `qualify_policy` is called with a production-backed `WedgePolicy`.
Everything else in Lane B's scope passes as reviewed.
