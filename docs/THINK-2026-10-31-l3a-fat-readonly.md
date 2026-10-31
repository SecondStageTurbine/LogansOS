<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: L3a, FAT read-only from a block device (2026-10-31)

Lane 3, first rung (`docs/THINK-2026-10-31-lived-in-os-lanes.md` §5). This THINK is the trap-mining pass and the
design choice; it goes to the PM for a read before any build.

## 1. What is already there (the lanes THINK missed this)

§5 of the lanes THINK says the vendored `rust-fatfs` is used only by the host image builder. That is true, but the
kernel **already mounts FAT32 read-only**, with its own driver:

- `kernel/src/fs/fat32.rs` (591 lines, Phase 3 BUILD-4C): a hand-written read-only FAT32 reader behind the
  `MountedFs`/`Inode` boundary. It handles LFN (with checksum), the NT lowercase flags, the `0x05` → `0xE5` first-byte
  rule, cluster-chain cycle bounding, and rejects writes.
- `kernel/src/fs/mod.rs` `vfs_mount_at_parent`: `fs_type == 3` mounts it from `"deviceN"`. WRITE is always denied,
  and EXECUTE too unless `MOUNT_FLAG_EXEC_ALLOWED` is set.
- `SYS_MOUNT`/`SYS_UMOUNT` already check the mount authority (`mount-authority` feature, #240) via
  `caller_holds_mount_authority()`. Nothing mounts FAT at boot.
- Nine gate scenarios (`phase3_fat32_*` in `tools/xtask/src/main.rs`): mount, readdir, open/read, LFN lookup,
  subdir walk, write rejected, exec blocked by default, exec with opt-in, type import is Regular.

So "mount FAT read-only through the VFS mount table, behind the mount authority, not at boot" is **already met for
FAT32 on a whole device.** L3a is closing the gaps between that and "read an ordinary stick or card".

## 2. The gaps

Found by reading `fat32.rs`, `fs/mod.rs` and `drivers/block/` at `07d7f6e`.

| # | Gap | Where | Effect on a real volume |
|---|---|---|---|
| G1 | FAT type comes from the BPB's shape (`root_entry_count == 0`, `sectors_per_fat_16 == 0`), not from the cluster count | `Fat32::mount_readonly` | FAT12 and FAT16 are refused outright. Cards of 2 GB and under, and many small sticks, are FAT16. |
| G2 | Always reads FAT #0; ignores `BPB_ExtFlags` (bit 7 = mirroring off, bits 0–3 = active FAT) | `mount_readonly` | A volume with mirroring off and FAT #1 active is read through a stale table: wrong chains, garbage files. |
| G3 | The whole FAT is read into a `Vec<u32>` sized from `sectors_per_fat` (untrusted), not capped to `data_clusters + 2` | `mount_readonly` | A crafted BPB asks for a very large allocation before any other check fails. A large real volume costs megabytes of kernel heap per mount. |
| G4 | `read_at` re-reads the whole cluster chain into memory on **every** call; `readdir_at` re-reads the whole directory on every cursor step | `Fat32Inode::read_at`, `readdir_at` | Reading a file in N chunks is O(N × size) device I/O and holds the whole file in heap. A multi-GB file can't be read at all. |
| G5 | Logical block size and `BPB_BytsPerSec` must be 512 | `mount_readonly` | 4Kn media and FAT volumes with 1024/2048/4096-byte sectors are refused. Rare on sticks; refusing them is acceptable for L3a if it is a clean `Invalid`. |
| G6 | `BPB_FSVer` not checked (must be 0); FSInfo not read | `mount_readonly` | Harmless for read-only (FSInfo is advisory and never trusted for reads). FSVer ≠ 0 should be refused, as the spec says. |
| G7 | Source is a whole device only (`"deviceN"`); partitions are GPT-only (`drivers/block/partition.rs`) | `parse_block_device_source`, `partition.rs` | Almost every stick and card has an **MBR** table with one FAT partition at LBA 2048 or 63. It can't be mounted. |
| G8 | No USB mass-storage driver. Block devices are AHCI, NVMe and virtio-blk only | `drivers/block/` | A USB stick never shows up as a block device at all. SD cards behind a USB reader neither. |

G7 and G8 are not FAT work. G8 is a whole driver (USB Bulk-Only Transport + SCSI) and needs its own trap-mining and
I-14 analysis. L3a can be proven on virtio-blk and AHCI images in the gate, and on the Zenbook's internal SD reader
only if that reader is PCIe SDHCI or a USB reader (it would need its own driver either way). **"Read an ordinary USB
stick" is L3a + MBR (G7) + USB MSC (G8).** G8 is its own lane (§7, PM decision); G7 is in L3a (§6a).

## 3. Prior-art survey (checked 2026-10-31)

| Candidate | Licence | Last commit | Fit | Take |
|---|---|---|---|---|
| rafalh/rust-fatfs (vendored, `third_party/fatfs`) | MIT | `38b3a79`, 2026-10-16 (active) | `no_std` + `alloc`, own `Read/Write/Seek` traits in `io.rs`, FAT12/16/32, LFN, streams files cluster by cluster, never loads the whole FAT. Read-write, which L3b needs. | **The code** (already vendored, MIT-compatible). |
| Our `kernel/src/fs/fat32.rs` | MPL-2.0 | on `main` | FAT32 only, read-only, the gaps above. | Its **scenarios** (the nine `phase3_fat32_*`) as the regression oracle. |
| Linux `fs/fat/` (vfat, msdos) | GPL-2.0 | not checked: principle only | Read for principle only, never copied. Source of the traps in §4. | Nothing (principle only). |
| ChaN FatFs (elm-chan.org) | FatFs licence (BSD-style) | not checked: not a code candidate | C; would need FFI. | Nothing; rust-fatfs covers it in Rust. |
| Microsoft FAT32 File System Specification v1.03 (2000), and the exFAT spec (2019) | spec | n/a | The normative source for type detection, ExtFlags, FSVer, dirty bits. | The rules in §4, with bytes taken from the raw spec PDF, not a summarizing fetch. |

## 4. Trap mining: documented field gotchas, each with a discriminator

Each trap gets a scenario or host KAT that fails if we fall into it. Host KATs run in push-CI on images built by
the vendored `fatfs` (or `mkfs.fat` from `dosfstools` where a non-default layout is needed); gate scenarios mount the
same images through `SYS_MOUNT`.

| T | Trap (source) | Discriminator |
|---|---|---|
| T1 | **The FAT type is decided only by the count of data clusters**: < 4085 FAT12, < 65525 FAT16, else FAT32 (MS FAT spec, "FAT Type Determination"). `mkfs.fat` can make "FAT32" volumes under 65525 clusters, which Windows reads as FAT16. rust-fatfs, in strict mode, refuses a BPB whose shape disagrees with the count. | Host KAT: one FAT12, one FAT16 and one FAT32 image each mount with the right `fat_type()`; a FAT32-shaped BPB with 60 000 clusters is refused with `Invalid`, not mis-read. |
| T2 | **FAT12 entries straddle sector boundaries** (1.5 bytes each; an entry at byte offset 511 spans two sectors). A sector-at-a-time reader that ignores this returns wrong chains only for some files. | Host KAT: a FAT12 image with a file whose chain crosses the first FAT sector boundary reads back byte-exact. |
| T3 | **Active FAT with mirroring off** (`BPB_ExtFlags`, FAT32 only). G2. | Host KAT: an image with ExtFlags = `0x0081` (mirroring off, FAT #1 active) and a deliberately stale FAT #0 reads the file through FAT #1. |
| T4 | **Two dirty flags**, not one: FAT32 keeps "clean shutdown" (bit 27) and "hard error" (bit 26) in FAT[1]; Windows NT and Linux also use the BPB reserved byte (`0x25` on FAT12/16, `0x41` on FAT32). rust-fatfs reads only the BPB byte. Read-only mounts must **never clear or set either**. | Gate scenario: mount a volume marked dirty in both places, read, unmount; then hash sectors 0 and the first FAT sector and assert they are unchanged. Mount still succeeds read-only and the log says the volume is dirty. |
| T5 | **A read-only session must not write.** rust-fatfs writes on `Drop` (`unmount_internal`: FSInfo flush, dirty flag) only if the session changed state, which a pure read doesn't. That holds only because of the library's internals, so we make it mechanical: the kernel's `IO` adapter for an L3a mount returns an error from every `write`. | Unit test on the adapter: `write` errors. Gate scenario: the T4 hash check also covers this. `FsOptions::update_accessed_date(false)` is asserted at the construction site. |
| T6 | **LFN edge cases** (Linux `vfat` namei comments; MS FAT spec, long-name chapter): orphaned LFN slots whose checksum doesn't match the short entry; LFN runs with a missing order number; `0xFFFF` padding after the `0x0000` terminator; names with UTF-16 surrogate pairs; a deleted entry (`0xE5`) in the middle of an LFN run. The rule: on any mismatch, fall back to the 8.3 name, never merge. | Host KAT over a hand-laid-out directory cluster (the byte oracle, laid out by hand, not by the writer): each case decodes to the expected name. Our `fat32.rs` already does checksum fallback; its LFN scenario stays as is. |
| T7 | **Name matching**: lookup is case-insensitive for ASCII (and Windows upcases via its own table for non-ASCII); two entries can differ only in case only on a corrupted volume. | Host KAT: `README.TXT` is found as `readme.txt`; a non-ASCII name is found by exact match. |
| T8 | **Directory and chain corruption**: cycles in the cluster chain; a chain pointing at cluster 0, 1, a bad cluster (`0x?FFFFFF7`) or past the end; a directory with `first_cluster` 0 that isn't `..`; a file whose size is longer than its chain. | Host KAT per case: an error (`Invalid`/`Io`), never a hang or a panic. A file shorter than its size reads the chain and then returns `Invalid` (not zero-fill). The existing cycle bound in `fat32.rs` is the model. |
| T9 | **`..` with cluster 0 means the root** (FAT32 root is a real cluster, but `..` in a first-level directory still stores 0). | Covered by the existing `phase3_fat32_subdir_walk`; we keep refusing `..` in a lookup path as today. |
| T10 | **Heap use from untrusted sizes** (G3, G4). | Host KAT: a BPB with `sectors_per_fat = 0xFFFF_FFFF` fails before any allocation larger than one sector. Reading a file larger than the kernel heap's budget for one request in 4 KiB chunks succeeds (streamed). |
| T11 | **MBR vs superfloppy**: a stick may have no partition table (the FAT boot sector is at LBA 0, which also ends in `0x55AA`). The two look alike; Linux tells them apart by checking the boot sector's BPB fields for sanity before treating sector 0 as an MBR. | Belongs to G7, not L3a. Recorded here so the MBR rung starts from it. |
| T12 | **exFAT is not FAT.** SDXC cards (> 32 GB) ship exFAT. The boot sector says `EXFAT   ` at offset 3 and all BPB fields are zero. | Host KAT: an exFAT boot sector is refused with `Invalid` (clean refusal, no panic). exFAT itself is out of scope. |

## 5. Design: move FAT to the vendored rust-fatfs, keep the scenarios

**Recommendation.** L3a replaces the native `fat32.rs` with the vendored `rust-fatfs` behind a new kernel feature
`fat-fs`, mounted read-only through an adapter that refuses writes. The nine `phase3_fat32_*` scenarios run unchanged
against it and are the regression oracle; new host KATs cover T1–T10 and T12.

Why replace and not extend:

- G1–G4 are each a rewrite of a core piece of `fat32.rs` (type detection, FAT access, streamed reads). Done in our own
  driver, that is a second FAT implementation to maintain next to the vendored one, which L3b needs anyway for writes.
- rust-fatfs already handles T1, T2 and T3 and streams files. We keep one FAT implementation in the tree, upstream
  maintained, used by both xtask (formatting templates) and the kernel.
- The cost is a kernel dependency on a crate that uses `RefCell` internally: a `FileSystem` is not `Sync`. The adapter
  holds it behind the kernel's `Mutex` (one lock per mount, held for one VFS call, never across a block-device wait
  that takes another FS lock). That lock is a new lock; it takes no AI-governance lock (I-14(a)) and is ordered after
  the mount table lock (the build confirms the mount table lock is not held across an FS call).

Pieces:

1. **`kernel/src/fs/fat/` (new):** `BlockIo`, an implementation of rust-fatfs's `Read + Seek` (and `Write` that always
   errors) over `Arc<dyn BlockDevice>`, with a small sector cache (one cluster or a few sectors; rust-fatfs reads the
   FAT one entry at a time, which without a cache is one block request per entry). `FatMount`, the `MountedFs` impl.
   `FatInode`, the `Inode` impl, which keeps a path and re-opens through the `FileSystem` for each call (rust-fatfs's
   `File` borrows the `FileSystem`, so it can't be stored in an `Arc<dyn Inode>`).
2. **`fs_type == 3`** dispatches to `FatMount::mount_readonly` when `fat-fs` is on. The rights mask stays exactly as
   today (WRITE always denied; EXECUTE denied unless the exec flag). Kill-switch: with `fat-fs` off, `fs_type == 3`
   returns `Invalid` and no FAT code is linked (§5 of the lanes THINK). The `neo-test-phase3-fat32*` features imply
   `fat-fs`.
3. **Keep `fat32.rs` as the kill-switch's previous layer (build decision, 2026-10-31).** With `fat-fs` off,
   `fs_type == 3` is the native driver exactly as before (not `Invalid`), so "off ≡ the previous layer" holds bit
   for bit, and a `phase3_fat32_native_control` scenario keeps it armed. Retiring `fat32.rs` is a later
   consolidation, once the swap has run on hardware. The nine scenarios build with `fat-fs` and are the swap's
   regression oracle.
4. **Vendoring:** the kernel needs rust-fatfs with `default-features = false, features = ["alloc", "lfn",
   "unicode"]` (no `std`, no logging). `third_party/fatfs/VENDORED.md` changes its "the kernel never links it" line,
   which was true for OT-87; OT-87's statement that the kernel never formats or rewrites FAT still holds through L3a,
   and L3b revisits it. Clippy and fmt still don't apply to the vendored source.

### Kill-switch proof

- `fat-fs` off: `fs_type == 3` is the native driver, as before; `fs::fat` and the `fatfs` dependency are not
  compiled (`#[cfg(feature = "fat-fs")]`, an optional dependency).
- `fat-fs` on, write path: every `write` reaches the adapter's error; the T4/T5 sector hashes prove nothing was
  written. Bit-for-bit equality to "the previous layer" here means the same nine scenario outputs as before the swap.

### Governance (I-14) and the write guard

- I-14(a): the FAT path runs in syscall context, never in an ISR, and takes no governance lock (grep-checkable).
- I-14(b): N/A; no new interrupt source.
- I-14(c): no new DMA path. Reads go through the existing block drivers and their fenced `DmaBuffer`s.
- The write guard (`GuardedDevice`) is untouched in L3a, because the adapter can't write. L3b routes writes through
  it, and that is where the Zenbook rule (never write the internal Windows NVMe) is re-proven.

## 6. What L3a ships and how it is gated

- Host KATs (push-CI) for T1–T8, T10, T12, on images made by the vendored `fatfs` and, for T3/T4, patched by hand
  (the patched bytes are the byte oracle, written out in the test).
- Host KATs for the MBR view (G7, now part of L3a, §6a): a hand-laid MBR sector written out byte by byte in the test
  (the byte oracle, independent of the parser), plus the T11 superfloppy cases.
- The nine existing gate scenarios, unchanged, plus three new ones: `fat_fat16_mount_read` (a FAT16 image reads back
  byte-exact), `fat_readonly_no_write` (the T4/T5 sector-hash check on a dirty-marked volume) and `fat_mbr_mount_read`
  (an MBR-partitioned virtio disk, FAT32 partition at LBA 2048, mounted as `"deviceN:p1"`, reads back byte-exact).
- Where the volume comes from: until USB mass storage lands (§7), L3a mounts from the block devices that exist today
  (virtio-blk in the gate, AHCI/NVMe on hardware). No USB in L3a.
- Kernel-touching, so the 4-shard gate and an AAG. Not SMP-core, so no cross-family concurrency lens is required; the
  cross-family review is a contained-code review.
- Size: one dependency step (the adapter) and one swap. Uncertainty is low on the FAT side (upstream does the hard
  part); the likely iteration is the adapter's locking and cache, and the `RefCell`/`Arc<dyn Inode>` lifetime shape.

### 6a. MBR partitions (G7), in L3a

PM decision (2026-10-31): MBR parsing belongs in the FAT lane, because a USB stick or SD card is MBR-partitioned. It
ships in L3a, read-only:

- A read-only MBR partition view next to the GPT one in `drivers/block/partition.rs`: the four primary entries only
  (no extended/logical chains yet), FAT types `0x01`, `0x04`, `0x06`, `0x0B`, `0x0C`, `0x0E`. A protective MBR
  (`0xEE`) defers to the GPT path, as today.
- The T11 superfloppy rule: sector 0 is a FAT boot sector, not an MBR, when its BPB is sane (bytes per sector a power
  of two 512..4096, sectors per cluster a power of two, reserved sectors non-zero, FAT count 1 or 2). Then the whole
  device is the volume.
- Entries are checked before use: start LBA non-zero, `start + count` inside the device, no overlap between entries;
  a bad entry is skipped and logged, never mounted.
- Source string `"deviceN:pM"` (M = 1..4); `"deviceN"` keeps today's whole-device meaning.
- Byte oracle: a hand-laid 512-byte MBR (boot code zeroed, one `0x0C` entry at LBA 2048, `0x55AA`) written out as a
  literal array in the host test, asserted against the parser; plus a superfloppy sector and a bad-entry sector.

## 7. After L3a

1. **USB mass storage (G8), its own lane, mine after L3a (PM, 2026-10-31):** Bulk-Only Transport and the SCSI subset
   (INQUIRY, READ CAPACITY(10/16), READ(10), TEST UNIT READY, REQUEST SENSE), on the existing xHCI host. It starts with
   its own prior-art survey and driver trap-mining (stall recovery, CSW tag and residue checks, devices that lie in
   READ CAPACITY, the "last LBA vs block count" off-by-one) and an I-14 analysis, written as its own THINK. Only after
   this does "read an ordinary USB stick" become true.
2. **L3b, read-write** through `GuardedDevice`, as the lanes THINK says. The T4 dirty-bit rule turns into: set both
   dirty flags on the first write, clear them on a clean unmount.

## 8. Decisions (PM, 2026-10-31)

1. MBR (G7) is in L3a (§6a), read-only, with a byte oracle on a hand-laid MBR.
2. USB mass storage (G8) is its own lane, this seat's after L3a, prior-art survey and trap-mining first. Until then L3a
   mounts from the existing block devices or a FAT image on virtio.
3. Replace `fat32.rs` with rust-fatfs (§5): not answered explicitly; the seat goes ahead with the recommendation,
   since keeping two FAT implementations costs more and L3b needs rust-fatfs anyway. The nine scenarios are the
   regression oracle for the swap.
