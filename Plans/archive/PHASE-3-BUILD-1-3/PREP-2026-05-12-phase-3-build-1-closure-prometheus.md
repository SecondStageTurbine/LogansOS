# PREP — Phase 3 BUILD-1 Closure Dossier Shell (Prometheus)

**Date:** 2026-05-12
**Author:** Prometheus (Qwen3.6-27B)
**Type:** Dossier shell — inventory, evidence slots, close-report structure. Does NOT claim closure.
**Authority:** PM dispatch. No edits to ADR-021, Bible, README, or phase docs until PM has Jarvis's 1D result.

---

## 1. Mandate

Produce a closure dossier shell for Phase 3 BUILD-1 that:
(a) inventories all sub-commit artifacts and commits;
(b) lists required 1D evidence slots;
(c) prepares the close-report structure;
(d) notes unresolved gates.

**This shell does NOT claim BUILD-1 closed.** It is a working artifact for PM consumption after Jarvis returns with 1D results. No normative files are edited.

---

## 2. BUILD-1 Scope (from ADR-021)

ADR-021 §BUILD-1: Block layer + virtio-blk skeleton + minimal VFS.

Includes: block layer trait/request model, DMA buffer infrastructure, PCI/PCIe enumeration extensions for virtio, virtio-blk write-through driver (F_FLUSH asserted), minimal VFS and test filesystem, `sys_open`/`sys_read`/`sys_write`/`sys_close`, libneo wrappers, block/VFS smoke tests.

Acceptance gates (ADR-021):
- Feature-negotiation log records F_FLUSH offered and not negotiated;
- Abandoned-I/O test proves weak scheduler wake-token rendezvous + Arc-managed DMA buffers do not leak;
- aarch64 QEMU CI lane passes before BUILD-1 closes.

---

## 3. Sub-Commit Inventory

### 3A — Block substrate, DMA buffers, virtio PCI probe

| Field | Value |
|---|---|
| Commit SHA | `982b16d` |
| Subject | `kernel/storage: scaffold block substrate, DMA buffers, and virtio PCI probe (1A)` |
| Branch | `codex/phase-3-build-1-jarvis` |
| Verification | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1A-VERIFICATION-2026-05-12.md` |
| Build gates | `cargo k` PASS, `cargo k --no-default-features` PASS |
| Runtime gates | BLOCKED — missing Limine on Windows host. No runtime verification achieved on this host. Later validated by 1B0 regression in Docker. |
| Status | Build PASS; runtime deferred to Docker lane (validated by 1B0 addendum) |

### 1B0 — Scheduler weak wake-token parking substrate

| Field | Value |
|---|---|
| Commit SHA | `6402986` |
| Subject | `kernel/sched: add weak wake-token parking substrate (1B0)` |
| Branch | `codex/phase-3-build-1-jarvis` |
| Verification | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-VERIFICATION-2026-05-12.md` (initial — build PASS, QEMU BLOCKED) |
| Runtime addendum | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-RUNTIME-ADDENDUM-2026-05-12.md` — **PASS** (Docker, SMP=4, all 11 targeted scenarios + full regression) |
| Blocker resolution | `Plans/PM-DECISION-2026-05-12-phase-3-build-1-1b-scheduler-lifetime-blocker.md` — PM ratified option 2: scheduler-owned parked-thread table + generation-checked `ThreadWakeToken { cpu_id, slot, generation }` |
| Predecessor failure | `Plans/FAILURE-2026-05-12-phase-3-build-1-jarvis.md` — STOPPED before 1B. Scheduler `Box<Thread>` ownership model incompatible with storage `Weak<Thread>` rendezvous requirement. |
| Status | **PASS** (runtime-ratified in Docker) |

### 1B — virtio-blk raw read/write, SC-3C IPI wake composition

| Field | Value |
|---|---|
| Commit SHA | `b25c17d` |
| Subject | `kernel/block: implement virtio-blk raw read write (1B)` |
| Branch | `codex/phase-3-build-1-jarvis` |
| Verification | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B-VERIFICATION-2026-05-12.md` |
| Runtime gates | F_FLUSH offered/not-negotiated gate PASS. Abandoned-I/O no-leak PASS. Full SMP=4 regression PASS (after one `irqsafe_mutex_bench` timing flake). |
| Status | **PASS** |

