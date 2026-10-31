# ADR-001: Monolithic Kernel Reaffirmation

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-05-02 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Articles 4.1, 2.4, 6, 17, 27 · ANSPEC-001 §1.2 · `docs/design.md` v0.2 → v0.3 |

---

## Context

The NeoPlutarch project has two foundational documents that, until now, contradicted each other on the most fundamental architectural question — what kind of kernel are we building.

**The NeoPlutarch Bible**, ratified at v1.0.0, states unambiguously in Article 4.1:

> NeoPlutarch is a **monolithic kernel with disciplined module boundaries**, not a microkernel. We chose this because:
>
> - Microkernel IPC overhead is hostile to inference workloads where data is large and frequent.
> - Driver-as-userspace is a Linux-era assumption; the cost/benefit collapses when most "drivers" are accelerator firmware blobs anyway.
> - Monolithic does not mean unstructured. Internal module boundaries are enforced by Rust's module system and crate boundaries.

Article 2.4 reinforces this:

> We reject the "AI is a service over a socket" model (LM Studio, Ollama, llama.cpp daemons). Inference is a syscall family, not an IPC dance.

Article 6 places the AI subsystem in `kernel/src/ai/` with syscalls in the `0x100..0x1FF` range — kernel-resident, not service-resident.

**The design document** (`docs/design.md` v0.2), authored before the Bible's ratification, framed the project as a "Pure microkernel" with the AI subsystem decomposed into three userspace services communicating via IPC:

- `neo-inference` — a privileged userspace daemon owning model execution.
- `neo-models` — a system service treating models as filesystem objects, loaded by the daemon.
- `neo-intent` — a publish/subscribe fabric for AI-derived events, also userspace.

`design.md` v0.2 §1 listed "Pure microkernel" as the first design pillar. §2 named the architecture "a pure microkernel with capability-based security, taking inspiration from seL4, Redox OS, and Fuchsia's Zircon." §4 built the entire AI subsystem on top of this assumption.

This is a direct contradiction with Bible Articles 4.1, 2.4, and 6. It is not a wording difference; it is a divergent architecture.

The contradiction was identified during a deviation audit prior to Phase 2 work. Eleven deviations from the Bible were catalogued; this one was rated CRITICAL because every Phase 2 PR — capability system, SMP, allocator work — would otherwise be litigating the ambiguity in code review.

This ADR resolves it.

## Decision

**The Bible is authoritative. NeoPlutarch is a monolithic kernel with disciplined module boundaries. The microkernel framing of `design.md` v0.2 is rejected and superseded.**

Concretely:

1. The AI subsystem is kernel-resident, per Bible Article 6 and ANSPEC-001 §1–5. Tensors, models, and inference contexts are kernel objects. Inference is a syscall family in the `0x100..0x1FF` range, not IPC to a userspace daemon.

2. `neo-inference` (as a userspace daemon owning all model execution) is removed from the architecture. Its responsibilities — multi-tenant request scheduling, KV cache management, hardware affinity, priority enforcement — move to the kernel's inference scheduler (ANSPEC-001 §7). The name is retired.

3. `neo-models` is preserved as a *userspace registry and CLI tool* — `neo-models list`, `neo-models load`, `neo-models set-default --role coding ...`. It is not a service that loads or executes models. The kernel loads models via `sys_model_load` (ANSPEC-001 §6.3.1). The CLI tool calls that syscall and tracks user-visible role assignments in a userspace database.

4. `neo-intent` is preserved as a *userspace application* built on top of the kernel's AI syscalls and capability-passing IPC. It is not a kernel subsystem. The kernel does not depend on it; NEO boots and runs without it. It is scheduled for Phase 8+ in the application layer.

5. `design.md` is amended to v0.3 as a *vision document* — describing what NEO feels like to use and why it exists — and explicitly defers all architectural authority to the Bible and ANSPEC-001. Where the vision document and the Bible disagree, the Bible wins.

## Consequences

### Positive

- **Single source of architectural truth.** Contributors reading the Bible, ANSPEC-001, and `design.md` v0.3 see one consistent architecture. PRs no longer need to litigate the kernel model.
- **Inference performance.** Kernel-resident inference avoids per-call IPC and tensor-passing overhead. ANSPEC-001 §1.2 enumerates this rationale in detail.
- **Model deduplication is possible.** Kernel ownership of weight tensors enables content-hash deduplication across processes (ANSPEC-001 §8.2). A microkernel architecture, where each process loads its own model into its own daemon-managed memory, cannot do this safely.
- **Scheduler visibility.** The kernel's main scheduler can see inference workloads as first-class scheduling citizens (Bible Article 8.1, inference priority class). It cannot do this when inference happens behind an IPC boundary.
- **Vision document survives.** The aesthetic identity, the boot-sequence vision, the conversational shell, the privacy panel, the Intent Bus, model roles, and predictive model preloading all carry forward into v0.3 — reframed as application-layer features rather than kernel services. None of the v0.2 product vision is lost; it just lives at the right layer now.

### Negative

