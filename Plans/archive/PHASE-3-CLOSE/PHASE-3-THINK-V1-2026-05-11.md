# Phase 3 THINK v1 — NeoFS, block layer, real disk I/O (exploration)

**Date:** 2026-05-11
**Author:** PM
**Cadence:** OBSERVE/THINK exploration per Bible Article 14. **v1 is exploration-shape**, not the final THINK — design surface is wider than SC-3C/SC-4 (which were tightly scoped to specific ADR-013 Q-items). Phase 3 covers a clean-slate filesystem subsystem plus storage device drivers from scratch. Phase 3 THINK v2 follows Cerebro deep-research returns on at least two dispatch candidates flagged in §7.
**Status:** Draft v1. PM-tier scope per the operational posture; PL ratification needed only for (a) BUILD boundary if the proposal is significantly different from PHASE-2 cadence, (b) Cerebro dispatch authorization, (c) Bible amendment if any clause needs sharpening for Phase 3 implementation.

---

## 1. Mandate

Phase 3 closes Bible Article 13's third milestone: *"NeoFS, block layer, real disk I/O."* It builds three things that don't currently exist:

1. **Block layer.** Abstraction over byte-addressable persistent storage devices, with driver class for virtio-blk (VM testing per Bible 9.9), SATA AHCI (bare-metal), and NVMe over PCIe (modern hardware).
2. **VFS layer.** Abstraction between filesystem drivers and the block layer; typed inodes per Bible 10.3; mountpoint hierarchy; POSIX-ish path namespace per Bible 10.2.
3. **NeoFS.** Native filesystem, content-addressed at the block layer, deduplication by content hash for models per Bible 10.2 + ANSPEC §8.2.

Plus foreign-FS read-only support (FAT32, ext4) for interop per Bible 10.4.

## 2. Why Phase 3 next (corrects PM Phase 5 lean)

PM's Q9+ exploration §6.4 originally leaned to "Phase 5 next." PL pushed back; PM Phase 5 lean was project-aesthetics-driven rather than substrate-driven. The corrected reasoning:

- **Bible Article 10.2** specifies NeoFS as content-addressed at the block layer: *"loading the same 70B model twice from disk costs one copy in cache."* This is Phase 5's model deduplication infrastructure (ANSPEC §8.2 BLAKE3 content-hash dedup keyed in a per-device dedup table).
- **Bible Article 10.3** specifies typed FS extensions: *"A .neom file is tagged as a model; the kernel can refuse to mmap it as executable code."* This is Phase 5's defense against the model-as-confusion-attack-vector per ANSPEC §10.5.
- **Bible Article 13** orders the phases dependency-aware; Phase 3 → 4 → 5 reflects substrate-build ordering. Skipping to Phase 5 would either ship without dedup (regresses ANSPEC §8.2), or build dedup machinery to be redesigned when NeoFS lands, or use a temporary in-memory shim that won't survive into production.

Phase 3 is the natural next critical-path. Phase 4 (network stack) follows Phase 3 per Bible Article 13; Phase 5 (AI subsystem foundation per ANSPEC-001) follows Phase 4.

## 3. Substrate snapshot

### 3.1 What exists today

```text
kernel/src/loader/
├── elf.rs       262 lines — ELF64 loader; reads from Limine modules
└── mod.rs        10 lines

kernel/src/init/
├── boot_frame.rs 85 lines — boot frame allocator
└── mod.rs        11 lines

kernel/src/drivers/
├── console/      serial UART
├── fb/           framebuffer
└── mod.rs        12 lines

kernel/src/main.rs — has MODULE_REQUEST: ModulesRequest::new()
```

**Limine module loading is in place** via `loader/elf.rs` + the `MODULE_REQUEST` static. Phase 1/2 userspace processes (`neo-init`, test workers) load as ELF64 from Limine modules. This is the "read-only Limine-module filesystem" Bible 10.1 names as the pre-Phase-3 stopgap.

**No persistent storage code exists.** No block layer, no FS, no disk drivers. Phase 3 is a clean-slate addition.

