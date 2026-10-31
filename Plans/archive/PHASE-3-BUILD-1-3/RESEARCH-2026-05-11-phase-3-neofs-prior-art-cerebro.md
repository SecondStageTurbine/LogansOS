# Research: Phase 3 NeoFS Prior Art — CAS Filesystem Design

| | |
|---|---|
| **Date** | 2026-05-11 (fresh-context Cerebro session) |
| **Author** | Cerebro deep-research artifact (dispatched via `DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`) |
| **Type** | Research artifact (informational; not normative). Prior-art survey for Phase 3 THINK v1 §4 Q5.a-e. |
| **Scope** | Content-addressed FS on-disk format (Q1), crash consistency (Q2), GC strategy (Q3), Plan 9 Venti (Q4), typed inodes (Q5), synchronous I/O (Q6) |
| **Authority** | None. PM consumes to update Phase 3 THINK v2 and author ADR-021. |
| **Not surveyed** | HF Xet, ZipLLM, Ollama (covered in `ALTIMIT-real-world-implementation.md` §D+§E); BLAKE3 vs alternatives (decided); FAT32/ext4 implementation (out of scope) |
| **Time budget actual** | ~2.5 hours (three parallel research agents + synthesis) |

---

## 1. Executive Summary

**The single most load-bearing finding for Phase 3 planning is a strategic reframing of PM lean D5:** defer content-addressing at the block layer to Phase 3 BUILD-2. NeoFS BUILD-1 should ship a correct, crash-consistent, snapshot-capable single-tier CoW filesystem with physical block addresses — not with hash-as-address. All of the structural elegance of the Merkle-DAG design (crash consistency, snapshots-for-free, fsck-free operation, future send/receive replication) is available without eager content-addressing, because it flows from the tree structure, not from the address scheme. ZFS dedup is the canonical cautionary tale: "operationally good now, don't use it" is the 2024 OpenZFS maintainers' own verdict on the most-scrutinized CAS dedup implementation in production. The Fossil-over-Venti precedent mirrors this: Plan 9 shipped the mutable layer (Fossil) years before the CAS backend (Venti) was production-ready, and when Fossil was retired it was replaced by a single-tier CoW system (GEFS), not a different two-tier system.

**On crash consistency:** PM's lean — CoW with atomic superblock swap — is correct and is confirmed by ZFS, Btrfs, and HAMMER independently. But the implementation directive needs refinement. "Atomic superblock swap" in production does not mean "the hardware atomically writes a 4 KiB sector." Consumer NVMe drives advertise AWUPF=0 (Atomic Write Unit Powerfail = 0 physical sectors), meaning no multi-LBA write is guaranteed atomic across power loss. What ZFS, Btrfs, and HAMMER actually do is: (a) multiple redundant superblock copies at known offsets, (b) a checksum on each copy so torn writes are self-detecting, (c) a generation/txg counter so the newest valid copy is chosen at mount. The write barrier discipline is REQ_PREFLUSH/REQ_FUA on Linux (ATA FLUSH CACHE / NVMe FLUSH at the hardware level). For NeoFS, the commit sequence is: write all content blocks → FLUSH → write new superblock → FLUSH. Two flushes per transaction is the unavoidable correctness cost and matches all production CoW filesystems.

**On GC:** the literature has converged. Real-time refcount GC (ZFS DDT style) is correct but operationally expensive: 216–424 bytes RAM per unique block entry, 1–3 GiB RAM per TB of dedup data, CPU-intensive on every read and write. The 2024 OpenZFS maintainer assessment is "good now, don't use it." For NeoFS BUILD-2 (when CAS dedup lands), Cerebro recommends starting with mark-sweep GC (IPFS model: zero per-write overhead, full-blockstore scan at idle/watermark), adding refcount as an advisory hint only if measured workloads show GC-pause latency is a problem.

**On synchronous I/O:** NeoFS's single-thread-per-process model (SC-4 D13) is the original Unix model, not a constraint to work around. FFS (1984), Plan 9, and xv6 all assumed it. The IRQ→DMA-completion→thread-wake discipline has a canonical solution (Plan 9 rendezvous, Linux completions, xv6 sleep/wakeup) that composes directly with NeoPlutarch's existing SC-3B SwitchMutex / SC-3C IPI-cross-CPU-wake machinery. NeoFS should be the first non-scheduler-internal client of the SC-3C IPI cross-CPU-wake primitive; it should not introduce a parallel wake mechanism.

**On typed inodes (Bible 10.3):** NeoPlutarch is past the production prior-art horizon. No production OS implements per-inode type-discriminant mmap refusal at the FS layer in the way Bible 10.3 envisions. SELinux file types with LSM hooks are the closest in outcome; Capsicum per-fd rights restriction is the closest in mechanism; Plan 9 9P Qid types are the closest in on-disk structure. Bible 10.3 synthesizes all three into something genuinely novel — and that novelty is justified by the ALTIMIT canon. The implementation is tractable: 1-byte type field in the inode discriminant, small fixed enum, VFS mmap/execve dispatch reads the type. The hard engineering is the boundary cases: foreign-FS type import, type extension versioning, retype operations.

---

## 2. Sources Surveyed

### Primary sources — verified at URL

