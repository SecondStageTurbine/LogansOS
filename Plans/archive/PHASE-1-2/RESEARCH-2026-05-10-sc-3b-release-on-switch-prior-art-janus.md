# Research: SC-3B release-on-switch prior art

**Date:** 2026-05-10
**Author:** Janus research artifact
**Type:** Research artifact (informational; not normative). External prior-art survey for release-on-context-switch lock ownership, scheduler/IPI/IPC lock handoff, Rust guard interactions with asm context switches, and related deadlock/memory-ordering failure surfaces. Doc-only artifact; it does not propose code changes or project decisions.
**Authority:** None. PM consumes this artifact with Cerebro's Phase D lock-ownership findings and performs the synthesis.

---

## TL;DR

There is real prior art for carrying a lock across a context switch and releasing it from the post-switch execution context, but the pattern is usually narrow and scheduler-owned. Linux carries the runqueue lock across the low-level switch and has the incoming task drop it in `finish_task_switch()`. FreeBSD's historical scheduler lock is explicitly described as a special exception where ownership is passed from the switched-out thread to the switched-in thread. xv6 carries `p->lock` across `swtch()` to preserve process-state invariants. Zircon has the richest modern production source example: it stashes the previous thread before `arch_context_switch()` and performs a post-switch lock handoff that releases the previous thread's lock while keeping the new thread's lock. Redox's Rust kernel has a public guard-plus-finish-hook pattern where context guards are stored in per-CPU switch state and dropped by `switch_finish_hook()` after the arch switch.

No public source surveyed shows a general-purpose primitive that releases arbitrary N lock bytes directly in the asm switch sequence as a reusable kernel synchronization API. The closest production patterns either release after the switch in C/C++/Rust scheduler code, or treat the lock as a scheduler/thread ownership invariant rather than an arbitrary caller-owned guard. seL4's SMP big-kernel-lock material and a time-protection paper include a kernel-lock release after thread-context switch, but the public material consulted was not enough to classify it as an arbitrary release-on-switch primitive.

The most relevant bug-pattern evidence is around ownership accounting, lock ordering, and control-flow discipline rather than x86 store reordering. Linux special-cases lockdep and debug owner metadata for a lock released by a different task. Zircon source contains a detailed AB/BA deadlock discussion around scheduler/thread lock acquisition during reschedule and uses chain-lock transaction state to make the handoff auditable. Redox's code comments state that locals cannot be relied on after `arch::switch_to()` and that `switch_finish_hook()` is needed to release locks. I found no strong CVE/mailing-list record for a two-lock asm release asymmetry bug of the exact requested class.

## Sources surveyed

### Primary sources consulted directly

