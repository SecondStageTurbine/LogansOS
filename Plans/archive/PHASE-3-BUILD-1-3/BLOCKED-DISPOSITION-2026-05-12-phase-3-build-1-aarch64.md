# Blocked Disposition — Phase 3 BUILD-1: Block layer + virtio-blk + minimal VFS

**Date:** 2026-05-12
**Author:** Prometheus (Qwen3.6-27B)
**Type:** Blocked disposition note — NOT a close report. Does NOT claim BUILD-1 closure.
**Authority:** PM decision `Plans/PM-DECISION-2026-05-12-phase-3-build-1-1d-consumption-aarch64-blocked.md`

---

## 1. Verdict

```
x86 integration PASS / BUILD-1 closure BLOCKED on aarch64 lane
```

BUILD-1 is not closed. No close report exists and none should be created until the aarch64 gate resolves.

---

## 2. Committed x86_64 Implementation State

The branch `codex/phase-3-build-1-jarvis` contains the full BUILD-1 x86_64 implementation series, HEAD at:

| SHA | Sub-commit | Description |
|---|---|---|
| `ccdacda` | 1D | Integration gates, aarch64 blocker artifact |
| `77d4f29` | 1C | Minimal VFS, testfs, file syscalls (open/read/write/close) |
| `b25c17d` | 1B | virtio-blk raw read/write, SC-3C IPI wake composition |
| `6402986` | 1B0 | Scheduler-owned weak wake-token parking substrate |
| `7453f69` | — | 1B scheduler lifetime blocker documentation |
| `982b16d` | 1A | Block substrate, DMA buffers, virtio PCI probe |

Branch is **not merged** to `origin/main`. Merge blocked on aarch64 gate resolution.

---

## 3. Accepted Verification Artifacts

| Sub-commit | Verification Report | Status |
|---|---|---|
| 1A | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1A-VERIFICATION-2026-05-12.md` | Build PASS; runtime deferred to Docker (validated by 1B0 addendum) |
| 1B0 | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-VERIFICATION-2026-05-12.md` + runtime addendum `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-RUNTIME-ADDENDUM-2026-05-12.md` | PASS (Docker, SMP=4) |
| 1B | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B-VERIFICATION-2026-05-12.md` | PASS |
| 1C | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1C-VERIFICATION-2026-05-12.md` | PASS |
| 1D | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md` | x86 integration PASS; aarch64 BLOCKED |

---

## 4. Ratified x86_64 PASS Evidence (from 1D)

### Integration scenarios
- `phase3_block_vfs_smoke` — PASS. Virtio-blk attaches, F_FLUSH offered + not negotiated, CONFIG_WCE not negotiated, raw block R/W + VFS/testfs syscalls pass in same boot.
- `phase3_storage_completion_cross_cpu_wake` — PASS. Real virtio used-ring completion path and `ThreadWakeToken` wake path instrumentation. QEMU IRQ routing limitation documented.
- `phase3_build1_reboot_persistence_smoke` — PASS. Two-boot reuse of `target/neo-test/phase3_build1_reboot_persistence_smoke.raw`.

### Regression gates
- `phase3_virtio_blk_raw_rw_smoke` — PASS.
- `phase3_vfs_testfs_open_read_close` — PASS (retained 1C evidence).
- `phase3_vfs_testfs_write_readback` — PASS (retained 1C evidence).
- `phase3_virtio_blk_abandoned_io_no_leak` — PASS (rerun).
- VFS stdout namespace rule — PASS (retained 1C evidence).
- VFS type-vs-rights distinction — PASS (retained 1C evidence).
- `smp_cross_cpu_rendezvous_stress` at SMP=4 — PASS.

### Build gates
- `cargo k` — PASS.
- `cargo k --no-default-features` — PASS.
- `git diff --check` — PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` — PASS.
- `cargo fmt --check` — BLOCKED (non-blocking tooling gap; pinned nightly lacks rustfmt on host and Docker).

### Known exclusion
- `cargo neo-test all` stopped at `irqsafe_mutex_bench` timing-sensitive benchmark threshold miss (`overhead_ns=11017 > threshold_ns=6000`). Accepted as known isolated flake, not a storage/VFS regression. Non-explicit SMP suite passed.

