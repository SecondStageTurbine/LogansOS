# CEREBRO-OUTPUT: Phase 2 OBSERVE — landed-work inventory + remaining-surface map

- **Author:** Cerebro (Claude — deep research + synthesis lane)
- **Date:** 2026-05-24
- **Scope binding:** `Plans/DISPATCH-2026-05-24-phase-2-observe-cerebro.md` (PL-ratified OBSERVE→THINK→PL-ratification→PLAN→/simplify→BUILD-N cadence; OBSERVE as a single Cerebro dispatch). Worker-tier execution; PM consumes, ratifies, then dispatches THINK separately.
- **Repo state consumed:** `origin/main` at `df5bfb5` (dispatch commit). Pre-Phase-2 audit chain landed and closed: `5da364d` (scope), `c2a4411` (Day 1), `bb1b679` (Day 2), `199fb51` (audit close), `9a282f2` (F-005 fix), `69df60b` (F-005 close report), `5884ffd` (F-005 PM-DISPOSITION — Phase 2 PLAN unblocked).
- **Discipline (per dispatch §6):** Inventory + question-surfacing only. I do **not** answer THINK questions (§5 enumerates, does not resolve), do **not** write ADRs or bible amendments, do **not** amend any audit artifact, do **not** scope-decide Phase-2-vs-Phase-3 ambiguities (flagged to §7 for PM).
- **Headline:** Phase 2's core — capability foundation (BUILD-1), buddy+slab allocators (BUILD-2), and the SMP retrofit (BUILD-3, Q1–Q8) — is **substantially landed and has been since 2026-05-11**. The dispatch's framing that "buddy+slab allocators are pending" is inaccurate: BUILD-2 closed at `f5a4984` (buddy live; slab live for Thread; Process/Channel caches declared-but-unused pending Arc-aware integration). Phase 2 was never *formally closed* — no phase tag, `PHASE-2.md` still reads "In progress (BUILD-3 active)," and the Bible Article 13 bare-metal multi-CPU verification remains the one open close-gate. **The dominant Phase 2 OBSERVE finding is therefore a scope-class question, not a build-surface gap (see §7.1):** is "Phase 2 PLAN" planning *formal completion/close* of an already-built phase, or net-new BUILD work that pulls deferred Phase-2+ optimizations forward? THINK's entire shape depends on PM's answer.

---

## Sources scanned

Mirrors the audit's §3 shape. COMPLETE = read end-to-end or to the depth the task needs; SAMPLED = targeted read sufficient for inventory; DEFERRED = not read, with reason.

| Source | Status | Notes |
|---|---|---|
| `docs/bible.md` Articles 4, 4.5, 5, 7 (7.4), 8 (8.3, 8.5.1–8.5.5), 12 (12.1–12.3), 13, 14, 16.4 | **COMPLETE** | Bible is 352 lines, non-standard headers (no `#` article markers). Art 13 names Phase 2 = "SMP, capability system, buddy+slab allocators, advanced TLB management." Art 14 = OBSERVE/THINK/PLAN/BUILD-VERIFY + /simplify. Art 16.4 mandates perf-regression tracking *from Phase 2 onward* (surfaced in §2 cross-cutting). |
| `docs/phases/PHASE-2.md` | **COMPLETE** | Single most-consolidated landed-work source. BUILD-1/2/3 prose + two "Forward reference" subsections enumerating deferred TLB and cap/exit optimizations. **Header stale:** status line still reads "In progress (BUILD-3 active…)" — see §7.7. BUILD-3 prose sections sub-commit 1a then jumps to SC-3B; sub-commits 1b/2 (AP bringup) live only in ADR-013 amendments — see §1 note. |
| `docs/phases/PHASE-1.5b.md`, `PHASE-1.5c.md` | SAMPLED | Audit Day-1/Day-2 already characterized 1.5c (INV-IPC-10/12/13 lineage). Foundation Phase 2 builds on; no new Phase-2 surface. |
| `docs/ADR-010` (capability foundation) | **COMPLETE** | Deferred-ops inventory: copy/move (Q2 → "Phase 3+"), sys_close (BUILD-2+ reserved CLOSE bit), sys_wait/sys_inspect (reserved WAIT/INSPECT bits), CapId recycling + per-CPU banks (deferred), forwarding caps + per-variant validator (Phase 5). |
| `docs/ADR-012` (buddy+slab) | **COMPLETE** | Allocator remaining surface: Arc-aware slab integration (post-close "explicit architecture question for BUILD-3 THINK" — BUILD-3 deferred it), buddy SMP contention story, DerivationEntry slab decision (stayed on heap), config tuning, `slab_channel_migration` regression pin. |
| `docs/ADR-013` (Phase 2 SMP retrofit) | **COMPLETE** | Q1–Q9 + anti-scope + 6 BUILD-3 amendment subsections. Anti-scope = the Phase-2+ deferred surface (PCID, RCU, load-balancing, vruntime, x2APIC, hot-plug). Q5 deletion reverted; Q9 deferred-permanently under D13; Q8.a RwLock superseded by SC-4 D5 (PROCESSES stayed single Mutex). |
| `docs/ADR-019` (IrqSafeMutex) | **COMPLETE** | Sub-phase A landed (`27d1b99`); B/C pending. **Header stale:** "Implementation status: PENDING" despite A landed — see §7.7. |
| `docs/ADR-022` (fault-recovery forward marker) | **COMPLETE** | Revisit triggers are Phase 4+/6+ (network/accelerator), not Phase 2. |
| `docs/ADR-007` (process termination) | **COMPLETE** | "Future Phase 2 hardening": cap-bits (done), sys_wait (not done), lift-Ready-restriction (done via SC-4+F-005), Phase 4 self-kill (deferred). Impl note 6 = PID-recycling/stale-handle-revocation concern. |
| `docs/ADR-004, 005, 006, 011, 020, 021, 023` | DEFERRED | 004 (IPC base) + 011 (superseded by 013) are substrate, audit-characterized. 005 (ring transition), 006 (userspace heap/mmap), 020 (LockOwner diagnostic, ratified) are Phase-1/diagnostic — no Phase-2-remaining surface. 021 (Phase 3 substrate), 023 (Linux personality, Phase 8+) out of Phase-2 scope. |
| `docs/ADR-016` (IRQ-safe heap + scheduler decoupling) | SAMPLED | Forward-marked as Phase 3 implementation per `HANDOFF-2026-05-11` §10.2. Cross-listed in §2-C as a coordination dependency; confirm landing status (§7). |
| Git log (filtered: Phase 2/BUILD-/ADR-010/013/SMP/buddy/slab/TLB) | **COMPLETE** | Confirms: no Phase 2 close commit/tag; work jumps from `64a92e1` (Q9 deferral) straight into Phase 3 BUILD-1. SC-3B `e2b3f4f`, SC-3C `ead8b19`, SC-4 `57f3cb1`. |
| Kernel source: tree + `arch/x86_64/smp.rs`, `main.rs` boot order, `mm/{frame,slab,tlb_shootdown,pmap}.rs`, `sched/*`, `sync/*` line-counts + targeted greps | SAMPLED | Confirmed AP bringup is **live** (`smp.rs:bring_up_aps()` via Limine MP + `ap_entry`). No `kernel/src/cap/` dir — capability code lives in `sched/process.rs` + `ipc/mod.rs`. Allocators present (`frame.rs` 391, `slab.rs` 693), TLB shootdown present (`tlb_shootdown.rs` 396), both IrqSafeMutex + SwitchMutex present. Per CLAUDE.md, deep end-to-end source reads are not OBSERVE-inventory work; the audit already did them. |
| Audit chain (Day-1, Day-2 findings, audit-close + F-005-close dispositions, F-005 close report) | **COMPLETE** | Source for §3 integration. |
| RAG semantic search (`:8404`, 7,620 pts) — 5 discovery queries | **COMPLETE** | Surfaced `Q9-EXPLORATION-2026-05-11.md` (canonical "what's-left-in-BUILD-3" synthesis) and `HANDOFF-2026-05-11-…phase-2-build-3-substantially-complete…` (the pivot-to-Phase-3 record) — both read in full. RAG confirmed PHASE-2.md/ADR-012 forward-marker sections as top hits (already read). |
| `PHASE-4-THINK-v0.2-…` + `PL-RATIFICATION-…q1-q12` | **COMPLETE** | §5 question-format reference (each Q = one addressable paragraph; Q1–Q12 ratified as a batch). |
| `PHASE-2-AND-3-VMWARE-HYPERVISOR-VM-VERIFICATION-2026-05-20`, `PHASE-3-BARE-METAL-OPTIPLEX-7080-BOOT-SMOKE-EVIDENCE-2026-05-22` | DEFERRED | Not deep-read. **Material to §7.5** — whether either exercised `NEO_SMP>1` on real hardware determines if the Bible Art 13 Phase-2 bare-metal multi-CPU mandate is already (partially) citable. Flagged for PM, not resolved here. |