- Linux source, `kernel/sched/core.c`, `prepare_lock_switch()`, `finish_lock_switch()`, `finish_task_switch()`, and `finish_task()`: `https://android.googlesource.com/kernel/common/+/b584a17628b0bf4b04ceba066dd1a69c5f097276/kernel/sched/core.c`.
- Linux source browser for current `kernel/sched/core.c`, same functions and line anchors: `https://codebrowser.dev/linux/linux/kernel/sched/core.c.html`.
- Linux scheduler architecture notes, unlocked context-switch support: `https://static.lwn.net/kerneldoc/scheduler/sched-arch.html`.
- Linux locking docs, "Lock types and their rules": `https://www.kernel.org/doc/html/latest/locking/locktypes.html`.
- Linux lockdep design docs: `https://static.lwn.net/kerneldoc/locking/lockdep-design.html`.
- Linux memory-barrier docs: `https://docs.kernel.org/6.1/core-api/wrappers/memory-barriers.html`.
- FreeBSD `mi_switch(9)` manpage: `https://manpages.ubuntu.com/manpages/questing/man9/cpu_switch.9freebsd.html`.
- FreeBSD locking docs / `locking(9)`: `https://manpages.debian.org/unstable/freebsd-manpages/locking.9freebsd.en.html`.
- FreeBSD Architecture Handbook, SMPng and sleepqueue material: `https://docs.freebsd.org/en/books/arch-handbook/book/`.
- John H. Baldwin, "Locking in the Multithreaded FreeBSD Kernel," BSDCon 2002: `https://papers.freebsd.org/2002/jhb-locking_in_multithreaded_kernel.files/jhb-locking_in_multithreaded_kernel-paper.pdf`.
- seL4 IPC tutorial: `https://docs.sel4.systems/Tutorials/ipc.html`.
- seL4 release notes 6.0.0, ARM SMP big kernel lock and IPI caveats: `https://docs.sel4.systems/releases/sel4/6.0.0.html`.
- seL4 proof overview: `https://sel4.systems/Verification/proofs.html`.
- Gernot Heiser et al., "Time Protection: The Missing OS Abstraction": `https://sel4.systems/Research/pdfs/time-protection-missing-os-abstraction.pdf`.
- "Practical Rely/Guarantee Verification of an Efficient Lock for seL4 on Multicore Architectures": `https://arxiv.org/abs/2407.20559`.
- Zircon scheduler docs: `https://fuchsia.dev/fuchsia-src/concepts/kernel/kernel_scheduling`.
- Zircon scheduler source, `zircon/kernel/kernel/scheduler.cc`: `https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/zircon/kernel/kernel/scheduler.cc`.
- Zircon runtime lock validation docs: `https://fuchsia.dev/fuchsia-src/concepts/kernel/lockdep`.
- xv6 RISC-V book, scheduling / sleep-wakeup chapters: `https://pdos.csail.mit.edu/6.1810/2025/xv6/book-riscv-rev5.pdf`.
- xv6 RISC-V source, `kernel/proc.c`: `https://github.com/mit-pdos/xv6-riscv/blob/riscv/kernel/proc.c`.
- Theseus tasking subsystem docs: `https://www.theseus-os.com/Theseus/book/subsystems/task.html`.
- Theseus context-switch crate docs: `https://www.theseus-os.com/Theseus/doc/context_switch/index.html`.
- Theseus OSDI 2020 paper page: `https://www.usenix.org/conference/osdi20/presentation/boos`.
- Asterinas issue #2217, "SMP LTP hangs": `https://github.com/asterinas/asterinas/issues/2217`.
- Asterinas PR #2488, "Fix LTP CI hang problems": `https://github.com/asterinas/asterinas/pull/2488`.
- Asterinas book, framekernel architecture: `https://asterinas.github.io/book/kernel/the-framekernel-architecture.html`.
- Asterinas ATC 2025 paper page: `https://www.usenix.org/conference/atc25/presentation/peng-yuke`.
- Redox kernel source, `src/context/switch.rs`: `https://github.com/redox-os/kernel/blob/master/src/context/switch.rs`.
- Redox kernel docs: `https://doc.redox-os.org/book/kernel.html`.
- Tock book, syscall/context-switch interface: `https://book.tockos.org/doc/syscalls.html`.
- Tock scheduler API docs: `https://docs.tockos.org/kernel/scheduler/`.
- Intel Software Developer's Manual landing page: `https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html`.
- Intel SDM combined-volume page: `https://www.intel.com/content/www/us/en/content-details/916575/intel-64-and-ia-32-architectures-software-developer-s-manual-combined-volumes-1-2a-2b-2c-2d-3a-3b-3c-3d-and-4.html`.
- Sewell et al., "x86-TSO: A Rigorous and Usable Programmer's Model for x86 Multiprocessors": `https://cacm.acm.org/research/x86-tso-a-rigorous-and-usable-programmers-model-for-x86-multiprocessors/`.

### Secondary / community references

- OSDev discussion, "SMP and the lost + wake-up problem": `https://forum.osdev.org/viewtopic.php?t=40281`.
- Stack Overflow discussion of Linux runqueue-lock handoff: `https://stackoverflow.com/questions/33968165/why-schedule-does-not-lead-to-deadlock-while-using-the-default-prepare-arch-switch`.
- Stack Overflow discussion of xv6 scheduler lock handoff: `https://stackoverflow.com/questions/62337198/xv6-locking-of-process-table-in-scheduler`.
- Ruby issue #21612, "Make sure we never context switch while holding the VM lock": `https://bugs.ruby-lang.org/issues/21612`. This is not kernel prior art; it is included only as a non-kernel analogy for "context-switch while holding a global execution lock" hazards.

## Findings by query

### 2.1 Prior art for release-on-context-switch lock primitives

