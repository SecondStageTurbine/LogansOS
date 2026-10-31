# Dispatch: SC-3B Sub-step 5b — IPI infrastructure + place_thread_on_cpu wake notification

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-09
**Type:** BUILD-stage execution dispatch with embedded PLAN. Per Bible Article 14, this lands inside the SC-3B sub-commit boundary (BUILD dispatch §8: "Multi-commit within one BUILD-3 sub-commit boundary") between Sub-step 5 (`a61e318`) and the still-blocked Sub-step 6 docs. Single-recipient. Mechanism-clear (Linux/Asterinas prior art via Janus); PLAN-stage parallel analysis is overkill for this scope. Ratified by PM under the four-anchor delegation.
**Authority:** `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3 (Hp at 75% post-NMI+Janus); `Plans/SC-3B-SOURCE-AUDIT-2026-05-09.md` §Hp confirmation status (no IPI infrastructure at all + no APIC on APs + no per-CPU timer).
**Substrate:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`. Tracked working tree clean. Stash list `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment. None of the stashes is applied during this dispatch.

---

## 0. Standing instructions

1. **One commit, lands on top of `a61e318` within the SC-3B sub-commit boundary.** Sub-step 5b designation. Sub-step 6 (docs + close) follows after the verification gate passes.
2. **Build verification before commit.** `cargo k` passes at the existing 18-warning baseline plus any warnings the new code adds. Document new warnings in the commit body if any.
3. **No stash interactions.** All four stashes remain untouched.
4. **Backup runs after commit lands.** The 2026-05-08-refined backup script handles four stashes per Phase B's verification.
5. **Stop conditions are honored** (§7).
6. **PM is on call** for any design ambiguity.

### 0.1 Honest framing on confidence

This dispatch is ship-quality regardless of whether it closes the gate. The IPI infrastructure is load-bearing for SMP correctness:

- Janus prior art (`Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` §1.4): Linux's design explicitly separates "queue work on remote CPU" and "tell remote CPU to look at its queue" via `resched_curr()` / `smp_send_reschedule()`. AltimitNeo lacks the second half.
- ADR-013 §Q7 plans IPI infrastructure for BUILD-3 sub-commit 4 TLB shootdowns. This dispatch lands the foundation early; sub-commit 4 uses it for shootdowns.
- Asterinas PR #2488 documented the analogous bug class as a major Rust-kernel SMP issue.

If the gate passes after this fix, SC-3B closes via Sub-step 6. The close report credits the IPI fix as causal even if the exact causation chain is partial.

If the gate fails, the IPI infrastructure remains landed (it's standalone-correct work) and Phase C instrumentation opens (PROGRESS frames + cpu_id tracking + channel state dump in NMI path).

Either way, this commit ships ship-quality architectural correction. Bible 1.3 satisfied.

---

## 1. Goal

Add a minimal cross-CPU reschedule-IPI mechanism. Wire it into `place_thread_on_cpu` to notify the target CPU after enqueue. Re-run the SC-3B closure gate (BUILD §9 Step 6 + Step 7).

Specifically:

1. APIC initialization on APs (in addition to existing BSP PIC for IRQ0 timer).
2. Reschedule-IPI vector + handler.
3. `send_reschedule_ipi(target_cpu: u32)` function.
4. `place_thread_on_cpu` modified to call `send_reschedule_ipi(target_cpu)` after enqueue.
5. Audit `wake_thread_on_current_cpu` and siblings — they're current-CPU by name so should not need IPI, but verify.

---

## 2. Design specifications (PM-decided, not negotiable absent ADR)

These are the ratified design positions for this fix. They've been considered against Janus prior art and project posture; Cerebro implements per these. If any specification appears unimplementable for a substrate reason, halt and ping PM.

### 2.1 APIC enablement on APs

Currently APs initialize traps (`init_traps_ap`) but no APIC. Add APIC initialization:

- **xAPIC mode** (not x2APIC). xAPIC is sufficient for current AltimitNeo (≤8 CPUs typical); x2APIC is a future expansion. Bible 4.3: minimal added surface.
- **Initialization point:** in `init_traps_ap` or a new `init_apic_ap` called from `ap_entry` after `init_traps_ap`. PM defers to Cerebro's judgment on placement.
- **What to enable:** APIC base MSR (`IA32_APIC_BASE`, MSR 0x1b) — verify enable bit, set xAPIC mode if not already; APIC SVR (Spurious Vector Register) at xAPIC offset 0xf0 — set enable bit (bit 8) and a spurious-IRQ vector (e.g., 0xff). EOI register is at offset 0xb0.
- **What to NOT enable:** APIC timer. Per ADR-013 §Q4's design intent, AltimitNeo Phase 2 stays cooperative-scheduled on APs; no LAPIC timer in this dispatch. Future work (Phase 3+ or as part of Q7).
- **What to NOT touch:** BSP's PIC IRQ0 for the existing timer. PIC and APIC coexist; PIC is masked except IRQ0. APIC handles IPIs.

### 2.2 Reschedule-IPI vector

- **Vector number:** `0xfd` (253). Below Linux's `RESCHEDULE_VECTOR` of `0xfd` for symmetry, but verify it's not already used by AltimitNeo IDT. The audit confirms only vector 32 (PIC IRQ0) is currently registered; 0xfd is free.
- **IDT entry:** install in shared IDT (loaded by both BSP and APs).
- **Handler shape:** minimal C-ABI handler. The handler does:
  1. APIC EOI (write 0 to APIC offset 0xb0).
  2. Return.
- **No scheduler call from the handler.** The handler does NOT call `yield_now`, does NOT context_switch, does NOT touch ready queues. Yielding from interrupt context is a known footgun and not needed for this design — the IPI's job is to break the target CPU out of any PAUSE/spin so its next instruction is normal kernel code, which will reach the next yield_now in due course.
- **Naming:** `reschedule_ipi_handler` or similar. PM defers naming to Cerebro.

### 2.3 `send_reschedule_ipi` function

- **Signature:** `pub fn send_reschedule_ipi(target_cpu: u32)`. Returns `()`. No error result — APIC ICR writes are infallible from the issuer's perspective.
- **Location:** `kernel/src/arch/x86_64/apic.rs` (new file) or extend an existing arch module. PM defers to Cerebro.
- **Implementation:**
  1. Look up target CPU's LAPIC ID (already known from Limine MP info; per-CPU stored via init_ap or accessible via `PER_CPU[target_cpu].lapic_id`).
  2. Compose the ICR (Interrupt Command Register) value: vector 0xfd, delivery mode Fixed (000), destination mode Physical (0), level Assert, trigger mode Edge, destination shorthand "No Shorthand" (00).
  3. Write ICR-high (offset 0x310 in xAPIC) with destination LAPIC ID.
  4. Write ICR-low (offset 0x300) with the composed value. The ICR-low write triggers the IPI.
  5. Optionally poll ICR-low's Delivery Status bit (bit 12) until clear, indicating the IPI was delivered. PM recommends NOT polling — under cooperative scheduling, the issuer can proceed; if the IPI is somehow lost (extremely rare), the worst case is a missed wakeup that the target CPU's next yield catches.
- **Concurrency:** ICR writes are per-LAPIC. Multiple CPUs can issue IPIs concurrently to different targets without contention. Concurrent writes to the SAME LAPIC's ICR (issuer concurrency on a single CPU) shouldn't happen in our design — `send_reschedule_ipi` is called from `without_interrupts` contexts, so the issuer CPU isn't preempted mid-write.

### 2.4 `place_thread_on_cpu` modification

After the existing `s.ready.push_back(thread)` and before returning `Ok(())`:

```rust
// SAFETY: target_cpu was validated above; PER_CPU[target_cpu].lapic_id was
// installed at AP bringup. The IPI write itself is infallible; if the
// target CPU has interrupts disabled, the IPI is queued by the local APIC
// and delivered when interrupts re-enable.
crate::arch::x86_64::apic::send_reschedule_ipi(target_cpu);
```

Place the call OUTSIDE the `s = sched.inner.lock()` critical section if possible — the IPI doesn't need the target's scheduler lock. But ensure the IPI is sent AFTER the push_back (memory ordering: x86 TSO ensures the push is visible to the target before the IPI is observed). The natural place is after `mut s = ...` is dropped (end of without_interrupts closure or just after the lock guard's natural drop).

Actual structure:

```rust
pub fn place_thread_on_cpu(thread: Box<Thread>, target_cpu: u32) -> Result<(), &'static str> {
    without_interrupts(|| {
        // ... existing validation + lock + push_back ...
        // (lock guard drops at end of scope)
    });
    // After the lock is released, send the IPI.
    crate::arch::x86_64::apic::send_reschedule_ipi(target_cpu);
    Ok(())
}
```

Or equivalently, send the IPI inside the without_interrupts closure but after the lock guard drops. Either is correct. PM defers ordering to Cerebro.

### 2.5 No modification to `wake_thread_on_current_cpu`, `wake_thread`, `wake_thread_front`

These are current-CPU by design and don't need IPI. The audit confirms they only push to `current_scheduler()`. No change.

If Cerebro discovers any cross-CPU enqueue path PM didn't anticipate (e.g., a wake helper that under specific conditions enqueues to a non-current CPU), audit and report. Add `send_reschedule_ipi` there too if the path exists.

### 2.6 What does NOT change in this dispatch

- No PIC modifications. The existing PIC IRQ0 timer on BSP stays.
- No APIC timer setup. Future work.
- No second-pass instrumentation. Phase C remains as fallback if the gate fails.
- No test scenario changes. Stress test is unmodified. Fix 6.1 stash remains untouched (its disposition stays deferred).
- No modification to Q4/Q6/IPC retrofit code from Sub-steps 1-5. Those are mechanically correct per the NMI snapshot.

---

## 3. Implementation guidance (Cerebro judgment within these guardrails)

1. **APIC base address.** xAPIC is memory-mapped at the APIC base (typically `0xfee00000` physical, mapped via HHDM). Use `MSR_IA32_APIC_BASE` (`0x1b`) to read the actual base; APIC base is per-CPU but typically the same across CPUs in modern x86. Map the base via the kernel's existing physical-to-virtual mechanism (HHDM or pmap as appropriate).

2. **PER_CPU.lapic_id.** Already populated at AP bringup per the audit (Limine MP info supplies `lapic_id`; `init_ap` stores it). If the field doesn't exist or isn't accessible from `send_reschedule_ipi`'s scope, surface and PM clarifies.

3. **Spurious-IRQ vector.** Convention: `0xff`. Install a minimal handler that just EOIs and returns.

4. **EOI mechanism.** Write 0 to APIC offset `0xb0`. Both the reschedule-IPI handler and the spurious-IRQ handler need to EOI.

5. **Existing `without_interrupts` semantics.** The audit confirms `without_interrupts` saves/restores IF correctly. APs run with IF=0 during ap-N-main; placed threads run with IF=1 between yields. The IPI is queued by LAPIC when IF=0 and delivered when IF=1 — this is correct LAPIC behavior. So when ap-N-main is running, the IPI queues; when ap-N-main yields and a placed thread becomes current (IF=1), the IPI fires immediately. This matches the design intent: the IPI is the "kick" that breaks any spin-wait when the target CPU transitions to an IF=1 thread.

6. **Sanity test scenario (optional).** PM does NOT require a new test scenario for this dispatch. The closure gate retry is the verification. If Cerebro wants to add a small smoke test (e.g., send IPI, verify counter increments in handler, verify EOI is observed) for confidence, that's fine and counts as ship-quality work.

7. **`unsafe` discipline.** All new MSR reads/writes and APIC MMIO accesses carry SAFETY comments per Bible 15.2. Justify each block with the invariant being upheld.

8. **Documentation in commit body.** Reference Janus prior art (Linux `resched_curr()`, `smp_send_reschedule()`; Asterinas PR #2488). Reference ADR-013 §Q7 (this lands the foundation Q7 was planning). Reference the SC-3B investigation chain.

---

## 4. Pre-commit checklist

Before the commit lands:

- [ ] `docker compose run --rm neo cargo k` passes. Document warning count delta from 18-baseline.
- [ ] `git diff --stat` shows changes confined to expected files (likely: `kernel/src/arch/x86_64/apic.rs` (new), `kernel/src/arch/x86_64/mod.rs` (extend), `kernel/src/arch/x86_64/idt.rs` (vector registration), `kernel/src/arch/x86_64/smp.rs` (init_apic_ap call), `kernel/src/sched/scheduler.rs` (place_thread_on_cpu modification)).
- [ ] All new `unsafe` blocks have SAFETY comments.
- [ ] All existing default `cargo neo-test` scenarios pass at NEO_SMP=4. The IPI infrastructure should not regress any existing scenario; verify by running the default suite (per BUILD dispatch §9 Step 4 form).
- [ ] All five SC-3B SMP scenarios (`smp_block_release_smoke`, `smp_send_blocks_first_then_completes`, `smp_recv_blocks_first_then_completes`, `smp_grant_cross_cpu`, `smp_cross_cpu_migration_reap`) pass single-run sanity at NEO_SMP=4.
- [ ] Single-run `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4 passes.