---

## 5. Blocker: aarch64 Barrier-Exposure Lane

**Blocker artifact:** `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`

ADR-021 requires an aarch64 QEMU CI lane before BUILD-1 closes because x86 TSO (Total Store Ordering) can hide descriptor-ring ordering bugs that weaker memory models like aarch64 would expose. Specifically, the gate targets confidence that:

- Descriptor writes are visible before the available-ring index publish;
- Available-ring publication is visible before queue notification;
- Completion observation and wake-token state use correct acquire/release ordering;
- Notification-suppression reads are separated from descriptor publication by the intended barrier class.

The current repo lacks aarch64 kernel arch modules, boot/runtime substrate (EL transition, exception vectors, GIC, MMU, per-CPU), an aarch64 QEMU test runner (`qemu-system-aarch64`), aarch64 virtio transport (virtio-mmio or PCI), and aarch64 target/toolchain configuration. A compile-only probe (`cargo build --target aarch64-unknown-none -Zbuild-std=...`) failed on uncoupled x86_64 code paths (`x86_64::instructions`, `extern "x86-interrupt"`, inline asm registers, control-register APIs, Limine MP fields). Even a successful compile-only probe would not constitute barrier-correctness evidence, because `fence()` vs `compiler_fence()` both compile successfully and only runtime/codegen behavior exposes the missing hardware barrier.

The x86_64 QEMU lane validates function, feature negotiation, write-through behavior, DMA lifetime, and wake-token composition — but it cannot prove the weak-memory ordering ADR-021 requires for closure.

---

## 6. Forward State

### Current status
```
Phase 3 BUILD-1 x86_64 implementation complete; BUILD-1 closure blocked on aarch64 barrier-exposure lane.
```

### What is NOT done
- No close report exists for BUILD-1. Do not create one until the aarch64 gate resolves.
- No BUILD-2 NeoFS work is authorized under this disposition.
- The `codex/phase-3-build-1-jarvis` branch is not merged to `origin/main`.

### Pending PM decision
PM must decide whether to:
1. Authorize a dedicated aarch64 substrate/runtime implementation lane to unblock BUILD-1 closure; or
2. Leave BUILD-1 open with this blocked disposition while scheduling the aarch64 lane as the next critical-path work.

Cerebro's feasibility artifact (`Plans/RESEARCH-2026-05-12-phase-3-build-1-aarch64-lane-feasibility-cerebro.md`) serves as the technical brief for any future implementation dispatch.

### rustfmt tooling gap (non-blocking)
`cargo fmt --check` remains blocked on both host and Docker because pinned nightly `nightly-2026-04-16` does not ship rustfmt. This is a non-blocking forward marker, carried from Phase 2 SC-3C through all Phase 3 BUILD-1 sub-commits.

---

## 7. Cross-References

| Artifact | Purpose |
|---|---|
| `Plans/PM-DECISION-2026-05-12-phase-3-build-1-1d-consumption-aarch64-blocked.md` | PM decision accepting 1D results and authorizing this disposition |
| `Plans/PM-DECISION-2026-05-12-phase-3-build-1-prometheus-cerebro-consumption-and-aarch64-gate.md` | Pre-1D gate decision (aarch64 gate stands, no BUILD-2 deferral) |
| `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md` | Jarvis 1D verification report |
| `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md` | aarch64 blocker artifact (exact missing substrate, smallest viable lane) |
| `Plans/PREP-2026-05-12-phase-3-build-1-closure-prometheus.md` | Dossier shell (inventory, evidence slots, structure) |
| `Plans/RESEARCH-2026-05-12-phase-3-build-1-aarch64-lane-feasibility-cerebro.md` | Cerebro aarch64 feasibility research (technical brief for future lane) |
| `docs/ADR-021-phase-3-design-substrate.md` | Canonical Phase 3 design substrate with BUILD-1 acceptance gates |

---

**End of blocked disposition note.**

*This is NOT a close report. BUILD-1 closure remains blocked on the aarch64 barrier-exposure lane per ADR-021. No normative files were edited.*