- Quinlan & Dorward, "Venti: A New Approach to Archival Storage," USENIX FAST 2002 (HTML): `https://9p.io/sys/doc/venti/venti.html` and `http://doc.cat-v.org/plan_9/4th_edition/papers/venti/`. Read for score definition, arena structure, block sizes, index structure, WORM semantics.
- Fossil paper, Quinlan et al.: `http://doc.cat-v.org/plan_9/4th_edition/papers/fossil`. Read for Fossil-Venti two-tier composition, local vs Venti scores, archival snapshot discipline.
- fossil(4) man page: `https://9p.io/magic/man2html/4/fossil`. Read for operational description.
- 9front FQA §7 (Fossil retirement rationale): `https://fqa.9front.org/fqa7.html`. Read for Plan 9 community Fossil post-mortem.
- Foundation paper (Russ Cox et al., compare-by-value CAS): `https://swtch.com/~rsc/papers/fndn/`. Read for the compare-by-value response to SHA-1 collision risk in Venti.
- Valerie Henson, "An Analysis of Compare-by-hash" (2003): `https://www.andrew.cmu.edu/course/15-749/READINGS/required/cas/henson-hash.pdf`. Read for original CAS collision-risk argument.
- Pro Git, "Git Internals: Git Objects": `https://git-scm.com/book/en/v2/Git-Internals-Git-Objects`. Read for git object model, packfiles, GC discipline.
- OpenZFS `module/zfs/ddt.c` master: `https://github.com/openzfs/zfs/blob/master/module/zfs/ddt.c`. Read for DDT entry structure, `ddt_phys_t`, `ddt_phys_decref`, refcount mechanics.
- OpenZFS `module/zfs/spa_log_spacemap.c`: `https://github.com/openzfs/zfs/blob/master/module/zfs/spa_log_spacemap.c`. Read for log-structured Space Map.
- OpenZFS tuning documentation: `https://openzfs.github.io/openzfs-docs/Performance%20and%20Tuning/Workload%20Tuning.html`. Read for recordsize and dedup guidance.
- Klara Systems, "Tuning recordsize in OpenZFS": `https://klarasystems.com/articles/tuning-recordsize-in-openzfs/`. Read for block-size/dedup-ratio tradeoff.
- Klara Systems, "Introducing OpenZFS Fast Dedup": `https://klarasystems.com/articles/introducing-openzfs-fast-dedup/`. Read for 2024 fast_dedup DDT memory improvements.
- Despair Labs, "OpenZFS deduplication is good now and you shouldn't use it" (2024): `https://despairlabs.com/blog/posts/2024-10-27-openzfs-dedup-is-good-dont-use-it/`. Read for operational cost analysis and industry verdict.
- TrueNAS ZFS Deduplication guide: `https://www.truenas.com/docs/references/zfsdeduplication/`. Read for DDT memory cost (1–3 GiB/TB).
- Btrfs on-disk format spec: `https://btrfs.readthedocs.io/en/latest/dev/On-disk-format.html`. Read for superblock locations (three copies), generation fields, checksum discipline.
- Btrfs design notes: `https://btrfs.readthedocs.io/en/latest/dev/dev-btrfs-design.html`. Read for CoW write ordering.
- mkfs.btrfs(8): `https://btrfs.readthedocs.io/en/latest/mkfs.btrfs.html`. Read for sectorsize/nodesize defaults.
- DragonFlyBSD HAMMER: `https://www.dragonflybsd.org/hammer/`. Read for CoW + UNDO/REDO FIFO log hybrid design.
- HAMMER Wikipedia: `https://en.wikipedia.org/wiki/HAMMER_(file_system)`. Read for no-fsck claim.
- IPFS GC documentation: `https://docs.ipfs.tech/how-to/kubo-garbage-collection/`. Read for mark-sweep triggers, 1-hour period, 90% watermark.
- kubo `gc/gc.go`: `https://github.com/ipfs/kubo/blob/master/gc/gc.go`. Read for GCLock acquisition, AllKeysChan sweep.
- IPFS File Systems documentation: `https://docs.ipfs.tech/concepts/file-systems/`. Read for 256 KiB default chunk size, UnixFS DAG.
- IPFS Merkle DAG documentation: `https://docs.ipfs.tech/concepts/merkle-dag/`. Read for immutability and self-verification properties.
- Linux `Documentation/block/writeback_cache_control.txt`: `https://docs.kernel.org/5.19/block/writeback_cache_control.html`. Read for REQ_PREFLUSH, REQ_FUA semantics.
- LWN, "Atomic block-write operations": `https://lwn.net/Articles/933015/`. Read for AWUPF=0 on consumer hardware.
- NVMe NVM Command Set Specification v1.0a: `https://nvmexpress.org/wp-content/uploads/NVMe-NVM-Command-Set-Specification-1.0a-2021.07.26-Ratified.pdf`. Read for FLUSH "shall commit to non-volatile media" mandate.
- OASIS virtio v1.2: `https://docs.oasis-open.org/virtio/virtio/v1.2/csd01/virtio-v1.2-csd01.html`. Read for VIRTIO_BLK_F_FLUSH feature bit and write-through fallback.
- FFS paper (McKusick, Joy, Leffler, Fabry, 1984): `https://dsf.berkeley.edu/cs262/FFS.pdf`. Read for synchronous-reads / async-writes-via-buffer-cache discipline.
- Plan 9, "Process Sleep and Wakeup on a Shared-memory Multiprocessor" (Pike, Presotto, Thompson): `http://doc.cat-v.org/plan_9/4th_edition/papers/sleep`. Read for rendezvous discipline, inhibit-before-lock, SMP-safe wakeup invariant.
- Linux Kernel completions docs: `https://www.kernel.org/doc/html/latest/scheduler/completion.html`. Read for IRQ-safe `complete()` and `done` counter semantics.
- FreeBSD msleep(9) man page: `https://man.freebsd.org/cgi/man.cgi?msleep(9)`. Read for sleep/wakeup wait-channel model.
- MIT 6.828 xv6 sleep/wakeup materials: `https://pdos.csail.mit.edu/6.828/2019/lec/l-coordination.txt`. Read for simplest correct lost-wakeup solution.
- 9P2000 RFC (Van Hensbergen): `https://ericvh.github.io/9p-rfc/rfc9p2000.html`. Read for Qid type bits.
- Plan 9 stat(5) man page: `https://9p.io/magic/man2html/5/stat`. Read for Qid type enum.
- seL4 Reference Manual 13.0.0: `https://sel4.systems/Info/Docs/seL4-manual-13.0.0.pdf`. Read for typed-object model.
- Watson et al., "Capsicum: Practical Capabilities for UNIX," USENIX Security 2010: `https://www.cl.cam.ac.uk/research/security/capsicum/`. Read for per-fd rights and capability-mode propagation.
- FreeBSD rights(4): `https://man.freebsd.org/cgi/man.cgi?query=rights&sektion=4`. Read for CAP_MMAP_X.
- OpenBSD mmap(2): `https://man.openbsd.org/mmap.2`. Read for W^X wxallowed/wxneeded mechanism.
- LWN, "Enhance SELinux control of executable mappings": `https://lwn.net/Articles/113917/`. Read for file-type-based mmap refusal.
- Eclectic Light, "APFS extended attributes revisited" (2024): `https://eclecticlight.co/2024/05/13/apfs-extended-attributes-revisited/`. Read for macOS foreign-FS type-mapping via AppleDouble.
- Be File System / Haiku BFS: `https://en.wikipedia.org/wiki/Be_File_System`, `https://birdhouse.org/beos/byte/24-scripting_the_bfs/`. Read for BEOS:TYPE xattr as advisory kernel-stored type.

### Cannot verify from primary source

- HAMMER design paper (Dillon 2008) at `https://www.dragonflybsd.org/hammer/hammer.pdf` — PDF binary, not text-extractable. HAMMER design confirmed via DragonFlyBSD website and Wikipedia secondary sources.
- ZFS on-disk format spec PDF — binary, not extractable. ZFS format confirmed via OpenZFS source comments and secondary documentation.
- Theseus OS I/O model — primary source not fetchable within budget. Not used as a design basis.

### Secondary sources used where primary not directly quotable

- iBug.io, "Understanding ZFS block sizes" — corroborates recordsize/dedup tradeoff.
- Internals-for-Interns, "Btrfs Filesystem" — corroborates three-superblock and write-ordering.
- uberblock Struct Reference (FreeBSD Doxygen) — corroborates label/ring layout.
- Oracle Solaris blog on dedup sizing — corroborates DDT memory cost figures.

---

## 3. Q1 — On-Disk Format for Content-Addressed FS

### 3.1 Block sizes in production CAS systems

**Plan 9 Venti:** Variable-sized blocks up to approximately 52–56 KiB. Per the 9p.io HTML: blocks "up to a current limit of 52 Kbytes." In Fossil-over-Venti deployments, the default block size used by Fossil for data blocks is **8 KiB**. Each block is identified by its **score** — a 20-byte SHA-1 hash of the block contents. The score is both address and integrity check.

**ZFS:** Default `recordsize = 128 KiB`, configurable from 512 B to 16 MiB. Dedup granularity equals recordsize — a 128 KiB record is the smallest dedup unit. Smaller recordsize increases dedup ratio but proportionally multiplies DDT entries (and thus RAM and disk overhead). At 8 KiB recordsize vs 128 KiB, dedup ratio on a database workload may improve 10×, but DDT size grows 16×.

**IPFS:** Default block size (fixed chunker) = **256 KiB**. An optional Rabin fingerprint variable-chunk chunker exists but is not the default. Block hash = SHA-256 encoded as CIDv1 (multihash). Each block is an independent content-addressed unit.

**Btrfs:** Sectorsize default = **4 KiB** (matching MMU page size). Nodesize (metadata tree block) = **16 KiB** (bumped from 4 KiB in v3.12 to reduce tree depth and lock contention). Btrfs has no native in-kernel dedup; offline tools (`bees`, `duperemove`) operate at extent granularity on a batch/periodic basis.

