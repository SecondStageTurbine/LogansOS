# Research: Phase 3 BUILD-2 NeoFS native v1 format — structural challenge

| | |
|---|---|
| **Date** | 2026-05-12 (fresh-context Cerebro session) |
| **Author** | Cerebro structural-challenge artifact |
| **Type** | Research artifact (informational; not normative). Structural challenge + format recommendation. |
| **Dispatch** | `DISPATCH-2026-05-12-phase-3-build-2-neofs-format-structural-challenge-cerebro.md` |
| **Authority** | None. PM consumes; Jarvis dispatches against it. No normative-doc edits by this artifact. |

---

## 1. Verdict

**The nine questions are correctly posed. No Level-0 reframe ("should we be asking a different question entirely?") is warranted.** The BUILD-2 framing — minimum on-disk format that satisfies native NeoFS correctness without cornering BUILD-3 CAS — is the right question.

Three Level-1 reframes (question-narrowings) are warranted and materially simplify the implementation:

**Reframe A — Q3 intent-log:** The question "what minimal intent-log record set is needed for BUILD-2?" should become "**Is an intent log needed in BUILD-2 at all?**" The answer is no. BUILD-2 uses synchronous write-through I/O; every write returns only after the block device has committed the data. `sys_fsync` in BUILD-2 is a documented no-op — all writes are already durable by the time the syscall returns. The intent log is a BUILD-3 artifact whose only purpose is ordering durable commits from a write-back cache. Designing one for BUILD-2 would be building infrastructure for a problem that does not yet exist.

**Reframe B — Q4 allocator:** The question "bitmap, extent list, free-tree, or other?" should become "**Do we need traditional free-space management at all in BUILD-2?**" The answer is no. NeoFS's CoW semantics mean every write allocates new blocks; old blocks are never overwritten. The only allocator BUILD-2 needs is a bump pointer (`next_free_block` in the superblock, incremented at allocation, never decremented). Orphaned blocks accumulate. BUILD-3 mark-sweep GC retroactively reclaims them. This reduces Q4 from a data-structure design choice to a three-line counter advance.

**Reframe C — Q5 inode/tree shape:** The question "fixed direct blocks, extents, or B-tree?" has a clear answer given BUILD-2's scope: **eight direct block pointers and one reserved indirect slot, nothing more.** At 4 KiB blocks, eight direct pointers serve files up to 32 KiB — sufficient for every BUILD-2 test scenario. The indirect slot is reserved (stored as 0, not followed) for forward-compat. Extents and B-trees add complexity without serving a BUILD-2 correctness requirement.

Underneath all nine questions lies one integrating observation: **BUILD-1 already gave us the VFS contract.** The `Inode` trait, `InodeKind` enum, `FileRights` u32, `VfsError` with `TypeForbidden`/`InsufficientRights`, and `OpenFile` with type-first-rights-second dispatch are all in `kernel/src/fs/mod.rs` and will not change. BUILD-2 adds `NeoFsInode: Inode` (the on-disk implementation) and `NeoFs` (the mount backend). The format spec exists to define what `NeoFsInode` reads from disk. It does not define the kernel API — that already exists.

---

## 2. Corrected Questions

Three of the nine questions are narrowed (not replaced):

| Original | Corrected |
|---|---|
| Q3: What minimal intent-log record set is needed? | Q3: Does BUILD-2 need an intent log at all? (Answer: no.) |
| Q4: Bitmap, extent list, free-tree, or other? | Q4: Can BUILD-2 use a bump allocator and defer reclamation to BUILD-3 GC? (Answer: yes.) |
| Q5: Direct blocks, extents, or B-tree? | Q5: What is the simplest inode block-pointer layout that serves BUILD-2 and forward-compat BUILD-3 CAS? (Answer: 8 direct + 1 reserved indirect.) |

The other six questions (on-disk format, crash consistency, FileCap generation, typed-inode tests, mkfs/mount tests, anti-scope) are correctly posed and answered directly below.

---

## 3. Proposed NeoFS v1 On-Disk Layout

### 3.1 Design constraints from existing substrate

Before specifying the format, the constraints from the existing source:

From `kernel/src/fs/mod.rs`:
- `InodeKind`: currently `{ Directory, Regular, Symlink }` — BUILD-2 adds `Model` (value 3)
- `FileRights`: already a `u32` with named reserved bits through `MOUNT_RESERVED = 1 << 27`
- `VfsError`: `TypeForbidden` and `InsufficientRights` are already distinct
- The `Inode` trait is the NeoFS inode's interface: `kind()`, `available_rights()`, `read_at()`, `write_at()`
- `OpenFile` already checks type before rights: `kind != Regular → TypeForbidden` (BUILD-2 must extend this to include `Model` as valid for read-only open but forbidden for mmap-exec)

