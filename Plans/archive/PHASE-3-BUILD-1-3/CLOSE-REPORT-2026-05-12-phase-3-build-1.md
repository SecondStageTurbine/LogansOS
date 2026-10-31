# CLOSE REPORT — Phase 3 BUILD-1: Block layer + virtio-blk + minimal VFS

**Date:** 2026-05-12
**Author:** Prometheus (Qwen3.6-27B)
**Status:** CLOSED
**Authority:** PM decision `Plans/PM-DECISION-2026-05-12-phase-3-build-1-aarch64-lane-consumption-and-close-report.md`

---

## 1. Closure Summary

Phase 3 BUILD-1 is closed. All ADR-021 acceptance gates are satisfied: block substrate, DMA buffers, virtio-blk write-through driver, minimal VFS with testfs and file syscalls, x86_64 integration gates, and the aarch64 barrier-exposure runtime lane.

The BUILD-1 implementation series landed on `codex/phase-3-build-1-jarvis` through seven commits (including documentation-only sub-commits), HEAD at `7f48c85`. One course correction occurred during 1B when the scheduler lifetime model blocked storage's `Weak<Thread>` rendezvous requirement, resolved by introducing 1B0 with a scheduler-owned parked-thread table and generation-checked `ThreadWakeToken { cpu_id, slot, generation }`. A second blocker arose in 1D when the aarch64 barrier-exposure lane could not be implemented due to missing substrate; this was resolved by a dedicated follow-up lane (`7f48c85`) that added a standalone no_std aarch64 harness under QEMU `virt` with virtio-mmio.

---

## 2. Commits on Branch

| SHA | Sub-commit | Description |
|---|---|---|
| `7f48c85` | aarch64 lane | Add aarch64 virtio barrier lane (resolves 1D blocker) |
| `ccdacda` | 1D | Integration gates, aarch64 blocker artifact |
| `77d4f29` | 1C | Minimal VFS, testfs, file syscalls (open/read/write/close) |
| `b25c17d` | 1B | virtio-blk raw read/write, SC-3C IPI wake composition |
| `3b5d35d` | — | Ratify 1B0 runtime wake-token gate (doc-only) |
| `6402986` | 1B0 | Scheduler-owned weak wake-token parking substrate |
| `7453f69` | — | File 1B scheduler lifetime blocker (doc-only) |
| `982b16d` | 1A | Block substrate, DMA buffers, virtio PCI probe |

Branch: `codex/phase-3-build-1-jarvis`. Not yet merged to `origin/main`.

---

## 3. Verification

### 3.1 Per-sub-commit verification artifacts

| Sub-commit | Verification Report | Status |
|---|---|---|
| 1A | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1A-VERIFICATION-2026-05-12.md` | Build PASS; runtime deferred to Docker (validated by 1B0 addendum) |
| 1B0 | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-VERIFICATION-2026-05-12.md` + `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-RUNTIME-ADDENDUM-2026-05-12.md` | PASS (Docker, SMP=4) |
| 1B | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B-VERIFICATION-2026-05-12.md` | PASS |
| 1C | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1C-VERIFICATION-2026-05-12.md` | PASS |
| 1D | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md` | x86 integration PASS; aarch64 blocker filed |
| aarch64 lane | `Plans/PHASE-3-BUILD-1-AARCH64-LANE-VERIFICATION-2026-05-12.md` | PASS — resolves 1D blocker |

### 3.2 Integration scenarios (1D)

- `phase3_block_vfs_smoke` — PASS. Virtio-blk attaches, F_FLUSH offered + not negotiated, CONFIG_WCE not negotiated, raw block R/W and VFS/testfs syscalls pass in same boot.
- `phase3_storage_completion_cross_cpu_wake` — PASS. Real virtio used-ring completion path and `ThreadWakeToken` wake path instrumentation. QEMU IRQ routing limitation documented.
- `phase3_build1_reboot_persistence_smoke` — PASS. Two-boot reuse of generated raw disk image; boot 1 writes pattern, boot 2 reads back.

### 3.3 Regression gates (aarch64 lane rerun)

- `phase3_virtio_blk_raw_rw_smoke` — PASS.
- `phase3_block_vfs_smoke` — PASS.
- `phase3_storage_completion_cross_cpu_wake` — PASS.
- `phase3_build1_reboot_persistence_smoke` — PASS (both boots).
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` — PASS (full non-explicit SMP suite).