**ADR cross-references that bound Phase 3:**
- ADR-002 §124: confirms `kernel/src/fs/` directory placement (Phase 3+).
- ADR-010 §56, §228: copy/move cap ops deferred to Phase 3+ (capability-on-file work, naturally Phase 3 timing).
- ADR-012 §185, §193, §195: `linked_list_allocator` replacement and slab debug instrumentation marked as possible Phase 3+ work.
- ADR-014: kernel observability deliberately deferred until NeoFS exists; procfs-equivalent on top of NeoFS is the working hypothesis.
- ADR-015: KASLR-tolerant heap placement names "Phase 3 NeoFS" as next priority.
- ADR-016 §109: IRQ-safe heap + scheduler decoupling identifies Phase 3 as the implementation phase for A1 (Cerebro work, ~1-2 hours estimated).

### 3.2 Bible-binding Phase 3 requirements

**Article 10:**
- 10.1: NeoFS = Phase 3.
- 10.2: NeoFS content-addressed at block layer; POSIX-ish path namespace; model dedup by content hash.
- 10.3: Typed inodes (not filename extensions); `.neom` is model-typed; kernel can refuse mmap-as-executable of a model file.
- 10.4: FAT32 + ext4 read-only for interop; no write support for foreign FSes unless absolutely necessary.

**Article 9.9 (storage device support, phase-binding):**
- virtio-blk — **required from Phase 3** for VM testing.
- SATA AHCI — **required from Phase 3**.
- NVMe over PCIe — **required from Phase 3**.
- USB mass storage — Phase 4+.
- eMMC/SD — Phase 5+.
- IDE — not supported.

### 3.3 Inherited Phase 2+ forward markers landing in Phase 3 scope

Several SC-3B/SC-3C/SC-4 forward markers are Phase 3+ candidates:

- **FM-D5.4** (PROCESSES single-Mutex correctness boundary) — informational; not Phase 3 actionable.
- **FM-D14** (COMPLETED_EXIT_CODES growth bound; sys_wait consumer) — Phase 5+ multi-threaded gating; not Phase 3 actionable.
- **FM-Q9** (per-CPU reaper distribution with multi-thread) — Phase 2+ multi-thread gating; not Phase 3 actionable.
- **ADR-012 deferred Arc-aware slab integration** — likely Phase 3 actionable. Block-device drivers and FS structures will have allocation patterns (per-IO buffers, inode cache, dentry cache) that may surface the Arc-vs-slab coupling more than Phase 2 did. **PM flag for Phase 3 BUILD-1 OBSERVE pass.**
- **`linked_list_allocator` revisit** (ADR-012 §195) — possible Phase 3+ if FS workloads stress the kernel heap differently than Phase 2 workloads. **Defer to Phase 3 mid-BUILD measurement-driven decision.**
- **ADR-016 A1 IRQ-safe heap + scheduler decoupling** — explicitly named as Phase 3 work. **Should land in Phase 3 BUILD-1 if it doesn't conflict with the block-layer bring-up.**

## 4. Design surface — Q-item enumeration

The Phase 3 design space resolves into ten Q-items spanning three layers (block, VFS, NeoFS) plus foreign-FS drivers plus syscall surface. Each Q-item gets a brief framing here; v2 fills in PM leans + alternatives after Cerebro returns.

### Q1 — Block layer abstraction

**Q1.a — `BlockDevice` trait shape.** What does the kernel-internal API for "a thing you can read blocks from and write blocks to" look like? Linux's `block_device_operations` and FreeBSD's `g_geom` are reference points. Considerations: synchronous vs async (kernel currently has no general-purpose async runtime); sector size assumptions (512 bytes vs 4096 vs device-reported); request queueing.

**Q1.b — Request queue / completion model.** Block I/O is fundamentally async at the hardware level (DMA + completion interrupt). Two camps:
- **Synchronous-blocking-on-completion** — issuing thread blocks until I/O completes. Simplest for the FS layer; composes with the existing `block_current_into` / `wake_thread` primitives.
- **Async with completion callbacks** — issuing thread continues; callback wakes the consumer. More complex but enables overlapping I/O and CPU work.

Phase 3 doesn't have an inference scheduler yet (Phase 7), and the FS workloads at our scale (typical kernel boot + few-thousand-file directory trees + maybe a few-GB model files) don't strictly need async to perform. PM lean: synchronous-blocking with explicit room to upgrade to async later.

**Q1.c — Per-device vs global request queue.** Linux historically used per-device queues + a global elevator; modern blk-mq has per-CPU queues feeding per-device queues. At our scale (1-4 storage devices typical), per-device single-queue is sufficient. PM lean: per-device single-queue; revisit if Phase 5+ inference accelerator scheduling pressure surfaces queue contention.