### 1C — VFS skeleton, testfs, sys_open/read/write/close

| Field | Value |
|---|---|
| Commit SHA | `77d4f29` |
| Subject | `kernel/fs: add testfs file syscalls (1C)` |
| Branch | `codex/phase-3-build-1-jarvis` |
| Verification | `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1C-VERIFICATION-2026-05-12.md` |
| Runtime gates | VFS open/read/close/write/readback all PASS. Type vs rights errors distinguished (EISDIR/EACCES). Stdout compat PASS. Full SMP=4 regression PASS. |
| Status | **PASS** |

### 1D — Integration closure (NOT YET IMPLEMENTED)

| Field | Value |
|---|---|
| Commit SHA | N/A — not yet committed |
| Dispatch | `Plans/DISPATCH-2026-05-12-phase-3-build-1-1d-integration-closure-jarvis.md` |
| Verification report | **NOT YET FILED** |
| Close report | **NOT YET FILED** |
| Blocker artifact | **NOT YET FILED** (aarch64 lane may produce `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`) |
| Status | PENDING — awaiting Jarvis 1D execution |

---

## 4. Required 1D Evidence Slots

Per dispatch `DISPATCH-2026-05-12-phase-3-build-1-1d-integration-closure-jarvis.md`, the following evidence slots must be filled before BUILD-1 can close:

### 4.1 Integration scenarios (new)

| Slot | Scenario | Required evidence |
|------|----------|-------------------|
| S1 | `phase3_block_vfs_smoke` | QEMU boots with Limine modules; virtio-blk attaches and logs feature policy; F_FLUSH offered + not negotiated; CONFIG_WCE not negotiated; raw block R/W smoke passes; VFS/testfs syscalls pass in same boot; serial output includes both storage and VFS evidence lines |
| S2 | `phase3_storage_completion_cross_cpu_wake` | I/O issued from schedulable thread/process path; completion wakes through 1B0 `ThreadWakeToken`; cross-CPU wake composition demonstrated or instrumentation proves SC-3C wake path exercised (with documented QEMU IRQ routing limitation) |
| S3 | `phase3_build1_reboot_persistence_smoke` | Deterministic raw disk image created under `target/neo-test/`; first QEMU run writes known pattern through virtio-blk; second QEMU run reuses same image and reads pattern back; serial/host logs tie both runs to same scenario and image path |

### 4.2 Regression gates (existing scenarios, must still pass)

| Slot | Scenario | Source sub-commit |
|------|----------|-------------------|
| R1 | `phase3_virtio_blk_raw_rw_smoke` | 1B |
| R2 | `phase3_vfs_testfs_open_read_close` | 1C |
| R3 | `phase3_vfs_testfs_write_readback` | 1C |
| R4 | Abandoned-I/O no-leak evidence | 1B (reference or rerun) |
| R5 | VFS stdout namespace rule | 1C (reference or rerun) |
| R6 | `smp_cross_cpu_rendezvous_stress` at SMP=4 | Phase 2 anti-regression |

### 4.3 Architecture coverage

| Slot | Requirement | Acceptable outcome |
|------|-------------|-------------------|
| A1 | aarch64 QEMU CI lane | Real lane that builds and runs virtqueue descriptor-publish/notify path against emulated virtio-blk; PASS evidence recorded |
| A1 (blocker) | If aarch64 substrate insufficient | Formal blocker artifact `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md` with exact missing substrate, smallest viable proposal, x86-only limitation explanation, compile-only attempt status, PM decision needed |

### 4.4 Build gates

| Slot | Command |
|------|---------|
| B1 | `cargo k` |
| B2 | `cargo k --no-default-features` |
| B3 | `git diff --check` |
| B4 | `cargo fmt --check` (known BLOCKED — see §6.1) |

---

## 5. Close Report Structure (template)

When Jarvis returns with 1D results, the close report will follow this structure:

```
# CLOSE REPORT — Phase 3 BUILD-1: Block layer + virtio-blk + minimal VFS

**Date:** 2026-05-MM
**Owner:** PM
**Status:** [CLOSED / BLOCKED on aarch64]

## 1. Closure summary
[Sub-commits 1A, 1B0, 1B, 1C + 1D all landed. Gates status.]

## 2. Commits on branch
| SHA | Sub-commit | Description | Verification report |

## 3. Verification
### 3.1 Per-sub-commit gates (all PASS)
[Reference five verification reports]
### 3.2 Integration scenarios
[S1, S2, S3 results]
### 3.3 Regression gates
[R1-R6 results]
### 3.4 aarch64 lane
[PASS evidence OR blocker artifact path]

## 4. Acceptance gate reconciliation
| ADR-021 Gate | Evidence | Status |
| F_FLUSH offered/not negotiated | [serial log path + line] | PASS/... |
| Abandoned-I/O no-leak | [1B verification ref or rerun] | PASS/... |
| aarch64 QEMU CI lane | [PASS or blocker] | PASS/BLOCKED |

## 5. Mechanism named
[If any — likely N/A for BUILD-1 substrate work]

## 6. Course corrections during BUILD
[1B scheduler lifetime blocker + 1B0 resolution; any 1D corrections]

## 7. Forward markers
[rustfmt tooling gap; aarch64 lane (if blocked); Phase 3 BUILD-2+ pointers]

## 8. Calibration
[Any process observations]

## 9. Cross-references
[All consumed artifacts]

## 10. Closure statement
[BUILD-1 closed / BUILD-1 x86 integration PASS, closure BLOCKED on aarch64]
```

---

## 6. Unresolved Gates

### 6.1 1D runtime evidence (PENDING)

Sub-commit 1D has not been implemented. The dispatch exists (`DISPATCH-2026-05-12-phase-3-build-1-1d-integration-closure-jarvis.md`) but no verification report, no commit, no close report. Three new integration scenarios (S1/S2/S3) and the aarch64 lane are all pending Jarvis execution.

**Impact:** BUILD-1 cannot close until 1D completes. This is the current critical-path item.

### 6.2 aarch64 lane (PENDING — may BLOCK closure)

ADR-021 requires an aarch64 QEMU CI lane before BUILD-1 closes because x86 TSO can hide descriptor-ring barrier bugs. Two outcomes:
- **Preferred:** Real aarch64 lane implemented, PASS evidence recorded in 1D verification artifact.
- **Acceptable blocker:** Formal blocker artifact filed per dispatch §aarch64 Gate. Verdict becomes "x86 integration PASS / BUILD-1 closure BLOCKED on aarch64 lane." No close report produced.

**Impact:** If blocked, BUILD-1 cannot claim closure. x86 work is complete; architecture coverage is the remaining gap.

### 6.3 rustfmt environment block (KNOWN — non-blocking)

The pinned nightly `nightly-2026-04-16-x86_64-pc-windows-msvc` does not have rustfmt installed. `cargo fmt --check` is BLOCKED on both host and Docker (`nightly-2026-04-16-x86_64-unknown-linux-gnu`). This gap has persisted since SC-3C (forward marker from `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` §6.1, continued through SC-4 §6.1). All Phase 3 BUILD-1 verification reports (1A, 1B0, 1B, 1C) record `cargo fmt --check` as BLOCKED.

**Impact:** Non-blocking. Does not prevent closure; recorded as a forward marker. Three options enumerated in SC-3C close report: (1) install rustfmt for pinned nightly, (2) pin different nightly that ships rustfmt, (3) accept gap with alternative linting. PL operational decision pending.

---

## 7. Supporting Artifact Inventory

### THINK + design artifacts

| File | Purpose |
|------|---------|
| `Plans/PHASE-3-THINK-V1-2026-05-11.md` | Initial exploration (superseded by V2) |
| `Plans/PHASE-3-THINK-V2-2026-05-11.md` | Post-structural-challenge synthesis; reframed Q5 |
| `docs/ADR-021-phase-3-design-substrate.md` | Canonical Phase 3 design substrate (BUILD-1 through BUILD-5) |

### Cerebro research artifacts

| File | Purpose |
|------|---------|
| `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` | Cerebro #1: NeoFS prior art; source for ADR-021 citation corrections |
| `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md` | Cerebro #2: storage driver prior art (virtio-blk WCE/FLUSH, AHCI, NVMe) |
| `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md` | Cerebro #3: VFS typed inode + capability prior art |

### PM notes consuming Cerebro results

