# Dispatch - Phase 3 BUILD-4C FAT32 Read-Only Foreign Filesystem

Date: 2026-05-20
Agent: Jarvis
Status: AUTHORIZED IMPLEMENTATION

Authority:

- `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4B-close-consumption.md` (BUILD-4B
  closure; mount-table substrate now available)
- `Plans/PHASE-3-BUILD-4B-CLOSE-EVIDENCE-2026-05-20.md` (commit `3097126`; mount
  ABI, `MountedFs` trait, live-handle accounting, `fs_type` discriminator)
- `Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md` (BUILD-4
  split; foreign-FS tooling availability; D9 FAT32-before-ext4; D10 noexec
  policy)
- `Plans/PM-DISPOSITION-2026-05-16-phase-3-build-4-prereq-audit-consumption.md`
  (D10 noexec accepted)
- `docs/ADR-021-phase-3-design-substrate.md` BUILD-4 boundaries; Q9 foreign-FS
  type import to Regular; foreign-FS xattr distrust
- `docs/bible.md` v1.6.0 Articles 10.3 (typed inodes), 10.4 (FAT32/ext4 read-only
  interop), 12.5 (model weights as data)

Substrate:

```text
3097126 build-4B: add mount table and directory-capability mount syscalls
```

## 1. Mission

Land a minimal FAT32 read-only driver as a `MountedFs` implementation under
the BUILD-4B mount-table substrate. Extend the mount-flags ABI to carry
noexec policy. Type import defaults to `Regular`; everything else in the
foreign-FS surface is anti-scope.

The mission is the substrate proof that BUILD-4B's mount table is genuinely
filesystem-agnostic — i.e. NeoFS-specific assumptions did not leak into the
mount-table abstraction. If FAT32 needs anything special-cased into the
mount table, that's a stop condition.

## 2. Scope

### 2.1 In Scope

**FAT32 on-disk parser (read-only).**

- BPB / extended BPB validation;
- FAT region read (FAT32 specifically; no FAT12/FAT16 fallback path);
- root directory cluster chain walk;
- subdirectory walk;
- short (8.3) directory entries;
- VFAT long filename (LFN) entries with checksum validation;
- file cluster chain read;
- case-insensitive lookup, case-preserving display per VFAT convention.

**MountedFs implementation.**

- `MountedFs::root()` returns the FAT32 root directory inode;
- `MountedFs::lookup(relative_path)` walks the chain;
- Inode kinds limited to `Regular` and `Directory`;
- Foreign-FS inodes do not carry NeoFS generation tokens; FAT32 has no
  inode-generation concept, so FileCap staleness is anchored differently.
  Use an immutable-mount-snapshot model: the mount's FAT32 image is treated
  as immutable for the duration of the mount; FileCap generation always
  matches; ESTALE is not returnable from FAT32 in BUILD-4C.

**fs_type discriminator extension.**

Add to the BUILD-4B fs_type enumeration:

```text
3 = FAT32 device1 readonly
```

`source` field semantics extend: `device1` is the second virtio-blk device
attached at QEMU launch. Reuse the existing block-device registry. Do not
introduce a generic device-string parser; the test harness mounts a known
device index.

**Mount-flags ABI extension.**

The BUILD-4B `flags: u32` parameter currently accepts only `0 = default`.
Extend with:

```text
bit 0: MOUNT_FLAG_EXEC_ALLOWED  -- if set, regular files on this mount
                                   may be executed; if clear, exec returns
                                   TypeForbidden regardless of FileCap rights
```

All other bits remain reserved and return `EINVAL` if set.

**Default policy:** native NeoFS mounts continue to default `EXEC_ALLOWED=0`
implicit (existing NeoFS executable policy unchanged from BUILD-2/4A). FAT32
mounts default `EXEC_ALLOWED=0` (noexec) per D10. Tests cover both the
default-blocked path and the explicit-opt-in path.

**Mount rights mask.**

`MountEntry` gains a `rights_mask: FileRights` field. VFS open computes
effective rights as:

```text
effective = inode.available_rights() & mount.rights_mask
```