**Linux has a scheduler-owned lock handoff, not a general arbitrary-lock asm release.** In Linux `kernel/sched/core.c`, `prepare_lock_switch()` records that the runqueue lock will be released by the next task, performs an early lockdep release, and under `CONFIG_DEBUG_SPINLOCK` changes the debug owner to `next`. `finish_lock_switch()` later reacquires the lockdep map and calls `raw_spin_unlock_irq()`. `finish_task_switch()` is annotated as releasing `rq->lock` and is run after the stack flip. This is close prior art for "lock carried across switch and released in the switched-to context," but the low-level arch switch is not the component that directly stores to the lock byte.

**Linux documents an architecture option for unlocked context switches.** The scheduler-architecture documentation says architectures can request that `switch_to()` be called with the runqueue unlocked by defining `__ARCH_WANT_UNLOCKED_CTXSW`. That is useful negative evidence: Linux treats "locked switch" as a scheduler special case with architecture opt-out, not as a general-purpose caller-visible synchronization primitive.

**FreeBSD has historical scheduler-lock handoff.** The FreeBSD `mi_switch(9)` manpage says `mi_switch()`, `cpu_switch()`, and `cpu_throw()` must be called with the scheduler lock held. Baldwin's FreeBSD locking paper states that a context switch cannot normally be performed while holding a spin mutex, but the scheduler lock is an exception whose ownership is passed from the switched-out thread to the switched-in thread. This is a very direct conceptual match to release-on-switch, again scoped to scheduler state.

**xv6 deliberately carries `p->lock` across `swtch()`.** The xv6 RISC-V book and `proc.c` source show the process lock held while entering `sched()` and released after `swtch()` returns in scheduler code. The book explains the invariant: once a process begins changing state for yield/sleep, the lock must remain held until the process's registers are saved and the scheduler is running on its own stack. xv6 is an educational kernel, but this is primary source and highly relevant to the "handoff lock ownership across switch" pattern.

**Zircon has a production post-switch thread-lock handoff.** Zircon `scheduler.cc` releases the scheduler queue lock before the context switch, but carries thread locks across the switch boundary. It records `previous_thread_ = current_thread` immediately before `arch_context_switch()`. After the switch, it calls `Scheduler::LockHandoffInternal()`, which restores transaction state, synchronizes lock tokens, and releases the previous thread's lock. `TrampolineLockHandoff()` handles the first-run/trampoline case and explains that the current thread's lock was acquired during the context switch and must be released before letting the new thread run. This is the richest source surveyed for ownership metadata and post-switch handoff discipline.

**Redox has a Rust guard-plus-post-switch hook pattern.** Redox `src/context/switch.rs` stores `_prev_guard` and `_next_guard` in a `SwitchResultInner` placed in per-CPU switch state before calling `arch::switch_to()`. `switch_finish_hook()` later takes that stored object, drops it, and clears `CONTEXT_SWITCH_LOCK`. The source also warns not to call `switch()` while holding locks. This is not a generic lock-byte asm release, but it is a Rust kernel retaining RAII guards across an asm switch and arranging for their drop from a post-switch hook.

**seL4 public material points to a big-kernel-lock release-after-switch pattern, but with limited source-level detail in this pass.** seL4 6.0.0 release notes say ARM SMP used the big kernel lock and reserved SGIs for remote call and reschedule. The time-protection paper's kernel-exit sequence includes switching thread context and then releasing the kernel lock. I did not find, in the public material surveyed here, enough source-level detail to classify seL4 as using a caller-specified release-on-switch primitive. Treat it as single-source mechanism-adjacent evidence.

**Theseus, Asterinas, and Tock did not show public release-on-switch primitives in the docs/source snippets surveyed.** Theseus documents context switch crates and task cleanup via Rust ownership/drop, but I found no public guard handoff coupled to the asm switch. Asterinas public sources emphasize framekernel architecture and OSTD as the unsafe boundary; PR #2488 is about SMP hang fixes and runqueue IRQ-disabled locking, not a release-on-switch primitive. Tock documents architecture-specific userspace/kernel boundary switching and scheduler traits, but not a kernel-thread lock handoff across a scheduler asm switch.

### 2.2 Known bug patterns in lock-release-during-context-switch sequences

