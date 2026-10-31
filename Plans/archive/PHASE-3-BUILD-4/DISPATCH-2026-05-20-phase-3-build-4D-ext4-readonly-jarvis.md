# Dispatch - Phase 3 BUILD-4D ext4 Read-Only Foreign Filesystem

Date: 2026-05-20
Agent: Jarvis
Status: AUTHORIZED IMPLEMENTATION

Authority:

- `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4C-close-consumption.md` (BUILD-4C
  closure; FAT32 substrate-validation pattern established)
- `Plans/PHASE-3-BUILD-4C-CLOSE-EVIDENCE-2026-05-20.md` (commits `a627d670` and
  `ad31f1269`; mount-flags ABI, `MountEntry.rights_mask`, `fs_type=3` precedent,
  virtio-blk all-device polling)
- `Plans/DISPATCH-2026-05-20-phase-3-build-4C-fat32-readonly-jarvis.md` (BUILD-4C
  dispatch; the canonical pattern this dispatch follows)
- `Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md` (BUILD-4
  split; D9 foreign-FS order — FAT32 before ext4; e2fsprogs tooling
  prerequisite)
- `Plans/PM-DISPOSITION-2026-05-16-phase-3-build-4-prereq-audit-consumption.md`
  (D9, D10 accepted)
- `Plans/PM-NOTE-2026-05-20-build-4C-mtools-stop-condition-resolution.md`
  (canonical container-tooling verification idiom established)
- `docs/ADR-021-phase-3-design-substrate.md` BUILD-4 boundaries; Q9 foreign-FS
  type import to Regular regardless of xattrs; foreign-FS xattr distrust
- `docs/bible.md` v1.6.0 Articles 10.3 (typed inodes), 10.4 (FAT32/ext4
  read-only interop), 12.5 (model weights as data)

Substrate:

```text
a627d670  build-4C: add FAT32 read-only foreign filesystem driver
```

(Plus close-evidence commit `ad31f1269`. Both authorized for push by
`PM-DISPOSITION-2026-05-20-phase-3-build-4C-close-consumption.md` §5.1.)

## 1. Mission

Land a minimal ext4 read-only driver as a `MountedFs` implementation under
the BUILD-4B mount-table substrate, mirroring the BUILD-4C FAT32 shape.
Type import defaults to `Regular`; xattrs are read but ignored for type
import (the Article 10.3 + ADR-021 Q9 invariant); everything else in the
foreign-FS surface is anti-scope.

This is the third filesystem under the substrate (NeoFS, FAT32, ext4).
The substrate-validation test from BUILD-4C extends: does `MountedFs`
accept ext4 — a substantially more complex filesystem than FAT32 —
without trait modification? If yes, the abstraction is genuinely
filesystem-agnostic at scale. If no, surface as a stop condition.

## 2. Pre-Implementation Verification (Required First Step)

Per the canonical container-tooling idiom established by the BUILD-4C
mtools-stop-condition resolution, the first BUILD-4D action is to verify
e2fsprogs is available inside the `neo` service runtime:

```text
docker compose run --rm neo bash -c 'which mke2fs e2cp debugfs dumpe2fs'
```

Expected output: four paths under `/usr/bin/` (or `/sbin/` for some
distributions; either is acceptable).

If any binary is missing inside the container, surface as a genuine stop
condition (Dockerfile amendment lane). PM expects this to succeed given
the BUILD-4 prereq audit noted e2fsprogs as the standard option for ext4
image generation, but the verification is mandatory before any
implementation work begins.

If `e2cp` is unavailable (it ships in some distributions but not others),
fall back to mounting the image via `mtools`-equivalent userspace tooling
or `debugfs` write commands; surface the substitution in close evidence.
`mke2fs`, `debugfs`, and `dumpe2fs` are the load-bearing tools.

## 3. Scope

### 3.1 In Scope

**ext4 on-disk parser (read-only, extents-only).**

- Superblock parse (offset 1024 from image start; magic 0xEF53);
- Block-group descriptor table read;
- Inode table read with inode size respected (typically 256 bytes for
  modern ext4);
