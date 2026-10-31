# Handoff: End-of-day 2026-05-11 — Phase 2 BUILD-3 substantially complete, Phase 3 THINK v1 filed, Cerebro #1 dispatch ready to ferry

| | |
|---|---|
| **Date filed** | 2026-05-11 (end of day) |
| **Author** | PM (Claude/Cerebro lane) |
| **Project** | NeoPlutarch (formerly AltimitNeo; rename completed 2026-05-10/11) |
| **Origin/main HEAD at handoff** | `64a92e1` — `docs: defer ADR-013 Q9 permanently under SC-4 D13 invariant; add FM-Q9 forward marker` |
| **Active phase** | Phase 2 BUILD-3 substantially complete; Phase 3 THINK v1 filed; awaiting Cerebro #1 dispatch ferry |
| **Type** | Comprehensive end-of-day handoff matching 2026-05-10 precedent (`HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md`). 13 sections including §13 opening message for new chat bootstrap. |

---

## 0. Snapshot

Phase 2 BUILD-3 substantially complete on 2026-05-11. All ADR-013 Q-items (Q1-Q9) either implemented or explicitly deferred with documented rationale. Origin/main carries SC-3C closure (Q7.b CR3 cache + Q7.c IPI TLB shootdowns), SC-4 closure (Q8.a/b capability table SMP + Q8.c blocked-thread-kill walk), and Q9 deferral (per-CPU reapers + `live_thread_count` obsoleted under SC-4 D13's single-thread-per-process invariant via D8's `Process.exit_state` AtomicU8 CAS-claim discipline).

PM cadence pivoted to Phase 3 (NeoFS + block layer + real disk I/O) after PL pushback on PM's initial Phase 5 lean as wrong-shaped (project-aesthetics-driven, not substrate-dependency-driven; Bible Article 13 ordered phases dependency-aware; Phase 3 NeoFS unblocks Phase 5 content-addressed model dedup per ANSPEC §8.2 + Bible 10.2). Phase 3 THINK v1 filed at 27.6 KB (exploration shape; 10 Q-items across block/VFS/NeoFS/foreign-FS/syscalls; 4-BUILD scope proposal; 3 Cerebro dispatch candidates).

Cerebro #1 dispatch (NeoFS prior-art research; 6 questions anchoring Q5.a-e) drafted and ready to ferry on PL signal. Authorization given mid-session; ferry pending.

VMWare Workstation hypervisor-VM verification procedure document filed for PL direct execution (manual procedure, non-blocking, ~2-4 hours wall-clock; explicitly labeled hypervisor-VM not bare-metal so Bible Article 13 bare-metal mandate stays open).

No agents in flight at handoff. No blocked critical-path items. Phase 3 BUILD-1 work begins after Cerebro #1 returns + PM consumes + THINK v2 + ADR-021 draft.

---

## 1. Context for the new chat session

### Project basics

NeoPlutarch is an AI-native operating system written in Rust, targeting x86_64 (later aarch64), with AI/ML treated as first-class kernel concerns. Bible at `docs/bible.md` is the constitution; pull-requests follow OBSERVE → THINK → PLAN → BUILD/VERIFY cadence per Article 14.

**Project root:** `E:\AI Automation\AltimitNeo\` (disk path NOT renamed; project rename was internal only via Bible v1.5.0 + v1.6.0 amendments and rename audit closure).

**GitHub:** `https://github.com/SecondStageTurbine/NeoPlutarch` (private repository).

### Agent roster (current, per memory update 2026-05-10)

- **Cerebro** = Claude (this assistant). Deep online research + PM operational + technical decisions. Lighter coding work.
- **Jarvis** = Codex. Heavy coding lane, kernel source authoritative work. Calibrated voice profile per `Plans/PM-NOTE-2026-05-11-jarvis-voice-profile.md`.
- **Prometheus** = Qwen3.6-27B. Documentation work, amendment integration, document cleanup, trial for small coding. Voice profile per `Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md`.
- **Janus** = retired (no longer in roster per memory update).

### PM operational posture (PL clarification 2026-05-10, post-Sub-step-6)

PM (Cerebro) is the expert and owns operational + technical decisions. PL is dispatch courier + sign-off on legitimately PL-tier calls (ratifications, bible amendments, scope, major architectural pivots). For genuinely stumped cases, escalate via Cerebro deep-online research. Don't over-surface (a)/(b)/(c) micro-decisions; make the call, document reasoning in the artifact, ferry on.

### Hybrid-git posture

Per `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md`:
- `Plans/` directory is **local-only**; not committed to GitHub. All PM artifacts (THINKs, dispatches, close reports, verification reports, PM-notes, handoffs) live in `Plans/` on disk.
- Code (`kernel/`, `userland/`, `tools/`, `tests/`) + canonical docs (`docs/`) live in git and push to GitHub.
- Agent dispatches reference Plans/ artifacts; agents have direct filesystem access to read them.

### Dispatch shorthand discipline

Per PL preference established mid-2026-05-11:
- Dispatches are filed as standalone artifacts at `Plans/DISPATCH-YYYY-MM-DD-<topic>-<agent>.md`.
- Chat surfacing follows the pattern: `"This is the dispatch for X. File: <path>. Ready to ferry."`
- Full dispatch content is NOT pasted into chat; PL ferries by reading the artifact and pasting to agent session.
- Closure reports follow same shorthand: agent reports back; PM consumes; concise chat summary; no full report contents in chat.

### Bible articles load-bearing for current work

- **Article 10 — Filesystem and Storage** (4 sentences): NeoFS = Phase 3; content-addressed at block layer; typed inodes (`.neom` tagged as model); FAT32+ext4 read-only for interop.
- **Article 9.9 — Storage device support (phase-binding):** virtio-blk + SATA AHCI + NVMe required from Phase 3; USB mass storage Phase 4+; eMMC/SD Phase 5+; IDE not supported.
- **Article 13 — Phase ledger** (8.x lines): Phase 2 (capabilities/SMP/allocators/TLB) → Phase 3 (NeoFS/block/disk I/O) → Phase 4 (network) → Phase 5 (AI subsystem foundation) → Phase 6 (accelerator drivers) → Phase 7 (inference scheduler) → Phase 8+ (userland). Dependency-aware order.
- **Article 14 — Algorithm:** OBSERVE → THINK → PLAN → BUILD/VERIFY. `/simplify` gate before BUILD.
- **Article 27 — Amendments:** PR-based; rationale section; version bump; PL approval.