**No exact two-lock asm-release bug report was found.** Searches across Linux/LKML, FreeBSD docs, seL4, Zircon, Redox, Theseus, Asterinas, Tock, and OSDev did not surface a settled CVE, issue, or mailing-list thread whose root cause was "asm context switch released one of two lock bytes but not the other" or "lock release store executed on the wrong side of stack switch."

**Ownership metadata drift is a documented concern.** Linux's `prepare_lock_switch()` explicitly performs an early lockdep release and changes debug spinlock ownership to `next` because the runqueue lock is an invalid-but-intended scheduler special case. Zircon similarly has comments around static-analysis confusion after the stack swap: source variables no longer mean what the analyzer thinks, so the code uses explicit lock-handoff internals and marker operations. These are not bug reports, but they are primary-source evidence that ownership metadata is a load-bearing part of switch-coupled locking.

**AB/BA scheduler/thread-lock deadlock is documented in Zircon source.** Zircon's `Scheduler::RescheduleCommon()` contains a detailed scenario: CPU X has T0 and wants T1, CPU Y has T1 and wants T0 during owner-chain operations. The source comment says this is deadlock potential and explains why a special scheduler token/backoff discipline is required. This is the strongest public source for a cross-CPU lock-ordering failure surface close to the current research question.

**Release-hook control flow is a visible Redox risk surface.** Redox source says execution after `arch::switch_to()` may not resume at the same return address and that `switch_finish_hook()` is needed to release locks. This is not presented as a known bug in the source, but it is direct evidence for one of the dispatch's hypothesized failure classes: a release path must be reached through all arch-switch return/trampoline paths.

**FreeBSD's WITNESS and Linux lockdep exist because lock-order cycles are expected to be hard to audit manually.** FreeBSD locking docs and `locking(9)` describe WITNESS checking and sleep/lock interaction rules. Linux lockdep docs describe runtime validation of lock-order cycles. These sources do not identify release-on-switch as a special bug class; they classify the surrounding AB/BA and sleep-while-atomic failures as first-class kernel hazards.

**Memory-order bugs at this exact x86 switch boundary were not found.** I found general memory-ordering sources and lock-release rules, but no public report of a normal write-back x86 lock release store appearing after the new thread starts because of hardware reordering. The absence is a negative search result, not proof. The stronger public warning is to maintain acquire/release semantics and avoid compiler/control-flow/metadata mistakes.

### 2.3 The guard type interacting with asm context switching in Rust kernels

**Redox is the strongest Rust-kernel match.** Redox keeps Rust write guards alive across `arch::switch_to()` by moving them into per-CPU `switch_result` storage. The guards are dropped by `switch_finish_hook()`, not by normal lexical scope after the call site. The code uses unsafe lifetime manipulation around `prev_context` and `next_context`, then relies on the hook because local variables after `switch_to()` cannot be trusted to resume normally.

**Theseus uses Rust ownership heavily, but no release-on-switch guard idiom was found.** Theseus's book emphasizes affine types, Rust task abstractions, stack unwinding, and cleanup through `Drop`. Its `context_switch` crate wraps architecture-specific switch implementations. In the docs/source snippets consulted, context switch functions take stack-pointer arguments and do not expose a guard/lock release token API.

**Asterinas/OSTD public material did not surface a mem::forget-style switch-lock release.** The framekernel literature says unsafe code is confined to OSTD and the rest of the kernel is safe Rust, and the ATC 2025 paper positions `Task` and related abstractions as safe OSTD mechanisms. PR #2488's SMP hang fixes concern runqueue locking with IRQs disabled and a missing wake after child movement; it does not document a guard-destructor-vs-asm-switch handoff pattern.

**Tock's model is different.** Tock is embedded, process-oriented, and documents context switches through an architecture-specific `UserspaceKernelBoundary` trait. Its scheduler APIs and syscall docs do not show kernel-thread RAII guards crossing an asm scheduler switch. Tock's Rust type system is used heavily for capsule/kernel boundaries, but this pass found no public evidence of a release-on-switch guard type.

**General Rust pitfall: lexical Drop is not enough if the call may not resume normally.** Redox's `switch_finish_hook()` is the concrete source-backed example. Theseus's cleanup/unwinding docs show the opposite tendency: prefer ordinary ownership/drop when control flow remains compatible with Rust's model. No academic source found here treats "Rust `MutexGuard` moved across asm context switch and later dropped by another logical task" as a named pattern.