From `kernel/src/drivers/block/mod.rs`:
- `BlockDevice` trait: synchronous `read_blocks` and `write_blocks` using `&[Arc<DmaBuffer>]`
- `BlockGeometry`: `logical_block_size`, `physical_block_size`, `block_count`
- NeoFS maps its block size to the device's logical block size or a multiple thereof

From ADR-021 Q6:
- Inodes store both physical block address AND content hash — forward-compat for CAS
- Generation counter in every inode, initialized to 1, stored on disk
- Reserved retype audit bytes (16 bytes) per inode
- RETYPE reserved bit in FileRights word (bit 16)
- Root inode pointer and hash in superblock
- Three superblock copies (Btrfs prior art, simultaneous write, BLAKE3 checksum)

### 3.2 Superblock layout (512 bytes, three copies)

Three copies at fixed device offsets:
- Copy 0: byte offset 0 (block 0)
- Copy 1: 1 MiB into device (byte offset 0x10_0000)
- Copy 2: 64 MiB into device (byte offset 0x400_0000); present only if device ≥ 128 MiB

On mount: scan all three valid (magic + checksum OK) copies; select the one with the highest `generation` field. If no copy is valid, refuse mount.

```
NeoFsSuperblock (512 bytes, little-endian)
─────────────────────────────────────────────────────────────────────
Offset  Size  Field
──────  ────  ────────────────────────────────────────────────────────
  0       8   magic: u64 = 0x464E454F5F534631 ("FNEO_SF1", NeoFS v1)
  8       2   format_version: u16 = 1
 10       2   block_size_log2: u16  (e.g., 12 = 4096 bytes per block)
 12       1   type_version: u8      (highest InodeKind value defined)
 13       3   flags: [u8; 3]        (reserved, zeroed)
 16       8   total_blocks: u64     (device capacity in FS blocks)
 24       8   next_free_block: u64  (bump-allocator head; first unallocated block)
 32       8   generation: u64       (superblock generation; strictly monotone; pick highest on mount)
 40       8   root_inode_block: u64 (physical block address of root Directory inode)
 48      32   root_inode_hash: [u8;32] (BLAKE3 of root inode block; future CAS verification)
 80       4   inode_size_bytes: u16  (size of each on-disk inode record, must be power of 2)
             [note: stored as u32 in 4 bytes for alignment]
 84      16   reserved_for_uuid: [u8;16]  (future volume UUID)
100     380   reserved: [u8;380]     (zeroed; reserved for future fields)
480      32   checksum: [u8;32]      (BLAKE3 of bytes 0..479; computed last, written into bytes 480..511)
─────────────────────────────────────────────────────────────────────
Total: 512 bytes (one 512-byte sector, or first block at block_size alignment)
```

**Endianness:** little-endian throughout. NeoPlutarch targets x86_64 (LE) and aarch64 (LE under the `-M virt` configuration used in CI). No byte-swap needed on either target.

**Checksum discipline:** BLAKE3 over bytes 0..479 (all fields except the checksum field itself). Written as the last operation before the superblock write commits. On mount, recompute and compare; mismatch → this copy is invalid.

**`next_free_block` initialization:** set to the first block after the last superblock copy. On a small VM disk (e.g., 64 MiB / 4 KiB blocks = 16384 blocks), Copy 2 may be absent if device < 128 MiB; `next_free_block` starts after the last present copy.

### 3.3 Inode layout (256 bytes)

Each inode is a fixed-size 256-byte record. Inode blocks are ordinary data blocks; the root directory inode is at `root_inode_block`. Each directory entry contains its child's physical block address (not an inode number — there is no inode table in BUILD-2; inodes are directly addressed by their block address).

```
NeoFsInode (256 bytes, little-endian)
─────────────────────────────────────────────────────────────────────
Offset  Size  Field
──────  ────  ────────────────────────────────────────────────────────
  0       8   magic: u64 = 0x4E454F4653494E4F ("NEOFSINO")
  8       1   kind: u8  (0=Regular, 1=Directory, 2=Symlink, 3=Model, 0xFF=free)
  9       1   type_version: u8  (type_version at creation, from superblock; for future type enum validation)
 10       2   flags: u16  (reserved, zeroed)
 12       4   generation: u32  (starts at 1; incremented on retype; never 0; global monotone from superblock)
 16       8   file_size: u64  (byte length of content; 0 for Directory)
 24       8   block_count: u64  (number of direct blocks actually used; ≤ 8)
 32      64   direct_blocks: [u64; 8]  (physical block addresses; unused slots = 0)
 96       8   indirect_block: u64  (reserved; must be 0 in BUILD-2; BUILD-3+ may use)
104      32   content_hash: [u8; 32]  (BLAKE3 of full file content; 0s if empty; verified on read)
136      16   reserved_retype_audit: [u8; 16]  (zeroed; reserved for retype timestamp + initiator cap)
152       4   available_rights_bits: u32  (FileRights bits available for open(); derived from kind at mount)
156      36   reserved: [u8; 36]  (zeroed; future fields)
192       4   directory_entry_count: u32  (only valid for Directory kind; 0 for non-directory)
196      28   reserved2: [u8; 28]  (zeroed)
224      32   checksum: [u8; 32]  (BLAKE3 of bytes 0..223)
─────────────────────────────────────────────────────────────────────
Total: 256 bytes
```

