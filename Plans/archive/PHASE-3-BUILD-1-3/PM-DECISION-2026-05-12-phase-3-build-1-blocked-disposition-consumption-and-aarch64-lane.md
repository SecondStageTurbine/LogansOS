# PM Decision: BUILD-1 Blocked Disposition Consumption and aarch64 Lane Authorization

Date: 2026-05-12
Authority: PM
Inputs:

- `Plans/BLOCKED-DISPOSITION-2026-05-12-phase-3-build-1-aarch64.md`
- `Plans/PM-DECISION-2026-05-12-phase-3-build-1-1d-consumption-aarch64-blocked.md`
- `Plans/BLOCKER-2026-05-12-phase-3-build-1-aarch64-lane-jarvis.md`
- `Plans/RESEARCH-2026-05-12-phase-3-build-1-aarch64-lane-feasibility-cerebro.md`

## Decision

Prometheus's blocked-disposition note is accepted after correcting the rustfmt pinned-nightly typo to `nightly-2026-04-16`.

Current status remains:

```text
Phase 3 BUILD-1 x86_64 implementation complete; BUILD-1 closure blocked on aarch64 barrier-exposure lane.
```

PM chooses the next critical-path option: authorize a dedicated aarch64 barrier-exposure implementation lane to unblock BUILD-1 closure.

This is not BUILD-2. No NeoFS work is authorized.

## Rationale

The x86_64 BUILD-1 implementation is complete enough to make the remaining blocker crisp. ADR-021's aarch64 gate exists because x86 TSO cannot expose the relevant virtqueue barrier class. Leaving BUILD-1 open without starting the unblock lane would only preserve an already-understood blocker.

The aarch64 lane should be the smallest honest runtime lane that exercises the shared BUILD-1 virtqueue barrier discipline. It does not need to become a full aarch64 NeoPlutarch port.

## Authorized Dispatch

Create and ferry:

- `Plans/DISPATCH-2026-05-12-phase-3-build-1-aarch64-barrier-lane-jarvis.md`

Recipient: Jarvis

Branch: continue on `codex/phase-3-build-1-jarvis` unless Jarvis identifies a strong reason to split a dedicated branch.

## Success Criterion

The lane succeeds only if a real aarch64 QEMU runtime scenario passes and exercises:

- descriptor-publish ordering before `avail.idx`;
- notification-suppression ordering before reading `avail_event` or equivalent flags;
- virtio-blk data movement through a split virtqueue;
- a barrier wrapper shape shared with or directly auditing the x86_64 BUILD-1 virtio path.

Compile-only aarch64 evidence remains insufficient.

## Stop Rule

If the work expands into a broad aarch64 kernel port rather than a narrow closure-unblock lane, Jarvis must stop and report scope expansion before continuing.

## Prometheus / Cerebro

Prometheus stands by.

Cerebro has no immediate follow-up unless Jarvis hits a specific aarch64 implementation ambiguity.
