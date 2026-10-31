# ADR-014: Kernel Observability Deferred to Phase 8+

| | |
|---|---|
| **Status** | Accepted (deferral; Phase 8+ implementation) |
| **Date** | 2026-05-06 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Articles 12.1 (capability-based security), 12.4 (no in-kernel programmable hooks), 23.2 (no telemetry) · ANSPEC-001 §11 (AI-subsystem observability — separate scope) · ADR-013 (per-CPU schedulers — produces stats this ADR does not yet specify how to expose) · `docs/research/ALTIMIT-real-world-implementation.md` §K (KernelAttestation, sibling concern) |

---

## Context

The kernel's existing observability surface is ad-hoc: serial-print log lines (`[ kernel ] sched: ready ...`), boot-time diagnostic prints, frame leak audit output, and the AI event taxonomy in ANSPEC-001 §11. There is no programmatic introspection API.

Phase 1.5+ has accumulated real per-process and per-subsystem state — process registry, capability tables, slab cache statistics, scheduler queues, buddy allocator order histograms. None of it is queryable from userspace.

A future system monitor — analogous to Linux `top`/`htop` but capability-gated and NeoPlutarch-shaped — needs a programmatic surface. That surface is "kernel observability."

This ADR does not design that surface. It names the slot, enumerates the design questions a future ADR will need to answer, and explicitly defers the work to Phase 8+. The ALTIMIT research artifact's triage queue named "general kernel observability (deferred to Phase 8+, mark intent now)" — this ADR is the "mark intent now" act.

## Why deferred, not done now

- **Phase 2 has higher-priority work.** SMP, capabilities, allocators (ADRs 010, 012, 013) all sit between us and Phase 3+. Observability that doesn't exist yet doesn't bottleneck anything.
- **Premature design risk.** The right observability shape depends on what's actually being introspected. By Phase 8 we'll have stable AI subsystem internals (Phase 5–7), real accelerator drivers, and per-CPU scheduler internals that have proven their shape under load. Designing observability before its consumers exist invites rework.
- **Engineering parsimony.** Observability is a vector for security regressions (info-leak side channels, capability-table introspection that bypasses cap discipline). It is also a vector for ABI lock-in: once a `/proc`-equivalent ships, every script in userland depends on its exact shape. Better to ship deliberately and late than by accretion.

## Design questions a future ADR will answer

### Q1 — What is the introspection mechanism?

Syscall (`sys_proc_stats(pid, kind)`-shaped)? Pseudo-filesystem (a `procfs`-equivalent built on top of NeoFS once it exists, Phase 3+)? An opaque IPC service hosted by a privileged userspace daemon? Each has tradeoffs. The Bible's no-eBPF stance (Article 12.4) rules out programmable in-kernel queries; everything else is open.

### Q2 — What is the capability discipline?

Bible Article 12.1: every kernel object is referenced by a capability handle. Observability means querying state about objects the caller may not hold capabilities for. The simplest discipline: the caller queries only what it has caps for (per-process info for its own pid, per-channel info for its own handles). The most useful: a privileged "observer" capability grants system-wide read access. ADR-010's two-tier rights model is the substrate; future work picks the policy.

### Q3 — What is the field set?

What does the kernel expose, per subsystem? The list grows with each phase. Phase 8 starts from what exists then; this ADR just names the categories: scheduler stats, memory pressure, capability-table summary, process registry, IPC channel counts, slab cache stats, accelerator load. Each has its own per-field observability question.

### Q4 — What is the output format?

Structured (CBOR, MessagePack, native binary)? Line-oriented `key=value`-shaped (matching ANSPEC-001 §11.3)? Per-syscall opaque struct (matching ANSPEC-001 §5.5 `InfStats`)? Candidates include all three; the choice is Phase 8's.

### Q5 — What is the tracing relationship?

ANSPEC-001 §11.4 names "tracing hooks (likely Phase 7)" for inference profiling and rejects perf/eBPF. ADR-014's general-kernel tracing surface composes with that spec but is broader (it covers scheduler, IPC, and capability operations). Does general-kernel tracing get its own future ADR, or is it folded into the per-subsystem tracing ADRs as they land? Either is defensible. Punt to Phase 8.

### Q6 — Identity / attestation

How does the kernel prove its own identity to a remote verifier (or a local audit tool)? `KernelAttestation` per the research artifact §K. This ADR names this as related but does not design it; that's likely its own ADR at the time it ships.

### Q7 — Performance budget

Observability that costs measurable runtime (TLB shootdowns to read stats, cache-line bouncing across CPUs to maintain monitor counters) corrupts the system it claims to observe. Phase 8 design must specify the per-query and per-event cost budget. This ADR names the constraint; the number is Phase 8's.

## Anti-scope