**`available_rights_bits` derivation:** stored on disk as a hint; at mount time the kernel recomputes from `kind`:
- Regular: READ | WRITE | EXECUTE_RESERVED | MMAP_RESERVED | GRANT_RESERVED | DERIVE_RESERVED
- Directory: LOOKUP_RESERVED | READDIR_RESERVED | GRANT_RESERVED | DERIVE_RESERVED
- Symlink: READ
- Model: READ | MMAP_RESERVED (no EXECUTE, no WRITE after creation; MMAP_EXEC permanently forbidden)

The on-disk `available_rights_bits` is redundant but allows future kinds to carry their own policy without a kernel update. BUILD-2 always recomputes from `kind` and ignores the stored value; BUILD-4+ may use it.

**`generation` source:** the global monotone generation counter lives in the superblock at a separate field (add `next_generation: u32` at superblock offset 84; advance atomically with each inode creation or retype). Each new inode receives `generation = superblock.next_generation++`. This ensures uniqueness across inode block reuse: if block B held inode-A (generation=5), was freed, and is reused for inode-B, inode-B gets generation=6. Any FileCap that stored generation=5 for block B correctly detects inode-B as stale.

### 3.4 Directory entry layout (64 bytes)

Directory inodes store their entries packed in direct data blocks. Each entry is 64 bytes.

```
NeoFsDirEntry (64 bytes, little-endian)
─────────────────────────────────────────────────────────────────────
Offset  Size  Field
──────  ────  ────────────────────────────────────────────────────────
  0      48   name: [u8; 48]  (null-terminated UTF-8 filename; remaining bytes zeroed)
 48       8   child_block: u64  (physical block address of child inode; 0 = unused slot)
 56       1   kind: u8  (duplicate of child inode kind; avoids extra read for directory listing)
 57       1   reserved: u8
 58       6   reserved2: [u8; 6]
─────────────────────────────────────────────────────────────────────
Total: 64 bytes
```

Max filename length: 47 bytes of UTF-8 + null terminator. Max entries per 4 KiB block: 64. Max entries per inode: 64 × 8 direct blocks = 512 entries. For BUILD-2, this is sufficient. The `kind` duplicate in the directory entry allows VFS path walk to check type before reading the child inode block — a cheap optimization that avoids a disk read for the common case of "is this a directory?"

**Unused slots:** `child_block = 0` marks a slot as unused. The `directory_entry_count` field in the parent inode records the total count of live entries for iteration termination.

### 3.5 mkfs image layout summary

```
NeoFS v1 disk image layout (example: 64 MiB, 4 KiB blocks)
─────────────────────────────────────────────────────────────────────
Block 0     (bytes 0..4095):        Superblock copy 0 (512 bytes at offset 0; rest zeroed)
Blocks 1..255:                      Superblock copy 1 reserved space (1 MiB region; Copy 1 at block 256)
Block 256   (byte offset 0x10_0000): Superblock copy 1
Blocks 257..:                       Data region begins
  Block 257:  Root directory inode
  Block 258:  [first file/directory data]
  ...
Blocks 16383..:                     (64 MiB / 4096 = 16384 blocks total)
  Block 16384: Superblock copy 2 would be here (64 MiB offset); absent on 64 MiB image
─────────────────────────────────────────────────────────────────────
`next_free_block` initialized to 258 (after root dir inode block).
```

---

## 4. Crash-Consistency and Write-Ordering Model

### 4.1 Is ADR-021's BUILD-2 crash model coherent on write-through?

**Yes, fully coherent — and simpler than ADR-021 implies.**

On BUILD-2's synchronous write-through virtio-blk path, every `write_blocks()` call returns only after the data is stable on the device (Case-1 virtio durability: F_FLUSH offered, not negotiated → write-through semantics, completed writes are stable). There is no volatile write cache. Therefore:

- No explicit FLUSH commands needed in BUILD-2.
- Every block write is already durable by the time the write system call returns.
- The only crash-safety requirement is **write ordering** — which blocks are written in which order — because a crash between two writes is possible.

### 4.2 Write ordering requirements per operation

