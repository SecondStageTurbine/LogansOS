# Dispatch: SC-3B Sub-step 5c — IRQ-safe lock discipline on Channel.state callers

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-10
**Type:** BUILD dispatch. Lands a kernel fix commit on top of `2d8f65d`. Single-recipient. Closes SC-3B if the gate passes.
**Authority:** `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` §3 (mechanism captured: preempted same-CPU lock owner + irq-off waiter), §6 (recommendation specifies irq-safe ownership discipline as the fix). `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` §2.5 (cross-kernel context for Bible-8.5.4-equivalent enforcement; convention-only discipline is documented-fragile across kernels surveyed). Phase D ruled Hd / Hr / Hc out cleanly; Cerebro's source-path inference (debug poll path → preempted holder) is concrete and well-supported by the captured trace.
**Substrate:** Project HEAD `2d8f65da77b4ea10f3d53226724e9381961c48c0` (Sub-step 5b, IPI infrastructure landed). Tracked working tree clean per Cerebro's Phase D exit. Stash list `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment. Backup `Z:\AltimitNeo\2026-05-10-111123` captures clean post-Phase-D state.

---

## 0. Standing instructions

1. **Commit lands.** Sub-step 5c is a BUILD commit on top of `2d8f65d`. Subject: `build-3 sub-step 5c: enforce irq-safe acquisition for Channel.state`. Body cites Phase D findings.
2. **Stashes untouched.** `{0}`, `{1}`, `{2}`, `{3}` not applied, not popped. Any state change to the stash list is a halt-and-ping condition.
3. **Pre-commit verification required** (§4) — full sweep including the gate scenarios at small N.
4. **Backup runs after commit, before gate kickoff.** The 2026-05-08-refined script handles four stashes.
5. **Stop conditions are honored** (§7).
6. **PM is on call.**

### 0.1 Honest framing

