# Research: SC-3B silent-timeout prior art

**Date:** 2026-05-09
**Author:** Janus research artifact
**Type:** Research artifact (informational; not normative). External prior-art survey for the SC-3B silent-timeout recurrence pattern under QEMU SMP. Doc-only artifact; it does not propose code changes or project decisions.
**Authority:** None. PM consumes this artifact with the Cerebro NMI-capture artifact and performs the synthesis.

---

## TL;DR

No exact public match was found for "cross-CPU IPC stress under QEMU SMP, low-single-digit-percent silent timeout, no panic, no final terminal frame." The closest high-value prior art is the 2023 Linux/QEMU `printk.time` rare boot-hang regression: a QEMU-visible, low-rate, timing-sensitive kernel hang that required hundreds to hundreds of thousands of iterations to classify and was ultimately fixed in guest-kernel timer setup, not in QEMU. The broader evidence supports treating host-load/QEMU scheduling as a timing amplifier and sampling problem, not as proof that the bug is a QEMU artifact. Mechanism-adjacent findings are stronger: Linux, FreeBSD, OSDev, and Asterinas all document that wake/sleep boundaries, runqueue locks, interrupt masking, and remote wake placement are recurrent SMP footguns.

## Sources surveyed

### Primary sources consulted directly

- QEMU issue: "Linux kernel hangs rarely when booting on the latest qemu," `https://gitlab.com/qemu-project/qemu/-/issues/1696`.
- LKML mirror: "printk.time causes rare kernel boot hangs," `https://lkml.org/lkml/2023/6/13/733`.
- Linux fix commit: `13bb06f8dd42071cb9a49f6e21099eea05d4b856`, "tick/common: Align tick period during sched_timer setup," `https://git.kernel.org/pub/scm/linux/kernel/git/tip/tip.git/commit/?id=13bb06f8dd42071cb9a49f6e21099eea05d4b856`.
- Linux source reference: `kernel/sched/core.c`, `wake_q`, `resched_curr`, and `ttwu_queue_wakelist`, `https://code.googlesource.com/linux/torvalds/linux/+/9c70b2a33cd2aa6a5a59c5523ef053bd42265209/kernel/sched/core.c`.
- Linux lockup-watchdog docs, `https://www.kernel.org/doc/html/latest/admin-guide/lockup-watchdogs.html`.
- QEMU MTTCG docs, `https://www.qemu.org/docs/master/devel/multi-thread-tcg.html`.
- QEMU replay / nondeterminism docs, `https://www.qemu.org/docs/master/devel/replay.html`.
- QEMU monitor docs for `nmi`, `https://www.qemu.org/docs/master/system/monitor.html`.
- Red Hat RHEL 10 virtualization soft-lockup guidance, `https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/10/html/managing_monitoring_and_updating_the_kernel/keeping-kernel-panic-parameters-disabled-in-virtualized-environments`.
- QEMU-devel / Launchpad mirror: "multi-core FreeBSD guest hangs after warm reboot," `https://lists.gnu.org/archive/html/qemu-devel/2016-10/msg03983.html`.
- FreeBSD Architecture Handbook, SMPng design, `https://docs.freebsd.org/en/books/arch-handbook/smp/`.
- OSDev forum, "SMP and the lost + wake-up problem," `https://forum.osdev.org/viewtopic.php?t=40281`.
- Asterinas issue #2217, "SMP LTP hangs," `https://github.com/asterinas/asterinas/issues/2217`.
- Asterinas issue #2152, "LTP test kill02/kill08 broken on SMP," `https://github.com/asterinas/asterinas/issues/2152`.
- Asterinas PR #2488, "Fix LTP CI hang problems," `https://github.com/asterinas/asterinas/pull/2488`.
- Asterinas issue #2851, "`fcntl(F_SETOWN)` may hang indefinitely under concurrent forked calls," `https://github.com/asterinas/asterinas/issues/2851`.
- seL4 IPC docs, `https://docs.sel4.systems/Tutorials/ipc.html`.
- seL4 multikernel IPI RFC 0170, `https://sel4.github.io/rfcs/implemented/0170-multikernel-ipi-api.html`.
- seL4 6.0.0 SMP release notes, `https://docs.sel4.systems/releases/sel4/6.0.0.html`.
- Fuchsia Zircon scheduling docs, `https://fuchsia.dev/docs/concepts/kernel/kernel_scheduling`.
- Fuchsia Zircon channel docs, `https://fuchsia.dev/docs/reference/kernel_objects/channel`.
- Theseus repository / project description, `https://github.com/theseus-os/Theseus`.
- Theseus tasking docs, `https://www.theseus-os.com/Theseus/book/subsystems/task.html`.
- Theseus OSDI 2020 paper page, `https://www.usenix.org/conference/osdi20/presentation/boos`.
- Asterinas USENIX ATC 2025 paper page, `https://www.usenix.org/conference/atc25/presentation/peng-yuke`.

