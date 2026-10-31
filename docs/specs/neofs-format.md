# NeoFS v1 Format - Phase 3 BUILD-3

| | |
|---|---|
| **Status** | Implementation-binding for Phase 3 BUILD-2 plus BUILD-3 3C free-space extension |
| **Date** | 2026-05-14 |
| **Authority** | PM decision `Plans/PM-DECISION-2026-05-13-phase-3-build-2-plan-consumption-and-2a-authorization.md` |
| **Scope** | On-disk format, host-side mkfs skeleton, kernel mount/write, and quiescent GC |

---

## 1. Format Goals

NeoFS v1 is the smallest native NeoFS layout needed for Phase 3 BUILD-2 and
BUILD-3. It is single-tier, physical-block-addressed, copy-on-write ready, and
preserves the fields needed for later BUILD-3 content-addressing without
implementing CAS in BUILD-2/3C.

BUILD-3B negotiates `F_FLUSH` while keeping `CONFIG_WCE` disabled. BUILD-3C
uses FLUSH only for NeoFS metadata publication that introduces or updates
persistent free-space metadata. `sys_fsync` remains the BUILD-2 documented
no-op until a later durable-fsync/write-back-cache sub-commit.

All integer fields are little-endian. Magic values are fixed byte arrays, not
endian-sensitive integer literals.

## 2. Constants

| Name | Value |
|---|---|
| Superblock magic | `b"NEOFSv1\0"` |
| Inode magic | `b"NEOFSINO"` |
| Format version | `1` |
| Type version | `1` |
| Filesystem block size | `4096` bytes |
| `block_size_log2` | `12` |
| Superblock size | exactly `512` bytes |
| Inode size | exactly `256` bytes |
| Directory entry size | exactly `64` bytes |
| Direct blocks per inode | `8` |
| BUILD-2 file size cap | `8 * 4096 = 32768` bytes |
| Free-map bits per block | `4096 * 8 = 32768` physical blocks |
| Hash/checksum algorithm | BLAKE3 |

## 3. Superblock

Superblocks are exactly 512 bytes. Each copy is written at a fixed byte offset:

| Copy | Offset | Present when |
|---|---:|---|
| 0 | `0` | always |
| 1 | `1 MiB` | image/device is large enough for this offset |
| 2 | `64 MiB` | image/device is large enough for this offset |

The BUILD-2 2A default mkfs image is sparse and large enough to exercise all
three copies.

### 3.1 Layout

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | `magic: [u8; 8] = b"NEOFSv1\0"` |
| 8 | 2 | `format_version: u16 = 1` |
| 10 | 2 | `block_size_log2: u16 = 12` |
| 12 | 1 | `type_version: u8 = 1` |
| 13 | 3 | `flags_reserved: [u8; 3] = 0` |
| 16 | 8 | `total_blocks: u64` |
| 24 | 8 | `next_free_block: u64` |
| 32 | 8 | `generation: u64` |
| 40 | 8 | `root_inode_block: u64` |
| 48 | 32 | `root_inode_hash: [u8; 32]` |
| 80 | 4 | `inode_size_bytes: u32 = 256` |
| 84 | 4 | `next_generation: u32` |
| 88 | 8 | `free_map_start_block: u64` or `0` for legacy BUILD-2 bump-only |
| 96 | 8 | `free_map_block_count: u64` or `0` for legacy BUILD-2 bump-only |
| 104 | 32 | `free_map_hash: [u8; 32]` or zero for legacy BUILD-2 bump-only |
| 136 | 344 | `reserved: [u8; 344] = 0` |
| 480 | 32 | `checksum: [u8; 32] = BLAKE3(bytes 0..479)` |

The checksum covers bytes `0..480` and excludes bytes `480..512`.

### 3.2 Copy Selection

Mount-time selection scans all present superblock copies. A copy is valid only
if its magic, fixed fields, BLAKE3 checksum, root inode hash, and any referenced
free-map hash are valid. If a free map is present, mount also rejects a map that
marks reachable metadata or file blocks free. The selected superblock is the
valid copy with the highest `generation`. If no copy is valid, mount must fail.

