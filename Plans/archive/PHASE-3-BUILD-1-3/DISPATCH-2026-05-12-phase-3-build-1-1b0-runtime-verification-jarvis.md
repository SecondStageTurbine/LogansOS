# Dispatch: Phase 3 BUILD-1 1B0 Runtime Verification

Date: 2026-05-12
Recipient: Jarvis
Branch: `codex/phase-3-build-1-jarvis`
Priority: Hard gate before original 1B resumes

## PM Decision

1B0 is accepted as build-passing and structurally responsive to the BUILD-1 1B scheduler lifetime blocker.

1B0 is not yet ratified as a runtime PASS. The original 1B virtio-blk read/write wiring remains held. Do not proceed into 1B, 1C, or 1D until this runtime gate closes.

This is not a design blocker. It is an environment/runtime verification gate: the Windows host lacks `/home/neo/limine`, but the repository's Docker dev environment is expected to provide it.

## Required Runtime Lane

Run the gate inside the dev container, not directly on the Windows host:

```powershell
docker compose build
docker compose run --rm neo cargo neo-test phase3_sched_wake_token_round_trip
docker compose run --rm neo cargo neo-test phase3_sched_wake_token_abandon
docker compose run --rm neo cargo neo-test phase3_sched_wake_token_stale_generation
docker compose run --rm neo cargo neo-test smp_cross_cpu_rendezvous_stress
docker compose run --rm neo cargo neo-test q8c_blocked_kill_walk
docker compose run --rm neo cargo neo-test q8c_close_peer_my_side_extract
docker compose run --rm neo cargo neo-test cap_grant_basic
docker compose run --rm neo cargo neo-test cap_grant_missing_meta
docker compose run --rm neo cargo neo-test cap_revoke_local
docker compose run --rm neo cargo neo-test cap_revoke_cross_process
docker compose run --rm neo cargo neo-test cap_revoke_descendants_walk
docker compose run --rm neo cargo neo-test all
```

The first eleven scenario commands are the 1B0 ratification gate. `cargo neo-test all` is required as a full-regression attempt; if it fails after the targeted lane passes, classify whether the failure is introduced by 1B0 or is pre-existing/unrelated.

## Acceptance Criteria

- The three new wake-token scenarios runtime-pass under QEMU.
- The listed scheduler, blocked-kill, IPC/capability, and revoke regressions runtime-pass under QEMU.
- No parked thread can be resurrected by a late/stale wake token.
- Process-exit cleanup drains token-parked scheduler-owned `Box<Thread>` storage.
- No `Arc<Thread>` migration is introduced.
- No original 1B virtio-blk I/O wiring resumes until this dispatch closes PASS.

## Reporting Protocol

On PASS:

- Append runtime results to `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-VERIFICATION-2026-05-12.md`, or create `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-RUNTIME-ADDENDUM-2026-05-12.md`.
- Include serial-log paths for the three wake-token scenarios.
- Report that PM may authorize resuming original 1B with `IoRendezvous.waiting_thread: Option<ThreadWakeToken>`.

On runtime FAIL:

- Stop.
- File `Plans/FAILURE-2026-05-12-phase-3-build-1-1b0-runtime-jarvis.md`.
- Include the failing scenario, expected invariant, observed serial log, and smallest suspected source area.

On environment BLOCK:

- Stop.
- File `Plans/BLOCKER-2026-05-12-phase-3-build-1-runtime-env.md`.
- Include the exact Docker/Limine/QEMU failure and the command that produced it.

## PM Posture

The 1B0 code path is promising, but the abandoned-I/O lifetime gate is inherently runtime-sensitive. Build PASS is enough to preserve the branch; it is not enough to reopen storage I/O implementation.