### Secondary / community references

- OSDev QEMU/GDB and APIC wiki pages were consulted as community background: `https://wiki.osdev.org/QEMU`, `https://wiki.osdev.org/GDB`, `https://wiki.osdev.org/APIC`.
- A few Reddit search results were inspected only as weak signal. No Reddit thread is used as a load-bearing citation here.

## Findings by query

### 1.1 Failure-pattern findings

**Closest direct failure-pattern match: Linux/QEMU rare boot hang.** QEMU issue #1696 describes Fedora Linux guest kernels rarely hanging during boot on QEMU 8.0.0, stopping after "Freeing SMP alternatives memory" before the next `smpboot` line. The initial estimate was about 1 in 300 boots; one attached run failed on iteration 260; another failure required thousands of iterations; the reporter later ran 292,612 successful iterations on a pre-regression commit before treating the regression as absent there. This is not IPC stress, and `-smp 2` reportedly made no difference, but it is a close public example of low-rate, QEMU-visible, silent-ish kernel progress failure that reproduces unevenly across windows.

**The Linux/QEMU case ended as a guest-kernel timer bug, not a QEMU bug.** The QEMU issue was closed after kernel commit `13bb06f8dd42071cb9a49f6e21099eea05d4b856`, "tick/common: Align tick period during sched_timer setup." The fix rationale says an early tick-period alignment could leave too little time for boot progress, causing the system to hang. This is important for our case because QEMU was the reproduction substrate, but the accepted fix was in the guest kernel.

**There is older FreeBSD/QEMU multicore hang prior art.** A QEMU-devel mirror of Launchpad bug 1329956 reports multi-core FreeBSD guests hanging after warm reboot during SMP initialization on some Linux KVM hosts. It reproduced with `-smp 2` and was CPU-model dependent across host machines. The thread is older and resolution details are weak ("fixed somewhere with the last kernel or QEMU releases"), so it is useful only as a single-source pattern match for "multi-core guest hangs during SMP init under QEMU/KVM, host-dependent."

**Virtualization documentation supports host-load timing as a confounder.** Red Hat's virtualization guidance explicitly warns that host workload or resource contention can schedule out a guest CPU long enough to trigger guest lockup detection falsely. QEMU's replay documentation lists QEMU thread scheduling and host clocks as nondeterministic inputs. This does not prove our bug is a QEMU artifact; it does justify treating host load and vCPU scheduling as variables that can change whether a narrow race is sampled.

**No strong FreeBSD, seL4, Theseus, or Zircon report matching our exact IPC-stress signature was found.** FreeBSD and seL4 sources were more useful for architecture and synchronization posture than for comparable bug reports. Asterinas did have recent SMP hang issues, discussed below, but not a channel/IPC stress report of the exact SC-3B shape.

### 1.2 Mechanism findings

**Linux has a wake-on-target-CPU pattern, but not the same as our Q4 wording.** In `kernel/sched/core.c`, Linux's `__ttwu_queue_wakelist()` queues a task on the target CPU's wake list; comments state that the wakee CPU receives the IPI and performs activation so the wakee pays the wakeup cost. This is not "wake on completer CPU" as a named primitive, but it is an established pattern where remote wake work is intentionally shifted to the CPU where the task will run.

**Linux `wake_q` encodes "ready to be woken here" as a hard contract.** The `wake_q_add()` comments say callers must use it as if calling `wake_up_process()` at that location, because wakeup can happen instantly rather than later. `wake_up_q()` relies on `wake_up_process()` as a barrier pairing with queueing. The lesson is not a fix proposal; it is a mechanism reference: deferred wake queues are safe only when the queued task is already in a state where an immediate wake is legal.

