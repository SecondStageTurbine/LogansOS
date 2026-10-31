# Phase 3 THINK v2 - NeoFS, block layer, real disk I/O

| | |
|---|---|
| **Date** | 2026-05-11 |
| **Author** | PM (NeoPlutarch Phase 3 cadence) |
| **Status** | Post-structural-challenge synthesis. Supersedes `Plans/PHASE-3-THINK-V1-2026-05-11.md`. |
| **Cadence** | Bible Article 14 THINK pass, revised after Cerebro #1 structural challenge. THINK v1 is now explicitly classified as pre-structural-challenge exploration. |
| **Consumes** | `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`; `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md`; `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md`; `Plans/PHASE-3-THINK-V1-2026-05-11.md`. |
| **Next artifact** | `docs/ADR-021-phase-3-design-substrate.md` |

---

## 0. Structural-challenge gate result

The live gate for this revision is: **should I be asking a different question entirely?**

For Phase 3 Q5, the answer was yes. THINK v1 asked: "How should NeoFS implement content-addressing at the block layer?" That question was coherent, but it was not the right first substrate question. The right first question is: **which properties belong to the first shippable NeoFS correctness substrate, and which properties are optimization features that must be measurement-gated?**

Cerebro #1's answer is now PM-ratified:

- First NeoFS BUILD ships a single-tier, physical-block-addressed CoW filesystem with Merkle-DAG integrity.
- Content-addressing as hash-as-address is deferred to a discrete measurement-gated BUILD.
- Crash consistency comes from CoW tree structure, redundant checksummed superblocks, generation counters, and FLUSH discipline, not from CAS.
- Deduplication is an optimization, not the Phase 3 correctness root.
- GC starts as mark-sweep when CAS lands; real-time refcounted dedup is a Phase 5+ forward marker unless measured workload proves otherwise.

This revision carries that reframe through the full Phase 3 Q-map, BUILD split, and dispatch cadence.

## 1. Mandate

Phase 3 closes Bible Article 13's third milestone: **NeoFS, block layer, real disk I/O.**

It delivers five substrate families:

1. **Block layer.** A kernel-internal abstraction over persistent block devices, including request issue, completion, DMA-buffer discipline, device-reported geometry, and the first synchronous I/O path.
2. **Storage drivers.** virtio-blk for VM testing, SATA AHCI for bare-metal coverage, and NVMe over PCIe for modern hardware, per Bible Article 9.9.
3. **VFS.** A POSIX-ish path namespace, mount hierarchy, file handles as capabilities, initial file syscalls, and typed-inode dispatch points.
4. **NeoFS.** The native filesystem. First shippable form is single-tier physical-block-addressed CoW with Merkle-DAG integrity and typed inode discriminants. CAS dedup follows only if the measurement gate earns it.
5. **Foreign read-only FS.** FAT32 and ext4 read-only interop per Bible Article 10.4.

The Phase 3 implementation must preserve Phase 2 invariants:

- single-thread-per-process remains in force;
- SC-3B `SwitchMutex` IF=0 discipline remains the wakeup substrate;
- SC-3C cross-CPU IPI wake becomes a general subsystem primitive through NeoFS I/O completion;
- SC-4 process-exit serialization handles "thread killed while blocked on I/O."

## 2. Why Phase 3 next

The corrected Phase ordering remains: Phase 3 before Phase 4 before Phase 5.

Bible Article 10 makes filesystem substrate a Phase 3 obligation. Bible Article 13 binds Phase 3 to "NeoFS, block layer, real disk I/O." Phase 5's AI subsystem depends on storage substrate for `.neom` files, model identity, model integrity, and eventual deduplication. Skipping to Phase 5 would either build a temporary model-storage path that NeoFS later replaces, or make Phase 5 silently depend on unimplemented filesystem behavior.

The Cerebro #1 reframe does not weaken Article 10.2. It resolves sequencing. Article 10.2 says NeoFS is content-addressed at the block layer and models are deduplicated by content hash. This remains the constitutional destination. The Phase 3 engineering conclusion is that eager CAS is not the first correctness substrate. The first correctness substrate must be a simpler single-tier CoW filesystem that reserves the format fields needed for CAS, then admits CAS behind a measured gate.

