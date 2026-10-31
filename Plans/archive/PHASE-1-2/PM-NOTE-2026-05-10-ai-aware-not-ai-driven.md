# PM-NOTE — "AI-native" means AI-aware, not AI-driven (durable answer to a recurring critique)

**Date filed:** 2026-05-10 (later session)
**Author:** PM
**Type:** PM-NOTE. Doc-consolidation artifact. Doc-only. No bible/ANSPEC/ADR mutation; no scope change; no phase-roadmap implication. Purpose is to consolidate an answer that is currently scattered across Bible Articles 2.5, 8.1–8.2, 23.3, ANSPEC §7.6, and the `ALTIMIT-real-world-implementation.md` research artifact §J, into a single readable artifact the project can point to when the question recurs.
**Trigger:** PL surfaced a critique from external discussion on 2026-05-10: "Schedulers have to run and make decisions within less than microseconds. Full context switches take at most 10us nowadays. You cannot reasonably run anything AI related for scheduling without eating all gains you'll make, probably hundredfold. AI is non-deterministic and high-latency, these are two things you do not want in your OS control plane." The critique is correct on its premises and lands hard on the design it attacks. NeoPlutarch's Bible rejects that design explicitly. This PM-NOTE makes that rejection legible in one place.

---

## 1. The critique

Restated to make sure it's understood, not strawmanned:

- **Scheduler decisions are sub-microsecond hot paths.** A modern x86_64 kernel scheduler picks the next thread, performs the context switch, and resumes execution in well under 10µs end-to-end. The decision portion is hundreds of nanoseconds.
- **AI inference is millisecond-to-second.** A small classifier model is ~1ms at best; a 7B-parameter LLM forward pass is tens of milliseconds; anything agent-shaped is hundreds of milliseconds to seconds. The cheapest possible "AI" is three to four orders of magnitude slower than the decision it would be informing.
- **AI is non-deterministic.** Same input does not produce same output (sampling temperature, KV-cache state, model version drift). Non-determinism in a kernel control plane is anathema to debuggability, reproducibility, and real-time guarantees.
- **Therefore:** running AI in the scheduler decision path eats every performance gain hundreds-fold and introduces non-deterministic failure modes into the kernel's most-load-bearing code. Net negative.

**This critique is correct.** Every premise is right; the conclusion follows from the premises. The design the critique attacks is bad design and should be rejected.

## 2. The critique attacks a design NeoPlutarch does not have

The design the critique describes — AI invoked inside the scheduler decision path — is the **Heimdall / Windows 11 24H2 AI scheduler / AIOS LLM-as-kernel** direction. Per `ALTIMIT-real-world-implementation.md` §C:

> Microsoft MLOS, Linux 6.x I/O ML schedulers ("Heimdall"), Windows 11 24H2 AI scheduler — these are AI-*augmented* kernels, where an ML model runs in some component to optimize scheduling. They embed models in the kernel — a pattern Bible Article 23.3 explicitly rejects. The rejection is correct: embedding a specific model couples kernel ABI stability to model lifecycle, and any model bug becomes a kernel bug. AltimitNeo's posture (kernel arbitrates AI, kernel is not itself an AI) is the more disciplined design.

The field-survey artifact `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md` (PM-preliminary; Cerebro pass forthcoming) found the same pattern across most Sense-B agent-frameworks-branded-as-OS projects: OpenFang ships seven AI personas as named "Hands"; AIOS's design is LLM-as-kernel; Pneuma's product is "an OS that is pure AI"; Synthesis-OS says "The OS itself is the AI." Every one of these projects sits inside the trap the critique names.

**NeoPlutarch's Bible explicitly rejects all of these designs.** Five anchor clauses, scattered across the document:

### 2.1 Bible Article 2.5

> 2.5 This does not mean the kernel runs neural networks. It means the kernel schedules and protects them. The actual matmul happens in driver code, accelerator firmware, or a dedicated kernel inference module — but the kernel knows it is happening and to whom.

The kernel **schedules** AI workloads; the kernel does **not** invoke AI to make scheduling decisions. The decision plane and the workload are distinct.

### 2.2 Bible Article 8.1–8.2

> 8.1 The scheduler is a multi-class priority scheduler with the following classes, in priority order:
> - Real-time (RT) — bounded-latency threads, e.g., audio, input handling.
> - Inference — AI workloads with explicit deadline hints.
> - Interactive — UI and short-lived foreground threads.
> - Compute — CPU-bound batch work.
> - Background — opportunistic, yields readily.
>
> 8.2 Inference workloads have their own class because their scheduling is fundamentally different: they are bursty, often have soft deadlines (token-per-second targets), and frequently block on accelerator dispatch rather than CPU.

