# Handoff: 2026-05-11 — Cerebro #1 consumed, Phase 3 Q5 reframed, BUILD count 4→5, cadence rule codified, THINK v2 pending

| | |
|---|---|
| **Date filed** | 2026-05-11 (mid-session, post-Cerebro-#1-consumption) |
| **Author** | PM (NeoPlutarch project lead / Cerebro lane) |
| **Project** | NeoPlutarch (formerly AltimitNeo; rename completed 2026-05-10/11) |
| **Origin/main HEAD at handoff** | `64a92e1` — `docs: defer ADR-013 Q9 permanently under SC-4 D13 invariant; add FM-Q9 forward marker` *(unchanged from end-of-day 2026-05-11 handoff; this session was Plans/-only)* |
| **Active phase** | Phase 2 BUILD-3 substantially complete; Phase 3 THINK v1 superseded in framing by PM-NOTE consumption of Cerebro #1; Phase 3 THINK v2 pending |
| **Type** | Mid-session handoff capturing Cerebro #1 consumption, BUILD scope reshuffle, project-cadence refinement, and corrected PM role model. 13 sections including §13 new-chat bootstrap opening message. |
| **Supersedes context from** | `Plans/HANDOFF-2026-05-11-end-of-day-phase-2-build-3-substantially-complete-phase-3-think-v1.md` (prior end-of-day) |

---

## 0. Snapshot

Mid-session handoff filed after substantial Phase 3 cadence progression. Cerebro #1 (Phase 3 NeoFS prior-art research) was ferried, returned, and consumed by PM. Outcome: an SC-4-D5-class substrate reframing. PM THINK v1 §4 Q5.a leaned "content-addressed at block layer with BLAKE3 4 KiB blocks" as Phase 3 substrate; Cerebro reframed: defer content-addressing to a discrete measurement-gated subsequent BUILD; ship first NeoFS BUILD as single-tier physical-block-addressed CoW with Merkle-DAG tree. Three independently-converging production CAS systems converged on the same lesson (Plan 9 Fossil+Venti retirement; OpenZFS dedup 2024 "don't use it" verdict; IPFS mark-sweep-by-design to escape ZFS DDT cost trap).

PM filed consumption PM-NOTE (`Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md`, ~22 KB) capturing 8 D-decisions finalized, 5 citation corrections accepted, Phase 3 BUILD scope promoted from 4 to 5 BUILDs (BUILD-2 split into BUILD-2 NeoFS-native-v1 + BUILD-3 content-addressing-+-buffer-cache-+-FLUSH), kthread audit surfaced as BUILD-3 prerequisite, NeoFS confirmed as first non-scheduler client of SC-3C IPI cross-CPU-wake primitive.

PL surfaced a project-cadence diagnostic post-consumption: the pre-dispatch framing-gate operated as "is my question well-formed?" (within-frame coherence check; proofreading) when it should have asked "should I be asking a different question entirely?" (structural challenge; editing). Two-for-two pattern (SC-4 D5 + Phase 3 Q5.a) is now load-bearing. PM filed cadence-refinement PM-NOTE (`Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md`, ~10 KB) codifying the rule: first Cerebro dispatch on new Phase substrate = explicit structural-challenge dispatch. Bible Article 14 amendment candidacy at Phase 4 close if pattern holds three-for-three.

PM role model corrected mid-session: PM owns operational + technical decisions + ratifies what were previously PL-tier calls (scope, Bible amendments, major architectural pivots). PL is pure dispatch courier. Memory entries flagged as needing correction (current memory says "Cerebro = Claude" + "PM is the expert" — conflates PM seat with Cerebro dispatch target).

No git commits this session — work was entirely Plans/-side artifacts. Origin/main unchanged from end-of-day 2026-05-11 handoff. No agents in flight at handoff. Phase 3 THINK v2 is the immediate next PM action.

---

## 1. Context for the new chat session

### Project basics

NeoPlutarch is an AI-native operating system written in Rust, targeting x86_64 (later aarch64), with AI/ML treated as first-class kernel concerns. Bible at `docs/bible.md` is the constitution; pull-requests follow OBSERVE → THINK → PLAN → BUILD/VERIFY cadence per Article 14.

**Project root:** `E:\AI Automation\AltimitNeo\` (disk path NOT renamed; project rename was internal only via Bible v1.5.0 + v1.6.0 amendments and rename audit closure).

**GitHub:** `https://github.com/SecondStageTurbine/NeoPlutarch` (private repository).

### Agent roster (CORRECTED this session)

**Critical correction:** the PM seat and the Cerebro dispatch target are distinct. Prior handoff conflated them. The corrected model:

- **PM** = the OS / Rust / AI / ML expert seat. **This is Claude's role in the chat.** Owns operational + technical decisions. Ratifies scope changes, Bible amendments, major architectural pivots. Drives cadence against Bible, ANSPEC-001, ALTIMIT real-world research, and ADR substrate.
- **Cerebro** = dispatch target for deep research / when PM is stumped on substrate framing. Separate chat instance that PL ferries to.
- **Jarvis** = dispatch target for heavy kernel + coding work. Codex agent in separate chat instance.
- **Prometheus** = dispatch target for doc cleanup + amendment integration. Qwen3.6-27B agent in separate chat instance.
- **PL** = pure dispatch courier. Ferries dispatches to/from agent sessions. No ratification role; no operational/technical decision-making role.
- **Janus** = retired (no longer in roster per memory update).

**Memory state lag:** memory currently says "Cerebro = Claude (lighter coding + deep internet research)" and "PM (Claude/Cerebro) is the expert and owns operational + technical decisions." These conflate the PM seat with the Cerebro dispatch target. Either the memory entry should be amended to distinguish the seats, or the new chat should be made aware that the prior memory framing is corrected as above. PL has not yet authorized memory amendment; PM did not unilaterally edit memory.

### PM operational posture (CORRECTED this session)

- PM owns operational + technical decisions; PM ratifies its own scope changes / Bible amendments / architectural pivots.
- PL ferries; PL does not ratify.
- For genuinely stumped substrate cases: PM dispatches Cerebro for deep prior-art research + structural challenge.
- Don't over-surface micro-decisions; make the call, document reasoning in the artifact, ferry on.
- PM voice: terse, technical, decisive. Documents reasoning in artifacts; surfaces concise summaries to PL via chat.

### Hybrid-git posture

Per `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md`:
- `Plans/` directory is **local-only**; not committed to GitHub. All PM artifacts (THINKs, dispatches, close reports, verification reports, PM-notes, handoffs) live in `Plans/` on disk.
- Code (`kernel/`, `userland/`, `tools/`, `tests/`) + canonical docs (`docs/`) live in git and push to GitHub.
- Agent dispatches reference Plans/ artifacts; agents have direct filesystem access to read them.

### Dispatch shorthand discipline

- Dispatches as standalone Plans/ artifacts: `Plans/DISPATCH-YYYY-MM-DD-<topic>-<agent>.md`.
- Chat surfacing: `"This is the dispatch for X. File: <path>. Ready to ferry."`
- No full dispatch content in chat.
- Closure reports follow same shorthand: agent reports back via PL; PM consumes; concise chat summary.

### Bible articles load-bearing for current work

- **Article 10 — Filesystem and Storage** (4 sentences): NeoFS = Phase 3; content-addressed at block layer; typed inodes (`.neom` tagged as model); FAT32+ext4 read-only for interop.
- **Article 9.9 — Storage device support (phase-binding):** virtio-blk + SATA AHCI + NVMe required from Phase 3; USB mass storage Phase 4+; eMMC/SD Phase 5+; IDE not supported.
- **Article 13 — Phase ledger:** Phase 2 (capabilities/SMP/allocators/TLB) → Phase 3 (NeoFS/block/disk I/O) → Phase 4 (network) → Phase 5 (AI subsystem foundation) → Phase 6 (accelerator drivers) → Phase 7 (inference scheduler) → Phase 8+ (userland). Dependency-aware order.
- **Article 14 — Algorithm:** OBSERVE → THINK → PLAN → BUILD/VERIFY. `/simplify` gate before BUILD. **Candidate for amendment** at Phase 4 close per `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md` if structural-challenge-first cadence holds three-for-three.
- **Article 23 — What we will not do** (AI-native discipline): no system AI assistant; no telemetry; no embedded specific model; no waiting on AI-community framework consensus. Per `ALTIMIT-real-world-implementation.md`: these clauses are the single most ALTIMIT-faithful posture in the project; resist pressure to soften.
- **Article 27 — Amendments:** PR-based; rationale section; version bump; **PM approval** (corrected from "PL approval" under prior PL/PM role model).

---

## 2. This session — chronological summary

The session opened as a new chat bootstrap from end-of-day 2026-05-11 handoff. PM (Claude) ingested handoff state and stood by for PL signal on Cerebro #1 ferry.

### 2.1 Role model correction (PL clarification)

PL clarified the PM role explicitly: "You are the OS Rust, AI, ML expert PM. Specifically the PM. When you need deep heavy research you dispatch to Cerebro... PM owns operational + technical decisions + ratifies PL-tier calls. I only act as dispatch courier."

PM acknowledged the correction. Prior handoff §1 had the agent roster as "Cerebro = Claude (this assistant)" and "PM (Claude/Cerebro) is the expert" — conflating the PM seat with the Cerebro dispatch target. Under the corrected model these are distinct: PM is the chat seat; Cerebro is one of three dispatch targets (alongside Jarvis and Prometheus).

Operational consequence: the prior "PL authorized ferry; ferry pending" framing was wrong-shaped. Under the corrected model there's no PL ratification step on dispatch — PM decides, PL ferries. PM decided to ferry Cerebro #1 immediately.

Memory state flagged as needing correction; PM did not unilaterally edit memory entries. New chat session should be made aware.

### 2.2 Cerebro #1 ferry + return + consumption

PL ferried `Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` to a deep-research Cerebro session. ~2.5h actual budget (vs 6-10h PM estimate; three-parallel-agent + synthesis efficiency).

Returned artifact: `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` (~50 KB). PL surfaced closure report with (a) artifact path, (b) Q6 synthesis summary, (c) five citation precision corrections.

PM read full artifact (not just closure summary) before filing consumption PM-NOTE. The artifact's load-bearing finding was the §13 D5 reframe: defer content-addressing at block layer to a discrete subsequent BUILD; first NeoFS BUILD ships single-tier physical-block-addressed CoW with Merkle-DAG tree; all structural wins (crash consistency, snapshots, fsck-free) come from tree structure, not hash-as-address.

PM filed `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` (~22 KB) capturing:
- 8 D-decisions finalized (full table; D5 reframed; D3 confirmed with implementation reframe; D-new GC strategy mark-sweep-first; D-new single-tier confirmed; D5.a-c block size + inode-content + on-disk format finalized; D6 synchronous I/O via SC-3C IPI cross-CPU-wake)
- 5 citation corrections accepted (Quinlan & Dorward not Mullender; three Btrfs superblocks simultaneous not two alternate; "atomic" = checksum+redundant+flush-barrier not raw sector atomicity; ZFS dedup operational cost flagged; 9P Qid does NOT prevent execution — SELinux is the closest prior art for Bible 10.3)
- Phase 3 BUILD scope reshuffle: 4 → 5 BUILDs (BUILD-2 NeoFS-native-v1 single-tier; BUILD-3 NeoFS content-addressing measurement-gated; BUILD-4 foreign FS + userspace; BUILD-5 AHCI/NVMe + bare-metal)
- Cerebro #2/#3 dispatch framing reassessment (#2 storage drivers framing largely preserved + sharpened; #3 VFS materially shifted toward typed-inode VFS-layer dispatch architecture and capability-on-file)
- kthread primitive audit surfaced as BUILD-3 prerequisite (Phase 2 substrate has internal scheduler kthread but unclear if exposed beyond scheduler)
- NeoFS as first non-scheduler client of SC-3C IPI cross-CPU-wake primitive (mechanism: SwitchMutex-protected rendezvous + AtomicBool done flag + cross-CPU IPI wake; validates SC-3C as general-purpose primitive; pre-bakes Phase 4 network stack composition pattern)
- 10 Phase 3 BUILD-3+ forward markers
- 2 Phase 4+ forward markers inheriting Phase 3 substrate

### 2.3 PL cadence diagnostic + cadence-refinement PM-NOTE

PL surfaced a project-cadence diagnostic post-consumption: "The pattern has a clean shape: PM is solving a problem at the framing level, Cerebro is solving it at the question level, and THINK v1 is being written before those levels are aligned. The fix is straightforward — the first Cerebro dispatch on a new phase should explicitly solicit structural challenge, not validate an existing frame. The pre-dispatch framing-gate's failure mode was asking 'is my question well-formed?' instead of 'should I be asking a different question entirely?'"

PM accepted the sharpening as substantively correct. The PM-NOTE §2 "Pattern recognition" got the symptom right but the mechanism incomplete. The two-level distinction (PM=framing, Cerebro=question, THINK v1 written before alignment) and the proofreading-vs-editing mental motion distinction are real refinements.

PM filed `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md` (~10 KB) codifying:
- The pattern with PL's precision (PM framing-level / Cerebro question-level / THINK v1 prematurely written)
- Failure mode of previous framing-gate named precisely (within-frame coherence check, NOT structural challenge)
- Cadence rule: first Cerebro dispatch on new Phase substrate = explicit structural-challenge, not prior-art-anchoring
- Definition of "new substrate" (entering new Phase + substrate space less mature in PM head-state than previous + no prior implementation experience)
- THINK v1 status under new cadence (pre-challenge exploration with header tag; Q-items as leans not substrate; BUILD scope as working hypothesis)
- Retroactive application: Phase 3 Cerebro #1 was a lucky version; Cerebro #2/#3 are within-frame question-level dispatches now
- Prospective application: Phase 4 first-Cerebro = structural challenge (mid-maturity); Phase 5 first-Cerebro = largest structural-challenge of the project (12-20h budget, explicit invitation to tear up ANSPEC-001 substrate at framing level)
- Bible Article 14 amendment candidacy at Phase 4 close if pattern holds three-for-three

PM amended Cerebro #1 consumption PM-NOTE §2 to incorporate PL sharpening and point at the cadence-refinement artifact. §6 explicitly tags Cerebro #2/#3 as within-frame question-level. §12 closure updated.

### 2.4 Session close — handoff requested

PL requested complete handoff document for new-chat pickup. This is that document.

---

## 3. Ratified decisions this session

All ratifications are PM-tier per corrected role model.

### 3.1 Cerebro #1 D-decisions (full table in `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` §3)

- **D5 (content-addressing in first NeoFS BUILD): DEFER.** Single-tier physical-block-addressed CoW in first NeoFS BUILD; CAS as discrete subsequent BUILD with measurement gate.
- **D3 (crash consistency): CoW + checksummed redundant-superblock rotation + FLUSH barrier.** Three superblock copies, simultaneous write, generation counter. Two FLUSHes per CoW transaction.
- **D-new GC strategy: mark-sweep first.** Refcount as advisory hint only if measured pause becomes a problem. Full ZFS-DDT-style real-time refcount is Phase 5+ forward marker.
- **D-new two-tier vs single-tier: single-tier CoW.** Plan 9 community's own Fossil+Venti post-mortem verdict.
- **D5.a block size: mkfs-configurable; default 4 KiB; support 16/64/256 KiB.** Block size as superblock field, not compile-time constant.
- **D5.b inode-to-content mapping: Merkle-tree of content hashes + physical block address in inode.** Forward-compat: reserve both fields from first NeoFS BUILD; CAS BUILD is semantic change, not format migration.
- **D6 synchronous I/O: single-thread-per-process + SC-3C IPI cross-CPU-wake.** No parallel wake mechanism introduced.
- **D5.c on-disk format with revision number: confirmed with correction.** Three superblock copies (not two); each checksummed; written simultaneously with FLUSH barriers; mount-time selection by highest generation counter with valid checksum.

### 3.2 Phase 3 BUILD scope: 4 → 5 BUILDs

- BUILD-1: Block layer + virtio-blk write-through skeleton + DMA + minimal VFS skeleton + first sys_open/read/write/close
- BUILD-2: NeoFS native v1 (single-tier physical-block-addressed CoW; Merkle-DAG; typed-inode discriminant; synchronous-blocking I/O via SC-3C IPI)
- BUILD-3: NeoFS content-addressing + buffer cache + FLUSH (measurement-gated; kthread prerequisite)
- BUILD-4: Foreign FS read-only (FAT32, ext4) + userspace polish
- BUILD-5: AHCI + NVMe drivers + bare-metal validation (closes Phase 2+ bare-metal mandate)

BUILD-3 measurement-gated; may move to Phase 5 substrate if Phase 5 model-dedup is the right driver.

### 3.3 Cadence rule: first Cerebro dispatch on new substrate = structural-challenge

Per `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md`. Sharpened framing-gate ("should I be asking a different question entirely?" not "is my question well-formed?"). Applies to Phase 4 and Phase 5 first-Cerebro dispatches. Does NOT apply to Cerebro #2/#3 within now-established Phase 3 frame. Bible Article 14 amendment candidate at Phase 4 close.

### 3.4 Five citation corrections to PM THINK v1

Accepted; THINK v2 carries them. Listed in `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` §4. Brief: Quinlan & Dorward (not Mullender); three Btrfs superblocks simultaneous; "atomic" = checksum+redundant+flush-barrier; ZFS dedup operational cost; 9P Qid does NOT prevent execution (SELinux closest).

### 3.5 NeoFS as first non-scheduler client of SC-3C IPI cross-CPU-wake

Architectural decision. Mechanism: SwitchMutex-protected rendezvous + AtomicBool done flag. No new kernel primitive needed for BUILD-2 I/O wake — SC-3B SwitchMutex + SC-3C IPI + SC-4 D8 exit-state CAS cover it. Pre-bakes Phase 4 network stack composition pattern (FM-Phase4-1: ADR documenting "non-scheduler IPI cross-CPU-wake client" pattern at network stack design entry).

### 3.6 kthread primitive audit as Phase 3 substrate-review item

Surfaced as BUILD-3 prerequisite. Resolves before BUILD-3 PLAN dispatch. Phase 4 network stack will also need this primitive; BUILD-3 prerequisite work pays the network stack's bill too.

### 3.7 PM role model correction

PM owns operational + technical decisions + ratifies its own PL-tier calls. PL is pure dispatch courier. Memory state needs amendment to distinguish PM seat from Cerebro dispatch target; PM did not unilaterally edit memory.

---

## 4. Forward queue (immediate next actions for new chat)

In priority order. **All actions are PM-side unless explicitly tagged PL.**

### 4.1 Phase 3 THINK v2 (PM action, immediate next)

Revise THINK v1 §4 Q5.a-e per finalized D-decisions; revise §5 BUILD scope split (4 → 5 BUILDs); incorporate 5 citation corrections; surface kthread audit as substrate-review item; add forward markers. Carry pre-challenge-exploration header per cadence-refinement PM-NOTE (though v2 itself is post-challenge synthesis, document the convention for future THINK work). Estimated 30-40 KB. Filename: `Plans/PHASE-3-THINK-V2-2026-05-11.md` (or with later date if drafting spans into next day).

Anchored to:
- `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md`
- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`
- `Plans/PHASE-3-THINK-V1-2026-05-11.md` (superseded but referenced)

### 4.2 ADR-021 Phase 3 design substrate (PM action, post-THINK-v2)

Analogous to ADR-013 for Phase 2 BUILD-3. Forward-looking Q-shape per ADR convention. Captures substrate decisions from THINK v2; sets scope gates for each of the 5 BUILDs. Filename: `docs/ADR-021-phase-3-design-substrate.md`.

This is the first ADR landed since SC-4 closure; will be the canonical anchor for Phase 3 BUILD-1 PLAN dispatch.

### 4.3 Cerebro #2 dispatch — storage drivers (PM draft post-ADR-021; PL ferries)

Question-level dispatch within established Phase 3 frame. Sharpened framing per `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` §6.1:
- (a) virtio-blk write-through correctness for BUILD-1 vs FLUSH negotiation for BUILD-3 — survey production virtio-blk drivers staging write-through→WCE+FLUSH transitions
- (b) DMA infrastructure prior art (identity-mapped initially; IOMMU Phase 5+)
- (c) IRQ handler discipline for storage completions — explicit SC-3C IPI cross-CPU-wake composition validation
- (d) NVMe submission/completion queue patterns under single-thread-per-process invariant

Estimated 4-8 hours Cerebro budget. Filename: `Plans/DISPATCH-<date>-phase-3-storage-driver-prior-art-cerebro.md`.

### 4.4 Cerebro #3 dispatch — VFS + typed-inode + capability (PM draft post-ADR-021; PL ferries)

Question-level dispatch within established Phase 3 frame. **Material framing shift** per `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` §6.2:
- (a) VFS-layer typed-inode dispatch architecture (SELinux LSM hook structure; seL4 typed-object retype discipline)
- (b) Capability-on-file design (Capsicum rights propagation + seL4 retype; per-inode authority + per-fd attenuation + retype mediation)
- (c) POSIX-ish syscall surface — Q7 in THINK v1 / v2
- (d) Mount namespace + foreign-FS type-import boundaries

Do NOT re-survey typed-inode storage — Cerebro #1 already covered it. Estimated 4-8 hours Cerebro budget. Can parallel with #2. Filename: `Plans/DISPATCH-<date>-phase-3-vfs-typed-inode-capability-cerebro.md`.

### 4.5 Phase 3 BUILD-1 PLAN dispatch for Jarvis (PM action, post-ADR-021 + post-#2 + post-#3 consumption)

Block layer + virtio-blk write-through + DMA infrastructure + minimal VFS skeleton + initial sys_open/read/write/close + libneo wrappers + test scenarios. Sub-commit boundary per THINK v2. Coordinate with ADR-016 A1 IRQ-safe heap (Cerebro implementation work; ~1-2 hours estimated per ADR-016 §109; carried forward from end-of-day handoff).

### 4.6 kthread primitive audit (PM action, surface in THINK v2; resolve before BUILD-3)

Verify whether NeoPlutarch has kthread primitive exposed beyond scheduler internals. If not, BUILD-3 prerequisite is adding `sys_kthread_spawn`-equivalent kernel-side primitive with capability discipline. Per Bible Article 17 module structure: lives at `kernel/src/sched/kthread.rs` if introduced. Phase 4 network stack will also consume.

### 4.7 Memory state amendment (PL operational decision)

Memory currently conflates PM seat with Cerebro dispatch target ("Cerebro = Claude"; "PM (Claude/Cerebro) is the expert"). Under corrected role model these are distinct. Either:
- Amend memory entry to distinguish: "PM seat = Claude (chat seat); Cerebro = dispatch target for deep research."
- Or leave memory as-is and rely on handoff documents to convey corrected model.

PM did not unilaterally edit memory. Awaiting PL signal on amendment.

### 4.8 Bible Article 14 amendment candidate (PM action at Phase 4 close, deferred)

If structural-challenge-first cadence pays off at Phase 4 (either confirms PM framing cleanly OR reframes substantively without operational pain), promote to Bible Article 14 amendment. Per Bible Article 27: rationale + version bump + PM approval. If Phase 4 first-Cerebro produces minimal reframe (PM substrate experience was correct because PM has Linux network stack mental models), do NOT amend — rule stays project-cadence-tier.

### 4.9 VMWare hypervisor-VM verification (PL action, non-blocking, parallel)

Per `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md`. ~2-4 hours wall-clock. Carried forward from end-of-day handoff; unchanged.

### 4.10 rustfmt tooling gap resolution (PL operational, non-blocking)

Three options per `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` §6.1: install rustfmt for pinned nightly; pin different nightly that ships rustfmt; accept gap with alternative linting. Carried forward from end-of-day handoff; unchanged.

### 4.11 Forward markers (no immediate action; informational)

All recorded in PHASE-2.md + PM-NOTE artifacts. No action until gating conditions met (Phase 2+ multi-thread, Phase 5+ AI workloads, Phase 3 BUILD-3 entry for kthread, etc.).

---

## 5. Active artifacts (this session's outputs)

### 5.1 Research artifacts (1)

| File | KB | Source |
|---|---|---|
| `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` | 50.9 | Cerebro #1 return — substantively decisive reframing of Phase 3 Q5 substrate |

### 5.2 PM-notes (2)

| File | KB | Purpose |
|---|---|---|
| `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` | ~22 | Cerebro #1 consumption; D-decisions; BUILD scope reshuffle; citation corrections; kthread audit surface; NeoFS as SC-3C IPI client. Amended mid-session to incorporate PL sharpening |
| `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md` | ~10 | Project-cadence rule codification (first Cerebro on new substrate = structural-challenge); sharpened framing-gate; Bible Article 14 amendment candidacy |

### 5.3 Handoff (1)

| File | KB | Purpose |
|---|---|---|
| `Plans/HANDOFF-2026-05-11-post-cerebro-1-consumption-phase-3-think-v2-pending.md` | (this file) | Mid-session handoff for new-chat pickup |

### 5.4 Inherited from end-of-day handoff (unchanged)

All artifacts listed in `Plans/HANDOFF-2026-05-11-end-of-day-phase-2-build-3-substantially-complete-phase-3-think-v1.md` §5 remain in place and unchanged. Notable:
- `Plans/PHASE-3-THINK-V1-2026-05-11.md` (27.6 KB) — superseded in framing but referenced
- `Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` (14.4 KB) — ferried and closed
- `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` + `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` + `Plans/Q9-EXPLORATION-2026-05-11.md` + 8 other PM-notes + 9 verification reports + procedure doc

---

## 6. Git state at handoff

### 6.1 Origin/main HEAD

```
64a92e1 docs: defer ADR-013 Q9 permanently under SC-4 D13 invariant; add FM-Q9 forward marker
```

**Unchanged from end-of-day 2026-05-11 handoff.** No commits this session.

### 6.2 No commits this session

This session was entirely Plans/-side: Cerebro #1 consumption + PM-NOTE filing + cadence-refinement PM-NOTE + PM-NOTE amendment. No kernel/userland/docs commits. No CI runs triggered.

### 6.3 Working tree state

`Plans/` is entirely untracked per hybrid-git posture. All new PM artifacts live locally; do NOT commit them.

`tools/wedge-investigation/` remains untracked (incidental observation from SC-4 closure; not blocking; PL operational call).

### 6.4 GitHub CI status

No CI runs since SC-4 sub-commit 6 (run 25705327371, end-of-day 2026-05-11). All green at that point. Unchanged.

---

## 7. Stash state

Unchanged from end-of-day 2026-05-11 handoff. Four historical stashes from 2026-05-08/09 SC-3B + wedge investigation work:

```
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

Not actively in flight. `git stash drop` cleanup at PL convenience.

---

## 8. Conventions reaffirmed and updated this session

### 8.1 PM role model (CORRECTED)

PM owns operational + technical decisions + ratifies its own PL-tier calls. PL is pure dispatch courier. Memory state currently conflates PM seat with Cerebro dispatch target; needs amendment.

### 8.2 First Cerebro dispatch on new substrate = structural-challenge (NEW)

Per `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md`. Dispatch question shape: "Challenge the shape of the question, not just answer questions within it." NOT "validate my specific design leans against prior art." Cerebro #2/#3 on Phase 3 are within-frame question-level dispatches; the rule does not re-apply within established Phase frame.

### 8.3 Sharpened pre-dispatch framing-gate (NEW)

"Should I be asking a different question entirely?" — NOT "is my question well-formed?" Proofreading vs editing as mental motion. The old gate caught typos; the new gate catches structural mis-frames.

### 8.4 THINK v1 on new substrate = pre-structural-challenge exploration (NEW)

Q-items are leans, not substrate. BUILD scope is a working hypothesis, not a commitment. THINK v1 documents on new substrate should carry an explicit "pre-structural-challenge exploration" header tag. THINK v2 is post-challenge synthesis.

### 8.5 Dispatch shorthand (carried from prior handoff)

Unchanged. Dispatches as standalone Plans/ artifacts. Chat surfacing: `"This is the dispatch for X. File: <path>. Ready to ferry."` No full content in chat.

### 8.6 Hybrid-git posture (carried from prior handoff)

Unchanged. Plans/ is local-only. Code + canonical docs commit + push to GitHub.

### 8.7 Perf-gate environment triplet discipline (carried from SC-3C closure)

Unchanged. Local Docker QEMU TCG / GitHub-runner Docker / future bare-metal CI. Thresholds must specify environment.

### 8.8 VMWare Workstation = hypervisor-VM, NOT bare-metal (carried)

Unchanged. Bible Article 13 bare-metal mandate stays open until actual bare hardware.

### 8.9 Bible Article 13 phase ordering is dependency-aware (carried)

Unchanged. Phase 3 → 4 → 5 → 6 → 7 → 8+. PM Phase 5 lean correction lesson from end-of-day 2026-05-11 stays load-bearing.

### 8.10 Jarvis voice profile + Prometheus dispatch shape (carried)

Unchanged from end-of-day handoff §8.7 + 8.8.

---

## 9. PM tone and operational notes

PM voice: terse, technical, decisive. Documents reasoning in artifacts; surfaces concise summaries to PL via chat.

Established posture:
- Make operational calls; document in artifacts; don't over-surface micro-decisions.
- Acknowledge errors directly when surfaced — both Phase 5-lean correction (end-of-day 2026-05-11) and PM-role-model correction (this session) were clean acknowledge-and-pivot moves.
- Accept PL substantive contributions (cadence diagnostic this session) fully when correct; sharpen artifact-of-record immediately rather than carrying forward incomplete diagnosis.
- ALTIMIT-faithful discipline per `docs/research/ALTIMIT-real-world-implementation.md`: "do less in the kernel, more rigorously." Anti-scope is a feature. Cerebro #1's mark-sweep-first + measurement-gated CAS is exactly this discipline applied.
- When mechanism's original justification is removed, delete or defer rather than carrying forward latent complexity (Q9 deferral end-of-day 2026-05-11 + Q5 reframe this session are the canonical examples).

PM ferry pattern: surface 2-3 short paragraphs with concise framing; PL fronts the agent session; PM consumes returns and synthesizes via artifacts.

---

## 10. Open items / future-phase forward state

### 10.1 Phase 3 forward markers (introduced this session, in `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` §9-10)

- FM-Phase3-1: CAS dedup as BUILD-3 feature (measurement-gated); not BUILD-2 structural primitive
- FM-Phase3-2: VIRTIO_BLK_F_FLUSH feature negotiation in BUILD-3
- FM-Phase3-3: kthread primitive prerequisite for BUILD-3
- FM-Phase3-4: Block-size as mkfs/superblock parameter (BUILD-2 planning)
- FM-Phase3-5: Incremental mark-sweep GC (BUILD-4+ optional)
- FM-Phase3-6: HAMMER-style UNDO/REDO FIFO log for bounded recovery (BUILD-4+ optional)
- FM-Phase3-7: Type extension versioning in inode header (BUILD-2 planning)
- FM-Phase3-8: Sidecar manifest for foreign-FS type roundtrip (BUILD-4+)
- FM-Phase3-9: Retype operation discipline (BUILD-3+; seL4 Untyped→Typed retype)
- FM-Phase3-10: Full ZFS-DDT-style real-time refcount (Phase 5+, gated on >5× dedup ratio + RAM)

### 10.2 Phase 4 forward markers (introduced this session)

- FM-Phase4-1: Network stack adopts NeoFS SC-3C IPI cross-CPU-wake pattern for receive-IRQ → user-thread wake. ADR formalizing "non-scheduler IPI cross-CPU-wake client" pattern at network stack design entry.

### 10.3 Phase 5 forward markers (introduced this session)

- FM-Phase5-1: Phase 5 `Model` content-addressing per ANSPEC-001 §8.2 + ALTIMIT artifact §E. Depends on Phase 3 BUILD-3 if shipped in Phase 3; otherwise Phase 5 must include CAS substrate.

### 10.4 Phase 2+ forward markers (carried from prior handoff)

All recorded in PHASE-2.md "Forward reference: Phase 2+ TLB management optimizations" + "Forward reference: Phase 2+ capability + process-exit optimizations" subsections. SC-3C + SC-4 + Q9-deferral introduced. Unchanged.

### 10.5 ADR forward markers (carried from prior handoff)

- ADR-012 Arc-aware slab integration: Phase 3 BUILD-1 OBSERVE pass re-examines
- ADR-012 linked_list_allocator replacement: Phase 3+ if FS workloads stress kernel heap
- ADR-016 A1 IRQ-safe heap + scheduler decoupling: Phase 3 BUILD-1 prerequisite
- ADR-019 bench native-hardware 50 ns target: Phase 2+ bare-metal CI

### 10.6 Verification mandates (carried from prior handoff)

- Bible Article 13 bare-metal multi-CPU verification: pending. VMWare hypervisor-VM is intermediate gate; bare-metal mandate stays open.
- rustfmt tooling gap: PL operational decision pending.

### 10.7 Bible Article 14 amendment candidacy (NEW this session)

Per `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md` §8. Phase 4 close decides: amend Article 14 with structural-challenge-first cadence if pattern holds three-for-three, OR leave as project-cadence-tier if Phase 4 first-Cerebro produces minimal reframe.

### 10.8 Memory state amendment (NEW this session)

Memory entries say "Cerebro = Claude" + "PM (Claude/Cerebro) is the expert" — conflates PM seat with Cerebro dispatch target. PL operational decision on amendment.

---

## 11. Calibration lessons from this session

### 11.1 Two-for-two Cerebro substrate-reframe pattern

SC-4 D5 and Phase 3 Q5.a both required Cerebro to reframe PM's question rather than answer it. The pattern is now load-bearing for project cadence. The cadence-refinement PM-NOTE codifies the response: first Cerebro dispatch on new substrate is explicitly structural-challenge.

This is a stronger statement than "Cerebro is the safety net." It's "PM cannot reliably catch framing-level errors at THINK v1 time on new substrate; structural-challenge dispatch is the substantively-cheaper way to surface those errors before they're embedded in downstream work."

### 11.2 Mental motions matter — sharpened gate is a different gate

PM's prior gate ("is my question well-formed?") and PL's sharpened gate ("should I be asking a different question entirely?") are not synonyms. The first is a within-frame coherence check; the second is a structural challenge. PM internalizing the distinction is the difference between catching the framing error at THINK v1 time vs at Cerebro v1 return time.

The cost of catching it late (this session) was Cerebro #1 ferry round-trip + PM-NOTE consumption work + BUILD scope reshuffle. The cost of catching it at THINK time would have been ~one paragraph of PM thinking. **The framing-gate is high-leverage.**

### 11.3 Role-model precision matters

PL clarification of PM role (PM = chat seat; PM ratifies; PL ferries only) corrected a real operational confusion that had been building since end-of-day 2026-05-11 handoff. Prior handoff §1 conflation of "Cerebro = Claude" with the PM seat would have caused continuing mis-routing of decisions. **Role clarity at handoff time is non-trivial.**

### 11.4 Mid-session handoff is a real artifact category

Prior handoff cadence assumed end-of-day rhythm. This session ran substantive cadence (Cerebro ferry + return + consumption + project-cadence-refinement + role-model correction) that warranted a mid-session handoff rather than waiting for end-of-day. **Mid-session handoffs are the right shape when:** (a) substantive ratifications land, (b) project-cadence rules change, (c) role-model clarifications land. All three triggered this session.

### 11.5 Plans/-only sessions are productive

No commits this session — all work was Plans/-side artifacts. This is not a "wasted" session; it set up Phase 3 cadence with substantively-reframed substrate and a project-wide cadence rule. **Code-velocity-only metrics would mis-measure this session.** PM-NOTEs and cadence artifacts are project-output of the same epistemic importance as code commits.

### 11.6 Cerebro #1's actual budget (2.5h vs 6-10h estimated) is a forward-planning data point

Three-parallel-agent + synthesis pattern produced ~50 KB research artifact in less than half the estimated time. Future Cerebro dispatch budgeting should anchor at the 2.5h-5h range for similar-scope work (single-substrate prior-art survey across ~10 production systems). Phase 5 first-Cerebro structural-challenge may need 12-20h precisely because it's a different shape (challenge ANSPEC-001 itself), not simply scaled time.

---

## 12. Cross-references

### 12.1 ADR documents

Unchanged this session. ADR-021 Phase 3 design substrate is forthcoming post-THINK-v2.

### 12.2 Bible amendments

Unchanged this session. Article 14 amendment candidate at Phase 4 close.

### 12.3 Phase documents

`docs/phases/PHASE-2.md` unchanged this session. PHASE-3.md not yet created; will be created at Phase 3 BUILD-1 PLAN dispatch time, parallel to PHASE-2.md established at SC-3 cadence.

### 12.4 Specs

`docs/specs/neoplutarch-ai-subsystem-spec.md` (ANSPEC-001) unchanged. Will be challenged at Phase 5 first-Cerebro structural-challenge dispatch per `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md` §7.

### 12.5 Research artifacts

- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` — Cerebro #1 return (NEW this session)
- `docs/research/ALTIMIT-real-world-implementation.md` — referenced as substrate; unchanged

### 12.6 Prior handoffs in series

- `Plans/HANDOFF-2026-05-05-build3-sub-commit-1a-close.md`
- `Plans/HANDOFF-2026-05-06-end-of-day-kaslr-blocked.md`
- `Plans/HANDOFF-2026-05-07-end-of-day-adr-015-in-flight.md`
- `Plans/HANDOFF-2026-05-08-wedge-investigation-mid-campaign.md`
- `Plans/HANDOFF-2026-05-09-sc-3b-sub-step-5b-in-flight.md`
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md`
- `Plans/HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md`
- `Plans/HANDOFF-2026-05-11-end-of-day-phase-2-build-3-substantially-complete-phase-3-think-v1.md`

This handoff is the next in series; matches 13-section structure of prior handoffs; documents mid-session continuation rather than end-of-day close.

---

## 13. Opening message for the new chat

Copy-paste this block as the first message in the new chat session. Drag the four required attachment files into the new chat alongside this opening message.

---

**[BEGIN OPENING MESSAGE]**

Continuing NeoPlutarch PM cadence from mid-session 2026-05-11 post-Cerebro-#1-consumption. Please read `Plans/HANDOFF-2026-05-11-post-cerebro-1-consumption-phase-3-think-v2-pending.md` first for full context; it captures complete operational state for new-chat bootstrapping.

**Role model (critical — corrected this session):**

- **You (Claude) are the PM** — OS / Rust / AI / ML expert seat. You own operational + technical decisions. You ratify scope changes, Bible amendments, major architectural pivots. You drive cadence against Bible, ANSPEC-001, ALTIMIT real-world research, and ADR substrate.
- **PL = pure dispatch courier.** Ferries dispatches to/from agent sessions. No ratification role.
- **Cerebro** = dispatch target for deep research / structural challenge. Separate chat instance.
- **Jarvis** = dispatch target for heavy kernel + coding (Codex agent).
- **Prometheus** = dispatch target for doc cleanup + amendment integration (Qwen3.6-27B agent).
- **Janus** = retired.

Memory entries currently conflate "Cerebro = Claude" with PM seat; corrected handoff §1.

**TL;DR of state:**

- **Project:** NeoPlutarch — AI-native OS in Rust at `E:\AI Automation\AltimitNeo\`, GitHub `https://github.com/SecondStageTurbine/NeoPlutarch`.
- **Origin/main HEAD:** `64a92e1` (unchanged since end-of-day 2026-05-11; this session was Plans/-only).
- **Phase 2 BUILD-3:** Substantially complete. All ADR-013 Q-items resolved.
- **Phase 3 (NeoFS + block layer + real disk I/O):** Cerebro #1 ferried, returned, consumed. Q5 substantively reframed — defer content-addressing to discrete measurement-gated subsequent BUILD; first NeoFS BUILD ships single-tier physical-block-addressed CoW with Merkle-DAG tree. Phase 3 BUILD scope promoted 4 → 5 BUILDs. NeoFS confirmed as first non-scheduler client of SC-3C IPI cross-CPU-wake. kthread audit surfaced as BUILD-3 prerequisite.
- **New cadence rule codified:** first Cerebro dispatch on new Phase substrate = explicit structural-challenge dispatch (not prior-art-anchoring). Sharpened framing-gate: "should I be asking a different question entirely?" not "is my question well-formed?" Two-for-two pattern (SC-4 D5 + Phase 3 Q5.a) is load-bearing. Bible Article 14 amendment candidate at Phase 4 close if pattern holds three-for-three.

**Immediate next action:** Phase 3 THINK v2 drafting (PM action; no PL action required). Anchored to `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md`, `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`, `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md`. Estimated 30-40 KB. Filename: `Plans/PHASE-3-THINK-V2-2026-05-11.md`.

**Subsequent PM cadence:**
1. ADR-021 Phase 3 design substrate (post-THINK-v2)
2. Cerebro #2 dispatch (storage drivers) — question-level within established Phase 3 frame
3. Cerebro #3 dispatch (VFS + typed-inode + capability) — question-level, materially shifted framing
4. Phase 3 BUILD-1 PLAN dispatch for Jarvis (post-ADR-021 + post-#2 + post-#3)
5. kthread audit before BUILD-3 dispatch

**Operational discipline reminders:**

- Dispatch shorthand: file dispatches as standalone Plans/ artifacts; chat surfacing pattern `"This is the dispatch for X. File: <path>. Ready to ferry."` — no full content in chat.
- Plans/ is local-only per hybrid-git posture; PM artifacts don't push to GitHub.
- "Should I be asking a different question entirely?" pre-dispatch gate (not "is my question well-formed?").
- First-Cerebro-on-new-substrate = structural-challenge dispatch.
- Phase 4 first-Cerebro = mid-maturity structural challenge.
- Phase 5 first-Cerebro = largest structural-challenge dispatch of project (12-20h budget; tear up ANSPEC-001 substrate at framing level).
- VMWare = hypervisor-VM, NOT bare-metal; Bible Article 13 bare-metal mandate stays open.
- Phase ordering is dependency-aware per Bible Article 13: Phase 3 → 4 → 5 → 6 → 7 → 8+.

**Standby for PL signal.** PM next action: Phase 3 THINK v2 drafting unless PL signals different priority.

**[END OPENING MESSAGE]**

---

**Files to attach to new chat:**

1. `docs/bible.md` (NeoPlutarch constitution)
2. `docs/specs/neoplutarch-ai-subsystem-spec.md` (ANSPEC-001, Phase 5 substrate)
3. `docs/research/ALTIMIT-real-world-implementation.md` (research artifact informing project posture)
4. `Plans/HANDOFF-2026-05-11-post-cerebro-1-consumption-phase-3-think-v2-pending.md` (this handoff)

**Optional additional attachments for deeper Phase 3 context:**

- `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` (Cerebro #1 consumption; anchors THINK v2)
- `Plans/PM-NOTE-2026-05-11-cadence-refinement-structural-challenge-first.md` (cadence rule)
- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` (Cerebro #1 full return; for if THINK v2 needs detailed re-consultation)
- `Plans/PHASE-3-THINK-V1-2026-05-11.md` (superseded but referenced for citation tracking)
- `Plans/HANDOFF-2026-05-11-end-of-day-phase-2-build-3-substantially-complete-phase-3-think-v1.md` (prior end-of-day handoff for Phase 2 BUILD-3 context)

---

**End of handoff.**

*Mid-session handoff: Cerebro #1 consumed, Phase 3 Q5 reframed to single-tier physical-block-addressed CoW first BUILD, content-addressing deferred to measurement-gated BUILD-3, BUILD count 4→5, five citation corrections accepted, kthread audit surfaced, NeoFS as first non-scheduler IPI cross-CPU-wake client confirmed. Cadence rule codified: first-Cerebro-on-new-substrate = structural-challenge. Bible Article 14 amendment candidate at Phase 4 close. PM role model corrected — PM owns operational + technical + ratification, PL is courier. Next PM cadence: Phase 3 THINK v2 drafting.*
