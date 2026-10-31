# SC-3B Sub-step 5c verification

Date: 2026-05-10

Substrate:
- Dispatch substrate: `2d8f65da77b4ea10f3d53226724e9381961c48c0`
- BUILD commit: `e2b3f4f15a449ea828c1329e4182413cf6345a3a`
- Subject: `build-3 sub-step 5c: enforce irq-safe acquisition for Channel.state`
- Backup after commit: `Z:\AltimitNeo\2026-05-10-121704`
- Current HEAD after long gate: `cca6889` (`docs: project rename AltimitNeo -> NeoPlutarch (bible v1.3.0)`)

Concurrent repository movement:
- While the long closure gate was running, three docs-only commits landed above `e2b3f4f`.
- Diff from `e2b3f4f` to final HEAD touched only `README.md` and `docs/**`; no kernel, userland, or xtask source files changed.
- The SC-3B Sub-step 5c code commit remains in history below those docs commits.

## Implementation summary

Sub-step 5c enforced IRQ-safe acquisition discipline for `Channel.state` in `kernel/src/ipc/mod.rs`.

Changes:
- Wrapped `fail_recv_grant` in `without_interrupts`.
- Wrapped `debug_endpoint_is_grant_blocked` in `without_interrupts`.
- Wrapped `debug_endpoint_is_send_blocked` in `without_interrupts`.
- Wrapped `debug_endpoint_is_recv_blocked` in `without_interrupts`.
- Wrapped `close_peer` in `without_interrupts`.
- Added call-site comments on existing IPC fastpaths that were already IRQ-off by caller.
- Did not change `SwitchMutex`.
- Did not change scheduler code.
- Did not retain instrumentation.

## Channel.state audit

All `Channel.state` acquisitions found by:

```powershell
rg -n "state\.(lock|lock_at|try_lock|try_lock_at)\(" kernel/src/ipc kernel/src/test_harness.rs
```

Classification:

| Caller | Classification | Action |
| --- | --- | --- |
| `Channel::send` | (a) already IRQ-off by syscall/test caller | Added discipline comment only. |
| `Channel::recv` | (a) already IRQ-off by syscall/test caller | Added discipline comment only. |
| `Channel::grant` | (a) already IRQ-off by syscall/test caller | Added discipline comment only. |
| `Channel::recv_grant` | (a) already IRQ-off by syscall/test caller | Added discipline comment only. |
| `Channel::fail_recv_grant` | (b) preemptible acquisition | Wrapped in `without_interrupts`. |
| `Channel::debug_endpoint_is_grant_blocked` | (b) preemptible acquisition | Wrapped in `without_interrupts`. |
| `Channel::debug_endpoint_is_send_blocked` | (b) preemptible acquisition | Wrapped in `without_interrupts`. |
| `Channel::debug_endpoint_is_recv_blocked` | (b) preemptible acquisition | Wrapped in `without_interrupts`. |
| `Channel::close_peer` | (b) preemptible acquisition | Wrapped in `without_interrupts`; wake happens after guard drop. |

Category (c), preemptible-by-design, is empty.

## Pre-commit verification

`docker compose run --rm neo cargo k`
- Result: PASS
- Warning count: 18
- Baseline: 18
- Delta: 0
- New unsafe-op-in-unsafe-fn warnings: 0

`docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all`
- Result: PASS
- Serial logs observed: 34
- PASS terminal frames: 33
- Expected meta-test FAIL frame: 1

Expected meta-test frame:

```text
NEO_TEST/1 event=FAIL name=expect_pass_but_fail reason=deliberate_harness_failure
```

Sibling SMP scenarios at `NEO_SMP=4`:

```text
NEO_TEST/1 event=PASS name=smp_block_release_smoke blocked_cpu=0 completer_cpu=1 resumed_cpu=1 release_path=asm
NEO_TEST/1 event=PASS name=smp_send_blocks_first_then_completes blocked_cpu=0 completer_cpu=1 resumed_cpu=1
NEO_TEST/1 event=PASS name=smp_recv_blocks_first_then_completes blocked_cpu=0 completer_cpu=1 resumed_cpu=1
NEO_TEST/1 event=PASS name=smp_grant_cross_cpu blocked_cpu=0 completer_cpu=1 resumed_cpu=1 grant_rights=0x101 handle_idx=0
NEO_TEST/1 event=PASS name=smp_cross_cpu_migration_reap blocked_cpu=0 resumed_cpu=1 exit_reaped=ok
```

Single stress runs:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=2
```

Five consecutive `NEO_SMP=4` stress runs before commit:

| Run | Result | Elapsed seconds |
| --- | --- | --- |
| 1 | PASS | 73.3 |
| 2 | PASS | 68.9 |
| 3 | PASS | 76.3 |
| 4 | PASS | 67.4 |
| 5 | PASS | 65.1 |

## Post-commit backup

Command:

```powershell
powershell -ExecutionPolicy Bypass -File "Plans\backup-to-synology.ps1"
```

Result:
- PASS
- Backup path: `Z:\AltimitNeo\2026-05-10-121704`
- Backed-up commit: `e2b3f4f15a449ea828c1329e4182413cf6345a3a`
- Stashes preserved: 4

## Closure gate: NEO_SMP=4

Command shape:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress
```

Artifact directory:

```text
target/verify/sc3b-sub-step-5c-gate/neo-smp4/
```

Scored result:
- PASS: 200
- FAIL/PANIC/MISSING_TERMINAL_FRAME scored failures: 0
- Timing minimum: 62.8s
- Timing maximum: 510.9s
- Timing average: 112.9065s

Representative terminal frame:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

Artifact note:
- Raw `results.csv` contains one unscored `run=82 exit_code=137 classification=NO_SERIAL` row.
- Cause: the host command timed out after run 81, leaving an orphan `docker compose run` for run 82. I stopped that orphan before it reached a serial terminal frame and reran run 82 cleanly.
- The rerun of run 82 passed and is the scored gate attempt.
- Final scored count is 200 completed PASS boots.

## Closure gate: NEO_SMP=2

Command shape:

```powershell
docker compose run --rm -e NEO_SMP=2 neo cargo neo-test smp_cross_cpu_rendezvous_stress
```

Artifact directory:

```text
target/verify/sc3b-sub-step-5c-gate/neo-smp2/
```

Scored result:
- PASS: 25
- FAIL/PANIC/MISSING_TERMINAL_FRAME scored failures: 0
- Timing minimum: 78.4s
- Timing maximum: 88.3s
- Timing average: 82.356s

Representative terminal frame:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=2
```

## Final cargo check

`docker compose run --rm neo cargo k`
- Result: PASS at final HEAD `cca6889`
- Warning count: 18
- Delta against baseline: 0

## State after verification

Recent history:

```text
cca6889 docs: project rename AltimitNeo -> NeoPlutarch (bible v1.3.0)
57439a6 docs(adr): ADR-017 - WiFi support strategy and phase-binding
04f4a95 docs(bible): v1.2.0 - Article 9 expansion + Article 28 (Product Accessibility) + WiFi Phase 7.5 commitment
e2b3f4f build-3 sub-step 5c: enforce irq-safe acquisition for Channel.state
```

Tracked source status:
- No tracked source changes from this verification work after the code commit.
- Untracked `Plans/**`, `.claude/`, and `tools/wedge-investigation/` files remain from ongoing planning/investigation work.

Stashes preserved:

```text
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

Outcome: SC-3B Sub-step 5c gate PASS. The Hpw mechanism's necessary condition, mixed preemptible/IRQ-off acquisition of `Channel.state`, is removed for all audited call sites.
