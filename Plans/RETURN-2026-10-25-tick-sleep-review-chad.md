<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #56): tick-sleep post-merge review (#119)

**Overall verdict: PASS for all seven requested checklist items.**

Reviewed SHA: `bbca1c439c3a869d4be2fddcde4c163db8def999`.
Branch: `codex/tick-sleep-review-chad`.
Review only: no source changes, builds, or new test execution.

Read CP-8, CP-14, CP-73 and CP-81, the requested diff from `24ab67a0`, the complete new
sleep module, and the surrounding scheduler/token, context-switch, mutex, timer, caller,
feature and scenario paths. RAG health was healthy (ollama=true, qdrant=true); used semantic
search for discovery and checked the actual worktree sources for conclusions.

## Checklist

| # | Verdict | Enforcing evidence at the reviewed SHA |
|---|---|---|
| 1. No lost wake | **PASS** | `kernel/src/sched/tick_sleep.rs:91` masks local interrupts before acquiring TABLE and checking TICKS (`:93`); installation (`:101`) occurs through `block_current_with_token_and_guard` (`:100`). The scheduler takes its lock at `kernel/src/sched/scheduler.rs:1836`, owns the blocked thread through its token at `:1861`, calls the installer at `:1862`, and releases both guards through the switch at `:1881`. Both asm variants save the old stack and change stacks before unlocking: `kernel/src/arch/x86_64/context.rs:66` through `:69`, and `:146` through `:159`. Expiry takes TABLE at `tick_sleep.rs:127` and wakes the captured token at `:143`. Same-CPU and remote-tick sequences are detailed below. |
| 2. Lock order | **PASS** | TABLE is acquired at `kernel/src/sched/tick_sleep.rs:92`; its only nested scheduler acquisition is the park at `:100`. Expiry's TABLE scope ends at `:141`, before `wake_thread_token` at `:143` takes a scheduler lock (`scheduler.rs:2116`). The selftest's TABLE scan at `tick_sleep.rs:187` only inspects entries. Counters/fast checks are atomic; no new governance-lock acquisition exists. The park/switch path uses existing scheduler/address-space preparation (`scheduler.rs:1852`, `:990`) rather than governance. |
| 3. Fast path | **PASS** | Installation writes the slot and updates both SLEEPING and EARLIEST under TABLE at `kernel/src/sched/tick_sleep.rs:101` through `:103`. Expiry removes due entries and decrements SLEEPING under the same lock (`:131` through `:134`), recomputes the minimum of all survivors (`:136`), and always stores it (`:140`), including MAX when empty. Relaxed fast reads (`:118`, `:122`) are hints, not permission to read slot payloads; the authoritative scan acquires TABLE (`:127`, mutex acquire at `kernel/src/sync/switch_mutex.rs:183`). Concurrent/stale hints can skip a scan, but do not remove a token or permanently suppress later periodic scans on this x86_64 target. |
| 4. Full table / reuse | **PASS** | No free slot returns Park::Full at `kernel/src/sched/tick_sleep.rs:96` through `:97`, before the installer or PARKED increment. `sleep_until` then counts FULL and yields/spins until the original deadline (`:65` through `:70`), installing nothing. Expiry copies the old owner's complete token into its local due array before freeing the slot (`:132`, `:133`). A new install cannot change that copy. Scheduler tokens identify CPU/slot/generation (`scheduler.rs:153`); park increments generation (`:1242`), and wake validates it and takes the thread once (`:1258` through `:1267`). Reusing a sleep-table slot while its previous wake is in flight cannot redirect that wake to the new owner. |
| 5. Feature off | **PASS** | The module is cfg-gated at `kernel/src/sched/mod.rs:26`, the timer hook at `kernel/src/arch/x86_64/timer.rs:376`, and the watchdog additions at `kernel/src/watchdog.rs:348`. USB's new counter/API require both usb-hotplug and tick-sleep (`kernel/src/drivers/usb/mod.rs:298`, `:301`); its increment is gated at `:314`. Both nap implementations preserve the base commit's yield-loop bodies when both sleep features are absent: USB `:331` through `:339`, touchpad `kernel/src/drivers/i2c_hid/touchpad.rs:1323` through `:1329`. Defaults omit tick-sleep (`kernel/Cargo.toml:77`). The named CI check is **Product Feature Build (blocking), cargo build kernel --no-default-features (FXSAVE asm, blocking)** (`.github/workflows/ci.yml:722`, command `:724`); it passed in the cited run. The feature-off desktop matrix and i2c_hid_absent / usb_xhci_select scenarios provide caller-specific coverage as described below. |
| 6. Scenario / control | **PASS** | `kernel/src/test_harness.rs:24873` rejects !parked, early wake, !woken, !past_skips, or slept<20. `:24916` rejects zero parks, any full-table fallback, or any spin count. Selftest observes installation at `kernel/src/sched/tick_sleep.rs:184`, checks early execution at `:208`, completion at `:212`, elapsed ticks at `:214`, and Park::Due for a past deadline at `:218`. The control replaces park with Park::Full (`:111` through `:113`), so no entry is installed and the control cannot pass the parked/past-deadline checks. Feature composition is at `kernel/Cargo.toml:1394` and `:1398`. Existing positive CI PASS was inspected; the control's rejection was checked from source, not newly executed. A never-woken worker fails by the outer QEMU timeout, as clarified below. |
| 7. Stop semantics / callers | **PASS** | Tick-sleep stores only its deadline/token and has no stop-wait registration (`kernel/src/sched/tick_sleep.rs:40`, `:101`); expiry wakes only when deadline<=now (`:131`). `request_stop_control` sets the flag and takes only control.state.stop_waiter (`kernel/src/sched/kthread.rs:326` through `:330`); tick-sleep never populates that field. Therefore stop cannot consume its sleep token or cut the delay short. USB's service is spawned detached (`kernel/src/drivers/usb/mod.rs:285`), sleeps 25 ticks (`:295`, `:316`), and checks stop between passes (`:311`). Touchpad service is also detached (`touchpad.rs:1221`), with one-tick polled naps (`:1290`). Remaining production callers are boot input/evidence polling at USB `:464` and touchpad `:1536`, `:1637`, using ten-tick naps. No caller retains stop/join authority requiring prompt cancellation of these sleeps. |