**Git:** No fixed block size. Objects (blobs, trees, commits, tags) are variable-length, zlib-compressed, stored in loose-object files keyed by SHA-1 hash of `"<type> <len>\0<content>"`. Git is **object-level CAS, not block-level CAS** — there is no fixed-size content-addressed primitive. Packfiles add delta-compression across objects. This model is not directly applicable to a block-level filesystem.

**Summary for NeoFS D1 (4 KiB block size lean):**

The 4 KiB lean is confirmed-with-qualification. It is the Btrfs sectorsize default (matching MMU page granularity), enables direct page-cache integration, and maximizes dedup ratio for small files. The qualification: at 1 TB of unique data with 4 KiB blocks, an index mapping 256 M content hashes at ~64 B each costs ~16 GiB of index metadata — this is the operational wall ZFS dedup hits first (DDT must stay hot in RAM). The recommendation is to make block size a **mkfs parameter** (default 4 KiB, support 16/64/256 KiB) and document the index-pressure trade-off. This preserves the lean while avoiding a compile-time hard-code that limits model-storage workloads.

### 3.2 Two-level indirection encoding

**Venti — bloom filter + disk-resident hash table with binary search per bucket:**

Per 9p.io: a disk-resident hash table distributed across dedicated index disks. Hash buckets are fixed-size disk blocks; within each bucket, entries are sorted by score and found by binary search. An in-memory bloom filter front-ends the on-disk table for negative lookups. Design target: **one disk seek per lookup** in the steady state. Each index entry maps (score, type) → (arena identity, offset within arena).

**ZFS DDT — ZAP-backed hashtable in the Meta Object Set:**

Key: `ddt_key_t` = 256-bit checksum + block properties. Value: `ddt_phys_t` = up to 3 DVAs (Data Virtual Addresses, physical location pointers for ditto copies) + `ddp_refcnt` (uint64) + `ddp_phys_birth` (txg number). The DDT lives in the MOS (Meta Object Set) and is synced at txg commit. In-RAM entry size: ~216 B (fast_dedup, post-2024) to ~424 B (traditional). On-disk: 300–900 B per entry. At 4 KiB blocks with 50% duplication on a 1 TB volume: ~13–27 GiB RAM for the hot DDT.

**IPFS — flat blockstore keyed by CID on the host filesystem:**

Each block is a file in a sharded directory structure (`$IPFS_PATH/blocks/`) keyed by the last few characters of the CID. No B-tree; lookup is O(1) amortized via the host FS's directory hash, but requires the host FS to be present and accessible. GC requires scanning every block in the store (mark-sweep from pinned roots), because there is no in-memory reference-count structure.

**Trade-off for NeoFS:**

| System | Index structure | Insert | Lookup | Memory overhead |
|---|---|---|---|---|
| Venti | bloom + disk hashtable + binary search | append | ~1 disk seek | bloom filter only (bloom) + on-disk index |
| ZFS | ZAP in MOS, txg-batched | txg atomic | RAM cache or 1 ZAP read | 216–424 B/entry in RAM |
| IPFS | host-FS file keyed by CID | host-FS create | host-FS open | none (no in-RAM structure) |

For NeoFS BUILD-2 CAS, the Venti model (bloom filter + on-disk disk hash table with binary search) is the most directly applicable and lowest-memory-overhead option. It achieves ~1 seek per lookup without requiring the entire index in RAM.

### 3.3 Free-space tracking

**ZFS Space Maps (log-structured per metaslab):**

ZFS divides each vdev into ~200 metaslabs. Each metaslab has a Space Map: an **append-only log of (allocated | free) range entries stored as a DMU object**. On metaslab activation, the log replays into an in-memory AVL tree for efficient allocation. The Log Spacemap feature (post-0.8) adds a pool-wide log that batches changes, periodically condensed to reduce write amplification. Random frees are O(1) appends — as efficient as sequential frees.

**Venti — no free-space tracking (append-only):**

Per 9p.io: "the resources needed to store the data have been consumed and cannot be reclaimed." Each arena is sealed when full and fingerprinted. Free space at the Venti layer is purely the tail of the current unsealed arena. This model is only feasible for archival-only storage.

**IPFS — mark-sweep over CID blockstore:**

No free-space tracking at the CAS layer. IPFS deletes blocks not reachable from the marked pinset during GC. The host filesystem handles physical allocation.

**For NeoFS BUILD-1:** free space management is a **physical-layer concern, not a CAS concern**, because BUILD-1 uses physical block addresses. A bitmap or block group descriptor (ext2 style) is sufficient. When CAS lands in BUILD-2, refcount-on-content-hash (ZFS model) or bloom-filter-accelerated mark-sweep (Venti-adjacent) become the choices.

---

## 4. Q2 — Crash Consistency for CoW CAS FS

### 4.1 Superblock swap atomicity — what production systems actually do

**ZFS uberblock rotation (the canonical implementation):**

ZFS maintains four vdev labels (L0, L1 near the beginning of the vdev; L2, L3 near the end). Each label contains a **128-entry uberblock ring**, 1 KiB per slot. New uberblock written at slot `txg mod 128` — never overwrites a recent valid uberblock until the ring wraps. Active uberblock = highest txg with valid SHA-256 checksum. Write order: even labels (L0, L2) → cache flush → odd labels (L1, L3). This guarantees at least one valid pair survives any single torn write.

**The atomic primitive ZFS relies on is NOT raw sector atomicity.** ZFS relies on: (a) SHA-256 checksumming each uberblock so a torn write is detected, (b) writing redundant copies in a known order separated by a flush, so at least one valid copy survives. The `AWUPF` (Atomic Write Unit Powerfail) parameter on modern consumer NVMe drives is **0** — no multi-LBA write is guaranteed atomic across power loss. Designing on "atomic 4 KiB write" is unsound.

**Btrfs — three superblock copies, written simultaneously:**

**Critical correction to PM THINK v1:** Btrfs has **three** superblock copies at fixed offsets — `0x10000` (64 KiB), `0x4000000` (64 MiB), and `0x4000000000` (256 GiB, present only on devices ≥256 GiB) — **not two**. And they are written **simultaneously**, not alternately. Per the Btrfs on-disk format spec, each superblock contains a checksum covering bytes `0x20` through `0x1000` (format: algorithm-specific, defaulting to crc32c). Write ordering: all data + metadata extents flushed → flush barrier → write all three superblock copies → flush barrier. "A single superblock write switches the entire filesystem from one consistent state to the next" (Internals-for-Interns, corroborating the spec).

**HAMMER FS (DragonFly BSD) — hybrid CoW + UNDO/REDO FIFO log:**

HAMMER uses CoW for steady-state operation plus an explicit UNDO/REDO FIFO log in dedicated reserved space. REDO entries record writes, extends, and txid updates for in-progress transactions. Operational claim from DragonFlyBSD.org: "HAMMER file systems are immediately available after a crash. There is no fsck." The hybrid adds an intent-log for the write window, combining CoW's natural consistency with journaling's bounded recovery time. This is worth reading (Dillon 2008 hammer.pdf) if NeoFS wants bounded recovery time beyond what pure CoW provides.

### 4.2 Inode-tree update ordering and crash safety

The ordering analysis for a CoW CAS write: (1) write new content block → (2) write new inode → (3) write new tree-root → (4) commit superblock. The crash analysis for each prefix:

- **Crash after (1) only:** new block is orphaned — no inode references it. At next GC, the block is reclaimed. Safe.
- **Crash after (2) only:** new inode written, old superblock active. Old superblock still points to old inode tree; new inode is unreferenced. At next GC, orphaned. Safe.
- **Crash after (3) only:** new tree-root written but superblock not yet updated. Old superblock still valid; new tree-root is unreferenced. Safe.
- **Crash during (4) [superblock write itself]:** Detected via checksum on the new superblock copy. Old superblock copy (written in the prior transaction) remains valid and is chosen at mount. Safe — provided the superblock write is checksummed and at least one redundant copy survives.

