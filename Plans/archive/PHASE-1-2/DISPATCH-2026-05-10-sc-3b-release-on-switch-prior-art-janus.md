# Dispatch: SC-3B "release-on-switch" prior-art (Janus second-pass)

**To:** Janus (deepseek-v4-flash:cloud, deep-research specialist)
**From:** PM
**Date:** 2026-05-10
**Type:** Single-recipient deep-research dispatch. Research artifact only — no code, no fix proposals, no project-decision recommendations. PM consumes the artifact alongside Cerebro's parallel Phase D findings and performs the synthesis.
**Authority:** `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §2.1 — recurrence trigger satisfied (same failure now recurred across two distinct fix attempts: Fix 6.1 attempt at run 6/200 + Sub-step 5b IPI fix at run 52/200; Phase B null at 100/100; Phase C partial NMI at run 5/50 with new signature). Internal investigation has produced four passes (Phase A three-way, Phase B NMI, Outcome B NMI, Phase C NMI+instrumentation) and the signature has refined but a clean mechanism is not yet in hand. Per the trigger conditions, a second Janus pass is appropriate.
**Substrate context:** Project HEAD `2d8f65da77b4ea10f3d53226724e9381961c48c0`. AltimitNeo is a Rust monolithic kernel, no_std, x86_64, Limine-bootloaded, no POSIX. Phase 2 BUILD-3 sub-commit 3 (SC-3B) closure is blocked on the cross-CPU rendezvous stress test. Phase D lock-ownership probe runs in parallel with this dispatch.

---

## 0. Standing instructions

1. **Research artifact only.** No code. No fix proposals. No "you should do X" recommendations to the project. The artifact is consumed by PM and informs PM's design dispatches.
2. **Primary vs secondary distinction maintained throughout.** Primary = original kernel sources, peer-reviewed papers, official mailing-list/PR threads, project documentation. Secondary = community forums, blog posts, Reddit, Stack Overflow, Wiki articles. Single-source findings flagged explicitly.
3. **Honest negative findings.** If a query returns no clear prior art, say so. Don't pad with adjacent material that doesn't address the question.
4. **Specific citations.** URLs, commit hashes, issue numbers, line references where applicable. The first-pass artifact (`Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md`) sets the citation density expectation.
5. **Anti-scope:** no AltimitNeo-internal recommendations; no implementation suggestions; no architectural prescriptions for the project. Description and citation only.
6. **Output filename:** `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`.

---

## 1. Background context

The first-pass artifact at `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` should be loaded as prior context. It surfaced Linux's `wake_q` / `resched_curr` / `ttwu_queue_wakelist` patterns, Asterinas PR #2488 ("init not woken after dead children moved"), the Linux/QEMU 2023 `printk.time` rare boot hang, and FreeBSD/seL4/Theseus architectural references. PM's first-pass calibration verdict: high-fidelity. (Citation spot-check from PM is still pending but non-blocking.)

The signature has now refined. Sub-step 5b landed proper xAPIC-on-APs + reschedule-IPI infrastructure + `place_thread_on_cpu` IPI notification. The gate failed again with the same MISSING_TERMINAL_FRAME signature at run 52/200. Phase C's NMI capture (run 5/50, new instrumentation included) shows a different snapshot from the prior Outcome B capture:

- **CPU0 RIP in `_mm_pause`**, current thread is `stress-receiver id=14` in `Running` state — *not* parked.
- **The receiver's last instrumentation frame was `pre_block` on CPU0** for pair=3 iter=4. No matching `post_wake` ever emitted.
- **Channel ch=3's state lock is held** (try_lock failed in NMI handler).
- **CPU1's `PerCpuScheduler.inner` lock is held** (try_lock failed in NMI handler).
- **CPU3 is running ap-3-main (the AP idle loop)** with three stress workers Ready in CPU3's queue, but those workers don't drain — they apparently yield instantly back when given CPU time, suggesting they're hitting the same contended channel ch=3 lock.
- **CPU0/CPU2/CPU3 scheduler.inner are unlocked.** Channels 0/1/2 are Idle.
- **No zombie/reap inconsistencies.** No panic. No FAIL frame.

The receiver thread holding the channel lock and being stuck mid-block is a deadlock-shape or stuck-holder-shape pattern — different from Outcome B's "no parked threads, scheduler shows AP-main running with workers Ready" signature.

The relevant AltimitNeo-internal mechanism is a **SwitchMutex** primitive (kernel-owned, ratified in `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` §3.2) used for `Channel.state` and `PerCpuScheduler.inner`. It supports a **release-on-context-switch** discipline: a "guard" object holds the mutex, and the lock byte is released *atomically with the kernel context switch* by an asm helper `context_switch_with_release2(prev_rsp_ptr, next_rsp, channel_lock, sched_lock)` that drops both locks after the stack switch but before resuming the new thread. The Q6 design goal is to avoid the Bible 8.5.4 pattern violation (no blocking while holding a spinlock) while preserving same-CPU rendezvous handoff semantics.

The hypothesis Phase D will test directly is:

- **Hr (release-on-switch ordering bug):** the asm release of one or both locks is misordered, mistimed, or actually fails to drop the lock byte under some path — making Bible 8.5.4 effectively violated at runtime even though the design honors it on paper.

The Janus second-pass complements Phase D by surveying real-world prior art for this kind of mechanism: who else has built it, what bugs have been seen in the wild, what implementation discipline has emerged.

---

## 2. Research questions

### 2.1 Prior art for "release-on-context-switch" lock primitives

What kernels (any language) implement lock primitives whose release is coupled to a context switch — i.e., the lock byte is dropped as part of the asm switch sequence, not by a separate `unlock()` call before the switch?

Specifically:

- Linux's `spin_unlock_irq` patterns, `migrate_disable`, `preempt_disable`, `__schedule()` interactions — does Linux ever drop a held lock as part of `__schedule()`'s asm path?
- FreeBSD's `mtx_unlock_spin` + `mi_switch()` — does FreeBSD have a "release during switch" pattern, or is it always release-then-switch?
- seL4's IPC fastpath — the synchronous endpoint design has lock disciplines around the call/reply transitions; is there a release-during-switch pattern in the kernel proper or in the IPC fastpath asm?
- Asterinas's process/scheduler lock patterns — given the recent SMP hang fixes (PR #2488), is there a release-during-switch primitive or has it been rejected as a design pattern there?
- Theseus's tasking subsystem — Theseus is Rust-only and intralingual; is there a guard/release pattern coupled to context switch?
- Redox's scheduler + IPC — same question.
- Tock's preemption + capability mechanics — Tock is Rust-based and has a different model but worth surveying.
- Zircon's (Fuchsia) kernel — known patterns in the scheduler around lock release at the switch boundary.

If no kernel implements release-on-switch as a primitive, what's the closest design pattern, and why did those projects choose alternatives?

### 2.2 Known bug patterns in lock-release-during-context-switch sequences

If release-on-switch is implemented anywhere, what bug patterns have been seen?

- Lock byte released after the switch instead of before, leading to a window where the new thread can re-acquire it.
- Lock byte released before the switch but after a memory barrier issue, leading to other CPUs observing the release out of order.
- Asm release path not visited in some control-flow branch (e.g., a guard's drop runs but the asm release is skipped on a specific path).
- Two-lock release where one releases and the other doesn't — an asymmetry bug.
- Owner metadata or instrumentation getting out of sync with actual lock state.

CVEs, fixed commits, mailing-list discussions, conference papers — anything that documents this class of bug.

### 2.3 The "guard" type interacting with asm context switching in Rust kernels

The Rust idiom of `MutexGuard` releasing the lock on Drop is well-established, but Rust kernels that need to release a lock as part of an asm switch can't rely on Drop alone — the asm runs after the guard's lifetime would normally end.

- Theseus, Asterinas, Redox, Tock — how do they handle this? `mem::forget(guard)` patterns? Manual lock-byte storage and release? `unsafe fn` that takes ownership of the lock state?
- Are there documented pitfalls — e.g., a guard's destructor running when it shouldn't, or not running when it should?
- Are there academic-paper-level treatments of this pattern? (USENIX, OSDI, EuroSys, ASPLOS, SOSP venues.)

### 2.4 Releasing multiple locks atomically with a context switch

AltimitNeo's `context_switch_with_release2` releases two locks (channel + scheduler) coupled to one switch. Prior art for releasing N>1 locks in this kind of sequence:

- Lock-order discipline at release time — does Linux/FreeBSD/seL4/etc. enforce a release order, and what does that order look like (acquired-last-released-first, or something else)?
- Known bugs where two-lock release went wrong — one lock released before the other in a way that opened a window.
- The "switch with lock release" pattern in any RTOS or research kernel — RT-Linux, NOVA, Composite, Barrelfish.

### 2.5 Bible 8.5.4 invariant in real kernels

The "no blocking while holding a spinlock" rule is a canonical kernel discipline. How is it enforced in real kernels?

- Linux: `might_sleep()`, `lockdep`, `CONFIG_PROVE_LOCKING`, the `__must_check` discipline.
- FreeBSD: `WITNESS` lock-tracking, `KASSERT` on sleep-while-locked.
- seL4: the verification proof — does the proof guarantee no-block-holding-lock by construction?
- Rust kernels (Theseus, Asterinas, Redox): how do they enforce this — type-level, runtime, or convention?

What patterns have emerged for satisfying this rule when the kernel needs to "block" (in the sense of yielding to another thread) while logically holding a lock — i.e., for IPC rendezvous? The release-on-switch pattern is one answer; what are the others?

### 2.6 Cross-CPU lock acquisition orderings under IPC stress

The signature shows `Channel.state` (held by a thread on CPU0) and `PerCpuScheduler.inner` (held by a thread on CPU1) both contended. If two threads on different CPUs each hold one of these locks and reach for the other, that's a textbook AB/BA deadlock.

- How do Linux/FreeBSD/seL4 prevent this in their IPC code paths?
- What lock-acquisition order disciplines have been documented?
- Specific bugs where IPC paths inadvertently formed cycles in the lock graph.

### 2.7 Memory ordering at the context-switch boundary

The asm context switch involves stack-pointer replacement, register restore, and the lock release. Memory ordering matters — store-buffer flushes, fences, etc.

- x86 TSO behavior at the switch boundary: when does the "lock release" store become visible to other CPUs vs. when does the new thread start executing?
- Known bugs where the release happened in program order but was observed out of order by another CPU.
- The interaction with `LOCK CMPXCHG` / `XCHG` for the lock byte release vs. plain `MOV` releases — Linux uses different patterns in different paths.

---

## 3. Anti-scope clarification

Things this dispatch does NOT ask for:

- AltimitNeo-internal recommendations of any kind.
- Implementation suggestions or design patterns to apply.
- Code (even pseudocode).
- Confirmation or refutation of specific AltimitNeo hypotheses (Hd / Hs / Hr / Hc) — that's PM's job after consuming both Phase D and this artifact.
- Speculative reasoning beyond what citations support.

---

## 4. Output expectations

Mirror the first-pass artifact's structure (`Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md`):

1. **TL;DR** — 2-3 paragraphs summarizing the headline findings.
2. **Sources surveyed** — primary (with URLs) and secondary, separated.
3. **Findings by query** — one section per question 2.1–2.7. Cite specific commits, issues, papers, source files where applicable. Mark single-source findings.
4. **Conflicts and contested claims** — where sources disagree.
5. **Single-source findings (low confidence)** — flagged separately.
6. **Recommended next steps for the research thread** — *not* fix recommendations to AltimitNeo. If a particular kernel's source code merits a deeper read, say so. If a particular paper's bibliography looks like the right thread to follow, say so.
7. **References** — full URLs / IDs, formatted consistently.

Citation density target: comparable to first-pass (~24 primary sources). If the topic surface is narrower than the first-pass topic, fewer is fine — quality over quantity.

End the artifact with an italicized one-sentence summary of the research's bottom line.

---

## 5. Estimated wall time

The first-pass artifact took roughly a day of deep search and synthesis. This dispatch's scope is narrower (specific mechanism class) but goes deeper (asm-level details, memory ordering). Expect comparable or slightly longer wall time. PM does not impose a time budget — the artifact's quality matters more than its arrival time.

---

## 6. Cross-references

**Authority:**
- `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §2.1 — recurrence trigger conditions.
- `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md` — Janus role definition.

