# DISPATCH — Phase 3 NeoFS prior-art research (Cerebro #1)

**Date filed:** 2026-05-11
**From:** PM
**To:** Cerebro (Claude deep-research lane)
**Ferry:** PL ferries to Cerebro session on signal. Status: ready to ferry; awaiting PL authorization per Phase 3 THINK v1 §8.2.
**Anchors:** `Plans/PHASE-3-THINK-V1-2026-05-11.md` §7 (Cerebro dispatch candidate #1); anchors Q5.a through Q5.e (NeoFS native layer design).
**Type:** Deep-research prior-art survey. Comparable in scope and shape to `Plans/DISPATCH-2026-05-11-sc-4-d5-process-table-smp-prior-art-cerebro.md` (which returned `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` at 50.9 KB and reframed the entire SC-4 D4 question). Estimated 6-10 hours Cerebro session.

---

## 1. Context

NeoPlutarch's Phase 3 builds NeoFS — the native filesystem mandated by Bible Article 10.2 as **content-addressed at the block layer, with a POSIX-ish path namespace on top, with models deduplicated by content hash**. Phase 3 also builds the block layer + VFS that NeoFS sits on, plus FAT32 and ext4 read-only foreign-FS drivers.

The PM-tier design decisions for the NeoFS native layer are open and substantial. Phase 3 THINK v1 §4 Q5.a-e enumerates them:

- **Q5.a:** Content-addressed block layer mechanics (block size, hash algorithm, two-level indirection inode→hash→physical-block).
- **Q5.b:** Inode-to-content mapping (Merkle-tree structure, large-file handling).
- **Q5.c:** On-disk format (superblock, inode table, free space, format revision discipline).
- **Q5.d:** Crash consistency (CoW + atomic superblock swap vs journaling vs no-CC).
- **Q5.e:** Free space management (refcount + GC vs allocation bitmap; mark-sweep vs incremental).

Phase 3 THINK v1 records PM leans on each but explicitly flags them as **pre-Cerebro**. Several are technically dependent on prior-art that PM has not surveyed: the trade-offs between BLAKE3 block hashing and SHA-256 in production CAS systems, the actual atomicity invariants needed by CoW filesystems at the superblock level, the refcount-GC vs mark-sweep performance characteristics on mixed-workload corpora.

The SC-4 D5 precedent showed that Cerebro deep-research can reframe entire PM design questions (D4 went from "what lock type for PROCESSES?" to "should PROCESSES be on the hot path at all?"). PM expects similar reframing potential here — particularly around Q5.d (crash consistency) where the project's no-async-runtime + single-thread-per-process invariants may make different atomicity disciplines preferable than the obvious ZFS/Btrfs analogs.

## 2. What this dispatch is NOT asking

PM has already surveyed (or doesn't need surveyed):

- **AI model content-addressed storage** — covered by `docs/research/ALTIMIT-real-world-implementation.md` §D + §E (Hugging Face Xet, ZipLLM, Ollama). Phase 5 will consume Phase 3 substrate but Phase 3's job is FS-layer dedup, not model-storage-layer dedup. Cerebro should NOT re-survey HF Xet / ZipLLM / Ollama.
- **Hash algorithm choice** — BLAKE3 is decided per ANSPEC §8.2 + the Phase 3 THINK v1 §4 Q5.a alignment. Cerebro should NOT relitigate BLAKE3 vs alternatives.
- **POSIX-ish syscall surface** — well-understood; Phase 3 will choose; no Cerebro prior-art needed.
- **Foreign FS reading (FAT32, ext4)** — well-documented; reference implementations in Linux/BSD/Redox/FuseFS. Out of scope for this dispatch (handled by Cerebro dispatch #2 if needed).

## 3. Six research questions

### Question 1 — On-disk format for content-addressed FS

What does the literature on content-addressed filesystems show works (and what doesn't) for on-disk format? Specifically:

- **Block size + dedup-granularity trade-offs.** What block sizes do production CAS systems use? 4 KiB matches OS page size; 8/16/32 KiB blocks reduce metadata overhead but reduce dedup ratio for small variations. Plan 9 Venti uses 8 KiB. ZFS uses variable (record-size up to 1 MiB). What's the empirical sweet spot for mixed workloads (text files + binary blobs + AI model weights)?
- **Two-level indirection mechanics.** Inode → content-hash → physical-block address. How do production systems encode the content-hash table on disk? Hash table with bucket chaining? B-tree keyed by hash? Linear probe with rebalance? What are the access patterns and cache discipline?
- **Free-space tracking at the physical layer.** Once you have content-addressed dedup, "free" means "no content-hash references this block." How do systems track this efficiently?

PM wants concrete numbers + design patterns from real systems, not just abstract enumeration.

### Question 2 — Crash consistency for CoW content-addressed FS

The single biggest open Phase 3 design question. PM leans toward CoW + atomic superblock swap (ZFS/Btrfs pattern; natural fit for content-addressing because writes never overwrite existing content blocks). But the specific atomicity invariants needed are unclear to PM.

- **Superblock swap atomicity discipline.** ZFS uses uberblock rotation; Btrfs uses two superblocks at fixed offsets. What's the minimum-correct atomic-write primitive needed at the block device layer? Single-sector atomic write (assumed on modern storage)? Block device fsync semantics? Write barriers?
- **Inode-tree update atomicity.** When a write modifies a file: new content block written; new inode pointer written; superblock points to new inode tree root. What ordering discipline does the literature require? What goes wrong if any one of these is partially-flushed at crash time?
- **Journaling alternatives.** ext4 uses journaling; could NeoFS? PM's instinct is no (CoW + content-addressing makes journaling redundant), but what does the literature show about hybrid CoW-with-intent-log systems (e.g., HAMMER FS)?
- **Sync syscall semantics.** What durability guarantees do production CoW filesystems offer userspace? fsync semantics? Per-file vs whole-FS sync?

PM specifically wants prior art on the *minimum-correct* atomicity discipline; over-engineering on durability discipline is a real Phase 3 risk.

### Question 3 — Refcount-vs-mark-sweep GC for content-addressed blocks

With content-addressing + dedup, "delete file" means "decrement refcount on the content blocks the inode points to; free blocks where refcount hits zero." This is GC, not direct delete. Two strategies:

- **Refcount per content hash.** Decrement at unlink time; free immediately when refcount → 0. Tracking overhead is a small atomic counter per content block.
- **Mark-sweep periodic GC.** No refcounts maintained; periodically walk all live inodes, mark referenced content blocks, free unmarked. Lower steady-state overhead but periodic stop-the-world (or incremental discipline).

- **Production system choices.** What do IPFS, Plan 9 Venti, ZFS dedup actually do? What are the operational characteristics (steady-state CPU/memory overhead, mount-time cost, fragmentation)?
- **Hybrid strategies.** Generational GC? Bloom-filter-accelerated mark? What does the literature show works at typical filesystem scales (10K-10M files)?
- **Failure-mode discipline.** What happens to refcounts at crash time? Does mark-sweep need to be re-run after every uncontrolled-shutdown event? How do production systems handle this?

PM lean from THINK v1 §4 Q5.e was refcount + GC; Cerebro may surface that mark-sweep is the production-correct choice. Pre-dispatch PM honesty: PM doesn't know which.

### Question 4 — Plan 9 Venti as a closest-architectural-analog

Plan 9's Venti is the most architecturally-similar production system PM is aware of: archival content-addressed block store with hash-indexed blocks, used by Plan 9's Fossil filesystem as a backing store.

- **Venti's on-disk format and arena structure.** Specifically: how does Venti compose with Fossil's "active" filesystem? Could NeoPlutarch's NeoFS use a similar two-tier design (active filesystem on top of content-addressed block store)?
- **What did Venti get wrong that NeoFS should avoid?** Venti is dead (Plan 9 itself is moribund); the post-mortem literature on Venti's operational characteristics is informative.
- **Venti vs git object model.** Both are content-addressed block-storage systems with different primary use cases (FS-like vs version-control). What discipline differences are relevant for NeoFS's general-purpose-FS use case?

### Question 5 — Mount-namespace + typed-inode integration

Bible Article 10.3 mandates typed inodes — file types live in the inode, not the filename. This is a meaningful divergence from Linux/BSD VFS (where types live in inode mode bits but filename extension is the user-visible discriminant) and from Plan 9 (where everything is a file with no typing).

- **Prior art for inode-level file typing.** What production or research systems have implemented typed-inode discipline? How is it surfaced to userspace? How does it compose with mount-point namespace traversal (where a mounted FS may have its own type discipline)?
- **Capability-on-inode disciplines.** Where in the literature do FS designs incorporate capability-based access (vs the more common Unix-uid model)? seL4 has cap-on-file; Plan 9's 9P has per-fid permissions; Capsicum has rights restriction. What patterns are NeoPlutarch-portable?
- **Foreign-FS type mapping.** When mounting FAT32 or ext4 (which have no typed inodes), how should NeoPlutarch present them in the typed-inode namespace? Coerce everything to `Regular`? Per-mount type-mapping rule? What does the literature show?

### Question 6 — Synchronous-blocking I/O at the FS layer + composability with existing kernel discipline

NeoPlutarch's kernel is single-thread-per-process (SC-4 D13), has no async runtime, has IRQ-safe / preempt-safe locking discipline (SC-3B/SC-3C/SC-4 inheritance), has per-CPU schedulers with cross-CPU rendezvous (SC-3B). The block layer in Phase 3 will need to compose with all of this.

- **Synchronous block I/O in production OSes.** Linux uses blk-mq async; older Unixes used synchronous-blocking. What does the literature show about FS-layer correctness disciplines when I/O blocks the issuing thread?
- **Interrupt handler → completion → wake discipline.** Storage device DMA completes via interrupt; interrupt handler needs to wake the blocked issuer. NeoPlutarch's SC-3C demonstrates IPI-driven cross-CPU coordination is fine. Does the literature show patterns for FS-layer issuer/completer that compose with IRQ-safe locking?
- **Composability with single-thread-per-process invariant.** Multi-threaded user processes (Phase 2+ forward marker) will eventually share an FS workload. Are there single-thread-FS-design lessons in the literature that prepare the design for eventual multi-thread without retrofitting?

## 4. Deliverable shape

Match the precedent set by `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` (50.9 KB, 6 PM-question answers + structural reframing + PM-correction list + cross-validation of related decisions). For each of Questions 1-6:

- Specific citations from production systems + research literature.
- Concrete numerical/design data where applicable (block sizes, dedup ratios, atomicity primitive widths, etc.).
- PM-leaned-but-may-be-wrong flags called out explicitly; reframing welcomed if the right question is different than what PM asked.
- Cross-validation of related Phase 3 design choices where Cerebro sees risk.

Additional structural notes (optional but valuable, parallel to SC-4 D5):

- **NeoFS-vs-prior-art alignment summary.** Where does each Cerebro answer push NeoFS toward an existing system's design? Where does it suggest divergence?
- **Forward markers for Phase 3+.** What deferred work does the literature suggest Phase 3 BUILD-1 should NOT take on, that should be future-BUILD scope?
- **Anti-scope confirmation.** What Phase 3 anti-scope items (THINK v1 §6) does the literature confirm are the right deferrals?

## 5. Expected output anchors

After PM consumes the research:

- **Phase 3 THINK v2** updates Q5.a through Q5.e with cited design choices.
- **ADR-021 (or next-numbered) Phase 3 design substrate** — analogous to ADR-013 for Phase 2 BUILD-3. Will reference the Cerebro research artifact as canonical prior art.
- **Phase 3 BUILD-1 dispatch for Jarvis** — once THINK v2 and ADR-021 land.

If Cerebro returns reframing of the order of Q5.a-e, PM consumes the reframing and updates accordingly per the SC-4 D5 precedent.

## 6. Stop conditions

- Cerebro session exceeds 12 hours wall-clock → surface to PM for re-scoping rather than continuing.
- Cerebro identifies a category of prior art PM did not anticipate that warrants a separate dispatch → surface as a follow-up dispatch candidate.
- Cerebro identifies that PM's framing (any of Q1-Q6) is structurally wrong → reframe in the deliverable per SC-4 D5 precedent; do not silently answer the wrong question.

## 7. Cross-references

**Substrate Cerebro should consume:**
- `Plans/PHASE-3-THINK-V1-2026-05-11.md` §3.1 (substrate snapshot), §3.2 (Bible-binding requirements), §4 Q5.a-e (the questions this dispatch deepens), §6 (anti-scope), §7 (this dispatch's framing context).
- `docs/bible.md` Article 10 (4 sub-points; load-bearing for NeoFS scope).
- `docs/bible.md` Article 9.9 (storage device support; phase-binding).
- `docs/specs/neoplutarch-ai-subsystem-spec.md` §8.2 (Phase 5 deduplication consumer of Phase 3 substrate).
- `docs/research/ALTIMIT-real-world-implementation.md` §D + §E (HF Xet / ZipLLM / Ollama prior art — context-setting only; do NOT re-survey).

**Established Cerebro precedent:**
- `Plans/DISPATCH-2026-05-11-sc-4-d5-process-table-smp-prior-art-cerebro.md` — dispatch shape.
- `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` — deliverable shape (50.9 KB, 6 PM-question answers + structural reframing + PM-correction list + cross-validation).
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` — earlier Cerebro deep-research at 39.7 KB; comparable scope.

**Phase 3 dependencies:**
- ADR-013 (Phase 2 BUILD-3) — composes with; NeoFS will use the SMP / capability / IPC primitives ADR-013 ratified.
- ADR-016 §109 — IRQ-safe heap + scheduler decoupling A1 named Phase 3 implementation; Cerebro should be aware this composes with FS I/O paths.
- ANSPEC-001 — Phase 5 consumer; informs what Phase 3 NeoFS must expose (content-hash visibility per Q10.a in THINK v1).

---

**End of dispatch.**

*Phase 3 NeoFS prior-art research. 6 questions anchored on THINK v1 §4 Q5.a-e. Estimated 6-10 hour Cerebro session. Output: ~40-55 KB research artifact in shape of SC-4 D5 precedent. Anchors Phase 3 THINK v2 and ADR-021. Ready to ferry on PL signal.*
