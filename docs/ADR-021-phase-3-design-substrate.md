# ADR-021: Phase 3 NeoFS, block layer, and real disk I/O substrate

| | |
|---|---|
| **Status** | Accepted (Phase 3 THINK v2 substrate; BUILD-1 through BUILD-5 plan) |
| **Date** | 2026-05-12 |
| **Authority** | PM |
| **Supersedes** | None |
| **Superseded by** | - |
| **Related** | Bible Articles 9.9 (storage device support), 10 (filesystem and storage), 12.1 (capabilities), 13 (phase ledger), 14 (algorithm); ANSPEC-001 sections 4.1, 4.3, 8.2, 10.3, 10.5; ADR-010 (capability system foundation); ADR-012 (buddy + slab allocator foundation); ADR-013 (Phase 2 SMP substrate); ADR-014 (observability deferred until NeoFS exists); ADR-016 (IRQ-safe heap + scheduler decoupling); `Plans/PHASE-3-THINK-V2-2026-05-11.md` |

---

## Context

Phase 3 closes Bible Article 13's milestone: **NeoFS, block layer, real disk I/O**. The kernel currently loads userspace from Limine modules and has no persistent-storage stack: no block layer, no block drivers, no VFS, no disk-backed native filesystem, and no rootfs pivot.

This ADR is the canonical Phase 3 design substrate. It follows the forward-looking Q-shape used by ADR-013: enumerate the design questions, ratify the Phase-level choices, and define the BUILD boundaries that subsequent dispatches implement.

Phase 3 THINK v1 initially framed NeoFS around eager content-addressing at the block layer. Cerebro #1 reframed that substrate. The ratified correction is:

- first shippable NeoFS is a single-tier physical-block-addressed CoW filesystem with Merkle-DAG integrity;
- content-addressing as hash-as-address is a measurement-gated later BUILD, not the first correctness primitive;
- crash consistency comes from CoW ordering, redundant checksummed superblocks, generation counters, and FLUSH discipline;
- typed inode discriminants are a security requirement and land with native NeoFS;
- mark-sweep is the first CAS GC strategy when CAS lands;
- full real-time refcounted dedup is Phase 5+ unless measured workload proves otherwise.

This ADR does not soften Bible Article 10.2. NeoFS still aims at content-addressing and model dedup by content hash. This ADR resolves sequencing: correctness first, optimization when measured.

### Bible alignment

- Bible Article 9.9 requires virtio-blk, SATA AHCI, and NVMe from Phase 3.
- Bible Article 10.1 names NeoFS as Phase 3.
- Bible Article 10.2 binds NeoFS to a POSIX-ish path namespace and content-addressed block-layer direction.
- Bible Article 10.3 requires typed inodes, not filename extensions, with model-file executable-mapping refusal.
- Bible Article 10.4 requires read-only FAT32 and ext4 interop.
- Bible Article 13 requires real boot verification on QEMU and at least one bare-metal target before phase closure.

### Scope

Within scope:

- Block layer abstraction and synchronous completion model.
- Initial DMA-buffer discipline.
- virtio-blk write-through path for BUILD-1.
- VFS skeleton and capability-shaped file handles.
- NeoFS native v1 single-tier physical-addressed CoW format.
- Typed inode discriminant and VFS enforcement hooks.
- CAS dedup as measurement-gated BUILD-3.
- FAT32/ext4 read-only support.
- AHCI/NVMe and bare-metal validation.

Out of scope:

- FAT32/ext4 write support.
- USB mass storage, eMMC/SD, IDE.
- IOMMU-aware DMA.
- POSIX-strict compatibility.
- Network filesystems and distributed filesystems.
- Async user-visible I/O.
- Multi-thread-per-process.
- Kernel model loader implementation beyond typed-file substrate.

---

## Q1: Block layer abstraction

### The question

What is the kernel-internal abstraction for a persistent block device, and what completion model does the filesystem consume?

### Choice: synchronous-blocking API over async hardware completion

The block device API is synchronous from the filesystem caller's perspective. The issuing thread blocks until the hardware request completes. Internally, drivers still issue DMA requests and complete through interrupts.

The first `BlockDevice` shape must expose:

- logical and physical block size;
- device capacity;
- block-aligned read/write operations;
- request completion through a rendezvous object;
- optional FLUSH capability;
- stable device identity for mount/root selection.