- **Extent tree read.** Modern ext4 marks inodes with the `EXTENTS_FL`
  flag and stores extent records in the inode's `i_block` field plus
  external extent index blocks if the tree is deep. BUILD-4D supports
  extent-based inodes only.
- **Legacy block-mapping (indirect/double-indirect/triple-indirect) is
  NOT supported.** An inode without the `EXTENTS_FL` flag returns an
  appropriate error on open. ext3-style images and ext4 images created
  without extents are not in scope.
- Linear directory walk. Directory entries are linked-list entries in
  data blocks. **HTree (hashed) directory index is NOT consulted**;
  the linear directory entries are present alongside the HTree index
  and BUILD-4D walks them linearly regardless of whether the directory
  is HTree-indexed. This is a defensible read-only simplification.
- Symbolic links: directory entries of type symlink are **SKIPPED** in
  readdir output. They do not appear as accessible entries. This is
  consistent with ADR-021's no-symlink-traversal posture and avoids
  the complexity of representing symlinks as `Regular` or any other
  type in BUILD-4D scope.
- Special file types (sockets, pipes, character devices, block devices,
  FIFOs) are also **SKIPPED** in readdir output. ext4 may contain
  these; the foreign-FS surface does not need to expose them.
- xattrs are **read but ignored** for type import. The Bible Article
  10.3 + ADR-021 Q9 invariant: type comes from inode type field, not
  from filename extension or xattr. A `.neom`-named file with a
  hypothetical `user.neoplutarch.type=Model` xattr still imports as
  `Regular`. This must be specifically tested.

**MountedFs implementation.**

- `MountedFs::root()` returns the ext4 root directory inode (always
  inode 2 in ext4);
- `MountedFs::lookup(relative_path)` walks linear directory entries;
- Inode kinds limited to `Regular` and `Directory`; other types
  skipped per §3.1 above;
