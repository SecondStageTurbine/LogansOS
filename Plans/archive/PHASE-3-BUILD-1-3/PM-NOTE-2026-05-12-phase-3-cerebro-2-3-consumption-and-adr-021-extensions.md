# PM-NOTE: Phase 3 Cerebro #2/#3 consumption - storage and VFS sequencing confirmed; ADR-021 extensions accepted

| | |
|---|---|
| **Date** | 2026-05-12 |
| **Author** | PM (NeoPlutarch Phase 3 cadence) |
| **Type** | PM consumption note; records ratification of Cerebro #2/#3 question-level returns |
| **Consumes** | `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md`; `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md` |
| **Updates** | `docs/ADR-021-phase-3-design-substrate.md` |
| **Status** | Ratified. No structural reframe. ADR-021 sequencing confirmed with extensions. |

---

## 1. Headline

Cerebro #2 and #3 confirm ADR-021's Phase 3 sequencing. No structural challenge to the five-BUILD split landed.

The returns are still materially useful: both surface acceptance-gate details that should be locked into ADR-021 before Jarvis BUILD-1 planning. Storage-driver work contributes three canonical extensions; VFS/FileCap work contributes six accepted extensions once the full artifact is reconciled with the closure summary.

Time-budget actual for both dispatches: ~2.5 hours each.

## 2. Storage-driver return - ratified extensions

ADR-021 storage sequencing is confirmed: virtio-blk first, AHCI/NVMe in BUILD-5, identity-DMA, and SC-3C wake composition are all sound.

PM accepts these extensions:

1. **BUILD-1 virtio-blk must assert `VIRTIO_BLK_F_FLUSH` is offered.** The write-through durability path relies on OASIS virtio Case (1): device offers F_FLUSH, driver does not negotiate F_FLUSH/CONFIG_WCE. If F_FLUSH is not offered, durability downgrades from MUST to MAY. BUILD-1 driver refuses to attach in that case.
2. **IoRendezvous stores `Weak<Thread>`, not `Arc<Thread>`.** Holding `Arc<Thread>` fights SC-4 exit cleanup. IRQ completion upgrades the weak reference; failure means the issuer is gone and wake is skipped.
3. **BUILD-1 needs an aarch64 CI lane before closure.** x86 TSO hides descriptor-ring barrier bugs. aarch64 exposes the step-3 compiler-fence vs step-5 real-fence distinction.

PM also records implementation risks for downstream dispatches:

- `VIRTIO_BLK_F_CONFIG_WCE` bit 11 is distinct from `VIRTIO_BLK_F_FLUSH` bit 9.
- AHCI top risks: port-restart ordering, `PxSERR` clear, and IS-latched-before-process.
- NVMe top risks: completion-queue phase bit, doorbell stride `4 << DSTRD`, and `CC.EN` handshake race.

## 3. VFS/FileCap return - ratified extensions

ADR-021 VFS/FileCap sequencing is confirmed: minimal VFS in BUILD-1, typed-inode enforcement in BUILD-2, foreign-FS/userspace polish in BUILD-4.

PM accepts these extensions:

1. **Foreign-FS mounts default `noexec` in BUILD-4.** Coerce-to-Regular alone would make FAT32/ext4 ELFs executable by typed-inode policy. Mount-level `noexec` is the defense layer; operator must opt into `exec`.
2. **Inode generation counter lands in BUILD-2.** FileCaps store the inode generation at issue time. This makes future retype and stale-cap invalidation possible without an on-disk migration.
3. **FileRights is `u32`, not `u16`.** The rights surface needs room for `MMAP_MODEL`, `GRANT`, `DERIVE`, reserved `RETYPE`, and `MOUNT_HERE`/mount rights.
4. **`sys_fsync` is per-file intent-log flush, not global CoW transaction flush.** BUILD-2 dispatch must not accidentally specify global transaction flush for fsync.
5. **`TypeForbidden` and `InsufficientRights` are distinct errors.** Audit clarity and tests require differentiating "inode type forbids this operation" from "capability lacks right."
6. **FileCap GRANT/DERIVE/REVOKE follows the seL4 CDT model.** Artifact section 12 listed this as an ADR extension. PM accepts it: FileCap derivation joins ADR-010's capability derivation tree; retype invalidates descendants through generation/stale semantics.

Additional ratified findings:

- `..` traversal clamps at capability root rather than rejecting escape attempts.
- ext4 xattrs are distrusted in BUILD-4; Scenario 10 becomes a permanent regression test.
- `sys_readdir` is cursor-based, not Vec-returning.
- Hard links are a permanent skip candidate and should be considered for Bible-level prohibition at Phase 3 close.

## 4. ADR-021 amendment directives

Patch ADR-021 with:

- Q2: `IoRendezvous.waiting_thread: Option<Weak<Thread>>`, Arc-managed `DmaBuffer`, atomic park-and-release requirement.
- Q3: DMA API carries `Arc<DmaBuffer>` and accepts slices for future scatter-gather.
- Q4/BUILD-1: virtio-blk probe asserts F_FLUSH offered and refuses attachment otherwise.
- BUILD-1 acceptance gates: add aarch64 CI lane before BUILD-1 close.
- Q5: FileRights `u32`, type-first rights-second, distinct error codes.
- Q7/Q8/BUILD-2: inode generation counter and per-file intent-log fsync.
- Q9/BUILD-4: foreign-FS default `noexec`, ext4 xattr distrust, Scenario 10 regression test.
- Substrate review: retype stays BUILD-5/Phase 5 with BUILD-2 forward-compat invariants.

## 5. Next cadence

After ADR-021 amendment:

1. PM drafts Phase 3 BUILD-1 PLAN dispatch for Jarvis.
2. BUILD-1 dispatch must include storage and VFS acceptance gates from this PM-NOTE.
3. kthread audit remains deferred until pre-BUILD-3.

No new Cerebro dispatch is needed before BUILD-1 PLAN.

---

**End of PM-NOTE.**

*Cerebro #2/#3 confirm ADR-021. Storage extensions: F_FLUSH-offered assertion, Weak<Thread> rendezvous, aarch64 CI lane. VFS extensions: foreign-FS noexec default, inode generation, FileRights u32, per-file fsync intent log, distinct TypeForbidden/InsufficientRights, FileCap CDT integration. Next: amend ADR-021, then draft Phase 3 BUILD-1 Jarvis dispatch.*