Per-device single queues are sufficient for Phase 3. blk-mq-style per-CPU queues are deferred.

### Rationale

NeoPlutarch has no general async runtime, and Phase 3 does not need one. Single-thread-per-process remains the kernel invariant. Synchronous file I/O is the classical Unix model and composes cleanly with syscall blocking.

The completion path is still fully SMP-aware: storage IRQ on CPU B wakes a thread blocked on CPU A using the SC-3C cross-CPU IPI wake primitive.

### Alternatives rejected

- **User-visible async I/O:** premature; no runtime substrate or workload need.
- **Global request queue:** simple but unnecessary contention.
- **Per-CPU queues from day one:** performance feature before there is a performance problem.

---

## Q2: I/O completion and wakeup discipline

### The question

How does a storage interrupt wake the thread that issued the request without reintroducing lost wakeups or a parallel scheduler mechanism?

### Choice: SC-3C IPI cross-CPU wake, with a SwitchMutex-protected rendezvous

Each blocking I/O owns a small rendezvous slot:

```rust
struct IoRendezvous {
    done: AtomicBool,
    waiting_thread: Option<ThreadWakeToken>,
    dma_buffers: Vec<Arc<DmaBuffer>>,
    result: IoResult,
}
```

The slot is protected by the existing IRQ-safe lock discipline. The issuing thread records itself, checks `done`, and blocks. The IRQ handler records completion, sets `done`, and wakes the waiting thread. If the waiting thread belongs to a different CPU, the handler uses the SC-3C cross-CPU IPI wake path.

The issuing side must park through an atomic park-and-release primitive: it checks `done`, installs `waiting_thread`, and parks while releasing the SwitchMutex as one scheduler operation. Splitting those steps reopens the lost-wakeup race. `waiting_thread` is a weak scheduler wake token, not an owning thread reference, so an abandoned I/O does not keep a thread alive after process exit; IRQ completion resolves the token and skips wake if the issuer is gone. `dma_buffers` are Arc-managed so storage completion, not process lifetime, owns the final buffer reference.

Implementation refinement from BUILD-1 1B: NeoPlutarch's current scheduler owns threads as `Box<Thread>` in current/ready/zombie queues and existing IPC blocked states. BUILD-1 must not migrate the entire scheduler to `Arc<Thread>` solely to satisfy the storage rendezvous sketch. The concrete storage rendezvous token is a generation-checked `ThreadWakeToken` backed by a scheduler-owned parked-thread table. This is the local equivalent of `Weak<Thread>`: it grants non-owning wake authority, fails after process exit removes or zombifies the parked thread, and prevents stale wake after slot reuse.

### Rationale

This makes NeoFS the first non-scheduler client of the SC-3C IPI cross-CPU wake primitive. That is intentional. Storage IRQ completion has the same shape as scheduler cross-CPU wake: a producer on CPU B must make a thread runnable on CPU A.

The lost-wakeup invariant is the same one already ratified for Phase 2: the condition and waiter are observed under the same lock discipline, and `done` catches early completion.

### Consequences

- No new wake primitive is introduced for storage.
- Phase 4 network receive wakeups should reuse this pattern.
- A Phase 4 ADR should formalize "non-scheduler cross-CPU wake client" if the network stack adopts it.
- BUILD-1 must confirm the atomic park-and-release primitive exists before implementing block I/O sleep.

---

## Q3: DMA infrastructure

### The question

What DMA substrate is sufficient for Phase 3 block drivers?

### Choice: identity-mapped DMA first

Phase 3 starts with identity-mapped DMA buffers backed by explicit frame ownership. The DMA layer must provide:

- allocation of DMA-capable buffers;
- stable physical addresses;
- explicit lifetime and ownership;
- Arc-managed buffer ownership across in-flight I/O;
- submission over `&[Arc<DmaBuffer>]` so virtio-blk can use one buffer now and AHCI/NVMe can use scatter-gather later;
- no allocation in interrupt context;
- x86_64 cache-coherence assumptions documented at the call sites.

IOMMU-aware DMA is deferred to Phase 5+.

### Rationale

virtio-blk, AHCI, and NVMe all need DMA. They do not need a full IOMMU abstraction to land the Phase 3 correctness substrate. A small kernel-internal DMA layer prevents each driver from inventing its own unsafe ownership story.

### Alternatives rejected