For NeoFS mounts: `rights_mask = FileRights::all()` (no attenuation; existing
behavior preserved).

For FAT32 mounts:
- default `rights_mask` clears `WRITE` (foreign-FS is read-only);
- if `MOUNT_FLAG_EXEC_ALLOWED` not set, clears `EXECUTE` as well;
- if `MOUNT_FLAG_EXEC_ALLOWED` is set, leaves `EXECUTE` intact (subject
  to inode-type policy: only `Regular` may execute; `Directory` and
  any future foreign-FS-imported types may not).

**xtask FAT32 image generation.**

Generate a deterministic FAT32 image into `target/` from xtask before
QEMU boots. Use the existing `mtools` tooling (`mformat -i image -F`,
`mcopy -i image hostfile ::/destfile`, `mmd -i image ::/subdir`). Image
contents must be fully reproducible from xtask invocation; no committed
binary fixtures.

Suggested fixture contents (Jarvis chooses concrete bytes; this is
illustrative):

```text
/hello.txt           "Hello from FAT32!\n"
/readme.md           multi-line markdown content
/subdir/             directory
/subdir/nested.txt   "nested file"
/long filename with spaces.txt   "LFN test"
```

The "long filename with spaces.txt" entry verifies VFAT LFN read support.

**libneo additions.**

Add fs_type constant and exec-allowed flag:

```text
MOUNT_FS_TYPE_FAT32_DEVICE1_RO = 3
MOUNT_FLAG_EXEC_ALLOWED        = 0x00000001
```

No new syscalls. Existing `sys_mount`/`sys_umount` accept the new fs_type
and the new flag bit.

**Scenarios.**

Required new scenarios:

```text
phase3_fat32_mount_readonly
phase3_fat32_readdir_root
phase3_fat32_open_read_regular_file
phase3_fat32_lfn_lookup
phase3_fat32_subdir_walk
phase3_fat32_write_is_rejected
phase3_fat32_exec_blocked_by_default
phase3_fat32_exec_with_opt_in
phase3_fat32_type_import_is_regular
```

Scenario expectations:

- `phase3_fat32_mount_readonly` — mount fs_type=3 at a child name with `MOUNT`
  right; receive rc=0; mount appears in namespace.
- `phase3_fat32_readdir_root` — enumerate root entries; verify presence of
  fixture files in case-preserving display.
- `phase3_fat32_open_read_regular_file` — open `/hello.txt` through the
  mount, read content, verify bytes match xtask-generated fixture.
- `phase3_fat32_lfn_lookup` — look up "long filename with spaces.txt" through
  the mount; case-insensitive lookup variant works (e.g. uppercase form
  resolves to same inode).
- `phase3_fat32_subdir_walk` — open `subdir/nested.txt` through the mount;
  read content; verify.
- `phase3_fat32_write_is_rejected` — open `/hello.txt` with `WRITE`
  requested; either receive `InsufficientRights` at open (if rights mask
  filters at open) or `InsufficientRights` at write (if it filters at write).
  Either is acceptable; document which in close evidence. Foreign-FS is
  read-only regardless of FileCap held.
- `phase3_fat32_exec_blocked_by_default` — mount with default flags, open a
  regular file with `EXECUTE` requested; receive `InsufficientRights` or
  the appropriate errno reflecting the cleared-EXECUTE rights mask.
- `phase3_fat32_exec_with_opt_in` — mount with `MOUNT_FLAG_EXEC_ALLOWED`;
  open a regular file with `EXECUTE`; receive success. Do NOT actually
  execute; this test verifies the policy path, not the ELF loader.
- `phase3_fat32_type_import_is_regular` — enumerate the root; verify every
  non-directory entry imports as `Regular` (kind=2 in NeoDirentV1), never
  `Model` (kind=4) or `Symlink` (kind=3), regardless of filename extension.
  Specifically include a `.neom` fixture file and verify it imports as
  `Regular`, not `Model`. This is the Bible Article 10.3 + ADR-021 Q9
  invariant.

### 2.2 Anti-Scope

NOT in this BUILD:

- FAT32 write support;
- exFAT, FAT12, FAT16;
- ext4 (that's BUILD-4D);
- mount namespaces;
- symlink traversal (FAT32 has no symlinks; for the unlikely case of a FAT32
  variant with symlinks, ignore them);
- hard links;
- dentry cache;
- rootfs pivot;
- file-backed mmap/exec;
- `stat`/`lseek`/`unlink`/`rename`;
- ext4 xattr import (out of BUILD-4C scope; xattr distrust applies to
  BUILD-4D when ext4 lands);
- noexec enforcement extensions beyond the EXEC_ALLOWED mount flag;
- multi-device FAT32 mount (one device, one mount in BUILD-4C);
- FAT32 access via paths longer than the BUILD-4B single-child-name target
  (deferred to whatever phase introduces nested mount targets, if any);
- BUILD-3F CAS/dedup;
- BUILD-5 AHCI/NVMe/bare-metal work;
- Any change to the NeoFS code path beyond `MountedFs` trait implementation
  refinement if absolutely required.

## 3. Architecture Notes

### 3.1 Mount-table abstraction validation

The BUILD-4B `MountedFs` trait should accept FAT32 without modification.
If the trait needs to change to accommodate FAT32, that is itself
significant — surface it before committing the change. The expected
modification surface for FAT32 is *zero* on `MountedFs` itself; any
FAT32-specific helpers (cluster chain iterators, BPB parsers) live in the
FAT32 module.

### 3.2 FAT32 inode shape

FAT32 inodes need a `cluster_chain_start: u32` and `size: u64` for files,
plus the directory-entry's attributes. Choose a representation that
composes cleanly with the existing `Inode` trait. Foreign-FS inodes are
distinct from NeoFS inodes; they share the `Inode` trait but their backing
storage differs.

For generation handling: FAT32 has no per-file generation counter. The
mount-immutable-snapshot model means FileCap captures `MountId` and the
mount's snapshot is stable; any FileCap from a FAT32 mount remains valid
until the mount is unmounted, at which point the cap becomes stale via
BUILD-4B's existing live-handle accounting (the mount can't be unmounted
while caps reference it).

### 3.3 noexec mount flag plumbing

Implementation thinking (not prescriptive):

1. `MountEntry` gains `rights_mask: FileRights` and `flags: u32` fields;
2. `vfs_mount_at_parent` reads the request flags, computes the rights mask
   from fs_type defaults plus per-mount flags;
3. `OpenFile::new_with_mount` consults the mount's rights mask before
   sealing the OpenFile's rights;
4. effective rights = inode.available_rights() & mount.rights_mask;
5. `OpenFile::check_executable_policy()` remains the final type-first,
   rights-second gate.

### 3.4 IF discipline

FAT32 driver code runs from syscall context (IF=1) and possibly from
storage-completion paths. No new `SwitchMutex` site should be needed if
FAT32 state is owned by the mount entry and accessed under the existing
`MOUNTS` lock. If a new lock is required for FAT32-internal state, it
must have documented interrupt-state discipline at the declaration site,
per the ongoing mixed-IF discipline carry-forward from
`Plans/PM-DISPOSITION-2026-05-20-wake-token-state-if-invariant-correction-consumption.md`.

## 4. Verification Gates

Run in order:

```text
cargo k
cargo k --no-default-features
cargo neo-test phase3_fat32_mount_readonly
cargo neo-test phase3_fat32_readdir_root
cargo neo-test phase3_fat32_open_read_regular_file
cargo neo-test phase3_fat32_lfn_lookup
cargo neo-test phase3_fat32_subdir_walk
cargo neo-test phase3_fat32_write_is_rejected
cargo neo-test phase3_fat32_exec_blocked_by_default
cargo neo-test phase3_fat32_exec_with_opt_in
cargo neo-test phase3_fat32_type_import_is_regular
```

Then anti-regressions (BUILD-4B-era scenarios must remain green):

