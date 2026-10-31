# PM Disposition - Phase 3 BUILD-4D Close Consumption And Ratification

Date: 2026-05-20
From: PM
Status: ACCEPTED / BUILD-4D CLOSED / PUSH-AND-MERGE AUTHORIZED
Consumed artifact:

```text
Plans/PHASE-3-BUILD-4D-CLOSE-EVIDENCE-2026-05-20.md
```

Commits:

```text
aca3fb19256c9ccfde6f19e01aa957c141baec63  build-4D: add ext4 read-only foreign filesystem driver
fb70057b74e34e8ab6f1da508289c99fdef5b97e  plans: add BUILD-4D close evidence
```

Diff: 6 files, 1457 insertions, 21 deletions.

## 1. Disposition

BUILD-4D is closed. All dispatch acceptance gates from
`Plans/DISPATCH-2026-05-20-phase-3-build-4D-ext4-readonly-jarvis.md` are
met. PM consumes the close artifact and authorizes push and merge of
both commits to main.

## 2. Gate Verification

PM walked the close artifact against the dispatch contract; every gate
passes:

- Pre-implementation verification (§2): canonical e2fsprogs check ran;
  `mke2fs`, `debugfs`, `dumpe2fs` confirmed present at `/usr/sbin/`;
  `e2cp` absent but pre-authorized as fallback to debugfs per dispatch
  §3.1.
- ext4 parser (§3.1): superblock, group descriptors, extent-tree read
  with recursive depth handling, linear directory walk; all present.
- Inodes without `EXT4_EXTENTS_FL` are rejected per dispatch (no legacy
  block-mapping path).
- HTree index not consulted; linear directory entries walked.
- Symlinks and special file types skipped in readdir output;
  `phase3_ext4_readdir_root` scenario emits `symlink_skipped=true`.
- xattrs read but ignored for type import; `phase3_ext4_type_import_is_regular`
  scenario emits `xattr_type_ignored=true model_imports=0` for a `.neom`
  file carrying `user.neoplutarch.type=Model`. The Article 10.3 +
  ADR-021 Q9 invariant is verified end-to-end under ext4.
- `MountedFs` trait did NOT require modification (§3 explicit
  confirmation). Third filesystem under the substrate, including extent
  trees + group descriptors + variable-size inodes + xattrs, and the
  trait still composes without change.
- `fs_type = 4` for ext4 device2 readonly; libneo constant added.
- Mount-flags ABI unchanged from BUILD-4C; `MOUNT_FLAG_EXEC_ALLOWED`
  semantics reused.
- Mount rights mask: WRITE always cleared; EXECUTE conditional on flag.
- xtask ext4 image generation via mke2fs + debugfs; debugfs `filefrag`
  validates the 50-extent fragmented fixture; debugfs `ea_list` validates
  xattr presence; no committed binary fixtures.
- All nine required scenarios PASS with literal `NEO_TEST/1 event=PASS`
  frames.
- All seventeen anti-regression scenarios PASS, including the full
  BUILD-4C FAT32 suite and the NeoFS generation-stale lanes.
- `NEO_SMP=4 cargo neo-test --no-meta` exits 0.
- `cargo fmt --check` PASS (additional gate Jarvis added beyond
  dispatch requirements; PM accepts).
- Warning count 52 (unchanged from BUILD-4C close baseline).

## 3. Substantive Disclosures

### 3.1 The substrate-agnosticism result is now load-bearing

Three filesystems under the `MountedFs` substrate without trait
modification:

1. NeoFS (BUILD-3): native filesystem with generation-stale semantics
2. FAT32 (BUILD-4C): foreign filesystem with VFAT LFN, cluster chains
3. ext4 (BUILD-4D): foreign filesystem with extent trees, group
   descriptors, variable-size inodes, xattrs

The complexity span between FAT32 and ext4 is meaningful: ext4 has
fundamentally different on-disk structures (B+ extent trees vs cluster
chains, group descriptors vs nothing equivalent, variable-size inodes
vs fixed). The fact that the same `MountedFs` trait accepts both
without modification is the engineering result the BUILD-4B abstraction
was designed to produce.

PM records this explicitly because future filesystem additions (e.g.
BUILD-5+ bare-metal NVMe drivers exposing additional native or foreign
filesystems) can now rely on the substrate's filesystem-agnosticism
as an established property, not a speculative one.

### 3.2 No new lock sites

BUILD-4D introduced no new locks. The lock-hygiene rule PM established
after BUILD-4C (lock type + IF discipline location must be named in
close evidence) was therefore not exercised. The rule remains in force
for future builds; BUILD-4D simply did not need to invoke it.

This is the cleanest possible outcome on the concurrency side. ext4's
read-only nature means most of its state is mount-immutable
(snapshotted at mount time) and per-call state lives on the syscall
stack.

### 3.3 Performance optimization disclosed