---

## 2. This session — chronological summary

The 2026-05-11 session was substantial. Carried multiple major closures from morning to end-of-day:

### 2.1 Morning: SC-3C closure work (carried over from 2026-05-10 EOD handoff)

SC-3C (Phase 2 BUILD-3 sub-commit 4) had landed sub-commits 4a (Q7.b per-CPU CR3 cache; commit `88c7bc2`) and 4b (Q7.c TLB shootdown machinery; commit `3272840`) on 2026-05-10. 4c (Q7.c call sites + D14 negative-feature detection; commit `ead8b19`) closed early 2026-05-11.

Filed `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` (16.3 KB) consuming three verification reports. SC-3C contribution: Q7.b + Q7.c closed; rustfmt tooling gap surfaced; 7 Phase 2+ TLB management optimizations recorded as forward markers (PCID/ASID, T2 targeted shootdown with strictly-vs-loosely-accurate cpumask, lazy TLB freed_tables exception, shootdown batching, INVLPGB/RAR hardware-broadcast, Spectre-v2 BTB flush).

Prometheus integrated SC-3C closure into PHASE-2.md + INDEX.md via `Plans/DISPATCH-2026-05-11-sc-3c-closure-integration-prometheus.md`. Landed at commit `165a2e2`.

### 2.2 Mid-morning: Bible v1.6.0 + rename stragglers + PL CI fix

Bible Article 17 source-tree diagram still showed `altimitneo/` root after the 2026-05-10 rename audit. Filed `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md`; Prometheus landed Bible v1.6.0 amendment at commit `6231cf0`.

Rename audit straggler cleanup landed at commit `f44bf45` via `Plans/DISPATCH-2026-05-11-rename-mechanical-execution-prometheus.md`.

PL operational commit `e7782fc tools/xtask: keep IrqSafeMutex bench out of default CI` (CI reliability fix; parallel to BUILD-3 main work).

### 2.3 Afternoon: SC-4 cadence (THINK → Cerebro D5 → PLAN → BUILD)

Same-day SC-4 cadence — five-stage compression unprecedented for this project's pace:

**SC-4 THINK** filed at `Plans/SC-4-THINK-2026-05-11.md` (39.3 KB). 18 D-decisions (D1-D18 after Cerebro D5 reframing). 11 races enumerated. D5 flagged for Cerebro deep-research dispatch.

**Cerebro D5 dispatch + return + consumption**: `Plans/DISPATCH-2026-05-11-sc-4-d5-process-table-smp-prior-art-cerebro.md` (15.4 KB dispatch) returned as `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` (50.9 KB research artifact). **Architecturally-decisive contribution:** Cerebro reframed PM's D4 from "what lock type for PROCESSES?" to "should PROCESSES be on the hot path at all?" — Phase A retirement of `cr3_for_thread` in favor of Thread carrying `Arc<Pmap>` directly (Linux `loaded_mm` parallel). Four PM THINK corrections folded; D1 (SwitchMutex) and D7 (exiting flag) cross-validated by Cerebro citations (seL4 Peters/Heiser BKL evaluation; illumos doors `DOOR_INVALID` parallel; FreeBSD `allproc`).

PM filed `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md` capturing D4 finalized to Phase A only; Phase B (`arc-swap`) deferred unless Phase A blocker; sub-commit 5b scope reduced from ~500-800 to ~200-300 lines; D18 added (Phase A coverage gate); four Phase 2+ forward markers (FM-D5.1 spin::RwLock pathological warning, FM-D5.2 arc-swap Phase B fallback, FM-D5.3 illumos doors architectural reference, FM-D5.4 PROCESSES single-Mutex correctness boundary).

**SC-4 PLAN dispatch:** `Plans/DISPATCH-2026-05-11-sc-4-plan-stage-jarvis.md` (44 KB). Three sub-commits per D15 boundary. Voice-profile-calibrated dispatch (omitted explicit calibration framing, branching strategy, verification report format, hybrid-git posture; emphasized D-decision references, Cerebro citations, sub-commit boundaries, Phase 2+ forward markers embedded inline).

**SC-4 BUILD** — three Jarvis sub-commits landed clean:
- `c18646e` — 5a (handles + derivations SwitchMutex retrofit per D1/D2/D3/D6)
- `c02abf7` — 5b (Phase A: retire `cr3_for_thread`, Thread carries `pmap: Option<Arc<Pmap>>`; D7 exiting flag; D8 `Process.exit_state: AtomicU8` CAS)
- `57f3cb1` — 6 (D9 blocked-thread-kill walk + D10 `close_peer` my-side fix + D12 `zombify_thread` reentrant primitive)

Plus PL operational commit `5a88925 tools/xtask: retry transient neo-test scenario failures` (CI reliability, parallel to e7782fc pattern from SC-3C).

All gates passed locally + GitHub CI green (run 25705327371). `cargo fmt --check` BLOCKED (rustfmt tooling gap continues from SC-3C).

**SC-4 closure** filed at `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` (11.5 KB). Prometheus closure integration via `Plans/DISPATCH-2026-05-11-sc-4-closure-integration-prometheus.md` landed at commit `3367a4f`. Six new SC-4 forward markers added to PHASE-2.md "Forward reference: Phase 2+ capability + process-exit optimizations" subsection (FM-D5.1 through FM-D14).

### 2.4 Evening: Q9+ exploration + Q9 deferral landed

PM signaled Q9+ THINK exploration. Filed `Plans/Q9-EXPLORATION-2026-05-11.md` (22 KB). Read full ADR-013 §Q9 + BUILD-3 amendments at lines 1170-1199 (sub-commit 2 Q9 deferred with 8-experiment bisection evidence A-H; hypothesis informational not investigated).

**Critical insight surfaced:** SC-4 D8 `Process.exit_state: AtomicU8` CAS + D13 single-thread-per-process invariant **obsolete** Q9's `live_thread_count` machinery under Phase 2 scope. Under D13, thread reap IS process exit; CAS handles serialization.

**PM decision (alternative b from Q9-EXPLORATION §3):** Q9 deferred permanently in current shape. BSP-scans-all-zombie-queues interim is correctness-equivalent under D13. Phase 2+ multi-threaded user processes will lift the deferral; recommended Phase 2+ starting alternative is (c) per-CPU reapers without `live_thread_count`, using D8's CAS as unified Process-exit serialization primitive. No Cerebro dispatch needed.