---

## 5. Verification gate

Re-run BUILD dispatch §9 Step 6 + Step 7 on the post-Sub-step-5b substrate.

### 5.1 Required steps

1. **Step 6:** 200-run loop of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4. **200/200 PASS, zero classifier-signature timeouts.**
2. **Step 7:** 25-run loop of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=2. **25/25 PASS.**

Stop on first failure of any classifier signature per BUILD §9.4 / §9.5 (unchanged).

### 5.2 Artifact directory

```
target/verify/sc3b-sub-step-5b-gate/neo-smp4/
target/verify/sc3b-sub-step-5b-gate/neo-smp2/
```

### 5.3 Verification log

`Plans/SC-3B-SUB-STEP-5B-VERIFICATION-2026-05-09.md` (or 2026-05-10 if calendar advances). Structure mirrors prior verification logs.

### 5.4 Statistical context

If Step 6 passes 200/200, combined with prior 100/100 clean observations from Phase B:

- Total clean post-Fix-6.1-failure: 300/300 across multiple sample windows.
- P[300/300 | original rate=2.8%] ≈ 0.02%.
- P[200/200 alone | original rate=2.8%] ≈ 0.3%.

This is statistically strong but not airtight. The close report's framing (per `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §5) accommodates this: the bug observed twice is logged as causally addressed by the IPI fix; the close acknowledges the causation chain isn't fully proven but the architectural correction is real and ship-quality.

