# Research: Prior art for the AltimitNeo SMP partial-reap heisenbug

**Date:** 2026-05-08
**Author:** PM
**Type:** Research artifact (informational; not normative). Surveys public material on the bug class observed in WEDGE-INV-2 through WEDGE-INV-7 and the observation strategies the field has developed for it. Doc-only artifact; nothing here is binding on the project. Recommendations herein require ADR or PM-NOTE to take effect.
**Authority:** None — research report, not a PM decision or commitment. Cited as input to WEDGE-INV-8 dispatch design and to forward-looking work after the mechanism is localized.

---

## 0. TL;DR

- The bug class — SMP partial-reap race that masks under instrumentation, surfaces as multiple distinct signatures depending on host timing — is a textbook **heisenbug**, in the technical sense of the term. The field has well-developed literature on this class, most authoritatively from Paul McKenney (Linux RCU maintainer, Meta).
- **GDB stubs are documented to perturb guest timing** even when no client is attached. Our WEDGE-INV-5 + WEDGE-INV-7 combined 0/120 is consistent with the expected perturbation profile. This is not surprising once the precedents are read.
- **QEMU's monitor `nmi` command was specifically designed for the use case Path J targets**: triggering an in-guest debug crash dump. The mechanism we're proposing is not a workaround — it's the intended use of a feature QEMU added for this purpose.
- **The detect-then-instrument pattern is McKenney's canonical anti-heisenbug technique.** Adding instrumentation that runs only AFTER the bug is detected (rather than continuously) is documented to preserve the timing window that allows the bug to fire. Path J implements this pattern.
- **Direct shape-level precedents exist** — at least two well-documented bugs in production kernels (Zephyr #106653, Linux libsas `b90cd6f`) match the "check-then-act gap allows race" structure that is implicated in our `zombie_count == 0` short-circuit. McKenney's TREE03 heisenbug (Linux RCU) is a multi-mechanism interaction with similar de-heisenbugging arc to ours.
- **No source documents an exact match for our wedge** — but the structural elements (lock-free fastpath read, partial reap during worker spawn, multi-CPU race window, timing-shifted manifestation) are individually well-documented in the literature. The combination is novel only in that it occurs in our specific reaper architecture; the constituent races are all known patterns.
- **The literature's prevention guidance is what Bible Article 1.3 already mandates.** Concurrency-first design, code review, verification-where-applicable. McKenney's closing slide reads "Don't randomly hack concurrent code." There is no published silver-bullet remediation; the path forward is mechanism localization (Path J), then targeted fix, then design-level prevention.

---

## 1. The bug class — what we have

For reference inside this report, the WEDGE-INV-3 through WEDGE-INV-7 picture:

| Configuration | Boots | Timeouts | Rate | Signatures observed |
|---|---:|---:|---:|---|
| Cargo, no stub (W-3 + W-4) | 150 | 4 | 2.67% | `POST_SMOKE_SMP_SCENARIO` ×4 |
| Direct QEMU, GDB stub (W-5) | 60 | 0 | 0.0% | none |
| Direct QEMU, no stub (W-6) | 200 | 1 | 0.5% | `SMOKE_MID_PHASE` ×1 |
| Cargo, GDB stub (W-7) | 60 | 0 | 0.0% | none |

Plus PATCH1's three pre-substrate-shift signatures (`SMOKE_MID_PHASE`, `SMOKE_COMPLETION` literal, `POST_SMOKE_USER_PROCESS`) all going quiet on the post-ADR-015 substrate. Total: at least four distinct latent partial-reap-class races, each with its own timing profile.

The pattern attributes:

- **Multi-CPU**. Surfaces only at `NEO_SMP=4`, not `NEO_SMP=1`.
- **Specific phase**. Always partial-reap-class — between the spawn and reap of a sub-population of related worker threads.
- **Timing-sensitive**. Rate varies with host cadence (cargo vs. direct QEMU).
- **Heisenbug under instrumentation**. WEDGE-INV-2 saw 0/30 with instrumented smoke paths despite a historical baseline of ~14% in-kernel.
- **Heisenbug under GDB stub**. WEDGE-INV-5/7 combined 0/120 with stub on, despite cargo+no-stub baseline of 2.67%.
- **Memory-layout-sensitive**. ADR-015's heap shift damped without eliminating; signatures shifted but the bug class persists.

---

## 2. The heisenbug literature

The field of multi-CPU race debugging is mature. Two pieces of canonical material:

### 2.1 McKenney, "Hunting Heisenbugs" (LPC 2023, LCA 2024, multiple LWN follow-ups)

Paul McKenney maintains the Linux kernel's RCU subsystem and the Linux-Kernel Memory Model. His recurring "Hunting Heisenbugs" talk is the most authoritative public treatment of the class.

**Key principles from the LPC 2023 deck** (`https://lpc.events/event/17/contributions/1504/attachments/1322/2653/heisenbug.2023.11.13c.pdf`):

- **Why heisenbugs happen**: rare events + observation that changes timing = bug hides when looked at. McKenney's frame: "Anti-heisenbug: reduce MTBF" — i.e., make the bug fire faster so it can be observed without making the observation itself the masking factor.
- **Increase workload intensity**: focused stress on the relevant subsystem (rcutorture-style) rather than general workload. McKenney: "What has caused trouble in the past?" — promote those events.
- **Inject strategic delays**: deliberately slow down code in suspected race-prone paths to widen the timing window. RCU-CPU-hotplug strategic delays are McKenney's example.
- **Count near misses**: the FAA reports near-miss collisions because they are 2 OOM more frequent than actual collisions. Do the same with bugs — instrument the kernel to count "almost hit the bug" events; this produces useful gradient signal for evaluating fixes.
- **De-emphasize fastpaths**: fastpaths can hide bugs in the slowpath. McKenney's DYNIX/ptx allocator story: focused test that bypassed the per-CPU cache reduced MTBF from 5–27 hours to 12 minutes — 1–2 OOM improvement vs. the full stress test. **This is structurally exactly our `zombie_count == 0` short-circuit.**
- **Detect, then instrument**: code executed *after* the heisenbug occurs typically does not affect MTBF. Add debugging that fires only on detection (timeout, stall warning, etc.). The TREE_RCU stuck-grace-period bug was localized this way: ~300-hour MTBF, instrumentation hid the bug — but adding the dump trigger AFTER an 8-second timer overrun did not. **This is Path J's exact pattern.**
- **Force rare risky legal states (whitebox)**: when a rare combination of events takes the system to a vulnerable-but-legal state, start the system in that state directly. "The nuclear option."
- **Avoiding heisenbugs in the first place** is McKenney's preferred path: careful concurrency-first design, stress testing, code review, formal verification, and his closing slide — "Don't randomly hack concurrent code."

### 2.2 McKenney, "Hunting a TREE03 Heisenbug" (people.kernel.org)

A long-form blog post on the localization of a specific Linux RCU heisenbug. Worth reading end-to-end if you want a worked example. Highlights:

- Initial MTBF of "once per year per system," reduced through anti-heisenbug techniques to "once per minute per system" — five OOM reduction.
- Insight that CPU-hotplug operations and RCU grace periods interacted; the "rare event combination" had to be stressed deliberately.
- Multiple instrumentation attempts hid the bug; the eventual capture used a near-miss-counting approach.
- Conclusion: bug was a symmetric-access pattern to `->gp_seq` that was locally elegant but globally racy. McKenney's closing reflection includes the observation that the bug had been in a public Linux code base for two years before this campaign — formal verification and validation tools had had the chance to find it and didn't.

The arc of TREE03 — "once per year" → "once per few hours" → "once per few minutes" → mechanism localized → fix + test → ship — is the arc the AltimitNeo wedge investigation is on. Our current state of "0.5% to 2.67% depending on configuration" is roughly equivalent to McKenney's "few hours per system" stage. Path J is the next reduction in MTBF, via post-detection instrumentation.

### 2.3 Other heisenbug references

- **"Heisenbugs" in O'Reilly's "Debug It!"** — general-purpose framing of heisenbugs as bugs whose observation changes their behavior. Not kernel-specific.
- **"How to Catch Your Heisenbug" (DEV Community)** — accessible introduction to the class. Good link to point engineers at; not authoritative.

---

## 3. GDB stubs perturb guest timing — confirmed by primary sources

WEDGE-INV-5/7's 0/120 result matches expected GDB-stub behavior, not anomalous behavior:

- **Corellium's kernel debugging documentation** (`corellium.com/blog/debug-kernel`) directly states that high latency from the underlying GDB-stub machinery can cause the VM to visually freeze or stutter — observable timing perturbation even during normal use.
- **OSDev wiki on QEMU debugging** notes that the GDB stub is implemented as breakpoint-trap callback machinery; the registration of those callbacks changes some emulation paths whether or not a GDB client connects.
- **QEMU 2.10.1 monitor behavior, documented on OSDev forums**: the stub becomes part of the emulation pipeline at startup; "stub on but unattended" is a meaningful state, not a no-op.
- **Quarkslab's Linux kernel instrumentation guide** (`blog.quarkslab.com/linux-kernel-instrumentation-from-qemu-and-gdb.html`) treats the GDB stub as a debugging tool, not a transparent observer — the guide's setup steps include disabling features (KASLR, frame-pointer omission) so GDB sees what it expects, which implies the stub is part of the build/run profile, not just a passive observer.

The 0/120 we observed is therefore not surprising in retrospect. The retrospective framing: GDB-stub class observation was a reasonable first attempt, but it was always going to be unable to distinguish its own perturbation from masking the bug. WEDGE-INV-7's pre-flight stop condition (the dispatch design) caught this correctly.

---

## 4. QEMU monitor `nmi` command — designed for exactly this use case

The QEMU `nmi` command was added (per upstream commit `9cb805fd` by Alexey Kardashevskiy, reviewed by Alexander Graf, signed off by Paolo Bonzini) with a specific stated purpose: to invoke an in-guest debug crash dump from the host side. From the commit message paraphrased: the callback implements an action required to cause debug crash dump on in-kernel debugger invocation.

Per the QEMU documentation (`qemu-project.gitlab.io/qemu/system/monitor.html`), the monitor `nmi <cpu>` command injects an NMI on the named CPU (x86/s390) or all CPUs (ppc64). On x86 — our target — the NMI is delivered through the standard LAPIC injection path, the same way a hardware NMI would arrive. The KVM path traces show that NMI injection is just an APIC IRQ accept with vector 0 / NMI flag (per the OSDev forum thread on KVM NMI behavior).

The mechanism is structurally distinct from GDB-stub observation:

| Property | GDB stub | QEMU monitor `nmi` |
|---|---|---|
| Always-on perturbation | Yes (callback registration) | No |
| Affects emulation paths | Yes (single-step, breakpoint traps) | No |
| Triggered by host action | No (continuous) | Yes (single command) |
| In-guest visibility | Implicit (breakpoints) | Explicit (NMI handler) |
| Originally for | Source-level debugging | In-kernel crash dump |

Path J's design — host-side wrapper detects timeout, sends `nmi 0..3` via TCP monitor connection, in-stash kernel-side NMI handler dumps state to serial — is a textbook use of the QEMU `nmi` feature. The kernel-side handler is the contract QEMU's `nmi` command was designed to invoke.

---

## 5. Direct shape-level precedents

No public source documents our exact bug. Three documented bugs match constituent shapes.

### 5.1 Zephyr RTOS issue #106653 — "kernel: SMP thread timeout handler race issue"

(`github.com/zephyrproject-rtos/zephyr/issues/106653`)

The bug: `sys_clock_announce()` unlocks the timeout list lock before calling the timeout's handler. The handler `z_thread_timeout()` then locks the scheduler spinlock. In the gap between those two events, on SMP, a race can fire — the wrong timeout gets processed. The race window widens if the CPU services another ISR or contends for the scheduler spinlock.

**Why it matches our pattern**: a check-then-act sequence with a gap that is multi-CPU-vulnerable. Our `zombie_count == 0` short-circuit is structurally similar — read a counter without a lock, decide based on the read, then act. If the counter is being modified concurrently on another CPU, the action is racy. Zephyr's solution to #106653 was complex enough that it spawned a dedicated tracking issue separate from the original fix attempt.

### 5.2 Linux libsas `b90cd6f` — "scsi: libsas: fix a race condition when smp task timeout"

(`github.com/torvalds/linux/commit/b90cd6f2b905905fb42671009dc0e27c310a16ae`)

The bug: when an LLDD (low-level device driver) sets a SAS task to `DONE` in interrupt context, the SMP timeout timer may fire concurrently. The timeout handler `smp_task_timedout()` would call `complete()` regardless of the `DONE` flag. The task could be freed by the LLDD path before the timeout handler completes its access — use-after-free.

**Why it matches our pattern**: the bug is a check-then-act sequence in a multi-context race (interrupt + timer). The fix added the gating check inside the lock window. Conceptually similar to our short-circuit, with use-after-free being the consequence in libsas's case versus deadlock/wedge in ours.

### 5.3 McKenney TREE_RCU stuck-grace-period heisenbug

The bug: an interaction between timers, CPU hotplug, and RCU grace-period processing left RCU stuck waiting for a quiescent state that was already met. ~100s of hours MTBF on stress test. Adding debugging hid the bug. The eventual localization used post-detection instrumentation: when a timer took more than eight seconds and more than three times longer than requested, dump state.

**Why it matches our pattern**: multi-mechanism interaction (timers + CPU hotplug + RCU grace periods) producing a wedge. Heisenbug under instrumentation. Detect-then-instrument was the unlock.

---

## 6. The lock-free fastpath as known anti-pattern

McKenney's "De-emphasize fastpaths" anti-heisenbug guidance is directly applicable. The DYNIX/ptx memory allocator story, paraphrased: a per-CPU cache fastpath made the global slowpath rare; bugs in the slowpath were therefore rare. Focused test that bypassed the per-CPU cache produced the bug at 12-minute MTBF, 1–2 OOM faster than full-stress test.

Translating to our case:

- The lock-free `if zombie_count == 0 return` is the fastpath.
- The slowpath — where reap acquires the lock, walks the zombie list, dequeues, etc. — is exercised only when `zombie_count > 0` per the lock-free read.
- A workload that **forces every reap call into the slowpath** (e.g., always claim a zombie exists, force the lock-acquire-walk-dequeue sequence) would surface slowpath bugs faster.

This is a forward-looking observation, not a Path J input. It would inform a future regression-test design once mechanism is localized.

The other side of the same coin: `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` analyzed the lock-free `zombie_count == 0` short-circuit and concluded composition-safe under H1's narrow refutation. McKenney's framework would say: the analysis is correct *for the specific race H1 named*, but lock-free reads of counters that gate slowpath entry are a known anti-pattern class — the analysis-is-correct-for-this-race and the pattern-is-suspicious-in-general are not in conflict. The right disposition is to treat the short-circuit as a candidate fastpath to de-emphasize once mechanism is localized, regardless of whether mechanism turns out to be H1-class.

---

## 7. Implications for the wedge investigation

These follow from the research, in priority order:

### 7.1 Path J is well-grounded

The detect-then-instrument pattern is canonical anti-heisenbug technique. The QEMU `nmi` mechanism is its intended-use-case interface. WEDGE-INV-8's design (in `Plans/DISPATCH-2026-05-08-wedge-investigation-8-nmi-watchdog-cerebro.md`) is consistent with field practice. No design adjustment from the research.

### 7.2 If Path J also masks (Case G in WEDGE-INV-8 §6), strategic delays are next

McKenney's "inject strategic delays" technique is the natural follow-up. A stash that adds a deliberate small delay (say, 100 µs) after each `smp-hog-N` reap event would *widen* the partial-reap timing window. If the bug is sensitive to that window, the rate should rise; if it's not, we know the timing window isn't the discriminator.

This is Path D from the prior status update, refined: not "always-on watchdog" but "intentionally inject the rare event" per McKenney's framework. Stash, not main. The dispatch would be similar in shape to WEDGE-INV-8 minus the NMI handler, plus a measurement loop to compare rate-with-delay vs. rate-without-delay.

### 7.3 Near-miss instrumentation may apply once mechanism is partially localized

If Path J narrows the mechanism to "race between X and Y" but doesn't fully name it, near-miss counting (instrument the suspected race window with a counter that increments when timing is "close to bad") could resolve the remaining ambiguity at much higher rate than the actual bug fires. This is McKenney's RCU near-miss example. Worth keeping in reserve.

### 7.4 The fastpath-de-emphasize technique informs the eventual regression test

Once mechanism is named and a fix lands, the regression test should not just stress-test the working configuration — it should explicitly exercise the slowpath. A test that artificially forces every reap call to take the lock-acquire-walk-dequeue path would catch regressions in slowpath logic that the production fastpath would mask.

This is forward-looking, not a current-investigation input. Note as input to the eventual SC-3b verification design.

### 7.5 Prevention guidance is consistent with Bible Article 1.3

McKenney's prevention list — concurrency-first design, stress test, integration test, code review, verification — is exactly what Bible Article 1.3 ("real operating systems engineering") and Article 15 ("code standards") already require. The research does not add new requirements; it provides external validation that the project's standing discipline is correct for this class of bug.

The implication: once mechanism is localized and a fix lands, the post-mortem should evaluate which of those five disciplines (design, unit test, integration test, code review, verification) failed for the specific bug, and whether the project's instance of that discipline needs strengthening. This is normal post-bug practice; flagging here for completeness.

### 7.6 Observability for similar future bugs

ADR-014 (kernel observability deferred to Phase 8+) currently does not commit to specific mechanisms. The Path J experience suggests two design hooks that future-Phase-8+ observability should include:

- **NMI-triggerable state dump** (the in-kernel side of Path J) is generally useful, not specific to this bug. Could be promoted from stash to mainline as an ongoing kernel-debugging facility.
- **Near-miss counters at known race-prone points** — McKenney's pattern. Modest runtime cost, very high diagnostic value when bugs eventually surface.

These are forward-looking suggestions for Phase 8+ ADR-014 expansion, not current-investigation inputs.

---

## 8. Caveats

1. **No public source documents the exact AltimitNeo wedge.** The research above identifies structural matches and applicable techniques, not a copy-paste fix. Expectation calibration: Path J's job is to localize *our* mechanism, which will be specific to the AltimitNeo reaper architecture.
2. **Several cited Linux commits are years old.** The bug shapes (lock-order gaps, check-then-act races, multi-mechanism interactions) are timeless; the specific commits are illustrative, not canonical. Do not assume the Linux fixes apply directly.
3. **Some publication metadata in arXiv-style references is approximate** — for academic papers cited above I checked authority by first-author name and venue rather than DOI verification.
4. **The McKenney material is available as PDF + YouTube + blog posts.** The PDF I fetched (LPC 2023) is the most structured presentation; the LCA 2024 talk is the most recent and may contain additional anti-heisenbug techniques not captured here. If WEDGE-INV-8 also produces an inconclusive result, watching the LCA 2024 video before the next dispatch design would be reasonable.
5. **This research was completed before WEDGE-INV-8 ran.** If WEDGE-INV-8 captures a mechanism, this report's framework for "what's next" may need to be revisited based on what the mechanism actually is.

---

## 9. References (selected; full list in inline citations above)

- McKenney, Paul. "Hunting Heisenbugs." Linux Plumbers Conference 2023. PDF at `lpc.events/event/17/contributions/1504/attachments/1322/2653/heisenbug.2023.11.13c.pdf`.
- McKenney, Paul. "Hunting a TREE03 Heisenbug." `people.kernel.org/8q9a9dt4q3`.
- McKenney, Paul. "Hunting Heisenbugs." Linux Conf AU 2024. `2024.everythingopen.au/schedule/presentation/15`.
- Zephyr RTOS issue #106653, "kernel: SMP thread timeout handler race issue." `github.com/zephyrproject-rtos/zephyr/issues/106653`.
- Linux kernel commit `b90cd6f2b905905fb42671009dc0e27c310a16ae`, "scsi: libsas: fix a race condition when smp task timeout."
- QEMU upstream commit `9cb805fd2674f474d058fee6d7aa9e83fcd3d336`, "cpus: Define callback for QEMU 'nmi' command." Author: Alexey Kardashevskiy. Reviewer: Alexander Graf. Sign-off: Paolo Bonzini.
- QEMU project documentation, "QEMU Monitor." `qemu-project.gitlab.io/qemu/system/monitor.html`.
- Corellium engineering blog, "Kernel Debugging" (`corellium.com/blog/debug-kernel`) — direct evidence of GDB-stub timing perturbation.
- Linux kernel documentation, "Debugging kernel and modules via gdb." `kernel.org/doc/html/latest/process/debugging/gdb-kernel-debugging.html`.
- McKenney, Paul. Various blog posts at `paulmck.livejournal.com` (LiveJournal archive of pre-Meta posts) and `people.kernel.org` (current).
- Eberhardt, Robert. "My First Kernel Module: A Debugging Nightmare." `reberhardt.com/blog/2020/06/06/my-first-kernel-module.html`. Accessible illustration of read-destruct race patterns.

---

## 10. Cross-references (project-internal)

- `Plans/DISPATCH-2026-05-08-wedge-investigation-8-nmi-watchdog-cerebro.md` — the dispatch this research informs.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md` — the project-state note that triggered the research thread.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` — the convergence-note framing that the lock-free `zombie_count == 0` short-circuit analysis depends on; §6.4 of this report is directly applicable.
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — the lock-free short-circuit composition-safety analysis; §6.4 of this report is the McKenney-framework view of that analysis.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment whose discharge this investigation continues to inform.
- `docs/ADR-014-kernel-observability-deferred-to-phase-8.md` — relevant to §7.6 above.

---

**End of research report.**

*The bug class is well-studied. The observation strategy we're attempting is field-canonical. The constituent shapes of our specific bug all have public precedents. What remains for the project is to localize our specific mechanism — Path J is the right tool and McKenney's framework is the validation that it is.*
