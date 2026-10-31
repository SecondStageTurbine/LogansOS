# SC-3B PLAN synthesis: Cerebro × Prometheus

**Date:** 2026-05-09
**Author:** PM
**Status:** Synthesis of two parallel PLAN-stage analyses. Input to project-lead ratification. Per Bible Article 14, this closes the PLAN stage; project-lead ratification opens BUILD.
**Inputs:**
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md`
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md`

---

## 0. Headline

Both agents converge on the architectural shape: SC-3B = Q4 wake-on-completer-CPU + Q6 lock-order asm release + IPC retrofit at 4 call sites. They agree on substrate, panic-safety discipline, cross-process-bookkeeping ordering, A1 composition, and Bible 8.5.3 partial advancement.

They diverge meaningfully on **two coupled questions**:

1. **Q6 ABI shape** — Cerebro recommends per-call-site specialization (`context_switch_with_release2`); Prometheus recommends const-generic array (`context_switch_with_release<const N: usize>`).
2. **Lock-byte ownership** — Cerebro recommends introducing a kernel-owned `SwitchMutex` type to avoid depending on `spin::Mutex` private layout; Prometheus recommends using `spin::Mutex::into_raw()` directly (and acknowledges in §4 that the API needs verification).

There is also a **third divergence** that PM caught during review:

3. **Lock release order** — Cerebro specifies Channel-first, scheduler-second (consistent with `SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` Invariant I1). Prometheus's implementation sketch passes `&[sched_lock_ptr, channel_lock_ptr]` — scheduler-first. The pre-thinking note's I1 explicitly load-bearing reasoning ("the parked thread becomes reachable for rendezvous completion before scheduler becomes available for other operations") favors Channel-first. Prometheus's text doesn't address ordering as a hard constraint.

The remaining six questions converge or have minor differences not worth rehashing.

---

## 1. Per-question synthesis

### 1.1 Q6 ABI shape — DIVERGENCE

**Cerebro recommendation:** Option (c) — per-call-site specialization.

```rust
unsafe extern "C" fn context_switch_with_release2(
    prev_rsp_ptr: *mut u64,
    next_rsp: u64,
    release_first: *const AtomicBool,
    release_second: *const AtomicBool,
);
```

Lock pointers passed in registers (RDX, RCX). No release-list metadata on the old stack. If a four-lock variant is ever needed, add `context_switch_with_release4` with all four pointers in registers and null = unused.

**Prometheus recommendation:** Option (a) — const-generic array.

```rust
unsafe extern "C" fn context_switch_with_release<const N: usize>(
    prev_rsp_ptr: *mut u64,
    next_rsp: u64,
    locks: &[*const AtomicBool; N],
) -> i64;
```

One helper, monomorphizes per N, asm loops over the array.

**The trade:**

- **Cerebro's argument:** A `&[...; N]` array is a fat pointer (or at least a pointer to N stack slots). The slice/array data lives on the **old stack** at the moment the asm enters. After `mov rsp, rsi`, the helper is on the new stack but its release-loop wants to read from `r8 + r9*8` — which is the *old stack* address it captured pre-switch. This is technically safe if the asm captures all release pointers into registers BEFORE the stack switch, but it's a footgun: any future maintainer who refactors the asm risks reading post-switch from an old-stack address that another CPU may have started overwriting (the old thread's stack becomes reusable once it's parked and the stack-switch publishes the unlock).
- **Prometheus's argument:** Forward-compatibility for Q8.c. The blocked-thread-kill walk may need to release N>2 locks. Const-generic handles any N naturally.

**PM analysis:**

The Cerebro old-stack-read concern is real and matches the pre-thinking note §5.1's stated rejection of "borrowed slice" forms ("the helper must not read release metadata from the blocked thread's old stack while releasing the first lock may make that old thread visible to other CPUs"). The pre-thinking note is unambiguous: the old-stack metadata read is the failure mode being avoided. That is a normative input.

Prometheus's Q8.c forward-compat argument is real but premature — Q8.c is in a future BUILD (BUILD-5+ per the THINK note's roadmap). When Q8.c lands, we can add `context_switch_with_release_n` if needed. Specializing now is consistent with Bible 14's /simplify gate ("anything that can be deleted from the plan without breaking the goal milestone is deleted").