### 2.4 Releasing multiple locks atomically with a context switch

**No direct N-lock asm release primitive was found.** Linux, FreeBSD, xv6, Zircon, and Redox all have switch-boundary lock handoff examples, but none of the public sources surveyed exposes an arbitrary lock-list release API in the low-level asm switch. Redox carries two context guards, but they are scheduler context guards, dropped as a stored Rust object in a finish hook. Zircon carries previous/current thread locks and restores transaction state; it does not present this as a generic N-lock release helper.

**Zircon's two-lock handoff is the closest production source.** The `TrampolineLockHandoff()` comments state that two locks are held: the previous thread's lock and the current/new thread's lock. `LockHandoffInternal()` then releases the previous thread's lock while keeping transaction state coherent. This is not "release both locks atomically"; it is a disciplined ownership handoff with explicit transaction bookkeeping.

**Linux and FreeBSD emphasize release order through lock-order validation rather than atomic multi-release.** Linux lockdep validates acquisition-order cycles. FreeBSD WITNESS validates lock-order and sleep/lock interactions. Neither source presents "acquired-last-released-first at context switch" as the central rule; the central rule is that lock acquisition graphs must not contain cycles and that blocking/sleeping with unsleepable locks is forbidden except for scheduler-specific exceptions.

**seL4's big kernel lock is a single-lock pattern in public docs.** The surveyed seL4 SMP docs discuss a big kernel lock, remote-call IPIs, and reschedule IPIs. They do not show an N>1 lock release at switch time.

**RTOS/research-kernel search result was mostly negative.** Searches over RT-Linux/NOVA/Composite/Barrelfish-style terms did not produce a strong public hit for arbitrary multi-lock release in the asm switch path. This may reflect terminology mismatch; "lock handoff," "scheduler lock," "runqueue lock," and "thread lock" were more productive than "release-on-context-switch."

### 2.5 Bible 8.5.4 invariant in real kernels

**Linux enforces the rule through lock categories, preemption state, `might_sleep()`, and lockdep.** The Linux locktypes doc classifies sleeping locks, CPU-local locks, and spinning locks, and states that sleeping locks cannot nest inside CPU-local/spinning lock categories. Lockdep validates lock-order cycles and context rules. The Linux scheduler runqueue lock handoff is an explicit scheduler exception, not a general license to block while holding spinlocks.

**FreeBSD enforces the rule with WITNESS and sleep-specific APIs.** FreeBSD locking docs state that sleep includes condition-variable waits and sleep functions, and list which lock categories may be held across sleep. They also list APIs such as `mtx_sleep()`, `rw_sleep()`, and `msleep_spin()` that atomically release a primitive while going to sleep and reacquire as needed. Baldwin's paper states that spin mutexes normally cannot be held across context switches, with the scheduler lock as the special exception.

**xv6 teaches the same rule by exception.** xv6 forbids sleeping with arbitrary locks but has `sleep(chan, lk)` acquire `p->lock`, release `lk`, set the sleep state, and call `sched()`. The special lock held across `swtch()` is the process lock, because it protects the process-state invariant that makes wakeup safe.

**seL4 reduces the surface differently.** seL4's public model is synchronous endpoint IPC, a small verified kernel, and an SMP big kernel lock in the release notes surveyed. The proof overview describes functional correctness and refinement guarantees, but this pass did not find a source specifically naming "no blocking while holding spinlock" as a proof theorem. The practical effect is architectural: kernel execution is constrained and the IPC fastpath has strict eligibility conditions.

**Rust kernels surveyed rely on different mixtures of type discipline, runtime locks, and convention.** Theseus leans on ownership/drop and task-state abstractions. Asterinas/OSTD confines unsafe to the OS framework and had public SMP hang fixes around runqueue locking and IRQ-disabled discipline. Redox uses `CleanLockToken`, explicit warnings around `switch()`, and a switch finish hook. Tock uses Rust capabilities and architecture-specific boundary traits; no spinlock sleep invariant comparable to Linux lockdep was found in the brief public-doc survey.

**The common alternatives to release-on-switch are well represented.** They are: atomically release-and-sleep APIs (`mtx_sleep`, xv6 `sleep(chan, lk)`), scheduler-owned lock handoff (Linux, FreeBSD, xv6, Zircon), big-kernel-lock serialization (seL4 SMP public docs), and explicit lock-order validators (Linux lockdep, FreeBSD WITNESS, Zircon lockdep).

