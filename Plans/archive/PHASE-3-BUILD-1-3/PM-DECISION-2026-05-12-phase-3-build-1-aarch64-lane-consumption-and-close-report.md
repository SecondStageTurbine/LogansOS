# PM Decision: BUILD-1 aarch64 Lane Consumption and Close Report Authorization

Date: 2026-05-12
Authority: PM
Inputs:

- `Plans/PHASE-3-BUILD-1-AARCH64-LANE-VERIFICATION-2026-05-12.md`
- `Plans/BLOCKED-DISPOSITION-2026-05-12-phase-3-build-1-aarch64.md`
- `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`
- `Plans/PM-DECISION-2026-05-12-phase-3-build-1-blocked-disposition-consumption-and-aarch64-lane.md`
- `docs/ADR-021-phase-3-design-substrate.md`

## Decision

The dedicated aarch64 barrier-exposure lane is accepted as PASS.

Commit:

- `7f48c85 kernel/storage: add aarch64 virtio barrier lane`

Accepted aarch64 evidence:

- `cargo neo-test phase3_aarch64_virtio_barrier_lane` runs real `qemu-system-aarch64`.
- The lane boots a standalone no_std aarch64 harness under QEMU `virt`.
- The lane uses virtio-mmio with a modern virtio-blk device.
- The harness negotiates `VIRTIO_F_EVENT_IDX`.
- The PASS frame reports `compile_only=false` and `sentinel=runtime_qemu_aarch64`.
- The PASS frame reports `publish_descriptors=2` and `before_notification_check=2`.
- The PASS frame reports `notification_decisions=2`, `notifications_sent=2`, and `data_integrity=ok`.
- Production virtio-blk and the aarch64 harness share `kernel/src/drivers/block/virtio_barrier.rs`.
- BUILD-1 x86 storage/VFS gates were rerun and passed.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` passed.

This resolves the ADR-021 aarch64 closure blocker.

## BUILD-1 Closure Status

Phase 3 BUILD-1 gates are now satisfied at the PM level:

- F_FLUSH offered/not-negotiated gate: PASS.
- CONFIG_WCE not negotiated: PASS.
- abandoned-I/O wake-token/no-leak gate: PASS.
- minimal VFS/file syscall gates: PASS.
- x86_64 integration gates: PASS.
- aarch64 barrier-exposure runtime lane: PASS.

Prometheus is authorized to prepare the BUILD-1 close report.

Do not begin BUILD-2 NeoFS until the close report is filed and PM issues the follow-on merge/docs decision.

## Formatting Caveat

`rustfmt` is now installed for the pinned nightly through the aarch64 lane work, so global `cargo fmt --check` can run.

Global formatting check currently fails on broad pre-existing formatting drift across the repo. The aarch64 artifact records scoped rustfmt PASS for the new standalone aarch64 harness and the new shared barrier wrapper.

This formatting state is accepted as a non-blocking style debt for BUILD-1 closure, with this caveat:

- the close report must name the global formatting drift explicitly;
- the close report must not describe `cargo fmt --check` as PASS;
- future cleanup may need a dedicated formatting/mechanical lane, separate from BUILD-1 correctness.

## Instruction To Prometheus

Prepare the BUILD-1 close report.

Required output:

- `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`

Use the blocked-disposition note and Prometheus dossier shell as inputs, but supersede the blocked verdict with the accepted aarch64 PASS evidence from `7f48c85`.

Do not edit normative docs yet.

## Instruction To Jarvis

No further implementation work is requested under BUILD-1 before PM consumes the close report.

Stand by.

## Current External Status

Until the close report is filed:

```text
Phase 3 BUILD-1 gates satisfied; close report in preparation.
```