Prometheus's asm sketch reads release pointers from `[r8 + r9*8]` *after* the stack switch (`mov rsp, rsi` precedes the lock-release loop). r8 was loaded from rdx pre-switch, but **the memory r8 points to is on the old stack**. After `mov rsp, rsi`, the helper has switched to the new stack and the old stack is potentially reclaimable. This is the exact failure mode the pre-thinking note rejected.

It is fixable in Prometheus's design — capture all pointers into registers before the stack switch. But you'd need 6+ live registers (prev_rsp_ptr, next_rsp, plus N lock pointers) across the switch, which limits N severely (probably N≤4 anyway given x86_64 register pressure). At N≤4, the const-generic abstraction provides minimal benefit over per-call-site specialization.

**Recommendation:** Adopt Cerebro's Option (c). Per-call-site specialization. `context_switch_with_release2` for SC-3B's two-lock case. If Q8.c later needs more, add `context_switch_with_release4` then.

---

### 1.2 Lock-byte ownership — DIVERGENCE (coupled with 1.1)

**Cerebro recommendation:** Introduce a new kernel-owned spinlock type.

```rust
pub struct SwitchMutex<T> { lock: AtomicBool, inner: UnsafeCell<T> }
// guard.into_raw() returns *const AtomicBool
// guard.release_token() returns *const AtomicBool (alternative API name)
```

Migrate `Channel.state` and `PerCpuScheduler.inner` to it. All other `spin::Mutex` users stay.

**Prometheus recommendation:** Use `spin::Mutex::into_raw()`.

Acknowledged risk in §4: "The `spin::Mutex::into_raw()` API must exist. If it doesn't, we may need to use `LockResult` or a different pattern. Verify spin crate API."

**PM analysis:**

The `spin::Mutex` API does **not** have a stable `into_raw()` method as of the version in our `Cargo.toml`. The crate's `Mutex<T>` exposes `lock()`, `try_lock()`, `force_unlock()` (unsafe), and the underlying `RawMutex` has internal-but-not-publicly-stable representation. The `AtomicBool` field is `pub(crate)` not `pub`, meaning it's accessible inside the spin crate but not to consumers.

Cerebro's flag is correct: depending on `spin::Mutex` internal layout would create a hidden ABI tie to a third-party crate. Bible Article 4.3 ("every external dependency is a liability") explicitly disfavors this. If the `spin` crate updates and changes its internal lock representation (the `lock_api` feature, for instance, has different internals), our asm helper silently breaks.

A kernel-owned lock type is a small addition. Cerebro's sketch is ~30-50 lines of straightforward code: AtomicBool + UnsafeCell + RAII guard + `release_token()` accessor. This is the same magnitude as A1's IRQ-safe wrapper. Bible 4.3 prefers in-house.

**Recommendation:** Adopt Cerebro's `SwitchMutex` proposal. Migrate `Channel.state` and `PerCpuScheduler.inner` to it as Sub-step 1 of the BUILD (no behavior change at that step; just type substitution). Keep `spin::Mutex` for all non-Q6 callers.

---

### 1.3 Lock release order — DIVERGENCE (PM-surfaced, both agents could clarify)

**Cerebro recommendation:** Channel-first, scheduler-second. Explicitly per pre-thinking note Invariant I1.

**Prometheus implementation sketch:** `&[sched_lock_ptr, channel_lock_ptr]` — scheduler-first.

**Prometheus rationale text:** Doesn't address release order as a load-bearing invariant.

**PM analysis:**

Pre-thinking note `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` §3.4 and §4 (Invariant I1) explicitly establish Channel-first ordering as load-bearing: "releasing Channel mutex first means the parked thread is immediately reachable for completion, and only then does CPU N's scheduler become available for other operations (BSP reaper, kill scans, etc.)."

This is a Q4-correctness consideration: cross-CPU rendezvous wake (Q4) should be able to proceed as soon as the parked thread is published in Channel state, even if CPU N's scheduler is still nominally locked. Channel-first release minimizes the window.

