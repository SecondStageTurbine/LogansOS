# PM-NOTE: Redox DWRR scheduler — prior art for NeoPlutarch's within-class scheduler algorithm

**Date:** 2026-05-10
**Author:** PM
**Status:** Research triage; carry-forward declared for Phase 2 (or Phase 7) scheduler-algorithm ADR work.
**Subject:** Triage of the Redox OS DWRR scheduler announcement (April 2026) for applicability to NeoPlutarch's scheduler design.

---

## 1. Source and corroboration

Primary source: Redox OS blog, "RSoC 2026: A new CPU scheduler for Redox" at `https://www.redox-os.org/news/rsoc-dwrr/` (Redox's own site returned 403 to automated fetch during triage; content accessed via web-search snippets).

Corroborating sources:
- Phoronix: "Redox OS Introducing New CPU Scheduler For ~1.5x Performance In Heavy Tasks" (`https://www.phoronix.com/news/Redox-OS-New-CPU-Sched`)
- OSnews: "Redox gets new CPU scheduler" (`https://www.osnews.com/story/144733/redox-gets-new-cpu-scheduler/`)
- Madshrimps: same coverage
- Wikipedia: "Deficit round robin" — original DRR/DWRR algorithm description (Shreedhar & Varghese, 1995, originally for network scheduling)

Four independent sources confirm the work; no fabrication concern. Implementation by Akshit Gaur as part of Redox Summer of Code (RSoC) 2026.

## 2. Summary of the Redox work

Redox replaced their legacy plain Round Robin CPU scheduler with a Deficit Weighted Round Robin variant. Key technical details:

- DWRR groups processes into multiple queues with per-queue quantum + deficit counter (canonical algorithm; O(1) per dispatch).
- Per-round behavior: scan queues in priority order; for each non-empty queue, increment the deficit counter by the queue's quantum; dequeue and run tasks while the counter exceeds the per-task cost.
- Naive DWRR has a starvation failure mode: a high-priority queue with continuously-arriving work can exhaust its quantum repeatedly while low-priority queues never accumulate enough deficit to run.
- Redox's mitigation: an **interleaved** variant. Instead of letting one queue exhaust its full balance before moving to the next, the scheduler serves one round of the high-priority queue's work, then immediately checks if any lower-priority queue has enough tokens for a single dispatch, then loops back to the high-priority queue. Reduces starvation without abandoning priority weighting.

Measured outcomes on Redox's benchmarks:
- ~150 FPS gain in their `pixelcannon` 3D demo
- ~1.5× operations/sec on CPU-bound tasks
- Similar responsiveness improvements (measured via Redox's `schedrs` scheduling benchmark)

Implementation is in safe-Rust per Redox's general kernel discipline.

## 3. Applicability to NeoPlutarch

Bible Article 8.1 currently specifies a five-class priority scheduler:
- Real-time (RT)
- Inference
- Interactive
- Compute
- Background

Article 8.1 does NOT specify the within-class scheduling algorithm. That's an unwritten design choice that will surface during Phase 2 BUILD work (when SMP + per-CPU run queues land in code) or Phase 7 work (when the inference scheduler design is articulated).

DWRR's natural placement in NeoPlutarch:
- **Within Compute class** — weighted-fair-share among CPU-bound batch threads is the canonical DWRR use case.
- **Within Background class** — same reasoning; background threads benefit from fair share over priority preemption.
- **Probably NOT within RT class.** Real-time needs deterministic scheduling — EDF or fixed-priority preemptive — and DWRR's fairness focus is not latency-deterministic.
- **Probably NOT within Inference class.** Article 8.2 notes inference workloads have soft deadlines and accelerator-blocked behavior; EDF-style deadline-driven scheduling is the more natural fit for ANSPEC-001's deadline machinery.
- **Probably NOT within Interactive class.** UI threads expect snappy first-response; DWRR's deficit accumulation could feel laggy for a freshly-spawned interactive thread.

Redox's interleaved-DWRR variant is candidate prior art specifically for the starvation-mitigation question: if NeoPlutarch's Compute and Background classes share a CPU and both contain weighted queues, naive DWRR's failure mode is real and the interleaved variant is a sensible engineering response.

Bible Article 16.4 commits to performance regression tracking from Phase 2 onward (boot time, syscall latency, context-switch cost, inference throughput). Adding a scheduling-fairness benchmark (Redox's `schedrs` equivalent, or a custom NeoPlutarch bench) to that list is worth considering when Phase 2 closes. Cheap; lets us measure within-class fairness regressions early.

## 4. What does NOT transfer

- **Redox's framing of "we finally have priorities."** Redox migrated FROM plain round-robin (no priorities) TO DWRR (priorities). NeoPlutarch had a five-class priority structure in the bible from v1.0. DWRR isn't bringing priorities to NeoPlutarch; it'd be within-class fairness underneath the existing class structure. Different motivation, different scope.
- **Redox's specific benchmarks.** pixelcannon FPS gains aren't a meaningful regression target for an AI-native OS where Phase 7 inference scheduling is the load-bearing case.
- **Their migration discipline.** NeoPlutarch is greenfield; we don't have a legacy round-robin scheduler to migrate from. We can pick DWRR (or any other algorithm) without backward-compatibility constraints.
- **DWRR as a project-wide scheduler.** It's a within-class algorithm; the cross-class structure (Article 8.1) is unchanged.

## 5. Carry-forward declaration

This PM-NOTE is the canonical record for two carry-forward items:

**Carry-forward 1: ADR citation for scheduler within-class algorithm.** When Phase 2 BUILD work or Phase 7 design work surfaces the within-class algorithm choice, an ADR is required per Bible Article 18.1. This PM-NOTE is the citation source for the Redox DWRR prior art that the ADR will reference. The ADR will also need to consider: Linux CFS (Completely Fair Scheduler), FreeBSD's ULE scheduler, Theseus's scheduling discipline (already cited in `ALTIMIT-real-world-implementation.md`), and possibly the Asterinas framekernel scheduler design.

**Carry-forward 2: Bench surface expansion for Article 16.4.** When Phase 2 closes (or earlier if convenient), Article 16.4's enumerated benches gain a scheduling-fairness item. Either Redox's `schedrs` if portable, or a custom NeoPlutarch bench measuring within-class fairness regressions. Small bible amendment territory; bundled with Sub-step 6 docs closure dispatch (2026-05-10) as bible v1.4.0 amendment.

PM tracks both as pending in the broader Sub-step 6 docs dispatch queue (after SC-3B closes). PM will include them in the next post-SC-3B Sub-step 6 docs dispatch to Prometheus.

## 6. Cross-references

- `docs/bible.md` Article 8.1, 8.2, 8.3 — scheduler class structure
- `docs/bible.md` Article 16.4 — performance regression tracking
- `docs/bible.md` Article 18.1 — ADR mandate for second-guessable choices
- `docs/specs/neoplutarch-ai-subsystem-spec.md` (ANSPEC-001) §7 — Inference Scheduler; relevant for Phase 7 inference-class scheduling decisions
- `ALTIMIT-real-world-implementation.md` — broader prior-art research artifact; Theseus and Asterinas scheduler discipline referenced there
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` §2.1 — Redox's `switch_finish_hook` and per-CPU switch-state pattern (different mechanism from DWRR but same project; reinforces Redox as a citation channel for scheduler-internal mechanism design)

## 7. Calibration note (light)

Source quality assessment for this research triage:

- Primary source (Redox blog): could not fetch directly (403), but content captured via web-search index snippets; cross-confirmed against three independent secondary sources (Phoronix, OSnews, Madshrimps) all carrying consistent technical claims and numerical figures.
- Algorithm description: cross-confirmed against Wikipedia's DRR/DWRR article citing the original Shreedhar & Varghese 1995 paper.
- No anomalies; no fabrication signal; technical claims internally consistent across sources.

This is an external-source triage, not an agent-output review, so the calibration discipline established for Janus and Cerebro doesn't apply directly. The multi-source independent confirmation pattern is the analogous discipline for external research.

---

**End of note.**
