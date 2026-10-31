# Phase 3 - NeoFS, Block Layer, Real Disk I/O

| | |
|---|---|
| **Status** | In progress. BUILD-1, BUILD-2, BUILD-3, and BUILD-4A are closed; BUILD-4B is authorized next. |
| **Started** | 2026-05-11 |
| **Closed** | - |
| **Active ADRs** | ADR-021: Phase 3 NeoFS, block layer, and real disk I/O substrate |
| **BUILD-1 close report** | `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md` |
| **BUILD-2 close report** | `Plans/CLOSE-REPORT-2026-05-14-phase-3-build-2.md` |
| **BUILD-3 close report** | `Plans/CLOSE-REPORT-2026-05-16-phase-3-build-3.md` |

---

## Scope

Phase 3 moves NeoPlutarch from memory-only substrate work into durable storage.
ADR-021 defines the phase split: BUILD-1 establishes the block/VFS substrate,
BUILD-2 is the first NeoFS-native build, and BUILD-3 adds the durability layer
above NeoFS: kthreads, virtio FLUSH, free-space GC, write-back cache, and
durable fsync.

BUILD-1, BUILD-2, BUILD-3, and BUILD-4A are closed. The BUILD-4 stability
blockers found after BUILD-4A are closed. BUILD-4B is authorized as a narrow
mount-table and directory-capability mount-syscall lane; FAT32 and ext4 remain
deferred until the mount substrate is green.

## BUILD-1 - Block Layer, Virtio-blk, and Minimal VFS

BUILD-1 closed with the following substrate landed:

- Block-device registry and typed block error surface.
- Arc-managed DMA buffers retained through asynchronous storage completion.
- Virtio-blk write-through raw read/write path.
- Virtio feature policy where `F_FLUSH` is offered but not negotiated and
  `CONFIG_WCE` is not negotiated.
- Minimal VFS/testfs layer with file handles as capabilities.
- `sys_open`, `sys_read`, `sys_write`, and `sys_close`.
- `libneo` wrappers for the file syscalls.
- Serial stdout compatibility for `sys_write(1, ...)` under the explicit
  namespace rule.
- Same-boot block+VFS smoke coverage, storage-completion wake composition, and
  reboot-persistence smoke coverage.
- Dedicated QEMU aarch64 virtio split-ring barrier lane.

The BUILD-1 implementation commits run from `982b16d` through `7f48c85`; the
close report records the full commit and evidence table.

## BUILD-2 - NeoFS Native v1

BUILD-2 closed with NeoFS native v1 mounted from virtio-blk and backed by the
BUILD-1 synchronous write-through block path. The implementation series was:

- `f781957` 2A: NeoFS format spec and deterministic host `neofs-mkfs`
  skeleton.
- `309f28e` 2B: read-only NeoFS mount from mkfs-created virtio-blk images.
- `d2c8810` 2C: narrow `/neofs/<name>` Regular-file create/write/readback path.
- `a2f1e49` 2D: CoW ordering, recovery-safe mount selection, and
  `SYS_FSYNC = 15` no-op semantics.
- `f6532cb` 2E: typed-inode executable policy helper and FileCap
  stale-generation rejection.
- `d49f9ab` close evidence: full non-explicit `NEO_SMP=4 --no-meta` sweep PASS.

BUILD-2 delivered:

- NeoFS v1 on-disk format, redundant superblocks, BLAKE3 validation, and
  highest-valid-generation mount selection.
- Kernel read-only and read-write NeoFS mount over virtio-blk.
- Narrow Regular-file creation through `OPEN_CREATE_REGULAR`.
- Small-file read/write up to the 8 direct-block limit.
- Two-boot persistence for created files.
- CoW write ordering and recovery-safe root/child validation without an intent
  log.
- `SYS_FSYNC = 15` as a documented no-op over synchronous write-through I/O.
- Type-first executable policy where `Model` executable access returns
  `TypeForbidden`.
- FileCap generation capture and stale-handle rejection with `ESTALE = -116`.

## BUILD-3 - NeoFS Durability Layer

BUILD-3 closed with the storage durability layer above BUILD-2 NeoFS native v1.
The implementation series was:

- `e4ac074` 3A: owned kthread lifecycle substrate.
- `a3023a0` 3B: virtio-blk FLUSH negotiation and command path.
- `a7eeb21` 3C: NeoFS free-space metadata and foreground mark-sweep GC.
- `b2a93f2` 3D: NeoFS write-back cache and worker lifecycle.
- `8cd3549` 3E: durable NeoFS `sys_fsync` for Regular files.
- `60319e3` 3E evidence hardening: tightened fsync evidence around mount-wide
  drain, clean fsync, dirty FLUSH, write-back hold, and stale FileCap ordering.
- `c799cec` close evidence: final build/check gates, aarch64 runtime lane, and
  full `NEO_SMP=4 --no-meta` sweep PASS.
- `c68d729` close report: BUILD-3 closed.

BUILD-3 delivered:

- Owned kernel worker substrate with spawn, stop, join, detached worker, and
  parked-worker wake coverage.
- Virtio-blk FLUSH negotiation with `VIRTIO_BLK_T_FLUSH` request/completion
  accounting.
- Explicit policy that `VIRTIO_BLK_F_CONFIG_WCE` remains not negotiated.
- NeoFS free-space metadata and mark-sweep GC with reclaimed-block reuse.
- Legacy BUILD-2 image compatibility through the all-zero superblock extension
  path.
- NeoFS write-back cache with dirty drain, reboot-after-drain persistence, and
  crash-before-drain loss semantics.
- GC/write-back coordination: GC forces clean cache state before mark/sweep.
- Durable `sys_fsync` through the `fsync_regular` insertion point.
- Dirty fsync persistence across reboot with cache drain and virtio FLUSH.
- Clean fsync no-extra-FLUSH behavior from a fully clean mount.
- Stale FileCap rejection before durability work.

BUILD-3F CAS/dedup was not admitted. It remained measurement-gated, and no
duplicate-block workload or ratio artifact was produced to justify starting it.

## Course Corrections

The 1B scheduler lifetime blocker was resolved by the 1B0 scheduler-owned
`ThreadWakeToken` substrate. `IoRendezvous.waiting_thread` now uses
`Option<ThreadWakeToken>` and does not migrate scheduler ownership to
`Arc<Thread>`.

The 1D aarch64 closure blocker was resolved by commit `7f48c85`, which added a
dedicated real QEMU aarch64 barrier-exposure lane instead of treating
compile-only aarch64 evidence as closure evidence.

The BUILD-3E full-SMP timeout blocker was resolved without kernel code changes.
Targeted reruns, a NeoFS 3C/3D cluster rerun, and the final full
`NEO_SMP=4 --no-meta` sweep passed. The accepted classification is
full-suite/QEMU timeout pressure with successful retry behavior, not a
deterministic BUILD-3E fsync correctness failure.

## Verification

BUILD-1 closure evidence is recorded in:

- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1A-VERIFICATION-2026-05-12.md`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-VERIFICATION-2026-05-12.md`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-RUNTIME-ADDENDUM-2026-05-12.md`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B-VERIFICATION-2026-05-12.md`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1C-VERIFICATION-2026-05-12.md`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1D-VERIFICATION-2026-05-12.md`
- `Plans/PHASE-3-BUILD-1-AARCH64-LANE-VERIFICATION-2026-05-12.md`
- `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`

BUILD-2 closure evidence is recorded in:

- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2A-VERIFICATION-2026-05-13.md`
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2B-VERIFICATION-2026-05-13.md`
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2C-VERIFICATION-2026-05-13.md`
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2D-VERIFICATION-2026-05-13.md`
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2E-VERIFICATION-2026-05-13.md`
- `Plans/PHASE-3-BUILD-2-CLOSE-SMP-SWEEP-ADDENDUM-2026-05-14.md`
- `Plans/CLOSE-REPORT-2026-05-14-phase-3-build-2.md`

BUILD-3 closure evidence is recorded in:

- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3A-VERIFICATION-2026-05-14.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3B-VERIFICATION-2026-05-14.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3C-VERIFICATION-2026-05-14.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3D-VERIFICATION-2026-05-14.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-SMP-TIMEOUT-RESOLUTION-2026-05-15.md`
- `Plans/PHASE-3-BUILD-3-CLOSE-EVIDENCE-2026-05-15.md`
- `Plans/CLOSE-REPORT-2026-05-16-phase-3-build-3.md`

