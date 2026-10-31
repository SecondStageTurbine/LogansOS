# Handoff: End-of-day 2026-05-09 — SC-3B investigation complete, fix ratified, ready for execution

**Date:** 2026-05-09
**Author:** PM
**Purpose:** Comprehensive context handoff to enable a fresh chat session to pick up SC-3B work without loss of context. New chat session opens by drafting the SC-3B Fix 6.1 BUILD execution dispatch for Cerebro. Per Bible Article 14, this is operational handoff, not normative — no governance changes embedded.
**Substrate at handoff:** HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` (SC-3B Sub-step 5 commit). Backup: `Z:\AltimitNeo\2026-05-09-144314`. Three preserved stashes unchanged.

---

## 0. Snapshot

**Where we are:** SC-3B (Phase 2 BUILD-3 sub-commit 3, Q4 + Q6 + IPC retrofit) implemented through Sub-step 5. Stress-gate failed at run 36/200 (silent kernel hang). Cerebro followed stop discipline correctly. Three-way parallel investigation (Cerebro, Prometheus, Claude-3) completed. PM identified leading hypothesis (Hn — deterministic Q4 migration accumulation from constant pair-direction assignment in stress test) at 50–55% confidence with two independent corroborations. Fix 6.1 (per-pair direction alternation in test harness, ~5-10 line test-only change) is RATIFIED by project lead. Sub-step 6 (docs + close) is blocked until Fix 6.1 closes the 200/200 gate.

**Immediate next action:** PM drafts SC-3B Fix 6.1 BUILD execution dispatch for Cerebro. Cerebro implements (1 commit), re-runs the stress closure gate, captures new verification log. If gate passes, Sub-step 6 opens.

**Synthesis bookkeeping:** Option (a) chosen — Cerebro's pre-synthesis at `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` stands as PM's synthesis-of-record. PM's three-way comparison + calibration observations + recommendation are captured inline in this session's chat history and summarized in §3 below; they will fold into the eventual close report. No separate synthesis file produced.

---

## 1. Context for the new chat session

The new PM session inherits an active project. Key things to know on day one:

**Project:** AltimitNeo — clean-slate AI-native operating system, Rust kernel + userland, Limine-bootloaded x86_64 (aarch64 + riscv64 future), `#![no_std]`, no POSIX. Phase 2 BUILD-3 in progress. Bible at `docs/bible.md` v1.0.5. Project root `E:\AI Automation\AltimitNeo\`. Backups `Z:\AltimitNeo\`. Filesystem MCP for read/write to user machine.

**PM identity:** the PL. PM voice is tight, source-grounded, calibrated about uncertainty, defers to project lead on ratifications. PM does not unilaterally amend Bible, ADRs, or commits. PM produces dispatches and notes; coders implement.

**Coders:**
- **Cerebro** (coder 1) — primary kernel implementer. Has direct source access, can `git grep`, can build, can run kernel tests in QEMU. Most reliable for kernel work.
- **Prometheus** (coder 3) — recently added helper. Has source access; performed the SC-3B PLAN parallel analysis and the stress-gate parallel investigation. Calibration data (see §3.4 below) suggests Prometheus's source audits sometimes miss serialization context. Useful as parallel/redundant analyst, not yet PM's primary for execution.
- **Jarvis** (coder 2) — docs and INDEX.md hygiene. Currently on standby. Could pick up an INDEX sweep.

**Three preserved stashes** (untouched by this session):
- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

`stash@{0}` is referenced as candidate Phase B instrumentation if Fix 6.1 doesn't close the gate. Apply for investigation only; do not commit.

**Bible articles invoked frequently this session:**
- 1.2 (correctness > coherence > performance)
- 1.3 (every commit is intended to ship)
- 4.3 (every external dependency is a liability)
- 8.3 (no global locks in hot paths)
- 8.5 (critical-section discipline, ratified at v1.0.5)
- 12.1 (capability-based security from Phase 2)
- 14 (OBSERVE → THINK → PLAN → BUILD/VERIFY cadence; /simplify gate)
- 15.2 (SAFETY comments on unsafe blocks)

---

## 2. This session — chronological summary

Session date 2026-05-09, ~12 hours of activity.

### 2.1 Started post-wedge-fix-discharge