- Does not amend Article 12.4 (no programmable kernel hooks). If a future observability design needs eBPF-shaped programmability, that requires a Bible amendment first.
- Does not amend Article 23.2 (no telemetry). Observability is for the local operator. Anything that would phone home is out of scope at the constitutional level, not just the ADR level.
- Does not adopt perf, eBPF, journald, syslog, OpenTelemetry, or Linux audit. ANSPEC-001 §11.3 already documents the rejection for AI-events; this ADR inherits it system-wide.
- Does not duplicate ANSPEC-001 §11. The AI subsystem's event taxonomy is its own surface; this ADR's future work covers everything else.
- Does not specify a `/proc` filesystem. NeoFS doesn't exist yet (Phase 3); discussions of FS-mediated observability wait until then.

## Cross-references

- Bible Article 12.1 (capability-based security) — the substrate for Q2's capability discipline.
- Bible Article 12.4 (no programmable kernel hooks) — load-bearing constraint; rules out eBPF-shaped in-kernel observability without a Bible amendment.
- Bible Article 23.2 (no telemetry) — load-bearing constraint; observability is local-only.
- ADR-010 (capability system foundation) — the two-tier rights model future observer caps would extend.
- ANSPEC-001 §11 (AI subsystem observability) — sibling spec; this ADR does not duplicate it.
- ANSPEC-001 §6.3.4 (stack-frame discipline for blocking syscalls) — the same composability concern (kernel-resident state held across blocking points) that observability queries against.
- `docs/research/ALTIMIT-real-world-implementation.md` §K (KernelAttestation, identity) — sibling future ADR; shares a "trustworthy introspection" shape but is a separable concern.
- ADR-013 §Q1 (per-CPU state shape) — produces the per-CPU stats this ADR's future work would expose.

## 2026-06-20 aggregate status exception

The `sys_sysstat` syscall is an aggregate-only exception for the interactive
`neo-shell status` demo. It returns fixed system-wide counters only:
registered user-process count, frames and bytes in use, online core count, and
uptime ticks. It returns no process ids, names, capability-table contents,
thread identities, or per-process rows.

This does not close ADR-014 and does not ship the deferred `/proc`, `ps`, or
capability-scoped observability design. It is the first tiny slice of the
future observability surface, gated by the default-off `kernel-sysstat`
kill-switch and deliberately scoped so the deferred OT-9/ADR-014 process
enumeration questions remain unanswered.

## 2026-07-05 OT-9 aggregate scheduler and memory slice

The `SYS_OBSERVE` v3 snapshot is the first OT-9 implementation slice following
the proven aggregate-counter pattern. It remains default-off behind
`kernel-observability` and exposes read-only system aggregates only:
per-CPU scheduler counters (context switches, dispatches, timer preemptions,
voluntary yields, run/idle ticks, ready-queue depth/high-water), heap
current/high-water/allocation-failure counters, frame allocator used/free
summary fields, and a fixed overwrite-oldest trace ring for dispatch,
page-fault, and syscall-class events.

This partially answers Q1, Q3, Q4, and Q7 for aggregate counters:

- **Q1 mechanism:** a fixed-size `SYS_OBSERVE` syscall snapshot is accepted for
  this slice only.
- **Q3 field set:** the slice covers scheduler stats and memory pressure.
- **Q4 output format:** the kernel/user ABI is a native `#[repr(C)]` binary
  struct, with `neo-shell status` providing the human-facing text surface.
- **Q5 tracing relationship:** the slice adds only a fixed event ring with
  enumerated dispatch/page-fault/syscall event kinds; programmable tracing and
  per-subsystem tracing policy remain deferred.
- **Q7 performance budget:** event producers are relaxed atomics; timer-ISR
  additions are single lock-free atomic updates and snapshots take allocator
  reads only on the explicit query path.

Q2 capability discipline remains unanswered beyond the present default-off
feature gate. Q6 identity/attestation is untouched, and per-principal/process
attribution is explicitly deferred to a future OT-9/OT-15 slice. These counters
and trace samples are not actuators: no kernel scheduling, allocation, or policy
decision reads them.

The 2026-07-09 Rule-4 consolidation added the missing host-side ABI
byte-oracle: `tools/observability-abi-tests` independently pins
`KernelObservabilityV1`, `SchedulerStatsV1`, and `TraceEventV1` field order,
offsets, array strides, and representative bytes against both kernel and libneo
source definitions. The syscall size handshake still catches old binaries; this
oracle catches a same-size field reorder.

## Notes

This ADR commits to one thing: kernel observability is deliberate, not forgotten. The design questions in §"Design questions" are the minimum set a successor ADR must answer before shipping any introspection surface. Future PRs that add one-off observability hooks (e.g., "just a quick stats syscall") should be measured against this ADR — if the hook doesn't follow from answers to Q1–Q7, it's premature and should wait for the Phase 8 design conversation.

ADR-009 is the precedent: bare-metal verification was deliberately deferred at Phase 1.5c close with conditions for revisiting recorded in the ADR. This ADR is the same shape applied to kernel observability.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR.