---

## 6. Outcomes and forward path

### 6.1 Outcome A — Step 6 + Step 7 pass

This is the expected outcome.

PM:
1. Reads verification log.
2. Produces Sub-step 6 dispatch for **Prometheus** (per the routing rules — docs work):
   - ADR-013 §Q4 + §Q6 + §Q7 amendments. §Q7 amendment notes the IPI infrastructure landed in SC-3B (sub-step 5b) rather than waiting for sub-commit 4; sub-commit 4 then uses the existing infrastructure for TLB shootdowns specifically.
   - ADR-011 supersession note.
   - PHASE-2.md update.
   - Close report at `Plans/CLOSE-REPORT-2026-05-09-sc-3b.md` (or appropriate date) with the honest framing per `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §5 and `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §1.
   - INDEX sweep (Prometheus per the new routing rules; Jarvis retired).
3. Fix 6.1 stash disposition surfaces post-closure as standalone test-quality decision.

### 6.2 Outcome B — Step 6 fails

The IPI fix landed correctly but didn't close the gate. The architectural gap was real but not the proximate cause (or only partial cause).

The fix commit STAYS LANDED. Bible 1.3: it ships ship-quality architectural correction independent of the gate result. We don't revert.

PM produces a Phase C instrumentation dispatch:
- PROGRESS frames in stress orchestrator (every 50 iterations) — test-only change; mirrors Fix 6.1's pattern but for instrumentation, not closure.
- cpu_id tracking on workers at block/wake points (test-only; per synthesis §3.2-3.4).
- Channel state dump extension to NMI handler (kernel modification under stash, not committed).
- Run targeted loop on the post-Sub-step-5b substrate (which now has IPI infrastructure).
- Capture data showing the actual hang mechanism with both NMI and PROGRESS visibility.

