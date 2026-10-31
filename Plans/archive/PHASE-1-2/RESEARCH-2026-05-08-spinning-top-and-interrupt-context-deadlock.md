# Research: `spinning_top` posture and interrupt-context lock-acquire deadlock prior art

**Date:** 2026-05-08
**Author:** PM
**Type:** Research artifact (informational; not normative). Follow-up to `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md`, focused on the now-known specific shape: BSP spins in `spinning_top::RawSpinlock::lock` trying to acquire CPU 0's `PerCpuScheduler.inner`. Surveys the Rust no_std kernel community's prior experience with the bare-metal `spinning_top`-class pattern, and identifies the **strongest candidate hypothesis** to feed into the WEDGE-INV-J1 audit.
**Authority:** None — research report, not a PM decision or commitment. Recommendations herein require ADR or PM-NOTE to take effect. The J1 audit dispatch (independent artifact) is the operational follow-up.

---

## 0. TL;DR

- **The strongest single candidate hypothesis is interrupt-context same-CPU lock re-entrance.** This is a sub-case of the "re-entrant deadlock" family from `Plans/PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md` §2, and it is directly documented in the Rust no_std kernel community as a known pitfall of using plain spinlocks (which `spinning_top` provides) without an IRQ-safe wrapper. Pattern: process-context code on CPU 0 holds `PerCpuScheduler.inner`; a timer ISR or IPI handler arrives on the same CPU; the handler's path tries to take the same lock; spinlock is not re-entrant; CPU 0 spins forever waiting on a lock its own context holds.
- **`spinning_top` provides plain `Spinlock` with no built-in IRQ-safe variant.** The crate's API surface is `lock()` and `try_lock()`. There is no `lock_irq_disable()` equivalent. Callers must either (a) externally disable interrupts before acquire and re-enable after release, or (b) use a wrapper crate that handles this (e.g., `irq_safety`).
- **Theseus, Rust-for-Linux, and Redox have all hit this and built standardized solutions.** Theseus has a dedicated `irq_safety` crate. Rust-for-Linux has `SpinLock::lock_irqdisable()` as a first-class API distinct from `lock()`. Redox's kernel runs with "interrupts disabled almost everywhere in kernel mode" as a baseline. The fact that three independent Rust kernel projects have converged on irq-disabled-while-locked solutions is strong signal that the un-wrapped pattern is a known footgun.
- **The hypothesis matches the AltimitNeo wedge symptoms.** Timing-sensitive (timer-arrival distribution depends on host timing), heisenbug under instrumentation (instrumentation perturbs timer cadence), four signatures consistent with different timer arrival points relative to the lock-hold window, ADR-015 substrate-shift damping consistent with changed instruction-cache behavior changing the timer-arrival-vs-lock-hold race window.
- **If the hypothesis holds, the fix pattern is well-established.** Three options, in increasing invasiveness: (a) add `interrupts::disable()` / `interrupts::enable()` (or a guard type) around every `PerCpuScheduler.inner.lock()` call on the same CPU; (b) introduce an in-kernel wrapper type (similar to Theseus's `irq_safety::MutexIrqSafe`) that handles this automatically; (c) switch from `spinning_top` to a crate that provides irq-safe locking out of the box.
- **The hypothesis is NOT proved by this research.** It's a strong candidate that the WEDGE-INV-J1 audit can verify or refute by inspecting whether `PerCpuScheduler.inner` is ever acquired from interrupt context AND whether the process-context acquirers disable interrupts. If the audit refutes this hypothesis, the next strongest candidate is cross-CPU lock-while-blocked; if it confirms, the fix-design dispatch follows directly.

---

## 1. The pattern: bare-metal spinlock + interrupt handler

### 1.1 The core warning (matklad)

Aleksey Kladov's "Spinlocks Considered Harmful" (Jan 2020, `matklad.github.io/2020/01/02/spinlocks-considered-harmful.html`) is the canonical Rust-community-adjacent treatment. The bare-metal section, paraphrased: on bare metal we don't worry about thread preemption (no scheduler to preempt us) but we must worry about processor interrupts. If main code is in the middle of a critical section when an interrupt arrives, and the interrupt handler enters the same critical section, the result is guaranteed deadlock. Kladov names this as a kernel-specific reason (one of several) to avoid spinlocks where a mutex would do, or to ensure interrupts are disabled while a spinlock is held.

The deadlock geometry: the interrupted CPU is paused mid-critical-section (lock held). The handler runs on the same CPU (handlers are CPU-local). The handler's `lock()` call spins waiting for the lock. The CPU never returns from the handler because it's spinning. The lock is never released because the original code is still suspended waiting for the handler to return. Two stack frames on one CPU, one holding a lock, one trying to acquire it. Forever.

### 1.2 Linux convention

The Linux kernel's locking documentation (`docs.kernel.org/locking/locktypes.html`, `kernel.org/doc/Documentation/locking/spinlocks.txt`) is unambiguous: any lock used in both process context and interrupt context must disable interrupts on the local CPU during process-context acquisition. The canonical APIs are `spin_lock_irqsave()` / `spin_unlock_irqrestore()`. The lockdep tool actively checks this invariant; violations are caught at runtime in debug builds.

The Linux locking-types table (in `docs.kernel.org/kernel-hacking/locking.html`'s "Table of Minimum Requirements") explicitly tabulates the requirement: process-context × IRQ-handler intersection requires `SLI` (`spin_lock_irq`) or `SLIS` (`spin_lock_irqsave`). The plain `SL` (`spin_lock`) is only safe when both contexts are process-context with no interrupt-handler intersection.

This isn't a Linux-specific quirk — it's the standard pattern of every modern OS kernel that uses spinlocks. FreeBSD, NetBSD, Solaris, Windows kernel, all enforce equivalent rules.

### 1.3 Rust no_std kernel community

Three independent Rust kernel projects have converged on solutions that handle this automatically:

**Theseus's `irq_safety` crate** (`github.com/theseus-os/irq_safety`, also forked at `github.com/kevinaboos/irq_safety`): provides `MutexIrqSafe` and `RwLockIrqSafe` wrappers that disable interrupts for the duration of the lock guard, restoring the prior interrupt state on guard drop. The crate's stated purpose is "Irq-safe locking via Mutex and RwLock... offers identical behavior to the regular spin crate's Mutex and RwLock, with the added behavior of holding interrupts for the duration of the Mutex guard." The fact that Theseus felt the need to package this as a standalone crate, rather than using plain `spin::Mutex` directly, indicates the team hit the deadlock pattern in practice.

**Rust-for-Linux's `kernel::sync::SpinLock`** (`rust-for-linux.github.io/docs/kernel/sync/struct.SpinLock.html`): provides two distinct lock methods — `lock()` (no interrupt state change) and `lock_irqdisable()` (disables interrupts while held, restores on release). The API requires the caller to choose explicitly; there is no implicit safe default. The fact that this distinction is encoded as two named methods, not one, signals that the choice is meaningful and that getting it wrong is a known bug class.

A 2025 LWN article (`lwn.net/Articles/1039374/`, "A new API for interrupt-aware spinlocks") covers active discussion in the Rust-for-Linux community about refinements to this API. The article includes the canonical statement of the problem in plain language: if process-context code acquires a spinlock and then an interrupt arrives that tries to acquire the same lock, the system will deadlock.

**Redox kernel posture** (`redox-os.org/news/kernel-10/`): "non-maskable interrupt IPIs to interrupt other CPUs at scheduled intervals, which is necessary since interrupts are disabled almost everywhere in kernel mode." Redox's design baseline is "interrupts off in kernel mode." This avoids the pattern entirely — if interrupts are off whenever a spinlock could be held, the interrupt handler can't run during a critical section. AltimitNeo's posture on this question is the audit's first job to determine.

The convergence across three projects is the signal: this problem is real, recurring, and serious enough that the Rust no_std kernel community has standardized solutions for it. AltimitNeo's use of plain `spinning_top` puts it in the un-wrapped state that the other three projects moved away from after experiencing the problem.

---

## 2. Why this matches the AltimitNeo wedge symptoms

### 2.1 Timing sensitivity

Interrupt-arrival timing is host-dependent. QEMU's timer-tick scheduling depends on host load, instruction emulation cadence, and the QEMU build's timing model. The cargo-driven path (with its own subprocess overhead) produces a different timer cadence than direct QEMU. WEDGE-INV-6's signature shift (`SMOKE_MID_PHASE` instead of `POST_SMOKE_SMP_SCENARIO`) under direct QEMU is consistent with "the same race fires at a different point in execution because timer arrival is shifted by the host-side timing change."

### 2.2 Heisenbug under instrumentation

WEDGE-INV-2's instrumented smoke paths produced 0/30 because the instrumentation (extra prints, extra branches) widened the lock-hold window. A wider lock-hold window means more chances for a timer ISR to arrive, which would actually INCREASE the bug rate if the bug were "interrupt arrives during lock hold." But the observation is the opposite — instrumentation HID the bug. That's not consistent with "wider window = more interrupts" naively.

The reconciliation: the instrumentation also changed the instruction cache layout, the branch predictor state, and the relative timing of the lock-acquire phase vs. the timer-arrival phase. McKenney's framework (`Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` §2.1) is explicit: any change to the kernel's instruction stream changes timing in ways that can move the bug's window in or out of the firing zone. The interrupt-context hypothesis is consistent with this — the instrumentation simply happens to put the timer-arrival window outside the lock-hold window in the instrumented configuration.

### 2.3 Four signatures

The four historical signatures (`POST_SMOKE_SMP_SCENARIO`, `SMOKE_MID_PHASE`, `SMOKE_COMPLETION` literal, `POST_SMOKE_USER_PROCESS`) all sit in the partial-reap-class of execution, but at different points. Under the interrupt-context hypothesis, these correspond to different points in the same lock-hold window where a timer ISR happened to arrive. Different memory layouts (PATCH1 vs. post-ADR-015) shift the lock-hold window's absolute position in execution; different host-side timing (cargo vs. direct QEMU) shifts the timer-arrival distribution; the intersection is the bug, and which point in the window the timer happens to hit produces which signature.

This is consistent — though not proved — with the interpretation in `Plans/PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md` §3 that the four signatures could be one bug, four masks. The interrupt-context hypothesis provides a specific mechanism for that interpretation.

### 2.4 ADR-015 damping

ADR-015 shifted the kernel heap to a KASLR-tolerant placement. This changes:

- Heap object addresses (so iteration cache patterns shift).
- Instruction cache contents (so branch-prediction and prefetch behavior shift).
- The relative time spent in different scheduler/reaper code paths (because cache misses shift).

All of these change the timer-arrival-vs-lock-hold race window. ADR-015 didn't fix the bug; it shifted the window. The substrate-shift-damping observation in `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10 is consistent.

---

## 3. The `spinning_top` crate's posture

`spinning_top` is the simple-spinlock crate maintained by the Rust OSDev project (`github.com/rust-osdev/spinning_top`). It's built on `lock_api`. The exposed types are `Spinlock<T>`, `SpinlockGuard<T>`, and the underlying `RawSpinlock` raw-mutex implementation that we observed in the WEDGE-INV-8 stack capture.

The API is intentionally minimal:

- `Spinlock::new(data)` — construct.
- `Spinlock::lock()` — blocking acquire, returns guard.
- `Spinlock::try_lock()` — non-blocking acquire.
- Guard drop releases the lock.

There is no `lock_irq_disable()` or equivalent. There is no `lock_no_irq()` variant that asserts interrupts are already disabled. The crate provides primitive locking; IRQ-state management is the caller's responsibility.

This is consistent with `lock_api`'s design philosophy: provide the locking primitive, leave the IRQ-state policy to the caller. It's the same posture as the Rust `spin` crate (`crates.io/crates/spin`), which links Kladov's "Spinlocks Considered Harmful" prominently in its README to flag this exact concern.

Conclusion: if AltimitNeo uses `spinning_top` directly without a wrapper that disables interrupts during acquire, **and** any interrupt handler can call into a code path that takes `PerCpuScheduler.inner`, then the deadlock pattern from §1.1 is structurally enabled. Whether it actually fires depends on whether the conditions (lock held + ISR arrives + ISR takes lock) intersect — which is exactly the timing-sensitive question the wedge investigation has been chasing.

---

## 4. What WEDGE-INV-J1 should check explicitly

`Plans/DISPATCH-2026-05-08-wedge-investigation-J1-audit-cerebro.md` already covers the relevant ground in §3.1 (lock-hold-region analysis) and §3.4 (schedule-tick and IPI handlers, marked "best-effort"). This research suggests promoting §3.4 from best-effort to highest-priority. Specifically:

1. **Is `PerCpuScheduler.inner` ever acquired from interrupt context?** Look for any path: timer ISR, APIC IPI handler, exception handler, that calls into a function that ends up calling `inner.lock()` or `inner.try_lock()`. The smoking gun is "yes, the timer tick takes this lock."

2. **Do process-context acquirers disable interrupts?** Look for `interrupts::disable()` / `interrupts::enable()` (or whatever the kernel's IRQ-state API is) around the `inner.lock()` calls. If the answer is "no" and the answer to (1) is "yes," that's the bug.

3. **What lock type wraps `PerCpuScheduler.inner`?** Confirm it's plain `spinning_top::Spinlock` (or similar plain spinlock) and not a wrapped IRQ-safe variant. If it's wrapped, this hypothesis is refuted at step zero.

4. **Are there other locks taken in any same-CPU interrupt-context path?** Even if `PerCpuScheduler.inner` itself is fine, a related lock might have the same issue.

The audit's verdict matrix in §6 of the J1 dispatch can absorb the interrupt-context-specific finding under Verdict A (bug identified) if the audit confirms the hypothesis, or under Verdict B/C (candidates found, need J2 confirmation) if the audit finds the structural risk but can't conclusively name the firing path.

If the audit refutes this hypothesis (e.g., interrupts ARE disabled around every `inner.lock()`, or the lock is never taken from ISR context), the J1 dispatch's other candidates (cross-CPU lock-while-blocked, atomicity bug) move up. The audit is well-equipped to answer this question regardless of which hypothesis turns out to be right.

---

## 5. If the hypothesis holds: known fix patterns

If WEDGE-INV-J1 confirms interrupt-context lock-acquire as the mechanism, the fix has well-established forms. In rough order of invasiveness:

### 5.1 Local interrupt disable around acquire

Wrap each call to `inner.lock()` with `interrupts::disable()` before and `interrupts::enable()` after (or restore-prior-state pattern). Smallest delta. Risk: easy to miss a call site, and the discipline is not enforced by the type system.

```rust
// schematic
let _flags = interrupts::save_and_disable();
let mut guard = scheduler.inner.lock();
// ... critical section ...
drop(guard);
interrupts::restore(_flags);
```

### 5.2 Wrapper type (preferred)

Define a wrapper `IrqSafeSpinlock<T>` or `PerCpuLock<T>` that implements `lock()` to save IRQ state, disable interrupts, then call the underlying `spinning_top::Spinlock::lock()`. The guard's `Drop` restores IRQ state. This is structurally what Theseus's `irq_safety` crate provides. Type-system enforcement: any `lock()` on the wrapper is automatically IRQ-safe; misuse becomes hard.

### 5.3 Adopt `irq_safety` directly (or a similar crate)

Replace `spinning_top::Spinlock` with `irq_safety::MutexIrqSafe` for the per-CPU scheduler inner. Brings in the dependency but skips the in-house wrapper code. AltimitNeo would join Theseus in using a community-supported solution.

### 5.4 Restructure to never hold lock during interrupt-prone sections

Architectural fix: ensure no path that can be interrupted by a same-CPU ISR can hold `PerCpuScheduler.inner`. Hardest. Likely overkill.

The first three are well-understood patterns. Project lead's choice of fix family is an ADR-class decision (architectural impact: every lock acquire site touched). The fix dispatch design depends on which fix family is chosen.

---

## 6. Caveats

1. **The hypothesis is not proved.** This is research-grade speculation grounded in pattern-matching against literature precedents and the wedge symptoms. WEDGE-INV-J1's job is to verify or refute by direct inspection of AltimitNeo's code.
2. **Even if the hypothesis is correct, the specific firing path is not yet named.** "Some ISR takes `PerCpuScheduler.inner`" is the family; "the timer tick at line X of file Y takes the lock at offset Z relative to the BSP's outer acquire" is the mechanism. The audit names the mechanism.
3. **The fix is straightforward only if the hypothesis is correct.** If the audit refutes interrupt-context and the actual mechanism is cross-CPU lock-while-blocked or atomicity, the fix is different. Don't pre-commit.
4. **The substrate-shift-damping interpretation in §2.4 is consistent but not exclusive.** Other mechanism families could also produce timing-sensitive damping under instruction-cache changes. ADR-015 isn't a uniqueness test for the interrupt-context hypothesis.
5. **The literature cited is not exhaustive.** I focused on the highest-yield sources; there's more in the LWN archives, RustBelt papers, and individual kernel project docs that I didn't pull. If WEDGE-INV-J1 narrows but doesn't conclude, deeper research into specific Linux libsas/Zephyr/Hubris bugs of similar shape may be worth doing.

---

## 7. References (selected)

- Kladov, Aleksey. "Spinlocks Considered Harmful." Jan 2020. `matklad.github.io/2020/01/02/spinlocks-considered-harmful.html`. The bare-metal-with-interrupts deadlock warning.
- Linux kernel locking documentation. "Lock types and their rules." `docs.kernel.org/locking/locktypes.html`. The canonical taxonomy.
- Linux kernel locking documentation. "Unreliable Guide To Locking." `docs.kernel.org/kernel-hacking/locking.html`. The minimum-requirements table for source × destination context.
- Linux kernel commit history. `kernel.org/doc/Documentation/locking/spinlocks.txt`. Linus's own writeup on spin_lock_irq vs spin_lock — direct, pithy, foundational.
- Theseus OS `irq_safety` crate. `github.com/theseus-os/irq_safety`. Theseus's standardized solution.
- Rust-for-Linux `kernel::sync::SpinLock`. `rust-for-linux.github.io/docs/kernel/sync/struct.SpinLock.html`. The lock vs. lock_irqdisable API distinction.
- Linux Weekly News. "A new API for interrupt-aware spinlocks." `lwn.net/Articles/1039374/`. The 2025 active discussion in the Rust kernel community.
- Redox kernel design notes. `redox-os.org/news/kernel-10/`. The "interrupts disabled almost everywhere" baseline.
- `spinning_top` crate. `github.com/rust-osdev/spinning_top` and `docs.rs/spinning_top/`. The crate AltimitNeo uses; plain `Spinlock`, no IRQ variant.
- `spin` crate. `crates.io/crates/spin`. Companion crate with the same posture; links to Kladov's article in its README.

---

## 8. Cross-references (project-internal)

- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` — the prior research note this follows up on. The general framework (heisenbug literature, observation strategies, structural precedents); this note focuses on the now-known specific lock crate.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-J1-audit-cerebro.md` — the audit dispatch this research feeds into. §3.4 of the dispatch (schedule-tick / IPI handlers) should be elevated from best-effort to highest-priority based on this research; if the dispatch updates, the cross-reference here is to the updated version.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md` — the project-state framing for what's known/unknown. §2 of that note lists three plausible mechanism families; this research note refines "re-entrant deadlock" into the specific sub-case "interrupt-context same-CPU lock re-entrance" and identifies it as the highest-prior candidate.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10 — the timing-window-disruption story; §2.4 of this note provides a specific mechanism (interrupt-arrival timing relative to lock-hold window) for that story.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment. If the hypothesis confirms, §5 of this research note gives three concrete fix-family options for the eventual fix dispatch.

---

**End of research note.**

*The bug shape "BSP spins on CPU 0's per-CPU scheduler inner lock" plus the substrate "spinning_top with no IRQ-safe wrapper" plus the symptoms "timing-sensitive, heisenbug, four-signature" plus the literature "every Rust no_std kernel that's been at this longer has built irq-safety wrappers" point at one specific candidate. The candidate is the WEDGE-INV-J1 audit's first thing to check. If the audit confirms, fix dispatch follows; if the audit refutes, the next-strongest candidate moves up. Either way, this research narrows the search space.*