That sequencing is the ALTIMIT-faithful version of Article 10: do less in the kernel first, but do it rigorously enough that the later AI-native layer has a real substrate to stand on.

## 3. Existing substrate snapshot

The repo currently has boot/module loading and no persistent-storage stack:

```text
kernel/src/loader/
  elf.rs       - ELF64 loader from Limine modules
  mod.rs

kernel/src/init/
  boot_frame.rs
  mod.rs

kernel/src/drivers/
  console/
  fb/
  mod.rs

kernel/src/main.rs
  MODULE_REQUEST: ModulesRequest::new()
```

Phase 1/2 userspace loads from Limine modules. This remains the pre-Phase-3 stopgap named by Bible Article 10.1.

No block layer, block drivers, VFS, disk-backed filesystem, or persistent rootfs exists yet. Phase 3 is a clean-slate subsystem addition, not an extension of existing FS code.

Inherited Phase 2/ADR substrate that Phase 3 must respect:

- ADR-002 places filesystem code under `kernel/src/fs/`.
- ADR-010 defers capability copy/move and file-related capability semantics into Phase 3+.
- ADR-012 marks Arc-aware slab integration and allocator replacement as Phase 3+ candidates if filesystem workloads stress the heap.
- ADR-014 assumes a procfs-equivalent can land only after NeoFS exists.
- ADR-015 names Phase 3 NeoFS as the next priority after KASLR-tolerant heap placement.
- ADR-016 A1 (IRQ-safe heap + scheduler decoupling) is a Phase 3 implementation item and should be coordinated with BUILD-1.

### 3.1 Substrate review item: kthread primitive

Cerebro #1 surfaced a Phase 3 prerequisite for BUILD-3: a background kernel-thread primitive usable by subsystems outside the scheduler.

BUILD-1 and BUILD-2 do not need background FS threads. Synchronous I/O runs in the syscalling process context, and NeoFS native v1 can avoid buffer-cache write-back and CAS GC.

BUILD-3 does need background work:

- buffer-cache write-back;
- periodic or watermark-triggered mark-sweep GC;
- FLUSH batching;
- possible advisory-refcount reconciliation.

PM action before BUILD-3 PLAN dispatch: audit whether the current scheduler exposes a kernel-internal kthread spawn primitive to non-scheduler subsystems. If not, add a kernel-side primitive, likely `kernel/src/sched/kthread.rs`, with capability/lifetime discipline. FS instances should own their kthread handles; unmount should reap them through the normal exit path.

This is also Phase 4 preparation. The network stack will need background timers, retransmit work, and queue drainers. Resolving kthreads for NeoFS pays that bill early.

## 4. Bible and ANSPEC bindings

### 4.1 Bible Article 9.9

Phase 3 must support:

- virtio-blk, required for VM testing;
- SATA AHCI, required for Phase 3 bare-metal storage;
- NVMe over PCIe, required for modern hardware coverage.

USB mass storage is Phase 4+. eMMC/SD is Phase 5+. IDE is unsupported.

### 4.2 Bible Article 10

Article 10 remains normative:

- NeoFS is the native filesystem designed in Phase 3.
- NeoFS has a POSIX-ish path namespace.
- NeoFS is content-addressed at the block layer.
- models are deduplicated by content hash.
- file type is inode-level, not filename-level.
- `.neom` model files can be refused as executable mappings by the kernel.
- FAT32 and ext4 are read-only interop filesystems.

Phase 3 v2 interpretation:

- BUILD-2 satisfies "NeoFS exists" with a forward-compatible single-tier CoW format.
- BUILD-3 is the first candidate for satisfying "content-addressed at block layer" if measurement earns it in Phase 3.
- If BUILD-3 is deferred to Phase 5, Phase 3 still ships the format hooks and integrity hashes needed for Article 10.2, and Phase 5 must explicitly close the CAS substrate.
- Typed inode discriminants ship in BUILD-2 because Bible 10.3 is a correctness/security requirement, not an optimization.