- **Driver-local ad hoc frame handling:** repeats unsafe hardware-boundary code and makes auditing harder.
- **IOMMU first:** too much substrate for Phase 3; no current hardware mandate.

---

## Q4: Storage driver sequencing

### The question

Which block drivers land in which BUILD, and how does Phase 3 satisfy Bible Article 9.9?

### Choice: virtio-blk first; AHCI/NVMe in BUILD-5

BUILD-1 implements virtio-blk in write-through mode. AHCI and NVMe remain Phase 3 requirements but move to BUILD-5 with bare-metal validation.

BUILD-1 attaches only to modern virtio-blk devices that offer `VIRTIO_BLK_F_FLUSH`. The driver does not negotiate `VIRTIO_BLK_F_FLUSH` or `VIRTIO_BLK_F_CONFIG_WCE` in BUILD-1. This intentionally selects the virtio Case-(1) write-through durability path: F_FLUSH offered, not negotiated, completed writes are stable. If F_FLUSH is not offered, the driver refuses to attach.

### Rationale

virtio-blk is the fastest path to QEMU coverage and exercises the block layer without hardware variability. Per OASIS virtio v1.2, a guest that does not negotiate write-back caching gets write-through behavior. That gives a correctness path before FLUSH support lands.

AHCI and NVMe are larger hardware-driver efforts. They should not block NeoFS native v1.

### Forward work

Cerebro #2 surveys:

- virtio-blk write-through to WCE+FLUSH staging;
- safe Rust DMA driver patterns;
- AHCI command/FIS structure;
- NVMe submission/completion queue shape;
- storage IRQ completion discipline under SC-3C wake.

Implementation risk markers from Cerebro #2:

- `VIRTIO_BLK_F_CONFIG_WCE` is bit 11 and is distinct from `VIRTIO_BLK_F_FLUSH` bit 9.
- AHCI top risks: port-restart ordering, `PxSERR` clear, and IS-latched-before-process.
- NVMe top risks: completion-queue phase bit, doorbell stride `4 << DSTRD`, and `CC.EN` handshake race.

---

## Q5: VFS and file capabilities

### The question

What minimum VFS substrate does Phase 3 need before native NeoFS, and how does it compose with the capability system?

### Choice: minimal VFS in BUILD-1; typed dispatch and FileCap refinement in BUILD-2+

BUILD-1 introduces:

- mount table;
- path lookup sufficient for smoke tests;
- inode object abstraction;
- file object / open-handle abstraction;
- file handles as capabilities;
- `sys_open`, `sys_read`, `sys_write`, `sys_close`;
- thin libneo wrappers.

BUILD-2 adds typed-inode enforcement hooks. BUILD-4 broadens the POSIX-ish syscall surface.

File handles are capability handles, not ambient authority. Initial semantic rights include at least READ, WRITE, EXEC, DELETE, and an executable-mapping right or equivalent.

The FileRights word is `u32`, not `u16`. BUILD-2 reserves room for `MMAP_MODEL`, `GRANT`, `DERIVE`, `RETYPE`, and `MOUNT_HERE`/mount authority without a rights-format migration. VFS dispatch is type-first and rights-second: inode type determines whether an operation is in the object's operation namespace; FileRights then attenuate that permission. Failures distinguish `TypeForbidden` from `InsufficientRights`.

### Rationale

ADR-010 already establishes the capability substrate. Files are kernel objects and must join that discipline. The VFS should not smuggle Unix uid/root semantics back into the kernel.

### Deferred detail

Cerebro #3 refines:

- FileCap semantic rights;
- Capsicum-style per-fd attenuation;
- VFS typed-inode dispatch points;
- mount namespace boundaries;
- foreign-FS type import.

FileCap derivation joins the ADR-010 capability derivation tree. GRANT and DERIVE create descendants; REVOKE invalidates descendants. Future retype uses this CDT discipline plus inode generation checks rather than trying to update live FileCaps in place.

---

## Q6: NeoFS native v1 substrate

### The question

What is the first shippable NeoFS filesystem?

### Choice: single-tier physical-block-addressed CoW with Merkle-DAG integrity

NeoFS native v1 is not hash-as-address. It is:

- single-tier;
- physical-block-addressed;
- copy-on-write;
- Merkle-DAG-integrity checked;
- typed at the inode level;
- block-size-configurable at mkfs time;
- synchronous-blocking;
- rooted by redundant checksummed superblocks.