### Q2 — Block device drivers

**Q2.a — virtio-blk first** (priority 1 per Bible 9.9 for VM testing). Well-documented spec, simple ring-buffer descriptor model, no exotic hardware quirks. Should land in Phase 3 BUILD-1. Cerebro dispatch candidate: prior-art survey of Rust virtio-blk implementations (Theseus, Redox, others) for trait shape + DMA handling patterns.

**Q2.b — SATA AHCI** (priority 2 per Bible 9.9 for bare-metal). More complex than virtio-blk; PCI device discovery, ABAR mapping, command list / received FIS structures, port multiplexing. Substantial driver code. PM lean: separate BUILD from virtio-blk; can land in Phase 3 BUILD-2.

**Q2.c — NVMe over PCIe** (priority 3 per Bible 9.9 for modern hardware). NVMe queue pairs, PRP/SGL, admin queue + I/O queues, MSI-X interrupts. The most architecturally distinct driver of the three. PM lean: Phase 3 BUILD-3 or later.

**Q2.d — Probe + enumeration.** PCI device walking; firmware-reported boot disk identification; mount-by-device-id resolution. Touches `kernel/src/arch/x86_64/` (PCI access) and a new `kernel/src/drivers/block/` module. Independent of which driver lands first.

### Q3 — DMA infrastructure

Phase 3 introduces persistent storage drivers, which need DMA-capable buffers. Current kernel has no DMA layer. Considerations:
- IOMMU-aware vs identity-mapped DMA (Phase 3 BUILD-1 likely identity-mapped; IOMMU is Phase 5+ work).
- Bounce buffers for non-DMA-capable memory regions.
- DMA buffer allocation discipline — slab vs custom pool.
- Cache coherence (mostly trivial on x86_64 with snoop-coherent buses; matters more for aarch64 in Phase 5+).

PM lean: identity-mapped DMA with a small dedicated DMA frame pool reserved at boot. Cerebro dispatch candidate: prior-art survey of minimal-kernel DMA layers (Theseus, Redox, Plan 9, BSD geom_disk).

### Q4 — VFS layer

**Q4.a — Inode shape and typed extensions.** Bible 10.3 mandates typed inodes (not filename-based). Inode struct carries: file type discriminant, size, content-block pointer(s), per-type metadata (for Phase 5: model metadata, model hash). The discriminant is the load-bearing innovation per Bible 10.3.

Design space: which file types does Phase 3 ship? Minimum viable: `Regular`, `Directory`, `Symlink`. Phase 5+ adds `Model`, `Tensor`. Phase 7+ adds `InferenceContext` (per ANSPEC). Phase 3's job is the framework + initial 3 types; adding types later should be a backward-compatible extension.

**Q4.b — Mountpoint hierarchy.** How are multiple FS instances composed? Linux's mount namespace; BSD's mount(2). Bible 10.2 "POSIX-ish path namespace" implies hierarchical mount; not flat. PM lean: single namespace, mount-point hierarchy via VFS-level inode binding (the directory inode at mount-point delegates lookups to the mounted FS root).

