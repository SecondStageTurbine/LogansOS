# PM-NOTE: Convergence — wedge investigation + §Q9 redo + SC-3A reaper assumptions

**Date:** 2026-05-08 (drafted late 2026-05-07, dated for tomorrow's planning conversation)
**Author:** PM
**Type:** Project-strategic finding. Names a three-way convergence between (1) the test-runner intermittent wedge investigation, (2) the §Q9 redo design notes, and (3) SC-3A's pre-implementation reaper-composition analysis. Recommends that the three be treated as one work stream rather than three. Doc-only artifact; informs but does not bind future planning. The actual sequencing decision (do we tackle the convergence before, during, or after SC-3A) is a project-lead governance call per Bible Article 24.1.
**Authority:** PM. Per Bible Article 18.1, this PM-NOTE is a research artifact; any normative claim it implies requires an ADR or amendment to take effect.
**Amended:** 2026-05-08 — §10 added after the H1 validation experiment refuted H1's narrow form. Original sections §1–§9 preserved as written; §10 captures what changed. The Alpha-prime recommendation in §6 is now superseded by §10's updated picture — see §10.4.

---

## 1. The convergence, in one sentence

Three independently-developed project artifacts — Jarvis's wedge investigation notes, the §Q9 redo design notes, and the SC-3A pre-implementation checkpoint pre-thinking — all centrally analyze the *same code path* (the BSP-scans-all-CPUs `reap()` in `kernel/src/sched/scheduler.rs`), each from a different angle, and one of them (SC-3A) treats as a feature what another (the wedge investigation) identifies as a bug. They cannot all three be correct simultaneously; resolving the contradiction is the sub-commit 3a-vs-3b sequencing decision.

## 2. The three artifacts

### 2.1 Jarvis's wedge investigation (`Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md`)

Empirical analysis of 5 wedge instances across 4 diagnostic campaigns. Two signatures (`smoke-test-partial-reap` ×4, `smp-hog-partial-reap` ×1), single root-cause hypothesis (H1):

> The BSP's `reap()` iterates all per-CPU schedulers and checks `zombie_count` (lock-free Acquire load) before acquiring the lock. If an AP produces a zombie between the BSP's `zombie_count == 0` check and the BSP's next iteration of that CPU, the zombie is missed.

Structurally consistent with all observed wedges. Confirming evidence: partial-reap pattern (some threads reaped, others not). Refuting evidence: none observed.

### 2.2 The §Q9 redo design notes (`Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md`)

Design-level analysis of what a future §Q9 replacement must address. §3.3 ("Avoid AP-side reaper participation until threads can route to APs") states explicitly that the BSP-scans-all-CPUs stopgap is *acceptable for sub-commit 2 only because no production thread is routed to APs yet*; AP zombie queues are empty in steady state, so the cross-CPU scan is a bounded no-op. The note then names this as a constraint:

> **Any later sub-commit that introduces cross-CPU thread movement (sub-commit 3b's wake-on-completer-CPU) must revisit reaper distribution.**

The §Q9 redo notes were written assuming the stopgap held cleanly under SC 2's load. Jarvis's wedge data suggests that assumption was already false — partial-reap wedges occur even with the BSP-only design, before cross-CPU thread movement has been introduced.

### 2.3 SC-3A pre-implementation pre-thinking (`Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`)

Lock-order × cross-CPU reap composition analysis. SC-3A's mechanism (ManuallyDrop + asm release per ADR-013 §Q6) is shown to compose correctly with the BSP-scans-all-CPUs reaper. §3.4 explicitly notes the lock-free short-circuit:

> The reaper short-circuits when `ZOMBIE_COUNT == 0`, avoiding the per-CPU lock acquisitions. This is preserved under SC 2's BSP-scans-all-CPUs design.

The §3.5 conclusion ("composition confirmed") rests on this property: the reaper takes no per-CPU locks in the common case (when `zombie_count == 0`), so it cannot deadlock with SC-3A's asm release path. **The lock-free short-circuit is what makes SC-3A's correctness analysis clean.**

## 3. The contradiction

The same lock-free `zombie_count == 0` short-circuit is:

- **Per Jarvis's H1:** the racy code path that causes the wedge. The short-circuit reads `zombie_count` outside any lock; an AP's zombie production between read-and-skip can be missed.
- **Per SC-3A's pre-thinking §3.4–§3.5:** the optimization that keeps the reaper from contending with cross-CPU asm-release operations. Removing it would introduce per-iteration lock acquisitions on every AP scheduler.

Both analyses are internally correct. The contradiction is in their conclusions about the same code:

- **If H1 is right**, the short-circuit must be removed or hardened (e.g., re-check under the lock, or use a stronger memory ordering with explicit fences). Either change introduces lock acquisitions or fence costs the SC-3A composition analysis didn't account for.
- **If SC-3A's composition analysis is right** as written, the short-circuit should be preserved. But then H1's explanation of the wedge cannot stand, and the wedge has a different root cause we haven't identified.

The most parsimonious resolution: **H1 is right, the short-circuit is racy, and SC-3A's composition analysis needs to be redone against a hardened reaper**. This is the working hypothesis for the rest of this note.

> **Status as of §10:** This "most parsimonious resolution" was tested by experiment and refuted. See §10. The contradiction itself remains real; only the resolution changes. SC-3A's composition argument is now *less* in tension with the wedge data than this section feared, because the wedge does not appear to be the lock-free short-circuit at all.

## 4. Why the contradiction wasn't caught earlier

Each artifact was authored against a different evidence base:

- **§Q9 redo notes** (early 2026-05-07): drafted while sub-commit 2 was being closed. Wedge data was minimal (1-2 instances known). The notes assumed the wedge was an artifact of the test harness, not a kernel bug.
- **SC-3A pre-thinking** (2026-05-07 12:46): drafted same day, based on the §Q9 redo notes' framing and ADR-013 §Q6's design. Inherited the assumption that the BSP-only stopgap was correct in steady state.
- **Wedge investigation** (later 2026-05-07): Jarvis catalogued the actual evidence after KASLR-Q1B's TIMEOUT distribution made the wedge undeniable. He arrived at H1 by reasoning forward from the kernel code, independently of the §Q9 redo notes — which is why the convergence is striking. Two independent code-reading exercises landed on the same culprit.

Each artifact was correct relative to what its author knew. The convergence emerges only when all three are read together.

## 5. The PATCH1 wedge data sharpens the picture

ADR-015's PATCH1 50-run loop (`target/verify/ADR-015-PATCH1-loop-summary.txt`) produced 11 TIMEOUTs across 9 different scenarios:

```
Run 4:  buddy_alloc_free_alloc_returns_same_frame
Run 11: cap_survive_exit
Run 13: slab_local_to_global_promotion
Run 15: cap_revoke_local
Run 17: cap_survive_exit
Run 21: smep_catch
Run 35: smp_per_cpu_scheduler_smoke
Run 36: cap_revoke_descendants_walk
Run 38: expect_pass_but_fail
Run 40: kaslr_heap_contract
Run 42: cap_survive_exit
```

Only 1 of 11 wedges occurred on `smp_per_cpu_scheduler_smoke` — the scenario most directly stressing AP-resident threads. The other 10 spread across 8 scenarios that *don't* explicitly route threads to APs. Per Jarvis's §4.2 correlation analysis, the hang locus in those cases is the in-kernel scheduler smoke test that runs at the start of every scenario, before any user-space test code executes. The hang is not scenario-specific; it's pre-scenario.

This confirms two things:

1. **The wedge is not "smp_per_cpu_scheduler_smoke is buggy."** The smoke test in `main.rs` runs ping/pong/hog at every boot, before any scenario, and that's where 10 of 11 wedges manifest. The scenario name in the close report reflects which scenario was running when the timeout fired, not which scenario caused the wedge.

2. **The wedge does not require explicit AP routing to manifest.** Even with all production threads on the BSP, the wedge fires. This is consistent with H1 if APs produce *kernel* zombies (e.g., per-CPU bootstrap threads transitioning state under timer interrupts) that the BSP's reap scan misses. It also opens the door to alternative hypotheses where the bug is not in cross-CPU coordination at all — e.g., a single-CPU race between the timer interrupt handler and the reaper.

The §Q9 redo notes' §3.3 framing — "AP zombie queues are empty in steady state" — may be empirically wrong. If APs produce kernel zombies (idle threads transitioning, bootstrap threads finishing init, etc.), the queues are not actually empty, and the BSP's lock-free short-circuit can race with their production.

> **Status as of §10:** Observation 2's "opens the door to alternative hypotheses" was prescient. The H1 experiment's data shows AP schedulers produced *no* zombies during 88 active reap observations — kernel zombies on APs are also not the explanation. The wedge cause is genuinely elsewhere. See §10.

## 6. Implications for SC-3A's sequencing

Three options for sequencing the fix relative to SC-3A:

### Option Alpha: Fix the reaper before SC-3A

**Mechanism:** Land §Q9-redo (or a smaller hardening of the lock-free short-circuit) as a separate sub-commit before SC-3A starts. SC-3A's pre-thinking analysis is then redone against the hardened reaper.

**Pros:**
- SC-3A's correctness analysis is performed against the correct (post-fix) substrate.
- The wedge stops contaminating SC-3A's verification gate.
- The wedge-fix commitment (`Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md`) is met early rather than late.

**Cons:**
- Adds a sub-commit before SC-3A, delaying SC-3A's start.
- The reaper fix may itself require careful design (hardening the short-circuit vs. full §Q9 redo with `live_thread_count` is a real design choice).
- The §Q9 redo notes argue (§3.4) that `live_thread_count` work should be co-scheduled with multi-thread-per-process spawn, which doesn't exist yet. Doing the full §Q9 redo now means doing it without the test substrate (`smp_concurrent_process_exit`, §3.5) that would catch the relevant bugs.

### Option Beta: Fix the reaper as part of SC-3A

**Mechanism:** Expand SC-3A's scope to include the reaper hardening. SC-3A becomes "asm release primitive + reaper short-circuit fix" as a single bundled commit.

**Pros:**
- The composition analysis happens once, against the final substrate.
- One commit fixes two related issues.
- The wedge fix lands at the same milestone as SC-3A.

**Cons:**
- Violates the project's "single focused commit" discipline. SC-3A's stated scope is one mechanism; bundling the reaper fix dilutes that.
- Increases the correctness-risk of an already-high-risk commit. Asm release primitives are unforgiving; debugging a reaper race in the same commit doubles the failure modes.
- If the reaper fix turns out to require deeper rework (e.g., needs `live_thread_count`), SC-3A is held hostage to that rework.

### Option Gamma: Fix the reaper after SC-3A, before SC-3B

**Mechanism:** SC-3A lands as currently planned, with the wedge as known baseline noise. The reaper fix is its own work between SC-3A and SC-3B. SC-3A's pre-thinking analysis is documented as "valid only for the duration of the BSP-only stopgap; revisit before SC-3B."

**Pros:**
- SC-3A's scope stays focused.
- The reaper fix is its own commit with its own verification gate.
- Honors the wedge-fix-before-SC-3B commitment.
- Buys time to decide between minimal hardening (just fix the short-circuit) vs. full §Q9 redo.

**Cons:**
- SC-3A's verification gate accepts ~22% TIMEOUT noise (per the ADR-015 PATCH1 baseline), which means SC-3A's load-bearing claims are made against a noisy substrate.
- The "redo SC-3A's composition analysis after the reaper fix" step has to actually happen; if it's deferred, we end up with SC-3A's correctness rationale resting on an outdated reaper assumption.

### Recommended option

**Alpha-prime: minimal hardening of the lock-free short-circuit, before SC-3A; full §Q9 redo deferred to multi-thread-per-process spawn.**

The minimal hardening would replace:

```rust
if scheduler.zombie_count.load(Acquire) == 0 { continue; }
let zombies = without_interrupts(|| {
    let mut s = scheduler.lock();
    mem::take(&mut s.zombies)
});
```

with:

```rust
let zombies = without_interrupts(|| {
    let mut s = scheduler.lock();
    mem::take(&mut s.zombies)
});
if zombies.is_empty() { continue; }
```

— always acquire the lock, drain whatever's there, and treat the empty case as the no-op. This costs one lock acquisition per CPU per reaper iteration (bounded, single-digit-microsecond overhead) and eliminates the race entirely. It does not require `live_thread_count` or multi-thread-per-process design; it is a targeted fix to the H1 race.

SC-3A's pre-thinking analysis would need a one-paragraph addendum explaining that the lock acquisition replaces the short-circuit, and that the composition argument now rests on "lock acquisition is bounded and cannot block on cross-CPU asm-release operations." This is straightforward to verify: the per-CPU scheduler lock is held for a short critical section in `reap()` (just the `mem::take`), and the asm-release path doesn't acquire any per-CPU lock that the reaper could contend with.

Full §Q9 redo (with `live_thread_count`, AP-side reapers, and the `smp_concurrent_process_exit` test) remains correctly deferred to whenever multi-thread-per-process spawn lands. The wedge fix does not require it.

> **Status as of §10:** The Alpha-prime recommendation in this section is **superseded** by §10. The H1 validation experiment ran 20 boots with the lock-free short-circuit removed (i.e., effectively running Alpha-prime's behavior); the wedge still fired. Alpha-prime is therefore not the wedge fix. See §10.4 for what the recommendation now looks like.

## 7. What this note recommends for tomorrow's conversation

1. **Validate H1 with one targeted experiment** before committing to a fix path. Run the diagnostic dispatch sketched in Jarvis's investigation §6 (Q3 specifically): instrument `reap()` to log each CPU's `zombie_count` before-and-after lock acquisition, run 20× `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4`, observe whether the count differs between the lock-free read and the post-lock state. ~2 hours of Cerebro work. If the experiment shows the race directly, H1 is confirmed and Alpha-prime is the right fix. If it doesn't, the convergence picture needs revisiting before any fix lands.

2. **Defer the sequencing decision until after the validation experiment.** Project lead chooses Alpha, Alpha-prime, Beta, or Gamma based on what the experiment shows. PM cannot make this call; it's a project-shape decision.

3. **Update SC-3A's pre-thinking** with a note pointing to this convergence document. The §3.4–§3.5 composition analysis should be flagged as "valid against the current (broken) reaper; revisit when reaper is hardened." This is a small in-place edit Jarvis can do as a doc-only commit.

4. **Update the wedge-fix commitment record** with the convergence finding. `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` named the deadline (before SC-3B) without naming the mechanism; this convergence note is the mechanism.

5. **Tracking entry in INDEX.md.** When the "Open project commitments" section is added (per the wedge-fix commitment note's §what-this-note-will-be-used-for), entry 1 is the wedge fix and this convergence note is its primary supporting analysis.

> **Status as of §10:** Item 1 was executed. Items 2 and 4 are partially superseded by §10 — sequencing is still deferred, but the mechanism that fix takes is no longer specifically Alpha-prime. Items 3 and 5 are unchanged.

## 8. What this note explicitly does NOT do

- It does not propose an ADR. The minimal hardening is a kernel patch, not an architectural choice; it can land via dispatch + close-report cycle without an ADR. The full §Q9 redo, when scheduled, will need ADR-016 (per the §Q9 redo notes' framing), but that's separate from this convergence finding.
- It does not pre-empt the project lead's sequencing decision. Alpha, Alpha-prime, Beta, Gamma are all defensible; the experiment in §7 item 1 changes which is best.
- It does not commit to the H1 hypothesis as proven. Jarvis's analysis is strong, the convergence is striking, but only a controlled experiment confirms it. The validation step is non-negotiable before code lands.
- It does not redo SC-3A's pre-thinking. That's its own work, after the experiment, under whichever option is chosen.

## 9. Cross-references

**Primary sources (the three converging artifacts):**
- `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md` — Jarvis's investigation; H1 hypothesis at §5.
- `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` — §Q9 redo design; the BSP-scans-all-CPUs analysis at §3.3.
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — SC-3A composition analysis; the lock-free short-circuit at §3.4–§3.5.

**Empirical evidence:**
- `target/verify/ADR-015-PATCH1-loop-summary.txt` — 11 wedges across 9 scenarios in PATCH1 50-run loop; sharper distribution than KASLR-Q1B's data alone.
- `target/verify/KASLR-Q1B-main-run-3.buddy_max_order.serial.log` — Signature A reference instance.
- `target/verify/ADR-015-loop-run-1.smp_per_cpu_scheduler_smoke.serial.log` — Signature B reference instance.
- `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` — 14/50 (28%) baseline wedge rate.
- `Plans/CLOSE-REPORT-2026-05-07-adr-015-implementation.md` — 11/50 (22%) post-patch wedge rate; consistent with H1 (the bug is unaffected by ADR-015's heap fix).

**Project-state references:**
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — wedge fix lands before SC-3B; this convergence note is the mechanism that fix likely takes.
- `Plans/PM-DECISION-2026-05-06-sub-commit-2-bisect-rollback.md` — original §Q9 deferral decision; established the BSP-scans-all-CPUs reaper that the wedge investigation has now identified as racy.
- `Plans/PM-DECISION-2026-05-06-sub-commit-2-close-without-q9.md` — sub-commit 2's closure-with-Q9-deferred decision; the working assumption that the stopgap was clean is now empirically falsified.

**Code references:**
- `kernel/src/sched/scheduler.rs` `reap()` — the function containing the lock-free short-circuit.
- `kernel/src/main.rs:173` — `while any_user_process() { yield_now(); reap(); }` — the wait loop where the wedge manifests.
- `kernel/src/sched/process.rs:634` — `any_user_process()` — what the wait loop checks; stays true when zombies are missed.

**Bible articles:**
- Article 8.3 — SMP per-CPU state discipline; the lock-free short-circuit may violate this in spirit by reading per-CPU state without synchronization.
- Article 16.3 — real boot verification at every phase close; SC-3B's gate cannot meet this with ~22% TIMEOUT noise.
- Article 24.1 — project lead authority over sequencing decisions.

---

## 10. Updated picture post-H1 experiment (added 2026-05-08)

The H1 validation experiment (see `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md`) ran on the substrate of `47972c2` plus throwaway `reap()` instrumentation. Twenty runs of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4`, with the lock-free short-circuit removed and forced lock acquisition on every per-CPU scheduler scan.

### 10.1 What the experiment found

- **88 active zombie observations across 20 runs.** Every observation had `z_pre == z_post`. **Zero divergences.** Zero H1-signature cases (`z_pre=0,z_post>0`).
- **All zombie activity on cpu=0.** AP schedulers (cpu=1,2,3) were initialized and scanned but produced no zombies during the runs.
- **One TIMEOUT in 20 runs (5%, sample-noisy).** Run 4 wedged with the experiment already running Alpha-prime's behavior — i.e., the lock-free short-circuit was already removed. The reaper drained one zombie correctly (`reaped 'ping' (id=4)`) and then the kernel made no further progress until QEMU timed out.

### 10.2 Why H1 in its narrow form is refuted

The H1 race requires AP-side zombie production between the BSP's pre-lock read and lock acquisition. The experiment observed zero AP-side zombie production across thousands of reap iterations. The race the hypothesis named cannot fire if APs do not produce zombies — and in this scenario, on this substrate, they do not.

This refutation is **scoped to the measured scenario and substrate**. It does not prove that an AP-zombie race is impossible in a future test that actually routes threads onto AP scheduler queues. The current `smp_per_cpu_scheduler_smoke` is misnamed in this respect — it calls `sched::spawn(...)` from the BSP, which enqueues on `current_scheduler()` (the BSP's), so its `smp-hog-*` workers are BSP-local. The scenario name suggests AP scheduler coverage; the implementation does not deliver it.

### 10.3 What the run-4 timeout tells us

Run 4 is the most strategically informative data point in the experiment. The forced-lock reaper drained correctly. The kernel reached `[ pong ] 5`, reaped `ping`, and then went silent until QEMU timed out. **The wedge happened anyway, in the absence of any cross-CPU race, with the reaper functioning correctly.**

The run-4 timeout's serial signature implicates code outside `reap()`. Plausible suspects:

- **The smoke test's wait loop** (`while any_user_process() { yield_now(); reap(); }` per the §Q9 redo notes' code references). If `any_user_process()` returns `true` forever — because a process is registered but its threads aren't all being deregistered — this loops forever even when `reap()` itself works correctly. The bug would be in `on_thread_reap`'s registry interaction, not in `reap()`'s zombie drain.
- **The scheduler's idle/yield path.** If a CPU goes to halt and never wakes (e.g., a missed IPI or a wait condition that never fires), the wait-loop's `yield_now()` doesn't return on that CPU. Other CPUs continue but the wedged CPU is stuck.
- **A deeper mechanism in the smoke test's spawn/exit pattern.** If `ping`/`pong`/`smp-hog-*` are spawned in a way that produces a process that's registered without all its threads being tracked, deregistration fails silently.

The H1 experiment did not instrument any of these paths. We do not yet have data on which of them is implicated.

### 10.4 What the recommendation now looks like

**Alpha-prime is no longer recommended as the wedge fix.** The experiment ran with Alpha-prime's behavior in place, and the wedge still fired. Landing Alpha-prime as a "wedge fix" would be shipping a tightening that does not actually do what its commit message would claim.

Two adjusted dispositions for Alpha-prime:

- **As a defensive correctness tightening:** the lock-free short-circuit is still racy in principle (the experiment did not prove it cannot race; it proved that this particular workload doesn't trigger it). Future workloads — sub-commit 3b's wake-on-completer-CPU, multi-thread-per-process spawn, anything that puts threads on APs — will produce AP-resident zombies, and at that point Alpha-prime's tightening becomes load-bearing. Landing it now would prevent a future race we'd otherwise have to chase.
- **Shelved entirely until evidence specifically supports it:** if we don't land Alpha-prime now, SC-3A's composition argument (which depends on the short-circuit) holds against current evidence. SC-3A can ship without the tightening. When future workloads need the tightening, we land it then.

PM does not have a strong preference between these; it's a project-lead call. The convergence note's original §6 "Recommended option" is superseded; whichever disposition is chosen, the choice should be recorded as its own PM-DECISION or ADR.

**The wedge-fix commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` still holds.** The deadline (before SC-3B) is unchanged. What changes is that we don't yet know what fix to write. The next investigation focuses on run 4's mechanism.

### 10.5 What changes in §7's recommendations

- **Item 1 (validate H1):** done. Result: refuted in narrow form for the measured scenario.
- **Item 2 (defer sequencing decision):** still deferred. The new investigation must complete before the sequencing decision can be made on solid ground. PM cannot recommend a sequencing option without a known fix mechanism.
- **Item 3 (update SC-3A pre-thinking with a "valid against the current reaper" note):** still recommended, but the framing changes — instead of "valid until the reaper is hardened," it is now "valid against the current reaper, which the H1 experiment shows is empirically not the wedge cause for current workloads." SC-3A's composition argument is *less* in tension with reality than this convergence note originally suggested.
- **Item 4 (update wedge-fix commitment):** still recommended, but the commitment's mechanism field stays "to be determined" rather than naming Alpha-prime.
- **Item 5 (INDEX.md "Open project commitments"):** done at `46984a1`. Entry 1 is the wedge fix, with this convergence note as primary supporting analysis. The cross-reference is now to §10 specifically, not just to the convergence note as a whole.

### 10.6 What the next investigation needs to address

Q1 from PM's post-experiment analysis: **what stopped run 4 from making progress?** This is the most direct path forward — we have one concrete reproduction with full serial log, no exotic conditions, and the cause must be in code we haven't yet instrumented.

Secondary questions deferred to later investigations:

- Q2: is run 4's wedge the same as Jarvis's catalogued wedges, or a different signature?
- Q3: is `smp_per_cpu_scheduler_smoke`'s misnaming itself a bug worth fixing? (Project hygiene; deferable.)
- Q4: what's the wedge rate when measured controlled with vs. without lock-forcing? (Side question; not blocking.)
- Q5: are there scenarios that wedge but aren't smoke-test-related?

The next dispatch (`Plans/DISPATCH-2026-05-08-wedge-investigation-2-cerebro.md`) targets Q1 directly and includes a small Q4 measurement as a free side-effect.

### 10.7 Cross-references added by this amendment

- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — the experiment that refuted H1's narrow form.
- `Plans/PM-DECISION-2026-05-08-wedge-investigation-continues.md` — PM decision recording the choice to keep investigating rather than land Alpha-prime as an unmotivated tightening.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-2-cerebro.md` — the next investigation dispatch (when written).
- `target/verify/H1-EXPERIMENT-run-*.serial.log` — raw experimental data.
- `target/verify/H1-EXPERIMENT-run-4.serial.log` — the specific timeout instance whose mechanism is the next investigation target.

---

**End of convergence note.**

*Originally written for tomorrow's planning conversation; amended after the H1 validation experiment refuted the parsimonious resolution. The contradiction the convergence names is real; the resolution is now an open question the next investigation aims to close.*