### 3.4 aarch64 barrier-exposure lane

The dedicated aarch64 lane (`7f48c85`) boots a standalone no_std aarch64 harness under QEMU `virt` with a modern virtio-mmio block device. The harness negotiates `VIRTIO_F_EVENT_IDX`, exercises the notification-suppression branch, and shares barrier wrapper source with production virtio-blk via `kernel/src/drivers/block/virtio_barrier.rs`.

PASS frame:

```
NEO_TEST/1 event=PASS name=phase3_aarch64_virtio_barrier_lane runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true feature_low=0x30006e54 feature_high=0x00000101 sector=8 block_size=512 write_ok=true read_ok=true data_integrity=ok publish_descriptors=2 before_notification_check=2 notification_decisions=2 notifications_sent=2 compile_only=false sentinel=runtime_qemu_aarch64
```

Barrier exposure counts:
- `publish_descriptors=2` — descriptor writes ordered before available-ring index publish via `fence(Ordering::Release)`
- `before_notification_check=2` — notification-suppression reads separated from descriptor publication via `fence(Ordering::SeqCst)`
- `event_idx_negotiated=true` — notification-suppression branch exercised, not always-notify shortcut
- `compile_only=false sentinel=runtime_qemu_aarch64` — real runtime execution, not compile-only evidence

---

## 4. Acceptance Gate Reconciliation

| ADR-021 Gate | Evidence | Status |
|---|---|---|
| F_FLUSH offered, not negotiated | `F_FLUSH_offered=true`, `F_FLUSH_negotiated=false` in x86 and aarch64 serial logs | PASS |
| CONFIG_WCE not negotiated | `F_CONFIG_WCE_negotiated=false` in serial logs | PASS |
| Abandoned-I/O no-resurrection/no-leak | `phase3_virtio_blk_abandoned_io_no_leak` — `retained_dma=3`, `live_before=3`, `live_after=3`, `late_wake=2` | PASS |
| Minimal VFS and file syscall scenarios | 1C: testfs open/read/close/write/readback, stdout namespace (fd=1 serial), type vs rights errors (EISDIR/EACCES) | PASS |
| x86_64 block+VFS integration | `phase3_block_vfs_smoke`, `phase3_storage_completion_cross_cpu_wake` — all PASS | PASS |
| Reboot persistence smoke | Two-boot reuse of `phase3_build1_reboot_persistence_smoke.raw`; boot 1 write + same-boot verify, boot 2 readback | PASS |
| aarch64 barrier-exposure runtime lane | `phase3_aarch64_virtio_barrier_lane` — real QEMU aarch64, virtio-mmio, EVENT_IDX negotiated, shared barrier wrappers, `compile_only=false` | PASS |

---

## 5. Mechanism Named

N/A — BUILD-1 is substrate work (block layer, virtio-blk driver, VFS skeleton). No novel mechanism requires naming under this build.

---

## 6. Course Corrections During BUILD

### 6.1 Scheduler lifetime blocker (1B → 1B0)

Sub-commit 1B stopped when the existing scheduler `Box<Thread>` ownership model proved incompatible with storage's need for `Weak<Thread>` rendezvous in completion wake paths. A formal blocker was filed (`Plans/FAILURE-2026-05-12-phase-3-build-1-jarvis.md`). PM ratified option 2: a scheduler-owned parked-thread table with generation-checked `ThreadWakeToken { cpu_id, slot, generation }`. Sub-commit 1B0 implemented this substrate, validated in Docker at SMP=4 (all 11 targeted scenarios + full regression), and 1B resumed successfully.

### 6.2 aarch64 barrier-exposure blocker (1D → follow-up lane)

Sub-commit 1D could not implement the required aarch64 lane because the repo lacked aarch64 kernel arch modules, boot/runtime substrate, QEMU runner, virtio transport, and target configuration. A formal blocker artifact was filed (`Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`), and the blocked disposition note was prepared (`Plans/BLOCKED-DISPOSITION-2026-05-12-phase-3-build-1-aarch64.md`). A dedicated follow-up lane (`7f48c85`) then implemented a standalone no_std aarch64 harness under QEMU `virt` with virtio-mmio, sharing barrier wrapper source with production virtio-blk. The lane passed all runtime evidence, resolving the blocker.