**Q4.c — Path resolution.** From "/foo/bar" to inode. Component-by-component lookup, with mount-point traversal. Symlink expansion bounds (Linux's 40-symlink limit). Permission checks at each component (Phase 5+ capability-on-file).

**Q4.d — Caching strategy.** Inode cache, dentry cache, page cache. Phase 3 BUILD-1 can ship minimal (no page cache; sync I/O on every read); BUILD-2 adds caching for performance. PM lean: defer page cache to a measurement-driven Phase 3 BUILD.

### Q5 — NeoFS native layer

**Q5.a — Content-addressed block layer.** Bible 10.2: NeoFS is content-addressed *at the block layer*. This means every block has a content hash, and blocks with identical content share storage. Inodes point to content hashes; content hashes resolve to physical block addresses. Two-level indirection.

Content hash algorithm: BLAKE3 per ANSPEC §8.2 (already PM-leaned for model dedup; consistent choice for FS block dedup). BLAKE3 is fast (~7 GB/s on modern x86_64), no_std-friendly, well-audited.

Block size: standard 4 KiB pages or NeoFS-specific (8/16/32 KiB for better dedup ratio)? Trade-off: smaller blocks → finer-grained dedup; larger blocks → less metadata overhead. PM lean: 4 KiB blocks initially, document the choice for revision if model-storage workloads show poor dedup.

**Q5.b — Inode-to-content mapping.** Inode carries a Merkle-tree of content hashes (for files > one block). Each tree node carries hashes of children; leaf hashes are block content hashes. Cerebro dispatch candidate: prior-art survey of content-addressed filesystems (IPFS, Tahoe-LAFS, ZFS dedup, Venti from Plan 9, git's object model).

**Q5.c — On-disk format.** What does the NeoFS image on disk look like? Superblock at known offset; inode table; content-block region; free-space bitmap. PM lean: minimal Phase 3 BUILD-1 format with explicit "this format is unstable; format revision number in superblock; tools may need rebuild on revision bump" discipline.

**Q5.d — Crash consistency.** What happens if power is lost mid-write? Options:
- Journaling (ext4 / NTFS pattern).
- Copy-on-write with atomic superblock swap (ZFS / Btrfs pattern; natural fit for content-addressed storage).
- No crash consistency — accept fsck-style recovery (FAT32 pattern; weakest discipline).

PM lean: CoW with atomic superblock swap. Content-addressed storage makes this natural — writing a new block doesn't overwrite an old block, so atomicity is at the inode-tree-pointer level. Cerebro dispatch candidate: ZFS / Btrfs CoW discipline review for the specific atomicity invariants.

**Q5.e — Free space management.** With CoW + content-addressing, "free" means "no inode references this content hash." Reference counts on content hashes; GC sweep when refcount drops to zero. Trade-off vs allocation bitmap (faster allocation, harder dedup tracking). PM lean: refcounted content hashes; GC at unmount + periodic.

### Q6 — Foreign FS drivers

**Q6.a — FAT32 read-only.** Long-history filesystem, well-documented, primary use case is reading UEFI ESP and bootable USB sticks. Reference implementations everywhere (Linux, FreeBSD, ReactOS, redox-os, FuseFS).

**Q6.b — ext4 read-only.** More complex (ext2/3/4 lineage, extents, journals, htree directories). Linux-canonical; reference implementations in Linux + Redox.

Both are read-only per Bible 10.4. Phase 3 BUILD-3 territory.

### Q7 — Syscall surface

**Q7.a — POSIX-ish file syscalls.** sys_open / sys_close / sys_read / sys_write / sys_lseek / sys_stat / sys_mkdir / sys_unlink / sys_rename / sys_readdir / sys_mount / sys_umount. Bible 5.2: "the native syscall ABI is our own." We can deviate from Linux numbering and semantics where it earns its keep.

PM lean: handle-based (capabilities-on-files per ADR-010 framework); inode-typed open semantics (sys_open returns a capability handle to a Regular file vs Directory vs Model with appropriate rights); explicit reject of mmap-as-executable on Model-typed inodes per Bible 10.3.

**Q7.b — Capability discipline.** Per ADR-010 §228, copy/move cap ops deferred to Phase 3+. Phase 3 may need to introduce these for file capabilities specifically (passing an open file handle to another process via grant). PM lean: extend ADR-010 capability framework to FileCap; reuse sys_grant/sys_recv_grant mechanism with file-specific rights (READ/WRITE/EXEC/DELETE).

### Q8 — Boot-time integration

**Q8.a — Root filesystem at boot.** Phase 1/2 use Limine modules as the "filesystem." Phase 3 needs a real boot story: where does `neo-init` come from?

Options:
- **Initramfs-equivalent from Limine module.** Limine loads a cpio-style archive as a module; kernel parses and mounts as initial ramdisk; neo-init runs from initramfs; later mounts root FS from disk.
- **Direct disk boot.** Kernel + Limine on disk; kernel mounts NeoFS root directly; neo-init from disk.
- **Hybrid.** Initramfs from Limine module for early boot; pivots to disk root once block drivers are up.

PM lean: hybrid. Initramfs for early boot (consistent with existing Limine-module pattern); pivot to disk root after virtio-blk + NeoFS mount succeed. Backward compatible with existing test infrastructure (test runs use Limine-module initramfs only; no disk image needed for most tests).

**Q8.b — Test infrastructure.** How do existing scenarios interact with Phase 3? Most current scenarios don't touch FS; they remain valid. New scenarios needed: block I/O smoke, FS mount, file read/write, mount-foreign-FS, model-file-mmap-refused-as-executable. Phase 3 test harness extension parallel to SC-4's q8a/b/c/d additions.

### Q9 — libneo + userspace integration

**Q9.a — libneo file API.** Wrappers around Phase 3 syscalls; opens/reads/writes/etc.; capability-aware. PM lean: thin wrappers, no caching, no buffered I/O at libneo layer (userspace can build buffered I/O on top if desired).

**Q9.b — neo-init evolution.** Currently spawned from Limine module. Phase 3 neo-init may need to: probe block devices, mount root FS, exec userspace init proper from disk. Minor logic addition; doesn't fundamentally change neo-init's role.

### Q10 — Phase 5+ prep

**Q10.a — Model-typed inode + content-hash visibility.** Phase 5's model dedup (ANSPEC §8.2) needs the FS layer to expose content hashes to upper layers. Phase 3 BUILD-2 NeoFS naturally produces this; expose via inode metadata or a dedicated syscall (`sys_file_content_hash(fd) -> [u8; 32]`).

**Q10.b — mmap discipline.** Phase 5 loads `.neom` files via sys_model_load; the kernel reads from FS. Whether this uses mmap or read syscalls is a Phase 5 design choice, but Phase 3 should not preclude either by hardcoding assumptions. PM lean: support both; Phase 5 chooses per ANSPEC.

**Q10.c — Initial model storage convention.** Where do `.neom` files live on disk? `/lib/models/` per Unix-ish convention? Phase 3 sets the convention; Phase 5 follows.

## 5. BUILD boundary proposal

Phase 3 scope is substantially wider than Phase 2 BUILD-3. PM lean: split into four BUILDs.

### Phase 3 BUILD-1 — Block layer + virtio-blk + minimal VFS

Goal milestone: kernel boots, brings up virtio-blk under QEMU, reads + writes individual blocks, mounts a tmpfs-equivalent VFS, exposes minimal sys_open/sys_read/sys_write syscalls.

Sub-commits (proposed):
- **3-BUILD-1 sub-commit 0** — ADR for Phase 3 design (analogous to ADR-013 for Phase 2 BUILD-3). PM authors after THINK v2 closes.
- **3-BUILD-1 sub-commit 1** — Block layer trait + DMA infrastructure + PCI walking.
- **3-BUILD-1 sub-commit 2** — virtio-blk driver.
- **3-BUILD-1 sub-commit 3** — VFS layer + minimal in-RAM FS (tmpfs-shape).
- **3-BUILD-1 sub-commit 4** — Initial sys_open/sys_read/sys_write/sys_close syscalls; libneo wrappers.
- **3-BUILD-1 sub-commit 5** — Test scenarios for block I/O + VFS smoke + read/write round-trip.

Coordinated with: ADR-016 A1 IRQ-safe heap + scheduler decoupling (Phase 3 BUILD-1 if it composes; otherwise BUILD-2).

### Phase 3 BUILD-2 — NeoFS native

Goal milestone: NeoFS image creation tooling (userspace), kernel mounts NeoFS from virtio-blk, content-addressed block storage works, model-file-typed inode supported, dedup verified by loading same content twice and observing one block.

Sub-commits roughly:
- NeoFS on-disk format spec (separate spec doc at `docs/specs/neofs-format.md`).
- Content-addressed block layer (Merkle tree, content hashing).
- Inode layer + typed extensions.
- CoW + atomic superblock swap discipline.
- Refcounted free space + GC.
- Test scenarios for dedup, CoW, crash-recovery.

### Phase 3 BUILD-3 — Foreign FS read-only + initial userspace

Goal milestone: FAT32 mount works; ext4 mount works; neo-init can load executables from a foreign-mounted FS.

Sub-commits roughly:
- FAT32 driver.
- ext4 driver.
- sys_mount / sys_umount syscalls.
- Test scenarios.

### Phase 3 BUILD-4 — Real hardware drivers + bare-metal validation

Goal milestone: SATA AHCI works on real hardware; NVMe works on real hardware; bare-metal verification of full FS stack.

Sub-commits roughly:
- AHCI driver.
- NVMe driver.
- Bare-metal verification artifact per Bible Article 13.

## 6. Anti-scope

Phase 3 does NOT include:

- **Write support for FAT32 / ext4** unless absolutely necessary (Bible 10.4).
- **Page cache** in initial BUILD-1; deferred to measurement-driven later BUILD if needed.
- **Async I/O** at the block layer; synchronous-blocking is the Phase 3 design.
- **IOMMU-aware DMA**; identity-mapped DMA is sufficient for Phase 3 hardware support. IOMMU is Phase 5+.
- **Network filesystems** (NFS, SMB, etc.); Phase 4+ at earliest, ADR-gated.
- **Distributed filesystems**; out of scope (Bible 4.5 single-node).
- **eMMC / SD / M.2-non-NVMe** (Bible 9.9 Phase 5+).
- **USB mass storage** (Bible 9.9 Phase 4+).
- **Procfs-equivalent** (ADR-014 deferred until NeoFS exists, then becomes Phase 3+ follow-on).
- **Capability on file extensions** beyond minimum READ/WRITE/EXEC/DELETE; richer cap semantics (e.g., "model-load" capability) deferred.
- **Multi-thread per process** still single-thread-per-process per SC-4 D13.

## 7. Cerebro dispatch candidates

Three Cerebro deep-research dispatches identified as likely needed for Phase 3 THINK v2:

### Cerebro dispatch #1 — Content-addressed filesystem prior art

Survey IPFS, Tahoe-LAFS, ZFS dedup, Plan 9's Venti, git's object model. Focus: on-disk format choices, content-block addressing schemes, refcount-vs-mark-sweep GC, deduplication ratios in practice for mixed workloads (text + media + large binary blobs), CoW + atomicity disciplines.

Output anchors Q5.a/b/c/d/e.

Estimated 6-10 hours Cerebro session; comparable to SC-4 D5 in scope.

### Cerebro dispatch #2 — Storage driver prior art (virtio-blk + AHCI + NVMe)

Survey Theseus, Redox, FreeBSD, Linux storage driver patterns. Focus: DMA-buffer discipline in safe-Rust kernels; PCI device probe patterns; interrupt handling for completion; request queue shapes; how to compose with the kernel's existing IRQ-safe / preempt-safe discipline (SC-3B/SC-3C/SC-4 inheritance).

Output anchors Q1.a/b/c, Q2.a/b/c, Q3.

Estimated 6-10 hours.

### Cerebro dispatch #3 — VFS layer design prior art

Survey Linux VFS, BSD VFS, Plan 9 9P, V Operating System, seL4 capability-on-file. Focus: typed-inode disciplines (where do file types live in the data model?), mount-point hierarchy implementations, path resolution algorithms, capability composition with VFS (most existing systems are uid-based; we need capability-shape).

Output anchors Q4.a/b/c, Q7.b.

Estimated 4-8 hours.

PM operational call: **dispatch #1 (NeoFS prior art) first**; it's the load-bearing Phase 3 design that the most invention rides on. Dispatches #2 and #3 can ferry after PM consumes #1, or in parallel if PL prefers maximum throughput. SC-3C D15 + SC-4 D5 precedent shows Cerebro can handle parallel dispatches; PM ferry preference governs ordering.

## 8. Open PL-tier decisions

Most Phase 3 design is PM-tier per the operational posture clarification. PL sign-off needed only for:

1. **BUILD boundary scope.** §5 proposes 4 BUILDs. PL may prefer fewer/larger or more/smaller BUILDs. PM lean: 4 is right — each BUILD has a single coherent goal milestone; each maps to a Bible/ANSPEC requirement; each gets its own verification cycle.
2. **Cerebro dispatch authorization.** §7 proposes 3 dispatches. PL operational call on whether to authorize all three now (parallel) or one-at-a-time (sequential per SC-4 D5 pattern). PM lean: sequential, dispatching #1 first.
3. **Bible amendment candidates.** Bible Article 10 is 4 sentences — likely needs sharpening for Phase 3 implementation (specific format hashes, mount syscall shape, etc.). PM lean: defer Bible amendment to AFTER Phase 3 BUILD-1 closes — let implementation experience inform what the Bible should sharpen. Article 10 in its current four-sentence form is intentionally underspecified and that's a feature.

## 9. PM-tier dispositions (recorded; no PL ratification needed)

- **Sub-commit 0 of Phase 3 BUILD-1 lands a new ADR** (ADR-021 or whatever next number) — Phase 3 design substrate, parallel to ADR-013 for Phase 2 BUILD-3.
- **Phase 3 THINK v2** follows Cerebro dispatch #1 returns + PM consumption.
- **Phase 3 PLAN dispatch for Jarvis** follows THINK v2 completion.
- **Phase 3 BUILD-1 ETA:** rough 3-6 weeks once started, comparable to Phase 2 BUILD-3 sub-commits 0+1a+1b cumulative scope.
- **rustfmt tooling gap from Phase 2** carries forward; needs PL operational call before Phase 3 BUILD-1 closes.
- **VMWare Workstation hypervisor-VM verification** (per PL hardware availability signal) can land in parallel with Phase 3 BUILD-1 since they're independent workstreams.

## 10. Next operational steps

1. PM authors Cerebro dispatch #1 (NeoFS prior art) — drafted next; ferries on PL signal.
2. PL ratifies BUILD boundary scope (§5) and Cerebro dispatch sequencing (§7).
3. After Cerebro #1 returns: PM consumes, authors Phase 3 THINK v2 with Q5 design finalized.
4. After THINK v2: Cerebro dispatches #2 + #3 (storage drivers + VFS prior art) in parallel.
5. After both return: PM consumes, files final THINK, drafts ADR-021 Phase 3 design substrate.
6. PM authors Phase 3 BUILD-1 PLAN dispatch for Jarvis.

Parallel non-blocking workstream: VMWare Workstation hypervisor-VM verification procedure document.

## 11. Cross-references

**Substrate:**
- `kernel/src/loader/elf.rs` (262 lines) — existing module loader.
- `kernel/src/main.rs` `MODULE_REQUEST` static — Limine module support already in place.
- `kernel/src/drivers/{console,fb}/` — only existing driver classes.

**Bible references:**
- Article 9.9 (storage device support, phase-binding).
- Article 10 (filesystem and storage; 4 sub-points).
- Article 13 (phase ledger).

**ADR cross-references:**
- ADR-010 §56, §228 — copy/move cap ops deferred to Phase 3+.
- ADR-012 §185, §193, §195 — heap allocator + slab debug + slab integration revisit candidates.
- ADR-013 — Phase 2 BUILD-3 SMP retrofit (just closed).
- ADR-014 — kernel observability (procfs-on-NeoFS hypothesis).
- ADR-015 — KASLR-tolerant heap (Phase 3 next priority).
- ADR-016 §109 — IRQ-safe heap + scheduler decoupling (A1 = Phase 3 implementation).

**ANSPEC-001 cross-references:**
- §3.7 capability handles on tensors (Phase 3 inode-typed analog).
- §4.1 .neom format binary structure (Phase 5 consumer of Phase 3 FS).
- §4.3 sys_model_load with hash verification (Phase 5; needs Phase 3 content-hash visibility).
- §8.2 deduplication keyed by BLAKE3 content hash (Phase 5; backed by Phase 3 NeoFS dedup).
- §10.3 model integrity hash verification (Phase 5; Phase 3 content-hash provides the substrate).
- §10.5 maliciously crafted `.neom` mitigations (typed-inode refusal-of-executable per Bible 10.3 = Phase 3 mitigation).

**Phase 2 BUILD-3 lineage:**
- `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md`, `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` — completed work that Phase 3 builds on.
- `Plans/Q9-EXPLORATION-2026-05-11.md` — Phase 2 BUILD-3 substantially-complete declaration.

---

## 12. THINK v1 summary

Phase 3 closes Bible Article 13's third milestone (NeoFS + block layer + real disk I/O) and unblocks Phase 5 AI subsystem infrastructure (content-addressed model dedup per ANSPEC §8.2; typed-inode model file refusal per Bible 10.3). Design surface: 10 Q-items spanning block layer, VFS, NeoFS, foreign FS drivers, syscall surface; PM-leaned BUILD split into 4 (block + minimal VFS / NeoFS native / foreign FS / real hardware).

Cerebro deep-research needed before THINK v2 lands; three dispatches identified, PM operational call to sequence with dispatch #1 (NeoFS prior art) first.

Phase 3 ETA: roughly 6-12 months wall-clock for full closure across 4 BUILDs, given Phase 2 BUILD-3's scope-to-time ratio. BUILD-1 alone targets 3-6 weeks.

---

**End of Phase 3 THINK v1.**

*Exploration shape; identifies design space + Q-item enumeration + Cerebro dispatch candidates. PM operational decisions recorded; PL ratification needed only on BUILD-boundary scope, Cerebro sequencing, and Bible-amendment timing. v2 follows Cerebro dispatch #1 (NeoFS prior art) consumption.*