### 2.6 Cross-CPU lock acquisition orderings under IPC stress

**Zircon is the most detailed cross-CPU lock-order source.** Its scheduler code uses queue locks, thread locks, chain-lock transactions, scheduler tokens, and explicit backoff reasoning. The source's AB/BA scenario is not abstract: one CPU enters reschedule holding one thread lock and wants another, while another CPU performs an owner-chain operation in the opposite order. Zircon lockdep docs describe runtime detection of cycles, including multi-thread cycles.

**Linux avoids many runqueue AB/BA cases through explicit runqueue locking helpers and migration protocols.** Linux scheduler source contains runqueue locking, task migration, and wakeup paths that acquire task and runqueue locks in disciplined sequences. The first-pass artifact already covered `ttwu_queue_wakelist()` and wake-on-target-CPU activation. For this second pass, the relevant addition is that runqueue lock handoff is lockdep-special-cased and that lockdep tracks cycles across held locks.

**FreeBSD documents lock order and sleepqueue interlocks.** The SMPng handbook describes sleepqueue locking and interlock release before blocking. FreeBSD WITNESS tracks lock order and reports violations. This is mechanism-adjacent to IPC stress even though FreeBSD's public docs surveyed here do not give a modern endpoint-IPC AB/BA bug matching SC-3B.

**seL4 endpoint IPC public docs do not expose a fine-grained lock graph.** The IPC tutorial describes endpoint queues of waiting senders/receivers and fastpath constraints. The SMP release notes describe big-kernel-lock operation. In the material surveyed, there is no public fine-grained channel-lock/scheduler-lock acquisition order comparable to Zircon's chain locks.

**Asterinas PR #2488 remains close mechanism-adjacent Rust-kernel evidence.** The PR notes that runqueues needed to be locked with IRQ disabled to avoid deadlock in IRQ callbacks registered by `enable_preemption_on_cpu`, and that init needed to be woken after dead children were moved. This is not an IPC lock-cycle source, but it is a public Rust-kernel SMP hang fix involving scheduler/runqueue lock discipline and missed wake semantics.

### 2.7 Memory ordering at the context-switch boundary

**x86 TSO makes ordinary release stores stronger than on weakly ordered ISAs, but not magic.** Intel SDM memory-ordering material and the x86-TSO paper agree on the broad model: writes are strongly ordered relative to other writes, and locked instructions have stronger ordering. Linux memory-barrier docs still model lock release with explicit release semantics because the kernel must be portable and compiler ordering matters.

**Linux has an explicit release before dropping scheduler ownership.** In `finish_task()`, Linux uses `smp_store_release(&prev->on_cpu, 0)` before the runqueue lock is dropped in the finish-switch path. The comment says other CPUs must not move the task until the switch is completely finished. This is important prior art because it separates "stack/register switch has occurred" from "other CPUs may now treat the old task as movable."

**Plain `MOV` release can be valid on x86 for normal write-back memory, but source discipline still matters.** Linux and x86 sources support the usual conclusion that a simple store can implement release semantics on x86 for ordinary memory, while locked instructions provide full ordering when needed. However, this does not address all switch-boundary hazards: the release store still must be executed on every intended control-flow path, target the correct lock byte, and be paired with acquire-side code that observes the protected data correctly.

**No public source showed a normal x86 context-switch release store being observed out of program order relative to the new thread starting.** The negative result is narrow. Store-buffer visibility can delay other CPUs observing a release, and weakly ordered architectures need stronger discipline, but the surveyed sources did not support a claim that x86 hardware reorders a normal lock-release store past later stores in a way that would explain a held lock after the release instruction executed.

**The more credible source-backed risk is mismatch between logical ownership and actual lock state.** Linux's lockdep/debug-owner special cases, Zircon's transaction restoration and analyzer comments, and Redox's finish hook all point at the same class: after a stack switch, ordinary lexical reasoning breaks down. The critical facts become "which lock is still held," "which logical task owns it," and "which post-switch path is responsible for releasing it."

## Conflicts and contested claims

