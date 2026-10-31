# Phase 3 BUILD-1 Sub-Commit 1B0 Runtime Addendum

Date: 2026-05-12
Lane: Jarvis runtime gate
Branch: codex/phase-3-build-1-jarvis
Commit under test: 6402986
Dispatch: DISPATCH-2026-05-12-phase-3-build-1-1b0-runtime-verification-jarvis.md

## Verdict

PASS. Sub-commit 1B0 is runtime-ratified in the Docker dev container with Limine available at `/home/neo/limine`.

No original 1B/1C/1D storage I/O work was resumed during this gate.

## Environment

- Host path: `E:\AI Automation\AltimitNeo`
- Container path: `/workspace`
- Docker command prefix: `docker compose run --rm neo`
- SMP setting: `NEO_SMP=4`
- Full-regression host log: `target\neo-test\phase3_1b0_cargo_neo_test_all.host.log`

The `NEO_SMP=4` setting was required for the cross-CPU rendezvous stress gate to exercise the SMP path instead of failing its scenario precondition.

## Targeted Runtime Gate

All targeted commands returned exit code 0:

| Command | Result | Serial log |
| --- | --- | --- |
| `docker compose build` | PASS | N/A |
| `docker compose run --rm neo cargo neo-test phase3_sched_wake_token_round_trip` | PASS | `target\neo-test\phase3_sched_wake_token_round_trip.serial.log` |
| `docker compose run --rm neo cargo neo-test phase3_sched_wake_token_abandon` | PASS | `target\neo-test\phase3_sched_wake_token_abandon.serial.log` |
| `docker compose run --rm neo cargo neo-test phase3_sched_wake_token_stale_generation` | PASS | `target\neo-test\phase3_sched_wake_token_stale_generation.serial.log` |
| `docker compose run --rm neo cargo neo-test smp_cross_cpu_rendezvous_stress` | PASS | `target\neo-test\smp_cross_cpu_rendezvous_stress.serial.log` |
| `docker compose run --rm neo cargo neo-test q8c_blocked_kill_walk` | PASS | `target\neo-test\q8c_blocked_kill_walk.serial.log` |
| `docker compose run --rm neo cargo neo-test q8c_close_peer_my_side_extract` | PASS | `target\neo-test\q8c_close_peer_my_side_extract.serial.log` |
| `docker compose run --rm neo cargo neo-test cap_grant_basic` | PASS | `target\neo-test\cap_grant_basic.serial.log` |
| `docker compose run --rm neo cargo neo-test cap_grant_missing_meta` | PASS | `target\neo-test\cap_grant_missing_meta.serial.log` |
| `docker compose run --rm neo cargo neo-test cap_revoke_local` | PASS | `target\neo-test\cap_revoke_local.serial.log` |
| `docker compose run --rm neo cargo neo-test cap_revoke_cross_process` | PASS | `target\neo-test\cap_revoke_cross_process.serial.log` |
| `docker compose run --rm neo cargo neo-test cap_revoke_descendants_walk` | PASS | `target\neo-test\cap_revoke_descendants_walk.serial.log` |

Note: the standalone `cap_revoke_descendants_walk` targeted run reported `PASS after retry attempt 2/2`; the later full-regression run reported a normal PASS for the same scenario.

## Serial Evidence

Current serial logs contain these PASS lines:

```text
NEO_TEST/1 event=PASS name=phase3_sched_wake_token_round_trip token_cpu=0 token_slot=0 generation=1 return=0x1b0
NEO_TEST/1 event=PASS name=phase3_sched_wake_token_abandon token_cpu=0 token_slot=0 generation=1 late_wake=Gone abandon=zombified_and_reaped
NEO_TEST/1 event=PASS name=phase3_sched_wake_token_stale_generation slot=0 old_generation=1 new_generation=2 stale_result=Stale
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
NEO_TEST/1 event=PASS name=q8c_blocked_kill_walk mechanism=q8c_blocked_kill_walk detection=parked_thread_zombified channel_closed_both_sides=true frame_audit_clean=true
NEO_TEST/1 event=PASS name=q8c_close_peer_my_side_extract mechanism=close_peer_my_side_extract detection=parked_thread_zombified channel_closed_both_sides=true frame_audit_clean=true
NEO_TEST/1 event=PASS name=cap_grant_basic cap_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=cap_grant_missing_meta cap_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=cap_revoke_local cap_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=cap_revoke_cross_process cap_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=cap_revoke_descendants_walk cap_scenario=ok exit_code=0
```

## Full Regression Attempt

Command:

```powershell
$env:NEO_SMP='4'; docker compose run --rm neo cargo neo-test all
```

Result: PASS, exit code 0.

The full-regression host log includes PASS entries for all normal scenarios, including:

```text
[xtask] neo-test: `smp_cross_cpu_rendezvous_stress` PASS
[xtask] neo-test: `phase3_pci_virtio_blk_probe_smoke` PASS
[xtask] neo-test: `phase3_sched_wake_token_round_trip` PASS
[xtask] neo-test: `phase3_sched_wake_token_abandon` PASS
[xtask] neo-test: `phase3_sched_wake_token_stale_generation` PASS
[xtask] neo-test meta: deliberate failure was detected
```

The final meta line is expected: the runner verified that `expect_pass_but_fail` is detected as a failing test.

## Closure

1B0 is runtime-ratified. Original 1B may resume only after PM authorization, with `IoRendezvous.waiting_thread: Option<ThreadWakeToken>` as the wake substrate.
