# CLOSE REPORT - Phase 3 BUILD-2: NeoFS native v1

**Date:** 2026-05-14
**Author:** PM / Codex
**Status:** CLOSED
**Authority:**
- `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-2-2e-consumption.md`
- `Plans/PHASE-3-BUILD-2-CLOSE-SMP-SWEEP-ADDENDUM-2026-05-14.md`

---

## 1. Closure Summary

Phase 3 BUILD-2 is closed.

NeoFS native v1 now mounts from virtio-blk and supports crash-consistent
Regular-file create/read/write over the BUILD-1 synchronous write-through block
substrate. BUILD-2 also closes the typed-inode and FileCap-generation gates:
`Model` inodes are readable but fail executable policy checks with
`TypeForbidden`, Regular executable policy succeeds only with the execute right,
and stale FileCaps are rejected with `ESTALE = -116` using current reachable
inode generation.

BUILD-2 does not introduce BUILD-3 mechanisms. No CAS, dedup, write-back cache,
virtio FLUSH negotiation, mark-sweep GC, kthread, file-backed mmap/exec,
runtime mkfs, retype, or file-cap grant/derive/revoke semantics were started.

The implementation branch is `codex/phase-3-build-2-2e-typed-inode-filecap`,
with close evidence HEAD at `d49f9ab`.

---

## 2. Commit Series

| SHA | Sub-commit | Description |
|---|---|---|
| `d49f9ab` | close evidence | Record clean full non-explicit SMP sweep PASS |
| `f6532cb` | 2E | Enforce NeoFS typed FileCaps |
| `a2f1e49` | 2D | Harden NeoFS CoW recovery |
| `d2c8810` | 2C | Add NeoFS create-write path |
| `309f28e` | 2B | Mount NeoFS images read-only |
| `f781957` | 2A | Add NeoFS v1 mkfs skeleton |

Base from BUILD-1 docs integration: `e5af108`.

No merge to `main` and no push are claimed by this close report.

---

## 3. Verification Artifacts

| Sub-commit | Artifact | Status |
|---|---|---|
| 2A | `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2A-VERIFICATION-2026-05-13.md` | PASS; accepted |
| 2B | `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2B-VERIFICATION-2026-05-13.md` | PASS; accepted |
| 2C | `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2C-VERIFICATION-2026-05-13.md` | PASS; accepted |
| 2D | `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2D-VERIFICATION-2026-05-13.md` | PASS; accepted |
| 2E | `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2E-VERIFICATION-2026-05-13.md` | 2E PASS; close initially pending SMP sweep |
| Close addendum | `Plans/PHASE-3-BUILD-2-CLOSE-SMP-SWEEP-ADDENDUM-2026-05-14.md` | Full `NEO_SMP=4 --no-meta` sweep PASS |

Sidecar artifacts consumed:

- `Plans/RESEARCH-2026-05-13-phase-3-build-2-2e-policy-generation-cerebro.md`
- `Plans/PREP-2026-05-13-phase-3-build-2-close-evidence-prometheus.md`
- `Plans/PM-NOTE-2026-05-13-phase-3-build-2-2e-sidecar-consumption.md`

---

## 4. Capability Summary

BUILD-2 delivers:

- NeoFS v1 on-disk format and deterministic host mkfs tool.
- Redundant superblock copies with BLAKE3 checksums and highest-valid-generation
  selection.
- Kernel read-only and read-write NeoFS mount from virtio-blk.
- Narrow `/neofs/<name>` Regular-file creation through `OPEN_CREATE_REGULAR`.
- Small-file read/write up to 8 direct 4 KiB blocks.
- Monotone bump allocator through `next_free_block`; no free-space reuse.
- CoW write ordering and mount-time root/child validation.
- `SYS_FSYNC = 15` as a documented no-op over synchronous write-through I/O.
- Typed-inode executable policy helper; no real file-backed mmap/exec.
- FileCap generation capture and stale-handle rejection with `ESTALE = -116`.
- FileRights remains `u32` with forward reserved bits representable.

---

## 5. Runtime Evidence

### 5.1 Format and Mount

- 2A deterministic mkfs image hash:
  `dd845cb0e9a4065e3d548274f4259da948ee4762d8335eb620ddad55175b1c85`.
- 2B read-only mount PASS:
  `phase3_neofs_mount_readonly_root` selected superblock 0, generation 1,
  read `hello.txt`, observed `model.neom`, and denied read-only write.
- Highest-generation selection PASS:
  `phase3_neofs_mount_selects_highest_generation` selected slot 2,
  generation 3.
- Bad superblock rejection PASS:
  `phase3_neofs_mount_rejects_bad_superblock` rejected primary copy and
  selected a valid fallback.

### 5.2 Native File I/O

- `phase3_neofs_create_small_file` PASS:
  Regular-file create, close/reopen/readback, no Directory or Model create.
- `phase3_neofs_write_readback_same_boot` PASS:
  32 KiB write/readback using 8 direct blocks.
- `phase3_neofs_allocator_reuse_guard` PASS:
  distinct live content, bump allocation, no duplicate live block alias.
- `phase3_neofs_reboot_persistence_small_file` PASS:
  two-boot persistence using the reused disk image.

### 5.3 CoW, Recovery, and Fsync

- `phase3_neofs_cow_write_after_write` PASS:
  old and new physical data blocks are distinct.
- `phase3_neofs_crash_mid_overwrite_recovery` PASS:
  pre-superblock-publish interruption recovered committed version A; unpublished
  version B remained absent.
