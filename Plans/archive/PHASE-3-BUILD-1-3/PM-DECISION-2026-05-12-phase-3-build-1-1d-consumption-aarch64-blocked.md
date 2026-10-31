# PM Decision: BUILD-1 1D Consumption and aarch64 Blocked Disposition

Date: 2026-05-12
Authority: PM
Inputs:

- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md`
- `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`
- `Plans/PREP-2026-05-12-phase-3-build-1-closure-prometheus.md`
- `Plans/RESEARCH-2026-05-12-phase-3-build-1-aarch64-lane-feasibility-cerebro.md`
- `Plans/PM-DECISION-2026-05-12-phase-3-build-1-prometheus-cerebro-consumption-and-aarch64-gate.md`

## Decision

BUILD-1 sub-commit 1D is accepted as committed and verified for x86_64 integration.

Commit:

- `ccdacda7d355138873380e49007eac22d545e677`
- `kernel/storage: add Phase 3 BUILD-1 integration gates`

Accepted 1D result:

```text
x86 integration PASS / BUILD-1 closure BLOCKED on aarch64 lane
```

BUILD-1 is not closed. No close report should be produced yet. No BUILD-2 NeoFS work is authorized.

## Ratified PASS Evidence

The following 1D evidence is accepted:

- `cargo k` PASS.
- `cargo k --no-default-features` PASS.
- `phase3_block_vfs_smoke` PASS.
- `phase3_storage_completion_cross_cpu_wake` PASS with real virtio used-ring completion path and `ThreadWakeToken` wake path instrumentation.
- `phase3_build1_reboot_persistence_smoke` PASS with two-boot reuse of `target/neo-test/phase3_build1_reboot_persistence_smoke.raw`.
- `phase3_virtio_blk_raw_rw_smoke` PASS.
- `phase3_virtio_blk_abandoned_io_no_leak` PASS rerun.
- 1C VFS scenarios retained PASS evidence, including stdout namespace and type-vs-rights distinction.
- `smp_cross_cpu_rendezvous_stress` PASS under `NEO_SMP=4`.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` PASS.
- `git diff --check` PASS.

The explicit `cargo neo-test all` stop at `irqsafe_mutex_bench` is accepted as the known timing-sensitive benchmark threshold miss, not a storage/VFS regression, because the non-explicit SMP suite and targeted gates passed.

`cargo fmt --check` remains blocked on host and Docker because pinned nightly rustfmt is missing. This remains a non-blocking tooling gap.

## Ratified Blocker

The aarch64 blocker artifact is accepted.

Ratified blocker path:

- `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`

The blocker correctly states:

- current repo lacks enough aarch64 kernel, boot, interrupt, storage transport, QEMU runner, and CI substrate;
- compile-only aarch64 probing fails on x86-coupled code paths;
- even a successful compile-only probe would not be barrier-correctness evidence;
- x86_64 testing cannot expose the descriptor-publish and notification-ordering class ADR-021 calls out;
- BUILD-1 closure remains blocked until a real aarch64 barrier-exposure lane passes or PM explicitly changes the ADR/closure gate.

## Current Project State

The branch `codex/phase-3-build-1-jarvis` now contains the full x86_64 BUILD-1 implementation series:

- 1A block substrate / DMA / PCI / probe
- 1B0 scheduler-owned wake-token substrate
- 1B virtio-blk raw read/write
- 1C minimal VFS/testfs/file syscalls
- 1D x86 integration gates and aarch64 blocker

This branch is not ready to merge as "BUILD-1 closed" because the aarch64 acceptance gate remains open.

## Instruction To Prometheus

Prepare a blocked-disposition note, not a close report.

Required output:

- `Plans/BLOCKED-DISPOSITION-2026-05-12-phase-3-build-1-aarch64.md`

Scope:

- summarize the committed BUILD-1 x86_64 implementation state through `ccdacda`;
- list the accepted verification artifacts from 1A, 1B0, 1B, 1C, and 1D;
- state that x86_64 integration is PASS;
- state that BUILD-1 closure is BLOCKED on the aarch64 barrier-exposure lane;
- include the exact blocker path and one-paragraph technical reason;
- explicitly say no close report exists and none should be created yet;
- explicitly say no BUILD-2 NeoFS work is authorized;
- do not edit ADR-021, Bible, README, phase docs, or indexes.

## Instruction To Jarvis

No additional 1D work is requested in the current dispatch.

Stand by for a separate PM decision on the aarch64 implementation lane.

Do not begin BUILD-2.

## Instruction To Cerebro

No further immediate research is required.

Cerebro's aarch64 feasibility artifact remains the technical brief for a future implementation dispatch.

## Next PM Decision Needed

PM must decide whether to:

1. authorize a dedicated aarch64 substrate/runtime implementation lane to unblock BUILD-1 closure; or
2. leave BUILD-1 open with the blocked disposition while scheduling the aarch64 lane as the next critical-path work.

Until that decision, the correct external status is:

```text
Phase 3 BUILD-1 x86_64 implementation complete; BUILD-1 closure blocked on aarch64 barrier-exposure lane.
```