---

## 7. Formatting Caveat

`rustfmt` is now installed for the pinned nightly through the aarch64 lane work, so global `cargo fmt --check` can run. However, the repo has broad pre-existing formatting drift across multiple files (`kernel/src/arch/x86_64/apic.rs`, `gdt.rs`, `pic.rs`, `drivers/block/virtio_blk.rs`, `drivers/fb/font.rs`, `loader/elf.rs`, `mm/mod.rs`, and others). Global `cargo fmt --check` **fails** on this pre-existing drift.

Scoped rustfmt for the new aarch64 harness source tree and the new shared barrier wrapper (`kernel/src/drivers/block/virtio_barrier.rs`) **passes**.

This is recorded as a non-blocking style debt. Future cleanup may require a dedicated formatting/mechanical lane separate from BUILD-1 correctness work.

---

## 8. Forward Markers

### 8.1 Formatting/mechanical cleanup (optional)

Broad pre-existing format drift across the repo will cause `cargo fmt --check` to fail globally. A future mechanical lane could address this, but it is not a correctness blocker and is not scoped under BUILD-1.

### 8.2 BUILD-2 NeoFS native

ADR-021 §BUILD-2 defines NeoFS native: journaling layer, write-back policy, F_FLUSH/F_CONFIG_WCE negotiation, multi-device allocation, directory inodes, extended attributes. BUILD-2 is **not** authorized by this close report. A separate PM dispatch is required to begin BUILD-2 work.

### 8.3 Docs/index/phase updates

Creating `docs/phases/PHASE-3.md`, updating `Plans/INDEX.md` with BUILD-1 artifact entries, and merging `codex/phase-3-build-1-jarvis` to `origin/main` require a follow-on PM instruction. This close report does not edit normative docs.

---

## 9. Cross-References

| Artifact | Purpose |
|---|---|
| `Plans/PM-DECISION-2026-05-12-phase-3-build-1-aarch64-lane-consumption-and-close-report.md` | PM decision accepting aarch64 PASS, authorizing close report |
| `Plans/PM-DECISION-2026-05-12-phase-3-build-1-prometheus-cerebro-consumption-and-aarch64-gate.md` | Pre-1D gate decision (aarch64 gate stands) |
| `Plans/PHASE-3-BUILD-1-AARCH64-LANE-VERIFICATION-2026-05-12.md` | aarch64 lane verification — PASS frame, regression gates |
| `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md` | 1D x86 integration verification |
| `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1C-VERIFICATION-2026-05-12.md` | 1C VFS/syscall verification |
| `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B-VERIFICATION-2026-05-12.md` | 1B virtio-blk verification |
| `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-RUNTIME-ADDENDUM-2026-05-12.md` | 1B0 Docker runtime gate |
| `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1A-VERIFICATION-2026-05-12.md` | 1A block substrate verification |
| `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md` | Original aarch64 blocker (resolved by `7f48c85`) |
| `Plans/BLOCKED-DISPOSITION-2026-05-12-phase-3-build-1-aarch64.md` | Blocked disposition note (superseded by this report) |
| `Plans/PREP-2026-05-12-phase-3-build-1-closure-prometheus.md` | Dossier shell (inventory, evidence slots, structure) |
| `Plans/RESEARCH-2026-05-12-phase-3-build-1-aarch64-lane-feasibility-cerebro.md` | Cerebro aarch64 feasibility research |
| `Plans/FAILURE-2026-05-12-phase-3-build-1-jarvis.md` | 1B scheduler lifetime blocker (resolved by 1B0) |
| `docs/ADR-021-phase-3-design-substrate.md` | Canonical Phase 3 design substrate with BUILD-1 acceptance gates |

---

## 10. Closure Statement

Phase 3 BUILD-1 is **CLOSED**. All ADR-021 acceptance gates are satisfied: x86_64 block+VFS integration PASS, F_FLUSH offered/not negotiated, CONFIG_WCE not negotiated, abandoned-I/O no-leak, and aarch64 barrier-exposure runtime lane PASS with real QEMU execution (`compile_only=false`, `sentinel=runtime_qemu_aarch64`).

No BUILD-2 NeoFS work is authorized by this report. No normative docs were edited. Merge to `origin/main` and documentation updates require follow-on PM instruction.

---

**End of close report.**