This is Phase C of the synthesis's instrumentation arc — third pass after Phase A (triple-parallel investigation) and Phase B (NMI + Janus).

### 6.3 Outcome C — Step 6 passes 200/200 but Step 7 fails

NEO_SMP=2 failure refutes "the bug is NEO_SMP=4-specific." Hypothesis space shifts.

PM produces a follow-up: NMI capture at NEO_SMP=2 with the IPI-equipped substrate. Different topology, possibly different failure mechanism.

### 6.4 Forward path summary

```
Sub-step 5b commit ──Outcome A──→ Sub-step 6 (docs + close → Prometheus)
                    │
                    ──Outcome B──→ Phase C instrumentation (PROGRESS + cpu_id + channel state) → Cerebro
                    │
                    ──Outcome C──→ NMI capture at NEO_SMP=2 → Cerebro
```

---

## 7. Stop conditions

Halt and ping PM if any of these fire:

- The diff exceeds ~300 lines net (modest scope). If APIC enablement turns out to require more substantial changes (e.g., x2APIC support somehow forced, or IDT machinery needing rework), surface before committing.
- New compile warnings exceed +5 above the 18-baseline.
- Any existing default scenario regresses at NEO_SMP=4.
- Any new `unsafe` block lacks a SAFETY comment.
- Pre-commit single-run sanity for any of the five sibling SMP scenarios fails.
- The IPI hits an unexpected exception (e.g., GPF, double fault) when issued — that means APIC initialization is wrong; halt and audit.
- Step 6 records ANY failure run with the missing-terminal-frame signature OR any other classifier-signature failure. Per BUILD §9.5: stop, don't extend. Outcome B path opens.
- Step 7 records any failure. Outcome C path opens.
- Backup script reports failure for two consecutive attempts.

