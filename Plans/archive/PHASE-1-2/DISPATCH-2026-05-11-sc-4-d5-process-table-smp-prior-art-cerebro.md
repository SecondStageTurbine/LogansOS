# DISPATCH — SC-4 D5: multi-CPU process-table lookup prior art (Cerebro)

**Date filed:** 2026-05-11
**From:** PM
**To:** Cerebro (Claude-backed deep-research agent per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3)
**Ferry:** PL ferries to Cerebro session.
**Status:** SC-4 THINK-stage Cerebro dispatch. SC-3C D15 analogue. Resolves D4 in `Plans/SC-4-THINK-2026-05-11.md` (multi-CPU process-table lookup design choice). Output gates SC-4 PLAN-stage dispatch authoring.

---

## 1. Mandate

Deep-research the prior art on multi-CPU process-table lookup in production-grade SMP kernels. NeoPlutarch's `PROCESSES: Lazy<Mutex<BTreeMap<ProcessId, Arc<Process>>>>` registry is a single-mutex bottleneck under SMP: every cross-process context switch calls `cr3_for_thread(pid)` which takes the registry lock. Under N CPUs with frequent cross-process context switches, contention is real and the lock type choice matters.

PM's THINK-stage lean is RCU-style `AtomicPtr<Arc<BTreeMap>>` with epoch-based reclamation. PM is not confident this is the right answer for a small-N kernel; alternatives (per-CPU caches, sharded BTreeMap, SwitchRwLock-if-designed, "just use a big-RWLock") need evaluation against actual prior art rather than PM's training-data heuristics.

**Six research questions in §3 below.** Output is a Cerebro research artifact at `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` mirroring the SC-3C D15 research artifact format.

## 2. Background

### 2.1 What NeoPlutarch has today

```rust
// kernel/src/sched/process.rs lines 305-308
static PROCESSES: Lazy<Mutex<BTreeMap<ProcessId, Arc<Process>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
static COMPLETED_EXIT_CODES: Lazy<Mutex<BTreeMap<ProcessId, i32>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
```

- `spin::Mutex` (single-CPU spinlock from `spin` crate).
- BTreeMap<ProcessId, Arc<Process>>.
- All cross-process operations take this lock multiple times in sequence (see `Plans/SC-4-THINK-2026-05-11.md` §2.2 hot-path table).
- `cr3_for_thread(pid)` is the hottest reader — called on every cross-process context switch.

### 2.2 What changed with SC-3C

SC-3C closed Q7 (per-CPU CR3 cache + cross-CPU TLB shootdowns). Cross-process context switches now skip CR3 writes on same-process resumes; only true process-transitions hit CR3-write + PROCESSES lookup. Workload-dependent, but in the inference-scheduler vision (Bible Article 23), cross-process switches will be frequent (multiple inference clients sharing accelerator scheduling time).

### 2.3 What scale targets NeoPlutarch has

Per Bible + ANSPEC-001:
- Consumer/embedded boot targets (laptop, workstation, edge inference appliance).
- Typical process count: 10-100 user processes. Heavy multi-tenant inference could reach low-thousands.
- CPU count: 4-64 cores at this scale.
- Cross-process context switch rate: hard to predict; could be high during inference scheduling.

The "right" SMP discipline depends on this scale. Linux RCU is engineered for thousand-CPU machines with millions of processes; NeoPlutarch is two orders of magnitude smaller. A simpler design that doesn't pay RCU's complexity tax may be the better fit.

### 2.4 What PM's THINK leaned and why it's uncertain

PM leaned RCU-style AtomicPtr<Arc<BTreeMap>> in §5.1 of SC-4 THINK because:
- Reads via atomic pointer load are lock-free → no per-CPU contention.
- Writes via copy-on-write of the BTreeMap inside a writer's lock → infrequent.
- Old tables retired via natural Arc Drop when the last reader releases.