Inodes store physical block addresses for lookup and content hashes for verification/future CAS transition.

### Rationale

Cerebro #1 found convergent production evidence:

- Plan 9 Fossil+Venti's two-tier CAS architecture was retired by its own community in favor of simpler single-tier successors.
- ZFS dedup remains operationally expensive even after 2024 improvements; it is not a lightweight default.
- IPFS uses mark-sweep GC to avoid the real-time refcount/DDT cost trap.

The lesson is to separate correctness from dedup optimization. CoW tree structure gives crash safety, snapshots, and no-fsck behavior without hash-as-address lookup.

### Format implications

`docs/specs/neofs-format.md` must include:

- superblock revision;
- generation counter;
- checksum;
- block size field;
- type-version field;
- inode generation counter, initialized to 1;
- reserved inode metadata for future retype audit fields;
- root inode pointer/hash;
- physical address plus content hash fields in inode tree entries;
- reserved type IDs for `Model`, `Tensor`, and `InferenceContext`.

Every FileCap issued for a NeoFS inode stores the inode generation. VFS entry points reject stale caps when the stored generation no longer matches the inode's current generation. BUILD-2 does not implement retype, but it lays this invariant now so BUILD-5/Phase 5 can add retype without an on-disk migration.

---

## Q7: NeoFS crash consistency

### The question

What durable commit protocol does NeoFS use?

### Choice: CoW plus checksummed redundant-superblock rotation plus FLUSH barriers

NeoFS does not rely on raw sector atomicity. The commit protocol is:

1. Write new data and metadata blocks to free physical locations.
2. Wait for write completions.
3. FLUSH if the device has volatile write-back caching.
4. Write redundant superblock copies with incremented generation and checksum.
5. Wait for superblock write completions.
6. FLUSH again.
7. On mount, choose the highest-generation superblock with a valid checksum.

NeoFS should use three superblock copies where device size permits, following Btrfs prior art.

### Rationale

Consumer NVMe does not provide the multi-LBA atomicity that "atomic superblock swap" can accidentally imply. Production CoW filesystems obtain atomicity from redundancy, checksums, generation counters, and flush ordering.

BUILD-1 can avoid explicit FLUSH by using virtio-blk write-through mode. BUILD-3 adds FLUSH negotiation and buffer-cache write-back.

`sys_fsync` is per-file intent-log flush, not global CoW transaction flush. BUILD-2 must not specify "flush the entire filesystem transaction" as fsync semantics; that would make fsync-heavy workloads pathological. Detailed log format belongs in `docs/specs/neofs-format.md`, but the semantic boundary is ratified here.

### Alternatives rejected

- **Journaling first:** heavier than necessary and not aligned with the CoW/Merkle tree design.
- **No crash consistency / fsck first:** below project correctness bar.
- **Assume 4 KiB hardware atomic writes:** not a valid consumer-hardware assumption.

---

## Q8: Content-addressing and GC

### The question

When does NeoFS become hash-as-address, and what GC strategy is used?

### Choice: CAS is BUILD-3 and measurement-gated; mark-sweep first

BUILD-3 may add:

- content-hash index;
- bloom-filter front end;
- on-disk hash table;
- mark-sweep GC;
- optional advisory refcount only if measured pause warrants it.

Entry gate:

- measured dedup ratio justifies complexity, or
- Phase 5 model-storage planning makes CAS critical path, or
- PM explicitly moves the CAS work into Phase 5 substrate.

### Rationale

Hash-as-address adds an index, GC, crash-recovery proof, memory pressure, and operational complexity. It should not contaminate the first correctness BUILD.

Mark-sweep is crash-safe by construction because the live set is derived from the current inode forest. Refcounted DDT-style dedup remains a Phase 5+ forward marker gated on dedup ratio greater than 5x and RAM budget.

### Consequences

- Article 10.2 remains open until BUILD-3 or Phase 5 CAS closure.
- BUILD-2 reserves the format fields needed for CAS.
- Phase 5 cannot assume model dedup is available unless BUILD-3 shipped.

---

## Q9: Typed inodes and executable-mapping refusal

### The question

How does Phase 3 implement Bible Article 10.3?

### Choice: inode-level type discriminant enforced by VFS dispatch

Each NeoFS inode has a type discriminant. Initial types include:

- `Regular`;
- `Directory`;
- `Symlink` if needed;
- `Model` reserved or implemented in BUILD-2 for `.neom` policy tests;
- future-reserved `Tensor` and `InferenceContext`.

VFS `mmap` / executable mapping / future exec-equivalent dispatch consults the inode type before authorizing the operation. A `Model` inode cannot be mapped executable regardless of filename.

Foreign filesystems cannot import `Model` type in BUILD-4. FAT32 non-directories and ext4 regular files coerce to `Regular`; ext4 xattrs are ignored because they are not cryptographically anchored. All foreign-FS mounts default `noexec`, so Regular-typed ELFs on removable media are still non-executable unless the operator explicitly opts into `exec`.

### Rationale

Cerebro #1 found no exact production prior art. The closest analogs are:

- SELinux for type-based executable mapping refusal;
- Capsicum for per-fd rights attenuation;
- Plan 9 9P for type-in-file plumbing, though 9P Qid type does not prevent execution.

NeoPlutarch's synthesis is novel but tractable: the type is part of the inode ground truth, not an advisory xattr or filename convention.

### Forward markers

- Retype operations must revoke old-type capabilities and issue new-type capabilities.
- Foreign-FS type import defaults to `Regular`.
- Foreign-FS mounts default `noexec`; xattr-distrust is a permanent regression-test posture.
- `.neofs-types` sidecar manifests are deferred.
- Hard links are a permanent skip candidate because they conflict with typed-inode authority and CoW content identity.

---

## Q10: Boot and userspace integration

### The question

How does Phase 3 move from Limine-module userspace to disk-backed userspace?

### Choice: hybrid boot

Limine modules remain the earliest root. BUILD-1 can run existing userspace from modules while attaching a test disk image for block/VFS tests. Later Phase 3 pivots to a NeoFS disk root once virtio-blk and NeoFS mounting are stable.

libneo starts as thin wrappers over file syscalls. `neo-init` evolves only when rootfs pivot becomes real.

### Rationale

This preserves existing tests and keeps storage bring-up focused. Direct disk root is the production direction, but it should not be the first dependency.

---

## BUILD boundaries

### BUILD-1: Block layer + virtio-blk skeleton + minimal VFS

Goal: QEMU boots, enumerates virtio-blk, performs block reads/writes, exposes initial file syscalls, and validates SC-3C wake composition from storage completion.

Includes:

- this ADR;
- block layer trait and request model;
- DMA buffer infrastructure;
- PCI/PCIe enumeration extensions needed by virtio;
- virtio-blk write-through driver that asserts F_FLUSH is offered and refuses attachment otherwise;
- minimal VFS and test filesystem;
- `sys_open`/`sys_read`/`sys_write`/`sys_close`;
- libneo wrappers;
- block/VFS smoke tests.

Acceptance gates:

- feature-negotiation log records F_FLUSH offered and not negotiated;
- abandoned-I/O test proves the weak scheduler wake-token rendezvous and Arc-managed DMA buffers do not leak or keep an exited thread alive;
- an aarch64 QEMU CI lane passes before BUILD-1 closes, to expose descriptor-ring barrier bugs masked by x86 TSO.

### BUILD-2: NeoFS native v1

Goal: kernel mounts a NeoFS image from virtio-blk and performs crash-consistent native file create/read/write.

Includes:

- `docs/specs/neofs-format.md`;
- mkfs tooling;
- physical block allocator;
- inode/tree layout with physical address plus content hash fields;
- inode generation counter and FileCap generation check;
- typed inode discriminant;
- CoW transaction path;
- per-file `sys_fsync` intent-log semantics;
- redundant superblock rotation;
- typed-inode executable-mapping refusal tests.

Excludes CAS, dedup, buffer-cache write-back, background GC, and kthreads.

### BUILD-3: NeoFS content-addressing + buffer cache + FLUSH

Measurement-gated.

Includes only if admitted:

- content-hash index;
- mark-sweep GC;
- virtio-blk FLUSH negotiation;
- buffer-cache write-back;
- kthread primitive if not already present;
- dedup/GC/flush durability tests.

### BUILD-4: Foreign FS read-only + userspace polish

Includes:

- FAT32 read-only;
- ext4 read-only;
- default `noexec` on all foreign-FS mounts, with explicit operator opt-in for `exec`;
- ext4 xattrs ignored for type import;
- `sys_mount`/`sys_umount`;
- directory enumeration;
- cursor-based `sys_readdir`;
- broader POSIX-ish syscall surface;
- foreign-FS type import defaulting to `Regular`.