### 4.3 ANSPEC-001

ANSPEC-001 depends on Phase 3 in four places:

- `.neom` model files require a real FS substrate and a model inode type.
- model load integrity needs stable content hashes and explicit file identity.
- model weight dedup wants content-hash identity, but can wait until CAS is justified.
- model-as-code confusion attacks are mitigated by typed inode refusal of executable mappings.

The most important sequencing correction: Phase 3 need not eagerly build full ZFS-DDT-style model dedup to unblock Phase 5. It must build the file typing, integrity, and future CAS hooks cleanly. Phase 5 can then either consume Phase 3 BUILD-3 CAS or trigger the CAS work as Phase 5 substrate if model workloads are the first measured driver.

## 5. Design surface and PM resolutions

This section supersedes THINK v1 section 4. The Q-map remains useful, but several answers change.

### Q1 - Block layer abstraction

**Decision:** synchronous-blocking first, per-device request queue, explicit completion rendezvous.

The kernel has no general async runtime and does not need one for Phase 3. Synchronous I/O is the original Unix filesystem model and composes with single-thread-per-process. The hardware path is still asynchronous internally: DMA issue, interrupt completion, wake blocked thread.

Initial `BlockDevice` shape should support:

- device-reported logical block size and physical block size;
- read/write of block-aligned buffers;
- request submission returning a completion token or blocking result;
- optional FLUSH capability, absent in BUILD-1 virtio-blk write-through mode;
- a stable device identifier for mount-by-device and later rootfs selection.

Per-device single queue is sufficient. blk-mq-style per-CPU queues are a Phase 5+ or performance-gated optimization.

### Q2 - Storage drivers

**Decision:** virtio-blk first, write-through correctness path; AHCI and NVMe later in BUILD-5.

BUILD-1 uses virtio-blk because it is required for VM testing and is the smallest path to exercise the block layer. Per OASIS virtio v1.2, if the guest does not negotiate write-back caching, the host operates in write-through mode. That gives correctness without implementing FLUSH in BUILD-1.

AHCI and NVMe remain Phase 3 obligations, but they should not block NeoFS native v1. They move to BUILD-5 with bare-metal validation. Cerebro #2 will survey production driver staging and queue discipline before ADR-021 finalizes the detailed driver plan.

### Q3 - DMA infrastructure

**Decision:** identity-mapped DMA first, small dedicated DMA allocation discipline, IOMMU deferred.

Phase 3 needs DMA-capable buffers for virtio-blk, AHCI, and NVMe. The first implementation should be conservative:

- identity-mapped DMA under the current x86_64 physical map;
- pre-allocated or frame-allocator-backed DMA buffers with explicit lifetime;
- no heap allocation in IRQ context;
- cache-coherence assumptions documented for x86_64;
- aarch64/IOMMU work deferred.

The DMA layer must be shaped so storage drivers do not directly improvise frame ownership. The goal is a small kernel-internal abstraction, not a general driver framework.

### Q4 - VFS layer

**Decision:** minimal VFS in BUILD-1; typed-inode dispatch hooks by BUILD-2; capability-on-file design refined by Cerebro #3.

BUILD-1 VFS needs only enough structure to let syscalls target a mounted in-RAM or trivial filesystem and to prove file handles, path lookup, and read/write flow. BUILD-2 makes the VFS real enough for NeoFS native v1.

Required concepts:

- inode object with stable type discriminant;
- mount table and single global namespace;
- directory lookup;
- file handle table integrated with the capability system;
- rights mask for file capabilities;
- VFS dispatch points for `open`, `read`, `write`, `close`, `stat`, `mmap`, and later `execve` equivalent checks.

Typed-inode policy belongs in VFS dispatch, not in filename parsing. The VFS asks the inode type whether a requested operation is permitted, then intersects that with the file capability rights.

### Q5 - NeoFS native layer

#### Q5.a - First NeoFS storage substrate

**Decision: first NeoFS BUILD is not hash-as-address.**

NeoFS native v1 is:

- single-tier;
- physical-block-addressed;
- copy-on-write;
- Merkle-DAG integrity;
- checksummed redundant superblocks;
- typed inodes;
- synchronous-blocking I/O.

Inodes store both:

- physical block addresses used for lookup in BUILD-2;
- content hashes used for integrity verification and future CAS transition.

This gives the format a CAS-compatible shape without making CAS a correctness dependency. The later CAS BUILD can change the meaning of lookup from "use physical address, verify hash" to "use hash to resolve physical location through a content index."

#### Q5.b - Block size

**Decision:** mkfs-configurable block size; default 4 KiB; support 16 KiB, 64 KiB, and 256 KiB.

4 KiB matches page granularity and Btrfs sectorsize. Larger sizes reduce index pressure for model-storage workloads. Block size is a superblock field, not a compile-time constant.

#### Q5.c - Inode-to-content mapping

**Decision:** Merkle tree of content hashes plus physical addresses in v1.

Small files can use direct extents or inline direct-block metadata. Larger files use an inode-rooted tree:

- leaf entries: physical block address + content hash + length;
- interior nodes: child physical address + child hash;
- inode metadata: file size, type discriminant, permissions/rights seed, tree root pointer/hash.

Directories are also tree objects. Directory entries should carry name, inode reference, and child inode hash where useful for integrity.

The exact fanout and node layout moves to ADR-021 / `docs/specs/neofs-format.md`, but the v2 decision is that the format reserves both physical and hash identity from day one.

#### Q5.d - Crash consistency

**Decision:** CoW + checksummed redundant-superblock rotation + FLUSH barriers.

"Atomic superblock swap" is retired as phrasing because it implies raw sector atomicity. Production filesystems do not get that guarantee on consumer NVMe hardware. The actual primitive is:

1. write new data and metadata blocks to free locations;
2. wait for completions;
3. FLUSH if the device has volatile write-back cache;
4. write all redundant superblock copies with checksum and incremented generation;
5. wait for completions;
6. FLUSH again;
7. mount selects highest generation valid checksum.

NeoFS should use three superblock copies where device size permits, following the Btrfs correction surfaced by Cerebro. Each copy has a checksum over the superblock payload and a generation counter. Torn or stale copies lose at mount.

BUILD-1 virtio-blk can avoid explicit FLUSH by staying in write-through mode. BUILD-2 NeoFS native v1 may still use write-through under virtio-blk. BUILD-3 negotiates FLUSH and buffer-cache write-back.

#### Q5.e - Free space and GC

**Decision:** physical free-space management first; CAS GC later starts with mark-sweep.

In NeoFS native v1, free space is physical. A bitmap or block-group allocator is sufficient. CoW creates orphaned blocks after crashes or aborted transactions; they are reclaimable by walking the current root tree and rebuilding free-space state.

When CAS lands:

- mark-sweep is the authoritative GC;
- refcount may be introduced only as an advisory fast-free hint if mark-sweep pause is measured as a problem;
- full real-time refcounted DDT is Phase 5+ and gated on dedup ratio greater than 5x plus RAM budget.

### Q6 - Foreign FS drivers

**Decision:** FAT32 and ext4 remain read-only, now BUILD-4 scope.

Foreign FS support should not delay NeoFS native v1. It belongs after the native FS substrate is working. Type import policy:

- default: coerce all foreign files to `Regular`;
- optional per-mount default type can come later;
- sidecar `.neofs-types` manifest is a forward marker, not BUILD-4 minimum.

No FAT32/ext4 write support without a separate ADR-level justification.

### Q7 - Syscall surface

**Decision:** minimal file syscalls in BUILD-1; broader POSIX-ish surface in BUILD-4.

BUILD-1:

- `sys_open`;
- `sys_read`;
- `sys_write`;
- `sys_close`;
- minimal `sys_stat` if needed for tests;
- libneo wrappers.

BUILD-2:

- typed-inode enforcement in file-open/mmap/exec pathways;
- NeoFS create/write/read semantics;
- model type refusal of executable mappings.