PM is not confident this is best because:
- "Copy-on-write the whole BTreeMap on every process register/remove" is expensive — every spawn + every exit allocates + clones the entire BTreeMap.
- Real RCU (Linux) doesn't do this; it uses RCU-protected pointer + linked-list style update-in-place.
- Simpler designs may exist that PM hasn't surveyed (per-CPU pid→Process pointer cache; hazard pointers; just a global RWLock).
- SC-3C D15 taught: PM's training-data prior-art reasoning is sometimes imprecise; Cerebro pass corrects.

## 3. Research questions

### Q1 — Linux RCU `task_struct` lookup

How does Linux protect `find_task_by_pid_ns` and related task-lookup functions?

Specific subquestions:
- What's the data structure (currently `idr` per `pid_namespace`, historically `tasklist` linked list)?
- What's the read-side primitive (`rcu_read_lock` + which lookup function)?
- What's the write-side serialization for adding/removing tasks (`tasklist_lock`, `pidmap_lock`, `pid->lock`)?
- How does the task→pid linkage get cleaned up on exit? When is the task actually freed?
- What's the grace-period requirement on the read side? How does `synchronize_rcu` fit in?
- What's the interaction with `get_task_struct` / `put_task_struct` reference counting?

Cite: Linux source (specific file paths + line numbers if findable), kernel documentation, LWN articles.

### Q2 — seL4 thread/capability lookup pattern

How does seL4 handle multi-CPU thread/cap lookup?

Specific subquestions:
- seL4 multikernel design — is it big-kernel-lock or fine-grained per-core?
- What's the CSpace/CNode lookup synchronization? Are capabilities mutable from multiple cores concurrently?
- For cross-thread operations (IPC send to thread on another core, kill thread on another core), what's the lock-ordering rule?
- How does seL4 prove these lookups are race-free? Is it formal verification or runtime invariants?