**The dangerous case (Btrfs, confirmed):** if the new tree-root references blocks that were not yet flushed to durable storage before the superblock write committed, those blocks may be silently missing after a crash. Mitigation: **flush all data + metadata blocks before writing the tree-root, and flush again before writing the superblock**. This is the two-flush protocol from §4.3.

**Claim: CoW + content-addressing makes NeoFS fsck-free by construction**, stronger than ZFS, because content-addressing adds a second integrity layer: every data block's address IS its hash, so a torn write to a content block produces a hash mismatch on read and is self-detecting without a separate checksum field.

### 4.3 Write barriers and storage commands

**Linux block layer primitives:**
- `REQ_PREFLUSH` — flushes the device's volatile write cache before the I/O starts.
- `REQ_FUA` (Force Unit Access) — I/O completion signaled only after data reaches non-volatile storage. Single-round-trip durability for that write.
- The block layer translates these to: SCSI `SYNCHRONIZE CACHE` / ATA `FLUSH CACHE` (0xE7) / ATA `FLUSH CACHE EXT` (0xEA) / NVMe `Flush`.

**NVMe FLUSH (spec verbatim):** *"If a volatile write cache is enabled, then the Flush command shall commit data and metadata associated with the specified namespace(s) to non-volatile media."* The "shall" is the conformance guarantee.

**virtio-blk FLUSH (OASIS virtio v1.2):** `VIRTIO_BLK_F_FLUSH` is feature bit 9. If the guest does **not** negotiate `VIRTIO_BLK_F_WCE`, the host operates in **write-through mode** — no volatile cache, correctness guaranteed without flush. This provides a **graceful BUILD-1 path**: ship without flush implementation; virtio-blk write-through gives correctness at a perf cost. Add FLUSH in BUILD-2 to recover the performance.

**The NeoFS CoW commit sequence:**
1. Write all new content blocks to free regions → wait for IRQ completions.
2. Issue `FLUSH` → wait for flush IRQ.
3. Write new superblock (with redundant copies + checksum + incremented generation).
4. Wait for superblock write IRQ.
5. Issue `FLUSH` → wait for flush IRQ.

Two flushes per commit transaction is the unavoidable correctness cost. All production CoW filesystems (ZFS, Btrfs, HAMMER) pay this.

---

## 5. Q3 — Refcount vs Mark-Sweep GC

### 5.1 IPFS mark-sweep (the simpler starting point)

IPFS uses mark-sweep GC with the following characteristics (from kubo `gc/gc.go` and IPFS documentation):
- **Mark phase:** colored set = recursive pins + their DAG descendants + direct pins + best-effort roots + internal pinner blocks. Computed by walking the DAG from all roots.
- **Sweep phase:** `bs.AllKeysChan()` — iterate every CID in the blockstore; delete any not in the colored set.
- **Trigger:** manual (`ipfs repo gc`), or automatic when daemon launched with `--enable-gc`. Auto defaults: **GCPeriod = 1 hour**, **StorageGCWatermark = 90%** of StorageMax.
- **Locking:** acquires `bs.GCLock(ctx)` for the entire GC cycle. **All writes are blocked during the GC scan.** For a large blockstore (millions of CIDs), this is multi-minute.
- **Per-write overhead: zero.** No refcount maintained; no DDT lookup.

### 5.2 ZFS DDT refcount (operational cost at scale)

ZFS maintains per-block refcounts in `ddt_phys_t.ddp_refcnt` (uint64). Each DDT entry records the checksum → physical DVA + refcount. Changes are txg-batched. `ddt_phys_decref()` decrements; when zero, `ddt_phys_free()` clears the dedup bit and the underlying block is freed at txg commit.

**Operational cost (primary sources):**
- In-RAM per-entry: ~216 B (fast_dedup, OpenZFS 2.3+) to ~424 B (traditional)
- On-disk per-entry: 300–900 B
- RAM budget: 1–3 GiB per TB of dedup data (when dedup ratio ≥3×)
- Per-write cost: DDT lookup (RMW) on every write; CPU-intensive even on reads (integrity check)
- Scrub cost: can saturate 8–32 CPU cores on a pool-wide scrub

**Industry verdict (despairlabs 2024, verbatim):** "OpenZFS deduplication is good now and you shouldn't use it." The 2024 fast_dedup feature reduced the worst memory pathology but the per-I/O CPU cost and I/O amplification remain prohibitive for most workloads. iXsystems guidance: "do not even consider dedup unless the dedup ratio is expected to be greater than 5×."

### 5.3 Venti — no GC at the storage layer

Venti blocks are never freed. The write-once model is an architectural decision: "archival" means storage grows monotonically. GC at the Fossil layer (deciding which snapshots to keep) is the client's concern. NeoFS as a general-purpose mutable FS cannot adopt this model.

### 5.4 Crash behavior of refcounts

In a **txg-atomic design** (ZFS model): refcount update and block free are grouped in the same txg. A crash rolls back to the prior uberblock, which references the prior DDT state. Neither "decrement without free" nor "free without decrement" can occur.

In a **non-transactional refcount design**: the prompt's analysis holds — the failure mode is "silent leak" (refcount not decremented before crash; block stays allocated), not "false free" (block freed before all references gone). Silent-leak is safe; false-free is corruption. Conclusion: a non-transactional refcount that fails toward leak, not free, is crash-safe in the correctness sense.

### 5.5 Recommendation for NeoFS GC

**Phase 3 BUILD-2 (when CAS dedup lands): start with mark-sweep.** Zero per-write overhead, crash-safe by construction (stateless function of the live inode forest at scan time), ~100 LOC implementation. Trigger: idle/manual + 90% watermark + periodic (configurable, default daily).

**Phase 3 BUILD-3 (only if mark-sweep pause latency is measured as a problem):** add refcount as an advisory hint. Treat refcount as a fast-free candidate: if refcount says zero, the block is immediately freed without waiting for the next GC cycle. Mark-sweep remains the authoritative source of truth; a periodic GC reconciles any refcount drift. This avoids the false-free class entirely while recovering the immediacy of refcount.

**Full ZFS-DDT-style real-time refcount:** file as Phase 5+ forward marker, gated on "measured dedup ratio >5× AND available RAM budget for DDT."

---

## 6. Q4 — Plan 9 Venti as Closest Architectural Analog

### 6.1 Venti architecture

**Author correction: the paper is by Sean Quinlan and Sean Dorward** — not Quinlan & Mullender as PM THINK v1 implied. Source: USENIX FAST 2002 abstract page confirmation; Wikipedia Venti entry.

**Score** = SHA-1 hash (20 bytes / 160 bits) of a block's content. The score is both address and integrity check — a read-back of wrong content produces a score mismatch, which is self-detecting. Venti does not address SHA-1 collision probability; the Foundation paper (Russ Cox et al.) later introduced compare-by-value mode precisely to not rely on SHA-1 collision-resistance.

**Arena** = an append-only log segment containing: a header (arena identity), a sequence of compressed data blocks (variable-sized, up to ~52–56 KiB post-compression), a directory (replicating block headers and offsets), and a trailer summarizing arena state. Arenas are sealed when full and permanently fingerprinted. **Blocks cannot be deleted.**

**Block sizes:** variable 512 B – 52 KiB. Fossil's default data-block size over Venti was 8 KiB.

**Index structure:** a disk-resident hash table distributed across dedicated index disks. Buckets are single disk blocks; entries within a bucket are sorted by score and found by binary search. An in-memory bloom filter fronts the table for negative-lookup short-circuits. Design target: one disk seek per lookup.