**Asterinas has a direct modern Rust-kernel SMP hang precedent.** Asterinas issue #2217 says many SMP LTP CI failures were hangs that could happen "almost everywhere" and warned that the project was still far from SMP correctness at that point. PR #2488 later tied the long-standing LTP SMP hangs to two concrete problems: run queues needed locking with IRQs disabled to avoid deadlock in IRQ callbacks, and an init process was not woken after dead children were moved, so reaping could never complete. This is one of the closest mechanism-adjacent matches because it combines SMP hangs, scheduler/runqueue locking, interrupt context, and a missing wake/reap condition in a Rust OS kernel.

**Asterinas also has current low-level concurrency hang reports outside scheduler IPC.** Issue #2851 reports concurrent forked `fcntl(F_SETOWN)` calls causing child processes to hang inside the kernel while the parent waits forever. This is not our mechanism, but it confirms that Asterinas's public tracker contains modern examples of indefinite kernel hangs triggered by concurrent syscall stress.

**OSDev has a direct discussion of the SMP lost-wakeup boundary.** The 2021 OSDev thread frames the classic check-then-sleep gap: a condition can change after a thread checks it but before the thread is put to sleep. The thread discusses spinlocks with interrupts disabled, futex-style wait/wake contracts, and atomic "sleep plus unlock" primitives. This is community forum material, not authority, but it directly matches the H1-variant family.

**FreeBSD's SMPng document independently supports the same discipline.** FreeBSD's architecture handbook states that if a lock is needed to check state and act on that state, dropping the lock before the action can invalidate the decision. Its sleep queue design locks a sleepqueue chain before adding a thread and requires explicit interlock handling before blocking. This is a primary-source architecture document and a useful cross-check against the OSDev discussion.

**No public source was found for per-pair direction-locking causing migration accumulation under wake-on-completer semantics.** Searches against Linux/LKML, OSDev, seL4, Theseus, Asterinas, Redox, and QEMU did not surface that pattern as a named or documented stress-test bias. Treat Hn's "direction-locking accumulation" as project-local unless a later source turns up.

**No public source was found naming asymmetric yield-vs-no-yield IPC completion paths as a known footgun.** Related sources discuss wake/sleep atomicity, interrupt-safe runqueue locks, and target-CPU activation, but not this exact sibling-path asymmetry.

### 1.3 Diagnostic-pattern findings

**The Linux hardlockup/softlockup watchdog is the closest documented NMI watchdog pattern.** The Linux docs define soft lockups as long kernel-mode loops without scheduling and hard lockups as CPUs not taking interrupts. `watchdog_thresh` trades detection latency against overhead; the hardlockup detector can use periodic NMI/perf events. This is relevant for threshold tuning and false positive analysis, not as a drop-in mechanism.

**QEMU exposes host-triggered NMI injection.** The QEMU monitor `nmi` command injects an NMI on the default CPU for x86/s390, and on all CPUs for ppc64. This matches the general detect-then-instrument pattern used in prior AltimitNeo research and provides a documented host-side trigger for in-guest state dumps.

**Virtualization can produce false lockup signals.** Red Hat's RHEL 10 guidance says soft lockups in guests can be false when host contention schedules out the guest CPU and later causes a guest time jump. This argues for interpreting watchdog output carefully under QEMU/KVM: a watchdog firing may be evidence of a real guest wedge, a host scheduling artifact, or both interacting.

**The Linux/QEMU rare boot-hang investigation is a good closure-process precedent.** Richard W.M. Jones did not treat short no-failure windows as closure; he used many-iteration loops, bisected, tested candidate reverts, and documented that an apparent culprit merely reduced manifestation rate before a stronger bisect was completed. This is directly relevant to "low-rate intermittent bug observed but unsamplable in some windows."

### 1.4 Architectural-pattern findings

**Linux scheduler wake architecture:** Linux uses `wake_q` for batching/deferred wakeups, `resched_curr()` for cross-CPU reschedule IPIs, and `ttwu_queue_wakelist()` to offload activation to a target CPU in selected cases. The architecture deliberately separates "make task ready," "choose/validate CPU," "queue," and "reschedule target CPU."

**FreeBSD SMP architecture:** FreeBSD SMPng emphasizes lock coverage across condition checks and actions, sleep queues protected by spin mutexes, and explicit sleep/wakeup interlock design. This is a strong architectural source for lost-wakeup avoidance.