All dispatch §2 sources scanned or accounted for. No source blocked completion.

---

## §1 — Phase 2 landed work inventory ("what's done")

Phase 2 BUILD-1/2/3 closed across 2026-05-04 → 2026-05-11. The authoritative consolidated log is `PHASE-2.md`; the sub-commit→Q-item→commit mapping below merges `PHASE-2.md`, ADR-013's amendment cluster, and `Q9-EXPLORATION-2026-05-11.md` §2.1.

### BUILD-1 — Capability foundation (ADR-010)

| Item | Close commit | Load-bearing invariants produced/confirmed |
|---|---|---|
| Handle gains `(cap_id, rights)`; two-tier rights (meta `USE\|DERIVE\|GRANT\|REVOKE` + per-variant semantic); `sys_derive`/`sys_revoke`/`sys_grant`/`sys_recv_grant`; rights-gated `sys_send`/`sys_recv`/`sys_kill`; per-process derivation log; grant=derived-copy; revoke=slot-free; survive-exit lifetime; 10 `cap_*` scenarios | `d759cbd` + impl | Three-way rights intersection (Principle 1); reserved-bit pattern (Principle 2, CLOSE/WAIT/INSPECT); errors-at-causing-actor (Principle 3); single-CPU `without_interrupts` around cap critical sections (replaced by SMP discipline in BUILD-3). |

### BUILD-2 — Buddy + slab allocators (ADR-012) — **LANDED at `f5a4984`**

| Sub-commit | Q-item | Commit | Invariants / status |
|---|---|---|---|
| 0 | Slab interface stub | `03e8759` | `SlabCache<T>` trait, per-CPU magazine + global cache shape. |
| 1 | Buddy frame allocator (orders 0–14) | `b617acf` | Inv-1: split/coalesce never touch `allocated_pages`; Inv-3: HHDM-before-buddy-init ordering. Retires `BumpFrameAllocator` + free-list overlay. `expect_pass_but_alloc_fail` meta-test. |
| 2 | Slab impl (per-CPU magazines + global fallback) | `93bf178` | `slab_local_to_global_promotion`, `slab_global_to_buddy_overflow`. |
| 3 | Channel→slab migration | **REVERTED** `e754723` | Arc-vs-slab boundary: `Arc::from(Box<T>)` routes Drop to global allocator, not slab free. `slab_channel_migration` pinned `ignored:true`. CHANNEL_CACHE declared-but-unused. |
| 4 | Thread→slab migration | `f5a4984` | `slab_thread_migration` (alloc-traverses-slab + free-returns-to-cache). `without_interrupts` is allocator discipline (consolidated finding 3). |

**BUILD-2 production status: THREAD_CACHE live; PROCESS_CACHE + CHANNEL_CACHE declared-but-unused (Arc-aware integration deferred); HANDLE/EndpointState not slab-cached (inline by design); DerivationEntry stayed on kernel heap.**

### BUILD-3 — SMP retrofit (ADR-013, supersedes ADR-011)

