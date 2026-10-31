# PREP — Phase 3 BUILD-3 Evidence Matrix Shell (Prometheus)

**Date:** 2026-05-14
**Author:** Prometheus (Qwen3.6-27B)
**Type:** PREP ONLY — evidence-matrix shell. Does NOT claim BUILD-3 closure. No implementation authorized.
**Authority:** `Plans/PM-DECISION-2026-05-14-phase-3-build-3-opening.md`

---

## 1. Mission

Prepare an evidence-matrix shell for Phase 3 BUILD-3 while Jarvis audits prerequisites and Cerebro challenges the BUILD-3 boundary. This artifact defines evidence slots, regression gates, stop conditions, and anti-scope assertions. It does not authorize implementation or claim closure.

BUILD-3 remains blocked on:
1. Jarvis prerequisite audit (`Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md`) — **[PENDING]**
2. Cerebro structural challenge (`Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md`) — **[PENDING]**
3. PM consumption of both artifacts before any BUILD-3 PLAN dispatch

---

## 2. Inputs

| Input Artifact | Status | Purpose |
|---|---|---|
| `docs/phases/PHASE-3.md` | Current | Phase 3 phase doc — BUILD-1/BUILD-2 closed, BUILD-3 not started |
| `Plans/CLOSE-REPORT-2026-05-14-phase-3-build-2.md` | CLOSED | BUILD-2 close report — regression gate source |
| `Plans/PM-DECISION-2026-05-14-phase-3-build-3-opening.md` | OPEN OBSERVE GATE | PM decision opening BUILD-3 planning/observe gate only |
| `Plans/PHASE-3-THINK-V2-2026-05-11.md` | Active | THINK v2 — BUILD-3 scope, entry gate, measurement-gated CAS |
| `Plans/DISPATCH-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md` | DISPATCHED | Jarvis prerequisite audit dispatch |
| `Plans/DISPATCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md` | DISPATCHED | Cerebro structural challenge dispatch |

Pending artifacts:

| Slot ID | Source Artifact | Status |
|---|---|---|
| OBS-JARVIS | `Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md` | **[PENDING]** — Jarvis observe audit not yet filed |
| RES-CEREBRO | `Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md` | **[PENDING]** — Cerebro structural challenge not yet filed |

---

## 3. BUILD-3 Closure Claim Template

When all gates pass, the close report will assert:

```
Phase 3 BUILD-3 CLOSED.
NeoFS content-addressing (if admitted), kthread ownership substrate,
virtio FLUSH negotiation, write-back buffer cache, durable sys_fsync,
CoW crash/recovery with cached dirty data, and mark-sweep GC all
pass runtime evidence with full BUILD-1/BUILD-2 regression sweep.
```

Stop conditions in §11 must all be satisfied before this claim is honest.

---

## 4. Evidence Slots — BUILD-3 Sub-Commit Areas

Sub-commit boundaries are not yet finalized. Jarvis's observe audit will propose the split. This matrix defines the evidence slots that any sub-commit plan must fill. Each slot maps to a closure gate.

### 4.1 Kthread ownership/lifecycle substrate

| Slot ID | Work Area | Evidence Required |
|---|---|---|
| SC-KT-OWN | Kernel-thread ownership primitive | kthread handle identity; stop/cancellation flag; join/reap or detached semantics; per-mount worker teardown on unmount; panic/exit behavior documented; no `Arc<Thread>` lifetime mistakes. Must distinguish from existing global `sched::spawn` + infinite-loop completion worker pattern |
| SC-KT-LIFE | Kthread lifecycle discipline | FS instance owns worker handles; unmount reaps workers through normal exit path; no leaked workers after unmount; worker count verifiable at mount/unmount boundaries |

### 4.2 Virtio FLUSH negotiation and command issue path

| Slot ID | Work Area | Evidence Required |
|---|---|---|
| SC-FLUSH-NEG | `VIRTIO_BLK_F_FLUSH` feature negotiation | Feature bit negotiated during virtio-blk init; `F_FLUSH_negotiated=true` in serial log; `CONFIG_WCE` handling documented. Must not break BUILD-1 write-through path when FLUSH unavailable |
| SC-FLUSH-ISSUE | FLUSH command issue and completion | FLUSH command issued after dirty data writes; completion verified before superblock publish; ordering: dirty write completions → FLUSH → superblock write → FLUSH → mount-time recovery validates durability |

### 4.3 Write-back buffer cache

| Slot ID | Work Area | Evidence Required |
|---|---|---|
| SC-CACHE-LK | Cache lookup and dirty tracking | Dirty-buffer tracking structure; cache-hit vs cache-miss path distinguishable in serial log; dirty metadata (superblock, inode tree) tracked separately from clean data blocks |
| SC-CACHE-WB | Write-back execution | Background write-back worker flushes dirty buffers to disk; watermark or periodic trigger verifiable; no silent data loss on unclean shutdown with write-back cache active |

