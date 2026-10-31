# Dispatch: Phase 3 BUILD-1 aarch64 Barrier-Exposure Lane

Date: 2026-05-12
Recipient: Jarvis
Branch: `codex/phase-3-build-1-jarvis`
Base state: `ccdacda kernel/storage: add Phase 3 BUILD-1 integration gates`
Priority: BUILD-1 closure-unblock lane

## PM Decision

BUILD-1 x86_64 implementation is complete, but BUILD-1 closure is blocked on the aarch64 barrier-exposure gate.

PM now authorizes the dedicated aarch64 lane needed to unblock BUILD-1 closure.

This is not BUILD-2. Do not start NeoFS.

## Required Inputs

Read first:

1. `Plans/BLOCKED-DISPOSITION-2026-05-12-phase-3-build-1-aarch64.md`
2. `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`
3. `Plans/RESEARCH-2026-05-12-phase-3-build-1-aarch64-lane-feasibility-cerebro.md`
4. `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md`
5. `docs/ADR-021-phase-3-design-substrate.md`

## Mandate

Implement the smallest honest aarch64 runtime lane that exposes the BUILD-1 virtio split-virtqueue barrier class.

The lane must run under QEMU aarch64 and prove that descriptor-publish ordering and notification-suppression ordering are not merely x86-TSO accidents.

This may be a focused aarch64 test harness rather than a full aarch64 NeoPlutarch port, but it must exercise the same barrier discipline used by the BUILD-1 virtio-blk path.

## Preferred Shape

Use the path Cerebro identified as smallest:

- direct QEMU `-kernel` ELF load, not Limine;
- QEMU `virt` machine;
- PL011 serial for test output;
- semihosting or equivalent clean exit for CI status;
- virtio-mmio block device, not PCI, because the split-ring barrier discipline is transport-independent;
- generated raw disk image under `target/neo-test/`;
- a single targeted scenario, preferably named `phase3_aarch64_virtio_barrier_lane`.

Representative QEMU shape:

```text
qemu-system-aarch64
  -M virt
  -cpu cortex-a57
  -m 128M
  -nographic
  -kernel <aarch64-kernel-or-harness-elf>
  -drive if=none,format=raw,file=<generated-test-image>,id=hd0,cache=none
  -device virtio-blk-device,drive=hd0
  -no-reboot
  -d guest_errors
  -semihosting-config enable=on,target=native
```

Adjust exact flags if required by the installed QEMU version, but preserve the technical intent.

## Barrier Requirements

Introduce or preserve named barrier wrappers so the intended ordering is human-auditable.

Required names or equivalent:

```rust
publish_descriptors()
before_notification_check()
```

Required semantics:

- `publish_descriptors()` is the step-3 barrier before publishing `avail.idx`; it must use a real `fence(Ordering::Release)` or stronger, not `compiler_fence`.
- `before_notification_check()` is the step-5 barrier before reading notification-suppression state; it must use a real `fence(Ordering::SeqCst)` or equivalently strong full barrier, not `compiler_fence`.
- The existing x86_64 BUILD-1 virtio-blk path should call the same wrappers or an explicitly shared equivalent.

If the implementation cannot share the exact wrapper code between x86_64 and the aarch64 harness, explain why in the verification artifact and show how the harness still audits the production barrier policy.

## Required Runtime Coverage

The aarch64 scenario must:

- boot QEMU aarch64;
- initialize enough serial output for machine-readable PASS/FAIL;
- initialize a virtio-mmio block device;
- set up a split virtqueue;
- submit at least one write and one read or otherwise prove data movement through the virtqueue;
- verify exact data integrity;
- exercise descriptor-publish before `avail.idx`;
- exercise the notification-suppression decision branch, preferably by negotiating `VIRTIO_F_EVENT_IDX`;
- include a hang/deadline failure path so a missing notification or ordering bug fails loudly;
- exit QEMU with a host-visible success/failure status.

An always-notify harness that never exercises notification suppression is insufficient.

Compile-only aarch64 evidence is insufficient.

## Allowed Scope

Allowed:

- aarch64 target spec/config;
- minimal aarch64 boot/entry/linker pieces;
- minimal PL011 serial output;
- minimal semihosting/exit path;
- minimal virtio-mmio transport code;
- test-harness/xtask/Docker/CI plumbing required to run `qemu-system-aarch64`;
- refactoring current virtio barrier wrappers into a shared module;
- targeted changes needed to keep x86_64 BUILD-1 tests passing.

Not allowed:

- BUILD-2 NeoFS;
- full aarch64 SMP bring-up;
- full aarch64 syscall/userland port;
- a broad replacement of x86_64 boot/runtime code;
- weakening ADR-021's closure gate;
- claiming BUILD-1 closure from compile-only evidence.

## Verification Gate

Run at minimum:

```powershell
cargo k
cargo k --no-default-features
docker compose run --rm neo cargo neo-test phase3_aarch64_virtio_barrier_lane
docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke
docker compose run --rm neo cargo neo-test phase3_block_vfs_smoke
docker compose run --rm neo cargo neo-test phase3_storage_completion_cross_cpu_wake
docker compose run --rm neo cargo neo-test phase3_build1_reboot_persistence_smoke
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
git diff --check
cargo fmt --check
docker compose run --rm neo cargo fmt --check
```

If the exact aarch64 command is not exposed through `cargo neo-test`, document the exact Docker/QEMU command used and why.

The known `irqsafe_mutex_bench` explicit timing miss remains non-blocking only if targeted storage/VFS/SMP gates and the non-explicit SMP suite pass.

If `cargo fmt --check` remains blocked by missing pinned-nightly rustfmt, record the exact error.

## Required Artifact

On PASS, create:

```text
Plans/PHASE-3-BUILD-1-AARCH64-LANE-VERIFICATION-2026-05-12.md
```

The artifact must include:

- commit hash or working-tree note;
- files changed;
- exact aarch64 build/run command;
- QEMU serial log path;
- raw disk image path;
- PASS/FAIL sentinel line;
- evidence that descriptor-publish and notification-suppression paths both executed;
- evidence that a real aarch64 runtime lane ran, not compile-only;
- statement about barrier wrapper sharing with the x86_64 virtio path;
- rerun status for the x86_64 BUILD-1 integration gates.

Do not create a BUILD-1 close report. PM will consume the verification artifact first.

## Failure / Blocker Protocol

If implementation fails due to a normal bug:

- stop;
- file `Plans/FAILURE-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`;
- include the failing command, serial output, suspected source area, and whether the failure is boot, QEMU, target config, virtio-mmio, barrier, or xtask/CI plumbing.

If the lane expands beyond the intended narrow closure-unblock scope:

- stop;
- file `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-scope-expansion-jarvis.md`;
- explain what full-port substrate is being dragged in and propose the smallest next cut.

## Suggested Commit Message

If the aarch64 lane passes:

```text
kernel/storage: add aarch64 virtio barrier exposure lane
```

If only a scoped blocker lands:

```text
plans: document aarch64 barrier lane scope blocker
```

## PM Posture

The goal is closure honesty, not architecture heroics. Build the smallest runtime proof that x86 did not hide a virtqueue barrier bug. Keep the path narrow, measurable, and reversible.
