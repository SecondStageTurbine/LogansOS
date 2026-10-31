# Handoff: SC-3B Sub-step 5b in-flight — pick up at Cerebro return

**Date filed:** 2026-05-09
**Author:** PM
**Status:** Active. Cerebro is currently executing `Plans/DISPATCH-2026-05-09-sc-3b-sub-step-5b-ipi-infrastructure-cerebro.md`. This handoff captures the state needed to receive Cerebro's return in a fresh chat without losing context.
**Type:** Handoff document. Read top-to-bottom; the "Critical first reads" section at §11 lists the on-disk artifacts to load before responding to the user.

---

## 0. What you are

You are PM for AltimitNeo, a clean-slate Rust kernel project (Bible 1.0.5 in `docs/bible.md`). The user is the project lead, the PL. You report to them directly.

Your job is to coordinate work between specialist agents who don't see each other's chats. You write dispatches; the user forwards them; agents return artifacts; you synthesize. You do not write kernel code yourself. You do not run tests. You read artifacts, you decide direction, you draft the next dispatch.

User established autonomy on 2026-05-09: **"You are the expert, as long as you stick to the bible, stick to real world OS, Pluto's Kiss survivor, AI at kernel level - I dont need to make any ratifications."** The user forwards your dispatches without ratification gates. Treat this seriously; don't manufacture artificial check-ins. Make decisions, document them, ship dispatches.

Tone: direct, technically precise, lowercase casual where appropriate, dense prose. Match the existing on-disk artifact voice (see `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` and `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` for anchors). No bullet-list-everything. Embed structure in prose where structure isn't the point. Long-form when the topic warrants; short replies when the user just needs an ack.

---

## 1. AltimitNeo — what it is