If a stop condition fires AFTER the commit lands, the commit STAYS. Outcome B/C handling proceeds from the post-commit substrate.

---

## 8. Estimated wall time

- APIC initialization on APs: ~30-60 minutes (xAPIC is well-documented; Cerebro has Intel SDM Vol 3A Ch. 10 reference per Janus's prior-art recommendations).
- IPI vector + handler + send function: ~30-60 minutes.
- `place_thread_on_cpu` modification: ~10 minutes.
- Pre-commit sanity (cargo k + sibling scenarios): ~30 minutes.
- Commit + backup: ~15 minutes.
- 200-run NEO_SMP=4 gate: ~30-60 minutes (per Phase B timing, ~10s avg per PASS run; if the IPI fix works, all 200 should be PASS).
- 25-run NEO_SMP=2 gate: ~5-15 minutes.
- Verification log: ~30 minutes.

Total: ~3-5 hours from dispatch acceptance to handoff if Outcome A; longer if stop conditions force iteration.

---

## 9. Cross-references

**Authority for this dispatch:**
- `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3 + §5 — ranking-with-data + recommendation.
- `Plans/SC-3B-SOURCE-AUDIT-2026-05-09.md` — confirmation that no IPI infrastructure exists; AP main yields aggressively.
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` §1.4 — Linux `resched_curr()` design lineage; Asterinas PR #2488 prior art.

**Substrate context:**
- HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` — SC-3B Sub-step 5 commit (kernel work mechanically correct per audit; this dispatch lands additive infrastructure).
- BUILD dispatch §1.7 originally scheduled IPI infrastructure for sub-commit 4 (Q7 TLB shootdowns); this dispatch advances it into SC-3B because the bug forces it.

**Specification context (unchanged):**
- BUILD dispatch §2.4 — `place_thread_on_cpu` original spec.
- ADR-013 §Q4 — wake-on-completer-CPU (mechanically correct per audit + NMI snapshot; unchanged).
- ADR-013 §Q6 — lock-order asm release (mechanically correct per audit; unchanged).
- ADR-013 §Q7 — TLB shootdowns (this dispatch lands the IPI foundation Q7 will use; Q7 itself remains in sub-commit 4).

**Verification gate:**
- BUILD dispatch §9 Step 6 + Step 7 — the gate-as-specified, unchanged.
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — L's discharge framing as statistical precedent.

**Outcome A close-report inputs:**
- `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` §5 — pre-figured close-report framing.
- `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §1 — convergent synthesis headline.
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` — original three-way investigation.
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §2.7 — three-way investigation summary.

**Outcome B / C follow-up:**
- `stash@{1}` (NMI watchdog) preserved for re-arm.
- Synthesis §3.2-3.4 — second-pass instrumentation strategy (PROGRESS + cpu_id + channel state).

**Routing rule applies:**
- Per `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md` §2.4 — Sub-step 6 docs go to Prometheus (after this dispatch's gate passes).
- Phase C instrumentation kernel work (if Outcome B) goes to Cerebro.

**Governing posture:**
- `docs/bible.md` Articles 1.2 (correctness > coherence > performance), 1.3 (every commit ships — IPI infrastructure ships ship-quality architectural correction independent of gate outcome), 4.3 (every dependency a liability — minimal APIC surface; xAPIC not x2APIC; no APIC timer in this dispatch), 8.3 (no global locks in hot paths — IPI design respects this), 14 (cadence — this is BUILD/VERIFY of an embedded PLAN), 15.2 (SAFETY comments).
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §10.2 — forward concern: AI inference workloads under capability-passing IPC need this same IPI infrastructure when implemented at Phase 5+.

---

**End of dispatch.**

*Sub-step 5b: APIC enablement on APs + reschedule-IPI vector + handler + send_reschedule_ipi function + place_thread_on_cpu modification. One commit on top of a61e318. Verification: BUILD §9 Step 6 (200/200 NEO_SMP=4) + Step 7 (25/25 NEO_SMP=2). Honest framing: ship-quality architectural correction regardless of gate outcome. Outcome A → Sub-step 6 docs to Prometheus. Outcome B → Phase C instrumentation to Cerebro. Outcome C → NMI capture at NEO_SMP=2 to Cerebro. Estimated 3-5 hours. PM is on call.*