**First-pass artifact:**
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` — load as prior context.
- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-janus-deep-research-janus.md` — the dispatch that produced the first-pass.

**Substrate / signature evidence:**
- `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-10.md` — current signature with channel + scheduler locks held.
- `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md` — prior signature without channel state visibility.
- `Plans/SC-3B-SOURCE-AUDIT-2026-05-09.md` — confirmed substrate facts (no IPI infrastructure pre-Sub-step-5b; AP main yields aggressively; SwitchMutex is the load-bearing primitive).

**AltimitNeo design artifacts referenced for context (not for prescription):**
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` §3.2 — Q6 ABI ratification (per-call-site, channel-first release order).
- `docs/bible.md` Article 8.5.4 — no blocking while holding a spinlock.

**Parallel work:**
- `Plans/DISPATCH-2026-05-10-sc-3b-phase-d-lock-ownership-cerebro.md` — Cerebro's Phase D lock-ownership probe, dispatched in parallel.

**PM action item carried forward:**
- First-pass citation spot-check is still pending. Per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3, this is non-blocking but accruing. PM aims to clear it before the synthesis step that consumes this second-pass artifact. Calibration data continues to accumulate; second-pass review intensity remains somewhat-elevated.

---

**End of dispatch.**

*Janus second-pass: prior art for "release-on-context-switch" lock primitives, known bug patterns in such primitives, the Rust-kernel guard-type-meets-asm-switch idiom, and the cross-CPU lock-ordering disciplines that prevent or expose AB/BA cycles in IPC paths. Output at `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`. Research artifact only; no code, no project recommendations.*