### 4.4 `sys_fsync` transition from BUILD-2 no-op to durable write-back contract

| Slot ID | Work Area | Evidence Required |
|---|---|---|
| SC-FSYNC-DUR | Durable fsync over write-back cache | `sys_fsync` forces all dirty buffers for target file through FLUSH path; fsync_rc=0 only after disk durability confirmed. Must NOT behave as BUILD-2 no-op — if fsync still returns 0 without flushing, this slot FAILS |
| SC-FSYNC-SCOPE | Per-file fsync scope isolation | fsync on file A does not flush unrelated dirty buffers for file B; serial log distinguishes flushed vs unflushed targets |

### 4.5 CoW crash/recovery with cached dirty data

| Slot ID | Work Area | Evidence Required |
|---|---|---|
| SC-CRASH-DC | Crash recovery with dirty cache | Kill mid-write-back; remount recovers committed data; no torn state in previously fsynced files; orphaned CoW blocks from cached writes do not corrupt free-space state |
| SC-CRASH-META | Dirty metadata recovery | Inode tree and superblock changes survive crash if fsynced; unfsynced metadata may be lost but must not leave mount-inconsistent state |

### 4.6 Mark-sweep GC over reachable NeoFS blocks

| Slot ID | Work Area | Evidence Required |
|---|---|---|
| SC-GC-MARK | Reachable-block walk | Full mark pass from current root inode traverses all reachable inodes and data blocks; marked block set verifiable against allocator state |
| SC-GC-EEP | Sweep and orphan reclamation | Unmarked blocks returned to free-space allocator; post-sweep allocator state consistent (no double-free, no leaked orphans); bump allocator monotonicity preserved for live allocations |

### 4.7 CAS/dedup measurement gate (conditional)

| Slot ID | Work Area | Evidence Required |
|---|---|---|
| SC-CAS-ADMIT | CAS admission decision | PM admits CAS with explicit language, OR PM defers CAS with explicit language. If admitted: bloom filter or on-disk hash table content index present; hash-as-address lookup path functional; dedup ratio measurement evidence exists |
| SC-CAS-MEASURE | Dedup measurement gate | Measured workload shows meaningful dedup value, OR Phase 5 model-storage planning proves CAS is critical path. Without measurement or PM override: slot REMAINS PENDING, CAS not closed |

---

## 5. Regression Gates — Inherited from BUILD-1

These gates must still pass after every BUILD-3 sub-commit to prove new mechanisms do not regress the block/VFS substrate.

| Gate ID | Scenario | Source | Required Evidence |
|---|---|---|---|
| R-BLK-1 | `phase3_virtio_blk_raw_rw_smoke` | BUILD-1 SC-1B | F_FLUSH offered; descriptor publish barriers=2; notification suppression barriers=2 |
| R-BLK-2 | `phase3_block_vfs_smoke` | BUILD-1 SC-1D | Virtio-blk attaches; raw block R/W + VFS syscalls pass in same boot |
| R-BLK-3 | `phase3_storage_completion_cross_cpu_wake` | BUILD-1 SC-1D | Real virtio used-ring completion path; ThreadWakeToken wake path |
| R-BLK-4 | `phase3_build1_reboot_persistence_smoke` | BUILD-1 SC-1D | Two-boot reuse of raw disk image; write + readback |
| R-BLK-5 | `phase3_virtio_blk_abandoned_io_no_leak` | BUILD-1 SC-1B | `retained_dma=3`, `live_before=3`, `live_after=3` |
| R-VFS-1 | VFS stdout namespace rule | BUILD-1 SC-1C | `sys_write(1, ...)` routes to serial; fd=1 not consumed by VFS open |
| R-SMP-1 | `smp_cross_cpu_rendezvous_stress` at SMP=4 | Phase 2 anti-regression | `iterations=1000`, `send_first=500`, `recv_first=500`, `cpus=4` |
| R-AARCH64-1 | `phase3_aarch64_virtio_barrier_lane` | BUILD-1 aarch64 lane | `runtime=aarch64`, `compile_only=false`, `publish_descriptors=2`, `before_notification_check=2` |
| R-BUILD-1 | Full non-explicit SMP sweep | BUILD-1 full sweep | `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` exit status 0 |

---

## 6. Regression Gates — Inherited from BUILD-2

These gates must still pass after every BUILD-3 sub-commit to prove NeoFS native v1 correctness is preserved.