**seL4 architecture:** seL4 IPC documentation describes synchronous endpoint-based IPC with a fastpath constrained by small messages, no cap transfer, valid address spaces, and no higher-priority runnable thread. Its SMP release notes state older ARM SMP used a big kernel lock and reserved per-core SGIs for remote call and reschedule operations. The implemented multikernel IPI RFC treats IPIs as user-visible asynchronous signals between cores in partitioned multikernel configurations. No seL4 source found here describes an intermittent QEMU SMP IPC hang of our shape.

**Fuchsia/Zircon architecture:** Zircon has per-CPU schedulers that coordinate with IPIs. When a blocked thread becomes runnable, the scheduler reevaluates CPU choice and may move it. Channels are bidirectional message queues; `zx_channel_call()` sends and waits for a response using transaction IDs. Public docs did not surface a channel handoff hang matching SC-3B.

**Theseus architecture:** Theseus is an intralingual Rust OS that treats tasks as threads in a single address space and uses timer interrupts on each CPU for preemptive scheduling. Theseus sources surveyed did not surface a public SMP IPC hang report matching this signature.

**Asterinas architecture and issues:** The ATC 2025 paper page positions Asterinas as a Linux ABI-compatible Rust framekernel. Its tracker has the strongest modern Rust-kernel public prior art for SMP hangs: LTP SMP hangs, runqueue IRQ-disabled lock discipline, and missing wake after child-state movement.

## Conflicts and contested claims

- **"QEMU artifact" vs "guest kernel bug" is contested by evidence, not by source disagreement.** The Linux/QEMU 2023 case looked like a QEMU reproduction problem but ended as a guest Linux timer bug. Red Hat's VM guidance also says host scheduling can cause false lockup reports. Together these imply: QEMU/host scheduling can be a reproducer, amplifier, and false-positive source; it is not enough to classify a bug as "only QEMU."
- **Short clean windows are not strong evidence of absence.** The Linux/QEMU issue has examples where reverts changed the apparent rate from hundreds to thousands of iterations, and where a pre-regression commit was tested to 292,612 successful boots. This conflicts with any closure posture that treats 50/50 clean as decisive for a historically low-rate bug.
- **Forum-level OSDev advice is directionally aligned with FreeBSD/Linux but not authoritative.** OSDev contributors discuss spinlocks, interrupt disable, futexes, and sleep+unlock atomicity in terms that match primary-source designs, but individual posts should not be cited as community consensus.
- **Asterinas is close in substrate and symptom but not in exact workload.** Asterinas #2217/#2488 are strong for Rust-kernel SMP hangs and scheduler wake/runqueue discipline. They are weak for cross-CPU IPC/channel rendezvous specifically.

## Single-source findings (low confidence)

- The FreeBSD multi-core QEMU warm-reboot hang comes from a QEMU-devel/Launchpad mirror and lacks a precise fixing commit in the source consulted. Keep it as a low-confidence pattern match.
- Asterinas issue #2851 is a single open issue report for concurrent `fcntl(F_SETOWN)` hangs. Useful as evidence of current concurrent-syscall hang reports, not as a settled mechanism.
- OSDev's "SMP and lost wake-up" thread is one forum discussion. The pattern is real, but claims about specific implementation recipes should be downgraded unless backed by FreeBSD/Linux source.
- The absence of per-pair direction-locking prior art is a negative search result, not proof none exists.

## Recommended next steps for our project

- PM should spot-check QEMU issue #1696, LKML `printk.time` thread, and Linux commit `13bb06f8dd42071cb9a49f6e21099eea05d4b856` first; this is the highest-value prior-art chain.
- PM/Cerebro should compare SC-3B's wake/deferred-wake contract against Linux's `wake_q_add()` comment-level contract: the task must already be ready for an immediate wake at the queueing point.
- PM/Cerebro should compare SC-3B's target-CPU and wake-cost placement against Linux `ttwu_queue_wakelist()` and Zircon scheduler CPU-assignment docs as architecture references, not as implementation templates.
- If NMI-capture work continues, PM should read Linux lockup-watchdog docs and Red Hat's virtualization warning together before interpreting any future watchdog output under QEMU.
- Treat Asterinas PR #2488 as a close Rust-kernel SMP-hang reference for audit questions around runqueue locking, IRQ-disabled lock discipline, and missing wake after child/reap state transitions.

## References