BUILD-4:

- `mkdir`, `rmdir`, `unlink`, `rename`, `readdir`;
- `mount`, `umount`;
- fuller `stat`;
- path and directory polish.

File handles are capabilities, not ambient fd integers. Rights should include at least READ, WRITE, EXEC, DELETE, and maybe MMAP_EXEC or a semantic equivalent. Cerebro #3 must sharpen the FileCap rights surface against Capsicum and ADR-010.

### Q8 - Boot-time integration

**Decision:** hybrid boot.

Limine modules remain the earliest root. BUILD-1 and many tests can boot with module-backed userspace plus a test disk image for block I/O. Later Phase 3 pivots to a disk root after virtio-blk and NeoFS mount succeed.

This avoids blocking storage work on a full rootfs pivot, while preserving the production direction.

### Q9 - libneo and userspace integration

**Decision:** thin libneo wrappers first.

libneo should expose direct wrappers over file syscalls with capability handles. No userspace buffered I/O layer in BUILD-1. Userspace buffering can be added later if needed.

`neo-init` evolves only when rootfs pivot becomes real. Until then, it remains minimal.

### Q10 - Phase 5 preparation

**Decision:** Phase 3 must carry model-type and content-identity hooks, but does not need to complete model dedup early.

Phase 3 delivers:

- inode type space with reserved future values: `Model`, `Tensor`, `InferenceContext`;
- superblock `type_version` or equivalent;
- content hash fields on file data blocks;
- refusal path for executable mappings of model-typed inodes;
- optional syscall or metadata path for retrieving file content hash later.

Phase 5 consumes or completes CAS depending on whether Phase 3 BUILD-3 shipped.

## 6. BUILD boundary proposal

Phase 3 is now five BUILDs.

### BUILD-1 - Block layer + virtio-blk skeleton + minimal VFS

Goal: kernel boots under QEMU, enumerates virtio-blk, performs block reads/writes, exposes minimal VFS/syscall path, and proves synchronous completion via SC-3C wake composition.

Likely sub-commits:

1. ADR-021 Phase 3 design substrate.
2. Block layer trait, request model, and completion rendezvous.
3. DMA buffer infrastructure and PCI/PCIe enumeration extensions needed by virtio.
4. virtio-blk driver in write-through mode.
5. Minimal VFS and in-RAM filesystem or test filesystem.
6. `sys_open`/`sys_read`/`sys_write`/`sys_close` plus libneo wrappers.
7. Scenarios: block smoke, read/write block round-trip, VFS smoke, syscall read/write round-trip.

Coordinates with ADR-016 A1 if it composes cleanly.

### BUILD-2 - NeoFS native v1

Goal: kernel mounts a NeoFS image from virtio-blk and performs crash-consistent file create/read/write on a single-tier physical-block-addressed CoW filesystem.

Scope:

- `docs/specs/neofs-format.md`;
- mkfs tooling;
- superblock format with revision, generation, checksum, block size, and type-version fields;
- physical block allocator;
- inode/tree format with physical address + content hash fields;
- typed inode discriminant;
- checksummed redundant-superblock rotation;
- CoW transaction path;
- model-typed inode executable-mapping refusal;
- tests for mount, create/read/write, crash prefix simulation where practical, typed-inode policy.

Explicitly not included:

- hash-as-address lookup;
- dedup;
- buffer-cache write-back;
- background GC;
- kthread prerequisite.

### BUILD-3 - NeoFS content-addressing + buffer cache + FLUSH

Goal: add CAS dedup only if measurement justifies it, while also adding the background machinery needed for performant CoW.

Entry gate:

- measured workload shows meaningful dedup value, or
- Phase 5 model-storage planning proves CAS is now critical path, or
- PM explicitly moves BUILD-3 to Phase 5 substrate instead of Phase 3.

Scope if admitted:

- Venti-style bloom filter plus on-disk hash table content index;
- mark-sweep GC;
- optional advisory refcount only if measured pause warrants it;
- virtio-blk FLUSH negotiation;
- buffer-cache write-back;
- kthread primitive if not already present;
- tests for dedup, GC, flush durability, and cache write-back.