The invariant: **the superblock is always the last thing written.** Everything the new superblock points to must be on disk before the new superblock is written. The new superblock, once written with its generation counter incremented, is the new source of truth.

**CREATE a new file:**
```
1. allocate inode block (advance next_free_block in memory only)
2. write inode block to disk (zeroed, then populated with kind/size/hash/generation)
3. if file has content: allocate data block(s), write data, update inode's direct_blocks + content_hash, write inode again
4. allocate parent dir block (CoW: allocate new block for updated parent dir data)
5. write new parent dir data block (with new directory entry pointing at child inode block)
6. write updated parent dir inode (pointing at new dir data block)
7. write updated superblock copies (with new next_free_block, new root inode pointer if path changed, incremented generation)
```

**OVERWRITE an existing file (sys_write):**
```
1. allocate new data block(s) (advance bump pointer)
2. write new data to new block(s)
3. write updated inode (new direct_blocks[], new file_size, new content_hash)
4. write updated superblock (new next_free_block, same root — unless inode is root-adjacent, incremented generation)
```
Old data blocks become orphaned. They are not freed; BUILD-3 GC reclaims them.

**CREATE a directory:**
Same as CREATE a file, but inode kind=Directory, no data blocks initially.

**SYS_FSYNC (per ADR-021):**
On write-through synchronous I/O: **no-op.** All writes are already durable. Return `Ok(())`. Document this explicitly in the BUILD-2 implementation comment: `// On write-through storage, fsync is a no-op: all writes are already stable. // When buffer-cache write-back lands in BUILD-3, this becomes the per-file // intent-log flush point.`

**No intent log in BUILD-2.** This is the Reframe A resolution. The intent log is a BUILD-3 artifact.

**TRUNCATE:** Out of BUILD-2 scope per anti-scope list (Q9). The implementation does not need truncate.

**RENAME, UNLINK:** Out of BUILD-2 scope. Directory mutation is limited to CREATE (new entries added, no entries removed or renamed).

### 4.3 Superblock rotation discipline

Three superblock copies, written simultaneously with FLUSH barriers per ADR-021/Btrfs prior art. In BUILD-2 write-through mode, the writes are already ordered by the synchronous completion discipline; no explicit FLUSH is required:

```
write_blocks(superblock_copy_0_block, superblock_bytes);  // wait for completion
write_blocks(superblock_copy_1_block, superblock_bytes);  // wait for completion
write_blocks(superblock_copy_2_block, superblock_bytes);  // if device large enough; wait
```

On the BUILD-2 test device (64 MiB), copy 2 is absent; two copies suffice.

On mount, the kernel reads all present copies, validates checksums, and selects the highest-generation valid copy. A partially-written copy (torn write) is detected by checksum failure.

### 4.4 Crash scenario analysis

**Crash after step 1–2 (new inode block on disk, superblock not yet updated):** The new inode block exists on disk but is not referenced by any superblock or directory. BUILD-3 GC will find it as orphaned and reclaim it. Safe.

**Crash after step 3–6 (updated directory and inode on disk) but before step 7 (new superblock):** The old superblock remains the latest valid generation. The old superblock points to the old root/directory. The new directory entries and inodes exist on disk but are unreferenced. BUILD-3 GC reclaims them. Safe.

**Crash during step 7 (superblock write):** At least one of the two (or three) copies may be corrupt (torn write). The checksum mechanism detects the torn copy; the remaining valid copy with the next-highest generation is selected. If the written copy had the new generation and the unwritten had the old generation, the old generation wins — the create appears to not have happened. If the written copy was the new one and it validates, the create is committed. Either outcome is a consistent filesystem state. Safe.

**No corruption scenario.** There is no write sequence in BUILD-2 that can produce an inconsistent filesystem state that mount cannot recover from by selecting a valid-checksum superblock.

---

## 5. FileCap / Generation / Type Invariants

### 5.1 The complete generation invariant

```
Invariant G1: inode.generation is a u32, stored on disk, initialized to 1 (never 0),
              sourced from superblock.next_generation which is monotonically increasing.

Invariant G2: Every FileCap carries the generation at which it was issued:
              FileCap { inode_block: u64, generation: u32, rights: FileRights }.

Invariant G3: At every VFS entry point (read, write, fsync, close, stat):
              if cap.generation != inode.generation { return Err(NeoError::Stale); }

Invariant G4: Retype (BUILD-5): increment inode.generation. All existing FileCaps
              immediately return Stale on next use. No in-place FileCap update.

Invariant G5: Inode block reuse: the new inode at the reused block gets generation
              from superblock.next_generation++ (strictly greater than any previous
              generation at that block). Old FileCaps have smaller generation, see Stale.

Invariant G6: superblock.next_generation is advanced atomically with each new inode
              creation. It is written to disk as part of the superblock commit (step 7).
              If the superblock commit is lost (crash), the next_generation is rolled back
              with the superblock — the lost-generation inode is orphaned (BUILD-3 GC).
```