| Gate ID | Scenario | Source | Required Evidence |
|---|---|---|---|
| R-MNT-1 | `phase3_neofs_mount_readonly_root` | BUILD-2 2B | Mount selects superblock 0, generation 1; read `hello.txt`; observe `model.neom` |
| R-MNT-2 | `phase3_neofs_mount_selects_highest_generation` | BUILD-2 2B | Selects slot 2, generation 3 |
| R-MNT-3 | `phase3_neofs_mount_rejects_bad_superblock` | BUILD-2 2B | Rejects primary copy; selects valid fallback |
| R-FILE-1 | `phase3_neofs_create_small_file` | BUILD-2 2C | Regular-file create, close/reopen/readback, no Directory or Model create |
| R-FILE-2 | `phase3_neofs_write_readback_same_boot` | BUILD-2 2C | 32 KiB write/readback using 8 direct blocks |
| R-FILE-3 | `phase3_neofs_allocator_reuse_guard` | BUILD-2 2C | Distinct live content; no duplicate live block alias |
| R-FILE-4 | `phase3_neofs_reboot_persistence_small_file` | BUILD-2 2C | Two-boot persistence using reused disk image |
| R-COW-1 | `phase3_neofs_cow_write_after_write` | BUILD-2 2D | Old and new physical data blocks are distinct |
| R-COW-2 | `phase3_neofs_crash_mid_overwrite_recovery` | BUILD-2 2D | Pre-superblock-publish interruption recovers committed version A; unpublished B absent |
| R-COW-3 | `phase3_neofs_crash_mid_create_recovery` | BUILD-2 2D | Interrupted create does not expose torn file on reboot |
| R-COW-4 | `phase3_neofs_superblock_rotation_recovery` | BUILD-2 2D | Newer root-invalid slot rejected; older fully mountable generation selected |
| R-FSYNC-1 | `phase3_neofs_fsync_noop_write_through` | BUILD-2 2D | `fsync_rc=0`, no intent log, no FLUSH command, no generation mutation from fsync itself |
| R-TYPE-1 | `phase3_neofs_typed_inode_exec_refusal` | BUILD-2 2E | `Model` read succeeds; executable policy returns `TypeForbidden` |
| R-TYPE-2 | `phase3_neofs_regular_exec_policy_stub` | BUILD-2 2E | Regular executable policy succeeds with execute right |
| R-TYPE-3 | `phase3_neofs_type_vs_rights_policy_errors` | BUILD-2 2E | Four-corner matrix: type-first / rights-second semantics |
| R-CAP-1 | `phase3_neofs_filecap_generation_stale` | BUILD-2 2E | Stale cap returns `ESTALE=-116`; checks current reachable generation |
| R-CAP-2 | `phase3_neofs_reserved_type_refusal` | BUILD-2 2E | Reserved/future type cannot acquire executable access |
| R-CAP-3 | `phase3_neofs_filerights_u32_forward_bits` | BUILD-2 2E | Kernel and libneo `FileRights` sizes are 4 bytes; high forward bits survive |
| R-SMP-2 | Full non-explicit SMP sweep | BUILD-2 close addendum | `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` exit status 0 |

---

## 7. Anti-Scope Assertions

These assertions prove BUILD-3 did not creep into BUILD-4/5 territory or introduce prohibited mechanisms. Each must be verifiable by code inspection or test output.

| Gate ID | Assertion | Verification Method |
|---|---|---|
| A-BUILD4-FS | No foreign FS (FAT32/ext4) support | No FAT32 parser; no ext4 superblock reader; no foreign-FS mount path |
| A-BUILD4-SYS | No `sys_mount`/`sys_umount` syscall surface | No mount/umount syscall handlers; no multi-mount table beyond single NeoFS root |
| A-BUILD4-DIR | No directory enumeration (`mkdir`, `rmdir`, `unlink`, `rename`, `readdir`) | No directory create/remove/unlink/rename syscalls; no cursor-based readdir |
| A-BUILD5-AHCI | No AHCI driver | No `kernel/src/drivers/storage/ahci.rs` or PCI class 0x0106 probe |
| A-BUILD5-NVME | No NVMe driver | No `kernel/src/drivers/storage/nvme.rs` or PCI class 0x0108 probe |
| A-ROOTFS | No rootfs pivot | Limine modules remain the earliest root; NeoFS is mounted as additional filesystem |
| A-MMAP | No file-backed mmap/exec implementation | Typed-inode policy helper exists (from BUILD-2); real file-backed mmap/ELF loading not implemented |
| A-RETYPE | No retype operation | No runtime inode type change; generation counter and RETYPE bit reserved but unused |
| A-CAPGRANT | No FileCap grant/derive/revoke semantics | FileCap rights set at creation/open only; no runtime rights mutation |
| A-POSIX | No broader POSIX surface (ACLs, quotas, locks) | No POSIX ACL, quota, or lock syscalls |
| A-MULTITHREAD | No multi-thread-per-process | Single-thread-per-process invariant preserved; kthreads are kernel-owned workers, not user-visible threads |

---

## 8. PM Lean Summary

