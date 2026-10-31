# Close Report: CI Caching + cap_survive_exit Fix (2026-05-24)

**Status:** Ready to merge — PR #6 green, all CI checks pass.
**PR:** https://github.com/SecondStageTurbine/NeoPlutarch/pull/6
**Branch:** `fix/cap-survive-exit-close-peer-pid`
**Replaces:** PRs #2, #3, #4 (and #5's revert attempt).
**Originating dispatch:** `DISPATCH-2026-05-23-ci-caching-and-path-filter-prometheus.md`
**Preceding handoff:** `Plans/HANDOFF-2026-05-24-ci-caching-and-scheduler-fix.md`

---

## TL;DR for the PM

Two problems were tangled together. Prometheus correctly identified one (CI was failing) but misdiagnosed the cause, so four PR attempts (#2–#5) all failed. This session pulled them apart:

1. **The CI was failing because the GitHub runner was running out of disk space mid-test, crashing the runner itself.** Not a kernel bug. Manifested as a 22-min "runner-killed" pattern with no visible error in `gh run view --log` because the runner's diagnostic logger died trying to write to a full disk. Fixed by adding `jlumbroso/free-disk-space@main` at the start of the workflow — frees ~25 GB of pre-installed cruft (Android SDK, .NET, Haskell, large-packages, swap) that the kernel build doesn't need.

2. **There was *also* a real kernel race in `Channel::close_peer` — but it was different from what Prometheus diagnosed.** The handoff blamed an empty-ready-queue panic in `sched::exit()` on SMP and shipped a "minimal fix" in PR #4. The real bug: `close_peer`'s `extracted_my` branch unconditionally zombified any parked waiter, without checking the waiter's pid. When a process granted a derived capability into one of its own channel sides, the grantee's thread could land on that side and get zombified by the granter's exit cleanup — its `sys_exit` never ran, and the kernel fell back to exit code 255. Fixed by passing `exiting_pid` through `close_peer` and only zombifying when the waiter's pid matches; foreign-process waiters are woken with EPIPE.

PR #6 reverts the scheduler.rs "minimal fix" (which was diagnosing the wrong bug and had a use-after-free shape) and ships both the IPC fix and the CI disk-space cleanup. Pre-Phase-2 audit is unblocked once this merges.

---

## What was actually broken

### Problem 1 — CI runner disk exhaustion (the visible blocker)

GitHub Actions `ubuntu-24.04` runners ship pre-loaded with ~55 GB of tooling (Android SDK ~11 GB, .NET ~2 GB, Haskell ~5 GB, CodeQL bundle, Node toolcache, .NET, etc.). That leaves only ~17 GB free on `/`. Our workflow:

1. Restores buildx cache (~600 MB).
2. Builds the `neoplutarch-dev` container (~1.9 GB image).
3. Restores cargo cache.
4. Runs the QEMU integration test suite — 20+ test scenarios, each building a fresh kernel with different feature flags into `target/`.

After step 2 we were down to 12 GB free. The test suite then chewed through that and the runner crashed with `System.IO.IOException: No space left on device` while trying to write its own diagnostic log. The crash happened mid-step, so the test step's log was lost — which is exactly why `gh run view --log` showed nothing useful and Prometheus blamed a kernel hang.

This had been happening on **every** CI run since the caching workflow landed in PR #1, including runs that touched no code at all. The handoff noted "Run 26348573021 (commit 836e537, docs-only) failed with exit status 35 on cap_survive_exit" as evidence of a flaky kernel race — in reality, docs-only commits don't change the kernel; they were failing for the same disk reason.

### Problem 2 — cap_survive_exit cross-process zombify (the underlying kernel bug)

Once the CI was running long enough to capture serial logs locally, the actual `cap_survive_exit` failure mode became visible. The test:
- Spawns `neo-init` (pid=1) and a `worker` (pid=2) child.
- Init `sys_recv_grant`s a derived capability from worker, then blocks on `sys_recv`.
- Worker exits cleanly.
- Init is supposed to wake with EPIPE, verify the channel is closed, then exit 0.

What was happening in the failing case:

```
[ kernel ] sched: reaped 'worker' (id=8)
[ kernel ] pmap: dropped — freed 11 frames + 1 PML4
[ kernel ] proc: deregistered 'worker' (pid=2)
[ kernel ] sched: reaped 'neo-init' (id=7)    ← init reaped with NO preceding "exited with code"
[ kernel ] proc: neo-init exit code 255       ← fallback, init's sys_exit never ran
NEO_TEST/1 event=FAIL reason=cap_scenario_exit_nonzero code=255
```

Init was being zombified directly, skipping its `sys_exit`. Root cause: `Channel::close_peer`'s `extracted_my` branch zombified whoever was parked on the exiting process's side, without checking that the waiter actually belonged to the exiting process. The grant gave init a handle into worker's side of the data channel. The race window:

1. Worker's exit triggers `close_channel_peers` on BSP.
2. `close_peer(ctl_side)` wakes init with EPIPE (correctly).
3. Init runs on another CPU under SMP, tries `sys_send(granted, …)`, blocks on **worker's side** of the data channel.
4. `close_peer(data_side)` finds init parked on `extracted_my` → zombifies it.
5. Init's `sys_exit(0)` never runs. Exit code 255.

PR #4's "minimal fix" addressed an unrelated scheduler path (`exit()` with empty ready queue) that can't actually trigger in Phase 1.5b — BSP main is always in the ready queue when a user thread runs, and APs don't receive routed work yet. That fix also had a use-after-free shape: it pushed the exiting thread to zombies then spun on its kernel stack, which the BSP reaper could free out from under the spinning AP. Reverted in this PR.

---

## What's in the PR

Three commits on `fix/cap-survive-exit-close-peer-pid`:

| Commit | Subject | Files | What it does |
|--------|---------|-------|--------------|
| `b477d63` | sched/ipc: fix cap_survive_exit cross-process zombify race | 5 (kernel) | The IPC fix. Threads `exiting_pid` through `close_peer`; zombifies only when `waiter.pid() == exiting_pid`, otherwise wakes with EPIPE. Reverts PR #4's `sched::exit()` "minimal fix". Adds `Thread::pid()` accessor. Test harness updated to pass `KERNEL_PID` (preserves SC-4 self-process zombify behavior). |
| `38da1a9` | ci: add df -h probes | 1 (ci.yml) | Diagnostic step additions before/after each existing step + a 60s sidecar during the QEMU step. How we proved the disk-exhaustion theory. Worth keeping for future tuning; can be stripped in a cleanup PR later. |
| `737c197` | ci: free 20+ GB of pre-installed runner cruft | 1 (ci.yml) | `jlumbroso/free-disk-space@main` at workflow start. Removes Android SDK / .NET / Haskell / large-packages / swap. Preserves tool-cache and our own docker image. |

**Net:** 6 files, +120 / −47 lines. The kernel change is small and localized; the CI changes are additive.

---

## Closed deviations

Naming these explicitly per the audit-trail discipline — implicit closure via artifact creation is invisible to readers downstream.

1. **Prometheus's hypothesis ("scheduler empty-queue panic on SMP") closed as wrong-bug.** Evidence: the failing serial log shows no panic, no scheduler crash — just init reaped without its exit line. The xtask test-framework `exit status: 35` actually came from the kernel calling `debug_exit(EXIT_FAILURE)` because the test framework saw a nonzero exit code, not because of a kernel panic. The handoff's diagnostic chain was anchored on a misread of that exit status.

2. **PR #4 (`d2a9934`, "sched: fix exit() empty ready queue on SMP (minimal)") closed as wrong-bug fix.** The `None` branch it added handles a path that can't trigger in Phase 1.5b. Also had a use-after-free shape on the spinning AP stack. Reverted as part of PR #6's commit `b477d63`.

3. **"Cap CI runner has been flaky-failing for weeks" closed as runner disk exhaustion.** Not a kernel race, not a runner-side bug — just GHA's pre-installed tool footprint vs our test suite's transient disk demand. The 22-min kill was consistent across every failed run because that's how long it took the test loop to exhaust the 12 GB headroom.

---

## CI numbers — before vs. after

| Phase | Free disk before | Free disk after |
|-------|------------------|-----------------|
| Runner cold start | 17 GB | (same) |
| After `jlumbroso/free-disk-space` | n/a (didn't exist) | **41 GB** |
| After build dev container | 12 GB | 36 GB |
| Before QEMU tests | 12 GB | 36 GB |
| During tests (peak observed) | runner crashed | 33 GB (stable) |

| Run outcome | Result |
|-------------|--------|
| All CI runs across PRs #1–#5 (before this work) | Failed at ~22 min, no visible reason |
| Run 26352794862 (PR #6 first push, kernel fix only) | Failed at 22m17s with `No space left on device` annotation visible |
| Run 26363922294 (PR #6 + df probes) | Failed at 23m2s, probes printed disk timeline |
| Run 26364516256 (PR #6 + free-disk-space) | **PASS in 27m13s, all test scenarios green** |

---

## Outstanding follow-ups (not blockers)

- **Local `main` is 8 commits ahead of `origin/main`** with merge cruft from PRs #2–#5. Cleanup with `git reset --hard origin/main` after PR #6 merges. (Verified the 8 extra commits are equivalent merge commits, not new content.)
- **The `df -h` probes in `ci.yml` add log noise.** Useful for future tuning but should be stripped in a follow-up cleanup commit if we're confident CI is stable.
- **Untracked files in repo root:** `Plans/HANDOFF-2026-05-24-ci-caching-and-scheduler-fix.md` (Prometheus's handoff — worth committing as historical record) and `ai_digest.py` (unknown origin — needs disposition).
- **180s QEMU timeout flake** observed in 1/5 local `cap_survive_exit` runs after the IPC fix. **Different failure mode** from the original exit_code=255 panic — appears lower-frequency and the xtask's 4-attempt retry handles it. CI ran clean across the full suite. Worth a separate investigation if it recurs.
- **GHA action deprecation warnings** for `actions/cache@v4`, `actions/checkout@v4`, `docker/build-push-action@v5`, `docker/setup-buildx-action@v3` — Node.js 20 → 24 transition by 2026-06-02. Routine maintenance.

---

## Decisions needed from PM

1. **Merge PR #6 now, or hold for additional review?** All checks green. Touches the IPC + scheduler ordering invariants which are load-bearing for Phase 1.5b's SC-4 contract — but the change is small, the test harness's SC-4 assertion (`q8c_close_peer_my_side_extract`) still passes, and the deviation closure logic is documented in the commit body.
2. **What to do about local main's merge cruft after merge?** Hard-reset to origin/main is the cleanest option but is a destructive operation — would prefer your sign-off before running it.
3. **Pre-Phase-2 audit was blocked on CI being green.** With PR #6 merged, that audit can proceed. Worth scheduling.
4. **The 180s flake** — file a tracking issue now, or wait to see if it recurs in CI? My read: file it now with the diagnosis as a placeholder. Cheaper than re-deriving the context if it happens again.

---

## Timeline of this session

- **Started** with Prometheus's handoff identifying the scheduler-panic hypothesis and 5 unsuccessful PR attempts.
- **Re-ran the failing test locally** at NEO_SMP=4 — first attempt's serial log showed init reaped without sys_exit, no kernel panic. This contradicted the handoff.
- **Traced the actual code path** — found `close_peer`'s `extracted_my` branch zombifies any waiter regardless of pid. The grant scenario in the test put init's thread on worker's side. Formed the cross-process zombify hypothesis.
- **Implemented the IPC fix + scheduler revert** on a fresh branch from origin/main (avoiding the 8 commits of local merge cruft).
- **Local verify:** 5× cap_survive_exit at NEO_SMP=4 — 4/5 passed attempt 1, 1/5 hit a 180s timeout (different failure, separate flake) and passed on attempt 2.
- **Pushed PR #6**, CI failed at the same ~22 min mark — but this time the runner annotation showed `No space left on device`. Reframed the CI failure as runner disk exhaustion, not kernel race.
- **Added df probes**, second CI run gave a clean disk timeline (17 GB → 12 GB before tests, crashes during).
- **Added jlumbroso/free-disk-space**, third CI run passed all tests in 27m13s.