Jarvis disclosed in §"Deviations And Disposition" that an early ext4
large-file read path reread the whole file per VFS read call, which
timed out in QEMU during scenario testing. The final implementation
uses range reads over extent overlaps; the 50-extent fixture in
`phase3_ext4_extent_chain_read` validates the optimization.

PM accepts this as in-scope. Discovery-and-fix-during-implementation
for a performance regression caught by the dispatched scenarios is
exactly the workflow the scenario set was designed to surface. The
fix is contained to the ext4 driver; no substrate change was required.

For future awareness: this is the second BUILD in a row (after
BUILD-4C's virtio-blk completion polling fix) where a
discovered-during-implementation infrastructure issue surfaced
naturally from the scenario set. The scenario-driven discipline is
working as intended.

## 4. Phase 3 Status After BUILD-4D

- **BUILD-1, 2, 3, 4A, 4B, 4C, 4D:** closed.
- **BUILD-4E (integration close / userspace polish):** ahead — see §5.2.
- **BUILD-5 (AHCI/NVMe + bare-metal validation):** ahead. Bare-metal
  target hardware acquisition still pending PL commitment.

Phase 3 is now six-sevenths closed by BUILD count. The substrate work
of Phase 3 is substantially complete; what remains is integration
closure and bare-metal validation.

## 5. Authorized Forward Lanes

### 5.1 Push and merge

Per BUILD-4B/4C precedent, push and merge of commits `aca3fb19` and
`fb70057b` to main is now authorized. Standard git posture from current
handoff applies.

### 5.2 BUILD-4E question for PL

ADR-021's BUILD-4 boundaries named: "Foreign FS read-only + userspace
polish." The sub-builds delivered:

- BUILD-4A: directory enumeration + `sys_readdir`
- BUILD-4B: mount table + `sys_mount` / `sys_umount`
- BUILD-4C: FAT32 read-only
- BUILD-4D: ext4 read-only

The "userspace polish" piece of BUILD-4's scope — broader POSIX-ish
syscall surface (e.g. `stat` equivalents, additional foreign-FS-level
syscalls) — has not been explicitly delivered as a discrete BUILD.

PM surfaces three options to PL:

**Option A: Declare BUILD-4 complete at BUILD-4D.** Treat the original
"userspace polish" framing as folded into the four sub-builds already
delivered. Phase 3 proceeds directly to BUILD-5 (bare-metal). The
"polish" syscalls (stat-equivalents, etc.) become Phase 4 or later work
when there's a userspace consumer that needs them.

**Option B: Open BUILD-4E as a small integration-close lane.** A
dedicated dispatch for any remaining BUILD-4 syscalls or integration
testing PL wants before BUILD-5. Likely small, possibly Cerebro's
return-to-active-lane.

**Option C: Open BUILD-4E as a deferred placeholder.** Declare the
BUILD-4 substrate complete, but reserve BUILD-4E for any specific
foreign-FS integration work that surfaces between now and Phase 3
closure (likely none).

PM recommendation: **Option A.** The four sub-builds delivered cover
the load-bearing foreign-FS scope. "Userspace polish" without a
userspace consumer is speculative work; the right time to add stat-
equivalents or similar syscalls is when something userspace-side needs
them, which is plausibly Phase 4+ when the network stack and broader
userland start exercising the filesystem layer. Bible Article 1.3
discourages speculative scope; folding BUILD-4 closed at BUILD-4D and
revisiting polish syscalls when demand surfaces matches that
discipline.

PL's call.

### 5.3 BUILD-5 bare-metal target — still pending PL hardware

The T480s 20L8 acquisition path remains the front-runner. PM does not
authorize BUILD-5 in this disposition. BUILD-5 dispatch follows once
PL commits to specific hardware and the bare-metal validation lane
opens.

### 5.4 Small follow-up tasks (opportunistic, all non-blocking)

- Documentation hygiene: BUILD-4D close evidence does not name the diff
  stat outside the implementation commit (i.e. the close-evidence
  commit's stat). Trivial; future close artifacts may want to include
  both. Not a blocker.
- The substrate-agnosticism result (per §3.1 above) is significant
  enough that a brief addition to ADR-021's accomplishments section, or
  a small documentation note, might be valuable for future reference.
  Opportunistic Prometheus task; not urgent.

## 6. PM Posture

Lanes after this disposition:

- **Cerebro:** idle.
- **Jarvis:** BUILD-4D closed, idle. Available for BUILD-4E or BUILD-5
  when authorized.
- **Prometheus:** idle.

The day's progress: BUILD-4B, BUILD-4C, BUILD-4D, the Jarvis FD-teardown
audit, the ANSPEC-002 v0.1.1 amendment, the three-item Prometheus
bundle, and the ADR-022 ratification all closed today. Two stop-
condition surfaces handled cleanly (mtools-in-container, Prometheus
wrong-file). Three sub-builds of BUILD-4 closed in one day, including
two foreign-filesystem drivers, with no substrate modifications
required.

PM stands ready for:

- PL's BUILD-4E option signal (§5.2).
- PL's BUILD-5 bare-metal target signal (§5.3).
- Any other PL direction.