### 3.3 Free-Map Extension

An all-zero `88..136` extension means legacy BUILD-2 bump-only allocation and
must remain mountable. A non-zero extension references one or more contiguous
filesystem blocks containing a raw bitmap:

- bit value `1` means the physical block is free and reusable;
- bit value `0` means allocated, metadata, live, reserved, or virgin beyond
  `next_free_block`;
- the required block count is `ceil(total_blocks / 32768)`;
- the superblock stores `BLAKE3(all free-map bytes)`.

`next_free_block` remains the high-water cursor. Allocation first consumes
free-map bits below the high-water mark, then falls back to bump allocation.

### 3.4 Quiescent GC and Publication

BUILD-3 3C GC is a quiescent, explicit internal operation. It marks the
selected superblock copies, selected free-map blocks, root inode tree, directory
entry child inodes, and all reachable direct data blocks. It sweeps unreachable
allocated blocks below `next_free_block` into a newly computed free map.

GC and every other commit publish in this order (`kernel/src/fs/neofs/commit_logic.rs`):

1. write the new data, inode, directory and free-map blocks, each to a new physical location;
2. issue a block-device FLUSH (the **barrier**): no superblock copy is written while any of those blocks may still
   be in the device's volatile write cache;
3. write valid superblock copies pointing at the new root, map and hash;
4. issue a FLUSH; only then treat the in-memory state as committed.

A FLUSH that fails or doesn't complete stops the commit before step 3, so the old copies stay valid. A device that
can't FLUSH is never mounted writable.

### 3.5 Reuse of Freed Blocks

Mount verifies the file contents a candidate copy names (§3.2), so an older copy that a cut can bring back must
still find its blocks intact. A block a commit frees (a truncate releases the old tree) is marked free in that
commit's map, but is not allocated again until every present superblock copy holds that commit's generation or a
later one, as proven by a completed FLUSH. Until then, the copies that may still be selected all name it.

## 4. Inode

Inodes are exactly 256 bytes. An inode is addressed by its physical filesystem
block number. There is no inode table in BUILD-2.

### 4.1 Type Discriminants

| Value | Type | BUILD-2 behavior |
|---:|---|---|
| `0` | `Regular` | normal small file |
| `1` | `Directory` | directory entries in direct blocks |
| `2` | `Symlink` | type reserved; following is out of scope |
| `3` | `Model` | readable typed model file; executable mapping must be refused in later enforcement |
| `4..=127` | reserved | reject as unsupported in BUILD-2 |
| `128..=255` | future/private | reject as unsupported in BUILD-2 |

Type is an inode field. Filename and file extension never determine type.

### 4.2 Layout

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | `magic: [u8; 8] = b"NEOFSINO"` |
| 8 | 1 | `kind: u8` |
| 9 | 1 | `type_version: u8 = 1` |
| 10 | 2 | `flags: u16 = 0` |
| 12 | 4 | `generation: u32` |
| 16 | 8 | `file_size: u64` |
| 24 | 8 | `block_count: u64` |
| 32 | 64 | `direct_blocks: [u64; 8]` |
| 96 | 8 | `single_indirect_block: u64` |
| 104 | 32 | `content_hash: [u8; 32]` |
| 136 | 8 | `double_indirect_block: u64` |
| 144 | 8 | `triple_indirect_block: u64` |
| 152 | 4 | `available_rights_bits: u32` |
| 156 | 4 | `directory_entry_count: u32` |
| 160 | 64 | `reserved: [u8; 64] = 0` |
| 224 | 32 | `checksum: [u8; 32] = BLAKE3(bytes 0..223)` |

The inode checksum covers bytes `0..224` and excludes bytes `224..256`.

### 4.3 Direct and Indirect Blocks

The first eight logical file blocks use `direct_blocks`. Larger files are
addressed by 4096-byte pointer blocks containing 512 little-endian `u64`
physical filesystem block numbers:

- `single_indirect_block` covers logical blocks `8..520`.
- `double_indirect_block` covers the next `512 * 512` logical blocks through a
  root pointer block whose entries point to leaf pointer blocks.