Prometheus filed Q9 deferral documentation via `Plans/DISPATCH-2026-05-11-q9-deferral-documentation-prometheus.md`. Landed at commit `64a92e1`. FM-Q9 forward marker + ADR-013 §Q9 amendment + INDEX.md entry. **24 insertions across 3 files.**

### 2.5 Late evening: Phase 3 prep cadence

PL pushback on PM Phase 5 lean: "Why not Phase 3 → 4 → 5?" PM acknowledged the lean was wrong-shaped — project-aesthetics-driven (AI-native premise), not substrate-dependency-driven. Bible Article 10.2 NeoFS content-addressed dedup + Bible Article 10.3 typed-inode `.neom` refusal + ANSPEC §8.2 BLAKE3 model dedup all require Phase 3 NeoFS substrate. Phase 5 first would either ship without dedup, build dedup machinery to be redesigned, or use a throwaway shim. **Phase 3 → 4 → 5 is the correct order.**

**Phase 3 THINK v1** filed at `Plans/PHASE-3-THINK-V1-2026-05-11.md` (27.6 KB). Exploration shape, NOT final THINK. 10 Q-items:
- Q1 — Block layer abstraction (Q1.a/b/c: trait shape, sync vs async, per-device queues)
- Q2 — Block device drivers (Q2.a virtio-blk first, Q2.b SATA AHCI, Q2.c NVMe, Q2.d probe+enumeration)
- Q3 — DMA infrastructure (identity-mapped initially; IOMMU Phase 5+)
- Q4 — VFS layer (Q4.a typed-inode shape, Q4.b mount hierarchy, Q4.c path resolution, Q4.d caching deferred)
- Q5 — NeoFS native (Q5.a content-addressed BLAKE3 4 KiB blocks, Q5.b Merkle-tree inode-to-content, Q5.c on-disk format with revision number, Q5.d CoW + atomic superblock swap, Q5.e refcounted GC)
- Q6 — Foreign FS drivers (FAT32 read-only, ext4 read-only)
- Q7 — Syscall surface (POSIX-ish; capability discipline via ADR-010 extension to FileCap)
- Q8 — Boot-time integration (Q8.a hybrid initramfs+disk root, Q8.b test infrastructure extensions)
- Q9 — libneo + userspace integration (thin wrappers)
- Q10 — Phase 5+ prep (content-hash visibility via inode metadata or sys_file_content_hash)

**Proposed 4-BUILD split:**
1. Phase 3 BUILD-1: block layer + virtio-blk + minimal VFS (3-6 weeks; coordinate with ADR-016 A1 IRQ-safe heap)
2. Phase 3 BUILD-2: NeoFS native (on-disk format + content-addressing + CoW + GC)
3. Phase 3 BUILD-3: foreign FS read-only + initial userspace (FAT32, ext4, VFS syscalls)
4. Phase 3 BUILD-4: real hardware drivers (AHCI + NVMe) + bare-metal validation

**Three Cerebro dispatch candidates identified:**
1. NeoFS prior art (Plan 9 Venti, ZFS dedup, Btrfs CoW, IPFS, git object model) — anchors Q5.a-e
2. Storage driver prior art (Theseus, Redox, FreeBSD, Linux virtio-blk/AHCI/NVMe) — anchors Q1, Q2, Q3
3. VFS layer design prior art (Linux/BSD VFS, Plan 9 9P, V Operating System, seL4 cap-on-file) — anchors Q4, Q7

PM operational call: dispatch #1 first; sequential per SC-4 D5 pattern.

### 2.6 End of session: VMWare verification + Cerebro #1 dispatch

PL signaled VMWare Workstation hardware availability for verification (Bible Article 13 mandate). PM filed `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` (13.3 KB) — manual procedure for PL direct execution. **Explicitly labeled hypervisor-VM verification, NOT bare-metal verification** — VMWare uses VT-x/AMD-V hardware virtualization (substantially closer to bare-metal than QEMU TCG software emulation) but still a hypervisor. Bible Article 13 bare-metal mandate stays formally open. Estimated 2-4 hours wall-clock; non-blocking.

PM filed Cerebro #1 dispatch at `Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` (14.4 KB). 6 research questions anchoring Q5.a-e:
1. On-disk format for content-addressed FS (block size/dedup-granularity trade-offs, two-level indirection mechanics, free-space tracking)
2. Crash consistency for CoW content-addressed FS (superblock swap atomicity, inode-tree update ordering, journaling alternatives, sync syscall semantics)
3. Refcount-vs-mark-sweep GC for content-addressed blocks (production-system choices, hybrid strategies, failure-mode discipline)
4. Plan 9 Venti as closest-architectural-analog (on-disk format, what Venti got wrong, Venti vs git object model)
5. Mount-namespace + typed-inode integration (prior art for inode-level typing, capability-on-inode disciplines, foreign-FS type mapping)
6. Synchronous-blocking I/O at FS layer + composability with existing kernel discipline (Linux blk-mq vs older Unix patterns, interrupt-handler discipline, single-thread-per-process invariant preparation)

Explicitly NOT re-surveying: HF Xet / ZipLLM / Ollama (already covered in `docs/research/ALTIMIT-real-world-implementation.md`). Estimated 6-10 hour Cerebro session. Expected return: ~40-55 KB research artifact in shape of SC-4 D5 precedent.

PL authorized ferry. Cerebro #1 ready to ferry on PL signal at end of session.

---

## 3. Ratified decisions this session

### 3.1 SC-3C ratification (commit `165a2e2`)

- Q7.b per-CPU CR3 cache complete (commit `88c7bc2`).
- Q7.c TLB shootdown machinery complete (commit `3272840`).
- Q7.c call sites + D14 negative-feature detection complete (commit `ead8b19`).
- Seven Phase 2+ TLB management optimizations recorded as forward markers.

### 3.2 SC-4 ratification (commits `c18646e` → `c02abf7` → `57f3cb1`; closure integration `3367a4f`)

- D1-D18 decisions ratified per `Plans/SC-4-THINK-2026-05-11.md` + `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md`.
- **D4 finalized to Phase A only.** `cr3_for_thread` retired; Thread carries `pmap: Option<Arc<Pmap>>`; PROCESSES off the hot path. Linux `loaded_mm` parallel.
- **D8 `Process.exit_state: AtomicU8` CAS-claim** established as the unified Process-exit serialization primitive across natural-exit, forced-kill, and reap paths.
- **D13 single-thread-per-process invariant** formally codified for Phase 2 scope; Phase 2+ multi-thread is explicit forward marker.
- Six new Phase 2+ forward markers (FM-D5.1 through FM-D14) added to PHASE-2.md.

