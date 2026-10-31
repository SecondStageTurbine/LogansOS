# Dispatch: Phase 3 BUILD-1 1D - Integration, Closure Gate, aarch64 Decision

Date: 2026-05-12
Recipient: Jarvis
Branch: `codex/phase-3-build-1-jarvis`
Base state: `77d4f29 kernel/fs: add testfs file syscalls`
Priority: BUILD-1 final sub-commit attempt

## PM Decision

BUILD-1 sub-commit 1C is accepted as PASS.

Evidence:

- Commit: `77d4f2966908cb1cb411678e7531bc50fdd9c6e0`
- Artifact: `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1C-VERIFICATION-2026-05-12.md`
- `cargo k` PASS.
- `cargo k --no-default-features` PASS.
- Four 1C VFS Docker scenarios PASS.
- `phase3_virtio_blk_raw_rw_smoke` PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` PASS after isolating the known `irqsafe_mutex_bench` timing miss.
- `git diff --check` PASS.
- `cargo fmt --check` remains environment-blocked on host and Docker because pinned nightly rustfmt is missing.

PM now authorizes BUILD-1 sub-commit 1D.

This is the final BUILD-1 integration/closure lane. It may close BUILD-1 only if every required gate is satisfied, including the aarch64 barrier-exposure lane. If aarch64 cannot be honestly implemented in this repo today, file the formal blocker and do not claim BUILD-1 closed.

## 1D Scope

Implement the BUILD-1 final integration gates:

- block + VFS same-boot smoke;
- storage completion wake composition smoke;
- raw-block reboot persistence smoke;
- final CI/test plumbing for all BUILD-1 scenarios;
- aarch64 lane or formal aarch64 blocker artifact;
- BUILD-1 closure report if and only if closure is honest.

Keep 1D focused on integration and closure evidence. Do not start BUILD-2 NeoFS.

## Required Scenarios

Add or complete:

- `phase3_block_vfs_smoke`
- `phase3_storage_completion_cross_cpu_wake`
- `phase3_build1_reboot_persistence_smoke`

### `phase3_block_vfs_smoke`

Expected evidence:

- QEMU boots with Limine modules still as early root.
- virtio-blk attaches and logs feature policy.
- F_FLUSH is offered and not negotiated.
- CONFIG_WCE is not negotiated.
- raw block read/write smoke passes or is exercised in the same scenario.
- VFS/testfs syscalls pass in the same boot.
- serial output includes both storage and VFS evidence lines.

This test does not need to make testfs disk-backed. BUILD-1 may still use synthetic/in-memory testfs; NeoFS native and disk-root pivot remain BUILD-2+.

### `phase3_storage_completion_cross_cpu_wake`

Expected evidence:

- I/O is issued from a schedulable thread/process path.
- Completion wakes through the 1B0 `ThreadWakeToken` path.
- The test demonstrates cross-CPU wake composition if QEMU IRQ routing permits.
- If deterministic interrupt routing cannot force the actual completion on a different CPU, add instrumentation that proves the SC-3C wake composition path is exercised and explain the limitation in the verification artifact.

Do not fake the gate with a purely local synchronous call that bypasses the storage completion path.

### `phase3_build1_reboot_persistence_smoke`

Expected evidence:

- A deterministic raw disk image is created under `target/neo-test/` or equivalent generated output.
- First QEMU run writes a known pattern through the BUILD-1 virtio-blk path.
- Second QEMU run reuses the same raw disk image and reads the pattern back.
- Serial or host logs tie both runs to the same scenario and image path.
- This is a durability sanity smoke for BUILD-1 write-through mode, not a CoW crash-consistency claim.

## aarch64 Gate

ADR-021 requires an aarch64 lane before BUILD-1 can close because x86 TSO can hide descriptor-ring barrier bugs.

Preferred path:

- add a real aarch64 QEMU/CI lane that builds and runs the relevant virtqueue descriptor-publish/notify path against an emulated virtio-blk device;
- include it in CI if feasible;
- record PASS evidence in the 1D verification artifact.

Acceptable blocker path:

- if NeoPlutarch lacks enough aarch64 boot/runtime substrate to do this honestly in 1D, file:
  `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`
- include:
  - exact missing substrate;
  - smallest viable aarch64 lane proposal;
  - why x86-only tests cannot expose the barrier class;
  - whether any compile-only aarch64 check was attempted;
  - PM decision needed.

If the blocker path is taken, do not claim BUILD-1 closed. The 1D sub-commit may still land integration tests and the blocker artifact, but its verdict must be "x86 integration PASS / BUILD-1 closure BLOCKED on aarch64 lane."

## Verification Gate

Run at minimum:

```powershell
cargo k
cargo k --no-default-features
docker compose run --rm neo cargo neo-test phase3_block_vfs_smoke
docker compose run --rm neo cargo neo-test phase3_storage_completion_cross_cpu_wake
docker compose run --rm neo cargo neo-test phase3_build1_reboot_persistence_smoke
docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke
docker compose run --rm neo cargo neo-test phase3_vfs_testfs_open_read_close
docker compose run --rm neo cargo neo-test phase3_vfs_testfs_write_readback
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_block_vfs_smoke
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all
git diff --check
cargo fmt --check
docker compose run --rm neo cargo fmt --check
```

For the aarch64 lane, add the actual command(s) used or the blocker artifact path.

If `cargo fmt --check` remains blocked by missing `cargo-fmt` for `nightly-2026-04-16`, record it exactly as prior artifacts do.

## Required Artifacts

Always create:

- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md`

If aarch64 blocks closure, also create:

- `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`

If BUILD-1 truly closes, also create:

- `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1-jarvis.md`

Do not create the close report if aarch64 is blocker-only.

## 1D Verification Artifact Must Include

- commit hash or working-tree note if not committed yet;
- files changed;
- all test commands and results;
- serial-log paths for new scenarios;
- feature-negotiation log showing F_FLUSH offered/not negotiated and CONFIG_WCE not negotiated;
- abandoned-I/O no-leak reference from 1B or rerun evidence;
- VFS stdout namespace rule reference from 1C or rerun evidence;
- reboot persistence image path and two-run evidence;
- storage completion wake evidence, including cross-CPU/instrumentation details;
- aarch64 PASS evidence or blocker artifact path;
- explicit verdict: either `BUILD-1 CLOSED` or `x86 integration PASS / BUILD-1 closure BLOCKED`.

## Stop Conditions

Stop and file a failure artifact if:

- F_FLUSH offered/not-negotiated evidence regresses;
- CONFIG_WCE is negotiated;
- abandoned-I/O wake-token/no-leak behavior regresses;
- VFS file-handle/stdout namespace behavior regresses;
- reboot persistence cannot reuse the same generated disk image;
- storage completion wake evidence bypasses the completion path;
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` regresses for reasons other than the known isolated benchmark timing flake;
- aarch64 is claimed closed without a real lane.

Failure artifact path:

- `Plans/FAILURE-2026-05-12-phase-3-build-1-1d-jarvis.md`

## Suggested Commit Message

If aarch64 passes and BUILD-1 closes:

```text
kernel/storage: close Phase 3 BUILD-1 block/VFS smoke gates
```

If x86 integration passes but aarch64 is formally blocked:

```text
kernel/storage: add Phase 3 BUILD-1 integration gates
```

## PM Posture

1A through 1C built the substrate. 1D is where we prove the pieces compose and where we stay honest about architecture coverage. A clean blocker is acceptable; a premature BUILD-1 closure is not.