### 6.2 Fossil + Venti two-tier composition

Fossil (the mutable active FS) uses a **dual-addressing scheme**: local scores (16 zero bytes + 4-byte local block number) for mutable local-disk blocks, and Venti scores (true SHA-1 hashes) for archived immutable blocks. A Fossil pointer block can contain either type, traversed uniformly.

A background **archiver process** walks archival snapshots bottom-up: writes each block to Venti, receives its score, rewrites the parent pointer from local address to Venti score, archives the parent. This bottom-up tree archival ensures that when a node is archived, all its children are already in Venti.

Fossil maintains an in-memory block cache (LRU with dirty blocks capped at half the cache) for both local-disk and Venti blocks. Writes are continuous to local disk; archival to Venti is a separate periodic process.

### 6.3 Venti post-mortem and what went wrong

**9front's verdict (Plan 9's active fork, FQA §7):** "fossil never attained the level of maturity, reliability, or polish of the older filesystem [Kfs]... fossil is large and complicated. It uses soft-updates for crash safety, an approach that has worked poorly in practice for the BSD filesystems. Fossil also has a history (and present) of deadlocks and crashes due to its inherent complexity."

9front has **abandoned Fossil** in favor of cwfs (Ken Thompson's original userspace FS server, simple but not crash-safe), hjfs (experimental single-partition), and gefs (Ori Bernstein's new CoW FS) — all single-tier, without a Venti backend.

**The SHA-1 risk:** Valerie Henson's 2003 "Analysis of Compare-by-hash" argued that CAS systems should not assume collision-resistance. The 2017 SHAttered attack against full SHA-1 vindicated the concern. The Foundation paper (Russ Cox et al.) explicitly introduced compare-by-value mode — not trusting SHA-1 alone — as a direct post-mortem response to Venti's pure compare-by-hash assumption.

**Operational complexity:** the two-tier model required understanding two on-disk formats, two recovery models, two daemons, and a non-trivial dual-addressing scheme in the active layer. The 9fans community repeatedly identifies this as the dominant operational pain. The Plan 9 community's successor choice — single-tier CoW FS — is the architectural verdict.

### 6.4 Venti vs git object model — for NeoFS

| Dimension | Venti | Git | NeoFS implication |
|---|---|---|---|
| **Granularity** | 512 B – 52 KiB blocks (~8 KiB typical) | Whole-object per file | Block granularity needed for partial file reads/writes; git's whole-object model forces re-hashing on small edits |
| **Object types** | 8-bit untyped label (server-opaque) | Typed: blob/tree/commit/tag | NeoFS needs typed-at-storage per Bible 10.3; git's model is closer than Venti's |
| **Index** | Bloom + disk hashtable + binary search | Packfile fanout + sorted offset table | Both are content-hash indexed; NeoFS BUILD-2 should follow Venti's bloom-filter approach |
| **Deletion** | None (WORM) | `git gc` from ref roots | NeoFS needs GC; WORM is a non-starter for a general-purpose FS |
| **Mutability boundary** | Fossil (mutable) above Venti (immutable) | Refs (mutable) above objects (immutable) | NeoFS must decide: two-tier or single-tier |

**Critical observation for NeoFS:** both Venti and git are *single-purpose* CAS systems (archival storage; version control). NeoFS is attempting something neither achieved: general-purpose mutable POSIX + content-addressing. Fossil+Venti tried the two-tier path for 15 years and the community chose single-tier as the successor.

### 6.5 Two-tier vs single-tier for NeoFS — verdict

**Single-tier CoW is the recommendation**, based on the Fossil+Venti post-mortem and the Plan 9 community's own consensus.

What two-tier gives you: clean immutability boundary at the CAS layer; natural dedup at archival time; cheap snapshots; potential separation of live vs cold storage media. What it costs: two on-disk formats with independent recovery models; dual-addressing complexity in the active layer; GC at the active layer is now structurally hard (the CAS layer is WORM, so reclamation requires active-layer coordination); two daemons to operate. The 15-year Fossil+Venti experience produced deadlocks, crashes, and ultimately retirement.

**Single-tier CoW with optional CAS dedup as a phase-2 feature** gives all the structural wins (crash consistency, snapshots, no-fsck) with a single format, a single daemon, and no dual-addressing hack. This is the path ZFS, Btrfs, and GEFS each took independently. For Phase 3, NeoFS should be single-tier.

---

## 7. Q5 — Mount-Namespace + Typed-Inode Integration

### 7.1 Production systems with inode-level type discriminants

**BeOS / Haiku BFS** is the closest production prior art. BFS stores per-file extended attributes as kernel-level name/value pairs — for small attrs, inline in the inode; for large attrs, in a per-file B-tree. The `BEOS:TYPE` attribute (a MIME string like `application/x-person`) stores the content type. The kernel stores this attribute and provides indexed query over it. **Enforcement is advisory** — the Tracker (file manager) and Registrar (launch service) use the type for app assignment, but the kernel does not prevent execution based on content type. NeoPlutarch's Bible 10.3 goes further than BFS by placing the type in the inode discriminant field and having the kernel enforce execution semantics.

**seL4** maintains strict typed-object semantics for all kernel objects (TCB, Endpoint, Notification, CNode, PageDirectory, PageTable, Frame, Untyped). Every capability operation is type-checked. However, **seL4 has no "file object" type** — filesystems run entirely in userspace in seL4. The lesson seL4 transfers to NeoFS: typed objects with capability-mediated access work correctly at production scale; the discipline of "object type determines valid operations; capability rights further constrain which are permitted" is validated by seL4. The mechanism (inode discriminant + kernel VFS dispatch + capability rights on file-cap handles) is NeoFS's design to implement.

**Plan 9 9P Qid type bits:** every 9P file has a Qid with a type byte containing: `QTDIR` (0x80), `QTAPPEND` (0x40), `QTEXCL` (0x20), `QTMOUNT` (0x10), `QTAUTH` (0x08), `QTTMP` (0x04), `QTSYMLINK` (0x02), `QTLINK` (0x01), `QTFILE` (0x00). Type is server-asserted and travels with the file across the protocol/mount boundary.

**Critical clarification:** 9P Qid type does NOT prevent execution of regular files. Execution permission is purely the mode bits (`DMEXEC` etc.). `QTAPPEND` and `QTEXCL` enforce specific file-operation policies (append-only; exclusive-use), but there is no type bit for "not executable." The 9P model provides the *plumbing* (type-in-inode, type-travels-with-file) but not the *policy* (type → allowed operations). NeoFS must add the policy layer.

### 7.2 Capsicum rights restriction as per-fd capability

**Capsicum** (FreeBSD 10+) converts file descriptors into unforgeable capability tokens with a rights mask. Relevant rights for Bible 10.3:

- `CAP_MMAP_X` — mmap with `PROT_EXEC`
- `CAP_MMAP_RX` — mmap with `PROT_READ | PROT_EXEC`
- `CAP_MMAP_RWX` — mmap with all three

In capability mode (`cap_enter()`), global namespaces (filesystem paths, PIDs) become inaccessible. A process with only a `CAP_READ | CAP_MMAP_R` directory fd cannot escalate to a `CAP_MMAP_X` fd on a child file — rights propagate through `openat()` at or below the parent fd's rights.

**The key limitation for Bible 10.3:** Capsicum rights are **per-fd**, not per-inode. Capsicum says "this process in capability mode cannot mmap-exec this fd." It does not say "this file cannot be mmap-exec'd by anyone." The latter — **universal per-inode execution refusal based on content type** — is what Bible 10.3 requires. Capsicum provides the rights-restriction mechanism but not the inode-authority claim.

### 7.3 SELinux type enforcement — the closest production analog