Cite: seL4 manual, seL4 mailing list, papers (Klein et al. seL4 formal verification, Heiser's multikernel talks).

### Q3 — Solaris/illumos `proc` table SMP discipline

Solaris/illumos has a process table that scales to large SMP. Key questions:

- What's the process-table data structure (`procdir`, `proc_t`, the lookup hash)?
- What synchronization protects lookups (`pidlock`, `p_lock`, hash-bucket locks)?
- Reference counting pattern: how do `proc_hold` and `proc_rele` (or equivalents) compose with exit?
- Doors IPC — conceptually similar to NeoPlutarch's Channel-grant pattern. How do doors handle cross-process synchronization at lookup time?
- Lock-ordering rule for multi-process operations.

Cite: illumos source (github.com/illumos/illumos-gate), Solaris Internals book references where applicable, Sun/Oracle papers.

### Q4 — FreeBSD `pfind` + per-process locking

FreeBSD has `pfind(pid)` and per-`proc` locks. Specific questions:

- What's the `allproc` list synchronization (`allproc_lock`)?
- Per-`proc` mutex (`p_mtx`) — when is it acquired, when is it released?
- Lock-ordering rule documented in source comments or developer handbook?
- How does `pfind` compose with `kern_kill` / `kern_wait`? Lifetime model.
- Is there an RCU-like primitive (smr / sx slock)?

Cite: FreeBSD source (cgit.freebsd.org or github mirror), FreeBSD Developer's Handbook, kernel mailing list threads.

### Q5 — Alternative simpler designs that may match NeoPlutarch's scale

PM is interested in designs that are SIMPLER than full RCU but still SMP-correct. Survey:

- **Per-CPU `(ProcessId, Arc<Process>)` cache** — each CPU caches the last-looked-up process; cache invalidation on register/remove. What's the prior art? Linux per-CPU runqueue's `cpu_curr` is conceptually related.
- **Hazard pointers** (Folly's `HazardPointer`, Linux's `typesafe_by_rcu`) — readers publish what they're reading; writers wait for hazard clearance before reclaiming.
- **`BTreeMap` sharded by `ProcessId.0 % NUM_SHARDS`** — N independent locks, lookup hashes to one shard.
- **Just a global RWLock** — many readers; rare writers; what's the contention story under realistic NeoPlutarch workloads (cross-process context switch rate)?
- **Read-Copy-Update with explicit pointer swap, no epoch reclamation** — the "swap the whole BTreeMap on every write" pattern PM leaned. Is this used anywhere in production? When?

For each alternative, characterize: complexity, expected performance at NeoPlutarch's scale (10-100 processes, 4-64 CPUs), prior-art examples.

Cite: Folly source, Linux source, McKenney's "Is Parallel Programming Hard" book references, paper references.

### Q6 — Cost/benefit framing for small-N kernels

At NeoPlutarch's expected scale (10-100 processes, 4-64 CPUs, mixed but not pathological cross-process context-switch rate), what does the right SMP discipline look like? Specifically:

- Does Linux RCU's complexity earn its keep at this scale, or is it over-engineering?
- What does seL4 do at smaller scales (seL4 is often used in embedded/microkernel contexts with similar scale)?
- Is there a kernel literature consensus on "the right SMP discipline for a 32-CPU process table"?
- What does Redox (Rust microkernel, modern, somewhat similar in scope) actually do?
- What does the unikernel literature say (Mirage, IncludeOS, etc.) about cross-process synchronization (where applicable — many unikernels are single-process)?

Synthesize a recommendation: given NeoPlutarch's scale, what's the design choice PM should ratify for D4?

## 4. Substrate references for Cerebro

PM has already surveyed (so Cerebro can verify rather than re-research):
- `kernel/src/sched/process.rs` lines 305-720 — PROCESSES registry + cross-process operations.
- `kernel/src/ipc/mod.rs` close_peer + GrantBlocked / RecvGrantBlocked.
- `Plans/SC-4-THINK-2026-05-11.md` §2-§7 — 11 races, 17 D-decisions, PM leans documented.
- `Plans/SC-3C-THINK-2026-05-10.md` and `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` — pattern reference for the Cerebro research output shape.

## 5. Output expectations

### 5.1 Artifact filename

`Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md`

### 5.2 Format

Mirror the SC-3C D15 research artifact (`Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md`). Structure:

1. Header (date, author=Cerebro, type=research-artifact, scope, status).
2. Executive summary — 3-5 paragraphs synthesizing the answer to Q6 (the cost/benefit framing question). This is the load-bearing output.
3. Per-question deep-dive (Q1-Q6) with:
   - Primary citations (real URLs, accessible documentation, source-code line numbers where available).
   - Specific implementation pattern descriptions (with code-equivalent pseudocode where helpful).
   - PM-relevance commentary: how does this apply to NeoPlutarch's scale?
4. Citation precision corrections — flag any places where PM's THINK-stage characterization is imprecise or wrong (mirror SC-3C D15's seven-citation-corrections pattern).
5. PM-leaned forward markers — items Cerebro surfaces that PM should consider as Phase 2+ work even if not directly needed for D4 resolution.
6. Recommended D4 resolution — Cerebro's synthesized recommendation. PM may or may not adopt; this is input, not authority.

### 5.3 Citation discipline

Every factual claim about a specific kernel's implementation must cite:
- Source file path + line numbers OR
- Official documentation URL + section OR
- Specific paper / book / mailing-list thread.

No claims based on "general knowledge of how Linux works" without citation. The SC-3C D15 dispatch returned seven citation-precision corrections (Linux cpumask eventually-consistent vs strictly-accurate, FreeBSD API signature, curcpu_cb timing, broadcast_tlb_flush vs broadcast_flush_tlb_all, consider_global_asid sampling gate, seL4 mailing-list date, FreeBSD-EN-18:07 verbatim quote). Same precision standard applies here.

If a primary source is paywalled (academic paper, book), cite + summarize from publicly-available secondary sources (LWN, blog posts, ACM previews) while noting the primary citation. Don't hallucinate paper contents.

### 5.4 Length