Reverse order (scheduler-first) is not deadlocking, but is suboptimal: a completer-CPU waiting on Channel state has to wait for both releases, increasing latency.

**Recommendation:** Channel-first, scheduler-second. This is a documentation/code-comment item more than a behavior split — the asm just stores in array order, so the IPC call site passes `[channel_lock_ptr, sched_lock_ptr]`. Cerebro's recommendation is consistent with pre-thinking I1; Prometheus's sketch should be inverted.

This is a small fix to either proposal at BUILD time, but PL should ratify the order explicitly so the BUILD dispatch can enforce it.

---

### 1.4 Memory ordering — CONVERGENCE

Both agents recommend: no explicit fence (`mfence`/`lfence`/`sfence`) needed.

Both cite x86 TSO and `Ordering::Release` lowering to plain MOV + compiler barrier. Both reference Linux's `spin_unlock` precedent.

Cerebro adds: the helper should not save/restore RFLAGS unless separately ratified. Current `context_switch` doesn't, the dispatch path runs under `without_interrupts`, and adding `pushfq`/`popfq` could resurrect stale interrupt state from old context. **PM concurs** — RFLAGS preservation is a separate question and shouldn't be silently added.

**Recommendation:** No fence. No RFLAGS save/restore. Both agents agree on the no-fence point; ratify Cerebro's RFLAGS clarification too.

---

### 1.5 Panic-safety of forget-then-asm window — CONVERGENCE

Both agents recommend: single `unsafe` block, no panicable operations between `mem::forget` and the asm call.

Both reject `#[track_caller]` and `core::intrinsics::abort` as primary defenses (overkill given the window is provably panic-free by code structure).

Both propose a generic helper signature:
- Cerebro: `block_current_into_with_guard<T, F>(outer_guard: SwitchMutexGuard<'_, T>, installer: F)`
- Prometheus: takes the channel guard via closure capture

Cerebro's signature is more explicit about ownership. Prometheus's via-closure-capture is simpler but harder to audit.

**Recommendation:** Cerebro's signature. The explicit guard parameter makes the ownership chain auditable: who passes the guard in, who releases it. Closure-capture obscures this. PL ratifies; small detail.

---

### 1.6 Cross-process bookkeeping ordering — CONVERGENCE

Both agents agree:
- `cross_process_prep` and `cross_thread_kstack` happen inside the SCHED-locked window, before the installer.
- TSS.rsp0 and PER_CPU.kernel_rsp are written before the stack switch (otherwise SYSCALL entry on the new thread could land on a stale stack).
- `next_rsp` is read from `s.current.rsp` after `mem::replace`.
- Neither bookkeeping function allocates from the heap (verified by both).

Cerebro adds: `cr3_for_thread()` locks PROCESSES to read pmap. That's existing behavior, not new. SC-3B should not optimize this; Q7.b CR3 cache is the right place for that work later.

**Recommendation:** Adopt the converged ordering. Don't optimize cr3_for_thread inside SC-3B.

---

### 1.7 Block-target enumeration — CONVERGENCE (with one PM concern)

Both agents enumerate the same 4 call sites:
- `kernel/src/ipc/mod.rs` — `Channel::send` (SendBlocked)
- `kernel/src/ipc/mod.rs` — `Channel::recv` (RecvBlocked)
- `kernel/src/ipc/mod.rs` — `Channel::grant` (GrantBlocked)
- `kernel/src/ipc/mod.rs` — `Channel::recv_grant` (RecvGrantBlocked)

Both confirm: all 4 hold exactly 2 locks (Channel state + scheduler).

**PM concern:** Prometheus cites specific line numbers (198, 257, 321, 388) that PM did not independently verify. Cerebro names the functions without line numbers. Cerebro's approach is safer when line numbers may drift. The line numbers are not load-bearing; the function-level identification is what BUILD needs. Both proposals are usable.

Cerebro raises an **additional** issue: the current `Thread` struct does NOT expose a `cpu_id` field, but ADR-013 §Q4 assumes it. SC-3B may need to add it (likely in `kernel/src/sched/thread.rs`). Prometheus does not surface this. **PM concurs with Cerebro's flag** — needs to be explicit in BUILD planning.