### 5.2 FileCap at the BUILD-1/BUILD-2 boundary

BUILD-1's `OpenFile` struct (in `fs/mod.rs`) does not yet carry a generation counter because testfs has no persistence. BUILD-2 extends this:

```rust
// BUILD-2 extension to kernel/src/fs/mod.rs or a new neofs/cap.rs:
pub struct NeoFileCap {
    inode_block: u64,     // physical block address on disk
    generation: u32,      // generation at open time
    rights: FileRights,   // attenuation of inode's available_rights
}
```

Every VFS method call that enters the on-disk NeoFS path resolves the inode block, reads `inode.generation`, and checks against `cap.generation`. The check is O(1) — a single u32 comparison after the block read. It costs one block read per VFS call (the inode re-read), which is acceptable in the synchronous-no-cache BUILD-2 model. BUILD-3 caching makes this cheaper.

### 5.3 Typed inode discriminant — precise BUILD-2 invariants

```
Invariant T1: InodeKind on disk (inode.kind: u8) is the ground truth for type.
              Filename, file extension, and content are irrelevant to type enforcement.

Invariant T2: InodeKind is written once at inode creation (sys_create).
              It is not changed in BUILD-2. Retype is a BUILD-5 operation.

Invariant T3: VFS dispatch is type-first, rights-second.
              For every operation O:
                if !INODE_TYPE_POLICY[inode.kind].allows(O) → TypeForbidden
                else if !cap.rights.allows(O)               → InsufficientRights

Invariant T4: BUILD-2 policy table:
              Regular:   all operations allowed (READ, WRITE, EXECUTE_RESERVED, MMAP_RESERVED, LOOKUP_RESERVED for parent dir)
              Directory: LOOKUP_RESERVED, READDIR_RESERVED only; no READ/WRITE/EXECUTE
              Symlink:   READ only; no WRITE, no EXECUTE
              Model:     READ, MMAP_RESERVED (as MMAP_R); no WRITE after creation; no EXECUTE; no MMAP as PROT_EXEC

Invariant T5: Model → PROT_EXEC mmap refusal.
              Even if MMAP_RESERVED bit is granted in the FileCap, PROT_EXEC for a
              Model inode returns TypeForbidden. The VFS mmap hook checks:
                if prot.contains(PROT_EXEC) && inode.kind == InodeKind::Model → TypeForbidden
              This must remain true regardless of FileCap rights bits.

Invariant T6: InodeKind is extended by adding entries to the enum in the kernel, not
              by filename convention. Adding kind=Tensor in Phase 5 requires a kernel
              change; it cannot be smuggled through by naming a Regular file "foo.tensor".
```

### 5.4 `available_rights_bits` derivation function

```rust
impl InodeKind {
    pub fn available_rights(self) -> FileRights {
        match self {
            InodeKind::Regular   => FileRights::READ | FileRights::WRITE
                                  | FileRights::EXECUTE_RESERVED | FileRights::MMAP_RESERVED,
            InodeKind::Directory => FileRights::LOOKUP_RESERVED | FileRights::READDIR_RESERVED,
            InodeKind::Symlink   => FileRights::READ,
            InodeKind::Model     => FileRights::READ | FileRights::MMAP_RESERVED,
        }
    }
}
```

---

## 6. Required BUILD-2 Scenarios

The scenarios below must be passed before BUILD-2 closes. They are ordered from simplest to most complex; Jarvis can implement them in this order and stop at the first failure.

### 6.1 mkfs smoke

```
Create a 64 MiB raw image with one root directory, one Regular file ("hello.txt"),
one Model-typed file ("model.neom"), and one empty Directory ("subdir").
Verify: all three superblock copies have valid BLAKE3 checksums.
Verify: root inode block address in superblock copy 0 resolves to a valid Directory inode.
Verify: model.neom inode has kind=3 (Model).
```

### 6.2 Mount and read

```
QEMU boot with the mkfs image as virtio-blk disk.
Mount NeoFS image.
Open "hello.txt" with READ rights.
Read content; compare to expected bytes.
Open "hello.txt" with WRITE rights → InsufficientRights (available_rights has no WRITE on this file? — 
  actually at mkfs time, if we gave it READ only, then yes. If we gave READ|WRITE at mkfs, then WRITE succeeds.)
  Clarification: mkfs assigns available_rights from inode.kind. For a Regular file: READ|WRITE both available.
  The test should open with READ-only rights and verify WRITE attempt on the cap returns InsufficientRights.
Open "subdir" with READ rights → TypeForbidden (Directory kind, READ not in available_rights).
```