SELinux file types (e.g., `bin_t`, `lib_t`, `httpd_sys_content_t`) are stored in the `security.selinux` xattr and read by the kernel on file lookup. The `selinux_file_mprotect()` and `selinux_mmap_file()` LSM hooks check `vma->vm_file`'s type against the calling process's domain.

A policy rule `neverallow * neofs_model_t:file { execute execmod };` would unconditionally prohibit any process from making an executable mapping of files typed `neofs_model_t`. This is functionally identical to NeoPlutarch's Bible 10.3 mandate.

**Structural difference:** SELinux types live in xattrs (advisory at the FS layer, enforced by LSM hook); NeoPlutarch's design places the type in the inode discriminant (the FS layer itself is the authority). The NeoFS approach is **architecturally cleaner**: there is no "what if the xattr is wrong or absent" question; the inode discriminant is the ground truth, not a separate attribute.

### 7.4 Foreign-FS type mapping

No production OS does extension-free, sidecar-free typed import from FAT32. macOS uses AppleDouble sidecar (`._<file>`) for FAT32 volumes. BeOS/Haiku use `mimeset` with filename-extension fallback. None of these approaches satisfies Bible 10.3's "not filename extensions" principle.

For NeoFS foreign-FS mounts (FAT32, ext4), the three viable options:

1. **Coerce-to-Regular (recommended for BUILD-1):** safest, loses no information (the foreign FS has no type metadata to lose), no operational complexity. Cost: Model files on USB sticks cannot roundtrip as Model without explicit rename/retype.
2. **Per-mount default type:** `mount -o default-type=Regular ...`. Simple, explicit, no surprises. Best for "this entire partition is one kind of thing."
3. **Sidecar manifest (`.neofs-types` at FS root):** preserves type mapping across roundtrips at the cost of sidecar-management discipline. File as Phase 3+ enhancement.

### 7.5 mmap refusal prior art

**OpenBSD W^X (6.0+):** `mmap()` with `PROT_WRITE | PROT_EXEC` returns `ENOTSUP`. Exception: per-filesystem `wxallowed` mount flag + per-binary `wxneeded` link tag. This is per-mount + per-binary enforcement, not per-inode-type enforcement, but the `wxallowed` mount flag is structurally adjacent to per-inode-type policy.

**SELinux:** the closest prior art to Bible 10.3, as described in §7.3 above.

**No production OS implements per-inode type-discriminant mmap refusal at the FS layer** in the way Bible 10.3 envisions. NeoPlutarch is past the production prior-art horizon. The mechanism is tractable (1-byte type enum in inode, VFS dispatch reads it before authorizing mmap, policy is a one-line kernel constant); the engineering risk is in the boundary cases (foreign-FS import, type extension versioning, retype operations, initial type assignment at file creation).

---

## 8. Q6 — Synchronous-Blocking I/O at the FS Layer

### 8.1 Pre-blk-mq synchronous I/O prior art

**FFS (McKusick, Joy, Leffler, Fabry, 1984) — the foundational model:**

Per the FFS paper: reads are synchronous (thread blocks until disk returns the block); writes are asynchronous-via-buffer-cache (write returns when block is in cache, kernel flushes later). This two-tier discipline — synchronous reads, async-write-back — is the original Unix model and is production-proven across 40+ years of FFS, ext2/3/4, and early Linux. **This is exactly the model NeoFS Phase 3 BUILD-1 should adopt.**

**Reframing:** NeoPlutarch's single-thread-per-process model (SC-4 D13) is not a constraint to work around — it is the classical Unix assumption. Every filesystem design prior to Linux 2.6 aio_read and 2019 io_uring assumed it. FFS, Plan 9, xv6, and seL4 userspace filesystems all assume a process-level blocking model. NeoFS gains simplicity by embracing it.

### 8.2 IRQ → DMA completion → thread wake discipline

**The lost-wakeup problem (canonical statement, MIT 6.828):** if the issuing thread releases its condition lock before calling `sleep()`, a storage IRQ can fire between the release and the sleep, causing the thread to sleep forever with nothing to wake it up.

**The canonical solutions:**

**Plan 9 rendezvous** (Pike, Presotto, Thompson — verified from primary paper): disable interrupts first (`inhibit()`), then acquire the rendezvous lock. Both `sleep()` and `wakeup()` follow this sequence. Verbatim invariant: *"the manipulation of the rendezvous structure is all done under the lock, and wakeup only examines it under lock."* SMP note from the paper: *"the lock must be set only with interrupts disabled on the current processor, so that if the interrupt comes during sleep it will occur only on a different processor; if it occurred on the processor executing sleep, the spin lock in wakeup would hang forever."*

**Linux completion variables** (from kernel.org docs): `complete()` is IRQ-safe — it uses `spin_lock_irqsave()` internally. The `done` unsigned int counter solves the lost-wakeup problem: if `complete()` fires before `wait_for_completion()`, the non-zero `done` count is observed immediately.

**xv6 `sleep`/`wakeup`** (simplest correct implementation): `sleep(chan, lock)` atomically releases the condition lock and acquires `ptable.lock` before marking the process SLEEPING. The dual-lock discipline ensures no wakeup can be missed between the condition-check and the sleep.

**Composability with NeoPlutarch's SC-3B/SC-3C discipline:**

If a block I/O is issued from CPU A and the storage IRQ fires on CPU B (which is typical — APIC can route IRQs to any CPU), the IRQ handler on CPU B must wake the thread that is blocked on CPU A's per-CPU scheduler. This is **exactly the cross-CPU IPI wake problem** that SC-3C's IPI infrastructure was built for.

**NeoFS should be the first non-scheduler-internal client of the SC-3C IPI cross-CPU-wake primitive.** It should not introduce a parallel wake mechanism. The storage-IRQ handler on CPU B sends an IPI to CPU A to mark the waiting thread runnable — using the same cross-CPU-wake primitive the scheduler uses for preemption and thread migration.

The per-I/O rendezvous structure: a `SwitchMutex`-protected struct with a `done: AtomicBool` and a `waiting_thread: Option<Arc<Thread>>`. Both the issuing process context and the storage IRQ handler acquire the SwitchMutex at IF=0, following SC-3B discipline exactly.

### 8.3 Background FS work and the kthread prerequisite

NeoFS BUILD-1 (synchronous I/O, no buffer cache, no GC) needs no background kernel threads. All FS work runs in the context of the syscalling process.

NeoFS BUILD-2 (buffer cache write-back, GC sweep, periodic FLUSH) needs at least one background kernel thread per FS instance (or per CPU, following the SC-3C per-CPU model). This requires a **kthread primitive** in NeoPlutarch's kernel. If NeoPlutarch does not have a kthread primitive before Phase 3 BUILD-2, it must be added as a Phase 3 BUILD-1 or BUILD-2 prerequisite. This is a dependency PM should check against the current Phase 3 substrate.

---

## 9. NeoFS vs Prior-Art Alignment Summary