The final BUILD-3 close gate included:

- `cargo check -p xtask`
- `cargo check -p neofs-mkfs`
- `cargo k`
- `cargo k --no-default-features`
- `phase3_aarch64_virtio_barrier_lane` with `compile_only=false`
- full `NEO_SMP=4 cargo neo-test --no-meta`, exit `0`

## BUILD-4 - Directory And Mount Substrate

BUILD-4A closed with directory FileCaps and cursor-based `SYS_READDIR = 16`
over testfs and NeoFS. BUILD-4A also established the stable `NeoDirentV1`
record ABI.

Post-BUILD-4A stability work root-caused and fixed two SMP liveness issues:

- virtio-blk completion worker mixed-IF device lock deadlock during NeoFS fsync;
- `WAKE_TOKEN_STATE` mixed-IF test-harness lock deadlock across wake-token
  scenarios.

BUILD-4B is authorized next:

- explicit mount table;
- `FileRights::MOUNT`;
- directory-capability `SYS_MOUNT = 17`;
- directory-capability `SYS_UMOUNT = 18`;
- live-handle accounting so unmount returns `EBUSY` while the mounted filesystem
  is still referenced.

No BUILD-4 foreign filesystem support, AHCI/NVMe, rootfs pivot, file-backed
mmap/exec, broad POSIX expansion, or BUILD-3F CAS/dedup work is authorized by
BUILD-4B.


---

## Phase 3 Formal Close — 2026-05-28

Phase 3 is **CLOSED**. Full close-report: `Plans/CLOSE-REPORT-2026-05-28-phase-3.md`.

### Builds completed after the BUILD-4B authorization recorded above

- **BUILD-4B–4D:** mount table + `SYS_MOUNT`/`SYS_UMOUNT` + live-handle `EBUSY`; FAT32 read-only; ext4 read-only. Formal closure at 4D (`Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4-formal-closure-at-build-4D.md`).
- **BUILD-5a:** AHCI driver and NVMe driver (`Plans/PHASE-3-BUILD-5a-AHCI-CLOSE-EVIDENCE-2026-05-20.md`, `...-NVMe-...`).
- **BUILD-5b:** bare-metal storage validation on the Dell OptiPlex 7080 Micro (`Plans/PHASE-3-BUILD-5b-CLOSE-EVIDENCE-2026-05-28.md`).

### Verification log

- QEMU: green across all builds; BUILD-5b added bare-metal-mode QEMU coverage for the AHCI/NVMe log-sink path (`87a49d1`).
- Bare metal (OptiPlex 7080): AHCI lane green (NeoFS RW + 9202-byte clean log extraction); NVMe lane green (NeoFS RW `bytes=30`; `NEO_LOG_v1` at `0x10000000` with valid header; 9191-byte clean extraction; no `.rodata` leak).

### Incidents resolved on hardware

- APIC-base init race before storage probe — fixed `72a0a41`.
- AHCI log-sink missing publication fence before `PX_CI` doorbell (wrong LBA + kernel `.rodata` DMA'd to disk) — fixed `2fdb7cb` by matching the NVMe release/acquire discipline. QEMU did not reproduce; bare metal was the arbiter.

### Deviations

- BUILD-4 absorbed two unplanned SMP-liveness deadlock fixes before its sub-builds could close.
- BUILD-5b required two unanticipated on-hardware fixes (APIC ordering, AHCI publish barrier).

### Outcome

The Phase 3 storage stack — generic block layer, NeoFS (format/mount/COW/fsync/GC/writeback/durable-fsync), VFS file + mount syscalls with typed inodes and capability gating, read-only FAT32/ext4 interop, and AHCI + NVMe drivers — is complete and verified on real disk on bare metal. Foundation phases 1–3 are done. Phase 4 (network stack) is next per the ledger; the Bible ledger amendment to reflect Phase 2 + Phase 3 completion is a pending PL-tier action.