Inherited state: wedge-investigation arc closed, Bible 8.5 ratified at v1.0.5, ADR-016 (A1 IRQ-safe heap) ratified, β bookkeeping complete. SC-3B THINK stage queued.

### 2.2 SC-3B THINK stage

PM produced `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md`. Three-lens analysis (Bible + ALTIMIT real-world posture + AI-native premise via ANSPEC-001) converged on Option (ii): SC-3B = ADR-013 Q4 + Q6 + IPC retrofit + cross-CPU scenarios, multi-commit within one sub-commit. SC-3A retired as a name. PL ratified.

### 2.3 SC-3B PLAN stage (parallel)

PM dispatched parallel PLAN analysis at `Plans/DISPATCH-2026-05-09-sc-3b-plan-stage-parallel.md` to both Cerebro and Prometheus. Both produced PLAN documents:
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` — recommended per-call-site ABI, kernel-owned `SwitchMutex`, Channel-first release order.
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md` — recommended const-generic array ABI, `spin::Mutex::into_raw()` (API doesn't exist), scheduler-first release order in implementation sketch.

PM produced `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` walking each open question. Recommended Cerebro's positions on the three meaningful divergences (ABI shape, lock-byte ownership, release order) and the converged answers on the rest. PL ratified the synthesis (8 of 10 lean Cerebro + 2 converged). Prometheus revised their PLAN to align.

### 2.4 SC-3B BUILD dispatch and execution

PM produced `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` — comprehensive 6-sub-step playbook covering:
- Sub-step 1: SwitchMutex + Channel/Scheduler migration + Thread.cpu_id + test placement hook
- Sub-step 2: context_switch_with_release2 asm helper + block_current_into_with_guard wrapper + smp_block_release_smoke
- Sub-step 3: send/recv retrofit + Q4 wake-on-completer-CPU + 2 cross-CPU scenarios
- Sub-step 4: grant/recv_grant retrofit + smp_grant_cross_cpu
- Sub-step 5: migration/reap + stress + verification gate (200/200 closure)
- Sub-step 6: ADR-013 §Q4+§Q6 amendment + close report + INDEX sweep

Cerebro executed Sub-steps 1-5 over ~3 hours. Final commit `a61e318` lands at 14:43. Backup at `Z:\AltimitNeo\2026-05-09-144314`.

**Sub-step 5 verification gate:** All steps 1-5 passed. Step 6 (200-run primary stress at NEO_SMP=4) failed at run 36/200 — silent kernel hang, no PASS/FAIL/BUG/PANIC, QEMU 180s timeout. Cerebro followed dispatch §9.5 stop discipline: halted, classified, reported. Captured `Plans/SC-3B-VERIFICATION-2026-05-09.md` and failure log at `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log`.

### 2.5 Stress-gate triage

PM produced `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md` (~300 lines). Affirmed stop discipline. Surfaced six hypotheses (H1–H6). Recommended 5-phase investigation arc.

### 2.6 Triple parallel investigation

PL directed three parallel investigations (Cerebro, Prometheus, plus a fresh Claude session "Claude-3"). PM produced `Plans/DISPATCH-2026-05-09-sc-3b-investigation-parallel.md` (~600 lines, self-contained for fresh-session onboarding). PM provided briefing text for PL to paste to Claude-3.

Three investigation reports landed:
- `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md` — H3 (stress orchestration) at 30% top, H1 25%, H5 18%, H4 12%, H2 8%, H7 5%, H6 2%. Conservative; did not isolate Hn.
- `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md` — H7 (atomic extraction not safe) at 70% top. **Ruled out by SwitchMutex serialization** which Prometheus did not engage with. Calibration data.
- `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md` — Hn (deterministic Q4 migration) at 55% top. Independent mathematical derivation of pair-direction determinism. Q4 migration map table.

Cerebro additionally produced an extensive 45KB analysis at `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` — Cerebro's deeper second-pass which independently identified Hn at 50%. Originally named "synthesis" prematurely (before all three reports were in). Per option (a) bookkeeping ratified by PL, this document stands as PM's synthesis-of-record.

### 2.7 Three-way comparison and ratification

PM compared all four documents inline in chat (final assistant message before this handoff):
- **Cerebro pre-synthesis (50%) and Claude-3 (55%) independently converged on Hn.** Strongest signal.
- **Prometheus's H7 ruled out by SwitchMutex audit.** Calibration data.
- **Cerebro's per-agent report ranked H3 top (30%)** — different from Cerebro's pre-synthesis. Single-pass analysis under-resolved Hn even with full source access. Methodologically useful observation.

PM recommended **Fix 6.1: per-pair direction alternation in stress test** (test-only change, ~5-10 lines, replaces `let send_first = (seq & 1) == 1` with per-pair `AtomicBool::fetch_xor`). High independent confidence; low risk; no kernel changes.

PL ratified: Fix 6.1 + option (a) for synthesis bookkeeping.

---

## 3. Ratified decisions this session

These are the active normative outputs of the session. New chat treats as authoritative.

### 3.1 SC-3B THINK stage scope ratified

SC-3B = ADR-013 Q4 + Q6 + IPC retrofit + cross-CPU scenarios, multi-commit within one sub-commit boundary. SC-3A retired as a name; pre-thinking note becomes input to SC-3B PLAN.

### 3.2 SC-3B PLAN ratified positions

- Q6 ABI: `context_switch_with_release2(prev_rsp_ptr, next_rsp, channel_lock, sched_lock)`, per-call-site, explicit register lock pointers, no release-list metadata after stack switch.
- Lock primitive: kernel-owned `SwitchMutex` (~30-50 lines), not `spin::Mutex` internals.
- Release order: Channel first, scheduler second.
- BUILD scope includes Thread.cpu_id field and controlled cross-CPU test placement hook.
- IPC retrofit covers exactly four block paths: send, recv, grant, recv_grant.

### 3.3 SC-3B Sub-steps 1-5 implementation

Substrate commit `a61e318` contains:
- `SwitchMutex<T>` type at (location chosen by Cerebro during Sub-step 1).
- `Channel.state` and `PerCpuScheduler.inner` migrated to SwitchMutex.
- `Thread.cpu_id: AtomicU32` field, set on spawn/push_ready/wake/wake_front.
- `place_thread_on_cpu` test placement hook (test-feature-gated).
- `context_switch_with_release2` asm helper in `kernel/src/arch/x86_64/context.rs:48-68`.
- `block_current_into_with_guard<T, F>` wrapper in `kernel/src/sched/scheduler.rs:483-531`.
- `wake_thread_on_current_cpu` Q4 helper.
- All 4 IPC sites retrofitted (send, recv, grant, recv_grant).
- Six new test scenarios: smp_block_release_smoke, smp_send_blocks_first_then_completes, smp_recv_blocks_first_then_completes, smp_grant_cross_cpu, smp_cross_cpu_migration_reap, smp_cross_cpu_rendezvous_stress.

Sub-step 5 verification confirmed: cargo k passes (18 warnings), default cargo neo-test suite passes 33/33 + expected meta-test FAIL, all 5 supplementary scenarios pass 50/50, primary stress fails at 36/200. The closure gate is blocked.

### 3.4 Calibration on Prometheus

Prometheus's PLAN-stage analysis had three issues PM flagged in `SC-3B-PLAN-SYNTHESIS-2026-05-09.md`: lock-order inverted from pre-thinking I1, asm sketch had old-stack-read failure mode, `into_raw()` API uncertainty unresolved. Prometheus revised after synthesis feedback.

Prometheus's investigation H7 (atomic extraction) is ruled out by SwitchMutex serialization that Prometheus didn't audit. Their proposed `try_extract_parked_thread` fix addresses a non-existent bug.

This is calibration data. Prometheus engages seriously and produces clean structure, but source audits sometimes miss serialization context. Useful as parallel/redundant analyst; PM's primary for execution remains Cerebro.

### 3.5 Synthesis bookkeeping (option a)

`Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` — Cerebro's pre-synthesis, comprehensive source-audit and Hn identification — stands as PM's synthesis-of-record. PM's three-way comparison + Prometheus calibration + Claude-3 corroboration are captured in this session's chat history and §2.7 above. New chat does not need to produce a separate synthesis file. Eventual close report references this handoff for the comparison content.

### 3.6 Fix 6.1 ratified

**Per-pair direction alternation in stress test.** Specifically:

```rust
// In test_harness.rs, add to static section:
#[cfg(feature = "neo-test-smp-cross-cpu-rendezvous-stress")]
static STRESS_PAIR_DIRECTION: [core::sync::atomic::AtomicBool; STRESS_MAX_PAIRS] =
    [const { core::sync::atomic::AtomicBool::new(false) }; STRESS_MAX_PAIRS];

// In reset_stress_state():
for idx in 0..STRESS_MAX_PAIRS {
    STRESS_PAIR_DIRECTION[idx].store(false, Ordering::Release);
    // ... existing resets ...
}

// In run_smp_cross_cpu_rendezvous_stress(), replace:
let send_first = (seq & 1) == 1;
// With:
let send_first = STRESS_PAIR_DIRECTION[pair].fetch_xor(true, Ordering::Relaxed);
```

Test-only change. No kernel code modification. ~5-10 lines.

Verification: re-run the BUILD dispatch §9 verification gate. Specifically, the 200-run primary stress closure at NEO_SMP=4 is the relevant subset. Mirror L's discharge framing if 200/200 passes.

If Fix 6.1 fails (gate still red after the change), Phase B instrumentation opens. Apply `stash@{0}` (NMI watchdog) for investigation only — do not commit. Add per-iteration PROGRESS frames every 50 iterations. Add migration logging in workers. Collect data, return to ranking-with-data.

---

## 4. Forward queue (immediate next actions)

In execution order:

### 4.1 PM drafts SC-3B Fix 6.1 BUILD execution dispatch (next action for new chat)

Filename: `Plans/DISPATCH-2026-05-09-sc-3b-fix-6-1-cerebro.md` (or 2026-05-10 if calendar advances).

Dispatch shape:
- Single commit: test-only change to stress test direction assignment.
- Pre-commit checklist: cargo k passes (18 warnings); diff confined to `kernel/src/test_harness.rs`; stash untouched.
- Verification: re-run BUILD dispatch §9 gate. Specifically Step 6 (200-run primary stress at NEO_SMP=4). Steps 1-5 should already pass (no kernel change). Step 7 (NEO_SMP=2 25-run smoke) opens after Step 6 closes.
- Stop conditions: any failure in primary stress (back to Phase B); any regression in existing scenarios (revert).
- Post-success: handoff back to PM for Sub-step 6 (docs + close).
- Backup after commit lands.

Estimate: Cerebro ~30-60 minutes for code change; ~30-60 minutes for 200-run gate at NEO_SMP=4 depending on host load. Total ~1-2 hours.

### 4.2 Cerebro executes Fix 6.1

Cerebro implements per dispatch. Re-runs gate. Reports outcome.

**Outcome A (gate passes 200/200):** Forward queue advances to §4.3.

**Outcome B (gate still fails):** PM produces Phase B instrumentation dispatch. Apply NMI watchdog stash@{0} for investigation. Add PROGRESS frames every 50 iterations. Run smaller loop (50 runs). NMI captures stack traces. PM analyzes; new hypothesis ranking. Return to fix-design.

### 4.3 Sub-step 6 dispatch (post-Fix 6.1 success)

PM produces `Plans/DISPATCH-2026-05-09-sc-3b-sub-step-6-cerebro.md` (or appropriate date) covering:
- ADR-013 §Q4 amendment (implementation 2026-05-09 sub-section: wake-on-completer-CPU mechanism, Thread.cpu_id, consumer-first wake from ADR-011 BUILD-1).
- ADR-013 §Q6 amendment (implementation 2026-05-09 sub-section: per-call-site ABI, SwitchMutex, Channel-first release, Thread.cpu_id, no fence, no RFLAGS save/restore).
- ADR-011 supersession note (single-CPU rendezvous invariants formally superseded by Q4 + Q6).
- PHASE-2.md update.
- Close report at `Plans/CLOSE-REPORT-2026-05-XX-sc-3b.md`. Include three-way investigation summary from §2.7 above and Fix 6.1 verification evidence.
- INDEX sweep.

Sub-step 6 may be 1-2 commits (Cerebro can split if helpful). Jarvis can take INDEX sweep if PL prefers parallel.

### 4.4 Cosmetic cleanup deferred

Cerebro's Sub-step 5 commit body has literal `\n` separators due to PowerShell quoting. Per "no amend without explicit request" standing rule, this was correctly left alone. Can be addressed later via a non-amend commit (e.g., an INDEX sweep or close-report commit can include a brief note in the body about the historical artifact). Not load-bearing.

### 4.5 INDEX hygiene

Multiple working-tree artifacts not yet in `Plans/INDEX.md`:
- PM-NOTE-2026-05-09-sc-3b-think-stage.md
- DISPATCH-2026-05-09-sc-3b-plan-stage-parallel.md
- SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md
- SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md
- SC-3B-PLAN-SYNTHESIS-2026-05-09.md
- DISPATCH-2026-05-09-sc-3b-build-cerebro.md
- SC-3B-VERIFICATION-2026-05-09.md
- PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md
- DISPATCH-2026-05-09-sc-3b-investigation-parallel.md
- SC-3B-INVESTIGATION-2026-05-09-cerebro.md
- SC-3B-INVESTIGATION-2026-05-09-prometheus.md
- SC-3B-INVESTIGATION-2026-05-09-claude.md
- SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md
- (this) HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md

Plus possible carryovers from earlier sessions (verify against current INDEX). Jarvis sweep candidate after Sub-step 6 lands. Or before, if PL wants ordering before close report.

### 4.6 After SC-3B closes (forward to BUILD-3 sub-commit 4)

Phase 2 BUILD-3 has Q7 (TLB management) and Q8 (capability table SMP composition) remaining. Phase 2 BUILD-4-or-later picks up ADR-016 A2 (scheduler heap-decoupling) as Bible 8.5.3 conformance. After Phase 2 closes, Phase 3 (NeoFS) and Phase 4 (network stack) before AI-native phases (5+).

---

## 5. Active artifacts (this session's outputs)

All in `E:\AI Automation\AltimitNeo\Plans\` unless noted otherwise. Working-tree only (no commits) except where annotated.

**THINK-stage and scoping:**
- `PM-NOTE-2026-05-09-sc-3b-think-stage.md` — three-lens THINK analysis. Ratified.

**PLAN-stage:**
- `DISPATCH-2026-05-09-sc-3b-plan-stage-parallel.md` — parallel PLAN dispatch.
- `SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` — Cerebro's PLAN.
- `SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md` — Prometheus's PLAN (revised post-synthesis).
- `SC-3B-PLAN-SYNTHESIS-2026-05-09.md` — PM synthesis of PLANs. Ratified.

**BUILD-stage:**
- `DISPATCH-2026-05-09-sc-3b-build-cerebro.md` — 6-sub-step BUILD playbook. Ratified. Sub-steps 1-5 executed; Sub-step 6 blocked.

**Verification + triage:**
- `SC-3B-VERIFICATION-2026-05-09.md` — Cerebro's gate run log including the run-036 failure.
- `PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md` — PM triage.

**Investigation:**
- `DISPATCH-2026-05-09-sc-3b-investigation-parallel.md` — three-way investigation dispatch.
- `SC-3B-INVESTIGATION-2026-05-09-cerebro.md` — Cerebro's per-agent investigation.
- `SC-3B-INVESTIGATION-2026-05-09-prometheus.md` — Prometheus's investigation.
- `SC-3B-INVESTIGATION-2026-05-09-claude.md` — Claude-3's investigation.
- `SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` — Cerebro's pre-synthesis, treated as PM synthesis-of-record per option (a).

**This handoff:**
- `HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md`

**Source artifacts referenced (read-only):**
- `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log` — failure evidence.
- `target/verify/sc3b-gate-06-stress-neo-smp4/run-001..035.serial.log` — clean runs preceding failure (not personally read by PM but available).

---

## 6. Git state at handoff

```
a61e318  test: SC-3B cross-CPU IPC stress + migration/reap scenarios (Sub-step 5)  ← HEAD
[Sub-steps 1-4 commits — exact SHAs in cargo log]
79b35f4  docs: Plans/INDEX.md — wedge-fix discharge + β outcome artifacts
1dbbfa7  docs: ADR-016 — Amendment 2026-05-08, A2 reframed to Phase 2
2fe5a70  docs: Plans/INDEX.md — sweep through 2026-05-08 governance commits
aac0c11  mm: ADR-016 IRQ-safe global heap allocator (A1 defense-in-depth)
8a2311d  docs: ADR-016 IRQ-safe heap allocator and scheduler heap-decoupling (ratified)
8ef8cac  docs: bible.md — Article 8.5 critical-section discipline (v1.0.5)
4c6abf4  sched: prevent heap-lock re-entry in reap zombie drain
3b3b4db  xtask: optional QEMU monitor for investigation use
6edb012  xtask: optional QEMU GDB stub for investigation use
```

Cerebro's Sub-step 5 commit body has literal `\n` due to PowerShell quoting (cosmetic only; not amended per standing rule). Sub-steps 1-4 commits between 79b35f4 and a61e318; exact SHAs visible via `git log --oneline`.

`git status` should be clean (no uncommitted changes in tracked files). Working-tree Plans/ artifacts from this session are present but not git-tracked (only `Plans/INDEX.md` and `Plans/backup-to-synology.ps1` are tracked in Plans/).

**Backup at handoff:** `Z:\AltimitNeo\2026-05-09-144314` with three preserved stashes intact. Captures HEAD `a61e318`. Restore-tested OK.

---

## 7. Stash state

Three preserved stashes (unchanged across all commits this session):

```
stash@{0}: On main: wedge-investigation-8-nmi-watchdog
stash@{1}: On main: wedge-investigation-2-instrumentation
stash@{2}: On main: h1-experiment-instrumentation
```

`stash@{0}` is the candidate Phase B instrumentation if Fix 6.1 doesn't close the gate. It was developed during the wedge investigation. Recommended NMI threshold for SC-3B Phase B usage: 5 seconds (per Claude-3 §3.1) or 10 seconds (per Cerebro §3.7) — PM defers to Cerebro on the choice given Cerebro's deeper understanding of typical iteration timing. Apply with `git stash apply stash@{0}` for investigation only; do not commit.

`stash@{1}` and `stash@{2}` are wedge-era; not currently candidates for SC-3B but preserved for historical reference.

---

## 8. Conventions reaffirmed

- **Bible 1.3:** every commit is intended to ship. No scaffolding-only commits.
- **Bible 4.3:** every external dependency is a liability. Kernel-owned types preferred where stability matters (SwitchMutex precedent following A1 IRQ-safe heap precedent).
- **Bible 8.5.4:** no blocking while holding a spinlock. Q6's asm-release-on-new-stack is structural conformance.
- **Bible 14:** OBSERVE → THINK → PLAN → BUILD/VERIFY cadence honored explicitly. /simplify gate scrutinizes scope before BUILD.
- **No amend without explicit request:** Cerebro's literal `\n` artifact is correctly left alone.
- **No stash interactions during regular work:** stashes are reserved for explicit investigation phases.
- **Backups after every commit:** `Plans\backup-to-synology.ps1` handles the preservation, including the three stashes (2026-05-08-refined mechanism handles 3+ stashes correctly).
- **Working-tree-only artifacts in Plans/:** only `INDEX.md` and the backup script are git-tracked. Everything else is captured by Synology backups.
- **Multi-recipient dispatches use `-parallel.md` suffix:** convention established this session for the PLAN-stage and investigation-stage parallel dispatches.
- **Per-agent output filenames** for parallel work: `{base}-{agent}.md` pattern (e.g., `SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md`).
- **Independence in parallel work:** agents do not coordinate or read each other's outputs before submitting. PM synthesizes after all return.
- **PASS frame fields enumerated explicitly** when introducing new scenarios. SC-3B introduced: blocked_cpu, completer_cpu, resumed_cpu, iterations, send_first, recv_first, grant_rights, exit_reaped, release_path.
- **Stop conditions honored:** when a stop condition fires, halt and triage, don't push through. Wedge-investigation discipline.
- **Statistical framing for closure gates:** 200/200 with zero historical-signature failures is the standard, established by L's wedge-fix discharge. P[200/200|rate>0.5%]≈0.45%.

---

## 9. PM tone and operational notes

- **Voice:** tight, source-grounded, calibrated about uncertainty. Honest about Prometheus calibration without being pejorative. Defers to PL on ratifications.
- **Length:** dispatches and synthesis docs are substantial (300-900 lines). PM-NOTEs and triage docs are tight (200-400 lines). Brief responses (30-80 lines) for status updates and forward-queue confirmations.
- **No emoji.** Markdown headings and tables for structure.
- **Cross-references at end of every artifact.** File paths are absolute or project-rooted.
- **Calibration data is honest.** Prometheus's H7 calibration is surfaced honestly in synthesis but with the qualifier that it's not pejorative.
- **Hopes and concerns separated** in triage documents (PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md §7). Communicates uncertainty to PL.
- **Decisions surface explicitly to PL.** PM proposes; PL ratifies. Multi-option decision surfacing (a/b/c options) is the default for non-trivial choices.

---

## 10. Open items / future-phase forward state

- **SC-3B Fix 6.1 dispatch + execution + gate** — IMMEDIATE NEXT.
- **SC-3B Sub-step 6** (docs + close) — after Fix 6.1 closes the gate.
- **BUILD-3 sub-commit 4** (Q7.b/c — CR3 cache + IPI TLB shootdowns) — after SC-3B closes.
- **BUILD-3 sub-commit 5** (Q8.a/b — capability table SMP composition) — after Q7.
- **BUILD-3 sub-commit 6** (Q8.c — blocked-thread-kill walk) — after Q8.a/b.
- **BUILD-4 or BUILD-5 sub-commit** (ADR-016 A2 — scheduler heap-decoupling) — Bible 8.5.3 conformance milestone. Not gated by SMP retrofit.
- **Phase 2 close** — pending all BUILD-N sub-commits.
- **Phase 3** — NeoFS.
- **Phase 4** — network stack.
- **Phases 5-7** — AI-native phases (tensor primitive, accelerators, inference scheduler) per ANSPEC-001.

**Forward concern flagged by Claude-3 investigation:** the deterministic Q4 migration accumulation pattern observed in the stress test could plausibly affect production AI workloads under ANSPEC-001 §3.7 / §10.2 (capability-passing IPC), if real workloads exhibit similar long-running same-pair rendezvous patterns. Worth monitoring during Phase 5 design but not actionable in Phase 2. Note for ANSPEC-001 cross-reference if PL wants.

---

## 11. Cross-references

**This handoff:**
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md`

**Substrate:**
- `docs/bible.md` v1.0.5
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6
- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` (ratified + amended)
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001)
- `docs/phases/PHASE-2.md`

**This session's primary outputs (active normative):**
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md`
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md`
- `Plans/SC-3B-VERIFICATION-2026-05-09.md`
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md`
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` (Cerebro pre-synthesis = PM synthesis-of-record per option a)

**Per-agent investigation reports:**
- `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md`
- `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md`
- `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md`

**Per-agent PLAN reports:**
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md`
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md`

**Inherited context:**
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — pre-thinking note with Invariants I1–I6.
- `Plans/PM-NOTE-2026-05-08-beta-outcome.md` — wedge-fix β outcome.
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — wedge-fix discharge.

**Key source files (read by multiple investigators):**
- `kernel/src/arch/x86_64/context.rs` — context_switch_with_release2 asm
- `kernel/src/sched/scheduler.rs` — block_current_into_with_guard, wake helpers
- `kernel/src/ipc/mod.rs` — 4 IPC retrofit sites
- `kernel/src/sync/switch_mutex.rs` — SwitchMutex
- `kernel/src/test_harness.rs` — stress test scenario (target of Fix 6.1)

---

## 12. Opening message for the new chat

Suggested PL opening to the new chat session:

> "Continuing AltimitNeo project. Read Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md for full context. Next action: draft SC-3B Fix 6.1 BUILD execution dispatch for Cerebro per §4.1 of the handoff. Fix 6.1 is the per-pair direction alternation in the stress test (~5-10 line test-only change in kernel/src/test_harness.rs). After your draft is on disk, I'll review, ratify, and forward to Cerebro."

The new PM session reads the handoff, picks up at §4.1, drafts the dispatch.

---

**End of handoff.**

*Session ends with SC-3B Sub-steps 1-5 implemented (HEAD a61e318), stress closure gate blocked at 36/200, three-way investigation complete, Hn identified at 50–55% confidence with two independent corroborations, Fix 6.1 ratified, synthesis bookkeeping resolved (option a), forward queue clear. New chat picks up by drafting Fix 6.1 BUILD execution dispatch.*