SC-3C D15 returned 39.7 KB. Same target range (30-50 KB). Don't pad; if Q1-Q6 can be answered comprehensively in 20 KB, fine. If thorough answers require 60 KB, fine. Length is a tool, not a target.

### 5.5 What PM does with the output

PM consumes the artifact, updates SC-4 THINK D4 with Cerebro's recommendation (or PM's revised lean if PM disagrees with citations), files a consumption PM-NOTE, then authors the SC-4 PLAN-stage dispatch for Jarvis.

If Cerebro's research surfaces issues PM didn't anticipate (e.g., "actually D8 also needs to consider X"), PM revises THINK and may issue follow-up dispatches.

## 6. Out of scope

Cerebro is NOT asked to:

- Make implementation decisions for NeoPlutarch (D4 finalization is PM's call after consuming the research; Cerebro recommends, PM decides).
- Survey AI-native OS prior art (already covered by `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md`).
- Cover Q8.a or Q8.c design space; only Q8.b PROCESSES registry retrofit is in scope here. The other Q8 D-decisions are PM-tier.
- Estimate runtime performance numbers — characterize complexity and prior-art usage, not benchmark.

## 7. Time budget

PM estimate: 4-8 hours of Cerebro deep-research session time. SC-3C D15 came back within a similar window. If the research expands significantly (e.g., a sub-question opens up a substantial sub-area), Cerebro should pause and surface to PM via PL ferry rather than push through.

## 8. Stop conditions

- If a research question turns out to be malformed (e.g., Q3 — Solaris/illumos sources are inaccessible for some reason), Cerebro surfaces and PM re-scopes.
- If Cerebro finds the answer is "the question doesn't apply at NeoPlutarch's scale" (e.g., "RCU is unequivocally wrong for 10-100 processes; everyone agrees"), surface that as the finding rather than padding the artifact.
- If the cost/benefit synthesis (Q6) can't be made conclusive without information PM doesn't have (e.g., expected workload patterns), Cerebro surfaces the conditionals and PM either provides the info or accepts the conditional answer.

## 9. Closure protocol

After Cerebro files the artifact:

1. Cerebro reports back to PL with: (a) artifact filename, (b) brief summary of Q6 synthesis recommendation, (c) any citation-precision corrections to PM's THINK characterizations, (d) time-budget actual.
2. PL ferries to PM.
3. PM consumes; updates SC-4 THINK D4 disposition; files consumption PM-NOTE (`Plans/PM-NOTE-2026-05-MM-sc-4-d5-cerebro-consumption.md`).
4. PM authors SC-4 PLAN-stage dispatch for Jarvis.

---

## 10. Cross-references

**SC-3C D15 pattern reference:**
- `Plans/DISPATCH-2026-05-10-sc-3c-d15-tlb-shootdown-prior-art-cerebro.md` — the dispatch shape.
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` — the output shape (39.7 KB; seven citation corrections; six forward markers).
- `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` — PM consumption pattern.

**SC-4 THINK input:**
- `Plans/SC-4-THINK-2026-05-11.md` — full design space; D4 is the open decision this dispatch resolves.

**Substrate anchors:**
- `kernel/src/sched/process.rs` — PROCESSES registry + cross-process operations.
- `kernel/src/sched/scheduler.rs` (746 lines) — context-switch path where cr3_for_thread is called.

**Cerebro voice-profile context:**
- `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` (58.7 KB) — prior Cerebro output, demonstrates voice + citation discipline expected here.

**Agent lane:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3 — Cerebro lane definition.

---

**End of dispatch.**

*SC-4 D5: multi-CPU process-table lookup prior art. Six research questions (Linux RCU task_struct, seL4 thread/cap lookup, Solaris/illumos proc table, FreeBSD pfind, alternative simpler designs, small-N cost/benefit synthesis). Output: `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md`. Format mirrors SC-3C D15. Time budget 4-8 hours. PM consumes and finalizes D4; SC-4 PLAN-stage dispatch follows.*