### 3.3 Q9 deferral ratification (commit `64a92e1`)

- Q9 deferred permanently in current shape under D13 invariant.
- BSP-scans-all-zombie-queues interim is the Phase 2 BUILD-3 final answer.
- D8 CAS obsoletes Q9's `live_thread_count` design under D13.
- ADR-013 §Q9 amended with "Q9 deferred permanently under D13 invariant" recording final disposition.
- FM-Q9 forward marker added to PHASE-2.md.
- Phase 2+ multi-thread will reopen Q9 design space; recommended starting point: `Plans/Q9-EXPLORATION-2026-05-11.md` §3 alternative (c) — per-CPU reapers without `live_thread_count`, composing with D8's CAS as unified serialization.

### 3.4 Phase ordering ratification (PM correction, PL signal)

- PM Phase 5 lean was wrong-shaped (project-aesthetics-driven, not substrate-dependency-driven).
- Correct order per Bible Article 13: Phase 3 → Phase 4 → Phase 5.
- Phase 3 NeoFS unblocks Phase 5 content-addressed model dedup (Bible 10.2 + ANSPEC §8.2) and typed-inode `.neom` refusal (Bible 10.3).

### 3.5 VMWare Workstation verification scoping

- VMWare Workstation = hypervisor-VM verification (VT-x/AMD-V hardware virtualization, real CPU, real APIC, real TSC).
- NOT bare-metal verification; Bible Article 13 bare-metal mandate stays formally open.
- Procedure document at `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md`.
- PL direct execution; non-blocking; ~2-4 hours wall-clock.

### 3.6 Cerebro #1 dispatch authorization