### 6.3 Create file and read back

```
Mount fresh NeoFS image (empty root dir only).
Create "newfile" with Regular kind.
Write 4096 bytes of known pattern.
Close the file.
Re-open "newfile" with READ rights.
Read 4096 bytes; compare to written pattern.
Unmount.
```

### 6.4 Reboot persistence (two-boot)

```
Boot 1: mount NeoFS, create "persistent.txt", write known bytes, fsync (no-op; write-through), shutdown.
Boot 2: mount same NeoFS image, open "persistent.txt" with READ, read back, compare.
PASS: bytes identical.
```

### 6.5 Crash consistency (superblock rotation)

```
Create a NeoFS image.
Using an offline test tool (or mkfs variant), corrupt superblock copy 0 (flip one byte in checksum field).
Mount the image.
PASS: kernel selects copy 1 (valid checksum, same or next generation).
Verify: content readable.
```

### 6.6 Typed-inode enforcement — four-corner matrix (Bible 10.3)

```
Scenario A: inode kind=Model, attempt open_with_mmap_exec_request → TypeForbidden
Scenario B: inode kind=Regular, content=valid ELF bytes, attempt open_with_mmap_exec_request → success (if cap rights allow)
Scenario C: inode named "legit.neom", kind=Regular, attempt open_with_mmap_exec_request → success (filename irrelevant)
Scenario D: inode named "binary", kind=Model, attempt open_with_mmap_exec_request → TypeForbidden (kind overrides name)
```

For BUILD-2 where the full mmap syscall surface is not yet wired: implement as a VFS-layer test function `test_mmap_exec_allowed(cap: &NeoFileCap) -> Result<(), NeoError>` that applies Invariant T5 and returns TypeForbidden or Ok. The actual mmap dispatch wiring is BUILD-4; the type-check logic and its test are BUILD-2.

### 6.7 FileCap generation staleness

```
Open a Regular file → get FileCap with generation=N.
Simulate retype (increment inode.generation on disk by writing a new inode version).
Attempt read through the original FileCap → Stale.
Re-open the file → new FileCap with generation=N+1 → read succeeds.
```

For BUILD-2 (where retype is not a user-visible operation): the simulation writes directly to the inode block with an incremented generation field. This is a unit-level test of the Stale detection logic.

### 6.8 Content hash integrity

```
Create a file, write N bytes, record the content hash stored in the inode.
Independently compute BLAKE3(bytes). Compare.
Read back the file; verify the VFS re-computes hash on read and returns Io error if stored hash mismatches.
```

The content hash stored in the inode is verified on read by the NeoFsInode implementation. This is the pre-CAS integrity primitive — same hash field that BUILD-3 CAS will use as the block address key.

---

## 7. Implementation Risks

### 7.1 Generation counter monotonicity across crash

**Risk:** If the kernel increments `superblock.next_generation` in memory, allocates a new inode with that generation, writes the inode, but then crashes before writing the new superblock — the old superblock is recovered with the old `next_generation`. On next mount, `next_generation` is rolled back. The orphaned inode (with the rolled-back generation value) will get a generation assigned from the same rolled-back pool when it's eventually reused.

**Mitigation:** This is only a problem if the reused block gets the SAME generation value as an outstanding FileCap. Since FileCaps are in-kernel memory and don't survive reboot, all FileCaps from the crashed session are gone. No live FileCap can have a stale-dangerous generation value. **This risk is zero in BUILD-2** because FileCaps are not persistent.

**Forward-compat note for Phase 5+ persistent capabilities:** if NeoPlutarch ever persists FileCaps across reboot (not planned but flagged), this risk becomes real. The generation counter should be advanced by a large stride per boot (e.g., `next_generation += 0x10000` at mount time) to create a generation-space gap that makes cross-boot collisions negligible.

### 7.2 Bump allocator overflow

**Risk:** BUILD-2's bump allocator never reclaims space. Repeated overwrites of the same file allocate new blocks each time. On a small test image (64 MiB), an adversarial workload could exhaust blocks before BUILD-3 GC lands.

**Mitigation:** The build-2 acceptance scenarios are bounded (smoke tests, not stress tests). Add a `DeviceFull` error return from `alloc_block()` when `next_free_block >= total_blocks`. Return `Err(NeoError::OutOfSpace)` from the filesystem operation (maps to `ENOSPC`). This is the correct error handling even in the bump-allocator model.

### 7.3 No unlink in BUILD-2

**Risk:** Without `sys_unlink` (anti-scoped per Q9), test scenarios cannot clean up files between runs on the same image. An image that starts with files from a previous run may interfere with scenarios expecting a clean state.