### BUILD-5: AHCI + NVMe + bare-metal validation

Includes:

- SATA AHCI driver;
- NVMe driver;
- real-hardware boot/storage verification;
- Phase 3 bare-metal closure artifact.

---

## Substrate review item: kthreads

Before BUILD-3, PM audits whether the scheduler exposes a kernel-internal kthread spawn primitive to non-scheduler subsystems.

If not present, BUILD-3 prerequisite is a kernel-side kthread primitive, likely in `kernel/src/sched/kthread.rs`, with subsystem-owned lifetime handles and unmount/reap discipline. This is not exposed as a userspace syscall.

Network stack Phase 4 likely consumes the same primitive for timers and queue draining.

Retype remains BUILD-5/Phase 5 scope. BUILD-2 lays the forward-compat invariants only: inode generation, FileCap generation, versioned discriminant, reserved retype audit bytes, and reserved `RETYPE` FileRights bit.

---

## Citation corrections incorporated

ADR-021 incorporates the five Cerebro #1 corrections:

1. Venti is Quinlan and Dorward, not Quinlan and Mullender.
2. Btrfs has three superblock copies, written simultaneously.
3. Superblock atomicity means checksum + redundancy + flush barriers, not raw sector atomicity.
4. ZFS dedup operational cost makes refcounted CAS an optimization with a high gate.
5. 9P Qid type does not prevent execution; SELinux and Capsicum are closer analogs for enforcement.

Primary-source URLs are recorded in `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`; implementation docs should cite those sources when writing detailed format or driver specs.

---

## Anti-scope

This ADR does not:

- design the exact NeoFS on-disk binary layout;
- ratify a FileCap rights byte;
- ratify AHCI/NVMe register-level implementation;
- ratify a page cache;
- ratify a kthread API;
- ratify a retype API;
- amend the Bible;
- close Article 10.2 CAS obligations by itself.

Those are subsequent ADR/spec/PLAN artifacts.

---

## Ratification decisions

- **Q1:** synchronous-blocking block API over async hardware completion.
- **Q2:** storage completion uses SC-3C cross-CPU wake; no parallel wake primitive.
- **Q3:** identity-mapped DMA first; IOMMU deferred.
- **Q4:** virtio-blk write-through first; AHCI/NVMe in BUILD-5.
- **Q5:** VFS starts minimal; file handles are capabilities; FileRights is `u32`.
- **Q6:** NeoFS native v1 is single-tier physical-block-addressed CoW, not CAS.
- **Q7:** crash consistency uses CoW + redundant checksummed superblocks + FLUSH discipline; fsync is per-file intent-log flush.
- **Q8:** CAS dedup is measurement-gated BUILD-3; mark-sweep first.
- **Q9:** typed inode discriminant is enforced by VFS dispatch; foreign-FS mounts default `noexec`.
- **Q10:** hybrid Limine-module early boot, later disk-root pivot.
- **Storage acceptance:** BUILD-1 virtio-blk requires F_FLUSH offered at probe and an aarch64 CI lane before closure.
- **VFS acceptance:** type and rights errors are distinguishable as `TypeForbidden` and `InsufficientRights`; ext4 xattr-distrust gets a permanent regression test.
- **BUILD split:** Phase 3 uses five BUILDs.
- **Article 10.2:** remains constitutional destination; sequencing correction does not require Bible amendment now.

---

## Cross-references

- `Plans/PHASE-3-THINK-V2-2026-05-11.md` - immediate design-synthesis source for this ADR.
- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` - prior-art source and citation correction source.
- `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` - PM ratification of Q5 reframe and BUILD split.
- `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md` - cadence rule that classifies THINK v1/v2.
- ADR-010 - capability handles and two-tier rights model.
- ADR-012 - allocator substrate and Phase 3 heap-stress forward markers.
- ADR-013 - SMP, per-CPU scheduler, SC-3C wake substrate.
- ADR-014 - observability deferred until NeoFS substrate exists.
- ADR-016 - IRQ-safe heap and scheduler decoupling, coordinated with BUILD-1.
- Bible Articles 9.9, 10, 12.1, 13, 14.
- ANSPEC-001 sections 4.1, 4.3, 8.2, 10.3, 10.5.

---

**End of ADR.**
