# PM DECISION - Phase 3 BUILD-2 planning consumption and 2A authorization

**Date:** 2026-05-13
**Status:** RATIFIED; 2A AUTHORIZED
**Consumes:**
- `Plans/RESEARCH-2026-05-12-phase-3-build-2-neofs-format-cerebro.md`
- `Plans/OBSERVE-2026-05-12-phase-3-build-2-neofs-subcommit-plan-jarvis.md`
- `Plans/PREP-2026-05-12-phase-3-build-2-evidence-matrix-prometheus.md`

---

## 1. Decision

The BUILD-2 PLAN/SPEC lane is consumed.

There is no Level-0 structural reframe. The BUILD-2 question remains:

**What is the smallest native NeoFS v1 that lets the kernel mount a virtio-blk
image and perform crash-consistent native file create/read/write without
cornering BUILD-3 CAS?**

BUILD-2 sub-commit 2A is authorized.

Implementation beyond 2A is not authorized by this decision.

---

## 2. Ratified Narrowings

PM ratifies Cerebro's three Level-1 narrowings:

1. **No intent log in BUILD-2.** The BUILD-1 virtio-blk posture is
   write-through: `F_FLUSH` is offered, `F_FLUSH` is not negotiated, and
   `CONFIG_WCE` is not negotiated. Completed writes are already durable.
   `sys_fsync` is therefore a documented no-op in BUILD-2. Intent-log flush
   semantics move to BUILD-3 when write-back caching exists.
2. **Bump allocator only.** BUILD-2 does not need a free-space manager.
   `next_free_block++` is sufficient. Orphaned CoW blocks accumulate until
   BUILD-3 mark-sweep GC.
3. **Eight direct blocks only.** BUILD-2 files are capped at 8 direct 4 KiB
   blocks, i.e. 32 KiB. The inode reserves one indirect block pointer but it
   must remain zero and unused in BUILD-2.

---

## 3. Evidence Matrix Corrections

Prometheus's evidence matrix is accepted as a closure-gate shell with the
following PM overlays:

- Rows that refer to BUILD-2 per-file intent-log flushing are superseded.
  BUILD-2 evidence for `sys_fsync` is: documented no-op, returns success on a
  valid file cap, preserves the type/rights/stale-cap gates, and relies on
  synchronous write-through persistence already proved by readback and two-boot
  tests.
- Allocator evidence is monotone bump-allocation evidence: no duplicate live
  block, `next_free_block` persistence, and `OutOfSpace`/`ENOSPC` when exhausted.
  No alloc/free cycle or free-list evidence is required.
- Optional-input slots in the matrix that say the Cerebro/Jarvis artifacts are
  "NOT YET FILED" are stale. PM has now consumed both artifacts.
- Any closure template that says "per-file fsync intent-log semantics" must be
  revised before BUILD-2 close to "BUILD-2 fsync no-op over write-through
  synchronous I/O; BUILD-3 owns intent-log semantics."

---

## 4. Format Shape Ratified For 2A

2A should create the canonical BUILD-2 NeoFS format spec and mkfs image
skeleton using this shape:

- little-endian fields throughout;
- BLAKE3 checksums and content hashes;
- 512-byte superblock;
- 256-byte inode;
- 64-byte directory entry;
- three superblock locations where device size permits: offset `0`, `1 MiB`,
  and `64 MiB`;
- root directory inode address and root inode hash in the superblock;
- inode generation sourced from `superblock.next_generation`;
- typed inode discriminant in the inode;
- `content_hash` and physical block address fields both present from v1;
- `reserved_retype_audit[16]`;
- no CAS lookup, no dedup index, no GC, no write-back cache, no kthread.

2A must normalize the wire table before code treats it as binding:

1. Define magic fields as fixed byte arrays, not endian-sensitive `u64`
   literals. PM-approved defaults are `b"NEOFSv1\0"` for the superblock and
   `b"NEOFSINO"` for inodes unless Jarvis files a deviation.
2. Resolve the `next_generation` offset conflict in Cerebro's draft. The
   canonical superblock table should include both `inode_size_bytes: u32` and
   `next_generation: u32` without overlapping the UUID/reserved area.
3. BUILD-2 supports 4 KiB filesystem blocks only. `block_size_log2` is retained
   for forward compatibility, but mkfs should reject non-4096 block sizes in
   2A.
4. The default BUILD-2 test image should be sparse and large enough to exercise
   all three superblock locations. A smaller image path may be supported only if
   tests clearly prove the expected reduced-copy behavior.

Suggested normalized superblock field order:

```text
0   [u8; 8]  magic = b"NEOFSv1\0"
8   u16      format_version = 1
10  u16      block_size_log2 = 12
12  u8       type_version
13  [u8; 3]  flags/reserved
16  u64      total_blocks
24  u64      next_free_block
32  u64      generation
40  u64      root_inode_block
48  [u8;32]  root_inode_hash
80  u32      inode_size_bytes = 256
84  u32      next_generation
88  [u8;16]  volume_uuid/reserved
104 [u8;376] reserved
480 [u8;32]  checksum = BLAKE3(bytes 0..479)
```

---

## 5. 2A Authorization

Jarvis is authorized to implement sub-commit 2A only:

**NeoFS format spec + mkfs image skeleton, no kernel mount.**

Dispatch:

`Plans/DISPATCH-2026-05-13-phase-3-build-2-2a-neofs-format-mkfs-jarvis.md`

2A may create/update:

- `docs/specs/neofs-format.md`
- `tools/neofs-mkfs/**`
- root `Cargo.toml` workspace membership for the mkfs tool
- `Cargo.lock` if dependencies change
- `tools/xtask/src/main.rs` only for host-side mkfs/image sanity orchestration
- a verification artifact under `Plans/`

2A must not edit kernel mount, VFS, syscall, virtio, userland scenario, or QEMU
runtime code except where necessary for host-side image sanity plumbing. Runtime
NeoFS mounting starts in 2B only after PM consumes 2A.

---

## 6. Stop Conditions

Jarvis must stop and file a blocker if:

- BLAKE3 cannot be used in the host mkfs tool without an unapproved substitute;
- the 512-byte superblock cannot fit the normalized required fields;
- deterministic image generation cannot be proved;
- the mkfs tool needs kernel code changes to pass 2A;
- a 2A test requires QEMU/kernel mount;
- the format spec cannot keep the 32 KiB direct-file cap and reserved indirect
  slot coherent;
- any implementation pressure appears to add CAS, dedup, GC, write-back cache,
  kthreads, AHCI/NVMe, foreign FS, rootfs pivot, or broad POSIX expansion.

---

## 7. External Status

Phase 3 BUILD-1: CLOSED and integrated on local `main`.

Phase 3 BUILD-2: 2A authorized only.