```text
cargo neo-test phase3_mount_neofs_device0_readonly
cargo neo-test phase3_mount_requires_mount_right
cargo neo-test phase3_umount_busy_is_ebusy
cargo neo-test phase3_neofs_readdir_root
cargo neo-test phase3_neofs_filecap_generation_stale
cargo neo-test phase3_neofs_fsync_filecap_stale_regression
cargo neo-test phase3_sched_wake_token_abandon
cargo neo-test phase3_vfs_directory_read_is_eisdir
cargo neo-test ipc_heap_kill_smoke
cargo neo-test cap_revoke_local
```

Final close gate:

```text
NEO_SMP=4 cargo neo-test --no-meta
```

Literal `NEO_TEST/1 event=PASS name=<scenario>` lines required for every
new scenario in the close artifact.

## 5. Stop Conditions

Stop and surface to PM before committing if:

- `MountedFs` trait requires modification to accommodate FAT32. The trait
  is supposed to be filesystem-agnostic; if FAT32 needs special-casing
  in the trait, the BUILD-4B abstraction is leaky and that's a substrate
  issue not solvable inside BUILD-4C scope.
- A regular file on FAT32 cannot be expressed as an `Inode` without
  changes to the `Inode` trait. Same reasoning as above; the trait is
  the abstraction line.
- mtools tooling turns out to be unavailable or insufficient in the dev
  container at runtime (Jarvis's audit confirmed presence as of 2026-05-16,
  but if the tooling has changed, surface). Do not work around by adding
  binary FAT32 fixtures to the repo.
- The noexec mount flag plumbing requires a syscall-ABI change beyond
  the dispatch-authorized single bit. Specifically, do not change
  `MountRequestV1`'s layout; the `flags` field on the syscall is the
  authorized extension point.
- Any FAT32 fixture must be larger than ~64 KiB. The image as a whole may
  be larger (FAT32 minimum is ~33 MB for valid format), but individual
  fixture files should be small for test determinism.
- LFN handling requires unsigned hostile-input tolerance beyond Rust's
  natural type system (e.g. attacker-controlled FAT32 image with
  malformed LFN chains). Read-only is read-only, but malformed input
  must not panic. If a specific malformed-input vector can't be handled
  cleanly, surface it.
- A new `SwitchMutex` site is introduced without explicit interrupt-state
  discipline at the declaration site.
- New unjustified warnings appear.

## 6. Required Close Artifact

Create:

```text
Plans/PHASE-3-BUILD-4C-CLOSE-EVIDENCE-2026-05-20.md
```

(Or a later date if BUILD-4C takes multiple days to land; date matches
final commit.)

Required content:

- commit hash;
- diff stat;
- build outputs with warning count;
- literal `NEO_TEST/1 event=PASS` lines for all nine new scenarios;
- targeted anti-regression results;
- final `NEO_SMP=4 --no-meta` result;
- explanation of where the FAT32 driver lives and how it composes with
  the existing `MountedFs` trait;
- explanation of the mount-flags ABI extension (the EXEC_ALLOWED bit
  semantics, default behavior, errno disposition);
- explanation of the rights-mask composition (effective = inode rights
  & mount mask);
- explicit confirmation: "FAT32 implementation did not require
  modifications to the `MountedFs` trait" — or, if it did, exact
  description of what changed and why (which would be a stop-condition
  trigger if surfaced as required);
- explicit confirmation: "no FAT32 write code landed; no ext4 code
  landed; no exFAT/FAT12/FAT16 code landed";
- explicit confirmation: "all FAT32 directory entries imported as
  `Regular` or `Directory`; no entry imported as `Model` or `Symlink`
  regardless of filename extension";
- deviations / course corrections.

Suggested implementation commit subject:

```text
build-4C: add FAT32 read-only foreign filesystem driver
```

## 7. PM Posture

PM is in monitoring mode. Cerebro's BUILD-4B is closed and merged.
Prometheus is idle (ANSPEC-002 v0.1.1 ratified). Jarvis owns BUILD-4C
solo; surface stop conditions through PM if any trigger.

The Bible's posture on FAT32 — "We will not ship write support for
foreign filesystems unless absolutely necessary" (Article 10.4) — is
explicit. Do not be tempted into FAT32 write support even if the parser
naturally suggests it.

Anti-scope items in §2.2 are hard. Surface before drifting.
