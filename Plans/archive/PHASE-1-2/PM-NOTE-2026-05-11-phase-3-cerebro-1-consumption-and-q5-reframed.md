# PM-NOTE: Phase 3 Cerebro #1 consumption — Q5 reframed to single-tier physical-block-addressed CoW; BUILD count promoted to 5; five citation corrections accepted

| | |
|---|---|
| **Date** | 2026-05-11 (consumption of Cerebro #1 return) |
| **Author** | PM (NeoPlutarch Phase 3 cadence) |
| **Type** | PM consumption note (informational artifact; documents PM-tier decisions arising from research consumption) |
| **Consumes** | `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` (Cerebro #1 deep-research return, ~50 KB, ~2.5h actual budget vs 6-10h estimated) |
| **Updates** | `Plans/PHASE-3-THINK-V1-2026-05-11.md` §4 Q5.a-e framing + §5 BUILD scope split |
| **Anchors** | Phase 3 THINK v2 (forthcoming); ADR-021 Phase 3 design substrate (forthcoming); Cerebro #2 + #3 dispatch framing reassessment (this PM-NOTE §6) |
| **Precedent** | `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md` — same SC-4 D5 substrate-reframing Cerebro contribution shape |
| **Subsequent sharpening** | `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md` — codified cadence rule arising from §2 below (PL diagnostic post-this-PM-NOTE) |

---

## 1. Headline

Cerebro #1 surfaced an SC-4-D5-class substrate reframing on Q5 (NeoFS native CAS substrate). PM THINK v1 §4 Q5.a leaned "content-addressed at block layer with BLAKE3 4 KiB blocks" as Phase 3 substrate. **Cerebro's verdict: that's wrong-shaped. NeoFS's first shippable BUILD should be a single-tier CoW filesystem with physical block addresses; content-addressing as dedup is a discrete subsequent BUILD with measurement-gated entry.**

The Merkle-DAG tree structure (inode → hashes of data blocks; directory → hashes of child inodes; superblock → root inode) delivers all the structural wins — crash consistency, snapshots, fsck-free correctness, future send/receive replication — without hash-as-address. Hash-as-address is an optimization layer that buys dedup at the cost of an index, a GC mechanism, a tighter crash-recovery proof, and an entire family of operational concerns (DDT memory pressure, scrub cost, refcount-vs-mark-sweep trade-off, false-free risk).

Cerebro anchor evidence: three independently-converging production CAS systems. **Plan 9 Fossil+Venti community post-mortem** — two-tier abandoned for single-tier successors (GEFS, cwfs, hjfs). **ZFS dedup OpenZFS-maintainer 2024 verdict** — "good now, don't use it" (despairlabs / iXsystems / TrueNAS guidance consistent). **IPFS chose mark-sweep specifically to avoid ZFS DDT cost trap** (1-3 GiB RAM/TB; CPU on every read+write). The lesson is consistent across three systems built by independent teams in different decades: separate correctness (CoW + redundant superblocks + flush barriers) from optimization (CAS dedup + GC); ship correctness first; gate optimization on measured workload need.

This is the most important architectural change to NeoPlutarch Phase 3 since Phase 3 THINK v1 was filed.

---

## 2. Pattern recognition — SC-4 D5 again

This is the second consecutive Cerebro dispatch where the substantively decisive contribution was reframing PM's question rather than answering it.

Cadence note: the "Have I framed the right question?" pre-dispatch gate established post-SC-4 D5 did NOT prevent this recurrence. Probable reasons:

- The framing was deeply embedded in THINK v1 (Q5.a was the lead substrate question), so challenging it without prior-art lookup would have required PM to suspect a problem PM did not yet know about.
- Phase 3 substrate space is genuinely less mature in PM head-state than Phase 2 was. Phase 2 PM had implementation experience to draw on; Phase 3 PM does not.
- The wrong-shaped question is precisely the kind of thing only Cerebro-class deep prior-art survey catches — three production CAS systems' post-mortems point to one conclusion that no single-system reading would surface.

Operational implication for Phase 3+ THINK work: PM should explicitly enumerate "what production system would refuse this design and why" as a THINK-stage gate, not just "what production system has done this." Cerebro #2 + #3 framing reassessment (§6 below) applies this corrective.

Cerebro is, again, validated as the substrate safety-net. The two-for-two pattern is now load-bearing for project cadence — Cerebro substrate-survey dispatches should remain the first move on any new Phase substrate, not a reactive escalation.

**PL sharpening (filed post-PM-NOTE):** the §2 diagnostic above is correct but incomplete. The pattern's precise shape — PM operates at the *framing level*, Cerebro operates at the *question level*, and THINK v1 is written before those levels are aligned. The previous framing-gate's failure mode was asking "is my question well-formed?" (within-frame coherence check; proofreading) instead of "should I be asking a different question entirely?" (structural challenge; editing) — distinct mental motions. Fix: first Cerebro dispatch on new substrate is explicitly structural-challenge, not prior-art-anchoring. See `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md` for the codified cadence rule, retroactive applicability (Phase 3 Cerebro #1 was a lucky version of the right cadence), Cerebro #2/#3 on Phase 3 staying at question-level under the now-established frame, Phase 4/5 first-Cerebro prospective application (Phase 5 first-Cerebro is highest-stakes — explicit invitation to tear up ANSPEC-001 substrate at framing level), and Bible Article 14 amendment candidacy at Phase 4 close if pattern holds three-for-three.

---

## 3. D-decisions finalized per Cerebro #1

| D-decision | Pre-Cerebro PM lean | Post-Cerebro finalization | Source |
|---|---|---|---|
| **D5 (Q5.a content-addressing in first NeoFS BUILD)** | Eager CAS with BLAKE3 4 KiB blocks | **DEFER.** Single-tier physical-block-addressed CoW in first NeoFS BUILD; CAS as discrete subsequent BUILD with measurement gate | Cerebro §13 D5; §1 executive summary; §6.5 verdict |
| D3 (Q5.d crash consistency) | CoW + atomic superblock swap | **CONFIRMED with implementation reframe.** "Atomic" = checksummed redundant-superblock rotation + FLUSH barrier between content-block writes and superblock writes. Two FLUSHes per CoW transaction is unavoidable correctness cost. Generation counter on each superblock copy. Three superblock copies (Btrfs prior art) written simultaneously | Cerebro §4.1, §4.3; AWUPF=0 evidence; ZFS+Btrfs+HAMMER independent convergence |
| D-new (Q5.e GC strategy) | Refcount + periodic GC | **Mark-sweep first** (IPFS model, zero per-write overhead, ~100 LOC implementation, crash-safe by construction as stateless function of live inode forest). Refcount as advisory hint only if measured GC pause becomes a problem (BUILD-3+ scope). Full ZFS-DDT-style real-time refcount is Phase 5+ forward marker gated on measured dedup ratio >5× + RAM budget | Cerebro §5; ZFS DDT cost evidence (216-424 B/entry, 1-3 GiB/TB); IPFS kubo `gc/gc.go` reference |
| D-new (Q5 two-tier vs single-tier) | Not explicitly decided in THINK v1 | **Single-tier CoW.** Plan 9 community's own Fossil+Venti post-mortem is unambiguous: single-tier successor (GEFS) is the verdict. 15-year Fossil+Venti experience produced deadlocks, crashes, and ultimately retirement | Cerebro §6.5; 9front FQA §7 |
| D5.a (Q5.a block size) | 4 KiB fixed | **mkfs-configurable; default 4 KiB; support 16/64/256 KiB.** Document index-pressure trade-off (16 GiB index metadata at 4 KiB blocks / 1 TB / 50% dedup). Block size = superblock field, not compile-time constant | Cerebro §3.1; Btrfs 4 KiB sectorsize default; ZFS 128 KiB recordsize context |
| D5.b (Q5.b inode-to-content mapping) | Merkle-tree of content hashes in inode | **CONFIRMED with forward-compat hint.** First NeoFS BUILD: inode contains content hash (integrity verification on read) + physical block address (lookup). CAS BUILD: drop physical address; hash becomes the address via content-hash index. **Forward-compat hint:** reserve both hash and physical-address fields in inode format from first NeoFS BUILD; CAS BUILD is a semantic change, not a format migration | Cerebro §13 D5.b |
| D6 (Q6 synchronous I/O) | Single-thread-per-process synchronous-blocking I/O | **CONFIRMED as classical Unix model**, not a constraint. FFS (1984), Plan 9, xv6 all assumed it. **NeoFS becomes first non-scheduler-internal client of SC-3C IPI cross-CPU-wake primitive**; no parallel wake mechanism introduced | Cerebro §8.1, §8.2; FFS primary (McKusick et al. 1984); Plan 9 rendezvous primary (Pike, Presotto, Thompson); xv6 sleep/wakeup |
| D5.c (Q5.c on-disk format with revision number) | Revision number + Merkle-DAG-rooted-at-superblock | **CONFIRMED with correction.** Three superblock copies (not two) at fixed offsets; each checksummed; written simultaneously with FLUSH barriers; mount-time selection by highest generation counter with valid checksum | Cerebro §4.1, §12 correction 2; Btrfs on-disk format spec |

---

## 4. Five citation corrections to PM THINK v1 (accepted)

Cerebro flagged five citation-precision errors in PM THINK v1 §4-5. All accepted; THINK v2 will carry the corrections:

1. **Venti authorship: Quinlan & Dorward**, not Quinlan & Mullender. USENIX FAST 2002. PM THINK v1 §4 Q5.b mis-attributed. (Source: USENIX abstract page; Wikipedia Venti entry.)

2. **Btrfs superblock count: three copies, written simultaneously**, not two written alternately. Offsets `0x10000` (64 KiB), `0x4000000` (64 MiB), `0x4000000000` (256 GiB; present only on devices ≥256 GiB). On-disk format spec verified. PM THINK v1 §5.d mis-characterized. (Source: `https://btrfs.readthedocs.io/en/latest/dev/On-disk-format.html`.)

3. **"Atomic superblock swap" is not raw sector atomicity.** Consumer NVMe AWUPF=0; no multi-LBA atomic write guaranteed across power loss. Production CoW filesystems achieve "atomic" via checksum + redundant copies + flush barrier. PM THINK v1 §5.d implied hardware atomicity as the correctness primitive. (Source: LWN "Atomic block-write operations" `https://lwn.net/Articles/933015/`.)

4. **ZFS dedup operational cost flagged.** PM THINK v1 §5.e did not surface that refcounted CAS dedup requires 216-424 B RAM per entry; 1-3 GiB RAM per TB of deduped data; CPU on every read+write; 2024 OpenZFS maintainer verdict is "don't use it." This evidence is the substantive support for the D-new mark-sweep-first decision. (Source: despairlabs 2024 + TrueNAS ZFS Deduplication guide + Klara Systems Fast Dedup analysis.)

5. **9P Qid type does NOT prevent execution.** PM THINK v1 §4 Q5.e cited Plan 9 9P as typed-inode-with-execution-restriction prior art. 9P Qid type bits (QTDIR, QTAPPEND, QTEXCL, QTMOUNT, QTAUTH, QTTMP, QTSYMLINK, QTLINK, QTFILE) do not restrict execution — Unix mode bits do. **SELinux is the closest prior art for type-based mmap refusal, not 9P.** Capsicum is closest in mechanism (per-fd CAP_MMAP_X rights); 9P is closest in on-disk structure (type-in-inode plumbing). Bible 10.3 synthesizes SELinux outcome + Capsicum mechanism + 9P structure into something genuinely novel. (Source: 9P2000 RFC; Plan 9 stat(5) man page; LWN SELinux mmap controls.)

PM THINK v2 carries all five corrections; ADR-021 cites them with primary-source URLs per Cerebro §2.

---

## 5. Phase 3 BUILD scope reshuffle — 4 BUILDs → 5 BUILDs

PM THINK v1 §5 proposed a 4-BUILD Phase 3 split:

1. BUILD-1: block layer + virtio-blk + minimal VFS
2. BUILD-2: NeoFS native (on-disk format + content-addressing + CoW + GC) [ALL-IN-ONE]
3. BUILD-3: foreign FS read-only + initial userspace
4. BUILD-4: real hardware drivers (AHCI + NVMe) + bare-metal validation

**Cerebro's reframe forces BUILD-2 to split.** Single-tier physical-block-addressed CoW is a discrete shippable unit; content-addressing + dedup + advanced GC is a discrete subsequent unit with its own correctness and operational gates. **PM decision: promote to 5-BUILD Phase 3 split:**

| # | Name | Scope | Notes |
|---|---|---|---|
| **BUILD-1** | Block layer + virtio-blk skeleton | Block layer trait; virtio-blk **write-through** (no FLUSH negotiation); DMA infrastructure (identity-mapped initially; IOMMU Phase 5+); PCI/PCIe enumeration extensions; minimal in-RAM VFS skeleton; initial `sys_open`/`read`/`write`/`close`; libneo wrappers; test scenarios | Unchanged from THINK v1. virtio-blk write-through gives correctness without FLUSH per OASIS virtio v1.2 §VIRTIO_BLK_F_WCE. Coordinates with ADR-016 A1 IRQ-safe heap |
| **BUILD-2** | NeoFS native v1 | Single-tier physical-block-addressed CoW; Merkle-DAG tree (inode → block hashes; dir → child-inode hashes; superblock → root inode); checksummed redundant-superblock rotation with FLUSH barrier (three copies, simultaneous write, generation counter); typed-inode discriminant per Bible 10.3 (1-byte type field; small fixed enum; VFS mmap/execve dispatch reads type); synchronous-blocking I/O via SC-3C IPI cross-CPU-wake | First shippable NeoFS. **No CAS dedup; no buffer-cache write-back; no kthread requirement.** Block-size superblock field reserves CAS BUILD entry; inode reserves both hash and physical-address fields for CAS forward-compat |
| **BUILD-3** | NeoFS content-addressing + buffer cache + FLUSH | CAS dedup with bloom-filter + on-disk hashtable index (Venti model, ~1 seek per lookup); mark-sweep GC (IPFS model); virtio-blk VIRTIO_BLK_F_FLUSH negotiation; buffer-cache write-back with kthread; refcount-as-advisory-hint (only if mark-sweep pause measured as problem) | **Measurement-gated entry.** Ship only when measurable dedup ratio + workload justifies. **kthread primitive prerequisite** (see §7). May move to Phase 5 if Phase 5 model-dedup is the right driver |
| **BUILD-4** | Foreign FS read-only + userspace polish | FAT32 read-only; ext4 read-only; full POSIX-ish syscall surface (`stat`, `mkdir`, `rmdir`, `unlink`, `rename`, etc.); foreign-FS type-import policy (Coerce-to-Regular default; per-mount default-type option); libneo polish | Was THINK v1 BUILD-3. Type policy per Cerebro §7.4 |
| **BUILD-5** | Real hardware + bare-metal validation | SATA AHCI driver; NVMe driver; bare-metal validation per Bible Article 13 mandate; closes Phase 2+ bare-metal mandate carried forward | Was THINK v1 BUILD-4. Heavy driver lift; bare-metal verification |

**Net effect:** Phase 3 grows by one BUILD. BUILD-3 (content-addressing) is measurement-gated. If Phase 5 model-storage dedup becomes the primary driver of CAS work, BUILD-3 may move into Phase 5 substrate, with Phase 3 closing at BUILD-5. This is consistent with Bible Article 14 `/simplify` discipline — the substrate is shaped to allow deletion of BUILD-3 from Phase 3 if substrate evidence warrants.

---

## 6. Cerebro #2 + #3 dispatch framing reassessment

PM THINK v1 §7 identified three Cerebro dispatch candidates. #1 has returned and reshaped both #2 and #3 framings.

### 6.1 Cerebro #2 (storage driver prior art) — framing largely preserved, sharpened

**Original THINK v1 framing:** Theseus, Redox, FreeBSD, Linux virtio-blk/AHCI/NVMe.

**Sharpening from Cerebro #1:**

- **(a) virtio-blk write-through correctness for BUILD-1 vs FLUSH negotiation for BUILD-3.** Cerebro #1 §4.3 confirmed write-through is correct without VIRTIO_BLK_F_WCE; FLUSH negotiation is the BUILD-3 perf recovery. #2 should survey how production virtio-blk drivers stage write-through→WCE+FLUSH transitions.
- **(b) DMA infrastructure prior art (identity-mapped initially; IOMMU Phase 5+).** Cerebro #1 did not cover this; #2 should fill the gap.
- **(c) IRQ handler discipline for storage completions.** Explicit reference to SC-3C IPI cross-CPU-wake composition — #2 should validate that storage IRQ → cross-CPU-wake → process resume composition matches the pattern Cerebro #1 derived for NeoFS BUILD-2.
- **(d) NVMe submission/completion queue patterns for BUILD-5.** Single-thread-per-process invariant means we don't need per-thread submission queues; #2 should confirm the simpler discipline is production-viable.

**Drafting:** post-THINK-v2 lands (so the sharpened framing is anchored against finalized Q5.a-e substrate).

### 6.2 Cerebro #3 (VFS + capability surface) — framing materially shifted

**Original THINK v1 framing:** Linux/BSD VFS, Plan 9 9P, V Operating System, seL4 cap-on-file.

**Material shift from Cerebro #1:** Cerebro #1 §7 surveyed typed-inode prior art (SELinux, Capsicum, BFS, 9P) extensively and located NeoPlutarch's Bible 10.3 design "past production prior-art horizon." Cerebro #3 should NOT re-survey typed-inode storage; it should focus on the VFS-layer mechanism for Bible 10.3 enforcement:

- **(a) VFS-layer typed-inode dispatch architecture.** How does VFS mmap/execve dispatch read the type discriminant? Where does the policy table live (compile-time const? runtime configurable?)? How are retype operations mediated? Prior art: SELinux LSM hook structure; seL4 typed-object retype discipline.
- **(b) Capability-on-file design.** Capsicum rights propagation model (CAP_MMAP_X, CAP_MMAP_RX, CAP_MMAP_RWX; rights propagate through openat() at-or-below parent fd's rights) + seL4 typed-object Untyped→Typed retype discipline. Bible 10.3 needs both: per-inode authority claim (SELinux model) + per-fd attenuation (Capsicum model) + retype mediation (seL4 model).
- **(c) POSIX-ish syscall surface (Q7 in THINK v1).** What's the minimum viable POSIX-like syscall set for BUILD-1/BUILD-2? Linux, FreeBSD, illumos comparison; ADR-010 (Phase 2 capability) extension to FileCap.
- **(d) Mount namespace + foreign-FS type-import boundaries.** How does the VFS represent a mount of an FS that has no native typed-inode concept (FAT32, ext4)? Coerce-to-Regular vs per-mount default-type policy.

**Drafting:** post-THINK-v2 lands. Both #2 and #3 can dispatch in parallel after THINK v2 + ADR-021.

**Cadence note per `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md`:** these are within-frame question-level dispatches, not structural-challenge dispatches — Phase 3 substrate frame was established by Cerebro #1. The structural-challenge cadence rule does not re-apply within an established Phase frame.

---

## 7. kthread primitive audit — BUILD-3 prerequisite

Cerebro §8.3 flagged: "NeoFS BUILD-2 [Cerebro's numbering = our BUILD-3] needs at least one background kernel thread per FS instance (or per CPU, following the SC-3C per-CPU model). This requires a kthread primitive in NeoPlutarch's kernel."

**PM action item:** before BUILD-3 dispatch, audit Phase 2 substrate for kthread primitive availability.

Phase 2 BUILD-3 cadence delivered SC-3C IPI infrastructure + SC-4 per-CPU run queues; kernel threads exist *internally* to the scheduler (IPI handler bodies, per-CPU idle threads, etc.). The question is whether a subsystem outside the scheduler can request "spawn a kernel thread that runs this closure on this CPU forever, with these capabilities, reaped on FS unmount."

**Audit candidates:**

- If kthread is only scheduler-internal: BUILD-3 prerequisite is a `sys_kthread_spawn` or equivalent — kernel-side primitive (not exposed to userspace; created by other kernel subsystems) with capability discipline (FS instance owns kthread capability handle; on FS unmount, kthread is reaped via the standard exit path that consumes SC-4 D8's `Process.exit_state` CAS).
- If already exposed beyond scheduler: BUILD-3 consumes directly; no prerequisite.

Filing as Phase 3 substrate-review item in THINK v2 §3. Not a blocker for BUILD-1 or BUILD-2 entry. Will resolve before BUILD-3 PLAN dispatch.

**Forward-compat note:** if kthread primitive needs to be added, it lives in `kernel/src/sched/kthread.rs` (matches existing module structure per Bible Article 17). Phase 4 network stack will also need this primitive (backlog processor, retransmit timer); BUILD-3 prerequisite work pays the network stack's bill too.

---

## 8. NeoFS as first non-scheduler client of SC-3C IPI cross-CPU-wake — architectural decision

Cerebro §8.2 prescription accepted in full.

**Mechanism (per Cerebro):** the per-I/O rendezvous structure is a `SwitchMutex`-protected struct with `done: AtomicBool` and `waiting_thread: Option<Arc<Thread>>`. Both issuing-process context and storage-IRQ handler acquire the SwitchMutex at IF=0 per SC-3B discipline. Storage IRQ on CPU B sends IPI to CPU A (where issuing thread is blocked); CPU A's IPI handler marks the waiting thread runnable. Lost-wakeup invariant: AtomicBool `done` is checked under the SwitchMutex; if IRQ fires before sleep, the non-zero `done` is observed before park.

**Architectural significance:**

- First time SC-3C's IPI cross-CPU-wake primitive serves a non-scheduler client. Validates the SC-3C primitive is general-purpose, not scheduler-specific.
- Documents the composition pattern for future subsystems: IRQ on CPU B → cross-CPU IPI → CPU A wake → original thread resumes. This is the pattern Phase 4 network stack will need for receive-IRQ → user-thread wake.

**Forward marker FM-Phase4-1:** if Phase 4 network stack also adopts this pattern (overwhelmingly likely), an ADR documenting the "non-scheduler IPI cross-CPU-wake client" pattern is formalized at network stack design entry. Documents the composition contract: subsystem provides `(SwitchMutex<RendezvousSlot>)`; SC-3C provides cross-CPU wake.

**Validation against SC-3B/SC-3C closures:**

- SC-3B SwitchMutex (commit `de8965c` and follow-ons) provides the IF=0 lock discipline. ✓
- SC-3C IPI infrastructure (commits `88c7bc2`/`3272840`/`ead8b19`) provides the cross-CPU wake primitive. ✓
- SC-4 D8 `Process.exit_state` CAS provides exit serialization that handles "issuing thread killed while blocked on I/O" — kill walk (commit `57f3cb1`) is the prior art for this exact pattern. ✓

No new kernel primitive needed for BUILD-2 I/O wake. SC-3B+SC-3C+SC-4 substrate covers it.

---

## 9. Phase 3 forward markers (BUILD-3+ and beyond)

Per Cerebro §10, recorded as Phase 3 forward markers. Will move to PHASE-3.md when the phase doc is created (at Phase 3 BUILD-1 PLAN dispatch time, parallel to PHASE-2.md established at SC-3 cadence):

- **FM-Phase3-1:** CAS dedup as BUILD-3 feature (measurement-gated); not BUILD-2 structural primitive. Gate: measured dedup ratio + workload justifies the DDT/index/GC complexity.
- **FM-Phase3-2:** VIRTIO_BLK_F_FLUSH feature negotiation in BUILD-3. BUILD-2 ships virtio-blk write-through (correct, slightly slow per OASIS virtio v1.2).
- **FM-Phase3-3:** kthread primitive prerequisite for BUILD-3; audit Phase 2 substrate at THINK v2 (§7 above).
- **FM-Phase3-4:** Block-size as mkfs/superblock parameter (BUILD-2 planning). Reserve 16/64/256 KiB as valid alternatives for model-storage workloads.
- **FM-Phase3-5:** Incremental mark-sweep GC (BUILD-4+ optional). IPFS's full-blockstore GCLock is multi-minute at large scales; tri-color marking is the upgrade path. Defer behind workload measurement.
- **FM-Phase3-6:** HAMMER-style UNDO/REDO FIFO log for bounded recovery time (BUILD-4+ optional). Pure CoW with two-flush commit gives no-fsck correctness; HAMMER adds intent log for fsync-heavy workloads. Read Dillon 2008 hammer.pdf before committing.
- **FM-Phase3-7:** Type extension versioning in inode header (BUILD-2 planning). `type_version` superblock field; reserved-bits range in inode type field. Prevents forward-compat breaks when `Model`, `Tensor`, `InferenceContext` are added in Phase 5+.
- **FM-Phase3-8:** Sidecar manifest (`.neofs-types`) for foreign-FS type roundtrip (BUILD-4+). Per Cerebro §7.4 option 3.
- **FM-Phase3-9:** Retype operation discipline (BUILD-3+; or wherever typed-inode lifecycle becomes load-bearing). Follow seL4 Untyped→Typed retype: old type's capabilities revoked; new type's capabilities issued. Design before implementing.
- **FM-Phase3-10:** Full ZFS-DDT-style real-time refcount (Phase 5+). Gated on measured dedup ratio >5× AND RAM budget. Possible if Phase 5 base-model + fine-tune dedup produces these ratios (per ANSPEC-001 §8.2 BLAKE3 model dedup; per ALTIMIT artifact §D HF Xet / ZipLLM lineage).

## 10. Phase 4+ forward markers inheriting Phase 3 substrate

- **FM-Phase4-1:** Network stack adopts NeoFS's SC-3C IPI cross-CPU-wake pattern for receive-IRQ → user-thread wake. ADR on "non-scheduler IPI cross-CPU-wake client" pattern formalization at network stack design entry.
- **FM-Phase5-1:** Phase 5 `Model` content-addressing per ANSPEC-001 §8.2 + ALTIMIT artifact §E (HF Xet / ZipLLM lineage). Depends on Phase 3 BUILD-3 if shipped in Phase 3; otherwise Phase 5 must include the CAS substrate. PM decision deferred until Phase 5 substrate work; Phase 3 BUILD-3 is the simpler path if substrate becomes load-bearing earlier.

---

## 11. PM consumption cadence — next steps

Order of operations from this PM-NOTE forward:

1. **Phase 3 THINK v2** (PM action, immediate next): revises THINK v1 §4 Q5.a-e per finalized D-decisions above; revises §5 BUILD scope split per §5 of this PM-NOTE (4 → 5 BUILDs); incorporates five citation corrections per §4; surfaces kthread audit as substrate-review item per §7; adds forward markers per §9-10. **Carries pre-challenge-exploration header per cadence-refinement PM-NOTE — though v2 is post-challenge synthesis, the v1 it supersedes was implicitly pre-challenge despite lacking the header.** Estimated 30-40 KB output. Filename: `Plans/PHASE-3-THINK-V2-2026-05-11.md` (or v2 with later date if drafting spans a day).

2. **ADR-021 Phase 3 design substrate** (PM action, post-THINK-v2): analogous to ADR-013 for Phase 2 BUILD-3. Forward-looking Q-shape per ADR convention; captures the substrate decisions from THINK v2; sets scope gates for each of the 5 BUILDs. Filename: `docs/ADR-021-phase-3-design-substrate.md`.

3. **Cerebro #2 dispatch** (PM draft post-ADR-021; PL ferries): storage driver prior art with sharpened framing per §6.1. Question-level dispatch within established Phase 3 frame. Estimated 4-8 hours Cerebro budget. Filename: `Plans/DISPATCH-<date>-phase-3-storage-driver-prior-art-cerebro.md`.

4. **Cerebro #3 dispatch** (PM draft post-ADR-021; PL ferries): VFS + typed-inode dispatch architecture + capability-on-file with shifted framing per §6.2. Question-level dispatch within established Phase 3 frame. Estimated 4-8 hours Cerebro budget. Can parallel with #2. Filename: `Plans/DISPATCH-<date>-phase-3-vfs-typed-inode-capability-cerebro.md`.

5. **Phase 3 BUILD-1 PLAN dispatch for Jarvis** (PM action, post-ADR-021 + post-#2 + post-#3 consumption): block layer + virtio-blk write-through + DMA infrastructure + minimal VFS skeleton. Sub-commit boundary per THINK v2.

6. **kthread audit** (PM action, surface in THINK v2; resolve before BUILD-3 dispatch): verify whether NeoPlutarch has kthread primitive exposed beyond scheduler internals. If not, BUILD-3 prerequisite adds it.

Parallel non-blocking PL operational items (unchanged from end-of-day handoff):
- VMWare hypervisor-VM verification per `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md`.
- rustfmt tooling gap resolution.

---

## 12. Closure

Cerebro #1 contribution: equivalent in architectural decisiveness to Cerebro D5 (SC-4). PM Q5.a framing was substantively wrong-shaped; Cerebro reframed cleanly with anchor evidence from three independently-converging production CAS systems (Plan 9 Fossil+Venti retirement; ZFS dedup OpenZFS-maintainer 2024 verdict; IPFS mark-sweep-by-design). Phase 3 BUILD scope reshuffled to 5 BUILDs to give content-addressing its own measurement-gated entry. Five citation corrections accepted; THINK v2 carries them. kthread audit surfaced as Phase 3 substrate-review item; not blocker for BUILD-1 or BUILD-2 entry. NeoFS confirmed as first non-scheduler client of SC-3C IPI cross-CPU-wake primitive, validating SC-3C primitive's general-purpose intent and pre-baking the Phase 4 network stack composition pattern.

PL diagnostic post-this-PM-NOTE sharpened the §2 cadence diagnosis from "framing-gate failed" to the precise two-level model (PM at framing, Cerebro at question, THINK v1 written before alignment) and prescribed the cadence fix (first-Cerebro-on-new-substrate = structural-challenge). Codified at `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md`. Bible Article 14 amendment candidacy at Phase 4 close if three-for-three pattern holds.

PM holding for THINK v2 drafting. No PL action required pre-THINK-v2. PL operational items (VMWare verification, rustfmt) remain non-blocking.

---

**End of PM-NOTE.**

*Phase 3 Q5 reframed to single-tier physical-block-addressed CoW in first NeoFS BUILD; CAS dedup deferred to measurement-gated BUILD-3. Five citation corrections accepted. Phase 3 BUILD count promoted to 5. kthread audit surfaced as BUILD-3 prerequisite. NeoFS as first non-scheduler IPI cross-CPU-wake client confirmed. §2 cadence diagnosis sharpened post-filing by PL; codified in cadence-refinement PM-NOTE. Next: Phase 3 THINK v2.*