- **"Release-on-switch" can mean several different things.** Linux/FreeBSD/xv6/Zircon are strong matches for lock handoff across context switch, but weak matches for "asm helper directly stores zero into arbitrary lock bytes before resuming the new thread." Redox is a stronger Rust guard example, but its guards are released by a finish hook, not by the asm sequence itself.
- **Linux is both a positive and negative example.** It carries `rq->lock` across the switch, but it does so as a scheduler special case with lockdep/debug-owner fixups. It also documents an architecture option to switch with the runqueue unlocked.
- **FreeBSD paper vs modern FreeBSD implementation.** The 2002 paper is explicit about scheduler-lock ownership transfer, but it reflects the SMPng-era scheduler lock. It is high-quality historical primary material, not a guarantee that every modern FreeBSD scheduler path still has the same implementation shape.
- **seL4 source confidence is lower than Linux/Zircon/Redox.** Public docs and the time-protection paper indicate a kernel-lock release after thread-context switch, but I did not trace current seL4 source in this pass to the same depth as Zircon or Linux.
- **Educational kernels are relevant but not production evidence.** xv6 is the clearest explanation of why a lock is held across switch, but it is intentionally small and pedagogical.
- **No exact bug report is not exonerating.** The absence of a public two-lock asm-release bug report may mean the pattern is rare, not safe.

## Single-source findings (low confidence)

- The seL4 "switch thread context, then release kernel lock" sequence comes from the time-protection paper in this pass. It should be source-checked in current seL4 code before being treated as implementation fact.
- Redox's post-switch guard-drop behavior is based on current public `switch.rs` source and a brief line-level read. It should be checked against the architecture-specific `arch::switch_to()` implementation for each supported architecture before drawing stronger conclusions.
- The negative finding for Theseus/Asterinas/Tock release-on-switch guard primitives is based on docs, public issue/PR material, and targeted searches, not a full source audit.
- No public N>1 asm lock-release helper was found. This is a search result, not proof that none exists in an RTOS or research kernel under different terminology.
- Ruby issue #21612 is included only as a non-kernel analogy. It should not be used as operating-system prior art.

## Recommended next steps for the research thread

- Read Zircon `scheduler.cc` together with Zircon's `thread_lock` / `chain_lock_transaction` implementation and lockdep docs. This is the highest-value public source chain for switch-boundary lock ownership metadata.
- Read Redox `src/context/switch.rs` together with each architecture's `arch::switch_to()` and `switch_finish_hook()` entry path. This is the highest-value Rust-specific guard-crossing-asm source chain.
- Source-check current seL4 SMP kernel-lock acquire/release paths around kernel entry/exit and context switch, because the paper evidence is suggestive but not source-complete.
- Compare Linux historical and current scheduler code around `prepare_lock_switch()`, `finish_lock_switch()`, and `__ARCH_WANT_UNLOCKED_CTXSW` to separate "lock handoff by policy" from "asm release by mechanism."
- If pursuing academic bibliography, follow Zircon lockdep/chain-lock references and seL4 multicore lock verification (`arXiv:2407.20559`) before broader RTOS searches; those are closer to this mechanism than generic spinlock papers.

## References