- PL authorized ferry of `Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`.
- Ferry pending; awaiting PL execution.
- Sequential dispatch pattern per SC-4 D5 precedent (Cerebro #2 + #3 after Cerebro #1 returns + PM consumes + THINK v2).

---

## 4. Forward queue (immediate next actions for new chat)

In priority order:

### 4.1 Cerebro #1 ferry + return monitoring (PL action)

PL ferries `Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` to Cerebro deep-research session. Expected return: research artifact at `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`, ~40-55 KB.

**On return:** PM (Cerebro/Claude) consumes; files PM-NOTE if any THINK v1 framing needs correction (parallel to PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md pattern); drafts Phase 3 THINK v2 with Q5.a-e finalized.

### 4.2 Phase 3 THINK v2 (PM action, post-Cerebro)

Updates Q5.a-e per Cerebro #1 findings. Adds detailed per-Q design with PM leans + alternatives. Estimated 30-40 KB output. Anchors ADR-021 (Phase 3 design substrate).

### 4.3 ADR-021 Phase 3 design substrate draft (PM action, post-THINK v2)

Analogous to ADR-013 for Phase 2 BUILD-3. Forward-looking Q1/Q2/Q3 shape per ADR convention. Covers Q1-Q10 from THINK v2.

### 4.4 Cerebro #2 + #3 dispatches (parallel, PM drafts; PL ferries)

Per Phase 3 THINK v1 §7:
- Cerebro #2: Storage driver prior art (Theseus, Redox, FreeBSD, Linux virtio-blk/AHCI/NVMe). Anchors Q1, Q2, Q3. Estimated 6-10 hours.
- Cerebro #3: VFS layer design prior art (Linux/BSD VFS, Plan 9 9P, V Operating System, seL4 cap-on-file). Anchors Q4, Q7. Estimated 4-8 hours.

Can ferry in parallel after Cerebro #1 returns.

### 4.5 Phase 3 BUILD-1 PLAN dispatch for Jarvis (PM action, post-ADR-021)

After ADR-021 lands: PM authors Phase 3 BUILD-1 PLAN dispatch. Sub-commit boundary proposal per THINK v1 §5: BUILD-1 sub-commits 0 (ADR-021 land) → 1 (block layer + DMA + PCI) → 2 (virtio-blk driver) → 3 (VFS minimal in-RAM) → 4 (initial sys_open/read/write/close + libneo) → 5 (test scenarios).

Coordinate with: ADR-016 A1 IRQ-safe heap + scheduler decoupling (Cerebro implementation work; ~1-2 hours estimated per ADR-016 §109).

### 4.6 VMWare hypervisor-VM verification (PL action, non-blocking, parallel)

PL executes per procedure in `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md`. Files verification report. PM consumes; surfaces to PHASE-2.md as cross-cutting verification artifact.

### 4.7 rustfmt tooling gap resolution (PL operational decision)

Per `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` §6.1 and SC-4 close report §3.4. Three options:
- Install rustfmt for pinned nightly `nightly-2026-04-16-x86_64-pc-windows-msvc`.
- Pin different nightly that ships rustfmt.
- Accept gap with alternative linting.

PL operational decision when convenient. Blocking eventual cleanup but not BUILD work.

### 4.8 Forward markers (no immediate action)

All Phase 2+ forward markers documented in PHASE-2.md. No action until their gating conditions are met (Phase 2+ multi-thread, Phase 5+ AI workloads, Phase 3 BUILD-1 start for ADR-016 A1).

---

## 5. Active artifacts (this session's outputs)

### 5.1 Close reports (3)

| File | KB | Purpose |
|---|---|---|
| `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` | 16.3 | SC-3C closure (Q7.b CR3 cache + Q7.c TLB shootdowns) |
| `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` | 11.5 | SC-4 closure (Q8.a/b/c capability table + blocked-kill walk) |
| `Plans/Q9-EXPLORATION-2026-05-11.md` | 22.0 | Q9+ exploration; PM-tier decision to defer permanently |

### 5.2 Dispatches filed today (10)

| File | KB | Agent | Status |
|---|---|---|---|
| `Plans/DISPATCH-2026-05-11-bible-amendment-article-17-prometheus.md` | 9.6 | Prometheus | Closed (commit `6231cf0`) |
| `Plans/DISPATCH-2026-05-11-rename-mechanical-execution-prometheus.md` | 8.8 | Prometheus | Closed (commit `f44bf45`) |
| `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md` | 42.2 | Jarvis | Closed (commits `88c7bc2`/`3272840`/`ead8b19`) |
| `Plans/DISPATCH-2026-05-11-sc-3c-closure-integration-prometheus.md` | 9.9 | Prometheus | Closed (commit `165a2e2`) |
| `Plans/DISPATCH-2026-05-11-sc-4-d5-process-table-smp-prior-art-cerebro.md` | 15.4 | Cerebro | Closed (research returned) |
| `Plans/DISPATCH-2026-05-11-sc-4-plan-stage-jarvis.md` | 44.0 | Jarvis | Closed (commits `c18646e`/`c02abf7`/`57f3cb1`) |
| `Plans/DISPATCH-2026-05-11-sc-4-closure-integration-prometheus.md` | 8.8 | Prometheus | Closed (commit `3367a4f`) |
| `Plans/DISPATCH-2026-05-11-q9-deferral-documentation-prometheus.md` | 6.9 | Prometheus | Closed (commit `64a92e1`) |
| `Plans/DISPATCH-2026-05-11-sub-commit-3-irqsafe-mutex-finalize-jarvis.md` | 10.1 | Jarvis | Closed (closure context) |
| **`Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`** | **14.4** | **Cerebro** | **READY TO FERRY** |

### 5.3 Research artifacts (1)

| File | KB | Purpose |
|---|---|---|
| `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` | 50.9 | Cerebro D5 return — reframed PM D4 question; substantively decisive for SC-4 |

### 5.4 THINK / exploration artifacts (3)

| File | KB | Purpose |
|---|---|---|
| `Plans/SC-4-THINK-2026-05-11.md` | 39.3 | SC-4 design space; 18 D-decisions |
| `Plans/Q9-EXPLORATION-2026-05-11.md` | 22.0 | Q9+ exploration; PM decision to defer permanently |
| `Plans/PHASE-3-THINK-V1-2026-05-11.md` | 27.6 | Phase 3 exploration THINK; 10 Q-items; 4-BUILD scope; 3 Cerebro dispatch candidates |

### 5.5 PM-notes (8)

| File | KB | Purpose |
|---|---|---|
| `Plans/PM-NOTE-2026-05-11-prometheus-closure-prep-and-audit-consumption.md` | 15.4 | Prometheus voice calibration capture |
| `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` | 20.3 | SC-3C D15 Cerebro consumption |
| `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md` | 17.8 | SC-4 D5 Cerebro consumption + D4 finalized to Phase A |
| `Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md` | 13.6 | Jarvis first-dispatch calibration |
| `Plans/PM-NOTE-2026-05-11-jarvis-voice-profile.md` | 14.6 | Jarvis voice profile (calibration 3-of-3) |
| `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` | 14.8 | ADR-019 bench threshold environment anchoring |
| `Plans/PM-NOTE-2026-05-11-ai-native-os-field-survey-consumption.md` | 12.8 | AI-native OS field survey consumption |

### 5.6 Verification reports (9)

| File | KB | Source |
|---|---|---|
| `Plans/SC-3C-SUB-COMMIT-4A-Q7B-CR3-CACHE-VERIFICATION-2026-05-11.md` | 2.3 | Jarvis |
| `Plans/SC-3C-SUB-COMMIT-4B-Q7C-TLB-SHOOTDOWN-MACHINERY-VERIFICATION-2026-05-11.md` | 2.5 | Jarvis |
| `Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-2026-05-11.md` | 4.1 | Jarvis |
| `Plans/SC-3C-CLOSURE-INTEGRATION-VERIFICATION-2026-05-11.md` | 4.8 | Prometheus |
| `Plans/SC-4-SUB-COMMIT-5A-HANDLES-DERIVATIONS-SWITCHMUTEX-VERIFICATION-2026-05-11.md` | 2.6 | Jarvis |
| `Plans/SC-4-SUB-COMMIT-5B-PHASE-A-RETIRE-CR3-FOR-THREAD-VERIFICATION-2026-05-11.md` | 2.9 | Jarvis |
| `Plans/SC-4-SUB-COMMIT-6-BLOCKED-THREAD-KILL-WALK-VERIFICATION-2026-05-11.md` | 2.5 | Jarvis |
| `Plans/SC-4-CLOSURE-INTEGRATION-VERIFICATION-2026-05-11.md` | 2.8 | Prometheus |
| `Plans/Q9-DEFERRAL-DOCUMENTATION-VERIFICATION-2026-05-11.md` | 2.0 | Prometheus |

### 5.7 Procedure document (1)

| File | KB | Purpose |
|---|---|---|
| `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` | 13.3 | Manual procedure for PL direct execution of VMWare Workstation hypervisor-VM verification |

### 5.8 Other (2)

| File | KB | Purpose |
|---|---|---|
| `Plans/AUDIT-2026-05-11-altimitneo-to-neoplutarch-stragglers.md` | 19.2 | Rename audit (carried from 2026-05-10) |
| `Plans/BIBLE-AMENDMENT-V1.6.0-VERIFICATION-2026-05-11.md` | 3.2 | Bible v1.6.0 amendment verification |
| `Plans/RENAME-MECHANICAL-EXECUTION-VERIFICATION-2026-05-11.md` | 3.5 | Rename mechanical execution verification |
| `Plans/IRQSAFE-MUTEX-SUB-COMMIT-3-VERIFICATION-2026-05-11.md` | 4.0 | IrqSafeMutex finalize verification |
| `Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md` | 12.8 | SC-3C closure pre-staging (parallel discipline experiment) |

---

## 6. Git state at handoff

### 6.1 Origin/main HEAD

```
64a92e1 docs: defer ADR-013 Q9 permanently under SC-4 D13 invariant; add FM-Q9 forward marker
```

### 6.2 Commits landed today (2026-05-11)

Chronological from oldest to newest within session window:

```
de8965c kernel/sync: add LockOwner field to SwitchMutex (ADR-020 foundation)
27d1b99 kernel/sync: introduce IrqSafeMutex (ADR-019 sub-phase A)
e7782fc tools/xtask: keep IrqSafeMutex bench out of default CI [PL operational]
88c7bc2 kernel/sched: cache active CR3 per CPU; skip redundant CR3 writes (Q7.b) [SC-3C 4a]
3272840 kernel/mm: introduce TLB shootdown machinery (Q7.c machinery; no callers) [SC-3C 4b]
f44bf45 docs: complete AltimitNeo→NeoPlutarch rename stragglers per audit [Prometheus]
ead8b19 kernel/mm: wire TLB shootdown into Pmap mutation sites (Q7.c call sites) [SC-3C 4c]
6231cf0 bible: Article 17 source tree diagram root altimitneo/ → neoplutarch/ (v1.6.0) [Prometheus]
165a2e2 docs/phases: integrate SC-3C closure section + Phase 2+ TLB forward markers [Prometheus]
5a88925 tools/xtask: retry transient neo-test scenario failures [PL operational]
c18646e kernel/sched: retrofit process handles for SMP [SC-4 5a]
c02abf7 kernel/sched: retire cr3_for_thread for Thread pmap [SC-4 5b]
57f3cb1 kernel/sched + ipc: handle blocked-thread kill walk [SC-4 6]
3367a4f docs/phases: integrate SC-4 closure section + Phase 2+ capability/exit forward markers [Prometheus]
64a92e1 docs: defer ADR-013 Q9 permanently under SC-4 D13 invariant; add FM-Q9 forward marker [Prometheus]
```

14 substantive commits + 2 operational PL CI fixes.

### 6.3 Working tree state

`Plans/` is entirely untracked per hybrid-git posture (per `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md`). All PM artifacts live locally; do NOT commit them.

`tools/wedge-investigation/` is also untracked — incidental observation noted in SC-4 closure report §6.4; not blocking; PL operational call on whether to formalize, archive, or leave as-is.

### 6.4 GitHub CI status

Latest run 25705327371 (SC-4 sub-commit 6): QEMU Integration Tests success. All three SC-4 sub-commits + closure integration + Q9 deferral commits passed CI green.

---

## 7. Stash state

Four historical stashes present:

```
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

All historical (from 2026-05-08/09 SC-3B + wedge investigation work). Not actively in flight. No action needed; can be `git stash drop` cleaned up at PL convenience. PM operational note: leave as-is unless they cause confusion.

---

## 8. Conventions reaffirmed and updated

### 8.1 Dispatch shorthand (established this session)

Per PL preference mid-2026-05-11:
- Dispatches as standalone Plans/ artifacts.
- Chat surfacing: `"This is the dispatch for X. File: <path>. Ready to ferry."`
- No full dispatch content in chat.
- Closure reports follow same shorthand: concise chat summary; full reports in Plans/.

### 8.2 PM operational posture (carried from 2026-05-10)

- PM owns operational + technical decisions.
- PL = dispatch courier + sign-off on legitimately PL-tier calls (ratifications, bible amendments, scope, major architectural pivots).
- Don't over-surface micro-decisions; make the call, document reasoning in artifact, ferry on.
- For genuinely stumped cases: escalate via Cerebro deep-research.

### 8.3 "Have I framed the right question?" pre-dispatch gate (established this session)

Per SC-4 D5 Cerebro reframing lesson: PM cadence now incorporates a pre-dispatch gate. Before dispatching Cerebro, PM checks whether the question being asked is the right one or whether the framing itself is wrong-shaped. Cerebro is safety net when PM misses it; the dispatch question-framing is now a PM-level checkpoint.

Applied this session: Phase 3 NeoFS prior-art Cerebro dispatch framing was explicitly checked against substrate (THINK v1 §4 Q5.a-e; ANSPEC §8.2 BLAKE3 decided; HF Xet already surveyed by ALTIMIT artifact) before drafting.

### 8.4 Perf-gate environment triplet discipline (carried from SC-3C closure)

Three distinct environments with different perf characteristics:
- Local Docker QEMU TCG (current testing baseline)
- GitHub-runner Docker (CI)
- Future bare-metal CI

Thresholds must specify environment. ADR-019 bench threshold 6000 ns is local Docker QEMU TCG anchored; 50 ns native-hardware target deferred to Phase 2+ bare-metal CI per `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md`.

### 8.5 VMWare Workstation explicitly NOT bare-metal (established this session)

VMWare uses VT-x/AMD-V hardware virtualization — substantially closer to bare-metal than QEMU TCG but still a hypervisor. Explicitly labeled "hypervisor-VM verification" in procedure documents. Bible Article 13 bare-metal mandate stays open until actual bare hardware becomes available.

### 8.6 Bible Article 13 phase ordering is dependency-aware (established this session)

PM Phase 5 lean (after Phase 2 BUILD-3 close) was wrong-shaped — project-aesthetics-driven, not substrate-dependency-driven. Bible Article 13 ordered phases dependency-aware. PM should not skip phases on aesthetic grounds. Phase 3 → 4 → 5 is the correct order.

### 8.7 Jarvis post-calibration dispatch shape (stable per 3-of-3 calibration)

Per `Plans/PM-NOTE-2026-05-11-jarvis-voice-profile.md` §5.1-5.3:
- Omit explicit calibration framing.
- Omit branching strategy discussion.
- Omit verification report format prescriptions.
- Omit hybrid-git posture explanation.
- Emphasize: D-decision references, Cerebro-corrected citations, sub-commit boundaries, forward markers embedded inline as code comments.

SC-4 PLAN dispatch applied this shape successfully; three sub-commits landed clean.

### 8.8 Prometheus dispatch shape (stable)

Doc-only single commits; clear file targets; verification report format matches prior precedents; closure protocol: SHA + report filename + diff summary back to PL via Prometheus.

---

## 9. PM tone and operational notes

PM voice is terse, technical, decisive. Documents reasoning in artifacts; surfaces concise summaries to PL.

Established posture:
- Make operational calls; document in artifacts; don't over-surface micro-decisions.
- Acknowledge errors directly when surfaced (Phase 5 lean correction was a clean acknowledge-and-pivot).
- ALTIMIT-faithful discipline per `docs/research/ALTIMIT-real-world-implementation.md`: "do less in the kernel, more rigorously." Anti-scope is a feature.
- When a mechanism's original justification is removed (Q9 `live_thread_count` under D13), delete or defer rather than carrying forward as latent complexity (parallel to §Q5 amendment discipline from ADR-013 sub-commit 2).

PM ferry pattern: surface 2-3 short paragraphs with "ready to ferry" framing; PL fronts the agent session; PM consumes returns and synthesizes.

---

## 10. Open items / future-phase forward state

### 10.1 Phase 2+ forward markers (no immediate action)

All recorded in PHASE-2.md "Forward reference: Phase 2+ TLB management optimizations" and "Forward reference: Phase 2+ capability + process-exit optimizations" subsections:

**SC-3C-introduced:**
- PCID/ASID enablement
- T2 targeted shootdown with strictly-vs-loosely-accurate cpumask
- Lazy TLB freed_tables exception
- Shootdown batching / mmu_gather analogue
- INVLPGB/RAR hardware-broadcast (AMD Zen 4+ / Intel 4th-gen Xeon+)
- Spectre-v2 BTB flush on context switch

**SC-4-introduced:**
- FM-D5.1: spin::RwLock pathological at 4-64 CPU range (5× regression vs Mutex per Cerebro §5.4)
- FM-D5.2: arc-swap Phase B fallback deferred unless Phase A blocker
- FM-D5.3: illumos doors architectural reference for cross-process IPC (Phase 3 capability-streaming variant)
- FM-D5.4: PROCESSES single-Mutex correctness boundary validated by FreeBSD allproc + illumos pidlock at NeoPlutarch scale
- FM-D13: Single-thread-per-process invariant load-bearing for many design choices
- FM-D14: COMPLETED_EXIT_CODES growth bound; sys_wait consumer deferred

**Q9-introduced:**
- FM-Q9: Per-CPU reaper distribution with multi-thread support deferred to Phase 2+ multi-threaded user processes per ADR-013 §Q9 amendment

### 10.2 ADR forward markers

- ADR-012 Arc-aware slab integration: deferred from BUILD-2 post-close; Phase 3 BUILD-1 OBSERVE pass should re-examine since block-device + FS structures will have new allocation patterns.
- ADR-012 `linked_list_allocator` replacement: possible Phase 3+ if FS workloads stress kernel heap differently than Phase 2.
- ADR-016 A1 IRQ-safe heap + scheduler decoupling: explicitly named Phase 3 implementation; Cerebro work ~1-2 hours estimated.
- ADR-019 bench native-hardware 50 ns target: deferred to Phase 2+ bare-metal CI.

### 10.3 Verification mandates

- Bible Article 13 bare-metal multi-CPU verification at Phase 2 BUILD-3 close: PENDING. VMWare hypervisor-VM verification (`Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md`) is intermediate gate; bare-metal stays open.
- rustfmt tooling gap: PL operational decision pending.

### 10.4 Phase 3 / 4 / 5 / 6 / 7 / 8+ scoping

- **Phase 3:** THINK v1 filed; awaiting Cerebro #1 ferry + return + THINK v2 + ADR-021 + BUILD-1 PLAN.
- **Phase 4:** No PM cadence work yet; follows Phase 3 closure.
- **Phase 5:** ANSPEC-001 design substrate already exists at `docs/specs/neoplutarch-ai-subsystem-spec.md`; THINK work follows Phase 4.
- **Phase 6+:** Far future; no PM cadence work.

---

## 11. Calibration lessons from this session

### 11.1 Cerebro D5 was the highest-value Cerebro contribution to date

Reframed PM's D4 from a wrong-shaped lock-type question to the substrate-dependency-correct "should PROCESSES be on the hot path?" question. Reduced sub-commit 5b scope from ~500-800 lines to ~200-300 lines. Cross-validated D1 + D7 via independent prior-art citations (seL4, illumos, FreeBSD).

Lesson: Cerebro is the safety net when PM misses substrate-framing errors. The "have I framed the right question?" pre-dispatch gate is now PM cadence.

### 11.2 PM Phase 5 lean was a recurrence of the same framing-error class

Phase 5 lean: project-aesthetics ("AI-native premise") not substrate-dependencies (Phase 3 unblocks Phase 5). PL caught it; PM acknowledged and pivoted. This is the same shape as the original D4 framing error — reasoning from project posture instead of substrate.

Lesson: PM cadence vigilance for this specific class of error. Substrate-check before posture-check.

### 11.3 Same-day cadence is sustainable for tightly-scoped work

SC-4 cadence: THINK → Cerebro dispatch + return + consumption → PLAN → BUILD all on 2026-05-11. Five-stage compression unprecedented. Worked because:
- D-decision list was well-anchored before Cerebro dispatch.
- Cerebro reframing was substantive but surgical (D4 only).
- Jarvis post-calibration dispatch shape kept PLAN-stage tight.
- BUILD was three small sub-commits, not one large one.

Not generalizable — Phase 3 BUILD-1 will not have a same-day cadence; it's a wider design surface.

### 11.4 Pre-staging discipline (SC-3C → SC-4 cadence comparison)

SC-3C pre-staged closure documentation via Prometheus during BUILD (Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md). SC-4 did not; closure documentation drafted post-BUILD via direct integration dispatch.

SC-3C cadence was cleaner. PM operational note for future BUILDs: consider parallel Prometheus pre-staging during BUILD wall-clock.

### 11.5 Q9 deferral as ALTIMIT-faithful discipline application

Q9's `live_thread_count` mechanism's original justification (multi-thread "is this the last?") was obsoleted by D13 single-thread invariant + D8 CAS-claim. Per ALTIMIT-faithful discipline ("when a mechanism's original justification is removed, delete or defer"), Q9 deferred permanently. No code change; pure documentation work; ~24 line insertions across 3 files.

Compare to §Q5 amendment lesson: "future cleanup proposals must cleanly separate 'is this mechanism still needed?' from 'are we changing the container shape?'" — Q9 deferral applies this cleanly because D13 + D8 changed the substrate that determined whether live_thread_count was needed at all.

### 11.6 VMWare Workstation framing precision

Initial PM instinct was to call VMWare "bare-metal" because VT-x/AMD-V uses real CPU. PM caught the imprecision in drafting; labeled as hypervisor-VM explicitly. Bible Article 13 bare-metal mandate stays open as a separate workstream. The semantic precision matters because the project's verification disciplines depend on accurate environment labeling (parallel to ADR-019 bench threshold environment-anchoring discipline).

---

## 12. Cross-references

### 12.1 ADR documents touched this session

- `docs/ADR-013-phase-2-smp-retrofit.md` — added "Q9 deferred permanently under D13 invariant" amendment (commit `64a92e1`).
- `docs/ADR-019-irq-safe-spinlock-discipline.md` — bench threshold environment clarification per PM-NOTE.

### 12.2 Bible amendments this session

- v1.6.0: Article 17 source-tree diagram root `altimitneo/` → `neoplutarch/` (commit `6231cf0` per Prometheus).

### 12.3 Phase document touched

- `docs/phases/PHASE-2.md` — SC-3C section + Phase 2+ TLB forward markers (commit `165a2e2`); SC-4 section + Phase 2+ capability/exit forward markers (commit `3367a4f`); FM-Q9 forward marker (commit `64a92e1`).

### 12.4 Specs unchanged

- `docs/specs/neoplutarch-ai-subsystem-spec.md` — ANSPEC-001; Phase 5 substrate; no changes this session.
- `docs/specs/neoplutarch-ai-capability-semantics-spec.md` — Phase 7 capability semantics; no changes.

### 12.5 Research artifacts referenced

- `docs/research/ALTIMIT-real-world-implementation.md` (42.6 KB) — referenced as substrate for Phase 3 Cerebro #1 dispatch context (HF Xet / ZipLLM / Ollama prior art coverage).

### 12.6 Prior handoffs in series

- `Plans/HANDOFF-2026-05-05-build3-sub-commit-1a-close.md` (28.4 KB)
- `Plans/HANDOFF-2026-05-06-end-of-day-kaslr-blocked.md` (21.5 KB)
- `Plans/HANDOFF-2026-05-07-end-of-day-adr-015-in-flight.md` (27.6 KB)
- `Plans/HANDOFF-2026-05-08-wedge-investigation-mid-campaign.md` (17.9 KB)
- `Plans/HANDOFF-2026-05-09-sc-3b-sub-step-5b-in-flight.md` (33.0 KB)
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` (28.7 KB)
- `Plans/HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md` (48.0 KB)

This handoff is the next in series; matches 13-section structure of prior handoffs.

---

## 13. Opening message for the new chat

Copy-paste this block as the first message in the new chat session. It includes the three project files (Bible, ANSPEC, ALTIMIT research) plus this handoff doc as attachments — drag those four files into the new chat alongside this opening message.

---

**[BEGIN OPENING MESSAGE]**

Continuing NeoPlutarch PM cadence from end-of-day 2026-05-11. Please read `Plans/HANDOFF-2026-05-11-end-of-day-phase-2-build-3-substantially-complete-phase-3-think-v1.md` first for full context; it captures complete operational state for new-chat bootstrapping.

**TL;DR of state:**

- **Project:** NeoPlutarch (formerly AltimitNeo) — AI-native OS in Rust at `E:\AI Automation\AltimitNeo\`, GitHub `https://github.com/SecondStageTurbine/NeoPlutarch`.
- **Agent roster:** Cerebro=Claude (this assistant, PM + deep research), Jarvis=Codex (heavy kernel coding), Prometheus=Qwen3.6-27B (docs/cleanup). Janus retired.
- **Origin/main HEAD:** `64a92e1` (Q9 deferral documentation, end of 2026-05-11 work).
- **Phase 2 BUILD-3:** Substantially complete. All ADR-013 Q-items resolved (Q1-Q8 implemented; Q9 deferred permanently under SC-4 D13 single-thread-per-process invariant + D8 `Process.exit_state` CAS obsoleting `live_thread_count`).
- **Phase 3 (NeoFS + block layer + real disk I/O):** THINK v1 filed at `Plans/PHASE-3-THINK-V1-2026-05-11.md` (27.6 KB; 10 Q-items; 4-BUILD scope; 3 Cerebro dispatch candidates).

**Immediate next action:** Cerebro #1 dispatch (`Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`) is ready to ferry on PL signal. PL authorized ferry at end of last session; not yet executed. Expected return: `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`, ~40-55 KB, anchoring Q5.a-e (NeoFS native layer design).

**On Cerebro #1 return:** PM (me) consumes, files PM-NOTE if THINK v1 framing needs correction (parallel to SC-4 D5 pattern), drafts Phase 3 THINK v2 with Q5.a-e finalized, then drafts ADR-021 Phase 3 design substrate.

**Non-blocking parallel work available:**
- VMWare Workstation hypervisor-VM verification per `Plans/PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` (PL direct execution; ~2-4 hours).
- rustfmt tooling gap resolution (PL operational decision; carried from SC-3C).

**Operational discipline reminders:**
- Dispatch shorthand: file dispatches as standalone Plans/ artifacts; chat surfacing pattern is `"This is the dispatch for X. File: <path>. Ready to ferry."` — no full content in chat.
- Plans/ is local-only per hybrid-git posture; PM artifacts don't push to GitHub.
- PM owns operational + technical decisions; PL ratifies legitimately PL-tier calls (scope, bible amendments, major architectural pivots). Don't over-surface micro-decisions.
- "Have I framed the right question?" pre-dispatch gate is PM cadence per SC-4 D5 lesson.
- VMWare = hypervisor-VM, NOT bare-metal; Bible Article 13 bare-metal mandate stays open.
- Phase ordering is dependency-aware per Bible Article 13: Phase 3 → 4 → 5 → 6 → 7 → 8+. PM Phase 5 lean post-SC-4 was wrong-shaped; corrected.

**Standby for PL signal.** PM next action: drive Phase 3 cadence — Cerebro #1 ferry → return consumption → THINK v2 → ADR-021 → BUILD-1 PLAN dispatch for Jarvis.

**[END OPENING MESSAGE]**

---

**Files to attach to new chat:**
1. `docs/bible.md` (NeoPlutarch constitution)
2. `docs/specs/neoplutarch-ai-subsystem-spec.md` (ANSPEC-001, Phase 5 substrate)
3. `docs/research/ALTIMIT-real-world-implementation.md` (research artifact informing project posture)
4. `Plans/HANDOFF-2026-05-11-end-of-day-phase-2-build-3-substantially-complete-phase-3-think-v1.md` (this handoff)

Optional additional attachments for deeper Phase 3 context:
- `Plans/PHASE-3-THINK-V1-2026-05-11.md` (current Phase 3 THINK)
- `Plans/DISPATCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` (ready-to-ferry dispatch)
- `Plans/Q9-EXPLORATION-2026-05-11.md` (Phase 2 BUILD-3 substantial-completion declaration)

---

**End of handoff.**

*Phase 2 BUILD-3 substantially complete. Phase 3 cadence initialized. Cerebro #1 ready to ferry. Next PM cadence: consume Cerebro #1 return; draft THINK v2 + ADR-021; queue Cerebro #2 + #3 in parallel; draft Phase 3 BUILD-1 PLAN dispatch for Jarvis.*