### BUILD-4 - Foreign FS read-only + userspace polish

Goal: FAT32 and ext4 read-only mounts work; the POSIX-ish syscall surface is broad enough for normal file-tree traversal and basic userland workflows.

Scope:

- FAT32 read-only;
- ext4 read-only;
- `sys_mount`/`sys_umount`;
- directory enumeration;
- `stat`, `mkdir`, `rmdir`, `unlink`, `rename` as appropriate for native FS;
- foreign-FS type import policy, defaulting to Regular;
- libneo polish and test coverage.

### BUILD-5 - AHCI + NVMe + bare-metal validation

Goal: Phase 3 closes real disk I/O on bare hardware.

Scope:

- SATA AHCI driver;
- NVMe driver;
- real-hardware boot/storage validation;
- bare-metal verification artifact satisfying Bible Article 13;
- closure of carried Phase 2+ bare-metal mandate where applicable.

## 7. Anti-scope

Phase 3 does not include:

- FAT32/ext4 write support;
- network filesystems;
- distributed filesystems;
- USB mass storage;
- eMMC/SD;
- IDE;
- IOMMU-aware DMA;
- page-cache or buffer-cache write-back in BUILD-1;
- async user-visible I/O in BUILD-1/2;
- POSIX strict compatibility;
- POSIX ACLs, quotas, advisory locks, mandatory locks;
- kernel-resident model loader implementation beyond typed-file substrate;
- training, inference scheduler, accelerator scheduling;
- multi-thread-per-process.

Specific correction from v1: content-addressing is no longer anti-scope for Phase 3 overall, but it is anti-scope for the first shippable NeoFS BUILD and measurement-gated for BUILD-3.

## 8. Cerebro dispatch cadence

Phase 3's first Cerebro dispatch has already performed the structural challenge. Therefore Cerebro #2 and #3 are question-level dispatches within an established frame.

### Cerebro #2 - storage drivers

Draft after ADR-021.

Questions:

- virtio-blk write-through BUILD-1 path vs WCE+FLUSH BUILD-3 path;
- DMA buffer discipline in small Rust kernels;
- storage IRQ completion and SC-3C cross-CPU wake composition;
- AHCI command/FIS structure sequencing;
- NVMe submission/completion queue shape under single-thread-per-process;
- driver testing patterns under QEMU and bare metal.

### Cerebro #3 - VFS, typed-inode dispatch, FileCap

Draft after ADR-021. Can parallel with #2.

Questions:

- VFS typed-inode dispatch architecture;
- type policy table location and versioning;
- retype operations modeled on seL4 Untyped-to-Typed discipline;
- Capsicum-style per-fd rights attenuation;
- SELinux-style type-based mmap refusal as closest outcome prior art;
- mount namespace and foreign-FS type import boundaries;
- minimum POSIX-ish syscall set for BUILD-1/2/4.

Do not re-survey typed-inode storage prior art. Cerebro #1 already established that NeoPlutarch's Bible 10.3 design is past production prior-art horizon.

## 9. Forward markers

### Phase 3

- **FM-Phase3-1:** CAS dedup as BUILD-3 feature, measurement-gated; not BUILD-2 structural primitive.
- **FM-Phase3-2:** `VIRTIO_BLK_F_FLUSH` feature negotiation in BUILD-3.
- **FM-Phase3-3:** kthread primitive prerequisite before BUILD-3 PLAN dispatch.
- **FM-Phase3-4:** block size as mkfs/superblock parameter; default 4 KiB, support 16/64/256 KiB.
- **FM-Phase3-5:** incremental mark-sweep GC if full scan pause becomes visible.
- **FM-Phase3-6:** HAMMER-style UNDO/REDO FIFO log if fsync-heavy workloads need bounded recovery/performance.
- **FM-Phase3-7:** type extension versioning in inode/superblock format.
- **FM-Phase3-8:** `.neofs-types` sidecar manifest for foreign-FS type roundtrip.
- **FM-Phase3-9:** privileged retype operation discipline, revoking old-type capabilities and issuing new-type capabilities.
- **FM-Phase3-10:** full ZFS-DDT-style real-time refcount, Phase 5+ only, gated on dedup ratio greater than 5x and RAM budget.