- **In-flight design assumed userspace services.** Any draft code, prototype work, or contributor onboarding material that assumed `neo-inference` as a daemon needs to be re-scoped before Phase 2. As of this ADR's date, no such code has been merged; the cost is bounded.
- **The "crashes are isolated by design" claim from v0.2 §1 is weaker.** A monolithic kernel has a single failure domain at the kernel level. Userspace processes are still isolated from each other (Bible Article 12), but a kernel bug in the inference subsystem brings down the system, where in a microkernel design it would only crash the `neo-inference` daemon. We accept this trade for the performance and coherence reasons above. Mitigation: kernel code is held to the discipline of Bible Article 15 — no `unwrap()` in steady-state paths, every `unsafe` annotated, formal review for MMIO and page-table code.
- **The v0.2 references to seL4, Redox, and Zircon as architectural inspirations are misleading and have been removed.** NeoPlutarch's architectural cousins are closer to Linux (monolithic, AI-native) and the BSDs (monolithic, disciplined) than to the microkernel family — though we owe nothing to either lineage's ABI.

### Neutral

- **The capability system (Bible Article 12.1, Phase 2) is unchanged.** Capabilities mediate access to kernel objects — tensors, models, inference handles, files — exactly as Article 12.1 prescribes. The kernel architecture (monolithic vs. microkernel) is orthogonal to whether security is capability-based. We are capability-based; we are also monolithic.
- **The userspace ABI is unchanged.** Applications still link `libneo` and call `libneo::ai::*`. They never see the IPC layer — because, post-ADR, there is no IPC layer for inference. The libneo wrapper goes directly to a syscall instead of marshaling an IPC request to `neo-inference`. Application code written against the v0.2 surface is mostly forward-compatible; the changes are below `libneo`'s exposed API.

## Alternatives Considered

### Alternative 1: Amend the Bible to match `design.md` v0.2 (microkernel)

Rejected. The Bible's monolithic-kernel rationale (Article 4.1) is grounded in the AI-native premise — IPC overhead is hostile to tensor-passing, scheduler blindness across an IPC boundary makes priority-aware inference impossible, and content-deduplication of weight tensors requires a kernel arbitrator (ANSPEC-001 §1.1). Reverting to a microkernel would invalidate the AI subsystem design at its foundation. The project's reason to exist — AI as a kernel primitive — collapses if AI lives behind IPC.

This alternative was considered seriously because Article 27 permits Bible amendment by PR with rationale. We do not exercise that option here because the rationale would have to argue against the project's central premise.

### Alternative 2: Hybrid — kernel-resident tensors and inference scheduler, but model execution in userspace

Rejected. This is what the v0.2 architecture implicitly was, and it inherits the worst of both worlds: kernel objects (tensors) handed across an IPC boundary to a userspace executor (`neo-inference`) which then dispatches to accelerators. Every inference call costs an IPC round-trip plus tensor marshaling, and the kernel's inference scheduler cannot directly dispatch to accelerators because it has to ask the daemon to do so. The architectural seam between kernel-tensor and userspace-executor becomes the system's bottleneck. Rejected on performance and coherence grounds.

### Alternative 3: Keep v0.2 framing in `design.md` and treat the Bible as aspirational

Rejected. Bible Article 27 explicitly states the bible is law: "When the bible and a pull request disagree, the pull request loses or the bible is amended — never both, never neither." Treating the Bible as aspirational while shipping a contradicting `design.md` violates the project's own governance. If the Bible cannot be relied on as authoritative, every other article in it loses force, and the project loses its discipline mechanism. Not a viable path.

### Alternative 4: Defer the decision until Phase 5

Rejected. Phase 2 lands the capability system and SMP. Capability-mediated access to AI objects (tensors, models, inference tickets, ANSPEC-001 §10.2) requires those objects to *exist* in the kernel — which requires the monolithic decision to be made before Phase 2 begins. Deferral is not actually possible without blocking Phase 2.

## Implementation

This ADR is implemented by the following actions, all completed or scheduled at the time of acceptance:

- [x] `docs/bible.md` v1.0.0 ratified with Article 4.1 stating monolithic kernel.
- [x] `docs/specs/neoplutarch-ai-subsystem-spec.md` (ANSPEC-001) authored on the monolithic premise.
- [x] `docs/design.md` rewritten as v0.3 vision document; v0.2 superseded.
- [ ] In-tree code references to `neo-inference` as a service are removed or relabeled (none currently exist; this is a forward-looking item).
- [ ] Contributor onboarding material (when authored, Phase 2+) cites this ADR as the architectural framing.
- [ ] Subsequent ADRs (planned: ADR-002 source layout migration, ADR-003 Phase 1 security debt) build on this decision.

## Notes

This ADR is unusually long because it is the first one and it resolves the project's foundational architectural ambiguity. Subsequent ADRs may be much shorter — the ADR format does not require length, only honesty.

The v0.2 → v0.3 transition is preserved in git history. Future contributors curious about why the project rejected microkernel architecture can read this ADR, the Bible Articles cited, and the v0.2 document for the alternative that was considered and rejected.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR. It is not amendable by drift.