**Mitigation:** mkfs the image fresh before each test run. The BUILD-2 verification suite always starts from a mkfs'd image, not an image from a previous run. This is the correct discipline for deterministic testing.

### 7.4 content_hash computation cost

**Risk:** Computing BLAKE3 of the full file content on every write is O(file-size) in the critical path of `write_at()`. For a 32 KiB file (8 × 4 KiB blocks), this is 32 KiB of hashing per write call.

**Mitigation:** At BUILD-2 scale (test files, small writes), this cost is invisible. The BLAKE3 implementation in `no_std` is highly optimized (~7 GB/s on x86_64). For 32 KiB, hashing takes ~5 microseconds — negligible against the disk write latency (virtio-blk in QEMU: ~100–500 μs). If profiling later shows hash overhead, defer to BUILD-3 incremental hashing. Do not defer hash computation — it is the content-integrity primitive that validates every read and is the CAS seed for BUILD-3.

### 7.5 VFS abstraction at the BUILD-1/BUILD-2 boundary

**Risk:** BUILD-1's `fs/mod.rs` exports a `TestFs`-backed root. BUILD-2 adds `NeoFs` as a new mount backend. Concurrently mounted testfs and NeoFs (e.g., for testfs-backed stdout while NeoFs handles disk I/O) must not share state.

**Mitigation:** The `MountTable` in `fs/mod.rs` currently holds exactly one `root: Arc<TestFs>`. BUILD-2 extends this to support multiple mounted filesystems identified by mount path. The testfs root (`/`) remains for early-boot scenarios; the NeoFS mount appears at `/mnt` or a test path. The `open_root_temporary` function routes to the correct backend based on the path prefix. This is a ~50-line extension to the existing MountTable.

---

## 8. Stop Conditions for Jarvis Implementation

Jarvis should stop and file a deviation/blocker if:

### 8.1 Superblock format migration

If Jarvis discovers a reason to change the superblock binary layout (field additions, offset changes, size changes) after beginning BUILD-2 implementation, stop and surface to PM before proceeding. Format changes mid-BUILD are high-risk because mkfs tooling, kernel mount code, and test infrastructure must all be updated atomically. File `Plans/DEVIATION-2026-05-MM-phase-3-build-2-format-change-jarvis.md`.

### 8.2 Generation counter range overflow

If the implementation encounters a scenario where `next_generation` could wrap (e.g., u32 max at 4.29 billion) in the BUILD-2 test scenarios, stop and report. This should not happen for any realistic BUILD-2 test workload, but if it does, the fix is expanding the counter to u64. Stop rather than silently change the format.

### 8.3 Unresolvable crash-safety gap

If Jarvis identifies a write-ordering scenario that is NOT safe under the bump-allocator + superblock-rotation model (i.e., a crash scenario that produces an inconsistent filesystem state), stop and file a blocker. Per §4.4, no such scenario should exist in the BUILD-2 model, but if one is found, it must be addressed before proceeding.

### 8.4 BLAKE3 dependency not available in no_std

If the BLAKE3 implementation needed for content hashes and superblock checksums is not available as a `no_std`-compatible crate in the project's dependency context, stop and surface to PM. The recommended crate is `blake3` with `no_std` feature disabled and `no_atomic64` enabled, or `b3sum` / `tiny-keccak`-family alternatives. Do not use SHA-256 as a substitute without PM instruction — BLAKE3 is the project's ratified algorithm per ANSPEC §8.2.

### 8.5 `InodeKind::Model` mmap-exec test reveals surface not yet present

If the mmap-exec TypeForbidden test (scenario 6.6) cannot be wired because the kernel's mmap dispatch does not yet exist to hook into, file a forward-compat stub: implement `test_mmap_exec_allowed(cap)` as an isolated function verified by unit test. Document in the BUILD-2 close report that scenario 6.6 is verified at the VFS-function level, not at the syscall-mmap level. This is not a blocker — it matches ADR-021's BUILD-2 scope (typed-inode enforcement hooks) which explicitly defers the full mmap syscall to BUILD-4.

---

## 9. Anti-Scope Guardrails (Q9)

These must remain out of BUILD-2. They are not deferred "until later in BUILD-2" — they are out of scope entirely. If Jarvis is tempted to add any of these during BUILD-2, stop and surface to PM first.