| Design dimension | PM lean | Prior-art alignment | Verdict |
|---|---|---|---|
| Block size (Q1.a) | 4 KiB fixed | Btrfs 4 KiB sector; ZFS 128 KiB default; Venti 8 KiB | **Confirmed with extension:** make mkfs-configurable (default 4 KiB); document index-pressure trade-off |
| Hash indirection (Q1.b) | Two-level inode→hash→block | Venti bloom+hashtable; ZFS DDT ZAP | **Confirmed.** Venti's bloom+disk-hashtable model is lowest-overhead for BUILD-2 |
| Crash consistency (Q2) | CoW + atomic superblock swap | ZFS, Btrfs, HAMMER all converge here | **Confirmed with implementation reframe:** "atomic" = checksum + redundant copies + flush barrier, NOT raw sector atomicity |
| Inode-tree ordering (Q2.b) | Implicit | ZFS txg; Btrfs generation numbers; HAMMER UNDO | **Extension:** flush all data before tree-root write; generation numbers on all tree nodes |
| GC strategy (Q3) | Refcount + periodic GC | ZFS DDT expensive; IPFS mark-sweep simpler | **Reframe:** start with mark-sweep; add refcount as advisory hint only if GC pause is measured as a problem |
| Two-tier vs single-tier (Q4.5) | Not yet decided | Plan 9 community converged on single-tier | **Single-tier.** Fossil+Venti post-mortem is unambiguous |
| Typed inodes (Q5.1) | Inode discriminant + kernel enforcement | BFS (advisory), 9P (no exec restriction), SELinux (closest outcome) | **Confirmed as novel.** Past production prior-art horizon; mechanism is tractable; boundary cases need explicit design |
| Synchronous I/O (Q6) | Single-thread-per-process | FFS, Plan 9, xv6 all assume it | **Confirmed.** Original Unix model; not a constraint, an assumption |
| Content-addressing in BUILD-1 | Eager CAS | Venti history; ZFS dedup regret | **Reframe: defer to BUILD-2.** Single most important reframing in this dispatch |

---

## 10. Forward Markers

Items the research surfaces for Phase 3 BUILD-2+ and beyond:

1. **CAS dedup as BUILD-2 feature, not BUILD-1 structural primitive.** Gate: measurable dedup ratio on actual workload. When shipped, use Venti-style bloom-filter + disk-hashtable index. Start with mark-sweep GC; add refcount hint if pause latency is measured as a problem.

2. **virtio-blk VIRTIO_BLK_F_FLUSH feature negotiation (BUILD-2).** BUILD-1 relies on write-through mode (correct, slightly slow). BUILD-2 negotiates FLUSH and implements the two-flush CoW commit sequence for full performance.

3. **kthread primitive prerequisite for BUILD-2.** NeoFS BUILD-2 needs a background kernel thread for buffer-cache write-back and GC. Audit whether NeoPlutarch has a kthread primitive before Phase 3 BUILD-2 dispatch.

4. **Block-size as mkfs parameter (BUILD-1 planning).** Represent block size as a superblock field, not a compile-time constant. Reserve 16/64/256 KiB as valid alternatives for model-storage workloads.

5. **Incremental mark-sweep (BUILD-3+).** IPFS's full-blockstore scan under GCLock blocks writers for multi-minutes at large scales. Incremental GC (tri-color marking, similar to Btrfs's incremental balance) avoids the pause at the cost of implementation complexity. Defer to BUILD-3 behind workload measurement.

6. **HAMMER-style UNDO/REDO FIFO log (BUILD-3+ optional).** Pure CoW with two-flush commit gives no-fsck correctness. HAMMER adds an intent log for bounded recovery time and `fsync` performance improvement. If NeoFS's workload shows `fsync`-heavy applications (databases), HAMMER's hybrid is the right upgrade path. Read Dillon's 2008 hammer.pdf before committing.

7. **Type extension versioning in inode header (BUILD-1 planning).** The inode type field should include a reserved-bits range for future type additions. A `type_version` field in the superblock documents the highest type ID in use. This prevents forward-compatibility breaks when `Model`, `Tensor`, `InferenceContext` are added in Phase 5+.

8. **Sidecar manifest for foreign-FS type import (BUILD-2+).** A `.neofs-types` file at the root of a mounted FAT32/ext4 volume provides per-path type overrides, enabling Model files to roundtrip across foreign FS without filename extensions. Intentionally deferred from BUILD-1.

9. **Retype operation discipline (BUILD-2).** Changing an inode's type (e.g., converting a Regular file to a Model file after validation) requires a privileged kernel operation. This should follow seL4's Untyped→Typed retype discipline: the old type's capabilities are revoked, the new type's capabilities are issued. Design before implementing the type discriminant.

10. **Full ZFS-DDT-style real-time refcount (Phase 5+).** Gated on measured dedup ratio >5× AND confirmed RAM budget for DDT at expected process/model count. If Phase 5 model dedup produces these ratios (possible for identical base models with different fine-tunes), the DDT investment earns its keep.

---

## 11. Anti-Scope Confirmation

Items from Phase 3 THINK v1 §6 that the literature confirms are the right deferrals:

- **AI model content-addressed storage (HF Xet / ZipLLM / Ollama):** confirmed out of scope for Phase 3. Phase 5 consumes Phase 3's CAS substrate; it doesn't retrofit it.
- **POSIX-strict (versus POSIX-ish) semantics:** confirmed deferrable. `O_DIRECT`, POSIX advisory locks, mandatory locks, extended POSIX ACLs are not needed for Phase 3's workload. Each can be added as a discrete feature.
- **RAID / volume management at the FS layer:** confirmed out of scope. Single-disk only. ZFS's RAIDZ + dedup combination is what made ZFS dedup unrecoverable when things went wrong.
- **Multi-threaded FS clients:** SC-4 D13 single-thread-per-process is the classical Unix model and simplifies the FS state machine. Confirmed deferrable.
- **Page cache write-back in BUILD-1:** confirmed deferrable. Synchronous writes to storage in BUILD-1; add buffer cache in BUILD-2 driven by measured I/O performance.
- **xattrs / POSIX ACLs / quotas in BUILD-1:** confirmed out of scope for Phase 3 BUILD-1.
- **USB mass storage, eMMC/SD drivers:** confirmed as Phase 4+ per Bible 9.9. virtio-blk + SATA AHCI + NVMe are the Phase 3 required set.

---

## 12. Citation Precision Corrections to PM THINK v1

**Five corrections to PM's Phase 3 THINK v1 characterizations:**

1. **Venti paper authorship.** PM THINK v1 §4 Q5.b referenced "Quinlan & Mullender." The paper is by Sean Quinlan and Sean Dorward. **Correction: Quinlan & Dorward, USENIX FAST 2002.** Source: USENIX abstract page, Wikipedia Venti entry.

2. **Btrfs superblock count and write discipline.** PM THINK v1 §5.d characterized Btrfs as "two superblocks at fixed offsets (0x10000 and 0x4000000), alternately written." The on-disk format specification shows **three** superblock copies (0x10000 / 0x4000000 / 0x4000000000) written **simultaneously** (not alternately). The 256 GiB copy is present only on devices ≥256 GiB. **Correction: three copies, simultaneous write.** Source: `https://btrfs.readthedocs.io/en/latest/dev/On-disk-format.html`.

3. **"Atomic superblock swap" is not raw sector atomicity.** PM THINK v1 used "atomic superblock swap" as the correctness primitive. Consumer NVMe drives advertise AWUPF=0; no multi-LBA write is guaranteed atomic across power loss. What production CoW filesystems actually rely on is: checksum + redundant copies + flush barrier. **Correction: restate as "checksummed redundant-superblock rotation with write barrier."** The implementation directive must specify multiple superblock copies and a FLUSH between content-block writes and superblock writes. Source: `https://lwn.net/Articles/933015/`.

4. **ZFS dedup operational cost.** PM THINK v1 §5.e did not flag the operational cost of refcounted dedup. ZFS DDT requires 216–424 bytes RAM per unique block entry; 1–3 GiB RAM per TB of deduplicated data; CPU cost on every read and write. The 2024 OpenZFS maintainer assessment is "good now, don't use it." **PM should not treat refcount-based dedup as a lightweight feature.** Source: `https://despairlabs.com/blog/posts/2024-10-27-openzfs-dedup-is-good-dont-use-it/`.