From `Plans/PM-DECISION-2026-05-14-phase-3-build-3-opening.md` §4:

1. Do not start CAS first.
2. Treat the likely BUILD-3 first slice as a kthread ownership/lifecycle prerequisite if Jarvis confirms the gap.
3. Separate "write-back cache + FLUSH durability" from "CAS dedup" unless Cerebro proves they must land together.
4. Preserve BUILD-2 `sys_fsync` no-op semantics until a specific BUILD-3 dispatch changes it to a durable write-back/FLUSH contract.

---

## 9. Pending Artifacts — Cross-References

| Artifact | Purpose | Status |
|---|---|---|
| `Plans/DISPATCH-2026-05-14-phase-3-build-3-evidence-matrix-prometheus.md` | This dispatch mandate | CONSUMED |
| `Plans/PM-DECISION-2026-05-14-phase-3-build-3-opening.md` | PM decision opening BUILD-3 observe gate | CONSUMED |
| `Plans/CLOSE-REPORT-2026-05-14-phase-3-build-2.md` | BUILD-2 close report (regression gate source) | CONSUMED |
| `Plans/PHASE-3-THINK-V2-2026-05-11.md` | THINK v2 — BUILD-3 scope, entry gate, measurement-gated CAS | CONSUMED |
| `Plans/DISPATCH-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md` | Jarvis prerequisite audit dispatch | DISPATCHED |
| `Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md` | **Jarvis observe audit return** | **[PENDING]** |
| `Plans/DISPATCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md` | Cerebro structural challenge dispatch | DISPATCHED |
| `Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md` | **Cerebro structural challenge return** | **[PENDING]** |
| `docs/ADR-021-phase-3-design-substrate.md` | Canonical Phase 3 design substrate | REFERENCE |
| `docs/specs/neofs-format.md` | NeoFS v1 on-disk format specification (BUILD-2) | REFERENCE |

---

## 10. Formatting Caveat (inherited from BUILD-2)

`rustfmt` is installed for the pinned nightly, but global `cargo fmt --check` fails on broad pre-existing formatting drift across the repo. Scoped rustfmt for new BUILD-3 source should pass. Global formatting is a non-blocking style debt carried forward from BUILD-1 through BUILD-2. This shell does NOT claim global `cargo fmt --check` PASS.

---

## 11. Stop Conditions

Do NOT claim BUILD-3 closed if any of the following hold:

- Jarvis prerequisite audit (`OBS-JARVIS`) has not been filed and consumed by PM;
- Cerebro structural challenge (`RES-CEREBRO`) has not been filed and consumed by PM;
- Kthread ownership/teardown evidence is missing — no handle identity, stop flag, or unmount-time worker reclamation proof;
- `sys_fsync` still behaves as a BUILD-2 no-op (returns 0 without flushing dirty buffers through FLUSH path);
- FLUSH negotiation evidence is missing — `F_FLUSH_negotiated` not observed in serial log;
- Write-back cache evidence is missing — no dirty-buffer tracking, no write-back worker execution proof;
- CoW crash/recovery with cached dirty data shows torn state or silent data loss in previously fsynced files;
- Mark-sweep GC evidence is missing — no reachable-block walk, no orphan reclamation proof;
- CAS closure claimed without measurement-gate evidence OR without explicit PM deferral language;
- Any BUILD-1 regression gate (§5) fails;
- Any BUILD-2 regression gate (§6) fails;
- Any anti-scope assertion (§7) is violated (BUILD-4/5 creep, rootfs pivot, mmap/exec, retype, POSIX surface, multi-thread-per-process);
- Global `cargo fmt --check` claimed as PASS when only scoped rustfmt was run;
- BUILD-4 or BUILD-5 scope entered without separate PM authorization.

---

## 12. Caveats

- BUILD-3 is not started. This shell defines evidence slots for future sub-commits. Sub-commit boundaries await Jarvis's observe audit proposal.
- CAS/dedup is measurement-gated per THINK v2 §Q5.a and PM decision §4. No CAS implementation authorized until measurement or PM override.
- The existing `sched::spawn` primitive (`kernel/src/sched/scheduler.rs:231`) answers "does a kernel-thread spawn hook exist?" as **yes**, but does not answer the ownership question — no worker handle, cancellation, join/reap, or unmount teardown. Jarvis audit will determine the gap.
- `sys_fsync` remains a BUILD-2 no-op until a specific BUILD-3 dispatch transitions it to durable write-back semantics.
- The bump allocator from BUILD-2 has no free-space reuse. Orphaned CoW blocks accumulate until mark-sweep GC lands.

---

**End of evidence-matrix shell.**

*PREP ONLY. Does NOT claim BUILD-3 closure. No normative files were edited. No BUILD-3 implementation is authorized. This artifact defines evidence slots, regression gates, and stop conditions for future consumption when Jarvis and Cerebro returns are available.*