**Recommendation:** Adopt the 4-call-site enumeration. Add `Thread.cpu_id` to BUILD scope per Cerebro's flag.

---

### 1.8 Test scenario set — DIVERGENCE (counts and emphasis)

**Cerebro recommendation:** 6 scenarios.
1. `smp_block_release_smoke` — minimal Q6 helper validation
2. `smp_send_blocks_first_then_completes` — sender-first
3. `smp_recv_blocks_first_then_completes` — receiver-first
4. `smp_grant_cross_cpu` — capability transfer cross-CPU
5. `smp_cross_cpu_migration_reap` — exit on completer-CPU + reap
6. `smp_cross_cpu_rendezvous_stress` — primary stress test (200/200 closure gate)

Cerebro flags a test infrastructure prerequisite: **no current way to place a test thread on a specific AP.** SC-3B needs a minimal placement hook (production-safe or test-feature-gated).

Cerebro specifies PASS frame fields explicitly: `blocked_cpu`, `completer_cpu`, `resumed_cpu`, `iterations`, `send_first`, `recv_first`, `grant_rights`, `exit_reaped`.

**Prometheus recommendation:** 4 primary + 1 deferred.
1. `smp_send_blocks_first_then_completes`
2. `smp_recv_blocks_first_then_completes`
3. `smp_grant_cross_cpu`
4. `smp_concurrent_rendezvous_storm`
5. `smp_kill_during_rendezvous` — deferred to Q8.c

Prometheus does not surface test placement infrastructure or PASS frame fields.

**PM analysis:**

Cerebro's coverage is broader and more implementation-aware:
- The minimal smoke test (`smp_block_release_smoke`) is valuable — it exercises Q6's helper alone before layering Q4 scenarios.
- The migration-reap test catches a real risk class (cross-CPU reap of a thread whose CPU ownership migrated through rendezvous).
- The test placement hook is a real prerequisite — without it, "cross-CPU" tests may all run on the BSP by accident, masking bugs. This is exactly the kind of test-infrastructure gap that produces false-green CI.
- The PASS frame field specification makes test failures debuggable.

Prometheus's set is leaner but misses these.

**Recommendation:** Adopt Cerebro's 6-scenario set with the placement-hook prerequisite and PASS frame field specification. Prometheus's `smp_concurrent_rendezvous_storm` and Cerebro's `smp_cross_cpu_rendezvous_stress` are the same scenario; pick a name.

---

### 1.9 Verification methodology — CONVERGENCE (with Cerebro more specific)