- `triple_indirect_block` covers the remaining address space through
  root -> level-2 -> leaf pointer blocks.

Unused direct slots and unused indirect roots must be zero. Required data and
metadata block pointers must be non-zero and less than the superblock's
`total_blocks`. Kernel and mkfs readers must fail closed on any out-of-range
pointer.

The current NeoFS v1 maximum file size is 2 GiB. Kernel writes through an open
file (`write_at`) still allocate only direct blocks (files of at most 32 KiB).
With `neofs-large-write` (NFS-W1) the kernel can also create a file of any size
up to that limit in one piece (`NeoFs::create_file_from_bytes`):

- its `n` data blocks and its pointer blocks are allocated as one contiguous
  run, data first, so logical block `i` is block `data + i` and each pointer
  block follows from its position (`kernel/src/fs/neofs/tree_logic.rs`);
- pointer blocks are numbered in this order: the single-indirect root; the
  double-indirect root, then its leaves; the triple-indirect root, then each
  level-2 block followed by its leaves;
- the data and pointer blocks are written to the device before the inode and
  its directory entry are committed, with a flush between, so nothing refers to
  them until that one commit;
- the files it writes are ordinary v1 files: the kernel's reader, mkfs's
  `verify` and `tools/neofs-inspect.py` all read them (the `neofs_large_write`
  scenario checks all three).

### 4.4 Content Hash

`content_hash` is `BLAKE3(logical file bytes)`.

For a regular or model file, the logical file bytes are `file_size` bytes from
the direct/indirect logical block walk. For a directory, the logical file bytes
are the packed directory entries. For an empty file or empty directory, the
content hash is `BLAKE3(empty)`.

### 4.5 Inode Generation

`superblock.next_generation` is the source for inode generations. Each new
inode receives the current `next_generation`; the superblock value is then
incremented. Generation zero is invalid.

BUILD-2 uses generation fields for future FileCap stale-handle checks. Retype
is out of scope for NeoFS v1.

## 5. Directory Entry

Directory entries are exactly 64 bytes and are packed into directory data
blocks.

| Offset | Size | Field |
|---:|---:|---|
| 0 | 48 | `name: [u8; 48]` |
| 48 | 8 | `child_block: u64` |
| 56 | 1 | `kind: u8` |
| 57 | 1 | `reserved0: u8 = 0` |
| 58 | 6 | `reserved1: [u8; 6] = 0` |

`name` is UTF-8 bytes followed by a NUL terminator and then zeros. The maximum
filename payload is 47 bytes. Mkfs must reject longer names.

`child_block = 0` marks an unused slot. The `kind` field duplicates the child
inode type for cheap directory validation, but the child inode remains the
source of truth.

## 6. Mkfs Image

The mkfs skeleton must produce a deterministic sparse image containing:

- a root `Directory` inode;
- `hello.txt` as a `Regular` inode with deterministic contents;
- `model.neom` as a `Model` inode with deterministic contents;
- `subdir` as an empty `Directory` inode;
- `.arthur/memory` as an empty `Directory` inode subtree reserved for
  Arthur memory M1 (`*.mem` record files are created only when the
  `arthur-memory` userland feature is enabled);
- BUILD-3 3C free-map metadata in new images;
- valid superblock copies at `0`, `1 MiB`, and `64 MiB` in the default image;
- zeroed reserved fields;
- zeroed indirect-root fields for direct-only files;
- valid BLAKE3 superblock checksums, inode checksums, content hashes, and
  `root_inode_hash`.

The root inode hash in the superblock is `BLAKE3(root inode bytes 0..256)` after
the root inode checksum has been written.

## 7. BUILD-3 3C Anti-scope

The following remain explicitly out of scope for BUILD-3 3C:

- CAS or hash-as-address lookup;
- deduplication;
- write-back cache;
- durable `sys_fsync` behavior changes;
- kthreads, background GC, or background writeback;
- AHCI or NVMe;
- foreign filesystems;
- rootfs pivot;
- broad POSIX syscall expansion;
- symlink following;
- hard links;
- FileCap grant/derive/revoke semantics for files;
- retype operations;
- public `sys_gc`.