The Inference class exists because **AI workloads are scheduling subjects** with distinct characteristics (bursty, deadline-driven, accelerator-bound), not because the scheduler **invokes AI** to make decisions. Same factoring as Linux's `SCHED_FIFO`, `SCHED_RR`, `SCHED_DEADLINE`, `SCHED_BATCH`, `SCHED_IDLE`: priority classes for workload types, no learning component in the dispatcher.

### 2.3 Bible Article 23.3

> 23.3 We will not embed a specific model in the kernel. The kernel is model-agnostic.

Direct rejection of the design the critique attacks. The `ALTIMIT-real-world-implementation.md` §J argues this is the single most ALTIMIT-faithful clause in the project, on the explicit reasoning that "embedding a persistent model in the kernel converts every model bug, dataset bias, and prompt-injection vulnerability into a kernel bug. The Pluto's Kiss test fails immediately."

### 2.4 ANSPEC §7.6

> 7.6 Interaction with the main scheduler
>
> The inference scheduler is a sibling to the main thread scheduler, not a subordinate. They communicate:
> - A thread blocked on a synchronous inference call is parked by the main scheduler and woken by the inference scheduler upon completion.
> - An async inference call does not park the issuing thread.
> - Inference completion may itself trigger thread wake-ups via the standard wait mechanism.

**Two schedulers, deliberately decoupled.** The thread scheduler runs at sub-microsecond decision latency on the standard kernel hot path; it never invokes AI, ever. The inference scheduler runs at op-batch granularity (ANSPEC §7.3 specifies ~10ms quanta) on a separate code path. They communicate only via the same park/wake protocol the thread scheduler uses for any blocking call (I/O, IPC, lock acquisition). The inference scheduler is the thing that makes scheduling decisions *for AI workloads*; it is not the thing that uses AI to make scheduling decisions for anything else.

### 2.5 ANSPEC §7.3 — op-level granularity

> 7.3 Op vs model granularity
>
> Scheduling happens at op-batch granularity, not whole-model granularity. A long-running model (a 70B LLM forward pass) does not block a small interactive model (a wake-word classifier) — the small model's ops interleave at op boundaries.
>
> This requires that op execution be reasonably short. Ops that would individually take longer than a scheduling quantum (~10ms) are split by the scheduler into sub-ops where the algorithm permits (matmul tiling).