- `phase3_neofs_crash_mid_create_recovery` PASS:
  interrupted create did not expose a torn file on reboot.
- `phase3_neofs_superblock_rotation_recovery` PASS:
  newer root-invalid slot was rejected; older fully mountable generation was
  selected.
- `phase3_neofs_fsync_noop_write_through` PASS:
  `fsync_rc=0`, no intent log, no FLUSH command, no generation mutation from
  fsync itself.

### 5.4 Typed Inode and FileCap

2E PASS frames include:

- `phase3_neofs_typed_inode_exec_refusal`:
  `Model` read succeeds and executable policy returns `TypeForbidden` with no
  real mmap/exec.
- `phase3_neofs_regular_exec_policy_stub`:
  Regular executable policy succeeds with execute right.
- `phase3_neofs_type_vs_rights_policy_errors`:
  four-corner matrix proves type-first / rights-second semantics.
- `phase3_neofs_filecap_generation_stale`:
  stale cap returns `ESTALE=-116` and checks current reachable generation.
- `phase3_neofs_reserved_type_refusal`:
  reserved/future type cannot acquire executable access.
- `phase3_neofs_filerights_u32_forward_bits`:
  kernel and libneo `FileRights` sizes are 4 bytes; high forward bits survive
  request/observation.

---

## 6. Regression Evidence

The 2E verification reran the required 2D/2C/2B/BUILD-1 targeted regression
lane and recorded PASS frames for:

- `phase3_neofs_fsync_noop_write_through`
- `phase3_neofs_cow_write_after_write`
- `phase3_neofs_crash_mid_overwrite_recovery`
- `phase3_neofs_superblock_rotation_recovery`
- `phase3_neofs_crash_mid_create_recovery`
- `phase3_neofs_create_small_file`
- `phase3_neofs_write_readback_same_boot`
- `phase3_neofs_allocator_reuse_guard`
- `phase3_neofs_reboot_persistence_small_file`
- `phase3_neofs_mount_readonly_root`
- `phase3_neofs_mount_selects_highest_generation`
- `phase3_neofs_mount_rejects_bad_superblock`
- `phase3_block_vfs_smoke`
- `phase3_virtio_blk_raw_rw_smoke`

The final close addendum records the full non-explicit SMP sweep:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Exit status: `0`.

The prior `phase3_virtio_blk_abandoned_io_no_leak` terminal failure did not
reproduce. In the closing sweep, xtask retried a transient QEMU timeout and the
scenario passed on attempt 2 with:

```text
NEO_TEST/1 event=PASS name=phase3_virtio_blk_abandoned_io_no_leak bdf=0x000300 retained_dma=3 live_before=3 live_after=3 late_wake=2 completions_delta=1 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
```

The full sweep also included the aarch64 runtime barrier lane:

```text
NEO_TEST/1 event=PASS name=phase3_aarch64_virtio_barrier_lane runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true ... data_integrity=ok publish_descriptors=2 before_notification_check=2 ... compile_only=false sentinel=runtime_qemu_aarch64
```

---

## 7. Build And Tooling

2E artifact records PASS for:

- `cargo check -p xtask`
- `cargo check -p neofs-mkfs`
- `cargo check -p libneo -p neofs-mkfs`
- `cargo k`
- `cargo k --no-default-features`
- six targeted 2E kernel feature checks
- `git diff --check`
- `git diff --cached --check`

Scoped rustfmt was applied to all Rust files changed by 2E. This close report
does not claim global `cargo fmt --check` over the full repository. Inherited
warnings/dead-code noise remain outside BUILD-2 closure.

---

## 8. Caveats

- `SYS_FSYNC = 15` is a BUILD-2 no-op over write-through synchronous I/O. The
  BUILD-3 write-back/cache lane owns intent-log or FLUSH semantics.
- The executable policy is a VFS/kernel policy helper. Real file-backed mmap,
  ELF loading from NeoFS, and disk-backed exec are not implemented.
- The allocator is a monotone bump allocator. Orphaned CoW blocks accumulate
  until BUILD-3 mark-sweep GC.
- Reserved/future type evidence is deterministic rejection, not broad inert
  directory enumeration.
- The close sweep completed with transient QEMU timeout retries that ultimately
  passed and returned exit status 0.

---

## 9. Deferred Work

Not started in BUILD-2:

- BUILD-3 CAS, dedup, content-address index, write-back cache, FLUSH
  negotiation, mark-sweep GC, and kthreads.
- BUILD-4 foreign filesystem read-only support, `sys_mount`, `sys_umount`,
  cursor-based `sys_readdir`, unlink, rename, and broader POSIX surface.
- BUILD-5 AHCI, NVMe, bare-metal storage validation, and retype.
- Rootfs pivot from Limine modules to NeoFS.
- Phase docs, `Plans/INDEX.md`, ADR, or Bible updates. Those require a
  separate post-close dispatch.

---

## 10. Closure Statement

Phase 3 BUILD-2 is CLOSED.

NeoFS native v1 is mounted from virtio-blk with crash-consistent file
create/read/write. CoW ordering, redundant superblock validation, and
mount-time root checking provide recovery without an intent log or journal.
Typed-inode enforcement rejects `Model` executable policy through
type-first/rights-second dispatch. FileCap generation checks reject stale
handles with `ESTALE`. BUILD-1 block/VFS, aarch64 barrier, and full
non-explicit SMP regression gates pass.

No BUILD-3 mechanism was introduced. No normative docs or index updates were
edited by this report. BUILD-3 is not started.