| Item | Reason | Next milestone |
|---|---|---|
| CAS / hash-as-address block lookup | Measurement-gated; ZFS dedup cautionary tale | BUILD-3 (if gate passes) |
| Dedup index / bloom filter | Depends on CAS | BUILD-3 |
| Buffer-cache write-back | Requires kthread; adds complexity without BUILD-2 correctness need | BUILD-3 |
| Background GC / mark-sweep scan | Requires kthread | BUILD-3 |
| AHCI / NVMe block drivers | Separate implementation effort | BUILD-5 |
| FAT32 / ext4 read-only | Separate parsers; no type-import correctness at BUILD-2 | BUILD-4 |
| Default `noexec` on foreign mounts | No foreign mounts in BUILD-2 | BUILD-4 |
| Rootfs pivot / neo-init evolution | Depends on mature VFS | BUILD-3 or BUILD-4 |
| `sys_rename`, `sys_unlink`, `sys_rmdir` | Directory mutation beyond create | BUILD-4 |
| `sys_truncate` | File shrink; orphan accounting | BUILD-4 |
| Symlink resolution (follow) | Safety edge cases; clamp-at-root complexity | BUILD-2 partial: symlink kind reserved but walk blocks it |
| Hard links | Permanent skip: conflict with typed inodes and CoW identity | Never |
| POSIX ACLs | Redundant with FileCap rights | Never |
| xattrs on NeoFS inodes | Reserve the field; do not implement | Phase 5+ |
| `kthread` primitive | Required by BUILD-3; not needed for BUILD-2 synchronous writes | BUILD-3 prerequisite audit |
| VFS cursor-based readdir | Directory enumeration syscall | BUILD-4 |
| Per-process mount namespaces | Global namespace-only until BUILD-4 | BUILD-4 |

**One clarification on symlinks:** BUILD-2 must recognize `InodeKind::Symlink` in the discriminant (the enum variant exists to reserve the type value and so that the typed-inode test matrix can test "opening a symlink returns TypeForbidden for most operations") but need not implement symlink *following* (resolving the target). The symlink kind is reserved; reading a symlink inode returns the target path string via `read_at`; the VFS walk does not follow it. Symlink following is BUILD-2 optional and BUILD-4 normal scope.

---

## 10. Cross-References

**This artifact consumes:**
- `docs/ADR-021-phase-3-design-substrate.md` — ratified BUILD-2 scope and format field requirements
- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` — prior-art basis for CoW, superblock rotation, bump allocator via Venti/ZFS/Btrfs analysis
- `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md` — typed-inode enforcement architecture, FileCap rights, generation invariant
- `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md` — confirmed substrate available in tree (virtio_barrier.rs, block/mod.rs, fs/mod.rs)
- `kernel/src/fs/mod.rs` — existing VFS contract BUILD-2 must respect
- `kernel/src/drivers/block/mod.rs` — BlockDevice trait BUILD-2 reads from
- `kernel/src/drivers/block/virtio_barrier.rs` — barrier wrappers shared with aarch64 lane

**This artifact does NOT produce:**
- `docs/specs/neofs-format.md` (that is a separate normative artifact PM will commission)
- No ADR edits, no Bible edits, no normative-doc changes

**This artifact is consumed by:**
- PM's BUILD-2 PLAN dispatch for Jarvis
- `docs/specs/neofs-format.md` (the normative format spec that turns §3 of this artifact into the authoritative binary layout document)

---

## 11. Closure

The nine questions are answerable without a structural reframe. Three question-narrowings (intent-log is empty in BUILD-2, bump allocator is sufficient, direct blocks only) materially simplify the BUILD-2 implementation — together they remove approximately 500 lines of code that would otherwise be premature infrastructure. The format proposed in §3 is the minimum binary layout that satisfies ADR-021's forward-compat requirements (generation counter, type_version, content_hash reservation, retype audit bytes, reserved inode fields) while keeping the superblock at 512 bytes and the inode at 256 bytes — both power-of-two sizes that align with device sectors and cache lines. The crash-consistency model is coherent on write-through and requires no intent log. The FileCap generation invariant is tight enough to support future retype without format migration. The eight test scenarios prove the correctness properties BUILD-2 claims. The stop conditions ensure Jarvis surfaces format changes before they proliferate.

---

**End of research artifact.**

*No Level-0 reframe needed — questions correctly posed. Three Level-1 reframings: Q3 intent-log is empty in BUILD-2 (write-through = already durable); Q4 allocator is a bump pointer (no free-space management; orphans → BUILD-3 GC); Q5 inode layout is 8 direct blocks + 1 reserved indirect. FORMAT: 512-byte superblock (magic, version, block_size_log2, total_blocks, next_free_block, generation, root_inode_block, root_inode_hash, next_generation, checksum); 256-byte inode (magic, kind, type_version, generation, file_size, block_count, direct_blocks[8], indirect_block=0, content_hash, reserved_retype_audit, checksum); 64-byte directory entry (name[48], child_block, kind). Three superblock copies. BLAKE3 checksums. Monotone generation from superblock.next_generation. sys_fsync = no-op in BUILD-2. Eight required scenarios. Nine anti-scope guardrails.*