The bug class is **Hpw — preempt-while-holding + irq-off same-CPU waiter**. The minimum fix is small (wrap a known set of `Channel.state.lock()` callers under `without_interrupts(...)` so they match the IPC paths' acquisition discipline). The audit is the load-bearing piece — every `Channel.state.lock()` call site needs to be classified as either (a) genuinely irq-off (correct), (b) preemptible-but-must-not-be (fix), or (c) preemptible-by-design and provably safe (rare, requires justification).

The expected outcome of this dispatch is gate PASS at 200/200 NEO_SMP=4 + 25/25 NEO_SMP=2. Confidence is high — the captured signature pinpoints the mechanism, and the fix removes the necessary condition for that mechanism (mixed acquisition discipline on the same lock).

The structural follow-up — making `SwitchMutex` IRQ-off by construction, or introducing a type-level distinction between irq-safe and preemptible locks — is **out of scope for this dispatch**. It is a Bible-amendment + ADR concern that PM will queue post-SC-3B closure. Sub-step 5c is the minimum fix that closes the gate; the structural fix is its own dispatch chain.

If the gate fails, the dispatch's commit stays landed (Bible 1.3) and a follow-up investigation dispatches.

---

## 1. Goal

Land a kernel commit that enforces irq-safe acquisition discipline on every `Channel.state.lock()` call site, matching the discipline of the IPC fastpath that already acquires under `without_interrupts(...)`. Verify via the SC-3B BUILD §9 gate (200 boots NEO_SMP=4 + 25 boots NEO_SMP=2 of `smp_cross_cpu_rendezvous_stress`).

---

## 2. Design specifications

### 2.1 Audit scope

Every call site that acquires `Channel.state` (whether via `state.lock()`, `state.lock_at(...)`, or any equivalent) must be classified. Cerebro performs the audit.

Phase D specifically named these as plain-`self.state.lock()` callers (out of without_interrupts):

- `fail_recv_grant`
- `debug_endpoint_is_grant_blocked`
- `debug_endpoint_is_send_blocked`
- `debug_endpoint_is_recv_blocked`
- `close_peer`

Plus the orchestration path through `wait_for_stress_endpoint_blocked` that exposed the bug.

The audit must also surface any other callers that don't appear in this list. A `git grep` on `state.lock` and `state.lock_at` inside `kernel/src/ipc/` and any test-harness files that touch channel state is the floor; any caller found is classified.

### 2.2 Classification and fix

For each `Channel.state` caller:

- **(a) Already under `without_interrupts(...)` or in an irq-off context (CLI'd, NMI handler, etc.) — no change.** Cite the surrounding context inline if non-obvious.
- **(b) Preemptible at acquisition — fix.** Wrap the lock acquisition (and any associated state read/write that depends on the lock being held) in `without_interrupts(...)`. Add a comment: `// Channel.state must be acquired under without_interrupts to match IPC fastpath discipline (Phase D, SC-3B).`
- **(c) Preemptible-by-design and provably safe — justify.** This category should be empty after the audit. If Cerebro identifies any caller that genuinely should remain preemptible, halt and ping PM with the rationale. PM expects to ratify zero category (c) callers in this dispatch.

The fix shape per Cerebro's recommendation: "Convert the channel debug helpers used by SMP tests to acquire `Channel.state` under `without_interrupts`, or remove lock-taking polling from the test orchestration path." PM lean: prefer wrapping the helpers themselves (so all callers benefit) unless there's a reason a helper is also called from a context where IF must remain enabled. Cerebro decides placement.

### 2.3 Fix shape: comment discipline

Every site that acquires `Channel.state` after this dispatch must carry a brief inline comment naming the discipline, matching the format:

```rust
// Channel.state requires irq-off acquisition (matches IPC fastpath; SC-3B Phase D).
without_interrupts(|| {
    let guard = self.state.lock_at(/* site label */);
    // ... critical section ...
})
```

The comment makes the discipline visible at the call site, so future maintainers don't accidentally re-introduce the Hpw bug class. This is a convention-level guardrail, not a type-level one.

### 2.4 Site labels for `lock_at(...)`

Where Cerebro is touching a `state.lock()` call to wrap it, also convert it to `state.lock_at(SITE_LABEL)` if `lock_at` exists in the SwitchMutex API and accepts a `&'static str` site identifier (Phase D added this for instrumentation; it may be production-stable). Site labels improve future debug capture. If `lock_at` is investigation-only and not merged, skip this part.

This is a stretch goal. If it would expand the diff materially (>50 LOC), defer to a follow-up. The load-bearing fix is the `without_interrupts` wrap.

### 2.5 What does NOT change in this dispatch

- **No `SwitchMutex` API change.** The lock primitive itself is unchanged. Only its caller discipline is corrected.
- **No `Channel` data structure change.** No new fields, no method signatures broken.
- **No IPC fastpath change.** The IPC paths already acquire correctly.
- **No scheduler change.**
- **No Bible amendment in this dispatch.** Bible 8.5.4 amendment proposal is queued as ADR carry-forward.
- **No structural ADR in this dispatch.** "Make SwitchMutex IRQ-off by construction" is a separate dispatch.
- **No test-harness changes** beyond what's necessary to wrap the orchestration's progress poll. The stress test logic itself is correct.
- **No instrumentation retained.** Phase D's owner metadata, BLOCK_TRACE frames, multi-probe NMI extensions, etc., are all investigation-only. They do not land in this commit.

### 2.6 Diff size expectation

PM expects this dispatch's diff to be <100 LOC net, plausibly <50. Each call site is a `without_interrupts(|| { ... })` wrap plus a comment. The audit may surface 5–10 sites; each is a small change. If the diff exceeds ~150 LOC net, halt and ping — that suggests scope expansion that needs PM ratification.

---

## 3. Pre-commit checklist

- [ ] `git grep` audit complete; every `Channel.state` caller classified (§2.1).
- [ ] Category (b) callers wrapped in `without_interrupts(...)` with comment.
- [ ] Category (c) callers either empty or PM-ratified.
- [ ] Inline comments at every `Channel.state` acquisition site naming the discipline.
- [ ] No instrumentation residue from Phase D (no owner metadata, no BLOCK_TRACE, no NMI patch).
- [ ] No `unsafe` blocks added (none should be needed; `without_interrupts` is safe Rust).
- [ ] Any `unsafe` blocks affected by the change still carry SAFETY comments.

---

## 4. Pre-commit verification

Before commit:

- [ ] `docker compose run --rm neo cargo k`: PASS at 18-warning baseline (delta 0; this dispatch should not introduce warnings).
- [ ] `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all`: PASS. Default suite passes.
- [ ] All five sibling SMP scenarios pass single-run sanity at NEO_SMP=4:
  - `smp_block_release_smoke`
  - `smp_send_blocks_first_then_completes`
  - `smp_recv_blocks_first_then_completes`
  - `smp_grant_cross_cpu`
  - `smp_cross_cpu_migration_reap`
- [ ] `smp_cross_cpu_rendezvous_stress` single-run at NEO_SMP=4: PASS.
- [ ] `smp_cross_cpu_rendezvous_stress` single-run at NEO_SMP=2: PASS.
- [ ] Smoke gate: 5 consecutive runs of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4 PASS. (This is a quick confidence check before committing to the full 200-run gate. If any of the 5 fail, halt and ping — the fix isn't sufficient and we need to re-investigate before kickoff.)

If pre-commit verification passes:

- [ ] Backup script runs and reports OK.
- [ ] Commit lands with subject `build-3 sub-step 5c: enforce irq-safe acquisition for Channel.state`. Body cites Phase D findings, the bug class (Hpw — preempt-while-holding + irq-off same-CPU waiter), and the audit's classification summary.

---

## 5. The gate

After commit:

### 5.1 Step 6 — 200 boots at NEO_SMP=4

Same shape as Sub-step 5b's Step 6:

```powershell
$ErrorActionPreference = 'Stop'
$artifact = 'target/verify/sc3b-sub-step-5c-gate/neo-smp4'
New-Item -ItemType Directory -Path $artifact -Force | Out-Null
# 200-run loop with first-failure stop
```

Stop on first classifier-signature failure. Save serial + xtask logs per run.

### 5.2 Step 7 — 25 boots at NEO_SMP=2

Same shape, NEO_SMP=2, 25 runs. Only run after Step 6 completes with 200/200 PASS.

```powershell
$artifact = 'target/verify/sc3b-sub-step-5c-gate/neo-smp2'
New-Item -ItemType Directory -Path $artifact -Force | Out-Null
# 25-run loop
```

### 5.3 Verification log

Write `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` mirroring the Sub-step 5b verification log structure: substrate, implementation summary, audit classification table, pre-commit verification, gate result, exit state.

---

## 6. Outcomes and forward path

### 6.1 Outcome 5C-A — both gates PASS (expected)

Step 6: 200/200 PASS. Step 7: 25/25 PASS. **SC-3B closes.**

PM produces Sub-step 6 docs dispatch for Prometheus:
- ADR-013 §Q4/§Q6/§Q7 amendments (note §Q7 IPI infrastructure landed in Sub-step 5b; sub-commit 4 will use that infrastructure for TLB shootdowns; Hpw fix landed in Sub-step 5c).
- ADR-011 supersession (wake-on-completer-CPU pattern was correct; the residual gate failures were Hpw-class, not Q4-class).
- New ADR proposal (let's call it ADR-024 placeholder) for Bible 8.5.4 amendment + SwitchMutex IRQ-off-by-construction structural concern.
- PHASE-2.md update.
- Close report at `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` framed per `PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §5 + outcome-b-synthesis §1, with Phase D as the load-bearing diagnostic event in the narrative.
- Statistical context: ~325/325 clean post-Sub-step-5c (Sub-step 5b's 51 PASS pre-failure + Phase D's 42 PASS pre-NMI-capture + Sub-step 5c's 200+25 = empirical clean band substantially above the original ~2.8% rate).
- INDEX.md sweep.

### 6.2 Outcome 5C-B — Step 6 fails

The commit stays landed (Bible 1.3). Hpw fix didn't fully close the gate, meaning either:

- Another `Channel.state` caller was missed in the audit. PM dispatches a tighter audit + re-fix.
- A second mechanism class was masked by Hpw and is now visible. PM dispatches a Phase E investigation (likely NMI-armed loop on the post-5c substrate).

Hypothesis ranking on this branch: Hpw was real (Phase D evidence is solid) but not the only mechanism. The Phase D capture's *signature* matches all prior failures' MISSING_TERMINAL_FRAME classifier exactly, so Hpw is the most parsimonious explanation for *all* observed failures — but if 5c's gate fails, that parsimony breaks and we re-investigate.

### 6.3 Outcome 5C-C — Step 6 passes 200/200, Step 7 fails

Different mechanism at NEO_SMP=2. Possible: Hpw is closed; a topology-specific mechanism remains visible only at smaller N. PM dispatches Phase E targeted at NEO_SMP=2 (likely NMI-armed loop with the same Phase D instrumentation pattern).

### 6.4 Outcome 5C-D — pre-commit verification fails

If the smoke gate (§4 last bullet) fails before commit, the fix isn't sufficient. Halt before commit. PM ingests the failure mode and re-designs.

This outcome has low prior probability given Phase D's clean capture, but it is possible if:
- The audit missed a caller.
- The bug has a secondary trigger that Hpw wrapping doesn't address.
- Instrumentation residue from Phase D leaked into the substrate (shouldn't happen given Cerebro's clean exit at HEAD `2d8f65d`).

---

## 7. Stop conditions

Halt and ping PM if:

- The audit surfaces any category (c) caller (preemptible-by-design that should remain so). PM ratifies before proceeding.
- Diff exceeds ~150 LOC net.
- Cargo k warning delta exceeds +0 (this dispatch should not introduce warnings).
- Pre-commit smoke gate (5 consecutive PASS at NEO_SMP=4) fails. The fix isn't sufficient.
- Sibling SMP scenario regresses post-fix.
- Backup script reports failure for two consecutive attempts.
- Step 6 hits a *different* failure classifier than MISSING_TERMINAL_FRAME (e.g., a kernel panic, FAIL frame from a regression). The commit stays landed; PM diagnoses.
- Step 7 fails at smaller N — see Outcome 5C-C.
- Tracked tree exits non-clean post-commit (commit didn't land cleanly).

---

## 8. Estimated wall time

- Audit + classification: ~30–45 minutes.
- Fix implementation (wraps + comments, ~5–10 sites): ~30–45 minutes.
- Pre-commit verification (cargo k + sibling sanity + 5-run smoke): ~30–45 minutes.
- Commit + backup: ~10 minutes.
- Step 6 (200 boots NEO_SMP=4): ~30–120 minutes depending on per-run timing (Phase D averaged 128s; Phase B/C/Sub-step-5b averaged 13–67s per run; if Sub-step 5c reverts to the lower band that's ~45 minutes for 200 runs; if it tracks Phase D's 128s avg that's ~7 hours — instrumentation overhead from Phase D is gone in 5c, so the lower band is more likely).
- Step 7 (25 boots NEO_SMP=2): ~10–30 minutes.
- Verification log + close artifacts: ~30 minutes.

Total: ~3–6 hours from acceptance to verification log delivery if Outcome 5C-A.

---

## 9. Cross-references

**Authority:**
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` — bug class identified (Hpw); recommendation specifies the fix scope.
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` §2.5, §2.6 — cross-kernel context for Bible-8.5.4-equivalent enforcement.
- `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3 — hypothesis ranking framework, retroactively confirms why earlier passes missed Hpw (channel state was unreadable until Phase D's lock-owner probe).

**Substrate:**
- HEAD `2d8f65da77b4ea10f3d53226724e9381961c48c0` — Sub-step 5b, IPI infrastructure landed. Sub-step 5c commits on top.
- Backup `Z:\AltimitNeo\2026-05-10-111123` — clean post-Phase-D exit.

**Investigation chain (Sub-step 5c is the closure of the SC-3B arc):**
- Phase A — three-way investigation: `Plans/SC-3B-INVESTIGATION-{cerebro,prometheus,claude}-2026-05-09.md` + synthesis.
- Phase B — NMI watchdog: `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md`.
- Outcome B / NMI re-arm: `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`.
- Phase C — NMI + instrumentation: `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-10.md`.
- Phase D — lock ownership probe: `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md`. **Resolving capture.**
- Sub-step 5b — IPI infrastructure: `Plans/SC-3B-SUB-STEP-5B-VERIFICATION-2026-05-09.md`.
- Sub-step 5c — this dispatch.

**Source files in scope (audit candidates):**
- `kernel/src/ipc/channel.rs` (or whichever file defines `Channel` and its `state` field, plus `fail_recv_grant`, `close_peer`, the four `debug_endpoint_is_*_blocked` helpers).
- `kernel/src/test_harness.rs` (for `wait_for_stress_endpoint_blocked` and any orchestration that polls channel state).
- Any other `kernel/src/ipc/**` files that touch `Channel.state`.

**Bible anchors:**
- Article 1.3 — every commit is intended to ship. Sub-step 5c lands a real fix.
- Article 8.5.4 — no blocking while holding a spinlock. Phase D shows this also covers preemption-while-holding (the preempted holder is effectively blocked from the waiter's perspective). Bible amendment proposal carried forward (ADR placeholder).
- Article 14 — phase cadence; Sub-step 5c is the BUILD step closing the SC-3B sub-commit's gate.
- Article 15.2 — SAFETY comments. The fix uses `without_interrupts` (safe Rust); no new SAFETY-comment territory.

**ADR carry-forwards (NOT in this dispatch's scope):**
- ADR proposal (placeholder ID): Bible 8.5.4 amendment to clarify "no blocking" includes "no preemption" while holding a spinlock-class lock.
- ADR proposal (placeholder ID): SwitchMutex IRQ-off by construction (or type-level distinction between irq-safe and preemptible locks). Janus second-pass §2.5 surveys cross-kernel options. This is structural and probably Phase 2 territory.
- ADR proposal (placeholder ID): owner metadata as a production-stable diagnostic affordance (Phase D's instrumentation was investigation-only; whether to retain it as production diagnostic capability is its own decision).

These are queued for Sub-step 6 docs work or ADR drafts. They do NOT modify Sub-step 5c.

**Forward concern:**
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §10.2 — every `Channel.state` caller in the AI subsystem (capability handle ops, model-handle IPC, inference-context IPC) inherits this discipline. Sub-step 6 docs work should note this in the forward AI-subsystem design.

**Routing:**
- Sub-step 5c → Cerebro (this dispatch). Kernel BUILD work.
- Sub-step 6 → Prometheus (post-5C-A). Documentation: ADR amendments, PHASE-2.md update, close report, INDEX sweep, ADR proposals for the carry-forward items above.

---

**End of dispatch.**

*Sub-step 5c: audit every `Channel.state` caller, wrap category (b) callers in `without_interrupts(...)` with discipline-naming comment, commit on top of `2d8f65d`. Pre-commit gates include 5-run smoke. Gate is BUILD §9 Step 6 (200/200 NEO_SMP=4) + Step 7 (25/25 NEO_SMP=2). Outcome 5C-A → SC-3B closes; Sub-step 6 docs dispatch to Prometheus. Outcome 5C-B → Phase E investigation. Outcome 5C-C → topology-specific Phase E. Outcome 5C-D → pre-commit halt. Estimated 3–6 hours. Bible 8.5.4 amendment + SwitchMutex structural ADRs carried forward — out of scope here. PM is on call.*