5. **9P Qid type does not prevent execution.** PM THINK v1 §4 Q5.e cited Plan 9 9P as a typed-inode prior art implying execution restrictions. 9P Qid type bits (QTDIR, QTAPPEND, QTEXCL, etc.) do not prevent execution of regular files — execution permission is entirely the Unix mode bits. **Correction: 9P provides type-in-inode plumbing but not type-based execution policy.** SELinux is the closest prior art for type-based mmap refusal, not 9P. Source: 9P2000 RFC, Plan 9 stat(5) man page.

---

## 13. Recommended D-Decision Resolutions

**D5 (Q5.a — content-addressing in BUILD-1): DEFER TO BUILD-2.**

This is the single most important recommendation. NeoFS BUILD-1 should ship with physical block addresses in inodes (like ext2). The Merkle-DAG structure (inode contains hashes of its data blocks; directory contains hashes of child inodes; superblock contains the root inode address) provides crash consistency, snapshots, and fsck-free correctness without requiring hash-as-address for block lookup. Content-addressing as a dedup mechanism is BUILD-2 scope.

Rationale: ZFS dedup is the most cautionary prior art. Plan 9's Fossil+Venti sequencing (mutable layer first, CAS archival second) is the closest successful precedent. The complexity of eager CAS compounds into every subsequent BUILD that depends on it. Isolating CAS to a discrete BUILD-2 milestone gives PM a measurable gate (dedup ratio on actual workload) before committing to the DDT/GC complexity.

**D3 (Q5.d — crash consistency): CoW + checksummed redundant-superblock rotation + FLUSH barrier. Confirmed.**

Specifically: (a) at least two superblock locations on disk at known offsets, (b) BLAKE3 (or crc32c for speed in BUILD-1) checksum over each superblock, (c) generation counter, (d) flush all content blocks before writing tree-root, (e) FLUSH between tree-root write and superblock write, (f) FLUSH after superblock write. No journaling, no intent log in BUILD-1.

**D5.e (Q5.e — GC in BUILD-2): mark-sweep first, refcount as advisory hint in BUILD-3 if needed.**

**D4.5 (two-tier vs single-tier): single-tier CoW.**

**D5.a (block size): mkfs-configurable, default 4 KiB, support 16/64/256 KiB.**

**D5.b (inode-to-content mapping): Merkle-tree of content hashes in inode, as PM leaned.**

For BUILD-1 specifically: the "content hash" in the inode is the hash of the block's content, computed at write time and stored for integrity verification on read. The hash is NOT used as the physical block address in BUILD-1 — the physical address is stored alongside it. In BUILD-2, the transition is: remove the physical address field; the hash IS the address (looked up via the content-hash index). The on-disk inode format should anticipate this by reserving both a hash field and a physical-address field from BUILD-1, so BUILD-2 is a semantic change, not a format migration.

---

## 14. Calibration Notes

**Verified primary sources.** All major system architectures addressed from primary sources: Venti from 9p.io HTML + cat-v.org mirror; ZFS from OpenZFS source (`ddt.c`, `spa_log_spacemap.c`) and documentation; Btrfs from on-disk format spec and mkfs.btrfs man page; IPFS from kubo `gc/gc.go` and IPFS documentation; virtio-blk from OASIS virtio v1.2 spec; Linux write barriers from `writeback_cache_control.txt`; Plan 9 sleep from the primary paper; 9P from RFC and stat(5) man page; Capsicum from FreeBSD man pages; SELinux from LWN; BFS from Wikipedia and birdhouse.org; git from Pro Git.

**Single-source findings:**
- HAMMER design paper (Dillon 2008) — PDF binary, not text-extractable. HAMMER confirmed via website and Wikipedia secondary sources. The no-fsck claim and UNDO/REDO log design are well-corroborated, but the specific log format details are single-sourced from secondary descriptions.
- ZFS uberblock ring detail (4 labels × 128 slots × 1KiB per slot; even-then-odd write order) — confirmed from FreeBSD ZFS Doxygen cross-reference, a secondary but technically authoritative source.
- FFS paper synchronous-read / async-write characterization — confirmed from search results; direct paper fetch corroborated. The exact verbatim quotes about synchronous reads come from search summaries rather than the raw PDF, but the characterization is consistent across multiple secondary sources.

**Author correction:** Venti paper is Quinlan & Dorward, not Quinlan & Mullender. Confirmed from the USENIX abstract page and Wikipedia.

**Time-budget actual:** ~2.5 hours (three parallel research agents, synthesis, artifact write-up). Within the 6–10 hour PM estimate; the three-agent parallelization reduced wall-clock substantially.

---

## 15. Cross-References

**This artifact consumes:**
- `Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` — the dispatch
- `Plans/PHASE-3-THINK-V1-2026-05-11.md` §3.1 (substrate), §3.2 (Bible bindings), §4 Q5.a-e (open questions), §6 (anti-scope)
- `docs/bible.md` Article 10 (NeoFS bindings) and Article 9.9 (storage device phase bindings)
- `docs/research/ALTIMIT-real-world-implementation.md` §D+§E — context only; not re-surveyed

**This artifact is consumed by:**
- Phase 3 THINK v2 (PM updates Q5.a-e with cited design choices and BUILD-1 vs BUILD-2 scoping)
- ADR-021 (Phase 3 design substrate — analogous to ADR-013 for Phase 2)
- Phase 3 BUILD-1 Jarvis dispatch (after THINK v2 and ADR-021 land)

**This artifact does NOT produce:**
- No Bible amendment
- No ANSPEC amendment
- No ADR (ADR-021 is PM's artifact after consuming this)
- No code changes

---

## 16. Closure

The "AI-native OS" filesystem design space has substantially more production prior art than the process-table SMP design space did — but the prior art's message is sobering rather than encouraging. Plan 9's Fossil+Venti produced the most architecturally elegant content-addressed filesystem ever shipped in production and the Plan 9 community replaced it with a single-tier CoW system. ZFS dedup pioneered real-time block-level deduplication in a production kernel and its maintainers' 2024 verdict is "don't use it." IPFS chose mark-sweep precisely to escape the ZFS DDT cost trap. The accumulated lesson is: separate the correctness properties (crash safety, no-fsck, snapshots) from the optimization properties (dedup, GC efficiency), ship correctness first, and add optimization only when workload measurement justifies the complexity.

NeoFS Phase 3 BUILD-1 should ship a Merkle-DAG-rooted-at-superblock single-tier CoW filesystem with physical block addresses, typed inode discriminants, synchronous-blocking I/O using SC-3C IPI cross-CPU wake, and virtio-blk write-through for correctness without flush implementation. Everything else — content-addressing, dedup, GC, buffer cache write-back, FLUSH negotiation, and sidecar type-mapping for foreign FSes — is BUILD-2+ scope with discrete measurement-gated rationale for when to ship each. That sequencing is the most important output of this dispatch.

---

**End of research artifact.**

*Defer content-addressing to BUILD-2 (single most important recommendation). CoW + checksummed-redundant-superblock-rotation + flush-barrier for crash consistency (Btrfs has three superblock copies, not two; "atomic" ≠ raw sector atomicity). Start GC with mark-sweep. Single-tier CoW per Plan 9 community's own Fossil+Venti post-mortem verdict. Bible 10.3 typed inodes are past production prior-art horizon — SELinux is closest in outcome, 9P Qid is closest in structure, Capsicum is closest in mechanism. NeoFS should be first non-scheduler client of SC-3C IPI cross-CPU-wake for I/O completion. Five PM THINK v1 corrections: Quinlan & Dorward (not Mullender); Btrfs has three superblocks written simultaneously (not two alternately); "atomic superblock" is checksum+redundant+flush-barrier; ZFS dedup is operationally expensive (don't start there); 9P Qid does not prevent execution.*