- Immutable-mount-snapshot model: the mount's ext4 image is treated as
  immutable for the duration of the mount; FileCap generation always
  matches; ESTALE is not returnable from ext4 in BUILD-4D (same as
  BUILD-4C's FAT32 posture).

**fs_type discriminator extension.**

Add to the BUILD-4C fs_type enumeration:

```text
4 = ext4 device2 readonly
```

`source = "device2"` is a fixed test-harness device selector, consistent
with BUILD-4C's `device1` for FAT32. Do not introduce a generic device-
string parser; reuse the existing block-device registry.

**Mount-flags ABI: no new bits.**

BUILD-4C established bit 0 = `MOUNT_FLAG_EXEC_ALLOWED`. ext4 reuses the
same semantics:

- default ext4 mount: `EXEC_ALLOWED=0` (noexec);
- with `MOUNT_FLAG_EXEC_ALLOWED`: EXECUTE admissible on regular files,
  subject to the type-first executable policy.

Reserved bits continue to return `EINVAL` if set.

**Mount rights mask.**

ext4 default mask matches FAT32 default mask:

- WRITE always cleared (foreign-FS read-only);
- EXECUTE cleared unless `MOUNT_FLAG_EXEC_ALLOWED` is set.

No new rights bits introduced.

**xtask ext4 image generation.**

Generate a deterministic ext4 image into `target/` from xtask before QEMU
boots. Use `mke2fs -t ext4 -F` and `debugfs`'s `write` and `mkdir`
commands (or equivalent e2fsprogs tooling). Image contents must be fully
reproducible from xtask invocation; no committed binary fixtures.

Suggested fixture contents (Jarvis chooses concrete bytes; this is
illustrative):

```text
/hello.txt           "Hello from ext4!\n"
/readme.md           multi-line markdown content
/subdir/             directory
/subdir/nested.txt   "nested file"
/large.bin           a file large enough to span multiple extents
                     (e.g. 1 MB of repeating-pattern data)
/notes.neom          short file with .neom extension — must import as
                     Regular, never Model
/link_to_hello       symbolic link to /hello.txt — must NOT appear in
                     readdir output
```

The `large.bin` entry verifies extent-chain read for files spanning
multiple extents. The `notes.neom` entry verifies the Article 10.3
invariant. The `link_to_hello` entry verifies symlink skipping.

If feasible, add an xattr to one of the fixtures to verify xattr-ignore:

```text
debugfs: ea_set /notes.neom user.neoplutarch.type Model
```

If `debugfs`'s xattr set is unreliable or unavailable, surface and PM
will accept testing the xattr-ignore invariant via a synthetic
in-memory test instead.

**libneo additions.**

Add fs_type constant:

```text
MOUNT_FS_TYPE_EXT4_DEVICE2_RO = 4
```

No new flag bits, no new syscalls.

**Scenarios.**

Required new scenarios:

```text
phase3_ext4_mount_readonly
phase3_ext4_readdir_root
phase3_ext4_open_read_regular_file
phase3_ext4_subdir_walk
phase3_ext4_extent_chain_read
phase3_ext4_write_is_rejected
phase3_ext4_exec_blocked_by_default
phase3_ext4_exec_with_opt_in
phase3_ext4_type_import_is_regular
```

Scenario expectations:

- `phase3_ext4_mount_readonly` — mount fs_type=4 at a child name with
  `MOUNT` right; receive rc=0; mount appears in namespace.
- `phase3_ext4_readdir_root` — enumerate root entries; verify presence of
  fixture Regular and Directory entries; verify symlink is SKIPPED (not
  in output); verify special file types absent; verify xattr-bearing
  file (notes.neom) imports as Regular.
- `phase3_ext4_open_read_regular_file` — open `/ext4/hello.txt` through
  the mount, read content, verify bytes match xtask fixture.
- `phase3_ext4_subdir_walk` — open `/ext4/subdir/nested.txt`; read; verify.
- `phase3_ext4_extent_chain_read` — open `/ext4/large.bin`; read all
  bytes; verify content matches xtask-generated pattern; explicitly
  verify that the file spans more than one extent record (this is the
  load-bearing test for extent tree walking).
- `phase3_ext4_write_is_rejected` — open `/ext4/hello.txt` with `WRITE`
  requested; receive `InsufficientRights` or `EACCES`. Foreign-FS is
  read-only regardless of FileCap.
- `phase3_ext4_exec_blocked_by_default` — mount with default flags, open
  a regular file with `EXECUTE`; receive `EACCES`.
- `phase3_ext4_exec_with_opt_in` — mount with `MOUNT_FLAG_EXEC_ALLOWED`;
  open a regular file with `EXECUTE`; receive success. Do NOT actually
  execute.
- `phase3_ext4_type_import_is_regular` — enumerate the root; specifically
  verify `notes.neom` imports as kind=Regular (kind=2 in NeoDirentV1),
  never kind=Model (kind=4), even though it has an xattr that
  hypothetically claims to override the type. The Bible Article 10.3 +
  ADR-021 Q9 invariant under ext4.

### 3.2 Anti-Scope

NOT in this BUILD:

- ext4 write support;
- ext3 compatibility;
- ext2 compatibility;
- legacy block-mapping inodes (non-extent);
- HTree index parsing;
- journal replay (read-only; we don't need it);
- ACL enforcement (xattrs read but ignored, per §3.1);
- symlink traversal or symlink presentation as Regular (symlinks SKIPPED
  in readdir);
- hard link detection or deduplication;
- xattrs surfaced to userspace;
- sparse-file hole optimization (read zeros);
- encrypted directories or encrypted files (refuse to read; if the
  superblock indicates an encrypted volume, fail mount with a clear
  error);
- quota;
- snapshots;
- ext4-on-device1 (FAT32 owns device1; ext4 uses device2);
- multi-device ext4 (one image, one mount);
- mount namespaces, bind mounts, rootfs pivot, dentry cache, file-backed
  mmap/exec, broad POSIX expansion (all BUILD-4B/4C anti-scope inherited);
- BUILD-3F CAS/dedup;
- BUILD-5 AHCI/NVMe/bare-metal.

## 4. Architecture Notes

### 4.1 MountedFs abstraction validation — third filesystem

The substrate-validation test from BUILD-4C extends. ext4 is substantially
more complex than FAT32 (extent trees, block-group descriptors, larger
inode structures, xattr blocks). If `MountedFs` accepts ext4 without
trait modification, the abstraction is validated against meaningful
complexity diversity, not just FAT32-vs-NeoFS.

If the trait needs to change, that is a substrate-leakiness signal and
a stop condition. Surface before modifying.

### 4.2 Extent tree walking

The ext4 extent tree is a B+ tree rooted in the inode's `i_block` field.
For files small enough that the inode-root's four extent records
suffice, no external blocks are read. For larger files, the inode root
holds extent-index entries pointing to extent blocks, which themselves
may be index or leaf nodes.

The extent tree depth is unbounded in theory but practically rarely
exceeds 2-3 levels. BUILD-4D should support arbitrary depth via a
recursive or iterative walk; do not assume depth ≤ 1.

Test fixture `large.bin` must be sized to force at least two leaf
extent records, ideally also a deep-enough tree that an external extent
block is involved. Empirically, ~1 MB of data with some intentional
fragmentation typically suffices, but Jarvis sizes the fixture
appropriately.

### 4.3 Inode size handling

Modern ext4 typically uses 256-byte inodes (the superblock's
`s_inode_size` field). The standard inode struct is 128 bytes;
additional fields (timestamps with nanosecond precision, fast xattrs)
occupy bytes 128-255. BUILD-4D respects `s_inode_size` and does not
assume 128.

### 4.4 IF discipline

Same posture as BUILD-4C. ext4 driver code runs from syscall context
(IF=1) and possibly from storage-completion paths. If a new lock is
required for ext4-internal state, it must have documented interrupt-
state discipline at the declaration site, AND the close artifact must
explicitly name the lock type (`SwitchMutex`, `spin::Mutex`, etc.) —
this is the documentation hygiene PM directed after BUILD-4C's
`VIRTIO_DEVICES` lock-type ambiguity.

The virtio-blk infrastructure now polls all devices (BUILD-4C fix in
commit `a627d670`). ext4-on-device2 reuses this machinery without
additional driver-level changes; if a driver-level change IS required,
surface as deviation.

## 5. Verification Gates

Container-canonical idiom (per BUILD-4C precedent):

```text
docker compose run --rm neo bash -c 'which mke2fs e2cp debugfs dumpe2fs'
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
docker compose run --rm neo cargo neo-test phase3_ext4_mount_readonly
docker compose run --rm neo cargo neo-test phase3_ext4_readdir_root
docker compose run --rm neo cargo neo-test phase3_ext4_open_read_regular_file
docker compose run --rm neo cargo neo-test phase3_ext4_subdir_walk
docker compose run --rm neo cargo neo-test phase3_ext4_extent_chain_read
docker compose run --rm neo cargo neo-test phase3_ext4_write_is_rejected
docker compose run --rm neo cargo neo-test phase3_ext4_exec_blocked_by_default
docker compose run --rm neo cargo neo-test phase3_ext4_exec_with_opt_in
docker compose run --rm neo cargo neo-test phase3_ext4_type_import_is_regular
```

Anti-regressions (BUILD-4B and BUILD-4C-era scenarios must remain green):

```text
phase3_mount_neofs_device0_readonly
phase3_mount_requires_mount_right
phase3_umount_busy_is_ebusy
phase3_fat32_mount_readonly
phase3_fat32_readdir_root
phase3_fat32_open_read_regular_file
phase3_fat32_lfn_lookup
phase3_fat32_write_is_rejected
phase3_fat32_exec_blocked_by_default
phase3_fat32_type_import_is_regular
phase3_neofs_readdir_root
phase3_neofs_filecap_generation_stale
phase3_neofs_fsync_filecap_stale_regression
phase3_sched_wake_token_abandon
phase3_vfs_directory_read_is_eisdir
ipc_heap_kill_smoke
cap_revoke_local
```

Final close gate:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Literal `NEO_TEST/1 event=PASS name=<scenario>` lines required for every
new scenario in the close artifact.

## 6. Stop Conditions

Stop and surface to PM before committing if:

- e2fsprogs tooling is missing inside the dev container (would require
  Dockerfile amendment lane).
- `MountedFs` trait requires modification to accommodate ext4. The trait
  is supposed to be filesystem-agnostic; if ext4 needs special-casing
  in the trait, the substrate abstraction is leaky.
- An ext4 inode in the test fixture turns out to use legacy block-
  mapping rather than extents. If `mke2fs -t ext4` is producing
  non-extent inodes for some reason, surface — do not implement
  block-mapping support to work around it.
- HTree index parsing turns out to be required for any test fixture's
  directory walks to succeed. (It shouldn't — linear entries are
  always present — but surface if encountered.)
- xattr-ignore invariant cannot be tested because the fixture cannot
  carry an xattr. Surface; PM will accept a synthetic in-memory test
  as alternative.
- An encrypted-volume superblock is encountered during testing. Surface.
- The extent tree walk requires recursion deeper than reasonable for
  fixture sizes (≥ 4 levels). Surface; this would suggest a fixture
  sizing problem, not a code problem.
- A new lock site is introduced without explicit interrupt-state
  discipline AND explicit lock-type naming at the declaration site
  (the BUILD-4C close-artifact hygiene rule, now mandatory).
- New unjustified warnings appear.
- Any anti-scope item starts appearing in the diff.

## 7. Required Close Artifact

Create:

```text
Plans/PHASE-3-BUILD-4D-CLOSE-EVIDENCE-2026-05-20.md
```

(Or a later date if BUILD-4D takes multiple days; date matches final
commit.)

Required content:

- container-canonical e2fsprogs verification output (the §2 first step);
- commit hash(es) — implementation and close-evidence as in BUILD-4C;
- diff stat;
- build outputs with warning count;
- literal `NEO_TEST/1 event=PASS` lines for all nine new scenarios;
- targeted anti-regression results (including all BUILD-4C scenarios);
- final `NEO_SMP=4 --no-meta` result;
- explanation of where the ext4 driver lives and how it composes with
  the existing `MountedFs` trait;
- explanation of extent tree walking implementation and depth handling;
- explanation of symlink/special-file skipping behavior;
- explanation of xattr-ignore (Article 10.3 invariant) handling;
- explicit confirmation: "ext4 implementation did not require
  modifications to the `MountedFs` trait" — or, if it did, a stop
  condition was already surfaced;
- explicit confirmation: "no ext4 write code landed; no ext3/ext2 code
  landed; no legacy block-mapping code landed; no HTree index parsing
  landed";
- explicit confirmation: "all ext4 directory entries imported as
  `Regular` or `Directory`; symlinks and special types skipped; `.neom`
  imported as `Regular` regardless of xattr";
- explicit naming of any new lock site WITH lock type AND interrupt-
  state discipline location (BUILD-4C hygiene rule);
- deviations / course corrections.

Suggested implementation commit subject:

```text
build-4D: add ext4 read-only foreign filesystem driver
```

## 8. PM Posture

PM monitoring. Three lanes after this dispatch:

- **Jarvis:** BUILD-4D (this dispatch).
- **Cerebro:** idle.
- **Prometheus:** idle.

This dispatch closes BUILD-4D's authorization gap and brings Phase 3 to
its final pre-bare-metal lane. After BUILD-4D closes, Phase 3 awaits
BUILD-4E (integration close) and BUILD-5 (AHCI/NVMe + bare-metal
validation; pending PL hardware acquisition).

Expected turnaround: ext4 is meaningfully more complex than FAT32, so
BUILD-4D will likely take longer than BUILD-4C. PM does not impose a
hard timeline; quality over speed per Bible Article 1.2. Surface stop
conditions immediately; do not work around them.

The Bible's posture on ext4 — "We will not ship write support for
foreign filesystems unless absolutely necessary" (Article 10.4) — is
explicit. Do not be tempted into ext4 write support even if the parser
naturally suggests it; the journal layer would have to land too.