- QEMU issue #1696, "Linux kernel hangs rarely when booting on the latest qemu." `https://gitlab.com/qemu-project/qemu/-/issues/1696`
- Richard W.M. Jones, LKML, "printk.time causes rare kernel boot hangs." `https://lkml.org/lkml/2023/6/13/733`
- Linux commit `13bb06f8dd42071cb9a49f6e21099eea05d4b856`, "tick/common: Align tick period during sched_timer setup." `https://git.kernel.org/pub/scm/linux/kernel/git/tip/tip.git/commit/?id=13bb06f8dd42071cb9a49f6e21099eea05d4b856`
- Linux source, `kernel/sched/core.c`. `https://code.googlesource.com/linux/torvalds/linux/+/9c70b2a33cd2aa6a5a59c5523ef053bd42265209/kernel/sched/core.c`
- Linux kernel docs, "Softlockup detector and hardlockup detector (aka nmi_watchdog)." `https://www.kernel.org/doc/html/latest/admin-guide/lockup-watchdogs.html`
- QEMU docs, "Multi-threaded TCG." `https://www.qemu.org/docs/master/devel/multi-thread-tcg.html`
- QEMU docs, "Execution Record/Replay." `https://www.qemu.org/docs/master/devel/replay.html`
- QEMU docs, "QEMU Monitor." `https://www.qemu.org/docs/master/system/monitor.html`
- Red Hat docs, "Keeping kernel panic parameters disabled in virtualized environments." `https://docs.redhat.com/en/documentation/red_hat_enterprise_linux/10/html/managing_monitoring_and_updating_the_kernel/keeping-kernel-panic-parameters-disabled-in-virtualized-environments`
- QEMU-devel archive, "[Bug 1329956] Re: multi-core FreeBSD guest hangs after warm reboot." `https://lists.gnu.org/archive/html/qemu-devel/2016-10/msg03983.html`
- FreeBSD Architecture Handbook, "Chapter 8. SMPng Design Document." `https://docs.freebsd.org/en/books/arch-handbook/smp/`
- OSDev forum, "SMP and the lost + wake-up problem." `https://forum.osdev.org/viewtopic.php?t=40281`
- Asterinas issue #2217, "SMP LTP hangs." `https://github.com/asterinas/asterinas/issues/2217`
- Asterinas issue #2152, "LTP test `kill02`/`kill08` broken on SMP." `https://github.com/asterinas/asterinas/issues/2152`
- Asterinas PR #2488, "Fix LTP CI hang problems." `https://github.com/asterinas/asterinas/pull/2488`
- Asterinas issue #2851, "`fcntl(F_SETOWN)` may hang indefinitely under concurrent forked calls." `https://github.com/asterinas/asterinas/issues/2851`
- seL4 docs, "IPC." `https://docs.sel4.systems/Tutorials/ipc.html`
- seL4 RFC 0170, "seL4 multikernel IPI API." `https://sel4.github.io/rfcs/implemented/0170-multikernel-ipi-api.html`
- seL4 release notes 6.0.0. `https://docs.sel4.systems/releases/sel4/6.0.0.html`
- Fuchsia docs, "Zircon Scheduling." `https://fuchsia.dev/docs/concepts/kernel/kernel_scheduling`
- Fuchsia docs, "Channel." `https://fuchsia.dev/docs/reference/kernel_objects/channel`
- Theseus repository. `https://github.com/theseus-os/Theseus`
- Theseus book, "Tasking Subsystem in Theseus." `https://www.theseus-os.com/Theseus/book/subsystems/task.html`
- Kevin Boos et al., "Theseus: an Experiment in Operating System Structure and State Management," OSDI 2020. `https://www.usenix.org/conference/osdi20/presentation/boos`
- Yuke Peng et al., "ASTERINAS: A Linux ABI-Compatible, Rust-Based Framekernel OS with a Small and Sound TCB," USENIX ATC 2025. `https://www.usenix.org/conference/atc25/presentation/peng-yuke`

---

**End of research artifact.**

*External prior art is partial and suggestive, not decisive. The strongest source chain says rare QEMU-visible kernel hangs can be real guest-kernel timing bugs requiring very high iteration counts to classify. The strongest mechanism-adjacent material says wake/sleep atomicity, runqueue IRQ discipline, and deferred wake contracts are recurring SMP failure surfaces across real kernels and hobby/Rust kernels alike.*