### Phase 4

- **FM-Phase4-1:** network stack adopts the NeoFS SC-3C non-scheduler cross-CPU wake pattern for receive IRQ to user-thread wake. Formalize in an ADR at Phase 4 design entry if reused.

### Phase 5

- **FM-Phase5-1:** model content-addressing per ANSPEC-001 section 8.2 depends on Phase 3 BUILD-3 if shipped; otherwise Phase 5 must include CAS substrate explicitly.

## 10. Citation corrections carried from Cerebro #1

THINK v2 accepts and carries these corrections:

1. Venti paper authorship is Quinlan and Dorward, not Quinlan and Mullender.
2. Btrfs has three superblock copies at fixed offsets, written simultaneously, not two alternated copies.
3. "Atomic superblock swap" means checksum + redundant copies + flush barriers, not raw sector atomicity.
4. ZFS dedup has real operational cost: DDT memory, CPU on reads/writes, and a 2024 maintainer-adjacent "do not use it by default" verdict. Refcounted CAS is not a lightweight default.
5. 9P Qid type does not prevent execution. SELinux is closest in outcome for type-based mmap refusal; Capsicum is closest in per-fd mechanism; 9P is closest in typed-file plumbing.

ADR-021 should cite primary sources from the Cerebro artifact rather than repeating unverified v1 phrasing.

## 11. PM dispositions

- Phase 3 THINK v1 is superseded and classified as pre-structural-challenge exploration.
- Phase 3 THINK v2 is the active design-synthesis artifact.
- ADR-021 is the next PM artifact and should become the canonical Phase 3 substrate ADR.
- Phase 3 uses a five-BUILD plan.
- BUILD-3 is measurement-gated and may move to Phase 5 if model dedup is the first real driver.
- No Bible amendment is filed now. Article 10 is broad enough to permit the sequencing correction.
- Bible Article 14 amendment remains a Phase 4-close candidate if structural-challenge-first cadence proves out.
- No PL ratification is required for this THINK v2 under the corrected role model; PL is courier.

## 12. Next operational steps

1. PM authors ADR-021 Phase 3 design substrate.
2. PM drafts Cerebro #2 storage-driver dispatch after ADR-021.
3. PM drafts Cerebro #3 VFS/typed-inode/FileCap dispatch after ADR-021; can ferry in parallel with #2.
4. PM consumes #2/#3 returns and updates ADR/PLAN details if needed.
5. PM drafts Phase 3 BUILD-1 PLAN dispatch for Jarvis.
6. Before BUILD-3, PM audits kthread primitive availability and files the prerequisite decision.

Parallel non-blocking PL items remain unchanged:

- VMWare Workstation hypervisor-VM verification.
- rustfmt tooling-gap resolution.

## 13. Closure

Phase 3's core substrate is now corrected. The first NeoFS implementation is not an eager CAS filesystem. It is a simpler, stronger, single-tier physical-block-addressed CoW filesystem that reserves content hashes for integrity and forward compatibility. CAS is demoted to a measured optimization BUILD, not because Article 10 is being softened, but because the prior art says correctness and dedup must not be entangled too early.

This is the right Phase 3 shape: block layer and virtio-blk first; NeoFS native v1 second; CAS/buffer-cache/FLUSH only if earned; foreign FS after native FS; AHCI/NVMe and bare-metal validation to close the phase.

---

**End of Phase 3 THINK v2.**

*Post-structural-challenge synthesis. Supersedes THINK v1. Ratifies Cerebro #1 Q5 reframe: single-tier physical-block-addressed CoW first; content-addressing deferred to measurement-gated BUILD-3; Phase 3 BUILD count 4 to 5; kthread audit before BUILD-3; NeoFS is first non-scheduler SC-3C cross-CPU wake client; Cerebro #2/#3 are question-level dispatches inside the established Phase 3 frame.*