Both agents converge on:
- 200/200 primary stress at NEO_SMP=4 (mirrors L's discharge standard).
- 50/50 supplementary scenarios.
- Anti-regression set covers all existing single-CPU IPC + cap + slab + buddy + kaslr scenarios.
- Statistical confidence target: 0/200 is the minimum closure signal.

Cerebro is more specific:
- 7-step gate order (cargo k → single-boot smoke → per-scenario boots → full default suite → 50-run loops → 200-run primary stress → NEO_SMP=2 cross-check → optional NEO_SMP=8).
- Failure-class enumeration (lost wakeup, deadlock-unpublished, deadlock-unwoken, wrong-CPU migration, duplicate frame, missing frame, panic, frame leak, rights mismatch, EBADF/EPIPE).

Prometheus has the same shape with less granularity.

**Recommendation:** Adopt Cerebro's gate order and failure-class enumeration. They make the BUILD dispatch's verification §writeable.

---

### 1.10 A1 composition + 8.5.3 advancement — CONVERGENCE

Both agents:
- Verify `cross_process_prep` and `cross_thread_kstack` do not allocate.
- Confirm Q6's asm helper does not touch the heap.
- Confirm Q6 advances Bible 8.5.3 partially (not full conformance — A2 remains the formal milestone).
- Confirm A1's IRQ-safe heap doesn't constrain Q6 work.

Cerebro adds caveat: immediate grant completion may mutate handle tables outside the Q6 window. This is existing Phase 2 BUILD-1 behavior and is consistent with 8.5.3 by being outside the SCHED-locked window. SC-3B should not expand this.

**Recommendation:** Adopt the converged composition. Note Cerebro's caveat about grant completion in the BUILD dispatch's commit-message guidance.

---

## 2. Quality observations

This section is for project-lead calibration only. Both agents performed real analysis; the differences below are observations, not pejoratives.

**Cerebro's analysis:**
- Source-grounded: explicit list of files read at the top.
- Identifies the spin::Mutex private-layout risk early and proposes a sound mitigation.
- Surfaces two critical infrastructure issues PM hadn't seen: missing `Thread.cpu_id`, missing test placement hook.
- Verification methodology is specified at gate-order granularity.
- Risk enumeration is thorough.
- Implementation slicing (6 sub-steps) is concrete and shippable.

**Prometheus's analysis:**
- Well-organized, follows dispatch structure cleanly.
- Forward-looking re: Q8.c (const-generic for compatibility).
- Implementation sketch is detailed (asm pseudocode, Rust pseudocode).
- Three notable issues:
  1. Lock release order inverted from pre-thinking I1 (likely an oversight; not engaged with the I1 reasoning).
  2. Asm release-loop reads release-list metadata from the old stack post-switch, the exact failure mode pre-thinking §5.1 rejects. Fixable but indicates the asm sequence wasn't audited against pre-thinking constraints.
  3. Acknowledges in §4 that `spin::Mutex::into_raw()` API may not exist; the API doesn't actually exist as proposed, so the proposed implementation needs revision. Cerebro caught this and proposed an alternative; Prometheus flagged uncertainty without resolving it.

These issues don't make Prometheus's analysis unusable — they make it a first draft that PM revision could correct. Prometheus engaged seriously with the dispatch and produced material that informs the synthesis. The differences from Cerebro are the kind of thing that a second pass with the pre-thinking note open would resolve.

This is calibration data on Prometheus, not a criticism. Project lead can weight as needed.

---

## 3. Recommended ratifications

Per question, PM recommends:

| # | Question | Recommendation | Rationale |
|---|----------|----------------|-----------|
| 1.1 | Q6 ABI shape | **Per-call-site (Cerebro)** | Old-stack metadata read avoided; pre-thinking §5.1 alignment |
| 1.2 | Lock-byte ownership | **`SwitchMutex` (Cerebro)** | Bible 4.3; spin::Mutex private layout not stable |
| 1.3 | Release order | **Channel-first, scheduler-second** | Pre-thinking I1; Q4 latency optimization |
| 1.4 | Memory ordering | **No fence** | Both agents converge; x86 TSO + Release sufficient |
| 1.5 | Panic-safety | **Single unsafe block, explicit guard parameter (Cerebro)** | Auditability |
| 1.6 | Bookkeeping ordering | **Converged ordering** | Both agents agree |
| 1.7 | Block-target enumeration | **4 call sites + Thread.cpu_id flag (Cerebro)** | Adds infrastructure work |
| 1.8 | Test scenarios | **6 scenarios (Cerebro) + placement hook prerequisite + PASS field spec** | Coverage and debuggability |
| 1.9 | Verification methodology | **Cerebro's 7-step gate order + failure-class enumeration** | Dispatch-writeable |
| 1.10 | A1 + 8.5.3 advancement | **Converged composition + Cerebro's grant-completion caveat** | Both agents agree |

Net: 8 of 10 lean Cerebro, 2 are converged.

---

## 4. SC-3B BUILD scope (consolidated)

If PL ratifies the synthesis, the BUILD dispatch covers:

**Sub-step 1: Infrastructure prerequisites (~1-2 days)**
- Add `SwitchMutex<T>` type in `kernel/src/sync/` or `kernel/src/sched/`. RAII guard with `release_token()` returning `*const AtomicBool`. ~30-50 lines.
- Migrate `Channel.state` (in `kernel/src/ipc/mod.rs`) and `PerCpuScheduler.inner` (in `kernel/src/sched/scheduler.rs`) to `SwitchMutex<T>`. No behavior change.
- Add `Thread.cpu_id` field. Spawn/push_ready/wake/wake_front update it.
- Add minimal cross-CPU test placement hook (test-feature-gated or production-safe).
- Verify: cargo k passes, all existing default scenarios pass at NEO_SMP=4.

**Sub-step 2: Q6 helper (~1-2 days)**
- Add `context_switch_with_release2` asm helper in `kernel/src/arch/x86_64/context.rs`. Two explicit register lock pointers, Channel-first release order, no RFLAGS save/restore.
- Add `block_current_into_with_guard<T, F>` Rust wrapper.
- Add `smp_block_release_smoke` scenario.
- Verify: smoke passes, no regressions.

**Sub-step 3: Send/recv retrofit (~2-3 days)**
- Modify `Channel::send` and `Channel::recv` to hold the Channel state lock through `block_current_into_with_guard`.
- Implement Q4 wake-on-completer-CPU semantics in the rendezvous-completion paths.
- Add `smp_send_blocks_first_then_completes` and `smp_recv_blocks_first_then_completes`.
- Verify: cross-CPU IPC tests pass, anti-regression clean.

**Sub-step 4: Grant retrofit (~1-2 days)**
- Modify `Channel::grant` and `Channel::recv_grant` to use the same pattern.
- Add `smp_grant_cross_cpu`.
- Verify: cap-grant cross-CPU passes, capability rights correct.

**Sub-step 5: Migration/reap + stress (~2-3 days)**
- Add `smp_cross_cpu_migration_reap` and `smp_cross_cpu_rendezvous_stress`.
- Run 200-boot closure gate on stress.
- Verify: zero failures of any classifier signature.

**Sub-step 6: Documentation + close (~1 day)**
- Update ADR-013 §Q6 with the chosen ABI shape, SwitchMutex introduction, and verification evidence.
- Possible amendment to ADR-011 (further superseded supplementation).
- Close report at `Plans/SC-3B-CLOSE-REPORT-2026-05-09.md` (or whenever it lands).
- INDEX sweep capturing SC-3B sub-step commits.

**Total estimate:** ~8-13 days of Cerebro time. Multi-commit within one BUILD-3 sub-commit boundary.

---

## 5. Open items not in the eight questions

PM-surfaced during synthesis:

- **`spin::Mutex::into_raw()` doesn't exist.** Prometheus's design assumed it; SwitchMutex obviates the need.
- **`Thread.cpu_id` doesn't exist.** Cerebro flagged; needs to be added in Sub-step 1.
- **Test placement hook doesn't exist.** Cerebro flagged; needs minimal implementation in Sub-step 1.
- **PASS frame fields not in the test harness yet.** Cerebro specified what should be in the new frames; harness changes needed in Sub-step 2.

None of these are scope-altering, but each adds 0.5-1 day to the BUILD timeline.

---

## 6. Recommended next moves

1. **Project lead reviews this synthesis.**
2. **Project lead ratifies (or modifies) per-question recommendations** in §3.
3. **PM drafts SC-3B BUILD dispatch** using ratified answers.
4. **Cerebro implements** per the multi-step plan in §4.
5. **Verification at NEO_SMP=4** per Cerebro's gate order.
6. **Close report + INDEX sweep + ADR-013 amendment** at sub-commit close.

**Forward queue:** PL ratifies → PM drafts BUILD → Cerebro builds. Jarvis remains on standby; could pick up the eventual close-report INDEX sweep if PL wants parallel work, but no other immediate need.

---

## 7. Cross-references

- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` — input.
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md` — input.
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — pre-thinking inputs (Invariants I1–I6 cited).
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md` — THINK-stage scope ratification.
- `Plans/DISPATCH-2026-05-09-sc-3b-plan-stage-parallel.md` — dispatch that produced both PLAN inputs.
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6 — substrate.
- `docs/bible.md` Articles 4.3, 8.3, 8.5, 14, 15.2 — governing posture.
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §10.2 — AI-native primitives that reuse this work.

---

**End of synthesis.**

*Convergence is high (6 of 10 questions converged or near-converged). Divergences cluster on the Q6 mechanism (ABI, lock-byte ownership, release order) — which is also where the pre-thinking note has the most-detailed prior analysis. Cerebro's proposals align with pre-thinking I1–I6 more cleanly. PM recommends Cerebro's positions on the divergent questions and the converged answers on the rest. Project lead ratifies; PM drafts BUILD dispatch.*