## Wake and fast-path sequences

- **Own CPU:** with IF off, no local timer ISR can enter between the deadline check and
  installation/switch. If the deadline is already reached, Park::Due returns. Otherwise
  the pending local tick can expire the installed token after the switch releases TABLE.
- **Other CPU:** a tick may advance TICKS after the sleeper's check. If its hints allow a
  scan, it must wait for TABLE; the sleeper installs its token and switches before TABLE
  is released. The scanner then sees the token. Although TABLE is released before the
  scheduler lock, `wake_thread_token` cannot take the parked thread until that scheduler
  lock is released on the new stack. It cannot dispatch an incompletely saved old stack.
- **Remote scan skips during installation:** it can observe SLEEPING=0, or the old later
  EARLIEST, and return. This consumes no token and sets no disabling flag. Installation
  still publishes the count and minimum under TABLE; later ticks retry the hint checks.
  Concurrent expiry/install/recompute operations cannot overwrite a newer minimum outside
  TABLE. A pre-lock `now` sampled before a remote increment may defer expiry until another
  scan, but cannot expire a future deadline early.

The timer increments TICKS at `kernel/src/arch/x86_64/timer.rs:336` and calls expiry before
scheduler preemption at `:377`, `:386`. Remote wakes send a reschedule IPI at
`kernel/src/sched/scheduler.rs:2158`.

## Existing CI evidence and limits

[Run 40174433840](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40174433840)
reports success for all 23 jobs on `eecde97f2d21504604346d8d50d779fffaf4c9dc`. I fetched
that object and verified no diff against the reviewed squash under `kernel`, `tools`, or `.github`.

[QEMU shard 4/4](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40174433840/job/121112629781)
records:

```text
2026-10-25T08:58:38.1591455Z [xtask] neo-test: `usb_hotplug` PASS
```

The tick-sleep-off caller scenarios also passed: `i2c_hid_absent` at
2026-10-25T08:13:09.6028093Z in shard 3/4 (job 121112629726), and `usb_xhci_select` at
2026-10-25T08:34:32.1810208Z in shard 2/4 (job 121112629755). Their feature definitions
(`kernel/Cargo.toml:614`, `:1389`) compile the respective drivers without tick-sleep.
The desktop feature matrix (`.github/workflows/ci.yml:809`) likewise omits tick-sleep and
passed job 121111971779. The no-default-features build completed successfully at
2026-10-25T07:55:57.8121092Z in product-build job 121111971592.

These logs establish the existing positive run, not a newly executed control, full-table
stress test, or deterministic cross-CPU check-to-park race test. Those checklist conclusions
come from the source/ownership analysis above.

Two selftest robustness limitations do not permit a false green for a never-woken worker:

- At `tick_sleep.rs:213`, join is unconditional even when the bounded wait returned
  woken=false. A worker that never wakes therefore hangs this local test instead of reaching
  `tick_sleep_not_parked`. The enclosing xtask invocation passes the scenario budget at
  `tools/xtask/src/main.rs:8815`; its timeout kills QEMU and returns an error at `:11893`
  through `:11914`. The gate still fails, with a timeout diagnostic.
- The early-wake observer tests whether WOKE_AT is nonzero, rather than comparing its value
  with the deadline (`tick_sleep.rs:209`). A parent descheduled at start+17 and resumed only
  after the correctly waking child at start+20 can report woken_early=true. This is a possible
  false red under scheduling delay; the requested rejection conditions remain enforced.
  Comparing the recorded wake tick to the actual sleep deadline would make that check robust.

## Workspace handoff

Only this RETURN is committed. Preexisting untracked graft cache files were preserved in the
named stash `chad preexisting graft cache before relay 56 tick sleep review`. No source fix,
revert, build, or new test run was performed. After push, verify empty output from both
`git status --porcelain` and `git log '@{u}..'`.