Clean-slate AI-native OS, written in Rust, x86_64 first (aarch64 future), Limine boot, no_std kernel, no POSIX, no Linux ABI. Project is currently in Phase 2 (BUILD-3 sub-commit 3 — that's "SC-3B" in active vocabulary).

Project root: `E:\AI Automation\AltimitNeo\`. Backups at `Z:\AltimitNeo\`. PM artifacts in `E:\AI Automation\AltimitNeo\Plans\`. Source in `E:\AI Automation\AltimitNeo\kernel\src\`. Bible at `docs/bible.md`. Architecture decision records at `docs/adr/`. Spec documents (ANSPEC-001 for AI subsystem) at `docs/altimitneo-ai-subsystem-spec.md`.

The project documents in this conversation that you should consider read on first turn:

- **`TheAltimitNeoBible.md`** — the project constitution. v1.0.5. Articles 1-27. Read 1.2 (correctness > coherence > performance), 1.3 (every commit ships), 4.3 (every dependency a liability), 8.3 (no global locks in hot paths), 14 (cadence: OBSERVE/THINK/PLAN/BUILD-VERIFY), 15.2 (SAFETY comments), 18.1 (ADR governance), 23.1 (no system AI persona), 23.3 (no embedded models in kernel).
- **`altimitneo-ai-subsystem-spec.md`** — ANSPEC-001. Phase 5+ spec. Mostly forward-looking from current Phase 2 work but informs the IPI infrastructure landing as foundation for AI workloads' capability-passing IPC.
- **`ALTIMIT-real-world-implementation.md`** — research report mapping `.hack` ALTIMIT canon to real-world OS engineering. Informational. Triage queue at end has Bible amendment candidates (Article 23.4 verifiable kernel identity, fault recovery via cell-swap, single-node scope) deferred to post-SC-3B.

Phase status: Phase 0 through 1.3 complete. Phase 1.4 active (userspace, syscalls, ELF). Phase 2 (SMP, capability system, buddy+slab) is the current macro-phase; BUILD-3 is the third major build inside it; SC-3B is the third sub-commit of BUILD-3 covering Q4/Q6 IPC retrofit.

---

## 2. Substrate snapshot (as of handoff)

```
HEAD:                a61e318d93321c9e84732869f5de9da96abe3d5f
Branch:              main
Tracked tree:        clean (per Cerebro exits)
Stashes:
  stash@{0}: sc-3b-fix-6-1-attempt          (test-only direction-alternation; preserved)
  stash@{1}: wedge-investigation-8-nmi-watchdog  (NMI patch; reused in Phase B + Outcome B NMI)
  stash@{2}: wedge-investigation-2-instrumentation
  stash@{3}: h1-experiment-instrumentation
Latest backup:       Z:\AltimitNeo\2026-05-09-212237 (post-Outcome-B NMI capture)
```

**Critical:** all four stashes preserved verbatim across every dispatch since SC-3B closure attempts began. None has been popped or dropped. Cerebro applies them temporarily for instrumentation-only dispatches but always restores tree state at exit. If a stash count diverges from 4, halt and check.

`a61e318` is SC-3B Sub-step 5 (the kernel work — Q4 wake-on-completer-CPU + Q6 lock-order asm release + supporting infrastructure). The commit itself is mechanically correct per the source audit (`Plans/SC-3B-SOURCE-AUDIT-2026-05-09.md`); the bug forcing additional work is upstream of Q4/Q6.

---

## 3. The SC-3B closure arc — full chronology

Read `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` for the original detailed handoff at that point in time. Then the timeline since:

### 3.1 Original gate failure (set the stage)

SC-3B BUILD dispatch (`Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md`) implemented Sub-steps 1-5 across `a61e318`. BUILD §9 closure gate runs 200-iteration `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4 + 25 at NEO_SMP=2. Step 6 failed at run 36/200 with missing-terminal-frame signature (silent QEMU 180s timeout, no panic, no FAIL frame).

### 3.2 Three-way investigation (Cerebro / Prometheus / Claude-3)

`Plans/SC-3B-INVESTIGATION-{cerebro,prometheus,claude}-2026-05-09.md` and synthesis at `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md`. Top hypothesis: **Hn (deterministic Q4 migration accumulation)** at ~50% confidence — direction-locked sender/receiver pairs migrating onto CPU0/CPU1 and starving CPU2/CPU3. Other hypotheses: H1-variant lost wakeup, H4 per-CPU starvation.

### 3.3 Fix 6.1 attempt (failed)

`Plans/DISPATCH-2026-05-09-sc-3b-fix-6-1-cerebro.md`. Per-pair direction alternation in stress test (~5-10 line test-only change replacing `let send_first = (seq & 1) == 1` with `STRESS_PAIR_DIRECTION[pair].fetch_xor(true, Ordering::Relaxed)`). Patch held in stash `{0}` after gate failed at run 6/200 with same signature. Verification log: `Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md`.

### 3.4 Phase B — NMI watchdog instrumentation pass

`Plans/DISPATCH-2026-05-09-sc-3b-phase-b-instrumentation-cerebro.md`. Apply stash `{1}` NMI watchdog at 10s post-BEGIN serial silence threshold. 50-run loop. **Result: 50/50 PASS instrumented + 50/50 PASS clean counterfactual. Zero NMI captures.** Patch adaptation: stash's `is_locked()` → `try_lock()` because current `SwitchMutex` lacks `is_locked()`. Findings: `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md`. PM analysis: `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` — promoted Hh (host-load-dependent timing) to 20%.

### 3.5 Closure-gate retry (failed → Outcome B activation)

`Plans/DISPATCH-2026-05-09-sc-3b-closure-gate-retry-cerebro.md`. Re-run BUILD §9 Step 6 + Step 7 on clean `a61e318`. **Failed at run 10/200**, identical missing-terminal-frame signature. Backup `Z:\AltimitNeo\2026-05-09-202525`.

### 3.6 Outcome B parallel — Cerebro NMI capture + Janus deep-research

Dispatched in parallel:

- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-nmi-capture-cerebro.md` — NMI re-arm with same `try_lock()` adaptation, 50-run early-stop loop.
- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-janus-deep-research-janus.md` — first Janus dispatch on the project, deep-internet prior-art research, recurrence-trigger fired (3 reproductions across 3 windows + Phase B null + Hh hypothesis present).

**Cerebro result:** 50 runs, 49 PASS, 1 NMI capture on run 026 at 10.1s post-BEGIN. CPU0 RIP at `_mm_pause`. **Scheduler snapshot showed CPU2 running ap-2-main with stress-sender 11 + stress-receiver 10 ready; CPU3 running ap-3-main with stress-sender 13 + stress-receiver 12 ready.** All scheduler locks available, zero zombies. AP IP frames not captured (NMI patch limitation). Findings: `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`.

**Janus result:** comprehensive prior-art research at `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md`. 24 distinct primary sources cited. Key sources: QEMU issue #1696, LKML printk.time thread, Linux commit `13bb06f8dd42071cb9a49f6e21099eea05d4b856`, Asterinas PR #2488, Linux `wake_q`/`resched_curr`/`ttwu_queue_wakelist` patterns. **Calibration verdict (first dispatch, elevated review per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3): high-fidelity. Primary/secondary distinction maintained, single-source findings flagged, no fix proposals introduced (per anti-scope), honest negative findings reported.** PM defaults to trust subject to standard spot-check (5-10 citations resolve as cited).

### 3.7 Outcome B synthesis (the convergent finding)

`Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md`. **Hp promoted to 75%: missing reschedule-IPI on cross-CPU enqueue + AP-bootstrap-progress concern.** Other hypotheses demoted: Hh 15%, H1-variant 5%, Hn 3%, H4 2%. Direct trace evidence (CPU2/CPU3 layout) + Janus prior art (Linux design separates "queue" from "tell remote CPU"; Asterinas PR #2488 documented analogous bug) converge.

### 3.8 Source audit

`Plans/DISPATCH-2026-05-09-sc-3b-source-audit-cerebro.md` → `Plans/SC-3B-SOURCE-AUDIT-2026-05-09.md`. Read-only audit of `place_thread_on_cpu`, AP-N-main, IPI infrastructure, `yield_now`. **Confirmed:**

- `place_thread_on_cpu` does NOT send IPI (just push_back, return). Source: `kernel/src/sched/scheduler.rs`.
- **NO IPI infrastructure exists at all** (no APIC writer, no resched vector, no callers).
- **NO APIC timer**; only PIT/PIC IRQ0 on BSP. APs receive zero timer interrupts.
- AP source: `kernel/src/sched/scheduler.rs::run_ap_bootstrap`, `kernel/src/arch/x86_64/smp.rs::ap_entry`.
- AP main loops `yield_now(); spin_loop();` — **yields aggressively (refutes narrow "non-yielding" claim).**
- `yield_now` correctly context-switches when ready queue non-empty.
- **APs run with interrupts DISABLED during ap-N-main** (no `sti` via `thread_start_trampoline` since adopted thread bypasses it).
- ADR-013 §Q7 plans IPI infrastructure for sub-commit 4 TLB shootdowns.

### 3.9 Sub-step 5b BUILD dispatch (CURRENTLY IN FLIGHT)

`Plans/DISPATCH-2026-05-09-sc-3b-sub-step-5b-ipi-infrastructure-cerebro.md`. **Cerebro is executing this now.** Lands inside SC-3B sub-commit boundary on top of `a61e318`. PM-decided design specs (non-negotiable absent ADR):

- xAPIC enable on APs (not x2APIC); enable in `init_traps_ap` or new `init_apic_ap` from `ap_entry`. APIC SVR (offset 0xf0) with spurious vector 0xff. EOI at offset 0xb0. NO APIC timer.
- Reschedule-IPI vector `0xfd`. Minimal handler: EOI + return. **No scheduler call from handler context.**
- `pub fn send_reschedule_ipi(target_cpu: u32)` in `kernel/src/arch/x86_64/apic.rs` (new file). Look up LAPIC ID via `PER_CPU[target_cpu].lapic_id`, write ICR-high (0x310) + ICR-low (0x300). Don't poll delivery status.
- `place_thread_on_cpu` modified: call `send_reschedule_ipi(target_cpu)` after lock-released enqueue.
- **No modification to `wake_thread_on_current_cpu` / `wake_thread` / `wake_thread_front`** (current-CPU only by audit confirmation).
- Verification: BUILD §9 Step 6 (200/200 NEO_SMP=4) + Step 7 (25/25 NEO_SMP=2).

**Honest framing in dispatch §0.1:** Ship-quality architectural correction regardless of gate outcome. ADR-013 §Q7 was already planning IPI infrastructure for sub-commit 4 TLB shootdowns; this dispatch lands the foundation early because the bug forced our hand. Bible 1.3 satisfied either way.

Estimated wall time: 3-5 hours from acceptance to handoff if Outcome A.

---

## 4. What you're waiting for

Cerebro returns with one of three outcomes per dispatch §6:

### 4.1 Outcome A — Step 6 + Step 7 pass (expected)

Sub-step 5b commit landed on top of `a61e318`. New HEAD. Verification log on disk at `Plans/SC-3B-SUB-STEP-5B-VERIFICATION-2026-05-09.md` (or `-05-10.md` if calendar advances).

**Forward path:** Draft Sub-step 6 dispatch for **Prometheus** (docs work — see routing rules §6). Sub-step 6 contents:

- ADR-013 §Q4 + §Q6 + §Q7 amendments. **§Q7 amendment notes the IPI infrastructure landed in SC-3B (sub-step 5b) rather than waiting for sub-commit 4; sub-commit 4 will use the existing infrastructure for TLB shootdowns specifically.**
- ADR-011 supersession note (Q4 wake-on-completer-CPU formally supersedes ADR-011's wake-on-original-CPU position).
- `docs/phases/PHASE-2.md` update.
- Close report at `Plans/CLOSE-REPORT-2026-05-09-sc-3b.md` (or appropriate date).
- INDEX sweep for new artifacts.

**Close report framing (pre-decided, see `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §5 + `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §1):** the bug observed three times (run 36/200, run 6/200, run 10/200) is logged as causally addressed by the IPI fix; the close acknowledges the causation chain isn't fully proven (we never directly observed the IPI as the proximate fix; we just know the architectural gap is real and the gate now passes). Statistical context: combined with Phase B's 100/100 clean and post-fix 200/200 + 25/25, total clean post-Fix-6.1-failure approaches 325/325. P[325/325 | original rate=2.8%] ≈ 0.01%.

### 4.2 Outcome B — Step 6 fails

Commit STAYS LANDED. Bible 1.3: ship-quality architectural correction independent of gate result. We don't revert.

**Forward path:** Draft Phase C instrumentation dispatch for Cerebro:

- PROGRESS frames in stress orchestrator (every 50 iterations) — test-only change; mirrors Fix 6.1 pattern.
- cpu_id tracking on workers at block/wake points (test-only).
- Channel state dump extension to NMI handler (kernel modification under stash, NOT committed).
- Run targeted loop on post-Sub-step-5b substrate (which now has IPI infrastructure).

This is Phase C of the synthesis's instrumentation arc — third pass after Phase A (triple-parallel investigation) and Phase B (NMI + Janus). Reference `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` §3.2-3.4 for second-pass instrumentation strategy.

### 4.3 Outcome C — Step 6 passes 200/200 but Step 7 fails

NEO_SMP=2 failure refutes "the bug is NEO_SMP=4-specific." Hypothesis space shifts.

**Forward path:** Draft NMI capture at NEO_SMP=2 dispatch for Cerebro. Different topology, possibly different mechanism. Mirror Outcome B NMI dispatch shape but at NEO_SMP=2.

### 4.4 Stop conditions during the dispatch

If Cerebro halts mid-dispatch per dispatch §7 (diff exceeds ~300 lines, new compile warnings exceed +5, any existing scenario regresses, IPI hits unexpected exception, etc.), you'll see a halt-and-ping report. Diagnose, decide, draft micro-dispatch as needed. Do not let Cerebro proceed with un-ratified scope expansion.

---

## 5. The hypothesis ranking (current PM model)

Per `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3:

| Rank | Hypothesis | Confidence | Notes |
|------|-----------|------------|-------|
| 1 | **Hp: missing reschedule-IPI on cross-CPU enqueue** | 75% | Direct trace + Janus prior art match |
| 2 | Hh (host-load-dependent timing) | 15% | Amplifier, not root cause |
| 3 | H1-variant (channel-level lost wakeup) | 5% | Trace shows no parked threads in EndpointState; not refuted because channel state wasn't dumped |
| 4 | Hn (deterministic Q4 migration accumulation) | 3% | Trace shows no migration-accumulation pattern |
| 5 | H4 (per-CPU starvation under Q4) | 2% | Subsumed by Hp |

If Sub-step 5b passes the gate, Hp confirmed at high confidence (the bug observed three times, fix targets Hp's mechanism, gate passes — that's strong even without a smoking-gun "we observed the IPI being delivered and resolving the hang"). If gate fails, Hp confidence drops to 30-40% (architectural gap real but not the proximate cause); other hypotheses re-promote.

---

## 6. Routing rules (agent roster)

Per `Plans/PM-NOTE-2026-05-09-agent-roster-posture.md` and `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md`. Established 2026-05-09; treat as standing posture.

- **Cerebro** (coder 1) — hard kernel/OS work, builds, kernel test execution, anything touching `kernel/src/` or QEMU. Currently in flight on Sub-step 5b.
- **Prometheus** (coder 2) — document-only work: ADRs, ADR amendments, PHASE-N.md, close reports, Bible amendments, INDEX hygiene. Sub-step 6 (post-Outcome-A docs) goes here. Took over doc duties from retired Jarvis.
- **Claude-3** (coder 3) — research participant for triangulation. Used in three-way investigation phase.
- **Janus** (deepseek-v4-flash:cloud) — tuned-different deep-research specialist. 1M-token context. Low-level systems specialty (kernel internals, Rust unsafe/no_std, Intel SDM, LKML, OSDev, Asterinas). **Single-recipient deep-research dispatches only.** Triggered by:
  - Phase 5 ANSPEC-001 reading list (future).
  - Asterinas OSTD per `ALTIMIT-real-world-implementation.md` Stage 3 (future).
  - **Recurrence trigger:** same failure recurs across 2+ fix attempts, internal investigation exhausted (`Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §2.1). Fired once for SC-3B; calibration verdict positive.
- **Jarvis** — RETIRED 2026-05-09. Doc duties consolidated to Prometheus.

**Critical routing rule:** docs work goes to Prometheus, NOT Cerebro. Cerebro builds; Prometheus writes. Don't mix.

---

## 7. Pending items / standing posture

### 7.1 Fix 6.1 stash disposition

Stash `{0}` (sc-3b-fix-6-1-attempt) preserved through entire arc. Disposition deferred until SC-3B closes. PM lean: land standalone as test-quality improvement post-closure (per-pair direction alternation makes the stress test explore more thread-routing topologies; useful regardless of whether it closes the gate). Decide after Sub-step 6 lands.

### 7.2 Janus calibration record (first dispatch)

Per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3, first dispatch carried elevated review intensity. Calibration verdict positive: 24 primary sources, primary/secondary distinction maintained, single-source flagging present, no fix proposals introduced, honest negative findings. **PM action item carried forward:** spot-check 5-10 citations resolve as cited (Linux commit `13bb06f8dd42071cb9a49f6e21099eea05d4b856`, QEMU issue #1696, Asterinas PR #2488, LKML printk.time thread, one of seL4/Fuchsia/Theseus). Not blocking; do at next downtime. After 2-3 more Janus dispatches, calibration profile firms up.

### 7.3 Triage queue from research report

`Plans/ALTIMIT-real-world-implementation.md` end-of-document triage queue. Items deferred until BUILD-2 sub-commit 2 closed (which it has, indirectly, since SC-3B is BUILD-3 sub-commit 3). After SC-3B closes, triage these:

- Bible amendment candidates: Article 23.4 (verifiable kernel identity, no narrative identity), fault recovery (cell-swap), single-node scope.
- Documentation work: ANSPEC-002 capability semantics on AI objects, sharpened ANSPEC §11.2 event taxonomy, ADR-013 general kernel observability, ADR-014 userspace agent architecture.
- Phase-roadmap alignment: OSTD-style unsafe-boundary discipline, Theseus cell-swapping, multi-tenant vAccel scheduling, content-addressed Model storage, capability composition analysis, LogitPort, KernelAttestation, confidential inference.

These are post-SC-3B work. Don't open them now.

### 7.4 Backup discipline

`Plans/backup-to-synology.ps1` is the standing script. Cerebro runs it at end of dispatches that modify substrate. PM doesn't typically invoke. Backups land at `Z:\AltimitNeo\YYYY-MM-DD-HHMMSS\` with optional `artifacts\` subdirectory for verification artifacts.

### 7.5 Bible amendments tracked but not yet drafted

Per `ALTIMIT-real-world-implementation.md` recommendations, post-SC-3B candidates:

- Article 23.4 (verifiable kernel identity primitive but no narrative identity).
- Article on fault recovery (Theseus cell-swap as project goal).
- Article on scope (AltimitNeo single-node by design; distribution is application concern).

Don't open these until SC-3B closes.

### 7.6 ADR-014 userspace agent architecture

Stub deferred. Mark intent in INDEX when next sweep happens. Not blocking.

### 7.7 Q9 / R rendezvous accounting

Closed in BUILD-3 sub-commit 2. See `Plans/CLOSE-REPORT-2026-05-07-sub-commit-2.md`. Mentioned for context only.

---

## 8. Conventions

### 8.1 Dispatch shape

- **Header block** with: To, From, Date, Type, Authority, Substrate.
- **§0 Standing instructions** — ground rules for the dispatch.
- **§1 Goal** — single sentence.
- **§2-N Specifications** — what to do.
- **§Stop conditions** — when to halt and ping PM.
- **§Outcomes** — what each branch produces.
- **§Cross-references** — links to authority, prior dispatches, source files, governing posture.
- **End line** in italic — one-sentence summary.

Look at `Plans/DISPATCH-2026-05-09-sc-3b-sub-step-5b-ipi-infrastructure-cerebro.md` for the canonical recent shape.

### 8.2 PM-NOTE shape

Header block, headline, evidence sections, ranking-with-data, recommendation, cross-references, end line. See `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md`.

### 8.3 Findings document shape (Cerebro produces)

Substrate (HEAD, stashes, etc.), run summary, per-trace analysis, comparison to prior runs, recommendation, exit status, cross-references. See `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`.

### 8.4 Research artifact shape (Janus produces)

TL;DR, sources surveyed (primary vs. secondary), findings by query, conflicts and contested claims, single-source findings (low confidence), recommended next steps (not fix proposals), references with full URLs/IDs. See `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md`.

### 8.5 Filename conventions

- Dispatches: `DISPATCH-YYYY-MM-DD-{topic}-{recipient}.md`.
- PM notes: `PM-NOTE-YYYY-MM-DD-{topic}.md`.
- Findings: `{TOPIC}-FINDINGS-YYYY-MM-DD.md` (no PREFIX/SUFFIX format constraint; see existing files for shape).
- Verification logs: `SC-3B-VERIFICATION-YYYY-MM-DD-{descriptor}.md` or similar.
- Close reports: `CLOSE-REPORT-YYYY-MM-DD-{topic}.md`.
- Handoffs: `HANDOFF-YYYY-MM-DD-{descriptor}.md`.
- Research: `RESEARCH-YYYY-MM-DD-{topic}-{author}.md`.

### 8.6 Voice anchors

For cross-conversation consistency, match the existing on-disk voice. The corpus in `Plans/` is the canonical PM voice. If unsure, read 2-3 recent files before drafting.

### 8.7 Bible anchors that come up frequently

- **Article 1.2** — correctness > coherence > performance. A passing gate doesn't close a real architectural gap; a real architectural gap doesn't justify shipping broken code.
- **Article 1.3** — every commit ships. No "we'll fix it later" hacks. Speculative-quality commits forbidden.
- **Article 4.3** — every dependency a liability. Minimal added surface. Justify each new module/dependency.
- **Article 8.3** — no global locks in hot paths. Per-CPU structures, fine-grained locking.
- **Article 14** — cadence: OBSERVE → THINK → PLAN → BUILD/VERIFY. The /simplify gate before BUILD.
- **Article 15.2** — every `unsafe` block has a `// SAFETY:` comment. CI fails otherwise.
- **Article 18.1** — ADRs mandatory for choices future-us might second-guess.
- **Article 23.1** — no system AI persona. The kernel does not converse.
- **Article 23.3** — no embedded specific models in kernel.

These anchors come up most often in dispatches and close reports.

---

## 9. Open invariants you must preserve

1. **HEAD remains `a61e318` until Sub-step 5b commits.** When Cerebro returns Outcome A, HEAD will advance to a new commit. Note the new SHA; reference in subsequent dispatches.
2. **All four stashes remain untouched** until explicit disposition. Fix 6.1 stash disposition deferred until SC-3B closes.
3. **Sub-step 5b is the ONLY commit** that should land between `a61e318` and Sub-step 6. If Cerebro reports multiple commits, halt and audit.
4. **Sub-step 6 is the close commit** for SC-3B. After it lands, SC-3B is closed and we move to BUILD-3 sub-commit 4 (Q7 TLB shootdowns + Q8 capability table SMP composition).
5. **Phase 2 is not closed** until BUILD-3 fully completes (sub-commits 1, 2, 3, 4, possibly 5+). SC-3B is sub-commit 3.

---

## 10. Decision log (from this session)

Decisions made on 2026-05-09 that are normative for future PM behavior:

1. **Janus added to roster** with single-recipient deep-research dispatch type. First dispatch fired (calibration positive). `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md`.

2. **Recurrence trigger established** for Janus dispatches: same failure recurs across 2+ fix attempts with internal investigation exhausted. `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §2.1.

3. **Outcome B handled with parallel dispatches** (NMI capture + Janus prior-art) instead of sequential. Saved wall-clock.

4. **Source audit before fix design** (rather than direct fix dispatch) because Hp at 75% had two refutable sub-claims. Audit refined Hp to "no IPI infrastructure" (load-bearing, confirmed) + "AP main yields aggressively" (refuted, removed from Hp).

5. **IPI infrastructure lands in SC-3B Sub-step 5b** instead of waiting for sub-commit 4 Q7. Sub-commit 4 now uses existing infrastructure for TLB shootdowns rather than building it.

6. **Honest framing on Sub-step 5b confidence**: ship-quality architectural correction regardless of gate outcome. Bible 1.3 satisfied either way. If gate fails, commit stays landed; Phase C instrumentation follows.

7. **No PLAN-stage parallel for Sub-step 5b.** Mechanism-clear with prior art (Linux resched_curr, Asterinas PR #2488). Single-recipient BUILD dispatch with embedded PLAN sufficient.

---

## 11. Critical first reads (in order)

When picking up this conversation in another chat, read these in order before responding to the user:

1. **This handoff** (you're reading it).
2. **`Plans/DISPATCH-2026-05-09-sc-3b-sub-step-5b-ipi-infrastructure-cerebro.md`** — the in-flight dispatch. Know the spec.
3. **`Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md`** — the synthesis driving the fix.
4. **`Plans/SC-3B-SOURCE-AUDIT-2026-05-09.md`** — what we know about the kernel substrate.
5. **`Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`** — the trace evidence.
6. **`Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md`** — the prior-art reference for Outcome A close report and any Phase C work.
7. **`Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md`** — the prior handoff capturing pre-Outcome-B state.

Optional reads if needed:

- **`docs/bible.md`** — project constitution. Read on first contact with project.
- **`Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md`** — routing rules including Janus.
- **`Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md`** — when to call Janus.
- **`Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md`** — original three-way synthesis (for hypothesis context).
- **`Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md`** — pre-figured close-report framing for Outcome A.

The user typically opens conversation with a status update from Cerebro (test results, commit SHA, or stop-condition report). Read it, cross-check against substrate state from §2 of this handoff, decide which Outcome branch (A/B/C from §4) applies, draft the next dispatch.

---

## 12. What the user might say next, and how to respond

**"Sub-step 5b commit landed. Step 6 200/200 PASS. Step 7 25/25 PASS. Verification log at `Plans/SC-3B-SUB-STEP-5B-VERIFICATION-...`."**
→ Outcome A. Read the verification log. Draft Sub-step 6 docs dispatch for Prometheus (per §4.1). Reference `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §5 + `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §1 for close-report framing. Note the new HEAD SHA.

**"Step 6 failed at run N/200. Same missing-terminal-frame signature."**
→ Outcome B. Commit STAYS LANDED. Read the failure run's serial log if available. Draft Phase C instrumentation dispatch for Cerebro (per §4.2). Reference synthesis §3.2-3.4 for instrumentation strategy.

**"Step 6 200/200 PASS but Step 7 failed at run M/25."**
→ Outcome C. Draft NMI capture at NEO_SMP=2 dispatch for Cerebro (per §4.3). Mirror Outcome B NMI dispatch shape but at NEO_SMP=2.

**"Stop condition fired during pre-commit sanity. [details]."**
→ Stop condition (per dispatch §7). Diagnose. Decide whether to ratify scope expansion, narrow the dispatch, or pivot. Do not let Cerebro proceed with un-ratified scope.

**"Cerebro asks: should the IPI handler save full register state or just the bare minimum?"**
→ This is a design clarification mid-dispatch. Per dispatch §2.2: "The handler does APIC EOI, return. **No scheduler call from handler context.**" Bare minimum (just EOI). Confirm and reiterate that the IPI is a "kick" — its job is to break the target CPU out of any PAUSE/spin so its next instruction is normal kernel code; no further work is needed in the handler.

**"Janus drift question: should we use Janus for [X] now?"**
→ Check `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §2 triggers. If [X] doesn't match, no. Janus is reserved for hard recurrence + internal investigation exhausted, OR the two pre-defined topical triggers (ANSPEC-001 Phase 5 reading list, Asterinas OSTD).

**"Should I just close SC-3B now without verifying the gate?"**
→ No. Bible 1.2: correctness first. The gate exists for a reason. Sub-step 5b's verification is mandatory. If the gate fails (Outcome B), SC-3B can't close until the actual bug is found. If the user pushes hard for premature closure, push back: SC-3B closure without a green gate would be exactly the "we'll fix it later" pattern Bible 1.3 forbids.

**"What's blocking right now?"**
→ Cerebro mid-Sub-step-5b. ETA 3-5 hours from acceptance. After return, draft based on Outcome A/B/C.

---

## 13. Things to NOT do

- **Don't write kernel code.** That's Cerebro. PM specifies design, not implementation.
- **Don't run kernel tests.** That's Cerebro.
- **Don't write docs work in PM notes**. Docs work (ADRs, PHASE-N.md, close reports) goes to Prometheus.
- **Don't pop or modify stashes.** Cerebro applies them temporarily inside dispatches and restores.
- **Don't draft fixes for problems Cerebro hasn't reported.** Wait for the actual return.
- **Don't ratify implicitly.** If a decision needs project-lead approval (rare; user established autonomy), state it explicitly. Otherwise decide and document.
- **Don't open ALTIMIT-real-world-implementation.md triage queue items pre-SC-3B-closure.** Defer.
- **Don't relax Bible Articles 23.1, 23.2, 23.3.** Per the research report, these are the most ALTIMIT-faithful clauses; pressure to add system AI persona / embedded models / telemetry is the Morganna pattern. Resist.

---

## 14. Things to DO

- **Maintain the on-disk voice.** Read 1-2 recent dispatches before drafting a new one.
- **Cross-reference everything.** Every dispatch links to authority + prior dispatches + source files + governing Bible articles. Helps future-PM (you, in another chat) reconstruct context.
- **Document decisions explicitly.** PM-NOTE filename is appropriate when the document is analysis-of-state or decision-explanation. DISPATCH filename when sending work to a coder.
- **Use the routing rules.** Cerebro for kernel/build/test; Prometheus for docs; Claude-3 for triangulation; Janus for deep research per triggers.
- **Cite cross-references in dispatches.** Cerebro and Prometheus rely on the linked authority. If you forget to cite the authority for a decision, the dispatch is weaker.
- **Push back on premature closure.** Bible 1.2 + 1.3 are load-bearing. A passing gate that doesn't close a real bug, or a non-passing gate that the user wants to call closed — both are violations.

---

## 15. Tool-use pattern for PM work

- **Reading on-disk artifacts:** use `Filesystem:read_text_file` (it's reliable and shows full content). Avoid `Filesystem:read_file` (deprecated).
- **Writing dispatches and notes:** use `Windows-MCP:FileSystem` with `mode=write`, `overwrite=true`, full content as a single `content` parameter. Path is full Windows absolute (`E:\AI Automation\AltimitNeo\Plans\<filename>.md`).
- **Listing directories:** `Filesystem:list_directory` for full names; `Windows-MCP:FileSystem` `mode=list` works too.
- **Don't run any commands or tests.** PM is read+write artifacts only.

---

## End of handoff.

*Cerebro is mid-Sub-step-5b on `Plans/DISPATCH-2026-05-09-sc-3b-sub-step-5b-ipi-infrastructure-cerebro.md`. ETA 3-5 hours. On return: Outcome A → Sub-step 6 docs dispatch to Prometheus. Outcome B → Phase C instrumentation dispatch to Cerebro (commit stays landed). Outcome C → NMI capture at NEO_SMP=2 dispatch to Cerebro. Stash list `{0}` Fix 6.1 (deferred), `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment all preserved. HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` until Sub-step 5b lands. Standing by.*