- Linux `kernel/sched/core.c`, Android common mirror. `https://android.googlesource.com/kernel/common/+/b584a17628b0bf4b04ceba066dd1a69c5f097276/kernel/sched/core.c`
- Linux current source browser, `kernel/sched/core.c`. `https://codebrowser.dev/linux/linux/kernel/sched/core.c.html`
- Linux scheduler architecture notes. `https://static.lwn.net/kerneldoc/scheduler/sched-arch.html`
- Linux docs, "Lock types and their rules." `https://www.kernel.org/doc/html/latest/locking/locktypes.html`
- Linux docs, "Runtime locking correctness validator." `https://static.lwn.net/kerneldoc/locking/lockdep-design.html`
- Linux docs, "Linux kernel memory barriers." `https://docs.kernel.org/6.1/core-api/wrappers/memory-barriers.html`
- FreeBSD `mi_switch(9)` manpage. `https://manpages.ubuntu.com/manpages/questing/man9/cpu_switch.9freebsd.html`
- FreeBSD `locking(9)` manpage. `https://manpages.debian.org/unstable/freebsd-manpages/locking.9freebsd.en.html`
- FreeBSD Architecture Handbook. `https://docs.freebsd.org/en/books/arch-handbook/book/`
- John H. Baldwin, "Locking in the Multithreaded FreeBSD Kernel." `https://papers.freebsd.org/2002/jhb-locking_in_multithreaded_kernel.files/jhb-locking_in_multithreaded_kernel-paper.pdf`
- seL4 IPC tutorial. `https://docs.sel4.systems/Tutorials/ipc.html`
- seL4 6.0.0 release notes. `https://docs.sel4.systems/releases/sel4/6.0.0.html`
- seL4 proof overview. `https://sel4.systems/Verification/proofs.html`
- "Time Protection: The Missing OS Abstraction." `https://sel4.systems/Research/pdfs/time-protection-missing-os-abstraction.pdf`
- "Practical Rely/Guarantee Verification of an Efficient Lock for seL4 on Multicore Architectures." `https://arxiv.org/abs/2407.20559`
- Zircon scheduler docs. `https://fuchsia.dev/fuchsia-src/concepts/kernel/kernel_scheduling`
- Zircon scheduler source. `https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/zircon/kernel/kernel/scheduler.cc`
- Zircon runtime lock validation docs. `https://fuchsia.dev/fuchsia-src/concepts/kernel/lockdep`
- xv6 RISC-V book. `https://pdos.csail.mit.edu/6.1810/2025/xv6/book-riscv-rev5.pdf`
- xv6 RISC-V `proc.c`. `https://github.com/mit-pdos/xv6-riscv/blob/riscv/kernel/proc.c`
- Theseus tasking docs. `https://www.theseus-os.com/Theseus/book/subsystems/task.html`
- Theseus context-switch crate docs. `https://www.theseus-os.com/Theseus/doc/context_switch/index.html`
- Kevin Boos et al., "Theseus: an Experiment in Operating System Structure and State Management," OSDI 2020. `https://www.usenix.org/conference/osdi20/presentation/boos`
- Asterinas issue #2217. `https://github.com/asterinas/asterinas/issues/2217`
- Asterinas PR #2488. `https://github.com/asterinas/asterinas/pull/2488`
- Asterinas framekernel architecture docs. `https://asterinas.github.io/book/kernel/the-framekernel-architecture.html`
- Yuke Peng et al., "ASTERINAS: A Linux ABI-Compatible, Rust-Based Framekernel OS with a Small and Sound TCB," USENIX ATC 2025. `https://www.usenix.org/conference/atc25/presentation/peng-yuke`
- Redox kernel `switch.rs`. `https://github.com/redox-os/kernel/blob/master/src/context/switch.rs`
- Redox kernel docs. `https://doc.redox-os.org/book/kernel.html`
- Tock syscall/context-switch docs. `https://book.tockos.org/doc/syscalls.html`
- Tock scheduler API docs. `https://docs.tockos.org/kernel/scheduler/`
- Intel SDM landing page. `https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html`
- Intel SDM combined volumes page. `https://www.intel.com/content/www/us/en/content-details/916575/intel-64-and-ia-32-architectures-software-developer-s-manual-combined-volumes-1-2a-2b-2c-2d-and-4.html`
- Sewell et al., "x86-TSO: A Rigorous and Usable Programmer's Model for x86 Multiprocessors." `https://cacm.acm.org/research/x86-tso-a-rigorous-and-usable-programmers-model-for-x86-multiprocessors/`
- OSDev forum, "SMP and the lost + wake-up problem." `https://forum.osdev.org/viewtopic.php?t=40281`
- Stack Overflow, Linux runqueue-lock handoff discussion. `https://stackoverflow.com/questions/33968165/why-schedule-does-not-lead-to-deadlock-while-using-the-default-prepare-arch-switch`
- Stack Overflow, xv6 scheduler lock handoff discussion. `https://stackoverflow.com/questions/62337198/xv6-locking-of-process-table-in-scheduler`
- Ruby issue #21612. `https://bugs.ruby-lang.org/issues/21612`

---

**End of research artifact.**

*The strongest public prior art says release-on-switch exists as a tightly scoped scheduler/thread-lock handoff discipline, while arbitrary multi-lock asm release appears rare enough that ownership metadata, post-switch control flow, and lock-order validation are better-supported research surfaces than exact bug-report matches.*