The inference scheduler decides which op runs on which accelerator using **earliest-deadline-first within priority class**, the same algorithm any real-time scheduler uses. No model invocation, no learning component, no non-determinism beyond the FIFO tie-break. The decision is cheap (microseconds on the inference scheduler's path), and the work the decision dispatches (the op itself) is the only thing that costs milliseconds.

## 3. The useful parallel: Linux schedules databases without invoking databases

The critique's framing implies a category error: "AI-native scheduler" must mean "scheduler runs AI." The parallel that disposes of this read:

- **Linux schedules database workloads as first-class entities.** PostgreSQL workloads can be placed in cgroups with CPU shares, memory limits, I/O priorities, and `SCHED_BATCH`-class priority. The kernel knows the database workload exists, applies appropriate isolation and accounting, but does not invoke PostgreSQL to decide what runs next.
- **Linux schedules video-encoding workloads the same way.** `nice` levels, ionice priorities, GPU compute queue arbitration via DRM. The kernel knows transcoding is happening; the kernel does not call into ffmpeg to make scheduling decisions.
- **Linux schedules GPU compute jobs the same way.** AMD ROCm and NVIDIA CUDA workloads get accelerator residency, memory pinning, and IRQ-driven completion notifications. The kernel arbitrates the accelerator; the kernel does not run CUDA kernels in the scheduler to figure out which thread to run next.

"Scheduler is X-aware" means **the scheduler knows X workloads exist** and applies appropriate priority, isolation, and accounting. It does not mean the scheduler **calls into X** to decide what runs next.

NeoPlutarch's claim — "kernel-resident AI primitives" — means the kernel has first-class **objects** for tensors, models, inference contexts, and accelerator devices (ANSPEC §3–§5). The kernel can therefore: arbitrate multi-tenant accelerator scheduling without userspace runtime coordination; content-dedupe model weights so two processes loading "the same" 70B model cost one copy not two (Bible Article 7.5, ANSPEC §8.2); apply capability semantics to tensors and models (Bible Article 12, ANSPEC §10); account per-process AI resource consumption (ANSPEC §11.1). None of these require the kernel to **run** AI on the hot path. They require the kernel to **know AI workloads exist** as distinct from "anonymous mmap region holding floats" or "thread blocked on opaque GPU dispatch."

## 4. Where AI primitives actually sit in the kernel (and why that's not the same thing)

To make this concrete: every place in ANSPEC where the AI subsystem touches the kernel runs **outside** the hot scheduler path.

| Subsystem touch | Hot scheduler path? | Cost |
|---|---|---|
| `sys_tensor_create` / `_read` / `_write` (§6.2) | No — syscall path | Microseconds; same class as any other I/O syscall |
| `sys_model_load` (§4.3) | No — explicit load | Milliseconds to seconds; called once per model lifetime |
| `sys_inf_run` (§5.1) | No — application explicitly requests inference | Returns when inference completes; thread is parked via standard wait mechanism |
| Inference scheduler op dispatch (§7.2–§7.3) | No — sibling scheduler, op-batch granularity | ~microseconds per dispatch decision; not in the main thread scheduler's hot path |
| Tensor allocator (§8.1) | No — explicit alloc path | Same class as any other large allocation |
| Deduplication BLAKE3 hash (§8.2) | No — load-time, once per model | Hashes a model file once; result cached for that model's lifetime |
| Capability check on AI handle (§10.2) | No — syscall entry path | Microseconds; same class as any other capability check |

There is no codepath in the design where the main thread scheduler does anything AI-related. There is no codepath where a context switch decision depends on inference output. There is no codepath where a model is invoked to make a kernel decision of any kind. The closest the kernel ever comes to AI work is dispatching an op-batch to an accelerator and being notified on completion — and that work is the **scheduling subject**, not the **scheduling decision**.

## 5. The critique's correct landing target

The critique is genuinely correct, and it lands squarely on real projects that deserve the criticism:

- **Linux Heimdall** (in-tree ML I/O scheduler experiments) — embeds a model in the kernel to predict I/O patterns. Adds inference latency to I/O completion handling. Critique applies.
- **Windows 11 24H2 AI scheduler** — uses ML inference to optimize thread placement on heterogeneous cores. Adds inference latency to dispatcher decisions. Critique applies.
- **AIOS / Cerebrum / agent-OS frameworks** — LLM-as-kernel; everything mediated through a userspace LLM call. Adds full LLM round-trip latency to every operation. Critique applies a fortiori.
- **Most Sense-B "Agent OS" projects** — userspace agent frameworks branded as OSes; the model is on the control plane by design. Critique applies in proportion to how kernel-shaped the project claims to be.

NeoPlutarch's Bible Article 23.3 was written precisely to refuse the move all of these projects make. The Bible-23 stance is not a stylistic preference; it is a load-bearing engineering decision that keeps NeoPlutarch out of the trap the critique identifies. The critique is, accidentally, a strong endorsement of Bible Article 23.3.

## 6. Why this question will keep recurring

The phrase "AI-native operating system" has become loaded with the design the critique attacks, because most projects using the phrase are doing that exact thing. A reader encountering NeoPlutarch's framing reasonably assumes — given the field's vocabulary — that NeoPlutarch is doing what OpenFang or AIOS or Heimdall is doing. The critique is the natural response to that assumed design.

The answer to the critique is currently scattered across:

- `docs/bible.md` Article 2.5 (kernel schedules, does not run, neural networks)
- `docs/bible.md` Article 8.1–8.2 (Inference scheduling class for AI workloads, not via AI)
- `docs/bible.md` Article 23.3 (no embedded model in kernel)
- `docs/specs/neoplutarch-ai-subsystem-spec.md` §7.3 (op-batch granularity scheduling)
- `docs/specs/neoplutarch-ai-subsystem-spec.md` §7.6 (sibling-scheduler architecture)
- `ALTIMIT-real-world-implementation.md` §C and §J (rationale for rejecting Linux Heimdall / Windows 11 / AIOS pattern)
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md` (audit showing most "AI-native OS" projects sit in the trap)

A reader has to triangulate seven different documents to find the answer. This PM-NOTE consolidates the answer in one place so the next time the critique surfaces — and it will, repeatedly, as the field-survey shows — there is a single artifact the project can point to.

## 7. Forward considerations (not actions, just markers)

**7.1 Possible future Bible Article 23.4 or 23.5.** The `ALTIMIT-real-world-implementation.md` triage queue already proposes Article 23.4 (verifiable kernel identity, no narrative identity). A subsequent article making the AI-aware-not-AI-driven distinction explicit at Bible level would close the gap durably:

> *Proposed (PL-tier; not a PM action):* The scheduler is AI-aware, not AI-driven. The kernel's control plane is model-free by construction: no learning component runs in the thread scheduler's decision path, no model is invoked to make any sub-microsecond kernel decision, and no inference output gates a context switch. AI workloads are scheduling subjects (priority class, deadline, accelerator residency, capability ownership); they are not scheduling decisions.

PM does not propose this as a today action — Article 23 stance is already in force, and consolidation in this PM-NOTE is sufficient for now. If the question recurs frequently enough to warrant Bible-level clarity, PL can amend.

**7.2 External-communication clarity.** When NeoPlutarch is publicly described, the phrase "kernel-resident AI primitives" reliably triggers the critique. Future external-facing material (README, public-launch documentation, eventual ATC/USENIX paper) should pre-empt the misreading with explicit "AI-aware, not AI-driven" framing in the abstract / opening paragraph. README rewrite already happened in Sub-step 6 (commit `27f2f8f`) under Prometheus; a future README pass could fold this distinction in directly. Not a today action.

**7.3 Cerebro field-survey closure.** When `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` lands (via the dispatched Cerebro pass), the Sense-A/B/C/D taxonomy will become Cerebro-confirmed rather than PM-preliminary. That artifact's published version is the right place to anchor "the field looks like X; NeoPlutarch is doing Y, not X" for any future external discussion. Not a today action; flagged for when Cerebro returns.

## 8. Closure

The critique is right about its premises and right about the conclusion that follows from those premises. The design it attacks is bad design. NeoPlutarch's Bible explicitly refuses that design at five distinct anchor points (Articles 2.5, 8.1–8.2, 23.3; ANSPEC §7.3, §7.6). The architecture has two schedulers, decoupled, with no AI invocation in the main thread scheduler's hot path under any circumstance, ever.

"AI-native operating system" in NeoPlutarch's sense means **AI-aware**, not **AI-driven**: the kernel knows tensors, models, and inference contexts exist as distinct first-class objects (so it can arbitrate, isolate, account, and dedupe them); the kernel does not run AI to make scheduling decisions.

The parallel that disposes of the misreading: Linux schedules databases as first-class workloads without invoking the database to decide what runs next. NeoPlutarch schedules AI workloads as first-class workloads without invoking AI to decide what runs next. Same factoring; different subject; well-trodden engineering pattern.

No bible/ANSPEC/ADR mutation. No scope change. The artifact this PM-NOTE produces is a single readable answer to point at when the critique recurs.

---

## 9. Cross-references

**Bible / ANSPEC anchor points consolidated:**
- `docs/bible.md` Article 2.5 — kernel schedules and protects neural networks; does not run them.
- `docs/bible.md` Article 8.1 — multi-class priority scheduler; Inference is a class, not an invocation.
- `docs/bible.md` Article 8.2 — rationale for Inference class (bursty, deadline-driven, accelerator-bound; not "needs AI to schedule").
- `docs/bible.md` Article 23.3 — no embedded model in kernel; kernel is model-agnostic.
- `docs/specs/neoplutarch-ai-subsystem-spec.md` §5 — inference context as scheduled entity, not as decision-making component.
- `docs/specs/neoplutarch-ai-subsystem-spec.md` §7.2–§7.3 — inference scheduler design (EDF within priority class; op-batch granularity).
- `docs/specs/neoplutarch-ai-subsystem-spec.md` §7.6 — sibling-scheduler architecture; explicit decoupling from main thread scheduler.

**Research-artifact references:**
- `ALTIMIT-real-world-implementation.md` §C (Linux Heimdall / Windows 11 24H2 / AIOS pattern critique).
- `ALTIMIT-real-world-implementation.md` §J (ambient AI as substrate vs. system persona).
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md` (Sense-A/B/C/D taxonomy; most "AI-native OS" projects sit in Sense B's trap).

**Future-marker links (not today actions):**
- Possible Bible Article 23.4 or 23.5 making the AI-aware-not-AI-driven distinction explicit at constitutional level.
- Future README revision folding the distinction into the project's public framing.
- `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` (when Cerebro pass lands) as the published artifact anchoring this distinction publicly.

**Operational discipline anchors:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` — PM-tier doc-consolidation is PM-owned operational work; no PL ratification required.
- Bible Article 18.1 — ADRs / PM-NOTES are normative artifacts; this PM-NOTE is consolidation, not amendment.

---

**End of PM-NOTE.**

*The critique is correct on its premises. The design it attacks — AI inference in the scheduler decision path — is exactly the design Bible Article 23.3 refuses. NeoPlutarch's "AI-native" means AI-aware (kernel has first-class objects for tensors, models, inference contexts; arbitrates accelerator multi-tenancy; capability-protected; deduped) not AI-driven (no model invoked in scheduler decision path, ever). Two schedulers, decoupled: main thread scheduler operates at sub-microsecond decision latency with zero AI invocation; inference scheduler operates at op-batch granularity with EDF-within-priority-class. The parallel: Linux schedules databases as first-class workloads without invoking databases. Five Bible/ANSPEC anchor points consolidated into one readable answer.*