| Sub-commit | Q-items | Close commit | Invariants produced/confirmed |
|---|---|---|---|
| 0 | GLOBAL-bit fixup (Q7.a partial) | `fe5bc3d`/`b4b3ac9`/`57fd235` | Kernel-half leaves GLOBAL; CR4.PGE enabled; post-fixup `sanity_check global=true`. |
| 1a | Q1 per-CPU state shape | `305f4d9` | `MAX_CPUS=64`; `PER_CPU:[PerCpu;64]`; per-AP GDT/TSS/IST as **static** storage (boot-order deviation: `init_traps` precedes `mm::init`); single-cache-line PerCpu discipline. |
| **1b** | **Q2 AP bring-up** | (Limine MP) | **Documented only in ADR-013 amendments, not sectioned in PHASE-2.md.** NEO-local sequential cpu_id (BSP=0, APs 1..N via `extra_argument`); AP CR0/CR4 setup before trap init; APs sentinel-check `gs:[24]`, halt. Confirmed live in `smp.rs:bring_up_aps()`+`ap_entry`. |
| **2** | **Q3 per-CPU schedulers; Q5 deferred-quantum; Q9 deferred** | (per-CPU sched) | Global `SCHED` removed; `PerCpuScheduler` per CPU. **Q5 deletion REVERTED** (deferred-quantum mechanism restored per-CPU — bet failed at run 24). **Q9 reaper distribution DEFERRED** (bisection localized hang; BSP-scans-all interim). AP GS-base rewire-after-GDT-load invariant. |
| 3 (SC-3B) | Q4 wake-on-completer-CPU; Q6 `block_current_into` lock-order | `e2b3f4f` | `context_switch_with_release` (ManuallyDrop+asm, ≤4 locks released on new stack); consumer-first preserved per-rendezvous; **Hpw discipline** (Bible 8.5.4). 200/200 NEO_SMP=4. |
| 4 (SC-3C) | Q7.b per-CPU CR3 cache; Q7.c IPI TLB shootdowns | `ead8b19` | `active_pmap_cr3` compare-and-skip; IPI vector `0xfc`, `ShootdownPayload` ACK counter, INVLPG/full-flush hybrid (≤33 pages), synchronous 1ms-bounded; D14 missing-shootdown-detection. |
| 5+6 (SC-4) | Q8.a/b/c cap-table SMP + blocked-kill walk | `57f3cb1` | `handles`/`derivations` on SwitchMutex (lock order handles→derivations); **D5: `cr3_for_thread` retired, Thread carries `Arc<Pmap>`, PROCESSES off hot path** (deviates from ADR-013 Q8.a's RwLock choice — PROCESSES stayed single Mutex per FM-D5.4); D8 `Process.exit_state:AtomicU8` CAS; D13 single-thread-per-process invariant; `zombify_thread` reentrant. |

### Post-BUILD-3 fixes (this session)

| Item | Commit | Note |
|---|---|---|
| F-005 cross-process zombify in forced-kill blocked-walk | `9a282f2` | Unified `Channel::extract_parked_if_owned(side, expected_pid)` — the *only* public extraction primitive; restores INV-IPC-8 lockstep by construction. Closes F-003 + F-005. |

**Net §1:** Every ADR-013 Q-item is implemented (Q1–Q8) or deferred-permanently (Q9). ADR-012 buddy+slab landed (Thread production-live; Process/Channel deferred). ADR-010 cap foundation landed. The four pre-Phase-2-audit DEFECTs (F-001..F-005, less F-004 which was a (b) test item) are all closed. **What is NOT done is enumerated in §2.**

---

## §2 — Phase 2 remaining work surface

Decomposed per Bible Article 13's four workstreams, plus cross-cutting. Each item: scope sketch, dependencies, catalogued open questions (not answered — those route to §5/§7). Scope-class ambiguities (Phase-2 vs Phase-2+/Phase-3) are flagged `[→§7]` and **not decided here**.

### 2.A — SMP residual

SMP is substantially landed (Q1–Q8). Residual is verification + deferred-by-design items, not core mechanism.

- **A1. Bare-metal multi-CPU verification at BUILD-3 close** *(Bible Art 13 mandate; the formal close-gate).* All SMP sub-commits verified on QEMU `-smp 4`; VMware hypervisor-VM verification (`PROCEDURE-2026-05-11-…`) was filed as an *intermediate* gate explicitly **not** bare-metal. `Q9-EXPLORATION` §2.4 names this the *only* unchecked migration-checklist line. Dep: 4+ core bare-metal target + formal protocol (`Q9-EXPLORATION` §5.1). Open: does existing OptiPlex-7080 / VMware evidence already satisfy multi-CPU bare-metal? `[→§7.5]`
- **A2. Q9 per-CPU reaper distribution** — deferred *permanently* under D13 single-thread-per-process; reopens only when Phase-2+ multi-thread-per-process processes ship. Recommended re-entry: `Q9-EXPLORATION` §3 alt (c) (per-CPU reapers, no `live_thread_count`, composing with D8 CAS). **Not Phase-2-close scope** unless multi-thread is pulled into Phase 2 `[→§7]`.
- **A3. Cross-CPU thread migration beyond rendezvous** (load balancing, work stealing, periodic rebalance) — ADR-013 anti-scope; deferred to future BUILDs. Q4 wake-on-completer-CPU is the one bounded migration that exists.
- **A4. x2APIC** (ADR-013 anti-scope; BUILD-3 uses xAPIC) and **hot-plug CPUs** (anti-scope; count fixed at boot). Both deferred; hot-plug likely never.
- **A5. Buddy allocator SMP contention** — cross-listed with 2.C (the buddy is a single global `Mutex` today; Bible 8.3 forbids global locks in hot paths).

### 2.B — Capability system full buildout

Foundation landed (BUILD-1); the "full buildout" is the reserved-bit-activating syscalls and SMP-era identity hardening.

- **B1. `sys_close`** — CLOSE semantic bit reserved (ADR-010 Principle 2); ADR-010 note 8 says it "ships Phase 2 BUILD-2 or later, once a use case appears." Never shipped. Today close is implicit at exit via `close_channel_peers`. Dep: identified use case; CLOSE-bit gating discipline ("enforcement, not theatre"). Audit INV-CAP-12 = PASS-deferred-correctly (re-audit when it lands).
- **B2. `sys_wait(handle)`** — WAIT bit reserved; ADR-007 "Future Phase 2 hardening" names it; **directly retires FM-D14** (`COMPLETED_EXIT_CODES` grows unbounded without a wait/consume consumer). Dep: exit-code delivery semantics; single-thread-per-process (D13) interaction.
- **B3. `sys_inspect(handle)`** — INSPECT bit reserved; what process state is inspectable, composing with no-ambient-authority (Bible 12.2).
- **B4. Capability copy / move ops** — ADR-010 Q2 defers to "Phase 3+." Move is a no-op at the flat-table layer. **Scope-ambiguous: Phase-2 "full buildout" or genuinely Phase 3+? `[→§7.3]`**
- **B5. PID recycling + stale-handle revocation** — ADR-007 Impl note 6: `NEXT_PID` is monotonic today, so a stale `ProcessHandle` can't mis-resolve; *if* Phase 2 adds PID recycling, a stale handle could `sys_kill` a *different* live process. Fix is handle revocation (mark Process variants stale on target death → ESRCH). **Conditional on whether Phase 2 adds PID recycling `[→§7.6]`.** Cross-cuts cap + SMP.
- **B6. CapId allocation under SMP** — ADR-010 note 2: monotonic `AtomicU64::fetch_add` is a global atomic; ADR-010 said "Phase 2 SMP decides whether per-CPU banks are needed." SMP BUILD-3 did *not* address it (still a single global atomic). Open whether Phase 2 close needs per-CPU CapId banks.
- **B7. INV-CAP-11 spawn-inheritance descendants** — audit (b) test (see §3): confirm/add `cap_revoke_spawn_inheritance_descendants`. Absorbs into whichever BUILD touches cap revoke.
- **Deferred-to-Phase-5:** per-variant subset validator, forwarding caps, Tensor/Model/InferenceTicket/File variants. Out of Phase 2.

### 2.C — Buddy + slab allocators

Landed (BUILD-2); remaining is the deferred architectural question plus SMP hardening.

- **C1. Arc-aware slab integration** *(the big one).* `Arc::from(Box<T>)` returns storage to the global allocator on final drop, bypassing slab — blocks PROCESS_CACHE + CHANNEL_CACHE production migration. Three paths (ADR-012 post-close): custom `Drop` returning to slab; `Arc::new_in()` + `allocator_api` (unstable; project is on nightly); refactor away from `Arc<T>` final owner. ADR-012 post-close named this "an explicit architecture question for BUILD-3 THINK" — **BUILD-3 explicitly did NOT solve it.** Flipping `slab_channel_migration` `ignored:false` is the regression pin. Open: is it Phase-2 scope at all, or deferred until a workload justifies the complexity? `[→§5-ALLOC]`
- **C2. Buddy SMP contention story** — buddy is held in `static FRAME_ALLOC: Mutex<Option<BuddyAllocator>>` (single global lock). ADR-012 note 1 + §"Future Phase 5+": "SMP BUILD-3 hardens — per-order locking, per-CPU magazines for the buddy, or stays single-lock; Phase 2 SMP ADR decides." SC-3B/3C/4 did not address it. Tension with Bible 8.3 (no global locks in hot paths) — though slab magazines absorb most frame traffic, so "hot path" is arguable. Open.
- **C3. DerivationEntry slab decision** — BUILD-2 deferred to sub-commit 3 analysis (which got reverted); entries stay on the kernel heap (option c by default). Open whether to slab-back them.
- **C4. Slab config tuning** — `SlabConfig` numbers are "educated guesses, not measured optima" (Principle 5: mechanism stays, numbers move with data). Retuning under SMP/workload measurement is explicitly anticipated.
- **C5. ADR-016 A1 IRQ-safe heap + scheduler decoupling** — forward-marked as Phase 3 implementation (`HANDOFF-2026-05-11` §10.2). Confirm landing status `[→§7]`; cross-listed because it touches allocator/scheduler boundary.
- **Deferred:** Bonwick pre-construct, slab shrinking under pressure, deeper debug instrumentation, `linked_list_allocator` replacement — all Phase-3+/Phase-5+ or measurement-gated.

### 2.D — Advanced TLB management

**Correctness baseline landed** (Q7.a GLOBAL bit, Q7.b CR3 cache, Q7.c IPI shootdown with INVLPG/full-flush hybrid). Everything else is recorded as Phase-2+/Phase-3+ forward markers in `PHASE-2.md` §"Forward reference: Phase 2+ TLB management optimizations" — **all measurement-gated, none implemented:**

- **D1. PCID / ASID enablement** (SC-3C D2; ADR-013 anti-scope). Every CR3 write currently full-local-flushes. Linux global-ASID promotion = sampling-gate-plus-rising-floor pattern.
- **D2. T2 targeted shootdown** (per-Pmap CPU mask; SC-3C D4). Current shootdown broadcasts to all CPUs. Strictly- vs loosely-accurate per-mm cpumask is the documented design fork (NeoPlutarch's inference scheduler may favor strictly-accurate).
- **D3. Lazy TLB** (receiver-side short-circuit; SC-3C D6). Must encode the `freed_tables=true` unconditional-flush exception.
- **D4. Shootdown batching** (mmu_gather analogue; SC-3C D7).
- **D5. INVLPGB (AMD Zen 4+) / RAR (Intel 4th-gen Xeon+) hardware-broadcast** — Phase 3+ forward marker; gate NEO_SMP>4 or measurement.
- **D6. Spectre-v2 BTB flush / IBPB on context switch** — CPU-vuln mitigation cluster; "deserves its own ADR." Interacts with Q7.b's conditional CR3 writes.

**The central D-workstream question is interpretive:** does "advanced TLB management" (Bible Art 13, Phase 2) mean the *correctness baseline already shipped*, or does it require pulling some of D1–D6 forward — several of which are explicitly Phase-2+/Phase-3+ forward-marked? `[→§7.4]`

### 2.X — Cross-cutting

- **X1. Four audit (b) BUILD-N test items** (see §3): `smp_concurrent_reap_after_terminate`, `ipc_rendezvous_translate_fault`, `cap_revoke_spawn_inheritance_descendants`, `sched_ready_queue_capacity_pressure`. Absorb into Phase-2 BUILD subphases as they touch the relevant code.
- **X2. ADR-019 sub-phases B/C** (IrqSafeMutex migration) — defense-in-depth: migrate mixed-context `SwitchMutex`/`without_interrupts`-wrapped sites (esp. `Channel.state`) to type-enforced `IrqSafeMutex`, removing the manual wrappers. Audit (d) item; Phase-2-appropriate. Not a live correctness gap (INV-SYNC-1 manual discipline is TESTED) — a structural-enforcement gap.
- **X3. Phase 2 formal-close semantics** — no closure artifact exists; `PHASE-2.md` reads "In progress"; the queued `PM-NOTE-*-phase-2-build-3-substantially-complete.md` (`Q9-EXPLORATION` §7.2) was never filed. `[→§7.1, §7.2]`
- **X4. Performance-regression tracking** — **Bible Article 16.4 mandates** per-commit tracking of boot time, syscall latency, context-switch cost, and scheduling fairness (Redox `schedrs`-adapted) **"from Phase 2 onward."** No evidence this harness exists. Plausibly a Phase-2-close deliverable. Also: ADR-019 bench 50ns native-hardware target deferred to bare-metal CI (`Q9-EXPLORATION` §5.3).
- **X5. rustfmt tooling gap** — pinned nightly lacks rustfmt; `cargo fmt --check` BLOCKED across SC-3C/SC-4 (`Q9-EXPLORATION` §5.4). PL operational decision pending; blocks eventual cleanup, not BUILD.

---

## §3 — Audit closure integration

Per the final ratified table (`PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md` §1, updated by `…f005-close.md` §2). One line per finding: where it lands in Phase 2.

### PASS (5) — foundation invariants Phase 2 must preserve (regression anchors)

| Finding | Phase 2 disposition |
|---|---|
| INV-IPC-11/12 (REMOVED deferred-quantum mechanism; dependent `ipc_heap_kill_smoke` survives) | Foundation. *Note:* Q5's deferred-quantum was actually **restored** (ADR-013 amendment) — the audit's "REMOVED" refers to the BUILD-2/4 handoff amendments under ADR-013 Q5, not the per-CPU mechanism. Any Phase-2 scheduler BUILD preserves the smoke. |
| INV-CAP-12 (CLOSE-bit enforcement; `sys_close` not shipped) | **Re-audit trigger when B1 `sys_close` lands** — the CLOSE bit must gate it ("enforcement, not theatre"). |
| INV-CAP-13 (kernel-cleanup paths exempt from rights checks by construction) | Foundation invariant; *why* F-005 was reachable. Any new cleanup path must carry the pid check (now structural via `extract_parked_if_owned`). |
| INV-NEG-3 / INV-PROC-7 lift (blocked-kill, same-process) | Foundation; preserve under any cap/SMP BUILD. |
| INV-SEC-1 (SMEP/SMAP/NX/KASLR/W^X) | Phase-1-closed foundation; regression anchor. |

### (b) BUILD-N test items (4) — map to Phase 2 BUILD subphase candidates

| Finding | Recommended test | Lands in |
|---|---|---|
| F-004 (INV-PROC-5 reaper double-cleanup under SMP) | `smp_concurrent_reap_after_terminate` | SMP-verification / formal-close BUILD (2.A). |
| INV-IPC-5 (rendezvous translate-fault) | `ipc_rendezvous_translate_fault` | IPC/cap-touching BUILD (e.g. cap-buildout 2.B). |
| INV-CAP-11 (spawn-inheritance descendants in revoke walk) | `cap_revoke_spawn_inheritance_descendants` (or confirm existing covers) | Capability-buildout BUILD (2.B / B7). |
| INV-SYNC-5 (ready-queue capacity-bound release-build hazard) | `sched_ready_queue_capacity_pressure` | SMP/scheduler BUILD (2.A); may surface a Phase-2 growth-policy ADR. |

### (d) documented deferrals (2) — revisit triggers

| Finding | Trigger |
|---|---|
| INV-PROC-2 (self-kill -EINVAL runtime test) | Phase 4 (cap-passing-over-IPC makes it constructible). ADR-007-tracked. **Not Phase-2.** |
| INV-SYNC-2 (IrqSafeMutex sub-phases B/C) | Phase-2-appropriate defense-in-depth (=2.X2). ADR-019-tracked. |

### DEFECT → (a) COMPLETE (4) — foundation (closed)

F-001 (INV-IPC-7), F-002 (INV-CAP-6) closed in PR #6 (`4dfb13f`); F-003 (INV-IPC-8 lockstep), F-005 (cross-process zombify) closed in `9a282f2`. INV-IPC-8 is now **structurally guaranteed** by the unified `extract_parked_if_owned` helper — a third extraction site cannot land without inheriting the pid check. This is substrate Phase 2 builds on; any new IPC cleanup path must route through the unified helper.

**Net:** the audit characterized the foundation's correctness boundary. Phase 2 inherits 5 PASS invariants to preserve, 4 (b) tests to absorb, 2 (d) deferrals (one Phase-2 = ADR-019 B/C, one Phase-4), and a hardened IPC-cleanup substrate. Zero blocking items remain (F-005 closed; Phase 2 PLAN unblocked per `5884ffd`).

---

## §4 — Deferred ADR revisit candidates

Disposition per ADR for Phase 2 scope. (Revisit now / revisit in a specific BUILD / leave deferred.)

| ADR | Status | Phase 2 disposition |
|---|---|---|
| **ADR-010** (cap foundation) | Accepted; BUILD-1 done | **Revisit (cap buildout):** activate reserved bits — `sys_close` (B1), `sys_wait` (B2), `sys_inspect` (B3). **Leave deferred:** copy/move (Q2 Phase 3+ `[→§7.3]`), CapId recycling, forwarding caps, per-variant validator (Phase 5). CapId-under-SMP banks (B6) revisit if contention measured. |
| **ADR-012** (buddy+slab) | Accepted; BUILD-2 done | **Revisit:** Arc-aware slab integration (C1 — the named "BUILD-3 THINK question" BUILD-3 punted), DerivationEntry slab (C3), config tuning (C4). **Leave deferred:** `linked_list_allocator` replacement (Phase 3+), Bonwick/shrinking (Phase 5+). |
| **ADR-013** (SMP retrofit) | Accepted; Q1–Q8 done, Q9 deferred-permanent | **Revisit (close-gate):** bare-metal multi-CPU verification (A1). **Leave deferred:** Q9 (reopens on multi-thread), anti-scope set (PCID, RCU, load-balancing, vruntime, x2APIC, hot-plug). Note Q8.a's RwLock decision was *superseded* in implementation by SC-4 D5 (single Mutex; doc the deviation if not already — `[→§7.7]`). |
| **ADR-016** (IRQ-safe heap + sched decoupling) | Forward-marked Phase 3 | **Confirm landing status `[→§7]`.** If unlanded, it's a Phase-3 dependency, not Phase-2. |
| **ADR-019** (IrqSafeMutex) | A landed; B/C pending; **header stale ("PENDING")** | **Revisit (Phase-2 defense-in-depth):** sub-phases B (migrate mixed-context `SwitchMutex` callers) + C (migrate `Channel.state` `without_interrupts` sites, remove wrappers). Audit (d). Each its own gate. |
| **ADR-022** (fault-recovery forward marker) | Forward marker; no commitment | **Leave deferred.** Revisit triggers are Phase 4+ (network) / Phase 6+ (accelerator), explicitly not Phase 2. |
| **ADR-007** (process termination) | Accepted | **Revisit:** `sys_wait` (=B2, retires FM-D14), PID-recycling/stale-handle-revocation (B5, conditional `[→§7.6]`). **Leave deferred:** Phase-4 self-kill guard (=INV-PROC-2 (d)). |
| ADR-021 (Phase 3 substrate), ADR-023 (Linux personality) | — | Out of Phase-2 scope (Phase 3 / Phase 8+). |

No ADR was found to *contradict* the bible. ADR-013 Q8.a (RwLock) vs the landed single-Mutex is a *documented supersession* (SC-4 D5/FM-D5.4), not a contradiction.

---

## §5 — Design question seeds for THINK

Enumerated, **not answered.** Grouped by workstream. Format mirrors Phase 4 THINK v0.2 Q1–Q12 (each a single addressable paragraph). Goal: identify what PLAN must ratify before BUILD subphases can be enumerated. **PM + PL resolve these in THINK; Cerebro does not.**

**Scope-framing (must resolve first — gates the rest):**

> **Q1 — Phase-2 close shape.** Given BUILD-1/2/3 are substantially landed, does "Phase 2 PLAN" produce (a) a *formal-completion* BUILD set (bare-metal multi-CPU verification + cap reserved-bit activation + audit (b) absorption + perf-regression harness), (b) a *deferred-optimization* BUILD set pulling Phase-2+ forward markers (Arc-aware slab, PCID/lazy-TLB) into Phase 2, or (c) both as ordered sub-phases? Every other question's *in-scope/out-of-scope* answer depends on this. (Companion scope-class ambiguity in §7.1.)

**Capability buildout (2.B):**

> **Q2 — `sys_close` use case + semantics.** ADR-010 reserved CLOSE pending "a use case." Does Phase 2 surface one (explicit channel teardown distinct from process-exit `close_channel_peers`), and what are the semantics — does `sys_close` wake the peer with `-EPIPE` like exit-time cleanup, and how does it interact with survive-exit cap lifetime (a closed channel cap held by a third process)?

> **Q3 — `sys_wait` semantics and FM-D14 retirement.** ADR-007 names `sys_wait(handle)` returning the target's exit code; FM-D14 records that `COMPLETED_EXIT_CODES` grows unbounded without this consumer. What are the blocking semantics (block until target exits; what if already exited), exit-code delivery shape, and the interaction with D13 single-thread-per-process and D8 `exit_state` CAS? Does landing `sys_wait` fully retire FM-D14 or only bound it?

> **Q4 — `sys_inspect` surface.** INSPECT is reserved. What process/thread state is inspectable through it (state, exit status, cap inventory?), and how does it stay inside Bible 12.2's no-ambient-authority discipline — i.e., what does holding INSPECT on a ProcessHandle authorize observing, without leaking another process's capabilities?

> **Q5 — PID recycling and stale-handle identity.** ADR-007 Impl note 6: `NEXT_PID` is monotonic today, so a stale `ProcessHandle` is safe; if Phase 2 introduces PID recycling, a stale handle can resolve to a *different* live process and `sys_kill` the wrong target. Does Phase 2 add PID recycling at all, and if so, does the fix require cap-id-based target identity (revoke-on-death marking Process variants stale → `-ESRCH`) rather than pid-based lookup? (Whether PID recycling is in scope is the §7.6 ambiguity.)

> **Q6 — CapId allocation under SMP.** ADR-010 left "per-CPU CapId banks vs single global atomic" to "Phase 2 SMP." BUILD-3 kept the single `AtomicU64::fetch_add`. Is the global atomic acceptable at NeoPlutarch's ≤64-CPU scale (cap mint/derive/grant frequency), or does Phase 2 close want per-CPU allocation banks — and what evidence (contention measurement) would decide?

**Allocators (2.C):**

> **Q7 — Arc-aware slab integration scope and path.** PROCESS_CACHE and CHANNEL_CACHE are declared-but-unused because `Arc::from(Box<T>)` bypasses slab free. Is Arc-aware integration Phase-2-close scope or deferred-until-workload-justifies? If in scope, which of the three paths — custom `Drop`-returns-to-slab, `Arc::new_in()` + unstable `allocator_api`, or refactor away from `Arc<T>` final owner — and does the choice ripple into IPC/process design? (The decision flips the `slab_channel_migration` regression pin.)

> **Q8 — Buddy SMP contention.** The buddy is a single global `Mutex<Option<BuddyAllocator>>`. Slab magazines absorb most frame traffic, but Bible 8.3 forbids global locks in hot paths. Is the single lock acceptable for Phase 2 close, or does it need per-order locking / per-CPU buddy magazines — and is "frame alloc behind slab magazines" a hot path under SMP load? What measurement triggers the change?

> **Q9 — DerivationEntry storage.** The per-process derivation log is `BTreeMap<CapId, Option<CapId>>` on the kernel heap (BUILD-2 deferred slab-backing). Does Phase 2 keep it heap-backed (Principle 6: variable-size container → heap) or move to a slab-friendly structure, and does cap-churn under SMP create a fragmentation concern that justifies the change?

**Advanced TLB (2.D):**

> **Q10 — "Advanced TLB management" satisfaction bar.** Bible Art 13 lists "advanced TLB management" as Phase 2. The correctness baseline (GLOBAL bit, CR3 cache, IPI shootdown) shipped; PCID, T2 targeted shootdown, lazy-TLB, batching, INVLPGB/RAR are forward-marked Phase-2+/Phase-3+. Is the Art-13 requirement *met* by the baseline, or does Phase 2 close require pulling specific optimizations forward — and which, on what measurement trigger? (Interpretive scope ambiguity in §7.4.)

> **Q11 — Spectre-v2 / IBPB on context switch.** Every CR3 switch is a potential BTI surface; Q7.b's conditional CR3 writes complicate IBPB placement. Bible 12.3 enables CET/UMIP/LAM "where supported." Is CPU-vulnerability mitigation (IBPB on context switch) a Phase-2 security deliverable, a dedicated ADR, or a later-phase item — and where do IBPB call sites sit relative to the CR3-skip path?

**Cross-cutting (2.X):**

> **Q12 — Performance-regression harness (Bible Art 16.4).** Art 16.4 mandates per-commit tracking of boot time, syscall latency, context-switch cost, and scheduling fairness (Redox `schedrs`-adapted) "from Phase 2 onward." No such harness appears to exist. Is standing it up a Phase-2-close deliverable, what reference VM/config anchors it, and how does it compose with the existing environment-triplet discipline (Docker QEMU TCG / CI / bare-metal) and the deferred ADR-019 50ns native bench?

> **Q13 — IrqSafeMutex B/C migration scope and pairing.** ADR-019 sub-phases B (migrate mixed-context `SwitchMutex` callers) and C (migrate `Channel.state` `without_interrupts` sites, remove wrappers) convert manual Hpw-discipline to type-enforcement. Is this a standalone Phase-2 defense-in-depth BUILD, and should it pair with the audit (b) tests (it touches the same Channel.state lock surface as INV-IPC-5 / INV-SYNC-5)? What is the per-sub-phase gate (the 200/200 NEO_SMP=4 equivalent)?

> **Q14 — Bare-metal multi-CPU verification protocol.** Bible Art 13 requires bare-metal multi-CPU verification at BUILD-3 close (the open close-gate). What is the acceptance bar — which SMP scenarios (`smp_cross_cpu_rendezvous_stress`, `q7c_*`, `q8a/b/c_*`), what core count, what evidence (frame-audit-clean, no PANIC/BUG, GS-base sentinels) — and does existing VMware/OptiPlex evidence partially satisfy it? (Confirmation dependency in §7.5.)

(14 questions across 5 groups; within the 8–15 target. Q1, Q10 carry the scope-class weight; Q2–Q9, Q11–Q14 are the per-workstream design seeds.)

---

## §6 — Recommended next BUILD subphase candidates

2–3 candidates, ranked by dependency-cleanness × impact. **Cerebro recommends; PM/PL ratify ordering in PLAN (scope-class).** All assume THINK answers Q1 (close-shape) first.

1. **Capability reserved-bit activation BUILD** *(highest dependency-cleanness × impact).* Ship `sys_wait` (B2), then `sys_close` (B1), then `sys_inspect` (B3) — each activates an already-reserved bit with zero ABI break (Principle 2). Cleanest dependency: the cap foundation, rights vocabulary, and IPC rendezvous all exist; this is additive syscall work. High impact: `sys_wait` retires FM-D14; the three syscalls complete ADR-010/ADR-007's "Future Phase 2 hardening." Absorbs audit (b) INV-CAP-11 (`cap_revoke_spawn_inheritance_descendants`) and INV-IPC-5 (`ipc_rendezvous_translate_fault`). Scope sketch: ~one sub-commit per syscall + tests; no SMP-mechanism risk. **Open dependency:** B1 needs a `sys_close` use case (Q2) — if none surfaces, ship B2/B3 and defer B1.

2. **Phase 2 formal-close verification BUILD** *(clean, low mechanism-risk, satisfies the Art-13 gate).* Bare-metal multi-CPU verification (A1) + perf-regression harness stand-up (X4, Bible 16.4) + remaining audit (b) tests (F-004 `smp_concurrent_reap_after_terminate`, INV-SYNC-5 `sched_ready_queue_capacity_pressure`) + rustfmt-gap resolution (X5). Dependency-clean: no new kernel arch, mostly verification + test + tooling. Impact: closes the *only* outstanding Bible Art-13 Phase-2 mandate and lights up the perf-tracking the bible requires "from Phase 2 onward." **Dependency:** PL hardware session for bare-metal (A1); confirm whether OptiPlex/VMware evidence already counts (§7.5).

3. **ADR-019 B/C IrqSafeMutex migration BUILD** *(defense-in-depth; medium cleanness).* Type-enforce the Hpw discipline at the mixed-context lock sites (sub-phase B then C). Pairs naturally with the Channel.state-touching audit (b) tests (Q13). Lower urgency than #1/#2 (no live correctness gap — manual discipline is TESTED), higher structural value. Scope sketch: ~2 sub-commits, each gated at 200/200 NEO_SMP=4.

*(Deliberately NOT recommended as a first BUILD: Arc-aware slab integration (C1) — it is the highest-risk architectural item BUILD-3 already deferred, touches Arc lifetime broadly, and per §7.1 may not even be Phase-2-close scope. If pursued, it wants its own THINK pass, not a first subphase.)*

---

## §7 — Open questions for PM

Scope-class / judgment items Cerebro hit that need PM (or PL) resolution before THINK can start. **Surfaced, not decided** (per dispatch §5/§6, C6).

1. **THE framing question — close vs greenfield vs both.** Phase 2's core (cap foundation, allocators, SMP) is substantially landed since 2026-05-11. The dispatch §1.2 describes "buddy+slab allocators are pending" and frames remaining surface as if greenfield; OBSERVE finds them **landed** (BUILD-2 `f5a4984`). Is "Phase 2 PLAN" planning *formal completion/close* (verification + cap reserved-bit activation + audit (b) absorption), or net-new BUILD pulling Phase-2+ forward markers (Arc-aware slab, PCID) forward, or both as ordered sub-phases? This is the single highest-leverage decision; it determines the in/out-scope answer for nearly every §2 item and §5 question. **PM/PL scope call.**

2. **Phase 2 has no formal closure artifact and `PHASE-2.md` says "In progress (BUILD-3 active)."** The team declared BUILD-3 "substantially complete" (2026-05-11), pivoted to Phase 3→4, but never formally closed Phase 2; the queued `PM-NOTE-…-phase-2-build-3-substantially-complete.md` (`Q9-EXPLORATION` §7.2) was apparently never filed. Does the remaining buildout (sys_close/wait/inspect, Arc-aware slab, defense-in-depth) happen *under a still-open Phase 2*, or does Phase 2 *close first* (on the existing landed work + bare-metal verification) and the rest become Phase-2.5 / fold into later phases? Phase-ledger semantics call.

3. **Copy/Move capability ops — Phase 2 or Phase 3+?** ADR-010 Q2 defers copy/move to "Phase 3+." But "capability system *full buildout*" (the dispatch's §1.2 framing of the cap workstream) could be read to include them. Genuinely ambiguous; PM scope call. (Move is a no-op at the flat-table layer, so the stakes are low — but the framing should be explicit.)

4. **"Advanced TLB management" interpretation.** Bible Art 13 names it a Phase-2 workstream. The correctness baseline (GLOBAL/CR3-cache/IPI-shootdown) shipped, but PCID, T2 targeted shootdown, lazy-TLB, batching, and INVLPGB/RAR are explicitly forward-marked **Phase-2+ / Phase-3+** in `PHASE-2.md`. There is a real tension: Art 13 says "Phase 2," the forward markers say "Phase 2+/Phase 3+." Is the Art-13 requirement *satisfied by the baseline*, or must Phase 2 close pull specific optimizations forward (overriding their forward-marker placement)? **Bible-adjacent interpretive call** — possibly warrants a one-line PM-NOTE or, if the resolution moves items, a bible clarification (which I do not draft per C6).

5. **Bare-metal multi-CPU verification — already satisfied?** I did not deep-read `PHASE-2-AND-3-VMWARE-HYPERVISOR-VM-VERIFICATION-2026-05-20` or `PHASE-3-BARE-METAL-OPTIPLEX-7080-BOOT-SMOKE-EVIDENCE-2026-05-22`. If the OptiPlex Phase-3 bare-metal smoke ran at `NEO_SMP>1`, the Bible Art-13 Phase-2 multi-CPU bare-metal mandate may already be (partially) citable, collapsing A1 from a BUILD to a documentation pass. **PM should check those two evidence docs against the A1 protocol** (`Q9-EXPLORATION` §5.1) before scoping a verification BUILD.

6. **Is PID recycling in Phase 2 scope?** ADR-007 Impl note 6 flags the stale-handle-revocation concern as conditional on Phase 2 adding PID recycling. If Phase 2 does *not* add it, the concern stays dormant and B5/Q5 drop out of scope. Whether to add PID recycling (and thus need cap-id-based handle identity + revoke-on-death) is itself a scope decision. **PM call.**

7. **Documentation staleness (flagged, not fixed — per C6).** Two docs are stale and may mislead THINK/PLAN: (a) `PHASE-2.md` header reads "In progress (BUILD-3 active…)" though BUILD-3 is substantially complete and Phase 3/4 came after; (b) `ADR-019` header reads "Implementation status: PENDING" though sub-phase A landed at `27d1b99`. Also: ADR-013 Q8.a (RwLock) was superseded in implementation by SC-4 D5 (single Mutex) — recorded in FM-D5.4 but not in the Q8.a section itself. None blocking, but PM may want a doc-reconciliation pass before or alongside THINK. (I do not amend — dispatch C6.)

8. **ADR-016 landing status.** ADR-016 (IRQ-safe heap + scheduler decoupling) was forward-marked as Phase-3 implementation (`HANDOFF-2026-05-11` §10.2). I did not confirm whether it landed during Phase 3. If unlanded it's a Phase-3 dependency; if landed it's foundation. Worth a one-line confirmation so it isn't mis-scoped into Phase 2.

---

## §8 — Close

Deliverable filed at `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md`. Per dispatch §4 acceptance criteria: C1 (sources scanned — table complete; two evidence docs DEFERRED with stated §7.5 reason), C2 (§1–§4 complete), C3 (§5 = 14 questions, grouped, unanswered), C4 (§6 = 3 candidates with scope sketches + dependencies), C5/C6 pending (push + report). Per C6: no audit artifact amended, no ADR/bible drafted, no THINK question answered, no scope-class item decided.

Time: within the 4–8h envelope; no checkpoint/continuation needed.

PM next (per dispatch §7): consume → review §5/§6 → resolve §7 scope-class items (especially §7.1, the close-vs-greenfield framing, which gates THINK shape) → file THINK dispatch or a clarifying NOTES exchange → `PM-DISPOSITION-*-phase-2-observe-close.md`.

## §9 — References

- `Plans/DISPATCH-2026-05-24-phase-2-observe-cerebro.md` — this dispatch.
- Audit chain: `PM-AUDIT-SCOPE-…` (`5da364d`), `PM-AUDIT-…` (`c2a4411`), `CEREBRO-OUTPUT-…audit-day-2-findings.md` (`bb1b679`), `PM-DISPOSITION-…audit-close.md` (`199fb51`), `PM-DISPOSITION-…f005-close.md` (`5884ffd`), `CLOSE-REPORT-…f005-forced-kill-zombify.md` (`69df60b`).
- `docs/phases/PHASE-2.md`; `docs/bible.md` (v1.7.0) Articles 7.4, 8.3, 8.5, 12.1, 13, 14, 16.4.
- ADRs: 010 (cap foundation), 012 (buddy+slab), 013 (SMP retrofit), 007 (process termination), 019 (IrqSafeMutex), 022 (fault-recovery marker).
- `Plans/Q9-EXPLORATION-2026-05-11.md` (canonical Phase-2-BUILD-3 remaining-work synthesis); `Plans/HANDOFF-2026-05-11-end-of-day-phase-2-build-3-substantially-complete-phase-3-think-v1.md` (pivot-to-Phase-3 record).
- `Plans/PHASE-4-THINK-v0.2-…`, `Plans/PL-RATIFICATION-…q1-q12.md` (§5 format reference).
- Source confirmation: `kernel/src/arch/x86_64/smp.rs` (`bring_up_aps`, `ap_entry`), `kernel/src/mm/{frame,slab,tlb_shootdown}.rs`, `kernel/src/sync/{irqsafe_mutex,switch_mutex}.rs`.
- Commits: `f5a4984` (BUILD-2 close), `e2b3f4f` (SC-3B), `ead8b19` (SC-3C), `57f3cb1` (SC-4), `64a92e1` (Q9 deferral), `27d1b99` (IrqSafeMutex A), `9a282f2` (F-005 fix).

---

**End of OBSERVE deliverable.**

*Headline: Phase 2 core is substantially landed; the dominant finding is the close-vs-greenfield scope question (§7.1). §5 enumerates 14 THINK seeds, unanswered. §6 recommends cap-reserved-bit activation as the cleanest first BUILD candidate, pending PM's §7.1 resolution.*