| File | Purpose |
|------|---------|
| `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` | Cerebro #1 consumption; Q5 reframe |
| `Plans/PM-NOTE-2026-05-12-phase-3-cerebro-2-3-consumption-and-adr-021-extensions.md` | Cerebro #2 and #3 consumption; ADR-021 extensions |

### Dispatches

| File | Purpose |
|------|---------|
| `Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` | Cerebro #1 dispatch |
| `Plans/DISPATCH-2026-05-12-phase-3-build-1-implementation-jarvis.md` | Initial BUILD-1 implementation (produced 1A + FAILURE) |
| `Plans/DISPATCH-2026-05-12-phase-3-build-1-1b0-scheduler-wake-token-jarvis.md` | 1B0 wake-token substrate |
| `Plans/DISPATCH-2026-05-12-phase-3-build-1-1b0-runtime-verification-jarvis.md` | 1B0 Docker runtime gate |
| `Plans/DISPATCH-2026-05-12-phase-3-build-1-1b-resume-virtio-blk-jarvis.md` | 1B resume after 1B0 ratification |
| `Plans/DISPATCH-2026-05-12-phase-3-build-1-1c-vfs-file-syscalls-jarvis.md` | 1C VFS/syscall |
| `Plans/DISPATCH-2026-05-12-phase-3-build-1-1d-integration-closure-jarvis.md` | 1D closure (PENDING execution) |

### Handoff documents

| File | Purpose |
|------|---------|
| `Plans/HANDOFF-2026-05-11-end-of-day-phase-2-build-3-substantially-complete-phase-3-think-v1.md` | End-of-day handoff; Phase 2 BUILD-3 substantially complete |
| `Plans/HANDOFF-2026-05-11-post-cerebro-1-consumption-phase-3-think-v2-pending.md` | Post-Cerebro #1; THINK v2 pending |

---

## 8. Branch State

| Field | Value |
|---|---|
| Branch | `codex/phase-3-build-1-jarvis` (local only) |
| HEAD | `77d4f29` — sub-commit 1C (last committed work) |
| origin/main HEAD | `64a92e1` — Q9 deferral documentation |
| Commits ahead of main | 6 (1A, FAILURE doc, 1B0, 1B0 runtime addendum doc, 1B, 1C) |
| Merged to main? | **No** — BUILD-1 has not been merged. Merge blocked on 1D completion + PM consumption of all evidence. |

---

## 9. Stop Conditions for Closure

Do NOT claim BUILD-1 closed if any of the following hold:

- 1D verification report not filed;
- F_FLUSH offered/not-negotiated evidence regresses;
- CONFIG_WCE is negotiated;
- Abandoned-I/O wake-token/no-leak behavior regresses;
- VFS file-handle/stdout namespace behavior regresses;
- Reboot persistence cannot reuse the same generated disk image;
- Storage completion wake evidence bypasses the completion path;
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` regresses (excluding known `irqsafe_mutex_bench` timing flake);
- aarch64 is claimed closed without a real lane;
- Any sub-commit verification report shows FAIL that hasn't been resolved.

---

## 10. Next Steps (PM action items after Jarvis 1D returns)

1. **Consume 1D verification report.** Verify all evidence slots in §4 are filled. Cross-check serial-log paths, feature-negotiation lines, abandoned-I/O references.
2. **Decide aarch64 disposition.** If blocker artifact filed: accept "x86 PASS / BUILD-1 BLOCKED" verdict OR commission follow-up aarch64 lane. If PASS: proceed to closure.
3. **Merge to origin/main.** Only after PM ratifies all evidence. Merge the full `codex/phase-3-build-1-jarvis` branch (all 6 commits + any 1D commit).
4. **PHASE-3.md creation.** Create `docs/phases/PHASE-3.md` with BUILD-1 section, following PHASE-2.md format precedent.
5. **INDEX.md sweep.** Add Phase 3 BUILD-1 artifact entries.
6. **Close report.** File `Plans/CLOSE-REPORT-2026-05-MM-phase-3-build-1.md` (or blocker disposition note).

---

**End of dossier shell.**

*This is a PREP artifact — inventory and structure only. Does NOT claim BUILD-1 closure. Awaiting Jarvis 1D execution results before any normative file edits.*